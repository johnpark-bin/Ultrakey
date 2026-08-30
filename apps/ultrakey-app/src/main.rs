//! Ultrakey — Tauri 앱 껍데기.
//!
//! ⭐ M1 에서 이 크레이트는 배선(wiring)만 했다. M2 1차(F-09, 이슈 #13)부터
//! 환경설정 창이 실재한다 — Seek·Presets 탭은 아직 자리만, Hyperkey·General 탭은
//! 실제로 동작한다. M2 2차(F-10, 이 갱신)부터 메뉴바(`NSStatusItem`)가 실재하고,
//! Dock 아이콘 재클릭·창 닫기로 대신하던 임시 진입점들이 메뉴 클릭으로 옮겨간다.
//!
//! 배선 순서(`docs/dev/architecture.md` §2.1 의 스레드 배치 그대로):
//!
//! 1. 로그 초기화 — `ULTRAKEY_LOG` 환경변수. 수동 검증 절차가 이 로그에 기댄다
//!    (`docs/dev/manual-verification.md` §0)
//!
//! 1-b. ⭐ 단일 인스턴스 보장(F-10, `menu-bar-and-lifecycle.md` §2 시나리오 D·§5
//!    항목 1) — 같은 번들 ID 로 이미 떠 있는 인스턴스가 있으면 Tauri 자체를
//!    띄우지 않고 즉시 종료한다. 두 번째 `CGEventTap` 이 설치되면 키 입력이 두
//!    번 처리되기 때문이다.
//! 2. 로케일 결정 → 문자열 카탈로그(D4: ko + en)
//! 3. ⭐ Accessory 앱으로 전환 — Dock 아이콘·⌘Tab 미노출(`menu-bar-and-lifecycle.md`
//!    §3, 원본 `LSUIElement = true` 실측에 대응)
//! 4. ⭐ 설정 저장소 로드(F-15) — "부재 = 기본값" 규약으로 `HyperkeySettings` 조립.
//!    같은 자리에서 `general.disabledApps` 를 읽어 `AppGateController` 를 복원한다.
//! 5. F-11 권한 감시 시작 → 권한이 생기면 F-07 엔진 시작. 메뉴바가 생긴 뒤로는
//!    이 전이가 설정 창을 자동으로 띄우지 않는다 — `Settings…` 메뉴 클릭이 그
//!    자리를 대신한다.
//!
//! 5-b. ⭐ 메뉴바(`NSStatusItem`) 구성 — 정상 메뉴/`unauthorizedMenu` 두 벌을
//!    미리 만들어 두고, 권한 상태 전이 때마다 트레이의 메뉴만 갈아 끼운다(§3.1).
//!    최전면 앱 추적은 이 앱 계층이 `ultrakey_platform::workspace` 를 **독립적으로**
//!    한 번 더 구독해서 한다 — 엔진 내부의 `SystemHooks` 도 같은 알림을 구독하지만
//!    관측 로그만 남긴다(`ultrakey-engine::system_hooks` 문서 주석: 게이트 갱신은
//!    `AppGateController` 를 쥔 앱 계층의 몫이다).
//! 6. 엔진 사건 처리 — ⛔ 치명적 탭 생성 실패는 재시도가 아니라 **종료**다
//!    (`key-remapping-engine.md` §3-a, §5#16)

// `unsafe` 는 전부 `ultrakey-platform` 에만 있다.
#![forbid(unsafe_code)]
// 릴리스 빌드에서 콘솔 창을 띄우지 않는다(macOS 에서는 무해하지만 관례를 따른다).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::BTreeMap;
use std::io::Write as _;
use std::sync::{Arc, Mutex};

use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIcon;
use tauri::{LogicalSize, Manager, State, Wry};

use ultrakey_core::gate::{AppGate, AppGateController, AppIdentity, AtomicAppGate};
use ultrakey_core::keycode::{KeyCode, SourceKey};
use ultrakey_core::perdevice::{DeviceId, FKey, ManagedLedger, SystemFunction};
use ultrakey_core::settings::{keys, EngineConfig, LoadOutcome, MouseApply, SettingsStore};
use ultrakey_engine::path_b::LedgerStore;
use ultrakey_engine::{Engine, EngineEvent};
use ultrakey_hyperkey::{HyperkeySettings, SettingsWarning, SlotSettings, TrackpadArea};
use ultrakey_i18n::Catalog;
use ultrakey_korean::KoreanSettings;
use ultrakey_presets::{
    ArrowKeySet, BracketPair, Conflict, ConflictKind, HomeRowScheme, PasteTrigger, PresetSettings,
    QuickPressCapsAction, RemapCapsTarget,
};
use ultrakey_permissions::{
    dev_build_warning, onboarding_copy, open_accessibility_settings, out_of_sync_copy,
    PermissionMonitor, PermissionState,
};
use ultrakey_platform::bundle;
use ultrakey_platform::fn_state;
use ultrakey_platform::hid_device;
use ultrakey_platform::login_item;
use ultrakey_platform::workspace::{observe_system_events, SystemEvent, SystemEventObserver};

/// ⭐ F-10 메뉴 항목 id — 그대로 i18n 카탈로그 키이기도 하다(고유하고, 라벨을
/// 조회할 때도 같은 문자열을 쓸 수 있어 별도 매핑표가 필요 없다).
mod menu_ids {
    pub const IGNORE_APP: &str = "menu.ignore_app";
    pub const SETTINGS: &str = "menu.settings";
    pub const ABOUT: &str = "menu.about";
    pub const ADVANCED: &str = "menu.advanced";
    pub const SYNTHESIZE_CAPS_REMAP: &str = "menu.advanced.synthesize_caps_remap";
    pub const RELAUNCH: &str = "menu.advanced.relaunch";
    pub const QUIT: &str = "menu.quit";
    pub const AUTHORIZE: &str = "menu.unauthorized.authorize";
}

/// F-10 이 소유하는 저장 키. `ultrakey_core::settings::keys` 에 넣지 않는 이유:
/// 그 모듈은 `crates/ultrakey-core` 소속이고 이번 위임은 그 크레이트를 건드리지
/// 않는다(동시 작업 중인 다른 위임의 경로다) — `SettingsStore::get`/`set` 은
/// 임의의 문자열 키를 받으므로(`keys::all()` 화이트리스트는 `settings_set`
/// 커맨드 하나만의 검증 규칙이지 저장 계층 자체의 제약이 아니다) 여기서 지역
/// 상수로 선언해도 저장 형식은 동일하다.
mod settings_keys {
    /// F-10 §3.4 — 앱별 비활성화 목록(블랙리스트, `Vec<String>` 번들 ID).
    pub const GENERAL_DISABLED_APPS: &str = "general.disabledApps";
    /// F-10 §3.5 — `General` 탭 `Launch on login` 체크박스의 마지막 사용자 의도.
    /// ⚠️ 실제 로그인 항목 등록 상태의 정본은 OS(`SMAppService`/plist 존재)이고,
    /// 이 키는 UI 가 재부팅 없이도 마지막으로 사용자가 고른 값을 보여주기 위한
    /// 거울(mirror)일 뿐이다.
    pub const GENERAL_LAUNCH_ON_LOGIN: &str = "general.launchOnLogin";
    /// F-10 §3 `Hide menu bar icon` 체크박스.
    pub const GENERAL_HIDE_MENU_BAR_ICON: &str = "general.hideMenuBarIcon";

    // ⭐ `ultrakey-core` 가 소유한 키를 재수출한다 — 여기서 문자열을 다시 쓰면
    // 오타가 컴파일을 통과해 버린다(`keys.rs` 상단 주석과 같은 이유).
    pub use ultrakey_core::settings::keys::{
        HYPERKEY_BLEH_ENABLED, HYPERKEY_HYPER_ENABLED, HYPERKEY_MEH_ENABLED,
    };
}

/// 프런트엔드(권한 모달)가 조회하는 문구 묶음.
///
/// ⭐ `localization-and-input-sources.md` §3.1.5 의 "단일 카탈로그" 결정을 지키는 방식:
/// 웹뷰가 JSON 을 따로 `fetch` 하지 않고 **Rust 가 이미 읽은 같은 카탈로그**를 커맨드로
/// 되돌려준다. 소비자가 둘이어도 소스는 하나뿐이므로 drift 가 구조적으로 불가능하다.
#[derive(serde::Serialize)]
struct ModalCopy {
    kind: &'static str,
    title: String,
    body: String,
    path: String,
    open_button: String,
    checkbox_hint: String,
    locked_hint: String,
    manual_steps: String,
    quit: String,
}

/// F-15 §5(엣지 케이스)의 로드 경고 — 손상 복구/미래 스키마. 프런트엔드가
/// 카탈로그로 문구를 조립할 수 있게 메시지 키 + 위치 인자 하나만 보낸다.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Notice {
    kind: &'static str,
    message_key: &'static str,
    arg: String,
}

fn notice_from_outcome(outcome: &LoadOutcome) -> Option<Notice> {
    match outcome {
        LoadOutcome::Fresh | LoadOutcome::Loaded { .. } => None,
        LoadOutcome::Recovered { backup, .. } => Some(Notice {
            kind: "recovered",
            message_key: "settings.load_recovered",
            arg: backup.display().to_string(),
        }),
        LoadOutcome::NewerSchema { found } => Some(Notice {
            kind: "newerSchema",
            message_key: "settings.load_newer_schema",
            arg: found.to_string(),
        }),
    }
}

/// `TrackpadSettings` 를 그대로 내보내지 않고 감싸는 이유: 프런트엔드가 기대하는
/// 필드 이름(`changeMenuBarIcon`·`haptic`)이 저장 계층의 필드 이름(`change_menu_bar_icon`·
/// `haptic_feedback`)과 살짝 다르다(`preferences-ui.md` 지시 스펙, 이슈 #13). `SlotSettings`·
/// `MouseApply` 는 이미 기대하는 이름 그대로라 감싸지 않고 재사용한다.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TrackpadView {
    enabled: bool,
    area: TrackpadArea,
    change_menu_bar_icon: bool,
    haptic: bool,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct HyperkeyView {
    hyper: SlotSettings,
    include_shift_in_hyper: bool,
    meh: SlotSettings,
    bleh: SlotSettings,
    mouse_apply: MouseApply,
    trackpad: TrackpadView,
}

fn hyperkey_view(h: &HyperkeySettings) -> HyperkeyView {
    HyperkeyView {
        hyper: h.hyper.clone(),
        include_shift_in_hyper: h.include_shift_in_hyper,
        meh: h.meh.clone(),
        bleh: h.bleh.clone(),
        mouse_apply: h.mouse_apply,
        trackpad: TrackpadView {
            enabled: h.trackpad.enabled,
            area: h.trackpad.area,
            change_menu_bar_icon: h.trackpad.change_menu_bar_icon,
            haptic: h.trackpad.haptic_feedback,
        },
    }
}

