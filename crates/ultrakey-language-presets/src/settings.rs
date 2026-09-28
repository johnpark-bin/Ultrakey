//! `日本語`·`中文` 노드의 설정값 — `docs/spec/language-presets.md` §3.2·§3.3.
//!
//! 저장 키는 `japanese.*`·`chinese.*` 네임스페이스(명세 §4.3). **전부 "부재 = 기본값(☐)"**
//! — 기본이 ☑ 인 항목은 없다(F-16 §4.2 반전 주의는 해당 없음).

use serde::{Deserialize, Serialize};

use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::KeyCode;
use ultrakey_core::rules::{LanguageGate, LanguageOut, LanguageRule, LanguageTrigger, RuleId};

/// `日本語` 노드 전체 — F-19.3~F-19.6 체크박스 4개. 기본값 전부 ☐(전 bool false — derive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct JapaneseSettings {
    /// F-19.3 `캡스락 = 英数/かな 토글`. 기본 ☐.
    #[serde(default)]
    pub caps_lock_toggles_eisu_kana: bool,
    /// F-19.4 `⌘ 단독 탭 = 英数/かな`. 기본 ☐. 좌⌘=英数·우⌘=かな 고정(P2 확정 안 A) —
    /// 하나의 값이 규칙 두 개를 만든다.
    #[serde(default)]
    pub command_toggles_eisu_kana: bool,
    /// F-19.5 `¥ ↔ \ / 백틱 치환`. 기본 ☐.
    #[serde(default)]
    pub swap_yen_backslash: bool,
    /// F-19.6 `JIS 키보드를 US 배열처럼`. 기본 ☐. 심볼 치환 20행.
    #[serde(default)]
    pub jis_as_us_symbols: bool,
}

/// `中文` 노드 전체 — F-19.7 체크박스 1개. 기본값 ☐(derive).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ChineseSettings {
    /// F-19.7 `캡스락 = 중/영 전환`. 기본 ☐.
    #[serde(default)]
    pub caps_lock_switches_input_source: bool,
}

/// ⌃Space 재주입용 출력 flags — F-16.1 과 같은 값(CONTROL | NX_DEVICELCTLKEYMASK).
const CTRL_SPACE_FLAGS: EventFlags = EventFlags(0x0004_0001);
/// left_shift — `SHIFT`(0x20000) | `DEVICE_LEFT_SHIFT`(0x2). F-16.4 가
/// `0x0008_0020`(ALTERNATE | NX_DEVICELALTKEYMASK) 로 좌 option 을 표현한 것과 같은 관례.
const LEFT_SHIFT_FLAGS: EventFlags = EventFlags(0x0002_0002);
/// ⌥(option) — F-19.5 의 `\`→`⌥\` 경유 합성 및 F-19.6 행 19 의 `]`→`⌥¥`.
const OPTION_FLAGS: EventFlags = EventFlags(0x0008_0020);

impl JapaneseSettings {
    /// [`ultrakey_core::settings::SettingsStore`] 로 조립("부재 = 기본값").
    pub fn from_store(store: &ultrakey_core::settings::SettingsStore) -> JapaneseSettings {
        use ultrakey_core::settings::keys;
        JapaneseSettings {
            caps_lock_toggles_eisu_kana: store
                .get(keys::JAPANESE_CAPS_LOCK_TOGGLES_EISU_KANA)
                .unwrap_or_default(),
            command_toggles_eisu_kana: store
                .get(keys::JAPANESE_COMMAND_TOGGLES_EISU_KANA)
                .unwrap_or_default(),
            swap_yen_backslash: store
                .get(keys::JAPANESE_SWAP_YEN_BACKSLASH)
                .unwrap_or_default(),
            jis_as_us_symbols: store
                .get(keys::JAPANESE_JIS_AS_US_SYMBOLS)
                .unwrap_or_default(),
        }
    }

