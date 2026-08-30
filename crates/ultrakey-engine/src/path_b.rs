//! 경로 B(IOHID 커널 매핑) 관리 — F-17(`docs/spec/per-device-settings.md`) 디바이스별
//! 재설계.
//!
//! ⭐ **이 파일은 M2(D-1 전역 caps lock 정규화)를 완전히 갈아엎은 것이다.** M1/M2 는
//! "경로 B 배열 전체 = D-1 매핑 하나"였고, 매칭 없는 `--set` 으로 **전역**에 썼다.
//! F-17 은 그 전제를 깨야 한다 — 스파이크 S-6(실측, 두 번 재현) 이 확정한 대로
//! 매칭 없는 `--set` 은 디바이스별 배열까지 포함해 전부 갈아치운다. 그래서:
//!
//! 1. **모든 쓰기가 디바이스 한정이다.** D-1 도 예외가 아니다 — 붙어 있는 *모든*
//!    키보드에 개별로 설치한다(`docs/dev/architecture.md` §7.1, D-17-1). 경로 A
//!    (`CGEventTap`)가 세션 전체의 병합 스트림을 받아 어느 키보드가 caps lock 을
//!    보냈는지 구분하지 못하기 때문이다.
//! 2. **한 디바이스의 최종 배열 = D-1 + 기능1 + 기능2 를 합성한 하나**
//!    (`ultrakey_core::perdevice::compose`, §3.6 규칙 4·5)다. `--matching {vid,pid}
//!    --set` 한 번으로 쓴다.
//! 3. **소유권은 디바이스별 원장으로 판정한다**(D-17-2) — "우리가 마지막으로 이
//!    디바이스에 실제로 쓴 배열"을 기억해 두고, 그것과의 차집합으로 "남의 매핑"을
//!    가려낸다. 크래시 안전을 위해 쓰기 전(상위집합)·쓰기 후(정확집합) 2단계로
//!    영속화한다([`PathBManager::write_device`] 문서 참고).
//! 4. **구버전이 남긴 전역 D-1 잔재는 딱 한 번 이관한다**(D-17-5) —
//!    [`GlobalD1Migration::migrate_clear_global_d1`] 가 그 유일한 매칭 없는 `--set`
//!    이고, 기능 쓰기 경로([`HidMappingBackend`])에서는 애초에 호출할 수 없다.
//!
//! 원장·전역 마이그레이션 양쪽 모두 이 크레이트가 저장소 구현을 모른다 —
//! [`LedgerStore`]/[`GlobalD1Migration`] 트레이트로 앱 계층에 위임한다
//! (CONTRACT.md 부록 B.2·B.3).

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use ultrakey_core::keycode::{KeyCode, SourceKey};
use ultrakey_core::perdevice::{compose, validate, DeviceId, ManagedLedger, PerDeviceSettings};
use ultrakey_core::settings::EngineConfig;
use ultrakey_platform::hid_mapping::{
    DeviceInfo, HidMappingBackend, HidMappingError, HidutilBackend, KeyMapping,
};

// ── D-1 — caps lock 모멘터리 정규화 매핑 ─────────────────────────────────────

/// ⭐ D-1 이 설치하는 **바로 그** 매핑(`docs/dev/architecture.md` §6.1) — caps lock
/// (`0x700000039`, 실측) → F18(`0x70000006D`, 실측). 전역 잔재 이관(D-17-5)의 서명이기도
/// 하다 — [`GlobalD1Migration::migrate_clear_global_d1`] 이 이 값과 정확히 일치하는
/// 항목만 전역 배열에서 제거한다.
pub fn d1_mapping() -> KeyMapping {
    KeyMapping {
        src: SourceKey::CapsLock
            .hid_usage()
            .expect("SourceKey::hid_usage() 는 35종 전부 Some 이다(keycode.rs)"),
        dst: SourceKey::F18
            .hid_usage()
            .expect("SourceKey::hid_usage() 는 35종 전부 Some 이다(keycode.rs)"),
    }
}

/// `cfg.caps_lock_alias` 로부터 이번 재조정에서 **모든 붙어 있는 키보드에** 설치해야
/// 할 D-1 매핑을 계산한다(D-17-1 — D-1 은 디바이스 한정이지만 설치 대상은 전체다).
/// alias 가 없으면(caps lock 에 의존하는 프리셋이 하나도 없거나 `Synthesize Caps Lock
/// Remap` 이 켜져 있음) `None`.
pub fn d1_for(cfg: &EngineConfig) -> Option<KeyMapping> {
    match cfg.caps_lock_alias {
        Some(alias) if alias == KeyCode::F18 => Some(d1_mapping()),
        Some(_) => {
            // D-1 은 F18 로 고정돼 있다(architecture.md §6.1) — 다른 값이 들어오면
            // 설계 위반이다. 지어낸 매핑을 만들지 않고 방어적으로 None 을 돌려준다.
            tracing::error!(
                "caps_lock_alias 가 F18 이 아니다 — D-1 설계 위반, 경로 B 매핑을 만들지 않는다"
            );
            None
        }
        None => None,
    }
}

// ── B.2 — 원장 접근 트레이트(CONTRACT.md 부록 B.2) ──────────────────────────

