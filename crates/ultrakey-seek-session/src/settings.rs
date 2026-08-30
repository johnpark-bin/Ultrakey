//! `Seek` 탭 설정의 **저장 표현** — 명세 §4 표 그대로.
//!
//! [`SeekConfig`](crate::config::SeekConfig)와의 차이: `SeekConfig` 는 상태 머신이
//! 판정에 쓰는 정제된 값(예: `remap_key: Option<KeyCode>`)을 들지만, [`SeekSettings`]
//! 는 사용자가 UI 에서 고른 값 그대로(`SourceKey` variant, 웹 `KeyboardEvent.code`
//! 문자열)를 들고 있다가 [`SeekSettings::to_config`] 로 옮긴다 — `KoreanSettings`/
//! `PresetSettings` 가 이미 쓰는 관례(저장 표현 → 런타임 판정 구조체 분리)를 그대로
//! 따른다.
//!
//! ⭐ **부재 = 기본값**(F-15 §3.1) — [`SeekSettings::default()`]/[`SeekSettings::
//! from_store`] 는 명세 §4 의 실측 출고 기본값(전부 미설정/꺼짐)과 정확히 같다.

use serde::{Deserialize, Serialize};

use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::SourceKey;
use ultrakey_core::settings::{keys, SettingsStore};

use crate::config::SeekConfig;

/// `Seek` 탭 설정의 저장 표현. `SeekConfig`(런타임 판정용)와 달리 사용자가 고른
/// 값 그대로를 들고 있다.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SeekSettings {
    /// `Toggle Seek with shortcut:` — 미설정이면 `None`(버튼 라벨 `Record Shortcut`,
    /// 실측 출고 기본값).
    pub toggle_shortcut: Option<SeekShortcut>,
    /// `Remap key to Seek:` — `-`(미설정)이 `None`(실측 출고 기본값).
    pub remap_key: Option<SourceKey>,
    /// `Only show while the remapped key is held`(저장 키 `seekExecuteOnClose`).
    /// 기본 ☐.
    pub execute_on_close: bool,
    /// `Semicolon highlights next match`(저장 키 `semicolonCycleSeek`). 기본 ☐.
    pub semicolon_cycles: bool,
}

/// 전역 단축키 하나 — 웹 `KeyboardEvent.code` + modifier 비트.
///
/// ⭐ 웹 `KeyboardEvent.code` 를 그대로 저장 표현으로 쓰는 이유: 전역 단축키
/// 레코더는 설정 창(브라우저 웹뷰) 안에서 동작하고, 그 안에서 얻을 수 있는 값이
/// `KeyboardEvent.code` 뿐이다. 물리 키코드로의 변환은
/// `ultrakey_core::keycode::from_web_code` 가 맡는다 — 이 구조체는 원본 문자열을
/// 그대로 보존해 그 변환이 언제든 재현 가능하게 한다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeekShortcut {
    /// 웹 `KeyboardEvent.code`(예: `"Space"`, `"KeyA"`).
    pub code: String,
    /// 그 키와 함께 눌린 modifier — `EventFlags` 비트마스크 관례.
    pub modifiers: EventFlags,
}

impl SeekShortcut {
    /// 표시 문자열 — modifier 기호(`⌃⌥⇧⌘`)를 macOS 표준 순서로 붙이고 뒤에 키
    /// 이름을 붙인다(예: `"⌥Space"`, `"⇧⌘A"`).
    #[must_use]
    pub fn display(&self) -> String {
        let mut out = String::new();
        if self.modifiers.contains(EventFlags::CONTROL) {
            out.push('⌃');
        }
        if self.modifiers.contains(EventFlags::ALTERNATE) {
            out.push('⌥');
        }
        if self.modifiers.contains(EventFlags::SHIFT) {
            out.push('⇧');
        }
        if self.modifiers.contains(EventFlags::COMMAND) {
            out.push('⌘');
        }
        out.push_str(&human_key_name(&self.code));
        out
    }
}

