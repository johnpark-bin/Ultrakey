//! ⭐ F-01 — Seek 활성화·세션 워커. 명세: `docs/spec/seek-activation-and-session.md`.
//!
//! `ultrakey-seek-session` 이 이미 순수 상태 머신([`SeekSessionMachine`])을 갖고
//! 있다 — 이 파일이 하는 일은 그 머신을 **워커 전용 스레드**에 얹고, 세 활성화
//! 경로(전역 단축키·리매핑 키·quick press caps lock)·세션 중 키·F-02 검출 결과를
//! [`SeekSignal`] 채널로 받아 머신에 먹이고, 나온 [`SessionEffect`] 를 실제
//! 오버레이(F-03)·클릭(F-04, `click_executor.rs`)에 실행하는 조립이다.
//!
//! ## 스레드 모양
//!
//! - **탭 스레드**(`ultrakey-engine`) — `Effect::SeekTriggerDown`/`Up`/`SeekKey` 를
//!   낸다. `EngineEvent` 문서 계약대로 콜백은 **절대 블록하지 않는다** — `main.rs`
//!   의 `on_engine_event` 는 이 신호를 [`SeekSignal`] 로 채널에 밀어 넣기만 하고
//!   즉시 반환한다(`tracing` 호출조차 하지 않는다, `SeekKey` 는 매 키마다 온다).
//! - **워커 스레드**(이 파일의 [`spawn`] 이 만든다) — [`SeekController`] 를 **단독
//!   소유**한다. 상태 머신 판정·오버레이 렌더·검출 트리거·클릭 위임을 전부 이
//!   스레드에서 순차 처리한다 — 락이 필요 없다(단일 소유자).
//! - **검출 스레드**(세션마다 하나씩, [`SessionEffect::Opened`] 를 받을 때 뜬다) —
//!   `ultrakey_seek::detect_candidates` 를 돌린다. ⛔ **워커 스레드 자신이 돌리면
//!   안 된다** — 775 ms(`seek-ocr-latency-spike.md` §2.6) 동안 세션 중 키 입력을
//!   전혀 못 받는다. `overlay_demo.rs` 가 쓰는 것과 같은 모양이되, 그쪽은 컨트롤러를
//!   `Arc<Mutex<..>>` 로 공유해 직접 부르는 반면 여기는 **채널로 결과를 되돌린다**
//!   (워커가 단일 소유자라 잠금이 아예 없다 — S-1·S-6, `seek-ocr-latency-spike.md`).
//! - **전역 단축키 포워더 스레드**([`spawn_hotkey_forwarder`]) — `global-hotkey`
//!   0.8.0 의 `GlobalHotKeyEvent::receiver()` 를 블로킹으로 읽어 워커 채널로 옮긴다
//!   (명세 §6 "가로채기가 아니라 등록" — `CGEventTap` 과 무관한 별도 경로).
//!
//! ## ⚠️ 세대(generation) 번호
//!
//! 세션이 닫힌 뒤에도 이전 검출 스레드가 살아 있을 수 있다(늦게 끝난 OCR). 그
//! 결과가 **다음** 세션에 섞이면 안 되므로, [`SeekController::generation`] 을 세션을
//! 열 때마다 증가시키고 [`SeekSignal::Candidates`]/[`SeekSignal::ExtraCandidates`]/
//! [`SeekSignal::DetectionFinished`] 세 신호 모두에 그 번호를 실어, 워커가 현재
//! 세대와 다르면 조용히 버린다. 위임 지시서는 `Candidates` 에만 세대 필드를
//! 명시했지만, `ExtraCandidates`·`DetectionFinished` 도 같은 검출 스레드가 보내는
//! 신호라 같은 위험(늦게 끝난 이전 세션의 결과가 다음 세션의 `finish_detection()`
//! 을 앞당겨 부르는 것)이 있어 셋 다에 붙였다 — 이 판단은 이 구현이 스스로 내렸다.
//!
//! ## ⚠️ 레이아웃 문자 해석의 한계
//!
//! 세션 중 키를 검색어로 넣으려면 keycode+flags → 문자 변환이 필요하다.
//! [`resolve_typed_char`] 는 먼저 `ultrakey_layout::LayoutTable::char_for`(엔진이
//! 부팅 시 실제 macOS 키보드 레이아웃으로 채워 둔 표, `system_hooks::
//! refresh_input_source`)를 쓰고, 그 표가 비어 있을 때만(부팅 초기 등)
//! [`ascii_fallback`] — `KeyCode::from_web_code` 의 역방향으로 만든 **US-ASCII
//! 전용** 최소 표 — 로 떨어진다. ⚠️ 이 폴백은 한글·비ASCII 레이아웃을 전혀 모른다
//! — 정상 경로(레이아웃 표가 채워진 뒤)에서는 문제가 없지만, 표가 비어 있는
//! 드문 창에서 세션이 열리면 그 폴백이 적용되는 동안 비ASCII 검색어가 제한된다.

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use crossbeam_channel::{unbounded, Receiver, Sender};

