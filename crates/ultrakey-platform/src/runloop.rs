//! 전용 스레드 런루프와 커맨드 소스.
//!
//! `architecture.md` §2.1 이 정한 배치: 이벤트 탭은 전용 스레드의 독립
//! `CFRunLoop` 위에서 돈다. 이 모듈은 그 런루프를 다루는 데 필요한 세 가지
//! 원시 장치를 제공한다 — 다른 스레드에서 이 런루프를 멈추는 [`RunLoopHandle`],
//! 다른 스레드에서 "일이 있다"고 깨우는 [`CommandSource`]/[`CommandSignaller`],
//! 그리고 주기 타이머 [`RepeatingTimer`](quick press 타이머 등에 쓰인다).
//!
//! ⭐ **`CommandSource` 자체는 큐를 갖지 않는다.** architecture.md §2.2 의
//! "무잠금 MPSC 큐" 는 이 소스를 쓰는 쪽(엔진)이 별도로 들고 있는 큐를 뜻한다
//! — 이 소스는 그 큐를 "지금 비워도 된다"고 깨우는 신호(signal) 장치일
//! 뿐이다. `CommandSource::new` 의 `handler` 가 곧 "깨어났을 때 할 일"이고,
//! 그 안에서 엔진이 자신의 큐를 드레인한다.

use core::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

/// ⭐ **엔진 크레이트 추가분(2026-08-30, 이슈 #5 M1 구현)** — 탭 전용 스레드가 등록한
/// 콜백들(탭 이벤트 트램폴린 · quick press 타이머 · 커맨드 perform) 사이에서 가변
/// 상태를 공유하기 위한 손잡이.
///
/// `ultrakey-engine` 은 `#![forbid(unsafe_code)]` 라 이 정당화가 필요한 타입을 직접
/// 만들 수 없다 — "이 저장소의 모든 `unsafe` 는 `ultrakey-platform` 에만 있다"는
/// 전제(이 크레이트 최상단 문서, `docs/dev/architecture.md` §1)를 지키려면 이 자리가
/// 맞다. 이 타입 없이는 `TapCallback`/`CommandSource`/`RepeatingTimer` 세 콜백 시그니처가
/// 전부 `Send` 를 요구하는데, 그 셋이 공유해야 하는 정본 상태(`Arbiter` 등)를
/// `Rc<RefCell<_>>` 로 두면 `Rc` 가 `!Send` 라 타입이 맞지 않고, `Arc<Mutex<_>>` 로
/// 두면 architecture.md §2.2 가 콜백 안에서 명시적으로 금지하는 "뮤텍스 획득"이
/// 필요해진다.
///
/// ⭐ **왜 안전한가**: `Rc<RefCell<T>>` 는 원래 스레드 경계를 넘지 못하지만, 이 값을
/// 캡처하는 세 콜백은 전부 **같은 전용 스레드가 등록**하며, 그 스레드의 `CFRunLoop` 가
/// 항상 하나씩만 직렬로 호출한다(`docs/dev/architecture.md` §2.2 — "콜백과 같은
/// 스레드에서 직렬 실행되므로 상태 테이블에 락 없이 접근할 수 있다"가 정의하는 바로 그
/// 불변식). `Box<dyn FnMut() + Send>` 의 `Send` 는 "이 클로저 값 자체를 다른 스레드로
/// 옮길 수 있어야 한다"는 타입 시스템의 일반 요구를 표현할 뿐이며, 이 클로저들이 실제로
/// 옮겨진다는 뜻은 아니다 — 전부 탭 전용 스레드 안에서 생성되고, 그 스레드가 등록한
/// 런루프 소스만 호출한다. 이 불변식을 어기는 유일한 방법은 이 값을 캡처한 클로저를
/// 직접 다른 스레드로 옮겨 실행하는 것인데, `ultrakey-engine` 은 이 값을 절대 스레드
/// 경계 밖으로 내보내지 않는다(탭 스레드 로컬 콜백 셋을 만드는 데만 쓴다).
///
/// macOS 여부와 무관한 순수 Rust 코드다 — CF 타입에 의존하지 않으므로 플랫폼 분기가
/// 필요 없다.
pub struct RunLoopConfined<T>(Rc<RefCell<T>>);

impl<T> RunLoopConfined<T> {
    pub fn new(value: T) -> Self {
        RunLoopConfined(Rc::new(RefCell::new(value)))
    }

    pub fn borrow(&self) -> Ref<'_, T> {
        self.0.borrow()
    }

    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        self.0.borrow_mut()
    }
}

impl<T> Clone for RunLoopConfined<T> {
    fn clone(&self) -> Self {
        RunLoopConfined(Rc::clone(&self.0))
    }
}

