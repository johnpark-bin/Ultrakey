//! 저장 키 상수(F-15 `settings-store-and-integrity.md`).
//!
//! 문자열 리터럴을 호출부마다 흩어 쓰면 오타가 컴파일을 통과해 버린다 — 그래서 전부
//! 여기 상수로 모아 둔다. 값 자체(어떤 설정이 존재하는가)는 이 문서(F-15)가 정한 것이
//! 아니다 — F-15 §4 는 "이 문서가 소유하는 사용자 노출 설정은 없다"고 명시한다. 이
//! 파일이 나열하는 항목은 F-05(`hyperkey.md`)·F-09(`preferences-ui.md`)가 이미 정의한
//! 설정에, F-15 §3.1 이 정한 "부재 = 기본값" 저장 규약을 적용하기 위한 평평한 키
//! 이름일 뿐이다.
//!
//! 표기: 점(`.`) 구분 평평한 키, 각 세그먼트는 camelCase. 최상위 접두사는 소유 기능을
//! 가리킨다 — `hyperkey.*`(F-05), `ui.*`(F-09 환경설정 창 자체의 UI 상태).

/// hyper 슬롯 — `Remap key to hyper key:` 체크박스.
pub const HYPERKEY_HYPER_ENABLED: &str = "hyperkey.hyper.enabled";
/// hyper 슬롯 — 소스 키 팝업(`hyperkey.md` §4 항목 1).
pub const HYPERKEY_HYPER_SOURCE: &str = "hyperkey.hyper.source";
/// `Include shift in hyper key` — hyper 에만 적용된다(`hyperkey.md` §3.1: meh·bleh 는
/// shift 를 끌 수 있는 옵션이 없다).
pub const HYPERKEY_INCLUDE_SHIFT_IN_HYPER: &str = "hyperkey.includeShiftInHyper";
/// meh 슬롯 — 체크박스.
pub const HYPERKEY_MEH_ENABLED: &str = "hyperkey.meh.enabled";
/// meh 슬롯 — 소스 키 팝업.
pub const HYPERKEY_MEH_SOURCE: &str = "hyperkey.meh.source";
/// bleh 슬롯 — 체크박스.
pub const HYPERKEY_BLEH_ENABLED: &str = "hyperkey.bleh.enabled";
/// bleh 슬롯 — 소스 키 팝업.
pub const HYPERKEY_BLEH_SOURCE: &str = "hyperkey.bleh.source";
/// `Apply modifiers to keypress events and:` 4개 체크박스 — Click.
pub const HYPERKEY_MOUSE_APPLY_CLICK: &str = "hyperkey.mouseApply.click";
/// 상동 — Drag.
pub const HYPERKEY_MOUSE_APPLY_DRAG: &str = "hyperkey.mouseApply.drag";
/// 상동 — Move.
pub const HYPERKEY_MOUSE_APPLY_MOVE: &str = "hyperkey.mouseApply.move";
/// 상동 — Scroll.
pub const HYPERKEY_MOUSE_APPLY_SCROLL: &str = "hyperkey.mouseApply.scroll";
/// `Engage hyper key using trackpad:` — 체크박스. ⚠️ 값을 저장할 뿐, 제스처 인식
/// 규칙은 아직 아무것도 만들지 않는다(F-06/M5, `HyperkeySettings::to_modifier_rules()`
/// 문서 주석 참고).
pub const HYPERKEY_TRACKPAD_ENABLED: &str = "hyperkey.trackpad.enabled";
/// 상동 — 5분할 트랙패드 영역 선택.
pub const HYPERKEY_TRACKPAD_AREA: &str = "hyperkey.trackpad.area";
/// 상동 — 메뉴바 아이콘 변경 옵션.
pub const HYPERKEY_TRACKPAD_CHANGE_MENU_BAR_ICON: &str = "hyperkey.trackpad.changeMenuBarIcon";
/// 상동 — 햅틱 피드백 옵션.
pub const HYPERKEY_TRACKPAD_HAPTIC: &str = "hyperkey.trackpad.haptic";
/// 환경설정 창이 마지막으로 열려 있던 탭(F-09).
pub const UI_LAST_TAB: &str = "ui.lastTab";

/// 전량 나열 — 테스트가 오타·중복·접두사 규칙을 검증하는 데 쓴다.
pub fn all() -> &'static [&'static str] {
    &[
        HYPERKEY_HYPER_ENABLED,
        HYPERKEY_HYPER_SOURCE,
        HYPERKEY_INCLUDE_SHIFT_IN_HYPER,
        HYPERKEY_MEH_ENABLED,
        HYPERKEY_MEH_SOURCE,
        HYPERKEY_BLEH_ENABLED,
        HYPERKEY_BLEH_SOURCE,
        HYPERKEY_MOUSE_APPLY_CLICK,
        HYPERKEY_MOUSE_APPLY_DRAG,
        HYPERKEY_MOUSE_APPLY_MOVE,
        HYPERKEY_MOUSE_APPLY_SCROLL,
        HYPERKEY_TRACKPAD_ENABLED,
        HYPERKEY_TRACKPAD_AREA,
        HYPERKEY_TRACKPAD_CHANGE_MENU_BAR_ICON,
        HYPERKEY_TRACKPAD_HAPTIC,
        UI_LAST_TAB,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    // keys::all() 의 모든 키가 서로 다르다.
    #[test]
    fn all_keys_are_unique() {
        let keys = all();
        let mut sorted = keys.to_vec();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), keys.len(), "중복된 키가 있다: {keys:?}");
    }

    // keys::all() 의 모든 키가 접두사 규칙(hyperkey.* / ui.*)을 지킨다.
    #[test]
    fn all_keys_follow_prefix_convention() {
        for key in all() {
            assert!(
                key.starts_with("hyperkey.") || key.starts_with("ui."),
                "접두사 규칙을 벗어난 키: {key}"
            );
        }
    }
}