/// hyper 의 현재 조합 미리보기 문자열(`preferences-ui.md` "Hyperkey 탭" 항목 3).
/// `HyperkeySettings::hyper_flags()` 가 이미 같은 판정을 갖고 있지만 그건
/// `EventFlags` 비트마스크를 돌려준다 — 여기서는 사람이 읽는 기호가 필요하다.
fn hyper_preview(h: &HyperkeySettings) -> String {
    if h.include_shift_in_hyper {
        "⌃⌥⌘⇧".to_string()
    } else {
        "⌃⌥⌘".to_string()
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SourceKeyView {
    value: String,
    label: String,
    has_keycode: bool,
}

fn source_key_view(source: SourceKey) -> SourceKeyView {
    // ⭐ `SourceKey` 는 variant 이름으로 직렬화된다("CapsLock") — 이 값이 곧
    // `settings_set` 이 받는 값과 저장 형식이므로, 손으로 다시 나열해 어긋날
    // 여지를 두지 않고 직렬화 결과를 그대로 재사용한다.
    let value = serde_json::to_value(source)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default();
    SourceKeyView {
        value,
        label: source.label().to_string(),
        has_keycode: source.keycode().is_some(),
    }
}

fn trackpad_area_label_key(area: TrackpadArea) -> &'static str {
    match area {
        TrackpadArea::TopLeft => "settings.hyperkey.area.top_left",
        TrackpadArea::TopRight => "settings.hyperkey.area.top_right",
        TrackpadArea::BottomLeft => "settings.hyperkey.area.bottom_left",
        TrackpadArea::BottomRight => "settings.hyperkey.area.bottom_right",
        TrackpadArea::Top => "settings.hyperkey.area.top",
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TrackpadAreaView {
    value: String,
    label_key: String,
}

fn trackpad_area_view(area: TrackpadArea) -> TrackpadAreaView {
    TrackpadAreaView {
        value: area.as_str().to_string(),
        label_key: trackpad_area_label_key(area).to_string(),
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WarningView {
    kind: &'static str,
    key: String,
}

fn warning_view(warning: SettingsWarning) -> WarningView {
    match warning {
        SettingsWarning::DuplicateSourceKey { source, .. } => WarningView {
            kind: "duplicate",
            key: source.label().to_string(),
        },
        SettingsWarning::UnknownKeycode { source, .. } => WarningView {
            kind: "unknownKey",
            key: source.label().to_string(),
        },
    }
}

// ============================================================================
// F-08 Presets 탭 — `ultrakey_presets` 의 타입을 settings.html 이 기대하는 계약
// (camelCase JSON, variant 이름 = value, 서술형 항목만 labelKey)으로 옮긴다.
// ============================================================================

/// enum 값 하나를 그 값의 직렬화 variant 이름(저장 형식과 같은 문자열)으로 되돌린다.
/// `source_key_view` 가 이미 쓰던 관례를 팝업 enum 6종에도 그대로 적용한다 — 손으로
/// 다시 나열하면 serde derive 와 어긋날 여지가 생긴다.
fn serde_variant_name<T: serde::Serialize>(v: &T) -> String {
    serde_json::to_value(v)
        .ok()
        .and_then(|j| j.as_str().map(str::to_string))
        .unwrap_or_default()
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PresetOptionView {
    value: String,
    label: String,
    label_key: Option<String>,
}

fn preset_option_view<T: Copy + serde::Serialize>(
    v: T,
    label: &str,
    label_key: Option<&str>,
) -> PresetOptionView {
    PresetOptionView {
        value: serde_variant_name(&v),
        label: label.to_string(),
        label_key: label_key.map(str::to_string),
    }
}

/// ⭐ 이 함수들만 `labelKey` 를 채운다 — 위임 지시서가 "정확히 이 키들만" 이라고
/// 못박은 목록 그대로다. 그 밖의 항목(키캡 각인·`Seek`·`H J K L`·`( )` 류)은 전부
/// `None` — 카탈로그를 거치지 않고 값 그대로 UI 에 보인다.
fn remap_caps_target_label_key(v: RemapCapsTarget) -> Option<&'static str> {
    matches!(v, RemapCapsTarget::Nothing).then_some("settings.presets.option.nothing")
}

fn home_row_scheme_label_key(v: HomeRowScheme) -> Option<&'static str> {
    match v {
        HomeRowScheme::SymbolRow => Some("settings.presets.option.home_row.symbol"),
        HomeRowScheme::FunctionRow => Some("settings.presets.option.home_row.function"),
    }
}

fn paste_trigger_label_key(v: PasteTrigger) -> Option<&'static str> {
    match v {
        PasteTrigger::RightCommand => Some("settings.presets.option.paste.right_cmd"),
        PasteTrigger::LeftCommand => Some("settings.presets.option.paste.left_cmd"),
        PasteTrigger::EitherCommand => Some("settings.presets.option.paste.either_cmd"),
        PasteTrigger::HyperKey => Some("settings.presets.option.paste.hyper"),
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PresetOptionsView {
    caps_remap_targets: Vec<PresetOptionView>,
    caps_quick_actions: Vec<PresetOptionView>,
    arrow_key_sets: Vec<PresetOptionView>,
    home_row_schemes: Vec<PresetOptionView>,
    bracket_pairs: Vec<PresetOptionView>,
    paste_triggers: Vec<PresetOptionView>,
}

/// 팝업 6종 전량(50/48/2/2/4/4) — 값은 매번 다시 계산해도 비용이 무시할 만한
/// 정적 목록이라 캐시하지 않는다.
fn preset_options_view() -> PresetOptionsView {
    PresetOptionsView {
        caps_remap_targets: RemapCapsTarget::all()
            .iter()
            .map(|&v| preset_option_view(v, v.label(), remap_caps_target_label_key(v)))
            .collect(),
        caps_quick_actions: QuickPressCapsAction::all()
            .iter()
            .map(|&v| preset_option_view(v, v.label(), None))
            .collect(),
        arrow_key_sets: ArrowKeySet::all()
            .iter()
            .map(|&v| preset_option_view(v, v.label(), None))
            .collect(),
        home_row_schemes: HomeRowScheme::all()
            .iter()
            .map(|&v| preset_option_view(v, v.label(), home_row_scheme_label_key(v)))
            .collect(),
        bracket_pairs: BracketPair::all()
            .iter()
            .map(|&v| preset_option_view(v, v.label(), None))
            .collect(),
        paste_triggers: PasteTrigger::all()
            .iter()
            .map(|&v| preset_option_view(v, v.label(), paste_trigger_label_key(v)))
            .collect(),
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CapsLockRemapView {
    enabled: bool,
    target: String,
}
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CapsQuickPressView {
    enabled: bool,
    action: String,
}
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CapsHjklArrowsView {
    enabled: bool,
    key_set: String,
}
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct CapsHomeRowView {
    enabled: bool,
    scheme: String,
}
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ShiftQuickPressBracketsView {
    enabled: bool,
    pair: String,
}
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PasteWithoutFormattingView {
    enabled: bool,
    trigger: String,
}

/// `PresetSettings` 를 그대로 내보내지 않는 이유는 `HyperkeyView` 와 같다 —
/// 저장 계층은 snake_case 필드 이름을 쓰고(`ultrakey-presets` 는 이번 위임이
/// 건드리지 않는 크레이트다), 프런트엔드는 camelCase 를 기대한다.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PresetsView {
    caps_lock_remap: CapsLockRemapView,
    caps_quick_press: CapsQuickPressView,
    quick_press_duration_ms: u64,
    caps_space_enter: bool,
    caps_wasd_arrows: bool,
    caps_hjkl_arrows: CapsHjklArrowsView,
    caps_home_row: CapsHomeRowView,
    double_tap_shift_to_caps: bool,
    left_right_shift_to_caps: bool,
    shift_caps_to_caps: bool,
    shift_quick_press_brackets: ShiftQuickPressBracketsView,
    hyper_delete_to_forward: bool,
    delete_to_forward: bool,
    shift_delete_to_forward: bool,
    paste_without_formatting: PasteWithoutFormattingView,
    home_end_on_lines: bool,
}

fn presets_view(p: &PresetSettings) -> PresetsView {
    PresetsView {
        caps_lock_remap: CapsLockRemapView {
            enabled: p.caps_lock_remap.enabled,
            target: serde_variant_name(&p.caps_lock_remap.target),
        },
        caps_quick_press: CapsQuickPressView {
            enabled: p.caps_quick_press.enabled,
            action: serde_variant_name(&p.caps_quick_press.action),
        },
        quick_press_duration_ms: p.quick_press_duration_ms,
        caps_space_enter: p.caps_space_enter,
        caps_wasd_arrows: p.caps_wasd_arrows,
        caps_hjkl_arrows: CapsHjklArrowsView {
            enabled: p.caps_hjkl_arrows.enabled,
            key_set: serde_variant_name(&p.caps_hjkl_arrows.key_set),
        },
        caps_home_row: CapsHomeRowView {
            enabled: p.caps_home_row.enabled,
            scheme: serde_variant_name(&p.caps_home_row.scheme),
        },
        double_tap_shift_to_caps: p.double_tap_shift_to_caps,
        left_right_shift_to_caps: p.left_right_shift_to_caps,
        shift_caps_to_caps: p.shift_caps_to_caps,
        shift_quick_press_brackets: ShiftQuickPressBracketsView {
            enabled: p.shift_quick_press_brackets.enabled,
            pair: serde_variant_name(&p.shift_quick_press_brackets.pair),
        },
        hyper_delete_to_forward: p.hyper_delete_to_forward,
        delete_to_forward: p.delete_to_forward,
        shift_delete_to_forward: p.shift_delete_to_forward,
        paste_without_formatting: PasteWithoutFormattingView {
            enabled: p.paste_without_formatting.enabled,
            trigger: serde_variant_name(&p.paste_without_formatting.trigger),
        },
        home_end_on_lines: p.home_end_on_lines,
    }
}

/// `Korean` 탭 5개 항목 — F-16(`docs/spec/korean-input.md` §4.2). `presets_view` 와
/// 같은 형식. `han_eng_switches_input_source`/`hanja_key_converts_hanja` 는 2단계에서
/// 활성화됐다(D-K14) — §3.2 의 키코드가 근거 3중으로 해소되어 UI 도 규칙 평가도
/// 정상 동작한다.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct KoreanView {
    shift_space_switches_input_source: bool,
    han_eng_switches_input_source: bool,
    hanja_key_converts_hanja: bool,
    won_key_types_backtick: bool,
    disable_in_remote_desktop: bool,
}

fn korean_view(k: &KoreanSettings) -> KoreanView {
    KoreanView {
        shift_space_switches_input_source: k.shift_space_switches_input_source,
        han_eng_switches_input_source: k.han_eng_switches_input_source,
        hanja_key_converts_hanja: k.hanja_key_converts_hanja,
        won_key_types_backtick: k.won_key_types_backtick,
        disable_in_remote_desktop: k.disable_in_remote_desktop,
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GeneralView {
    launch_on_login: bool,
    hide_menu_bar_icon: bool,
}

fn general_view(store: &SettingsStore) -> GeneralView {
    GeneralView {
        launch_on_login: store
            .get(settings_keys::GENERAL_LAUNCH_ON_LOGIN)
            .unwrap_or(false),
        hide_menu_bar_icon: store
            .get(settings_keys::GENERAL_HIDE_MENU_BAR_ICON)
            .unwrap_or(false),
    }
}

// ============================================================================
// F-17 키보드별 설정(`per-device-settings.md`) — `Keyboards` 탭 백엔드 계약
// (`settings.html` 851~866행 주석이 정본으로 삼는 모양). 정본은 이 브랜치의
// `CONTRACT.md` §B.6 이다.
// ============================================================================

/// `state.perDevice.devices` 항목 하나 — 팝업이 그대로 쓴다(§3.1.2).
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PerDeviceDeviceView {
    id: String,
    name: String,
    connected: bool,
}

/// `state.perDevice.systemFunctions` 항목 하나 — 기능 2 선택 팝업(§3.5).
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PerDeviceSystemFunctionView {
    value: String,
    label_key: String,
}

/// `Keyboards` 탭 전체를 그리는 데 필요한 다섯 필드(계약 §B.6).
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PerDeviceView {
    devices: Vec<PerDeviceDeviceView>,
    /// `state.sourceKeys` 와 같은 형식(`{value,label}` — 여기서는 `hasKeycode` 도
    /// 함께 실리지만 프런트는 그 필드를 쓰지 않는다) — `SourceKey::all()` 35종.
    source_keys: Vec<SourceKeyView>,
    system_functions: Vec<PerDeviceSystemFunctionView>,
    fn_state_is_standard: Option<bool>,
    values: serde_json::Map<String, serde_json::Value>,
}

/// `SystemFunction` variant 이름(PascalCase, 예: `"DisplayBrightnessDown"`)을
/// `preferences.keyboards.functionKeys.function.<camelCase>` i18n 키로 바꾼다.
/// ⚠️ `SystemFunction` 은 `SourceKey`/`KeyRemapRow` 와 같은 이유로 variant 이름
/// 그대로 직렬화된다(`perdevice/mod.rs` 문서 주석) — 그래서 `serde_variant_name`
/// 이 주는 문자열의 첫 글자만 낮추면 §4.1 카탈로그의 camelCase 세그먼트와 정확히
/// 맞아떨어진다(예: `"Mute"` → `mute`, `"DoNotDisturb"` → `doNotDisturb`).
fn system_function_label_key(f: SystemFunction) -> String {
    let variant = serde_variant_name(&f);
    let mut chars = variant.chars();
    let camel = match chars.next() {
        Some(c) => c.to_lowercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    };
    format!("preferences.keyboards.functionKeys.function.{camel}")
}

/// `perDevice.` 접두사 키 전부를 원본 JSON 그대로 모은다 — `perDevice._managed`
/// (D-17-2 원장)만 제외한다(§B.1: "엔진은 원장이 아니라 설정 스냅샷만 본다").
/// ⭐ `store.get::<Value>()` 는 JSON `null` 도 그대로 `Some(Value::Null)` 로 돌려준다
/// (`Value` 는 자기 자신으로 항상 역직렬화된다) — §3.3 이 요구하는 "`null` 과 부재의
/// 구별"이 이 함수를 거쳐도 사라지지 않는다.
fn collect_per_device_values(store: &SettingsStore) -> BTreeMap<String, serde_json::Value> {
    let mut values = BTreeMap::new();
    for key in store.keys() {
        if key.starts_with("perDevice.") && key != keys::PER_DEVICE_MANAGED {
            if let Some(v) = store.get::<serde_json::Value>(key) {
                values.insert(key.to_string(), v);
            }
        }
    }
    values
}

/// `state.perDevice` 조립 — 계약 §B.6 다섯 필드.
fn build_per_device_view(store: &SettingsStore) -> PerDeviceView {
    // devices — list_attached_keyboards() ∪ 설정 키가 존재하는 디바이스, (vid,pid)
    // 중복 제거(§3.2). `BTreeMap<DeviceId, _>` 자체가 중복 제거 역할을 한다.
    let mut devices: BTreeMap<DeviceId, PerDeviceDeviceView> = BTreeMap::new();
    for info in hid_device::list_attached_keyboards() {
        let id = DeviceId::new(info.vendor_id, info.product_id);
        let name = info
            .product_name
            .clone()
            .unwrap_or_else(|| id.as_str().to_string());
        devices.insert(
            id.clone(),
            PerDeviceDeviceView { id: id.as_str().to_string(), name, connected: true },
        );
    }
    for key in store.keys() {
        let Some(rest) = key.strip_prefix("perDevice.") else {
            continue;
        };
        let Some((scope, _tail)) = rest.split_once('.') else {
            continue;
        };
        // `scope` 가 "all"·"_managed" 면 `DeviceId::parse` 가 자연히 `None` 을 준다
        // (콜론이 없다) — 별도 분기 없이 걸러진다.
        let Some(id) = DeviceId::parse(scope) else {
            continue;
        };
        devices.entry(id.clone()).or_insert_with(|| {
            // 미연결 디바이스의 이름 — 저장된 제품명이 없으니 id 그대로 쓴다
            // (⛔ 개인 디바이스 이름 하드코딩 금지, 계약 §B.6).
            PerDeviceDeviceView { id: id.as_str().to_string(), name: id.as_str().to_string(), connected: false }
        });
    }

    // sourceKeys — `state.sourceKeys` 와 같은 조립 함수를 재사용한다(35종, 키캡 각인).
    let source_keys = SourceKey::all().iter().copied().map(source_key_view).collect();

    // systemFunctions — hid_usage() 가 Some 인 것만(CONTRACT §2.2, 팝업 규약).
    let system_functions = SystemFunction::all()
        .iter()
        .copied()
        .filter(|f| f.hid_usage().is_some())
        .map(|f| PerDeviceSystemFunctionView {
            value: serde_variant_name(&f),
            label_key: system_function_label_key(f),
        })
        .collect();

    PerDeviceView {
        devices: devices.into_values().collect(),
        source_keys,
        system_functions,
        fn_state_is_standard: fn_state::f_keys_are_standard(),
        values: collect_per_device_values(store).into_iter().collect(),
    }
}

/// `settings_set`/`settings_unset`. `perDevice.*` 는 계약 §3.3 이 정한 두 모양뿐이다:
/// `perDevice.<scope>.keyRemap.rows` 또는 `perDevice.<scope>.functionKeys.f1`~`f12`.
/// ⛔ `perDevice._managed`(D-17-2 원장)는 이 경로로 건드릴 수 없다 — `LedgerStore`
/// (엔진이 부른다)만의 채널이다.
fn validate_per_device_key(key: &str) -> Result<(), String> {
    if key == keys::PER_DEVICE_MANAGED {
        return Err(format!("{key} 는 원장 키다 — 이 커맨드로 바꿀 수 없다"));
    }
    let rest = key
        .strip_prefix("perDevice.")
        .ok_or_else(|| format!("알 수 없는 설정 키: {key}"))?;
    let (scope, tail) = rest
        .split_once('.')
        .ok_or_else(|| format!("알 수 없는 설정 키: {key}"))?;
    if scope != keys::PER_DEVICE_COMMON_SCOPE && DeviceId::parse(scope).is_none() {
        return Err(format!("알 수 없는 디바이스 식별자: {scope}"));
    }
    let is_key_remap = tail == "keyRemap.rows";
    let is_function_key = FKey::all()
        .iter()
        .any(|f| tail == format!("functionKeys.{}", f.key_segment()));
    if !is_key_remap && !is_function_key {
        return Err(format!("알 수 없는 설정 키: {key}"));
    }
    Ok(())
}

/// `settings_set` 의 `perDevice.*` 경로(F-17). 다른 `settings_set_*` 와 달리 메모리
/// 캐시가 없다 — `perDevice.*` 값은 저장소 자체가 정본이고, `EngineConfig::
/// per_device_values` 는 매번 저장소에서 다시 채운다(`build_engine_config`).
///
/// 순서: 1) 키 모양 검증 2) **저장**(⚠️ JSON `null` 을 그대로 쓴다 — §3.3 의 명시적
/// 끔이지 삭제가 아니다) 3) **엔진 반영**(`reconfigure_engine` 이 저장소를 다시 읽어
/// `per_device_values` 를 채운다 — `SettingsStore::set()` 은 디스크 쓰기가 실패해도
/// 메모리 값은 이미 갱신해 두므로, 이 순서로도 D-B 근거("저장 실패와 무관하게 엔진
/// 반영")가 그대로 성립한다) 4) 새 `SettingsState`.
fn settings_set_per_device(
    state: &Arc<AppState>,
    key: &str,
    value: &serde_json::Value,
) -> Result<SettingsState, String> {
    validate_per_device_key(key)?;

    let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let presets_snapshot = *state.presets.lock().map_err(|e| e.to_string())?;
    let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;

    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "설정 저장 실패");
                Some(e.to_string())
            }
        }
    };

    // perDevice.* 는 hyperkey/meh/bleh 소스 키 자체를 바꾸지 않으므로 force_reset
    // (stuck modifier 방지) 은 필요 없다 — D-D 의 대상 밖이다.
    reconfigure_engine(state, &hyperkey_snapshot, &presets_snapshot, &korean_snapshot, false)?;

    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &store,
        save_error,
        None,
    ))
}

/// F-17 §3.3 "공통 따름" — 값 `null` 저장이 아니라 키 **삭제**다(부재와 명시적 끔은
/// 다르다). `SettingsStore`(F-15, `crates/ultrakey-core`)는 개별 키 삭제 API 를
/// 노출하지 않는다 — "삭제 없는 write-through"가 그 크레이트의 명시적 설계 결정이기
/// 때문이다(`store.rs` 모듈 문서: "되돌려도 키를 지우지 않는다"). 그 크레이트를 고치는
/// 대신(위임 범위 밖) 이 앱 계층에서만, 저장 파일을 직접 읽어 그 키만 제거하고 다시
/// 쓴 뒤 `SettingsStore::load()` 로 재적재해 메모리 캐시를 동기화한다.
///
/// ⚠️ 아직 디스크에 한 번도 쓴 적 없는 in-memory 스토어(`path() == None`)는 지울
/// 파일도, 지울 키도 없다 — 아무 것도 하지 않는다(그 상태에서는 애초에 이 키가
/// 저장돼 있을 수 없다).
fn remove_setting_key(store: &mut SettingsStore, key: &str) -> Result<(), String> {
    let Some(path) = store.path().map(|p| p.to_path_buf()) else {
        return Ok(());
    };

    let raw = match std::fs::read_to_string(&path) {
        Ok(raw) => raw,
        // 파일이 아예 없으면 지울 키도 없다 — 조용히 성공 취급한다.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("설정 파일을 읽을 수 없다: {e}")),
    };
    let mut envelope: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("설정 파일 파싱 실패: {e}"))?;
    if let Some(values) = envelope.get_mut("values").and_then(|v| v.as_object_mut()) {
        values.remove(key);
    }
    let json = serde_json::to_vec_pretty(&envelope).map_err(|e| e.to_string())?;

    // 원자적 쓰기 — `SettingsStore::persist()` 와 같은 절차(임시 파일 → 동기화 →
    // rename). `state.store` 락을 쥔 채로만 호출되므로(호출부 참고) 동시 쓰기 경합은
    // 없다.
    let tmp_path = path.with_extension("unset.tmp");
    {
        let mut f = std::fs::File::create(&tmp_path).map_err(|e| e.to_string())?;
        f.write_all(&json).map_err(|e| e.to_string())?;
        f.sync_all().map_err(|e| e.to_string())?;
    }
    std::fs::rename(&tmp_path, &path).map_err(|e| e.to_string())?;

    let (reloaded, _outcome) = SettingsStore::load(path);
    *store = reloaded;
    Ok(())
}

/// `perDevice._managed` 원장 — `ultrakey_engine::path_b::LedgerStore` 구현
/// (계약 §B.2). 엔진은 저장소 구현을 모른다 — 이 앱 계층이 `SettingsStore` 위에
/// 얹는다. `AppState` 를 통째로 쥐는 이유는 `store` 가 그 안의 `Mutex` 필드라서다
/// (별도로 `Arc<Mutex<SettingsStore>>` 를 다시 만들지 않는다).
struct AppLedgerStore {
    app_state: Arc<AppState>,
}

impl LedgerStore for AppLedgerStore {
    /// ⚠️ 저장 실패가 앱을 죽이면 안 된다(계약 §B.6 항목 1) — 읽기는 실패할 수 없는
    /// 경로다(`get` 이 없으면 빈 원장), 락이 poison 된 경우만 panic 한다(기존 코드
    /// 전반이 `.lock().unwrap()` 을 쓰는 것과 같은 관례 — poison 은 이미 다른 곳에서
    /// panic 이 난 뒤라는 뜻이라 여기서 감출 이유가 없다).
    fn load(&self) -> ManagedLedger {
        let store = self.app_state.store.lock().unwrap();
        match store.get::<serde_json::Value>(keys::PER_DEVICE_MANAGED) {
            Some(v) => ultrakey_core::perdevice::read_managed_ledger(&v),
            None => ManagedLedger::new(),
        }
    }

    fn store(&self, ledger: &ManagedLedger) -> Result<(), String> {
        let value = ultrakey_core::perdevice::write_managed_ledger(ledger);
        let mut store = self.app_state.store.lock().map_err(|e| e.to_string())?;
        store.set(keys::PER_DEVICE_MANAGED, &value).map_err(|e| {
            tracing::error!(error = %e, "perDevice._managed 원장 저장 실패");
            e.to_string()
        })
    }
}

/// `settings_set`/`settings_resolve_conflict` 가 돌려주는 충돌 대화상자 페이로드
/// (architecture.md §6.5). `kind` 문자열은 `settings.presets.conflict.title.<kind>`
/// i18n 키와 맞물리므로 `ConflictKind` 의 정확한 camelCase 표기여야 한다.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PendingConflictView {
    kind: &'static str,
    key: String,
    value: serde_json::Value,
    disable_label_keys: Vec<String>,
}

fn conflict_kind_str(kind: ConflictKind) -> &'static str {
    match kind {
        ConflictKind::CapsLockAlreadyRemapped => "capsLockAlreadyRemapped",
        ConflictKind::CapsLockArrows => "capsLockArrows",
        ConflictKind::CapsLockHomeRow => "capsLockHomeRow",
    }
}

/// 끄게 될 설정의 저장 키 → UI 라벨 카탈로그 키. `ultrakey_presets::conflicts` 가
/// 만드는 `to_disable` 목록은 이 네 개 상수만으로 구성된다(`conflicts.rs`
/// `DISABLE_*` 상수 참고) — 그 밖의 값은 있을 수 없는 경로다.
///
/// ⚠️ HJKL 항목은 완전한 문장형 카탈로그 키가 없다 — `settings.presets.caps_hjkl`
/// 은 prefix/suffix 두 조각(그 사이에 팝업이 낀다)으로만 존재한다(`resources/i18n/
/// en.json` 확인). 다른 항목과 달리 prefix 조각만 인용한다 — 완전한 새 키를
/// 만들 수는 없으므로(⛔ `resources/i18n/**` 는 이 위임의 편집 범위 밖) 이미
/// 있는 키 중 가장 가까운 것을 쓴다.
fn disable_label_key_for_setting(store_key: &str) -> &'static str {
    match store_key {
        k if k == keys::PRESETS_CAPS_LOCK_REMAP_ENABLED => "settings.presets.caps_remap",
        k if k == keys::PRESETS_CAPS_WASD_ARROWS => "settings.presets.caps_wasd",
        k if k == keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED => "settings.presets.caps_hjkl.prefix",
        k if k == keys::PRESETS_CAPS_HOME_ROW_ENABLED => "settings.presets.caps_home_row",
        // ⭐ 대화상자 #1 의 배타 대상 — caps lock 을 점유한 hyper/meh/bleh 슬롯.
        k if k == keys::HYPERKEY_HYPER_ENABLED => "settings.hyperkey.hyper.label",
        k if k == keys::HYPERKEY_MEH_ENABLED => "settings.hyperkey.meh.label",
        k if k == keys::HYPERKEY_BLEH_ENABLED => "settings.hyperkey.bleh.label",
        other => {
            // 방어적 — conflicts.rs 가 이 네 개 밖의 키를 내놓는 일은 없어야 한다.
            tracing::error!(key = other, "충돌 해소 목록에 알 수 없는 설정 키가 있다");
            "settings.presets.heading"
        }
    }
}

fn pending_conflict_view(conflict: Conflict, key: &str, value: &serde_json::Value) -> PendingConflictView {
    PendingConflictView {
        kind: conflict_kind_str(conflict.kind),
        key: key.to_string(),
        value: value.clone(),
        disable_label_keys: conflict
            .to_disable
            .iter()
            .map(|k| disable_label_key_for_setting(k).to_string())
            .collect(),
    }
}

/// hyper/meh/bleh 중 **활성화된** 슬롯의 소스가 caps lock 인가(architecture.md §6
/// "caps_is_modifier_source" 정의 그대로).
fn caps_is_modifier_source(h: &HyperkeySettings) -> bool {
    (h.hyper.enabled && h.hyper.source == SourceKey::CapsLock)
        || (h.meh.enabled && h.meh.source == SourceKey::CapsLock)
        || (h.bleh.enabled && h.bleh.source == SourceKey::CapsLock)
}

/// caps lock 을 소스로 쓰고 있는 **활성** hyper/meh/bleh 슬롯의 저장 키 목록.
///
/// ⭐ 충돌 대화상자 #1(`CapsLockAlreadyRemapped`)의 **배타 대상**이 이것이다 —
/// `Remap caps lock to:` 를 켜려 할 때 꺼야 하는 것은 그 설정 자신이 아니라 caps lock
/// 을 이미 점유하고 있는 이 슬롯들이다(architecture.md §6.5).
fn caps_modifier_slot_keys(h: &HyperkeySettings) -> Vec<&'static str> {
    let mut out = Vec::new();
    if h.hyper.enabled && h.hyper.source == SourceKey::CapsLock {
        out.push(settings_keys::HYPERKEY_HYPER_ENABLED);
    }
    if h.meh.enabled && h.meh.source == SourceKey::CapsLock {
        out.push(settings_keys::HYPERKEY_MEH_ENABLED);
    }
    if h.bleh.enabled && h.bleh.source == SourceKey::CapsLock {
        out.push(settings_keys::HYPERKEY_BLEH_ENABLED);
    }
    out
}

