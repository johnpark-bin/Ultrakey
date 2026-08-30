//! macOS `Use F1, F2, etc. keys as standard function keys` 토글 읽기.
//!
//! `docs/spec/per-device-settings.md` §3.5.1 이 이 토글을 "F-17 기능 2 의 전제
//! 조건"으로 요구한다 — 우리가 이 값을 바꾸지는 않는다(사용자의 시스템 설정을
//! 대신 조작하지 않는다), `Keyboards` 탭 상단에 현재 상태만 표시한다.
//!
//! ⭐ **IOKit 레지스트리에서 읽는다 — `CFPreferences` 가 아니다(이슈 #31).**
//! 예전 구현은 `CFPreferencesCopyValue` 로 `NSGlobalDomain` 의
//! `com.apple.keyboard.fnState` 를 읽었다. 이슈 #31 에서 사용자가 **시스템
//! 설정에서 토글을 꺼도 앱이 계속 On 으로 표시한다**고 보고됐다 — 원인은
//! `CFPreferences` 가 프로세스 안에서 값을 캐시해, 우리 프로세스가 살아있는
//! 동안 시스템 설정 앱이 값을 바꿔도 그 변경을 반영하지 못하는 것이다.
//!
//! Karabiner-Elements 는 정확히 이 이유로 `CFPreferences` 대신 IOKit
//! 레지스트리에서 이 값을 읽는다(이 값은 커널이 들고 있어 캐시되지 않는다).
//! 경로(사실만 참조 — 구현은 이 크레이트가 직접 짠다):
//! - 레지스트리 엔트리: `IOHIDSystem` 서비스(`IOService:/IOResources/IOHIDSystem`)
//! - 프로퍼티: `HIDParameters`(`CFDictionary`)
//! - 그 딕셔너리 안의 키: `HIDFKeyMode`(정수). `0` 이 아니면 토글 켜짐(F1~F12
//!   단독 입력이 표준 F-키 usage 로 도착한다, F-17 기능 2 의 전제조건).
//! - Karabiner 원문 주석: "retrieving data using CFPreferencesCopyAppValue can
//!   result in issues, such as returning no value for guest users."
//!
//! ⭐ 이 워크트리에서 직접 실측 확인했다(2026-08-30):
//! ```text
//! $ ioreg -l -w 0 -c IOHIDSystem | grep -o 'HIDFKeyMode"=[0-9]*'
//! HIDFKeyMode"=1
//! $ defaults read -g com.apple.keyboard.fnState
//! 1
//! ```
//! 두 경로가 같은 값을 준다 — 차이는 IOKit 쪽이 **커널의 현재 값**이라 절대
//! 캐시되지 않는다는 점이다. `ioreg -r -c IOHIDSystem` 으로 트리를 직접 보면
//! `HIDParameters` 는 `IOHIDSystem` 서비스 엔트리 **자신의 직속(최상위)**
//! 프로퍼티다(개별 HID 장치의 `HIDEventServiceProperties` 안에도 같은 이름의
//! 키가 복제돼 있지만, 우리는 그쪽을 읽지 않는다 — `IOHIDSystem` 서비스
//! 자신의 `HIDParameters` 만 읽는다).
//!
//! ⚠️ **반환 계약이 예전과 다르다 — "키 없음"의 의미가 바뀌었다.** CFPreferences
//! 경로에서는 키가 아예 없는 것이 macOS 기본값(꺼짐)이라 `Some(false)`로
//! 해석하는 것이 합리적이었다. 이 IOKit 경로에서는 반대다 — `IOHIDSystem`
//! 서비스의 `HIDParameters`/`HIDFKeyMode` 는 이 기종에서 실측상 **항상
//! 존재한다**. 그러므로 못 찾는 것은 "꺼짐 상태"가 아니라 "우리가 잘못 읽고
//! 있다"는 신호다 — 그래서 **키가 없으면 `None`**(읽기 실패)으로 취급한다.
//! UI 는 이 경우를 "알 수 없음"으로 표시한다.

#[cfg(target_os = "macos")]
mod macos_impl {
    use crate::ffi::{self, IoServiceT, K_IOHID_SYSTEM_CLASS};
    use objc2_core_foundation::{CFBoolean, CFDictionary, CFNumber, CFRetained, CFString, CFType};
    use std::ffi::c_void;
    use std::ptr::NonNull;

    /// `IOHIDSystem` 서비스의 최상위 프로퍼티 — 값은 `CFDictionary`.
    const K_HID_PARAMETERS_KEY: &str = "HIDParameters";
    /// `HIDParameters` 딕셔너리 안의 정수 키 — `0` 이 아니면 토글 켜짐.
    const K_HID_F_KEY_MODE_KEY: &str = "HIDFKeyMode";

