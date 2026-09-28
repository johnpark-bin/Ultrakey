//! F-06 트랙패드 원시 멀티터치 — 비공개 `MultitouchSupport.framework` 경계.
//!
//! `docs/spec/trackpad-hyper-gesture.md` §6.2·§7 이 판정한 "손수 작성 비공개 FFI"의
//! 유일한 구현 자리다(architecture.md §1 — 모든 `unsafe` 는 이 크레이트에만).
//!
//! ⭐⭐ **링크 타임 의존이 아니라 `dlopen` 동적 로드로 쓴다.** 명세 §5 항목 9·§8 이
//! 요구하는 격하(degradable) 설계의 핵심이다: `#[link]` 로 정적 링크하면 프레임워크가
//! 없거나 심벌이 사라진 OS 에서 **프로세스 자체가 기동하지 못한다.** `dlopen`+`dlsym`
//! 은 심벌 하나라도 실패하면 `Err` 로 돌려줘 이 기능만 조용히 비활성화할 수 있다 —
//! hyper 물리 키 경로(F-05)는 무영향이다(§8 수용 기준).
//!
//! ## 시그니처의 근거 (2026-09-01 실측 + 커뮤니티 역공학, 이슈 #63 Phase 1)
//!
//! 공개 헤더가 없어 컴파일러가 검증해 주지 않는다. 아래 선언의 근거:
//!
//! 1. **런타임 심벌 실측** — 이 기기(macOS 26.5)에서 `dlsym` 으로 명세 §6.2 의 8개
//!    심벌 전량과 정리 함수군(`MTDeviceStop`·`MTDeviceRelease`·
//!    `MTUnregisterContactFrameCallback`·`MTDeviceGetDeviceID`)의 존재를 확인했다.
//! 2. `asmagill/hs._asm.undocumented.touchdevice` `MultitouchSupport.h` — 함수
//!    시그니처 원형(`CFArrayRef MTDeviceCreateList(void)`, `OSStatus MTDeviceStart
//!    (MTDeviceRef, MTRunMode)` 등)과 out-파라미터 관례. "Tested against 10.12.6".
//! 3. `calftrail/Touch` · `lokxii/Mac-trackpad-mapper` · `ggbond268/MacTools` ·
//!    `INRIA/libpointing` — `MTTouch` 17필드 레이아웃(순서·타입 일치).
//! 4. `jonas-k/macos-multitouch`(Rust) — 직접 링크와 콜백 슬라이스화
//!    (`from_raw_parts(touches, numTouches)`)의 실동작 사례.
//! 5. `RATDEO/ratremote`(Swift) — `dlopen`+`dlsym` 동적 로드 패턴의 실재 증명.
//! 6. `jnordberg/FingerMgmt` — 최초 공개 사례(명세 §9 #3 이 지목한 fingermgmt 계열).
//!
//! ⭐ 커뮤니티 헤더 간 갈리는 콜백 정수 폭(`numTouches`·`frame`: `size_t` vs
//! `int32`)은 **`int32` 로 채택**한다 — 6헤더 중 4헤더가 `int32` 계열(libpointing·
//! everypinch·jonas-k·ggbond)이고, arm64/x86_64 ABI 에서 `i32` 파라미터는 레지스터
//! 하위 32비트만 읽으므로 실제가 `size_t` 이어도 정확한 값(접촉 수 ≤ 11, 프레임 번호
//! 하위 32비트)을 받는다. 반대 방향(우리가 `size_t` 로 선언, 실제 `int`)은 상위
//! 32비트 오염을 읽을 수 있다 — 비대칭 위험을 피하는 방향이다 `(추정 — 실기기에서만
//! 확정 가능)`.
//!
//! ## 방어 설계 (§7 마지막 항목)
//!
//! - `dlopen`/`dlsym` 실패 → [`LoadError`] — 이 기능만 비활성화(§8). 프로세스는
//!   계속 살고 다른 기능은 무영향이다.
//! - 원시 `MTTouch` 포인터는 콜백 안에서 **즉시** 안전한 값으로 번역되고
//!   ([`TouchFrame`]) 이 경계 밖으로 나가지 않는다. 음수·과대 `numTouches` 는 잘라
//!   버린다.
//! - 판정에 쓰는 필드(`pathIndex`·`stage`·`normalized`·`absolute`·`majorAxis`·
//!   `minorAxis`·`zTotal`)만 읽는다. 뒤쪽 필드(field9 의 float/int 이견 등)는
//!   읽지 않는다 — 읽지 않는 오프셋은 리스크가 없다.
//! - `MTDeviceStart` 실패는 그 장치만의 실패다 — 다른 장치와 이 기능의 존재에는
//!   영향을 주지 않는다.

