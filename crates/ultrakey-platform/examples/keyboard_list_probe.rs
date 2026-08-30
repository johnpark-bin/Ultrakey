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

fn main() {
    let devices = ultrakey_platform::hid_device::list_attached_keyboards();

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
