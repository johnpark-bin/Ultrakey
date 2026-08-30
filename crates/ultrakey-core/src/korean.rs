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
    let Some(first) = langs.first() else {
        return KoreanImeState::Unknown;
    };
    let subtag = first.split('-').next().unwrap_or(first);
    if subtag.eq_ignore_ascii_case("ko") {
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
}
