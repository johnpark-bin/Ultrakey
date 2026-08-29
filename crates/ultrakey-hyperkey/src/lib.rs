//! `ultrakey-hyperkey` — F-05 Hyperkey(`hyperkey.md`)의 **규칙 정의** 크레이트.
//!
//! ⭐ 이 크레이트는 이벤트를 가로채는 메커니즘을 스스로 갖지 않는다(그것은 F-07
//! `ultrakey-engine` 의 일이다, `hyperkey.md` §1). 여기서 하는 일은 딱 하나 — Hyperkey
//! 탭의 설정값을 `ultrakey_core::rules::ModifierRule` 목록으로 번역하는 것뿐이다.
//! 그래서 macOS 에 전혀 의존하지 않고, `unsafe` 도 전혀 필요 없다.

#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::SourceKey;
use ultrakey_core::rules::{ModifierKind, ModifierRule};
use ultrakey_core::settings::MouseApply;

/// hyper/meh/bleh 각각의 "체크박스 + 소스 키 팝업" 한 쌍(`hyperkey.md` §4 항목 1·3·4).
///
/// ⭐ "끔"의 표현은 팝업의 `-` 가 아니라 이 체크박스다(§5 항목 2 — Hyperkey 팝업 35종에는
/// `-`(미설정)가 없다. `Seek` 팝업과 달리 소스 키는 항상 어떤 값이든 갖고 있고, `enabled`
/// 가 그 값을 실제로 쓸지 결정한다).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SlotSettings {
    /// 실측 기본값 ☐ = false.
    #[serde(default)]
    pub enabled: bool,
    /// 실측 기본값 `caps lock` — 체크박스가 꺼져 있어도 팝업 값은 유지된다(UI 는 값을
    /// 지우지 않고 dimmed 상태로 보존한다는 것이 실측된 동작이다).
    #[serde(default = "default_source_key")]
    pub source: SourceKey,
}

impl Default for SlotSettings {
    fn default() -> Self {
        SlotSettings {
            enabled: false,
            source: SourceKey::CapsLock,
        }
    }
}

fn default_source_key() -> SourceKey {
    SourceKey::CapsLock
}

fn default_include_shift_in_hyper() -> bool {
    true
}

/// Hyperkey 탭의 설정 전체(`hyperkey.md` §4 의 8개 항목 중, 트랙패드 관련 3개는 `F-06`
/// 소관이라 여기서 다루지 않는다 — §3.5 "소유 판정" 참고).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HyperkeySettings {
    #[serde(default)]
    pub hyper: SlotSettings,
    /// ⭐ hyper 에만 적용된다(§3.1 — meh·bleh 는 shift 를 끌 수 있는 옵션이 없다).
    /// 실측 기본값 ☑ = true.
    #[serde(default = "default_include_shift_in_hyper")]
    pub include_shift_in_hyper: bool,
    #[serde(default)]
    pub meh: SlotSettings,
    #[serde(default)]
    pub bleh: SlotSettings,
    /// §3.3 — 실측 기본값은 `Click` 만 ON.
    #[serde(default)]
    pub mouse_apply: MouseApply,
}

impl Default for HyperkeySettings {
    fn default() -> Self {
        HyperkeySettings {
            hyper: SlotSettings::default(),
            include_shift_in_hyper: true,
            meh: SlotSettings::default(),
            bleh: SlotSettings::default(),
            mouse_apply: MouseApply::default(),
        }
    }
}

/// 설정 검증 결과. 저장 자체는 막지 않는다(§3-b "동일 소스 키 중복 배정" 결정 — 런타임
/// 우선순위가 1차 방어선이고, 이 경고는 그 위에 얹는 2차 방어선인 UI 알림용이다).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettingsWarning {
    /// 같은 소스 키가 hyper/meh/bleh 중 둘 이상에 배정됨(§5 항목 1, §8).
    DuplicateSourceKey {
        source: SourceKey,
        kinds: Vec<ModifierKind>,
    },
    /// 소스 키의 물리 keycode 를 알 수 없다(`F21`~`F24`, `menu (PC)`) — 규칙을 만들 수 없다.
    UnknownKeycode {
        source: SourceKey,
        kind: ModifierKind,
    },
}

