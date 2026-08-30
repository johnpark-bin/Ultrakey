//! 절전 · 깨어남 · 세션전환 · 화면 잠금 · 핫플러그 · 입력 소스 변경 훅.
//! `key-remapping-engine.md` §3-a, §5#3~#5, #10, #11.
//!
//! ⚠️ 이 모듈의 구독 등록(`observe_system_events`/`watch_keyboards`/
//! `observe_input_source_changes`)은 **호출한 스레드의 `CFRunLoop`** 에 걸린다
//! (`ultrakey-platform` 각 모듈 문서 참고). `docs/dev/architecture.md` §2.1 배치상 이
//! 스레드는 Tauri/`NSApplication` 이 소유한 **메인 스레드**여야 한다 — [`SystemHooks::start`]
//! 는 `Engine::start` 를 호출한 바로 그 스레드에서(탭 전용 스레드를 새로 만들지 않고)
//! 동기적으로 실행되어야 이 전제가 성립한다.
//!
//! ⚠️ **지연은 스레드를 새로 띄우지 않는다.** 지연 전용 스레드 하나
//! ([`DelayScheduler`])가 `crossbeam_channel::recv_timeout` 으로 예약된 작업의 마감을
//! 관리한다 — 이벤트마다 `thread::spawn` 하지 않는다.

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};

use ultrakey_core::korean::classify_input_source_languages;
use ultrakey_core::perdevice::DeviceId;
use ultrakey_platform::hotplug::{watch_keyboards, HotplugEvent, HotplugEventKind, KeyboardHotplugWatcher};
use ultrakey_platform::text_input_source::{observe_input_source_changes, InputSourceObserver};
use ultrakey_platform::workspace::{observe_system_events, SystemEvent, SystemEventObserver};

use crate::command::{CommandChannel, EngineCommand};
use crate::lifecycle::should_skip_restart;
use crate::state::SharedState;

/// 지연 스케줄러에 예약하는 작업 종류.
///
/// ⚠️ `ReapplyHidMapping` 이 `Option<DeviceId>` 를 실으므로 더 이상 `Copy` 가 아니다.
#[derive(Debug, Clone, PartialEq, Eq)]
enum DelayedJob {
    /// 절전 복귀·세션 활성화·화면 잠금 해제 → 지연 뒤 `RecoverTap`. ⭐ 재시작
    /// 디바운스가 적용된다(§3-a).
    Recover,
    /// 키보드 핫플러그(연결) → 지연 뒤 `ReapplyHidMapping(device)`. 경로 B 는 경로 A 와
    /// 다른 자원이라 디바운스를 공유하지 않는다. `Some(device)` 면 그 디바이스 하나만,
    /// `None` 이면(디바이스 속성을 읽지 못한 경우, `HotplugEvent::device` 문서 참고)
    /// 전체 재조정으로 대응한다.
    ReapplyHidMapping(Option<DeviceId>),
}

enum SchedulerMsg {
    Schedule { after_ms: u64, job: DelayedJob },
    Shutdown,
}

/// [`DelayScheduler`] 로 작업을 예약하고, 탭 스레드로 즉시 명령을 보낼 수 있는 손잡이.
/// `Clone` 이 가능해 시스템 이벤트 콜백마다 자유롭게 복제해 캡처할 수 있다.
#[derive(Clone)]
pub struct DelaySchedulerHandle {
    sender: Sender<SchedulerMsg>,
    commands: CommandChannel,
    shared: Arc<SharedState>,
}

impl DelaySchedulerHandle {
    pub fn commands(&self) -> &CommandChannel {
        &self.commands
    }

    pub fn shared(&self) -> &Arc<SharedState> {
        &self.shared
    }

    fn schedule(&self, after_ms: u64, job: DelayedJob) {
        let _ = self.sender.send(SchedulerMsg::Schedule { after_ms, job });
    }
}

/// 지연 전용 스레드 하나 — 절전/세션/핫플러그 복구를 `recv_timeout` 기반으로 예약한다.
pub struct DelayScheduler {
    thread: Option<JoinHandle<()>>,
    handle: DelaySchedulerHandle,
}

impl DelayScheduler {
    pub fn spawn(shared: Arc<SharedState>, commands: CommandChannel) -> Self {
        let (tx, rx) = unbounded::<SchedulerMsg>();
        let handle = DelaySchedulerHandle {
            sender: tx,
            commands: commands.clone(),
            shared: Arc::clone(&shared),
        };
        let thread = thread::Builder::new()
            .name("ultrakey-delay-scheduler".to_string())
            .spawn(move || scheduler_loop(rx, shared, commands))
            .expect("지연 스케줄러 스레드 생성 실패");
        DelayScheduler {
            thread: Some(thread),
            handle,
        }
    }

