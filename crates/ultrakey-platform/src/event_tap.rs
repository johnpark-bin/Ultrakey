//! 경로 A — `CGEventTap` 안전 래퍼.
//!
//! `CGEventType` → `ultrakey_core::event::EventKind` 변환은 [`map_event_kind`]
//! 하나로 격리해 둔다 — `EventKind` 가 `CGEventType` 을 1:1 로 미러링하므로
//! 단순 매핑이지만, 나중에 어느 한쪽이 바뀌어도 고칠 지점이 한 곳이다.
//!
//! ⭐ **탭 비활성화(`kCGEventTapDisabledByTimeout`/`ByUserInput`) 통지는
//! 트램폴린이 콜백 호출과 무관하게 항상 먼저 `CGEventTapEnable` 로
//! 복구한다.** `key-remapping-engine.md` §8 수용 기준이 "예외 없이" 재활성화를
//! 요구하므로, 이 재활성화 자체를 엔진 콜백의 반환값에 의존시키지 않는다 —
//! 콜백을 등록하는 시점에는 아직 `EventTap` 자신이 만들어지지 않아 엔진이
//! 자기 참조를 캡처할 수 없다는 부트스트랩 문제도 함께 피한다. 다만
//! `EventKind::TapDisabledByTimeout`/`TapDisabledByUserInput` 변형이 존재하는
//! 것은 엔진의 탭 생명주기 FSM(`ultrakey-engine::tap`)이 이 사건을 관찰해야
//! 한다는 뜻이므로, 재활성화 뒤에도 평소처럼 콜백을 호출해 알린다 — 다만
//! 콜백의 반환값(`TapAction`)은 무시하고 항상 원본 이벤트를 그대로 돌려준다
//! (`ultrakey-engine` 의 별도 1Hz 워치독 폴링은 여전히 `is_enabled()` 로 이
//! 크레이트 밖에서 독립적으로 동작한다).

use crate::event::CgEventRef;
use ultrakey_core::event::EventKind;

/// 탭 생성 실패 사유.
///
/// 구분 기준(§3-a): 권한이 아예 확인되지 않은 상태의 실패는 F-11 온보딩이
/// 흡수해야 하고, 권한이 확인된 상태에서의 실패는 치명적이다. 이 크레이트는
/// 실패 시점에 [`crate::accessibility::is_process_trusted`] 를 함께 확인해
/// 두 경우를 기계적으로 나눌 뿐, 그 이후의 대응(재시도/프로세스 종료)은
/// 호출자(엔진) 소관이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum TapCreateError {
    #[error("Accessibility 권한이 확인되지 않아 이벤트 탭을 만들 수 없다")]
    NotTrusted,
    #[error("권한이 확인된 상태에서도 CGEventTapCreate 자체가 실패했다")]
    CreateFailed,
}

/// 탭 콜백이 이벤트를 어떻게 처리했는지.
pub enum TapAction {
    /// 원본 이벤트를 그대로 통과시킨다(아무 것도 바꾸지 않음).
    Pass,
    /// `CgEventRef` 의 필드(예: flags)를 제자리에서 바꾼 뒤 그 결과를 내보낸다.
    Replace,
    /// 이벤트를 소비한다 — 대상 앱에 원본이 전달되지 않는다.
    Consume,
}

