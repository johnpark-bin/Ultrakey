//! macOS 전용 수기(手記) FFI 선언 모음.
//!
//! `objc2-*` 계열 자동 생성 바인딩이 커버하지 못하는 함수만 여기 모은다.
//! ⛔ **시그니처를 추측해서 선언하지 않는다.** 아래 선언은 전부
//! `/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk` 아래의 공개 헤더를
//! 직접 `grep` 해서 확인한 시그니처다 (근거는 각 블록 주석에 헤더 경로로 남긴다).
//!
//! 이 모듈은 크레이트 내부에서만 쓴다 (`pub(crate)`).

#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use objc2_core_foundation::{
    CFAllocator, CFDictionary, CFMutableDictionary, CFRunLoopSource, CFString,
};
use std::ffi::{c_char, c_void};

// ============================================================================
// Mach 기본 타입
// ============================================================================
//
// 근거: `/usr/include/mach/i386/vm_types.h`, `/usr/include/i386/_types.h`
// `mach_port_t` = `natural_t` = `__darwin_natural_t` = `unsigned int` (Darwin 은
// 32/64비트 아키텍처 어디서나 mach port 를 32비트 값으로 표현한다).
pub(crate) type MachPortT = u32;

extern "C" {
    // libSystem 이 항상 링크되어 있으므로 별도 #[link] 가 필요 없다.
    // 근거: `/usr/include/mach/mach_init.h` — `#define mach_task_self() mach_task_self_`
    pub(crate) static mach_task_self_: MachPortT;
}

// ============================================================================
// IOKit — 공통 타입과 IOKitLib.h 함수
// ============================================================================
//
// 근거: `IOKit.framework/Versions/A/Headers/IOTypes.h`
// `io_object_t`/`io_connect_t`/`io_service_t`/`io_iterator_t` 는 전부
// `typedef mach_port_t io_object_t;` 의 별칭이다.
pub(crate) type IoObjectT = MachPortT;
pub(crate) type IoConnectT = IoObjectT;
pub(crate) type IoServiceT = IoObjectT;
pub(crate) type IoIteratorT = IoObjectT;
pub(crate) type KernReturnT = i32;

/// `IONotificationPortRef` — `typedef struct __IONotificationPort * IONotificationPortRef;`
/// (근거: `IOKitLib.h`). 불투명 포인터라 빈 enum 으로 표현한다.
pub(crate) enum OpaqueIONotificationPort {}
pub(crate) type IONotificationPortRef = *mut OpaqueIONotificationPort;

