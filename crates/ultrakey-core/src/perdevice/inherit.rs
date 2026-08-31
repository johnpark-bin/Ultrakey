//! F-17 "공통 설정 복사"(`docs/plan/issue-46-rule-inherit.md` D1·D3) — 어떤 키·값을
//! 공통(`For all devices`) 계층에서 디바이스 계층으로 복사할지 판정하는 순수 헬퍼.
//!
//! ⭐ 이 모듈은 **복사 계획만** 만든다 — 저장소에 실제로 쓰는 것은
//! `ultrakey-app` 의 `settings_copy_common_to_device` 커맨드다. 계획을 순수 함수로
//! 분리한 이유는 "무엇이 복사 가능한가 / 무엇을 복사하는가"가 제품 의미론(D3 —
//! 기능 2 는 희소 복사, 옛 `SystemFunction` 저장 표현도 해석 없이 그대로 실림)이라
//! 테스트로 못박아야 하기 때문이다. `PerDeviceSettings` 위의 판정이라 macOS
//! 의존이 없다(`#![forbid(unsafe_code)]` 유지).
//!
//! **의미론**(계획 §3): 복사는 공통 계층의 명시적 값들을 복사 시점 그대로 디바이스
//! 계층에 물질화하는 것이고, 일단 복사된 값은 공통이 나중에 바뀌어도 따라가지 않는다
//! (독립 스냅샷). 복사되지 않은 키는 계속 상속한다 — "스냅샷"의 단위는 '복사 시점에
//! 공통에 값이 있던 항목'이고, 그 밖은 상속이 유지된다.

use crate::perdevice::{FKey, KeyRemapRow, PerDeviceSettings, Tri};

/// 기능 1 복사 계획 — 공통 계층의 `keyRemap.rows` 가 명시적 **비어 있지 않은**
/// 배열(`Tri::Value`)일 때만 `Some(rows)` 를 돌려준다.
///
/// - `Tri::Inherit`(키 부재) / `Tri::Off`(JSON `null` = 끔) / 빈 배열(이론상 수기
///   파일에만 존재) → `None`(복사할 것이 없다).
///
/// 반환값은 소유 타입(`Vec`)이라 호출자의 store borrow 가 여기서 끝난다 —
/// 계획 계산 뒤 같은 락 안에서 쓰기로 이어질 수 있다(`settings_copy_common_to_device`).
pub fn plan_key_remap_copy(settings: &PerDeviceSettings<'_>) -> Option<Vec<KeyRemapRow>> {
    match settings.key_remap_rows_common() {
        Tri::Value(rows) if !rows.is_empty() => Some(rows),
        _ => None,
    }
}

