//! 오버레이를 실제로 그리는 계층의 트레이트 — §7 이 요구한 "교체 가능한
//! 렌더링 계층"의 이음매.
//!
//! ⭐ 1차 구현은 WKWebView(Tauri 웹뷰)로 [`crate::model::OverlayFrame`]/
//! [`crate::model::SearchBarFrame`] 을 JS 로 emit 해 그리는 방식이 될 것으로
//! 예상된다(§7 판정). 하지만 §3.6 의 지연 예산을 WKWebView 가 만족하는지는
//! 명세 §9 미해결 질문 2번으로 남아 있다 — 실측 결과 예산을 넘으면, 이
//! 트레이트를 구현하는 두 번째 타입(`objc2-quartz-core` 의 `CALayer` 직접
//! 그리기)으로 교체하면 된다. 그 교체가 [`crate::session::OverlaySession`]
//! 이나 이 크레이트의 다른 어떤 것도 건드리지 않고 가능하다는 것이 이
//! 트레이트가 존재하는 이유다.

use crate::geometry::OverlayDisplay;
use crate::model::{OverlayFrame, SearchBarFrame};

/// 오버레이 렌더링 실패.
#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    /// 서피스(창/레이어) 생성·동기화에 실패했다.
    #[error("오버레이 서피스 동기화 실패: {0}")]
    SurfaceSync(String),
    /// 프레임 그리기에 실패했다.
    #[error("프레임 렌더링 실패: {0}")]
    Present(String),
    /// 검색 바 위치 갱신에 실패했다.
    #[error("검색 바 위치 갱신 실패: {0}")]
    SearchBarOrigin(String),
    /// 표시/숨김 전환에 실패했다.
    #[error("표시/숨김 전환 실패: {0}")]
    Visibility(String),
}

/// 오버레이를 실제로 그리는 계층.
///
/// `OverlaySession` 이 산출한 렌더 모델([`OverlayFrame`]/[`SearchBarFrame`])을
/// 실제 화면 출력으로 옮기는 경계다 — 이 트레이트 아래로는 플랫폼 의존
/// 코드(WKWebView IPC, `CALayer`, `NSWindow`)가 있을 수 있지만, 이 크레이트는
/// 그 구현을 갖지 않는다(모듈 상단 문서 참조).
pub trait OverlayRenderer {
    /// 디스플레이 목록에 맞춰 렌더링 서피스(창/레이어)를 만들거나 갱신한다.
    /// 핫플러그(명세 §5 #1)마다 다시 호출된다.
    fn sync_surfaces(&mut self, displays: &[OverlayDisplay]) -> Result<(), RenderError>;

    /// 프레임 목록(디스플레이별)과 검색 바 프레임을 실제 화면에 그린다.
    fn present(
        &mut self,
        frames: &[OverlayFrame],
        bar: &SearchBarFrame,
    ) -> Result<(), RenderError>;

    /// 검색 바 위치를 옮긴다(사용자 드래그, §3.4 위치 저장).
    fn set_search_bar_origin(&mut self, x: f64, y: f64) -> Result<(), RenderError>;

    /// 오버레이를 표시한다(§3.1 "표시" 상태 진입).
    fn show(&mut self) -> Result<(), RenderError>;

    /// 오버레이를 숨긴다(§3.1 "숨김" 상태 진입).
    fn hide(&mut self) -> Result<(), RenderError>;
}

/// 아무것도 그리지 않는 렌더러 — 마지막으로 받은 값만 기록한다.
///
/// `OverlaySession` → `OverlayRenderer` 배선을 테스트하는 데 쓰고, 이
/// 트레이트가 실제로 교체 가능한 이음매임을 코드로 증명한다(§7). 실제 UI
/// 프로세스에서는 쓰이지 않는다 — 테스트·프로토타입 전용이다.
#[derive(Debug, Default)]
pub struct NullRenderer {
    /// 마지막으로 `sync_surfaces` 에 전달된 디스플레이 수.
    pub last_synced_display_count: usize,
    /// 마지막으로 `present` 에 전달된 프레임(디스플레이별).
    pub last_frames: Vec<OverlayFrame>,
    /// 마지막으로 `present` 에 전달된 검색 바 프레임.
    pub last_bar: Option<SearchBarFrame>,
    /// 마지막으로 설정된 검색 바 원점.
    pub last_bar_origin: Option<(f64, f64)>,
    /// 현재 표시 중인가.
    pub visible: bool,
    /// ⭐(이슈 #132) 렌더러 수명주기 이벤트 기록 — `present`/`show`/`hide` 가
    /// 호출된 **순서**를 남긴다. "표시 전에 빈 프레임 present 가 선행한다"는
    /// 열림 계약이 이 로그로 회귀 테스트된다(아래 `tests` — 현행
    /// `session_to_renderer_round_trip` 이 기록값을 assert 하지 않으므로
    /// 필드 추가만으로는 기존 테스트를 깨지 않는다).
    pub lifecycle: Vec<LifecycleEvent>,
}

