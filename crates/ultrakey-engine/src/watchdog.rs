//! 워치독 — `docs/dev/architecture.md` §2.1. ⭐ **탭 스레드가 아니라 별도 스레드에서
//! 돈다** — 탭 런루프 자체가 막혀도(콜백이 멈춰 있어도) 감지할 수 있어야 하기 때문이다.
//! `timings.watchdog_poll_ms`(기본 1000ms) 주기로 [`TapHealthProbe::is_enabled`] 를
//! 폴링하고, `false` 면 [`EngineCommand::RecoverTap`] 을 보낸다.
//!
//! 탭이 아직 설치되지 않은 동안(`probe` 슬롯이 비어 있는 동안)은 폴링 대상이 없으므로
//! 아무 것도 하지 않는다 — `NotInstalled`/`Terminated` 상태에서 재활성화를 스팸하지
//! 않기 위함이다.

use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::Duration;

use arc_swap::ArcSwapOption;
use crossbeam_channel::{bounded, Receiver, RecvTimeoutError, Sender};

use ultrakey_platform::event_tap::TapHealthProbe;

use crate::command::{CommandChannel, EngineCommand};
use crate::state::SharedState;

pub struct Watchdog {
    thread: Option<JoinHandle<()>>,
    shutdown_tx: Sender<()>,
}

impl Watchdog {
    /// `probe` 는 탭 스레드가 탭을 만들거나 없앨 때마다 갱신하는 슬롯이다
    /// (`ultrakey_platform::event_tap::EventTap::health_probe`).
    pub fn spawn(
        shared: Arc<SharedState>,
        probe: Arc<ArcSwapOption<TapHealthProbe>>,
        commands: CommandChannel,
    ) -> Self {
        let (shutdown_tx, shutdown_rx) = bounded::<()>(0);
        let thread = thread::Builder::new()
            .name("ultrakey-watchdog".to_string())
            .spawn(move || watchdog_loop(shared, probe, commands, shutdown_rx))
            .expect("워치독 스레드 생성 실패");
        Watchdog {
            thread: Some(thread),
            shutdown_tx,
        }
    }

    /// ⚠️ `recv_timeout`/`select` 로 종료 신호를 받아 스레드를 반드시 정리한다
    /// (스레드 누수 금지 — `docs/dev/architecture.md` §2.1, 위임 지시 "스레드를 누수시키지 마라").
    pub fn shutdown(mut self) {
        let _ = self.shutdown_tx.send(());
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn watchdog_loop(
    shared: Arc<SharedState>,
    probe: Arc<ArcSwapOption<TapHealthProbe>>,
    commands: CommandChannel,
    shutdown_rx: Receiver<()>,
) {
    loop {
        let poll_ms = shared.config.load().timings.watchdog_poll_ms;
        match shutdown_rx.recv_timeout(Duration::from_millis(poll_ms)) {
            Ok(()) | Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {}
        }

        if let Some(p) = probe.load_full() {
            if !p.is_enabled() {
                tracing::warn!("watchdog: tap detected disabled; requesting re-enable");
                commands.send(EngineCommand::RecoverTap);
            }
        }
    }
    tracing::debug!("watchdog thread exiting");
}
