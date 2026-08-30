//! ⭐ F-03 — 오버레이 창의 **네이티브 설정**. `ns_window()` 로 얻은 `NSWindow`
//! 를 직접 만진다.
//!
//! 대응 명세: `docs/spec/seek-overlay-ui.md` §3.2 · §7 (A).
//!
//! ## 왜 이 파일이 존재하는가
//!
//! 명세 §7 (A) 가 이미 판정했다 — **Tauri 설정만으로는 이 오버레이를 완성할 수
//! 없다.** `decorations`/`transparent`/`always_on_top`/`visible_on_all_workspaces`
//! /`shadow`/클릭 통과는 Tauri 로 되지만, 아래 넷은 Tauri 설정에 아예 없다.
//!
//! | 요구 | 이 모듈이 부르는 것 |
//! | :--- | :--- |
//! | 전체화면 앱 위 표시 | `setLevel:` |
//! | **활성화하지 않고 표시** | `setStyleMask:`(borderless) + `orderFrontRegardless` |
//! | Space·전체화면 동행 | `setCollectionBehavior:` |
//! | 스크린샷 포함 여부 | `setSharingType:` |
//!
//! ⛔ **Swift/Objective-C 소스 파일은 하나도 없다.** `objc2-app-kit` 이 헤더
//! 자동 생성 바인딩이라 전부 Rust 안에서 끝난다(명세 §7 의 결론).
//!
//! ## ⭐⭐ `canBecomeKey = false` — 실기기가 첫 번째 방법을 반증했다
//!
//! `NSWindow.canBecomeKeyWindow` 는 **읽기 전용 메서드**라 프로퍼티로 끌 수
//! 없다.
//!
//! **처음 시도한 방법**: AppKit 문서가 *"The value of this property is `true`
//! if the window has a title bar or a resize bar, or `false` otherwise"* 라고
//! 하므로, 스타일 마스크를 `NSWindowStyleMaskBorderless` 로 못박으면 기본
//! 구현이 알아서 `false` 를 낸다 — 서브클래싱이 필요 없다.
//!
//! ⛔ **실기기에서 반증됐다.** 마스크를 borderless 로 바꾼 **뒤에도**
//! `canBecomeKeyWindow` 가 `true` 였다(로그: `can_become_key=Some(true)`).
//! 이유는 Tauri 의 창 백엔드(tao)가 `NSWindow` 를 서브클래싱하면서
//! **`canBecomeKeyWindow` 를 무조건 `YES` 로 오버라이드**해 두었기 때문이다 —
//! 데코레이션 없는 창도 키 입력을 받아야 하는 일반 앱을 위한 선택이고,
//! AppKit 의 기본 구현은 애초에 호출되지 않는다.
//!
//! **두 번째 시도도 실기기가 반증했다 — `object_setClass`**: 런타임에 tao 의
//! 클래스를 상속한 서브클래스를 만들어 `object_setClass` 로 바꿔 끼우는 방법
//! (`tauri-nspanel` 이 쓰는 기법)이다. ⛔ **앱이 즉시 죽었다** — 크래시
//! 리포트도 Rust 패닉 로그도 남지 않는 하드 크래시였고, 킬 스위치
//! (`ULTRAKEY_OVERLAY_NO_SWAP`)로 껐을 때만 살아남는 것으로 원인을 확정했다.
//! 이유로 추정되는 것: Tauri 의 창은 생성 시점에 이미 KVO 로 스위즐돼 있어
//! (`object_getClass` 가 `NSKVONotifying_TaoWindow` 를 돌려준다) isa 를 우리가
//! 다시 바꾸면 KVO 의 isa 복원·알림 경로와 어긋난다.
//!
//! **그래서 실제로 쓰는 방법 — isa 를 건드리지 않는 표적 스위즐**:
//! [`NsWindowHandle::force_non_activating`] 은 창의 **클래스에**
//! `canBecomeKeyWindow`/`canBecomeMainWindow` 구현을 갈아 끼우고
//! (`class_replaceMethod`), 그 구현이 **등록부에 있는 창에 대해서만** `NO` 를
//! 돌려주고 나머지는 **원래 구현으로 넘긴다**. 즉:
//!
//! - 객체의 isa 는 그대로다 → KVO 가 깨지지 않는다.
//! - 같은 클래스를 쓰는 다른 Tauri 창(환경설정·권한 모달)은 등록부에 없으므로
//!   **동작이 한 치도 달라지지 않는다** — 그 창들은 여전히 키 윈도우가 된다.
//! - 스위즐은 프로세스당 한 번만 한다([`std::sync::OnceLock`]).
//!
//! ⭐ **왜 이것이 중요한가**: 검색 바는 클릭 통과를 걸 수 없다(드래그로 옮길
//! 수 있어야 하므로, §3.2 정정). 클릭 통과가 아닌 창은 사용자가 누르는 순간
//! 키 윈도우가 되려 하고, 그러면 Accessory 앱이 활성화되어 **원래 앱의 입력
//! 포커스가 깨진다** — 이 문서 §1 이 "가장 중요한 제약" 이라 부른 바로 그것이다.
//!
//! [`NsWindowHandle::can_become_key`] 로 설정 뒤 실제 값을 되읽어 확인한다 —
//! 이 검증이 없었으면 첫 번째 방법의 실패를 눈치채지 못했을 것이다.

