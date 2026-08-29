# Rust / Tauri × macOS 역량 조사 노트

> 조사일: 2026-08-30 · 버전은 **crates.io API 로 직접 조회해 확인**했다 (조회일 기준 최신 stable).
> 이 문서는 각 기능 명세의 **"필요한 플랫폼 API"** 와 **"구현 접근"** 절이 참조하는 공통 근거다.
> 종합 판정과 결론 표는 [`docs/spec/platform-constraints.md`](../spec/platform-constraints.md) 에 있다.

## 1. 크레이트 실사 결과 (crates.io 직접 조회)

`https://crates.io/api/v1/crates/<name>` 로 조회한 `max_stable_version` 이다. **존재하지 않는 크레이트는 이 표에 없다.**

### 1.1 Objective-C 인터롭 기반

| 크레이트 | 버전 | 역할 |
| :--- | :--- | :--- |
| `objc2` | **0.6.4** | Objective-C 런타임 바인딩. 이 생태계의 기반 |
| `objc2-foundation` | 0.3.2 | Foundation |
| `objc2-app-kit` | 0.3.2 | AppKit — `NSWindow`, `NSStatusItem`, `NSScreen`, `NSApplication` |
| `objc2-core-foundation` | 0.3.2 | Core Foundation |
| `objc2-core-graphics` | 0.3.2 | Core Graphics — `CGEvent`, `CGEventTap`, `CGDisplay` |
| `objc2-application-services` | 0.3.2 | Application Services (Accessibility `AX*` 가 여기 속함) |
| `objc2-vision` | 0.3.2 | **Vision** — `VNRecognizeTextRequest`, `VNRecognizedTextObservation` |
| `objc2-screen-capture-kit` | 0.3.2 | **ScreenCaptureKit** |
| `objc2-quartz-core` | 0.3.2 | QuartzCore / Core Animation |
| `objc2-service-management` | 0.3.2 | ServiceManagement — `SMAppService` |
| `objc2-core-video` | 0.3.2 | Core Video — `CVPixelBuffer` (SCK 프레임 수신에 필요) |

> `objc2-*` 프레임워크 크레이트는 **헤더 자동 생성 바인딩**이다. 해당 프레임워크의 공개 API 는 원칙적으로 전부 노출되지만, 안전한 상위 래퍼는 없고 호출부는 `unsafe` 다.
> `cocoa`, `objc`, `core-foundation`(servo 계열)은 **`objc2` 생태계로 대체되어 사실상 유지보수 종료 상태**다. 신규 코드에서 쓰지 않는다.

### 1.2 기능별 상위 크레이트

| 크레이트 | 버전 | 최근 릴리스 | 역할 · 판단 |
| :--- | :--- | :--- | :--- |
| `core-graphics` | 0.25.0 | 2025-05 | `CGEventTap` 안전 래퍼 + `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` |
| `accessibility-sys` | 0.2.0 | 2025-03 | `AXUIElement*` 원시 FFI. `AXIsProcessTrustedWithOptions` 포함 |
| `accessibility` | 0.2.0 | 2025-03 | 위의 얇은 안전 래퍼. 커버리지 제한적 |
| `axuielement` | **0.9.1** | 2026-06 | AX 고수준 안전 API. `AXUIElement`/`AXObserver`/`AXValue`/`AXAttribute`/`AXAction`/`ProcessTrust`. **현재 가장 활발** |
| `macos-accessibility-client` | 0.0.2 | 2026-01 | `application_is_trusted()` / `application_is_trusted_with_prompt()` 만 제공하는 초소형 크레이트 |
| `screencapturekit` | **8.0.1** | 2026-07 | ScreenCaptureKit 고수준 바인딩. macOS 12.3+ |
| `xcap` | 0.9.8 | 2026-08 | 크로스플랫폼 화면 캡처 (구 `screenshots` 의 후속) |
| `rdev` | 0.5.3 | **2023-06** | 전역 키 이벤트. ⚠️ 3년간 릴리스 없음 — 신규 의존 비권장 |
| `global-hotkey` | 0.8.0 | 2026-05 | 전역 단축키 등록 (Tauri 팀 관리). **가로채기가 아니라 등록** |
| `tray-icon` | **0.24.2** | 2026-07 | 메뉴바 아이콘. macOS 에서 `ns_status_item()` 로 `NSStatusItem` 직접 접근 제공 |
| `auto-launch` | 0.6.0 | 2026-01 | 로그인 시 실행 (AppleScript / LaunchAgent 모드) |
| `smappservice-rs` | 0.1.3 | 2025-05 | `SMAppService` 직접 래퍼. **macOS 13+ 전용** |
| `window-vibrancy` | 0.8.0 | 2026-07 | 창 vibrancy 효과 |