/// `code` 를 사람이 읽는 키 이름으로 바꾼다(`"KeyA"` → `"A"`, `"Digit1"` → `"1"`,
/// 그 밖의 이름 있는 키는 그대로 — `"Space"` → `"Space"`).
fn human_key_name(code: &str) -> String {
    if let Some(letter) = code.strip_prefix("Key") {
        return letter.to_string();
    }
    if let Some(digit) = code.strip_prefix("Digit") {
        return digit.to_string();
    }
    match code {
        "ArrowUp" => "↑".to_string(),
        "ArrowDown" => "↓".to_string(),
        "ArrowLeft" => "←".to_string(),
        "ArrowRight" => "→".to_string(),
        "Backquote" => "`".to_string(),
        "Minus" => "-".to_string(),
        "Equal" => "=".to_string(),
        "BracketLeft" => "[".to_string(),
        "BracketRight" => "]".to_string(),
        "Backslash" => "\\".to_string(),
        "Semicolon" => ";".to_string(),
        "Quote" => "'".to_string(),
        "Comma" => ",".to_string(),
        "Period" => ".".to_string(),
        "Slash" => "/".to_string(),
        other => other.to_string(),
    }
}

impl SeekSettings {
    /// [`SettingsStore`]에서 필드별로 조립한다 — 키가 없는 필드는 그 필드의 기본값을
    /// 쓴다("부재 = 기본값", F-15 §3.1). `KoreanSettings::from_store`/
    /// `PresetSettings::from_store` 와 같은 조립 관례를 따른다.
    #[must_use]
    pub fn from_store(store: &SettingsStore) -> Self {
        let toggle_shortcut = store
            .get::<String>(keys::SEEK_TOGGLE_SHORTCUT_CODE)
            .map(|code| {
                let bits: u64 = store.get(keys::SEEK_TOGGLE_SHORTCUT_MODIFIERS).unwrap_or(0);
                SeekShortcut {
                    code,
                    modifiers: EventFlags(bits),
                }
            });

        SeekSettings {
            toggle_shortcut,
            remap_key: store.get(keys::SEEK_REMAP_KEY),
            execute_on_close: store.get(keys::SEEK_EXECUTE_ON_CLOSE).unwrap_or_default(),
            semicolon_cycles: store.get(keys::SEEK_SEMICOLON_CYCLE).unwrap_or_default(),
        }
    }

    /// 런타임 판정용 설정으로 옮긴다. `quick_press_opens` 는 `Presets` 탭 소관이라
    /// 이 구조체가 모른다 — 호출자가 넘긴다(`Presets` 탭 `Quick press caps lock to
    /// execute:` = `Seek` 인지).
    #[must_use]
    pub fn to_config(&self, quick_press_opens: bool) -> SeekConfig {
        SeekConfig {
            // ⚠️ `menu (PC)`·F21~F24 는 `SourceKey::keycode()` 가 `None` 을 낸다
            // (표준 kVK_* 상수가 없다) — 그 경우 이 경로는 트리거가 성립하지
            // 않는다(`remap_key_options_with_no_keycode_produce_no_trigger` 테스트).
            remap_key: self.remap_key.and_then(SourceKey::keycode),
            execute_on_close: self.execute_on_close,
            semicolon_cycles: self.semicolon_cycles,
            quick_press_opens,
            // ⭐ 조합 자체를 넘긴다 — 세션 중 재입력 판정에 쓰인다
            // ([`SeekConfig::global_shortcut`] 문서 ②, 실기 검증이 찾은 결함).
            // `KeyCode::from_web_code` 가 모르는 코드면 `None` 이 되어 그 경로가
            // 성립하지 않는다 — 지어낸 키코드를 넣지 않는다.
            global_shortcut: self.toggle_shortcut.as_ref().and_then(|s| {
                ultrakey_core::keycode::from_web_code(&s.code).map(|kc| (kc, s.modifiers))
            }),
        }
    }

