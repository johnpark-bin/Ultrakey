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
///
/// ⚠️ `to_disable` 이 `&'static [..]` 이 아니라 `Vec` 인 이유: `CapsLockAlreadyRemapped`
/// 의 배타 대상은 **지금 caps lock 을 소스로 쓰고 있는 hyper/meh/bleh 슬롯**이라
/// 실행 시점에야 정해진다(정적 상수로 표현할 수 없다). 이 타입은 설정 UI 경로에서만
/// 만들어지고 콜백 임계 경로에는 들어가지 않으므로 할당은 문제가 되지 않는다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub kind: ConflictKind,
    pub to_disable: Vec<&'static str>,
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
/// ⭐ 반대 방향(hyper/meh/bleh 소스를 caps lock 으로 켜려는데 `Remap caps lock to:` 가
/// 이미 켜져 있는 경우)은 [`detect_modifier_slot_conflict`] 가 대칭적으로 다룬다.
///
/// `caps_modifier_slots` 는 **지금 caps lock 을 소스로 쓰고 있는 활성 hyper/meh/bleh
/// 슬롯의 저장 키** 목록이다(비어 있으면 그런 슬롯이 없다는 뜻).
pub fn detect_conflict(
    current: &PresetSettings,
    caps_modifier_slots: &[&'static str],
    changing_key: &str,
    new_value: bool,
) -> Option<Conflict> {
    if !new_value {
        // 끄는 동작은 충돌을 만들지 않는다 — 충돌은 항상 "새로 켜려는" 쪽에서만 발생한다.
        return None;
    }

    match changing_key {
        k if k == keys::PRESETS_CAPS_LOCK_REMAP_ENABLED => {
            // ⭐ 배타 대상은 **상대**다 — 지금 caps lock 을 소스로 쓰는 hyper/meh/bleh
            // 슬롯을 끈다. (이전 판은 지금 켜려는 설정 자신을 목록에 넣는 버그가
            // 있었고, 대화상자가 "이걸 켜면 이걸 끕니다"라고 같은 항목을 가리켰다 —
            // 실기기 검증에서 잡았다.)
            if caps_modifier_slots.is_empty() {
                None
            } else {
                Some(Conflict {
                    kind: ConflictKind::CapsLockAlreadyRemapped,
                    to_disable: caps_modifier_slots.to_vec(),
                })
            }
        }

        k if k == keys::PRESETS_CAPS_WASD_ARROWS => {
            if current.caps_home_row.enabled {
                Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_HOME_ROW.to_vec() })
            } else if current.caps_hjkl_arrows.enabled {
                Some(Conflict { kind: ConflictKind::CapsLockArrows, to_disable: DISABLE_HJKL.to_vec() })
            } else {
                None
            }
        }

        k if k == keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED => {
            if current.caps_home_row.enabled {
                Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_HOME_ROW.to_vec() })
            } else if current.caps_wasd_arrows {
                Some(Conflict { kind: ConflictKind::CapsLockArrows, to_disable: DISABLE_WASD.to_vec() })
            } else {
                None
            }
        }

        k if k == keys::PRESETS_CAPS_HOME_ROW_ENABLED => {
            match (current.caps_wasd_arrows, current.caps_hjkl_arrows.enabled) {
                (true, true) => {
                    Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_WASD_AND_HJKL.to_vec() })
                }
                (true, false) => Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_WASD.to_vec() }),
                (false, true) => Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_HJKL.to_vec() }),
                (false, false) => None,
            }
        }

        // F-08.4(space)를 비롯한 나머지 전부 — 배타 대상이 없다.
        _ => None,
    }
}

