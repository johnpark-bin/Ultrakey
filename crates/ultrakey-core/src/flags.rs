//! `CGEventFlags` 에 대응하는 비트마스크 modifier 표현.
//!
//! ⭐ hyper/meh/bleh 는 "command 포함?"/"shift 포함?" 같은 불리언 여러 개가 아니라
//! **단일 비트마스크 정수**로 저장·연산된다(`hyperkey.md` §3.1, §7 판정 — 실측:
//! `hyperFlags = 1966080`). 이 모듈이 그 표현을 정의한다.

use std::ops::{BitAnd, BitOr, BitOrAssign, Not};

use serde::{Deserialize, Serialize};

/// `CGEventFlags` 비트마스크 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventFlags(pub u64);

impl EventFlags {
    /// 비어 있는(모디파이어 없음) 플래그.
    pub const NONE: EventFlags = EventFlags(0);

    pub const SHIFT: EventFlags = EventFlags(0x20000);
    pub const CONTROL: EventFlags = EventFlags(0x40000);
    pub const ALTERNATE: EventFlags = EventFlags(0x80000);
    pub const COMMAND: EventFlags = EventFlags(0x100000);
    /// caps lock 의 `CGEventFlags` 비트(잠금 상태 자체가 아니라 이벤트 플래그 비트).
    pub const CAPS_LOCK: EventFlags = EventFlags(0x10000);
    /// `kCGEventFlagMaskSecondaryFn` — globe/fn 키.
    pub const SECONDARY_FN: EventFlags = EventFlags(0x800000);

    // ── 좌/우 구분 비트(device-dependent masks) ────────────────────────────────
    //
    // ⭐ 왜 필요한가 (2026-08-30, 이슈 #19). 위의 `SHIFT`/`CONTROL`/`ALTERNATE`/
    // `COMMAND` 는 **좌우를 구분하지 않는** 일반 마스크다. macOS 가 실제로 보내는
    // modifier `flagsChanged` 이벤트에는 그 일반 비트와 **함께** 어느 쪽 키인지를
    // 나타내는 device-dependent 비트가 항상 실려 온다. 우리가 modifier 를 합성해
    // 내보낼 때 일반 비트만 얹으면, 좌우를 구분해 읽는 수신자(브라우저의
    // `KeyboardEvent.code`, 좌우를 구분하는 앱 단축키)에게는 **실물과 다른 모양**의
    // 이벤트가 된다. `docs/spec/key-remapping-engine.md` §5 #18 이 입력 판정에서
    // "좌/우 shift 가 같은 비트를 공유해 구분할 수 없다"고 지적한 바로 그 문제의
    // **출력 쪽 대응**이다 — 입력에서는 정본 눌림 테이블로 우회했고, 출력에서는
    // 이 비트를 실어 해결한다.
    //
    // 값의 출처: `IOKit/hidsystem/IOLLEvent.h` 의 `NX_DEVICE*KEYMASK` 상수.
    /// `NX_DEVICELCTLKEYMASK`.
    pub const DEVICE_LEFT_CONTROL: EventFlags = EventFlags(0x00000001);
    /// `NX_DEVICELSHIFTKEYMASK`.
    pub const DEVICE_LEFT_SHIFT: EventFlags = EventFlags(0x00000002);
    /// `NX_DEVICERSHIFTKEYMASK`.
    pub const DEVICE_RIGHT_SHIFT: EventFlags = EventFlags(0x00000004);
    /// `NX_DEVICELCMDKEYMASK`.
    pub const DEVICE_LEFT_COMMAND: EventFlags = EventFlags(0x00000008);
    /// `NX_DEVICERCMDKEYMASK`.
    pub const DEVICE_RIGHT_COMMAND: EventFlags = EventFlags(0x00000010);
    /// `NX_DEVICELALTKEYMASK`.
    pub const DEVICE_LEFT_OPTION: EventFlags = EventFlags(0x00000020);
    /// `NX_DEVICERALTKEYMASK`.
    pub const DEVICE_RIGHT_OPTION: EventFlags = EventFlags(0x00000040);
    /// `NX_DEVICERCTLKEYMASK`.
    pub const DEVICE_RIGHT_CONTROL: EventFlags = EventFlags(0x00002000);

    /// hyper (`Include shift in hyper key` ☑, 기본) = `⌃⌥⌘⇧`.
    ///
    /// ⭐ 실측: `~/Library/Preferences/com.knollsoft.Superkey.plist` 의
    /// `hyperFlags = 1966080` (`hyperkey.md` §3.1, app-bundle-analysis.md §2.1).
    /// `1966080 = 0x1E0000 = SHIFT(0x20000) | CONTROL(0x40000) | ALTERNATE(0x80000) | COMMAND(0x100000)`.
    pub const HYPER_WITH_SHIFT: EventFlags = EventFlags(
        Self::SHIFT.0 | Self::CONTROL.0 | Self::ALTERNATE.0 | Self::COMMAND.0,
    );

