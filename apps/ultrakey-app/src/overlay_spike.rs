//! ⭐ F-03 · P3 실측 하네스 — **제품 코드가 아니다.**
//!
//! `platform-constraints.md` §4.3 / 조사 노트 **P3** 가 남긴 단 하나의 미실측:
//! *"이미 그려진 오버레이를 키 입력마다 다시 그리는 WKWebView 왕복이
//! §3.6 예산(프레임 1개 ≈ 16 ms, 총 체감 100 ms)에 드는가."*
//!
//! F-02 가 `seek_ocr_spike` 로 검출 지연을 닫았듯, 이 모듈이 렌더링 지연을 닫는다.
//! 산출물 해석은 `docs/dev/seek-overlay-render-spike.md`.
//!
//! ## 무엇을 재는가
//!
//! ```text
//!  Rust                                   WKWebView(JS)
//!   │ t0 = Instant::now()
//!   │ emit("spike://frame", payload)  ──►  이벤트 콜백 진입
//!   │                                      DOM/Canvas 갱신   ┐ build_ms
//!   │                                      rAF × 2 (커밋)    ┘ paint_ms
//!   │ ack 수신  ◄────────────────────────  invoke("overlay_spike_ack")
//!   │ round_trip = t0.elapsed()
//! ```
//!
//! `round_trip` 은 **왕복**이라 실제 사용자 체감(편도 emit→paint)보다 ack 다리
//! 하나만큼 길다. 그 다리 길이는 `n = 0` 시행이 그대로 보여 준다 — 그려야 할
//! 것이 없을 때 남는 것이 순수 IPC 오버헤드다.
//!
//! ## 실행
//!
//! ```sh
//! open -n …/Ultrakey.app --env ULTRAKEY_OVERLAY_SPIKE=1
//! # 결과는 ~/Library/Logs/Ultrakey/ultrakey.log 와
//! # ~/Library/Logs/Ultrakey/overlay-spike.md 양쪽에 남는다.
//! ```

use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager};

/// 한 시행에서 각 창에 보내는 페이로드.
#[derive(Clone, serde::Serialize)]
struct SpikeFrame {
    seq: u64,
    mode: &'static str,
    items: Vec<SpikeItem>,
    /// 연결선 `[x1, y1, x2, y2]` — 창-로컬 좌표. 없으면 `None`.
    line: Option<[f64; 4]>,
}

/// 하이라이트 하나.
#[derive(Clone, serde::Serialize)]
struct SpikeItem {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    sel: bool,
    label: String,
}

/// 창 하나가 한 프레임을 그리고 돌려보낸 보고.
#[derive(Debug, Clone)]
pub struct SpikeAck {
    pub seq: u64,
    pub label: String,
    pub build_ms: f64,
    pub paint_ms: f64,
}

/// `#[tauri::command]` 가 ack 을 흘려보낼 통로. `manage()` 로 등록한다.
pub struct SpikeChannel {
    ack_tx: Mutex<Option<Sender<SpikeAck>>>,
    ready_tx: Mutex<Option<Sender<String>>>,
}

impl SpikeChannel {
    fn new() -> Self {
        Self { ack_tx: Mutex::new(None), ready_tx: Mutex::new(None) }
    }

    fn send_ack(&self, ack: SpikeAck) {
        if let Ok(guard) = self.ack_tx.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(ack);
            }
        }
    }

    fn send_ready(&self, label: String) {
        if let Ok(guard) = self.ready_tx.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(label);
            }
        }
    }
}

impl Default for SpikeChannel {
    fn default() -> Self {
        Self::new()
    }
}

/// 스파이크 창이 렌더를 마쳤다고 알린다.
#[tauri::command]
pub fn overlay_spike_ack(
    channel: tauri::State<'_, Arc<SpikeChannel>>,
    seq: u64,
    label: String,
    build_ms: f64,
    paint_ms: f64,
) {
    channel.send_ack(SpikeAck { seq, label, build_ms, paint_ms });
}

/// 스파이크 창이 리스너 등록을 마쳤다고 알린다.
#[tauri::command]
pub fn overlay_spike_ready(channel: tauri::State<'_, Arc<SpikeChannel>>, label: String) {
    channel.send_ready(label);
}

