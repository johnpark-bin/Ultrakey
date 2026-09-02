//! 디바이스 열거 — `hidutil list --matching {...}` 대응, IOKit 직접 경로.
//!
//! `docs/spec/per-device-settings.md` §3.2 · `docs/research/per-device-hid-spike.md`
//! §10 이 요구하는 API 다: 알림 등록 없이 `IOServiceGetMatchingServices` 로 한 번
//! 순회해 현재 붙어 있는 키보드를 열거한다. `Keyboards` 탭의 디바이스 팝업 초기
//! 목록과 시작 시 재조정(startup reconciliation) 둘 다 이 API 를 쓴다.
//!
//! ⭐ **매칭 필터는 `hotplug.rs::build_keyboard_matching_dict()` 와 완전히 같다**
//! (`IOHIDDevice` + `PrimaryUsagePage=1`/`PrimaryUsage=6`) — 그 함수를 그대로
//! 재사용한다(§3.2 "기존 자산 재사용 판정"). 이터레이터 순회 골격도
//! `hotplug.rs::drain_iterator()` 를 재사용한다.

use crate::hid_mapping::DeviceInfo;

/// 매칭된 IOHID **서비스 하나**의 원시 스냅샷(이슈 #86).
///
/// [`DeviceInfo`] 는 VID/PID 를 **못 읽으면 항목 전체를 버린다**(식별자 없는
/// 항목은 per-device 설정의 대상이 될 수 없으므로). 그래서 "내장 키보드가
/// 목록에 없다"는 원인이 "매칭 필터가 안 걸린 것"인지 "VID/PID 를 못 읽어
/// 버려진 것"인지 이 타입으로만 구분할 수 있다 — [`KeyboardFilter`] 별로
/// 이 타입을 받아 두 경로를 따로 본다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawKeyboardService {
    /// `hidutil list` 의 `RegistryID` 컬럼과 같은 IORegistry 엔트리 ID.
    pub registry_id: Option<u64>,
    pub vendor_id: Option<u32>,
    pub product_id: Option<u32>,
    pub product_name: Option<String>,
    pub transport: Option<String>,
    /// `Built-In` — ⚠️ 이 프로퍼티 키·타입은 실기기 검증 전까지 `(미확정)`
    /// 이다(명세 §9 질문 7).
    pub built_in: Option<bool>,
    pub device_usage_page: Option<u32>,
    pub device_usage: Option<u32>,
    pub primary_usage_page: Option<u32>,
    pub primary_usage: Option<u32>,
}

