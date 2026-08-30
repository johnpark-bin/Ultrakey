//! `Presets` 탭의 팝업 선택지 열거형(`docs/spec/preferences-ui.md` §4.3 "팝업 선택지
//! 전량" 이 정본). SuperKey 는 임의 커스텀 리매핑을 제공하지 않는다 — 선택지는 전부
//! 사전 정의된 고정 목록이며, 이 파일이 그 고정 목록을 코드로 못박는다
//! (`docs/spec/power-user-presets.md` §1 제품 철학).
//!
//! ⭐ 저장은 `serde` derive 기본값(variant 이름)을 그대로 쓴다 — `ultrakey-hyperkey::SourceKey`
//! 와 같은 관례: UI 라벨(`label()`)을 고치더라도 저장된 설정이 깨지지 않는다.

use serde::{Deserialize, Serialize};

use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::KeyCode;
use ultrakey_core::rules::RuleAction;

/// `Remap caps lock to:`(F-08.1) 팝업 — 50종, 표시 순서 그대로
/// (`docs/spec/power-user-presets.md` §3.2 F-08.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RemapCapsTarget {
    Esc,
    /// `nothing (disable it)` — caps lock 을 완전히 무효화한다(`RuleAction::Nothing`).
    Nothing,
    LeftControl,
    LeftShift,
    LeftOption,
    LeftCommand,
    RightControl,
    RightShift,
    RightOption,
    RightCommand,
    Return,
    Delete,
    DeleteForward,
    Tab,
    Spacebar,
    Home,
    End,
    PageUp,
    PageDown,
    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,
    Mute,
    VolumeUp,
    VolumeDown,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    /// ⚠️ F21~F24 는 표준 `kVK_*` 상수가 없다(`ultrakey_core::keycode` 상단 주석 참고).
    /// `keycode()`/`action()` 이 이 넷에 대해 `None` 을 반환한다 — 지어낸 값을 넣지 않는다.
    F21,
    F22,
    F23,
    F24,
}

