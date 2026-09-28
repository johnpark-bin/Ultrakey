//! ⭐ 순수 판정 로직 — macOS 없이 단위 테스트되는 상태 머신.
//!
//! `docs/spec/permissions-onboarding.md` §2(S1~S4)·§3.3·§3.4·§3.5 를 그대로 옮긴다.
//! 이 모듈은 실제 `AXIsProcessTrusted()` 호출이나 스레드를 전혀 알지 못한다 —
//! [`crate::monitor::PermissionMonitor`] 가 이 모델을 실제 시스템에 연결한다.

/// F-11 이 추적하는 권한 상태(§3.5 표의 행에 대응).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermissionState {
    /// 아직 한 번도 확인하지 않음.
    Unknown,
    /// `AXIsProcessTrusted() == false` — 온보딩 모달을 띄운다(§2 S1).
    Denied,
    /// `AXIsProcessTrusted() == true` — 정상.
    Granted,
    /// ⭐ `AXIsProcessTrusted() == true` 인데 탭 생성이 실패 — 진단 화면(§3.3).
    OutOfSync,
}

/// 폴링 주기. 온보딩 중에는 짧게, 배경에서는 길게(§3.4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollMode {
    Onboarding,
    Background,
}

/// 상태 전이. UI(Tauri 앱)가 이것을 구독한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PermissionTransition {
    pub from: PermissionState,
    pub to: PermissionState,
}

/// ⭐ 순수 판정 로직 — macOS 없이 단위 테스트된다.
///
/// 내부 상태 전이는 전부 이 파일 안에서 일어난다. 같은 상태로의 "전이"는 전이가
/// 아니라고 보고 `None` 을 반환한다 — 중복 알림을 UI 에 흘리지 않기 위해서다.
pub struct PermissionModel {
    state: PermissionState,
    /// ⭐ 이슈 #149 D1 — `OutOfSync` 진입 시각(monotonic ms). 호출자(`monitor.rs`)가
    /// `Instant` baseline 으로 계산해 주입하므로 모델은 시스템 시계와 무관하다.
    /// 재시도 실패(`NotTrusted`/예산 소진 → `OutOfSync` 복귀)마다 갱신돼 다음
    /// 백오프 단계를 탄다.
    last_out_of_sync_ms: Option<u64>,
    /// `OutOfSync` 자동 재창착 시도 횟수. 성공(`observe_tap_created`)이나 사용자
    /// 토글(`trusted == false` 관측)에서 0 으로 리셋된다.
    out_of_sync_attempts: u8,
}

/// ⭐ 이슈 #149 D1 — `OutOfSync` 자동 재창착 백오프 단계별 대기 시간(ms).
/// [30초, 2분, 10분] — PR #66 이 기각한 "5초마다 저속 순환"이 재발하지 않게
/// 장시간으로 둔다.
const OUT_OF_SYNC_RETRY_BACKOFF_MS: [u64; 3] = [30_000, 120_000, 600_000];

/// 재창착 상한 — 3회 실패 뒤에는 자동으로 더 시도하지 않고 진단 화면(사용자
/// 절차)이 남는다.
const OUT_OF_SYNC_RETRY_MAX_ATTEMPTS: u8 = 3;

impl PermissionModel {
    pub fn new() -> Self {
        PermissionModel {
            state: PermissionState::Unknown,
            last_out_of_sync_ms: None,
            out_of_sync_attempts: 0,
        }
    }

    pub fn state(&self) -> PermissionState {
        self.state
    }

    /// 실제 상태가 바뀔 때만 `Some` 을 반환하고, 그렇지 않으면 `None` 을 반환한다.
    fn move_to(&mut self, to: PermissionState) -> Option<PermissionTransition> {
        if self.state == to {
            return None;
        }
        let from = self.state;
        self.state = to;
        Some(PermissionTransition { from, to })
    }

