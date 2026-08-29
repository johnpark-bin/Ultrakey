//! Secure Input(암호 입력 필드 포커스) 상태 확인.
//!
//! ⭐ **트레이트 뒤에 감춘 이유** — `key-remapping-engine.md` §9 #12 가 "매
//! 콜백마다 확인할지, 별도 주기로 폴링할지"를 `(미확정)` 으로 남겼다.
//! 지금은 "매 콜백 확인"으로 가되(§9 의 전제 — 오버헤드가 작은 syscall 이라는
//! 가정), 나중에 실측으로 오버헤드가 크다고 판명되면 짧은 주기로 캐시하는
//! 구현체로 갈아끼울 수 있도록 교체 지점을 이 트레이트 하나로 모은다.
//! 호출자(엔진)는 항상 `&dyn SecureInputProbe` 로만 이 값을 받아써야 한다.

/// Secure Input 활성 여부를 확인하는 방법을 추상화한다.
pub trait SecureInputProbe: Send + Sync {
    fn is_enabled(&self) -> bool;
}

/// 매 호출마다 `IsSecureEventInputEnabled()` 를 직접 부르는 1차 구현체.
pub struct DirectSecureInputProbe;

#[cfg(target_os = "macos")]
impl SecureInputProbe for DirectSecureInputProbe {
    fn is_enabled(&self) -> bool {
        // SAFETY: `IsSecureEventInputEnabled` 는 인자가 없고 부작용도 없는
        // (순수 조회) 함수다 — 매 호출이 항상 안전하다.
        unsafe { crate::ffi::IsSecureEventInputEnabled() != 0 }
    }
}

#[cfg(not(target_os = "macos"))]
impl SecureInputProbe for DirectSecureInputProbe {
    fn is_enabled(&self) -> bool {
        false
    }
}
