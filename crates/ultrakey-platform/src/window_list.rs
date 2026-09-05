//! F-02 소스 C — 창 목록 (`CGWindowListCopyWindowInfo`, 이슈 #133).
//!
//! 화면에 보이는 일반 앱 창의 **제목**을 읽어 검색 대상에 넣는 보강 소스다.
//! Accessibility(소스 B)가 최전면 창 하나로 범위가 고정되는 반면, 이 소스는
//! **모든 레이어 0 창**(가려진 창 포함)을 대상으로 한다 — 사용자가 다른 창에
//! 가려 찾지 못한 창 제목도 후보로 노출되는 것이 의도된 동작이다.
//!
//! ⚠️ **Screen Recording 권한과의 관계**: `CGWindowListCopyWindowInfo` 자체는
//! 권한 없이도 호출에 성공하지만, 권한이 없으면 `kCGWindowName`(창 제목)이
//! 통상 **빈 문자열**로 돌아온다. 이 모듈은 제목이 빈 창을 항목에서 제외하므로
//! 권한이 없으면 실질적으로 빈 결과가 된다 — 조용한 실패다. F-02 는 애초에
//! Screen Recording 권한을 요구·보유하므로(`screen_recording.rs`), 실제 사용
//! 경로에서는 제목이 읽힌다.
//!
//! ⚠️ `kCGWindowListOptionOnScreenOnly` 를 쓰므로 이 소스는 **화면에 나타나
//! 있는 창**만 본다. "안 보이는 창"이란 위에서 말한 대로 가려진(occluded)
//! 창을 뜻한다 — 최소화된 창은 온스크린 목록에서 빠진다.

/// 레이어 0(일반 앱 창) 하나의 식별·기하 정보.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInfo {
    /// `kCGWindowNumber` — 이 세션에서 창을 유일하게 식별하는 값.
    pub window_id: u32,
    /// `kCGWindowOwnerPID` — 창을 소유한 프로세스의 pid.
    pub pid: i32,
    /// `kCGWindowOwnerName` — 소유 앱 이름. 키가 없으면 빈 문자열.
    pub owner_name: String,
    /// `kCGWindowName` — 창 제목. **비어 있으면 이 항목 자체를 제외**한다
    /// (필터 규칙).
    pub title: String,
    /// `kCGWindowBounds` — (x, y, width, height) 전역 좌표(포인트, 좌상단 원점).
    /// `kCGWindowBounds` 의 값은 CFDictionary 이고 키는 `X`/`Y`/`Width`/`Height`
    /// 다(CGWindow.h 76행이 `CGRectMakeWithDictionaryRepresentation` 을 가리키는
    /// dict 이며, 키 이름은 2026-09-05 이 기기에서 전수 출력 실측으로 확정).
    pub bounds: (f64, f64, f64, f64),
}

