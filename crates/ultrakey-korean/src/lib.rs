//! `ultrakey-korean` — F-16 한국어 입력 지원(`docs/spec/korean-input.md`)의 **규칙 정의**
//! 크레이트.
//!
//! ⭐ `ultrakey-presets`/`ultrakey-hyperkey` 와 정확히 같은 자리다: 이벤트를 가로채는
//! 메커니즘을 스스로 갖지 않는다(그것은 F-07 `ultrakey-engine`/`ultrakey-core::arbitration`
//! 의 일이다). 여기서 하는 일은 `Korean` 탭의 설정값을
//! `ultrakey_core::rules::KoreanRule` 목록으로 번역하는 것과, F-16 기본 제외 앱 목록을
//! 소유하는 것뿐이다. macOS 에 전혀 의존하지 않고, `unsafe` 도 전혀 필요 없다.
//!
//! ⚠️ **1단계 범위**: F-16.1(`Shift+Space`)과 F-16.4(`₩`) 두 규칙만 낸다. 한/영(F-16.2)·
//! 한자(F-16.3)는 설정 필드만 존재하고 [`KoreanSettings::to_rules`] 가 규칙을 하나도
//! 내지 않는다 — `lang1`/`lang2` 의 virtual keycode 가 `(미확정)`이기 때문이다
//! (`docs/spec/korean-input.md` §3.2, D-K8).

#![forbid(unsafe_code)]

pub mod apps;
pub mod settings;

pub use apps::default_excluded_bundle_ids;
pub use settings::KoreanSettings;
