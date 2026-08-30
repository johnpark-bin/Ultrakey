//! `ultrakey-presets` — F-08 Power User Presets(`docs/spec/power-user-presets.md`)의
//! **규칙 정의** 크레이트.
//!
//! ⭐ `ultrakey-hyperkey` 와 같은 자리다: 이벤트를 가로채는 메커니즘을 스스로 갖지 않는다
//! (그것은 F-07 `ultrakey-engine` 의 일이다). 여기서 하는 일은 `Presets` 탭의 설정값을
//! `ultrakey_core::rules::{ComboRule, SimpleRemap, SourceKeyActions}` 목록으로 번역하는
//! 것뿐이다. macOS 에 전혀 의존하지 않고, `unsafe` 도 전혀 필요 없다.
//!
//! 중재 설계 정본은 `docs/dev/architecture.md` §6(D-1, P1~P12, §6.3~§6.6)이다 — 이
//! 크레이트는 그 결정을 코드로 옮길 뿐 스스로 재설계하지 않는다.

#![forbid(unsafe_code)]

pub mod popups;
pub mod rules;
pub mod settings;
pub mod conflicts;

pub use popups::*;
pub use rules::PresetRules;
pub use settings::PresetSettings;
pub use conflicts::{detect_conflict, Conflict, ConflictKind};