### 1.3 Tauri 생태계

| 크레이트 | 버전 | 비고 |
| :--- | :--- | :--- |
| `tauri` | **2.11.5** | v2 stable |
| `tauri-plugin-updater` | 2.10.1 | Tauri 자체 업데이트 (Sparkle 아님) |
| `tauri-plugin-sparkle-updater` | 0.2.5 | 커뮤니티 플러그인. Sparkle 프레임워크 래핑. ⚠️ 0.x, `tauri dev` 에서는 동작 안 함(`.app` 번들 필요) |
| `tauri-plugin-autostart` | 2.5.1 | 내부적으로 `auto-launch` 사용 |
| `tauri-plugin-macos-permissions` | 2.3.0 | Accessibility / Full Disk / 마이크 / 카메라 TCC 확인·요청 |

---

## 2. 역량별 판정

### 2.1 전역 키 이벤트 가로채기 — `CGEventTap`

- **필요한 것**: `CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, callback, userInfo)` — `Default` 옵션이어야 이벤트를 **소비·치환**할 수 있다(`ListenOnly` 는 관찰만).
- **Rust 가능성**: ✅ `core-graphics` 의 `CGEventTap` 또는 `objc2-core-graphics` 의 `CGEventTapCreate`. 콜백에서 `CGEvent` 를 반환/`None`(소비)/치환 가능.
- ⚠️ **런루프 결합**: event tap 은 `CFRunLoopSource` 로 런루프에 붙여야 하고, 그 런루프가 계속 돌아야 한다. Tauri 는 메인 스레드에서 자체 이벤트 루프(`NSApplication` 런루프)를 돌린다 → 같은 런루프에 소스를 추가하거나 전용 스레드의 런루프를 쓴다. 상세 판단은 `platform-constraints.md`.
- ⚠️ **탭 비활성화**: 타임아웃이나 사용자 개입으로 탭이 `kCGEventTapDisabledByTimeout` / `kCGEventTapDisabledByUserInput` 를 받으면 죽는다. **이 이벤트를 받아 `CGEventTapEnable` 로 재활성화하는 로직이 필수**다. SuperKey v1.58 의 "절전 복귀·로그인 시 리매핑 미동작" 이 정확히 이 실패 모드다.
- `rdev` 는 3년째 미유지 + 이벤트 소비 제어가 제한적이라 **비권장**.

### 2.2 Accessibility API — `AXUIElement`

- **필요한 것**: `AXUIElementCreateApplication(pid)` / `AXUIElementCreateSystemWide()`, `AXUIElementCopyAttributeValue` 로 `kAXRoleAttribute`·`kAXTitleAttribute`·`kAXValueAttribute`·`kAXPositionAttribute`·`kAXSizeAttribute`·`kAXChildrenAttribute` 순회, `AXUIElementPerformAction(kAXPressAction)`.
- **Rust 가능성**: ✅ `axuielement` 0.9.1 (고수준) 또는 `accessibility-sys` 0.2.0 (FFI). 필요한 함수는 모두 노출된다.
- ⚠️ **성능**: AX 트리 순회는 **프로세스 간 동기 IPC** 라 느리다. 깊은 트리를 매 키 입력마다 순회하면 UI 가 멈춘다. 캐싱·깊이 제한·비동기화가 필요하다.
- ⚠️ **크래시**: 대상 앱이 응답하지 않으면 블로킹된다. `AXUIElementSetMessagingTimeout` 로 타임아웃을 걸어야 한다. SuperKey v1.19 의 "potential crash with Seek using the Accessibility API" 참고.

