//! 백그라운드 폴링 스레드. [`crate::model::PermissionModel`] 을 실제
//! `AXIsProcessTrusted()` 호출에 연결한다(§3.4).
//!
//! ⚠️ 종료 신호를 `crossbeam_channel::Receiver::recv_timeout` 으로 기다린다 —
//! `std::thread::sleep` 으로 주기를 만들면 `stop()`/`Drop` 이 최대 한 주기만큼
//! 지연된다. `recv_timeout` 은 "종료 신호가 오면 즉시 깨어나고, 오지 않으면
//! 주기가 다 됐을 때 폴링한다"를 한 호출로 표현한다.

use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crossbeam_channel::{bounded, RecvTimeoutError, Sender};

use crate::model::{PermissionModel, PermissionState, PermissionTransition, PollMode};

type TransitionCallback = dyn Fn(PermissionTransition) + Send + Sync;

/// 백그라운드 폴링 스레드.
///
/// 상태는 [`Mutex<PermissionModel>`] 하나로 보호한다 — 폴링 스레드와, 메인
/// 스레드가 호출하는 [`Self::check_now`]/[`Self::report_tap_create_failed`]/
/// [`Self::report_tap_created`] 가 동시에 상태를 바꿀 수 있으므로, 순수 판정
/// 로직([`PermissionModel`])은 그대로 두고 이 크레이트 레벨에서만 락을 씌운다.
pub struct PermissionMonitor {
    model: Arc<Mutex<PermissionModel>>,
    on_transition: Arc<TransitionCallback>,
    /// 종료 신호. `stop()` 이 `&self` 를 받으므로 `Sender` 자체는 불변으로 두고
    /// 반복 호출에도 안전하게(수신자가 이미 사라졌으면 `send` 가 조용히 실패)
    /// 동작하게 한다.
    stop_tx: Sender<()>,
    /// `stop()`/`Drop` 양쪽에서 딱 한 번만 join 하기 위한 핸들. `stop(&self)` 가
    /// 시그니처상 `&mut self` 를 받을 수 없으므로 내부 가변성이 필요하다.
    handle: Mutex<Option<JoinHandle<()>>>,
}

impl PermissionMonitor {
    /// `on_transition` 은 폴링 스레드에서 호출된다 — 무거운 일을 하지 마라.
    pub fn start(
        onboarding_ms: u64,
        background_ms: u64,
        on_transition: Box<TransitionCallback>,
    ) -> Self {
        let model = Arc::new(Mutex::new(PermissionModel::new()));
        let on_transition: Arc<TransitionCallback> = Arc::from(on_transition);

        // 용량 0(랑데부) 채널 — 신호가 오기 전까지는 `recv_timeout` 이 순수하게
        // 주기만큼만 블록한다.
        let (stop_tx, stop_rx) = bounded::<()>(0);

        let thread_model = Arc::clone(&model);
        let thread_callback = Arc::clone(&on_transition);
        // ⚠️ 스레드에 이름을 준다 — 이 스레드가 어떤 콜백을 실행했는지가
        // 진단에서 결정적일 수 있다(이슈 #10: 전이 콜백이 이 스레드에서
        // 엔진을 시작해 IOKit 런루프 소스가 여기 걸렸다). 이름이 없으면
        // 로그에 `ThreadId(N)` 만 남아 아무것도 알려주지 않는다.
        let handle = thread::Builder::new()
            .name("ultrakey-permission-poll".to_string())
            .spawn(move || loop {
                let trusted = ultrakey_platform::accessibility::is_process_trusted();
                // ⚠️ 락을 놓은 뒤에 콜백을 호출한다. `on_transition` 은 결국
                // `show_modal()` → tao `make_key_and_order_front_sync` → 메인
                // 스레드로 동기 디스패치(블로킹)까지 이어진다. 그동안 메인
                // 스레드가 `modal_copy` 커맨드에서 `monitor.state()` 를 불러
                // 같은 `model` 뮤텍스를 기다리면, 폴링 스레드는 메인 스레드를
                // 기다리고 메인 스레드는 폴링 스레드가 쥔 락을 기다리는
                // 교착이 생긴다. 그래서 `guard` 를 블록 끝에서 드롭해 락을 놓은
                // 뒤에 콜백을 부른다.
                let (transition, poll_mode) = {
                    let mut guard = thread_model
                        .lock()
                        .expect("permission model mutex poisoned");
                    let transition = guard.observe_trusted(trusted);
                    (transition, guard.poll_mode())
                };
                if let Some(transition) = transition {
                    (thread_callback)(transition);
                }

                let interval_ms = match poll_mode {
                    PollMode::Onboarding => onboarding_ms,
                    PollMode::Background => background_ms,
                };

                match stop_rx.recv_timeout(Duration::from_millis(interval_ms)) {
                    // 종료 신호를 받았거나, 송신 측(이 구조체)이 드롭돼 채널이
                    // 끊어졌다 — 두 경우 모두 루프를 빠져나간다.
                    Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
                    Err(RecvTimeoutError::Timeout) => continue,
                }
            })
            .expect("권한 폴링 스레드 생성 실패");

        PermissionMonitor {
            model,
            on_transition,
            stop_tx,
            handle: Mutex::new(Some(handle)),
        }
    }

    pub fn state(&self) -> PermissionState {
        self.model
            .lock()
            .expect("permission model mutex poisoned")
            .state()
    }