use ultrakey_core::event::InputEvent;
use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::KeyCode;
use ultrakey_engine::SharedState;
use ultrakey_layout::ModifierCombo;
use ultrakey_overlay::renderer::OverlayRenderer;
use ultrakey_seek::{detect_candidates, CandidateSource, DetectionParams, TextCandidate};
use ultrakey_seek_session::keys::{classify, SessionKey};
use ultrakey_seek_session::{
    ActivationPath, ClickExecutor, ClickSettings, SeekConfig, SeekSessionMachine, SessionEffect,
};

use crate::overlay::{
    default_search_bar_origin, overlay_displays, SurfaceState, WebviewOverlayRenderer,
};

/// 탭 스레드/전역 단축키 포워더 → Seek 워커로 넘어가는 신호.
///
/// ⚠️ 이 신호를 만드는 쪽(`on_engine_event`, 전역 단축키 포워더)은 채널에 밀어
/// 넣기만 하고 **즉시 반환**한다 — 실제 판정·렌더·검출은 전부 워커가 한다.
pub enum SeekSignal {
    /// 활성화 경로 3 — quick press caps lock(`Effect::OpenSeek`).
    OpenRequested,
    /// 활성화 경로 2 — `Remap key to Seek:` 키 다운.
    TriggerDown,
    /// 같은 키 릴리즈 — hold 모드의 확정 신호. `flags` 는 릴리즈 순간의 modifier
    /// 스냅샷이다(F-04 §5 #10).
    TriggerUp(EventFlags),
    /// 세션 중 라우팅되는 키(계층 1). `kind` 는 이미 정규화돼 있다.
    Key(InputEvent),
    /// 활성화 경로 1 — 전역 단축키(`Toggle Seek with shortcut:`) 다운.
    GlobalShortcut,
    /// F-02 `on_display` 증분 콜백 — 세대 번호가 현재 세션과 다르면 버려진다.
    /// `arrived_at_ms` 는 검출 시작부터 이 결과가 도착하기까지 걸린 시간(검출
    /// 스레드가 `Instant` 로 잰 값 — 로그가 아니라 데이터라 검출 스레드에서
    /// 계산해도 "로그는 워커 스레드에서만" 계약을 어기지 않는다).
    Candidates {
        generation: u64,
        display_id: u32,
        candidates: Vec<TextCandidate>,
        arrived_at_ms: f64,
    },
    /// 디스플레이가 특정되지 않는 후보(AX). 세대 번호는 위와 같은 규칙.
    ExtraCandidates {
        generation: u64,
        candidates: Vec<TextCandidate>,
    },
    /// F-02 검출이 완전히 끝났다. 통계는 워커 스레드에서 로그로 남기기 위한 것이다
    /// (⚠️ 검출 스레드 자신은 로그를 남기지 않는다 — 검증 하네스 요구사항이 로그를
    /// 워커 스레드로 한정한다).
    DetectionFinished {
        generation: u64,
        total_ms: f64,
        ocr: usize,
        ax: usize,
        merged: usize,
    },
    /// 디스플레이 구성이 바뀌었다(핫플러그, §5 #6).
    DisplaysChanged,
    /// 설정이 바뀌었다 — 열려 있는 세션의 모드는 바꾸지 않는다
    /// (`SeekSessionMachine::set_config` 계약). F-04 클릭 설정(`ClickSettings`)
    /// 은 같은 저장 갱신에서 태어나므로(A6 — 단일 소스 → 단일 신호 원칙)
    /// 함께 실어 보낸다.
    ConfigChanged(SeekConfig, ClickSettings),
    /// 워커를 끝낸다. 지금은 어디서도 보내지 않는다(앱은 프로세스 종료로 끝난다) —
    /// 워커 루프(`run_worker`)를 유한하게 만들 수 있는 신호가 이것뿐이라는 것을
    /// 이음매로 남겨 둔다(향후 유닛 테스트·정상 종료 경로가 쓸 자리).
    #[allow(dead_code)]
    Shutdown,
}

