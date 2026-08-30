//! ⭐ F-10 앱별 비활성화 게이트 — `key-remapping-engine.md` §3-f, `docs/dev/architecture.md` §2.3.
//! + ⭐ F-16 한국어 전용 앱 제외 게이트(`docs/spec/korean-input.md` §3.5, D-K3).
//!
//! 이 모듈은 의도적으로 **비대칭**이다: 콜백(탭 스레드)이 읽는 [`AtomicAppGate`] 는 원자값
//! 하나만 갖고 락이 전혀 없다(O(1)·무할당). 반대로 이를 갱신하는 [`AppGateController`] 는
//! 메인 스레드에서만 호출되므로 `Mutex` 를 써도 안전하다 — 콜백은 이 `Mutex` 를 절대 만지지
//! 않으므로 경합 자체가 없다(`docs/dev/architecture.md` §2.2 표의 "앱별 비활성화 게이트" 행).
//!
//! `menu-bar-and-lifecycle.md` 가 `disabledApps`/`enabledApps` 두 저장 모드 가능성을
//! `(미확정)` 으로 남겼으므로, 이 공개 API 는 모드 개념을 노출하지 않는다 — 지금은
//! **블랙리스트(`disabledApps`)로만** 구현한다. 모드가 확정되면 [`AppGateController`] 내부
//! 구현만 바뀌고 이 트레이트/구조체의 시그니처는 그대로다.
//!
//! ## ⭐ 왜 게이트가 두 개인가(F-10 전역 vs F-16 한국어 전용)
//!
//! `docs/spec/korean-input.md` §3.5 가 정한 것이다: **F-10 전역 게이트는 hyper 까지
//! 죽인다.** `is_remapping_disabled()` 는 `docs/dev/architecture.md` §2.3 의 `0-c`에서
//! 계층 1~5 전부를 통과시키므로, 원격 데스크톱 앱 안에서 hyper 를 계속 쓰고 싶은
//! 사용자는 F-10 목록에 그 앱을 넣을 수 없다. 반면 F-16 이 원하는 것은 "그 앱에서
//! 한국어 키 처리만 끄고 hyper 는 계속 쓴다"이다 — 그래서 `korean_disabled` 를
//! **별도의 비트**로 둔다. 콜백은 두 부울을 각자 다른 자리에서 읽는다: `disabled` 는
//! `0-c`(계층 0, 모든 것을 막음), `korean_disabled` 는 계층 3 안의 F-16 규칙 평가
//! 직전(`arbitration.rs::evaluate_korean_rules` 진입 조건)에서만 참조된다.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// 최전면 앱 식별자.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppIdentity {
    pub bundle_id: String,
    pub name: String,
}

/// F-07 이 매 이벤트마다 읽는 쪽. **반드시 O(1)·무할당**이어야 한다
/// (`key-remapping-engine.md` §3-b 계층 1의 "전역 플래그 1개로 O(1) 확인" 요구와 동일한 이유).
pub trait AppGate: Send + Sync {
    fn is_remapping_disabled(&self) -> bool;
    /// ⭐ D-K3 — F-16 한국어 전용 앱 제외(`docs/spec/korean-input.md` §3.5). `true` 면
    /// F-16 규칙을 평가하지 않는다. `is_remapping_disabled()` 와 달리 hyper 등 다른
    /// 계층에는 영향을 주지 않는다.
    fn is_korean_disabled(&self) -> bool;
}

/// `AtomicBool` 두 개로 구현된 게이트. 판정 자체는 [`AppGateController`] 가 미리 계산해
/// 여기에 게시(publish)한다 — 콜백은 이 값들을 읽기만 한다.
pub struct AtomicAppGate {
    disabled: AtomicBool,
    /// ⭐ D-K3. `disabled` 와 별도 비트인 이유는 모듈 문서 참고.
    korean_disabled: AtomicBool,
}

impl AtomicAppGate {
    pub fn new() -> Self {
        AtomicAppGate {
            disabled: AtomicBool::new(false),
            korean_disabled: AtomicBool::new(false),
        }
    }
}

impl Default for AtomicAppGate {
    fn default() -> Self {
        Self::new()
    }
}

impl AppGate for AtomicAppGate {
    fn is_remapping_disabled(&self) -> bool {
        self.disabled.load(Ordering::Acquire)
    }

    fn is_korean_disabled(&self) -> bool {
        self.korean_disabled.load(Ordering::Acquire)
    }
}

/// 메인 스레드가 소유하는 가변 상태. 최전면 앱과 비활성화 목록을 여기서만 바꾼다.
struct ControllerState {
    front_app: Option<AppIdentity>,
    /// ⭐ 블랙리스트만 구현한다(위 모듈 문서 참고) — `disabledApps` 저장 형식에 대응.
    disabled_apps: Vec<String>,
    /// ⭐ D-K3 — 명세 §4.2 항목 5 기본값 `true`. `AppGateController::new` 가 설정한다.
    korean_exclusion_enabled: bool,
    /// ⭐ D-K3 — 목록 자체는 `ultrakey-korean` 크레이트가 소유하고 앱이 주입한다.
    /// `ultrakey-core` 는 정책을 모른다 — 여기 하드코딩하지 않는다.
    korean_excluded_bundle_ids: Vec<String>,
}

