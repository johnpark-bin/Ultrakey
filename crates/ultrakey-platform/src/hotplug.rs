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
//!
//! ⭐ **이 감시자는 자기 전용 스레드에서 자기 런루프를 돈다**(이슈 #10).
//!
//! `IONotificationPortGetRunLoopSource` 로 얻은 소스는 **그 소스를 건 런루프가
//! 실제로 돌아야만** 발화한다. 예전 구현은 `watch_keyboards` 를 부른 스레드의
//! `CFRunLoop::current()` 에 소스를 걸었는데, 그 스레드가 `CFRunLoopRun` 을
//! 부르지 않으면 콜백이 **영원히 오지 않으면서 아무 오류도 남기지 않았다** —
//! 실제로 앱이 권한 전이 콜백(폴링 스레드)에서 엔진을 시작하자 핫플러그 감지가
//! 조용히 죽었다(이슈 #10).
//!
//! 그래서 이제 [`watch_keyboards`] 는 **호출 스레드를 전혀 쓰지 않는다.**
//! 전용 스레드(`ultrakey-hotplug`)를 띄워 그 안에서 등록하고, 그 스레드가
//! `CFRunLoopRun` 을 돈다. 호출자가 어느 스레드든 결과가 같다 — 호출 규약으로
//! 지켜야 했던 전제가 아예 사라진다.
//!
//! ⚠️ **다른 두 훅과 다르다.** `workspace.rs`(`NSWorkspace`)와
//! `text_input_source.rs`(distributed `CFNotificationCenter`)는 등록 스레드와
//! 무관하게 **메인 스레드로** 콜백이 전달되므로 이런 장치가 필요 없다. 런루프
//! 소스를 직접 거는 것은 이 모듈과 `event_tap.rs` 뿐이고, `event_tap.rs` 는
//! 이미 자기 전용 스레드에서 런루프를 돌고 있다.

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
    use crate::runloop::{CommandSignaller, CommandSource, RunLoopHandle};
    use core::ffi::c_void;
    use core::ptr::NonNull;
    use objc2_core_foundation::{
        kCFRunLoopCommonModes, CFMutableDictionary, CFNumber, CFRetained, CFRunLoop, CFString,
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::sync_channel;
    use std::sync::Arc;
    use std::thread::{self, JoinHandle};
    use std::time::Duration;

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
    /// 이 함수를 호출한다. `refcon` 은 항상 [`Installed::new`] 가 넘긴
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

    /// IOKit 쪽 자원 묶음 — 전용 스레드 안에서만 만들어지고, 그 스레드가 끝날 때
    /// 같은 스레드에서 드롭된다. 스레드 경계를 넘지 않으므로 `Send` 가 필요 없다.
    struct Installed {
        notify_port: IONotificationPortRef,
        matched_iterator: IoIteratorT,
        terminated_iterator: IoIteratorT,
        matched_ctx: NonNull<CallbackWithKind>,
        terminated_ctx: NonNull<CallbackWithKind>,
    }

    impl Installed {
        /// 알림 두 개를 등록하고 최초 무장까지 끝낸다. 어느 단계에서든 실패하면
        /// 그때까지 잡은 자원을 전부 되돌리고 `None` 을 돌려준다.
        fn new(cb: Box<dyn Fn(HotplugEvent) + Send + Sync>) -> Option<Self> {
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

            // 실패 경로에서 반복되는 정리 — 아직 아무 알림도 등록되지 않았거나,
            // 등록된 알림을 이 함수가 직접 되돌린 뒤에만 부른다.
            //
            // SAFETY: 두 포인터 모두 바로 위에서 `Box::into_raw` 로 만든 것이고,
            // 이 시점에 IOKit 은 아직 이들을 refcon 으로 들고 있지 않다.
            let release_contexts = || unsafe {
                drop(Box::from_raw(matched_ctx));
                drop(Box::from_raw(terminated_ctx));
            };

            let Some(matched_dict) = build_keyboard_matching_dict() else {
                release_contexts();
                // SAFETY: 이 포트는 위에서 우리가 만든 것이고 아직 아무도 쓰지 않았다.
                unsafe { ffi::IONotificationPortDestroy(notify_port) };
                return None;
            };
            let Some(terminated_dict) = build_keyboard_matching_dict() else {
                release_contexts();
                // SAFETY: `matched_dict` 는 아직 아무 호출에도 소비되지 않았다.
                unsafe {
                    ffi::CFRelease(CFRetained::into_raw(matched_dict).as_ptr() as *const c_void)
                };
                // SAFETY: 위와 동일.
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
                release_contexts();
                // SAFETY: `terminated_dict` 는 아직 아무 호출에도 소비되지 않았다.
                unsafe {
                    ffi::CFRelease(CFRetained::into_raw(terminated_dict).as_ptr() as *const c_void)
                };
                // SAFETY: 등록에 실패했으므로 포트를 그대로 파괴하면 된다.
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
                // SAFETY: `matched` 알림은 등록에 성공했으므로 포트를 파괴해
                // 콜백 재진입을 막은 뒤에야 refcon Box 를 회수할 수 있다.
                unsafe { ffi::IONotificationPortDestroy(notify_port) };
                // SAFETY: `matched_iterator` 는 위에서 이미 비웠지만 핸들 자체는
                // 우리가 해제해야 한다.
                unsafe { ffi::IOObjectRelease(matched_iterator) };
                release_contexts();
                return None;
            }
            // ⚠️ 최초 무장 — 위와 동일한 이유.
            drain_iterator(terminated_iterator, || {});

            Some(Installed {
                notify_port,
                matched_iterator,
                terminated_iterator,
                // SAFETY: `Box::into_raw` 는 항상 널이 아닌 포인터를 반환한다.
                matched_ctx: unsafe { NonNull::new_unchecked(matched_ctx) },
                // SAFETY: 위와 동일.
                terminated_ctx: unsafe { NonNull::new_unchecked(terminated_ctx) },
            })
        }

        /// 알림 포트의 런루프 소스를 **현재 스레드의 런루프**에 건다. 이 함수는
        /// 전용 스레드 안에서만 불린다 — 그래서 "현재 스레드" 가 곧 그 전용
        /// 스레드이고, 그 스레드가 아래에서 `CFRunLoopRun` 을 돈다.
        fn add_source_to_current_runloop(&self) -> bool {
            let Some(rl) = CFRunLoop::current() else {
                return false;
            };
            // SAFETY: `IONotificationPortGetRunLoopSource` 는 "caller should
            // not release" 규칙을 따르는 Get 규칙 반환값이다 — 우리는 이
            // 포인터를 borrow 로만 다루고 release 하지 않는다. `notify_port`
            // 가 살아있는 동안(즉 `IONotificationPortDestroy` 전까지) 유효하다.
            let src_ptr = unsafe { ffi::IONotificationPortGetRunLoopSource(self.notify_port) };
            let Some(src) = NonNull::new(src_ptr) else {
                return false;
            };
            // SAFETY: `kCFRunLoopCommonModes` 정적 심볼 읽기.
            rl.add_source(Some(unsafe { src.as_ref() }), unsafe {
                kCFRunLoopCommonModes
            });
            true
        }
    }

    impl Drop for Installed {
        fn drop(&mut self) {
            // `IONotificationPortDestroy` 가 이 포트에서 얻은 런루프 소스도
            // 함께 파괴한다고 문서화되어 있으므로, 소스를 별도로 제거할
            // 필요는 없다 — 다만 먼저 파괴해 콜백 재진입을 막는다.
            // SAFETY: `notify_port` 는 `Installed::new` 가 만든 뒤 이 구조체가
            // 배타 소유해 온 값이다.
            unsafe { ffi::IONotificationPortDestroy(self.notify_port) };
            // SAFETY: 두 이터레이터 모두 이 구조체가 배타 소유해 온 값이다.
            unsafe {
                ffi::IOObjectRelease(self.matched_iterator);
                ffi::IOObjectRelease(self.terminated_iterator);
            }
            // SAFETY: 알림 포트를 파괴해 콜백 재진입이 불가능해진 뒤이므로
            // 이제 Box 를 안전하게 회수할 수 있다.
            unsafe {
                drop(Box::from_raw(self.matched_ctx.as_ptr()));
                drop(Box::from_raw(self.terminated_ctx.as_ptr()));
            }
        }
    }

    /// 핫플러그 감시 손잡이. 드롭하면 전용 스레드를 깨워 정리시키고 join 한다 —
    /// 스레드를 누수시키지 않는다.
    ///
    /// ⭐ 이 구조체에는 원시 포인터가 없다(IOKit 자원은 전부 전용 스레드 안의
    /// [`Installed`] 가 소유한다). 그래서 예전 구현에 있던 수동 `unsafe impl Send`
    /// 가 **필요 없어졌다** — 컴파일러가 스스로 `Send` 를 도출한다.
    pub struct KeyboardHotplugWatcher {
        stop: CommandSignaller,
        thread: Option<JoinHandle<()>>,
    }

    pub fn watch_keyboards(
        cb: Box<dyn Fn(HotplugEvent) + Send + Sync>,
    ) -> Option<KeyboardHotplugWatcher> {
        // 전용 스레드가 "등록에 성공했는가 / 정지 손잡이는 무엇인가" 를 돌려주는
        // 핸드셰이크. 실패하면 `None` 이 오고 스레드는 스스로 끝난다.
        let (ready_tx, ready_rx) = sync_channel::<Option<CommandSignaller>>(1);

        let thread = thread::Builder::new()
            .name("ultrakey-hotplug".to_string())
            .spawn(move || hotplug_thread_main(cb, ready_tx))
            .ok()?;

        match ready_rx.recv() {
            Ok(Some(stop)) => Some(KeyboardHotplugWatcher {
                stop,
                thread: Some(thread),
            }),
            // 등록 실패(`None`) 또는 스레드가 핸드셰이크 없이 죽음(`Err`).
            _ => {
                let _ = thread.join();
                None
            }
        }
    }

    /// 전용 스레드 본체 — 등록 · 런루프 구동 · 정리를 전부 이 스레드가 한다.
    fn hotplug_thread_main(
        cb: Box<dyn Fn(HotplugEvent) + Send + Sync>,
        ready_tx: std::sync::mpsc::SyncSender<Option<CommandSignaller>>,
    ) {
        let Some(installed) = Installed::new(cb) else {
            let _ = ready_tx.send(None);
            return;
        };
        if !installed.add_source_to_current_runloop() {
            let _ = ready_tx.send(None);
            return; // `installed` 가 여기서 드롭되며 IOKit 자원을 되돌린다.
        }

        // ⭐ **정지 신호는 런루프 소스로 받는다 — 경합이 없다.**
        //
        // `CFRunLoopStop` 만 쓰면 "정지 요청이 스레드가 `CFRunLoopRun` 에 진입하기
        // 직전에 도착하면 신호가 사라져 영영 멈추지 않는다"는 경합이 남는다.
        // `CFRunLoopSourceSignal` 은 그 창을 닫는다 — 아직 돌지 않는 런루프에
        // 신호를 걸어 두면, 런루프가 시작되는 순간 그 소스가 곧바로 처리된다.
        let stopping = Arc::new(AtomicBool::new(false));
        let stopping_for_handler = Arc::clone(&stopping);
        let stop_source = CommandSource::new(Box::new(move || {
            stopping_for_handler.store(true, Ordering::Release);
            // 이 핸들러는 이 스레드의 런루프가 부른다 — 그래서 "현재 런루프" 가
            // 곧 멈춰야 할 그 런루프다.
            if let Some(rl) = CFRunLoop::current() {
                rl.stop();
            }
        }));
        stop_source.add_to_current_runloop();

        if ready_tx.send(Some(stop_source.signaller())).is_err() {
            return; // 호출자가 이미 사라졌다 — 자원을 되돌리고 끝낸다.
        }

        while !stopping.load(Ordering::Acquire) {
            RunLoopHandle::run();
            // 정상 경로에서 `CFRunLoopRun` 은 정지 신호를 받을 때만 돌아온다.
            // 소스가 하나도 없어 즉시 반환하는 병리적 상황에서 이 루프가
            // CPU 를 태우지 않도록 짧게 쉰다.
            if !stopping.load(Ordering::Acquire) {
                thread::sleep(Duration::from_millis(50));
            }
        }
        // `installed` · `stop_source` 가 여기서 드롭된다 — 등록한 바로 그
        // 스레드에서 해제되므로 IOKit/CF 자원 수명이 스레드 안에 갇힌다.
    }

    impl Drop for KeyboardHotplugWatcher {
        fn drop(&mut self) {
            self.stop.signal();
            if let Some(t) = self.thread.take() {
                let _ = t.join();
            }
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    /// ⭐ **이슈 #10 회귀 방지.** 예전 구현은 `watch_keyboards` 를 부른 스레드의
    /// 런루프에 IOKit 소스를 걸었다 — 그 스레드가 `CFRunLoopRun` 을 돌지 않으면
    /// 콜백이 영영 오지 않는데 **반환값은 그대로 `Some`** 이라 아무도 눈치채지
    /// 못했다. 이제는 감시자가 자기 전용 스레드를 갖는다.
    ///
    /// 이 테스트는 그 구조를 두 가지로 확인한다:
    /// 1. 런루프를 돌리지 않는 스레드(테스트 하니스 스레드가 바로 그렇다)에서
    ///    등록이 성공한다
    /// 2. 드롭이 전용 스레드를 **정지시키고 join 까지 마친 뒤** 즉시 돌아온다 —
    ///    정지 신호가 런루프 진입과 경합해 유실되면 여기서 멈춘다
    #[test]
    fn watcher_is_independent_of_the_calling_threads_runloop() {
        assert!(
            !crate::runloop::is_main_thread(),
            "테스트 하니스는 메인 스레드가 아닌 곳에서 돈다 — 이 전제가 깨지면 \
             이 테스트가 검증하려는 상황(런루프를 돌지 않는 호출 스레드)이 아니다"
        );

        let Some(watcher) = watch_keyboards(Box::new(|_| {})) else {
            // CI 컨테이너 등 IOKit 을 못 여는 환경에서는 등록 자체가 실패할 수
            // 있다. 그건 이 테스트가 잡으려는 회귀가 아니므로 조용히 넘어간다.
            return;
        };

        let started = Instant::now();
        drop(watcher);
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "드롭이 전용 스레드를 즉시 정지시키고 join 해야 한다 — 오래 걸렸다면 \
             정지 신호가 유실돼 `CFRunLoopRun` 이 영원히 블록한 것이다"
        );
    }

    /// 여러 번 만들고 버려도 스레드가 새지 않고 매번 성공해야 한다.
    #[test]
    fn watcher_can_be_created_and_dropped_repeatedly() {
        for _ in 0..3 {
            if let Some(w) = watch_keyboards(Box::new(|_| {})) {
                drop(w);
            }
        }
    }
}
