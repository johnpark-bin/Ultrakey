//! `ultrakey-seek-session` — F-01 Seek 활성화·세션 상태 머신의 순수 로직.
//!
//! 대응 명세: `docs/spec/seek-activation-and-session.md`.
//!
//! ⭐ 이 크레이트는 F-01 이 정의하는 것만 정의한다 — **활성화 경로 3종**
//! (전역 단축키 토글, 키 리매핑 트리거, `Presets` 탭 quick press caps lock),
//! **세션의 생명주기**(유휴→시작→후보준비→질의→선택→확정/취소), **세션 중 키
//! 입력의 라우팅 규칙**(명세 §1). 후보를 어떻게 찾는지(F-02), 어떻게
//! 그리는지(F-03), 클릭을 어떻게 합성하는지(F-04)는 이 크레이트의 관심사가
//! 아니다 — 각각 `ultrakey-seek`·`ultrakey-overlay`·(장차) F-04 구현체의
//! 몫이다.
//!
//! ⭐ 매치 선택·순환·질의 필터링·200개 상한(R-4)은 **여기서 다시 만들지
//! 않는다.** [`ultrakey_overlay::OverlaySession`] 이 이미 그 문제를 풀어
//! 뒀고, [`machine::SeekSessionMachine`] 은 그것을 소유(compose)한 채
//! **생명주기·모드·활성화 경로·키 라우팅**만 더한다.
//!
//! ⭐ **S-6 (`docs/dev/seek-ocr-latency-spike.md` §5)** — 세션은 F-02 의
//! 검출이 끝나기를 기다리지 않는다. `activate()` 는 후보 0개인 채로 즉시
//! 세션을 연다. 검출 완료는 `finish_detection()` 이 별도로 알려준다.
//!
//! ⭐ 이 크레이트에는 `unsafe` 도 macOS 의존성도 없다. F-01 의 입력 경계
//! (물리 키코드·이벤트 종류 판정)는 `ultrakey-core` 가 이미 macOS 독립적으로
//! 정의해 둔 타입(`InputEvent`/`KeyCode`/`EventFlags`)에 의존할 뿐, `CGEventTap`
//! 이나 `global-hotkey` 를 직접 호출하지 않는다 — 그래서
//! `cargo test -p ultrakey-seek-session` 은 macOS 가 아닌 곳에서도 전량
//! 검증된다(명세 §7 "Rust 바인딩" 판정의 의미가 바로 이것이다: F-01 의 로직
//! 자체는 순수하고, 플랫폼 바인딩 타입은 F-07 이 만들어 전달한다).

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod config;
pub mod confirm;
pub mod keys;
pub mod machine;
pub mod settings;

pub use config::{ActivationPath, SeekConfig, SessionMode};
pub use confirm::{ClickError, ClickExecutor, ClickSettings, ConfirmedMatch, NullClickExecutor};
pub use keys::{classify, SessionKey};
pub use machine::{CloseReason, SeekSessionMachine, SessionEffect, SessionState};
pub use settings::{SeekSettings, SeekShortcut};
