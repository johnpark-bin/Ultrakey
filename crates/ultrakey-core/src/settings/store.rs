//! 설정 저장 계층(F-15 `settings-store-and-integrity.md` §3.1).
//!
//! ⭐ **디스크 모델 = 희소 키-값 맵.** `#[serde(default)]` 구조체를 통째로 직렬화하면
//! 모든 필드가 파일에 쓰여 "부재 = 기본값" 규약이 깨진다(§3.1, §8 수용 기준 1·2). 그래서
//! 디스크 표현은 다음 봉투(envelope)다:
//!
//! ```json
//! { "schemaVersion": 1, "values": { "hyperkey.hyper.enabled": true } }
//! ```
//!
//! `values` 에는 사용자가 실제로 건드린 키만 들어간다. **설정을 한 번도 건드리지
//! 않으면 파일 자체가 생기지 않는다** — [`SettingsStore::load`] 는 파일을 만들지 않고,
//! [`SettingsStore::set`] 이 처음 호출될 때 비로소 만든다.
//!
//! ⭐ **저장 시점 = 컨트롤 단위 즉시 동기 원자적 write-through.** 근거: M1 실측에서
//! **로그아웃 시 graceful shutdown 경로가 실행되지 않았다**(`종료 요청` 로그 0건,
//! `docs/dev/manual-verification.md` "실측 결과" 절). 종료 시점에 무언가를 flush 하는
//! 설계는 애초에 그 시점이 오지 않을 수 있다는 뜻이다. 그래서 [`SettingsStore::set`]
//! 은 호출 안에서 저장을 끝낸다. 기각한 대안:
//! - **디바운스** — 유예 창을 두면, 그 창 안의 로그아웃이 정확히 M1 이 관찰한 유실
//!   시나리오를 재현한다.
//! - **`RunEvent::ExitRequested` 에서 flush** — 그 경로 자체가 실행되지 않음이
//!   실측되었다(위 근거).
//!
//! 원자성은 "같은 디렉터리에 임시 파일 쓰기 → `sync_all` → `rename`"으로 확보한다.
//! `rename` 은 같은 파일시스템 안에서 원자적이므로, 도중에 프로세스가 죽어도 절반만
//! 쓰인 파일이 최종 경로에 나타나지 않는다.
//!
//! ⭐ **"되돌려도 키를 지우지 않는다"(삭제 없는 write-through, §3.1 결정).** 값이
//! 기본값과 같아져도 `values` 에서 키를 제거하지 않는다 — 그렇게 하려면 저장
//! 계층이 각 필드의 기본값을 알아야 하고 매 쓰기마다 비교 연산이 필요한데, 그 복잡도를
//! 감수할 이유가 없다(원본 SuperKey 의 잔여물도 기능적으로 무해했다, §3.1 시나리오 B).

use std::collections::BTreeMap;
use std::fmt;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

/// 스키마 버전. 마이그레이션 판별용(SuperKey 원본의 `lastVersion` 키에 대응, §5 항목 2).
pub const SCHEMA_VERSION: u32 = 1;

/// 저장 계층 오류.
#[derive(Debug)]
pub enum StoreError {
    Io(std::io::Error),
    Serialize(serde_json::Error),
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Io(e) => write!(f, "settings file I/O error: {e}"),
            StoreError::Serialize(e) => write!(f, "settings value serialization error: {e}"),
        }
    }
}

impl std::error::Error for StoreError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StoreError::Io(e) => Some(e),
            StoreError::Serialize(e) => Some(e),
        }
    }
}

impl From<std::io::Error> for StoreError {
    fn from(e: std::io::Error) -> Self {
        StoreError::Io(e)
    }
}

impl From<serde_json::Error> for StoreError {
    fn from(e: serde_json::Error) -> Self {
        StoreError::Serialize(e)
    }
}

/// [`SettingsStore::load`] 가 파일을 읽을 때 실제로 무슨 일이 있었는지. 호출자가
/// 사용자에게 알릴 수 있게(예: "설정 파일이 손상되어 초기화했습니다" 토스트) 남긴다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoadOutcome {
    /// 파일이 아예 없었다 — 전부 기본값(정상 최초 실행).
    Fresh,
    /// 정상적으로 읽었다(스키마 마이그레이션을 거쳤을 수도 있다 — 마이그레이션은
    /// 사용자에게 알릴 실패가 아니므로 별도 variant 를 두지 않는다).
    Loaded { key_count: usize },
    /// 파싱에 실패해 손상 파일을 백업으로 옮기고 빈 상태로 시작했다(§5 항목 1·2).
    Recovered { backup: PathBuf, reason: String },
    /// `schemaVersion` 이 이 빌드가 아는 것보다 미래다 — 값을 버리지 않고 그대로
    /// 읽되 경고한다(예: 사용자가 새 버전으로 한 번 실행했다가 이 버전으로 되돌아옴).
    NewerSchema { found: u32 },
}