impl HyperkeySettings {
    /// hyper 의 현재 조합 마스크.
    ///
    /// ⭐ `include_shift_in_hyper` 는 별도 필드로 저장되지만, 원본 SuperKey 는 이를
    /// **같은 정수 안의 shift 비트**로 환원해 `hyperFlags` 하나로 저장한다(실측:
    /// `hyperFlags = 1966080`, §3.1). 이 함수가 그 환원을 수행한다 — 반환값은 항상
    /// 단일 비트마스크이지 별도 불리언 조합이 아니다.
    pub fn hyper_flags(&self) -> EventFlags {
        if self.include_shift_in_hyper {
            EventFlags::HYPER_WITH_SHIFT
        } else {
            EventFlags::HYPER_NO_SHIFT
        }
    }

    /// meh 는 항상 `⌃⌥⇧` 고정(§3.1 — shift 포함 여부를 끌 수 있는 옵션이 없다).
    pub fn meh_flags(&self) -> EventFlags {
        EventFlags::MEH
    }

    /// bleh 는 항상 `⌃⌘⇧` 고정. ⭐ option 은 어떤 경우에도 포함되지 않는다
    /// (v1.65 회귀 방지 — `EventFlags::BLEH` 자체가 이미 그렇게 정의되어 있다).
    pub fn bleh_flags(&self) -> EventFlags {
        EventFlags::BLEH
    }

    /// ⭐ 이 크레이트의 본체 — 설정을 엔진이 소비하는 규칙 목록으로 번역한다.
    ///
    /// - `enabled == false` 인 슬롯은 건너뛴다.
    /// - `SourceKey::keycode()` 가 `None` 인 슬롯(F21~F24, menu (PC))은 규칙을 만들지
    ///   않는다 — 조용히 무시하는 것이 아니라 `validate()` 가 `UnknownKeycode` 로 알린다.
    /// - hyper → meh → bleh 순서로 결정론적으로 규칙을 만든다. 동일 소스 키가 둘 이상에
    ///   배정된 경우 `RuleTable::modifier_rule_for` 가 "처음 등록된 것"을 고르므로, 이
    ///   순서가 곧 우선순위다: **hyper 가 이긴다.** hyper 는 세 조합 중 modifier 개수가
    ///   가장 많아(4개) 다른 앱 단축키와 충돌할 여지가 가장 크므로, 실수로 중복 배정했을
    ///   때 "아무 동작도 안 한다"보다는 "hyper 로 동작한다"는 결과가 사용자에게 덜
    ///   놀랍다는 판단이다(명세가 직접 정하지 않은 부분이라 이 크레이트가 내린 판단임을
    ///   명시한다 — §5 엣지 케이스 1, §8 수용 기준).
    pub fn to_modifier_rules(&self) -> Vec<ModifierRule> {
        let slots: [(ModifierKind, &SlotSettings, EventFlags); 3] = [
            (ModifierKind::Hyper, &self.hyper, self.hyper_flags()),
            (ModifierKind::Meh, &self.meh, self.meh_flags()),
            (ModifierKind::Bleh, &self.bleh, self.bleh_flags()),
        ];

        slots
            .into_iter()
            .filter(|(_, slot, _)| slot.enabled)
            .filter_map(|(kind, slot, flags)| {
                slot.source.keycode().map(|source| ModifierRule {
                    source,
                    kind,
                    flags,
                })
            })
            .collect()
    }

