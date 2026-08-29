//! §3-a 이벤트 탭 생명주기 상태표 — 순수 판정 로직만 이 모듈에 둔다(테스트 가능하게).
//!
//! 실제 `CGEventTapCreate` 호출·스레드 배선은 `engine.rs` 가 담당한다. 이 모듈은
//! (1) 상태 enum 과 스레드 경계를 넘는 원자적 게시, (2) 재시작 디바운스 판정, (3) 재활성화/
//! 재생성 에스컬레이션 카운터 — 셋 다 macOS 없이 단위 테스트할 수 있는 순수 로직이다.

use std::sync::atomic::{AtomicU8, Ordering};

/// §3-a 표의 상태 6종.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapState {
    NotInstalled,
    Installing,
    Active,
    Disabled,
    SuspendedBySleepOrLock,
    Terminated,
}

impl TapState {
    fn to_u8(self) -> u8 {
        match self {
            TapState::NotInstalled => 0,
            TapState::Installing => 1,
            TapState::Active => 2,
            TapState::Disabled => 3,
            TapState::SuspendedBySleepOrLock => 4,
            TapState::Terminated => 5,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            0 => TapState::NotInstalled,
            1 => TapState::Installing,
            2 => TapState::Active,
            3 => TapState::Disabled,
            4 => TapState::SuspendedBySleepOrLock,
            _ => TapState::Terminated,
        }
    }
}

/// `Engine::tap_state()` 가 메인(호출) 스레드에서 읽고, 탭 스레드가 전이마다 게시하는
/// 원자적 상태값. `docs/dev/architecture.md` §2.2 의 "락 없는 장치" 원칙을 상태 enum
/// 하나에 적용한 것이다.
pub struct AtomicTapState(AtomicU8);

impl AtomicTapState {
    pub fn new(initial: TapState) -> Self {
        AtomicTapState(AtomicU8::new(initial.to_u8()))
    }

    pub fn store(&self, s: TapState) {
        self.0.store(s.to_u8(), Ordering::Release);
    }

    pub fn load(&self) -> TapState {
        TapState::from_u8(self.0.load(Ordering::Acquire))
    }
}

/// §3-a `Installing` 시도(최초 설치 또는 재생성) 결과 분류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreateAttemptResult {
    /// `CGEventTapCreate` 가 성공했고, 곧바로 `CGEventTapIsEnabled() == true` 였다.
    Success,
    /// `CGEventTapCreate` 는 성공했지만(mach port 획득) 아직 비활성 상태다.
    CreatedButDisabled,
    /// 권한이 확인되지 않아 생성 자체를 시도하지 못했다(`TapCreateError::NotTrusted`).
    NotTrusted,
    /// 권한이 확인된 상태에서도 생성이 실패했다 — 치명적(`TapCreateError::CreateFailed`).
    Fatal,
}

/// §3-a `Installing` 전이 규칙 — 순수 함수. `engine.rs::handle_recreate_tap` 이 이
/// 함수 하나로 다음 상태를 결정한다(분기 로직을 여기 한 곳에 모아 단위 테스트로
/// §3-a 표를 직접 검증할 수 있게 한다).
pub fn tap_state_after_create_attempt(result: CreateAttemptResult) -> TapState {
    match result {
        CreateAttemptResult::Success => TapState::Active,
        CreateAttemptResult::CreatedButDisabled => TapState::Disabled,
        CreateAttemptResult::NotTrusted => TapState::NotInstalled,
        CreateAttemptResult::Fatal => TapState::Terminated,
    }
}

/// §3-a `Disabled` 전이 규칙 — 재활성화(`CGEventTapEnable` 뒤 `is_enabled()` 확인)
/// 시도 결과로부터 다음 상태를 결정하는 순수 함수.
pub fn tap_state_after_reenable(succeeded: bool) -> TapState {
    if succeeded {
        TapState::Active
    } else {
        TapState::Disabled
    }
}

/// §3-a 재시작 디바운스 판정 — 순수 함수(시간을 인자로 주입해 테스트 가능).
///
/// "직전 복구 시각 + 임계값" 과 지금 시각을 비교한다. 직전 복구가 없었으면(`None`)
/// 디바운스 대상이 아니다.
pub fn should_skip_restart(last_recovery_ms: Option<u64>, now_ms: u64, debounce_ms: u64) -> bool {
    match last_recovery_ms {
        Some(last) => now_ms.saturating_sub(last) < debounce_ms,
        None => false,
    }
}

/// 재활성화 시도 결과 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReenableOutcome {
    /// 재활성화가 성공했다 — 실패 카운터가 0으로 리셋된다.
    Recovered,
    /// 실패했지만 아직 상한에 못 미쳤다.
    StillFailing,
    /// 실패가 상한에 도달했다 — 탭 재생성으로 에스컬레이션해야 한다.
    EscalateToRecreate,
}

