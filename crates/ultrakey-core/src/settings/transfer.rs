//! 설정 export / import — F-15 [`settings-store-and-integrity.md`] §3.4 (이슈 #39).
//!
//! ⭐ **이 모듈의 설계 전체가 §3.1 의 "부재 = 기본값" 규약에서 도출된다.** 그 규약을
//! 지키지 못하는 설계는 전부 기각됐다. 요약하면 넷이다.
//!
//! 1. **export 는 "건드린 키만" 담는다**(§3.4 결정 1). 전체 유효값을 펼쳐 담으면,
//!    그 파일을 import 한 기기의 저장 파일이 키 몇 개에서 수백 개로 부풀고 **그
//!    순간 "부재 = 기본값" 규약이 그 기기에서 영구히 깨진다.** 구체적 손실:
//!    다음 버전에서 어떤 항목의 출고 기본값을 바꾸면, 규약이 살아 있는 사용자에게는
//!    새 기본값이 적용되지만 전체가 펼쳐진 사용자에게는 **옛 기본값이 명시적
//!    사용자 선택으로 굳어 영원히 도달하지 못한다.**
//! 2. **기기 상태인 키는 뺀다**(§3.4 결정 3) — [`EXCLUDED_PREFIXES`].
//! 3. **import 는 병합이 아니라 교체다**(§3.4 결정 4). 병합은 결과가 지금 상태에
//!    따라 달라지고, 무엇보다 **끈 설정이 되살아나지 않는다** — 파일에 키가 없는데
//!    (= 기본값) 지금 기기에 값이 있으면 그 값이 살아남아 "불러왔는데 안 불러와졌다"가
//!    된다. 교체는 **파일이 곧 결과다.**
//! 4. **없는 디바이스의 설정은 보존하되 비활성**(§3.4 결정 5). 특별한 처리가 아니라
//!    F-17 이 평소에 하는 일이다 — 연결되지 않은 디바이스의 키는 원래 잠들어 있다.
//!
//! ⛔ **실패는 저장소를 건드리지 않는다**(§3.4 결정 7). 판정은 전부 읽는 단계에서
//! 끝내고, 하나라도 걸리면 저장소를 전혀 건드리지 않은 채 실패를 돌려준다.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use super::store::{SettingsStore, StoreError, SCHEMA_VERSION};

/// export 파일의 `kind` 표식. ⭐ 엉뚱한 JSON 을 설정으로 삼키지 않기 위한 것이다 —
/// 이 값이 다르면 import 를 거절한다.
pub const EXPORT_KIND: &str = "ultrakey.settings-export";

/// export 파일 자체의 형식 버전. `schema_version`(설정 키들의 스키마)과 다른 축이다.
pub const EXPORT_FORMAT_VERSION: u32 = 1;

/// ⭐ **export 에서 빼고 import 에서 받지 않는 키 접두사**(§3.4 결정 3).
///
/// "건드린 키만" 담는다고 해서 *전부* 담는 것은 아니다. 저장 파일에는 사용자의
/// 의사가 아니라 **이 기기의 상태**인 키가 섞여 있다.
///
/// - `perDevice._managed` — 우리가 이 기기의 커널에 실제로 써 넣은 `hidutil` 매핑의
///   원장(D-17-2)이다. 사용자 설정이 아니라 "지금 이 기기에서 우리가 청소해야 할
///   것들"의 목록이다. 다른 기기로 옮기면 그 기기의 정리 로직이 **자기가 쓰지도
///   않은 배열을 자기 것이라고 착각한다.**
/// - `ui.` — 창 크기와 마지막 탭은 세션·디스플레이에 묶인 상태다. 세로 해상도가
///   다른 기기로 옮기면 창이 화면을 넘칠 수 있고(F-09 §3.3 의 알려진 한계), 얻는
///   것이 없다.
pub const EXCLUDED_PREFIXES: [&str; 2] = ["perDevice._managed", "ui."];

/// 디바이스별 설정 키의 접두사(F-17). `perDevice.all.` 과 `perDevice._managed` 는
/// 디바이스 식별자가 아니다.
const PER_DEVICE_PREFIX: &str = "perDevice.";
const PER_DEVICE_COMMON: &str = "perDevice.all";

