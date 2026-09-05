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

use std::collections::{BTreeMap, VecDeque};
use std::io::Write as _;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use arc_swap::{ArcSwap, ArcSwapOption};
use objc2::MainThreadMarker;
use tauri::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIcon;
use tauri::{Emitter, LogicalSize, Manager, PhysicalSize, State, WebviewWindow, WindowEvent, Wry};

use ultrakey_core::flags::EventFlags;
use ultrakey_core::gate::{AppGate, AppGateController, AppIdentity, AtomicAppGate};
use ultrakey_core::keycode::{KeyCode, SourceKey};
use ultrakey_core::perdevice::destinations::{self, DestinationCategory};
use ultrakey_core::perdevice::{
    inherit, DeviceId, FKey, KeyRemapRow, ManagedLedger, PerDeviceSettings,
};
use ultrakey_core::settings::{keys, transfer, EngineConfig, LoadOutcome, MouseApply, SettingsStore};
use ultrakey_engine::path_b::LedgerStore;
use ultrakey_engine::{Engine, EngineEvent, SharedState, TapState};
use ultrakey_hyperkey::{HyperkeySettings, SettingsWarning, SlotSettings, TrackpadArea};
use ultrakey_i18n::{Catalog, Locale};
use ultrakey_korean::KoreanSettings;
use ultrakey_language_presets::{ChineseSettings, JapaneseSettings};
use ultrakey_permissions::{
    dev_build_warning, onboarding_copy, open_accessibility_settings, out_of_sync_copy,
    PermissionMonitor, PermissionState,
};
use ultrakey_platform::bundle;
use ultrakey_platform::fn_state;
use ultrakey_platform::hid_device;
use ultrakey_platform::login_item;
use ultrakey_platform::single_instance::{self, ShowSettingsObserver};
use ultrakey_platform::workspace::{observe_system_events, SystemEvent, SystemEventObserver};
use ultrakey_presets::{
    ArrowKeySet, BracketPair, Conflict, ConflictKind, HomeRowScheme, PasteTrigger, PresetSettings,
    QuickPressCapsAction, RemapCapsTarget,
};
use ultrakey_seek_session::{ClickSettings, SeekSettings, SeekShortcut};

/// ⭐ F-10 메뉴 항목 id — 그대로 i18n 카탈로그 키이기도 하다(고유하고, 라벨을
/// 조회할 때도 같은 문자열을 쓸 수 있어 별도 매핑표가 필요 없다).
/// ⭐ F-03 Seek 오버레이 (이슈 #34) — 창 생성·네이티브 설정·증분 수신.
mod click_executor;
/// ⭐ F-12 — 라이선스 배선(`LicenseController`) + General 탭 UI 커맨드.
mod license;
mod overlay;
/// ⭐ F-03 실기기 검증 하네스 (이슈 #34). `ULTRAKEY_OVERLAY_DEMO` 가 없으면
/// 아무것도 하지 않는다. F-01(세션 상태 머신)이 들어오면 지워도 된다.
mod overlay_demo;
/// ⭐ F-03 P3 실측 하네스 (이슈 #34). `ULTRAKEY_OVERLAY_SPIKE` 가 없으면
/// 아무것도 하지 않는다 — 제품 경로에 끼어들지 않는다.
mod overlay_spike;
/// ⭐ F-01 — Seek 활성화·세션 워커(이슈 #38). `docs/spec/
/// seek-activation-and-session.md` 를 코드로 옮긴다.
mod seek;
/// ⭐ F-06 — 트랙패드·Magic Mouse hyper 제스처 리스너(이슈 #63).
/// `docs/spec/trackpad-hyper-gesture.md` 를 코드로 옮긴다.
mod trackpad;

mod menu_ids {
    pub const IGNORE_APP: &str = "menu.ignore_app";
    pub const SETTINGS: &str = "menu.settings";
    pub const CHECK_FOR_UPDATES: &str = "menu.check_for_updates";
    pub const ABOUT: &str = "menu.about";
    pub const ADVANCED: &str = "menu.advanced";
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
    /// ⭐ `General` 탭 언어 선택 팝업(D6, 이슈 #39,
    /// `localization-and-input-sources.md` §3.1.2-a). **부재 = 시스템 언어를
    /// 따른다**(F-15 "부재 = 기본값") — `System` 을 고르면 이 키 자체를 지운다.
    pub const GENERAL_LANGUAGE: &str = "general.language";
    /// F-13 — General 탭 `Check for updates automatically` 체크박스. ⚠️ 이 키는
    /// `SettingsStore` 에 저장되지 않는다 — 정본은 Sparkle 의 UserDefaults
    /// (`SUEnableAutomaticChecks`)이고, 이 체크박스는 `settings_set` 라우팅에서
    /// 플러그인 API 로만 다룬다(F-15 저장 규약 밖 — launch-on-login 과 같은 이유로
    /// 거울 파일을 만들지 않는다).
    pub const GENERAL_AUTO_UPDATE: &str = "general.autoUpdate";

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
    /// ⭐ 이슈 #77 — General 탭 Advanced 섹션의 체크박스가 이 값을 렌더한다.
    synthesize_caps_lock_remap: bool,
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
        synthesize_caps_lock_remap: p.synthesize_caps_lock_remap,
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct KoreanView {
    shift_space_switches_input_source: bool,
    han_eng_switches_input_source: bool,
    hanja_key_converts_hanja: bool,
    won_key_types_backtick: bool,
    disable_in_remote_desktop: bool,
    /// ⭐ K9(이슈 #73, D-K18) — `modifier 키와 함께 누른 문자 키를 영어 소문자로 입력`.
    modifier_key_types_lowercase: bool,
    /// ⭐ K5(이슈 #73, D-K17) — 원격 데스크톱 제외 목록. `None` = 오버라이드 없음
    /// (기본 12종을 쓴다 — UI 는 기본 목록을 보여준다). `Some(list)` = 사용자가
    /// 편집한 목록(빈 배열 포함 — "아무 앱도 제외하지 않음"의 명시적 의도).
    excluded_bundle_ids: Option<Vec<String>>,
    /// ⭐ K5 — 앱에 내장된 기본 목록(`ultrakey-korean::apps::default_excluded_bundle_ids`).
    /// 오버라이드 부재 시 UI 가 이 값을 편집기에 채워 넣는다.
    default_excluded_bundle_ids: Vec<String>,
    /// ⭐ F-19.1 `캡스락 탭 = 한/영 전환` — ko 노드 소속(F-19 명세 §3.0). 저장 키 `korean.*`.
    caps_lock_switches_input_source: bool,
    /// ⭐ F-19.2 `오른쪽 command = 한/영 전환`.
    right_command_switches_input_source: bool,
}

/// ⭐ F-19 `日本語` 노드(F-19.3~F-19.6) — 프런트용 뷰.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct JapaneseView {
    caps_lock_toggles_eisu_kana: bool,
    command_toggles_eisu_kana: bool,
    swap_yen_backslash: bool,
    jis_as_us_symbols: bool,
    /// ⭐ D-7 — 앱 제외 목록 오버라이드(`japanese.excludedBundleIds`). `None` = 기본
    /// 목록(`ultrakey_korean::default_excluded_bundle_ids` — F-16 제외 정책 재사용,
    /// Q5). `Some(list)` = 사용자 편집분.
    excluded_bundle_ids: Option<Vec<String>>,
    /// 오버라이드 부재 시 UI 가 보여줄 기본 목록.
    default_excluded_bundle_ids: Vec<String>,
}

fn japanese_view(j: &JapaneseSettings, store: &SettingsStore) -> JapaneseView {
    JapaneseView {
        caps_lock_toggles_eisu_kana: j.caps_lock_toggles_eisu_kana,
        command_toggles_eisu_kana: j.command_toggles_eisu_kana,
        swap_yen_backslash: j.swap_yen_backslash,
        jis_as_us_symbols: j.jis_as_us_symbols,
        excluded_bundle_ids: store.get(keys::JAPANESE_EXCLUDED_BUNDLE_IDS),
        default_excluded_bundle_ids: ultrakey_korean::default_excluded_bundle_ids()
            .iter()
            .map(|s| s.to_string())
            .collect(),
    }
}

/// ⭐ F-19 `中文` 노드(F-19.7) — 프런트용 뷰.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct ChineseView {
    caps_lock_switches_input_source: bool,
    excluded_bundle_ids: Option<Vec<String>>,
    default_excluded_bundle_ids: Vec<String>,
}

fn chinese_view(c: &ChineseSettings, store: &SettingsStore) -> ChineseView {
    ChineseView {
        caps_lock_switches_input_source: c.caps_lock_switches_input_source,
        excluded_bundle_ids: store.get(keys::CHINESE_EXCLUDED_BUNDLE_IDS),
        default_excluded_bundle_ids: ultrakey_korean::default_excluded_bundle_ids()
            .iter()
            .map(|s| s.to_string())
            .collect(),
    }
}

/// ⭐ F-19 ko 노드 — F-19.1·F-19.2 도 `korean.*` 키라 `KoreanView` 에 함께 실린다.
fn korean_view(k: &KoreanSettings, store: &SettingsStore) -> KoreanView {
    // K5 — 저장된 오버라이드 목록(부재 = None). 정규화는 `resolve_excluded_bundle_ids`
    // 쪽에서 하므로 여기서는 원본을 그대로 실어 보낸다.
    let excluded: Option<Vec<String>> = store
        .get(keys::KOREAN_EXCLUDED_BUNDLE_IDS);
    KoreanView {
        shift_space_switches_input_source: k.shift_space_switches_input_source,
        han_eng_switches_input_source: k.han_eng_switches_input_source,
        hanja_key_converts_hanja: k.hanja_key_converts_hanja,
        won_key_types_backtick: k.won_key_types_backtick,
        disable_in_remote_desktop: k.disable_in_remote_desktop,
        modifier_key_types_lowercase: k.modifier_key_types_lowercase,
        excluded_bundle_ids: excluded,
        default_excluded_bundle_ids: ultrakey_korean::default_excluded_bundle_ids()
            .iter()
            .map(|s| s.to_string())
            .collect(),
        // ⭐ F-19.1·F-19.2 — ko 노드 신규(F-19 명세 §3.0, 저장 키 `korean.*`).
        caps_lock_switches_input_source: k.caps_lock_switches_input_source,
        right_command_switches_input_source: k.right_command_switches_input_source,
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct GeneralView {
    launch_on_login: bool,
    /// ⭐ B-1(이슈 #39, `menu-bar-and-lifecycle.md` §3.5-a) — OS 정본
    /// (`login_item::status()`)의 로그용 안정 식별자를 그대로 UI 에도 실어
    /// 보낸다. `requires-approval` 일 때만 프런트가 안내 행을 보인다.
    launch_on_login_status: String,
    hide_menu_bar_icon: bool,
    /// ⭐ 저장된 `general.language`(D6, 이슈 #39). `None` = "시스템 설정 따름"
    /// (키 부재 또는 알 수 없는 값 — §3.1.2-a "폴백"). 언어 선택 팝업의 초기값에
    /// 쓴다.
    language: Option<String>,
    /// F-13 — General 탭 `Check for updates automatically` 체크박스. 정본은
    /// Sparkle(UserDefaults)이고 `build_settings_state` 가 넘겨주는 미러다
    /// (`AppState::auto_update_checks_enabled`).
    auto_update: bool,
}

/// ⭐ A-2(이슈 #39, `localization-and-input-sources.md` §3.1.2-a) — 저장된
/// `general.language` 값을 로케일로 해석한다. 키가 없으면 `None`("시스템 설정
/// 따름", F-15 "부재 = 기본값"). 값이 있는데 알 수 없는 언어 코드면(손으로 고친
/// 설정 파일 등) 실패시키지 않고 `None` 으로 폴백하며 영어 로그를 남긴다.
fn resolve_stored_language(store: &SettingsStore) -> Option<Locale> {
    let code: String = store.get(settings_keys::GENERAL_LANGUAGE)?;
    let found = Locale::all().iter().copied().find(|l| l.code() == code);
    if found.is_none() {
        tracing::warn!(
            value = %code,
            "unknown general.language value in settings; falling back to the system language"
        );
    }
    found
}

fn general_view(store: &SettingsStore, auto_update: bool) -> GeneralView {
    // ⭐ B-1 — 정본은 언제나 OS 다. 저장된 값(거울)은 판정 자체가 불가능할 때
    // (`Unsupported`)만 폴백으로 쓴다.
    let status = login_item::status();
    let launch_on_login = if status == login_item::LoginItemStatus::Unsupported {
        store
            .get(settings_keys::GENERAL_LAUNCH_ON_LOGIN)
            .unwrap_or(false)
    } else {
        status.is_active()
    };

    GeneralView {
        launch_on_login,
        launch_on_login_status: status.as_log_str().to_string(),
        hide_menu_bar_icon: store
            .get(settings_keys::GENERAL_HIDE_MENU_BAR_ICON)
            .unwrap_or(false),
        language: resolve_stored_language(store).map(|locale| locale.code().to_string()),
        auto_update,
    }
}

// ============================================================================
// F-17 키보드별 설정(`per-device-settings.md`) — `Keyboards` 탭 백엔드 계약
// (`settings.html` 851~866행 주석이 정본으로 삼는 모양). 정본은 이 브랜치의
// `CONTRACT.md` §B.6 이다.
// ============================================================================

/// `state.perDevice.devices` 항목 하나 — 좌측 패인이 그대로 쓴다(§3.1.2,
/// 이슈 #31 ④ — Karabiner 식 좌측 세로 목록으로 재배치).
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PerDeviceDeviceView {
    id: String,
    name: String,
    connected: bool,
    /// `[VID: …, PID: …]` 서브라벨용 — ⚠️ **십진수**다(`DeviceId`/`id` 는 저장 키
    /// 조립에 쓰는 16진 문자열이라 그대로 못 쓴다). 스크린샷
    /// (`docs/research/screenshots/issue-31-karabiner-function-keys-menu.png`)이
    /// 십진 표기를 쓴다 — 그 배치를 그대로 옮긴다.
    vendor_id: u32,
    product_id: u32,
}

/// `state.perDevice.destinationCategories` 항목 하나 — 기능 2 팝업의
/// `<optgroup>` 하나(§3.5, 이슈 #31 ②). 순서는
/// [`destinations::DestinationCategory::all`] 그대로다(15종).
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PerDeviceDestinationCategoryView {
    key: String,
    label_key: String,
}

/// `state.perDevice.destinations` 항목 하나 — 기능 2 팝업의 `<option>`
/// (§3.5, 이슈 #31 ②). 카탈로그 313종 전량, 카테고리 순서 그대로.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PerDeviceDestinationView {
    value: String,
    /// ⛔ 번역하지 않는다 — HID usage 이름 그대로다(키캡 각인 규약,
    /// `destinations::FunctionDestination::label` 문서 참고). 예외는 `"disable"`
    /// 하나뿐이고, 그 경우도 이 필드가 아니라 프런트가 `value === "disable"` 을
    /// 보고 i18n 카탈로그에서 문구를 가져온다.
    label: String,
    category: String,
    /// [`destinations::PathBSupport::is_verified`] — `false` 면 이 목적지는
    /// usage page 가 실제로 동작하는지 확인되지 않았다. 프런트는 이 값이 `false`
    /// 인 채로 선택돼 있을 때만 "동작 미확인" 힌트를 보인다(카탈로그에서 숨기지
    /// 않는다 — 이슈 #31 ② "고를 수는 있는데 아무 일도 안 일어나는 UX 를 만들지
    /// 마라"에 대한 이 세션의 결정: 뺄지 말지 대신 사실을 알린다).
    verified: bool,
}

/// `Keyboards` 탭 전체를 그리는 데 필요한 필드(계약 §B.6, 이슈 #31 ②④ 반영).
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct PerDeviceView {
    devices: Vec<PerDeviceDeviceView>,
    /// `state.sourceKeys` 와 같은 형식(`{value,label}` — 여기서는 `hasKeycode` 도
    /// 함께 실리지만 프런트는 그 필드를 쓰지 않는다) — `SourceKey::all()` 35종.
    source_keys: Vec<SourceKeyView>,
    destination_categories: Vec<PerDeviceDestinationCategoryView>,
    destinations: Vec<PerDeviceDestinationView>,
    fn_state_is_standard: Option<bool>,
    values: serde_json::Map<String, serde_json::Value>,
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

/// `state.perDevice` 조립 — 계약 §B.6, 이슈 #31 ②④ 로 확장된 필드 전부.
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
            PerDeviceDeviceView {
                id: id.as_str().to_string(),
                name,
                connected: true,
                vendor_id: info.vendor_id,
                product_id: info.product_id,
            },
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
            // (⛔ 개인 디바이스 이름 하드코딩 금지, 계약 §B.6). VID/PID 는
            // `DeviceId::to_match()` 로 되돌린다 — 저장 키(16진 문자열)와 달리
            // 좌측 패인의 `[VID: …, PID: …]` 서브라벨은 십진수다(이슈 #31 ④).
            let m = id.to_match();
            PerDeviceDeviceView {
                id: id.as_str().to_string(),
                name: id.as_str().to_string(),
                connected: false,
                vendor_id: m.vendor_id,
                product_id: m.product_id,
            }
        });
    }

    // sourceKeys — `state.sourceKeys` 와 같은 조립 함수를 재사용한다(35종, 키캡 각인).
    let source_keys = SourceKey::all()
        .iter()
        .copied()
        .map(source_key_view)
        .collect();

    // destinationCategories/destinations — 기능 2 팝업의 목적지 카탈로그
    // 313종/15카테고리(이슈 #31 ②) 그대로 실어 보낸다. 카탈로그 자체가 이미
    // 팝업 표시 순서(카테고리별로 묶여)라 여기서 재정렬하지 않는다.
    let destination_categories = DestinationCategory::all()
        .iter()
        .map(|c| PerDeviceDestinationCategoryView {
            key: c.label_segment().to_string(),
            label_key: format!(
                "preferences.keyboards.functionKeys.category.{}",
                c.label_segment()
            ),
        })
        .collect();
    let destinations = destinations::all()
        .iter()
        .map(|d| PerDeviceDestinationView {
            value: d.id.to_string(),
            label: d.label.to_string(),
            category: d.category.label_segment().to_string(),
            verified: d.support.is_verified(),
        })
        .collect();

    PerDeviceView {
        devices: devices.into_values().collect(),
        source_keys,
        destination_categories,
        destinations,
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
        return Err(format!("{key} is a ledger key; it cannot be changed through this command"));
    }
    let rest = key
        .strip_prefix("perDevice.")
        .ok_or_else(|| format!("unknown settings key: {key}"))?;
    let (scope, tail) = rest
        .split_once('.')
        .ok_or_else(|| format!("unknown settings key: {key}"))?;
    if scope != keys::PER_DEVICE_COMMON_SCOPE && DeviceId::parse(scope).is_none() {
        return Err(format!("unknown device identifier: {scope}"));
    }
    let is_key_remap = tail == "keyRemap.rows";
    let is_function_key = FKey::all()
        .iter()
        .any(|f| tail == format!("functionKeys.{}", f.key_segment()));
    if !is_key_remap && !is_function_key {
        return Err(format!("unknown settings key: {key}"));
    }
    Ok(())
}

/// ⭐ 이슈 #31 ② — `validate_per_device_key` 가 **키 모양**만 본 뒤, 기능 2 슬롯에
/// 한해 **값의 내용**도 검증한다. `key` 가 `functionKeys.f1`~`f12` 이고 `value` 가
/// 문자열이면 그 문자열이 [`destinations::resolve_stored`] 로 풀려야 한다 — 그
/// 외(기능 1 의 배열/`null`, 기능 2 의 `null`=명시적 끔)는 이 함수의 관심사가
/// 아니므로 그냥 통과시킨다(§3.3 의 끔/부재 구분을 이 검증이 막으면 안 된다).
///
/// **근거**: 프런트가 보내는 값을 검증 없이 그대로 저장하면, 카탈로그에 없는
/// 문자열(오타·구버전 프런트·수동 조작으로 만든 값)이 설정 파일에 조용히 남는다.
/// `PerDeviceSettings::resolved_function_key`(perdevice/mod.rs)는 그런 값을 경고
/// 로그만 남기고 `None`으로 처리해 앱을 죽이지는 않지만, 그 결과는 "골랐는데
/// 아무 일도 일어나지 않는다"다 — 이슈 #31 이 명시적으로 경계한 UX 다. 저장 전에
/// 거부하면 그 상태 자체가 설정 파일에 생기지 않는다.
fn validate_per_device_function_key_value(
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    let Some(rest) = key.strip_prefix("perDevice.") else {
        return Ok(());
    };
    let Some((_scope, tail)) = rest.split_once('.') else {
        return Ok(());
    };
    let is_function_key = FKey::all()
        .iter()
        .any(|f| tail == format!("functionKeys.{}", f.key_segment()));
    if !is_function_key {
        return Ok(());
    }
    // `null`(명시적 끔) 등 문자열이 아닌 값은 이 검증의 대상이 아니다.
    let Some(s) = value.as_str() else {
        return Ok(());
    };
    if destinations::resolve_stored(s).is_none() {
        return Err(format!("unknown destination id: {s}"));
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
    validate_per_device_function_key_value(key, value)?;

    let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let presets_snapshot = *state.presets.lock().map_err(|e| e.to_string())?;
    let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;
    let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
    let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;
    let seek_snapshot = state.seek.lock().map_err(|e| e.to_string())?.clone();

    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "failed to save setting");
                Some(e.to_string())
            }
        }
    };

    // perDevice.* 는 hyperkey/meh/bleh 소스 키 자체를 바꾸지 않으므로 force_reset
    // (stuck modifier 방지) 은 필요 없다 — D-D 의 대상 밖이다.
    reconfigure_engine(
        state,
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        false,
    )?;

    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        &store,
        state.auto_update_checks_enabled(),
        save_error,
        None,
        caps_lock_kernel_map_missing(state),
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
        Err(e) => return Err(format!("could not read settings file: {e}")),
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
            tracing::error!(error = %e, "failed to save perDevice._managed ledger");
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
        // ⭐ F-19 — 언어 규칙 충돌(`languageKeycodeShared`)의 배타/해소 대상.
        // F-19.1~F-19.7 저장 키 → 탭 전용 카탈로그 라벨.
        k if k == keys::KOREAN_CAPS_LOCK_SWITCHES_INPUT_SOURCE => {
            "settings.korean.caps_switch"
        }
        k if k == keys::KOREAN_RIGHT_COMMAND_SWITCHES_INPUT_SOURCE => {
            "settings.korean.right_cmd"
        }
        k if k == keys::JAPANESE_CAPS_LOCK_TOGGLES_EISU_KANA => {
            "settings.japanese.caps_eisu_kana"
        }
        k if k == keys::JAPANESE_COMMAND_TOGGLES_EISU_KANA => {
            "settings.japanese.cmd_eisu_kana"
        }
        k if k == keys::JAPANESE_SWAP_YEN_BACKSLASH => "settings.japanese.yen_backslash",
        k if k == keys::JAPANESE_JIS_AS_US_SYMBOLS => "settings.japanese.jis_us",
        k if k == keys::CHINESE_CAPS_LOCK_SWITCHES_INPUT_SOURCE => {
            "settings.chinese.caps_switch"
        }
        other => {
            // 방어적 — conflicts.rs 가 이 네 개 밖의 키를 내놓는 일은 없어야 한다.
            tracing::error!(key = other, "unknown settings key in conflict resolution list");
            "settings.presets.heading"
        }
    }
}