#[cfg(target_os = "macos")]
pub mod mt {
    use objc2_core_foundation::{CFArray, CFRetained};
    use std::ffi::{c_char, c_int, c_void};
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    /// `MultitouchSupport.framework` 설치 경로. 공개 프레임워크가 아니라
    /// `/System/Library/PrivateFrameworks/` 아래에 있다(§6.2). NUL 종료 — `dlopen`
    /// 이 기대하는 C 문자열이다.
    const FRAMEWORK_PATH: &[u8] =
        b"/System/Library/PrivateFrameworks/MultitouchSupport.framework/MultitouchSupport\0";

    // libSystem 이 항상 링크돼 있어 별도 #[link] 가 필요 없다(ffi.rs 의 mach 선언과
    // 같은 근거).
    extern "C" {
        fn dlopen(filename: *const c_char, flags: c_int) -> *mut c_void;
        fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
        fn dlclose(handle: *mut c_void) -> c_int;
    }
    const RTLD_NOW: c_int = 0x2;

    type MtDeviceRef = *mut c_void;

    /// 프레임 콜백이 담는 접촉 최대 개수. 물리적으로 접촉 11개(10손가락+예비)를 넘는
    /// 프레임은 관찰된 적이 없다 `(추정)` — 여유를 두고 16으로 방어한다.
    pub const TOUCHES_PER_FRAME: usize = 16;

    /// `MTTouch` 구조체의 총 크기(바이트) — 17필드 레이아웃(커뮤니티 헤더 전원 일치):
    /// `frame(i32)@0 timestamp(f64)@8 pathIndex(i32)@12 stage(i32)@16 fingerID(i32)@20
    /// handID(i32)@24 normalized(4×f32)@28 zTotal(f32)@44 field9(4B)@48 angle(f32)@52
    /// majorAxis(f32)@56 minorAxis(f32)@60 absolute(4×f32)@64 field14(i32)@80
    /// field15(i32)@84 zDensity(f32)@88` → 92B, 8바이트 정렬 → 96B. 접촉 원소 사이의
    /// 보폭으로 쓴다.
    const MT_TOUCH_SIZE: usize = 96;

    /// `MTPathStage`/`MTTouchState` 3·4 — 표면에 실제로 닿아 있는 상태
    /// (MakeTouch=새 접촉, Touching=유지). 커뮤니티 헤더 전원이 같은 열거값을 기술한다.
    pub const STAGE_MAKE_TOUCH: i32 = 3;
    pub const STAGE_TOUCHING: i32 = 4;

