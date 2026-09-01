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
    /// F-16.2 `한/영 키로 입력 소스 변경`. 기본 ☐. §3.2 의 키코드가 근거 3중으로
    /// 해소되어(`KeyCode::JIS_KANA`, 2단계) [`KoreanSettings::to_rules`] 가 규칙을 낸다.
    #[serde(default)]
    pub han_eng_switches_input_source: bool,
    /// F-16.3 `한자 키로 한자 변환`. 기본 ☐. 상동 — 규칙을 낸다(`KeyCode::JIS_EISU`).
    #[serde(default)]
    pub hanja_key_converts_hanja: bool,
    /// F-16.4 `₩ 키로 백틱(\`) 입력`. 기본 ☐.
    #[serde(default)]
    pub won_key_types_backtick: bool,
    /// 항목 5 `원격 데스크톱 클라이언트에서 한국어 키 처리 끄기`.
    /// ⚠️ **기본값 `true`** — F-16 항목 중 유일하게 부재가 켜짐을 뜻한다(명세 §4.2 각주).
    #[serde(default = "default_disable_in_remote_desktop")]
    pub disable_in_remote_desktop: bool,
    /// ⭐ K9(이슈 #73, D-K18) — `modifier 키와 함께 누른 문자 키를 영어 소문자로
    /// 입력`. 기본 ☐. ⭐ 원본 SuperKey 에 없는 클론 고유 확장이다(명세 §3.7) —
    /// 이 필드가 규칙을 내는 게 아니라 중재기(KoreanInput 계층 앞)의 **행동 옵션**이라
    /// `to_rules` 가 아니라 `EngineConfig` 쪽 플래그로 흘러간다.
    #[serde(default)]
    pub modifier_key_types_lowercase: bool,
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
            modifier_key_types_lowercase: false,
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
            modifier_key_types_lowercase: store
                .get(keys::KOREAN_MODIFIER_KEY_TYPES_LOWERCASE)
                .unwrap_or_default(),
        }
    }

    /// 켜진 F-16 규칙을 `RuleId` 오름차순으로 정렬해 반환한다.
    ///
    /// ⭐ **2단계**: F-16.2(`Korean(14)` 한/영)·F-16.3(`Korean(15)` 한자)가 여기 얹혔다
    /// (D-K14) — §3.2 의 키코드가 근거 3중으로 해소되어(`docs/spec/korean-input.md`
    /// §3.2) 더 이상 착수를 막지 않는다.
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

        // F-16.2 한/영 — D-K14. ⭐ `requires_korean_ime = false`. 한/영은 *입력
        // 소스를 바꾸는* 키다. IME 활성을 요구하면 한국어→영문 전환만 되고, 영문
        // 상태에서는 게이트가 거짓이라 발화하지 않아 한국어로 되돌아올 수 없는
        // **편도 키**가 된다(명세 §3.1). 조건은 modifier 부재 + 앱 제외뿐이다.
        if self.han_eng_switches_input_source {
            rules.push(KoreanRule {
                id: RuleId::Korean(14),
                trigger_key: KeyCode::JIS_KANA,
                trigger: KoreanTrigger::NoModifier,
                requires_korean_ime: false,
                out_keycode: KeyCode::SPACE,
                // CONTROL(0x40000) | NX_DEVICELCTLKEYMASK(0x1) — F-16.1 과 동일 출력.
                out_flags: EventFlags(0x0004_0001),
            });
        }

        // F-16.3 한자 — D-K14. ⭐ 출력이 `return` 이라 오발 시 폼 제출·의도치 않은
        // 줄바꿈 같은 되돌리기 어려운 부작용이 난다(명세 §3.1) — 그래서 modifier
        // 부재와 한국어 IME 활성 조건을 **둘 다** 요구한다(`requires_korean_ime = true`).
        if self.hanja_key_converts_hanja {
            rules.push(KoreanRule {
                id: RuleId::Korean(15),
                trigger_key: KeyCode::JIS_EISU,
                trigger: KoreanTrigger::NoModifier,
                requires_korean_ime: true,
                out_keycode: KeyCode::RETURN,
                // ALTERNATE(0x80000) | NX_DEVICERALTKEYMASK(0x40) — D-K14 표.
                out_flags: EventFlags(0x0008_0040),
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

    /// 네 규칙이 동시에 켜지면 `RuleId` 오름차순(`Korean(13)` → `Korean(14)` →
    /// `Korean(15)` → `Korean(16)`)으로 정렬돼 반환된다.
    #[test]
    fn to_rules_sorts_by_rule_id_ascending() {
        let s = KoreanSettings {
            won_key_types_backtick: true,
            shift_space_switches_input_source: true,
            han_eng_switches_input_source: true,
            hanja_key_converts_hanja: true,
            ..KoreanSettings::default()
        };
        let rules = s.to_rules();
        assert_eq!(rules.len(), 4);
        assert_eq!(rules[0].id, RuleId::Korean(13));
        assert_eq!(rules[1].id, RuleId::Korean(14));
        assert_eq!(rules[2].id, RuleId::Korean(15));
        assert_eq!(rules[3].id, RuleId::Korean(16));
    }

    /// ⭐ 2단계(D-K14) — §3.2 의 키코드가 해소되어(`docs/spec/korean-input.md` §3.2)
    /// 이제는 켜면 규칙을 낸다. 1단계 때 이 자리에 있던 "규칙을 내지 않는다" 테스트는
    /// 뜻이 뒤집혔다 — 착수를 막던 전제(D-K8)가 근거 3중으로 해소됐기 때문이다.
    #[test]
    fn to_rules_emits_stage_two_rules_when_enabled() {
        let s = KoreanSettings {
            han_eng_switches_input_source: true,
            hanja_key_converts_hanja: true,
            ..KoreanSettings::default()
        };
        let rules = s.to_rules();
        assert_eq!(rules.len(), 2, "2단계(한/영·한자) 두 규칙이 나와야 한다: {rules:?}");

        assert_eq!(rules[0].id, RuleId::Korean(14));
        assert_eq!(rules[0].trigger_key, KeyCode::JIS_KANA);
        assert_eq!(rules[0].trigger, KoreanTrigger::NoModifier);
        // ⭐ 한/영은 입력 소스를 바꾸는 키라 IME 조건을 걸면 편도 키가 된다(D-K14).
        assert!(!rules[0].requires_korean_ime);
        assert_eq!(rules[0].out_keycode, KeyCode::SPACE);

        assert_eq!(rules[1].id, RuleId::Korean(15));
        assert_eq!(rules[1].trigger_key, KeyCode::JIS_EISU);
        assert_eq!(rules[1].trigger, KoreanTrigger::NoModifier);
        // ⭐ 한자는 출력이 return 이라 되돌리기 어렵다 — IME 조건을 반드시 건다(D-K14).
        assert!(rules[1].requires_korean_ime);
        assert_eq!(rules[1].out_keycode, KeyCode::RETURN);
    }

    /// 4개 F-16 항목이 전부 켜지면 네 규칙 전부가 나온다(F-16.1~F-16.4).
    #[test]
    fn to_rules_emits_all_four_rules_when_everything_is_enabled() {
        let s = KoreanSettings {
            shift_space_switches_input_source: true,
            han_eng_switches_input_source: true,
            hanja_key_converts_hanja: true,
            won_key_types_backtick: true,
            disable_in_remote_desktop: true,
            modifier_key_types_lowercase: false,
        };
        let rules = s.to_rules();
        assert_eq!(rules.len(), 4, "F-16.1~F-16.4 네 규칙 전부가 나와야 한다: {rules:?}");
    }

    // ── ⭐ K9(이슈 #73, D-K18) — modifier+문자키 영어 소문자 변환 ─────────────────

    /// K9 옵션은 기본 꺼짐이다(부재 = `false`).
    #[test]
    fn k9_defaults_to_off() {
        assert!(!KoreanSettings::default().modifier_key_types_lowercase);
        let store = SettingsStore::in_memory();
        assert!(!KoreanSettings::from_store(&store).modifier_key_types_lowercase);
    }

    /// K9 옵션을 켜면 저장소에서 `true` 로 읽힌다.
    #[test]
    fn k9_reads_explicit_true() {
        let mut store = SettingsStore::in_memory();
        store
            .set(keys::KOREAN_MODIFIER_KEY_TYPES_LOWERCASE, &true)
            .expect("in-memory store 는 실패하지 않는다");
        assert!(KoreanSettings::from_store(&store).modifier_key_types_lowercase);
    }

    /// ⭐ K9 는 규칙 테이블에 영향을 주지 않는다 — 켜도 `to_rules()` 결과가 같다.
    /// 이 옵션은 중재기의 행동 플래그(D-K18)이지 규칙이 아니기 때문이다.
    #[test]
    fn k9_does_not_change_to_rules() {
        let off = KoreanSettings {
            shift_space_switches_input_source: true,
            ..KoreanSettings::default()
        };
        let on = KoreanSettings {
            modifier_key_types_lowercase: true,
            ..off
        };
        assert_eq!(off.to_rules(), on.to_rules());
    }
}
