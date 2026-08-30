//! Ultrakey — Tauri 앱 껍데기.
//!
//! ⭐ M1 에서 이 크레이트는 배선(wiring)만 했다. M2 1차(F-09, 이슈 #13)부터
//! 환경설정 창이 실재한다 — Seek·Presets 탭은 아직 자리만, Hyperkey·General 탭은
//! 실제로 동작한다. 메뉴바(F-10)는 여전히 M2 2차 이후다.
//!
//! 배선 순서(`docs/dev/architecture.md` §2.1 의 스레드 배치 그대로):
//!
//! 1. 로그 초기화 — `ULTRAKEY_LOG` 환경변수. 수동 검증 절차가 이 로그에 기댄다
//!    (`docs/dev/manual-verification.md` §0)
//! 2. 로케일 결정 → 문자열 카탈로그(D4: ko + en)
//! 3. ⭐ Accessory 앱으로 전환 — Dock 아이콘·⌘Tab 미노출(`menu-bar-and-lifecycle.md`
//!    §3, 원본 `LSUIElement = true` 실측에 대응)
//! 4. ⭐ 설정 저장소 로드(F-15) — "부재 = 기본값" 규약으로 `HyperkeySettings` 조립
//! 5. F-11 권한 감시 시작 → 권한이 생기면 F-07 엔진 시작 + (M2 1차 임시) 설정 창 표시
//! 6. 엔진 사건 처리 — ⛔ 치명적 탭 생성 실패는 재시도가 아니라 **종료**다
//!    (`key-remapping-engine.md` §3-a, §5#16)

// `unsafe` 는 전부 `ultrakey-platform` 에만 있다.
#![forbid(unsafe_code)]
// 릴리스 빌드에서 콘솔 창을 띄우지 않는다(macOS 에서는 무해하지만 관례를 따른다).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use tauri::{LogicalSize, Manager, State};

use ultrakey_core::gate::{AppGateController, AtomicAppGate};
use ultrakey_core::keycode::SourceKey;
use ultrakey_core::settings::{keys, EngineConfig, LoadOutcome, MouseApply, SettingsStore};
use ultrakey_engine::{Engine, EngineEvent};
use ultrakey_hyperkey::{HyperkeySettings, SettingsWarning, SlotSettings, TrackpadArea};
use ultrakey_i18n::Catalog;
use ultrakey_permissions::{
    dev_build_warning, onboarding_copy, open_accessibility_settings, out_of_sync_copy,
    PermissionMonitor, PermissionState,
};
use ultrakey_platform::bundle;

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

/// 환경설정 창이 화면을 다시 그리는 데 필요한 전부. `settings_bootstrap`·
/// `settings_set` 이 공통으로 돌려준다.
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
}

fn build_settings_state(
    hyperkey: &HyperkeySettings,
    store: &SettingsStore,
    save_error: Option<String>,
) -> SettingsState {
    let last_tab = store
        .get::<String>(keys::UI_LAST_TAB)
        .unwrap_or_else(|| "hyperkey".to_string());
    SettingsState {
        hyperkey: hyperkey_view(hyperkey),
        hyper_preview: hyper_preview(hyperkey),
        source_keys: SourceKey::all().iter().copied().map(source_key_view).collect(),
        trackpad_areas: TrackpadArea::all()
            .iter()
            .copied()
            .map(trackpad_area_view)
            .collect(),
        warnings: hyperkey.validate().into_iter().map(warning_view).collect(),
        last_tab,
        save_error,
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
        "general" => Some((613, 273)),
        _ => None,
    }
}

