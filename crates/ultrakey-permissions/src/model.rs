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
}

impl PermissionModel {
    pub fn new() -> Self {
        PermissionModel {
            state: PermissionState::Unknown,
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
    pub fn observe_trusted(&mut self, trusted: bool) -> Option<PermissionTransition> {
        let target = if trusted {
            PermissionState::Granted
        } else {
            PermissionState::Denied
        };
        self.move_to(target)
    }

    /// ⭐ F-07 이 "탭 생성 실패" 를 알려주는 지점. §3.3 항목 1 의 판정 조건:
    /// `trusted == true` 인데 탭 생성 실패 → `OutOfSync`.
    ///
    /// `trusted == false` 일 때 탭 생성이 실패하는 것은 정상이다 — 권한이 아예
    /// 없어서 실패한 것이므로 `OutOfSync`(권한 DB 불일치)가 아니라 그냥
    /// `Denied` 로 간다. 이 구분은 `key-remapping-engine.md` §3-a 가 요구한다.
    pub fn observe_tap_create_failed(&mut self, trusted: bool) -> Option<PermissionTransition> {
        let target = if trusted {
            PermissionState::OutOfSync
        } else {
            PermissionState::Denied
        };
        self.move_to(target)
    }

    /// 탭 생성에 성공했다 — `OutOfSync` 진단에서 회복되는 유일한 경로(§2 S4 항목 5).
    pub fn observe_tap_created(&mut self) -> Option<PermissionTransition> {
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

        let transition = model.observe_trusted(false);
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
        model.observe_trusted(false);

        let transition = model.observe_trusted(true);
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
        model.observe_trusted(true);
        assert_eq!(model.state(), PermissionState::Granted);

        let transition = model.observe_trusted(false);
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
        assert!(model.observe_trusted(true).is_some());
        assert_eq!(model.observe_trusted(true), None);
        assert_eq!(model.observe_trusted(true), None);
        assert_eq!(model.state(), PermissionState::Granted);
    }

    /// ⭐ §3.3 판정 조건 — `trusted == true` 인데 탭 생성이 실패하면 `OutOfSync`.
    #[test]
    fn tap_create_failed_while_trusted_is_out_of_sync() {
        let mut model = PermissionModel::new();
        model.observe_trusted(true);

        let transition = model.observe_tap_create_failed(true);
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

        let transition = model.observe_tap_create_failed(false);
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
        model.observe_trusted(true);
        model.observe_tap_create_failed(true);
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

    #[test]
    fn poll_mode_is_background_only_when_granted() {
        let mut model = PermissionModel::new();
        assert_eq!(model.poll_mode(), PollMode::Onboarding);

        model.observe_trusted(false);
        assert_eq!(model.poll_mode(), PollMode::Onboarding);

        model.observe_trusted(true);
        assert_eq!(model.poll_mode(), PollMode::Background);

        model.observe_tap_create_failed(true);
        assert_eq!(model.poll_mode(), PollMode::Onboarding);
    }
}
