//! 이슈 #140(D1) — 탭 이벤트 마스크를 `EngineConfig` 에서 도출한다.
//!
//! 마우스 11종을 항상 받는 것이 구조적 상수 비용이다(커서 이동마다 WindowServer ⇄
//! 탭 스레드 동기 왕복). 이 모듈은 "지금 구성에서 마우스 이벤트 종류별로 실제
//! 소비될 수 있는가"만 판정하는 순수 함수를 둔다 — 비트 자체(`CGEventType` 매핑)는
//! `ultrakey-platform::event_tap::build_event_mask` 가 맡는다(계층 분리는
//! `docs/plan/issue-140-event-mask.md` §2 D1 근거 참고).

use crate::event::EventKind;
use crate::rules::{RuleAction, RuleTable};
use crate::settings::EngineConfig;

/// 마우스 이벤트 종류별로 탭이 실제로 받아야 하는가.
///
/// `hyperkey.md` §3.3 의 Click/Drag/Move/Scroll 네 체크박스에 대응한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MouseEventNeeds {
    pub click: bool,
    pub drag: bool,
    pub r#move: bool,
    pub scroll: bool,
}

impl MouseEventNeeds {
    /// 넷 중 하나라도 필요한가 — 전부 `false` 면 마우스 이벤트는 마스크에서
    /// 완전히 빠진다.
    pub fn any(&self) -> bool {
        self.click || self.drag || self.r#move || self.scroll
    }

    /// 주어진 이벤트 종류가 이 필요 목록에 포함되는가. 키 이벤트(`KeyDown`/
    /// `KeyUp`/`FlagsChanged`)는 항상 포함되고, 탭 비활성화 통지
    /// (`TapDisabledByTimeout`/`TapDisabledByUserInput`)는 마스크 멤버가 아니라
    /// macOS 가 무조건 보내는 통지이므로 항상 제외한다(`event_tap.rs` 모듈 문서).
    pub fn includes(&self, kind: EventKind) -> bool {
        if kind.is_key() {
            return true;
        }
        if kind.is_click() {
            return self.click;
        }
        if kind.is_drag() {
            return self.drag;
        }
        if kind.is_move() {
            return self.r#move;
        }
        if kind.is_scroll() {
            return self.scroll;
        }
        // TapDisabledByTimeout/TapDisabledByUserInput 등 마스크 멤버가 아닌 통지.
        false
    }
}

/// 합성 modifier flags(`active_synth_flags()`)가 생길 수 있는 규칙이 하나라도
/// 있는가 — arbitration.rs §5 C5 판정과 동일하다.
///
/// ⭐ **결합 지점**(이슈 #140). `active_synth_flags()` 의 원천은 지금 딱 둘뿐이다 —
/// ① `RuleTable::modifier_rules`(hyper/meh/bleh) ② `RuleTable::source_actions` 중
/// `hold_remap` 이 modifier 키를 대상으로 하는 프리셋(`keystate.rs::register_sources`
/// 가 이 둘을 합쳐 추적 슬롯을 만든다). 트랙패드 hyper flags
/// (`arbitration.rs::trackpad_hyper_flags`)도 ①과 같은 `modifier_rules` 를 읽으므로
/// 별도 조건이 필요 없다. **`active_synth_flags()` 에 새 원천이 추가되면 이 함수도
/// 함께 고쳐야 한다** — `keystate.rs::register_sources` 에도 같은 취지의 주석을
/// 남겨 둔다.
pub fn synthetic_modifier_flags_possible(rules: &RuleTable) -> bool {
    let modifier_rule_has_flags = rules.modifier_rules.iter().any(|r| !r.flags.is_empty());
    if modifier_rule_has_flags {
        return true;
    }
    rules.source_actions.iter().any(|sa| {
        matches!(
            sa.hold_remap,
            Some(RuleAction::Key { keycode, .. })
                if keycode.modifier_flags().map(|f| !f.is_empty()).unwrap_or(false)
        )
    })
}

