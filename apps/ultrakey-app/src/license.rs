//! F-12 — 라이선스 배선(`licensing-and-trial.md` §7 결정: Paddle no-op).
//!
//! 순수 로직(`ultrakey-license`)과 FFI 어댑터(`ultrakey-platform::keychain`,
//! `ultrakey-platform::device_id`)를 조립해 상태 판정을 수행하고, General 탭 UI
//! (`settings.html`)에 라이선스 상태를 노출한다.
//!
//! ⭐ 제품 결정 D2 — 실제 Paddle 은 붙이지 않는다. `NoopLicenseProvider`(항상
//! 라이선스 활성)가 기본이다. 상태 머신·오프라인 유예·비활성화 UI 는 전부 이
//! provider 위에서 동작한다. 실제 연동 시 이 모듈의 provider 주입 지점 하나만
//! 바꾼다.

use std::sync::{Arc, Mutex};

use ultrakey_license::{
    ActivationOutcome, CacheStore, DeactivationOutcome, EvaluateResult, LicenseMachine,
    NoopLicenseProvider, SystemClock, TrialStore,
};
use ultrakey_platform::device_id;
use ultrakey_platform::keychain::KeychainStore;

/// F-12 실행 시점 상태 판정·활성화·비활성화를 앱 루프에 노출하는 컨트롤러.
///
/// `store`(Keychain)는 [`LicenseMachine`] 에 `TrialStore`/`CacheStore` 로 주입되고,
/// 동시에 이 컨트롤러가 비활성화 대상 키를 읽는 데 쓴다(캐시된 키를 다시 받아
/// `deactivate` 에 넘긴다).
pub struct LicenseController {
    machine: Mutex<LicenseMachine>,
    store: Arc<KeychainStore>,
    /// 현재 device_id(IOPlatformUUID). 읽기 실패 시 `None` — no-op 은 ID 없이도
    /// 항상 활성화로 귀결된다.
    device_id: Option<String>,
}

impl LicenseController {
    /// 부팅 시 조립. Keychain 어댑터·no-op provider 를 넣는다.
    pub fn boot() -> Arc<Self> {
        let store = Arc::new(KeychainStore::new());
        let provider: Arc<dyn ultrakey_license::LicenseProvider> =
            Arc::new(NoopLicenseProvider::new());
        let machine = LicenseMachine::new(
            Arc::new(SystemClock),
            store.clone() as Arc<dyn TrialStore + Send + Sync>,
            store.clone() as Arc<dyn CacheStore + Send + Sync>,
            provider,
        );
        Arc::new(Self {
            machine: Mutex::new(machine),
            store,
            device_id: device_id::hardware_device_id(),
        })
    }

    /// 현재 라이선스 상태를 판정해 반환한다(General 탭 UI 가 그린다).
    pub fn evaluate(&self) -> EvaluateResult {
        let res = self.machine.lock().unwrap().evaluate_on_start();
        // 판정 상태를 로그에 남긴다(로그는 항상 영어 — 명세 §3.1).
        match res.trial.and_then(|t| t.days_remaining) {
            Some(days) => {
                tracing::debug!(
                    state = res.state.as_log_str(),
                    days_remaining = days,
                    "license state evaluated"
                );
            }
            None => {
                tracing::debug!(state = res.state.as_log_str(), "license state evaluated");
            }
        }
        res
    }

    /// 사용자가 키를 입력하고 "활성화"를 눌렀을 때(§3.3.1).
    pub fn activate_key(&self, license_key: String) -> ActivationOutcome {
        let machine = self.machine.lock().unwrap();
        let device_id = self.device_id.clone().unwrap_or_default();
        let outcome = machine.activate(license_key, device_id);
        tracing::info!(
            outcome = %activation_log(&outcome),
            "license activation attempted"
        );
        outcome
    }

    /// "이 기기 비활성화"(§3.3.2). 캐시된 키를 읽어 `deactivate` 에 넘긴다.
    /// 캐시가 없으면 `NotFound`. ⚠️ 네트워크 오류면 낙관적으로 풀지 않는다(§5-8).
    pub fn deactivate_device(&self) -> DeactivationOutcome {
        let machine = self.machine.lock().unwrap();
        let Some(cached) = self.store.read_cache() else {
            tracing::warn!("deactivation requested but no cached license key present");
            return DeactivationOutcome::NotFound;
        };
        let device_id = self.device_id.clone().unwrap_or_default();
        let outcome = machine.deactivate(cached.license_key, device_id);
        tracing::info!(
            outcome = %deactivation_log(&outcome),
            "license deactivation attempted"
        );
        outcome
    }
}

fn activation_log(o: &ActivationOutcome) -> &'static str {
    match o {
        ActivationOutcome::Activated { .. } => "activated",
        ActivationOutcome::InvalidKey => "invalid_key",
        ActivationOutcome::Refunded => "refunded",
        ActivationOutcome::LimitReached { .. } => "limit_reached",
        ActivationOutcome::NetworkError => "network_error",
    }
}

fn deactivation_log(o: &DeactivationOutcome) -> &'static str {
    match o {
        DeactivationOutcome::Deactivated { .. } => "deactivated",
        DeactivationOutcome::NotFound => "not_found",
        DeactivationOutcome::NetworkError => "network_error",
    }
}
