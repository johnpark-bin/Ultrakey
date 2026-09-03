//! 경로 B — IOHID 커널 레벨 매핑, 1차 구현은 `hidutil` 서브프로세스.
//!
//! ⭐ **`hidutil` 서브프로세스를 1차 구현으로 채택한다.** 이는
//! `key-remapping-engine.md` §7 의 "FFI 1차" 판정을 뒤집는 것이며, 근거는
//! `docs/dev/architecture.md` §3 "결정 1" 에 있다 — 요약: `IOHIDEventSystemClientCreateSimpleClient`/
//! `IOHIDServiceClientSetProperty` 는 공개 헤더에 없는 심볼이고 시그니처를
//! 검증할 수 없다(`platform-constraints.md` P6 이 `MultitouchSupport` 에
//! 남긴 것과 같은 경고). **잘못된 시그니처의 `extern "C"` 호출은 컴파일도
//! 통과하고 테스트도 통과하다가 임의 시점에 UB 를 낸다** — 그래서 이
//! 크레이트는 그 함수들을 추측 시그니처로 선언하지 않는다.
//!
//! 이 모듈은 `std::process::Command` 만 쓰므로 `unsafe` 가 전혀 없고, macOS
//! 가 아닌 타깃에서도(런타임에 `hidutil` 실행이 실패할 뿐) 그대로 컴파일된다.
//!
//! ⚠️ 셸을 거치지 않는다 — `Command::new("hidutil").arg(...)` 로 인자를 직접
//! 넘겨 셸 인용(quoting) 오류를 원천 차단한다(`key-remapping-engine.md` §7 이
//! 지적한 원본의 `bash command error` 리스크 대응).
//!
//! ⭐ **F-17(`per-device-settings.md`) 확장 — 모든 쓰기가 디바이스 한정이다.**
//! 스파이크(`docs/research/per-device-hid-spike.md`) S-3·S-6 이 실측으로 확정한
//! 대로, VID 단독 매칭·매칭 없는 `--set` 은 사실상 전역 쓰기가 되어 다른
//! 디바이스의 배열을 통째로 갈아치운다. 그래서 [`HidMappingBackend`] 의 모든
//! 메서드가 [`DeviceMatch`]`{vendor_id, product_id}` 를 **필수 인자로** 받는다
//! — VID 단독 매칭이 타입 수준에서 표현 불가능해진다(CONTRACT.md D-17-1).
//! ⭐ 이슈 #110 — 실제 매칭 사전은 여기에 `PrimaryUsagePage:1`/`PrimaryUsage:6` 을
//! 항상 더한 4키다(`matching_json`): VID/PID 프로퍼티가 없는 내장 키보드(`0:0`,
//! 스파이크 S-10)를 키보드 서비스 하나로 정확히 잡고, `IOHIDSystem`(usage 65280/23)
//! 을 PID 와 무관하게 배제한다. 쓰기 뒤에는 되읽기로 실렸는지 확인한다
//! (`HidMappingError::NotApplied`, `ultrakey_engine::path_b::verify_readback`).
//! 유일한 예외는 [`HidutilBackend::migrate_clear_global_d1`] 이며, 그것이 이
//! 코드베이스 전체에서 **유일하게 허용된 매칭 없는 `--set`** 이다 — 그래서
//! 트레이트가 아니라 구체 타입에만 둔다(CONTRACT.md D-17-5).

use std::process::Command;

/// `HIDKeyboardModifierMappingSrc`/`Dst` 한 쌍, `DeviceMatch`, `DeviceInfo` —
/// 전부 `ultrakey-core::perdevice` 의 순수 자료형이다. F-17 이 여러 크레이트
/// (`ultrakey-core`/`ultrakey-platform`/`ultrakey-engine`)에 걸쳐 같은 값
/// 타입을 공유해야 하므로 여기서 새로 정의하지 않고 재수출한다 — 기존
/// `hid_mapping::KeyMapping` 을 참조하던 호출부를 깨지 않는다.
pub use ultrakey_core::perdevice::{DeviceInfo, DeviceMatch, KeyMapping};

