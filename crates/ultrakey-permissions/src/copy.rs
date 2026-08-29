//! UI 가 그대로 표시할 안내 문구 묶음. ⭐ 문자열은 전부 [`ultrakey_i18n::Catalog`]
//! 에서 온다 — 하드코딩하지 않는다(D4). 이 모듈은 카탈로그 키 이름과 OS 버전
//! 분기(§3.2, §2 S2 항목 4)만 알고 있으면 되고, 문구 자체의 내용은 전혀 모른다.

use ultrakey_i18n::Catalog;

/// UI 가 그대로 표시할 Accessibility 온보딩 안내 문구 묶음(§3.2).
pub struct OnboardingCopy {
    pub title: String,
    pub body: String,
    pub path: String,
    pub open_button: String,
    pub checkbox_hint: String,
    pub locked_hint: String,
}

/// ⭐ `path`·`open_button` 은 [`Catalog::get_os_variant`] 로 OS 버전에 따라 문구를
/// 바꾼다(§2 S2 항목 4, §3.2 — macOS 13 미만이면 "시스템 환경설정", 이상이면
/// "시스템 설정"). `ultrakey_platform::bundle::macos_version()` 의 major 를 넘긴다.
pub fn onboarding_copy(catalog: &Catalog) -> OnboardingCopy {
    let (macos_major, _minor) = ultrakey_platform::bundle::macos_version();

    OnboardingCopy {
        title: catalog.get("permissions.accessibility.title").to_string(),
        body: catalog.get("permissions.accessibility.body").to_string(),
        path: catalog
            .get_os_variant("permissions.accessibility.path", macos_major)
            .to_string(),
        open_button: catalog
            .get_os_variant("permissions.accessibility.open_settings", macos_major)
            .to_string(),
        checkbox_hint: catalog
            .get("permissions.accessibility.checkbox_hint")
            .to_string(),
        locked_hint: catalog
            .get("permissions.accessibility.locked_hint")
            .to_string(),
    }
}

/// UI 가 그대로 표시할 out-of-sync 진단 화면 문구 묶음(§3.3). ⭐ (a) 자체 리셋
/// 경로는 의도적으로 여기 없다 — 이 크레이트는 (b) 수동 절차 안내만 제공한다
/// (모듈 문서 및 `lib.rs` 참고).
pub struct OutOfSyncCopy {
    pub title: String,
    pub body: String,
    pub manual_steps: String,
    pub quit: String,
}

/// `manual_steps` 는 카탈로그의 `{0}` 자리에 앱 이름(`app.name`)을 넣어
/// [`Catalog::format`] 으로 만든다.
pub fn out_of_sync_copy(catalog: &Catalog) -> OutOfSyncCopy {
    let app_name = catalog.get("app.name");

    OutOfSyncCopy {
        title: catalog.get("permissions.out_of_sync.title").to_string(),
        body: catalog.get("permissions.out_of_sync.body").to_string(),
        manual_steps: catalog.format("permissions.out_of_sync.manual_steps", &[app_name]),
        quit: catalog.get("permissions.out_of_sync.quit").to_string(),
    }
}