/// 봉투(envelope) — 디스크 표현 그 자체. `values` 의 타입이 객체가 아니면(배열·문자열
/// 등) 또는 `schemaVersion`/`values` 필드가 없으면 역직렬화가 그대로 실패한다 — 이것이
/// "봉투가 아니거나 `values` 가 객체가 아님" 케이스를 손상 케이스와 같은 경로로
/// 자연스럽게 합류시킨다(§3.1).
#[derive(Debug, Serialize, Deserialize)]
struct Envelope {
    #[serde(rename = "schemaVersion")]
    schema_version: u32,
    values: BTreeMap<String, serde_json::Value>,
}

/// F-15 §3.1 이 정한 저장 계층. 희소 키-값 맵을 감싸고, "부재 = 기본값" 역직렬화와
/// "값이 바뀔 때만(그리고 즉시 동기적으로) 쓴다"는 직렬화 규칙을 강제한다.
pub struct SettingsStore {
    path: Option<PathBuf>,
    values: BTreeMap<String, serde_json::Value>,
    schema_version: u32,
}

impl SettingsStore {
    /// 주어진 경로에서 읽는다. **파일을 만들지 않는다** — 없으면 그냥 빈 상태로
    /// 시작하고 [`LoadOutcome::Fresh`] 를 돌려준다(§8 수용 기준 1·2).
    pub fn load(path: impl Into<PathBuf>) -> (Self, LoadOutcome) {
        let path = path.into();

        let raw = match std::fs::read_to_string(&path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let store = SettingsStore {
                    path: Some(path),
                    values: BTreeMap::new(),
                    schema_version: SCHEMA_VERSION,
                };
                return (store, LoadOutcome::Fresh);
            }
            Err(e) => {
                // 파일은 있는데(또는 있을 수도 있는데) 읽을 수 없다 — 권한 등. 파싱
                // 실패와 같은 층위로 취급해 손상 취급하고 격리를 시도한다.
                let outcome = quarantine_corrupt_file(&path, format!("설정 파일을 읽을 수 없음: {e}"));
                let store = SettingsStore {
                    path: Some(path),
                    values: BTreeMap::new(),
                    schema_version: SCHEMA_VERSION,
                };
                return (store, outcome);
            }
        };

        let envelope: Envelope = match serde_json::from_str(&raw) {
            Ok(e) => e,
            Err(e) => {
                let outcome = quarantine_corrupt_file(&path, format!("설정 파일 파싱 실패: {e}"));
                let store = SettingsStore {
                    path: Some(path),
                    values: BTreeMap::new(),
                    schema_version: SCHEMA_VERSION,
                };
                return (store, outcome);
            }
        };

        let Envelope {
            schema_version,
            mut values,
        } = envelope;

        if schema_version > SCHEMA_VERSION {
            // 미래 버전이 쓴 파일 — 이해하지 못하는 필드가 섞여 있을 수 있으니
            // 값을 임의로 버리거나 우리 버전으로 낮춰 쓰지 않는다. 그대로 보존한다.
            let store = SettingsStore {
                path: Some(path),
                values,
                schema_version,
            };
            return (store, LoadOutcome::NewerSchema { found: schema_version });
        }

        if schema_version < SCHEMA_VERSION {
            migrate(schema_version, &mut values);
        }

