//! 16종 설정 → 엔진 규칙 번역(`docs/dev/architecture.md` §6.3 계층 배정 표 · §6.6 채택값
//! 표를 그대로 구현한다).

use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::KeyCode;
use ultrakey_core::rules::{ComboRule, HoldCondition, RuleAction, RuleId, SimpleRemap, SourceKeyActions};

use crate::settings::PresetSettings;

/// 홈로우 11키의 물리 순서(`A S D F G H J K L ; '`, §6.6).
const HOME_ROW_KEYS: [KeyCode; 11] = [
    KeyCode::ANSI_A,
    KeyCode::ANSI_S,
    KeyCode::ANSI_D,
    KeyCode::ANSI_F,
    KeyCode::ANSI_G,
    KeyCode::ANSI_H,
    KeyCode::ANSI_J,
    KeyCode::ANSI_K,
    KeyCode::ANSI_L,
    KeyCode::ANSI_SEMICOLON,
    KeyCode::ANSI_QUOTE,
];

/// symbol row 스킴의 출력 문자 — `! @ # $ % ^ & * ( ) _`(§6.6, `A = !` 실측점의 자연스러운
/// 연장).
const HOME_ROW_SYMBOLS: [char; 11] = ['!', '@', '#', '$', '%', '^', '&', '*', '(', ')', '_'];

/// function row 스킴의 출력 키 — `F1`…`F11`(§6.6, `A = F1` 실측점의 자연스러운 연장).
const HOME_ROW_FUNCTION_KEYS: [KeyCode; 11] = [
    KeyCode::F1,
    KeyCode::F2,
    KeyCode::F3,
    KeyCode::F4,
    KeyCode::F5,
    KeyCode::F6,
    KeyCode::F7,
    KeyCode::F8,
    KeyCode::F9,
    KeyCode::F10,
    KeyCode::F11,
];

fn key_action(keycode: KeyCode) -> RuleAction {
    RuleAction::Key { keycode, flags: EventFlags::NONE }
}

/// 16종 설정을 번역한 결과 — `ultrakey_core::rules::RuleTable` 에 그대로 옮겨 담을 수 있다.
#[derive(Debug, Clone, Default)]
pub struct PresetRules {
    /// `RuleId` 오름차순으로 정렬돼 있다(architecture.md §6.4 서두 — 평가 순서를 입력
    /// 순서에 의존하지 않게 하기 위함).
    pub combos: Vec<ComboRule>,
    pub simple_remaps: Vec<SimpleRemap>,
    pub source_actions: Vec<SourceKeyActions>,
}

/// caps lock/좌우 shift 각각에 대해 quick press·double tap·hold_remap 을 누적해 담는
/// 내부 누산기. 여러 프리셋(예: F-08.8 과 F-08.11)이 같은 물리 키(shift)를 각각 다른
/// 액션으로 채울 수 있어, `SourceKeyActions` 를 프리셋마다 따로 만들면 나중 것이 앞의
/// 것을 조용히 덮어써 버린다(`RuleTable::source_actions_for` 는 첫 매치만 본다) — 그래서
/// 누산기 하나로 모은 뒤 마지막에 한 번씩만 `Vec` 에 넣는다.
#[derive(Default)]
struct SourceActionAccum {
    quick_press: Option<RuleAction>,
    double_tap: Option<RuleAction>,
    hold_remap: Option<RuleAction>,
}

impl SourceActionAccum {
    fn is_empty(&self) -> bool {
        self.quick_press.is_none() && self.double_tap.is_none() && self.hold_remap.is_none()
    }

    fn into_source_actions(self, key: KeyCode) -> Option<SourceKeyActions> {
        if self.is_empty() {
            None
        } else {
            Some(SourceKeyActions {
                key,
                quick_press: self.quick_press,
                double_tap: self.double_tap,
                hold_remap: self.hold_remap,
            })
        }
    }
}

impl PresetSettings {
    /// D-1 — caps lock 모멘터리 정규화가 필요한가(`docs/dev/architecture.md` §6.1).
    /// F-08.1·2·4·5·6·7·10 중 하나라도 켜져 있거나, hyper/meh/bleh 소스가 caps lock 이면
    /// 참이다. F-08.8/9 는 caps lock 을 **출력**(경로 C 토글 대상)으로만 쓰고 입력 조건으로
    /// 읽지 않으므로 여기 포함하지 않는다 — architecture.md §6.1 의 목록과 정확히 같다.
    pub fn needs_caps_lock_alias(&self, caps_is_modifier_source: bool) -> bool {
        caps_is_modifier_source
            || self.caps_lock_remap.enabled
            || self.caps_quick_press.enabled
            || self.caps_space_enter
            || self.caps_wasd_arrows
            || self.caps_hjkl_arrows.enabled
            || self.caps_home_row.enabled
            || self.shift_caps_to_caps
    }

