//! ⭐(이슈 #93) — 인풋 박스 모드(다국어 Seek 세션)의 키 통과 판정.
//!
//! 다국어 세션에서 검색 바는 실제 `<input>` 이 되고, macOS `NSTextInputContext` 가
//! IME 조합을 수행하려면 **물리 키 이벤트가 웹뷰에 그대로 도달**해야 한다. 그래서
//! 계층 1 게이트가 "이 키는 인풋 박스로 통과시킨다 / 이 키는 세션 컨트롤을 위해
//! 계속 소비한다"를 물리 keycode+flags 로 가른다. 이 모듈이 그 유일한 판정 헬퍼다.
//!
//! ⚠️ **크레이트 경계**: 이 판정은 `ultrakey-critical` 이 아니라 `ultrakey-core` 가
//! 소유한다 — `GateSnapshot` 을 넘겨받는 `Arbiter::arbitrate` 가 그 자리에서 이
//! 분기를 판정하므로, `ultrakey-seek-session`(의존 방향: 세션 → core)에 두면 순환
//! 의존이 생긴다. `classify`(`ultrakey-seek-session/keys.rs`)가 `Text`/`Backspace`
//! 로 보는 키와 **일치하는지**는 테스트(T13)로 대조한다.

use crate::event::InputEvent;
use crate::flags::EventFlags;
use crate::keycode::KeyCode;

/// 인풋 박스 모드에서 이 키를 **원본 그대로 통과**시킬 것인가.
///
/// 통과하면 웹뷰 `<input>` 이 실제 IME 조합·데드키·Backspace 를 네이티브로
/// 처리한다. 소비하면 기존 계층 1 경로(Enter·Esc·↑↓·Tab·`;` 순환 → 세션 컨트롤,
/// ⌘⌃ 조합 → 앱 단축키 보호)를 그대로 탄다.
///
/// 규칙(Plan §3 D4, 상급 리뷰 §9 #3·#4 반영):
/// - `Delete`(backspace) → 통과(입력 요소가 조합 단계를 포함해 올바르게 지운다).
/// - 문자·숫자·공백·기호 keycode 로서 **⌘/⌃ modifier 가 안 실려 있는 것** → 통과.
///   ⭐ `⌥` 는 차단하지 않는다 — US 배열 데드키(`⌥+e` → ´)가 스페인어 악센트
///   입력에 필요하기 때문. 단 설정된 전역 단축키 조합은 호출자가 이 함수로 들어
///   오기 전에(또는 별도 가드로) Consume 해야 한다.
/// - ⌘/⌃ 을 실은 키 · `Enter`·`Esc`·`↑`·`↓`·`Tab` → 소비.
/// - `;` 는 `semicolon_cycles` 가 켜져 있고 ⌘⌃⌥ 를 안 실었으면 순환 키로 소비,
///   아니면 통과(`;` 를 검색어에 넣을 권리).
/// - `Shift`(대문자)·`⌥` 단독·modifier 키·`JIS_EISU` 등은 여기서 다루지 않는다
///   (modifier 키는 <code>FlagsChanged</code> 가 통과되는 편이 상태가 깨지지
///   않는다 — 호출자가 KeyDown/KeyUp/FlagsChanged 전체를 같은 판정으로 보낸다).
#[must_use]
pub fn is_input_box_pass_key(ev: &InputEvent, semicolon_cycles: bool) -> bool {
    let flags = &ev.flags;

    match ev.keycode {
        // 인터·세션 컨트롤 키 — 반드시 소비(웹뷰에 빼앗기면 안 된다).
        KeyCode::RETURN | KeyCode::ESCAPE | KeyCode::UP_ARROW | KeyCode::DOWN_ARROW
        | KeyCode::TAB => return false,
        // Backspace — 인풋 박스가 조합 단계를 포함해 처리. ⌘⌃ 없이 단독 키일
        // 때만(위 match 는 이미 제어 키를 걸렀다 — 아래에서 ⌘⌃ 은 돌려준다).
        KeyCode::DELETE => {
            return !has_browser_command_modifier(flags);
        }
        // `;` 순환 — 설정이 켜져 있고 ⌘⌃⌥ 를 안 실었으면 다음 매치 순환 키다.
        // (`keys.rs` 의 `CycleSemicolon` 판정과 동일한 조건.)
        KeyCode::ANSI_SEMICOLON if semicolon_cycles && !has_any_modifier(flags) => return false,
        _ => {}
    }

    // ⌘/⌃ 조합은 항상 소비 — ⌘Q 로 Ultrakey 프로세스가 죽는 것을 막고(다국어
    // 세션 중에는 우리 앱이 활성 앱이다), 앱 단축키가 검색어에 새지 않게 한다.
    if has_browser_command_modifier(flags) {
        return false;
    }

    // 나머지 — 프린터블 keycode(문자·숫자·공백·기호)면 통과.
    is_printable_keycode(ev.keycode)
}

