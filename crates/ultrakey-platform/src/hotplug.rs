//! 키보드 핫플러그 감지.
//!
//! ⭐ **`IOServiceAddMatchingNotification`(`kIOMatchedNotification` /
//! `kIOTerminatedNotification`)을 쓴다.** 명세(`key-remapping-engine.md` §6)
//! 는 `IOHIDManagerRegisterDeviceMatchingCallback` 계열을 나열하지만, 그
//! 경로는 `IOHIDManagerOpen` 이 필요하고 그것이 곧 Input Monitoring(TCC)
//! 권한 요구다. 근거는 `docs/dev/architecture.md` §3 "결정 2" — SuperKey
//! 원본은 Input Monitoring 을 명시적으로 확인하지 않고 `IOHIDManagerOpen`
//! 실패를 재시도로 흡수하는데, 핫플러그 감지 하나 때문에 권한을 하나 더
//! 요구하는 구조를 만들 이유가 없다. `IOServiceAddMatchingNotification` 은
//! TCC 권한 없이 같은 정보(키보드 HID 장치의 등장·소멸)를 준다.
//!
//! ⚠️ **함정**: `IOServiceAddMatchingNotification` 은 첫 등록 시 **기존
//! 장치 전부에 대해 즉시 콜백이 온다.** 이 최초 호출에서 이터레이터를 끝까지
//! 비우지 않으면 알림이 "무장(arm)"되지 않아 이후 진짜 연결/해제가 와도
//! 통지되지 않는다. 이 크레이트는 최초 등록 직후 한 번 조용히(콜백을 부르지
//! 않고) 비워서 무장만 하고, 그 이후에 도착하는 이터레이터만 실제
//! [`HotplugEvent`] 로 사용자 콜백에 전달한다.

/// 키보드 장치의 연결/해제.
pub enum HotplugEvent {
    Attached,
    Detached,
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::HotplugEvent;
    use crate::ffi::{
        self, IONotificationPortRef, IoIteratorT, K_HID_USAGE_GENERIC_DESKTOP_KEYBOARD,
        K_HID_USAGE_PAGE_GENERIC_DESKTOP, K_IOHID_DEVICE_KEY, K_IOHID_DEVICE_USAGE_KEY,
        K_IOHID_DEVICE_USAGE_PAGE_KEY, K_IO_MATCHED_NOTIFICATION, K_IO_TERMINATED_NOTIFICATION,
    };
    use core::ffi::c_void;
    use core::ptr::NonNull;
    use objc2_core_foundation::{
        kCFRunLoopCommonModes, CFMutableDictionary, CFNumber, CFRetained, CFRunLoop,
        CFString,
    };
    use std::sync::Arc;

    struct HotplugContext {
        callback: Box<dyn Fn(HotplugEvent) + Send + Sync>,
    }

    /// 매칭/종료 알림 각각에 넘길 refcon — 어느 이벤트로 통지할지를 함께 싣는다.
    struct CallbackWithKind {
        inner: Arc<HotplugContext>,
        kind: HotplugEventKind,
    }

    #[derive(Clone, Copy)]
    enum HotplugEventKind {
        Attached,
        Detached,
    }

    fn build_keyboard_matching_dict() -> Option<CFRetained<CFMutableDictionary>> {
        // SAFETY: 정적 C 문자열을 넘기는 순수 함수 호출이다.
        let dict_ptr = unsafe {
            ffi::IOServiceMatching(K_IOHID_DEVICE_KEY.as_ptr() as *const std::os::raw::c_char)
        };
        let dict = NonNull::new(dict_ptr)?;
        // SAFETY: `IOServiceMatching` 은 CF_RETURNS_RETAINED — 이 포인터의
        // 소유권은 이미 우리에게 있다.
        let dict = unsafe { CFRetained::from_raw(dict) };

        let usage_page_key = CFString::from_str(K_IOHID_DEVICE_USAGE_PAGE_KEY);
        let usage_key = CFString::from_str(K_IOHID_DEVICE_USAGE_KEY);
        let usage_page_value = CFNumber::new_i32(K_HID_USAGE_PAGE_GENERIC_DESKTOP);
        let usage_value = CFNumber::new_i32(K_HID_USAGE_GENERIC_DESKTOP_KEYBOARD);

        // SAFETY: 네 포인터 모두 위에서 만든, 이 호출 동안 유효한 CF 객체다.
        // `CFDictionarySetValue` 는 표준 retain 콜백으로 값을 스스로
        // retain 하므로 우리가 만든 `CFRetained` 지역 변수가 이 함수 종료
        // 시점에 드롭돼도 안전하다.
        unsafe {
            CFMutableDictionary::set_value(
                Some(&dict),
                &*usage_page_key as *const CFString as *const c_void,
                &*usage_page_value as *const CFNumber as *const c_void,
            );
            CFMutableDictionary::set_value(
                Some(&dict),
                &*usage_key as *const CFString as *const c_void,
                &*usage_value as *const CFNumber as *const c_void,
            );
        }

        Some(dict)
    }

