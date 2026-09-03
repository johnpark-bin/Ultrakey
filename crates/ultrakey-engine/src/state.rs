//! 스레드 간 공유 상태 — `docs/dev/architecture.md` §2.2 "락을 쓰지 않는 세 가지 장치".
//!
//! ⛔ **이 구조체는 원자값과 `ArcSwap` 만 담는다. `Mutex`/`RwLock` 을 넣지 마라** —
//! [`SharedState`] 는 탭 스레드의 `CGEventTap` 콜백이 매 이벤트마다 읽는다. 콜백 안에서
//! 락 획득 가능성이 있는 자료구조에 접근하는 것은 `docs/dev/architecture.md` §2.2 의
//! "콜백 안에서 절대 하지 않는 것"을 정면으로 어기는 것이다.

use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU64};
use std::sync::Arc;

use arc_swap::ArcSwap;

use ultrakey_core::gate::AtomicAppGate;
use ultrakey_core::jis::AtomicJisGate;
use ultrakey_core::korean::AtomicKoreanImeGate;
use ultrakey_core::settings::EngineConfig;
use ultrakey_core::trackpad::AtomicTrackpadPhase;
use ultrakey_layout::LayoutResolver;

/// 탭 스레드와 메인(호출) 스레드가 공유하는 상태.
///
/// - `config` — 설정·규칙 테이블. 메인이 드물게 쓰고(`Engine::reconfigure`) 콜백이 매
///   이벤트 읽는다 → `ArcSwap`(대기 없는 읽기, 원자적 포인터 교체 쓰기).
/// - `gate` — F-10 앱별 비활성화 게이트. 판정은 메인이 미리 계산해 `AtomicBool` 로
///   게시한다(§3-f 계층 0). 콜백은 이 부울만 읽는다.
/// - `seek_session_active` — F-01 Seek 세션 활성 여부(§3-b 계층 1). ⭐ 이 크레이트는
///   이 값을 켜지 않는다 — F-01 세션 상태 머신을 소유한 앱(`apps/ultrakey-app`,
///   `ultrakey-seek-session`)이 세션을 열고 닫을 때마다 `Engine::shared().
///   seek_session_active` 를 직접 `store` 한다. 이 엔진은 콜백 임계 경로에서 이
///   원자값을 O(1) 로드만 한다(§3-b 계층 1 "전역 플래그 1개로 확인").
/// - `seek_input_box` / `seek_semicolon_cycles` — ⭐(이슈 #93) 인풋 박스 모드
///   게이트. `seek_session_active` 와 같은 주인(Seek 워커)이 **세션 열림/닫힘
///   시점에 래칭**해 게시한다. 콜백은 계층 1 통과 판정에 이 원자값들을 O(1)
///   로드만 한다(세션이 비활성이면 통과 분기가 아예 평가되지 않아 기존
///   동작과 완전히 같다).
/// - `seek_shortcut_keycode` / `seek_shortcut_mods` — ⭐(이슈 #93) `Toggle Seek
///   with shortcut:` 조합. 인풋 박스 모드에서 문자 키가 통과되는 동안 이 조합만
///   **통과시키지 않아** 세션을 단축키로 다시 토글 닫을 수 있게 한다(계층 1 이
///   Consume 한 뒤 machine 의 `matches_global_shortcut` 재입력 판정을 탄다).
///   keycode `0` = 미설정(통과 가드 무효).
/// - `layout` — F-14(B) 레이아웃 테이블. 콜백이 임계 경로에서 읽어야 할 때를 대비해
///   `LayoutResolver` 자체가 이미 무잠금 `ArcSwap` 을 내부에 두고 있다(`ultrakey-layout`).
/// - `korean_ime` — F-16 한국어 입력기 활성 판정(`docs/spec/korean-input.md` §3.3).
///   메인 스레드가 입력 소스 변경 알림(및 기동 시 1회 초기화, `system_hooks.rs::
///   refresh_input_source`)을 받을 때마다 `classify_input_source_languages` 로 판정한
///   값을 게시하고, 콜백은 이 원자값을 O(1) 로드만 한다. 기본값 `Unknown` 은 fail-closed
///   다 — 게시가 아직 한 번도 일어나지 않았어도 F-16 규칙이 오발화하지 않는다.
/// - `trackpad` — F-06 트랙패드 제스처 게이트(`trackpad-hyper-gesture.md` §3.4). ⭐ 이
///   엔진은 이 값을 켜지 않는다 — 트랙패드 리스너 스레드(`apps/ultrakey-app` 소유,
///   `ultrakey-platform::multitouch` 비공개 FFI)가 제스처 상태 머신의 전이마다
///   `store` 한다. 이 엔진은 콜백 임계 경로에서 이 원자값을 O(1) 로드만 한다 —
///   리스너가 존재하지 않는 구성(비공개 API 격하)에서도 값은 항상 `Off` 이고
///   중재 판정은 기존과 완전히 동일하다(§8 격하 수용 기준).
pub struct SharedState {
    pub config: ArcSwap<EngineConfig>,
    pub gate: Arc<AtomicAppGate>,
    pub seek_session_active: AtomicBool,
    /// ⭐(이슈 #93) 인풋 박스 모드(다국어 검색 언어) — 세션 열림 시점에 Seek
    /// 워커가 래칭해 게시한다.
    pub seek_input_box: AtomicBool,
    /// ⭐(이슈 #93) `seek.semicolonCycle` 값 — 인풋 박스 모드에서 `;` 의
    /// 통과/소비 판정에 쓰인다(설정 변경 시 워커가 함께 게시).
    pub seek_semicolon_cycles: AtomicBool,
    /// ⭐(이슈 #93) `Toggle Seek with shortcut:` 의 물리 keycode(`0` = 미설정).
    pub seek_shortcut_keycode: AtomicU16,
    /// ⭐(이슈 #93) 같은 단축키의 modifier 비트마스크(`EventFlags` 비트 관례).
    pub seek_shortcut_mods: AtomicU64,
    pub layout: Arc<LayoutResolver>,
    pub korean_ime: AtomicKoreanImeGate,
    /// ⭐ F-19(D-7) — 일본어 입력기 활성 판정. `korean_ime` 과 같은 패턴: 메인 스레드가
    /// `kTISPropertyInputSourceLanguages` 를 판정해 게시하고, 콜백은 O(1) 로드만 한다.
    /// `KoreanImeState` 3상태를 재사용한다(현재 소스의 언어가 무엇이냐만 다르다).
    pub japanese_ime: AtomicKoreanImeGate,
    /// ⭐ F-19(D-4) — 키보드 타입 게이트(JIS/NotJis/Unknown). 메인 스레드가
    /// `ultrakey_platform::keyboard_type::current_keyboard_is_jis()` 로 판정해 게시하고,
    /// 콜백은 O(1) 로드만 한다. `Unknown`(기본)은 fail-closed — JIS 행·US 행 전부
    /// 미발화(둘 다 파괴적인 실패 모드라 안전 방향이 유일하다, `jis.rs` 모듈 문서).
    pub is_jis: AtomicJisGate,
    /// ⭐ F-06 — 트랙패드 제스처 게이트. 리스너 스레드가 게시하고 콜백은 읽기만 한다.
    /// ⛔ 리스너가 `store` 하는 값은 `ultrakey-core::trackpad::TrackpadPhase` 이다 —
    /// 이 엔진은 이 값을 쓰지 않고, `GateSnapshot` 으로 스냅샷을 찍어 넘길 뿐이다.
    pub trackpad: Arc<AtomicTrackpadPhase>,
    /// ⭐ 이슈 #108 자동 복구 안전망 — 직전 `Effect::ToggleCapsLock` 이 실제로 캡스락을
    /// **켰다**(`toggle_caps_lock_via_path_c` 의 `after==on`)면 `true`. 탭 스레드가
    /// `apply_effects_in_tap`/`apply_effects_outside_tap` 직후 게시하고, `Watchdog` 이
    /// `lifecycle::caps_lock_recovery` 판정에 읽기만 한다 — 그 잠금이 `Double tap
    /// shift`·`Left/right shift`·`Shift + caps lock = caps lock` 류 경로 C 규칙의
    /// **의도된** 결과이므로 안전망이 되돌리지 않아야 함을 뜻한다.
    pub caps_lock_owned_lock: AtomicBool,
    /// ⭐ 이슈 #110 — D-1 커널 매핑이 되읽기로 **확인된** 상태인가
    /// (`PathBManager::d1_confirmed` 의 사본). 엔진이 경로 B 재조정 직후마다
    /// 게시하고, 앱이 트레이·환경설정·Event Viewer 의 "caps lock 커널 매핑 미적용"
    /// 표시에 읽는다(표시 조건 `config.caps_lock_alias.is_some() && !d1_confirmed`).
    /// ⛔ 탭 콜백의 중재 판정은 이 값을 **읽지 않는다** — 방어선은 이벤트 모양으로
    /// 판정한다(`Arbiter::arbitrate` 의 D-1 우회 탭 환원).
    pub d1_confirmed: AtomicBool,
}