    /// 16종 설정 → 엔진 규칙. hyper 설정(caps lock 이 hyper/meh/bleh 소스 키인가)도
    /// F-08.1 과의 충돌 판정(R2 예외)에는 필요 없지만, 시그니처를 architecture.md 가 정한
    /// 그대로 유지한다 — 규칙 생성 자체는 이 인자를 읽지 않는다(F-08.1 은 아예 별도
    /// 설정이고, 충돌은 UI 단에서 `detect_conflict` 로 미리 막는다).
    pub fn to_rules(&self, _caps_is_modifier_source: bool) -> PresetRules {
        let mut combos: Vec<ComboRule> = Vec::new();
        let mut simple_remaps: Vec<SimpleRemap> = Vec::new();

        let mut caps_lock_actions = SourceActionAccum::default();
        let mut left_shift_actions = SourceActionAccum::default();
        let mut right_shift_actions = SourceActionAccum::default();

        // F-08.1 — Remap caps lock to:
        if self.caps_lock_remap.enabled {
            if let Some(action) = self.caps_lock_remap.target.action() {
                caps_lock_actions.hold_remap = Some(action);
            }
            // target.action() 이 None 인 경우(F21~F24, keycode 미확정)는 조용히 아무
            // 규칙도 만들지 않는다 — UI 가 이 팝업에서 그 넷을 선택하지 못하게 막거나
            // 선택 시 경고해야 한다(popups.rs 문서 주석).
        }

        // F-08.2 — Quick press caps lock to execute:
        if self.caps_quick_press.enabled {
            caps_lock_actions.quick_press = Some(self.caps_quick_press.action.action());
        }

        // F-08.4 — Caps lock + space = enter
        if self.caps_space_enter {
            combos.push(ComboRule {
                id: RuleId::Preset(4),
                hold: HoldCondition::Key(KeyCode::CAPS_LOCK),
                trigger: KeyCode::SPACE,
                action: key_action(KeyCode::RETURN),
            });
        }

        // F-08.5 — Caps lock + W A S D = ▲ ◀ ▼ ▶ (게임 이동 키 관례, 물리 위치 기반)
        if self.caps_wasd_arrows {
            let wasd: [(KeyCode, KeyCode); 4] = [
                (KeyCode::ANSI_W, KeyCode::UP_ARROW),
                (KeyCode::ANSI_A, KeyCode::LEFT_ARROW),
                (KeyCode::ANSI_S, KeyCode::DOWN_ARROW),
                (KeyCode::ANSI_D, KeyCode::RIGHT_ARROW),
            ];
            for (trigger, target) in wasd {
                combos.push(ComboRule {
                    id: RuleId::Preset(5),
                    hold: HoldCondition::Key(KeyCode::CAPS_LOCK),
                    trigger,
                    action: key_action(target),
                });
            }
        }

        // F-08.6 — Caps lock + [H J K L / I J K L] = ◀ ▼ ▲ ▶ (vim 방향 관례)
        if self.caps_hjkl_arrows.enabled {
            let (h_or_i, j, k, l) = self.caps_hjkl_arrows.key_set.keys();
            let vim: [(KeyCode, KeyCode); 4] = [
                (h_or_i, KeyCode::LEFT_ARROW),
                (j, KeyCode::DOWN_ARROW),
                (k, KeyCode::UP_ARROW),
                (l, KeyCode::RIGHT_ARROW),
            ];
            for (trigger, target) in vim {
                combos.push(ComboRule {
                    id: RuleId::Preset(6),
                    hold: HoldCondition::Key(KeyCode::CAPS_LOCK),
                    trigger,
                    action: key_action(target),
                });
            }
        }

        // F-08.7 — Caps lock + home row = [symbol row / function row]
        if self.caps_home_row.enabled {
            use crate::popups::HomeRowScheme;
            for i in 0..HOME_ROW_KEYS.len() {
                let action = match self.caps_home_row.scheme {
                    HomeRowScheme::SymbolRow => RuleAction::Text(HOME_ROW_SYMBOLS[i]),
                    HomeRowScheme::FunctionRow => key_action(HOME_ROW_FUNCTION_KEYS[i]),
                };
                combos.push(ComboRule {
                    id: RuleId::Preset(7),
                    hold: HoldCondition::Key(KeyCode::CAPS_LOCK),
                    trigger: HOME_ROW_KEYS[i],
                    action,
                });
            }
        }

        // F-08.8 — Double tap shift = caps lock (경로 C). 좌/우 shift 각각 독립적으로
        // double tap 을 감지한다(§3.2 F-08.8 "좌/우 특정 여부 미확인" — 이 구현은 양쪽
        // 모두에 적용해 어느 쪽을 두드려도 동작하게 한다).
        if self.double_tap_shift_to_caps {
            left_shift_actions.double_tap = Some(RuleAction::ToggleCapsLock);
            right_shift_actions.double_tap = Some(RuleAction::ToggleCapsLock);
        }

        // F-08.9 — Left shift + right shift = caps lock (경로 C). 어느 쪽이 나중에
        // 내려오든 발화해야 하므로 트리거·hold 를 서로 바꾼 두 규칙을 만든다.
        if self.left_right_shift_to_caps {
            combos.push(ComboRule {
                id: RuleId::Preset(9),
                hold: HoldCondition::Key(KeyCode::RIGHT_SHIFT),
                trigger: KeyCode::LEFT_SHIFT,
                action: RuleAction::ToggleCapsLock,
            });
            combos.push(ComboRule {
                id: RuleId::Preset(9),
                hold: HoldCondition::Key(KeyCode::LEFT_SHIFT),
                trigger: KeyCode::RIGHT_SHIFT,
                action: RuleAction::ToggleCapsLock,
            });
        }

        // F-08.10 — Shift + caps lock = caps lock (경로 C).
        if self.shift_caps_to_caps {
            combos.push(ComboRule {
                id: RuleId::Preset(10),
                hold: HoldCondition::EitherShift,
                trigger: KeyCode::CAPS_LOCK,
                action: RuleAction::ToggleCapsLock,
            });
        }

        // F-08.11 — Quick press left/right shift to input corresponding: (유니코드 출력)
        if self.shift_quick_press_brackets.enabled {
            let (left_char, right_char) = self.shift_quick_press_brackets.pair.pair();
            left_shift_actions.quick_press = Some(RuleAction::Text(left_char));
            right_shift_actions.quick_press = Some(RuleAction::Text(right_char));
        }

        // F-08.12 — Hyper + delete = forward delete (논리 hyper 신호 구독, R4/P8).
        if self.hyper_delete_to_forward {
            combos.push(ComboRule {
                id: RuleId::Preset(12),
                hold: HoldCondition::HyperActive,
                trigger: KeyCode::DELETE,
                action: key_action(KeyCode::FORWARD_DELETE),
            });
        }

        // F-08.13 — Remap delete to forward delete (계층 4, 단순 리매핑).
        if self.delete_to_forward {
            simple_remaps.push(SimpleRemap {
                id: RuleId::Preset(13),
                from: KeyCode::DELETE,
                to: KeyCode::FORWARD_DELETE,
                add_flags: EventFlags::NONE,
            });
        }

        // F-08.14 — Shift + delete = forward delete.
        if self.shift_delete_to_forward {
            combos.push(ComboRule {
                id: RuleId::Preset(14),
                hold: HoldCondition::EitherShift,
                trigger: KeyCode::DELETE,
                action: key_action(KeyCode::FORWARD_DELETE),
            });
        }

        // F-08.15 — Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):
        if self.paste_without_formatting.enabled {
            use crate::popups::PasteTrigger;
            let hold = match self.paste_without_formatting.trigger {
                PasteTrigger::RightCommand => HoldCondition::Key(KeyCode::RIGHT_COMMAND),
                PasteTrigger::LeftCommand => HoldCondition::Key(KeyCode::LEFT_COMMAND),
                PasteTrigger::EitherCommand => HoldCondition::EitherCommand,
                // F-08.12 에 이어 F-05 의 논리 hyper 신호를 구독하는 두 번째 프리셋(R4 와
                // 나란히 다룸, §3.2 F-08.15).
                PasteTrigger::HyperKey => HoldCondition::HyperActive,
            };
            combos.push(ComboRule {
                id: RuleId::Preset(15),
                hold,
                trigger: KeyCode::ANSI_V,
                action: RuleAction::Key {
                    keycode: KeyCode::ANSI_V,
                    flags: EventFlags::COMMAND | EventFlags::ALTERNATE | EventFlags::SHIFT,
                },
            });
        }

        // F-08.16 — Home & end operate on lines (계층 4). macOS 에서 Home/End 는 문서
        // 처음/끝, ⌘←/⌘→ 는 줄 처음/끝이다(§6.6 채택값).
        if self.home_end_on_lines {
            simple_remaps.push(SimpleRemap {
                id: RuleId::Preset(16),
                from: KeyCode::HOME,
                to: KeyCode::LEFT_ARROW,
                add_flags: EventFlags::COMMAND,
            });
            simple_remaps.push(SimpleRemap {
                id: RuleId::Preset(16),
                from: KeyCode::END,
                to: KeyCode::RIGHT_ARROW,
                add_flags: EventFlags::COMMAND,
            });
        }

        let mut source_actions = Vec::new();
        if let Some(sa) = caps_lock_actions.into_source_actions(KeyCode::CAPS_LOCK) {
            source_actions.push(sa);
        }
        if let Some(sa) = left_shift_actions.into_source_actions(KeyCode::LEFT_SHIFT) {
            source_actions.push(sa);
        }
        if let Some(sa) = right_shift_actions.into_source_actions(KeyCode::RIGHT_SHIFT) {
            source_actions.push(sa);
        }

        // §6.4 서두 — 평가 순서를 RuleId 오름차순으로 고정한다(입력 순서 비의존).
        // `sort_by_key` 는 안정 정렬이라 같은 id 내부(예: WASD 4규칙)의 상대 순서는
        // 위에서 push 한 순서 그대로 유지된다.
        combos.sort_by_key(|r| r.id);

        PresetRules { combos, simple_remaps, source_actions }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::popups::{ArrowKeySet, BracketPair, HomeRowScheme, PasteTrigger, QuickPressCapsAction, RemapCapsTarget};
    use crate::settings::{CapsHjklArrowsSettings, CapsHomeRowSettings, CapsLockRemapSettings, CapsQuickPressSettings, PasteWithoutFormattingSettings, ShiftQuickPressBracketsSettings};

    #[test]
    fn all_disabled_yields_no_rules() {
        let rules = PresetSettings::default().to_rules(false);
        assert!(rules.combos.is_empty());
        assert!(rules.simple_remaps.is_empty());
        assert!(rules.source_actions.is_empty());
    }

    #[test]
    fn f08_1_remap_caps_lock_produces_hold_remap_source_action() {
        let settings = PresetSettings {
            caps_lock_remap: CapsLockRemapSettings { enabled: true, target: RemapCapsTarget::Esc },
            ..PresetSettings::default()
        };
        let rules = settings.to_rules(false);
        let sa = rules.source_actions.iter().find(|s| s.key == KeyCode::CAPS_LOCK).unwrap();
        assert_eq!(sa.hold_remap, Some(key_action(KeyCode::ESCAPE)));
        assert_eq!(sa.quick_press, None);
    }

    #[test]
    fn f08_1_nothing_target_maps_to_rule_action_nothing() {
        let settings = PresetSettings {
            caps_lock_remap: CapsLockRemapSettings { enabled: true, target: RemapCapsTarget::Nothing },
            ..PresetSettings::default()
        };
        let rules = settings.to_rules(false);
        let sa = rules.source_actions.iter().find(|s| s.key == KeyCode::CAPS_LOCK).unwrap();
        assert_eq!(sa.hold_remap, Some(RuleAction::Nothing));
    }

    #[test]
    fn f08_2_quick_press_caps_lock_merges_with_f08_1_hold_remap() {
        let settings = PresetSettings {
            caps_lock_remap: CapsLockRemapSettings { enabled: true, target: RemapCapsTarget::LeftControl },
            caps_quick_press: CapsQuickPressSettings { enabled: true, action: QuickPressCapsAction::Esc },
            ..PresetSettings::default()
        };
        let rules = settings.to_rules(false);
        // R1 — 같은 caps lock 물리 키의 두 규칙(hold_remap·quick_press)이 하나의
        // SourceKeyActions 로 합쳐져야 한다(중복 소스 키 등록을 만들지 않는다).
        assert_eq!(rules.source_actions.iter().filter(|s| s.key == KeyCode::CAPS_LOCK).count(), 1);
        let sa = rules.source_actions.iter().find(|s| s.key == KeyCode::CAPS_LOCK).unwrap();
        assert_eq!(sa.hold_remap, Some(key_action(KeyCode::LEFT_CONTROL)));
        assert_eq!(sa.quick_press, Some(key_action(KeyCode::ESCAPE)));
    }

    #[test]
    fn f08_4_caps_space_enter_produces_combo() {
        let settings = PresetSettings { caps_space_enter: true, ..PresetSettings::default() };
        let rules = settings.to_rules(false);
        assert_eq!(rules.combos.len(), 1);
        assert_eq!(rules.combos[0].trigger, KeyCode::SPACE);
        assert_eq!(rules.combos[0].hold, HoldCondition::Key(KeyCode::CAPS_LOCK));
        assert_eq!(rules.combos[0].action, key_action(KeyCode::RETURN));
    }

    #[test]
    fn f08_5_wasd_produces_4_combos_with_game_movement_mapping() {
        let settings = PresetSettings { caps_wasd_arrows: true, ..PresetSettings::default() };
        let rules = settings.to_rules(false);
        assert_eq!(rules.combos.len(), 4);
        let find = |trigger: KeyCode| rules.combos.iter().find(|c| c.trigger == trigger).unwrap();
        assert_eq!(find(KeyCode::ANSI_W).action, key_action(KeyCode::UP_ARROW));
        assert_eq!(find(KeyCode::ANSI_A).action, key_action(KeyCode::LEFT_ARROW));
        assert_eq!(find(KeyCode::ANSI_S).action, key_action(KeyCode::DOWN_ARROW));
        assert_eq!(find(KeyCode::ANSI_D).action, key_action(KeyCode::RIGHT_ARROW));
    }

    #[test]
    fn f08_6_hjkl_produces_vim_direction_mapping() {
        let settings = PresetSettings {
            caps_hjkl_arrows: CapsHjklArrowsSettings { enabled: true, key_set: ArrowKeySet::Hjkl },
            ..PresetSettings::default()
        };
        let rules = settings.to_rules(false);
        assert_eq!(rules.combos.len(), 4);
        let find = |trigger: KeyCode| rules.combos.iter().find(|c| c.trigger == trigger).unwrap();
        assert_eq!(find(KeyCode::ANSI_H).action, key_action(KeyCode::LEFT_ARROW));
        assert_eq!(find(KeyCode::ANSI_J).action, key_action(KeyCode::DOWN_ARROW));
        assert_eq!(find(KeyCode::ANSI_K).action, key_action(KeyCode::UP_ARROW));
        assert_eq!(find(KeyCode::ANSI_L).action, key_action(KeyCode::RIGHT_ARROW));
    }

    #[test]
    fn f08_7_home_row_symbol_scheme_produces_11_text_combos() {
        let settings = PresetSettings {
            caps_home_row: CapsHomeRowSettings { enabled: true, scheme: HomeRowScheme::SymbolRow },
            ..PresetSettings::default()
        };
        let rules = settings.to_rules(false);
        assert_eq!(rules.combos.len(), 11);
        let a = rules.combos.iter().find(|c| c.trigger == KeyCode::ANSI_A).unwrap();
        assert_eq!(a.action, RuleAction::Text('!'));
        let quote = rules.combos.iter().find(|c| c.trigger == KeyCode::ANSI_QUOTE).unwrap();
        assert_eq!(quote.action, RuleAction::Text('_'));
    }

    #[test]
    fn f08_7_home_row_function_scheme_produces_11_key_combos() {
        let settings = PresetSettings {
            caps_home_row: CapsHomeRowSettings { enabled: true, scheme: HomeRowScheme::FunctionRow },
            ..PresetSettings::default()
        };
        let rules = settings.to_rules(false);
        let a = rules.combos.iter().find(|c| c.trigger == KeyCode::ANSI_A).unwrap();
        assert_eq!(a.action, key_action(KeyCode::F1));
    }

    #[test]
    fn f08_8_double_tap_shift_applies_to_both_shifts() {
        let settings = PresetSettings { double_tap_shift_to_caps: true, ..PresetSettings::default() };
        let rules = settings.to_rules(false);
        let left = rules.source_actions.iter().find(|s| s.key == KeyCode::LEFT_SHIFT).unwrap();
        let right = rules.source_actions.iter().find(|s| s.key == KeyCode::RIGHT_SHIFT).unwrap();
        assert_eq!(left.double_tap, Some(RuleAction::ToggleCapsLock));
        assert_eq!(right.double_tap, Some(RuleAction::ToggleCapsLock));
    }

    #[test]
    fn f08_9_left_right_shift_produces_2_symmetric_combos() {
        let settings = PresetSettings { left_right_shift_to_caps: true, ..PresetSettings::default() };
        let rules = settings.to_rules(false);
        assert_eq!(rules.combos.len(), 2);
        assert!(rules.combos.iter().any(|c| c.trigger == KeyCode::LEFT_SHIFT && c.hold == HoldCondition::Key(KeyCode::RIGHT_SHIFT)));
        assert!(rules.combos.iter().any(|c| c.trigger == KeyCode::RIGHT_SHIFT && c.hold == HoldCondition::Key(KeyCode::LEFT_SHIFT)));
        assert!(rules.combos.iter().all(|c| c.action == RuleAction::ToggleCapsLock));
    }

    #[test]
    fn f08_10_shift_caps_to_caps_uses_either_shift_hold() {
        let settings = PresetSettings { shift_caps_to_caps: true, ..PresetSettings::default() };
        let rules = settings.to_rules(false);
        assert_eq!(rules.combos.len(), 1);
        assert_eq!(rules.combos[0].hold, HoldCondition::EitherShift);
        assert_eq!(rules.combos[0].trigger, KeyCode::CAPS_LOCK);
        assert_eq!(rules.combos[0].action, RuleAction::ToggleCapsLock);
    }

    #[test]
    fn f08_11_quick_press_brackets_splits_pair_across_shifts() {
        let settings = PresetSettings {
            shift_quick_press_brackets: ShiftQuickPressBracketsSettings { enabled: true, pair: BracketPair::Brackets },
            ..PresetSettings::default()
        };
        let rules = settings.to_rules(false);
        let left = rules.source_actions.iter().find(|s| s.key == KeyCode::LEFT_SHIFT).unwrap();
        let right = rules.source_actions.iter().find(|s| s.key == KeyCode::RIGHT_SHIFT).unwrap();
        assert_eq!(left.quick_press, Some(RuleAction::Text('[')));
        assert_eq!(right.quick_press, Some(RuleAction::Text(']')));
    }

    /// 같은 shift 키가 F-08.8(double tap) 과 F-08.11(quick press) 양쪽에 걸릴 수 있다 —
    /// 두 액션이 한 `SourceKeyActions` 로 합쳐져야 한다.
    #[test]
    fn f08_8_and_f08_11_merge_on_same_shift_key() {
        let settings = PresetSettings {
            double_tap_shift_to_caps: true,
            shift_quick_press_brackets: ShiftQuickPressBracketsSettings { enabled: true, pair: BracketPair::Parens },
            ..PresetSettings::default()
        };
        let rules = settings.to_rules(false);
        assert_eq!(rules.source_actions.iter().filter(|s| s.key == KeyCode::LEFT_SHIFT).count(), 1);
        let left = rules.source_actions.iter().find(|s| s.key == KeyCode::LEFT_SHIFT).unwrap();
        assert_eq!(left.double_tap, Some(RuleAction::ToggleCapsLock));
        assert_eq!(left.quick_press, Some(RuleAction::Text('(')));
    }

    #[test]
    fn f08_12_hyper_delete_uses_hyper_active_hold() {
        let settings = PresetSettings { hyper_delete_to_forward: true, ..PresetSettings::default() };
        let rules = settings.to_rules(false);
        assert_eq!(rules.combos.len(), 1);
        assert_eq!(rules.combos[0].hold, HoldCondition::HyperActive);
        assert_eq!(rules.combos[0].trigger, KeyCode::DELETE);
        assert_eq!(rules.combos[0].action, key_action(KeyCode::FORWARD_DELETE));
    }

    #[test]
    fn f08_13_delete_to_forward_is_simple_remap() {
        let settings = PresetSettings { delete_to_forward: true, ..PresetSettings::default() };
        let rules = settings.to_rules(false);
        assert_eq!(rules.simple_remaps.len(), 1);
        assert_eq!(rules.simple_remaps[0].from, KeyCode::DELETE);
        assert_eq!(rules.simple_remaps[0].to, KeyCode::FORWARD_DELETE);
    }

    #[test]
    fn f08_14_shift_delete_uses_either_shift_hold() {
        let settings = PresetSettings { shift_delete_to_forward: true, ..PresetSettings::default() };
        let rules = settings.to_rules(false);
        assert_eq!(rules.combos.len(), 1);
        assert_eq!(rules.combos[0].hold, HoldCondition::EitherShift);
        assert_eq!(rules.combos[0].trigger, KeyCode::DELETE);
    }

    /// R6/엣지10 — forward delete 3종 동시 활성은 배타적이지 않다(사실로만 문서화).
    #[test]
    fn r6_forward_delete_trio_coexist_without_error() {
        let settings = PresetSettings {
            hyper_delete_to_forward: true,
            delete_to_forward: true,
            shift_delete_to_forward: true,
            ..PresetSettings::default()
        };
        let rules = settings.to_rules(false);
        assert_eq!(rules.combos.len(), 2); // hyper + shift
        assert_eq!(rules.simple_remaps.len(), 1); // 단순 리매핑
    }

    #[test]
    fn f08_15_paste_trigger_maps_to_hold_condition() {
        let cases = [
            (PasteTrigger::RightCommand, HoldCondition::Key(KeyCode::RIGHT_COMMAND)),
            (PasteTrigger::LeftCommand, HoldCondition::Key(KeyCode::LEFT_COMMAND)),
            (PasteTrigger::EitherCommand, HoldCondition::EitherCommand),
            (PasteTrigger::HyperKey, HoldCondition::HyperActive),
        ];
        for (trigger, expected_hold) in cases {
            let settings = PresetSettings {
                paste_without_formatting: PasteWithoutFormattingSettings { enabled: true, trigger },
                ..PresetSettings::default()
            };
            let rules = settings.to_rules(false);
            assert_eq!(rules.combos[0].hold, expected_hold, "trigger={trigger:?}");
            assert_eq!(rules.combos[0].trigger, KeyCode::ANSI_V);
            assert_eq!(
                rules.combos[0].action,
                RuleAction::Key { keycode: KeyCode::ANSI_V, flags: EventFlags::COMMAND | EventFlags::ALTERNATE | EventFlags::SHIFT }
            );
        }
    }

    #[test]
    fn f08_16_home_end_produces_two_simple_remaps_with_command_flag() {
        let settings = PresetSettings { home_end_on_lines: true, ..PresetSettings::default() };
        let rules = settings.to_rules(false);
        assert_eq!(rules.simple_remaps.len(), 2);
        let home = rules.simple_remaps.iter().find(|r| r.from == KeyCode::HOME).unwrap();
        assert_eq!(home.to, KeyCode::LEFT_ARROW);
        assert_eq!(home.add_flags, EventFlags::COMMAND);
        let end = rules.simple_remaps.iter().find(|r| r.from == KeyCode::END).unwrap();
        assert_eq!(end.to, KeyCode::RIGHT_ARROW);
        assert_eq!(end.add_flags, EventFlags::COMMAND);
    }

    #[test]
    fn combos_are_sorted_by_rule_id_ascending() {
        let settings = PresetSettings {
            home_end_on_lines: true, // 계층4, id 아님(simple_remaps)
            hyper_delete_to_forward: true, // Preset(12)
            caps_space_enter: true, // Preset(4)
            shift_delete_to_forward: true, // Preset(14)
            ..PresetSettings::default()
        };
        let rules = settings.to_rules(false);
        let ids: Vec<u8> = rules
            .combos
            .iter()
            .map(|c| match c.id {
                RuleId::Preset(n) => n,
                RuleId::Hyperkey => u8::MAX,
            })
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "combos 가 RuleId 오름차순이어야 한다: {ids:?}");
    }

