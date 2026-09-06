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

/// 탭 생성을 시도하는 이유 — 실패했을 때의 처리가 갈린다(이슈 #140, P3 심사 지적 2·3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapCreateTrigger {
    /// `tap_thread_main` 의 최초 설치. 실패 정책은 §3-a 표 그대로(`NotTrusted` →
    /// `NotInstalled` + F-11 온보딩, `CreateFailed` → `Terminated` 치명).
    InitialInstall,
    /// 설정 변경으로 마스크가 달라져 **살아 있던 탭을 해체하고** 다시 만드는 경우.
    /// 실패하면 어느 사유든 `NotInstalled` 로 두고 `EngineEvent::TapLost` 를 올린다 —
    /// 사용자의 클릭 하나가 프로세스를 `Terminated`(재시도 없음) 로 굳히면 안 되고,
    /// "살아 있던 탭이 사라졌다 → 권한 모델(`report_tap_create_failed`, `OutOfSync`
    /// 진단)에 이관" 이 정확히 `TapLost` 의 계약이기 때문이다. `NotTrusted` 이벤트는
    /// 온보딩 모달만 띄우고 권한 모니터에 알리지 않으므로 여기서는 쓰지 않는다.
    MaskChanged,
}

/// [`tap_state_after_create_attempt`] 의 트리거 인지 판. 최초 설치는 표 그대로,
/// 재생성 실패는 사유와 무관하게 `NotInstalled`(탭은 이미 해체됐고 복구는 권한
/// 모니터 몫).
pub fn tap_state_after_create_attempt_for(
    trigger: TapCreateTrigger,
    result: CreateAttemptResult,
) -> TapState {
    match (trigger, result) {
        (TapCreateTrigger::MaskChanged, CreateAttemptResult::NotTrusted)
        | (TapCreateTrigger::MaskChanged, CreateAttemptResult::Fatal) => TapState::NotInstalled,
        (_, r) => tap_state_after_create_attempt(r),
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

/// `EngineCommand::RecoverTap` 처리 판정 — 순수 함수(이슈 #65 Phase 1 리뷰 교정 2).
///
/// 트램폴린의 연속 재활성화 예산(`ultrakey_platform::event_tap::ReenableBudget`)이
/// 소진됐거나 권한이 이미 없으면 탭을 해체한다 — **재생성을 시도하지 않는다.**
/// 초안은 "예산 소진 → `handle_recreate_tap`" 을 제안했으나, stale
/// `AXIsProcessTrusted()` 상황에서는 재생성마다 새 탭 = 새 예산(5회)이 다시 채워져
/// **더 느린 폭주**가 된다는 것이 리뷰의 기각 사유다 — 재생성은 새 탭 인스턴스를
/// 만드니 예산도 자연히 리셋되기 때문이다. 복구는 오직 권한 모니터가 `Granted` 를
/// 재확인해 **완전히 새 `Engine`** 을 만드는 것으로만 일어난다
/// (`apps/ultrakey-app` 의 `start_engine_if_needed`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoverTapDecision {
    /// 탭을 해체하고(`NotInstalled`) `EngineEvent::TapLost` 를 올린다.
    Teardown,
    /// 트램폴린이 예산 안에서 이미 재활성화를 시도했다 — 그 결과를 관찰만 한다.
    KeepAlive,
}

/// `trusted` = 이 시점의 `AXIsProcessTrusted()`, `reenable_budget_exhausted` =
/// 트램폴린의 연속 재활성화 예산이 소진됐는가(`EventTap::reenable_budget_exhausted`).
/// 둘 중 하나라도 참이면 해체한다 — `trusted` 값의 순간적 stale 여부에 단독으로
/// 의존하지 않기 위해 "탭이 즉시 다시 꺼진다는 사실 자체"(예산 소진)를 대등한
/// 신호로 둔다(이슈 #65 Phase 1 진단 결론).
pub fn recover_tap_decision(trusted: bool, reenable_budget_exhausted: bool) -> RecoverTapDecision {
    if !trusted || reenable_budget_exhausted {
        RecoverTapDecision::Teardown
    } else {
        RecoverTapDecision::KeepAlive
    }
}

/// `EngineCommand::Reconfigure` 처리 중 탭을 재생성할지 판정 — 순수 함수(이슈 #140,
/// `docs/plan/issue-140-event-mask.md` §2 D2).
///
/// ⭐ **이슈 #65 와의 경계**. #65 가 굳힌 "재생성 금지"는 `RecoverTap` 트리거(탭
/// 자신의 비활성화 통지 · 트램폴린 예산 소진 · 권한 상실 — `recover_tap_decision` 이
/// 판정하는 그 셋)에 대한 것이다. 그 트리거 아래서 재생성하면 새 탭 인스턴스마다
/// `ReenableBudget` 이 다시 채워지고, stale `AXIsProcessTrusted()` 아래서는 그게
/// **더 느린 폭주**가 된다(`recover_tap_decision` 문서 참고) — 그래서 `RecoverTap`
/// 경로는 재생성 없이 해체만 한다.
///
/// 이 함수가 판정하는 건 그 트리거가 아니라 **사용자의 설정 변경**이다 —
/// `EngineCommand::Reconfigure` 는 사람이 설정 화면에서 클릭한 속도로만 도착하므로
/// 예산 소진 같은 폭주 신호가 아니다. 그래서 여기서만 재생성을 허용한다 — 단, 그때도
/// 폭주가 진행 중이면(`reenable_budget_exhausted`) 새 탭을 만들지 않고
/// (`KeepTap`), 탭이 아예 없으면(`!has_tap`) 재생성하지 않는다 — 복구는 여전히
/// 권한 모니터가 새 `Engine` 을 만드는 몫이다(#65 교정 2 유지). 다음 `Engine::start`
/// 가 갱신된 설정으로 마스크를 새로 도출하므로 이번 변경이 유실되지는 않는다.
///
/// 판정 순서:
/// 1. `fatal || !has_tap` → [`ReconfigureTapDecision::NoTap`] — 탭이 없거나 치명적
///    실패 이후에는 아무 것도 하지 않는다.
/// 2. `reenable_budget_exhausted` → [`ReconfigureTapDecision::KeepTap`] — 폭주 중에는
///    새 탭(= 새 예산)을 주지 않는다. `RecoverTap` 이 곧 해체한다.
/// 3. `!mask_changed` → [`ReconfigureTapDecision::KeepTap`] — 도출된 마스크가 그대로면
///    `CGEventTapCreate` 를 다시 부를 이유가 없다.
/// 4. 그 외 → [`ReconfigureTapDecision::Recreate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconfigureTapDecision {
    /// 지금 탭을 그대로 둔다.
    KeepTap,
    /// 지금 탭을 해체하고 새 마스크로 다시 만든다.
    Recreate,
    /// 탭이 없다(또는 치명적 실패 이후) — 아무 것도 하지 않는다.
    NoTap,
}