impl RemapCapsTarget {
    /// 50종 전량, 팝업 표시 순서 그대로.
    pub fn all() -> &'static [RemapCapsTarget] {
        use RemapCapsTarget::*;
        &[
            Esc, Nothing, LeftControl, LeftShift, LeftOption, LeftCommand, RightControl, RightShift,
            RightOption, RightCommand, Return, Delete, DeleteForward, Tab, Spacebar, Home, End, PageUp,
            PageDown, LeftArrow, RightArrow, UpArrow, DownArrow, Mute, VolumeUp, VolumeDown, F1, F2, F3,
            F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20, F21, F22, F23,
            F24,
        ]
    }

    /// SuperKey 원문 표기 그대로의 라벨.
    pub fn label(self) -> &'static str {
        use RemapCapsTarget::*;
        match self {
            Esc => "esc",
            Nothing => "nothing (disable it)",
            LeftControl => "left control",
            LeftShift => "left shift",
            LeftOption => "left option",
            LeftCommand => "left command",
            RightControl => "right control",
            RightShift => "right shift",
            RightOption => "right option",
            RightCommand => "right command",
            Return => "return (enter)",
            Delete => "delete (backspace)",
            DeleteForward => "delete forward",
            Tab => "tab",
            Spacebar => "spacebar",
            Home => "home",
            End => "end",
            PageUp => "pageup",
            PageDown => "pagedown",
            LeftArrow => "left arrow",
            RightArrow => "right arrow",
            UpArrow => "up arrow",
            DownArrow => "down arrow",
            Mute => "mute",
            VolumeUp => "volume up",
            VolumeDown => "volume down",
            F1 => "F1",
            F2 => "F2",
            F3 => "F3",
            F4 => "F4",
            F5 => "F5",
            F6 => "F6",
            F7 => "F7",
            F8 => "F8",
            F9 => "F9",
            F10 => "F10",
            F11 => "F11",
            F12 => "F12",
            F13 => "F13",
            F14 => "F14",
            F15 => "F15",
            F16 => "F16",
            F17 => "F17",
            F18 => "F18",
            F19 => "F19",
            F20 => "F20",
            F21 => "F21",
            F22 => "F22",
            F23 => "F23",
            F24 => "F24",
        }
    }

    /// 대상 keycode. `Nothing`(키가 아니라 "무효화" 의미)과 `F21`~`F24`(표준 상수 없음)는
    /// `None`.
    pub fn keycode(self) -> Option<KeyCode> {
        use RemapCapsTarget::*;
        match self {
            Esc => Some(KeyCode::ESCAPE),
            Nothing => None,
            LeftControl => Some(KeyCode::LEFT_CONTROL),
            LeftShift => Some(KeyCode::LEFT_SHIFT),
            LeftOption => Some(KeyCode::LEFT_OPTION),
            LeftCommand => Some(KeyCode::LEFT_COMMAND),
            RightControl => Some(KeyCode::RIGHT_CONTROL),
            RightShift => Some(KeyCode::RIGHT_SHIFT),
            RightOption => Some(KeyCode::RIGHT_OPTION),
            RightCommand => Some(KeyCode::RIGHT_COMMAND),
            Return => Some(KeyCode::RETURN),
            Delete => Some(KeyCode::DELETE),
            DeleteForward => Some(KeyCode::FORWARD_DELETE),
            Tab => Some(KeyCode::TAB),
            Spacebar => Some(KeyCode::SPACE),
            Home => Some(KeyCode::HOME),
            End => Some(KeyCode::END),
            PageUp => Some(KeyCode::PAGE_UP),
            PageDown => Some(KeyCode::PAGE_DOWN),
            LeftArrow => Some(KeyCode::LEFT_ARROW),
            RightArrow => Some(KeyCode::RIGHT_ARROW),
            UpArrow => Some(KeyCode::UP_ARROW),
            DownArrow => Some(KeyCode::DOWN_ARROW),
            Mute => Some(KeyCode::MUTE),
            VolumeUp => Some(KeyCode::VOLUME_UP),
            VolumeDown => Some(KeyCode::VOLUME_DOWN),
            F1 => Some(KeyCode::F1),
            F2 => Some(KeyCode::F2),
            F3 => Some(KeyCode::F3),
            F4 => Some(KeyCode::F4),
            F5 => Some(KeyCode::F5),
            F6 => Some(KeyCode::F6),
            F7 => Some(KeyCode::F7),
            F8 => Some(KeyCode::F8),
            F9 => Some(KeyCode::F9),
            F10 => Some(KeyCode::F10),
            F11 => Some(KeyCode::F11),
            F12 => Some(KeyCode::F12),
            F13 => Some(KeyCode::F13),
            F14 => Some(KeyCode::F14),
            F15 => Some(KeyCode::F15),
            F16 => Some(KeyCode::F16),
            F17 => Some(KeyCode::F17),
            F18 => Some(KeyCode::F18),
            F19 => Some(KeyCode::F19),
            F20 => Some(KeyCode::F20),
            // (미확정) — F21~F24 는 표준 kVK_* 상수가 없다. 지어낸 값을 넣지 않는다.
            F21 | F22 | F23 | F24 => None,
        }
    }

    /// 엔진 규칙으로 번역한 결과. `Nothing` 은 `Some(RuleAction::Nothing)`(소비만 하고
    /// 아무것도 내지 않음)이고, `F21`~`F24` 는 `keycode()` 가 없어 규칙을 만들 수 없으므로
    /// `None` — `SourceKey` 가 이미 쓰는 관례를 그대로 따른다. `None` 은 UI 가 경고를 낼
    /// 신호이지, 조용히 `Nothing` 취급해서는 안 된다(둘의 의미가 다르다).
    pub fn action(self) -> Option<RuleAction> {
        if matches!(self, RemapCapsTarget::Nothing) {
            return Some(RuleAction::Nothing);
        }
        self.keycode()
            .map(|keycode| RuleAction::Key { keycode, flags: EventFlags::NONE })
    }
}

