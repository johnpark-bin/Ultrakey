//! `Seek` 탭(+ `Presets` 탭 한 항목)의 F-01 관련 설정 — 명세 §4 표 그대로.
//!
//! ⭐ **출고 기본값은 전부 "미설정/꺼짐"** 이다(실측: AX 트리 + `defaults`
//! 부재, 명세 §4 "오기 정정" 절). 즉 [`SeekConfig::default()`] 상태에서는
//! Seek 을 발동할 방법이 **하나도 없다** — 명세 §1 이 그리는 온보딩 함의
//! ("아무것도 설정하지 않은 사용자는 Seek 를 단 한 번도 발동할 수 없다")를
//! 이 크레이트의 타입으로 코드에 못박은 것이다.

use ultrakey_core::event::InputEvent;
use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::KeyCode;

/// `Seek` 탭(+ `Presets` 탭 한 항목)의 F-01 관련 설정.
///
/// F-02·F-03·F-04 범위인 `Seek using macOS accessibility` · `Match on more
/// than one character` · `Only Seek in the frontmost window` · `Focus window
/// before clicking` · `Change click modes with modifier keys` 는 이 구조체가
/// 다루지 않는다(명세 §4 서두).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SeekConfig {
    /// `Remap key to Seek:` — 팝업이 `-`(미설정)면 `None`. 기본 `None`
    /// (실측: AX 트리 + `defaults` 부재).
    pub remap_key: Option<KeyCode>,
    /// `Only show while the remapped key is held`(저장 키
    /// `seekExecuteOnClose`). 기본 ☐. `remap_key == None` 이면 UI 에서
    /// 비활성(dimmed)이고, 이 필드도 그 상황에서는 의미가 없다 —
    /// [`SeekConfig::mode_for`] 는 `remap_key` 가 있을 때만 이 값을 본다.
    pub execute_on_close: bool,
    /// `Semicolon highlights next match`(저장 키 `semicolonCycleSeek`).
    /// 기본 ☐.
    pub semicolon_cycles: bool,
    /// `Presets` 탭 `Quick press caps lock to execute:` 의 선택지가 `Seek`
    /// 인가(명세 §2 시나리오 D, §4 표 마지막 행). 기본 ☐.
    pub quick_press_opens: bool,
    /// `Toggle Seek with shortcut:` — 미설정이면 `None`. 기본 `None`
    /// (실측: 버튼 라벨 `Record Shortcut`, 빈 값).
    ///
    /// 실제 시스템 핫키 **등록**은 앱(`global-hotkey` 크레이트)의 몫이고, 이
    /// 상태 머신은 두 가지에만 이 값을 쓴다: ① 그 활성화 경로가 존재하는가
    /// ([`SeekConfig::any_activation_configured`]), ② ⭐ **세션이 열려 있는 동안
    /// 같은 조합이 다시 눌렸는가**([`SeekConfig::matches_global_shortcut`]).
    ///
    /// ⭐ ②가 왜 필요한가 — **실기 검증이 찾은 결함이다.** 명세 §3.2 는 "toggle
    /// 모드에서 동일한 전역 단축키 재입력은 닫기로 처리한다" 고 정하는데, 세션이
    /// 열려 있는 동안에는 F-07 의 `CGEventTap` 이 계층 1 에서 **모든 키 이벤트를
    /// 소비**하므로(§3.1) 그 조합이 윈도 서버의 핫키 디스패치까지 **도달하지
    /// 못한다.** 즉 `global-hotkey` 는 두 번째 입력을 영영 보지 못하고, 세션은
    /// 그 경로로 닫힐 수 없다. 리매핑 키 경로는 트리거 키 판정이 계층 1 **앞**에
    /// 있어 이 문제가 없다. 그래서 전역 단축키만은 조합 자체를 여기 들고 있다가
    /// 세션 중 키 라우팅에서 직접 알아본다.
    pub global_shortcut: Option<(KeyCode, EventFlags)>,
}

impl SeekConfig {
    /// 세 활성화 경로(§1) 중 하나라도 설정돼 있는가. 전부 `false`/`None`
    /// 이면(= [`SeekConfig::default()`]) Seek 를 발동할 방법이 없다는 뜻이다
    /// — 온보딩 UI 가 이 값으로 "설정을 먼저 하라"는 안내를 띄울지 판단한다.
    #[must_use]
    pub fn any_activation_configured(&self) -> bool {
        self.global_shortcut.is_some() || self.remap_key.is_some() || self.quick_press_opens
    }

    /// 이 이벤트가 `Toggle Seek with shortcut:` 조합 그 자체인가.
    ///
    /// ⭐ 세션이 열려 있는 동안의 **재입력 판정 전용**이다([`SeekConfig::
    /// global_shortcut`] 문서의 ② 참조). 비교는 물리 키코드 + **네 modifier
    /// 비트**(⇧⌃⌥⌘)로만 한다 — `ev.flags` 에는 macOS 가 늘 얹는 시스템 비트와
    /// caps lock 잠금 비트(`alphaShift`)가 함께 실려 오므로 그대로 비교하면
    /// 영영 일치하지 않는다(실측: 릴리즈 flags 가 `0x20010000` 이었다).
    #[must_use]
    pub fn matches_global_shortcut(&self, ev: &InputEvent) -> bool {
        const MODS: EventFlags = EventFlags(
            EventFlags::SHIFT.0
                | EventFlags::CONTROL.0
                | EventFlags::ALTERNATE.0
                | EventFlags::COMMAND.0,
        );
        let Some((keycode, wanted)) = self.global_shortcut else {
            return false;
        };
        ev.keycode == keycode && EventFlags(ev.flags.0 & MODS.0) == EventFlags(wanted.0 & MODS.0)
    }