fn pending_conflict_view(
    conflict: Conflict,
    key: &str,
    value: &serde_json::Value,
) -> PendingConflictView {
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

/// ⭐ F-19 언어 규칙 충돌(`LanguageConflict`)을 프런트 페이로드로 옮긴다 —
/// `kind` 는 카탈로그 `settings.presets.conflict.title.<kind>` 키와 맞물린다
/// (`languageKeycodeShared`). `to_disable`(끌 상대) 저장 키는
/// [`disable_label_key_for_setting`] 으로 라벨 키를 만든다.
fn language_shared_conflict_view(
    conflict: &ultrakey_language_presets::LanguageConflict,
    key: &str,
    value: &serde_json::Value,
) -> PendingConflictView {
    PendingConflictView {
        kind: conflict.kind.as_str(),
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

/// ⭐ F-19(H3) — 캡스락을 **단독 탭 트리거**로 주장하는 언어 규칙이 하나라도
/// 켜져 있는가(F-19.1 캡스락=한/영 · F-19.3 캡스락=英数/かな · F-19.7 캡스락=중/영).
/// 그러면 캡스락의 릴리즈를 FSM 이 봐야 하므로 D-1 alias(F18)가 필요하다(§5 #20).
fn language_caps_lock_tap(
    korean: &KoreanSettings,
    japanese: &JapaneseSettings,
    chinese: &ChineseSettings,
) -> bool {
    korean.caps_lock_switches_input_source
        || japanese.caps_lock_toggles_eisu_kana
        || chinese.caps_lock_switches_input_source
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
///
/// ⭐ F-01 배선 — `seek_remap_is_caps_lock`(`Remap key to Seek: caps lock`)도 이
/// alias 를 반드시 요구하는 조건에 더했다. 근거: caps lock 은 **누를 때만**
/// `flagsChanged` 를 보내고 **뗄 때는 보내지 않는다**(`docs/dev/
/// manual-verification.md` "caps lock 을 소스로 쓸 때의 알려진 한계",
/// `key-remapping-engine.md` §5 #20). `Remap key to Seek:` 가 caps lock 을 직접
/// 감시하면(경로 B 를 거치지 않으면) hold 모드(`Only show while the remapped key
/// is held`)의 릴리즈가 영영 오지 않아 — 세션이 열린 채 닫힐 방법이 없어진다.
/// 경로 B 로 F18 에 별칭을 걸면 F18 은 정상적인 `KeyDown`/`KeyUp` 을 내므로 이
/// 문제가 사라진다. `synthesize_caps_lock_remap`(Advanced) 이 켜져 있으면 다른
/// 이유와 마찬가지로 이 요구도 무시된다 — 기존 alias 의미(경로 A 강제)를
/// 그대로 지킨다.
fn compute_caps_lock_alias(
    presets: &PresetSettings,
    caps_is_source: bool,
    seek_remap_is_caps_lock: bool,
    language_caps_lock_tap: bool,
) -> Option<KeyCode> {
    // ⭐ F-19(H3) — 캡스락 단독 탭 규칙(F-19.1·F-19.3·F-19.7)도 캡스락의
    // 릴리즈를 FSM 이 봐야 하므로 D-1 alias 가 필요하다(캡스락은 래칭 키 —
    // `key-remapping-engine.md` §5 #20). `synthesize_caps_lock_remap` 이 켜져
    // 있으면(경로 A 강제) 기존 alias 의미론과 같게 무시한다.
    let needs_alias = presets.needs_caps_lock_alias(caps_is_source)
        || seek_remap_is_caps_lock
        || language_caps_lock_tap;
    if needs_alias && !presets.synthesize_caps_lock_remap {
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
            RemapCapsTarget::F21
                | RemapCapsTarget::F22
                | RemapCapsTarget::F23
                | RemapCapsTarget::F24
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
        if slots
            .into_iter()
            .any(|(enabled, source)| enabled && source == SourceKey::F18)
        {
            tracing::warn!("D-1 caps lock alias (F18) conflicts with the F18 chosen as the hyper/meh/bleh source");
            warnings.push(WarningView {
                kind: "duplicate",
                key: SourceKey::F18.label().to_string(),
            });
        }
    }

    warnings
}

// ============================================================================
// F-01 Seek 탭(`seek-activation-and-session.md` §4) — `settings.html` 이 기대하는
// JSON 계약. ⭐ 이 모양은 다른 위임이 이미 그것을 전제로 `settings.html` 을 쓰고
// 있으므로 **글자 그대로** 지킨다.
// ============================================================================

/// `Toggle Seek with shortcut:` 하나 — 미설정이면 `SeekView::toggle_shortcut` 가
/// `None` 이다.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SeekShortcutView {
    code: String,
    modifiers: u32,
    display: String,
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SeekView {
    /// 미설정이면 null.
    toggle_shortcut: Option<SeekShortcutView>,
    /// `SourceKey` variant 이름, 미설정이면 "-".
    remap_key: String,
    execute_on_close: bool,
    semicolon_cycles: bool,
    /// ⭐ F-04(이슈 #44) — 체크박스 2종. 저장 키 `seek.focusWindowBeforeClicking`.
    focus_window_before_clicking: bool,
    /// 저장 키 `seek.changeClickModesWithModifiers`. 출고 기본값 ☑.
    change_click_modes_with_modifiers: bool,
    /// ⭐(이슈 #93) `검색 언어` — 명시 값(`"ko"`·`"zh"`·`"ja"`·`"es"`·`"en"`).
    /// **부재(`None`) = 영어 고정**(이슈 #131 — 이슈 #48 의 로케일 폴백은 폐기):
    /// OCR 은 영어 단일이고 인풋 박스 모드는 꺼진다. UI 는 부재 시 영어
    /// 기본(`"en"`)이 선택된 것으로 보여준다(Plan §9 #2).
    search_language: Option<String>,
    /// ⭐(이슈 #133, D12) `창 제목 검색` 체크박스 — 저장 키 `seek.includeWindowTitles`.
    /// 출고 기본값 ☑(부재 = true — `SeekSettings` 가 이미 반영).
    include_window_titles: bool,
    /// `Presets` 탭 `Quick press caps lock to execute:` 가 `Seek` 인가(읽기 전용
    /// 표시용).
    quick_press_opens: bool,
    /// 세 경로 중 하나라도 설정됐는가 — false 면 UI 가 안내 문구를 띄운다(§1).
    any_activation_configured: bool,
    /// 팝업 선택지 35종.
    remap_key_options: Vec<PresetOptionView>,
}

/// `Remap key to Seek:` 팝업 항목 하나 — `-`(`None`)은 값·라벨 모두 `"-"`.
fn seek_remap_key_option_view(k: Option<SourceKey>) -> PresetOptionView {
    match k {
        None => PresetOptionView {
            value: "-".to_string(),
            label: "-".to_string(),
            label_key: None,
        },
        Some(k) => PresetOptionView {
            value: serde_variant_name(&k),
            label: k.label().to_string(),
            label_key: None,
        },
    }
}

fn seek_view(seek: &SeekSettings, quick_press_opens: bool) -> SeekView {
    SeekView {
        toggle_shortcut: seek.toggle_shortcut.as_ref().map(|s| SeekShortcutView {
            code: s.code.clone(),
            modifiers: u32::try_from(s.modifiers.0).unwrap_or(u32::MAX),
            display: s.display(),
        }),
        remap_key: seek
            .remap_key
            .map(|k| serde_variant_name(&k))
            .unwrap_or_else(|| "-".to_string()),
        execute_on_close: seek.execute_on_close,
        semicolon_cycles: seek.semicolon_cycles,
        focus_window_before_clicking: seek.focus_window_before_clicking,
        change_click_modes_with_modifiers: seek.change_click_modes_with_modifiers,
        search_language: seek.search_language.clone(),
        include_window_titles: seek.include_window_titles,
        quick_press_opens,
        any_activation_configured: seek
            .to_config(quick_press_opens)
            .any_activation_configured(),
        remap_key_options: SeekSettings::remap_key_options()
            .iter()
            .copied()
            .map(seek_remap_key_option_view)
            .collect(),
    }
}

/// `Presets` 탭 `Quick press caps lock to execute:` 가 `Seek` 를 가리키는가
/// (☑ + 팝업 = `Seek`). `SeekView::quick_press_opens`·`SeekConfig::quick_press_opens`
/// 둘 다 이 판정을 쓴다.
fn quick_press_opens_seek(presets: &PresetSettings) -> bool {
    presets.caps_quick_press.enabled
        && presets.caps_quick_press.action == QuickPressCapsAction::Seek
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
    /// ⭐ 이슈 #110 — D-1 이 필요한데 마지막 경로 B 재조정에서 **되읽기 확인이 안
    /// 됐다**(`caps_lock_kernel_map_missing`). UI 는 `caps_alias.note` 힌트 대신
    /// `settings.presets.caps_alias.missing` 경고를 보인다.
    caps_lock_kernel_map_missing: bool,
    general: GeneralView,
    /// F-16 `Korean` 탭(ko 노드 — F-19.1·F-19.2 포함).
    korean: KoreanView,
    /// ⭐ F-19 `日本語` 노드.
    japanese: JapaneseView,
    /// ⭐ F-19 `中文` 노드.
    chinese: ChineseView,
    /// F-01 `Seek` 탭.
    seek: SeekView,
    /// 값을 아직 적용하지 않은 충돌(architecture.md §6.5) — `Some` 이면 그 앞의
    /// `settings_set` 호출은 아무것도 저장·반영하지 않았다.
    pending_conflict: Option<PendingConflictView>,
    /// F-17 `Keyboards` 탭(`per-device-settings.md`, `settings.html` 851~866행 계약).
    per_device: PerDeviceView,
}

/// 설정 창이 화면을 다시 그리는 데 필요한 전부를 한 번에 조립한다(각 인자는
/// 독립적으로 의미 있는 view 입력이라 `seek.rs` 의 관례와 같이 허용한다 —
/// 구조체로 묶으면 호출부마다 필드 채우기를 흩뜨린다).
#[allow(clippy::too_many_arguments)]
fn build_settings_state(
    hyperkey: &HyperkeySettings,
    presets: &PresetSettings,
    korean: &KoreanSettings,
    japanese: &JapaneseSettings,
    chinese: &ChineseSettings,
    seek: &SeekSettings,
    store: &SettingsStore,
    auto_update: bool,
    save_error: Option<String>,
    pending_conflict: Option<PendingConflictView>,
    caps_lock_kernel_map_missing: bool,
) -> SettingsState {
    let last_tab = store
        .get::<String>(keys::UI_LAST_TAB)
        .unwrap_or_else(|| "hyperkey".to_string());
    let caps_is_source = caps_is_modifier_source(hyperkey);
    let seek_remap_is_caps_lock = seek.remap_key == Some(SourceKey::CapsLock);
    let caps_lock_alias = compute_caps_lock_alias(
        presets,
        caps_is_source,
        seek_remap_is_caps_lock,
        language_caps_lock_tap(korean, japanese, chinese),
    );

    let mut warnings: Vec<WarningView> =
        hyperkey.validate().into_iter().map(warning_view).collect();
    warnings.extend(build_preset_warnings(hyperkey, presets, caps_lock_alias));

    SettingsState {
        hyperkey: hyperkey_view(hyperkey),
        hyper_preview: hyper_preview(hyperkey),
        source_keys: SourceKey::all()
            .iter()
            .copied()
            .map(source_key_view)
            .collect(),
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
        caps_lock_kernel_map_missing: caps_lock_alias.is_some() && caps_lock_kernel_map_missing,
        general: general_view(store, auto_update),
        korean: korean_view(korean, store),
        japanese: japanese_view(japanese, store),
        chinese: chinese_view(chinese, store),
        seek: seek_view(seek, quick_press_opens_seek(presets)),
        pending_conflict,
        per_device: build_per_device_view(store),
    }
}

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct AppMeta {
    version: String,
    bundle_id: String,
    /// ⭐ 이슈 #87 — About 창의 "앱 설치 위치" 행. `.app` 번들 경로를 유도한 값이며,
    /// 번들 밖(`tauri dev`)에서는 `None` — 프런트가 `about.outside_bundle` 로 대체한다.
    app_path: Option<String>,
    /// 제작자 이름 — `package_info().authors` 가 비어 있으면(워크스페이스에
    /// `authors` 필드 없음) 하드코딩 폴백 "John Park" 을 쓴다(`plan/issue-87` §9 #4).
    author: String,
    log_path: String,
    settings_path: Option<String>,
    settings_file_exists: bool,
}

/// `~/Library/Logs/Ultrakey` 디렉터리 경로 — 로그를 다루는 세 함수
/// (`log_file_path_display`·`open_log_file`·`open_log_folder`)가 공유하는 단일
/// 출처다(이슈 #47). 경로 조각을 두 곳에 하드코딩하던 것을 한 곳으로 모은다.
/// HOME 환경변수가 없으면 `None` — 호출자가 각자의 방식으로 반응한다(빈 문자열
/// 표시 · 로그 개방 포기 · 조용히 무시).
fn log_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME").map(|home| std::path::PathBuf::from(home).join("Library/Logs/Ultrakey"))
}

/// `log_dir()` 의 경로에 `ultrakey.log` 를 이어 붙인 표시 문자열 — About 패널에
/// "어디에 로그를 쓰는가"만 보여주면 된다(파일 자체는 열지 않는다). HOME 없음 시
/// 빈 문자열이 되는 것은 기존 동작 그대로다.
fn log_file_path_display() -> String {
    log_dir()
        .map(|dir| dir.join("ultrakey.log").display().to_string())
        .unwrap_or_default()
}

/// `PackageInfo` 의 제작자 필드 — Cargo.toml 의 `[package] authors` 다. 워크스페이스에
/// 그 필드가 없어 비어 있는 문자열이 오는 것이 기본이라, 비면 하드코딩 "John Park"
/// 을 폴백으로 쓴다(`plan/issue-87-about-window.md` §9 #4 — `(추정)` 이었다가 구현
/// 시 폴백 확정).
fn app_author(app: &tauri::AppHandle) -> String {
    let authors = app.package_info().authors.trim();
    if authors.is_empty() {
        "John Park".to_string()
    } else {
        authors.to_string()
    }
}

/// `.app` 번들 경로 — `current_exe()`(`<App>.app/Contents/MacOS/<binary>`)의 상위
/// 3 디렉터리를 걸어 올라가 유도한다. 번들 밖(`tauri dev` → `target/debug/<binary>`)
/// 은 의미 있는 경로가 아니므로 `None` 이 되고, About 창 프런트가 `about.outside_
/// bundle` 안내로 대체한다(`plan/issue-87-about-window.md` §5 항목 2).
fn app_bundle_path() -> Option<String> {
    if !bundle::is_running_from_app_bundle() {
        return None;
    }
    let exe = std::env::current_exe().ok()?;
    let dir = exe.as_path();
    // Contents/MacOS/<binary> → Contents → <App>.app
    let app_dir = dir.parent()?.parent()?.parent()?;
    Some(app_dir.display().to_string())
}

fn build_app_meta(app: &tauri::AppHandle, store: &SettingsStore) -> AppMeta {
    AppMeta {
        version: app.package_info().version.to_string(),
        bundle_id: app.config().identifier.clone(),
        app_path: app_bundle_path(),
        author: app_author(app),
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
    /// ⭐ A-4(이슈 #39, §3.1.2-a) — 언어 선택 팝업이 그릴 선택지. ⚠️ `endonym`
    /// 은 **번역하지 않는다** — 지금 UI 를 못 읽는 사람도 자기 언어를 찾을 수
    /// 있어야 하는 유일한 컨트롤이라, 목록 자체가 현재 로케일과 무관해야 한다.
    languages: Vec<LanguageOption>,
}

/// `bootstrap.languages` 항목 하나. 프런트가 하드코딩하지 않도록(로케일이 늘 때
/// 두 곳을 고쳐야 하는 drift 를 막는다) 백엔드가 `Locale::endonym()` 을 그대로
/// 실어 보낸다.
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LanguageOption {
    code: &'static str,
    endonym: &'static str,
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

/// 설정 창에 실재하는 탭 이름 화이트리스트 — `settings.html` 의 `const TABS` 와
/// 항상 양방향으로 일치해야 한다(`tests::every_tab_in_settings_html_is_a_known_tab`
/// 가 검증한다).
///
/// ⭐ 이 화이트리스트 자체는 이슈 #28 에서 실기기로 터진 결함 때문에 반드시
/// 있어야 한다: `settings.html` 이 `Keyboards` 탭을 `TABS` 에 넣었는데 이쪽(당시
/// `tab_window_size`)에 대응 항목을 넣지 않아 `settings_set_tab` 이 `알 수 없는
/// 탭: keyboards` 로 거부하고 **설정 창 전체가 오류 화면으로 죽었다.** 창 크기
/// 테이블(예전 `tab_window_size`)은 이슈 #32 Phase 1 에서 걷어냈다 — 탭마다
/// 창을 리사이즈하는 원본 SuperKey 동작을 클론이 따르지 않기로 결정했기
/// 때문이다(단일 크기 고정 + 사용자 조절 크기 영속, 아래 `SETTINGS_WINDOW_DEFAULT`
/// 참고). 그래도 "프런트가 아는 탭을 Rust 도 아는가"라는 검증 자체는 여전히
/// 필요해 이 화이트리스트로 남긴다.
const KNOWN_TABS: &[&str] = &["seek", "hyperkey", "presets", "korean", "japanese", "chinese", "keyboards", "general"];

fn is_known_tab(tab: &str) -> bool {
    KNOWN_TABS.contains(&tab)
}

/// 설정 창 기본 크기 — 가장 큰 탭(`Presets`)이 잘리지 않는 크기(이슈 #32 Phase 1).
///
/// ⭐ **실측값이다**(2026-08-30). 창 너비 825pt 에서 각 탭 `#panel-<탭>` 을 실제로
/// 렌더해 `scrollHeight` 를 쟀다:
///   seek 55 · hyperkey 402 · presets **749** · korean 539 · keyboards 665 ·
///   general 426
/// 가장 큰 것은 `presets` 749. 여기에 `.panel` 상하 패딩 40 + 창 크롬(타이틀바)
/// 32 를 더해 높이 **821**. (크롬 32 는 같은 실측에서 확인했다 — 창 높이 527 일
/// 때 `window.innerHeight` 가 495 였다.)
/// ⚠️ 예전 `tab_window_size` 의 `korean` 항목이 쓰던 "타이틀바 28" 은 이 실측과
/// 어긋난다 — 이 값을 정할 때는 32 를 쓴다.
/// 너비 825 는 원본 `Presets` 탭 실측값이자 6개 탭 중 최대값이라 그대로 쓴다.
///
/// ⚠️ 남은 불확실성: `seek` 은 M3 미구현이라 패널이 사실상 비어 있다(55px) — 이
/// 실측이 최종값이 아니다. M3 가 들어오면 다시 재야 한다.
///
/// ⭐ 실제 기본 크기는 `tauri.conf.json` 의 `settings` 창 선언이 정한다(Tauri 가
/// 창을 만들 때 그 값을 쓴다) — 이 상수는 그 값의 근거를 문서화하고
/// `tests::settings_window_default_matches_tauri_conf` 로 드리프트를 잡기 위한
/// 것이라 런타임 경로에서는 참조하지 않는다.
#[allow(dead_code)]
const SETTINGS_WINDOW_DEFAULT: (u32, u32) = (825, 821);

/// hyperkey + presets + korean + seek 네 설정 묶음을 합쳐 `EngineConfig` 하나로
/// 조립하는 단일 지점(위임 지시서 §4) — `Engine::reconfigure` 로 넘길 값은 항상 이
/// 함수를 거친다.
fn build_engine_config(
    hyperkey: &HyperkeySettings,
    presets: &PresetSettings,
    korean: &KoreanSettings,
    seek: &SeekSettings,
    japanese: &JapaneseSettings,
    chinese: &ChineseSettings,
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
    let seek_remap_is_caps_lock = seek.remap_key == Some(SourceKey::CapsLock);
    config.caps_lock_alias = compute_caps_lock_alias(
        presets,
        caps_is_source,
        seek_remap_is_caps_lock,
        language_caps_lock_tap(korean, japanese, chinese),
    );
    // F-16 — `korean.disableInRemoteDesktop` 은 여기 들어오지 않는다(엔진 설정이
    // 아니라 게이트다, D-K3) — `gate_controller.set_korean_exclusion_enabled` 이 따로 처리한다.
    config.rules.korean_rules = korean.to_rules();
    // ⭐ F-19 — 언어 규칙을 ko(ko 노드 `korean.*` 키) + ja + zh 세 소스에서 합쳐
    // 넣는다. 각자 `RuleId` 오름차순으로 반환하므로 순서 불변식은 병합 후에도 유지된다
    // (`RuleId::Language(_)` 끼리의 payload 비교로 정렬된다).
    let mut language_rules = korean.to_language_rules();
    language_rules.extend(japanese.to_language_rules());
    language_rules.extend(chinese.to_language_rules());
    language_rules.sort_by_key(|r| r.id);
    config.rules.language_rules = language_rules;
    // ⭐ K9(이슈 #73, D-K18) — modifier+문자키 → 영어 소문자 옵션은 규칙이 아니라
    // 중재기의 행동 플래그다(`EngineConfig::korean_modifier_lowercase`).
    config.korean_modifier_lowercase = korean.modifier_key_types_lowercase;
    // ⭐ F-01 — `Remap key to Seek:` 트리거 키. `quick_press_opens`/`toggle_shortcut`
    // 는 `EngineConfig`(F-07 규칙 테이블)의 관심사가 아니다 — F-07 은 리매핑 키
    // 자체의 감시만 하고, 어느 모드로 세션을 열지는 F-01(Seek 워커)이 판정한다.
    config.rules.seek_trigger = seek.to_config(quick_press_opens_seek(presets)).remap_key;
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
        _ => return Err(format!("{key} cannot be changed through this command")),
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
        return Err(format!("unknown settings key: {key}"));
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
        k if k == keys::PRESETS_SHIFT_CAPS_TO_CAPS => {
            presets.shift_caps_to_caps = parse(value, key)?
        }
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
        _ => return Err(format!("{key} cannot be changed through this command")),
    }
    Ok(())
}

fn validate_and_apply_preset(
    presets: &mut PresetSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    if !keys::all().contains(&key) {
        return Err(format!("unknown settings key: {key}"));
    }
    apply_preset_setting(presets, key, value)
}

struct AppState {
    /// ⭐ A-1(D6, 이슈 #39, `localization-and-input-sources.md` §3.1.2-a) —
    /// `Catalog`(불변)가 아니라 `ArcSwap<Catalog>` 다. 읽는 쪽(커맨드·트레이
    /// 메뉴 구성)이 여럿이고 쓰기는 사람이 `General` 탭에서 언어를 바꿀 때뿐이라
    /// `architecture.md` §2.2 가 설정 테이블에 쓴 것과 같은 근거로 이 자료구조를
    /// 쓴다. ⛔ 탭 콜백(hyperkey/presets/korean/perDevice)은 이 값을 읽지 않는다
    /// — 문자열은 UI 표면(설정 창·트레이 메뉴·온보딩 모달)에만 있다.
    catalog: ArcSwap<Catalog>,
    /// 엔진은 권한이 생긴 뒤에야 시작된다 — 그전에는 `None`.
    engine: Mutex<Option<Engine>>,
    /// ⭐ 이슈 #110 — 실행 중인 엔진의 `SharedState` 사본(락 없는 리프). 설정 창·
    /// Event Viewer·트레이가 "caps lock 커널 매핑 미적용" 을 판정할 때
    /// `state.engine` 뮤텍스를 잡지 않기 위해 둔다 — `reconfigure_engine` 이
    /// `engine` → `store` 순서로 잠그는데, `build_settings_state` 는 `store` 를 쥔
    /// 채 불리므로 그 안에서 `engine` 을 잡으면 순서가 뒤집혀 교착할 수 있다.
    /// 엔진이 새로 시작될 때마다 교체되고, 엔진이 없는 동안은 마지막 값이 남는다
    /// (그때의 표시는 권한 안내·`engine_not_running` 이 먼저 가린다).
    engine_shared: ArcSwapOption<SharedState>,
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
    /// ⭐ F-19 `日本語` 노드의 메모리 정본(F-19.3~F-19.6). `korean` 과 같은 캐시 규약.
    japanese: Mutex<ultrakey_language_presets::JapaneseSettings>,
    /// ⭐ F-19 `中文` 노드의 메모리 정본(F-19.7).
    chinese: Mutex<ultrakey_language_presets::ChineseSettings>,
    /// F-01 `Seek` 탭의 메모리 정본. `hyperkey`/`presets`/`korean` 과 같은 캐시 규약.
    seek: Mutex<SeekSettings>,
    /// Seek 워커(`seek.rs`)로 신호를 보내는 채널. 엔진이 아직 시작되지 않았으면
    /// (권한 대기 중) `None` — `send_seek_signal` 이 조용히 버린다.
    seek_tx: Mutex<Option<crossbeam_channel::Sender<seek::SeekSignal>>>,
    /// ⭐ F-06 — 트랙패드 제스처 리스너(`trackpad.rs`). 엔진 시작 시점에 뜨고,
    /// 설정 변경·종료 때 이 손잡이로 통신한다. 비공개 API 로드 실패 시 `thread`
    /// 가 없는 손잡이(격하)가 된다.
    trackpad: Mutex<Option<trackpad::TrackpadListener>>,
    /// ⭐ `Toggle Seek with shortcut:` 전역 단축키 등록기. **메인 스레드에서 만들고
    /// 앱 생애주기 내내 살려 둔다** — `global-hotkey` 크레이트 문서가 "macOS 에서는
    /// 메인 스레드의 실행 중인 이벤트 루프 위에서 만들어야 한다"고 명시한다
    /// (`global-hotkey-0.8.0/src/lib.rs` 모듈 문서). `overlay_demo::ScreenObserver`
    /// 를 `Box::leak` 하는 것과 같은 이유로, 여기서는 `AppState` 에 담아 두는 쪽을
    /// 골랐다 — 재등록(`Remap 설정 변경`)이 이 손잡이를 다시 써야 하므로 누수시켜
    /// 버리면 재사용할 수 없다.
    global_hotkey_manager: Mutex<Option<global_hotkey::GlobalHotKeyManager>>,
    /// 현재 등록돼 있는 `HotKey` — 설정이 바뀌면 이것부터 해제한 뒤 새로 등록한다.
    global_hotkey_registered: Mutex<Option<global_hotkey::hotkey::HotKey>>,
    /// 부트스트랩이 프런트엔드에 한 번만 알려줄 로드 경고(손상 복구/미래 스키마).
    load_notice: Mutex<Option<Notice>>,
    // ── F-10 메뉴바 상주(M2 2차) ──────────────────────────────────────────
    /// `NSStatusItem` 핸들. 드롭하면 아이콘이 사라지므로 앱 생애주기 내내 들고
    /// 있어야 한다. `setup_tray()` 가 채운다.
    tray: Mutex<Option<TrayIcon<Wry>>>,
    /// ⭐ 이슈 #110 — 트레이 메뉴가 마지막으로 조립될 때 반영된 "caps lock 커널
    /// 매핑 미적용" 값. `refresh_tray_caps_status` 가 변경 감지에 쓴다.
    tray_caps_missing: std::sync::atomic::AtomicBool,
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
    /// ⭐ 이슈 #68 — 두 번째 프로세스의 "설정창 띄워라" 분산 알림 구독.
    /// `system_event_observer` 와 같은 규약: 드롭되면 구독이 해지되므로 앱
    /// 생애주기 내내 들고 있어야 하고, 값 자체는 읽지 않는다. `setup()` 이 채운다.
    #[allow(dead_code)]
    show_settings_observer: Mutex<Option<ShowSettingsObserver>>,
    // ── 이슈 #32 Phase 1: 설정 창 크기 영속(F-15 규약) ──────────────────────
    /// 우리가 방금 `resize_settings_window()` 로 프로그램적으로 넣은 논리 크기.
    /// `on_settings_window_resized` 가 이 값과 같은 `Resized` 이벤트를 "사용자가
    /// 조절한 것"으로 착각하지 않도록 걸러내는 데 쓴다 — `settings_set_tab` 의
    /// `persist` 인자와 같은 이유(F-15 §8, 그 문서 주석 참고)로 존재한다.
    last_applied_window_size: Mutex<Option<(u32, u32)>>,
    /// 창 크기 저장 디바운스 스레드의 손잡이. `wire_window_size_persistence` 가
    /// `setup()` 안에서 한 번 채운다 — 이벤트마다 스레드를 새로 만들지 않는다.
    window_size_debouncer: Mutex<Option<WindowSizeDebouncer>>,
    // ── F-18 Event Viewer(이슈 #39 Phase 3) ─────────────────────────────────
    /// 드레인 스레드가 채우는 화면용 버퍼(§3.5, 상한 [`EVENT_VIEWER_BUFFER_CAP`]) —
    /// 넘치면 앞에서 버린다. `open_event_viewer` 가 등록하는 싱크가 여기 쓰고,
    /// `eventviewer_poll` 이 여기서 읽는다. 창을 닫으면 비운다(§3.2).
    event_viewer_buffer: Mutex<VecDeque<ultrakey_engine::trace::ViewerRecord>>,
    // ── F-13 자동 업데이트(Sparkle) ────────────────────────────────────────
    /// General 탭 체크박스가 그리는 자동 확인 상태 — 정본은 Sparkle 의
    /// UserDefaults(`SUEnableAutomaticChecks`)이고, 이 값은
    /// [`refresh_auto_update_checks`] 가 부팅·토글 시점에 동기화하는 미러다.
    /// `.app` 번들 밖(`tauri dev`)에서는 항상 `false`.
    auto_update_checks: ArcSwap<bool>,
    // ── F-12 라이선싱(이슈 #59) ────────────────────────────────────────────
    /// 라이선스 상태 판정·활성화·비활성화 컨트롤러. no-op provider(항상 활성) 가
    /// 기본이고, General 탭 UI 의 백엔드다.
    license_controller: Arc<license::LicenseController>,
}

impl AppState {
    /// [`GeneralView::auto_update`] 를 채우는 미러 읽기. 토글 결과를
    /// [`refresh_auto_update_checks`] 가 다시 읽어 반영하므로, 미러가 어긋날
    /// 수 있는 다른 쓰기 경로가 없다(Sparkle 의 UserDefaults 는 이 체크박스가
    /// 유일한 작성자 — launch-on-login 과 달리 OS 측 변경이 없다).
    fn auto_update_checks_enabled(&self) -> bool {
        **self.auto_update_checks.load()
    }
}

#[tauri::command]
fn modal_copy(state: State<'_, Arc<AppState>>) -> ModalCopy {
    let catalog = state.catalog.load_full();
    let catalog = catalog.as_ref();
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
    tracing::info!(kind = copy.kind, "modal_copy command invoked");
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
    let catalog = state.catalog.load_full();
    let hyperkey = state.hyperkey.lock().unwrap().clone();
    let presets = *state.presets.lock().unwrap();
    let korean = *state.korean.lock().unwrap();
    let japanese = *state.japanese.lock().unwrap();
    let chinese = *state.chinese.lock().unwrap();
    let seek = state.seek.lock().unwrap().clone();
    let store = state.store.lock().unwrap();
    let settings_state =
        build_settings_state(
            &hyperkey,
            &presets,
            &korean,
            &japanese,
            &chinese,
            &seek,
            &store,
            state.auto_update_checks_enabled(),
            None,
            None,
            caps_lock_kernel_map_missing(&state),
        );
    let meta = build_app_meta(&app, &store);
    drop(store);
    let notice = state.load_notice.lock().unwrap().clone();

    tracing::info!("settings_bootstrap command invoked");

    SettingsBootstrap {
        locale: catalog.locale().code().to_string(),
        strings: catalog.entries(),
        state: settings_state,
        meta,
        notice,
        languages: language_options(),
    }
}

/// 이슈 #87 — About 창 부트스트랩. 설정 창과 달리 설정 상태·언어 목록은 필요 없다
/// (정보 행 + 업데이트 버튼뿐) — 카탈로그 + `AppMeta` + 번들 여부만 실어 보낸다.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AboutBootstrap {
    locale: String,
    strings: BTreeMap<String, String>,
    meta: AppMeta,
    /// `.app` 번들 밖(`tauri dev`)에서는 `Check for Updates…` 버튼을 비활성화하고
    /// 설치 위치 행을 `about.outside_bundle` 안내로 대체한다(F-13 과 같은 기준,
    /// `menu-bar-and-lifecycle.md` §3.3 의 메뉴 항목 비활성과 동일).
    outside_bundle: bool,
}

/// 이슈 #87 — `about.html` 이 `invoke` 하는 부트스트랩 커맨드.
#[tauri::command]
fn about_bootstrap(state: State<'_, Arc<AppState>>, app: tauri::AppHandle) -> AboutBootstrap {
    let catalog = state.catalog.load_full();
    let store = state.store.lock().unwrap();
    let meta = build_app_meta(&app, &store);
    drop(store);

    tracing::info!("about_bootstrap command invoked");

    AboutBootstrap {
        locale: catalog.locale().code().to_string(),
        strings: catalog.entries(),
        meta,
        outside_bundle: !bundle::is_running_from_app_bundle(),
    }
}

/// 이슈 #87 — About 창의 `Check for Updates…` 버튼. 얇은 래퍼일 뿐이다 — 실제
/// 업데이트 확인은 F-13 의 `on_menu_check_for_updates` 가 그대로 담당하고, 진행·
/// 결과(있음/최신/오류) 표시는 Sparkle 표준 UI 에 위임한다(`auto-update.md` §2
/// 시나리오 3). `about.html` 은 번들 밖에서 버튼을 비활성화하지만, 메뉴 항목과
/// 같은 근거로 방어적으로 한 번 더 확인한다.
#[tauri::command]
fn check_for_updates(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) {
    on_menu_check_for_updates(&app, state.inner());
}

/// 이슈 #87 — About 창의 웹사이트·연락·이슈 링크. WebView 안에서 네비게이션하지
/// 않고(`open_keyboard_settings` 의 관례, 이슈 #31) 시스템 기본 브라우저로 연다.
/// 열 URL 은 `about.html` 이 자기 코드 상수로 고정해 보내는 것뿐이므로 앞 쪽에서
/// http(s) 만 허용해 어뷰징을 방지한다.
#[tauri::command]
fn open_external_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(format!("refusing to open non-http(s) url: {url}"));
    }
    if bundle::open_url(&url) {
        Ok(())
    } else {
        Err(format!("failed to open url: {url}"))
    }
}

/// ⭐ A-4 — 언어 선택 팝업의 선택지. `Locale::all()` 순서를 그대로 따른다
/// (en·ko·zh·es·ja). "System" 항목은 여기 없다 — 그것은 카탈로그 키
/// (`settings.general.language.system`)로 번역되는 유일한 항목이라 프런트가
/// 직접 덧붙인다.
fn language_options() -> Vec<LanguageOption> {
    Locale::all()
        .iter()
        .map(|locale| LanguageOption {
            code: locale.code(),
            endonym: locale.endonym(),
        })
        .collect()
}

/// 현재 저장된 값 그대로 `SettingsState` 를 다시 조립한다 — `general.*` 커맨드처럼
/// hyperkey/presets 를 건드리지 않는 변경 뒤에 새 상태를 돌려줄 때 쓴다.
fn current_settings_state(state: &Arc<AppState>) -> Result<SettingsState, String> {
    let hyperkey = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let presets = *state.presets.lock().map_err(|e| e.to_string())?;
    let korean = *state.korean.lock().map_err(|e| e.to_string())?;
    let japanese = *state.japanese.lock().map_err(|e| e.to_string())?;
    let chinese = *state.chinese.lock().map_err(|e| e.to_string())?;
    let seek = state.seek.lock().map_err(|e| e.to_string())?.clone();
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(
        &hyperkey,
        &presets,
        &korean,
        &japanese,
        &chinese,
        &seek,
        &store,
        state.auto_update_checks_enabled(),
        None,
        None,
        caps_lock_kernel_map_missing(state),
    ))
}

/// hyperkey.* 변경 뒤 `Engine::reconfigure` + (필요하면) `force_reset_state` 를
/// 함께 호출한다. presets.*/korean.*/japanese.*/chinese.*/seek.* 경로가 전부 이
/// 함수를 공유해 엔진 반영 로직이 여러 곳에 흩어지지 않게 한다.
/// ⭐ F-19 — japanese/chinese 설정을 추가로 받는다(setting 가족 5 + state = 8).
#[allow(clippy::too_many_arguments)]
fn reconfigure_engine(
    state: &Arc<AppState>,
    hyperkey: &HyperkeySettings,
    presets: &PresetSettings,
    korean: &KoreanSettings,
    japanese: &JapaneseSettings,
    chinese: &ChineseSettings,
    seek: &SeekSettings,
    force_reset: bool,
) -> Result<(), String> {
    let engine_guard = state.engine.lock().map_err(|e| e.to_string())?;
    if let Some(engine) = engine_guard.as_ref() {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let config =
            build_engine_config(hyperkey, presets, korean, seek, japanese, chinese, &store);
        drop(store);
        engine.reconfigure(config);
        if force_reset {
            engine.force_reset_state();
        }
    }
    drop(engine_guard);
    // ⭐ 이슈 #110 — `Engine::reconfigure` 가 경로 B 를 동기 재적용했으므로 되읽기
    // 결과가 바뀌었을 수 있다. 엔진 락을 놓은 뒤 트레이 상태 항목을 갱신한다.
    refresh_tray_caps_status(state);
    if let Err(e) = reconfigure_trackpad(state, hyperkey) {
        tracing::warn!(error = %e, "failed to reconfigure the trackpad gesture listener");
    }
    // 엔진이 아직 없으면(권한 대기 중) 건너뛴다 — 다음 `Engine::start` 가 이미
    // 갱신된 `state.hyperkey`/`state.presets`/`state.korean`/`state.seek` 으로
    // 조립되므로 이 변경이 유실되지 않는다.
    Ok(())
}

/// ⭐ F-06 — `hyperkey.trackpad.*` 변경을 리스너에 반영한다(이슈 #63).
///
/// - 끄는 변경이면 리스너가 Engaged 를 즉시 강제 해제한다(§5 항목 12) —
///   설정 변경이 상태 머신에 즉시 반영되어야 stuck hyper 를 막을 수 있다.
/// - 리스너가 아직 없는데 설정이 켜졌고 엔진이 살아 있으면, 여기서 리스너를
///   새로 띄운다(엔진 시작 시점에 꺼져 있던 경우를 흡수한다).
/// - 비공개 API 격하(스레드 없음)면 조용히 무시된다 — 게이트는 `Off` 유지.
fn reconfigure_trackpad(state: &Arc<AppState>, hyperkey: &HyperkeySettings) -> Result<(), String> {
    let want = hyperkey.trackpad.enabled && hyperkey.hyper.enabled;
    let mut slot = state.trackpad.lock().map_err(|e| e.to_string())?;
    match slot.as_ref() {
        Some(listener) => {
            listener.reconfigure(trackpad::TrackpadRuntimeConfig {
                enabled: want,
                area: hyperkey.trackpad.area,
            });
        }
        None => {
            if want {
                let shared = state
                    .engine
                    .lock()
                    .ok()
                    .and_then(|guard| guard.as_ref().map(ultrakey_engine::Engine::shared));
                if let Some(shared) = shared {
                    *slot = Some(trackpad::spawn(
                        &shared,
                        trackpad::TrackpadRuntimeConfig {
                            enabled: true,
                            area: hyperkey.trackpad.area,
                        },
                    ));
                }
            }
        }
    }
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
    app: tauri::AppHandle,
    key: String,
    value: serde_json::Value,
) -> Result<SettingsState, String> {
    if key == settings_keys::GENERAL_LAUNCH_ON_LOGIN {
        let on = value
            .as_bool()
            .ok_or_else(|| format!("{key} must be a boolean"))?;
        set_launch_on_login_internal(&state, on)?;
        return current_settings_state(&state);
    }
    if key == settings_keys::GENERAL_HIDE_MENU_BAR_ICON {
        let on = value
            .as_bool()
            .ok_or_else(|| format!("{key} must be a boolean"))?;
        set_hide_menu_bar_icon_internal(&state, on)?;
        return current_settings_state(&state);
    }
    if key == settings_keys::GENERAL_LANGUAGE {
        return settings_set_general_language(&app, &state, value);
    }
    if key == settings_keys::GENERAL_AUTO_UPDATE {
        return settings_set_auto_update_check(&app, &state, value);
    }

    if key.starts_with("presets.") {
        return settings_set_preset(&state, &key, &value);
    }

    if key.starts_with("korean.") {
        return settings_set_korean(&state, &key, &value);
    }

    if key.starts_with("japanese.") {
        return settings_set_japanese(&state, &key, &value);
    }

    if key.starts_with("chinese.") {
        return settings_set_chinese(&state, &key, &value);
    }

    if key.starts_with("perDevice.") {
        return settings_set_per_device(&state, &key, &value);
    }

    // ⚠️ `seek.searchBar.x`/`seek.searchBar.y`(F-03) 는 UI 가 이 커맨드로 쓰는
    // 키가 아니다(검색 바를 끌어 옮기면 `overlay.rs`/`seek.rs` 가 직접 저장한다)
    // — `settings_set_seek`/`apply_seek_setting` 은 그 둘을 매치하지 않으므로
    // 자연히 거부된다.
    if key.starts_with("seek.") {
        return settings_set_seek(&state, &app, &key, &value);
    }

    settings_set_hyperkey(&state, &key, &value)
}

/// `settings_set` 의 `general.language` 경로(D6, 이슈 #39, §3.1.2-a). 값이
/// 로케일 코드 문자열이면 그 로케일로, `null`(`System`)이면 키를 지워 시스템
/// 로케일을 따르게 한다. 두 경우 모두 ⭐ **즉시** 카탈로그를 교체하고 트레이
/// 메뉴를 다시 만든다 — 재시작을 요구하지 않는다. 프런트엔드는 이 반환값을
/// 쓰지 않고 `settings_bootstrap` 을 다시 불러 전체를 재렌더한다(§3.1.2-a "적용
/// 시점" 표 — 환경설정 창 표면) — 그래도 계약을 지키기 위해 유효한
/// `SettingsState` 를 돌려준다.
fn settings_set_general_language(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    value: serde_json::Value,
) -> Result<SettingsState, String> {
    let new_catalog = if value.is_null() {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        remove_setting_key(&mut store, settings_keys::GENERAL_LANGUAGE)?;
        drop(store);
        tracing::info!("general.language unset; following the system language again");
        Catalog::resolve(&bundle::preferred_languages())
    } else {
        let code = value.as_str().ok_or_else(|| {
            format!("{} must be a language code string or null", settings_keys::GENERAL_LANGUAGE)
        })?;
        let locale = Locale::all()
            .iter()
            .copied()
            .find(|l| l.code() == code)
            .ok_or_else(|| format!("unknown language code: {code}"))?;
        {
            let mut store = state.store.lock().map_err(|e| e.to_string())?;
            store
                .set(settings_keys::GENERAL_LANGUAGE, &code)
                .map_err(|e| e.to_string())?;
        }
        tracing::info!(locale = locale.code(), "general.language set");
        Catalog::for_locale(locale)
    };

    state.catalog.store(Arc::new(new_catalog));
    rebuild_tray_menu(app, state);

    // ⭐ 이슈 #87 상급 리뷰 #1 — 카탈로그가 바뀌었으므로 상주 About 창 내용·타이틀바를
    // 즉시 갱신한다(열려 있지 않으면 아무 일도 하지 않는다 — 열릴 때 새 카탈로그를 탐).
    notify_about_catalog_changed(app, state);

    current_settings_state(state)
}

/// F-13 — General 탭 `Check for updates automatically` 체크박스 토글.
///
/// 저장 계층은 `SettingsStore` 가 아니라 **Sparkle 의 UserDefaults** 다
/// (`settings_keys::GENERAL_AUTO_UPDATE` 주석). 이 함수는:
/// 1) 플러그인 API 로 Sparkle 에 요청하고
/// 2) **실제 도달한 상태를 다시 읽어** 미러를 갱신한 뒤(launch-on-login B-1 교훈:
///    Ok 를 받았다는 것과 값이 반영됐다는 것은 다르다)
/// 3) 새 `SettingsState` 를 돌려준다.
///
/// `.app` 번들 밖(`tauri dev`)에서는 플러그인이 준비되지 않아 오류를 돌려준다 —
/// General 탭 체크박스는 .app 에서만 동작하는 것이 정상이다(명세 §7, §8).
fn settings_set_auto_update_check(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    value: serde_json::Value,
) -> Result<SettingsState, String> {
    let enabled = value
        .as_bool()
        .ok_or_else(|| format!("{} must be a boolean", settings_keys::GENERAL_AUTO_UPDATE))?;

    refresh_auto_update_check(app, state, Some(enabled))?;
    current_settings_state(state)
}

/// F-13 — 플러그인(Sparkle)에 자동 확인 상태를 적용하고 결과를 미러에 반영한다.
///
/// `want` 가 `Some(on)` 이면 먼저 `set_automatically_checks_for_updates` 를
/// 호출하고, 그 뒤 **실제 상태를 다시 읽어** 미러에 반영한다 — API 가 `Ok` 를
/// 줬어도 UserDefaults 에 반영되지 않았을 수 있으므로 값 확인을 생략하지 않는다
/// (launch-on-login B-1 과 같은 순서). `None` 이면 요청 없이 현재 상태만 읽는다
/// (부팅 시점 동기화).
fn refresh_auto_update_check(
    app: &tauri::AppHandle,
    state: &Arc<AppState>,
    want: Option<bool>,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_sparkle_updater::SparkleUpdaterExt;
        let Some(updater) = app.sparkle_updater() else {
            // .app 밖 — 토글 요청이면 명시적으로 실패를 알리고, 부팅 동기화면
            // 미러를 false 로 유지한다(dev 모드에서는 자동 확인이 애초에 없다).
            if want.is_some() {
                return Err(state
                    .catalog
                    .load()
                    .get("menu.check_for_updates.not_ready")
                    .to_string());
            }
            state.auto_update_checks.store(Arc::new(false));
            return Ok(());
        };

        if let Some(on) = want {
            updater
                .set_automatically_checks_for_updates(on)
                .map_err(|e| e.to_string())?;
        }
        let reached = updater
            .automatically_checks_for_updates()
            .map_err(|e| e.to_string())?;
        if want == Some(reached) || want.is_none() {
            state.auto_update_checks.store(Arc::new(reached));
            tracing::info!(enabled = reached, "automatic update checks state synchronized");
        } else {
            tracing::error!(
                requested = want,
                reached,
                "automatic update checks did not reach the requested state"
            );
            state.auto_update_checks.store(Arc::new(reached));
            return Err(state
                .catalog
                .load()
                .get("menu.check_for_updates.not_ready")
                .to_string());
        }
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, state, want);
    }
    Ok(())
}

/// F-13 메뉴바 `Check for Updates…` 클릭 — 즉시 appcast 를 재조회한다.
/// 결과 표시("업데이트 있음"/"최신 버전"/오류)는 전부 **Sparkle 표준 UI** 가
/// 처리한다(명세 §2 시나리오 3, `check_for_updates` 는 `SPUStandardUpdater
/// Controller` 의 네이티브 대화상자를 연다). `.app` 밖에서는 항목이 비활성이라
/// 여기 도달하지 않는 것이 정상이지만, 방어적으로 한 번 더 확인한다.
fn on_menu_check_for_updates(app: &tauri::AppHandle, _state: &Arc<AppState>) {
    #[cfg(target_os = "macos")]
    {
        use tauri_plugin_sparkle_updater::SparkleUpdaterExt;
        match app.sparkle_updater() {
            Some(updater) => match updater.check_for_updates() {
                Ok(()) => tracing::info!("manual check for updates started"),
                Err(e) => tracing::error!(error = %e, "manual check for updates failed"),
            },
            None => tracing::warn!(
                "manual check for updates ignored: updater not ready (outside an .app bundle)"
            ),
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

/// F-13 §5-10 — Sparkle 이 업데이트를 설치하고 **재시작하기 직전**에 F-10 의
/// 종료 전 정리 계약을 실행한다: event tap 을 쥔 채 종료되면 눌려 있던 합성
/// modifier 가 시스템에 남는다(`menu-bar-and-lifecycle.md` §5 항목 5).
///
/// 배선: 플러그인 이벤트 `sparkle://will-relaunch-application` — Sparkle 의
/// `updaterWillRelaunchApplication:` delegate 가 **프로세스 종료 전에 동기로**
/// 부른다(tauri-plugin-sparkle-updater `delegate.rs`). 이 콜백 안에서
/// `force_reset_state()`(modifier 해소) → `shutdown()`(blocking — 탭 스레드가
/// 정리를 마칠 때까지 join) 순서로 실행하므로, 콜백이 끝나는 시점에는
/// 유저-보이는 상태가 이미 원상복구된 뒤다.
fn prepare_engine_for_update_restart(state: &Arc<AppState>) {
    let mut guard = match state.engine.lock() {
        Ok(g) => g,
        Err(e) => {
            tracing::error!(error = %e, "failed to lock engine before update relaunch");
            return;
        }
    };
    if let Some(engine) = guard.take() {
        engine.force_reset_state();
        engine.shutdown();
        tracing::info!("engine cleaned up before Sparkle update relaunch");
    }
    // ⭐ F-06 — 리스너도 함께 정리한다(게이트 `Off` 보장, §8).
    if let Ok(mut trackpad) = state.trackpad.lock() {
        if let Some(listener) = trackpad.take() {
            listener.shutdown();
            tracing::info!("trackpad gesture listener cleaned up before update relaunch");
        }
    }
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
    let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
    let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;
    let seek_snapshot = state.seek.lock().map_err(|e| e.to_string())?.clone();

    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        remove_setting_key(&mut store, &key)?;
    }

    reconfigure_engine(
        &state,
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        false,
    )?;

    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        &store,
        state.auto_update_checks_enabled(),
        None,
        None,
        caps_lock_kernel_map_missing(&state),
    ))
}

