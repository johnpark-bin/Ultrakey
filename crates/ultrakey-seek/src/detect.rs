//! F-02 검출 파이프라인 조립 — 소스 A + 소스 B → 병합된 후보 집합.
//!
//! 이 모듈만 `ultrakey-platform`(macOS FFI)에 의존한다. 나머지 모듈은 순수
//! 로직이라 macOS 없이 테스트된다.
//!
//! ## ⭐ 순서가 실측으로 정해졌다 (`docs/dev/seek-ocr-latency-spike.md`)
//!
//! - **S-2 — 캡처를 전부 먼저 끝낸다** (≈31 ms). 디스플레이 1 을 OCR 하는
//!   335 ms 동안 화면이 바뀌면 디스플레이 2 의 캡처는 *다른 시점의 화면*이
//!   된다(명세 §5 #9). 캡처는 전체 비용의 4 % 뿐이라 몰아서 해도 싸다.
//! - **S-1 — OCR 은 디스플레이별 순차**. `VNRecognizeTextRequest` 는
//!   병렬화되지 않는다(측정된 이득 **0 %**). 스레드를 늘리면 낭비일 뿐
//!   아니라 첫 결과 도착을 335 ms → 779 ms 로 **늦춘다**.
//! - **S-1 — 끝나는 대로 하나씩 내보낸다.** 그래서 이 모듈은 디스플레이별
//!   콜백([`detect_candidates`] 의 `on_display`)을 받는다.
//! - **S-6 — F-01 은 이것이 끝나기를 기다리면 안 된다.** 전체 화면 검출은
//!   약 775 ms 다. 세션은 즉시 열고 후보는 증분으로 받아야 한다.

use crate::candidate::TextCandidate;
use crate::merge::{merge_candidates, MergeParams};
use crate::transform::{normalized_bbox_to_global, DisplayFrame};
use ultrakey_platform::ax_text::{AxScanError, AxScanParams};
use ultrakey_platform::image_preprocess::PreprocessPreset;
use ultrakey_platform::screen_capture;
use ultrakey_platform::screen_recording::ScreenRecordingStatus;
use ultrakey_platform::vision_ocr::RecognitionParams;

/// 검출 파라미터.
#[derive(Debug, Clone, Default)]
pub struct DetectionParams {
    /// A1.5 전처리 프리셋. 기본 `None`(결정 S-3).
    pub preprocess: PreprocessPreset,
    /// A2 인식 파라미터.
    pub recognition: RecognitionParams,
    /// 소스 B 를 켤 것인가 — `Seek using macOS accessibility` 체크박스(§4).
    /// 기본 **꺼짐**(실측: plist 에 키 부재 = 기본값 OFF).
    pub use_accessibility: bool,
    /// 소스 B 순회 파라미터. `min_char_count` 가 `minAxCharCount`(§3.3.1)다.
    pub ax: AxScanParams,
    /// 병합 임계값(§3.4 M2).
    pub merge: MergeParams,
    /// `Only Seek in the frontmost window`(§3.3.2) — 켜지면 소스 A 의 캡처
    /// 범위를 이 사각형으로 좁힌다. ⭐ 소스 B 의 범위는 이 값과 **무관**하게
    /// 언제나 최전면 창이다.
    ///
    /// 창 프레임을 어떻게 얻는지는 명세가 `(미확정)` 으로 남겼으므로(§3.3.2)
    /// 이 모듈은 **호출자가 넘겨준 사각형을 쓴다** — 그 획득 책임은 F-01 이다.
    pub frontmost_window_rect: Option<crate::transform::Rect>,
}

/// 디스플레이 하나의 OCR 결과 — 끝나는 대로 하나씩 전달된다(S-1).
#[derive(Debug, Clone)]
pub struct DisplayResult {
    /// `CGDirectDisplayID`.
    pub display_id: u32,
    /// 이 디스플레이에서 나온 OCR 후보.
    pub candidates: Vec<TextCandidate>,
    /// 캡처 시작부터 이 디스플레이의 OCR 이 끝나기까지 걸린 시간(ms).
    pub elapsed_ms: f64,
}

/// 검출 전체의 결과.
#[derive(Debug, Clone)]
pub struct DetectionOutcome {
    /// 병합·정렬이 끝난 후보 목록(§3.4).
    pub candidates: Vec<TextCandidate>,
    /// Screen Recording 권한 상태 — 소스 A 가 0개일 때 그 **이유**를 구분한다.
    pub screen_recording: ScreenRecordingStatus,
    /// 소스 B 가 실패했다면 그 이유. `None` 이면 성공했거나 애초에 껐다.
    pub ax_error: Option<AxScanError>,
    /// 소스 A 가 만든 후보 수(병합 전).
    pub ocr_count: usize,
    /// 소스 B 가 만든 후보 수(병합 전).
    pub ax_count: usize,
    /// 캡처 전체에 걸린 시간(ms).
    pub capture_ms: f64,
    /// 검출 전체에 걸린 시간(ms).
    pub total_ms: f64,
}