//! ## 안전 경계 — [`NsWindowHandle`]
//!
//! 원시 포인터를 함수마다 받으면 "이 포인터가 유효한가"라는 계약이 함수 수만큼
//! 생긴다. 그래서 **`unsafe` 를 손잡이 생성 한 곳으로 모으고**, 그 뒤의
//! 메서드는 전부 안전하게 노출한다.

/// 오버레이 창의 종류 — 창마다 요구가 다르다(§3.2 첫 문단).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayWindowKind {
    /// 디스플레이별 하이라이트 창. 클릭을 **통과**시킨다.
    Highlight,
    /// 독립 검색 바(`EntryBarWindow`). ⭐ 클릭을 통과시키지 **않는다** —
    /// 사용자가 드래그해 옮길 수 있어야 하고(§3.4, §8 수용 기준 "검색 바를
    /// 드래그해 옮기면 위치가 유지된다"), 클릭 통과 창은 드래그 자체를 받지
    /// 못하기 때문이다. 명세 §3.2 본문은 "클릭 통과는 두 창 모두에
    /// 적용된다"고 적었으나, 그대로 하면 같은 명세 §8 의 수용 기준이
    /// 구현 불가능해진다 — 수용 기준을 우선했다.
    SearchBar,
}

/// 창을 스크린샷·화면 녹화에 포함할 것인가(§5 엣지케이스 12, §9 미해결 4).
///
/// 기본은 [`Sharing::ReadOnly`](Sharing::ReadOnly) — macOS 기본값과 같고,
/// 오버레이가 스크린샷에 **찍힌다**. 명세가 정책을 정하지 않았으므로 기본값을
/// 바꾸지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Sharing {
    /// 캡처에 포함된다(macOS 기본).
    ReadOnly,
    /// 캡처에서 제외된다.
    None,
}

/// ⭐ Tauri `WebviewWindow::ns_window()` 포인터를 감싼 손잡이.
///
/// 이 타입을 만드는 [`NsWindowHandle::from_raw`] 하나만 `unsafe` 이고, 그 뒤의
/// 메서드는 전부 안전하다 — 이 크레이트가 다른 FFI 에서도 쓰는 규약이다.
///
/// ⚠️ **메인 스레드 전용.** `NSWindow` 조작은 메인 스레드에서만 유효하므로
/// 이 타입은 의도적으로 `Send`/`Sync` 가 아니다(원시 포인터 필드가 그것을
/// 자동으로 막는다).
#[derive(Debug, Clone, Copy)]
pub struct NsWindowHandle {
    ptr: *mut core::ffi::c_void,
}

