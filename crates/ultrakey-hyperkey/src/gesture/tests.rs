//! 제스처 상태 머신 단위 테스트 — 명세 §3.2 상태 전이 표와 §8 수용 기준의 순수 로직
//! 부분을 **비공개 FFI 없이** 재현한다. 시각은 전부 주입값이다.

use super::GestureFrame;
use crate::gesture::{GestureMachine, GestureParams, GestureTouch, GestureVerdict};
use ultrakey_core::time::Millis;
use ultrakey_core::trackpad::TrackpadPhase;
use crate::TrackpadArea;

/// 표면 크기 가정: 150mm × 80mm — 테스트 전체가 이 비율을 고정한다.
const W: f64 = 150.0;
const H: f64 = 80.0;

fn touch(id: i32, stage: i32, x: f64, y: f64) -> GestureTouch {
    GestureTouch {
        path_index: id,
        stage,
        x,
        y,
        mm_x: x * W,
        mm_y: y * H,
        major_axis: 10.0,
        z_total: 0.6,
    }
}

/// 진입 영역 패치 중심 — 패치 안쪽 절반 지점을 고른다.
fn entry_point(area: TrackpadArea, patch: f64) -> (f64, f64) {
    match area {
        TrackpadArea::Top => (0.5, 1.0 - patch * 0.5),
        TrackpadArea::TopLeft => (patch * 0.5, 1.0 - patch * 0.5),
        TrackpadArea::TopRight => (1.0 - patch * 0.5, 1.0 - patch * 0.5),
        TrackpadArea::BottomLeft => (patch * 0.5, patch * 0.5),
        TrackpadArea::BottomRight => (1.0 - patch * 0.5, patch * 0.5),
    }
}

fn params(area: TrackpadArea) -> GestureParams {
    GestureParams {
        area,
        ..GestureParams::default()
    }
}

// ────────────────────────────────────────────────────────────────────────
// §3.2 상태 머신 — 대표 전이
// ────────────────────────────────────────────────────────────────────────

/// Idle → ZoneContact — 영역 안에서 손가락 1개가 새로 닿으면 시작한다.
#[test]
fn idle_to_zone_contact_on_entry_area_make_touch() {
    let p = params(TrackpadArea::TopRight);
    let (ex, ey) = entry_point(TrackpadArea::TopRight, p.entry_patch_frac);
    let mut m = GestureMachine::new(p);
    let frame = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, ex, ey)],
    };
    assert_eq!(m.on_frame(&frame, Millis(0)), GestureVerdict::ZoneEntered);
    assert_eq!(m.phase(), TrackpadPhase::Contact);
}

/// 영역 밖 시작 접촉은 무시한다(§8 수용 기준 3).
#[test]
fn contact_outside_zone_is_ignored() {
    let mut m = GestureMachine::new(params(TrackpadArea::TopRight));
    // 중앙 — 어느 영역에도 해당하지 않는다.
    let frame = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, 0.5, 0.5)],
    };
    assert_eq!(m.on_frame(&frame, Millis(0)), GestureVerdict::Unchanged);
    assert_eq!(m.phase(), TrackpadPhase::Off);
}

/// 짧은 탭 — 임계 미도달 소멸은 아무 신호도 내지 않는다(§8 수용 기준 3, 시나리오 C).
#[test]
fn short_touch_never_engages() {
    let p = params(TrackpadArea::TopRight);
    let (ex, ey) = entry_point(p.area, p.entry_patch_frac);
    let mut m = GestureMachine::new(p);

    let down = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, ex, ey)],
    };
    assert_eq!(m.on_frame(&down, Millis(0)), GestureVerdict::ZoneEntered);

    // 살짝만 밀고(프리즈 임계 미만) 뗀다.
    let tiny = touch(1, GestureTouch::STAGE_TOUCHING, ex, ey - 0.01);
    let frame2 = GestureFrame { touches: &[tiny] };
    assert_eq!(m.on_frame(&frame2, Millis(50)), GestureVerdict::Unchanged);

    let up = GestureFrame { touches: &[] };
    assert_eq!(m.on_frame(&up, Millis(60)), GestureVerdict::Cancelled);
    assert_eq!(m.phase(), TrackpadPhase::Off);
}

