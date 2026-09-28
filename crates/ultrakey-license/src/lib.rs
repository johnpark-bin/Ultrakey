//! F-12 라이선싱과 트라이얼 — 순수 로직.
//!
//! `docs/spec/licensing-and-trial.md` §7 은 이 기능의 **상태 머신(§3.2)·요청/응답
//! 계약(§3.3)·오프라인 유예 판정(§3.4)·시계 조작 완화(§3.5)는 전부 순수 애플리케이션
//! 로직**이라 판정했다. 이 크레이트가 그 순수 로직을 담는다.
//!
//! ⭐ 제품 결정 D2 · 명세 §7 — **실제 Paddle 은 붙이지 않는다.** 상태 머신·유예·시계
//! 보정은 `LicenseProvider` 추상 계약 뒤에 오고, 초기 구현은 모든 기기를 항상 라이선스
//! 활성으로 취급하는 **no-op 구현체**(`noop::NoopLicenseProvider`)가 기본이다. 나중에
//! 실제 Paddle 연동을 끼워 넣을 때 상태 머신이나 UI 를 다시 설계할 필요가 없도록,
//! 이 크레이트는 provider 의 정체(no-op vs 실연동)와 무관하게 검증된다.
//!
//! macOS 비의존 — `#![forbid(unsafe_code)]`. Keychain·IOKit 같은 FFI 는
//! `ultrakey-platform` 의 책임이고, 이 크레이트는 그 어댑터가 제공하는 순수 입력
//! (시계 [`Clock`]·기산점 저장 [`TrialStore`]·캐시 저장 [`CacheStore`])만 소비한다.

#![forbid(unsafe_code)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clock;
pub mod decision;
pub mod file_store;
pub mod machine;
pub mod noop;
mod provider;
mod state;
mod store;

pub use clock::{system_time_now, Clock, SystemClock, Timestamp};
pub use decision::{
    cache_within_grace, is_clock_rollback, next_last_seen, OfflineGrace, TrialDecisionOutcome,
    TRIAL_DAYS,
};
pub use machine::{
    ActivationOutcome, ActivationsView, DeactivationOutcome, EvaluateResult, LicenseMachine,
    RevalidateOutcome, TrialInfo,
};
pub use noop::NoopLicenseProvider;
pub use provider::{
    ActivationRequest, ActivationResponse, DeactivateRequest, DeactivateResponse, LicenseKey,
    LicenseProvider, ValidateRequest, ValidateResponse, ValidationStatus,
};
pub use file_store::FileStore;
pub use state::LicenseState;
pub use store::{CacheStore, InMemoryStore, LicenseCache, TrialClock, TrialRecord, TrialStore};
