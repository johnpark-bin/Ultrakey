//! F-02 소스 A · A1.5 — 캡처 이미지 전처리 (CoreImage).
//!
//! `docs/spec/seek-text-detection.md` §3.2.1 이 실측(번들 심볼)으로 확정한
//! 필터 5종을 CoreImage 로 적용한다:
//! `CILanczosScaleTransform` · `CIPhotoEffectMono` · `CIPhotoEffectNoir` ·
//! `CIMaximumComponent` · `CIMinimumComponent`.
//!
//! ⭐ **어떤 조건에서 어떤 필터를 고르는지는 명세가 `(미확정)`(Q-k)으로 남겼다.**
//! 이 구현은 §3.2.1 이 제시한 두 갈래(적응형 / 고정 순서) 중 **고정 순서**를
//! 고르되 프리셋으로 교체 가능하게 두었다 — 근거는 [`PreprocessPreset`] 문서.

/// 전처리 프리셋. 명세 §3.2.1 Q-k 가 미확정으로 남긴 "필터 선택 로직" 자리를
/// 채우는 값이다.
///
/// **이 구현이 고른 기본값은 [`PreprocessPreset::None`]** 이다(결정 S-3).
/// 근거는 실측 — `docs/dev/seek-ocr-latency-spike.md` §4: 일반적인 데스크톱
/// 화면에서 프리셋별 후보 집합의 차이는 거의 전부 **같은 텍스트의 다른
/// 오인식**이었고, 평균 신뢰도는 Noir 가 오히려 가장 낮았다. 이득이 측정되지
/// 않은 비용을 기본으로 켜지 않는다.
///
/// ⚠️ 그 실측은 명세 §5 #12 가 지목한 실패 사례(극소 텍스트, 검정 배경 위 특정
/// 파란색, 인접 줄)를 **포함하지 않은 화면 표본**에서 나온 것이다. 프리셋의
/// 진짜 값어치는 그 사례들에서만 드러나므로 열거값 자체는 남겨 둔다.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum PreprocessPreset {
    /// 전처리 없음 — 캡처 이미지를 그대로 Vision 에 넘긴다.
    #[default]
    None,
    /// 흑백 변환만(`CIPhotoEffectNoir`). 저대비 텍스트 대비 강화(§5 #12).
    Noir,
    /// 흑백 + 다운스케일(`CIPhotoEffectNoir` → `CILanczosScaleTransform`).
    /// `scale` 은 1.0 미만이면 축소다.
    NoirDownscale {
        /// `CILanczosScaleTransform` 의 `inputScale`.
        scale: f32,
    },
    /// 다운스케일만(`CILanczosScaleTransform`).
    Downscale {
        /// `CILanczosScaleTransform` 의 `inputScale`.
        scale: f32,
    },
    /// RGB 최대 성분 추출(`CIMaximumComponent`) — 검정 배경 위 유채색 텍스트(§5 #12).
    MaximumComponent,
    /// RGB 최소 성분 추출(`CIMinimumComponent`) — 밝은 배경 위 유채색 텍스트.
    MinimumComponent,
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::PreprocessPreset;
    use objc2::rc::Retained;
    use objc2::AllocAnyThread;
    use objc2_core_graphics::CGImage;
    use objc2_core_image::CIImage;
    use objc2_foundation::{NSDictionary, NSNumber, NSString};

    /// 캡처 `CGImage` 에 프리셋의 필터 사슬을 적용해 `CIImage` 를 만든다.
    ///
    /// ⭐ 결과를 `CGImage` 로 되돌리지 않는다 — `VNImageRequestHandler` 가
    /// `initWithCIImage:options:` 를 직접 받으므로 `CIContext` 왕복(GPU→CPU
    /// 래스터화)을 건너뛸 수 있다. 그 왕복이 전처리 비용의 대부분이다.
    pub fn preprocess(image: &CGImage, preset: PreprocessPreset) -> Retained<CIImage> {
        // SAFETY: `image` 는 살아 있는 `CGImage` 이고, `initWithCGImage:` 는
        // 그 내용을 복사하지 않고 참조만 잡되 CIImage 가 수명을 관리한다.
        let base = unsafe { CIImage::initWithCGImage(CIImage::alloc(), image) };
        match preset {
            PreprocessPreset::None => base,
            PreprocessPreset::Noir => apply(&base, "CIPhotoEffectNoir"),
            PreprocessPreset::MaximumComponent => apply(&base, "CIMaximumComponent"),
            PreprocessPreset::MinimumComponent => apply(&base, "CIMinimumComponent"),
            PreprocessPreset::Downscale { scale } => lanczos(&base, scale),
            PreprocessPreset::NoirDownscale { scale } => {
                let mono = apply(&base, "CIPhotoEffectNoir");
                lanczos(&mono, scale)
            }
        }
    }

    fn apply(image: &CIImage, filter_name: &str) -> Retained<CIImage> {
        let name = NSString::from_str(filter_name);
        // SAFETY: `filter_name` 은 CoreImage 내장 필터 이름이고 인자가 없는
        // 필터이므로 기본 파라미터로 적용하면 된다. 이름이 틀리면 CoreImage 는
        // 예외를 던지므로, 호출하는 이름은 명세 §3.2.1 실측 목록으로 한정한다.
        unsafe { image.imageByApplyingFilter(&name) }
    }

    fn lanczos(image: &CIImage, scale: f32) -> Retained<CIImage> {
        let name = NSString::from_str("CILanczosScaleTransform");
        let key = NSString::from_str("inputScale");
        let value = NSNumber::new_f32(scale);
        let params = NSDictionary::from_slices::<NSString>(
            &[&*key],
            &[&*value as &objc2::runtime::AnyObject],
        );
        // SAFETY: `CILanczosScaleTransform` 은 `inputScale`(NSNumber) 를 받는
        // 내장 필터다. 키 이름·값 타입이 맞으므로 파라미터 사전이 유효하다.
        unsafe { image.imageByApplyingFilter_withInputParameters(&name, &params) }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::preprocess;
