//! ⭐ F-03 — Seek 오버레이 배선. 명세: `docs/spec/seek-overlay-ui.md`.
//!
//! 이 파일이 하는 일은 셋뿐이고, 그 밖의 판단은 전부 다른 곳에 있다:
//!
//! | 무엇 | 어디 |
//! | :--- | :--- |
//! | 좌표 변환·연결선 클리핑·상한 정책·증분 누적 | `ultrakey-overlay` (순수, 테스트 가능) |
//! | `NSWindow` 레벨·collection behavior·활성화 없이 표시 | `ultrakey_platform::overlay_window` (모든 `unsafe`) |
//! | **창 만들기·payload 보내기·설정 영속화** | ⭐ 여기 |
//!
//! ## ⭐ 이 파일이 지키는 두 가지 실측된 계약
//!
//! 1. **S-6 (F-02 스파이크)** — 세션은 검출을 기다리지 않는다. [`OverlayController::open`]
//!    은 후보 **0개**로 즉시 창을 띄우고, [`OverlayController::ingest_display`] 가
//!    디스플레이별 결과를 도착하는 대로(주 디스플레이 ≈ 335 ms, 전체 ≈ 775 ms)
//!    채워 넣는다.
//! 2. **R-2 (F-03 스파이크)** — 그리기는 `<canvas>` 다. 이 파일은 렌더 모델을
//!    JSON 으로 emit 하기만 하고, 그리는 방식은 `ui/overlay-highlight.html` 이
//!    갖는다. 절대배치 DOM 으로 되돌아가면 후보 500개에서 예산을 넘는다.
//!
//! ## ⚠️ 준비되기 전에 보낸 프레임은 사라진다
//!
//! 웹뷰가 `listen()` 을 붙이기 **전에** `emit` 하면 그 프레임은 그냥 없어진다
//! (P3 스파이크에서 실제로 밟았다). S-6 때문에 이것이 치명적이다 — 세션 개시
//! 직후의 "후보 0개" 프레임이 사라지면 그 뒤 335 ms 동안 아무것도 안 그려진
//! 빈 창만 남는다. 그래서 [`SurfaceState`] 가 **마지막 프레임을 창별로 보관**
//! 하고, 웹뷰가 [`overlay_surface_ready`] 로 신고하는 순간 그것을 다시 보낸다.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use tauri::{Emitter, EventTarget, Manager};

use ultrakey_core::settings::{keys, SettingsStore};
use ultrakey_overlay::geometry::OverlayDisplay;
use ultrakey_overlay::model::{OverlayFrame, SearchBarFrame};
use ultrakey_overlay::palette::Appearance;
use ultrakey_overlay::renderer::{OverlayRenderer, RenderError};
use ultrakey_overlay::session::{OverlaySession, SEARCH_BAR_HEIGHT_PT, SEARCH_BAR_WIDTH_PT};
use ultrakey_platform::overlay_window::{NsWindowHandle, OverlayWindowKind, Sharing};
use ultrakey_platform::screens::{self, ScreenInfo};

/// 검색 바 창의 라벨. capabilities(`capabilities/overlay.json`)가 이 이름을
/// 글롭으로 허용한다 — 바꾸면 그쪽도 같이 바꿔야 한다.
pub const SEARCH_BAR_LABEL: &str = "seek-bar";

/// 디스플레이별 하이라이트 창의 라벨을 만든다 (`overlay-<display_id>`).
#[must_use]
pub fn highlight_label(display_id: u32) -> String {
    format!("overlay-{display_id}")
}

/// 매치 목록 한 행의 높이(px). 창 높이를 계산하는 데 쓴다.
const MATCH_ROW_PX: f64 = 26.0;
/// 검색 바 아래에 한 번에 보여 주는 매치 행의 최대 개수.
const MATCH_ROWS_VISIBLE: usize = 8;

/// 웹뷰 창들의 공유 상태. `#[tauri::command]` 와 렌더러가 함께 본다.
#[derive(Default)]
pub struct SurfaceState {
    /// `listen()` 을 붙였다고 신고한 창 라벨.
    ready: HashSet<String>,
    /// 창 라벨 → 마지막으로 보낸 하이라이트 프레임.
    last_frames: HashMap<String, OverlayFrame>,
    /// 마지막으로 보낸 검색 바 프레임.
    last_bar: Option<SearchBarFrame>,
}

