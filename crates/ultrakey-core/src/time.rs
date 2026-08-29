//! 밀리초 단위 시간값 — **반드시 호출자가 주입한다.**
//!
//! 이 크레이트는 `std::time::Instant`/`SystemTime` 을 직접 호출하지 않는다. quick press/hold
//! 판정은 항상 `Millis` 를 인자로 받아, 실제 시계 없이도 결정론적으로 테스트할 수 있어야 한다
//! (`key-remapping-engine.md` §3-c).

/// 밀리초 단위 시각 또는 지속시간. 의미는 호출자가 정한다(예: 단조 증가 타임스탬프).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Millis(pub u64);

impl Millis {
    pub const ZERO: Millis = Millis(0);

    /// `self - other`. 언더플로 대신 0으로 saturate 한다(타이머 판정에서 음수 경과시간이
    /// 나오는 것을 막기 위함 — 예: 시계가 뒤로 가는 비정상 상황에서도 패닉하지 않는다).
    pub fn saturating_sub(self, other: Self) -> Self {
        Millis(self.0.saturating_sub(other.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saturating_sub_does_not_underflow() {
        assert_eq!(Millis(100).saturating_sub(Millis(40)), Millis(60));
        assert_eq!(Millis(10).saturating_sub(Millis(100)), Millis(0));
    }

    #[test]
    fn ordering_works() {
        assert!(Millis(10) < Millis(20));
        assert!(Millis(20) >= Millis(20));
    }
}
