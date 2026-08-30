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
/// 사용자가 드래그로 조절한 설정 창 너비(논리 좌표, pt) — 이슈 #32 Phase 1.
/// `ui.windowHeight` 와 항상 짝으로 존재해야 복원한다(둘 중 하나만 있으면 무시).
pub const UI_WINDOW_WIDTH: &str = "ui.windowWidth";
/// 상동 — 높이.
pub const UI_WINDOW_HEIGHT: &str = "ui.windowHeight";

// ── F-08 Power User Presets(`power-user-presets.md`) — `ultrakey-presets` 가 소비한다 ──

/// F-08.1 `Remap caps lock to:` — 체크박스.
pub const PRESETS_CAPS_LOCK_REMAP_ENABLED: &str = "presets.capsLockRemap.enabled";
/// F-08.1 — 팝업(50종).
pub const PRESETS_CAPS_LOCK_REMAP_TARGET: &str = "presets.capsLockRemap.target";
/// F-08.2 `Quick press caps lock to execute:` — 체크박스.
pub const PRESETS_CAPS_QUICK_PRESS_ENABLED: &str = "presets.capsQuickPress.enabled";
/// F-08.2 — 팝업(48종 + 구분선 1).
pub const PRESETS_CAPS_QUICK_PRESS_ACTION: &str = "presets.capsQuickPress.action";
/// F-08.3 `Quick press duration` — 슬라이더(250~2000ms, 기본 1000ms).
pub const PRESETS_QUICK_PRESS_DURATION_MS: &str = "presets.quickPressDurationMs";
/// F-08.4 `Caps lock + space = enter` — 체크박스.
pub const PRESETS_CAPS_SPACE_ENTER: &str = "presets.capsSpaceEnter";
/// F-08.5 `Caps lock + W A S D = ▲◀▼▶` — 체크박스.
pub const PRESETS_CAPS_WASD_ARROWS: &str = "presets.capsWasdArrows";
/// F-08.6 `Caps lock +` [팝업] ` = ◀▼▲▶` — 체크박스.
pub const PRESETS_CAPS_HJKL_ARROWS_ENABLED: &str = "presets.capsHjklArrows.enabled";
/// F-08.6 — 인라인 팝업(`H J K L` · `I J K L`).
pub const PRESETS_CAPS_HJKL_ARROWS_KEY_SET: &str = "presets.capsHjklArrows.keySet";
/// F-08.7 `Caps lock + home row = ` [팝업] — 체크박스.
pub const PRESETS_CAPS_HOME_ROW_ENABLED: &str = "presets.capsHomeRow.enabled";
/// F-08.7 — 인라인 팝업(symbol row · function row).
pub const PRESETS_CAPS_HOME_ROW_SCHEME: &str = "presets.capsHomeRow.scheme";
/// F-08.8 `Double tap shift = caps lock` — 체크박스.
pub const PRESETS_DOUBLE_TAP_SHIFT_TO_CAPS: &str = "presets.doubleTapShiftToCaps";
/// F-08.9 `Left shift + right shift = caps lock` — 체크박스.
pub const PRESETS_LEFT_RIGHT_SHIFT_TO_CAPS: &str = "presets.leftRightShiftToCaps";
/// F-08.10 `Shift + caps lock = caps lock` — 체크박스.
pub const PRESETS_SHIFT_CAPS_TO_CAPS: &str = "presets.shiftCapsToCaps";
/// F-08.11 `Quick press left or right shift to input corresponding:` — 체크박스.
pub const PRESETS_SHIFT_QUICK_PRESS_BRACKETS_ENABLED: &str =
    "presets.shiftQuickPressBrackets.enabled";
/// F-08.11 — 팝업(4종 문자 쌍).
pub const PRESETS_SHIFT_QUICK_PRESS_BRACKETS_PAIR: &str = "presets.shiftQuickPressBrackets.pair";
/// F-08.12 `Hyper + delete = forward delete` — 체크박스.
pub const PRESETS_HYPER_DELETE_TO_FORWARD: &str = "presets.hyperDeleteToForward";
/// F-08.13 `Remap delete to forward delete` — 체크박스.
pub const PRESETS_DELETE_TO_FORWARD: &str = "presets.deleteToForward";
/// F-08.14 `Shift + delete = forward delete` — 체크박스.
pub const PRESETS_SHIFT_DELETE_TO_FORWARD: &str = "presets.shiftDeleteToForward";
/// F-08.15 `Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):` — 체크박스.
pub const PRESETS_PASTE_WITHOUT_FORMATTING_ENABLED: &str = "presets.pasteWithoutFormatting.enabled";
/// F-08.15 — 팝업(4종 트리거).
pub const PRESETS_PASTE_WITHOUT_FORMATTING_TRIGGER: &str = "presets.pasteWithoutFormatting.trigger";
/// F-08.16 `Home & end operate on lines` — 체크박스.
pub const PRESETS_HOME_END_ON_LINES: &str = "presets.homeEndOnLines";
/// Advanced 토글 — 켜져 있으면(true) 경로 B(`hidutil` 커널 매핑)를 쓰지 않고 경로 A
/// (이벤트 합성)만 쓴다(D-1, `docs/dev/architecture.md` §6.1 되돌릴 수단).
pub const PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP: &str = "presets.synthesizeCapsLockRemap";