    /// 설정 검증. 저장은 막지 않되 무엇이 문제인지는 알려준다.
    pub fn validate(&self) -> Vec<SettingsWarning> {
        let slots: [(ModifierKind, &SlotSettings); 3] = [
            (ModifierKind::Hyper, &self.hyper),
            (ModifierKind::Meh, &self.meh),
            (ModifierKind::Bleh, &self.bleh),
        ];

        let mut warnings = Vec::new();

        // keycode 를 알 수 없는 슬롯 — 활성화된 것만 의미가 있다. 꺼진 슬롯은
        // to_modifier_rules() 가 어차피 건너뛰므로 경고할 이유가 없다.
        for (kind, slot) in slots {
            if slot.enabled && slot.source.keycode().is_none() {
                warnings.push(SettingsWarning::UnknownKeycode {
                    source: slot.source,
                    kind,
                });
            }
        }

        // 동일 소스 키 중복 배정 — `SourceKey::all()` 순서로 순회해 결과가 결정론적이다.
        for &source in SourceKey::all() {
            let kinds: Vec<ModifierKind> = slots
                .iter()
                .filter(|(_, slot)| slot.enabled && slot.source == source)
                .map(|(kind, _)| *kind)
                .collect();
            if kinds.len() >= 2 {
                warnings.push(SettingsWarning::DuplicateSourceKey { source, kinds });
            }
        }

        warnings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultrakey_core::keycode::KeyCode;

    fn slot(enabled: bool, source: SourceKey) -> SlotSettings {
        SlotSettings { enabled, source }
    }

    // 1. hyper_flags() — include_shift_in_hyper: true → HYPER_WITH_SHIFT (= 1966080).
    #[test]
    fn hyper_flags_with_shift_matches_measured_value() {
        let settings = HyperkeySettings {
            include_shift_in_hyper: true,
            ..HyperkeySettings::default()
        };
        assert_eq!(settings.hyper_flags(), EventFlags::HYPER_WITH_SHIFT);
        assert_eq!(settings.hyper_flags().0, 1_966_080);
    }

    // 2. hyper_flags() — include_shift_in_hyper: false → ⌃⌥⌘ 3개만, shift 비트 없음.
    #[test]
    fn hyper_flags_without_shift_excludes_shift_bit() {
        let settings = HyperkeySettings {
            include_shift_in_hyper: false,
            ..HyperkeySettings::default()
        };
        let flags = settings.hyper_flags();
        assert_eq!(flags, EventFlags::HYPER_NO_SHIFT);
        assert!(!flags.contains(EventFlags::SHIFT));
        assert!(flags.contains(EventFlags::CONTROL));
        assert!(flags.contains(EventFlags::ALTERNATE));
        assert!(flags.contains(EventFlags::COMMAND));
    }

    // 3. meh_flags() → ⌃⌥⇧, command 비트 없음.
    #[test]
    fn meh_flags_excludes_command() {
        let settings = HyperkeySettings::default();
        let flags = settings.meh_flags();
        assert_eq!(flags, EventFlags::MEH);
        assert!(!flags.contains(EventFlags::COMMAND));
        assert!(flags.contains(EventFlags::CONTROL));
        assert!(flags.contains(EventFlags::ALTERNATE));
        assert!(flags.contains(EventFlags::SHIFT));
    }

    // 4. bleh_flags() → ⌃⌘⇧, option 비트 없음(v1.65 회귀 방지).
    #[test]
    fn bleh_flags_excludes_option() {
        let settings = HyperkeySettings::default();
        let flags = settings.bleh_flags();
        assert_eq!(flags, EventFlags::BLEH);
        assert!(!flags.contains(EventFlags::ALTERNATE));
        assert!(flags.contains(EventFlags::CONTROL));
        assert!(flags.contains(EventFlags::COMMAND));
        assert!(flags.contains(EventFlags::SHIFT));
    }

    // 5. include_shift_in_hyper 를 꺼도 meh·bleh 의 shift 는 영향받지 않는다(§3.1).
    #[test]
    fn include_shift_in_hyper_does_not_affect_meh_or_bleh() {
        let settings = HyperkeySettings {
            include_shift_in_hyper: false,
            ..HyperkeySettings::default()
        };
        assert!(settings.meh_flags().contains(EventFlags::SHIFT));
        assert!(settings.bleh_flags().contains(EventFlags::SHIFT));
    }

    // 6. to_modifier_rules() — 전부 비활성(기본값)이면 빈 목록.
    #[test]
    fn to_modifier_rules_empty_when_all_disabled() {
        let settings = HyperkeySettings::default();
        assert!(settings.to_modifier_rules().is_empty());
    }

    // 7. to_modifier_rules() — hyper 만 켜면 규칙 1개.
    #[test]
    fn to_modifier_rules_hyper_only() {
        let settings = HyperkeySettings {
            hyper: slot(true, SourceKey::CapsLock),
            ..HyperkeySettings::default()
        };
        let rules = settings.to_modifier_rules();
        assert_eq!(rules.len(), 1);
        assert_eq!(rules[0].source, KeyCode::CAPS_LOCK);
        assert_eq!(rules[0].kind, ModifierKind::Hyper);
        assert_eq!(rules[0].flags, EventFlags::HYPER_WITH_SHIFT);
    }

    // 8. to_modifier_rules() — 셋 다 서로 다른 소스 키로 켜면 규칙 3개.
    #[test]
    fn to_modifier_rules_all_three_distinct_sources() {
        let settings = HyperkeySettings {
            hyper: slot(true, SourceKey::CapsLock),
            meh: slot(true, SourceKey::RightOption),
            bleh: slot(true, SourceKey::LeftControl),
            ..HyperkeySettings::default()
        };
        let rules = settings.to_modifier_rules();
        assert_eq!(rules.len(), 3);
    }

    // 9. validate() — 같은 소스 키를 hyper 와 meh 에 배정하면 DuplicateSourceKey.
    #[test]
    fn validate_reports_duplicate_source_key() {
        let settings = HyperkeySettings {
            hyper: slot(true, SourceKey::CapsLock),
            meh: slot(true, SourceKey::CapsLock),
            ..HyperkeySettings::default()
        };
        let warnings = settings.validate();
        assert!(warnings.iter().any(|w| matches!(
            w,
            SettingsWarning::DuplicateSourceKey { source, kinds }
                if *source == SourceKey::CapsLock
                    && kinds.contains(&ModifierKind::Hyper)
                    && kinds.contains(&ModifierKind::Meh)
        )));
    }

    // 10. 중복 배정 시 to_modifier_rules() 가 비결정적이지 않다 — hyper 가 이긴다.
    #[test]
    fn duplicate_assignment_resolves_deterministically_to_hyper() {
        use ultrakey_core::rules::RuleTable;

        let settings = HyperkeySettings {
            hyper: slot(true, SourceKey::CapsLock),
            meh: slot(true, SourceKey::CapsLock),
            ..HyperkeySettings::default()
        };

        let first = settings.to_modifier_rules();
        let second = settings.to_modifier_rules();
        assert_eq!(first, second);

        let table = RuleTable {
            modifier_rules: first,
            ..RuleTable::default()
        };
        let winner = table.modifier_rule_for(KeyCode::CAPS_LOCK).unwrap();
        assert_eq!(winner.kind, ModifierKind::Hyper);
    }

    // 11. validate() — keycode 가 None 인 소스 키는 UnknownKeycode, to_modifier_rules 제외.
    #[test]
    fn unknown_keycode_source_is_reported_and_excluded_from_rules() {
        let settings = HyperkeySettings {
            hyper: slot(true, SourceKey::F21),
            ..HyperkeySettings::default()
        };

        let warnings = settings.validate();
        assert!(warnings.iter().any(|w| matches!(
            w,
            SettingsWarning::UnknownKeycode { source, kind }
                if *source == SourceKey::F21 && *kind == ModifierKind::Hyper
        )));

        assert!(settings.to_modifier_rules().is_empty());
    }

    // 12. serde: 빈 JSON "{}" 은 HyperkeySettings::default() 와 같다.
    #[test]
    fn empty_json_deserializes_to_default() {
        let parsed: HyperkeySettings = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, HyperkeySettings::default());
    }

