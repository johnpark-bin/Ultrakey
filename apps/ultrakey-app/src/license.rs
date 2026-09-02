//! F-12 — 라이선스 배선(`licensing-and-trial.md` §7 결정: Paddle no-op).
//!
//! 순수 로직(`ultrakey-license`)과 플랫폼 어댑터(저장소 — 빌드 분기,
//! `ultrakey-platform::device_id`)를 조립해 상태 판정을 수행하고, General 탭 UI
//! (`settings.html`)에 라이선스 상태를 노출한다.
//!
//! ⭐ 제품 결정 D2 — 실제 Paddle 은 붙이지 않는다. `NoopLicenseProvider`(항상
//! 라이선스 활성)가 기본이다. 상태 머신·오프라인 유예·비활성화 UI 는 전부 이
//! provider 위에서 동작한다. 실제 연동 시 이 모듈의 provider 주입 지점 하나만
//! 바꾼다.
//!
//! ⭐ 이슈 #99 — 저장소 빌드 분기(`licensing-and-trial.md` §3.5 개정):
//! - 기본(`keychain-store` feature OFF): **파일 저장**(`FileStore`) — 개발·자체
//!   서명·미서명 빌드. 키체인 접근 자체가 없어 시스템 로그인 프롬프트가 없다.
//! - `keychain-store` feature ON: **Keychain 저장**(`KeychainStore`) — 정식 Apple
//!   서명 릴리즈 빌드(`release.yml` 이 서명 시크릿이 있을 때만 켠다).

use std::sync::{Arc, Mutex};

use ultrakey_license::{
    ActivationOutcome, CacheStore, DeactivationOutcome, EvaluateResult, LicenseMachine,
    NoopLicenseProvider, SystemClock, TrialStore,
};
use ultrakey_platform::device_id;

// 저장소 구현체 — 컴파일 타임 분기(위 모듈 독 주석·명세 §3.5 개정 참조).
#[cfg(feature = "keychain-store")]
use ultrakey_platform::keychain::KeychainStore;
#[cfg(not(feature = "keychain-store"))]
use ultrakey_license::{FileStore, InMemoryStore};

/// F-12 실행 시점 상태 판정·활성화·비활성화를 앱 루프에 노출하는 컨트롤러.
///
/// `store`(빌드 분기로 선택된 저장소)는 [`LicenseMachine`] 에 `TrialStore`/
/// `CacheStore` 로 주입되고, 동시에 이 컨트롤러가 비활성화 대상 키를 읽는 데
/// 쓴다(캐시된 키를 다시 받아 `deactivate` 에 넘긴다).
pub struct LicenseController {
    machine: Mutex<LicenseMachine>,
    store: Arc<dyn CacheStore + Send + Sync>,
    /// 현재 device_id(IOPlatformUUID). 읽기 실패 시 `None` — no-op 은 ID 없이도
    /// 항상 활성화로 귀결된다.
    device_id: Option<String>,
}

impl LicenseController {
    /// 부팅 시 조립. 빌드 분기로 선택된 저장소 어댑터·no-op provider 를 넣는다.
    pub fn boot() -> Arc<Self> {
        #[cfg(feature = "keychain-store")]
        let (trial, cache): (
            Arc<dyn TrialStore + Send + Sync>,
            Arc<dyn CacheStore + Send + Sync>,
        ) = {
            let store = Arc::new(KeychainStore::new());
            (store.clone(), store)
        };

        #[cfg(not(feature = "keychain-store"))]
        let (trial, cache): (
            Arc<dyn TrialStore + Send + Sync>,
            Arc<dyn CacheStore + Send + Sync>,
        ) = match FileStore::default_dir() {
            Some(file_store) => {
                let store = Arc::new(file_store);
                (store.clone(), store)
            }
            None => {
                // 로그는 항상 영어(명세 §3.1.6).
                tracing::error!(
                    "HOME is not set; license store falls back to in-memory (trial state resets on every launch)"
                );
                let store = Arc::new(InMemoryStore::new());
                (store.clone(), store)
            }
        };

        let provider: Arc<dyn ultrakey_license::LicenseProvider> =
            Arc::new(NoopLicenseProvider::new());
        let machine = LicenseMachine::new(
            Arc::new(SystemClock),
            trial,
            cache.clone(),
            provider,
        );
        Arc::new(Self {
            machine: Mutex::new(machine),
            store: cache,
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