### 2.3 화면 캡처

- **ScreenCaptureKit** (macOS 12.3+): `screencapturekit` 8.0.1 또는 `objc2-screen-capture-kit`. 최신·고성능이지만 **비동기 델리게이트 기반**이라 "지금 한 장" 을 받는 코드가 다소 번거롭다. macOS 14+ 의 `SCScreenshotManager` 는 단발 캡처 API 를 제공한다.
- **`CGWindowListCreateImage`**: macOS 14 에서 **deprecated**. 신규 코드에서 쓰지 않는다.
- 대상 최소 버전이 macOS 12.0 인데 ScreenCaptureKit 은 **12.3+** 다 → 12.0–12.2 를 지원하려면 폴백이 필요하거나 최소 버전을 12.3 으로 올려야 한다. **결정 필요 사항.**

### 2.4 OCR — Apple Vision

- **필요한 것**: `VNImageRequestHandler(cgImage:)` + `VNRecognizeTextRequest` (`recognitionLevel = .accurate`, `usesLanguageCorrection`, `recognitionLanguages`), 결과 `VNRecognizedTextObservation` 의 `boundingBox`(정규화 좌표, **좌하단 원점**) 와 `topCandidates(1)`.
- **Rust 가능성**: ✅ `objc2-vision` 0.3.2 로 호출 가능. 단 `unsafe` FFI 수준이고, `CGImage` ↔ Vision 핸들러 연결과 좌표 변환을 직접 작성해야 한다.
- macOS 15+ 에는 Swift 전용 신 API(`RecognizeTextRequest`)가 있으나, **구 `VNRecognizeTextRequest` 는 계속 동작**하며 Objective-C 노출이 있어 Rust 에서 쓸 수 있다.
- 대안(`ocrs`, `tesseract`)은 TCC 가 불필요하지만 정확도·속도에서 Vision 에 못 미치고, 원본이 Vision 을 쓴다고 개발자가 명시했으므로 **Vision 채택**.

### 2.5 TCC 권한

| 권한 | 확인 | 요청 | Info.plist |
| :--- | :--- | :--- | :--- |
| Accessibility | `AXIsProcessTrusted()` | `AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt: true])` | 사용 설명 키 없음 |
| Screen Recording | `CGPreflightScreenCaptureAccess()` | `CGRequestScreenCaptureAccess()` | 없음 |
| Input Monitoring | `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)` | `IOHIDRequestAccess(...)` | 없음 |

- **Rust 가능성**: Accessibility·Screen Recording 은 ✅ 기존 크레이트로 커버. **Input Monitoring 의 `IOHIDCheckAccess`/`IOHIDRequestAccess` 는 전용 크레이트가 없어** `IOKit` 을 직접 링크(`#[link(name="IOKit", kind="framework")]`)하고 `extern "C"` 선언을 손으로 써야 한다.
- ⚠️ **권한 요청은 프롬프트를 한 번만 띄운다.** 사용자가 거부하면 이후 호출은 조용히 실패한다. 시스템 설정으로 유도하는 딥링크(`x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`)가 필요하다.

### 2.6 오버레이 창

Seek 오버레이의 요구사항과 커버리지:

| 요구사항 | Tauri v2 설정 | 네이티브 필요? |
| :--- | :--- | :--- |
| 테두리 없음 | `decorations: false` | 아니오 |
| 투명 배경 | `transparent: true` | 아니오 |
| 항상 위 | `always_on_top: true` | 아니오 |
| 모든 Space 에 표시 | `visible_on_all_workspaces: true` | 아니오 |
| 그림자 제거 | `shadow: false` | 아니오 |
| Dock 아이콘 없음 | `app.set_activation_policy(ActivationPolicy::Accessory)` | 아니오 |
| **클릭 통과** (`ignoresMouseEvents`) | `set_ignore_cursor_events(bool)` | 아니오 (Tauri v2 에 존재) |
| **전체화면 앱 위에 표시** (`NSScreenSaverWindowLevel` 등 특정 window level) | ❌ 없음 | **예** — `ns_window()` 로 `NSWindow.setLevel()` |
| **활성화하지 않고 표시** (`NSWindow.canBecomeKey = false`, `orderFrontRegardless`) | ❌ 부분적 | **예** |
| **다중 디스플레이 전체를 덮는 좌표계** | ❌ | **예** — `NSScreen.screens` 로 union frame 계산 |
| **`NSWindowCollectionBehavior` 세부 제어** (`.fullScreenAuxiliary`, `.stationary`) | ❌ | **예** |

