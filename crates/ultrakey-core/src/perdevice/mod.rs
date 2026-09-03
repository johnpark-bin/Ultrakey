//! F-17 키보드별 설정(`docs/spec/per-device-settings.md`) — 순수 모델 계층.
//!
//! ⭐ 이 모듈은 macOS 에 전혀 의존하지 않는다 — 크레이트 최상단
//! `#![forbid(unsafe_code)]` 와 같은 이유다(`docs/dev/architecture.md` §1: "실제
//! 키보드·`CGEventTap` 없이 전부 검증 가능해야 한다"). 경로 B(`hidutil`)의 실제
//! 읽기/쓰기는 `ultrakey-platform`, 디바이스별 원장·재조정 루프는 `ultrakey-engine`
//! 이 담당한다. 여기서 다루는 것은 다섯 가지뿐이다:
//!
//! 1. HID usage 닫힌 어휘(`usage.rs`) — [`crate::keycode::SourceKey::hid_usage`](이
//!    파일이 아니라 `keycode.rs` 에 있다)와 [`SystemFunction::hid_usage`].
//! 2. 2계층 폴백 해석(§3.3) — [`Tri`], [`PerDeviceSettings`].
//! 3. 합성과 우선순위 중재(§3.6 규칙 4·5) — [`compose`].
//! 4. 쓰기 전 검증(D-17-4) — [`validate`].
//! 5. 원장 직렬화(D-17-2) — [`read_managed_ledger`]/[`write_managed_ledger`].
//!
//! 설계 근거는 이 세션이 확정한 F-17 구현 계약(`CONTRACT.md`, 서브에이전트 공통
//! 토대)을 따른다 — 그 문서의 결정 번호(D-17-N)를 아래 주석이 그대로 인용한다.
//! ⛔ 계약과 명세(`per-device-settings.md`)가 어긋나면 명세가 이긴다.

use std::collections::{BTreeMap, HashMap, HashSet};

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::keycode::SourceKey;
use crate::settings::keys as settings_keys;

pub mod destinations;
pub mod inherit;
pub mod usage;
pub use usage::Evidence;

// ── 공개 자료형(CONTRACT.md §3.1) ───────────────────────────────────────────

/// `(page << 32) | usage` 한 쌍 — 경로 B(`UserKeyMapping`)의 `Src`/`Dst`.
///
/// ⚠️ `ultrakey_platform::hid_mapping::KeyMapping` 과 필드가 동일하다 — CONTRACT.md
/// D-17-1 이 이 타입의 정본을 `ultrakey-core` 로 옮기기로 결정했고, `ultrakey-platform`
/// 쪽은 `pub use ultrakey_core::perdevice::KeyMapping` 으로 재수출해 기존 호출부를
/// 깨지 않는 것으로 되어 있다(그 재수출 자체는 이 작업 범위 밖 — 동시에 진행 중인
/// 다른 작업이 담당한다).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct KeyMapping {
    pub src: u64,
    pub dst: u64,
}

/// 매칭 사전 — `VendorID`+`ProductID` 둘 다 필수(§3.2, 규칙 3). ⭐ 두 필드 모두
/// 필수인 이유는 VID 단독 매칭(스파이크 S-3 — `IOHIDSystem` 을 함께 잡아 사실상
/// 전역 쓰기가 된다)을 **타입으로 표현 불가능**하게 막기 위해서다(D-17-1).
///
/// ⭐ 이슈 #110 — 실제 `hidutil --matching` 사전은 이 두 값에 `PrimaryUsagePage:1`/
/// `PrimaryUsage:6` 을 항상 더한 **4키**다(`ultrakey_platform::hid_mapping::
/// HidutilBackend::matching_json`). 내장 키보드는 VID/PID 프로퍼티가 없어 `(0, 0)`
/// 이다(스파이크 S-10) — 그래서 이 타입은 0 값을 거부하지 않고, `IOHIDSystem`
/// 가드는 [`is_iohidsystem`]`(0x5ac, 0)` 정확 일치로만 건다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DeviceMatch {
    pub vendor_id: u32,
    pub product_id: u32,
}

/// 설정 키·원장에 쓰는 `"<vid>:<pid>"` 소문자 16진(0x 접두사 없음) 식별자(§3.2).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct DeviceId(String);

impl DeviceId {
    /// `(vendor_id, product_id)` 로부터 조립한다. 예: `(0x5ac, 0x24f)` → `"5ac:24f"`.
    pub fn new(vendor_id: u32, product_id: u32) -> Self {
        DeviceId(format!("{vendor_id:x}:{product_id:x}"))
    }

    /// `"<vid>:<pid>"` 문자열을 파싱한다. 구분자가 없거나 양쪽이 16진수가 아니면
    /// `None`.
    pub fn parse(s: &str) -> Option<Self> {
        let (vid, pid) = s.split_once(':')?;
        u32::from_str_radix(vid, 16).ok()?;
        u32::from_str_radix(pid, 16).ok()?;
        Some(DeviceId(s.to_string()))
    }

    /// `DeviceMatch` 로 되돌린다. `new`/`parse` 로만 만들어지므로(불변식) 항상 성공한다.
    pub fn to_match(&self) -> DeviceMatch {
        let (vid, pid) = self
            .0
            .split_once(':')
            .expect("DeviceId 불변식: 항상 <vid>:<pid> 형태로만 구성된다");
        DeviceMatch {
            vendor_id: u32::from_str_radix(vid, 16).expect("파싱 시점에 이미 검증됨"),
            product_id: u32::from_str_radix(pid, 16).expect("파싱 시점에 이미 검증됨"),
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// 열거된 디바이스 하나의 속성(§3.2) — `hidutil list`/IOKit 열거·핫플러그 이벤트가
/// 싣는다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub vendor_id: u32,
    pub product_id: u32,
    pub product_name: Option<String>,
    pub transport: Option<String>,
    /// `(미확정)` — 정확한 IOKit 프로퍼티 키가 스파이크에서 확인되지 않았다
    /// (§3.2, §9 질문 7).
    pub built_in: Option<bool>,
}

/// 저장 계층 **한 층**(디바이스 또는 공통)에 저장된 원시 값의 3상태 해석(§3.3).
///
/// ⚠️ 이 타입은 한 계층의 raw 상태를 나타낸다 — 두 계층을 아우른 최종 해석은
/// [`PerDeviceSettings::resolved_key_remap_rows`]/
/// [`PerDeviceSettings::resolved_function_key`] 가 반환하는 `Vec`/`Option` 이다.
/// UI 는 이 타입으로 "이 디바이스 전용 팝업이 지금 무엇을 보여줘야 하는가"(공통
/// 따름/특정 값/표준 F-키)를 그대로 그릴 수 있다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tri<T> {
    /// 키가 부재 — 상위 계층(공통)을 따른다. 공통 계층에서도 `Inherit` 면 "매핑 없음".
    Inherit,
    /// 명시적 값.
    Value(T),
    /// JSON `null` — 명시적 끔.
    Off,
}

/// 기능 1 행 — `{from, to}`. ⚠️ `Serialize`/`Deserialize` 는 `SourceKey` 의 variant
/// 이름 표현을 그대로 쓴다(`keycode.rs:175` 규약 — 저장 표현을 UI 라벨과 분리해
/// 라벨을 바꿔도 저장된 설정이 깨지지 않게 한다).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyRemapRow {
    pub from: SourceKey,
    pub to: SourceKey,
}

