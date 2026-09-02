//! `ultrakey-platform` — macOS FFI 경계.
//!
//! ⭐ **이 저장소의 모든 `unsafe` 가 여기에만 있다.** 나머지 크레이트는 전부
//! `#![forbid(unsafe_code)]` 다. 이 크레이트의 목적은 그 `unsafe` FFI 를
//! 안전한 러스트 API 로 감싸 노출하는 것이다 — 각 `unsafe` 블록 위에는
//! "이 호출이 지키는 불변식이 무엇인가"를 한국어 `SAFETY:` 주석으로 남긴다.
//!
//! **macOS 가 아닌 타깃에서도 컴파일된다.** 각 모듈은 `#[cfg(target_os =
//! "macos")]` 로 실제 구현을, `#[cfg(not(target_os = "macos"))]` 로 컴파일만
//! 되는 스텁을 둔다(테스트 CI 대비) — `hid_mapping` 은 `std::process::Command`
//! 만 쓰므로 애초에 `unsafe` 도 플랫폼 분기도 필요 없다.
//!
//! 모듈 구성은 `docs/spec/key-remapping-engine.md` §6(필요한 플랫폼 API)과
//! `docs/dev/architecture.md`(경로 A/B/C 배치)를 그대로 따른다.

pub mod accessibility;
pub mod apps;
pub mod ax_text;
pub mod bundle;
pub mod click_synthesis;
pub mod device_id;
pub mod event;
pub mod event_tap;
pub mod fn_state;
pub mod hid_device;
pub mod hid_lock;
pub mod hid_mapping;
pub mod hotplug;
pub mod image_preprocess;
pub mod keychain;
pub mod login_item;
pub mod multitouch;
pub mod overlay_window;
pub mod runloop;
pub mod screen_capture;
pub mod screen_recording;
pub mod screens;
pub mod secure_input;
pub mod single_instance;
pub mod text_input_source;
pub mod trace_ring;
pub mod vision_ocr;
pub mod workspace;

/// macOS 전용 수기 FFI 선언 모음. 공개 헤더에서 확인한 시그니처만 담는다
/// (근거는 `ffi.rs` 자체의 주석 참조) — 크레이트 내부에서만 쓴다.
#[cfg(target_os = "macos")]
mod ffi;