#[derive(Debug, thiserror::Error)]
pub enum HidMappingError {
    #[error("hidutil 프로세스를 실행할 수 없다: {0}")]
    Spawn(std::io::Error),
    #[error("hidutil 이 실패 상태로 종료했다(exit={status:?}): {stderr}")]
    NonZeroExit { status: Option<i32>, stderr: String },
    #[error("hidutil 출력 형식을 해석할 수 없다 — 원본 출력: {raw}")]
    ParseFailed { raw: String },
    #[error("hidutil 출력이 UTF-8 이 아니다")]
    InvalidUtf8,
    /// D-17-4 — 쓰기 직전 검증 게이트. 되읽기 성공을 동작 확인으로 쓰지 않는
    /// 것처럼(스파이크 S-7), IOHID 자체가 값을 전혀 검증하지 않으므로 우리가
    /// 쓰기 전에 거부해야 한다. 이 오류가 나면 **쓰지 않는다**.
    #[error("디바이스 한정 매핑 검증 실패: {reason}")]
    InvalidMapping { reason: String },
    /// ⭐ 이슈 #110 — `--set` 이 exit 0 으로 끝났지만 **되읽기에 우리 매핑이 없다.**
    /// `hidutil` 은 매칭 사전이 아무 서비스도 잡지 못해도 조용히 성공으로 끝나므로
    /// (이 기기의 내장 키보드가 정확히 그랬다 — 원장에는 "설치됨"으로 남고 커널에는
    /// 없었다), 쓰기 직후 `read_current` 로 우리 항목이 실제로 그 디바이스에
    /// 실렸는지 확인해야 한다. `matched_services == 0` 이면 매칭 사전이 디바이스를
    /// 못 잡은 것이고, 아니면 어느 서비스의 배열에 `missing` 이 빠져 있는 것이다.
    /// ⚠️ 이것은 S-7("되읽기 성공 = 동작 확인" 금지)과 어긋나지 않는다 — 여기서
    /// 되읽기는 "그 디바이스에 실렸는가"만 확인하고, 동작 확인은 여전히 수동 검증이다.
    #[error(
        "쓰기 후 되읽기에 우리 매핑이 없다(device={device}, matched_services={matched_services}, missing={missing:?})"
    )]
    NotApplied {
        device: String,
        matched_services: usize,
        missing: Vec<KeyMapping>,
    },
}

/// 한 디바이스에 대한 `--matching {...} --get UserKeyMapping` 되읽기 결과.
///
/// 스파이크 S-2 — 물리 디바이스 1개가 IOHID 서비스 여러 개다. `services` 는
/// `(RegistryID, 그 서비스의 배열)` 목록 그대로이고, `aggregated` 는 그 전체의
/// **합집합**(D-17-3)이다. `partial` 은 서비스마다 값이 다를 때(부분 실패,
/// §3.6 규칙 7) `true` 다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceMappingRead {
    pub services: Vec<(u64, Vec<KeyMapping>)>,
    pub aggregated: Vec<KeyMapping>,
    pub partial: bool,
}

/// 경로 B 백엔드 — 디바이스 한정 조회/적용/정리를 추상화한다.
///
/// ⭐ **모든 메서드가 `device: &DeviceMatch` 를 받는다** — VID 단독 매칭·매칭
/// 없는 `--set` 을 이 트레이트로는 표현할 수 없다(D-17-1). 유일한 예외
/// ([`HidutilBackend::migrate_clear_global_d1`])는 의도적으로 이 트레이트
/// 밖에 둔다.
pub trait HidMappingBackend: Send + Sync {
    fn read_current(&self, device: &DeviceMatch) -> Result<DeviceMappingRead, HidMappingError>;
    fn apply(&self, device: &DeviceMatch, mappings: &[KeyMapping]) -> Result<(), HidMappingError>;
    fn clear(&self, device: &DeviceMatch) -> Result<(), HidMappingError>;
}

/// D-17-4 규칙 3 — 목적지 usage 가 허용되는 page 목록. `(page << 32) | usage`
/// 인코딩(CONTRACT.md §0.3)에서 `page = value >> 32` 로 뽑는다.
/// `0x07`=Keyboard, `0x0C`=Consumer(스파이크 S-8 실측 동작 확인),
/// `0xFF`/`0xFF01`=Apple 벤더 정의(예: Globe/fn, §2.1 `(미확정)` 항목들의 여지).
const ALLOWED_USAGE_PAGES: [u64; 4] = [0x07, 0x0C, 0xFF, 0xFF01];

/// D-17-4 규칙 5 — 배열 길이 상한(폭주 방어).
const MAX_MAPPING_LEN: usize = 64;

/// `hidutil` 바이너리를 서브프로세스로 실행하는 1차 구현체.
pub struct HidutilBackend;

