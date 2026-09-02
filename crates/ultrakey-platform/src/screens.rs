//! ⭐ F-03 — `NSScreen` 열거와 디스플레이 구성 변경(핫플러그) 감지.
//!
//! 대응 명세: `docs/spec/seek-overlay-ui.md` §3.3 · §5 엣지케이스 1 · §6.
//!
//! ## 왜 `screen_capture::active_displays()` 로는 부족한가
//!
//! F-02 의 [`crate::screen_capture::active_displays`] 는 `CGDisplayBounds` 로
//! 기하만 준다. 오버레이는 그 위에 **`backingScaleFactor`** 가 더 필요하다 —
//! 명세 §8 의 수용 기준 하나가 *"서로 다른 배율의 디스플레이가 혼용된
//! 환경에서 각 디스플레이의 하이라이트가 흐려지거나 위치가 어긋나지
//! 않는다"* 이고, 배율을 모르면 그것을 판정할 수도, 로그로 증명할 수도 없다.
//!
//! ## 좌표계 — ⚠️ 여기서 뒤집는다
//!
//! `NSScreen.frame` 은 **주 디스플레이 좌하단 원점, Y 축 상향**이다. 이
//! 저장소의 나머지 전부(F-02 의 `TextCandidate::frame`, Tauri 의 창 위치)는
//! **좌상단 원점, Y 축 하향**이다. 이 모듈이 **경계에서 한 번만** 뒤집어
//! 좌상단 좌표로 내보낸다 — 그래야 변환식이 코드베이스에 하나만 존재한다
//! (F-02 `transform.rs` 가 같은 이유로 변환을 한곳에 가둔 것과 같은 규약).

