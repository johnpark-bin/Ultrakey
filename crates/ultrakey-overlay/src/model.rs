//! F-02 후보를 받아 렌더러(웹뷰/네이티브)에 넘기는 **렌더 모델**.
//!
//! 여기 정의된 타입은 전부 `serde::Serialize` 다 — §7 판정대로 1차 렌더러가
//! WKWebView 라면 이 구조체들이 그대로 JS 로 emit 된다. 좌표는
//! `OverlaySession::frames`/`search_bar_frame` 단계에서 이미 창-로컬 변환·
//! 상한 적용까지 끝나 있다 — 렌더러는 좌표 변환이나 후보 필터링을 다시
//! 할 필요가 없다.

use serde::Serialize;

use ultrakey_seek::{CandidateSource, Rect};

use crate::palette::Palette;

/// 하이라이트 하나 — 좌표는 이미 창-로컬로 변환돼 있다.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Highlight {
    /// 창-로컬 좌표(pt).
    pub rect: Rect,
    /// 표시할 짧은 라벨(매치 순번 등, 1-based).
    pub label: String,
    /// 현재 선택된 매치인가.
    pub selected: bool,
    /// 어느 소스(OCR/AX)에서 나왔는가 — §3.5 하이라이트 스타일 분기용.
    pub source: CandidateSource,
}

/// 전역(또는 창-로컬) 좌표 선분.
///
/// [`crate::geometry::clip_segment`] 의 입력은 **전역** 좌표, 출력은 **창-로컬**
/// 좌표다 — 같은 타입이 양쪽 다 담을 수 있게 좌표계를 필드에 강제하지
/// 않는다. 어느 좌표계인지는 호출 지점의 문맥으로 정해진다.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Segment {
    /// 시작점 x.
    pub x1: f64,
    /// 시작점 y.
    pub y1: f64,
    /// 끝점 x.
    pub x2: f64,
    /// 끝점 y.
    pub y2: f64,
}

/// 창 하나(디스플레이 하나)에 보내는 한 프레임.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct OverlayFrame {
    /// 이 프레임이 그려질 디스플레이.
    pub display_id: u32,
    /// 이 디스플레이에 그릴 하이라이트 목록(§4.2 상한 적용됨).
    pub highlights: Vec<Highlight>,
    /// 검색 바 → 선택 매치 연결선의 **이 창 몫**(창-로컬 좌표). 없으면 `None`.
    pub line: Option<Segment>,
    /// 색상·두께 팔레트.
    pub palette: Palette,
    /// `accessibilityDisplayShouldReduceMotion` 이 참이면 `false`(§3.5).
    pub animate: bool,
    /// 상한(§4.2)에 걸려 생략된 비선택 후보 수. 0 이면 전부 그렸다.
    pub omitted: usize,
}

/// 검색 바 창에 보내는 프레임.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SearchBarFrame {
    /// 현재 검색어.
    pub query: String,
    /// 검색 바 안 매치 목록(§1.1 `EntryOutlineView`). 상한
    /// [`crate::session::MATCH_LIST_MAX`].
    pub matches: Vec<MatchRow>,
    /// 현재 선택된 매치의 전체 매치 목록 내 인덱스(표시 상한과 무관하게,
    /// 필터링된 전체 목록 기준).
    pub selected_index: Option<usize>,
    /// 필터링 후 전체 매치 수(목록 상한과 무관).
    pub total_matches: usize,
    /// 색상 팔레트.
    pub palette: Palette,
    /// 검출이 아직 진행 중인가 (S-6 — 후보 0개로 먼저 뜬 상태를 UI 가 알아야 한다).
    pub detecting: bool,
}

/// 검색 바 매치 목록의 행 하나.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct MatchRow {
    /// 목록 내 순번(0-based, 필터링된 전체 목록 기준).
    pub index: usize,
    /// 매치 텍스트.
    pub text: String,
    /// 어느 디스플레이인가. AX 매치는 `None`.
    pub display_id: Option<u32>,
    /// 어느 소스인가.
    pub source: CandidateSource,
}