/// D-1 — 이 설정 조합에서 경로 B 가 실제로 설치해야 할 alias(`docs/dev/
/// architecture.md` §6.1). `PresetSettings::needs_caps_lock_alias` 가 "필요한가"를
/// 판정하고, `synthesize_caps_lock_remap`(Advanced 토글)이 그것을 무시하고 경로 A 만
/// 쓰게 만들 수 있다.
fn compute_caps_lock_alias(presets: &PresetSettings, caps_is_source: bool) -> Option<KeyCode> {
    if presets.needs_caps_lock_alias(caps_is_source) && !presets.synthesize_caps_lock_remap {
        Some(KeyCode::F18)
    } else {
        None
    }
}

/// hyperkey.validate() 의 경고에 F-08/D-1 전용 경고 2종을 더한다(위임 지시서 §5):
/// - `RemapCapsTarget` 의 F21~F24(keycode 미확정)를 고르면 `unknownKey`.
/// - D-1 alias(F18)와 hyper/meh/bleh 소스가 우연히 겹치면 `duplicate`(기존 hyperkey
///   중복 경고 키를 재사용 — 새 i18n 키를 만들지 않는다).
fn build_preset_warnings(
    hyperkey: &HyperkeySettings,
    presets: &PresetSettings,
    caps_lock_alias: Option<KeyCode>,
) -> Vec<WarningView> {
    let mut warnings = Vec::new();

    if presets.caps_lock_remap.enabled
        && matches!(
            presets.caps_lock_remap.target,
            RemapCapsTarget::F21 | RemapCapsTarget::F22 | RemapCapsTarget::F23 | RemapCapsTarget::F24
        )
    {
        warnings.push(WarningView {
            kind: "unknownKey",
            key: presets.caps_lock_remap.target.label().to_string(),
        });
    }

    if caps_lock_alias == Some(KeyCode::F18) {
        let slots = [
            (hyperkey.hyper.enabled, hyperkey.hyper.source),
            (hyperkey.meh.enabled, hyperkey.meh.source),
            (hyperkey.bleh.enabled, hyperkey.bleh.source),
        ];
        if slots.into_iter().any(|(enabled, source)| enabled && source == SourceKey::F18) {
            tracing::warn!(
                "D-1 caps lock alias(F18)가 hyper/meh/bleh 소스로 고른 F18 과 충돌한다"
            );
            warnings.push(WarningView { kind: "duplicate", key: SourceKey::F18.label().to_string() });
        }
    }

    warnings
}

/// 환경설정 창이 화면을 다시 그리는 데 필요한 전부. `settings_bootstrap`·
/// `settings_set`·`settings_set_tab`·`settings_resolve_conflict` 이 공통으로
/// 돌려준다.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SettingsState {
    hyperkey: HyperkeyView,
    hyper_preview: String,
    source_keys: Vec<SourceKeyView>,
    trackpad_areas: Vec<TrackpadAreaView>,
    warnings: Vec<WarningView>,
    last_tab: String,
    /// 직전 `settings_set` 호출에서 저장이 실패했다면 그 사유(§3.7: 엔진 반영은
    /// 이미 끝났고, 이건 UI 가 `settings.save_failed` 로 알리기만 하면 되는 정보다).
    save_error: Option<String>,
    presets: PresetsView,
    preset_options: PresetOptionsView,
    /// D-1 — 지금 경로 B 로 caps lock 이 F18 로 리매핑돼 있는가(`settings.presets.
    /// caps_alias.note` 힌트를 UI 가 이 값으로 보인다).
    caps_lock_alias_active: bool,
    general: GeneralView,
    /// F-16 `Korean` 탭.
    korean: KoreanView,
    /// 값을 아직 적용하지 않은 충돌(architecture.md §6.5) — `Some` 이면 그 앞의
    /// `settings_set` 호출은 아무것도 저장·반영하지 않았다.
    pending_conflict: Option<PendingConflictView>,
    /// F-17 `Keyboards` 탭(`per-device-settings.md`, `settings.html` 851~866행 계약).
    per_device: PerDeviceView,
}

fn build_settings_state(
    hyperkey: &HyperkeySettings,
    presets: &PresetSettings,
    korean: &KoreanSettings,
    store: &SettingsStore,
    save_error: Option<String>,
    pending_conflict: Option<PendingConflictView>,
) -> SettingsState {
    let last_tab = store
        .get::<String>(keys::UI_LAST_TAB)
        .unwrap_or_else(|| "hyperkey".to_string());
    let caps_is_source = caps_is_modifier_source(hyperkey);
    let caps_lock_alias = compute_caps_lock_alias(presets, caps_is_source);

    let mut warnings: Vec<WarningView> = hyperkey.validate().into_iter().map(warning_view).collect();
    warnings.extend(build_preset_warnings(hyperkey, presets, caps_lock_alias));

    SettingsState {
        hyperkey: hyperkey_view(hyperkey),
        hyper_preview: hyper_preview(hyperkey),
        source_keys: SourceKey::all().iter().copied().map(source_key_view).collect(),
        trackpad_areas: TrackpadArea::all()
            .iter()
            .copied()
            .map(trackpad_area_view)
            .collect(),
        warnings,
        last_tab,
        save_error,
        presets: presets_view(presets),
        preset_options: preset_options_view(),
        caps_lock_alias_active: caps_lock_alias.is_some(),
        general: general_view(store),
        korean: korean_view(korean),
        pending_conflict,
        per_device: build_per_device_view(store),
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppMeta {
    version: String,
    bundle_id: String,
    log_path: String,
    settings_path: Option<String>,
    settings_file_exists: bool,
}

/// `open_log_file()` 이 실제로 여는 경로와 같은 계산이지만 파일을 열지는 않는다
/// — About 패널에 "어디에 로그를 쓰는가"만 보여주면 된다.
fn log_file_path_display() -> String {
    match std::env::var_os("HOME") {
        Some(home) => std::path::PathBuf::from(home)
            .join("Library/Logs/Ultrakey/ultrakey.log")
            .display()
            .to_string(),
        None => String::new(),
    }
}

fn build_app_meta(app: &tauri::AppHandle, store: &SettingsStore) -> AppMeta {
    AppMeta {
        version: app.package_info().version.to_string(),
        bundle_id: app.config().identifier.clone(),
        log_path: log_file_path_display(),
        settings_path: store.path().map(|p| p.display().to_string()),
        settings_file_exists: store.path().map(|p| p.exists()).unwrap_or(false),
    }
}

#[derive(serde::Serialize)]
struct SettingsBootstrap {
    locale: String,
    strings: BTreeMap<String, String>,
    state: SettingsState,
    meta: AppMeta,
    notice: Option<Notice>,
}

/// hyper/meh/bleh 규칙에 영향을 주는 키인가.
///
/// ⭐ D-D(상위 세션 설계 결정): "규칙(`modifier_rules`)이 바뀌는 변경에는
/// `Engine::force_reset_state()` 도 함께 호출한다." 마우스 적용 범위·트랙패드
/// 설정은 물리 소스 키 자체를 바꾸지 않으므로(`to_modifier_rules()` 는 이 필드들을
/// 읽지 않는다 — `ultrakey-hyperkey` 문서 주석 참고) stuck modifier 위험이 없어
/// 대상에서 뺀다.
fn key_affects_modifier_rules(key: &str) -> bool {
    matches!(
        key,
        keys::HYPERKEY_HYPER_ENABLED
            | keys::HYPERKEY_HYPER_SOURCE
            | keys::HYPERKEY_INCLUDE_SHIFT_IN_HYPER
            | keys::HYPERKEY_MEH_ENABLED
            | keys::HYPERKEY_MEH_SOURCE
            | keys::HYPERKEY_BLEH_ENABLED
            | keys::HYPERKEY_BLEH_SOURCE
    )
}

/// `hyperkey.md` §4 / `preferences-ui.md` §3.1 실측값(pt). 탭 전환 시 창을 이
/// 크기로 리사이즈한다(D-E: 창 조작은 반드시 `on_main_thread` 로만).
fn tab_window_size(tab: &str) -> Option<(u32, u32)> {
    match tab {
        "seek" => Some((555, 378)),
        "hyperkey" => Some((710, 517)),
        "presets" => Some((825, 527)),
        // ⭐ **실측으로 확정했다**(명세 §9 #5 가 "구현 후 실제 렌더링 크기 측정"
        // 으로 남긴 자리). 이 탭의 마크업을 실제로 렌더해 잰 값:
        //   panel-korean scrollHeight = 416px · .panel 상하 패딩 = 40px ·
        //   타이틀바 = 28px  →  필요한 창 높이 = 484
        // 너비 613 은 항목이 한 줄에 들어가는 General 과 같아 그대로 쓴다.
        // ⚠️ 처음 잡았던 잠정치 400 은 **약 84px 모자라** 내용이 잘렸다 —
        // 지어낸 값을 쓰지 않는다는 규범이 실제로 값을 바꾼 사례다.
        //
        // ⓘ 관찰: 같은 방법으로 재면 `presets`(내용 690px, 창 527)와
        // `general`(내용 408px, 창 273)은 **내용이 창보다 크다.** 그 둘은
        // SuperKey v1.66 을 실측한 원본 창 크기를 그대로 쓰는 자리라
        // (preferences-ui.md §3.1) 클론의 마크업이 더 길어진 결과다.
        // ⛔ 이번 범위에서 고치지 않는다 — F-16 이 만든 문제가 아니다.
        "korean" => Some((613, 484)),
        "general" => Some((613, 273)),
        _ => None,
    }
}

/// hyperkey + presets + korean 세 설정 묶음을 합쳐 `EngineConfig` 하나로 조립하는
/// 단일 지점(위임 지시서 §4) — `Engine::reconfigure` 로 넘길 값은 항상 이 함수를 거친다.
fn build_engine_config(
    hyperkey: &HyperkeySettings,
    presets: &PresetSettings,
    korean: &KoreanSettings,
    store: &SettingsStore,
) -> EngineConfig {
    let mut config = EngineConfig::default();
    config.rules.modifier_rules = hyperkey.to_modifier_rules();
    config.mouse_apply = hyperkey.mouse_apply;
    config.timings.quick_press_duration_ms = presets.quick_press_duration_ms;

    let caps_is_source = caps_is_modifier_source(hyperkey);
    let preset_rules = presets.to_rules(caps_is_source);
    config.rules.combo_rules = preset_rules.combos;
    config.rules.simple_remaps = preset_rules.simple_remaps;
    config.rules.source_actions = preset_rules.source_actions;
    config.caps_lock_alias = compute_caps_lock_alias(presets, caps_is_source);
    // F-16 — `korean.disableInRemoteDesktop` 은 여기 들어오지 않는다(엔진 설정이
    // 아니라 게이트다, D-K3) — `gate_controller.set_korean_exclusion_enabled` 이 따로 처리한다.
    config.rules.korean_rules = korean.to_rules();
    // F-17 — `perDevice.*` 원본 스냅샷(`_managed` 원장 제외). 설정이 바뀔 때마다
    // 이 함수를 다시 거치므로 매번 저장소에서 새로 채운다(계약 §B.1).
    config.per_device_values = collect_per_device_values(store);

    config
}

/// 개별 필드 하나를 갱신한다. `key` 는 이미 `keys::all()` 멤버십 검사를 통과했다고
/// 가정한다(`validate_and_apply` 가 그 순서를 강제한다) — 그래도 이 함수가 모르는
/// 키(`ui.lastTab` 등, `settings_set_tab` 전용)가 들어오면 안전하게 거부한다.
fn apply_setting(
    hyperkey: &mut HyperkeySettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    fn parse<T: serde::de::DeserializeOwned>(
        value: &serde_json::Value,
        key: &str,
    ) -> Result<T, String> {
        serde_json::from_value(value.clone())
            .map_err(|e| format!("설정 값 타입이 맞지 않는다({key}): {e}"))
    }

    match key {
        keys::HYPERKEY_HYPER_ENABLED => hyperkey.hyper.enabled = parse(value, key)?,
        keys::HYPERKEY_HYPER_SOURCE => hyperkey.hyper.source = parse(value, key)?,
        keys::HYPERKEY_INCLUDE_SHIFT_IN_HYPER => {
            hyperkey.include_shift_in_hyper = parse(value, key)?
        }
        keys::HYPERKEY_MEH_ENABLED => hyperkey.meh.enabled = parse(value, key)?,
        keys::HYPERKEY_MEH_SOURCE => hyperkey.meh.source = parse(value, key)?,
        keys::HYPERKEY_BLEH_ENABLED => hyperkey.bleh.enabled = parse(value, key)?,
        keys::HYPERKEY_BLEH_SOURCE => hyperkey.bleh.source = parse(value, key)?,
        keys::HYPERKEY_MOUSE_APPLY_CLICK => hyperkey.mouse_apply.click = parse(value, key)?,
        keys::HYPERKEY_MOUSE_APPLY_DRAG => hyperkey.mouse_apply.drag = parse(value, key)?,
        keys::HYPERKEY_MOUSE_APPLY_MOVE => hyperkey.mouse_apply.r#move = parse(value, key)?,
        keys::HYPERKEY_MOUSE_APPLY_SCROLL => hyperkey.mouse_apply.scroll = parse(value, key)?,
        keys::HYPERKEY_TRACKPAD_ENABLED => hyperkey.trackpad.enabled = parse(value, key)?,
        keys::HYPERKEY_TRACKPAD_AREA => hyperkey.trackpad.area = parse(value, key)?,
        keys::HYPERKEY_TRACKPAD_CHANGE_MENU_BAR_ICON => {
            hyperkey.trackpad.change_menu_bar_icon = parse(value, key)?
        }
        keys::HYPERKEY_TRACKPAD_HAPTIC => hyperkey.trackpad.haptic_feedback = parse(value, key)?,
        _ => return Err(format!("{key} 는 이 커맨드로 바꿀 수 없다")),
    }
    Ok(())
}

/// `settings_set` 커맨드의 1~2단계(키 검증 + 메모리 갱신)만 담당하는 순수 함수 —
/// Tauri 상태·엔진·저장소 의존이 없어 유닛 테스트로 직접 부를 수 있다.
fn validate_and_apply(
    hyperkey: &mut HyperkeySettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    if !keys::all().contains(&key) {
        return Err(format!("알 수 없는 설정 키: {key}"));
    }
    apply_setting(hyperkey, key, value)
}

/// `apply_setting` 의 F-08(presets.*) 짝 — 16종 설정 + Advanced 토글 1개.
/// `presets.quickPressDurationMs` 는 `PresetSettings::from_store` 와 같은 유효
/// 범위(250~2000ms)로 클램프한다(F-08 §4).
fn apply_preset_setting(
    presets: &mut PresetSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    fn parse<T: serde::de::DeserializeOwned>(
        value: &serde_json::Value,
        key: &str,
    ) -> Result<T, String> {
        serde_json::from_value(value.clone())
            .map_err(|e| format!("설정 값 타입이 맞지 않는다({key}): {e}"))
    }

    match key {
        k if k == keys::PRESETS_CAPS_LOCK_REMAP_ENABLED => {
            presets.caps_lock_remap.enabled = parse(value, key)?
        }
        k if k == keys::PRESETS_CAPS_LOCK_REMAP_TARGET => {
            presets.caps_lock_remap.target = parse(value, key)?
        }
        k if k == keys::PRESETS_CAPS_QUICK_PRESS_ENABLED => {
            presets.caps_quick_press.enabled = parse(value, key)?
        }
        k if k == keys::PRESETS_CAPS_QUICK_PRESS_ACTION => {
            presets.caps_quick_press.action = parse(value, key)?
        }
        k if k == keys::PRESETS_QUICK_PRESS_DURATION_MS => {
            let raw: u64 = parse(value, key)?;
            presets.quick_press_duration_ms = raw.clamp(250, 2000);
        }
        k if k == keys::PRESETS_CAPS_SPACE_ENTER => presets.caps_space_enter = parse(value, key)?,
        k if k == keys::PRESETS_CAPS_WASD_ARROWS => presets.caps_wasd_arrows = parse(value, key)?,
        k if k == keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED => {
            presets.caps_hjkl_arrows.enabled = parse(value, key)?
        }
        k if k == keys::PRESETS_CAPS_HJKL_ARROWS_KEY_SET => {
            presets.caps_hjkl_arrows.key_set = parse(value, key)?
        }
        k if k == keys::PRESETS_CAPS_HOME_ROW_ENABLED => {
            presets.caps_home_row.enabled = parse(value, key)?
        }
        k if k == keys::PRESETS_CAPS_HOME_ROW_SCHEME => {
            presets.caps_home_row.scheme = parse(value, key)?
        }
        k if k == keys::PRESETS_DOUBLE_TAP_SHIFT_TO_CAPS => {
            presets.double_tap_shift_to_caps = parse(value, key)?
        }
        k if k == keys::PRESETS_LEFT_RIGHT_SHIFT_TO_CAPS => {
            presets.left_right_shift_to_caps = parse(value, key)?
        }
        k if k == keys::PRESETS_SHIFT_CAPS_TO_CAPS => presets.shift_caps_to_caps = parse(value, key)?,
        k if k == keys::PRESETS_SHIFT_QUICK_PRESS_BRACKETS_ENABLED => {
            presets.shift_quick_press_brackets.enabled = parse(value, key)?
        }
        k if k == keys::PRESETS_SHIFT_QUICK_PRESS_BRACKETS_PAIR => {
            presets.shift_quick_press_brackets.pair = parse(value, key)?
        }
        k if k == keys::PRESETS_HYPER_DELETE_TO_FORWARD => {
            presets.hyper_delete_to_forward = parse(value, key)?
        }
        k if k == keys::PRESETS_DELETE_TO_FORWARD => presets.delete_to_forward = parse(value, key)?,
        k if k == keys::PRESETS_SHIFT_DELETE_TO_FORWARD => {
            presets.shift_delete_to_forward = parse(value, key)?
        }
        k if k == keys::PRESETS_PASTE_WITHOUT_FORMATTING_ENABLED => {
            presets.paste_without_formatting.enabled = parse(value, key)?
        }
        k if k == keys::PRESETS_PASTE_WITHOUT_FORMATTING_TRIGGER => {
            presets.paste_without_formatting.trigger = parse(value, key)?
        }
        k if k == keys::PRESETS_HOME_END_ON_LINES => presets.home_end_on_lines = parse(value, key)?,
        k if k == keys::PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP => {
            presets.synthesize_caps_lock_remap = parse(value, key)?
        }
        _ => return Err(format!("{key} 는 이 커맨드로 바꿀 수 없다")),
    }
    Ok(())
}

fn validate_and_apply_preset(
    presets: &mut PresetSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    if !keys::all().contains(&key) {
        return Err(format!("알 수 없는 설정 키: {key}"));
    }
    apply_preset_setting(presets, key, value)
}

struct AppState {
    catalog: Catalog,
    /// 엔진은 권한이 생긴 뒤에야 시작된다 — 그전에는 `None`.
    engine: Mutex<Option<Engine>>,
    gate: Arc<AtomicAppGate>,
    gate_controller: Arc<AppGateController>,
    monitor: Mutex<Option<PermissionMonitor>>,
    /// F-15 저장 계층(`ultrakey_core::settings::SettingsStore`). 부팅 초기값은
    /// `in_memory()` 자리표시자이고, `setup()` 안에서 실제 `app_data_dir()` 경로로
    /// 교체된다 — `app_data_dir()` 은 실행 중인 앱 핸들이 있어야 얻을 수 있어
    /// `main()` 앞부분(state 생성 시점)에는 아직 없다.
    store: Mutex<SettingsStore>,
    /// 메모리 정본. 탭 스레드/엔진 커맨드마다 store 를 다시 역직렬화하지 않도록
    /// 캐시해 둔다 — `settings_set` 이 이 값과 store 를 함께 갱신한다.
    hyperkey: Mutex<HyperkeySettings>,
    /// F-08 Presets 탭의 메모리 정본. `hyperkey` 와 같은 캐시 규약을 쓴다.
    presets: Mutex<PresetSettings>,
    /// F-16 `Korean` 탭의 메모리 정본. `hyperkey`/`presets` 와 같은 캐시 규약을 쓴다.
    korean: Mutex<KoreanSettings>,
    /// 부트스트랩이 프런트엔드에 한 번만 알려줄 로드 경고(손상 복구/미래 스키마).
    load_notice: Mutex<Option<Notice>>,
    // ── F-10 메뉴바 상주(M2 2차) ──────────────────────────────────────────
    /// `NSStatusItem` 핸들. 드롭하면 아이콘이 사라지므로 앱 생애주기 내내 들고
    /// 있어야 한다. `setup_tray()` 가 채운다.
    tray: Mutex<Option<TrayIcon<Wry>>>,
    /// `Ignore <앱>` 항목 — 최전면 앱이 바뀔 때마다 라벨·체크 상태를 갱신해야 해서
    /// 따로 손잡이를 쥔다(`Menu` 는 항목별 개별 갱신 API 가 없다).
    ignore_item: Mutex<Option<CheckMenuItem<Wry>>>,
    /// 권한이 있을 때 보여주는 정상 메뉴 — 권한 전이 때마다 새로 만들지 않고
    /// 트레이의 메뉴만 이것/`unauthorized_menu` 로 갈아 끼운다.
    normal_menu: Mutex<Option<Menu<Wry>>>,
    /// `AXIsProcessTrusted() == false` 일 때 보여주는 2항목 메뉴(§3.1).
    unauthorized_menu: Mutex<Option<Menu<Wry>>>,
    /// 앱 계층 전용 `NSWorkspace` 구독(모듈 문서 5-b 참고). 드롭되면 구독이
    /// 해지되므로 앱 생애주기 내내 들고 있어야 한다 — 값 자체는 읽지 않는다.
    #[allow(dead_code)]
    system_event_observer: Mutex<Option<SystemEventObserver>>,
}

#[tauri::command]
fn modal_copy(state: State<'_, Arc<AppState>>) -> ModalCopy {
    let catalog = &state.catalog;
    let permission_state = state
        .monitor
        .lock()
        .ok()
        .and_then(|m| m.as_ref().map(|m| m.state()))
        .unwrap_or(PermissionState::Unknown);

    // ⭐ out-of-sync 는 온보딩과 다른 화면이다(`permissions-onboarding.md` §3.3).
    // ⛔ 자동 TCC 리셋 버튼은 없다 — §3.3·§7 이 채택하지 않기로 판정했다.
    let copy = if permission_state == PermissionState::OutOfSync {
        let c = out_of_sync_copy(catalog);
        ModalCopy {
            kind: "out_of_sync",
            title: c.title,
            body: c.body,
            path: String::new(),
            open_button: String::new(),
            checkbox_hint: String::new(),
            locked_hint: String::new(),
            manual_steps: c.manual_steps,
            quit: c.quit,
        }
    } else {
        let c = onboarding_copy(catalog);
        ModalCopy {
            kind: "onboarding",
            title: c.title,
            body: c.body,
            path: c.path,
            open_button: c.open_button,
            checkbox_hint: c.checkbox_hint,
            locked_hint: c.locked_hint,
            manual_steps: String::new(),
            quit: catalog.get("common.quit").to_string(),
        }
    };

    // ⭐ 프런트엔드가 실제로 invoke 에 성공했는지 판별하는 핵심 신호 — 이 로그가 없으면
    // 모달이 안 보이는 이유가 "커맨드가 실패했다" 인지 "창이 안 보인다" 인지 구분이 안 된다.
    tracing::info!(kind = copy.kind, "modal_copy 커맨드 호출됨");
    copy
}

#[tauri::command]
fn open_settings() -> bool {
    open_accessibility_settings()
}

/// ⭐ F-10 §2 시나리오 F — 정상 종료. 메뉴바 `Quit Ultrakey`(`on_menu_quit`)와
/// 이 커맨드(설정 창의 `Quit` 버튼)가 같은 절차(`shutdown_and_exit`)를 공유한다 —
/// 진입점이 둘이어도 순서(합성 modifier 해소 → 엔진 종료 → 프로세스 종료)는 하나다.
#[tauri::command]
fn quit_app(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) {
    shutdown_and_exit(&app, &state);
}

/// 환경설정 창 부트스트랩 — 카탈로그 전체 + 현재 설정 상태 + 앱 메타를 한 번에
/// 돌려준다(`preferences-ui.md`, ModalCopy 문서 주석과 같은 "단일 카탈로그" 근거).
#[tauri::command]
fn settings_bootstrap(state: State<'_, Arc<AppState>>, app: tauri::AppHandle) -> SettingsBootstrap {
    let catalog = &state.catalog;
    let hyperkey = state.hyperkey.lock().unwrap().clone();
    let presets = *state.presets.lock().unwrap();
    let korean = *state.korean.lock().unwrap();
    let store = state.store.lock().unwrap();
    let settings_state = build_settings_state(&hyperkey, &presets, &korean, &store, None, None);
    let meta = build_app_meta(&app, &store);
    drop(store);
    let notice = state.load_notice.lock().unwrap().clone();

    tracing::info!("settings_bootstrap 커맨드 호출됨");

    SettingsBootstrap {
        locale: catalog.locale().code().to_string(),
        strings: catalog.entries(),
        state: settings_state,
        meta,
        notice,
    }
}

/// 현재 저장된 값 그대로 `SettingsState` 를 다시 조립한다 — `general.*` 커맨드처럼
/// hyperkey/presets 를 건드리지 않는 변경 뒤에 새 상태를 돌려줄 때 쓴다.
fn current_settings_state(state: &Arc<AppState>) -> Result<SettingsState, String> {
    let hyperkey = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let presets = *state.presets.lock().map_err(|e| e.to_string())?;
    let korean = *state.korean.lock().map_err(|e| e.to_string())?;
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(&hyperkey, &presets, &korean, &store, None, None))
}

/// hyperkey.* 변경 뒤 `Engine::reconfigure` + (필요하면) `force_reset_state` 를
/// 함께 호출한다. presets.* 경로(`settings_set_preset`)와 이 함수를 공유해 엔진
/// 반영 로직이 두 곳에 흩어지지 않게 한다.
fn reconfigure_engine(
    state: &Arc<AppState>,
    hyperkey: &HyperkeySettings,
    presets: &PresetSettings,
    korean: &KoreanSettings,
    force_reset: bool,
) -> Result<(), String> {
    let engine_guard = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = engine_guard.as_ref() {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let config = build_engine_config(hyperkey, presets, korean, &store);
        drop(store);
        engine.reconfigure(config);
        if force_reset {
            engine.force_reset_state();
        }
    }
    // 엔진이 아직 없으면(권한 대기 중) 건너뛴다 — 다음 `Engine::start` 가 이미
    // 갱신된 `state.hyperkey`/`state.presets`/`state.korean` 으로 조립되므로 이
    // 변경이 유실되지 않는다.
    Ok(())
}

/// 컨트롤 하나가 바뀔 때마다 호출된다(§3.7 "적용 버튼 없음" — 즉시 반영).
///
/// `key` 접두사로 세 경로로 갈린다:
/// - `general.launchOnLogin`/`general.hideMenuBarIcon` — F-10 이 이미 만든
///   로직(`set_launch_on_login_internal`/`set_hide_menu_bar_icon_internal`)을
///   그대로 재사용한다. hyperkey/presets 도, 엔진도 건드리지 않는다.
/// - `presets.*` — [`settings_set_preset`](충돌 감지 → 적용 → 엔진 반영 → 저장).
/// - 그 밖(`hyperkey.*`) — 순서를 반드시 지킨다: 1) `key` 검증 2) 메모리 갱신
///   3) **엔진 반영**(저장 성공 여부와 무관하게 먼저 한다 — D-B: 로그아웃 시
///   graceful shutdown 이 실행되지 않는다는 M1 실측 근거) 4) **저장**(실패해도
///   3번은 이미 끝났다 — `saveError` 로 UI 에 알린다) 5) 새 `SettingsState` 반환.
#[tauri::command]
fn settings_set(
    state: State<'_, Arc<AppState>>,
    _app: tauri::AppHandle,
    key: String,
    value: serde_json::Value,
) -> Result<SettingsState, String> {
    if key == settings_keys::GENERAL_LAUNCH_ON_LOGIN {
        let on = value
            .as_bool()
            .ok_or_else(|| format!("{key} 는 bool 값이어야 한다"))?;
        set_launch_on_login_internal(&state, on)?;
        return current_settings_state(&state);
    }
    if key == settings_keys::GENERAL_HIDE_MENU_BAR_ICON {
        let on = value
            .as_bool()
            .ok_or_else(|| format!("{key} 는 bool 값이어야 한다"))?;
        set_hide_menu_bar_icon_internal(&state, on)?;
        return current_settings_state(&state);
    }

    if key.starts_with("presets.") {
        return settings_set_preset(&state, &key, &value);
    }

    if key.starts_with("korean.") {
        return settings_set_korean(&state, &key, &value);
    }

    if key.starts_with("perDevice.") {
        return settings_set_per_device(&state, &key, &value);
    }

    settings_set_hyperkey(&state, &key, &value)
}

/// F-17 §3.3 "공통 따름" — `settings_unset` 커맨드. `settings_set` 에 `value: null`
/// 을 보내는 것과 **다르다**: `null` 은 명시적 끔이고, 이 커맨드는 키 자체를
/// 지워 상위 계층(공통) 값을 따르게 한다(`settings.html` 851~866행 계약 —
/// `commitUnset`). 저장 뒤 절차는 `settings_set_per_device` 와 같다(엔진 반영 →
/// 새 `SettingsState`).
#[tauri::command]
fn settings_unset(state: State<'_, Arc<AppState>>, key: String) -> Result<SettingsState, String> {
    validate_per_device_key(&key)?;

    let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let presets_snapshot = *state.presets.lock().map_err(|e| e.to_string())?;
    let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;

    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        remove_setting_key(&mut store, &key)?;
    }

    reconfigure_engine(&state, &hyperkey_snapshot, &presets_snapshot, &korean_snapshot, false)?;

    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &store,
        None,
        None,
    ))
}