/// 디바이스별 원장(`perDevice._managed`, D-17-2) 접근 — 이 크레이트는 저장소 구현을
/// 모른다. 앱이 `SettingsStore` 위에 구현해 영속화한다.
pub trait LedgerStore: Send + Sync {
    fn load(&self) -> ManagedLedger;
    fn store(&self, ledger: &ManagedLedger) -> Result<(), String>;
}

/// ⚠️ **테스트·기본값 전용.** 항상 빈 원장을 주고, 저장은 아무 일도 하지 않는
/// no-op 이다. 이 타입을 그대로 쓰면 원장이 프로세스 재시작 사이에 전혀 영속화되지
/// 않아 D-17-2 의 크래시 안전(2단계 영속화)이 전혀 성립하지 않는다 — **앱은 반드시
/// `perDevice._managed` 위에 진짜 구현을 넘겨야 한다.**
#[derive(Debug, Default)]
pub struct NullLedgerStore;

impl LedgerStore for NullLedgerStore {
    fn load(&self) -> ManagedLedger {
        ManagedLedger::new()
    }

    fn store(&self, _ledger: &ManagedLedger) -> Result<(), String> {
        Ok(())
    }
}

// ── B.3 — 전역 D-1 마이그레이션 트레이트(CONTRACT.md 부록 B.3, D-17-5) ──────

/// 구버전이 남긴 **전역** D-1 잔재를 1회 이관한다. `HidMappingBackend` 와 별도
/// 트레이트로 두는 이유: 기능 쓰기 경로가 이 함수에 실수로도 닿을 수 없게 하기
/// 위해서다 — 이것이 이 코드베이스에서 유일하게 허용된 매칭 없는 `--set` 이다.
pub trait GlobalD1Migration: Send + Sync {
    fn migrate_clear_global_d1(&self, d1: KeyMapping) -> Result<bool, HidMappingError>;
}

/// `HidutilBackend::migrate_clear_global_d1` 을 트레이트로 감싸는 얇은 래퍼.
///
/// ⚠️ 고아 규칙(orphan rule) 때문에 `ultrakey-platform` 의 구체 타입
/// (`HidutilBackend`)에 이 크레이트가 정의한 트레이트([`GlobalD1Migration`])를 직접
/// impl 할 수 없다 — 그래서 이 크레이트 안에 새 타입을 두고 그 안에서 위임한다
/// (CONTRACT.md 부록 B.3).
#[derive(Debug, Default)]
pub struct HidutilGlobalMigration;

impl GlobalD1Migration for HidutilGlobalMigration {
    fn migrate_clear_global_d1(&self, d1: KeyMapping) -> Result<bool, HidMappingError> {
        HidutilBackend.migrate_clear_global_d1(d1)
    }
}

// ── 시작 시 재조정 결과 ──────────────────────────────────────────────────────

/// 시작 시 재조정([`PathBManager::reconcile_on_start`]) 결과.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReconcileReport {
    /// D-17-5 — 전역 D-1 잔재 이관이 실제로 무언가를 지웠는가.
    pub global_migration_cleared: bool,
    /// 이번 호출에서 배열을 다시 쓴(빈 배열 포함) 디바이스 수.
    pub devices_reconciled: usize,
}

// ── B.4 — PathBManager ──────────────────────────────────────────────────────

/// 경로 B 디바이스별 합성·쓰기·정리를 총괄한다. 저장소·마이그레이션 구현을 모르고
/// 트레이트 객체로만 다룬다(CONTRACT.md 부록 B.4).
pub struct PathBManager {
    backend: Box<dyn HidMappingBackend>,
    migration: Option<Box<dyn GlobalD1Migration>>,
    ledger: Box<dyn LedgerStore>,
    /// D-17-5 — 전역 마이그레이션은 이 매니저의 수명 동안 **정확히 한 번**만
    /// 시도한다(`reconcile_on_start` 가 여러 번 불려도 재시도하지 않는다).
    migrated: AtomicBool,
}

impl PathBManager {
    pub fn new(
        backend: Box<dyn HidMappingBackend>,
        migration: Option<Box<dyn GlobalD1Migration>>,
        ledger: Box<dyn LedgerStore>,
    ) -> Self {
        PathBManager {
            backend,
            migration,
            ledger,
            migrated: AtomicBool::new(false),
        }
    }

    /// 기동 시 재조정 — ① 전역 D-1 잔재 1회 이관 → ② 붙어 있는 디바이스 전부 재계산·쓰기.
    /// ⭐ 순서가 안전성을 보장한다(D-17-5) — ①이 어떤 per-device 쓰기보다 먼저
    /// 일어나야, ②가 다시 계산해 쓰는 디바이스별 배열이 전역 잔재에 오염되지 않는다.
    pub fn reconcile_on_start(
        &self,
        cfg: &EngineConfig,
        attached: &[DeviceInfo],
    ) -> Result<ReconcileReport, HidMappingError> {
        let global_migration_cleared = self.run_global_migration_once();
        let devices_reconciled = self.apply_all_inner(cfg, attached)?;
        Ok(ReconcileReport {
            global_migration_cleared,
            devices_reconciled,
        })
    }

