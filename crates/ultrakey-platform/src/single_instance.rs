//! F-10 단일 인스턴스 보장의 재실행 신호(이슈 #68) — 분산 알림 IPC.
//!
//! 두 번째 프로세스가 기존 인스턴스에게 "환경설정 창을 띄워라"라고 알리는
//! 수단은 `NSDistributedNotificationCenter` 다. 근거와 기각한 대안은
//! 이슈 #68 코멘트(Phase 1 설계)에 기록돼 있다. 요약:
//!
//! - 이 저장소는 분산 알림을 이미 쓴다 — `workspace`(화면 잠금)와
//!   `text_input_source`(입력 소스 변경)가 같은 센터를 구독한다. 같은
//!   패턴을 재사용하므로 새 FFI 스타일이 생기지 않는다.
//! - 두 번째 프로세스는 Tauri 를 띄우기 전에 종료해야 한다(F-10 §5 항목 1) —
//!   AppKit 초기화가 거의 필요 없는 이 IPC 가 그 제약에 가장 가볍다.
//! - `tauri-plugin-single-instance` 는 이벤트 루프 기동 후 콜백이 불려
//!   Tauri 를 통째로 띄워야 하고, 기존 1-b 판정과 이중화된다 — 기각.
//!
//! 알림 이름(`SHOW_SETTINGS_NOTIFICATION`)은 번들 ID 접두사로 스코프를
//! 명시한다. 구독은 기존 인스턴스만(`setup()` 에서) 하고, 발행은 두 번째
//! 프로세스만 한다 — 이 모듈의 두 함수가 정확히 그 두 역할이다.

/// "기존 인스턴스가 설정창을 띄워라" 분산 알림의 이름. 번들 ID
/// (`app.ultrakey.Ultrakey`)를 접두사로 써 다른 앱의 알림과 겹치지 않는다.
pub const SHOW_SETTINGS_NOTIFICATION: &str = "app.ultrakey.Ultrakey.showSettings";

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::SHOW_SETTINGS_NOTIFICATION;
    use block2::RcBlock;
    use core::ptr::NonNull;
    use objc2::rc::Retained;
    use objc2::runtime::{NSObjectProtocol, ProtocolObject};
    use objc2_foundation::{NSDistributedNotificationCenter, NSNotification, NSString};

    type Token = Retained<ProtocolObject<dyn NSObjectProtocol>>;

    /// 기존 인스턴스가 살아 있으면 환경설정 창을 띄우라고 분산 알림을 발행한다.
    ///
    /// 두 번째 프로세스가 Tauri 기동 전에 딱 한 번 부른다 — 발행은 소유할
    /// 자원도 남기지 않는 일회성 호출이다. 발행에 실패해도(분산 센터 획득
    /// 실패 등) 오류이지만 복구 경로는 없다 — 어차피 이 프로세스는 곧
    /// 종료하므로 로그만 남긴다(호출자가 반환값을 보고 다르게 할 일이 없다).
    pub fn notify_existing_instance_to_show_settings() {
        let center = NSDistributedNotificationCenter::defaultCenter();
        let name = NSString::from_str(SHOW_SETTINGS_NOTIFICATION);
        // SAFETY: `name` 은 방금 만든 유효한 `NSString` 이다. `object` 는
        // 없음(전역 신호라 수신자가 특정 객체를 걸러 볼 이유가 없다)을
        // 나타내는 nil 이며, `postNotificationName:object:` 는 둘 다
        // nullable 로 선언돼 있다.
        unsafe { center.postNotificationName_object(&name, None) };
    }

    /// 분산 알림을 구독하고 옵저버 토큰을 돌려준다 — 토큰을 소유하는 동안만
    /// 수신된다. 수신 스레드는 알림 센터의 관례(메인 런루프)를 따른다.
    ///
    /// `workspace.rs` 의 `SystemEventObserver` 와 같은 block 기반 패턴이다.
    pub fn observe_show_settings_requests(
        cb: Box<dyn Fn() + Send + Sync>,
    ) -> ShowSettingsObserver {
        let center = NSDistributedNotificationCenter::defaultCenter();
        let name = NSString::from_str(SHOW_SETTINGS_NOTIFICATION);
        let block = RcBlock::new(move |_note: NonNull<NSNotification>| {
            (cb)();
        });
        // SAFETY: `block` 은 이 호출 동안 유효하고, 알림 센터가 즉시
        // `Block_copy` 로 자체 사본을 만든다(`workspace.rs` 의 `add_simple`
        // 과 같은 계약). 이름은 방금 만든 유효한 `NSString` 이다.
        let token = unsafe {
            center.addObserverForName_object_queue_usingBlock(
                Some(&name),
                None,
                None,
                &block,
            )
        };
        ShowSettingsObserver {
            center,
            _name: name,
            token,
        }
    }

    /// 구독 토큰 — `Drop` 에서 옵저버를 제거한다(`SystemEventObserver` 와
    /// 같은 규약).
    pub struct ShowSettingsObserver {
        center: Retained<NSDistributedNotificationCenter>,
        // 알림 이름 `NSString` 을 등록 해제 때까지 소유해 두는 것 외에
        // 읽는 곳은 없다 — 등록과 해제가 같은 문자열을 쓰도록 묶는다.
        _name: Retained<NSString>,
        token: Token,
    }

    // SAFETY: 이 구조체가 담는 것은 센터 핸들·문자열·옵저버 토큰(순수
    // 핸들)뿐이다. 실제 콜백은 알림이 전달되는 스레드(관례상 메인
    // 런루프)에서만 불리고, 이 구조체를 다른 스레드로 옮겨 "보관"만 하는
    // 것은 안전하다(`workspace.rs` 의 `SystemEventObserver` 와 같은 근거).
    unsafe impl Send for ShowSettingsObserver {}

    impl Drop for ShowSettingsObserver {
        fn drop(&mut self) {
            // SAFETY: `self.token` 은 등록 시 받아 소유해 온 유효한 옵저버
            // 토큰이고, `_name` 은 등록 때 쓴 것과 같은 문자열이다.
            unsafe {
                self.center
                    .removeObserver_name_object(self.token.as_ref(), Some(&self._name), None);
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{
    notify_existing_instance_to_show_settings, observe_show_settings_requests,
    ShowSettingsObserver,
};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    pub struct ShowSettingsObserver(core::convert::Infallible);

    pub fn notify_existing_instance_to_show_settings() {}

    pub fn observe_show_settings_requests(
        _cb: Box<dyn Fn() + Send + Sync>,
    ) -> ShowSettingsObserver {
        panic!("ultrakey-platform: observe_show_settings_requests 는 macOS 전용이다")
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{
    notify_existing_instance_to_show_settings, observe_show_settings_requests,
    ShowSettingsObserver,
};