/// ⭐ 웹뷰 렌더러 — `platform-constraints.md` §4.3 이 요구한 "교체 가능한
/// 모듈" 의 **현재 구현**. 실측(R-1)이 이것으로 충분하다고 판정했다. 예산을
/// 넘기는 환경이 나오면 같은 [`OverlayRenderer`] 트레이트에 `CALayer` 구현이
/// 들어오고 이 타입은 그대로 남는다.
pub struct WebviewOverlayRenderer {
    app: tauri::AppHandle,
    shared: Arc<Mutex<SurfaceState>>,
    /// 현재 살아 있는 하이라이트 창: `display_id` → 라벨.
    surfaces: HashMap<u32, String>,
    /// 검색 바 창을 이미 만들었는가.
    bar_created: bool,
    visible: bool,
    /// 사용자가 검색 바를 끌어 옮겼을 때 불린다 — 영속화(F-15)는 호출자 몫이다.
    on_bar_moved: Option<Arc<dyn Fn(f64, f64) + Send + Sync>>,
}

impl WebviewOverlayRenderer {
    /// 렌더러를 만든다. 창은 아직 만들지 않는다(§3.1 미생성 상태).
    pub fn new(app: tauri::AppHandle, shared: Arc<Mutex<SurfaceState>>) -> Self {
        Self {
            app,
            shared,
            surfaces: HashMap::new(),
            bar_created: false,
            visible: false,
            on_bar_moved: None,
        }
    }

    /// 검색 바가 움직였을 때 부를 콜백을 건다(§3.4 위치 저장).
    #[must_use]
    pub fn on_search_bar_moved(
        mut self,
        f: impl Fn(f64, f64) + Send + Sync + 'static,
    ) -> Self {
        self.on_bar_moved = Some(Arc::new(f));
        self
    }

