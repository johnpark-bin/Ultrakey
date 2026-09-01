//! ⭐ F-06 — 트랙패드·Magic Mouse hyper 제스처 리스너(`trackpad-hyper-gesture.md`).
//!
//! `ultrakey-hyperkey::gesture` 가 이미 순수 상태 머신을 갖고 있다 — 이 파일이 하는
//! 일은 그 머신을 **리스너 전용 스레드**에 얹고, `ultrakey-platform::multitouch` 의
//! 비공개 FFI 로 원시 접촉 프레임을 받아 머신에 먹이고, 머신의 전이를 중재기 게이트
//! (`SharedState.trackpad`, `ultrakey-core::trackpad`)에 게시하는 조립이다. seek.rs
//! 워커와 같은 모양 — 상태는 이 스레드가 **단독 소유**하므로 락이 없다.
//!
//! ## 스레드 모양
//!
//! - **리스너 스레드**(이 파일의 [`spawn`]) — 멀티터치 API 를 로드하고, 장치마다
//!   콜백 세션을 만든 뒤, 프레임·명령·워치독 틱을 한 루프에서 소비한다.
//! - **멀티터치 콜백**(`MultitouchApi::start_session` 트램폴린) — 멀티터치
//!   프레임워크 스레드에서 불린다. 콜백은 프레임을 채널에 밀어 넣고 즉시 반환한다 —
//!   상태 전이·게이트 게시는 리스너 스레드가 단일 소유자로서 순차 처리한다.
//!
//! ## 격하(degradable) 계약(§8)
//!
//! - `MultitouchApi::load()` 실패(프레임워크 부재·심벌 누락) → 리스너 스레드가
//!   즉시 끝난다. 게이트는 초기값 `Off` 로 남고 다른 기능은 무영향이다.
//! - 워치독 재시작이 [`RESTART_MAX`] 를 넘으면 이 기능만 포기한다 — 게이트는 `Off`
//!   로 닫히고 hyper 물리 키 경로(F-05)는 무영향이다.
//! - 장치가 하나도 없으면(§5 항목 1) 리스너는 프레임을 받지 못해 FSM 이 움직이지
//!   않는다 — "장치 목록이 비면 절대 hyper 를 활성화하지 않는다" 불변식은 입력이
//!   없는 것만으로 성립한다.
//!
//! ## 로그
//!
//! 상태 변화(Engaged/Released/재시작/격하/등록·해제)에만 남긴다 — 접촉 프레임은
//! 초당 수십 회 도착하므로 프레임 단위 로그는 폭주가 된다(§2.2 정신).

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossbeam_channel::{unbounded, Receiver, Sender};

use ultrakey_core::time::Millis;
use ultrakey_core::trackpad::TrackpadPhase;
use ultrakey_engine::SharedState;
use ultrakey_hyperkey::gesture::{
    GestureFrame, GestureMachine, GestureParams, GestureTouch, GestureVerdict,
};
use ultrakey_hyperkey::TrackpadArea;

/// 워치독 — 이 시간 동안 프레임이 없으면 세션을 재시작한다. §5 항목 6 의 `(추정)`
/// 값(2초)을 채택하고 architecture.md §4 관례대로 상수·근거를 남긴다.
const WATCHDOG_STALE_MS: u64 = 2000;
/// 재시작 상한 — 이 횟수를 넘으면 이 기능만 포기한다(§8 격하).
const RESTART_MAX: u32 = 3;
/// 워치독 틱(`recv_timeout` 타임아웃) — 별도 폴링 스레드 없이 구현한다.
const WATCHDOG_TICK: Duration = Duration::from_millis(500);

/// 리스너가 살아 있는 동안 쓰는 런타임 설정 — `HyperkeySettings.trackpad` 의 미러.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TrackpadRuntimeConfig {
    pub enabled: bool,
    pub area: TrackpadArea,
}

impl TrackpadRuntimeConfig {
    fn params(self) -> GestureParams {
        GestureParams {
            area: self.area,
            ..GestureParams::default()
        }
    }
}