    /// hyper (`Include shift in hyper key` ☐) = `⌃⌥⌘`(shift 제외).
    pub const HYPER_NO_SHIFT: EventFlags =
        EventFlags(Self::CONTROL.0 | Self::ALTERNATE.0 | Self::COMMAND.0);

    /// meh = `⌃⌥⇧` — command 제외, shift 고정 포함(`hyperkey.md` §3.1).
    pub const MEH: EventFlags = EventFlags(Self::CONTROL.0 | Self::ALTERNATE.0 | Self::SHIFT.0);

    /// bleh = `⌃⌘⇧`.
    ///
    /// ⭐ option(`ALTERNATE`) 은 어떤 경우에도 포함되지 않는다 — v1.65 가
    /// "Fixes a bug where the bleh key could also contain the option key" 로 명시적으로
    /// 고친 회귀다(`hyperkey.md` §3.1). 이 상수에 `ALTERNATE` 를 넣지 않는 것이
    /// 그 회귀 방지의 코드상 표현이다.
    pub const BLEH: EventFlags = EventFlags(Self::CONTROL.0 | Self::COMMAND.0 | Self::SHIFT.0);

    /// `other` 의 모든 비트가 `self` 에도 켜져 있는지.
    pub fn contains(self, other: EventFlags) -> bool {
        self.0 & other.0 == other.0
    }

    pub fn insert(&mut self, other: EventFlags) {
        self.0 |= other.0;
    }

    pub fn remove(&mut self, other: EventFlags) {
        self.0 &= !other.0;
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }
}

impl Default for EventFlags {
    fn default() -> Self {
        EventFlags::NONE
    }
}

impl BitOr for EventFlags {
    type Output = EventFlags;
    fn bitor(self, rhs: Self) -> Self::Output {
        EventFlags(self.0 | rhs.0)
    }
}

impl BitAnd for EventFlags {
    type Output = EventFlags;
    fn bitand(self, rhs: Self) -> Self::Output {
        EventFlags(self.0 & rhs.0)
    }
}

impl BitOrAssign for EventFlags {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl Not for EventFlags {
    type Output = EventFlags;
    fn not(self) -> Self::Output {
        EventFlags(!self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐ 실측 회귀 방지: hyperFlags 저장값과 정확히 일치해야 한다.
    #[test]
    fn hyper_with_shift_matches_measured_plist_value() {
        assert_eq!(EventFlags::HYPER_WITH_SHIFT.0, 1_966_080);
    }

    /// ⭐ v1.65 회귀 방지: bleh 는 option(ALTERNATE) 을 포함하지 않는다.
    #[test]
    fn bleh_does_not_contain_alternate() {
        assert!(!EventFlags::BLEH.contains(EventFlags::ALTERNATE));
        assert!(EventFlags::BLEH.contains(EventFlags::CONTROL));
        assert!(EventFlags::BLEH.contains(EventFlags::COMMAND));
        assert!(EventFlags::BLEH.contains(EventFlags::SHIFT));
    }

    #[test]
    fn meh_excludes_command() {
        assert!(!EventFlags::MEH.contains(EventFlags::COMMAND));
        assert!(EventFlags::MEH.contains(EventFlags::CONTROL));
        assert!(EventFlags::MEH.contains(EventFlags::ALTERNATE));
        assert!(EventFlags::MEH.contains(EventFlags::SHIFT));
    }

    #[test]
    fn hyper_no_shift_excludes_shift_only() {
        assert!(!EventFlags::HYPER_NO_SHIFT.contains(EventFlags::SHIFT));
        assert!(EventFlags::HYPER_NO_SHIFT.contains(EventFlags::CONTROL));
        assert!(EventFlags::HYPER_NO_SHIFT.contains(EventFlags::ALTERNATE));
        assert!(EventFlags::HYPER_NO_SHIFT.contains(EventFlags::COMMAND));
    }

    #[test]
    fn bit_ops_round_trip() {
        let mut f = EventFlags::NONE;
        f.insert(EventFlags::SHIFT);
        f |= EventFlags::CONTROL;
        assert!(f.contains(EventFlags::SHIFT));
        assert!(f.contains(EventFlags::CONTROL));
        f.remove(EventFlags::SHIFT);
        assert!(!f.contains(EventFlags::SHIFT));
        assert!(f.contains(EventFlags::CONTROL));
        assert!(!f.is_empty());
        assert!(EventFlags::NONE.is_empty());
    }
}
