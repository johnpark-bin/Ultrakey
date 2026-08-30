//! §3.2.3 — 좌표 변환.
//!
//! Vision 이 돌려주는 `boundingBox` 는 **캡처 이미지 기준 정규화 좌표이고
//! 원점이 좌하단**이다. 이것을 AX(소스 B)와 같은 좌표계 — **전역 화면 좌표,
//! 포인트 단위, 좌상단 원점** — 으로 옮겨야 두 소스를 병합할 수 있다.
//!
//! ⚠️ 이 파일의 4단계(전역 오프셋)가 v1.55 에서 "다중 디스플레이에서 Seek
//! 동작이 깨짐" 으로 실제 버그가 났던 지점이다(명세 §3.2.3 경고, §5 #3).

/// 전역 화면 좌표(포인트, 좌상단 원점)의 사각형.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Rect {
    /// 좌상단 x (포인트).
    pub x: f64,
    /// 좌상단 y (포인트).
    pub y: f64,
    /// 폭 (포인트).
    pub width: f64,
    /// 높이 (포인트).
    pub height: f64,
}

impl Rect {
    /// 두 사각형의 교집합 넓이.
    #[must_use]
    pub fn intersection_area(&self, other: &Self) -> f64 {
        let x1 = self.x.max(other.x);
        let y1 = self.y.max(other.y);
        let x2 = (self.x + self.width).min(other.x + other.width);
        let y2 = (self.y + self.height).min(other.y + other.height);
        if x2 <= x1 || y2 <= y1 {
            0.0
        } else {
            (x2 - x1) * (y2 - y1)
        }
    }

    /// 넓이.
    #[must_use]
    pub fn area(&self) -> f64 {
        (self.width * self.height).max(0.0)
    }

    /// Intersection over Union.
    #[must_use]
    pub fn iou(&self, other: &Self) -> f64 {
        let inter = self.intersection_area(other);
        let union = self.area() + other.area() - inter;
        if union <= 0.0 {
            0.0
        } else {
            inter / union
        }
    }

    /// `self` 넓이 대비 교집합 비율 — "이 사각형이 상대에게 얼마나 먹혔는가".
    ///
    /// AX 매치는 요소 전체가 프레임이라 OCR 매치보다 훨씬 넓다(§3.3). 그런
    /// 쌍에서는 IoU 가 작게 나오므로 IoU 만으로는 중복을 못 잡는다 — 그래서
    /// 포함 관계를 이 비율로 따로 본다(§3.4 M2).
    #[must_use]
    pub fn containment_in(&self, other: &Self) -> f64 {
        let a = self.area();
        if a <= 0.0 {
            0.0
        } else {
            self.intersection_area(other) / a
        }
    }

    /// 중심점.
    #[must_use]
    pub fn center(&self) -> (f64, f64) {
        (self.x + self.width / 2.0, self.y + self.height / 2.0)
    }
}

/// 한 캡처의 기준틀 — 어느 디스플레이(또는 잘라낸 영역)의 이미지인가.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DisplayFrame {
    /// `CGDirectDisplayID`.
    pub display_id: u32,
    /// 이 캡처의 전역 원점 x (포인트). `CGDisplayBounds(displayID).origin.x`,
    /// 영역 캡처라면 그 사각형의 원점.
    pub origin_x: f64,
    /// 이 캡처의 전역 원점 y (포인트).
    pub origin_y: f64,
    /// 캡처 이미지의 픽셀 폭.
    pub image_width_px: f64,
    /// 캡처 이미지의 픽셀 높이.
    pub image_height_px: f64,
    /// 픽셀 → 포인트 배율(Retina 는 보통 2.0).
    pub scale: f64,
}