/// `IOServiceMatchingCallback` — 근거: `IOKitLib.h`
/// `typedef void (*IOServiceMatchingCallback)(void *refcon, io_iterator_t iterator);`
pub(crate) type IOServiceMatchingCallback =
    Option<unsafe extern "C" fn(refcon: *mut c_void, iterator: IoIteratorT)>;

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    /// 근거: `IOKitLib.h` — `extern const mach_port_t kIOMainPortDefault` (macOS 12.0+).
    /// 최소 지원 버전이 macOS 12.0 이므로(제품 결정 D1) deprecated 인
    /// `kIOMasterPortDefault` 대신 이것을 쓴다.
    pub(crate) static kIOMainPortDefault: MachPortT;

    /// 근거: `IOKitLib.h` — `CFMutableDictionaryRef IOServiceMatching(const char *name) CF_RETURNS_RETAINED;`
    pub(crate) fn IOServiceMatching(name: *const c_char) -> *mut CFMutableDictionary;

    /// 근거: `IOKitLib.h` — `io_service_t IOServiceGetMatchingService(mach_port_t mainPort, CFDictionaryRef matching CF_RELEASES_ARGUMENT);`
    /// `matching` 은 이 호출이 참조 하나를 소비한다(CF_RELEASES_ARGUMENT).
    pub(crate) fn IOServiceGetMatchingService(
        main_port: MachPortT,
        matching: *const CFDictionary,
    ) -> IoServiceT;

    /// 근거: `IOKitLib.h`
    pub(crate) fn IOServiceOpen(
        service: IoServiceT,
        owning_task: MachPortT,
        connect_type: u32,
        connect: *mut IoConnectT,
    ) -> KernReturnT;

    /// 근거: `IOKitLib.h`
    pub(crate) fn IOServiceClose(connect: IoConnectT) -> KernReturnT;

    /// 근거: `IOKitLib.h`
    pub(crate) fn IOObjectRelease(object: IoObjectT) -> KernReturnT;

    /// 근거: `IOKitLib.h`
    pub(crate) fn IOIteratorNext(iterator: IoIteratorT) -> IoObjectT;

    /// 근거: `IOKitLib.h` — `IONotificationPortRef IONotificationPortCreate(mach_port_t mainPort);`
    pub(crate) fn IONotificationPortCreate(main_port: MachPortT) -> IONotificationPortRef;

    /// 근거: `IOKitLib.h`
    pub(crate) fn IONotificationPortDestroy(notify: IONotificationPortRef);

    /// 근거: `IOKitLib.h` — 반환된 소스는 "caller should not release" (Get 규칙).
    pub(crate) fn IONotificationPortGetRunLoopSource(
        notify: IONotificationPortRef,
    ) -> *mut CFRunLoopSource;

    /// 근거: `IOKitLib.h` —
    /// `kern_return_t IOServiceGetMatchingServices(mach_port_t mainPort, CFDictionaryRef matching CF_RELEASES_ARGUMENT, io_iterator_t *existing);`
    /// `matching` 은 이 호출이 참조 하나를 소비한다(CF_RELEASES_ARGUMENT). `existing` 에
    /// 담기는 이터레이터는 호출자가 `IOObjectRelease` 로 해제해야 한다(문서:
    /// "should be released by the caller when the iteration is finished").
    pub(crate) fn IOServiceGetMatchingServices(
        main_port: MachPortT,
        matching: *const CFDictionary,
        existing: *mut IoIteratorT,
    ) -> KernReturnT;

    /// 근거: `IOKitLib.h` —
    /// `CFTypeRef IORegistryEntryCreateCFProperty(io_registry_entry_t entry, CFStringRef key, CFAllocatorRef allocator, IOOptionBits options);`
    /// `io_registry_entry_t` 는 `io_object_t` 의 별칭이다(`IOTypes.h`, 위 `IoObjectT` 참고).
    /// 반환값은 CF_RETURNS_RETAINED 규칙이다(문서: "The caller should release with
    /// CFRelease") — 우리가 소유권을 받는다.
    /// ⚠️ **`options` 는 이 함수에 한해 아무 플래그도 정의되어 있지 않다**(문서: "No
    /// options are currently defined") — `kIORegistryIterateRecursively`/
    /// `kIORegistryIterateParents` 는 이 함수가 아니라 `IORegistryEntrySearchCFProperty`
    /// (별도 함수, 여기서 선언하지 않음)의 옵션이다. 그래서 이 크레이트는 항상 `0` 을
    /// 넘긴다 — 상속 검색이 필요하면 `IORegistryEntrySearchCFProperty` 를 별도로
    /// 선언해야 하고, 지금은 직속 프로퍼티 조회만으로 충분하다(호출부 `hid_device.rs`
    /// 참고).
    pub(crate) fn IORegistryEntryCreateCFProperty(
        entry: IoObjectT,
        key: *const CFString,
        allocator: *const CFAllocator,
        options: u32,
    ) -> *mut c_void;

    /// 근거: `IOKitLib.h` — `kern_return_t IORegistryEntryGetRegistryEntryID(
    /// io_registry_entry_t entry, uint64_t *entryID );`
    /// `io_registry_entry_t` 는 `io_object_t` 의 별칭.
    /// 서비스의 레지스트리 ID — `hidutil list` 의 `RegistryID` 컬럼(스파이크 §10).
    /// ⚠️ 레지스트리 **프로퍼티**("RegistryID")가 아니라 이 함수의 반환값으로 얻는다
    /// — `IORegistryEntryCreateCFProperty` 로는 읽을 수 없다(실측, 이슈 #86). reboot
    /// 이전까지 전 태스크 공통의 개별 식별자다(문서: "global to all tasks").
    pub(crate) fn IORegistryEntryGetRegistryEntryID(
        entry: IoObjectT,
        entry_id: *mut u64,
    ) -> KernReturnT;

    /// 근거: `IOKitLib.h`. `matching` 은 참조 하나를 소비한다(CF_RELEASES_ARGUMENT).
    pub(crate) fn IOServiceAddMatchingNotification(
        notify_port: IONotificationPortRef,
        notification_type: *const c_char,
        matching: *mut CFMutableDictionary,
        callback: IOServiceMatchingCallback,
        ref_con: *mut c_void,
        notification: *mut IoIteratorT,
    ) -> KernReturnT;

    /// 근거: `IOKit.framework/.../hidsystem/IOHIDLib.h`
    /// `kern_return_t IOHIDGetModifierLockState(io_connect_t handle, int selector, bool *state);`
    pub(crate) fn IOHIDGetModifierLockState(
        handle: IoConnectT,
        selector: i32,
        state: *mut bool,
    ) -> KernReturnT;

    /// 근거: 위와 동일 헤더
    /// `kern_return_t IOHIDSetModifierLockState(io_connect_t handle, int selector, bool state);`
    pub(crate) fn IOHIDSetModifierLockState(
        handle: IoConnectT,
        selector: i32,
        state: bool,
    ) -> KernReturnT;
}