    /// 창 하나를 만들고 `ns_window()` 로 네이티브 설정을 건다.
    ///
    /// ⚠️ **메인 스레드로 비동기 디스패치**한다(`architecture.md` §2, `main.rs`
    /// 의 `on_main_thread` 와 같은 규약) — 창 조작을 다른 스레드에서 동기로
    /// 기다리면 그 스레드가 메인 스레드를 붙잡는다.
    fn spawn_window(&self, label: String, url: &'static str, kind: OverlayWindowKind, rect: (f64, f64, f64, f64)) {
        let app = self.app.clone();
        let (x, y, w, h) = rect;
        let moved_cb = self.on_bar_moved.clone();
        let dispatched = self.app.run_on_main_thread(move || {
            if app.get_webview_window(&label).is_some() {
                return;
            }
            let built = tauri::WebviewWindowBuilder::new(
                &app,
                &label,
                // ⚠️ 쿼리 문자열을 붙이지 않는다 — `WebviewUrl::App` 은
                // `PathBuf` 라 `?` 가 퍼센트 인코딩되어 404 가 된다(P3
                // 스파이크에서 실제로 밟았다). 창 이름은 JS 가
                // `getCurrentWebviewWindow().label` 로 직접 읽는다.
                tauri::WebviewUrl::App(url.into()),
            )
            .title("Ultrakey Seek")
            .position(x, y)
            .inner_size(w, h)
            .decorations(false)
            .transparent(true)
            .always_on_top(true)
            .visible_on_all_workspaces(true)
            .shadow(false)
            .resizable(false)
            .skip_taskbar(true)
            // ⭐ 창은 만들되 **보이지 않게** 시작한다. 표시는 반드시
            // `orderFrontRegardless` 를 거친다 — Tauri 의 `show()` 는 앱을
            // 활성화시킬 수 있고, 그러면 하위 앱의 포커스가 깨진다(§1).
            .visible(false)
            .build();

            let window = match built {
                Ok(w) => w,
                Err(e) => {
                    tracing::error!(%label, error = %e, "failed to create overlay window");
                    return;
                }
            };

            // 클릭 통과는 Tauri 로도 된다(§3.2). 하이라이트 창만이다 —
            // 검색 바는 드래그를 받아야 한다(§3.2 정정).
            let _ = window.set_ignore_cursor_events(kind == OverlayWindowKind::Highlight);

            // ⭐ 검색 바를 끌어 옮기면 그 위치를 저장한다(§3.4, §8 수용 기준).
            //
            // ⚠️ `Moved` 는 드래그하는 **내내** 초당 수십 번 온다. 매번
            // `settings.json` 을 원자적으로 다시 쓰면 드래그 한 번에 파일
            // 쓰기가 수백 번이다 — 그래서 콜백 쪽에서 합친다(호출자 책임).
            // 여기서는 물리 → 논리 좌표 변환만 하고 그대로 넘긴다.
            if kind == OverlayWindowKind::SearchBar {
                if let Some(cb) = moved_cb {
                    let scale = window.scale_factor().unwrap_or(1.0);
                    window.on_window_event(move |event| {
                        if let tauri::WindowEvent::Moved(pos) = event {
                            cb(f64::from(pos.x) / scale, f64::from(pos.y) / scale);
                        }
                    });
                }
            }

            // ⭐ 여기가 이 기능의 핵심 — Tauri 설정에 없는 넷을 건다.
            let raw = match window.ns_window() {
                Ok(p) => p,
                Err(e) => {
                    tracing::error!(%label, error = %e, "ns_window() failed; could not apply native configuration");
                    return;
                }
            };
            // ⭐ `window` 를 손에 쥔 채, 메인 스레드에서, 창이 살아 있는
            // 동안에만 손잡이를 만든다 — `from_tauri_ptr` 이 문서에 적어 둔
            // 신뢰 조건이 여기서 지켜진다.
            let Some(handle) = NsWindowHandle::from_tauri_ptr(raw) else {
                return;
            };
            let ok = handle.configure(kind, Sharing::ReadOnly);
            // ⭐ 스타일 마스크만으로는 `canBecomeKey` 가 꺼지지 않는다 — tao 가
            // 그것을 무조건 YES 로 오버라이드해 두었다(실기기 반증). 런타임
            // 서브클래스로 눌러야 한다.
            // 진단용 킬 스위치 — 클래스 바꿔 끼우기를 끄고 원인을 가른다.
            let non_activating = if std::env::var_os("ULTRAKEY_OVERLAY_NO_SWAP").is_some() {
                false
            } else {
                handle.force_non_activating()
            };

            // ⭐ 걸린 값을 되읽어 로그로 남긴다 — 실기기 검증이 "포커스를 안
            // 뺏는다"를 눈으로만이 아니라 **값으로도** 대조할 수 있게.
            tracing::info!(
                %label,
                ?kind,
                configured = ok,
                non_activating = non_activating,
                can_become_key = ?handle.can_become_key(),
                level = ?handle.level(),
                collection_behavior = ?handle.collection_behavior(),
                // ⭐ 창이 **정말 그 디스플레이를 덮고 있는지** 값으로 남긴다.
                // 요청한 위치와 실제 위치가 다르면(예: 음수 원점을 창 서버가
                // 옮겨 버리면) 그 화면의 하이라이트가 통째로 어긋난다.
                requested = ?(x, y, w, h),
                outer_position = ?window.outer_position(),
                outer_size = ?window.outer_size(),
                scale_factor = ?window.scale_factor(),
                "overlay window native configuration applied"
            );
        });
        if let Err(e) = dispatched {
            tracing::error!(error = %e, "failed to dispatch overlay window creation to the main thread");
        }
    }

    /// 창 하나에 대해 네이티브 표시/숨김을 건다.
    fn order(&self, label: String, front: bool) {
        let app = self.app.clone();
        let dispatched = self.app.run_on_main_thread(move || {
            let Some(window) = app.get_webview_window(&label) else {
                tracing::warn!(%label, "window not found; could not apply show/hide");
                return;
            };
            let Ok(raw) = window.ns_window() else { return };
            // `window` 를 손에 쥔 채 메인 스레드에서 만든다(위와 같은 조건).
            let Some(handle) = NsWindowHandle::from_tauri_ptr(raw) else { return };
            if front {
                // ⭐ `show()`/`set_focus()` 가 아니다. `orderFrontRegardless` 만이
                // 앱을 활성화하지 않고 창을 올린다(§3.1 표시 행).
                handle.order_front_regardless();
            } else {
                handle.order_out();
            }
        });
        if let Err(e) = dispatched {
            tracing::error!(error = %e, "failed to dispatch show/hide to the main thread");
        }
    }