        let key_count = values.len();
        let store = SettingsStore {
            path: Some(path),
            values,
            schema_version: SCHEMA_VERSION,
        };
        (store, LoadOutcome::Loaded { key_count })
    }

    /// 파일을 만들지 않는 메모리 전용 스토어(테스트·영속화 실패 시 폴백용).
    pub fn in_memory() -> Self {
        SettingsStore {
            path: None,
            values: BTreeMap::new(),
            schema_version: SCHEMA_VERSION,
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    /// 저장된 키가 하나도 없는가 — "부재 = 기본값" 수용 기준의 검증 지점.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.values.keys().map(String::as_str)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.values.contains_key(key)
    }

    /// 키가 없거나 타입이 안 맞으면 `None` — 호출자가 그 필드의 기본값을 쓴다.
    /// 타입 불일치는 값 하나가 망가졌다고 스토어 전체를 버릴 이유가 없으므로
    /// `tracing::warn!` 으로 남기고 해당 값만 `None` 취급한다.
    pub fn get<T: serde::de::DeserializeOwned>(&self, key: &str) -> Option<T> {
        let raw = self.values.get(key)?;
        match serde_json::from_value(raw.clone()) {
            Ok(value) => Some(value),
            Err(err) => {
                tracing::warn!(
                    key,
                    error = %err,
                    "setting value type didn't match what was expected; falling back to default for this value only"
                );
                None
            }
        }
    }

    /// ⭐ 즉시 동기 원자적 저장. 실패해도 **메모리 값은 이미 갱신되어 있다** — 저장
    /// 실패가 엔진 반영 실패로 번지지 않게 하기 위함이다(§3.7: 엔진 반영은 성공시키고
    /// 저장 실패만 알린다).
    pub fn set<T: Serialize>(&mut self, key: &str, value: &T) -> Result<(), StoreError> {
        let json = serde_json::to_value(value)?;
        self.values.insert(key.to_string(), json);
        self.persist()
    }

    /// 키 하나를 지운다. "부재 = 기본값" 규약에서 **부재로 되돌리는** 유일한 정식
    /// 경로다(F-15 §3.4 결정 4 의 import 가 이것을 쓴다).
    ///
    /// ⚠️ 일반 설정 변경은 여전히 [`set`](Self::set) 이다 — §3.1 의 "되돌려도 키를
    /// 지우지 않는다"(삭제 없는 write-through)는 그대로다. 이 함수는 사용자가
    /// **명시적으로 "따름/기본값으로 되돌린다"고 말한 경우**(디바이스별 설정의
    /// `--- (공통 설정을 따름)`, 언어의 `System`, import 의 교체)에만 쓴다.
    pub fn remove(&mut self, key: &str) -> Result<(), StoreError> {
        if self.values.remove(key).is_none() {
            // 없는 키를 지우는 것은 목표 상태에 이미 도달한 것 — 파일을 건드리지
            // 않는다. 그래야 "설정을 건드리지 않으면 파일이 생기지 않는다"(§8
            // 수용 기준 1·2)가 unset 경로에서도 깨지지 않는다.
            return Ok(());
        }
        self.persist()
    }

    /// 저장된 희소 맵 전체를 읽는다 — ⭐ **export 가 담는 것이 정확히 이것이다**
    /// (F-15 §3.4 결정 1: "건드린 키만"). 전체 유효값을 펼치지 않는 이유가 여기
    /// 있다 — 펼치면 import 한 기기에서 "부재 = 기본값" 규약이 영구히 깨진다.
    pub fn values(&self) -> &BTreeMap<String, serde_json::Value> {
        &self.values
    }

    /// ⭐ 희소 맵 전체를 **교체**한다(F-15 §3.4 결정 4: import 는 병합이 아니라 교체).
    ///
    /// 파일에 없던 키는 사라져 기본값으로 돌아간다 — 그것이 export 한 기기의
    /// 상태와 같아지는 유일한 방법이다. 병합이면 "끈 설정이 되살아나지 않는" 조용한
    /// 실패가 생긴다.
    ///
    /// 쓰기는 [`set`](Self::set) 과 같은 원자적 절차를 탄다.
    pub fn replace_all(
        &mut self,
        values: BTreeMap<String, serde_json::Value>,
    ) -> Result<(), StoreError> {
        self.values = values;
        self.persist()
    }

    /// 현재 파일을 같은 디렉터리에 `<파일명>.<suffix>` 로 복사한다. ⭐ import 가
    /// 교체 **직전에** 부른다 — 교체는 되돌리기 어려운 조작이고 백업 한 번의 비용은
    /// 거의 없다(F-15 §3.4 결정 4).
    ///
    /// 저장 파일이 아직 없으면(= 사용자가 아무것도 건드리지 않았다) 백업할 것도
    /// 없으므로 `Ok(None)` 이다.
    pub fn backup_to(&self, suffix: &str) -> Result<Option<PathBuf>, StoreError> {
        let Some(path) = &self.path else {
            return Ok(None);
        };
        if !path.exists() {
            return Ok(None);
        }
        let mut backup = path.as_os_str().to_owned();
        backup.push(".");
        backup.push(suffix);
        let backup = PathBuf::from(backup);
        std::fs::copy(path, &backup)?;
        Ok(Some(backup))
    }

    /// 원자적 쓰기: 임시 파일 → `sync_all` → `rename`. `in_memory()` 스토어(`path ==
    /// None`)는 애초에 저장할 곳이 없으므로 아무 것도 하지 않는다.
    fn persist(&self) -> Result<(), StoreError> {
        let Some(path) = &self.path else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let envelope = Envelope {
            schema_version: self.schema_version,
            values: self.values.clone(),
        };
        let json = serde_json::to_vec_pretty(&envelope)?;

        let tmp_path = tmp_path_for(path);
        {
            let mut f = File::create(&tmp_path)?;
            f.write_all(&json)?;
            // rename 전에 디스크에 실제로 도달했음을 보장한다 — 그래야 rename 직후
            // 프로세스가 죽어도 최종 경로에는 "전부 쓰였거나(옛 내용) 전부 쓰였거나
            // (새 내용)"만 남는다.
            f.sync_all()?;
        }
        std::fs::rename(&tmp_path, path)?;

        Ok(())
    }
}

