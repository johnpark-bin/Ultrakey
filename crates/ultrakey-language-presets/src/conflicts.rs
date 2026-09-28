//! F-19 언어 규칙 충돌 감지 — `docs/spec/language-presets.md` §5 #1, P2 확정(H1).
//!
//! 같은 물리 키코드를 **소스**로 하는 활성 규칙 ≥2 가 있으면 F-15 충돌 대화상자
//! ("상대 설정을 끄고 이걸 켤까요?")를 띄운다. 규칙 ID 로 조용히 승부를 가르지 않는다
//! (명세 §5 #1 — 먼저 만든 규칙과 나중에 켠 규칙의 우선순위를 사용자 선택으로 명시화).
//!
//! ## 확정 pair 세트 (2026-09-03 P2 리뷰, 카탈로그 원문 대조)
//!
//! | pair | 공유 소스 |
//! | :--- | :--- |
//! | F-19.1 ↔ F-19.3 · F-19.1 ↔ F-19.7 · F-19.3 ↔ F-19.7 | CAPS_LOCK AloneTap |
//! | F-08.2 ↔ F-19.1/3/7 각각 | CAPS_LOCK QuickPress (크로스 패밀리 — presets 스냅샷) |
//! | F-19.2 ↔ F-19.4(우⌘) | RIGHT_COMMAND AloneTap |
//! | F-19.5(JIS 행) ↔ F-19.6(행 10/11) | 0x5D NoModifier |
//!
//! ⛔ **아닌 것**: F-19.5(US 행)·F-19.6(행 19/20) 의 0x2A 는 키보드 타입 게이트로 상호
//! 배타(동시 활성 불가)라 pair 아님. F-16.2/16.3 ↔ F-19.3/19.4 는 출력(0x66/0x68) vs
//! 소스 관계이며 엔진 마커로 우회돼 런타임 상호작용이 없다. F-16.2 ↔ F-19.6 은 공유
//! 키코드가 없다(0x68 ∉ F-19.6 from 집합).

use ultrakey_core::settings::keys;

/// 충돌 종류 — `PendingConflictView.kind` 문자열과 대응한다(`conflict_kind_str`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageConflictKind {
    /// 같은 물리 키코드를 소스로 주장하는 언어 규칙 2개가 동시 활성.
    LanguageKeycodeShared,
}

/// 충돌 하나 — 끌 상대 설정의 저장 키 목록.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageConflict {
    pub kind: LanguageConflictKind,
    /// 끄게 될 설정의 저장 키 — `korean.*`·`japanese.*`·`chinese.*`·`presets.*`.
    /// UI 라벨은 `disable_label_key_for_setting`(main.rs)이 카탈로그 키로 옮긴다.
    pub to_disable: Vec<&'static str>,
}

impl LanguageConflictKind {
    /// `PendingConflictView.kind` — i18n `settings.presets.conflict.title.<kind>` 키와
    /// 맞물리는 camelCase 문자열.
    pub fn as_str(self) -> &'static str {
        match self {
            LanguageConflictKind::LanguageKeycodeShared => "languageKeycodeShared",
        }
    }
}

/// 캡스락을 **트리거**로 주장하는 F-19 규칙의 저장 키 (F-19.1·F-19.3·F-19.7).
const CAPS_LOCK_LANGUAGE_KEYS: &[&str] = &[
    keys::KOREAN_CAPS_LOCK_SWITCHES_INPUT_SOURCE,
    keys::JAPANESE_CAPS_LOCK_TOGGLES_EISU_KANA,
    keys::CHINESE_CAPS_LOCK_SWITCHES_INPUT_SOURCE,
];

/// 오른쪽 command 를 트리거로 주장하는 규칙의 저장 키 (F-19.2·F-19.4 우⌘ 행).
const RIGHT_COMMAND_LANGUAGE_KEYS: &[&str] = &[
    keys::KOREAN_RIGHT_COMMAND_SWITCHES_INPUT_SOURCE,
    keys::JAPANESE_COMMAND_TOGGLES_EISU_KANA,
];

/// `changing_key` 를 `new_value=true` 로 켤 때, 이미 켜져 있어 충돌하는 활성 규칙의
/// 저장 키 목록을 찾는다.
///
/// `active_keys` 는 현재 켜져 있는 언어 프리셋 저장 키들이다(앱이 상태 스냅샷에서
/// 추린다). `presets_caps_quick_enabled` 는 F-08.2(크로스 패밀리 pair)의 현재 상태다.
pub fn detect_language_conflict(
    active_keys: &[&str],
    presets_caps_quick_enabled: bool,
    changing_key: &str,
    new_value: bool,
) -> Option<LanguageConflict> {
    if !new_value {
        return None;
    }
    let shared = language_shared_source_keys(active_keys, changing_key, presets_caps_quick_enabled)?;
    Some(LanguageConflict {
        kind: LanguageConflictKind::LanguageKeycodeShared,
        to_disable: shared,
    })
}