/// 이 실행이 스파이크 모드인가.
#[must_use]
pub fn enabled() -> bool {
    std::env::var_os("ULTRAKEY_OVERLAY_SPIKE").is_some()
}

/// 시행할 후보 개수들. 200 은 명세 §4.2 의 렌더링 상한, 500/1000 은
/// 엣지케이스 10("후보가 수백 개")을 넘어서는 최악을 일부러 본다.
const COUNTS: &[usize] = &[0, 10, 50, 100, 200, 500, 1000];
/// 렌더러 후보 — 같은 페이로드를 DOM 과 Canvas 양쪽으로 그려 비교한다.
const MODES: &[&str] = &["dom", "canvas"];
/// 각 조합 반복 횟수. 앞의 [`WARMUP`] 회는 버린다.
const REPEATS: usize = 30;
const WARMUP: usize = 5;

/// 스파이크를 기동한다. `setup()` 안에서, 창을 만들 수 있는 시점에 부른다.
pub fn start(app: &tauri::AppHandle) {
    let displays = ultrakey_platform::screen_capture::active_displays();
    if displays.is_empty() {
        tracing::error!("디스플레이를 하나도 찾지 못했다 — 스파이크를 중단한다");
        return;
    }

    let channel: Arc<SpikeChannel> = app.state::<Arc<SpikeChannel>>().inner().clone();
    let (ack_tx, ack_rx) = std::sync::mpsc::channel::<SpikeAck>();
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<String>();
    *channel.ack_tx.lock().unwrap() = Some(ack_tx);
    *channel.ready_tx.lock().unwrap() = Some(ready_tx);

    // 디스플레이마다 창 하나 — 이것이 곧 제품이 채택할 구조(안 A)이므로,
    // 스파이크도 같은 창 개수에서 재야 의미가 있다.
    let mut windows = Vec::new();
    for g in &displays {
        let label = format!("spike-{}", g.display_id);
        // ⚠️ 쿼리 문자열을 붙이지 않는다 — `WebviewUrl::App` 은 `PathBuf` 라
        // `?` 가 경로의 일부로 퍼센트 인코딩되어 페이지가 404 가 된다.
        // 창 이름은 JS 가 `getCurrentWebviewWindow().label` 로 직접 읽는다.
        let built = tauri::WebviewWindowBuilder::new(
            app,
            &label,
            tauri::WebviewUrl::App("overlay-spike.html".into()),
        )
        .title("Ultrakey overlay spike")
        .position(g.origin_x, g.origin_y)
        .inner_size(g.width_pt, g.height_pt)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .visible(true)
        .build();
        match built {
            Ok(w) => {
                let _ = w.set_ignore_cursor_events(true);
                tracing::info!(%label, w = g.width_pt, h = g.height_pt, "스파이크 창 생성");
                windows.push((label, w, *g));
            }
            Err(e) => tracing::error!(%label, error = %e, "스파이크 창 생성 실패"),
        }
    }
    if windows.is_empty() {
        return;
    }

    let app_for_thread = app.clone();
    std::thread::Builder::new()
        .name("ultrakey-overlay-spike".into())
        .spawn(move || drive(&app_for_thread, windows, &ack_rx, &ready_rx))
        .expect("스파이크 스레드를 만들지 못했다");
}

type SpikeWindow = (String, tauri::WebviewWindow, ultrakey_platform::screen_capture::DisplayGeometry);

