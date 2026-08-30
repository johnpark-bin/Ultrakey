//! 경로 판정 — AX 요소 경로(`kAXPressAction`)를 쓸 조건과, 클릭 시도 실패의
//! 3분기(결정 J5, 이슈 #44).
//!
//! 명세 §3.3.1 의 좁은 조건("AX 후보 + 시작/끝 지점 단일 클릭 상당")과 §5 #1·#5
//! 의 실패 정책을 순수 판정으로 뽑아낸 것이다. 후보 출처(OCR)와 불가능한 모드
//! 조합이 AX 경로를 타지 않는 것은 [`should_use_ax_path`] 하나가, "요소가
//! 사라졌는가 vs press 가 실패했는가"의 갈림은 [`ax_path_outcome`] 하나가
//! 결정한다 — 실행부는 그 결과만 소비한다.

use crate::mode::ClickMode;
use ultrakey_seek::CandidateSource;

/// AX 요소 경로(`kAXPressAction`)를 시도할 조건 — **`source == Accessibility`
/// && 단일 클릭 모드**(`clickStartMatch`/`clickEndMatch`). 그 외(OCR 후보 ·
/// 6종 모드)는 항상 좌표 경로(§3.3.1 원칙, §8 수용 기준 1·2).
#[must_use]
pub fn should_use_ax_path(source: CandidateSource, mode: ClickMode) -> bool {
    source == CandidateSource::Accessibility
        && matches!(
            mode,
            ClickMode::ClickStartMatch | ClickMode::ClickEndMatch
        )
}

/// 클릭 시점 재조회(`AXUIElementCopyElementAtPosition`)의 결과 — 세 갈래.
/// `Ok(None)`(그 좌표에 요소 없음)과 `Err`(IPC 실패·타임아웃)를 구분해 둔다
/// — 실행 정책은 둘 다 같은 행동(취소)이지만, 판정의 근거와 로그는 갈라질 수
/// 있다(J5, A1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Requery {
    /// 요소를 얻었다.
    Found,
    /// `Ok(None)` — 좌표에 요소가 없다(§5 #1 창 닫힘·#2 창 이동).
    Missing,
    /// `Err` — AX IPC 실패(권한 회수·응답 없는 앱, 엣지 14).
    Failed,
}

/// `perform_action(kAXPressAction)` 의 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Press {
    /// 성공 — 요소 기반 클릭이 완료됐다.
    Succeeded,
    /// 실패(`kAXErrorActionUnsupported` 등 — §5 #5 disabled/미지원).
    Failed,
}

/// AX 클릭 시도의 최종 판정(결정 J5, 이슈 #44).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxOutcome {
    /// press 성공 — 여기서 끝.
    Pressed,
    /// press 실패(요소는 유효) — **좌표 기반 폴백**(§5 #5, §8 수용 기준 17).
    CoordinateFallback,
    /// 재조회 실패(요소 없음/IPC 오류) — **조용히 취소**(§5 #1, §8 수용
    /// 기준 13). 창이 닫혔다면 좌표 폴백은 엉뚱한 창을 클릭할 수 있으므로
    /// 취소가 유일한 안전한 행동이다.
    Cancel,
}

/// 재조회·press 결과로 AX 경로의 결말을 정한다.
///
/// ⚠️ 재조회 실패는 `Missing`/`Failed` 구분 없이 전부 `Cancel` — J5 가
/// "요소 없음→`Ok(None)`, IPC 오류→`Err`" 둘 다 조용히 취소로 흡수한다고
/// 못박았기 때문이다.
#[must_use]
pub fn ax_path_outcome(requery: Requery, press: Press) -> AxOutcome {
    match requery {
        Requery::Found => match press {
            Press::Succeeded => AxOutcome::Pressed,
            Press::Failed => AxOutcome::CoordinateFallback,
        },
        Requery::Missing | Requery::Failed => AxOutcome::Cancel,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §8 수용 기준 2 — OCR 후보는 AX 경로를 절대 타지 않는다.
    #[test]
    fn ocr_candidates_never_use_ax_path() {
        for mode in [
            ClickMode::ClickStartMatch,
            ClickMode::ClickEndMatch,
            ClickMode::DoubleClickCopy,
            ClickMode::OnlyMoveCursor,
        ] {
            assert!(!should_use_ax_path(CandidateSource::Ocr, mode), "{mode:?}");
        }
    }

    /// §8 수용 기준 1 — AX 후보의 단일 클릭(시작/끝)만 AX 경로.
    #[test]
    fn ax_candidates_use_ax_path_only_for_single_click_modes() {
        assert!(should_use_ax_path(
            CandidateSource::Accessibility,
            ClickMode::ClickStartMatch
        ));
        assert!(should_use_ax_path(
            CandidateSource::Accessibility,
            ClickMode::ClickEndMatch
        ));
        for mode in [
            ClickMode::OnlyMoveCursor,
            ClickMode::ClickAndReturn,
            ClickMode::ClickReturnClick,
            ClickMode::DoubleClickCopy,
            ClickMode::TripleClickCopy,
        ] {
            assert!(
                !should_use_ax_path(CandidateSource::Accessibility, mode),
                "{mode:?} 는 좌표 경로여야 한다"
            );
        }
    }

    /// J5 — 재조회 성공 + press 성공 → 완료.
    #[test]
    fn requery_found_and_press_succeeded_is_pressed() {
        assert_eq!(
            ax_path_outcome(Requery::Found, Press::Succeeded),
            AxOutcome::Pressed
        );
    }

    /// §8 수용 기준 17 — press 가 `Unsupported` 로 실패하면 좌표 폴백으로
    /// 클릭이 완료된다.
    #[test]
    fn requery_found_and_press_failed_falls_back_to_coordinate() {
        assert_eq!(
            ax_path_outcome(Requery::Found, Press::Failed),
            AxOutcome::CoordinateFallback
        );
    }

    /// §8 수용 기준 13 — 재조회 실패(요소 없음·IPC 오류)는 어느 쪽이든
    /// 조용히 취소다(창 닫힘 — J5).
    #[test]
    fn missing_or_failed_requery_cancels_even_if_press_would_succeed() {
        assert_eq!(ax_path_outcome(Requery::Missing, Press::Succeeded), AxOutcome::Cancel);
        assert_eq!(ax_path_outcome(Requery::Missing, Press::Failed), AxOutcome::Cancel);
        assert_eq!(ax_path_outcome(Requery::Failed, Press::Succeeded), AxOutcome::Cancel);
        assert_eq!(ax_path_outcome(Requery::Failed, Press::Failed), AxOutcome::Cancel);
    }
}