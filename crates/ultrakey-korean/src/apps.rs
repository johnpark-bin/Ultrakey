//! F-16 기본 제외 앱 목록 — `docs/spec/korean-input.md` §3.5, D-K9.
//!
//! ## 선정 기준(명세 §3.5 가 정한 문장 그대로)
//!
//! > **키 입력을 이 맥의 입력 스택이 아니라 다른 OS·다른 세션의 입력 스택으로 그대로
//! > 전달하는 앱.** 원격 데스크톱(RDP)·화면 공유(VNC) 클라이언트, 가상 머신 콘솔이
//! > 여기 해당한다.
//!
//! **근거**: 이 앱들 안에서 한/영·한자 키는 **대상 시스템의 입력기**가 처리해야 하는
//! 것이다. macOS 쪽에서 `⌃Space` 로 바꿔 버리면 대상 시스템에 원래 키가 도달하지
//! 못해, 원격 세션 안에서 한글 입력 자체가 불가능해진다.
//!
//! ⛔ **개인적으로 쓰는 앱을 넣지 않는다.** 이 저장소는 공개다 — 목록은 **공개적으로
//! 알려진 제품**만으로 구성하고, 항목마다 근거 등급을 단다.
//!
//! ⚠️ **`(추정)` 항목이 틀려도 해가 없다** — 틀린 번들 ID 는 어떤 앱과도 일치하지
//! 않아 게이트가 그냥 발화하지 않을 뿐이다. 반대로 **빠뜨린 항목은 그 앱에서 실제로
//! 오작동을 만든다.** 이 비대칭 때문에 `(모름)` 이 아닌 `(추정)` 까지는 포함한다.

/// F-16 기본 제외 목록 12종(`SCRATCH/F16-excluded-apps.md` 표 그대로).
///
/// 근거 등급별 항목:
/// - **실측** — `com.apple.ScreenSharing`(Apple 화면 공유): 이 기기의
///   `/System/Applications/Utilities/Screen Sharing.app` 에서 `CFBundleIdentifier` 를
///   직접 확인했다.
/// - **확실**(공개 1차 출처로 확인) — `com.microsoft.rdc.macos`(Microsoft Remote
///   Desktop 10, Karabiner-Elements 이슈 #1526), `com.teamviewer.TeamViewer`
///   (공개 MDM/PPPC 문서), `com.philandro.anydesk`(AnyDesk, 공개 앱 카탈로그),
///   `com.p5sys.jump.mac.viewer`(Jump Desktop, App Store/공개 카탈로그),
///   `com.parallels.desktop.console`(Parallels Desktop, 공식 문서),
///   `com.utmapp.UTM`(UTM, 공식 저장소·컨테이너 경로),
///   `com.lemonmojo.RoyalTSX`(Royal TSX, Homebrew Cask 정의).
/// - **`(추정)`**(간접 근거, 확정 1차 출처 없음) — `com.microsoft.WindowsApp`
///   (Windows App, 제품 개명은 확인했으나 번들 ID 1차 출처 미확인),
///   `com.vmware.fusion`(VMware Fusion, 공개 문서가 `com.vmware.fusion.application`
///   도 함께 언급해 확정하지 못함), `com.citrix.receiver.icaviewer.mac`
///   (Citrix Workspace, Citrix 지원 KB 구성요소 목록에서 유추),
///   `com.devolutions.remotedesktopmanager`(Remote Desktop Manager, 공식 문서의
///   설정 파일 경로에서 유추).
///
/// ## ⛔ 넣지 않은 것과 그 이유
///
/// - **RealVNC Viewer** — 1차 출처에서 번들 ID 를 확인하지 못했다 `(모름)`.
///   지어내지 않는다.
/// - **VirtualBox** — 같은 이유 `(모름)`. `com.oracle.VirtualBox`/
///   `org.virtualbox.app.VirtualBox` 두 후보가 있고 어느 쪽도 확인되지 않았다.
/// - **VMware Horizon Client** — `(모름)`.
/// - **Chrome Remote Desktop** — 클라이언트가 Chrome 안의 웹 앱이라 **최전면 앱
///   번들 ID 로는 구분할 수 없다.** 게이트가 원리적으로 닿지 않는 대상이라 목록에
///   넣는 것 자체가 무의미하다.
/// - **Remmina / TigerVNC** — macOS 앱 번들이 없거나 확인되지 않았다.
///
/// ⭐ **K5(이슈 #73, D-K17) 갱신** — 이 기본 목록은 이제 *출고 기본값*일 뿐이다.
/// 사용자가 `Korean` 탭에서 목록을 편집하면 그 오버라이드가 이 목록을 **통째로
/// 대체**한다([`resolve_excluded_bundle_ids`]). UI 는 이 목록을 **노출**한다 —
/// 명세 §3.5 의 "목록 편집 UI 를 두지 않는다" 결정은 이슈 #73 이 뒤집었다(§3.5).
pub fn default_excluded_bundle_ids() -> &'static [&'static str] {
    &[
        "com.apple.ScreenSharing",
        "com.microsoft.rdc.macos",
        "com.microsoft.WindowsApp",
        "com.teamviewer.TeamViewer",
        "com.philandro.anydesk",
        "com.p5sys.jump.mac.viewer",
        "com.parallels.desktop.console",
        "com.vmware.fusion",
        "com.utmapp.UTM",
        "com.lemonmojo.RoyalTSX",
        "com.citrix.receiver.icaviewer.mac",
        "com.devolutions.remotedesktopmanager",
    ]
}

