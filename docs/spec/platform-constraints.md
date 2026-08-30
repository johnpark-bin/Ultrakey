# 플랫폼 제약 — Rust/Tauri 로 어디까지 가능한가

> 이 문서는 **판정 문서**다. 근거 데이터는 [`docs/research/rust-macos-capability-notes.md`](../research/rust-macos-capability-notes.md) 와 [`docs/research/app-bundle-analysis.md`](../research/app-bundle-analysis.md) 에 있고, 여기서는 그 데이터로 **결론을 내린다.**
> 크레이트 버전은 조사일(2026-08-30) `https://crates.io/api/v1/crates/<name>` 직접 조회로 검증된 값이다.
> ⭐ **v1.66 실측 반영(2026-08-30)**: 설치된 SuperKey v1.66 의 `otool -L` · `nm -u` 결과가 §0 에 있다. 이 실측이 아래 판정 중 **세 곳을 바꿨다** — §0.2 참조.

---

## 0. ⭐ SuperKey v1.66 이 실제로 링크한 것 — 우리 판정의 대조군

원본이 무엇을 쓰는지는 "가능한가" 의 답이 아니라 **"충분한가" 의 답**이다. 원본이 어떤 API 로 출하 중이라면, 그 API 로 제품이 성립한다는 것이 증명된 셈이다. 반대로 원본이 쓰지 않는 API 를 우리가 필수로 잡고 있었다면 그 전제를 의심해야 한다.

전체 목록과 심볼 근거는 [`app-bundle-analysis.md` §3](../research/app-bundle-analysis.md) 에 있다. 여기서는 **판정을 바꾸는 것만** 다룬다.

### 0.1 대조표

| 우리가 필요하다고 판정했던 것 | SuperKey v1.66 실측 | 판정 영향 |
| :--- | :--- | :--- |
| ScreenCaptureKit (F-02 화면 캡처) | ❌ **링크 안 함.** `SCStream`·`SCContentFilter` 심볼 전무. 대신 `CGDisplayCreateImage` · `CGDisplayCreateImageForRect` · `CGWindowListCopyWindowInfo` | ⭐ **바뀜** — §0.2 (a) |
| Vision (F-02 OCR) | ✅ `VNRecognizeTextRequest` · `VNRecognizedTextObservation` · `VNImageRectForNormalizedRect` | 판정 유지, `(추정)` → 확정 |
| `CGEventTap` 계열 (F-05·F-07·F-08) | ✅ `CGEventTapCreate`/`Enable`/`IsEnabled`, `CGEventCreateKeyboardEvent`/`MouseEvent`, `CGEventGet`/`SetFlags`, `CGEventPost`, `CGEventKeyboardSetUnicodeString`, `CGEventSourceCreate` | 판정 유지, 확정 |
| AXUIElement (F-02·F-04·F-11) | ✅ `AXIsProcessTrusted`, `AXUIElementCopyAttributeValue`/`CopyElementAtPosition`/`PerformAction`/`SetAttributeValue`/`IsAttributeSettable` | 판정 유지, 확정 |
| Carbon HIToolbox (F-14 입력 소스) | ✅ `UCKeyTranslate` · `TISCopyCurrentKeyboardInputSource` · **`TISCopyCurrentASCIICapableKeyboardLayoutInputSource`** · `TISGetInputSourceProperty` · `LMGetKbdType` | 판정 유지 + API 하나 추가 |
| `MultitouchSupport` 비공개 (F-06) | ✅ **직접 링크.** 번들에서 **유일한 PrivateFramework**. 사용 심볼 9개 확인 | ⭐ **바뀜** — §0.2 (b) |
| `IOHIDCheckAccess` / `IOHIDRequestAccess` (F-11 Input Monitoring) | ❌ **심볼 없음.** SuperKey 는 Input Monitoring 을 명시적으로 확인하지 않는다 | ⭐ **바뀜** — §0.2 (c) |
| — (우리 판정에 없던 것) | ✅ **`IOHIDGetModifierLockState` / `IOHIDSetModifierLockState`** — caps lock 실제 잠금 상태 직접 조작 | 신규 요구 |
| — (우리 판정에 없던 것) | ✅ **`IOHIDManager*` 전 계열** — HID 디바이스 감시·핫플러그 | 신규 요구 |
| — (우리 판정에 없던 것) | ✅ **`kHISymbolicHotKeyCode` / `Enabled` / `Modifiers`** — 시스템 예약 단축키 조회 | §7 P-신규, 아래 §0.3 |
| — (우리 판정에 없던 것) | ✅ **`hidutil` 셸 호출 + `IOHIDServiceClientSetProperty`** — 커널 레벨 키 매핑 | 신규 요구, 아래 §0.3 |
| — (우리 판정에 없던 것) | ✅ **CoreImage** (`CILanczosScaleTransform` · `CIPhotoEffectMono`/`Noir` · `CIMaximumComponent`/`Minimum`) — OCR 전처리 | 신규 요구 |
| — (우리 판정에 없던 것) | ✅ **`SMAppService` 와 `SMLoginItemSetEnabled` 둘 다** — OS 버전 분기 | F-10 보강 |
| — (우리 판정에 없던 것) | ✅ **Security** (`SecStaticCodeCreateWithPath` 등) — 자체 코드 서명 검증 | F-12 보강 |
| Sparkle (F-13) | ✅ **2.9.2 번들.** `SUFeedURL`·`SUPublicEDKey`·`SUScheduledCheckInterval=172800` | 판정 유지, 확정 |
| Paddle (F-12) | ✅ **1.0.0 번들.** Paddle **Classic** (`PADActivateWindow.nib` 등), 제품 ID `750314` | 판정 유지, `(추정)` → 확정 |

