# 플랫폼 제약 — Rust/Tauri 로 어디까지 가능한가

> 이 문서는 **판정 문서**다. 근거 데이터는 [`docs/research/rust-macos-capability-notes.md`](../research/rust-macos-capability-notes.md) 에 있고, 여기서는 그 데이터로 **결론을 내린다.**
> 크레이트 버전은 조사일(2026-08-30) `https://crates.io/api/v1/crates/<name>` 직접 조회로 검증된 값이다.

## 요약 — 네 줄

1. **전체 Xcode 는 필요 없다.** Xcode Command Line Tools 만으로 Rust 빌드·Tauri 번들링·`codesign`·`notarytool` 이 전부 된다.
2. **공증(notarization)은 TCC 권한과 무관하다.** 공증은 배포용 Gatekeeper 요건이다. 다만 **코드 서명은 실질적으로 필수**이며, 개발 중에는 **고정된 자체 서명 인증서**를 써야 재빌드 때마다 권한이 날아가지 않는다.
3. **오버레이는 Tauri 창으로 만들되, 창 속성 제어는 `ns_window()` 로 내려가야 한다.** 하지만 그것은 여전히 **Rust 안에서** 끝난다 — `objc2-app-kit` 으로 `NSWindow` 를 직접 만진다. 네이티브 shim 이 아니다.
4. **`CGEventTap` 은 Tauri 메인 런루프에 붙이지 않는다.** 전용 스레드의 독립 `CFRunLoop` 에 등록한다.

⭐ **가장 중요한 결론**: 13개 기능 중 **Swift/Objective-C 소스를 별도로 빌드해야 하는 기능은 하나도 없다.** `objc2` 생태계와 수기 `extern "C"` 선언으로 전부 Rust 안에서 해결된다. 다만 그 중 상당수가 `unsafe` FFI 이며, **트랙패드 제스처(F-06) 하나는 비공개 프레임워크에 의존**해 위험 등급이 다르다.

---

## 1. 구현 접근 3분류

이 프로젝트 전체가 쓰는 어휘다. 각 명세의 7절이 이 중 하나로 판정한다.

| 분류 | 정의 | 판별 기준 |
| :--- | :--- | :--- |
| **순수 Rust** | 기존 안전(safe) 래퍼 크레이트만으로 커버. `unsafe` 블록도 네이티브 소스도 없다 | 애플리케이션 로직, 자료구조, 파일·네트워크 I/O |
| **Rust 바인딩** | 크레이트는 존재하나 `unsafe` FFI 직접 호출이 필요. `objc2-*` 헤더 자동 생성 바인딩이거나 수기 `extern "C"` 선언 | **별도로 빌드하는 `.m`/`.swift` 파일이 없으면 여기다** |
| **네이티브 shim 불가피** | Rust 에서 호출할 방법이 없어 Swift/Objective-C 소스를 별도 컴파일해 링크해야 함 | Swift 전용 API(ObjC 노출 없음), ObjC 블록/델리게이트를 넘겨야 하는데 바인딩이 없는 경우 |

> ⚠️ **"수기 `extern "C"` 선언" 은 네이티브 shim 이 아니다.** C ABI 함수는 Rust 에서 직접 선언해 부를 수 있다. `build.rs` 에 `println!("cargo:rustc-link-lib=framework=IOKit")` 한 줄을 추가하는 것과, `.m` 파일을 만들어 `cc` 크레이트로 컴파일하는 것은 유지보수 비용이 전혀 다르다. 이 구분을 흐리면 판정이 무의미해진다.

---

## 2. ⭐ 전체 Xcode 없이 어디까지 가능한가

**결론: `xcode-select --install` 만으로 충분하다. 전체 Xcode 는 iOS 타깃을 추가할 때만 필요하다.**

