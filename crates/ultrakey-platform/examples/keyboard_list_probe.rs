//! 키보드 디바이스 열거 프로브 — F-17(`per-device-settings.md` §3.2) 수동 검증 도구.
//!
//! `hid_device::list_attached_keyboards()` 가 실제로 무엇을 돌려주는지 눈으로
//! 확인한다. `docs/dev/manual-verification.md` 의 F-17 절차가 이 프로브를
//! `hidutil list` 출력과 대조하는 데 쓴다.
//!
//! ⭐ 특히 확인하는 것: **스파이크 S-2 — 물리 디바이스 1개가 IOHID 서비스
//! 여러 개다.** `hidutil list` 는 같은 키보드를 여러 줄로 보여주지만, 이
//! 열거 API 는 `(VendorID, ProductID)` 로 중복을 제거해 **키보드 1대당 한
//! 줄**을 돌려주어야 한다. 그러지 않으면 `Keyboards` 탭 팝업에 같은 키보드가
//! 여러 번 뜬다.
//!
//! ```sh
//! cargo run -p ultrakey-platform --example keyboard_list_probe
//! # 대조군:
//! hidutil list --matching '{"PrimaryUsagePage":1,"PrimaryUsage":6}'
//! ```
//!
//! ⛔ 이 프로브는 **읽기만 한다** — `UserKeyMapping` 을 쓰지 않는다.
//!
//! ---
//!
//! ## 이슈 #86 진단 모드
//!
//! 내장 키보드가 `Keyboards` 탭에 안 보인다는 보고(이슈 #86)를 **판정**하는
//! 목적으로 확장했다. 원인 후보는 둘이다(계획 `docs/plan/issue-86-built-in-keyboard.md`
//! §2 D1):
//!
//! - `<원인 a>` 매칭 필터 문제 — 코드가 쓰던 `DeviceUsagePage`/`DeviceUsage`
//!   (와 달리) 내장 키보드는 `hidutil list` 가 쓰는 `PrimaryUsagePage`/
//!   `PrimaryUsage` 에서만 매칭될 수 있다.
//! - `<원인 b>` VID/PID 를 못 읽어 `list_attached_keyboards()` 가 항목을 버린다.
//!
//! 그래서 아래 출력을 가진다:
//!
//! 1. **출력 1 — 원시 서비스 덤프**(중복 제거 전): 두 필터 각각으로 매칭된 모든
//!    IOHID 서비스의 `RegistryID`·`VendorID`·`ProductID`·`Product`·`Transport`·
//!    `Built-In`·`DeviceUsagePage/Usage`·`PrimaryUsagePage/Usage`.
//! 2. **출력 2 — 이중 매칭 비교**: 두 필터의 결과 집합 차이. **현재 기기에서
//!    두 필터가 같은 집합을 주면 필터 키 정렬이 회귀 없음**이다(그래도 내장
//!    키보드 포함 여부는 실기기 전까지 `(미검증)`).
//! 3. **출력 3 — 기존 dedup 회귀 체크 유지**: 같은 `(VID,PID)` 가 두 번 나오면
//!    `exit 1`.
//!
//! 판정 기준(계획 §2 D1 표):
//!
//! | 관측 | 판정 |
//! | :--- | :--- |
//! | 내장 키보드 서비스가 출력 1 에 **없다** | 원인 (a) — 매칭 필터 문제 |
//! | 출력 1 에는 있는데 VID/PID 가 `(못 읽음)` | 원인 (b) — `None` 폐기 |
//! | 출력 1 에는 있는데 dedup 후 사라짐 | 원인 (c) — dedup 충돌 |
//! | 출력 1·2 모두에 없음 | `ioreg -l` 덤프로 재판정 |

use ultrakey_platform::hid_device::{
    list_attached_keyboards, list_raw_keyboard_services, KeyboardFilter, RawKeyboardService,
};

fn fmt_hex(v: &Option<u32>) -> String {
    match v {
        Some(v) => format!("0x{v:x}"),
        None => "(못 읽음)".to_string(),
    }
}