/// F-17 §3.5.1 — `시스템 설정 열기` 버튼. `(추정)` URL 스킴(명세 §3.5.1·§9) — 실패해도
/// 앱은 죽지 않고 프런트에 `Err` 로만 알린다(`settings.html` 의
/// `keyboards-open-system-settings-btn` 리스너가 `.catch()` 로 받는다).
#[tauri::command]
fn open_keyboard_settings() -> Result<(), String> {
    if bundle::open_url("x-apple.systempreferences:com.apple.preference.keyboard") {
        Ok(())
    } else {
        Err("키보드 시스템 설정을 열지 못했다".to_string())
    }
}

/// `settings_set` 의 `hyperkey.*` 경로. [`settings_resolve_conflict`] 도 이 함수를
/// 재사용한다(배타 대상이 hyper 슬롯일 수 있으므로).
///
/// ⭐ **충돌 감지는 여기서도 대칭으로 한다** — hyper/meh/bleh 슬롯을 caps lock 소스로
/// 켜려는데 `Remap caps lock to:`(F-08.1)가 이미 켜져 있으면 같은 대화상자를 띄운다.
/// 두 설정은 서로 다른 탭에 있어, 한쪽에서만 물어보면 사용자가 켜는 순서에 따라
/// 동작이 달라진다(architecture.md §6.5).
///
/// 순서를 반드시 지킨다: 1) 충돌 감지 2) `key` 검증 + 메모리 갱신 3) **엔진 반영**
/// (저장 성공 여부와 무관하게 먼저 — D-B) 4) **저장** 5) 새 `SettingsState`.
fn settings_set_hyperkey(
    state: &Arc<AppState>,
    key: &str,
    value: &serde_json::Value,
) -> Result<SettingsState, String> {
    // 1) 충돌 감지 — 적용 전에. caps lock 을 소스로 삼는 슬롯을 **켜는** 경우만 해당한다.
    if value.as_bool() == Some(true) && slot_key_would_claim_caps_lock(state, key)? {
        let presets_before = *state.presets.lock().map_err(|e| e.to_string())?;
        if let Some(conflict) = ultrakey_presets::detect_modifier_slot_conflict(&presets_before) {
            let pending = pending_conflict_view(conflict, key, value);
            let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
            let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;
            let store = state.store.lock().map_err(|e| e.to_string())?;
            return Ok(build_settings_state(
                &hyperkey_snapshot,
                &presets_before,
                &korean_snapshot,
                &store,
                None,
                Some(pending),
            ));
        }
    }

    // 2)
    let hyperkey_snapshot = {
        let mut hyperkey = state.hyperkey.lock().map_err(|e| e.to_string())?;
        validate_and_apply(&mut hyperkey, key, value)?;
        hyperkey.clone()
    };
    let presets_snapshot = *state.presets.lock().map_err(|e| e.to_string())?;
    let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;

    // 3) 엔진 반영.
    // D-D: 규칙이 바뀌는 변경은 stuck modifier 를 막기 위해 상태도 리셋한다.
    reconfigure_engine(
        state,
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        key_affects_modifier_rules(key),
    )?;

    // 4) 저장.
    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "설정 저장 실패");
                Some(e.to_string())
            }
        }
    };

    // 5) 새 SettingsState.
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &store,
        save_error,
        None,
    ))
}

/// 이 `hyperkey.*` 키를 켜면 그 슬롯이 caps lock 을 점유하게 되는가 —
/// 즉 슬롯 활성화 키이고 그 슬롯의 **현재 소스가 caps lock** 인가.
fn slot_key_would_claim_caps_lock(state: &Arc<AppState>, key: &str) -> Result<bool, String> {
    let h = state.hyperkey.lock().map_err(|e| e.to_string())?;
    Ok(match key {
        k if k == settings_keys::HYPERKEY_HYPER_ENABLED => h.hyper.source == SourceKey::CapsLock,
        k if k == settings_keys::HYPERKEY_MEH_ENABLED => h.meh.source == SourceKey::CapsLock,
        k if k == settings_keys::HYPERKEY_BLEH_ENABLED => h.bleh.source == SourceKey::CapsLock,
        _ => false,
    })
}

/// `settings_set` 의 `presets.*` 경로 — [`settings_resolve_conflict`] 도 이 함수를
/// 재사용한다(충돌 상대를 `value=false` 로 먼저 적용해 끄는 재귀 호출).
///
/// 1. `ultrakey_presets::detect_conflict` 로 충돌을 **적용 전에** 확인한다. 충돌이면
///    아무것도 저장·반영하지 않고 `pendingConflict` 를 채워 돌려준다.
/// 2. 메모리 `PresetSettings` 갱신.
/// 3. 엔진 반영(D-1 재계산 포함 — `build_engine_config`/`Engine::reconfigure` 가
///    caps lock alias 를 다시 계산한다) + `force_reset_state`(프리셋 규칙 변경도
///    hyper/meh/bleh 규칙 변경과 같은 이유로 stuck modifier 위험이 있다 — 추적
///    키 집합 자체가 바뀌기 때문이다, D-D 를 presets.* 로 확장).
/// 4. 저장(컨트롤 단위 즉시 write-through, F-15 §3.1.1).
/// 5. 새 `SettingsState` 반환.
fn settings_set_preset(
    state: &Arc<AppState>,
    key: &str,
    value: &serde_json::Value,
) -> Result<SettingsState, String> {
    let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let caps_slots = caps_modifier_slot_keys(&hyperkey_snapshot);

    // 1) 충돌 감지 — 적용하기 전에.
    if let Some(new_value) = value.as_bool() {
        let presets_before = *state.presets.lock().map_err(|e| e.to_string())?;
        if let Some(conflict) =
            ultrakey_presets::detect_conflict(&presets_before, &caps_slots, key, new_value)
        {
            let pending = pending_conflict_view(conflict, key, value);
            let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;
            let store = state.store.lock().map_err(|e| e.to_string())?;
            return Ok(build_settings_state(
                &hyperkey_snapshot,
                &presets_before,
                &korean_snapshot,
                &store,
                None,
                Some(pending),
            ));
        }
    }

    // 2)
    let presets_snapshot = {
        let mut presets = state.presets.lock().map_err(|e| e.to_string())?;
        validate_and_apply_preset(&mut presets, key, value)?;
        *presets
    };
    let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;

    // 3) 엔진 반영 — presets.* 변경은 항상 규칙 테이블을 바꾼다(단순 슬라이더도
    // `quick_press_duration_ms` 를 통해 FSM 타이밍에 영향을 준다) — 언제나
    // force_reset 한다.
    reconfigure_engine(state, &hyperkey_snapshot, &presets_snapshot, &korean_snapshot, true)?;

    // 4) 저장.
    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "설정 저장 실패");
                Some(e.to_string())
            }
        }
    };

    // 5)
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &store,
        save_error,
        None,
    ))
}

/// `settings_set` 의 `korean.*` 경로(F-16) — 충돌 감지가 없다는 점만 `settings_set_preset`
/// 과 다르다(korean-input.md 는 다른 설정과 배타 관계가 없다, 위임 지시서 §W-2 3).
///
/// 순서: 1) `key` 검증 + 메모리 갱신 2) **엔진 반영**(규칙 테이블이 바뀌므로 항상
/// force_reset — `settings_set_preset` 과 같은 이유) 3) ⭐ `korean.disableInRemoteDesktop`
/// 이 바뀌었다면 `gate_controller.set_korean_exclusion_enabled` 도 함께 호출한다 —
/// 이 설정은 엔진 규칙이 아니라 게이트다(D-K3, 잊기 쉬운 지점). 4) 저장 5) 새 `SettingsState`.
fn settings_set_korean(
    state: &Arc<AppState>,
    key: &str,
    value: &serde_json::Value,
) -> Result<SettingsState, String> {
    let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let presets_snapshot = *state.presets.lock().map_err(|e| e.to_string())?;

    // 1)
    let korean_snapshot = {
        let mut korean = state.korean.lock().map_err(|e| e.to_string())?;
        validate_and_apply_korean(&mut korean, key, value)?;
        *korean
    };

    // 2) 엔진 반영 — korean.* 변경은 항상 규칙 테이블을 바꾼다.
    reconfigure_engine(state, &hyperkey_snapshot, &presets_snapshot, &korean_snapshot, true)?;

    // 3) ⭐ 항목 5 는 게이트다 — 엔진 설정이 아니다(D-K3).
    if key == keys::KOREAN_DISABLE_IN_REMOTE_DESKTOP {
        state
            .gate_controller
            .set_korean_exclusion_enabled(korean_snapshot.disable_in_remote_desktop);
    }

    // 4) 저장.
    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "설정 저장 실패");
                Some(e.to_string())
            }
        }
    };

    // 5)
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &store,
        save_error,
        None,
    ))
}

/// `settings_set_korean` 의 1~2단계(키 검증 + 메모리 갱신)만 담당하는 순수 함수 —
/// `validate_and_apply`/`validate_and_apply_preset` 과 같은 형식.
fn apply_korean_setting(
    korean: &mut KoreanSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    fn parse<T: serde::de::DeserializeOwned>(
        value: &serde_json::Value,
        key: &str,
    ) -> Result<T, String> {
        serde_json::from_value(value.clone())
            .map_err(|e| format!("설정 값 타입이 맞지 않는다({key}): {e}"))
    }

    match key {
        k if k == keys::KOREAN_SHIFT_SPACE_SWITCHES_INPUT_SOURCE => {
            korean.shift_space_switches_input_source = parse(value, key)?
        }
        k if k == keys::KOREAN_HAN_ENG_SWITCHES_INPUT_SOURCE => {
            korean.han_eng_switches_input_source = parse(value, key)?
        }
        k if k == keys::KOREAN_HANJA_KEY_CONVERTS_HANJA => {
            korean.hanja_key_converts_hanja = parse(value, key)?
        }
        k if k == keys::KOREAN_WON_KEY_TYPES_BACKTICK => {
            korean.won_key_types_backtick = parse(value, key)?
        }
        k if k == keys::KOREAN_DISABLE_IN_REMOTE_DESKTOP => {
            korean.disable_in_remote_desktop = parse(value, key)?
        }
        _ => return Err(format!("{key} 는 이 커맨드로 바꿀 수 없다")),
    }
    Ok(())
}

fn validate_and_apply_korean(
    korean: &mut KoreanSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    if !keys::all().contains(&key) {
        return Err(format!("알 수 없는 설정 키: {key}"));
    }
    apply_korean_setting(korean, key, value)
}