    /// 준비된 창에만 보낸다. 아직 준비 안 된 창의 몫은 [`SurfaceState`] 에
    /// 남아 있다가 [`overlay_surface_ready`] 가 다시 보낸다.
    fn emit_frame(&self, label: &str, frame: &OverlayFrame) {
        let ready = {
            let mut shared = self.shared.lock().unwrap();
            shared.last_frames.insert(label.to_string(), frame.clone());
            shared.ready.contains(label)
        };
        if !ready {
            return;
        }
        if let Err(e) = self
            .app
            .emit_to(EventTarget::webview_window(label), "overlay://frame", frame)
        {
            tracing::warn!(%label, error = %e, "failed to emit frame");
        }
    }
}

impl OverlayRenderer for WebviewOverlayRenderer {
    fn sync_surfaces(&mut self, displays: &[OverlayDisplay]) -> Result<(), RenderError> {
        // 검색 바는 세션과 무관하게 한 번만 만든다(§3.1 — 재생성 비용을 피해
        // 상주시킨다).
        if !self.bar_created {
            self.spawn_window(
                SEARCH_BAR_LABEL.to_string(),
                "overlay-searchbar.html",
                OverlayWindowKind::SearchBar,
                (0.0, 0.0, SEARCH_BAR_WIDTH_PT, SEARCH_BAR_HEIGHT_PT),
            );
            self.bar_created = true;
        }

        let wanted: HashMap<u32, String> =
            displays.iter().map(|d| (d.display_id, highlight_label(d.display_id))).collect();

        // 사라진 디스플레이의 창을 닫는다(§5 엣지케이스 1 — 핫플러그 해제).
        for (id, label) in &self.surfaces {
            if wanted.contains_key(id) {
                continue;
            }
            let app = self.app.clone();
            let to_close = label.clone();
            let _ = self.app.run_on_main_thread(move || {
                if let Some(w) = app.get_webview_window(&to_close) {
                    tracing::info!(label = %to_close, "display disappeared; closing overlay window");
                    let _ = w.close();
                }
            });
            let mut shared = self.shared.lock().unwrap();
            shared.ready.remove(label.as_str());
            shared.last_frames.remove(label.as_str());
        }

        // 새로 붙은 디스플레이의 창을 만든다.
        for d in displays {
            let label = highlight_label(d.display_id);
            if self.surfaces.contains_key(&d.display_id) {
                continue;
            }
            self.spawn_window(
                label,
                "overlay-highlight.html",
                OverlayWindowKind::Highlight,
                (d.frame.x, d.frame.y, d.frame.width, d.frame.height),
            );
        }

        self.surfaces = wanted;
        Ok(())
    }

    fn present(&mut self, frames: &[OverlayFrame], bar: &SearchBarFrame) -> Result<(), RenderError> {
        for frame in frames {
            let Some(label) = self.surfaces.get(&frame.display_id).cloned() else {
                tracing::warn!(display_id = frame.display_id, "no window for this display; dropping frame");
                continue;
            };
            // ⭐ 연결선이 이 창 몫으로 잘려 들어왔는지까지 남긴다 — 다중
            // 디스플레이 회귀(v1.55)를 로그만으로 판정할 수 있게.
            tracing::debug!(
                display_id = frame.display_id,
                highlights = frame.highlights.len(),
                omitted = frame.omitted,
                line = ?frame.line,
                "frame"
            );
            self.emit_frame(&label, frame);
        }

        // 검색 바 창의 높이는 매치 목록 길이를 따라간다. 매 키 입력마다
        // `set_size` 를 부르면 창 서버 왕복이 늘어나므로, **행 수가 실제로
        // 달라졌을 때만** 부른다.
        let rows = bar.matches.len().min(MATCH_ROWS_VISIBLE);
        let height = SEARCH_BAR_HEIGHT_PT + rows as f64 * MATCH_ROW_PX;
        let changed = {
            let mut shared = self.shared.lock().unwrap();
            let prev_rows =
                shared.last_bar.as_ref().map(|b| b.matches.len().min(MATCH_ROWS_VISIBLE));
            shared.last_bar = Some(bar.clone());
            prev_rows != Some(rows)
        };
        if changed {
            let app = self.app.clone();
            let _ = self.app.run_on_main_thread(move || {
                if let Some(w) = app.get_webview_window(SEARCH_BAR_LABEL) {
                    let _ = w.set_size(tauri::LogicalSize::new(SEARCH_BAR_WIDTH_PT, height));
                }
            });
        }

        let bar_ready = self.shared.lock().unwrap().ready.contains(SEARCH_BAR_LABEL);
        if bar_ready {
            if let Err(e) = self.app.emit_to(
                EventTarget::webview_window(SEARCH_BAR_LABEL),
                "overlay://searchbar",
                bar,
            ) {
                tracing::warn!(error = %e, "failed to emit search bar");
            }
        }
        Ok(())
    }

