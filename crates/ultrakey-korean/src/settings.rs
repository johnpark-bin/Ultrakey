//! `Korean` 탭 5개 항목의 설정값 — `docs/spec/korean-input.md` §4.2 표 그대로.
//!
//! ⭐ `ultrakey_presets::PresetSettings::from_store` 와 같은 "부재 = 기본값" 규약을
//! 따른다(F-15 `settings-store-and-integrity.md` §3.1) — **단 하나의 예외가 있다.**
//! [`KoreanSettings::disable_in_remote_desktop`] 은 기본값이 `true` 다. 명세 §4.2 각주가
//! "구현·리뷰 양쪽에서 놓치기 쉬운 지점"으로 명시한 곳이므로, `Default`·`from_store`·
//! 아래 테스트 세 곳 모두에서 이 반전을 명시적으로 다룬다.

use serde::{Deserialize, Serialize};

use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::KeyCode;
use ultrakey_core::korean::KoreanTrigger;
use ultrakey_core::rules::{KoreanRule, RuleId};
use ultrakey_core::settings::{keys, SettingsStore};

/// `Korean` 탭 전체 — F-16.1~F-16.4 체크박스 4개 + 항목 5(원격 데스크톱 제외 토글).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KoreanSettings {
    /// F-16.1 `Shift + Space 로 입력 소스 변경`. 기본 ☐.
    #[serde(default)]
    pub shift_space_switches_input_source: bool,
    /// F-16.2 `한/영 키로 입력 소스 변경`. 기본 ☐. ⚠️ §3.2 의 키코드가 `(미확정)`인
    /// 동안 [`KoreanSettings::to_rules`] 는 이 값을 읽어도 규칙을 내지 않는다(D-K8).
    #[serde(default)]
    pub han_eng_switches_input_source: bool,
    /// F-16.3 `한자 키로 한자 변환`. 기본 ☐. 상동(D-K8) — 규칙을 내지 않는다.
    #[serde(default)]
    pub hanja_key_converts_hanja: bool,
    /// F-16.4 `₩ 키로 백틱(\`) 입력`. 기본 ☐.
    #[serde(default)]
    pub won_key_types_backtick: bool,
    /// 항목 5 `원격 데스크톱 클라이언트에서 한국어 키 처리 끄기`.
    /// ⚠️ **기본값 `true`** — F-16 항목 중 유일하게 부재가 켜짐을 뜻한다(명세 §4.2 각주).
    #[serde(default = "default_disable_in_remote_desktop")]
    pub disable_in_remote_desktop: bool,
}

fn default_disable_in_remote_desktop() -> bool {
    true
}

impl Default for KoreanSettings {
    fn default() -> Self {
        KoreanSettings {
            shift_space_switches_input_source: false,
            han_eng_switches_input_source: false,
            hanja_key_converts_hanja: false,
            won_key_types_backtick: false,
            disable_in_remote_desktop: true,
        }
    }
}

impl KoreanSettings {
    /// [`SettingsStore`]에서 필드별로 조립한다 — 키가 없는 필드는 그 필드의 기본값을
    /// 쓴다("부재 = 기본값", F-15 §3.1). `disable_in_remote_desktop` 만 부재 시 `true`
    /// 로 읽는다 — 다른 네 필드와 반대 방향이다(명세 §4.2 각주).
    pub fn from_store(store: &SettingsStore) -> Self {
        KoreanSettings {
            shift_space_switches_input_source: store
                .get(keys::KOREAN_SHIFT_SPACE_SWITCHES_INPUT_SOURCE)
                .unwrap_or_default(),
            han_eng_switches_input_source: store
                .get(keys::KOREAN_HAN_ENG_SWITCHES_INPUT_SOURCE)
                .unwrap_or_default(),
            hanja_key_converts_hanja: store
                .get(keys::KOREAN_HANJA_KEY_CONVERTS_HANJA)
                .unwrap_or_default(),
            won_key_types_backtick: store
                .get(keys::KOREAN_WON_KEY_TYPES_BACKTICK)
                .unwrap_or_default(),
            // ⚠️ 부재 = true. 다른 필드처럼 `unwrap_or_default()` 를 쓰면 부재가
            // `false` 로 읽혀 명세 §4.2 각주를 조용히 어기게 된다.
            disable_in_remote_desktop: store
                .get(keys::KOREAN_DISABLE_IN_REMOTE_DESKTOP)
                .unwrap_or(true),
        }
    }