/// `settings_copy_common_to_device` 의 파라미터 검증만 뽑아낸 순수 함수(테스트
/// 대상) — 커맨드 본체가 `State` 를 받아 직접 호출하기 어렵기 때문에
/// `validate_per_device_key` 와 같은 관례를 따른다(그것도 순수 함수로 뽑아 인라인
/// 테스트한다).
///
/// 거부 조건(계획 §5-(b)): ① `device == "all"`(공통 계층 위에는 복사 원천이 없다)
/// ② `DeviceId::parse` 실패 ③ `feature` 가 `keyRemap`/`functionKeys` 화이트리스트 밖.
fn validate_copy_common_to_device_args(device: &str, feature: &str) -> Result<(), String> {
    if device == keys::PER_DEVICE_COMMON_SCOPE {
        return Err(format!(
            "device {device} is the common scope; copy targets a specific device"
        ));
    }
    DeviceId::parse(device).ok_or_else(|| format!("unknown device identifier: {device}"))?;
    if feature != "keyRemap" && feature != "functionKeys" {
        return Err(format!("unknown feature: {feature}"));
    }
    Ok(())
}

/// ⭐ 이슈 #46 — "공통 설정 복사" 배치 커맨드. 공통(`For all devices`) 계층의
/// 값을 선택된 디바이스 계층으로 물질화한다(복사 = 독립 스냅샷 — 계획 D3).
///
/// 순서(계획 §5-(b)): 1) 파라미터 화이트리스트 검증 2) store 락 1회 — `PerDevice
/// Settings` 로 복사 계획 계산(`perdevice::inherit`, 소유 타입으로 추출해 borrow
/// 종료 — 계획 계산과 쓰기를 **같은 락 안**에서 묶어 락을 한 번만 잡는다) 3) 같은
/// 락 안에서 쓰기(기능 1 = `keyRemap.rows` 1회, 기능 2 = 계획 수만큼(≤ 12) f-키
/// 쓰기) 4) 락 해제 → `reconfigure_engine` → `build_settings_state`.
///
/// **계획이 비어 있으면(복사할 것이 없음) 아무 키도 쓰지 않고** 현재 상태를 그대로
/// 돌려준다(에러 아님) — 프런트가 활성 조건으로 막지만 백엔드도 멱등·안전해야
/// 한다. 쓰기 실패 관용(D-B)은 `settings_set_per_device` 와 같다 — 실패해도
/// 메모리는 이미 갱신돼 엔진 반영·상태 반환이 그대로 진행되고 `save_error` 만
/// 실린다.
#[tauri::command]
fn settings_copy_common_to_device(
    state: State<'_, Arc<AppState>>,
    device: String,
    feature: String,
) -> Result<SettingsState, String> {
    validate_copy_common_to_device_args(&device, &feature)?;

    let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let presets_snapshot = *state.presets.lock().map_err(|e| e.to_string())?;
    let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;
    let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
    let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;
    let seek_snapshot = state.seek.lock().map_err(|e| e.to_string())?.clone();

    let mut store = state.store.lock().map_err(|e| e.to_string())?;

    // 복사 계획 계산 — `settings` 의 store borrow 는 이 블록 스코프 끝에서 종료되어
    // 아래 쓰기 루프(`&mut store`)와 충돌하지 않는다.
    let plan_remap: Option<Vec<KeyRemapRow>>;
    let plan_fnkeys: Vec<(FKey, String)>;
    {
        let settings = PerDeviceSettings::new(store.values());
        match feature.as_str() {
            "keyRemap" => {
                plan_remap = inherit::plan_key_remap_copy(&settings);
                plan_fnkeys = Vec::new();
            }
            "functionKeys" => {
                plan_remap = None;
                plan_fnkeys = inherit::plan_function_keys_copy(&settings);
            }
            _ => {
                unreachable!("validate_copy_common_to_device_args 가 이미 화이트리스트로 거부했다")
            }
        }
    }

    // 계획이 비어 있으면(복사할 것이 없음) 아무 키도 쓰지 않는다. 락을 먼저 놓아야
    // `current_settings_state` 의 재락이 데드락하지 않는다(std::sync::Mutex 는
    // 재진입 불가).
    let has_plan = plan_remap.is_some() || !plan_fnkeys.is_empty();
    if !has_plan {
        drop(store);
        return current_settings_state(&state);
    }

    let save_error = if let Some(rows) = &plan_remap {
        let key = keys::per_device_key_remap_rows(&device);
        match store.set(&key, rows) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "failed to save setting");
                Some(e.to_string())
            }
        }
    } else {
        let mut save_error = None;
        for (f, id) in &plan_fnkeys {
            let key = keys::per_device_function_key(&device, *f);
            if let Err(e) = store.set(&key, id) {
                tracing::error!(key = %key, error = %e, "failed to save setting");
                save_error = Some(e.to_string());
            }
        }
        save_error
    };
    drop(store);

    reconfigure_engine(
        &state,
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        false,
    )?;

    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        &store,
        state.auto_update_checks_enabled(),
        save_error,
        None,
        caps_lock_kernel_map_missing(&state),
    ))
}

/// F-17 §3.5.1 — `시스템 설정 열기` 버튼.
///
/// ⭐ 이슈 #31 ③(a) — **Function Keys 패널로 바로 들어간다.** 예전 URL
/// (`com.apple.preference.keyboard`)은 Ventura 이전 이름이라 지금은 Keyboard
/// 최상단만 열렸다 — 사용자 보고: "시스템 세팅스를 열었을 때 해당 메뉴로 바로
/// 진입하지 않는다". 아래 URL 은 Karabiner-Elements 의
/// `Open System Settings > Function Keys…` 버튼이 쓰는 것과 같은 문자열이고,
/// 이 머신의 `/Applications/Karabiner-Elements.app` 바이너리 `strings` 에서도
/// 그대로 확인했다. 확장자 번들 ID `com.apple.Keyboard-Settings.extension` 은
/// 이 머신의 `Keyboard-Settings.appex` `Info.plist` 로 교차 확인했다.
const KEYBOARD_FUNCTION_KEYS_SETTINGS_URL: &str =
    "x-apple.systempreferences:com.apple.Keyboard-Settings.extension?FunctionKeys";

/// 실패해도 앱은 죽지 않고 프런트에 `Err` 로만 알린다(`settings.html` 의
/// `keyboards-open-system-settings-btn` 리스너가 `.catch()` 로 받는다).
#[tauri::command]
fn open_keyboard_settings() -> Result<(), String> {
    if bundle::open_url(KEYBOARD_FUNCTION_KEYS_SETTINGS_URL) {
        Ok(())
    } else {
        Err("failed to open keyboard system settings".to_string())
    }
}

/// ⭐ 이슈 #31 ③(b) — `Use F1, F2, etc. keys as standard function keys` 의 **현재**
/// 값만 따로 읽는다. 사용자 보고: "그 메뉴에서 옵션을 껐음에도 울트라키에서 옵션의
/// 변경 상태를 인지하지 못하는 상황. 그래서 항상 on 으로 표현되는 것 같거든."
///
/// 값 자체는 `settings_bootstrap`/`settings_set` 이 돌려주는 `state.perDevice`
/// 안에도 들어 있지만, 그 경로는 **설정을 건드릴 때만** 갱신된다 — 사용자가 시스템
/// 설정 앱에서 토글을 바꾼 것은 우리 쪽에 아무 일도 일으키지 않는다. 그래서
/// 프런트가 이 커맨드만 따로 주기적으로 부른다(§3.5.1).
///
/// ⚠️ **변경 알림이 아니라 폴링이다.** macOS 가 이 값의 변경을 알려 주는 공개
/// 알림을 우리가 확인하지 못했다(`Keyboard-Settings.appex` 바이너리에
/// `com.apple.keyboard.fnstatedidchange` 문자열이 보이지만, 그것이 실제로
/// 분산 알림으로 게시되는지는 확인하지 못했다 — ⛔ 추측으로 배선하지 않는다).
/// Karabiner-Elements 도 같은 값을 3초 주기로 폴링한다. 폴링 비용은 IOKit
/// 레지스트리 프로퍼티 1회 읽기라 무시할 만하고, 프런트는 Keyboards 탭이 보일
/// 때만 호출한다.
#[tauri::command]
fn keyboard_fn_state() -> Option<bool> {
    fn_state::f_keys_are_standard()
}

// ════════════════════════════════════════════════════════════════════════════
// F-15 §3.4 — 설정 export/import (이슈 #39 Phase 2)
// ════════════════════════════════════════════════════════════════════════════

fn epoch_secs_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// 유닉스 일수(1970-01-01 = 0) → (년, 월, 일). Howard Hinnant 의
/// `civil_from_days` 알고리즘(그레고리력, 공개 도메인) — 새 시간 크레이트를 들이지
/// 말라는 지시(위임 지시서, `crates/ultrakey-core/src/time.rs` 가 이미 "시계는
/// 호출자가 주입한다"고 선언한 것과 같은 근거) 때문에 순수 정수 연산으로 직접 쓴다.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

/// `exported_at` 용 RFC3339 UTC 문자열(`"2026-08-31T00:12:34Z"`) — `SystemTime`
/// 에서 직접 만든다(A-2 지시: 새 시간 크레이트를 들이지 않는다).
fn rfc3339_utc(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let tod = epoch_secs % 86_400;
    let (year, month, day) = civil_from_days(days);
    let (h, m, s) = (tod / 3600, (tod / 60) % 60, tod % 60);
    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

/// export 기본 파일명 — `ultrakey-settings-<YYYYMMDD>.json`(위임 지시서 A-2).
fn export_default_file_name(epoch_secs: u64) -> String {
    let days = (epoch_secs / 86_400) as i64;
    let (year, month, day) = civil_from_days(days);
    format!("ultrakey-settings-{year:04}{month:02}{day:02}.json")
}

/// `settings_export` 커맨드의 응답.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportResultView {
    path: String,
}

/// `settings_import` 커맨드의 응답 — `transfer::ImportOutcome` 을 프런트가 그대로
/// 쓸 수 있게 camelCase 로 옮긴 것뿐이다(§3.4).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ImportResultView {
    applied_keys: usize,
    removed_keys: usize,
    absent_devices: Vec<String>,
    backup_path: Option<String>,
}

impl From<transfer::ImportOutcome> for ImportResultView {
    fn from(outcome: transfer::ImportOutcome) -> Self {
        ImportResultView {
            applied_keys: outcome.applied_keys,
            removed_keys: outcome.removed_keys,
            absent_devices: outcome.absent_devices,
            backup_path: outcome.backup.map(|p| p.display().to_string()),
        }
    }
}

/// F-15 §3.4 — `General` 탭의 `Export…` 버튼.
///
/// ⭐ 파일 대화상자는 Rust 쪽에서만 연다(위임 지시서, `settings-store-and-
/// integrity.md` §3.4). 프런트엔드는 이 커맨드만 부르고, `capabilities/` 는
/// 건드리지 않는다 — ACL 은 웹뷰가 `plugin:dialog|...` 를 직접 invoke 할 때만
/// 관문 역할을 하고, 이 커맨드처럼 Rust 코드가 플러그인의 Rust API
/// (`DialogExt::dialog()`)를 직접 호출하는 경로는 그 관문을 거치지 않는다.
/// `settings` 창은 지금도 `capabilities/overlay.json` 밖의 아무 capability 도
/// 없고(기존 커맨드들이 이미 그 상태로 동작한다), 이 변경도 그 상태를 유지한다.
///
/// ⭐ **왜 `async fn` + `blocking_save_file()`인가.** macOS 의 저장 패널
/// (`NSSavePanel`)은 메인 스레드 API 일 수 있다. `tauri_plugin_dialog` 의
/// `blocking_*` 계열은 자기 문서에 "메인 스레드에서 부르면 안 된다"고 적어
/// 두었고, 내부적으로 메인 스레드로 디스패치한 뒤 **호출 스레드만** 블로킹하며
/// 응답을 기다린다 — 그 크레이트 자신의 예제 코드도 정확히 `async fn` 커맨드
/// 안에서 이 함수를 쓴다. Tauri 커맨드 핸들러(`async fn` 이든 아니든)는 메인
/// 스레드가 아니라 별도 실행기 위에서 돈다는 사실은 이 파일의
/// `set_launch_on_login_internal` 주석(최대 0.8초 블로킹이 커맨드 스레드에서
/// 안전하다는 근거)과 같다 — 그래서 `blocking_save_file()` 을 그대로 쓴다.
#[tauri::command]
async fn settings_export(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<ExportResultView>, String> {
    use tauri_plugin_dialog::DialogExt;

    let state = state.inner().clone();
    let now = epoch_secs_now();

    let picked = app
        .dialog()
        .file()
        .set_file_name(export_default_file_name(now))
        .add_filter("JSON", &["json"])
        .blocking_save_file();

    // 사용자가 대화상자를 취소했다 — 오류가 아니다(위임 지시서 A-2).
    let Some(file_path) = picked else {
        return Ok(None);
    };
    let path = file_path.into_path().map_err(|e| e.to_string())?;

    let envelope = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        transfer::build_export(&store, env!("CARGO_PKG_VERSION"), rfc3339_utc(now))
    };
    let json = transfer::serialize_export(&envelope).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;

    tracing::info!(
        path = %path.display(),
        keys = envelope.values.len(),
        "settings exported"
    );

    Ok(Some(ExportResultView {
        path: path.display().to_string(),
    }))
}

/// F-15 §3.4 — `General` 탭의 `Import…` 버튼.
///
/// 순서를 반드시 지킨다(결정 6·7): 1) 파일을 읽고 파싱 — 실패하면 저장소를
/// 전혀 건드리지 않는다. 2) 지금 연결된 디바이스 목록(F-17 의 기존 열거 경로
/// `hid_device::list_attached_keyboards()` 를 재사용한다 — 새로 만들지 않는다).
/// 3) `transfer::apply_import` 로 교체(백업 → 교체). 4) **부팅과 같은 일을
/// 다시 한다** — [`reload_settings_after_replace`] 가 그 함수들을 그대로
/// 재호출한다(새 반영 경로를 만들지 않는다 — 부팅 경로는 이미 실기기로
/// 검증됐다).
#[tauri::command]
async fn settings_import(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<Option<ImportResultView>, String> {
    use tauri_plugin_dialog::DialogExt;

    let state = state.inner().clone();

    let picked = app.dialog().file().add_filter("JSON", &["json"]).blocking_pick_file();
    let Some(file_path) = picked else {
        return Ok(None);
    };
    let path = file_path.into_path().map_err(|e| e.to_string())?;

    // 1) 읽기 + 파싱 — 여기서 실패하면 저장소를 전혀 건드리지 않는다(§3.4 결정 7).
    let raw = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let envelope = transfer::parse_export(&raw).map_err(|e| e.to_string())?;

    // 2) 지금 연결된 디바이스 — F-17 의 기존 열거 경로를 그대로 쓴다.
    let present_devices: Vec<String> = hid_device::list_attached_keyboards()
        .into_iter()
        .map(|info| DeviceId::new(info.vendor_id, info.product_id).as_str().to_string())
        .collect();

    // 3) 백업 → 교체.
    let backup_suffix = format!("pre-import-{}", epoch_secs_now());
    let outcome = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        transfer::apply_import(&mut store, &envelope, &present_devices, &backup_suffix)
            .map_err(|e| e.to_string())?
    };

    // 4) 부팅과 같은 일을 다시 한다.
    reload_settings_after_replace(&app, &state)?;

    tracing::info!(
        path = %path.display(),
        applied = outcome.applied_keys,
        removed = outcome.removed_keys,
        absent_devices = outcome.absent_devices.len(),
        "settings imported"
    );

    Ok(Some(ImportResultView::from(outcome)))
}

/// import 뒤 "부팅과 같은 일을 다시 한다"(§3.4 결정 6)의 실제 구현. **새 반영
/// 경로가 아니다** — `setup()` 이 기동 시 쓰는 것과 같은 함수들
/// (`HyperkeySettings::from_store` 등, `reconfigure_engine`, `rebuild_tray_menu`)을
/// 저장소 교체 뒤 그대로 다시 부를 뿐이다. 부팅 경로는 이미 실기기로 검증된
/// 경로라 여기서 별도로 검증할 새 코드를 만들지 않는다.
fn reload_settings_after_replace(app: &tauri::AppHandle, state: &Arc<AppState>) -> Result<(), String> {
    // ⚠️ `seek` 도 함께 되살린다 — F-01(이슈 #39 와 병렬로 머지된 M3-3)이 `AppState`
    // 에 네 번째 메모리 정본을 더했다. 빠뜨리면 import 가 Seek 설정만 조용히
    // 반영하지 않는 "부분 교체"가 되어 §3.4 결정 4(교체)가 깨진다.
    let (hyperkey, presets, korean, japanese, chinese, seek, disabled_apps, korean_excluded, japanese_excluded, chinese_excluded) = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        let hyperkey = HyperkeySettings::from_store(&store);
        let presets = PresetSettings::from_store(&store);
        let korean = KoreanSettings::from_store(&store);
        let japanese = JapaneseSettings::from_store(&store);
        let chinese = ChineseSettings::from_store(&store);
        let seek = SeekSettings::from_store(&store);
        let disabled_apps: Vec<String> =
            store.get(settings_keys::GENERAL_DISABLED_APPS).unwrap_or_default();
        // ⭐ K5(D-K17) — 목록 오버라이드도 import 로 바뀔 수 있다. 같은 잠금 안에서
        // 함께 읽는다. F-19(D-7) — japanese/chinese 제외 목록도 같은 규약.
        let korean_excluded = ultrakey_korean::resolve_excluded_bundle_ids(
            store
                .get::<Vec<String>>(keys::KOREAN_EXCLUDED_BUNDLE_IDS)
                .as_deref(),
        );
        let japanese_excluded = ultrakey_korean::resolve_excluded_bundle_ids(
            store
                .get::<Vec<String>>(keys::JAPANESE_EXCLUDED_BUNDLE_IDS)
                .as_deref(),
        );
        let chinese_excluded = ultrakey_korean::resolve_excluded_bundle_ids(
            store
                .get::<Vec<String>>(keys::CHINESE_EXCLUDED_BUNDLE_IDS)
                .as_deref(),
        );
        (
            hyperkey,
            presets,
            korean,
            japanese,
            chinese,
            seek,
            disabled_apps,
            korean_excluded,
            japanese_excluded,
            chinese_excluded,
        )
    };

    *state.hyperkey.lock().map_err(|e| e.to_string())? = hyperkey.clone();
    *state.presets.lock().map_err(|e| e.to_string())? = presets;
    *state.korean.lock().map_err(|e| e.to_string())? = korean;
    *state.japanese.lock().map_err(|e| e.to_string())? = japanese;
    *state.chinese.lock().map_err(|e| e.to_string())? = chinese;
    *state.seek.lock().map_err(|e| e.to_string())? = seek.clone();

    // ⭐ 게이트도 boot 만큼 되돌린다 — `general.disabledApps`/
    // `korean.disableInRemoteDesktop` 도 import 로 바뀔 수 있는 값이다(D-K3 과
    // 같은 이유로 엔진 설정이 아니라 게이트라 `reconfigure_engine` 이 대신
    // 해주지 않는다). ⭐ K5(D-K17) — 제외 목록 오버라이드도 마찬가지다.
    // F-19(D-7) — japanese/chinese 제외 목록도 같이 재주입한다.
    state.gate_controller.set_disabled_apps(disabled_apps);
    state
        .gate_controller
        .set_korean_excluded_apps(korean_excluded);
    state
        .gate_controller
        .set_korean_exclusion_enabled(korean.disable_in_remote_desktop);
    state
        .gate_controller
        .set_japanese_excluded_apps(japanese_excluded);
    state
        .gate_controller
        .set_chinese_excluded_apps(chinese_excluded);

    // 규칙 테이블이 통째로 바뀔 수 있으므로 항상 force_reset 한다
    // (`settings_set_preset`/`settings_set_korean` 과 같은 이유).
    reconfigure_engine(state, &hyperkey, &presets, &korean, &japanese, &chinese, &seek, true)?;

    // 5) `general.language` 가 import 로 바뀌었으면 카탈로그도 교체한다
    // (A-2 언어 선택 경로, `settings_set_general_language` 와 같은 판정).
    let new_catalog = {
        let store = state.store.lock().map_err(|e| e.to_string())?;
        match resolve_stored_language(&store) {
            Some(locale) => Catalog::for_locale(locale),
            None => Catalog::resolve(&bundle::preferred_languages()),
        }
    };
    state.catalog.store(Arc::new(new_catalog));

    rebuild_tray_menu(app, state);

    // ⭐ 이슈 #87 상급 리뷰 #1 — import 로 카탈로그가 교체됐으므로(언어가 바뀌었는지
    // 여부와 무관하게 방어적으로) 상주 About 창도 갱신한다(열려 있지 않으면 무연산).
    notify_about_catalog_changed(app, state);
    Ok(())
}

// ════════════════════════════════════════════════════════════════════════════
// F-18 Event Viewer (이슈 #39 Phase 3, `docs/spec/event-viewer.md`)
// ════════════════════════════════════════════════════════════════════════════

const EVENT_VIEWER_WINDOW_LABEL: &str = "eventviewer";
/// §3.5 — 화면용 버퍼 상한. `eventviewer.html` 의 `MAX_ROWS` 와 같은 크기를 맞춘다.
const EVENT_VIEWER_BUFFER_CAP: usize = 500;

/// F-08 프리셋 번호(`RuleId::Preset(n)`) → 카탈로그 키(§3.4, `event-viewer.md` §9
/// 항목 1). ⭐ **`ultrakey-presets` 에는 이 대응표가 없다** — `rules.rs` 는
/// `RuleId::Preset(n)` 리터럴만 흩어 둘 뿐 이름을 모른다(탐색 완료). 그래서 대응을
/// 여기(앱 계층)에 둔다 — 숫자는 `ultrakey_presets::rules::PresetSettings::to_rules`
/// 의 `RuleId::Preset(n)` 배정과 반드시 같아야 한다(F-08.1~16). `3` 은 결번이다
/// (F-08.3 은 존재하지 않는다 — 실수가 아니다). `6`(F-08.8 vim 방향)은 라벨이
/// 문장 앞뒤로 쪼개져 있어(`caps_hjkl.prefix`/`.suffix`) 이 표에 넣지 않고
/// [`preset_rule_label`] 이 따로 이어붙인다.
///
/// ⚠️ **이 대응이 깨져도 아무도 못 잡는다는 위험**(§9 항목 1)을 막는 것이
/// `preset_rule_label_keys_exist_in_every_catalog` 테스트다 — 카탈로그 키
/// 오타·삭제를 5개 로케일 전부에서 잡는다.
const PRESET_RULE_LABEL_KEYS: &[(u8, &str)] = &[
    (1, "settings.presets.caps_remap"),
    (2, "settings.presets.caps_quick_press"),
    (4, "settings.presets.caps_space_enter"),
    (5, "settings.presets.caps_wasd"),
    (7, "settings.presets.caps_home_row"),
    (8, "settings.presets.double_tap_shift"),
    (9, "settings.presets.left_right_shift"),
    (10, "settings.presets.shift_caps"),
    (11, "settings.presets.shift_quick_press"),
    (12, "settings.presets.hyper_delete"),
    (13, "settings.presets.remap_delete"),
    (14, "settings.presets.shift_delete"),
    (15, "settings.presets.paste_plain"),
    (16, "settings.presets.home_end_lines"),
];

/// F-16 한국어 규칙 번호(`RuleId::Korean(n)`) → 카탈로그 키. `ultrakey_korean::
/// settings::KoreanSettings::to_rules` 의 `RuleId::Korean(n)` 배정과 같아야 한다.
const KOREAN_RULE_LABEL_KEYS: &[(u8, &str)] = &[
    (13, "settings.korean.shift_space"),
    (14, "settings.korean.han_eng"),
    (15, "settings.korean.hanja"),
    (16, "settings.korean.won_backtick"),
];

fn preset_rule_label(catalog: &Catalog, n: u8) -> Option<String> {
    if n == 6 {
        // §3.4 — 문장이 팝업 앞뒤로 쪼개져 있다(`settings.html` 의 같은 패턴).
        return Some(format!(
            "{} {}",
            catalog.get("settings.presets.caps_hjkl.prefix"),
            catalog.get("settings.presets.caps_hjkl.suffix"),
        ));
    }
    PRESET_RULE_LABEL_KEYS
        .iter()
        .find(|(idx, _)| *idx == n)
        .map(|(_, key)| catalog.get(key).to_string())
}

fn korean_rule_label(catalog: &Catalog, n: u8) -> Option<String> {
    KOREAN_RULE_LABEL_KEYS
        .iter()
        .find(|(idx, _)| *idx == n)
        .map(|(_, key)| catalog.get(key).to_string())
}

/// `ViewerRecord.rule`(`"preset:5"`·`"korean:13"`·`"hyperkey"`) → 사람이 읽는
/// 이름(§3.4). 못 찾으면 `None` — 프런트가 계층 이름만 보여준다(명세 그대로).
fn rule_label(catalog: &Catalog, rule: &str) -> Option<String> {
    if rule == "hyperkey" {
        // `settings.tab.hyperkey` 를 그대로 쓴다 — 사용자가 탭에서 본 그 이름이다.
        return Some(catalog.get("settings.tab.hyperkey").to_string());
    }
    if let Some(n) = rule.strip_prefix("preset:") {
        return preset_rule_label(catalog, n.parse().ok()?);
    }
    if let Some(n) = rule.strip_prefix("korean:") {
        return korean_rule_label(catalog, n.parse().ok()?);
    }
    None
}

/// §5 항목 1·2 — 왜 이벤트가 오지 않는지 알린다. 빈 화면으로 두지 않는다.
fn event_viewer_notice(catalog: &Catalog, state: &Arc<AppState>) -> Option<String> {
    let permission_state = state
        .monitor
        .lock()
        .ok()
        .and_then(|m| m.as_ref().map(|m| m.state()))
        .unwrap_or(PermissionState::Unknown);
    if permission_state != PermissionState::Granted {
        return Some(catalog.get("eventviewer.notice.no_permission").to_string());
    }
    let engine_running = state.engine.lock().map(|g| g.is_some()).unwrap_or(false);
    if !engine_running {
        return Some(catalog.get("eventviewer.notice.engine_not_running").to_string());
    }
    // ⭐ 이슈 #110 — D-1 이 필요한데 커널 매핑이 되읽기로 확인되지 않았다.
    if caps_lock_kernel_map_missing(state) {
        return Some(
            catalog
                .get("eventviewer.notice.caps_kernel_map_missing")
                .to_string(),
        );
    }
    None
}

/// `ultrakey_engine::trace::ViewerRecord` + `ruleLabel`(§3.4).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ViewerRecordView {
    #[serde(flatten)]
    record: ultrakey_engine::trace::ViewerRecord,
    rule_label: Option<String>,
}

/// `eventviewer_poll` 의 응답(§3.5).
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct EventViewerPoll {
    records: Vec<ViewerRecordView>,
    /// 계측 링이 가득 차 **버려진** 레코드의 누적 개수(`Engine::trace_dropped_count`).
    ///
    /// ⭐ 진단 도구가 자기 표시의 불완전함을 숨기면 사용자가 "이 키는 왜 안 보이지"를
    /// 잘못 해석하게 된다(§4·§5 항목 3). 엔진이 아직 없으면 0 이다 — 그때는 애초에
    /// 기록될 이벤트도 없다.
    dropped: u64,
    notice: Option<String>,
}

/// 이슈 #47 — `General` 탭의 `Open Log Folder in Finder` 버튼.
///
/// 위임 지시서의 제약("경로가 없으면(로그 미생성) 조용히 무시하거나 비활성화")을
/// 그대로 따른다 — 디렉터리 부재는 `Ok(())` + 경고 로그로 조용히 넘기고 **만들지도
/// 않는다**(이 커맨드는 읽기 전용이다: 디렉터리 생성은 기동 시 `open_log_file` 이
/// 맡는 고유 책임). 진짜 예외인 `open` spawn 실패만 `Err` 로 알린다 —
/// `open_event_viewer` 와 같은 기존 커맨드 계약(`Result<(), String>` + 프런트
/// 진단)이다. Finder 는 macOS 표준 `open <디렉터리>` CLI 로 연다(`on_menu_relaunch`
/// 선례) — 새 플랫폼 의존을 들이지 않는다.
#[tauri::command]
fn open_log_folder() -> Result<(), String> {
    let Some(dir) = log_dir() else {
        tracing::warn!("HOME is not set; cannot open the log folder");
        return Ok(());
    };
    if !dir.is_dir() {
        tracing::warn!(path = %dir.display(), "log folder does not exist; nothing to open");
        return Ok(());
    }
    std::process::Command::new("open")
        .arg(&dir)
        .spawn()
        .map_err(|e| e.to_string())?;
    tracing::info!(path = %dir.display(), "opened the log folder in Finder");
    Ok(())
}

/// F-18 §3.1·§3.2 — `General` 탭의 `Open Event Viewer` 버튼.
///
/// 오버레이 창(`overlay.rs`)과 같은 관례로 `tauri.conf.json` 에 미리 선언하지
/// 않고 여기서 동적으로 만든다 — 그 파일은 병렬 위임(#40)과 충돌할 수 있다
/// (위임 지시서). 이미 열려 있으면 새로 만들지 않고 최전면으로 올린다.
#[tauri::command]
fn open_event_viewer(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window(EVENT_VIEWER_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let window = tauri::WebviewWindowBuilder::new(
        &app,
        EVENT_VIEWER_WINDOW_LABEL,
        tauri::WebviewUrl::App("eventviewer.html".into()),
    )
    .title("Ultrakey — Event Viewer")
    .inner_size(700.0, 520.0)
    .resizable(true)
    // ⭐ §3.1 — `NSFloatingWindowLevel` 로 항상 위. 오버레이(`overlay.rs`)의
    // `ns_window()` 직접 조작(`NsWindowHandle::configure`, 스크린세이버 레벨 +
    // 비활성 클래스 치환)은 **여기 필요 없다** — 이 창은 오버레이와 달리
    // 활성화되어도 되고 키 윈도우를 뺏어도 된다(§3.1: 사용자가 스크롤·클릭해야
    // 한다). Tauri 의 `always_on_top(true)` 는 macOS 에서 그대로
    // `NSFloatingWindowLevel` 을 건다 — 그것으로 충분하다.
    .always_on_top(true)
    .build()
    .map_err(|e| e.to_string())?;

    let state_for_close = state.inner().clone();
    window.on_window_event(move |event| {
        if let WindowEvent::CloseRequested { .. } = event {
            // ⭐ §3.2 — 창이 닫히면 계측을 끈다. 사용자가 끄는 것을 잊어 계측이
            // 계속 도는 상태를 만들지 않는다. `ULTRAKEY_TRACE_TAP` 은 독립이라
            // 여기서 건드리지 않는다(그 환경변수 계측은 계속 돈다).
            ultrakey_engine::trace::set_viewer_enabled(false);
            ultrakey_engine::trace::set_viewer_sink(None);
            if let Ok(mut buf) = state_for_close.event_viewer_buffer.lock() {
                buf.clear();
            }
        }
    });

    let state_for_sink = state.inner().clone();
    let sink: ultrakey_engine::trace::ViewerSink = Arc::new(move |record| {
        if let Ok(mut buf) = state_for_sink.event_viewer_buffer.lock() {
            buf.push_back(record);
            while buf.len() > EVENT_VIEWER_BUFFER_CAP {
                buf.pop_front();
            }
        }
    });
    ultrakey_engine::trace::set_viewer_sink(Some(sink));
    ultrakey_engine::trace::set_viewer_enabled(true);

    tracing::info!("event viewer window opened");
    Ok(())
}

/// 프런트엔드가 100ms 마다 부른다(§3.5).
#[tauri::command]
fn eventviewer_poll(state: State<'_, Arc<AppState>>, after_seq: u64) -> EventViewerPoll {
    let catalog = state.catalog.load_full();
    let catalog: &Catalog = catalog.as_ref();

    let records: Vec<ViewerRecordView> = state
        .event_viewer_buffer
        .lock()
        .map(|buf| {
            buf.iter()
                .filter(|r| r.seq > after_seq)
                .cloned()
                .map(|record| {
                    let rule_label = record.rule.as_deref().and_then(|r| rule_label(catalog, r));
                    ViewerRecordView { record, rule_label }
                })
                .collect()
        })
        .unwrap_or_default();

    // 엔진 잠금은 짧게 잡는다 — 100ms 주기 폴링이라 여기서 오래 붙들면 안 된다.
    let dropped = state
        .engine
        .lock()
        .ok()
        .and_then(|e| e.as_ref().map(|engine| engine.trace_dropped_count()))
        .unwrap_or(0);

    EventViewerPoll {
        records,
        dropped,
        notice: event_viewer_notice(catalog, &state),
    }
}

/// `Clear` 버튼 — 앱 쪽 버퍼를 비운다(§3.2, 화면 쪽은 `eventviewer.html` 이 직접 비운다).
#[tauri::command]
fn eventviewer_clear(state: State<'_, Arc<AppState>>) {
    if let Ok(mut buf) = state.event_viewer_buffer.lock() {
        buf.clear();
    }
}

// ============================================================================
// 이슈 #87 — About 정보 창. `plan/issue-87-about-window.md` — D1(별도 `about.html`
// + Event Viewer 빌더 패턴)·D5(메뉴 라우팅 분리)·#88 정책(닫아도 숨김 상주).
// ============================================================================

const ABOUT_WINDOW_LABEL: &str = "about";

/// ⭐ 이슈 #87 상급 리뷰 #1 — 상주 About 창의 언어 변경 스테일니스 수정.
///
/// About 창은 #88 정책(닫아도 `hide` 상주, 파괴·재생성 없음)이라, 열어 본 적이
/// 있는 채로 언어를 바꾸면 생성 시 부트스트랩된 문자열·타이틀바가 **앱 재시작까지
/// 이전 언어로 남는다**(설정 창은 언어 변경 주체라 스스로 재부트스트랩하지만 About
/// 창에는 그 경로가 없다). 카탈로그가 교체되는 두 시점 — 언어 선택(`settings_set_
/// general_language`)과 설정 import 교체(`reload_settings_after_replace`) — 에서
/// 이 함수를 호출해 About 창에 갱신 이벤트를 보낸다.
///
/// - 네이티브 타이틀바는 여기서 `set_title`(새 카탈로그의 `about.title`)로 직접
///   갱신한다(Rust 쪽이 카탈로그를 쥐고 있어 가장 저렴하다).
/// - 창 내용(정보 행 라벨·번들 여부)은 `about.html` 이 `catalog-changed` 를
///   `listen` 하여 `about_bootstrap` 을 다시 부르고 다시 그린다.
/// - 창이 아직 없으면 아무 일도 하지 않는다 — 열릴 때 부트스트랩이 새 카탈로그를
///   타므로 빈 구간이 없다.
const CATALOG_CHANGED_EVENT: &str = "catalog-changed";

fn notify_about_catalog_changed(app: &tauri::AppHandle, state: &Arc<AppState>) {
    let Some(window) = app.get_webview_window(ABOUT_WINDOW_LABEL) else {
        return;
    };
    let catalog = state.catalog.load_full();
    let _ = window.set_title(catalog.get("about.title"));
    let _ = window.emit(CATALOG_CHANGED_EVENT, ());
    tracing::info!("about window notified of catalog change");
}

/// 트레이 메뉴 `About`·설정 창의 버전 버튼(`#version-btn`)이 여는 독립 정보 창.
///
/// Event Viewer(`open_event_viewer`)와 같은 동적 생성 패턴을 그대로 따른다 —
/// `tauri.conf.json` 에 정적 선언하지 않고, 이미 열려 있으면 새로 만들지 않고
/// 최전면으로 올린다. ⭐ #88 정책 — 설정 창과 같은 "닫으면 `prevent_close()` +
/// `hide()` 로 숨겨 상주" 배선을 건다(닫았다가 다시 열 수 없는 기존 결함을
/// 재현하지 않는다).
fn show_about_window(app: &tauri::AppHandle, state: &Arc<AppState>) {
    if let Some(window) = app.get_webview_window(ABOUT_WINDOW_LABEL) {
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }

    let catalog = state.catalog.load_full();
    let title = catalog.get("about.title").to_string();
    // ⭐ 이슈 #96 — 창 크기는 "본문 전체가 스크롤바 없이 보이는 고정 크기" 정책이다.
    // 5개 언어 × 번들 내·외 2상태 전부를 헤드리스 렌더로 실측해 가장 높은 조합
    // (에스파냐어 + 번들 밖 안내 표시)에 여백을 더한 값으로 정했다 — 콘텐츠는
    // `about.html` 이 세로 중앙 정렬하므로 짧은 조합은 위아래 여백이 균등하다.
    // `resizable(false)` 유지 — 사용자가 크기를 바꾸는 창이 아니라 정보 창이다.
    match tauri::WebviewWindowBuilder::new(
        app,
        ABOUT_WINDOW_LABEL,
        tauri::WebviewUrl::App("about.html".into()),
    )
    .title(title)
    .inner_size(540.0, 480.0)
    .resizable(false)
    .center()
    .build()
    {
        Ok(window) => {
            let window_for_close = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    tracing::info!(
                        "about window close requested; hiding instead of destroying (issue #88 policy)"
                    );
                    api.prevent_close();
                    let _ = window_for_close.hide();
                }
            });
            tracing::info!("about window opened");
        }
        Err(e) => tracing::error!(error = %e, "failed to open about window"),
    }
}