fn drive(
    app: &tauri::AppHandle,
    windows: Vec<SpikeWindow>,
    ack_rx: &Receiver<SpikeAck>,
    ready_rx: &Receiver<String>,
) {
    // 1) 모든 창이 리스너를 붙일 때까지 기다린다. 이걸 안 하면 첫 emit 이
    //    허공으로 사라지고 ack 이 영영 안 온다.
    let mut ready = 0usize;
    let deadline = Instant::now() + Duration::from_secs(20);
    while ready < windows.len() && Instant::now() < deadline {
        match ready_rx.recv_timeout(Duration::from_millis(500)) {
            Ok(label) => {
                ready += 1;
                tracing::info!(%label, ready, total = windows.len(), "스파이크 창 준비됨");
            }
            Err(_) => continue,
        }
    }
    if ready < windows.len() {
        tracing::error!(ready, total = windows.len(), "준비되지 않은 창이 있다 — 그래도 계속한다");
    }
    // 웹뷰 첫 프레임 합성이 끝나도록 잠깐 둔다(콜드 비용을 WARMUP 이 아니라
    // 여기서 흡수시킨다).
    std::thread::sleep(Duration::from_millis(800));

    let mut report = String::new();
    report.push_str("| 렌더러 | 후보 수 | 왕복 중앙값 | 왕복 p95 | 왕복 최대 | build 중앙값 | paint 중앙값 |\n");
    report.push_str("| :--- | ---: | ---: | ---: | ---: | ---: | ---: |\n");

    let mut seq: u64 = 0;
    for mode in MODES {
        for &count in COUNTS {
            let mut round_trips = Vec::with_capacity(REPEATS);
            let mut builds = Vec::with_capacity(REPEATS);
            let mut paints = Vec::with_capacity(REPEATS);
            let mut serialize_us = Vec::with_capacity(REPEATS);

            for rep in 0..REPEATS {
                seq += 1;
                // 페이로드는 매 시행 다르게 만든다 — 같은 좌표를 반복하면
                // 웹뷰가 레이아웃을 재사용해 실제보다 빨라 보인다.
                let frames: Vec<(usize, SpikeFrame)> = windows
                    .iter()
                    .enumerate()
                    .map(|(i, (_, _, g))| {
                        (i, make_frame(seq, mode, count, g.width_pt, g.height_pt, rep))
                    })
                    .collect();

                // 직렬화 비용만 따로 — emit 안에서 일어나는 일이라 왕복에
                // 이미 포함돼 있지만, 그중 얼마가 Rust 쪽 몫인지 알아야 한다.
                let ser_t0 = Instant::now();
                for (_, f) in &frames {
                    let _ = serde_json::to_string(f);
                }
                serialize_us.push(ser_t0.elapsed().as_secs_f64() * 1_000_000.0);

                // 지난 시행의 늦은 ack 이 남아 있으면 버린다.
                while ack_rx.try_recv().is_ok() {}

                let t0 = Instant::now();
                for (i, f) in &frames {
                    // ⚠️ `emit` 은 **브로드캐스트**다 — 창마다 다른 페이로드를
                    // 보내는 이 하네스에서는 창 하나가 남의 프레임까지 그려
                    // ack 이 창 수의 제곱만큼 온다. 반드시 대상 지정이다.
                    let target = tauri::EventTarget::webview_window(windows[*i].0.clone());
                    if let Err(e) = app.emit_to(target, "spike://frame", f) {
                        tracing::warn!(error = %e, "emit 실패");
                    }
                }

                let mut got = 0usize;
                let mut build_sum = 0.0;
                let mut paint_max: f64 = 0.0;
                let wait_until = Instant::now() + Duration::from_secs(5);
                while got < windows.len() {
                    let left = wait_until.saturating_duration_since(Instant::now());
                    if left.is_zero() {
                        break;
                    }
                    match ack_rx.recv_timeout(left) {
                        Ok(a) if a.seq == seq => {
                            got += 1;
                            build_sum += a.build_ms;
                            paint_max = paint_max.max(a.paint_ms);
                        }
                        Ok(stale) => {
                            tracing::debug!(seq = stale.seq, label = %stale.label, "지난 시행의 ack — 버린다");
                        }
                        Err(_) => break,
                    }
                }
                let rt = t0.elapsed().as_secs_f64() * 1000.0;
                if got < windows.len() {
                    tracing::warn!(seq, got, "ack 이 모자란다 — 이 시행은 버린다");
                    continue;
                }
                if rep >= WARMUP {
                    round_trips.push(rt);
                    builds.push(build_sum / windows.len() as f64);
                    paints.push(paint_max);
                }
                // 다음 시행 전에 한 프레임 쉰다 — 연속 emit 으로 웹뷰 큐가
                // 밀리면 왕복이 아니라 큐 대기를 재게 된다.
                std::thread::sleep(Duration::from_millis(20));
            }

            let line = format!(
                "| {} | {} | **{:.2} ms** | {:.2} ms | {:.2} ms | {:.2} ms | {:.2} ms |\n",
                mode,
                count,
                median(&mut round_trips),
                percentile(&mut round_trips, 0.95),
                max(&round_trips),
                median(&mut builds),
                median(&mut paints),
            );
            tracing::info!(
                mode,
                count,
                windows = windows.len(),
                round_trip_median_ms = median(&mut round_trips),
                round_trip_p95_ms = percentile(&mut round_trips, 0.95),
                build_median_ms = median(&mut builds),
                paint_median_ms = median(&mut paints),
                serialize_median_us = median(&mut serialize_us),
                "P3 시행"
            );
            report.push_str(&line);
        }
    }

    tracing::info!("\n=== ⭐ F-03 P3 실측 (창 {}개) ===\n{}", windows.len(), report);
    write_report(&windows, &report);

    // 창을 치우고 프로세스를 끝낸다 — 스파이크는 제품 세션이 아니다.
    for (_, w, _) in &windows {
        let _ = w.close();
    }
    std::thread::sleep(Duration::from_millis(300));
    app.exit(0);
}