/// 같은 물리 소스 키를 주장하는, `changing_key` 외의 활성 규칙 저장 키 — 없으면 `None`.
///
/// ⚠️ **кусок 로직 주의**: 한 저장 키가 여러 규칙을 낼 수 있다(F-19.4 는 좌·우⌘ 둘,
/// F-19.5 는 4행). 따라서 "같은 소스 키" 는 저장 키 단위가 아니라 **규칙 단위**로
/// 따져야 한다 — 그래서 이 함수는 아래 키-쌍 표를 저장 키 단위로 고정해 둔다(각 키가
/// 주장하는 소스 키 집합과 충돌 쌍이 결정적으로 보이는 표기).
fn language_shared_source_keys(
    active_keys: &[&str],
    changing_key: &str,
    presets_caps_quick_enabled: bool,
) -> Option<Vec<&'static str>> {
    let mut disabled = Vec::new();

    // 캡스락 트리거 pair — (A) F-19 규칙 상호, (B) F-08.2 크로스 패밀리.
    if changing_key == keys::KOREAN_CAPS_LOCK_SWITCHES_INPUT_SOURCE
        || changing_key == keys::JAPANESE_CAPS_LOCK_TOGGLES_EISU_KANA
        || changing_key == keys::CHINESE_CAPS_LOCK_SWITCHES_INPUT_SOURCE
    {
        for k in CAPS_LOCK_LANGUAGE_KEYS {
            if *k != changing_key && active_keys.contains(k) {
                disabled.push(*k);
            }
        }
        if presets_caps_quick_enabled {
            disabled.push(keys::PRESETS_CAPS_QUICK_PRESS_ENABLED);
        }
        return non_empty(disabled);
    }

    // 오른쪽 command 트리거 pair.
    if changing_key == keys::KOREAN_RIGHT_COMMAND_SWITCHES_INPUT_SOURCE
        || changing_key == keys::JAPANESE_COMMAND_TOGGLES_EISU_KANA
    {
        for k in RIGHT_COMMAND_LANGUAGE_KEYS {
            if *k != changing_key && active_keys.contains(k) {
                disabled.push(*k);
            }
        }
        return non_empty(disabled);
    }

    // 0x5D(JIS ¥) 트리거 pair — F-19.5(swap_yen_backslash) ↔ F-19.6(jis_as_us_symbols).
    // 둘은 JIS 행에서 같은 소스(0x5D)를 주장한다.
    if (changing_key == keys::JAPANESE_SWAP_YEN_BACKSLASH
        && active_keys.contains(&keys::JAPANESE_JIS_AS_US_SYMBOLS))
        || (changing_key == keys::JAPANESE_JIS_AS_US_SYMBOLS
            && active_keys.contains(&keys::JAPANESE_SWAP_YEN_BACKSLASH))
    {
        let other = if changing_key == keys::JAPANESE_SWAP_YEN_BACKSLASH {
            keys::JAPANESE_JIS_AS_US_SYMBOLS
        } else {
            keys::JAPANESE_SWAP_YEN_BACKSLASH
        };
        disabled.push(other);
        return non_empty(disabled);
    }

    None
}

