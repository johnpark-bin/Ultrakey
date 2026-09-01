//! F-06 트랙패드 원터치 hyper 제스처 — **순수 상태 머신**(`trackpad-hyper-gesture.md`
//! §3.2·§3.2.1).
//!
//! ⭐ 이 모듈은 macOS 를 전혀 모른다 — 판정은 프레임(접촉 목록)과 호출자가 주입하는
//! 단조 시각만으로 이뤄진다. 그래서 비공개 FFI(`ultrakey-platform::multitouch`) 없이
//! 전부 단위 테스트된다. architecture.md §1 "unsafe FFI 와 순수 판정 로직을 물리적으로
//! 분리"의 F-06 실현이다.
//!
//! ## 상태 머신(§3.2 표를 그대로 옮긴 것)
//!
//! ```text
//! Idle ──(영역 안 finger down, 유효 접촉 1)──▶ ZoneContact
//! ZoneContact ──(프리즈 임계 도달)──▶ CursorFrozen ──(트리거 임계 도달)──▶ Engaged
//!      │                                   │                                  │
//!      └─(finger up · 2손가락 · 타임아웃)──▶ Idle(취소)                     │
//! Engaged ──(finger up)──▶ Idle + hyper 해제 신호 ◀────────────────────────┘
//! ```
//!
//! 2단계 임계의 근거는 §3.2.1(번들 문자열 실측 — `cornerFreezeThreshold` 등 **이름**
//! 확정, 수치 `(미확정)`)이다. freeze < trigger 순서는 논리적 추정을 채택했다.
//!
//! ## 게이트 계약(§3.4)
//!
//! 이 모듈은 `CGEvent` modifier 를 합성하지 않는다. [`GestureVerdict::HyperEngaged`]/
//! [`GestureVerdict::HyperReleased`] 를 받은 리스너가 `ultrakey-core::trackpad::
//! AtomicTrackpadPhase` 에 게시하면 중재기(`ultrakey-core::arbitration`)가 그 값을
//! 읽어 hyper 규칙 flags 를 OR 한다 — F-06 은 "요청"만 하고 modifier 합성은
//! F-05/F-07 의 몫이다.

use crate::TrackpadArea;
use ultrakey_core::time::Millis;
use ultrakey_core::trackpad::TrackpadPhase;

/// 접촉 하나의 스냅샷 — FFI 경계(`ultrakey-platform::multitouch::TouchPoint`)에서
/// 판정에 필요한 필드만 옮겨 온 값. 이 크레이트가 소유하는 순수 타입이다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GestureTouch {
    /// 터치의 수명 동안 안정적인 식별자(`pathIndex`).
    pub path_index: i32,
    /// `MTPathStage` — `MakeTouch`(3)·`Touching`(4)만 유효 접촉이다.
    pub stage: i32,
    /// 표면 정규 좌표(0..1) — 진입 영역 판정에 쓴다.
    pub x: f64,
    pub y: f64,
    /// 절대 좌표(mm) — 이동 거리·방향 판정에 쓴다(`absoluteVector`).
    pub mm_x: f64,
    pub mm_y: f64,
    /// 타원 주축(mm) — 손바닥 거부에 쓴다.
    pub major_axis: f64,
    /// `zTotal`(0..1) — 접촉 품질. 스침 거부에 쓴다.
    pub z_total: f64,
}

impl GestureTouch {
    /// `MTPathStage` 3·4 — 표면에 닿아 있는 상태(커뮤니티 헤더 전원 일치).
    pub const STAGE_MAKE_TOUCH: i32 = 3;
    pub const STAGE_TOUCHING: i32 = 4;
}

/// 프레임 하나 — 접촉 목록(이미 FFI 경계에서 안전한 값으로 번역된 것).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GestureFrame<'a> {
    pub touches: &'a [GestureTouch],
}

