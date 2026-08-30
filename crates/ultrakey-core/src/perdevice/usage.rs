//! 기능 2(디바이스별 Function Keys)의 시스템 기능 12종 — HID usage 닫힌 어휘 표.
//!
//! CONTRACT.md(이 세션의 F-17 구현 계약) §2.2 를 그대로 코드로 옮긴 것이다.
//!
//! ⛔ 이 표에 없는 값을 지어내지 않는다. [`SystemFunction::hid_usage`] 가
//! `MissionControl`·`Spotlight`·`Dictation`·`DoNotDisturb` 4종에 대해 `None` 인
//! 것은 명세(`docs/spec/per-device-settings.md` §3.5)가 후보값조차 제시하지 않은
//! 것을 그대로 반영한 결과다 — 표준 Consumer Page 대응이 있는지 자체가 불분명하고,
//! Apple 벤더 정의 usage page(비공개)를 요구할 가능성이 있다.
//!
//! ⚠️ `SourceKey::hid_usage()`(기능 1 의 from/to 어휘, 35종)는 이 파일이 아니라
//! `keycode.rs` 의 기존 `impl SourceKey` 블록에 있다 — `keycode()` 메서드와 나란히
//! 두는 것이 그 파일의 기존 관례(물리 표현 메서드는 타입이 정의된 자리에 둔다)와
//! 맞기 때문이다. 이 파일은 `SystemFunction`(이 모듈이 새로 정의한 타입) 전용 표만
//! 다룬다.

use super::SystemFunction;

/// 근거 등급 — 명세 전반이 쓰는 (실측)/(표준 참고값)/(미확정) 구분을 코드로 옮긴
/// 것(CONTRACT.md §2.2). `docs/dev/architecture.md` §4 의 "실측 대 설계 판단" 구분과
/// 같은 정신이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// 키 입력으로 직접 확인됨(스파이크 S-8 — `volume_increment` 하나뿐).
    Measured,
    /// USB HID Usage Tables 표준 문서 정의 기준 후보값 — 이 프로젝트가 직접 검증한
    /// 사실이 아니다(`docs/spec/per-device-settings.md` §3.5 "후보값" 열과 동일 등급).
    StandardTable,
    /// 후보값조차 없음 — 표준 Consumer Page 대응이 있는지 자체가 불분명하다.
    Unknown,
}

impl SystemFunction {
    /// `(0x0C << 32) | usage`(Consumer Page) — 경로 B(`UserKeyMapping`)의 목적지
    /// (`Dst`)로 쓸 수 있는 값. `MissionControl`·`Spotlight`·`Dictation`·
    /// `DoNotDisturb` 4종은 `None` — ⛔ **값을 지어내지 않는다**(CONTRACT.md §2.2,
    /// `docs/spec/per-device-settings.md` §3.5).
    pub fn hid_usage(self) -> Option<u64> {
        use SystemFunction::*;
        const CONSUMER_PAGE: u64 = 0x0C << 32;
        let usage: u64 = match self {
            // ⭐ 실측(스파이크 S-8) — 키 입력으로 "볼륨이 실제로 올라간다"를 확인한
            // 유일한 값. 정확히 실측된 명제는 "0xC000000E9 를 Dst 로 쓰면 볼륨이
            // 올라간다"이며, F-키 위치(F12)가 검증된 것은 아니다(§3.5 각주).
            VolumeUp => 0xE9,
            // 아래 8종은 전부 표준 참고값 — 동작은 검증되지 않았다(§9 질문 5).
            VolumeDown => 0xEA,
            Mute => 0xE2,
            PlayPause => 0xCD,
            FastForward => 0xB3,
            Rewind => 0xB4,
            DisplayBrightnessUp => 0x6F,
            DisplayBrightnessDown => 0x70,
            // ⛔ 후보값조차 없다 — Apple 고유 시스템 제스처라 표준 usage 자체가 없을
            // 가능성이 있다(§3.5, §9 질문 6).
            MissionControl | Spotlight | Dictation | DoNotDisturb => return None,
        };
        Some(CONSUMER_PAGE | usage)
    }

    /// 이 기능의 [`SystemFunction::hid_usage`] 값이 갖는 근거 등급.
    pub fn evidence(self) -> Evidence {
        use SystemFunction::*;
        match self {
            VolumeUp => Evidence::Measured,
            VolumeDown | Mute | PlayPause | FastForward | Rewind | DisplayBrightnessUp
            | DisplayBrightnessDown => Evidence::StandardTable,
            MissionControl | Spotlight | Dictation | DoNotDisturb => Evidence::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// CONTRACT.md §2.2 표 — `VolumeUp` 만 유일하게 실측(스파이크 S-8, 키 입력으로
    /// 확인). 기대값을 구현이 쓰는 상수에서 가져오지 않고 실측 리터럴을 직접 적는다
    /// (PR #17 함정 회피 원칙, `keycode.rs` 의 `modifier_flags` 테스트와 동일).
    #[test]
    fn volume_up_hid_usage_matches_measured_value() {
        assert_eq!(SystemFunction::VolumeUp.hid_usage(), Some(0xC000000E9));
        assert_eq!(SystemFunction::VolumeUp.evidence(), Evidence::Measured);
    }

    /// ⛔ 값을 지어내지 않는다 — 후보값조차 없는 4종은 정확히 이 4종이어야 한다
    /// (`docs/spec/per-device-settings.md` §3.5).
    #[test]
    fn exactly_four_functions_have_no_hid_usage() {
        let none_functions: Vec<SystemFunction> = SystemFunction::all()
            .iter()
            .copied()
            .filter(|f| f.hid_usage().is_none())
            .collect();

        assert_eq!(
            none_functions,
            vec![
                SystemFunction::MissionControl,
                SystemFunction::Spotlight,
                SystemFunction::Dictation,
                SystemFunction::DoNotDisturb,
            ]
        );
        for f in &none_functions {
            assert_eq!(f.evidence(), Evidence::Unknown, "{f:?}");
        }
    }

    /// 나머지 8종(`VolumeUp` 제외)은 전부 `Some` — 등급은 `StandardTable`.
    #[test]
    fn remaining_eight_functions_have_standard_table_candidates() {
        for f in SystemFunction::all() {
            if *f != SystemFunction::VolumeUp {
                assert!(f.hid_usage().is_some() || f.evidence() == Evidence::Unknown, "{f:?}");
            }
            if f.hid_usage().is_some() && *f != SystemFunction::VolumeUp {
                assert_eq!(f.evidence(), Evidence::StandardTable, "{f:?}");
            }
        }
    }

    /// 기능 2 선택 팝업 UI 규약(CONTRACT.md §2.2) — `hid_usage()` 가 `Some` 인 8종만
    /// 팝업 항목으로 낸다. 이 테스트는 그 판정 함수(`hid_usage().is_some()`)가 정확히
    /// 8종을 골라내는지만 검증한다(팝업 자체는 UI 계층의 몫).
    #[test]
    fn eight_functions_are_selectable_in_feature2_popup() {
        let selectable = SystemFunction::all()
            .iter()
            .filter(|f| f.hid_usage().is_some())
            .count();
        assert_eq!(selectable, 8);
    }
}
