//! Quick press(탭 vs 홀드) 판정 상태 머신 — `key-remapping-engine.md` §3-c 표를 그대로 구현한다.
//!
//! 소스 키(물리 keycode) 하나당 이 상태 머신의 독립된 인스턴스가 하나 존재한다. hyper/meh/bleh
//! hold 판정, `Quick press caps lock to execute` 류, `Double tap shift = caps lock` 류가 모두
//! 이 상태 머신 하나를 공유한다 — 그것이 v1.20/v1.62 부류 버그를 구조적으로 막는 지점이다.
//!
//! ⭐ **재해석 — F-07 이 정본이다(2026-08-30, 이슈 #5).** `hyperkey.md` §3.2 는 "소스 키
//! keyDown 시점에 곧바로 합성 flagsChanged 를 낸다"고 쓰고, 이 문서(§3-b 계층 2)는
//! "`HoldConfirmed` 로 판정된 상태"를 조건으로 한다 — 둘은 문면상 충돌한다. `hyperkey.md` §1 이
//! "메커니즘은 F-07 이 제공한다"고 명시하므로 F-07(이 상태 머신)이 정본이며, 다음 최적화로
//! 충돌을 해소한다: **그 소스 키에 quick press 액션이 등록되어 있지 않으면, keyDown 즉시
//! `HoldConfirmed` 로 간다(보류하지 않는다).** 모호성이 없으므로 기다릴 이유가 없다. M1 에서
//! hyper/meh/bleh 는 quick press 액션이 없으므로(`RuleTable` 에 그런 액션 슬롯 자체가 없다)
//! 이 경로를 타고, 결과적으로 관측 가능한 동작은 `hyperkey.md` §3.2 의 서술과 일치한다.

use crate::time::Millis;

/// quick press 판정에 필요한 시간·설정값. `EngineConfig::timings` 에서 골라 조립한다.
#[derive(Debug, Clone, Copy)]
pub struct QuickPressConfig {
    pub quick_press_duration: Millis,
    pub double_tap_interval: Millis,
    /// 이 소스 키에 quick press 액션이 등록되어 있는가 — 위 모듈 문서의 재해석 최적화 조건.
    /// M1 에는 그런 액션 슬롯이 아직 없으므로 항상 `false` 다.
    pub has_quick_press_action: bool,
    /// 이 소스 키에 double tap 액션이 등록되어 있는가. M1 에는 없으므로 항상 `false`.
    pub has_double_tap_action: bool,
}

/// 상태 머신이 전이 시점에 방출하는 "액션" 신호. 실제 키 합성으로 옮기는 것은 호출자
/// (`arbitration.rs`)의 몫이다 — 이 모듈은 macOS 를 모른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickPressEvent {
    /// 소스 키가 modifier/hold 로 확정됨 — 계층 2 평가에 즉시 반영해야 한다.
    HoldStart,
    /// 소스 키의 hold 가 해제됨.
    HoldEnd,
    /// 단일 quick press 액션을 지금 방출한다(유예됐던 경우 포함).
    QuickPress,
    /// double tap 액션을 방출한다.
    DoubleTap,
}

/// §3-c 상태표의 상태 4종.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum QuickPressState {
    #[default]
    Idle,
    PendingDown { since: Millis },
    HoldConfirmed,
    WaitingSecondTap { first_up_at: Millis },
}

