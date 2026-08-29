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

use std::process::Command;

/// `HIDKeyboardModifierMappingSrc`/`Dst` 한 쌍.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyMapping {
    pub src: u64,
    pub dst: u64,
}

#[derive(Debug, thiserror::Error)]
pub enum HidMappingError {
    #[error("hidutil 프로세스를 실행할 수 없다: {0}")]
    Spawn(std::io::Error),
    #[error("hidutil 이 실패 상태로 종료했다(exit={status:?}): {stderr}")]
    NonZeroExit {
        status: Option<i32>,
        stderr: String,
    },
    #[error("hidutil 출력 형식을 해석할 수 없다 — 원본 출력: {raw}")]
    ParseFailed { raw: String },
    #[error("hidutil 출력이 UTF-8 이 아니다")]
    InvalidUtf8,
}

/// 경로 B 백엔드 — 조회/적용/정리를 추상화한다.
pub trait HidMappingBackend: Send + Sync {
    fn read_current(&self) -> Result<Vec<KeyMapping>, HidMappingError>;
    fn apply(&self, mappings: &[KeyMapping]) -> Result<(), HidMappingError>;
    fn clear(&self) -> Result<(), HidMappingError>;
}

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
}

impl HidMappingBackend for HidutilBackend {
    fn read_current(&self) -> Result<Vec<KeyMapping>, HidMappingError> {
        let raw = self.run(&["property", "--get", "UserKeyMapping"])?;
        parse_hidutil_mapping_output(&raw)
    }

    fn apply(&self, mappings: &[KeyMapping]) -> Result<(), HidMappingError> {
        let json = Self::set_mapping_json(mappings);
        self.run(&["property", "--set", &json]).map(|_| ())
    }

    fn clear(&self) -> Result<(), HidMappingError> {
        self.apply(&[])
    }
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
}