/// `settings.html` General 탭의 버전 버튼(`#version-btn`) — ⭐ 이슈 #87(#9 상급
/// 리뷰 #2)으로 클릭 동작이 "About 정보 펼치기"에서 **About 창 열기**로 바뀌었다.
/// 트레이 메뉴 `ABOUT` 분기와 같은 경로(`show_about_window`)를 공유한다.
#[tauri::command]
fn open_about_window(app: tauri::AppHandle, state: State<'_, Arc<AppState>>) {
    show_about_window(&app, state.inner());
}

// ============================================================================
// F-12 라이선싱 — General 탭 UI 백엔드(이슈 #59).
// `docs/spec/licensing-and-trial.md` §4.2 의 라이선스 UI(상태 표시·키 입력·
// 활성화·"이 기기 비활성화")를 노출한다. 판정은 `license::LicenseController` 가
// 담당하고, 이 커맨드들은 상태를 직렬화해 WebView 에 전달한다.
// ============================================================================

/// General 탭 "라이선스 상태 표시" 행이 그릴 뷰(명세 §4.2 + §8).
#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct LicenseView {
    /// 상태 머신 값(체험 중/체험 만료/라이선스 활성/무효).
    state: String,
    /// `state == trial` 일 때의 남은 일수. else `None`.
    days_remaining: Option<i64>,
    /// 활성화 슬롯 현황(라이선스 활성 시에만). else `None`.
    activations_used: Option<u32>,
    activations_limit: Option<u32>,
    /// 라이선스가 활성 상태인지 — "이 기기 비활성화" 버튼 활성화 판단.
    is_licensed: bool,
}

fn license_view(res: &ultrakey_license::EvaluateResult) -> LicenseView {
    let state_str = res.state.as_log_str().to_string();
    let days = res.trial.and_then(|t| t.days_remaining);
    let (used, limit) = match &res.activations {
        Some(a) => (Some(a.used), Some(a.limit)),
        None => (None, None),
    };
    LicenseView {
        state: state_str,
        days_remaining: days,
        activations_used: used,
        activations_limit: limit,
        is_licensed: res.state == ultrakey_license::LicenseState::Licensed,
    }
}

/// 실행 시점 라이선스 상태 조회 — General 탭 부팅·재렌더가 부른다.
#[tauri::command]
fn license_state(state: State<'_, Arc<AppState>>) -> LicenseView {
    let res = state.license_controller.evaluate();
    license_view(&res)
}

/// 사용자가 키를 입력하고 "활성화"를 눌렀을 때(§3.3.1).
///
/// no-op provider 에서는 항상 `activated`(3/3 과 무관하게) 로 응답한다.
#[tauri::command]
fn license_activate(
    state: State<'_, Arc<AppState>>,
    license_key: String,
) -> Result<LicenseView, String> {
    let outcome = state.license_controller.activate_key(license_key);
    // 실패(키 오류·한도·네트워크)는 사유 문자열로, 성공은 갱신된 상태로 응답.
    match outcome {
        ultrakey_license::ActivationOutcome::Activated {
            activations_used,
            activations_limit,
        } => {
            let res = state.license_controller.evaluate();
            tracing::info!(activations_used, activations_limit, "activation ok");
            Ok(license_view(&res))
        }
        ultrakey_license::ActivationOutcome::InvalidKey => Err("invalid_key".to_string()),
        ultrakey_license::ActivationOutcome::Refunded => Err("refunded".to_string()),
        ultrakey_license::ActivationOutcome::LimitReached { .. } => Err("limit_reached".to_string()),
        ultrakey_license::ActivationOutcome::NetworkError => Err("network_error".to_string()),
    }
}

/// "이 기기 비활성화"(§3.3.2). 성공만이 슬롯 반납의 진실이다 — 실패는 낙관적으로
/// 처리하지 않고 사유를 돌려준다(§5-8).
#[tauri::command]
fn license_deactivate(state: State<'_, Arc<AppState>>) -> Result<LicenseView, String> {
    match state.license_controller.deactivate_device() {
        ultrakey_license::DeactivationOutcome::Deactivated { .. } => {
            let res = state.license_controller.evaluate();
            tracing::info!("license deactivated; evaluating state");
            Ok(license_view(&res))
        }
        ultrakey_license::DeactivationOutcome::NotFound => {
            Err("not_found".to_string())
        }
        ultrakey_license::DeactivationOutcome::NetworkError => {
            Err("network_error".to_string())
        }
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
            let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
            let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;
            let seek_snapshot = state.seek.lock().map_err(|e| e.to_string())?.clone();
            let store = state.store.lock().map_err(|e| e.to_string())?;
            return Ok(build_settings_state(
                &hyperkey_snapshot,
                &presets_before,
                &korean_snapshot,
                &japanese_snapshot,
                &chinese_snapshot,
                &seek_snapshot,
                &store,
                state.auto_update_checks_enabled(),
                None,
                Some(pending),
                caps_lock_kernel_map_missing(state),
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
    let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
    let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;
    let seek_snapshot = state.seek.lock().map_err(|e| e.to_string())?.clone();

    // 3) 엔진 반영.
    // D-D: 규칙이 바뀌는 변경은 stuck modifier 를 막기 위해 상태도 리셋한다.
    reconfigure_engine(
        state,
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        key_affects_modifier_rules(key),
    )?;

    // 4) 저장.
    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "failed to save setting");
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
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        &store,
        state.auto_update_checks_enabled(),
        save_error,
        None,
        caps_lock_kernel_map_missing(state),
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
            let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
            let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;
            let seek_snapshot = state.seek.lock().map_err(|e| e.to_string())?.clone();
            let store = state.store.lock().map_err(|e| e.to_string())?;
            return Ok(build_settings_state(
                &hyperkey_snapshot,
                &presets_before,
                &korean_snapshot,
                &japanese_snapshot,
                &chinese_snapshot,
                &seek_snapshot,
                &store,
                state.auto_update_checks_enabled(),
                None,
                Some(pending),
                caps_lock_kernel_map_missing(state),
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
    let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
    let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;
    let seek_snapshot = state.seek.lock().map_err(|e| e.to_string())?.clone();

    // 3) 엔진 반영 — presets.* 변경은 항상 규칙 테이블을 바꾼다(단순 슬라이더도
    // `quick_press_duration_ms` 를 통해 FSM 타이밍에 영향을 준다) — 언제나
    // force_reset 한다.
    reconfigure_engine(
        state,
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        true,
    )?;

    // ⭐ F-01 — `Presets` 탭의 `Quick press caps lock to execute:` 가 Seek 워커의
    // `SeekConfig::quick_press_opens`(활성화 경로 3)에 영향을 준다. `seek.*` 가
    // 바뀌지 않았어도 이 값은 presets.* 변경만으로 달라질 수 있으므로 매번 다시
    // 밀어 넣는다.
    push_seek_config(state, &seek_snapshot, &presets_snapshot);

    // 4) 저장.
    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "failed to save setting");
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
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        &store,
        state.auto_update_checks_enabled(),
        save_error,
        None,
        caps_lock_kernel_map_missing(state),
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
    let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
    let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;
    let seek_snapshot = state.seek.lock().map_err(|e| e.to_string())?.clone();

    // 1)
    let korean_snapshot = {
        let mut korean = state.korean.lock().map_err(|e| e.to_string())?;
        validate_and_apply_korean(&mut korean, key, value)?;
        *korean
    };

    // 2) 엔진 반영 — korean.* 변경은 항상 규칙 테이블을 바꾼다.
    reconfigure_engine(
        state,
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        true,
    )?;

    // 3) ⭐ 항목 5 는 게이트다 — 엔진 설정이 아니다(D-K3). ⭐ K5(D-K17) — 목록
    // 오버라이드(`korean.excludedBundleIds`)도 게이트다. 이 자리에서 들어온 값으로
    // 목록을 **통째로 다시 주입**한다 — 저장(4) 이후의 저장소 값을 읽어 주입하면
    // 저장 실패 시 화면·게이트가 어긋나므로 **들어온 값**을 기준으로 한다.
    if key == keys::KOREAN_DISABLE_IN_REMOTE_DESKTOP {
        state
            .gate_controller
            .set_korean_exclusion_enabled(korean_snapshot.disable_in_remote_desktop);
    }
    if key == keys::KOREAN_EXCLUDED_BUNDLE_IDS {
        let ids: Vec<String> = serde_json::from_value(value.clone())
            .map_err(|e| format!("설정 값 타입이 맞지 않는다({key}): {e}"))?;
        state
            .gate_controller
            .set_korean_excluded_apps(ultrakey_korean::resolve_excluded_bundle_ids(Some(&ids)));
    }

    // 4) 저장.
    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "failed to save setting");
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
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        &store,
        state.auto_update_checks_enabled(),
        save_error,
        None,
        caps_lock_kernel_map_missing(state),
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
        k if k == keys::KOREAN_MODIFIER_KEY_TYPES_LOWERCASE => {
            korean.modifier_key_types_lowercase = parse(value, key)?
        }
        // ⭐ K5(이슈 #73, D-K17) — 원격 데스크톱 제외 목록의 사용자 오버라이드.
        // 값은 번들 ID 문자열 배열이다. `KoreanSettings` 가 이 목록을 갖지 않는
        // 이유: 이 키는 게이트 주입용 **저장 데이터**일 뿐 규칙 산출에 쓰이지 않고,
        // 부재(기본 12종)와 명시적 빈 배열("아무 앱도 제외 안 함")을 구별해야
        // 하므로 `KoreanSettings` 필드가 아니라 저장소에서 직접 읽는다. 메모리
        // 정본이 필요 없는 이유도 같다 — 매번 store 에서 읽어 주입하면 되고,
        // 저장은 이 커맨드의 step 4(`store.set`)가 그대로 한다.
        k if k == keys::KOREAN_EXCLUDED_BUNDLE_IDS => {
            // 배열 검증 — 각 원소는 비지 않은 문자열이어야 한다(정규화는 저장 시점).
            let ids: Vec<String> = parse(value, key)?;
            for id in &ids {
                if ultrakey_korean::normalize_bundle_id(id).is_none() {
                    return Err(format!("빈 번들 ID 는 저장할 수 없다({key})"));
                }
            }
        }
        _ => return Err(format!("{key} cannot be changed through this command")),
    }
    Ok(())
}

fn validate_and_apply_korean(
    korean: &mut KoreanSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    if !keys::all().contains(&key) {
        return Err(format!("unknown settings key: {key}"));
    }
    apply_korean_setting(korean, key, value)
}

// ============================================================================
// F-19 `settings_set` 의 `japanese.*`/`chinese.*` 경로.
// ============================================================================

/// 현재 켜져 있는 언어 프리셋 저장 키 목록(F-16 의 `korean.hanEng…` 규칙 제외 —
/// 충돌 감지는 F-19 규칙끼리만 본다). `detect_language_conflict` 의 `active_keys`
/// 인자다 — F-19.1·F-19.2 는 `korean.*` 키에 살지만 정의는 F-19 규칙이라 포함한다
/// (명세 §3.0 — ko 노드 신규분의 ID 는 Language 계열).
fn active_language_preset_keys(state: &Arc<AppState>) -> Vec<&'static str> {
    let korean = state.korean.lock().unwrap();
    let japanese = state.japanese.lock().unwrap();
    let chinese = state.chinese.lock().unwrap();
    let mut active = Vec::new();
    if korean.caps_lock_switches_input_source {
        active.push(keys::KOREAN_CAPS_LOCK_SWITCHES_INPUT_SOURCE);
    }
    if korean.right_command_switches_input_source {
        active.push(keys::KOREAN_RIGHT_COMMAND_SWITCHES_INPUT_SOURCE);
    }
    if japanese.caps_lock_toggles_eisu_kana {
        active.push(keys::JAPANESE_CAPS_LOCK_TOGGLES_EISU_KANA);
    }
    if japanese.command_toggles_eisu_kana {
        active.push(keys::JAPANESE_COMMAND_TOGGLES_EISU_KANA);
    }
    if japanese.swap_yen_backslash {
        active.push(keys::JAPANESE_SWAP_YEN_BACKSLASH);
    }
    if japanese.jis_as_us_symbols {
        active.push(keys::JAPANESE_JIS_AS_US_SYMBOLS);
    }
    if chinese.caps_lock_switches_input_source {
        active.push(keys::CHINESE_CAPS_LOCK_SWITCHES_INPUT_SOURCE);
    }
    active
}

/// `settings_set` 의 `japanese.*` 경로(F-19, P19~P22). `settings_set_korean` 과 같은
/// 5단계 골격에, **F-19 키코드 충돌 감지**(명세 §5 #1 — 같은 물리 키를 소스로 주장하는
/// 규칙이 둘 이상이면 `pendingConflict` 를 돌려준다)가 들어간다.
fn settings_set_japanese(
    state: &Arc<AppState>,
    key: &str,
    value: &serde_json::Value,
) -> Result<SettingsState, String> {
    let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let presets_snapshot = *state.presets.lock().map_err(|e| e.to_string())?;
    let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;
    let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
    let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;
    let seek_snapshot = state.seek.lock().map_err(|e| e.to_string())?.clone();

    // 1) 충돌 감지 — 적용 전에.
    if let Some(new_value) = value.as_bool() {
        let active = active_language_preset_keys(state);
        if let Some(conflict) = ultrakey_language_presets::detect_language_conflict(
            &active,
            presets_snapshot.caps_quick_press.enabled,
            key,
            new_value,
        ) {
            let pending = language_shared_conflict_view(&conflict, key, value);
            let store = state.store.lock().map_err(|e| e.to_string())?;
            return Ok(build_settings_state(
                &hyperkey_snapshot,
                &presets_snapshot,
                &korean_snapshot,
                &japanese_snapshot,
                &chinese_snapshot,
                &seek_snapshot,
                &store,
                state.auto_update_checks_enabled(),
                None,
                Some(pending),
                caps_lock_kernel_map_missing(state),
            ));
        }
    }

    // 2) 메모리 갱신.
    let japanese_snapshot = {
        let mut japanese = state.japanese.lock().map_err(|e| e.to_string())?;
        validate_and_apply_japanese(&mut japanese, key, value)?;
        *japanese
    };

    // 3) 엔진 반영 — japanese.* 변경은 규칙 테이블을 바꾼다.
    reconfigure_engine(
        state,
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        true,
    )?;

    // 3-b) F-19 목록 오버라이드(`japanese.excludedBundleIds`)는 게이트다(D-7) —
    // 들어온 값으로 통째로 재주입한다(저장 실패 시 화면·게이트 어긋남 방지).
    if key == keys::JAPANESE_EXCLUDED_BUNDLE_IDS {
        let ids: Vec<String> = serde_json::from_value(value.clone())
            .map_err(|e| format!("설정 값 타입이 맞지 않는다({key}): {e}"))?;
        state
            .gate_controller
            .set_japanese_excluded_apps(ultrakey_korean::resolve_excluded_bundle_ids(Some(&ids)));
    }

    // 4) 저장.
    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "failed to save setting");
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
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        &store,
        state.auto_update_checks_enabled(),
        save_error,
        None,
        caps_lock_kernel_map_missing(state),
    ))
}

/// `settings_set_japanese` 의 1~2단계(키 검증 + 메모리 갱신)만 담당하는 순수 함수.
fn apply_japanese_setting(
    japanese: &mut JapaneseSettings,
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
        k if k == keys::JAPANESE_CAPS_LOCK_TOGGLES_EISU_KANA => {
            japanese.caps_lock_toggles_eisu_kana = parse(value, key)?
        }
        k if k == keys::JAPANESE_COMMAND_TOGGLES_EISU_KANA => {
            japanese.command_toggles_eisu_kana = parse(value, key)?
        }
        k if k == keys::JAPANESE_SWAP_YEN_BACKSLASH => {
            japanese.swap_yen_backslash = parse(value, key)?
        }
        k if k == keys::JAPANESE_JIS_AS_US_SYMBOLS => {
            japanese.jis_as_us_symbols = parse(value, key)?
        }
        k if k == keys::JAPANESE_EXCLUDED_BUNDLE_IDS => {
            // 게이트 주입용 저장 데이터 — `KoreanSettings::excludedBundleIds` 와
            // 같은 이유로 필드가 아니라 저장소에서 읽는다. 배열 원소 검증만.
            let ids: Vec<String> = parse(value, key)?;
            for id in &ids {
                if ultrakey_korean::normalize_bundle_id(id).is_none() {
                    return Err(format!("빈 번들 ID 는 저장할 수 없다({key})"));
                }
            }
        }
        _ => return Err(format!("{key} cannot be changed through this command")),
    }
    Ok(())
}

fn validate_and_apply_japanese(
    japanese: &mut JapaneseSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    if !keys::all().contains(&key) {
        return Err(format!("unknown settings key: {key}"));
    }
    apply_japanese_setting(japanese, key, value)
}

/// `settings_set` 의 `chinese.*` 경로(F-19, P23). `settings_set_japanese` 와 같은
/// 골격 — 충돌 감지(F-19.7 캡스락) + 5단계.
fn settings_set_chinese(
    state: &Arc<AppState>,
    key: &str,
    value: &serde_json::Value,
) -> Result<SettingsState, String> {
    let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let presets_snapshot = *state.presets.lock().map_err(|e| e.to_string())?;
    let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;
    let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
    let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;
    let seek_snapshot = state.seek.lock().map_err(|e| e.to_string())?.clone();

    // 1) 충돌 감지 — F-19.7(캡스락)이 F-19.1/19.3/19.7 과 같은 물리 키를 소스로
    // 주장할 수 있다(명세 §5 #1).
    if let Some(new_value) = value.as_bool() {
        let active = active_language_preset_keys(state);
        if let Some(conflict) = ultrakey_language_presets::detect_language_conflict(
            &active,
            presets_snapshot.caps_quick_press.enabled,
            key,
            new_value,
        ) {
            let pending = language_shared_conflict_view(&conflict, key, value);
            let store = state.store.lock().map_err(|e| e.to_string())?;
            return Ok(build_settings_state(
                &hyperkey_snapshot,
                &presets_snapshot,
                &korean_snapshot,
                &japanese_snapshot,
                &chinese_snapshot,
                &seek_snapshot,
                &store,
                state.auto_update_checks_enabled(),
                None,
                Some(pending),
                caps_lock_kernel_map_missing(state),
            ));
        }
    }

    // 2) 메모리 갱신.
    let chinese_snapshot = {
        let mut chinese = state.chinese.lock().map_err(|e| e.to_string())?;
        validate_and_apply_chinese(&mut chinese, key, value)?;
        *chinese
    };

    // 3) 엔진 반영.
    reconfigure_engine(
        state,
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        true,
    )?;

    // 3-b) F-19.7 목록 오버라이드(`chinese.excludedBundleIds`) 게이트 재주입(D-7).
    if key == keys::CHINESE_EXCLUDED_BUNDLE_IDS {
        let ids: Vec<String> = serde_json::from_value(value.clone())
            .map_err(|e| format!("설정 값 타입이 맞지 않는다({key}): {e}"))?;
        state
            .gate_controller
            .set_chinese_excluded_apps(ultrakey_korean::resolve_excluded_bundle_ids(Some(&ids)));
    }

    // 4) 저장.
    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "failed to save setting");
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
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        &store,
        state.auto_update_checks_enabled(),
        save_error,
        None,
        caps_lock_kernel_map_missing(state),
    ))
}

/// `settings_set_chinese` 의 1~2단계(키 검증 + 메모리 갱신)만 담당하는 순수 함수.
fn apply_chinese_setting(
    chinese: &mut ChineseSettings,
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
        k if k == keys::CHINESE_CAPS_LOCK_SWITCHES_INPUT_SOURCE => {
            chinese.caps_lock_switches_input_source = parse(value, key)?
        }
        k if k == keys::CHINESE_EXCLUDED_BUNDLE_IDS => {
            let ids: Vec<String> = parse(value, key)?;
            for id in &ids {
                if ultrakey_korean::normalize_bundle_id(id).is_none() {
                    return Err(format!("빈 번들 ID 는 저장할 수 없다({key})"));
                }
            }
        }
        _ => return Err(format!("{key} cannot be changed through this command")),
    }
    Ok(())
}

fn validate_and_apply_chinese(
    chinese: &mut ChineseSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    if !keys::all().contains(&key) {
        return Err(format!("unknown settings key: {key}"));
    }
    apply_chinese_setting(chinese, key, value)
}

// ============================================================================
// F-01 `settings_set` 의 `seek.*` 경로.
// ============================================================================

/// `seek.remapKey` 값 파싱 — `"-"` 는 미설정(`None`), 그 밖은 `SourceKey` variant
/// 이름이어야 한다.
fn parse_seek_remap_key(value: &serde_json::Value) -> Result<Option<SourceKey>, String> {
    let raw = value
        .as_str()
        .ok_or_else(|| "seek.remapKey 는 문자열이어야 한다".to_string())?;
    if raw == "-" {
        return Ok(None);
    }
    serde_json::from_value(serde_json::Value::String(raw.to_string()))
        .map(Some)
        .map_err(|e| format!("알 수 없는 seek.remapKey 값 {raw}: {e}"))
}

/// `settings_set_seek` 의 1단계(키 검증 + 메모리 갱신)만 담당하는 순수 함수 —
/// `validate_and_apply`/`validate_and_apply_preset`/`validate_and_apply_korean` 과
/// 같은 형식.
///
/// ⭐ **판단** — `seek.toggleShortcut.code`/`.modifiers` 는 저장 키가 둘로
/// 나뉘어 있지만(명세 §4, `keys::SEEK_TOGGLE_SHORTCUT_CODE`/`_MODIFIERS`) 논리적으로는
/// 한 값(`SeekShortcut`)이다 — 한쪽만 갱신되고 와도 다른 쪽의 기존 값을 보존한다.
/// `null` 은 명시적 지우기(단축키 제거, 버튼 `Record Shortcut` 상태로 되돌림)로
/// 다룬다. 이 두 필드를 프런트가 어떤 순서·조합으로 보내는지는 `settings.html`
/// 계약(다른 위임 담당)에 달려 있다 — 이 함수는 "부분 갱신이 안전하다"만 보장한다.
fn apply_seek_setting(
    seek: &mut SeekSettings,
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
        k if k == keys::SEEK_TOGGLE_SHORTCUT_CODE => {
            if value.is_null() {
                seek.toggle_shortcut = None;
            } else {
                let code: String = parse(value, key)?;
                let modifiers = seek
                    .toggle_shortcut
                    .as_ref()
                    .map_or(EventFlags::NONE, |s| s.modifiers);
                seek.toggle_shortcut = Some(SeekShortcut { code, modifiers });
            }
        }
        k if k == keys::SEEK_TOGGLE_SHORTCUT_MODIFIERS => {
            if value.is_null() {
                seek.toggle_shortcut = None;
            } else {
                let bits: u64 = parse(value, key)?;
                let code = seek
                    .toggle_shortcut
                    .as_ref()
                    .map_or_else(String::new, |s| s.code.clone());
                seek.toggle_shortcut = Some(SeekShortcut {
                    code,
                    modifiers: EventFlags(bits),
                });
            }
        }
        k if k == keys::SEEK_REMAP_KEY => seek.remap_key = parse_seek_remap_key(value)?,
        k if k == keys::SEEK_EXECUTE_ON_CLOSE => seek.execute_on_close = parse(value, key)?,
        k if k == keys::SEEK_SEMICOLON_CYCLE => seek.semicolon_cycles = parse(value, key)?,
        // ⭐ F-04(이슈 #44) — 체크박스 2종. ⚠️ 이 매치를 빼먹으면 폴스루로
        // "{key} 는 이 커맨드로 바꿀 수 없다" 조용한 거부가 된다(A4).
        k if k == keys::SEEK_FOCUS_WINDOW_BEFORE_CLICKING => {
            seek.focus_window_before_clicking = parse(value, key)?
        }
        k if k == keys::SEEK_CHANGE_CLICK_MODES_WITH_MODIFIERS => {
            seek.change_click_modes_with_modifiers = parse(value, key)?
        }
        // ⭐(이슈 #133, 소스 C) — 창 제목 검색 체크박스. ⚠️ 이 매치를 빼먹으면
        // 폴스루로 조용한 거부가 된다(F-04 예시 암 스타일 그대로).
        k if k == keys::SEEK_INCLUDE_WINDOW_TITLES => {
            seek.include_window_titles = parse(value, key)?
        }
        // ⭐(이슈 #93) `검색 언어` — `null`/빈 문자열은 부재(= 영어 기본)로 되돌림
        // (이슈 #131 — 이슈 #48 의 로케일 폴백은 폐기).
        // 허용 값은 `"ko"`·`"zh"`·`"ja"`·`"es"`·`"en"` 뿐 — 그 외 값은 거부하고
        // `input_box_mode` 를 좌우하는 값이므로 조용히 넘어가지 않게 한다.
        k if k == keys::SEEK_SEARCH_LANGUAGE => {
            if value.is_null() {
                seek.search_language = None;
            } else {
                let lang: String = parse(value, key)?;
                if !matches!(lang.as_str(), "" | "ko" | "zh" | "ja" | "es" | "en") {
                    return Err(format!(
                        "알 수 없는 검색 언어 값: {lang:?} — \"ko\"·\"zh\"·\"ja\"·\"es\"·\"en\" 만 허용한다"
                    ));
                }
                // 빈 문자열은 부재와 같은 취급(영어 기본).
                seek.search_language = if lang.is_empty() { None } else { Some(lang) };
            }
        }
        _ => return Err(format!("{key} 는 이 커맨드로 바꿀 수 없다")),
    }
    Ok(())
}

fn validate_and_apply_seek(
    seek: &mut SeekSettings,
    key: &str,
    value: &serde_json::Value,
) -> Result<(), String> {
    if !keys::all().contains(&key) {
        return Err(format!("알 수 없는 설정 키: {key}"));
    }
    apply_seek_setting(seek, key, value)
}

/// Seek 워커에 현재 설정을 다시 알린다 — 워커가 아직 없으면(엔진 시작 전)
/// 조용히 버린다(`send_seek_signal` 과 같은 규약).
///
/// ⭐ F-04(이슈 #44, A6) — F-04 클릭 설정(`ClickSettings`)도 같은 저장
/// 갱신(`settings_set_seek`)에서 태어나므로 `SeekConfig` 와 **한 신호**에 함께
/// 실어 보낸다(단일 소스 → 단일 신호 원칙 — 별도 신호로 갈라면 "config 는
/// 갱신됐는데 click 설정은 안 갱신된" 중간 상태가 신호 스트림에 생긴다).
fn push_seek_config(state: &Arc<AppState>, seek: &SeekSettings, presets: &PresetSettings) {
    let config = seek.to_config(quick_press_opens_seek(presets));
    let click_settings = click_settings_from_seek(seek);
    send_seek_signal(
        state,
        seek::SeekSignal::ConfigChanged(config, click_settings),
    );
}

/// `SeekSettings` → `ClickSettings` — 저장 표현에서 F-04 가 쓰는 런타임 경계
/// 타입으로 옮긴다(기본값은 전부 `SeekSettings` 쪽이 이미 반영했다).
fn click_settings_from_seek(seek: &SeekSettings) -> ClickSettings {
    ClickSettings {
        focus_window_before_clicking: seek.focus_window_before_clicking,
        change_click_modes_with_modifiers: seek.change_click_modes_with_modifiers,
    }
}

/// `Toggle Seek with shortcut:` 등록을 새 설정과 맞춘다 — **메인 스레드에서**
/// 불러야 하므로 `run_on_main_thread` 로 디스패치한다(`seek::apply_global_shortcut`
/// 문서 참고). 큐잉만 하고 즉시 반환하므로 이 함수를 호출한 커맨드는 등록이 실제로
/// 끝나기 전에 반환할 수 있다 — 그래도 안전하다: 등록 실패는 `tracing::warn!` 으로
/// 관측 가능하고, `SettingsState.seek.toggleShortcut` 은 이미 저장된 값을 그대로
/// 반영해 UI 가 어긋나지 않는다.
fn reapply_global_shortcut(state: &Arc<AppState>, app: &tauri::AppHandle, seek: &SeekSettings) {
    let state = state.clone();
    let shortcut = seek.toggle_shortcut.clone();
    let dispatched = app.run_on_main_thread(move || {
        let manager_guard = state.global_hotkey_manager.lock().unwrap();
        let Some(manager) = manager_guard.as_ref() else {
            tracing::warn!("no GlobalHotKeyManager; cannot register the Seek global shortcut");
            return;
        };
        let mut registered = state.global_hotkey_registered.lock().unwrap();
        seek::apply_global_shortcut(manager, &mut registered, shortcut.as_ref());
    });
    if let Err(e) = dispatched {
        tracing::error!(error = %e, "failed to dispatch Seek global shortcut re-registration to the main thread");
    }
}

