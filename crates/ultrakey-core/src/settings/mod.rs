//! 엔진 설정 타입. 값의 근거(실측 대 설계 판단)를 각 필드 옆에 구분해 적는다
//! (`docs/dev/architecture.md` §4 와 일치).
//!
//! `store`(F-15 `settings-store-and-integrity.md` §3.1)는 이 모듈이 정의하는 타입들이
//! 디스크에서 조립되는 방식을 담당한다 — "부재 = 기본값" 규약의 실제 구현이다.
//! `keys`는 그 저장소가 쓰는 문자열 키 상수를 모아 둔다.

use crate::keycode::KeyCode;
use crate::rules::RuleTable;

pub mod keys;
pub mod store;
pub use store::*;

/// `Apply modifiers to keypress events and:` 4개 체크박스(`hyperkey.md` §3.3).
///
/// ⭐ 실측 기본값: `Click` 만 `true`, 나머지는 전부 `false`(app-bundle-analysis.md §6.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct MouseApply {
    pub click: bool,
    pub drag: bool,
    pub r#move: bool,
    pub scroll: bool,
}

impl Default for MouseApply {
    fn default() -> Self {
        MouseApply {
            click: true,
            drag: false,
            r#move: false,
            scroll: false,
        }
    }
}

/// 이 엔진이 쓰는 모든 타이밍 값. `docs/dev/architecture.md` §4 가 "실측이 아니라 이 구현의
/// 설계 판단"이라 명시한 값들은 여기서도 동일하게 구분해 둔다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timings {
    /// ⭐ 실측 확정(`hyperkey.md` §4, AX 트리): 최소 250ms · 최대 2000ms · 현재값(기본) 1000ms.
    pub quick_press_duration_ms: u64,
    /// (추정) — `key-remapping-engine.md` §4·§9 가 이미 근거 없는 추정치로 표기한 값을 그대로 쓴다.
    pub double_tap_interval_ms: u64,
    /// (설계 판단) — architecture.md §4: 탭이 죽은 뒤 첫 키 입력 전에 복구되길 기대하는 주기.
    pub watchdog_poll_ms: u64,
    /// (설계 판단) — architecture.md §4: 절전 복귀 직후 커널 HID/WindowServer 안정화 대기.
    pub wake_delay_ms: u64,
    /// (설계 판단) — architecture.md §4: 세션 활성화(로그인/화면 잠금 해제) 후 재확인 지연.
    pub session_delay_ms: u64,
    /// (설계 판단) — architecture.md §4: 외장 키보드 연결 후 장치 열거가 끝나길 기다리는 지연.
    pub keyboard_connect_delay_ms: u64,
    /// (설계 판단) — architecture.md §4: 절전→잠금해제→세션전환 연쇄를 한 번으로 합치는 디바운스.
    pub restart_debounce_ms: u64,
    /// (설계 판단) — architecture.md §4: 이 횟수를 넘으면 탭 재생성으로 에스컬레이션.
    pub tap_reenable_max_attempts: u32,
    /// (설계 판단) — architecture.md §4: 이 횟수를 넘으면 프로세스 재실행 신호로 에스컬레이션.
    pub tap_recreate_max_attempts: u32,
    /// (설계 판단) — architecture.md §4: 온보딩 중 권한 폴링 주기(짧게 — 사용자가 시스템
    /// 설정에서 막 돌아온 직후를 기다림).
    pub permission_poll_onboarding_ms: u64,
    /// (설계 판단) — architecture.md §4: 배경 권한 폴링 주기(권한 회수 감지용, 길어도 무방).
    pub permission_poll_background_ms: u64,
}

impl Default for Timings {
    fn default() -> Self {
        Timings {
            quick_press_duration_ms: 1000,
            double_tap_interval_ms: 300,
            watchdog_poll_ms: 1000,
            wake_delay_ms: 2000,
            session_delay_ms: 1000,
            keyboard_connect_delay_ms: 1500,
            restart_debounce_ms: 5000,
            tap_reenable_max_attempts: 5,
            tap_recreate_max_attempts: 3,
            permission_poll_onboarding_ms: 500,
            permission_poll_background_ms: 5000,
        }
    }
}

/// 엔진 하나가 쓰는 설정 전체 — 규칙 테이블 + 마우스 적용 범위 + 타이밍.
#[derive(Debug, Clone, Default)]
pub struct EngineConfig {
    pub rules: RuleTable,
    pub mouse_apply: MouseApply,
    pub timings: Timings,
    /// D-1 caps lock 모멘터리 정규화(`docs/dev/architecture.md` §6.1). `Some(kc)` 면
    /// 중재기가 진입 즉시 이 keycode(경로 B 로 설치된 `hidutil` 대체 키, 보통 F18)를
    /// `KeyCode::CAPS_LOCK` 으로 되돌려 판정한다. caps lock 에 의존하는 규칙이 하나도
    /// 없으면 `None` — 물리 caps lock 이 그대로(`FlagsChanged` 로만) 도착한다.
    pub caps_lock_alias: Option<KeyCode>,
    /// F-17(`docs/spec/per-device-settings.md`) — `perDevice.` 로 시작하는 설정 키의
    /// **원본 스냅샷**. 엔진은 이 값을 직접 해석하지 않고, 필요할 때
    /// `ultrakey_core::perdevice::PerDeviceSettings::new(&cfg.per_device_values)` 로
    /// 감싸 읽는다(§3.3 2계층 폴백 해석은 그 타입이 담당한다).
    ///
    /// ⚠️ 이 맵은 `ArcSwap<EngineConfig>`(`SharedState::config`)에 담기므로 **소유
    /// 값**이어야 한다(빌림 불가) — 그래서 `&BTreeMap` 이 아니라 `BTreeMap` 그 자체다.
    /// 앱이 설정 저장소에서 `perDevice.` 접두사 키를 추려 채운다(`perDevice._managed`
    /// 원장 키는 **제외** — 그건 설정이 아니라 `ultrakey-engine::path_b` 의 소유권
    /// 원장이다). `Default` 는 빈 맵이다.
    pub per_device_values: std::collections::BTreeMap<String, serde_json::Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mouse_apply_default_is_click_only() {
        let m = MouseApply::default();
        assert!(m.click);
        assert!(!m.drag);
        assert!(!m.r#move);
        assert!(!m.scroll);
    }

    #[test]
    fn timings_default_matches_documented_values() {
        let t = Timings::default();
        assert_eq!(t.quick_press_duration_ms, 1000);
        assert_eq!(t.double_tap_interval_ms, 300);
        assert_eq!(t.watchdog_poll_ms, 1000);
        assert_eq!(t.wake_delay_ms, 2000);
        assert_eq!(t.session_delay_ms, 1000);
        assert_eq!(t.keyboard_connect_delay_ms, 1500);
        assert_eq!(t.restart_debounce_ms, 5000);
        assert_eq!(t.tap_reenable_max_attempts, 5);
        assert_eq!(t.tap_recreate_max_attempts, 3);
        assert_eq!(t.permission_poll_onboarding_ms, 500);
        assert_eq!(t.permission_poll_background_ms, 5000);
    }
}
