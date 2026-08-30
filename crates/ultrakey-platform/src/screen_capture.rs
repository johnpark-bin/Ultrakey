//! F-02 소스 A · A1 — 화면 캡처 (`CGDisplayCreateImage` 계열).
//!
//! ⭐ **ScreenCaptureKit 이 아니다.** `docs/spec/seek-text-detection.md` §3.2 가
//! 실측(v1.66 번들 `nm -u`)으로 확정한 대로, 원본 SuperKey 는 ScreenCaptureKit
//! 심볼을 전혀 링크하지 않고 `CGDisplayCreateImage` / `CGDisplayCreateImageForRect`
//! 만 쓴다. 이 모듈도 같은 API 를 쓴다 — 그래야 `LSMinimumSystemVersion = 12.0`
//! (제품 결정 D1)과 정합한다.
//!
//! ⚠️ 이 두 함수는 macOS 14 에서 deprecated 다(`platform-constraints.md` P9).
//! 그럼에도 v1.66(2026-06 빌드, SDK macosx26.5)이 여전히 이것으로 출하 중이다.
//! `#[allow(deprecated)]` 를 **이 모듈 안에서만** 국소적으로 허용하고, 교체
//! 시점이 오면 이 파일 하나만 바꾸면 되도록 캡처 계층을 여기에 가둔다.
//!
//! ⚠️ **Screen Recording 권한이 없어도 이 호출은 실패하지 않는다.** 오류 대신
//! 데스크톱 배경(또는 벽지만 있는 빈 화면)이 돌아온다 — 조용한 실패다.
//! 그 판정은 `screen_recording.rs` 가 맡는다.

/// 한 디스플레이의 기하 정보. 좌표는 전부 **전역 화면 좌표(포인트, 좌상단 원점)** 다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayGeometry {
    /// `CGDirectDisplayID`.
    pub display_id: u32,
    /// `CGDisplayBounds(displayID)` — 전역 원점 x (포인트).
    pub origin_x: f64,
    /// `CGDisplayBounds(displayID)` — 전역 원점 y (포인트).
    pub origin_y: f64,
    /// 디스플레이 논리 폭 (포인트).
    pub width_pt: f64,
    /// 디스플레이 논리 높이 (포인트).
    pub height_pt: f64,
}

/// 캡처된 한 디스플레이의 이미지와 그 기하 정보.
#[cfg(target_os = "macos")]
pub struct CapturedDisplay {
    /// 이 캡처가 어느 디스플레이의 것인가.
    pub geometry: DisplayGeometry,
    /// 캡처 이미지의 **픽셀** 폭.
    pub image_width_px: usize,
    /// 캡처 이미지의 **픽셀** 높이.
    pub image_height_px: usize,
    /// ⭐ 배율. `NSScreen.backingScaleFactor` 를 따로 조회하지 않고
    /// `image_width_px / width_pt` 로 **유도**한다 — 결정 근거는 모듈 문서 참조.
    pub scale: f64,
    /// 캡처 결과.
    pub image: objc2_core_foundation::CFRetained<objc2_core_graphics::CGImage>,
}

#[cfg(target_os = "macos")]
impl core::fmt::Debug for CapturedDisplay {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CapturedDisplay")
            .field("geometry", &self.geometry)
            .field("image_width_px", &self.image_width_px)
            .field("image_height_px", &self.image_height_px)
            .field("scale", &self.scale)
            .finish_non_exhaustive()
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::{CapturedDisplay, DisplayGeometry};
    use objc2_core_graphics::{CGDisplayBounds, CGGetActiveDisplayList};

    /// 한 번에 조회할 디스플레이 수 상한. 실사용에서 이보다 많은 화면을 붙이는
    /// 경우는 없다고 보고 고정 배열로 받는다(할당 없음).
    const MAX_DISPLAYS: u32 = 16;

    /// 활성 디스플레이 목록과 각각의 전역 기하를 돌려준다.
    ///
    /// 미러링 중인 화면은 `CGGetActiveDisplayList` 가 "활성" 으로 한 개만
    /// 돌려주므로 중복 캡처가 생기지 않는다.
    pub fn active_displays() -> Vec<DisplayGeometry> {
        let mut ids = [0u32; MAX_DISPLAYS as usize];
        let mut count: u32 = 0;
        // SAFETY: `ids` 는 `MAX_DISPLAYS` 개를 담을 수 있는 유효한 가변 버퍼이고,
        // `count` 는 유효한 출력 포인터다. CG 는 버퍼 크기를 넘겨 쓰지 않는다.
        let err = unsafe { CGGetActiveDisplayList(MAX_DISPLAYS, ids.as_mut_ptr(), &mut count) };
        if err != objc2_core_graphics::CGError::Success {
            tracing::warn!(?err, "CGGetActiveDisplayList 실패 — 디스플레이 목록 비어 있음");
            return Vec::new();
        }
        ids.iter()
            .take(count as usize)
            .map(|&display_id| {
                let bounds = CGDisplayBounds(display_id);
                DisplayGeometry {
                    display_id,
                    origin_x: bounds.origin.x,
                    origin_y: bounds.origin.y,
                    width_pt: bounds.size.width,
                    height_pt: bounds.size.height,
                }
            })
            .collect()
    }