// SAFETY: 위 타입 문서 참고 — 이 값을 캡처하는 클로저는 전부 등록한 스레드의
// `CFRunLoop` 만 호출하고, 그 호출은 항상 직렬(재진입 없음)이다. 실제로 다른
// 스레드로 이동하지 않는다는 것은 이 값을 쓰는 `ultrakey-engine` 이 보장한다.
unsafe impl<T> Send for RunLoopConfined<T> {}

#[cfg(target_os = "macos")]
mod macos_impl {
    use block2::RcBlock;
    use core::cell::RefCell;
    use core::ffi::c_void;
    use objc2_core_foundation::{
        kCFRunLoopCommonModes, CFAbsoluteTimeGetCurrent, CFRetained, CFRunLoop, CFRunLoopSource,
        CFRunLoopSourceContext, CFRunLoopTimer,
    };
    use std::sync::{Arc, OnceLock};

    // ------------------------------------------------------------------
    // RunLoopHandle
    // ------------------------------------------------------------------

    /// 다른 스레드에서 이 런루프를 멈추거나 깨울 수 있게 하는 핸들.
    ///
    /// SAFETY: `CFRunLoopStop`/`CFRunLoopWakeUp` 은 Apple 문서상 "임의의
    /// 스레드에서 호출해도 안전하다"고 명시된 함수다 — 애초에 다른 스레드가
    /// 실행 중인 런루프를 깨우는 것이 이 API 들의 존재 이유다. 우리는 이
    /// 두 호출만 노출하므로 `Send` 를 부여해도 데이터 경합이 생기지 않는다.
    pub struct RunLoopHandle(CFRetained<CFRunLoop>);
    // SAFETY: 위 문서 참조.
    unsafe impl Send for RunLoopHandle {}

    impl RunLoopHandle {
        /// 현재 스레드의 런루프를 가리키는 핸들을 만든다.
        pub fn current() -> Option<Self> {
            CFRunLoop::current().map(Self)
        }

        pub fn stop(&self) {
            self.0.stop();
        }

        pub fn wake_up(&self) {
            self.0.wake_up();
        }

        /// ⭐ **엔진 크레이트 추가분(2026-08-30, 이슈 #5 M1 구현)** — 현재 스레드의
        /// 런루프를 블로킹으로 구동한다(`CFRunLoopRun`). `stop()` 이 호출되기
        /// 전까지 리턴하지 않는다. `docs/dev/architecture.md` §2.1 이 정한
        /// "전용 스레드가 독립 `CFRunLoop` 를 돈다" 배치를 실제로 성립시키려면
        /// 이 스레드가 `CFRunLoopRun` 을 호출해 등록된 소스(탭 mach port,
        /// 커맨드 소스, 타이머)를 서비스해야 하는데, 이 모듈은 그때까지
        /// `current()`/`stop()`/`wake_up()` 만 제공하고 구동(run) 자체를 노출하지
        /// 않았다 — `ultrakey-engine` 구현 중 발견한 누락이며, 기존 API 를
        /// 전혀 바꾸지 않는 순수 추가이므로 여기서 채운다. `CFRunLoopRun` 은
        /// 인자가 없는 함수(옛 API 자체가 "현재 스레드"에 고정되어 있다)라
        /// 특정 `RunLoopHandle` 인스턴스에 묶이지 않는 연관 함수로 둔다.
        pub fn run() {
            CFRunLoop::run();
        }
    }

    // ------------------------------------------------------------------
    // CommandSource / CommandSignaller
    // ------------------------------------------------------------------

    /// `CFRunLoop`/`CFRunLoopSource` 는 CF 문서상 참조 카운트 증감이 원자적
    /// 이라 여러 스레드가 안전하게 공유할 수 있지만, 이 크레이트가 쓰는
    /// `objc2-core-foundation` 은 기본적으로 `Send`/`Sync` 를 주지 않는다.
    /// `CFRunLoopSourceSignal`/`CFRunLoopWakeUp` 은 Apple 문서상 명시적으로
    /// 다른 스레드에서 호출하도록 설계된 API 이므로, 이 값들만 감싸는
    /// 전용 래퍼에 한해 수동으로 `Send`/`Sync` 를 부여한다.
    struct SendSyncCf<T>(T);
    // SAFETY: 위 문서 참조 — 이 크레이트는 이 래퍼를 통해 signal/wake_up 류의
    // "다른 스레드에서 호출해도 되는" 연산만 노출한다.
    unsafe impl<T> Send for SendSyncCf<T> {}
    // SAFETY: 위와 동일.
    unsafe impl<T> Sync for SendSyncCf<T> {}

