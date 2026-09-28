//! ⭐(이슈 #93) — 최전면 앱 pid 조회·활성화. 다국어 (인풋 박스) Seek 세션의
//! 포커스 핸드오프에 쓴다.
//!
//! - [`frontmost_pid`] — 세션 열림 시점의 최전면 앱을 기록(메인 스레드.
//!   `NSWorkspace.sharedWorkspace().frontmostApplication()`).
//! - [`activate_pid`] — 클릭 없이 닫힌 세션이 이전 앱으로 포커스를 되돌릴 때
//!   사용. `click_executor.rs` 의 `activate_application` 과 같은 호출이지만,
//!   그쪽은 `tauri::AppHandle` 에 묶인 채라 재사용성이 낮아 여기 한 번 더 둔다
//!   (같은 `NSRunningApplication.activateWithOptions(ActivateIgnoringOtherApps)`
//!   경로 — 이 패턴은 이미 이슈 #44 에서 실기 검증됐다).

/// 세션 열림 시점의 최전면 앱 pid. `None` = 판정 불가(활성 앱 없음, 접근 오류).
#[cfg(target_os = "macos")]
pub fn frontmost_pid() -> Option<i32> {
    let workspace = objc2_app_kit::NSWorkspace::sharedWorkspace();
    workspace
        .frontmostApplication()
        .map(|app| app.processIdentifier())
}

/// pid 로 앱을 활성화한다. 실패·종료된 앱이면 `false`(no-op).
#[cfg(target_os = "macos")]
#[allow(deprecated)] // ⚠️ `ActivateIgnoringOtherApps` 는 macOS 14 에서 deprecated
// 이지만 최소 지원 12.0 을 위해 의도적으로 쓴다(`click_executor.rs` 와 같은 판단).
pub fn activate_pid(pid: i32) -> bool {
    use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
    let Some(running) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
        return false;
    };
    running.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps)
}

#[cfg(not(target_os = "macos"))]
pub fn frontmost_pid() -> Option<i32> {
    None
}

#[cfg(not(target_os = "macos"))]
pub fn activate_pid(_pid: i32) -> bool {
    false
}