/// ⭐(이슈 #132) [`NullRenderer`] 가 남기는 수명주기 이벤트.
///
/// 발화 순서(`present` → `show` → … → `hide`)가 그대로 로그가 된다. 테스트는
/// `Present` 의 프레임이 전부 빈 프레임인지(`highlights` 0 · `line` None)와
/// 어느 이벤트가 어느 이벤트보다 앞섰는지를 이 로그로 검사한다.
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleEvent {
    /// `present` 에 전달된 프레임 목록.
    Present {
        /// 프레임 목록 — 테스트가 빈 프레임 여부(`highlights` 0 · `line` None)를
        /// 검사한다.
        frames: Vec<OverlayFrame>,
    },
    /// `show`.
    Show,
    /// `hide`.
    Hide,
}

impl OverlayRenderer for NullRenderer {
    fn sync_surfaces(&mut self, displays: &[OverlayDisplay]) -> Result<(), RenderError> {
        self.last_synced_display_count = displays.len();
        Ok(())
    }

    fn present(
        &mut self,
        frames: &[OverlayFrame],
        bar: &SearchBarFrame,
    ) -> Result<(), RenderError> {
        self.last_frames = frames.to_vec();
        self.last_bar = Some(bar.clone());
        self.lifecycle
            .push(LifecycleEvent::Present { frames: frames.to_vec() });
        Ok(())
    }

    fn set_search_bar_origin(&mut self, x: f64, y: f64) -> Result<(), RenderError> {
        self.last_bar_origin = Some((x, y));
        Ok(())
    }

    fn show(&mut self) -> Result<(), RenderError> {
        self.visible = true;
        self.lifecycle.push(LifecycleEvent::Show);
        Ok(())
    }