/// 제스처 판정 상수 세트. **이름**은 번들 문자열 실측 확정(§3.2.1), **수치**는 전부
/// 이 구현의 설계 판단이다(§4 — 실측 불가). 각 필드에 근거 구분을 남긴다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GestureParams {
    /// 진입 영역 5종(§4 실측 표시 순서). 저장 계층의 `TrackpadSettings::area` 를
    /// 그대로 받는다.
    pub area: TrackpadArea,
    /// `cornerFreezeThreshold` — 코너 프리즈 임계(mm). 3.0 `(미확정, 설계 판단)`.
    pub corner_freeze_mm: f64,
    /// `cornerTriggerThreshold` — 코너 트리거 임계(mm). 6.0 `(미확정, 설계 판단)`.
    pub corner_trigger_mm: f64,
    /// `oneTopCursorFreezeThreshold` — 상단 프리즈 임계(mm). 4.0 `(미확정, 설계
    /// 판단)` — 상단은 수직 밀어 넣기라 코너보다 여유가 필요하다는 UX 해석.
    pub top_freeze_mm: f64,
    /// `oneTopTriggerThreshold` — 상단 트리거 임계(mm). 9.0 `(미확정, 설계 판단)`.
    pub top_trigger_mm: f64,
    /// 진입 패치 크기(표면 한 변에 대한 비율). 0.12 `(추정 — 근거 없음)`; "진입 영역이
    /// 표면 크기에 대한 상대값"이라는 근거만 §6.2 심볼 존재가 뒷받침한다.
    pub entry_patch_frac: f64,
    /// "안쪽" 방향 허용각(도). ±30° `(추정 — §4 제안 채택, 근거 없음)`.
    pub direction_tolerance_deg: f64,
    /// 접촉 시작 후 임계 미도달 시 포기까지의 시간. 내부 이름 확인 안 됨 — 400ms
    /// `(추정)`(§4 300–500ms 제안의 중간값).
    pub gesture_timeout_ms: u64,
    /// 허용 유효 접촉 수 — **확인된 값 1**(§3.1 원문 직접 인용).
    pub max_valid_touches: usize,
    /// 손바닥 거부 주축 임계(mm). 22.0 `(추정)` — `largeTouches` 근거(§3.1).
    pub palm_major_axis_mm: f64,
    /// 스침 거부 품질 임계. 0.08 `(추정)` — `tooLightTouches` 근거(§3.1).
    pub light_z_total: f64,
}

impl Default for GestureParams {
    fn default() -> Self {
        GestureParams {
            area: TrackpadArea::TopRight,
            corner_freeze_mm: 3.0,
            corner_trigger_mm: 6.0,
            top_freeze_mm: 4.0,
            top_trigger_mm: 9.0,
            entry_patch_frac: 0.12,
            direction_tolerance_deg: 30.0,
            gesture_timeout_ms: 400,
            max_valid_touches: 1,
            palm_major_axis_mm: 22.0,
            light_z_total: 0.08,
        }
    }
}

impl GestureParams {
    /// 이 영역의 프리즈 임계(mm) — §3.2.1 2계열 구조(코너 `corner*` vs 상단
    /// `oneTop*`)을 코드로 옮긴다.
    pub fn freeze_mm(&self) -> f64 {
        if matches!(self.area, TrackpadArea::Top) {
            self.top_freeze_mm
        } else {
            self.corner_freeze_mm
        }
    }

    /// 상동 — 트리거 임계.
    pub fn trigger_mm(&self) -> f64 {
        if matches!(self.area, TrackpadArea::Top) {
            self.top_trigger_mm
        } else {
            self.corner_trigger_mm
        }
    }

    /// 접촉이 "유효"한가 — §3.1 팜/엄지/스침 거부의 이 구현이 채택한 방어선.
    /// `disablePalmRejection` 류 설정은 원본에서도 사용자 노출이 `(미확정)` 이라
    /// 만들지 않는다(§4 "의미 미확정 키").
    pub fn rejects(&self, touch: &GestureTouch) -> bool {
        !matches!(
            touch.stage,
            GestureTouch::STAGE_MAKE_TOUCH | GestureTouch::STAGE_TOUCHING
        ) || touch.major_axis > self.palm_major_axis_mm
            || touch.z_total < self.light_z_total
    }

    /// 프레임에서 유효 접촉만 걸러낸다(§3.1 "거부된 접촉을 제외한 유효 접촉" 재정의).
    pub fn valid_touches<'a>(
        &self,
        touches: &'a [GestureTouch],
    ) -> impl Iterator<Item = &'a GestureTouch> + use<'a> {
        let params = *self;
        touches
            .iter()
            .filter(move |t| !params.rejects(t))
    }
}

/// 상태 머신. 시각은 반드시 호출자가 주입한다(`Millis`) — 실제 시계를 부르지 않는다
/// (`ultrakey-core::time` 규약과 동일, 결정론적 단위 테스트의 전제).
#[derive(Debug, Clone, PartialEq)]
pub struct GestureMachine {
    params: GestureParams,
    state: State,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum State {
    Idle,
    ZoneContact {
        touch: i32,
        start_mm: (f64, f64),
        start_at: Millis,
    },
    CursorFrozen {
        touch: i32,
        start_mm: (f64, f64),
        start_at: Millis,
    },
    Engaged {
        touch: i32,
    },
}

/// 프레임 소비 결과 — 리스너가 게이트 게시 + F-05 신호로 번역한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureVerdict {
    /// 상태 변화 없음.
    Unchanged,
    /// 진입 영역에서 유효 접촉 1개가 시작됐다 — 부수효과 없다.
    ZoneEntered,
    /// 프리즈 임계 도달 — 리스너가 `Frozen` 을 게시한다(§8 마지막 항목).
    CursorFrozen,
    /// 트리거 임계 도달 — **hyper 활성 신호 방출**(§3.4).
    HyperEngaged,
    /// 접촉 종료 — 리스너가 `Off` 를 게시한다(§3.4 hyper 해제).
    HyperReleased,
    /// 임계 미도달 소멸·2손가락·타임아웃 — hyper 는 활성화되지 않았다(§8).
    Cancelled,
}

