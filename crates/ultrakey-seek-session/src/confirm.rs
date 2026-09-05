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
    /// 창 전면화 확정에 쓰는 창 식별자(`kCGWindowNumber`) — OCR/AX 는 `None`,
    /// 창 제목(WindowTitle) 후보는 `Some(kCGWindowNumber)`.
    pub window_id: Option<u32>,
    /// 창 전면화 확정에 쓰는 소유 프로세스 식별자(`kCGWindowOwnerPID`) —
    /// OCR/AX 는 `None`, 창 제목 후보는 `Some(kCGWindowOwnerPID)`.
    pub pid: Option<i32>,
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

    /// 이 확정을 **어떻게 실행할지**를 정한다 — 소스 하나로 분기한다(이슈 #133):
    /// 창 제목(WindowTitle) 후보는 **클릭이 아니라 창 전면화**가 확정 동작이고,
    /// 나머지(OCR·AX)는 기존 클릭 합성이 확정 동작이다.
    ///
    /// ⚠️ WindowTitle 인데 `window_id`·`pid` 중 하나라도 `None` 이면(구조상
    /// 불가 — 생성자가 두 값을 항상 채운다) 그래도 `FrontWindow` 를 반환하고,
    /// 실행기가 None 을 조용한 no-op(로그)으로 받는다(방어).
    #[must_use]
    pub fn action(&self) -> ConfirmAction {
        if self.source == CandidateSource::WindowTitle {
            ConfirmAction::FrontWindow
        } else {
            ConfirmAction::Click
        }
    }
}

/// 확정된 대상의 **실행 종류**. F-04 실행기는 이 값으로 먼저 분기한다(이슈 #133
/// 판정 조건 4 — "제목→창 전면화 / 텍스트→클릭").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    /// 기존 클릭 합성 — OCR·AX 후보의 확정 동작(현행 유지).
    Click,
    /// 창 전면화(제목 검색 후보의 확정 동작) — 좌표 클릭을 합성하지 않는다.
    FrontWindow,
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

    /// 클릭 설정(`Focus window before clicking`·`Change click modes with
    /// modifier keys`) 변경 통지 — F-04 가 `SeekSignal::ConfigChanged` 를 받을
    /// 때 함께 호출된다(A6, 결정 이슈 #44).
    ///
    /// 기본 구현은 no-op 이다 — `NullClickExecutor` 와 기존 테스트를 무변경으로
    /// 두기 위함이며, 즉 이 메서드가 생겨도 F-04 설치 전의 이음매는 그대로
    /// 컴파일·동작한다.
    fn configure(&mut self, _settings: ClickSettings) {}
}

/// F-04 의 클릭 실행 설정 — `Seek` 탭의 두 체크박스(명세 §4).
///
/// ⭐ **출고 기본값 실측**(명세 §4): `focus_window_before_clicking = false`(☐),
/// `change_click_modes_with_modifiers = true`(☑). 후자는 **Seek 탭에서 유일하게
/// 출고 기본값이 켜진 항목**이다 — 그래서 `Default` 는 파생이 아니라 수동
/// 구현이다(파생 `Default` 는 전부 `false` 를 만들기 때문).
///
/// ⛔ 저장 표현이 아니다 — 실제 영속화는 `SeekSettings`(+ `keys.rs`)가 맡고,
/// 이 타입은 저장소에서 조립된 값이 워커 → executor 로 전달되는 **런타임
/// 경계 타입**이다(A6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClickSettings {
    /// `Focus window before clicking` — 기본 ☐.
    pub focus_window_before_clicking: bool,
    /// `Change click modes with modifier keys` — 기본 ☑.
    pub change_click_modes_with_modifiers: bool,
}

impl Default for ClickSettings {
    fn default() -> Self {
        Self {
            focus_window_before_clicking: false,
            change_click_modes_with_modifiers: true,
        }
    }
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
            window_id: None,
            pid: None,
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

    /// ⭐ `ClickSettings::default()` — 출고 기본값 실측(명세 §4) 그대로:
    /// F-04 의 두 체크박스는 ☐ / ☑ 이다. 파생 `Default` 였다면 전부 `false` 가
    /// 되므로, 이 테스트가 수동 `Default` 구현을 고정한다(A4).
    #[test]
    fn click_settings_default_matches_measured_shipping_defaults() {
        let s = ClickSettings::default();
        assert!(!s.focus_window_before_clicking);
        assert!(
            s.change_click_modes_with_modifiers,
            "Change click modes with modifier keys 는 Seek 탭에서 유일하게 출고 기본값이 켜진 항목이다(§4)"
        );
    }

    /// `configure` 의 기본 구현은 no-op — `NullClickExecutor` 는 설정을 받아도
    /// 아무 일도 하지 않고 `execute` 왕복이 그대로 성립해야 한다(F-04 설치 전
    /// 이음매 호환성, A6).
    #[test]
    fn null_click_executor_configure_is_a_noop_and_keeps_round_trip_working() {
        let mut boxed: Box<dyn ClickExecutor> = Box::new(NullClickExecutor::default());
        boxed.configure(ClickSettings::default());
        boxed.configure(ClickSettings {
            focus_window_before_clicking: true,
            change_click_modes_with_modifiers: false,
        });

        let target = sample();
        assert!(boxed.execute(&target).is_ok());
    }

    /// ⭐ 이슈 #133 판정 조건 4 — `action()` 은 소스 하나로 분기한다:
    /// 창 제목(WindowTitle) → `FrontWindow`(창 전면화), 그 외(OCR·AX) →
    /// `Click`(기존 클릭 합성).
    #[test]
    fn action_branches_on_source_window_title_vs_rest() {
        // WindowTitle — 전면화 확정. window_id/pid 가 채워져 있다(생성자 보장).
        let title = ConfirmedMatch {
            source: CandidateSource::WindowTitle,
            window_id: Some(42),
            pid: Some(1337),
            ..sample()
        };
        assert_eq!(title.action(), ConfirmAction::FrontWindow);

        // OCR → 클릭.
        let ocr = ConfirmedMatch {
            source: CandidateSource::Ocr,
            window_id: None,
            pid: None,
            ..sample()
        };
        assert_eq!(ocr.action(), ConfirmAction::Click);

        // AX → 클릭.
        let ax = ConfirmedMatch {
            source: CandidateSource::Accessibility,
            window_id: None,
            pid: None,
            ..sample()
        };
        assert_eq!(ax.action(), ConfirmAction::Click);
    }

    /// ⚠️ 방어 — WindowTitle 인데 window_id/pid 가 None 이어도(구조상 불가 —
    /// 생성자가 보장) `FrontWindow` 를 반환하고, None 은 실행기가 조용한
    /// no-op 으로 받는다. 판정은 소스만 본다.
    #[test]
    fn action_still_returns_front_window_for_window_title_with_missing_ids() {
        let broken = ConfirmedMatch {
            source: CandidateSource::WindowTitle,
            window_id: None,
            pid: None,
            ..sample()
        };
        assert_eq!(broken.action(), ConfirmAction::FrontWindow);
    }
}