/// `settings_set` 의 `seek.*` 경로(F-01). 순서: 1) `key` 검증 + 메모리 갱신
/// 2) **엔진 반영**(`Remap key to Seek:` 가 바뀌면 F-07 규칙 테이블의 트리거 키
/// 자체가 바뀐다 — 항상 force_reset) 3) **전역 핫키 재등록**(메인 스레드로
/// 디스패치) 4) Seek 워커에 `ConfigChanged` 통지 5) 저장 6) 새 `SettingsState`.
fn settings_set_seek(
    state: &Arc<AppState>,
    app: &tauri::AppHandle,
    key: &str,
    value: &serde_json::Value,
) -> Result<SettingsState, String> {
    let hyperkey_snapshot = state.hyperkey.lock().map_err(|e| e.to_string())?.clone();
    let presets_snapshot = *state.presets.lock().map_err(|e| e.to_string())?;
    let korean_snapshot = *state.korean.lock().map_err(|e| e.to_string())?;
    let japanese_snapshot = *state.japanese.lock().map_err(|e| e.to_string())?;
    let chinese_snapshot = *state.chinese.lock().map_err(|e| e.to_string())?;

    // 1)
    let seek_snapshot = {
        let mut seek = state.seek.lock().map_err(|e| e.to_string())?;
        validate_and_apply_seek(&mut seek, key, value)?;
        seek.clone()
    };

    // 2) 엔진 반영 — remap_key(트리거 키)·caps lock alias 모두 바뀔 수 있다.
    reconfigure_engine(
        state,
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        true,
    )?;

    // 3)
    reapply_global_shortcut(state, app, &seek_snapshot);

    // 4)
    push_seek_config(state, &seek_snapshot, &presets_snapshot);

    // 5) 저장.
    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(key, value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "failed to save settings");
                Some(e.to_string())
            }
        }
    };

    // 6)
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(
        &hyperkey_snapshot,
        &presets_snapshot,
        &korean_snapshot,
        &japanese_snapshot,
        &chinese_snapshot,
        &seek_snapshot,
        &store,
        state.auto_update_checks_enabled(),
        save_error,
        None,
        caps_lock_kernel_map_missing(state),
    ))
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
        .ok_or_else(|| format!("{key} conflict resolution only supports a boolean value"))?;

    let to_disable: Vec<String> = {
        let presets_before = *state.presets.lock().map_err(|e| e.to_string())?;
        if key.starts_with("presets.") {
            ultrakey_presets::detect_conflict(&presets_before, &caps_slots, &key, new_value)
                .map(|c| c.to_disable.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default()
        } else if key.starts_with("korean.")
            || key.starts_with("japanese.")
            || key.starts_with("chinese.")
        {
            // ⭐ F-19 — 언어 규칙 충돌(같은 물리 키를 소스로 주장하는 규칙 ≥2, 명세 §5 #1).
            // F-19.1/19.3/19.7 ↔ F-08.2 캡스락 quick press 크로스 패밀리도 여기서
            // 걸러진다(presets 스냅샷의 `caps_quick_press.enabled` 를 넘긴다).
            let active = active_language_preset_keys(&state);
            ultrakey_language_presets::detect_language_conflict(
                &active,
                presets_before.caps_quick_press.enabled,
                &key,
                new_value,
            )
            .map(|c| c.to_disable.iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
        } else if new_value {
            // ⭐ 반대 방향 — hyper/meh/bleh 슬롯을 caps lock 소스로 켜려는데
            // `Remap caps lock to:` 가 이미 켜져 있는 경우(대칭 처리).
            ultrakey_presets::detect_modifier_slot_conflict(&presets_before)
                .map(|c| c.to_disable.iter().map(|s| s.to_string()).collect())
                .unwrap_or_default()
        } else {
            Vec::new()
        }
    };

    // ⭐ 배타 대상은 `presets.*` 일 수도 `hyperkey.*` 일 수도 있다 — 각각 자기 경로로
    // 끈다(엔진 반영·write-through 는 양쪽 경로가 이미 책임진다). ⭐ F-19 — 언어
    // 규칙이 배타 대상이면 `korean.*`/`japanese.*`/`chinese.*` 자기 경로로 끈다.
    for disable_key in &to_disable {
        let off = serde_json::Value::Bool(false);
        if disable_key.starts_with("presets.") {
            settings_set_preset(&state, disable_key, &off)?;
        } else if disable_key.starts_with("korean.") {
            settings_set_korean(&state, disable_key, &off)?;
        } else if disable_key.starts_with("japanese.") {
            settings_set_japanese(&state, disable_key, &off)?;
        } else if disable_key.starts_with("chinese.") {
            settings_set_chinese(&state, disable_key, &off)?;
        } else {
            settings_set_hyperkey(&state, disable_key, &off)?;
        }
    }

    if key.starts_with("presets.") {
        settings_set_preset(&state, &key, &value)
    } else if key.starts_with("korean.") {
        settings_set_korean(&state, &key, &value)
    } else if key.starts_with("japanese.") {
        settings_set_japanese(&state, &key, &value)
    } else if key.starts_with("chinese.") {
        settings_set_chinese(&state, &key, &value)
    } else {
        settings_set_hyperkey(&state, &key, &value)
    }
}

/// 탭 전환 — 탭 화이트리스트 검증 + (`persist` 일 때만) `ui.lastTab` 저장.
///
/// ⚠️ 이슈 #32 Phase 1 부터 이 커맨드는 **창을 리사이즈하지 않는다** — 원본
/// SuperKey 는 탭마다 창 크기를 바꾸지만(§3.1·§3.3), 클론은 단일 크기로
/// 고정하고 사용자가 조절한 크기를 영속화하기로 결정했다(`SETTINGS_WINDOW_DEFAULT`,
/// `wire_window_size_persistence` 참고). 그래도 프런트가 모르는 탭 이름이
/// 들어오면 여전히 거부한다 — `is_known_tab` 화이트리스트의 존재 이유(이슈 #28
/// 회귀 방지)는 리사이즈 여부와 무관하다.
///
/// ⭐ **`persist` 인자가 왜 필요한가 — F-15 §8 수용 기준을 지키기 위해서다.**
/// 프런트엔드는 창을 처음 그릴 때도 이 커맨드를 불러 탭 상태를 맞춰야 한다.
/// 그런데 그때도 `ui.lastTab` 을 쓰면 **환경설정 창을 열기만 해도 `settings.json`
/// 이 생긴다** — "설정을 한 번도 건드리지 않으면 저장 파일이 아예 생기지
/// 않는다"(F-15 §8, §3.1)가 첫 실행에서 바로 깨진다. 그래서 최초 렌더는
/// `persist: false`, 사용자가 실제로 탭을 누르거나 방향키로 옮긴 경우에만
/// `persist: true` 로 부른다.
///
/// 기각한 대안: "저장된 값과 다를 때만 쓴다" — 저장된 값이 **없을 때**(첫 실행)
/// 기본 탭을 쓰게 되므로 같은 문제가 그대로 남는다. 구분해야 하는 것은 값의 차이가
/// 아니라 **사용자의 의도적 조작인가**이고, 그것은 호출부만 알 수 있다.
#[tauri::command]
fn settings_set_tab(
    state: State<'_, Arc<AppState>>,
    tab: String,
    persist: bool,
) -> Result<(), String> {
    if !is_known_tab(&tab) {
        return Err(format!("unknown tab: {tab}"));
    }

    if persist {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        if let Err(e) = store.set(keys::UI_LAST_TAB, &tab) {
            tracing::error!(error = %e, "failed to save last tab");
        }
    }

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

    let dir = log_dir()?;
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

// ============================================================================
// ⭐ F-02 검출 프로브 — 실기기 검증 전용 (이슈 #30)
// ============================================================================
//
// `ULTRAKEY_SEEK_DETECT_DUMP=1` 로 실행하면 기동 직후 F-02 검출 파이프라인을
// **한 번** 돌려 결과를 로그와 덤프 파일에 남긴다.
//
// ⭐ **왜 이런 것이 필요한가**: Screen Recording 권한은 TCC 가 **부모 프로세스**
// 기준으로 판정한다(`platform-constraints.md` §3.2). 터미널에서 `cargo run` 한
// 바이너리는 *터미널의* 권한으로 캡처하므로, 앱이 실제로 권한을 받았는지를
// 그것으로는 절대 확인할 수 없다. 서명된 `.app` 을 `open` 으로 띄운 프로세스
// 안에서 돌려야만 유효한 증거가 된다(`manual-verification.md` 항목 7).
//
// ⛔ **이것은 F-01(세션 활성화)이 아니다.** 단축키도 오버레이도 없다. F-02 의
// 범위는 "후보 목록을 만드는 것" 까지이고, 이 프로브는 그 산출물을 눈으로 볼
// 수단일 뿐이다. F-01 이 들어오면 이 프로브는 그대로 두거나 지워도 된다.
/// ⭐ F-10 로그인 항목 진단 프로브(이슈 #39). `ULTRAKEY_LOGIN_ITEM_PROBE=1` 로만
/// 돈다. 등록·해제 왕복을 돌면서 매 단계의 **OS 정본 상태**(`SMAppService.status`)를
/// 남긴다 — "등록됐다고 보고했는데 로그인 시 뜨지 않는다"가 어느 상태에서 벌어지는지
/// 가설이 아니라 실측으로 확정하기 위한 것이다.
///
/// ⚠️ 이 프로브는 로그인 항목을 **실제로 등록했다가 해제한다.** 끝나면 시작 시점의
/// 상태로 되돌린다(원래 켜져 있었으면 다시 켜 둔다).
fn run_login_item_probe() {
    use ultrakey_platform::login_item;

    // `ULTRAKEY_LOGIN_ITEM_PROBE` 의 값으로 무엇을 할지 고른다.
    //   `status`     — 지금 상태만 읽고 끝낸다(아무것도 바꾸지 않는다)
    //   `register`   — 등록하고 그대로 둔다
    //   `unregister` — 해제하고 그대로 둔다
    //   그 외(`1` 등) — 등록·해제 왕복 후 시작 상태로 되돌린다
    let mode = std::env::var("ULTRAKEY_LOGIN_ITEM_PROBE").unwrap_or_default();

    let exe = std::env::current_exe()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());
    let bundle_id = ultrakey_platform::bundle::bundle_identifier().unwrap_or_default();

    let before = login_item::status();
    tracing::info!(
        step = "initial",
        status = before.as_log_str(),
        active = before.is_active(),
        exe = %exe,
        bundle_id = %bundle_id,
        "login item probe"
    );

    if mode == "status" {
        return;
    }

    if mode == "unregister" {
        match login_item::set_enabled(false) {
            Ok(()) => tracing::info!(step = "unregister-only", result = "ok", "login item probe"),
            Err(e) => tracing::error!(step = "unregister-only", result = "error", error = %e, "login item probe"),
        }
        tracing::info!(
            step = "after-unregister-only",
            status = login_item::status().as_log_str(),
            "login item probe"
        );
        return;
    }

    match login_item::set_enabled(true) {
        Ok(()) => tracing::info!(step = "register", result = "ok", "login item probe"),
        Err(e) => tracing::error!(step = "register", result = "error", error = %e, "login item probe"),
    }
    let after_register = login_item::status();
    tracing::info!(
        step = "after-register",
        status = after_register.as_log_str(),
        active = after_register.is_active(),
        "login item probe"
    );

    if mode == "register" {
        return;
    }

    match login_item::set_enabled(false) {
        Ok(()) => tracing::info!(step = "unregister", result = "ok", "login item probe"),
        Err(e) => tracing::error!(step = "unregister", result = "error", error = %e, "login item probe"),
    }
    let after_unregister = login_item::status();
    tracing::info!(
        step = "after-unregister",
        status = after_unregister.as_log_str(),
        active = after_unregister.is_active(),
        "login item probe"
    );

    // 시작 상태로 되돌린다 — 진단이 사용자의 설정을 바꿔 놓고 끝나면 안 된다.
    if before.is_active() {
        let _ = login_item::set_enabled(true);
    }
    tracing::info!(
        step = "restored",
        status = login_item::status().as_log_str(),
        "login item probe"
    );
}

fn run_seek_detect_probe() {
    use ultrakey_seek::detect::{detect_candidates, DetectionParams};

    let use_ax = std::env::var_os("ULTRAKEY_SEEK_DETECT_AX").is_some();
    let langs: Vec<String> = std::env::var("ULTRAKEY_SEEK_DETECT_LANGS")
        .ok()
        .filter(|v| !v.is_empty())
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_default();

    let mut params = DetectionParams {
        use_accessibility: use_ax,
        ..DetectionParams::default()
    };
    params.recognition.languages = langs.clone();

    // `ULTRAKEY_SEEK_DETECT_REQUEST=1` 이면 권한이 없을 때 시스템 프롬프트를
    // 한 번 띄운다(F-11 §1.3 의 요청 경로). ⚠️ 프로세스당 한 번만 뜬다.
    if std::env::var_os("ULTRAKEY_SEEK_DETECT_REQUEST").is_some()
        && !ultrakey_platform::screen_recording::has_screen_recording_access()
    {
        let granted = ultrakey_platform::screen_recording::request_screen_recording_access();
        tracing::info!(granted, "CGRequestScreenCaptureAccess called; system prompt path");
    }

    tracing::info!(
        use_accessibility = use_ax,
        ?langs,
        screen_recording = ?ultrakey_platform::screen_recording::status(),
        accessibility = ultrakey_platform::accessibility::is_process_trusted(),
        "F-02 detection probe started"
    );

    let outcome = detect_candidates(&params, |per_display| {
        // ⭐ S-1 의 증분 전달이 실제로 동작하는지 보이는 자리 — 디스플레이별
        // 후보가 전체 완료를 기다리지 않고 하나씩 도착한다.
        tracing::info!(
            display_id = per_display.display_id,
            candidates = per_display.candidates.len(),
            elapsed_ms = per_display.elapsed_ms,
            "F-02 display OCR complete (incremental delivery)"
        );
    });

    tracing::info!(
        total = outcome.candidates.len(),
        ocr = outcome.ocr_count,
        ax = outcome.ax_count,
        merged_away = outcome.ocr_count + outcome.ax_count - outcome.candidates.len(),
        capture_ms = outcome.capture_ms,
        total_ms = outcome.total_ms,
        screen_recording = ?outcome.screen_recording,
        ax_error = ?outcome.ax_error,
        "F-02 detection probe complete"
    );

    if outcome.ocr_blocked_by_permission() {
        tracing::error!(
            ocr_count = outcome.ocr_count,
            "no screen recording permission; source A has failed silently. capture \
             succeeded but it only contains the desktop background and menu bar, so \
             no matter how many candidates came back, none reflect actual screen \
             content. grant screen recording permission and restart the app (F-11 §1.3)"
        );
    }

    // 좌표를 눈으로 대조할 수 있게 상위 40개를 로그에 남긴다.
    for c in outcome.candidates.iter().take(40) {
        tracing::info!(
            text = %c.text,
            x = c.frame.x,
            y = c.frame.y,
            w = c.frame.width,
            h = c.frame.height,
            source = ?c.source,
            display = ?c.display_id,
            confidence = ?c.confidence,
            "F-02 candidate"
        );
    }

    // 전량을 파일로도 남긴다 — 좌표 검증에 쓴다.
    if let Some(home) = std::env::var_os("HOME") {
        let path =
            std::path::PathBuf::from(home).join("Library/Logs/Ultrakey/seek-candidates.json");
        match serde_json::to_string_pretty(&outcome.candidates) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    tracing::warn!(?path, %e, "failed to write F-02 candidate dump");
                } else {
                    tracing::info!(?path, "F-02 candidate dump complete");
                }
            }
            Err(e) => tracing::warn!(%e, "failed to serialize F-02 candidates"),
        }
    }
}

fn main() {
    // 1) 로그 — 기본은 `info`, `ULTRAKEY_LOG=debug` 로 켠다. stderr 는 항상 나가고,
    //    `open Ultrakey.app` 처럼 stderr 가 사라지는 실행 경로를 위해
    //    `~/Library/Logs/Ultrakey/ultrakey.log` 에도 같이 남긴다.
    init_logging();
    tracing::info!(
        pid = std::process::id(),
        exe = ?std::env::current_exe(),
        "=== Ultrakey starting ==="
    );

    // 1-a-2) ⭐ F-10 로그인 항목 프로브 (이슈 #39) — 실기기 진단 전용. 환경변수가
    // 없으면 아무 일도 하지 않는다. `register → status → unregister → status` 를
    // 돌면서 각 단계의 OS 정본 상태를 로그로 남긴다. 이것이 "Launch on login 이
    // 동작하지 않는다"의 원인을 가설이 아니라 실측으로 확정하는 도구다.
    //
    // ⭐ 단일 인스턴스 판정 **앞**에 둔다. F-03 스파이크·데모와 같은 근거다 — 이
    // 프로브는 `CGEventTap` 도 트레이도 만들지 않고 바로 반환하므로 §5 항목 1 이
    // 막으려는 상황(탭 두 개)이 발생하지 않는다. 뒤에 두면 이미 떠 있는 인스턴스
    // (병렬 위임의 검증용 빌드 등)를 **죽여야만** 진단할 수 있게 된다.
    if std::env::var_os("ULTRAKEY_LOGIN_ITEM_PROBE").is_some() {
        run_login_item_probe();
        return;
    }

    // 1-b) ⭐ 단일 인스턴스 보장(`menu-bar-and-lifecycle.md` §2 시나리오 D, §5
    // 항목 1, §8) — Tauri 를 아예 띄우기 전에 판정한다. 그래야 두 번째 프로세스가
    // 트레이 아이콘·엔진·`CGEventTap` 을 단 한 순간도 만들지 않는다. 종료는
    // 정리할 자원이 아무것도 없는 시점이라 `Engine::shutdown()` 같은 절차 없이
    // 바로 반환해도 안전하다.
    // ⭐ F-03 의 두 하네스(P3 스파이크·오버레이 데모, 이슈 #34)는 이 판정을
    // 건너뛴다. 둘 다 `setup()` 안에서 권한 감시·엔진 기동보다 **먼저
    // 반환**하므로 `CGEventTap` 을 단 한 순간도 만들지 않는다 — §5 항목 1 이
    // 막으려는 것(탭 두 개)이 애초에 발생하지 않는다. 이 예외가 없으면 이미
    // 떠 있는 인스턴스(병렬 위임의 검증용 빌드 등)를 죽이지 않고서는 오버레이를
    // 띄워 볼 수도, 렌더링 지연을 잴 수도 없다.
    if !overlay_spike::enabled() && !overlay_demo::enabled() && bundle::other_instance_running() {
        tracing::warn!(
            "another instance with the same bundle id is already running; asking it to open \
             the settings window and exiting without installing a CGEventTap (F-10 §5 item 1, \
             issue #68)"
        );
        // ⭐ 이슈 #68 — 조용히 죽는 대신 기존 인스턴스에게 설정창을 띄우라고
        // 신호를 보낸다. 이 지점은 Tauri·트레이·엔진보다 **앞**이므로 이
        // 프로세스가 `CGEventTap` 을 만들 일은 없다(§5 항목 1 유지). 신호는
        // 발행-소멸형이라 기존 인스턴스가 이미 죽었으면 아무도 받지 않는다 —
        // 그 상태에서 이 프로세스가 그냥 끝나는 것이 정확한 동작이다.
        single_instance::notify_existing_instance_to_show_settings();
        return;
    }

    // 1-c) ⭐ F-02 검출 프로브 (이슈 #30) — 실기기 검증 전용. 환경변수가 없으면
    // 아무 일도 하지 않는다. 단일 인스턴스 판정 뒤에 두어, 이미 떠 있는 인스턴스가
    // 있을 때 프로브만 돌고 끝나는 혼동을 만들지 않는다.
    if std::env::var_os("ULTRAKEY_SEEK_DETECT_DUMP").is_some() {
        run_seek_detect_probe();
    }

    // 2) 로케일 → 카탈로그 (D4: ko + en)
    let catalog = Catalog::resolve(&bundle::preferred_languages());
    tracing::info!(locale = catalog.locale().code(), "string catalog loaded");

    // ⭐ F-11 §8: `tauri dev` 산출물은 권한 검증에 쓸 수 없다는 경고.
    if dev_build_warning(&catalog).is_some() {
        tracing::warn!(
            copy_key = "dev.not_app_bundle.title",
            "running outside an .app bundle; permission-dependent features cannot be verified"
        );
    }

    let gate = Arc::new(AtomicAppGate::new());
    let gate_controller = Arc::new(AppGateController::new(gate.clone()));

    let state = Arc::new(AppState {
        catalog: ArcSwap::from_pointee(catalog),
        engine: Mutex::new(None),
        engine_shared: ArcSwapOption::empty(),
        gate: gate.clone(),
        gate_controller,
        monitor: Mutex::new(None),
        // setup() 이 실제 app_data_dir() 경로로 교체하기 전까지의 자리표시자.
        store: Mutex::new(SettingsStore::in_memory()),
        hyperkey: Mutex::new(HyperkeySettings::default()),
        presets: Mutex::new(PresetSettings::default()),
        korean: Mutex::new(KoreanSettings::default()),
        japanese: Mutex::new(ultrakey_language_presets::JapaneseSettings::default()),
        chinese: Mutex::new(ultrakey_language_presets::ChineseSettings::default()),
        seek: Mutex::new(SeekSettings::default()),
        seek_tx: Mutex::new(None),
        trackpad: Mutex::new(None),
        global_hotkey_manager: Mutex::new(None),
        global_hotkey_registered: Mutex::new(None),
        load_notice: Mutex::new(None),
        tray: Mutex::new(None),
        tray_caps_missing: std::sync::atomic::AtomicBool::new(false),
        ignore_item: Mutex::new(None),
        normal_menu: Mutex::new(None),
        unauthorized_menu: Mutex::new(None),
        system_event_observer: Mutex::new(None),
        show_settings_observer: Mutex::new(None),
        last_applied_window_size: Mutex::new(None),
        window_size_debouncer: Mutex::new(None),
        event_viewer_buffer: Mutex::new(VecDeque::new()),
        auto_update_checks: ArcSwap::new(Arc::new(false)),
        // F-12 — 부팅 시 저장소 어댑터(빌드 분기: 파일 또는 Keychain, 이슈 #99)
        // + no-op provider 로 상태 머신을 조립한다.
        license_controller: license::LicenseController::boot(),
    });

    let app_builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // F-15 §3.4 설정 export/import(이슈 #39 Phase 2) — 네이티브 파일 대화상자.
        // `settings_export`/`settings_import` 커맨드가 Rust 쪽에서만 이 플러그인의
        // Rust API 를 쓴다(위 Cargo.toml 주석 참고) — 웹뷰가 직접 invoke 하지 않으므로
        // capability 파일이 필요 없다.
        .plugin(tauri_plugin_dialog::init());

    #[cfg(target_os = "macos")]
    let app_builder = app_builder
        // ⭐ F-13 — Sparkle 업데이터. `init()` 은 인자가 없고 설정은 전부
        // Info.plist 에서 읽는다(책임 배분: docs/spec/auto-update.md §7).
        .plugin(tauri_plugin_sparkle_updater::init());

    app_builder
        .manage(state.clone())
        // ⭐ F-03 P3 스파이크가 웹뷰에서 ack 을 받는 통로(이슈 #34). 스파이크가
        // 꺼져 있으면 아무도 쓰지 않는 빈 상자로 남는다.
        .manage(std::sync::Arc::new(overlay_spike::SpikeChannel::default()))
        // ⭐ F-03 오버레이 웹뷰 창들의 공유 상태(준비 여부·마지막 프레임).
        .manage(std::sync::Arc::new(Mutex::new(
            overlay::SurfaceState::default(),
        )))
        .invoke_handler(tauri::generate_handler![
            modal_copy,
            open_settings,
            quit_app,
            settings_bootstrap,
            settings_set,
            settings_unset,
            settings_copy_common_to_device,
            settings_set_tab,
            settings_resolve_conflict,
            general_set_launch_on_login,
            general_set_hide_menu_bar_icon,
            open_keyboard_settings,
            keyboard_fn_state,
            settings_export,
            settings_import,
            open_log_folder,
            open_event_viewer,
            eventviewer_poll,
            eventviewer_clear,
            // ── 이슈 #87 — 독립 About 정보 창 ─────────────────────────────
            about_bootstrap,
            open_about_window,
            check_for_updates,
            open_external_url,
            // ── F-12 라이선싱(이슈 #59) — General 탭 UI 백엔드 ──────────────
            license_state,
            license_activate,
            license_deactivate,
            overlay_spike::overlay_spike_ack,
            overlay_spike::overlay_spike_ready,
            overlay::overlay_surface_ready,
            overlay::overlay_select_match,
            seek_set_query,
        ])
        .setup(move |app| {
            // 3) ⭐ Accessory 앱 — Dock 아이콘 없음, ⌘Tab 에 안 나타남.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            // 3-b) ⭐ F-03 P3 실측(이슈 #34) — 환경변수가 있을 때만. 스파이크는
            // 제품 세션이 아니라 숫자를 얻는 것이 목적이라, 측정이 끝나면
            // 스스로 프로세스를 끝낸다. 권한 감시·엔진 기동보다 **앞**에 두어
            // 측정 중에 엔진이 끼어들지 않게 한다.
            if overlay_spike::enabled() {
                tracing::warn!("ULTRAKEY_OVERLAY_SPIKE set; starting in F-03 P3 render-latency measurement mode");
                overlay_spike::start(app.handle());
                return Ok(());
            }

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
                        "failed to get app data directory; downgrading settings to memory-only"
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
                tracing::warn!(?warning, "Hyperkey settings warning (at boot)");
            }
            *state.hyperkey.lock().unwrap() = hyperkey_settings;
            // ⭐ F-08 Presets — hyperkey 와 같은 "부재 = 기본값" 조립 규약.
            *state.presets.lock().unwrap() = PresetSettings::from_store(&settings_store);
            // ⭐ F-16 Korean — 같은 규약. `disable_in_remote_desktop` 만 부재 시 `true`
            // 로 읽힌다(D-K9 각주, `KoreanSettings::from_store` 가 처리한다).
            let korean_settings = KoreanSettings::from_store(&settings_store);
            *state.korean.lock().unwrap() = korean_settings;
            // ⭐ F-19 — ja/zh 설정도 같은 "부재 = 기본값" 조립 규약.
            let japanese_settings = JapaneseSettings::from_store(&settings_store);
            *state.japanese.lock().unwrap() = japanese_settings;
            let chinese_settings = ChineseSettings::from_store(&settings_store);
            *state.chinese.lock().unwrap() = chinese_settings;
            // ⭐ F-01 Seek — 같은 "부재 = 기본값" 조립 규약(명세 §4 "오기 정정").
            let seek_settings = SeekSettings::from_store(&settings_store);
            *state.seek.lock().unwrap() = seek_settings.clone();
            *state.load_notice.lock().unwrap() = notice_from_outcome(&load_outcome);
            // ⭐ F-10 §3.4 — 앱별 비활성화 목록을 여기서 복원한다. `settings_store` 를
            // `state.store` 로 옮기기 *전에* 이 지역 변수에서 직접 읽는다(둘 다 아직
            // 같은 값이지만, 옮긴 뒤에 다시 락을 잡는 왕복을 피한다).
            let disabled_apps: Vec<String> = settings_store
                .get(settings_keys::GENERAL_DISABLED_APPS)
                .unwrap_or_default();
            state.gate_controller.set_disabled_apps(disabled_apps);
            // ⭐ F-16 D-K3 — 한국어 전용 앱 제외 게이트. ⭐ K5(이슈 #73, D-K17) —
            // 저장된 사용자 오버라이드 목록이 있으면 그것이 기본 12종을 **통째로
            // 대체**한다(`resolve_excluded_bundle_ids` — 병합 규칙의 정본). 활성
            // 여부는 `korean.disableInRemoteDesktop`(기본 `true`) 그대로 반영한다.
            let korean_excluded = ultrakey_korean::resolve_excluded_bundle_ids(
                settings_store
                    .get::<Vec<String>>(keys::KOREAN_EXCLUDED_BUNDLE_IDS)
                    .as_deref(),
            );
            state.gate_controller.set_korean_excluded_apps(korean_excluded);
            state
                .gate_controller
                .set_korean_exclusion_enabled(korean_settings.disable_in_remote_desktop);
            // ⭐ F-19(D-7) — ja/zh 제외 목록도 같은 방식으로 부팅 복원(기본 목록은
            // F-16 과 동일한 "원격 세션" 정책 — Q5, 명세 §3.5).
            let japanese_excluded = ultrakey_korean::resolve_excluded_bundle_ids(
                settings_store
                    .get::<Vec<String>>(keys::JAPANESE_EXCLUDED_BUNDLE_IDS)
                    .as_deref(),
            );
            state
                .gate_controller
                .set_japanese_excluded_apps(japanese_excluded);
            let chinese_excluded = ultrakey_korean::resolve_excluded_bundle_ids(
                settings_store
                    .get::<Vec<String>>(keys::CHINESE_EXCLUDED_BUNDLE_IDS)
                    .as_deref(),
            );
            state
                .gate_controller
                .set_chinese_excluded_apps(chinese_excluded);

            // ⭐ A-2(이슈 #39, §3.1.2-a) — 저장된 `general.language` 가 있으면 그
            // 로케일로 카탈로그를 교체한다. 없으면 main() 이 이미 만들어 둔
            // 시스템 로케일 기반 카탈로그를 그대로 쓴다. `setup_tray()`(트레이
            // 메뉴 최초 조립)보다 반드시 앞에 있어야 한다.
            if let Some(locale) = resolve_stored_language(&settings_store) {
                state.catalog.store(Arc::new(Catalog::for_locale(locale)));
                tracing::info!(locale = locale.code(), "applied stored general.language at boot");
            }

            *state.store.lock().unwrap() = settings_store;

            // ⭐ B-2(이슈 #39, `menu-bar-and-lifecycle.md` §3.5-a) — 거울
            // (`general.launchOnLogin`)과 OS 정본을 맞춘다. `login_item::
            // set_enabled` 가 최대 약 0.8초 블로킹하므로 별도 스레드에서 돈다 —
            // 기동을 늦추지 않는다.
            reconcile_login_item_at_boot(&state);

            // 4-b) ⭐ F-03 실기기 검증 하네스(이슈 #34) — 환경변수가 있을 때만.
            // 설정 저장소가 채워진 **뒤**에 둔다: 검색 바 위치(F-15 "부재 =
            // 기본값")를 읽어야 하기 때문이다.
            if overlay_demo::enabled() {
                let stored = {
                    let store = state.store.lock().unwrap();
                    overlay_demo::read_stored_origin(&store)
                };
                tracing::warn!(?stored, "ULTRAKEY_OVERLAY_DEMO set; F-03 overlay verification mode");
                overlay_demo::start(app.handle(), stored);
                // ⛔ 여기서 반환한다 — 데모 모드는 **엔진(CGEventTap)을 켜지
                // 않는다.** 오버레이 검증에 리매핑이 필요 없고, 탭을 안 켜야
                // 이미 떠 있는 인스턴스와 나란히 돌릴 수 있다(아래 단일
                // 인스턴스 예외의 전제가 이것이다).
                return Ok(());
            }

            let handle = app.handle().clone();

            // ⭐ F-01 활성화 경로 1 — `GlobalHotKeyManager` 는 **메인 스레드에서**
            // 만든다(`global-hotkey` 크레이트 문서: "On macOS, an event loop must be
            // running on the main thread so you also need to create the global
            // hotkey manager on the same thread as the event loop"). `setup()` 은
            // Tauri 가 메인 스레드에서 부르므로 여기가 그 자리다. 앱 생애주기 내내
            // 살려 둔다(`AppState.global_hotkey_manager` 문서 참고) — 재등록이 이
            // 손잡이를 다시 써야 하므로 `overlay_demo::ScreenObserver` 처럼 누수시키지
            // 않고 `AppState` 에 담아 둔다.
            match global_hotkey::GlobalHotKeyManager::new() {
                Ok(manager) => {
                    let mut registered = state.global_hotkey_registered.lock().unwrap();
                    seek::apply_global_shortcut(
                        &manager,
                        &mut registered,
                        seek_settings.toggle_shortcut.as_ref(),
                    );
                    drop(registered);
                    *state.global_hotkey_manager.lock().unwrap() = Some(manager);
                }
                Err(e) => tracing::error!(
                    error = %e,
                    "failed to create GlobalHotKeyManager; the Seek global shortcut path will not work"
                ),
            }

            // ⭐ F-01 §5 #6 — 세션 중 디스플레이 구성 변경(핫플러그). `overlay_demo`
            // 와 같은 알림(`NSApplicationDidChangeScreenParametersNotification`)을
            // 구독해 Seek 워커로 넘긴다. 옵저버는 메인 스레드에서 만들고 앱 생애주기
            // 내내 살아 있어야 하므로 누수시킨다(`overlay_demo.rs` 와 같은 이유 —
            // `ScreenObserver` 는 `Send` 가 아니다).
            let state_for_hotplug = state.clone();
            let hotplug_observer = ultrakey_platform::screens::ScreenObserver::start(move || {
                send_seek_signal(&state_for_hotplug, seek::SeekSignal::DisplaysChanged);
            });
            Box::leak(Box::new(hotplug_observer));

            // ⭐ 이슈 #32 Phase 1 — 설정 창 크기 복원 + 디바운스 저장 배선. 저장소를
            // `state.store` 로 옮긴 바로 다음(위 줄)이라야 저장된 크기를 읽을 수 있다.
            wire_window_size_persistence(&handle, &state);
            // ⭐ 이슈 #88(D1) — 설정 창 닫기(빨간 버튼·`⌘W`)를 숨김으로 전환.
            // 상주 정책이라 크기 영속 배선(위)과 순서 독립이다.
            wire_settings_window_lifecycle(&handle);
            let state_for_monitor = state.clone();

            // 5-b) ⭐ F-10 메뉴바(`NSStatusItem`) — 정상/`unauthorizedMenu` 두 벌을
            // 만들어 둔다. 권한 상태를 아직 모르니 안전한 기본값(`unauthorizedMenu`)
            // 으로 시작하고, 아래 최초 `on_permission_transition` 호출이 곧바로
            // 실제 상태에 맞는 메뉴로 갈아 끼운다.
            if let Err(e) = setup_tray(app.handle(), &state) {
                tracing::error!(error = %e, "failed to initialize menu bar (NSStatusItem)");
            }
            // ⭐ 이슈 #78 — 트레이 상태 스냅샷 폴러(1초 간격, `debug` 로그). 트레이가
            // 만들어진 뒤여야 의미 있는 샘플이 남으므로 `setup_tray` 다음에 띄운다.
            spawn_tray_snapshot_poller(app.handle().clone(), state.clone());
            setup_front_app_tracking(app.handle(), &state);

            // ⭐ 이슈 #68 — 두 번째 프로세스의 "설정창 띄워라" 분산 알림 구독.
            // 수신 콜백은 알림 센터의 관례상 메인 스레드에서 불리지만, 창 조작은
            // `on_main_thread` 헬퍼가 다시 메인 스레드로 큐잉하므로 어느 스레드에서
            // 와도 안전하다. `show()`/`set_focus()` 는 창이 이미 보일 때 멱등하다 —
            // "이미 떠 있으면 중복 띄우지 않는다"가 구조적으로 성립한다.
            setup_show_settings_observer(app.handle(), &state);

            // ⭐ F-13 — 부팅 시점 자동 확인 상태 동기화 + 업데이트 재시작 전 정리 훅.
            // General 탭 체크박스가 이 미러를 그린다(정본은 Sparkle). 이벤트 콜백은
            // Sparkle delegate 가 프로세스 종료 **전에 동기로** 부르므로, modifier
            // 해소가 끝난 뒤에만 재시작이 진행된다(명세 §5-10, §8).
            if let Err(e) = refresh_auto_update_check(app.handle(), &state, None) {
                tracing::error!(error = %e, "failed to synchronize automatic update checks at boot");
            }
            #[cfg(target_os = "macos")]
            {
                use tauri_plugin_sparkle_updater::SparkleUpdaterExt;
                if let Some(updater) = app.handle().sparkle_updater() {
                    let state_for_relaunch = state.clone();
                    updater.set_event_callback(Some(Arc::new(move |event, _payload| {
                        if event == "sparkle://will-relaunch-application" {
                            prepare_engine_for_update_restart(&state_for_relaunch);
                        }
                    })));
                }
            }

            // 5) F-11 권한 감시. 전이가 오면 엔진을 켜거나 모달을 띄운다.
            let timings = EngineConfig::default().timings;
            let monitor = PermissionMonitor::start(
                timings.permission_poll_onboarding_ms,
                timings.permission_poll_background_ms,
                Box::new(move |transition| {
                    tracing::info!(?transition, "permission state transition");
                    on_permission_transition(&handle, &state_for_monitor, transition.to);
                }),
            );

            let initial = monitor.check_now();
            *state.monitor.lock().unwrap() = Some(monitor);
            on_permission_transition(app.handle(), &state, initial);

            tracing::info!(
                windows = ?app.webview_windows().keys().collect::<Vec<_>>(),
                "setup() complete; webview window list"
            );

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Tauri 앱을 초기화하지 못했다")
        .run(|app_handle, event| match event {
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
                    tracing::info!("exit requested due to window close; continuing to run");
                    api.prevent_exit();
                } else {
                    // ⭐ F-13 — Sparkle 이 업데이트 재시작을 위해 프로세스를 끝내는
                    // 경로도 여기를 지난다. `will-relaunch` 이벤트 콜백이 이미
                    // 정리했다면 엔진은 `guard.take()` 로 빠져 있어 이 호출은
                    // 멱등으로 아무것도 하지 않는다. 어떤 종료 경로라도 엔진이
                    // 남아 있으면(콜백이 안 불린 엣지) 여기서 마저 정리한다 —
                    // `shutdown()` 은 탭 스레드를 join 하므로 반환 시점에는
                    // 유저-보이는 상태(눌린 modifier)가 원상복구돼 있다.
                    prepare_engine_for_update_restart(&app_handle.state::<Arc<AppState>>());
                    tracing::info!("exit requested; engine cleanup verified");
                }
            }
            tauri::RunEvent::Reopen { .. } => {
                // ⭐ 이슈 #92(실측 확정) — 사용자가 실행 중인 앱을 "다시 실행"하는
                // 실제 경로(Finder 더블클릭·`open`)는 **두 번째 프로세스를 만들지
                // 않는다.** macOS LaunchServices 가 이미 실행 중인 인스턴스를
                // 감지하고 `kAEReopenApplication`(reopen) Apple Event 를 그
                // 인스턴스로 보낸다 → tao `applicationShouldHandleReopen:` → 여기.
                // M2 2차가 남긴 주석("Accessory 앱은 Dock 아이콘이 없어 이
                // 이벤트가 사실상 발생하지 않는다")은 실측으로 **틀렸다** —
                // `LSUIElement` 액세서리 앱도 재실행(`open`) 시 이 콜백을 받는
                // 것을 검증 앱으로 확인했다(이슈 #92 원인 확정 단계).
                // 재실행의 기대 동작("설정 창 열기")은 여기서 처리한다. 이미 보이면
                // `show_settings_window` 의 `is_visible` 분기가 포커스만 주므로
                // (이슈 #68 체크리스트 2) 중복 창은 생기지 않는다. 진짜 두 번째
                // 프로세스가 뜨는 이중 실행(`open -n`)은 `setup_show_settings_observer`
                // 의 분산 알림 경로(#68)가 계속 담당한다 — 두 경로가 같은
                // `show_settings_window` 로 수렴한다.
                tracing::info!("reopen event received; opening the settings window (issue #92)");
                show_settings_window(app_handle);
            }
            _ => {}
        });
}

/// 권한 상태에 따라 엔진을 켜거나 모달을 띄운다.
///
/// ⭐ **이슈 #65 Phase 1 리뷰 결정 2** — `Granted`(엔진 시작)·`Denied`/`OutOfSync`/
/// `Unknown`(엔진 정지) 양쪽 모두 `run_on_main_thread` 로 큐잉한다(`queue_engine_start`/
/// `queue_engine_stop`, 기존 `on_main_thread` 헬퍼와 같은 패턴). 이 콜백은
/// `ultrakey-permission-poll` 스레드(백그라운드 폴링)나 `ultrakey-tap` 스레드
/// (`EngineEvent::TapLost` 경유, 교정 3)에서 불릴 수 있는데, `start_engine_if_needed`
/// 가 호출하는 `Engine::start` → `SystemHooks::start` 는 메인 스레드 호출을
/// 요구한다(`system_hooks.rs` 모듈 문서, 위반 시 `debug_assert!` 로 소리 낸다) —
/// `Granted` 쪽은 이미 이 전제를 어기고 있었다(이번 조사에서 확인). 큐잉만 하고
/// 즉시 반환하므로(`AppHandle::run_on_main_thread` 계약) 폴링/탭 스레드가 메인
/// 스레드를 기다리며 블록되는 일은 없다 — `show_modal`/`hide_modal`/
/// `apply_tray_menu_for_permission` 이 이미 같은 방식으로 안전하다.
///
/// 두 클로저는 같은 메인 스레드 이벤트 루프 큐에 순서대로 들어가므로, `Denied` →
/// `Granted` 가 빠르게 연달아 와도 정지 클로저가 시작 클로저보다 먼저 실행되는
/// 순서가 보존된다.
fn on_permission_transition(handle: &tauri::AppHandle, state: &Arc<AppState>, to: PermissionState) {
    tracing::info!(?to, "on_permission_transition entered");
    if should_stop_engine_for(to) {
        show_modal(handle);
        apply_tray_menu_for_permission(handle, state, false, "permission");
        queue_engine_stop(handle, state);
    } else {
        hide_modal(handle);
        // ⭐ M2 2차부터: 메뉴바 `Settings…` 가 생겼으니 권한이 생겼다고 창을
        // 자동으로 띄우지 않는다(과거 M2 1차 임시 조치를 걷어낸다 — 원래
        // 계획대로다). 대신 트레이 메뉴를 정상 메뉴로 되돌린다(§3.1).
        apply_tray_menu_for_permission(handle, state, true, "permission");
        queue_engine_start(handle, state);
    }
}