    struct SignallerInner {
        source: SendSyncCf<CFRetained<CFRunLoopSource>>,
        // 이 소스가 등록된 런루프. `add_to_current_runloop` 호출 시 한 번만
        // 채워지고(탭 스레드), 이후로는 다른 스레드가 읽기만 한다 —
        // `OnceLock` 이 이 "한 번 쓰고 여러 번 읽는" 패턴을 락 없이 보장한다.
        run_loop: OnceLock<SendSyncCf<CFRetained<CFRunLoop>>>,
    }

    /// 탭 스레드에서 실행할 "할 일"을 담는 컨텍스트.
    ///
    /// `perform` 콜백(버전 0 `CFRunLoopSourceContext`)이 매번 이걸 통해
    /// 핸들러를 부른다.
    struct PerformContext {
        handler: Box<dyn FnMut() + Send>,
    }

    /// # Safety
    /// CoreFoundation 은 이 콜백을 소스의 마지막 참조가 해제될 때 정확히
    /// 한 번 호출한다(버전 0 컨텍스트의 `release` 콜백 계약). `info` 는
    /// `CommandSource::new` 가 `Box::into_raw` 로 만든 `PerformContext` 다.
    unsafe extern "C-unwind" fn perform_context_release(info: *const c_void) {
        drop(unsafe { Box::from_raw(info as *mut PerformContext) });
    }

    /// # Safety
    /// `info` 는 소스가 살아있는(따라서 이 콜백이 호출될 수 있는) 동안
    /// 항상 유효한 `PerformContext` 를 가리킨다 — 소스가 무효화된 뒤에는
    /// CoreFoundation 이 이 콜백을 부르지 않는다.
    unsafe extern "C-unwind" fn perform_context_perform(info: *mut c_void) {
        let ctx = unsafe { &mut *(info as *mut PerformContext) };
        (ctx.handler)();
    }

    /// 다른 스레드에서 탭 스레드로 "일이 있다"는 신호만 보내는 장치.
    pub struct CommandSource {
        inner: Arc<SignallerInner>,
    }

    impl CommandSource {
        pub fn new(handler: Box<dyn FnMut() + Send>) -> Self {
            let ctx_ptr = Box::into_raw(Box::new(PerformContext { handler }));

            let mut context = CFRunLoopSourceContext {
                version: 0,
                info: ctx_ptr as *mut c_void,
                retain: None,
                release: Some(perform_context_release),
                copyDescription: None,
                equal: None,
                hash: None,
                schedule: None,
                cancel: None,
                perform: Some(perform_context_perform),
            };
            // SAFETY: `context` 는 이 호출 동안만 유효하면 되는 스택 값이고
            // (CF 가 내용을 복사해 간다), `perform`/`release` 콜백은 위에서
            // 정확한 시그니처로 정의했다. 생성이 실패하면(극히 드묾) 아래
            // `expect` 이전에 `ctx_ptr` 를 되찾아 누수를 막는다.
            let source = unsafe { CFRunLoopSource::new(None, 0, &mut context) };
            let source = match source {
                Some(s) => s,
                None => {
                    // SAFETY: 소스 생성이 실패했으므로 `release` 콜백이 절대
                    // 호출되지 않는다 — 여기서 직접 되찾아 드롭해야 한다.
                    drop(unsafe { Box::from_raw(ctx_ptr) });
                    panic!("CFRunLoopSourceCreate 실패 — 이 시스템은 이 기능을 지원하지 않는다");
                }
            };

            Self {
                inner: Arc::new(SignallerInner {
                    source: SendSyncCf(source),
                    run_loop: OnceLock::new(),
                }),
            }
        }

        /// 현재 스레드의 런루프에 `kCFRunLoopCommonModes` 로 등록한다.
        pub fn add_to_current_runloop(&self) {
            if let Some(rl) = CFRunLoop::current() {
                // SAFETY: 정적 심볼 읽기.
                rl.add_source(Some(&self.inner.source.0), unsafe {
                    kCFRunLoopCommonModes
                });
                let _ = self.inner.run_loop.set(SendSyncCf(rl));
            }
        }

        /// 다른 스레드로 보낼 수 있는 시그널러를 만든다.
        pub fn signaller(&self) -> CommandSignaller {
            CommandSignaller {
                inner: Arc::clone(&self.inner),
            }
        }
    }

    impl Drop for CommandSource {
        fn drop(&mut self) {
            // 이 소스를 더 이상 아무도 깨우지 못하게 막는다. 실제 메모리
            // 해제(Box 회수)는 `release` 콜백이 마지막 참조 시점에 처리한다.
            self.inner.source.0.invalidate();
        }
    }

    /// [`CommandSource`] 를 다른 스레드로 보내 깨울 수 있게 하는 손잡이.
    #[derive(Clone)]
    pub struct CommandSignaller {
        inner: Arc<SignallerInner>,
    }

    impl CommandSignaller {
        pub fn signal(&self) {
            self.inner.source.0.signal();
            if let Some(rl) = self.inner.run_loop.get() {
                rl.0.wake_up();
            }
        }
    }

