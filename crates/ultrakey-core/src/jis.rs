//! F-19 · 키보드 타입 게이트(`docs/spec/language-presets.md` §3.2 F-19.5·F-19.6, D-4).
//!
//! JIS(일본어 배열) 키보드와 ANSI/ISO 키보드는 심볼 키의 물리 배치가 다르다 —
//! F-19.6(JIS→US)은 **JIS 에서만**, F-19.5(¥↔\) 는 JIS·US 각자의 물리 키에서만
//! 동작해야 한다. 이 모듈은 "지금 쓰는 키보드가 JIS 인가"를 3상태로 게이트한다.
//!
//! ⭐ **3상태 fail-closed**(H4, P2 확정): `Unknown` 에서는 JIS 행·US 행 전부 발화하지
//! 않는다. `false`(NotJis) 기본으로 시작하면 JIS 에서 US 행이 `]` 키(0x2A)를 강탈하고,
//! `true`(Jis) 기본이면 ANSI 에서 F-19.6 이 심볼행을 오치환한다 — 어느 쪽도 파괴적이므로
//! 판정 전(Unknown)에는 어느 쪽도 발화시키지 않는 것이 안전하다.
//!
//! 판정은 메인 스레드가 TIS(`kTISPropertyInputSourceKeyboardType`) 또는 IOKit 폴백으로
//! 계산해 [`AtomicJisGate`] 에 게시하고, 콜백(탭 스레드)은 O(1) 로드만 한다 —
//! [`crate::korean::AtomicKoreanImeGate`] 와 같은 규약이다.

use std::sync::atomic::{AtomicU8, Ordering};

/// 키보드 타입 판정 3상태.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum JisState {
    Jis,
    NotJis,
    #[default]
    Unknown,
}

/// 콜백(탭 스레드)이 O(1)·무할당으로 읽는 쪽. 메인 스레드가 판정 결과를 게시하면
/// 콜백은 이 값을 읽기만 한다(`korean::AtomicKoreanImeGate` 와 같은 규약).
pub struct AtomicJisGate {
    state: AtomicU8,
}

impl AtomicJisGate {
    pub fn new() -> Self {
        AtomicJisGate {
            state: AtomicU8::new(encode(JisState::default())),
        }
    }

    /// 콜백이 부르는 쪽.
    pub fn load(&self) -> JisState {
        decode(self.state.load(Ordering::Acquire))
    }

    /// 메인 스레드가 부르는 쪽.
    pub fn store(&self, value: JisState) {
        self.state.store(encode(value), Ordering::Release);
    }
}

impl Default for AtomicJisGate {
    fn default() -> Self {
        Self::new()
    }
}

fn encode(state: JisState) -> u8 {
    match state {
        JisState::Jis => 0,
        JisState::NotJis => 1,
        JisState::Unknown => 2,
    }
}

fn decode(raw: u8) -> JisState {
    match raw {
        0 => JisState::Jis,
        1 => JisState::NotJis,
        _ => JisState::Unknown,
    }
}

/// TIS 키보드 타입 문자열(`"ANSI"`/`"ISO"`/`"JIS"`)을 [`JisState`] 로 옮긴다.
/// `kTISPropertyInputSourceKeyboardType` 가 macOS 12+ 에서 이 값을 반환한다
/// `(P2 확정 방향 — 실측 및 IOKit 폴백 여부는 구현 시 판정)`.
pub fn classify_keyboard_type(raw: &str) -> JisState {
    if raw.eq_ignore_ascii_case("JIS") {
        JisState::Jis
    } else if raw.is_empty() {
        JisState::Unknown
    } else {
        JisState::NotJis
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn case(raw: &str, expected: JisState) {
        assert_eq!(
            classify_keyboard_type(raw),
            expected,
            "키보드 타입 문자열 {raw:?} 에 대한 판정이 다르다"
        );
    }

    #[test]
    fn classify_keyboard_type_matches_tis_and_unknown() {
        case("JIS", JisState::Jis);
        case("jis", JisState::Jis);
        case("ANSI", JisState::NotJis);
        case("ISO", JisState::NotJis);
        case("", JisState::Unknown);
        case("klingon", JisState::NotJis);
    }

    #[test]
    fn default_is_unknown() {
        assert_eq!(JisState::default(), JisState::Unknown);
    }

    /// ⭐ H4 — `Unknown` (기본값)이 fail-closed 인지가 이 게이트의 핵심이다.
    /// 게시 전에는 JIS 행도 US 행도 발화하지 않아야 한다 — 양쪽 실패 모드가
    /// 파괴적이기 때문이다(모듈 문서).
    #[test]
    fn gate_defaults_to_unknown() {
        let gate = AtomicJisGate::new();
        assert_eq!(gate.load(), JisState::Unknown);
    }

    #[test]
    fn gate_roundtrip() {
        let gate = AtomicJisGate::new();
        gate.store(JisState::Jis);
        assert_eq!(gate.load(), JisState::Jis);
        gate.store(JisState::NotJis);
        assert_eq!(gate.load(), JisState::NotJis);
        gate.store(JisState::Unknown);
        assert_eq!(gate.load(), JisState::Unknown);
    }
}