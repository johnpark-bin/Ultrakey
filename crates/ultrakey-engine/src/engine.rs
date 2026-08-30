//! 조립 — `Engine` 공개 API, 탭 전용 스레드 본체, `CGEventTap` 콜백.
//!
//! ⛔ **콜백 안에서 금지**(`docs/dev/architecture.md` §2.2): `Mutex`/`RwLock` 획득,
//! 힙 할당(`Vec::new`, `String`, `format!`), 파일·네트워크 I/O, 동기 로깅(`tracing`
//! 매크로 포함 — 포맷팅이 할당한다). 아래 [`on_tap_event`] 가 그 콜백이다 — 이 함수와
//! 이 함수가 직접 부르는 것들은 이 규칙을 지킨다. 탭 비활성화 통지(0-b)를 받았을 때도
//! 로깅·`on_event` 호출 같은 무거운 일은 `commands.send(...)` 로 커맨드 큐에 위임하고
//! 콜백 자신은 즉시 리턴한다.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Instant;

use arc_swap::ArcSwapOption;
use crossbeam_channel::{bounded, Receiver, Sender};

use ultrakey_core::arbitration::{
    resolve_caps_lock_alias_for_trace, Arbiter, Disposition, Effect, Outcome, SynthEvent,
};
use ultrakey_core::event::{EventKind, InputEvent};
use ultrakey_core::gate::{AppGate, AtomicAppGate};
use ultrakey_core::settings::EngineConfig;
use ultrakey_core::time::Millis;

use ultrakey_platform::event::{CgEventRef, SyntheticEvent};
use ultrakey_platform::event_tap::{
    EventTap, TapAction, TapCallback, TapCreateError, TapHealthProbe, TapProxy,
};
use ultrakey_platform::hid_mapping::HidutilBackend;
use ultrakey_platform::runloop::{CommandSource, RepeatingTimer, RunLoopConfined, RunLoopHandle};
use ultrakey_platform::secure_input::{DirectSecureInputProbe, SecureInputProbe};

use crate::command::{self, CommandChannel, EngineCommand};
use crate::lifecycle::{
    quick_press_tick_delay_hint, tap_state_after_create_attempt, tap_state_after_reenable,
    AtomicTapState, CreateAttemptResult, RecoveryCounters, RecreateOutcome, ReenableOutcome,
    TapState,
};
use crate::path_b::PathBManager;
use crate::state::SharedState;
use crate::system_hooks::SystemHooks;
use crate::trace::{self, TapTrace, TraceDrainHandle, TraceEmit, TraceRing};
use crate::watchdog::Watchdog;

/// 앱(호출자)에게 알리는 엔진 사건.
///
/// ⛔ **이 사건들을 받는 `on_event` 콜백은 대부분 `ultrakey-tap` 전용 스레드에서
/// 호출된다 — 그 안에서 절대 블록하지 마라.**
///
/// 탭 스레드는 `CGEventTap` 의 mach port 를 서비스하는 **유일한** 스레드다. 활성
/// 탭은 시스템의 모든 키·마우스 이벤트가 동기적으로 통과하는 지점이므로, 이
/// 콜백이 늦어지면 **그 지연이 곧 시스템 전체의 입력 지연**이 된다. 특히 금지:
///
/// - 메인 스레드로의 **동기** 디스패치(Tauri `WebviewWindow::show`/`set_focus`/
///   `is_visible` 등은 메인 스레드 밖에서 부르면 전부 여기 해당한다). 대신
///   `AppHandle::run_on_main_thread` 로 큐잉만 하고 즉시 반환하라
/// - 다른 스레드가 오래 쥘 수 있는 `Mutex` 획득
/// - 네트워크·프로세스 실행 같은 무제한 대기
///
/// ⚠️ 실측 회귀: 앱이 `NotTrusted` 를 받아 탭 스레드에서 Tauri 창을 동기 조작하자
/// **머신 전체 입력이 멈췄다**(이슈 #10 후속). `apps/ultrakey-app` 의
/// `on_main_thread` 헬퍼가 이 계약을 지키는 방식이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineEvent {
    TapStateChanged(TapState),
    /// ⭐ 권한이 확인된 상태에서 탭 생성 실패 — 앱은 이것을 받으면 종료해야 한다
    /// (§3-a, §5#16). 이 엔진은 재시도하지 않는다.
    FatalTapCreateFailed,
    /// 권한이 없어 탭을 설치하지 못했다 — F-11 온보딩이 처리한다. 이 엔진은 재시도
    /// 루프를 돌지 않고 `NotInstalled` 로 남는다.
    NotTrusted,
    /// 복구(재활성화·재생성)가 반복 실패했다 — 앱이 프로세스 재실행을 고려해야 한다
    /// (§5#17). 트리거 조건은 `Timings::tap_recreate_max_attempts`.
    NeedsRelaunch,
}

#[derive(Debug, thiserror::Error)]
pub enum EngineError {
    #[error("탭 스레드를 시작할 수 없다: {0}")]
    ThreadSpawn(std::io::Error),
    #[error("탭 스레드 초기화 핸드셰이크에 실패했다")]
    HandshakeFailed,
    /// macOS 가 아닌 타깃에서는 이 엔진을 시작할 수 없다 — 경로 A(`CGEventTap`)가
    /// macOS 전용이기 때문이다. 이 크레이트 자체는 non-macOS 에서도 컴파일된다.
    #[cfg(not(target_os = "macos"))]
    #[error("이 플랫폼(macOS 아님)에서는 이벤트 탭 엔진을 시작할 수 없다")]
    UnsupportedPlatform,
}

/// 탭 스레드가 핸드셰이크로 돌려주는, 메인(호출) 스레드가 이후 계속 쥐고 있을 손잡이.
struct TapThreadHandshake {
    run_loop: RunLoopHandle,
    commands: CommandChannel,
}

