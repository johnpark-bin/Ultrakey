//! F-10 로그인 시 자동 실행("Launch on login") — `menu-bar-and-lifecycle.md` §3.5,
//! §5 항목 3·4, §8. `docs/dev/architecture.md` §6.7 이 이 크레이트가 채택할 경로를
//! 이미 확정했다: **원본의 헬퍼 앱(`SuperkeyLauncher.app`) + `SMLoginItemSetEnabled`
//! 패턴은 기각한다** — 헬퍼 번들을 하나 더 서명·배포·핑퐁 관리해야 하는데 얻는 것이
//! 없다. 대신:
//!
//! - **macOS 13+**: `SMAppService.mainAppService`(메인 앱 자신을 로그인 항목으로
//!   등록 — 헬퍼 불필요).
//! - **macOS 12 폴백**: `~/Library/LaunchAgents/<bundle-id>.plist` 를 직접 쓰고/지운다.
//!
//! ⭐ **`SMAppService` 접근 방법 — 새 크레이트를 추가하지 않는다(위임 지시).**
//! `smappservice-rs`/`objc2-service-management` 대신, 이미 워크스페이스에 있는 `objc2`
//! 의 런타임 클래스 조회(`AnyClass::get`)와 `msg_send!` 로 직접 호출한다. 이 선택의
//! 부수 효과가 §8 수용 기준("macOS 12 에서 `SMAppService` API 를 호출하지 않는다")을
//! **구조적으로** 보장한다는 점이 중요하다 — `AnyClass::get(c"SMAppService")` 가
//! macOS 12 에서는 클래스 자체가 없어 `None` 을 돌려주므로, 분기 로직이 "OS 버전을
//! 읽어서 if 문으로 나눈다" 형태가 아니라 "클래스가 없으면 애초에 그 경로로 들어갈
//! 수 없다"는 형태가 된다 — OS 버전 판정 코드가 틀려도 이 보장은 깨지지 않는다.
//!
//! ⭐ **`ServiceManagement.framework` 를 명시적으로 링크한다.** 클래스 런타임 조회
//! (`objc_getClass`)는 그 프레임워크가 **프로세스에 이미 로드되어 있어야만** 성공한다
//! — dyld 는 Mach-O 로드 커맨드(`LC_LOAD_DYLIB`)에 없는 프레임워크를 자동으로
//! 로드하지 않는다. `#[link(...)]` 없이 클래스 조회만 하면, macOS 13+ 에서도 아무도
//! 이 프레임워크를 미리 로드해 두지 않았다면 조회가 항상 실패해 **13+ 에서도 매번
//! macOS 12 폴백으로 떨어지는** 잘못된 결과가 나온다. 링크해 두면 프로세스 시작
//! 시점에 dyld 가 프레임워크를 로드하고, 그 안의 Objective-C 클래스가 런타임에
//! 등록되어 `AnyClass::get` 이 찾을 수 있다.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// 등록 실패 사유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginItemError {
    /// 이 플랫폼/상태에서는 판정 자체가 불가능하다(HOME·번들 ID·실행 파일 경로를
    /// 얻지 못함, 또는 macOS 가 아닌 타깃).
    Unsupported,
    /// 등록/해제 API(또는 plist 쓰기/지우기)가 실패했다 — 사용자에게 보여줄 수 있는
    /// 진단 메시지를 담는다.
    RegisterFailed(String),
}

impl std::fmt::Display for LoginItemError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoginItemError::Unsupported => write!(f, "이 환경에서는 로그인 항목 등록을 지원하지 않는다"),
            LoginItemError::RegisterFailed(reason) => write!(f, "로그인 항목 등록/해제 실패: {reason}"),
        }
    }
}

impl std::error::Error for LoginItemError {}

/// §5 항목 3 — 등록 실패 시 상한 있는 재시도. 원본은 0.1초 간격이었지만(실측 문자열
/// "Retrying in 0.1s"), 클론은 architecture.md §6.7 이 확정한 값(0.2초 간격 5회)을
/// 쓴다 — 원본을 그대로 재현할 이유가 없다는 판단은 F-10 명세 §7 이 이미 열어 뒀다.
const MAX_ATTEMPTS: u32 = 5;
const RETRY_INTERVAL: Duration = Duration::from_millis(200);