/// 앱 → 리스너 명령.
enum TrackpadSignal {
    /// 설정 변경(영역·활성) — Engaged 였다면 강제 해제(§5 항목 12) 후 재구성.
    Reconfigure(TrackpadRuntimeConfig),
    /// 절전 복귀 등(§5 항목 7) — 세션을 전부 다시 만든다.
    Restart,
    Shutdown,
}

/// 리스너를 띄운다.
///
/// `shared.trackpad` 게이트는 이 리스너가 유일한 `store` 자다. `MultitouchApi` 로드가
/// 실패하면(비공개 프레임워크 부재·심벌 누락) **이 기능만 조용히 비활성화**된다 —
/// 리스너 스레드는 게이트를 전혀 만지지 않고(초기값 `Off`) 종료한다(§8 격하, 나머지
/// 기능 무영향). 스레드 생성 자체가 실패해도 마찬가지다.
pub(crate) fn spawn(shared: &Arc<SharedState>, config: TrackpadRuntimeConfig) -> TrackpadListener {
    let (signal_tx, signal_rx) = unbounded::<TrackpadSignal>();
    let (frame_tx, frame_rx) = unbounded::<(u64, Vec<GestureTouch>)>();
    let shared_for_thread = Arc::clone(shared);

    let spawned = thread::Builder::new()
        .name("ultrakey-trackpad".into())
        .spawn(move || {
            run_listener(signal_rx, frame_rx, frame_tx, shared_for_thread, config);
        });
    match spawned {
        Ok(thread) => TrackpadListener {
            commands: Some(signal_tx),
            thread: Some(thread),
        },
        Err(e) => {
            tracing::error!(
                error = %e,
                "failed to spawn the trackpad gesture thread; the gesture stays disabled (§8)"
            );
            TrackpadListener {
                commands: Some(signal_tx),
                thread: None,
            }
        }
    }
}

/// 살아 있는 리스너에 대한 손잡이 — `AppState` 가 들고 있다가 reconfigure/shutdown
/// 에 쓴다. 리스너가 격하되면 스레드가 이미 끝났지만 `commands` send 는 안전하다
/// (수신자가 없으면 조용히 버려진다).
pub(crate) struct TrackpadListener {
    commands: Option<Sender<TrackpadSignal>>,
    thread: Option<JoinHandle<()>>,
}

impl TrackpadListener {
    /// 설정 변경(영역·활성)을 리스너에 알린다. 스레드가 없으면(격하·비활성) 조용히
    /// 무시된다 — 게이트는 `Off` 를 유지한다.
    pub fn reconfigure(&self, config: TrackpadRuntimeConfig) {
        if let Some(tx) = self.commands.as_ref() {
            let _ = tx.send(TrackpadSignal::Reconfigure(config));
        }
    }

    /// 절전 복귀 등 — 세션을 전부 다시 만들게 한다(§5 항목 7).
    pub fn restart_sessions(&self) {
        if let Some(tx) = self.commands.as_ref() {
            let _ = tx.send(TrackpadSignal::Restart);
        }
    }