    fn hide(&mut self) -> Result<(), RenderError> {
        self.visible = false;
        self.lifecycle.push(LifecycleEvent::Hide);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Appearance;
    use crate::session::OverlaySession;
    use ultrakey_seek::{Rect, TextCandidate};

    fn display(id: u32, x: f64, y: f64, w: f64, h: f64) -> OverlayDisplay {
        OverlayDisplay {
            display_id: id,
            frame: Rect { x, y, width: w, height: h },
            backing_scale: 2.0,
        }
    }

    /// `NullRenderer` 로 세션 → 렌더러 왕복 — `sync_surfaces`·`present`·
    /// `show`/`hide`·`set_search_bar_origin` 이 실제로 값을 전달하는가.
    /// 이 테스트 자체가 `OverlayRenderer` 가 교체 가능한 이음매임을
    /// 코드로 증명한다 — `OverlaySession` 은 `NullRenderer` 가 무엇인지
    /// 전혀 모른 채로 렌더 모델만 내놓는다.
    #[test]
    fn session_to_renderer_round_trip() {
        let displays = vec![
            display(1, 0.0, 0.0, 1000.0, 1000.0),
            display(2, 1000.0, 0.0, 1000.0, 1000.0),
        ];
        let session = OverlaySession::open(displays.clone(), Appearance::Light, false);

        let mut renderer = NullRenderer::default();
        renderer.sync_surfaces(&displays).unwrap();
        assert_eq!(renderer.last_synced_display_count, 2);

        renderer.show().unwrap();
        assert!(renderer.visible);

        let frames = session.frames();
        let bar = session.search_bar_frame();
        renderer.present(&frames, &bar).unwrap();
        assert_eq!(renderer.last_frames.len(), 2);
        assert_eq!(renderer.last_bar.as_ref().unwrap().query, "");

        renderer.set_search_bar_origin(123.0, 45.0).unwrap();
        assert_eq!(renderer.last_bar_origin, Some((123.0, 45.0)));

        renderer.hide().unwrap();
        assert!(!renderer.visible);
    }

    // ⭐(이슈 #132) — 렌더러 수명주기 계약 회귀 테스트.
    //
    // 현행 `session_to_renderer_round_trip` 은 메서드를 "올바른 순서로" 직접
    // 호출하므로, 호출 **순서**가 틀어져도 잡지 못한다. 아래 두 테스트는
    // `NullRenderer::lifecycle` 기록을 assert 해 그 빈칸을 메운다.
    //
    // ⚠️ 이 테스트는 "호출자가 어떤 순서로 렌더러를 드라이브해야 하는가"를
    // **실행 가능한 계약으로 고정**한다 — 시퀀스 자체는 이 모듈이 소유하지
    // 않으므로(실제 호출자는 `apps/ultrakey-app/src/seek.rs` 의 Opened
    // 브랜치), 호출자 쪽 회귀는 이 테스트로는 못 잡는다(그쪽은 실기기 +
    // `ULTRAKEY_SEEK_TRACE` 로그가 판정한다). 이 테스트가 고정하는 것은
    // 렌더러 계층의 계약이다: "신규 세션(빈 프레임) present 가 show 보다
    // 앞서야 하고, hide 다음 show 사이에는 빈 present 가 있어야 한다."

    /// 신규 세션 열림 — `OverlaySession::open` 의 프레임은 전부 빈 프레임이고,
    /// 그 present 가 `show` 보다 **앞서** 온다(§3.1 표시 행, 이슈 #132).
    #[test]
    fn session_open_presents_empty_frames_before_show() {
        let displays = vec![
            display(1, 0.0, 0.0, 1000.0, 1000.0),
            display(2, 1000.0, 0.0, 1000.0, 1000.0),
        ];
        let session = OverlaySession::open(displays.clone(), Appearance::Light, false);
        assert!(session.frames().iter().all(|f| f.highlights.is_empty()));

        let mut renderer = NullRenderer::default();
        renderer.sync_surfaces(&displays).unwrap();
        // ⭐ 계약 — 표시 **전에** 빈 세션을 present 한다(이슈 #132 설계 ②).
        let frames = session.frames();
        let bar = session.search_bar_frame();
        renderer.present(&frames, &bar).unwrap();
        renderer.show().unwrap();

        assert_eq!(
            renderer.lifecycle,
            vec![
                LifecycleEvent::Present { frames },
                LifecycleEvent::Show,
            ],
            "신규 세션의 빈 프레임 present 가 show 보다 앞서야 한다"
        );
    }

    /// 이전 세션(그려진 프레임) → hide → 신규 세션 — 그 사이 **빈** present 가
    /// 선행해야 한다(§3.1 숨김+표시 행, 이슈 #132). 이 테스트는 "숨김 후
    /// 다시 표시되기 직전 프레임은 반드시 빈 프레임"이라는 계약을 고정한다 —
    /// stale 그림을 그대로 올리는 `show` 는 이 assert 를 통과할 수 없다.
    #[test]
    fn reopen_after_hide_requires_empty_present_before_show() {
        let displays = vec![display(1, 0.0, 0.0, 1000.0, 1000.0)];

        // 이전 세션 — 후보 1개를 넣어 **그려진** 프레임을 만든다.
        let mut previous = OverlaySession::open(displays.clone(), Appearance::Light, false);
        previous.set_query("Save");
        previous.ingest_display(
            1,
            vec![TextCandidate::ocr(
                "Save".into(),
                Rect { x: 10.0, y: 10.0, width: 40.0, height: 20.0 },
                0.9,
                1,
            )],
        );
        let drawn = previous.frames();
        assert!(drawn.iter().any(|f| !f.highlights.is_empty()));

        let mut renderer = NullRenderer::default();
        renderer.sync_surfaces(&displays).unwrap();
        let bar = previous.search_bar_frame();
        renderer.present(&drawn, &bar).unwrap();
        renderer.hide().unwrap();

        // 신규 세션 — 후보 0(빈 프레임)으로 연다(S-6).
        let fresh = OverlaySession::open(displays.clone(), Appearance::Light, false);
        let empty = fresh.frames();
        assert!(empty.iter().all(|f| f.highlights.is_empty() && f.line.is_none()));
        renderer.present(&empty, &bar).unwrap();
        renderer.show().unwrap();

        // ⭐ hide 와 다음 show 사이의 present 는 반드시 빈 프레임이다.
        let hide_idx = renderer.lifecycle.iter().position(|e| *e == LifecycleEvent::Hide).unwrap();
        let mut after_hide = renderer.lifecycle[hide_idx + 1..].iter();
        let Some(LifecycleEvent::Present { frames }) = after_hide.next() else {
            panic!("hide 직후에는 빈 프레임 present 가 와야 한다 — stale 그림을 그대로 show 하면 재진입 플래시가 난다(이슈 #132)");
        };
        assert!(frames.iter().all(|f| f.highlights.is_empty() && f.line.is_none()));
        assert_eq!(after_hide.next(), Some(&LifecycleEvent::Show));
    }
}
