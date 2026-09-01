//! 세션 중 키 입력의 분류 — 명세 §3.1·§3.2.
//!
//! ⭐ **판정은 전부 물리 키코드 기준**이다(명세 §3.1) — `;` 순환이 v1.51 에서
//! "regardless of keyboard layout" 으로 고쳐진 이력(superkey-inventory.md
//! §2.1)이 그 근거이고, 제어 키 전부에 같은 원칙을 적용해 같은 버그를
//! 재생산하지 않는다.

use ultrakey_core::event::{EventKind, InputEvent};
use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::KeyCode;

/// 세션 중 키 하나의 분류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKey {
    /// `Enter` — 선택된 매치를 확정한다(§3.2 "Enter" 행).
    Confirm,
    /// `Esc` — 세션을 취소한다(§3.2, `(추정)` — §9 #3).
    Cancel,
    /// `↓`/`Tab` — 다음 매치로 순환한다(§3.2, 실측).
    Next,
    /// `↑`/`⇧Tab` — 이전 매치로 순환한다(§3.2, `↑` 실측·`⇧Tab` `(미확정)`).
    Prev,
    /// `;`(물리 세미콜론) — `Semicolon highlights next match` 가 켜져 있고
    /// modifier 가 없을 때 다음 매치로 순환한다(§3.2 `;` 행).
    CycleSemicolon,
    /// `Delete`(0x33) — 검색어 마지막 문자를 지운다(§3.2 Querying 행).
    Backspace,
    /// 검색어에 들어갈 문자. `typed` 는 호출자가 현재 키보드 레이아웃으로
    /// 해석한 값 그대로다.
    Text(char),
    /// 세션에 아무 영향도 주지 않는 입력.
    Ignore,
}

/// 세션 중 키 입력 하나를 분류한다.
///
/// `typed` 는 **호출자가 현재 키보드 레이아웃으로 해석한 문자**다. 제어 키
/// 판정은 이 값을 절대 보지 않는다 — 레이아웃이 바뀌어도 `↑↓Tab Enter Esc ;`
/// 는 같은 물리 위치여야 한다(§3.1). 반대로 검색어에 들어갈 문자는 레이아웃을
/// 따라야 하므로 이 크레이트가 스스로 만들지 않고 받는다(이 크레이트는 순수
/// 로직이라 macOS 의 문자 변환 API 를 호출할 수 없다 — 명세 §7 "Rust 바인딩"
/// 판정이 F-01 에 허용한 것은 이벤트 타입뿐이지 텍스트 입력 서비스가
/// 아니다).
#[must_use]
pub fn classify(ev: &InputEvent, typed: Option<char>, semicolon_cycles: bool) -> SessionKey {
    // 세션 라우팅 계약 — `handle_key` 는 이미 정규화된 kind(KeyDown/KeyUp)를
    // 받는다는 계약 위에 서 있다. caps lock 같은 modifier 키는 macOS 가
    // `FlagsChanged` 로만 보내고 `ultrakey-core::Arbiter` 가 그것을 down/up
    // 으로 환원한다(key-remapping-engine.md 참조) — 그 환원은 F-07 의
    // 책임이지 이 함수의 책임이 아니다. 정규화되지 않은 `FlagsChanged` 가
    // 그대로 들어오면 여기서는 **조용히 `Ignore`** 로 떨어진다. 이것은 버그가
    // 아니라 계약 위반의 증상이다 — 호출자가 정규화를 빼먹었다는 뜻이다.
    if ev.kind != EventKind::KeyDown {
        return SessionKey::Ignore;
    }

    match ev.keycode {
        // §3.2 "Enter" 행. 실측: AX + nib(§3.1 ⓘ 팝오버 원문).
        KeyCode::RETURN => return SessionKey::Confirm,
        // §3.2, `(추정)` — ⓘ 팝오버 원문이 Esc 를 언급하지 않는다(§9 #3).
        KeyCode::ESCAPE => return SessionKey::Cancel,
        // §3.2, 실측: AX + nib(§3.1).
        KeyCode::DOWN_ARROW => return SessionKey::Next,
        KeyCode::UP_ARROW => return SessionKey::Prev,
        // §3.2 — `⇧Tab` 은 방향 대칭성으로부터의 설계다(§9 #3b `(미확정)`,
        // ⓘ 팝오버 원문은 "tab" 만 언급하고 방향을 특정하지 않는다).
        KeyCode::TAB => {
            return if ev.flags.contains(EventFlags::SHIFT) {
                SessionKey::Prev
            } else {
                SessionKey::Next
            };
        }
        // §3.2 Querying 행이 Backspace 를 명시한다. 0x33 은 물리적으로
        // "delete"/backspace 키다(forward-delete 는 별도 keycode 0x75).
        KeyCode::DELETE => return SessionKey::Backspace,
        // §3.2 `;` 행 — `Semicolon highlights next match` 가 켜져 있고
        // shift/control/option/command 가 전혀 없을 때만 순환으로 판정한다.
        //
        // ⭐ 이 modifier 조건은 명세에 없는 **이 구현의 판단**이다. 근거:
        // `Semicolon highlights next match` 를 켜면 `:`(⇧`;`)조차 검색어에
        // 넣을 수 없게 되는데, 순환은 modifier 없는 단독 타격이 자연스러운
        // 제스처이고 명세도 그 이상을 요구하지 않는다. 조건을 만족하지
        // 못하면(설정이 꺼져 있거나 modifier 가 실려 있으면) 아래 문자
        // 경로로 떨어진다.
        KeyCode::ANSI_SEMICOLON if semicolon_cycles && has_no_cycle_blocking_modifier(ev.flags) => {
            return SessionKey::CycleSemicolon;
        }
        _ => {}
    }

    // ⌘Tab 등 단축키의 잔여 입력이 검색어에 섞이지 않게, COMMAND/CONTROL/
    // ALTERNATE 중 하나라도 실려 있으면 버린다. SHIFT 는 대문자 입력에
    // 필요하므로 제외한다.
    if ev.flags.contains(EventFlags::COMMAND)
        || ev.flags.contains(EventFlags::CONTROL)
        || ev.flags.contains(EventFlags::ALTERNATE)
    {
        return SessionKey::Ignore;
    }

    match typed {
        Some(c) if !c.is_control() => SessionKey::Text(c),
        _ => SessionKey::Ignore,
    }
}

