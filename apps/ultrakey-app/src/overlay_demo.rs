//! ⭐ F-03 실기기 검증 하네스 — **제품 경로가 아니다.**
//!
//! F-01(세션 상태 머신)이 이제 `apps/ultrakey-app/src/seek.rs` 로 들어왔다 — 이
//! 모듈은 더 이상 "F-01 이 없어서 대신하는 자리"가 아니다. ⭐ **그래도 지우지
//! 않는다** — `docs/dev/manual-verification.md` 항목 9(F-03 검증 절차)가 여전히
//! 이 하네스에 의존한다: `seek.rs` 워커는 세 활성화 경로(전역 단축키·리매핑 키·
//! quick press caps lock)를 통해서만 열리므로, 그 경로 중 하나를 설정하지 않고
//! "오버레이가 포커스를 안 뺏는가"·"연결선이 디스플레이 경계를 넘는가" 같은
//! F-03 자체의 렌더링 질문만 독립적으로 재현하려면 이 모듈처럼 활성화 경로를
//! 완전히 우회하는 하네스가 여전히 필요하다. 제품 경로는 `overlay.rs` +
//! `seek.rs` 쪽이다.
//!
//! (`ULTRAKEY_SEEK_DETECT_DUMP` 가 F-02 에서 한 역할과 정확히 같다.)
//!
//! ## 실행
//!
//! ```sh
//! open -n …/Ultrakey.app --env ULTRAKEY_OVERLAY_DEMO=1
//! # 선택: 타이핑을 흉내 낼 문자열 (기본 "se")
//! #       --env ULTRAKEY_OVERLAY_DEMO_QUERY=set
//! ```
//!
//! ⭐ **기동 후 [`ARM_DELAY_SECS`] 초 뒤에** 오버레이가 뜬다. 그 사이에 다른
//! 앱(텍스트 편집기 등)을 눌러 커서를 깜빡이게 두면, 오버레이가 뜬 뒤에도
//! 커서가 계속 깜빡이는지로 **포커스를 안 뺏는지**를 눈으로 판정할 수 있다.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::Manager;

use ultrakey_overlay::palette::Appearance;
use ultrakey_seek::DetectionParams;

use crate::overlay::{
    overlay_displays, persist_search_bar_origin, stored_search_bar_origin, OverlayController,
    SurfaceState, WebviewOverlayRenderer,
};

/// 기동 후 오버레이를 띄우기까지의 지연 — 검증자가 다른 앱으로 포커스를 옮길 시간.
const ARM_DELAY_SECS: u64 = 6;
/// 타이핑 흉내의 글자 간격.
const TYPE_INTERVAL_MS: u64 = 600;
/// 선택 순환 간격 — 연결선이 디스플레이 경계를 넘는 것을 보기 위한 것.
const CYCLE_INTERVAL_MS: u64 = 1500;

/// 이 실행이 데모 모드인가.
#[must_use]
pub fn enabled() -> bool {
    std::env::var_os("ULTRAKEY_OVERLAY_DEMO").is_some()
}

