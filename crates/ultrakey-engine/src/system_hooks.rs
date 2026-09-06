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

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, RecvTimeoutError, Sender};

use ultrakey_core::korean::{classify_input_source_languages, classify_input_source_languages_for};
use ultrakey_core::perdevice::DeviceId;
use ultrakey_platform::hotplug::{watch_keyboards, HotplugEvent, HotplugEventKind, KeyboardHotplugWatcher};
use ultrakey_platform::text_input_source::{observe_input_source_changes, InputSourceObserver};
use ultrakey_platform::workspace::{observe_system_events, SystemEvent, SystemEventObserver};

use crate::command::{CommandChannel, EngineCommand};
use crate::lifecycle::should_skip_restart;
use crate::path_b::PathBManager;
use crate::state::SharedState;

/// 지연 스케줄러에 예약하는 작업 종류.
///
/// ⚠️ `ReapplyHidMapping` 이 `Option<DeviceId>` 를 실으므로 더 이상 `Copy` 가 아니다.
///
/// ⭐ 이슈 #139 — `ReapplyHidMapping` 은 더 이상 `EngineCommand` 로 번역되지 않는다.
/// 이 스케줄러 스레드가 `fire_job` 안에서 `PathBManager::apply_device`/`apply_all` 을
/// **직접** 실행한다 — 탭 런루프 시간에 `hidutil` 서브프로세스·원장 fsync 를 넣지
/// 않기 위함이다(이슈 #139, `docs/dev/architecture.md` §2.2).
#[derive(Debug, Clone, PartialEq, Eq)]
enum DelayedJob {
    /// 절전 복귀·세션 활성화·화면 잠금 해제 → 지연 뒤 `RecoverTap`. ⭐ 재시작
    /// 디바운스가 적용된다(§3-a).
    Recover,
    /// 키보드 핫플러그(연결) → 지연 뒤 경로 B 재적용. 경로 B 는 경로 A 와 다른
    /// 자원이라 디바운스를 공유하지 않는다. `Some(device)` 면 그 디바이스 하나만,
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
    pub fn spawn(
        shared: Arc<SharedState>,
        commands: CommandChannel,
        path_b: Arc<PathBManager>,
    ) -> Self {
        let (tx, rx) = unbounded::<SchedulerMsg>();
        let handle = DelaySchedulerHandle {
            sender: tx,
            commands: commands.clone(),
            shared: Arc::clone(&shared),
        };
        let thread = thread::Builder::new()
            .name("ultrakey-delay-scheduler".to_string())
            .spawn(move || scheduler_loop(rx, shared, commands, path_b))
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

fn scheduler_loop(
    rx: Receiver<SchedulerMsg>,
    shared: Arc<SharedState>,
    commands: CommandChannel,
    path_b: Arc<PathBManager>,
) {
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
                    &path_b,
                    &mut last_recover_fired_ms,
                    now_ms,
                );
            } else {
                i += 1;
            }
        }
    }
    tracing::debug!("delay scheduler thread exiting");
}