/// `on_permission_transition` 이 엔진을 정지해야 하는 상태인지 — 순수 함수(이슈 #65
/// Phase 1 리뷰 "테스트 요구"). `Granted` 만 엔진을 살려 둔다: `Denied` 는 권한 상실
/// 그 자체, `OutOfSync`/`Unknown` 은 살아있는 탭이 있을 수 없는 상태라 방어적으로
/// 함께 정지한다(엔진이 이미 없으면 `queue_engine_stop` 이 멱등하게 아무 일도 하지
/// 않는다).
fn should_stop_engine_for(state: PermissionState) -> bool {
    !matches!(state, PermissionState::Granted)
}

/// `start_engine_if_needed` 를 메인 스레드에 큐잉만 하고 즉시 반환한다(교정 2 문서
/// 참고 — 호출 스레드가 메인 스레드를 기다리지 않는다).
fn queue_engine_start(handle: &tauri::AppHandle, state: &Arc<AppState>) {
    let handle_for_closure = handle.clone();
    let state_for_closure = state.clone();
    let dispatched = handle.run_on_main_thread(move || {
        start_engine_if_needed(&handle_for_closure, &state_for_closure);
    });
    if let Err(e) = dispatched {
        tracing::error!(error = %e, "failed to queue engine start on the main thread");
    }
}

/// `stop_engine` 을 메인 스레드에 큐잉만 하고 즉시 반환한다.
fn queue_engine_stop(handle: &tauri::AppHandle, state: &Arc<AppState>) {
    let state_for_closure = state.clone();
    let dispatched = handle.run_on_main_thread(move || {
        stop_engine(&state_for_closure);
    });
    if let Err(e) = dispatched {
        tracing::error!(error = %e, "failed to queue engine stop on the main thread");
    }
}

/// ⭐ 이슈 #65 Phase 1 리뷰 교정 — 권한을 잃으면(또는 `OutOfSync`/`Unknown`) 엔진
/// 전체(탭 스레드·워치독·시스템 훅·지연 스케줄러)를 정리한다.
///
/// **`state.engine` 슬롯을 반드시 비워야 한다** — 그래야 다음 `Granted` 전이가
/// `start_engine_if_needed` 를 통해 완전히 새 `Engine` 을 만들 수 있다. 교정 전에는
/// (a) 탭 스레드 자신의 FSM 이 권한 상실을 감지해 내부 탭만 내려놔도 `state.engine`
/// 슬롯은 죽은 탭을 담은 채 계속 `Some` 이었고, (b) `start_engine_if_needed` 는
/// `slot.is_some()` 이면 즉시 `return` 하므로 — 권한을 재부여해도 **앱을 재시작하기
/// 전까지 리매핑이 영구히 돌아오지 않았다**(이슈 #65 Phase 1 진단이 이 워크트리에서
/// 추가로 확정한 사실).
///
/// ⚠️ **`Engine::shutdown()` 을 메인 스레드에서 join 해도 안전한 이유** — 이 함수는
/// 탭 스레드에 `EngineCommand::Shutdown` 을 보내고 `run_loop.stop()` 을 부른 뒤
/// join 한다(`engine.rs::Engine::shutdown`). 탭 스레드의 런루프는 이 호출 시점에
/// 이미 자유롭다: 트램폴린의 연속 재활성화 예산이 시간이 아니라 연속 소비로만
/// 소진되므로(교정 1) 폭주는 최대 `REENABLE_MAX_CONSECUTIVE`(5)회 즉시 핑퐁으로
/// 끝나고, 이후에는 탭이 이미 해체됐거나(교정 2, `handle_recover_tap` 이 재생성을
/// 시도하지 않는다) 정상 서비스 중이다 — 어느 경우든 명령 큐(`drain_commands`)가
/// 굶주려 있지 않다. `prepare_engine_for_update_restart`(§5-10)가 같은
/// "lock → take → shutdown" 패턴으로 이미 이 join 을 실사용하고 있어 별도 위험이
/// 추가되지 않는다.
fn stop_engine(state: &Arc<AppState>) {
    let mut slot = match state.engine.lock() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to acquire engine lock while stopping the engine");
            return;
        }
    };
    if let Some(engine) = slot.take() {
        drop(slot);
        // ⭐ 종료 전 stuck modifier 해소 — `prepare_engine_for_update_restart` 와
        // 동일한 이유(§5-10): 하이퍼키/조합 modifier 를 누른 채 권한을 잃었을 수
        // 있다.
        engine.force_reset_state();
        engine.shutdown();
        tracing::info!("engine stopped after permission loss");
    }
    // ⭐ F-06 — 트랙패드 리스너도 함께 정리한다(`prepare_engine_for_update_restart`
    // 와 동일한 이유, §8).
    if let Ok(mut trackpad) = state.trackpad.lock() {
        if let Some(listener) = trackpad.take() {
            listener.shutdown();
            tracing::info!("trackpad gesture listener cleaned up after permission loss");
        }
    }
}

fn start_engine_if_needed(handle: &tauri::AppHandle, state: &Arc<AppState>) {
    let mut slot = match state.engine.lock() {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "failed to acquire engine lock");
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
    let japanese = *state.japanese.lock().unwrap();
    let chinese = *state.chinese.lock().unwrap();
    let seek_settings = state.seek.lock().unwrap().clone();
    let config = {
        let store = state.store.lock().unwrap();
        build_engine_config(
            &hyperkey,
            &presets,
            &korean,
            &seek_settings,
            &japanese,
            &chinese,
            &store,
        )
    };

    // F-17 — `LedgerStore` 구현(`perDevice._managed`, 계약 §B.2). 엔진이 이것을
    // `PathBManager` 에 넘겨 디바이스별 원장을 영속화한다.
    let ledger: Box<dyn LedgerStore> = Box::new(AppLedgerStore {
        app_state: state.clone(),
    });

    let handle_for_events = handle.clone();
    let state_for_events = state.clone();
    match Engine::start(
        config,
        state.gate.clone(),
        ledger,
        Box::new(move |event| on_engine_event(&handle_for_events, &state_for_events, event)),
    ) {
        Ok(engine) => {
            tracing::info!(state = ?engine.tap_state(), "engine started");
            let shared = engine.shared();
            state.engine_shared.store(Some(Arc::clone(&shared)));
            *slot = Some(engine);
            drop(slot);

            // ⭐ F-01 — Seek 워커 기동(위임 지시 §7). `overlay::SurfaceState` 는
            // `main()` 의 `.manage(Arc<Mutex<SurfaceState>>)` 로 이미 등록돼 있다.
            let surface_state = handle
                .state::<Arc<Mutex<overlay::SurfaceState>>>()
                .inner()
                .clone();
            let state_for_origin = state.clone();
            let stored_origin: Arc<dyn Fn() -> Option<(f64, f64)> + Send + Sync> =
                Arc::new(move || {
                    let store = state_for_origin.store.lock().ok()?;
                    overlay::stored_search_bar_origin(&store)
                });
            let state_for_persist = state.clone();
            let persist_origin: Arc<dyn Fn(f64, f64) + Send + Sync> = Arc::new(move |x, y| {
                if let Ok(mut store) = state_for_persist.store.lock() {
                    overlay::persist_search_bar_origin(&mut store, x, y);
                }
            });
            let quick_press_opens = quick_press_opens_seek(&presets);
            let seek_config = seek_settings.to_config(quick_press_opens);
            // ⭐ F-04(이슈 #44) — executor 의 초기 설정도 같은 저장 표현에서
            // 조립한다(부재 = 기본값 규약은 `SeekSettings::from_store` 쪽이
            // 이미 반영했다).
            let click_settings = click_settings_from_seek(&seek_settings);
            // ⭐(이슈 #93) — `seek.searchLanguage`(검색 언어)가 **명시**돼 있으면
            // 그 값이 OCR 언어의 정본이다: 비영어(`"ko"` 등) → `["<lang>-<region>",
            // "en-US"]`(첫 원소가 인식 언어를 가름, 스파이크 §3), 명시 `"en"` →
            // `[]`(영어 강제). **부재(키 없음) = 영어 고정** (이슈 #131 — 이슈
            // #48 의 로케일 폴백 `catalog.locale()` 은 폐기, Plan D1·D6 §9 #1~#2).
            let state_for_ocr = state.clone();
            let ocr_languages: Arc<dyn Fn() -> Vec<String> + Send + Sync> = Arc::new(move || {
                let explicit = state_for_ocr
                    .store
                    .lock()
                    .ok()
                    .and_then(|store| store.get::<String>(keys::SEEK_SEARCH_LANGUAGE));
                let locale = explicit
                    .as_deref()
                    .and_then(Locale::from_search_language)
                    // 부재(또는 비정규 값) → 영어 고정 (이슈 #131; 이슈 #48 의
                    // 로케일 폴백은 폐기). 명시 `"en"` 도 `Locale::En` 이라
                    // 부재와 같은 경로를 탄다.
                    .unwrap_or(Locale::En);
                locale
                    .ocr_recognition_languages()
                    .iter()
                    .map(|s| s.to_string())
                    .collect()
            });
            // ⭐(이슈 #133) — `seek.includeWindowTitles` 를 세션 열림 시점에
            // 읽는다. F-02 검출 설정이라 `ConfigChanged` 신호로 못 실어
            // `ocr_languages` 와 같은 클로저 경계로 주입한다 — 부재 = true 는
            // `SeekSettings::from_store` 가 이미 반영한다(D12 설계 결정).
            let state_for_window_titles = state.clone();
            let include_window_titles: Arc<dyn Fn() -> bool + Send + Sync> =
                Arc::new(move || {
                    let seek = state_for_window_titles
                        .store
                        .lock()
                        .ok()
                        .map(|store| SeekSettings::from_store(&store));
                    // 락 실패는 드물고 우발적 — 출고(☑) 기본값으로 내려간다.
                    seek.is_none_or(|s| s.include_window_titles)
                });
            let tx = seek::spawn(
                handle.clone(),
                shared.clone(),
                surface_state,
                seek_config,
                click_settings,
                stored_origin,
                ocr_languages,
                include_window_titles,
                persist_origin,
            );
            *state.seek_tx.lock().unwrap() = Some(tx);

            // ⭐ F-06 — 트랙패드 제스처 리스너 기동(이슈 #63). 엔진이 살아 있어야
            // `SharedState.trackpad` 게이트가 존재한다. 비공개 API 로드 실패 시
            // 리스너 스스로 격하된다(다른 기능 무영향, §8). 설정이 꺼져 있으면
            // 스레드를 띄우지 않는다 — 꺼진 설정에 스레드가 놀 필요가 없다.
            let hyperkey_started = state.hyperkey.lock().unwrap().clone();
            if hyperkey_started.trackpad.enabled && hyperkey_started.hyper.enabled {
                let runtime = trackpad::TrackpadRuntimeConfig {
                    enabled: true,
                    area: hyperkey_started.trackpad.area,
                };
                *state.trackpad.lock().unwrap() =
                    Some(trackpad::spawn(&shared, runtime));
            }
            // ⭐ 이슈 #110 — 기동 재조정의 되읽기 결과가 이제야 확정됐으므로 트레이
            // 상태 항목("caps lock 커널 매핑 미적용")을 반영한다.
            refresh_tray_caps_status(state);
        }
        Err(e) => tracing::error!(error = %e, "failed to start engine"),
    }
}

/// ⭐ 이슈 #110 — "caps lock 커널 매핑 미적용" 판정. D-1 이 필요한 설정
/// (`caps_lock_alias.is_some()`)인데 마지막 경로 B 재조정에서 되읽기 확인이 되지
/// 않았다(`SharedState::d1_confirmed == false`). 엔진이 없으면 `false` — 그 상태는
/// 권한 안내·`engine_not_running` 이 따로 알린다.
fn caps_lock_kernel_map_missing(state: &AppState) -> bool {
    match state.engine_shared.load_full() {
        Some(shared) => {
            shared.config.load().caps_lock_alias.is_some()
                && !shared.d1_confirmed.load(std::sync::atomic::Ordering::Acquire)
        }
        None => false,
    }
}

/// ⭐ 이슈 #110 — 트레이 상태 항목은 메뉴를 다시 조립해야 바뀌므로, 판정값이
/// **바뀐 경우에만** `rebuild_tray_menu` 를 부른다(설정 변경마다 메뉴를 새로
/// 만들지 않는다). 트레이가 아직 없으면(setup 전) 아무것도 하지 않는다.
/// ⚠️ 호출자는 `state.engine` 뮤텍스를 쥐고 있지 않아야 한다 — `rebuild_tray_menu`
/// 가 잡는 `monitor`/`tray`/메뉴 슬롯과의 순서를 새로 만들지 않기 위해서다.
fn refresh_tray_caps_status(state: &Arc<AppState>) {
    let missing = caps_lock_kernel_map_missing(state);
    let previous = state
        .tray_caps_missing
        .swap(missing, std::sync::atomic::Ordering::AcqRel);
    if previous == missing {
        return;
    }
    let handle = state
        .tray
        .lock()
        .ok()
        .and_then(|t| t.as_ref().map(|t| t.app_handle().clone()));
    if let Some(handle) = handle {
        rebuild_tray_menu(&handle, state);
    }
}

/// ⭐ F-01 배선 — `Seek*` 4종은 `EngineEvent` 문서 계약대로 채널에 밀어 넣기만 하고
/// **즉시 반환**한다. `tracing` 호출조차 넣지 않는다(`SeekKey` 는 세션 중 매 키마다
/// 온다 — 모듈 문서의 "블록하지 마라" 계약이 특히 무겁게 적용되는 자리다).
fn on_engine_event(handle: &tauri::AppHandle, state: &Arc<AppState>, event: EngineEvent) {
    match event {
        EngineEvent::TapStateChanged(s) => {
            tracing::info!(state = ?s, "tap state changed");
            if s == TapState::Active {
                // ⭐ 이슈 #65 Phase 1 리뷰 교정 3 — 그동안 호출자가 하나도 없어
                // 사문화돼 있던 `PermissionMonitor::report_tap_created()` 를
                // 여기 배선한다. `OutOfSync`(권한은 있는데 탭을 만들 수 없던 상태)
                // 에서 회복하는 유일한 경로(§2 S4 항목 5)가 이걸로 실제로
                // 연결된다.
                report_tap_created(state);
            }
        }
        EngineEvent::NotTrusted => {
            // 권한이 없어 탭을 못 연 것은 정상 경로다 — F-11 온보딩이 처리한다.
            tracing::info!("no permission; handing off to onboarding modal");
            show_modal(handle);
        }
        EngineEvent::FatalTapCreateFailed => {
            // ⛔ `key-remapping-engine.md` §3-a·§5#16: 재시도가 아니라 종료다.
            tracing::error!("tap creation failed despite confirmed permission; exiting process");
            show_modal(handle);
        }
        EngineEvent::TapLost => {
            // ⭐ 이슈 #65 Phase 1 리뷰 교정 2·3 — 탭이 살아있던 중 권한 상실(또는
            // 트램폴린의 연속 재활성화 예산 소진)로 해체됐다. 이 엔진은 스스로
            // 재생성을 시도하지 않는다 — 권한 모델에 이관해 `Denied`/`OutOfSync`
            // 를 판정하게 하고, 그 전이가 `on_permission_transition` 을 거쳐
            // `queue_engine_stop` 으로 이어진다(그 시점에 `state.engine` 슬롯이
            // 비워져야 다음 `Granted` 가 완전히 새 `Engine` 을 만들 수 있다).
            tracing::warn!("tap was dismantled after repeated immediate re-disable or permission loss; handing off to the permission monitor");
            report_tap_lost(state);
        }
        EngineEvent::SeekOpenRequested => send_seek_signal(state, seek::SeekSignal::OpenRequested),
        EngineEvent::SeekTriggerDown => send_seek_signal(state, seek::SeekSignal::TriggerDown),
        // ⭐ 실린 flags 가 **트리거 키를 떼는 그 순간의 modifier 스냅샷**이다 —
        // F-04(`seek-click-execution.md`) §5 #10 이 요구하는 값이고, 그 순간을 아는
        // 것은 탭 콜백뿐이다. 여기서 그대로 `ConfirmedMatch::modifiers` 까지 흘려
        // 보낸다(F-04 가 아직 없어 지금은 로그로만 관측된다).
        EngineEvent::SeekTriggerUp(flags) => {
            send_seek_signal(state, seek::SeekSignal::TriggerUp(flags))
        }
        EngineEvent::SeekKey(ev) => send_seek_signal(state, seek::SeekSignal::Key(ev)),
    }
}

/// ⭐(이슈 #93) — 다국어(인풋 박스) Seek 세션에서 웹뷰 `<input>` 이 디바운스해
/// 보낸 **완성된 검색어**를 워커에 전달한다. 워커가 없으면 조용히 버린다
/// (`send_seek_signal` 과 같은 규약). 제어 키가 아니라 **문자열**이므로 기존
/// `EngineEvent::SeekKey` 채널을 쓰지 않는다.
#[tauri::command]
fn seek_set_query(state: State<'_, Arc<AppState>>, query: String) {
    send_seek_signal(state.inner(), seek::SeekSignal::SetQuery(query));
}

/// `AppState.seek_tx` 로 신호를 보낸다 — 워커가 아직 뜨지 않았으면(엔진 시작 전)
/// 조용히 버린다.
fn send_seek_signal(state: &Arc<AppState>, signal: seek::SeekSignal) {
    if let Ok(guard) = state.seek_tx.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(signal);
        }
    }
}

/// ⭐ 이슈 #65 Phase 1 리뷰 교정 3 — `EngineEvent::TapLost` 를 권한 모델에 이관한다.
/// `PermissionMonitor::report_tap_create_failed()` 는 호출 시점의
/// `AXIsProcessTrusted()` 로 `Denied`(권한이 실제로 없다)/`OutOfSync`(권한은 있는데
/// 탭이 살 수 없다)를 가른다(`model.rs` §3.3). 이 함수는 탭 스레드에서 불릴 수
/// 있으므로(`on_engine_event` 의 "블록하지 마라" 계약) 락을 짧게만 쥐고, 실제 전이
/// 통지는 `on_transition` 콜백(=`on_permission_transition`, 내부적으로 전부
/// 큐잉만 한다)에 위임한다.
fn report_tap_lost(state: &Arc<AppState>) {
    if let Ok(guard) = state.monitor.lock() {
        if let Some(monitor) = guard.as_ref() {
            monitor.report_tap_create_failed();
        }
    }
}

/// ⭐ 이슈 #65 Phase 1 리뷰 교정 3 — 탭이 `Active` 로 전이할 때마다 부른다.
/// `PermissionMonitor::report_tap_created()` 는 `OutOfSync` 에서 회복하는 유일한
/// 경로다(§2 S4 항목 5) — 이 배선이 없으면 그 경로는 죽어 있는 API 로 남는다(이슈
/// #65 Phase 1 리뷰가 지적한 사실).
fn report_tap_created(state: &Arc<AppState>) {
    if let Ok(guard) = state.monitor.lock() {
        if let Some(monitor) = guard.as_ref() {
            monitor.report_tap_created();
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
                tracing::info!(window_label, what, "window found");
                action(&w);
                tracing::info!(
                    window_label,
                    what,
                    is_visible = ?w.is_visible(),
                    outer_position = ?w.outer_position(),
                    outer_size = ?w.outer_size(),
                    is_focused = ?w.is_focused(),
                    is_minimized = ?w.is_minimized(),
                    "window state after operation"
                );
            }
            None => {
                // ⭐ 이슈 #88(D1) — `"settings"` 창은 `wire_settings_window_lifecycle`
                // 가 닫기를 숨김으로 전환하므로 앱이 살아 있는 동안 파괴되지
                // 않는다. 따라서 이 분기는 정상 경로에서 도달하지 않는 예외
                // 상황이다(예: 부팅 시 창 생성 실패). **재생성(builder)을 만들지
                // 않는다** — 상주 정책(D1)이 파괴-재생성보다 최소 변경이다.
                tracing::error!(window_label, what, "window not found");
            }
        }
    });
    if let Err(e) = dispatched {
        tracing::error!(window_label, what, error = %e, "failed to dispatch to the main thread");
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

/// ⭐ 이슈 #88(D1) — 환경설정 창은 **상주**한다(닫으면 파괴되지 않고 숨김).
/// `wire_settings_window_lifecycle` 이 유일한 닫기 경로(빨간 버튼·`⌘W`)를
/// `prevent_close()` + `hide()` 로 처리하므로, `on_main_thread` 의 `None` 분기가
/// 도달하는 것은 설정 창이 파괴됐다는 뜻의 예외 상황이다 — 재생성 코드는
/// 만들지 않는다(계획 D1: 상주가 재생성보다 최소 변경).
///
/// 메뉴바 `Settings…`·이슈 #68 재실행 신호가 이 함수를 공통으로 탄다. ⭐ 이슈
/// #87 — `About` 은 더 이상 여기로 오지 않는다(`show_about_window` 로 분리).
fn show_settings_window(handle: &tauri::AppHandle) {
    on_main_thread(handle, "settings", "show_settings", |w| {
        // ⭐ 이슈 #68 — 이미 보이는 창을 다시 `show()` 하지 않는다. 두 번째
        // 프로세스의 재실행 신호(이슈 #68)도 이 함수로 흘러오므로, "이미 떠
        // 있으면 중복 띄우지 않는다"(체크리스트 2)가 이 분기 하나로 성립한다.
        // 숨겨져 있지 않으면(보이거나 최소화돼 있거나) 포커스만 시도해 사용자를
        // 그 창으로 데려간다.
        if matches!(w.is_visible(), Ok(true)) {
            tracing::info!("settings window is already visible; only requesting focus");
            let _ = w.set_focus();
            return;
        }
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

// ── 이슈 #32 Phase 1: 설정 창 크기 영속(F-15 규약) ──────────────────────────

/// 저장된 `ui.windowWidth`/`ui.windowHeight` 를 복원 가능한 크기로 해석한다.
///
/// 둘 다 있어야 `Some` 을 반환한다 — 하나만 있으면 `None`("부재 = 기본값",
/// F-15 §3.1, `tauri.conf.json` 의 `SETTINGS_WINDOW_DEFAULT` 가 그대로 쓰인다).
/// 방어적 범위(너비 320..=6000, 높이 240..=6000)를 벗어나도 `None` — 손상되었거나
/// 미래 버전이 써 둔 값을 그대로 믿지 않는다. 순수 함수라 단위 테스트로 직접
/// 부른다.
fn restored_window_size(width: Option<f64>, height: Option<f64>) -> Option<(u32, u32)> {
    let (w, h) = (width?, height?);
    if !w.is_finite() || !h.is_finite() {
        return None;
    }
    let w = w.round();
    let h = h.round();
    if !(320.0..=6000.0).contains(&w) || !(240.0..=6000.0).contains(&h) {
        return None;
    }
    Some((w as u32, h as u32))
}

/// 설정 창 크기 저장을 디바운스하는 전용 스레드의 손잡이.
///
/// ⚠️ `WindowEvent::Resized` 는 드래그 중 매 픽셀마다 발생한다 — 이벤트마다 파일을
/// 쓰면 안 되고, 이벤트마다 스레드를 새로 띄워서도 안 된다(스레드 이름이 로그에
/// 남는다는 이 저장소의 관례, 이슈 #10 — 매번 새로 뜨면 그 이름이 의미가 없다).
/// 그래서 스레드 하나를 앱 생애주기 내내 띄워 두고, `Mutex` + `Condvar` 로 "마지막
/// 이벤트로부터 400ms 동안 조용하면 그때 한 번만 쓴다"는 디바운스를 구현한다.
struct WindowSizeDebouncer {
    shared: Arc<WindowSizeDebounceShared>,
    /// 스레드를 앱 생애주기 내내 살려 두려고 들고 있을 뿐, 값 자체는 읽지 않는다
    /// (`system_event_observer` 와 같은 규약).
    #[allow(dead_code)]
    thread: JoinHandle<()>,
}

struct WindowSizeDebounceShared {
    lock: Mutex<WindowSizeDebounceInner>,
    condvar: Condvar,
}

#[derive(Default)]
struct WindowSizeDebounceInner {
    /// 가장 최근에 요청받은, 아직 쓰지 않은 크기.
    pending: Option<(u32, u32)>,
    /// 이 시각까지 새 이벤트가 없으면 `pending` 을 쓴다.
    deadline: Option<Instant>,
}

impl WindowSizeDebouncer {
    /// 마지막 `Resized` 로부터 이 시간 동안 조용하면 한 번만 저장한다.
    const DEBOUNCE: Duration = Duration::from_millis(400);

    fn spawn(state: Arc<AppState>) -> Self {
        let shared = Arc::new(WindowSizeDebounceShared {
            lock: Mutex::new(WindowSizeDebounceInner::default()),
            condvar: Condvar::new(),
        });
        let shared_for_thread = shared.clone();
        let thread = thread::Builder::new()
            .name("ultrakey-window-size".to_string())
            .spawn(move || window_size_debounce_loop(shared_for_thread, state))
            .expect("설정 창 크기 디바운스 스레드 생성 실패");
        WindowSizeDebouncer { shared, thread }
    }

    /// 사용자가 창을 조절했다고 판단된 새 크기를 알린다. 마감을 400ms 뒤로
    /// 미루기만 할 뿐, 여기서 직접 쓰지 않는다 — 실제 쓰기는 디바운스 스레드가 한다.
    fn notify(&self, size: (u32, u32)) {
        let mut inner = self.shared.lock.lock().unwrap();
        inner.pending = Some(size);
        inner.deadline = Some(Instant::now() + Self::DEBOUNCE);
        self.shared.condvar.notify_one();
    }
}

/// [`WindowSizeDebouncer::spawn`] 이 띄우는 전용 스레드의 본체.
fn window_size_debounce_loop(shared: Arc<WindowSizeDebounceShared>, state: Arc<AppState>) {
    loop {
        let mut guard = shared.lock.lock().unwrap();
        // 예약된 마감이 없으면 무기한 대기 — `notify()` 가 깨워 줄 때까지 스핀하지
        // 않는다. 마감이 있으면 그 시각까지만 기다리고, 깨어났을 때 아직 마감
        // 전이면(= 그사이 `notify()` 가 마감을 다시 미뤘다면) 다시 대기한다.
        loop {
            match guard.deadline {
                None => guard = shared.condvar.wait(guard).unwrap(),
                Some(deadline) => {
                    let now = Instant::now();
                    if now >= deadline {
                        break;
                    }
                    guard = shared
                        .condvar
                        .wait_timeout(guard, deadline - now)
                        .unwrap()
                        .0;
                }
            }
        }
        let size = guard.pending.take();
        guard.deadline = None;
        drop(guard);
        if let Some(size) = size {
            persist_window_size(&state, size);
        }
    }
}

/// `ui.windowWidth`/`ui.windowHeight` 를 실제로 디스크에 쓴다. 저장 실패는
/// (디스크·권한 문제 등) 로그만 남기고 앱을 죽이지 않는다 — 창 크기 저장은
/// 핵심 기능이 아니다.
fn persist_window_size(state: &Arc<AppState>, size: (u32, u32)) {
    let mut store = state.store.lock().unwrap();
    if let Err(e) = store.set(keys::UI_WINDOW_WIDTH, &size.0) {
        tracing::error!(error = %e, "failed to save settings window width");
    }
    if let Err(e) = store.set(keys::UI_WINDOW_HEIGHT, &size.1) {
        tracing::error!(error = %e, "failed to save settings window height");
    }
}

/// 설정 창의 `WindowEvent::Resized` 배선 — `setup()` 이 설정 저장소를 만든 직후
/// 한 번만 부른다. 저장된 크기가 있으면 복원하고, 디바운스 저장 스레드를 띄운
/// 뒤 `Resized` 이벤트를 구독한다.
fn wire_window_size_persistence(handle: &tauri::AppHandle, state: &Arc<AppState>) {
    // 1) 저장된 크기가 있으면 복원한다. 하나라도 없으면 `tauri.conf.json` 의
    //    `SETTINGS_WINDOW_DEFAULT` 가 그대로 쓰이므로 조용히 넘어간다("부재 =
    //    기본값"). 둘 다 있는데 범위를 벗어나면 그 저장값이 손상되었다는
    //    뜻이라 경고를 남긴다.
    let (raw_width, raw_height): (Option<f64>, Option<f64>) = {
        let store = state.store.lock().unwrap();
        (
            store.get(keys::UI_WINDOW_WIDTH),
            store.get(keys::UI_WINDOW_HEIGHT),
        )
    };
    if raw_width.is_some() && raw_height.is_some() {
        match restored_window_size(raw_width, raw_height) {
            Some(size) => {
                // 이 크기는 사용자가 조절한 게 아니라 우리가 방금 프로그램적으로
                // 넣는 것이다 — 곧이어 도착할 `Resized` 에코를 저장으로 착각하면
                // 안 된다(`settings_set_tab` 의 `persist` 인자와 같은 이유).
                *state.last_applied_window_size.lock().unwrap() = Some(size);
                resize_settings_window(handle, size);
            }
            None => {
                tracing::warn!(
                    ?raw_width,
                    ?raw_height,
                    "stored settings window size is outside the defensive range; using default"
                );
            }
        }
    }

    // 2) 디바운스 저장 스레드를 기동한다 — 앱 생애주기 동안 하나만 존재한다.
    *state.window_size_debouncer.lock().unwrap() = Some(WindowSizeDebouncer::spawn(state.clone()));

    // 3) 사용자가 드래그로 바꾼 크기를 구독한다.
    match handle.get_webview_window("settings") {
        Some(window) => {
            let window_for_events = window.clone();
            let state_for_events = state.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::Resized(physical_size) = event {
                    on_settings_window_resized(
                        &state_for_events,
                        &window_for_events,
                        *physical_size,
                    );
                }
            });
        }
        None => {
            tracing::error!("settings window not found; could not wire up window size persistence");
        }
    }
}

/// 설정 창의 `Resized` 이벤트 하나를 처리한다 — 저장할 가치가 있는 사용자
/// 조작인지 판정하고, 맞으면 디바운스 스레드로 넘긴다.
///
/// ⭐ 무시해야 하는 두 경우(둘 다 F-15 §8 "설정을 한 번도 건드리지 않으면
/// `settings.json` 이 생기지 않는다"를 지키기 위해서다 — `settings_set_tab` 의
/// `persist` 인자와 같은 근거):
/// 1. 창이 아직 보이지 않을 때(`is_visible() == Ok(false)`) — 창 생성·초기 배치
///    단계에서 오는 리사이즈일 뿐, 사용자가 조절한 게 아니다.
/// 2. 우리가 방금 `resize_settings_window()` 로 프로그램적으로 넣은 크기와 같을
///    때 — 그 결과로 되돌아오는 이벤트를 저장하면 창을 열기만 해도(복원 시)
///    `settings.json` 에 같은 값을 다시 쓰는 의미 없는 왕복이 생긴다.
fn on_settings_window_resized(
    state: &Arc<AppState>,
    window: &WebviewWindow,
    physical_size: PhysicalSize<u32>,
) {
    if !matches!(window.is_visible(), Ok(true)) {
        return;
    }

    // ⭐ 논리 좌표로 변환해서 저장한다 — `resize_settings_window` 가 복원 때 넣는
    // `LogicalSize` 와 단위를 맞춰야 한다. `PhysicalSize` 를 그대로 저장하면
    // Retina(scale_factor 2.0)에서 다음 실행 때 창이 두 배로 커진다.
    let scale_factor = window.scale_factor().unwrap_or(1.0);
    let logical = physical_size.to_logical::<f64>(scale_factor);
    let size = (logical.width.round() as u32, logical.height.round() as u32);

    {
        let mut last_applied = state.last_applied_window_size.lock().unwrap();
        if *last_applied == Some(size) {
            return;
        }
        // 크기가 다르면 더는 "우리가 방금 넣은 값" 취급을 하지 않는다 — 사용자가
        // 실제로 창을 조절하기 시작했다는 뜻이다.
        *last_applied = None;
    }

    if let Some(debouncer) = state.window_size_debouncer.lock().unwrap().as_ref() {
        debouncer.notify(size);
    }
}

/// ⭐ 이슈 #88(D1: 닫기 → 숨김, 창 상시 유지) — 설정 창 `CloseRequested` 배선.
///
/// `tauri.conf.json` 에 `visible:false` 로 static 선언된 설정 창은 부팅 시
/// 생성된다. `CloseRequested` 핸들러가 없으면 사용자가 빨간 버튼·`⌘W` 로 닫는
/// 순간 창이 **파괴**되고, `show_settings_window` 의 `on_main_thread` 는
/// `get_webview_window("settings")` 이 `None` 을 받아 에러 로그만 남긴 채 끝나서
/// "닫았다가 다시 열 수 없는" 상태가 된다(이슈 #88 이 드러낸 결함).
///
/// 여기서 닫기를 파괴가 아니라 숨김으로 전환해 창 인스턴스를 앱 종료까지 상주
/// 시킨다. 상주가 주는 것(계획 D1 근거): ① 파괴-재생성(builder) 코드가 원천적으로
/// 필요 없다 — `show_settings_window` 의 재열림 로직은 창 존재만 전제한다. ② 위
/// `wire_window_size_persistence` 의 크기 영속 배선(디바운스 저장 스레드 포함)이
/// setup() 1회 배선 그대로 살아 있다. ③ 이슈 #68(앱 재실행 → 설정 창)도 별도
/// 수정 없이 해결된다 — 창이 항상 존재하므로 항상 열린다. ④ 탭·스크롤 등의
/// 웹뷰 JS 세션이 유지된다.
///
/// `get_webview_window("settings")` 가 `Some` 이면 배선하고, `None` 이면 에러
/// 로그를 남긴다 — 위 `wire_window_size_persistence` 의 None 분기와 같은 처우.
fn wire_settings_window_lifecycle(handle: &tauri::AppHandle) {
    match handle.get_webview_window("settings") {
        Some(window) => {
            let window_for_close = window.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    tracing::info!(
                        "settings window close requested; hiding instead of destroying (D1, issue #88)"
                    );
                    api.prevent_close();
                    let _ = window_for_close.hide();
                }
            });
        }
        None => {
            tracing::error!("settings window not found; could not wire up close-to-hide lifecycle");
        }
    }
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
/// `Purchase`(F-12) 는 범위 밖이라 넣지 않는다.
fn build_normal_menu(
    handle: &tauri::AppHandle,
    catalog: &Catalog,
    front_app: Option<&AppIdentity>,
    front_app_disabled: bool,
    caps_kernel_map_missing: bool,
) -> tauri::Result<(Menu<Wry>, CheckMenuItem<Wry>)> {
    // ⭐ 이슈 #110 — D-1 미확인 상태 안내. `menu.unauthorized.title` 과 같은
    // 결의 **비활성** 항목이고, 조건이 참일 때만 메뉴 맨 위에 붙인다.
    let caps_status_item = if caps_kernel_map_missing {
        Some(MenuItem::new(
            handle,
            catalog.get("menu.status.caps_kernel_map_missing"),
            false,
            None::<&str>,
        )?)
    } else {
        None
    };
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
    // ⭐ F-13 — 메뉴바 수동 확인. 원본 실측 배치(Settings…와 About 사이, §3.3)를
    // 그대로 따른다. `.app` 번들 밖(`tauri dev`)에서는 확인할 대상 자체가 없으므로
    // Relaunch 와 같은 기준으로 비활성화한다(위임 지시 "수동 확인은 메뉴바").
    let check_for_updates_item = MenuItem::with_id(
        handle,
        menu_ids::CHECK_FOR_UPDATES,
        catalog.get(menu_ids::CHECK_FOR_UPDATES),
        bundle::is_running_from_app_bundle(),
        None::<&str>,
    )?;
    // ⭐ 결정(위임 지시서): 별도 About 창을 새로 만들지 않는다 — 설정 창 General
    // 탭에 이미 About 정보 행(번들 ID·로그 경로·설정 파일 경로, `settings.general.
    // about.*`, 이슈 #13)이 있다. 이 메뉴 항목은 설정 창을 여는 것으로 구현한다.
    // ⚠️ 알려진 한계: `ui/settings.html` 이 이 위임과 동시에 다른 세션이 편집
    // 중이라, "General 탭으로 자동 전환 + About 섹션 자동 펼침"까지는 여기서
    // 배선하지 않았다 — 사용자가 창이 열리면 General 탭과 버전 버튼을 직접
    // 눌러야 한다. 후속 과제로 남긴다(최종 보고 참고).
    // ⭐⭐ 2026-09-02 정정(이슈 #87) — 사용자 피드백("About 이 설정 창을 연다")이
    // 이 결정을 뒤집었다. About 은 이제 **독립 정보 창**(`ui/about.html` +
    // `show_about_window`)을 연다. 위 주석은 결정 내역의 역사 기록으로 남긴다.
    let about_item = MenuItem::with_id(
        handle,
        menu_ids::ABOUT,
        catalog.get(menu_ids::ABOUT),
        true,
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
    // ⭐ 이슈 #77 — Synthesize Caps Lock Remap 은 설정 화면 General 탭의
    // Advanced 섹션으로 이동했다. `Advanced ▸` 서브메뉴에는 Relaunch 만 남는다.
    let advanced_menu = Submenu::with_id_and_items(
        handle,
        menu_ids::ADVANCED,
        catalog.get(menu_ids::ADVANCED),
        true,
        &[&relaunch_item],
    )?;

    let sep_bottom = PredefinedMenuItem::separator(handle)?;
    let quit_item = MenuItem::with_id(
        handle,
        menu_ids::QUIT,
        catalog.get(menu_ids::QUIT),
        true,
        None::<&str>,
    )?;

    let mut items: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = Vec::with_capacity(10);
    let caps_status_sep = PredefinedMenuItem::separator(handle)?;
    if let Some(item) = caps_status_item.as_ref() {
        items.push(item);
        items.push(&caps_status_sep);
    }
    items.extend_from_slice(&[
        &ignore_item,
        &sep_top,
        &settings_item,
        &check_for_updates_item,
        &about_item,
        &advanced_menu,
        &sep_bottom,
        &quit_item,
    ]);
    let menu = Menu::with_items(handle, &items)?;

    Ok((menu, ignore_item))
}

/// `unauthorizedMenu`(§3.1) — 권한이 없을 때 메뉴 전체를 이 2항목으로 교체한다.
fn build_unauthorized_menu(
    handle: &tauri::AppHandle,
    catalog: &Catalog,
) -> tauri::Result<Menu<Wry>> {
    // 상태 안내는 클릭해도 아무 일도 일어나지 않는 비활성 항목이다.
    let status_item = MenuItem::new(
        handle,
        catalog.get("menu.unauthorized.title"),
        false,
        None::<&str>,
    )?;
    let authorize_item = MenuItem::with_id(
        handle,
        menu_ids::AUTHORIZE,
        catalog.get(menu_ids::AUTHORIZE),
        true,
        None::<&str>,
    )?;
    Menu::with_items(handle, &[&status_item, &authorize_item])
}

/// 트레이(`NSStatusItem`)를 만들고 `AppState` 에 손잡이를 채운다. `setup()` 안에서
/// 한 번만 불린다.
fn setup_tray(handle: &tauri::AppHandle, state: &Arc<AppState>) -> tauri::Result<()> {
    let catalog = state.catalog.load_full();
    let catalog = catalog.as_ref();

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
    let hide_menu_bar_icon: bool = store
        .get(settings_keys::GENERAL_HIDE_MENU_BAR_ICON)
        .unwrap_or(false);
    drop(store);

    let (normal_menu, ignore_item) =
        build_normal_menu(handle, catalog, None, false, caps_lock_kernel_map_missing(state))?;
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
        // ⭐ 이슈 #78 계측 — Bartender 가 숨김·복원할 때 우리 코드가 개입하는지
        // 시간적 상관으로 배제하기 위한 호출 기록이다. 빈도가 낮아 `info` 로
        // 남긴다(계획 §2 D2 — 원인 확정 후에도 존치).
        tracing::info!(
            visible = false,
            source = "boot",
            "tray set_visible called"
        );
        if let Err(e) = tray.set_visible(false) {
            tracing::warn!(error = %e, "failed to apply tray icon hidden state");
        }
    }

    *state.tray.lock().unwrap() = Some(tray);
    *state.ignore_item.lock().unwrap() = Some(ignore_item);
    *state.normal_menu.lock().unwrap() = Some(normal_menu);
    *state.unauthorized_menu.lock().unwrap() = Some(unauthorized_menu);

    Ok(())
}