impl HidutilBackend {
    fn run(&self, args: &[&str]) -> Result<String, HidMappingError> {
        let output = Command::new("hidutil")
            .args(args)
            .output()
            .map_err(HidMappingError::Spawn)?;

        if !output.status.success() {
            return Err(HidMappingError::NonZeroExit {
                status: output.status.code(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            });
        }

        String::from_utf8(output.stdout).map_err(|_| HidMappingError::InvalidUtf8)
    }

    /// `--matching '{"VendorID":…,"ProductID":…,"PrimaryUsagePage":1,"PrimaryUsage":6}'`
    /// 의 JSON 문자열(스파이크 S-1·S-3·S-10).
    ///
    /// ⚠️ VID 단독 매칭은 `IOHIDSystem`(0x5ac:0x0)을 함께 잡아 사실상 전역
    /// 쓰기가 된다(S-3) — `DeviceMatch` 의 두 필드가 모두 필수이므로 이 함수는
    /// 항상 둘 다 넣는다.
    ///
    /// ⭐ **이슈 #110 — usage 두 키를 항상 함께 넣는다.** MacBook Air 내장 키보드는
    /// VID/PID 프로퍼티가 없어 `0:0` 으로 열거되는데(S-10), `{"VendorID":0,
    /// "ProductID":0}` 만으로는 어떤 서비스가 잡힐지 보증할 수 없다. `PrimaryUsagePage
    /// 1`/`PrimaryUsage 6`(키보드 컬렉션, 열거 필터 `hotplug.rs::
    /// build_keyboard_matching_dict` 와 동일)을 더하면 (a) 내장 키보드는 정확히
    /// 그 키보드 서비스 1행(`0x100000b4a`, 실측)만 잡히고, (b) `IOHIDSystem`(usage
    /// 65280/23)은 PID 와 무관하게 원리적으로 배제된다 — S-3 방어가 VID+PID 에
    /// 더해 usage 로 한 겹 더 생긴다. 외장 키보드(S-2, 서비스 3개)에서는 키보드
    /// 컬렉션 서비스 1개에만 쓰게 된다 — 키보드 페이지 src 의 리매핑을 적용하는
    /// 것은 그 서비스이므로 충분하다 `(추정 — F108Pro 미부착으로 이 세션에서
    /// 실측하지 못함, docs/research/per-device-hid-spike.md S-10)`.
    fn matching_json(device: &DeviceMatch) -> String {
        format!(
            "{{\"VendorID\":{},\"ProductID\":{},\"PrimaryUsagePage\":1,\"PrimaryUsage\":6}}",
            device.vendor_id, device.product_id
        )
    }

    fn set_mapping_json(mappings: &[KeyMapping]) -> String {
        let entries: Vec<String> = mappings
            .iter()
            .map(|m| {
                format!(
                    "{{\"HIDKeyboardModifierMappingSrc\":{},\"HIDKeyboardModifierMappingDst\":{}}}",
                    m.src, m.dst
                )
            })
            .collect();
        format!("{{\"UserKeyMapping\":[{}]}}", entries.join(","))
    }

    /// D-17-5 — 구버전이 남긴 **전역** D-1 잔재를 1회 걷어낸다.
    ///
    /// 이슈 #19 머지 이후에도 `HidutilBackend::apply()` 가 매칭 없이 `--set` 을
    /// 불러 왔다(스파이크 §6 이 실측한 그 코드다) — 그래서 이 워크트리를 포함해
    /// 실제 사용자 환경에 `caps_lock → F18` 이 **전역**으로 남아 있을 수 있다.
    /// `reconcile_on_start` 의 가장 첫 단계에서 정확히 한 번, 전역 배열을 읽어
    /// `d1` 서명과 일치하는 항목만 제거하고 나머지(남의 매핑)는 그대로 다시
    /// 쓴다. 서명이 없으면 아무것도 하지 않는다 — 남의 전역 매핑을 건드리지
    /// 않는다(§3.6 규칙 6, PR #23 의 소유권 원칙).
    ///
    /// ⛔ **이것이 이 코드베이스에서 유일하게 허용된 매칭 없는 `--set` 이다.**
    /// 트레이트에 넣지 않고 이 구체 타입에만 둔 이유는 기능 경로(디바이스별
    /// 합성·재조정)에서 실수로도 호출될 수 없게 하기 위해서다.
    pub fn migrate_clear_global_d1(&self, d1: KeyMapping) -> Result<bool, HidMappingError> {
        let raw = self.run(&["property", "--get", "UserKeyMapping"])?;
        let current = parse_hidutil_mapping_output(&raw)?;
        let Some(remaining) = filter_out_signature(current, d1) else {
            return Ok(false);
        };
        let json = Self::set_mapping_json(&remaining);
        self.run(&["property", "--set", &json])?;
        Ok(true)
    }
}

impl HidMappingBackend for HidutilBackend {
    fn read_current(&self, device: &DeviceMatch) -> Result<DeviceMappingRead, HidMappingError> {
        let matching = Self::matching_json(device);
        let raw = self.run(&[
            "property",
            "--matching",
            &matching,
            "--get",
            "UserKeyMapping",
        ])?;
        let services = parse_hidutil_matched_output(&raw)?;
        let (aggregated, partial) = aggregate_services(&services);
        Ok(DeviceMappingRead {
            services,
            aggregated,
            partial,
        })
    }

