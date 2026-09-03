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

    // ── F-01 — `KeyCode::from_web_code` 가 필요로 하는 나머지 문자·숫자·기호 키 ──
    //
    // ⭐ 값은 전부 이 세션이 로컬 SDK 헤더에서 직접 읽었다(위임 지시서가 요구한
    // 그대로): `grep -n "kVK_" /Library/Developer/CommandLineTools/SDKs/MacOSX15.4.sdk/
    // System/Library/Frameworks/Carbon.framework/Versions/A/Frameworks/HIToolbox.framework/
    // Versions/A/Headers/Events.h`. 각 상수 주석에 그 헤더의 `kVK_*` 이름을 남긴다.
    pub const ANSI_B: KeyCode = KeyCode(0x0B);
    pub const ANSI_C: KeyCode = KeyCode(0x08);
    pub const ANSI_E: KeyCode = KeyCode(0x0E);
    pub const ANSI_M: KeyCode = KeyCode(0x2E);
    pub const ANSI_N: KeyCode = KeyCode(0x2D);
    pub const ANSI_O: KeyCode = KeyCode(0x1F);
    pub const ANSI_P: KeyCode = KeyCode(0x23);
    pub const ANSI_Q: KeyCode = KeyCode(0x0C);
    pub const ANSI_R: KeyCode = KeyCode(0x0F);
    pub const ANSI_T: KeyCode = KeyCode(0x11);
    pub const ANSI_U: KeyCode = KeyCode(0x20);
    pub const ANSI_X: KeyCode = KeyCode(0x07);
    pub const ANSI_Y: KeyCode = KeyCode(0x10);
    pub const ANSI_Z: KeyCode = KeyCode(0x06);
    /// `kVK_ANSI_0`.
    pub const ANSI_0: KeyCode = KeyCode(0x1D);
    /// `kVK_ANSI_1`.
    pub const ANSI_1: KeyCode = KeyCode(0x12);
    /// `kVK_ANSI_2`.
    pub const ANSI_2: KeyCode = KeyCode(0x13);
    /// `kVK_ANSI_3`.
    pub const ANSI_3: KeyCode = KeyCode(0x14);
    /// `kVK_ANSI_4`.
    pub const ANSI_4: KeyCode = KeyCode(0x15);
    /// `kVK_ANSI_5`.
    pub const ANSI_5: KeyCode = KeyCode(0x17);
    /// `kVK_ANSI_6`.
    pub const ANSI_6: KeyCode = KeyCode(0x16);
    /// `kVK_ANSI_7`.
    pub const ANSI_7: KeyCode = KeyCode(0x1A);
    /// `kVK_ANSI_8`.
    pub const ANSI_8: KeyCode = KeyCode(0x1C);
    /// `kVK_ANSI_9`.
    pub const ANSI_9: KeyCode = KeyCode(0x19);
    /// `kVK_ANSI_Minus`.
    pub const ANSI_MINUS: KeyCode = KeyCode(0x1B);
    /// `kVK_ANSI_Equal`.
    pub const ANSI_EQUAL: KeyCode = KeyCode(0x18);
    /// `kVK_ANSI_LeftBracket`.
    pub const ANSI_LEFT_BRACKET: KeyCode = KeyCode(0x21);
    /// `kVK_ANSI_RightBracket`.
    pub const ANSI_RIGHT_BRACKET: KeyCode = KeyCode(0x1E);
    /// `kVK_ANSI_Backslash`.
    pub const ANSI_BACKSLASH: KeyCode = KeyCode(0x2A);
    /// `kVK_ANSI_Comma`.
    pub const ANSI_COMMA: KeyCode = KeyCode(0x2B);
    /// `kVK_ANSI_Period`.
    pub const ANSI_PERIOD: KeyCode = KeyCode(0x2F);

    pub const SPACE: KeyCode = KeyCode(0x31);
    /// `` ` ``/`₩` 물리 키(`kVK_ANSI_Grave`). F-16.4(`docs/spec/korean-input.md` §3.1)의
    /// 트리거 키다. 근거: `Carbon/HIToolbox/Events.h` 243행 `kVK_ANSI_Grave = 0x32` —
    /// 이 세션이 로컬 SDK 헤더에서 직접 확인했다.
    pub const ANSI_GRAVE: KeyCode = KeyCode(0x32);
    /// 한/영 키(HangulMode, Karabiner `lang1`). F-16.2(`docs/spec/korean-input.md` §3.2,
    /// D-K12)의 트리거 키다.
    ///
    /// ⚠️ **이름이 뒤집혀 보이는 이유.** 이 상수의 이름은 `kVK_JIS_Kana` 다 — "가나"라는
    /// 이름과 한/영 전환이라는 기능이 정반대로 보인다. macOS 는 한국어 전용 `kVK_*`
    /// 상수를 두지 않는다: 한/영 키는 JIS(일본어) 배열의 `かな` 키와 **같은 물리
    /// 위치**를 쓰므로, macOS 가 그 물리 위치의 JIS 상수를 그대로 재사용한다. 즉
    /// 이름은 물리 키 위치를 가리키지, 이 상수로 눌리는 논리적 기능을 가리키지 않는다.
    ///
    /// ⚠️ **JIS 물리 키보드도 같은 keycode 를 낸다**(명세 §5 #13). JIS 배열 키보드의
    /// `かな` 키를 누르면 이 상수와 똑같은 `0x68` 이 도착한다 — 이 값만으로는 한국어
    /// 106키의 한/영 키와 JIS 키보드의 `かな` 키를 구분할 수 없다. 두 항목의 기본값이
    /// ☐ 이므로 사용자가 명시적으로 켜야만 일어나는 부작용으로 허용한다.
    ///
    /// **근거 3중(전부 일치, 명세 §3.2)**:
    /// 1. W3C `uievents-key` 이슈 #55 — `Lang1(HID 0x90) → Mac kVK_JIS_Kana`.
    ///    https://github.com/w3c/uievents-key/issues/55
    /// 2. ⭐ 실측 — 이 세션이 로컬 SDK 헤더를 직접 확인했다:
    ///    `…/MacOSX.sdk/…/HIToolbox.framework/Headers/Events.h` 328행 `kVK_JIS_Kana = 0x68`.
    /// 3. Chromium `ui/events/keycodes/keyboard_code_conversion_mac.mm` —
    ///    `{kVK_JIS_Kana, DomKey::KANJI_MODE}`.
    ///
    /// 등급: `(웹 조사 확정 + SDK 헤더 실측, 실기기 미검증)` — 한국어 106키 물리
    /// 키보드로 실제 눌러 본 것은 아니다(검증 기기에 그 키보드가 없다).
    pub const JIS_KANA: KeyCode = KeyCode(0x68);
    /// 한자 키(Hanja, Karabiner `lang2`). F-16.3(`docs/spec/korean-input.md` §3.2,
    /// D-K12)의 트리거 키다.
    ///
    /// ⚠️ **이름이 뒤집혀 보이는 이유**는 [`KeyCode::JIS_KANA`] 와 같다 — 한자 키는
    /// JIS 배열의 `英数`(에이스/영숫자) 키와 같은 물리 위치를 쓰므로, macOS 가 그
    /// 물리 위치의 `kVK_JIS_Eisu` 상수를 재사용한다.
    ///
    /// ⚠️ **JIS 물리 키보드도 같은 keycode 를 낸다**(명세 §5 #13) — [`KeyCode::JIS_KANA`]
    /// 의 같은 주의사항이 그대로 적용된다.
    ///
    /// **근거 3중(전부 일치, 명세 §3.2)**:
    /// 1. W3C `uievents-key` 이슈 #55 — `Lang2(HID 0x91) → Mac kVK_JIS_Eisu`.
    /// 2. ⭐ 실측 — `Events.h` 327행 `kVK_JIS_Eisu = 0x66`(이 세션이 직접 확인).
    /// 3. Chromium — `{kVK_JIS_Eisu, DomKey::EISU}`.
    ///
    /// 등급: [`KeyCode::JIS_KANA`] 와 동일 — `(웹 조사 확정 + SDK 헤더 실측, 실기기 미검증)`.
    pub const JIS_EISU: KeyCode = KeyCode(0x66);
    /// JIS 키보드의 `¥` 키(`kVK_JIS_Yen`, 로컬 SDK `Events.h` 324행 실측).
    /// F-19.5(¥↔\)·F-19.6(행 10/11)의 트리거·출력 키다(`docs/spec/language-presets.md`
    /// §3.2). ⚠️ Karabiner 카탈로그의 `international3` 이름은 **HID usage** 를 가리키므로
    /// macOS virtual keycode 인 이 상수와 혼동하지 않는다 — JIS 키보드에서 우리 탭에
    /// 도착하는 값은 `0x5D` 다.
    pub const JIS_YEN: KeyCode = KeyCode(0x5D);
    /// JIS 키보드의 `_` 키(`kVK_JIS_Underscore`, 로컬 SDK `Events.h` 325행 실측).
    /// F-19.6 행 7 의 출력 키(`shift+-` → `_`)다. Karabiner 의 `international1` 과
    /// 같은 물리 키지만 HID usage 이름이므로 위와 같은 주의가 적용된다.
    pub const JIS_UNDERSCORE: KeyCode = KeyCode(0x5E);
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