    /// 이 경로로 세션을 열면 어떤 모드가 되는가 — 명세 §3.2 상태표를 그대로
    /// 판정한다.
    #[must_use]
    pub fn mode_for(&self, path: ActivationPath) -> SessionMode {
        match path {
            // §3.2 1행 — 전역 단축키는 항상 toggle. hold 로 만들 수단이 없다
            // (릴리즈 이벤트가 없는 시스템 핫키 등록 방식, 명세 §6).
            ActivationPath::GlobalShortcut => SessionMode::Toggle,

            // §3.2 2·3행 — `Only show while the remapped key is held` 이
            // 그대로 hold/toggle 을 가른다.
            ActivationPath::RemapKey => {
                if self.execute_on_close {
                    SessionMode::Hold
                } else {
                    SessionMode::Toggle
                }
            }

            // ⭐ 명세 §3.2 4행·§9 #8 이 `(미확정)`으로 남긴 자리 — **이 구현이
            // `Toggle` 로 확정한다.**
            //
            // 근거(명세 §2 시나리오 D 가 이미 적어 둔 그대로): quick press 는
            // 정의상 **키를 뗀 뒤에야 판정이 성립**한다("짧게 누르고 뗌"이
            // 트리거이지, 누르고 있는 동작이 아니다). F-01 이 이 경로로
            // `activate()` 호출을 받는 시점에는 이미 그 키가 눌려 있지 않다
            // — hold 모드로 만들려면 "트리거 키의 릴리즈" 를 확정 신호로
            // 기다려야 하는데, 판정이 끝난 시점에 그 키는 이미 뗀 상태이므로
            // 기다릴 릴리즈 이벤트 자체가 존재하지 않는다. 그렇게 만들면
            // 세션이 영영 닫히지 않는다. 따라서 hold 는 이 경로에서
            // 구조적으로 성립할 수 없고, 남는 선택지는 toggle 뿐이다.
            ActivationPath::QuickPressCapsLock => SessionMode::Toggle,
        }
    }
}

/// 세션을 연 경로. §3.3 "다른 경로의 트리거는 무시" 판정에 쓴다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationPath {
    /// `Toggle Seek with shortcut:` — 전역 단축키(시스템 핫키 등록).
    GlobalShortcut,
    /// `Remap key to Seek:` — `CGEventTap` 리매핑 트리거.
    RemapKey,
    /// `Presets` 탭 `Quick press caps lock to execute:` = `Seek`.
    QuickPressCapsLock,
}

/// 세션이 닫히는 방식 — 명세 §3.2.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMode {
    /// 다시 트리거하면 닫힌다. `Enter` 로 확정한다.
    Toggle,
    /// 트리거 키를 **떼는 순간이 곧 확정**이다 — "Release the remapped key
    /// to click"(명세 §2 시나리오 B).
    Hold,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐ 명세 §1·§4 — 출고 기본값은 전부 미설정이라 `any_activation_configured`
    /// 가 `false` 여야 한다.
    #[test]
    fn default_config_has_no_activation_path_configured() {
        let config = SeekConfig::default();
        assert_eq!(config.remap_key, None);
        assert!(!config.execute_on_close);
        assert!(!config.semicolon_cycles);
        assert!(!config.quick_press_opens);
        assert!(config.global_shortcut.is_none());
        assert!(!config.any_activation_configured());
    }

    /// 세 경로 중 하나만 켜져도 `any_activation_configured` 가 `true`.
    #[test]
    fn any_activation_configured_true_if_any_single_path_set() {
        let config = SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        };
        assert!(config.any_activation_configured());

        let config = SeekConfig {
            remap_key: Some(KeyCode::CAPS_LOCK),
            ..SeekConfig::default()
        };
        assert!(config.any_activation_configured());

        let config = SeekConfig {
            quick_press_opens: true,
            ..SeekConfig::default()
        };
        assert!(config.any_activation_configured());
    }

    /// §3.2 1행 — 전역 단축키는 `execute_on_close` 값과 무관하게 항상 toggle.
    #[test]
    fn global_shortcut_is_always_toggle() {
        let mut config = SeekConfig::default();
        assert_eq!(
            config.mode_for(ActivationPath::GlobalShortcut),
            SessionMode::Toggle
        );
        config.execute_on_close = true;
        assert_eq!(
            config.mode_for(ActivationPath::GlobalShortcut),
            SessionMode::Toggle
        );
    }

    /// §3.2 2·3행 — 리매핑 키는 `execute_on_close` 그대로 hold/toggle 을 가른다.
    #[test]
    fn remap_key_mode_follows_execute_on_close() {
        let mut config = SeekConfig::default();
        assert_eq!(
            config.mode_for(ActivationPath::RemapKey),
            SessionMode::Toggle
        );
        config.execute_on_close = true;
        assert_eq!(config.mode_for(ActivationPath::RemapKey), SessionMode::Hold);
    }

    /// ⭐ §3.2 4행·§9 #8 — quick press caps lock 은 이 구현이 `Toggle` 로
    /// 확정한다. `execute_on_close` 를 켜도 이 경로의 모드는 바뀌지 않는다
    /// (그 설정은 `RemapKey` 경로 전용이다).
    #[test]
    fn quick_press_caps_lock_is_toggle_regardless_of_execute_on_close() {
        let mut config = SeekConfig {
            quick_press_opens: true,
            ..SeekConfig::default()
        };
        assert_eq!(
            config.mode_for(ActivationPath::QuickPressCapsLock),
            SessionMode::Toggle
        );
        config.execute_on_close = true;
        assert_eq!(
            config.mode_for(ActivationPath::QuickPressCapsLock),
            SessionMode::Toggle
        );
    }
}