    fn apply(&self, device: &DeviceMatch, mappings: &[KeyMapping]) -> Result<(), HidMappingError> {
        validate_device_mappings(device, mappings)?;
        let matching = Self::matching_json(device);
        let json = Self::set_mapping_json(mappings);
        self.run(&["property", "--matching", &matching, "--set", &json])
            .map(|_| ())
    }

    fn clear(&self, device: &DeviceMatch) -> Result<(), HidMappingError> {
        self.apply(device, &[])
    }
}

/// D-17-4 — 쓰기 직전 검증 게이트. 실패하면 거부하고(`InvalidMapping`) 쓰지
/// 않는다. `apply`(트레이트 구현) 진입 즉시 부른다.
fn validate_device_mappings(
    device: &DeviceMatch,
    mappings: &[KeyMapping],
) -> Result<(), HidMappingError> {
    // 규칙 2 — `0x5ac:0x0` 은 `IOHIDSystem` 의사 디바이스다(스파이크 S-3) — 하드
    // 가드로 거부한다. ⭐ 이슈 #110 — 옛 형태(`product_id == 0` 전체 거부)는 VID/PID
    // 프로퍼티가 없어 `0:0` 으로 열거되는 내장 키보드(S-10)까지 막았다. 정본 판정은
    // `ultrakey_core::perdevice::validate` 와 같다.
    if ultrakey_core::perdevice::is_iohidsystem(device) {
        return Err(HidMappingError::InvalidMapping {
            reason: format!(
                "0x{:x}:0x{:x} 는 IOHIDSystem 의사 디바이스라 거부한다(스파이크 S-3)",
                device.vendor_id, device.product_id
            ),
        });
    }

    // 규칙 5 — 폭주 방어.
    if mappings.len() > MAX_MAPPING_LEN {
        return Err(HidMappingError::InvalidMapping {
            reason: format!(
                "배열 길이 {}가 상한 {MAX_MAPPING_LEN}을 넘는다",
                mappings.len()
            ),
        });
    }

    // 규칙 3·4 — usage 값이 닫힌 어휘 범위 안에 있고, 중복 src 가 없다.
    let mut seen_src = std::collections::HashSet::with_capacity(mappings.len());
    for m in mappings {
        validate_usage(m.src, "src")?;
        validate_usage(m.dst, "dst")?;
        if !seen_src.insert(m.src) {
            return Err(HidMappingError::InvalidMapping {
                reason: format!("src 0x{:x} 가 배열 안에 중복된다", m.src),
            });
        }
    }
    Ok(())
}

/// `(page << 32) | usage` 인코딩 최종 수치 게이트(D-17-4 규칙 3).
fn validate_usage(value: u64, field: &str) -> Result<(), HidMappingError> {
    let page = value >> 32;
    let usage = value & 0xFFFF_FFFF;
    if !ALLOWED_USAGE_PAGES.contains(&page) {
        return Err(HidMappingError::InvalidMapping {
            reason: format!(
                "{field} 0x{value:x} 의 usage page 0x{page:x} 가 허용 목록(0x07,0x0C,0xFF,0xFF01) 밖이다"
            ),
        });
    }
    if usage > 0xFFFF {
        return Err(HidMappingError::InvalidMapping {
            reason: format!("{field} 0x{value:x} 의 usage 0x{usage:x} 가 0xFFFF 를 넘는다"),
        });
    }
    Ok(())
}

/// [`HidutilBackend::migrate_clear_global_d1`] 의 순수 부분 — 서명이 `current`
/// 안에 없으면 `None`(변경 없음), 있으면 그 항목만 뺀 나머지를 `Some` 으로
/// 돌려준다. 서브프로세스를 부르지 않아 단위 테스트가 가능하다.
fn filter_out_signature(
    current: Vec<KeyMapping>,
    signature: KeyMapping,
) -> Option<Vec<KeyMapping>> {
    if !current.contains(&signature) {
        return None;
    }
    Some(current.into_iter().filter(|m| *m != signature).collect())
}

/// D-17-3 — 서비스별 배열의 합집합과 부분 불일치(`partial`) 판정.
fn aggregate_services(services: &[(u64, Vec<KeyMapping>)]) -> (Vec<KeyMapping>, bool) {
    let mut aggregated: Vec<KeyMapping> = Vec::new();
    for (_, mappings) in services {
        for m in mappings {
            if !aggregated.contains(m) {
                aggregated.push(*m);
            }
        }
    }
    let partial = match services.split_first() {
        Some((first, rest)) => rest.iter().any(|(_, m)| m != &first.1),
        None => false,
    };
    (aggregated, partial)
}

/// `hidutil property --get UserKeyMapping` 의 실제 출력 형식(실측, 이
/// 워크트리에서 직접 실행해 확인함)을 파싱한다.
///
/// 이 출력은 JSON 이 아니라 **OpenStep/NeXTSTEP 스타일 plist 텍스트**다:
/// ```text
/// (null)                      // 프로퍼티가 아예 설정된 적 없음
/// (
/// )                           // 빈 배열로 설정됨
/// (
///         {
///         HIDKeyboardModifierMappingDst = 30064771129;
///         HIDKeyboardModifierMappingSrc = 30064771129;
///     }
/// )
/// ```
/// `plist` 크레이트는 이 크레이트의 의존성에 없고(XML/바이너리 plist 만
/// 다루며 이 텍스트 방언은 지원 범위 밖이다), 스펙이 요구하는 필드 두
/// 개(u64 정수)만 뽑으면 되므로 여기서는 간단한 수기 파서만 둔다. 실패는
/// 조용히 빈 벡터를 반환하지 않고 원본 출력을 담아 `Err` 로 정확히
/// 전달한다.
///
/// ⚠️ **매칭 없는(`--matching` 없는) `--get` 의 출력 모양**이다 —
/// [`migrate_clear_global_d1`](HidutilBackend::migrate_clear_global_d1) 이
/// 유일하게 이 모양을 읽는다. `--matching {...}` 를 붙인 출력은 서비스마다
/// 행이 나오는 다른 모양이라 [`parse_hidutil_matched_output`] 을 쓴다.
fn parse_hidutil_mapping_output(raw: &str) -> Result<Vec<KeyMapping>, HidMappingError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed == "(null)" {
        return Ok(Vec::new());
    }