fn fire_job(
    job: DelayedJob,
    shared: &Arc<SharedState>,
    commands: &CommandChannel,
    path_b: &Arc<PathBManager>,
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
                    "skipping recheck via debounce; too soon after the previous recovery"
                );
                return;
            }
            *last_recover_fired_ms = Some(now_ms);
            commands.send(EngineCommand::RecoverTap);
        }
        DelayedJob::ReapplyHidMapping(device) => {
            // ⭐ 이슈 #139 — 재적용은 이 스케줄러 스레드가 직접 실행한다. `EngineCommand`
            // 로 번역해 탭 스레드로 넘기지 않는다 — `hidutil` 서브프로세스·원장 fsync 를
            // 탭 런루프 시간에 넣지 않기 위함이다(architecture.md §2.2).
            //
            // ⚠️ 트레이드오프(이슈 #139 P3 심사 지적): 이 실행이 스케줄러 스레드를
            // 재적용 시간만큼(실측 1대 179 ms, 전체 재적용은 디바이스 수에 비례) 점유하므로
            // 그 사이 도착한 다른 지연 작업(`Recover`)의 발화가 그만큼 밀린다. 같은
            // 깨어남의 `Recover` 는 이 작업보다 먼저 예약되어 먼저 발화하고, 나머지는
            // 이미 300~2000 ms 지연 재확인이라 수백 ms 추가 지연은 무해하다 — 재적용
            // 시간이 크게 늘어나면 이 판단을 다시 하라.
            //
            // `list_attached_keyboards()` 는 `PathBManager::serial` 잠금 **밖**에서 열거한다
            // (`Engine::reconfigure` 와 동일). 잠금 대기 중 목록이 낡을 수 있지만 그 영향은
            // "방금 뽑힌 디바이스 쓰기 실패 로그 후 계속 / 방금 붙은 디바이스는 자기
            // 핫플러그 재적용이 따로 예약됨" 뿐이라 감수한다.
            let started = Instant::now();
            let cfg = shared.config.load_full();
            let (result, devices) = match &device {
                Some(dev) => (path_b.apply_device(&cfg, dev), 1usize),
                None => {
                    let attached = ultrakey_platform::hid_device::list_attached_keyboards();
                    let devices = attached.len();
                    (path_b.apply_all(&cfg, &attached), devices)
                }
            };
            shared
                .d1_confirmed
                .store(path_b.d1_confirmed(), Ordering::Release);
            let elapsed_ms = started.elapsed().as_millis() as u64;
            match result {
                Ok(()) => tracing::info!(
                    ?device,
                    elapsed_ms,
                    devices,
                    "Path B (F-17) reapply completed"
                ),
                Err(e) => tracing::warn!(
                    error = %e,
                    ?device,
                    elapsed_ms,
                    devices,
                    "Path B (F-17) reapply failed"
                ),
            }
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
    pub fn start(
        shared: Arc<SharedState>,
        commands: CommandChannel,
        path_b: Arc<PathBManager>,
    ) -> Self {
        warn_if_not_main_thread();

        let scheduler = DelayScheduler::spawn(Arc::clone(&shared), commands, path_b);
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
        thread = thread.name().unwrap_or("<unnamed>"),
        thread_id = ?thread.id(),
        "SystemHooks::start was called off the main thread; the app layer violated the \
         precondition this module documents (architecture.md §2.1). Hooks that attach \
         sources directly to the run loop will silently stop working"
    );
    debug_assert!(
        false,
        "SystemHooks::start 는 메인 스레드에서 호출되어야 한다(system_hooks.rs 모듈 문서)"
    );
}

/// 레이아웃 테이블을 재구축하고 한국어·일본어 IME 게이트와 키보드 타입 게이트를
/// 게시한다.
///
/// ⭐ **기동 시 1회(`SystemHooks::start`)와 입력 소스 변경 알림(옵저버 콜백)이 이
/// 함수 하나를 공유한다** — 두 벌로 복제하면 한쪽만 고쳐지는 사고가 난다(이 저장소가
/// 이미 겪은 계열, `docs/spec/korean-input.md` §7). 알림이 올 때만 재구축하던 기존
/// 결함 탓에 기동 직후에는 레이아웃 테이블이 한 번도 만들어지지 않아, 사용자가 입력
/// 소스를 바꾸기 전까지 게이트가 `Unknown` 인 채로 fail-closed 되어 F-16.4 가 영원히
/// 발화하지 않았다.
///
/// ⭐ F-19(D-7·D-4): 같은 함수가 ①일본어 IME 판정(`japanese_ime`) ②키보드 타입 판정
/// (`is_jis`) 도 함께 게시한다 — 입력 소스 변경은 키보드 타입이 바뀐다는 뜻은 아니지만,
/// keycode 상수값이 확정된 유일한 기동 시점이기도 하고 두 게이트 모두 "게시한 값"만
/// 의미가 있어 한 곳에서 갱신하는 것이 안전하다(키보드 핫플러그 시에도 이 함수는
/// `hotplug`가 재호출한다).
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

    // ⭐ F-19(D-7) — 일본어 입력기 활성. `classify_input_source_languages_for` 로
    // 같은 언어 목록에서 "ja" 를 판정한다(첫 원소만 보는 규약 동일).
    let japanese_state = classify_input_source_languages_for(languages, "ja");
    shared.japanese_ime.store(japanese_state);

    // ⭐ F-19(D-4) — 키보드 타입. 판정 실패(`None`)면 Unknown 을 게시해 fail-closed
    // 로 남긴다 — JIS·US 행 모두 미발화(둘 다 파괴적인 실패 모드다, `jis.rs` 문서).
    let is_jis = ultrakey_platform::keyboard_type::current_keyboard_is_jis();
    shared.is_jis.store(match is_jis {
        Some(true) => ultrakey_core::jis::JisState::Jis,
        Some(false) => ultrakey_core::jis::JisState::NotJis,
        None => ultrakey_core::jis::JisState::Unknown,
    });

    tracing::info!(
        rebuilt,
        used_ascii_fallback = table.used_ascii_fallback(),
        source_id = table.source_id(),
        original_source_id = table.original_source_id(),
        first_language = languages.first().map(String::as_str).unwrap_or(""),
        korean_ime = ?state,
        japanese_ime = ?japanese_state,
        is_jis = ?is_jis,
        "input source refreshed; layout table rebuilt and IME/keyboard-type gates published"
    );
}