    /// 켜진 F-16 규칙을 `RuleId` 오름차순으로 정렬해 반환한다.
    ///
    /// ⭐ **1단계 범위**: F-16.1(`Korean(13)`)·F-16.4(`Korean(16)`) 두 규칙만 낸다.
    /// `han_eng_switches_input_source`/`hanja_key_converts_hanja` 가 `true` 여도
    /// **규칙을 하나도 내지 않는다** — `lang1`/`lang2` 의 virtual keycode 가
    /// `docs/spec/korean-input.md` §3.2 기준 `(미확정)`이기 때문이다(D-K8). 명세 §8
    /// 수용 기준: "설정 파일에 켜져 있어도 발화하지 않는다."
    //
    // 2단계(F-16.2 한/영 · F-16.3 한자)가 이 아래에 얹힌다.
    pub fn to_rules(&self) -> Vec<KoreanRule> {
        let mut rules = Vec::new();

        if self.shift_space_switches_input_source {
            rules.push(KoreanRule {
                id: RuleId::Korean(13),
                trigger_key: KeyCode::SPACE,
                trigger: KoreanTrigger::ShiftOnly,
                requires_korean_ime: false,
                out_keycode: KeyCode::SPACE,
                // CONTROL(0x40000) | NX_DEVICELCTLKEYMASK(0x1) — D-K7 표.
                out_flags: EventFlags(0x0004_0001),
            });
        }

        if self.won_key_types_backtick {
            rules.push(KoreanRule {
                id: RuleId::Korean(16),
                trigger_key: KeyCode::ANSI_GRAVE,
                trigger: KoreanTrigger::NoModifier,
                requires_korean_ime: true,
                out_keycode: KeyCode::ANSI_GRAVE,
                // ALTERNATE(0x80000) | NX_DEVICELALTKEYMASK(0x20) — D-K7 표.
                out_flags: EventFlags(0x0008_0020),
            });
        }

        rules.sort_by_key(|r| r.id);
        rules
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 명세 §4.2 각주 — `disable_in_remote_desktop` 만 `true`, 나머지는 전부 `false`.
    #[test]
    fn default_only_flips_disable_in_remote_desktop() {
        let s = KoreanSettings::default();
        assert!(!s.shift_space_switches_input_source);
        assert!(!s.han_eng_switches_input_source);
        assert!(!s.hanja_key_converts_hanja);
        assert!(!s.won_key_types_backtick);
        assert!(s.disable_in_remote_desktop);
    }

    #[test]
    fn from_store_on_empty_store_matches_default() {
        let store = SettingsStore::in_memory();
        assert_eq!(KoreanSettings::from_store(&store), KoreanSettings::default());
    }

    /// ⚠️ 부재 = true 반전의 핵심 회귀 테스트 — 다른 필드를 하나 건드려도
    /// `disable_in_remote_desktop` 는 여전히 부재이므로 `true` 로 읽혀야 한다.
    #[test]
    fn from_store_defaults_disable_in_remote_desktop_to_true_when_absent() {
        let mut store = SettingsStore::in_memory();
        store
            .set(keys::KOREAN_WON_KEY_TYPES_BACKTICK, &true)
            .expect("in-memory store 는 실패하지 않는다");

        let s = KoreanSettings::from_store(&store);
        assert!(s.won_key_types_backtick);
        assert!(s.disable_in_remote_desktop, "부재 상태의 항목 5 는 true 로 읽혀야 한다");
    }

    /// 사용자가 명시적으로 항목 5 를 끄면 그 값을 그대로 읽는다.
    #[test]
    fn from_store_reads_explicit_false_for_disable_in_remote_desktop() {
        let mut store = SettingsStore::in_memory();
        store
            .set(keys::KOREAN_DISABLE_IN_REMOTE_DESKTOP, &false)
            .expect("in-memory store 는 실패하지 않는다");

        let s = KoreanSettings::from_store(&store);
        assert!(!s.disable_in_remote_desktop);
    }

    /// 기본 설정(전부 꺼짐)은 규칙을 하나도 내지 않는다.
    #[test]
    fn to_rules_is_empty_by_default() {
        assert!(KoreanSettings::default().to_rules().is_empty());
    }

    #[test]
    fn to_rules_emits_shift_space_rule_when_enabled() {
        let s = KoreanSettings {
            shift_space_switches_input_source: true,
            ..KoreanSettings::default()
        };
        let rules = s.to_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, RuleId::Korean(13));
        assert_eq!(rules[0].trigger_key, KeyCode::SPACE);
        assert_eq!(rules[0].trigger, KoreanTrigger::ShiftOnly);
        assert!(!rules[0].requires_korean_ime);
    }

    #[test]
    fn to_rules_emits_won_grave_rule_when_enabled() {
        let s = KoreanSettings {
            won_key_types_backtick: true,
            ..KoreanSettings::default()
        };
        let rules = s.to_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, RuleId::Korean(16));
        assert_eq!(rules[0].trigger_key, KeyCode::ANSI_GRAVE);
        assert_eq!(rules[0].trigger, KoreanTrigger::NoModifier);
        assert!(rules[0].requires_korean_ime);
    }

    /// 두 규칙이 동시에 켜지면 `RuleId` 오름차순(`Korean(13)` 다음 `Korean(16)`)으로
    /// 정렬돼 반환된다.
    #[test]
    fn to_rules_sorts_by_rule_id_ascending() {
        let s = KoreanSettings {
            won_key_types_backtick: true,
            shift_space_switches_input_source: true,
            ..KoreanSettings::default()
        };
        let rules = s.to_rules();
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].id, RuleId::Korean(13));
        assert_eq!(rules[1].id, RuleId::Korean(16));
    }

    /// ⭐ 명세 §8 수용 기준 — 2단계 설정을 켜도 규칙을 내지 않는다(D-K8).
    #[test]
    fn to_rules_emits_nothing_for_stage_two_settings_even_when_enabled() {
        let s = KoreanSettings {
            han_eng_switches_input_source: true,
            hanja_key_converts_hanja: true,
            ..KoreanSettings::default()
        };
        assert!(
            s.to_rules().is_empty(),
            "2단계(한/영·한자) 설정은 켜져 있어도 규칙을 내면 안 된다"
        );
    }

    /// 4개 F-16 항목이 전부 켜져도 2단계 두 항목은 여전히 규칙에 반영되지 않는다.
    #[test]
    fn to_rules_ignores_stage_two_settings_alongside_stage_one_rules() {
        let s = KoreanSettings {
            shift_space_switches_input_source: true,
            han_eng_switches_input_source: true,
            hanja_key_converts_hanja: true,
            won_key_types_backtick: true,
            disable_in_remote_desktop: true,
        };
        let rules = s.to_rules();
        assert_eq!(rules.len(), 2, "1단계 두 규칙만 나와야 한다: {rules:?}");
    }
}