    /// 켜진 ja 규칙을 `RuleId` 오름차순으로 반환한다 — `Language(19)`..`Language(22)`.
    pub fn to_language_rules(&self) -> Vec<LanguageRule> {
        let mut rules = Vec::new();
        if self.caps_lock_toggles_eisu_kana {
            rules.push(LanguageRule {
                id: RuleId::Language(19),
                trigger: LanguageTrigger::AloneTap { key: KeyCode::CAPS_LOCK },
                app_gate: Some(LanguageGate::Japanese),
                requires_jis: None,
                out: LanguageOut::EisuOrKana,
            });
        }
        if self.command_toggles_eisu_kana {
            rules.push(LanguageRule {
                id: RuleId::Language(20),
                trigger: LanguageTrigger::AloneTap { key: KeyCode::LEFT_COMMAND },
                app_gate: Some(LanguageGate::Japanese),
                requires_jis: None,
                out: LanguageOut::Key { keycode: KeyCode::JIS_EISU, flags: EventFlags::NONE },
            });
            rules.push(LanguageRule {
                id: RuleId::Language(20),
                trigger: LanguageTrigger::AloneTap { key: KeyCode::RIGHT_COMMAND },
                app_gate: Some(LanguageGate::Japanese),
                requires_jis: None,
                out: LanguageOut::Key { keycode: KeyCode::JIS_KANA, flags: EventFlags::NONE },
            });
        }
        if self.swap_yen_backslash {
            rules.extend(yen_backslash_rules());
        }
        if self.jis_as_us_symbols {
            rules.extend(jis_to_us_symbol_rules());
        }
        rules.sort_by_key(|r| r.id);
        rules
    }
}

/// F-19.5 — ¥↔\ 의 정적 규칙 4개(카탈로그 `swap_yen_and_backslash{,_jis}.json` 대조):
/// JIS(0x5D) 행은 `requires_jis: Some(true)`, US(0x2A) 행은 `Some(false)` — 물리 키가
/// 키보드 타입을 가르므로 한 프리셋이 두 타입을 모두 담는다.
fn yen_backslash_rules() -> Vec<LanguageRule> {
    let mut rules = Vec::new();
    for (key, jis) in [
        (KeyCode::JIS_YEN, true),
        (KeyCode::ANSI_BACKSLASH, false),
    ] {
        // key 단독 → ⌥+key (key 는 JIS 에서 ¥, US 에서 `\`)
        rules.push(LanguageRule {
            id: RuleId::Language(21),
            trigger: LanguageTrigger::NoModifier { key },
            app_gate: None,
            requires_jis: Some(jis),
            out: LanguageOut::Key { keycode: key, flags: OPTION_FLAGS },
        });
        // ⌥+key → key (↔ 역방향)
        rules.push(LanguageRule {
            id: RuleId::Language(21),
            trigger: LanguageTrigger::OptionOnly { key },
            app_gate: None,
            requires_jis: Some(jis),
            out: LanguageOut::Key { keycode: key, flags: EventFlags::NONE },
        });
    }
    rules
}