| 필요한 것 | CLT 로 되는가 | 근거 |
| :--- | :--- | :--- |
| Rust `aarch64-apple-darwin` / `x86_64-apple-darwin` 컴파일·링크 | ✅ | 두 타깃 모두 rustup 배포 tier 1. 링커는 CLT 의 `cc`/`ld` 를 쓴다. rustc 플랫폼 지원 문서가 전체 Xcode 를 요구하지 않는다 |
| macOS SDK (프레임워크 헤더 — AppKit·Vision·ScreenCaptureKit 등) | ✅ | `/Library/Developer/CommandLineTools/SDKs/MacOSX.sdk` 에 포함 |
| `objc2-*` 크레이트 빌드 | ✅ | 헤더가 아니라 **사전 생성된 Rust 소스**다. 빌드 시 SDK 헤더 파싱이 필요 없다 |
| Tauri v2 macOS 데스크톱 번들링(`.app`) | ✅ | Tauri v2 prerequisites 가 전체 Xcode 를 **iOS 개발에만** 요구한다 |
| `codesign` | ✅ | `/Library/Developer/CommandLineTools/usr/bin/codesign` |
| `xcrun notarytool` (공증) | ✅ | CLT 에 포함 |
| `Sparkle.framework` 번들링 + 서명 | ✅ | 사전 빌드된 `.framework` 를 복사 후 `codesign`. 소스 빌드가 필요 없다 |
| Interface Builder / xib / storyboard | ❌ | **쓰지 않는다.** UI 는 Tauri(웹) 또는 코드로 만든 `NSWindow` 다 |
| iOS 시뮬레이터 | ❌ | 해당 없음 (macOS 전용 제품) |

**검증 명령**

```
xcode-select -p              # /Library/Developer/CommandLineTools 이면 CLT 만 설치된 상태
which codesign notarytool    # 또는 xcrun --find notarytool
ls /Library/Developer/CommandLineTools/SDKs/
```

**따라오는 제약 하나**: CLT 만 있으면 Xcode 가 관리하는 **자동 서명(automatic signing)** 을 쓸 수 없다. 인증서와 프로비저닝을 직접 관리해야 한다. 이 제품은 App Store 배포가 아니라 직접 배포(`.dmg` + Sparkle)이므로 프로비저닝 프로파일 자체가 필요 없고, 인증서 하나만 관리하면 된다. 실질적 부담이 아니다.

---

## 3. ⭐⭐ TCC 권한과 코드 서명 — 이 프로젝트의 가장 위험한 지점

### 3.1 질문에 대한 직답

> **Q. 접근성·화면 녹화 TCC 권한을 받으려면 코드 서명·공증이 필요한가?**

- **공증은 필요 없다.** 공증은 Gatekeeper 가 "인터넷에서 받은 앱" 을 열 때 검사하는 것이다. TCC(권한 부여)와는 별개의 시스템이다. 로컬에서 빌드한 앱에 권한을 주는 데 공증은 관여하지 않는다.
- **코드 서명은 실질적으로 필요하다.** 그리고 더 중요한 것은 **어떤 서명이냐**다.

### 3.2 왜 그런가 — TCC 의 키잉 방식

TCC 데이터베이스는 권한을 **`(번들 ID, 코드 서명 요구사항 csreq)`** 쌍으로 키잉한다.

- **Developer ID / 자체 서명 인증서**로 서명하면 `csreq` 가 **서명 주체(인증서)** 를 가리킨다. 코드가 바뀌어도 서명 주체가 같으면 같은 앱으로 인식되어 **권한이 유지**된다.
- **ad-hoc 서명**(`codesign --sign -`)은 안정적인 서명 주체가 없다. `csreq` 가 **바이너리의 `cdhash`** 에 묶인다. 소스를 한 줄만 고쳐도 `cdhash` 가 바뀌고, macOS 는 이를 **다른 앱**으로 본다 → **재빌드할 때마다 권한이 사라진다.**

| 서명 상태 | 권한 부여 | 재빌드 후 유지 | 다른 기기에서 유지 | 이 프로젝트에서의 용도 |
| :--- | :---: | :---: | :---: | :--- |
| 미서명 | ❌ 사실상 불가 | — | — | 쓰지 않는다 |
| ad-hoc (`--sign -`) | ✅ | ❌ **매 빌드 소실** | ❌ | 쓰지 않는다 |
| **자체 서명 인증서 (고정)** | ✅ | ✅ | ❌ | ⭐ **개발 중 표준** |
| **Developer ID** | ✅ | ✅ | ✅ | ⭐ **배포용** |