/// 근거: `IOKit.framework/.../hidsystem/IOHIDShared.h` — `kIOHIDParamConnectType = 1`.
pub(crate) const K_IOHID_PARAM_CONNECT_TYPE: u32 = 1;
/// 근거: `IOKit.framework/.../hidsystem/IOHIDShared.h` — `#define kIOHIDSystemClass "IOHIDSystem"`
pub(crate) const K_IOHID_SYSTEM_CLASS: &[u8] = b"IOHIDSystem\0";
/// 근거: `IOKit.framework/.../hidsystem/IOHIDParameter.h` — `kIOHIDCapsLockState = 0x00000001`.
pub(crate) const K_IOHID_CAPS_LOCK_STATE: i32 = 0x0000_0001;

/// 근거: `IOKit.framework/.../IOKitKeys.h`
pub(crate) const K_IO_MATCHED_NOTIFICATION: &[u8] = b"IOServiceMatched\0";
pub(crate) const K_IO_TERMINATED_NOTIFICATION: &[u8] = b"IOServiceTerminate\0";
/// 근거: `IOKit.framework/.../hid/IOHIDKeys.h` / `IOHIDDeviceKeys.h`
pub(crate) const K_IOHID_DEVICE_KEY: &[u8] = b"IOHIDDevice\0";
/// 근거: `IOKit.framework/.../hid/IOHIDDeviceKeys.h` — `#define kIOHIDDeviceUsagePageKey "DeviceUsagePage"`.
/// ⚠️ **이 필터 키는 매칭 사전에서 쓰지 않는다**(이슈 #86 정렬로
/// `PrimaryUsagePage`/`PrimaryUsage` 를 쓴다, 명세 §3.2) — 디바이스가 선언한
/// **모든** 응용 컬렉션(behaviors, `DeviceUsagePairs`) 중 하나에 걸리는 키라
/// `hidutil list` 가 쓰는 `PrimaryUsage*` 와 다르게 매칭될 수 있다. 진단
/// probe(`keyboard_list_probe.rs`, 이슈 #86)가 두 필터의 집합 차이를 확인할 때만
/// 참조한다.
pub(crate) const K_IOHID_DEVICE_USAGE_PAGE_KEY: &str = "DeviceUsagePage";
/// 근거: 위와 동일 헤더 — `#define kIOHIDDeviceUsageKey "DeviceUsage"`.
pub(crate) const K_IOHID_DEVICE_USAGE_KEY: &str = "DeviceUsage";
/// 근거: `IOKit.framework/.../hid/IOHIDDeviceKeys.h` — `#define kIOHIDPrimaryUsagePageKey "PrimaryUsagePage"`.
/// ⭐ **`build_keyboard_matching_dict()` 가 쓰는 매칭 키**(이슈 #86 — 명세 §3.2 의
/// 정본 `hidutil list --matching '{"PrimaryUsagePage":1,"PrimaryUsage":6}'` 와 같은
/// 필터). `DeviceUsagePage`/`DeviceUsage` 는 같은 매칭 숫자(1/6)라도 **다른 IOKit
/// 프로퍼티**다 — 이슈 #86 은 그 불일치가 내장 키보드 누락의 원인 후보 (a) 다.
pub(crate) const K_IOHID_PRIMARY_USAGE_PAGE_KEY: &str = "PrimaryUsagePage";
/// 근거: 위와 동일 헤더 — `#define kIOHIDPrimaryUsageKey "PrimaryUsage"`.
pub(crate) const K_IOHID_PRIMARY_USAGE_KEY: &str = "PrimaryUsage";
/// USB HID Usage Tables — Generic Desktop 페이지(1) / Keyboard 사용처(6).
pub(crate) const K_HID_USAGE_PAGE_GENERIC_DESKTOP: i32 = 0x01;
pub(crate) const K_HID_USAGE_GENERIC_DESKTOP_KEYBOARD: i32 = 0x06;