/// 재생성 시도 결과 판정.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecreateOutcome {
    Recovered,
    StillFailing,
    /// 재생성도 상한만큼 실패했다 — 프로세스 재실행을 고려해야 한다(§5#17).
    NeedsRelaunch,
}

/// §5#17 에스컬레이션 카운터 — 탭 스레드 로컬 상태(공유되지 않는다, 명령이 이 스레드에서
/// 직렬 처리되므로 락이 필요 없다).
#[derive(Debug, Default)]
pub struct RecoveryCounters {
    reenable_failures: u32,
    recreate_failures: u32,
}

impl RecoveryCounters {
    pub fn new() -> Self {
        Self::default()
    }

    /// 재활성화(`CGEventTapEnable` 뒤 `is_enabled()` 확인) 결과를 기록하고 판정한다.
    pub fn record_reenable_result(
        &mut self,
        succeeded: bool,
        max_attempts: u32,
    ) -> ReenableOutcome {
        if succeeded {
            self.reenable_failures = 0;
            return ReenableOutcome::Recovered;
        }
        self.reenable_failures += 1;
        if self.reenable_failures >= max_attempts {
            self.reenable_failures = 0;
            ReenableOutcome::EscalateToRecreate
        } else {
            ReenableOutcome::StillFailing
        }
    }

    /// 재생성(`EventTap::create` 성공 뒤 `is_enabled()` 확인) 결과를 기록하고 판정한다.
    pub fn record_recreate_result(
        &mut self,
        succeeded: bool,
        max_attempts: u32,
    ) -> RecreateOutcome {
        if succeeded {
            self.recreate_failures = 0;
            return RecreateOutcome::Recovered;
        }
        self.recreate_failures += 1;
        if self.recreate_failures >= max_attempts {
            self.recreate_failures = 0;
            RecreateOutcome::NeedsRelaunch
        } else {
            RecreateOutcome::StillFailing
        }
    }

    /// 현재까지 누적된 재활성화 실패 횟수 — 로깅 전용(시도 횟수를 남기기 위함,
    /// `docs/dev/manual-verification.md` 가 이 로그를 근거로 판정한다). 호출해도
    /// 상태를 바꾸지 않는다.
    pub fn reenable_failures(&self) -> u32 {
        self.reenable_failures
    }

    /// 위와 동일 — 재생성 실패 횟수.
    pub fn recreate_failures(&self) -> u32 {
        self.recreate_failures
    }
}