impl DetectionOutcome {
    /// ⚠️ **조용한 실패 판정** — 소스 A 의 결과를 믿을 수 없는가.
    ///
    /// ⭐⭐ **후보 수를 보지 않고 권한 상태만 본다.** 처음에는
    /// `ocr_count == 0 && 권한 없음` 으로 썼는데, **실기기 검증이 그것을
    /// 반증했다**: 권한 없이 캡처하면 데스크톱 배경만 오는 것이 아니라
    /// **메뉴 막대까지 함께 온다**(메뉴 막대는 보호 대상이 아니다). 실제로
    /// 권한을 되돌린 뒤 캡처했을 때 `Gitkraken` · `window Help` · `view` 등
    /// **후보 5개**가 나왔다 — 0개가 아니므로 그 조건은 발화하지 않았을 것이다.
    ///
    /// 즉 "후보가 0개인가" 는 권한 부재의 신뢰할 수 있는 신호가 아니다.
    /// 권한이 없으면 후보가 몇 개 나오든 **소스 A 는 화면의 실제 내용을 보고
    /// 있지 않다.** 명세 §5 는 #1(권한 없음)과 #2(텍스트 없는 화면)를 서로
    /// 다른 케이스로 두는데, 그 둘을 가르는 것은 후보 수가 아니라 권한이다.
    #[must_use]
    pub fn ocr_blocked_by_permission(&self) -> bool {
        self.screen_recording == ScreenRecordingStatus::Denied
    }

    /// 소스 A·B 를 합쳐 후보가 하나도 없는가 (명세 §5 #2).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

/// 후보를 검출한다.
///
/// `on_display` 는 디스플레이 하나의 OCR 이 끝날 때마다 호출된다 — F-01/F-03
/// 이 **첫 후보를 약 335 ms 에** 그릴 수 있게 하는 자리다(S-1·S-6).
/// 증분 전달이 필요 없으면 `|_| {}` 를 넘기면 된다.
pub fn detect_candidates(
    params: &DetectionParams,
    mut on_display: impl FnMut(&DisplayResult),
) -> DetectionOutcome {
    let started = std::time::Instant::now();

    // ── 소스 A ─────────────────────────────────────────────────────────────
    // S-2: 모든 디스플레이를 **먼저** 캡처한다. 그래야 화면들의 시점이 같다.
    let displays = screen_capture::active_displays();
    let mut captures = Vec::with_capacity(displays.len());
    for geometry in displays {
        let captured = match params.frontmost_window_rect {
            Some(rect) if rects_intersect(&rect, &geometry) => {
                screen_capture::capture_display_rect(
                    geometry,
                    rect.x,
                    rect.y,
                    rect.width,
                    rect.height,
                )
            }
            // 창이 이 디스플레이에 없으면 캡처 자체를 건너뛴다 — 범위를
            // 좁히라고 한 설정인데 다른 화면을 통째로 뜨면 모순이다.
            Some(_) => None,
            None => screen_capture::capture_display(geometry),
        };
        if let Some(c) = captured {
            captures.push(c);
        }
    }
    let capture_ms = started.elapsed().as_secs_f64() * 1000.0;

    // S-1: OCR 은 순차. 끝나는 대로 하나씩 내보낸다.
    let mut ocr = Vec::new();
    for captured in &captures {
        let display_started = std::time::Instant::now();
        let frame = DisplayFrame {
            display_id: captured.geometry.display_id,
            origin_x: captured.geometry.origin_x,
            origin_y: captured.geometry.origin_y,
            image_width_px: captured.image_width_px as f64,
            image_height_px: captured.image_height_px as f64,
            scale: captured.scale,
        };
        let image =
            ultrakey_platform::image_preprocess::preprocess(&captured.image, params.preprocess);
        let observations = match ultrakey_platform::vision_ocr::recognize_text(
            &image,
            &params.recognition,
        ) {
            Ok(v) => v,
            Err(e) => {
                // 한 디스플레이가 실패해도 나머지로 계속한다(§5 #2).
                tracing::warn!(display_id = frame.display_id, error = %e, "OCR failed; skipping this display");
                Vec::new()
            }
        };
        let candidates: Vec<TextCandidate> = observations
            .into_iter()
            .map(|o| {
                // A3 — §3.2.3 의 좌표 변환.
                let rect = normalized_bbox_to_global(&frame, o.bbox_x, o.bbox_y, o.bbox_w, o.bbox_h);
                // A4 — 후보 생성.
                TextCandidate::ocr(o.text, rect, o.confidence, frame.display_id)
            })
            .collect();
        let result = DisplayResult {
            display_id: frame.display_id,
            candidates: candidates.clone(),
            elapsed_ms: display_started.elapsed().as_secs_f64() * 1000.0,
        };
        on_display(&result);
        ocr.extend(candidates);
    }

    // ── 소스 B ─────────────────────────────────────────────────────────────
    // ⭐ 언제나 최전면 창 한정 — `frontmost_window_rect` 와 무관하다(§3.3.2).
    let mut ax = Vec::new();
    let mut ax_error = None;
    if params.use_accessibility {
        match ultrakey_platform::ax_text::scan_frontmost_window(&params.ax) {
            Ok(elements) => {
                ax = elements
                    .into_iter()
                    .map(|e| {
                        TextCandidate::accessibility(
                            e.text,
                            crate::transform::Rect {
                                x: e.x,
                                y: e.y,
                                width: e.width,
                                height: e.height,
                            },
                        )
                    })
                    .collect();
            }
            Err(e) => {
                tracing::warn!(error = ?e, "AX traversal failed; continuing with source A results only");
                ax_error = Some(e);
            }
        }
    }

    let ocr_count = ocr.len();
    let ax_count = ax.len();
    let candidates = merge_candidates(ocr, ax, params.merge);

    DetectionOutcome {
        candidates,
        screen_recording: ultrakey_platform::screen_recording::status(),
        ax_error,
        ocr_count,
        ax_count,
        capture_ms,
        total_ms: started.elapsed().as_secs_f64() * 1000.0,
    }
}

/// 창 사각형이 이 디스플레이와 겹치는가.
fn rects_intersect(rect: &crate::transform::Rect, g: &screen_capture::DisplayGeometry) -> bool {
    rect.x < g.origin_x + g.width_pt
        && rect.x + rect.width > g.origin_x
        && rect.y < g.origin_y + g.height_pt
        && rect.y + rect.height > g.origin_y
}