/// `schemaVersion` 이 지금 이 빌드(`SCHEMA_VERSION`)보다 과거인 파일을 읽었을 때
/// 호출된다. 지금은 v1 하나뿐이라 할 일이 없다.
///
/// **필드 추가는 여기 들어오지 않는다** — "부재 = 기본값" 규약이 이미 자동으로
/// 해결한다(옛 파일에 없는 키는 그냥 그 필드의 기본값으로 읽힌다). 이 함수가 자리를
/// 잡아 두는 것은 규약만으로 해결되지 않는 두 경우 — **키 이름 변경**과 **키 삭제**
/// — 를 위해서다(§5 항목 2). 새 스키마 버전이 생기면 여기 `from` 값별 match 팔을
/// 추가한다.
// clippy::match_single_binding — 지금은 팔이 `_` 하나뿐이라 `if` 로 접어도 되지만,
// 다음 스키마 버전이 생기는 순간 `from` 값별 분기가 늘어날 자리라는 것을 코드
// 형태로 남겨 두기 위해 일부러 match 로 둔다.
#[allow(clippy::match_single_binding)]
fn migrate(from: u32, values: &mut BTreeMap<String, serde_json::Value>) {
    match from {
        // 예시(아직 실재하지 않음): 0 => { rename_key(values, "old.key", "new.key"); }
        _ => {
            let _ = values; // 현재는 v1 하나뿐이라 변형할 것이 없다.
        }
    }
}

/// 손상된(또는 읽을 수 없는) 설정 파일을 같은 디렉터리의 백업 이름으로 옮긴다.
/// rename 마저 실패하면 백업 없이 원래 위치에 남기고, 그 사실을 `reason` 에 적는다.
fn quarantine_corrupt_file(path: &Path, reason: String) -> LoadOutcome {
    let backup_path = corrupt_backup_path(path);
    match std::fs::rename(path, &backup_path) {
        Ok(()) => LoadOutcome::Recovered {
            backup: backup_path,
            reason,
        },
        Err(rename_err) => LoadOutcome::Recovered {
            // 옮기지 못했으니 원래 경로가 곧 "지금 손상 내용이 남아 있는 곳"이다.
            backup: path.to_path_buf(),
            reason: format!("{reason} (백업 이동 실패: {rename_err} — 손상 파일이 원래 위치에 그대로 남음)"),
        },
    }
}

fn corrupt_backup_path(path: &Path) -> PathBuf {
    let epoch_secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut backup = path.as_os_str().to_owned();
    backup.push(format!(".corrupt-{epoch_secs}"));
    PathBuf::from(backup)
}