- **탈출구**: `WebviewWindow::ns_window() -> tauri::Result<*mut std::ffi::c_void>` (macOS 전용). 이 포인터를 `objc2_app_kit::NSWindow` 로 캐스팅하면 AppKit 전체를 쓸 수 있다. → **네이티브 shim 없이 Rust 안에서 해결 가능**하다는 것이 핵심.

### 2.7 메뉴바 상주

- `tray-icon` 0.24.2 (Tauri 가 내부적으로 사용). macOS 에서 `ns_status_item() -> Option<Retained<NSStatusItem>>` 로 `NSStatusItem` 직접 접근을 제공한다.
- 아이콘 + 메뉴 + 클릭 핸들러는 Tauri `TrayIconBuilder` 로 충분. **v1.62 의 "menu item icons" 처럼 메뉴 항목마다 아이콘을 넣는 것**은 `NSMenuItem.image` 직접 설정이 필요할 수 있다 `(추정)`.
- ⚠️ 트레이 아이콘 생성과 이벤트 루프는 **메인 스레드**여야 한다.

### 2.8 자동 업데이트

| 선택지 | 장점 | 단점 |
| :--- | :--- | :--- |
| `tauri-plugin-updater` 2.10.1 | 공식·크로스플랫폼·Tauri 통합 | Sparkle appcast 포맷이 아님 (자체 JSON), 델타 업데이트 없음 |
| `tauri-plugin-sparkle-updater` 0.2.5 | 네이티브 Sparkle. appcast XML·EdDSA 서명·델타·자동 재시작 그대로 | 0.x 커뮤니티 플러그인, Sparkle.framework 번들 필요, `tauri dev` 에서 미동작 |
| Sparkle 직접 바인딩 | 완전한 제어 | `objc2` 로 Sparkle ObjC API 를 손으로 바인딩 — 비용 큼 |

### 2.9 로그인 시 실행

- macOS 13+: `SMAppService.mainApp.register()` — `smappservice-rs` 0.1.3 또는 `objc2-service-management`.
- macOS 12: `SMAppService` 없음 → LaunchAgent plist 또는 `auto-launch` 0.6.0 의 AppleScript/LaunchAgent 모드로 폴백. **최소 지원이 12.0 이므로 분기 필요.**

---

## 3. 툴체인 · 코드 서명 · TCC 지속성

### 3.1 Xcode 전체가 필요한가

| 항목 | 판정 | 근거 |
| :--- | :--- | :--- |
| Rust `aarch64-apple-darwin` / `x86_64-apple-darwin` 빌드·링크 | **CLT 로 충분** | rustc 플랫폼 지원 문서. 링커는 CLT 의 `cc`/`ld` 사용 |
| Tauri v2 macOS 데스크톱 빌드 | **CLT 로 충분** | Tauri v2 prerequisites — 전체 Xcode 는 **iOS 타깃**에만 요구 |
| `codesign` | **CLT 에 포함** | `/Library/Developer/CommandLineTools/usr/bin/codesign` |
| `notarytool` (공증) | **CLT 에 포함** (`xcrun notarytool`) | — |
| macOS SDK (프레임워크 헤더) | **CLT 에 포함** | `/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk` |
| Sparkle.framework 번들링 | CLT 로 가능 | 사전 빌드된 `.framework` 를 복사 + `codesign` |

→ **전체 Xcode 불필요.** `xcode-select --install` 만으로 충분하다.

### 3.2 ⭐ TCC 권한에 코드 서명·공증이 필요한가

이번 조사의 가장 중요한 질문이다. 답을 세 층으로 나눈다.