    // 13. serde 왕복.
    #[test]
    fn serde_round_trip() {
        let settings = HyperkeySettings {
            hyper: slot(true, SourceKey::CapsLock),
            include_shift_in_hyper: false,
            meh: slot(true, SourceKey::RightOption),
            bleh: slot(false, SourceKey::F5),
            mouse_apply: MouseApply {
                click: true,
                drag: true,
                r#move: false,
                scroll: true,
            },
        };
        let json = serde_json::to_string(&settings).unwrap();
        let parsed: HyperkeySettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, settings);
    }

    // 14. Default 가 §4 실측 기본값과 일치.
    #[test]
    fn default_matches_measured_defaults() {
        let settings = HyperkeySettings::default();
        assert!(!settings.hyper.enabled);
        assert_eq!(settings.hyper.source, SourceKey::CapsLock);
        assert!(settings.include_shift_in_hyper);
        assert!(!settings.meh.enabled);
        assert_eq!(settings.meh.source, SourceKey::CapsLock);
        assert!(!settings.bleh.enabled);
        assert_eq!(settings.bleh.source, SourceKey::CapsLock);
        assert_eq!(settings.mouse_apply, MouseApply::default());
        assert!(settings.mouse_apply.click);
        assert!(!settings.mouse_apply.drag);
        assert!(!settings.mouse_apply.r#move);
        assert!(!settings.mouse_apply.scroll);
    }

#[test]
fn serde_uses_stable_variant_names_not_ui_labels() {
    // ⭐ 저장 형식이 UI 표시 문자열에 결합되지 않았음을 못박는다.
    // `SourceKey::label()` 은 "caps lock" 이지만 저장은 variant 이름이어야 한다.
    let s = HyperkeySettings::default();
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("CapsLock"), "variant 이름으로 저장되어야 한다: {json}");
    assert!(!json.contains("caps lock"), "UI 라벨이 저장 형식에 새어 나왔다: {json}");
}

}