/// 충돌 대화상자의 `계속` 버튼 — `settings_set_preset` 이 돌려준 `pendingConflict`
/// 를 사용자가 승인했을 때 호출된다(architecture.md §6.5). 충돌 상대를 **먼저**
/// 끄고(각각 write-through) 그다음 원래 값을 적용한다 — 두 단계 모두
/// `settings_set_preset` 을 그대로 재사용한다: 상대를 끄고 나면 그 다음 호출의
/// `detect_conflict` 는 이미 해소된 상태를 보므로 자연히 `None` 이 되어 실제
/// 적용으로 이어진다.
#[tauri::command]
fn settings_resolve_conflict(
    state: State<'_, Arc<AppState>>,
    key: String,
    value: serde_json::Value,
) -> Result<SettingsState, String> {
    let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let caps_slots = caps_modifier_slot_keys(&hyperkey_snapshot);
    let new_value = value
        .as_bool()
        .ok_or_else(|| format!("{key} 충돌 해소는 bool 값만 지원한다"))?;

    let to_disable: Vec<String> = {
        let presets_before = *state.presets.lock().map_err(|e| e.to_string())?;
        if key.starts_with("presets.") {
            ultrakey_presets::detect_conflict(&presets_before, &caps_slots, &key, new_value)
        } else if new_value {
            // ⭐ 반대 방향 — hyper/meh/bleh 슬롯을 caps lock 소스로 켜려는데
            // `Remap caps lock to:` 가 이미 켜져 있는 경우(대칭 처리).
            ultrakey_presets::detect_modifier_slot_conflict(&presets_before)
        } else {
            None
        }
        .map(|c| c.to_disable.iter().map(|s| s.to_string()).collect())
        .unwrap_or_default()
    };

    // ⭐ 배타 대상은 `presets.*` 일 수도 `hyperkey.*` 일 수도 있다 — 각각 자기 경로로
    // 끈다(엔진 반영·write-through 는 양쪽 경로가 이미 책임진다).
    for disable_key in &to_disable {
        let off = serde_json::Value::Bool(false);
        if disable_key.starts_with("presets.") {
            settings_set_preset(&state, disable_key, &off)?;
        } else {
            settings_set_hyperkey(&state, disable_key, &off)?;
        }
    }

    if key.starts_with("presets.") {
        settings_set_preset(&state, &key, &value)
    } else {
        settings_set_hyperkey(&state, &key, &value)
    }
}

/// 탭 전환 — 창 리사이즈(§3.1·§3.3, D-E) + (`persist` 일 때만) `ui.lastTab` 저장.
///
/// ⭐ **`persist` 인자가 왜 필요한가 — F-15 §8 수용 기준을 지키기 위해서다.**
/// 프런트엔드는 창을 처음 그릴 때도 이 커맨드를 불러 "마지막 탭의 크기"로 창을
/// 맞춰야 한다(§3.1: 탭마다 창 크기가 다르다). 그런데 그때도 `ui.lastTab` 을 쓰면
/// **환경설정 창을 열기만 해도 `settings.json` 이 생긴다** — "설정을 한 번도 건드리지
/// 않으면 저장 파일이 아예 생기지 않는다"(F-15 §8, §3.1)가 첫 실행에서 바로 깨진다.
/// 그래서 최초 렌더는 `persist: false`(리사이즈만), 사용자가 실제로 탭을 누르거나
/// 방향키로 옮긴 경우에만 `persist: true` 로 부른다.
///
/// 기각한 대안: "저장된 값과 다를 때만 쓴다" — 저장된 값이 **없을 때**(첫 실행)
/// 기본 탭을 쓰게 되므로 같은 문제가 그대로 남는다. 구분해야 하는 것은 값의 차이가
/// 아니라 **사용자의 의도적 조작인가**이고, 그것은 호출부만 알 수 있다.
#[tauri::command]
fn settings_set_tab(
    state: State<'_, Arc<AppState>>,
    app: tauri::AppHandle,
    tab: String,
    persist: bool,
) -> Result<(), String> {
    let size = tab_window_size(&tab).ok_or_else(|| format!("알 수 없는 탭: {tab}"))?;

    if persist {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        if let Err(e) = store.set(keys::UI_LAST_TAB, &tab) {
            tracing::error!(error = %e, "마지막 탭 저장 실패");
        }
    }

    resize_settings_window(&app, size);
    Ok(())
}

/// 로그 구독자를 세운다 — stderr 는 항상, 파일은 열 수 있을 때만 함께 남긴다.
///
/// `open Ultrakey.app` 으로 실행하면 stderr 가 사라져 아무 로그도 못 본다. 그래서
/// `~/Library/Logs/Ultrakey/ultrakey.log` 에도 같이 쓴다. 파일을 못 열어도(권한·디스크
/// 문제 등) **앱은 죽지 않는다** — stderr 만으로 계속 진행한다.
fn init_logging() {
    use tracing_subscriber::layer::SubscriberExt as _;
    use tracing_subscriber::util::SubscriberInitExt as _;

    let make_filter = || {
        tracing_subscriber::EnvFilter::try_from_env("ULTRAKEY_LOG")
            .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"))
    };

    // ⭐ **스레드 이름을 매 줄에 남긴다**(이슈 #10). 이 저장소의 버그 하나가
    // "어느 스레드가 시스템 훅을 등록했는가" 에 달려 있었는데, 로그에 그 정보가
    // 없어 로그만 보고는 원인을 좁힐 수 없었다. 스레드 이름은 그 질문에
    // 직접 답한다 — `main` / `ultrakey-tap` / `ultrakey-permission-poll` /
    // `ultrakey-hotplug` / `ultrakey-delay-scheduler` 가 로그에 그대로 찍힌다.
    let init_result = match open_log_file() {
        Some(file) => {
            let stderr_layer = tracing_subscriber::fmt::layer()
                .with_thread_names(true)
                .with_writer(std::io::stderr);
            // 파일 레이어는 ANSI 컬러 코드를 끈다 — 텍스트 에디터로 볼 로그다.
            let file_layer = tracing_subscriber::fmt::layer()
                .with_ansi(false)
                .with_thread_names(true)
                .with_writer(Arc::new(file));
            tracing_subscriber::registry()
                .with(make_filter())
                .with(stderr_layer)
                .with(file_layer)
                .try_init()
        }
        None => {
            let stderr_layer = tracing_subscriber::fmt::layer()
                .with_thread_names(true)
                .with_writer(std::io::stderr);
            tracing_subscriber::registry()
                .with(make_filter())
                .with(stderr_layer)
                .try_init()
        }
    };

    if init_result.is_err() {
        // 로그 구독자 자체를 못 세운 것 — tracing 매크로가 아무 데도 안 나갈 상황이므로
        // stderr 에 직접 남긴다.
        eprintln!("[ultrakey] 로그 구독자 초기화 실패 — 로그가 나오지 않을 수 있다");
    }
}

/// `~/Library/Logs/Ultrakey/ultrakey.log` 를 append 모드로 연다.
///
/// ⭐ 무한정 커지지 않게: 열기 전에 크기를 확인해 2 MiB 를 넘으면 `ultrakey.log.1` 로
/// rename 하고 새로 시작한다(단순 1세대 롤오버). 홈 디렉터리를 못 찾거나, 디렉터리를
/// 못 만들거나, 파일을 못 열면 `None` — 호출자는 stderr 만으로 계속한다.
fn open_log_file() -> Option<std::fs::File> {
    const MAX_LOG_BYTES: u64 = 2 * 1024 * 1024;

    let home = std::env::var_os("HOME")?;
    let dir = std::path::PathBuf::from(home).join("Library/Logs/Ultrakey");
    std::fs::create_dir_all(&dir).ok()?;

    let path = dir.join("ultrakey.log");
    if let Ok(meta) = std::fs::metadata(&path) {
        if meta.len() > MAX_LOG_BYTES {
            let _ = std::fs::rename(&path, dir.join("ultrakey.log.1"));
        }
    }

    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .ok()
}

fn main() {
    // 1) 로그 — 기본은 `info`, `ULTRAKEY_LOG=debug` 로 켠다. stderr 는 항상 나가고,
    //    `open Ultrakey.app` 처럼 stderr 가 사라지는 실행 경로를 위해
    //    `~/Library/Logs/Ultrakey/ultrakey.log` 에도 같이 남긴다.
    init_logging();
    tracing::info!(
        pid = std::process::id(),
        exe = ?std::env::current_exe(),
        "=== Ultrakey 기동 ==="
    );

    // 1-b) ⭐ 단일 인스턴스 보장(`menu-bar-and-lifecycle.md` §2 시나리오 D, §5
    // 항목 1, §8) — Tauri 를 아예 띄우기 전에 판정한다. 그래야 두 번째 프로세스가
    // 트레이 아이콘·엔진·`CGEventTap` 을 단 한 순간도 만들지 않는다. 종료는
    // 정리할 자원이 아무것도 없는 시점이라 `Engine::shutdown()` 같은 절차 없이
    // 바로 반환해도 안전하다.
    if bundle::other_instance_running() {
        tracing::warn!(
            "같은 번들 ID 로 이미 실행 중인 인스턴스가 있다 — 새 CGEventTap 을 설치하지 \
             않고 이 프로세스를 즉시 종료한다(§5 항목 1)"
        );
        return;
    }

    // 2) 로케일 → 카탈로그 (D4: ko + en)
    let catalog = Catalog::resolve(&bundle::preferred_languages());
    tracing::info!(locale = catalog.locale().code(), "문자열 카탈로그 로드됨");

    // ⭐ F-11 §8: `tauri dev` 산출물은 권한 검증에 쓸 수 없다는 경고.
    if let Some((title, body)) = dev_build_warning(&catalog) {
        tracing::warn!(%title, %body, "앱 번들 밖에서 실행 중 — 권한 기능을 검증할 수 없다");
    }

    let gate = Arc::new(AtomicAppGate::new());
    let gate_controller = Arc::new(AppGateController::new(gate.clone()));

    let state = Arc::new(AppState {
        catalog,
        engine: Mutex::new(None),
        gate: gate.clone(),
        gate_controller,
        monitor: Mutex::new(None),
        // setup() 이 실제 app_data_dir() 경로로 교체하기 전까지의 자리표시자.
        store: Mutex::new(SettingsStore::in_memory()),
        hyperkey: Mutex::new(HyperkeySettings::default()),
        presets: Mutex::new(PresetSettings::default()),
        korean: Mutex::new(KoreanSettings::default()),
        load_notice: Mutex::new(None),
        tray: Mutex::new(None),
        ignore_item: Mutex::new(None),
        normal_menu: Mutex::new(None),
        unauthorized_menu: Mutex::new(None),
        system_event_observer: Mutex::new(None),
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            modal_copy,
            open_settings,
            quit_app,
            settings_bootstrap,
            settings_set,
            settings_unset,
            settings_set_tab,
            settings_resolve_conflict,
            general_set_launch_on_login,
            general_set_hide_menu_bar_icon,
            open_keyboard_settings,
        ])
        .setup(move |app| {
            // 3) ⭐ Accessory 앱 — Dock 아이콘 없음, ⌘Tab 에 안 나타남.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // 4) ⭐ 설정 저장소 초기화(F-15 §3.1) — `app_data_dir()` 은 실행 중인
            // 앱 핸들이 있어야 얻을 수 있어 `main()` 앞부분이 아니라 여기서 한다.
            // ⛔ 디렉터리를 미리 만들지 않는다 — `SettingsStore::set()` 이 필요할
            // 때(내부 `persist()` 가) `create_dir_all` 을 호출한다. "설정을 안
            // 건드리면 파일이 안 생긴다"(§3.6 "부재 = 기본값")를 지키기 위함이다.
            let settings_path = match app.path().app_data_dir() {
                Ok(dir) => Some(dir.join("settings.json")),
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        "앱 데이터 디렉터리를 얻지 못함 — 설정을 메모리 전용으로 격하한다"
                    );
                    None
                }
            };
            let (settings_store, load_outcome) = match settings_path {
                Some(path) => SettingsStore::load(path),
                None => (SettingsStore::in_memory(), LoadOutcome::Fresh),
            };
            let hyperkey_settings = HyperkeySettings::from_store(&settings_store);
            for warning in hyperkey_settings.validate() {
                tracing::warn!(?warning, "Hyperkey 설정 경고(부팅 시점)");
            }
            *state.hyperkey.lock().unwrap() = hyperkey_settings;
            // ⭐ F-08 Presets — hyperkey 와 같은 "부재 = 기본값" 조립 규약.
            *state.presets.lock().unwrap() = PresetSettings::from_store(&settings_store);
            // ⭐ F-16 Korean — 같은 규약. `disable_in_remote_desktop` 만 부재 시 `true`
            // 로 읽힌다(D-K9 각주, `KoreanSettings::from_store` 가 처리한다).
            let korean_settings = KoreanSettings::from_store(&settings_store);
            *state.korean.lock().unwrap() = korean_settings;
            *state.load_notice.lock().unwrap() = notice_from_outcome(&load_outcome);
            // ⭐ F-10 §3.4 — 앱별 비활성화 목록을 여기서 복원한다. `settings_store` 를
            // `state.store` 로 옮기기 *전에* 이 지역 변수에서 직접 읽는다(둘 다 아직
            // 같은 값이지만, 옮긴 뒤에 다시 락을 잡는 왕복을 피한다).
            let disabled_apps: Vec<String> = settings_store
                .get(settings_keys::GENERAL_DISABLED_APPS)
                .unwrap_or_default();
            state.gate_controller.set_disabled_apps(disabled_apps);
            // ⭐ F-16 D-K3 — 한국어 전용 앱 제외 게이트. 목록은 `ultrakey-korean` 이
            // 소유하고(`default_excluded_bundle_ids`) 여기서 주입만 한다. 활성 여부는
            // `korean.disableInRemoteDesktop`(기본 `true`) 그대로 반영한다.
            state.gate_controller.set_korean_excluded_apps(
                ultrakey_korean::default_excluded_bundle_ids()
                    .iter()
                    .map(|s| s.to_string())
                    .collect(),
            );
            state
                .gate_controller
                .set_korean_exclusion_enabled(korean_settings.disable_in_remote_desktop);
            *state.store.lock().unwrap() = settings_store;

            let handle = app.handle().clone();
            let state_for_monitor = state.clone();

            // 5-b) ⭐ F-10 메뉴바(`NSStatusItem`) — 정상/`unauthorizedMenu` 두 벌을
            // 만들어 둔다. 권한 상태를 아직 모르니 안전한 기본값(`unauthorizedMenu`)
            // 으로 시작하고, 아래 최초 `on_permission_transition` 호출이 곧바로
            // 실제 상태에 맞는 메뉴로 갈아 끼운다.
            if let Err(e) = setup_tray(app.handle(), &state) {
                tracing::error!(error = %e, "메뉴바(NSStatusItem) 초기화 실패");
            }
            setup_front_app_tracking(app.handle(), &state);

            // 5) F-11 권한 감시. 전이가 오면 엔진을 켜거나 모달을 띄운다.
            let timings = EngineConfig::default().timings;
            let monitor = PermissionMonitor::start(
                timings.permission_poll_onboarding_ms,
                timings.permission_poll_background_ms,
                Box::new(move |transition| {
                    tracing::info!(?transition, "권한 상태 전이");
                    on_permission_transition(&handle, &state_for_monitor, transition.to);
                }),
            );

            let initial = monitor.check_now();
            *state.monitor.lock().unwrap() = Some(monitor);
            on_permission_transition(app.handle(), &state, initial);

            tracing::info!(
                windows = ?app.webview_windows().keys().collect::<Vec<_>>(),
                "setup() 완료 — 웹뷰 창 목록"
            );

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Tauri 앱을 초기화하지 못했다")
        .run(|_app_handle, event| match event {
            tauri::RunEvent::ExitRequested { code, api, .. } => {
                // §5 항목 7: "창을 닫아도 앱은 종료되지 않는다" — 메뉴바 상주 앱은
                // 명시적 `Quit`(메뉴 또는 설정 창 버튼, 둘 다 `shutdown_and_exit`)
                // 으로만 끝난다. `code` 가 `None` 이면 사용자가 마지막 창을 닫아
                // 발생한 암묵적 종료 요청이고, `Some` 이면 `shutdown_and_exit` 이
                // 부른 `AppHandle::exit()` 다(`tauri::App::exit` 문서 참고) — 전자만
                // 막는다. 후자 시점에는 엔진 정리(합성 modifier 해소·탭 해제)가
                // `shutdown_and_exit` 안에서 `app.exit()` 보다 **먼저** 이미 끝나
                // 있다 — 여기서 더 할 일이 없다.
                if code.is_none() {
                    tracing::info!("창 닫힘으로 인한 종료 요청 — 계속 실행한다");
                    api.prevent_exit();
                } else {
                    tracing::info!("종료 요청 — 엔진 정리는 이미 끝났다");
                }
            }
            tauri::RunEvent::Reopen { .. } => {
                // ⭐ M2 2차부터: 메뉴바 `Settings…` 가 창을 여는 정식 경로다
                // (M2 1차 임시 조치였던 자동 오픈은 걷어낸다 — 원래 계획대로).
                // Accessory 앱은 Dock 아이콘이 없어 이 이벤트가 사실상 발생하지
                // 않지만(§1), 발생하더라도 로그만 남긴다.
                tracing::debug!("Reopen 이벤트 수신 — Accessory 앱이라 창을 자동으로 열지 않는다");
            }
            _ => {}
        });
}

/// 권한 상태에 따라 엔진을 켜거나 모달을 띄운다.
fn on_permission_transition(handle: &tauri::AppHandle, state: &Arc<AppState>, to: PermissionState) {
    tracing::info!(?to, "on_permission_transition 진입");
    match to {
        PermissionState::Granted => {
            hide_modal(handle);
            start_engine_if_needed(handle, state);
            // ⭐ M2 2차부터: 메뉴바 `Settings…` 가 생겼으니 권한이 생겼다고 창을
            // 자동으로 띄우지 않는다(과거 M2 1차 임시 조치를 걷어낸다 — 원래
            // 계획대로다). 대신 트레이 메뉴를 정상 메뉴로 되돌린다(§3.1).
            apply_tray_menu_for_permission(handle, state, true);
        }
        PermissionState::Denied | PermissionState::OutOfSync | PermissionState::Unknown => {
            show_modal(handle);
            apply_tray_menu_for_permission(handle, state, false);
        }
    }
}

fn start_engine_if_needed(handle: &tauri::AppHandle, state: &Arc<AppState>) {
    let mut slot = match state.engine.lock() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "엔진 잠금 획득 실패");
            return;
        }
    };
    if slot.is_some() {
        return;
    }

    // ⭐ M2: store 에서 조립된 실제 사용자 설정을 쓴다 — M1 이 여기 두었던
    // `HyperkeySettings::default()` 하드코딩은 이제 걷어낸다(환경설정 UI 가
    // 생겼으므로 더 이상 유효하지 않은 전제였다). M2 2차부터 presets 도 함께
    // 조립한다 — D-1 caps lock alias 가 이미 필요한 상태로 기동할 수 있다.
    let hyperkey = state.hyperkey.lock().unwrap().clone();
    let presets = *state.presets.lock().unwrap();
    let korean = *state.korean.lock().unwrap();
    let config = {
        let store = state.store.lock().unwrap();
        build_engine_config(&hyperkey, &presets, &korean, &store)
    };

    // F-17 — `LedgerStore` 구현(`perDevice._managed`, 계약 §B.2). 엔진이 이것을
    // `PathBManager` 에 넘겨 디바이스별 원장을 영속화한다.
    let ledger: Box<dyn LedgerStore> = Box::new(AppLedgerStore { app_state: state.clone() });

    let handle_for_events = handle.clone();
    match Engine::start(
        config,
        state.gate.clone(),
        ledger,
        Box::new(move |event| on_engine_event(&handle_for_events, event)),
    ) {
        Ok(engine) => {
            tracing::info!(state = ?engine.tap_state(), "엔진 시작됨");
            *slot = Some(engine);
        }
        Err(e) => tracing::error!(error = %e, "엔진을 시작하지 못했다"),
    }
}

