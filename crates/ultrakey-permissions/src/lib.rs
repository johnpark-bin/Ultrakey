//! `ultrakey-permissions` — F-11 권한 온보딩과 복구(`docs/spec/permissions-onboarding.md`).
//!
//! ⭐ §1.1 의 실측이 이 크레이트 전체의 전제다: 앱이 **명시적으로 확인·요청하는 TCC
//! 권한은 Accessibility(`AXIsProcessTrusted`) 하나뿐**이다. Input Monitoring·Screen
//! Recording 은 M1 범위에 없다(§7 "M1 이 실제로 구현한 범위" 표) — `ultrakey-engine` 이
//! 핫플러그를 `IOServiceAddMatchingNotification` 으로 구현해 `IOHIDManagerOpen` 자체를
//! 호출하지 않고(`key-remapping-engine.md` §6), M1 에는 Seek(F-02)가 없어
//! `CGDisplayCreateImage` 를 호출하는 코드도 없기 때문이다. 그래서 이 크레이트는
//! Screen Recording·Input Monitoring 을 확인하는 API 를 아예 갖지 않는다 — 확인할
//! 대상이 없는데 확인 코드만 미리 만들어 두면 검증되지 않은 채 방치될 뿐이다.
//!
//! ⭐ `AXIsProcessTrustedWithOptions`(시스템 프롬프트를 띄우는 변형)는 쓰지 않는다.
//! `ultrakey-platform::accessibility` 가 애초에 이 함수를 노출하지 않는다 — 원본이
//! 자체 모달로만 안내하기 때문이다(§1.1, §3.2).
//!
//! ⭐ 자체 TCC 리셋(`tccutil` 자동 호출)도 구현하지 않는다. §3.3·§5 항목 9·§7 이
//! "채택하지 않는 것을 권장"으로 판정했다 — `tccutil reset` 은 대상 서비스 전체나
//! 광범위한 항목을 초기화할 수 있어, 사용자의 명시적 확인 없이 앱이 스스로 시스템
//! 보안 데이터베이스를 건드리는 것은 위험하다는 것이 근거다. [`out_of_sync_copy`] 가
//! 안내하는 것은 (b) 수동 절차뿐이다.
//!
//! macOS 가 아닌 타깃에서도 컴파일된다 — `ultrakey-platform` 이 모든 macOS API 에
//! 대해 컴파일만 되는 스텁을 두고 있으므로([`ultrakey_platform::accessibility`],
//! [`ultrakey_platform::bundle`] 참고), 이 크레이트는 그 위에 순수 Rust 로만 쌓는다.

#![forbid(unsafe_code)]

mod copy;
mod model;
mod monitor;

pub use copy::{
    dev_build_warning, onboarding_copy, out_of_sync_copy, OnboardingCopy, OutOfSyncCopy,
};
pub use model::{PermissionModel, PermissionState, PermissionTransition, PollMode};
pub use monitor::PermissionMonitor;

/// 시스템 설정의 손쉬운 사용 패널로 이동하는 딥링크를 연다(§6).
///
/// ⚠️ 앵커 이름(`Privacy_Accessibility`)은 `rust-macos-capability-notes.md` §2.5 에서
/// 원문으로 확인된 값을 그대로 쓴다. 다만 이 URL 전체가 원본이 실제로 여는 것과
/// 정확히 같은지는 이번 조사 범위에서 문자열로 직접 확인되지 않았다 `(미확정)`
/// — §9 미해결 질문 7.
pub fn open_accessibility_settings() -> bool {
    ultrakey_platform::bundle::open_url(
        "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility",
    )
}
