//! 시계 추상화 — 상태 판정의 시간 입력.
//!
//! 순수 로직이 시스템 시계에 직접 의존하면 단위 테스트가 불가능하다. 이 크레이트의
//! 모든 시간 판정(체험 경과·오프라인 유예·시계 조작)은 [`Clock`] 을 입력으로 받는다.
//! 실제 앱은 [`Clock::system`] 을, 테스트는 고정 시각을 주입한다.

/// 유닉스 시각(초). 모든 시간 판정의 단위.
pub type Timestamp = i64;

/// 앱이 쓸 수 있는 시간원. 테스트는 결정론적 고정 시각을 주입한다.
pub trait Clock {
    /// 현재 시각(epoch seconds).
    fn now(&self) -> Timestamp;
}

/// 실제 시스템 시계. `SystemTime::now()` → epoch 초.
pub struct SystemClock;

impl Default for SystemClock {
    fn default() -> Self {
        SystemClock
    }
}

impl Clock for SystemClock {
    fn now(&self) -> Timestamp {
        system_time_now()
    }
}

/// `SystemTime::now()` 를 epoch 초로 변환한다. 1970 이전(음수)이나 과도하게 먼 미래
/// 시계 설정에서도 정수로 안전하게 떨어지게 `as_secs` 를 `i64` 로 캐스팅한다.
pub fn system_time_now() -> Timestamp {
    use std::time::{SystemTime, UNIX_EPOCH};
    let now = SystemTime::now().duration_since(UNIX_EPOCH);
    now.map(|d| d.as_secs() as Timestamp).unwrap_or(0)
}
