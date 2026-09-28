//! 화면 밖 판정(엣지 7 — 디스플레이 구성 변경 방어).
//!
//! 계산된 클릭 지점이 현재 활성 디스플레이 안에 있는지 검증한다. 벗어나면
//! 클릭을 실행하지 않고 세션을 취소한다(v1.55 다중 디스플레이 이력에 대한
//! 방어, 명세 §5 #7).

use crate::point::Point;
use ultrakey_seek::Rect;

/// 클릭 지점이 디스플레이 경계 **하나 이상**에 포함되는가.
///
/// ⭐ **바운딩 합집합 사각형이 아니라 "아무 디스플레이 경계에 포함"** 이면
/// 통과한다 — 합집합 사각형은 디스플레이 사이의 빈 틈(gap)까지 포함해 그
/// 틈을 향한 클릭을 통과시키는 오탐을 만들 수 있다.
///
/// 경계는 닫힌 구간(`[x, x+w] × [y, y+h]`)으로 본다 — 매치 끝 지점이 화면
/// 오른쪽/아래쪽 가장자리에 정확히 떨어지는 클릭을 취소하지 않기 위함이다
/// (반개구간이면 그 클릭이 1pt 차이로 "화면 밖"이 된다).
///
/// 한계(명세 §5 #3·#7): 다른 Space 의 좌표도 같은 화면 영역이라 통과한다 —
/// 이것은 사용자 설정(Focus OFF)의 명시적 트레이드오프와 같은 성격이다.
#[must_use]
pub fn point_visible(point: Point, display_bounds: &[Rect]) -> bool {
    display_bounds.iter().any(|d| {
        point.x >= d.x && point.x <= d.x + d.width && point.y >= d.y && point.y <= d.y + d.height
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const DISPLAY: Rect = Rect {
        x: 0.0,
        y: 0.0,
        width: 1440.0,
        height: 900.0,
    };

    /// 안쪽 점 — 당연히 통과.
    #[test]
    fn point_inside_a_display_is_visible() {
        assert!(point_visible(Point::new(720.0, 450.0), &[DISPLAY]));
    }

    /// 바깥쪽 점 — 통과하지 않는다.
    #[test]
    fn point_outside_every_display_is_not_visible() {
        assert!(!point_visible(Point::new(-1.0, 10.0), &[DISPLAY]));
        assert!(!point_visible(Point::new(1500.0, 10.0), &[DISPLAY]));
        assert!(!point_visible(Point::new(10.0, 1000.0), &[DISPLAY]));
        // 빈 목록 — 어떤 것도 통과하지 않는다(디스플레이 조회 실패 시 방어).
        assert!(!point_visible(Point::new(0.0, 0.0), &[]));
    }

    /// 경계는 닫힌 구간 — 가장자리에 정확히 떨어지는 클릭은 취소하지 않는다
    /// (매치 끝 지점 = 화면 우변 방어).
    #[test]
    fn boundary_points_count_as_visible() {
        assert!(point_visible(Point::new(0.0, 0.0), &[DISPLAY])); // 좌상단.
        assert!(point_visible(Point::new(1440.0, 900.0), &[DISPLAY])); // 우하단.
        assert!(point_visible(Point::new(720.0, 900.0), &[DISPLAY])); // 하단 변.
    }

    /// 다중 디스플레이 — 어느 한 디스플레이에만 들어도 통과.
    #[test]
    fn point_in_any_single_display_is_visible() {
        let second = Rect {
            x: 1440.0,
            y: 100.0,
            width: 1920.0,
            height: 1080.0,
        };
        let bounds = [DISPLAY, second];
        // 두 번째(오른쪽) 디스플레이에만 들어있는 점 — 첫 번째 밖이어도 통과.
        assert!(point_visible(Point::new(1500.0, 200.0), &bounds));
        assert!(point_visible(Point::new(3000.0, 500.0), &bounds));
        assert!(!point_visible(Point::new(5000.0, 300.0), &bounds)); // 둘 다 밖.
    }

    /// ⭐ 합집합 사각형이 아니라 "개별 디스플레이 포함" — 디스플레이 사이의
    /// 빈 틈은 통과시키지 않는다(오탐 방지).
    #[test]
    fn gap_between_displays_is_not_visible() {
        // 두 디스플레이가 x=0 아래/위에 나란히 있어 좌표상 "틈"이 생기는 구성:
        // (900, -50) 은 아무 디스플레이에도 들어가지 않지만,
        // 바운딩 합집합 사각형[-100..1440, -50..900] 에는 들어간다.
        let below = Rect {
            x: 0.0,
            y: 0.0,
            width: 800.0,
            height: 600.0,
        };
        let above = Rect {
            x: 0.0,
            y: 650.0,
            width: 800.0,
            height: 600.0,
        };
        assert!(!point_visible(Point::new(400.0, 625.0), &[below, above]));
    }
}