    /// 폴링 1회의 결과를 반영한다. 상태가 바뀌었으면 전이를 반환한다.
    ///
    /// §2 S1(최초 미부여 → 부여)과 §2 S3(런타임 취소, `Granted` → `Denied`)를
    /// 동일한 규칙으로 처리한다 — `trusted` 하나만으로 목표 상태가 정해지므로
    /// 별도 분기가 필요 없다.
    ///
    /// ⭐ **PR #66 검수 반영 — `OutOfSync` 가드 (+ 이슈 #149 D1 유한 백오프 개방).**
    /// `OutOfSync` 는 §2 S4·§3.3 이 규정하는 **sticky 진단 상태**다 — 진단 화면에서
    /// 사용자의 리셋·재시작 또는 수동 절차로만 벗어난다(`observe_tap_created` 와
    /// 아래 D1 백오프 도달이 그 탈출구). 이 가드가 없으면, 권한은 여전히 `true`인데
    /// 탭을 만들 수 없는 상태에서 배경 폴링의 `observe_trusted(true)` 가 곧바로
    /// `Granted` 로 되돌려 "5초마다 엔진 재기동 → 새 탭 → 재활성화 예산 소진(5회
    /// 핑퐁) → 해체 → `OutOfSync` → 폴링 → `Granted`" 저속 순환이 생긴다(시스템을
    /// 멈추지는 않지만 스펙 위반이자 로그 소음·불필요한 탭 생성 반복이다).
    /// `trusted == false` 는 이 가드에서 제외한다 — 권한을 껐다 켜는 것이 곧 §3.3
    /// 수동 복구 절차 (b)의 시작이므로 `Denied` 로의 전이는 열어 둔다.
    ///
    /// ⭐ **이슈 #149 D1 — 백오프 도달 시에만 가드를 연다.** `OutOfSync` 에서
    /// `trusted == true` 가 `last_out_of_sync_ms + backoff(attempts)` 에 도달했고
    /// `attempts < 3` 이면 `attempts += 1` 하고 `OutOfSync → Granted` 전이를
    /// 반환한다(반환된 전이는 기존 배선 그대로 완전히 새 `Engine` 을 탄다 — #65 가
    /// 금지한 엔진 내부 재생성이 아니라 권한 모니터의 `Granted` 재확인 경로이므로
    /// 금지 위반이 아니다). **백오프 미도달 통과는 금지**한다 — 미도달이면 `None` 으로
    /// 막아 PR #66 이 기각한 저속 순환이 재발하지 않게 한다. `now_ms` 는 호출자가
    /// `Instant` baseline 으로 계산해 주입하는 monotonic ms 다.
    pub fn observe_trusted(&mut self, trusted: bool, now_ms: u64) -> Option<PermissionTransition> {
        if !trusted {
            // ⭐ 이슈 #149 D1 — 권한을 껐다 켜는 것이 §3.3 수동 복구 절차 (b)의
            // 시작이므로 백오프 상태를 리셋한 뒤 기존 규칙대로 `Denied` 전이를
            // 연다(PR #66 가드 예외 유지).
            self.out_of_sync_attempts = 0;
            self.last_out_of_sync_ms = None;
            return self.move_to(PermissionState::Denied);
        }
        if self.state == PermissionState::OutOfSync {
            let due = self.out_of_sync_attempts < OUT_OF_SYNC_RETRY_MAX_ATTEMPTS
                && self.last_out_of_sync_ms.is_some_and(|entered_ms| {
                    now_ms.saturating_sub(entered_ms)
                        >= OUT_OF_SYNC_RETRY_BACKOFF_MS[self.out_of_sync_attempts as usize]
                });
            if !due {
                return None;
            }
            self.out_of_sync_attempts += 1;
            return self.move_to(PermissionState::Granted);
        }
        self.move_to(PermissionState::Granted)
    }

    /// ⭐ F-07 이 "탭 생성 실패" 를 알려주는 지점. §3.3 항목 1 의 판정 조건:
    /// `trusted == true` 인데 탭 생성 실패 → `OutOfSync`.
    ///
    /// `trusted == false` 일 때 탭 생성이 실패하는 것은 정상이다 — 권한이 아예
    /// 없어서 실패한 것이므로 `OutOfSync`(권한 DB 불일치)가 아니라 그냥
    /// `Denied` 로 간다. 이 구분은 `key-remapping-engine.md` §3-a 가 요구한다.
    ///
    /// ⭐ 이슈 #149 D1 — `OutOfSync` 진입(같은 상태 재진입 포함) 시각을
    /// `last_out_of_sync_ms` 에 갱신해 다음 백오프 단계를 탄다. 재시도가
    /// `NotTrusted`/예산 소진으로 다시 `OutOfSync` 로 복귀하는 경우가 이 경로로
    /// 온다. `now_ms` 는 `observe_trusted` 와 같은 monotonic ms 다.
    pub fn observe_tap_create_failed(
        &mut self,
        trusted: bool,
        now_ms: u64,
    ) -> Option<PermissionTransition> {
        let target = if trusted {
            PermissionState::OutOfSync
        } else {
            PermissionState::Denied
        };
        let transition = self.move_to(target);
        if trusted {
            self.last_out_of_sync_ms = Some(now_ms);
        }
        transition
    }