/// F-19.6 — JIS 심볼 키 20행 → US 배열 치환(카탈로그 `jis_to_us_symbols.json`
/// 2026-09-03 fetch 대조). 행: `(from_key, shift_mandatory, to_key, to_flags)`.
///
/// ⚠️ **"international3/international1" 은 Karabiner 의 HID usage 이름**이다 — macOS
/// virtual keycode 는 각각 `JIS_YEN`(0x5D)·`JIS_UNDERSCORE`(0x5E, 로컬 SDK 헤더 실측).
///
/// ⛔ 행 10-11(¥→\`)은 F-19.5 와 **같은 소스 키(0x5D)를 주장**한다 — 충돌 감지 대상
/// (conflicts.rs). 행 19(`]`→`\`)는 `international3 + option` 경유 합성이다 — ⛔ 직접
/// `0x5D → 0x2A` 매핑이 없어야 한다(명세 §5 #10, `]` 함정). 이 표는 전부 `option`
/// 경유만 쓴다.
fn jis_to_us_symbol_rules() -> Vec<LanguageRule> {
    type Row = (KeyCode, bool, KeyCode, EventFlags);
    const ROWS: &[Row] = &[
        // shift+2 `"` → `@` — JIS 에서 `[`(0x21) 단독 = @
        (KeyCode::ANSI_2, true, KeyCode::ANSI_LEFT_BRACKET, EventFlags::NONE),
        // shift+6 `&` → `^` — `=`(0x18) 단독 = ^
        (KeyCode::ANSI_6, true, KeyCode::ANSI_EQUAL, EventFlags::NONE),
        // shift+7 `'` → `&` — shift+6 = &
        (KeyCode::ANSI_7, true, KeyCode::ANSI_6, LEFT_SHIFT_FLAGS),
        // shift+8 `(` → `*` — shift+quote = *
        (KeyCode::ANSI_8, true, KeyCode::ANSI_QUOTE, LEFT_SHIFT_FLAGS),
        // shift+9 `)` → `(` — shift+8 = (
        (KeyCode::ANSI_9, true, KeyCode::ANSI_8, LEFT_SHIFT_FLAGS),
        // shift+0 `0` → `)` — shift+9 = )
        (KeyCode::ANSI_0, true, KeyCode::ANSI_9, LEFT_SHIFT_FLAGS),
        // shift+- `=` → `_` — JIS `_`(0x5E) 단독 = _
        (KeyCode::ANSI_MINUS, true, KeyCode::JIS_UNDERSCORE, EventFlags::NONE),
        // `^` → `=` — shift+- = =
        (KeyCode::ANSI_EQUAL, false, KeyCode::ANSI_MINUS, LEFT_SHIFT_FLAGS),
        // shift+^ `~` → `+` — shift+; = +
        (KeyCode::ANSI_EQUAL, true, KeyCode::ANSI_SEMICOLON, LEFT_SHIFT_FLAGS),
        // ¥ → `` ` `` — shift+[ = `
        (KeyCode::JIS_YEN, false, KeyCode::ANSI_LEFT_BRACKET, LEFT_SHIFT_FLAGS),
        // shift+¥ `|` → `~` — shift+= = ~
        (KeyCode::JIS_YEN, true, KeyCode::ANSI_EQUAL, LEFT_SHIFT_FLAGS),
        // `@` → `[` — `]`(0x1E) 단독 = [
        (KeyCode::ANSI_LEFT_BRACKET, false, KeyCode::ANSI_RIGHT_BRACKET, EventFlags::NONE),
        // shift+@ `` ` `` → `{` — shift+] = {
        (KeyCode::ANSI_LEFT_BRACKET, true, KeyCode::ANSI_RIGHT_BRACKET, LEFT_SHIFT_FLAGS),
        // `[` → `]` — `\`(0x2A) 단독 = ]
        (KeyCode::ANSI_RIGHT_BRACKET, false, KeyCode::ANSI_BACKSLASH, EventFlags::NONE),
        // shift+[ `{` → `}` — shift+\ = }
        (KeyCode::ANSI_RIGHT_BRACKET, true, KeyCode::ANSI_BACKSLASH, LEFT_SHIFT_FLAGS),
        // shift+; `+` → `:` — quote(0x27) 단독 = :
        (KeyCode::ANSI_SEMICOLON, true, KeyCode::ANSI_QUOTE, EventFlags::NONE),
        // `:` → `'` — shift+7 = '
        (KeyCode::ANSI_QUOTE, false, KeyCode::ANSI_7, LEFT_SHIFT_FLAGS),
        // shift+: `*` → `"` — shift+2 = "
        (KeyCode::ANSI_QUOTE, true, KeyCode::ANSI_2, LEFT_SHIFT_FLAGS),
        // `]` → `\` — ⛔ ⌥+¥ 경유 합성(직접 0x2A 불가, 명세 §5 #10)
        (KeyCode::ANSI_BACKSLASH, false, KeyCode::JIS_YEN, OPTION_FLAGS),
        // shift+] `}` → `|` — shift+¥ = |
        (KeyCode::ANSI_BACKSLASH, true, KeyCode::JIS_YEN, LEFT_SHIFT_FLAGS),
    ];

    ROWS.iter()
        .map(|&(from, shift, to, flags)| LanguageRule {
            id: RuleId::Language(22),
            trigger: if shift {
                LanguageTrigger::ShiftOnly { key: from }
            } else {
                LanguageTrigger::NoModifier { key: from }
            },
            app_gate: None,
            // 전 행 `keyboard_type_if [jis]`(카탈로그 원문) — JIS 에서만.
            requires_jis: Some(true),
            out: LanguageOut::Key { keycode: to, flags },
        })
        .collect()
}

