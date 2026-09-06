//! 스레드 QoS(Quality of Service) 승격 — 이슈 #139 D2.
//!
//! **왜 필요한가**: 이슈 #139 F2 — 이 앱의 이벤트 탭이 **활성 필터 탭**(session
//! event tap, `event_tap.rs`)이면 WindowServer 는 그 탭이 붙어 있는 스레드가
//! 콜백에서 돌아올 때까지 **동기로 기다린다**. 즉 탭 스레드의 OS 스케줄링
//! 지연은 곧바로 사용자가 체감하는 입력 지연으로 이어진다 — 시스템이 다른
//! 작업으로 바쁠 때 탭 스레드가 기본 QoS(`QOS_CLASS_DEFAULT` 부근)로 뒷전에
//! 밀리면, 키 입력 하나가 화면에 반영되기까지의 시간이 그만큼 늘어난다.
//!
//! **왜 QoS 이고 `thread_policy_set(TIME_CONSTRAINT)` 가 아닌가**
//! (`docs/plan/issue-139-input-latency.md` D2 기각 사유): 실시간 스레드 정책은
//! 커널에 "이 스레드는 주기적으로 짧게 깨어나 마감을 지킨다"는 구체적인 시간
//! 파라미터(period·computation·constraint)를 약속하는 것이라, 잘못 튜닝하면
//! 시스템 전체의 스케줄링을 오히려 해칠 수 있고 오남용 부담이 크다. QoS
//! 승격은 애플이 권장하는 표준 메커니즘이고, 커널이 이미 `WindowServer` 와
//! 상호작용하는 스레드들에 대해 QoS 기반 우선순위 상속·부스팅을 알아서
//! 처리하도록 설계돼 있다 — 이 모듈은 그 표준 경로에 올라타기만 한다.
//!
//! **실패는 치명적이지 않다.** `set_current_thread_user_interactive` 가
//! 실패해도(음수 반환) 탭 스레드는 계속 기본 QoS 로 동작한다 — 지연이
//! 조금 더 클 뿐 기능 자체는 그대로다. 그래서 이 모듈은 `Result`/`Option`
//! 으로만 실패를 알리고, 호출부(`ultrakey-engine`)가 로그만 남기고 계속 진행한다.

#[cfg(target_os = "macos")]
mod macos_impl {
    use crate::ffi::{
        self, pthread_get_qos_class_np, pthread_self, pthread_set_qos_class_self_np, qos_class_t,
    };
    use std::ffi::c_int;

    /// 이 스레드에서 관측한 QoS 등급. `sys/qos.h` 의 `qos_class_t` 값을 그대로
    /// 옮긴 것 — `Other` 는 알려진 상수와 일치하지 않는 값(있을 수 없지만
    /// 방어적으로 열어 둔다)을 담는다.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum QosClass {
        UserInteractive,
        UserInitiated,
        Default,
        Utility,
        Background,
        Unspecified,
        Other(u32),
    }

    impl QosClass {
        fn from_raw(raw: qos_class_t) -> Self {
            match raw {
                ffi::QOS_CLASS_USER_INTERACTIVE => Self::UserInteractive,
                ffi::QOS_CLASS_USER_INITIATED => Self::UserInitiated,
                ffi::QOS_CLASS_DEFAULT => Self::Default,
                ffi::QOS_CLASS_UTILITY => Self::Utility,
                ffi::QOS_CLASS_BACKGROUND => Self::Background,
                ffi::QOS_CLASS_UNSPECIFIED => Self::Unspecified,
                other => Self::Other(other),
            }
        }
    }

    /// 현재 스레드의 QoS 를 `QOS_CLASS_USER_INTERACTIVE` 로 승격한다.
    ///
    /// `relative_priority` 는 0 으로 고정한다 — QoS 등급 안의 상대 우선순위
    /// 세부 조정은 이 앱의 용도(탭 스레드가 `USER_INTERACTIVE` 등급 안에
    /// 있기만 하면 된다)에서 불필요하다.
    ///
    /// 반환값이 `Err(rc)` 면 `rc` 는 `pthread_set_qos_class_self_np` 가 돌려준
    /// 0 이 아닌 값(POSIX 에러 번호)이다 — 실패해도 스레드는 이전 QoS 로 계속
    /// 동작하므로 호출부는 로그만 남기고 진행하면 된다.
    pub fn set_current_thread_user_interactive() -> Result<(), i32> {
        // SAFETY: `pthread_set_qos_class_self_np` 는 항상 현재 스레드
        // (호출 스레드) 자신에게만 작용하는 non-throwing C 함수다. 인자는
        // `sys/qos.h` 가 정의한 상수와 리터럴 0 뿐이라 널 포인터나 잘못된
        // 핸들을 넘길 여지가 없다.
        let rc: c_int =
            unsafe { pthread_set_qos_class_self_np(ffi::QOS_CLASS_USER_INTERACTIVE, 0) };
        if rc == 0 {
            Ok(())
        } else {
            Err(rc as i32)
        }
    }

    /// 현재 스레드의 QoS 등급을 되읽는다. 실패하면 `None`.
    pub fn current_thread_qos_class() -> Option<QosClass> {
        let mut cls: qos_class_t = ffi::QOS_CLASS_UNSPECIFIED;
        let mut relative_priority: c_int = 0;
        // SAFETY: `pthread_self()` 는 항상 유효한 현재 스레드 핸들을 돌려준다.
        // `cls`/`relative_priority` 는 스택에 살아 있는 지역 변수에 대한
        // 유효한 포인터이고, `pthread_get_qos_class_np` 는 실패 시(rc != 0)
        // 이들을 건드리지 않을 수 있으므로 호출 전에 안전한 기본값으로
        // 초기화해 둔다.
        let rc: c_int =
            unsafe { pthread_get_qos_class_np(pthread_self(), &mut cls, &mut relative_priority) };
        if rc == 0 {
            Some(QosClass::from_raw(cls))
        } else {
            None
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{current_thread_qos_class, set_current_thread_user_interactive, QosClass};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    /// 비-macOS 스텁 — macOS 쪽과 이름·베리언트를 맞춰 둔다(컴파일만 되면 된다).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum QosClass {
        UserInteractive,
        UserInitiated,
        Default,
        Utility,
        Background,
        Unspecified,
        Other(u32),
    }

    pub fn set_current_thread_user_interactive() -> Result<(), i32> {
        Err(-1)
    }

    pub fn current_thread_qos_class() -> Option<QosClass> {
        None
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{current_thread_qos_class, set_current_thread_user_interactive, QosClass};

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn promotes_a_fresh_thread_to_user_interactive() {
        let handle = std::thread::spawn(|| {
            let result = set_current_thread_user_interactive();
            let observed = current_thread_qos_class();
            (result, observed)
        });
        let (result, observed) = handle.join().expect("스레드 join 실패");
        assert_eq!(result, Ok(()));
        assert_eq!(observed, Some(QosClass::UserInteractive));
    }

    #[test]
    fn default_thread_reports_some_qos_class() {
        // 승격을 호출하지 않은 새 스레드도 어떤 QoS 등급이든 되읽을 수 있어야
        // 한다 — `pthread_get_qos_class_np` 자체가 실패하지 않는지 확인.
        let handle = std::thread::spawn(current_thread_qos_class);
        let observed = handle.join().expect("스레드 join 실패");
        assert!(observed.is_some());
    }
}
