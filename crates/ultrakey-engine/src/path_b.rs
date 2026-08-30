//! 경로 B(IOHID 커널 매핑) 관리 — `key-remapping-engine.md` §3-a2, §5#15.
//!
//! ⭐ **M1 에는 경로 B 로 배정된 규칙이 0 개였다**(배정은 F-08/M2 소관, `docs/dev/
//! architecture.md` §3). M2 부터 D-1(§6.1)이 첫 규칙 — caps lock 에 의존하는 프리셋이
//! 하나라도 켜져 있으면 `caps lock → F18` 매핑을 설치한다. [`desired_mappings_for`] 가
//! `EngineConfig::caps_lock_alias` 로부터 그 매핑을 계산한다.
//!
//! 시작 시 잔존 매핑을 만났을 때 그것이 "우리 것"인지 "사용자가 Ultrakey 와 무관하게
//! 직접 걸어 둔 것"인지 구분할 방법이 없는 경우(M1 부터 이어지는 문제)의 판단 근거는
//! [`PathBManager::reconcile_on_start`] 문서에 있다.

use ultrakey_core::keycode::KeyCode;
use ultrakey_core::settings::EngineConfig;
use ultrakey_platform::hid_mapping::{HidMappingBackend, HidMappingError, KeyMapping};

/// D-1 이 쓰는 두 HID usage ID(USB HID Usage Tables, 키보드 페이지 `0x07`).
/// `hidutil` 관례대로 `(page << 32) | usage` 로 인코딩한다 — `hid_mapping.rs` 의
/// 실측 테스트가 쓰는 caps lock 값(`30064771129` = `0x7_0000_0039`)과 일치해
/// 인코딩 자체는 교차 확인됐다(위임 지시서가 확정한 값).
const HID_USAGE_CAPS_LOCK: u64 = 0x0000_0007_0000_0039;
const HID_USAGE_F18: u64 = 0x0000_0007_0000_006D;

/// ⭐ D-1 이 설치하는 **바로 그** 매핑. 잔존물 중 "우리 것"을 식별하는 서명이기도 하다
/// (2026-08-30, 이슈 #19).
///
/// M1 에는 이 엔진이 설치하는 경로 B 규칙이 **0개**였다. 그래서 잔존 매핑을 만나면
/// "우리 것인지 사용자 것인지 구분할 방법이 없다"는 것이 참이었고,
/// [`PathBManager::reconcile_on_start`] 가 아무것도 지우지 않기로 한 근거도 그것이었다.
/// **D-1 이 그 전제를 무효로 만들었다** — 이제 우리가 설치하는 매핑은 정확히 하나이고,
/// 그 값이 무엇인지 우리가 안다. 따라서 이 서명과 일치하는 항목은 "우리 것"으로 판정할
/// 수 있고, 일치하지 않는 항목은 여전히 손대지 않는다
/// (`docs/dev/manual-verification.md` 부록 A #3).
pub fn d1_mapping() -> KeyMapping {
    KeyMapping { src: HID_USAGE_CAPS_LOCK, dst: HID_USAGE_F18 }
}

/// 이 매핑이 우리(D-1)가 설치한 것인가.
fn is_ours(m: &KeyMapping) -> bool {
    *m == d1_mapping()
}

/// 남의 매핑만 남긴다 — 우리가 무엇을 적용하든 이 항목들은 항상 보존한다.
fn foreign_only(current: &[KeyMapping]) -> Vec<KeyMapping> {
    current.iter().copied().filter(|m| !is_ours(m)).collect()
}

