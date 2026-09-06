//! 경로 A — `CGEventTap` 안전 래퍼.
//!
//! `CGEventType` → `ultrakey_core::event::EventKind` 변환은 [`map_event_kind`]
//! 하나로 격리해 둔다 — `EventKind` 가 `CGEventType` 을 1:1 로 미러링하므로
//! 단순 매핑이지만, 나중에 어느 한쪽이 바뀌어도 고칠 지점이 한 곳이다.
//!
//! ⭐ **탭 비활성화(`kCGEventTapDisabledByTimeout`/`ByUserInput`) 통지는
//! 트램폴린이 콜백 호출과 무관하게 먼저 `CGEventTapEnable` 로 복구를 시도한다
//! — [`REENABLE_MAX_CONSECUTIVE`] 예산이 남아 있는 동안만.** `key-remapping-engine.md`
//! §8 수용 기준의 "예외 없이 재활성화를 시도한다"는 이 예산 안에서 지킨다(이슈 #65
//! Phase 1 리뷰 교정 1 — 문구 자체도 갱신 대상). 이 재활성화 자체를 엔진 콜백의
//! 반환값에 의존시키지 않는다 — 콜백을 등록하는 시점에는 아직 `EventTap` 자신이
//! 만들어지지 않아 엔진이 자기 참조를 캡처할 수 없다는 부트스트랩 문제도 함께
//! 피한다. 다만 `EventKind::TapDisabledByTimeout`/`TapDisabledByUserInput` 변형이
//! 존재하는 것은 엔진의 탭 생명주기 FSM(`ultrakey-engine::engine`)이 이 사건을
//! 관찰해야 한다는 뜻이므로, 재활성화 시도 여부와 무관하게 평소처럼 콜백을 호출해
//! 알린다 — 다만 콜백의 반환값(`TapAction`)은 무시하고 항상 원본 이벤트를 그대로
//! 돌려준다(`ultrakey-engine` 의 별도 1Hz 워치독 폴링은 여전히 `is_enabled()` 로 이
//! 크레이트 밖에서 독립적으로 동작한다).
//!
//! ⭐ 이슈 #140 — 이벤트 마스크는 더 이상 이 크레이트가 고정하지 않는다.
//! [`build_event_mask`] 는 `EngineConfig` 로부터 도출된 [`MouseEventNeeds`]
//! 를 받아 순수하게 비트를 계산하고, [`EventTap::create`] 는 그 결과를
//! 인자로 받아 `CGEventTapCreate` 에 넘긴다 — 마우스 이벤트를 실제로 소비할
//! 수 없는 구성에서는 탭이 그 이벤트를 아예 받지 않아, 커서 이동마다 발생하던
//! WindowServer ⇄ 탭 스레드 동기 왕복이 사라진다.

use crate::event::CgEventRef;
use ultrakey_core::event::EventKind;
use ultrakey_core::tap_mask::MouseEventNeeds;

/// `EventKind` → `CGEventType` 숫자값(`kCGEventXxx`, CoreGraphics 상수). `#[cfg]`
/// 밖에 순수 상수 표로 두는 이유는 [`build_event_mask`] 가 비-macOS 에서도 단위
/// 테스트로 돌게 하기 위함이다(`docs/plan/issue-140-event-mask.md` §2 D1). macOS
/// 테스트(`macos_impl::cg_event_type_value_tests` 아래 모듈)가 이 표를 실제
/// `CGEventType` 상수와 대조해 값을 고정해 둔다.
const fn cg_event_type_value(kind: EventKind) -> u32 {
    match kind {
        EventKind::KeyDown => 10,
        EventKind::KeyUp => 11,
        EventKind::FlagsChanged => 12,
        EventKind::LeftMouseDown => 1,
        EventKind::LeftMouseUp => 2,
        EventKind::RightMouseDown => 3,
        EventKind::RightMouseUp => 4,
        EventKind::OtherMouseDown => 25,
        EventKind::OtherMouseUp => 26,
        EventKind::LeftMouseDragged => 6,
        EventKind::RightMouseDragged => 7,
        EventKind::OtherMouseDragged => 27,
        EventKind::MouseMoved => 5,
        EventKind::ScrollWheel => 22,
        EventKind::TapDisabledByTimeout => 0xFFFF_FFFE,
        EventKind::TapDisabledByUserInput => 0xFFFF_FFFF,
    }
}