/// 가변 길이 목록 편집기(§3.4.1)의 UI 상태 행 — from/to 가 아직 미선택일 수 있다.
/// ⚠️ 이 타입 자체는 저장되지 않는다 — [`rows_for_storage`] 가 완성된 행만 걸러내
/// [`KeyRemapRow`] 로 바꾼다(§3.4.1 4항: "빈 행은 저장하지 않는다").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DraftKeyRemapRow {
    pub from: Option<SourceKey>,
    pub to: Option<SourceKey>,
}

/// 빈 행(from/to 미선택)을 걸러내고 완성된 행만 남긴다(§3.4.1 4항, §8 수용 기준
/// "빈 행은 저장 파일에 반영되지 않는다").
pub fn rows_for_storage(drafts: &[DraftKeyRemapRow]) -> Vec<KeyRemapRow> {
    drafts
        .iter()
        .filter_map(|d| {
            Some(KeyRemapRow {
                from: d.from?,
                to: d.to?,
            })
        })
        .collect()
}

/// 기능 2 슬롯 — F1~F12 물리 위치(§3.5). ⚠️ `SourceKey::F1`~`F24` 와 달리 F13 이상은
/// 없다 — 기능 2 는 F1~F12 12개 슬롯만 다룬다(명세 §3.5 표, §4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FKey {
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

impl FKey {
    /// 12종 전량, F1→F12 순서.
    pub fn all() -> &'static [FKey] {
        use FKey::*;
        &[F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12]
    }

    /// 저장 키 세그먼트 — `perDevice.<scope>.functionKeys.<segment>`(§3.3).
    pub fn key_segment(self) -> &'static str {
        use FKey::*;
        match self {
            F1 => "f1",
            F2 => "f2",
            F3 => "f3",
            F4 => "f4",
            F5 => "f5",
            F6 => "f6",
            F7 => "f7",
            F8 => "f8",
            F9 => "f9",
            F10 => "f10",
            F11 => "f11",
            F12 => "f12",
        }
    }

    /// 이 물리 F-키 위치에 대응하는 [`SourceKey`] — 합성 시 배열의 `src` 로 쓰인다
    /// (§3.5: "이 디바이스의 F1~F12 각 키가 무엇을 하는가").
    pub fn source_key(self) -> SourceKey {
        use FKey::*;
        match self {
            F1 => SourceKey::F1,
            F2 => SourceKey::F2,
            F3 => SourceKey::F3,
            F4 => SourceKey::F4,
            F5 => SourceKey::F5,
            F6 => SourceKey::F6,
            F7 => SourceKey::F7,
            F8 => SourceKey::F8,
            F9 => SourceKey::F9,
            F10 => SourceKey::F10,
            F11 => SourceKey::F11,
            F12 => SourceKey::F12,
        }
    }
}

/// ⚠️ **이것은 PR #29 가 출하한 *옛* 저장 표현이다 — 새 어휘는 [`destinations`] 다.**
/// 이슈 #31 ② 이전에는 이 12종(그중 [`SystemFunction::hid_usage`] 가 `Some` 인 8종만)
/// 이 기능 2 선택 팝업의 전체 선택지였다. 지금은 **더 이상 UI 선택지가 아니다** —
/// 팝업은 [`destinations::all`] 313종을 쓴다. 이 타입이 남아 있는 이유는 순전히
/// 마이그레이션 때문이다: 옛 버전이 이미 이 variant 이름으로 저장해 둔 값을
/// [`destinations::resolve_stored`] 가 읽을 때 옮겨 줘야 한다(같은 파일
/// `migrate_legacy_system_function`). 새 코드에서 이 타입을 저장 대상으로 쓰지
/// 않는다 — 이름은 F-키 위치가 아니라 기능 정체성으로 잡았었다(§4.1 옛 표) — F1 에
/// `Mute` 를 골라도 라벨이 어긋나지 않게 하려던 설계였다.
///
/// ⚠️ `Serialize`/`Deserialize` 는 variant 이름을 그대로 쓴다 — [`KeyRemapRow`] 와
/// 같은 이유(저장 표현을 §4.1 의 UI 문자열 카탈로그와 분리). 마이그레이션 테스트가
/// 이 직렬화 형태에 의존한다(`destinations.rs` 의
/// `legacy_system_functions_migrate_to_the_same_values`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SystemFunction {
    DisplayBrightnessDown,
    DisplayBrightnessUp,
    MissionControl,
    Spotlight,
    Dictation,
    DoNotDisturb,
    Rewind,
    PlayPause,
    FastForward,
    Mute,
    VolumeDown,
    VolumeUp,
}

impl SystemFunction {
    /// 12종 전량, §4.1 표시 순서 그대로.
    pub fn all() -> &'static [SystemFunction] {
        use SystemFunction::*;
        &[
            DisplayBrightnessDown,
            DisplayBrightnessUp,
            MissionControl,
            Spotlight,
            Dictation,
            DoNotDisturb,
            Rewind,
            PlayPause,
            FastForward,
            Mute,
            VolumeDown,
            VolumeUp,
        ]
    }
}

/// 한 디바이스의 최종 합성 배열과, 규칙 5(우선순위)로 밀려난 항목(§3.6).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Composition {
    pub mappings: Vec<KeyMapping>,
    pub suppressed: Vec<Suppressed>,
}

/// 우선순위(`D-1` > 기능1 > 기능2)에 밀려 배열에 들어가지 못한 항목 — UI 가 "이
/// 디바이스의 다른 규칙에 밀려 비활성" 배지를 붙이는 데 쓴다(§3.6 규칙 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Suppressed {
    Feature1Row(KeyRemapRow),
    Feature2Key(FKey),
}

// ── §3.3 2계층 폴백 해석 ────────────────────────────────────────────────────

/// 설정 저장소가 노출하는 원시 맵(`SettingsStore` 의 `values`)을 감싸, §3.3 의
/// 2계층 폴백(디바이스 → 공통 → 부재)을 이 타입 하나에 캡슐화한다.
///
/// `ultrakey-core` 는 `SettingsStore` 를 그대로 감싼 것이 아니라 그 안의
/// `BTreeMap<String, serde_json::Value>` 를 빌려 쓴다 — 저장 계층(F-15)의 실제
/// 영속화 로직과는 분리된 순수 판정만 여기 있다.
pub struct PerDeviceSettings<'a> {
    values: &'a BTreeMap<String, Value>,
}