/// `Quick press caps lock to execute:`(F-08.2) 팝업.
///
/// ⭐ **개수 정정.** `power-user-presets.md`·`preferences-ui.md`·`app-bundle-analysis.md`
/// 세 문서 모두 "49종 전량 확인"이라 적고 있으나, 그 직후 나열한 표시 순서 목록을 그대로
/// 세면(`Seek` 1 + `esc`~`volume down` 26 + `F1`~`F20` 20 + `/` 1) **48개**다 — 문서
/// 자신이 적은 목록과 자신이 주장한 개수가 어긋난다. 지어낸 49번째 항목을 넣을 근거가
/// 없으므로(원문에 없는 값을 만들어내지 않는다는 규약), 이 구현은 **목록을 그대로 따라
/// 48개**로 둔다. 목록 자체(순서·항목)는 세 문서가 동일하게 반복해 신뢰도가 높다 —
/// 어긋나는 것은 총계 숫자 쪽이므로, 그 숫자가 오기라고 본다. 이 판단과 근거는
/// 구현 보고에도 남긴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum QuickPressCapsAction {
    Seek,
    Esc,
    CapsLock,
    LeftControl,
    LeftShift,
    LeftOption,
    LeftCommand,
    RightControl,
    RightShift,
    RightOption,
    RightCommand,
    Return,
    Delete,
    DeleteForward,
    Tab,
    Spacebar,
    Home,
    End,
    PageUp,
    PageDown,
    LeftArrow,
    RightArrow,
    UpArrow,
    DownArrow,
    Mute,
    VolumeUp,
    VolumeDown,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    /// v1.62 에서 추가된 `/`.
    Slash,
}