/// 전체 `EventKind` 표(마스크 판정 순회용). `TapDisabledBy*` 는
/// [`MouseEventNeeds::includes`] 가 항상 `false` 를 돌려주므로(그 둘은 마스크
/// 멤버가 아니라 macOS 가 무조건 보내는 통지다) 여기 넣어도 마스크에 비트가
/// 서지 않는다 — 그래도 매핑 순회 표는 완전하게 유지한다.
const ALL_EVENT_KINDS: &[EventKind] = &[
    EventKind::KeyDown,
    EventKind::KeyUp,
    EventKind::FlagsChanged,
    EventKind::LeftMouseDown,
    EventKind::LeftMouseUp,
    EventKind::RightMouseDown,
    EventKind::RightMouseUp,
    EventKind::OtherMouseDown,
    EventKind::OtherMouseUp,
    EventKind::LeftMouseDragged,
    EventKind::RightMouseDragged,
    EventKind::OtherMouseDragged,
    EventKind::MouseMoved,
    EventKind::ScrollWheel,
    EventKind::TapDisabledByTimeout,
    EventKind::TapDisabledByUserInput,
];

/// 지금 구성(`MouseEventNeeds`)에서 `CGEventTapCreate` 에 넘길 이벤트 마스크를
/// 만든다. keyDown/keyUp/flagsChanged 는 항상 들어가고, 마우스 종류별로는
/// `needs.includes(kind)` 가 참인 것만 들어간다(이슈 #140 — 마우스 이벤트를 실제로
/// 소비할 수 없는 구성에서는 탭이 그 이벤트를 아예 받지 않는다).
///
/// `#[cfg(target_os = "macos")]` **밖**의 순수 함수다 — 마스크 판정 자체는
/// 플랫폼에 의존하지 않으므로 비-macOS 에서도 단위 테스트가 돈다.
pub fn build_event_mask(needs: &MouseEventNeeds) -> u64 {
    ALL_EVENT_KINDS
        .iter()
        .filter(|&&kind| needs.includes(kind))
        .fold(0u64, |mask, &kind| {
            mask | (1u64 << (cg_event_type_value(kind) as u64))
        })
}

#[cfg(test)]
mod build_event_mask_tests {
    use super::*;

    fn bit(v: u32) -> u64 {
        1u64 << (v as u64)
    }

    #[test]
    fn key_only_needs_yields_key_bits_only() {
        let mask = build_event_mask(&MouseEventNeeds::default());
        assert_eq!(mask, bit(10) | bit(11) | bit(12));
    }

    #[test]
    fn all_mouse_needs_matches_the_old_fixed_mask() {
        let needs = MouseEventNeeds {
            click: true,
            drag: true,
            r#move: true,
            scroll: true,
        };
        let mask = build_event_mask(&needs);
        // 이슈 #140 이전 `macos_impl::build_event_mask()` 가 고정으로 넣던
        // 14 종(키 3 + 마우스 11)을 같은 숫자 표로 재계산해 회귀를 막는다.
        let old_fixed_mask: u64 = [
            10u32, 11, 12, // key
            1, 2, 3, 4, 25, 26, // click
            6, 7, 27, // drag
            5,  // move
            22, // scroll
        ]
        .into_iter()
        .fold(0u64, |m, v| m | bit(v));
        assert_eq!(mask, old_fixed_mask);
    }

    #[test]
    fn click_only_needs_yields_key_plus_click_bits() {
        let needs = MouseEventNeeds {
            click: true,
            ..MouseEventNeeds::default()
        };
        let mask = build_event_mask(&needs);
        assert_eq!(
            mask,
            bit(10) | bit(11) | bit(12) | bit(1) | bit(2) | bit(3) | bit(4) | bit(25) | bit(26)
        );
    }
}

