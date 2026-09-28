//! 클릭 모드 7종과 modifier → 모드 해석.
//!
//! `Change click modes with modifier keys` 의 실체는 좌/우/중간 클릭이 아니라
//! **클릭 지점·횟수·커서 복귀 여부의 조합 7종 프리셋**이다(명세 §1.1, §3.3).
//! 이 모듈은 확정 순간의 modifier 스냅샷(§3.1 1단계)을 모드로 바꾸는 순수
//! 함수를 담는다.
//!
//! ⭐ **결정 기록(이슈 #44) — 아래 매핑표는 클론의 설계 결정(미검증)이다.**
//! 원본의 modifier↔모드 대응은 실측되지 않았고(명세 §9 Q10), "목록 순서 = enum
//! 순서"류의 추측으로 매핑을 만들지 않기 위해 학습 비용·실수 위험 설계 원칙으로
//! 정했다. 원본 실기 대조 뒤 교정이 필요하다:
//! - **기본 안전성**: 무-modifier 는 항상 `clickStartMatch` — 사용자가 modifier
//!   를 몰라도 제품 핵심 약속("매치를 클릭한다")이 성립한다.
//! - **⌘-복사 연상**: macOS 복사 = ⌘(⌘C) → `doubleClickCopy` 를 ⌘ 에 배정.
//! - **인접 차이의 명확성**: 기본(단일 클릭·시작 지점)과 ⌘(더블클릭+복사)는
//!   결과가 확연히 달라 "modifier 가 먹지 않았다"를 즉시 알아차릴 수 있다.
//! - **복합 조합은 고급/저위험 모드**: ⌘⌥(강한 복사 연상) → `tripleClickCopy`,
//!   ⌃⌥(실수 위험 큰 조합) → 클릭을 유발하지 않는 `onlyMoveCursor`.

use ultrakey_core::flags::EventFlags;

/// `Change click modes with modifier keys` 가 가리키는 7종 클릭 모드(명세 §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickMode {
    /// `Just move cursor` — 클릭하지 않고 커서만 매치 위치로 이동한다.
    OnlyMoveCursor,
    /// `Click at beginning of match` — 매치 **시작 지점** 단일 클릭(기본 모드).
    ClickStartMatch,
    /// `Click at end of match` — 매치 **끝 지점** 단일 클릭.
    ClickEndMatch,
    /// `Click and return cursor` — 클릭 후 커서를 원래 위치로 되돌린다.
    ClickAndReturn,
    /// `Click, return, click` — 클릭 → 커서 원위치 복귀 → 원위치에서 재클릭.
    ClickReturnClick,
    /// `Double click and copy` — 더블클릭으로 단어 선택 + 클립보드 복사(D11).
    DoubleClickCopy,
    /// `Triple click and copy` — 트리플클릭으로 줄/문단 선택 + 복사(D11).
    TripleClickCopy,
}

/// 모드 매칭에 쓰는 **주 4종 비트(⇧⌃⌥⌘)**. 확정 시점 스냅샷에는 캡스락 잠금
/// 비트(`alphaShift`)·fn·장치 좌우 구분 비트가 함께 실려 올 수 있으므로,
/// 매칭은 이 마스크로 좁힌 정확 일치로만 한다 — `SeekConfig::
/// matches_global_shortcut` 와 같은 관례(명세 §5 #10).
pub const MODIFIER_MASK: EventFlags = EventFlags(
    EventFlags::SHIFT.0 | EventFlags::CONTROL.0 | EventFlags::ALTERNATE.0 | EventFlags::COMMAND.0,
);

// D1 표의 7조합 상수 — 매칭 분기를 읽기 쉽게 이름만 붙인다.
const NO_MODS: EventFlags = EventFlags(0);
const CMD: EventFlags = EventFlags(EventFlags::COMMAND.0);
const OPT: EventFlags = EventFlags(EventFlags::ALTERNATE.0);
const CTRL: EventFlags = EventFlags(EventFlags::CONTROL.0);
const SHIFT: EventFlags = EventFlags(EventFlags::SHIFT.0);
const CMD_OPT: EventFlags = EventFlags(EventFlags::COMMAND.0 | EventFlags::ALTERNATE.0);
const CTRL_OPT: EventFlags = EventFlags(EventFlags::CONTROL.0 | EventFlags::ALTERNATE.0);