/// 이 키를 export 에 담는가.
pub fn is_exportable(key: &str) -> bool {
    !EXCLUDED_PREFIXES
        .iter()
        .any(|prefix| key == *prefix || key.starts_with(prefix))
}

/// export 파일의 디스크 표현.
///
/// `values` 는 §3.1 봉투의 `values` 와 **같은 것**이다 — 변환하지 않는다. 그래야
/// 손으로 고친 `settings.json` 을 그대로 import 할 수 있고, 두 형식이 갈라져 각자
/// 버그를 갖는 일이 없다.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExportEnvelope {
    pub kind: String,
    #[serde(rename = "formatVersion")]
    pub format_version: u32,
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    /// 사람이 파일을 구분하기 위한 것 — import 판정에 쓰지 않는다.
    #[serde(rename = "exportedAt")]
    pub exported_at: String,
    /// 사람이 파일을 구분하기 위한 것 — import 판정에 쓰지 않는다.
    #[serde(rename = "appVersion")]
    pub app_version: String,
    pub values: BTreeMap<String, serde_json::Value>,
}

/// import 가 거절하는 이유. ⭐ 전부 **저장소를 건드리기 전에** 판정된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportError {
    /// JSON 이 아니거나 봉투 모양이 아니다.
    NotAnExportFile(String),
    /// `kind` 가 우리 것이 아니다 — 엉뚱한 JSON 을 설정으로 삼키지 않는다.
    WrongKind { found: String },
    /// 이 빌드가 모르는 **파일 형식** 버전이다. 설정 스키마와 달리 파일 형식은
    /// 앞으로 어떻게 바뀔지 알 수 없으므로 미래 버전을 추측해 읽지 않는다.
    UnsupportedFormatVersion { found: u32 },
    /// 파일은 읽었으나 저장에 실패했다(디스크 가득 참 등).
    Store(String),
    /// 파일 자체를 읽지 못했다.
    Io(String),
}

impl std::fmt::Display for ImportError {
    // ⛔ 이 문구는 로그로도 흘러가므로 영어다
    // (`localization-and-input-sources.md` §3.1.6). 사용자에게 보여줄 문구는
    // 문자열 카탈로그에서 온다.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ImportError::NotAnExportFile(why) => write!(f, "not an Ultrakey settings export: {why}"),
            ImportError::WrongKind { found } => {
                write!(f, "not an Ultrakey settings export (kind={found})")
            }
            ImportError::UnsupportedFormatVersion { found } => write!(
                f,
                "export format version {found} is newer than this build understands ({EXPORT_FORMAT_VERSION})"
            ),
            ImportError::Store(why) => write!(f, "could not write settings: {why}"),
            ImportError::Io(why) => write!(f, "could not read export file: {why}"),
        }
    }
}

impl std::error::Error for ImportError {}

impl From<StoreError> for ImportError {
    fn from(e: StoreError) -> Self {
        ImportError::Store(e.to_string())
    }
}

/// import 가 실제로 무슨 일을 했는지. ⭐ 사용자에게 **무엇을 들여왔는지** 알리기
/// 위한 것이다 — 조용히 성공하면 "내가 방금 무엇을 바꿨는가"를 알 수 없다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportOutcome {
    /// 교체 후 저장소에 남은 키 개수.
    pub applied_keys: usize,
    /// 교체로 **사라진** 키 개수(파일에 없어서 기본값으로 돌아간 것).
    pub removed_keys: usize,
    /// 파일에 들어 있던, 이 기기에 지금 연결되어 있지 않은 디바이스 식별자들.
    /// ⭐ 버리지 않고 그대로 저장했다 — 그 키보드를 연결하면 적용된다(§3.4 결정 5).
    pub absent_devices: Vec<String>,
    /// 교체 직전에 남긴 백업 경로. 저장 파일이 아직 없었으면 `None`.
    pub backup: Option<PathBuf>,
    /// 파일이 이 빌드보다 과거 스키마라 마이그레이션을 태웠는가.
    pub migrated_from: Option<u32>,
}

