//! `Presets` 탭 16종의 설정값 — `docs/spec/power-user-presets.md` §4 표 그대로.
//!
//! ⭐ `ultrakey_hyperkey::HyperkeySettings::from_store` 와 **똑같은 "부재 = 기본값" 규약**을
//! 따른다(F-15 `settings-store-and-integrity.md` §3.1). 기본값은 전부 ☐ — 실측(§4 상단
//! 정정 사유)이 확정한 값이다.

use serde::{Deserialize, Serialize};

use ultrakey_core::settings::{keys, SettingsStore};

use crate::popups::{ArrowKeySet, BracketPair, HomeRowScheme, PasteTrigger, QuickPressCapsAction, RemapCapsTarget};

fn default_remap_caps_target() -> RemapCapsTarget {
    RemapCapsTarget::LeftControl
}

fn default_quick_press_caps_action() -> QuickPressCapsAction {
    QuickPressCapsAction::CapsLock
}

fn default_arrow_key_set() -> ArrowKeySet {
    ArrowKeySet::Hjkl
}

fn default_home_row_scheme() -> HomeRowScheme {
    HomeRowScheme::SymbolRow
}

fn default_bracket_pair() -> BracketPair {
    BracketPair::Parens
}

fn default_paste_trigger() -> PasteTrigger {
    PasteTrigger::RightCommand
}

fn default_quick_press_duration_ms() -> u64 {
    1000
}

/// F-08.1 `Remap caps lock to:` — 체크박스 + 팝업(50종) 한 쌍.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapsLockRemapSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_remap_caps_target")]
    pub target: RemapCapsTarget,
}

impl Default for CapsLockRemapSettings {
    fn default() -> Self {
        CapsLockRemapSettings { enabled: false, target: RemapCapsTarget::LeftControl }
    }
}

/// F-08.2 `Quick press caps lock to execute:` — 체크박스 + 팝업(48종, `QuickPressCapsAction`
/// 문서 참고) 한 쌍.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapsQuickPressSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_quick_press_caps_action")]
    pub action: QuickPressCapsAction,
}

impl Default for CapsQuickPressSettings {
    fn default() -> Self {
        CapsQuickPressSettings { enabled: false, action: QuickPressCapsAction::CapsLock }
    }
}

/// F-08.6 `Caps lock +` [팝업] ` = ◀▼▲▶` — 체크박스 + 인라인 팝업(2종) 한 쌍.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapsHjklArrowsSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_arrow_key_set")]
    pub key_set: ArrowKeySet,
}

impl Default for CapsHjklArrowsSettings {
    fn default() -> Self {
        CapsHjklArrowsSettings { enabled: false, key_set: ArrowKeySet::Hjkl }
    }
}

/// F-08.7 `Caps lock + home row = ` [팝업] — 체크박스 + 인라인 팝업(2종) 한 쌍.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapsHomeRowSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_home_row_scheme")]
    pub scheme: HomeRowScheme,
}

impl Default for CapsHomeRowSettings {
    fn default() -> Self {
        CapsHomeRowSettings { enabled: false, scheme: HomeRowScheme::SymbolRow }
    }
}

/// F-08.11 `Quick press left or right shift to input corresponding:` — 체크박스 + 팝업
/// (4종) 한 쌍.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShiftQuickPressBracketsSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_bracket_pair")]
    pub pair: BracketPair,
}

impl Default for ShiftQuickPressBracketsSettings {
    fn default() -> Self {
        ShiftQuickPressBracketsSettings { enabled: false, pair: BracketPair::Parens }
    }
}

/// F-08.15 `Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):` — 체크박스 + 팝업(4종) 한 쌍.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PasteWithoutFormattingSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_paste_trigger")]
    pub trigger: PasteTrigger,
}

impl Default for PasteWithoutFormattingSettings {
    fn default() -> Self {
        PasteWithoutFormattingSettings { enabled: false, trigger: PasteTrigger::RightCommand }
    }
}

