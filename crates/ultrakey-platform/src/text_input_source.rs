//! F-14(B) — Carbon HIToolbox 입력 소스 조회와 `UCKeyTranslate`.
//!
//! ⚠️ `TIS*`/`UCKeyTranslate`/`LMGetKbdType` 는 전용 크레이트가 없다 —
//! `crate::ffi` 의 수기 `extern "C"` 선언을 그대로 쓴다(명세 §7 이 이미
//! `Rust 바인딩`으로 판정했다). `TISCopy*` 가 반환하는 값은 전부 Copy
//! 규칙(호출자가 해제)을 따르므로, 아래 코드는 매 반환값을 정확히 한 번
//! `CFRelease` 한다.

/// 현재 입력 소스에서 뽑아낸, 레이아웃 독립 판정에 필요한 최소 정보.
pub struct LayoutSnapshot {
    pub source_id: String,
    pub is_ascii_capable: bool,
    pub used_ascii_fallback: bool,
    /// `LMGetKbdType()` — 물리 키보드 하드웨어 타입.
    pub keyboard_type: u32,
    /// `kTISPropertyUnicodeKeyLayoutData` 의 원시 바이트 — `uchr` 리소스.
    pub uchr_data: Vec<u8>,
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::LayoutSnapshot;
    use crate::ffi::{self, TISInputSourceRef};
    use core::ffi::c_void;
    use core::ptr::NonNull;
    use objc2_core_foundation::{
        CFDictionary, CFNotificationCenter, CFNotificationName, CFNotificationSuspensionBehavior,
        CFBoolean, CFData, CFString,
    };
    use ultrakey_core::keycode::KeyCode;

    /// ⭐ `key-remapping-engine.md` §3-e 의 5단계 절차를 그대로 구현한다.
    pub fn current_layout() -> Option<LayoutSnapshot> {
        // 1. TISCopyCurrentKeyboardInputSource()
        // SAFETY: 인자 없는 순수 조회 함수. 반환값은 Copy 규칙(호출자가 해제).
        let mut src: TISInputSourceRef = unsafe { ffi::TISCopyCurrentKeyboardInputSource() };
        if src.is_null() {
            return None;
        }

        // 2. ASCII 가능 여부 판정.
        // SAFETY: `src` 는 위에서 확인한 유효한 포인터다. `TISGetInputSourceProperty`
        // 는 Get 규칙이라 반환값을 release 하면 안 된다.
        let is_ascii_capable = unsafe {
            let prop = ffi::TISGetInputSourceProperty(
                src,
                ffi::kTISPropertyInputSourceIsASCIICapable,
            );
            if prop.is_null() {
                false
            } else {
                (*(prop as *const CFBoolean)).value()
            }
        };

        // 3. ASCII 불가면(CJK IME 등) 대체 레이아웃으로 교체.
        let mut used_ascii_fallback = false;
        if !is_ascii_capable {
            // SAFETY: 인자 없는 순수 조회 함수. 반환값은 Copy 규칙.
            let fallback = unsafe { ffi::TISCopyCurrentASCIICapableKeyboardLayoutInputSource() };
            if !fallback.is_null() {
                // SAFETY: `src` 는 우리가 아직 release 하지 않은, 우리가
                // 소유한 참조다.
                unsafe { ffi::CFRelease(src as *const c_void) };
                src = fallback;
                used_ascii_fallback = true;
            }
        }

        // source_id (문서화된 필드는 아니지만 진단·캐시 키로 유용해 함께 뽑는다)
        // SAFETY: `src` 는 유효하다. Get 규칙.
        let source_id = unsafe {
            let prop = ffi::TISGetInputSourceProperty(src, ffi::kTISPropertyInputSourceID);
            if prop.is_null() {
                String::new()
            } else {
                (*(prop as *const CFString)).to_string()
            }
        };

        // 4. LMGetKbdType() — 물리 키보드 하드웨어 타입.
        // SAFETY: 인자 없는 순수 조회 함수.
        let keyboard_type = unsafe { ffi::LMGetKbdType() } as u32;

        // 5. kTISPropertyUnicodeKeyLayoutData → uchr 원시 바이트.
        // SAFETY: `src` 는 유효하다. Get 규칙 — 복사(`to_vec`)해 소유권
        // 문제를 없앤다.
        let uchr_data = unsafe {
            let prop = ffi::TISGetInputSourceProperty(src, ffi::kTISPropertyUnicodeKeyLayoutData);
            if prop.is_null() {
                Vec::new()
            } else {
                (*(prop as *const CFData)).to_vec()
            }
        };

        // SAFETY: `src` 는 이 함수가 소유해 온 마지막 참조다.
        unsafe { ffi::CFRelease(src as *const c_void) };

        Some(LayoutSnapshot {
            source_id,
            is_ascii_capable,
            used_ascii_fallback,
            keyboard_type,
            uchr_data,
        })
    }