impl<'a> PerDeviceSettings<'a> {
    pub fn new(values: &'a BTreeMap<String, Value>) -> Self {
        PerDeviceSettings { values }
    }

    /// 저장 키 하나의 원시 3상태(§3.3). 키가 없으면 `Inherit`, JSON `null` 이면
    /// `Off`, 그 외에는 역직렬화해 `Value`. 타입이 안 맞으면(손상된 값)
    /// `SettingsStore::get()` 과 같은 관용대로 경고만 남기고 `Inherit`(그 계층은
    /// 없는 셈) 취급한다 — 값 하나가 스토어 전체를 무효화하지 않는다.
    fn read_tri<T: DeserializeOwned>(&self, key: &str) -> Tri<T> {
        match self.values.get(key) {
            None => Tri::Inherit,
            Some(v) if v.is_null() => Tri::Off,
            Some(v) => match serde_json::from_value(v.clone()) {
                Ok(value) => Tri::Value(value),
                Err(err) => {
                    tracing::warn!(
                        key,
                        error = %err,
                        "perDevice setting value type didn't match what was expected; treating this layer as absent"
                    );
                    Tri::Inherit
                }
            },
        }
    }

    /// 기능 1 — 디바이스 계층의 원시 3상태(UI 가 이 디바이스 전용 목록 편집기를
    /// 그리는 데 쓴다).
    pub fn key_remap_rows_device(&self, device: &DeviceId) -> Tri<Vec<KeyRemapRow>> {
        self.read_tri(&settings_keys::per_device_key_remap_rows(device.as_str()))
    }

    /// 기능 1 — 공통(`For all devices`) 계층의 원시 3상태.
    pub fn key_remap_rows_common(&self) -> Tri<Vec<KeyRemapRow>> {
        self.read_tri(&settings_keys::per_device_key_remap_rows(
            settings_keys::PER_DEVICE_COMMON_SCOPE,
        ))
    }

    /// 기능 1 — §3.3 최종 해석(디바이스 → 공통 → 없음). [`compose`] 가 쓴다.
    pub fn resolved_key_remap_rows(&self, device: &DeviceId) -> Vec<KeyRemapRow> {
        match self.key_remap_rows_device(device) {
            Tri::Value(rows) => rows,
            Tri::Off => Vec::new(),
            Tri::Inherit => match self.key_remap_rows_common() {
                Tri::Value(rows) => rows,
                Tri::Off | Tri::Inherit => Vec::new(),
            },
        }
    }

    /// 기능 2 — 디바이스 계층의 원시 3상태. ⚠️ 이슈 #31 ② — 저장값은 이제
    /// [`destinations::FunctionDestination::id`] 문자열이다(옛 [`SystemFunction`]
    /// variant 이름도 여전히 읽을 수 있다 — 해석은 [`resolved_function_key`]가 한다).
    /// 이 메서드 자체는 raw 문자열만 돌려준다 — 해석 전이라 아직 어느 어휘인지
    /// 모른다.
    pub fn function_key_device(&self, device: &DeviceId, f: FKey) -> Tri<String> {
        self.read_tri(&settings_keys::per_device_function_key(device.as_str(), f))
    }

    /// 기능 2 — 공통 계층의 원시 3상태.
    pub fn function_key_common(&self, f: FKey) -> Tri<String> {
        self.read_tri(&settings_keys::per_device_function_key(
            settings_keys::PER_DEVICE_COMMON_SCOPE,
            f,
        ))
    }

    /// 기능 2 — §3.3 최종 해석. `None` = 매핑 없음(표준 F-키 그대로 동작이거나,
    /// 저장된 문자열을 해석할 수 없는 경우 — 손상된 설정 하나가 앱을 죽이지 않는다,
    /// [`destinations::resolve_stored`] 와 같은 관용).
    pub fn resolved_function_key(
        &self,
        device: &DeviceId,
        f: FKey,
    ) -> Option<&'static destinations::FunctionDestination> {
        let stored = match self.function_key_device(device, f) {
            Tri::Value(s) => s,
            Tri::Off => return None,
            Tri::Inherit => match self.function_key_common(f) {
                Tri::Value(s) => s,
                Tri::Off | Tri::Inherit => return None,
            },
        };
        let resolved = destinations::resolve_stored(&stored);
        if resolved.is_none() {
            tracing::warn!(
                value = %stored,
                "perDevice function 2 stored value not found in the destination catalog; treating as no mapping"
            );
        }
        resolved
    }
}

// ── §3.6 규칙 4·5 — 합성과 우선순위 중재 ────────────────────────────────────

/// 한 디바이스의 최종 배열 합성(§3.6 규칙 4). 우선순위 **`D-1` > 기능1 > 기능2**
/// (규칙 5) — 같은 `src` 를 뒤에서 요구하면 배열에 넣지 않고 `suppressed` 에 담는다.
///
/// ⚠️ 쓰기 전 **전체를 다시 합성**한다(규칙 4, 증분 쓰기 금지) — 호출할 때마다
/// 새로 계산하고, 이 함수 자체는 상태를 갖지 않는다.
pub fn compose(
    device: &DeviceId,
    settings: &PerDeviceSettings<'_>,
    d1: Option<KeyMapping>,
) -> Composition {
    let mut mappings = Vec::new();
    let mut used_src: HashSet<u64> = HashSet::new();
    let mut suppressed = Vec::new();

    // 1순위 — D-1(caps lock 모멘터리 정규화). 존재하면 무조건 이긴다.
    if let Some(d1) = d1 {
        used_src.insert(d1.src);
        mappings.push(d1);
    }

    // 2순위 — 기능 1(사용자가 명시적으로 만든 변환 세트).
    for row in settings.resolved_key_remap_rows(device) {
        // SourceKey::hid_usage() 는 35종 전부에 대해 Some 이다(keycode.rs) — unwrap 이 안전하다.
        let src = row
            .from
            .hid_usage()
            .expect("SourceKey::hid_usage() 는 35종 전부 Some 이다");
        let dst = row
            .to
            .hid_usage()
            .expect("SourceKey::hid_usage() 는 35종 전부 Some 이다");
        if used_src.insert(src) {
            mappings.push(KeyMapping { src, dst });
        } else {
            suppressed.push(Suppressed::Feature1Row(row));
        }
    }

    // 3순위 — 기능 2(F1~F12 슬롯의 목적지 카탈로그 배치, 이슈 #31 ②).
    for &fkey in FKey::all() {
        // `resolved_function_key` 가 `None` 을 주는 경우는 둘이다: ① 이 F-키에
        // 매핑이 없다(부재/끔) ② 저장값이 있지만 카탈로그가 해석하지 못한다(손상된
        // 설정·옛 버전이 남긴 알 수 없는 문자열). 옛 구현에는 "hid_usage() 가
        // None 인 4종은 애초에 후보를 못 낸다"는 별도 분기가 있었지만, 새 카탈로그는
        // 모든 항목이 값을 가지므로(destinations.rs — 값 없는 목적지는 카탈로그
        // 자체에서 뺐다) 그 분기는 사라졌다 — 여기서는 해석 실패만 같은 자리에서
        // 조용히 건너뛴다. 두 경우 다 밀린 것이 아니므로 suppressed 에 담지 않는다.
        let Some(destination) = settings.resolved_function_key(device, fkey) else {
            continue;
        };
        let dst = destination.value;
        let src = fkey
            .source_key()
            .hid_usage()
            .expect("F1~F12 는 hid_usage() 가 Some 이다");
        if used_src.insert(src) {
            mappings.push(KeyMapping { src, dst });
        } else {
            suppressed.push(Suppressed::Feature2Key(fkey));
        }
    }

    Composition {
        mappings,
        suppressed,
    }
}

