//! 검증 보조 도구 — 세션 탭 **꼬리**에 붙는 수동(listen-only) 탭.
//!
//! ⭐ 왜 필요한가: `Effect::TypeChar` 처리가 §3.2.2 (i)/(ii) 중 무엇을 골랐는지,
//! 그리고 그 합성 이벤트가 실제로 세션 이벤트 스트림에 실렸는지는 코드를 읽는
//! 것만으로는 확신할 수 없다 — 이슈 #32 가 바로 그 사례다: 중재 엔진은 정상인데
//! 출력 쪽 호출자가 비어 있어 quick press 가 조용히 무동작했다. 이 도구는 그
//! 간극을 기계가 읽을 수 있는 로그로 남긴다. Ultrakey 의 탭(`HeadInsert`)보다
//! **뒤**(`TailAppendEventTap`)에 있으므로, 원본 물리 이벤트뿐 아니라 Ultrakey 가
//! 합성해 낸 이벤트까지 포함해 "다른 앱이 실제로 받게 되는" 스트림을 그대로
//! 찍는다.
//!
//! **어떻게 쓰는가**:
//! ```sh
//! # 1) 이 도구를 먼저 띄워 세션 스트림을 관찰한다(관찰만 하고 아무것도 만들지 않는다).
//! cargo run -p ultrakey-platform --example tap_listen
//! # 2) 다른 터미널에서 key_poke 로 물리 입력을 흉내 내(예: 좌 shift quick press,
//! #    FlagsChanged 로 구성), tap_listen 의 출력에 Ultrakey 가 합성한 문자 이벤트가
//! #    실제로 찍히는지 확인한다.
//! cargo run -p ultrakey-platform --example key_poke -- \
//!     fc:0x38:0x20000 sleep:50 fc:0x38:0x0
//! ```
//! Ultrakey 앱 자체가 `ULTRAKEY_TRACE_TAP=1` 로 떠 있어야 두 도구를 합쳐 "중재
//! 엔진이 무엇을 판정했는가(트레이스 로그)"와 "그 판정이 실제로 무엇을 세션에
//! 실었는가(이 도구의 출력)"를 대조할 수 있다.
//!
//! ⛔ **이 도구로 검증되지 않는 것**:
//! - **수신 앱이 그 이벤트를 어떻게 해석하는지는 알 수 없다.** 특히 IME 가 붙은
//!   앱(한국어 입력기 등)은 같은 `CGEvent` 를 받고도 TSM 파이프라인을 거쳐 전혀
//!   다른 결과를 낼 수 있다 — 이 도구는 세션 스트림에 "무엇이 실렸는가"까지만
//!   보여주고, 그 다음 단계(수신 앱의 해석)는 대상 앱에서 직접 눈으로 확인해야
//!   한다.
//! - HID 층 아래(경로 B `hidutil` 커널 매핑, caps lock 래칭)는 전혀 지나지
//!   않는다 — `key_poke.rs` 모듈 문서와 같은 제약이다.
//! - 사람이 실제로 키를 누르는 타이밍의 미세한 편차.

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    use objc2_core_foundation::{CFMachPort, CFRunLoop, kCFRunLoopCommonModes, CFRunLoopSource};
    use objc2_core_graphics::{
        CGEvent, CGEventField, CGEventMask, CGEventTapLocation, CGEventTapOptions,
        CGEventTapPlacement, CGEventTapProxy, CGEventType,
    };
    use std::ffi::c_void;

    unsafe extern "C-unwind" fn callback(
        _proxy: CGEventTapProxy,
        etype: CGEventType,
        event: core::ptr::NonNull<CGEvent>,
        _ud: *mut c_void,
    ) -> *mut CGEvent {
        let ev = unsafe { event.as_ref() };
        let keycode =
            CGEvent::integer_value_field(Some(ev), CGEventField::KeyboardEventKeycode);
        let flags = CGEvent::flags(Some(ev)).0;
        let src_ud =
            CGEvent::integer_value_field(Some(ev), CGEventField::EventSourceUserData);
        let mut len: core::ffi::c_ulong = 0;
        let mut buf = [0u16; 8];
        unsafe {
            CGEvent::keyboard_get_unicode_string(
                Some(ev),
                8,
                &mut len,
                buf.as_mut_ptr(),
            );
        }
        let s: String = String::from_utf16_lossy(&buf[..len as usize]);
        println!(
            "type={etype:?} keycode=0x{keycode:02X} flags=0x{flags:08X} srcUD=0x{src_ud:X} unicode={s:?}"
        );
        event.as_ptr()
    }

    let mask: CGEventMask = (1 << CGEventType::KeyDown.0)
        | (1 << CGEventType::KeyUp.0)
        | (1 << CGEventType::FlagsChanged.0);

    let port: Option<objc2_core_foundation::CFRetained<CFMachPort>> = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::TailAppendEventTap,
            CGEventTapOptions::ListenOnly,
            mask,
            Some(callback),
            core::ptr::null_mut(),
        )
    };
    let Some(port) = port else {
        eprintln!("탭 생성 실패 — Accessibility 권한을 확인한다");
        std::process::exit(1);
    };
    let source = CFMachPort::new_run_loop_source(None, Some(&port), 0)
        .expect("run loop source");
    let rl = CFRunLoop::current().expect("run loop");
    rl.add_source(Some::<&CFRunLoopSource>(&source), unsafe { kCFRunLoopCommonModes });
    CGEvent::tap_enable(&port, true);
    eprintln!("listening…");
    CFRunLoop::run();
}