// ── F-16 한국어 입력 지원(`korean-input.md` §4.2, D-K10) — `ultrakey-korean` 이 소비한다 ──

/// F-16.1 `Shift + Space 로 입력 소스 변경` — 체크박스. 기본값 ☐(부재 = `false`).
pub const KOREAN_SHIFT_SPACE_SWITCHES_INPUT_SOURCE: &str = "korean.shiftSpaceSwitchesInputSource";
/// F-16.2 `한/영 키로 입력 소스 변경` — 체크박스. 기본값 ☐(부재 = `false`). §3.2 키코드가
/// 근거 3중으로 해소되어(`KeyCode::JIS_KANA`) `KoreanSettings::to_rules()` 가 규칙을 낸다(D-K14).
pub const KOREAN_HAN_ENG_SWITCHES_INPUT_SOURCE: &str = "korean.hanEngSwitchesInputSource";
/// F-16.3 `한자 키로 한자 변환` — 체크박스. 상동(D-K14) — `KeyCode::JIS_EISU`, 규칙을 낸다.
pub const KOREAN_HANJA_KEY_CONVERTS_HANJA: &str = "korean.hanjaKeyConvertsHanja";
/// F-16.4 `₩ 키로 백틱(\`) 입력` — 체크박스. 기본값 ☐(부재 = `false`).
pub const KOREAN_WON_KEY_TYPES_BACKTICK: &str = "korean.wonKeyTypesBacktick";
/// F-16 항목 5 `원격 데스크톱 클라이언트에서 한국어 키 처리 끄기` — 체크박스.
/// ⚠️ **기본값 ☑(부재 = `true`)** — 다른 F-16 항목과 반대 방향이다(명세 §4.2 각주,
/// D-K9). 구현·리뷰 양쪽에서 놓치기 쉬운 지점이라 여기서도 명시해 둔다.
pub const KOREAN_DISABLE_IN_REMOTE_DESKTOP: &str = "korean.disableInRemoteDesktop";

// ── F-03 Seek 오버레이(`seek-overlay-ui.md` §3.4, 이슈 #34) ──
//
// ⭐ **부재 = 기본값** (F-15 §3.6). 검색 바를 한 번도 옮기지 않았으면 이 두 키가
// 아예 없고, 그때의 기본 위치는 "호출 시점에 커서가 있는 디스플레이의 상단부
// 중앙" 이다(§3.4 `(추정)`). 사용자가 드래그해 옮기는 **즉시** 기록한다 —
// 원본의 `persistPosition` 과 같은 규약(§1.1, defaults 실측).

/// 검색 바 창의 좌상단 x — **전역 화면 좌표(pt, 좌상단 원점)**.
pub const SEEK_SEARCH_BAR_X: &str = "seek.searchBar.x";
/// 검색 바 창의 좌상단 y — 상동.
pub const SEEK_SEARCH_BAR_Y: &str = "seek.searchBar.y";

// ── F-01 Seek 활성화·세션(`seek-activation-and-session.md` §4) ──
//
// ⭐ **부재 = 기본값**(F-15 §3.6, 이 절 전체가 명세 §4 표의 "오기 정정"이 확정한 실측
// 기본값을 그대로 따른다) — 세 활성화 경로가 전부 미설정이면 Seek 를 발동할 방법이
// 없다는 것이 §1 온보딩 함의다.