/// 2손가락 취소(§8 수용 기준 4) — 트리거 도달 전 두 번째 유효 접촉.
#[test]
fn second_finger_cancels_before_trigger() {
    let p = params(TrackpadArea::TopRight);
    let (ex, ey) = entry_point(p.area, p.entry_patch_frac);
    let mut m = GestureMachine::new(p);

    let down = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, ex, ey)],
    };
    assert_eq!(m.on_frame(&down, Millis(0)), GestureVerdict::ZoneEntered);

    // 두 번째 손가락 추가 — 즉시 취소.
    let frame2 = GestureFrame {
        touches: &[
            touch(1, GestureTouch::STAGE_TOUCHING, ex, ey),
            touch(2, GestureTouch::STAGE_MAKE_TOUCH, 0.5, 0.5),
        ],
    };
    assert_eq!(m.on_frame(&frame2, Millis(30)), GestureVerdict::Cancelled);
    assert_eq!(m.phase(), TrackpadPhase::Off);
}

/// ZoneContact → CursorFrozen → Engaged → Released 의 완주(§3.2 표 전체).
#[test]
fn full_gesture_freezes_then_engages_then_releases() {
    let area = TrackpadArea::TopRight;
    let mut m = GestureMachine::new(params(area));
    let (ex, ey) = entry_point(area, m.params().entry_patch_frac);

    let down = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, ex, ey)],
    };
    assert_eq!(m.on_frame(&down, Millis(0)), GestureVerdict::ZoneEntered);

    // 1단계 — 프리즈 임계(코너 3mm)만큼 중심 대각선 방향으로 이동한다.
    let diag = std::f64::consts::FRAC_1_SQRT_2;
    let freeze = m.params().corner_freeze_mm + 0.1; // 부동소수 여유
    let fx = ex - freeze * diag / W;
    let fy = ey - freeze * diag / H;
    let frame = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_TOUCHING, fx, fy)],
    };
    assert_eq!(m.on_frame(&frame, Millis(20)), GestureVerdict::CursorFrozen);
    assert_eq!(m.phase(), TrackpadPhase::Frozen);

    // 2단계 — 트리거 임계(코너 6mm)까지 누적 이동(시작점 기준 대각선).
    let trigger = m.params().corner_trigger_mm + 0.1;
    let tx = ex - trigger * diag / W;
    let ty = ey - trigger * diag / H;
    let frame2 = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_TOUCHING, tx, ty)],
    };
    assert_eq!(
        m.on_frame(&frame2, Millis(50)),
        GestureVerdict::HyperEngaged
    );
    assert_eq!(m.phase(), TrackpadPhase::Engaged);

    // 접촉 유지 — Engaged 유지.
    let hold = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_TOUCHING, tx, ty)],
    };
    assert_eq!(m.on_frame(&hold, Millis(80)), GestureVerdict::Unchanged);

    // finger up — 해제(§8 수용 기준 2).
    let empty = GestureFrame { touches: &[] };
    assert_eq!(
        m.on_frame(&empty, Millis(120)),
        GestureVerdict::HyperReleased
    );
    assert_eq!(m.phase(), TrackpadPhase::Off);
}

/// GESTURE_TIMEOUT — 임계 미도달 접촉은 시간이 지나면 폐기된다(§3.2 ZoneContact 표).
#[test]
fn gesture_timeout_discards_stalled_contact() {
    let area = TrackpadArea::Top;
    let p = params(area);
    let mut m = GestureMachine::new(p);
    let (ex, ey) = entry_point(area, m.params().entry_patch_frac);

    let down = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, ex, ey)],
    };
    assert_eq!(m.on_frame(&down, Millis(0)), GestureVerdict::ZoneEntered);

    // 조금 움직였지만 임계 미달 — Unchanged.
    let ny = ey - 1.0 / H;
    let frame = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_TOUCHING, 0.5, ny)],
    };
    assert_eq!(m.on_frame(&frame, Millis(200)), GestureVerdict::Unchanged);

    // 타임아웃 경과 후 같은 프레임 — Cancelled 로 Idle 복귀.
    let late = Millis(m.params().gesture_timeout_ms + 1);
    assert_eq!(m.on_frame(&frame, late), GestureVerdict::Cancelled);
    assert_eq!(m.phase(), TrackpadPhase::Off);
}