/// M1 엔진 인스턴스. `Engine::start` 가 성공하면 탭 전용 스레드·워치독 스레드·지연
/// 스케줄러 스레드가 모두 살아 있다. `shutdown()` 이 셋을 전부 정리한다.
pub struct Engine {
    shared: Arc<SharedState>,
    commands: CommandChannel,
    run_loop: RunLoopHandle,
    tap_state: Arc<AtomicTapState>,
    thread: Option<JoinHandle<()>>,
    watchdog: Option<Watchdog>,
    system_hooks: Option<SystemHooks>,
    path_b: Arc<PathBManager>,
    /// ⭐ 이슈 #19 진단 계측(`trace.rs`) — `ULTRAKEY_TRACE_TAP=1` 일 때만 `Some`.
    trace_drain: Option<TraceDrainHandle>,
}

impl Engine {
    /// ⚠️ **호출한 스레드에서 `SystemHooks::start` 를 동기적으로 실행한다** — 그 스레드가
    /// 곧 메인 스레드(Tauri/`NSApplication` 런루프 소유자)여야 한다(`system_hooks.rs`
    /// 모듈 문서 참고). 어기면 경고 로그가 남는다(`warn_if_not_main_thread`).
    ///
    /// ⛔ `on_event` 는 **탭 전용 스레드에서** 불린다 — [`EngineEvent`] 문서의 "블록하지
    /// 마라" 계약을 반드시 읽어라. 이 계약을 어기면 시스템 전체 입력이 멈춘다.
    pub fn start(
        config: EngineConfig,
        gate: Arc<AtomicAppGate>,
        on_event: Box<dyn Fn(EngineEvent) + Send + Sync>,
    ) -> Result<Self, EngineError> {
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (config, gate, on_event);
            return Err(EngineError::UnsupportedPlatform);
        }

        #[cfg(target_os = "macos")]
        {
            // 경로 B — ⭐ **정리의 정본은 종료가 아니라 기동이다**(F-15
            // `settings-store-and-integrity.md` §3.1.1 결정 2). M1 실기기 검증에서
            // **로그아웃 시 graceful shutdown 경로가 아예 실행되지 않는 것**이 관찰됐고
            // (`docs/dev/manual-verification.md` "실측 결과"), 경로 B 매핑은 프로세스가
            // 죽어도 커널 HID 층에 남는다(`docs/dev/architecture.md` §2.4). 즉 "정상
            // 종료 시 정리한다"는 설계는 가장 흔한 종료 경로에서 그냥 동작하지 않는다 —
            // 그래서 종료 정리는 부가적 최적화로 격하하고, **매 기동마다 무조건 재조정**한다.
            //
            // ⭐ M2 — `config` 가 이미 D-1 caps lock alias 를 요구할 수 있다(예: 지난
            // 실행에서 caps lock 프리셋을 켜 둔 채 재시작). `desired` 를 `config` 에서
            // 계산해 넘긴다 — M1 시절 하드코딩됐던 빈 목록을 걷어낸다. `config` 를
            // `SharedState::new` 로 옮기기 *전에* 계산해야 한다(그 호출이 값을 소비한다).
            let desired = crate::path_b::desired_mappings_for(&config);
            let path_b = Arc::new(PathBManager::new(Box::new(HidutilBackend)));
            if let Err(e) = path_b.reconcile_on_start(&desired) {
                tracing::warn!(error = %e, "경로 B 시작 시 재조정 확인에 실패했다");
            }

            let shared = SharedState::new(config, gate);
            let on_event: Arc<dyn Fn(EngineEvent) + Send + Sync> = Arc::from(on_event);
            let tap_state = Arc::new(AtomicTapState::new(TapState::NotInstalled));
            let health_probe_slot: Arc<ArcSwapOption<TapHealthProbe>> =
                Arc::new(ArcSwapOption::empty());
            // ⭐ 이슈 #19 진단 계측 — 링 자체는 `trace_enabled()` 와 무관하게 항상
            // 만든다(할당 한 번뿐이고, 비활성 상태에서는 아무도 push 하지 않아 계속
            // 비어 있다). 드레인 스레드만 게이트로 막는다(아래).
            let trace_ring = Arc::new(TraceRing::new());

            let (ready_tx, ready_rx) = bounded::<TapThreadHandshake>(1);

            let thread_shared = Arc::clone(&shared);
            let thread_on_event = Arc::clone(&on_event);
            let thread_tap_state = Arc::clone(&tap_state);
            let thread_probe_slot = Arc::clone(&health_probe_slot);
            let thread_path_b = Arc::clone(&path_b);
            let thread_trace_ring = Arc::clone(&trace_ring);

            let thread = thread::Builder::new()
                .name("ultrakey-tap".to_string())
                .spawn(move || {
                    tap_thread_main(
                        thread_shared,
                        thread_on_event,
                        thread_tap_state,
                        thread_probe_slot,
                        thread_path_b,
                        thread_trace_ring,
                        ready_tx,
                    );
                })
                .map_err(EngineError::ThreadSpawn)?;

            let handshake = ready_rx.recv().map_err(|_| EngineError::HandshakeFailed)?;

            let watchdog = Watchdog::spawn(
                Arc::clone(&shared),
                Arc::clone(&health_probe_slot),
                handshake.commands.clone(),
            );
            let system_hooks = SystemHooks::start(Arc::clone(&shared), handshake.commands.clone());
            // `trace::spawn_drain_thread` 는 `trace_enabled()` 가 false 면 스레드를
            // 만들지 않고 `None` 을 돌려준다.
            let trace_drain = trace::spawn_drain_thread(trace_ring);

            Ok(Engine {
                shared,
                commands: handshake.commands,
                run_loop: handshake.run_loop,
                tap_state,
                thread: Some(thread),
                watchdog: Some(watchdog),
                system_hooks: Some(system_hooks),
                path_b,
                trace_drain,
            })
        }
    }

    pub fn shared(&self) -> Arc<SharedState> {
        Arc::clone(&self.shared)
    }

    /// 설정을 교체한다 — `ArcSwap` 원자적 교체 후 탭 스레드에 `Reconfigure` 명령을 보내
    /// `Arbiter` 내부 quick press 슬롯 캐시를 다시 구성하게 한다.
    ///
    /// ⭐ M2/D-1 — 경로 B(`hidutil`) 설치·정리도 여기서 동기적으로 수행한다. 호출자는
    /// 항상 메인(Tauri 커맨드) 스레드다 — **탭 스레드가 아니다.** `hidutil` 서브프로세스
    /// 호출은 §2.2 가 콜백 임계 경로에서 금지하는 블로킹 I/O 그 자체이지만, 이 메서드는
    /// 그 경로 밖에서만 불린다(architecture.md §6.6 이 확정한 "설정이 바뀔 때마다
    /// 재계산해서 Engine::reconfigure 경로로 반영"의 구현).
    pub fn reconfigure(&self, config: EngineConfig) {
        let desired = crate::path_b::desired_mappings_for(&config);
        if let Err(e) = self.path_b.apply(&desired) {
            tracing::warn!(error = %e, "경로 B(D-1 caps lock alias) 재적용 실패");
        }
        self.shared.config.store(Arc::new(config));
        self.commands.send(EngineCommand::Reconfigure);
    }

    pub fn tap_state(&self) -> TapState {
        self.tap_state.load()
    }

    /// 규칙이 바뀌는 순간 소스 키가 물리적으로 눌린 채였다면 stuck modifier 가 남는다.
    /// 설정 변경 직후 이 명령을 함께 보내 상태 기계를 Idle 로 되돌린다 — 절전/잠금
    /// 진입 시 이미 같은 명령으로 방지하는 것과 같은 장치다(`key-remapping-engine.md`
    /// §5 항목 9, `EngineCommand::ForceResetState` 문서 주석).
    pub fn force_reset_state(&self) {
        self.commands.send(EngineCommand::ForceResetState);
    }

    /// ⚠️ 스레드를 누수시키지 않는다 — 탭 스레드·워치독·지연 스케줄러 순서로 전부
    /// 정리한 뒤에 리턴한다.
    pub fn shutdown(mut self) {
        self.commands.send(EngineCommand::Shutdown);
        // 이중 안전장치 — Shutdown 명령이 어떤 이유로든 처리되지 않아도 런루프를
        // 멈춘다(`CFRunLoopStop` 은 이미 멈춘 런루프에 불러도 안전하다).
        self.run_loop.stop();

        if let Some(w) = self.watchdog.take() {
            w.shutdown();
        }
        if let Some(h) = self.system_hooks.take() {
            h.shutdown();
        }
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        if let Err(e) = self.path_b.cleanup() {
            tracing::warn!(error = %e, "경로 B 정리 실패");
        }
        // 탭 스레드가 끝난 뒤에 정리한다 — 탭 스레드가 마지막으로 push 한 레코드까지
        // 드레인 스레드가 종료 전 마지막 한 바퀴에서 회수하게 하기 위함이다.
        if let Some(d) = self.trace_drain.take() {
            d.shutdown();
        }
    }
}