fn handle_system_event(ev: SystemEvent, sched: &DelaySchedulerHandle) {
    match ev {
        // ⭐ 즉시 ForceResetState — stuck modifier 방지(§5#9). 탭 자체는 유지한다.
        SystemEvent::WillSleep => {
            tracing::info!("sleep notification received; forcing immediate state reset");
            sched.commands().send(EngineCommand::ForceResetState);
        }
        SystemEvent::ScreenLocked => {
            tracing::info!("screen lock notification received; forcing immediate state reset");
            sched.commands().send(EngineCommand::ForceResetState);
        }
        SystemEvent::SessionDidResignActive => {
            tracing::info!("session deactivation notification received; forcing immediate state reset");
            sched.commands().send(EngineCommand::ForceResetState);
        }
        // ⭐ 즉시 재확인하지 않는다 — 지연 뒤 RecoverTap(§3-a).
        SystemEvent::DidWake => {
            let delay = sched.shared().config.load().timings.wake_delay_ms;
            tracing::info!(
                delay_ms = delay,
                "wake notification received; scheduling a delayed tap recheck"
            );
            sched.schedule(delay, DelayedJob::Recover);
            // ⭐ 이슈 #108 원인 (a) — 절전 중 커널이 경로 B(D-1 포함) 매핑을 유실했을
            // 가능성을 닫는다(BT 키보드가 핫플러그 재열거 없이 재개되는 경우 등,
            // 검증 불가능한 가설이라 계측 대신 재적용으로 닫는다). `apply_all` 은
            // 멱등이다. ⭐ 이슈 #139 — 재적용은 이 스케줄러 스레드가 직접 실행한다 —
            // 탭 런루프 시간에 `hidutil` 서브프로세스·원장 fsync 를 넣지 않기 위함이다
            // (architecture.md §2.2).
            sched.schedule(delay, DelayedJob::ReapplyHidMapping(None));
        }
        SystemEvent::ScreenUnlocked => {
            let delay = sched.shared().config.load().timings.session_delay_ms;
            tracing::info!(
                delay_ms = delay,
                "screen unlock notification received; scheduling a delayed tap recheck"
            );
            sched.schedule(delay, DelayedJob::Recover);
        }
        SystemEvent::SessionDidBecomeActive => {
            let delay = sched.shared().config.load().timings.session_delay_ms;
            tracing::info!(
                delay_ms = delay,
                "session activation notification received; scheduling a delayed tap recheck"
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
                "front app change detected (observation only; gate update is F-10's job)"
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
                device = device.as_ref().map(DeviceId::as_str).unwrap_or("<unknown>"),
                "external keyboard attach detected; scheduling a delayed Path B (F-17) reapply"
            );
            sched.schedule(delay, DelayedJob::ReapplyHidMapping(device));
        }
        HotplugEventKind::Detached => {
            tracing::info!(
                "external keyboard detach detected; not scheduling a Path B reapply since \
                 there is nothing left to write to (the ledger is left as-is, §3.6 rule 6)"
            );
        }
    }
}