/// 기능 2 복사 계획 — **희소 복사**(D3). 공통 계층에서 명시적 목적지 id 문자열
/// (`Tri::Value(id)`)인 F-키만 `(FKey, id)` 로 수집한다.
///
/// - 공통의 `null`(끔) 키는 복사할 필요가 없다 — 디바이스 키가 부재면 이미 공통의
///   `null` 을 상속한다(같은 결과). 부재 키도 마찬가지.
/// - 옛 `SystemFunction` variant 이름(마이그레이션 대상)이 공통에 있으면 그것도
///   **그대로** 수집한다 — 복사는 해석이 아니라 이동이고, 해석은
///   [`crate::perdevice::destinations::resolve_stored`] 가 읽는 시점에 한다(계획 §3
///   "레거시 값 호환").
pub fn plan_function_keys_copy(settings: &PerDeviceSettings<'_>) -> Vec<(FKey, String)> {
    FKey::all()
        .iter()
        .copied()
        .filter_map(|f| match settings.function_key_common(f) {
            Tri::Value(id) => Some((f, id)),
            Tri::Inherit | Tri::Off => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keycode::SourceKey;
    use crate::perdevice::{FKey, KeyRemapRow, PerDeviceSettings, SystemFunction};
    use crate::settings::keys as settings_keys;
    use serde_json::Value;
    use std::collections::BTreeMap;

    fn row(from: SourceKey, to: SourceKey) -> KeyRemapRow {
        KeyRemapRow { from, to }
    }

    // ── 기능 1 — 공통 계층의 3상태 × 계획 판정 ─────────────────────────────────

    #[test]
    fn plan_key_remap_copy_returns_common_rows_when_value_and_non_empty() {
        let common_key =
            settings_keys::per_device_key_remap_rows(settings_keys::PER_DEVICE_COMMON_SCOPE);
        let values = BTreeMap::from([(
            common_key,
            serde_json::to_value(vec![row(SourceKey::CapsLock, SourceKey::F18)]).unwrap(),
        )]);
        let settings = PerDeviceSettings::new(&values);

        let plan = plan_key_remap_copy(&settings).expect("Value + 비어 있지 않음 → Some");

        assert_eq!(plan, vec![row(SourceKey::CapsLock, SourceKey::F18)]);
    }

    #[test]
    fn plan_key_remap_copy_none_when_common_absent() {
        let values = BTreeMap::new();
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(plan_key_remap_copy(&settings), None);
    }

    #[test]
    fn plan_key_remap_copy_none_when_common_off() {
        let common_key =
            settings_keys::per_device_key_remap_rows(settings_keys::PER_DEVICE_COMMON_SCOPE);
        let values = BTreeMap::from([(common_key, Value::Null)]);
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(plan_key_remap_copy(&settings), None);
    }

    #[test]
    fn plan_key_remap_copy_none_when_common_empty_array() {
        // 수기 파일에만 존재할 수 있는 `[]` — "복사할 것이 없다"와 동일하게 취급한다.
        let common_key =
            settings_keys::per_device_key_remap_rows(settings_keys::PER_DEVICE_COMMON_SCOPE);
        let values = BTreeMap::from([(common_key, serde_json::json!([]))]);
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(plan_key_remap_copy(&settings), None);
    }

    // ── 기능 2 — 희소 복사(D3) ────────────────────────────────────────────────

    /// 공통 f1=Value(`consumer.mute`), f2=Off(`null`), f3=부재 → `[(F1, id)]` 만
    /// — `null`·부재 키는 복사하지 않는다(희소 복사 확정).
    #[test]
    fn plan_function_keys_copy_only_includes_value_keys() {
        let f1_key = settings_keys::per_device_function_key(
            settings_keys::PER_DEVICE_COMMON_SCOPE,
            FKey::F1,
        );
        let f2_key = settings_keys::per_device_function_key(
            settings_keys::PER_DEVICE_COMMON_SCOPE,
            FKey::F2,
        );
        let values = BTreeMap::from([
            (f1_key, Value::String("consumer.mute".to_string())),
            (f2_key, Value::Null),
            // f3 키는 아예 없다(부재).
        ]);
        let settings = PerDeviceSettings::new(&values);

        let plan = plan_function_keys_copy(&settings);

        assert_eq!(plan, vec![(FKey::F1, "consumer.mute".to_string())]);
    }

    /// 옛 `SystemFunction` 저장값(`"Mute"`)도 계획에 그대로 실린다 — 복사는 해석이
    /// 아니라 이동이고, 저장 표현을 바꾸지 않는다(계획 D3 "레거시 값 호환").
    #[test]
    fn plan_function_keys_copy_preserves_id_strings_from_common() {
        let f9_key = settings_keys::per_device_function_key(
            settings_keys::PER_DEVICE_COMMON_SCOPE,
            FKey::F9,
        );
        let values =
            BTreeMap::from([(f9_key, serde_json::to_value(SystemFunction::Mute).unwrap())]);
        let settings = PerDeviceSettings::new(&values);

        let plan = plan_function_keys_copy(&settings);

        // `"Mute"`(옛 variant 이름)가 그대로 — 새 id 로 바꾸지 않는다.
        assert_eq!(plan, vec![(FKey::F9, "Mute".to_string())]);
    }

    /// 공통 계층에 Value 인 F-키가 하나도 없으면 빈 계획 — 백엔드 커맨드가 아무것도
    /// 쓰지 않고 그대로 돌아가야 한다(`settings_copy_common_to_device` 의 멱등 경로).
    #[test]
    fn plan_function_keys_copy_empty_when_no_value_keys() {
        let values = BTreeMap::new();
        let settings = PerDeviceSettings::new(&values);
        assert_eq!(plan_function_keys_copy(&settings), Vec::new());
    }
}