// ============================================================================
// 탭 전용 스레드 로컬 상태
// ============================================================================

/// 탭 전용 스레드가 등록한 세 콜백(탭 이벤트 트램폴린 · quick press 타이머 · 커맨드
/// perform)이 공유하는 로컬 상태. `RunLoopConfined` 로 감싸 락 없이 공유한다
/// (`ultrakey_platform::runloop::RunLoopConfined` 문서 참고).
struct TapThreadState {
    shared: Arc<SharedState>,
    on_event: Arc<dyn Fn(EngineEvent) + Send + Sync>,
    /// 정본 키 상태 테이블 + 중재 로직. 이 스레드가 배타 소유한다(`docs/dev/
    /// architecture.md` §2.2).
    arbiter: Arbiter,
    tap: Option<EventTap>,
    secure_input: DirectSecureInputProbe,
    counters: RecoveryCounters,
    tap_state_snapshot: TapState,
    tap_state_atomic: Arc<AtomicTapState>,
    health_probe_slot: Arc<ArcSwapOption<TapHealthProbe>>,
    path_b: Arc<PathBManager>,
    /// 치명적 실패(§3-a "탭 생성 실패는 치명적이다") 이후에는 아무 것도 하지 않는다.
    fatal: bool,
    timer: Option<RepeatingTimer>,
    /// 프로세스 시작이 아니라 **이 탭 스레드가 시작한 시각** 기준 단조 시계(`Millis` 는
    /// 콜백이 매번 이 시각으로부터 계산해 넘긴다 — `Instant::elapsed()` 는 힙 할당이
    /// 없어 콜백에서 써도 된다).
    start: Instant,
    /// ⭐ 이슈 #19 진단 계측(`trace.rs`) — 무잠금 SPSC 링. 생산자는 이 스레드뿐이다.
    trace: Arc<TraceRing>,
    /// 계측 레코드의 단조 증가 시퀀스 번호. 이 스레드 배타 소유라 락 없이 증가시킨다.
    trace_seq: u64,
}

fn set_tap_state(st: &mut TapThreadState, s: TapState) {
    if st.tap_state_snapshot != s {
        st.tap_state_snapshot = s;
        st.tap_state_atomic.store(s);
        tracing::info!(?s, "탭 상태 전이");
        (st.on_event)(EngineEvent::TapStateChanged(s));
    }
}

fn make_synth_event(ev: &SynthEvent) -> Option<SyntheticEvent> {
    match ev.kind {
        EventKind::FlagsChanged => SyntheticEvent::flags_changed(ev.keycode, ev.flags),
        EventKind::KeyDown => SyntheticEvent::keyboard(ev.keycode, true, ev.flags),
        EventKind::KeyUp => SyntheticEvent::keyboard(ev.keycode, false, ev.flags),
        // M1: 마우스·기타 합성 이벤트는 아직 방출하지 않는다(M1 규칙 테이블이 비어
        // 있어 실제로 등장하지 않는다).
        _ => None,
    }
}