    /// 탭 생성에 성공했다 — `OutOfSync` 진단에서 회복되는 경로(§2 S4 항목 5).
    /// ⭐ **이슈 #149 D1 — 백오프 도달(`observe_trusted`)과 함께 탈출구 둘 중
    /// 하나다**(그전에는 이 함수가 유일했다 — PR #66 가드가 폴링 복귀를 막았으므로).
    /// 성공은 회복 증거이므로 백오프 상태(`attempts`·진입 시각)를 리셋한다.
    pub fn observe_tap_created(&mut self) -> Option<PermissionTransition> {
        self.out_of_sync_attempts = 0;
        self.last_out_of_sync_ms = None;
        self.move_to(PermissionState::Granted)
    }

    /// 현재 상태에서 써야 할 폴링 주기(§3.4 이원화). `Granted` 상태만 배경 감시로
    /// 완화하고, 그 외(아직 온보딩 중이거나 문제를 진단해야 하는 상태)는 전부
    /// 빠른 주기를 유지한다.
    pub fn poll_mode(&self) -> PollMode {
        match self.state {
            PermissionState::Granted => PollMode::Background,
            _ => PollMode::Onboarding,
        }
    }
}

impl Default for PermissionModel {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_to_denied_on_first_untrusted_observation() {
        let mut model = PermissionModel::new();
        assert_eq!(model.state(), PermissionState::Unknown);

        let transition = model.observe_trusted(false, 0);
        assert_eq!(
            transition,
            Some(PermissionTransition {
                from: PermissionState::Unknown,
                to: PermissionState::Denied,
            })
        );
        assert_eq!(model.state(), PermissionState::Denied);
    }

    #[test]
    fn denied_to_granted_when_trusted_becomes_true() {
        let mut model = PermissionModel::new();
        model.observe_trusted(false, 0);

        let transition = model.observe_trusted(true, 0);
        assert_eq!(
            transition,
            Some(PermissionTransition {
                from: PermissionState::Denied,
                to: PermissionState::Granted,
            })
        );
        assert_eq!(model.state(), PermissionState::Granted);
    }

