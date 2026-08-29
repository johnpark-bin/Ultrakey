//! 앱 번들 여부·번들 ID·OS 버전·시스템 설정 딥링크.

/// 실행 파일 경로가 `.app/Contents/MacOS/` 안인가.
///
/// F-11 수용 기준("`tauri dev` 로 실행한 프로세스에서는 경고가 표시된다")이
/// 이 판정에 기댄다 — `tauri dev` 는 `target/debug/<binary>` 를 직접
/// 실행하므로 이 경로 조각이 없다(`platform-constraints.md` §3.4).
///
/// 순수 `std::env::current_exe()` 문자열 비교라 macOS 가 아닌 타깃에서도
/// 그대로 컴파일·동작한다(다른 OS 에서는 항상 `false`).
pub fn is_running_from_app_bundle() -> bool {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.to_str().map(|s| s.contains(".app/Contents/MacOS/")))
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
mod macos_impl {
    /// `NSBundle.mainBundle().bundleIdentifier`.
    pub fn bundle_identifier() -> Option<String> {
        // SAFETY: 없음(순수 안전 API 호출) — `mainBundle()`/`bundleIdentifier()`
        // 모두 objc2-foundation 이 안전 래퍼로 노출한다.
        objc2_foundation::NSBundle::mainBundle()
            .bundleIdentifier()
            .map(|s| s.to_string())
    }

    /// `NSProcessInfo.processInfo().operatingSystemVersion` → `(major, minor)`.
    pub fn macos_version() -> (u32, u32) {
        let version = objc2_foundation::NSProcessInfo::processInfo().operatingSystemVersion();
        (version.majorVersion as u32, version.minorVersion as u32)
    }


    /// 사용자의 선호 언어 목록(우선순위 순, 예: `["ko-KR", "en-US"]`).
    ///
    /// F-14(A)/D4 로케일 결정(`localization-and-input-sources.md` §3.1.2)의 입력이다 —
    /// `ultrakey_i18n::Catalog::resolve` 가 이 목록을 앞에서부터 훑어 지원 로케일을 고른다.
    pub fn preferred_languages() -> Vec<String> {
        objc2_foundation::NSLocale::preferredLanguages()
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    /// 시스템 설정 딥링크 등 임의 URL 을 연다.
    pub fn open_url(url: &str) -> bool {
        let ns_url_string = objc2_foundation::NSString::from_str(url);
        let Some(ns_url) = objc2_foundation::NSURL::URLWithString(&ns_url_string) else {
            return false;
        };
        objc2_app_kit::NSWorkspace::sharedWorkspace().openURL(&ns_url)
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{bundle_identifier, macos_version, open_url, preferred_languages};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    pub fn bundle_identifier() -> Option<String> {
        None
    }
    pub fn macos_version() -> (u32, u32) {
        (0, 0)
    }
    pub fn open_url(_url: &str) -> bool {
        false
    }
    pub fn preferred_languages() -> Vec<String> {
        Vec::new()
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{bundle_identifier, macos_version, open_url, preferred_languages};