    /// 설정 변경·전체 재적용. `attached` 에 있는 디바이스에만 쓴다(B.4.3 — 뽑혀 있는
    /// 디바이스는 건드리지 않는다).
    pub fn apply_all(&self, cfg: &EngineConfig, attached: &[DeviceInfo]) -> Result<(), HidMappingError> {
        self.apply_all_inner(cfg, attached)?;
        Ok(())
    }

    fn apply_all_inner(&self, cfg: &EngineConfig, attached: &[DeviceInfo]) -> Result<usize, HidMappingError> {
        let settings = PerDeviceSettings::new(&cfg.per_device_values);
        let d1 = d1_for(cfg);
        let mut count = 0usize;
        for info in attached {
            let device = DeviceId::new(info.vendor_id, info.product_id);
            let composition = compose(&device, &settings, d1);
            self.write_device(&device, &composition.mappings)?;
            count += 1;
        }
        Ok(count)
    }

    /// 핫플러그 1대 재적용. `device` 가 방금 연결됐다는 전제(호출자, §3.6 규칙 8)로
    /// `attached` 목록 없이 바로 계산·쓴다.
    pub fn apply_device(&self, cfg: &EngineConfig, device: &DeviceId) -> Result<(), HidMappingError> {
        let settings = PerDeviceSettings::new(&cfg.per_device_values);
        let d1 = d1_for(cfg);
        let composition = compose(device, &settings, d1);
        self.write_device(device, &composition.mappings)
    }

    /// 정상 종료 — 원장의 디바이스 중 **붙어 있는 것**만 우리 것을 걷어내고 원장에서
    /// 제거한다. 뽑혀 있는 디바이스는 그때 정리할 수 없으므로 원장에 그대로 남긴다
    /// (§3.6 규칙 6) — 다음에 그 디바이스가 붙을 때(핫플러그 `Attached` →
    /// `apply_device`) 정리된다.
    pub fn cleanup(&self, attached: &[DeviceInfo]) -> Result<(), HidMappingError> {
        let attached_ids: HashSet<(u32, u32)> = attached
            .iter()
            .map(|d| (d.vendor_id, d.product_id))
            .collect();
        let ledger = self.ledger.load();
        for device in ledger.keys() {
            let m = device.to_match();
            if attached_ids.contains(&(m.vendor_id, m.product_id)) {
                self.write_device(device, &[])?;
            }
        }
        Ok(())
    }

    /// D-17-5 — 정확히 한 번만 시도한다. 이미 시도했으면(성공이든 실패든) 아무 것도
    /// 하지 않고 `false` 를 돌려준다. 마이그레이션이 등록돼 있지 않으면(테스트 등)
    /// 마찬가지로 `false`.
    fn run_global_migration_once(&self) -> bool {
        if self.migrated.swap(true, Ordering::SeqCst) {
            return false;
        }
        let Some(migration) = self.migration.as_ref() else {
            return false;
        };
        match migration.migrate_clear_global_d1(d1_mapping()) {
            Ok(cleared) => {
                if cleared {
                    tracing::info!(
                        "전역 D-1 잔재(구버전이 매칭 없이 설치한 caps lock → F18)를 \
                         발견해 제거했다(D-17-5)"
                    );
                } else {
                    tracing::debug!("전역 D-1 잔재가 없다 — 이관할 것이 없다");
                }
                cleared
            }
            Err(e) => {
                tracing::warn!(error = %e, "전역 D-1 잔재 이관 시도가 실패했다 — 계속 진행한다");
                false
            }
        }
    }