/// F-10 이 갱신하는 쪽. 메인 스레드에서만 호출된다(메뉴바 UI 는 M3).
pub struct AppGateController {
    gate: Arc<AtomicAppGate>,
    state: Mutex<ControllerState>,
}

impl AppGateController {
    pub fn new(gate: Arc<AtomicAppGate>) -> Self {
        AppGateController {
            gate,
            state: Mutex::new(ControllerState {
                front_app: None,
                disabled_apps: Vec::new(),
                // ⭐ D-K3 — 명세 §4.2 항목 5 기본값. "원격 데스크톱 클라이언트에서
                // 한국어 키 처리 끄기"는 F-16 항목 중 유일한 기본 켜짐이다.
                korean_exclusion_enabled: true,
                korean_excluded_bundle_ids: Vec::new(),
            }),
        }
    }

    /// 현재 판정 결과를 재계산해 `AtomicAppGate` 에 게시한다. 락을 쥔 채로만 호출한다.
    fn republish(&self, state: &ControllerState) {
        let disabled = match &state.front_app {
            Some(app) => state.disabled_apps.iter().any(|id| id == &app.bundle_id),
            None => false,
        };
        self.gate.disabled.store(disabled, Ordering::Release);

        // ⭐ D-K3 — korean_disabled = korean_exclusion_enabled && front_app.bundle_id
        // ∈ korean_excluded_bundle_ids.
        let korean_disabled = state.korean_exclusion_enabled
            && match &state.front_app {
                Some(app) => state
                    .korean_excluded_bundle_ids
                    .iter()
                    .any(|id| id == &app.bundle_id),
                None => false,
            };
        self.gate.korean_disabled.store(korean_disabled, Ordering::Release);
    }

    /// `NSWorkspaceDidActivateApplicationNotification` 수신 시 호출(§3-f).
    pub fn set_front_app(&self, ident: Option<AppIdentity>) {
        let mut state = self.state.lock().expect("AppGateController 뮤텍스가 오염되었다");
        state.front_app = ident;
        self.republish(&state);
    }

    /// 설정 변경(비활성화 목록 전체 교체) 시 호출.
    pub fn set_disabled_apps(&self, bundle_ids: Vec<String>) {
        let mut state = self.state.lock().expect("AppGateController 뮤텍스가 오염되었다");
        state.disabled_apps = bundle_ids;
        self.republish(&state);
    }

    /// ⭐ D-K3 — "원격 데스크톱 클라이언트에서 한국어 키 처리 끄기" 체크박스(명세 §4.2
    /// 항목 5). 기본값은 `AppGateController::new` 가 이미 `true` 로 채운다.
    pub fn set_korean_exclusion_enabled(&self, enabled: bool) {
        let mut state = self.state.lock().expect("AppGateController 뮤텍스가 오염되었다");
        state.korean_exclusion_enabled = enabled;
        self.republish(&state);
    }

    /// ⭐ D-K3 — 한국어 전용 앱 제외 목록 전체 교체. 목록 자체는 `ultrakey-korean` 이
    /// 소유한다(`default_excluded_bundle_ids()`) — 이 크레이트는 정책을 모른다.
    pub fn set_korean_excluded_apps(&self, bundle_ids: Vec<String>) {
        let mut state = self.state.lock().expect("AppGateController 뮤텍스가 오염되었다");
        state.korean_excluded_bundle_ids = bundle_ids;
        self.republish(&state);
    }

    /// 메뉴바 `Ignore <최전면앱>` (M3 UI 가 호출) — 현재 최전면 앱을 목록에 넣거나 뺀다.
    /// 반환값은 "토글 후 그 앱이 비활성화 상태인가".
    pub fn toggle_front_app(&self) -> bool {
        let mut state = self.state.lock().expect("AppGateController 뮤텍스가 오염되었다");
        let Some(bundle_id) = state.front_app.as_ref().map(|a| a.bundle_id.clone()) else {
            // 최전면 앱 정보가 없으면 토글할 대상이 없다 — 조용히 무시.
            return false;
        };
        if let Some(pos) = state.disabled_apps.iter().position(|id| *id == bundle_id) {
            state.disabled_apps.remove(pos);
        } else {
            state.disabled_apps.push(bundle_id);
        }
        self.republish(&state);
        self.gate.is_remapping_disabled()
    }

    pub fn front_app(&self) -> Option<AppIdentity> {
        self.state
            .lock()
            .expect("AppGateController 뮤텍스가 오염되었다")
            .front_app
            .clone()
    }