    /// 프레임 콜백이 번역해 주는 접촉 스냅샷 — 판정에 쓰는 필드만 담는다. 고정 크기
    /// 배열이라 콜백 안에서 힙 할당이 없다.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct TouchFrame {
        pub len: usize,
        pub touches: [TouchPoint; TOUCHES_PER_FRAME],
    }

    /// `MTTouch` 한 개를 안전하게 번역한 접촉 스냅샷.
    #[derive(Debug, Clone, Copy, PartialEq)]
    pub struct TouchPoint {
        /// `pathIndex`("P") — 터치의 수명 동안 안정적인 식별자(커뮤니티 헤더 공통 주석).
        pub path_index: i32,
        /// `stage` — `MakeTouch`(3)·`Touching`(4)만 유효 접촉이다.
        pub stage: i32,
        /// 표면 정규 좌표(0..1) — `normalizedVector`.
        pub x: f64,
        pub y: f64,
        /// 절대 좌표(mm — `absoluteVector`, 커뮤니티 헤더 공통 주석).
        pub mm_x: f64,
        pub mm_y: f64,
        /// 타원 주축 — 손바닥(큰 접촉) 거부에 쓴다.
        pub major_axis: f64,
        /// 타원 부축.
        pub minor_axis: f64,
        /// `zTotal` — 접촉 품질(0..1). 스침(가벼운 접촉) 거부에 쓴다.
        pub z_total: f64,
    }

    /// 프레임 콜백 — 멀티터치 런루프 스레드에서 호출된다.
    /// ⛔ 이 콜백 안에서 락 대기·동기 I/O·할당을 하지 않는다(엔진 콜백과 같은 예산).
    pub type FrameCallback = Arc<dyn Fn(&TouchFrame) + Send + Sync>;

    /// 심벌 로드 실패 사유 — 이 값이 오면 호출자는 이 기능만 격하한다(§8).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
    pub enum LoadError {
        #[error("dlopen(MultitouchSupport) failed — private framework unavailable")]
        FrameworkUnavailable,
        #[error("required MultitouchSupport symbol is missing: {0}")]
        SymbolMissing(&'static str),
    }

    /// 스트림 시작 실패 사유 — **그 장치만의** 문제다(다른 장치·기능에는 무영향).
    #[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
    pub enum StartError {
        #[error("MTDeviceCreateFromDeviceID returned null")]
        DeviceUnavailable,
        #[error("MTRegisterContactFrameCallbackWithRefcon failed")]
        RegisterFailed,
        #[error("MTDeviceStart failed")]
        StartFailed,
    }

    /// 열거된 장치 하나의 스냅샷.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct DeviceSnapshot {
        pub device_id: u64,
        /// 내장 트랙패드 여부(`MTDeviceIsBuiltIn`) — §3.5 `hasBuiltInTrackpad` 대응.
        pub built_in: Option<bool>,
        /// Force Touch 지원 여부(`MTDeviceSupportsForce`) — §3.5 `forceTouchConnected` 대응.
        pub supports_force: Option<bool>,
        /// 표면 센서 치수 — 진단·로그용. 판정은 정규 좌표·mm 를 쓰므로 실패해도
        /// 이 장치를 거부하지 않는다(원문 실패 문자열 `Unable to obtain device
        /// dimensions` 대응 — 로그만 남긴다).
        pub surface: Option<(c_int, c_int)>,
        /// 장치 세대/모델군(`MTDeviceGetFamilyID`) — 진단·로그용(§9 #3 미확정).
        pub family_id: Option<i32>,
    }

    type CreateListFn = unsafe extern "C" fn() -> *mut c_void;
    type CreateFromDeviceIdFn = unsafe extern "C" fn(u64) -> MtDeviceRef;
    type DeviceReleaseFn = unsafe extern "C" fn(MtDeviceRef);
    type DeviceStartFn = unsafe extern "C" fn(MtDeviceRef, c_int) -> c_int;
    type DeviceStopFn = unsafe extern "C" fn(MtDeviceRef) -> c_int;
    type DeviceIsBuiltInFn = unsafe extern "C" fn(MtDeviceRef) -> bool;
    type DeviceSupportsForceFn = unsafe extern "C" fn(MtDeviceRef) -> bool;
    type DeviceGetFamilyIdFn = unsafe extern "C" fn(MtDeviceRef, *mut c_int) -> c_int;
    type DeviceGetDeviceIdFn = unsafe extern "C" fn(MtDeviceRef, *mut u64) -> c_int;
    type DeviceGetDimensionsFn = unsafe extern "C" fn(MtDeviceRef, *mut c_int, *mut c_int) -> c_int;
    /// `MTFrameCallbackWithRefconFunction` — 커뮤니티 헤더 전원이 같은 인자 순서를
    /// 기술한다: `(device, touches[], numTouches, timestamp, frame, refcon)`.
    type ContactFrameFn = unsafe extern "C" fn(
        device: MtDeviceRef,
        touches: *const c_void,
        num_touches: c_int,
        timestamp: f64,
        frame: c_int,
        refcon: *mut c_void,
    );
    /// ⭐ `MTRegisterContactFrameCallbackWithRefcon(MTDeviceRef, callback, void*) → BOOL`.
    /// 반환형: asmagill·lokxii 계열 헤더는 `BOOL`, jonas-k(Rust)는 `void` 로 기술한다.
    /// **BOOL 을 채택**한다 — 실제가 `void` 여도 반환값을 무시하는 것은 안전하고,
    /// 실제가 `BOOL` 인데 `void` 로 선언하면 실패를 감지할 수 없다. 안전 쪽으로 고른다.
    type RegisterFn = unsafe extern "C" fn(MtDeviceRef, ContactFrameFn, *mut c_void) -> bool;
    /// `MTUnregisterContactFrameCallback(MTDeviceRef, callback) → BOOL`(asmagill) —
    /// "Make sure to use the correct unregistration function — the callback is stored
    /// in a different location depending upon the callback type"(원문 주석). refcon
    /// 변형으로 등록했다면 이 함수로 해제한다(같은 이름의 unreg 쌍).
    type UnregisterFn = unsafe extern "C" fn(MtDeviceRef, ContactFrameFn) -> bool;

    /// 로드된 비공개 API 세트. 필요한 심벌 전부가 로드 시점에 검증돼 있다 —
    /// 이후 호출은 실패 심벌이 없음을 보장한다.
    pub struct MultitouchApi {
        handle: *mut c_void,
        device_create_list: CreateListFn,
        device_create_from_device_id: CreateFromDeviceIdFn,
        device_release: DeviceReleaseFn,
        device_start: DeviceStartFn,
        device_stop: DeviceStopFn,
        device_is_built_in: DeviceIsBuiltInFn,
        device_supports_force: DeviceSupportsForceFn,
        /// 명세 §6.2 심볼 — 세대별 분기의 근거가 아직 없어(§9 #3) 열거 스냅샷 로그에만
        /// 쓴다. 심벌 존재 검증 자체가 로드 절차의 일부다.
        device_get_family_id: DeviceGetFamilyIdFn,
        device_get_device_id: DeviceGetDeviceIdFn,
        device_get_sensor_surface_dimensions: DeviceGetDimensionsFn,
        register_contact_frame_callback_with_refcon: RegisterFn,
        unregister_contact_frame_callback: UnregisterFn,
    }

    // SAFETY: dlopen 핸들과 함수 포인터는 스레드에 종속되지 않는 정적 값들이다.
    // 모든 메서드는 `&self` 로 이 값들을 읽기만 한다.
    unsafe impl Send for MultitouchApi {}
    unsafe impl Sync for MultitouchApi {}

    impl MultitouchApi {
        /// 비공개 프레임워크를 열고 필요한 심벌 전부를 해석한다. 하나라도 실패하면
        /// `Err` — 호출자는 이 기능만 비활성화한다(§8 격하).
        pub fn load() -> Result<Arc<Self>, LoadError> {
            // SAFETY: `FRAMEWORK_PATH` 는 NUL 종료 정적 경로 문자열이다.
            let handle = unsafe {
                dlopen(FRAMEWORK_PATH.as_ptr() as *const c_char, RTLD_NOW)
            };
            if handle.is_null() {
                return Err(LoadError::FrameworkUnavailable);
            }

            macro_rules! sym {
                ($name:literal, $ty:ty) => {{
                    // SAFETY: `$name` 은 NUL 종료 정적 심벌 이름이다.
                    let p =
                        unsafe { dlsym(handle, concat!($name, "\0").as_ptr() as *const c_char) };
                    if p.is_null() {
                        return Err(LoadError::SymbolMissing($name));
                    }
                    // SAFETY: 심벌 존재를 확인했고 타입은 모듈 문서의 출처 목록(6개
                    // 독립 헤더)과 정합시켰다. 레이아웃이 어긋난 OS 에서는 이 기능만
                    // 격하된다(§8) — 나머지 기능은 무영향이다.
                    unsafe { std::mem::transmute::<*mut c_void, $ty>(p) }
                }};
            }

            Ok(Arc::new(MultitouchApi {
                device_create_list: sym!("MTDeviceCreateList", CreateListFn),
                device_create_from_device_id: sym!(
                    "MTDeviceCreateFromDeviceID",
                    CreateFromDeviceIdFn
                ),
                device_release: sym!("MTDeviceRelease", DeviceReleaseFn),
                device_start: sym!("MTDeviceStart", DeviceStartFn),
                device_stop: sym!("MTDeviceStop", DeviceStopFn),
                device_is_built_in: sym!("MTDeviceIsBuiltIn", DeviceIsBuiltInFn),
                device_supports_force: sym!("MTDeviceSupportsForce", DeviceSupportsForceFn),
                device_get_family_id: sym!("MTDeviceGetFamilyID", DeviceGetFamilyIdFn),
                device_get_device_id: sym!("MTDeviceGetDeviceID", DeviceGetDeviceIdFn),
                device_get_sensor_surface_dimensions: sym!(
                    "MTDeviceGetSensorSurfaceDimensions",
                    DeviceGetDimensionsFn
                ),
                register_contact_frame_callback_with_refcon: sym!(
                    "MTRegisterContactFrameCallbackWithRefcon",
                    RegisterFn
                ),
                unregister_contact_frame_callback: sym!(
                    "MTUnregisterContactFrameCallback",
                    UnregisterFn
                ),
                handle,
            }))
        }

        /// 현재 연결된 모든 멀티터치 장치(트랙패드 + Magic Mouse, §3.5)를 열거한다.
        ///
        /// ⭐ 장치 핸들을 이 목록에서 소비하지 않는다 — 항상 `device_id` 로 새로
        /// 만든다. 열거와 스트림 수명이 분리되어 재시작(watchdog)·재구성이 단순해진다.
        pub fn enumerate(&self) -> Vec<DeviceSnapshot> {
            // SAFETY: `MTDeviceCreateList` 는 CF_RETURNS_RETAINED 규약의 CFArray 를
            // 돌려준다(커뮤니티 헤더 전원이 `CFArrayRef` 반환으로 기술).
            let raw = unsafe { (self.device_create_list)() };
            if raw.is_null() {
                return Vec::new();
            }
            // SAFETY: `raw` 는 방금 받은 소유 CFArray 포인터다.
            let array = match NonNull::new(raw as *mut CFArray) {
                Some(p) => unsafe { CFRetained::from_raw(p) },
                None => return Vec::new(),
            };

            let mut out = Vec::new();
            let count = array.count().max(0);
            for idx in 0..count {
                // SAFETY: `idx < count` — 배열 원소 접근 계약.
                let device = unsafe { array.value_at_index(idx) } as MtDeviceRef;
                if device.is_null() {
                    continue;
                }
                let Some(device_id) = self.device_id_of(device) else {
                    continue;
                };
                out.push(DeviceSnapshot {
                    device_id,
                    built_in: Some(unsafe { (self.device_is_built_in)(device) }),
                    supports_force: Some(unsafe { (self.device_supports_force)(device) }),
                    surface: self.surface_dimensions(device),
                    family_id: self.family_id(device),
                });
            }
            out
        }

        /// 장치를 열고 프레임 콜백을 등록해 스트림을 시작한다.
        ///
        /// ⭐ 콜백은 순수 Rust `extern "C" fn` 트램폴린이다(§7 — 별도 트램폴린 파일
        /// 불요). `refcon` 으로 컨텍스트를 전달해 여러 장치를 하나의 트램폴린으로
        /// 처리한다. `MTDeviceStart` 가 실패하면 등록 해제 후 그 장치만 포기한다.
        pub fn start_session(
            self: &Arc<Self>,
            device_id: u64,
            on_frame: FrameCallback,
        ) -> Result<MtDeviceSession, StartError> {
            // SAFETY: 검증된 심벌. 반환 참조 하나는 세션이 release 한다(Create 규약).
            let raw = unsafe { (self.device_create_from_device_id)(device_id) };
            if raw.is_null() {
                return Err(StartError::DeviceUnavailable);
            }
            let owned = OwnedDevice {
                raw,
                release: self.device_release,
            };

            let shared = Arc::new(SessionShared {
                on_frame,
                last_frame_ms: AtomicU64::new(0),
            });
            let refcon = Box::into_raw(Box::new(Refcon {
                shared: Arc::clone(&shared),
            }));

            // SAFETY: 검증된 심벌 + 유효 핸들 + 유효한 힙 refcon. 트램폴린은 이
            // 포인터가 회수되기 전(unregister 이전)에만 불린다.
            let registered = unsafe {
                (self.register_contact_frame_callback_with_refcon)(
                    owned.raw,
                    contact_frame_trampoline,
                    refcon as *mut c_void,
                )
            };
            if !registered {
                drop(owned);
                // SAFETY: 아직 아무도 이 refcon 을 받지 못했다 — 우리가 만든 Box 를
                // 회수한다.
                drop(unsafe { Box::from_raw(refcon) });
                return Err(StartError::RegisterFailed);
            }

            // MTRunModeLessVerbose(0x10000000) — 콘솔 스팸을 막는 커뮤니티 표준값
            // (everypinch·asmagill 공통; `0` 을 쓴 jonas-k 사례와 관측 차이 보고는
            // 없다 `(추정)`). 실패는 그 장치만의 문제다.
            // SAFETY: 검증된 심벌 + 유효 핸들.
            let status = unsafe { (self.device_start)(owned.raw, 0x1000_0000) };
            if status != 0 {
                // SAFETY: 등록이 성공했으므로 트램폴린을 먼저 등록 해제해 재진입을
                // 차단한 뒤 자원을 회수한다(§2.4 수명 규약과 같은 순서).
                unsafe {
                    (self.unregister_contact_frame_callback)(owned.raw, contact_frame_trampoline);
                }
                drop(owned);
                drop(unsafe { Box::from_raw(refcon) });
                return Err(StartError::StartFailed);
            }

            Ok(MtDeviceSession {
                device_id,
                api: Arc::clone(self),
                owned: Some(owned),
                refcon: Some(refcon),
                shared,
            })
        }

        fn device_id_of(&self, device: MtDeviceRef) -> Option<u64> {
            let mut id: u64 = 0;
            // SAFETY: `device` 는 `MTDeviceCreateList` 가 준 유효 핸들이고, out-
            // 파라미터 관례(`OSStatus MTDeviceGetDeviceID(MTDeviceRef, uint64_t*)`)는
            // asmagill 헤더와 Swift 사례(ratremote)가 일치시킨다.
            let status = unsafe { (self.device_get_device_id)(device, &mut id) };
            if status != 0 {
                return None;
            }
            Some(id)
        }

        fn family_id(&self, device: MtDeviceRef) -> Option<i32> {
            let mut family: c_int = 0;
            // SAFETY: out-파라미터 관례 — `OSStatus MTDeviceGetFamilyID
            // (MTDeviceRef, int32_t*)`(asmagill·lokxii 일치). 세대별 분기 근거는
            // 아직 없어(§9 #3) 스냅샷 로그에만 쓴다.
            let status = unsafe { (self.device_get_family_id)(device, &mut family) };
            if status != 0 {
                return None;
            }
            Some(family)
        }

        fn surface_dimensions(&self, device: MtDeviceRef) -> Option<(c_int, c_int)> {
            let (mut w, mut h): (c_int, c_int) = (0, 0);
            // SAFETY: out-파라미터 2개 관례 — `OSStatus MTDeviceGetSensorSurface
            // Dimensions(MTDeviceRef, int*, int*)`(커뮤니티 헤더 전원 일치).
            let status =
                unsafe { (self.device_get_sensor_surface_dimensions)(device, &mut w, &mut h) };
            if status != 0 || w <= 0 || h <= 0 {
                return None;
            }
            Some((w, h))
        }
    }

    impl Drop for MultitouchApi {
        fn drop(&mut self) {
            // SAFETY: 이 타입이 dlopen 핸들의 유일한 소유자다. `Arc` 참조가 모두
            // 사라진 뒤에 Drop 이 불린다.
            unsafe { dlclose(self.handle) };
        }
    }

    /// 절전 복귀·워치독이 쓰는 프레임 콜백 복제본. 세션 재시작 시 다시 넘긴다.
    pub type SessionFrameCallback = FrameCallback;

    /// 살아 있는 장치 스트림 하나. `Drop` 이 "unregister → refcon 회수 → release"
    /// 순서로 정리한다(architecture.md §2.4 의 수명 규약과 동일한 모양).
    pub struct MtDeviceSession {
        device_id: u64,
        api: Arc<MultitouchApi>,
        owned: Option<OwnedDevice>,
        refcon: Option<*mut Refcon>,
        shared: Arc<SessionShared>,
    }

    // SAFETY: 세션은 만든 스레드(리스너 스레드)에서 만들어지고 정리된다. 다만
    // 워치독(같은 스레드)이 `ms_since_last_frame` 을 읽고, 앱이 `Arc<AtomicTrackpad
    // Phase>` 를 공유하므로, 세션 자체를 다른 스레드로 옮길 일은 없다 — 이동은
    // 소유권 이동이므로 안전하다.
    unsafe impl Send for MtDeviceSession {}

    impl MtDeviceSession {
        /// 워치독이 마지막 프레임 수신 이후 경과를 읽는다(§5 항목 6). 프레임을 한 번도
        /// 받지 못했으면 `None` — 재시작 판정의 대상이다.
        pub fn ms_since_last_frame(&self, now_ms: u64) -> Option<u64> {
            let last = self.shared.last_frame_ms.load(Ordering::Acquire);
            if last == 0 {
                None
            } else {
                Some(now_ms.saturating_sub(last))
            }
        }

        pub fn device_id(&self) -> u64 {
            self.device_id
        }

        /// 스트림을 정지한다 — 워치독 재시작(§5 항목 6: 원문 `Touches are not being
        /// detected. Restarting.` 동등 동작)의 전반부다. `Drop` 전에 명시적으로
        /// 부르지 않아도 `Drop` 이 정리한다.
        ///
        /// ⚠️ `MTDeviceStop` 이 존재함은 런타임 심벌 실측으로 확정됐다(Phase 1) —
        /// 대칭 정리 함수군은 SuperKey 의 `nm -u` 에는 없었지만 이 OS 에는 실재한다.
        /// 호출 실패는 로그만 남기고 무시한다 — 스트림 정지 실패는 Drop 의
        /// unregister+release 가 흡수한다.
        pub fn stop_stream(&mut self) {
            if let Some(owned) = self.owned.as_ref() {
                // SAFETY: 검증된 심벌 + 세션이 소유한 유효 핸들
                // (`OSStatus MTDeviceStop(MTDeviceRef)`, asmagill).
                unsafe { (self.api.device_stop)(owned.raw) };
            }
        }
    }

    impl Drop for MtDeviceSession {
        fn drop(&mut self) {
            // SAFETY: 이 세션이 refcon Box 와 장치 참조의 유일한 소유자다. Drop 이후
            // 트램폴린은 다시 불릴 수 없다(unregister 가 먼저 실행되므로).
            unsafe {
                if let Some(owned) = self.owned.take() {
                    (self.api.unregister_contact_frame_callback)(
                        owned.raw,
                        contact_frame_trampoline,
                    );
                }
                if let Some(refcon) = self.refcon.take() {
                    drop(Box::from_raw(refcon));
                }
            }
            // `owned` 를 take 했으므로 `OwnedDevice::drop` 이 이미 release 했다.
        }
    }

    struct SessionShared {
        on_frame: FrameCallback,
        /// 마지막 프레임 수신 시각(epoch ms) — 워치독이 폴링한다(§5 항목 6).
        last_frame_ms: AtomicU64,
    }

    /// 콜백에 넘기는 사용자 컨텍스트(`refcon`). 힙 `Box` 로 두고 세션이 소유한다.
    struct Refcon {
        shared: Arc<SessionShared>,
    }

    /// `MTFrameCallbackWithRefconFunction` 트램폴린 — 순수 Rust `extern "C" fn`
    /// 이다(§7: 트램폴린 파일 불요).
    ///
    /// # Safety
    /// macOS 가 `MTRegisterContactFrameCallbackWithRefcon` 에 등록한 콜백으로서만
    /// 호출한다. `refcon` 은 `start_session` 이 넘긴 `*mut Refcon` 이며, unregister
    /// 이후에는 다시 호출되지 않는다.
    unsafe extern "C" fn contact_frame_trampoline(
        _device: MtDeviceRef,
        touches: *const c_void,
        num_touches: c_int,
        _timestamp: f64,
        _frame: c_int,
        refcon: *mut c_void,
    ) {
        if touches.is_null() || refcon.is_null() {
            // "MT callback with nil values"(SuperKey 원문 실패 문자열, §5 항목 6)에
            // 해당하는 상황 — 조용히 버린다. 로깅하지 않는다: 프레임 스트림은 초당
            // 수십 회라 로그 폭주가 된다(§2.2 정신).
            return;
        }
        if num_touches <= 0 {
            return;
        }
        // ⭐ 범위 방어(§7) — 비정상적으로 큰 개수는 잘라 버린다. 레이아웃이 어긋난
        // OS 에서 읽는 최악의 경우에도 배열 경계 안에 머문다.
        let n = (num_touches as usize).min(TOUCHES_PER_FRAME);

        let ctx = unsafe { &*(refcon as *const Refcon) };
        let mut frame = TouchFrame {
            len: 0,
            touches: [TouchPoint {
                path_index: 0,
                stage: 0,
                x: 0.0,
                y: 0.0,
                mm_x: 0.0,
                mm_y: 0.0,
                major_axis: 0.0,
                minor_axis: 0.0,
                z_total: 0.0,
            }; TOUCHES_PER_FRAME],
        };

        // SAFETY: `touches` 는 `num_touches` 개의 `MTTouch` 배열을 가리킨다는 것이
        // 콜백 계약이다. 우리는 원소 사이를 `MT_TOUCH_SIZE` 보폭으로 건너뛰고, 각
        // 원소의 **앞쪽 72바이트**만 읽는다 — 이 영역의 오프셋은 6개 독립 커뮤니티
        // 헤더가 동일하게 기술한다(모듈 문서 출처 목록). 뒤쪽 필드(field14 이후)는
        // 커뮤니티 헤더 간 이견이 있는 영역이라 읽지 않는다.
        let base = touches as *const u8;
        for i in 0..n {
            let rec = unsafe { base.add(i * MT_TOUCH_SIZE) };
            let point = unsafe { read_touch_prefix(rec) };
            frame.touches[i] = point;
        }
        frame.len = n;

        (ctx.shared.on_frame)(&frame);
        ctx.shared
            .last_frame_ms
            .store(unix_ms(), Ordering::Release);
    }

    /// `MTTouch` 의 앞쪽 필드들만 읽는다. 오프셋 근거(커뮤니티 헤더 전원 일치):
    /// `frame(i32)@0 · timestamp(f64)@8 · pathIndex(i32)@12 · stage(i32)@16 ·
    /// fingerID(i32)@20 · handID(i32)@24 · normalized.x(f32)@28 · normalized.y(f32)@32
    /// · zTotal(f32)@44 · majorAxis(f32)@56 · minorAxis(f32)@60 · absolute.x(f32)@64
    /// · absolute.y(f32)@68`. 이 영역은 6개 독립 헤더가 동일하게 기술한다.
    ///
    /// # Safety
    /// `rec` 은 유효한 `MTTouch` 원소의 시작 주소여야 한다(호출자가 범위 방어를
    /// 이미 했다).
    unsafe fn read_touch_prefix(rec: *const u8) -> TouchPoint {
        // SAFETY: 호출자가 원소 범위 방어를 마친 뒤다.
        unsafe {
            TouchPoint {
                path_index: read_i32(rec, 12),
                stage: read_i32(rec, 16),
                x: f64::from(read_f32(rec, 28)),
                y: f64::from(read_f32(rec, 32)),
                mm_x: f64::from(read_f32(rec, 64)),
                mm_y: f64::from(read_f32(rec, 68)),
                major_axis: f64::from(read_f32(rec, 56)),
                minor_axis: f64::from(read_f32(rec, 60)),
                z_total: f64::from(read_f32(rec, 44)),
            }
        }
    }

    /// # Safety
    /// `base + offset + 4` 까지 읽기 가능해야 한다.
    unsafe fn read_i32(base: *const u8, offset: usize) -> i32 {
        unsafe { (base.add(offset) as *const c_int).read_unaligned() }
    }

    /// # Safety
    /// `base + offset + 4` 까지 읽기 가능해야 한다.
    unsafe fn read_f32(base: *const u8, offset: usize) -> f32 {
        unsafe { (base.add(offset) as *const f32).read_unaligned() }
    }

    /// 현재 시각(epoch 밀리초) — 워치독 기준점.
    fn unix_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    /// `Create` 규약(+1) 참조의 RAII 래퍼 — `Drop` 이 release 한다.
    struct OwnedDevice {
        raw: MtDeviceRef,
        release: unsafe extern "C" fn(MtDeviceRef),
    }

    impl Drop for OwnedDevice {
        fn drop(&mut self) {
            // SAFETY: `MTDeviceCreate*` 계열의 +1 참조를 해제한다(asmagill 헤더의
            // `void MTDeviceRelease(MTDeviceRef)`).
            unsafe { (self.release)(self.raw) };
        }
    }
}

