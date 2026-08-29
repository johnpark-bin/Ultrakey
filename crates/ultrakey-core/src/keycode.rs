//! 물리 키코드(virtual keycode)와, hyper/meh/bleh·Seek 등이 소스 키로 고를 수 있는
//! 35종 키 목록(`hyperkey.md` §4).
//!
//! ⭐ 판정은 반드시 **물리 키코드** 기준이어야 한다(`key-remapping-engine.md` §8 수용 기준:
//! "문자 기반으로 소스 키를 식별하는 코드 경로가 없다"). 이 모듈이 그 유일한 판정 단위를 정의한다.

use serde::{Deserialize, Serialize};

/// macOS virtual keycode 하나(`CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode)` 의 값).
///
/// `Carbon/HIToolbox/Events.h` 의 `kVK_*` 상수와 1:1 대응하는 연관 상수를 제공한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct KeyCode(pub u16);

impl KeyCode {
    pub const CAPS_LOCK: KeyCode = KeyCode(0x39);
    pub const LEFT_SHIFT: KeyCode = KeyCode(0x38);
    pub const RIGHT_SHIFT: KeyCode = KeyCode(0x3C);
    pub const LEFT_CONTROL: KeyCode = KeyCode(0x3B);
    pub const RIGHT_CONTROL: KeyCode = KeyCode(0x3E);
    pub const LEFT_OPTION: KeyCode = KeyCode(0x3A);
    pub const RIGHT_OPTION: KeyCode = KeyCode(0x3D);
    pub const LEFT_COMMAND: KeyCode = KeyCode(0x37);
    pub const RIGHT_COMMAND: KeyCode = KeyCode(0x36);
    /// globe/fn 키(`kVK_Function`).
    pub const FUNCTION: KeyCode = KeyCode(0x3F);
    pub const ANSI_A: KeyCode = KeyCode(0x00);
    pub const ANSI_S: KeyCode = KeyCode(0x01);
    pub const ANSI_D: KeyCode = KeyCode(0x02);
    pub const ANSI_W: KeyCode = KeyCode(0x0D);
    pub const ANSI_V: KeyCode = KeyCode(0x09);
    pub const ANSI_SEMICOLON: KeyCode = KeyCode(0x29);
    pub const F1: KeyCode = KeyCode(0x7A);
    pub const F2: KeyCode = KeyCode(0x78);
    pub const F3: KeyCode = KeyCode(0x63);
    pub const F4: KeyCode = KeyCode(0x76);
    pub const F5: KeyCode = KeyCode(0x60);
    pub const F6: KeyCode = KeyCode(0x61);
    pub const F7: KeyCode = KeyCode(0x62);
    pub const F8: KeyCode = KeyCode(0x64);
    pub const F9: KeyCode = KeyCode(0x65);
    pub const F10: KeyCode = KeyCode(0x6D);
    pub const F11: KeyCode = KeyCode(0x67);
    pub const F12: KeyCode = KeyCode(0x6F);
    pub const F13: KeyCode = KeyCode(0x69);
    pub const F14: KeyCode = KeyCode(0x6B);
    pub const F15: KeyCode = KeyCode(0x71);
    pub const F16: KeyCode = KeyCode(0x6A);
    pub const F17: KeyCode = KeyCode(0x40);
    pub const F18: KeyCode = KeyCode(0x4F);
    pub const F19: KeyCode = KeyCode(0x50);
    pub const F20: KeyCode = KeyCode(0x5A);
    // ⚠️ F21~F24 는 표준 `kVK_*` 상수가 존재하지 않는다. 값을 지어내지 않는다 —
    // `SourceKey::keycode()` 가 이 네 항목에 대해 `None` 을 반환하는 것으로 표현한다.
    pub const UP_ARROW: KeyCode = KeyCode(0x7E);
    pub const DOWN_ARROW: KeyCode = KeyCode(0x7D);
    pub const LEFT_ARROW: KeyCode = KeyCode(0x7B);
    pub const RIGHT_ARROW: KeyCode = KeyCode(0x7C);
    pub const DELETE: KeyCode = KeyCode(0x33);
    pub const FORWARD_DELETE: KeyCode = KeyCode(0x75);
}