/// 연속 즉시 재활성화 예산 — 트램폴린 하나(=[`macos_impl::EventTap`] 인스턴스 하나)의
/// 생애주기 동안 유지되는 상태다.
///
/// ⭐ **이슈 #65 Phase 1 리뷰 교정 1.** 최초 구현(commit 7031351)은 이 예산을 "1초
/// 창" 으로 시간 리셋했다 — 권한이 계속 없는 동안 매초 예산이 다시 채워져, `(a)`
/// `handle_recover_tap` 이 스스로 부르는 재활성화(당시)가 한 번이라도 새 비활성화
/// 통지를 재점화하면 그 통지가 다시 최대치(5회)까지 mach-속도 핑퐁을 허용하는 구멍이
/// 있었다(§65 진단 "가설 (b)"). Phase 1 초안은 이를 "탭 인스턴스 생애주기 1회분"으로
/// 고치자고 제안했으나, 리뷰가 지적한 대로 그러면 **장기 실행에서 오탐한다** — 며칠
/// 켜 둔 앱에서 정상적인 `kCGEventTapDisabledByTimeout` 이 드문드문 5번만 누적돼도
/// 건강한 탭을 해체하게 된다. 그래서 리셋 트리거를 시간도, 생애주기 1회분도 아닌
/// **"트램폴린에 실제(비활성화 통지가 아닌) 이벤트가 도달했다는 사실"**로 확정한다 —
/// 그 순간 탭이 실제로 살아서 이벤트를 통과시키고 있음이 증명되기 때문이다. 폭주
/// 중에는 탭이 즉시 다시 꺼져 실제 이벤트가 하나도 통과하지 못하므로, 이 카운터는
/// 폭주가 계속되는 한 **절대** 리셋되지 않는다 — "연속 N회 즉시 재비활성화"가 정확히
/// 폭주의 정의다.
///
/// 플랫폼 FFI 와 무관한 순수 로직이라 `#[cfg(target_os = "macos")]` 밖에 둔다 — 이
/// 값이 그동안 `unsafe extern "C-unwind" fn` 트램폴린 안에 직접 박혀 있어 유닛
/// 테스트가 불가능했던 것이, 이 버그가 세 차례 완화에도 재발하며 테스트로 잡히지
/// 않은 이유 중 하나였다.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ReenableBudget {
    consecutive_disables: u32,
}

/// 1초 창이 아니라 **연속** 허용 횟수다(리뷰 교정 1 — 이름 자체가 시간 개념이 빠졌음을
/// 반영한다). 사용자 설정(`Timings`)과 분리된, 물리적 mach-속도 핑퐁을 막기 위한 낮은
/// 고정 안전값이다 — 이 값의 역할은 정책이 아니라 "최악의 경우 몇 번의 즉시 핑퐁을
/// 허용할 것인가"라는 물리적 상한이라 사용자가 조정할 이유가 없다.
pub(crate) const REENABLE_MAX_CONSECUTIVE: u32 = 5;

impl ReenableBudget {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    /// 비활성화 통지 하나를 예산에서 소비한다. 반환값이 `true` 면 아직 예산 안이라
    /// `CGEventTapEnable` 재시도를 해도 된다.
    pub(crate) fn note_disable(&mut self) -> bool {
        self.consecutive_disables += 1;
        self.consecutive_disables <= REENABLE_MAX_CONSECUTIVE
    }

    /// 실제(비활성화 아닌) 이벤트가 탭을 통과했다 — 탭이 살아있다는 증거이므로
    /// 연속 카운터를 리셋한다. **시간은 이 예산을 리셋하지 않는다 — 오직 이 사실만.**
    pub(crate) fn note_real_event(&mut self) {
        self.consecutive_disables = 0;
    }

    /// 예산이 소진됐는가 — `handle_recover_tap` 이 탭을 해체할지 판단하는 신호다.
    pub(crate) fn is_exhausted(&self) -> bool {
        self.consecutive_disables > REENABLE_MAX_CONSECUTIVE
    }
}

#[cfg(test)]
mod reenable_budget_tests {
    use super::*;

    #[test]
    fn allows_attempts_up_to_the_limit() {
        let mut b = ReenableBudget::new();
        for _ in 0..REENABLE_MAX_CONSECUTIVE {
            assert!(b.note_disable(), "한도 안에서는 재시도를 허용해야 한다");
        }
        assert!(!b.is_exhausted());
    }

