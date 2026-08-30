//! 검증 보조 도구 — **마커 없는** 키 이벤트를 합성해 세션에 던진다.
//!
//! ⭐ 왜 필요한가: F-08 프리셋을 실기기에서 확인하려면 "물리 키를 눌렀을 때 다른 앱이
//! 무엇을 받는가"를 봐야 한다. 사람이 직접 누르는 것이 가장 정확하지만, 자동화된 검증
//! 회차에서는 그것을 재현할 수 없다. 이 도구는 그 사이를 메운다 — `CGEventPost` 로
//! 키 이벤트를 세션에 넣으면 Ultrakey 의 `CGEventTap`(경로 A)이 그것을 물리 이벤트와
//! **구분하지 않고** 받으므로, 경로 A 의 중재·합성 전 구간을 실제 앱을 상대로 검증할 수 있다.
//!
//! ⛔ **이 도구로 검증되지 않는 것**(반드시 구분해 기록하라):
//! - **HID 층 아래**는 전혀 지나지 않는다. 즉 경로 B(`hidutil` 커널 매핑)도, caps lock 의
//!   래칭 동작도 이 경로로는 재현되지 않는다. `caps lock → F18` 매핑이 걸린 상태를
//!   흉내 내려면 이 도구에 **F18 을 직접** 넣어야 한다(그것이 커널 매핑이 하는 일이다).
//! - 사람이 실제로 키를 누르는 타이밍의 미세한 편차.
//!
//! ⚠️ `ultrakey-platform::event::SyntheticEvent` 를 쓰지 않는 이유: 그쪽은 자기 합성
//! 이벤트 마커(`ULTRAKEY_MAGIC`)를 반드시 심는데, 그러면 Ultrakey 의 탭이 콜백 0-a 단계에서
//! 즉시 통과시켜 버려(§5 엣지 12 무한 루프 방지) 중재가 아예 일어나지 않는다. 이 도구는
//! **마커를 심지 않는다** — 그래야 물리 입력과 같은 취급을 받는다.
//!
//! 사용법:
//! ```sh
//! cargo run -p ultrakey-platform --example key_poke -- \
//!     down:0x4F down:0x0D up:0x0D up:0x4F
//! # tap:0x38 은 down 뒤 지정 지연(기본 60ms)을 두고 up 을 낸다
//! # sleep:250 은 250ms 대기
//! # ⭐ fc:0x38:0x20000 은 flagsChanged 를 낸다 — modifier 키(shift·control·option·
//! #   command·caps lock·globe)의 누름/뗌을 macOS 가 실제로 보내는 형태다.
//! #   `KeyDown`/`KeyUp` 으로만 시험하면 M1 이 안고 머지됐던 바로 그 구멍을 재현한다.
//! ```

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("이 도구는 macOS 전용이다.");
    std::process::exit(1);
}

#[cfg(target_os = "macos")]
fn main() {
    use objc2_core_graphics::{
        CGEvent, CGEventFlags, CGEventSource, CGEventSourceStateID, CGEventTapLocation,
        CGEventType,
    };
    use std::time::Duration;

    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("사용법: key_poke <down:0xNN | up:0xNN | tap:0xNN[:ms] | sleep:ms> …");
        std::process::exit(2);
    }

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
    let source_ref = source.as_deref();

    let post = |keycode: u16, down: bool| {
        // SAFETY: `CGEvent::new_keyboard_event` 는 소스와 keycode 만으로 새 이벤트를
        // 만든다. 반환된 이벤트는 우리가 소유하며 `CFRetained` 가 해제를 책임진다.
        let ev = CGEvent::new_keyboard_event(source_ref, keycode, down);
        if let Some(ev) = ev {
            // ⭐ 마커를 심지 않는다(위 모듈 문서 참고).
            CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&ev));
        } else {
            eprintln!("이벤트 생성 실패: keycode=0x{keycode:02X} down={down}");
        }
    };

    // ⭐ modifier 키의 누름/뗌은 `KeyDown`/`KeyUp` 이 아니라 `flagsChanged` 하나로만 온다
    // (`key-remapping-engine.md` §5 #18). 그 형태를 그대로 재현한다.
    let post_flags_changed = |keycode: u16, flags: u64| {
        let ev = CGEvent::new_keyboard_event(source_ref, keycode, true);
        if let Some(ev) = ev {
            CGEvent::set_type(Some(&ev), CGEventType::FlagsChanged);
            CGEvent::set_flags(Some(&ev), CGEventFlags(flags));
            CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&ev));
        } else {
            eprintln!("flagsChanged 생성 실패: keycode=0x{keycode:02X}");
        }
    };

    for arg in &args {
        let (kind, rest) = match arg.split_once(':') {
            Some(pair) => pair,
            None => {
                eprintln!("알 수 없는 인자: {arg}");
                std::process::exit(2);
            }
        };
        match kind {
            "sleep" => {
                let ms: u64 = rest.parse().expect("sleep:<ms> 의 ms 를 읽을 수 없다");
                std::thread::sleep(Duration::from_millis(ms));
            }
            "down" | "up" => {
                let kc = parse_keycode(rest);
                post(kc, kind == "down");
                std::thread::sleep(Duration::from_millis(15));
            }
            "fc" => {
                let (kc_str, flags_str) = rest.split_once(':').unwrap_or((rest, "0"));
                let kc = parse_keycode(kc_str);
                let flags = parse_u64(flags_str);
                post_flags_changed(kc, flags);
                std::thread::sleep(Duration::from_millis(15));
            }
            "tap" => {
                let (kc_str, hold_ms) = match rest.split_once(':') {
                    Some((k, m)) => (k, m.parse::<u64>().expect("tap:<kc>:<ms>")),
                    None => (rest, 60),
                };
                let kc = parse_keycode(kc_str);
                post(kc, true);
                std::thread::sleep(Duration::from_millis(hold_ms));
                post(kc, false);
                std::thread::sleep(Duration::from_millis(15));
            }
            other => {
                eprintln!("알 수 없는 동작: {other}");
                std::process::exit(2);
            }
        }
    }
    println!("완료: {}개 동작", args.len());
}

#[cfg(target_os = "macos")]
fn parse_u64(s: &str) -> u64 {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(hex, 16).expect("16진 flags 를 읽을 수 없다")
    } else {
        s.parse().expect("10진 flags 를 읽을 수 없다")
    }
}

#[cfg(target_os = "macos")]
fn parse_keycode(s: &str) -> u16 {
    let s = s.trim();
    if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u16::from_str_radix(hex, 16).expect("16진 keycode 를 읽을 수 없다")
    } else {
        s.parse().expect("10진 keycode 를 읽을 수 없다")
    }
}