/// 워커가 세션 하나 열 때마다 필요한 화면 스냅샷 — 메인 스레드에서만 얻을 수
/// 있는 값들(`NSScreen`·다크 모드·모션 축소·커서 위치)을 한 번에 담는다.
struct ActivationSnapshot {
    displays: Vec<ultrakey_overlay::OverlayDisplay>,
    appearance: ultrakey_overlay::Appearance,
    reduce_motion: bool,
    mouse: Option<(f64, f64)>,
}

/// 메인 스레드에 화면 스냅샷을 부탁하고 결과를 받아온다(`overlay_demo::
/// fetch_displays` 와 같은 모양 — `run_on_main_thread` 는 큐잉만 하고 즉시
/// 반환하므로, 응답은 별도 채널로 기다린다).
fn snapshot_activation_context(app: &tauri::AppHandle) -> Option<ActivationSnapshot> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let displays = overlay_displays();
        let dark = ultrakey_platform::screens::is_dark_appearance().unwrap_or(true);
        let appearance = if dark {
            ultrakey_overlay::Appearance::Dark
        } else {
            ultrakey_overlay::Appearance::Light
        };
        let reduce_motion = ultrakey_platform::screens::should_reduce_motion();
        let mouse = ultrakey_platform::screens::mouse_location();
        let _ = tx.send(ActivationSnapshot {
            displays,
            appearance,
            reduce_motion,
            mouse,
        });
    })
    .ok()?;
    rx.recv_timeout(Duration::from_millis(500)).ok()
}

/// 워커 루프 전체가 공유하는 불변 환경.
struct WorkerEnv {
    app: tauri::AppHandle,
    shared: Arc<SharedState>,
    tx: Sender<SeekSignal>,
    /// `ULTRAKEY_SEEK_TRACE=1` — 검증 하네스(위임 지시 4-g #10).
    trace: bool,
    /// 저장된 검색 바 위치를 읽는다(F-15 "부재 = 기본값"). `None` 이면
    /// [`default_search_bar_origin`] 이 기본 위치를 정한다.
    stored_origin: Arc<dyn Fn() -> Option<(f64, f64)> + Send + Sync>,
    /// ⭐ 이슈 #48 — 세션을 열 때마다 현재 UI 로케일에서 계산한
    /// `recognitionLanguages` 목록을 읽는다(메인 스레드의 `catalog` 를 캡처한
    /// 클로저 — `stored_origin` 과 같은 경계).
    ocr_languages: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
}

/// F-01 세션 컨트롤러 — 워커 스레드가 **단독 소유**한다(잠금 없음).
struct SeekController {
    machine: SeekSessionMachine,
    renderer: Box<dyn OverlayRenderer + Send>,
    executor: Box<dyn ClickExecutor + Send>,
    /// 세션을 열 때마다 증가한다 — 늦게 도착하는 검출 결과를 걸러내는 열쇠.
    generation: u64,
    /// 직전 `activate()` 호출 시각 — `Opened` 효과의 `elapsed_ms` 로그용.
    activation_started: Option<Instant>,
}

impl SeekController {
    fn new(
        config: SeekConfig,
        renderer: Box<dyn OverlayRenderer + Send>,
        executor: Box<dyn ClickExecutor + Send>,
    ) -> Self {
        Self {
            machine: SeekSessionMachine::new(config),
            renderer,
            executor,
            generation: 0,
            activation_started: None,
        }
    }

    fn repaint(&mut self) {
        let Some(overlay) = self.machine.overlay() else {
            return;
        };
        let frames = overlay.frames();
        let bar = overlay.search_bar_frame();
        if let Err(e) = self.renderer.present(&frames, &bar) {
            tracing::warn!(error = ?e, "failed to render the Seek overlay");
        }
    }

    /// 세 활성화 경로 공통 처리 — 메인 스레드 스냅샷을 얻고 `machine.activate()`
    /// 를 부른 뒤 그 효과를 적용한다.
    fn activate(&mut self, path: ActivationPath, env: &WorkerEnv) {
        let Some(snapshot) = snapshot_activation_context(&env.app) else {
            tracing::error!(
                ?path,
                "failed to get main-thread screen snapshot; skipping Seek activation"
            );
            return;
        };
        let origin = (env.stored_origin)()
            .unwrap_or_else(|| default_search_bar_origin(&snapshot.displays, snapshot.mouse));

        self.activation_started = Some(Instant::now());
        let effects = self.machine.activate(
            path,
            snapshot.displays,
            snapshot.appearance,
            snapshot.reduce_motion,
            origin,
        );
        self.apply_effects(effects, env);
    }