    /// 종료 — 스레드를 조인한다. 게이트는 스레드가 종료 전 직접 내려놓는다.
    pub fn shutdown(mut self) {
        if let Some(tx) = self.commands.take() {
            let _ = tx.send(TrackpadSignal::Shutdown);
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// 살아 있는 장치 세션 하나 — 장치 ID 와 그 장치의 스트림 핸들. 필드는 로그
/// (`Unregistered device`)와 워치독 폴링(`stop_stream`)에서 쓰인다.
struct DeviceSession {
    #[allow(dead_code)]
    device_id: u64,
    /// ⚠️ 존재 자체가 세션 수명이다 — `Drop` 이 unregister+release 를 한다.
    /// 값은 읽지 않지만 이 필드를 지우면 스트림이 즉시 끊긴다.
    #[allow(dead_code)]
    session: ultrakey_platform::multitouch::MtDeviceSession,
}

/// 리스너 본체 — 명령 수신·프레임 소비·워치독을 한 루프에서 처리한다.
fn run_listener(
    signals: Receiver<TrackpadSignal>,
    frames: Receiver<(u64, Vec<GestureTouch>)>,
    frame_tx: Sender<(u64, Vec<GestureTouch>)>,
    shared: Arc<SharedState>,
    initial: TrackpadRuntimeConfig,
) {
    // ⭐ 격하 게이트(§8) — 비공개 API 로드가 실패하면 여기서 끝난다. 게이트는
    // 한 번도 `store` 되지 않으므로(초기값 `Off`) 다른 기능은 무영향이다.
    let api = match ultrakey_platform::multitouch::MultitouchApi::load() {
        Ok(api) => api,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "MultitouchSupport symbols unavailable; the trackpad hyper gesture is disabled \
                 for this session (other functionality is unaffected, F-06 §8)"
            );
            return;
        }
    };

    let mut machine = GestureMachine::new(initial.params());
    let mut sessions: Vec<DeviceSession> = Vec::new();
    let mut last_frame_ms: Option<u64> = None;
    let mut restart_failures: u32 = 0;
    let mut enabled = initial.enabled;

    if enabled {
        start_sessions(&mut sessions, &api, &frame_tx);
    }

    loop {
        // 워치독 + 명령·프레임 대기를 하나의 `select` 로 합친다 — 별도 폴링
        // 스레드를 두지 않는다(architecture.md §2.3 "25ms 폴링 금지" 정신).
        // 타임아웃 = 워치독 틱(§5 항목 6).
        crossbeam_channel::select! {
            recv(signals) -> signal => match signal {
                Ok(TrackpadSignal::Reconfigure(cfg)) => {
                    // §5 항목 12 — 끄는 변경이면 Engaged 를 즉시 강제 해제한다.
                    let released = machine.force_release();
                    if matches!(released, GestureVerdict::HyperReleased) {
                        tracing::info!(
                            "trackpad gesture was turned off while engaged; forced a hyper release"
                        );
                    }
                    machine = GestureMachine::new(cfg.params());
                    drop(sessions.drain(..));
                    enabled = cfg.enabled;
                    if cfg.enabled {
                        start_sessions(&mut sessions, &api, &frame_tx);
                    }
                    shared.trackpad.store(machine.phase());
                    tracing::info!(
                        enabled = cfg.enabled,
                        area = cfg.area.as_str(),
                        "trackpad gesture reconfigured"
                    );
                }
                Ok(TrackpadSignal::Restart) => {
                    // 절전 복귀 등(§5 항목 7) — 핸들 만료 가능성에 대비해 세션을
                    // 전부 다시 만든다. Engaged 였다면 강제 해제가 선행한다.
                    let released = machine.force_release();
                    if matches!(released, GestureVerdict::HyperReleased) {
                        tracing::warn!(
                            "device invalidation at wake; forcing the hyper key release (watchdog)"
                        );
                    }
                    drop(sessions.drain(..));
                    if enabled {
                        start_sessions(&mut sessions, &api, &frame_tx);
                    }
                }
                Ok(TrackpadSignal::Shutdown) | Err(_) => break,
            },
            recv(frames) -> frame => match frame {
                Ok((device_id, touches)) => {
                    last_frame_ms = Some(now_ms());
                    restart_failures = 0;
                    let before = machine.phase();
                    let now = Millis(now_ms());
                    let verdict = machine.on_frame(&GestureFrame { touches: &touches }, now);
                    let after = machine.phase();
                    // 게이트 게시 — 로그는 상태 변화에만 남긴다(프레임 단위 로그 방지).
                    if before != after {
                        shared.trackpad.store(after);
                        match verdict {
                            GestureVerdict::HyperEngaged => tracing::info!(
                                device = device_id,
                                "trackpad edge slide engaged the hyper key (F-06)"
                            ),
                            GestureVerdict::HyperReleased => tracing::info!(
                                device = device_id,
                                "trackpad touch removed; hyper released"
                            ),
                            GestureVerdict::Cancelled => tracing::debug!(
                                device = device_id,
                                "gesture cancelled (second touch / missed threshold)"
                            ),
                            _ => {}
                        }
                    }
                }
                // 콜백 채널이 닫히는 일은 없다(리스너 자신이 sender 를 들고 죽는다) —
                // 그래도 닫히면 루프를 종료한다.
                Err(_) => break,
            },
            // 워치독 틱(§5 항목 6 — 원문 `Touches are not being detected.
            // Restarting.` 동등 동작). 프레임이 멈춘 채 Engaged 면 stuck hyper
            // 이므로 강제 해제 후 세션을 다시 만든다.
            default(WATCHDOG_TICK) => {
                if !enabled || sessions.is_empty() {
                    continue;
                }
                let now = now_ms();
                let elapsed = last_frame_ms.map(|t| now.saturating_sub(t)).unwrap_or(0);
                if elapsed <= WATCHDOG_STALE_MS {
                    continue;
                }
                let released = machine.force_release();
                shared.trackpad.store(machine.phase());
                if matches!(released, GestureVerdict::HyperReleased) {
                    tracing::warn!(
                        "touches are not being detected; forcing the hyper key release (watchdog)"
                    );
                }
                if restart_failures >= RESTART_MAX {
                    // ⭐ 상한 초과 — 이 기능만 조용히 포기한다(§8 격하). 나머지
                    // 기능(물리 키 hyper 리매핑 등)에는 무영향이다.
                    tracing::error!(
                        "trackpad listener restart limit reached; disabling the trackpad gesture \
                         (other functionality is unaffected)"
                    );
                    break;
                }
                tracing::warn!(
                    elapsed_ms = elapsed,
                    "touches are not being detected. Restarting."
                );
                restart_failures += 1;
                drop(sessions.drain(..));
                start_sessions(&mut sessions, &api, &frame_tx);
            }
        }
    }

    // 종료 — 게이트는 반드시 `Off` 로 닫는다(무영향 보장).
    drop(sessions);
    shared.trackpad.store(TrackpadPhase::Off);
}

/// 현재 연결된 모든 멀티터치 장치에 콜백을 등록해 스트림을 시작한다(§3.5 —
/// 트랙패드와 Magic Mouse 를 둘 다). 시작 실패는 그 장치만 포기한다(§8).
fn start_sessions(
    sessions: &mut Vec<DeviceSession>,
    api: &Arc<ultrakey_platform::multitouch::MultitouchApi>,
    frames_tx: &Sender<(u64, Vec<GestureTouch>)>,
) {
    for device in api.enumerate() {
        let device_id = device.device_id;
        let tx = frames_tx.clone();
        match api.start_session(
            device_id,
            Arc::new(move |frame: &ultrakey_platform::multitouch::TouchFrame| {
                // 콜백 — 멀티터치 스레드에서 불린다. 원시 접근의 마지막 지점에서
                // 순수 머신 타입으로 번역해 채널에 넣는다.
                let touches: Vec<GestureTouch> = frame.touches[..frame.len]
                    .iter()
                    .map(|t| GestureTouch {
                        path_index: t.path_index,
                        stage: t.stage,
                        x: t.x,
                        y: t.y,
                        mm_x: t.mm_x,
                        mm_y: t.mm_y,
                        major_axis: t.major_axis,
                        z_total: t.z_total,
                    })
                    .collect();
                let _ = tx.send((device_id, touches));
            }),
        ) {
            Ok(session) => {
                tracing::info!(
                    device = device_id,
                    built_in = device.built_in.unwrap_or(false),
                    force_touch = device.supports_force.unwrap_or(false),
                    "Registered multitouch device for the hyper gesture"
                );
                sessions.push(DeviceSession {
                    device_id,
                    session,
                });
            }
            Err(e) => {
                // 그 장치만 포기한다 — 나머지 장치와 이 기능에는 무영향(§8).
                tracing::warn!(
                    device = device_id,
                    error = %e,
                    "Unable to register device; skipping it"
                );
            }
        }
    }
}