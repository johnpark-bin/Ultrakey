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
    });

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            modal_copy,
            open_settings,
            quit_app
        ])
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

            tracing::info!(
                windows = ?app.webview_windows().keys().collect::<Vec<_>>(),
                "setup() 완료 — 웹뷰 창 목록"
            );

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
fn on_permission_transition(handle: &tauri::AppHandle, state: &Arc<AppState>, to: PermissionState) {
    tracing::info!(?to, "on_permission_transition 진입");
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

/// ⭐ **창 조작은 반드시 메인 스레드로 "비동기" 디스패치한다**(이슈 #10 후속).
///
/// ⛔ **왜 동기 호출이면 안 되는가 — 시스템 전체 입력이 멈춘다.**
///
/// `WebviewWindow::show()`/`set_focus()`/`is_visible()` 류는 메인 스레드가 아닌 곳에서
/// 부르면 **메인 스레드로 동기 디스패치하고 응답을 기다린다.** 그런데 이 함수의
/// 호출자 중 하나는 **탭 전용 스레드**다 — `Engine::start` 에 넘긴 `on_event` 콜백이
/// `EngineEvent::NotTrusted` 를 탭 스레드에서 부르고(`ultrakey-engine` 의
/// `handle_recreate_tap`), 그것이 여기로 이어진다.
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
fn on_main_thread(
    handle: &tauri::AppHandle,
    what: &'static str,
    action: impl FnOnce(&tauri::WebviewWindow) + Send + 'static,
) {
    let handle_for_closure = handle.clone();
    let dispatched = handle.run_on_main_thread(move || {
        match handle_for_closure.get_webview_window("permissions") {
            Some(w) => {
                tracing::info!(what, "permissions 창을 찾았다");
                action(&w);
                tracing::info!(
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
                tracing::error!(what, "permissions 창을 찾지 못했다");
            }
        }
    });
    if let Err(e) = dispatched {
        tracing::error!(what, error = %e, "메인 스레드로 디스패치하지 못했다");
    }
}

fn show_modal(handle: &tauri::AppHandle) {
    on_main_thread(handle, "show_modal", |w| {
        let _ = w.show();
        let _ = w.set_focus();
    });
}

fn hide_modal(handle: &tauri::AppHandle) {
    on_main_thread(handle, "hide_modal", |w| {
        let _ = w.hide();
    });
}