/// ⌘/⌃ 중 하나라도 실려 있는가 — 데스크톱 앱 단축키(⌘C·⌃C 등) 보호.
fn has_browser_command_modifier(flags: &EventFlags) -> bool {
    flags.contains(EventFlags::COMMAND) || flags.contains(EventFlags::CONTROL)
}

/// ⭐(이슈 #101) — **시스템 입력 소스 전환 단축키**(⌃Space·⌃⌥Space 등)인가.
///
/// [`is_input_box_pass_key`] 의 ⌘/⌃ 소비 판정에 걸리기 **전에** 계층 1 게이트가 이
/// 조합을 가른다 — ⌃Space 는 앱 단축키가 아니라 **시스템(Carbon HIToolbox)** 이
/// 처리하는 입력 소스 전환 단축키라, 소비하면 전환 자체가 발화할 기회를 잃는다.
/// 인풋 박스 모드에서 검색 바가 키 윈도우일 때도 통과시키면 시스템이 처리한다
/// (플랜 §3 D1, 이슈 #101).
///
/// 판정: `SPACE` + ⌃ 실림 + ⌘ 미실림.
/// - ⌃Space(이전 입력 소스)·⌃⌥Space(입력 메뉴의 다음 소스) — macOS 기본 단축키 2종과
///   사용자 지정 ⌃계열 단축키를 커버한다.
/// - ⛔ ⌘ 가 실리면 제외 — ⌘Space(Spotlight) 등은 종전대로 소비(앱 단축키 보호 원칙).
/// - ⇧ 는 가리지 않는다 — 제한을 새로 만들지 않으며, 시스템이 처리하지 않는 ⌃⇧Space 는
///   통과해도 키 윈도우(우리 오버레이)에서 무해하다.
/// - ⚠️ 설정된 세션 토글 단축키와 일치하는 경우는 호출자가 이 함수 **다음**에 가드해야
///   한다 — 통과시키면 재입력 토글(세션 닫기)이 죽는다(플랜 §3 D1).
#[must_use]
pub fn is_input_source_switch_shortcut(ev: &InputEvent) -> bool {
    ev.keycode == KeyCode::SPACE
        && ev.flags.contains(EventFlags::CONTROL)
        && !ev.flags.contains(EventFlags::COMMAND)
}

/// ⇧⌃⌥⌘ 중 하나라도 실려 있는가 — `;` 순환 판정의 "modifier 부재" 조건.
fn has_any_modifier(flags: &EventFlags) -> bool {
    flags.contains(EventFlags::SHIFT)
        || flags.contains(EventFlags::CONTROL)
        || flags.contains(EventFlags::ALTERNATE)
        || flags.contains(EventFlags::COMMAND)
}

