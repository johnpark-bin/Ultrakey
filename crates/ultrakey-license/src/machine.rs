//! 라이선스 상태 머신 — 명세 §3.2.
//!
//! 상태 전이는 **앱 실행 시점**과 **주기적 재검증 시점** 두 군데에서 평가한다
//! (§3.2). 실시간 카운트다운은 요구되지 않는다.
//!
//! ⭐ 이 머신은 `LicenseProvider` 의 정체(no-op vs 실연동)와 무관하다. provider 는
//! 트레이트 객체로 주입받고, 판정은 순수 로직(`decision.rs`)이 한다. 그래서
//! 상태 머신 단위 테스트는 provider 를 목으로 바꿔가며 전이를 검증한다.

use crate::clock::{Clock, Timestamp};
use crate::decision::{
    cache_within_grace, evaluate_trial, next_last_seen, OfflineGrace,
};
use crate::provider::{
    ActivationRequest, ActivationResponse, DeactivateRequest, DeactivateResponse,
    LicenseProvider, ValidateRequest, ValidateResponse,
};
use crate::state::LicenseState;
use crate::store::{CacheStore, LicenseCache, TrialClock, TrialRecord, TrialStore};
use std::sync::Arc;

/// 한 번의 실행에 대한 상태 평가 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluateResult {
    pub state: LicenseState,
    /// UI "체험판 — N일 남음" 표시용(명세 §4.2).
    pub trial: Option<TrialInfo>,
    /// 활성화 슬롯 현황 — 라이선스 활성·무효에서만 `Some`.
    pub activations: Option<ActivationsView>,
}

/// UI 표시용 체험 정보.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrialInfo {
    /// 남은 체험 일수 — 체험 중(`Trial`)에서만 `Some`.
    pub days_remaining: Option<i64>,
    /// 시계가 과거로 되돌려져 판정이 보수적으로 고정됐는지(§3.5).
    pub clock_rolled_back: bool,
}

/// 활성화 슬롯 현황.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ActivationsView {
    pub used: u32,
    pub limit: u32,
}

/// 상태 머신 — 시계·기산점 저장·캐시 저장·provider 를 트레이트로 주입받는다.
pub struct LicenseMachine {
    clock: Arc<dyn Clock + Send + Sync>,
    store: Arc<dyn TrialStore + Send + Sync>,
    cache: Arc<dyn CacheStore + Send + Sync>,
    provider: Arc<dyn LicenseProvider>,
}

impl LicenseMachine {
    pub fn new(
        clock: Arc<dyn Clock + Send + Sync>,
        store: Arc<dyn TrialStore + Send + Sync>,
        cache: Arc<dyn CacheStore + Send + Sync>,
        provider: Arc<dyn LicenseProvider>,
    ) -> Self {
        Self {
            clock,
            store,
            cache,
            provider,
        }
    }

    /// 앱 실행 시점에 호출 — 저장 상태 + 시계로 상태를 판정하고, 최초 실행이면
    /// 기산점을 기록한다(§8-1). 상태를 바꾸는 곳은 기산점 기록·캐시 갱신뿐이다.
    pub fn evaluate_on_start(&self) -> EvaluateResult {
        let now = self.clock.now();
        let trial = self.ensure_trial(now);

        // 라이선스 캐시가 있으면, 유예 창 안이라면 즉시 라이선스 활성(§3.2
        // "최우선"). 창을 넘겼다면(만료) 체험 판정으로 폴백한다.
        if let Some(cached) = self.cache.read_cache() {
            let grace = OfflineGrace::default();
            if cache_within_grace(cached.cache_issued_at, now, &grace) {
                return EvaluateResult {
                    state: LicenseState::Licensed,
                    trial,
                    activations: Some(ActivationsView {
                        used: cached.activations_used,
                        limit: cached.activations_limit,
                    }),
                };
            }
        }

        // 체험 판정으로 폴백.
        // ⭐ §3.1: 체험이 **소진된**(라이선스를 한 번이라도 갖다가 무효/취소된 or
        // 이미 활성화된) 기기는 체험 기간이 남아 있어도 `Trial` 로 복귀하지 않는다.
        let consumed = self.store.read().map(|r| r.trial_consumed).unwrap_or(false);
        let has_days_remaining = trial.and_then(|t| t.days_remaining).is_some();
        let state = if has_days_remaining && !consumed {
            LicenseState::Trial
        } else {
            LicenseState::TrialExpired
        };
        // 유예를 넘긴 캐시는 남아 있어도 상태에 반영하지 않는다(재검증 실패로
        // 다음 기회에 처리).
        let activations = self
            .cache
            .read_cache()
            .map(|c| {
                ActivationsView {
                    used: c.activations_used,
                    limit: c.activations_limit,
                }
            })
            .filter(|_| state == LicenseState::Licensed);

        EvaluateResult {
            state,
            trial,
            activations,
        }
    }