impl NsWindowHandle {
    /// ⭐ Tauri 의 `WebviewWindow::ns_window()` 가 돌려준 포인터로 손잡이를
    /// 만든다. **`unsafe` 가 아니다** — 그 이유를 아래에 정확히 적는다.
    ///
    /// ## 왜 안전 함수인가
    ///
    /// `apps/ultrakey-app` 은 `#![forbid(unsafe_code)]` 다(`architecture.md`
    /// §1: "모든 `unsafe` 가 `ultrakey-platform` 에만 있다"). 그런데 Tauri 는
    /// 원시 포인터로만 `NSWindow` 를 내주므로, 그 포인터를 객체로 되살리는
    /// 단계가 **반드시 이 크레이트 안에 있어야** 한다.
    ///
    /// 이 함수는 두 가지를 실제로 검사한다:
    /// 1. 널이 아닌가.
    /// 2. ⭐ **정말 `NSWindow` 인스턴스인가** — objc2 의 `downcast_ref` 가
    ///    `isKindOfClass:` 를 돌려 확인한다. 다른 객체(예: `NSView`)를 잘못
    ///    넘기면 `None` 이 되지 조용히 엉뚱한 메서드를 부르지 않는다.
    ///
    /// ## ⚠️ 남는 신뢰 — 정직하게 적는다
    ///
    /// 검사로 걸러낼 수 **없는** 것이 하나 있다: `ptr` 가 **이미 해제된**
    /// 메모리를 가리키는 경우다(그 경우 isa 를 읽는 것 자체가 UB다). 이
    /// 함수는 그것을 "Tauri 가 살아 있는 창의 `ns_window()` 를 돌려준다"는
    /// 계약으로 신뢰한다. 호출자는 그 계약을 지키기 위해 **`WebviewWindow` 가
    /// 살아 있는 동안에만** 이 손잡이를 만들고 쓴다 — 실제로 `overlay.rs` 의
    /// 모든 호출은 창을 손에 쥔 채 메인 스레드 클로저 안에서 일어난다.
    #[must_use]
    pub fn from_tauri_ptr(ptr: *mut core::ffi::c_void) -> Option<Self> {
        if ptr.is_null() {
            tracing::error!("ns_window() returned null; cannot apply native overlay window settings");
            return None;
        }
        if !imp::is_ns_window(ptr) {
            tracing::error!("ns_window() returned a pointer that is not an NSWindow; skipping setup");
            return None;
        }
        Some(Self { ptr })
    }

    /// 네 가지 네이티브 요구(레벨·활성화 없이 표시·collection behavior·
    /// 스크린샷 공유)를 한 번에 건다. 성공 여부를 돌려준다.
    pub fn configure(self, kind: OverlayWindowKind, sharing: Sharing) -> bool {
        imp::configure(self.ptr, kind, sharing)
    }

    /// ⭐⭐ 이 창이 **키 윈도우가 될 수 없게** 만든다.
    ///
    /// 런타임에 서브클래스를 만들어 `canBecomeKeyWindow`/`canBecomeMainWindow`
    /// 를 `NO` 로 오버라이드하고 클래스를 바꿔 끼운다 — 왜 스타일 마스크만으로는
    /// 안 되는지는 모듈 문서 참조(실기기가 반증했다).
    ///
    /// 성공하면 `true`. 이미 바꿔 끼운 창에 다시 불러도 안전하다.
    pub fn force_non_activating(self) -> bool {
        imp::force_non_activating(self.ptr)
    }

    /// ⭐ 포커스를 빼앗지 않고 창을 앞으로 낸다(`orderFrontRegardless`).
    ///
    /// ⚠️ `makeKeyAndOrderFront:` 는 절대 부르지 않는다 — 그 순간 하위 앱의
    /// 키보드 포커스가 깨진다(§1 의 가장 중요한 제약).
    pub fn order_front_regardless(self) {
        imp::order_front_regardless(self.ptr);
    }

    /// 창을 화면에서 내린다(§3.1 숨김).
    pub fn order_out(self) {
        imp::order_out(self.ptr);
    }

    /// ⭐ 검증용 — 이 창이 키 윈도우가 **될 수 있는가**. `false` 여야 정상이다.
    ///
    /// 실기기 검증(`manual-verification.md`)이 "포커스를 안 뺏는다"를 눈으로만
    /// 보는 것이 아니라 **값으로도** 확인할 수 있게 노출한다.
    #[must_use]
    pub fn can_become_key(self) -> Option<bool> {
        imp::can_become_key(self.ptr)
    }

    /// ⭐ 검증용 — 실제로 걸린 window level. [`OVERLAY_WINDOW_LEVEL`] 이어야 한다.
    #[must_use]
    pub fn level(self) -> Option<isize> {
        imp::level(self.ptr)
    }

    /// ⭐ 검증용 — 실제로 걸린 collection behavior 비트.
    #[must_use]
    pub fn collection_behavior(self) -> Option<usize> {
        imp::collection_behavior(self.ptr)
    }
}

#[cfg(target_os = "macos")]
mod imp {
    use super::{OverlayWindowKind, Sharing};
    use objc2::runtime::{AnyClass, AnyObject, Bool, Sel};
    use objc2_app_kit::{
        NSScreenSaverWindowLevel, NSWindow, NSWindowCollectionBehavior, NSWindowSharingType,
        NSWindowStyleMask,
    };

