//! F-12 — Keychain 영속 어댑터 (`licensing-and-trial.md` §3.5).
//!
//! 트라이얼 기산점·라이선스 캐시를 macOS Keychain(`Security.framework`)에 저장해
//! 앱 삭제·재설치에도 기산점이 살아 있게 한다. `ultrakey-license` 의
//! [`TrialStore`]/[`CacheStore`] 트레이트를 구현해 순수 상태 머신과 `unsafe` FFI
//! 를 연결한다.
//!
//! ## 저장 설계
//! - generic password 두 항목: 트라이얼 상태(`trial_record`) + 라이선스 캐시
//!   (`license_cache`). 둘 다 JSON 바이트를 `kSecValueData` 로 담는다.
//! - `kSecAttrSynchronizable = false`(§3.5 "(3)") — iCloud 동기화로 기산점이 계정의
//!   다른 기기로 퍼져 3대 동시 활성 라이선스 모델과 어긋나지 않게.
//! - `kSecAttrAccessibleAfterFirstUnlock` — 잠금 해제 후(로그인 항목 포함) 읽을 수 있다.

#[cfg(not(target_os = "macos"))]
use std::marker::PhantomData;

#[cfg(target_os = "macos")]
mod macos_impl {
    // `kSec*` 접근자 이름은 Apple 의 C 상수 심볼명(kSecClass, kSecAttrService, …)과
    // 1:1 로 맞춘 것이다 — 원문 심볼 → Rust 접근자의 대응을 코드에서 직접 읽을 수
    // 있게 하려는 의도. snake_case 로 바꾸면 그 대응이 깨진다.
    #![allow(non_snake_case)]

    use objc2_core_foundation::{
        CFBoolean, CFData, CFDictionary, CFMutableDictionary, CFRetained, CFString, CFType,
    };
    use std::sync::Mutex;

    use ultrakey_license::{CacheStore, LicenseCache, TrialRecord, TrialStore};

    const SERVICE_TRIAL: &str = "com.ultrakey.license.trial";
    const SERVICE_CACHE: &str = "com.ultrakey.license.cache";
    const ACCOUNT_TRIAL: &str = "trial_record";
    const ACCOUNT_CACHE: &str = "license_cache";

    /// `TrialStore`/`CacheStore` 의 Keychain 구현.
    ///
    /// `_guard: Mutex<()>` 는 `SecItem*` FFI 호출을 직렬화한다. 값은 프로세스 밖
    /// Keychain 에 두므로 트레이트의 `&self` 로 읽고 쓴다.
    pub struct KeychainStore {
        _guard: Mutex<()>,
    }

    impl KeychainStore {
        pub fn new() -> Self {
            Self {
                _guard: Mutex::new(()),
            }
        }
    }

    impl Default for KeychainStore {
        fn default() -> Self {
            Self::new()
        }
    }

    // ── TrialStore ──────────────────────────────────────────────────────────

    impl TrialStore for KeychainStore {
        fn read(&self) -> Option<TrialRecord> {
            let bytes = read_item(SERVICE_TRIAL, ACCOUNT_TRIAL)?;
            serde_json::from_slice(&bytes).ok()
        }

        fn write(&self, record: TrialRecord) {
            // 직렬화 실패는 저장을 건너뛴다 — Keychain 에 잘못된 기록을 안 남긴다.
            if let Ok(json) = serde_json::to_vec(&record) {
                write_item(SERVICE_TRIAL, ACCOUNT_TRIAL, &json);
            }
        }
    }

    // ── CacheStore ──────────────────────────────────────────────────────────

    impl CacheStore for KeychainStore {
        fn read_cache(&self) -> Option<LicenseCache> {
            let bytes = read_item(SERVICE_CACHE, ACCOUNT_CACHE)?;
            serde_json::from_slice(&bytes).ok()
        }

        fn write_cache(&self, cache: &LicenseCache) {
            if let Ok(json) = serde_json::to_vec(cache) {
                write_item(SERVICE_CACHE, ACCOUNT_CACHE, &json);
            }
        }

        fn clear_cache(&self) {
            delete_item(SERVICE_CACHE, ACCOUNT_CACHE);
        }
    }

    // ── 내부 FFI 헬퍼 ───────────────────────────────────────────────────────

    /// 이기종 CF 값을 담는 dict 의 raw CFDictionary 포인터.
    fn dict_raw(dict: &CFRetained<CFMutableDictionary<CFString, CFType>>) -> *const CFDictionary {
        // CFRetained::as_ptr 는 NonNull<CFMutableDictionary> 를 주고, CF·CFMutable
        // 는 toll-free bridged 라 같은 옵스포크를 가리킨다.
        CFRetained::as_ptr(dict).as_ptr().cast()
    }

    /// CFData 의 바이트를 `Vec<u8>` 로 복사한다.
    fn read_cfdata_bytes(data: &CFData) -> Option<Vec<u8>> {
        let len = data.length() as usize;
        let mut buf = vec![0u8; len];
        // objc2-core-foundation 의 CFData::bytes 는 (range, buffer) 두 인자를 요구한다.
        // 근거: `CFData.h` — `void CFDataGetBytes(CFDataRef, CFRange, UInt8 *);`
        let range = objc2_core_foundation::CFRange {
            location: 0,
            length: len as isize,
        };
        unsafe { data.bytes(range, buf.as_mut_ptr()) };
        Some(buf)
    }