    /// `IOHIDSystem` 서비스를 찾아 그 `io_service_t` 핸들을 반환한다.
    ///
    /// ⚠️ `hid_lock.rs::open_connection` 과 달리 **핸들을 캐시하지 않는다** —
    /// 이 값은 자주 읽지 않고, 캐시하면 캐시가 새 값을 반영 못 하는 이슈 #31
    /// 과 같은 종류의 버그를 다시 만들 위험이 있다(이 함수 전체의 존재
    /// 이유가 "캐시 때문에 외부 변경을 못 본다"를 없애는 것이다). 호출자가
    /// 다 쓰고 나면 `IOObjectRelease` 로 해제해야 한다.
    fn open_hid_system_service() -> Option<IoServiceT> {
        // SAFETY: `IOServiceMatching` 은 정적 C 문자열을 받는 순수 함수이고,
        // 반환된 딕셔너리의 소유권은 우리에게 있다 — 아래
        // `IOServiceGetMatchingService` 가 그 참조 하나를 소비한다
        // (CF_RELEASES_ARGUMENT). `hid_lock.rs::open_connection` 과 동일
        // 패턴이다.
        let matching = unsafe {
            ffi::IOServiceMatching(K_IOHID_SYSTEM_CLASS.as_ptr() as *const std::os::raw::c_char)
        };
        if matching.is_null() {
            return None;
        }

        // SAFETY: `matching` 은 방금 만든 유효한(그리고 이 호출로 소비되는)
        // 딕셔너리다. `kIOMainPortDefault` 는 IOKit 프레임워크가 항상
        // 정의하는 정적 심볼이다.
        let service: IoServiceT = unsafe {
            ffi::IOServiceGetMatchingService(
                ffi::kIOMainPortDefault,
                matching as *const CFDictionary,
            )
        };
        if service == 0 {
            None
        } else {
            Some(service)
        }
    }

    /// `entry` 에서 `key` 프로퍼티를 읽는다(Create 규칙 — 우리가 소유권을
    /// 받는다). `hid_device.rs::create_cf_property` 와 같은 패턴이지만, 이
    /// 모듈은 `hid_device.rs` 의 비공개 헬퍼를 재사용할 수 없어(`pub(crate)`
    /// 로 노출돼 있지 않다) 여기서 다시 짠다.
    fn create_cf_property(entry: IoServiceT, key: &str) -> Option<CFRetained<CFType>> {
        let cf_key = CFString::from_str(key);
        // SAFETY: `entry` 는 호출자가 보증하는 유효한 io_registry_entry_t 이고,
        // `cf_key` 는 이 호출 동안 유효한 CFString 이다. `allocator` 는 기본
        // 할당자를 뜻하는 NULL 을 넘긴다. `options` 는 이 함수에 정의된
        // 플래그가 없으므로(ffi.rs 주석 참고) 0을 쓴다.
        let raw = unsafe {
            ffi::IORegistryEntryCreateCFProperty(
                entry,
                &*cf_key as *const CFString,
                std::ptr::null(),
                0,
            )
        };
        let ptr = NonNull::new(raw as *mut CFType)?;
        // SAFETY: `IORegistryEntryCreateCFProperty` 는 CF_RETURNS_RETAINED
        // 규칙을 따른다(문서: "The caller should release with CFRelease") —
        // 이 포인터의 소유권은 이미 우리에게 있다.
        Some(unsafe { CFRetained::from_raw(ptr) })
    }

    /// `service` 의 `HIDParameters` 딕셔너리에서 `HIDFKeyMode` 를 읽는다.
    fn read_f_key_mode(service: IoServiceT) -> Option<bool> {
        let params = create_cf_property(service, K_HID_PARAMETERS_KEY)?;
        // `CFDictionary`(제네릭 없는 기본형)로 다운캐스트한다 — `HIDParameters`
        // 는 문서·실측 둘 다 딕셔너리임을 확인했다(모듈 문서 참고).
        let dict = params.downcast::<CFDictionary>().ok()?;

        let key = CFString::from_str(K_HID_F_KEY_MODE_KEY);
        // SAFETY: `dict` 는 이 스코프 동안 살아있는 유효한 CFDictionary 다.
        // `key` 도 이 호출 동안 유효한 CFString 이다. `CFDictionary::value`
        // (`CFDictionaryGetValue`)는 Get 규칙이라 반환값을 release 하면 안
        // 된다 — `dict` 가 드롭되기 전까지만 참조로 즉시 값을 읽는다
        // (`text_input_source.rs` 의 `TISGetInputSourceProperty` 처리와 같은
        // 패턴).
        let value_ptr = unsafe { dict.value(&*key as *const CFString as *const c_void) };
        let value_ptr = NonNull::new(value_ptr as *mut CFType)?;
        // SAFETY: 위 근거대로 Get 규칙 값이고, `dict` 가 이 함수 스코프 동안
        // 살아있으므로 이 참조도 그동안 유효하다.
        let value = unsafe { value_ptr.as_ref() };

        // `ioreg` 출력상 `HIDFKeyMode` 는 bare 정수(`=1`)로 표시되어(다른
        // Boolean 프로퍼티들은 `Yes`/`No` 로 표시된다) `CFNumber` 로 저장돼
        // 있을 가능성이 높지만, 예전 CFPreferences 구현이 겪은 것과 같은
        // 종류의 타입 착각을 피하기 위해 `CFBoolean` 도 폴백으로 받는다.
        match value.downcast_ref::<CFNumber>() {
            Some(number) => number.as_i64().map(|v| v != 0),
            None => value.downcast_ref::<CFBoolean>().map(CFBoolean::value),
        }
    }

