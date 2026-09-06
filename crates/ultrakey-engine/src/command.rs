//! 탭 스레드로 보내는 명령 — `docs/dev/architecture.md` §2.2 네 번째 행.
//!
//! ⭐ **불변식**: 명령은 무잠금 MPSC 큐(`crossbeam_channel::unbounded`)에 넣고
//! [`ultrakey_platform::runloop::CommandSignaller::signal`] 로 탭 스레드의 런루프를
//! 깨운다. 큐를 실제로 비우는 쪽은 탭 스레드에 등록된
//! [`ultrakey_platform::runloop::CommandSource`] 의 perform 콜백이다 — 그 콜백은 탭
//! 이벤트 콜백·quick press 타이머 콜백과 **같은 런루프가, 같은 스레드에서, 직렬로만**
//! 호출한다. 따라서 명령을 처리하는 동안 `Arbiter`(정본 키 상태 테이블)에 **락 없이**
//! `&mut` 로 접근할 수 있다 — 이 큐가 있기 때문에 탭 스레드 바깥(메인·워치독·지연
//! 스케줄러)에서 상태를 바꿀 방법이 "명령을 큐에 넣고 깨운다" 하나로 좁혀지고, 그
//! 결과로 정본 상태에 진짜 동시 접근이 원천적으로 없어진다.

use crossbeam_channel::{unbounded, Receiver, Sender};

use ultrakey_core::arbitration::SynthEvent;
use ultrakey_platform::runloop::CommandSignaller;

/// 탭 스레드가 처리하는 명령 하나.
///
/// ⚠️ `PostSynthEvent(SynthEvent)` 가 있어 더 이상 `Copy` 가 아니다 — 값을 옮겨(move)
/// 보낸다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineCommand {
    /// 절전/잠금/Secure Input — stuck modifier 방지(§5 #9). `Arbiter::force_reset` 을
    /// 호출하고, 만들어진 off-flagsChanged 합성 이벤트를 방출한다.
    ForceResetState,
    /// 탭 상태 확인(워치독의 비정상 감지, 또는 탭 자신의 비활성화 통지). 트램폴린의
    /// 연속 재활성화 예산이 소진됐거나 권한이 없으면 탭을 해체한다 — 재생성은
    /// 시도하지 않는다(이슈 #65 Phase 1 리뷰 교정 2, `engine::handle_recover_tap`
    /// 문서 참고).
    RecoverTap,
    /// 설정 교체 후 `Arbiter::reconfigure` 호출 — quick press 슬롯 재구성.
    Reconfigure,
    /// ⭐ 이슈 #129 — 인풋 박스 세션 중 D2(F-16.1 세션 재평가)가 낸 합성 이벤트를
    /// 콜백 밖에서 낸다(`engine::emit_outcome`, `docs/plan/issue-129-seek-webview-inputsource.md`
    /// §7.4 순위 1). 판정 로직이 이미 만든 `SynthEvent` 를 그대로 실어 보낼 뿐,
    /// 이 커맨드 자체는 새 판정을 하지 않는다.
    PostSynthEvent(SynthEvent),
    /// 탭 스레드 런루프를 정지하고 종료한다.
    Shutdown,
}

/// 탭 스레드로 명령을 보내는 손잡이. `Sender` + `CommandSignaller` 를 한 쌍으로 묶어
/// "큐에 넣고 깨운다"를 항상 함께 실행하도록 강제한다 — 신호 없이 큐에만 넣으면 탭
/// 스레드가 잠들어 있는 동안 명령이 무한정 지연될 수 있다.
#[derive(Clone)]
pub struct CommandChannel {
    sender: Sender<EngineCommand>,
    signaller: CommandSignaller,
}

impl CommandChannel {
    pub(crate) fn new(sender: Sender<EngineCommand>, signaller: CommandSignaller) -> Self {
        CommandChannel { sender, signaller }
    }

    /// 명령을 큐에 넣고 탭 스레드의 런루프를 깨운다. 탭 스레드가 이미 종료돼 큐가
    /// 끊어졌으면 조용히 무시한다 — 종료 경합 상황에서 패닉하지 않기 위함이다.
    pub fn send(&self, cmd: EngineCommand) {
        if self.sender.send(cmd).is_ok() {
            self.signaller.signal();
        }
    }
}

/// `CommandChannel`/`CommandSource` 양쪽에 필요한 (Sender, Receiver) 쌍을 만든다.
pub(crate) fn channel() -> (Sender<EngineCommand>, Receiver<EngineCommand>) {
    unbounded()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 테스트 #5 — 명령 큐 처리 순서: `crossbeam_channel::unbounded` 는 FIFO 를
    /// 보장하므로, 탭 스레드의 perform 콜백이 순서대로 드레인하면 명령은 보낸
    /// 순서 그대로 처리된다. `CommandSignaller` 는 macOS 전용 실제 `CFRunLoopSource`
    /// 가 필요해 여기서는 큐 자체(플랫폼 비의존)만 검증한다 — 신호(signal) 메커니즘은
    /// `ultrakey-platform` 의 책임이고, 이 크레이트가 보장해야 하는 것은 "큐에 넣은
    /// 순서 == perform 콜백이 드레인하는 순서" 뿐이다.
    #[test]
    fn commands_are_drained_in_fifo_order() {
        let (tx, rx) = channel();
        tx.send(EngineCommand::ForceResetState).unwrap();
        tx.send(EngineCommand::RecoverTap).unwrap();
        tx.send(EngineCommand::Reconfigure).unwrap();

        let mut drained = Vec::new();
        while let Ok(cmd) = rx.try_recv() {
            drained.push(cmd);
        }

        assert_eq!(
            drained,
            vec![
                EngineCommand::ForceResetState,
                EngineCommand::RecoverTap,
                EngineCommand::Reconfigure,
            ]
        );
    }

    #[test]
    fn try_recv_is_empty_when_nothing_sent() {
        let (_tx, rx) = channel();
        assert!(rx.try_recv().is_err());
    }
}
