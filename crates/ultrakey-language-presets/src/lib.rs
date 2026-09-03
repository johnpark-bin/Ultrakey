//! `ultrakey-language-presets` — F-19 언어별 프리셋(`docs/spec/language-presets.md`)의
//! **ja·zh 규칙 정의**(P19~P23, `docs/dev/architecture.md` §6.4)와 언어 규칙 충돌 감지.
//!
//! ⭐ `ultrakey-presets`/`ultrakey-korean` 과 정확히 같은 자리다: 메커니즘을 스스로
//! 갖지 않는다(그것은 F-07 `ultrakey-core::arbitration` 의 일이다). 여기서 하는 일은
//! `日本語`/`中文` 노드의 설정값을 `ultrakey_core::rules::LanguageRule` 목록으로
//! 번역하는 것과, 같은 물리 키코드를 소스로 주장하는 활성 규칙 충돌(F-15 대화상자)
//! 을 감지하는 것뿐이다.
//!
//! ⭐ **ko 노드(F-19.1·F-19.2)는 여기 있지 않다** — 정의는 F-19 이지만 저장 키가
//! `korean.*` 네임스페이스라 `ultrakey-korean` 의 `KoreanSettings::to_language_rules()`
//! 가 발행한다(명세 §3.0 소유권 경계).
//!
//! ⭐ **카탈로그를 읽지 않는다**(명세 §8 부정형): F-19.6 심볼 치환 20행은
//! `pqrs-org/KE-complex_modifications` 의 `jis_to_us_symbols.json` 을 2026-09-03 에
//! fetch 해 대조한 뒤 이 크레이트에 **하드코딩**했다. 런타임·빌드타임 임포트 경로가
//! 없다.

#![forbid(unsafe_code)]

pub mod conflicts;
pub mod settings;

pub use conflicts::{detect_language_conflict, LanguageConflict, LanguageConflictKind};
pub use settings::{ChineseSettings, JapaneseSettings};