    fn apply_effects(&mut self, effects: Vec<SessionEffect>, env: &WorkerEnv) {
        for effect in effects {
            match effect {
                SessionEffect::Opened { path, mode } => {
                    // ⭐ 탭 스레드가 이 원자값을 읽어 계층 1(세션 활성) 게이트를
                    // 판정한다(`SharedState::seek_session_active` 문서 주석).
                    env.shared
                        .seek_session_active
                        .store(true, Ordering::Release);

                    let Some(overlay) = self.machine.overlay() else {
                        continue;
                    };
                    let displays: Vec<_> = overlay.displays().to_vec();
                    let origin = overlay.search_bar_origin();
                    let _ = self.renderer.sync_surfaces(&displays);
                    let _ = self.renderer.set_search_bar_origin(origin.0, origin.1);
                    let _ = self.renderer.show();

                    self.generation += 1;
                    spawn_detection(self.generation, env.tx.clone(), (env.ocr_languages)());

                    if env.trace {
                        let elapsed_ms = self
                            .activation_started
                            .map_or(0.0, |t| t.elapsed().as_secs_f64() * 1000.0);
                        tracing::warn!(
                            ?path,
                            ?mode,
                            elapsed_ms,
                            matches = 0,
                            "seek session opened"
                        );
                    }
                }

                SessionEffect::Repaint => self.repaint(),

                SessionEffect::Confirm(m) => {
                    if env.trace {
                        let (px, py) = m.click_point();
                        tracing::warn!(
                            text = %m.text,
                            point_x = px,
                            point_y = py,
                            modifiers = m.modifiers.0,
                            query = %m.query,
                            "confirmed"
                        );
                    }
                    // ⭐ ① 오버레이 해제가 항상 먼저다(F-04 §3.6) — ② 클릭 실행보다도.
                    let _ = self.renderer.hide();
                    if let Err(e) = self.executor.execute(&m) {
                        tracing::error!(error = ?e, "failed to execute Seek click");
                    }
                    let more = self.machine.notify_click_finished();
                    self.apply_effects(more, env);
                }

                SessionEffect::Closed { reason } => {
                    env.shared
                        .seek_session_active
                        .store(false, Ordering::Release);
                    let _ = self.renderer.hide();
                    if env.trace {
                        tracing::warn!(?reason, "session ended");
                    } else {
                        tracing::debug!(?reason, "Seek session ended");
                    }
                }
            }
        }
    }
}

/// ⛔ 워커 스레드 자신이 검출을 돌리면 안 된다(모듈 문서) — 별도 스레드를 띄운다.
fn spawn_detection(
    generation: u64,
    tx: Sender<SeekSignal>,
    recognition_languages: Vec<String>,
) {
    let spawned = thread::Builder::new()
        .name("ultrakey-seek-detect".into())
        .spawn(move || {
            let started = Instant::now();
            // ⭐ 이슈 #48 — `general.language`(UI 로케일)에서 매핑한
            // `recognitionLanguages` 목록이 유입된다. 빈 Vec 이면 Vision 기본값
            // (영어)을 그대로 쓴다 — 로케일 미설정/영어일 때 기존 동작과 동일하다.
            // ⭐ `Seek using macOS accessibility`(F-02 설정)은 아직 이 위임의 범위에
            // 없다 — 그 설정이 들어오면 이 자리에 `SeekConfig`/전용 설정을 통해
            // 값을 흘려보내야 한다(`use_accessibility: false` 는 출고 기본값 고정).
            let mut params = DetectionParams {
                use_accessibility: false,
                ..Default::default()
            };
            params.recognition.languages = recognition_languages;

            let tx_for_display = tx.clone();
            let outcome = detect_candidates(&params, move |result| {
                let arrived_at_ms = started.elapsed().as_secs_f64() * 1000.0;
                let _ = tx_for_display.send(SeekSignal::Candidates {
                    generation,
                    display_id: result.display_id,
                    candidates: result.candidates.clone(),
                    arrived_at_ms,
                });
            });

            let ax_only: Vec<TextCandidate> = outcome
                .candidates
                .iter()
                .filter(|c| c.source == CandidateSource::Accessibility)
                .cloned()
                .collect();
            if !ax_only.is_empty() {
                let _ = tx.send(SeekSignal::ExtraCandidates {
                    generation,
                    candidates: ax_only,
                });
            }

            let _ = tx.send(SeekSignal::DetectionFinished {
                generation,
                total_ms: outcome.total_ms,
                ocr: outcome.ocr_count,
                ax: outcome.ax_count,
                merged: outcome.candidates.len(),
            });
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "failed to create the Seek detection thread");
    }
}

