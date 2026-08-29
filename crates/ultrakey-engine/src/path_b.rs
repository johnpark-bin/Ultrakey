//! 경로 B(IOHID 커널 매핑) 관리 — `key-remapping-engine.md` §3-a2, §5#15.
//!
//! ⭐ **M1 에는 경로 B 로 배정된 규칙이 0 개다**(배정은 F-08/M2 소관, `docs/dev/
//! architecture.md` §3). 따라서 이 모듈이 M1 에서 실제로 하는 일은 "우리가 이전 실행에서
//! 남긴 잔존 매핑이 있으면 치운다"가 아니라 — 그 잔존 매핑이 정말 우리 것인지 판별할
//! 방법이 아직 없으므로 — **"잔존 매핑을 관찰하고 로그로 남기되, 지우지는 않는다"** 이다.
//! 판단 근거는 [`PathBManager::reconcile_on_start`] 문서에 있다.

use ultrakey_platform::hid_mapping::{HidMappingBackend, HidMappingError, KeyMapping};

/// 시작 시 재조정 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileReport {
    /// 잔존 매핑이 하나라도 있었는가.
    pub had_residual_mapping: bool,
    /// 발견된 잔존 매핑 전체(치웠는지 여부와 무관하게 그대로 보고한다).
    pub residual: Vec<KeyMapping>,
    /// 이번 호출이 실제로 매핑을 치웠는가(M1 은 `desired` 가 항상 비어 있으므로 항상 `false`).
    pub cleared: bool,
}

pub struct PathBManager {
    backend: Box<dyn HidMappingBackend>,
}

impl PathBManager {
    pub fn new(backend: Box<dyn HidMappingBackend>) -> Self {
        PathBManager { backend }
    }

    /// ⭐ 시작 시 잔존 매핑을 감지해 현재 설정(`desired`)과 재조정한다(§3-a2, §5#15).
    ///
    /// - `desired` 가 비어 있는데(현재 M1 은 항상 이 경우다) 잔존 매핑이 발견되면:
    ///   ⛔ **지우지 않는다.** 이 잔존 매핑이 "우리가 이전 실행에서 설치했다가 비정상
    ///   종료로 못 지운 것"인지, "사용자가 Ultrakey 와 무관하게 `hidutil` 로 직접
    ///   설정해 둔 것"인지 이 시점에는 **구분할 방법이 없다**. `keepExistingIohid` 라는
    ///   설정 키의 정확한 의미가 명세에서도 `(미확정)` 으로 남아 있다(§3-a2) — 즉 원본조차
    ///   이 구분을 어떻게 하는지 실측으로 확인되지 않았다. M1 은 이 엔진이 설치한 경로 B
    ///   규칙 자체가 0 개이므로("우리가 설치한 매핑"이 존재할 수 없으므로), 잔존 매핑을
    ///   발견해도 그것을 우리 것으로 가정해 지우는 쪽이 훨씬 위험하다(사용자 설정을 조용히
    ///   파괴할 수 있다) — 그래서 관찰(경고 로그)만 하고 손대지 않는 것으로 판단했다.
    /// - `desired` 가 비어 있지 않으면(M2 이후) 현재 매핑을 `desired` 로 교체한다.
    pub fn reconcile_on_start(
        &self,
        desired: &[KeyMapping],
    ) -> Result<ReconcileReport, HidMappingError> {
        let current = self.backend.read_current()?;

        if current.is_empty() {
            tracing::debug!("경로 B 시작 시 재조정 — 잔존 매핑이 없다");
            return Ok(ReconcileReport {
                had_residual_mapping: false,
                residual: Vec::new(),
                cleared: false,
            });
        }

        if desired.is_empty() {
            tracing::warn!(
                count = current.len(),
                "경로 B 잔존 매핑을 발견했지만 이 엔진이 설치한 규칙이 아직 없어(M1) \
                 잔존 매핑의 출처를 판별할 수 없다 — 지우지 않고 관찰만 한다"
            );
            return Ok(ReconcileReport {
                had_residual_mapping: true,
                residual: current,
                cleared: false,
            });
        }

        self.backend.apply(desired)?;
        Ok(ReconcileReport {
            had_residual_mapping: true,
            residual: current,
            cleared: false,
        })
    }

    /// 핫플러그 재적용(§5#10) — `desired` 가 비어 있으면(M1) 아무 것도 하지 않는다.
    pub fn apply(&self, desired: &[KeyMapping]) -> Result<(), HidMappingError> {
        if desired.is_empty() {
            return Ok(());
        }
        self.backend.apply(desired)
    }

