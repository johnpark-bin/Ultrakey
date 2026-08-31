//! 기기 식별자 — `IOPlatformUUID`(IOKit) 취득.
//!
//! `docs/spec/licensing-and-trial.md` §3.6: `device_id` 로 하드웨어 UUID 를 쓴다.
//! `IOPlatformExpertDevice`(IOKit 루트)의 `IOPlatformUUID` 프로퍼티를 읽는다.
//! 이 값은 시리얼번호·MAC 주소 같은 더 민감한 식별자가 아닌 **불투명 UUID 하나**라
//! 최소 식별자 원칙(§3.7)에 부합한다.
//!
//! ⚠️ `IOPlatformUUID` 는 부팅마다 달라질 수 있는 것으로 알려져 있다(애플이 문서화하지
//! 않은 동작). 엄밀한 것은 `(미확정)` 이지만, 이 프로젝트는 명세 §3.6 이 정한 대로
//! 이 값을 `device_id` 로 쓴다 — 하드웨어 교체 시 새 슬롯 소모를 원하는 동작이
//! 성립하려면 부팅 간 안정성이면 충분하다.

use std::ptr::NonNull;

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::*;
    use crate::ffi::{self, IoObjectT};
    use objc2_core_foundation::{CFDictionary, CFRetained, CFString, CFType};

    /// IOKit 루트 장치 클래스명(근거: `IOKitLib.h` — `IOPlatformExpertDevice`).
    const IO_PLATFORM_EXPERT_DEVICE: &[u8] = b"IOPlatformExpertDevice\0";
    /// 프로퍼티 키(근거: `IOKitKeys.h` — `#define kIOPlatformUUIDKey "IOPlatformUUID"`).
    const IO_PLATFORM_UUID_KEY: &str = "IOPlatformUUID";

    /// `IOPlatformExpertDevice` 의 `IOPlatformUUID` 를 읽는다.
    ///
    /// 실패(장치 없음·읽기 불가)하면 `None` — 호출자가 폴백 전략을 정한다.
    fn read_platform_uuid() -> Option<String> {
        // SAFETY: `IOServiceMatching` 은 NULL 종료 C 문자열(클래스명)을 받아
        // CFMutableDictionary 를 만든다(CF_RETURNS_RETAINED).
        let raw_matching =
            unsafe { ffi::IOServiceMatching(IO_PLATFORM_EXPERT_DEVICE.as_ptr().cast()) };
        if raw_matching.is_null() {
            return None;
        }
        // SAFETY: `raw_matching` 은 위에서 만든 유효한 CFMutableDictionary 를 가리킨다.
        let matching = unsafe { CFRetained::from_raw(NonNull::new(raw_matching)?) };

        // SAFETY: `IOServiceGetMatchingService` 는 `matching` 의 참조 하나를 소비한다
        // (CF_RELEASES_ARGUMENT) — `into_raw` 로 소유권을 넘긴다.
        let service: IoObjectT = unsafe {
            ffi::IOServiceGetMatchingService(
                ffi::kIOMainPortDefault,
                CFRetained::into_raw(matching).as_ptr() as *const CFDictionary,
            )
        };
        if service == 0 {
            return None;
        }

        // SAFETY: `service` 는 성공적으로 얻은 io_service_t 이고, `CFString::from_str`
        // 로 만든 `key` 가 이 호출 동안 유효하다. 반환값은 CF_RETURNS_RETAINED.
        let cf_key = CFString::from_str(IO_PLATFORM_UUID_KEY);
        let raw = unsafe {
            ffi::IORegistryEntryCreateCFProperty(
                service,
                &*cf_key as *const CFString,
                std::ptr::null(),
                0,
            )
        };
        // SAFETY: `service` 를 반드시 해제한다(io_object_t 는 CF 규약을 따른다).
        unsafe { ffi::IOObjectRelease(service) };

        let ptr = NonNull::new(raw as *mut CFType)?;
        // SAFETY: `IORegistryEntryCreateCFProperty` 는 CF_RETURNS_RETAINED 규칙 —
        // 이 포인터의 소유권은 우리에게 있다.
        let retained = unsafe { CFRetained::from_raw(ptr) };
        let string = retained.downcast::<CFString>().ok()?;
        let value = string.to_string();
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }

    /// 이 기기의 하드웨어 UUID. 읽지 못하면 `None`.
    pub fn hardware_device_id() -> Option<String> {
        read_platform_uuid()
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::hardware_device_id;

/// 비-macOS 폴백 — `device_id` 가 없으면 `None`.
#[cfg(not(target_os = "macos"))]
pub fn hardware_device_id() -> Option<String> {
    None
}