/// 웹 `KeyboardEvent.code`(ANSI 기준 물리 키 위치 문자열) → macOS 물리 키코드.
///
/// F-01(`seek-activation-and-session.md`)의 전역 단축키 레코더가 설정 창(브라우저
/// 웹뷰) 안에서 누른 키를 물리 키코드로 옮기는 유일한 지점이다 — 이 저장소는 키코드의
/// 진실을 이 파일 하나에 두는 관례를 지킨다(모듈 문서 참고). JS 쪽에는 이 표를
/// 두지 않는다.
///
/// ⭐ 값은 전부 로컬 SDK 헤더에서 직접 읽었다(`korean-input.md` §3.2 가 이미 쓴 방법과
/// 동일): `grep -n "kVK_" …/HIToolbox.framework/Versions/A/Headers/Events.h`. 그 헤더에
/// 없는 키(F21~F24 등)는 이 표에 넣지 않는다 — 이 파일 상단 규칙(값을 지어내지
/// 않는다) 그대로다. 덮는 범위는 위임 지시서가 명시한 것: 문자 `KeyA`~`KeyZ`, 숫자
/// `Digit0`~`Digit9`, `Space`·`Enter`·`Tab`·`Escape`·`Backspace`·`Delete`, 화살표 4종,
/// `Minus`·`Equal`·`BracketLeft`·`BracketRight`·`Backslash`·`Semicolon`·`Quote`·`Comma`·
/// `Period`·`Slash`·`Backquote`, `F1`~`F20`, `Home`·`End`·`PageUp`·`PageDown`.
///
/// ⚠️ `Backspace`(웹)는 macOS 물리 "delete"(뒤로 지움) 키를 가리키므로
/// [`KeyCode::DELETE`](0x33)로, 웹 `Delete`(포워드 삭제)는 [`KeyCode::FORWARD_DELETE`]
/// (0x75)로 옮긴다 — [`KeyCode::DELETE`] 문서 주석이 이미 밝힌 구분 그대로다.
#[must_use]
pub fn from_web_code(code: &str) -> Option<KeyCode> {
    Some(match code {
        "KeyA" => KeyCode::ANSI_A,
        "KeyB" => KeyCode::ANSI_B,
        "KeyC" => KeyCode::ANSI_C,
        "KeyD" => KeyCode::ANSI_D,
        "KeyE" => KeyCode::ANSI_E,
        "KeyF" => KeyCode::ANSI_F,
        "KeyG" => KeyCode::ANSI_G,
        "KeyH" => KeyCode::ANSI_H,
        "KeyI" => KeyCode::ANSI_I,
        "KeyJ" => KeyCode::ANSI_J,
        "KeyK" => KeyCode::ANSI_K,
        "KeyL" => KeyCode::ANSI_L,
        "KeyM" => KeyCode::ANSI_M,
        "KeyN" => KeyCode::ANSI_N,
        "KeyO" => KeyCode::ANSI_O,
        "KeyP" => KeyCode::ANSI_P,
        "KeyQ" => KeyCode::ANSI_Q,
        "KeyR" => KeyCode::ANSI_R,
        "KeyS" => KeyCode::ANSI_S,
        "KeyT" => KeyCode::ANSI_T,
        "KeyU" => KeyCode::ANSI_U,
        "KeyV" => KeyCode::ANSI_V,
        "KeyW" => KeyCode::ANSI_W,
        "KeyX" => KeyCode::ANSI_X,
        "KeyY" => KeyCode::ANSI_Y,
        "KeyZ" => KeyCode::ANSI_Z,
        "Digit0" => KeyCode::ANSI_0,
        "Digit1" => KeyCode::ANSI_1,
        "Digit2" => KeyCode::ANSI_2,
        "Digit3" => KeyCode::ANSI_3,
        "Digit4" => KeyCode::ANSI_4,
        "Digit5" => KeyCode::ANSI_5,
        "Digit6" => KeyCode::ANSI_6,
        "Digit7" => KeyCode::ANSI_7,
        "Digit8" => KeyCode::ANSI_8,
        "Digit9" => KeyCode::ANSI_9,
        "Space" => KeyCode::SPACE,
        "Enter" => KeyCode::RETURN,
        "Tab" => KeyCode::TAB,
        "Escape" => KeyCode::ESCAPE,
        "Backspace" => KeyCode::DELETE,
        "Delete" => KeyCode::FORWARD_DELETE,
        "ArrowUp" => KeyCode::UP_ARROW,
        "ArrowDown" => KeyCode::DOWN_ARROW,
        "ArrowLeft" => KeyCode::LEFT_ARROW,
        "ArrowRight" => KeyCode::RIGHT_ARROW,
        "Minus" => KeyCode::ANSI_MINUS,
        "Equal" => KeyCode::ANSI_EQUAL,
        "BracketLeft" => KeyCode::ANSI_LEFT_BRACKET,
        "BracketRight" => KeyCode::ANSI_RIGHT_BRACKET,
        "Backslash" => KeyCode::ANSI_BACKSLASH,
        "Semicolon" => KeyCode::ANSI_SEMICOLON,
        "Quote" => KeyCode::ANSI_QUOTE,
        "Comma" => KeyCode::ANSI_COMMA,
        "Period" => KeyCode::ANSI_PERIOD,
        "Slash" => KeyCode::ANSI_SLASH,
        "Backquote" => KeyCode::ANSI_GRAVE,
        "F1" => KeyCode::F1,
        "F2" => KeyCode::F2,
        "F3" => KeyCode::F3,
        "F4" => KeyCode::F4,
        "F5" => KeyCode::F5,
        "F6" => KeyCode::F6,
        "F7" => KeyCode::F7,
        "F8" => KeyCode::F8,
        "F9" => KeyCode::F9,
        "F10" => KeyCode::F10,
        "F11" => KeyCode::F11,
        "F12" => KeyCode::F12,
        "F13" => KeyCode::F13,
        "F14" => KeyCode::F14,
        "F15" => KeyCode::F15,
        "F16" => KeyCode::F16,
        "F17" => KeyCode::F17,
        "F18" => KeyCode::F18,
        "F19" => KeyCode::F19,
        "F20" => KeyCode::F20,
        "Home" => KeyCode::HOME,
        "End" => KeyCode::END,
        "PageUp" => KeyCode::PAGE_UP,
        "PageDown" => KeyCode::PAGE_DOWN,
        _ => return None,
    })
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

    /// `(page << 32) | usage` — 경로 B(`UserKeyMapping`)의 `Src`/`Dst` 로 쓸 수 있는 값
    /// (F-17, `docs/spec/per-device-settings.md` §3.4·§3.6 규칙 4). **35종 전부가
    /// `Some` 이다** — [`SourceKey::keycode`] 와 달리 `F21`~`F24`·`menu (PC)` 도 값을
    /// 갖는다: 경로 B 는 CG 가상 키코드가 아니라 HID usage 로 동작하므로, `kVK_*`
    /// 상수가 없는 키도 HID Usage Tables 상의 usage 는 존재할 수 있다.
    ///
    /// 값의 근거 등급(이 세션의 F-17 구현 계약 CONTRACT.md §2.1, 실측 교차검증 포함):
    /// - `CapsLock` = `0x39` — ⭐ 실측(이 기기의 현재 상태 + 스파이크가 교차 확인).
    /// - `F9`/`F10` = `0x42`/`0x43`, `F13`/`F14`/`F15`/`F18` = `0x68`/`0x69`/`0x6A`/`0x6D`
    ///   — ⭐ 실측으로 교차검증(스파이크 §2·§8 + 이 기기의 현재 상태).
    /// - 나머지 modifier·F-키·`menu (PC)` 는 USB HID Usage Tables 표준 정의를 그대로
    ///   따른다(교차 검증된 값들로부터 Keyboard Page 전체 표가 역산 가능하다).
    /// - `Globe` = `0xFF00000003`(Apple 벤더 정의값) — 저장은 확인됐으나(스파이크 S-7)
    ///   **동작은 미확인**이라 `(미확정)` 등급으로 남긴다. Keyboard Page(`0x07`) 공식을
    ///   따르지 않는 유일한 값이다.
    pub fn hid_usage(self) -> Option<u64> {
        use SourceKey::*;
        const KEYBOARD_PAGE: u64 = 0x07 << 32;
        match self {
            // ⚠️ Globe 만 Keyboard Page 밖의 Apple 벤더 정의 값이다(위 문서 주석).
            Globe => Some(0xFF00000003),
            CapsLock => Some(KEYBOARD_PAGE | 0x39),
            LeftControl => Some(KEYBOARD_PAGE | 0xE0),
            LeftShift => Some(KEYBOARD_PAGE | 0xE1),
            LeftOption => Some(KEYBOARD_PAGE | 0xE2),
            LeftCommand => Some(KEYBOARD_PAGE | 0xE3),
            RightControl => Some(KEYBOARD_PAGE | 0xE4),
            RightShift => Some(KEYBOARD_PAGE | 0xE5),
            RightOption => Some(KEYBOARD_PAGE | 0xE6),
            RightCommand => Some(KEYBOARD_PAGE | 0xE7),
            MenuPc => Some(KEYBOARD_PAGE | 0x65),
            F1 => Some(KEYBOARD_PAGE | 0x3A),
            F2 => Some(KEYBOARD_PAGE | 0x3B),
            F3 => Some(KEYBOARD_PAGE | 0x3C),
            F4 => Some(KEYBOARD_PAGE | 0x3D),
            F5 => Some(KEYBOARD_PAGE | 0x3E),
            F6 => Some(KEYBOARD_PAGE | 0x3F),
            F7 => Some(KEYBOARD_PAGE | 0x40),
            F8 => Some(KEYBOARD_PAGE | 0x41),
            F9 => Some(KEYBOARD_PAGE | 0x42),
            F10 => Some(KEYBOARD_PAGE | 0x43),
            F11 => Some(KEYBOARD_PAGE | 0x44),
            F12 => Some(KEYBOARD_PAGE | 0x45),
            F13 => Some(KEYBOARD_PAGE | 0x68),
            F14 => Some(KEYBOARD_PAGE | 0x69),
            F15 => Some(KEYBOARD_PAGE | 0x6A),
            F16 => Some(KEYBOARD_PAGE | 0x6B),
            F17 => Some(KEYBOARD_PAGE | 0x6C),
            F18 => Some(KEYBOARD_PAGE | 0x6D),
            F19 => Some(KEYBOARD_PAGE | 0x6E),
            F20 => Some(KEYBOARD_PAGE | 0x6F),
            F21 => Some(KEYBOARD_PAGE | 0x70),
            F22 => Some(KEYBOARD_PAGE | 0x71),
            F23 => Some(KEYBOARD_PAGE | 0x72),
            F24 => Some(KEYBOARD_PAGE | 0x73),
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

    // ── hid_usage() — F-17(`docs/spec/per-device-settings.md` §3.4) ──────────────
    //
    // ⭐ 기대값을 구현이 쓰는 상수에서 가져오지 않는다(PR #17 함정, `modifier_flags()`
    // 테스트와 같은 원칙). 이 세션의 F-17 구현 계약(CONTRACT.md §0.3)이 이 기기에서
    // 직접 실측·교차검증한 리터럴을 그대로 적는다.
    #[test]
    fn hid_usage_matches_measured_cross_check_values() {
        let expect: &[(SourceKey, u64)] = &[
            (SourceKey::CapsLock, 0x700000039),
            (SourceKey::F18, 0x70000006D),
            (SourceKey::F9, 0x700000042),
            (SourceKey::F10, 0x700000043),
            (SourceKey::F13, 0x700000068),
            (SourceKey::F15, 0x70000006A),
        ];
        for (k, want) in expect {
            assert_eq!(
                k.hid_usage(),
                Some(*want),
                "{k:?} 의 hid_usage() 가 실측값과 다르다"
            );
        }
    }

    /// 35종 전부가 `Some` 이다 — `keycode()`(물리 keycode)와 달리 `F21`~`F24`·
    /// `menu (PC)` 도 HID usage 는 존재한다(경로 B 는 CG 가상 키코드가 아니라 HID
    /// usage 로 동작하기 때문).
    #[test]
    fn hid_usage_is_some_for_all_35_source_keys() {
        for k in SourceKey::all() {
            assert!(k.hid_usage().is_some(), "{k:?} 의 hid_usage() 가 None 이다");
        }
    }

    /// Globe 만 Keyboard Page(`0x07`) 공식 밖의 Apple 벤더 정의 값이다(스파이크 S-7 —
    /// 저장은 확인됐으나 동작은 미확인).
    #[test]
    fn globe_hid_usage_is_apple_vendor_value() {
        assert_eq!(SourceKey::Globe.hid_usage(), Some(0xFF00000003));
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

    /// F-16.4 트리거 키(`docs/spec/korean-input.md` §3.1). 근거: 위에서 로컬 SDK 헤더
    /// (`Carbon/HIToolbox/Events.h` 243행)로 직접 확인한 `kVK_ANSI_Grave = 0x32`.
    #[test]
    fn ansi_grave_matches_kvk_ansi_grave() {
        assert_eq!(KeyCode::ANSI_GRAVE, KeyCode(0x32));
    }

    /// F-16.2·F-16.3 트리거 키(`docs/spec/korean-input.md` §3.2, D-K12). 근거 3중이
    /// 일치한 값을 리터럴로 직접 적는다 — 구현 상수에서 역산하지 않는다(PR #17 함정).
    /// ⚠️ 이름과 기능이 뒤집혀 보인다는 점(§3.2)을 이 테스트가 다시 못박는다: 한/영이
    /// `JIS_KANA`, 한자가 `JIS_EISU` 다.
    #[test]
    fn jis_kana_and_jis_eisu_match_carbon_header_values() {
        assert_eq!(KeyCode::JIS_KANA, KeyCode(0x68), "한/영 = kVK_JIS_Kana");
        assert_eq!(KeyCode::JIS_EISU, KeyCode(0x66), "한자 = kVK_JIS_Eisu");
    }

    // ── F-01 `from_web_code` — 헤더에서 읽은 값 몇 개를 직접 단언 ──────────────

    #[test]
    fn from_web_code_matches_header_values_for_a_sample() {
        assert_eq!(from_web_code("Space"), Some(KeyCode(0x31)), "kVK_Space");
        assert_eq!(from_web_code("Enter"), Some(KeyCode(0x24)), "kVK_Return");
        assert_eq!(from_web_code("Escape"), Some(KeyCode(0x35)), "kVK_Escape");
        assert_eq!(from_web_code("KeyA"), Some(KeyCode(0x00)), "kVK_ANSI_A");
        assert_eq!(from_web_code("KeyZ"), Some(KeyCode(0x06)), "kVK_ANSI_Z");
        assert_eq!(from_web_code("Digit0"), Some(KeyCode(0x1D)), "kVK_ANSI_0");
        assert_eq!(from_web_code("Digit1"), Some(KeyCode(0x12)), "kVK_ANSI_1");
        assert_eq!(from_web_code("F13"), Some(KeyCode(0x69)), "kVK_F13");
        assert_eq!(
            from_web_code("Semicolon"),
            Some(KeyCode(0x29)),
            "kVK_ANSI_Semicolon"
        );
        assert_eq!(
            from_web_code("Backquote"),
            Some(KeyCode(0x32)),
            "kVK_ANSI_Grave"
        );
    }

    /// `Backspace`(웹)는 macOS 물리 delete 키(0x33), `Delete`(웹)는 forward-delete
    /// (0x75) — 반대로 섞으면 안 된다.
    #[test]
    fn from_web_code_distinguishes_backspace_and_forward_delete() {
        assert_eq!(from_web_code("Backspace"), Some(KeyCode(0x33)));
        assert_eq!(from_web_code("Delete"), Some(KeyCode(0x75)));
    }

    /// 표에 없는 코드(F21~F24 등, 헤더에 `kVK_*` 상수가 없다)는 `None`.
    #[test]
    fn from_web_code_returns_none_for_unknown_codes() {
        assert_eq!(from_web_code("F21"), None);
        assert_eq!(from_web_code("NumpadEnter"), None);
        assert_eq!(from_web_code(""), None);
        assert_eq!(from_web_code("MetaLeft"), None);
    }

    /// 표 안에 중복 keycode 가 없다 — 서로 다른 웹 코드가 같은 물리 키로
    /// 뭉개지면 전역 단축키 레코더가 서로 다른 두 키를 구분하지 못한다.
    #[test]
    fn from_web_code_table_has_no_duplicate_keycodes() {
        const CODES: &[&str] = &[
            "KeyA",
            "KeyB",
            "KeyC",
            "KeyD",
            "KeyE",
            "KeyF",
            "KeyG",
            "KeyH",
            "KeyI",
            "KeyJ",
            "KeyK",
            "KeyL",
            "KeyM",
            "KeyN",
            "KeyO",
            "KeyP",
            "KeyQ",
            "KeyR",
            "KeyS",
            "KeyT",
            "KeyU",
            "KeyV",
            "KeyW",
            "KeyX",
            "KeyY",
            "KeyZ",
            "Digit0",
            "Digit1",
            "Digit2",
            "Digit3",
            "Digit4",
            "Digit5",
            "Digit6",
            "Digit7",
            "Digit8",
            "Digit9",
            "Space",
            "Enter",
            "Tab",
            "Escape",
            "Backspace",
            "Delete",
            "ArrowUp",
            "ArrowDown",
            "ArrowLeft",
            "ArrowRight",
            "Minus",
            "Equal",
            "BracketLeft",
            "BracketRight",
            "Backslash",
            "Semicolon",
            "Quote",
            "Comma",
            "Period",
            "Slash",
            "Backquote",
            "F1",
            "F2",
            "F3",
            "F4",
            "F5",
            "F6",
            "F7",
            "F8",
            "F9",
            "F10",
            "F11",
            "F12",
            "F13",
            "F14",
            "F15",
            "F16",
            "F17",
            "F18",
            "F19",
            "F20",
            "Home",
            "End",
            "PageUp",
            "PageDown",
        ];
        let mut seen: Vec<KeyCode> = CODES.iter().map(|c| from_web_code(c).unwrap()).collect();
        let before = seen.len();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(
            seen.len(),
            before,
            "from_web_code 표에 중복 keycode 가 있다"
        );
    }
}
