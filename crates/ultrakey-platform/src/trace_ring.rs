//! 무잠금(lock-free) SPSC(단일 생산자·단일 소비자) 링 버퍼 원시 자료구조.
//!
//! ⭐ 왜 여기(플랫폼 크레이트)에 있는가 — 이 자료구조는 `UnsafeCell` 로 슬롯에
//! 접근하고 그 위에 `unsafe impl Sync` 를 얹어야 스레드 간에 공유할 수 있다.
//! 이 저장소의 규약은 "이 저장소의 모든 `unsafe` 가 `ultrakey-platform` 에만
//! 있다"이다(이 크레이트 최상단 문서, `docs/dev/architecture.md` §1 표) — FFI 가
//! 아니어도 예외가 아니다. `ultrakey-engine` 은 `#![forbid(unsafe_code)]` 라 이
//! 자료구조를 직접 담을 수 없으므로, 여기 두고 `ultrakey_engine::trace::TraceRing`
//! 이 `TapTrace` 전용으로 얇게 감싼다(로직 중복 없이 안전한 API 만 노출).
//!
//! CGEventTap 콜백(생산자)에서 쓰기에 안전해야 한다: 락 없음, 힙 할당 없음
//! (`docs/dev/architecture.md` §2.2). `push`/`pop` 모두 그 조건을 지킨다.

use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

/// `T` 를 담는 고정 용량 `N` 의 SPSC 링 버퍼. **생산자는 반드시 한 스레드만,
/// 소비자도 반드시 한 스레드만**이어야 한다 — 그 이상은 이 타입의 메모리 순서
/// 보장을 벗어난다(호출자 책임).
///
/// `N` 은 2의 거듭제곱이어야 한다 — 인덱스 계산이 나눗셈 대신 비트마스크
/// (`idx & (N - 1)`)를 쓰기 때문이다. `new()` 가 `debug_assert!` 로 확인한다.
pub struct SpscRing<T: Copy, const N: usize> {
    /// 생산자만 쓰고, 소비자는 Acquire 로만 읽는다.
    head: AtomicUsize,
    /// 소비자만 쓰고, 생산자는 Acquire 로만 읽는다.
    tail: AtomicUsize,
    /// 가득 찬 상태에서 `push` 가 조용히 버린 개수 — 소비자가 폴링해 로그로 남긴다.
    dropped: AtomicU64,
    slots: [UnsafeCell<T>; N],
}

// SAFETY: `head`/`tail` 이 하나의 생산자·하나의 소비자 사이의 happens-before 를
// Acquire/Release 로 확립한다. 생산자는 자신이 마지막으로 쓴 `head` 위치까지만
// 읽고(그 이후 슬롯은 아직 유효한 값이 없어도 자기 자신은 절대 접근하지 않는다),
// 소비자는 `tail` 로 자신이 아직 읽지 않은 슬롯만 읽는다. 두 역할이 겹치는
// 슬롯에 동시 접근하는 경우가 구조적으로 없으므로 `UnsafeCell` 공유가 안전하다.
// 이 보장은 **생산자 스레드가 하나, 소비자 스레드가 하나**일 때만 성립한다 —
// 이 타입을 여러 생산자/소비자로 쓰면 이 SAFETY 근거가 깨진다(타입 문서에 명시).
unsafe impl<T: Copy, const N: usize> Sync for SpscRing<T, N> {}

impl<T: Copy, const N: usize> SpscRing<T, N> {
    /// `placeholder` 는 아직 쓰이지 않은 슬롯을 채우는 값일 뿐 절대 `pop` 으로
    /// 관측되지 않는다(`tail == head` 인 슬롯은 `pop` 이 읽지 않는다).
    pub fn new(placeholder: T) -> Self {
        debug_assert!(N.is_power_of_two(), "SpscRing 용량은 2의 거듭제곱이어야 한다");
        SpscRing {
            head: AtomicUsize::new(0),
            tail: AtomicUsize::new(0),
            dropped: AtomicU64::new(0),
            slots: std::array::from_fn(|_| UnsafeCell::new(placeholder)),
        }
    }

