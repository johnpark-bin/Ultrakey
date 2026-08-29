//! ⭐ F-10 앱별 비활성화 게이트 — `key-remapping-engine.md` §3-f, `docs/dev/architecture.md` §2.3.
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
}

/// `AtomicBool` 하나로 구현된 게이트. 판정 자체는 [`AppGateController`] 가 미리 계산해
/// 여기에 게시(publish)한다 — 콜백은 이 값을 읽기만 한다.
pub struct AtomicAppGate {
    disabled: AtomicBool,
}

impl AtomicAppGate {
    pub fn new() -> Self {
        AtomicAppGate {
            disabled: AtomicBool::new(false),
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
}

/// 메인 스레드가 소유하는 가변 상태. 최전면 앱과 비활성화 목록을 여기서만 바꾼다.
struct ControllerState {
    front_app: Option<AppIdentity>,
    /// ⭐ 블랙리스트만 구현한다(위 모듈 문서 참고) — `disabledApps` 저장 형식에 대응.
    disabled_apps: Vec<String>,
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
}
