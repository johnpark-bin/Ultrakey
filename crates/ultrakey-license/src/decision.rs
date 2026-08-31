//! 체험·유예·시계 조작 완화 판정의 **순수 로직**.
//!
//! 명세 §3.2(체험 경과)·§3.4(오프라인 유예)·§3.5/§3.6(시계 조작 완화)의 계산을
//! 상태 머신과 분리된 순수 함수로 모은다. 상태 머신(machine.rs)은 이 함수들을
//! 호출해 전이를 결정한다. 각 판정을 단독으로 단위 테스트할 수 있다.

use crate::clock::Timestamp;
use crate::store::TrialClock;

/// 체험 기간(일). 명세 §3.2: "현재 시각 − 기산점 < 20일" 이면 체험 중. 원본
/// SuperKey 의 "Free for 20 days"(`superkey-inventory.md` §1.5) — 확정된 값이다.
pub const TRIAL_DAYS: i64 = 20;

/// 초 단위.
pub const DAY_SECS: i64 = 86_400;

/// 체험 판정 결과 — 상태 머신이 전이를 결정하고, trial_info_from 이 표시용으로
/// 변환한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrialDecisionOutcome {
    /// 체험 분기: `None` 을 반환하는 대신 분기 플래그로 표현한다.
    pub expired: bool,
    /// 남은 체험 일수 — 체험 중(만료 아님)일 때만 의미.
    pub days_remaining: Option<i64>,
    /// 시계가 과거로 되돌아가 감지됐는지.
    pub clock_rolled_back: bool,
}

/// 체험 기산점 계산.
///
/// ⭐ **시계 조작 완화(§3.5/§3.6)**: 저장된 `last_seen_at` 은 "가장 최근에 관찰한
/// 시각"이다. 시계가 과거로 되돌려져 `now < last_seen_at` 이면, 경과 기간을
/// `now` 가 아니라 `last_seen_at` 기준으로 계산해 **남은 일수가 늘어나지 않게**
/// 보수적으로 고정한다(명세 §3.5 "남은 일수를 가장 최근에 계산된 값 유지").
pub fn evaluate_trial(store: &TrialClock, now: Timestamp) -> TrialDecisionOutcome {
    // 시계 되돌림 감지: 마지막 관찰 시각이 지금보다 미래면 과거로 되돌아간 것.
    let clock_rolled_back = now < store.last_seen_at;

    // 경과 기간의 기준 시각 — 시계가 되돌려졌으면 가장 최근 관찰 시각을 써서
    // 남은 일수를 늘리지 않는다.
    let effective = if clock_rolled_back {
        store.last_seen_at
    } else {
        now
    };
    let elapsed_secs = effective.saturating_sub(store.trial_started_at);
    let elapsed_days = elapsed_secs / DAY_SECS;

    if elapsed_days >= TRIAL_DAYS {
        TrialDecisionOutcome {
            expired: true,
            days_remaining: None,
            clock_rolled_back,
        }
    } else {
        // 남은 일수 = ceil(남은 시간 / 하루). 최소 1(남은 일수 0이 되면 즉시 만료로
        // 보이게 해서 "체험판 — 0일 남음" 같은 이상한 표시를 막는다).
        // `div_ceil` 은 nightly 라 정수 올림을 직접 계산한다.
        let remaining_secs = (TRIAL_DAYS - elapsed_days) * DAY_SECS - (elapsed_secs % DAY_SECS);
        let days_remaining = (remaining_secs + DAY_SECS - 1) / DAY_SECS;
        let days_remaining = days_remaining.max(1);
        TrialDecisionOutcome {
            expired: false,
            days_remaining: Some(days_remaining),
            clock_rolled_back,
        }
    }
}

/// 시계 되돌림 여부만 판정.
pub fn is_clock_rollback(last_seen_at: Timestamp, now: Timestamp) -> bool {
    now < last_seen_at
}

/// 다음 `last_seen_at` — 시계가 되돌려졌으면 이전 값을 유지(보수적 고정), 아니면
/// now 로 갱신한다.
pub fn next_last_seen(prev: &TrialClock, now: Timestamp) -> Timestamp {
    if is_clock_rollback(prev.last_seen_at, now) {
        prev.last_seen_at
    } else {
        now
    }
}

/// 오프라인 유예(§3.4) — 캐시 발급 시각으로부터 `grace` 일수까지 유효.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfflineGrace {
    /// 유예 일수. 명세 §3.4 는 구체 값을 `(미확정)` 으로 남겼다 — 제품이 고르는
    /// 합리적 기본값(architecture.md §4 "미확정 값"과 같은 성격). 기본 30일.
    pub days: i64,
}

impl Default for OfflineGrace {
    fn default() -> Self {
        Self { days: 30 }
    }
}

/// 캐시가 유예 창 안에 있는지.
pub fn cache_within_grace(
    cache_issued_at: Timestamp,
    now: Timestamp,
    grace: &OfflineGrace,
) -> bool {
    now <= cache_issued_at.saturating_add(grace.days * DAY_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: Timestamp = 1_000_000_000;

    fn fresh(now: Timestamp) -> TrialClock {
        TrialClock {
            trial_started_at: now,
            last_seen_at: now,
        }
    }

    #[test]
    fn early_trial_reports_remaining_days() {
        let store = fresh(T0);
        let r = evaluate_trial(&store, T0 + 5 * DAY_SECS);
        assert!(!r.expired);
        assert_eq!(r.days_remaining, Some(15));
        assert!(!r.clock_rolled_back);
    }

    #[test]
    fn initial_run_has_20_days() {
        let store = fresh(T0);
        let r = evaluate_trial(&store, T0);
        assert_eq!(r.days_remaining, Some(20));
    }

    #[test]
    fn exactly_20_days_is_expired() {
        // §3.2: "현재 시각 − 기산점 < 20일" → 정확히 20일은 만료.
        let store = fresh(T0);
        let r = evaluate_trial(&store, T0 + 20 * DAY_SECS);
        assert!(r.expired);
        assert_eq!(r.days_remaining, None);
    }

    #[test]
    fn over_20_days_is_expired() {
        let store = fresh(T0);
        let r = evaluate_trial(&store, T0 + 25 * DAY_SECS);
        assert!(r.expired);
    }

    #[test]
    fn clock_rollback_detected() {
        assert!(is_clock_rollback(T0 + 10, T0));
        assert!(!is_clock_rollback(T0, T0 + 10));
    }

    #[test]
    fn last_seen_holds_on_rollback_else_advances() {
        let prev = TrialClock {
            trial_started_at: T0,
            last_seen_at: T0 + 10,
        };
        // 시계를 T0 로 되돌림 → last_seen_at 유지.
        assert_eq!(next_last_seen(&prev, T0), T0 + 10);
        // 시계 정상 진행 → now 로 갱신.
        assert_eq!(next_last_seen(&prev, T0 + 12), T0 + 12);
    }

    #[test]
    fn grace_boundaries() {
        let g = OfflineGrace { days: 30 };
        assert!(cache_within_grace(T0, T0, &g));
        assert!(cache_within_grace(T0, T0 + 30 * DAY_SECS, &g));
        assert!(!cache_within_grace(T0, T0 + 30 * DAY_SECS + 1, &g));
    }
}