    #[test]
    fn needs_caps_lock_alias_true_for_each_of_the_seven_presets() {
        let base = PresetSettings::default();
        assert!(!base.needs_caps_lock_alias(false));
        assert!(base.needs_caps_lock_alias(true), "hyper 소스가 caps lock 이면 참");

        assert!(PresetSettings { caps_lock_remap: CapsLockRemapSettings { enabled: true, ..Default::default() }, ..base }.needs_caps_lock_alias(false));
        assert!(PresetSettings { caps_quick_press: CapsQuickPressSettings { enabled: true, ..Default::default() }, ..base }.needs_caps_lock_alias(false));
        assert!(PresetSettings { caps_space_enter: true, ..base }.needs_caps_lock_alias(false));
        assert!(PresetSettings { caps_wasd_arrows: true, ..base }.needs_caps_lock_alias(false));
        assert!(PresetSettings { caps_hjkl_arrows: CapsHjklArrowsSettings { enabled: true, ..Default::default() }, ..base }.needs_caps_lock_alias(false));
        assert!(PresetSettings { caps_home_row: CapsHomeRowSettings { enabled: true, ..Default::default() }, ..base }.needs_caps_lock_alias(false));
        assert!(PresetSettings { shift_caps_to_caps: true, ..base }.needs_caps_lock_alias(false));
    }