/// 정규화 `boundingBox`(좌하단 원점) → 전역 화면 좌표(좌상단 원점, 포인트).
///
/// 명세 §3.2.3 의 4단계 수식을 그대로 옮긴 것이다.
#[must_use]
pub fn normalized_bbox_to_global(
    frame: &DisplayFrame,
    bx: f64,
    by: f64,
    bw: f64,
    bh: f64,
) -> Rect {
    // 1) 좌하단 원점 → 좌상단 원점 (정규화 좌표 안에서 y 만 뒤집는다)
    let ty = 1.0 - by - bh;

    // 2) 정규화 → 이 캡처 이미지의 픽셀 좌표
    let px = bx * frame.image_width_px;
    let py = ty * frame.image_height_px;
    let pw = bw * frame.image_width_px;
    let ph = bh * frame.image_height_px;

    // 3) 픽셀 → 포인트 (Retina 배율 보정)
    //    ⚠️ scale 이 0 이면 나눗셈이 무한대가 되어 좌표가 통째로 망가진다.
    //    캡처가 없는 디스플레이에서 유도 배율이 0 이 될 수 있으므로 방어한다.
    let scale = if frame.scale > 0.0 { frame.scale } else { 1.0 };
    let qx = px / scale;
    let qy = py / scale;
    let qw = pw / scale;
    let qh = ph / scale;

    // 4) 디스플레이 로컬 포인트 → 전역 포인트 (다중 디스플레이 매핑)
    Rect {
        x: frame.origin_x + qx,
        y: frame.origin_y + qy,
        width: qw,
        height: qh,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 부동소수점 근사 비교 — 나눗셈이 섞인 결과를 하드코딩한 기대값과 대조할 때 쓴다.
    fn approx_eq(a: f64, b: f64) -> bool {
        (a - b).abs() < 1e-9
    }

    // ── §3.2.3 좌표 변환 수식 ────────────────────────────────────────────

    /// §3.2.3 수식 4단계 전부를 정규화 bbox (0.25, 0.5, 0.5, 0.25) 로 손계산해
    /// 하드코딩한 기대값과 대조한다. imgW=2000, imgH=1000, scale=2.0,
    /// origin=(100, 50) 을 골라 4단계 전부가 결과에 관여하게 했다.
    ///
    /// 손계산:
    /// 1) ty = 1 - 0.5 - 0.25 = 0.25, tx=0.25, tw=0.5, th=0.25
    /// 2) px = 0.25*2000=500, py=0.25*1000=250, pw=0.5*2000=1000, ph=0.25*1000=250
    /// 3) qx=500/2=250, qy=250/2=125, qw=1000/2=500, qh=250/2=125
    /// 4) gx=100+250=350, gy=50+125=175, gw=500, gh=125
    #[test]
    fn coordinate_formula_four_steps_hand_computed() {
        let frame = DisplayFrame {
            display_id: 1,
            origin_x: 100.0,
            origin_y: 50.0,
            image_width_px: 2000.0,
            image_height_px: 1000.0,
            scale: 2.0,
        };
        let rect = normalized_bbox_to_global(&frame, 0.25, 0.5, 0.5, 0.25);
        assert!(approx_eq(rect.x, 350.0));
        assert!(approx_eq(rect.y, 175.0));
        assert!(approx_eq(rect.width, 500.0));
        assert!(approx_eq(rect.height, 125.0));
    }

    /// §3.2.3 1단계 — y 뒤집기가 실제로 일어나는가. 이미지 아래쪽(by 가 작은)
    /// 관측이 전역 좌표(좌상단 원점)에서 더 큰 y 를 가져야 한다.
    #[test]
    fn y_axis_is_flipped_bottom_observation_has_larger_global_y() {
        let frame = DisplayFrame {
            display_id: 1,
            origin_x: 0.0,
            origin_y: 0.0,
            image_width_px: 1000.0,
            image_height_px: 1000.0,
            scale: 1.0,
        };
        // 이미지 하단 근처 관측 (by 작음 — Vision 원점은 좌하단)
        let bottom = normalized_bbox_to_global(&frame, 0.1, 0.0, 0.2, 0.1);
        // 이미지 상단 근처 관측 (by 가 1에 가까움)
        let top = normalized_bbox_to_global(&frame, 0.1, 0.9, 0.2, 0.1);
        assert!(
            bottom.y > top.y,
            "이미지 아래쪽 관측이 전역 좌표에서 더 큰 y 를 가져야 한다(좌상단 원점)"
        );
    }

    /// ⭐ Retina 배율(scale=2.0) — 이미지 2880x1800px, 논리 1440x900pt.
    /// 이 기기에는 Retina 디스플레이가 없어 실기기 확인이 불가능하므로 이
    /// 테스트가 유일한 검증 수단이다. 전체 이미지를 덮는 bbox 는 논리
    /// 해상도와 정확히 일치해야 한다.
    #[test]
    fn retina_scale_two_full_image_maps_to_logical_points() {
        let frame = DisplayFrame {
            display_id: 1,
            origin_x: 0.0,
            origin_y: 0.0,
            image_width_px: 2880.0,
            image_height_px: 1800.0,
            scale: 2.0,
        };
        let rect = normalized_bbox_to_global(&frame, 0.0, 0.0, 1.0, 1.0);
        assert!(approx_eq(rect.x, 0.0));
        assert!(approx_eq(rect.y, 0.0));
        assert!(approx_eq(rect.width, 1440.0));
        assert!(approx_eq(rect.height, 900.0));
    }

    /// ⭐ Retina 배율 — 부분 bbox 도 픽셀→포인트 나눗셈이 정확히 적용되는가.
    /// 손계산: bx=0.5,by=0.5,bw=0.1,bh=0.1 → ty=0.4 → px=1440,py=720,pw=288,ph=180
    /// → qx=720,qy=360,qw=144,qh=90 (scale=2 로 전부 나눔)
    #[test]
    fn retina_scale_two_partial_bbox_hand_computed() {
        let frame = DisplayFrame {
            display_id: 1,
            origin_x: 0.0,
            origin_y: 0.0,
            image_width_px: 2880.0,
            image_height_px: 1800.0,
            scale: 2.0,
        };
        let rect = normalized_bbox_to_global(&frame, 0.5, 0.5, 0.1, 0.1);
        assert!(approx_eq(rect.x, 720.0));
        assert!(approx_eq(rect.y, 360.0));
        assert!(approx_eq(rect.width, 144.0));
        assert!(approx_eq(rect.height, 90.0));
    }

    /// ⭐ §5 #3, v1.55 회귀 — 다중 디스플레이 오프셋. 같은 정규화 bbox 가
    /// 왼쪽 디스플레이(origin=(-2560,0), scale=1.0)와 주 디스플레이
    /// (origin=(0,0), scale=2.0)에서 서로 다른 전역 좌표로 가야 한다 —
    /// 배율까지 다른 경우를 포함한다.
    #[test]
    fn multi_display_offset_same_bbox_different_global_position() {
        let left = DisplayFrame {
            display_id: 1,
            origin_x: -2560.0,
            origin_y: 0.0,
            image_width_px: 2560.0,
            image_height_px: 1440.0,
            scale: 1.0,
        };
        let primary = DisplayFrame {
            display_id: 2,
            origin_x: 0.0,
            origin_y: 0.0,
            image_width_px: 2880.0,
            image_height_px: 1800.0,
            scale: 2.0,
        };
        let bx = 0.25;
        let by = 0.25;
        let bw = 0.1;
        let bh = 0.1;

        let left_rect = normalized_bbox_to_global(&left, bx, by, bw, bh);
        let primary_rect = normalized_bbox_to_global(&primary, bx, by, bw, bh);

        // 손계산 (왼쪽, scale=1.0): ty=0.65 → px=640,py=936,pw=256,ph=144
        // → qx=640,qy=936,qw=256,qh=144 → gx=-2560+640=-1920, gy=936
        assert!(approx_eq(left_rect.x, -1920.0));
        assert!(approx_eq(left_rect.y, 936.0));
        assert!(approx_eq(left_rect.width, 256.0));
        assert!(approx_eq(left_rect.height, 144.0));

        // 손계산 (주 디스플레이, scale=2.0): ty=0.65 → px=720,py=1170,pw=288,ph=180
        // → qx=360,qy=585,qw=144,qh=90 → gx=360, gy=585
        assert!(approx_eq(primary_rect.x, 360.0));
        assert!(approx_eq(primary_rect.y, 585.0));
        assert!(approx_eq(primary_rect.width, 144.0));
        assert!(approx_eq(primary_rect.height, 90.0));

        assert_ne!(left_rect, primary_rect);
    }

    /// §3.2.3 3단계 방어 — scale=0.0 이어도 0으로 나누지 않고 1.0 으로 폴백한다.
    #[test]
    fn zero_scale_falls_back_instead_of_dividing_by_zero() {
        let zero_scale = DisplayFrame {
            display_id: 1,
            origin_x: 0.0,
            origin_y: 0.0,
            image_width_px: 1000.0,
            image_height_px: 1000.0,
            scale: 0.0,
        };
        let one_scale = DisplayFrame {
            scale: 1.0,
            ..zero_scale
        };
        let rect_zero = normalized_bbox_to_global(&zero_scale, 0.2, 0.2, 0.3, 0.3);
        let rect_one = normalized_bbox_to_global(&one_scale, 0.2, 0.2, 0.3, 0.3);
        assert_eq!(rect_zero, rect_one, "scale=0.0 은 1.0 폴백과 동일한 결과를 내야 한다");
        assert!(rect_zero.x.is_finite());
        assert!(rect_zero.y.is_finite());
        assert!(rect_zero.width.is_finite());
        assert!(rect_zero.height.is_finite());
    }

    // ── Rect 기하 연산 ──────────────────────────────────────────────────

    /// 겹치지 않는 두 사각형 — IoU·포함 비율 모두 0.
    #[test]
    fn rect_no_overlap_gives_zero_iou_and_containment() {
        let a = Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 };
        let b = Rect { x: 20.0, y: 20.0, width: 10.0, height: 10.0 };
        assert!(approx_eq(a.intersection_area(&b), 0.0));
        assert!(approx_eq(a.iou(&b), 0.0));
        assert!(approx_eq(a.containment_in(&b), 0.0));
        assert!(approx_eq(b.containment_in(&a), 0.0));
    }

    /// 완전 포함 — 작은 사각형이 큰 사각형 안에 완전히 들어간 경우.
    #[test]
    fn rect_full_containment() {
        let outer = Rect { x: 0.0, y: 0.0, width: 100.0, height: 100.0 };
        let inner = Rect { x: 10.0, y: 10.0, width: 20.0, height: 20.0 };
        assert!(approx_eq(outer.intersection_area(&inner), 400.0));
        // inner 가 outer 안에 완전히 들어가므로 containment_in 은 1.0 이어야 한다.
        assert!(approx_eq(inner.containment_in(&outer), 1.0));
        // 반대 방향은 inner 넓이 비율만큼만.
        assert!(approx_eq(outer.containment_in(&inner), 400.0 / 10_000.0));
        // IoU = inter / (areaA + areaB - inter) = 400 / (10000 + 400 - 400)
        assert!(approx_eq(outer.iou(&inner), 400.0 / 10_000.0));
    }

    /// 부분 겹침 — 손계산: 교집합 25, 합집합 175, IoU = 25/175.
    #[test]
    fn rect_partial_overlap() {
        let a = Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 };
        let b = Rect { x: 5.0, y: 5.0, width: 10.0, height: 10.0 };
        assert!(approx_eq(a.intersection_area(&b), 25.0));
        assert!(approx_eq(a.iou(&b), 25.0 / 175.0));
    }

    /// 넓이 0인 사각형 — 나눗셈이 안전하게 0을 반환해야 한다.
    #[test]
    fn rect_zero_area_is_safe() {
        let zero = Rect { x: 5.0, y: 5.0, width: 0.0, height: 10.0 };
        let other = Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 };
        assert!(approx_eq(zero.area(), 0.0));
        assert!(approx_eq(zero.intersection_area(&other), 0.0));
        assert!(approx_eq(zero.iou(&other), 0.0));
        assert!(approx_eq(zero.containment_in(&other), 0.0));
    }

    /// 중심점 계산.
    #[test]
    fn rect_center() {
        let r = Rect { x: 2.0, y: 4.0, width: 6.0, height: 8.0 };
        let (cx, cy) = r.center();
        assert!(approx_eq(cx, 5.0));
        assert!(approx_eq(cy, 8.0));
    }
}