impl QuickPressState {
    /// 이 소스 키의 keyDown(§3-c 표 1·3·4행 + 6행 두 번째 탭).
    ///
    /// `autorepeat` 이 `true` 면 OS 가 보낸 키 반복 keyDown 이다 — ⭐ §5 엣지 케이스 13:
    /// 자동반복은 상태를 `PendingDown` 으로 되돌리지 않으며, 이미 `HoldConfirmed` 인 경우
    /// 아무 일도 하지 않는다(매 반복마다 `HoldStart` 를 다시 방출하면 안 된다).
    pub fn on_key_down(
        self,
        now: Millis,
        autorepeat: bool,
        cfg: &QuickPressConfig,
    ) -> (Self, Option<QuickPressEvent>) {
        match self {
            QuickPressState::Idle => {
                if autorepeat {
                    // Idle 상태에서 autorepeat 이 오는 것은 이례적이나(정상적으로는 non-repeat
                    // keyDown 이 먼저 온다), 방어적으로 아무 전이도 하지 않는다.
                    (QuickPressState::Idle, None)
                } else if !cfg.has_quick_press_action {
                    // 위 모듈 문서의 재해석 최적화: 대기할 이유가 없으므로 즉시 확정.
                    (QuickPressState::HoldConfirmed, Some(QuickPressEvent::HoldStart))
                } else {
                    (QuickPressState::PendingDown { since: now }, None)
                }
            }
            QuickPressState::PendingDown { since } => {
                // 동일 키의 반복 keyDown(§5 엣지 13) — PendingDown 을 그대로 유지한다.
                let _ = autorepeat;
                (QuickPressState::PendingDown { since }, None)
            }
            // ⭐ §5 엣지 13 핵심: 이미 확정된 상태에서는 autorepeat 이든 아니든 아무 것도
            // 하지 않는다 — 매 반복 keyDown 마다 새 HoldStart 를 내보내지 않는다.
            QuickPressState::HoldConfirmed => (QuickPressState::HoldConfirmed, None),
            QuickPressState::WaitingSecondTap { first_up_at } => {
                if !autorepeat && now.saturating_sub(first_up_at) < cfg.double_tap_interval {
                    // 두 번째 tap 의 keyDown — double tap 성립, 즉시 Idle 로 환원.
                    (QuickPressState::Idle, Some(QuickPressEvent::DoubleTap))
                } else {
                    (QuickPressState::WaitingSecondTap { first_up_at }, None)
                }
            }
        }
    }

    /// 이 소스 키의 keyUp(§3-c 표 2·5행).
    pub fn on_key_up(self, now: Millis, cfg: &QuickPressConfig) -> (Self, Option<QuickPressEvent>) {
        match self {
            QuickPressState::PendingDown { since } => {
                if now.saturating_sub(since) < cfg.quick_press_duration {
                    if cfg.has_double_tap_action {
                        (QuickPressState::WaitingSecondTap { first_up_at: now }, None)
                    } else {
                        (QuickPressState::Idle, Some(QuickPressEvent::QuickPress))
                    }
                } else {
                    // 방어적 경로: 정상적으로는 타이머 만료가 먼저 HoldConfirmed 로 전이시켰어야
                    // 한다(on_tick). 그러나 tick 이 늦게 도착했을 수 있으므로 hold 해제로 취급한다.
                    (QuickPressState::Idle, Some(QuickPressEvent::HoldEnd))
                }
            }
            QuickPressState::HoldConfirmed => (QuickPressState::Idle, Some(QuickPressEvent::HoldEnd)),
            other => (other, None),
        }
    }

    /// **다른** 키의 keyDown 수신(§3-c 표 3행) — v1.62 예방의 핵심 지점.
    pub fn on_other_key_down(self) -> (Self, Option<QuickPressEvent>) {
        match self {
            QuickPressState::PendingDown { .. } => {
                (QuickPressState::HoldConfirmed, Some(QuickPressEvent::HoldStart))
            }
            other => (other, None),
        }
    }