    /// F-08.4(caps lock+space)는 어느 그룹과도 충돌하지 않는다는 것과는 별개로, 이 값도
    /// alias 를 필요로 한다는 점만 확인(이미 위에서 검증됨). double tap/left-right shift
    /// 는 alias 를 요구하지 않는다(caps lock 을 입력으로 읽지 않는다).
    #[test]
    fn double_tap_and_left_right_shift_do_not_need_caps_lock_alias() {
        let base = PresetSettings::default();
        assert!(!PresetSettings { double_tap_shift_to_caps: true, ..base }.needs_caps_lock_alias(false));
        assert!(!PresetSettings { left_right_shift_to_caps: true, ..base }.needs_caps_lock_alias(false));
    }

    // ── F-08 → 엔진 규칙 실 설정 통합 테스트 — 이슈 #19 회귀 방지 ────────────────────────
    //
    // ⭐ 이것이 가장 값진 테스트다 — 사용자가 실제로 켜는 `PresetSettings` 에서 출발해
    // `to_rules` → `EngineConfig` → `Arbiter` 까지 전체 파이프라인을 통과시킨다. 이 크레이트
    // (`ultrakey-presets`)는 `ultrakey-core` 를 이미 일반 의존성으로 갖고 있으므로(Cargo.toml)
    // 별도 dev-dependency 없이 바로 쓴다.
    use ultrakey_core::arbitration::{Arbiter, Disposition};
    use ultrakey_core::event::{EventKind, InputEvent};
    use ultrakey_core::settings::EngineConfig;
    use ultrakey_core::time::Millis;