fn fmt_built_in(v: &Option<bool>) -> String {
    match v {
        Some(true) => "1".to_string(),
        Some(false) => "0".to_string(),
        None => "(못 읽음)".to_string(),
    }
}

fn print_raw_service(label: &str, services: &[RawKeyboardService]) {
    println!("--- {label} — 매칭 서비스 {n}개 (중복 제거 전) ---", n = services.len());
    if services.is_empty() {
        println!("  (없음)\n");
        return;
    }
    for (i, s) in services.iter().enumerate() {
        println!("[{i}] RegistryID={}", s.registry_id.map(|r| format!("0x{r:x}")).unwrap_or_else(|| "(못 읽음)".into()));
        println!("    VID={}  PID={}", fmt_hex(&s.vendor_id), fmt_hex(&s.product_id));
        println!(
            "    Product={}  Transport={}",
            s.product_name.as_deref().unwrap_or("(못 읽음)"),
            s.transport.as_deref().unwrap_or("(못 읽음)")
        );
        println!(
            "    Built-In={}  DeviceUsagePage/Usage={}/{}  PrimaryUsagePage/Usage={}/{}",
            fmt_built_in(&s.built_in),
            fmt_hex(&s.device_usage_page),
            fmt_hex(&s.device_usage),
            fmt_hex(&s.primary_usage_page),
            fmt_hex(&s.primary_usage)
        );
    }
    println!();
}

/// 진단 출력에서 서비스를 "집합 원소"로 비교하기 위한 식별자. `RegistryID` 를
/// 1순위로, 못 읽으면 (VID,PID,제품명) 으로 폴백한다.
fn identity(s: &RawKeyboardService) -> String {
    if let Some(reg) = s.registry_id {
        return format!("RegistryID=0x{reg:x}");
    }
    format!(
        "VID={} PID={} Product={}",
        fmt_hex(&s.vendor_id),
        fmt_hex(&s.product_id),
        s.product_name.as_deref().unwrap_or("(없음)")
    )
}

fn set_diff(
    device: &[RawKeyboardService],
    primary: &[RawKeyboardService],
) -> (Vec<String>, Vec<String>) {
    let a: std::collections::HashSet<String> =
        device.iter().map(identity).collect();
    let b: std::collections::HashSet<String> =
        primary.iter().map(identity).collect();
    let a_only: Vec<String> = a
        .iter()
        .filter(|id| !b.contains(*id))
        .cloned()
        .collect();
    let b_only: Vec<String> = b
        .iter()
        .filter(|id| !a.contains(*id))
        .cloned()
        .collect();
    (a_only, b_only)
}

/// (VID,PID) 로 중복 제거한 디바이스 집합 — `list_attached_keyboards()` 가 실제로
/// 돌려주는 수준이다. 문자열을 정렬해 출력하면 두 필터의 "키보드 목록"이 같은지
/// 읽기 쉽다.
fn dedup_pairs(services: &[RawKeyboardService]) -> Vec<String> {
    let mut set: std::collections::HashSet<(u32, u32)> = services
        .iter()
        .filter_map(|s| Some((s.vendor_id?, s.product_id?)))
        .collect();
    let mut lines: Vec<String> = set.drain().map(|(v, p)| format!("0x{v:x}:0x{p:x}")).collect();
    lines.sort_unstable();
    lines
}