    pub fn handle(&self) -> DelaySchedulerHandle {
        self.handle.clone()
    }

    pub fn shutdown(mut self) {
        let _ = self.handle.sender.send(SchedulerMsg::Shutdown);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct PendingJob {
    fire_at: Instant,
    job: DelayedJob,
}

fn scheduler_loop(rx: Receiver<SchedulerMsg>, shared: Arc<SharedState>, commands: CommandChannel) {
    let clock_start = Instant::now();
    let mut pending: Vec<PendingJob> = Vec::new();
    let mut last_recover_fired_ms: Option<u64> = None;

    loop {
        let timeout = pending
            .iter()
            .map(|p| p.fire_at)
            .min()
            .map(|t| t.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_secs(3600));

        match rx.recv_timeout(timeout) {
            Ok(SchedulerMsg::Schedule { after_ms, job }) => {
                pending.push(PendingJob {
                    fire_at: Instant::now() + Duration::from_millis(after_ms),
                    job,
                });
                continue; // 새 마감이 생겼으니 다음 timeout 을 다시 계산한다.
            }
            Ok(SchedulerMsg::Shutdown) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }

        let now = Instant::now();
        let now_ms = now.duration_since(clock_start).as_millis() as u64;
        let mut i = 0;
        while i < pending.len() {
            if pending[i].fire_at <= now {
                let p = pending.remove(i);
                fire_job(
                    p.job,
                    &shared,
                    &commands,
                    &mut last_recover_fired_ms,
                    now_ms,
                );
            } else {
                i += 1;
            }
        }
    }
    tracing::debug!("지연 스케줄러 스레드 종료");
}

fn fire_job(
    job: DelayedJob,
    shared: &Arc<SharedState>,
    commands: &CommandChannel,
    last_recover_fired_ms: &mut Option<u64>,
    now_ms: u64,
) {
    match job {
        DelayedJob::Recover => {
            let debounce_ms = shared.config.load().timings.restart_debounce_ms;
            if should_skip_restart(*last_recover_fired_ms, now_ms, debounce_ms) {
                let elapsed_ms = last_recover_fired_ms.map(|last| now_ms.saturating_sub(last));
                // ⭐ `info` 다(`debug` 아님). `manual-verification.md` 1-b #4·#5 가
                // "디바운스로 재시작을 건너뛴 판정이 로그에 남는다" 를 **통과 근거로
                // 요구**하는데, `debug` 면 기본 레벨에서 보이지 않아 검증자가
                // "디바운스가 동작했다" 와 "그런 로직이 아예 없다" 를 구분할 수 없다.
                // 빈도도 낮다 — 5초 안에 복구가 두 번 예약될 때만 찍힌다.
                tracing::info!(
                    elapsed_ms,
                    debounce_ms,
                    "직전 복구로부터 얼마 지나지 않아 재확인을 디바운스로 건너뛴다"
                );
                return;
            }
            *last_recover_fired_ms = Some(now_ms);
            commands.send(EngineCommand::RecoverTap);
        }
        DelayedJob::ReapplyHidMapping(device) => {
            commands.send(EngineCommand::ReapplyHidMapping(device));
        }
    }
}

/// 시스템 훅 전체 — 옵저버 3종 + 지연 스케줄러를 묶어 생애주기를 관리한다.
pub struct SystemHooks {
    _system_observer: SystemEventObserver,
    _hotplug_watcher: Option<KeyboardHotplugWatcher>,
    _input_source_observer: InputSourceObserver,
    scheduler: DelayScheduler,
}

impl SystemHooks {
    /// ⚠️ **호출한 스레드에서 동기적으로** 구독을 등록한다 — 이 스레드가 곧 메인
    /// 스레드여야 한다(위 모듈 문서 참고). 절대 이 함수 자체를 새 스레드에서 부르지 마라.
    pub fn start(shared: Arc<SharedState>, commands: CommandChannel) -> Self {
        warn_if_not_main_thread();

        let scheduler = DelayScheduler::spawn(Arc::clone(&shared), commands);
        let sched_handle = scheduler.handle();

        let sched_for_events = sched_handle.clone();
        let observer = observe_system_events(Box::new(move |ev| {
            handle_system_event(ev, &sched_for_events);
        }));

        let sched_for_hotplug = sched_handle.clone();
        let hotplug = watch_keyboards(Box::new(move |ev| {
            handle_hotplug_event(ev, &sched_for_hotplug);
        }));

        // ⭐ 기동 시 1회 — 옵저버 등록 *직후* 같은 절차를 동기적으로 실행한다. 이 함수는
        // (이 시점 기준) 메인 스레드에서 불리고 있으므로 `TIS*` 호출 규약을 지킨다.
        // 여기서 하지 않으면 사용자가 입력 소스를 최소 한 번 바꾸기 전까지 레이아웃
        // 테이블이 비어 있고, F-16 게이트는 `Unknown` 인 채로 fail-closed 되어
        // 영원히 발화하지 않는다(이 모듈이 고친 결함 — module doc 참고).
        refresh_input_source(&shared);

        let shared_for_layout = Arc::clone(&shared);
        let input_observer = observe_input_source_changes(Box::new(move || {
            refresh_input_source(&shared_for_layout);
        }));

        SystemHooks {
            _system_observer: observer,
            _hotplug_watcher: hotplug,
            _input_source_observer: input_observer,
            scheduler,
        }
    }

    /// ⚠️ 스레드를 누수시키지 않는다 — 지연 스케줄러 스레드를 정리한다. 옵저버 3종은 이
    /// 구조체가 드롭되며 각자의 `Drop` 이 구독을 해지한다.
    pub fn shutdown(self) {
        self.scheduler.shutdown();
    }
}

/// ⭐ **스레드 친화성 전제가 깨졌는지 소리 내어 알린다**(이슈 #10).
///
/// 이 모듈 문서가 요구하는 "메인 스레드" 전제는 지금껏 **주석으로만** 존재했고,
/// 앱 계층이 그것을 어겼을 때 아무 신호도 나지 않았다 — 기능(핫플러그 감지)만
/// 조용히 죽었다. 그 침묵이 이 버그의 본질이었다.
///
/// ⚠️ 여기서 **패닉하지 않는다.** 이 함수가 발견하는 위반은 M1 시점에서는
/// 치명적이지 않다(핫플러그는 [`watch_keyboards`] 가 자체 런루프 스레드를 갖도록
/// 고쳐졌고, 나머지 두 훅은 등록 스레드와 무관하게 메인 스레드로 전달된다).
/// 앱을 죽이는 대신 **경고 한 줄**을 남겨, 앞으로 스레드 친화적 훅이 추가될 때
/// 그 사실이 로그에서 즉시 보이게 한다. 디버그 빌드에서는 `debug_assert!` 가
/// 테스트·개발 중에 더 강하게 잡는다.
fn warn_if_not_main_thread() {
    if ultrakey_platform::runloop::is_main_thread() {
        return;
    }
    let thread = std::thread::current();
    tracing::warn!(
        thread = thread.name().unwrap_or("<이름 없음>"),
        thread_id = ?thread.id(),
        "⚠️ SystemHooks::start 가 메인 스레드가 아닌 곳에서 호출됐다 — \
         이 모듈 문서(architecture.md §2.1)가 요구하는 전제를 앱 계층이 어겼다. \
         런루프에 직접 소스를 거는 훅이 추가되면 조용히 동작하지 않게 된다"
    );
    debug_assert!(
        false,
        "SystemHooks::start 는 메인 스레드에서 호출되어야 한다(system_hooks.rs 모듈 문서)"
    );
}

/// 레이아웃 테이블을 재구축하고 한국어 IME 게이트를 게시한다.
///
/// ⭐ **기동 시 1회(`SystemHooks::start`)와 입력 소스 변경 알림(옵저버 콜백)이 이
/// 함수 하나를 공유한다** — 두 벌로 복제하면 한쪽만 고쳐지는 사고가 난다(이 저장소가
/// 이미 겪은 계열, `docs/spec/korean-input.md` §7). 알림이 올 때만 재구축하던 기존
/// 결함 탓에 기동 직후에는 레이아웃 테이블이 한 번도 만들어지지 않아, 사용자가 입력
/// 소스를 바꾸기 전까지 게이트가 `Unknown` 인 채로 fail-closed 되어 F-16.4 가 영원히
/// 발화하지 않았다.
///
/// ⚠️ 콜백이 아니라 메인 스레드 동기 호출이므로 `tracing` 로깅 제약(§2.2)이 적용되지
/// 않는다 — `original_source_id`·`languages[0]`·판정 결과를 남긴다. 실기기 검증
/// (Phase 7)이 이 로그를 읽는다.
fn refresh_input_source(shared: &SharedState) {
    let rebuilt = shared.layout.rebuild();
    let table = shared.layout.current();
    let languages = table.original_languages();
    let state = classify_input_source_languages(languages);
    shared.korean_ime.store(state);

    tracing::info!(
        rebuilt,
        used_ascii_fallback = table.used_ascii_fallback(),
        source_id = table.source_id(),
        original_source_id = table.original_source_id(),
        first_language = languages.first().map(String::as_str).unwrap_or(""),
        korean_ime = ?state,
        "입력 소스 갱신 — 레이아웃 테이블 재구축 + 한국어 IME 게이트 게시"
    );
}

fn handle_system_event(ev: SystemEvent, sched: &DelaySchedulerHandle) {
    match ev {
        // ⭐ 즉시 ForceResetState — stuck modifier 방지(§5#9). 탭 자체는 유지한다.
        SystemEvent::WillSleep => {
            tracing::info!("절전 진입 알림 수신 — 즉시 상태를 강제 리셋한다");
            sched.commands().send(EngineCommand::ForceResetState);
        }
        SystemEvent::ScreenLocked => {
            tracing::info!("화면 잠금 알림 수신 — 즉시 상태를 강제 리셋한다");
            sched.commands().send(EngineCommand::ForceResetState);
        }
        SystemEvent::SessionDidResignActive => {
            tracing::info!("세션 비활성화 알림 수신 — 즉시 상태를 강제 리셋한다");
            sched.commands().send(EngineCommand::ForceResetState);
        }
        // ⭐ 즉시 재확인하지 않는다 — 지연 뒤 RecoverTap(§3-a).
        SystemEvent::DidWake => {
            let delay = sched.shared().config.load().timings.wake_delay_ms;
            tracing::info!(
                delay_ms = delay,
                "절전 복귀 알림 수신 — 지연 후 탭 재확인을 예약한다"
            );
            sched.schedule(delay, DelayedJob::Recover);
        }
        SystemEvent::ScreenUnlocked => {
            let delay = sched.shared().config.load().timings.session_delay_ms;
            tracing::info!(
                delay_ms = delay,
                "화면 잠금 해제 알림 수신 — 지연 후 탭 재확인을 예약한다"
            );
            sched.schedule(delay, DelayedJob::Recover);
        }
        SystemEvent::SessionDidBecomeActive => {
            let delay = sched.shared().config.load().timings.session_delay_ms;
            tracing::info!(
                delay_ms = delay,
                "세션 활성화 알림 수신 — 지연 후 탭 재확인을 예약한다"
            );
            sched.schedule(delay, DelayedJob::Recover);
        }
        SystemEvent::FrontAppChanged(ident) => {
            // ⚠️ `AppGateController::set_front_app` 호출은 F-10 이 소유한 컨트롤러가
            // 필요하다. `Engine::start` 는 읽기 전용 `Arc<AtomicAppGate>` 만 받으므로
            // (설계 §2.3), 이 엔진 크레이트는 그 컨트롤러에 접근할 수 없다 — 최전면 앱
            // 변경을 게이트에 실제로 반영하는 배선은 F-10/앱 계층이 별도로
            // `NSWorkspaceDidActivateApplicationNotification` 을 구독해 자신의
            // `AppGateController::set_front_app` 을 호출하는 방식으로 이루어져야 한다.
            // 여기서는 관측 로그만 남긴다(§3-f 판정 로직 자체는 core::gate 가 갖고 있다).
            let bundle_id = ident.map(|a| a.bundle_id).unwrap_or_default();
            tracing::debug!(
                bundle_id,
                "최전면 앱 변경 감지(관측만 — 게이트 갱신은 F-10 소관)"
            );
        }
    }
}

/// ⭐ CONTRACT.md 부록 B.5 — `Attached` 만 재적용을 예약한다. `Detached` 는 쓸 대상이
/// 사라졌으므로(뽑힌 디바이스에는 `--matching` 이 애초에 아무 서비스도 찾지 못한다)
/// **쓰지 않는다** — 로그만 남긴다. 원장은 그대로 둔다(§3.6 규칙 6 — 다음에 그
/// 디바이스가 다시 붙을 때 정리한다).
fn handle_hotplug_event(ev: HotplugEvent, sched: &DelaySchedulerHandle) {
    match ev.kind {
        HotplugEventKind::Attached => {
            let delay = sched
                .shared()
                .config
                .load()
                .timings
                .keyboard_connect_delay_ms;
            let device = ev
                .device
                .as_ref()
                .map(|info| DeviceId::new(info.vendor_id, info.product_id));
            tracing::info!(
                delay_ms = delay,
                device = device.as_ref().map(DeviceId::as_str).unwrap_or("<알 수 없음>"),
                "외장 키보드 연결 감지 — 지연 후 경로 B(F-17) 재적용을 예약한다"
            );
            sched.schedule(delay, DelayedJob::ReapplyHidMapping(device));
        }
        HotplugEventKind::Detached => {
            tracing::info!(
                "외장 키보드 해제 감지 — 쓸 대상이 사라졌으므로 경로 B 재적용을 예약하지 \
                 않는다(원장은 그대로 둔다, §3.6 규칙 6)"
            );
        }
    }
}