/// 같은 계층 안에서 같은 `from` 을 요구하는 행이 있는지 판정한다(§3.4 충돌
/// 대화상자 트리거). 중복된 `from` 목록을 canonical 순서(`SourceKey::all()`)로
/// 돌려준다 — 비어 있으면 중복 없음. 이 함수는 **한 계층의 배열 하나**만 본다 —
/// 디바이스 배열과 공통 배열은 병합되지 않으므로(§3.4) 서로 비교 대상이 아니다.
pub fn find_duplicate_from(rows: &[KeyRemapRow]) -> Vec<SourceKey> {
    let mut counts: HashMap<SourceKey, u32> = HashMap::new();
    for row in rows {
        *counts.entry(row.from).or_insert(0) += 1;
    }
    SourceKey::all()
        .iter()
        .copied()
        .filter(|k| counts.get(k).copied().unwrap_or(0) > 1)
        .collect()
}

// ── D-17-4 — 쓰기 전 검증 ───────────────────────────────────────────────────

/// 배열 길이 상한(폭주 방어) — D-17-4 규칙 5.
pub const MAX_MAPPINGS: usize = 64;

/// D-17-4 규칙 3 이 요구하는 닫힌 어휘의 수치 게이트 — `page ∈ {0x07, 0x0C, 0xFF,
/// 0xFF01}`, `usage <= 0xFFFF`. `0xFF01` 은 현재 이 크레이트의 어떤 값도 실제로
/// 쓰지 않지만, CONTRACT.md D-17-4 규칙 3 이 명시한 허용 집합이라 그대로 반영한다.
const ALLOWED_PAGES: [u64; 4] = [0x07, 0x0C, 0xFF, 0xFF01];

/// `IOHIDSystem` 의사 디바이스(스파이크 S-3 실측: VID `0x5ac` / PID `0x0` /
/// UsagePage 65280 / Usage 23). 여기에 쓰면 사실상 전역 쓰기다.
///
/// ⭐ 이슈 #110 — 판정을 `product_id == 0` 에서 **`(0x5ac, 0)` 정확 일치**로 좁혔다.
/// MacBook Air 내장 키보드는 VID/PID 프로퍼티가 없어 `0:0` 으로 열거되는데
/// (`docs/research/per-device-hid-spike.md` S-10), 옛 판정은 그것까지 막아 D-1 이
/// 내장 키보드에 영원히 설치되지 못했다. `0:0` 과 `0x5ac:0x0` 은 VID 로 구분된다.
pub fn is_iohidsystem(device: &DeviceMatch) -> bool {
    device.vendor_id == 0x5ac && device.product_id == 0
}

/// 검증 실패 — 조용히 넘기지 않고 거부한다(D-17-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    /// 규칙 2 — `IOHIDSystem`(`0x5ac:0x0`)에 대한 쓰기를 막는 하드 가드.
    /// ⭐ 이슈 #110 — `0x5ac:0x0` 정확 일치만 거부한다(내장 키보드 `0:0` 은 통과).
    #[error("device is IOHIDSystem (0x5ac:0x0); writes to the global pseudo-device are forbidden")]
    ZeroProductId,
    /// 규칙 3 — 닫힌 어휘 밖의 usage page.
    #[error("unknown usage page: value={value:#x}, page={page:#x}")]
    UnknownPage { value: u64, page: u64 },
    /// 규칙 3 — usage 가 16비트 범위를 벗어남.
    #[error("usage value out of range: value={value:#x}, usage={usage:#x} (> 0xFFFF)")]
    UsageOutOfRange { value: u64, usage: u64 },
    /// 규칙 4 — 배열 안에 같은 `src` 중복(규칙 5 중재가 대부분 막아 주지만,
    /// 마지막 방어선으로 다시 확인한다).
    #[error("duplicate src (={0:#x}) within the array")]
    DuplicateSrc(u64),
    /// 규칙 5 — 배열 길이 상한 초과.
    #[error("array length exceeds the cap: {len} > {max}")]
    TooManyMappings { len: usize, max: usize },
}

fn validate_usage_value(value: u64) -> Result<(), ValidationError> {
    let page = value >> 32;
    let usage = value & 0xFFFF_FFFF;
    if !ALLOWED_PAGES.contains(&page) {
        return Err(ValidationError::UnknownPage { value, page });
    }
    if usage > 0xFFFF {
        return Err(ValidationError::UsageOutOfRange { value, usage });
    }
    Ok(())
}

/// 쓰기 직전 게이트 — D-17-4 의 5개 규칙 전부(규칙 1 은 [`DeviceMatch`] 의 두 필드가
/// 모두 필수라 타입으로 이미 강제된다 — 여기서는 나머지 4개를 값 수준에서 확인한다).
pub fn validate(device: &DeviceMatch, mappings: &[KeyMapping]) -> Result<(), ValidationError> {
    if is_iohidsystem(device) {
        return Err(ValidationError::ZeroProductId);
    }
    if mappings.len() > MAX_MAPPINGS {
        return Err(ValidationError::TooManyMappings {
            len: mappings.len(),
            max: MAX_MAPPINGS,
        });
    }
    let mut seen_src: HashSet<u64> = HashSet::new();
    for m in mappings {
        validate_usage_value(m.src)?;
        validate_usage_value(m.dst)?;
        if !seen_src.insert(m.src) {
            return Err(ValidationError::DuplicateSrc(m.src));
        }
    }
    Ok(())
}

// ── D-17-2 — 원장(`perDevice._managed`) 직렬화 ─────────────────────────────

/// `perDevice._managed` 원장 — 디바이스 → 직전에 우리가 실제로 쓴 배열(D-17-2).
/// 크래시 안전 2단계 영속화(쓰기 전 상위집합 → 쓰기 후 정확집합)는 이 타입을
/// 쓰는 쪽(`ultrakey-engine`)의 책임이다 — 여기서는 순수 직렬화 형태만 정의한다.
pub type ManagedLedger = BTreeMap<DeviceId, Vec<KeyMapping>>;