    /// B.4.1 — 크래시 안전 2단계 영속화. 한 디바이스의 최종 배열(`composed`, 이미
    /// `compose()` 로 합성·중재까지 끝난 값)을 실제로 쓴다.
    ///
    /// ```text
    /// ① claimed = ledger[dev] ∪ composed ;  ledger[dev] = claimed ; ledger.store()  (쓰기 전 상위집합)
    /// ② read = backend.read_current(dev) ; foreign = read.aggregated − claimed
    ///    foreign 에서 composed 와 src 가 겹치는 항목 제거(D-17-6, 경고 로그)
    ///    backend.apply(dev, foreign ++ composed)                                    (커널, 단일 --set)
    /// ③ ledger[dev] = composed(비었으면 제거) ; ledger.store()                       (쓰기 후 정확집합)
    /// ```
    /// ①이 있어 어느 시점에 죽어도 원장은 커널에 있을 수 있는 우리 것의 상위집합이다
    /// (이슈 #19 증상 B 류의 영구 누수를 막는다). ③이 있어 정상 경로에서는 원장이
    /// 정확해 남의 매핑을 잘못 지우지 않는다(PR #23 보존).
    fn write_device(&self, device: &DeviceId, composed: &[KeyMapping]) -> Result<(), HidMappingError> {
        // ① 쓰기 전 — 상위집합을 먼저 원장에 기록한다.
        let mut ledger = self.ledger.load();
        let prev = ledger.get(device).cloned().unwrap_or_default();
        let mut claimed = prev;
        for m in composed {
            if !claimed.contains(m) {
                claimed.push(*m);
            }
        }
        ledger.insert(device.clone(), claimed.clone());
        if let Err(e) = self.ledger.store(&ledger) {
            tracing::warn!(
                error = %e,
                device = device.as_str(),
                "원장 저장 실패(쓰기 전 상위집합) — 계속 진행한다"
            );
        }

        // ② 커널 — 지금 실제로 걸려 있는 것 중 우리가 방금 소유권을 주장한 것
        // (claimed)에 없는 항목이 진짜 남의 매핑이다.
        let read = self.backend.read_current(&device.to_match())?;
        let claimed_set: HashSet<KeyMapping> = claimed.into_iter().collect();
        let mut foreign: Vec<KeyMapping> = read
            .aggregated
            .into_iter()
            .filter(|m| !claimed_set.contains(m))
            .collect();

        // B.4.2 / D-17-6 — 남의 매핑과 우리 설정의 소스 키가 겹치면 우리가 이긴다.
        // 사용자가 방금 UI 에서 그 키를 명시적으로 지정했다 — 더 최근이고 더
        // 명시적인 의도가 이긴다. 원장에는 넣지 않는다(우리 것으로 소유하지
        // 않는다) — 다음 재조정에서 우리가 그 키를 더 이상 쓰지 않으면 그냥
        // 사라진 상태가 된다.
        let composed_src: HashSet<u64> = composed.iter().map(|m| m.src).collect();
        foreign.retain(|m| {
            if composed_src.contains(&m.src) {
                tracing::warn!(
                    src = format!("{:#x}", m.src),
                    dst = format!("{:#x}", m.dst),
                    device = device.as_str(),
                    "남의 매핑이 이 디바이스의 F-17 설정과 같은 소스 키를 요구해 \
                     이번 쓰기에서 제외한다(D-17-6 — 우리가 이긴다)"
                );
                false
            } else {
                true
            }
        });

        let mut merged = foreign;
        merged.extend_from_slice(composed);

        // D-17-4 — 쓰기 직전 게이트. `HidutilBackend::apply` 자체도 같은 검증을
        // 하지만(hid_mapping.rs), 여기서 먼저 확인해 실패 이유를 원장 오염 없이
        // 정확히 보고한다.
        validate(&device.to_match(), &merged).map_err(|e| HidMappingError::InvalidMapping {
            reason: e.to_string(),
        })?;

        self.backend.apply(&device.to_match(), &merged)?;

        // ③ 쓰기 후 — 정확집합으로 되돌린다. composed 가 비면 이 디바이스를 원장에서
        // 완전히 뺀다(더 이상 우리가 관리하지 않는다).
        let mut ledger = self.ledger.load();
        if composed.is_empty() {
            ledger.remove(device);
        } else {
            ledger.insert(device.clone(), composed.to_vec());
        }
        if let Err(e) = self.ledger.store(&ledger) {
            tracing::warn!(
                error = %e,
                device = device.as_str(),
                "원장 저장 실패(쓰기 후 정확집합) — 계속 진행한다"
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;
    use std::sync::{Arc, Mutex};

    use ultrakey_core::keycode::SourceKey;
    use ultrakey_core::perdevice::FKey;
    use ultrakey_core::settings::keys as settings_keys;
    use ultrakey_platform::hid_mapping::{DeviceMappingRead, DeviceMatch};

    // ── 테스트 더블 ──────────────────────────────────────────────────────────

    /// 디바이스별 커널 상태를 흉내 내는 Fake — `BTreeMap<DeviceMatch, Vec<KeyMapping>>`.
    /// CONTRACT.md 가 요구한 그대로(전역 통짜 Vec 이 아니라 디바이스별 배열) 재현한다.
    #[derive(Default)]
    struct FakeBackend {
        by_device: Mutex<std::collections::BTreeMap<(u32, u32), Vec<KeyMapping>>>,
        apply_calls: Arc<AtomicUsize>,
        /// `apply()` 진입 시점(② 커널 쓰기 직전)에 한 번 부르는 훅 — 그 순간의
        /// 원장을 관찰하기 위해 쓴다(2단계 영속화 검증, `FakeLedger` 를 캡처해 둔다).
        on_apply: Option<Arc<dyn Fn() + Send + Sync>>,
    }

    impl FakeBackend {
        fn with_device(mut self, m: DeviceMatch, mappings: Vec<KeyMapping>) -> Self {
            self.by_device
                .get_mut()
                .unwrap()
                .insert((m.vendor_id, m.product_id), mappings);
            self
        }
    }

    impl HidMappingBackend for FakeBackend {
        fn read_current(&self, device: &DeviceMatch) -> Result<DeviceMappingRead, HidMappingError> {
            let map = self.by_device.lock().unwrap();
            let mappings = map
                .get(&(device.vendor_id, device.product_id))
                .cloned()
                .unwrap_or_default();
            Ok(DeviceMappingRead {
                services: vec![(1, mappings.clone())],
                aggregated: mappings,
                partial: false,
            })
        }

        fn apply(&self, device: &DeviceMatch, mappings: &[KeyMapping]) -> Result<(), HidMappingError> {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            if let Some(hook) = &self.on_apply {
                hook();
            }
            self.by_device
                .lock()
                .unwrap()
                .insert((device.vendor_id, device.product_id), mappings.to_vec());
            Ok(())
        }

        fn clear(&self, device: &DeviceMatch) -> Result<(), HidMappingError> {
            self.apply(device, &[])
        }
    }

    /// 메모리 위의 `LedgerStore` — 진짜 저장소 흉내(테스트 전용). `Clone` 은
    /// `Arc<Mutex<_>>` 를 공유하므로, 매니저에 넘긴 것과 별도로 들고 있는 복제본으로도
    /// 같은 상태를 관찰할 수 있다(2단계 영속화 테스트가 이걸 쓴다).
    #[derive(Default, Clone)]
    struct FakeLedger {
        inner: Arc<Mutex<ManagedLedger>>,
    }

    impl LedgerStore for FakeLedger {
        fn load(&self) -> ManagedLedger {
            self.inner.lock().unwrap().clone()
        }
        fn store(&self, ledger: &ManagedLedger) -> Result<(), String> {
            *self.inner.lock().unwrap() = ledger.clone();
            Ok(())
        }
    }

    fn mapping(src: u64, dst: u64) -> KeyMapping {
        KeyMapping { src, dst }
    }

    fn device_a() -> DeviceId {
        DeviceId::new(0x5ac, 0x24f)
    }
    fn device_b() -> DeviceId {
        DeviceId::new(0x1234, 0x5678)
    }
    fn info(id: &DeviceId) -> DeviceInfo {
        let m = id.to_match();
        DeviceInfo {
            vendor_id: m.vendor_id,
            product_id: m.product_id,
            product_name: None,
            transport: None,
            built_in: None,
        }
    }

    fn d1() -> KeyMapping {
        d1_mapping()
    }

    // ── §8 수용 기준 3번 — 합성이 서로를 지우지 않는다(D-1 + 기능1 + 기능2 전부) ──

    #[test]
    fn write_device_composes_d1_feature1_feature2_without_erasing_each_other() {
        let dev = device_a();
        let backend = FakeBackend::default();
        let ledger = FakeLedger::default();
        let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));

        let device_rows_key = settings_keys::per_device_key_remap_rows(dev.as_str());
        let f9_key = settings_keys::per_device_function_key(
            dev.as_str(),
            ultrakey_core::perdevice::FKey::F9,
        );
        let values = std::collections::BTreeMap::from([
            (
                device_rows_key,
                serde_json::to_value(vec![ultrakey_core::perdevice::KeyRemapRow {
                    from: SourceKey::LeftOption,
                    to: SourceKey::LeftCommand,
                }])
                .unwrap(),
            ),
            (
                f9_key,
                serde_json::to_value(ultrakey_core::perdevice::SystemFunction::Mute).unwrap(),
            ),
        ]);
        let mut cfg = EngineConfig {
            caps_lock_alias: Some(KeyCode::F18),
            ..Default::default()
        };
        cfg.per_device_values = values;

        mgr.apply_device(&cfg, &dev).unwrap();

        let written = mgr.backend.read_current(&dev.to_match()).unwrap().aggregated;
        assert_eq!(written.len(), 3, "{written:?}");
        assert!(written.contains(&d1()));
        assert!(written.contains(&mapping(
            SourceKey::LeftOption.hid_usage().unwrap(),
            SourceKey::LeftCommand.hid_usage().unwrap()
        )));
        assert!(written.contains(&mapping(
            SourceKey::F9.hid_usage().unwrap(),
            ultrakey_core::perdevice::SystemFunction::Mute.hid_usage().unwrap()
        )));
    }

    // ── §8 수용 기준 2번 — 디바이스 격리(S-1 재현) ──────────────────────────────

    #[test]
    fn writing_device_a_does_not_affect_device_b() {
        let dev_a = device_a();
        let dev_b = device_b();
        let backend = FakeBackend::default();
        let ledger = FakeLedger::default();
        let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));