/// D-1 — `cfg.caps_lock_alias` 로부터 경로 B 가 실제로 설치해야 할 매핑을 계산한다.
/// alias 가 없으면(caps lock 에 의존하는 프리셋이 하나도 없거나 `Synthesize Caps Lock
/// Remap` 이 켜져 있음, `docs/dev/architecture.md` §6.1) 빈 벡터를 반환한다 — `apply`/
/// `reconcile_on_start` 양쪽이 빈 벡터를 "정리"로 해석한다.
pub fn desired_mappings_for(cfg: &EngineConfig) -> Vec<KeyMapping> {
    match cfg.caps_lock_alias {
        Some(alias) if alias == KeyCode::F18 => {
            vec![d1_mapping()]
        }
        Some(_) => {
            // D-1 은 F18 로 고정돼 있다(architecture.md §6.1) — 다른 값이 들어오면
            // 설계 위반이다. 지어낸 매핑을 만들지 않고 빈 벡터로 방어한다.
            tracing::error!(
                "caps_lock_alias 가 F18 이 아니다 — D-1 설계 위반, 경로 B 매핑을 만들지 않는다"
            );
            Vec::new()
        }
        None => Vec::new(),
    }
}

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
            // ⭐ `info!` 다(`debug!` 아님). F-15 §3.1.1 결정 2 가 "정리의 정본은 종료가
            // 아니라 기동" 이라고 못박았고, `docs/dev/manual-verification.md` 부록 A 가
            // 이 재조정이 매 기동마다 실제로 돌았는지를 **기본 로그 레벨에서** 확인한다.
            // 조용한 성공은 "돌았는데 깨끗했다"와 "아예 안 돌았다"를 구분해 주지 못한다.
            if desired.is_empty() {
                tracing::info!("경로 B 시작 시 재조정 — 잔존 매핑이 없다");
            } else {
                // ⭐ M2/D-1 — 잔존 매핑이 전혀 없는 깨끗한 상태에서 이번 설정이 이미
                // caps lock alias 를 요구한다면(예: 이전 실행에서 켠 caps lock 프리셋을
                // 이번 부팅에도 그대로 쓰는 경우) 소유권 판별 문제 자체가 없다 — 지울
                // 잔존물이 없으므로 그냥 설치한다.
                self.backend.apply(desired)?;
                tracing::info!(
                    count = desired.len(),
                    "경로 B 시작 시 재조정 — 잔존 매핑은 없었지만 이번 설정이 요구하는 \
                     매핑을 새로 설치했다(D-1)"
                );
            }
            return Ok(ReconcileReport {
                had_residual_mapping: false,
                residual: Vec::new(),
                cleared: false,
            });
        }

        let foreign = foreign_only(&current);
        let ours_count = current.len() - foreign.len();

        if desired.is_empty() {
            // ⭐ M2/D-1 재판정 (2026-08-30, 이슈 #19). M1 은 여기서 아무것도 지우지
            // 않았다 — 그때는 "우리가 설치한 매핑"이 존재할 수 없어 잔존물의 출처를
            // 판별할 방법이 정말로 없었기 때문이다. D-1 이후로 그 전제는 거짓이다.
            //
            // ⛔ **지우지 않으면 caps lock 이 복구 불가능하게 죽는다.** 실측(이슈 #19):
            // `Advanced ▸ Synthesize Caps Lock Remap` 을 켜면 `caps_lock_alias` 가
            // `None` 이 되어 `desired` 가 비지만, 커널에는 `caps lock → F18` 이 그대로
            // 남는다. 그러면 물리 caps lock 은 F18 로 도착하는데 그것을 caps lock 으로
            // 되돌릴 alias 가 없어 **어떤 규칙도 매칭되지 않고, 앱 안에서 되돌릴 수단도
            // 없다.** 사용자가 보고한 "아예 캡스락 이벤트 자체가 캡처가 안됨"이 이
            // 상태다. `manual-verification.md` 부록 A #1("앱 정상 종료 후 Ultrakey 가
            // 설치한 매핑이 남아 있지 않다")도 같은 것을 요구한다.
            if ours_count > 0 {
                self.backend.apply(&foreign)?;
                tracing::info!(
                    ours = ours_count,
                    foreign = foreign.len(),
                    "경로 B 시작 시 재조정 — 이번 설정은 매핑을 요구하지 않으므로 \
                     우리가 설치한 D-1 매핑만 제거했다(남의 매핑은 그대로 둔다)"
                );
                return Ok(ReconcileReport {
                    had_residual_mapping: true,
                    residual: current,
                    cleared: true,
                });
            }

            // 우리 서명과 일치하는 것이 하나도 없다 — 전부 남의 매핑이다.
            // ⛔ 손대지 않는다(부록 A #3).
            tracing::warn!(
                count = current.len(),
                "경로 B 잔존 매핑을 발견했지만 우리(D-1) 서명과 일치하지 않는다 \
                 — 사용자가 직접 건 매핑으로 보고 지우지 않는다"
            );
            return Ok(ReconcileReport {
                had_residual_mapping: true,
                residual: current,
                cleared: false,
            });
        }

        // ⭐ `desired` 만 적용하면 남의 매핑까지 함께 날아간다(`hidutil` 의
        // `UserKeyMapping` 은 전체 교체다). 부록 A #3 을 지키려면 남의 매핑을 반드시
        // 함께 실어 보내야 한다 — M1 구현은 이 부분이 빠져 있었다.
        let mut merged = foreign;
        merged.extend_from_slice(desired);
        self.backend.apply(&merged)?;
        Ok(ReconcileReport {
            had_residual_mapping: true,
            residual: current,
            cleared: false,
        })
    }

    /// 핫플러그 재적용(§5#10)과 설정 변경 시 재적용(D-1, `Engine::reconfigure`) 양쪽이
    /// 쓴다.
    ///
    /// ⭐ M1 은 `desired.is_empty()` 일 때 백엔드를 아예 건드리지 않았다 — 당시
    /// `desired` 가 항상 비어 있었고 설치된 매핑도 없어 정리할 것 자체가 없었기
    /// 때문이다. M2(D-1)부터는 caps lock 프리셋을 켰다 껐다 할 수 있으므로 "빈
    /// `desired` 로 재적용"이 실제로 "이미 설치된 매핑을 지운다"는 뜻이 될 수 있다 —
    /// 그 지름길을 없애고 항상 백엔드에 그대로 전달한다(`HidutilBackend::apply(&[])`
    /// 는 `clear()` 와 동일한 효과 — `hid_mapping.rs` 참고).
    pub fn apply(&self, desired: &[KeyMapping]) -> Result<(), HidMappingError> {
        self.backend.apply(desired)
    }

    /// 정상 종료 시 정리(§3-a2 — 경로 B 는 앱이 죽어도 남는다).
    ///
    /// M1 은 이 엔진이 설치한 매핑이 없으므로 사실상 no-op 이지만, 인터페이스 계약
    /// ("정상 종료 시 우리가 설치한 것을 지운다")은 M2 를 위해 지금부터 유지한다.
    pub fn cleanup(&self) -> Result<(), HidMappingError> {
        // ⭐ `clear()` 는 `UserKeyMapping` **전체**를 비운다 — 사용자가 Ultrakey 와
        // 무관하게 직접 걸어 둔 매핑까지 조용히 파괴한다(`manual-verification.md`
        // 부록 A #3 위반). M1 은 우리가 설치하는 매핑이 0개여서 이 차이가 드러나지
        // 않았을 뿐이다. D-1 이후로는 "우리 것만" 걷어내야 한다(이슈 #19).
        let current = self.backend.read_current()?;
        let foreign = foreign_only(&current);
        if foreign.len() == current.len() {
            // 우리 것이 없다 — 아무것도 하지 않는다(남의 매핑을 재적용할 이유도 없다).
            return Ok(());
        }
        self.backend.apply(&foreign)
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

    // 2. 잔존 매핑이 **우리 서명과 다르면**(= 사용자가 직접 건 매핑) desired 가 비어도
    //    지우지 않는다 — `manual-verification.md` 부록 A #3.
    //    ⚠️ 여기 쓰는 매핑은 `caps → caps`(dst 가 F18 이 아니다)라 D-1 서명이 아니다.
    #[test]
    fn reconcile_with_foreign_residual_and_empty_desired_does_not_clear() {
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

    // 5. apply() — 빈 desired 도 이제 백엔드에 그대로 전달한다(M2: 지름길을 없앴다 —
    //    D-1 이 이전에 설치한 매핑을 끌 수 있어야 하기 때문이다).
    #[test]
    fn apply_forwards_empty_desired_to_backend_for_cleanup() {
        let apply_calls = Arc::new(AtomicUsize::new(0));
        let backend = FakeBackend {
            apply_calls: apply_calls.clone(),
            ..Default::default()
        };
        let mgr = PathBManager::new(Box::new(backend));
        mgr.apply(&[]).unwrap();
        assert_eq!(
            apply_calls.load(Ordering::SeqCst),
            1,
            "빈 desired 도 백엔드에 전달해 잔존 매핑을 지울 수 있어야 한다(D-1)"
        );
    }

    // 5-bis. cleanup() — ⭐ 우리(D-1) 매핑만 걷어내고 남의 매핑은 **보존**한다.
    //
    // 기대값의 출처는 코드가 아니라 `docs/dev/manual-verification.md` 부록 A 다:
    //   #1 "앱 정상 종료 후 → Ultrakey 가 설치한 매핑이 남아 있지 않다"
    //   #3 "사용자가 직접 설정해 둔 매핑 → ⛔ 그것을 지우지 않는다"
    // 이전 구현(`backend.clear()`)은 #1 은 만족하지만 #3 을 어겼다.
    #[test]
    fn cleanup_removes_only_our_d1_mapping_and_keeps_foreign_ones() {
        let foreign = mapping(0x0000_0007_0000_0004, 0x0000_0007_0000_0005);
        let clear_calls = Arc::new(AtomicUsize::new(0));
        let backend = FakeBackend {
            current: Mutex::new(vec![foreign, d1_mapping()]),
            clear_calls: clear_calls.clone(),
            ..Default::default()
        };
        let mgr = PathBManager::new(Box::new(backend));

        mgr.cleanup().unwrap();

        // 백엔드의 최종 상태에 남의 매핑만 남아 있어야 한다.
        let left = mgr.backend.read_current().unwrap();
        assert_eq!(left, vec![foreign], "우리 것만 사라지고 남의 것은 그대로여야 한다");
        assert_eq!(
            clear_calls.load(Ordering::SeqCst),
            0,
            "전체를 비우는 clear() 를 불러서는 안 된다 — 남의 매핑까지 파괴한다"
        );
    }

    // 5-ter. cleanup() — 우리 매핑이 없으면 아무것도 건드리지 않는다.
    #[test]
    fn cleanup_is_noop_when_we_installed_nothing() {
        let foreign = mapping(1, 2);
        let apply_calls = Arc::new(AtomicUsize::new(0));
        let clear_calls = Arc::new(AtomicUsize::new(0));
        let backend = FakeBackend {
            current: Mutex::new(vec![foreign]),
            apply_calls: apply_calls.clone(),
            clear_calls: clear_calls.clone(),
            ..Default::default()
        };
        let mgr = PathBManager::new(Box::new(backend));

        mgr.cleanup().unwrap();

        assert_eq!(apply_calls.load(Ordering::SeqCst), 0);
        assert_eq!(clear_calls.load(Ordering::SeqCst), 0);
    }

    // 5-quater. ⭐ 이슈 #19 증상 B 의 회귀 방지 — 잔존물이 **우리 D-1 매핑**이고
    //           이번 설정이 매핑을 요구하지 않으면(= `Synthesize Caps Lock Remap` 을
    //           켰거나 caps lock 프리셋을 전부 껐다) **반드시 제거한다.**
    //
    // 기대값의 출처: `docs/dev/architecture.md` §6.1 — "되돌릴 수단을 남긴다 …
    // 켜면 경로 B 를 설치하지 않고 경로 A 만 쓴다". 매핑이 남아 있으면 그 스위치는
    // 되돌리는 수단이 되지 못하고, 물리 caps lock 은 F18 로 도착한 채 아무 규칙에도
    // 매칭되지 않아 앱 안에서 복구할 수 없다.
    #[test]
    fn reconcile_removes_our_own_residual_mapping_when_no_longer_desired() {
        let foreign = mapping(0x0000_0007_0000_0004, 0x0000_0007_0000_0005);
        let backend = FakeBackend {
            current: Mutex::new(vec![foreign, d1_mapping()]),
            ..Default::default()
        };
        let mgr = PathBManager::new(Box::new(backend));

        let report = mgr.reconcile_on_start(&[]).unwrap();

        assert!(report.had_residual_mapping);
        assert!(report.cleared, "우리 매핑을 실제로 걷어냈다고 보고해야 한다");
        let left = mgr.backend.read_current().unwrap();
        assert_eq!(left, vec![foreign], "남의 매핑은 그대로 남아야 한다(부록 A #3)");
    }

    // 5-quinquies. desired 를 설치할 때도 남의 매핑을 보존한다.
    #[test]
    fn reconcile_with_desired_preserves_foreign_mappings() {
        let foreign = mapping(0x0000_0007_0000_0004, 0x0000_0007_0000_0005);
        let backend = FakeBackend {
            current: Mutex::new(vec![foreign]),
            ..Default::default()
        };
        let mgr = PathBManager::new(Box::new(backend));

        mgr.reconcile_on_start(&[d1_mapping()]).unwrap();

        let left = mgr.backend.read_current().unwrap();
        assert!(left.contains(&foreign), "남의 매핑이 사라지면 안 된다(부록 A #3)");
        assert!(left.contains(&d1_mapping()), "이번 설정이 요구한 D-1 매핑은 설치돼야 한다");
    }

    // 6. reconcile_on_start — 잔존 매핑이 없는데 desired 가 이미 채워져 있으면(D-1,
    //    이전 실행에서 켠 caps lock 프리셋을 이번 부팅에도 그대로 쓰는 경우) 소유권
    //    판별 문제 없이 곧바로 설치한다.
    #[test]
    fn reconcile_with_no_residual_but_nonempty_desired_installs_it() {
        let apply_calls = Arc::new(AtomicUsize::new(0));
        let backend = FakeBackend {
            apply_calls: apply_calls.clone(),
            ..Default::default()
        };
        let mgr = PathBManager::new(Box::new(backend));

        let desired = vec![mapping(HID_USAGE_CAPS_LOCK, HID_USAGE_F18)];
        let report = mgr.reconcile_on_start(&desired).unwrap();
        assert!(!report.had_residual_mapping);
        assert!(report.residual.is_empty());
        assert_eq!(apply_calls.load(Ordering::SeqCst), 1);
    }

    // 7. desired_mappings_for() — D-1 판정.
    #[test]
    fn desired_mappings_for_none_alias_is_empty() {
        let cfg = EngineConfig::default();
        assert!(desired_mappings_for(&cfg).is_empty());
    }

    #[test]
    fn desired_mappings_for_f18_alias_installs_caps_lock_to_f18() {
        let cfg = EngineConfig { caps_lock_alias: Some(KeyCode::F18), ..Default::default() };
        assert_eq!(
            desired_mappings_for(&cfg),
            vec![KeyMapping { src: HID_USAGE_CAPS_LOCK, dst: HID_USAGE_F18 }]
        );
    }

    #[test]
    fn desired_mappings_for_non_f18_alias_is_empty_and_defensive() {
        let cfg = EngineConfig { caps_lock_alias: Some(KeyCode::ESCAPE), ..Default::default() };
        assert!(desired_mappings_for(&cfg).is_empty());
    }
}
