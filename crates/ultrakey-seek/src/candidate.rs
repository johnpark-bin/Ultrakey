//! 세 소스가 공유하는 후보 자료형 (§1, §3.2 A4, §3.3 B4, 이슈 #133 소스 C).

use crate::transform::Rect;

/// 후보를 만든 소스.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum CandidateSource {
    /// 소스 A — Vision OCR. 항상 켜지는 기본 소스(§1).
    Ocr,
    /// 소스 B — Accessibility 트리. 최전면 창 한정 보강 소스(§3.3).
    Accessibility,
    /// 소스 C — CGWindowList 창 제목. 화면에 안 보이는(가려진) 창의 제목을
    /// 검색 대상에 넣는 보강 소스(이슈 #133).
    WindowTitle,
}

/// 클릭 가능한 텍스트 후보 하나.
///
/// `frame` 은 **전역 화면 좌표, 포인트, 좌상단 원점** 이다 — 세 소스가 이
/// 좌표계로 정규화된 뒤에야 병합된다(§3.4).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TextCandidate {
    /// 후보 텍스트.
    pub text: String,
    /// 전역 화면 좌표 사각형. AX 매치는 **요소 전체**(§3.3), 창 제목 매치는
    /// **창 bounds 전체**(이슈 #133)다.
    pub frame: Rect,
    /// 어느 소스가 만들었는가.
    pub source: CandidateSource,
    /// OCR 신뢰도(0~1). AX·창 제목 매치는 `None` — 둘 다 신뢰도 개념이 없다.
    pub confidence: Option<f32>,
    /// 어느 디스플레이에서 나왔는가. AX·창 제목 매치는 디스플레이를 특정하지
    /// 않으므로 `None` 이다(§3.5 의 화면별 보관 구조와 정합).
    pub display_id: Option<u32>,
    /// 창 전면화 확정에 쓰는 창 식별자(`kCGWindowNumber`). OCR/AX 는 `None`.
    pub window_id: Option<u32>,
    /// 창 전면화 확정에 쓰는 소유 프로세스 식별자(`kCGWindowOwnerPID`). OCR/AX 는 `None`.
    pub pid: Option<i32>,
}

impl TextCandidate {
    /// OCR 후보를 만든다 (§3.2 A4).
    #[must_use]
    pub fn ocr(text: String, frame: Rect, confidence: f32, display_id: u32) -> Self {
        Self {
            text,
            frame,
            source: CandidateSource::Ocr,
            confidence: Some(confidence),
            display_id: Some(display_id),
            window_id: None,
            pid: None,
        }
    }

    /// AX 후보를 만든다 (§3.3 B4).
    #[must_use]
    pub fn accessibility(text: String, frame: Rect) -> Self {
        Self {
            text,
            frame,
            source: CandidateSource::Accessibility,
            confidence: None,
            display_id: None,
            window_id: None,
            pid: None,
        }
    }

    /// 창 제목 후보를 만든다(소스 C, 이슈 #133). `frame` 은 창 bounds 전체다.
    #[must_use]
    pub fn window_title(text: String, frame: Rect, window_id: u32, pid: i32) -> Self {
        Self {
            text,
            frame,
            source: CandidateSource::WindowTitle,
            confidence: None,
            display_id: None,
            window_id: Some(window_id),
            pid: Some(pid),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// §3.2 A4 — OCR 후보 생성자가 source/confidence/display_id 를 명세대로 채우는가.
    #[test]
    fn ocr_constructor_fills_source_confidence_display_id() {
        let rect = Rect { x: 1.0, y: 2.0, width: 3.0, height: 4.0 };
        let c = TextCandidate::ocr("Hello".to_string(), rect, 0.87, 5);
        assert_eq!(c.text, "Hello");
        assert_eq!(c.frame, rect);
        assert_eq!(c.source, CandidateSource::Ocr);
        assert_eq!(c.confidence, Some(0.87));
        assert_eq!(c.display_id, Some(5));
    }

    /// §3.3 B4 — AX 후보 생성자는 confidence: None, display_id: None 이어야 한다
    /// (AX 에는 신뢰도 개념이 없고, 디스플레이를 특정하지 않는다).
    #[test]
    fn accessibility_constructor_has_no_confidence_or_display_id() {
        let rect = Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 };
        let c = TextCandidate::accessibility("World".to_string(), rect);
        assert_eq!(c.text, "World");
        assert_eq!(c.frame, rect);
        assert_eq!(c.source, CandidateSource::Accessibility);
        assert_eq!(c.confidence, None);
        assert_eq!(c.display_id, None);
        assert_eq!(c.window_id, None);
        assert_eq!(c.pid, None);
    }

    /// 이슈 #133 소스 C — 창 제목 생성자는 window_id/pid 를 채우고 OCR/AX
    /// 전용 필드(confidence/display_id)는 None 으로 둬야 한다.
    #[test]
    fn window_title_constructor_fills_window_id_and_pid() {
        let rect = Rect { x: 0.0, y: 0.0, width: 800.0, height: 600.0 };
        let c = TextCandidate::window_title("Settings".to_string(), rect, 42, 1337);
        assert_eq!(c.text, "Settings");
        assert_eq!(c.frame, rect);
        assert_eq!(c.source, CandidateSource::WindowTitle);
        assert_eq!(c.confidence, None);
        assert_eq!(c.display_id, None);
        assert_eq!(c.window_id, Some(42));
        assert_eq!(c.pid, Some(1337));
    }
}
