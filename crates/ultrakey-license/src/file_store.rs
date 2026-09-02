//! F-12 — 파일 영속 어댑터 (`licensing-and-trial.md` §3.5 개정, 이슈 #99).
//!
//! 개발/자체 서명(미서명 포함) 빌드는 트라이얼 기산점·라이선스 캐시를
//! `~/Library/Application Support/Ultrakey/` 의 JSON 파일에 저장한다. Keychain
//! 접근 자체가 없어 자체 서명 신원 변화가 유발하는 시스템 로그인 프롬프트를
//! 원천 차단한다. 정식 Apple 서명 빌드는 [`crate::store`] 트레이트의 다른
//! 구현체(`ultrakey-platform::keychain::KeychainStore`)를 주입받는다 — 빌드 분기.
//!
//! ## 저장 설계
//! - 파일 2종: `trial.json`(`TrialRecord`) · `license-cache.json`(`LicenseCache`)
//!   — Keychain 의 generic password 2종과 1:1 대응. 직렬화는 Keychain 저장소와
//!   동일한 `serde_json` 바이트라 두 저장소가 바이트 호환된다.
//! - 원자적 쓰기: 같은 디렉터리의 임시 파일(`<이름>.tmp`)에 쓴 뒤 `rename`.
//!   단일 작성자 전제는 F-10 단일 인스턴스 보장(`single_instance`)으로 선다.
//! - 권한: 디렉터리 `0o700` · 파일 `0o600`(unix). 파일에 `license_key` +
//!   `cache_token` 이 담긴다.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use crate::store::{CacheStore, LicenseCache, TrialRecord, TrialStore};

const TRIAL_FILE: &str = "trial.json";
const CACHE_FILE: &str = "license-cache.json";

/// `TrialStore`/`CacheStore` 의 파일 구현 — 경로 주입 가능(테스트는 임시 디렉터리).
///
/// `_guard: Mutex<()>` 는 전 연산을 직렬화한다(`KeychainStore._guard` 와 대칭 —
/// 같은 임시 파일명에 대한 동시 쓰기 방지).
pub struct FileStore {
    dir: PathBuf,
    _guard: Mutex<()>,
}

impl FileStore {
    /// `dir` 을 저장 루트로 쓰는 저장소를 만든다.
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            _guard: Mutex::new(()),
        }
    }

    /// 기본 경로 `$HOME/Library/Application Support/Ultrakey`.
    ///
    /// `HOME` 이 없으면 `None` — 호출자(`LicenseController::boot`)는 인메모리
    /// 저장소로 강등한다.
    pub fn default_dir() -> Option<Self> {
        let home = std::env::var_os("HOME")?;
        let dir = PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("Ultrakey");
        Some(Self::new(dir))
    }

    fn trial_path(&self) -> PathBuf {
        self.dir.join(TRIAL_FILE)
    }

    fn cache_path(&self) -> PathBuf {
        self.dir.join(CACHE_FILE)
    }
}

// ── 내부 헬퍼 ───────────────────────────────────────────────────────────────

/// 직렬화 바이트를 `path` 에 원자적으로 쓴다(임시 파일 + `rename`).
///
/// 실패(디스크 오류 등)는 조용히 무시한다 — `KeychainStore` 가 `SecItemAdd`
/// 실패를 무시하는 것과 대칭. 디렉터리는 첫 쓰기에서 지연 생성한다
/// (상태를 한 번도 쓰지 않은 프로세스는 디렉터리도 만들지 않는다).
fn write_atomic(path: &Path, bytes: &[u8]) {
    let Some(dir) = path.parent() else {
        return;
    };
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    apply_dir_permissions(dir);

    let tmp = tmp_path(path);
    if std::fs::write(&tmp, bytes).is_err() {
        return;
    }
    apply_file_permissions(&tmp);
    if std::fs::rename(&tmp, path).is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
}

/// `path` 의 바이트를 읽는다. 파일 없음·읽기 실패는 `None`.
fn read_bytes(path: &Path) -> Option<Vec<u8>> {
    std::fs::read(path).ok()
}

fn tmp_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_os_string())
        .unwrap_or_default();
    name.push(".tmp");
    path.with_file_name(name)
}

