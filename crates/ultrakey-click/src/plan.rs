//! 클릭 계획 — 모드별 프리미티브 시퀀스(계획 초안 §2.2 표) 산출.
//!
//! 한 모드가 실행해야 하는 "클릭 몇 번 · 각각의 clickState · 어느 지점 · 워프
//! 복귀 여부 · ⌘C 합성 여부"를 하나의 불변 계획으로 뽑아낸다 — 실행부(executor)
//! 는 이 계획만 순서대로 소비하면 된다. 결정 값은 전부 이슈 #44 의 클론 설계
//! 결정(미검증).

use ultrakey_core::flags::EventFlags;

use crate::mode::{ClickMode, MODIFIER_MASK};
use crate::point::{point_for, Point};
use ultrakey_seek::Rect;

/// 모드 하나의 실행 순서를 담은 계획.
#[derive(Debug, Clone, PartialEq)]
pub struct ClickPlan {
    /// 실행할 모드(로그·진단용).
    pub mode: ClickMode,
    /// 기본 클릭/커서 이동 지점 — 시작/끝(모드별), 포인트 단위 전역 좌표.
    pub point: Point,
    /// `clickReturnClick` 만 `Some` — **원래 커서 위치의 두 번째 클릭 지점**.
    /// 계획을 세우는 시점에는 알 수 없어 실행부(executor)가 런타임에 채운다
    /// (§3.3 표 정의 — 모드 규칙이지 지점 선택이 아니다).
    pub second_point: Option<Point>,
    /// 클릭 수 — `onlyMoveCursor` 는 0(커서 이동만, §8 수용 기준 6).
    pub clicks: usize,
    /// 각 클릭의 `kCGMouseEventClickState` 시퀀스 — `clicks` 와 같은 길이.
    /// 더블클릭 = `[1, 2]`, 트리플 = `[1, 2, 3]`(D7 — 연결은 clickState 필드로만
    /// 표시하고 인공 딜레이는 없다).
    pub click_states: Vec<u8>,
    /// 클릭 후 원래 커서 위치로 워프 복귀할지 — `clickAndReturn`/`clickReturnClick`
    /// (§3.5 — 모드별 정의이지 전체 공통 후처리가 아니다).
    pub warp_back: bool,
    /// 클릭 후 `⌘C` 키 합성 1회를 할지 — `doubleClickCopy`/`tripleClickCopy`
    /// (결정 D11, 이슈 #44). macOS 더블/트리플클릭은 **선택만** 만들고 복사는
    /// 하지 않으므로, 클립보드 복사를 위해 키 합성이 필요하다 — 명세 §3.1 7단계
    /// 를 이 값으로 확정한다(클론 설계 결정, 미검증).
    pub copy_after: bool,
    /// 클릭 이벤트에 실을 modifier — 설정 OFF 상태에서의 전달 플래그(§3.2
    /// "modifiers will be applied to the click"). ON 상태에서는 `NONE`.
    pub flags: EventFlags,
    /// AX 요소 경로(`kAXPressAction`)를 먼저 시도할지 — `path::should_use_ax_path`
    /// 의 판정 결과.
    pub use_ax_path: bool,
    /// `clickReturnClick` — 두 번째 클릭이 원래 커서 위치에서 일어난다.
    pub second_click_at_origin: bool,
}

/// 계획을 산출한다 — 계획 초안 §2.2 표 그대로.
///
/// `flags` 는 위 [`ClickPlan::flags`] 문서대로 **이미 변환된** 값(`NONE` 또는
/// [`passthrough_flags`])을 받는다. `use_ax_path` 는 `path::should_use_ax_path`
/// 의 결과를 받는다(이 함수는 source 를 모르므로).
#[must_use]
pub fn plan_for(mode: ClickMode, frame: Rect, flags: EventFlags, use_ax_path: bool) -> ClickPlan {
    let (clicks, click_states, warp_back, copy_after, second_click_at_origin) = match mode {
        ClickMode::OnlyMoveCursor => (0, vec![], false, false, false),
        ClickMode::ClickStartMatch | ClickMode::ClickEndMatch => (1, vec![1], false, false, false),
        ClickMode::ClickAndReturn => (1, vec![1], true, false, false),
        ClickMode::ClickReturnClick => (2, vec![1, 1], true, false, true),
        ClickMode::DoubleClickCopy => (2, vec![1, 2], false, true, false),
        ClickMode::TripleClickCopy => (3, vec![1, 2, 3], false, true, false),
    };

    ClickPlan {
        mode,
        point: point_for(mode, frame),
        second_point: None,
        clicks,
        click_states,
        warp_back,
        copy_after,
        flags,
        use_ax_path,
        second_click_at_origin,
    }
}