impl GestureMachine {
    pub fn new(params: GestureParams) -> Self {
        GestureMachine {
            params,
            state: State::Idle,
        }
    }

    pub fn params(&self) -> &GestureParams {
        &self.params
    }

    /// 현재 상태의 중재기 게이트 표현 — 리스너가 이대로 게시한다.
    pub fn phase(&self) -> TrackpadPhase {
        match self.state {
            State::Idle => TrackpadPhase::Off,
            State::ZoneContact { .. } => TrackpadPhase::Contact,
            State::CursorFrozen { .. } => TrackpadPhase::Frozen,
            State::Engaged { .. } => TrackpadPhase::Engaged,
        }
    }

    /// 프레임 하나를 소비한다. `now` 는 호출자가 주입하는 단조 밀리초다.
    pub fn on_frame(&mut self, frame: &GestureFrame<'_>, now: Millis) -> GestureVerdict {
        let valid: Vec<GestureTouch> =
            self.params.valid_touches(frame.touches).copied().collect();
        match self.state {
            State::Idle => self.on_idle(&valid, now),
            State::ZoneContact {
                touch,
                start_mm,
                start_at,
            } => self.on_zone_contact(touch, start_mm, start_at, &valid, now),
            State::CursorFrozen {
                touch,
                start_mm,
                start_at,
            } => self.on_frozen(touch, start_mm, start_at, &valid, now),
            State::Engaged { touch } => self.on_engaged(touch, &valid),
        }
    }

    /// 강제 해제 — 설정 토글 OFF(§5 항목 12)·장치 무효화(§3.2 Engaged 마지막 행,
    /// §5 항목 3·14)·워치독 공용 진입점. `Engaged` 였다면 hyper 해제 신호를 내고,
    /// 아니면 조용히 `Idle` 로 되돌아간다.
    pub fn force_release(&mut self) -> GestureVerdict {
        let was_engaged = matches!(self.state, State::Engaged { .. });
        self.state = State::Idle;
        if was_engaged {
            GestureVerdict::HyperReleased
        } else {
            GestureVerdict::Unchanged
        }
    }

    fn on_idle(&mut self, valid: &[GestureTouch], now: Millis) -> GestureVerdict {
        if valid.len() != 1 {
            return GestureVerdict::Unchanged;
        }
        let touch = &valid[0];
        // 리스너 기동 전부터 닿아 있던 접촉은 새 시작이 아니다.
        if touch.stage != GestureTouch::STAGE_MAKE_TOUCH {
            return GestureVerdict::Unchanged;
        }
        if !in_entry_zone(&self.params, touch) {
            return GestureVerdict::Unchanged;
        }
        self.state = State::ZoneContact {
            touch: touch.path_index,
            start_mm: (touch.mm_x, touch.mm_y),
            start_at: now,
        };
        GestureVerdict::ZoneEntered
    }

    fn on_zone_contact(
        &mut self,
        touch_id: i32,
        start_mm: (f64, f64),
        start_at: Millis,
        valid: &[GestureTouch],
        now: Millis,
    ) -> GestureVerdict {
        // 2손가락 취소(§8 수용 기준 4) — 임계 도달 전 두 번째 유효 접촉.
        if valid.len() != 1 {
            self.state = State::Idle;
            return GestureVerdict::Cancelled;
        }
        let Some(touch) = valid.iter().find(|t| t.path_index == touch_id) else {
            // 임계 도달 전 접촉 소멸 — §5 "제스처 미완성".
            self.state = State::Idle;
            return GestureVerdict::Cancelled;
        };
        let moved = inward_distance(&self.params, start_mm, (touch.mm_x, touch.mm_y));
        if moved >= self.params.trigger_mm() {
            // freeze 를 한 프레임에 건너뛴 경우 — 즉시 트리거한다(§3.2.1 순서는
            // "먼저 도달"이지 "별도 프레임 요구"가 아니다).
            self.state = State::Engaged { touch: touch_id };
            return GestureVerdict::HyperEngaged;
        }
        if moved >= self.params.freeze_mm() {
            self.state = State::CursorFrozen {
                touch: touch_id,
                start_mm,
                start_at,
            };
            return GestureVerdict::CursorFrozen;
        }
        if now.saturating_sub(start_at).0 >= self.params.gesture_timeout_ms {
            self.state = State::Idle;
            return GestureVerdict::Cancelled;
        }
        GestureVerdict::Unchanged
    }