fn main() {
    println!("=== 이슈 #86 — 키보드 열거 진단 ===\n");

    // ── 출력 1·2: 두 필터 각각의 원시 서비스 덤프(중복 제거 전)
    let device_usage = list_raw_keyboard_services(KeyboardFilter::DeviceUsage);
    let primary_usage = list_raw_keyboard_services(KeyboardFilter::PrimaryUsage);

    print_raw_service(
        "출력 1 · DeviceUsagePage/DeviceUsage 필터(기존 매칭 사전)",
        &device_usage,
    );
    print_raw_service(
        "출력 1 · PrimaryUsagePage/PrimaryUsage 필터(hidutil list 정본, 명세 §3.2)",
        &primary_usage,
    );

    // ── 출력 2: 이중 매칭 비교 — 두 필터 결과 집합 차이
    let (a_only, b_only) = set_diff(&device_usage, &primary_usage);
    println!("=== 출력 2 · 두 필터 결과 집합 차이 ===");
    println!(
        "서비스 단위 — 공통 {}개 / DeviceUsage 전용 {}개 / PrimaryUsage 전용 {}개",
        device_usage.len() - a_only.len(),
        a_only.len(),
        b_only.len()
    );
    if !a_only.is_empty() {
        println!("DeviceUsage 필터에만 있는 서비스:");
        for id in &a_only {
            println!("  - {id}");
        }
    }
    if !b_only.is_empty() {
        println!("PrimaryUsage 필터에만 있는 서비스:");
        for id in &b_only {
            println!("  - {id}");
        }
    }

    // 회귀 게이트는 실제 API(`list_attached_keyboards()`)가 반환하는 (VID,PID)
    // 중복 제거 후 수준으로 본다 — 이 두 집합이 같으면 매칭 키 정렬이
    // `Keyboards` 탭 목록을 바꾸지 않는다는 뜻이다.
    let device_pairs = dedup_pairs(&device_usage);
    let primary_pairs = dedup_pairs(&primary_usage);
    println!(
        "\n(VID,PID) 중복 제거 후 — DeviceUsage: {}대, PrimaryUsage: {}대",
        device_pairs.len(),
        primary_pairs.len()
    );
    if device_pairs != primary_pairs {
        println!("  ⚠️ 두 필터의 (VID,PID) 디바이스 목록이 다르다!");
        println!("    DeviceUsage 전용: {:?}", device_pairs);
        println!("    PrimaryUsage 전용: {:?}", primary_pairs);
    } else {
        println!("  → 두 필터의 (VID,PID) 디바이스 목록이 동일하다.");
    }

    println!(
        "\n판정(현재 기기 기준): {} — 매칭 키 정렬이 `Keyboards` 탭 목록을 바꾸지 않으므로 회귀 없음.",
        if device_pairs == primary_pairs {
            "(VID,PID) 목록 동일"
        } else {
            "(VID,PID) 목록 상이 ⚠️"
        }
    );
    println!("⛔ 본 기기엔 내장 키보드가 없어 내장 키보드 포함 여부는 실기기 전까지 (미검증)입니다.\n");

    // ── 출력 3: 기존 dedup 회귀 체크가 깨졌으면 exit 1(스파이크 S-2 게이트)
    let devices = list_attached_keyboards();
    println!("=== 출력 3 · dedup 회귀 체크(color = list_attached_keyboards) ===");
    println!("열거된 키보드 {}대 (중복 제거 후)\n", devices.len());
    println!(
        "{:<8} {:<8} {:<32} {:<10} Built-In",
        "VID", "PID", "Product", "Transport"
    );
    for d in &devices {
        println!(
            "0x{:<6x} 0x{:<6x} {:<32} {:<10} {}",
            d.vendor_id,
            d.product_id,
            d.product_name.as_deref().unwrap_or("(없음)"),
            d.transport.as_deref().unwrap_or("(없음)"),
            match d.built_in {
                Some(true) => "1",
                Some(false) => "0",
                None => "(못 읽음)",
            }
        );
    }

    // ⭐ S-2 회귀 확인 — 같은 (VID,PID) 가 두 번 나오면 중복 제거가 깨진 것이다.
    let mut ids: Vec<(u32, u32)> = devices.iter().map(|d| (d.vendor_id, d.product_id)).collect();
    ids.sort_unstable();
    let before = ids.len();
    ids.dedup();
    if ids.len() != before {
        eprintln!(
            "\n⛔ 중복 발견 — (VID,PID) 가 같은 항목이 {}건 있다. \
             S-2 중복 제거가 깨졌다(§3.2).",
            before - ids.len()
        );
        std::process::exit(1);
    }
    println!("\n✅ (VID,PID) 중복 없음 — S-2 중복 제거가 동작한다.");
}