/// 강제 해제 — 설정 토글 OFF(§5 항목 12)·장치 무효화(§5 항목 3·14).
#[test]
fn force_release_engaged_publishes_release_signal() {
    let area = TrackpadArea::TopRight;
    let mut m = GestureMachine::new(params(area));
    let (ex, ey) = entry_point(area, m.params().entry_patch_frac);

    let down = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, ex, ey)],
    };
    m.on_frame(&down, Millis(0));

    // 트리거 임계(6mm 대각) 이상 안쪽 이동 — 즉시 Engaged. 부동소수 여유 +0.1mm.
    let trigger = m.params().corner_trigger_mm + 0.1;
    let diag = std::f64::consts::FRAC_1_SQRT_2;
    let tx = ex - trigger * diag / W;
    let ty = ey - trigger * diag / H;
    let engage = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_TOUCHING, tx, ty)],
    };
    assert_eq!(m.on_frame(&engage, Millis(10)), GestureVerdict::HyperEngaged);

    // Engaged 도중 설정이 꺼졌다 → 강제 해제(§8).
    assert_eq!(m.force_release(), GestureVerdict::HyperReleased);
    assert_eq!(m.phase(), TrackpadPhase::Off);

    // Engaged 가 아닐 때의 강제 해제는 조용하다.
    assert_eq!(m.force_release(), GestureVerdict::Unchanged);
}

/// 거부 로직 — 손바닥(큰 접촉)·스침(가벼운 접촉)은 유효 접촉에서 제외된다(§3.1).
#[test]
fn palm_and_light_touches_are_rejected() {
    let p = params(TrackpadArea::Top);
    let mut palm = touch(1, GestureTouch::STAGE_MAKE_TOUCH, 0.5, 0.98);
    palm.major_axis = 30.0; // 손바닥
    assert!(p.rejects(&palm));

    let mut light = touch(2, GestureTouch::STAGE_MAKE_TOUCH, 0.98, 0.5);
    light.z_total = 0.01;
    assert!(p.rejects(&light));

    let finger = touch(3, GestureTouch::STAGE_MAKE_TOUCH, 0.98, 0.98);
    assert!(!p.rejects(&finger));
}

/// 손바닥이 함께 닿아도 유효 접촉이 1개면 제스처는 시작된다(§3.1 "유효 접촉" 재정의).
#[test]
fn valid_touch_count_excludes_rejected_contacts() {
    let area = TrackpadArea::Top;
    let p = params(area);
    let (ex, ey) = entry_point(area, p.entry_patch_frac);
    let mut m = GestureMachine::new(p);

    let mut palm = touch(9, GestureTouch::STAGE_TOUCHING, 0.5, 0.5);
    palm.major_axis = 40.0; // 손바닥 — 거부 대상
    let down = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, ex, ey), palm],
    };
    // 유효 접촉은 손가락 1개뿐 — 시작된다(§3.1 "거부된 접촉을 제외한 유효 접촉").
    assert_eq!(m.on_frame(&down, Millis(0)), GestureVerdict::ZoneEntered);
}

/// 허용각 밖 이동은 안쪽으로 인정하지 않는다(§3.3 — 코너 "in" 해석).
#[test]
fn sideways_move_does_not_trigger() {
    let area = TrackpadArea::Top;
    let mut m = GestureMachine::new(params(area));
    // 상단 밴드에 진입해 수평으로만 민다.
    let frame = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, 0.5, 0.97)],
    };
    assert_eq!(m.on_frame(&frame, Millis(0)), GestureVerdict::ZoneEntered);

    // 수평 15mm — 허용각(±30°) 밖, 프리즈(4mm)·트리거(9mm) 모두 미인정.
    let moved = touch(1, GestureTouch::STAGE_TOUCHING, 0.6, 0.97);
    let frame2 = GestureFrame { touches: &[moved] };
    assert_eq!(m.on_frame(&frame2, Millis(50)), GestureVerdict::Unchanged);
    assert_eq!(m.phase(), TrackpadPhase::Contact);
}