impl ChineseSettings {
    /// [`ultrakey_core::settings::SettingsStore`] 로 조립("부재 = 기본값").
    pub fn from_store(store: &ultrakey_core::settings::SettingsStore) -> ChineseSettings {
        use ultrakey_core::settings::keys;
        ChineseSettings {
            caps_lock_switches_input_source: store
                .get(keys::CHINESE_CAPS_LOCK_SWITCHES_INPUT_SOURCE)
                .unwrap_or_default(),
        }
    }

    /// F-19.7 캡스락 단독 탭 → ⌃Space. `Language(23)`.
    pub fn to_language_rules(&self) -> Vec<LanguageRule> {
        if !self.caps_lock_switches_input_source {
            return Vec::new();
        }
        vec![LanguageRule {
            id: RuleId::Language(23),
            trigger: LanguageTrigger::AloneTap { key: KeyCode::CAPS_LOCK },
            app_gate: Some(LanguageGate::Chinese),
            requires_jis: None,
            out: LanguageOut::Key { keycode: KeyCode::SPACE, flags: CTRL_SPACE_FLAGS },
        }]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultrakey_core::settings::SettingsStore;

    #[test]
    fn default_is_off() {
        assert!(!JapaneseSettings::default().caps_lock_toggles_eisu_kana);
        assert!(!ChineseSettings::default().caps_lock_switches_input_source);
    }

    #[test]
    fn from_store_on_empty_store_matches_default() {
        let store = SettingsStore::in_memory();
        assert_eq!(JapaneseSettings::from_store(&store), JapaneseSettings::default());
        assert_eq!(ChineseSettings::from_store(&store), ChineseSettings::default());
    }

    #[test]
    fn to_rules_empty_when_everything_off() {
        assert!(JapaneseSettings::default().to_language_rules().is_empty());
        assert!(ChineseSettings::default().to_language_rules().is_empty());
    }

    #[test]
    fn f193_caps_lock_emits_eisu_or_kana() {
        let s = JapaneseSettings { caps_lock_toggles_eisu_kana: true, ..Default::default() };
        let rules = s.to_language_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, RuleId::Language(19));
        assert_eq!(rules[0].trigger, LanguageTrigger::AloneTap { key: KeyCode::CAPS_LOCK });
        assert_eq!(rules[0].app_gate, Some(LanguageGate::Japanese));
        assert_eq!(rules[0].out, LanguageOut::EisuOrKana);
    }

    /// ⭐ 안 A — 저장 키 하나가 규칙 두 개(좌⌘=英数, 우⌘=かな).
    #[test]
    fn f194_command_emits_left_eisuu_right_kana() {
        let s = JapaneseSettings { command_toggles_eisu_kana: true, ..Default::default() };
        let rules = s.to_language_rules();
        assert_eq!(rules.len(), 2);
        assert!(rules.iter().all(|r| r.id == RuleId::Language(20)));
        assert!(rules
            .iter()
            .any(|r| r.trigger == LanguageTrigger::AloneTap { key: KeyCode::LEFT_COMMAND }
                && r.out == LanguageOut::Key { keycode: KeyCode::JIS_EISU, flags: EventFlags::NONE }));
        assert!(rules
            .iter()
            .any(|r| r.trigger == LanguageTrigger::AloneTap { key: KeyCode::RIGHT_COMMAND }
                && r.out == LanguageOut::Key { keycode: KeyCode::JIS_KANA, flags: EventFlags::NONE }));
    }

    #[test]
    fn f195_yen_swap_emits_4_rules() {
        let s = JapaneseSettings { swap_yen_backslash: true, ..Default::default() };
        let rules = s.to_language_rules();
        assert_eq!(rules.len(), 4);
        assert!(rules.iter().all(|r| r.id == RuleId::Language(21)));
        // JIS 행 — 0x5D 단독 → ⌥0x5D
        assert!(rules.iter().any(|r| r.trigger == LanguageTrigger::NoModifier { key: KeyCode::JIS_YEN }
            && r.requires_jis == Some(true)
            && r.out == LanguageOut::Key { keycode: KeyCode::JIS_YEN, flags: OPTION_FLAGS }));
        // US 행 — 0x2A 단독 → ⌥0x2A (JIS 아님)
        assert!(rules.iter().any(|r| r.trigger == LanguageTrigger::NoModifier { key: KeyCode::ANSI_BACKSLASH }
            && r.requires_jis == Some(false)));
        // 역방향 ⌥+key → key
        assert!(rules.iter().any(|r| r.trigger == LanguageTrigger::OptionOnly { key: KeyCode::ANSI_BACKSLASH }
            && r.out == LanguageOut::Key { keycode: KeyCode::ANSI_BACKSLASH, flags: EventFlags::NONE }));
    }

