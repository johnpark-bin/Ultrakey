//! 워치독 — `docs/dev/architecture.md` §2.1. ⭐ **탭 스레드가 아니라 별도 스레드에서
//! 돈다** — 탭 런루프 자체가 막혀도(콜백이 멈춰 있어도) 감지할 수 있어야 하기 때문이다.
//! `timings.watchdog_poll_ms`(기본 1000ms) 주기로 [`TapHealthProbe::is_enabled`] 를
//! 폴링하고, `false` 면 [`EngineCommand::RecoverTap`] 을 보낸다.
//!
//! 탭이 아직 설치되지 않은 동안(`probe` 슬롯이 비어 있는 동안)은 폴링 대상이 없으므로
//! 아무 것도 하지 않는다 — `NotInstalled`/`Terminated` 상태에서 재활성화를 스팸하지
//! 않기 위함이다.
//!
//! ⭐ 이슈 #108 — 같은 폴링 주기에 caps lock 자동 복구 안전망도 얹는다(새 타이머
//! 없음). 판정은 [`crate::lifecycle::caps_lock_recovery`](순수 함수)에 위임한다.

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use arc_swap::ArcSwapOption;
use crossbeam_channel::{bounded, Receiver, RecvTimeoutError, Sender};

use ultrakey_platform::event_tap::TapHealthProbe;
use ultrakey_platform::secure_input::{DirectSecureInputProbe, SecureInputProbe};

use crate::command::{CommandChannel, EngineCommand, TapDisableReason};
use crate::state::SharedState;

pub struct Watchdog {
    thread: Option<JoinHandle<()>>,
    shutdown_tx: Sender<()>,
}

impl Watchdog {
    /// `probe` 는 탭 스레드가 탭을 만들거나 없앨 때마다 갱신하는 슬롯이다
    /// (`ultrakey_platform::event_tap::EventTap::health_probe`).
    /// `secure_input` 은 `DirectSecureInputProbe` 단위형이라 복제 자유 — 호출자
    /// (`Engine::start`)가 값으로 넘긴다. 새 스레드 없이 기존 폴링 주기 안에서
    /// 읽고, ⭐ 이슈 #152 관측성: 전이에서만 `SecureInputChanged` 를 발송한다.
    pub fn spawn(
        shared: Arc<SharedState>,
        probe: Arc<ArcSwapOption<TapHealthProbe>>,
        commands: CommandChannel,
        secure_input: DirectSecureInputProbe,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = bounded::<()>(0);
        let thread = thread::Builder::new()
            .name("ultrakey-watchdog".to_string())
            .spawn(move || watchdog_loop(shared, probe, commands, secure_input, shutdown_rx))
            .expect("워치독 스레드 생성 실패");
        Watchdog {
            thread: Some(thread),
            shutdown_tx,
        }
    }