    // ------------------------------------------------------------------
    // RepeatingTimer
    // ------------------------------------------------------------------

    /// 콜백을 담는 셀 — 타이머가 같은 런루프 스레드에서 직렬로만 호출되므로
    /// `RefCell` 로 `FnMut` 을 `Fn`(블록이 요구하는 트레이트) 뒤에 감싸도
    /// 재진입 경합이 없다.
    type TimerHandlerCell = RefCell<Box<dyn FnMut() + Send>>;

    pub struct RepeatingTimer {
        timer: CFRetained<CFRunLoopTimer>,
        // CoreFoundation 이 `Block_copy` 로 자체 사본을 갖긴 하지만, 방어적으로
        // 우리도 타이머와 같은 수명으로 하나 들고 있는다.
        _handler_block: RcBlock<dyn Fn(*mut CFRunLoopTimer)>,
    }

    impl RepeatingTimer {
        pub fn new(interval_ms: u64, handler: Box<dyn FnMut() + Send>) -> Self {
            let handler_cell: TimerHandlerCell = RefCell::new(handler);
            let interval_s = interval_ms as f64 / 1000.0;

            let block: RcBlock<dyn Fn(*mut CFRunLoopTimer)> =
                RcBlock::new(move |_timer: *mut CFRunLoopTimer| {
                    // SAFETY 재진입 불가: `CFRunLoopTimer` 콜백은 항상 그
                    // 타이머가 등록된 런루프의 같은 스레드에서 직렬 실행된다.
                    if let Ok(mut h) = handler_cell.try_borrow_mut() {
                        (h)();
                    }
                });

            let fire_date = CFAbsoluteTimeGetCurrent() + interval_s;
            let block_ref: &block2::DynBlock<dyn Fn(*mut CFRunLoopTimer)> = &block;
            // SAFETY: `block` 은 이 호출이 끝나기 전까지 유효하고,
            // `CFRunLoopTimerCreateWithHandler` 는 문서상 블록을
            // `Block_copy` 로 즉시 복사해 자신의 참조를 만든다.
            let timer = unsafe {
                CFRunLoopTimer::with_handler(None, fire_date, interval_s, 0, 0, Some(block_ref))
            }
            .expect("CFRunLoopTimerCreateWithHandler 실패");

            Self {
                timer,
                _handler_block: block,
            }
        }

        pub fn add_to_current_runloop(&self) {
            if let Some(rl) = CFRunLoop::current() {
                // SAFETY: 정적 심볼 읽기.
                rl.add_timer(Some(&self.timer), unsafe { kCFRunLoopCommonModes });
            }
        }

        pub fn set_next_fire_after_ms(&self, ms: u64) {
            let fire_date = CFAbsoluteTimeGetCurrent() + (ms as f64 / 1000.0);
            self.timer.set_next_fire_date(fire_date);
        }
    }

    impl Drop for RepeatingTimer {
        fn drop(&mut self) {
            self.timer.invalidate();
        }
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::{CommandSignaller, CommandSource, RepeatingTimer, RunLoopHandle};

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    pub struct RunLoopHandle(core::convert::Infallible);
    impl RunLoopHandle {
        pub fn current() -> Option<Self> {
            None
        }
        pub fn stop(&self) {
            match self.0 {}
        }
        pub fn wake_up(&self) {
            match self.0 {}
        }

        /// 위 macOS 구현과 짝을 맞추는 스텁 — 실제로 호출될 일이 없다
        /// (`ultrakey-engine` 이 non-macOS 에서는 탭 스레드 자체를 시작하지 않는다).
        pub fn run() {}
    }

    pub struct CommandSource(core::convert::Infallible);
    impl CommandSource {
        pub fn new(_handler: Box<dyn FnMut() + Send>) -> Self {
            panic!("ultrakey-platform: CommandSource 는 macOS 전용이다")
        }
        pub fn add_to_current_runloop(&self) {
            match self.0 {}
        }
        pub fn signaller(&self) -> CommandSignaller {
            match self.0 {}
        }
    }

    #[derive(Clone)]
    pub struct CommandSignaller(());
    impl CommandSignaller {
        pub fn signal(&self) {}
    }

    pub struct RepeatingTimer(core::convert::Infallible);
    impl RepeatingTimer {
        pub fn new(_interval_ms: u64, _handler: Box<dyn FnMut() + Send>) -> Self {
            panic!("ultrakey-platform: RepeatingTimer 는 macOS 전용이다")
        }
        pub fn add_to_current_runloop(&self) {
            match self.0 {}
        }
        pub fn set_next_fire_after_ms(&self, _ms: u64) {
            match self.0 {}
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::{CommandSignaller, CommandSource, RepeatingTimer, RunLoopHandle};