/// 레이어 0 창 목록을 얻는다. 실패·창 없음은 모두 빈 `Vec`(조용한 실패).
///
/// 필터 규칙(이슈 #133 판정 조건 1):
/// - `kCGWindowLayer == 0` 인 창만 — 일반 앱 창. 메뉴 막대(24 · `kCGMainMenuWindowLevel`)·
///   팝업 메뉴(101 · `kCGPopUpMenuWindowLevel`)·화면 보호기(1000 · `kCGScreenSaverWindowLevel`)
///   등 다른 레이어는 후보가 될 수 없다(`CGWindowLevel.h` 의 `kCGNormalWindowLevel == 0`).
/// - `kCGWindowName`(제목)이 비어 있는 창 제외 — 제목 없는 창은 검색 대상이 없다.
#[must_use]
pub fn front_layer_windows() -> Vec<WindowInfo> {
    #[cfg(target_os = "macos")]
    {
        macos_impl::front_layer_windows()
    }
    #[cfg(not(target_os = "macos"))]
    {
        stub_impl::front_layer_windows()
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    // `kCGWindow*` 접근자 이름은 Apple 의 C 상수 심볼명과 1:1 로 맞춘 것이다
    // — keychain.rs 의 `kSec*` 접근자와 같은 관례.
    #![allow(non_snake_case)]

    use super::WindowInfo;
    use objc2_core_foundation::{
        CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType,
    };

    /// macOS 실구현.
    pub fn front_layer_windows() -> Vec<WindowInfo> {
        let Some(array) = list_window_info() else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for dict in &*array {
            // `dict` 는 CFRetained<CFDictionary> 다 — CFArray 의 iter 는 원소를
            // retain 해서 넘긴다(값 수명은 루프 반복 안에서 안전).
            // SAFETY: `CGWindowListCopyWindowInfo` 가 돌려주는 dict 는 키가 전부
            // `CFString`(`kCGWindow*` 상수)이고 값은 CFNumber/CFString/CFDictionary
            // 류의 CFType 서브타입이다 — CGWindow.h 의 키별 문서가 그대로 보증한다.
            // `<CFString, CFType>` 제네릭은 그 사실을 러스트 타입으로 옮긴 것뿐이다.
            let dict = unsafe {
                &*CFRetained::as_ptr(&dict)
                    .as_ptr()
                    .cast::<CFDictionary<CFString, CFType>>()
            };
            // 필터 1 — 레이어 0(일반 앱 창)만.
            let Some(layer) = read_i32(dict, kCGWindowLayer()) else {
                continue;
            };
            if layer != 0 {
                continue;
            }
            let Some(window_id) = read_i32(dict, kCGWindowNumber()) else {
                continue;
            };
            let Some(pid) = read_i32(dict, kCGWindowOwnerPID()) else {
                continue;
            };
            // 필터 2 — 제목이 비어 있으면 항목 자체를 제외.
            let Some(title) = read_string(dict, kCGWindowName()) else {
                continue;
            };
            if title.is_empty() {
                continue;
            }
            let Some(bounds) = read_bounds(dict) else {
                continue;
            };
            let owner_name = read_string(dict, kCGWindowOwnerName()).unwrap_or_default();
            // 헤더는 kCGWindowNumber 를 "CFNumber 32-bit signed integer" 로
            // 명시하므로 i32 로 읽고, 부호 없는 필드로 정체성 보존 캐스트한다.
            out.push(WindowInfo {
                window_id: window_id as u32,
                pid,
                owner_name,
                title,
                bounds,
            });
        }
        out
    }

    /// `CGWindowListCopyWindowInfo(OptionOnScreenOnly, kCGNullWindowID)` 호출과
    /// 반환 배열의 해제 책임을 `CFRetained` 으로 묶는다.
    fn list_window_info() -> Option<CFRetained<CFArray<CFDictionary>>> {
        // SAFETY: 시그니처는 ffi.rs 의 헤더 근거 주석(CGWindow.h 169행)에 있다.
        // `OptionOnScreenOnly` + `kCGNullWindowID` 조합은 헤더가 명시한 사용법이다.
        let raw = unsafe {
            crate::ffi::CGWindowListCopyWindowInfo(
                crate::ffi::K_CG_WINDOW_LIST_OPTION_ON_SCREEN_ONLY,
                crate::ffi::K_CG_NULL_WINDOW_ID,
            )
        };
        if raw.is_null() {
            tracing::debug!("CGWindowListCopyWindowInfo returned NULL; treating window list as empty");
            return None;
        }
        // SAFETY: 위에서 null 이 아님을 확인했다. 타입 캐스트는 "배열의 원소가
        // CFDictionary 다"를 선언하는 것 — CGWindow.h 가 배열 원소를 "window
        // dictionaries" 라고 명시한다.
        let ptr = std::ptr::NonNull::new(raw.cast::<CFArray<CFDictionary>>())?;
        // SAFETY: CGWindowListCopyWindowInfo 는 CF_RETURNS_RETAINED 다(헤더:
        // "You should release the array when you are finished using it") —
        // 원시 포인터의 소유권을 넘겨받았으므로 CFRetained 가 drop 시점에 해제한다.
        Some(unsafe { CFRetained::from_raw(ptr) })
    }

    /// `kCGWindow*` dict(키=CFString, 값=CFType)에서 32비트 정수 키 값을 읽는다.
    fn read_i32(dict: &CFDictionary<CFString, CFType>, key: &'static CFString) -> Option<i32> {
        dict.get(key)?.downcast_ref::<CFNumber>()?.as_i32()
    }

    /// `kCGWindow*` dict에서 문자열 키 값을 읽는다.
    fn read_string(dict: &CFDictionary<CFString, CFType>, key: &'static CFString) -> Option<String> {
        let value = dict.get(key)?;
        let string = value.downcast_ref::<CFString>()?;
        Some(string.to_string())
    }

    /// `kCGWindowBounds`(CFDictionary)를 (x, y, width, height) 로 읽는다.
    ///
    /// 키 문자열 `X`/`Y`/`Width`/`Height` 는 이 기기 실측 확정값이다 —
    /// 2026-09-05, `CGWindowListCopyWindowInfo` 결과 dict 의 bounds 를 전수
    /// 출력해 확인했다. objc2-core-foundation 에는 이 키 상수가 없어
    /// 문자열 리터럴을 쓴다(CGWindow.h 76행이 가리키는
    /// `CGRectMakeWithDictionaryRepresentation` 의 문서도 같은 키다).
    fn read_bounds(dict: &CFDictionary<CFString, CFType>) -> Option<(f64, f64, f64, f64)> {
        let value = dict.get(kCGWindowBounds())?;
        let raw = value.downcast_ref::<CFDictionary>()?;
        // SAFETY: `kCGWindowBounds` 의 값 dict 도 키=CFString·값=CFType 구조다
        // (CGWindow.h: "The value of this key is a CFDictionary").
        let bounds =
            unsafe { &*(raw as *const CFDictionary).cast::<CFDictionary<CFString, CFType>>() };
        let x = read_cgfloat(bounds, "X")?;
        let y = read_cgfloat(bounds, "Y")?;
        let width = read_cgfloat(bounds, "Width")?;
        let height = read_cgfloat(bounds, "Height")?;
        Some((x, y, width, height))
    }

    /// bounds dict 에서 CGFloat 값을 읽는다.
    fn read_cgfloat(dict: &CFDictionary<CFString, CFType>, key: &str) -> Option<f64> {
        // CFDictionary::get 은 키를 해시 비교할 뿐 retain 하지 않으므로, 임시
        // `cfkey` 는 이 호출 동안만 살아 있으면 된다.
        let cfkey = CFString::from_str(key);
        dict.get(&cfkey)?.downcast_ref::<CFNumber>()?.as_cgfloat()
    }

    // ── 데이터 심볼 접근 (Option<&CFString> -> &CFString) ────────────────────

    fn kCGWindowNumber() -> &'static CFString {
        unsafe { crate::ffi::kCGWindowNumber }.expect("CoreGraphics kCGWindowNumber")
    }
    fn kCGWindowOwnerPID() -> &'static CFString {
        unsafe { crate::ffi::kCGWindowOwnerPID }.expect("CoreGraphics kCGWindowOwnerPID")
    }
    fn kCGWindowOwnerName() -> &'static CFString {
        unsafe { crate::ffi::kCGWindowOwnerName }.expect("CoreGraphics kCGWindowOwnerName")
    }
    fn kCGWindowName() -> &'static CFString {
        unsafe { crate::ffi::kCGWindowName }.expect("CoreGraphics kCGWindowName")
    }
    fn kCGWindowBounds() -> &'static CFString {
        unsafe { crate::ffi::kCGWindowBounds }.expect("CoreGraphics kCGWindowBounds")
    }
    fn kCGWindowLayer() -> &'static CFString {
        unsafe { crate::ffi::kCGWindowLayer }.expect("CoreGraphics kCGWindowLayer")
    }
}

/// 비-macOS 스텁 — `screen_capture.rs`·`ax_text.rs` 의 관례와 동일.
#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::WindowInfo;

    pub fn front_layer_windows() -> Vec<WindowInfo> {
        Vec::new()
    }
}