    /// 주기적/이벤트 유도 타이머 점검(§3-c 표 4·7행) — `Arbiter::on_tick` 이 호출한다.
    pub fn on_tick(self, now: Millis, cfg: &QuickPressConfig) -> (Self, Option<QuickPressEvent>) {
        match self {
            QuickPressState::PendingDown { since }
                if now.saturating_sub(since) >= cfg.quick_press_duration =>
            {
                (QuickPressState::HoldConfirmed, Some(QuickPressEvent::HoldStart))
            }
            QuickPressState::WaitingSecondTap { first_up_at }
                if now.saturating_sub(first_up_at) >= cfg.double_tap_interval =>
            {
                // 두 번째 tap 이 오지 않았다 — 유예됐던 단일 quick press 를 지금 방출한다.
                (QuickPressState::Idle, Some(QuickPressEvent::QuickPress))
            }
            other => (other, None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(has_quick_press: bool) -> QuickPressConfig {
        QuickPressConfig {
            quick_press_duration: Millis(1000),
            double_tap_interval: Millis(300),
            has_quick_press_action: has_quick_press,
            has_double_tap_action: false,
        }
    }

    /// ⭐ 재해석 검증(테스트 #11): quick press 액션이 없는 소스 키는 keyDown 즉시 HoldConfirmed.
    #[test]
    fn no_quick_press_action_confirms_hold_immediately() {
        let (next, ev) = QuickPressState::Idle.on_key_down(Millis(0), false, &cfg(false));
        assert_eq!(next, QuickPressState::HoldConfirmed);
        assert_eq!(ev, Some(QuickPressEvent::HoldStart));
    }

    #[test]
    fn quick_press_action_buffers_into_pending_down() {
        let (next, ev) = QuickPressState::Idle.on_key_down(Millis(0), false, &cfg(true));
        assert_eq!(next, QuickPressState::PendingDown { since: Millis(0) });
        assert_eq!(ev, None);
    }

    #[test]
    fn pending_down_same_key_up_before_duration_emits_quick_press() {
        let pending = QuickPressState::PendingDown { since: Millis(0) };
        let (next, ev) = pending.on_key_up(Millis(200), &cfg(true));
        assert_eq!(next, QuickPressState::Idle);
        assert_eq!(ev, Some(QuickPressEvent::QuickPress));
    }

    /// §3-c 표 3행: 다른 키 keyDown 은 즉시 HoldConfirmed 로 확정한다.
    #[test]
    fn other_key_down_confirms_hold_immediately() {
        let pending = QuickPressState::PendingDown { since: Millis(0) };
        let (next, ev) = pending.on_other_key_down();
        assert_eq!(next, QuickPressState::HoldConfirmed);
        assert_eq!(ev, Some(QuickPressEvent::HoldStart));
    }

    /// §3-c 표 4행: 타이머 만료로 HoldConfirmed 확정.
    #[test]
    fn timer_expiry_confirms_hold() {
        let pending = QuickPressState::PendingDown { since: Millis(0) };
        let (next, ev) = pending.on_tick(Millis(1000), &cfg(true));
        assert_eq!(next, QuickPressState::HoldConfirmed);
        assert_eq!(ev, Some(QuickPressEvent::HoldStart));

        // 아직 만료 전이면 유지.
        let (next2, ev2) = pending.on_tick(Millis(999), &cfg(true));
        assert_eq!(next2, pending);
        assert_eq!(ev2, None);
    }

    #[test]
    fn hold_confirmed_key_up_returns_to_idle() {
        let (next, ev) = QuickPressState::HoldConfirmed.on_key_up(Millis(5000), &cfg(true));
        assert_eq!(next, QuickPressState::Idle);
        assert_eq!(ev, Some(QuickPressEvent::HoldEnd));
    }

    /// ⭐ §5 엣지 13: autorepeat keyDown 은 PendingDown 으로 되돌리지 않고, HoldConfirmed
    /// 상태에서 반복돼도 다시 HoldStart 를 내지 않는다.
    #[test]
    fn autorepeat_does_not_reset_or_re_emit() {
        let mut cfg_v = cfg(true);
        cfg_v.has_quick_press_action = true;

        // HoldConfirmed 상태에서 autorepeat 이 반복돼도 그대로 유지, 이벤트 없음.
        let (next, ev) = QuickPressState::HoldConfirmed.on_key_down(Millis(100), true, &cfg_v);
        assert_eq!(next, QuickPressState::HoldConfirmed);
        assert_eq!(ev, None);

        // PendingDown 상태에서 (이례적이지만) autorepeat 이 와도 PendingDown 유지.
        let pending = QuickPressState::PendingDown { since: Millis(0) };
        let (next2, ev2) = pending.on_key_down(Millis(50), true, &cfg_v);
        assert_eq!(next2, pending);
        assert_eq!(ev2, None);
    }

    #[test]
    fn waiting_second_tap_double_tap_confirmed_within_interval() {
        let waiting = QuickPressState::WaitingSecondTap { first_up_at: Millis(100) };
        let (next, ev) = waiting.on_key_down(Millis(300), false, &cfg(true));
        assert_eq!(next, QuickPressState::Idle);
        assert_eq!(ev, Some(QuickPressEvent::DoubleTap));
    }

    #[test]
    fn waiting_second_tap_timeout_emits_deferred_quick_press() {
        let waiting = QuickPressState::WaitingSecondTap { first_up_at: Millis(0) };
        let (next, ev) = waiting.on_tick(Millis(300), &cfg(true));
        assert_eq!(next, QuickPressState::Idle);
        assert_eq!(ev, Some(QuickPressEvent::QuickPress));
    }
}
