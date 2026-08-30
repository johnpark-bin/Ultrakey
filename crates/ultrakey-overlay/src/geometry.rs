//! 다중 디스플레이 좌표 변환 — 전역 좌표 ↔ 창-로컬 좌표.
//!
//! ⭐ 이 모듈은 **안 A**(디스플레이마다 창 하나씩)를 전제한다. 명세 §3.3 이
//! 원래 채택했던 안 B(전 디스플레이를 덮는 단일 창)를 뒤집은 것이고, 그
//! **재결정은 명세 §3.3 "재결정" 절에 근거와 함께 기록돼 있다**(이슈 #34).
//! 요지 둘: (1) 단일 창은 backing store 가 하나라서 배율이 다른 디스플레이가
//! 섞이면 한쪽이 반드시 리샘플링된다 — §8 의 배율 수용 기준을 구조적으로
//! 만족할 수 없다. (2) 안 A 의 유일한 알려진 결함(경계를 넘는 연결선)은
//! [`clip_segment`] 로 없앨 수 있고, 그것은 **단위 테스트로 회귀를 막을 수
//! 있는** 종류의 해법이다.

use crate::model::Segment;
use ultrakey_seek::Rect;

/// 오버레이가 덮는 디스플레이 하나. 좌표는 전역 화면 좌표(pt, 좌상단 원점).
///
/// 안 A(디스플레이별 창)에서는 이 프레임이 곧 그 디스플레이 오버레이 창의
/// 프레임이다 — 창 하나가 디스플레이 하나에 정확히 겹친다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OverlayDisplay {
    /// `CGDirectDisplayID`.
    pub display_id: u32,
    /// 이 디스플레이의 전역 프레임(= 이 디스플레이 오버레이 창의 프레임).
    pub frame: Rect,
    /// `backingScaleFactor`. 이 모듈의 좌표 계산에는 쓰이지 않는다 — 좌표는
    /// 항상 포인트 단위이고 배율 합성은 창 서버가 처리한다(명세 §3.3 구현
    /// 함의). 렌더러가 두께 상수 보정 등에 참조할 수 있게 여기 보관한다.
    pub backing_scale: f64,
}

/// 전 디스플레이를 덮는 최소 사각형(union frame). 빈 슬라이스면 `None`.
///
/// 명세 §3.3 은 이 값을 안 B(단일 창)의 창 프레임으로 쓴다. 이 구현(안 A)
/// 에서는 창 프레임으로는 쓰이지 않지만, 핫플러그 시 "전체 배치가 어디까지
/// 뻗어 있는가"를 확인하는 진단·검색 바 초기 배치 상한 검사 등에 여전히
/// 유용해 남겨 둔다.
#[must_use]
pub fn union_frame(displays: &[OverlayDisplay]) -> Option<Rect> {
    let mut iter = displays.iter();
    let first = iter.next()?;
    let mut min_x = first.frame.x;
    let mut min_y = first.frame.y;
    let mut max_x = first.frame.x + first.frame.width;
    let mut max_y = first.frame.y + first.frame.height;

    for d in iter {
        min_x = min_x.min(d.frame.x);
        min_y = min_y.min(d.frame.y);
        max_x = max_x.max(d.frame.x + d.frame.width);
        max_y = max_y.max(d.frame.y + d.frame.height);
    }

    Some(Rect {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    })
}

/// 전역 좌표 사각형을 그 디스플레이 창의 로컬 좌표(창 좌상단 기준)로 옮긴다.
///
/// 안 A(디스플레이별 창)에서는 각 창의 원점이 곧 그 디스플레이의 전역 프레임
/// 원점이므로, 변환은 단순한 오프셋 뺄셈이다 — 크기는 배율과 무관하게 그대로
/// 옮긴다(명세 §3.3 구현 함의: 좌표는 항상 포인트 단위).
#[must_use]
pub fn to_local(display: &OverlayDisplay, global: &Rect) -> Rect {
    Rect {
        x: global.x - display.frame.x,
        y: global.y - display.frame.y,
        width: global.width,
        height: global.height,
    }
}

/// 전역 좌표 점 `(x, y)` 를 품은 디스플레이를 찾는다. 어디에도 없으면 `None`.
///
/// 겹치는 디스플레이 배치는 실기기에서 일어나지 않는다고 가정하고, 겹치는
/// 경우엔 목록에서 먼저 나오는 디스플레이를 우선한다.
#[must_use]
pub fn display_for_point(displays: &[OverlayDisplay], x: f64, y: f64) -> Option<&OverlayDisplay> {
    displays.iter().find(|d| {
        x >= d.frame.x
            && x < d.frame.x + d.frame.width
            && y >= d.frame.y
            && y < d.frame.y + d.frame.height
    })
}

