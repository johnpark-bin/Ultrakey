//! F-04 — 마우스 클릭·커서 워프 실행 프리미티브(`CGEvent` 마우스 합성).
//!
//! 명세: `docs/spec/seek-click-execution.md` §6 이 **실측**(번들 심볼)으로
//! 확정한 API 를 `objc2-core-graphics` 0.3.2 의 안전 래퍼로 호출한다. 이
//! 모듈은 **unsafe FFI 표면만** 다룬다 — 모드 해석·지점·계획·경로 판정은
//! `ultrakey-click` 크레이트의 순수 로직, 오케스트레이션(Focus 전환·AX press·
//! 폴백 순서)은 `apps/ultrakey-app::click_executor` 이 맡는다.
//!
//! ⭐ **모든 합성 마우스 이벤트에 자기 합성 마커(`ULTRAKEY_MAGIC`)를 심는다**
//! (결정 D6, 이슈 #44) — `event_tap.rs` 콜백 0-a 단계가 이 마커를 보고 즉시
//! 통과시켜, 합성 클릭이 중재(계층 1~5)에 다시 도달해 이중 처리·무한 루프를
//! 만들 가능성이 없다(§3.8). 마커 메커니즘 자체는 키보드 합성
//! (`SyntheticEvent`)이 이미 실측 검증한 것이고, 마우스 이벤트 적용은 같은
//! 필드·검사 경로라 위험이 낮다(계획 초안 D6).
//!
//! ⭐ 이벤트 소스는 `event.rs` 의 **공유 소스(프로세스당 하나)** 를
//! 재사용한다(스펙 지시).
//!
//! ⚠️ 이 모듈의 함수는 전부 **워커 스레드**에서 호출된다(seek.rs 문서) —
//! `NSScreen` 기반이 아니라 `CG` 계열 API 만 쓰므로 스레드 구속이 없다
//! (`screens::mouse_location` 은 `NSEvent` 기반이라 메인 스레드 전용 —
//! 계획 초안 A5).

use ultrakey_core::flags::EventFlags;