/// ⭐ 지금 저장된 것 중 **사용자가 건드린 키만** 담아 봉투를 만든다(§3.4 결정 1·3).
pub fn build_export(
    store: &SettingsStore,
    app_version: &str,
    exported_at: String,
) -> ExportEnvelope {
    let values = store
        .values()
        .iter()
        .filter(|(k, _)| is_exportable(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    ExportEnvelope {
        kind: EXPORT_KIND.to_string(),
        format_version: EXPORT_FORMAT_VERSION,
        schema_version: SCHEMA_VERSION,
        exported_at,
        app_version: app_version.to_string(),
        values,
    }
}

/// 봉투를 사람이 읽을 수 있는 JSON 으로.
pub fn serialize_export(envelope: &ExportEnvelope) -> Result<String, StoreError> {
    let mut json = serde_json::to_string_pretty(envelope).map_err(StoreError::Serialize)?;
    json.push('\n');
    Ok(json)
}

/// 파일 내용을 봉투로 해석한다. ⭐ **저장소를 건드리지 않는다** — 판정만 한다.
pub fn parse_export(raw: &str) -> Result<ExportEnvelope, ImportError> {
    let envelope: ExportEnvelope = serde_json::from_str(raw)
        .map_err(|e| ImportError::NotAnExportFile(e.to_string()))?;

    if envelope.kind != EXPORT_KIND {
        return Err(ImportError::WrongKind {
            found: envelope.kind,
        });
    }
    if envelope.format_version > EXPORT_FORMAT_VERSION {
        return Err(ImportError::UnsupportedFormatVersion {
            found: envelope.format_version,
        });
    }
    Ok(envelope)
}

/// 키에서 디바이스 식별자(`<vid>:<pid>`)를 뽑는다. 디바이스별 키가 아니면 `None`.
fn device_id_of(key: &str) -> Option<&str> {
    let rest = key.strip_prefix(PER_DEVICE_PREFIX)?;
    if key.starts_with(PER_DEVICE_COMMON) || rest.starts_with('_') {
        return None;
    }
    rest.split('.').next().filter(|s| !s.is_empty())
}

/// 봉투에 들어 있는 디바이스 식별자 전부(중복 없이, 정렬됨).
pub fn devices_in(envelope: &ExportEnvelope) -> Vec<String> {
    let mut ids: Vec<String> = envelope
        .values
        .keys()
        .filter_map(|k| device_id_of(k))
        .map(|s| s.to_string())
        .collect();
    ids.sort();
    ids.dedup();
    ids
}

/// ⭐ 봉투를 저장소에 **교체 적용**한다(§3.4 결정 4).
///
/// - 파일에 있는 키 → 파일의 값
/// - 파일에 없는 키 → **삭제**(= 기본값으로 돌아간다)
/// - [`EXCLUDED_PREFIXES`] 에 걸리는 키 → ⭐ **이 기기의 것을 그대로 유지한다.**
///   커널 원장과 창 크기는 import 대상이 아니다
/// - 교체 **직전에** 현재 파일을 백업한다 — 되돌릴 수 있어야 한다
///
/// `present_devices` 는 지금 연결되어 있는 디바이스 식별자들이다. 여기 없는
/// 디바이스의 설정도 **그대로 저장한다** — 버리지 않는다(§3.4 결정 5). 다만
/// 무엇이 잠든 채 들어왔는지를 [`ImportOutcome::absent_devices`] 로 알린다.
pub fn apply_import(
    store: &mut SettingsStore,
    envelope: &ExportEnvelope,
    present_devices: &[String],
    backup_suffix: &str,
) -> Result<ImportOutcome, ImportError> {
    // 1) 되돌릴 수 있게 만든다. ⭐ 교체 **전에** 한다 — 실패하면 아무것도 바꾸지 않는다.
    let backup = store.backup_to(backup_suffix)?;

    // 2) 이 기기의 것으로 남길 키를 먼저 건져 둔다.
    let preserved: BTreeMap<String, serde_json::Value> = store
        .values()
        .iter()
        .filter(|(k, _)| !is_exportable(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    let before: Vec<String> = store.values().keys().cloned().collect();

    // 3) 새 맵을 조립한다 — 파일의 값 + 이 기기에서 보존하는 것.
    //    ⚠️ 파일에 실수로 들어 있는 제외 대상 키는 여기서 버려진다(보존본이 이긴다).
    let mut next: BTreeMap<String, serde_json::Value> = envelope
        .values
        .iter()
        .filter(|(k, _)| is_exportable(k))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    next.extend(preserved);

    let removed_keys = before.iter().filter(|k| !next.contains_key(*k)).count();
    let applied_keys = next.len();

    store.replace_all(next)?;

    let absent_devices = devices_in(envelope)
        .into_iter()
        .filter(|id| !present_devices.iter().any(|p| p == id))
        .collect();

    Ok(ImportOutcome {
        applied_keys,
        removed_keys,
        absent_devices,
        backup,
        migrated_from: (envelope.schema_version < SCHEMA_VERSION).then_some(envelope.schema_version),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn store_with(pairs: &[(&str, serde_json::Value)]) -> (SettingsStore, tempdir::Guard) {
        let guard = tempdir::Guard::new();
        let (mut store, _) = SettingsStore::load(guard.path().join("settings.json"));
        for (k, v) in pairs {
            store.set(k, v).expect("set 실패");
        }
        (store, guard)
    }

    /// 테스트용 임시 디렉터리 — 워크스페이스에 개발 의존성을 더하지 않으려고 직접 만든다.
    mod tempdir {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU64, Ordering};

        static COUNTER: AtomicU64 = AtomicU64::new(0);

        pub struct Guard(PathBuf);

        impl Guard {
            pub fn new() -> Self {
                let n = COUNTER.fetch_add(1, Ordering::Relaxed);
                let pid = std::process::id();
                let dir = std::env::temp_dir().join(format!("ultrakey-transfer-{pid}-{n}"));
                std::fs::create_dir_all(&dir).expect("임시 디렉터리 생성 실패");
                Guard(dir)
            }
            pub fn path(&self) -> &Path {
                &self.0
            }
        }

        impl Drop for Guard {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
    }

    // ── §3.4 결정 1·3 — export 는 "건드린 키만", 기기 상태는 뺀다 ──────────────

    #[test]
    fn export_carries_only_touched_keys() {
        let (store, _g) = store_with(&[
            ("hyperkey.hyper.enabled", json!(true)),
            ("presets.capsWasdArrows", json!(false)),
        ]);
        let env = build_export(&store, "0.1.0", "2026-08-31T00:00:00Z".into());

        // ⭐ 전체 유효값을 펼치지 않는다 — 딱 두 개다.
        assert_eq!(env.values.len(), 2);
        assert_eq!(env.values["hyperkey.hyper.enabled"], json!(true));
        // 기본값과 같은 값이어도 "사용자가 건드렸다"는 사실이므로 담는다.
        assert_eq!(env.values["presets.capsWasdArrows"], json!(false));
    }

    #[test]
    fn export_excludes_the_kernel_ledger_and_window_state() {
        let (store, _g) = store_with(&[
            ("hyperkey.hyper.enabled", json!(true)),
            ("perDevice._managed", json!({"5ac:24f": []})),
            ("ui.lastTab", json!("presets")),
            ("ui.windowWidth", json!(825)),
        ]);
        let env = build_export(&store, "0.1.0", "t".into());

        assert_eq!(env.values.len(), 1, "제외 대상이 빠져야 한다: {:?}", env.values);
        assert!(!env.values.contains_key("perDevice._managed"));
        assert!(!env.values.contains_key("ui.lastTab"));
    }

    #[test]
    fn per_device_user_settings_are_exported_but_the_ledger_is_not() {
        assert!(is_exportable("perDevice.5ac:24f.keyRemap.rows"));
        assert!(is_exportable("perDevice.all.keyRemap.rows"));
        assert!(!is_exportable("perDevice._managed"));
        assert!(!is_exportable("ui.windowHeight"));
    }

    // ── 봉투 형식 ─────────────────────────────────────────────────────────

    #[test]
    fn export_round_trips_through_json() {
        let (store, _g) = store_with(&[("hyperkey.hyper.enabled", json!(true))]);
        let env = build_export(&store, "0.1.0", "2026-08-31T00:00:00Z".into());
        let json_text = serialize_export(&env).unwrap();
        assert_eq!(parse_export(&json_text).unwrap(), env);
    }

    #[test]
    fn parse_rejects_a_json_file_that_is_not_ours() {
        let raw = r#"{"kind":"something.else","formatVersion":1,"schemaVersion":1,
                      "exportedAt":"t","appVersion":"0.1.0","values":{}}"#;
        assert!(matches!(
            parse_export(raw),
            Err(ImportError::WrongKind { .. })
        ));
    }

    #[test]
    fn parse_rejects_arbitrary_json() {
        assert!(matches!(
            parse_export("{\"hello\":1}"),
            Err(ImportError::NotAnExportFile(_))
        ));
        assert!(matches!(
            parse_export("not json at all"),
            Err(ImportError::NotAnExportFile(_))
        ));
    }

    #[test]
    fn parse_rejects_a_future_format_version() {
        let raw = format!(
            r#"{{"kind":"{EXPORT_KIND}","formatVersion":{},"schemaVersion":1,
                 "exportedAt":"t","appVersion":"9.9","values":{{}}}}"#,
            EXPORT_FORMAT_VERSION + 1
        );
        assert!(matches!(
            parse_export(&raw),
            Err(ImportError::UnsupportedFormatVersion { .. })
        ));
    }

    // ── §3.4 결정 4 — import 는 교체다 ─────────────────────────────────────

    #[test]
    fn import_replaces_rather_than_merges() {
        let (mut store, _g) = store_with(&[
            ("presets.capsWasdArrows", json!(true)),
            ("hyperkey.hyper.enabled", json!(true)),
        ]);
        // 파일에는 `presets.capsWasdArrows` 가 **없다** — 내보낸 기기에서는 기본값이었다.
        let env = ExportEnvelope {
            kind: EXPORT_KIND.into(),
            format_version: EXPORT_FORMAT_VERSION,
            schema_version: SCHEMA_VERSION,
            exported_at: "t".into(),
            app_version: "0.1.0".into(),
            values: BTreeMap::from([("hyperkey.meh.enabled".to_string(), json!(true))]),
        };

        let outcome = apply_import(&mut store, &env, &[], "pre-import-test").unwrap();

        // ⭐ 병합이었다면 살아남았을 두 키가 사라져야 한다 — 그것이 "파일이 곧 결과"다.
        assert!(!store.contains("presets.capsWasdArrows"));
        assert!(!store.contains("hyperkey.hyper.enabled"));
        assert_eq!(store.get::<bool>("hyperkey.meh.enabled"), Some(true));
        assert_eq!(outcome.removed_keys, 2);
        assert_eq!(outcome.applied_keys, 1);
    }

    #[test]
    fn import_keeps_this_devices_kernel_ledger_and_window_state() {
        let (mut store, _g) = store_with(&[
            ("perDevice._managed", json!({"5ac:24f": [{"src": 1, "dst": 2}]})),
            ("ui.windowWidth", json!(900)),
            ("hyperkey.hyper.enabled", json!(true)),
        ]);
        let env = ExportEnvelope {
            kind: EXPORT_KIND.into(),
            format_version: EXPORT_FORMAT_VERSION,
            schema_version: SCHEMA_VERSION,
            exported_at: "t".into(),
            app_version: "0.1.0".into(),
            // 다른 기기의 원장이 파일에 섞여 있어도 받지 않아야 한다.
            values: BTreeMap::from([
                ("perDevice._managed".to_string(), json!({"dead:beef": []})),
                ("presets.capsWasdArrows".to_string(), json!(true)),
            ]),
        };

        apply_import(&mut store, &env, &[], "pre-import-test").unwrap();

        // ⭐ 이 기기의 원장이 그대로다 — 남의 원장을 자기 것으로 착각하지 않는다.
        assert_eq!(
            store.get::<serde_json::Value>("perDevice._managed"),
            Some(json!({"5ac:24f": [{"src": 1, "dst": 2}]}))
        );
        assert_eq!(store.get::<u32>("ui.windowWidth"), Some(900));
        assert_eq!(store.get::<bool>("presets.capsWasdArrows"), Some(true));
    }

    #[test]
    fn import_writes_a_backup_before_replacing() {
        let (mut store, _g) = store_with(&[("hyperkey.hyper.enabled", json!(true))]);
        let env = ExportEnvelope {
            kind: EXPORT_KIND.into(),
            format_version: EXPORT_FORMAT_VERSION,
            schema_version: SCHEMA_VERSION,
            exported_at: "t".into(),
            app_version: "0.1.0".into(),
            values: BTreeMap::new(),
        };

        let outcome = apply_import(&mut store, &env, &[], "pre-import-42").unwrap();
        let backup = outcome.backup.expect("백업 경로가 있어야 한다");
        let raw = std::fs::read_to_string(&backup).expect("백업 파일을 읽을 수 있어야 한다");
        // 백업에는 **교체 전** 내용이 들어 있다.
        assert!(raw.contains("hyperkey.hyper.enabled"));
        assert!(store.is_empty(), "교체 후에는 비어 있어야 한다");
    }

    #[test]
    fn import_into_a_fresh_store_has_no_backup() {
        let g = tempdir::Guard::new();
        let (mut store, _) = SettingsStore::load(g.path().join("settings.json"));
        let env = ExportEnvelope {
            kind: EXPORT_KIND.into(),
            format_version: EXPORT_FORMAT_VERSION,
            schema_version: SCHEMA_VERSION,
            exported_at: "t".into(),
            app_version: "0.1.0".into(),
            values: BTreeMap::from([("hyperkey.hyper.enabled".to_string(), json!(true))]),
        };
        let outcome = apply_import(&mut store, &env, &[], "pre-import-test").unwrap();
        assert_eq!(outcome.backup, None, "저장 파일이 없었으면 백업할 것도 없다");
        assert_eq!(store.get::<bool>("hyperkey.hyper.enabled"), Some(true));
    }

    // ── §3.4 결정 5 — 없는 디바이스는 보존하되 비활성 ──────────────────────

    #[test]
    fn absent_device_settings_are_kept_and_reported() {
        let (mut store, _g) = store_with(&[]);
        let env = ExportEnvelope {
            kind: EXPORT_KIND.into(),
            format_version: EXPORT_FORMAT_VERSION,
            schema_version: SCHEMA_VERSION,
            exported_at: "t".into(),
            app_version: "0.1.0".into(),
            values: BTreeMap::from([
                ("perDevice.5ac:24f.keyRemap.rows".to_string(), json!([])),
                ("perDevice.dead:beef.functionKeys.f1".to_string(), json!("Mute")),
                ("perDevice.all.keyRemap.rows".to_string(), json!([])),
            ]),
        };

        let outcome = apply_import(&mut store, &env, &["5ac:24f".to_string()], "b").unwrap();

        // ⭐ 버리지 않는다 — 그 키보드를 연결하면 적용된다.
        assert!(store.contains("perDevice.dead:beef.functionKeys.f1"));
        // 그러나 조용히 넘어가지도 않는다.
        assert_eq!(outcome.absent_devices, vec!["dead:beef".to_string()]);
    }

    #[test]
    fn device_id_extraction_ignores_the_common_layer_and_the_ledger() {
        assert_eq!(device_id_of("perDevice.5ac:24f.keyRemap.rows"), Some("5ac:24f"));
        assert_eq!(device_id_of("perDevice.all.keyRemap.rows"), None);
        assert_eq!(device_id_of("perDevice._managed"), None);
        assert_eq!(device_id_of("hyperkey.hyper.enabled"), None);
    }

    // ── §3.4 결정 7 — 실패는 저장소를 건드리지 않는다 ──────────────────────

    #[test]
    fn a_rejected_file_never_reaches_the_store() {
        let (store, _g) = store_with(&[("hyperkey.hyper.enabled", json!(true))]);
        // parse 단계에서 걸리므로 `apply_import` 에 도달하지 않는다 — 그것이 설계다.
        assert!(parse_export("{\"kind\":\"nope\"}").is_err());
        assert_eq!(store.get::<bool>("hyperkey.hyper.enabled"), Some(true));
    }
}