/// 코너 임계와 상단 임계가 서로 다른 값이다(§3.2.1 2계열 구조).
#[test]
fn corner_and_top_thresholds_are_separate() {
    let corner = params(TrackpadArea::TopRight);
    let top = params(TrackpadArea::Top);
    assert_ne!(corner.trigger_mm(), top.trigger_mm());
    assert_ne!(corner.freeze_mm(), top.freeze_mm());
}

/// 프리즈 → 트리거 2단계가 서로 다른 프레임에서 일어난다(§3.2.1).
#[test]
fn freeze_precedes_trigger_as_separate_steps() {
    let area = TrackpadArea::Top;
    let mut m = GestureMachine::new(params(area));

    let down = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, 0.5, 0.97)],
    };
    assert_eq!(m.on_frame(&down, Millis(0)), GestureVerdict::ZoneEntered);

    // 5mm 하향 = 프리즈(4mm) 초과, 트리거(9mm) 미달. 시작점(0.97) 기준 누적.
    let p5 = touch(1, GestureTouch::STAGE_TOUCHING, 0.5, 0.97 - 5.0 / H);
    let frame2 = GestureFrame { touches: &[p5] };
    assert_eq!(m.on_frame(&frame2, Millis(30)), GestureVerdict::CursorFrozen);
    assert_eq!(m.phase(), TrackpadPhase::Frozen);

    // 10mm 하향(누적, 시작점 기준) — 트리거.
    let p9 = touch(1, GestureTouch::STAGE_TOUCHING, 0.5, 0.97 - 10.0 / H);
    let frame3 = GestureFrame { touches: &[p9] };
    assert_eq!(
        m.on_frame(&frame3, Millis(80)),
        GestureVerdict::HyperEngaged
    );
    assert_eq!(m.phase(), TrackpadPhase::Engaged);
}

/// 프리즈 도달 전 접촉 소멸은 커서 고정을 푼다(Frozen → Idle, phase Off).
#[test]
fn frozen_touch_up_returns_to_idle() {
    let mut m = GestureMachine::new(params(TrackpadArea::Top));
    let down = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, 0.5, 0.97)],
    };
    m.on_frame(&down, Millis(0));

    let p5 = touch(1, GestureTouch::STAGE_TOUCHING, 0.5, 0.97 - 5.0 / H);
    m.on_frame(&GestureFrame { touches: &[p5] }, Millis(30));
    assert_eq!(m.phase(), TrackpadPhase::Frozen);

    // 트리거 전 손가락을 뗀다 — §5 "제스처 미완성".
    let empty = GestureFrame { touches: &[] };
    assert_eq!(m.on_frame(&empty, Millis(60)), GestureVerdict::Cancelled);
    assert_eq!(m.phase(), TrackpadPhase::Off);
}

/// Engaged 도중 두 번째 접촉이 닿아도 유지한다(§3.2 표 Engaged 행 `(추정)` — 유지 채택,
/// §9 #6 근거 기록).
#[test]
fn engaged_survives_extra_touch() {
    let area = TrackpadArea::Top;
    let mut m = GestureMachine::new(params(area));
    let down = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_MAKE_TOUCH, 0.5, 0.97)],
    };
    m.on_frame(&down, Millis(0));
    let p9 = touch(1, GestureTouch::STAGE_TOUCHING, 0.5, 0.97 - 10.0 / H);
    let engage = GestureFrame { touches: &[p9] };
    assert_eq!(m.on_frame(&engage, Millis(10)), GestureVerdict::HyperEngaged);

    // 두 번째 접촉이 닿아도 유지.
    let frame2 = GestureFrame {
        touches: &[touch(1, GestureTouch::STAGE_TOUCHING, 0.5, 0.85), touch(2, GestureTouch::STAGE_MAKE_TOUCH, 0.3, 0.3)],
    };
    assert_eq!(m.on_frame(&frame2, Millis(100)), GestureVerdict::Unchanged);
    assert_eq!(m.phase(), TrackpadPhase::Engaged);

    // 원래 접촉이 떠지면 해제한다.
    let other_only = GestureFrame {
        touches: &[touch(2, GestureTouch::STAGE_TOUCHING, 0.3, 0.3)],
    };
    assert_eq!(
        m.on_frame(&other_only, Millis(120)),
        GestureVerdict::HyperReleased
    );
}