    /// `Remap caps lock to: left control`(F-08.1) — D-1 이 켜지는 실 설정에서 출발해
    /// F18 의 `KeyDown` 이 `left control` 의 `FlagsChanged` 로 바르게 옮겨지는지 끝까지
    /// 확인한다.
    #[test]
    fn caps_lock_remap_to_left_control_end_to_end_under_d1() {
        let settings = PresetSettings {
            caps_lock_remap: CapsLockRemapSettings { enabled: true, target: RemapCapsTarget::LeftControl },
            ..PresetSettings::default()
        };
        assert!(settings.needs_caps_lock_alias(false), "D-1 이 켜지는 구성이어야 한다");

        let preset_rules = settings.to_rules(false);
        let mut cfg = EngineConfig::default();
        cfg.rules.combo_rules = preset_rules.combos;
        cfg.rules.simple_remaps = preset_rules.simple_remaps;
        cfg.rules.source_actions = preset_rules.source_actions;
        // D-1 이 실제로 설치했을 경로 B 매핑을 재현 — 탭에는 F18 의 KeyDown/KeyUp 이 도착한다.
        cfg.caps_lock_alias = Some(KeyCode::F18);

        let mut arb = Arbiter::new(&cfg);
        let out = arb.arbitrate(
            &cfg,
            &InputEvent { kind: EventKind::KeyDown, keycode: KeyCode::F18, flags: EventFlags::NONE, autorepeat: false },
            false,
            Millis(0),
        );

        assert_eq!(out.emitted().len(), 1);
        assert_eq!(out.emitted()[0].kind, EventKind::FlagsChanged);
        assert_eq!(out.emitted()[0].keycode, KeyCode::LEFT_CONTROL);
        // 기대값의 출처는 macOS 헤더다: kCGEventFlagMaskControl = 0x00040000
        // (`CGEventTypes.h`), NX_DEVICELCTLKEYMASK = 0x00000001 (`IOKit/hidsystem/IOLLEvent.h`).
        const LEFT_CONTROL_HELD: u64 = 0x0004_0001;
        assert_eq!(out.emitted()[0].flags.0 & LEFT_CONTROL_HELD, LEFT_CONTROL_HELD);
    }