/// quick press 타이머 재예약 힌트(`docs/dev/architecture.md` §2.3) — outcome 이
/// "여전히 판정 대기 중"(소비했지만 아무것도 방출하지 않음)임을 시사하면, 다음 tick 을
/// 언제 예약해야 하는지 알려준다.
///
/// ⭐ M1 은 `has_quick_press_action`/`has_double_tap_action` 이 항상 `false` 라 이 함수가
/// `Some` 을 반환하는 경로에 실제로는 도달하지 않는다(`Arbiter` 가 항상 즉시
/// `HoldConfirmed` 로 확정하기 때문 — `quickpress.rs` 문서 참고). 그러나 M2 가 quick
/// press/double tap 액션을 등록하면 이 로직이 별도 수정 없이 그대로 맞물린다 — "매
/// 25ms 씩 깨우지 마라, 대기 중인 상태 머신이 있을 때만 재예약하라"는 설계 의도를
/// 지금부터 코드로 고정해 둔다.
pub fn quick_press_tick_delay_hint(
    kind: ultrakey_core::event::EventKind,
    outcome_is_pending: bool,
    quick_press_duration_ms: u64,
    double_tap_interval_ms: u64,
) -> Option<u64> {
    use ultrakey_core::event::EventKind;

    if !outcome_is_pending {
        return None;
    }
    match kind {
        // PendingDown 유지 — 타이머 만료(§3-c 표 4행)를 그 시각에 맞춰 예약한다.
        EventKind::KeyDown => Some(quick_press_duration_ms),
        // WaitingSecondTap 유지 — double tap 타임아웃(§3-c 표 7행)에 맞춰 예약한다.
        EventKind::KeyUp => Some(double_tap_interval_ms),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_tap_state_round_trips_all_variants() {
        let states = [
            TapState::NotInstalled,
            TapState::Installing,
            TapState::Active,
            TapState::Disabled,
            TapState::SuspendedBySleepOrLock,
            TapState::Terminated,
        ];
        for s in states {
            let a = AtomicTapState::new(s);
            assert_eq!(a.load(), s);
        }
    }

    #[test]
    fn atomic_tap_state_store_updates_load() {
        let a = AtomicTapState::new(TapState::NotInstalled);
        a.store(TapState::Active);
        assert_eq!(a.load(), TapState::Active);
    }

    // --- §3-a 전이 규칙(순수 함수) ---

    #[test]
    fn create_attempt_transitions_match_table() {
        assert_eq!(
            tap_state_after_create_attempt(CreateAttemptResult::Success),
            TapState::Active
        );
        assert_eq!(
            tap_state_after_create_attempt(CreateAttemptResult::CreatedButDisabled),
            TapState::Disabled
        );
        assert_eq!(
            tap_state_after_create_attempt(CreateAttemptResult::NotTrusted),
            TapState::NotInstalled
        );
        assert_eq!(
            tap_state_after_create_attempt(CreateAttemptResult::Fatal),
            TapState::Terminated
        );
    }

    #[test]
    fn reenable_transitions_match_table() {
        assert_eq!(tap_state_after_reenable(true), TapState::Active);
        assert_eq!(tap_state_after_reenable(false), TapState::Disabled);
    }

    // --- 재시작 디바운스 판정 ---

    #[test]
    fn no_previous_recovery_never_skips() {
        assert!(!should_skip_restart(None, 0, 5000));
        assert!(!should_skip_restart(None, 100_000, 5000));
    }

    #[test]
    fn within_threshold_skips_restart() {
        // 직전 복구가 1000ms 전이고 임계값이 5000ms 면 건너뛴다.
        assert!(should_skip_restart(Some(0), 1000, 5000));
    }

    #[test]
    fn at_or_past_threshold_does_not_skip() {
        // 정확히 임계값과 같으면 더 이상 디바운스 대상이 아니다(< 비교이므로).
        assert!(!should_skip_restart(Some(0), 5000, 5000));
        assert!(!should_skip_restart(Some(0), 6000, 5000));
    }

    #[test]
    fn does_not_underflow_when_now_before_last() {
        // 비정상적으로 now < last 여도(시계 이상) 패닉하지 않고 스킵 판정으로 처리된다.
        assert!(should_skip_restart(Some(10_000), 100, 5000));
    }

    // --- 에스컬레이션 카운터 ---

    #[test]
    fn reenable_success_resets_counter() {
        let mut c = RecoveryCounters::new();
        assert_eq!(
            c.record_reenable_result(false, 5),
            ReenableOutcome::StillFailing
        );
        assert_eq!(
            c.record_reenable_result(true, 5),
            ReenableOutcome::Recovered
        );
        // 리셋된 뒤 다시 실패해도 1회차부터 시작한다.
        for _ in 0..4 {
            assert_eq!(
                c.record_reenable_result(false, 5),
                ReenableOutcome::StillFailing
            );
        }
        assert_eq!(
            c.record_reenable_result(false, 5),
            ReenableOutcome::EscalateToRecreate
        );
    }

    /// 재활성화 5회 실패 → 재생성으로 에스컬레이션(architecture.md §4 기본값).
    #[test]
    fn reenable_escalates_to_recreate_after_max_attempts() {
        let mut c = RecoveryCounters::new();
        let mut last = ReenableOutcome::Recovered;
        for _ in 0..5 {
            last = c.record_reenable_result(false, 5);
        }
        assert_eq!(last, ReenableOutcome::EscalateToRecreate);
    }

    /// 재생성 3회 실패 → NeedsRelaunch(architecture.md §4 기본값).
    #[test]
    fn recreate_needs_relaunch_after_max_attempts() {
        let mut c = RecoveryCounters::new();
        let mut last = RecreateOutcome::Recovered;
        for _ in 0..3 {
            last = c.record_recreate_result(false, 3);
        }
        assert_eq!(last, RecreateOutcome::NeedsRelaunch);
    }

    #[test]
    fn recreate_success_resets_counter() {
        let mut c = RecoveryCounters::new();
        c.record_recreate_result(false, 3);
        c.record_recreate_result(false, 3);
        assert_eq!(
            c.record_recreate_result(true, 3),
            RecreateOutcome::Recovered
        );
        // 리셋됐으므로 다시 2회 실패로는 상한에 도달하지 않는다.
        assert_eq!(
            c.record_recreate_result(false, 3),
            RecreateOutcome::StillFailing
        );
        assert_eq!(
            c.record_recreate_result(false, 3),
            RecreateOutcome::StillFailing
        );
        assert_eq!(
            c.record_recreate_result(false, 3),
            RecreateOutcome::NeedsRelaunch
        );
    }

    // --- quick press 타이머 재예약 힌트 ---

    #[test]
    fn tick_hint_none_when_not_pending() {
        assert_eq!(
            quick_press_tick_delay_hint(ultrakey_core::event::EventKind::KeyDown, false, 1000, 300),
            None
        );
    }

    #[test]
    fn tick_hint_uses_quick_press_duration_for_key_down() {
        assert_eq!(
            quick_press_tick_delay_hint(ultrakey_core::event::EventKind::KeyDown, true, 1000, 300),
            Some(1000)
        );
    }

    #[test]
    fn tick_hint_uses_double_tap_interval_for_key_up() {
        assert_eq!(
            quick_press_tick_delay_hint(ultrakey_core::event::EventKind::KeyUp, true, 1000, 300),
            Some(300)
        );
    }
}