/// ⭐ 반대 방향 — hyper/meh/bleh 슬롯을 caps lock 소스로 **켜려는데**
/// `Remap caps lock to:`(F-08.1)가 이미 켜져 있는 경우.
///
/// `detect_conflict` 와 대칭이어야 하는 이유: 두 설정은 서로 다른 탭에 있고,
/// 사용자가 어느 쪽을 먼저 켜느냐에 따라 한쪽만 경고가 뜨면 "왜 이번엔 안 물어보지"
/// 라는 비대칭이 생긴다(`key-remapping-engine.md` §3-b "동일 소스 키 중복 배정 방지"
/// 가 UI 경고를 2차 방어선으로 둔 취지).
pub fn detect_modifier_slot_conflict(current: &PresetSettings) -> Option<Conflict> {
    if current.caps_lock_remap.enabled {
        Some(Conflict {
            kind: ConflictKind::CapsLockAlreadyRemapped,
            to_disable: DISABLE_CAPS_LOCK_REMAP.to_vec(),
        })
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ⭐ 배타 대상은 **상대**(hyper 슬롯)여야 한다 — 지금 켜려는 설정 자신이 아니다.
    /// 실기기 검증에서 대화상자가 "이걸 켜면 이걸 끕니다"라고 같은 항목을 가리키는
    /// 것을 보고 잡은 회귀다.
    #[test]
    fn caps_lock_already_remapped_disables_the_modifier_slot_not_itself() {
        let current = PresetSettings::default();
        let conflict = detect_conflict(
            &current,
            &[keys::HYPERKEY_HYPER_ENABLED],
            keys::PRESETS_CAPS_LOCK_REMAP_ENABLED,
            true,
        );
        assert_eq!(
            conflict,
            Some(Conflict {
                kind: ConflictKind::CapsLockAlreadyRemapped,
                to_disable: vec![keys::HYPERKEY_HYPER_ENABLED],
            })
        );
        // caps lock 을 쓰는 슬롯이 없으면 충돌이 아니다.
        assert_eq!(
            detect_conflict(&current, &[], keys::PRESETS_CAPS_LOCK_REMAP_ENABLED, true),
            None
        );
    }

    /// ⭐ 반대 방향도 대칭으로 감지된다.
    #[test]
    fn modifier_slot_conflict_is_symmetric() {
        let remap_on = PresetSettings {
            caps_lock_remap: crate::settings::CapsLockRemapSettings {
                enabled: true,
                ..Default::default()
            },
            ..PresetSettings::default()
        };
        assert_eq!(
            detect_modifier_slot_conflict(&remap_on),
            Some(Conflict {
                kind: ConflictKind::CapsLockAlreadyRemapped,
                to_disable: vec![keys::PRESETS_CAPS_LOCK_REMAP_ENABLED],
            })
        );
        assert_eq!(detect_modifier_slot_conflict(&PresetSettings::default()), None);
    }

    /// R2 예외 — caps lock 조합 프리셋(WASD 등)은 hyper 소스가 caps lock 이어도 충돌이
    /// 아니다.
    #[test]
    fn caps_lock_combo_presets_do_not_conflict_with_hyper_source() {
        let current = PresetSettings::default();
        assert_eq!(detect_conflict(&current, &[keys::HYPERKEY_HYPER_ENABLED], keys::PRESETS_CAPS_WASD_ARROWS, true), None);
        assert_eq!(detect_conflict(&current, &[keys::HYPERKEY_HYPER_ENABLED], keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED, true), None);
        assert_eq!(detect_conflict(&current, &[keys::HYPERKEY_HYPER_ENABLED], keys::PRESETS_CAPS_HOME_ROW_ENABLED, true), None);
    }

    #[test]
    fn caps_lock_arrows_conflict_both_directions() {
        let hjkl_on = PresetSettings {
            caps_hjkl_arrows: crate::settings::CapsHjklArrowsSettings { enabled: true, ..Default::default() },
            ..PresetSettings::default()
        };
        let conflict = detect_conflict(&hjkl_on, &[], keys::PRESETS_CAPS_WASD_ARROWS, true);
        assert_eq!(conflict, Some(Conflict { kind: ConflictKind::CapsLockArrows, to_disable: DISABLE_HJKL.to_vec() }));

        let wasd_on = PresetSettings { caps_wasd_arrows: true, ..PresetSettings::default() };
        let conflict2 = detect_conflict(&wasd_on, &[], keys::PRESETS_CAPS_HJKL_ARROWS_ENABLED, true);
        assert_eq!(conflict2, Some(Conflict { kind: ConflictKind::CapsLockArrows, to_disable: DISABLE_WASD.to_vec() }));
    }

    #[test]
    fn caps_lock_home_row_conflict_against_wasd_and_hjkl() {
        let wasd_on = PresetSettings { caps_wasd_arrows: true, ..PresetSettings::default() };
        assert_eq!(
            detect_conflict(&wasd_on, &[], keys::PRESETS_CAPS_HOME_ROW_ENABLED, true),
            Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_WASD.to_vec() })
        );

        let home_row_on = PresetSettings {
            caps_home_row: crate::settings::CapsHomeRowSettings { enabled: true, ..Default::default() },
            ..PresetSettings::default()
        };
        assert_eq!(
            detect_conflict(&home_row_on, &[], keys::PRESETS_CAPS_WASD_ARROWS, true),
            Some(Conflict { kind: ConflictKind::CapsLockHomeRow, to_disable: DISABLE_HOME_ROW.to_vec() })
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
        assert_eq!(detect_conflict(&everything_on, &[keys::HYPERKEY_HYPER_ENABLED], keys::PRESETS_CAPS_SPACE_ENTER, true), None);
    }

    #[test]
    fn turning_off_never_conflicts() {
        let current = PresetSettings::default();
        assert_eq!(detect_conflict(&current, &[keys::HYPERKEY_HYPER_ENABLED], keys::PRESETS_CAPS_LOCK_REMAP_ENABLED, false), None);
    }
}