    /// 최초 실행이면 기산점을 기록하고, 이미 있으면 시계 조작 완화를 반영해
    /// `last_seen_at` 을 갱신한 뒤 체험 판정을 돌려준다.
    fn ensure_trial(&self, now: Timestamp) -> Option<TrialInfo> {
        match self.store.read() {
            None => {
                // 최초 실행(§3.2): 지금을 기산점으로 기록하고 체험 중으로 전이.
                let start = TrialClock {
                    trial_started_at: now,
                    last_seen_at: now,
                };
                let info = evaluate_trial(&start, now);
                self.store.write(TrialRecord {
                    clock: start,
                    trial_consumed: false,
                });
                Some(self.trial_info_from(info))
            }
            Some(prev) => {
                let info = evaluate_trial(&prev.clock, now);
                // 시계 조작 완화: 되돌아갔으면 last_seen_at 유지, 아니면 now.
                self.store.write(TrialRecord {
                    clock: TrialClock {
                        trial_started_at: prev.clock.trial_started_at,
                        last_seen_at: next_last_seen(&prev.clock, now),
                    },
                    trial_consumed: prev.trial_consumed,
                });
                Some(self.trial_info_from(info))
            }
        }
    }

    /// 체험 판정 결과 → UI 표시용 정보로 변환.
    fn trial_info_from(&self, info: crate::decision::TrialDecisionOutcome) -> TrialInfo {
        TrialInfo {
            days_remaining: if info.expired {
                None
            } else {
                info.days_remaining
            },
            clock_rolled_back: info.clock_rolled_back,
        }
    }

    /// 활성화 요청(§3.3.1) — 성공하면 로컬 캐시로 저장해 라이선스 활성으로 전이.
    pub fn activate(&self, license_key: String, device_id: String) -> ActivationOutcome {
        let request = ActivationRequest {
            license_key: license_key.clone(),
            device_id: device_id.clone(),
            device_label: None,
            app_version: None,
        };
        match self.provider.activate(request) {
            ActivationResponse::Activated {
                cache_token,
                cache_issued_at,
                activations_used,
                activations_limit,
            } => {
                let cache = LicenseCache {
                    license_key,
                    device_id,
                    cache_token,
                    cache_issued_at,
                    activations_used,
                    activations_limit,
                };
                self.cache.write_cache(&cache);
                ActivationOutcome::Activated {
                    activations_used,
                    activations_limit,
                }
            }
            ActivationResponse::InvalidKey => ActivationOutcome::InvalidKey,
            ActivationResponse::Refunded => ActivationOutcome::Refunded,
            ActivationResponse::LimitReached {
                activations_used,
                activations_limit,
            } => ActivationOutcome::LimitReached {
                activations_used,
                activations_limit,
            },
            ActivationResponse::NetworkError => ActivationOutcome::NetworkError,
        }
    }

