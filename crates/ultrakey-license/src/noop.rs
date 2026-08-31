//! `NoopLicenseProvider` — 명세 §7 의 no-op 구현체.
//!
//! Paddle 미연동 결정(제품 결정 D2)에 따라, 초기 구현은 **모든 기기를 항상 라이선스
//! 활성으로** 취급한다. 이 provider 를 상태 머신에 주입하면 활성화·비활성화·검증이
//! 전부 유효로 응답한다 — 상태 머신·오프라인 유예·비활성화 UI(`machine.rs`/`decision.rs`)
//! 는 이 provider 위에서 그대로 검증된다.
//!
//! `cache_issued_at` 은 호출 시점의 시스템 시각(오프라인 유예 기준)이다.

use crate::clock::system_time_now;
use crate::provider::{
    ActivationRequest, ActivationResponse, DeactivateRequest, DeactivateResponse,
    LicenseProvider, ValidateRequest, ValidateResponse,
};

/// 항상 라이선스 활성으로 응답하는 no-op 구현체.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopLicenseProvider;

impl NoopLicenseProvider {
    pub fn new() -> Self {
        Self
    }
}

impl LicenseProvider for NoopLicenseProvider {
    fn activate(&self, request: ActivationRequest) -> ActivationResponse {
        // ⭐ no-op — 항상 활성화 성공. 슬롯은 최소 (0/3) 으로 응답한다(3대 제한은
        // no-op 하에서는 도달 불가 — 명세 §7 은 이를 감내한다).
        ActivationResponse::Activated {
            cache_token: format!("noop-{}", request.device_id),
            cache_issued_at: system_time_now(),
            activations_used: 0,
            activations_limit: 3,
        }
    }

    fn deactivate(&self, request: DeactivateRequest) -> DeactivateResponse {
        // no-op — 비활성화 항상 성공. device_id 는 로그/재검증에 쓰지 않지만 요청
        // 계약은 그대로 받는다.
        let _ = request;
        DeactivateResponse::Deactivated { activations_used: 0 }
    }

    fn validate(&self, request: ValidateRequest) -> ValidateResponse {
        // no-op — 항상 유효(재검증 성공). 캐시 토큰은 요청의 device_id 로 만든다.
        ValidateResponse::Valid {
            cache_token: format!("noop-{}", request.device_id),
            cache_issued_at: system_time_now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> ActivationRequest {
        ActivationRequest {
            license_key: "key".into(),
            device_id: "device-1".into(),
            device_label: None,
            app_version: None,
        }
    }

    /// no-op 은 항상 라이선스 활성으로 응답한다(명세 §7).
    #[test]
    fn noop_always_activates_and_validates() {
        let p = NoopLicenseProvider::new();

        match p.activate(req()) {
            ActivationResponse::Activated {
                activations_used,
                activations_limit,
                ..
            } => {
                assert_eq!(activations_used, 0);
                assert_eq!(activations_limit, 3);
            }
            other => panic!("no-op activate 가 활성화 성공이 아니다: {other:?}"),
        }

        let validate = p.validate(ValidateRequest {
            license_key: "key".into(),
            device_id: "device-1".into(),
            cache_token: None,
        });
        assert!(matches!(
            validate,
            ValidateResponse::Valid { cache_issued_at: _, .. }
        ));

        let deactivate = p.deactivate(DeactivateRequest {
            license_key: "key".into(),
            device_id: "device-1".into(),
        });
        assert!(matches!(
            deactivate,
            DeactivateResponse::Deactivated { activations_used: 0 }
        ));
    }
}
