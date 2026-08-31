//! 라이선스 상태 머신의 상태 타입 — 명세 §3.2.

/// 라이선스 상태 머신의 상태(명세 §3.2 표).
///
/// ⭐ **체험과 라이선스는 동일한 하나의 상태 값**이다(§3.1) — `is_trial`/`is_licensed`
/// 두 불리언으로 쪼개지 않는다. 그래야 "체험도 만료되고 라이선스도 무효"인 상태와
/// "체험 중"인 상태를 상태 머신 밖에서 구분하는 로직이 생기지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LicenseState {
    /// 로컬에 체험 기산점도, 라이선스 캐시도 없음(최초 실행 직후). 내부 경유 상태 —
    /// 같은 실행 안에서 기산점을 기록하고 곧바로 `Trial` 로 전이한다(§3.2).
    NotStarted,
    /// `현재 시각 − 기산점 < 20일` 이고 유효한 라이선스 캐시가 없음.
    Trial,
    /// `현재 시각 − 기산점 ≥ 20일`, 또는 라이선스 무효/취소에서 전이.
    TrialExpired,
    /// 활성화 성공, 또는 오프라인 유예 안의 유효 캐시. 체험 상태와 무관하게 최우선.
    Licensed,
    /// 검증이 명시적 무효(refunded/revoked/invalid)를 응답, 또는 유예 만료 후 재검증
    /// 계속 실패. 다음 판정 사이클에서 `TrialExpired` 로 전이(체험 중 복귀 금지).
    Invalid,
}

impl LicenseState {
    /// 체험이 여전히 쓰이고 있는지(사용자에게 "체험판 — N일 남음" 표시 여부).
    pub fn is_trial_active(self) -> bool {
        self == LicenseState::Trial
    }

    /// 라이선스가 활성 상태인지.
    pub fn is_licensed(self) -> bool {
        self == LicenseState::Licensed
    }

    /// 이 상태에서 기능을 제한해야 하는지. 제품 동작상 Licensed 만 무제한이고,
    /// 나머지(NotStarted·Trial·TrialExpired·Invalid)는 모두 "풀기능 아님" 경계다.
    pub fn restricts_features(self) -> bool {
        !self.is_licensed()
    }

    /// 상태의 안정적 영어 이름 — 로그 전용(명세 §3.1 "로그는 항상 영어").
    pub fn as_log_str(self) -> &'static str {
        match self {
            LicenseState::NotStarted => "not_started",
            LicenseState::Trial => "trial",
            LicenseState::TrialExpired => "trial_expired",
            LicenseState::Licensed => "licensed",
            LicenseState::Invalid => "invalid",
        }
    }
}