    /// ⭐ 이번 회귀의 핵심 방지 테스트 — 한도를 넘으면 **시간이 아무리 지나도** 계속
    /// 거부한다(이 타입에는 애초에 시간이라는 입력 자체가 없다).
    #[test]
    fn denies_forever_once_exhausted_no_matter_how_many_more_disables_arrive() {
        let mut b = ReenableBudget::new();
        // 정확히 한도만큼은 허용된다 — 한도를 넘는 1회가 소진을 확정한다.
        for _ in 0..=REENABLE_MAX_CONSECUTIVE {
            b.note_disable();
        }
        assert!(b.is_exhausted());
        for _ in 0..1000 {
            assert!(!b.note_disable(), "소진된 뒤에는 계속 거부해야 한다");
        }
        assert!(b.is_exhausted());
    }

    /// 실제 이벤트 1건이 예산을 리셋한다 — 탭이 살아있다는 증거이기 때문이다.
    #[test]
    fn a_single_real_event_resets_the_budget() {
        let mut b = ReenableBudget::new();
        for _ in 0..=REENABLE_MAX_CONSECUTIVE {
            b.note_disable();
        }
        assert!(b.is_exhausted());

        b.note_real_event();
        assert!(!b.is_exhausted());
        for _ in 0..REENABLE_MAX_CONSECUTIVE {
            assert!(
                b.note_disable(),
                "리셋된 뒤에는 다시 한도만큼 허용해야 한다"
            );
        }
    }

