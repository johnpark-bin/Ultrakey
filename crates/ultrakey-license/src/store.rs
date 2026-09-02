//! 영속 기산점·라이선스 캐시 저장 추상화 — 순수 상태 머신이 저장 뒤를 모르게 한다.
//!
//! 명세 §3.5 는 트라이얼 기산점의 저장을 **빌드별로 분기**한다(이슈 #99 개정):
//! 정식 Apple 서명 빌드는 Keychain(`Security.framework` — `ultrakey-platform` 의
//! `unsafe` FFI 책임), 개발/자체 서명 빌드는 파일([`crate::file_store`]).
//! 이 크레이트의 순수 상태 머신은 오직 트레이트(`TrialStore`·`CacheStore`)를 통해
//! 읽고 쓴다. 그래서 상태 머신 테스트는 Keychain 없이 인메모리 저장으로 돈다.
//!
//! 저장하는 값은 명세 §3.5 그대로 최소화한다:
//! - 트라이얼: `trial_started_at`(체험 20일 기산점) + `last_seen_at`(시계 조작 완화용).
//! - 라이선스 캐시: `cache_token` + `cache_issued_at`(오프라인 유예 기준) + 활성화 슬롯.

use crate::clock::Timestamp;
use std::sync::Mutex;

/// 체험 상태에 대한 영속화 단위 — 기산점과 최근 관찰 시각.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TrialClock {
    /// 체험 20일을 재는 기산점(최초 실행 시각).
    pub trial_started_at: Timestamp,
    /// 시계 조작 완화용 최근 관찰 시각(§3.6).
    pub last_seen_at: Timestamp,
}

/// 체험 영속 상태 — 기산점·최근 관찰 시각 + **체험 소진 여부**.
///
/// ⭐ §3.1: "체험은 기기(설치)당 1회다. 라이선스가 무효화되거나 취소돼도 체험으로
/// 되돌아가지 않는다." `trial_consumed` 가 `true` 면 이 기기는 이미 라이선스를
/// 유효화했거나 무효화를 겪어, 체험 기간이 남아 있어도 `Trial` 로 복귀하지 못한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct TrialRecord {
    pub clock: TrialClock,
    pub trial_consumed: bool,
}

/// 트라이얼 기산점을 저장하는 저장소 추상화.
///
/// ⚠️ 요구사항(명세 §3.5): 정식 서명 빌드에서는 **앱 삭제·재설치에도 기산점이
/// 살아 있어야 한다**(Keychain). 파일 빌드는 일반 재설치에는 생존하나 명시적
/// 파일 삭제에는 리셋된다(§5 #10). 저장 위치를 결정하는 것은 구현체(어댑터)의
/// 책임이다.
pub trait TrialStore {
    /// 저장된 기산점. 없으면 `None`(아직 체험을 시작하지 않은 최초 실행).
    fn read(&self) -> Option<TrialRecord>;
    /// 기산점을 기록한다.
    fn write(&self, record: TrialRecord);
}

/// 라이선스 캐시 — 오프라인 재실행 시 서버 재검증 없이 신뢰하는 값(§3.5).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LicenseCache {
    pub license_key: String,
    pub device_id: String,
    pub cache_token: String,
    pub cache_issued_at: Timestamp,
    pub activations_used: u32,
    pub activations_limit: u32,
}

/// 라이선스 캐시 저장 추상화 — 기산점과 같은 저장소에 둔다(명세 §3.5: "저장
/// 메커니즘이 하나로 통일된다").
pub trait CacheStore {
    fn read_cache(&self) -> Option<LicenseCache>;
    fn write_cache(&self, cache: &LicenseCache);
    fn clear_cache(&self);
}

// ---------------------------------------------------------------------------
// 인메모리 구현 — 순수 로직 단위 테스트·진단용.
// `Mutex` 로 `&self` 가변 갱신을 허용해 상태 머신이 `&mut self` 없이도 프로세스
// 내에서 상태를 담고, 트레이트 객체 `Send + Sync` 계약을 만족한다.
// Keychain 어댑터는 `ultrakey-platform` 이 트레이트를 구현해 주입한다.
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
pub struct InMemoryStore {
    trial: Mutex<Option<TrialRecord>>,
    cache: Mutex<Option<LicenseCache>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self {
            trial: Mutex::new(None),
            cache: Mutex::new(None),
        }
    }

    /// 테스트가 기산점을 직접 심는다.
    pub fn with_trial(clock: TrialClock) -> Self {
        Self {
            trial: Mutex::new(Some(TrialRecord {
                clock,
                trial_consumed: false,
            })),
            cache: Mutex::new(None),
        }
    }
}

impl TrialStore for InMemoryStore {
    fn read(&self) -> Option<TrialRecord> {
        *self.trial.lock().unwrap()
    }
    fn write(&self, record: TrialRecord) {
        *self.trial.lock().unwrap() = Some(record);
    }
}

impl CacheStore for InMemoryStore {
    fn read_cache(&self) -> Option<LicenseCache> {
        self.cache.lock().unwrap().clone()
    }
    fn write_cache(&self, cache: &LicenseCache) {
        *self.cache.lock().unwrap() = Some(cache.clone());
    }
    fn clear_cache(&self) {
        *self.cache.lock().unwrap() = None;
    }
}