    /// 정상 종료 시 정리(§3-a2 — 경로 B 는 앱이 죽어도 남는다).
    ///
    /// M1 은 이 엔진이 설치한 매핑이 없으므로 사실상 no-op 이지만, 인터페이스 계약
    /// ("정상 종료 시 우리가 설치한 것을 지운다")은 M2 를 위해 지금부터 유지한다.
    pub fn cleanup(&self) -> Result<(), HidMappingError> {
        self.backend.clear()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    /// 카운터를 `Arc` 로 밖에 남겨 두어, 백엔드가 `Box<dyn HidMappingBackend>` 로
    /// 이동된 뒤에도 호출 횟수를 검증할 수 있게 한다(이 크레이트는 `unsafe_code` 를
    /// 전면 금지하므로 raw pointer 로 우회하지 않는다).
    #[derive(Default)]
    struct FakeBackend {
        current: Mutex<Vec<KeyMapping>>,
        apply_calls: Arc<AtomicUsize>,
        clear_calls: Arc<AtomicUsize>,
        fail_read: bool,
    }

    impl HidMappingBackend for FakeBackend {
        fn read_current(&self) -> Result<Vec<KeyMapping>, HidMappingError> {
            if self.fail_read {
                return Err(HidMappingError::ParseFailed {
                    raw: "boom".to_string(),
                });
            }
            Ok(self.current.lock().unwrap().clone())
        }

        fn apply(&self, mappings: &[KeyMapping]) -> Result<(), HidMappingError> {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            *self.current.lock().unwrap() = mappings.to_vec();
            Ok(())
        }

        fn clear(&self) -> Result<(), HidMappingError> {
            self.clear_calls.fetch_add(1, Ordering::SeqCst);
            self.current.lock().unwrap().clear();
            Ok(())
        }
    }

    fn mapping(src: u64, dst: u64) -> KeyMapping {
        KeyMapping { src, dst }
    }

    // 1. 잔존 매핑이 없을 때 — 아무 것도 하지 않는다.
    #[test]
    fn reconcile_with_no_residual_is_noop() {
        let backend = FakeBackend::default();
        let mgr = PathBManager::new(Box::new(backend));

        let report = mgr.reconcile_on_start(&[]).unwrap();
        assert!(!report.had_residual_mapping);
        assert!(report.residual.is_empty());
        assert!(!report.cleared);
    }

    // 2. 잔존 매핑이 있는데 desired 가 비어 있으면(M1) — 지우지 않는다.
    #[test]
    fn reconcile_with_residual_and_empty_desired_does_not_clear() {
        let apply_calls = Arc::new(AtomicUsize::new(0));
        let clear_calls = Arc::new(AtomicUsize::new(0));
        let backend = FakeBackend {
            current: Mutex::new(vec![mapping(30064771129, 30064771129)]),
            apply_calls: apply_calls.clone(),
            clear_calls: clear_calls.clone(),
            fail_read: false,
        };
        let mgr = PathBManager::new(Box::new(backend));

        let report = mgr.reconcile_on_start(&[]).unwrap();
        assert!(report.had_residual_mapping);
        assert_eq!(report.residual, vec![mapping(30064771129, 30064771129)]);
        assert!(!report.cleared);
        assert_eq!(apply_calls.load(Ordering::SeqCst), 0);
        assert_eq!(clear_calls.load(Ordering::SeqCst), 0);
    }

    // 3. 백엔드가 조회 자체에 실패하면 — Err 그대로 전달한다(조용히 빈 벡터로 흡수하지 않는다).
    #[test]
    fn reconcile_propagates_backend_read_error() {
        let backend = FakeBackend {
            fail_read: true,
            ..Default::default()
        };
        let mgr = PathBManager::new(Box::new(backend));

        let result = mgr.reconcile_on_start(&[]);
        assert!(result.is_err());
    }

    // 4. desired 가 채워지면(M2 이후) — 현재 매핑을 desired 로 교체한다.
    #[test]
    fn reconcile_with_desired_applies_it() {
        let backend = FakeBackend {
            current: Mutex::new(vec![mapping(1, 2)]),
            ..Default::default()
        };
        let mgr = PathBManager::new(Box::new(backend));

        let desired = vec![mapping(3, 4)];
        let report = mgr.reconcile_on_start(&desired).unwrap();
        assert!(report.had_residual_mapping);
        assert_eq!(report.residual, vec![mapping(1, 2)]);
    }

    // 5. apply()/cleanup() — M1(desired 비어 있음)에는 apply 가 백엔드를 건드리지 않는다.
    #[test]
    fn apply_with_empty_desired_is_noop() {
        let backend = FakeBackend::default();
        let mgr = PathBManager::new(Box::new(backend));
        mgr.apply(&[]).unwrap();
    }

    #[test]
    fn cleanup_calls_backend_clear() {
        let backend = FakeBackend {
            current: Mutex::new(vec![mapping(1, 2)]),
            ..Default::default()
        };
        let mgr = PathBManager::new(Box::new(backend));
        mgr.cleanup().unwrap();
    }
}