#[cfg(not(target_os = "macos"))]
pub mod mt {
    /// non-macOS 빌드에서도 컴파일되는 스텁(테스트 CI 대비, lib.rs 모듈 문서 규약).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct DeviceSnapshot {
        pub device_id: u64,
        pub built_in: Option<bool>,
        pub supports_force: Option<bool>,
        pub surface: Option<(i32, i32)>,
        pub family_id: Option<i32>,
    }
}

pub use mt::DeviceSnapshot;

#[cfg(target_os = "macos")]
pub use mt::{MultitouchApi, MtDeviceSession, StartError, TouchFrame, TOUCHES_PER_FRAME};
#[cfg(target_os = "macos")]
pub use mt::LoadError;

#[cfg(test)]
mod tests {
    //! 비공개 FFI 자체는 실기기 검증 대상이다(Phase 3 문서화) — 여기서는 격하 게이트의
    //! 순수 성질만 확인한다.

    #[cfg(target_os = "macos")]
    #[test]
    fn load_error_messages_are_english() {
        use crate::multitouch::LoadError;
        // 로그 문자열 규약(log_string_discipline) — 에러 문구도 영어다.
        let e = LoadError::FrameworkUnavailable.to_string();
        assert!(e.contains("unavailable"), "{e}");
        let e = LoadError::SymbolMissing("MTDeviceStart").to_string();
        assert!(e.contains("MTDeviceStart"), "{e}");
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn stub_compiles() {
        let _ = mt::DeviceSnapshot {
            device_id: 0,
            built_in: None,
            supports_force: None,
            surface: None,
        };
    }
}