/// `Toggle Seek with shortcut:` — 웹 `KeyboardEvent.code`(예: `"Space"`).
/// 부재 = **미설정**(버튼 라벨 `Record Shortcut`, 실측 출고 기본값).
pub const SEEK_TOGGLE_SHORTCUT_CODE: &str = "seek.toggleShortcut.code";
/// 같은 단축키의 modifier 비트마스크(`EventFlags` 관례). 부재 = 0.
pub const SEEK_TOGGLE_SHORTCUT_MODIFIERS: &str = "seek.toggleShortcut.modifiers";
/// `Remap key to Seek:` — `SourceKey` variant 이름. 부재 = `-`(미설정, 실측 기본값).
pub const SEEK_REMAP_KEY: &str = "seek.remapKey";
/// `Only show while the remapped key is held`(명세 §4 저장 키 `seekExecuteOnClose`).
/// 부재 = ☐.
pub const SEEK_EXECUTE_ON_CLOSE: &str = "seek.executeOnClose";
/// `Semicolon highlights next match`(명세 §4 저장 키 `semicolonCycleSeek`). 부재 = ☐.
pub const SEEK_SEMICOLON_CYCLE: &str = "seek.semicolonCycle";
/// `Focus window before clicking`(F-04 §4). 기본 ☐(부재 = `false`).
pub const SEEK_FOCUS_WINDOW_BEFORE_CLICKING: &str = "seek.focusWindowBeforeClicking";
/// `Change click modes with modifier keys`(F-04 §4).
/// ⚠️ **기본값 ☑(부재 = `true`)** — Seek 탭에서 유일하게 출고 기본값이 켜진
/// 항목이다(명세 §4). F-16 의 `KOREAN_DISABLE_IN_REMOTE_DESKTOP` 과 같은
/// "부재 = true" 주석 관례를 그대로 따른다 — `unwrap_or_default()` 를 쓰면
/// 부재가 `false` 로 읽혀 실측 기본값을 조용히 어기게 된다.
pub const SEEK_CHANGE_CLICK_MODES_WITH_MODIFIERS: &str = "seek.changeClickModesWithModifiers";

// ── F-17 키보드별 설정(`per-device-settings.md` §3.3~3.4, D-17-2) —
// `ultrakey_core::perdevice` 가 소비한다 ──
//
// ⚠️ 이 기능의 저장 키는 다른 절과 달리 **디바이스마다 동적으로 갈라지는
// 세그먼트**(`perDevice.<vid>:<pid>.*`)를 갖는다 — 그래서 고정 상수 하나로 못 두고
// 조립 함수를 둔다(`all()`/중복·접두사 테스트가 다루는 "고정 키 목록"에는 포함하지
// 않는다 — 무한히 많은 키가 나올 수 있기 때문이다). `perDevice._managed` 원장 키만
// 디바이스에 무관하게 고정이라 상수로 둔다.

/// 공통(`For all devices`) 계층의 디바이스 세그먼트 이름 — `perDevice.all.*`(§3.3).
pub const PER_DEVICE_COMMON_SCOPE: &str = "all";

/// D-17-2 원장 — `{ "<vid>:<pid>": [{src,dst}, …] }`. "직전에 우리가 실제로 쓴
/// 배열"을 디바이스별로 추적해, 정리(cleanup) 시점에 남의 매핑을 지우지 않는다
/// (`per-device-settings.md` §3.6 규칙 6, CONTRACT.md D-17-2).
pub const PER_DEVICE_MANAGED: &str = "perDevice._managed";

/// `perDevice.<scope>.keyRemap.rows` 조립(§3.3·§3.4). `scope` 는
/// [`PER_DEVICE_COMMON_SCOPE`] 또는 `DeviceId::as_str()`(`ultrakey_core::perdevice`).
pub fn per_device_key_remap_rows(scope: &str) -> String {
    format!("perDevice.{scope}.keyRemap.rows")
}

