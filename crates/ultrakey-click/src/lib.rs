//! `ultrakey-click` — F-04 Seek 클릭 실행의 **순수 로직**.
//!
//! 대응 명세: `docs/spec/seek-click-execution.md`.
//!
//! ⭐ 이 크레이트에는 `unsafe` 도 macOS 의존성도 없다. 클릭 모드 해석 · 클릭
//! 지점 계산 · 프리미티브 시퀀스 계획 · AX 경로 판정 · 화면 밖 판정이 전부
//! 여기 있고, macOS 가 아닌 곳에서도 `cargo test -p ultrakey-click` 로 전량
//! 검증된다(`docs/dev/architecture.md` §1 의 "unsafe 와 순수 판정 로직의
//! 물리적 분리" 기준 그대로 — F-04 는 ① 이 크레이트 + ② `ultrakey-platform::
//! click_synthesis`(unsafe FFI) + ③ `apps/ultrakey-app::click_executor`
//! (오케스트레이션) 셋에 걸쳐 정착한다).
//!
//! ⭐ **결정 기록**: 이 크레이트가 확정한 값(모드 매핑표·지점·실패 정책 등)은
//! 전부 **클론의 설계 결정(미검증)** 이다(이슈 #44). 원본 SuperKey 의 대응
//! 동작은 실측되지 않았으며, 원본 실기 대조로 교정이 필요하다 — 각 상수 위의
//! 주석이 그 근거를 남긴다.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod mode;
pub mod path;
pub mod plan;
pub mod point;
pub mod visible;

pub use mode::ClickMode;
pub use path::{ax_path_outcome, should_use_ax_path, AxOutcome, Press, Requery};
pub use plan::{passthrough_flags, plan_for, ClickPlan};
pub use point::{end_point, point_for, start_point, Point};
pub use visible::point_visible;