/// 근거: `IOKit.framework/.../hid/IOHIDDeviceKeys.h` — `#define kIOHIDVendorIDKey "VendorID"`.
/// F-17(`per-device-settings.md` §3.2) 디바이스 열거·핫플러그가 읽는 프로퍼티 키다.
pub(crate) const K_IOHID_VENDOR_ID_KEY: &str = "VendorID";
/// 근거: 위와 동일 헤더 — `#define kIOHIDProductIDKey "ProductID"`.
pub(crate) const K_IOHID_PRODUCT_ID_KEY: &str = "ProductID";
/// 근거: 위와 동일 헤더 — `#define kIOHIDProductKey "Product"`.
pub(crate) const K_IOHID_PRODUCT_KEY: &str = "Product";
/// 근거: 위와 동일 헤더 — `#define kIOHIDTransportKey "Transport"`.
pub(crate) const K_IOHID_TRANSPORT_KEY: &str = "Transport";
/// 근거: 위와 동일 헤더 — `#define kIOHIDBuiltInKey "Built-In"`. ⚠️ 하이픈 포함, 원문 그대로.
pub(crate) const K_IOHID_BUILT_IN_KEY: &str = "Built-In";

// ============================================================================
// CoreFoundation — 범용 CFRetain/CFRelease
// ============================================================================
//
// `objc2-core-foundation` 은 임의 CF 타입에 대한 범용 CFRelease/CFRetain 을
// 공개 심볼로 노출하지 않으므로(타입별 `CFRetained` 래퍼로 대체), TIS* 계열처럼
// 이 크레이트가 직접 만든 불투명 타입에는 이 원시 선언을 쓴다.
#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    pub(crate) fn CFRelease(cf: *const c_void);

    /// 근거: `CFPreferences.h` — `CFPropertyListRef CFPreferencesCopyValue(
    /// CFStringRef key, CFStringRef applicationID, CFStringRef userName, CFStringRef hostName);`
    /// "Copy" 규칙(CF_RETURNS_RETAINED) — 문서: "Caller must release the returned value".
    ///
    /// ⭐ **`CFPreferencesCopyAppValue` 가 아니라 이것을 쓴다.** 같은 헤더가
    /// "the App functions ... should never be called with kCFPreferencesAnyApplication"
    /// 이라고 명시적으로 권고하기 때문이다 — `NSGlobalDomain` 을 읽는 정식 경로는
    /// 도메인 3요소(application/user/host)를 모두 받는 이 함수다.
    ///
    /// ⭐ **네 조합을 이 기기에서 직접 실측해 고른 것이다**(2026-08-30, 이슈 #28).
    /// `com.apple.keyboard.fnState` 로 시험한 결과:
    /// ```text
    /// CFPreferencesCopyAppValue(key, kCFPreferencesAnyApplication)                     → CFBoolean 1
    /// CFPreferencesCopyAppValue(key, kCFPreferencesCurrentApplication)                 → CFBoolean 1
    /// CFPreferencesCopyValue(key, AnyApplication, CurrentUser, AnyHost)                → CFBoolean 1  ← 채택
    /// CFPreferencesCopyValue(key, AnyApplication, CurrentUser, CurrentHost)            → NULL
    /// ```
    /// 즉 헤더가 권고하지 않는 조합도 이 기기에서는 값을 주지만, **권고를 따르면서도
    /// 같은 값을 주는 조합이 존재하므로** 그쪽을 택한다. `CurrentHost` 는 값을 주지
    /// 않으므로 반드시 `AnyHost` 여야 한다(이것도 실측이다).
    /// ⚠️ 반환 타입이 `CFNumber` 가 아니라 **`CFBoolean`** 이다(`defaults` 는 `1` 로
    /// 출력하지만 실제 저장 타입은 불리언이다) — `fn_state.rs` 가 둘 다 받는다.
    pub(crate) fn CFPreferencesCopyValue(
        key: *const CFString,
        application_id: *const CFString,
        user_name: *const CFString,
        host_name: *const CFString,
    ) -> *mut c_void;

    /// 근거: `CFPreferences.h` — `CF_EXPORT const CFStringRef kCFPreferencesAnyApplication;`.
    /// `CFPreferencesCopyValue` 의 `applicationID` 자리에 쓰면 `NSGlobalDomain` 을
    /// 가리킨다. ⚠️ 헤더가 쓰지 말라고 한 것은 **App 계열 함수**(`…CopyAppValue`)와의
    /// 조합이지 이 함수와의 조합이 아니다.
    pub(crate) static kCFPreferencesAnyApplication: Option<&'static CFString>;

    /// 근거: `CFPreferences.h` — `CF_EXPORT const CFStringRef kCFPreferencesCurrentUser;`
    pub(crate) static kCFPreferencesCurrentUser: Option<&'static CFString>;

    /// 근거: `CFPreferences.h` — `CF_EXPORT const CFStringRef kCFPreferencesAnyHost;`
    /// ⚠️ `kCFPreferencesCurrentHost` 를 쓰면 `com.apple.keyboard.fnState` 는 `NULL` 이
    /// 온다(위 실측 표) — 이 값은 host 중립으로 저장된다.
    pub(crate) static kCFPreferencesAnyHost: Option<&'static CFString>;
}