/// `perDevice.<scope>.functionKeys.f1`~`f12` 조립(§3.3·§3.5).
pub fn per_device_function_key(scope: &str, f: crate::perdevice::FKey) -> String {
    format!("perDevice.{scope}.functionKeys.{}", f.key_segment())
}

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
        UI_WINDOW_WIDTH,
        UI_WINDOW_HEIGHT,
        PRESETS_CAPS_LOCK_REMAP_ENABLED,
        PRESETS_CAPS_LOCK_REMAP_TARGET,
        PRESETS_CAPS_QUICK_PRESS_ENABLED,
        PRESETS_CAPS_QUICK_PRESS_ACTION,
        PRESETS_QUICK_PRESS_DURATION_MS,
        PRESETS_CAPS_SPACE_ENTER,
        PRESETS_CAPS_WASD_ARROWS,
        PRESETS_CAPS_HJKL_ARROWS_ENABLED,
        PRESETS_CAPS_HJKL_ARROWS_KEY_SET,
        PRESETS_CAPS_HOME_ROW_ENABLED,
        PRESETS_CAPS_HOME_ROW_SCHEME,
        PRESETS_DOUBLE_TAP_SHIFT_TO_CAPS,
        PRESETS_LEFT_RIGHT_SHIFT_TO_CAPS,
        PRESETS_SHIFT_CAPS_TO_CAPS,
        PRESETS_SHIFT_QUICK_PRESS_BRACKETS_ENABLED,
        PRESETS_SHIFT_QUICK_PRESS_BRACKETS_PAIR,
        PRESETS_HYPER_DELETE_TO_FORWARD,
        PRESETS_DELETE_TO_FORWARD,
        PRESETS_SHIFT_DELETE_TO_FORWARD,
        PRESETS_PASTE_WITHOUT_FORMATTING_ENABLED,
        PRESETS_PASTE_WITHOUT_FORMATTING_TRIGGER,
        PRESETS_HOME_END_ON_LINES,
        PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP,
        KOREAN_SHIFT_SPACE_SWITCHES_INPUT_SOURCE,
        KOREAN_HAN_ENG_SWITCHES_INPUT_SOURCE,
        KOREAN_HANJA_KEY_CONVERTS_HANJA,
        KOREAN_WON_KEY_TYPES_BACKTICK,
        KOREAN_DISABLE_IN_REMOTE_DESKTOP,
        SEEK_SEARCH_BAR_X,
        SEEK_SEARCH_BAR_Y,
        SEEK_TOGGLE_SHORTCUT_CODE,
        SEEK_TOGGLE_SHORTCUT_MODIFIERS,
        SEEK_REMAP_KEY,
        SEEK_EXECUTE_ON_CLOSE,
        SEEK_SEMICOLON_CYCLE,
        SEEK_FOCUS_WINDOW_BEFORE_CLICKING,
        SEEK_CHANGE_CLICK_MODES_WITH_MODIFIERS,
        PER_DEVICE_MANAGED,
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

    // keys::all() 의 모든 키가 접두사 규칙(hyperkey.* / ui.* / presets.* / general.* /
    // korean.* / perDevice.*)을 지킨다. ⭐ M2 에서 `presets.*`(F-08, 이 파일)와
    // `general.*`(F-10, 다른 크레이트가 동시에 작업 중)를 추가로 허용하도록 넓혔다.
    // F-16 이 `korean.*` 를 더한다(D-K10) — 카탈로그 키는 `settings.korean.*` 이지만
    // **저장 키**는 명세 그대로 `korean.*` 다(D-K11, 이 파일이 다루는 것은 저장 키다).
    // F-17 이 `perDevice.*` 를 더한다(§3.3) — 동적 세그먼트(`perDevice.<vid>:<pid>.*`)
    // 는 `all()` 에 나열되지 않지만(위 주석 참고), 접두사 규칙 자체는 여기서도 지킨다.
    #[test]
    fn all_keys_follow_prefix_convention() {
        for key in all() {
            assert!(
                key.starts_with("hyperkey.")
                    || key.starts_with("ui.")
                    || key.starts_with("presets.")
                    || key.starts_with("general.")
                    || key.starts_with("korean.")
                    // F-03 Seek 오버레이(이슈 #34) — 검색 바 위치.
                    || key.starts_with("seek.")
                    || key.starts_with("perDevice."),
                "접두사 규칙을 벗어난 키: {key}"
            );
        }
    }

    // ── F-17 조립 함수 — `per-device-settings.md` §3.3 저장 키 모양 그대로 ──────────

    #[test]
    fn per_device_key_remap_rows_assembles_expected_key() {
        assert_eq!(
            per_device_key_remap_rows(PER_DEVICE_COMMON_SCOPE),
            "perDevice.all.keyRemap.rows"
        );
        assert_eq!(
            per_device_key_remap_rows("5ac:24f"),
            "perDevice.5ac:24f.keyRemap.rows"
        );
    }

    #[test]
    fn per_device_function_key_assembles_expected_key() {
        assert_eq!(
            per_device_function_key(PER_DEVICE_COMMON_SCOPE, crate::perdevice::FKey::F1),
            "perDevice.all.functionKeys.f1"
        );
        assert_eq!(
            per_device_function_key("5ac:24f", crate::perdevice::FKey::F12),
            "perDevice.5ac:24f.functionKeys.f12"
        );
    }
}