    /// ⚠️ `recv_timeout`/`select` 로 종료 신호를 받아 스레드를 반드시 정리한다
    /// (스레드 누수 금지 — `docs/dev/architecture.md` §2.1, 위임 지시 "스레드를 누수시키지 마라").
    pub fn shutdown(mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn watchdog_loop(
    shared: Arc<SharedState>,
    probe: Arc<ArcSwapOption<TapHealthProbe>>,
    commands: CommandChannel,
    secure_input: DirectSecureInputProbe,
    shutdown_rx: Receiver<()>,
) {
    // ⭐ 이슈 #152 관측성 — 마지막으로 **게시한** 상태. 초기값은 굳힌 `false` 다:
    // 앱(트레이)의 기본 표식이 "중단 아님"이므로, 기동 때부터 Secure Input 이
    // 켜져 있으면 첫 폴링이 ON→전이로 게시해야 무표식 정지가 남지 않는다.
    // (기동 시 1회 `is_enabled()` 로 초기화하면 이미 켜진 상태가 전이로 안 잡혀
    // 이 관측성의 핵심 케이스가 사라진다.)
    let mut last_secure_input = false;
    loop {
        let poll_ms = shared.config.load().timings.watchdog_poll_ms;
        match shutdown_rx.recv_timeout(Duration::from_millis(poll_ms)) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }

        if let Some(p) = probe.load_full() {
            if !p.is_enabled() {
                // ⭐ 이슈 #149 D2 — 통지 없이 꺼진 탭. silent 예산으로 재활성화를
                // 시도하는 유일한 경로다.
                tracing::warn!("watchdog: tap detected disabled; requesting re-enable");
                commands.send(EngineCommand::RecoverTap {
                    reason: TapDisableReason::WatchdogSilent,
                });
            }
        }

        // ⭐ 이슈 #152 관측성 — Secure Input 전이 감지. 기존 폴링 주기에 얹는다
        // (새 스레드 없음). 0-d 게이트 안에 있어 이벤트가 탭에 닿지 않는 구간을
        // 전이에서만 `SecureInputChanged` 로 게시한다 — 같은 값 반복은 발송하지
        // 않는다. caps-lock 안전망(#108/#144)은 아래 그대로 둔다.
        let now_secure = secure_input.is_enabled();
        if let Some(changed) = secure_input_transition(last_secure_input, now_secure) {
            last_secure_input = now_secure;
            commands.send(EngineCommand::SecureInputChanged(changed));
        }

        // ⭐ 이슈 #108 자동 복구 안전망. `alias_active` 검사를 가장 먼저 두어, D-1 이
        // 설치되지 않은 구성(caps lock 이 modifier 소스가 아님)에서는
        // `caps_lock_state()`(mach 왕복 1회)조차 부르지 않는다 — 새 타이머를 만들지
        // 않고 이미 있는 폴링 주기(`watchdog_poll_ms`)에 얹는다.
        // ⭐ 이슈 #144 — suspended 중에는 물리 Capslock 이 네이티브라 관측된 잠금은
        // 사용자 것이다. `caps_lock_recovery` 가 `d1_suspended` 를 보면 개입하지
        // 않으므로, 여기서도 같은 플래그를 읽어 넘긴다. 검사는 `alias_active` 다음에
        // 두어 D-1 비활성 구성에서는 mach 호출을 건너뛰는 기존 최적화를 유지한다.
        let alias_active = shared.config.load().caps_lock_alias.is_some();
        if alias_active {
            let d1_suspended = shared.d1_suspended.load(Ordering::Relaxed);
            if d1_suspended {
                continue;
            }
            let observed = ultrakey_platform::hid_lock::caps_lock_state();
            let owned = shared.caps_lock_owned_lock.load(Ordering::Relaxed);
            if crate::lifecycle::caps_lock_recovery(alias_active, d1_suspended, observed, owned) {
                tracing::warn!(
                    "watchdog: caps lock hardware lock observed without a matching \
                     Effect::ToggleCapsLock; reverting it (issue #108 safety net)"
                );
                ultrakey_platform::hid_lock::set_caps_lock_state(false);
            }
        }
    }
    tracing::debug!("watchdog thread exiting");
}

/// ⭐ 이슈 #152 관측성 — Secure Input 전이 판정 순수 함수. 같으면 `None`(발송
/// 없음), 다르면 새 값 `Some(bool)`(발송). 워치독 루프의 발송 횟수 회귀 테스트가
/// 이 함수로 ON→OFF→ON 3회 전이를 고정한다.
fn secure_input_transition(previous: bool, current: bool) -> Option<bool> {
    if previous == current {
        None
    } else {
        Some(current)
    }
}

#[cfg(test)]
mod tests {
    use super::secure_input_transition;

    /// ⭐ 이슈 #152 — ON→OFF→ON 에서 3회만 발송(같은 값 반복은 발송 없음).
    #[test]
    fn secure_input_transition_fires_only_on_change() {
        assert_eq!(secure_input_transition(false, false), None);
        assert_eq!(secure_input_transition(true, true), None);
        assert_eq!(secure_input_transition(false, true), Some(true));
        assert_eq!(secure_input_transition(true, false), Some(false));

        // ON→OFF→ON 시퀀스: 매 폴링이 전이에서만 발송하면 정확히 3회다.
        let sequence = [true, true, false, false, true, true];
        let mut previous = false;
        let mut sends = 0;
        for current in sequence {
            if secure_input_transition(previous, current).is_some() {
                sends += 1;
                previous = current;
            }
        }
        assert_eq!(sends, 3);
    }
}
