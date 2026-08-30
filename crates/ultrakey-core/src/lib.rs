//! `ultrakey-core` — F-07 판정 로직(`key-remapping-engine.md`) · F-10 게이트 상태(`gate.rs`).
//!
//! macOS 에 의존하지 않는 **순수 판정 로직** 크레이트다. 어떤 macOS 크레이트도 의존하지
//! 않으며, `cargo test -p ultrakey-core` 만으로 실제 키보드·`CGEventTap` 없이 전부 검증
//! 가능해야 한다 — 그것이 이 크레이트가 존재하는 이유다(`docs/dev/architecture.md` §1).

#![forbid(unsafe_code)]

pub mod arbitration;
pub mod event;
pub mod flags;
pub mod gate;
pub mod keycode;
pub mod keystate;
pub mod korean;
pub mod perdevice;
pub mod quickpress;
pub mod rules;
pub mod settings;
pub mod time;