impl QuickPressCapsAction {
    /// 전량(48개, 위 문서 주석 참고), 팝업 표시 순서 그대로.
    pub fn all() -> &'static [QuickPressCapsAction] {
        use QuickPressCapsAction::*;
        &[
            Seek, Esc, CapsLock, LeftControl, LeftShift, LeftOption, LeftCommand, RightControl,
            RightShift, RightOption, RightCommand, Return, Delete, DeleteForward, Tab, Spacebar, Home,
            End, PageUp, PageDown, LeftArrow, RightArrow, UpArrow, DownArrow, Mute, VolumeUp, VolumeDown,
            F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12, F13, F14, F15, F16, F17, F18, F19, F20,
            Slash,
        ]
    }

    pub fn label(self) -> &'static str {
        use QuickPressCapsAction::*;
        match self {
            Seek => "Seek",
            Esc => "esc",
            CapsLock => "caps lock",
            LeftControl => "left control",
            LeftShift => "left shift",
            LeftOption => "left option",
            LeftCommand => "left command",
            RightControl => "right control",
            RightShift => "right shift",
            RightOption => "right option",
            RightCommand => "right command",
            Return => "return (enter)",
            Delete => "delete (backspace)",
            DeleteForward => "delete forward",
            Tab => "tab",
            Spacebar => "spacebar",
            Home => "home",
            End => "end",
            PageUp => "pageup",
            PageDown => "pagedown",
            LeftArrow => "left arrow",
            RightArrow => "right arrow",
            UpArrow => "up arrow",
            DownArrow => "down arrow",
            Mute => "mute",
            VolumeUp => "volume up",
            VolumeDown => "volume down",
            F1 => "F1",
            F2 => "F2",
            F3 => "F3",
            F4 => "F4",
            F5 => "F5",
            F6 => "F6",
            F7 => "F7",
            F8 => "F8",
            F9 => "F9",
            F10 => "F10",
            F11 => "F11",
            F12 => "F12",
            F13 => "F13",
            F14 => "F14",
            F15 => "F15",
            F16 => "F16",
            F17 => "F17",
            F18 => "F18",
            F19 => "F19",
            F20 => "F20",
            Slash => "/",
        }
    }

    /// 엔진 규칙으로 번역한 결과. 이 팝업의 모든 항목은 유효한 keycode 를 갖거나
    /// (F1~F20, `RemapCapsTarget` 과 달리 F21~F24 가 없다) 전용 효과(`Seek`·`caps lock`)
    /// 로 번역되므로 `Option` 이 아니라 항상 값을 낸다.
    pub fn action(self) -> RuleAction {
        use QuickPressCapsAction::*;
        match self {
            Seek => RuleAction::OpenSeek,
            CapsLock => RuleAction::ToggleCapsLock,
            Slash => RuleAction::Text('/'),
            Esc => key(KeyCode::ESCAPE),
            LeftControl => key(KeyCode::LEFT_CONTROL),
            LeftShift => key(KeyCode::LEFT_SHIFT),
            LeftOption => key(KeyCode::LEFT_OPTION),
            LeftCommand => key(KeyCode::LEFT_COMMAND),
            RightControl => key(KeyCode::RIGHT_CONTROL),
            RightShift => key(KeyCode::RIGHT_SHIFT),
            RightOption => key(KeyCode::RIGHT_OPTION),
            RightCommand => key(KeyCode::RIGHT_COMMAND),
            Return => key(KeyCode::RETURN),
            Delete => key(KeyCode::DELETE),
            DeleteForward => key(KeyCode::FORWARD_DELETE),
            Tab => key(KeyCode::TAB),
            Spacebar => key(KeyCode::SPACE),
            Home => key(KeyCode::HOME),
            End => key(KeyCode::END),
            PageUp => key(KeyCode::PAGE_UP),
            PageDown => key(KeyCode::PAGE_DOWN),
            LeftArrow => key(KeyCode::LEFT_ARROW),
            RightArrow => key(KeyCode::RIGHT_ARROW),
            UpArrow => key(KeyCode::UP_ARROW),
            DownArrow => key(KeyCode::DOWN_ARROW),
            Mute => key(KeyCode::MUTE),
            VolumeUp => key(KeyCode::VOLUME_UP),
            VolumeDown => key(KeyCode::VOLUME_DOWN),
            F1 => key(KeyCode::F1),
            F2 => key(KeyCode::F2),
            F3 => key(KeyCode::F3),
            F4 => key(KeyCode::F4),
            F5 => key(KeyCode::F5),
            F6 => key(KeyCode::F6),
            F7 => key(KeyCode::F7),
            F8 => key(KeyCode::F8),
            F9 => key(KeyCode::F9),
            F10 => key(KeyCode::F10),
            F11 => key(KeyCode::F11),
            F12 => key(KeyCode::F12),
            F13 => key(KeyCode::F13),
            F14 => key(KeyCode::F14),
            F15 => key(KeyCode::F15),
            F16 => key(KeyCode::F16),
            F17 => key(KeyCode::F17),
            F18 => key(KeyCode::F18),
            F19 => key(KeyCode::F19),
            F20 => key(KeyCode::F20),
        }
    }
}

fn key(keycode: KeyCode) -> RuleAction {
    RuleAction::Key { keycode, flags: EventFlags::NONE }
}

/// `Caps lock +` [팝업] ` = ◀▼▲▶`(F-08.6) — 방향키 트리거 키셋 2종.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArrowKeySet {
    #[serde(rename = "HJKL")]
    Hjkl,
    #[serde(rename = "IJKL")]
    Ijkl,
}

