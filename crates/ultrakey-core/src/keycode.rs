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
    pub const ANSI_F: KeyCode = KeyCode(0x03);
    pub const ANSI_H: KeyCode = KeyCode(0x04);
    pub const ANSI_G: KeyCode = KeyCode(0x05);
    pub const ANSI_W: KeyCode = KeyCode(0x0D);
    pub const ANSI_V: KeyCode = KeyCode(0x09);
    pub const ANSI_J: KeyCode = KeyCode(0x26);
    pub const ANSI_K: KeyCode = KeyCode(0x28);
    pub const ANSI_L: KeyCode = KeyCode(0x25);
    pub const ANSI_I: KeyCode = KeyCode(0x22);
    pub const ANSI_SEMICOLON: KeyCode = KeyCode(0x29);
    pub const ANSI_QUOTE: KeyCode = KeyCode(0x27);
    pub const ANSI_SLASH: KeyCode = KeyCode(0x2C);
    pub const SPACE: KeyCode = KeyCode(0x31);
    pub const RETURN: KeyCode = KeyCode(0x24);
    pub const TAB: KeyCode = KeyCode(0x30);
    pub const ESCAPE: KeyCode = KeyCode(0x35);
    pub const HOME: KeyCode = KeyCode(0x73);
    pub const END: KeyCode = KeyCode(0x77);
    pub const PAGE_UP: KeyCode = KeyCode(0x74);
    pub const PAGE_DOWN: KeyCode = KeyCode(0x79);
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
    /// `kVK_Mute`.
    pub const MUTE: KeyCode = KeyCode(0x4A);
    /// `kVK_VolumeUp`.
    pub const VOLUME_UP: KeyCode = KeyCode(0x48);
    /// `kVK_VolumeDown`.
    pub const VOLUME_DOWN: KeyCode = KeyCode(0x49);

    /// ⭐ 이 키가 **modifier 키**이면, 그것이 눌려 있는 동안 이벤트에 실려야 하는
    /// `CGEventFlags` 비트(일반 마스크 + 좌/우 구분 비트). modifier 가 아니면 `None`.
    ///
    /// **왜 필요한가 (2026-08-30, 이슈 #19 / 증상 A).** macOS 는 modifier 키의
    /// 눌림/뗌을 `kCGEventKeyDown`/`KeyUp` 이 **아니라** `kCGEventFlagsChanged` 와
    /// 그 이벤트의 flags 비트로만 표현한다 — `key-remapping-engine.md` §5 #18 이
    /// 입력 쪽에서 이미 확정한 사실이다. 그런데 **출력 쪽**은 그 사실을 반영하지
    /// 않고 있었다: `Remap caps lock to: left control`(F-08.1)이 `left control` 의
    /// `KeyDown`(flags 0)을 합성해 내보내면, 받는 앱은 control 비트가 켜진 적이
    /// 없으므로 **control 이 눌린 것으로 보지 않는다.** 이 함수는 그 대칭을 회복하는
    /// 자리다 — 대상이 modifier 면 여기서 얻은 비트를 실은 `FlagsChanged` 로 낸다.
    ///
    /// ⚠️ **caps lock 은 `None` 이다.** caps lock 의 `alphaShift` 비트는 키의 눌림이
    /// 아니라 **잠금(lock) 상태**를 뜻한다(§5 #18 (b)). 그것을 "눌림" 표현으로 쓰면
    /// 잠금이 켜진 것처럼 보이게 되어, `Arbiter::strip_caps_lock_bit` 이 이슈 #13 에서
    /// 고친 결함을 되살린다. caps lock 을 **대상 키로** 고른 리매핑은 잠금 토글이
    /// 목적이므로 경로 C 가 다룬다(`key-remapping-engine.md` §3-d).
    pub fn modifier_flags(self) -> Option<crate::flags::EventFlags> {
        use crate::flags::EventFlags as F;
        let f = match self {
            KeyCode::LEFT_SHIFT => F(F::SHIFT.0 | F::DEVICE_LEFT_SHIFT.0),
            KeyCode::RIGHT_SHIFT => F(F::SHIFT.0 | F::DEVICE_RIGHT_SHIFT.0),
            KeyCode::LEFT_CONTROL => F(F::CONTROL.0 | F::DEVICE_LEFT_CONTROL.0),
            KeyCode::RIGHT_CONTROL => F(F::CONTROL.0 | F::DEVICE_RIGHT_CONTROL.0),
            KeyCode::LEFT_OPTION => F(F::ALTERNATE.0 | F::DEVICE_LEFT_OPTION.0),
            KeyCode::RIGHT_OPTION => F(F::ALTERNATE.0 | F::DEVICE_RIGHT_OPTION.0),
            KeyCode::LEFT_COMMAND => F(F::COMMAND.0 | F::DEVICE_LEFT_COMMAND.0),
            KeyCode::RIGHT_COMMAND => F(F::COMMAND.0 | F::DEVICE_RIGHT_COMMAND.0),
            KeyCode::FUNCTION => F::SECONDARY_FN,
            _ => return None,
        };
        Some(f)
    }

    /// 이 키가 macOS 가 `flagsChanged` 로만 전달하는 modifier 키인가
    /// — `modifier_flags()` 가 값을 내는 키 **와 caps lock**. caps lock 은 flags 비트를
    /// 갖지 않지만(위 참고) 이벤트 종류만은 `flagsChanged` 다.
    pub fn is_modifier_key(self) -> bool {
        self == KeyCode::CAPS_LOCK || self.modifier_flags().is_some()
    }
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

    // ── modifier_flags() — 이슈 #19 증상 A ────────────────────────────────────────
    //
    // ⭐ **기대값을 구현이 쓰는 상수에서 가져오지 않는다.** PR #17 이 지적한 함정이
    // 그것이었다("테스트가 코드와 같은 상수를 기대값으로 써서 무엇이 옳은가가 아니라
    // 코드가 무엇을 하는가를 검증했다"). 여기서는 **macOS 헤더의 값을 리터럴로 직접**
    // 적는다 — 출처는 아래 주석에 남긴다.
    //
    //   `CoreGraphics/CGEventTypes.h`
    //     kCGEventFlagMaskShift      = 0x00020000
    //     kCGEventFlagMaskControl    = 0x00040000
    //     kCGEventFlagMaskAlternate  = 0x00080000
    //     kCGEventFlagMaskCommand    = 0x00100000
    //     kCGEventFlagMaskSecondaryFn= 0x00800000
    //   `IOKit/hidsystem/IOLLEvent.h`
    //     NX_DEVICELCTLKEYMASK   = 0x00000001   NX_DEVICERCTLKEYMASK   = 0x00002000
    //     NX_DEVICELSHIFTKEYMASK = 0x00000002   NX_DEVICERSHIFTKEYMASK = 0x00000004
    //     NX_DEVICELCMDKEYMASK   = 0x00000008   NX_DEVICERCMDKEYMASK   = 0x00000010
    //     NX_DEVICELALTKEYMASK   = 0x00000020   NX_DEVICERALTKEYMASK   = 0x00000040
    #[test]
    fn modifier_flags_match_macos_header_values() {
        let expect: &[(KeyCode, u64)] = &[
            (KeyCode::LEFT_SHIFT, 0x0002_0002),
            (KeyCode::RIGHT_SHIFT, 0x0002_0004),
            (KeyCode::LEFT_CONTROL, 0x0004_0001),
            (KeyCode::RIGHT_CONTROL, 0x0004_2000),
            (KeyCode::LEFT_OPTION, 0x0008_0020),
            (KeyCode::RIGHT_OPTION, 0x0008_0040),
            (KeyCode::LEFT_COMMAND, 0x0010_0008),
            (KeyCode::RIGHT_COMMAND, 0x0010_0010),
            (KeyCode::FUNCTION, 0x0080_0000),
        ];
        for (k, want) in expect {
            assert_eq!(
                k.modifier_flags().map(|f| f.0),
                Some(*want),
                "keycode {:#04X} 의 modifier flags 가 macOS 헤더 값과 다르다",
                k.0
            );
        }
    }

    /// ⚠️ caps lock 은 `None` 이다 — `alphaShift` 비트는 눌림이 아니라 잠금을 뜻한다
    /// (`key-remapping-engine.md` §5 #18 (b)). 그것을 눌림 표현으로 쓰면 이슈 #13 에서
    /// 고친 "다른 앱이 caps lock 이 켜진 것으로 인식" 결함이 되살아난다.
    #[test]
    fn caps_lock_has_no_press_flags_but_is_still_a_modifier_key() {
        assert_eq!(KeyCode::CAPS_LOCK.modifier_flags(), None);
        assert!(KeyCode::CAPS_LOCK.is_modifier_key());
    }

    /// modifier 가 아닌 키는 `None` — 과잉 교정 방지(esc·tab·방향키 리매핑은
    /// 종전대로 `KeyDown`/`KeyUp` 으로 나가야 한다).
    #[test]
    fn non_modifier_keys_have_no_modifier_flags() {
        for k in [
            KeyCode::ESCAPE,
            KeyCode::TAB,
            KeyCode::SPACE,
            KeyCode::RETURN,
            KeyCode::LEFT_ARROW,
            KeyCode::F18,
            KeyCode::ANSI_A,
        ] {
            assert_eq!(k.modifier_flags(), None, "keycode {:#04X}", k.0);
            assert!(!k.is_modifier_key(), "keycode {:#04X}", k.0);
        }
    }
}
