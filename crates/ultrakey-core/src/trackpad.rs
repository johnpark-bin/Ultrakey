//! F-06 트랙패드 제스처 게이트 — `docs/spec/trackpad-hyper-gesture.md` §3.4 계약의
//! "게시(publish)/소비(consume)" 원자 쌍.
//!
//! `korean.rs` 의 [`crate::korean::AtomicKoreanImeGate`] 와 같은 비대칭 규약이다:
//! 트랙패드 리스너 스레드가 `store` 하고, 탭 콜백(`ultrakey-engine`)이 매 이벤트
//! O(1)·무할당으로 `load` 한다. `GateSnapshot` 필드 두 개(`trackpad_hyper_active`
//! ·`trackpad_freeze_cursor`)의 단일 출처다.
//!
//! ⭐ 이 크레이트는 판정을 하지 않는다 — 제스처 인식은 `ultrakey-hyperkey` 의
//! 순수 상태 머신이, 원시 프레임은 `ultrakey-platform::multitouch` 의 비공개 FFI 가
//! 담당한다. 여기는 그 결과가 흐르는 **공유 자리**만 정의한다. 초기값이
//! [`TrackpadPhase::Off`] 인 것이 fail-closed 다: 리스너가 살아 있지 않아도(비공개
//! API 격하, §8) 중재기는 "트랙패드 소스의 hyper 요청"이 없다고 읽는다.

use std::sync::atomic::{AtomicU8, Ordering};

/// 트랙패드 제스처 상태 머신(`trackpad-hyper-gesture.md` §3.2)의 중재기 관점 압축 표현.
///
/// `Off` 가 기본값이자 fail-closed 다 — 리스너가 아직 게시한 적이 없거나(기동 전),
/// 격하(비공개 API 로드 실패)되어 아예 존재하지 않아도 게이트는 `Off` 이고
/// 중재기는 기존과 완전히 동일하게 판정한다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackpadPhase {
    /// 제스처 진행 없음 — hyper 요청 없음, 커서 이동을 소비하지 않는다.
    #[default]
    Off,
    /// 진입 영역에 접촉이 시작됐으나 아직 프리즈 임계에 도달하지 않았다.
    /// hyper 요청 없음 — 아무 부수효과도 없다.
    Contact,
    /// 프리즈 임계 도달 — 마우스 이동 이벤트를 소비한다(§8 마지막 항목).
    /// hyper 는 아직 미활성.
    Frozen,
    /// 트리거 임계 도달 — hyper 활성 요청이 유효하다. 접촉 종료까지 유지.
    Engaged,
}

/// 트랙패드 리스너가 `store` 하고 탭 콜백이 `load` 만 하는 원자 게이트.
/// `gate.rs`·`korean.rs` 의 게이트와 같은 규약 — 콜백 임계 경로에서 락·할당이 없다.
pub struct AtomicTrackpadPhase {
    phase: AtomicU8,
}

impl AtomicTrackpadPhase {
    pub fn new() -> Self {
        AtomicTrackpadPhase {
            phase: AtomicU8::new(encode(TrackpadPhase::Off)),
        }
    }

    /// 중재기 쪽(탭 콜백). `Acquire` — 리스너의 `Release` 게시를 이 로드 이후의
    /// 판정이 관찰하도록 보장한다.
    pub fn load(&self) -> TrackpadPhase {
        decode(self.phase.load(Ordering::Acquire))
    }

    /// 리스너(트랙패드 스레드)가 부르는 쪽.
    pub fn store(&self, phase: TrackpadPhase) {
        self.phase.store(encode(phase), Ordering::Release);
    }
}

impl Default for AtomicTrackpadPhase {
    fn default() -> Self {
        Self::new()
    }
}

fn encode(phase: TrackpadPhase) -> u8 {
    match phase {
        TrackpadPhase::Off => 0,
        TrackpadPhase::Contact => 1,
        TrackpadPhase::Frozen => 2,
        TrackpadPhase::Engaged => 3,
    }
}

fn decode(raw: u8) -> TrackpadPhase {
    match raw {
        0 => TrackpadPhase::Off,
        1 => TrackpadPhase::Contact,
        2 => TrackpadPhase::Frozen,
        _ => TrackpadPhase::Engaged,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_off_and_round_trips() {
        assert_eq!(AtomicTrackpadPhase::default().load(), TrackpadPhase::Off);
        for phase in [
            TrackpadPhase::Off,
            TrackpadPhase::Contact,
            TrackpadPhase::Frozen,
            TrackpadPhase::Engaged,
        ] {
            let gate = AtomicTrackpadPhase::new();
            gate.store(phase);
            assert_eq!(gate.load(), phase);
        }
    }
}