//! F-16 · 한국어 입력 지원 — 한국어 입력기 활성 판정(`docs/spec/korean-input.md` §3.3, D-K2)
//! + 규칙 트리거 조건과 modifier 키 정의(§3.1, D-K5).
//!
//! macOS 에 의존하지 않는다 — `crate` 전체의 `#![forbid(unsafe_code)]` 를 그대로 따른다.

use std::sync::atomic::{AtomicU8, Ordering};

use crate::keycode::KeyCode;

/// 한국어 입력기 활성 여부의 3상태.
///
/// ⭐ `Unknown` 이 fail-closed 의 표현이다(`docs/spec/korean-input.md` §3.3) — 입력 소스
/// 조회 자체가 실패했거나 아직 이루어지지 않은 상태와, "한국어가 아님"을 구분한다.
/// `Default` 가 `Unknown` 인 것 자체가 안전한 초기값이다: 게이트가 아직 한 번도
/// 게시되지 않은 상태에서 한국어 전용 규칙이 오발화하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KoreanImeState {
    Active,
    Inactive,
    #[default]
    Unknown,
}

/// `kTISPropertyInputSourceLanguages` 로 얻은 언어 태그 목록에서 "한국어 입력기가
/// 활성인가"를 판정한다.
///
/// 판정 규칙(`docs/spec/korean-input.md` §3.3):
/// - 빈 슬라이스 → [`KoreanImeState::Unknown`] — 조회 자체가 실패한 것과 구분할 수
///   없으므로 fail-closed 로 다룬다.
/// - **첫 원소만** 본다 — Karabiner 의 `input_source_if { language: "ko" }` 와 같은
///   기준이다. 언어 서브태그(`-` 앞부분)를 ASCII 소문자로 비교해 `"ko"` 면
///   [`KoreanImeState::Active`].
/// - 그 밖은 전부 [`KoreanImeState::Inactive`].
pub fn classify_input_source_languages(langs: &[String]) -> KoreanImeState {
    classify_input_source_languages_for(langs, "ko")
}

/// [`classify_input_source_languages`] 를 언어 태그 일반화한 것 — F-19.3 이
/// "일본어 입력 소스 활성"을 같은 3상태 fail-closed 로 판정할 때 쓴다
/// (`docs/spec/language-presets.md` §3 F-19.3). `want` 는 `"ko"`·`"ja"` 같은
/// 언어 서브태그다.
pub fn classify_input_source_languages_for(langs: &[String], want: &str) -> KoreanImeState {
    let Some(first) = langs.first() else {
        return KoreanImeState::Unknown;
    };
    let subtag = first.split('-').next().unwrap_or(first);
    if subtag.eq_ignore_ascii_case(want) {
        KoreanImeState::Active
    } else {
        KoreanImeState::Inactive
    }
}

/// 콜백(탭 스레드)이 O(1)·무할당으로 읽는 쪽. `gate.rs` 의 [`crate::gate::AtomicAppGate`] 와
/// 같은 규약이다 — 메인 스레드가 `kTISNotifySelectedKeyboardInputSourceChanged` 수신 시
/// [`classify_input_source_languages`] 로 판정한 결과를 여기에 게시(publish)하고, 콜백은
/// 이 값을 읽기만 한다.
pub struct AtomicKoreanImeGate {
    state: AtomicU8,
}

impl AtomicKoreanImeGate {
    pub fn new() -> Self {
        AtomicKoreanImeGate {
            state: AtomicU8::new(encode(KoreanImeState::default())),
        }
    }

    /// ⭐ 콜백이 부르는 쪽. `Ordering::Acquire` — `store` 가 `Release` 로 게시한 값을
    /// 이 로드 이후의 코드가 관찰하도록 보장한다.
    pub fn load(&self) -> KoreanImeState {
        decode(self.state.load(Ordering::Acquire))
    }

    /// 메인 스레드가 부르는 쪽.
    pub fn store(&self, value: KoreanImeState) {
        self.state.store(encode(value), Ordering::Release);
    }
}

impl Default for AtomicKoreanImeGate {
    fn default() -> Self {
        Self::new()
    }
}

fn encode(state: KoreanImeState) -> u8 {
    match state {
        KoreanImeState::Active => 0,
        KoreanImeState::Inactive => 1,
        KoreanImeState::Unknown => 2,
    }
}

