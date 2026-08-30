//! 설정 충돌 감지(`docs/dev/architecture.md` §6.5) — 대화형 배타 선택(P12). 자동
//! 우선순위가 아니라 "상대 설정을 끌까요?" 라고 사용자에게 묻는 방식을 채택한 이유는
//! §6.5 상단 산문과 `power-user-presets.md` §3.3 R8 의 "채택 권고" 절을 참고하라.

use ultrakey_core::settings::keys;

use crate::settings::PresetSettings;

/// 대화상자 3종의 내부 식별자.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictKind {
    /// `Remap caps lock to:` 를 켜려는데 hyper/meh/bleh 소스가 이미 caps lock.
    CapsLockAlreadyRemapped,
    /// `Caps lock + W A S D` 와 `Caps lock + [H J K L]` 중 꺼진 쪽을 켜려 함.
    CapsLockArrows,
    /// `Caps lock + home row` 와 방향키 프리셋(F-08.5·F-08.6) 중 한쪽을 켜려 함.
    CapsLockHomeRow,
}

/// 충돌 하나 — 어떤 종류인지와, 해소하려면 꺼야 할 설정 키 목록.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Conflict {
    pub kind: ConflictKind,
    pub to_disable: &'static [&'static str],
}

const DISABLE_CAPS_LOCK_REMAP: &[&str] = &[keys::PRESETS_CAPS_LOCK_REMAP_ENABLED];
const DISABLE_HJKL: &[&str] = &[keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED];
const DISABLE_WASD: &[&str] = &[keys::PRESETS_CAPS_WASD_ARROWS];
const DISABLE_HOME_ROW: &[&str] = &[keys::PRESETS_CAPS_HOME_ROW_ENABLED];
const DISABLE_WASD_AND_HJKL: &[&str] =
    &[keys::PRESETS_CAPS_WASD_ARROWS, keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED];