    /// 디스플레이 하나를 통째로 캡처한다 (A1, 전체 화면 경로).
    ///
    /// `None` 은 캡처 자체가 실패한 경우다. ⚠️ **권한이 없을 때는 `None` 이
    /// 아니라 데스크톱 배경 이미지가 돌아온다** — `screen_recording.rs` 참조.
    pub fn capture_display(geometry: DisplayGeometry) -> Option<CapturedDisplay> {
        // ⚠️ deprecated 를 의도적으로 쓴다 — 모듈 문서의 근거 참조.
        #[allow(deprecated)]
        let image = objc2_core_graphics::CGDisplayCreateImage(geometry.display_id)?;
        Some(finish(geometry, image))
    }

    /// 디스플레이의 특정 영역만 캡처한다 (A1, `Only Seek in the frontmost window`
    /// 경로 — §3.3.2). 인자는 **전역 화면 좌표(포인트, 좌상단 원점)** 다.
    ///
    /// ⭐ `CGRect` 를 인자로 노출하지 않고 `f64` 넷을 받는다 — 이 크레이트가
    /// FFI 경계이므로, 상위 크레이트가 `objc2-core-foundation` 타입을 알아야
    /// 할 이유를 만들지 않는다(`docs/dev/architecture.md` §1 의 경계 원칙).
    pub fn capture_display_rect(
        geometry: DisplayGeometry,
        x: f64,
        y: f64,
        width: f64,
        height: f64,
    ) -> Option<CapturedDisplay> {
        let rect = objc2_core_foundation::CGRect::new(
            objc2_core_foundation::CGPoint::new(x, y),
            objc2_core_foundation::CGSize::new(width, height),
        );
        #[allow(deprecated)]
        let image = objc2_core_graphics::CGDisplayCreateImageForRect(geometry.display_id, rect)?;
        // 잘라낸 영역이 캡처의 기준이 되므로, 좌표 변환이 쓸 원점·논리 크기를
        // 그 사각형으로 바꿔 둔다. 그래야 §3.2.3 4단계 오프셋이 그대로 맞는다.
        let cropped = DisplayGeometry {
            display_id: geometry.display_id,
            origin_x: rect.origin.x,
            origin_y: rect.origin.y,
            width_pt: rect.size.width,
            height_pt: rect.size.height,
        };
        Some(finish(cropped, image))
    }

    fn finish(
        geometry: DisplayGeometry,
        image: objc2_core_foundation::CFRetained<objc2_core_graphics::CGImage>,
    ) -> CapturedDisplay {
        let image_width_px = objc2_core_graphics::CGImage::width(Some(&image));
        let image_height_px = objc2_core_graphics::CGImage::height(Some(&image));
        // ⭐ 배율을 `NSScreen.backingScaleFactor` 로 따로 구하지 않는다.
        //
        // 근거: `NSScreen` 은 `CGDirectDisplayID` 를 직접 노출하지 않아
        // (`deviceDescription` 의 `NSScreenNumber` 를 거쳐야 한다) 매핑이 한 단계
        // 더 필요하고, 그 매핑이 틀리면 다중 디스플레이에서 **엉뚱한 화면의
        // 배율**을 쓰게 된다 — 명세 §5 #3 이 지목한 v1.55 회귀의 정확한 형태다.
        // 반면 "이 캡처 이미지의 픽셀 폭 ÷ 이 디스플레이의 포인트 폭" 은 정의상
        // 그 캡처의 실제 배율이므로 매핑 오류가 원천적으로 불가능하다.
        // 기각한 대안: `NSScreen.screens` 순회 후 `NSScreenNumber` 대조.
        let scale = if geometry.width_pt > 0.0 {
            image_width_px as f64 / geometry.width_pt
        } else {
            1.0
        };
        CapturedDisplay {
            geometry,
            image_width_px,
            image_height_px,
            scale,
            image,
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{active_displays, capture_display, capture_display_rect};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::DisplayGeometry;

    pub fn active_displays() -> Vec<DisplayGeometry> {
        Vec::new()
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::active_displays;
