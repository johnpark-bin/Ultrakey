//! F-11 — Accessibility 권한 확인.
//!
//! ⚠️ **`AXIsProcessTrustedWithOptions` 는 의도적으로 선언하지 않는다.**
//! `permissions-onboarding.md`(F-11) §1.1 이 실측으로 확정한 사실은, SuperKey
//! 원본이 이 "프롬프트 변형"(옵션에 `kAXTrustedCheckOptionPrompt`를 넣어
//! 시스템 권한 요청 팝업을 띄우는 API) 을 쓰지 않는다는 것이다 —
//! `platform-constraints.md` §0.1/§0.2(c) 가 근거로 삼는 실측(`nm -u`)에서
//! 이 심볼이 링크되어 있지 않았다. 원본은 시스템 프롬프트 대신 **자체
//! 모달**로 시스템 설정(개인정보 보호 및 보안 ▸ 손쉬운 사용)으로 안내한다.
//! 이 크레이트가 프롬프트를 띄우는 API 를 아예 노출하지 않으면, 상위
//! 계층(F-11)이 실수로 시스템 프롬프트 경로를 골라 원본과 다른 사용자
//! 경험을 만들 여지 자체가 없어진다.

#[cfg(target_os = "macos")]
mod macos_impl {
    /// `AXIsProcessTrusted()` — 이 프로세스가 Accessibility 권한을 부여받았는가.
    pub fn is_process_trusted() -> bool {
        // SAFETY: `AXIsProcessTrusted` 는 인자가 없고 부작용도 없는(순수
        // 조회) 함수다 — 매 호출이 항상 안전하다.
        unsafe { crate::ffi::AXIsProcessTrusted() != 0 }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::is_process_trusted;

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    pub fn is_process_trusted() -> bool {
        false
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::is_process_trusted;