fn on_engine_event(handle: &tauri::AppHandle, event: EngineEvent) {
    match event {
        EngineEvent::TapStateChanged(s) => tracing::info!(state = ?s, "탭 상태 변경"),
        EngineEvent::NotTrusted => {
            // 권한이 없어 탭을 못 연 것은 정상 경로다 — F-11 온보딩이 처리한다.
            tracing::info!("권한 없음 — 온보딩 모달로 넘긴다");
            show_modal(handle);
        }
        EngineEvent::FatalTapCreateFailed => {
            // ⛔ `key-remapping-engine.md` §3-a·§5#16: 재시도가 아니라 종료다.
            tracing::error!("권한이 확인된 상태에서 탭 생성 실패 — 프로세스를 종료한다");
            show_modal(handle);
        }
        EngineEvent::NeedsRelaunch => {
            // §5#17 — 재활성화·재생성이 반복 실패. M1 은 로그만 남긴다(자동 재실행은
            // F-10/M2 의 `AppRelauncher` 소관).
            tracing::error!("탭 복구가 반복 실패했다 — 앱 재실행이 필요할 수 있다");
        }
    }
}

/// ⭐ **창 조작은 반드시 메인 스레드로 "비동기" 디스패치한다**(이슈 #10 후속).
///
/// ⛔ **왜 동기 호출이면 안 되는가 — 시스템 전체 입력이 멈춘다.**
///
/// `WebviewWindow::show()`/`set_focus()`/`set_size()`/`is_visible()` 류는 메인
/// 스레드가 아닌 곳에서 부르면 **메인 스레드로 동기 디스패치하고 응답을 기다린다.**
/// 그런데 이 함수의 호출자 중 하나는 **탭 전용 스레드**다 — `Engine::start` 에 넘긴
/// `on_event` 콜백이 `EngineEvent::NotTrusted` 를 탭 스레드에서 부르고
/// (`ultrakey-engine` 의 `handle_recreate_tap`), 그것이 여기로 이어진다.
///
/// 탭 스레드는 `CGEventTap` 의 mach port 를 서비스하는 **유일한** 스레드다. 그 스레드가
/// 메인 스레드를 기다리며 블록되면 그동안 탭이 이벤트를 처리하지 못하고, 활성 탭은
/// 모든 키·마우스 이벤트가 동기적으로 통과하는 지점이므로 **시스템 전체 입력이 멈춘다.**
/// 실제로 Accessibility 권한을 실행 중에 회수하면 이 경로로 머신이 멈췄다(실측: 로그에
/// 모달 표시와 탭 재활성화 사이 14초 공백).
///
/// ⚠️ PR #9 이 `ultrakey-permissions/monitor.rs` 에서 고친 것과 **같은 계열**이다 —
/// 그때는 폴링 스레드가 락을 쥔 채 메인 스레드를 기다리는 교착이었고, 이번엔 탭
/// 스레드가 메인 스레드를 기다리는 입력 정지다. 규칙은 하나로 정리된다:
/// **백그라운드 스레드에서 메인 스레드를 동기적으로 기다리지 마라.**
///
/// `AppHandle::run_on_main_thread` 는 이벤트 루프에 클로저를 **큐잉만 하고 즉시
/// 반환**한다 — 호출 스레드는 아무것도 기다리지 않으므로 위 문제가 구조적으로 사라진다.
/// 창 상태 진단 로그(이슈 #8 이 의존한다)는 클로저 **안**으로 옮겨 메인 스레드에서
/// 찍는다 — 그러면 그 조회들은 스레드 왕복이 아니라 지역 호출이 된다.
///
/// ⭐ M2 1차(F-09)에서 **창 라벨을 인자로 일반화했다** — M1 은 `"permissions"` 에
/// 고정되어 있었지만, 이제 `"settings"` 창도 같은 규약으로 조작해야 한다.
fn on_main_thread(
    handle: &tauri::AppHandle,
    window_label: &'static str,
    what: &'static str,
    action: impl FnOnce(&tauri::WebviewWindow) + Send + 'static,
) {
    let handle_for_closure = handle.clone();
    let dispatched = handle.run_on_main_thread(move || {
        match handle_for_closure.get_webview_window(window_label) {
            Some(w) => {
                tracing::info!(window_label, what, "창을 찾았다");
                action(&w);
                tracing::info!(
                    window_label,
                    what,
                    is_visible = ?w.is_visible(),
                    outer_position = ?w.outer_position(),
                    outer_size = ?w.outer_size(),
                    is_focused = ?w.is_focused(),
                    is_minimized = ?w.is_minimized(),
                    "창 조작 후 상태"
                );
            }
            None => {
                tracing::error!(window_label, what, "창을 찾지 못했다");
            }
        }
    });
    if let Err(e) = dispatched {
        tracing::error!(window_label, what, error = %e, "메인 스레드로 디스패치하지 못했다");
    }
}

fn show_modal(handle: &tauri::AppHandle) {
    on_main_thread(handle, "permissions", "show_modal", |w| {
        let _ = w.show();
        let _ = w.set_focus();
    });
}

fn hide_modal(handle: &tauri::AppHandle) {
    on_main_thread(handle, "permissions", "hide_modal", |w| {
        let _ = w.hide();
    });
}

/// ⭐ M2 1차(F-09) 임시 조치 — 메뉴바(F-10)가 아직 없어 환경설정 창을 열 다른
/// 경로가 없다. 그래서 이번 범위에 한해:
/// - 권한이 `Granted` 로 전이하면(`on_permission_transition`) 여기로 창을 띄운다.
/// - `RunEvent::Reopen`(Dock 아이콘 재클릭)에서도 여기로 다시 띄운다.
///
/// **F-10 이 `Settings…` 메뉴 항목을 넣으면 이 두 자동 오픈 경로는 걷어내고 메뉴
/// 클릭으로만 연다.** 그때까지는 이 함수가 유일한 진입점이다.
fn show_settings_window(handle: &tauri::AppHandle) {
    on_main_thread(handle, "settings", "show_settings", |w| {
        let _ = w.show();
        let _ = w.set_focus();
    });
}

fn resize_settings_window(handle: &tauri::AppHandle, size: (u32, u32)) {
    let (width, height) = size;
    on_main_thread(handle, "settings", "resize_settings", move |w| {
        let _ = w.set_size(LogicalSize::new(width as f64, height as f64));
    });
}

// ============================================================================
// F-10 메뉴바 상주(`NSStatusItem`) — M2 2차, `menu-bar-and-lifecycle.md`.
// ============================================================================

/// `Ignore <앱>` 항목의 라벨 — 최전면 앱을 모르면(`front_app.is_some() == false`,
/// 예: 기동 직후 아직 첫 `FrontAppChanged` 를 못 받았을 때) 이름 없이 안내만 한다
/// (`menu.ignore_app.none`). Tauri 의존이 없는 순수 함수라 단위 테스트로 직접 부른다.
fn ignore_menu_text(catalog: &Catalog, front_app: Option<&AppIdentity>) -> String {
    match front_app {
        Some(app) => catalog.format(menu_ids::IGNORE_APP, &[app.name.as_str()]),
        None => catalog.get("menu.ignore_app.none").to_string(),
    }
}

/// 정상 메뉴(§3.3, `docs/dev/architecture.md` §6.7 이 확정한 구성) 조립.
/// `Purchase`(F-12)·`Check for Updates…`(F-13)는 범위 밖이라 넣지 않는다.
fn build_normal_menu(
    handle: &tauri::AppHandle,
    catalog: &Catalog,
    front_app: Option<&AppIdentity>,
    front_app_disabled: bool,
    synth_caps_checked: bool,
) -> tauri::Result<(Menu<Wry>, CheckMenuItem<Wry>)> {
    let ignore_item = CheckMenuItem::with_id(
        handle,
        menu_ids::IGNORE_APP,
        ignore_menu_text(catalog, front_app),
        front_app.is_some(),
        front_app_disabled,
        None::<&str>,
    )?;
    let sep_top = PredefinedMenuItem::separator(handle)?;

    let settings_item = MenuItem::with_id(
        handle,
        menu_ids::SETTINGS,
        catalog.get(menu_ids::SETTINGS),
        true,
        None::<&str>,
    )?;
    // ⭐ 결정(위임 지시서): 별도 About 창을 새로 만들지 않는다 — 설정 창 General
    // 탭에 이미 About 정보 행(번들 ID·로그 경로·설정 파일 경로, `settings.general.
    // about.*`, 이슈 #13)이 있다. 이 메뉴 항목은 설정 창을 여는 것으로 구현한다.
    // ⚠️ 알려진 한계: `ui/settings.html` 이 이 위임과 동시에 다른 세션이 편집
    // 중이라, "General 탭으로 자동 전환 + About 섹션 자동 펼침"까지는 여기서
    // 배선하지 않았다 — 사용자가 창이 열리면 General 탭과 버전 버튼을 직접
    // 눌러야 한다. 후속 과제로 남긴다(최종 보고 참고).
    let about_item = MenuItem::with_id(
        handle,
        menu_ids::ABOUT,
        catalog.get(menu_ids::ABOUT),
        true,
        None::<&str>,
    )?;

    let synth_caps_item = CheckMenuItem::with_id(
        handle,
        menu_ids::SYNTHESIZE_CAPS_REMAP,
        catalog.get(menu_ids::SYNTHESIZE_CAPS_REMAP),
        true,
        synth_caps_checked,
        None::<&str>,
    )?;
    // `.app` 번들 밖(`tauri dev`)에서는 `open -n -b <bundle-id>` 로 재실행할 대상
    // 자체가 없다 — 항목을 비활성화한다(위임 지시서).
    let relaunch_enabled = bundle::is_running_from_app_bundle();
    let relaunch_item = MenuItem::with_id(
        handle,
        menu_ids::RELAUNCH,
        catalog.get(menu_ids::RELAUNCH),
        relaunch_enabled,
        None::<&str>,
    )?;
    let advanced_menu = Submenu::with_id_and_items(
        handle,
        menu_ids::ADVANCED,
        catalog.get(menu_ids::ADVANCED),
        true,
        &[&synth_caps_item, &relaunch_item],
    )?;

    let sep_bottom = PredefinedMenuItem::separator(handle)?;
    let quit_item = MenuItem::with_id(
        handle,
        menu_ids::QUIT,
        catalog.get(menu_ids::QUIT),
        true,
        None::<&str>,
    )?;

    let menu = Menu::with_items(
        handle,
        &[
            &ignore_item,
            &sep_top,
            &settings_item,
            &about_item,
            &advanced_menu,
            &sep_bottom,
            &quit_item,
        ],
    )?;

    Ok((menu, ignore_item))
}

/// `unauthorizedMenu`(§3.1) — 권한이 없을 때 메뉴 전체를 이 2항목으로 교체한다.
fn build_unauthorized_menu(handle: &tauri::AppHandle, catalog: &Catalog) -> tauri::Result<Menu<Wry>> {
    // 상태 안내는 클릭해도 아무 일도 일어나지 않는 비활성 항목이다.
    let status_item = MenuItem::new(handle, catalog.get("menu.unauthorized.title"), false, None::<&str>)?;
    let authorize_item = MenuItem::with_id(
        handle,
        menu_ids::AUTHORIZE,
        catalog.get(menu_ids::AUTHORIZE),
        true,
        None::<&str>,
    )?;
    Menu::with_items(handle, &[&status_item, &authorize_item])
}

/// `presets.synthesizeCapsLockRemap` 의 현재 값 — 기본값은 꺼짐(§7 판정: 원본
/// 디버깅용 스위치를 추정으로 켤 이유가 없다).
fn synthesize_caps_lock_remap_enabled(store: &SettingsStore) -> bool {
    store
        .get(keys::PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP)
        .unwrap_or(false)
}

/// 트레이(`NSStatusItem`)를 만들고 `AppState` 에 손잡이를 채운다. `setup()` 안에서
/// 한 번만 불린다.
fn setup_tray(handle: &tauri::AppHandle, state: &Arc<AppState>) -> tauri::Result<()> {
    let catalog = &state.catalog;

    // ⭐ 전용 모노크롬 template 아이콘을 쓴다(이슈 #16). 앱 번들 아이콘(둥근
    // 타일 + 밝은 글리프)을 그대로 재사용하던 과거 코드는 메뉴바에서 **타일
    // 전체가 불투명한 사각 실루엣**으로 뭉개졌다 — 템플릿 모드
    // (`icon_as_template`)에서 macOS 는 아이콘의 알파 채널만 남기고 현재 시스템
    // 외관(라이트/다크·메뉴 강조)에 맞는 단색으로 다시 칠하기 때문에, 색이 아니라
    // **알파 모양**이 곧 보이는 것 전부다.
    //
    // `menubar-template.png` 는 배경 없이 선화만 알파로 남긴 36×36 자산이다.
    // 36px 인 이유: `tray-icon` 0.24.2 가 NSImage 크기를 18pt 로 고정하므로
    // (platform_impl/macos/mod.rs `icon_height: f64 = 18.0`) Retina 에서 1:1 이
    // 되는 픽셀 크기가 36 이다. 생성 절차는 `scripts/generate-icons.sh`,
    // 디자인 근거는 `docs/dev/icons.md`.
    let icon_bytes = include_bytes!("../icons/menubar-template.png");
    // `Image::from_bytes` 는 `image-png` 기능이 있어야 존재하고(앱 Cargo.toml 에
    // 이미 켬), 실패하면 `tauri::Error::Image` 로 `?` 가 그대로 전파한다.
    let icon = tauri::image::Image::from_bytes(icon_bytes)?;

    let store = state.store.lock().unwrap();
    let synth_caps_checked = synthesize_caps_lock_remap_enabled(&store);
    let hide_menu_bar_icon: bool = store
        .get(settings_keys::GENERAL_HIDE_MENU_BAR_ICON)
        .unwrap_or(false);
    drop(store);

    let (normal_menu, ignore_item) =
        build_normal_menu(handle, catalog, None, false, synth_caps_checked)?;
    let unauthorized_menu = build_unauthorized_menu(handle, catalog)?;

    let state_for_events = state.clone();
    let tray = tauri::tray::TrayIconBuilder::new()
        .icon(icon)
        .icon_as_template(true)
        .menu(&unauthorized_menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            handle_menu_event(app, &state_for_events, event);
        })
        .build(handle)?;

    // §5 항목 2·§4 "Hide menu bar icon" — 마지막으로 저장된 값을 기동 시 반영한다.
    if hide_menu_bar_icon {
        if let Err(e) = tray.set_visible(false) {
            tracing::warn!(error = %e, "트레이 아이콘 숨김 반영 실패");
        }
    }

    *state.tray.lock().unwrap() = Some(tray);
    *state.ignore_item.lock().unwrap() = Some(ignore_item);
    *state.normal_menu.lock().unwrap() = Some(normal_menu);
    *state.unauthorized_menu.lock().unwrap() = Some(unauthorized_menu);

    Ok(())
}

/// 권한 상태 전이에 맞춰 트레이의 메뉴를 정상/`unauthorizedMenu` 로 갈아 끼운다.
///
/// ⭐ **메인 스레드로 비동기 디스패치한다** — 호출자(`on_permission_transition`)는
/// `PermissionMonitor` 의 배경 폴링 스레드에서도 불릴 수 있다(`show_modal`/
/// `hide_modal` 과 같은 이유, `on_main_thread` 문서 참고). `TrayIcon::set_menu` 는
/// Tauri 내부에서 메인 스레드로 동기 디스패치하고 응답을 기다리는데, 배경
/// 스레드에서 그걸 직접 부르면 그 스레드가 블록된다 — 탭 스레드만큼 치명적이진
/// 않지만 같은 규칙("백그라운드 스레드에서 메인 스레드를 동기적으로 기다리지
/// 마라")을 일관되게 지킨다.
fn apply_tray_menu_for_permission(handle: &tauri::AppHandle, state: &Arc<AppState>, granted: bool) {
    let state_for_closure = state.clone();
    let dispatched = handle.run_on_main_thread(move || {
        let Some(tray) = state_for_closure.tray.lock().unwrap().clone() else {
            return;
        };
        let menu = if granted {
            state_for_closure.normal_menu.lock().unwrap().clone()
        } else {
            state_for_closure.unauthorized_menu.lock().unwrap().clone()
        };
        if let Some(menu) = menu {
            if let Err(e) = tray.set_menu(Some(menu)) {
                tracing::warn!(error = %e, "트레이 메뉴 교체 실패");
            }
        }
    });
    if let Err(e) = dispatched {
        tracing::error!(error = %e, "트레이 메뉴 교체를 메인 스레드로 디스패치하지 못했다");
    }
}

/// 트레이 메뉴 클릭 처리. `TrayIconBuilder::on_menu_event` 콜백은 항상 메인
/// 스레드에서 불린다(AppKit 이 메뉴 클릭을 메인 런루프에서 전달한다) — 메뉴/트레이
/// 조작을 여기서 직접(비동기 디스패치 없이) 해도 안전하다.
fn handle_menu_event(app: &tauri::AppHandle, state: &Arc<AppState>, event: MenuEvent) {
    // `MenuId` 는 `pub struct MenuId(pub String)` 다(muda) — `.0.as_str()` 로 직접
    // 꺼내 쓰면 `AsRef` 구현 다중화로 인한 타입 추론 모호성 여지가 없다.
    match event.id().0.as_str() {
        menu_ids::IGNORE_APP => on_menu_ignore_app(state),
        menu_ids::SETTINGS | menu_ids::ABOUT => show_settings_window(app),
        menu_ids::SYNTHESIZE_CAPS_REMAP => on_menu_toggle_synthesize_caps_lock_remap(state),
        menu_ids::RELAUNCH => on_menu_relaunch(app, state),
        menu_ids::QUIT => on_menu_quit(app, state),
        menu_ids::AUTHORIZE => show_modal(app),
        other => tracing::debug!(id = other, "알 수 없는 메뉴 이벤트 id"),
    }
}

/// `Ignore <앱>` 클릭 — `AppGateController::toggle_front_app()` 을 호출하고, 결과
/// 목록을 `general.disabledApps` 로 즉시 원자적 write-through 한다(F-15 §3.1.1 —
/// 종료 시점 flush 에 기대지 않는다).
fn on_menu_ignore_app(state: &Arc<AppState>) {
    let now_disabled = state.gate_controller.toggle_front_app();
    tracing::info!(now_disabled, "Ignore <앱> 토글됨");
    persist_disabled_apps(state);
    refresh_ignore_menu_item(state);
}

fn persist_disabled_apps(state: &Arc<AppState>) {
    let bundle_ids = state.gate_controller.disabled_apps();
    let mut store = state.store.lock().unwrap();
    if let Err(e) = store.set(settings_keys::GENERAL_DISABLED_APPS, &bundle_ids) {
        tracing::error!(error = %e, "general.disabledApps 저장 실패");
    }
}

/// `Ignore <앱>` 항목의 라벨·체크 상태를 최전면 앱/게이트의 현재 값으로 되맞춘다.
fn refresh_ignore_menu_item(state: &Arc<AppState>) {
    let front_app = state.gate_controller.front_app();
    let disabled = state.gate.is_remapping_disabled();

    let item_guard = state.ignore_item.lock().unwrap();
    let Some(item) = item_guard.as_ref() else {
        return;
    };
    let _ = item.set_text(ignore_menu_text(&state.catalog, front_app.as_ref()));
    let _ = item.set_enabled(front_app.is_some());
    let _ = item.set_checked(disabled);
}