/// `global-hotkey` 의 `GlobalHotKeyEvent::receiver()` 를 블로킹으로 읽어 워커
/// 채널로 옮긴다.
///
/// ⚠️ **`Pressed` 만 전달한다.** `global-hotkey` 0.8.0 은 macOS 에서
/// `RegisterEventHotKey`(Carbon)로 다운·업 두 이벤트를 **모두** 받는다(크레이트
/// 소스 `platform_impl/macos/mod.rs` 의 `InstallEventHandler` 가
/// `kEventHotKeyPressed`/`kEventHotKeyReleased` 둘 다를 설치한다 — 이 세션이
/// 소스로 직접 확인했다). `Released` 까지 `SeekSignal::GlobalShortcut` 으로
/// 전달하면, toggle 모드 세션에서 단축키를 한 번 눌렀다 뗀 것만으로
/// (`Pressed`→열림, `Released`→같은 경로 재입력으로 오인돼 즉시 닫힘) 세션이
/// 열리자마자 닫히는 결함이 생긴다(§3.2 "토글 재입력" 행). `HotKeyState::
/// Released` 는 조용히 버린다.
fn spawn_hotkey_forwarder(tx: Sender<SeekSignal>) {
    let spawned = thread::Builder::new()
        .name("ultrakey-seek-hotkey".into())
        .spawn(move || {
            let rx = global_hotkey::GlobalHotKeyEvent::receiver();
            while let Ok(event) = rx.recv() {
                if event.state() != global_hotkey::HotKeyState::Pressed {
                    continue;
                }
                if tx.send(SeekSignal::GlobalShortcut).is_err() {
                    break; // 워커가 끝났다 — 이 스레드도 끝낸다.
                }
            }
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "failed to spawn the Seek global-shortcut forwarder thread");
    }
}

/// 세션 중 키 하나를 현재 레이아웃으로 해석한 문자.
///
/// 정방향(keycode+flags → char) API 를 `ultrakey_layout::LayoutTable::char_for`
/// 가 이미 제공한다(F-14(B), `crates/ultrakey-layout/src/lib.rs`) — 그것을 먼저
/// 쓰고, 레이아웃 표가 비어 있을 때만(부팅 초기 등) [`ascii_fallback`] 으로
/// 떨어진다(모듈 문서의 한계 참고).
fn resolve_typed_char(shared: &SharedState, ev: &InputEvent) -> Option<char> {
    let table = shared.layout.current();
    if !table.is_empty() {
        let combo = combo_for_flags(ev.flags);
        if let Some(s) = table.char_for(ev.keycode, combo) {
            return s.chars().next();
        }
    }
    ascii_fallback(ev.keycode, ev.flags)
}

fn combo_for_flags(flags: EventFlags) -> ModifierCombo {
    let shift = flags.contains(EventFlags::SHIFT);
    let option = flags.contains(EventFlags::ALTERNATE);
    match (shift, option) {
        (false, false) => ModifierCombo::None,
        (true, false) => ModifierCombo::Shift,
        (false, true) => ModifierCombo::Option,
        (true, true) => ModifierCombo::ShiftOption,
    }
}