/// 콜백 **안**에서 방출 — `post_to_tap` 은 재진입이 없다(`ultrakey_platform::event`
/// 문서 참고). `post()` 를 쓰지 않는다.
fn apply_outcome_in_tap(outcome: &Outcome, proxy: TapProxy) {
    for ev in outcome.emitted() {
        if let Some(synth) = make_synth_event(ev) {
            synth.post_to_tap(proxy);
        }
    }
}

/// 콜백 **밖**(커맨드 perform·타이머)에서 방출 — `post()` 로 세션 이벤트 스트림에
/// 직접 주입한다. 자기 합성 마커가 무한 루프를 막아 준다(`event.rs` 0-a 단계).
fn apply_outcome_outside_tap(outcome: &Outcome) {
    for ev in outcome.emitted() {
        if let Some(synth) = make_synth_event(ev) {
            synth.post();
        }
    }
}

/// P11 — 경로 C(`IOHIDSetModifierLockState`) caps lock 토글. 현재 상태를 읽어
/// 반전해 쓴다. `docs/dev/architecture.md` §6.6 결정: **콜백 안에서 직접 실행한다**
/// — mach 메시지 한 번이라 마이크로초 단위이고, 사용자가 실제로 제스처를 완료했을
/// 때만(매 이벤트가 아니라) 실행되므로 탭 타임아웃 예산 대비 무시할 만하다.
/// 기각한 대안(워커 스레드로 큐잉)은 무한 큐 `send` 가 오히려 콜백 안에서 할당을
/// 유발할 수 있어 얻는 것보다 잃는 것이 크다는 것이 §6.6 의 결론이다.
///
/// ⛔ 실패해도 여기서는 로깅하지 않는다(§2.2 — 콜백 임계 경로에서 동기 로깅 금지).
/// caps lock 상태를 읽거나 쓰지 못하는 것은 하드웨어/커널 이상 같은 드문 상황에서도
/// 반복될 수 있어, 이 자리에서 로그를 남기면 로그 폭주로 이어질 수 있다.
///
/// ⭐ 이슈 #19 진단 계측 — 반환값은 실측 결과를 **값으로만** 넘긴다(로깅은 하지
/// 않는다). `(result, before, after)`: `result` 는 0=시도안함(이 함수는 항상
/// 시도하므로 실제로는 나오지 않는다) 1=성공 2=실패, `before`/`after` 는
/// 0=off 1=on 2=읽기실패. `after` 는 쓰기 직후 상태를 한 번 더 읽어 담는다 —
/// "쓰기가 실제로 반영됐는가"까지 계측이 확인할 수 있어야 원인을 좁힐 수 있다.
fn toggle_caps_lock_via_path_c() -> (u8, u8, u8) {
    fn state_code(s: Option<bool>) -> u8 {
        match s {
            Some(false) => 0,
            Some(true) => 1,
            None => 2,
        }
    }

    let current = ultrakey_platform::hid_lock::caps_lock_state();
    let before = state_code(current);
    let result = match current {
        Some(c) if ultrakey_platform::hid_lock::set_caps_lock_state(!c) => 1,
        Some(_) => 2,
        // 읽기부터 실패하면 반전할 기준값이 없어 쓰기를 시도하지 않는다 — 실패로 기록.
        None => 2,
    };
    let after = state_code(ultrakey_platform::hid_lock::caps_lock_state());
    (result, before, after)
}

/// `Outcome::effects()` 실행 — 콜백 **안**에서 부른다(`proxy` 가 있다).
///
/// `Effect::TypeChar` 는 `SyntheticEvent::unicode` 로 down+up 한 쌍을 합성해
/// `post_to_tap` 으로 낸다(architecture.md §6.4 P9). `Effect::OpenSeek` 은 M3(F-01)
/// 범위 — 위임 지시서가 명시적으로 요구한 대로 `tracing::info!` 로만 남기고
/// 아무것도 하지 않는다.
///
/// ⚠️ 이 `tracing::info!` 자체는 §2.2 의 "콜백 안 동기 로깅 금지"의 글자 그대로는
/// 어긋난다 — 하지만 `toggle_caps_lock_via_path_c` 에 이미 적용한 것과 같은 근거
/// (사용자가 실제로 quick press 를 완료했을 때만 드물게 발생, 매 이벤트 비용이
/// 아니다)로 둔 **의도적 예외**다. M3 가 이 자리를 실제 기능으로 채우면 그때
/// 다시 검토한다.
///
/// ⭐ 이슈 #19 계측 배선 — `Effect::ToggleCapsLock` 을 실행했다면 그 실측 결과
/// `(result, before, after)` 를 반환한다. 트레이스가 꺼져 있어도 이 튜플은 계산된다
/// (경로 C 실행 자체의 일부이지 계측 전용 비용이 아니다) — 호출자가 `trace_enabled()`
/// 일 때만 계측 레코드에 채워 넣는다.
fn apply_effects_in_tap(outcome: &Outcome, proxy: TapProxy) -> Option<(u8, u8, u8)> {
    let mut path_c = None;
    for effect in outcome.effects() {
        match effect {
            Effect::ToggleCapsLock => path_c = Some(toggle_caps_lock_via_path_c()),
            Effect::TypeChar(c) => {
                if let Some(down) = SyntheticEvent::unicode(*c, true) {
                    down.post_to_tap(proxy);
                }
                if let Some(up) = SyntheticEvent::unicode(*c, false) {
                    up.post_to_tap(proxy);
                }
            }
            Effect::OpenSeek => {
                tracing::info!("Seek 열기 요청 수신 — M3(F-01) 범위, 아직 구현되지 않음");
            }
        }
    }
    path_c
}