/// `Synthesize Caps Lock Remap` 클릭 — `presets.synthesizeCapsLockRemap` 를
/// 토글한다. ⚠️ 이 값을 실제로 읽어 경로 B 설치 여부를 바꾸는 쪽은 F-08(프리셋)
/// 소관이다 — 이 메뉴 항목은 `SettingsStore` 에 값을 쓰고 읽는 것까지만 한다
/// (위임 지시서). muda 는 클릭 시 항목의 체크 상태를 먼저 스스로 뒤집은 뒤에
/// 이벤트를 보내므로(objc2 macOS 백엔드 관찰), 여기서 다시 `set_checked` 를
/// 부를 필요가 없다 — 저장 값만 그 새 상태와 맞춰 주면 된다.
fn on_menu_toggle_synthesize_caps_lock_remap(state: &Arc<AppState>) {
    let next = {
        let store = state.store.lock().unwrap();
        !synthesize_caps_lock_remap_enabled(&store)
    };

    // ⭐ 1) 메모리 정본 갱신 + 2) 엔진 반영 (2026-08-30, 이슈 #19 / 증상 B).
    //
    // **이전 구현은 `SettingsStore` 에만 값을 썼다.** 위임 지시서가 "이 값을 실제로
    // 읽어 경로 B 설치 여부를 바꾸는 쪽은 F-08 소관" 이라고 경계를 그었던 것을,
    // "메뉴는 저장만 한다"로 좁게 구현한 결과다. 그래서 이 항목을 눌러도
    // `AppState::presets`(탭 스레드가 실제로 보는 정본)와 `EngineConfig::
    // caps_lock_alias` 가 그대로였고, **경로 B 커널 매핑이 토글과 어긋난 채 남았다.**
    //
    // 실측 근거(이슈 #19 로그): `05:39:52 Synthesize Caps Lock Remap 토글됨 value=true`
    // 직후인 `05:40:49` 의 경로 B 재적용이 여전히 `count=1` 이었다 — 토글이 매핑을
    // 전혀 바꾸지 못했다는 뜻이다. `architecture.md` §6.1 이 이 스위치에 부여한 역할
    // ("켜면 경로 B 를 설치하지 않고 경로 A 만 쓴다")이 성립하지 않았다.
    //
    // ⚠️ 락을 겹쳐 잡지 않는다 — `reconfigure_engine` 은 `state.engine` 을 잡으므로
    // `store`/`presets` 잠금을 놓은 뒤에 부른다.
    let presets_snapshot = {
        let mut presets = match state.presets.lock() {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = %e, "presets 정본 잠금 실패 — 토글을 반영하지 못했다");
                return;
            }
        };
        presets.synthesize_caps_lock_remap = next;
        *presets
    };
    let hyperkey_snapshot = match state.hyperkey.lock() {
        Ok(h) => h.clone(),
        Err(e) => {
            tracing::error!(error = %e, "hyperkey 정본 잠금 실패 — 토글을 반영하지 못했다");
            return;
        }
    };
    let korean_snapshot = match state.korean.lock() {
        Ok(k) => *k,
        Err(e) => {
            tracing::error!(error = %e, "korean 정본 잠금 실패 — 토글을 반영하지 못했다");
            return;
        }
    };
    // `force_reset = true` — alias 가 바뀌면 추적 키 집합(F18 ↔ caps lock)이 통째로
    // 바뀌므로, 규칙 변경과 같은 이유로 stuck modifier 위험이 있다(D-D).
    if let Err(e) = reconfigure_engine(state, &hyperkey_snapshot, &presets_snapshot, &korean_snapshot, true) {
        tracing::error!(error = %e, "Synthesize Caps Lock Remap 을 엔진에 반영하지 못했다");
    }

    // 3) 저장.
    {
        let mut store = state.store.lock().unwrap();
        if let Err(e) = store.set(keys::PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP, &next) {
            tracing::error!(error = %e, "presets.synthesizeCapsLockRemap 저장 실패");
        }
    }
    tracing::info!(value = next, "Synthesize Caps Lock Remap 토글됨 — 엔진·경로 B 에 반영했다");
}

/// `Relaunch` 클릭 — 현재 실행 파일을 `open -n -b <bundle-id>` 로 새로 띄우고
/// 자신은 정상 종료 절차(`shutdown_and_exit`)를 밟는다. `.app` 번들 밖에서
/// 실행 중이면 메뉴 항목 자체가 비활성이라(`build_normal_menu`) 여기 도달하지
/// 않는 것이 정상이지만, 방어적으로 한 번 더 확인한다.
fn on_menu_relaunch(app: &tauri::AppHandle, state: &Arc<AppState>) {
    if !bundle::is_running_from_app_bundle() {
        tracing::warn!(".app 번들 밖에서 실행 중이라 Relaunch 를 건너뛴다");
        return;
    }
    let Some(bundle_id) = bundle::bundle_identifier() else {
        tracing::warn!("번들 ID 를 얻지 못해 Relaunch 를 건너뛴다");
        return;
    };
    tracing::info!(bundle_id, "Relaunch 요청 — open -n -b 로 새 인스턴스를 띄운다");
    match std::process::Command::new("open").args(["-n", "-b", &bundle_id]).spawn() {
        Ok(_) => shutdown_and_exit(app, state),
        Err(e) => tracing::error!(error = %e, "Relaunch 를 위한 open 실행 실패 — 기존 인스턴스를 유지한다"),
    }
}

/// `Quit Ultrakey` 클릭.
fn on_menu_quit(app: &tauri::AppHandle, state: &Arc<AppState>) {
    tracing::info!("Quit Ultrakey 선택 — 정상 종료 절차를 시작한다");
    shutdown_and_exit(app, state);
}

/// F-10 §2 시나리오 F — 정상 종료 절차. (1) 합성 modifier 해소 (2) 엔진 종료
/// (3) 프로세스 종료. `quit_app` 커맨드와 메뉴의 `Quit Ultrakey` 가 이 함수를
/// 공유한다.
fn shutdown_and_exit(app: &tauri::AppHandle, state: &Arc<AppState>) {
    if let Ok(mut guard) = state.engine.lock() {
        if let Some(engine) = guard.take() {
            // `force_reset_state()`/`shutdown()` 은 둘 다 같은 탭 스레드가 순서대로
            // 처리하는 커맨드 큐에 넣는다(`command.rs`: "큐에 들어간 순서 그대로
            // 처리된다") — force_reset 을 shutdown 보다 먼저 보내면 modifier 해소가
            // 종료보다 먼저 끝난다는 것이 구조적으로 보장된다(`settings_set` 이 이미
            // 같은 순서 보장에 기대는 전례가 있다).
            engine.force_reset_state();
            engine.shutdown();
        }
    }
    app.exit(0);
}

/// F-10 §3.4 — 앱 계층 전용 `NSWorkspace` 구독. `AppState::system_event_observer`
/// 에 손잡이를 보관해 앱 생애주기 내내 살려 둔다(드롭되면 구독이 해지된다).
fn setup_front_app_tracking(handle: &tauri::AppHandle, state: &Arc<AppState>) {
    let handle_for_events = handle.clone();
    let state_for_events = state.clone();
    let observer = observe_system_events(Box::new(move |ev| {
        if let SystemEvent::FrontAppChanged(ident) = ev {
            on_front_app_changed(&handle_for_events, &state_for_events, ident);
        }
    }));
    *state.system_event_observer.lock().unwrap() = Some(observer);
}

/// `NSWorkspaceDidActivateApplicationNotification` 수신 — 게이트를 갱신하고
/// `Ignore <앱>` 라벨을 되맞춘다.
///
/// ⚠️ 이 콜백은 알림이 도착하는 스레드(관례상 메인 스레드, `ultrakey_platform::
/// workspace` 모듈 문서)에서 불린다 — 메뉴 항목 갱신을 직접(비동기 디스패치 없이)
/// 해도 안전하다. 판정(`bundle_id ∈ disabledApps`) 자체는 `AppGateController::
/// set_front_app` 이 즉시 `AtomicBool` 에 게시한다(`docs/dev/architecture.md`
/// §2.3) — 콜백 임계 경로(탭 스레드)는 이 함수와 전혀 만나지 않는다.
fn on_front_app_changed(_handle: &tauri::AppHandle, state: &Arc<AppState>, ident: Option<AppIdentity>) {
    state.gate_controller.set_front_app(ident);
    refresh_ignore_menu_item(state);
}

// ============================================================================
// F-10 §3.5 — General 탭 백엔드(Launch on login / Hide menu bar icon).
//
// `ui/settings.html` 은 두 체크박스를 `data-key="general.launchOnLogin"`/
// `"general.hideMenuBarIcon"` 로 이미 배선해 두었다 — 값은 범용 `commit(key,
// value)` 경로를 거쳐 `settings_set` 커맨드로 들어온다(전용 커맨드를 직접 부르지
// 않는다). 그래서 실제 로직은 여기 `*_internal` 두 함수에 두고, `settings_set` 과
// 아래 전용 `#[tauri::command]` 양쪽이 그 함수를 부른다 — 로직을 복제하지 않는다.
// ============================================================================

/// `Launch on login` 체크박스 — `login_item::set_enabled` 로 OS 에 등록/해제하고,
/// 결과와 무관하게 사용자의 마지막 의도를 `general.launchOnLogin` 에 기록한다.
///
/// ⚠️ `login_item::set_enabled` 는 상한 있는 재시도(§5 항목 3, 최대 5회·0.2초
/// 간격 — 최악 약 0.8초)로 **동기 블로킹**한다. Tauri 커맨드 핸들러는 메인
/// 스레드가 아닌 별도 스레드에서 실행되므로(Tauri 의 IPC 커맨드 디스패치 자체가
/// 그렇게 되어 있다) 여기서 블로킹해도 UI 는 멎지 않는다 — 다만 이 함수를 메인
/// 스레드에서 직접 부르는 새 경로가 생기면 반드시 스레드를 분리해야 한다
/// (`login_item` 모듈 문서 참고).
fn set_launch_on_login_internal(state: &Arc<AppState>, on: bool) -> Result<(), String> {
    let result = login_item::set_enabled(on);

    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        if let Err(e) = store.set(settings_keys::GENERAL_LAUNCH_ON_LOGIN, &on) {
            tracing::error!(error = %e, "general.launchOnLogin 저장 실패");
        }
    }

    result.map_err(|e| {
        tracing::error!(error = %e, on, "로그인 항목 등록/해제 실패");
        state.catalog.get("menu.launch_on_login.failed").to_string()
    })
}

/// F-10 이 이미 등록해 둔 전용 커맨드 — 실제 OS 상태(`login_item::is_enabled()`)를
/// 돌려준다. `settings_set` 은 이 반환값 대신 저장된 마지막 의도를 거울처럼
/// 보여주는 `SettingsState.general.launchOnLogin` 을 쓴다(둘의 용도가 다르다).
#[tauri::command]
fn general_set_launch_on_login(state: State<'_, Arc<AppState>>, on: bool) -> Result<bool, String> {
    set_launch_on_login_internal(&state, on)?;
    Ok(login_item::is_enabled())
}