/// 저장소에서 읽은 raw JSON(`perDevice._managed` 키의 값)을 원장으로 파싱한다.
/// 형식에 맞지 않는 항목(디바이스 키 파싱 실패·배열 형식 불일치)은 건너뛰고
/// 경고만 남긴다 — 원장 항목 하나의 손상이 전체 원장을 무효화하지 않는다.
pub fn read_managed_ledger(value: &Value) -> ManagedLedger {
    let mut ledger = ManagedLedger::new();
    let Some(obj) = value.as_object() else {
        return ledger;
    };
    for (key, raw) in obj {
        let Some(device) = DeviceId::parse(key) else {
            tracing::warn!(
                key,
                "could not parse a perDevice._managed device key; skipping"
            );
            continue;
        };
        match serde_json::from_value::<Vec<KeyMapping>>(raw.clone()) {
            Ok(mappings) => {
                ledger.insert(device, mappings);
            }
            Err(err) => {
                tracing::warn!(
                    key,
                    error = %err,
                    "perDevice._managed array shape didn't match what was expected; skipping"
                );
            }
        }
    }
    ledger
}

/// 원장을 저장 값(JSON)으로 직렬화한다.
pub fn write_managed_ledger(ledger: &ManagedLedger) -> Value {
    let map: serde_json::Map<String, Value> = ledger
        .iter()
        .map(|(id, mappings)| {
            (
                id.as_str().to_string(),
                serde_json::to_value(mappings).expect("KeyMapping 직렬화는 실패하지 않는다"),
            )
        })
        .collect();
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn device() -> DeviceId {
        DeviceId::new(0x5ac, 0x24f)
    }

    fn row(from: SourceKey, to: SourceKey) -> KeyRemapRow {
        KeyRemapRow { from, to }
    }

    // ── §3.3 — 3상태 폴백 3가지 전부 × 기능 1·기능 2 ────────────────────────────

    // 기능 1 — 부재(디바이스) = 공통 따름(값이 있을 때).
    #[test]
    fn feature1_absent_device_falls_back_to_common_value() {
        let common_key =
            settings_keys::per_device_key_remap_rows(settings_keys::PER_DEVICE_COMMON_SCOPE);
        let values = BTreeMap::from([(
            common_key,
            serde_json::to_value(vec![row(SourceKey::CapsLock, SourceKey::F18)]).unwrap(),
        )]);
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(
            settings.resolved_key_remap_rows(&device()),
            vec![row(SourceKey::CapsLock, SourceKey::F18)]
        );
    }

    // 기능 1 — 공통도 부재면 매핑 없음.
    #[test]
    fn feature1_absent_at_both_layers_means_no_mapping() {
        let values = BTreeMap::new();
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(settings.resolved_key_remap_rows(&device()), Vec::new());
    }

    // 기능 1 — 디바이스 계층의 명시적 값이 공통을 덮는다.
    #[test]
    fn feature1_device_value_overrides_common() {
        let dev = device();
        let device_key = settings_keys::per_device_key_remap_rows(dev.as_str());
        let common_key =
            settings_keys::per_device_key_remap_rows(settings_keys::PER_DEVICE_COMMON_SCOPE);
        let values = BTreeMap::from([
            (
                common_key,
                serde_json::to_value(vec![row(SourceKey::CapsLock, SourceKey::F18)]).unwrap(),
            ),
            (
                device_key,
                serde_json::to_value(vec![row(SourceKey::LeftOption, SourceKey::LeftCommand)])
                    .unwrap(),
            ),
        ]);
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(
            settings.resolved_key_remap_rows(&dev),
            vec![row(SourceKey::LeftOption, SourceKey::LeftCommand)]
        );
    }

    // 기능 1 — 디바이스 계층 JSON null = 명시적 끔(공통에 값이 있어도 이긴다).
    #[test]
    fn feature1_device_null_is_explicit_off_even_if_common_has_value() {
        let dev = device();
        let device_key = settings_keys::per_device_key_remap_rows(dev.as_str());
        let common_key =
            settings_keys::per_device_key_remap_rows(settings_keys::PER_DEVICE_COMMON_SCOPE);
        let values = BTreeMap::from([
            (
                common_key,
                serde_json::to_value(vec![row(SourceKey::CapsLock, SourceKey::F18)]).unwrap(),
            ),
            (device_key, Value::Null),
        ]);
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(settings.resolved_key_remap_rows(&dev), Vec::new());
    }

    // 기능 2 — 부재(디바이스) = 공통 따름(값이 있을 때).
    // ⚠️ 저장값은 옛 SystemFunction variant 이름("Mute") 그대로 둔다 — 마이그레이션
    // 경로(destinations::resolve_stored)가 여전히 그것을 읽어낸다는 것을 이 테스트가
    // 증명한다(이슈 #31 ②, PR #29 저장값 호환).
    #[test]
    fn feature2_absent_device_falls_back_to_common_value() {
        let common_key = settings_keys::per_device_function_key(
            settings_keys::PER_DEVICE_COMMON_SCOPE,
            FKey::F9,
        );
        let values = BTreeMap::from([(
            common_key,
            serde_json::to_value(SystemFunction::Mute).unwrap(),
        )]);
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(
            settings.resolved_function_key(&device(), FKey::F9),
            destinations::find("consumer.mute")
        );
    }

    // 기능 2 — 공통도 부재면 매핑 없음(표준 F-키).
    #[test]
    fn feature2_absent_at_both_layers_means_no_mapping() {
        let values = BTreeMap::new();
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(settings.resolved_function_key(&device(), FKey::F9), None);
    }

    // 기능 2 — 디바이스 계층의 명시적 값이 공통을 덮는다.
    #[test]
    fn feature2_device_value_overrides_common() {
        let dev = device();
        let device_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F9);
        let common_key = settings_keys::per_device_function_key(
            settings_keys::PER_DEVICE_COMMON_SCOPE,
            FKey::F9,
        );
        let values = BTreeMap::from([
            (
                common_key,
                serde_json::to_value(SystemFunction::Mute).unwrap(),
            ),
            (
                device_key,
                serde_json::to_value(SystemFunction::VolumeUp).unwrap(),
            ),
        ]);
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(
            settings.resolved_function_key(&dev, FKey::F9),
            destinations::find("consumer.volume_increment")
        );
    }

    // 기능 2 — 디바이스 계층 JSON null = 명시적 끔(공통에 값이 있어도 이긴다).
    #[test]
    fn feature2_device_null_is_explicit_off_even_if_common_has_value() {
        let dev = device();
        let device_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F9);
        let common_key = settings_keys::per_device_function_key(
            settings_keys::PER_DEVICE_COMMON_SCOPE,
            FKey::F9,
        );
        let values = BTreeMap::from([
            (
                common_key,
                serde_json::to_value(SystemFunction::Mute).unwrap(),
            ),
            (device_key, Value::Null),
        ]);
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(settings.resolved_function_key(&dev, FKey::F9), None);
    }

    // ⭐ 이슈 #31 ② 회귀 방지 — 새 카탈로그 id 로 저장한 값도 정확히 해석된다.
    #[test]
    fn feature2_resolves_new_catalog_id() {
        let dev = device();
        let device_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F1);
        let values = BTreeMap::from([(device_key, Value::String("consumer.mute".to_string()))]);
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(
            settings.resolved_function_key(&dev, FKey::F1),
            destinations::find("consumer.mute")
        );
    }

    // ⭐ 손상된 설정(모르는 문자열)이 앱을 죽이지 않는다 — 매핑 없음으로 취급.
    #[test]
    fn feature2_unknown_stored_id_resolves_to_no_mapping_without_panicking() {
        let dev = device();
        let device_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F1);
        let values = BTreeMap::from([(device_key, Value::String("nope.not_a_key".to_string()))]);
        let settings = PerDeviceSettings::new(&values);

        assert_eq!(settings.resolved_function_key(&dev, FKey::F1), None);
    }

    // ── §3.6 규칙 4 — 합성이 서로를 지우지 않는다 ────────────────────────────────

    #[test]
    fn compose_keeps_all_non_conflicting_entries() {
        let dev = device();
        let device_rows_key = settings_keys::per_device_key_remap_rows(dev.as_str());
        let f9_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F9);
        let f10_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F10);
        let f11_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F11);
        let values = BTreeMap::from([
            (
                device_rows_key,
                serde_json::to_value(vec![
                    row(SourceKey::LeftOption, SourceKey::LeftCommand),
                    row(SourceKey::LeftCommand, SourceKey::LeftOption),
                ])
                .unwrap(),
            ),
            (f9_key, serde_json::to_value(SystemFunction::Mute).unwrap()),
            (
                f10_key,
                serde_json::to_value(SystemFunction::VolumeDown).unwrap(),
            ),
            (
                f11_key,
                serde_json::to_value(SystemFunction::VolumeUp).unwrap(),
            ),
        ]);
        let settings = PerDeviceSettings::new(&values);
        let d1 = Some(KeyMapping {
            src: SourceKey::CapsLock.hid_usage().unwrap(),
            dst: SourceKey::F18.hid_usage().unwrap(),
        });

        // D-1(1) + 기능1 2행 + 기능2 3키 — 전부 서로 다른 src 이므로 6개가 전부 남는다.
        let composition = compose(&dev, &settings, d1);

        assert_eq!(composition.mappings.len(), 6, "{composition:?}");
        assert!(composition.suppressed.is_empty(), "{composition:?}");
    }

    // ── §3.6 규칙 5 — 우선순위 D-1 > 기능1 > 기능2 ──────────────────────────────

    #[test]
    fn compose_priority_d1_over_feature1() {
        let dev = device();
        let device_rows_key = settings_keys::per_device_key_remap_rows(dev.as_str());
        // 기능 1 이 caps lock 을 명시적으로 다른 곳으로 재배정 — D-1 도 caps lock 을 쓴다.
        let conflicting_row = row(SourceKey::CapsLock, SourceKey::LeftControl);
        let values = BTreeMap::from([(
            device_rows_key,
            serde_json::to_value(vec![conflicting_row]).unwrap(),
        )]);
        let settings = PerDeviceSettings::new(&values);
        let d1_mapping = KeyMapping {
            src: SourceKey::CapsLock.hid_usage().unwrap(),
            dst: SourceKey::F18.hid_usage().unwrap(),
        };

        let composition = compose(&dev, &settings, Some(d1_mapping));

        assert_eq!(composition.mappings, vec![d1_mapping]);
        assert_eq!(
            composition.suppressed,
            vec![Suppressed::Feature1Row(conflicting_row)]
        );
    }

    #[test]
    fn compose_priority_feature1_over_feature2() {
        let dev = device();
        let device_rows_key = settings_keys::per_device_key_remap_rows(dev.as_str());
        // 기능 1 이 F9 물리 키를 다른 곳으로 재배정 — 기능 2 도 같은 F9 슬롯을 쓴다.
        let conflicting_row = row(SourceKey::F9, SourceKey::LeftControl);
        let f9_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F9);
        let values = BTreeMap::from([
            (
                device_rows_key,
                serde_json::to_value(vec![conflicting_row]).unwrap(),
            ),
            (f9_key, serde_json::to_value(SystemFunction::Mute).unwrap()),
        ]);
        let settings = PerDeviceSettings::new(&values);

        let composition = compose(&dev, &settings, None);

        let expected_mapping = KeyMapping {
            src: SourceKey::F9.hid_usage().unwrap(),
            dst: SourceKey::LeftControl.hid_usage().unwrap(),
        };
        assert_eq!(composition.mappings, vec![expected_mapping]);
        assert_eq!(
            composition.suppressed,
            vec![Suppressed::Feature2Key(FKey::F9)]
        );
    }

    // ⚠️ 이슈 #31 ② 로 전제가 바뀐 테스트 — 예전에는 `SystemFunction::hid_usage()`
    // 가 `None` 인 4종(MissionControl·Spotlight·Dictation·DoNotDisturb) 전부가
    // 배열 후보를 못 냈다. 새 카탈로그에서는 그중 3종(MissionControl 포함)이 벤더
    // page 값을 얻어 후보를 낸다 — `destinations.rs` 의
    // `legacy_system_functions_migrate_to_the_same_values` 가 이미 그 사실을
    // 못박았다. 여기서는 그 새 동작을 compose 계층에서 재확인한다.
    #[test]
    fn compose_migrates_legacy_function_that_gained_a_value() {
        let dev = device();
        let f3_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F3);
        let values = BTreeMap::from([(
            f3_key,
            serde_json::to_value(SystemFunction::MissionControl).unwrap(),
        )]);
        let settings = PerDeviceSettings::new(&values);

        let composition = compose(&dev, &settings, None);

        let expected = destinations::find("appleKeyboard.mission_control").unwrap();
        assert_eq!(
            composition.mappings,
            vec![KeyMapping {
                src: SourceKey::F3.hid_usage().unwrap(),
                dst: expected.value,
            }]
        );
        assert!(composition.suppressed.is_empty());
    }

    // `DoNotDisturb` 만은 새 카탈로그에도 값이 없다(Generic Desktop page, D-17-4
    // 허용 집합 밖 — destinations.rs 모듈 문서 "무엇을 뺐는가" 표) — 애초에 배열
    // 후보를 못 낸다. 밀린 것이 아니므로 suppressed 에도 담기지 않는다.
    #[test]
    fn compose_silently_skips_do_not_disturb_which_still_has_no_destination() {
        let dev = device();
        let f6_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F6);
        let values = BTreeMap::from([(
            f6_key,
            serde_json::to_value(SystemFunction::DoNotDisturb).unwrap(),
        )]);
        let settings = PerDeviceSettings::new(&values);

        let composition = compose(&dev, &settings, None);

        assert!(composition.mappings.is_empty());
        assert!(composition.suppressed.is_empty());
    }

    // ⭐ 이슈 #31 ② — 새 id 로 저장한 값이 compose 에서 그대로 매핑을 만든다.
    #[test]
    fn compose_maps_new_catalog_id_to_its_value() {
        let dev = device();
        let f1_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F1);
        let values = BTreeMap::from([(f1_key, Value::String("consumer.mute".to_string()))]);
        let settings = PerDeviceSettings::new(&values);

        let composition = compose(&dev, &settings, None);

        assert_eq!(
            composition.mappings,
            vec![KeyMapping {
                src: SourceKey::F1.hid_usage().unwrap(),
                dst: destinations::find("consumer.mute").unwrap().value,
            }]
        );
        assert!(composition.suppressed.is_empty());
    }

    // ⭐ 모르는 id 는 매핑도 suppressed 도 만들지 않는다 — panic 없이 조용히 건너뛴다.
    #[test]
    fn compose_skips_unknown_id_without_panicking() {
        let dev = device();
        let f1_key = settings_keys::per_device_function_key(dev.as_str(), FKey::F1);
        let values = BTreeMap::from([(f1_key, Value::String("nope.not_a_key".to_string()))]);
        let settings = PerDeviceSettings::new(&values);

        let composition = compose(&dev, &settings, None);

        assert!(composition.mappings.is_empty());
        assert!(composition.suppressed.is_empty());
    }

    // ── D-17-4 — 쓰기 전 검증 ───────────────────────────────────────────────────

    #[test]
    fn validate_rejects_zero_product_id() {
        let device = DeviceMatch {
            vendor_id: 0x5ac,
            product_id: 0,
        };
        let mappings = vec![KeyMapping {
            src: SourceKey::CapsLock.hid_usage().unwrap(),
            dst: SourceKey::F18.hid_usage().unwrap(),
        }];

        assert_eq!(
            validate(&device, &mappings),
            Err(ValidationError::ZeroProductId)
        );
    }

    // 이슈 #110 — 내장 키보드는 VID/PID 프로퍼티가 없어 `0:0` 이다(스파이크 S-10).
    // 규칙 2 는 IOHIDSystem(`0x5ac:0x0`)만 거부해야 하고, `0:0` 은 통과해야 한다.
    #[test]
    fn validate_accepts_built_in_keyboard_with_zero_vid_and_pid() {
        let built_in = DeviceMatch {
            vendor_id: 0,
            product_id: 0,
        };
        let mappings = vec![KeyMapping {
            src: SourceKey::CapsLock.hid_usage().unwrap(),
            dst: SourceKey::F18.hid_usage().unwrap(),
        }];
        assert_eq!(validate(&built_in, &mappings), Ok(()));
        assert!(!is_iohidsystem(&built_in));
        assert!(is_iohidsystem(&DeviceMatch {
            vendor_id: 0x5ac,
            product_id: 0
        }));
    }

    #[test]
    fn validate_rejects_unknown_page() {
        let device = DeviceMatch {
            vendor_id: 0x5ac,
            product_id: 0x24f,
        };
        // page = 0x99 는 닫힌 어휘(D-17-4 규칙 3, {0x07,0x0C,0xFF,0xFF01})에 없다.
        let bogus_src = (0x99u64 << 32) | 1;
        let mappings = vec![KeyMapping {
            src: bogus_src,
            dst: SourceKey::F18.hid_usage().unwrap(),
        }];

        assert_eq!(
            validate(&device, &mappings),
            Err(ValidationError::UnknownPage {
                value: bogus_src,
                page: 0x99
            })
        );
    }

    #[test]
    fn validate_rejects_duplicate_src() {
        let device = DeviceMatch {
            vendor_id: 0x5ac,
            product_id: 0x24f,
        };
        let src = SourceKey::CapsLock.hid_usage().unwrap();
        let mappings = vec![
            KeyMapping {
                src,
                dst: SourceKey::F18.hid_usage().unwrap(),
            },
            KeyMapping {
                src,
                dst: SourceKey::LeftControl.hid_usage().unwrap(),
            },
        ];

        assert_eq!(
            validate(&device, &mappings),
            Err(ValidationError::DuplicateSrc(src))
        );
    }

    #[test]
    fn validate_rejects_over_length_cap() {
        let device = DeviceMatch {
            vendor_id: 0x5ac,
            product_id: 0x24f,
        };
        // 서로 다른 src(닫힌 어휘 안, Keyboard Page)를 가진 MAX_MAPPINGS+1 개.
        let mappings: Vec<KeyMapping> = (0..=MAX_MAPPINGS as u64)
            .map(|i| KeyMapping {
                src: (0x07u64 << 32) | i,
                dst: (0x07u64 << 32) | 0xFFF,
            })
            .collect();
        assert_eq!(mappings.len(), MAX_MAPPINGS + 1);

        assert_eq!(
            validate(&device, &mappings),
            Err(ValidationError::TooManyMappings {
                len: MAX_MAPPINGS + 1,
                max: MAX_MAPPINGS,
            })
        );
    }

    // ── DeviceId 왕복 ───────────────────────────────────────────────────────────

    #[test]
    fn device_id_round_trips_through_string_form() {
        let id = DeviceId::new(0x5ac, 0x24f);
        assert_eq!(id.as_str(), "5ac:24f");

        let parsed = DeviceId::parse(id.as_str()).expect("파싱 가능해야 한다");
        assert_eq!(parsed, id);
        assert_eq!(
            parsed.to_match(),
            DeviceMatch {
                vendor_id: 0x5ac,
                product_id: 0x24f
            }
        );
    }

    #[test]
    fn device_id_parse_rejects_malformed_strings() {
        assert!(DeviceId::parse("not-a-device-id").is_none());
        assert!(DeviceId::parse("zz:zz").is_none());
        assert!(DeviceId::parse("").is_none());
    }

    // ── §3.4.1 4항 — 빈 행 필터링 ─────────────────────────────────────────────

    #[test]
    fn rows_for_storage_drops_incomplete_drafts() {
        let drafts = vec![
            DraftKeyRemapRow {
                from: Some(SourceKey::CapsLock),
                to: Some(SourceKey::F18),
            },
            DraftKeyRemapRow {
                from: Some(SourceKey::LeftOption),
                to: None, // to 미선택 — 빈 행
            },
            DraftKeyRemapRow {
                from: None,
                to: None, // 완전히 빈 행
            },
        ];

        let rows = rows_for_storage(&drafts);

        assert_eq!(rows, vec![row(SourceKey::CapsLock, SourceKey::F18)]);
    }

    // ── §3.4 — 중복 from 검출(충돌 대화상자 트리거) ─────────────────────────────

    #[test]
    fn find_duplicate_from_detects_repeated_source() {
        let rows = vec![
            row(SourceKey::CapsLock, SourceKey::F18),
            row(SourceKey::CapsLock, SourceKey::LeftControl),
            row(SourceKey::LeftOption, SourceKey::LeftCommand),
        ];

        assert_eq!(find_duplicate_from(&rows), vec![SourceKey::CapsLock]);
    }

    #[test]
    fn find_duplicate_from_is_empty_when_all_distinct() {
        let rows = vec![
            row(SourceKey::CapsLock, SourceKey::F18),
            row(SourceKey::LeftOption, SourceKey::LeftCommand),
        ];

        assert!(find_duplicate_from(&rows).is_empty());
    }

    // ── ⭐ 이슈 #31 회귀 방지 — 충돌은 **source 중복만**이다 ──────────────────────
    //
    // 사용자 보고: "Left option -> Left Command, Left Command -> Left option 이렇게
    // 표현하게 하고 싶은데 … 어떤 한 키가 할당이 되면 다른 방향(Source/Target)으로
    // 설정할 수 없게 동작하는 것 같음." 양방향 스왑은 이 기능의 대표 사용례이고
    // (이슈 #21 이 참조한 사용자 Karabiner 설정이 정확히 그것이다), **같은 키가 한
    // 행에서는 from, 다른 행에서는 to 로 나타나는 것은 충돌이 아니다.** 아래 세 층
    // (검출·합성·쓰기 전 검증) 전부에서 그것을 고정한다.

    /// 양방향 스왑 2행 — from 은 서로 다르다. 중복 검출에 걸리면 안 된다.
    #[test]
    fn bidirectional_swap_is_not_a_duplicate_from() {
        let rows = vec![
            row(SourceKey::LeftOption, SourceKey::LeftCommand),
            row(SourceKey::LeftCommand, SourceKey::LeftOption),
        ];

        assert!(
            find_duplicate_from(&rows).is_empty(),
            "source↔target 교차는 충돌이 아니다: {rows:?}"
        );
    }

    /// 한 키가 여러 행의 **to** 로 나타나는 것도 충돌이 아니다 — 충돌 판정은
    /// `from` 만 본다.
    #[test]
    fn repeated_to_is_not_a_duplicate() {
        let rows = vec![
            row(SourceKey::LeftOption, SourceKey::LeftCommand),
            row(SourceKey::RightOption, SourceKey::LeftCommand),
        ];

        assert!(find_duplicate_from(&rows).is_empty(), "{rows:?}");
    }

    /// 같은 `from` 이 두 번이면 — 그리고 그때만 — 충돌이다.
    #[test]
    fn only_repeated_from_is_a_duplicate() {
        let rows = vec![
            // from 이 겹치는 유일한 쌍.
            row(SourceKey::LeftCommand, SourceKey::LeftOption),
            row(SourceKey::LeftCommand, SourceKey::LeftControl),
            // 아래 두 행은 위 행들과 to/from 이 교차하지만 from 은 고유하다.
            row(SourceKey::LeftOption, SourceKey::LeftCommand),
            row(SourceKey::LeftControl, SourceKey::LeftCommand),
        ];

        assert_eq!(find_duplicate_from(&rows), vec![SourceKey::LeftCommand]);
    }

    /// 합성 — 양방향 스왑 4행(좌우 각각)이 **한 행도 밀려나지 않고** 전부 배열에
    /// 들어간다. 이슈 #21 이 참조한 사용자 Karabiner 설정 그대로다.
    #[test]
    fn compose_keeps_both_directions_of_a_swap() {
        let dev = device();
        let rows = vec![
            row(SourceKey::LeftCommand, SourceKey::LeftOption),
            row(SourceKey::LeftOption, SourceKey::LeftCommand),
            row(SourceKey::RightCommand, SourceKey::RightOption),
            row(SourceKey::RightOption, SourceKey::RightCommand),
        ];
        let values = BTreeMap::from([(
            settings_keys::per_device_key_remap_rows(dev.as_str()),
            serde_json::to_value(&rows).unwrap(),
        )]);
        let settings = PerDeviceSettings::new(&values);

        let composition = compose(&dev, &settings, None);

        assert!(
            composition.suppressed.is_empty(),
            "양방향 스왑은 서로를 밀어내지 않는다: {composition:?}"
        );
        assert_eq!(composition.mappings.len(), 4, "{composition:?}");
        for r in &rows {
            let want = KeyMapping {
                src: r.from.hid_usage().unwrap(),
                dst: r.to.hid_usage().unwrap(),
            };
            assert!(
                composition.mappings.contains(&want),
                "{r:?} 가 빠졌다: {composition:?}"
            );
        }
    }

    /// 쓰기 전 검증 — 양방향 스왑 배열은 `DuplicateSrc` 에 걸리지 않는다.
    #[test]
    fn validate_accepts_bidirectional_swap() {
        let mappings = vec![
            KeyMapping {
                src: SourceKey::LeftCommand.hid_usage().unwrap(),
                dst: SourceKey::LeftOption.hid_usage().unwrap(),
            },
            KeyMapping {
                src: SourceKey::LeftOption.hid_usage().unwrap(),
                dst: SourceKey::LeftCommand.hid_usage().unwrap(),
            },
        ];

        assert_eq!(validate(&device().to_match(), &mappings), Ok(()));
    }

    // 반대 방향(같은 `src` 가 두 번이면 여전히 거부한다 — 충돌 검사 자체를 없앤 것이
    // 아니다)은 아래 `validate_rejects_duplicate_src` 가 이미 고정하고 있다.

    // ── D-17-2 — 원장 직렬화 ────────────────────────────────────────────────────

    #[test]
    fn managed_ledger_round_trips_through_json() {
        let dev = device();
        let mut ledger = ManagedLedger::new();
        ledger.insert(
            dev,
            vec![KeyMapping {
                src: SourceKey::CapsLock.hid_usage().unwrap(),
                dst: SourceKey::F18.hid_usage().unwrap(),
            }],
        );

        let json = write_managed_ledger(&ledger);
        let round_tripped = read_managed_ledger(&json);

        assert_eq!(round_tripped, ledger);
    }

    #[test]
    fn read_managed_ledger_skips_malformed_entries_without_panicking() {
        let raw = json!({
            "not-hex:zzz": [{"src": 1, "dst": 2}],
            "5ac:24f": [{"src": 1, "dst": 2}],
        });

        let ledger = read_managed_ledger(&raw);

        assert_eq!(ledger.len(), 1);
        assert!(ledger.contains_key(&DeviceId::new(0x5ac, 0x24f)));
    }
}