fn decode(raw: u8) -> KoreanImeState {
    match raw {
        0 => KoreanImeState::Active,
        1 => KoreanImeState::Inactive,
        _ => KoreanImeState::Unknown,
    }
}

/// F-16 규칙의 트리거 성립 조건(`docs/spec/korean-input.md` §3.1, D-K5).
/// ⭐ 판정은 **정본 눌림 테이블**로만 한다 — `ev.flags` 의 modifier 비트로 판정하지
/// 않는다(좌/우 shift 가 같은 비트를 공유해 구분 불가, `docs/dev/architecture.md` §6.4 P3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KoreanTrigger {
    /// F-16.1 — shift(좌·우 아무거나)가 눌려 있고, 그 밖의 modifier 키는 하나도 눌려
    /// 있지 않다.
    ShiftOnly,
    /// F-16.4 — modifier 키가 **하나도** 눌려 있지 않다.
    NoModifier,
}

/// ⭐ "modifier 가 눌려 있는가" 를 판정할 때 보는 물리 키 전량.
/// caps lock 도 포함한다 — 명세 §3.1 "그 외 어떤 modifier 도 눌려 있지 않을 때".
/// ⚠️ `alphaShift` **잠금 비트**는 애초에 보지 않는다(눌림 테이블로 판정하므로 자동 배제).
pub const MODIFIER_KEYS: [KeyCode; 10] = [
    KeyCode::LEFT_SHIFT,
    KeyCode::RIGHT_SHIFT,
    KeyCode::LEFT_CONTROL,
    KeyCode::RIGHT_CONTROL,
    KeyCode::LEFT_OPTION,
    KeyCode::RIGHT_OPTION,
    KeyCode::LEFT_COMMAND,
    KeyCode::RIGHT_COMMAND,
    KeyCode::FUNCTION,
    KeyCode::CAPS_LOCK,
];

/// ⭐ K9(이슈 #73, D-K18) — "⌘/⌃(command·control) 를 제외한 modifier 가 눌린 상태에서
/// **문자 키**(A~Z)의 keyDown 이 왔는가".
///
/// 이것이 K9 옵션의 **발화 조건 판정**이다 — 명세 §3.7. 판정은 [`MODIFIER_KEYS`] 와
/// 같은 정본 눌림 테이블(`is_pressed`) 기준이며, `ev.flags` 를 보지 않는다(§3.1 P3 —
/// 좌/우 shift 가 같은 비트를 공유해 구분 불가).
///
/// ⭐ **command·control 은 변환을 유발하는 modifier 에서 뺀다.** `⌘C`·`⌃C` 같은 조합은
/// 거의 항상 앱·시스템 단축키다 — 그 문자를 소문자 "c" 로 바꿔 버리면 복사가 깨진다.
/// 문제가 되는 것은 `⌥`/`⇧`/fn 계열과 함께 누른 문자가 한국어 IME 에서 한글 자모로
/// 나가는 경우고, 그 사용자만 이 옵션을 켠다(명세 §3.7). caps lock 은 애초에
/// `alphaShift` 잠금이지 눌림이 아니라(§3.1) 대상에서 빼는 것이 아니라, **caps lock
/// 조합(shift+caps+A 등)은 대문자 의도**라 K9 의 판정에서 제외한다 — 즉 caps lock 이
/// 눌려 있으면 발화하지 않는다.
///
/// 반환값은 `Some(소문자)` — `None` 이면 발화 조건이 아니다.
pub fn lowercase_action_for(
    keycode: KeyCode,
    pressed: &dyn Fn(KeyCode) -> bool,
) -> Option<char> {
    // ① trigger_key 가 ANSI 문자 키(A~Z, 0x00~0x0C·0x0D..0x20 의 문자 구간)인가.
    let c = char_of_letter_key(keycode)?;
    // ② 어떤 modifier 가 눌려 있는가 — 하나도 없으면 조건이 아니다(F-16.4 영역).
    let any_modifier = MODIFIER_KEYS.iter().any(|k| pressed(*k));
    if !any_modifier {
        return None;
    }
    // ③ ⛔ command·control·caps lock 이 눌려 있으면 발화하지 않는다(§3.7 — 앱 단축키
    //    보호, 대문자 의도 보호). shift·option·fn 계열만 변환 대상이다.
    let protected = pressed(KeyCode::LEFT_COMMAND)
        || pressed(KeyCode::RIGHT_COMMAND)
        || pressed(KeyCode::LEFT_CONTROL)
        || pressed(KeyCode::RIGHT_CONTROL)
        || pressed(KeyCode::CAPS_LOCK);
    if protected {
        return None;
    }
    // ④ 출력은 영어 소문자다 — modifier+문자의 대문자/한글 조합을 소문자로 바꿔 인식.
    Some(c.to_ascii_lowercase())
}