impl ArrowKeySet {
    pub fn all() -> &'static [ArrowKeySet] {
        &[ArrowKeySet::Hjkl, ArrowKeySet::Ijkl]
    }

    pub fn label(self) -> &'static str {
        match self {
            ArrowKeySet::Hjkl => "H J K L",
            ArrowKeySet::Ijkl => "I J K L",
        }
    }

    /// (◀, ▼, ▲, ▶) 순서로 트리거 키코드 4개 — vim 방향 관례.
    pub fn keys(self) -> (KeyCode, KeyCode, KeyCode, KeyCode) {
        match self {
            ArrowKeySet::Hjkl => (KeyCode::ANSI_H, KeyCode::ANSI_J, KeyCode::ANSI_K, KeyCode::ANSI_L),
            ArrowKeySet::Ijkl => (KeyCode::ANSI_I, KeyCode::ANSI_J, KeyCode::ANSI_K, KeyCode::ANSI_L),
        }
    }
}

/// `Caps lock + home row = ` [팝업](F-08.7) — 홈로우 매핑 스킴 2종.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HomeRowScheme {
    SymbolRow,
    FunctionRow,
}

impl HomeRowScheme {
    pub fn all() -> &'static [HomeRowScheme] {
        &[HomeRowScheme::SymbolRow, HomeRowScheme::FunctionRow]
    }

    pub fn label(self) -> &'static str {
        match self {
            HomeRowScheme::SymbolRow => "symbol row (A = !)",
            HomeRowScheme::FunctionRow => "function row (A = F1)",
        }
    }
}

/// `Quick press left or right shift to input corresponding:`(F-08.11) 팝업 — 문자 쌍 4종.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BracketPair {
    Parens,
    Brackets,
    Braces,
    Angles,
}

impl BracketPair {
    pub fn all() -> &'static [BracketPair] {
        &[BracketPair::Parens, BracketPair::Brackets, BracketPair::Braces, BracketPair::Angles]
    }

    pub fn label(self) -> &'static str {
        match self {
            BracketPair::Parens => "( )",
            BracketPair::Brackets => "[ ]",
            BracketPair::Braces => "{ }",
            BracketPair::Angles => "< >",
        }
    }

    /// (좌 shift 출력 문자, 우 shift 출력 문자).
    pub fn pair(self) -> (char, char) {
        match self {
            BracketPair::Parens => ('(', ')'),
            BracketPair::Brackets => ('[', ']'),
            BracketPair::Braces => ('{', '}'),
            BracketPair::Angles => ('<', '>'),
        }
    }
}

/// `Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):`(F-08.15) 팝업 — 트리거 4종.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PasteTrigger {
    RightCommand,
    LeftCommand,
    EitherCommand,
    HyperKey,
}