    fn set_search_bar_origin(&mut self, x: f64, y: f64) -> Result<(), RenderError> {
        let app = self.app.clone();
        let _ = self.app.run_on_main_thread(move || {
            if let Some(w) = app.get_webview_window(SEARCH_BAR_LABEL) {
                let _ = w.set_position(tauri::LogicalPosition::new(x, y));
            }
        });
        Ok(())
    }

    fn show(&mut self) -> Result<(), RenderError> {
        // ⭐ Tauri 의 `show()` 로 창을 화면에 붙이고, 그다음
        // `orderFrontRegardless` 로 **우리가 건 레벨대로** 올린다.
        // ⛔ `set_focus()` 는 절대 부르지 않는다.
        for label in self.surfaces.values().cloned().chain(std::iter::once(SEARCH_BAR_LABEL.to_string())) {
            let app = self.app.clone();
            let l = label.clone();
            let _ = self.app.run_on_main_thread(move || {
                if let Some(w) = app.get_webview_window(&l) {
                    // Tauri 의 `show()` 는 내부적으로 `makeKeyAndOrderFront:` 로
                    // 내려간다. ⭐ 그런데 `force_non_activating` 이 이미 이 창을
                    // **키 윈도우가 될 수 없게** 만들어 두었으므로, 그 호출은
                    // 사실상 `orderFront:` 로 격하된다 — 포커스를 뺏지 못한다.
                    // (스타일 마스크만 믿고 이 호출을 남겨 두면 안 된다. 순서가
                    // 뒤집히면 그대로 포커스를 뺏는다.)
                    let _ = w.show();
                }
            });
            self.order(label, true);
        }
        self.visible = true;
        Ok(())
    }

    fn hide(&mut self) -> Result<(), RenderError> {
        for label in self.surfaces.values().cloned().chain(std::iter::once(SEARCH_BAR_LABEL.to_string())) {
            self.order(label, false);
        }
        self.visible = false;
        Ok(())
    }
}

/// 웹뷰가 `listen()` 을 붙였다고 신고한다. ⭐ 보관해 둔 마지막 프레임을 즉시
/// 다시 보낸다 — 이것이 없으면 세션 개시 직후의 "후보 0개" 프레임이 사라진다
/// (모듈 문서의 경고 참조).
#[tauri::command]
pub fn overlay_surface_ready(app: tauri::AppHandle, label: String) {
    let shared = app.state::<Arc<Mutex<SurfaceState>>>().inner().clone();
    let (frame, bar) = {
        let mut s = shared.lock().unwrap();
        s.ready.insert(label.clone());
        (s.last_frames.get(&label).cloned(), s.last_bar.clone())
    };
    tracing::debug!(%label, has_frame = frame.is_some(), "overlay surface ready");
    if label == SEARCH_BAR_LABEL {
        if let Some(bar) = bar {
            let _ =
                app.emit_to(EventTarget::webview_window(&label), "overlay://searchbar", &bar);
        }
    } else if let Some(frame) = frame {
        let _ = app.emit_to(EventTarget::webview_window(&label), "overlay://frame", &frame);
    }
}

/// 검색 바의 매치 목록에서 행을 클릭했다.
///
/// ⚠️ 선택을 실제로 옮기는 것은 F-01(세션 상태 머신)의 책임이라 이 위임의
/// 범위 밖이다. 지금은 요청을 로그로만 남긴다 — 웹뷰 쪽이 실패를 조용히
/// 삼키도록 되어 있어(`.catch`) 이 커맨드가 없어도 UI 는 깨지지 않지만,
/// 있으면 F-01 이 붙일 자리가 명확해진다.
#[tauri::command]
pub fn overlay_select_match(index: usize) {
    tracing::debug!(index, "search bar match row clicked; applying the selection is F-01's scope");
}