> ⚠️ "미서명 = 불가" 는 실무 관찰에 기반한 강한 경험칙이며 권한·OS 버전에 따라 예외 보고가 있다. **개발 첫날부터 고정 자체 서명 인증서를 쓰면 이 불확실성 자체가 사라지므로**, 예외를 탐색할 이유가 없다.

### 3.3 개발 중 우회 — 확정된 워크플로

1. 로컬에서 코드 서명용 자체 서명 인증서를 하나 만든다 (키체인 접근 → 인증서 지원 → 인증서 생성, 유형 "코드 서명").
2. 키체인에서 그 인증서를 **"항상 신뢰"** 로 설정한다.
3. **모든 개발 빌드를 같은 인증서로 서명한다**: `codesign --force --deep --sign "Ultrakey Dev" target/release/bundle/macos/Ultrakey.app`
4. 번들 ID 를 **개발 중에도 고정**한다. 바뀌면 새 앱으로 취급된다.
5. 배포 단계에서만 Developer ID + `notarytool` 로 교체한다.

### 3.4 ⚠️ `tauri dev` 의 함정 — 개발 루프 설계에 직결

`tauri dev` 는 `.app` 번들이 아니라 `target/debug/<binary>` 를 직접 실행한다. 그러면:

- 번들 ID 가 없거나 다르므로 TCC 가 **부모 프로세스(터미널·IDE)의 권한**으로 판정한다. 터미널에 접근성 권한을 주면 "동작하는 것처럼 보이지만" 실제 배포 조건과 다르다.
- Sparkle 계열 업데이터가 아예 동작하지 않는다 (`.app` 번들 필요).

**결론: 권한이 필요한 기능(F-01~F-08, F-11, F-13)은 `tauri build` 산출 `.app` 을 서명해서 테스트한다.** 이것을 개발 루프의 전제로 삼는다 — 나중에 발견하면 이미 잘못된 가정 위에 코드가 쌓여 있다.

### 3.5 업데이트가 권한을 깨뜨리지 않을 조건

자동 업데이트(F-13)가 `.app` 을 교체할 때 **신·구 앱의 서명 주체와 번들 ID 가 같아야** TCC 권한이 유지된다. 서명 인증서를 갱신할 때(만료 등) 같은 Team ID 를 유지하지 않으면 **모든 사용자가 권한을 다시 부여**해야 한다. 릴리스 파이프라인의 불변 조건으로 못박는다.

---

## 4. ⭐ Tauri 는 이 앱의 UI 에 적합한가

**이 앱은 성격이 전혀 다른 창을 두 개 갖는다. 하나로 답할 수 없다.**

### 4.1 환경설정 창 (F-09) — **Tauri 적합**

폼 컨트롤이 30개 남짓(Seek 9 · Hyperkey 6 · Presets 16 + General), 지연 예산이 느슨하고, 상태 변화가 사용자 클릭 단위다. WebView 가 잘 맞는 영역이다.

- **비용**: macOS 네이티브 룩앤필(사이드바 스타일, 팝업 버튼, 슬라이더 눈금, 키캡 일러스트)을 CSS 로 재현해야 한다. 이것은 **기능 공백이 아니라 프론트엔드 디자인 투자**다.
- **예외 하나**: 단축키 레코더(`Toggle Seek with shortcut:`)는 웹뷰가 modifier 조합을 온전히 못 받는 경우가 있어 `NSEvent` 로컬 모니터(`objc2-app-kit`)로 내려갈 가능성이 높다 `(추정)`.

### 4.2 Seek 오버레이 (F-03) — **Tauri 창 + `ns_window()` 네이티브 제어**

Tauri 설정만으로는 부족하다. 필요한 것과 커버 여부:

| 요구 속성 | Tauri v2 설정으로 되는가 | 안 되면 무엇으로 |
| :--- | :--- | :--- |
| 테두리 없음 | ✅ `decorations: false` | — |
| 투명 배경 | ✅ `transparent: true` | — |
| 항상 위 | ✅ `always_on_top: true` | — |
| 모든 Space 에 표시 | ✅ `visible_on_all_workspaces: true` | — |
| 그림자 제거 | ✅ `shadow: false` | — |
| 클릭 통과 | ✅ `set_ignore_cursor_events(true)` | — |
| Dock 아이콘 없음 | ✅ `ActivationPolicy::Accessory` | — |
| **전체화면 앱 위에 표시** | ❌ | `ns_window()` → `NSWindow.setLevel()` |
| **포커스를 뺏지 않고 표시** | ❌ | `ns_window()` → `canBecomeKey = false` + `orderFrontRegardless` |
| **모든 디스플레이를 덮는 union frame** | ❌ | `NSScreen.screens` 로 계산 후 `setFrame:` |
| **`NSWindowCollectionBehavior` 세부 제어**(`.fullScreenAuxiliary`, `.stationary`) | ❌ | `ns_window()` → `setCollectionBehavior:` |

**탈출구는 확보되어 있다**: `WebviewWindow::ns_window() -> tauri::Result<*mut std::ffi::c_void>` (macOS 전용)로 얻은 포인터를 `objc2_app_kit::NSWindow` 로 캐스팅하면 AppKit 전체를 쓸 수 있다. → **`Rust 바인딩`. 네이티브 shim 이 아니다.**

### 4.3 남은 미검증 — 오버레이 콘텐츠 렌더링

Seek 은 **매 키 입력마다** 후보를 다시 필터링해 하이라이트와 연결선을 다시 그린다. WKWebView 왕복(Rust → IPC → JS → DOM/Canvas)이 이 예산에 맞는지는 **실측하지 않았다**(조사 노트 P3).

**결정**: 1차 구현은 WKWebView 로 그리되, **렌더링 계층을 교체 가능한 모듈로 분리**한다. 실측에서 예산을 넘으면 `objc2-quartz-core` 의 `CALayer` 직접 그리기로 교체한다. 그 대안도 **Rust 안에서 끝난다**(네이티브 shim 아님)는 점이 이 결정을 안전하게 만든다.

**기각한 대안**: 처음부터 순수 AppKit 오버레이. 검색 바의 텍스트 입력·IME 처리를 직접 만들어야 하고, 실측 전에 비용 큰 쪽으로 확정하는 것이 정당화되지 않는다.

---

## 5. ⭐ `CGEventTap` 과 Tauri 이벤트 루프의 공존

### 5.1 문제

`CGEventTap` 은 `CFRunLoopSource` 로 런루프에 붙어야 하고, 그 런루프가 **계속 돌아야** 이벤트를 받는다. Tauri 는 메인 스레드에서 `NSApplication` 런루프를 돌린다. 두 가지 선택지가 있다.

| 방식 | 장점 | 단점 |
| :--- | :--- | :--- |
| Tauri 메인 런루프에 소스 추가 | 스레드 하나로 끝남. 콜백에서 앱 상태 접근이 쉬움 | ⚠️ 메인 런루프가 **모드를 전환**하면(메뉴 트래킹, 창 리사이즈 중의 `NSEventTrackingRunLoopMode`) 기본 모드에만 등록된 소스가 멈춘다 → 그 동안 키 리매핑이 죽는다. 웹뷰 렌더링이 메인 스레드를 점유하면 콜백이 지연되어 **탭이 타임아웃으로 비활성화**된다 |
| **전용 스레드의 독립 `CFRunLoop`** | 메인 스레드 부하와 완전히 분리. 모드 전환 영향 없음. 콜백 지연 예산을 독립적으로 지킴 | 앱 상태 공유에 동기화 필요(채널 또는 락) |

### 5.2 결정

**전용 스레드에 독립 `CFRunLoop` 를 두고 거기에 event tap 을 등록한다.**

근거:
- 이 앱에서 event tap 콜백은 **모든 키 입력의 임계 경로**다. 메인 스레드의 UI 작업과 예산을 공유해서는 안 된다.
- `CGEventTap` 은 콜백이 늦으면 macOS 가 **탭을 강제 비활성화**한다(`kCGEventTapDisabledByTimeout`). 메인 런루프 결합은 이 위험을 구조적으로 안고 간다.
- 모드 전환 문제는 `CFRunLoopAddSource(..., kCFRunLoopCommonModes)` 로 완화할 수 있으나, 완화보다 **분리**가 확실하다.