/// hyper/meh/bleh·Seek 소스 키 팝업에 실제로 나열되는 35종(`hyperkey.md` §4, 표시 순서 그대로).
///
/// ⭐ `Serialize`/`Deserialize` 는 **variant 이름**으로 직렬화된다(`"CapsLock"`). 설정 파일
/// 형식을 `label()`(UI 표시 문자열)에 결합하면 라벨을 고치는 순간 저장된 설정이 깨지므로,
/// 저장 표현은 UI 와 분리한다 — F-15("부재 = 기본값") 저장 모델의 전제이기도 하다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceKey {
    CapsLock,
    RightOption,
    RightShift,
    RightCommand,
    RightControl,
    LeftOption,
    LeftShift,
    LeftCommand,
    LeftControl,
    Globe,
    MenuPc,
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
    F21,
    F22,
    F23,
    F24,
}

impl SourceKey {
    /// 대응하는 물리 keycode. F21~F24 와 `menu (PC)` 는 표준 `kVK_*` 상수가 확인되지 않아
    /// `None` 을 반환한다 — 값을 추측해서 채우지 않는다(§7 판정, `keycode.rs` 상단 주석 참고).
    pub fn keycode(self) -> Option<KeyCode> {
        use SourceKey::*;
        match self {
            CapsLock => Some(KeyCode::CAPS_LOCK),
            RightOption => Some(KeyCode::RIGHT_OPTION),
            RightShift => Some(KeyCode::RIGHT_SHIFT),
            RightCommand => Some(KeyCode::RIGHT_COMMAND),
            RightControl => Some(KeyCode::RIGHT_CONTROL),
            LeftOption => Some(KeyCode::LEFT_OPTION),
            LeftShift => Some(KeyCode::LEFT_SHIFT),
            LeftCommand => Some(KeyCode::LEFT_COMMAND),
            LeftControl => Some(KeyCode::LEFT_CONTROL),
            Globe => Some(KeyCode::FUNCTION),
            // (미확정) — PC 키보드의 `menu` 키에 대응하는 표준 kVK_* 상수가 없다.
            MenuPc => None,
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

    /// 35종 전량, 팝업 표시 순서 그대로(`hyperkey.md` §4 실측).
    pub fn all() -> &'static [SourceKey] {
        use SourceKey::*;
        &[
            CapsLock,
            RightOption,
            RightShift,
            RightCommand,
            RightControl,
            LeftOption,
            LeftShift,
            LeftCommand,
            LeftControl,
            Globe,
            MenuPc,
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
            F21,
            F22,
            F23,
            F24,
        ]
    }

    /// SuperKey 원문 표기 그대로의 라벨(로컬라이즈하지 않는다 — 원문 고유명사 유지 규약).
    pub fn label(self) -> &'static str {
        use SourceKey::*;
        match self {
            CapsLock => "caps lock",
            RightOption => "right option",
            RightShift => "right shift",
            RightCommand => "right command",
            RightControl => "right control",
            LeftOption => "left option",
            LeftShift => "left shift",
            LeftCommand => "left command",
            LeftControl => "left control",
            Globe => "globe",
            MenuPc => "menu (PC)",
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_returns_35_source_keys_in_spec_order() {
        assert_eq!(SourceKey::all().len(), 35);
        assert_eq!(SourceKey::all()[0], SourceKey::CapsLock);
        assert_eq!(SourceKey::all()[9], SourceKey::Globe);
        assert_eq!(SourceKey::all()[10], SourceKey::MenuPc);
    }

    #[test]
    fn f21_to_f24_and_menu_pc_have_no_keycode() {
        assert_eq!(SourceKey::F21.keycode(), None);
        assert_eq!(SourceKey::F22.keycode(), None);
        assert_eq!(SourceKey::F23.keycode(), None);
        assert_eq!(SourceKey::F24.keycode(), None);
        assert_eq!(SourceKey::MenuPc.keycode(), None);
    }

    #[test]
    fn caps_lock_keycode_matches_kvk_capslock() {
        assert_eq!(SourceKey::CapsLock.keycode(), Some(KeyCode(0x39)));
    }

    #[test]
    fn label_preserves_original_text() {
        assert_eq!(SourceKey::CapsLock.label(), "caps lock");
        assert_eq!(SourceKey::MenuPc.label(), "menu (PC)");
        assert_eq!(SourceKey::Globe.label(), "globe");
    }
}