// ── 검색 바 위치 영속화 (F-15 "부재 = 기본값") ──────────────────────────────

/// 저장된 검색 바 위치를 읽는다. **키가 없으면 `None`** — 그때의 기본 위치는
/// [`default_search_bar_origin`] 이 정한다(§3.4 `(추정)`).
#[must_use]
pub fn stored_search_bar_origin(store: &SettingsStore) -> Option<(f64, f64)> {
    let x: f64 = store.get(keys::SEEK_SEARCH_BAR_X)?;
    let y: f64 = store.get(keys::SEEK_SEARCH_BAR_Y)?;
    Some((x, y))
}

/// 검색 바 위치를 즉시 기록한다(원본의 `persistPosition` 과 같은 규약, §1.1).
pub fn persist_search_bar_origin(store: &mut SettingsStore, x: f64, y: f64) {
    if let Err(e) = store.set(keys::SEEK_SEARCH_BAR_X, &x) {
        tracing::error!(error = %e, "failed to persist search bar x");
    }
    if let Err(e) = store.set(keys::SEEK_SEARCH_BAR_Y, &y) {
        tracing::error!(error = %e, "failed to persist search bar y");
    }
}

/// ⭐ 최초 기본 위치 — **커서가 있는 디스플레이의 상단부 중앙**(§3.4 제안,
/// 근거는 Spotlight 의 배치 관행 `(추정)`).
///
/// 커서 위치를 못 얻으면 주 디스플레이로 떨어진다.
#[must_use]
pub fn default_search_bar_origin(displays: &[OverlayDisplay], mouse: Option<(f64, f64)>) -> (f64, f64) {
    let target = mouse
        .and_then(|(mx, my)| ultrakey_overlay::geometry::display_for_point(displays, mx, my))
        .or_else(|| displays.first());
    let Some(d) = target else { return (0.0, 0.0) };
    (
        d.frame.x + (d.frame.width - SEARCH_BAR_WIDTH_PT) / 2.0,
        // 화면 상단에서 1/5 지점 — Spotlight 와 비슷한 높이 `(추정)`.
        d.frame.y + d.frame.height * 0.2,
    )
}

// ── 디스플레이 열거 ─────────────────────────────────────────────────────────

/// `NSScreen` 목록을 오버레이의 디스플레이 모델로 옮긴다.
///
/// ⚠️ **메인 스레드에서만 부른다** — `NSScreen.screens` 가 메인 스레드
/// 전용이기 때문이다([`screens::screens`] 참조).
#[must_use]
pub fn overlay_displays() -> Vec<OverlayDisplay> {
    screens::screens().into_iter().map(to_overlay_display).collect()
}

fn to_overlay_display(s: ScreenInfo) -> OverlayDisplay {
    OverlayDisplay {
        display_id: s.display_id,
        frame: ultrakey_seek::transform::Rect {
            x: s.origin_x,
            y: s.origin_y,
            width: s.width,
            height: s.height,
        },
        backing_scale: s.backing_scale,
    }
}

// ── 컨트롤러 ────────────────────────────────────────────────────────────────

/// 세션 하나와 렌더러 하나를 묶는다. F-01 이 생기면 이 타입을 그대로 쓴다.
pub struct OverlayController {
    session: OverlaySession,
    renderer: Box<dyn OverlayRenderer + Send>,
}

impl OverlayController {
    /// ⭐ 세션을 **즉시** 연다 — 검출을 기다리지 않는다(S-6).
    pub fn open(
        displays: Vec<OverlayDisplay>,
        appearance: Appearance,
        reduce_motion: bool,
        search_bar_origin: (f64, f64),
        mut renderer: Box<dyn OverlayRenderer + Send>,
    ) -> Self {
        let mut session = OverlaySession::open(displays.clone(), appearance, reduce_motion);
        session.set_search_bar_origin(search_bar_origin.0, search_bar_origin.1);
        let _ = renderer.sync_surfaces(&displays);
        let _ = renderer.set_search_bar_origin(search_bar_origin.0, search_bar_origin.1);
        let _ = renderer.show();
        let mut me = Self { session, renderer };
        me.repaint();
        me
    }