/// `KeyCode::from_web_code` 의 역방향 — **US-ASCII 전용 최소 폴백**(모듈 문서 참고).
/// 레이아웃 표가 준비되기 전에만 쓰인다.
fn ascii_fallback(keycode: KeyCode, flags: EventFlags) -> Option<char> {
    let shift = flags.contains(EventFlags::SHIFT);
    let c = match keycode {
        KeyCode::ANSI_A => 'a',
        KeyCode::ANSI_B => 'b',
        KeyCode::ANSI_C => 'c',
        KeyCode::ANSI_D => 'd',
        KeyCode::ANSI_E => 'e',
        KeyCode::ANSI_F => 'f',
        KeyCode::ANSI_G => 'g',
        KeyCode::ANSI_H => 'h',
        KeyCode::ANSI_I => 'i',
        KeyCode::ANSI_J => 'j',
        KeyCode::ANSI_K => 'k',
        KeyCode::ANSI_L => 'l',
        KeyCode::ANSI_M => 'm',
        KeyCode::ANSI_N => 'n',
        KeyCode::ANSI_O => 'o',
        KeyCode::ANSI_P => 'p',
        KeyCode::ANSI_Q => 'q',
        KeyCode::ANSI_R => 'r',
        KeyCode::ANSI_S => 's',
        KeyCode::ANSI_T => 't',
        KeyCode::ANSI_U => 'u',
        KeyCode::ANSI_V => 'v',
        KeyCode::ANSI_W => 'w',
        KeyCode::ANSI_X => 'x',
        KeyCode::ANSI_Y => 'y',
        KeyCode::ANSI_Z => 'z',
        // ⚠️ 숫자·기호 줄의 shift 조합(`!@#$…`)은 US 배열에서도 물리 위치와
        // 문자가 어긋나는 경우가 있어 다루지 않는다 — 이 폴백은 최소 수준이다.
        KeyCode::ANSI_0 => '0',
        KeyCode::ANSI_1 => '1',
        KeyCode::ANSI_2 => '2',
        KeyCode::ANSI_3 => '3',
        KeyCode::ANSI_4 => '4',
        KeyCode::ANSI_5 => '5',
        KeyCode::ANSI_6 => '6',
        KeyCode::ANSI_7 => '7',
        KeyCode::ANSI_8 => '8',
        KeyCode::ANSI_9 => '9',
        KeyCode::SPACE => ' ',
        KeyCode::ANSI_MINUS => '-',
        KeyCode::ANSI_EQUAL => '=',
        KeyCode::ANSI_LEFT_BRACKET => '[',
        KeyCode::ANSI_RIGHT_BRACKET => ']',
        KeyCode::ANSI_BACKSLASH => '\\',
        KeyCode::ANSI_SEMICOLON => ';',
        KeyCode::ANSI_QUOTE => '\'',
        KeyCode::ANSI_COMMA => ',',
        KeyCode::ANSI_PERIOD => '.',
        KeyCode::ANSI_SLASH => '/',
        KeyCode::ANSI_GRAVE => '`',
        _ => return None,
    };
    Some(if shift { c.to_ascii_uppercase() } else { c })
}

/// Seek 워커를 띄운다 — `main.rs` 가 엔진 시작 시점에 부른다.
///
/// `stored_origin` 은 저장된 검색 바 위치를 읽고, `persist_origin` 은 사용자가
/// 검색 바를 끌어 옮겼을 때 저장한다(둘 다 `AppState.store` 를 캡처한 클로저 —
/// 이 파일은 `AppState` 를 모른다, `overlay.rs`/`overlay_demo.rs` 와 같은 경계).
/// `ocr_languages` 는 세션을 열 때마다 현재 UI 로케일(`general.language`)에서
/// 계산한 `recognitionLanguages` 목록을 읽는다(`AppState.catalog` 캡처 — 이슈 #48).
#[allow(clippy::too_many_arguments)]
pub fn spawn(
    app: tauri::AppHandle,
    shared: Arc<SharedState>,
    surface_state: Arc<Mutex<SurfaceState>>,
    initial_config: SeekConfig,
    initial_click_settings: ClickSettings,
    stored_origin: Arc<dyn Fn() -> Option<(f64, f64)> + Send + Sync>,
    ocr_languages: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
    persist_origin: Arc<dyn Fn(f64, f64) + Send + Sync>,
) -> Sender<SeekSignal> {
    let (tx, rx) = unbounded::<SeekSignal>();

    spawn_hotkey_forwarder(tx.clone());

    let tx_for_worker = tx.clone();
    let spawned = thread::Builder::new()
        .name("ultrakey-seek".into())
        .spawn(move || {
            run_worker(
                rx,
                tx_for_worker,
                app,
                shared,
                surface_state,
                initial_config,
                initial_click_settings,
                stored_origin,
                ocr_languages,
                persist_origin,
            );
        });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "failed to spawn the Seek worker thread; Seek will not work in this session");
    }

    tx
}

