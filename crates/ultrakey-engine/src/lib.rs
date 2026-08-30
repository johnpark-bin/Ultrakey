//! `ultrakey-engine` — F-07(`key-remapping-engine.md`) 의 **인프라** 부분.
//!
//! ⭐ 판정 로직(중재 우선순위 표·quick press 상태 머신)은 `ultrakey-core` 소관이다.
//! 이 크레이트는 그 판정을 실제로 구동하는 인프라만 다룬다: 전용 탭 스레드의
//! `CFRunLoop`, 탭 생명주기 FSM(`lifecycle`), 워치독(`watchdog`), 절전·깨어남·세션·
//! 핫플러그·입력소스 훅(`system_hooks`), 경로 B 관리(`path_b`), 그리고 위 전부를
//! 조립하는 `Engine`(`engine`).
//!
//! 설계 근거는 `docs/dev/architecture.md` §2(콜백 동시성·수명 설계)를 그대로 따른다 —
//! 이 크레이트의 모듈 하나하나가 그 문서의 절 하나씩에 대응한다.
//!
//! ⛔ **`unsafe` 코드가 전혀 없다.** 이 저장소의 모든 `unsafe` FFI 는
//! `ultrakey-platform` 에만 있다(그 크레이트 최상단 문서 참고). 이 크레이트는 그
//! 크레이트가 노출하는 안전한 API 만 조합한다.

#![forbid(unsafe_code)]

pub mod command;
pub mod engine;
pub mod lifecycle;
pub mod path_b;
pub mod state;
pub mod system_hooks;
pub mod trace;
pub mod watchdog;

pub use command::{CommandChannel, EngineCommand};
pub use engine::{Engine, EngineError, EngineEvent};
pub use lifecycle::TapState;
pub use path_b::{
    d1_for, d1_mapping, GlobalD1Migration, HidutilGlobalMigration, LedgerStore, NullLedgerStore,
    PathBManager, ReconcileReport,
};
pub use state::SharedState;
pub use system_hooks::SystemHooks;
pub use watchdog::Watchdog;