원본의 구현 언어는 **Swift + Objective-C**(`libswiftCore` 강한 링크, `NSMainStoryboardFile = Main`, SwiftUI·AppKit)다. **웹뷰를 UI 에 쓰지 않는다** — §4.3 의 미실측 위험 P3 가 그대로 남는다는 뜻이다.

### 0.2 ⭐ 판정이 바뀌는 세 곳

**(a) ScreenCaptureKit 은 필수가 아니다 — 제품 결정 D1 의 근거가 사라졌다.**

SuperKey 는 `LSMinimumSystemVersion = 12.0` 을 유지하면서 `CGDisplayCreateImage` / `CGDisplayCreateImageForRect` 로 화면을 캡처한다. ScreenCaptureKit 심볼은 하나도 없다.

- ⭐ `README.md` 의 **D1("최소 macOS 를 12.0 으로 둘 것인가, 12.3 으로 올릴 것인가")은 답이 나왔다: 12.0 을 유지한다.** 12.3 상향의 유일한 근거였던 ScreenCaptureKit 이 필수가 아니기 때문이다.
- **`screencapturekit` 크레이트(8.0.1)를 F-02 의 필수 의존에서 뺀다.** 대신 **`objc2-core-graphics` 0.3.2**(이 저장소가 이미 쓰는 크레이트)로 `CGDisplayCreateImage` 를 부른다.
  ⭐ **확인 완료(이슈 #30, §7 P11 해소): `objc2-core-graphics`(0.3.2)가 노출한다.** `CGDisplayCreateImage` · `CGDisplayCreateImageForRect` 둘 다 기본 기능에 포함되고 `Option<CFRetained<CGImage>>` 를 돌려주는 안전한 래퍼다. `core-graphics` 0.25.0 을 새로 들일 필요도, 수기 `extern "C"` 도 필요 없다 — 실제 컴파일로 증명했다(`crates/ultrakey-platform/src/screen_capture.rs`).
- **대가**: `CGDisplayCreateImage` 는 macOS 14 에서 deprecated 다. 다만 SDK `macosx26.5` 로 빌드된 v1.66 이 여전히 이를 쓰고 출하 중이라는 것이 **deprecated ≠ 제거**의 실증이다. 이 트레이드오프를 감수하고 12.0 을 지지대로 삼되, ScreenCaptureKit 경로를 **나중에 추가 가능한 대안**으로 남긴다(캡처 계층을 교체 가능하게 분리 — §4.3 의 렌더링 계층 분리와 같은 논리).
- **기각한 대안**: 12.3 으로 올리고 ScreenCaptureKit 만 쓴다 — 원본이 12.0 사용자를 버리지 않고 제품을 성립시켰다는 증거가 있는데, 클론이 먼저 사용자를 버릴 이유가 없다.

**(b) F-06 의 비공개 API 위험은 여전하지만, 함수 이름은 더 이상 추측이 아니다.**

`nm -u` 로 SuperKey 가 실제로 부르는 `MultitouchSupport` 심볼 9개가 확인되었다:
`MTDeviceCreateList` · `MTDeviceCreateFromDeviceID` · `MTDeviceGetDeviceID` · `MTDeviceGetFamilyID` · `MTDeviceGetSensorSurfaceDimensions` · `MTDeviceIsBuiltIn` · `MTDeviceSupportsForce` · `MTDeviceStart` · `MTRegisterContactFrameCallbackWithRefcon`

- §7 **P6 이 절반 해소**된다: **어떤 함수를 부르는지는 확정**이다. 여전히 미확정인 것은 **시그니처(인자 타입·구조체 레이아웃)** 뿐이다.
- 위험 등급은 **바뀌지 않는다.** 오히려 이 목록이 "출하 중인 상용 앱도 이 비공개 API 에 의존한다" 는 사실과 "그래서 OS 업데이트마다 깨질 수 있다" 는 사실을 동시에 보여준다. F-06 을 **최후순위 · 격하 가능한 선택 기능**으로 두는 결정을 유지한다.
- ⭐ 추가 사실: `MTDeviceSupportsForce` · `MTDeviceIsBuiltIn` · `MTDeviceGetFamilyID` 의 존재는 **기기 종류별 분기**(내장 트랙패드 / Magic Trackpad / Magic Mouse / Force Touch 지원 여부)가 필요함을 뜻한다. 단일 디바이스 전제는 틀렸다.

**(c) Input Monitoring 을 명시적으로 확인할 필요가 없을 수 있다.**

SuperKey 에는 `IOHIDCheckAccess` / `IOHIDRequestAccess` 심볼이 **없다**. `AXIsProcessTrustedWithOptions`(프롬프트 변형)도 없다. 확인하는 것은 `AXIsProcessTrusted` 하나뿐이고, IOHID 실패는 **재시도로 흡수**한다 — 문자열 `IOHIDManagerOpen failed with kIOReturnNotPermitted. Retrying... (Attempt ` (SuperKey 원문).

- §6.1 의 "수기 FFI 선언이 필요한 것" 목록에서 `IOHIDCheckAccess` / `IOHIDRequestAccess` 를 **필수에서 선택으로 내린다.** 없어도 제품이 성립한다는 증거가 있다.
- 다만 이것이 "Input Monitoring 권한이 불필요하다" 는 뜻은 **아니다.** OS 가 요구할 수 있고, SuperKey 는 그저 **사전 확인 대신 실패 후 재시도**를 택했을 뿐이다. F-11 은 이 실패-재시도 모델을 명세해야 한다.
- ⚠️ 대신 새로운 필수가 생겼다: `IOHIDManagerCreate`/`Open`/`Close`/`ScheduleWithRunLoop`/`RegisterInputValueCallback`/`RegisterDeviceMatchingCallback`/`RegisterDeviceRemovalCallback`, `IOHIDGetModifierLockState`/`SetModifierLockState`, `IOHIDServiceClientSetProperty`. 전부 C ABI 라 **수기 `extern "C"` 로 커버된다** — shim 이 필요 없다는 결론은 유지된다.

### 0.3 새로 드러난 두 가지 — 3분류 판정

**`hidutil` 서브프로세스 호출 — 어느 분류인가**

SuperKey 는 caps lock 리매핑에 `CGEventTap` 만 쓰지 않는다. 실행 파일에 문자열 `hidutil property -g UserKeyMapping` 과 키 `HIDKeyboardModifierMappingSrc`/`Dst` 가 있고, `Tools_Tools.bundle` 에 셸 실행 오류 문자열이 있다.

**판정: `순수 Rust`.** 근거 — 서브프로세스 실행(`std::process::Command`)은 FFI 가 아니다. `unsafe` 블록도 네이티브 소스도 없고, 출력(plist)을 파싱하는 것도 순수 Rust 다. 3분류의 판별 기준("별도로 빌드하는 `.m`/`.swift` 파일이 없으면 Rust 바인딩, `unsafe` 도 없으면 순수 Rust")에 그대로 걸린다.

**대가와 기각한 대안**: `hidutil` 은 사용자 PATH 에 있는 시스템 바이너리를 부르는 것이라 **버전 간 출력 형식 변화에 취약**하다. 대안으로 `IOHIDServiceClientSetProperty`(SuperKey 도 링크하고 있다)를 직접 부르면 서브프로세스 없이 같은 일을 할 수 있고 그쪽은 `Rust 바인딩` 이다. **`IOHIDServiceClientSetProperty` 직접 호출을 1순위, `hidutil` 서브프로세스를 폴백으로 둔다** — 원본이 둘 다 갖고 있는 것이 이 이중화를 지지한다.

**시스템 예약 단축키 조회 — §4/§5 의 "공개 API 없음" 판정 정정**

`preferences-ui.md` 가 "시스템 예약 조합 전체를 열거할 공개 API 가 없어 알려진 위험 목록 기반의 부분 대응만 가능" 이라고 판정했으나, SuperKey 는 `kHISymbolicHotKeyCode` / `kHISymbolicHotKeyEnabled` / `kHISymbolicHotKeyModifiers` 를 쓴다 — Carbon 의 `CopySymbolicHotKeys` 경로다.

**판정: `Rust 바인딩` (수기 `extern "C"`).** 다만 이 함수는 **공식 문서화되지 않은 Carbon API** 다 — `MultitouchSupport` 만큼은 아니지만 비공개성이 있다. 단축키 충돌 경고는 **있으면 좋은 기능이지 필수가 아니므로**, 실패해도 기능이 격하될 뿐이도록 설계한다. §7 에 P8 로 남긴다.

---

## 요약 — 네 줄

1. **전체 Xcode 는 필요 없다.** Xcode Command Line Tools 만으로 Rust 빌드·Tauri 번들링·`codesign`·`notarytool` 이 전부 된다.
2. **공증(notarization)은 TCC 권한과 무관하다.** 공증은 배포용 Gatekeeper 요건이다. 다만 **코드 서명은 실질적으로 필수**이며, 개발 중에는 **고정된 자체 서명 인증서**를 써야 재빌드 때마다 권한이 날아가지 않는다.
3. **오버레이는 Tauri 창으로 만들되, 창 속성 제어는 `ns_window()` 로 내려가야 한다.** 하지만 그것은 여전히 **Rust 안에서** 끝난다 — `objc2-app-kit` 으로 `NSWindow` 를 직접 만진다. 네이티브 shim 이 아니다.
4. **`CGEventTap` 은 Tauri 메인 런루프에 붙이지 않는다.** 전용 스레드의 독립 `CFRunLoop` 에 등록한다.

⭐ **가장 중요한 결론**: 15개 기능 중 **Swift/Objective-C 소스를 별도로 빌드해야 하는 기능은 하나도 없다.** v1.66 실측(§0)이 이 결론을 뒤집지 않았다 — 원본이 쓰는 프레임워크는 전부 C ABI 또는 Objective-C 로 노출되며, 유일하게 Swift 전용인 의존(`KeyboardShortcuts` 패키지)은 기능이 아니라 **기성 위젯**이라 우리가 직접 만들면 된다(§4.1). `objc2` 생태계와 수기 `extern "C"` 선언으로 전부 Rust 안에서 해결된다. 다만 그 중 상당수가 `unsafe` FFI 이며, **트랙패드 제스처(F-06) 하나는 비공개 프레임워크에 의존**해 위험 등급이 다르다.

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

### 3.6 ⭐ SuperKey v1.66 의 서명 구성 — 우리 워크플로의 확인

실측(`codesign -dv --entitlements :-`)이 §3.1~§3.5 의 판정을 그대로 지지한다.

| 항목 | SuperKey v1.66 | 우리 워크플로와의 관계 |
| :--- | :--- | :--- |
| 서명 | Developer ID (Team ID **`XSYZ3E4B7D`**) | §3.2 표의 "배포용 Developer ID" 그대로 |
| Hardened Runtime | 활성 (`flags=0x10000(runtime)`) | 공증 전제 조건. 우리도 동일하게 간다 |
| App Sandbox | **없음** | ⭐ 예상대로다. `CGEventTap`·AXUIElement·IOHID·비공개 프레임워크 어느 것도 샌드박스에서 쓸 수 없다. **클론도 비샌드박스이며, 따라서 Mac App Store 배포가 불가능하다** — 직접 배포 + Sparkle 이 유일한 경로라는 기존 전제가 확정된다 |
| Entitlements | **`com.apple.security.cs.allow-jit` 하나뿐** | ⭐ **TCC 권한은 entitlement 로 선언되지 않는다**는 사실의 직접 증거다. "어떤 권한이 필요한가" 를 번들 메타데이터로는 판정할 수 없고, 링크된 API 로만 알 수 있다(§0.1). `allow-jit` 자체는 WebKit(Sparkle·Paddle 이 링크)이 요구하는 것으로 보인다 `(미확정)` |
| usage description 키 | **없음** | Accessibility·Screen Recording·Input Monitoring 은 `Info.plist` 문구를 요구하지 않는다는 뜻이다. 마이크·카메라 계열과 다르다 |
| 아키텍처 | Universal (`x86_64` + `arm64`) | 클론도 `lipo` 로 두 타깃을 합쳐야 한다 |

⭐ **우리 워크플로에 더해지는 것 하나**: F-12 가 **자기 자신의 서명을 검증**한다(`SecStaticCodeCreateWithPath`, 요구사항 문자열 `anchor apple generic`, 오류 키 `bundleSignatureCheckFailed`). 즉 서명 구성이 라이선스 검증과도 얽힌다 — 개발 중 자체 서명 인증서를 쓰면 이 검증이 실패할 수 있으므로, **라이선스 검증은 서명 주체에 따라 우회 가능한 형태로 설계**해야 개발 루프가 막히지 않는다. F-12 §7 참조.

---

## 4. ⭐ Tauri 는 이 앱의 UI 에 적합한가

**이 앱은 성격이 전혀 다른 창을 두 개 갖는다. 하나로 답할 수 없다.**

### 4.1 환경설정 창 (F-09) — **Tauri 적합**

폼 컨트롤이 **40개 남짓**(실측: AX 트리 — Seek 8 + 조건부 1 · Hyperkey 9 + 조건부 2 · Presets 16 + 조건부 2 · General 7). 지연 예산이 느슨하고, 상태 변화가 사용자 클릭 단위다. WebView 가 잘 맞는 영역이다.

- **비용**: macOS 네이티브 룩앤필(사이드바 스타일, 팝업 버튼, 슬라이더, 키캡 일러스트, ⓘ 팝오버)을 CSS 로 재현해야 한다. 이것은 **기능 공백이 아니라 프론트엔드 디자인 투자**다.
- ⭐ **실측이 더한 요구 3가지**(전부 웹으로 가능하지만 설계에 미리 반영해야 한다):
  1. **탭마다 창 크기가 다르다** — Seek 555×378 · Hyperkey 710×517 · Presets 825×527 · General 613×273 pt. 탭 전환 시 창이 리사이즈된다.
  2. **종속 컨트롤의 표현이 두 가지다** — 비활성화(dimmed, 자리는 유지)와 **숨김**(레이아웃에서 제거). 원본이 둘을 구분해 쓴다.
  3. **팝업이 라벨 문장 중간에 낀다** — `Caps lock +` [팝업] ` = ◀︎ ▼ ▲ ▶︎`.
- **예외 하나 — 이제 근거가 있다**: 단축키 레코더(`Toggle Seek with shortcut:`)를 `NSEvent` 로컬 모니터로 내려야 할 것이라는 `(추정)` 을 뒷받침하는 실측이 나왔다. SuperKey 는 이 레코더를 직접 만들지 않고 **Sindre Sorhus 의 오픈소스 `KeyboardShortcuts` Swift 패키지**를 쓴다(번들에 `KeyboardShortcuts_KeyboardShortcuts.bundle`, 심볼에 `KeyboardShortcuts.RecorderCocoa` · `RecorderModifierCocoa` · `CarbonKeyboardShortcuts`). AppKit `NSView` 기반이고 Carbon 핫키 등록을 병용한다.
  ⚠️ **그 패키지 자체는 Swift 전용이라 우리가 링크할 수 없다.** 우리는 같은 일을 `objc2-app-kit` 의 `NSEvent` 로컬 모니터 + `global-hotkey` 로 직접 해야 한다 — 여전히 `Rust 바인딩` 이고 shim 은 필요 없지만, **"기성품이 없으니 직접 만든다" 는 비용**이 여기 있다는 것이 확정되었다.
  ⭐ 같은 패키지의 `RecorderModifierCocoa`(modifier 만 녹화하는 변형)도 우리가 직접 만들어야 한다 — Seek 클릭 모드 7종의 modifier 지정에 필요한 것으로 보인다 `(미확정)`.

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
| F-02 | Seek — 텍스트 후보 검출 | Rust 바인딩 | `objc2-vision` 0.3.2, **`core-graphics` 0.25.0 (`CGDisplayCreateImage`)**, `objc2-core-image` (전처리), `axuielement` 0.9.1 | 불필요 |
| F-03 | Seek — 오버레이 UI | Rust 바인딩 | `tauri` 2.11.5 + `ns_window()` → `objc2-app-kit` 0.3.2 | 불필요 |
| F-04 | Seek — 클릭 실행 | Rust 바인딩 | `core-graphics` 0.25.0, `axuielement` 0.9.1, `objc2-app-kit` 0.3.2 | 불필요 |
| F-05 | Hyperkey (hyper·meh·bleh) | Rust 바인딩 | `core-graphics` 0.25.0 / `objc2-core-graphics` 0.3.2 | 불필요 |
| F-06 | **트랙패드 hyper 제스처** | **Rust 바인딩 ⚠️** | **비공개 `MultitouchSupport.framework`** — 크레이트 없음, 수기 `extern "C"`. 부를 함수 9개는 실측 확정(§0.2 b), 시그니처만 미확정 | 불필요하나 **위험 등급 다름** |
| F-07 | 키 리매핑 엔진 | Rust 바인딩 | `core-graphics` 0.25.0, `objc2-core-graphics` 0.3.2, Carbon `HIToolbox`(수기 선언) | 불필요 |
| F-08 | Power User Presets | Rust 바인딩 (+ `hidutil` 폴백은 순수 Rust) | F-07 과 동일 + `IOKit`(수기 선언 — `IOHIDGet`/`SetModifierLockState`, `IOHIDServiceClientSetProperty`) | 불필요 |
| F-09 | 환경설정 UI | Rust 바인딩 | `tauri` 2.11.5, `tauri-plugin-store`, `objc2-app-kit` 0.3.2 (레코더) | 불필요 |
| F-10 | 메뉴바 상주와 수명주기 | Rust 바인딩 | `tray-icon` 0.24.2, `smappservice-rs` 0.1.3, `auto-launch` 0.6.0 | 불필요 |
| F-11 | 권한 온보딩과 복구 | Rust 바인딩 | `axuielement` 0.9.1 (`AXIsProcessTrusted`), `core-graphics` 0.25.0, `IOKit`(수기 선언 — 사전 확인은 **선택**, §0.2 c) | 불필요 |
| F-12 | 라이선싱과 트라이얼 | 순수 Rust (+ Keychain 은 바인딩) | HTTPS 클라이언트 + `Security.framework` | 불필요 |
| F-13 | 자동 업데이트 | Rust 바인딩 | `tauri-plugin-sparkle-updater` 0.2.5 (+ `Sparkle.framework` 번들) | 불필요 |
| F-14 | 현지화와 입력 소스 | Rust 바인딩 | Carbon `HIToolbox` (`UCKeyTranslate`, `TISCopyCurrentKeyboardInputSource`, **`TISCopyCurrentASCIICapableKeyboardLayoutInputSource`**, `LMGetKbdType`) — 수기 선언 | 불필요 |
| **F-15** | **설정 저장 모델과 무결성** | **순수 Rust** | `tauri-plugin-store` (+ 클라우드 동기화 채택 시 `objc2-foundation` 바인딩) | 불필요 |

### 6.1 경계선 — 한 문단으로

**Rust/Tauri 가 커버하는 것**: 애플리케이션 로직 전부, 환경설정 UI, 메뉴바, 라이선싱, 자동 업데이트, 그리고 **공개 macOS 프레임워크 호출 전부**(`objc2-*` 자동 생성 바인딩 또는 안전 래퍼 크레이트).

**Rust 안에 있지만 수기 FFI 선언이 필요한 것** — 크레이트가 없는 C ABI 함수들 (⭐ v1.66 실측으로 목록이 늘고 우선순위가 바뀌었다):

| 함수군 | 프레임워크 | 필수인가 | 근거 |
| :--- | :--- | :--- | :--- |
| `UCKeyTranslate` · `TISCopyCurrentKeyboardInputSource` · **`TISCopyCurrentASCIICapableKeyboardLayoutInputSource`** · `TISGetInputSourceProperty` · `LMGetKbdType` | Carbon HIToolbox | ✅ **필수** | SuperKey 가 전부 링크 (§0.1) |
| **`IOHIDGetModifierLockState` / `IOHIDSetModifierLockState`** | IOKit | ✅ **필수 (신규)** | caps lock 실제 잠금 토글. F-08.8/9/10 이 이것 없이는 성립하지 않는다 |
| **`IOHIDManagerCreate`/`Open`/`Close`/`ScheduleWithRunLoop`/`RegisterInputValueCallback`/`RegisterDeviceMatchingCallback`/`RegisterDeviceRemovalCallback`** | IOKit | ✅ **필수 (신규)** | 키보드 핫플러그 감지. F-07 이 요구 |
| **`IOHIDServiceClientSetProperty`** · `IOHIDEventSystemClientCreateSimpleClient` | IOKit | ⚠️ 권장 | 커널 레벨 키 매핑 1순위 경로 (§0.3). 실패 시 `hidutil` 서브프로세스 폴백 |
| `IsSecureEventInputEnabled` | Carbon | ✅ 필수 | Secure Input 구간 판정 |
| `MTDeviceCreateList` 계열 9개 | **비공개 MultitouchSupport** | ⚠️ 선택 (F-06) | 함수명 실측 확정, 시그니처 미확정 (§0.2 b, §7 P6) |
| `kHISymbolicHotKey*` (`CopySymbolicHotKeys`) | Carbon (문서화 안 됨) | ⚠️ 선택 | 단축키 충돌 경고용. 실패 시 기능 격하 (§0.3, §7 P8) |
| `IOHIDCheckAccess` / `IOHIDRequestAccess` | IOKit | ⬇️ **선택으로 강등** | SuperKey 는 쓰지 않는다. 사전 확인 대신 실패-재시도 (§0.2 c) |

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
| ~~P2~~ | ~~최소 macOS 버전 12.0 vs 12.3~~ | ✅ **해소.** SuperKey v1.66 이 `LSMinimumSystemVersion = 12.0` 을 유지하며 ScreenCaptureKit 없이 `CGDisplayCreateImage` 로 출하 중이다(§0.2 a). **12.0 을 유지한다.** 남는 것은 `CGDisplayCreateImage` 의 deprecated 상태를 언제까지 감수할지이며, 이는 P9 로 이관 | — |
| P3 | WKWebView 오버레이 렌더링 지연 | ⭐ **절반 해소(이슈 #30).** **검출 쪽은 실측됐다** — 전체 화면(3840×1600 + 2560×1440) `.accurate` OCR 이 **약 775 ms**(캡처 31 ms + OCR 744 ms)이고, `VNRecognizeTextRequest` 는 **병렬화되지 않는다**(이득 0 %). ⭐ 중요한 것은 이 비용이 **세션당 1회**이고 매 키 입력마다 드는 비용이 아니라는 점이다 — 입력마다 하는 일은 이미 만들어진 후보 목록에 대한 순수 문자열 필터링이다. **따라서 P3 가 남긴 진짜 미실측은 "이미 그려진 오버레이를 키 입력마다 다시 그리는 WKWebView 왕복" 하나로 좁혀졌다.** 근거: `docs/dev/seek-ocr-latency-spike.md` | (남은 절반) 후보를 실제로 그린 상태에서 키 입력 → 화면 갱신 왕복 시간 측정 — F-03 위임 |
| P4 | `NSMenuItem` 별 아이콘을 Tauri 트레이 API 로 넣을 수 있는가 | Tauri 문서에서 확인 못 함 | `tray-icon` 의 `ns_status_item()` 으로 내려가면 확실히 가능 `(추정)` |
| P5 | `axuielement` 가 `AXUIElementSetMessagingTimeout` 을 노출하는가 | docs.rs 목록에서 확인 못 함 | 미노출이면 `accessibility-sys` 로 그 한 함수만 직접 선언 |
| P6 | `MultitouchSupport` **함수 시그니처** | 절반 해소(§0.2 b). **부를 함수 9개는 실측 확정**, 인자 타입·`MTTouch` 구조체 레이아웃은 여전히 비공개 | 오픈소스 구현체(`fingermgmt` 등) 대조. **시그니처를 추측으로 쓰지 말 것** |
| P7 | Paddle Classic 라이선스 검증 엔드포인트 사양 | Paddle **Classic** 사용은 확정(§0.1, 번들에 `Paddle.framework` 1.0.0 + `PAD*Window.nib`), 제품 ID `750314` 도 확정. 그러나 **엔드포인트 사양은 여전히 미확정** — 공개 문서가 Paddle Billing 으로 이동했다 | Paddle 대시보드 / 지원 문의 |
| **P8** | `CopySymbolicHotKeys`(`kHISymbolicHotKey*`)의 시그니처와 안정성 | ⭐ **신규.** SuperKey 가 쓰는 것은 확정이나(§0.3) 공식 문서화되지 않은 Carbon API 다 | 단축키 충돌 경고를 **격하 가능한 선택 기능**으로 설계해 실패를 흡수한다 |
| **P9** | `CGDisplayCreateImage` deprecated 대응 시한 | ⭐ **신규(P2 에서 이관).** macOS 14 에서 deprecated 이나 SDK `macosx26.5` 빌드에서 여전히 동작한다(§0.2 a) | 캡처 계층을 교체 가능하게 분리하고, 제거 신호가 보이면 ScreenCaptureKit 경로를 추가한다 |
| **P10** | OCR 전처리 필터 조합의 효과 | ⭐ **신규.** SuperKey 가 `CILanczosScaleTransform`·`CIPhotoEffectMono`/`Noir`·`CIMaximumComponent`/`Minimum` 을 쓰는 것은 확정이나, **어떤 조건에서 어떤 필터를 고르는지**는 알 수 없다 | F-02 실측 스파이크에서 전처리 유무의 OCR 정확도·지연 차이를 직접 측정 |
| ~~P11~~ | ~~`core-graphics` 크레이트가 `CGDisplayCreateImage` 를 노출하는가~~ | ⭐ **해소(이슈 #30).** 이 저장소가 이미 쓰는 **`objc2-core-graphics`(0.3.2)** 가 `CGDisplayCreateImage` · `CGDisplayCreateImageForRect` 를 기본 기능(`CGDirectDisplay` + `CGImage`)으로 노출한다 — `Option<CFRetained<CGImage>>` 를 돌려주는 안전한 래퍼다. 함께 필요한 `CGGetActiveDisplayList` · `CGDisplayBounds`, 그리고 `CGPreflightScreenCaptureAccess` · `CGRequestScreenCaptureAccess` 도 같은 크레이트에 있다. ⭐ **`core-graphics`/`core-graphics-sys` 를 새로 들이거나 원시 `extern "C"` 를 선언할 필요가 없다** — P6 의 시그니처 추측 위험에 새로 노출되는 지점이 하나도 늘지 않았다. 확인 방법: 크레이트 소스 대조 + **실제 컴파일**(`crates/ultrakey-platform/src/screen_capture.rs`) | 해소됨 |
