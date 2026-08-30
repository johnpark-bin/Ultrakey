//! `Effect::TypeChar` 를 실제 합성 키 이벤트로 옮기는 방법 —
//! `docs/spec/localization-and-input-sources.md` §3.2.2.
//!
//! ⭐ 이 저장소가 세 번째로(PR #23 포함) 겪은 결함 계열은 "입력 쪽에 있는 대칭이
//! 출력 쪽에 없다"는 것이었다. `ultrakey-layout::LayoutTable::keycode_for_char` 는
//! §3.2.2 (i) 역방향 탐색을 이미 구현하고 테스트까지 갖췄지만, 그것을 §3.2.2 가
//! 정한 우선순위(먼저 (i), 실패 시 (ii) 폴백)로 이어받는 호출자가 없었다 — 그
//! 결과 F-08.11 quick press 괄호가 (ii) `CGEventKeyboardSetUnicodeString` 로만
//! 나갔다. 이 모듈이 그 호출자다.

use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::KeyCode;
use ultrakey_layout::LayoutTable;

/// `Effect::TypeChar` 를 실제 이벤트로 옮기는 방법 — §3.2.2 의 (i)/(ii) 두 경로.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextOutputPlan {
    /// (i) `UCKeyTranslate` 역방향 탐색 성공 — 현재 레이아웃에서 그 문자를 내는
    /// 진짜 물리 키 + modifier 로 keyDown/keyUp 을 합성한다.
    KeyStroke { keycode: KeyCode, flags: EventFlags },
    /// (ii) 폴백 — keycode 0 짜리 이벤트에 `CGEventKeyboardSetUnicodeString` 로
    /// 문자열만 얹는다. 현재 레이아웃에 그 문자를 낼 수 있는 조합이 없을 때(또는
    /// 레이아웃 테이블이 아직 구축되지 않아 비어 있을 때) 쓴다.
    UnicodeString(char),
}