/// `;` 순환 판정에서 shift/control/option/command 가 전혀 없는가.
fn has_no_cycle_blocking_modifier(flags: EventFlags) -> bool {
    !(flags.contains(EventFlags::SHIFT)
        || flags.contains(EventFlags::CONTROL)
        || flags.contains(EventFlags::ALTERNATE)
        || flags.contains(EventFlags::COMMAND))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_down(keycode: KeyCode, flags: EventFlags) -> InputEvent {
        InputEvent {
            kind: EventKind::KeyDown,
            keycode,
            flags,
            autorepeat: false,
        }
    }

    // ── 제어 키 — 실측·물리 keycode 기준 ─────────────────────────────────

    #[test]
    fn return_is_confirm() {
        let ev = key_down(KeyCode::RETURN, EventFlags::NONE);
        assert_eq!(classify(&ev, Some('\r'), false), SessionKey::Confirm);
    }

    #[test]
    fn escape_is_cancel() {
        let ev = key_down(KeyCode::ESCAPE, EventFlags::NONE);
        assert_eq!(classify(&ev, None, false), SessionKey::Cancel);
    }

    #[test]
    fn down_and_up_arrow_cycle() {
        let down = key_down(KeyCode::DOWN_ARROW, EventFlags::NONE);
        let up = key_down(KeyCode::UP_ARROW, EventFlags::NONE);
        assert_eq!(classify(&down, None, false), SessionKey::Next);
        assert_eq!(classify(&up, None, false), SessionKey::Prev);
    }

    /// ⭐ Tab(no shift) → Next. `⇧Tab` 은 `EventFlags::SHIFT` 비트를 실은
    /// `KeyCode::TAB` 의 `KeyDown` 으로 만든다 — 위임 지시가 요구하는 정확한
    /// 이벤트 모양이다.
    #[test]
    fn tab_without_shift_is_next_shift_tab_is_prev() {
        let tab = key_down(KeyCode::TAB, EventFlags::NONE);
        let shift_tab = key_down(KeyCode::TAB, EventFlags::SHIFT);
        assert_eq!(classify(&tab, Some('\t'), false), SessionKey::Next);
        assert_eq!(classify(&shift_tab, Some('\t'), false), SessionKey::Prev);
    }

    #[test]
    fn delete_is_backspace() {
        let ev = key_down(KeyCode::DELETE, EventFlags::NONE);
        assert_eq!(classify(&ev, None, false), SessionKey::Backspace);
    }

    // ── `;` 순환 — §3.2 `;` 행, §4 설정 ──────────────────────────────────

    #[test]
    fn semicolon_cycles_when_enabled_and_no_modifier() {
        let ev = key_down(KeyCode::ANSI_SEMICOLON, EventFlags::NONE);
        assert_eq!(classify(&ev, Some(';'), true), SessionKey::CycleSemicolon);
    }

    /// 설정이 꺼져 있으면 세미콜론은 그냥 문자로 들어간다.
    #[test]
    fn semicolon_is_text_when_setting_disabled() {
        let ev = key_down(KeyCode::ANSI_SEMICOLON, EventFlags::NONE);
        assert_eq!(classify(&ev, Some(';'), false), SessionKey::Text(';'));
    }

    /// ⭐ 이 구현의 판단 — 설정이 켜져 있어도 shift(`:`)가 실려 있으면 순환이
    /// 아니라 문자 경로로 떨어진다(`Semicolon highlights next match` 를
    /// 켜도 `:` 는 여전히 입력할 수 있어야 한다).
    #[test]
    fn semicolon_with_shift_is_text_even_when_setting_enabled() {
        let ev = key_down(KeyCode::ANSI_SEMICOLON, EventFlags::SHIFT);
        assert_eq!(classify(&ev, Some(':'), true), SessionKey::Text(':'));
    }

    // ── caps lock 정규화 계약 ────────────────────────────────────────────

    /// ⭐ caps lock 은 `EventKind::FlagsChanged` + `KeyCode::CAPS_LOCK`(0x39) +
    /// `EventFlags::CAPS_LOCK` 비트로 도착한다 — `KeyDown`/`KeyUp` 으로
    /// 흉내 내지 않는다. `handle_key`(및 이 함수)는 이미 정규화된 kind 를
    /// 받는다는 계약 위에 서 있으므로, 정규화되지 않은 `FlagsChanged` 를
    /// 그대로 주면 **아무 일도 일어나지 않는다**(`Ignore`) — 그래서 호출자
    /// (`ultrakey-core::Arbiter`)가 반드시 down/up 으로 환원해서 넘겨야
    /// 한다.
    #[test]
    fn unnormalized_flags_changed_caps_lock_is_ignored() {
        let ev = InputEvent {
            kind: EventKind::FlagsChanged,
            keycode: KeyCode::CAPS_LOCK,
            flags: EventFlags::CAPS_LOCK,
            autorepeat: false,
        };
        assert_eq!(classify(&ev, None, false), SessionKey::Ignore);
    }

    // ── modifier 잔여 입력 필터링 ────────────────────────────────────────

    #[test]
    fn command_control_option_are_filtered_out_of_query() {
        let cmd = key_down(KeyCode::ANSI_A, EventFlags::COMMAND);
        let ctrl = key_down(KeyCode::ANSI_A, EventFlags::CONTROL);
        let opt = key_down(KeyCode::ANSI_A, EventFlags::ALTERNATE);
        assert_eq!(classify(&cmd, Some('a'), false), SessionKey::Ignore);
        assert_eq!(classify(&ctrl, Some('a'), false), SessionKey::Ignore);
        assert_eq!(classify(&opt, Some('a'), false), SessionKey::Ignore);
    }

    /// SHIFT 는 대문자 입력에 필요하므로 필터링 대상이 아니다.
    #[test]
    fn shift_alone_is_still_text() {
        let ev = key_down(KeyCode::ANSI_A, EventFlags::SHIFT);
        assert_eq!(classify(&ev, Some('A'), false), SessionKey::Text('A'));
    }

    #[test]
    fn plain_character_is_text() {
        let ev = key_down(KeyCode::ANSI_S, EventFlags::NONE);
        assert_eq!(classify(&ev, Some('s'), false), SessionKey::Text('s'));
    }

    #[test]
    fn key_up_is_always_ignored() {
        let ev = InputEvent {
            kind: EventKind::KeyUp,
            keycode: KeyCode::ANSI_A,
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        assert_eq!(classify(&ev, Some('a'), false), SessionKey::Ignore);
    }

    // ── ⭐ 이슈 #76 — 한/영 키(0x68) 방어 테스트 ──────────────────────────────────

    /// ⭐ 계약 방어 — 한/영 키(`JIS_KANA`)는 세션 중 계층 1 에서 원본 그대로 통과되므로
    /// `SeekKey` 로 여기까지 도달하지 않는다(`docs/spec/seek-activation-and-session.md`
    /// §3.1 한/영 키 예외). 그래도 계약 위반(예: 통과 분기가 정규화 경로를 벗어남) 시
    /// 조용히 `Ignore` 로 떨어지는 것을 고정한다. `typed = None` 이면 이미 `Ignore`
    /// 경로로 떨어지므로 구현 변경은 없다 — 테스트만 추가한다.
    #[test]
    fn jis_kana_key_down_without_typed_char_is_ignored() {
        let ev = key_down(KeyCode::JIS_KANA, EventFlags::NONE);
        assert_eq!(classify(&ev, None, false), SessionKey::Ignore);
    }
}