// ============================================================================
// ApplicationServices(HIServices) — Accessibility
// ============================================================================
//
// 근거: `ApplicationServices.framework/.../HIServices.framework/.../AXUIElement.h`
// `extern Boolean AXIsProcessTrusted(void);`
// ⚠️ `AXIsProcessTrustedWithOptions` 는 의도적으로 선언하지 않는다 — accessibility.rs 주석 참조.
#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    pub(crate) fn AXIsProcessTrusted() -> u8;
}

// ============================================================================
// Carbon — Secure Input, Text Input Sources
// ============================================================================
//
// 근거: `Carbon.framework/.../HIToolbox.framework/.../CarbonEventsCore.h`
// `extern Boolean IsSecureEventInputEnabled(void);`
#[link(name = "Carbon", kind = "framework")]
extern "C" {
    pub(crate) fn IsSecureEventInputEnabled() -> u8;

    /// 근거: `Events.h` — `extern UInt8 LMGetKbdType(void);`
    pub(crate) fn LMGetKbdType() -> u8;

    /// 근거: `TextInputSources.h`
    pub(crate) fn TISCopyCurrentKeyboardInputSource() -> TISInputSourceRef;
    /// 근거: `TextInputSources.h` — ⭐ CJK IME 등 비-ASCII 입력기 폴백용(§3-e).
    pub(crate) fn TISCopyCurrentASCIICapableKeyboardLayoutInputSource() -> TISInputSourceRef;
    /// 근거: `TextInputSources.h` — "Get" 규칙, 반환값을 release 하면 안 된다.
    pub(crate) fn TISGetInputSourceProperty(
        input_source: TISInputSourceRef,
        property_key: Option<&CFString>,
    ) -> *mut c_void;

    /// 근거: `TextInputSources.h` — 프로퍼티 키 상수(전부 데이터 심볼).
    pub(crate) static kTISPropertyInputSourceID: Option<&'static CFString>;
    pub(crate) static kTISPropertyInputSourceIsASCIICapable: Option<&'static CFString>;
    pub(crate) static kTISPropertyUnicodeKeyLayoutData: Option<&'static CFString>;
    /// 근거: `TextInputSources.h` — 값은 `CFArrayRef`(원소 `CFString`, BCP-47 언어 태그).
    /// F-16(korean-input.md §3.3)이 "폴백 교체 이전 원본 소스가 한국어 입력기인가"를
    /// 판정하기 위해 새로 요구한 프로퍼티다(F-14 갱신 사항).
    pub(crate) static kTISPropertyInputSourceLanguages: Option<&'static CFString>;
    /// 근거: `TextInputSources.h` — Distributed Notification 이름.
    pub(crate) static kTISNotifySelectedKeyboardInputSourceChanged: Option<&'static CFString>;
}