    let mut mappings = Vec::new();
    let mut rest = trimmed;
    while let Some(open) = rest.find('{') {
        let after_open = &rest[open + 1..];
        let close = after_open
            .find('}')
            .ok_or_else(|| HidMappingError::ParseFailed {
                raw: raw.to_string(),
            })?;
        let block = &after_open[..close];

        let src = extract_u64_field(block, "HIDKeyboardModifierMappingSrc").ok_or_else(|| {
            HidMappingError::ParseFailed {
                raw: raw.to_string(),
            }
        })?;
        let dst = extract_u64_field(block, "HIDKeyboardModifierMappingDst").ok_or_else(|| {
            HidMappingError::ParseFailed {
                raw: raw.to_string(),
            }
        })?;
        mappings.push(KeyMapping { src, dst });

        rest = &after_open[close + 1..];
    }
    Ok(mappings)
}

/// `hidutil property --matching '{"VendorID":…,"ProductID":…}' --get UserKeyMapping`
/// 의 실제 출력(CONTRACT.md §0.1, 이 워크트리에서 직접 실측)을 파싱한다.
///
/// 매칭 없는 출력과 형태가 다르다 — 물리 디바이스 1개가 IOHID 서비스 여러 개로
/// 나타나므로(스파이크 S-2), 각 서비스마다 한 "행"이 나온다:
/// ```text
/// RegistryID  Key                   Value
/// 100001251   UserKeyMapping   (
///         {
///         HIDKeyboardModifierMappingDst = 30064771181;
///         HIDKeyboardModifierMappingSrc = 30064771129;
///     }
/// )
/// 100001292   UserKeyMapping   (
/// )
/// ```
/// 각 행은 `<RegistryID(16진)> UserKeyMapping (...)` 로 시작하고, 그 값은 위
/// [`parse_hidutil_mapping_output`] 과 동일한 OpenStep plist 배열이다 — 그래서
/// 이 함수는 행을 나눈 뒤 값 부분을 그 파서에 그대로 넘긴다. 헤더 줄
/// (`RegistryID  Key  Value`)은 행 시작 패턴에 맞지 않아 자연히 걸러진다 —
/// 별도로 건너뛸 필요가 없다. 매칭에 걸리는 디바이스가 없으면 출력이 헤더뿐
/// 이거나 완전히 비어 있다 — 두 경우 다 서비스 0개로 파싱된다.
fn parse_hidutil_matched_output(raw: &str) -> Result<Vec<(u64, Vec<KeyMapping>)>, HidMappingError> {
    let mut services = Vec::new();
    let mut current: Option<(u64, String)> = None;

    for line in raw.lines() {
        if let Some((id, rest)) = parse_service_row_start(line) {
            if let Some((prev_id, buf)) = current.take() {
                services.push((prev_id, parse_hidutil_mapping_output(&buf)?));
            }
            current = Some((id, rest));
        } else if let Some((_, buf)) = current.as_mut() {
            buf.push('\n');
            buf.push_str(line);
        }
        // else: 헤더 줄이거나, 아직 어떤 행도 시작되지 않은 잡음 — 무시한다.
    }
    if let Some((id, buf)) = current.take() {
        services.push((id, parse_hidutil_mapping_output(&buf)?));
    }
    Ok(services)
}