impl SharedState {
    pub fn new(config: EngineConfig, gate: Arc<AtomicAppGate>) -> Arc<Self> {
        Arc::new(SharedState {
            config: ArcSwap::from_pointee(config),
            gate,
            seek_session_active: AtomicBool::new(false),
            seek_input_box: AtomicBool::new(false),
            seek_semicolon_cycles: AtomicBool::new(false),
            seek_shortcut_keycode: AtomicU16::new(0),
            seek_shortcut_mods: AtomicU64::new(0),
            layout: Arc::new(LayoutResolver::new()),
            korean_ime: AtomicKoreanImeGate::new(),
            japanese_ime: AtomicKoreanImeGate::new(),
            is_jis: AtomicJisGate::new(),
            trackpad: Arc::new(AtomicTrackpadPhase::new()),
            caps_lock_owned_lock: AtomicBool::new(false),
            d1_confirmed: AtomicBool::new(false),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_with_seek_inactive_and_given_config() {
        let gate = Arc::new(AtomicAppGate::new());
        let cfg = EngineConfig::default();
        let shared = SharedState::new(cfg.clone(), gate);

        assert!(!shared
            .seek_session_active
            .load(std::sync::atomic::Ordering::Acquire));
        assert_eq!(
            shared.config.load().timings.quick_press_duration_ms,
            cfg.timings.quick_press_duration_ms
        );
        assert!(shared.layout.current().is_empty());
    }
}