/// `Outcome::effects()` 실행 — 콜백 **밖**(커맨드 perform·타이머)에서 부른다.
/// 탭 콜백이 아니므로 로깅 제약이 없다.
/// 콜백 밖 경로(타이머 만료, `ForceResetState` 등)는 계측 레코드를 만들지 않는다
/// (이슈 #19 계측은 탭 콜백 경로의 `arbitrate` 직후만 다룬다) — 그래도
/// `apply_effects_in_tap` 과 시그니처를 맞춰 둔다. 호출자는 대부분 결과를 버린다.
fn apply_effects_outside_tap(outcome: &Outcome) -> Option<(u8, u8, u8)> {
    let mut path_c = None;
    for effect in outcome.effects() {
        match effect {
            Effect::ToggleCapsLock => path_c = Some(toggle_caps_lock_via_path_c()),
            Effect::TypeChar(c) => {
                if let Some(down) = SyntheticEvent::unicode(*c, true) {
                    down.post();
                }
                if let Some(up) = SyntheticEvent::unicode(*c, false) {
                    up.post();
                }
            }
            Effect::OpenSeek => {
                tracing::info!("Seek 열기 요청 수신 — M3(F-01) 범위, 아직 구현되지 않음");
            }
        }
    }
    path_c
}

fn build_callback(
    cell: RunLoopConfined<TapThreadState>,
    commands: CommandChannel,
    start: Instant,
) -> TapCallback {
    Box::new(
        move |proxy: TapProxy, kind: EventKind, event: &mut CgEventRef| {
            on_tap_event(&cell, &commands, start, proxy, kind, event)
        },
    )
}

/// ⭐ `CGEventTap` 콜백 — `docs/dev/architecture.md` §2.3 이 정한 진입 순서를 그대로
/// 구현한다. 0-a(자기 합성 마커)와 탭 비활성화 자체 재활성화는 `ultrakey_platform::
/// event_tap` 트램폴린이 이미 처리했다 — 이 함수는 0-b(비활성화 통지의 우리 쪽 후속
/// 처리) 부터 시작한다.
fn on_tap_event(
    cell: &RunLoopConfined<TapThreadState>,
    commands: &CommandChannel,
    start: Instant,
    proxy: TapProxy,
    kind: EventKind,
    event: &mut CgEventRef,
) -> TapAction {
    // 0-b: 탭 비활성화 통지. 트램폴린이 이미 `CGEventTapEnable` 로 재활성화를
    // 시도했다 — 우리는 그 결과를 우리 FSM/카운터에 반영해야 하지만, 로깅과
    // `on_event` 호출은 힙 할당·동기 I/O 를 수반할 수 있어 이 콜백 안에서 직접 하지
    // 않는다. 대신 명령 큐에 넣고 즉시 리턴한다 — perform 콜백(같은 스레드, 콜백이
    // 아닌 자리)이 실제 처리를 한다.
    if matches!(
        kind,
        EventKind::TapDisabledByTimeout | EventKind::TapDisabledByUserInput
    ) {
        commands.send(EngineCommand::RecoverTap);
        return TapAction::Pass;
    }

    let mut st = cell.borrow_mut();

    // ⭐ M2 — `cfg` 를 여기서 한 번만 읽는다. `Arbiter::force_reset` 이 `&EngineConfig`
    // 를 받도록 바뀌었으므로(force_reset 이 프리셋 hold_remap 대상까지 알아야
    // stuck modifier 를 정확히 정리할 수 있다) 0-c/0-d 게이트도 이 값이 필요하다.
    // `ArcSwap::load_full` 은 원자적 포인터 로드 + `Arc` 참조 카운트 증가일 뿐이라
    // 힙 할당·락이 없다 — 콜백 임계 경로에서 불러도 안전하다(architecture.md §2.2).
    let cfg = st.shared.config.load_full();

    // 0-c: 앱별 비활성화 게이트(F-10, 계층 0, §3-f).
    if st.shared.gate.is_remapping_disabled() {
        let reset = st.arbiter.force_reset(&cfg);
        apply_outcome_in_tap(&reset, proxy);
        apply_effects_in_tap(&reset, proxy);
        return TapAction::Pass;
    }

    // 0-d: Secure Input(§5 엣지 6) — 경로 A 에만 적용된다.
    if st.secure_input.is_enabled() {
        let reset = st.arbiter.force_reset(&cfg);
        apply_outcome_in_tap(&reset, proxy);
        apply_effects_in_tap(&reset, proxy);
        return TapAction::Pass;
    }

    // ──── 여기부터 ultrakey-core 의 순수 함수 ────
    let now = Millis(start.elapsed().as_millis() as u64);
    let input = InputEvent {
        kind,
        keycode: event.keycode(),
        flags: event.flags(),
        autorepeat: event.is_autorepeat(),
    };
    let seek_active = st.shared.seek_session_active.load(Ordering::Acquire);
    let outcome = st.arbiter.arbitrate(&cfg, &input, seek_active, now);

    // quick press 타이머 재예약 힌트(architecture.md §2.3 — 25ms 폴링 금지).
    let outcome_is_pending =
        outcome.disposition() == Disposition::Consume && outcome.emitted().is_empty();
    if let Some(delay_ms) = quick_press_tick_delay_hint(
        kind,
        outcome_is_pending,
        cfg.timings.quick_press_duration_ms,
        cfg.timings.double_tap_interval_ms,
    ) {
        if let Some(timer) = st.timer.as_ref() {
            timer.set_next_fire_after_ms(delay_ms);
        }
    }

    apply_outcome_in_tap(&outcome, proxy);
    let path_c = apply_effects_in_tap(&outcome, proxy);

    // ⭐ 이슈 #19 진단 계측 — `arbitrate` 호출 직후(위)가 아니라 여기, effects 적용
    // 결과까지 알고 난 뒤에 레코드를 만든다(경로 C 실측을 한 레코드에 함께 담기
    // 위해서다). `trace_enabled()` 가 false 면 레코드를 만들지도 push 하지도
    // 않는다 — 비용 0. `tracing::*` 매크로는 여기서 절대 부르지 않는다(§2.2).
    if trace::should_trace(input.kind) {
        let resolved = resolve_caps_lock_alias_for_trace(&cfg, input.keycode);
        let mut rec = TapTrace {
            seq: st.trace_seq,
            raw_kind: trace::event_kind_to_code(input.kind),
            raw_keycode: input.keycode.0,
            raw_flags: input.flags.0,
            autorepeat: input.autorepeat,
            resolved_keycode: resolved.0,
            alias_active: cfg.caps_lock_alias.is_some(),
            layer: trace::layer_to_code(outcome.layer()),
            disposition: trace::disposition_to_code(outcome.disposition()),
            disposition_flags: trace::disposition_flags_of(outcome.disposition()),
            ..Default::default()
        };

        // `TapTrace::emitted`/`effects` 는 진단 요약이라 4개까지만 담는다(`trace.rs`
        // 문서 참고) — `Outcome` 자체는 최대 8개까지 낼 수 있다.
        for (i, ev) in outcome.emitted().iter().take(4).enumerate() {
            rec.emitted[i] = TraceEmit {
                kind: trace::event_kind_to_code(ev.kind),
                keycode: ev.keycode.0,
                flags: ev.flags.0,
            };
        }
        rec.emitted_len = outcome.emitted().len().min(4) as u8;

        for (i, eff) in outcome.effects().iter().take(4).enumerate() {
            rec.effects[i] = trace::effect_to_code(*eff);
        }
        rec.effects_len = outcome.effects().len().min(4) as u8;

        if let Some((result, before, after)) = path_c {
            rec.path_c_result = result;
            rec.path_c_before = before;
            rec.path_c_after = after;
        }

        st.trace_seq = st.trace_seq.wrapping_add(1);
        st.trace.push(rec);
    }

    match outcome.disposition() {
        Disposition::Pass => TapAction::Pass,
        Disposition::PassWithFlags(f) => {
            event.set_flags(f);
            TapAction::Replace
        }
        Disposition::Consume => TapAction::Consume,
    }
}