/// ANSI 문자 키(A~Z)의 keycode → 대문자. 그 밖은 `None` — 숫자·기호·한/영·한자 등은
/// K9 의 대상이 아니다(명세 §3.7 "문자 키").
///
/// ⚠️ **범위 비교로 하지 않는다.** Carbon keycode 는 A~Z 가 연속적이지 않다 —
/// `kVK_ANSI_A`(0x00)~`kVK_ANSI_Z`(0x06) 사이에 0x04(=H)·0x05(=G)·0x06(=Z)가 섞여
/// 있고, `0x0A`(kVK_ANSI_Section)·0x10(Y 위쪽)·0x18(Equal)·0x19(0)·0x1C(8) 같은
/// 비문자 키가 사이사이 있다. 그래서 이 함수는 아래 표(A~Z 각각의 헤더 값)를 **이진
/// 탐색**하며 — 표는 오름차순 정렬돼 있다.
pub fn char_of_letter_key(keycode: KeyCode) -> Option<char> {
    /// ⚠️ **이 표는 오름차순 정렬**이어야 `binary_search` 가 성립한다 — Carbon keycode
    /// 는 A~Z 가 알파벳 순서가 아니라(0x00=A, 0x01=S, 0x02=D …) 물리 배열 순서라
    /// **알파벳이 아니라 keycode 기준으로 정렬한 대응표**를 둔다.
    const LETTER_KEYS: [(u16, char); 26] = [
        (KeyCode::ANSI_A.0, 'A'),
        (KeyCode::ANSI_S.0, 'S'),
        (KeyCode::ANSI_D.0, 'D'),
        (KeyCode::ANSI_F.0, 'F'),
        (KeyCode::ANSI_H.0, 'H'),
        (KeyCode::ANSI_G.0, 'G'),
        (KeyCode::ANSI_Z.0, 'Z'),
        (KeyCode::ANSI_X.0, 'X'),
        (KeyCode::ANSI_C.0, 'C'),
        (KeyCode::ANSI_V.0, 'V'),
        (KeyCode::ANSI_B.0, 'B'),
        (KeyCode::ANSI_Q.0, 'Q'),
        (KeyCode::ANSI_W.0, 'W'),
        (KeyCode::ANSI_E.0, 'E'),
        (KeyCode::ANSI_R.0, 'R'),
        (KeyCode::ANSI_Y.0, 'Y'),
        (KeyCode::ANSI_T.0, 'T'),
        (KeyCode::ANSI_O.0, 'O'),
        (KeyCode::ANSI_U.0, 'U'),
        (KeyCode::ANSI_I.0, 'I'),
        (KeyCode::ANSI_P.0, 'P'),
        (KeyCode::ANSI_L.0, 'L'),
        (KeyCode::ANSI_J.0, 'J'),
        (KeyCode::ANSI_K.0, 'K'),
        (KeyCode::ANSI_N.0, 'N'),
        (KeyCode::ANSI_M.0, 'M'),
    ];
    LETTER_KEYS
        .binary_search_by_key(&keycode.0, |&(kc, _)| kc)
        .ok()
        .map(|idx| LETTER_KEYS[idx].1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(items: &[&str]) -> Vec<String> {
        items.iter().map(|s| s.to_string()).collect()
    }

    // 표 형태 검증 — 기대값은 전부 명세(korean-input.md §3.3, D-K2)에서 그대로 옮긴 것.
    #[test]
    fn classify_matches_spec_table() {
        let cases: &[(&[&str], KoreanImeState)] = &[
            (&["ko"], KoreanImeState::Active),
            (&["ko-KR"], KoreanImeState::Active),
            (&["ja"], KoreanImeState::Inactive),
            (&["zh-Hans"], KoreanImeState::Inactive),
            (&["en"], KoreanImeState::Inactive),
            (&[], KoreanImeState::Unknown),
            (&["KO"], KoreanImeState::Active),
            // ⭐ 첫 원소만 본다 — 두 번째 원소에 "ko" 가 있어도 무시한다.
            (&["en", "ko"], KoreanImeState::Inactive),
        ];

        for (langs, expected) in cases {
            let input = v(langs);
            assert_eq!(
                classify_input_source_languages(&input),
                *expected,
                "입력 {langs:?} 에 대한 판정이 명세와 다르다"
            );
        }
    }

    // fail-closed 의 표현: Default 가 Unknown 이다.
    #[test]
    fn default_state_is_unknown() {
        assert_eq!(KoreanImeState::default(), KoreanImeState::Unknown);
    }

    // AtomicKoreanImeGate 왕복.
    #[test]
    fn gate_roundtrip() {
        let gate = AtomicKoreanImeGate::new();
        assert_eq!(gate.load(), KoreanImeState::Unknown);

        gate.store(KoreanImeState::Active);
        assert_eq!(gate.load(), KoreanImeState::Active);

        gate.store(KoreanImeState::Inactive);
        assert_eq!(gate.load(), KoreanImeState::Inactive);

        gate.store(KoreanImeState::Unknown);
        assert_eq!(gate.load(), KoreanImeState::Unknown);
    }

    #[test]
    fn gate_default_matches_new() {
        let gate = AtomicKoreanImeGate::default();
        assert_eq!(gate.load(), KoreanImeState::Unknown);
    }

    /// D-K5 — modifier 키 10종: 좌우 shift/control/option/command + FUNCTION + CAPS_LOCK.
    #[test]
    fn modifier_keys_has_10_entries_including_caps_lock() {
        assert_eq!(MODIFIER_KEYS.len(), 10);
        assert!(MODIFIER_KEYS.contains(&KeyCode::CAPS_LOCK));
        assert!(MODIFIER_KEYS.contains(&KeyCode::LEFT_SHIFT));
        assert!(MODIFIER_KEYS.contains(&KeyCode::RIGHT_SHIFT));
        assert!(MODIFIER_KEYS.contains(&KeyCode::FUNCTION));
    }

    // ── ⭐ K9(이슈 #73, D-K18) — modifier+문자키 → 영어 소문자 변환 ────────────────

    /// ANSI 문자 키 26개가 순서대로 A~Z 로 나온다. keycode 0x00~0x19 는 Carbon 헤더의
    /// `kVK_ANSI_A`(0x00)~`kVK_ANSI_Z`(0x12) 구간이며 **연속적이지 않다** — 0x04~0x07
    /// 사이에 `kVK_ANSI_H`(0x04)·`kVK_ANSI_G`(0x05)·`kVK_ANSI_Z`(0x06)가 끼어 있다.
    /// 그래서 이 테스트가 전체 지도를 고정한다.
    #[test]
    fn char_of_letter_key_covers_exactly_the_26_ansi_letters() {
        for (keycode, expected) in [
            (KeyCode::ANSI_A, 'A'),
            (KeyCode::ANSI_S, 'S'),
            (KeyCode::ANSI_D, 'D'),
            (KeyCode::ANSI_F, 'F'),
            (KeyCode::ANSI_H, 'H'),
            (KeyCode::ANSI_G, 'G'),
            (KeyCode::ANSI_Z, 'Z'),
            (KeyCode::ANSI_X, 'X'),
            (KeyCode::ANSI_C, 'C'),
            (KeyCode::ANSI_V, 'V'),
            (KeyCode::ANSI_B, 'B'),
            (KeyCode::ANSI_Q, 'Q'),
            (KeyCode::ANSI_W, 'W'),
            (KeyCode::ANSI_E, 'E'),
            (KeyCode::ANSI_R, 'R'),
            (KeyCode::ANSI_Y, 'Y'),
            (KeyCode::ANSI_T, 'T'),
            (KeyCode::ANSI_U, 'U'),
            (KeyCode::ANSI_O, 'O'),
            (KeyCode::ANSI_U, 'U'),
            (KeyCode::ANSI_I, 'I'),
            (KeyCode::ANSI_P, 'P'),
            (KeyCode::ANSI_L, 'L'),
            (KeyCode::ANSI_J, 'J'),
            (KeyCode::ANSI_K, 'K'),
            (KeyCode::ANSI_N, 'N'),
            (KeyCode::ANSI_M, 'M'),
        ] {
            assert_eq!(char_of_letter_key(keycode), Some(expected), "keycode {:#04X}", keycode.0);
        }

        // 문자 키가 아닌 것 — 전부 None.
        for not_a_letter in [
            KeyCode::ANSI_0,
            KeyCode::ANSI_1,
            KeyCode::ANSI_MINUS,
            KeyCode::ANSI_EQUAL,
            KeyCode::ANSI_LEFT_BRACKET,
            KeyCode::ANSI_RIGHT_BRACKET,
            KeyCode::ANSI_BACKSLASH,
            KeyCode::ANSI_SEMICOLON,
            KeyCode::ANSI_QUOTE,
            KeyCode::ANSI_COMMA,
            KeyCode::ANSI_PERIOD,
            KeyCode::ANSI_SLASH,
            KeyCode::ANSI_GRAVE,
            KeyCode::SPACE,
            KeyCode::RETURN,
            KeyCode::ESCAPE,
            KeyCode::TAB,
            KeyCode::LEFT_SHIFT,
            KeyCode::JIS_KANA,
            KeyCode(0x30),
            KeyCode(0x33),
            KeyCode(0x1A),
        ] {
            assert_eq!(
                char_of_letter_key(not_a_letter),
                None,
                "keycode {:#04X} 는 문자 키가 아니어야 한다",
                not_a_letter.0
            );
        }
    }

    /// `lowercase_action_for` — 명세 §3.7 표 그대로.
    #[test]
    fn k9_matches_spec_table() {
        let pressed: Vec<KeyCode> = vec![KeyCode::LEFT_SHIFT];
        let any = |k: KeyCode| pressed.contains(&k);

        // shift+A → 'a'
        assert_eq!(lowercase_action_for(KeyCode::ANSI_A, &any), Some('a'));
        // shift+S → 's'
        assert_eq!(lowercase_action_for(KeyCode::ANSI_S, &any), Some('s'));

        // option+A → 'a'
        let alt: Vec<KeyCode> = vec![KeyCode::LEFT_OPTION];
        let any_alt = |k: KeyCode| alt.contains(&k);
        assert_eq!(lowercase_action_for(KeyCode::ANSI_A, &any_alt), Some('a'));

        // fn+A → 'a'
        let fn_held: Vec<KeyCode> = vec![KeyCode::FUNCTION];
        let fn_any = |k: KeyCode| fn_held.contains(&k);
        assert_eq!(lowercase_action_for(KeyCode::ANSI_A, &fn_any), Some('a'));

        // ⛔ command+A → None — 앱 단축키 보호(§3.7).
        let cmd: Vec<KeyCode> = vec![KeyCode::LEFT_COMMAND];
        let cmd_any = |k: KeyCode| cmd.contains(&k);
        assert_eq!(lowercase_action_for(KeyCode::ANSI_A, &cmd_any), None);

        // ⛔ control+A → None.
        let ctrl: Vec<KeyCode> = vec![KeyCode::RIGHT_CONTROL];
        let ctrl_any = |k: KeyCode| ctrl.contains(&k);
        assert_eq!(lowercase_action_for(KeyCode::ANSI_A, &ctrl_any), None);

        // ⛔ caps lock+A → None — 대문자 의도 보호(§3.7).
        let caps: Vec<KeyCode> = vec![KeyCode::CAPS_LOCK];
        let caps_any = |k: KeyCode| caps.contains(&k);
        assert_eq!(lowercase_action_for(KeyCode::ANSI_A, &caps_any), None);

        // modifier 없음 → None — F-16.4(백틱) 등 다른 규칙 영역이다.
        let none: Vec<KeyCode> = Vec::new();
        let none_any = |k: KeyCode| none.contains(&k);
        assert_eq!(lowercase_action_for(KeyCode::ANSI_A, &none_any), None);

        // 문자 키가 아닌 것(space·grave·한/영) → None — 어떤 modifier 조합이든.
        let grave_held: Vec<KeyCode> = vec![KeyCode::LEFT_OPTION];
        let grave_any = |k: KeyCode| grave_held.contains(&k);
        assert_eq!(lowercase_action_for(KeyCode::ANSI_GRAVE, &grave_any), None);
        assert_eq!(lowercase_action_for(KeyCode::JIS_KANA, &grave_any), None);
        assert_eq!(lowercase_action_for(KeyCode::SPACE, &grave_any), None);
    }
}