/// `Hide menu bar icon` 체크박스 — 저장 후 트레이 가시성을 즉시 반영한다.
fn set_hide_menu_bar_icon_internal(state: &Arc<AppState>, on: bool) -> Result<(), String> {
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        store
            .set(settings_keys::GENERAL_HIDE_MENU_BAR_ICON, &on)
            .map_err(|e| e.to_string())?;
    }
    let tray_guard = state.tray.lock().map_err(|e| e.to_string())?;
    if let Some(tray) = tray_guard.as_ref() {
        tray.set_visible(!on).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn general_set_hide_menu_bar_icon(state: State<'_, Arc<AppState>>, on: bool) -> Result<(), String> {
    set_hide_menu_bar_icon_internal(&state, on)
}

#[cfg(test)]
mod tests {
    use super::*;

    // tab_window_size() — `preferences-ui.md` §3.1 실측값과 정확히 일치해야 한다.
    #[test]
    fn tab_window_size_matches_spec_values() {
        assert_eq!(tab_window_size("seek"), Some((555, 378)));
        assert_eq!(tab_window_size("hyperkey"), Some((710, 517)));
        assert_eq!(tab_window_size("presets"), Some((825, 527)));
        assert_eq!(tab_window_size("korean"), Some((613, 484)));
        assert_eq!(tab_window_size("general"), Some((613, 273)));
        assert_eq!(tab_window_size("bogus"), None);
    }

    // validate_and_apply() — keys::all() 에 없는 키는 거부된다.
    #[test]
    fn validate_and_apply_rejects_unknown_key() {
        let mut hyperkey = HyperkeySettings::default();
        let err = validate_and_apply(&mut hyperkey, "not.a.real.key", &serde_json::json!(true))
            .unwrap_err();
        assert!(err.contains("not.a.real.key"));
        // 거부된 키는 메모리 상태를 건드리지 않는다.
        assert_eq!(hyperkey, HyperkeySettings::default());
    }

    // validate_and_apply() — 알려진 키는 값을 실제로 갱신한다.
    #[test]
    fn validate_and_apply_updates_known_key() {
        let mut hyperkey = HyperkeySettings::default();
        validate_and_apply(
            &mut hyperkey,
            keys::HYPERKEY_HYPER_ENABLED,
            &serde_json::json!(true),
        )
        .unwrap();
        assert!(hyperkey.hyper.enabled);
    }

    // validate_and_apply() — 소스 키 팝업은 variant 이름 문자열로 갱신된다.
    #[test]
    fn validate_and_apply_updates_source_key() {
        let mut hyperkey = HyperkeySettings::default();
        validate_and_apply(
            &mut hyperkey,
            keys::HYPERKEY_MEH_SOURCE,
            &serde_json::json!("RightOption"),
        )
        .unwrap();
        assert_eq!(hyperkey.meh.source, SourceKey::RightOption);
    }

    // validate_and_apply() — 타입이 맞지 않으면 거부되고 메모리 상태는 그대로다.
    #[test]
    fn validate_and_apply_rejects_type_mismatch() {
        let mut hyperkey = HyperkeySettings::default();
        let err = validate_and_apply(
            &mut hyperkey,
            keys::HYPERKEY_HYPER_ENABLED,
            &serde_json::json!("아니오"),
        )
        .unwrap_err();
        assert!(err.contains(keys::HYPERKEY_HYPER_ENABLED));
        assert_eq!(hyperkey, HyperkeySettings::default());
    }

    // validate_and_apply() — ui.lastTab 은 이 커맨드로 바꿀 수 없다(전용 커맨드가 따로 있다).
    #[test]
    fn validate_and_apply_rejects_ui_last_tab() {
        let mut hyperkey = HyperkeySettings::default();
        let err = validate_and_apply(
            &mut hyperkey,
            keys::UI_LAST_TAB,
            &serde_json::json!("general"),
        )
        .unwrap_err();
        assert!(err.contains(keys::UI_LAST_TAB));
    }

    // key_affects_modifier_rules() — hyper/meh/bleh 관련 키만 true.
    #[test]
    fn key_affects_modifier_rules_covers_only_slot_keys() {
        assert!(key_affects_modifier_rules(keys::HYPERKEY_HYPER_ENABLED));
        assert!(key_affects_modifier_rules(keys::HYPERKEY_HYPER_SOURCE));
        assert!(key_affects_modifier_rules(
            keys::HYPERKEY_INCLUDE_SHIFT_IN_HYPER
        ));
        assert!(key_affects_modifier_rules(keys::HYPERKEY_MEH_ENABLED));
        assert!(key_affects_modifier_rules(keys::HYPERKEY_BLEH_SOURCE));
        assert!(!key_affects_modifier_rules(keys::HYPERKEY_MOUSE_APPLY_CLICK));
        assert!(!key_affects_modifier_rules(keys::HYPERKEY_TRACKPAD_ENABLED));
        assert!(!key_affects_modifier_rules(keys::UI_LAST_TAB));
    }

    // build_engine_config() — HyperkeySettings 의 규칙·마우스 적용 범위를 그대로 옮긴다.
    #[test]
    fn build_engine_config_reflects_hyperkey_settings() {
        let mut hyperkey = HyperkeySettings::default();
        hyperkey.hyper.enabled = true;
        hyperkey.mouse_apply.drag = true;

        let config = build_engine_config(
            &hyperkey,
            &PresetSettings::default(),
            &KoreanSettings::default(),
            &SettingsStore::in_memory(),
        );
        assert_eq!(config.rules.modifier_rules.len(), 1);
        assert!(config.mouse_apply.drag);
        assert!(config.mouse_apply.click); // 기본값 유지
    }

    // build_engine_config() — presets.* 도 규칙 테이블·quick_press_duration_ms 로
    // 옮겨진다.
    #[test]
    fn build_engine_config_reflects_preset_settings() {
        let hyperkey = HyperkeySettings::default();
        let presets = PresetSettings { caps_space_enter: true, quick_press_duration_ms: 500, ..PresetSettings::default() };

        let config = build_engine_config(
            &hyperkey,
            &presets,
            &KoreanSettings::default(),
            &SettingsStore::in_memory(),
        );
        assert_eq!(config.rules.combo_rules.len(), 1);
        assert_eq!(config.timings.quick_press_duration_ms, 500);
    }

    // build_engine_config() — D-1: caps lock 프리셋이 하나라도 켜지면 F18 alias 가
    // 채워진다. synthesize_caps_lock_remap 이 켜지면 alias 를 다시 끈다.
    #[test]
    fn build_engine_config_computes_d1_caps_lock_alias() {
        let hyperkey = HyperkeySettings::default();

        let none_needed = build_engine_config(
            &hyperkey,
            &PresetSettings::default(),
            &KoreanSettings::default(),
            &SettingsStore::in_memory(),
        );
        assert_eq!(none_needed.caps_lock_alias, None);

        let needs_alias = PresetSettings { caps_space_enter: true, ..PresetSettings::default() };
        let with_alias = build_engine_config(
            &hyperkey,
            &needs_alias,
            &KoreanSettings::default(),
            &SettingsStore::in_memory(),
        );
        assert_eq!(with_alias.caps_lock_alias, Some(KeyCode::F18));

        let synthesize_on = PresetSettings {
            caps_space_enter: true,
            synthesize_caps_lock_remap: true,
            ..PresetSettings::default()
        };
        let with_synthesize = build_engine_config(
            &hyperkey,
            &synthesize_on,
            &KoreanSettings::default(),
            &SettingsStore::in_memory(),
        );
        assert_eq!(with_synthesize.caps_lock_alias, None);
    }

    // build_engine_config() — F-16: korean.* 이 규칙 테이블에 실제로 반영된다.
    #[test]
    fn build_engine_config_reflects_korean_settings() {
        let korean = KoreanSettings {
            shift_space_switches_input_source: true,
            ..KoreanSettings::default()
        };
        let config = build_engine_config(
            &HyperkeySettings::default(),
            &PresetSettings::default(),
            &korean,
            &SettingsStore::in_memory(),
        );
        assert_eq!(config.rules.korean_rules.len(), 1);
    }

    // build_engine_config() — F-17: `perDevice.*` 키만 스냅샷에 실리고 `_managed`
    // 원장은 제외된다(계약 §B.1).
    #[test]
    fn build_engine_config_collects_per_device_values_excluding_managed_ledger() {
        let mut store = SettingsStore::in_memory();
        store
            .set(&keys::per_device_key_remap_rows(keys::PER_DEVICE_COMMON_SCOPE), &serde_json::json!([]))
            .unwrap();
        store.set(keys::PER_DEVICE_MANAGED, &serde_json::json!({})).unwrap();

        let config = build_engine_config(
            &HyperkeySettings::default(),
            &PresetSettings::default(),
            &KoreanSettings::default(),
            &store,
        );

        assert!(config
            .per_device_values
            .contains_key(&keys::per_device_key_remap_rows(keys::PER_DEVICE_COMMON_SCOPE)));
        assert!(!config.per_device_values.contains_key(keys::PER_DEVICE_MANAGED));
    }

    // F-17 — validate_per_device_key() §3.3 이 정한 두 모양(keyRemap.rows·
    // functionKeys.f1~f12)만 통과시킨다. `_managed` 원장 키는 이 경로로 건드릴 수
    // 없다(계약 §B.2, D-17-2).
    #[test]
    fn validate_per_device_key_accepts_key_remap_and_function_key_shapes() {
        assert!(validate_per_device_key("perDevice.all.keyRemap.rows").is_ok());
        assert!(validate_per_device_key("perDevice.5ac:24f.keyRemap.rows").is_ok());
        assert!(validate_per_device_key("perDevice.all.functionKeys.f1").is_ok());
        assert!(validate_per_device_key("perDevice.5ac:24f.functionKeys.f12").is_ok());
    }

    #[test]
    fn validate_per_device_key_rejects_managed_ledger_key() {
        assert!(validate_per_device_key(keys::PER_DEVICE_MANAGED).is_err());
    }

    #[test]
    fn validate_per_device_key_rejects_bad_device_scope_and_unknown_tail() {
        // ⭐ 콜론이 없는 스코프는 "all" 도 유효한 디바이스 id 도 아니다.
        assert!(validate_per_device_key("perDevice.notADevice.keyRemap.rows").is_err());
        // ⭐ FKey 는 f1~f12 뿐이다 — f13 은 기능 2 의 슬롯이 아니다(§3.5).
        assert!(validate_per_device_key("perDevice.all.functionKeys.f13").is_err());
        assert!(validate_per_device_key("hyperkey.hyper.enabled").is_err());
    }

    // F-17 — collect_per_device_values() 는 `perDevice.` 접두사 키만 원본 JSON
    // 그대로 모으고 `_managed` 원장은 제외한다. `null` 값(명시적 끔, §3.3)도 그대로
    // 실린다 — 부재와 구별돼야 하기 때문이다.
    #[test]
    fn collect_per_device_values_includes_null_and_excludes_managed_ledger() {
        let mut store = SettingsStore::in_memory();
        store
            .set(&keys::per_device_function_key("all", FKey::F1), &serde_json::Value::Null)
            .unwrap();
        store.set(keys::PER_DEVICE_MANAGED, &serde_json::json!({})).unwrap();
        store.set(settings_keys::GENERAL_LAUNCH_ON_LOGIN, &true).unwrap();

        let values = collect_per_device_values(&store);

        assert_eq!(
            values.get(&keys::per_device_function_key("all", FKey::F1)),
            Some(&serde_json::Value::Null)
        );
        assert!(!values.contains_key(keys::PER_DEVICE_MANAGED));
        assert!(!values.contains_key(settings_keys::GENERAL_LAUNCH_ON_LOGIN));
    }

    // F-17 — system_function_label_key() 는 §4.1 카탈로그의 camelCase 세그먼트와
    // 정확히 맞아떨어져야 한다(실제 en.json 키와 대조).
    #[test]
    fn system_function_label_key_matches_catalog_segments() {
        assert_eq!(
            system_function_label_key(SystemFunction::Mute),
            "preferences.keyboards.functionKeys.function.mute"
        );
        assert_eq!(
            system_function_label_key(SystemFunction::DoNotDisturb),
            "preferences.keyboards.functionKeys.function.doNotDisturb"
        );
        assert_eq!(
            system_function_label_key(SystemFunction::DisplayBrightnessDown),
            "preferences.keyboards.functionKeys.function.displayBrightnessDown"
        );
    }

    // F-17 — build_per_device_view() 의 systemFunctions 는 hid_usage() 가 Some 인
    // 8종만 낸다(CONTRACT §2.2 — MissionControl·Spotlight·Dictation·DoNotDisturb 는
    // 팝업에서 빠진다).
    #[test]
    fn build_per_device_view_system_functions_excludes_four_unknown_functions() {
        let view = build_per_device_view(&SettingsStore::in_memory());
        assert_eq!(view.system_functions.len(), 8);
        let values: Vec<&str> = view.system_functions.iter().map(|f| f.value.as_str()).collect();
        assert!(!values.contains(&"MissionControl"));
        assert!(!values.contains(&"Spotlight"));
        assert!(!values.contains(&"Dictation"));
        assert!(!values.contains(&"DoNotDisturb"));
    }

    // caps_is_modifier_source() — hyper/meh/bleh 중 활성화된 슬롯만 본다.
    /// ⭐ 충돌 대화상자 #1 의 배타 대상은 **caps lock 을 점유한 슬롯**이지 지금 켜려는
    /// 설정 자신이 아니다 — 실기기 검증에서 잡은 회귀의 재발 방지.
    #[test]
    fn caps_modifier_slot_keys_lists_only_enabled_caps_lock_slots() {
        let mut h = HyperkeySettings::default();
        assert!(caps_modifier_slot_keys(&h).is_empty(), "기본값은 전부 꺼져 있다");

        h.hyper.enabled = true;
        h.hyper.source = SourceKey::CapsLock;
        assert_eq!(caps_modifier_slot_keys(&h), vec![settings_keys::HYPERKEY_HYPER_ENABLED]);

        // 소스가 caps lock 이 아니면 세지 않는다.
        h.hyper.source = SourceKey::RightCommand;
        assert!(caps_modifier_slot_keys(&h).is_empty());

        // 여러 슬롯이 동시에 caps lock 을 쓰면 전부 나열한다.
        h.hyper.source = SourceKey::CapsLock;
        h.meh.enabled = true;
        h.meh.source = SourceKey::CapsLock;
        assert_eq!(
            caps_modifier_slot_keys(&h),
            vec![settings_keys::HYPERKEY_HYPER_ENABLED, settings_keys::HYPERKEY_MEH_ENABLED]
        );
    }

    /// 배타 대상의 라벨 키가 실제로 카탈로그에 있는 것이어야 한다.
    #[test]
    fn disable_label_key_covers_modifier_slots() {
        for (store_key, expected) in [
            (settings_keys::HYPERKEY_HYPER_ENABLED, "settings.hyperkey.hyper.label"),
            (settings_keys::HYPERKEY_MEH_ENABLED, "settings.hyperkey.meh.label"),
            (settings_keys::HYPERKEY_BLEH_ENABLED, "settings.hyperkey.bleh.label"),
        ] {
            assert_eq!(disable_label_key_for_setting(store_key), expected);
        }
    }

    #[test]
    fn caps_is_modifier_source_only_counts_enabled_slots() {
        let mut hyperkey = HyperkeySettings::default();
        assert!(!caps_is_modifier_source(&hyperkey));

        hyperkey.hyper.source = SourceKey::CapsLock;
        assert!(!caps_is_modifier_source(&hyperkey), "꺼진 슬롯은 세지 않는다");

        hyperkey.hyper.enabled = true;
        assert!(caps_is_modifier_source(&hyperkey));
    }

    // compute_caps_lock_alias() — needs_caps_lock_alias × synthesize 조합.
    #[test]
    fn compute_caps_lock_alias_matrix() {
        assert_eq!(compute_caps_lock_alias(&PresetSettings::default(), false), None);
        assert_eq!(compute_caps_lock_alias(&PresetSettings::default(), true), Some(KeyCode::F18));

        let synthesize = PresetSettings { synthesize_caps_lock_remap: true, ..PresetSettings::default() };
        assert_eq!(compute_caps_lock_alias(&synthesize, true), None);
    }

    // preset_options_view() — 팝업 6종 개수(50/48/2/2/4/4)와 labelKey 배정.
    #[test]
    fn preset_options_view_has_expected_counts_and_label_keys() {
        let opts = preset_options_view();
        assert_eq!(opts.caps_remap_targets.len(), 50);
        assert_eq!(opts.caps_quick_actions.len(), 48);
        assert_eq!(opts.arrow_key_sets.len(), 2);
        assert_eq!(opts.home_row_schemes.len(), 2);
        assert_eq!(opts.bracket_pairs.len(), 4);
        assert_eq!(opts.paste_triggers.len(), 4);

        let nothing = opts
            .caps_remap_targets
            .iter()
            .find(|o| o.value == "Nothing")
            .unwrap();
        assert_eq!(nothing.label_key.as_deref(), Some("settings.presets.option.nothing"));

        let seek = opts.caps_quick_actions.iter().find(|o| o.value == "Seek").unwrap();
        assert_eq!(seek.label, "Seek");
        assert_eq!(seek.label_key, None, "Seek 는 고유명사라 labelKey 가 없다");

        let esc = opts.caps_remap_targets.iter().find(|o| o.value == "Esc").unwrap();
        assert_eq!(esc.label, "esc");
        assert_eq!(esc.label_key, None, "키캡 각인은 labelKey 가 없다");

        let symbol = opts
            .home_row_schemes
            .iter()
            .find(|o| o.value == "SymbolRow")
            .unwrap();
        assert_eq!(symbol.label_key.as_deref(), Some("settings.presets.option.home_row.symbol"));

        let hyper_trigger = opts.paste_triggers.iter().find(|o| o.value == "HyperKey").unwrap();
        assert_eq!(hyper_trigger.label_key.as_deref(), Some("settings.presets.option.paste.hyper"));
    }

    // presets_view() — variant 이름이 저장 형식(serde 기본값)과 일치한다.
    #[test]
    fn presets_view_uses_serialized_variant_names() {
        let presets = PresetSettings {
            caps_lock_remap: ultrakey_presets::settings::CapsLockRemapSettings {
                enabled: false,
                target: RemapCapsTarget::Esc,
            },
            ..PresetSettings::default()
        };
        let view = presets_view(&presets);
        assert_eq!(view.caps_lock_remap.target, "Esc");
        assert_eq!(view.caps_home_row.scheme, "SymbolRow");
        assert_eq!(view.quick_press_duration_ms, 1000);
    }

    // 충돌 응답 JSON 모양 — kind/disableLabelKeys.
    #[test]
    fn pending_conflict_view_shape() {
        let conflict = Conflict {
            kind: ConflictKind::CapsLockArrows,
            to_disable: vec![keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED],
        };
        let view = pending_conflict_view(conflict, keys::PRESETS_CAPS_WASD_ARROWS, &serde_json::json!(true));
        assert_eq!(view.kind, "capsLockArrows");
        assert_eq!(view.key, keys::PRESETS_CAPS_WASD_ARROWS);
        assert_eq!(view.value, serde_json::json!(true));
        assert_eq!(view.disable_label_keys, vec!["settings.presets.caps_hjkl.prefix".to_string()]);
    }

    // apply_preset_setting()/validate_and_apply_preset() — 알려진 키를 갱신하고,
    // 모르는 키는 거부한다.
    #[test]
    fn validate_and_apply_preset_updates_known_key() {
        let mut presets = PresetSettings::default();
        validate_and_apply_preset(
            &mut presets,
            keys::PRESETS_CAPS_SPACE_ENTER,
            &serde_json::json!(true),
        )
        .unwrap();
        assert!(presets.caps_space_enter);
    }

    #[test]
    fn validate_and_apply_preset_clamps_quick_press_duration() {
        let mut presets = PresetSettings::default();
        validate_and_apply_preset(
            &mut presets,
            keys::PRESETS_QUICK_PRESS_DURATION_MS,
            &serde_json::json!(50u64),
        )
        .unwrap();
        assert_eq!(presets.quick_press_duration_ms, 250);
    }

    #[test]
    fn validate_and_apply_preset_rejects_unknown_key() {
        let mut presets = PresetSettings::default();
        let err = validate_and_apply_preset(&mut presets, "presets.doesNotExist", &serde_json::json!(true))
            .unwrap_err();
        assert!(err.contains("알 수 없는"));
    }

    // ── F-16 korean.* — validate_and_apply_korean ───────────────────────────────

    #[test]
    fn validate_and_apply_korean_updates_known_key() {
        let mut korean = KoreanSettings::default();
        validate_and_apply_korean(
            &mut korean,
            keys::KOREAN_SHIFT_SPACE_SWITCHES_INPUT_SOURCE,
            &serde_json::json!(true),
        )
        .unwrap();
        assert!(korean.shift_space_switches_input_source);
    }

    /// ⚠️ 항목 5 는 기본값이 `true` 다 — 끄는 방향으로도 정확히 반영되는지 확인한다
    /// (명세 §4.2 각주, D-K9 — 구현·리뷰 양쪽에서 놓치기 쉬운 지점).
    #[test]
    fn validate_and_apply_korean_can_turn_off_remote_desktop_exclusion() {
        let mut korean = KoreanSettings::default();
        assert!(korean.disable_in_remote_desktop);
        validate_and_apply_korean(
            &mut korean,
            keys::KOREAN_DISABLE_IN_REMOTE_DESKTOP,
            &serde_json::json!(false),
        )
        .unwrap();
        assert!(!korean.disable_in_remote_desktop);
    }

    #[test]
    fn validate_and_apply_korean_rejects_unknown_key() {
        let mut korean = KoreanSettings::default();
        let err = validate_and_apply_korean(&mut korean, "korean.doesNotExist", &serde_json::json!(true))
            .unwrap_err();
        assert!(err.contains("알 수 없는"));
    }

    // SettingsState 직렬화가 camelCase 인지 — 프런트엔드가 기대하는 필드 이름 계약.
    #[test]
    fn settings_state_serializes_camel_case() {
        let hyperkey = HyperkeySettings::default();
        let presets = PresetSettings::default();
        let korean = KoreanSettings::default();
        let store = SettingsStore::in_memory();
        let state = build_settings_state(
            &hyperkey,
            &presets,
            &korean,
            &store,
            Some("디스크 가득 참".to_string()),
            None,
        );

        let json = serde_json::to_value(&state).unwrap();
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("hyperPreview"));
        assert!(obj.contains_key("sourceKeys"));
        assert!(obj.contains_key("trackpadAreas"));
        assert!(obj.contains_key("lastTab"));
        assert!(obj.contains_key("saveError"));
        assert!(obj.contains_key("presets"));
        assert!(obj.contains_key("presetOptions"));
        assert!(obj.contains_key("capsLockAliasActive"));
        assert!(obj.contains_key("general"));
        assert!(obj.contains_key("pendingConflict"));
        assert_eq!(obj["pendingConflict"], serde_json::Value::Null);

        let hyperkey_json = obj["hyperkey"].as_object().unwrap();
        assert!(hyperkey_json.contains_key("includeShiftInHyper"));
        assert!(hyperkey_json.contains_key("mouseApply"));
        let trackpad_json = hyperkey_json["trackpad"].as_object().unwrap();
        assert!(trackpad_json.contains_key("changeMenuBarIcon"));
        assert!(trackpad_json.contains_key("haptic"));
        assert!(!trackpad_json.contains_key("hapticFeedback"));

        let presets_json = obj["presets"].as_object().unwrap();
        assert!(presets_json.contains_key("capsLockRemap"));
        assert!(presets_json.contains_key("quickPressDurationMs"));
        assert!(!presets_json.contains_key("synthesizeCapsLockRemap"));

        let preset_options_json = obj["presetOptions"].as_object().unwrap();
        assert!(preset_options_json.contains_key("capsRemapTargets"));

        let general_json = obj["general"].as_object().unwrap();
        assert!(general_json.contains_key("launchOnLogin"));
        assert!(general_json.contains_key("hideMenuBarIcon"));

        assert_eq!(obj["lastTab"], "hyperkey"); // 빈 스토어의 기본값
        assert_eq!(obj["saveError"], "디스크 가득 참");
    }

    // source_key_view() — value 는 SourceKey 의 직렬화 값(variant 이름)과 일치한다.
    #[test]
    fn source_key_view_uses_serialized_variant_name() {
        let view = source_key_view(SourceKey::CapsLock);
        assert_eq!(view.value, "CapsLock");
        assert_eq!(view.label, "caps lock");
        assert!(view.has_keycode);

        let unknown = source_key_view(SourceKey::F21);
        assert!(!unknown.has_keycode);
    }

    // notice_from_outcome() — Fresh/Loaded 는 알림이 없고, Recovered/NewerSchema 는 있다.
    #[test]
    fn notice_from_outcome_only_for_recovered_and_newer_schema() {
        assert!(notice_from_outcome(&LoadOutcome::Fresh).is_none());
        assert!(notice_from_outcome(&LoadOutcome::Loaded { key_count: 3 }).is_none());

        let recovered = notice_from_outcome(&LoadOutcome::Recovered {
            backup: std::path::PathBuf::from("/tmp/settings.json.corrupt-1"),
            reason: "파싱 실패".to_string(),
        })
        .unwrap();
        assert_eq!(recovered.kind, "recovered");
        assert_eq!(recovered.message_key, "settings.load_recovered");

        let newer = notice_from_outcome(&LoadOutcome::NewerSchema { found: 99 }).unwrap();
        assert_eq!(newer.kind, "newerSchema");
        assert_eq!(newer.arg, "99");
    }

    // build_settings_state() — 빈 스토어에서도 hyperkey.validate() 의 경고가 그대로 실린다.
    #[test]
    fn build_settings_state_carries_validation_warnings() {
        let mut hyperkey = HyperkeySettings::default();
        hyperkey.hyper.enabled = true;
        hyperkey.meh.enabled = true; // 기본 source 가 둘 다 CapsLock → 중복.
        let store = SettingsStore::in_memory();

        let state = build_settings_state(
            &hyperkey,
            &PresetSettings::default(),
            &KoreanSettings::default(),
            &store,
            None,
            None,
        );
        assert_eq!(state.warnings.len(), 1);
        assert_eq!(state.warnings[0].kind, "duplicate");
        assert_eq!(state.warnings[0].key, "caps lock");
    }

    // build_preset_warnings() — F21~F24 를 Remap caps lock to: 대상으로 고르면
    // unknownKey 경고가 나온다(keycode 미확정).
    #[test]
    fn build_preset_warnings_flags_unknown_keycode_target() {
        let hyperkey = HyperkeySettings::default();
        let presets = PresetSettings {
            caps_lock_remap: ultrakey_presets::settings::CapsLockRemapSettings {
                enabled: true,
                target: RemapCapsTarget::F21,
            },
            ..PresetSettings::default()
        };
        let warnings = build_preset_warnings(&hyperkey, &presets, None);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, "unknownKey");
        assert_eq!(warnings[0].key, "F21");
    }

    // build_preset_warnings() — D-1 alias(F18)와 hyper 소스 F18 이 겹치면 duplicate
    // 경고(settings.hyperkey.warning.duplicate 재사용, §5).
    #[test]
    fn build_preset_warnings_flags_d1_alias_conflict_with_f18_source() {
        let mut hyperkey = HyperkeySettings::default();
        hyperkey.hyper.enabled = true;
        hyperkey.hyper.source = SourceKey::F18;

        let warnings = build_preset_warnings(&hyperkey, &PresetSettings::default(), Some(KeyCode::F18));
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].kind, "duplicate");
        assert_eq!(warnings[0].key, "F18");
    }

    #[test]
    fn build_preset_warnings_is_empty_when_nothing_conflicts() {
        let hyperkey = HyperkeySettings::default();
        assert!(build_preset_warnings(&hyperkey, &PresetSettings::default(), None).is_empty());
    }

    // ── F-10 메뉴바(M2 2차) — 순수 로직만 뽑아 단위 테스트한다 ──────────────

    // ignore_menu_text() — 최전면 앱을 알면 이름을 넣고, 모르면 안내 문구로 대체한다.
    #[test]
    fn ignore_menu_text_uses_front_app_name_when_known() {
        let catalog = Catalog::for_locale(ultrakey_i18n::Locale::En);
        let app = AppIdentity {
            bundle_id: "com.example.Ghostty".to_string(),
            name: "Ghostty".to_string(),
        };
        assert_eq!(ignore_menu_text(&catalog, Some(&app)), "Ignore Ghostty");
    }

    #[test]
    fn ignore_menu_text_falls_back_when_front_app_unknown() {
        let catalog = Catalog::for_locale(ultrakey_i18n::Locale::En);
        assert_eq!(ignore_menu_text(&catalog, None), catalog.get("menu.ignore_app.none"));
    }

    // synthesize_caps_lock_remap_enabled() — 부재 = 기본값(꺼짐), 저장된 값이 있으면 그대로.
    #[test]
    fn synthesize_caps_lock_remap_enabled_defaults_to_false() {
        let store = SettingsStore::in_memory();
        assert!(!synthesize_caps_lock_remap_enabled(&store));
    }

    #[test]
    fn synthesize_caps_lock_remap_enabled_reflects_stored_value() {
        let mut store = SettingsStore::in_memory();
        store
            .set(keys::PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP, &true)
            .unwrap();
        assert!(synthesize_caps_lock_remap_enabled(&store));
    }

    // menu_ids 상수들이 서로 다르다 — 오타로 두 메뉴 항목이 같은 id 를 갖는 회귀를 막는다.
    #[test]
    fn menu_ids_are_all_distinct() {
        let ids = [
            menu_ids::IGNORE_APP,
            menu_ids::SETTINGS,
            menu_ids::ABOUT,
            menu_ids::ADVANCED,
            menu_ids::SYNTHESIZE_CAPS_REMAP,
            menu_ids::RELAUNCH,
            menu_ids::QUIT,
            menu_ids::AUTHORIZE,
        ];
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "중복된 메뉴 항목 id 가 있다: {ids:?}");
    }
}