/// 활성 디스플레이 하나의 전역 경계(포인트, 좌상단 원점) — 화면 밖 판정의
/// 입력(엣지 7). `CG` 기반이라 워커 스레드에서 안전하다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayBounds {
    /// 좌상단 x (포인트).
    pub x: f64,
    /// 좌상단 y (포인트).
    pub y: f64,
    /// 폭 (포인트).
    pub width: f64,
    /// 높이 (포인트).
    pub height: f64,
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::{DisplayBounds, EventFlags};
    use crate::event::{mark_synthetic, shared_event_source};
    use objc2_core_foundation::CGPoint;
    use objc2_core_graphics::{
        CGAssociateMouseAndMouseCursorPosition, CGDisplayBounds, CGError, CGEvent, CGEventField,
        CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, CGGetActiveDisplayList,
        CGWarpMouseCursorPosition,
    };

    /// 한 번에 조회할 디스플레이 수 상한 — `screen_capture.rs` 와 같은 값:
    /// 실사용에서 이보다 많은 화면을 붙이는 경우는 없다(할당 없는 고정 배열).
    const MAX_DISPLAYS: u32 = 16;

    /// 활성 디스플레이 경계 목록 — 화면 밖 판정(엣지 7)용. 워커 스레드에서
    /// 안전하다(`screen_capture.rs:64-87` 선례: `CGGetActiveDisplayList` +
    /// `CGDisplayBounds` 는 스레드 구속이 없다).
    ///
    /// ⚠️ **`NSScreen.screens` 는 메인 스레드 전용이라 여기서 쓰지 않는다**
    /// (screens.rs:57-59). 한계: 다른 Space 의 좌표도 같은 화면 영역이라
    /// 통과한다 — `point_visible` 문서 참고.
    pub fn active_display_bounds() -> Vec<DisplayBounds> {
        let mut ids = [0u32; MAX_DISPLAYS as usize];
        let mut count: u32 = 0;
        // SAFETY: `ids` 는 `MAX_DISPLAYS` 개를 담을 수 있는 유효한 가변 버퍼이고
        // `count` 는 유효한 출력 포인터다. CG 는 버퍼 크기를 넘겨 쓰지 않는다.
        let err = unsafe { CGGetActiveDisplayList(MAX_DISPLAYS, ids.as_mut_ptr(), &mut count) };
        if err != CGError::Success {
            tracing::warn!(?err, "CGGetActiveDisplayList failed; returning empty display list");
            return Vec::new();
        }
        ids.iter()
            .take(count as usize)
            .map(|&display_id| {
                // CGDisplayBounds 는 objc2 가 safe 래퍼로 노출한다(스레드 구속 없음).
                let bounds = CGDisplayBounds(display_id);
                DisplayBounds {
                    x: bounds.origin.x,
                    y: bounds.origin.y,
                    width: bounds.size.width,
                    height: bounds.size.height,
                }
            })
            .collect()
    }

    /// 현재 커서 위치(포인트, 전역 좌표계) — 워커 스레드에서 안전.
    ///
    /// ⚠️ `screens::mouse_location()` 은 `NSEvent.mouseLocation` 기반이라
    /// **메인 스레드 전용**(screens.rs:96-105)이다 — 클릭 실행은 워커에
    /// 있으므로 `CGEvent::location(None)` 을 쓴다(계획 초안 A5).
    ///
    /// `None` 은 현재 커서 이벤트(전역 HID 상태의 위치)를 얻는 데 실패했을 때다.
    pub fn cursor_position() -> Option<(f64, f64)> {
        let event = CGEvent::new(None)?;
        // SAFETY: `event` 는 방금 만든, 소유한 유효한 CGEvent 다.
        let location = CGEvent::location(Some(&event));
        Some((location.x, location.y))
    }

    /// 좌클릭 하나(같은 좌표에 down → up)를 합성한다.
    ///
    /// - `click_state`(1/2/3)는 `kCGMouseEventClickState` 필드로 실어 더블·트리플
    ///   클릭의 연결 판정을 macOS 에 위임한다(D7 — **인공 딜레이 없음**).
    /// - `flags` 는 설정 OFF 상태에서 전달되는 modifier(§3.2 — `plan.flags`).
    /// - 마우스 이벤트는 마커를 실어 `SessionEventTap` 으로 발행한다(D6).
    /// - down/up 은 같은 좌표 연속 발행이라 macOS 가 드래그로 해석하지 않는다
    ///   (엣지 8 — 이동이 없는 down/up 에는 mouseDragged 가 없다).
    ///
    /// `false` 는 이벤트 **생성**(할당) 실패다 — 발행(`CGEventPost`)은 실패
    /// 개념이 없다.
    pub fn mouse_click(x: f64, y: f64, click_state: u8, flags: EventFlags) -> bool {
        let point = CGPoint::new(x, y);
        // SAFETY: 공유 소스는 프로세스 생애주기 동안 유효하고, 반환된
        // `CFRetained<CGEvent>` 는 이 함수가 소유한다.
        let Some(down) = CGEvent::new_mouse_event(
            shared_event_source(),
            CGEventType::LeftMouseDown,
            point,
            CGMouseButton::Left,
        ) else {
            return false;
        };
        prepare_mouse_event(&down, click_state, flags);
        CGEvent::post(CGEventTapLocation::SessionEventTap, Some(&down));

        let Some(up) = CGEvent::new_mouse_event(
            shared_event_source(),
            CGEventType::LeftMouseUp,
            point,
            CGMouseButton::Left,
        ) else {
            return false;
        };
        prepare_mouse_event(&up, click_state, flags);
        CGEvent::post(CGEventTapLocation::SessionEventTap, Some(&up));
        true
    }

    /// 좌클릭 이벤트 하나에 플래그·clickState·자기 합성 마커를 실는다.
    fn prepare_mouse_event(event: &CGEvent, click_state: u8, flags: EventFlags) {
        CGEvent::set_flags(Some(event), CGEventFlags(flags.0));
        CGEvent::set_integer_value_field(
            Some(event),
            CGEventField::MouseEventClickState,
            i64::from(click_state),
        );
        mark_synthetic(event);
    }

    /// 커서를 지정 좌표로 즉시 옮긴다 — `CGAssociateMouseAndMouseCursorPosition`
    /// (false) → 워프 → (true) 로 감싸, 물리 마우스가 큐에 남긴 이동 델타와
    /// 충돌해 커서가 다시 튀는 것을 막는다(§3.5 구현 유의점).
    ///
    /// 워프 자체가 실패하면(드묾) `false` — 호출자가 로그만 남기고 "커서가
    /// 클릭 지점에 남는" 안전한 열화를 수용한다(엣지 12).
    pub fn warp_cursor(x: f64, y: f64) -> bool {
        // 두 함수는 objc2 가 safe 래퍼로 노출한다. 워프 성공 여부와 무관하게
        // 연결을 (true) 로 되돌려야 물리 마우스 동기화가 영구히 분리되지 않는다.
        let _ = CGAssociateMouseAndMouseCursorPosition(false);
        let warped = CGWarpMouseCursorPosition(CGPoint::new(x, y));
        let _ = CGAssociateMouseAndMouseCursorPosition(true);
        warped == CGError::Success
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{active_display_bounds, cursor_position, mouse_click, warp_cursor};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::{DisplayBounds, EventFlags};

    /// 비-macOS 스텁 — 컴파일만 되고 실제로 만들어질 일이 없다(`screen_capture`
    /// 스텁과 같은 관례).
    pub fn active_display_bounds() -> Vec<DisplayBounds> {
        Vec::new()
    }

    pub fn cursor_position() -> Option<(f64, f64)> {
        None
    }

    pub fn mouse_click(_x: f64, _y: f64, _click_state: u8, _flags: EventFlags) -> bool {
        false
    }

    pub fn warp_cursor(_x: f64, _y: f64) -> bool {
        false
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{active_display_bounds, cursor_position, mouse_click, warp_cursor};