**대가**: 콜백 스레드와 앱 상태 사이에 명시적 경계가 생긴다. 설정 스냅샷을 락 없이 읽을 수 있는 구조(예: 원자적 교체)로 넘겨야 한다. 이것은 오히려 콜백에서 블로킹하지 않게 만드는 강제 장치라 **바람직한 제약**이다.

### 5.3 반드시 구현해야 하는 것 — 탭 재활성화

`kCGEventTapDisabledByTimeout` / `kCGEventTapDisabledByUserInput` 이벤트를 수신하면 `CGEventTapEnable(tap, true)` 로 되살린다. 여기에 더해 `NSWorkspace` 의 깨어남·세션 활성 알림을 구독해 재초기화한다.

원본이 실제로 겪은 실패다 — v1.58: *"In certain scenarios, Superkey's key remapping was not working upon wake or login. This should now be fixed."*

---

## 6. ⭐ 결론 — Rust/Tauri 경계와 네이티브 shim 경계

| ID | 기능 | 판정 | 핵심 크레이트 (검증된 버전) | 네이티브 shim? |
| :--- | :--- | :--- | :--- | :---: |
| F-01 | Seek — 활성화와 세션 | Rust 바인딩 | `core-graphics` 0.25.0, `global-hotkey` 0.8.0 | 불필요 |
| F-02 | Seek — 텍스트 후보 검출 | Rust 바인딩 | `objc2-vision` 0.3.2, `screencapturekit` 8.0.1, `axuielement` 0.9.1 | 불필요 |
| F-03 | Seek — 오버레이 UI | Rust 바인딩 | `tauri` 2.11.5 + `ns_window()` → `objc2-app-kit` 0.3.2 | 불필요 |
| F-04 | Seek — 클릭 실행 | Rust 바인딩 | `core-graphics` 0.25.0, `axuielement` 0.9.1, `objc2-app-kit` 0.3.2 | 불필요 |
| F-05 | Hyperkey (hyper·meh·bleh) | Rust 바인딩 | `core-graphics` 0.25.0 / `objc2-core-graphics` 0.3.2 | 불필요 |
| F-06 | **트랙패드 hyper 제스처** | **Rust 바인딩 ⚠️** | **비공개 `MultitouchSupport.framework`** — 크레이트 없음, 수기 `extern "C"` | 불필요하나 **위험 등급 다름** |
| F-07 | 키 리매핑 엔진 | Rust 바인딩 | `core-graphics` 0.25.0, `objc2-core-graphics` 0.3.2, Carbon `HIToolbox`(수기 선언) | 불필요 |
| F-08 | Power User Presets | Rust 바인딩 | F-07 과 동일 + `IOKit`(수기 선언) | 불필요 |
| F-09 | 환경설정 UI | Rust 바인딩 | `tauri` 2.11.5, `tauri-plugin-store`, `objc2-app-kit` 0.3.2 (레코더) | 불필요 |
| F-10 | 메뉴바 상주와 수명주기 | Rust 바인딩 | `tray-icon` 0.24.2, `smappservice-rs` 0.1.3, `auto-launch` 0.6.0 | 불필요 |
| F-11 | 권한 온보딩과 복구 | Rust 바인딩 | `axuielement` 0.9.1, `core-graphics` 0.25.0, **`IOKit`(수기 선언)** | 불필요 |
| F-12 | 라이선싱과 트라이얼 | 순수 Rust (+ Keychain 은 바인딩) | HTTPS 클라이언트 + `Security.framework` | 불필요 |
| F-13 | 자동 업데이트 | Rust 바인딩 | `tauri-plugin-sparkle-updater` 0.2.5 (+ `Sparkle.framework` 번들) | 불필요 |
| F-14 | 현지화와 입력 소스 | Rust 바인딩 | Carbon `HIToolbox` (`UCKeyTranslate`, `TIS*`) — 수기 선언 | 불필요 |

### 6.1 경계선 — 한 문단으로