    /// 이터레이터를 끝까지 비운다(무장/재무장에 필수). `on_each` 는 살아있는
    /// 항목마다 호출한 뒤 즉시 해제한다.
    fn drain_iterator(iterator: IoIteratorT, mut on_each: impl FnMut()) {
        loop {
            // SAFETY: `iterator` 는 호출자가 보증하는 유효한 io_iterator_t 다.
            let item = unsafe { ffi::IOIteratorNext(iterator) };
            if item == 0 {
                break;
            }
            on_each();
            // SAFETY: `IOIteratorNext` 문서 — 반환된 항목은 호출자가 해제해야 한다.
            unsafe { ffi::IOObjectRelease(item) };
        }
    }

    /// # Safety
    /// IOKit 이 `IOServiceAddMatchingNotification` 에 등록된 콜백으로서만
    /// 이 함수를 호출한다. `refcon` 은 항상 `watch_keyboards` 가 넘긴
    /// `*mut CallbackWithKind` 다.
    unsafe extern "C" fn matching_notification_callback(
        refcon: *mut c_void,
        iterator: IoIteratorT,
    ) {
        // SAFETY: 위 문서 참조.
        let ctx = unsafe { &*(refcon as *const CallbackWithKind) };
        drain_iterator(iterator, || {
            let event = match ctx.kind {
                HotplugEventKind::Attached => HotplugEvent::Attached,
                HotplugEventKind::Detached => HotplugEvent::Detached,
            };
            (ctx.inner.callback)(event);
        });
    }

    pub struct KeyboardHotplugWatcher {
        notify_port: IONotificationPortRef,
        matched_iterator: IoIteratorT,
        terminated_iterator: IoIteratorT,
        matched_ctx: NonNull<CallbackWithKind>,
        terminated_ctx: NonNull<CallbackWithKind>,
        installed_run_loop: Option<CFRetained<CFRunLoop>>,
    }

    // SAFETY: 이 구조체가 담는 것은 전부 IOKit 이 발급한 정수/포인터 핸들과
    // Box 포인터다 — 실제 스레드 접근은 항상 이 구조체를 소유한 스레드가
    // 하고, IOKit 콜백 자체는 이 구조체가 등록해 둔 런루프의 스레드에서만
    // 호출된다. `KeyboardHotplugWatcher` 를 다른 스레드로 옮겨 보관하는 것
    // 자체는 안전하다(핸들 값을 옮기는 것뿐이므로).
    unsafe impl Send for KeyboardHotplugWatcher {}