/// 확정 시점의 modifier 스냅샷을 클릭 모드로 해석한다(§3.1 3단계).
///
/// - `change_modes_enabled == false`(설정 OFF): 모드는 언제나 기본
///   [`ClickMode::ClickStartMatch`] 이고, modifier 는 클릭 **이벤트**에 실린다
///   (부제 원문: "If this setting is disabled, modifiers will be applied to the
///   click") — 그 플래그 변환은 [`plan::passthrough_flags`] 가 맡는다.
/// - `change_modes_enabled == true`(설정 ON, 출고 기본값): 아래 D1 표로 해석.
///
/// ⚠️ **미정의 조합**(⌘⇧, ⌃⇧ 등)은 기본 모드로 안전하게 떨어진다(Q10-c) —
/// 우선순위 규칙을 지어내지 않는다는 클론 설계 결정(이슈 #44).
#[must_use]
pub fn resolve_click_mode(change_modes_enabled: bool, modifiers: EventFlags) -> ClickMode {
    if !change_modes_enabled {
        return ClickMode::ClickStartMatch;
    }
    match EventFlags(modifiers.0 & MODIFIER_MASK.0) {
        NO_MODS => ClickMode::ClickStartMatch,
        CMD => ClickMode::DoubleClickCopy,
        OPT => ClickMode::ClickEndMatch,
        CTRL => ClickMode::ClickAndReturn,
        SHIFT => ClickMode::ClickReturnClick,
        CMD_OPT => ClickMode::TripleClickCopy,
        CTRL_OPT => ClickMode::OnlyMoveCursor,
        _ => ClickMode::ClickStartMatch,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 출고 기본값(OFF 가 아니라 'ON + 무-modifier')에서 아무 modifier 도 없으면
    /// 기본 모드다(Q10-b — 결정 D2).
    #[test]
    fn no_modifiers_with_modes_enabled_is_click_start_match() {
        assert_eq!(
            resolve_click_mode(true, EventFlags::NONE),
            ClickMode::ClickStartMatch
        );
    }

    /// D1 표 7행 — 각 조합이 정확히 그 모드로 해석된다(§8 수용 기준 3·4).
    #[test]
    fn enabled_maps_all_seven_combinations() {
        let cases = [
            (EventFlags::NONE, ClickMode::ClickStartMatch),
            (EventFlags::COMMAND, ClickMode::DoubleClickCopy),
            (EventFlags::ALTERNATE, ClickMode::ClickEndMatch),
            (EventFlags::CONTROL, ClickMode::ClickAndReturn),
            (EventFlags::SHIFT, ClickMode::ClickReturnClick),
            (
                EventFlags::COMMAND | EventFlags::ALTERNATE,
                ClickMode::TripleClickCopy,
            ),
            (
                EventFlags::CONTROL | EventFlags::ALTERNATE,
                ClickMode::OnlyMoveCursor,
            ),
        ];
        for (mods, expected) in cases {
            assert_eq!(
                resolve_click_mode(true, mods),
                expected,
                "조합 {:?} 가 {expected:?} 이어야 한다",
                mods
            );
        }
    }

    /// ⚠️ 매치 대상은 마스킹된 주 4종이다 — 캡스락·fn·장치 좌/우 구분 비트가
    /// 섞여 와도 같은 조합으로 해석된다(명세 §5 #10, `matches_global_shortcut`
    /// 관례).
    #[test]
    fn caps_lock_and_fn_bits_are_masked_out_of_matching() {
        let noisy = EventFlags::COMMAND | EventFlags::CAPS_LOCK | EventFlags::SECONDARY_FN;
        assert_eq!(
            resolve_click_mode(true, noisy),
            ClickMode::DoubleClickCopy,
            "⌘+캡스락+fn 은 ⌘ 와 같은 모드여야 한다"
        );

        let device_bits = EventFlags(
            EventFlags::COMMAND.0
                | EventFlags::DEVICE_LEFT_COMMAND.0
                | EventFlags::DEVICE_RIGHT_COMMAND.0,
        );
        assert_eq!(
            resolve_click_mode(true, device_bits),
            ClickMode::DoubleClickCopy
        );
    }

    /// Q10-c — 미정의 조합(정의된 7조합 밖)은 기본 모드로 안전하게 떨어진다.
    #[test]
    fn undefined_combinations_fall_back_to_default_mode() {
        let undefined = [
            EventFlags::COMMAND | EventFlags::SHIFT,
            EventFlags::CONTROL | EventFlags::SHIFT,
            EventFlags::COMMAND | EventFlags::CONTROL,
            EventFlags::SHIFT | EventFlags::ALTERNATE,
            EventFlags::COMMAND | EventFlags::CONTROL | EventFlags::SHIFT,
            EventFlags::COMMAND
                | EventFlags::CONTROL
                | EventFlags::ALTERNATE
                | EventFlags::SHIFT,
        ];
        for mods in undefined {
            assert_eq!(
                resolve_click_mode(true, mods),
                ClickMode::ClickStartMatch,
                "미정의 조합 {:?} 는 기본 모드여야 한다",
                mods
            );
        }
    }

    /// 설정 OFF(사용자가 명시적으로 끔) — 모드 해석은 무조건 기본이고
    /// modifier 는 아래 `plan::passthrough_flags` 쪽으로 넘어간다(§3.2 행).
    /// §8 수용 기준 5: "클릭 모드는 항상 기본 동작".
    #[test]
    fn disabled_always_returns_default_mode_regardless_of_modifiers() {
        for mods in [
            EventFlags::NONE,
            EventFlags::COMMAND,
            EventFlags::CONTROL | EventFlags::ALTERNATE,
            EventFlags::COMMAND | EventFlags::SHIFT,
        ] {
            assert_eq!(
                resolve_click_mode(false, mods),
                ClickMode::ClickStartMatch,
                "OFF 상태의 {:?} 는 기본 모드여야 한다",
                mods
            );
        }
    }
}