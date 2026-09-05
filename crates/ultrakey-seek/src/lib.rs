//! `ultrakey-seek` — F-02 Seek 텍스트 후보 검출의 **순수 로직**.
//!
//! 대응 명세: `docs/spec/seek-text-detection.md`.
//!
//! ⭐ 이 크레이트에는 `unsafe` 도 macOS 의존성도 없다. 화면 캡처·CoreImage·
//! Vision·Accessibility 는 전부 `ultrakey-platform` 이 맡고, 여기는 그 산출물을
//! 받아 **좌표 변환(§3.2.3) · 후보 정규화 · 병합(§3.4) · 질의 매칭(§3.6)** 만
//! 한다. 그래서 macOS 없이도 `cargo test -p ultrakey-seek` 로 전량 검증된다 —
//! 명세 §5 #3(v1.55 다중 디스플레이 회귀)이 요구하는 회귀 테스트가 CI 에서
//! 돌 수 있는 유일한 구조다(`docs/dev/architecture.md` §1).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod candidate;
#[cfg(target_os = "macos")]
pub mod detect;
pub mod merge;
pub mod query;
pub mod transform;

pub use candidate::{CandidateSource, TextCandidate};
#[cfg(target_os = "macos")]
pub use detect::{detect_candidates, DetectionOutcome, DetectionParams, DisplayResult};
pub use merge::{merge_candidates, merge_window_titles, MergeParams};
pub use query::{filter_by_query, normalize_for_match, QueryParams};
pub use transform::{normalized_bbox_to_global, DisplayFrame, Rect};