/// `disabled`(끌 상대 목록)가 비면 `None`(충돌 아님) — 활성 규칙 중 어느 것도
/// `changing_key` 와 소스 키를 겹치게 주장하지 않았다는 뜻이다. 비어 있는데도
/// `Some(vec![])` 를 돌려주면 "켜는 조작이 항상 충돌한다" 로 오판하게 된다.
fn non_empty(disabled: Vec<&'static str>) -> Option<Vec<&'static str>> {
    if disabled.is_empty() {
        None
    } else {
        Some(disabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn active(keys: &[&'static str]) -> Vec<&'static str> {
        keys.to_vec()
    }

    #[test]
    fn turning_off_never_conflicts() {
        for k in CAPS_LOCK_LANGUAGE_KEYS {
            assert!(detect_language_conflict(&active(CAPS_LOCK_LANGUAGE_KEYS), false, k, false).is_none());
        }
    }

    /// F-19.1 켜기 → F-19.3·F-19.7 이 켜져 있으면 둘 다 끄게 된다.
    #[test]
    fn f191_conflicts_with_f193_and_f197() {
        let c = detect_language_conflict(
            &active(&[keys::JAPANESE_CAPS_LOCK_TOGGLES_EISU_KANA, keys::CHINESE_CAPS_LOCK_SWITCHES_INPUT_SOURCE]),
            false,
            keys::KOREAN_CAPS_LOCK_SWITCHES_INPUT_SOURCE,
            true,
        )
        .expect("캡스락 소스 규칙 2개가 이미 활성이면 충돌이다");
        assert_eq!(c.kind, LanguageConflictKind::LanguageKeycodeShared);
        assert_eq!(c.to_disable.len(), 2);
        assert!(c.to_disable.contains(&keys::JAPANESE_CAPS_LOCK_TOGGLES_EISU_KANA));
        assert!(c.to_disable.contains(&keys::CHINESE_CAPS_LOCK_SWITCHES_INPUT_SOURCE));
    }

    /// ⭐ 크로스 패밀리 — F-08.2(Quick press caps lock)가 켜져 있는데 F-19.1 을 켜려 하면
    /// F-08.2 를 끄게 된다(P1 "FSM 하나, 출력 하나" 결정론).
    #[test]
    fn f191_conflicts_with_f082_quick_press() {
        let c = detect_language_conflict(
            &[],
            true,
            keys::KOREAN_CAPS_LOCK_SWITCHES_INPUT_SOURCE,
            true,
        )
        .expect("F-08.2 가 켜져 있으면 충돌이다");
        assert_eq!(c.to_disable, vec![keys::PRESETS_CAPS_QUICK_PRESS_ENABLED]);
    }

    /// F-19.4(우⌘) ↔ F-19.2(우⌘) — `korean.*`↔`japanese.*` 네임스페이스 교차 해제.
    #[test]
    fn f192_conflicts_with_f194_right_command() {
        let c = detect_language_conflict(
            &active(&[keys::JAPANESE_COMMAND_TOGGLES_EISU_KANA]),
            false,
            keys::KOREAN_RIGHT_COMMAND_SWITCHES_INPUT_SOURCE,
            true,
        )
        .expect("우⌘ 소스 규칙이 이미 활성이면 충돌이다");
        assert_eq!(c.to_disable, vec![keys::JAPANESE_COMMAND_TOGGLES_EISU_KANA]);
    }

    #[test]
    fn f194_right_command_conflicts_with_f192() {
        let c = detect_language_conflict(
            &active(&[keys::KOREAN_RIGHT_COMMAND_SWITCHES_INPUT_SOURCE]),
            false,
            keys::JAPANESE_COMMAND_TOGGLES_EISU_KANA,
            true,
        )
        .expect("우⌘ 소스 규칙이 이미 활성이면 충돌이다");
        assert_eq!(c.to_disable, vec![keys::KOREAN_RIGHT_COMMAND_SWITCHES_INPUT_SOURCE]);
    }

    /// F-19.5(JIS 행) ↔ F-19.6 — 0x5D 소스 공유.
    #[test]
    fn f195_conflicts_with_f196_on_yen_key() {
        let both = detect_language_conflict(
            &active(&[keys::JAPANESE_JIS_AS_US_SYMBOLS]),
            false,
            keys::JAPANESE_SWAP_YEN_BACKSLASH,
            true,
        );
        assert_eq!(both.map(|c| c.to_disable), Some(vec![keys::JAPANESE_JIS_AS_US_SYMBOLS]));

        let reverse = detect_language_conflict(
            &active(&[keys::JAPANESE_SWAP_YEN_BACKSLASH]),
            false,
            keys::JAPANESE_JIS_AS_US_SYMBOLS,
            true,
        );
        assert_eq!(reverse.map(|c| c.to_disable), Some(vec![keys::JAPANESE_SWAP_YEN_BACKSLASH]));
    }

    /// ⛔ 제외 pair — 캡스락 아닌 키·무관 키는 충돌이 아니다.
    #[test]
    fn unrelated_keys_do_not_conflict() {
        assert!(detect_language_conflict(&active(&[keys::JAPANESE_JIS_AS_US_SYMBOLS]), false, keys::KOREAN_CAPS_LOCK_SWITCHES_INPUT_SOURCE, true).is_none());
        assert!(detect_language_conflict(&active(CAPS_LOCK_LANGUAGE_KEYS), false, keys::KOREAN_WON_KEY_TYPES_BACKTICK, true).is_none());
    }
}