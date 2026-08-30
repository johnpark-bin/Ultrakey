//! F-02 소스 A · A2 — Vision OCR (`VNRecognizeTextRequest`).
//!
//! `docs/spec/seek-text-detection.md` §3.2 의 A2 단계. 이 모듈은 **정규화
//! 좌표(좌하단 원점)를 그대로 돌려준다** — 전역 화면 좌표로의 변환(§3.2.3)은
//! `unsafe` 가 필요 없는 순수 로직이라 `ultrakey-seek` 크레이트가 맡는다.

/// Vision 이 돌려준 관측 하나. 좌표는 **정규화(0~1), 좌하단 원점** 이다.
#[derive(Debug, Clone, PartialEq)]
pub struct RawObservation {
    /// `topCandidates(1)` 의 첫 후보 문자열.
    pub text: String,
    /// `boundingBox.origin.x` (정규화).
    pub bbox_x: f64,
    /// `boundingBox.origin.y` (정규화, 좌하단 원점).
    pub bbox_y: f64,
    /// `boundingBox.size.width` (정규화).
    pub bbox_w: f64,
    /// `boundingBox.size.height` (정규화).
    pub bbox_h: f64,
    /// `VNRecognizedText.confidence` (0~1).
    pub confidence: f32,
}

/// 인식 파라미터. 명세 §3.2.2 의 표를 그대로 옮긴 것이며, 그 표가 `(추정)` 으로
/// 남긴 값들의 기본값을 여기서 고정한다.
#[derive(Debug, Clone)]
pub struct RecognitionParams {
    /// `true` 면 `.accurate`, `false` 면 `.fast`. 기본 `.accurate`(§3.2.2).
    pub accurate: bool,
    /// `usesLanguageCorrection`. 기본 `false` — 짧은 UI 라벨("OK")이 다른
    /// 단어로 교정되는 것을 막는다(§3.2.2).
    pub uses_language_correction: bool,
    /// `minimumTextHeight`. `0.0` 이면 Vision 기본값을 그대로 둔다 —
    /// "extra small text" 가 확인된 실패 사례이므로 올려 잡지 않는다(§3.2.2).
    pub minimum_text_height: f32,
    /// `recognitionLanguages`. 비어 있으면 Vision 기본값을 그대로 둔다.
    ///
    /// ⭐⭐ **실측으로 확정된 것**(`docs/dev/seek-ocr-latency-spike.md` §3):
    /// Vision 의 기본 인식 언어는 **영어뿐**이다. 화면이 한글로 가득해도 후보가
    /// 하나도 나오지 않는다 — 조용한 실패다. 한국어를 인식시키려면 `"ko-KR"` 을
    /// 넣되 **목록의 첫 번째**여야 한다: `["en-US", "ko-KR"]` 는 결과가
    /// `["en-US"]` 와 **완전히 동일**했다(관측 수·시간·신뢰도 전부).
    ///
    /// ⚠️ 대가가 크다 — `["ko-KR", "en-US"]` 는 전체 화면 기준 약 775 ms →
    /// **약 3.1 s** 로 3~4배 느려지고 평균 신뢰도도 0.83 → 0.59 로 떨어진다.
    /// 그래서 기본값은 비워 두고(결정 S-4) 사용자가 켜는 설정으로 노출한다.
    pub languages: Vec<String>,
}

impl Default for RecognitionParams {
    fn default() -> Self {
        Self {
            accurate: true,
            uses_language_correction: false,
            minimum_text_height: 0.0,
            languages: Vec::new(),
        }
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::{RawObservation, RecognitionParams};
    use objc2::rc::Retained;
    use objc2::AllocAnyThread;
    use objc2_core_image::CIImage;
    use objc2_foundation::{NSArray, NSDictionary, NSString};
    use objc2_vision::{
        VNImageRequestHandler, VNRecognizeTextRequest, VNRequest, VNRequestTextRecognitionLevel,
    };

    /// 전처리된 `CIImage` 하나에 OCR 을 돌린다 (디스플레이 1개분).
    ///
    /// `performRequests:error:` 는 **동기**다 — 모든 요청이 끝난 뒤 돌아온다.
    /// 그래서 호출자가 디스플레이별로 스레드를 나누면 그대로 병렬이 된다.
    pub fn recognize_text(
        image: &CIImage,
        params: &RecognitionParams,
    ) -> Result<Vec<RawObservation>, String> {
        let request = build_request(params);
        let handler = build_handler(image);

        let requests: Retained<NSArray<VNRequest>> =
            NSArray::from_slice(&[&*request as &VNRequest]);
        handler
            .performRequests_error(&requests)
            .map_err(|e| format!("VNImageRequestHandler.performRequests 실패: {e:?}"))?;

        let Some(results) = request.results() else {
            return Ok(Vec::new());
        };
        let mut out = Vec::with_capacity(results.len());
        for observation in results.iter() {
            // SAFETY: `boundingBox` 는 인자 없는 읽기 전용 프로퍼티다.
            let bbox = unsafe { observation.boundingBox() };
            let candidates = observation.topCandidates(1);
            let Some(best) = candidates.iter().next() else {
                continue;
            };
            let text = best.string().to_string();
            if text.is_empty() {
                continue;
            }
            out.push(RawObservation {
                text,
                bbox_x: bbox.origin.x,
                bbox_y: bbox.origin.y,
                bbox_w: bbox.size.width,
                bbox_h: bbox.size.height,
                confidence: best.confidence(),
            });
        }
        Ok(out)
    }

    fn build_request(params: &RecognitionParams) -> Retained<VNRecognizeTextRequest> {
        let request = VNRecognizeTextRequest::new();
        request.setRecognitionLevel(if params.accurate {
            VNRequestTextRecognitionLevel::Accurate
        } else {
            VNRequestTextRecognitionLevel::Fast
        });
        request.setUsesLanguageCorrection(params.uses_language_correction);
        if params.minimum_text_height > 0.0 {
            request.setMinimumTextHeight(params.minimum_text_height);
        }
        if !params.languages.is_empty() {
            let strings: Vec<Retained<NSString>> = params
                .languages
                .iter()
                .map(|s| NSString::from_str(s))
                .collect();
            let refs: Vec<&NSString> = strings.iter().map(|s| &**s).collect();
            request.setRecognitionLanguages(&NSArray::from_slice(&refs));
        }
        request
    }

    fn build_handler(image: &CIImage) -> Retained<VNImageRequestHandler> {
        let options: Retained<NSDictionary<objc2_vision::VNImageOption, objc2::runtime::AnyObject>> =
            NSDictionary::new();
        // SAFETY: `options` 는 빈 사전이라 제네릭 타입 요구가 자명하게 만족된다.
        // `image` 는 호출 동안 살아 있고, 핸들러는 이미지를 스스로 retain 한다.
        unsafe { VNImageRequestHandler::initWithCIImage_options(
            VNImageRequestHandler::alloc(),
            image,
            &options,
        ) }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::recognize_text;