    /// ⭐ §2 S3 — 런타임 중 시스템 설정에서 Accessibility 를 끄면 다음 폴링에서
    /// 즉시 감지되어야 한다.
    #[test]
    fn granted_to_denied_on_runtime_revocation() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);
        assert_eq!(model.state(), PermissionState::Granted);

        let transition = model.observe_trusted(false, 0);
        assert_eq!(
            transition,
            Some(PermissionTransition {
                from: PermissionState::Granted,
                to: PermissionState::Denied,
            })
        );
        assert_eq!(model.state(), PermissionState::Denied);
    }

    #[test]
    fn repeated_same_observation_does_not_re_emit_transition() {
        let mut model = PermissionModel::new();
        assert!(model.observe_trusted(true, 0).is_some());
        assert_eq!(model.observe_trusted(true, 0), None);
        assert_eq!(model.observe_trusted(true, 0), None);
        assert_eq!(model.state(), PermissionState::Granted);
    }

    /// ⭐ §3.3 판정 조건 — `trusted == true` 인데 탭 생성이 실패하면 `OutOfSync`.
    #[test]
    fn tap_create_failed_while_trusted_is_out_of_sync() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);

        let transition = model.observe_tap_create_failed(true, 0);
        assert_eq!(
            transition,
            Some(PermissionTransition {
                from: PermissionState::Granted,
                to: PermissionState::OutOfSync,
            })
        );
        assert_eq!(model.state(), PermissionState::OutOfSync);
    }

    /// ⭐ 권한이 아예 없어서 탭 생성이 실패한 경우는 `OutOfSync` 가 아니다 —
    /// `key-remapping-engine.md` §3-a 가 두 실패 원인을 구분하라고 요구한다.
    #[test]
    fn tap_create_failed_while_untrusted_is_just_denied() {
        let mut model = PermissionModel::new();

        let transition = model.observe_tap_create_failed(false, 0);
        assert_eq!(
            transition,
            Some(PermissionTransition {
                from: PermissionState::Unknown,
                to: PermissionState::Denied,
            })
        );
        assert_eq!(model.state(), PermissionState::Denied);
        assert_ne!(model.state(), PermissionState::OutOfSync);
    }

    #[test]
    fn tap_created_recovers_out_of_sync_to_granted() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);
        model.observe_tap_create_failed(true, 0);
        assert_eq!(model.state(), PermissionState::OutOfSync);

        let transition = model.observe_tap_created();
        assert_eq!(
            transition,
            Some(PermissionTransition {
                from: PermissionState::OutOfSync,
                to: PermissionState::Granted,
            })
        );
        assert_eq!(model.state(), PermissionState::Granted);
    }

    /// ⭐ PR #66 검수 반영 — 폴링(`observe_trusted(true)`)만으로는 `OutOfSync` 를
    /// 벗어나지 못하고 그 자리에 머문다(sticky, §2 S4·§3.3).
    #[test]
    fn out_of_sync_stays_put_on_trusted_polling() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);
        model.observe_tap_create_failed(true, 0);
        assert_eq!(model.state(), PermissionState::OutOfSync);

        assert_eq!(model.observe_trusted(true, 0), None);
        assert_eq!(model.state(), PermissionState::OutOfSync);
    }

    /// ⭐ PR #66 검수 반영 — `trusted == false` 는 `OutOfSync` 가드에서 제외된다.
    /// 권한을 껐다 켜는 것이 §3.3 수동 복구 절차 (b)의 시작이므로, `OutOfSync` →
    /// `Denied` → `Granted` 왕복은 정상적으로 열려 있어야 한다.
    #[test]
    fn out_of_sync_can_still_be_escaped_by_toggling_permission_off_and_on() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);
        model.observe_tap_create_failed(true, 0);
        assert_eq!(model.state(), PermissionState::OutOfSync);

        let denied = model.observe_trusted(false, 0);
        assert_eq!(
            denied,
            Some(PermissionTransition {
                from: PermissionState::OutOfSync,
                to: PermissionState::Denied,
            })
        );
        assert_eq!(model.state(), PermissionState::Denied);

        let granted = model.observe_trusted(true, 0);
        assert_eq!(
            granted,
            Some(PermissionTransition {
                from: PermissionState::Denied,
                to: PermissionState::Granted,
            })
        );
        assert_eq!(model.state(), PermissionState::Granted);
    }

    #[test]
    fn poll_mode_is_background_only_when_granted() {
        let mut model = PermissionModel::new();
        assert_eq!(model.poll_mode(), PollMode::Onboarding);

        model.observe_trusted(false, 0);
        assert_eq!(model.poll_mode(), PollMode::Onboarding);

        model.observe_trusted(true, 0);
        assert_eq!(model.poll_mode(), PollMode::Background);

        model.observe_tap_create_failed(true, 0);
        assert_eq!(model.poll_mode(), PollMode::Onboarding);
    }

    /// ⭐ 이슈 #149 D1 — `OutOfSync` 에서 백오프 미도달 시각 → `None`(PR #66 가드 유지).
    #[test]
    fn out_of_sync_backoff_not_yet_due_stays_put() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);
        model.observe_tap_create_failed(true, 1_000);
        assert_eq!(model.state(), PermissionState::OutOfSync);

        // 1단계 백오프 30_000ms 미도달(29_999ms) → 차단.
        assert_eq!(model.observe_trusted(true, 1_000 + 29_999), None);
        assert_eq!(model.state(), PermissionState::OutOfSync);
    }

    /// ⭐ 이슈 #149 D1 — 백오프 도달 → `Some(OutOfSync→Granted)` + `attempts` 증가.
    #[test]
    fn out_of_sync_backoff_due_releases_one_retry() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);
        model.observe_tap_create_failed(true, 1_000);
        assert_eq!(model.state(), PermissionState::OutOfSync);

        let transition = model.observe_trusted(true, 1_000 + 30_000);
        assert_eq!(
            transition,
            Some(PermissionTransition {
                from: PermissionState::OutOfSync,
                to: PermissionState::Granted,
            })
        );
        assert_eq!(model.state(), PermissionState::Granted);
    }

    /// ⭐ 이슈 #149 D1 — 3회 재시도 뒤 4회째는 `None`(상한). 백오프 단계는
    /// [30초, 2분, 10분] 으로 다음 진입 시각마다 갱신된다.
    #[test]
    fn out_of_sync_retry_capped_at_three_attempts() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);
        // 1차: 진입 0 → 30초 뒤 재시도.
        model.observe_tap_create_failed(true, 0);
        assert!(model.observe_trusted(true, 30_000).is_some());
        // 2차: 재진입 30_000 → +120초 뒤 재시도.
        model.observe_tap_create_failed(true, 30_000);
        assert_eq!(model.state(), PermissionState::OutOfSync);
        assert_eq!(model.observe_trusted(true, 30_000 + 119_999), None);
        assert!(model.observe_trusted(true, 30_000 + 120_000).is_some());
        // 3차: 재진입 150_000 → +600초 뒤 재시도.
        model.observe_tap_create_failed(true, 150_000);
        assert_eq!(model.observe_trusted(true, 150_000 + 599_999), None);
        assert!(model.observe_trusted(true, 150_000 + 600_000).is_some());
        // 4회째: 상한 소진 → 아무리 시간이 지나도 `None`.
        model.observe_tap_create_failed(true, 750_000);
        assert_eq!(model.observe_trusted(true, 750_000 + 600_000), None);
        assert_eq!(model.observe_trusted(true, 750_000 + 3_600_000), None);
        assert_eq!(model.state(), PermissionState::OutOfSync);
    }

    /// ⭐ 이슈 #149 D1 — `observe_tap_created` 후 attempts 0 리셋: 다음 `OutOfSync`
    /// 진입은 다시 1단계(30초) 백오프를 탄다.
    #[test]
    fn tap_created_resets_retry_attempts() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);
        model.observe_tap_create_failed(true, 0);
        assert!(model.observe_trusted(true, 30_000).is_some());
        model.observe_tap_created();
        assert_eq!(model.state(), PermissionState::Granted);

        // 새 `OutOfSync` 진입 → 30초 백오프(2단계가 아닌 1단계).
        model.observe_tap_create_failed(true, 100_000);
        assert_eq!(model.observe_trusted(true, 100_000 + 29_999), None);
        assert!(model.observe_trusted(true, 100_000 + 30_000).is_some());
    }

    /// ⭐ 이슈 #149 D1 — `trusted == false` 관측 시 attempts 0 리셋 + `Denied` 전이 열림.
    #[test]
    fn untrusted_observation_resets_retry_attempts() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);
        model.observe_tap_create_failed(true, 0);
        assert!(model.observe_trusted(true, 30_000).is_some());
        // 재시도 1회 소비 뒤 권한 회수 → `Denied` 로 전이하고 카운터 리셋.
        model.observe_tap_create_failed(true, 30_000);
        let denied = model.observe_trusted(false, 40_000);
        assert_eq!(
            denied,
            Some(PermissionTransition {
                from: PermissionState::OutOfSync,
                to: PermissionState::Denied,
            })
        );
        // 다시 부여 → `Granted`, 다음 `OutOfSync` 는 1단계 백오프.
        assert!(model.observe_trusted(true, 40_000).is_some());
        model.observe_tap_create_failed(true, 50_000);
        assert_eq!(model.observe_trusted(true, 50_000 + 29_999), None);
        assert!(model.observe_trusted(true, 50_000 + 30_000).is_some());
    }

    /// ⭐ 이슈 #149 D1 — 같은 상태 재진입도 진입 시각을 갱신해 다음 백오프 단계를 탄다.
    #[test]
    fn reentering_out_of_sync_restarts_the_backoff_clock() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true, 0);
        model.observe_tap_create_failed(true, 0);
        // 재진입(전이 없음 → `None`)이어도 시각은 갱신된다.
        assert_eq!(model.observe_tap_create_failed(true, 10_000), None);
        assert_eq!(model.state(), PermissionState::OutOfSync);
        assert_eq!(model.observe_trusted(true, 10_000 + 29_999), None);
        assert!(model.observe_trusted(true, 10_000 + 30_000).is_some());
    }
}