/// 전역 좌표 선분을 이 디스플레이 창의 경계로 자르고, 결과를 그 창의 로컬
/// 좌표로 돌려준다. 교차하지 않으면 `None`.
///
/// ⭐ **이 크레이트에서 가장 중요한 함수다.**
///
/// 명세 §3.3 은 안 B(전 디스플레이를 덮는 단일 창)를 채택했는데, 그 이유가
/// 정확히 이 함수가 푸는 문제였다 — "안 A(디스플레이별 창)의 유일한 알려진
/// 결함은, 연결선이 디스플레이 경계를 넘을 때 어느 캔버스에도 선의 중간
/// 구간이 존재하지 않는다는 것"이다(§1 의 개발자 인용 "다른 디스플레이의
/// 매치로는 선을 그리지 못했다", v1.55 체인지로그 "Fixed broken Seek
/// behavior on additional displays"가 그 결함의 수정 이력).
///
/// 이 구현은 안 A 를 채택하면서 그 결함을 **구조적으로** 없앤다: 연결선을
/// **전역 좌표에서 한 번만** 정의하고(`OverlaySession::frames` 참조), 창마다
/// 이 함수로 자기 몫만 잘라 그린다. 두 창이 각자 자른 조각을 전역 좌표로
/// 되돌려 이어 붙이면 항상 원래 선분과 정확히 일치한다 — 경계에서 끊기는
/// 중간 구간이 애초에 존재할 수 없는 구조다(아래 테스트
/// `two_pieces_across_boundary_reassemble_without_gap` 가 이 무결성을 직접
/// 검증한다).
///
/// **Liang–Barsky** 매개변수 클리핑 알고리즘으로 구현했다 — Cohen-Sutherland
/// 류의 코드 비트 분기 대신, 매개변수 구간 `[t0, t1]` 을 4개 경계(왼/오/아래/
/// 위)로 순서대로 좁혀 가는 방식이다. 이 방식은 수직·수평 선분(어느 한
/// 방향의 변화량이 0인 경우, Cohen-Sutherland 라면 기울기 계산에서 0 나눗셈
/// 위험이 있는 지점) 도 "그 경계와 평행하다"는 별도 분기(`p.abs() < EPSILON`)
/// 로 자연스럽게 처리되어 특별 취급이 필요 없다.
#[must_use]
pub fn clip_segment(display: &OverlayDisplay, seg: Segment) -> Option<Segment> {
    let dx = seg.x2 - seg.x1;
    let dy = seg.y2 - seg.y1;

    let x_min = display.frame.x;
    let x_max = display.frame.x + display.frame.width;
    let y_min = display.frame.y;
    let y_max = display.frame.y + display.frame.height;

    // 4 개 경계(왼/오/아래/위)에 대한 (p, q) 쌍. p<0 은 "선분이 그 경계
    // 안쪽으로 들어가는 방향", p>0 은 "바깥으로 나가는 방향", p==0 은 그
    // 경계와 평행(수직/수평 선분에서 자연히 발생).
    let checks = [
        (-dx, seg.x1 - x_min), // 왼쪽: x >= x_min
        (dx, x_max - seg.x1),  // 오른쪽: x <= x_max
        (-dy, seg.y1 - y_min), // 아래: y >= y_min
        (dy, y_max - seg.y1),  // 위: y <= y_max
    ];

    let mut t0 = 0.0_f64;
    let mut t1 = 1.0_f64;

    for (p, q) in checks {
        if p.abs() < f64::EPSILON {
            // 이 경계와 평행한 성분 — 시작점이 이미 그 경계 밖이면, 이
            // 선분은 그 경계와 나란히 달리기만 할 뿐 절대 안으로 들어오지
            // 않는다.
            if q < 0.0 {
                return None;
            }
            continue;
        }
        let r = q / p;
        if p < 0.0 {
            // 들어가는 방향 — 구간의 아래쪽 한계(t0)를 조인다.
            if r > t1 {
                return None;
            }
            if r > t0 {
                t0 = r;
            }
        } else {
            // 나가는 방향 — 구간의 위쪽 한계(t1)를 조인다.
            if r < t0 {
                return None;
            }
            if r < t1 {
                t1 = r;
            }
        }
    }

    if t0 > t1 {
        return None;
    }

    let gx1 = seg.x1 + t0 * dx;
    let gy1 = seg.y1 + t0 * dy;
    let gx2 = seg.x1 + t1 * dx;
    let gy2 = seg.y1 + t1 * dy;

    Some(Segment {
        x1: gx1 - display.frame.x,
        y1: gy1 - display.frame.y,
        x2: gx2 - display.frame.x,
        y2: gy2 - display.frame.y,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    fn display(id: u32, x: f64, y: f64, w: f64, h: f64) -> OverlayDisplay {
        OverlayDisplay {
            display_id: id,
            frame: Rect { x, y, width: w, height: h },
            backing_scale: 2.0,
        }
    }

    // ── union_frame ──────────────────────────────────────────────────────

    #[test]
    fn union_frame_empty_slice_is_none() {
        assert_eq!(union_frame(&[]), None);
    }

    #[test]
    fn union_frame_single_display_is_its_own_frame() {
        let d = display(1, 0.0, 0.0, 1920.0, 1080.0);
        let u = union_frame(&[d]).unwrap();
        assert_eq!(u, d.frame);
    }

    /// 실제 배치: 주 디스플레이(0,0,2880x1800) 왼쪽에 보조 디스플레이가
    /// (-2560,0,2560x1440)로 붙어 있다 — 보조 디스플레이 원점이 음수다.
    #[test]
    fn union_frame_negative_origin_secondary_display() {
        let primary = display(1, 0.0, 0.0, 2880.0, 1800.0);
        let secondary = display(2, -2560.0, 0.0, 2560.0, 1440.0);
        let u = union_frame(&[primary, secondary]).unwrap();
        assert!(approx_eq(u.x, -2560.0));
        assert!(approx_eq(u.y, 0.0));
        assert!(approx_eq(u.width, 2880.0 - (-2560.0)));
        assert!(approx_eq(u.height, 1800.0));
    }

    /// 세 디스플레이 — 위/아래로도 벗어나는 배치라 y 방향 union 도 검증한다.
    #[test]
    fn union_frame_three_displays_covers_all_extents() {
        let a = display(1, 0.0, 0.0, 1000.0, 1000.0);
        let b = display(2, 1000.0, -500.0, 1000.0, 1000.0);
        let c = display(3, -500.0, 200.0, 400.0, 2000.0);
        let u = union_frame(&[a, b, c]).unwrap();
        assert!(approx_eq(u.x, -500.0));
        assert!(approx_eq(u.y, -500.0));
        assert!(approx_eq(u.x + u.width, 2000.0));
        assert!(approx_eq(u.y + u.height, 2200.0));
    }

    // ── to_local ─────────────────────────────────────────────────────────

    /// 원점이 음수인 디스플레이에서 전역 (-2000, 300) 이 로컬 (560, 300) 이 되는가.
    #[test]
    fn to_local_negative_origin_display() {
        let d = display(2, -2560.0, 0.0, 2560.0, 1440.0);
        let global = Rect { x: -2000.0, y: 300.0, width: 50.0, height: 20.0 };
        let local = to_local(&d, &global);
        assert!(approx_eq(local.x, 560.0));
        assert!(approx_eq(local.y, 300.0));
        assert!(approx_eq(local.width, 50.0));
        assert!(approx_eq(local.height, 20.0));
    }

    // ── display_for_point ────────────────────────────────────────────────

    #[test]
    fn display_for_point_finds_containing_display() {
        let a = display(1, 0.0, 0.0, 1000.0, 1000.0);
        let b = display(2, 1000.0, 0.0, 1000.0, 1000.0);
        let displays = [a, b];
        assert_eq!(display_for_point(&displays, 1500.0, 500.0).unwrap().display_id, 2);
        assert_eq!(display_for_point(&displays, 500.0, 500.0).unwrap().display_id, 1);
    }

    #[test]
    fn display_for_point_outside_all_is_none() {
        let a = display(1, 0.0, 0.0, 1000.0, 1000.0);
        assert!(display_for_point(&[a], 5000.0, 5000.0).is_none());
    }

    // ── clip_segment ─────────────────────────────────────────────────────

    #[test]
    fn clip_segment_fully_inside_is_shifted_to_local_unchanged() {
        let d = display(1, 100.0, 50.0, 1000.0, 1000.0);
        let seg = Segment { x1: 200.0, y1: 150.0, x2: 300.0, y2: 250.0 };
        let clipped = clip_segment(&d, seg).unwrap();
        assert!(approx_eq(clipped.x1, 100.0));
        assert!(approx_eq(clipped.y1, 100.0));
        assert!(approx_eq(clipped.x2, 200.0));
        assert!(approx_eq(clipped.y2, 200.0));
    }

    #[test]
    fn clip_segment_fully_outside_is_none() {
        let d = display(1, 0.0, 0.0, 1000.0, 1000.0);
        let seg = Segment { x1: 2000.0, y1: 2000.0, x2: 3000.0, y2: 3000.0 };
        assert!(clip_segment(&d, seg).is_none());
    }

    #[test]
    fn clip_segment_one_end_inside_one_outside() {
        let d = display(1, 0.0, 0.0, 1000.0, 1000.0);
        // 안쪽 (500,500) 에서 시작해 오른쪽 밖 (1500,500) 으로 나간다.
        let seg = Segment { x1: 500.0, y1: 500.0, x2: 1500.0, y2: 500.0 };
        let clipped = clip_segment(&d, seg).unwrap();
        assert!(approx_eq(clipped.x1, 500.0));
        assert!(approx_eq(clipped.y1, 500.0));
        assert!(approx_eq(clipped.x2, 1000.0)); // 로컬 — 경계에서 잘림
        assert!(approx_eq(clipped.y2, 500.0));
    }

    /// ⭐ v1.55 회귀 방지 테스트 — 두 디스플레이를 가로지르는 선분을 각각
    /// `clip_segment` 로 자른 두 조각을, 전역 좌표로 되돌려 이어 붙이면 원래
    /// 선분과 정확히 일치해야 한다(경계에서 끊기지 않는다).
    #[test]
    fn two_pieces_across_boundary_reassemble_without_gap() {
        let left = display(1, 0.0, 0.0, 1000.0, 1000.0);
        let right = display(2, 1000.0, 0.0, 1000.0, 1000.0);
        let seg = Segment { x1: 500.0, y1: 500.0, x2: 1500.0, y2: 700.0 };

        let left_piece = clip_segment(&left, seg).unwrap();
        let right_piece = clip_segment(&right, seg).unwrap();

        // 각 조각을 전역 좌표로 되돌린다.
        let left_end_global = (left_piece.x2 + left.frame.x, left_piece.y2 + left.frame.y);
        let right_start_global = (right_piece.x1 + right.frame.x, right_piece.y1 + right.frame.y);

        // 왼쪽 조각의 끝점과 오른쪽 조각의 시작점이 경계(x=1000)에서 정확히 맞물린다.
        assert!(approx_eq(left_end_global.0, 1000.0));
        assert!(approx_eq(right_start_global.0, 1000.0));
        assert!(approx_eq(left_end_global.1, right_start_global.1));

        // 왼쪽 조각의 시작점(전역)이 원래 선분의 시작점과 같다.
        assert!(approx_eq(left_piece.x1 + left.frame.x, seg.x1));
        assert!(approx_eq(left_piece.y1 + left.frame.y, seg.y1));
        // 오른쪽 조각의 끝점(전역)이 원래 선분의 끝점과 같다.
        assert!(approx_eq(right_piece.x2 + right.frame.x, seg.x2));
        assert!(approx_eq(right_piece.y2 + right.frame.y, seg.y2));
    }

    /// 수직 선분(dx=0) — Liang–Barsky 의 p=0 분기가 왼/오 경계에서 걸린다.
    #[test]
    fn clip_segment_vertical_line_no_divide_by_zero() {
        let d = display(1, 0.0, 0.0, 1000.0, 1000.0);
        let seg = Segment { x1: 500.0, y1: -100.0, x2: 500.0, y2: 1500.0 };
        let clipped = clip_segment(&d, seg).unwrap();
        assert!(approx_eq(clipped.x1, 500.0));
        assert!(approx_eq(clipped.y1, 0.0));
        assert!(approx_eq(clipped.x2, 500.0));
        assert!(approx_eq(clipped.y2, 1000.0));
    }

    /// 수평 선분(dy=0) — 위/아래 경계에서 p=0 분기가 걸린다.
    #[test]
    fn clip_segment_horizontal_line_no_divide_by_zero() {
        let d = display(1, 0.0, 0.0, 1000.0, 1000.0);
        let seg = Segment { x1: -100.0, y1: 500.0, x2: 1500.0, y2: 500.0 };
        let clipped = clip_segment(&d, seg).unwrap();
        assert!(approx_eq(clipped.x1, 0.0));
        assert!(approx_eq(clipped.y1, 500.0));
        assert!(approx_eq(clipped.x2, 1000.0));
        assert!(approx_eq(clipped.y2, 500.0));
    }

    /// 수직선이 디스플레이 x-범위 밖 — p=0(평행)이고 q<0 이므로 즉시 `None`.
    #[test]
    fn clip_segment_vertical_line_outside_x_range_is_none() {
        let d = display(1, 0.0, 0.0, 1000.0, 1000.0);
        let seg = Segment { x1: 2000.0, y1: -100.0, x2: 2000.0, y2: 1500.0 };
        assert!(clip_segment(&d, seg).is_none());
    }

    /// 디스플레이 사이에 빈틈(gap)이 있는 배치 — 선분이 두 디스플레이 사이
    /// 빈 영역을 지나가면 그 영역은 어느 창에도 그려지지 않되, 각 디스플레이
    /// 안쪽 구간은 정확히 자기 창 경계에서 잘려야 한다.
    #[test]
    fn clip_segment_with_gap_between_displays() {
        let left = display(1, 0.0, 0.0, 1000.0, 1000.0);
        let right = display(2, 2000.0, 0.0, 1000.0, 1000.0); // 1000~2000 사이가 빈틈
        let seg = Segment { x1: 500.0, y1: 500.0, x2: 2500.0, y2: 500.0 };

        let left_piece = clip_segment(&left, seg).unwrap();
        assert!(approx_eq(left_piece.x1, 500.0));
        assert!(approx_eq(left_piece.x2, 1000.0)); // 로컬 — 자기 창 경계에서 멈춘다

        let right_piece = clip_segment(&right, seg).unwrap();
        assert!(approx_eq(right_piece.x1, 0.0)); // 로컬 — 자기 창 시작부터
        assert!(approx_eq(right_piece.x2, 500.0));
    }

    /// 선분과 아예 무관한 위치(다른 디스플레이 사이 빈틈이 아니라, 선분 경로
    /// 자체에서 완전히 벗어난 디스플레이)는 `None`이어야 한다.
    #[test]
    fn clip_segment_display_far_from_segment_path_is_none() {
        let elsewhere = display(3, 5000.0, 5000.0, 100.0, 100.0);
        let seg = Segment { x1: 500.0, y1: 500.0, x2: 2500.0, y2: 500.0 };
        assert!(clip_segment(&elsewhere, seg).is_none());
    }
}

#[cfg(test)]
mod boundary_regression_tests {
    use super::*;
    use crate::model::Segment;

    fn d(id: u32, x: f64, y: f64, w: f64, h: f64) -> OverlayDisplay {
        OverlayDisplay {
            display_id: id,
            frame: Rect { x, y, width: w, height: h },
            backing_scale: 1.0,
        }
    }

    /// ⭐ **실기기에서 관측된 배치 그대로** — 주 디스플레이가 `(0,0) 3840×1600`,
    /// 보조가 `(-2560,0) 2560×1440`. 검색 바가 보조 화면에 있고 선택 매치가
    /// 주 화면에 있을 때, 두 창이 각각 자기 몫을 그려 **경계에서 이어져야**
    /// 한다(v1.55 "Fixed broken Seek behavior on additional displays" 회귀 방지).
    #[test]
    fn real_dual_display_line_meets_at_boundary() {
        let d1 = d(3, 0.0, 0.0, 3840.0, 1600.0);
        let d2 = d(2, -2560.0, 0.0, 2560.0, 1440.0);
        // 검색 바 하단 중앙(실측 위치 (-1480, 288) + (200, 40)).
        let seg = Segment { x1: -1280.0, y1: 328.0, x2: 1500.0, y2: 700.0 };

        let c2 = clip_segment(&d2, seg).expect("보조 화면 몫이 있어야 한다");
        let c1 = clip_segment(&d1, seg).expect("⭐ 주 화면 몫도 있어야 한다 — 없으면 선이 끊긴다");

        // 보조 화면의 끝점은 그 창의 오른쪽 끝(로컬 x = 2560)이다.
        assert!((c2.x2 - 2560.0).abs() < 1e-6, "보조 화면 끝점 x={}", c2.x2);
        // 주 화면의 시작점은 그 창의 왼쪽 끝(로컬 x = 0)이다.
        assert!((c1.x1 - 0.0).abs() < 1e-6, "주 화면 시작점 x={}", c1.x1);
        // ⭐ 그리고 두 점의 y 가 같아야 한다 — 여기가 어긋나면 경계에서 꺾인다.
        assert!(
            (c2.y2 - c1.y1).abs() < 1e-6,
            "경계에서 y 가 어긋났다: 보조 {} vs 주 {}",
            c2.y2,
            c1.y1
        );
    }
}