    /// `Remap key to Seek:` 팝업의 선택지 **35종**(명세 §4 표, 표시 순서 그대로).
    ///
    /// ⭐ **`SourceKey::all()`(Hyperkey 팝업, 35종, `globe` 포함·`-` 없음)과 의도적으로
    /// 다르다**(명세 §5 #7 "소스 키 열거형의 비대칭"). `globe` 은 여기 없다 — Seek 의
    /// 트리거로 지정할 수 없는 유일한 소스 키다. 대신 `-`(미설정, `None`)이 맨
    /// 앞에 있다.
    #[must_use]
    pub fn remap_key_options() -> &'static [Option<SourceKey>] {
        use SourceKey::{
            CapsLock, LeftCommand, LeftControl, LeftOption, LeftShift, MenuPc, RightCommand,
            RightControl, RightOption, RightShift, F1, F10, F11, F12, F13, F14, F15, F16, F17, F18,
            F19, F2, F20, F21, F22, F23, F24, F3, F4, F5, F6, F7, F8, F9,
        };
        &[
            None,
            Some(CapsLock),
            Some(RightOption),
            Some(RightShift),
            Some(RightCommand),
            Some(RightControl),
            Some(LeftOption),
            Some(LeftShift),
            Some(LeftCommand),
            Some(LeftControl),
            // ⚠️ Globe 없음 — 명세 §5 #7.
            Some(MenuPc),
            Some(F1),
            Some(F2),
            Some(F3),
            Some(F4),
            Some(F5),
            Some(F6),
            Some(F7),
            Some(F8),
            Some(F9),
            Some(F10),
            Some(F11),
            Some(F12),
            Some(F13),
            Some(F14),
            Some(F15),
            Some(F16),
            Some(F17),
            Some(F18),
            Some(F19),
            Some(F20),
            Some(F21),
            Some(F22),
            Some(F23),
            Some(F24),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ActivationPath;
    use ultrakey_core::keycode::KeyCode;

    // ── 기본값(부재 = 기본값, F-15 §3.1) ─────────────────────────────────────

    #[test]
    fn default_matches_measured_shipping_defaults() {
        let s = SeekSettings::default();
        assert_eq!(s.toggle_shortcut, None);
        assert_eq!(s.remap_key, None);
        assert!(!s.execute_on_close);
        assert!(!s.semicolon_cycles);
    }

    #[test]
    fn from_store_on_empty_store_matches_default() {
        let store = SettingsStore::in_memory();
        assert_eq!(SeekSettings::from_store(&store), SeekSettings::default());
    }

    #[test]
    fn from_store_reads_toggle_shortcut_code_and_modifiers() {
        let mut store = SettingsStore::in_memory();
        store
            .set(keys::SEEK_TOGGLE_SHORTCUT_CODE, &"Space")
            .unwrap();
        store
            .set(
                keys::SEEK_TOGGLE_SHORTCUT_MODIFIERS,
                &EventFlags::ALTERNATE.0,
            )
            .unwrap();

        let s = SeekSettings::from_store(&store);
        assert_eq!(
            s.toggle_shortcut,
            Some(SeekShortcut {
                code: "Space".to_string(),
                modifiers: EventFlags::ALTERNATE
            })
        );
    }

    #[test]
    fn from_store_reads_remap_key_as_source_key_variant() {
        let mut store = SettingsStore::in_memory();
        store
            .set(keys::SEEK_REMAP_KEY, &SourceKey::CapsLock)
            .unwrap();

        let s = SeekSettings::from_store(&store);
        assert_eq!(s.remap_key, Some(SourceKey::CapsLock));
    }

    // ── display() ───────────────────────────────────────────────────────────

    #[test]
    fn display_formats_letter_and_modifier() {
        let s = SeekShortcut {
            code: "Space".to_string(),
            modifiers: EventFlags::ALTERNATE,
        };
        assert_eq!(s.display(), "⌥Space");
    }

    #[test]
    fn display_orders_modifiers_control_option_shift_command() {
        let s = SeekShortcut {
            code: "KeyA".to_string(),
            modifiers: EventFlags::COMMAND | EventFlags::SHIFT,
        };
        assert_eq!(s.display(), "⇧⌘A");
    }

    #[test]
    fn display_with_no_modifiers_is_just_the_key_name() {
        let s = SeekShortcut {
            code: "Digit1".to_string(),
            modifiers: EventFlags::NONE,
        };
        assert_eq!(s.display(), "1");
    }

    #[test]
    fn display_with_all_four_modifiers() {
        let s = SeekShortcut {
            code: "KeyZ".to_string(),
            modifiers: EventFlags::CONTROL
                | EventFlags::ALTERNATE
                | EventFlags::SHIFT
                | EventFlags::COMMAND,
        };
        assert_eq!(s.display(), "⌃⌥⇧⌘Z");
    }

    // ── remap_key_options() — 명세 §4·§5 #7 ──────────────────────────────────

    #[test]
    fn remap_key_options_has_exactly_35_entries() {
        assert_eq!(SeekSettings::remap_key_options().len(), 35);
    }

    #[test]
    fn remap_key_options_starts_with_none_dash() {
        assert_eq!(SeekSettings::remap_key_options()[0], None);
    }

    /// ⭐ 명세 §5 #7 — `globe` 은 Seek 의 트리거로 지정할 수 없다. `Hyperkey` 팝업
    /// (`SourceKey::all()`)에는 있지만 여기에는 없어야 한다.
    #[test]
    fn remap_key_options_does_not_contain_globe() {
        assert!(!SeekSettings::remap_key_options().contains(&Some(SourceKey::Globe)));
        // 대조 — Hyperkey 팝업(`SourceKey::all()`)에는 `globe` 이 있다.
        assert!(SourceKey::all().contains(&SourceKey::Globe));
    }

    #[test]
    fn remap_key_options_ends_with_f24() {
        let options = SeekSettings::remap_key_options();
        assert_eq!(options[options.len() - 1], Some(SourceKey::F24));
    }

    // ── to_config() ────────────────────────────────────────────────────────

    #[test]
    fn to_config_maps_fields_and_derives_global_shortcut() {
        let s = SeekSettings {
            toggle_shortcut: Some(SeekShortcut {
                code: "Space".to_string(),
                modifiers: EventFlags::ALTERNATE,
            }),
            remap_key: Some(SourceKey::CapsLock),
            execute_on_close: true,
            semicolon_cycles: true,
        };
        let config = s.to_config(false);
        assert!(config.global_shortcut.is_some());
        assert_eq!(config.remap_key, Some(KeyCode::CAPS_LOCK));
        assert!(config.execute_on_close);
        assert!(config.semicolon_cycles);
        assert!(!config.quick_press_opens);
    }

    #[test]
    fn to_config_carries_quick_press_opens_from_caller() {
        let s = SeekSettings::default();
        assert!(s.to_config(true).quick_press_opens);
        assert!(!s.to_config(false).quick_press_opens);
    }

    /// ⚠️ `menu (PC)`·F21~F24 는 `SourceKey::keycode()` 가 `None` 이다 — 지어낸
    /// keycode 를 채우지 않고, 그 경우 이 경로의 트리거가 성립하지 않는다는 것을
    /// `SeekSessionMachine::activate` 를 거쳐 확인한다.
    #[test]
    fn remap_key_options_with_no_keycode_produce_no_trigger() {
        for unmapped in [
            SourceKey::MenuPc,
            SourceKey::F21,
            SourceKey::F22,
            SourceKey::F23,
            SourceKey::F24,
        ] {
            assert_eq!(
                unmapped.keycode(),
                None,
                "{unmapped:?} 는 keycode 가 없어야 한다"
            );

            let settings = SeekSettings {
                remap_key: Some(unmapped),
                ..SeekSettings::default()
            };
            let config = settings.to_config(false);
            assert_eq!(
                config.remap_key, None,
                "{unmapped:?} 는 SeekConfig::remap_key 가 None 이어야 한다"
            );
            assert!(
                !config.any_activation_configured(),
                "{unmapped:?} 만으로는 활성화 경로가 성립하지 않아야 한다"
            );

            let mut machine = crate::machine::SeekSessionMachine::new(config);
            let effects = machine.activate(
                ActivationPath::RemapKey,
                Vec::new(),
                ultrakey_overlay::Appearance::Light,
                false,
                (0.0, 0.0),
            );
            assert!(
                effects.is_empty(),
                "{unmapped:?} 로는 세션이 열리면 안 된다"
            );
            assert!(!machine.is_active());
        }
    }
}