    /// macOS 표준 "dead key 를 스페이스로 방출" 기법(Carbon `UCKeyTranslate`
    /// 문서, Chromium/WebKit 등이 동일하게 쓰는 기법) — dead key 는 그
    /// 자체로는 문자를 내지 않으므로 두 번째 호출로 홀로 눌렸을 때의
    /// 표시형을 확정한다.
    const K_VK_SPACE: u16 = 49;

    /// `uchr` 바이트 + 키보드 타입 + 물리 keycode + modifier 상태로 문자를
    /// 역산한다. `modifiers` 는 이미 Carbon `modifierKeyState` 형식(예:
    /// `shiftKeyBit`=9 등)으로 변환되어 있다고 가정한다 — 그 변환은 이
    /// 함수를 호출하는 쪽(`ultrakey-layout`, F-14)의 책임이다.
    pub fn translate(
        uchr: &[u8],
        kbd_type: u32,
        keycode: KeyCode,
        modifiers: u32,
    ) -> Option<String> {
        if uchr.is_empty() {
            return None;
        }
        // SAFETY: `uchr` 는 `kTISPropertyUnicodeKeyLayoutData` 에서 그대로
        // 복사해 온 바이트라 `UCKeyboardLayout` 이 기대하는 레이아웃과
        // 일치한다. macOS(Darwin) 의 기본 할당자는 항상 최소 16바이트
        // 정렬을 보장하므로, `Vec<u8>` 로 복사해도 C 구조체가 요구하는
        // 정렬을 실무적으로 만족한다 — 이는 Apple 문서가 권장하고
        // Chromium/WebKit 등이 동일하게 쓰는 표준 기법이다.
        let layout_ptr = uchr.as_ptr() as *const ffi::OpaqueUCKeyboardLayout;

        let mut dead_key_state: u32 = 0;
        let mut buf = [0u16; 4];
        let mut actual_len: usize = 0;
        // SAFETY: 모든 포인터 인자가 유효한 스택 변수/슬라이스를 가리킨다.
        let status = unsafe {
            ffi::UCKeyTranslate(
                layout_ptr,
                keycode.0,
                ffi::K_UC_KEY_ACTION_DOWN,
                modifiers,
                kbd_type,
                0,
                &mut dead_key_state,
                buf.len(),
                &mut actual_len,
                buf.as_mut_ptr(),
            )
        };
        if status != 0 {
            return None;
        }

        if dead_key_state != 0 {
            // ⭐ dead key — 두 번째 호출로 방출한다(spec 지시).
            let mut buf2 = [0u16; 4];
            let mut actual_len2: usize = 0;
            // SAFETY: 위와 동일.
            let status2 = unsafe {
                ffi::UCKeyTranslate(
                    layout_ptr,
                    K_VK_SPACE,
                    ffi::K_UC_KEY_ACTION_DOWN,
                    0,
                    kbd_type,
                    ffi::K_UC_KEY_TRANSLATE_NO_DEAD_KEYS_MASK,
                    &mut dead_key_state,
                    buf2.len(),
                    &mut actual_len2,
                    buf2.as_mut_ptr(),
                )
            };
            if status2 != 0 {
                return None;
            }
            return String::from_utf16(&buf2[..actual_len2]).ok();
        }

        String::from_utf16(&buf[..actual_len]).ok()
    }

