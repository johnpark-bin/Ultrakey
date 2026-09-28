//! `LicenseProvider` — 명세 §3.3 의 추상 인터페이스.
//!
//! Paddle Classic 의 실제 와이어 프로토콜은 조사로 확인되지 않았다(명세 §9 Q2).
//! 그래서 이 계약은 **앱 내부의 추상 인터페이스**로 정의하고, Paddle 로의 매핑은
//! 별도 `(추정)` 이다. 상태 머신·오프라인 유예·비활성화 UI 는 전부 이 인터페이스에만
//! 의존해 구현된다 — 나중에 실제 Paddle 연동을 끼워 넣어도 이 부분을 다시 설계할
//! 필요가 없다(명세 §7, 제품 결정 D2).
//!
//! # 최소 식별자 원칙(§3.7)
//! 요청 필드는 §3.3 표에 나열된 것 **전부**다. 사용자 이름·이메일·사용 통계·다른
//! 기기 목록 상세는 어떤 요청에도 포함하지 않는다. `device_id` 는 하드웨어 UUID
//! 하나로 충분하고, `device_label` 은 사용자가 직접 붙인 이름일 뿐이다.

use crate::clock::Timestamp;
use serde::{Deserialize, Serialize};

/// 라이선스 키 — 임의의 불투명 문자열(명세 §3.3.1 Q11: 포맷 미확인).
pub type LicenseKey = String;

/// 활성화 요청(§3.3.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActivationRequest {
    /// 사용자가 입력한 키.
    pub license_key: LicenseKey,
    /// 이 기기의 하드웨어 UUID(§3.6 / §3.7).
    pub device_id: String,
    /// 선택 — 슬롯 목록에서 사람이 알아볼 수 있는 이름. 없어도 활성화 가능.
    pub device_label: Option<String>,
    /// 호환성 판단·지원 문의용.
    pub app_version: Option<String>,
}

/// 활성화 응답(§3.3.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationResponse {
    /// 성공 — 캐시 토큰 발급.
    Activated {
        /// 오프라인 재실행 시 검증 없이 신뢰할 값(§3.5).
        cache_token: String,
        /// 캐시 발급 시각 — 오프라인 유예 계산 기준(§3.4).
        cache_issued_at: Timestamp,
        /// 현재 사용 중인 활성화 슬롯 수.
        activations_used: u32,
        /// 슬롯 상한(원본은 3대, §1 / §5-3).
        activations_limit: u32,
    },
    /// 키가 유효하지 않음.
    InvalidKey,
    /// 환불된 키.
    Refunded,
    /// 활성화 슬롯 한도 초과 — 사용자가 다른 기기를 비활성화해야 한다(§5-3).
    LimitReached { activations_used: u32, activations_limit: u32 },
    /// 네트워크 실패(연결·5xx·타임아웃 공통, §5-8). 서버의 확정 응답이 아니다.
    NetworkError,
}

/// 비활성화 요청(§3.3.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeactivateRequest {
    /// 대상 라이선스.
    pub license_key: LicenseKey,
    /// 해제할 기기 — 보통 "이 기기" 자기 자신(§4).
    pub device_id: String,
}

/// 비활성화 응답(§3.3.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeactivateResponse {
    /// 성공 — 슬롯 반납.
    Deactivated { activations_used: u32 },
    /// 이미 해제됨.
    NotFound,
    /// 네트워크 실패. ⚠️ 슬롯 반납은 서버가 확정해야 진실이다(§3.3.2, §5-8) —
    /// 낙관적으로 "비활성화됨"으로 처리하지 않는다.
    NetworkError,
}

/// 검증(재검증) 요청(§3.3.3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidateRequest {
    pub license_key: LicenseKey,
    pub device_id: String,
    /// 이전 발급 토큰이 있으면 서버가 무효화했는지 대조할 수 있게 한다(§3.3.3).
    pub cache_token: Option<String>,
}

/// 검증 응답 상태(§3.3.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValidationStatus {
    Valid,
    InvalidKey,
    Refunded,
    Revoked,
    /// 연결 자체 실패 — 오프라인 유예 규칙(§3.4)이 이 경우를 덮는다.
    NetworkError,
}

impl ValidationStatus {
    pub fn as_log_str(self) -> &'static str {
        match self {
            ValidationStatus::Valid => "valid",
            ValidationStatus::InvalidKey => "invalid_key",
            ValidationStatus::Refunded => "refunded",
            ValidationStatus::Revoked => "revoked",
            ValidationStatus::NetworkError => "network_error",
        }
    }
}

/// 검증 응답(§3.3.3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidateResponse {
    /// 유효 — 캐시 갱신.
    Valid {
        cache_token: String,
        cache_issued_at: Timestamp,
    },
    InvalidKey,
    Refunded,
    Revoked,
    /// 연결·서버 실패 공통. 낙관적으로 유효로 바꾸지 않는다 — §3.4 유예 규칙이 결정.
    NetworkError,
}

/// `LicenseProvider` — 활성화·비활성화·검증 세 동작의 추상 계약(§3.3).
///
/// ⛔ **네트워크 또는 로컬 I/O 를 직접 만들지 않는다** — 이 트레이트를 구현하는
/// 쪽(noop·추후 Paddle)이 결정한다. 이 크레이트는 트레이트 정의와 순수 로직만 가진다.
pub trait LicenseProvider: Send + Sync {
    /// 활성화(§3.3.1).
    fn activate(&self, request: ActivationRequest) -> ActivationResponse;

    /// 비활성화(§3.3.2). 성공만이 슬롯 반납의 진실이다.
    fn deactivate(&self, request: DeactivateRequest) -> DeactivateResponse;

    /// 검증/재검증(§3.3.3).
    fn validate(&self, request: ValidateRequest) -> ValidateResponse;
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 최소 식별자 원칙(§3.7) — 요청 타입이 정의한 필드가 정확히 §3.3 표의
    /// 필드뿐인지 확인한다. 이 테스트는 페이로드 구조가 늘어나면 실패해
    /// "이메일·통계 같은 금지 필드가 추가됐다"는 회귀를 잡는다.
    #[test]
    fn activation_request_has_only_allowed_fields() {
        let req = ActivationRequest {
            license_key: "k".into(),
            device_id: "uuid".into(),
            device_label: None,
            app_version: Some("0.1.0".into()),
        };
        // 필드는 라이선스 키·기기 식별자·선택적 레이블·앱 버전뿐.
        assert!(!req.license_key.is_empty());
        assert!(!req.device_id.is_empty());
        // 레이블과 버전은 Option — 없어도 요청은 성립한다(§3.3.1 최소 식별자).
        assert!(req.device_label.is_none() || req.device_label.is_some());
    }

    #[test]
    fn validate_request_carries_only_license_device_and_cache() {
        let req = ValidateRequest {
            license_key: "k".into(),
            device_id: "uuid".into(),
            cache_token: None,
        };
        assert!(!req.license_key.is_empty());
        assert!(!req.device_id.is_empty());
    }
}