#[allow(clippy::too_many_arguments)]
fn run_worker(
    rx: Receiver<SeekSignal>,
    tx: Sender<SeekSignal>,
    app: tauri::AppHandle,
    shared: Arc<SharedState>,
    surface_state: Arc<Mutex<SurfaceState>>,
    initial_config: SeekConfig,
    initial_click_settings: ClickSettings,
    stored_origin: Arc<dyn Fn() -> Option<(f64, f64)> + Send + Sync>,
    ocr_languages: Arc<dyn Fn() -> Vec<String> + Send + Sync>,
    persist_origin: Arc<dyn Fn(f64, f64) + Send + Sync>,
) {
    let trace = std::env::var_os("ULTRAKEY_SEEK_TRACE").is_some();

    let renderer: Box<dyn OverlayRenderer + Send> = Box::new(
        WebviewOverlayRenderer::new(app.clone(), surface_state).on_search_bar_moved({
            let persist_origin = persist_origin.clone();
            move |x, y| persist_origin(x, y)
        }),
    );
    // ⭐ F-04(이슈 #44) — `NullClickExecutor` 자리를 실 구현으로 교체한다.
    // executor 는 이 워커가 단독 소유하므로 설정은 `configure` 호출로만
    // 바뀐다(락 없음).
    let executor: Box<dyn ClickExecutor + Send> = Box::new(crate::click_executor::ClickExecutor::new(
        app.clone(),
        initial_click_settings,
    ));

    let env = WorkerEnv {
        app,
        shared,
        tx,
        trace,
        stored_origin,
        ocr_languages,
    };
    let mut controller = SeekController::new(initial_config, renderer, executor);

    for signal in rx.iter() {
        match signal {
            SeekSignal::Shutdown => break,

            SeekSignal::ConfigChanged(config, click_settings) => {
                controller.machine.set_config(config);
                controller.executor.configure(click_settings);
            }

            SeekSignal::DisplaysChanged => {
                let effects = controller.machine.displays_changed();
                controller.apply_effects(effects, &env);
            }

            SeekSignal::OpenRequested => {
                controller.activate(ActivationPath::QuickPressCapsLock, &env)
            }
            SeekSignal::TriggerDown => controller.activate(ActivationPath::RemapKey, &env),
            SeekSignal::GlobalShortcut => controller.activate(ActivationPath::GlobalShortcut, &env),

            SeekSignal::TriggerUp(mods) => {
                let effects = controller.machine.trigger_released(mods);
                controller.apply_effects(effects, &env);
            }

            SeekSignal::Key(ev) => {
                let typed = resolve_typed_char(&env.shared, &ev);
                // ⭐ 로그 전용 재분류 — `handle_key` 가 이미 같은 판정을 내부에서
                // 하지만(순수·부수효과 없음) 어떤 키였는지를 밖에서 알 수 있는
                // 유일한 방법이 이것뿐이다. 실제 상태 전이는 `handle_key` 만 한다.
                let key_kind = env
                    .trace
                    .then(|| classify(&ev, typed, controller.machine.config().semicolon_cycles));

                let effects = controller.machine.handle_key(&ev, typed);
                controller.apply_effects(effects, &env);

                // ⚠️ **로그는 반드시 `handle_key` 뒤다.** 앞에서 찍으면 그 키를
                // 처리하기 *전*의 query/matches/state 가 남아 실기 검증 로그가 한 칸씩
                // 밀려 읽힌다 — 첫 글자를 친 줄에 `query=`(빈 값)가 찍히는 식이다.
                // 실제로 첫 검증 회차에서 그렇게 나와 여기서 고쳤다.
                if let Some(kind) = key_kind {
                    log_key_trace(&controller, kind);
                }
            }

            SeekSignal::Candidates {
                generation,
                display_id,
                candidates,
                arrived_at_ms,
            } => {
                if generation != controller.generation {
                    continue;
                }
                let n = candidates.len();
                let effects = controller.machine.ingest_display(display_id, candidates);
                controller.apply_effects(effects, &env);
                if env.trace {
                    let matches = controller
                        .machine
                        .overlay()
                        .map_or(0, |o| o.search_bar_frame().total_matches);
                    tracing::warn!(
                        display_id,
                        candidates = n,
                        arrived_at_ms,
                        matches,
                        "incremental receive"
                    );
                }
            }

            SeekSignal::ExtraCandidates {
                generation,
                candidates,
            } => {
                if generation != controller.generation {
                    continue;
                }
                let effects = controller.machine.ingest_extra(candidates);
                controller.apply_effects(effects, &env);
            }

            SeekSignal::DetectionFinished {
                generation,
                total_ms,
                ocr,
                ax,
                merged,
            } => {
                if generation != controller.generation {
                    continue;
                }
                let effects = controller.machine.finish_detection();
                controller.apply_effects(effects, &env);
                if env.trace {
                    let matches = controller
                        .machine
                        .overlay()
                        .map_or(0, |o| o.search_bar_frame().total_matches);
                    tracing::warn!(total_ms, ocr, ax, merged, matches, "detection finished");
                }
            }
        }
    }
}

/// [`SeekSignal::Key`] 트레이스 로그 — Next/Prev/CycleSemicolon 은 "선택 이동",
/// Text/Backspace 는 "키 입력"으로 남긴다(위임 지시 4-g #10).
fn log_key_trace(controller: &SeekController, key_kind: SessionKey) {
    let Some(overlay) = controller.machine.overlay() else {
        return;
    };
    match key_kind {
        SessionKey::Next | SessionKey::Prev | SessionKey::CycleSemicolon => {
            let bar = overlay.search_bar_frame();
            let text = overlay
                .selected()
                .map(|c| c.text.as_str())
                .unwrap_or_default();
            tracing::warn!(index = ?bar.selected_index, text, "selection moved");
        }
        SessionKey::Text(_) | SessionKey::Backspace => {
            let bar = overlay.search_bar_frame();
            tracing::warn!(
                query = %controller.machine.query(),
                matches = bar.total_matches,
                state = ?controller.machine.state(),
                "key input"
            );
        }
        _ => {}
    }
}