    /// 새 인스턴스(= 새 탭)는 항상 신선한 예산으로 시작한다 — `EventTap::create()` 가
    /// 새 `CallbackContext` 를 만들 때만 예산이 새로 생긴다는 설계를 보장한다.
    #[test]
    fn a_new_instance_always_starts_fresh() {
        let b = ReenableBudget::new();
        assert!(!b.is_exhausted());
        assert_eq!(b, ReenableBudget::default());
    }
}

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
pub type TapCallback = Box<dyn FnMut(TapProxy, EventKind, &mut CgEventRef) -> TapAction + Send>;

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::{TapAction, TapCallback, TapCreateError};
    use crate::event::{CgEventRef, ULTRAKEY_MAGIC};
    use core::cell::{Cell, RefCell};
    use core::ffi::c_void;
    use core::ptr::NonNull;
    use objc2_core_foundation::{
        kCFRunLoopCommonModes, CFMachPort, CFRetained, CFRunLoop, CFRunLoopSource,
    };
    use objc2_core_graphics::{
        CGEvent, CGEventField, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement,
        CGEventTapProxy, CGEventType,
    };
    use ultrakey_core::event::EventKind;

    use super::ReenableBudget;

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
        /// ⭐ 즉시 재활성화 **차단기**(circuit breaker) 상태 — 아래
        /// [`trampoline`] 의 "재활성화 폭주" 주석 참고. 트램폴린은 항상 같은
        /// 탭 스레드에서 직렬로만 실행되므로 `Cell` 로 충분하다. 예산의 리셋
        /// 규칙은 [`super::ReenableBudget`] 문서 참고 — 시간이 아니라 실제
        /// 이벤트 통과로만 리셋된다(이슈 #65 Phase 1 리뷰 교정 1).
        reenable_budget: Cell<ReenableBudget>,
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
            // ⛔ **무조건 재활성화하면 시스템 전체 입력이 멈춘다** — 실측 회귀.
            //
            // Accessibility 권한을 실행 중에 회수하면 macOS 가 탭을 끈다. 여기서
            // 조건 없이 `CGEventTapEnable(true)` 를 부르면 macOS 가 곧바로 다시
            // 끄고 또 통지를 보낸다 — 재활성화⇄비활성화가 mach 메시지 속도로
            // 무한 반복된다. 그동안 이 스레드의 런루프는 그 통지만 처리하느라
            // **탭 자신의 실제 이벤트도, 커맨드 소스(`drain_commands`)도** 서비스하지
            // 못한다. 결과: 키 입력과 클릭이 전부 죽고(탭을 통과하지 못한다),
            // 커서만 움직이며(WindowServer 가 직접 그린다), **로그는 한 줄도 남지
            // 않는다**(커맨드 소스가 굶어서). 실측 증상이 정확히 이것이었다.
            //
            // ⚠️ 그래서 §8 수용 기준("예외 없이 재활성화를 시도한다")은 **예산 안에서만**
            // 지킨다(이슈 #65 Phase 1 리뷰 교정 1 — 예산은 시간 창이 아니라 연속
            // 소비다, `super::ReenableBudget` 참고). 정상적인 `TapDisabledByTimeout`
            // 은 드물게 한 번씩 오고 그 사이사이 실제 이벤트가 예산을 리셋하므로 이
            // 상한에 걸리지 않는다. 상한을 넘으면(=권한이 없는 동안 재활성화해도
            // 실제 이벤트가 단 하나도 통과하지 못하고 즉시 다시 꺼지는 상태가
            // 계속됨) 재활성화를 멈추고 엔진 FSM 에 맡긴다 — 꺼진 탭은 이벤트를
            // 막지 않으므로 **그 순간 시스템 입력이 즉시 정상으로 돌아온다**. 이후
            // 복구는 `RecoverTap` 이 권한을 확인해 가며 처리하되, 예산이 소진된
            // 탭은 재활성화가 아니라 **해체**로 이어진다 — 재생성 에스컬레이션은
            // stale `AXIsProcessTrusted()` 상황에서 재생성마다 예산이 다시 채워져
            // 더 느린 폭주가 되므로 채택하지 않는다(Phase 1 리뷰 교정 2).
            let mut budget = ctx.reenable_budget.get();
            let should_reenable = budget.note_disable();
            ctx.reenable_budget.set(budget);
            if should_reenable {
                if let Some(port) = ctx.mach_port.borrow().as_ref() {
                    CGEvent::tap_enable(port, true);
                }
            }
            if let Some(kind) = map_event_kind(event_type) {
                // SAFETY: `event` 는 이 콜백 호출 동안에만 유효하다는 계약을
                // `CgEventRef` 가 그대로 물려받는다.
                let mut event_ref = unsafe { CgEventRef::from_raw(event) };
                let _ = (ctx.callback)(TapProxy(proxy), kind, &mut event_ref);
            }
            return event.as_ptr();
        }

        // ⭐ 실제(비활성화 아닌) 이벤트가 여기까지 도달했다 — 탭이 살아서 이벤트를
        // 통과시키고 있다는 증거이므로 연속 재활성화 예산을 리셋한다(이슈 #65
        // Phase 1 리뷰 교정 1). 시간은 이 예산을 리셋하지 않는다 — 오직 이 사실만.
        let mut budget = ctx.reenable_budget.get();
        budget.note_real_event();
        ctx.reenable_budget.set(budget);

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
        /// 이 탭이 만들어질 때 넘겨진 마스크(이슈 #140) — `EventTap::mask()` 가
        /// 그대로 돌려준다. `CGEventTapCreate` 는 마스크를 보관하지 않으므로
        /// 직접 들고 있어야 한다.
        mask: u64,
    }

    // SAFETY: `EventTap` 은 생성된 스레드(전용 탭 스레드) 안에서만 만들어지고
    // 쓰이고 버려지는 것이 architecture.md §2.1 배치의 전제다. 이 타입 자체는
    // 스레드 경계를 넘지 않으므로 `Send`/`Sync` 를 부여하지 않는다(기본값 유지).

    impl EventTap {
        /// `CGEventTapCreate` 로 탭을 만든다. 마스크는 더 이상 이 크레이트가
        /// 고정하지 않는다(이슈 #140) — 호출자(엔진)가 `EngineConfig` 로부터
        /// `ultrakey_core::tap_mask::mouse_event_needs` + `build_event_mask` 로
        /// 도출해 넘긴다.
        pub fn create(callback: TapCallback, mask: u64) -> Result<Self, TapCreateError> {
            let boxed = Box::new(CallbackContext {
                callback,
                mach_port: RefCell::new(None),
                reenable_budget: Cell::new(ReenableBudget::new()),
            });
            let ctx_ptr = Box::into_raw(boxed);

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
                mask,
            })
        }

        /// 이 탭이 만들어질 때 넘겨진 이벤트 마스크(이슈 #140). 재생성 여부
        /// 판정(`lifecycle::reconfigure_tap_decision` 의 `mask_changed`)이
        /// 이 값을 새로 도출한 마스크와 비교한다.
        pub fn mask(&self) -> u64 {
            self.mask
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

        /// ⭐ 이슈 #65 Phase 1 리뷰 교정 2 — 트램폴린의 연속 재활성화 예산
        /// ([`ReenableBudget`])이 소진됐는가. `engine.rs` 의 `handle_recover_tap`
        /// 이 이 신호("탭이 즉시 다시 꺼진다는 사실 자체")로 탭을 해체할지
        /// 판단한다 — `AXIsProcessTrusted()` 하나에만 의존하지 않기 위함이다
        /// (그 값이 회수 직후 순간적으로 stale `true` 를 돌려줄 수 있다는 것이
        /// 이슈 #65 Phase 1 진단의 결론이었다).
        pub fn reenable_budget_exhausted(&self) -> bool {
            match self.ctx {
                // SAFETY: `ctx` 는 `create()` 에서 `Box::into_raw` 로 만든 뒤 이
                // `EventTap` 이 배타 소유해 온, 살아있는 동안 항상 유효한 포인터다
                // (트램폴린과 동일한 근거, 이 파일 상단 모듈 문서 참고). 이 조회는
                // `Cell<ReenableBudget>::get()` 하나뿐이라 부작용이 없다.
                Some(ptr) => unsafe { (*ptr.as_ptr()).reenable_budget.get() }.is_exhausted(),
                None => false,
            }
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

    /// 이슈 #140 — [`super::cg_event_type_value`] 의 숫자 표가 실제
    /// `objc2_core_graphics::CGEventType` 상수와 정확히 일치하는지 macOS 에서만
    /// 고정한다. 비-macOS 에서는 이 크레이트를 통해 `objc2_core_graphics` 를
    /// 링크할 수 없으므로 여기 둔다(`build_event_mask` 자체의 단위 테스트는
    /// cfg 밖에 있어 어디서나 돈다).
    #[cfg(test)]
    mod cg_event_type_value_tests {
        use super::super::cg_event_type_value;
        use objc2_core_graphics::CGEventType;
        use ultrakey_core::event::EventKind;

        #[test]
        fn matches_cgeventtype_constants_for_every_event_kind() {
            let pairs: &[(EventKind, CGEventType)] = &[
                (EventKind::KeyDown, CGEventType::KeyDown),
                (EventKind::KeyUp, CGEventType::KeyUp),
                (EventKind::FlagsChanged, CGEventType::FlagsChanged),
                (EventKind::LeftMouseDown, CGEventType::LeftMouseDown),
                (EventKind::LeftMouseUp, CGEventType::LeftMouseUp),
                (EventKind::RightMouseDown, CGEventType::RightMouseDown),
                (EventKind::RightMouseUp, CGEventType::RightMouseUp),
                (EventKind::OtherMouseDown, CGEventType::OtherMouseDown),
                (EventKind::OtherMouseUp, CGEventType::OtherMouseUp),
                (EventKind::LeftMouseDragged, CGEventType::LeftMouseDragged),
                (EventKind::RightMouseDragged, CGEventType::RightMouseDragged),
                (EventKind::OtherMouseDragged, CGEventType::OtherMouseDragged),
                (EventKind::MouseMoved, CGEventType::MouseMoved),
                (EventKind::ScrollWheel, CGEventType::ScrollWheel),
                (
                    EventKind::TapDisabledByTimeout,
                    CGEventType::TapDisabledByTimeout,
                ),
                (
                    EventKind::TapDisabledByUserInput,
                    CGEventType::TapDisabledByUserInput,
                ),
            ];
            for (kind, cg_type) in pairs {
                assert_eq!(
                    cg_event_type_value(*kind),
                    cg_type.0,
                    "{kind:?} 의 숫자값이 CGEventType 상수와 어긋난다"
                );
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
        pub fn create(_callback: TapCallback, _mask: u64) -> Result<Self, TapCreateError> {
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
        pub fn reenable_budget_exhausted(&self) -> bool {
            match self.0 {}
        }
        pub fn mask(&self) -> u64 {
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