    pub fn disabled_apps(&self) -> Vec<String> {
        self.state
            .lock()
            .expect("AppGateController 뮤텍스가 오염되었다")
            .disabled_apps
            .clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(bundle_id: &str) -> AppIdentity {
        AppIdentity {
            bundle_id: bundle_id.to_string(),
            name: bundle_id.to_string(),
        }
    }

    #[test]
    fn disabled_when_front_app_in_list() {
        let gate = Arc::new(AtomicAppGate::new());
        let ctrl = AppGateController::new(gate.clone());

        ctrl.set_disabled_apps(vec!["com.example.Ghostty".to_string()]);
        assert!(!gate.is_remapping_disabled());

        ctrl.set_front_app(Some(app("com.example.Ghostty")));
        assert!(gate.is_remapping_disabled());

        ctrl.set_front_app(Some(app("com.example.Other")));
        assert!(!gate.is_remapping_disabled());
    }

    #[test]
    fn toggle_front_app_adds_and_removes() {
        let gate = Arc::new(AtomicAppGate::new());
        let ctrl = AppGateController::new(gate.clone());
        ctrl.set_front_app(Some(app("com.example.Ghostty")));

        let now_disabled = ctrl.toggle_front_app();
        assert!(now_disabled);
        assert!(gate.is_remapping_disabled());
        assert_eq!(ctrl.disabled_apps(), vec!["com.example.Ghostty".to_string()]);

        let now_disabled2 = ctrl.toggle_front_app();
        assert!(!now_disabled2);
        assert!(!gate.is_remapping_disabled());
    }

    #[test]
    fn toggle_without_front_app_is_noop() {
        let gate = Arc::new(AtomicAppGate::new());
        let ctrl = AppGateController::new(gate.clone());
        assert!(!ctrl.toggle_front_app());
        assert!(ctrl.disabled_apps().is_empty());
    }

    #[test]
    fn no_front_app_is_never_disabled() {
        let gate = Arc::new(AtomicAppGate::new());
        let ctrl = AppGateController::new(gate.clone());
        ctrl.set_disabled_apps(vec!["com.example.A".to_string()]);
        ctrl.set_front_app(None);
        assert!(!gate.is_remapping_disabled());
    }

    // ── D-K3 — 한국어 전용 앱 제외 게이트 ────────────────────────────────────────

    /// 기본값이 `true` 다(명세 §4.2 항목 5) — 새 컨트롤러는 목록을 채우기만 해도
    /// `set_korean_exclusion_enabled` 를 호출하지 않고서도 즉시 게이트가 동작해야 한다.
    #[test]
    fn korean_exclusion_enabled_by_default() {
        let gate = Arc::new(AtomicAppGate::new());
        let ctrl = AppGateController::new(gate.clone());

        ctrl.set_korean_excluded_apps(vec!["com.apple.ScreenSharing".to_string()]);
        ctrl.set_front_app(Some(app("com.apple.ScreenSharing")));

        assert!(gate.is_korean_disabled());
        // ⭐ F-10 전역 게이트와 구분되는 지점(명세 §8) — hyper 등 다른 계층은 이 비트의
        // 영향을 받지 않는다. `AtomicAppGate` 수준에서는 이것이 "두 비트가 독립적으로
        // 계산된다"는 사실로 나타난다.
        assert!(!gate.is_remapping_disabled());
    }

    /// 항목 5 를 끄면 그 앱에서도 F-16 규칙이 정상 평가된다(명세 §8 수용 기준).
    #[test]
    fn korean_exclusion_disabled_clears_gate_even_for_listed_app() {
        let gate = Arc::new(AtomicAppGate::new());
        let ctrl = AppGateController::new(gate.clone());

        ctrl.set_korean_excluded_apps(vec!["com.apple.ScreenSharing".to_string()]);
        ctrl.set_front_app(Some(app("com.apple.ScreenSharing")));
        assert!(gate.is_korean_disabled());

        ctrl.set_korean_exclusion_enabled(false);
        assert!(!gate.is_korean_disabled());
    }

    /// 목록에 없는 앱은 항목 5 가 켜져 있어도 영향받지 않는다.
    #[test]
    fn korean_gate_is_false_for_apps_not_in_the_excluded_list() {
        let gate = Arc::new(AtomicAppGate::new());
        let ctrl = AppGateController::new(gate.clone());

        ctrl.set_korean_excluded_apps(vec!["com.apple.ScreenSharing".to_string()]);
        ctrl.set_front_app(Some(app("com.example.Other")));

        assert!(!gate.is_korean_disabled());
    }

    /// F-10 전역 게이트와 F-16 한국어 게이트는 **독립적으로** 계산된다 — F-10 목록에
    /// 없는 앱이 F-16 목록에는 있을 수 있고, 그 반대도 가능하다.
    #[test]
    fn global_gate_and_korean_gate_are_independent() {
        let gate = Arc::new(AtomicAppGate::new());
        let ctrl = AppGateController::new(gate.clone());

        ctrl.set_disabled_apps(vec!["com.example.Ghostty".to_string()]);
        ctrl.set_korean_excluded_apps(vec!["com.apple.ScreenSharing".to_string()]);

        ctrl.set_front_app(Some(app("com.apple.ScreenSharing")));
        assert!(gate.is_korean_disabled());
        assert!(!gate.is_remapping_disabled());

        ctrl.set_front_app(Some(app("com.example.Ghostty")));
        assert!(gate.is_remapping_disabled());
        assert!(!gate.is_korean_disabled());
    }
}
