//! `CGEvent` 조작 — 안전 래퍼.
//!
//! `CgEventRef` 는 탭 콜백이 넘겨주는 이벤트를 **빌린** 참조로만 다루고,
//! `SyntheticEvent` 는 이 크레이트가 새로 합성해 **소유**하는 이벤트다.
//!
//! ⭐ 자기 합성 이벤트 마커(§5 엣지 12): `CGEventSetIntegerValueField` 로
//! `kCGEventSourceUserData`(필드 번호 42) 에 [`ULTRAKEY_MAGIC`] 을 심어 두고,
//! 콜백 최초 진입 시 이 값을 확인해 자기 자신이 합성한 이벤트를 즉시 통과시킨다
//! (`event_tap.rs` 의 콜백 0-a 단계). 이 필드는 "이벤트를 만든 애플리케이션이
//! 자유롭게 쓰라"고 문서화된 사용자 정의 슬롯이라 다른 프로세스와 충돌할
//! 위험이 낮다.

/// 자기 합성 이벤트 식별 매직 넘버.
///
/// 값 자체에 의미는 없다 — 우연히 다른 프로세스가 같은 값을 쓸 확률을 낮추기
/// 위해 임의의 32비트 상수를 골랐다. `i64` 필드에 저장하지만 우리가 쓰는 값은
/// 항상 이 범위 안이므로 부호 문제가 없다.
pub const ULTRAKEY_MAGIC: i64 = 0x554B_4559; // "UKEY" 를 ASCII 코드로 늘어놓은 값(가독성용 관례일 뿐, 근거는 아님).

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::ULTRAKEY_MAGIC;
    use crate::event_tap::TapProxy;
    use core::ptr::NonNull;
    use objc2_core_foundation::CFRetained;
    use objc2_core_graphics::{
        CGEvent, CGEventField, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventType,
    };
    use std::sync::OnceLock;
    use ultrakey_core::flags::EventFlags;
    use ultrakey_core::keycode::KeyCode;

    /// `CGEventSourceCreate(kCGEventSourceStateHIDSystemState)` 로 만든 이벤트
    /// 소스를 프로세스 생애주기 동안 하나만 만들어 재사용한다(스펙 지시).
    ///
    /// SAFETY: `CGEventSource` 는 불투명 CF 객체로, Apple 문서상 참조 카운트
    /// 증감(`CFRetain`/`CFRelease`)이 원자적이라 여러 스레드에서 안전하게
    /// 공유할 수 있다. 우리는 생성 이후 이 객체의 프로퍼티를 바꾸지 않고
    /// 읽기 전용으로만 쓰므로(이벤트 합성 시 소스로 넘기기만 함) `Send`/`Sync`
    /// 를 수동으로 부여해도 데이터 경합이 생기지 않는다.
    struct SharedEventSource(CFRetained<CGEventSource>);
    // SAFETY: 위 문서 참조.
    unsafe impl Send for SharedEventSource {}
    // SAFETY: 위 문서 참조.
    unsafe impl Sync for SharedEventSource {}

    static EVENT_SOURCE: OnceLock<Option<SharedEventSource>> = OnceLock::new();

    fn shared_event_source() -> Option<&'static CGEventSource> {
        EVENT_SOURCE
            .get_or_init(|| {
                CGEventSource::new(CGEventSourceStateID::HIDSystemState).map(SharedEventSource)
            })
            .as_ref()
            .map(|s| &*s.0)
    }

    /// SAFETY 불변식: `mark_synthetic` 은 우리가 방금 만든(따라서 유효함이
    /// 보장된) `CGEvent` 에만 호출한다.
    fn mark_synthetic(event: &CGEvent) {
        CGEvent::set_integer_value_field(
            Some(event),
            CGEventField::EventSourceUserData,
            ULTRAKEY_MAGIC,
        );
    }

    /// 탭 콜백이 넘겨주는 `CGEventRef` 를 빌려서 다루는 래퍼.
    ///
    /// 콜백 호출 동안에만 유효하다 — 콜백이 리턴한 뒤에는 macOS 가 이 포인터를
    /// 해제하거나 재사용할 수 있으므로 절대 콜백 밖으로 들고 나가면 안 된다.
    pub struct CgEventRef(NonNull<CGEvent>);

    impl CgEventRef {
        /// # Safety
        /// `ptr` 은 현재 실행 중인 탭 콜백이 넘겨받은, 아직 유효한 `CGEventRef`
        /// 여야 한다. 이 함수는 `event_tap.rs` 의 트램폴린에서만 호출된다.
        pub(crate) unsafe fn from_raw(ptr: NonNull<CGEvent>) -> Self {
            Self(ptr)
        }

        pub(crate) fn as_raw(&self) -> NonNull<CGEvent> {
            self.0
        }

        pub fn keycode(&self) -> KeyCode {
            // SAFETY: `self.0` 은 콜백이 보증하는 유효한 CGEvent 포인터다.
            let v = CGEvent::integer_value_field(
                Some(unsafe { self.0.as_ref() }),
                CGEventField::KeyboardEventKeycode,
            );
            KeyCode(v as u16)
        }

        pub fn flags(&self) -> EventFlags {
            // SAFETY: 위와 동일.
            let f = CGEvent::flags(Some(unsafe { self.0.as_ref() }));
            EventFlags(f.0)
        }

        pub fn set_flags(&mut self, f: EventFlags) {
            // SAFETY: 위와 동일. `CGEventSetFlags` 는 이벤트를 제자리에서 변경한다.
            CGEvent::set_flags(Some(unsafe { self.0.as_ref() }), CGEventFlags(f.0));
        }

        pub fn is_autorepeat(&self) -> bool {
            // SAFETY: 위와 동일.
            CGEvent::integer_value_field(
                Some(unsafe { self.0.as_ref() }),
                CGEventField::KeyboardEventAutorepeat,
            ) != 0
        }

        pub fn is_ultrakey_synthetic(&self) -> bool {
            // SAFETY: 위와 동일.
            CGEvent::integer_value_field(
                Some(unsafe { self.0.as_ref() }),
                CGEventField::EventSourceUserData,
            ) == ULTRAKEY_MAGIC
        }
    }

    /// 이 크레이트가 새로 합성해 소유하는 `CGEvent`.
    pub struct SyntheticEvent(CFRetained<CGEvent>);

    impl SyntheticEvent {
        pub fn keyboard(keycode: KeyCode, down: bool, flags: EventFlags) -> Option<Self> {
            let source = shared_event_source();
            let ev = CGEvent::new_keyboard_event(source, keycode.0, down)?;
            CGEvent::set_flags(Some(&ev), CGEventFlags(flags.0));
            mark_synthetic(&ev);
            Some(Self(ev))
        }

        pub fn flags_changed(keycode: KeyCode, flags: EventFlags) -> Option<Self> {
            let source = shared_event_source();
            // flagsChanged 합성은 표준 기법을 쓴다: keyDown 이벤트를 만든 뒤
            // 타입만 FlagsChanged 로 바꾸고 원하는 flags 를 얹는다.
            let ev = CGEvent::new_keyboard_event(source, keycode.0, true)?;
            CGEvent::set_type(Some(&ev), CGEventType::FlagsChanged);
            CGEvent::set_flags(Some(&ev), CGEventFlags(flags.0));
            mark_synthetic(&ev);
            Some(Self(ev))
        }

        /// 탭 콜백 **안**에서 방출할 때 쓴다. `CGEventTapPostEvent` 는 우리
        /// 탭보다 뒤에 이벤트를 주입하므로, 우리 탭이 이 이벤트를 다시
        /// 가로채는 재진입 자체가 없다 — 따라서 마커가 없어도 무한 루프가
        /// 생기지 않지만, 다른 리매퍼(§5 엣지 12)와의 공존을 위해 항상 마커를
        /// 남긴다.
        pub fn post_to_tap(self, proxy: TapProxy) {
            // SAFETY: `proxy` 는 현재 실행 중인 탭 콜백에서 얻은 유효한
            // `CGEventTapProxy` 다(콜백 인자로만 만들어짐, `event_tap.rs` 참조).
            unsafe { CGEvent::tap_post_event(proxy.0, Some(&self.0)) }
        }

        /// 탭 콜백 **밖**(타이머 등 다른 스레드/시점)에서 방출할 때 쓴다.
        /// 이 경로로 나간 이벤트는 우리 탭에 다시 도착해 재진입을 일으킬 수
        /// 있으므로, `is_ultrakey_synthetic` 마커가 **유일한 방어선**이다
        /// (`event_tap.rs` 콜백 0-a 단계가 이 마커를 확인한다).
        pub fn post(self) {
            use objc2_core_graphics::CGEventTapLocation;
            CGEvent::post(CGEventTapLocation::SessionEventTap, Some(&self.0));
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{CgEventRef, SyntheticEvent};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::ULTRAKEY_MAGIC;
    use ultrakey_core::flags::EventFlags;
    use ultrakey_core::keycode::KeyCode;

    /// 비-macOS 스텁 — 컴파일만 되고 실제로 만들어질 일이 없다.
    pub struct CgEventRef(core::convert::Infallible);

    impl CgEventRef {
        pub fn keycode(&self) -> KeyCode {
            match self.0 {}
        }
        pub fn flags(&self) -> EventFlags {
            match self.0 {}
        }
        pub fn set_flags(&mut self, _f: EventFlags) {
            match self.0 {}
        }
        pub fn is_autorepeat(&self) -> bool {
            match self.0 {}
        }
        pub fn is_ultrakey_synthetic(&self) -> bool {
            match self.0 {}
        }
    }

    pub struct SyntheticEvent(core::convert::Infallible);

    impl SyntheticEvent {
        pub fn keyboard(_keycode: KeyCode, _down: bool, _flags: EventFlags) -> Option<Self> {
            None
        }
        pub fn flags_changed(_keycode: KeyCode, _flags: EventFlags) -> Option<Self> {
            None
        }
        pub fn post_to_tap(self, _proxy: crate::event_tap::TapProxy) {
            match self.0 {}
        }
        pub fn post(self) {
            match self.0 {}
        }
    }

    // `ULTRAKEY_MAGIC` 을 참조해 미사용 경고를 막는다(스텁에서는 실제로 쓰이지 않음).
    #[allow(dead_code)]
    const _: i64 = ULTRAKEY_MAGIC;
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{CgEventRef, SyntheticEvent};