/// `TISInputSourceRef` — `typedef struct __TISInputSource* TISInputSourceRef;`
/// (근거: `TextInputSources.h`). CF 규약을 따르는 불투명 CF 객체이므로
/// `CFRelease` 로 해제한다(위 CFRelease 참조).
#[repr(C)]
pub(crate) struct OpaqueTISInputSource {
    _priv: [u8; 0],
}
pub(crate) type TISInputSourceRef = *mut OpaqueTISInputSource;

// ============================================================================
// CoreServices(CarbonCore) — UCKeyTranslate
// ============================================================================
//
// 근거: `CoreServices.framework/.../CarbonCore.framework/.../UnicodeUtilities.h`
// `UCKeyTranslate` 는 Carbon.framework 가 아니라 CoreServices/CarbonCore 가 심볼을 갖는다
// (`nm` 대조: Carbon.tbd 에는 없고 CoreServices.tbd 에 있다).
#[link(name = "CoreServices", kind = "framework")]
extern "C" {
    pub(crate) fn UCKeyTranslate(
        key_layout_ptr: *const OpaqueUCKeyboardLayout,
        virtual_key_code: u16,
        key_action: u16,
        modifier_key_state: u32,
        keyboard_type: u32,
        key_translate_options: u32,
        dead_key_state: *mut u32,
        max_string_length: usize,
        actual_string_length: *mut usize,
        unicode_string: *mut u16,
    ) -> i32;
}

/// `UCKeyboardLayout` — `uchr` 리소스 바이트를 그대로 가리키는 포인터로만 쓰인다.
/// 필드 레이아웃을 우리가 해석할 필요가 없으므로(전부 `UCKeyTranslate` 내부가 해석)
/// 불투명 타입으로만 선언한다.
pub(crate) enum OpaqueUCKeyboardLayout {}

/// 근거: `UnicodeUtilities.h` — `kUCKeyActionDown = 0`, `kUCKeyActionUp = 1`.
pub(crate) const K_UC_KEY_ACTION_DOWN: u16 = 0;
/// 근거: `UnicodeUtilities.h` — `kUCKeyTranslateNoDeadKeysMask = 1 << 0`.
pub(crate) const K_UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK: u32 = 1;