    /// `Use F1, F2, etc. keys as standard function keys` 의 현재 값을 읽는다.
    ///
    /// - `Some(true)` — 켜짐. F1~F12 단독 입력이 표준 F-키 usage 로 도착한다
    ///   (F-17 기능 2 의 전제조건, §3.5).
    /// - `Some(false)` — 꺼짐(`HIDFKeyMode == 0`).
    /// - `None` — 읽기 자체가 실패했다: `IOHIDSystem` 서비스를 못 찾음 /
    ///   `HIDParameters` 가 없거나 딕셔너리가 아님 / `HIDFKeyMode` 가 없거나
    ///   숫자·불리언이 아님. UI 는 이 경우를 "알 수 없음"으로 표시한다(모듈
    ///   문서의 "반환 계약" 절 참고 — 예전 CFPreferences 구현과 다르다).
    pub fn f_keys_are_standard() -> Option<bool> {
        let service = open_hid_system_service()?;
        let result = read_f_key_mode(service);
        // SAFETY: `service` 는 위에서 얻은 유효한 서비스 핸들이고 더 이상
        // 필요 없다 — `IOServiceGetMatchingService` 는 소유권을 우리에게
        // 넘기므로 우리가 해제해야 한다(`hid_lock.rs::open_connection` 과
        // 동일 패턴).
        unsafe { ffi::IOObjectRelease(service) };
        result
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::f_keys_are_standard;

#[cfg(not(target_os = "macos"))]
pub fn f_keys_are_standard() -> Option<bool> {
    None
}

#[cfg(target_os = "macos")]
#[cfg(test)]
mod tests {
    use super::f_keys_are_standard;

    /// ⚠️ **실기(實機) 종속 테스트다.** 이 크레이트의 다른 테스트는 전부
    /// 캔버스 문자열(예: `hid_mapping.rs` 의 실측 `hidutil` 출력)만 파싱해
    /// 순수 함수를 검증하지만, 이 함수는 IOKit 레지스트리를 실제로 읽는
    /// 얇은 계층이라 "값이 온다"·"그 값이 맞다"를 확인하려면 실제 커널
    /// 상태와 대조하는 수밖에 없다. `ioreg` 파싱이 이 실행 환경에서
    /// 실패하면(예: 다른 CI 이미지의 `ioreg` 출력 형식 차이) 이 사실 자체가
    /// `f_keys_are_standard()` 의 결함이 아니므로 테스트를 실패시키지 않고
    /// `eprintln!` 후 조용히 통과시킨다.
    #[test]
    fn matches_ioreg_hidfkeymode() {
        let Some(actual) = f_keys_are_standard() else {
            panic!("IOKit 경로가 이 머신에서 HIDFKeyMode 를 읽지 못했다(None) — 회귀 의심");
        };

        let output = std::process::Command::new("ioreg")
            .args(["-l", "-w", "0", "-c", "IOHIDSystem"])
            .output();
        let Ok(output) = output else {
            eprintln!("ioreg 실행 실패 — 대조 검증을 건너뛴다(실기 종속 테스트)");
            return;
        };
        let stdout = String::from_utf8_lossy(&output.stdout);

        let Some(idx) = stdout.find("HIDFKeyMode\"=") else {
            eprintln!("ioreg 출력에서 HIDFKeyMode 를 찾지 못했다 — 대조 검증을 건너뛴다");
            return;
        };
        let after = &stdout[idx + "HIDFKeyMode\"=".len()..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        let Ok(expected) = digits.parse::<i64>() else {
            eprintln!(
                "ioreg 출력의 HIDFKeyMode 값을 파싱하지 못했다({digits:?}) — 대조 검증을 건너뛴다"
            );
            return;
        };

        assert_eq!(
            actual,
            expected != 0,
            "f_keys_are_standard()={actual} 가 ioreg 의 HIDFKeyMode={expected} 와 다르다"
        );
    }
}