    /// 비활성화 요청(§3.3.2). ⚠️ 성공만이 슬롯 반납의 진실이다 — 네트워크 오류면
    /// 로컬 캐시를 지우지 않는다(§5-8 "낙관적으로 풀지 않는다").
    pub fn deactivate(&self, license_key: String, device_id: String) -> DeactivationOutcome {
        let request = DeactivateRequest {
            license_key: license_key.clone(),
            device_id: device_id.clone(),
        };
        match self.provider.deactivate(request) {
            DeactivateResponse::Deactivated { activations_used } => {
                self.cache.clear_cache();
                DeactivationOutcome::Deactivated { activations_used }
            }
            DeactivateResponse::NotFound => {
                // 이미 해제됨 — 로컬 캐시도 정리한다.
                self.cache.clear_cache();
                DeactivationOutcome::NotFound
            }
            DeactivateResponse::NetworkError => DeactivationOutcome::NetworkError,
        }
    }

    /// 검증/재검증(§3.3.3). ⭐ `refunded`/`revoked`/`invalid_key` 는 유예 대상이
    /// 아니므로 로컬 캐시를 지우고 즉시 무효로 만든다(§3.4, §5-4).
    pub fn revalidate(&self) -> RevalidateOutcome {
        let Some(cached) = self.cache.read_cache() else {
            return RevalidateOutcome::NoCache;
        };
        let request = ValidateRequest {
            license_key: cached.license_key.clone(),
            device_id: cached.device_id.clone(),
            cache_token: Some(cached.cache_token.clone()),
        };
        match self.provider.validate(request) {
            ValidateResponse::Valid {
                cache_token,
                cache_issued_at,
            } => {
                // 캐시 갱신 — 유예 창 기준점을 갱신한다.
                let next = LicenseCache {
                    license_key: cached.license_key,
                    device_id: cached.device_id,
                    cache_token,
                    cache_issued_at,
                    activations_used: cached.activations_used,
                    activations_limit: cached.activations_limit,
                };
                self.cache.write_cache(&next);
                RevalidateOutcome::Valid
            }
            ValidateResponse::Refunded
            | ValidateResponse::Revoked
            | ValidateResponse::InvalidKey => {
                self.cache.clear_cache();
                self.mark_trial_consumed();
                RevalidateOutcome::Invalidated
            }
            ValidateResponse::NetworkError => RevalidateOutcome::NetworkError,
        }
    }

    /// ⭐ §3.1 — 무효/취소를 겪은 기기의 체험을 영속적으로 소진 처리한다.
    /// 그러면 체험 기간이 남아 있어도 `Trial` 로 복귀하지 않는다(§8-9).
    fn mark_trial_consumed(&self) {
        if let Some(mut rec) = self.store.read() {
            rec.trial_consumed = true;
            self.store.write(rec);
        }
    }
}

// ── 결과 타입 ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationOutcome {
    Activated { activations_used: u32, activations_limit: u32 },
    InvalidKey,
    Refunded,
    LimitReached { activations_used: u32, activations_limit: u32 },
    NetworkError,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeactivationOutcome {
    Deactivated { activations_used: u32 },
    NotFound,
    NetworkError,
}

