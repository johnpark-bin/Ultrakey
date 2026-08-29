//! macOS 전용 수기(手記) FFI 선언 모음.
//!
//! `objc2-*` 계열 자동 생성 바인딩이 커버하지 못하는 함수만 여기 모은다.
//! ⛔ **시그니처를 추측해서 선언하지 않는다.** 아래 선언은 전부
//! `/Library/Developer/CommandLineTools/SDKs/MacOSX26.5.sdk` 아래의 공개 헤더를
//! 직접 `grep` 해서 확인한 시그니처다 (근거는 각 블록 주석에 헤더 경로로 남긴다).
//!
//! 이 모듈은 크레이트 내부에서만 쓴다 (`pub(crate)`).

#![allow(non_camel_case_types, non_upper_case_globals, dead_code)]

use objc2_core_foundation::{CFDictionary, CFMutableDictionary, CFRunLoopSource, CFString};
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
pub(crate) const K_IOHID_DEVICE_USAGE_PAGE_KEY: &str = "DeviceUsagePage";
pub(crate) const K_IOHID_DEVICE_USAGE_KEY: &str = "DeviceUsage";
/// USB HID Usage Tables — Generic Desktop 페이지(1) / Keyboard 사용처(6).
pub(crate) const K_HID_USAGE_PAGE_GENERIC_DESKTOP: i32 = 0x01;
pub(crate) const K_HID_USAGE_GENERIC_DESKTOP_KEYBOARD: i32 = 0x06;

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