    /// ⭐ 즉시 1회 확인(§3.4 "보조 트리거": 앱이 포그라운드로 돌아올 때, F-07 이
    /// 탭 설치를 시도하기 직전).
    ///
    /// ⚠️ 폴링 루프와 같은 이유로, 락을 놓은 뒤에 콜백을 호출한다 —
    /// `on_transition` 이 메인 스레드로 동기 디스패치되므로 락을 들고 있으면
    /// 교착으로 이어질 수 있다.
    pub fn check_now(&self) -> PermissionState {
        let trusted = ultrakey_platform::accessibility::is_process_trusted();
        let (transition, state) = {
            let mut guard = self.model.lock().expect("permission model mutex poisoned");
            let transition = guard.observe_trusted(trusted);
            (transition, guard.state())
        };
        if let Some(transition) = transition {
            (self.on_transition)(transition);
        }
        state
    }

    /// ⚠️ 락을 놓은 뒤에 콜백을 호출한다(위 `check_now` 와 동일한 이유).
    pub fn report_tap_create_failed(&self) {
        let trusted = ultrakey_platform::accessibility::is_process_trusted();
        let transition = {
            let mut guard = self.model.lock().expect("permission model mutex poisoned");
            guard.observe_tap_create_failed(trusted)
        };
        if let Some(transition) = transition {
            (self.on_transition)(transition);
        }
    }

    /// ⚠️ 락을 놓은 뒤에 콜백을 호출한다(위 `check_now` 와 동일한 이유).
    pub fn report_tap_created(&self) {
        let transition = {
            let mut guard = self.model.lock().expect("permission model mutex poisoned");
            guard.observe_tap_created()
        };
        if let Some(transition) = transition {
            (self.on_transition)(transition);
        }
    }

    /// 폴링 스레드를 종료한다. 여러 번 호출해도 안전하다(두 번째 호출부터는
    /// `send`/`join` 이 아무 일도 하지 않는다).
    pub fn stop(&self) {
        // 스레드가 이미 스스로 끝났다면(있을 수 없지만) 수신자가 없어 `send`
        // 가 실패한다 — 그래도 문제 없다, 아래에서 join 만 시도하면 된다.
        let _ = self.stop_tx.send(());
        if let Some(handle) = self
            .handle
            .lock()
            .expect("permission monitor handle mutex poisoned")
            .take()
        {
            let _ = handle.join();
        }
    }
}

impl Drop for PermissionMonitor {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// 폴링 스레드가 실제로 짧은 주기로 동작하고, `stop()` 이 스레드를
    /// 지연 없이 종료시키는지 확인한다. macOS 가 아닌 타깃에서는
    /// `is_process_trusted()` 스텁이 항상 `false` 를 반환하므로 상태는
    /// `Denied` 에 머문다 — 그래도 스레드 생애주기 자체는 플랫폼 무관하게
    /// 검증할 수 있다.
    #[test]
    fn stop_terminates_polling_thread_promptly() {
        let (tx, rx) = mpsc::channel();
        // 배경 주기를 일부러 아주 길게 잡아, `stop()` 이 그 주기를 기다리지
        // 않고 즉시 반환하는지를 검증한다.
        let monitor = PermissionMonitor::start(
            10,
            60_000,
            Box::new(move |transition| {
                let _ = tx.send(transition);
            }),
        );

        // 최초 1회 폴링 결과(전이)가 도착할 때까지 기다린다.
        let _ = rx.recv_timeout(Duration::from_secs(2));

        let started = std::time::Instant::now();
        monitor.stop();
        assert!(
            started.elapsed() < Duration::from_secs(1),
            "stop() 이 배경 주기(60s)를 기다리지 않고 즉시 반환해야 한다"
        );
    }

    /// `observe_tap_created` 는 `trusted` 여부와 무관하게 무조건 `Granted` 로
    /// 옮긴다 — F-07 이 실제로 탭을 만드는 데 성공했다면 그 자체가 신뢰의 증거다.
    #[test]
    fn report_tap_created_forces_granted_state() {
        let monitor = PermissionMonitor::start(60_000, 60_000, Box::new(|_| {}));
        monitor.report_tap_created();
        assert_eq!(monitor.state(), PermissionState::Granted);
        monitor.stop();
    }

    /// `report_tap_create_failed` 는 실제 `is_process_trusted()` 값을 그대로
    /// 판정에 반영해야 한다(§3.3) — `trusted` 면 `OutOfSync`, 아니면 `Denied`.
    ///
    /// ⚠️ 이 테스트는 macOS 실기에서 돌아갈 때, 이 프로세스(터미널을 통해
    /// 상속된 권한)의 실제 Accessibility 신뢰 여부에 따라 기대값이 달라진다
    /// — 그래서 하드코딩하지 않고 같은 API 를 직접 호출해 기대값을 구한다.
    /// 판정 로직 자체(`trusted` 분기)는 `model.rs` 의 순수 단위 테스트가
    /// 이미 macOS 없이 확정적으로 검증한다.
    #[test]
    fn report_tap_create_failed_reflects_current_trust_state() {
        let trusted = ultrakey_platform::accessibility::is_process_trusted();
        let expected = if trusted {
            PermissionState::OutOfSync
        } else {
            PermissionState::Denied
        };

        let monitor = PermissionMonitor::start(60_000, 60_000, Box::new(|_| {}));
        monitor.report_tap_create_failed();
        assert_eq!(monitor.state(), expected);
        assert_ne!(monitor.state(), PermissionState::Granted);
        monitor.stop();
    }
}