/// 파일 권한 `0o600`(unix) — 실패는 무시한다.
#[cfg(unix)]
fn apply_file_permissions(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn apply_file_permissions(_path: &Path) {}

/// 디렉터리 권한 `0o700`(unix, 멱등) — 이미 존재하는 디렉터리에도 재적용.
#[cfg(unix)]
fn apply_dir_permissions(dir: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
}

#[cfg(not(unix))]
fn apply_dir_permissions(_dir: &Path) {}

// ── 트레이트 구현 ───────────────────────────────────────────────────────────

impl TrialStore for FileStore {
    fn read(&self) -> Option<TrialRecord> {
        let _guard = self._guard.lock().unwrap();
        let bytes = read_bytes(&self.trial_path())?;
        serde_json::from_slice(&bytes).ok()
    }

    fn write(&self, record: TrialRecord) {
        let _guard = self._guard.lock().unwrap();
        if let Ok(json) = serde_json::to_vec(&record) {
            write_atomic(&self.trial_path(), &json);
        }
    }
}

impl CacheStore for FileStore {
    fn read_cache(&self) -> Option<LicenseCache> {
        let _guard = self._guard.lock().unwrap();
        let bytes = read_bytes(&self.cache_path())?;
        serde_json::from_slice(&bytes).ok()
    }

    fn write_cache(&self, cache: &LicenseCache) {
        let _guard = self._guard.lock().unwrap();
        if let Ok(json) = serde_json::to_vec(cache) {
            write_atomic(&self.cache_path(), &json);
        }
    }

    fn clear_cache(&self) {
        let _guard = self._guard.lock().unwrap();
        match std::fs::remove_file(self.cache_path()) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::Timestamp;
    use crate::store::TrialClock;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    /// 병렬 테스트끼리 안 겹치는 고유 임시 디렉터리 — 드롭 시 정리한다.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> Self {
            let n = DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir = std::env::temp_dir().join(format!(
                "ultrakey-filestore-{}-{n}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> PathBuf {
            self.0.clone()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn sample_trial(at: Timestamp) -> TrialRecord {
        TrialRecord {
            clock: TrialClock {
                trial_started_at: at,
                last_seen_at: at,
            },
            trial_consumed: false,
        }
    }

    fn sample_cache(key: &str) -> LicenseCache {
        LicenseCache {
            license_key: key.to_string(),
            device_id: "device-1".to_string(),
            cache_token: "token-1".to_string(),
            cache_issued_at: 1_000,
            activations_used: 1,
            activations_limit: 3,
        }
    }

    #[test]
    fn file_store_trial_roundtrip() {
        let dir = TempDir::new();
        let store = FileStore::new(dir.path());
        assert_eq!(store.read(), None);

        let record = sample_trial(1_700_000_000);
        store.write(record);
        assert_eq!(store.read(), Some(record));
    }

    #[test]
    fn file_store_cache_roundtrip_and_clear() {
        let dir = TempDir::new();
        let store = FileStore::new(dir.path());
        assert_eq!(store.read_cache(), None);

        let cache = sample_cache("KEY-123");
        store.write_cache(&cache);
        assert_eq!(store.read_cache(), Some(cache.clone()));

        store.clear_cache();
        assert_eq!(store.read_cache(), None);
    }

    #[test]
    fn file_store_clear_cache_leaves_trial_intact() {
        let dir = TempDir::new();
        let store = FileStore::new(dir.path());
        let record = sample_trial(1_700_000_100);
        store.write(record);
        store.write_cache(&sample_cache("KEY-123"));

        store.clear_cache();

        assert_eq!(store.read(), Some(record));
        assert_eq!(store.read_cache(), None);
    }

    #[test]
    fn file_store_corrupt_json_reads_none() {
        let dir = TempDir::new();
        std::fs::write(dir.path().join(TRIAL_FILE), b"not-json{{").unwrap();
        let store = FileStore::new(dir.path());
        assert_eq!(store.read(), None);
    }

    #[test]
    fn file_store_atomic_write_leaves_no_tmp_and_writes_final() {
        let dir = TempDir::new();
        let store = FileStore::new(dir.path());
        store.write(sample_trial(1_700_000_200));

        assert!(store.trial_path().exists());
        assert!(!tmp_path(&store.trial_path()).exists());
    }

    #[cfg(unix)]
    #[test]
    fn file_store_permissions_are_user_only() {
        use std::os::unix::fs::PermissionsExt;

        let dir = TempDir::new();
        let dir_path = dir.path();
        let store = FileStore::new(dir_path.clone());
        store.write(sample_trial(1_700_000_300));
        store.write_cache(&sample_cache("KEY-PERM"));

        let dir_mode = std::fs::metadata(&dir_path).unwrap().permissions().mode() & 0o777;
        assert_eq!(dir_mode, 0o700);

        let trial_mode = std::fs::metadata(store.trial_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(trial_mode, 0o600);

        let cache_mode = std::fs::metadata(store.cache_path())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(cache_mode, 0o600);
    }

    #[test]
    fn file_store_default_dir_points_at_application_support() {
        let Some(store) = FileStore::default_dir() else {
            return;
        };
        let path = store.dir.to_string_lossy().to_string();
        assert!(
            path.ends_with("Library/Application Support/Ultrakey"),
            "예상 경로 접미 어긋남: {path}"
        );
    }

    #[test]
    fn file_store_serializes_concurrent_writes() {
        let dir = TempDir::new();
        let store = Arc::new(FileStore::new(dir.path()));
        let mut handles = Vec::new();
        for i in 0..8i64 {
            let store = Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                for j in 0..25i64 {
                    let at = 1_700_000_000 + i * 1_000 + j;
                    store.write(sample_trial(at));
                    store.write_cache(&sample_cache(&format!("KEY-{i}-{j}")));
                    let _ = store.read();
                    let _ = store.read_cache();
                }
            }));
        }
        for h in handles {
            h.join().unwrap();
        }

        let record = store.read().expect("마지막 쓰기가 온전한 기록을 남겨야 한다");
        assert!(record.clock.trial_started_at >= 1_700_000_000);
        assert!(store.read_cache().is_some());
    }
}