fn build_engine_config(hyperkey: &HyperkeySettings) -> EngineConfig {
    let mut config = EngineConfig::default();
    config.rules.modifier_rules = hyperkey.to_modifier_rules();
    config.mouse_apply = hyperkey.mouse_apply;
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

struct AppState {
    catalog: Catalog,
    /// 엔진은 권한이 생긴 뒤에야 시작된다 — 그전에는 `None`.
    engine: Mutex<Option<Engine>>,
    gate: Arc<AtomicAppGate>,
    #[allow(dead_code)] // M3(F-10)의 메뉴바 `Ignore <앱>` 이 이것을 쓴다.
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
    /// 부트스트랩이 프런트엔드에 한 번만 알려줄 로드 경고(손상 복구/미래 스키마).
    load_notice: Mutex<Option<Notice>>,
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

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

/// 환경설정 창 부트스트랩 — 카탈로그 전체 + 현재 설정 상태 + 앱 메타를 한 번에
/// 돌려준다(`preferences-ui.md`, ModalCopy 문서 주석과 같은 "단일 카탈로그" 근거).
#[tauri::command]
fn settings_bootstrap(state: State<'_, Arc<AppState>>, app: tauri::AppHandle) -> SettingsBootstrap {
    let catalog = &state.catalog;
    let hyperkey = state.hyperkey.lock().unwrap().clone();
    let store = state.store.lock().unwrap();
    let settings_state = build_settings_state(&hyperkey, &store, None);
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

/// 컨트롤 하나가 바뀔 때마다 호출된다(§3.7 "적용 버튼 없음" — 즉시 반영).
///
/// ⭐ 순서를 반드시 지킨다:
/// 1. `key` 가 `keys::all()` 에 있는지 검증.
/// 2. 메모리 `HyperkeySettings` 갱신.
/// 3. **엔진 반영** — 저장 성공 여부와 무관하게 먼저 한다(D-B: 로그아웃 시 graceful
///    shutdown 이 실행되지 않는다는 M1 실측 근거 — 종료 시점에 뭔가를 flush 하는
///    설계는 애초에 그 시점이 오지 않을 수 있다).
/// 4. **저장** — 실패해도 3번은 이미 끝났다. 실패는 반환값(`saveError`)에 실어
///    UI 가 `settings.save_failed` 로 알리게 한다.
/// 5. 새 `SettingsState` 반환.
#[tauri::command]
fn settings_set(
    state: State<'_, Arc<AppState>>,
    _app: tauri::AppHandle,
    key: String,
    value: serde_json::Value,
) -> Result<SettingsState, String> {
    // 1) + 2)
    let hyperkey_snapshot = {
        let mut hyperkey = state.hyperkey.lock().map_err(|e| e.to_string())?;
        validate_and_apply(&mut hyperkey, &key, &value)?;
        hyperkey.clone()
    };

    // 3) 엔진 반영.
    {
        let engine_guard = state.engine.lock().map_err(|e| e.to_string())?;
        if let Some(engine) = engine_guard.as_ref() {
            engine.reconfigure(build_engine_config(&hyperkey_snapshot));
            if key_affects_modifier_rules(&key) {
                // D-D: 규칙이 바뀌는 변경은 stuck modifier 를 막기 위해 상태도 리셋한다.
                engine.force_reset_state();
            }
        }
        // 엔진이 아직 없으면(권한 대기 중) 건너뛴다 — 다음 `Engine::start` 가 이미
        // 갱신된 `state.hyperkey` 로 조립되므로 이 변경이 유실되지 않는다.
    }

    // 4) 저장.
    let save_error = {
        let mut store = state.store.lock().map_err(|e| e.to_string())?;
        match store.set(&key, &value) {
            Ok(()) => None,
            Err(e) => {
                tracing::error!(key = %key, error = %e, "설정 저장 실패");
                Some(e.to_string())
            }
        }
    };

    // 5) 새 SettingsState.
    let store = state.store.lock().map_err(|e| e.to_string())?;
    Ok(build_settings_state(&hyperkey_snapshot, &store, save_error))
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
        load_notice: Mutex::new(None),
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
            settings_set_tab,
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
            *state.load_notice.lock().unwrap() = notice_from_outcome(&load_outcome);
            *state.store.lock().unwrap() = settings_store;

            let handle = app.handle().clone();
            let state_for_monitor = state.clone();

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
        .run(|app_handle, event| match event {
            tauri::RunEvent::ExitRequested { code, api, .. } => {
                // ⭐ M2 1차 임시 조치 — 메뉴바(F-10)가 아직 없어 `quit_app` 커맨드가
                // 유일한 명시적 종료 경로다(§5 항목 7: "창을 닫아도 앱은 종료되지
                // 않는다"). `code` 가 `None` 이면 사용자가 마지막 창을 닫아 발생한
                // 암묵적 종료 요청이고, `Some` 이면 `quit_app` 이 부른
                // `AppHandle::exit()` 다(`tauri::App::exit` 문서 참고) — 전자만 막는다.
                if code.is_none() {
                    tracing::info!("창 닫힘으로 인한 종료 요청 — 계속 실행한다");
                    api.prevent_exit();
                } else {
                    tracing::info!("종료 요청(quit_app) — 엔진을 정리한다");
                }
            }
            tauri::RunEvent::Reopen { .. } => {
                // ⭐ M2 1차 임시 조치 — 메뉴바(F-10)가 없어 Dock 아이콘 재클릭이
                // 설정 창을 다시 여는 유일한 경로다. F-10 이 `Settings…` 메뉴
                // 항목을 넣으면 그 경로로 옮기고 여기서는 걷어낸다.
                show_settings_window(app_handle);
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
            // ⭐ M2 1차 임시 조치 — `show_settings_window` 문서 주석 참고.
            show_settings_window(handle);
        }
        PermissionState::Denied | PermissionState::OutOfSync | PermissionState::Unknown => {
            show_modal(handle);
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

    // ⭐ M2 1차: store 에서 조립된 실제 사용자 설정을 쓴다 — M1 이 여기 두었던
    // `HyperkeySettings::default()` 하드코딩은 이제 걷어낸다(환경설정 UI 가
    // 생겼으므로 더 이상 유효하지 않은 전제였다).
    let hyperkey = state.hyperkey.lock().unwrap().clone();
    let config = build_engine_config(&hyperkey);

    let handle_for_events = handle.clone();
    match Engine::start(
        config,
        state.gate.clone(),
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

#[cfg(test)]
mod tests {
    use super::*;

    // tab_window_size() — `preferences-ui.md` §3.1 실측값과 정확히 일치해야 한다.
    #[test]
    fn tab_window_size_matches_spec_values() {
        assert_eq!(tab_window_size("seek"), Some((555, 378)));
        assert_eq!(tab_window_size("hyperkey"), Some((710, 517)));
        assert_eq!(tab_window_size("presets"), Some((825, 527)));
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

        let config = build_engine_config(&hyperkey);
        assert_eq!(config.rules.modifier_rules.len(), 1);
        assert!(config.mouse_apply.drag);
        assert!(config.mouse_apply.click); // 기본값 유지
    }

    // SettingsState 직렬화가 camelCase 인지 — 프런트엔드가 기대하는 필드 이름 계약.
    #[test]
    fn settings_state_serializes_camel_case() {
        let hyperkey = HyperkeySettings::default();
        let store = SettingsStore::in_memory();
        let state = build_settings_state(&hyperkey, &store, Some("디스크 가득 참".to_string()));

        let json = serde_json::to_value(&state).unwrap();
        let obj = json.as_object().unwrap();
        assert!(obj.contains_key("hyperPreview"));
        assert!(obj.contains_key("sourceKeys"));
        assert!(obj.contains_key("trackpadAreas"));
        assert!(obj.contains_key("lastTab"));
        assert!(obj.contains_key("saveError"));

        let hyperkey_json = obj["hyperkey"].as_object().unwrap();
        assert!(hyperkey_json.contains_key("includeShiftInHyper"));
        assert!(hyperkey_json.contains_key("mouseApply"));
        let trackpad_json = hyperkey_json["trackpad"].as_object().unwrap();
        assert!(trackpad_json.contains_key("changeMenuBarIcon"));
        assert!(trackpad_json.contains_key("haptic"));
        assert!(!trackpad_json.contains_key("hapticFeedback"));

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

        let state = build_settings_state(&hyperkey, &store, None);
        assert_eq!(state.warnings.len(), 1);
        assert_eq!(state.warnings[0].kind, "duplicate");
        assert_eq!(state.warnings[0].key, "caps lock");
    }
}