    /// F-02 `on_display` 콜백의 소비자 (S-1·S-6).
    pub fn ingest_display(&mut self, display_id: u32, candidates: Vec<ultrakey_seek::TextCandidate>) {
        self.session.ingest_display(display_id, candidates);
        self.repaint();
    }

    /// 디스플레이가 특정되지 않는 후보(AX 등).
    pub fn ingest_extra(&mut self, candidates: Vec<ultrakey_seek::TextCandidate>) {
        self.session.ingest_extra(candidates);
        self.repaint();
    }

    /// 검출이 끝났다 — 검색 바의 진행 표시를 끈다.
    pub fn finish_detection(&mut self) {
        self.session.finish_detection();
        self.repaint();
    }

    /// 키 입력 — 질의 갱신(§3.6 트리거).
    pub fn set_query(&mut self, query: &str) {
        self.session.set_query(query);
        self.repaint();
    }

    /// 순환(↑/↓/Tab/`;`).
    pub fn cycle_next(&mut self) {
        self.session.cycle_next();
        self.repaint();
    }

    /// 역방향 순환(⇧Tab). 지원 여부 자체는 §9 항목 14 로 미확정이지만,
    /// 구현 비용이 0 에 가까워 넣어 둔다.
    pub fn cycle_prev(&mut self) {
        self.session.cycle_prev();
        self.repaint();
    }

    /// 디스플레이 구성이 **실제로** 달라졌을 때만 재배치한다.
    ///
    /// `didChangeScreenParameters` 는 배치와 무관한 변화(해상도 동일한 재연결,
    /// 색 프로파일 변경 등)에도 발화하므로, 매번 창을 다시 만들면 오버레이가
    /// 이유 없이 깜빡인다.
    pub fn maybe_refresh_displays(&mut self, displays: Vec<OverlayDisplay>) {
        if self.session.displays() == displays.as_slice() {
            return;
        }
        tracing::info!(count = displays.len(), "display configuration changed; repositioning overlay");
        self.refresh_displays(displays);
    }

    /// 핫플러그(§5 #1) — 디스플레이 구성이 바뀌었다.
    pub fn refresh_displays(&mut self, displays: Vec<OverlayDisplay>) {
        self.session.set_displays(displays.clone());
        let _ = self.renderer.sync_surfaces(&displays);
        let (x, y) = self.session.search_bar_origin();
        let _ = self.renderer.set_search_bar_origin(x, y);
        self.repaint();
    }

    /// 사용자가 검색 바를 끌어 옮겼다 — 세션에 반영한다(영속화는 호출자 몫).
    pub fn search_bar_moved(&mut self, x: f64, y: f64) {
        self.session.set_search_bar_origin(x, y);
        self.repaint();
    }

    /// 세션 종료(§3.1 숨김) — 창은 파괴하지 않고 내리기만 한다.
    ///
    /// ⭐ 창을 상주시키는 이유는 §3.1 이 `(추정)` 으로 제안한 그대로다 —
    /// 세션마다 `NSWindow` + 웹뷰를 다시 만들면 그 비용이 §3.6 의 개시 예산에
    /// 통째로 들어온다.
    pub fn close(&mut self) {
        let _ = self.renderer.hide();
    }

    /// 숨겼던 오버레이를 다시 띄운다(§3.1 표시).
    pub fn reopen(&mut self) {
        let _ = self.renderer.show();
        self.repaint();
    }

    /// 현재 선택된 후보.
    #[must_use]
    pub fn selected(&self) -> Option<&ultrakey_seek::TextCandidate> {
        self.session.selected()
    }

    /// 진단용 — 지금 몇 개를 들고 있는가.
    #[must_use]
    pub fn match_count(&self) -> usize {
        self.session.search_bar_frame().total_matches
    }

    fn repaint(&mut self) {
        let frames = self.session.frames();
        let bar = self.session.search_bar_frame();
        if let Err(e) = self.renderer.present(&frames, &bar) {
            tracing::warn!(error = ?e, "overlay render failed");
        }
    }
}