/// `Presets` 탭 전체 — 16종 + Advanced `Synthesize Caps Lock Remap` 토글.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PresetSettings {
    /// F-08.1.
    #[serde(default)]
    pub caps_lock_remap: CapsLockRemapSettings,
    /// F-08.2.
    #[serde(default)]
    pub caps_quick_press: CapsQuickPressSettings,
    /// F-08.3 — 250~2000ms, 기본 1000ms.
    #[serde(default = "default_quick_press_duration_ms")]
    pub quick_press_duration_ms: u64,
    /// F-08.4.
    #[serde(default)]
    pub caps_space_enter: bool,
    /// F-08.5.
    #[serde(default)]
    pub caps_wasd_arrows: bool,
    /// F-08.6.
    #[serde(default)]
    pub caps_hjkl_arrows: CapsHjklArrowsSettings,
    /// F-08.7.
    #[serde(default)]
    pub caps_home_row: CapsHomeRowSettings,
    /// F-08.8.
    #[serde(default)]
    pub double_tap_shift_to_caps: bool,
    /// F-08.9.
    #[serde(default)]
    pub left_right_shift_to_caps: bool,
    /// F-08.10.
    #[serde(default)]
    pub shift_caps_to_caps: bool,
    /// F-08.11.
    #[serde(default)]
    pub shift_quick_press_brackets: ShiftQuickPressBracketsSettings,
    /// F-08.12.
    #[serde(default)]
    pub hyper_delete_to_forward: bool,
    /// F-08.13.
    #[serde(default)]
    pub delete_to_forward: bool,
    /// F-08.14.
    #[serde(default)]
    pub shift_delete_to_forward: bool,
    /// F-08.15.
    #[serde(default)]
    pub paste_without_formatting: PasteWithoutFormattingSettings,
    /// F-08.16.
    #[serde(default)]
    pub home_end_on_lines: bool,
    /// `Advanced ▸ Synthesize Caps Lock Remap` — 켜면 D-1 의 경로 B(`hidutil` 커널 매핑)를
    /// 설치하지 않고 경로 A(이벤트 합성)만 쓴다(`docs/dev/architecture.md` §6.1).
    #[serde(default)]
    pub synthesize_caps_lock_remap: bool,
}

impl Default for PresetSettings {
    fn default() -> Self {
        PresetSettings {
            caps_lock_remap: CapsLockRemapSettings::default(),
            caps_quick_press: CapsQuickPressSettings::default(),
            quick_press_duration_ms: 1000,
            caps_space_enter: false,
            caps_wasd_arrows: false,
            caps_hjkl_arrows: CapsHjklArrowsSettings::default(),
            caps_home_row: CapsHomeRowSettings::default(),
            double_tap_shift_to_caps: false,
            left_right_shift_to_caps: false,
            shift_caps_to_caps: false,
            shift_quick_press_brackets: ShiftQuickPressBracketsSettings::default(),
            hyper_delete_to_forward: false,
            delete_to_forward: false,
            shift_delete_to_forward: false,
            paste_without_formatting: PasteWithoutFormattingSettings::default(),
            home_end_on_lines: false,
            synthesize_caps_lock_remap: false,
        }
    }
}