// ============================================================================
// 순수 로직 — plist 내용 조립·경로 계산. macOS 여부와 무관하게 항상 컴파일·테스트된다.
// ============================================================================

/// `~/Library/LaunchAgents` 디렉터리 경로.
fn launch_agents_dir(home: &Path) -> PathBuf {
    home.join("Library").join("LaunchAgents")
}

/// 번들 ID 에 대응하는 LaunchAgent plist 경로 — `<home>/Library/LaunchAgents/<bundle-id>.plist`.
fn launch_agent_plist_path(home: &Path, bundle_id: &str) -> PathBuf {
    launch_agents_dir(home).join(format!("{bundle_id}.plist"))
}

/// LaunchAgent plist 의 최소 구성(§3.5: `Label`·`ProgramArguments`·`RunAtLoad`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct LaunchAgentPlist {
    #[serde(rename = "Label")]
    label: String,
    #[serde(rename = "ProgramArguments")]
    program_arguments: Vec<String>,
    #[serde(rename = "RunAtLoad")]
    run_at_load: bool,
}

/// 순수 조립 — I/O 없음. `bundle_id` 를 `Label` 로, `exe_path` 하나만 `ProgramArguments`
/// 로 쓴다(인자 없이 그대로 실행). `RunAtLoad` 는 항상 `true` — "로그인 시 자동 실행"
/// 이라는 기능 자체가 이 값을 요구한다.
fn build_launch_agent_plist(bundle_id: &str, exe_path: &Path) -> LaunchAgentPlist {
    LaunchAgentPlist {
        label: bundle_id.to_string(),
        program_arguments: vec![exe_path.display().to_string()],
        run_at_load: true,
    }
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::{
        build_launch_agent_plist, launch_agent_plist_path, LoginItemError, MAX_ATTEMPTS,
        RETRY_INTERVAL,
    };
    use objc2::msg_send;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject};
    use objc2_foundation::NSError;
    use std::path::PathBuf;

    // 근거: `SMAppService.h`(SDK 실측, `ServiceManagement.framework/Versions/A/Headers/
    // SMAppService.h`) — 클래스 자체가 이 프레임워크 안에 있다. 위 모듈 문서의 이유로
    // 명시 링크한다. 이 extern 블록은 선언할 C 심볼이 없다(전부 Objective-C 메서드
    // 호출이라 `msg_send!` 로 하기 때문) — 링크 디렉티브만을 위한 빈 블록이다.
    #[link(name = "ServiceManagement", kind = "framework")]
    extern "C" {}

    fn sm_app_service_class() -> Option<&'static AnyClass> {
        AnyClass::get(c"SMAppService")
    }

    /// `SMAppServiceStatus`(헤더 실측): `NotRegistered=0` · `Enabled=1` ·
    /// `RequiresApproval=2` · `NotFound=3`.
    const SM_APP_SERVICE_STATUS_ENABLED: isize = 1;

    /// `SMAppService.mainAppService`(class 프로퍼티). ⚠️ 헤더의 `NS_SWIFT_NAME(mainApp)`
    /// 는 **Swift 쪽 이름**일 뿐, Objective-C 선택자(그리고 이 프로퍼티의 실제 이름)는
    /// `mainAppService` 다(SDK 헤더 원문: `@property (class, readonly) SMAppService
    /// *mainAppService NS_SWIFT_NAME(mainApp)`). `mainApp` 을 선택자로 그대로 쓰면
    /// `doesNotRecognizeSelector:` 로 죽는다.
    fn main_app_service() -> Option<Retained<AnyObject>> {
        let cls = sm_app_service_class()?;
        // SAFETY: `mainAppService` 는 인자 없는 class 프로퍼티 getter다(헤더 실측
        // 시그니처와 일치). objc2 의 `msg_send!` 는 반환 타입(`Retained<AnyObject>`)을
        // 보고 ARC 규약대로 autorelease 된 반환값을 스스로 리테인한다 — 이 호출
        // 자체는 부작용이 없는 조회다.
        let obj: Retained<AnyObject> = unsafe { msg_send![cls, mainAppService] };
        Some(obj)
    }

    pub fn is_enabled() -> bool {
        if let Some(service) = main_app_service() {
            // SAFETY: `status` 는 인자 없는 인스턴스 프로퍼티 getter다(헤더 실측).
            let status: isize = unsafe { msg_send![&service, status] };
            return status == SM_APP_SERVICE_STATUS_ENABLED;
        }
        legacy_is_enabled()
    }

    pub fn set_enabled(on: bool) -> Result<(), LoginItemError> {
        if main_app_service().is_some() {
            with_retry(|| set_enabled_smappservice(on))
        } else {
            with_retry(|| legacy_set_enabled(on))
        }
    }

    /// §5 항목 3 — 상한 있는 재시도(0.2초 간격, 최대 5회). 등록/해제 자체가 아니라
    /// 이 재시도 루프가 "메인 스레드를 막지 않아야 한다"는 요구는 **호출부 책임**이다
    /// — 이 함수는 그 계약대로 동기·블로킹이다(모듈 문서·`set_enabled` 시그니처 참고).
    /// 호출하는 쪽(앱 계층)이 별도 스레드에서 불러야 한다.
    fn with_retry(
        mut op: impl FnMut() -> Result<(), LoginItemError>,
    ) -> Result<(), LoginItemError> {
        let mut last_err = LoginItemError::Unsupported;
        for attempt in 1..=MAX_ATTEMPTS {
            match op() {
                Ok(()) => return Ok(()),
                Err(e) => {
                    tracing::warn!(
                        attempt,
                        max = MAX_ATTEMPTS,
                        error = %e,
                        "로그인 항목 등록/해제 실패 — 재시도한다"
                    );
                    last_err = e;
                    if attempt < MAX_ATTEMPTS {
                        std::thread::sleep(RETRY_INTERVAL);
                    }
                }
            }
        }
        tracing::error!(
            max = MAX_ATTEMPTS,
            "로그인 항목 등록/해제가 상한만큼 반복 실패했다 — 포기한다(무한 재시도 금지, §5 항목 3)"
        );
        Err(last_err)
    }

    fn set_enabled_smappservice(on: bool) -> Result<(), LoginItemError> {
        let service = main_app_service().ok_or(LoginItemError::Unsupported)?;
        // SAFETY: `registerAndReturnError:`/`unregisterAndReturnError:` 는 헤더 실측
        // 시그니처(`- (BOOL)…AndReturnError:(NSError **)error`)와 일치한다. `msg_send!`
        // 의 `sel: _` 오류 변형이 `NSError **` out-parameter 관례를 대신 처리해
        // `Result<(), Retained<NSError>>` 로 돌려준다(objc2 문서의 표준 관례).
        let result: Result<(), Retained<NSError>> = if on {
            unsafe { msg_send![&service, registerAndReturnError: _] }
        } else {
            unsafe { msg_send![&service, unregisterAndReturnError: _] }
        };
        result.map_err(|e| LoginItemError::RegisterFailed(e.to_string()))
    }

    fn home_dir() -> Option<PathBuf> {
        std::env::var_os("HOME").map(PathBuf::from)
    }

    /// macOS 12 폴백 계산에 필요한 세 가지(plist 경로 · 번들 ID · 실행 파일 경로)를
    /// 한 번에 얻는다. 셋 중 하나라도 없으면 판정 자체가 불가능하다.
    fn legacy_context() -> Option<(PathBuf, String, PathBuf)> {
        let home = home_dir()?;
        let bundle_id = crate::bundle::bundle_identifier()?;
        let exe = std::env::current_exe().ok()?;
        let path = launch_agent_plist_path(&home, &bundle_id);
        Some((path, bundle_id, exe))
    }

    fn legacy_is_enabled() -> bool {
        match legacy_context() {
            Some((path, _, _)) => path.exists(),
            None => false,
        }
    }

    fn legacy_set_enabled(on: bool) -> Result<(), LoginItemError> {
        let (path, bundle_id, exe) = legacy_context().ok_or(LoginItemError::Unsupported)?;
        if on {
            let plist = build_launch_agent_plist(&bundle_id, &exe);
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| LoginItemError::RegisterFailed(e.to_string()))?;
            }
            plist::to_file_xml(&path, &plist)
                .map_err(|e| LoginItemError::RegisterFailed(e.to_string()))
        } else {
            match std::fs::remove_file(&path) {
                Ok(()) => Ok(()),
                // 이미 없으면(중복 해제 요청 등) 목표 상태에 이미 도달한 것 — 성공 취급.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(LoginItemError::RegisterFailed(e.to_string())),
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{is_enabled, set_enabled};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::LoginItemError;

    pub fn is_enabled() -> bool {
        false
    }

    pub fn set_enabled(_on: bool) -> Result<(), LoginItemError> {
        Err(LoginItemError::Unsupported)
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{is_enabled, set_enabled};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launch_agents_dir_is_under_library() {
        let home = Path::new("/Users/hongildong");
        assert_eq!(
            launch_agents_dir(home),
            PathBuf::from("/Users/hongildong/Library/LaunchAgents")
        );
    }

    #[test]
    fn launch_agent_plist_path_uses_bundle_id_as_filename() {
        let home = Path::new("/Users/hongildong");
        assert_eq!(
            launch_agent_plist_path(home, "app.ultrakey.Ultrakey"),
            PathBuf::from("/Users/hongildong/Library/LaunchAgents/app.ultrakey.Ultrakey.plist")
        );
    }

    #[test]
    fn build_launch_agent_plist_sets_label_run_at_load_and_program_arguments() {
        let plist = build_launch_agent_plist(
            "app.ultrakey.Ultrakey",
            Path::new("/Applications/Ultrakey.app/Contents/MacOS/Ultrakey"),
        );
        assert_eq!(plist.label, "app.ultrakey.Ultrakey");
        assert!(plist.run_at_load);
        assert_eq!(
            plist.program_arguments,
            vec!["/Applications/Ultrakey.app/Contents/MacOS/Ultrakey".to_string()]
        );
    }

    /// `plist` 크레이트로 실제 XML 라운드트립까지 확인한다 — 순수 Rust 직렬화라
    /// macOS 가 아닌 환경에서도 이 테스트는 그대로 돈다.
    #[test]
    fn build_launch_agent_plist_round_trips_through_xml() {
        let original = build_launch_agent_plist("app.ultrakey.Ultrakey", Path::new("/bin/ultrakey"));

        let mut buf = Vec::new();
        plist::to_writer_xml(&mut buf, &original).expect("plist XML 직렬화 실패");
        let xml = String::from_utf8(buf).unwrap();
        assert!(xml.contains("<key>Label</key>"));
        assert!(xml.contains("app.ultrakey.Ultrakey"));
        assert!(xml.contains("<key>RunAtLoad</key>"));

        let decoded: LaunchAgentPlist = plist::from_bytes(xml.as_bytes()).expect("plist XML 파싱 실패");
        assert_eq!(decoded, original);
    }

    #[test]
    fn login_item_error_display_is_human_readable() {
        let err = LoginItemError::RegisterFailed("디스크가 가득 찼다".to_string());
        assert!(err.to_string().contains("디스크가 가득 찼다"));
        assert_eq!(
            LoginItemError::Unsupported.to_string(),
            "이 환경에서는 로그인 항목 등록을 지원하지 않는다"
        );
    }

    /// macOS 가 아닌 타깃(이 테스트가 도는 CI 를 포함)에서는 항상 `Unsupported` 로
    /// 격하되고, `is_enabled()` 는 항상 `false` 다 — non-macOS 스텁 계약.
    #[cfg(not(target_os = "macos"))]
    #[test]
    fn stub_is_always_unsupported() {
        assert!(!is_enabled());
        assert_eq!(set_enabled(true), Err(LoginItemError::Unsupported));
    }
}
