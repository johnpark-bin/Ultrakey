//! Ultrakey — Tauri 앱 껍데기.
//!
//! ⭐ **M1 에서 이 크레이트가 하는 일은 배선(wiring)뿐이다.** 환경설정 UI 는 M2(F-09),
//! 메뉴바는 M2·M3(F-10) 범위이며, 여기 있는 유일한 UI 는 F-11 권한 안내 모달이다.
//!
//! 배선 순서(`docs/dev/architecture.md` §2.1 의 스레드 배치 그대로):
//!
//! 1. 로그 초기화 — `ULTRAKEY_LOG` 환경변수. 수동 검증 절차가 이 로그에 기댄다
//!    (`docs/dev/manual-verification.md` §0)
//! 2. 로케일 결정 → 문자열 카탈로그(D4: ko + en)
//! 3. ⭐ Accessory 앱으로 전환 — Dock 아이콘·⌘Tab 미노출(`menu-bar-and-lifecycle.md`
//!    §3, 원본 `LSUIElement = true` 실측에 대응)
//! 4. F-11 권한 감시 시작 → 권한이 생기면 F-07 엔진 시작
//! 5. 엔진 사건 처리 — ⛔ 치명적 탭 생성 실패는 재시도가 아니라 **종료**다
//!    (`key-remapping-engine.md` §3-a, §5#16)

// `unsafe` 는 전부 `ultrakey-platform` 에만 있다.
#![forbid(unsafe_code)]
// 릴리스 빌드에서 콘솔 창을 띄우지 않는다(macOS 에서는 무해하지만 관례를 따른다).
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{Arc, Mutex};

use tauri::{Manager, State};

use ultrakey_core::gate::{AppGateController, AtomicAppGate};
use ultrakey_core::settings::EngineConfig;
use ultrakey_engine::{Engine, EngineEvent};
use ultrakey_hyperkey::HyperkeySettings;
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

struct AppState {
    catalog: Catalog,
    /// 엔진은 권한이 생긴 뒤에야 시작된다 — 그전에는 `None`.
    engine: Mutex<Option<Engine>>,
    gate: Arc<AtomicAppGate>,
    #[allow(dead_code)] // M3(F-10)의 메뉴바 `Ignore <앱>` 이 이것을 쓴다.
    gate_controller: Arc<AppGateController>,
    monitor: Mutex<Option<PermissionMonitor>>,
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
    if permission_state == PermissionState::OutOfSync {
        let c = out_of_sync_copy(catalog);
        return ModalCopy {
            kind: "out_of_sync",
            title: c.title,
            body: c.body,
            path: String::new(),
            open_button: String::new(),
            checkbox_hint: String::new(),
            locked_hint: String::new(),
            manual_steps: c.manual_steps,
            quit: c.quit,
        };
    }

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
}

#[tauri::command]
fn open_settings() -> bool {
    open_accessibility_settings()
}

#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}

fn main() {
    // 1) 로그 — 기본은 조용하고, `ULTRAKEY_LOG=debug` 로 켠다.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_env("ULTRAKEY_LOG")
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .with_writer(std::io::stderr)
        .init();

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
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![modal_copy, open_settings, quit_app])
        .setup(move |app| {
            // 3) ⭐ Accessory 앱 — Dock 아이콘 없음, ⌘Tab 에 안 나타남.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();
            let state_for_monitor = state.clone();

            // 4) F-11 권한 감시. 전이가 오면 엔진을 켜거나 모달을 띄운다.
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

            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Tauri 앱을 초기화하지 못했다")
        .run(|_app, event| {
            if let tauri::RunEvent::ExitRequested { .. } = event {
                tracing::info!("종료 요청 — 엔진을 정리한다");
            }
        });
}

/// 권한 상태에 따라 엔진을 켜거나 모달을 띄운다.
fn on_permission_transition(
    handle: &tauri::AppHandle,
    state: &Arc<AppState>,
    to: PermissionState,
) {
    match to {
        PermissionState::Granted => {
            hide_modal(handle);
            start_engine_if_needed(handle, state);
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

    // ⭐ M1 의 규칙 테이블. 환경설정 UI 가 없으므로(M2/F-09) 기본값으로 시작한다 —
    // 즉 hyper/meh/bleh 는 전부 비활성이고, 엔진은 "아무 리매핑도 없이 탭만 살아 있는"
    // 상태로 돈다. 이것이 "M1 완료 판정" 첫 항목이 요구하는 상태다.
    //
    // ⚠️ 값을 바꿔 시험하려면 `HyperkeySettings` 를 여기서 구성하면 된다 — 설정 영속화는
    // F-15(M2) 소관이라 M1 에는 저장 경로가 없다.
    let hyperkey = HyperkeySettings::default();
    for warning in hyperkey.validate() {
        tracing::warn!(?warning, "Hyperkey 설정 경고");
    }
    let mut config = EngineConfig::default();
    config.rules.modifier_rules = hyperkey.to_modifier_rules();
    config.mouse_apply = hyperkey.mouse_apply;

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

fn show_modal(handle: &tauri::AppHandle) {
    if let Some(w) = handle.get_webview_window("permissions") {
        let _ = w.show();
        let _ = w.set_focus();
    }
}

fn hide_modal(handle: &tauri::AppHandle) {
    if let Some(w) = handle.get_webview_window("permissions") {
        let _ = w.hide();
    }
}