/// `has_tap` = 이 시점에 `TapThreadState.tap` 이 `Some` 인가, `fatal` =
/// `TapThreadState.fatal`, `reenable_budget_exhausted` = 지금 탭의
/// `EventTap::reenable_budget_exhausted()`, `mask_changed` = 이번에 도출한
/// 이벤트 마스크가 지금 탭의 `EventTap::mask()` 와 다른가.
pub fn reconfigure_tap_decision(
    has_tap: bool,
    fatal: bool,
    reenable_budget_exhausted: bool,
    mask_changed: bool,
) -> ReconfigureTapDecision {
    if fatal || !has_tap {
        ReconfigureTapDecision::NoTap
    } else if reenable_budget_exhausted || !mask_changed {
        ReconfigureTapDecision::KeepTap
    } else {
        ReconfigureTapDecision::Recreate
    }
}

/// ⭐ 이슈 #108 자동 복구(안전망) 판정 — 순수 함수. `Watchdog` 이 매 폴링마다
/// 이 결과를 물어보고, 참일 때만 `hid_lock::set_caps_lock_state(false)` 를 부른다
/// (탭 콜백 밖 — §3-a 콜백 금지 규정 밖이다).
///
/// - `alias_active` — `EngineConfig::caps_lock_alias.is_some()`(D-1 설치, caps lock 이
///   modifier 소스로 배정된 동안). `false` 면 캡스락은 여전히 사용자가 직접 켜고 끄는
///   평범한 키이므로 안전망이 개입하지 않는다 — 이 검사를 가장 먼저 두어 D-1 비활성
///   구성에서는 `caps_lock_state()`(mach 호출)조차 부르지 않는다.
/// - `observed` — 이번 폴링에서 읽은 `caps_lock_state()`. 읽기 자체가 실패하면(`None`)
///   개입하지 않는다 — 실패를 "켜짐"으로 추정해 끄려고 시도하면 성공한 잠금까지
///   흔들 수 있다.
/// - `owned` — `SharedState::caps_lock_owned_lock`. 직전에 우리 자신의
///   `Effect::ToggleCapsLock` 이 이 잠금을 켰다면(`Double tap shift`·`Left/right shift`·
///   `Shift + caps lock = caps lock` 류 경로 C 규칙의 **의도된** 결과) 참이다 — 그
///   잠금은 사용자가 명시적으로 만든 것이므로 되돌리지 않는다.
///
/// D-1 이 설치된 동안은 물리 caps lock 키가 커널에서 F18 로 이미 바뀌어 있어, 사용자가
/// 키보드로 진짜 caps lock 을 잠글 수 있는 유일한 수단은 우리 자신의 경로 C
/// (`Effect::ToggleCapsLock`)뿐이다 — 그래서 `owned` 하나만으로 "의도된 잠금"과 "그
/// 밖의 모든 잠금"(외부 유틸리티·`hidutil`·절전/화면잠금 중 커널이 직접 토글한 경우
/// 등, 이슈 #108 원인 (c))을 안전하게 나눌 수 있다. 시간 기반 유효기간을 두지 않는다
/// — 사용자가 그 잠금을 오래 켜 두어도 안전망이 되돌리면 안 되므로(명세 §5 #20 이
/// 이미 기각한 "시간 기반 자동 해제"와 같은 문제가 생긴다), `owned` 는 우리가 다음에
/// 그 상태를 off 로 되돌리거나 다시 on 시킬 때까지 유효한 불리언이다.
pub fn caps_lock_recovery(alias_active: bool, observed: Option<bool>, owned: bool) -> bool {
    alias_active && observed == Some(true) && !owned
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

    /// 이슈 #140 — 재생성 실패는 치명(`Terminated`)이 아니라 `NotInstalled` + `TapLost`.
    #[test]
    fn recreate_failures_fall_back_to_not_installed_instead_of_terminated() {
        use TapCreateTrigger::*;
        assert_eq!(
            tap_state_after_create_attempt_for(MaskChanged, CreateAttemptResult::Fatal),
            TapState::NotInstalled
        );
        assert_eq!(
            tap_state_after_create_attempt_for(MaskChanged, CreateAttemptResult::NotTrusted),
            TapState::NotInstalled
        );
        assert_eq!(
            tap_state_after_create_attempt_for(MaskChanged, CreateAttemptResult::Success),
            TapState::Active
        );
        // 최초 설치는 표 그대로 — 치명 실패는 여전히 `Terminated`.
        assert_eq!(
            tap_state_after_create_attempt_for(InitialInstall, CreateAttemptResult::Fatal),
            TapState::Terminated
        );
        assert_eq!(
            tap_state_after_create_attempt_for(InitialInstall, CreateAttemptResult::NotTrusted),
            TapState::NotInstalled
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

    // --- `RecoverTap` 판정(이슈 #65 Phase 1 리뷰 교정 2) — 4조합 전부 ---

    #[test]
    fn recover_tap_keeps_alive_when_trusted_and_budget_not_exhausted() {
        assert_eq!(
            recover_tap_decision(true, false),
            RecoverTapDecision::KeepAlive
        );
    }

    #[test]
    fn recover_tap_tears_down_when_not_trusted_even_if_budget_remains() {
        assert_eq!(
            recover_tap_decision(false, false),
            RecoverTapDecision::Teardown
        );
    }

    #[test]
    fn recover_tap_tears_down_when_budget_exhausted_even_if_still_trusted() {
        // ⭐ 이 조합이 이슈 #65 의 핵심 — `AXIsProcessTrusted()` 가 stale `true` 를
        // 돌려줘도, "탭이 즉시 다시 꺼진다는 사실 자체"(예산 소진)만으로 해체해야
        // 한다.
        assert_eq!(
            recover_tap_decision(true, true),
            RecoverTapDecision::Teardown
        );
    }

    #[test]
    fn recover_tap_tears_down_when_neither_trusted_nor_within_budget() {
        assert_eq!(
            recover_tap_decision(false, true),
            RecoverTapDecision::Teardown
        );
    }

    // --- `Reconfigure` 재생성 판정(이슈 #140) ---

    #[test]
    fn reconfigure_no_tap_when_tap_absent() {
        assert_eq!(
            reconfigure_tap_decision(false, false, false, true),
            ReconfigureTapDecision::NoTap
        );
    }

    #[test]
    fn reconfigure_no_tap_when_fatal_even_if_mask_changed() {
        assert_eq!(
            reconfigure_tap_decision(true, true, false, true),
            ReconfigureTapDecision::NoTap
        );
    }

    #[test]
    fn reconfigure_keeps_tap_when_budget_exhausted_even_if_mask_changed() {
        // ⭐ 폭주 중(예산 소진)에는 마스크가 바뀌었어도 새 탭 = 새 예산을 주지 않는다
        // — `RecoverTap` 이 곧 해체한다.
        assert_eq!(
            reconfigure_tap_decision(true, false, true, true),
            ReconfigureTapDecision::KeepTap
        );
    }

    #[test]
    fn reconfigure_keeps_tap_when_mask_unchanged() {
        assert_eq!(
            reconfigure_tap_decision(true, false, false, false),
            ReconfigureTapDecision::KeepTap
        );
    }

    #[test]
    fn reconfigure_recreates_when_mask_changed_and_no_storm() {
        assert_eq!(
            reconfigure_tap_decision(true, false, false, true),
            ReconfigureTapDecision::Recreate
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

    // --- 이슈 #108 자동 복구 안전망 판정 — (alias_active, observed, owned) 전수 ---

    #[test]
    fn recovery_fires_only_when_alias_active_and_locked_and_not_owned() {
        assert!(caps_lock_recovery(true, Some(true), false));
    }

    #[test]
    fn recovery_does_not_revert_our_own_intended_lock() {
        // ⭐ Double tap shift·Left/right shift·Shift + caps lock = caps lock 류 경로 C
        // 규칙이 방금 낸 의도된 잠금 — 이 케이스가 불리언 설계의 충분성을 증명한다.
        assert!(!caps_lock_recovery(true, Some(true), true));
    }

    #[test]
    fn recovery_skips_when_d1_not_installed() {
        // caps lock 이 modifier 소스가 아니면 평범한 키다 — 안전망이 개입하지 않는다.
        assert!(!caps_lock_recovery(false, Some(true), false));
    }

    #[test]
    fn recovery_skips_when_not_locked() {
        assert!(!caps_lock_recovery(true, Some(false), false));
    }

    #[test]
    fn recovery_skips_when_read_failed() {
        // 읽기 실패를 "켜짐"으로 추정하지 않는다 — 성공한 잠금까지 흔들 위험을 피한다.
        assert!(!caps_lock_recovery(true, None, false));
    }
}