        let rows_key = settings_keys::per_device_key_remap_rows(dev_a.as_str());
        let values = std::collections::BTreeMap::from([(
            rows_key,
            serde_json::to_value(vec![ultrakey_core::perdevice::KeyRemapRow {
                from: SourceKey::LeftOption,
                to: SourceKey::LeftCommand,
            }])
            .unwrap(),
        )]);
        let cfg = EngineConfig {
            per_device_values: values,
            ..Default::default()
        };

        mgr.apply_all(&cfg, &[info(&dev_a), info(&dev_b)]).unwrap();

        let a = mgr.backend.read_current(&dev_a.to_match()).unwrap().aggregated;
        let b = mgr.backend.read_current(&dev_b.to_match()).unwrap().aggregated;
        assert_eq!(a.len(), 1, "디바이스 A 는 자신의 설정을 받아야 한다: {a:?}");
        assert!(b.is_empty(), "디바이스 B 는 디바이스 A 전용 설정의 영향을 받으면 안 된다: {b:?}");
    }

    // ── PR #23 — 남의 매핑 보존 ──────────────────────────────────────────────

    #[test]
    fn foreign_mapping_not_in_ledger_survives_reconciliation() {
        let dev = device_a();
        let foreign = mapping(0x0000_0007_0000_0004, 0x0000_0007_0000_0005);
        let backend = FakeBackend::default().with_device(dev.to_match(), vec![foreign]);
        let ledger = FakeLedger::default();
        let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));

        // 설정에 아무 것도 없다 — 우리가 이 디바이스에 요구하는 것이 없다.
        let cfg = EngineConfig::default();
        mgr.apply_all(&cfg, &[info(&dev)]).unwrap();

        let left = mgr.backend.read_current(&dev.to_match()).unwrap().aggregated;
        assert_eq!(left, vec![foreign], "원장에 없던 남의 매핑은 그대로 남아야 한다(PR #23)");
    }

    // ── 이슈 #19 회귀 방지 — 우리 것 제거 ────────────────────────────────────

    #[test]
    fn our_mapping_is_removed_once_setting_no_longer_requires_it() {
        let dev = device_a();
        let backend = FakeBackend::default().with_device(dev.to_match(), vec![d1()]);
        let ledger = FakeLedger::default();
        // 원장에도 이미 우리가 이 매핑을 썼다고 기록돼 있다고 가정한다.
        {
            let mut l = ManagedLedger::new();
            l.insert(dev.clone(), vec![d1()]);
            ledger.store(&l).unwrap();
        }
        let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));

        // 이번 설정은 caps lock alias 를 요구하지 않는다(프리셋을 껐다).
        let cfg = EngineConfig::default();
        mgr.apply_all(&cfg, &[info(&dev)]).unwrap();

        let left = mgr.backend.read_current(&dev.to_match()).unwrap().aggregated;
        assert!(left.is_empty(), "더 이상 요구되지 않는 우리 매핑은 사라져야 한다: {left:?}");
    }

    // ── B.4.2 / D-17-6 — src 충돌 시 우리가 이긴다 ───────────────────────────

    #[test]
    fn our_mapping_wins_over_foreign_mapping_with_same_src_and_array_stays_valid() {
        let dev = device_a();
        let src = SourceKey::CapsLock.hid_usage().unwrap();
        // 남이 이미 caps lock 을 escape 로 매핑해 뒀다(우리 원장 밖).
        let foreign_conflicting = mapping(src, SourceKey::LeftControl.hid_usage().unwrap());
        let backend =
            FakeBackend::default().with_device(dev.to_match(), vec![foreign_conflicting]);
        let ledger = FakeLedger::default();
        let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));

        // 우리 설정도 같은 caps lock(D-1)을 요구한다.
        let cfg = EngineConfig {
            caps_lock_alias: Some(KeyCode::F18),
            ..Default::default()
        };
        mgr.apply_device(&cfg, &dev).unwrap();

        let left = mgr.backend.read_current(&dev.to_match()).unwrap().aggregated;
        assert_eq!(left, vec![d1()], "우리 매핑이 이겨야 하고, 배열에 src 중복이 없어야 한다");
    }

    // ── ⭐ 2단계 영속화 — ② 커널 쓰기 직전 원장이 prev ∪ composed 상위집합이다 ──

    #[test]
    fn ledger_is_superset_of_prev_and_composed_at_kernel_write_time() {
        let dev = device_a();
        let prev_mapping = mapping(0x0000_0007_0000_0011, 0x0000_0007_0000_0012);

        let ledger = FakeLedger::default();
        {
            let mut l = ManagedLedger::new();
            l.insert(dev.clone(), vec![prev_mapping]);
            ledger.store(&l).unwrap();
        }

        // `on_apply` 훅은 `apply()`(② 커널 쓰기) 진입 시점에 불린다 — 그 순간 원장이
        // 이미 ①(쓰기 전 상위집합)을 반영했는지 이 복제본(같은 `Arc<Mutex<_>>` 공유)
        // 으로 관찰한다.
        let observed: Arc<Mutex<Option<ManagedLedger>>> = Arc::new(Mutex::new(None));
        let observed_clone = Arc::clone(&observed);
        let ledger_for_hook = ledger.clone();
        let backend = FakeBackend {
            on_apply: Some(Arc::new(move || {
                *observed_clone.lock().unwrap() = Some(ledger_for_hook.load());
            })),
            ..Default::default()
        };

        let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));

        let cfg = EngineConfig {
            caps_lock_alias: Some(KeyCode::F18),
            ..Default::default()
        };
        mgr.apply_device(&cfg, &dev).unwrap();

        let seen = observed.lock().unwrap().clone().expect("apply() 안에서 원장을 관찰했어야 한다");
        let seen_for_dev = seen.get(&dev).cloned().unwrap_or_default();
        assert!(
            seen_for_dev.contains(&prev_mapping),
            "쓰기 직전 원장은 이전 값을 포함하는 상위집합이어야 한다: {seen_for_dev:?}"
        );
        assert!(
            seen_for_dev.contains(&d1()),
            "쓰기 직전 원장은 새로 합성한 값도 포함해야 한다: {seen_for_dev:?}"
        );
    }

    // ── 미연결 디바이스 — §5 항목 1, §3.6 규칙 6 ─────────────────────────────

    #[test]
    fn apply_all_does_not_touch_devices_absent_from_attached_list() {
        let dev = device_a();
        let backend = FakeBackend::default();
        let ledger = FakeLedger::default();
        // 원장에 디바이스가 이미 등록돼 있지만(과거에 관리했었다) 이번엔 붙어 있지 않다.
        {
            let mut l = ManagedLedger::new();
            l.insert(dev.clone(), vec![d1()]);
            ledger.store(&l).unwrap();
        }
        let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));

        let cfg = EngineConfig {
            caps_lock_alias: Some(KeyCode::F18),
            ..Default::default()
        };
        // attached 목록이 비어 있다 — 이 디바이스는 뽑혀 있다.
        mgr.apply_all(&cfg, &[]).unwrap();

        let ledger_after = mgr.ledger.load();
        assert!(
            ledger_after.contains_key(&dev),
            "뽑혀 있는 디바이스는 원장에서 지워지면 안 된다"
        );
    }

    #[test]
    fn cleanup_skips_devices_not_currently_attached() {
        let dev = device_a();
        let foreign = mapping(0x0000_0007_0000_0004, 0x0000_0007_0000_0005);
        let backend = FakeBackend::default().with_device(dev.to_match(), vec![foreign, d1()]);
        let ledger = FakeLedger::default();
        {
            let mut l = ManagedLedger::new();
            l.insert(dev.clone(), vec![d1()]);
            ledger.store(&l).unwrap();
        }
        let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));

        // 종료 시점에 이 디바이스가 뽑혀 있다 — cleanup 이 건드리면 안 된다.
        mgr.cleanup(&[]).unwrap();

        let left = mgr.backend.read_current(&dev.to_match()).unwrap().aggregated;
        assert_eq!(left, vec![foreign, d1()], "뽑혀 있는 디바이스는 손대지 않아야 한다");
        assert!(mgr.ledger.load().contains_key(&dev), "원장에도 그대로 남아야 한다");
    }

    #[test]
    fn cleanup_removes_our_mapping_only_from_attached_devices() {
        let dev = device_a();
        let foreign = mapping(0x0000_0007_0000_0004, 0x0000_0007_0000_0005);
        let backend = FakeBackend::default().with_device(dev.to_match(), vec![foreign, d1()]);
        let ledger = FakeLedger::default();
        {
            let mut l = ManagedLedger::new();
            l.insert(dev.clone(), vec![d1()]);
            ledger.store(&l).unwrap();
        }
        let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));

        mgr.cleanup(&[info(&dev)]).unwrap();

        let left = mgr.backend.read_current(&dev.to_match()).unwrap().aggregated;
        assert_eq!(left, vec![foreign], "우리 매핑만 사라지고 남의 매핑은 남아야 한다");
        assert!(!mgr.ledger.load().contains_key(&dev), "정리한 디바이스는 원장에서 빠져야 한다");
    }

    // ── 전역 마이그레이션 — 정확히 한 번, per-device 쓰기보다 먼저(D-17-5) ───

    #[test]
    fn reconcile_on_start_calls_global_migration_exactly_once_before_any_device_write() {
        let dev = device_a();
        let backend = FakeBackend::default();
        let ledger = FakeLedger::default();
        let calls: Arc<Mutex<Vec<KeyMapping>>> = Arc::new(Mutex::new(Vec::new()));
        // 호출 순서를 기록하는 공유 로그 — 마이그레이션과 백엔드 apply 양쪽이 여기 적는다.
        let order: Arc<Mutex<Vec<&'static str>>> = Arc::new(Mutex::new(Vec::new()));

        struct OrderedMigration {
            calls: Arc<Mutex<Vec<KeyMapping>>>,
            order: Arc<Mutex<Vec<&'static str>>>,
        }
        impl GlobalD1Migration for OrderedMigration {
            fn migrate_clear_global_d1(&self, d1: KeyMapping) -> Result<bool, HidMappingError> {
                self.calls.lock().unwrap().push(d1);
                self.order.lock().unwrap().push("migration");
                Ok(true)
            }
        }

        struct OrderedBackend {
            inner: FakeBackend,
            order: Arc<Mutex<Vec<&'static str>>>,
        }
        impl HidMappingBackend for OrderedBackend {
            fn read_current(&self, d: &DeviceMatch) -> Result<DeviceMappingRead, HidMappingError> {
                self.inner.read_current(d)
            }
            fn apply(&self, d: &DeviceMatch, m: &[KeyMapping]) -> Result<(), HidMappingError> {
                self.order.lock().unwrap().push("device_write");
                self.inner.apply(d, m)
            }
            fn clear(&self, d: &DeviceMatch) -> Result<(), HidMappingError> {
                self.apply(d, &[])
            }
        }

        let migration = OrderedMigration {
            calls: calls.clone(),
            order: order.clone(),
        };
        let backend = OrderedBackend { inner: backend, order: order.clone() };
        let mgr = PathBManager::new(Box::new(backend), Some(Box::new(migration)), Box::new(ledger));

        let cfg = EngineConfig {
            caps_lock_alias: Some(KeyCode::F18),
            ..Default::default()
        };
        let report = mgr.reconcile_on_start(&cfg, &[info(&dev)]).unwrap();

        assert!(report.global_migration_cleared);
        assert_eq!(calls.lock().unwrap().len(), 1, "마이그레이션은 정확히 한 번만 호출돼야 한다");
        assert_eq!(calls.lock().unwrap()[0], d1_mapping());

        // 두 번째 reconcile_on_start 호출 — 더 이상 호출되면 안 된다.
        mgr.reconcile_on_start(&cfg, &[info(&dev)]).unwrap();
        assert_eq!(calls.lock().unwrap().len(), 1, "두 번째 호출에서는 재시도하지 않아야 한다");

        let seen_order = order.lock().unwrap().clone();
        assert_eq!(seen_order[0], "migration", "마이그레이션이 어떤 디바이스 쓰기보다 먼저 와야 한다");
        assert!(seen_order[1..].contains(&"device_write"));
    }

    // ── 3상태 폴백이 엔진 수준에서도 성립 ────────────────────────────────────

    #[test]
    fn tri_state_fallback_holds_at_engine_level_common_value_device_override_and_off() {
        let dev = device_a();
        let common_key = settings_keys::per_device_function_key(
            settings_keys::PER_DEVICE_COMMON_SCOPE,
            FKey::F9,
        );
        let device_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F9);

        // 1) 공통값을 따른다.
        {
            let backend = FakeBackend::default();
            let ledger = FakeLedger::default();
            let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));
            let cfg = EngineConfig {
                per_device_values: std::collections::BTreeMap::from([(
                    common_key.clone(),
                    serde_json::to_value(ultrakey_core::perdevice::SystemFunction::Mute).unwrap(),
                )]),
                ..Default::default()
            };
            mgr.apply_device(&cfg, &dev).unwrap();
            let left = mgr.backend.read_current(&dev.to_match()).unwrap().aggregated;
            assert_eq!(
                left,
                vec![mapping(
                    SourceKey::F9.hid_usage().unwrap(),
                    ultrakey_core::perdevice::SystemFunction::Mute.hid_usage().unwrap()
                )]
            );
        }

        // 2) 디바이스 값이 공통을 덮는다.
        {
            let backend = FakeBackend::default();
            let ledger = FakeLedger::default();
            let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));
            let cfg = EngineConfig {
                per_device_values: std::collections::BTreeMap::from([
                    (
                        common_key.clone(),
                        serde_json::to_value(ultrakey_core::perdevice::SystemFunction::Mute)
                            .unwrap(),
                    ),
                    (
                        device_key.clone(),
                        serde_json::to_value(ultrakey_core::perdevice::SystemFunction::VolumeUp)
                            .unwrap(),
                    ),
                ]),
                ..Default::default()
            };
            mgr.apply_device(&cfg, &dev).unwrap();
            let left = mgr.backend.read_current(&dev.to_match()).unwrap().aggregated;
            assert_eq!(
                left,
                vec![mapping(
                    SourceKey::F9.hid_usage().unwrap(),
                    ultrakey_core::perdevice::SystemFunction::VolumeUp.hid_usage().unwrap()
                )]
            );
        }

        // 3) 디바이스의 JSON null 은 명시적 끔 — 공통에 값이 있어도 이긴다.
        {
            let backend = FakeBackend::default();
            let ledger = FakeLedger::default();
            let mgr = PathBManager::new(Box::new(backend), None, Box::new(ledger));
            let cfg = EngineConfig {
                per_device_values: std::collections::BTreeMap::from([
                    (
                        common_key,
                        serde_json::to_value(ultrakey_core::perdevice::SystemFunction::Mute)
                            .unwrap(),
                    ),
                    (device_key, serde_json::Value::Null),
                ]),
                ..Default::default()
            };
            mgr.apply_device(&cfg, &dev).unwrap();
            let left = mgr.backend.read_current(&dev.to_match()).unwrap().aggregated;
            assert!(left.is_empty(), "명시적 끔은 매핑이 없어야 한다: {left:?}");
        }
    }

    // ── d1_for() 판정 — 기존 desired_mappings_for() 판정을 그대로 재사용 ──────

    #[test]
    fn d1_for_none_alias_is_none() {
        let cfg = EngineConfig::default();
        assert_eq!(d1_for(&cfg), None);
    }

    #[test]
    fn d1_for_f18_alias_returns_d1_mapping() {
        let cfg = EngineConfig {
            caps_lock_alias: Some(KeyCode::F18),
            ..Default::default()
        };
        assert_eq!(d1_for(&cfg), Some(d1_mapping()));
    }

    #[test]
    fn d1_for_non_f18_alias_is_none_and_defensive() {
        let cfg = EngineConfig {
            caps_lock_alias: Some(KeyCode::ESCAPE),
            ..Default::default()
        };
        assert_eq!(d1_for(&cfg), None);
    }

    // ── HidutilGlobalMigration — 실제 hidutil 을 부르지 않는 순수 배선 확인 ───
    // ⛔ 여기서는 `HidutilGlobalMigration` 을 인스턴스화·트레이트 바운드 확인만
    // 한다(실제 서브프로세스 실행 금지 — 사용자 키보드가 걸려 있다). 실행 경로 자체는
    // `ultrakey_platform::hid_mapping` 의 단위 테스트가 이미 순수 함수 단위로 검증한다.
    #[test]
    fn hidutil_global_migration_implements_trait() {
        fn assert_impl<T: GlobalD1Migration>() {}
        assert_impl::<HidutilGlobalMigration>();
    }

    #[test]
    fn null_ledger_store_is_always_empty_and_store_is_noop() {
        let store = NullLedgerStore;
        assert!(store.load().is_empty());
        let mut l = ManagedLedger::new();
        l.insert(device_a(), vec![d1()]);
        assert!(store.store(&l).is_ok());
        // no-op 이므로 다음 load() 도 여전히 비어 있어야 한다.
        assert!(store.load().is_empty());
    }
}