/// 문자·숫자·공백·기호를 내는 물리 keycode 인가.
///
/// `handle_key` 의 `SessionKey::Text(char)` 경로가 레이아웃 번역(`typed`)을 거쳐
/// 문자를 받는 것과 달리, 계층 1 은 레이아웃을 모르므로 **keycode 로만** 판정한다.
/// QWERTY/로케일과 무관하게 "글자를 낼 수 있는 물리 키"의 목록이 여기 있다 —
/// AZERTY 등에서도 물리 위치는 같다(`;` 물리 위치가 AZERTY 에서 `M` 을 내도
/// keycode 는 `ANSI_SEMICOLON`). IME 가 실제 문자를 결정하는 것은 웹뷰의 몫이다.
#[must_use]
pub fn is_printable_keycode(keycode: KeyCode) -> bool {
    // ⚠️ 연관 const 를 matches! 패턴에 그대로 쓸 수 없어(`KeyCode` 는 struct),
    // 전체 경로로 나열한다.
    matches!(
        keycode,
        KeyCode::ANSI_A
            | KeyCode::ANSI_B
            | KeyCode::ANSI_C
            | KeyCode::ANSI_D
            | KeyCode::ANSI_E
            | KeyCode::ANSI_F
            | KeyCode::ANSI_G
            | KeyCode::ANSI_H
            | KeyCode::ANSI_I
            | KeyCode::ANSI_J
            | KeyCode::ANSI_K
            | KeyCode::ANSI_L
            | KeyCode::ANSI_M
            | KeyCode::ANSI_N
            | KeyCode::ANSI_O
            | KeyCode::ANSI_P
            | KeyCode::ANSI_Q
            | KeyCode::ANSI_R
            | KeyCode::ANSI_S
            | KeyCode::ANSI_T
            | KeyCode::ANSI_U
            | KeyCode::ANSI_V
            | KeyCode::ANSI_W
            | KeyCode::ANSI_X
            | KeyCode::ANSI_Y
            | KeyCode::ANSI_Z
            | KeyCode::ANSI_0
            | KeyCode::ANSI_1
            | KeyCode::ANSI_2
            | KeyCode::ANSI_3
            | KeyCode::ANSI_4
            | KeyCode::ANSI_5
            | KeyCode::ANSI_6
            | KeyCode::ANSI_7
            | KeyCode::ANSI_8
            | KeyCode::ANSI_9
            | KeyCode::ANSI_MINUS
            | KeyCode::ANSI_EQUAL
            | KeyCode::ANSI_LEFT_BRACKET
            | KeyCode::ANSI_RIGHT_BRACKET
            | KeyCode::ANSI_BACKSLASH
            | KeyCode::ANSI_SEMICOLON
            | KeyCode::ANSI_QUOTE
            | KeyCode::ANSI_COMMA
            | KeyCode::ANSI_PERIOD
            | KeyCode::ANSI_SLASH
            | KeyCode::ANSI_GRAVE
            | KeyCode::SPACE
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventKind;

    fn ev(keycode: KeyCode, flags: EventFlags) -> InputEvent {
        InputEvent {
            kind: EventKind::KeyDown,
            keycode,
            flags,
            autorepeat: false,
        }
    }

    // ── 통과 (⌘⌃ 없음, 프린터블/backspace) ──────────────────────────────

    #[test]
    fn plain_letter_passes() {
        let e = ev(KeyCode::ANSI_A, EventFlags::NONE);
        assert!(is_input_box_pass_key(&e, false));
        assert!(is_input_box_pass_key(&e, true));
    }

    #[test]
    fn shift_letter_passes() {
        let e = ev(KeyCode::ANSI_A, EventFlags::SHIFT);
        assert!(is_input_box_pass_key(&e, false));
    }

    /// ⭐ 상급 리뷰 #3 — `⌥+문자` 는 **통과**한다(US 배열 데드키 → 스페인어
    /// 악센트). ⌘⌃ 보호와 정반대의 의도된 비대칭.
    #[test]
    fn option_letter_passes_for_dead_keys() {
        let e = ev(KeyCode::ANSI_E, EventFlags::ALTERNATE);
        assert!(is_input_box_pass_key(&e, false));
    }

    #[test]
    fn space_passes() {
        let e = ev(KeyCode::SPACE, EventFlags::NONE);
        assert!(is_input_box_pass_key(&e, false));
    }

    #[test]
    fn punctuation_passes() {
        let e = ev(KeyCode::ANSI_SEMICOLON, EventFlags::NONE);
        // 설정 꺼짐 → `;` 는 그냥 문자로 통과.
        assert!(is_input_box_pass_key(&e, false));
    }

    #[test]
    fn delete_passes_as_backspace() {
        let e = ev(KeyCode::DELETE, EventFlags::NONE);
        assert!(is_input_box_pass_key(&e, false));
    }

    // ── 소비 (세션 컨트롤 · ⌘⌃ 보호) ──────────────────────────────────────

    #[test]
    fn confirm_and_cancel_keys_are_consumed() {
        for kc in [KeyCode::RETURN, KeyCode::ESCAPE] {
            let e = ev(kc, EventFlags::NONE);
            assert!(!is_input_box_pass_key(&e, false));
        }
    }

    #[test]
    fn arrow_and_tab_keys_are_consumed() {
        for kc in [KeyCode::UP_ARROW, KeyCode::DOWN_ARROW, KeyCode::TAB] {
            let e = ev(kc, EventFlags::NONE);
            assert!(!is_input_box_pass_key(&e, false));
        }
    }

    #[test]
    fn semicolon_is_consumed_when_cycling_enabled() {
        let e = ev(KeyCode::ANSI_SEMICOLON, EventFlags::NONE);
        assert!(!is_input_box_pass_key(&e, true));
        // ⭐ 상급 리뷰 #4 — 설정 파라미터화: 켜지면 순환(소비), 꺼지면 통과.
        assert!(is_input_box_pass_key(&e, false));
    }

    #[test]
    fn command_control_are_consumed() {
        // ⌘A·⌃A — 앱 단축키 보호(다국어 세션 중 우리 앱이 활성이므로 ⌘Q 까지
        // 통과시키면 프로세스가 죽는다).
        for flags in [EventFlags::COMMAND, EventFlags::CONTROL] {
            let e = ev(KeyCode::ANSI_A, flags);
            assert!(!is_input_box_pass_key(&e, false));
        }
    }

    #[test]
    fn command_control_backspace_are_consumed() {
        for flags in [EventFlags::COMMAND, EventFlags::CONTROL] {
            let e = ev(KeyCode::DELETE, flags);
            assert!(!is_input_box_pass_key(&e, false));
        }
    }

    #[test]
    fn function_and_navigation_keys_are_not_printable() {
        for kc in [
            KeyCode::F1,
            KeyCode::F12,
            KeyCode::JIS_EISU,
            KeyCode::CAPS_LOCK,
            KeyCode::LEFT_SHIFT,
        ] {
            assert!(!is_printable_keycode(kc), "{kc:?}");
        }
    }

    // ── ⭐(이슈 #101, 플랜 §4 S1~S6) — 시스템 입력 소스 전환 단축키 판정 ──────────────

    /// S1 — ⌃Space(일반 CONTROL + device 좌측 control 비트, macOS 실제 모양) → true.
    /// 값은 `flags.rs` 상수가 아니라 macOS 헤더 리터럴(0x40000 | 0x1) — 검증 하네스 관례.
    #[test]
    fn control_space_is_input_source_switch() {
        let e = ev(KeyCode::SPACE, EventFlags(0x0004_0001));
        assert!(is_input_source_switch_shortcut(&e));
    }

    /// S2 — ⌃⌥Space(0x40000 | 0x1 | 0x80000) → true. "입력 메뉴의 다음 소스" 기본 단축키.
    #[test]
    fn control_option_space_is_input_source_switch() {
        let e = ev(KeyCode::SPACE, EventFlags(0x000C_0001));
        assert!(is_input_source_switch_shortcut(&e));
    }

    /// S2-b(리뷰 #2-a) — ⌃⇧Space → true. ⇧ 는 가리지 않는 결정을 고정한다.
    #[test]
    fn control_shift_space_is_input_source_switch() {
        let e = ev(KeyCode::SPACE, EventFlags(0x0006_0001));
        assert!(is_input_source_switch_shortcut(&e));
    }

    /// S2-c(리뷰 #2-c) — **우측** control 로 ⌃Space(0x40000 | 0x2000) → true.
    /// 좌/우 control 이 같은 일반 비트를 공유하므로 device 비트가 달라도 판정은 같다.
    #[test]
    fn control_space_with_right_device_bit_is_input_source_switch() {
        let e = ev(KeyCode::SPACE, EventFlags(0x0004_2000));
        assert!(is_input_source_switch_shortcut(&e));
    }

    /// S2-d — ⌃Space + caps lock 잠금 비트(0x10000) → true. 잠금 상태는 판정을 흔들지 않는다.
    #[test]
    fn control_space_with_caps_lock_bit_is_input_source_switch() {
        let e = ev(KeyCode::SPACE, EventFlags(0x0005_0001));
        assert!(is_input_source_switch_shortcut(&e));
    }

    /// S3 — ⌘Space → false(앱 단축키 보호 — Spotlight 등은 계속 소비).
    #[test]
    fn command_space_is_not_input_source_switch() {
        let e = ev(KeyCode::SPACE, EventFlags::COMMAND);
        assert!(!is_input_source_switch_shortcut(&e));
    }

    /// S4 — ⌘⌃Space → false(COMMAND 가드가 CONTROL 보다 우선).
    #[test]
    fn command_control_space_is_not_input_source_switch() {
        let e = ev(KeyCode::SPACE, EventFlags(0x0014_0009));
        assert!(!is_input_source_switch_shortcut(&e));
    }

    /// S5 — ⌃ 없는 Space 변형(무modifier·⇧·⌥) → false.
    #[test]
    fn space_without_control_is_not_input_source_switch() {
        for flags in [
            EventFlags::NONE,
            EventFlags::SHIFT,
            EventFlags::ALTERNATE,
            EventFlags::SHIFT | EventFlags::ALTERNATE,
        ] {
            let e = ev(KeyCode::SPACE, flags);
            assert!(!is_input_source_switch_shortcut(&e), "{flags:?}");
        }
    }

    /// S6 — ⌃ 이지만 SPACE 가 아닌 키(⌃A) → false. 판정은 keycode 로 한정된다.
    #[test]
    fn control_without_space_is_not_input_source_switch() {
        let e = ev(KeyCode::ANSI_A, EventFlags::CONTROL);
        assert!(!is_input_source_switch_shortcut(&e));
    }
}