/// ⭐ K5(이슈 #73, D-K17) — 저장된 사용자 오버라이드 목록과 기본 목록을 병합해
/// 게이트에 실제로 주입할 최종 목록을 낸다.
///
/// **병합 규칙 — "오버라이드는 완전 대체다"**:
/// - `override_ids` 가 `None`(저장 키 부재) → 기본 목록을 그대로 쓴다(F-15 "부재 =
///   기본값").
/// - `Some(list)` → **기본 목록을 무시하고** 그 값 그대로 쓴다. 빈 배열도 "아무 앱도
///   제외하지 않음"이라는 **명시적 값**이다 — 사용자가 기본 항목을 전부 지운 의도를
///   존중한다. 병합(합집합)이 아니라 대체인 근거: 사용자가 기본 항목 하나를 지웠는데
///   되살아나면 "제거" 조작이 동작하지 않는 것이고, "리스트 하나 = 원자적 단위"라는
///   저장 계층의 성질(per-device-settings.md §3.4)과 맞물린다.
/// - 저장 값 안의 중복·공백은 정규화해 제거한다(손으로 settings.json 을 고친 경우의
///   방어 — 결정론적 순서를 위해 **첫 등장 순서**를 유지한다).
///
/// ⛔ 이 함수는 판정을 하지 않는다 — 최전면 앱 매칭은 `AppGateController` 소관이다.
/// 여기는 `Vec<String>` 조립(정규화)만 한다.
pub fn resolve_excluded_bundle_ids(override_ids: Option<&[String]>) -> Vec<String> {
    let Some(ids) = override_ids else {
        return default_excluded_bundle_ids()
            .iter()
            .map(|s| s.to_string())
            .collect();
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::with_capacity(ids.len());
    for id in ids {
        let trimmed = id.trim();
        if trimmed.is_empty() || !seen.insert(trimmed.to_string()) {
            continue;
        }
        out.push(trimmed.to_string());
    }
    out
}

/// 저장 후보가 되기 전에 행 하나를 정규화한다 — UI 쪽에서도 같은 규칙(공백 제거·
/// 비어 있으면 저장 안 함)을 쓰지만, 이 함수가 정본이다(단일 출처).
pub fn normalize_bundle_id(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── ⭐ K5(이슈 #73, D-K17) — 오버라이드 목록 병합 ─────────────────────────────

    #[test]
    fn resolve_without_override_returns_the_built_in_defaults() {
        assert_eq!(
            resolve_excluded_bundle_ids(None),
            default_excluded_bundle_ids()
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(resolve_excluded_bundle_ids(None).len(), 12);
    }

    /// 오버라이드가 있으면 기본 목록을 **통째로 대체**한다 — 병합하지 않는다(D-K17).
    #[test]
    fn resolve_with_override_replaces_defaults_entirely() {
        let ov = vec!["com.example.MyClient".to_string()];
        let resolved = resolve_excluded_bundle_ids(Some(&ov));
        assert_eq!(resolved, vec!["com.example.MyClient".to_string()]);
        assert!(!resolved.contains(&"com.apple.ScreenSharing".to_string()));
    }

    /// 빈 배열은 "아무 앱도 제외하지 않음"이라는 명시적 의도다 — 기본 목록으로 되돌리지
    /// 않는다. 사용자가 전부 지웠다면 그것이 의도다.
    #[test]
    fn resolve_with_empty_override_yields_empty_list() {
        let empty: Vec<String> = Vec::new();
        assert!(resolve_excluded_bundle_ids(Some(&empty)).is_empty());
    }

    /// 중복 제거는 **첫 등장 순서를 유지**한다(결정론) — 뒤에 온 중복이 앞을 밀어내지
    /// 않는다.
    #[test]
    fn resolve_dedupes_preserving_first_occurrence_order() {
        let input = vec![
            "com.b".to_string(),
            "com.a".to_string(),
            "com.b ".to_string(), // 뒤의 것(공백 포함)이 앞의 것과 같아야 한다
            "  ".to_string(),     // 공백만 — 버려진다
        ];
        let resolved = resolve_excluded_bundle_ids(Some(&input));
        assert_eq!(resolved, vec!["com.b".to_string(), "com.a".to_string()]);
    }

    /// 저장 후보 정규화 — 공백 제거, 빈 값은 None.
    #[test]
    fn normalize_bundle_id_trims_and_rejects_empty() {
        assert_eq!(normalize_bundle_id("  com.x.Y  "), Some("com.x.Y".to_string()));
        assert_eq!(normalize_bundle_id("   "), None);
        assert_eq!(normalize_bundle_id(""), None);
    }

    #[test]
    fn has_exactly_12_entries() {
        assert_eq!(default_excluded_bundle_ids().len(), 12);
    }

    #[test]
    fn entries_are_unique() {
        let ids = default_excluded_bundle_ids();
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), ids.len(), "중복된 번들 ID 가 있다: {ids:?}");
    }

    /// ⭐ 이 기기에서 실측 확인된 유일한 항목 — 회귀 방지.
    #[test]
    fn contains_the_measured_screen_sharing_bundle_id() {
        assert!(default_excluded_bundle_ids().contains(&"com.apple.ScreenSharing"));
    }

    /// 넣지 않기로 판정한 항목이 실수로라도 들어가 있지 않은지 확인한다.
    #[test]
    fn does_not_contain_unconfirmed_candidates() {
        let ids = default_excluded_bundle_ids();
        for excluded in [
            "com.oracle.VirtualBox",
            "org.virtualbox.app.VirtualBox",
            "com.realvnc.vncviewer",
            "com.vmware.horizon.client",
        ] {
            assert!(!ids.contains(&excluded), "확인되지 않은 후보가 목록에 들어갔다: {excluded}");
        }
    }
}