/// 물리 키/마우스 이벤트가 도착할 때마다 호출되는 콜백.
///
/// 콜백은 절대 블로킹하면 안 된다(`key-remapping-engine.md` §3-a "콜백 금지 사항").
pub type TapCallback =
    Box<dyn FnMut(TapProxy, EventKind, &mut CgEventRef) -> TapAction + Send>;

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::{TapAction, TapCallback, TapCreateError};
    use crate::event::{CgEventRef, ULTRAKEY_MAGIC};
    use core::cell::RefCell;
    use core::ffi::c_void;
    use core::ptr::NonNull;
    use objc2_core_foundation::{kCFRunLoopCommonModes, CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource};
    use objc2_core_graphics::{
        CGEvent, CGEventField, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
        CGEventTapProxy, CGEventType,
    };
    use ultrakey_core::event::EventKind;

    /// 콜백 하나가 호출될 때마다 주어지는, 탭 자신을 가리키는 불투명 핸들.
    ///
    /// 이 값으로 할 수 있는 유일한 일은 [`crate::event::SyntheticEvent::post_to_tap`]
    /// 로 콜백 **안**에서 이벤트를 방출하는 것이다.
    #[derive(Clone, Copy)]
    pub struct TapProxy(pub(crate) CGEventTapProxy);

    /// 트램폴린이 실행되는 동안 붙잡고 있는 상태.
    ///
    /// `mach_port` 는 [`EventTap`] 이 갖는 것과 별도로 하나 더 retain 해 둔
    /// 사본이다 — 탭 비활성화 통지를 자체 복구(`CGEventTapEnable`)할 때
    /// 트램폴린이 `EventTap` 구조체 전체에 접근할 필요 없이 이 값만으로
    /// 충분하게 하기 위함이다.
    struct CallbackContext {
        callback: TapCallback,
        mach_port: RefCell<Option<CFRetained<CFMachPort>>>,
    }

    fn build_event_mask() -> u64 {
        // keyDown/keyUp/flagsChanged 는 항상 필요하다(§3-b 전 계층의 입력).
        // 마우스 이벤트는 hyperkey.md §3.3 의 Click/Drag/Move/Scroll 전부를
        // 커버해야 하므로(사용자가 어느 것을 켤지는 규칙 테이블이 나중에
        // 결정한다 — 탭 자체는 항상 전부 받아 둔다) 관련 kCGEventType 을 모두
        // 넣는다.
        let types: &[CGEventType] = &[
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::FlagsChanged,
            // Click
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            // Drag
            CGEventType::LeftMouseDragged,
            CGEventType::RightMouseDragged,
            CGEventType::OtherMouseDragged,
            // Move
            CGEventType::MouseMoved,
            // Scroll
            CGEventType::ScrollWheel,
        ];
        types.iter().fold(0u64, |mask, t| mask | (1u64 << (t.0 as u64)))
    }

    /// `CGEventType` → `ultrakey_core::event::EventKind` 변환. 실제
    /// `EventKind` 정의(`ultrakey-core/src/event.rs`)가 `CGEventType` 을
    /// 그대로 1:1 미러링하므로 단순 매핑이다.
    fn map_event_kind(t: CGEventType) -> Option<EventKind> {
        match t {
            CGEventType::KeyDown => Some(EventKind::KeyDown),
            CGEventType::KeyUp => Some(EventKind::KeyUp),
            CGEventType::FlagsChanged => Some(EventKind::FlagsChanged),
            CGEventType::LeftMouseDown => Some(EventKind::LeftMouseDown),
            CGEventType::LeftMouseUp => Some(EventKind::LeftMouseUp),
            CGEventType::RightMouseDown => Some(EventKind::RightMouseDown),
            CGEventType::RightMouseUp => Some(EventKind::RightMouseUp),
            CGEventType::OtherMouseDown => Some(EventKind::OtherMouseDown),
            CGEventType::OtherMouseUp => Some(EventKind::OtherMouseUp),
            CGEventType::LeftMouseDragged => Some(EventKind::LeftMouseDragged),
            CGEventType::RightMouseDragged => Some(EventKind::RightMouseDragged),
            CGEventType::OtherMouseDragged => Some(EventKind::OtherMouseDragged),
            CGEventType::MouseMoved => Some(EventKind::MouseMoved),
            CGEventType::ScrollWheel => Some(EventKind::ScrollWheel),
            CGEventType::TapDisabledByTimeout => Some(EventKind::TapDisabledByTimeout),
            CGEventType::TapDisabledByUserInput => Some(EventKind::TapDisabledByUserInput),
            _ => None,
        }
    }

    /// # Safety
    /// macOS 가 `CGEventTapCreate` 에 등록된 콜백으로서만 이 함수를 호출한다.
    /// `user_info` 는 항상 `create()` 가 넘긴 `*mut CallbackContext` 다.
    unsafe extern "C-unwind" fn trampoline(
        proxy: CGEventTapProxy,
        event_type: CGEventType,
        event: NonNull<CGEvent>,
        user_info: *mut c_void,
    ) -> *mut CGEvent {
        // SAFETY: `user_info` 는 `create()` 가 넘긴, 탭이 살아있는 동안
        // 항상 유효한 `CallbackContext` 다(탭이 죽을 때는 `CFMachPortInvalidate`
        // 가 먼저 실행되어 이 트램폴린 자체가 다시 호출되지 않는다).
        let ctx = unsafe { &mut *(user_info as *mut CallbackContext) };

        // ⭐ 탭 비활성화 통지 — 판정 없이 **즉시** 자체 복구한다(§8 수용 기준:
        // "예외 없이 CGEventTapEnable 로 재활성화를 시도한다"). 엔진이 무엇을
        // 하든 이 재활성화 자체는 항상 일어나도록 콜백 호출과 분리해 둔다.
        // 다만 `ultrakey-core::event::EventKind` 가 이 두 통지를 위한 변형을
        // 갖고 있으므로(엔진의 탭 생명주기 FSM 이 이를 관찰해야 한다는 뜻),
        // 재활성화 뒤에도 평소처럼 콜백을 호출해 엔진에 알린다 — 다만
        // 반환값(TapAction)은 무시하고 항상 원본 이벤트를 그대로 돌려준다.
        if event_type == CGEventType::TapDisabledByTimeout
            || event_type == CGEventType::TapDisabledByUserInput
        {
            if let Some(port) = ctx.mach_port.borrow().as_ref() {
                CGEvent::tap_enable(port, true);
            }
            if let Some(kind) = map_event_kind(event_type) {
                // SAFETY: `event` 는 이 콜백 호출 동안에만 유효하다는 계약을
                // `CgEventRef` 가 그대로 물려받는다.
                let mut event_ref = unsafe { CgEventRef::from_raw(event) };
                let _ = (ctx.callback)(TapProxy(proxy), kind, &mut event_ref);
            }
            return event.as_ptr();
        }

        // 0-a: 자기 합성 이벤트 마커 확인 — 무한 루프 방지(§5 엣지 12).
        // SAFETY: `event` 는 콜백 인자로 받은, 이 호출 동안 유효한 이벤트다.
        let is_synthetic = CGEvent::integer_value_field(
            Some(unsafe { event.as_ref() }),
            CGEventField::EventSourceUserData,
        ) == ULTRAKEY_MAGIC;
        if is_synthetic {
            return event.as_ptr();
        }

        let Some(kind) = map_event_kind(event_type) else {
            // 마스크에 넣지 않은 타입은 이론상 오지 않지만, 방어적으로 통과시킨다.
            return event.as_ptr();
        };

        // SAFETY: `event` 는 이 콜백 호출 동안에만 유효하다는 계약을
        // `CgEventRef` 가 그대로 물려받는다 — 콜백 밖으로 반출되지 않는다.
        let mut event_ref = unsafe { CgEventRef::from_raw(event) };
        let action = (ctx.callback)(TapProxy(proxy), kind, &mut event_ref);
        match action {
            TapAction::Pass | TapAction::Replace => event_ref.as_raw().as_ptr(),
            TapAction::Consume => core::ptr::null_mut(),
        }
    }

    /// 경로 A(`CGEventTap`) 인스턴스.
    pub struct EventTap {
        mach_port: Option<CFRetained<CFMachPort>>,
        run_loop_source: Option<CFRetained<CFRunLoopSource>>,
        installed_run_loop: Option<CFRetained<CFRunLoop>>,
        ctx: Option<NonNull<CallbackContext>>,
    }

    // SAFETY: `EventTap` 은 생성된 스레드(전용 탭 스레드) 안에서만 만들어지고
    // 쓰이고 버려지는 것이 architecture.md §2.1 배치의 전제다. 이 타입 자체는
    // 스레드 경계를 넘지 않으므로 `Send`/`Sync` 를 부여하지 않는다(기본값 유지).

    impl EventTap {
        /// `CGEventTapCreate` 로 탭을 만든다. 마스크는 keyDown/keyUp/
        /// flagsChanged 와 hyperkey.md §3.3 의 마우스 이벤트 전부로 고정된다.
        pub fn create(callback: TapCallback) -> Result<Self, TapCreateError> {
            let boxed = Box::new(CallbackContext {
                callback,
                mach_port: RefCell::new(None),
            });
            let ctx_ptr = Box::into_raw(boxed);

            let mask = build_event_mask();
            // SAFETY: `trampoline` 은 `CGEventTapCallBack` 시그니처와 정확히
            // 일치하고, `ctx_ptr` 은 방금 `Box::into_raw` 로 만든 유효한
            // 포인터다. 탭이 아직 어떤 런루프에도 등록되지 않았으므로 이
            // 시점에는 콜백이 호출될 수 없다.
            let mach_port = unsafe {
                CGEvent::tap_create(
                    CGEventTapLocation::SessionEventTap,
                    CGEventTapPlacement::HeadInsertEventTap,
                    CGEventTapOptions::Default,
                    mask,
                    Some(trampoline),
                    ctx_ptr as *mut c_void,
                )
            };

            let Some(mach_port) = mach_port else {
                // SAFETY: `ctx_ptr` 은 아직 아무도 공유하지 않은, 우리가 만든
                // Box 포인터다 — 실패 경로이므로 여기서 되찾아 드롭한다.
                drop(unsafe { Box::from_raw(ctx_ptr) });
                return Err(if crate::accessibility::is_process_trusted() {
                    TapCreateError::CreateFailed
                } else {
                    TapCreateError::NotTrusted
                });
            };

            let Some(run_loop_source) = CFMachPort::new_run_loop_source(None, Some(&mach_port), 0)
            else {
                mach_port.invalidate();
                // SAFETY: 위와 동일 — 아직 런루프에 등록되지 않아 콜백이
                // 호출될 수 없는 상태에서 되찾아 드롭한다.
                drop(unsafe { Box::from_raw(ctx_ptr) });
                return Err(TapCreateError::CreateFailed);
            };

            // SAFETY: `ctx_ptr` 은 여전히 우리가 배타 소유한 유효한 포인터다.
            unsafe { (*ctx_ptr).mach_port.replace(Some(mach_port.clone())) };

            Ok(Self {
                mach_port: Some(mach_port),
                run_loop_source: Some(run_loop_source),
                installed_run_loop: None,
                // SAFETY: `Box::into_raw` 는 항상 널이 아닌 포인터를 반환한다.
                ctx: Some(unsafe { NonNull::new_unchecked(ctx_ptr) }),
            })
        }

        /// 현재 스레드의 런루프에 `kCFRunLoopCommonModes` 로 등록한다
        /// (`platform-constraints.md` §5.2 — 트래킹 모드 문제 회피).
        pub fn add_to_current_runloop(&mut self) {
            let Some(rl) = CFRunLoop::current() else {
                return;
            };
            if let Some(src) = self.run_loop_source.as_ref() {
                // SAFETY: `kCFRunLoopCommonModes` 는 CoreFoundation 이 항상
                // 정의하는 정적 심볼을 읽는 것뿐이다.
                rl.add_source(Some(src), unsafe { kCFRunLoopCommonModes });
            }
            self.installed_run_loop = Some(rl);
        }

        /// `CGEventTapEnable`.
        pub fn enable(&self, enable: bool) {
            if let Some(port) = self.mach_port.as_ref() {
                CGEvent::tap_enable(port, enable);
            }
        }

        /// `CGEventTapIsEnabled`.
        pub fn is_enabled(&self) -> bool {
            self.mach_port
                .as_ref()
                .map(|p| CGEvent::tap_is_enabled(p))
                .unwrap_or(false)
        }

        /// ⭐ **엔진 크레이트 추가분(2026-08-30, 이슈 #5 M1 구현)** — 이 탭의
        /// mach port 생존 여부를 **다른 스레드에서** 폴링할 수 있는 손잡이를
        /// 만든다. `docs/dev/architecture.md` §2.1 은 워치독을 탭 스레드가
        /// 아닌 독립 스레드에 배치한다 — "탭 런루프가 막혀도 감지할 수
        /// 있어야 하기 때문"이다. 그런데 `EventTap` 자신은 의도적으로
        /// `Send`/`Sync` 를 갖지 않는다(탭 전용 스레드만 쓴다는 설계, 위
        /// 구조체 주석 참고) — 워치독이 `EventTap` 을 직접 참조할 수 없다는
        /// 뜻이다. `CGEventTapIsEnabled` 자체는 mach port 플래그를 읽기만
        /// 하는 순수 조회라 어느 스레드에서 불러도 안전하므로, 그 조회 하나만
        /// 다른 스레드에 노출하는 별도 손잡이를 둔다. 반환값이 `None` 이면
        /// 탭이 아직 만들어지지 않은 상태다.
        pub fn health_probe(&self) -> Option<TapHealthProbe> {
            self.mach_port.as_ref().map(|p| TapHealthProbe(p.clone()))
        }
    }

    /// [`EventTap::health_probe`] 가 반환하는, 다른 스레드에서 안전하게 폴링할 수 있는
    /// 탭 생존 확인 손잡이. 이 타입이 노출하는 연산은 [`TapHealthProbe::is_enabled`]
    /// 하나뿐이다.
    #[derive(Clone)]
    pub struct TapHealthProbe(CFRetained<CFMachPort>);

    // SAFETY: `CFMachPort` 의 참조 카운트 증감(`CFRetain`/`CFRelease`)은 CF 문서상
    // 원자적이고, `CGEventTapIsEnabled` 는 mach port 의 플래그를 읽기만 하는 순수
    // 조회이며 부작용이 없다(Apple 문서). 이 타입이 노출하는 유일한 연산이 바로 그
    // 조회이므로, 이 값을 다른 스레드와 공유해도 데이터 경합이 생기지 않는다
    // (`runloop.rs` 의 `SendSyncCf` 와 동일한 근거).
    unsafe impl Send for TapHealthProbe {}
    // SAFETY: 위와 동일 — 여러 스레드가 동시에 `is_enabled()` 를 호출해도 각자
    // 독립적인 읽기 전용 조회라 경합이 없다.
    unsafe impl Sync for TapHealthProbe {}

    impl TapHealthProbe {
        /// `CGEventTapIsEnabled` — 워치독 스레드가 1초 주기로 폴링하는 값
        /// (`key-remapping-engine.md` §3-a `Active` 행).
        pub fn is_enabled(&self) -> bool {
            CGEvent::tap_is_enabled(&self.0)
        }
    }

    impl Drop for EventTap {
        fn drop(&mut self) {
            // architecture.md §2.4 가 못박은 순서:
            // 런루프 소스 제거 → CFMachPortInvalidate → 참조 해제 → Box::from_raw.
            //
            // 1. 런루프 소스 제거.
            if let Some(rl) = self.installed_run_loop.take() {
                if let Some(src) = self.run_loop_source.as_ref() {
                    // SAFETY: 위와 동일한 정적 심볼 읽기.
                    rl.remove_source(Some(src), unsafe { kCFRunLoopCommonModes });
                }
            }
            // 2. CFMachPortInvalidate — 이 시점 이후로는 트램폴린이 다시
            //    호출되지 않는다는 것이 보장된다.
            if let Some(port) = self.mach_port.as_ref() {
                port.invalidate();
            }
            // 3. 참조 해제 — CFRetained 를 지금 이 순서로 명시적으로 drop 한다.
            self.run_loop_source.take();
            self.mach_port.take();
            // 4. 마지막으로 콜백 컨텍스트 Box 를 회수한다.
            if let Some(ctx) = self.ctx.take() {
                // SAFETY: 2단계로 재진입이 불가능해진 뒤이므로 이제 이
                // `Box` 를 유일한 소유자로서 안전하게 회수할 수 있다. `ctx`
                // 는 `create()` 에서 `Box::into_raw` 로 만든 뒤 이 구조체가
                // 배타 소유해 온 포인터다.
                drop(unsafe { Box::from_raw(ctx.as_ptr()) });
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{EventTap, TapHealthProbe, TapProxy};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::{TapCallback, TapCreateError};

    /// 콜백 하나가 호출될 때마다 주어지는, 탭 자신을 가리키는 불투명 핸들
    /// (비-macOS 스텁 — 실제로 만들어질 일이 없다).
    #[derive(Clone, Copy)]
    pub struct TapProxy(core::marker::PhantomData<()>);

    pub struct EventTap(core::convert::Infallible);

    impl EventTap {
        pub fn create(_callback: TapCallback) -> Result<Self, TapCreateError> {
            Err(TapCreateError::CreateFailed)
        }
        pub fn add_to_current_runloop(&mut self) {
            match self.0 {}
        }
        pub fn enable(&self, _enable: bool) {
            match self.0 {}
        }
        pub fn is_enabled(&self) -> bool {
            match self.0 {}
        }
        pub fn health_probe(&self) -> Option<TapHealthProbe> {
            match self.0 {}
        }
    }

    /// macOS 구현의 [`super::macos_impl::TapHealthProbe`] 와 짝을 맞추는 스텁 —
    /// `EventTap` 자체가 만들어질 일이 없으므로 이 값도 만들어질 일이 없다.
    #[derive(Clone)]
    pub struct TapHealthProbe(core::marker::PhantomData<()>);

    impl TapHealthProbe {
        pub fn is_enabled(&self) -> bool {
            false
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{EventTap, TapHealthProbe, TapProxy};