/// `EngineConfig` 로부터 마우스 이벤트 종류별 필요 여부를 도출한다
/// (`docs/plan/issue-140-event-mask.md` §2 D1).
pub fn mouse_event_needs(cfg: &EngineConfig) -> MouseEventNeeds {
    let synth = synthetic_modifier_flags_possible(&cfg.rules);
    MouseEventNeeds {
        click: synth && cfg.mouse_apply.click,
        drag: synth && cfg.mouse_apply.drag,
        // ⭐ 트랙패드 커서 프리즈(C4)는 hyper/합성 flags 와 무관하게 `MouseMoved`
        // 를 소비한다 — 그래서 `trackpad_gesture_enabled` 하나만으로도 켜진다.
        r#move: cfg.trackpad_gesture_enabled || (synth && cfg.mouse_apply.r#move),
        scroll: synth && cfg.mouse_apply.scroll,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flags::EventFlags;
    use crate::keycode::KeyCode;
    use crate::rules::{ModifierKind, ModifierRule, SourceKeyActions};

    #[test]
    fn default_config_needs_nothing() {
        let cfg = EngineConfig::default();
        let needs = mouse_event_needs(&cfg);
        assert_eq!(needs, MouseEventNeeds::default());
        assert!(!needs.any());
    }

    #[test]
    fn hyper_modifier_rule_with_default_mouse_apply_needs_click_only() {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::COMMAND,
        });
        let needs = mouse_event_needs(&cfg);
        assert!(needs.click);
        assert!(!needs.drag);
        assert!(!needs.r#move);
        assert!(!needs.scroll);
    }

    /// 판정은 `ModifierKind` 에 무관하다 — meh/bleh 규칙도 flags 가 있으면 같은 결과다.
    #[test]
    fn meh_and_bleh_modifier_rules_count_the_same_as_hyper() {
        for kind in [ModifierKind::Meh, ModifierKind::Bleh] {
            let mut cfg = EngineConfig::default();
            cfg.rules.modifier_rules.push(ModifierRule {
                source: KeyCode::CAPS_LOCK,
                kind,
                flags: EventFlags::COMMAND,
            });
            let needs = mouse_event_needs(&cfg);
            assert!(needs.click, "{kind:?} 규칙도 click 을 요구해야 한다");
            assert!(!needs.r#move);
        }
    }

    /// A2 의 현실적인 구성 — hyper 규칙 + 트랙패드 제스처 + 기본 `mouse_apply`(click) →
    /// click 과 move 둘 다.
    #[test]
    fn hyper_rule_plus_trackpad_gesture_needs_click_and_move() {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::COMMAND,
        });
        cfg.trackpad_gesture_enabled = true;
        let needs = mouse_event_needs(&cfg);
        assert!(needs.click);
        assert!(needs.r#move);
        assert!(!needs.drag);
        assert!(!needs.scroll);
    }

    #[test]
    fn hyper_modifier_rule_with_all_mouse_apply_needs_all_four() {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::COMMAND,
        });
        cfg.mouse_apply.drag = true;
        cfg.mouse_apply.r#move = true;
        cfg.mouse_apply.scroll = true;
        let needs = mouse_event_needs(&cfg);
        assert!(needs.click);
        assert!(needs.drag);
        assert!(needs.r#move);
        assert!(needs.scroll);
        assert!(needs.any());
    }

    #[test]
    fn trackpad_gesture_enabled_alone_needs_move_only() {
        let cfg = EngineConfig {
            trackpad_gesture_enabled: true,
            ..EngineConfig::default()
        };
        let needs = mouse_event_needs(&cfg);
        assert!(!needs.click);
        assert!(!needs.drag);
        assert!(needs.r#move);
        assert!(!needs.scroll);
    }

    #[test]
    fn modifier_hold_remap_target_needs_click_with_default_mouse_apply() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: None,
            double_tap: None,
            hold_remap: Some(RuleAction::Key {
                keycode: KeyCode::LEFT_CONTROL,
                flags: EventFlags::NONE,
            }),
        });
        let needs = mouse_event_needs(&cfg);
        assert!(needs.click);
        assert!(!needs.drag);
        assert!(!needs.r#move);
        assert!(!needs.scroll);
    }

    #[test]
    fn non_modifier_hold_remap_target_needs_nothing() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: None,
            double_tap: None,
            hold_remap: Some(RuleAction::Key {
                keycode: KeyCode::ANSI_A,
                flags: EventFlags::NONE,
            }),
        });
        let needs = mouse_event_needs(&cfg);
        assert!(!needs.any());
    }

    #[test]
    fn modifier_rule_with_empty_flags_needs_nothing() {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::NONE,
        });
        let needs = mouse_event_needs(&cfg);
        assert!(!needs.any());
    }

    #[test]
    fn includes_key_kinds_always_true_and_tap_disabled_kinds_always_false() {
        let needs = MouseEventNeeds::default();
        assert!(needs.includes(EventKind::KeyDown));
        assert!(needs.includes(EventKind::KeyUp));
        assert!(needs.includes(EventKind::FlagsChanged));
        assert!(!needs.includes(EventKind::TapDisabledByTimeout));
        assert!(!needs.includes(EventKind::TapDisabledByUserInput));

        let all = MouseEventNeeds {
            click: true,
            drag: true,
            r#move: true,
            scroll: true,
        };
        assert!(!all.includes(EventKind::TapDisabledByTimeout));
        assert!(!all.includes(EventKind::TapDisabledByUserInput));
    }
}
