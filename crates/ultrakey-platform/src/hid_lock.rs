//! 경로 C — HID 잠금 상태(caps lock) 직접 조작.
//!
//! `IOServiceOpen` 으로 `IOHIDSystem` 서비스에 연결(`io_connect_t`)을 한 번
//! 얻어 캐시해 재사용한다 — 매번 새로 여는 것은 불필요한 커널 왕복이다.
//! 이 경로는 이벤트 합성이 아니라 caps lock 의 **실제** 잠금 상태(대문자
//! 고정·LED)를 읽고 쓴다(§3-d).

#[cfg(target_os = "macos")]
mod macos_impl {
    use crate::ffi::{
        self, IoConnectT, IoServiceT, K_IOHID_CAPS_LOCK_STATE, K_IOHID_PARAM_CONNECT_TYPE,
        K_IOHID_SYSTEM_CLASS,
    };
    use std::sync::OnceLock;

    /// `IOHIDSystem` 서비스에 대한 `io_connect_t` 캐시.
    ///
    /// SAFETY: `io_connect_t` 는 커널이 발급한 정수 핸들일 뿐이라(포인터가
    /// 아니다) 스레드 사이에 값으로 복사해 공유해도 안전하다. `IOHIDGet/
    /// SetModifierLockState` 자체가 이 핸들을 여러 스레드에서 동시에 쓰는
    /// 상황을 상정한 syscall 래퍼다.
    static CONNECTION: OnceLock<Option<IoConnectT>> = OnceLock::new();

    fn open_connection() -> Option<IoConnectT> {
        // SAFETY: `IOServiceMatching` 은 정적 C 문자열을 받는 순수 함수이고,
        // 반환된 딕셔너리의 소유권은 우리에게 있다 — 아래 `IOServiceGetMatchingService`
        // 가 그 참조 하나를 소비한다(CF_RELEASES_ARGUMENT).
        let matching = unsafe {
            ffi::IOServiceMatching(K_IOHID_SYSTEM_CLASS.as_ptr() as *const std::os::raw::c_char)
        };
        if matching.is_null() {
            return None;
        }

        // SAFETY: `matching` 은 방금 만든 유효한(그리고 이 호출로 소비되는)
        // 딕셔너리다. `kIOMainPortDefault` 는 IOKit 프레임워크가 항상
        // 정의하는 정적 심볼이다.
        let service: IoServiceT = unsafe {
            ffi::IOServiceGetMatchingService(
                ffi::kIOMainPortDefault,
                matching as *const objc2_core_foundation::CFDictionary,
            )
        };
        if service == 0 {
            return None;
        }

        let mut connect: IoConnectT = 0;
        // SAFETY: `service` 는 위에서 얻은 유효한 서비스 핸들이고,
        // `mach_task_self_` 는 libSystem 이 항상 정의하는 현재 태스크 포트다.
        let result = unsafe {
            ffi::IOServiceOpen(
                service,
                ffi::mach_task_self_,
                K_IOHID_PARAM_CONNECT_TYPE,
                &mut connect,
            )
        };
        // SAFETY: `service` 는 더 이상 필요 없다 — `IOServiceOpen` 은 이
        // 핸들의 소유권을 가져가지 않으므로 우리가 직접 해제해야 한다.
        unsafe { ffi::IOObjectRelease(service) };

        if result != 0 {
            return None;
        }
        Some(connect)
    }

    fn connection() -> Option<IoConnectT> {
        *CONNECTION.get_or_init(open_connection)
    }

    /// `IOHIDGetModifierLockState` — caps lock 의 현재 잠금 상태.
    pub fn caps_lock_state() -> Option<bool> {
        let connect = connection()?;
        let mut state = false;
        // SAFETY: `connect` 는 `open_connection` 이 확인한 유효한 연결이다.
        let result = unsafe {
            ffi::IOHIDGetModifierLockState(connect, K_IOHID_CAPS_LOCK_STATE, &mut state)
        };
        if result == 0 {
            Some(state)
        } else {
            None
        }
    }

    /// `IOHIDSetModifierLockState` — caps lock 잠금을 강제로 켜거나 끈다.
    /// 성공 여부를 반환한다.
    pub fn set_caps_lock_state(on: bool) -> bool {
        let Some(connect) = connection() else {
            return false;
        };
        // SAFETY: 위와 동일.
        let result =
            unsafe { ffi::IOHIDSetModifierLockState(connect, K_IOHID_CAPS_LOCK_STATE, on) };
        result == 0
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{caps_lock_state, set_caps_lock_state};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    pub fn caps_lock_state() -> Option<bool> {
        None
    }
    pub fn set_caps_lock_state(_on: bool) -> bool {
        false
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{caps_lock_state, set_caps_lock_state};
