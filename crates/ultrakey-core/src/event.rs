//! 물리/합성 이벤트의 종류와, 판정 엔진에 들어오는 입력 이벤트 표현.

use crate::flags::EventFlags;
use crate::keycode::KeyCode;

/// `CGEventType` 에 대응하는 이벤트 종류. 탭 비활성화 통지 두 종도 포함한다
/// (`key-remapping-engine.md` §3-a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventKind {
    KeyDown,
    KeyUp,
    FlagsChanged,
    LeftMouseDown,
    LeftMouseUp,
    RightMouseDown,
    RightMouseUp,
    OtherMouseDown,
    OtherMouseUp,
    LeftMouseDragged,
    RightMouseDragged,
    OtherMouseDragged,
    MouseMoved,
    ScrollWheel,
    TapDisabledByTimeout,
    TapDisabledByUserInput,
}

impl EventKind {
    /// `hyperkey.md` §3.3 `Click` 체크박스에 대응(추정 — 버튼별 세분화 여부는 원문 미명시, §9).
    pub fn is_click(self) -> bool {
        matches!(
            self,
            EventKind::LeftMouseDown
                | EventKind::LeftMouseUp
                | EventKind::RightMouseDown
                | EventKind::RightMouseUp
                | EventKind::OtherMouseDown
                | EventKind::OtherMouseUp
        )
    }

    /// `hyperkey.md` §3.3 `Drag` 체크박스에 대응(추정).
    pub fn is_drag(self) -> bool {
        matches!(
            self,
            EventKind::LeftMouseDragged | EventKind::RightMouseDragged | EventKind::OtherMouseDragged
        )
    }

    /// `hyperkey.md` §3.3 `Move` 체크박스에 대응(추정).
    pub fn is_move(self) -> bool {
        matches!(self, EventKind::MouseMoved)
    }

    /// `hyperkey.md` §3.3 `Scroll` 체크박스에 대응(추정).
    pub fn is_scroll(self) -> bool {
        matches!(self, EventKind::ScrollWheel)
    }

    /// 키보드 이벤트(마우스·탭 상태 통지 제외).
    pub fn is_key(self) -> bool {
        matches!(self, EventKind::KeyDown | EventKind::KeyUp | EventKind::FlagsChanged)
    }
}

/// 판정 엔진에 들어오는 입력 이벤트 하나. 물리 keycode 기준으로만 판정한다
/// (`key-remapping-engine.md` §8 — 문자 기반 판정 경로 금지).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InputEvent {
    pub kind: EventKind,
    pub keycode: KeyCode,
    pub flags: EventFlags,
    /// `kCGKeyboardEventAutorepeat` — OS 가 보내는 키 반복 여부(§5 엣지 케이스 13).
    pub autorepeat: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn click_kinds() {
        assert!(EventKind::LeftMouseDown.is_click());
        assert!(EventKind::RightMouseUp.is_click());
        assert!(EventKind::OtherMouseDown.is_click());
        assert!(!EventKind::LeftMouseDragged.is_click());
    }

    #[test]
    fn drag_kinds() {
        assert!(EventKind::LeftMouseDragged.is_drag());
        assert!(!EventKind::LeftMouseDown.is_drag());
    }

    #[test]
    fn move_and_scroll_kinds() {
        assert!(EventKind::MouseMoved.is_move());
        assert!(EventKind::ScrollWheel.is_scroll());
        assert!(!EventKind::MouseMoved.is_scroll());
    }

    #[test]
    fn key_kinds() {
        assert!(EventKind::KeyDown.is_key());
        assert!(EventKind::KeyUp.is_key());
        assert!(EventKind::FlagsChanged.is_key());
        assert!(!EventKind::LeftMouseDown.is_key());
        assert!(!EventKind::TapDisabledByTimeout.is_key());
    }
}
