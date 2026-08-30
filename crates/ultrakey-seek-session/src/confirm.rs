//! ⭐ F-01 이 F-04(`docs/spec/seek-click-execution.md`)로 넘기는 계약 —
//! "무엇을 클릭할지"까지만 낸다.
//!
//! ⛔ **F-04(실제 클릭 합성)는 이 크레이트의 범위 밖이다.** 창 포커스·클릭
//! 모드·modifier 조합은 전부 F-04 소관이며, 여기서는 확정된 좌표와 문맥만
//! 넘긴다.

use ultrakey_core::flags::EventFlags;
use ultrakey_seek::{CandidateSource, Rect};

/// ⭐ F-01 이 F-04 에 넘기는 **확정된 클릭 대상**.
///
/// 이것이 두 기능 사이의 유일한 계약이다 — F-01 은 좌표를 정하고, F-04 는
/// 그 좌표에 실제 클릭을 합성한다.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfirmedMatch {
    /// 확정된 후보의 텍스트.
    pub text: String,
    /// **전역 화면 좌표, 포인트, 좌상단 원점** — `TextCandidate::frame`
    /// 그대로다.
    pub frame: Rect,
    /// 이 후보를 만든 소스(OCR/AX).
    pub source: CandidateSource,
    /// 어느 디스플레이에서 나왔는가. AX 매치는 `None`.
    pub display_id: Option<u32>,
    /// 확정 시점의 검색어(로그·진단용).
    pub query: String,
    /// 필터링된 매치 목록 내 0-based 순번.
    pub match_index: usize,
    /// ⭐ **확정 순간의 modifier 스냅샷.**
    ///
    /// F-04 명세(`seek-click-execution.md`) §5 #10 이 명시적으로 요구한다 —
    /// `Change click modes with modifier keys` 가 켜져 있으면 어느 클릭 모드를
    /// 쓸지가 **확정 시점에 눌려 있던 modifier** 로 갈리고, hold 모드의 확정
    /// 시점은 "리매핑 키를 떼는 그 순간" 이다. 그 순간은 F-01 만 알고 F-04 는
    /// 나중에야 호출되므로, **F-01 이 스냅샷을 떠서 함께 넘겨야** 한다. F-04 가
    /// 자기 시점에 다시 조회하면 사용자가 이미 손을 뗀 뒤일 수 있다.
    ///
    /// ⚠️ 리매핑 키 자신은 여기 나타나지 않는다 — F-07 의 이벤트 탭이 그 키를
    /// 이미 소비했기 때문이다(F-04 §5 #10 이 "구조적으로 성립하지 않음" 이라고
    /// 적어 둔 그대로다).
    pub modifiers: EventFlags,
}

impl ConfirmedMatch {
    /// 클릭 지점 — 사각형의 중심.
    ///
    /// ⚠️ 이보다 나은 지점(예: 첫 글자 앞)이 있는지는 F-04 의 판단이다.
    /// 여기서는 명세가 요구하는 "좌표를 넘긴다"만 만족한다.
    #[must_use]
    pub fn click_point(&self) -> (f64, f64) {
        self.frame.center()
    }
}

/// 확정된 대상을 실제로 클릭하는 계층 — `OverlayRenderer`(F-03)와 같은
/// 자리의 이음매다.
///
/// ⛔ **이 크레이트에는 구현이 없다.** F-04 가 `ultrakey-platform` 의
/// `CGEvent` 마우스 합성 위에 구현한다.
pub trait ClickExecutor {
    /// 확정된 대상을 클릭한다.
    ///
    /// # Errors
    /// 클릭 합성이 실패하면 [`ClickError`] 를 반환한다.
    fn execute(&mut self, target: &ConfirmedMatch) -> Result<(), ClickError>;
}

/// F-04 가 아직 없다 — 요청을 기록만 하고 성공을 돌려준다.
///
/// `NullRenderer`(F-03, `ultrakey_overlay::renderer`)와 같은 역할이며, 이
/// 트레이트가 실제로 교체 가능한 이음매임을 코드로 증명한다.
#[derive(Debug, Default)]
pub struct NullClickExecutor {
    /// 마지막으로 요청받은 확정 대상.
    pub last: Option<ConfirmedMatch>,
    /// `execute` 가 불린 총 횟수.
    pub calls: usize,
}

impl ClickExecutor for NullClickExecutor {
    fn execute(&mut self, target: &ConfirmedMatch) -> Result<(), ClickError> {
        self.last = Some(target.clone());
        self.calls += 1;
        Ok(())
    }
}

/// 클릭 합성 실패.
#[derive(Debug, thiserror::Error)]
pub enum ClickError {
    /// 클릭 이벤트를 합성하는 데 실패했다.
    #[error("클릭 합성 실패: {0}")]
    Synthesize(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> ConfirmedMatch {
        ConfirmedMatch {
            text: "Settings".to_string(),
            frame: Rect {
                x: 10.0,
                y: 20.0,
                width: 100.0,
                height: 40.0,
            },
            source: CandidateSource::Ocr,
            display_id: Some(1),
            query: "set".to_string(),
            match_index: 0,
            modifiers: EventFlags::NONE,
        }
    }

    /// 클릭 지점은 사각형의 중심이다.
    #[test]
    fn click_point_is_frame_center() {
        let m = sample();
        assert_eq!(m.click_point(), (60.0, 40.0));
    }

    /// ⭐ `NullClickExecutor` 로 `ConfirmedMatch` → `ClickExecutor` 왕복 —
    /// F-04 이음매가 실제로 교체 가능함을 코드로 증명한다(§3.2 Confirming
    /// 행). 트레이트 객체로도 호출할 수 있어야 한다.
    #[test]
    fn null_click_executor_round_trips_confirmed_match() {
        let mut executor = NullClickExecutor::default();
        let target = sample();

        let mut boxed: Box<dyn ClickExecutor> = Box::new(NullClickExecutor::default());
        assert!(boxed.execute(&target).is_ok());

        assert!(executor.execute(&target).is_ok());
        assert_eq!(executor.calls, 1);
        assert_eq!(executor.last, Some(target.clone()));

        assert!(executor.execute(&target).is_ok());
        assert_eq!(executor.calls, 2);
    }
}
