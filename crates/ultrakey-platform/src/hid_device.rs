//! 디바이스 열거 — `hidutil list --matching {...}` 대응, IOKit 직접 경로.
//!
//! `docs/spec/per-device-settings.md` §3.2 · `docs/research/per-device-hid-spike.md`
//! §10 이 요구하는 API 다: 알림 등록 없이 `IOServiceGetMatchingServices` 로 한 번
//! 순회해 현재 붙어 있는 키보드를 열거한다. `Keyboards` 탭의 디바이스 팝업 초기
//! 목록과 시작 시 재조정(startup reconciliation) 둘 다 이 API 를 쓴다.
//!
//! ⭐ **매칭 필터는 `hotplug.rs::build_keyboard_matching_dict()` 와 완전히 같다**
//! (`IOHIDDevice` + `DeviceUsagePage=1`/`DeviceUsage=6`) — 그 함수를 그대로
//! 재사용한다(§3.2 "기존 자산 재사용 판정"). 이터레이터 순회 골격도
//! `hotplug.rs::drain_iterator()` 를 재사용한다.

use crate::hid_mapping::DeviceInfo;

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::DeviceInfo;
    use crate::ffi::{
        self, IoObjectT, K_IOHID_BUILT_IN_KEY, K_IOHID_PRODUCT_ID_KEY, K_IOHID_PRODUCT_KEY,
        K_IOHID_TRANSPORT_KEY, K_IOHID_VENDOR_ID_KEY,
    };
    use crate::hotplug::{build_keyboard_matching_dict, drain_iterator};
    use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType};
    use std::collections::HashSet;
    use std::ptr::NonNull;

    /// 현재 붙어 있는 키보드(usage 1/6)를 열거한다.
    ///
    /// ⚠️ **S-2 대응(스파이크 §3).** 물리 디바이스 1개가 IOHID 서비스 여러 개로
    /// 나타난다(F108Pro 는 실측 3개) — 그래서 `(vendor_id, product_id)` 로 중복을
    /// 제거해 반환한다. UI 에 같은 키보드가 여러 번 보이면 안 된다.
    pub fn list_attached_keyboards() -> Vec<DeviceInfo> {
        let Some(matching) = build_keyboard_matching_dict() else {
            return Vec::new();
        };

        let mut iterator: IoObjectT = 0;
        // SAFETY: `matching` 은 `IOServiceGetMatchingServices` 가 소비한다
        // (CF_RELEASES_ARGUMENT) — `CFRetained::into_raw` 로 소유권을 그대로
        // 넘긴다. `kIOMainPortDefault` 는 IOKit 이 항상 정의하는 정적 심볼이다.
        let result = unsafe {
            ffi::IOServiceGetMatchingServices(
                ffi::kIOMainPortDefault,
                CFRetained::into_raw(matching).as_ptr() as *const CFDictionary,
                &mut iterator,
            )
        };
        if result != 0 {
            return Vec::new();
        }

        let mut devices = Vec::new();
        let mut seen = HashSet::new();
        drain_iterator(iterator, |entry| {
            let Some(info) = read_device_properties(entry) else {
                return;
            };
            if seen.insert((info.vendor_id, info.product_id)) {
                devices.push(info);
            }
        });
        // SAFETY: `iterator` 는 `IOServiceGetMatchingServices` 가 성공 시 채운
        // 유효한 io_iterator_t 다 — `drain_iterator` 는 순회한 항목만 해제하고
        // 이터레이터 핸들 자체는 해제하지 않으므로 여기서 우리가 해제한다.
        unsafe { ffi::IOObjectRelease(iterator) };

        devices
    }

    /// `entry` 에서 `VendorID`/`ProductID`/`Product`/`Transport`/`Built-In` 을 읽어
    /// [`DeviceInfo`] 를 만든다.
    ///
    /// ⛔ **VID/PID 를 읽지 못하면 `None`** — 식별자가 없는 항목은 쓸 수 없으므로
    /// 버린다. 나머지 필드(제품명·전송 방식·내장 여부)는 읽지 못해도 `None` 으로
    /// 두고 항목 자체는 살린다.
    ///
    /// 열거(`list_attached_keyboards`)와 핫플러그 콜백(`hotplug.rs`) 양쪽이 이
    /// 함수를 공유한다.
    pub(crate) fn read_device_properties(entry: IoObjectT) -> Option<DeviceInfo> {
        let vendor_id = read_u32_property(entry, K_IOHID_VENDOR_ID_KEY)?;
        let product_id = read_u32_property(entry, K_IOHID_PRODUCT_ID_KEY)?;
        let product_name = read_string_property(entry, K_IOHID_PRODUCT_KEY);
        let transport = read_string_property(entry, K_IOHID_TRANSPORT_KEY);
        let built_in = read_bool_property(entry, K_IOHID_BUILT_IN_KEY);
        Some(DeviceInfo {
            vendor_id,
            product_id,
            product_name,
            transport,
            built_in,
        })
    }

    /// `IORegistryEntryCreateCFProperty` 로 `entry` 의 `key` 프로퍼티를 읽는다.
    /// 실패(키 없음·엔트리 무효 등)하면 `None`.
    fn create_cf_property(entry: IoObjectT, key: &str) -> Option<CFRetained<CFType>> {
        let cf_key = CFString::from_str(key);
        // SAFETY: `entry` 는 호출자가 보증하는 유효한 io_registry_entry_t 이고,
        // `cf_key` 는 이 호출 동안 유효한 CFString 이다. `allocator` 는 기본
        // 할당자를 뜻하는 NULL 을 넘긴다(`CFAllocatorRef` 는 NULL 이면 기본
        // 할당자로 해석된다 — `CFBase.h` 통상 규약). `options` 는 이 함수에
        // 정의된 플래그가 없으므로(ffi.rs 주석 참고) 0을 쓴다.
        let raw = unsafe {
            ffi::IORegistryEntryCreateCFProperty(
                entry,
                &*cf_key as *const CFString,
                std::ptr::null(),
                0,
            )
        };
        let ptr = NonNull::new(raw as *mut CFType)?;
        // SAFETY: `IORegistryEntryCreateCFProperty` 는 CF_RETURNS_RETAINED
        // 규칙을 따른다(헤더: "The caller should release with CFRelease") —
        // 이 포인터의 소유권은 이미 우리에게 있다.
        Some(unsafe { CFRetained::from_raw(ptr) })
    }

    fn read_u32_property(entry: IoObjectT, key: &str) -> Option<u32> {
        let value = create_cf_property(entry, key)?;
        let number = value.downcast::<CFNumber>().ok()?;
        u32::try_from(number.as_i64()?).ok()
    }

    fn read_string_property(entry: IoObjectT, key: &str) -> Option<String> {
        let value = create_cf_property(entry, key)?;
        let string = value.downcast::<CFString>().ok()?;
        Some(string.to_string())
    }

    /// `Built-In` 은 헤더 상 "Number property"(0/1)로 문서화되어 있지만, 이
    /// 프로젝트가 실측하지 않아 정확한 표현은 `(미확정)`이다(§3.2). `CFNumber`
    /// 를 먼저 시도하고, 아니면 `CFBoolean` 도 폴백으로 받아들인다.
    fn read_bool_property(entry: IoObjectT, key: &str) -> Option<bool> {
        let value = create_cf_property(entry, key)?;
        match value.downcast::<CFNumber>() {
            Ok(number) => number.as_i64().map(|v| v != 0),
            Err(value) => value.downcast::<CFBoolean>().ok().map(|b| b.value()),
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::list_attached_keyboards;
#[cfg(target_os = "macos")]
pub(crate) use macos_impl::read_device_properties;

#[cfg(not(target_os = "macos"))]
pub fn list_attached_keyboards() -> Vec<DeviceInfo> {
    Vec::new()
}