    pub fn watch_keyboards(
        cb: Box<dyn Fn(HotplugEvent) + Send + Sync>,
    ) -> Option<KeyboardHotplugWatcher> {
        // SAFETY: `kIOMainPortDefault` 는 IOKit 이 항상 정의하는 정적 심볼이다.
        let notify_port = unsafe { ffi::IONotificationPortCreate(ffi::kIOMainPortDefault) };
        if notify_port.is_null() {
            return None;
        }

        let shared = Arc::new(HotplugContext { callback: cb });

        let matched_ctx = Box::into_raw(Box::new(CallbackWithKind {
            inner: Arc::clone(&shared),
            kind: HotplugEventKind::Attached,
        }));
        let terminated_ctx = Box::into_raw(Box::new(CallbackWithKind {
            inner: shared,
            kind: HotplugEventKind::Detached,
        }));

        let Some(matched_dict) = build_keyboard_matching_dict() else {
            // SAFETY: 아직 아무 콜백도 등록되지 않았으므로 되찾아 드롭해도 안전하다.
            drop(unsafe { Box::from_raw(matched_ctx) });
            drop(unsafe { Box::from_raw(terminated_ctx) });
            unsafe { ffi::IONotificationPortDestroy(notify_port) };
            return None;
        };
        let Some(terminated_dict) = build_keyboard_matching_dict() else {
            drop(unsafe { Box::from_raw(matched_ctx) });
            drop(unsafe { Box::from_raw(terminated_ctx) });
            unsafe { ffi::IONotificationPortDestroy(notify_port) };
            return None;
        };

        let mut matched_iterator: IoIteratorT = 0;
        // SAFETY: `matched_dict` 은 여기서 소비된다(CF_RELEASES_ARGUMENT) —
        // `into_raw` 로 소유권을 그대로 넘긴다. `matched_ctx` 는 유효한
        // `CallbackWithKind` 포인터다.
        let result = unsafe {
            ffi::IOServiceAddMatchingNotification(
                notify_port,
                K_IO_MATCHED_NOTIFICATION.as_ptr() as *const std::os::raw::c_char,
                CFRetained::into_raw(matched_dict).as_ptr(),
                Some(matching_notification_callback),
                matched_ctx as *mut c_void,
                &mut matched_iterator,
            )
        };
        if result != 0 {
            drop(unsafe { Box::from_raw(matched_ctx) });
            drop(unsafe { Box::from_raw(terminated_ctx) });
            // SAFETY: `terminated_dict` 는 아직 아무 호출에도 소비되지 않았다.
            unsafe { ffi::CFRelease(CFRetained::into_raw(terminated_dict).as_ptr() as *const c_void) };
            unsafe { ffi::IONotificationPortDestroy(notify_port) };
            return None;
        }
        // ⚠️ 최초 무장 — 기존 장치 목록을 조용히(콜백 호출 없이) 비운다.
        drain_iterator(matched_iterator, || {});

        let mut terminated_iterator: IoIteratorT = 0;
        // SAFETY: 위와 동일한 계약.
        let result = unsafe {
            ffi::IOServiceAddMatchingNotification(
                notify_port,
                K_IO_TERMINATED_NOTIFICATION.as_ptr() as *const std::os::raw::c_char,
                CFRetained::into_raw(terminated_dict).as_ptr(),
                Some(matching_notification_callback),
                terminated_ctx as *mut c_void,
                &mut terminated_iterator,
            )
        };
        if result != 0 {
            // SAFETY: 등록된 알림은 아직 없거나(matched 만 있음) 이 시점에
            // 정리해야 한다. `matched_iterator` 는 위에서 이미 비워졌지만
            // 알림 자체(및 그 iterator 핸들)는 여전히 살아있으므로 해제한다.
            drop(unsafe { Box::from_raw(matched_ctx) });
            drop(unsafe { Box::from_raw(terminated_ctx) });
            unsafe { ffi::IOObjectRelease(matched_iterator) };
            unsafe { ffi::IONotificationPortDestroy(notify_port) };
            return None;
        }
        // ⚠️ 최초 무장 — 위와 동일한 이유.
        drain_iterator(terminated_iterator, || {});

        let installed_run_loop = CFRunLoop::current().inspect(|rl| {
            // SAFETY: `IONotificationPortGetRunLoopSource` 는 "caller should
            // not release" 규칙을 따르는 Get 규칙 반환값이다 — 우리는 이
            // 포인터를 borrow 로만 다루고 release 하지 않는다. `notify_port`
            // 가 살아있는 동안(즉 `IONotificationPortDestroy` 전까지) 유효하다.
            let src_ptr = unsafe { ffi::IONotificationPortGetRunLoopSource(notify_port) };
            if let Some(src) = NonNull::new(src_ptr) {
                // SAFETY: `kCFRunLoopCommonModes` 정적 심볼 읽기.
                rl.add_source(Some(unsafe { src.as_ref() }), unsafe {
                    kCFRunLoopCommonModes
                });
            }
        });

        Some(KeyboardHotplugWatcher {
            notify_port,
            matched_iterator,
            terminated_iterator,
            // SAFETY: `Box::into_raw` 는 항상 널이 아닌 포인터를 반환한다.
            matched_ctx: unsafe { NonNull::new_unchecked(matched_ctx) },
            terminated_ctx: unsafe { NonNull::new_unchecked(terminated_ctx) },
            installed_run_loop,
        })
    }

    impl Drop for KeyboardHotplugWatcher {
        fn drop(&mut self) {
            // `IONotificationPortDestroy` 가 이 포트에서 얻은 런루프 소스도
            // 함께 파괴한다고 문서화되어 있으므로, 소스를 별도로 제거할
            // 필요는 없다 — 다만 먼저 파괴해 콜백 재진입을 막는다.
            // SAFETY: `notify_port` 는 `watch_keyboards` 가 만든 뒤 이
            // 구조체가 배타 소유해 온 값이다.
            unsafe { ffi::IONotificationPortDestroy(self.notify_port) };
            // SAFETY: 두 이터레이터 모두 이 구조체가 배타 소유해 온 값이다.
            unsafe {
                ffi::IOObjectRelease(self.matched_iterator);
                ffi::IOObjectRelease(self.terminated_iterator);
            }
            // SAFETY: 알림 포트를 파괴해 콜백 재진입이 불가능해진 뒤이므로
            // 이제 Box 를 안전하게 회수할 수 있다.
            drop(unsafe { Box::from_raw(self.matched_ctx.as_ptr()) });
            drop(unsafe { Box::from_raw(self.terminated_ctx.as_ptr()) });
            self.installed_run_loop = None;
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{watch_keyboards, KeyboardHotplugWatcher};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::HotplugEvent;

    pub struct KeyboardHotplugWatcher(core::convert::Infallible);

    pub fn watch_keyboards(
        _cb: Box<dyn Fn(HotplugEvent) + Send + Sync>,
    ) -> Option<KeyboardHotplugWatcher> {
        None
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{watch_keyboards, KeyboardHotplugWatcher};
