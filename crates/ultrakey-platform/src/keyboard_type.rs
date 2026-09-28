//! ⭐ F-19(D-4) 키보드 타입 판정 — JIS(일본어 배열)인가가 F-19.5·F-19.6 의 전제다
//! (`docs/spec/language-presets.md` §3.2, `ultrakey-core::jis::JisState`).
//!
//! macOS 에서 "생리 키보드가 JIS 인가"를 묻는 두 경로를 순서대로 시도한다:
//! ① `CGEventSourceGetKeyboardType` — Quartz 이벤트 소스에 연결된 키보드 타입.
//!    ⚠️ 반환 상수(`kCGKeyboardTypeANSI=40`·`ISO=41`·`JIS=42`)는 **Apple 공개 문서값**으로
//!    SDK 헤더에 매크로가 없다(현대 SDK 에서 상수 정의가 제거됨) — `(추정, 실기기 검증 대기)`.
//! ② 폴백 — `kTISPropertyInputSourceKeyboardType` 은 CLT SDK 에 헤더가 없어 선언하지
//!    않는다(이 저장소 원칙: 값·시그니처를 지어내지 않는다). 확보되면 이 자리에 추가.
//! ⭐ 이슈 #149 C9/D5 — `NULL` 대신 `event::shared_event_source()` 를 넘긴다. `NULL` 호출은
//! SkyLight 가 unified log 에 `invalid CGEventSource: 0x0` 에러 2건을 남긴다(실측 로그 소음). 판정 동작은 변하지 않는다.
//!
//! ⭐ 결과는 3상태(fail-closed) 로 내려간다 — 판정 불가(`Unknown`)는 `ultrakey-core::jis`
//! 의 게이트가 JIS 행·US 행 **양쪽 다** 발화시키지 않는다(H4). 두 실패 모드가 파괴적이므로
//! (JIS 에서 US 행이 `]` 키를, ANSI 에서 JIS 행이 심볼행을 강탈) 안전 방향이 맞다.

use crate::ffi;

/// 이 기기가 JIS 키보드를 쓰는가. `None` = 판정 불가(fail-closed).
pub fn current_keyboard_is_jis() -> Option<bool> {
    // 이슈 #149 C9/D5 — `NULL` 을 넘기면 SkyLight 가 호출당 `invalid CGEventSource: 0x0`
    // 에러 2건을 unified log 에 남기므로(실측), 합성 경로와 공유하는 정적 소스
    // (`event::shared_event_source()`, C10)의 핸들을 넘긴다. 소스가 없으면 기존처럼
    // null 폴백한다 — 아래 판정 매치는 그대로라 `is_jis` 게시값이 변하지 않는다.
    let source_ptr: *mut core::ffi::c_void = match crate::event::shared_event_source() {
        Some(source) => source as *const _ as *mut core::ffi::c_void,
        None => std::ptr::null_mut(),
    };
    // SAFETY: 포인터는 프로세스 생애주기 동안 유효한 정적 소스(`event.rs` 불변식)이거나
    // null 이고, `CGEventSourceGetKeyboardType` 은 인자를 읽기만 하는 순수 조회 함수다
    // (헤더상 `__nullable`, 시그니처 근거는 `ffi.rs`).
    let raw = unsafe { ffi::CGEventSourceGetKeyboardType(source_ptr) };
    match raw {
        // kCGKeyboardTypeJIS — Apple 문서값(위 모듈 문서 참고).
        42 => Some(true),
        // kCGKeyboardTypeANSI / kCGKeyboardTypeISO.
        40 | 41 => Some(false),
        _ => None,
    }
}
