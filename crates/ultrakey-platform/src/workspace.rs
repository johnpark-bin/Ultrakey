//! `NSWorkspace` / 화면 잠금 알림 — 절전·로그인·세션전환·최전면 앱·화면 잠금.
//!
//! ⭐ ObjC 클래스를 직접 선언하지 않고 **`block2` 크레이트로
//! `addObserverForName:object:queue:usingBlock:`** 를 쓴다 — 옵저버 토큰을
//! [`SystemEventObserver`] 가 소유하고 `Drop` 에서 `removeObserver:` 한다.
//!
//! ⚠️ 이 알림들은 전부 **메인 스레드**에서 온다(`NSWorkspace`/AppKit 알림
//! 센터의 일반 규약, 화면 잠금 Distributed Notification 도 통상 메인
//! 런루프에서 처리된다). 콜백은 신호만 넘기고 무거운 일을 하지 말아야
//! 한다 — 실제 무거운 작업(리매핑 상태 갱신 등)은 호출자(엔진)가 커맨드
//! 큐로 다른 스레드에 위임해야 한다.

/// 이 크레이트가 감지해 넘기는 시스템 이벤트.
#[derive(Clone)]
pub enum SystemEvent {
    WillSleep,
    DidWake,
    SessionDidBecomeActive,
    SessionDidResignActive,
    ScreenLocked,
    ScreenUnlocked,
    FrontAppChanged(Option<ultrakey_core::gate::AppIdentity>),
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::SystemEvent;
    use block2::RcBlock;
    use core::ptr::NonNull;
    use objc2::rc::Retained;
    use objc2::runtime::{AnyObject, NSObjectProtocol, ProtocolObject};
    use objc2_app_kit::{
        NSRunningApplication, NSWorkspace, NSWorkspaceApplicationKey,
        NSWorkspaceDidActivateApplicationNotification, NSWorkspaceDidWakeNotification,
        NSWorkspaceSessionDidBecomeActiveNotification,
        NSWorkspaceSessionDidResignActiveNotification, NSWorkspaceWillSleepNotification,
    };
    use objc2_foundation::{
        NSDistributedNotificationCenter, NSNotification, NSNotificationCenter, NSString,
    };
    use std::sync::Arc;
    use ultrakey_core::gate::AppIdentity;

    type Token = Retained<ProtocolObject<dyn NSObjectProtocol>>;

    fn front_app_identity(note: &NSNotification) -> Option<AppIdentity> {
        let info = note.userInfo()?;
        // SAFETY: `NSWorkspaceApplicationKey` 는 AppKit 이 항상 정의하는
        // 정적 심볼을 읽는 것뿐이고, `NSString` → `AnyObject` 재해석은 모든
        // objc2 클래스 참조가 공유하는 Objective-C 객체 표현(첫 워드가
        // `isa` 포인터)에 기반한다 — `AnyObject` 자체가 "임의의 objc 객체"
        // 를 표현하기 위해 이 표현으로 정의된 타입이다.
        let key: &AnyObject =
            unsafe { &*(NSWorkspaceApplicationKey as *const NSString as *const AnyObject) };
        let obj = info.objectForKey(key)?;
        let app = obj.downcast::<NSRunningApplication>().ok()?;
        Some(AppIdentity {
            bundle_id: app.bundleIdentifier().map(|s| s.to_string()).unwrap_or_default(),
            name: app.localizedName().map(|s| s.to_string()).unwrap_or_default(),
        })
    }

    /// 이 크레이트가 아는 알림 이름 각각을 [`SystemEvent`] 로 매핑한다.
    /// `FrontAppChanged` 만 알림 자체(userInfo)에서 값을 뽑아야 해서 별도
    /// 클로저로 처리한다.
    fn add_simple(
        center: &NSNotificationCenter,
        name: &'static objc2_foundation::NSNotificationName,
        shared: Arc<dyn Fn(SystemEvent) + Send + Sync>,
        event: SystemEvent,
    ) -> Token {
        let block = RcBlock::new(move |_note: NonNull<NSNotification>| {
            (shared)(event.clone());
        });
        // SAFETY: `block` 은 이 호출 동안 유효하고, AppKit 이 즉시
        // `Block_copy` 로 자체 사본을 만든다.
        unsafe { center.addObserverForName_object_queue_usingBlock(Some(name), None, None, &block) }
    }

    pub struct SystemEventObserver {
        workspace_center: Retained<NSNotificationCenter>,
        distributed_center: Retained<NSDistributedNotificationCenter>,
        tokens: Vec<Token>,
        distributed_tokens: Vec<Token>,
    }

    // SAFETY: 이 구조체는 옵저버 토큰(순수 핸들)만 담는다. 실제 콜백은
    // 항상 알림이 실제로 전달되는 스레드(관례상 메인 스레드)에서만
    // 호출되며, 이 구조체를 다른 스레드로 옮겨 "보관"만 하는 것은 안전하다.
    unsafe impl Send for SystemEventObserver {}