/// 재검증 결과 — 상태 전이 정보.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RevalidateOutcome {
    /// 캐시 갱신 성공 — 라이선스 유효 지속.
    Valid,
    /// 명시적 무효(환불·취소·키 오류) — 캐시 제거, 다음 판정에서 체험 만료로.
    Invalidated,
    /// 네트워크 실패 — 유예 규칙이 결정을 이어받는다.
    NetworkError,
    /// 캐시 없음 — 재검증할 대상이 없다.
    NoCache,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Clock;
    use crate::decision::TRIAL_DAYS;
    use crate::store::InMemoryStore;
    use std::sync::Mutex;

    const T0: Timestamp = 1_000_000_000;
    const DAY: Timestamp = 86_400;

    /// 고정 시각 시계 — 테스트에서 앞/뒤로 조작한다.
    struct FixedClock(Mutex<Timestamp>);
    impl FixedClock {
        fn new(t: Timestamp) -> Arc<Self> {
            Arc::new(FixedClock(Mutex::new(t)))
        }
        fn set(&self, t: Timestamp) {
            *self.0.lock().unwrap() = t;
        }
    }
    impl Clock for FixedClock {
        fn now(&self) -> Timestamp {
            *self.0.lock().unwrap()
        }
    }

    /// 목 provider — 시나리오별 응답을 주입.
    #[derive(Clone)]
    struct MockProvider {
        activate: std::sync::Arc<Mutex<ActivationResponse>>,
        validate: std::sync::Arc<Mutex<ValidateResponse>>,
        deactivate: std::sync::Arc<Mutex<DeactivateResponse>>,
    }
    impl MockProvider {
        fn activated() -> Arc<Self> {
            Arc::new(Self {
                activate: std::sync::Arc::new(Mutex::new(ActivationResponse::Activated {
                    cache_token: "tok".into(),
                    cache_issued_at: T0,
                    activations_used: 1,
                    activations_limit: 3,
                })),
                validate: std::sync::Arc::new(Mutex::new(ValidateResponse::Valid {
                    cache_token: "tok".into(),
                    cache_issued_at: T0,
                })),
                deactivate: std::sync::Arc::new(Mutex::new(DeactivateResponse::Deactivated {
                    activations_used: 0,
                })),
            })
        }
        fn with_refunded() -> Arc<Self> {
            let p = Self::activated();
            *p.validate.lock().unwrap() = ValidateResponse::Refunded;
            p
        }
    }
    impl LicenseProvider for MockProvider {
        fn activate(&self, _: ActivationRequest) -> ActivationResponse {
            self.activate.lock().unwrap().clone()
        }
        fn deactivate(&self, _: DeactivateRequest) -> DeactivateResponse {
            self.deactivate.lock().unwrap().clone()
        }
        fn validate(&self, _: ValidateRequest) -> ValidateResponse {
            self.validate.lock().unwrap().clone()
        }
    }

    /// store 와 cache 를 같은 InMemoryStore(두 트레이트 구현)로 주입한다.
    fn machine(
        clock: Arc<FixedClock>,
        provider: Arc<dyn LicenseProvider>,
    ) -> (LicenseMachine, Arc<InMemoryStore>) {
        let store: Arc<InMemoryStore> = Arc::new(InMemoryStore::new());
        let cache = store.clone();
        // store/cache 는 같은 Arc 를 각각 트레이트로 캐스팅.
        let m = LicenseMachine::new(
            clock as Arc<dyn Clock + Send + Sync>,
            store.clone() as Arc<dyn TrialStore + Send + Sync>,
            cache as Arc<dyn CacheStore + Send + Sync>,
            provider,
        );
        (m, store)
    }

    #[test]
    fn first_run_records_epoch_and_enters_trial() {
        let clock = FixedClock::new(T0);
        let (m, store) = machine(clock.clone(), MockProvider::activated());
        let res = m.evaluate_on_start();
        assert_eq!(res.state, LicenseState::Trial);
        assert_eq!(res.trial.unwrap().days_remaining, Some(TRIAL_DAYS));
        let t = store.read().expect("최초 실행에서 기산점이 기록되어야 한다");
        assert_eq!(t.clock.trial_started_at, T0);
    }

    #[test]
    fn after_20_days_trial_expires() {
        let clock = FixedClock::new(T0);
        let (m, _) = machine(clock.clone(), MockProvider::activated());
        m.evaluate_on_start();
        clock.set(T0 + 20 * DAY);
        let res = m.evaluate_on_start();
        assert_eq!(res.state, LicenseState::TrialExpired);
        assert_eq!(res.trial.unwrap().days_remaining, None);
    }

    #[test]
    fn activation_saves_cache_and_state_is_licensed() {
        let clock = FixedClock::new(T0);
        let (m, store) = machine(clock.clone(), MockProvider::activated());
        m.evaluate_on_start();

        let out = m.activate("KEY".into(), "DEV".into());
        assert!(matches!(
            out,
            ActivationOutcome::Activated { activations_limit: 3, .. }
        ));
        assert!(store.read_cache().is_some());

        // 다음 실행 — 라이선스 활성.
        clock.set(T0 + 5 * DAY);
        let res = m.evaluate_on_start();
        assert_eq!(res.state, LicenseState::Licensed);
        assert_eq!(res.activations.unwrap().limit, 3);
    }

    #[test]
    fn within_offline_grace_keeps_licensed() {
        let clock = FixedClock::new(T0);
        let (m, _) = machine(clock.clone(), MockProvider::activated());
        m.evaluate_on_start();
        m.activate("KEY".into(), "DEV".into());

        // 10일 뒤 오프라인(재검증 없이) 재실행 — 유예(30일) 안이면 여전히 라이선스 활성.
        clock.set(T0 + 10 * DAY);
        let res = m.evaluate_on_start();
        assert_eq!(res.state, LicenseState::Licensed);
    }

    #[test]
    fn refunded_revalidation_clears_cache_and_invalidates() {
        let clock = FixedClock::new(T0);
        let (m, store) = machine(clock.clone(), MockProvider::with_refunded());
        m.evaluate_on_start();
        m.activate("KEY".into(), "DEV".into());
        assert!(store.read_cache().is_some());

        let out = m.revalidate();
        assert_eq!(out, RevalidateOutcome::Invalidated);
        assert!(store.read_cache().is_none());

        // 캐시가 사라졌으므로 다음 판정은 체험 만료로 귀결(체험 중 복귀 금지).
        let res = m.evaluate_on_start();
        assert_eq!(res.state, LicenseState::TrialExpired);
        assert_ne!(res.state, LicenseState::Trial);
    }

    #[test]
    fn trial_does_not_resume_after_invalidate() {
        // §3.1: 라이선스 무효 → 체험 만료로 재전이, 체험 중 복귀 금지.
        let clock = FixedClock::new(T0);
        let (m, _) = machine(clock.clone(), MockProvider::with_refunded());
        m.evaluate_on_start();
        m.activate("KEY".into(), "DEV".into());
        // 체험 기간 안이지만 라이선스 활성.
        clock.set(T0 + 3 * DAY);
        assert_eq!(m.evaluate_on_start().state, LicenseState::Licensed);
        // 환불 → 재검증 → 무효 → 다음 실행은 체험 만료(체험 중 아님).
        m.revalidate();
        clock.set(T0 + 4 * DAY);
        assert_eq!(m.evaluate_on_start().state, LicenseState::TrialExpired);
    }

    #[test]
    fn clock_rollback_does_not_increase_days() {
        let clock = FixedClock::new(T0);
        let (m, store) = machine(clock.clone(), MockProvider::activated());
        m.evaluate_on_start();
        // 5일 뒤 실행.
        clock.set(T0 + 5 * DAY);
        let res = m.evaluate_on_start();
        assert_eq!(res.trial.unwrap().days_remaining, Some(15));

        // 시계를 5일 전(시작 직후)으로 되돌림 → last_seen_at 은 유지.
        clock.set(T0 + 1);
        let res = m.evaluate_on_start();
        assert!(res.trial.unwrap().clock_rolled_back);
        // 남은 일수가 15일보다 늘지 않는다(이전 관찰 기준 보수 고정은
        // last_seen_at 유지로 달성된다).
        let t = store.read().unwrap();
        assert_eq!(t.clock.last_seen_at, T0 + 5 * DAY); // 되돌림 후에도 유지
        assert_eq!(res.trial.unwrap().days_remaining, Some(15));
    }

    #[test]
    fn deactivation_clears_cache_and_returns_activated_or_expired() {
        let clock = FixedClock::new(T0);
        let (m, store) = machine(clock.clone(), MockProvider::activated());
        m.evaluate_on_start();
        m.activate("KEY".into(), "DEV".into());

        let out = m.deactivate("KEY".into(), "DEV".into());
        assert_eq!(out, DeactivationOutcome::Deactivated { activations_used: 0 });
        assert!(store.read_cache().is_none());

        // 체험이 남아 있으므로(5일) 체험 중으로 복귀.
        clock.set(T0 + 5 * DAY);
        let res = m.evaluate_on_start();
        assert_eq!(res.state, LicenseState::Trial);
    }
}