/// §3.2.2 채택 원칙 그대로: `table.keycode_for_char(c)` 가 있으면 (i), 없으면
/// (ii) 로 떨어진다. `LayoutTable::is_empty()` 인 경우(아직 레이아웃 테이블이
/// 구축되지 않음)도 `keycode_for_char` 가 항상 `None` 을 돌려주므로 자연히
/// `UnicodeString` 로 떨어진다 — 별도 분기가 필요 없다.
pub fn plan_text_output(table: &LayoutTable, c: char) -> TextOutputPlan {
    match table.keycode_for_char(c) {
        Some((keycode, combo)) => TextOutputPlan::KeyStroke {
            keycode,
            flags: combo.event_flags(),
        },
        None => TextOutputPlan::UnicodeString(c),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultrakey_layout::{ModifierCombo, Translator};

    /// US-QWERTY 일부만 흉내 낸 가짜 번역기 — `ultrakey-layout` 의
    /// `#[cfg(test)]` `FakeQwerty` 와 같은 조합을 재현한다(같은 크레이트 밖에서는
    /// 그 비공개 테스트 타입을 재사용할 수 없어 이 모듈 안에 다시 정의한다 —
    /// `keycode_for_char` 의 공개 계약만 검증하면 되므로 표가 완전히 같을 필요는
    /// 없고, "숫자 줄 + Shift → 괄호" 관계만 재현하면 충분하다).
    struct FakeQwerty;

    impl Translator for FakeQwerty {
        fn translate(
            &self,
            _uchr: &[u8],
            _keyboard_type: u32,
            keycode: KeyCode,
            modifiers: u32,
        ) -> Option<String> {
            let shift = modifiers == ModifierCombo::Shift.uc_key_modifiers();
            let none = modifiers == ModifierCombo::None.uc_key_modifiers();
            match keycode {
                // kVK_ANSI_9 = 0x19, kVK_ANSI_0 = 0x1D.
                KeyCode(0x19) if none => Some("9".into()),
                KeyCode(0x19) if shift => Some("(".into()),
                KeyCode(0x1D) if none => Some("0".into()),
                KeyCode(0x1D) if shift => Some(")".into()),
                _ => None,
            }
        }
    }

    /// ⭐ AZERTY 흉내 — 숫자 줄이 Shift 없이 기호를 내는 반전 배치(§3.2.4).
    /// `(` 가 QWERTY 와 다른 keycode/조합에서 나오는지를 이 모듈 수준에서도
    /// 다시 확인한다(레이아웃 독립성, R5/v1.51).
    struct FakeAzerty;

    impl Translator for FakeAzerty {
        fn translate(
            &self,
            _uchr: &[u8],
            _keyboard_type: u32,
            keycode: KeyCode,
            modifiers: u32,
        ) -> Option<String> {
            let none = modifiers == ModifierCombo::None.uc_key_modifiers();
            match keycode {
                // 물리 "5" 키(kVK_ANSI_5 = 0x17): unshifted 가 "(".
                KeyCode(0x17) if none => Some("(".into()),
                _ => None,
            }
        }
    }

    /// 아무것도 번역하지 못하는 가짜 번역기 — 폴백 경로 확인용.
    struct FakeEmpty;

    impl Translator for FakeEmpty {
        fn translate(
            &self,
            _uchr: &[u8],
            _keyboard_type: u32,
            _keycode: KeyCode,
            _modifiers: u32,
        ) -> Option<String> {
            None
        }
    }

    fn fake_snapshot(source_id: &str) -> ultrakey_platform::text_input_source::LayoutSnapshot {
        ultrakey_platform::text_input_source::LayoutSnapshot {
            source_id: source_id.to_string(),
            is_ascii_capable: true,
            used_ascii_fallback: false,
            keyboard_type: 0,
            uchr_data: Vec::new(),
            original_source_id: String::new(),
            original_languages: Vec::new(),
        }
    }

    #[test]
    fn qwerty_open_paren_plans_a_keystroke() {
        let table = LayoutTable::build(&fake_snapshot("qwerty"), &FakeQwerty);
        assert_eq!(
            plan_text_output(&table, '('),
            TextOutputPlan::KeyStroke {
                keycode: KeyCode(0x19),
                flags: EventFlags::SHIFT,
            }
        );
        assert_eq!(
            plan_text_output(&table, ')'),
            TextOutputPlan::KeyStroke {
                keycode: KeyCode(0x1D),
                flags: EventFlags::SHIFT,
            }
        );
    }

    /// ⭐ 레이아웃 독립성(R5/v1.51) — 같은 목표 문자 `(` 가 QWERTY 와 AZERTY 에서
    /// 서로 다른 keycode 로 계획된다.
    #[test]
    fn open_paren_plan_differs_by_layout() {
        let qwerty = LayoutTable::build(&fake_snapshot("qwerty"), &FakeQwerty);
        let azerty = LayoutTable::build(&fake_snapshot("azerty"), &FakeAzerty);

        let qwerty_plan = plan_text_output(&qwerty, '(');
        let azerty_plan = plan_text_output(&azerty, '(');
        assert_ne!(qwerty_plan, azerty_plan);

        assert_eq!(
            azerty_plan,
            TextOutputPlan::KeyStroke {
                keycode: KeyCode(0x17),
                flags: EventFlags::NONE,
            }
        );
    }

    /// (ii) 폴백 — 레이아웃에 없는 문자는 UnicodeString 으로 떨어진다.
    #[test]
    fn char_not_producible_by_layout_falls_back_to_unicode_string() {
        let table = LayoutTable::build(&fake_snapshot("qwerty"), &FakeQwerty);
        assert_eq!(plan_text_output(&table, '€'), TextOutputPlan::UnicodeString('€'));
    }

    /// 빈 표(`LayoutTable::empty()`) — 항상 UnicodeString.
    #[test]
    fn empty_table_always_falls_back_to_unicode_string() {
        let empty = LayoutTable::empty();
        assert_eq!(plan_text_output(&empty, '('), TextOutputPlan::UnicodeString('('));

        // 아무것도 번역 못 하는 번역기로 빌드해도 결과적으로 빈 표와 같다.
        let built_empty = LayoutTable::build(&fake_snapshot("empty"), &FakeEmpty);
        assert!(built_empty.is_empty());
        assert_eq!(
            plan_text_output(&built_empty, '('),
            TextOutputPlan::UnicodeString('(')
        );
    }

    // ── ⭐ 끝단(end-to-end) 재현 — F-08.11 quick press shift → 괄호 ────────────
    //
    // 이 저장소는 "테스트가 실제 이벤트 모양을 안 써서" 이미 세 번 데었다(M1
    // hyper 전면 미발동, PR #23 modifier 출력, 그리고 이번 이슈 #32). 실기기
    // 진단 로그(`ULTRAKEY_TRACE_TAP=1`)가 확정한 실제 입력 모양은
    // `FlagsChanged` 뿐이다 — `KeyDown`/`KeyUp` 으로 구성한 테스트는 이 계열의
    // 결함을 재현하지 못한다. 그래서 여기서도 `FlagsChanged` 로만 구성한다.
    use ultrakey_core::arbitration::{Arbiter, Effect, GateSnapshot};
    use ultrakey_core::event::{EventKind, InputEvent};
    use ultrakey_core::flags::EventFlags as CoreEventFlags;
    use ultrakey_core::rules::{RuleAction, SourceKeyActions};
    use ultrakey_core::settings::EngineConfig;
    use ultrakey_core::time::Millis;

    fn flags_changed(keycode: KeyCode, flags: CoreEventFlags) -> InputEvent {
        InputEvent {
            kind: EventKind::FlagsChanged,
            keycode,
            flags,
            autorepeat: false,
        }
    }

    /// quick press shift → 괄호 설정을 그대로 흉내 낸 `EngineConfig`.
    fn shift_bracket_config() -> EngineConfig {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::LEFT_SHIFT,
            quick_press: Some(RuleAction::Text('(')),
            double_tap: None,
            hold_remap: None,
        });
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::RIGHT_SHIFT,
            quick_press: Some(RuleAction::Text(')')),
            double_tap: None,
            hold_remap: None,
        });
        cfg
    }

    /// 왼쪽 shift 를 quick press(누름→ 지속시간 안에 뗌)하면 `Arbiter` 가
    /// `Effect::TypeChar('(')` 를 내고, 그 문자를 QWERTY 가짜 표로 계획하면
    /// (i) 경로(진짜 keystroke)가 나온다.
    #[test]
    fn left_shift_quick_press_plans_a_real_keystroke_on_qwerty() {
        let cfg = shift_bracket_config();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::LEFT_SHIFT, CoreEventFlags::SHIFT),
            GateSnapshot::default(),
            Millis(0),
        );
        let up = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::LEFT_SHIFT, CoreEventFlags::SHIFT),
            GateSnapshot::default(),
            Millis(50),
        );
        assert_eq!(up.effects(), &[Effect::TypeChar('(')]);

        let Effect::TypeChar(c) = up.effects()[0] else {
            unreachable!()
        };
        let table = LayoutTable::build(&fake_snapshot("qwerty"), &FakeQwerty);
        assert_eq!(
            plan_text_output(&table, c),
            TextOutputPlan::KeyStroke {
                keycode: KeyCode(0x19),
                flags: EventFlags::SHIFT,
            }
        );
    }

    /// 우 shift 쪽도 대칭으로 확인한다 — `)`.
    #[test]
    fn right_shift_quick_press_plans_a_real_keystroke_on_qwerty() {
        let cfg = shift_bracket_config();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::RIGHT_SHIFT, CoreEventFlags::SHIFT),
            GateSnapshot::default(),
            Millis(0),
        );
        let up = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::RIGHT_SHIFT, CoreEventFlags::SHIFT),
            GateSnapshot::default(),
            Millis(50),
        );
        assert_eq!(up.effects(), &[Effect::TypeChar(')')]);

        let Effect::TypeChar(c) = up.effects()[0] else {
            unreachable!()
        };
        let table = LayoutTable::build(&fake_snapshot("qwerty"), &FakeQwerty);
        assert_eq!(
            plan_text_output(&table, c),
            TextOutputPlan::KeyStroke {
                keycode: KeyCode(0x1D),
                flags: EventFlags::SHIFT,
            }
        );
    }
}