    /// ⭐ 수용 기준 — F-19.6 은 전부 JIS, 전부 정적.
    #[test]
    fn f196_jis_to_us_emits_20_rules_all_jis() {
        let s = JapaneseSettings { jis_as_us_symbols: true, ..Default::default() };
        let rules = s.to_language_rules();
        assert_eq!(rules.len(), 20, "카탈로그 원문 20행과 일치해야 한다");
        assert!(rules.iter().all(|r| r.id == RuleId::Language(22)));
        assert!(rules.iter().all(|r| r.requires_jis == Some(true)));
        assert!(rules.iter().all(|r| r.app_gate.is_none()));
        // keycode 중복이 없다(한 물리 키가 두 행에 걸리지 않는다 — 결정론).
        let mut keys: Vec<_> = rules
            .iter()
            .map(|r| match r.trigger {
                LanguageTrigger::NoModifier { key } | LanguageTrigger::ShiftOnly { key } => key,
                _ => unreachable!("F-19.6 은 NoModifier/ShiftOnly 만 쓴다"),
            })
            .collect();
        keys.sort();
        keys.dedup();
        assert!(keys.len() >= 13, "20행이 13개 이상의 고유 키로 수렴해야 한다(중복 from 허용): {keys:?}");
    }

    /// ⛔ 부정형 — `international3(0x5D) → backslash(0x2A)` 직접 매핑이 없다(명세 §5 #10,
    /// `]` 함정). F-19.6 행 19 의 출력은 `0x5D + option` 경유다. 검사 대상은 **트리거가
    /// 0x5D(JIS ¥) 인 규칙만** — F-19.5 의 US 행(트리거 0x2A)이 출력 0x2A 를 내는 것은
    /// 역방향(⌥\ → \) 동작이라 정상이다.
    #[test]
    fn forbidden_direct_yen_to_backslash_is_absent() {
        let s = JapaneseSettings { swap_yen_backslash: true, jis_as_us_symbols: true, ..Default::default() };
        for r in s.to_language_rules() {
            let trigger_is_yen = matches!(
                r.trigger,
                LanguageTrigger::NoModifier { key } | LanguageTrigger::ShiftOnly { key }
                    if key == KeyCode::JIS_YEN
            );
            if !trigger_is_yen {
                continue;
            }
            if let LanguageOut::Key { keycode, flags } = r.out {
                // `\`(0x2A)를 0x5D 에서 직접(option 없이) 내면 안 된다.
                if keycode == KeyCode::ANSI_BACKSLASH {
                    assert!(!flags.is_empty(), "0x5D → 0x2A 직접 매핑은 금지: {r:?}");
                }
            }
        }
    }

    #[test]
    fn f197_chinese_caps_lock_emits_ctrl_space() {
        let s = ChineseSettings { caps_lock_switches_input_source: true };
        let rules = s.to_language_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].id, RuleId::Language(23));
        assert_eq!(rules[0].out, LanguageOut::Key { keycode: KeyCode::SPACE, flags: CTRL_SPACE_FLAGS });
        assert_eq!(rules[0].app_gate, Some(LanguageGate::Chinese));
    }

    #[test]
    fn rules_sorted_by_id() {
        let s = JapaneseSettings { caps_lock_toggles_eisu_kana: true, command_toggles_eisu_kana: true, swap_yen_backslash: true, jis_as_us_symbols: true };
        let rules = s.to_language_rules();
        let ids: Vec<_> = rules.iter().map(|r| r.id).collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(ids, sorted, "RuleId 오름차순 정렬이어야 한다");
    }
}