/// ⭐ 이슈 #78 — 트레이(NSStatusItem) 상태 스냅샷을 1초 간격으로 남기는 계측
/// 폴러. `setup()` 이 트레이를 만든 직후 한 번만 띄운다.
///
/// Bartender 설치 실기기에서 판정해야 할 핵심 질문(계획 §2 D3)은 "Bartender 가
/// 아이콘을 숨길 때 NSStatusItem 이 **제거되는가**(`status_item_exists=false`),
/// **숨김 처리되는가**(`=true`)다. 볼 수 있는 정보는 `tray-icon` 의
/// `ns_status_item()` 접근자와 그 `button.image` 유무뿐이다 — 둘 다 **메인 스레드
/// 전용**이라 `run_on_main_thread` 로 디스패치한다(리스크 #6 — `apply_tray_menu_
/// for_permission` 의 선례). 폴링 스레드는 큐잉만 하고 즉시 돌아오므로
/// "백그라운드 스레드에서 메인 스레드를 동기적으로 기다리지 마라" 규칙을 지킨다.
///
/// 로그 레벨은 `debug` 다 — 1초 간격이라 `info` 로 남기면 로그 파일을 오염시킨다
/// (계획 §2 D2 ⚠️·리스크 #3). `ULTRAKEY_LOG=debug` 로 켰을 때만 기록된다.
fn spawn_tray_snapshot_poller(handle: tauri::AppHandle<Wry>, state: Arc<AppState>) {
    thread::Builder::new()
        .name("ultrakey-tray-poll".to_string())
        .spawn(move || loop {
            let state_for_closure = state.clone();
            let dispatched = handle.run_on_main_thread(move || {
                let Some(tray) = state_for_closure.tray.lock().unwrap().clone() else {
                    return;
                };
                let _ = tray.with_inner_tray_icon(|inner| {
                    if let Some(status_item) = inner.ns_status_item() {
                        // 메인 스레드 안이므로 `MainThreadMarker::new()` 는
                        // 항상 `Some` — `button(mtm)` 호출 조건을 그대로
                        // 반영한다(marker 를 못 얻는 경우는 존재하지 않는다).
                        let has_button_image = MainThreadMarker::new()
                            .and_then(|mtm| status_item.button(mtm))
                            .map(|button| button.image().is_some())
                            .unwrap_or(false);
                        tracing::debug!(
                            status_item_exists = true,
                            button_image_exists = has_button_image,
                            "tray status_item snapshot"
                        );
                    } else {
                        tracing::debug!(
                            status_item_exists = false,
                            "tray status_item snapshot"
                        );
                    }
                });
            });
            if let Err(e) = dispatched {
                tracing::error!(error = %e, "failed to dispatch tray status snapshot to the main thread");
            }
            thread::sleep(Duration::from_secs(1));
        })
        .expect("트레이 상태 스냅샷 폴러 스레드 생성 실패");
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
///
/// `reason` 은 교체를 유발한 트리거 출처("permission"=권한 전이 / "language"=언어
/// 변경)다 — ⭐ 이슈 #78 계측이 깜빡임 발생 시각과 `set_menu` 교체 시각의
/// **시간적 상관**을 보기 위한 레이블이다(계획 §2 D2). `set_menu` 는 NSStatusItem 을
/// 유지한 채 메뉴만 교체하므로 아이콘 리페인트를 직접 유발하지 않는다(`tray-icon`
/// 0.24.2 실측) — 이 계측은 교체 시점과 Bartender 의 메뉴 추적이 겹치는 타이밍
/// 문제(H2)를 판정하는 재료다.
fn apply_tray_menu_for_permission(
    handle: &tauri::AppHandle,
    state: &Arc<AppState>,
    granted: bool,
    reason: &'static str,
) {
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
            tracing::info!(
                reason,
                granted,
                "tray set_menu replaced"
            );
            if let Err(e) = tray.set_menu(Some(menu)) {
                tracing::warn!(error = %e, "failed to replace tray menu");
            }
        }
    });
    if let Err(e) = dispatched {
        tracing::error!(error = %e, "failed to dispatch tray menu replacement to the main thread");
    }
}

/// ⭐ A-3 3단계(D6, 이슈 #39, §3.1.2-a) — `general.language` 가 바뀐 뒤 트레이
/// 메뉴를 새 카탈로그로 다시 만든다. `setup_tray()` 가 부팅 시 쓰는 것과 같은
/// 조립 함수(`build_normal_menu`/`build_unauthorized_menu`)와, 권한 전이 때
/// 이미 쓰는 교체 절차(`apply_tray_menu_for_permission` 의 메인 스레드 디스패치)
/// 를 그대로 재사용한다 — 새 경로를 만들지 않는다. `TrayIcon` 자체는 다시
/// 만들지 않고 메뉴 두 벌만 갈아 끼운다.
fn rebuild_tray_menu(handle: &tauri::AppHandle, state: &Arc<AppState>) {
    let catalog = state.catalog.load_full();
    let catalog = catalog.as_ref();

    // ⛔ 최전면 앱 이름은 여기서 다시 조회하지 않는다 — `refresh_ignore_menu_item`
    // 이 그 일을 전담한다. 새 `ignore_item` 을 조립한 뒤 곧바로 그 함수를 한 번
    // 더 불러 실제 최전면 앱 라벨로 채운다.
    let (normal_menu, ignore_item) =
        match build_normal_menu(handle, catalog, None, false, caps_lock_kernel_map_missing(state)) {
            Ok(v) => v,
            Err(e) => {
                tracing::error!(error = %e, "failed to rebuild tray normal menu after a language change");
                return;
            }
        };
    let unauthorized_menu = match build_unauthorized_menu(handle, catalog) {
        Ok(v) => v,
        Err(e) => {
            tracing::error!(error = %e, "failed to rebuild tray unauthorized menu after a language change");
            return;
        }
    };

    *state.normal_menu.lock().unwrap() = Some(normal_menu);
    *state.unauthorized_menu.lock().unwrap() = Some(unauthorized_menu);
    *state.ignore_item.lock().unwrap() = Some(ignore_item);

    // 지금 보여야 하는 것이 정상 메뉴인지 unauthorized 메뉴인지는 권한
    // 모니터의 현재 상태로 판정한다 — `apply_tray_menu_for_permission` 이
    // 권한 전이 때 쓰는 것과 같은 신호.
    let granted = state
        .monitor
        .lock()
        .ok()
        .and_then(|m| m.as_ref().map(|m| m.state()))
        .map(|s| s == PermissionState::Granted)
        .unwrap_or(false);
    apply_tray_menu_for_permission(handle, state, granted, "language");
    refresh_ignore_menu_item(state);
}

/// 트레이 메뉴 클릭 처리. `TrayIconBuilder::on_menu_event` 콜백은 항상 메인
/// 스레드에서 불린다(AppKit 이 메뉴 클릭을 메인 런루프에서 전달한다) — 메뉴/트레이
/// 조작을 여기서 직접(비동기 디스패치 없이) 해도 안전하다.
fn handle_menu_event(app: &tauri::AppHandle, state: &Arc<AppState>, event: MenuEvent) {
    // `MenuId` 는 `pub struct MenuId(pub String)` 다(muda) — `.0.as_str()` 로 직접
    // 꺼내 쓰면 `AsRef` 구현 다중화로 인한 타입 추론 모호성 여지가 없다.
    match event.id().0.as_str() {
        menu_ids::IGNORE_APP => on_menu_ignore_app(state),
        // ⭐ 이슈 #87 — About 이 설정 창을 여는 라우팅(`SETTINGS | ABOUT`)을 분리
        // 했다. 이제 About 은 독립 정보 창(`show_about_window`)을 연다.
        menu_ids::SETTINGS => show_settings_window(app),
        menu_ids::ABOUT => show_about_window(app, state),
        menu_ids::CHECK_FOR_UPDATES => on_menu_check_for_updates(app, state),
        menu_ids::RELAUNCH => on_menu_relaunch(app, state),
        menu_ids::QUIT => on_menu_quit(app, state),
        menu_ids::AUTHORIZE => show_modal(app),
        other => tracing::debug!(id = other, "unknown menu event id"),
    }
}

/// `Ignore <앱>` 클릭 — `AppGateController::toggle_front_app()` 을 호출하고, 결과
/// 목록을 `general.disabledApps` 로 즉시 원자적 write-through 한다(F-15 §3.1.1 —
/// 종료 시점 flush 에 기대지 않는다).
fn on_menu_ignore_app(state: &Arc<AppState>) {
    let now_disabled = state.gate_controller.toggle_front_app();
    tracing::info!(now_disabled, "Ignore <app> toggled");
    persist_disabled_apps(state);
    refresh_ignore_menu_item(state);
}

fn persist_disabled_apps(state: &Arc<AppState>) {
    let bundle_ids = state.gate_controller.disabled_apps();
    let mut store = state.store.lock().unwrap();
    if let Err(e) = store.set(settings_keys::GENERAL_DISABLED_APPS, &bundle_ids) {
        tracing::error!(error = %e, "failed to save general.disabledApps");
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
    let catalog = state.catalog.load_full();
    let _ = item.set_text(ignore_menu_text(&catalog, front_app.as_ref()));
    let _ = item.set_enabled(front_app.is_some());
    let _ = item.set_checked(disabled);

    // ⭐ 이슈 #78 계측(H3 판정 재료) — 최전면 앱 전환·언어 변경·Ignore 토글마다
    // 항목을 갱신한 사실과 그 시점의 최전면 앱을 남긴다. `NSMenuItem` 수준의
    // 변경이라 NSStatusItem 의 아이콘·가시성과는 무관하지만(`tray-icon` 실측),
    // Bartender 가 메뉴 항목 변경까지 추적해 아이콘을 다시 그리는지 시간적
    // 상관으로 판정한다. 앱 전환 빈도로 찍히지만 `debug` 로 두면 판정 로그가
    // 보이지 않아 `info` 를 유지한다(계획 §2 D2 ⚠️ — 원인 확정 후에도 존치).
    tracing::info!(
        front_app = ?front_app.as_ref().map(|a| a.name.as_str()),
        disabled,
        "tray ignore menu item refreshed"
    );
}

/// `Relaunch` 클릭 — 현재 실행 파일을 `open -n -b <bundle-id>` 로 새로 띄우고
/// 자신은 정상 종료 절차(`shutdown_and_exit`)를 밟는다. `.app` 번들 밖에서
/// 실행 중이면 메뉴 항목 자체가 비활성이라(`build_normal_menu`) 여기 도달하지
/// 않는 것이 정상이지만, 방어적으로 한 번 더 확인한다.
fn on_menu_relaunch(app: &tauri::AppHandle, state: &Arc<AppState>) {
    if !bundle::is_running_from_app_bundle() {
        tracing::warn!("running outside an .app bundle; skipping relaunch");
        return;
    }
    let Some(bundle_id) = bundle::bundle_identifier() else {
        tracing::warn!("failed to get bundle id; skipping relaunch");
        return;
    };
    tracing::info!(
        bundle_id,
        "relaunch requested; starting a new instance with open -n -b"
    );
    match std::process::Command::new("open")
        .args(["-n", "-b", &bundle_id])
        .spawn()
    {
        Ok(_) => shutdown_and_exit(app, state),
        Err(e) => {
            tracing::error!(error = %e, "failed to run open for relaunch; keeping the existing instance")
        }
    }
}

/// `Quit Ultrakey` 클릭.
fn on_menu_quit(app: &tauri::AppHandle, state: &Arc<AppState>) {
    tracing::info!("Quit Ultrakey selected; starting graceful shutdown");
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
        match ev {
            SystemEvent::FrontAppChanged(ident) => {
                on_front_app_changed(&handle_for_events, &state_for_events, ident);
            }
            // ⭐ F-06(§5 항목 7) — 절전 복귀 직후 `MTDevice` 핸들이 만료했을
            // 가능성(비공개 API 라 문서화되지 않았다, §9 #7)에 대비해 리스너 세션을
            // 전부 다시 만든다. 리스너가 없으면(격하·비활성) 조용히 무시된다.
            SystemEvent::DidWake => {
                if let Ok(guard) = state_for_events.trackpad.lock() {
                    if let Some(listener) = guard.as_ref() {
                        listener.restart_sessions();
                    }
                }
            }
            _ => {}
        }
    }));
    *state.system_event_observer.lock().unwrap() = Some(observer);
}

/// 이슈 #68 — 두 번째 프로세스가 보내는 "설정창 띄워라" 분산 알림 구독.
/// `AppState::show_settings_observer` 에 손잡이를 보관해 앱 생애주기 내내
/// 살려 둔다(드롭되면 구독이 해지된다 — `setup_front_app_tracking` 과 같은 규약).
fn setup_show_settings_observer(handle: &tauri::AppHandle, state: &Arc<AppState>) {
    let handle_for_signal = handle.clone();
    let observer =
        single_instance::observe_show_settings_requests(Box::new(move || {
            tracing::info!("show-settings request from a second instance received");
            show_settings_window(&handle_for_signal);
        }));
    *state.show_settings_observer.lock().unwrap() = Some(observer);
}