    /// `*mut c_void`(Tauri `ns_window()`)를 `&NSWindow` 로 되살린다.
    ///
    /// # Safety
    /// `ptr` 는 살아 있는 `NSWindow` 인스턴스를 가리켜야 하고, 호출 동안
    /// 해제되지 않아야 한다. Tauri 의 `WebviewWindow::ns_window()` 가 돌려주는
    /// 포인터는 그 `WebviewWindow` 가 살아 있는 동안 유효하다.
    unsafe fn window<'a>(ptr: *mut core::ffi::c_void) -> Option<&'a NSWindow> {
        if ptr.is_null() {
            return None;
        }
        // SAFETY: 호출자가 보장한 대로 `ptr` 는 살아 있는 `NSWindow` 다.
        // `AnyObject` → `NSWindow` 재해석은 objc2 가 모든 클래스 참조에 쓰는
        // 공통 표현(첫 워드가 `isa`)에 기반한다.
        unsafe { (ptr as *const AnyObject as *const NSWindow).as_ref() }
    }

    /// ⭐ 키 윈도우가 되면 안 되는 창들의 등록부(포인터 값).
    ///
    /// 스위즐된 구현이 이 목록을 보고 자기 소관인지 판정한다. 목록에 없는
    /// 창은 원래 구현으로 그대로 넘어간다.
    static NON_ACTIVATING: std::sync::RwLock<Vec<usize>> = std::sync::RwLock::new(Vec::new());

    /// `class_replaceMethod` 가 돌려준 원래 구현. 셀렉터 2개를 각각 보관한다.
    static ORIGINAL_CAN_BECOME_KEY: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    static ORIGINAL_CAN_BECOME_MAIN: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

    /// 등록부에 있으면 `NO`, 없으면 원래 구현.
    fn dispatch(this: &AnyObject, sel: Sel, original: &std::sync::OnceLock<usize>) -> Bool {
        let key = std::ptr::from_ref(this) as usize;
        let ours = NON_ACTIVATING.read().is_ok_and(|list| list.contains(&key));
        if ours {
            return Bool::NO;
        }
        match original.get() {
            // SAFETY: `original` 에는 `class_replaceMethod` 가 돌려준, 같은
            // 셀렉터의 원래 IMP 만 들어간다. 그 셀렉터는 둘 다 인자 없는
            // `-(BOOL)` 이므로 아래 함수 타입이 정확히 그 시그니처다.
            Some(&imp) => unsafe {
                let f: extern "C-unwind" fn(&AnyObject, Sel) -> Bool =
                    core::mem::transmute::<usize, extern "C-unwind" fn(&AnyObject, Sel) -> Bool>(imp);
                f(this, sel)
            },
            // 원래 구현을 못 잡았다면(있을 수 없지만) 보수적으로 YES —
            // 남의 창을 우리가 망가뜨리지 않는 쪽으로 떨어진다.
            None => Bool::YES,
        }
    }

    extern "C-unwind" fn can_become_key_window(this: &AnyObject, sel: Sel) -> Bool {
        dispatch(this, sel, &ORIGINAL_CAN_BECOME_KEY)
    }

    extern "C-unwind" fn can_become_main_window(this: &AnyObject, sel: Sel) -> Bool {
        dispatch(this, sel, &ORIGINAL_CAN_BECOME_MAIN)
    }

    /// 셀렉터 하나를 클래스에 갈아 끼우고 원래 IMP 를 보관한다.
    fn swizzle(
        cls: &AnyClass,
        sel: Sel,
        replacement: extern "C-unwind" fn(&AnyObject, Sel) -> Bool,
        slot: &std::sync::OnceLock<usize>,
    ) -> bool {
        if slot.get().is_some() {
            return true; // 이미 했다.
        }
        let Some(method) = cls.instance_method(sel) else {
            tracing::error!("class has no {sel:?}; skipping swizzle");
            return false;
        };
        // SAFETY: `method` 는 방금 이 클래스에서 얻은 유효한 메서드다.
        let types = unsafe { objc2::ffi::method_getTypeEncoding(std::ptr::from_ref(method).cast()) };
        // SAFETY: `cls` 는 유효한 클래스이고, `replacement` 는 이 셀렉터와
        // 같은 시그니처(`-(BOOL)`, 인자 없음)를 갖는다. `types` 는 원래
        // 메서드에서 그대로 가져온 인코딩이라 시그니처가 일치한다.
        let previous = unsafe {
            objc2::ffi::class_replaceMethod(
                std::ptr::from_ref(cls).cast_mut(),
                sel,
                core::mem::transmute::<
                    extern "C-unwind" fn(&AnyObject, Sel) -> Bool,
                    unsafe extern "C-unwind" fn(),
                >(replacement),
                types,
            )
        };
        match previous {
            Some(imp) => {
                let _ = slot.set(imp as usize);
                true
            }
            None => {
                tracing::error!("swizzling {sel:?} did not return the original implementation");
                false
            }
        }
    }