/// quick press 타이머 만료 처리 — 탭 콜백이 아니므로 로깅 제약이 없다.
fn on_timer_tick(cell: &RunLoopConfined<TapThreadState>) {
    let outcome = {
        let mut st = cell.borrow_mut();
        let cfg = st.shared.config.load_full();
        let now = Millis(st.start.elapsed().as_millis() as u64);
        st.arbiter.on_tick(&cfg, now)
    };
    apply_outcome_outside_tap(&outcome);
    apply_effects_outside_tap(&outcome);
}

/// `EngineCommand::RecoverTap` — §3-a `Disabled` 전이의 재활성화 시도.
fn handle_recover_tap(cell: &RunLoopConfined<TapThreadState>, commands: &CommandChannel) {
    // 시도 횟수를 로깅하려면 `record_reenable_result` 가 카운터를 리셋하기 *전* 값을
    // 잡아 둬야 한다(에스컬레이션 시 0으로 리셋된다) — `docs/dev/manual-verification.md`
    // 가 "재활성화 성공/실패, 시도 횟수" 로그를 근거로 판정한다.
    // ⛔ **권한이 이미 사라졌으면 재활성화하지 않는다.**
    //
    // 권한을 잃은 탭은 `CGEventTapEnable(true)` 를 불러도 macOS 가 곧바로 다시
    // 끄고 비활성화 통지를 또 보낸다. 그 통지가 다시 `RecoverTap` 을 만들면
    // "재활성화 → 즉시 비활성화 → 통지 → 재활성화" 가 끝없이 돈다. 게다가
    // `tap.is_enabled()` 는 방금 `enable(true)` 한 직후라 **`true` 를 돌려주므로**
    // 실패 카운터가 매번 리셋되어 재생성 에스컬레이션에도 영원히 도달하지 못한다.
    //
    // ⚠️ 이 루프는 탭 스레드의 런루프를 포화시켜 **시스템 전체 입력을 멈춘다**
    // (실측 회귀 — `event_tap.rs` 트램폴린의 차단기 주석 참고). 여기서 권한을
    // 먼저 확인하고, 없으면 재활성화 대신 재생성 경로로 보낸다 —
    // `handle_recreate_tap` 은 탭을 놓은 뒤 `NotTrusted` 를 올려 F-11 온보딩이
    // 이어받게 한다(§3-a: 권한이 없어 실패하는 것은 정상 경로다).
    if !ultrakey_platform::accessibility::is_process_trusted() {
        tracing::warn!(
            "Accessibility 권한이 없는 상태에서 탭 재활성화 요청이 왔다 — 재활성화하지 않고 \
             탭을 놓는다(재활성화 폭주 방지)"
        );
        handle_recreate_tap(cell, commands);
        return;
    }

    let outcome = {
        let mut st = cell.borrow_mut();
        if st.fatal {
            return;
        }
        match st.tap.as_ref() {
            Some(tap) => {
                tap.enable(true);
                let enabled = tap.is_enabled();
                let max_attempts = st.shared.config.load().timings.tap_reenable_max_attempts;
                let attempt_number = st.counters.reenable_failures() + 1;
                let result = st.counters.record_reenable_result(enabled, max_attempts);
                Some((result, attempt_number, max_attempts))
            }
            None => None,
        }
    };

    match outcome {
        Some((ReenableOutcome::Recovered, attempt, _max)) => {
            tracing::info!(attempt, "탭 재활성화 성공");
            let mut st = cell.borrow_mut();
            set_tap_state(&mut st, tap_state_after_reenable(true));
        }
        Some((ReenableOutcome::StillFailing, attempt, max)) => {
            tracing::warn!(attempt, max, "탭 재활성화 실패 — 다음 시도를 기다린다");
            let mut st = cell.borrow_mut();
            set_tap_state(&mut st, tap_state_after_reenable(false));
        }
        Some((ReenableOutcome::EscalateToRecreate, attempt, max)) => {
            tracing::warn!(
                attempt,
                max,
                "탭 재활성화가 상한만큼 반복 실패해 재생성으로 에스컬레이션한다"
            );
            handle_recreate_tap(cell, commands);
        }
        None => {
            tracing::debug!("탭이 아직 만들어지지 않았다 — 재생성을 직접 시도한다");
            handle_recreate_tap(cell, commands);
        }
    }
}

