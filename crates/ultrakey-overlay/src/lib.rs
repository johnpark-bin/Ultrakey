//! `ultrakey-overlay` — F-03 Seek 화면 오버레이 UI의 **순수 로직**.
//!
//! 대응 명세: `docs/spec/seek-overlay-ui.md`.
//!
//! ⭐ 이 크레이트에는 `unsafe` 도 macOS 의존성도 없다. `NSWindow`/`NSScreen`
//! 제어, WKWebView IPC, `CALayer` 그리기 같은 실제 화면 출력은 전부
//! [`renderer::OverlayRenderer`] 구현체(플랫폼 크레이트)가 맡고, 여기는
//! **좌표 변환·클리핑·렌더 모델 산출·세션 상태**만 한다. 그래서 macOS 없이도
//! `cargo test -p ultrakey-overlay` 로 전량 검증된다 — F-02(`ultrakey-seek`)
//! 가 이미 확립한 것과 같은 구조다.
//!
//! ⭐ **명세 §3.3 과의 차이**: 명세는 전 디스플레이를 덮는 단일 창(안 B)을
//! 채택했지만, 이 구현은 **디스플레이별 창**(안 A)을 채택한다. 판단 근거는
//! [`geometry::clip_segment`] 문서에 있다 — 안 A 의 유일한 알려진 결함(연결선이
//! 디스플레이 경계를 넘을 때 어느 캔버스에도 중간 구간이 없음, v1.55 회귀
//! 이력)을 "전역 좌표에서 선분을 한 번 정의하고 창마다 자기 몫만 잘라
//! 그린다"는 구조로 없앤다. ⭐ **명세 §3.3 에 "재결정" 절로 반영돼 있다**
//! (이슈 #34) — 거기에 안 B 가 §8 의 배율 수용 기준을 구조적으로 만족할 수
//! 없다는 근거가 함께 적혀 있다.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod geometry;
pub mod model;
pub mod palette;
pub mod renderer;
pub mod session;

pub use geometry::{clip_segment, display_for_point, to_local, union_frame, OverlayDisplay};
pub use model::{Highlight, MatchRow, OverlayFrame, SearchBarFrame, Segment};
pub use palette::{Appearance, Palette};
pub use renderer::{NullRenderer, OverlayRenderer, RenderError};
pub use session::{
    OverlaySession, HIGHLIGHT_CAP_PER_DISPLAY, MATCH_LIST_MAX, SEARCH_BAR_HEIGHT_PT,
    SEARCH_BAR_WIDTH_PT,
};

// F-02 계약 타입을 그대로 재수출한다 — 이 크레이트의 공개 API 를 쓰는 쪽이
// `ultrakey_seek` 를 별도로 의존성에 추가하지 않아도 되게 한다.
pub use ultrakey_seek::{CandidateSource, Rect, TextCandidate};