/// ⭐ §8: "`tauri dev` 로 실행한 프로세스에서는 경고가 표시된다".
/// `.app` 번들 밖에서 실행 중이면 `Some((제목, 본문))`.
pub fn dev_build_warning(catalog: &Catalog) -> Option<(String, String)> {
    if ultrakey_platform::bundle::is_running_from_app_bundle() {
        return None;
    }

    Some((
        catalog.get("dev.not_app_bundle.title").to_string(),
        catalog.get("dev.not_app_bundle.body").to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultrakey_i18n::Locale;

    /// 카탈로그 키 오타가 있으면 `Catalog::get` 이 키 이름 자체를 그대로
    /// 반환한다(존재하지 않는 키 조회 시의 폴백 동작) — 그 값이 새어 나오면
    /// 카탈로그 키가 잘못됐다는 뜻이므로 실패로 간주한다.
    fn assert_looks_translated(label: &str, value: &str) {
        assert!(!value.is_empty(), "{label} 이 비어 있다");
        assert!(
            !value.starts_with("permissions.") && !value.starts_with("dev."),
            "{label} 이 카탈로그 키 이름을 그대로 반환했다: {value:?}"
        );
    }

    #[test]
    fn onboarding_copy_has_all_strings_populated_in_both_locales() {
        for locale in Locale::all() {
            let catalog = Catalog::for_locale(*locale);
            let copy = onboarding_copy(&catalog);
            assert_looks_translated("title", &copy.title);
            assert_looks_translated("body", &copy.body);
            assert_looks_translated("path", &copy.path);
            assert_looks_translated("open_button", &copy.open_button);
            assert_looks_translated("checkbox_hint", &copy.checkbox_hint);
            assert_looks_translated("locked_hint", &copy.locked_hint);
        }
    }

    #[test]
    fn out_of_sync_copy_has_all_strings_populated_in_both_locales() {
        for locale in Locale::all() {
            let catalog = Catalog::for_locale(*locale);
            let copy = out_of_sync_copy(&catalog);
            assert_looks_translated("title", &copy.title);
            assert_looks_translated("body", &copy.body);
            assert_looks_translated("manual_steps", &copy.manual_steps);
            assert_looks_translated("quit", &copy.quit);
        }
    }

    /// ⭐ `{0}` 자리 표시자가 남아 있지 않고 앱 이름(`Ultrakey`)이 들어가 있다.
    #[test]
    fn out_of_sync_manual_steps_substitutes_app_name() {
        for locale in Locale::all() {
            let catalog = Catalog::for_locale(*locale);
            let copy = out_of_sync_copy(&catalog);
            assert!(
                !copy.manual_steps.contains("{0}"),
                "치환되지 않은 자리 표시자가 남아 있다: {:?}",
                copy.manual_steps
            );
            assert!(
                copy.manual_steps.contains("Ultrakey"),
                "앱 이름이 들어가 있지 않다: {:?}",
                copy.manual_steps
            );
        }
    }

    /// `cargo test` 로 실행되는 테스트 바이너리는 `.app/Contents/MacOS/` 안에
    /// 있을 수 없다 — `tauri dev` 와 동일한 조건이므로 항상 경고가 나와야 한다.
    #[test]
    fn dev_build_warning_fires_outside_app_bundle() {
        let catalog = Catalog::for_locale(Locale::En);
        let warning = dev_build_warning(&catalog);
        let (title, body) = warning.expect("테스트 바이너리는 앱 번들 밖에서 실행된다");
        assert_looks_translated("title", &title);
        assert_looks_translated("body", &body);
    }

    /// macOS 13 이상/미만에서 `path`·`open_button` 문구가 달라야 한다(§2 S2
    /// 항목 4 — "시스템 설정" vs "시스템 환경설정").
    #[test]
    fn onboarding_copy_path_and_open_button_differ_by_macos_major() {
        // `onboarding_copy` 는 `ultrakey_platform::bundle::macos_version()` 을
        // 직접 호출하므로(비macOS 타깃에서는 항상 (0, 0)), 여기서는 그 안에서
        // 위임하는 `Catalog::get_os_variant` 자체가 버전에 따라 값을 바꾼다는
        // 것을 카탈로그 키 단위로 직접 검증한다 — `permissions-onboarding.md`
        // §3.2 가 요구하는 대응이 실제로 두 값의 차이로 나타나는지 확인한다.
        for locale in Locale::all() {
            let catalog = Catalog::for_locale(*locale);
            let modern_path = catalog.get_os_variant("permissions.accessibility.path", 13);
            let legacy_path = catalog.get_os_variant("permissions.accessibility.path", 12);
            assert_ne!(modern_path, legacy_path);

            let modern_button =
                catalog.get_os_variant("permissions.accessibility.open_settings", 13);
            let legacy_button =
                catalog.get_os_variant("permissions.accessibility.open_settings", 12);
            assert_ne!(modern_button, legacy_button);
        }
    }
}