    fn on_frozen(
        &mut self,
        touch_id: i32,
        start_mm: (f64, f64),
        start_at: Millis,
        valid: &[GestureTouch],
        now: Millis,
    ) -> GestureVerdict {
        if valid.len() != 1 {
            self.state = State::Idle;
            return GestureVerdict::Cancelled;
        }
        let Some(touch) = valid.iter().find(|t| t.path_index == touch_id) else {
            self.state = State::Idle;
            return GestureVerdict::Cancelled;
        };
        let moved = inward_distance(&self.params, start_mm, (touch.mm_x, touch.mm_y));
        if moved >= self.params.trigger_mm() {
            self.state = State::Engaged { touch: touch_id };
            GestureVerdict::HyperEngaged
        } else if now.saturating_sub(start_at).0 >= self.params.gesture_timeout_ms {
            self.state = State::Idle;
            GestureVerdict::Cancelled
        } else {
            GestureVerdict::Unchanged
        }
    }

    fn on_engaged(&mut self, touch_id: i32, valid: &[GestureTouch]) -> GestureVerdict {
        if valid.iter().any(|t| t.path_index == touch_id) {
            // Engaged 유지 — 두 번째 접촉이 닿아도 유지한다. §3.2 표 Engaged 행의
            // 정책 `(추정)` 두 후보 중 **유지** 채택: 우발적 접촉으로 사용자가 쓰고
            // 있던 hyper 가 끊기는 것보다 덜 놀랍다(§9 #6 근거 기록).
            GestureVerdict::Unchanged
        } else {
            self.state = State::Idle;
            GestureVerdict::HyperReleased
        }
    }
}

/// 진입 영역 판정 — 표면 정규 좌표(0..1) 기준. 상단은 밴드, 코너는 패치.
/// ⚠️ 좌표축 원점 방향은 `MTTouch` 관례가 문서화돼 있지 않다 `(추정)` — 실기기 검증
/// (`manual-verification.md`) 항목으로 남긴다.
fn in_entry_zone(params: &GestureParams, touch: &GestureTouch) -> bool {
    let patch = params.entry_patch_frac;
    match params.area {
        TrackpadArea::Top => touch.y >= 1.0 - patch,
        TrackpadArea::TopLeft => touch.x <= patch && touch.y >= 1.0 - patch,
        TrackpadArea::TopRight => touch.x >= 1.0 - patch && touch.y >= 1.0 - patch,
        TrackpadArea::BottomLeft => touch.x <= patch && touch.y <= patch,
        TrackpadArea::BottomRight => touch.x >= 1.0 - patch && touch.y <= patch,
    }
}

/// "안쪽" 방향 이동 거리(mm) — 허용각 밖의 이동은 0 으로 간주한다.
///
/// 방향 벡터(§3.3 해석): 코너는 표면 중심을 향한 대각선, 상단은 수직 하향.
/// 거리는 시작점 대비 변위의 **안쪽 성분**(코사인 투영)이다 — 허용각 밖이면 0.
fn inward_distance(params: &GestureParams, start_mm: (f64, f64), now_mm: (f64, f64)) -> f64 {
    let (dx, dy) = (now_mm.0 - start_mm.0, now_mm.1 - start_mm.1);
    let (dir_x, dir_y) = inward_direction(params.area);
    let len = (dx * dx + dy * dy).sqrt();
    if len <= f64::EPSILON {
        return 0.0;
    }
    let cos_angle = (dx * dir_x + dy * dir_y) / len;
    if cos_angle < params.direction_tolerance_deg.to_radians().cos() {
        0.0
    } else {
        len * cos_angle
    }
}

/// 진입 영역의 "안쪽" 단위 벡터 — 코너는 표면 중심 대각선, 상단은 수직 하향(§3.3).
/// 좌표 관례는 x: 좌→우, y: 하→상 `(추정)` — 실기기 검증 항목이다.
fn inward_direction(area: TrackpadArea) -> (f64, f64) {
    const S: f64 = std::f64::consts::FRAC_1_SQRT_2;
    match area {
        TrackpadArea::Top => (0.0, -1.0),
        TrackpadArea::TopLeft => (S, -S),
        TrackpadArea::TopRight => (-S, -S),
        TrackpadArea::BottomLeft => (S, S),
        TrackpadArea::BottomRight => (-S, S),
    }
}

#[cfg(test)]
mod tests;