**Rust/Tauri 가 커버하는 것**: 애플리케이션 로직 전부, 환경설정 UI, 메뉴바, 라이선싱, 자동 업데이트, 그리고 **공개 macOS 프레임워크 호출 전부**(`objc2-*` 자동 생성 바인딩 또는 안전 래퍼 크레이트).

**Rust 안에 있지만 수기 FFI 선언이 필요한 것** — 크레이트가 없는 C ABI 함수들:
`IOHIDCheckAccess` / `IOHIDRequestAccess` (Input Monitoring TCC), `IsSecureEventInputEnabled`, `UCKeyTranslate` · `TISCopyCurrentKeyboardInputSource` (Carbon HIToolbox), `MTDeviceCreateList` 계열(비공개 MultitouchSupport).
→ `build.rs` 에서 해당 프레임워크를 링크하고 `extern "C"` 블록을 쓴다. **`.m`/`.swift` 파일은 만들지 않는다.**

**네이티브 shim 이 불가피한 것**: **없다.** 이번 조사 범위에서는 Swift 전용 API 나 ObjC 블록 전달이 필수인 지점이 발견되지 않았다.

### 6.2 ⚠️ 진짜 위험은 "shim 이 필요한가" 가 아니라 다른 곳에 있다

판정 표가 전부 "Rust 바인딩" 이라는 것이 이 프로젝트가 쉽다는 뜻이 **아니다.** 실제 위험은 세 가지다.

| 위험 | 어디 | 왜 위험한가 | 완화 |
| :--- | :--- | :--- | :--- |
| **비공개 API 의존** | F-06 트랙패드 제스처 | `MultitouchSupport` 는 헤더도 문서도 없다. OS 업데이트로 시그니처가 바뀌면 조용히 깨진다 | 선택적 기능으로 설계. 실패 시 F-05 물리 키 경로로 격하. **구현 순서 최후순위** |
| **TCC 권한 소실** | 전 기능 | ad-hoc 서명·`tauri dev`·번들 ID 변경 어느 하나만 어긋나도 개발 내내 헛다리를 짚는다 | §3.3 워크플로를 **1일차에** 세팅 |
| **미실측 지연 예산** | F-03 오버레이 렌더링, F-02 OCR·AX 파이프라인 | 기능적으로는 되지만 체감이 느리면 제품이 아니다 | 렌더링 계층 분리, 초기에 실측 스파이크 |

---

## 7. 확정 못 한 것

| # | 항목 | 왜 남았는가 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| P1 | 전용 스레드 런루프의 실제 지연 특성 | §5.2 는 원리에 근거한 결정이며 실측이 아니다 | 프로토타입에서 콜백 지연 분포 측정 |
| P2 | **최소 macOS 버전 12.0 vs 12.3** | ScreenCaptureKit 이 12.3+ 다. F-02 명세는 **12.3 상향**을 제안했다. `CGWindowListCreateImage` 폴백은 macOS 14 deprecated | ⚠️ **제품 결정 필요** — 12.0–12.2 사용자 비중을 판단해야 한다 |
| P3 | WKWebView 오버레이 렌더링 지연 | 실측 없음 | 키 입력 → 화면 갱신 왕복 시간 측정 스파이크 |
| P4 | `NSMenuItem` 별 아이콘을 Tauri 트레이 API 로 넣을 수 있는가 | Tauri 문서에서 확인 못 함 | `tray-icon` 의 `ns_status_item()` 으로 내려가면 확실히 가능 `(추정)` |
| P5 | `axuielement` 가 `AXUIElementSetMessagingTimeout` 을 노출하는가 | docs.rs 목록에서 확인 못 함 | 미노출이면 `accessibility-sys` 로 그 한 함수만 직접 선언 |
| P6 | `MultitouchSupport` 함수 시그니처 | 비공개 API — 공식 문서 없음 | 오픈소스 구현체(`fingermgmt` 등) 대조. **시그니처를 추측으로 쓰지 말 것** |
| P7 | Paddle Classic 라이선스 검증 엔드포인트 사양 | 공개 문서가 Paddle Billing 으로 이동 | Paddle 대시보드 / 지원 문의 |