    /// 이 창만 키 윈도우가 될 수 없게 만든다. 모듈 문서의 "표적 스위즐" 참조.
    pub(super) fn force_non_activating(ptr: *mut core::ffi::c_void) -> bool {
        // SAFETY: 널·클래스 검사는 `NsWindowHandle::from_tauri_ptr` 이 이미 했다.
        let Some(obj) = (unsafe { (ptr as *const AnyObject).as_ref() }) else {
            return false;
        };

        // ⭐ `-class`(= `[obj class]`)를 쓴다. KVO 로 스위즐된 객체에서
        // `object_getClass` 는 숨은 `NSKVONotifying_…` 클래스를 주지만,
        // `-class` 는 **원래 클래스**(`TaoWindow`)를 준다. 원래 클래스에
        // 갈아 끼우면 KVO 클래스도 그것을 상속하므로 양쪽 다 덮인다.
        let cls: &AnyClass = unsafe { objc2::msg_send![obj, class] };

        let a = swizzle(
            cls,
            objc2::sel!(canBecomeKeyWindow),
            can_become_key_window,
            &ORIGINAL_CAN_BECOME_KEY,
        );
        let b = swizzle(
            cls,
            objc2::sel!(canBecomeMainWindow),
            can_become_main_window,
            &ORIGINAL_CAN_BECOME_MAIN,
        );
        if !(a && b) {
            return false;
        }

        if let Ok(mut list) = NON_ACTIVATING.write() {
            let key = std::ptr::from_ref(obj) as usize;
            if !list.contains(&key) {
                list.push(key);
            }
            tracing::info!(
                class = %cls.name().to_string_lossy(),
                registered = list.len(),
                "registered this window as non-activating (targeted swizzle)"
            );
            true
        } else {
            false
        }
    }

    /// ⭐ 포인터가 정말 `NSWindow` 인스턴스인가 — `isKindOfClass:` 로 확인한다.
    pub(super) fn is_ns_window(ptr: *mut core::ffi::c_void) -> bool {
        // SAFETY: 널이 아님은 호출자([`NsWindowHandle::from_tauri_ptr`])가 이미
        // 확인했다. 여기서 읽는 것은 객체의 isa 뿐이고, 그 결과를 objc2 의
        // `downcast_ref` 가 `isKindOfClass:` 로 판정한다.
        let Some(obj) = (unsafe { (ptr as *const AnyObject).as_ref() }) else {
            return false;
        };
        if obj.downcast_ref::<NSWindow>().is_some() {
            return true;
        }
        tracing::error!(class = %obj.class().name().to_string_lossy(), "object is not an NSWindow");
        false
    }

    /// 네 가지 네이티브 요구를 한 번에 건다.
    pub(super) fn configure(ptr: *mut core::ffi::c_void, kind: OverlayWindowKind, sharing: Sharing) -> bool {
        // SAFETY: 호출자 계약 — `ptr` 는 Tauri 가 방금 내준 살아 있는 NSWindow.
        let Some(w) = (unsafe { window(ptr) }) else {
            tracing::error!("ns_window() returned null; failed to apply native overlay window settings");
            return false;
        };

        // ① ⭐ 활성화하지 않고 표시 — borderless 마스크가 `canBecomeKeyWindow`
        //    를 false 로 만든다(모듈 문서 참조). Tauri 가 남긴 마스크를
        //    **덮어쓴다** — `decorations: false` 만으로는 부족하다.
        w.setStyleMask(NSWindowStyleMask::Borderless);

        // ② ⭐ 전체화면 앱 위 표시. `always_on_top`(= NSFloatingWindowLevel)
        //    으로는 전체화면 Space 의 특수 레벨을 넘지 못한다.
        //    `NSScreenSaverWindowLevel`(1000) 은 메뉴 막대·Dock·전체화면 앱보다
        //    위다. `CGShieldingWindowLevel()` 은 더 위지만 화면 보호기/로그인
        //    창까지 덮어 과하다고 보고 채택하지 않았다.
        w.setLevel(NSScreenSaverWindowLevel);

        // ③ ⭐ collection behavior — 모든 Space 에 따라다니되(`CanJoinAllSpaces`),
        //    전체화면 Space 에도 보조 창으로 들어가고(`FullScreenAuxiliary`),
        //    Space 전환 애니메이션에 딸려 움직이지 않는다(`Stationary`).
        //    `IgnoresCycle` 은 ⌘` 창 순환 목록에서 빼기 위한 것이다.
        w.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::FullScreenAuxiliary
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );

        // ④ 스크린샷 포함 여부(§5 #12).
        let sharing_type = match sharing {
            Sharing::ReadOnly => NSWindowSharingType::ReadOnly,
            Sharing::None => NSWindowSharingType::None,
        };
        w.setSharingType(sharing_type);

        // 부수 설정 — Tauri 로도 되는 것이지만 창을 만든 뒤에 다시 확인해 둔다.
        w.setHasShadow(false);
        w.setOpaque(false);
        // 검색 바는 배경 아무 데나 잡고 끌 수 있어야 한다(§3.4).
        w.setMovableByWindowBackground(kind == OverlayWindowKind::SearchBar);
        // 하이라이트 창은 클릭을 통째로 통과시킨다.
        w.setIgnoresMouseEvents(kind == OverlayWindowKind::Highlight);

        true
    }

    /// ⭐ 포커스를 빼앗지 않고 창을 앞으로 낸다.
    ///
    /// `orderFront:` 와 달리 `orderFrontRegardless` 는 **앱이 활성 상태가
    /// 아니어도** 창을 올린다. Accessory 앱이라 애초에 활성화되지 않으므로
    /// 이것이 아니면 창이 그냥 안 보인다.
    ///
    /// ⚠️ `makeKeyAndOrderFront:` 는 절대 부르지 않는다 — 그 순간 하위 앱의
    /// 키보드 포커스가 깨진다(§1 의 가장 중요한 제약).
    pub(super) fn order_front_regardless(ptr: *mut core::ffi::c_void) {
        // SAFETY: 호출자 계약.
        if let Some(w) = unsafe { window(ptr) } {
            w.orderFrontRegardless();
        }
    }

    /// 창을 화면에서 내린다(§3.1 숨김).
    pub(super) fn order_out(ptr: *mut core::ffi::c_void) {
        // SAFETY: 호출자 계약.
        if let Some(w) = unsafe { window(ptr) } {
            w.orderOut(None);
        }
    }

    /// ⭐ 검증용 — 이 창이 키 윈도우가 **될 수 있는가**. `false` 여야 정상이다.
    ///
    /// 실기기 검증(`manual-verification.md`)이 "포커스를 안 뺏는다"를 눈으로
    /// 보는 것 말고 **숫자로도** 확인할 수 있게 노출한다.
    pub(super) fn can_become_key(ptr: *mut core::ffi::c_void) -> Option<bool> {
        // SAFETY: 호출자 계약.
        let w = unsafe { window(ptr) }?;
        Some(w.canBecomeKeyWindow())
    }

    /// ⭐ 검증용 — 실제로 걸린 window level. `NSScreenSaverWindowLevel`(1000)
    /// 이어야 정상이다.
    pub(super) fn level(ptr: *mut core::ffi::c_void) -> Option<isize> {
        // SAFETY: 호출자 계약.
        let w = unsafe { window(ptr) }?;
        Some(w.level())
    }

    /// ⭐ 검증용 — 실제로 걸린 collection behavior 비트.
    pub(super) fn collection_behavior(ptr: *mut core::ffi::c_void) -> Option<usize> {
        // SAFETY: 호출자 계약.
        let w = unsafe { window(ptr) }?;
        Some(w.collectionBehavior().0)
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    use super::{OverlayWindowKind, Sharing};

    pub(super) fn configure(_p: *mut core::ffi::c_void, _k: OverlayWindowKind, _s: Sharing) -> bool {
        false
    }
    pub(super) fn order_front_regardless(_p: *mut core::ffi::c_void) {}
    pub(super) fn order_out(_p: *mut core::ffi::c_void) {}
    pub(super) fn can_become_key(_p: *mut core::ffi::c_void) -> Option<bool> {
        None
    }
    pub(super) fn level(_p: *mut core::ffi::c_void) -> Option<isize> {
        None
    }
    pub(super) fn collection_behavior(_p: *mut core::ffi::c_void) -> Option<usize> {
        None
    }
}


/// `NSScreenSaverWindowLevel` — [`configure`] 가 거는 값. 검증이 대조할 상수.
pub const OVERLAY_WINDOW_LEVEL: isize = 1000;
