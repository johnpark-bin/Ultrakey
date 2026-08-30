//! 클릭 지점 계산 — 모드별 기준점(Q12·Q13).
//!
//! 입력 사각형은 **전역 화면 좌표, 포인트, 좌상단 원점**(`TextCandidate::frame`
//! 그대로, A2) — 이 모듈은 픽셀→포인트 환산이나 다중 디스플레이 변환을 하지
//! 않는다. 그 변환은 F-02 가 이미 끝냈다(명세 §3.4 "입력값을 그대로 신뢰").

use crate::mode::ClickMode;
use ultrakey_seek::Rect;

/// 전역 화면 좌표(포인트, 좌상단 원점)의 한 점.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    /// 좌상단 원점 기준 x (포인트).
    pub x: f64,
    /// 좌상단 원점 기준 y (포인트).
    pub y: f64,
}

impl Point {
    /// 새 점.
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

/// 매치 **시작 지점** — 프레임 좌변 × 세로 중심(Q12, 결정 D4, 이슈 #44).
///
/// ⭐ **인셋 없음** — 임계값을 지어내지 않는다(명세 §3.4). 세로 중심을 쓰는
/// 이유: OCR frame 은 텍스트 bounding box 이므로 세로 중심은 텍스트 블록 안;
/// 가운데 정렬 라벨 컨트롤에서 좌상단 모서리 `(x, y)` 는 오히려 빈 여백일 수
/// 있다.
#[must_use]
pub fn start_point(frame: Rect) -> Point {
    Point::new(frame.x, frame.y + frame.height / 2.0)
}

/// 매치 **끝 지점** — 프레임 우변 × 세로 중심(Q12, 결정 D4, 이슈 #44). 텍스트
/// LTR 진행 기준의 우측 가장자리다(인셋 없음).
#[must_use]
pub fn end_point(frame: Rect) -> Point {
    Point::new(frame.x + frame.width, frame.y + frame.height / 2.0)
}

/// 모드별 클릭 지점 — 나머지 5종(`onlyMoveCursor`·`clickAndReturn`·
/// `clickReturnClick`·`doubleClickCopy`·`tripleClickCopy`)은 **전부 시작 지점**
/// 을 재사용한다(결정 D5, 이슈 #44 — 명세 §3.4 의 "동일한 시작 지점 규칙
/// 재사용" 제안 채택). `clickReturnClick` 의 두 번째 클릭이 원래 커서 위치라는
/// 것은 모드 규칙이지 지점 선택이 아니다 — 실행부가 채운다(계획 참고).
#[must_use]
pub fn point_for(mode: ClickMode, frame: Rect) -> Point {
    match mode {
        ClickMode::ClickEndMatch => end_point(frame),
        _ => start_point(frame),
    }
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

    /// Q12 — 시작 = `(frame.x, frame.y + height/2)`(인셋 없음).
    #[test]
    fn start_point_is_left_edge_at_vertical_center() {
        assert_eq!(start_point(FRAME), Point::new(10.0, 40.0));
    }

    /// Q12 — 끝 = `(frame.x + width, frame.y + height/2)`(인셋 없음).
    #[test]
    fn end_point_is_right_edge_at_vertical_center() {
        assert_eq!(end_point(FRAME), Point::new(110.0, 40.0));
    }

    /// §8 수용 기준 7 — 시작/끝 모드는 매치 **중심**이 아니라 시작/끝 지점을
    /// 클릭한다.
    #[test]
    fn start_and_end_modes_use_edges_not_center() {
        assert_eq!(point_for(ClickMode::ClickStartMatch, FRAME), start_point(FRAME));
        assert_eq!(point_for(ClickMode::ClickEndMatch, FRAME), end_point(FRAME));
        // 중심 (60, 40) 이 아니다.
        assert_ne!(point_for(ClickMode::ClickEndMatch, FRAME), Point::new(60.0, 40.0));
    }

    /// Q13(D5) — 나머지 6종은 전부 시작 지점을 재사용한다.
    #[test]
    fn all_other_modes_reuse_start_point() {
        for mode in [
            ClickMode::OnlyMoveCursor,
            ClickMode::ClickAndReturn,
            ClickMode::ClickReturnClick,
            ClickMode::DoubleClickCopy,
            ClickMode::TripleClickCopy,
        ] {
            assert_eq!(point_for(mode, FRAME), start_point(FRAME), "{mode:?}");
        }
    }
}