impl PasteTrigger {
    pub fn all() -> &'static [PasteTrigger] {
        &[PasteTrigger::RightCommand, PasteTrigger::LeftCommand, PasteTrigger::EitherCommand, PasteTrigger::HyperKey]
    }

    pub fn label(self) -> &'static str {
        match self {
            PasteTrigger::RightCommand => "Right ⌘",
            PasteTrigger::LeftCommand => "Left ⌘",
            PasteTrigger::EitherCommand => "Either ⌘",
            PasteTrigger::HyperKey => "Hyper key",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remap_caps_target_has_50_variants_in_order() {
        assert_eq!(RemapCapsTarget::all().len(), 50);
        assert_eq!(RemapCapsTarget::all()[0], RemapCapsTarget::Esc);
        assert_eq!(RemapCapsTarget::all()[1], RemapCapsTarget::Nothing);
        assert_eq!(RemapCapsTarget::all()[49], RemapCapsTarget::F24);
    }

    #[test]
    fn remap_caps_target_f21_to_f24_have_no_keycode_and_action() {
        for f in [RemapCapsTarget::F21, RemapCapsTarget::F22, RemapCapsTarget::F23, RemapCapsTarget::F24] {
            assert_eq!(f.keycode(), None);
            assert_eq!(f.action(), None, "F21~F24 는 action() 도 None 이어야 UI 가 경고할 수 있다");
        }
    }

    #[test]
    fn remap_caps_target_nothing_maps_to_rule_action_nothing() {
        assert_eq!(RemapCapsTarget::Nothing.action(), Some(RuleAction::Nothing));
        assert_eq!(RemapCapsTarget::Nothing.keycode(), None);
    }

    #[test]
    fn remap_caps_target_normal_key_maps_to_rule_action_key() {
        assert_eq!(
            RemapCapsTarget::LeftControl.action(),
            Some(RuleAction::Key { keycode: KeyCode::LEFT_CONTROL, flags: EventFlags::NONE })
        );
    }

    #[test]
    fn remap_caps_target_labels_preserve_original_text() {
        assert_eq!(RemapCapsTarget::Nothing.label(), "nothing (disable it)");
        assert_eq!(RemapCapsTarget::Return.label(), "return (enter)");
    }

    /// ⭐ 문서 상의 "49종" 이 실제로는 48개다(위 타입 문서 주석 참고) — 목록을 그대로
    /// 따르고 총계는 목록에서 나온 실제 값을 쓴다.
    #[test]
    fn quick_press_caps_action_has_48_variants_in_order() {
        assert_eq!(QuickPressCapsAction::all().len(), 48);
        assert_eq!(QuickPressCapsAction::all()[0], QuickPressCapsAction::Seek);
        assert_eq!(QuickPressCapsAction::all()[1], QuickPressCapsAction::Esc);
        assert_eq!(QuickPressCapsAction::all()[47], QuickPressCapsAction::Slash);
    }

    #[test]
    fn quick_press_caps_action_special_cases() {
        assert_eq!(QuickPressCapsAction::Seek.action(), RuleAction::OpenSeek);
        assert_eq!(QuickPressCapsAction::CapsLock.action(), RuleAction::ToggleCapsLock);
        assert_eq!(QuickPressCapsAction::Slash.action(), RuleAction::Text('/'));
    }

    #[test]
    fn quick_press_caps_action_key_case() {
        assert_eq!(
            QuickPressCapsAction::F1.action(),
            RuleAction::Key { keycode: KeyCode::F1, flags: EventFlags::NONE }
        );
    }

    #[test]
    fn arrow_key_set_has_2_variants() {
        assert_eq!(ArrowKeySet::all().len(), 2);
        assert_eq!(ArrowKeySet::Hjkl.label(), "H J K L");
        assert_eq!(
            ArrowKeySet::Hjkl.keys(),
            (KeyCode::ANSI_H, KeyCode::ANSI_J, KeyCode::ANSI_K, KeyCode::ANSI_L)
        );
        assert_eq!(
            ArrowKeySet::Ijkl.keys(),
            (KeyCode::ANSI_I, KeyCode::ANSI_J, KeyCode::ANSI_K, KeyCode::ANSI_L)
        );
    }

    #[test]
    fn home_row_scheme_has_2_variants() {
        assert_eq!(HomeRowScheme::all().len(), 2);
        assert_eq!(HomeRowScheme::SymbolRow.label(), "symbol row (A = !)");
        assert_eq!(HomeRowScheme::FunctionRow.label(), "function row (A = F1)");
    }

    #[test]
    fn bracket_pair_has_4_variants() {
        assert_eq!(BracketPair::all().len(), 4);
        assert_eq!(BracketPair::Parens.pair(), ('(', ')'));
        assert_eq!(BracketPair::Angles.pair(), ('<', '>'));
    }

    #[test]
    fn paste_trigger_has_4_variants() {
        assert_eq!(PasteTrigger::all().len(), 4);
        assert_eq!(PasteTrigger::HyperKey.label(), "Hyper key");
    }

    #[test]
    fn serde_uses_stable_variant_names_not_ui_labels() {
        let json = serde_json::to_string(&RemapCapsTarget::Nothing).unwrap();
        assert!(json.contains("Nothing"));
        assert!(!json.contains("disable"));
    }
}