/// `EngineCommand::RecreateTap` — 탭을 처음부터 다시 만든다. 최초 설치 시도도 이
/// 함수를 그대로 쓴다(§3-a `NotInstalled`→`Installing` 전이와 동일한 절차이기 때문).
fn handle_recreate_tap(cell: &RunLoopConfined<TapThreadState>, commands: &CommandChannel) {
    let start = {
        let mut st = cell.borrow_mut();
        if st.fatal {
            return;
        }
        st.tap = None;
        st.health_probe_slot.store(None);
        set_tap_state(&mut st, TapState::Installing);
        st.start
    };

    let callback = build_callback(cell.clone(), commands.clone(), start);

    match EventTap::create(callback) {
        Ok(mut tap) => {
            tap.add_to_current_runloop();
            let probe = tap.health_probe();
            let alive = tap.is_enabled();

            let mut st = cell.borrow_mut();
            st.health_probe_slot.store(probe.map(Arc::new));
            st.tap = Some(tap);
            let max_attempts = st.shared.config.load().timings.tap_recreate_max_attempts;
            // 리셋 전 값을 먼저 잡아 둔다(에스컬레이션 시 카운터가 0으로 리셋된다) —
            // 위 `handle_recover_tap` 과 같은 이유.
            let attempt_number = st.counters.recreate_failures() + 1;
            let attempt_result = if alive {
                CreateAttemptResult::Success
            } else {
                CreateAttemptResult::CreatedButDisabled
            };
            match st.counters.record_recreate_result(alive, max_attempts) {
                RecreateOutcome::Recovered => {
                    tracing::info!(attempt = attempt_number, "탭 재생성 성공");
                    set_tap_state(&mut st, tap_state_after_create_attempt(attempt_result));
                }
                RecreateOutcome::StillFailing => {
                    tracing::warn!(
                        attempt = attempt_number,
                        max = max_attempts,
                        "탭을 재생성했지만 여전히 비활성 상태다 — 다음 시도를 기다린다"
                    );
                    set_tap_state(&mut st, tap_state_after_create_attempt(attempt_result));
                }
                RecreateOutcome::NeedsRelaunch => {
                    tracing::error!(
                        attempt = attempt_number,
                        max = max_attempts,
                        "탭 재생성이 상한만큼 반복 실패했다 — 프로세스 재실행이 필요하다(§5#17)"
                    );
                    // ⭐ 프로세스 재실행이 필요하다는 신호일 뿐, 이 탭 자체는 살아있는
                    // 그대로(`attempt_result` 기준) 상태를 게시한다 — §3-a 표는 이
                    // 경우를 별도 상태로 정의하지 않는다.
                    set_tap_state(&mut st, tap_state_after_create_attempt(attempt_result));
                    (st.on_event)(EngineEvent::NeedsRelaunch);
                }
            }
        }
        Err(TapCreateError::NotTrusted) => {
            let mut st = cell.borrow_mut();
            tracing::warn!("Accessibility 권한이 확인되지 않아 탭을 만들 수 없다 — F-11 온보딩을 기다린다(재시도 루프를 돌지 않는다)");
            set_tap_state(
                &mut st,
                tap_state_after_create_attempt(CreateAttemptResult::NotTrusted),
            );
            (st.on_event)(EngineEvent::NotTrusted);
        }
        Err(TapCreateError::CreateFailed) => {
            let mut st = cell.borrow_mut();
            tracing::error!("권한이 확인된 상태에서도 CGEventTapCreate 가 실패했다 — 치명적, 재시도하지 않는다(§3-a, §5#16)");
            st.fatal = true;
            set_tap_state(
                &mut st,
                tap_state_after_create_attempt(CreateAttemptResult::Fatal),
            );
            (st.on_event)(EngineEvent::FatalTapCreateFailed);
        }
    }
}

/// `CommandSource` perform 콜백이 부르는 드레인 루프. 명령은 항상 이 함수 안에서,
/// 큐에 들어간 순서 그대로 처리된다(`command.rs` 문서 참고).
fn drain_commands(
    cell: &RunLoopConfined<TapThreadState>,
    rx: &Receiver<EngineCommand>,
    commands: &CommandChannel,
    run_loop: &RunLoopHandle,
) {
    let mut should_stop = false;

    while let Ok(cmd) = rx.try_recv() {
        match cmd {
            EngineCommand::ForceResetState => {
                let outcome = {
                    let mut st = cell.borrow_mut();
                    let cfg = st.shared.config.load_full();
                    st.arbiter.force_reset(&cfg)
                };
                apply_outcome_outside_tap(&outcome);
                apply_effects_outside_tap(&outcome);
                tracing::info!(
                    "절전/잠금/Secure Input 대응 — 상태를 강제로 리셋했다(stuck modifier 방지)"
                );
            }
            EngineCommand::RecoverTap => handle_recover_tap(cell, commands),
            EngineCommand::RecreateTap => handle_recreate_tap(cell, commands),
            EngineCommand::Reconfigure => {
                let mut st = cell.borrow_mut();
                let cfg = st.shared.config.load_full();
                st.arbiter.reconfigure(&cfg);
                tracing::info!("설정 변경을 반영해 Arbiter 를 재구성했다");
            }
            EngineCommand::ReapplyHidMapping => {
                // ⭐ M2/D-1 — 더 이상 하드코딩된 빈 매핑이 아니다. 현재 설정에서
                // 요구하는 매핑(caps lock alias 가 켜져 있으면 F18)을 다시 계산해
                // 재적용한다. 핫플러그(외장 키보드 연결)가 드물게만 이 경로를 타므로,
                // 이 커맨드 perform 콜백(탭 이벤트 콜백 자체는 아니다) 안에서
                // hidutil 서브프로세스를 동기 호출해도 §2.2 가 금지하는 "매 이벤트
                // 임계 경로"에는 해당하지 않는다 — M1 부터 이어진 판단이다.
                let (path_b, cfg) = {
                    let st = cell.borrow();
                    (st.path_b.clone(), st.shared.config.load_full())
                };
                let desired = crate::path_b::desired_mappings_for(&cfg);
                match path_b.apply(&desired) {
                    Ok(()) => tracing::info!(count = desired.len(), "경로 B 재적용을 완료했다"),
                    Err(e) => tracing::warn!(error = %e, "경로 B 재적용에 실패했다"),
                }
            }
            EngineCommand::Shutdown => {
                should_stop = true;
            }
        }
        if should_stop {
            break;
        }
    }

    if should_stop {
        tracing::info!("종료 명령을 받아 탭 스레드 런루프를 정지한다");
        run_loop.stop();
    }
}

