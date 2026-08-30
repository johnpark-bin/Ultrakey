//! F-02 · Screen Recording 권한 — 확인과 **조용한 실패** 감지.
//!
//! ⭐ **이 앱이 두 번째로 필요해진 권한이다.** F-11 §1.1 까지 이 앱이 사전
//! 확인하는 권한은 Accessibility 하나뿐이었다. F-02 의 소스 A 가 화면을
//! 캡처하면서 Screen Recording 이 실제로 필요해진다.
//!
//! ⚠️ **`CGDisplayCreateImage` 는 권한이 없어도 실패하지 않는다.** 오류가 아니라
//! **데스크톱 배경(벽지)만 담긴 이미지**가 돌아온다 — 창도 메뉴 막대도 없다.
//! 이것이 조용한 실패이고, 감지하지 않으면 사용자에게는 "Seek 가 아무것도 못
//! 찾는다" 로만 보인다.
//!
//! ## 이 구현이 원본과 다른 점 — 의도한 차이
//!
//! 명세 §6 은 원본 v1.66 번들에 `CGPreflightScreenCaptureAccess` /
//! `CGRequestScreenCaptureAccess` 심볼이 **없다**고 실측했다. 원본은 사전 확인
//! 없이 `CGDisplayCreateImage` 를 부르고 OS 의 암묵적 프롬프트에 맡긴다.
//!
//! 이 구현은 **preflight 를 쓴다.** 근거:
//! - `CGPreflightScreenCaptureAccess` 는 **프롬프트를 띄우지 않는** 순수 조회다.
//!   즉 F-11 §1.1 이 경계한 "시스템 프롬프트가 우리 자체 모달을 대신해 버리는"
//!   문제가 생기지 않는다. 그 경계는 `AXIsProcessTrustedWithOptions`(프롬프트
//!   변형)에 대한 것이었고, 여기에는 해당하지 않는다.
//! - 조용한 실패를 사용자에게 설명하려면 "권한이 없다" 를 **알아야** 한다.
//!   결과 이미지만 보고 추론하는 것(후보 0개 → 권한 없음?)은 텍스트 없는
//!   화면과 구분되지 않는다(명세 §5 #2 와 #1 이 서로 다른 케이스인 이유).
//!
//! 기각한 대안: 원본과 똑같이 아무 확인도 하지 않기 — 그러면 §5 #1 의 기대
//! 동작("소스 A 전체가 죽고 소스 B 로 계속")은 만족하지만 사용자에게 이유를
//! 말해 줄 수 없다.

/// 권한 상태 판정 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRecordingStatus {
    /// 권한이 있다.
    Granted,
    /// 권한이 없다 — 캡처하면 데스크톱 배경만 돌아온다.
    Denied,
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::ScreenRecordingStatus;

    /// `CGPreflightScreenCaptureAccess()` — **프롬프트 없이** 현재 권한만 조회한다.
    #[must_use]
    pub fn has_screen_recording_access() -> bool {
        objc2_core_graphics::CGPreflightScreenCaptureAccess()
    }

    /// `CGRequestScreenCaptureAccess()` — 시스템 권한 프롬프트를 띄운다.
    ///
    /// ⚠️ 이 프롬프트는 **프로세스당 한 번만** 뜬다. 이미 거부한 사용자에게는
    /// 아무 일도 일어나지 않으므로, 반환값이 `false` 면 상위 계층(F-11)이
    /// 시스템 설정으로 안내해야 한다.
    #[must_use]
    pub fn request_screen_recording_access() -> bool {
        objc2_core_graphics::CGRequestScreenCaptureAccess()
    }

    /// 현재 상태.
    #[must_use]
    pub fn status() -> ScreenRecordingStatus {
        if has_screen_recording_access() {
            ScreenRecordingStatus::Granted
        } else {
            ScreenRecordingStatus::Denied
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{has_screen_recording_access, request_screen_recording_access, status};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::ScreenRecordingStatus;

    #[must_use]
    pub fn has_screen_recording_access() -> bool {
        false
    }

    #[must_use]
    pub fn request_screen_recording_access() -> bool {
        false
    }

    #[must_use]
    pub fn status() -> ScreenRecordingStatus {
        ScreenRecordingStatus::Denied
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{has_screen_recording_access, request_screen_recording_access, status};