/// 데모를 기동한다. ⚠️ **`setup()` 안, 메인 스레드에서** 불러야 한다 —
/// `NSScreen` 열거가 메인 스레드 전용이다.
pub fn start(app: &tauri::AppHandle, store_origin: Option<(f64, f64)>) {
    // ⭐ 패닉을 반드시 로그 파일에 남긴다. `open` 으로 띄우면 stderr 가
    // 사라지므로, 이것이 없으면 "앱이 그냥 없어졌다" 밖에 관측되지 않는다 —
    // 실제로 이 검증 중에 그 상황을 겪었다.
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(panic = %info, "panic");
        previous(info);
    }));

    let displays = overlay_displays();
    if displays.is_empty() {
        tracing::error!("failed to get any NSScreen; aborting demo");
        return;
    }
    for d in &displays {
        tracing::info!(
            display_id = d.display_id,
            x = d.frame.x,
            y = d.frame.y,
            w = d.frame.width,
            h = d.frame.height,
            backing_scale = d.backing_scale,
            "display"
        );
    }
    if let Some(u) = ultrakey_overlay::geometry::union_frame(&displays) {
        tracing::info!(x = u.x, y = u.y, w = u.width, h = u.height, "union frame");
    }

    // ⭐(이슈 #95) 검색 바 위치 — 프로덕션(`seek.rs`)과 같은 결정 경로(§3.4):
    // 표시 디스플레이는 마우스 → 포커스 → 주 디스플레이 순위로 매번 새로
    // 고르고, 저장된 위치는 상대 오프셋으로만 해석한다.
    let mouse = ultrakey_platform::screens::mouse_location();
    let focused_display = ultrakey_platform::screens::focused_display_id();
    let origin = ultrakey_overlay::geometry::resolve_search_bar_origin(
        &displays,
        mouse,
        focused_display,
        store_origin,
    );
    tracing::info!(?mouse, ?focused_display, ?origin, "search bar origin resolved (§3.4 priority rule)");

    let shared = app.state::<Arc<Mutex<SurfaceState>>>().inner().clone();
    let app_for_persist = app.clone();
    // 드래그 중에는 `Moved` 가 초당 수십 번 온다. 마지막 값만 들고 있다가
    // 아래 루프가 주기적으로 flush 한다 — 파일 쓰기를 드래그마다 한 번으로 줄인다.
    let pending_move: Arc<Mutex<Option<(f64, f64)>>> = Arc::new(Mutex::new(None));
    let pending_for_cb = pending_move.clone();

    let renderer = WebviewOverlayRenderer::new(app.clone(), shared)
        .on_search_bar_moved(move |x, y| {
            *pending_for_cb.lock().unwrap() = Some((x, y));
        });

    // ⭐ §3.5 — 팔레트는 **시스템 외관 모드**를 따르고, 애니메이션은 축소된
    // 모션 설정을 따른다. 둘 다 메인 스레드에서 읽는다.
    let dark = ultrakey_platform::screens::is_dark_appearance().unwrap_or(true);
    let appearance = if dark { Appearance::Dark } else { Appearance::Light };
    let reduce_motion = ultrakey_platform::screens::should_reduce_motion();
    tracing::info!(dark, reduce_motion, "appearance/motion settings");

    // ⭐ 핫플러그(§5 #1) — `NSApplicationDidChangeScreenParametersNotification`.
    // 옵저버는 메인 스레드에서 만들고 앱 수명 내내 살아 있어야 하므로 누수시킨다
    // (`ScreenObserver` 는 `Retained` 를 들고 있어 `Send` 가 아니다 — 다른
    // 스레드로 옮길 수 없고, 옮길 이유도 없다).
    let hotplug = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let hotplug_for_obs = hotplug.clone();
    let observer = ultrakey_platform::screens::ScreenObserver::start(move || {
        tracing::warn!("received display configuration change notification (hotplug)");
        hotplug_for_obs.store(true, std::sync::atomic::Ordering::Release);
    });
    Box::leak(Box::new(observer));

    let app_thread = app.clone();
    std::thread::Builder::new()
        .name("ultrakey-overlay-demo".into())
        .spawn(move || {
            tracing::warn!(
                secs = ARM_DELAY_SECS,
                "waiting before showing overlay; click another app now to place the cursor"
            );
            std::thread::sleep(Duration::from_secs(ARM_DELAY_SECS));

            // ⭐ S-6 — 검출을 기다리지 않고 **즉시** 연다.
            let opened = Instant::now();
            let controller = Arc::new(Mutex::new(OverlayController::open(
                displays,
                appearance,
                reduce_motion,
                origin,
                Box::new(renderer),
            )));
            tracing::warn!(
                elapsed_ms = opened.elapsed().as_secs_f64() * 1000.0,
                "overlay opened with zero candidates (S-6); the original app's cursor should still be alive now"
            );

            // ── 검출 — 결과를 디스플레이 단위로 **도착하는 대로** 밀어 넣는다.
            //
            // ⭐ AX 소스(§3.3)는 `ULTRAKEY_OVERLAY_DEMO_AX` 로만 켠다. 출고
            // 기본값이 꺼짐이고(§3.5 마지막 줄), 켜야만 "AX 매치는 요소 전체가
            // 하이라이트된다"는 §3.5 의 구분이 화면에 나타난다.
            let use_ax = std::env::var_os("ULTRAKEY_OVERLAY_DEMO_AX").is_some();
            let params = DetectionParams { use_accessibility: use_ax, ..Default::default() };

            // ⭐⭐ 질의를 **검출보다 먼저** 건다. 이것이 실제 시나리오다 —
            // 사용자는 Seek 을 열자마자 타이핑을 시작하고, 검출 결과는 그
            // 뒤에(주 디스플레이 ≈ 335 ms, 전체 ≈ 775 ms) 도착한다. 질의를
            // 먼저 걸어 두어야 **하이라이트가 눈앞에서 늘어나는 것**을 볼 수
            // 있다(S-6 의 육안 판정). 빈 질의로는 아무것도 안 그려진다(§3.1).
            let query = std::env::var("ULTRAKEY_OVERLAY_DEMO_QUERY").unwrap_or_else(|_| "se".into());
            let first: String = query.chars().take(1).collect();
            controller.lock().unwrap().set_query(&first);
            tracing::warn!(query = %first, "query set before detection; highlights should now grow incrementally");
            let ctrl_for_detect = controller.clone();
            let detect_started = Instant::now();
            let outcome = ultrakey_seek::detect_candidates(&params, |result| {
                let n = result.candidates.len();
                let at = detect_started.elapsed().as_secs_f64() * 1000.0;
                ctrl_for_detect
                    .lock()
                    .unwrap()
                    .ingest_display(result.display_id, result.candidates.clone());
                tracing::warn!(
                    display_id = result.display_id,
                    candidates = n,
                    arrived_at_ms = at,
                    "incremental result received; highlights should grow on screen at this point"
                );
            });
            {
                let mut c = controller.lock().unwrap();
                // ⭐ `on_display` 는 OCR 만 준다(디스플레이 단위라서). AX 후보는
                // 디스플레이를 특정하지 않으므로(§3.3 B4) 병합 결과에서 골라
                // `ingest_extra` 로 따로 넣는다.
                let ax_only: Vec<_> = outcome
                    .candidates
                    .iter()
                    .filter(|c| c.source == ultrakey_seek::CandidateSource::Accessibility)
                    .cloned()
                    .collect();
                if !ax_only.is_empty() {
                    tracing::warn!(count = ax_only.len(), "adding AX candidates; should render with a dashed border");
                    c.ingest_extra(ax_only);
                }
                c.finish_detection();
                tracing::warn!(
                    total_ms = outcome.total_ms,
                    capture_ms = outcome.capture_ms,
                    ocr = outcome.ocr_count,
                    ax = outcome.ax_count,
                    merged = outcome.candidates.len(),
                    screen_recording = ?outcome.screen_recording,
                    matches = c.match_count(),
                    "detection complete"
                );
                if outcome.ocr_blocked_by_permission() {
                    tracing::error!(
                        "no screen recording permission; candidates are menu bar only. \
                         grant permission in system settings and re-run (same trap as F-02 verification)"
                    );
                }
            }

            // ── 타이핑 흉내 — §3.6 의 "매 키 입력마다 재렌더링" 경로.
            std::thread::sleep(Duration::from_millis(1200));
            let mut typed = first.clone();
            for ch in query.chars().skip(1) {
                typed.push(ch);
                let t0 = Instant::now();
                let count = {
                    let mut c = controller.lock().unwrap();
                    c.set_query(&typed);
                    c.match_count()
                };
                tracing::warn!(
                    query = %typed,
                    matches = count,
                    dispatch_ms = t0.elapsed().as_secs_f64() * 1000.0,
                    "key input; re-rendering"
                );
                std::thread::sleep(Duration::from_millis(TYPE_INTERVAL_MS));
            }

            // ── 순환 — 선택이 디스플레이를 넘나들며 연결선이 경계를 넘는지 본다.
            tracing::warn!(
                "cycling selection every {CYCLE_INTERVAL_MS} ms from now; \
                 watch whether the connector line breaks at display boundaries. quit the app to stop"
            );
            let mut tick: u32 = 0;
            loop {
                tick += 1;
                {
                    let mut c = controller.lock().unwrap();
                    // ⭐ 5회마다 한 번은 역방향(⇧Tab, §9 항목 14)으로 돈다 —
                    // 앞뒤 순환이 같은 지점으로 돌아오는지 눈으로 볼 수 있다.
                    if tick % 5 == 0 {
                        c.cycle_prev();
                    } else {
                        c.cycle_next();
                    }
                    if let Some(sel) = c.selected() {
                        tracing::info!(
                            text = %sel.text,
                            x = sel.frame.x,
                            y = sel.frame.y,
                            display = ?sel.display_id,
                            source = ?sel.source,
                            "selection moved"
                        );
                    }
                }
                // 드래그로 밀린 검색 바 위치를 여기서 한 번에 저장한다.
                if let Some((x, y)) = pending_move.lock().unwrap().take() {
                    if let Some(state) = app_for_persist.try_state::<Arc<crate::AppState>>() {
                        let mut store = state.store.lock().unwrap();
                        persist_search_bar_origin(&mut store, x, y);
                        tracing::warn!(x, y, "persisted search bar position (§3.4)");
                    }
                    controller.lock().unwrap().search_bar_moved(x, y);
                }
                // 핫플러그 알림이 왔을 때만 다시 열거한다(§5 #1).
                // ⚠️ `NSScreen` 열거는 메인 스레드 전용이라 여기서 직접 못 부른다 —
                // 메인 스레드에 물어 결과를 채널로 받는다.
                if hotplug.swap(false, std::sync::atomic::Ordering::AcqRel) {
                    if let Some(next) = fetch_displays(&app_thread) {
                        controller.lock().unwrap().maybe_refresh_displays(next);
                    }
                }
                // ⭐ 20회마다 한 번은 숨겼다 다시 띄운다 — §8 수용 기준
                // "세션 종료 시 오버레이가 즉시 숨겨지고 하이라이트·연결선
                // 잔상이 남지 않는다" 를 반복 관찰할 수 있게.
                if tick % 20 == 0 {
                    tracing::warn!("hiding overlay; no residual artifacts should remain");
                    controller.lock().unwrap().close();
                    std::thread::sleep(Duration::from_millis(1500));
                    tracing::warn!("showing overlay again");
                    controller.lock().unwrap().reopen();
                }
                std::thread::sleep(Duration::from_millis(CYCLE_INTERVAL_MS));
            }
        })
        .expect("데모 스레드를 만들지 못했다");
}

/// 메인 스레드에 `NSScreen` 열거를 부탁하고 결과를 받아 온다.
fn fetch_displays(app: &tauri::AppHandle) -> Option<Vec<ultrakey_overlay::geometry::OverlayDisplay>> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(overlay_displays());
    })
    .ok()?;
    rx.recv_timeout(Duration::from_millis(500)).ok()
}

/// 저장소에서 검색 바 위치를 읽어 온다 — `setup()` 이 부른다.
#[must_use]
pub fn read_stored_origin(store: &ultrakey_core::settings::SettingsStore) -> Option<(f64, f64)> {
    stored_search_bar_origin(store)
}