/// 설정 OFF 상태에서 클릭 이벤트에 실을 modifier — 스냅샷에서 **주 4종
/// (⇧⌃⌥⌘)만** 남긴다(§3.2). 캡스락·fn·장치 구분 비트는 클릭에 실을 플래그로
/// 옮기지 않는다 — 클릭 모드 매칭(`mode::MODIFIER_MASK`)과 같은 마스크 관례다.
#[must_use]
pub fn passthrough_flags(modifiers: EventFlags) -> EventFlags {
    EventFlags(modifiers.0 & MODIFIER_MASK.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME: Rect = Rect {
        x: 10.0,
        y: 20.0,
        width: 100.0,
        height: 40.0,
    };

    /// 기본 모드 — 클릭 1회(clickState 1) · 시작 지점 · 워프/복사 없음.
    #[test]
    fn click_start_match_is_single_click_at_start() {
        let plan = plan_for(ClickMode::ClickStartMatch, FRAME, EventFlags::NONE, false);
        assert_eq!(plan.clicks, 1);
        assert_eq!(plan.click_states, vec![1]);
        assert_eq!(plan.point, crate::point::start_point(FRAME));
        assert!(!plan.warp_back);
        assert!(!plan.copy_after);
        assert!(!plan.second_click_at_origin);
    }

    /// 끝 지점 모드는 시작 지점이 아니라 끝 지점을 찍는다(Q12).
    #[test]
    fn click_end_match_uses_end_point() {
        let plan = plan_for(ClickMode::ClickEndMatch, FRAME, EventFlags::NONE, false);
        assert_eq!(plan.point, crate::point::end_point(FRAME));
        assert_eq!(plan.clicks, 1);
    }

    /// §8 수용 기준 6 — `onlyMoveCursor` 는 클릭 이벤트가 전혀 없고 커서만
    /// 이동한다(clicks 0).
    #[test]
    fn only_move_cursor_has_zero_clicks() {
        let plan = plan_for(ClickMode::OnlyMoveCursor, FRAME, EventFlags::NONE, false);
        assert_eq!(plan.clicks, 0);
        assert!(plan.click_states.is_empty());
        assert!(!plan.warp_back);
        assert!(!plan.copy_after);
    }

    /// §8 수용 기준 8 — `clickAndReturn` 은 클릭 1회 + 워프 복귀.
    #[test]
    fn click_and_return_warps_back_after_one_click() {
        let plan = plan_for(ClickMode::ClickAndReturn, FRAME, EventFlags::NONE, false);
        assert_eq!(plan.clicks, 1);
        assert!(plan.warp_back);
        assert!(!plan.second_click_at_origin);
        assert!(plan.second_point.is_none());
    }

    /// §8 수용 기준 8 — `clickReturnClick` 은 클릭 → 복귀 → 원위치 재클릭.
    /// 두 번째 클릭 지점은 플랜 시점에 미지(실행부가 채움) — 표시만 남긴다.
    #[test]
    fn click_return_click_has_two_clicks_and_second_at_origin() {
        let plan = plan_for(ClickMode::ClickReturnClick, FRAME, EventFlags::NONE, false);
        assert_eq!(plan.clicks, 2);
        // 서로 다른 지점이라 각각 clickState 1(더블클릭 아님).
        assert_eq!(plan.click_states, vec![1, 1]);
        assert!(plan.warp_back);
        assert!(plan.second_click_at_origin);
        assert!(plan.second_point.is_none(), "실행부가 채워야 한다");
    }

    /// §8 수용 기준 9 — 더블클릭은 클릭 2회(clickState `[1,2]`) + ⌘C 복사.
    #[test]
    fn double_click_copy_has_two_clicks_and_copy() {
        let plan = plan_for(ClickMode::DoubleClickCopy, FRAME, EventFlags::NONE, false);
        assert_eq!(plan.clicks, 2);
        assert_eq!(plan.click_states, vec![1, 2]);
        assert!(plan.copy_after);
        assert!(!plan.warp_back);
        assert!(!plan.second_click_at_origin);
        // 단어 선택을 위해 시작 지점이 기준이다(Q13).
        assert_eq!(plan.point, crate::point::start_point(FRAME));
    }

    /// §8 수용 기준 9 — 트리플클릭은 클릭 3회(clickState `[1,2,3]`) + ⌘C 복사.
    #[test]
    fn triple_click_copy_has_three_clicks_and_copy() {
        let plan = plan_for(ClickMode::TripleClickCopy, FRAME, EventFlags::NONE, false);
        assert_eq!(plan.clicks, 3);
        assert_eq!(plan.click_states, vec![1, 2, 3]);
        assert!(plan.copy_after);
        assert!(!plan.warp_back);
    }

    /// §8 수용 기준 5 — 설정 OFF + ⌘: 클릭 모드는 기본 동작(클릭 1)이고
    /// ⌘ 플래그가 계획의 `flags` 에 실려 있다.
    #[test]
    fn disabled_mode_passes_modifiers_into_click_flags() {
        let mods = EventFlags::COMMAND | EventFlags::CAPS_LOCK;
        let flags = passthrough_flags(mods);
        let plan = plan_for(ClickMode::ClickStartMatch, FRAME, flags, false);
        assert_eq!(plan.clicks, 1);
        assert_eq!(plan.flags, EventFlags::COMMAND, "캡스락은 실리지 않는다");
    }

    /// modifier 플래그 변환 — 주 4종만 남고 나머지(caps lock·fn·장치 비트)는
    /// 마스킹된다.
    #[test]
    fn passthrough_flags_keeps_only_the_four_main_bits() {
        let noisy = EventFlags(
            EventFlags::SHIFT.0
                | EventFlags::COMMAND.0
                | EventFlags::CAPS_LOCK.0
                | EventFlags::SECONDARY_FN.0
                | EventFlags::DEVICE_LEFT_COMMAND.0,
        );
        assert_eq!(
            passthrough_flags(noisy),
            EventFlags::SHIFT | EventFlags::COMMAND
        );
        assert_eq!(passthrough_flags(EventFlags::NONE), EventFlags::NONE);
    }
}