    pub fn observe_system_events(
        cb: Box<dyn Fn(SystemEvent) + Send + Sync>,
    ) -> SystemEventObserver {
        let shared: Arc<dyn Fn(SystemEvent) + Send + Sync> = Arc::from(cb);

        // SAFETY: `sharedWorkspace()`/`notificationCenter()` 는 인자가
        // 없는 단순 접근자다.
        let workspace = NSWorkspace::sharedWorkspace();
        let workspace_center = workspace.notificationCenter();

        let mut tokens = Vec::new();
        // SAFETY: 각 정적 심볼은 AppKit 이 항상 정의하는 알림 이름이다.
        unsafe {
            tokens.push(add_simple(
                &workspace_center,
                NSWorkspaceWillSleepNotification,
                Arc::clone(&shared),
                SystemEvent::WillSleep,
            ));
            tokens.push(add_simple(
                &workspace_center,
                NSWorkspaceDidWakeNotification,
                Arc::clone(&shared),
                SystemEvent::DidWake,
            ));
            tokens.push(add_simple(
                &workspace_center,
                NSWorkspaceSessionDidBecomeActiveNotification,
                Arc::clone(&shared),
                SystemEvent::SessionDidBecomeActive,
            ));
            tokens.push(add_simple(
                &workspace_center,
                NSWorkspaceSessionDidResignActiveNotification,
                Arc::clone(&shared),
                SystemEvent::SessionDidResignActive,
            ));
        }

        // 최전면 앱 변경 — userInfo 에서 직접 뽑아야 하므로 별도 블록.
        {
            let shared = Arc::clone(&shared);
            let block = RcBlock::new(move |note: NonNull<NSNotification>| {
                // SAFETY: AppKit 이 유효성을 보장하는 통지 객체다.
                let note = unsafe { note.as_ref() };
                (shared)(SystemEvent::FrontAppChanged(front_app_identity(note)));
            });
            // SAFETY: 위와 동일한 계약.
            let token = unsafe {
                workspace_center.addObserverForName_object_queue_usingBlock(
                    Some(NSWorkspaceDidActivateApplicationNotification),
                    None,
                    None,
                    &block,
                )
            };
            tokens.push(token);
        }

        // 화면 잠금/해제 — `com.apple.screenIsLocked`/`com.apple.screenIsUnlocked`
        // Distributed Notification. `NSDistributedNotificationCenter` 는
        // `NSNotificationCenter` 의 서브클래스라 같은 block 기반 API 를
        // 그대로 상속한다 — 별도 ObjC 클래스나 raw C 콜백을 새로 만들
        // 필요가 없다.
        let distributed_center = NSDistributedNotificationCenter::defaultCenter();
        let mut distributed_tokens = Vec::new();
        {
            let locked_name = NSString::from_str("com.apple.screenIsLocked");
            let unlocked_name = NSString::from_str("com.apple.screenIsUnlocked");

            let shared_locked = Arc::clone(&shared);
            let block_locked = RcBlock::new(move |_note: NonNull<NSNotification>| {
                (shared_locked)(SystemEvent::ScreenLocked);
            });
            // SAFETY: `NSDistributedNotificationCenter` 는 `Deref` 를 통해
            // `NSNotificationCenter` 의 메서드를 그대로 쓸 수 있다.
            let token = unsafe {
                distributed_center.addObserverForName_object_queue_usingBlock(
                    Some(&locked_name),
                    None,
                    None,
                    &block_locked,
                )
            };
            distributed_tokens.push(token);

            let shared_unlocked = Arc::clone(&shared);
            let block_unlocked = RcBlock::new(move |_note: NonNull<NSNotification>| {
                (shared_unlocked)(SystemEvent::ScreenUnlocked);
            });
            let token = unsafe {
                distributed_center.addObserverForName_object_queue_usingBlock(
                    Some(&unlocked_name),
                    None,
                    None,
                    &block_unlocked,
                )
            };
            distributed_tokens.push(token);
        }

        SystemEventObserver {
            workspace_center,
            distributed_center,
            tokens,
            distributed_tokens,
        }
    }

    impl Drop for SystemEventObserver {
        fn drop(&mut self) {
            for token in self.tokens.drain(..) {
                // SAFETY: `token` 은 이 구조체가 등록 시 받아 소유해 온
                // 유효한 옵저버 토큰이다.
                unsafe { self.workspace_center.removeObserver(token.as_ref()) };
            }
            for token in self.distributed_tokens.drain(..) {
                // SAFETY: 위와 동일.
                unsafe { self.distributed_center.removeObserver(token.as_ref()) };
            }
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{observe_system_events, SystemEventObserver};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::SystemEvent;

    pub struct SystemEventObserver(core::convert::Infallible);

    pub fn observe_system_events(
        _cb: Box<dyn Fn(SystemEvent) + Send + Sync>,
    ) -> SystemEventObserver {
        panic!("ultrakey-platform: observe_system_events 는 macOS 전용이다")
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{observe_system_events, SystemEventObserver};