// ============================================================================
// Security.framework — Keychain (F-12, `licensing-and-trial.md` §3.5)
// ============================================================================
//
// 근거: `Security.framework/Versions/A/Headers/SecItem.h`
//   OSStatus SecItemCopyMatching(CFDictionaryRef query, CFTypeRef * __nullable
//       CF_RETURNS_RETAINED result);
//   OSStatus SecItemAdd(CFDictionaryRef attributes, CFTypeRef * __nullable
//       CF_RETURNS_RETAINED result);
//   OSStatus SecItemUpdate(CFDictionaryRef query, CFDictionaryRef
//       attributesToUpdate);
//   OSStatus SecItemDelete(CFDictionaryRef query);
//   OSStatus == int32_t (SecBase.h: `typedef int32_t OSStatus;`).
//
// 트라이얼 기산점·라이선스 캐시는 Keychain 의 **generic password** 로 저장한다
// (§3.5). kSecAttrSynchronizable 를 명시적으로 꺼 iCloud 동기화로 기산점이 여러
// 기기로 퍼지지 않게 한다(§3.5 "(3) kSecAttrSynchronizable 을 명시적으로 끈다").
//
// ⚠️ dict 상수를 담으려면 CFMutableDictionary 를 만들어 넘긴다 — CF·CFMutable 간
// toll-free bridged 라 `*const CFDictionary` 로 선언한다.
#[link(name = "Security", kind = "framework")]
extern "C" {
    pub(crate) fn SecItemCopyMatching(
        query: *const CFDictionary,
        result: *mut *mut c_void,
    ) -> i32;

    pub(crate) fn SecItemAdd(
        attributes: *const CFDictionary,
        result: *mut *mut c_void,
    ) -> i32;

    pub(crate) fn SecItemUpdate(
        query: *const CFDictionary,
        attributes_to_update: *const CFDictionary,
    ) -> i32;

    pub(crate) fn SecItemDelete(query: *const CFDictionary) -> i32;

    // Security.framework — Keychain 속성 상수(데이터 심볼). 근거: `SecItem.h`.
    // 각 상수는 데이터 심볼로 노출되어 `Option<&'static CFString>` 로 접근한다.
    pub(crate) static kSecClass: Option<&'static CFString>;
    pub(crate) static kSecClassGenericPassword: Option<&'static CFString>;
    pub(crate) static kSecAttrService: Option<&'static CFString>;
    pub(crate) static kSecAttrAccount: Option<&'static CFString>;
    pub(crate) static kSecAttrAccessible: Option<&'static CFString>;
    pub(crate) static kSecAttrAccessibleAfterFirstUnlock: Option<&'static CFString>;
    pub(crate) static kSecAttrSynchronizable: Option<&'static CFString>;
    pub(crate) static kSecValueData: Option<&'static CFString>;
    pub(crate) static kSecReturnData: Option<&'static CFString>;
}

// ============================================================================
// CoreGraphics — F-19(D-4) 키보드 타입 판정
// ============================================================================
//
// 근거: `CoreGraphics.framework/.../CGEventSource.h` 65행 —
// `CG_EXTERN CGEventSourceKeyboardType CGEventSourceGetKeyboardType(
//     CGEventSourceRef __nullable source) API_AVAILABLE(macos(10.4));`
// `CGEventSourceKeyboardType` 은 `uint32_t`(같은 헤더 488행 실측).
// ⚠️ 반환 상수(ANSI=40/ISO=41/JIS=42) 는 SDK 헤더에 매크로가 없다 — 값의 근거는
// `keyboard_type.rs` 모듈 문서를 참고(Apple 공개 문서값, `(추정)`).
#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    pub(crate) fn CGEventSourceGetKeyboardType(source: *mut c_void) -> u32;
}