/// 이 설정 키를 이 값으로 바꾸려 할 때 발생하는 충돌. 없으면 `None`.
///
/// ⭐ **F-08.4(`Caps lock + space`)는 어느 그룹과도 충돌하지 않는다** — `space` 는 세
/// 집합(WASD/HJKL/home row) 어디에도 없으므로 이 함수는 `PRESETS_CAPS_SPACE_ENTER` 에
/// 대해 항상 `None` 을 반환한다(architecture.md §6.5).
///
/// ⭐ **caps lock 조합 프리셋(F-08.4~7)과 "hyper 소스 = caps lock" 은 충돌로 다루지
/// 않는다** — R2(v1.20 회귀 방지)가 "두 구성이 동시에 성립해야 한다"를 수용 기준으로
/// 명시했기 때문이다. 대화상자 #1(`CapsLockAlreadyRemapped`)의 배타 대상은
/// `Remap caps lock to:`(F-08.1) 쪽뿐이다 — 그래서 `caps_is_modifier_source` 는 오직
/// `PRESETS_CAPS_LOCK_REMAP_ENABLED` 를 켜는 경우에만 참조된다.
///
/// ⚠️ "그 반대"(hyper 소스를 caps lock 으로 지정하려는데 `Remap caps lock to:` 가 이미
/// 켜져 있는 경우)는 이 함수의 범위 밖이다 — `changing_key` 가 항상 `presets.*` 키라고
/// 전제하기 때문이다. 반대 방향의 감지는 hyper/meh/bleh 소스 키를 다루는 쪽
/// (`ultrakey-hyperkey` 또는 설정 UI 조합 지점)이 대칭적으로 구현해야 한다.
pub fn detect_conflict(
    current: &PresetSettings,
    caps_is_modifier_source: bool,
    changing_key: &str,
    new_value: bool,
) -> Option<Conflict> {
    if !new_value {
        // 끄는 동작은 충돌을 만들지 않는다 — 충돌은 항상 "새로 켜려는" 쪽에서만 발생한다.
        return None;
    }

    match changing_key {
        k if k == keys::PRESETS_CAPS_LOCK_REMAP_ENABLED => {
            if caps_is_modifier_source {
                Some(Conflict { kind: ConflictKind::CapsLockAlreadyRemapped, to_disable: DISABLE_CAPS_LOCK_REMAP })
            } else {
                None
            }
        }

        k if k == keys::PRESETS_CAPS_WASD_ARROWS => {
            if current.caps_home_row.enabled {
                Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_HOME_ROW })
            } else if current.caps_hjkl_arrows.enabled {
                Some(Conflict { kind: ConflictKind::CapsLockArrows, to_disable: DISABLE_HJKL })
            } else {
                None
            }
        }

        k if k == keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED => {
            if current.caps_home_row.enabled {
                Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_HOME_ROW })
            } else if current.caps_wasd_arrows {
                Some(Conflict { kind: ConflictKind::CapsLockArrows, to_disable: DISABLE_WASD })
            } else {
                None
            }
        }

        k if k == keys::PRESETS_CAPS_HOME_ROW_ENABLED => {
            match (current.caps_wasd_arrows, current.caps_hjkl_arrows.enabled) {
                (true, true) => {
                    Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_WASD_AND_HJKL })
                }
                (true, false) => Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_WASD }),
                (false, true) => Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_HJKL }),
                (false, false) => None,
            }
        }

        // F-08.4(space)를 비롯한 나머지 전부 — 배타 대상이 없다.
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_lock_already_remapped_conflict() {
        let current = PresetSettings::default();
        let conflict = detect_conflict(&current, true, keys::PRESETS_CAPS_LOCK_REMAP_ENABLED, true);
        assert_eq!(
            conflict,
            Some(Conflict { kind: ConflictKind::CapsLockAlreadyRemapped, to_disable: DISABLE_CAPS_LOCK_REMAP })
        );
    }

    /// R2 예외 — caps lock 조합 프리셋(WASD 등)은 hyper 소스가 caps lock 이어도 충돌이
    /// 아니다.
    #[test]
    fn caps_lock_combo_presets_do_not_conflict_with_hyper_source() {
        let current = PresetSettings::default();
        assert_eq!(detect_conflict(&current, true, keys::PRESETS_CAPS_WASD_ARROWS, true), None);
        assert_eq!(detect_conflict(&current, true, keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED, true), None);
        assert_eq!(detect_conflict(&current, true, keys::PRESETS_CAPS_HOME_ROW_ENABLED, true), None);
    }

    #[test]
    fn caps_lock_arrows_conflict_both_directions() {
        let hjkl_on = PresetSettings {
            caps_hjkl_arrows: crate::settings::CapsHjklArrowsSettings { enabled: true, ..Default::default() },
            ..PresetSettings::default()
        };
        let conflict = detect_conflict(&hjkl_on, false, keys::PRESETS_CAPS_WASD_ARROWS, true);
        assert_eq!(conflict, Some(Conflict { kind: ConflictKind::CapsLockArrows, to_disable: DISABLE_HJKL }));

        let wasd_on = PresetSettings { caps_wasd_arrows: true, ..PresetSettings::default() };
        let conflict2 = detect_conflict(&wasd_on, false, keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED, true);
        assert_eq!(conflict2, Some(Conflict { kind: ConflictKind::CapsLockArrows, to_disable: DISABLE_WASD }));
    }

    #[test]
    fn caps_lock_home_row_conflict_against_wasd_and_hjkl() {
        let wasd_on = PresetSettings { caps_wasd_arrows: true, ..PresetSettings::default() };
        assert_eq!(
            detect_conflict(&wasd_on, false, keys::PRESETS_CAPS_HOME_ROW_ENABLED, true),
            Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_WASD })
        );

        let home_row_on = PresetSettings {
            caps_home_row: crate::settings::CapsHomeRowSettings { enabled: true, ..Default::default() },
            ..PresetSettings::default()
        };
        assert_eq!(
            detect_conflict(&home_row_on, false, keys::PRESETS_CAPS_WASD_ARROWS, true),
            Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_HOME_ROW })
        );
    }

    /// F-08.4(space)는 어느 그룹과도 충돌하지 않는다.
    #[test]
    fn caps_space_enter_never_conflicts() {
        let everything_on = PresetSettings {
            caps_wasd_arrows: true,
            caps_hjkl_arrows: crate::settings::CapsHjklArrowsSettings { enabled: true, ..Default::default() },
            caps_home_row: crate::settings::CapsHomeRowSettings { enabled: true, ..Default::default() },
            ..PresetSettings::default()
        };
        assert_eq!(detect_conflict(&everything_on, true, keys::PRESETS_CAPS_SPACE_ENTER, true), None);
    }

    #[test]
    fn turning_off_never_conflicts() {
        let current = PresetSettings::default();
        assert_eq!(detect_conflict(&current, true, keys::PRESETS_CAPS_LOCK_REMAP_ENABLED, false), None);
    }
}