/// 결과를 로그 디렉터리에 마크다운으로도 떨어뜨린다 — `open` 으로 띄우면
/// stderr 가 사라지므로 파일이 유일하게 확실한 산출물이다.
fn write_report(windows: &[SpikeWindow], report: &str) {
    let Some(home) = std::env::var_os("HOME") else { return };
    let dir = std::path::Path::new(&home).join("Library/Logs/Ultrakey");
    let _ = std::fs::create_dir_all(&dir);
    let mut body = String::new();
    body.push_str("# F-03 P3 실측 원자료 (자동 생성)\n\n");
    body.push_str(&format!("- 창 {}개\n", windows.len()));
    for (label, _, g) in windows {
        body.push_str(&format!(
            "  - `{label}` — {:.0}×{:.0} pt @ ({:.0}, {:.0})\n",
            g.width_pt, g.height_pt, g.origin_x, g.origin_y
        ));
    }
    body.push_str(&format!("- 시행 {REPEATS}회(앞 {WARMUP}회 버림)\n\n"));
    body.push_str(report);
    let _ = std::fs::write(dir.join("overlay-spike.md"), body);
}

/// 한 창에 보낼 프레임을 만든다. 좌표는 화면 안에 고르게 흩뿌린다.
fn make_frame(
    seq: u64,
    mode: &'static str,
    count: usize,
    width: f64,
    height: f64,
    salt: usize,
) -> SpikeFrame {
    let mut items = Vec::with_capacity(count);
    // 결정론적인 유사 난수 — 시행마다 좌표가 달라야 레이아웃 재사용을 막는다.
    let mut s = (seq.wrapping_mul(6364136223846793005).wrapping_add(salt as u64 | 1)) | 1;
    let mut next = || {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        (s >> 11) as f64 / (1u64 << 53) as f64
    };
    for i in 0..count {
        let w = 40.0 + next() * 160.0;
        let h = 14.0 + next() * 6.0;
        items.push(SpikeItem {
            x: (next() * (width - w)).max(0.0),
            y: (next() * (height - h)).max(0.0),
            w,
            h,
            sel: i == 0,
            label: format!("match {i}"),
        });
    }
    let line = items.first().map(|it| [width / 2.0, 60.0, it.x + it.w / 2.0, it.y]);
    SpikeFrame { seq, mode, items, line }
}

fn median(v: &mut [f64]) -> f64 {
    percentile(v, 0.5)
}

fn percentile(v: &mut [f64], p: f64) -> f64 {
    if v.is_empty() {
        return f64::NAN;
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let idx = ((v.len() - 1) as f64 * p).round() as usize;
    v[idx]
}

fn max(v: &[f64]) -> f64 {
    v.iter().copied().fold(f64::NAN, f64::max)
}