| 서명 상태 | 권한 부여 가능? | 재빌드 후 유지? | 다른 기기에서 유지? |
| :--- | :--- | :--- | :--- |
| **미서명** | ❌ 사실상 불가 | — | — |
| **ad-hoc 서명** (`codesign --sign -`) | ✅ 가능 | ❌ **매 빌드마다 소실** | ❌ |
| **자체 서명 인증서** (로컬 생성, 키체인 신뢰) | ✅ 가능 | ✅ 유지 | ❌ (해당 기기만) |
| **Developer ID 인증서** | ✅ 가능 | ✅ 유지 | ✅ |

**핵심 메커니즘**: TCC 데이터베이스는 권한을 **번들 ID + 코드 서명 요구사항(`csreq`)** 으로 키잉한다. ad-hoc 서명은 안정적인 서명 주체가 없어 `csreq` 가 **바이너리의 `cdhash`** 에 묶인다. 코드가 1바이트만 바뀌어도 `cdhash` 가 달라지고, macOS 는 이를 **다른 앱**으로 보아 기존 권한을 적용하지 않는다.

**공증(notarization)은 TCC 와 무관하다.** 공증은 Gatekeeper 가 요구하는 것으로, **배포**를 위해 필요하지 로컬에서 권한을 받기 위해 필요한 것이 아니다.

**개발 중 우회**: 로컬 자체 서명 인증서를 하나 만들어 키체인에 "항상 신뢰" 로 등록하고, 모든 개발 빌드를 **동일한 인증서로** 서명한다. 서명 주체가 고정되므로 `csreq` 가 안정되고 재빌드해도 권한이 유지된다. 배포 단계에서만 Developer ID + 공증으로 교체한다.

**출처**:
- https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution (공증 = 배포 요건)
- https://evoleinik.com/posts/macos-dev-signing-preserve-permissions/ (자체 서명 인증서로 권한 유지)
- https://github.com/CapSoftware/Cap/issues/1722 (미서명 바이너리에서 TCC/ScreenCaptureKit 실패 사례)

> ⚠️ 위 3층 표에서 "미서명 = 불가" 는 실무 관찰에 기반한 강한 경험칙이며, 일부 권한·OS 버전에서는 미서명도 프롬프트가 뜨는 사례가 보고된다. **개발 초기부터 자체 서명 인증서를 쓰는 것을 전제로 설계하면 이 불확실성 자체가 사라진다.**

### 3.3 Tauri dev 모드의 함정

`tauri dev` 는 `.app` 번들이 아니라 `target/debug/<binary>` 를 직접 실행한다. 그러면:
- 번들 ID 가 없거나 다르다 → TCC 가 **터미널 앱**(또는 부모 프로세스)의 권한으로 판정한다.
- Sparkle 계열 업데이터가 동작하지 않는다.

→ **권한이 필요한 기능은 `tauri build` 산출 `.app` 을 서명해서 테스트**해야 한다. 개발 루프 설계에 반영할 것.

---

## 4. 남은 확인 필요 사항

| # | 항목 | 왜 남았는가 |
| :--- | :--- | :--- |
| P1 | `CGEventTap` 의 `CFRunLoopSource` 를 Tauri 메인 런루프에 붙이는 것과 전용 스레드 런루프를 쓰는 것 중 무엇이 나은가 | 실측 필요. 전용 스레드가 더 안전해 보이나 `(추정)` 수준 |
| P2 | macOS 12.0–12.2 에서 ScreenCaptureKit 부재 시 폴백 | `CGWindowListCreateImage` 는 14 에서 deprecated 이나 12 에서는 동작. 최소 버전을 12.3 으로 올릴지 결정 필요 |
| P3 | Tauri WebView(WKWebView) 기반 오버레이가 Seek 의 지연 예산을 맞추는가 | 검색 바 + 하이라이트 + 연결선을 매 키 입력마다 다시 그린다. WebView 왕복 비용 실측 필요 |
| P4 | `NSMenuItem` 별 아이콘을 Tauri 트레이 메뉴 API 로 넣을 수 있는가 | Tauri 문서에서 확인 못 함 |
| P5 | Paddle Classic 의 라이선스 검증 엔드포인트 사양 | 공개 문서가 Paddle Billing 위주로 이동함 |