/// IOKit 매칭 사전의 필터 키 두 변형(이슈 #86 원인 후보 (a)).
///
/// - [`KeyboardFilter::DeviceUsage`] — 옛 구현이 쓰던 `DeviceUsagePage`/
///   `DeviceUsage`. 디바이스가 선언한 전체 응용 컬렉션 중 하나에 걸린다.
/// - [`KeyboardFilter::PrimaryUsage`] — ⭐ 명세 §3.2 정본(`hidutil list`)과
///   같은 `PrimaryUsagePage`/`PrimaryUsage` — 현재 프로덕션 매칭 사전
///   (`hotplug.rs::build_keyboard_matching_dict`)이 쓰는 필터다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardFilter {
    DeviceUsage,
    PrimaryUsage,
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::{DeviceInfo, KeyboardFilter, RawKeyboardService};
    use crate::ffi::{
        self, IoObjectT, K_IOHID_BUILT_IN_KEY, K_IOHID_DEVICE_USAGE_KEY,
        K_IOHID_DEVICE_USAGE_PAGE_KEY, K_IOHID_PRIMARY_USAGE_KEY, K_IOHID_PRIMARY_USAGE_PAGE_KEY,
        K_IOHID_PRODUCT_ID_KEY, K_IOHID_PRODUCT_KEY, K_IOHID_TRANSPORT_KEY, K_IOHID_VENDOR_ID_KEY,
    };
    use crate::hotplug::{
        build_keyboard_matching_dict, build_keyboard_matching_dict_with_keys, drain_iterator,
    };
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

    /// `RegistryID` — 레지스트리 ID 는 프로퍼티가 아니라
    /// `IORegistryEntryGetRegistryEntryID()` 반환값으로 얻는다(ffi.rs 문서 참고).
    fn read_registry_id(entry: IoObjectT) -> Option<u64> {
        let mut id: u64 = 0;
        // SAFETY: `entry` 는 호출자가 보증하는 유효한 io_registry_entry_t 이고,
        // `id` 는 이 호출 동안 유효한 u64 출력 버퍼다.
        let result = unsafe { ffi::IORegistryEntryGetRegistryEntryID(entry, &mut id) };
        (result == 0).then_some(id)
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

    impl KeyboardFilter {
        fn keys(self) -> (&'static str, &'static str) {
            match self {
                KeyboardFilter::DeviceUsage => {
                    (K_IOHID_DEVICE_USAGE_PAGE_KEY, K_IOHID_DEVICE_USAGE_KEY)
                }
                KeyboardFilter::PrimaryUsage => {
                    (K_IOHID_PRIMARY_USAGE_PAGE_KEY, K_IOHID_PRIMARY_USAGE_KEY)
                }
            }
        }
    }

    /// 지정한 필터로 매칭된 **모든** IOHID 서비스를 중복 제거 없이 나열한다.
    ///
    /// ⭐ 이 함수는 프로덕션 경로에 쓰지 않는다 — [`list_attached_keyboards`] 의
    /// (VID,PID) 중복 제거를 우회해서 "어느 필터가 어떤 서비스를 건졌는가"를 눈으로
    /// 확인하는 진단 전용이다(`keyboard_list_probe.rs`, 이슈 #86).
    pub fn list_raw_keyboard_services(filter: KeyboardFilter) -> Vec<RawKeyboardService> {
        let (page_key, usage_key) = filter.keys();
        let Some(matching) = build_keyboard_matching_dict_with_keys(page_key, usage_key) else {
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

        let mut services = Vec::new();
        drain_iterator(iterator, |entry| {
            services.push(read_raw_service_properties(entry));
        });
        // SAFETY: `iterator` 는 `IOServiceGetMatchingServices` 가 성공 시 채운
        // 유효한 io_iterator_t 다 — `drain_iterator` 는 순회한 항목만 해제하고
        // 이터레이터 핸들 자체는 해제하지 않으므로 여기서 우리가 해제한다.
        unsafe { ffi::IOObjectRelease(iterator) };

        services
    }

    fn read_raw_service_properties(entry: IoObjectT) -> RawKeyboardService {
        RawKeyboardService {
            registry_id: read_registry_id(entry),
            vendor_id: read_u32_property(entry, K_IOHID_VENDOR_ID_KEY),
            product_id: read_u32_property(entry, K_IOHID_PRODUCT_ID_KEY),
            product_name: read_string_property(entry, K_IOHID_PRODUCT_KEY),
            transport: read_string_property(entry, K_IOHID_TRANSPORT_KEY),
            built_in: read_bool_property(entry, K_IOHID_BUILT_IN_KEY),
            device_usage_page: read_u32_property(entry, K_IOHID_DEVICE_USAGE_PAGE_KEY),
            device_usage: read_u32_property(entry, K_IOHID_DEVICE_USAGE_KEY),
            primary_usage_page: read_u32_property(entry, K_IOHID_PRIMARY_USAGE_PAGE_KEY),
            primary_usage: read_u32_property(entry, K_IOHID_PRIMARY_USAGE_KEY),
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::list_attached_keyboards;
#[cfg(target_os = "macos")]
pub use macos_impl::list_raw_keyboard_services;
#[cfg(target_os = "macos")]
pub(crate) use macos_impl::read_device_properties;

#[cfg(not(target_os = "macos"))]
pub fn list_attached_keyboards() -> Vec<DeviceInfo> {
    Vec::new()
}

#[cfg(not(target_os = "macos"))]
pub fn list_raw_keyboard_services(_filter: KeyboardFilter) -> Vec<RawKeyboardService> {
    Vec::new()
}