fn tmp_path_for(path: &Path) -> PathBuf {
    let mut tmp = path.as_os_str().to_owned();
    tmp.push(".tmp");
    PathBuf::from(tmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `remove` 는 "부재 = 기본값"으로 되돌리는 정식 경로다(F-15 §3.4).
    #[test]
    fn remove_deletes_the_key_and_persists() {
        let dir = std::env::temp_dir().join(format!("ultrakey-store-remove-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        let _ = std::fs::remove_file(&path);

        let (mut store, _) = SettingsStore::load(&path);
        store.set("a.b", &true).unwrap();
        store.set("c.d", &1u32).unwrap();
        store.remove("a.b").unwrap();

        assert!(!store.contains("a.b"));
        assert!(store.contains("c.d"));

        // 디스크에도 반영됐는가 — 다시 읽어서 확인한다.
        let (reloaded, _) = SettingsStore::load(&path);
        assert!(!reloaded.contains("a.b"));
        assert!(reloaded.contains("c.d"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// 없는 키를 지우는 것은 목표 상태에 이미 도달한 것 — 파일을 만들지 않는다.
    /// 그래야 §8 수용 기준 1·2("건드리지 않으면 파일이 생기지 않는다")가 깨지지 않는다.
    #[test]
    fn removing_an_absent_key_does_not_create_the_file() {
        let dir = std::env::temp_dir().join(format!("ultrakey-store-noremove-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let path = dir.join("settings.json");
        let _ = std::fs::remove_file(&path);

        let (mut store, _) = SettingsStore::load(&path);
        store.remove("never.set").unwrap();
        assert!(!path.exists(), "파일이 생기면 안 된다");

        let _ = std::fs::remove_dir_all(&dir);
    }
    use std::sync::atomic::{AtomicU64, Ordering};

    /// 병렬 테스트 실행에서도 서로 다른 임시 디렉터리를 쓰게 하는 카운터.
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    /// 테스트 전용 임시 디렉터리 — 끝나면(패닉 포함) `Drop` 에서 지운다.
    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let n = COUNTER.fetch_add(1, Ordering::Relaxed);
            let dir = std::env::temp_dir().join(format!(
                "ultrakey-settings-store-test-{label}-{}-{n}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("임시 디렉터리 생성 실패");
            TestDir(dir)
        }

        fn settings_path(&self) -> PathBuf {
            self.0.join("settings.json")
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    // 1. 새 경로 load() → Fresh, is_empty(), 파일이 생기지 않았다(§8 첫 두 항목).
    #[test]
    fn load_on_missing_path_is_fresh_and_creates_no_file() {
        let dir = TestDir::new("fresh");
        let path = dir.settings_path();

        let (store, outcome) = SettingsStore::load(&path);

        assert_eq!(outcome, LoadOutcome::Fresh);
        assert!(store.is_empty());
        assert!(!path.exists(), "load() 가 파일을 만들면 안 된다");
    }

    // 2. set() 한 번 → 파일 생성, values 에 정확히 그 키 하나만 있다.
    #[test]
    fn set_once_creates_file_with_exactly_one_key() {
        let dir = TestDir::new("set-once");
        let path = dir.settings_path();
        let (mut store, _) = SettingsStore::load(&path);

        store.set("hyperkey.hyper.enabled", &true).unwrap();

        assert!(path.exists());
        assert_eq!(store.keys().collect::<Vec<_>>(), vec!["hyperkey.hyper.enabled"]);

        let raw = std::fs::read_to_string(&path).unwrap();
        let envelope: Envelope = serde_json::from_str(&raw).unwrap();
        assert_eq!(envelope.values.len(), 1);
        assert!(envelope.values.contains_key("hyperkey.hyper.enabled"));
    }

    // 3. 기본값과 같은 값을 다시 set() 해도 키가 지워지지 않는다(§8, D-C).
    #[test]
    fn set_with_default_equivalent_value_does_not_remove_key() {
        let dir = TestDir::new("no-delete");
        let path = dir.settings_path();
        let (mut store, _) = SettingsStore::load(&path);

        store.set("hyperkey.hyper.enabled", &true).unwrap();
        store.set("hyperkey.hyper.enabled", &false).unwrap(); // "되돌림" — 기본값과 동등

        assert!(store.contains("hyperkey.hyper.enabled"));
        assert_eq!(store.get::<bool>("hyperkey.hyper.enabled"), Some(false));
    }

    // 4. 저장 → 새 SettingsStore::load() → 같은 값이 나온다(왕복).
    #[test]
    fn save_then_reload_round_trips() {
        let dir = TestDir::new("round-trip");
        let path = dir.settings_path();

        {
            let (mut store, _) = SettingsStore::load(&path);
            store.set("hyperkey.hyper.enabled", &true).unwrap();
            store.set("ui.lastTab", &"hyperkey".to_string()).unwrap();
        }

        let (store2, outcome) = SettingsStore::load(&path);
        assert_eq!(outcome, LoadOutcome::Loaded { key_count: 2 });
        assert_eq!(store2.get::<bool>("hyperkey.hyper.enabled"), Some(true));
        assert_eq!(
            store2.get::<String>("ui.lastTab"),
            Some("hyperkey".to_string())
        );
    }

    // 5. 손상 JSON 파일 → Recovered, 백업 파일이 실제로 존재하고 원문이 보존됨,
    //    스토어는 비어 있음.
    #[test]
    fn corrupt_json_is_quarantined_and_store_starts_empty() {
        let dir = TestDir::new("corrupt");
        let path = dir.settings_path();
        std::fs::write(&path, b"{ not valid json").unwrap();

        let (store, outcome) = SettingsStore::load(&path);

        let LoadOutcome::Recovered { backup, reason } = outcome else {
            panic!("Recovered 를 기대했다");
        };
        assert!(!reason.is_empty());
        assert!(backup.exists(), "백업 파일이 존재해야 한다: {backup:?}");
        assert_eq!(
            std::fs::read(&backup).unwrap(),
            b"{ not valid json",
            "백업에는 손상된 원문이 그대로 보존돼야 한다"
        );
        assert!(!path.exists(), "원래 경로는 백업으로 옮겨져 비어 있어야 한다");
        assert!(store.is_empty());
    }

    // 6. schemaVersion: 999 → NewerSchema, 값은 유지된다.
    #[test]
    fn newer_schema_version_preserves_values() {
        let dir = TestDir::new("newer-schema");
        let path = dir.settings_path();
        std::fs::write(
            &path,
            r#"{"schemaVersion":999,"values":{"hyperkey.hyper.enabled":true}}"#,
        )
        .unwrap();

        let (store, outcome) = SettingsStore::load(&path);

        assert_eq!(outcome, LoadOutcome::NewerSchema { found: 999 });
        assert_eq!(store.get::<bool>("hyperkey.hyper.enabled"), Some(true));
    }

    // 7. 타입 불일치("hyperkey.hyper.enabled": "yes") → get::<bool>() 이 None,
    //    다른 키는 멀쩡히 읽힌다.
    #[test]
    fn type_mismatch_yields_none_without_poisoning_other_keys() {
        let dir = TestDir::new("type-mismatch");
        let path = dir.settings_path();
        std::fs::write(
            &path,
            r#"{"schemaVersion":1,"values":{"hyperkey.hyper.enabled":"yes","ui.lastTab":"seek"}}"#,
        )
        .unwrap();

        let (store, _) = SettingsStore::load(&path);

        assert_eq!(store.get::<bool>("hyperkey.hyper.enabled"), None);
        assert_eq!(store.get::<String>("ui.lastTab"), Some("seek".to_string()));
    }

    // 8은 ultrakey-hyperkey 크레이트(HyperkeySettings::from_store) 소관 — 여기서는
    // 스토어 자체의 성질만 검증한다.

    // 9. migrate() 는 지금 v1 하나뿐이라 아무것도 바꾸지 않는다(자리만 잡아 둠).
    #[test]
    fn migrate_from_older_schema_is_currently_a_no_op() {
        let dir = TestDir::new("migrate-noop");
        let path = dir.settings_path();
        std::fs::write(
            &path,
            r#"{"schemaVersion":0,"values":{"hyperkey.hyper.enabled":true}}"#,
        )
        .unwrap();

        let (store, outcome) = SettingsStore::load(&path);

        assert_eq!(outcome, LoadOutcome::Loaded { key_count: 1 });
        assert_eq!(store.get::<bool>("hyperkey.hyper.enabled"), Some(true));
    }

    // 10 은 ultrakey-core::settings::keys 모듈 자체 테스트(keys.rs)에서 검증한다.

    // 11. 원자성 증거: set() 후 디렉터리에 .tmp 잔여 파일이 없다.
    #[test]
    fn set_leaves_no_tmp_file_behind() {
        let dir = TestDir::new("no-tmp-residue");
        let path = dir.settings_path();
        let (mut store, _) = SettingsStore::load(&path);

        store.set("hyperkey.hyper.enabled", &true).unwrap();

        let leftover: Vec<_> = std::fs::read_dir(&dir.0)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().ends_with(".tmp"))
            .collect();
        assert!(leftover.is_empty(), ".tmp 잔여 파일이 남았다: {leftover:?}");
    }

    // 부가: in_memory() 스토어는 set() 을 해도 디스크에 아무것도 쓰지 않는다.
    #[test]
    fn in_memory_store_never_touches_disk() {
        let mut store = SettingsStore::in_memory();
        assert_eq!(store.path(), None);
        store.set("ui.lastTab", &"hyperkey".to_string()).unwrap();
        assert_eq!(store.get::<String>("ui.lastTab"), Some("hyperkey".to_string()));
    }
}