// ── `Toggle Seek with shortcut:` — 전역 단축키 등록(활성화 경로 1) ─────────────
//
// ⭐ `global-hotkey` 0.8.0 의 `Code`/`Modifiers` 는 `keyboard-types` 크레이트가
// 제공한다. 크레이트 소스(`~/.cargo/registry/.../keyboard-types-0.7.0/src/
// code.rs` 의 `impl FromStr for Code`)를 이 세션이 직접 읽어 확인했다 — `Code::
// from_str` 은 W3C `KeyboardEvent.code` 문자열(`"KeyA"`·`"Space"`·`"F13"`…)을
// **그대로** 받아들인다. 즉 `SeekShortcut::code`(우리 저장 표현)를 그대로
// `Code::from_str` 에 넘기면 된다 — 별도 변환 표가 필요 없다.

/// 저장된 `SeekShortcut` 을 `global-hotkey` 의 `HotKey` 로 바꾼다. `code` 가 W3C
/// `KeyboardEvent.code` 로 인식되지 않으면 `None`(값을 지어내지 않는다).
fn to_global_hotkey(
    shortcut: &ultrakey_seek_session::SeekShortcut,
) -> Option<global_hotkey::hotkey::HotKey> {
    let code: global_hotkey::hotkey::Code = shortcut.code.parse().ok()?;
    Some(global_hotkey::hotkey::HotKey::new(
        Some(to_global_hotkey_modifiers(shortcut.modifiers)),
        code,
    ))
}

/// `EventFlags`(우리 비트마스크) → `global_hotkey::hotkey::Modifiers`.
///
/// ⭐ Command 는 `Modifiers::SUPER` 로 매핑한다 — `global-hotkey` 자신의
/// `hotkey::CMD_OR_CTRL` 상수가 macOS 에서 `Modifiers::SUPER` 로 정의돼 있고
/// (크레이트 소스로 확인), macOS `platform_impl` 의 `register()` 도 `SUPER|META`
/// 둘 다를 Cmd 로 받아들인다 — `SUPER` 하나만 세팅해도 등록에는 문제가 없다.
fn to_global_hotkey_modifiers(flags: EventFlags) -> global_hotkey::hotkey::Modifiers {
    use global_hotkey::hotkey::Modifiers;
    let mut m = Modifiers::empty();
    if flags.contains(EventFlags::SHIFT) {
        m |= Modifiers::SHIFT;
    }
    if flags.contains(EventFlags::CONTROL) {
        m |= Modifiers::CONTROL;
    }
    if flags.contains(EventFlags::ALTERNATE) {
        m |= Modifiers::ALT;
    }
    if flags.contains(EventFlags::COMMAND) {
        m |= Modifiers::SUPER;
    }
    m
}

/// `Toggle Seek with shortcut:` 등록을 현재 설정과 맞춘다 — **메인 스레드에서
/// 불러야 한다**(`GlobalHotKeyManager` 문서, `AppState.global_hotkey_manager` 주석).
///
/// ⚠️ **등록 실패를 삼키지 않는다** — 이미 다른 앱이 쓰는 조합이면 실패한다
/// (위임 지시 4-e). `tracing::warn!` 으로 남겨 관측 가능하게 한다.
pub fn apply_global_shortcut(
    manager: &global_hotkey::GlobalHotKeyManager,
    previous: &mut Option<global_hotkey::hotkey::HotKey>,
    shortcut: Option<&ultrakey_seek_session::SeekShortcut>,
) {
    if let Some(old) = previous.take() {
        if let Err(e) = manager.unregister(old) {
            tracing::warn!(error = %e, "failed to unregister the previous Seek global shortcut");
        }
    }

    let Some(shortcut) = shortcut else { return };
    let Some(hotkey) = to_global_hotkey(shortcut) else {
        tracing::warn!(
            code = %shortcut.code,
            "Seek global shortcut code is not a known KeyboardEvent.code; skipping registration"
        );
        return;
    };

    match manager.register(hotkey) {
        Ok(()) => *previous = Some(hotkey),
        Err(e) => tracing::warn!(
            error = %e,
            code = %shortcut.code,
            "failed to register the Seek global shortcut; another app may already use this combination"
        ),
    }
}