    /// generic password 를 읽는다. 항목 없음·손상이면 `None`.
    fn read_item(service: &str, account: &str) -> Option<Vec<u8>> {
        let query = secure_item_query(service, account, true);
        let mut result: *mut std::ffi::c_void = std::ptr::null_mut();
        // SAFETY: `query` 는 유효한 CFDictionary 를 가리킨다(dict_raw). `result` 는
        // CF_RETURNS_RETAINED 수신지 — 성공 시 우리가 소유권을 받는다.
        let status =
            unsafe { crate::ffi::SecItemCopyMatching(dict_raw(&query), &mut result) };
        if status != 0 || result.is_null() {
            return None;
        }
        // SAFETY: 성공 시 `result` 는 CF_RETURNS_RETAINED CFData 소유권을 넘겨준다.
        let ptr = std::ptr::NonNull::new(result as *mut CFType)?;
        let retained = unsafe { CFRetained::from_raw(ptr) };
        let data = retained.downcast::<CFData>().ok()?;
        read_cfdata_bytes(&data)
    }

    /// 없으면 `SecItemAdd`, 있으면 `SecItemUpdate` 로 기록한다.
    fn write_item(service: &str, account: &str, bytes: &[u8]) {
        let value_data = CFData::from_bytes(bytes);

        // 1) 항목이 이미 있으면 update.
        let query = secure_item_query(service, account, false);
        let update = new_dict();
        update.set(kSecValueData(), &value_data);
        // SAFETY: `query`·`update` 는 각각 유효한 CFDictionary 다(dict_raw).
        let updated =
            unsafe { crate::ffi::SecItemUpdate(dict_raw(&query), dict_raw(&update)) };
        if updated == 0 {
            return;
        }

        // 2) 없으면 add(update 실패는 항목 미존재 — errSecItemNotFound).
        let attributes = secure_item_query(service, account, false);
        attributes.set(kSecValueData(), &value_data);
        // SAFETY: `attributes` 는 유효한 CFDictionary 다(dict_raw).
        unsafe { crate::ffi::SecItemAdd(dict_raw(&attributes), std::ptr::null_mut()) };
    }

    fn delete_item(service: &str, account: &str) {
        let query = secure_item_query(service, account, true);
        // SAFETY: `query` 는 유효한 CFDictionary 다(dict_raw).
        unsafe { crate::ffi::SecItemDelete(dict_raw(&query)) };
    }

    /// 공통 query dict — class/서비스/계정/접근정책/동기화(끔)/returnData.
    fn secure_item_query(
        service: &str,
        account: &str,
        return_data: bool,
    ) -> CFRetained<CFMutableDictionary<CFString, CFType>> {
        let dict = new_dict();
        dict.set(kSecClass(), kSecClassGenericPassword());
        dict.set(kSecAttrService(), &CFString::from_str(service));
        dict.set(kSecAttrAccount(), &CFString::from_str(account));
        // §3.5: iCloud Keychain 동기화를 명시적으로 끈다.
        dict.set(kSecAttrSynchronizable(), CFBoolean::new(false));
        // 잠금 해제 후 접근 가능(log-in item 포함).
        dict.set(kSecAttrAccessible(), kSecAttrAccessibleAfterFirstUnlock());
        if return_data {
            dict.set(kSecReturnData(), CFBoolean::new(true));
        }
        dict
    }

    fn new_dict() -> CFRetained<CFMutableDictionary<CFString, CFType>> {
        CFMutableDictionary::empty()
    }

    // ── Security 데이터 심볼 접근 (Option -> &CFString) ─────────────────────

    fn kSecClass() -> &'static CFString {
        unsafe { crate::ffi::kSecClass }.expect("Security kSecClass")
    }
    fn kSecClassGenericPassword() -> &'static CFString {
        unsafe { crate::ffi::kSecClassGenericPassword }.expect("Security kSecClassGenericPassword")
    }
    fn kSecAttrService() -> &'static CFString {
        unsafe { crate::ffi::kSecAttrService }.expect("Security kSecAttrService")
    }
    fn kSecAttrAccount() -> &'static CFString {
        unsafe { crate::ffi::kSecAttrAccount }.expect("Security kSecAttrAccount")
    }
    fn kSecAttrAccessible() -> &'static CFString {
        unsafe { crate::ffi::kSecAttrAccessible }.expect("Security kSecAttrAccessible")
    }
    fn kSecAttrAccessibleAfterFirstUnlock() -> &'static CFString {
        unsafe { crate::ffi::kSecAttrAccessibleAfterFirstUnlock }
            .expect("Security kSecAttrAccessibleAfterFirstUnlock")
    }
    fn kSecAttrSynchronizable() -> &'static CFString {
        unsafe { crate::ffi::kSecAttrSynchronizable }.expect("Security kSecAttrSynchronizable")
    }
    fn kSecValueData() -> &'static CFString {
        unsafe { crate::ffi::kSecValueData }.expect("Security kSecValueData")
    }
    fn kSecReturnData() -> &'static CFString {
        unsafe { crate::ffi::kSecReturnData }.expect("Security kSecReturnData")
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::KeychainStore;

/// 비-macOS 스텁 — 컴파일만 통과한다.
#[cfg(not(target_os = "macos"))]
pub struct KeychainStore(PhantomData<()>);
#[cfg(not(target_os = "macos"))]
impl KeychainStore {
    pub fn new() -> Self {
        Self(PhantomData)
    }
}