    /// `Double tap shift = caps lock`(F-08.8) — architecture.md §6.1 의 D-1 목록에
    /// 없으므로 alias 를 요구하지 않고, shift 는 `Arbiter::has_substitute_output` 판정에
    /// 따라 원본 그대로 통과한다(`Disposition::Pass`) — 이 프리셋이 shift 를 죽이지
    /// 않는다는 것을 실 설정에서 끝까지 확인한다.
    #[test]
    fn double_tap_shift_preset_keeps_shift_working_end_to_end() {
        let settings = PresetSettings { double_tap_shift_to_caps: true, ..PresetSettings::default() };
        assert!(!settings.needs_caps_lock_alias(false), "F-08.8 은 D-1 을 켜지 않는다(architecture.md §6.1)");

        let preset_rules = settings.to_rules(false);
        let mut cfg = EngineConfig::default();
        cfg.rules.combo_rules = preset_rules.combos;
        cfg.rules.simple_remaps = preset_rules.simple_remaps;
        cfg.rules.source_actions = preset_rules.source_actions;
        // alias 는 기본값 None 그대로 둔다 — D-1 이 꺼져 있다.

        let mut arb = Arbiter::new(&cfg);

        let shift_down = arb.arbitrate(
            &cfg,
            &InputEvent {
                kind: EventKind::FlagsChanged,
                keycode: KeyCode::LEFT_SHIFT,
                flags: EventFlags::SHIFT,
                autorepeat: false,
            },
            false,
            Millis(0),
        );
        assert_eq!(
            shift_down.disposition(),
            Disposition::Pass,
            "shift 가 통째로 삼켜진다 — `Double tap shift = caps lock` 을 켜면 Shift+a 가 소문자로 나간다"
        );

        let a_down = arb.arbitrate(
            &cfg,
            &InputEvent { kind: EventKind::KeyDown, keycode: KeyCode::ANSI_A, flags: EventFlags::NONE, autorepeat: false },
            false,
            Millis(10),
        );
        assert_eq!(
            a_down.disposition(),
            Disposition::Pass,
            "shift 가 통째로 삼켜진다 — `Double tap shift = caps lock` 을 켜면 Shift+a 가 소문자로 나간다"
        );
    }
}