    /// 생산자(탭 스레드) 전용. 락 없음·힙 할당 없음 — 콜백에서 불러도 안전하다.
    /// 가득 찼으면 조용히 버리고 `dropped` 카운터만 올린다.
    pub fn push(&self, value: T) {
        let head = self.head.load(Ordering::Relaxed);
        // 소비자가 마지막으로 게시한 `tail` — Acquire 로 읽어 "그 지점까지는
        // 소비자가 이미 다 읽었다"는 것을 확인한다.
        let tail = self.tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= N {
            self.dropped.fetch_add(1, Ordering::Relaxed);
            return;
        }
        let idx = head & (N - 1);
        // SAFETY: 이 슬롯은 아직 소비자가 읽지 않았거나(비어 있음) 이미 다 읽어
        // 다시 채울 수 있는 슬롯이다(위 가득 참 확인이 보장) — 생산자는 나 하나뿐이라
        // 동시 쓰기가 없다.
        unsafe {
            *self.slots[idx].get() = value;
        }
        self.head.store(head.wrapping_add(1), Ordering::Release);
    }

    /// 소비자(드레인 스레드) 전용. 비어 있으면 `None`.
    pub fn pop(&self) -> Option<T> {
        let tail = self.tail.load(Ordering::Relaxed);
        // 생산자가 마지막으로 게시한 `head` — Acquire 로 읽어 그 슬롯에 쓰인 값이
        // 이 스레드에서도 보이게 한다.
        let head = self.head.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let idx = tail & (N - 1);
        // SAFETY: `tail != head` 이므로 이 슬롯은 생산자가 이미 다 쓴 뒤다. 소비자는
        // 나 하나뿐이라 동시 읽기가 없고, 생산자는 이 슬롯을 `head` 가 한 바퀴
        // 돌아오기 전까지 다시 건드리지 않는다(위 `push` 의 가득 참 확인).
        let value = unsafe { *self.slots[idx].get() };
        self.tail.store(tail.wrapping_add(1), Ordering::Release);
        Some(value)
    }

    /// 가득 찬 상태에서 버려진 누적 개수.
    pub fn dropped_count(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pop_on_empty_is_none() {
        let ring: SpscRing<u32, 4> = SpscRing::new(0);
        assert_eq!(ring.pop(), None);
    }

    #[test]
    fn push_pop_is_fifo() {
        let ring: SpscRing<u32, 4> = SpscRing::new(0);
        ring.push(1);
        ring.push(2);
        ring.push(3);
        assert_eq!(ring.pop(), Some(1));
        assert_eq!(ring.pop(), Some(2));
        assert_eq!(ring.pop(), Some(3));
        assert_eq!(ring.pop(), None);
    }

    #[test]
    fn push_drops_and_counts_when_full() {
        let ring: SpscRing<u32, 4> = SpscRing::new(0);
        for i in 0..4 {
            ring.push(i);
        }
        assert_eq!(ring.dropped_count(), 0);
        // 가득 찬 상태에서 두 번 더 push — 조용히 버려지고 카운터만 오른다.
        ring.push(100);
        ring.push(101);
        assert_eq!(ring.dropped_count(), 2);
        // 기존 내용은 그대로 FIFO 로 남아 있어야 한다(덮어쓰기 없음).
        assert_eq!(ring.pop(), Some(0));
        assert_eq!(ring.pop(), Some(1));
        assert_eq!(ring.pop(), Some(2));
        assert_eq!(ring.pop(), Some(3));
        assert_eq!(ring.pop(), None);
    }

    #[test]
    fn wraps_around_after_drain() {
        let ring: SpscRing<u32, 4> = SpscRing::new(0);
        for i in 0..4 {
            ring.push(i);
        }
        assert_eq!(ring.pop(), Some(0));
        assert_eq!(ring.pop(), Some(1));
        // 두 자리를 비웠으니 두 개 더 채울 수 있다 — 링을 감아 돈다.
        ring.push(4);
        ring.push(5);
        assert_eq!(ring.pop(), Some(2));
        assert_eq!(ring.pop(), Some(3));
        assert_eq!(ring.pop(), Some(4));
        assert_eq!(ring.pop(), Some(5));
        assert_eq!(ring.pop(), None);
    }
}