/// `NSWorkspaceDidActivateApplicationNotification` 수신 — 게이트를 갱신하고
/// `Ignore <앱>` 라벨을 되맞춘다.
///
/// ⚠️ 이 콜백은 알림이 도착하는 스레드(관례상 메인 스레드, `ultrakey_platform::
/// workspace` 모듈 문서)에서 불린다 — 메뉴 항목 갱신을 직접(비동기 디스패치 없이)
/// 해도 안전하다. 판정(`bundle_id ∈ disabledApps`) 자체는 `AppGateController::
/// set_front_app` 이 즉시 `AtomicBool` 에 게시한다(`docs/dev/architecture.md`
/// §2.3) — 콜백 임계 경로(탭 스레드)는 이 함수와 전혀 만나지 않는다.
fn on_front_app_changed(
    _handle: &tauri::AppHandle,
    state: &Arc<AppState>,
    ident: Option<AppIdentity>,
) {
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

/// `Launch on login` 체크박스 — `login_item::set_enabled` 로 OS 에 등록/해제한 뒤
/// **결과를 확인하고** 거울(`general.launchOnLogin`)에는 실제로 도달한 상태를
/// 기록한다.
///
/// ⭐ B-3(이슈 #39, `menu-bar-and-lifecycle.md` §3.5-a 결함 ②) — 예전 순서는
/// OS 호출이 실패해도 거울에 사용자 의도(`on`)를 먼저 썼다. 그러면 실패한 상태가
/// `true` 로 굳어 "켜 놨는데 안 된다"가 영구화된다. 지금은: 1) OS 호출 2)
/// `login_item::status()` 로 **실제 도달한 상태**를 다시 읽는다(`Ok` 를 받았다는
/// 것과 로그인 시 실제로 뜬다는 것은 다르다) 3) 거울에는 그 실제 상태를 쓴다
/// 4) `RequiresApproval` 은 재시도하지 않고 전용 문구로 알린다.
///
/// ⚠️ `login_item::set_enabled` 는 상한 있는 재시도(§5 항목 3, 최대 5회·0.2초
/// 간격 — 최악 약 0.8초)로 **동기 블로킹**한다. Tauri 커맨드 핸들러는 메인
/// 스레드가 아닌 별도 스레드에서 실행되므로(Tauri 의 IPC 커맨드 디스패치 자체가
/// 그렇게 되어 있다) 여기서 블로킹해도 UI 는 멎지 않는다 — 다만 이 함수를 메인
/// 스레드에서 직접 부르는 새 경로가 생기면 반드시 스레드를 분리해야 한다
/// (`login_item` 모듈 문서 참고).
fn set_launch_on_login_internal(state: &Arc<AppState>, on: bool) -> Result<(), String> {
    let op_result = login_item::set_enabled(on);
    if let Err(e) = &op_result {
        tracing::error!(error = %e, on, "failed to register/unregister login item");
    }

    // OS 조작 뒤 상태를 다시 읽는다 — 이것이 §3.5-a 가 확정한 "정본은 OS" 다.
    let status = login_item::status();
    {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        if let Err(e) = store.set(settings_keys::GENERAL_LAUNCH_ON_LOGIN, &status.is_active()) {
            tracing::error!(error = %e, "failed to save general.launchOnLogin");
        }
    }

    if status == login_item::LoginItemStatus::RequiresApproval {
        // ⛔ 재등록을 시도하지 않는다 — 사용자만 되돌릴 수 있다(§3.5-a 결정 4).
        tracing::warn!(
            on,
            status = status.as_log_str(),
            "login item requires user approval in System Settings; not retrying"
        );
        return Err(state
            .catalog
            .load()
            .get("menu.launch_on_login.requires_approval")
            .to_string());
    }

    if op_result.is_err() || status.is_active() != on {
        tracing::error!(
            on,
            status = status.as_log_str(),
            reached = status.is_active(),
            "login item did not reach the requested state"
        );
        return Err(state
            .catalog
            .load()
            .get("menu.launch_on_login.failed")
            .to_string());
    }

    Ok(())
}

/// ⭐ B-2(이슈 #39, `menu-bar-and-lifecycle.md` §3.5-a 결함 ③) — 기동 시 거울과
/// OS 정본을 맞춘다. 이 프로젝트가 워크트리의 `target/` 빌드 디렉터리에서
/// 실행되므로, 등록된 절대 경로가 그 디렉터리를 가리킨다(BTM 실측). 그
/// 디렉터리가 사라지면 로그인 시 아무것도 뜨지 않는데, 이 재조정이 없으면
/// 아무도 다시 등록해 주지 않는다.
///
/// ⚠️ `login_item::set_enabled` 는 최대 약 0.8초 블로킹한다 — 별도 스레드에서
/// 돌려 `setup()` 을 늦추지 않는다.
fn reconcile_login_item_at_boot(state: &Arc<AppState>) {
    let mirror: bool = {
        let store = state.store.lock().unwrap();
        store
            .get(settings_keys::GENERAL_LAUNCH_ON_LOGIN)
            .unwrap_or(false)
    };
    let state = state.clone();
    thread::spawn(move || {
        let status = login_item::status();
        tracing::info!(
            mirror,
            status = status.as_log_str(),
            "reconciling login item mirror against OS state at boot"
        );

        match status {
            login_item::LoginItemStatus::RequiresApproval => {
                // ⛔ 재등록을 시도하지 않는다 — 사용자만 되돌릴 수 있다.
                tracing::warn!(
                    "login item requires user approval in System Settings > General > \
                     Login Items; not attempting to re-register at boot"
                );
            }
            login_item::LoginItemStatus::NotRegistered | login_item::LoginItemStatus::NotFound
                if mirror =>
            {
                tracing::warn!(
                    status = status.as_log_str(),
                    "the launch-on-login mirror says enabled but the OS has no active \
                     registration; re-registering at the current executable path"
                );
                match login_item::set_enabled(true) {
                    Ok(()) => tracing::info!("login item re-registered at boot"),
                    Err(e) => {
                        tracing::error!(error = %e, "failed to re-register login item at boot")
                    }
                }
            }
            login_item::LoginItemStatus::Enabled if !mirror => {
                tracing::info!(
                    "OS reports the login item enabled but the mirror was false; \
                     syncing the mirror to true"
                );
                let mut store = state.store.lock().unwrap();
                if let Err(e) = store.set(settings_keys::GENERAL_LAUNCH_ON_LOGIN, &true) {
                    tracing::error!(
                        error = %e,
                        "failed to sync the general.launchOnLogin mirror at boot"
                    );
                }
            }
            _ => {}
        }
    });
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
        // ⭐ 이슈 #78 계측 — 트리거 출처는 "toggle"(General 탭 `Hide menu bar icon`
        // 체크박스)이다. Bartender 의 숨김·복원과 우리 `set_visible` 의 시각이
        // 겹치는지 상관을 보기 위한 기록(계획 §2 D2 — 원인 확정 후에도 존치).
        tracing::info!(
            visible = !on,
            source = "toggle",
            "tray set_visible called"
        );
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

    /// 이슈 #65 Phase 1 리뷰 "테스트 요구" — `Granted` 만 엔진을 살려 둔다.
    #[test]
    fn should_stop_engine_for_only_keeps_the_engine_alive_when_granted() {
        assert!(!should_stop_engine_for(PermissionState::Granted));
        assert!(should_stop_engine_for(PermissionState::Denied));
        assert!(should_stop_engine_for(PermissionState::OutOfSync));
        assert!(should_stop_engine_for(PermissionState::Unknown));
    }

    /// `is_known_tab` 이 6개 탭을 전부 알고, 모르는 이름은 거부한다.
    #[test]
    fn is_known_tab_knows_all_six_tabs() {
        for tab in [
            "seek",
            "hyperkey",
            "presets",
            "korean",
            "keyboards",
            "general",
        ] {
            assert!(is_known_tab(tab), "`{tab}` 은 KNOWN_TABS 에 있어야 한다");
        }
        assert!(!is_known_tab("bogus"));
    }

    /// A-2(이슈 #39) — 저장된 값이 없으면 `None`("시스템 설정 따름" 경로를 탄다).
    #[test]
    fn resolve_stored_language_is_none_when_key_absent() {
        let store = SettingsStore::in_memory();
        assert_eq!(resolve_stored_language(&store), None);
    }

    /// A-2 — 저장된 값이 있으면 그 로케일이 선택된다.
    #[test]
    fn resolve_stored_language_returns_the_stored_locale() {
        let mut store = SettingsStore::in_memory();
        store
            .set(settings_keys::GENERAL_LANGUAGE, &"ko")
            .unwrap();
        assert_eq!(resolve_stored_language(&store), Some(Locale::Ko));
    }

    /// A-2 ⚠️ — 알 수 없는 값(손으로 고친 설정 파일 등)이면 실패시키지 않고
    /// `None`(시스템 로케일 폴백)으로 돌아간다.
    #[test]
    fn resolve_stored_language_falls_back_to_none_for_unknown_value() {
        let mut store = SettingsStore::in_memory();
        store
            .set(settings_keys::GENERAL_LANGUAGE, &"klingon")
            .unwrap();
        assert_eq!(resolve_stored_language(&store), None);
    }

    /// ⭐ **회귀 방지 — 프론트의 탭 목록과 Rust 의 탭 화이트리스트가 어긋나지 않는다.**
    ///
    /// F-17(이슈 #28) 실기기 검증에서 실제로 터진 결함이다: `settings.html` 이
    /// `Keyboards` 탭을 `TABS` 에 넣었는데 (당시 `tab_window_size`, 지금은
    /// `KNOWN_TABS`) 에 대응 항목을 넣지 않아, `settings_set_tab` 이 `알 수 없는
    /// 탭: keyboards` 로 거부하고 **설정 창 전체가 오류 화면으로 죽었다.** 이슈
    /// #32 Phase 1 에서 탭별 창 크기 테이블(`tab_window_size`)은 없앴지만 — 탭
    /// 전환이 더 이상 창을 리사이즈하지 않기 때문이다 — 이 화이트리스트와 이
    /// 회귀 방지 테스트는 그대로 남는다. 크기와 무관하게 "프런트가 아는 탭을
    /// Rust 도 아는가"는 여전히 검증해야 한다.
    ///
    /// ⚠️ `tests/frontend_wiring.rs` 는 HTML **텍스트**만 검사하므로 이 어긋남을
    /// 잡을 수 없다 — 한쪽은 JS 배열이고 다른 쪽은 Rust 슬라이스다. 두 목록을
    /// 실제로 대조하는 것은 이 테스트뿐이다(같은 크레이트 안이라 private
    /// 상수를 참조할 수 있다). **양방향**으로 검사한다 — `TABS` 에 있는데
    /// `KNOWN_TABS` 에 없는 탭(이슈 #28 의 결함)뿐 아니라, `KNOWN_TABS` 에
    /// 있는데 `TABS` 에 없는 죽은 항목도 잡는다.
    #[test]
    fn every_tab_in_settings_html_is_a_known_tab() {
        let html = include_str!("../ui/settings.html");
        let line = html
            .lines()
            .find(|l| l.contains("const TABS"))
            .expect("settings.html 에 `const TABS = [...]` 선언이 있어야 한다");
        let inside = line
            .split_once('[')
            .and_then(|(_, r)| r.split_once(']'))
            .map(|(m, _)| m)
            .expect("`const TABS` 가 대괄호 배열이어야 한다");

        let tabs: Vec<&str> = inside
            .split(',')
            .map(|t| t.trim().trim_matches(['"', '\'']))
            .filter(|t| !t.is_empty())
            .collect();

        assert!(
            tabs.len() >= 6,
            "탭이 6개 미만이다 — 파싱이 깨졌을 가능성이 크다: {tabs:?}"
        );
        for tab in &tabs {
            assert!(
                is_known_tab(tab),
                "settings.html 의 TABS 에 있는 `{tab}` 탭이 KNOWN_TABS 에 없다 — \
                 settings_set_tab 이 그 탭을 거부해 설정 창이 죽는다"
            );
        }
        for tab in KNOWN_TABS {
            assert!(
                tabs.contains(tab),
                "KNOWN_TABS 에 있는 `{tab}` 탭이 settings.html 의 TABS 에 없다 — \
                 죽은 화이트리스트 항목이다"
            );
        }
    }

    // restored_window_size() — 이슈 #32 Phase 1, F-15 §3.1 "부재 = 기본값".
    #[test]
    fn restored_window_size_needs_both_dimensions() {
        assert_eq!(
            restored_window_size(Some(900.0), Some(700.0)),
            Some((900, 700))
        );
        assert_eq!(restored_window_size(Some(900.0), None), None);
        assert_eq!(restored_window_size(None, Some(700.0)), None);
        assert_eq!(restored_window_size(None, None), None);
    }

    #[test]
    fn restored_window_size_rejects_out_of_range_values() {
        assert_eq!(restored_window_size(Some(100.0), Some(700.0)), None); // 너비 미달
        assert_eq!(restored_window_size(Some(900.0), Some(100.0)), None); // 높이 미달
        assert_eq!(restored_window_size(Some(9000.0), Some(700.0)), None); // 너비 초과
        assert_eq!(restored_window_size(Some(900.0), Some(9000.0)), None); // 높이 초과
                                                                           // 경계값은 포함이다.
        assert_eq!(
            restored_window_size(Some(320.0), Some(240.0)),
            Some((320, 240))
        );
        assert_eq!(
            restored_window_size(Some(6000.0), Some(6000.0)),
            Some((6000, 6000))
        );
    }

    /// `tauri.conf.json` 의 `settings` 창 기본 크기가 `SETTINGS_WINDOW_DEFAULT`
    /// 와 어긋나지 않는지 재확인한다 — 값 자체는 이 상수의 doc 주석이 근거를
    /// 댄다(2026-08-30 실측), 여기서는 두 값이 드리프트하지 않는지만 본다.
    #[test]
    fn settings_window_default_matches_tauri_conf() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json"))
            .expect("tauri.conf.json 파싱 실패");
        let windows = conf["app"]["windows"]
            .as_array()
            .expect("tauri.conf.json 에 app.windows 배열이 있어야 한다");
        let settings_window = windows
            .iter()
            .find(|w| w["label"] == "settings")
            .expect("`settings` 라벨 창이 있어야 한다");
        let (width, height) = SETTINGS_WINDOW_DEFAULT;
        assert_eq!(settings_window["width"].as_u64(), Some(width as u64));
        assert_eq!(settings_window["height"].as_u64(), Some(height as u64));
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
        assert!(!key_affects_modifier_rules(
            keys::HYPERKEY_MOUSE_APPLY_CLICK
        ));
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
            &SeekSettings::default(),
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
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
        let presets = PresetSettings {
            caps_space_enter: true,
            quick_press_duration_ms: 500,
            ..PresetSettings::default()
        };

        let config = build_engine_config(
            &hyperkey,
            &presets,
            &KoreanSettings::default(),
            &SeekSettings::default(),
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
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
            &SeekSettings::default(),
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
            &SettingsStore::in_memory(),
        );
        assert_eq!(none_needed.caps_lock_alias, None);

        let needs_alias = PresetSettings {
            caps_space_enter: true,
            ..PresetSettings::default()
        };
        let with_alias = build_engine_config(
            &hyperkey,
            &needs_alias,
            &KoreanSettings::default(),
            &SeekSettings::default(),
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
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
            &SeekSettings::default(),
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
            &SettingsStore::in_memory(),
        );
        assert_eq!(with_synthesize.caps_lock_alias, None);
    }

    /// ⭐ F-01 — `Remap key to Seek: caps lock` 만으로도(다른 D-1 이유가 전혀 없어도)
    /// F18 alias 가 반드시 켜진다. `synthesize_caps_lock_remap` 이 켜지면 이 요구도
    /// 무시된다(기존 D-1 의미와 같은 override).
    #[test]
    fn build_engine_config_forces_caps_lock_alias_when_seek_remap_key_is_caps_lock() {
        let hyperkey = HyperkeySettings::default();
        let presets = PresetSettings::default();

        let seek_targets_caps_lock = SeekSettings {
            remap_key: Some(SourceKey::CapsLock),
            ..SeekSettings::default()
        };
        let with_alias = build_engine_config(
            &hyperkey,
            &presets,
            &KoreanSettings::default(),
            &seek_targets_caps_lock,
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
            &SettingsStore::in_memory(),
        );
        assert_eq!(
            with_alias.caps_lock_alias,
            Some(KeyCode::F18),
            "Remap key to Seek: caps lock 이면 hold 모드 릴리즈를 위해 D-1 이 반드시 켜져야 한다"
        );
        // ⭐ 다른 소스 키(F13 등)를 골랐을 때는 이 요구가 성립하지 않는다.
        let seek_targets_f13 = SeekSettings {
            remap_key: Some(SourceKey::F13),
            ..SeekSettings::default()
        };
        let without_alias = build_engine_config(
            &hyperkey,
            &presets,
            &KoreanSettings::default(),
            &seek_targets_f13,
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
            &SettingsStore::in_memory(),
        );
        assert_eq!(without_alias.caps_lock_alias, None);

        // ⭐ Advanced 토글이 켜지면 이 요구도 무시된다(기존 D-1 override 의미 유지).
        let synthesize_on = PresetSettings {
            synthesize_caps_lock_remap: true,
            ..PresetSettings::default()
        };
        let overridden = build_engine_config(
            &hyperkey,
            &synthesize_on,
            &KoreanSettings::default(),
            &seek_targets_caps_lock,
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
            &SettingsStore::in_memory(),
        );
        assert_eq!(overridden.caps_lock_alias, None);
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
            &SeekSettings::default(),
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
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
            .set(
                &keys::per_device_key_remap_rows(keys::PER_DEVICE_COMMON_SCOPE),
                &serde_json::json!([]),
            )
            .unwrap();
        store
            .set(keys::PER_DEVICE_MANAGED, &serde_json::json!({}))
            .unwrap();

        let config = build_engine_config(
            &HyperkeySettings::default(),
            &PresetSettings::default(),
            &KoreanSettings::default(),
            &SeekSettings::default(),
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
            &store,
        );

        assert!(config
            .per_device_values
            .contains_key(&keys::per_device_key_remap_rows(
                keys::PER_DEVICE_COMMON_SCOPE
            )));
        assert!(!config
            .per_device_values
            .contains_key(keys::PER_DEVICE_MANAGED));
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

    // ⭐ 이슈 #31 ② — validate_per_device_function_key_value() 는 기능 2 슬롯의
    // 문자열 값만 카탈로그 대조로 거부한다. 키 변환 세트(배열)·`null`(끔)은 이
    // 검증의 대상이 아니다 — §3.3 의 끔/부재 구분을 건드리면 안 된다.
    #[test]
    fn validate_per_device_function_key_value_checks_only_function_key_strings() {
        // 카탈로그에 있는 새 id — 통과.
        assert!(validate_per_device_function_key_value(
            "perDevice.all.functionKeys.f1",
            &serde_json::Value::String("consumer.mute".to_string())
        )
        .is_ok());

        // 카탈로그에 없는 문자열 — 거부.
        assert!(validate_per_device_function_key_value(
            "perDevice.all.functionKeys.f1",
            &serde_json::Value::String("nope.not_a_key".to_string())
        )
        .is_err());

        // 옛 SystemFunction variant 이름 — 마이그레이션 경로로 여전히 풀리므로 통과.
        assert!(validate_per_device_function_key_value(
            "perDevice.all.functionKeys.f1",
            &serde_json::Value::String("Mute".to_string())
        )
        .is_ok());

        // null(명시적 끔)은 문자열이 아니므로 이 검증을 그냥 통과한다.
        assert!(validate_per_device_function_key_value(
            "perDevice.all.functionKeys.f1",
            &serde_json::Value::Null
        )
        .is_ok());

        // 기능 1(키 변환 세트, 배열)은 이 함수의 관심사가 아니다 — 카탈로그에
        // 없는 문자열이 배열 안에 있어도 이 함수 자체는 통과시킨다(모양 검증은
        // validate_per_device_key 의 몫).
        assert!(validate_per_device_function_key_value(
            "perDevice.all.keyRemap.rows",
            &serde_json::json!([{"from": "CapsLock", "to": "F18"}])
        )
        .is_ok());
    }

    // ⭐ 이슈 #46 — settings_copy_common_to_device 파라미터 검증(계획 §6.2). 커맨드
    // 본체가 State 를 받아 직접 호출하기 어려워, 검증 판정을 순수 함수
    // `validate_copy_common_to_device_args` 로 뽑아 `validate_per_device_key` 와
    // 같은 관례대로 인라인 테스트한다.
    #[test]
    fn settings_copy_common_refuses_device_all() {
        assert!(
            validate_copy_common_to_device_args(keys::PER_DEVICE_COMMON_SCOPE, "keyRemap").is_err(),
            "공통 계층('all')에는 복사 원천이 없다 — 거부해야 한다"
        );
        assert!(
            validate_copy_common_to_device_args("all", "functionKeys").is_err(),
            "공통 계층('all')으로의 복사는 방향이 성립하지 않는다 — 거부해야 한다"
        );
    }

    #[test]
    fn settings_copy_common_refuses_bad_device_and_feature() {
        // DeviceId::parse 실패 — 콜론 구분자·16진 검증을 통과하지 못하는 스코프.
        assert!(
            validate_copy_common_to_device_args("notADevice", "keyRemap").is_err(),
            "디바이스 id 파싱 실패는 거부해야 한다"
        );
        // 알 수 없는 feature — 화이트리스트 {"keyRemap","functionKeys"} 밖.
        assert!(
            validate_copy_common_to_device_args("5ac:24f", "keyRemap.rows").is_err(),
            "키 모양('keyRemap.rows')은 feature 값이 아니다 — 거부해야 한다"
        );
        assert!(
            validate_copy_common_to_device_args("5ac:24f", "all").is_err(),
            "feature 화이트리스트 밖 값은 거부해야 한다"
        );
    }

    #[test]
    fn settings_copy_common_accepts_valid_device_and_feature() {
        assert!(validate_copy_common_to_device_args("5ac:24f", "keyRemap").is_ok());
        assert!(validate_copy_common_to_device_args("5ac:24f", "functionKeys").is_ok());
    }

    // ⭐ 이슈 #46 — 커맨드가 generate_handler! 등록 목록에 존재한다(§6.2 — 정적
    // 배선 테스트의 main.rs 쪽 1줄 확인).
    #[test]
    fn settings_copy_common_to_device_is_registered_in_generate_handler() {
        let source = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs"));
        assert!(
            source.contains("settings_copy_common_to_device,"),
            "generate_handler! 등록 목록에 settings_copy_common_to_device 가 없다"
        );
    }

    // F-17 — collect_per_device_values() 는 `perDevice.` 접두사 키만 원본 JSON
    // 그대로 모으고 `_managed` 원장은 제외한다. `null` 값(명시적 끔, §3.3)도 그대로
    // 실린다 — 부재와 구별돼야 하기 때문이다.
    #[test]
    fn collect_per_device_values_includes_null_and_excludes_managed_ledger() {
        let mut store = SettingsStore::in_memory();
        store
            .set(
                &keys::per_device_function_key("all", FKey::F1),
                &serde_json::Value::Null,
            )
            .unwrap();
        store
            .set(keys::PER_DEVICE_MANAGED, &serde_json::json!({}))
            .unwrap();
        store
            .set(settings_keys::GENERAL_LAUNCH_ON_LOGIN, &true)
            .unwrap();

        let values = collect_per_device_values(&store);

        assert_eq!(
            values.get(&keys::per_device_function_key("all", FKey::F1)),
            Some(&serde_json::Value::Null)
        );
        assert!(!values.contains_key(keys::PER_DEVICE_MANAGED));
        assert!(!values.contains_key(settings_keys::GENERAL_LAUNCH_ON_LOGIN));
    }

    // ⭐ 이슈 #31 ② — build_per_device_view() 의 destinationCategories/destinations
    // 는 목적지 카탈로그 313종/15카테고리 전량을 실어 보낸다(옛 8종짜리
    // systemFunctions 를 대체). label_key 조립 규칙과 카탈로그 개수를 함께 고정한다.
    #[test]
    fn build_per_device_view_carries_the_full_destination_catalog() {
        let view = build_per_device_view(&SettingsStore::in_memory());

        assert_eq!(view.destination_categories.len(), 15);
        assert_eq!(view.destinations.len(), destinations::all().len());

        let disable = view
            .destination_categories
            .iter()
            .find(|c| c.key == "disable")
            .expect("disable 카테고리가 있어야 한다");
        assert_eq!(
            disable.label_key,
            "preferences.keyboards.functionKeys.category.disable"
        );

        // 새 카탈로그가 이전 8종짜리 옛 어휘를 아예 쓰지 않는다는 것도 확인한다.
        let values: Vec<&str> = view.destinations.iter().map(|d| d.value.as_str()).collect();
        assert!(values.contains(&"consumer.mute"));
        assert!(values.contains(&"appleKeyboard.mission_control"));
        assert!(!values
            .iter()
            .any(|v| v.chars().next().is_some_and(char::is_uppercase)));
    }

    // caps_is_modifier_source() — hyper/meh/bleh 중 활성화된 슬롯만 본다.
    /// ⭐ 충돌 대화상자 #1 의 배타 대상은 **caps lock 을 점유한 슬롯**이지 지금 켜려는
    /// 설정 자신이 아니다 — 실기기 검증에서 잡은 회귀의 재발 방지.
    #[test]
    fn caps_modifier_slot_keys_lists_only_enabled_caps_lock_slots() {
        let mut h = HyperkeySettings::default();
        assert!(
            caps_modifier_slot_keys(&h).is_empty(),
            "기본값은 전부 꺼져 있다"
        );

        h.hyper.enabled = true;
        h.hyper.source = SourceKey::CapsLock;
        assert_eq!(
            caps_modifier_slot_keys(&h),
            vec![settings_keys::HYPERKEY_HYPER_ENABLED]
        );

        // 소스가 caps lock 이 아니면 세지 않는다.
        h.hyper.source = SourceKey::RightCommand;
        assert!(caps_modifier_slot_keys(&h).is_empty());

        // 여러 슬롯이 동시에 caps lock 을 쓰면 전부 나열한다.
        h.hyper.source = SourceKey::CapsLock;
        h.meh.enabled = true;
        h.meh.source = SourceKey::CapsLock;
        assert_eq!(
            caps_modifier_slot_keys(&h),
            vec![
                settings_keys::HYPERKEY_HYPER_ENABLED,
                settings_keys::HYPERKEY_MEH_ENABLED
            ]
        );
    }

    /// 배타 대상의 라벨 키가 실제로 카탈로그에 있는 것이어야 한다.
    #[test]
    fn disable_label_key_covers_modifier_slots() {
        for (store_key, expected) in [
            (
                settings_keys::HYPERKEY_HYPER_ENABLED,
                "settings.hyperkey.hyper.label",
            ),
            (
                settings_keys::HYPERKEY_MEH_ENABLED,
                "settings.hyperkey.meh.label",
            ),
            (
                settings_keys::HYPERKEY_BLEH_ENABLED,
                "settings.hyperkey.bleh.label",
            ),
        ] {
            assert_eq!(disable_label_key_for_setting(store_key), expected);
        }
    }

    #[test]
    fn caps_is_modifier_source_only_counts_enabled_slots() {
        let mut hyperkey = HyperkeySettings::default();
        assert!(!caps_is_modifier_source(&hyperkey));

        hyperkey.hyper.source = SourceKey::CapsLock;
        assert!(
            !caps_is_modifier_source(&hyperkey),
            "꺼진 슬롯은 세지 않는다"
        );

        hyperkey.hyper.enabled = true;
        assert!(caps_is_modifier_source(&hyperkey));
    }

    // compute_caps_lock_alias() — needs_caps_lock_alias × synthesize 조합.
    #[test]
    fn compute_caps_lock_alias_matrix() {
        assert_eq!(
            compute_caps_lock_alias(&PresetSettings::default(), false, false, false),
            None
        );
        assert_eq!(
            compute_caps_lock_alias(&PresetSettings::default(), true, false, false),
            Some(KeyCode::F18)
        );

        let synthesize = PresetSettings {
            synthesize_caps_lock_remap: true,
            ..PresetSettings::default()
        };
        assert_eq!(compute_caps_lock_alias(&synthesize, true, false, false), None);
    }

    /// ⭐ F-01 — `seek_remap_is_caps_lock` 하나만으로도 다른 이유 없이 F18 alias 가
    /// 켜진다. Advanced 토글이 켜지면 이 셋째 이유도 다른 이유들과 마찬가지로
    /// 무시된다.
    #[test]
    fn compute_caps_lock_alias_seek_remap_is_caps_lock_forces_alias() {
        assert_eq!(
            compute_caps_lock_alias(&PresetSettings::default(), false, true, false),
            Some(KeyCode::F18)
        );
        assert_eq!(
            compute_caps_lock_alias(&PresetSettings::default(), false, false, false),
            None,
            "seek_remap_is_caps_lock 이 false 면 이 이유만으로는 alias 가 켜지지 않는다"
        );

        let synthesize = PresetSettings {
            synthesize_caps_lock_remap: true,
            ..PresetSettings::default()
        };
        assert_eq!(
            compute_caps_lock_alias(&synthesize, false, true, false),
            None,
            "Advanced 토글이 켜지면 seek_remap_is_caps_lock 이유도 무시된다"
        );
    }

    // ⭐ F-19 — 언어 프리셋(캡스락 단독 탭 규칙)이 켜지면 D-1 alias 가 필요하다.
    #[test]
    fn compute_caps_lock_alias_language_caps_lock_tap_forces_alias() {
        assert_eq!(
            compute_caps_lock_alias(&PresetSettings::default(), false, false, true),
            Some(KeyCode::F18)
        );
        assert_eq!(
            compute_caps_lock_alias(&PresetSettings::default(), false, false, false),
            None
        );
        // Advanced 토글이 켜지면 다른 이유들과 마찬가지로 무시된다.
        let synthesize = PresetSettings {
            synthesize_caps_lock_remap: true,
            ..PresetSettings::default()
        };
        assert_eq!(compute_caps_lock_alias(&synthesize, false, false, true), None);
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
        assert_eq!(
            nothing.label_key.as_deref(),
            Some("settings.presets.option.nothing")
        );

        let seek = opts
            .caps_quick_actions
            .iter()
            .find(|o| o.value == "Seek")
            .unwrap();
        assert_eq!(seek.label, "Seek");
        assert_eq!(seek.label_key, None, "Seek 는 고유명사라 labelKey 가 없다");

        let esc = opts
            .caps_remap_targets
            .iter()
            .find(|o| o.value == "Esc")
            .unwrap();
        assert_eq!(esc.label, "esc");
        assert_eq!(esc.label_key, None, "키캡 각인은 labelKey 가 없다");

        let symbol = opts
            .home_row_schemes
            .iter()
            .find(|o| o.value == "SymbolRow")
            .unwrap();
        assert_eq!(
            symbol.label_key.as_deref(),
            Some("settings.presets.option.home_row.symbol")
        );

        let hyper_trigger = opts
            .paste_triggers
            .iter()
            .find(|o| o.value == "HyperKey")
            .unwrap();
        assert_eq!(
            hyper_trigger.label_key.as_deref(),
            Some("settings.presets.option.paste.hyper")
        );
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
        let view = pending_conflict_view(
            conflict,
            keys::PRESETS_CAPS_WASD_ARROWS,
            &serde_json::json!(true),
        );
        assert_eq!(view.kind, "capsLockArrows");
        assert_eq!(view.key, keys::PRESETS_CAPS_WASD_ARROWS);
        assert_eq!(view.value, serde_json::json!(true));
        assert_eq!(
            view.disable_label_keys,
            vec!["settings.presets.caps_hjkl.prefix".to_string()]
        );
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
        let err = validate_and_apply_preset(
            &mut presets,
            "presets.doesNotExist",
            &serde_json::json!(true),
        )
        .unwrap_err();
        assert!(err.contains("unknown settings key"));
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
        let err =
            validate_and_apply_korean(&mut korean, "korean.doesNotExist", &serde_json::json!(true))
                .unwrap_err();
        assert!(err.contains("unknown settings key"));
    }

    // ============================================================================
    // F-01 `settings_set` 의 `seek.*` 경로 — `apply_seek_setting`/
    // `parse_seek_remap_key`/`quick_press_opens_seek`/`seek_view`.
    // ============================================================================

    // apply_seek_setting() — toggleShortcut 의 code/modifiers 는 부분 갱신이다:
    // 한쪽만 바뀌어도 다른 쪽의 기존 값을 보존한다.
    #[test]
    fn apply_seek_setting_updates_toggle_shortcut_code_preserving_modifiers() {
        let mut seek = SeekSettings {
            toggle_shortcut: Some(SeekShortcut {
                code: "Space".to_string(),
                modifiers: EventFlags::ALTERNATE,
            }),
            ..SeekSettings::default()
        };
        apply_seek_setting(
            &mut seek,
            keys::SEEK_TOGGLE_SHORTCUT_CODE,
            &serde_json::json!("KeyA"),
        )
        .unwrap();
        let shortcut = seek.toggle_shortcut.unwrap();
        assert_eq!(shortcut.code, "KeyA");
        assert_eq!(
            shortcut.modifiers,
            EventFlags::ALTERNATE,
            "modifiers 는 보존돼야 한다"
        );
    }

    #[test]
    fn apply_seek_setting_updates_toggle_shortcut_modifiers_preserving_code() {
        let mut seek = SeekSettings {
            toggle_shortcut: Some(SeekShortcut {
                code: "Space".to_string(),
                modifiers: EventFlags::NONE,
            }),
            ..SeekSettings::default()
        };
        apply_seek_setting(
            &mut seek,
            keys::SEEK_TOGGLE_SHORTCUT_MODIFIERS,
            &serde_json::json!(EventFlags::COMMAND.0),
        )
        .unwrap();
        let shortcut = seek.toggle_shortcut.unwrap();
        assert_eq!(shortcut.code, "Space", "code 는 보존돼야 한다");
        assert_eq!(shortcut.modifiers, EventFlags::COMMAND);
    }

    // apply_seek_setting() — `null` 은 명시적 지우기(단축키 제거).
    #[test]
    fn apply_seek_setting_null_clears_toggle_shortcut() {
        let mut seek = SeekSettings {
            toggle_shortcut: Some(SeekShortcut {
                code: "Space".to_string(),
                modifiers: EventFlags::NONE,
            }),
            ..SeekSettings::default()
        };
        apply_seek_setting(
            &mut seek,
            keys::SEEK_TOGGLE_SHORTCUT_CODE,
            &serde_json::Value::Null,
        )
        .unwrap();
        assert_eq!(seek.toggle_shortcut, None);
    }

    // apply_seek_setting() — seek.remapKey 는 "-" 를 None 으로, 그 밖은 SourceKey
    // variant 이름으로 파싱한다.
    #[test]
    fn apply_seek_setting_parses_remap_key_dash_and_variant() {
        let mut seek = SeekSettings::default();
        apply_seek_setting(
            &mut seek,
            keys::SEEK_REMAP_KEY,
            &serde_json::json!("CapsLock"),
        )
        .unwrap();
        assert_eq!(seek.remap_key, Some(SourceKey::CapsLock));

        apply_seek_setting(&mut seek, keys::SEEK_REMAP_KEY, &serde_json::json!("-")).unwrap();
        assert_eq!(seek.remap_key, None);
    }

    #[test]
    fn apply_seek_setting_rejects_unknown_remap_key_value() {
        let mut seek = SeekSettings::default();
        let err = apply_seek_setting(
            &mut seek,
            keys::SEEK_REMAP_KEY,
            &serde_json::json!("NotARealKey"),
        )
        .unwrap_err();
        assert!(err.contains("알 수 없는"));
    }

    #[test]
    fn apply_seek_setting_updates_execute_on_close_and_semicolon_cycles() {
        let mut seek = SeekSettings::default();
        apply_seek_setting(
            &mut seek,
            keys::SEEK_EXECUTE_ON_CLOSE,
            &serde_json::json!(true),
        )
        .unwrap();
        assert!(seek.execute_on_close);
        apply_seek_setting(
            &mut seek,
            keys::SEEK_SEMICOLON_CYCLE,
            &serde_json::json!(true),
        )
        .unwrap();
        assert!(seek.semicolon_cycles);
    }

    // ⭐ F-04(이슈 #44, A4) — 새 키 2종. ⚠️ 이 매치가 빠지면 폴스루로 조용히
    // 거부되므로(true 저장이 안 됨), 반드시 매치가 있는지 단정한다.
    #[test]
    fn apply_seek_setting_updates_focus_window_and_change_click_modes() {
        let mut seek = SeekSettings::default();
        // 출고 기본값(명세 §4): focusWindow ☐, changeClickModes ☑.
        assert!(!seek.focus_window_before_clicking);
        assert!(seek.change_click_modes_with_modifiers);

        apply_seek_setting(
            &mut seek,
            keys::SEEK_FOCUS_WINDOW_BEFORE_CLICKING,
            &serde_json::json!(true),
        )
        .unwrap();
        assert!(seek.focus_window_before_clicking);

        apply_seek_setting(
            &mut seek,
            keys::SEEK_CHANGE_CLICK_MODES_WITH_MODIFIERS,
            &serde_json::json!(false),
        )
        .unwrap();
        assert!(!seek.change_click_modes_with_modifiers);

        apply_seek_setting(
            &mut seek,
            keys::SEEK_CHANGE_CLICK_MODES_WITH_MODIFIERS,
            &serde_json::json!(true),
        )
        .unwrap();
        assert!(seek.change_click_modes_with_modifiers);

        // 타입이 안 맞으면 거부 — 다른 Seek 키와 같은 계약.
        assert!(
            apply_seek_setting(
                &mut seek,
                keys::SEEK_FOCUS_WINDOW_BEFORE_CLICKING,
                &serde_json::json!("yes"),
            )
            .is_err()
        );
    }

    // ⭐(이슈 #133, D12) — 창 제목 검색 키. ⚠️ 이 매치가 빠지면 폴스루로 조용히
    // 거부되므로(체크 해제 저장이 안 됨), 반드시 매치가 있는지 단정한다.
    #[test]
    fn apply_seek_setting_updates_include_window_titles() {
        let mut seek = SeekSettings::default();
        // 출고(☑) 기본값 — D12 클론 결정.
        assert!(seek.include_window_titles);

        apply_seek_setting(
            &mut seek,
            keys::SEEK_INCLUDE_WINDOW_TITLES,
            &serde_json::json!(false),
        )
        .unwrap();
        assert!(!seek.include_window_titles);

        apply_seek_setting(
            &mut seek,
            keys::SEEK_INCLUDE_WINDOW_TITLES,
            &serde_json::json!(true),
        )
        .unwrap();
        assert!(seek.include_window_titles);

        // 타입이 안 맞으면 거부 — 다른 Seek 키와 같은 계약.
        assert!(
            apply_seek_setting(
                &mut seek,
                keys::SEEK_INCLUDE_WINDOW_TITLES,
                &serde_json::json!("yes"),
            )
            .is_err()
        );
    }

    // ⭐(이슈 #93) — `seek.searchLanguage` 채택/거부. 인풋 박스 모드를 좌우하는
    // 값이라 허용 코드가 아니면 조용히 넘어가지 않게 거부한다.
    #[test]
    fn apply_seek_setting_search_language_accepts_and_rejects() {
        let mut seek = SeekSettings::default();
        assert_eq!(seek.search_language, None);

        apply_seek_setting(
            &mut seek,
            keys::SEEK_SEARCH_LANGUAGE,
            &serde_json::json!("ko"),
        )
        .unwrap();
        assert_eq!(seek.search_language.as_deref(), Some("ko"));
        assert!(seek.to_config(false).input_box_mode);

        apply_seek_setting(
            &mut seek,
            keys::SEEK_SEARCH_LANGUAGE,
            &serde_json::json!("en"),
        )
        .unwrap();
        assert_eq!(seek.search_language.as_deref(), Some("en"));
        assert!(!seek.to_config(false).input_box_mode, "en = 영어 강제(인풋 아님)");

        // null → 부재(영어 기본)로 되돌린다.
        apply_seek_setting(&mut seek, keys::SEEK_SEARCH_LANGUAGE, &serde_json::Value::Null)
            .unwrap();
        assert_eq!(seek.search_language, None);

        // 빈 문자열도 부재와 같은 취급.
        apply_seek_setting(
            &mut seek,
            keys::SEEK_SEARCH_LANGUAGE,
            &serde_json::json!(""),
        )
        .unwrap();
        assert_eq!(seek.search_language, None);

        // 허용 코드가 아니면 거부(조용히 무시하지 않는다).
        assert!(
            apply_seek_setting(
                &mut seek,
                keys::SEEK_SEARCH_LANGUAGE,
                &serde_json::json!("fr"),
            )
            .is_err()
        );
    }

    // ⭐ F-04 — `click_settings_from_seek` 가 저장 표현을 경계 타입으로
    // 옮긴다(기본값은 `SeekSettings` 쪽이 이미 반영 — 부재 = 기본값).
    #[test]
    fn click_settings_from_seek_maps_the_two_fields() {
        let settings = click_settings_from_seek(&SeekSettings::default());
        assert!(!settings.focus_window_before_clicking);
        assert!(settings.change_click_modes_with_modifiers);

        let flipped = SeekSettings {
            focus_window_before_clicking: true,
            change_click_modes_with_modifiers: false,
            ..SeekSettings::default()
        };
        let settings = click_settings_from_seek(&flipped);
        assert!(settings.focus_window_before_clicking);
        assert!(!settings.change_click_modes_with_modifiers);
    }

    // ⚠️ 위임 지시 §5-5 — `seek.searchBar.x/y` 는 이 커맨드로 바꿀 수 없다(F-03 이
    // 직접 저장한다).
    #[test]
    fn apply_seek_setting_rejects_search_bar_position_keys() {
        let mut seek = SeekSettings::default();
        assert!(
            apply_seek_setting(&mut seek, keys::SEEK_SEARCH_BAR_X, &serde_json::json!(1.0))
                .is_err()
        );
        assert!(
            apply_seek_setting(&mut seek, keys::SEEK_SEARCH_BAR_Y, &serde_json::json!(1.0))
                .is_err()
        );
    }

    #[test]
    fn validate_and_apply_seek_rejects_unknown_key() {
        let mut seek = SeekSettings::default();
        let err = validate_and_apply_seek(&mut seek, "seek.doesNotExist", &serde_json::json!(true))
            .unwrap_err();
        assert!(err.contains("알 수 없는"));
    }

    // quick_press_opens_seek() — 체크박스 + 팝업 값이 둘 다 맞아야 true.
    #[test]
    fn quick_press_opens_seek_requires_enabled_and_seek_action() {
        assert!(!quick_press_opens_seek(&PresetSettings::default()));

        let enabled_wrong_action = PresetSettings {
            caps_quick_press: ultrakey_presets::settings::CapsQuickPressSettings {
                enabled: true,
                action: QuickPressCapsAction::Esc,
            },
            ..PresetSettings::default()
        };
        assert!(!quick_press_opens_seek(&enabled_wrong_action));

        let enabled_seek = PresetSettings {
            caps_quick_press: ultrakey_presets::settings::CapsQuickPressSettings {
                enabled: true,
                action: QuickPressCapsAction::Seek,
            },
            ..PresetSettings::default()
        };
        assert!(quick_press_opens_seek(&enabled_seek));
    }

    // seek_view() — remap_key_options 35종, `-` 값·라벨, any_activation_configured.
    #[test]
    fn seek_view_reflects_settings_and_lists_35_remap_key_options() {
        let view = seek_view(&SeekSettings::default(), false);
        assert_eq!(view.remap_key, "-");
        assert!(!view.any_activation_configured);
        assert_eq!(view.remap_key_options.len(), 35);
        assert_eq!(view.remap_key_options[0].value, "-");
        assert_eq!(view.remap_key_options[0].label, "-");
        // ⭐ F-04(이슈 #44) — 출고 기본값 그대로: focusWindow ☐, changeModes ☑.
        assert!(!view.focus_window_before_clicking);
        assert!(view.change_click_modes_with_modifiers);
        // ⭐(이슈 #133, D12) — 창 제목 검색 기본 ☑(실측이 아닌 클론 결정).
        assert!(view.include_window_titles);

        let with_caps_lock = seek_view(
            &SeekSettings {
                remap_key: Some(SourceKey::CapsLock),
                focus_window_before_clicking: true,
                ..SeekSettings::default()
            },
            false,
        );
        assert_eq!(with_caps_lock.remap_key, "CapsLock");
        assert!(with_caps_lock.any_activation_configured);
        assert!(with_caps_lock.focus_window_before_clicking);
    }

    // SettingsState 직렬화가 camelCase 인지 — 프런트엔드가 기대하는 필드 이름 계약.
    #[test]
    fn settings_state_serializes_camel_case() {
        let hyperkey = HyperkeySettings::default();
        let presets = PresetSettings::default();
        let korean = KoreanSettings::default();
        let seek = SeekSettings::default();
        let store = SettingsStore::in_memory();
        let state = build_settings_state(
            &hyperkey,
            &presets,
            &korean,
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
            &seek,
            &store,
            false,
            Some("디스크 가득 참".to_string()),
            None,
            false,
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
        // ⭐ F-13 — General 탭 자동 확인 체크박스도 camelCase 로 실린다.
        assert_eq!(obj["general"]["autoUpdate"], false);

        // ⭐ F-01 `Seek` 탭 계약 — 다른 위임의 `settings.html` 이 이 모양을 전제한다.
        let seek_json = obj["seek"].as_object().unwrap();
        assert!(seek_json.contains_key("toggleShortcut"));
        assert_eq!(seek_json["toggleShortcut"], serde_json::Value::Null);
        assert_eq!(seek_json["remapKey"], "-");
        assert!(seek_json.contains_key("executeOnClose"));
        assert!(seek_json.contains_key("semicolonCycles"));
        // ⭐ F-04(이슈 #44) — 체크박스 2종도 camelCase 로 실린다.
        assert_eq!(seek_json["focusWindowBeforeClicking"], false);
        assert_eq!(seek_json["changeClickModesWithModifiers"], true);
        // ⭐(이슈 #133, D12) — 창 제목 검색 체크박스(기본 ☑).
        assert_eq!(seek_json["includeWindowTitles"], true);
        assert!(seek_json.contains_key("quickPressOpens"));
        assert_eq!(seek_json["anyActivationConfigured"], false);
        assert_eq!(seek_json["remapKeyOptions"].as_array().unwrap().len(), 35);

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
        // ⭐ 이슈 #77 — General 탭 Advanced 섹션의 synthesize 체크박스가 이 값을
        // 렌더한다(`$("synthesize-caps-remap").checked = state.presets.
        // synthesizeCapsLockRemap`). 설정 화면은 카탈로그 부재 = 기본값(꺼짐)을
        // 이 경로로 직접 받는다 — camelCase 필드로 실려야 한다.
        assert_eq!(presets_json["synthesizeCapsLockRemap"], false);

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
            &JapaneseSettings::default(),
            &ChineseSettings::default(),
            &SeekSettings::default(),
            &store,
            false,
            None,
            None,
            false,
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

        let warnings =
            build_preset_warnings(&hyperkey, &PresetSettings::default(), Some(KeyCode::F18));
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
        assert_eq!(
            ignore_menu_text(&catalog, None),
            catalog.get("menu.ignore_app.none")
        );
    }

    // `presets.synthesizeCapsLockRemap` — 부재 = 기본값(꺼짐), 저장된 값이 있으면 그대로.
    // ⭐ 이슈 #77 — 메뉴 전용 헬퍼 `synthesize_caps_lock_remap_enabled` 는 (메뉴
    // 항목과 함께) 제거됐다. 저장 키 자체는 살아있고(설정 화면 체크박스가
    // `settings_set_preset` 경로로 쓴다), 이 테스트는 키의 불변 계약(부재 = 기본값)
    // 을 `SettingsStore::get` 직접 단언으로 보존한다.
    #[test]
    fn synthesize_caps_lock_remap_defaults_to_false() {
        let store = SettingsStore::in_memory();
        assert!(!store
            .get::<bool>(keys::PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP)
            .unwrap_or(false));
    }

    #[test]
    fn synthesize_caps_lock_remap_reflects_stored_value() {
        let mut store = SettingsStore::in_memory();
        store
            .set(keys::PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP, &true)
            .unwrap();
        assert!(store
            .get::<bool>(keys::PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP)
            .unwrap_or(false));
    }

    // menu_ids 상수들이 서로 다르다 — 오타로 두 메뉴 항목이 같은 id 를 갖는 회귀를 막는다.
    #[test]
    fn menu_ids_are_all_distinct() {
        let ids = [
            menu_ids::IGNORE_APP,
            menu_ids::SETTINGS,
            menu_ids::CHECK_FOR_UPDATES,
            menu_ids::ABOUT,
            menu_ids::ADVANCED,
            menu_ids::RELAUNCH,
            menu_ids::QUIT,
            menu_ids::AUTHORIZE,
        ];
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            ids.len(),
            "중복된 메뉴 항목 id 가 있다: {ids:?}"
        );
    }

    // ── 이슈 #39 Phase 2 — 설정 export/import 날짜 헬퍼 ──────────────────────
    //
    // ⭐ 새 시간 크레이트를 들이지 않고 직접 쓴 `civil_from_days`/`rfc3339_utc`/
    // `export_default_file_name` 이 실제로 맞는지, 잘 알려진 유닉스 시각 세 개로
    // 확인한다(값은 파이썬 `datetime.utcfromtimestamp` 로 교차 검증했다).

    #[test]
    fn civil_from_days_matches_known_epoch_values() {
        // 1970-01-01T00:00:00Z (day 0).
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        // 1700000000 초 = 2023-11-14T22:13:20Z.
        assert_eq!(civil_from_days((1_700_000_000u64 / 86_400) as i64), (2023, 11, 14));
        // 1893456000 초 = 2030-01-01T00:00:00Z.
        assert_eq!(civil_from_days((1_893_456_000u64 / 86_400) as i64), (2030, 1, 1));
    }

    #[test]
    fn rfc3339_utc_formats_known_epoch_values() {
        assert_eq!(rfc3339_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(rfc3339_utc(1_700_000_000), "2023-11-14T22:13:20Z");
    }

    #[test]
    fn export_default_file_name_uses_yyyymmdd() {
        assert_eq!(
            export_default_file_name(1_700_000_000),
            "ultrakey-settings-20231114.json"
        );
    }

    // ── 이슈 #39 Phase 2 — `ImportOutcome` → `ImportResultView` ──────────────

    #[test]
    fn import_result_view_carries_outcome_fields_in_camel_case() {
        let outcome = transfer::ImportOutcome {
            applied_keys: 3,
            removed_keys: 1,
            absent_devices: vec!["dead:beef".to_string()],
            backup: Some(std::path::PathBuf::from("/tmp/settings.json.pre-import-1")),
            migrated_from: None,
        };
        let view = ImportResultView::from(outcome);
        assert_eq!(view.applied_keys, 3);
        assert_eq!(view.removed_keys, 1);
        assert_eq!(view.absent_devices, vec!["dead:beef".to_string()]);
        assert_eq!(
            view.backup_path,
            Some("/tmp/settings.json.pre-import-1".to_string())
        );

        let json = serde_json::to_value(&view).unwrap();
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("appliedKeys"), "{obj:?}");
        assert!(obj.contains_key("removedKeys"), "{obj:?}");
        assert!(obj.contains_key("absentDevices"), "{obj:?}");
        assert!(obj.contains_key("backupPath"), "{obj:?}");
    }

    #[test]
    fn import_result_view_has_no_backup_path_when_there_was_no_backup() {
        let outcome = transfer::ImportOutcome {
            applied_keys: 0,
            removed_keys: 0,
            absent_devices: vec![],
            backup: None,
            migrated_from: None,
        };
        assert_eq!(ImportResultView::from(outcome).backup_path, None);
    }

    // ── F-18 Event Viewer — 규칙 식별자 → 사람이 읽는 이름 ───────────────────

    #[test]
    fn rule_label_resolves_a_preset_identifier() {
        let catalog = Catalog::for_locale(ultrakey_i18n::Locale::En);
        assert_eq!(
            rule_label(&catalog, "preset:5"),
            Some(catalog.get("settings.presets.caps_wasd").to_string())
        );
    }

    #[test]
    fn rule_label_resolves_the_split_sentence_hjkl_preset() {
        let catalog = Catalog::for_locale(ultrakey_i18n::Locale::En);
        let label = rule_label(&catalog, "preset:6").expect("preset:6 은 이름이 있어야 한다");
        assert!(label.contains(catalog.get("settings.presets.caps_hjkl.prefix")));
        assert!(label.contains(catalog.get("settings.presets.caps_hjkl.suffix")));
    }

    #[test]
    fn rule_label_resolves_a_korean_identifier() {
        let catalog = Catalog::for_locale(ultrakey_i18n::Locale::En);
        assert_eq!(
            rule_label(&catalog, "korean:14"),
            Some(catalog.get("settings.korean.han_eng").to_string())
        );
    }

    #[test]
    fn rule_label_resolves_hyperkey_to_the_tab_name() {
        let catalog = Catalog::for_locale(ultrakey_i18n::Locale::En);
        assert_eq!(
            rule_label(&catalog, "hyperkey"),
            Some(catalog.get("settings.tab.hyperkey").to_string())
        );
    }

    /// 결번(F-08.3)·모르는 프리셋 번호·모르는 규칙 문자열은 전부 `None` — 프런트가
    /// 계층 이름만 보여준다(명세 §3.4 그대로).
    #[test]
    fn rule_label_is_none_for_unknown_identifiers() {
        let catalog = Catalog::for_locale(ultrakey_i18n::Locale::En);
        assert_eq!(rule_label(&catalog, "preset:3"), None);
        assert_eq!(rule_label(&catalog, "preset:255"), None);
        assert_eq!(rule_label(&catalog, "korean:1"), None);
        assert_eq!(rule_label(&catalog, "bogus"), None);
    }
}