impl PresetSettings {
    /// [`SettingsStore`]에서 필드별로 조립한다 — 키가 없는 필드는 그 필드의 기본값을
    /// 쓴다("부재 = 기본값", F-15 §3.1). `quick_press_duration_ms` 는 저장된 값이 있어도
    /// 250~2000 범위로 클램프한다(§4 F-08.3 유효 범위).
    pub fn from_store(store: &SettingsStore) -> Self {
        PresetSettings {
            caps_lock_remap: CapsLockRemapSettings {
                enabled: store.get(keys::PRESETS_CAPS_LOCK_REMAP_ENABLED).unwrap_or_default(),
                target: store
                    .get(keys::PRESETS_CAPS_LOCK_REMAP_TARGET)
                    .unwrap_or(RemapCapsTarget::LeftControl),
            },
            caps_quick_press: CapsQuickPressSettings {
                enabled: store.get(keys::PRESETS_CAPS_QUICK_PRESS_ENABLED).unwrap_or_default(),
                action: store
                    .get(keys::PRESETS_CAPS_QUICK_PRESS_ACTION)
                    .unwrap_or(QuickPressCapsAction::CapsLock),
            },
            quick_press_duration_ms: store
                .get::<u64>(keys::PRESETS_QUICK_PRESS_DURATION_MS)
                .map(|ms| ms.clamp(250, 2000))
                .unwrap_or(1000),
            caps_space_enter: store.get(keys::PRESETS_CAPS_SPACE_ENTER).unwrap_or_default(),
            caps_wasd_arrows: store.get(keys::PRESETS_CAPS_WASD_ARROWS).unwrap_or_default(),
            caps_hjkl_arrows: CapsHjklArrowsSettings {
                enabled: store.get(keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED).unwrap_or_default(),
                key_set: store
                    .get(keys::PRESETS_CAPS_HJKL_ARROWS_KEY_SET)
                    .unwrap_or(ArrowKeySet::Hjkl),
            },
            caps_home_row: CapsHomeRowSettings {
                enabled: store.get(keys::PRESETS_CAPS_HOME_ROW_ENABLED).unwrap_or_default(),
                scheme: store
                    .get(keys::PRESETS_CAPS_HOME_ROW_SCHEME)
                    .unwrap_or(HomeRowScheme::SymbolRow),
            },
            double_tap_shift_to_caps: store
                .get(keys::PRESETS_DOUBLE_TAP_SHIFT_TO_CAPS)
                .unwrap_or_default(),
            left_right_shift_to_caps: store
                .get(keys::PRESETS_LEFT_RIGHT_SHIFT_TO_CAPS)
                .unwrap_or_default(),
            shift_caps_to_caps: store.get(keys::PRESETS_SHIFT_CAPS_TO_CAPS).unwrap_or_default(),
            shift_quick_press_brackets: ShiftQuickPressBracketsSettings {
                enabled: store
                    .get(keys::PRESETS_SHIFT_QUICK_PRESS_BRACKETS_ENABLED)
                    .unwrap_or_default(),
                pair: store
                    .get(keys::PRESETS_SHIFT_QUICK_PRESS_BRACKETS_PAIR)
                    .unwrap_or(BracketPair::Parens),
            },
            hyper_delete_to_forward: store
                .get(keys::PRESETS_HYPER_DELETE_TO_FORWARD)
                .unwrap_or_default(),
            delete_to_forward: store.get(keys::PRESETS_DELETE_TO_FORWARD).unwrap_or_default(),
            shift_delete_to_forward: store
                .get(keys::PRESETS_SHIFT_DELETE_TO_FORWARD)
                .unwrap_or_default(),
            paste_without_formatting: PasteWithoutFormattingSettings {
                enabled: store
                    .get(keys::PRESETS_PASTE_WITHOUT_FORMATTING_ENABLED)
                    .unwrap_or_default(),
                trigger: store
                    .get(keys::PRESETS_PASTE_WITHOUT_FORMATTING_TRIGGER)
                    .unwrap_or(PasteTrigger::RightCommand),
            },
            home_end_on_lines: store.get(keys::PRESETS_HOME_END_ON_LINES).unwrap_or_default(),
            synthesize_caps_lock_remap: store
                .get(keys::PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP)
                .unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_store_on_empty_store_matches_default() {
        let store = SettingsStore::in_memory();
        assert_eq!(PresetSettings::from_store(&store), PresetSettings::default());
    }

    #[test]
    fn default_matches_measured_defaults() {
        let s = PresetSettings::default();
        assert!(!s.caps_lock_remap.enabled);
        assert_eq!(s.caps_lock_remap.target, RemapCapsTarget::LeftControl);
        assert!(!s.caps_quick_press.enabled);
        assert_eq!(s.caps_quick_press.action, QuickPressCapsAction::CapsLock);
        assert_eq!(s.quick_press_duration_ms, 1000);
        assert!(!s.caps_space_enter);
        assert!(!s.caps_wasd_arrows);
        assert!(!s.caps_hjkl_arrows.enabled);
        assert_eq!(s.caps_hjkl_arrows.key_set, ArrowKeySet::Hjkl);
        assert!(!s.caps_home_row.enabled);
        assert_eq!(s.caps_home_row.scheme, HomeRowScheme::SymbolRow);
        assert!(!s.double_tap_shift_to_caps);
        assert!(!s.left_right_shift_to_caps);
        assert!(!s.shift_caps_to_caps);
        assert!(!s.shift_quick_press_brackets.enabled);
        assert_eq!(s.shift_quick_press_brackets.pair, BracketPair::Parens);
        assert!(!s.hyper_delete_to_forward);
        assert!(!s.delete_to_forward);
        assert!(!s.shift_delete_to_forward);
        assert!(!s.paste_without_formatting.enabled);
        assert_eq!(s.paste_without_formatting.trigger, PasteTrigger::RightCommand);
        assert!(!s.home_end_on_lines);
        assert!(!s.synthesize_caps_lock_remap);
    }

    #[test]
    fn from_store_clamps_quick_press_duration_to_valid_range() {
        let mut store = SettingsStore::in_memory();
        store.set(keys::PRESETS_QUICK_PRESS_DURATION_MS, &50u64).unwrap();
        assert_eq!(PresetSettings::from_store(&store).quick_press_duration_ms, 250);

        store.set(keys::PRESETS_QUICK_PRESS_DURATION_MS, &5000u64).unwrap();
        assert_eq!(PresetSettings::from_store(&store).quick_press_duration_ms, 2000);

        store.set(keys::PRESETS_QUICK_PRESS_DURATION_MS, &1500u64).unwrap();
        assert_eq!(PresetSettings::from_store(&store).quick_press_duration_ms, 1500);
    }

    #[test]
    fn from_store_applies_only_the_stored_field() {
        let mut store = SettingsStore::in_memory();
        store.set(keys::PRESETS_CAPS_SPACE_ENTER, &true).unwrap();

        let s = PresetSettings::from_store(&store);
        assert!(s.caps_space_enter);
        assert_eq!(
            s,
            PresetSettings { caps_space_enter: true, ..PresetSettings::default() }
        );
    }

    #[test]
    fn empty_json_deserializes_to_default() {
        let parsed: PresetSettings = serde_json::from_str("{}").unwrap();
        assert_eq!(parsed, PresetSettings::default());
    }

    #[test]
    fn serde_round_trip() {
        let s = PresetSettings {
            caps_lock_remap: CapsLockRemapSettings { enabled: true, target: RemapCapsTarget::Esc },
            caps_quick_press: CapsQuickPressSettings { enabled: true, action: QuickPressCapsAction::Slash },
            quick_press_duration_ms: 500,
            caps_space_enter: true,
            caps_wasd_arrows: true,
            caps_hjkl_arrows: CapsHjklArrowsSettings { enabled: true, key_set: ArrowKeySet::Ijkl },
            caps_home_row: CapsHomeRowSettings { enabled: true, scheme: HomeRowScheme::FunctionRow },
            double_tap_shift_to_caps: true,
            left_right_shift_to_caps: true,
            shift_caps_to_caps: true,
            shift_quick_press_brackets: ShiftQuickPressBracketsSettings { enabled: true, pair: BracketPair::Angles },
            hyper_delete_to_forward: true,
            delete_to_forward: true,
            shift_delete_to_forward: true,
            paste_without_formatting: PasteWithoutFormattingSettings { enabled: true, trigger: PasteTrigger::HyperKey },
            home_end_on_lines: true,
            synthesize_caps_lock_remap: true,
        };
        let json = serde_json::to_string(&s).unwrap();
        let parsed: PresetSettings = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, s);
    }
}