/// 디스플레이 하나. 좌표는 **전역 화면 좌표(pt, 좌상단 원점)** 로 이미
/// 변환돼 있다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenInfo {
    /// `CGDirectDisplayID` — `NSScreen.deviceDescription[NSScreenNumber]`.
    /// 못 읽으면 `0`.
    pub display_id: u32,
    /// 좌상단 x (pt).
    pub origin_x: f64,
    /// 좌상단 y (pt).
    pub origin_y: f64,
    /// 폭 (pt).
    pub width: f64,
    /// 높이 (pt).
    pub height: f64,
    /// ⭐ `backingScaleFactor` — Retina 는 2.0.
    pub backing_scale: f64,
    /// 주 디스플레이인가 (`NSScreen.screens[0]`).
    pub is_main: bool,
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::ScreenInfo;
    use block2::RcBlock;
    use core::ptr::NonNull;
    use objc2::rc::Retained;
    use objc2::runtime::{NSObjectProtocol, ProtocolObject};
    use objc2_app_kit::{
        NSApplication, NSApplicationDidChangeScreenParametersNotification, NSEvent, NSScreen,
        NSWorkspace,
    };
    use objc2_foundation::{MainThreadMarker, NSNotification, NSNotificationCenter, NSNumber, NSString};

    /// `NSScreen` 목록을 좌상단 원점 좌표로 돌려준다.
    ///
    /// ⚠️ **메인 스레드에서만 부른다.** `NSScreen.screens` 는 메인 스레드
    /// 전용이다. 메인 스레드가 아니면 빈 벡터를 돌려준다(패닉시키지 않는다 —
    /// 오버레이가 못 뜨는 것이 앱이 죽는 것보다 낫다).
    pub fn screens() -> Vec<ScreenInfo> {
        let Some(mtm) = MainThreadMarker::new() else {
            tracing::error!("screens() called off the main thread; returning empty list");
            return Vec::new();
        };
        let all = NSScreen::screens(mtm);
        // ⭐ 전역 좌표계의 Y 를 뒤집으려면 **주 디스플레이의 높이**가 기준이다.
        // `NSScreen.screens[0]` 이 주 디스플레이이고 그 frame.origin 은 (0,0) 다.
        let Some(main) = all.iter().next() else {
            return Vec::new();
        };
        let main_top = main.frame().origin.y + main.frame().size.height;

        all.iter()
            .enumerate()
            .map(|(i, s)| {
                let f = s.frame();
                ScreenInfo {
                    display_id: screen_number(&s),
                    origin_x: f.origin.x,
                    // 좌하단 원점(Y 상향) → 좌상단 원점(Y 하향).
                    origin_y: main_top - (f.origin.y + f.size.height),
                    width: f.size.width,
                    height: f.size.height,
                    backing_scale: s.backingScaleFactor(),
                    is_main: i == 0,
                }
            })
            .collect()
    }

    /// 마우스 커서의 현재 위치 — **전역 화면 좌표(pt, 좌상단 원점)**.
    ///
    /// 검색 바의 **최초 기본 위치**(§3.4)가 "호출 시점에 커서가 있는
    /// 디스플레이의 상단부 중앙" 이라 필요하다.
    ///
    /// ⚠️ `NSEvent.mouseLocation` 은 `NSScreen` 과 같은 좌하단 원점 좌표이므로
    /// 여기서 같은 식으로 뒤집는다.
    pub fn mouse_location() -> Option<(f64, f64)> {
        let mtm = MainThreadMarker::new()?;
        let all = NSScreen::screens(mtm);
        let main = all.iter().next()?;
        let main_top = main.frame().origin.y + main.frame().size.height;
        let p = NSEvent::mouseLocation();
        Some((p.x, main_top - p.y))
    }

    /// ⭐(이슈 #95) **포커스된 윈도우가 있는 디스플레이**의 `CGDirectDisplayID`.
    ///
    /// `NSScreen.mainScreen` 은 AppKit 정의상 *"현재 키보드·마우스 이벤트를
    /// 받고 있는 윈도우가 속한 스크린"* — 즉 포커스된 윈도우의 디스플레이
    /// 다. 검색 바 표시 디스플레이 우선순위(§3.4)의 ② 신호로 쓴다: 마우스가
    /// 다른 디스플레이에 멈춰 있어도 사용자의 실제 작업 위치를 가리킨다.
    ///
    /// ⭐ AX(`AXUIElement`) 경로를 쓰지 않는 이유: `NSScreen.main` 이 같은
    /// 정보를 권한 리스크 없이 준다. 스냅샷 시점(세션 열림 직전)에는 검색
    /// 바가 아직 키 윈도우가 아니라(§1 `canBecomeKey = false`) 이 값은 사용자
    /// 앱의 포커스 위치다.
    ///
    /// ⚠️ **메인 스레드에서만 부른다.** 메인 스레드가 아니거나 스크린이
    /// 없으면 `None`. `NSScreenNumber` 를 못 읽으면 `Some(0)` — 호출 쪽의
    /// 디스플레이 선택 로직이 매칭 실패로 폴백 처리한다.
    pub fn focused_display_id() -> Option<u32> {
        let mtm = MainThreadMarker::new()?;
        let main = NSScreen::mainScreen(mtm)?;
        Some(screen_number(&main))
    }

    /// ⭐ 시스템이 **다크 모드**인가 (`NSApp.effectiveAppearance`).
    ///
    /// 명세 §3.5 마지막 두 행이 요구하는 팔레트 전환의 입력이다. ⚠️ 배경색을
    /// 분석하는 것이 아니라 **시스템 외관 모드**를 본다 — 오버레이가 얹히는
    /// 배경은 임의의 앱 화면이라 실시간 배경 분석은 범위 밖이다(§3.5).
    ///
    /// 메인 스레드가 아니면 `None`.
    pub fn is_dark_appearance() -> Option<bool> {
        let mtm = MainThreadMarker::new()?;
        let app = NSApplication::sharedApplication(mtm);
        let name = app.effectiveAppearance().name();
        Some(name.to_string().to_ascii_lowercase().contains("dark"))
    }

    /// ⭐ `NSWorkspace.accessibilityDisplayShouldReduceMotion` (§3.5 "축소된 모션").
    ///
    /// 참이면 하이라이트·연결선의 등장/이동 애니메이션을 생략하고 즉시
    /// 스냅 전환해야 한다.
    pub fn should_reduce_motion() -> bool {
        NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    }

    /// `deviceDescription[@"NSScreenNumber"]` → `CGDirectDisplayID`.
    fn screen_number(s: &NSScreen) -> u32 {
        let desc = s.deviceDescription();
        let key = NSString::from_str("NSScreenNumber");
        let Some(obj) = desc.objectForKey(&key) else {
            return 0;
        };
        obj.downcast::<NSNumber>().map(|n| n.unsignedIntValue()).unwrap_or(0)
    }

    type Token = Retained<ProtocolObject<dyn NSObjectProtocol>>;

    /// `NSApplicationDidChangeScreenParametersNotification` 구독 토큰.
    /// `Drop` 에서 `removeObserver:` 한다.
    pub struct ScreenObserver {
        center: Retained<NSNotificationCenter>,
        token: Option<Token>,
    }

    impl ScreenObserver {
        /// 디스플레이 구성이 바뀔 때마다 `on_change` 를 부른다(§5 엣지케이스 1).
        ///
        /// ⚠️ 콜백은 **메인 스레드**에서 온다 — 그 안에서 [`screens`] 를
        /// 그대로 부를 수 있다.
        pub fn start(on_change: impl Fn() + Send + Sync + 'static) -> Self {
            let center = NSNotificationCenter::defaultCenter();
            let block = RcBlock::new(move |_note: NonNull<NSNotification>| {
                on_change();
            });
            // SAFETY: `block` 은 호출 동안 유효하고 AppKit 이 즉시 복사본을
            // 만든다(`workspace.rs` 의 같은 패턴).
            let token = unsafe {
                center.addObserverForName_object_queue_usingBlock(
                    Some(NSApplicationDidChangeScreenParametersNotification),
                    None,
                    None,
                    &block,
                )
            };
            Self { center, token: Some(token) }
        }
    }

    impl Drop for ScreenObserver {
        fn drop(&mut self) {
            if let Some(token) = self.token.take() {
                // SAFETY: 우리가 등록한 토큰을 정확히 한 번 해제한다.
                unsafe { self.center.removeObserver(token.as_ref()) };
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
mod macos_impl {
    use super::ScreenInfo;

    pub fn screens() -> Vec<ScreenInfo> {
        Vec::new()
    }

    pub fn mouse_location() -> Option<(f64, f64)> {
        None
    }

    pub fn focused_display_id() -> Option<u32> {
        None
    }

    pub fn is_dark_appearance() -> Option<bool> {
        None
    }

    pub fn should_reduce_motion() -> bool {
        false
    }

    /// macOS 아닌 곳의 자리표시자.
    pub struct ScreenObserver;

    impl ScreenObserver {
        pub fn start(_on_change: impl Fn() + Send + Sync + 'static) -> Self {
            Self
        }
    }
}

pub use macos_impl::{
    focused_display_id, is_dark_appearance, mouse_location, screens, should_reduce_motion,
    ScreenObserver,
};