    struct ObserverContext {
        callback: Box<dyn Fn() + Send + Sync>,
    }

    /// # Safety
    /// CoreFoundation 이 `CFNotificationCenterAddObserver` 에 등록된
    /// 콜백으로서만 이 함수를 호출한다. `observer` 는 항상
    /// `observe_input_source_changes` 가 넘긴 `*mut ObserverContext` 다.
    unsafe extern "C-unwind" fn notification_callback(
        _center: *mut CFNotificationCenter,
        observer: *mut c_void,
        _name: *const CFNotificationName,
        _object: *const c_void,
        _user_info: *const CFDictionary,
    ) {
        // SAFETY: 위 문서 참조.
        let ctx = unsafe { &*(observer as *const ObserverContext) };
        (ctx.callback)();
    }

    /// `kTISNotifySelectedKeyboardInputSourceChanged` 구독 토큰.
    pub struct InputSourceObserver {
        ctx: NonNull<ObserverContext>,
    }

    // SAFETY: 이 구조체가 담는 것은 Box 포인터 하나뿐이고, 실제 콜백은
    // Distributed Notification Center 가 등록 당시의 스레드에서 부른다 —
    // 핸들 자체를 다른 스레드로 옮겨 보관하는 것은 안전하다.
    unsafe impl Send for InputSourceObserver {}

    pub fn observe_input_source_changes(
        cb: Box<dyn Fn() + Send + Sync>,
    ) -> InputSourceObserver {
        let ctx_ptr = Box::into_raw(Box::new(ObserverContext { callback: cb }));

        if let Some(center) = CFNotificationCenter::distributed_center() {
            // SAFETY: `ctx_ptr` 은 방금 만든 유효한 포인터이고,
            // `notification_callback` 은 `CFNotificationCallback` 시그니처와
            // 정확히 일치한다. `kTISNotifySelectedKeyboardInputSourceChanged`
            // 는 Carbon 프레임워크가 항상 정의하는 정적 심볼이다.
            unsafe {
                center.add_observer(
                    ctx_ptr as *const c_void,
                    Some(notification_callback),
                    ffi::kTISNotifySelectedKeyboardInputSourceChanged,
                    std::ptr::null(),
                    CFNotificationSuspensionBehavior::DeliverImmediately,
                );
            }
        }

        InputSourceObserver {
            // SAFETY: `Box::into_raw` 는 항상 널이 아닌 포인터를 반환한다.
            ctx: unsafe { NonNull::new_unchecked(ctx_ptr) },
        }
    }

    impl Drop for InputSourceObserver {
        fn drop(&mut self) {
            if let Some(center) = CFNotificationCenter::distributed_center() {
                // SAFETY: `self.ctx` 는 등록 시 넘긴 것과 같은 포인터다.
                unsafe {
                    center.remove_observer(
                        self.ctx.as_ptr() as *const c_void,
                        ffi::kTISNotifySelectedKeyboardInputSourceChanged,
                        std::ptr::null(),
                    );
                }
            }
            // SAFETY: 위에서 구독을 해지해 콜백 재진입이 불가능해진 뒤이므로
            // 이제 이 Box 를 안전하게 회수할 수 있다.
            drop(unsafe { Box::from_raw(self.ctx.as_ptr()) });
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{current_layout, observe_input_source_changes, translate, InputSourceObserver};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::LayoutSnapshot;
    use ultrakey_core::keycode::KeyCode;

    pub fn current_layout() -> Option<LayoutSnapshot> {
        None
    }

    pub fn translate(
        _uchr: &[u8],
        _kbd_type: u32,
        _keycode: KeyCode,
        _modifiers: u32,
    ) -> Option<String> {
        None
    }

    pub struct InputSourceObserver(core::convert::Infallible);

    pub fn observe_input_source_changes(
        _cb: Box<dyn Fn() + Send + Sync>,
    ) -> InputSourceObserver {
        panic!("ultrakey-platform: observe_input_source_changes 는 macOS 전용이다")
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{current_layout, observe_input_source_changes, translate, InputSourceObserver};