/// 탭 전용 스레드 본체. `docs/dev/architecture.md` §2.1 "이벤트 탭 전용 스레드" 가
/// 배선하는 전부가 이 함수 안에 있다: 독립 런루프, `CGEventTap`, quick press 타이머,
/// 커맨드 소스.
fn tap_thread_main(
    shared: Arc<SharedState>,
    on_event: Arc<dyn Fn(EngineEvent) + Send + Sync>,
    tap_state_atomic: Arc<AtomicTapState>,
    health_probe_slot: Arc<ArcSwapOption<TapHealthProbe>>,
    path_b: Arc<PathBManager>,
    trace_ring: Arc<TraceRing>,
    ready_tx: Sender<TapThreadHandshake>,
) {
    let start = Instant::now();
    let initial_cfg = shared.config.load_full();
    let arbiter = Arbiter::new(&initial_cfg);

    let cell = RunLoopConfined::new(TapThreadState {
        shared,
        on_event,
        arbiter,
        tap: None,
        secure_input: DirectSecureInputProbe,
        counters: RecoveryCounters::new(),
        tap_state_snapshot: TapState::NotInstalled,
        tap_state_atomic,
        health_probe_slot,
        path_b,
        fatal: false,
        timer: None,
        start,
        trace: trace_ring,
        trace_seq: 0,
    });

    let (cmd_tx, cmd_rx) = command::channel();

    let run_loop_for_stop = RunLoopHandle::current()
        .expect("탭 스레드에 CFRunLoop 를 가져올 수 없다 — 있을 수 없는 상황");

    // `CommandChannel` 은 `CommandSource::signaller()` 가 있어야 완성되는데, perform
    // 콜백 자신도 그 `CommandChannel` 을 알아야(재생성 시 콜백을 다시 만드는 데 필요)
    // 하는 닭과 달걀 문제가 있다 — 아래 `RunLoopConfined<Option<CommandChannel>>` 로
    // 늦은 채움(late-fill)을 해결한다. perform 콜백이 실제로 처음 호출되는 시점은
    // 이 함수가 아래에서 값을 채운 **이후**이므로 항상 `Some` 을 본다.
    let commands_holder: RunLoopConfined<Option<CommandChannel>> = RunLoopConfined::new(None);

    let perform_cell = cell.clone();
    let perform_commands_holder = commands_holder.clone();
    let cmd_source = CommandSource::new(Box::new(move || {
        let maybe_commands = perform_commands_holder.borrow().clone();
        if let Some(commands) = maybe_commands {
            drain_commands(&perform_cell, &cmd_rx, &commands, &run_loop_for_stop);
        }
    }));
    cmd_source.add_to_current_runloop();

    let commands = CommandChannel::new(cmd_tx, cmd_source.signaller());
    *commands_holder.borrow_mut() = Some(commands.clone());

    // 최초 설치 시도 — §3-a `NotInstalled` → `Installing` → `Active`/`NotInstalled`/`Terminated`.
    handle_recreate_tap(&cell, &commands);

    // quick press 타이머 — architecture.md §2.3: "매 25ms 씩 깨우지 마라. 대기 중인
    // 상태 머신이 있을 때만 `set_next_fire_after_ms` 로 다음 만료 시각에 맞춰
    // 재예약하라." 기본 주기를 1시간으로 잡아 사실상 스스로는 발화하지 않게 해 두고,
    // `on_tap_event` 가 판정 대기 상태(§3-c `PendingDown`/`WaitingSecondTap`)를
    // 감지했을 때만 다음 만료 시각으로 재예약한다. M1 은 quick press/double tap
    // 액션이 없어(`lifecycle::quick_press_tick_delay_hint` 문서 참고) 재예약 경로에
    // 실제로 도달하지 않지만, 배선 자체는 M2 를 위해 지금부터 갖춰 둔다.
    const IDLE_TIMER_INTERVAL_MS: u64 = 3_600_000;
    let timer_cell = cell.clone();
    let timer = RepeatingTimer::new(
        IDLE_TIMER_INTERVAL_MS,
        Box::new(move || on_timer_tick(&timer_cell)),
    );
    timer.add_to_current_runloop();
    cell.borrow_mut().timer = Some(timer);

    let run_loop_for_handshake = RunLoopHandle::current()
        .expect("탭 스레드에 CFRunLoop 를 가져올 수 없다 — 있을 수 없는 상황");
    let _ = ready_tx.send(TapThreadHandshake {
        run_loop: run_loop_for_handshake,
        commands,
    });

    // 블로킹 — `EngineCommand::Shutdown` 처리(`drain_commands`)가 `run_loop_for_stop.
    // stop()` 을 부르거나, `Engine::shutdown()` 이 직접 정지시킬 때까지 리턴하지 않는다.
    RunLoopHandle::run();

    // 정리 — `docs/dev/architecture.md` §2.4 순서는 `EventTap`/`CommandSource`/
    // `RepeatingTimer` 각자의 `Drop` 이 구현한다. 여기서는 명시적으로 탭을 먼저
    // 놓아 준다.
    cell.borrow_mut().tap = None;
    tracing::info!("탭 스레드가 종료됐다");
}
