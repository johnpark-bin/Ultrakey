//! macOS `Use F1, F2, etc. keys as standard function keys` 토글 읽기.
//!
//! `docs/spec/per-device-settings.md` §3.5.1 이 이 토글을 "F-17 기능 2 의 전제
//! 조건"으로 요구한다 — 우리가 이 값을 바꾸지는 않는다(사용자의 시스템 설정을
//! 대신 조작하지 않는다), `Keyboards` 탭 상단에 현재 상태만 표시한다.
//!
//! ⭐ **키 이름·도메인은 이 워크트리에서 직접 실측 확인했다**(CONTRACT.md §0.4):
//! ```text
//! $ defaults read -g com.apple.keyboard.fnState
//! 1
//! ```
//! `NSGlobalDomain`(`defaults` 의 `-g`)의 `com.apple.keyboard.fnState` 가 맞는
//! 키·도메인이다. 값 `1` = 토글 켜짐(F1~F12 를 표준 F-키로 사용), 키가 아예
//! 없으면 = 꺼짐(macOS 기본값)으로 해석한다.
//!
//! 구현은 `defaults` 서브프로세스를 쓰지 않고 `CFPreferencesCopyValue` 로 직접
//! 읽는다 — `NSGlobalDomain` 을 읽는 정식 경로다(`ffi.rs` 의 실측 표 참고:
//! App 계열 함수는 헤더가 `kCFPreferencesAnyApplication` 과 함께 쓰지 말라고
//! 권고하고, `CurrentHost` 로는 값이 오지 않는다).

#[cfg(target_os = "macos")]
mod macos_impl {
    use crate::ffi;
    use objc2_core_foundation::{CFBoolean, CFNumber, CFRetained, CFString, CFType};
    use std::ptr::NonNull;

    /// 실측 확정 키 이름(CONTRACT.md §0.4) — `NSGlobalDomain` 아래.
    const K_FN_STATE_KEY: &str = "com.apple.keyboard.fnState";

    /// `Use F1, F2, etc. keys as standard function keys` 의 현재 값을 읽는다.
    ///
    /// - `Some(true)` — 켜짐. F1~F12 단독 입력이 표준 F-키 usage 로 도착한다
    ///   (F-17 기능 2 의 전제 조건, §3.5).
    /// - `Some(false)` — 꺼짐. 키가 아예 없을 때도 이 값으로 해석한다 — 이것이
    ///   macOS 기본값이기 때문이다.
    /// - `None` — 읽기 자체가 실패했다(예: 값이 있지만 우리가 아는 타입이
    ///   아니다). UI 는 이 경우를 "알 수 없음"으로 표시한다.
    pub fn f_keys_are_standard() -> Option<bool> {
        let key = CFString::from_str(K_FN_STATE_KEY);
        // SAFETY: 세 상수 모두 CoreFoundation 이 항상 정의하는 정적 심볼이다 —
        // 정적 심볼 읽기 자체가 unsafe 다.
        let (app_id, user, host) = unsafe {
            (
                ffi::kCFPreferencesAnyApplication?,
                ffi::kCFPreferencesCurrentUser?,
                ffi::kCFPreferencesAnyHost?,
            )
        };
        // SAFETY: 네 인자 모두 이 호출 동안 유효한 CFString 이다. 반환값은
        // CF_RETURNS_RETAINED(문서: "Caller must release the returned value") —
        // 우리가 소유권을 받는다(ffi.rs 주석 참고).
        let raw = unsafe {
            ffi::CFPreferencesCopyValue(
                &*key as *const CFString,
                app_id as *const CFString,
                user as *const CFString,
                host as *const CFString,
            )
        };
        let Some(ptr) = NonNull::new(raw as *mut CFType) else {
            // 키가 아예 없다 — macOS 기본값(꺼짐)으로 해석한다(§3.5.1, CONTRACT §0.4).
            return Some(false);
        };
        // SAFETY: 위 근거 참조 — 소유권이 이미 우리에게 있다.
        let value = unsafe { CFRetained::from_raw(ptr) };

        match value.downcast::<CFBoolean>() {
            Ok(boolean) => Some(boolean.value()),
            Err(value) => match value.downcast::<CFNumber>() {
                Ok(number) => number.as_i64().map(|v| v != 0),
                // 값이 있지만 Boolean 도 Number 도 아니다 — 읽기 실패로 취급한다.
                Err(_) => None,
            },
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::f_keys_are_standard;

#[cfg(not(target_os = "macos"))]
pub fn f_keys_are_standard() -> Option<bool> {
    None
}