/// 한 줄이 서비스 행의 시작(`<16진 RegistryID> UserKeyMapping ...`)인지
/// 판정한다. 헤더 줄(`RegistryID  Key  Value`)이나 값 블록의 계속 줄
/// (`{`·필드·`}`·`)`)은 이 패턴에 맞지 않아 `None` 을 돌려준다.
fn parse_service_row_start(line: &str) -> Option<(u64, String)> {
    let trimmed = line.trim_start();
    let hex_len = trimmed
        .find(|c: char| !c.is_ascii_hexdigit())
        .unwrap_or(trimmed.len());
    if hex_len == 0 {
        return None;
    }
    let id = u64::from_str_radix(&trimmed[..hex_len], 16).ok()?;
    let rest = trimmed[hex_len..].trim_start();
    let rest = rest.strip_prefix("UserKeyMapping")?;
    Some((id, rest.trim_start().to_string()))
}

fn extract_u64_field(block: &str, key: &str) -> Option<u64> {
    let idx = block.find(key)?;
    let after_key = &block[idx + key.len()..];
    let eq = after_key.find('=')?;
    let after_eq = &after_key[eq + 1..];
    let end = after_eq.find(';').unwrap_or(after_eq.len());
    after_eq[..end].trim().parse::<u64>().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── 매칭 없는 출력(migrate_clear_global_d1 이 유일하게 읽는 모양) ──
    // ⛔ 기존 테스트 4개 — 그대로 유지한다.

    #[test]
    fn parses_null_as_empty() {
        assert_eq!(parse_hidutil_mapping_output("(null)\n").unwrap(), vec![]);
    }

    #[test]
    fn parses_empty_array() {
        assert_eq!(parse_hidutil_mapping_output("(\n)\n").unwrap(), vec![]);
    }

    #[test]
    fn parses_real_hidutil_output() {
        // 이 워크트리에서 `hidutil property --get UserKeyMapping` 을 직접
        // 실행해 확인한 실제 출력 형식(캡슐화된 caps lock 코드 예시).
        let raw = "(\n        {\n        HIDKeyboardModifierMappingDst = 30064771129;\n        HIDKeyboardModifierMappingSrc = 30064771129;\n    }\n)\n";
        let parsed = parse_hidutil_mapping_output(raw).unwrap();
        assert_eq!(
            parsed,
            vec![KeyMapping {
                src: 30064771129,
                dst: 30064771129,
            }]
        );
    }

    #[test]
    fn set_mapping_json_shape() {
        let json = HidutilBackend::set_mapping_json(&[KeyMapping { src: 1, dst: 2 }]);
        assert_eq!(
            json,
            "{\"UserKeyMapping\":[{\"HIDKeyboardModifierMappingSrc\":1,\"HIDKeyboardModifierMappingDst\":2}]}"
        );
    }

    #[test]
    fn matching_json_shape() {
        // §3.2 — 매칭 사전에는 반드시 VendorID 와 ProductID 를 함께 넣고(S-3), 이슈
        // #110 이후로는 키보드 usage 두 키도 항상 함께 넣는다(S-10).
        let json = HidutilBackend::matching_json(&DeviceMatch {
            vendor_id: 0x5ac,
            product_id: 0x24f,
        });
        assert_eq!(
            json,
            "{\"VendorID\":1452,\"ProductID\":591,\"PrimaryUsagePage\":1,\"PrimaryUsage\":6}"
        );
    }

    #[test]
    fn matching_json_for_built_in_keyboard_without_vid_pid_keeps_usage_keys() {
        // 이슈 #110 — 내장 키보드(VID/PID 부재 → 0:0)도 같은 4키 사전이다. 이 사전이
        // 실기기에서 정확히 키보드 서비스 1행을 잡는 것은 S-10-5 실측.
        let json = HidutilBackend::matching_json(&DeviceMatch {
            vendor_id: 0,
            product_id: 0,
        });
        assert_eq!(
            json,
            "{\"VendorID\":0,\"ProductID\":0,\"PrimaryUsagePage\":1,\"PrimaryUsage\":6}"
        );
    }

    // ── 매칭 있는(디바이스 한정) 출력(§0.1, 실측) ──

    /// CONTRACT.md §0.1 이 이 워크트리에서 직접 실측한 출력을 **글자 그대로**
    /// 재현한다(축약·정규화하지 않는다). 서비스 3개 — 둘은 같은 매핑, 하나는
    /// 빈 배열 — 이므로 partial == true 여야 한다(D-17-3, 서비스마다 값이
    /// 다르다는 사실 자체가 §3.6 규칙 7 의 "부분 실패" 판정 대상이다).
    const REAL_MATCHED_OUTPUT: &str = "RegistryID  Key                   Value\n100001251   UserKeyMapping   (\n        {\n        HIDKeyboardModifierMappingDst = 30064771181;\n        HIDKeyboardModifierMappingSrc = 30064771129;\n    }\n)\n10000129f   UserKeyMapping   (\n        {\n        HIDKeyboardModifierMappingDst = 30064771181;\n        HIDKeyboardModifierMappingSrc = 30064771129;\n    }\n)\n100001292   UserKeyMapping   (\n)\n";

    #[test]
    fn parses_real_matched_output_with_header_and_three_services() {
        let services = parse_hidutil_matched_output(REAL_MATCHED_OUTPUT).unwrap();
        assert_eq!(services.len(), 3);

        let expected_mapping = KeyMapping {
            src: 30064771129,
            dst: 30064771181,
        };
        assert_eq!(services[0].0, 0x100001251);
        assert_eq!(services[0].1, vec![expected_mapping]);
        assert_eq!(services[1].0, 0x10000129f);
        assert_eq!(services[1].1, vec![expected_mapping]);
        // 세 번째 행 — 빈 배열 행 `(\n)` 이 섞인 경우.
        assert_eq!(services[2].0, 0x100001292);
        assert_eq!(services[2].1, vec![]);
    }

    #[test]
    fn aggregates_matched_output_as_union_and_flags_partial() {
        let read = {
            let services = parse_hidutil_matched_output(REAL_MATCHED_OUTPUT).unwrap();
            let (aggregated, partial) = aggregate_services(&services);
            DeviceMappingRead {
                services,
                aggregated,
                partial,
            }
        };
        // 합집합 — 서비스 1·2 의 매핑 하나만 있고 중복 제거된다(D-17-3).
        assert_eq!(
            read.aggregated,
            vec![KeyMapping {
                src: 30064771129,
                dst: 30064771181,
            }]
        );
        // 서비스마다 값이 다르다(둘은 매핑 1개, 하나는 0개) — partial == true.
        assert!(read.partial, "서비스 값이 다르면 partial 이 true 여야 한다");
    }

    #[test]
    fn parses_header_only_output_as_no_services() {
        // 매칭에 걸리는 디바이스가 없을 때(§3.2) — 헤더만 있고 행이 없다.
        let raw = "RegistryID  Key                   Value\n";
        let services = parse_hidutil_matched_output(raw).unwrap();
        assert!(services.is_empty());
        let (aggregated, partial) = aggregate_services(&services);
        assert!(aggregated.is_empty());
        assert!(!partial);
    }

    #[test]
    fn parses_completely_empty_output_as_no_services() {
        let services = parse_hidutil_matched_output("").unwrap();
        assert!(services.is_empty());
    }

    #[test]
    fn matched_output_without_partial_when_services_agree() {
        // 서비스 두 개가 완전히 같은 배열을 되읽으면 partial == false 여야 한다.
        let raw = "100001251   UserKeyMapping   (\n        {\n        HIDKeyboardModifierMappingDst = 30064771181;\n        HIDKeyboardModifierMappingSrc = 30064771129;\n    }\n)\n10000129f   UserKeyMapping   (\n        {\n        HIDKeyboardModifierMappingDst = 30064771181;\n        HIDKeyboardModifierMappingSrc = 30064771129;\n    }\n)\n";
        let services = parse_hidutil_matched_output(raw).unwrap();
        let (_aggregated, partial) = aggregate_services(&services);
        assert!(!partial, "서비스 값이 같으면 partial 이 false 여야 한다");
    }

    // ── D-17-4 검증 게이트 ──

    fn device(vendor_id: u32, product_id: u32) -> DeviceMatch {
        DeviceMatch {
            vendor_id,
            product_id,
        }
    }

    #[test]
    fn rejects_product_id_zero_as_iohidsystem_guard() {
        // 스파이크 S-3 — VID 0x5ac/PID 0x0 은 IOHIDSystem 의사 디바이스다.
        let err = validate_device_mappings(&device(0x5ac, 0), &[]).unwrap_err();
        assert!(matches!(err, HidMappingError::InvalidMapping { .. }));
    }

    #[test]
    fn accepts_built_in_keyboard_with_zero_vid_pid() {
        // 이슈 #110 — 내장 키보드는 VID/PID 프로퍼티가 없어 0:0 이다(S-10). 규칙 2 는
        // IOHIDSystem(0x5ac:0x0)만 거부해야 한다.
        assert!(validate_device_mappings(&device(0, 0), &[]).is_ok());
        assert!(validate_device_mappings(&device(0x1234, 0), &[]).is_ok());
    }

    #[test]
    fn rejects_usage_page_outside_closed_vocabulary() {
        // page 0x08 은 D-17-4 허용 목록(0x07,0x0C,0xFF,0xFF01) 밖이다.
        let mapping = KeyMapping {
            src: (0x08u64 << 32) | 0x39,
            dst: (0x07u64 << 32) | 0x3A,
        };
        let err = validate_device_mappings(&device(0x5ac, 0x24f), &[mapping]).unwrap_err();
        assert!(matches!(err, HidMappingError::InvalidMapping { .. }));
    }

    #[test]
    fn rejects_usage_above_0xffff() {
        let mapping = KeyMapping {
            src: (0x07u64 << 32) | 0x1_0000,
            dst: (0x07u64 << 32) | 0x3A,
        };
        let err = validate_device_mappings(&device(0x5ac, 0x24f), &[mapping]).unwrap_err();
        assert!(matches!(err, HidMappingError::InvalidMapping { .. }));
    }

    #[test]
    fn rejects_duplicate_src_within_array() {
        let a = KeyMapping {
            src: (0x07u64 << 32) | 0x39,
            dst: (0x07u64 << 32) | 0x3A,
        };
        let b = KeyMapping {
            src: (0x07u64 << 32) | 0x39,
            dst: (0x07u64 << 32) | 0x3B,
        };
        let err = validate_device_mappings(&device(0x5ac, 0x24f), &[a, b]).unwrap_err();
        assert!(matches!(err, HidMappingError::InvalidMapping { .. }));
    }

    #[test]
    fn rejects_array_longer_than_64() {
        let mappings: Vec<KeyMapping> = (0..65)
            .map(|i| KeyMapping {
                src: (0x07u64 << 32) | i,
                dst: (0x07u64 << 32) | 0x3A,
            })
            .collect();
        let err = validate_device_mappings(&device(0x5ac, 0x24f), &mappings).unwrap_err();
        assert!(matches!(err, HidMappingError::InvalidMapping { .. }));
    }

    #[test]
    fn accepts_well_formed_device_scoped_mapping() {
        // caps lock(0x700000039) → F18(0x70000006D), volume_increment 목적지
        // (0xC000000E9, 스파이크 S-8 실측) — 둘 다 닫힌 어휘 안이다.
        let mappings = vec![
            KeyMapping {
                src: 0x7_0000_0039,
                dst: 0x7_0000_006D,
            },
            KeyMapping {
                src: (0x07u64 << 32) | 0x42,
                dst: 0xC_0000_00E9,
            },
        ];
        assert!(validate_device_mappings(&device(0x5ac, 0x24f), &mappings).is_ok());
    }

    // ── D-17-5 마이그레이션의 순수 필터 로직 ──

    #[test]
    fn filter_out_signature_removes_only_matching_entry() {
        let d1 = KeyMapping {
            src: 0x7_0000_0039,
            dst: 0x7_0000_006D,
        };
        let foreign = KeyMapping { src: 1, dst: 2 };
        let current = vec![d1, foreign];
        let remaining =
            filter_out_signature(current, d1).expect("서명이 있으므로 Some 이어야 한다");
        assert_eq!(remaining, vec![foreign]);
    }

    #[test]
    fn filter_out_signature_is_none_when_absent() {
        let d1 = KeyMapping {
            src: 0x7_0000_0039,
            dst: 0x7_0000_006D,
        };
        let foreign = KeyMapping { src: 1, dst: 2 };
        let remaining = filter_out_signature(vec![foreign], d1);
        assert_eq!(remaining, None, "서명이 없으면 아무것도 하지 않아야 한다");
    }
}
