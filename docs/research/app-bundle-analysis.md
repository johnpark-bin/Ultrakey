# SuperKey v1.66 (66) 앱 번들 정적 분석 · 실행 중 UI 실측

> 조사일: 2026-08-30 · 대상: `/Applications/Superkey.app` (v1.66, build 66)
> 방법: 번들 메타데이터 조회(`plutil`·`codesign`·`otool`·`nm`·`strings`) + 실행 중 앱의 Accessibility(AX) 트리 판독
> ⛔ **바이너리 디컴파일·역어셈블은 하지 않았다.** 문자열·심볼 테이블·링크 정보·nib 리소스까지만이다.
> ⚖️ 아래 인용된 영어 문구는 전부 **SuperKey 원문**이며, 기능 식별을 위한 인용이다. Ultrakey 의 UI 문자열로 사용하지 않는다.

이 문서는 [`spec-verification-report.md`](spec-verification-report.md) 와 `docs/spec/*.md` 가 인용하는 **1차 근거**다.

---

## 0. 근거 등급 표기

명세에서 다음 표기를 쓴다.

| 표기 | 의미 |
| :--- | :--- |
| `(실측: 0N-*.png)` | 환경설정 스크린샷 5장에서 직접 판독 |
| `(실측: AX 트리)` | 실행 중 앱의 Accessibility 트리에서 판독. 본 문서 §6 |
| `(실측: 번들 심볼)` | `otool -L` / `nm -u` 결과. 본 문서 §3 |
| `(실측: 번들 문자열)` | 실행 파일 `strings` 또는 nib 리소스 문자열. 본 문서 §4·§5 |
| `(실측: defaults)` | `~/Library/Preferences/com.knollsoft.Superkey.plist`. 본 문서 §2 |
| `(미확정)` | 관찰로 확정하지 못함. 추측으로 메우지 않는다 |

---

## 1. 번들 메타데이터

`plutil -p /Applications/Superkey.app/Contents/Info.plist`

| 키 | 값 | 의미 |
| :--- | :--- | :--- |
| `CFBundleIdentifier` | `com.knollsoft.Superkey` | 조사 §5 의 추정이 확정됨 |
| `CFBundleShortVersionString` / `CFBundleVersion` | `1.66` / `66` | General 탭 표기와 일치 |
| `LSMinimumSystemVersion` | **`12.0`** | ⭐ ScreenCaptureKit(12.3+)을 쓰지 않는다는 강한 방증 (§3 에서 확정) |
| `LSUIElement` | `true` | Dock 아이콘 없는 메뉴바 상주 앱 |
| `NSMainStoryboardFile` | `Main` | AppKit + Storyboard. Tauri/WebView 아님 |
| `LSApplicationCategoryType` | `public.app-category.utilities` | — |
| `SUFeedURL` | `https://superkey.app/downloads/updates.xml` | Sparkle appcast |
| `SUPublicEDKey` | `lpt9M3PhocbZ3MZiLH+crEqRfU11kfoNzGxSqiEIdvM=` | EdDSA(ed25519) 서명 검증. 조사의 추정이 확정됨 |
| `SUScheduledCheckInterval` | **`172800`** (48시간) | ⭐ Sparkle 기본값 24시간이 아니다 |
| `NSHumanReadableCopyright` | `Copyright © 2022-2026 Ryan Hanson. All rights reserved.` | — |
| usage description 키 (`NS*UsageDescription`) | **없음** | 마이크·카메라·AppleEvents 등 어떤 것도 선언하지 않는다 |
| `CFBundleLocalizations` | **없음** | ⭐ §5.1 참조 |

### 1.1 서명·아키텍처·샌드박스

```
codesign -dv --entitlements :- /Applications/Superkey.app
```

| 항목 | 값 |
| :--- | :--- |
| 아키텍처 | Universal (`x86_64` + `arm64`) |
| Team ID | `XSYZ3E4B7D` |
| Hardened Runtime | **활성** (`flags=0x10000(runtime)`) |
| Entitlements | **`com.apple.security.cs.allow-jit` 단 하나** |
| App Sandbox | **없음** (`com.apple.security.app-sandbox` 미선언) |

⭐ **판정에 중요**: TCC 권한(Accessibility·Input Monitoring·Screen Recording)은 **entitlement 로 선언되지 않는다.** 런타임에 시스템이 부여하는 것이며, 번들에는 흔적이 남지 않는다. 즉 "어떤 권한이 필요한가"는 entitlement 가 아니라 **링크된 API(§3)** 로만 판정할 수 있다.

### 1.2 번들 구조

```
Contents/
  MacOS/Superkey                       (universal, Swift + Objective-C)
  Frameworks/Sparkle.framework          v2.9.2  (자동 업데이트)
  Frameworks/Paddle.framework           v1.0.0  (라이선싱·결제)
  Library/LoginItems/SuperkeyLauncher.app        (로그인 항목 헬퍼)
  Resources/
    Base.lproj/Main.storyboardc          환경설정 창 · 권한 모달 · 메뉴
    Info.storyboardc                     ⓘ 정보 팝오버 4종
    About.storyboardc                    About 창
    Screenshot.storyboardc               (용도 미확정)
    Assets.car, AppIcon.icns
    KeyboardShortcuts_KeyboardShortcuts.bundle   (SPM 리소스 번들)
    Tools_Tools.bundle                            (SPM 리소스 번들)
    Logging_Logging.bundle                        (SPM 리소스 번들)
```

- `/Library/LaunchAgents` · `~/Library/LaunchAgents` 에 SuperKey 항목 **없음**. 로그인 실행은 LaunchAgent plist 가 아니라 **`SuperkeyLauncher.app` + ServiceManagement** 경로다(§3.2).
- `KeyboardShortcuts_KeyboardShortcuts.bundle` 은 Sindre Sorhus 의 오픈소스 `KeyboardShortcuts` 패키지다. `Toggle Seek with shortcut:` 의 `Record Shortcut` 레코더가 이것이다 (§4 의 `KeyboardShortcuts.RecorderCocoa` 심볼로 확인).

---

## 2. 사용자 설정 저장 (실측: defaults)

**저장 위치**: `~/Library/Preferences/com.knollsoft.Superkey.plist` (표준 `NSUserDefaults`).
**추가 저장 위치**: `~/Library/Application Support/Superkey/750314.padl`, `750314.spadl` — Paddle 라이선스 캐시(바이너리 plist). `750314` 은 Paddle 제품 ID.
`~/Library/Containers/` 에 항목 없음(비샌드박스와 정합).

### 2.1 ⭐ 조사 시점의 실제 plist 전량

```
{
  "hyperFlags"                    => 1966080
  "lastVersion"                   => "66"
  "minAxCharCount"                => 2
  "NSWindow Frame EntryBarWindow" => "1692 1147 400 40 0 0 3840 1570 "
  "Paddle-Superkey-750314-SD"     => "323ef0…0313"
  "SUEnableAutomaticChecks"       => false
  "SUHasLaunchedBefore"           => true
}
```

**이것이 Q3("각 설정의 출고 기본값")에 대한 결정적 답이다.** 사용자가 어떤 설정도 바꾸지 않은 상태에서 plist 에는 **7개 키밖에 없다**. 즉 `Remap key to hyper key`, `Remap caps lock to`, 16개 프리셋 등 **나머지 모든 설정은 키 자체가 존재하지 않는다 = 값이 없을 때의 기본값으로 동작한다.** §6 의 AX 실측에서 그 전부가 **꺼짐(☐)** 으로 관찰되었으므로, **출고 기본값은 전부 OFF** 로 확정된다.

예외적으로 값이 있는 두 키:

| 키 | 값 | 해석 |
| :--- | :--- | :--- |
| `hyperFlags` | `1966080` | ⭐ `0x1E0000` = `kCGEventFlagMaskShift(0x20000)` \| `Control(0x40000)` \| `Alternate(0x80000)` \| `Command(0x100000)`. 즉 **`⌃⌥⌘⇧` 4개 비트가 모두 켜진 `CGEventFlags` 비트마스크**. `Include shift in hyper key` 가 기본 ☑ 라는 UI 관찰과 정확히 일치한다. **hyper 조합은 불리언 여러 개가 아니라 `CGEventFlags` 마스크 하나로 저장된다.** |
| `minAxCharCount` | `2` | `Match on more than one character` 의 실체. 불리언이 아니라 **최소 글자 수 정수**다(§6.1) |

### 2.2 실행 파일에서 확인된 설정 키 이름 (실측: 번들 문자열)

UI 항목 ↔ 저장 키 대응. 스크린샷·AX 로 확인된 UI 와 짝지어진 것만 적는다.

| UI 항목 (SuperKey 원문) | 저장 키 후보 | IB 아웃렛 |
| :--- | :--- | :--- |
| `Remap key to Seek:` | `seekRemapKeycode` | `remapKeySeekSelect` |
| `Only show while the remapped key is held` | `seekExecuteOnClose` | `holdModeCheckbox` |
| `Seek using macOS accessibility` | (`seekOptions` 비트) | `axSeekCheckbox` |
| `Match on more than one character` | `minAxCharCount` (정수) | `axMoreThanOneCheckbox` |
| `Only Seek in the frontmost window` | `seekFrontmostOnly` | `onlyFrontmostCheckbox` |
| `Focus window before clicking` | — | `frontmostFirstCheckbox` |
| `Semicolon highlights next match` | `semicolonCycleSeek` | `semicolonCycleCheckbox` |
| `Change click modes with modifier keys` | (`seekOptions` 비트) | `clickModesCheckbox` |
| `Remap key to hyper key:` | `hyperFlags` + 키코드 | `keyRemapCheckbox` / `physicalKeySelect` |
| `Include shift in hyper key` | `hyperFlags` 의 shift 비트 | `includeShiftCheckbox` |
| `Remap key to meh key (⌃⌥⇧):` | — | `mehRemapCheckbox` / `mehKeySelect` |
| `Remap key to bleh key (⌃⌘⇧):` | — | `blehRemapCheckbox` / `blehKeySelect` |
| `Click`/`Drag`/`Move`/`Scroll` | — | `clickEventsCheckbox` / `dragEventsCheckbox` / `moveEventsCheckbox` / `scrollEventsCheckbox` |
| `Engage hyper key using trackpad:` | `oneSwipeFromTop` | `trackpadCheckbox` / `trackpadGestureSelect` |
| `Change menu bar icon when engaged` | `changeMenuBarIcon` | `changeMenuBarIconCheckbox` |
| `Provide haptic feedback when triggered` | — | `hapticFeedbackCheckbox` |
| `Remap caps lock to:` | `capsLockRemapped` | `capsLockRemapCheckbox` / `capsLockRemapSelect` |
| `Quick press caps lock to execute:` | — | `quickPressCheckbox` / `quickPressKeySelect` |
| `Quick press duration` | `quickPressTimeout` | `quickPressSlider` / `quickPressDurationLabel` |
| `Caps lock + space = enter` | `capsSpaceEnter` | `capsSpaceCheckbox` |
| `Caps lock + W A S D = …` | `capsWasdArrows` | `capsWasdCheckbox` |
| `Caps lock + [H J K L] = …` | `capsHjklArrows` / `capsIjklArrows` | `capsHjklCheckbox` / `hjklSelect` |
| `Caps lock + home row = …` | `capsHomeSymbol` / `capsHomeFunction` | `capsHomeSymbolCheckbox` / `capsHomeSelect` |
| `Double tap shift = caps lock` | `doubleShiftToCaps` | `doubleTapShiftCheckbox` |
| `Left shift + right shift = caps lock` | `leftRightShiftToCaps` | `leftRightShiftCheckbox` |
| `Shift + caps lock = caps lock` | `shiftPlusCapsToCaps` | `shiftPlusCapsCheckbox` |
| `Quick press left or right shift …` | `shiftToBraceEntersString` / `shiftToBraceSelection` | `shiftToBraceCheckbox` / `shiftToBraceSelect` |
| `Hyper + delete = forward delete` | `hyperDeleteToForward` | `hyperDeleteCheckbox` |
| `Remap delete to forward delete` | — | `deleteToForwardCheckbox` |
| `Shift + delete = forward delete` | `shiftDeleteToForward` | `shiftDeleteCheckbox` |
| `Remap paste (⌘+V) …` | — | `pasteRemapCheckbox` / `pasteRemapSelect` |
| `Home & end operate on lines` | — | `homeEndCheckbox` |
| `Launch on login` | — | `launchOnLoginCheckbox` |
| `Check for updates automatically` | `SUEnableAutomaticChecks` | `checkForUpdatesAutomaticallyCheckbox` |
| `Hide menu bar icon` | — | `hideMenuBarIconCheckbox` |
| `Menu bar icon` | — | `menuBarIconSelect` |
| `Relaunch on wake` | `restartKeyListenOnWake` | `wakeRelaunchCheckbox` |
| `Apply hyper to arrows` | `applyHyperToCapsArrows` | `applyHyperToArrowsCheckbox` |
| (Windows 키보드 리매핑, 라벨 미확정) | — | `winKeyRemapCheckbox` |

### 2.3 UI 에 노출되지 않는 내부 키 (실측: 번들 문자열)

사용자 설정으로 보이지만 4개 탭 어디에도 컨트롤이 없는 것들. **모두 (미확정)** — 숨은 설정인지, 코드 내부 상수인지, 조건부로만 나타나는지 확인되지 않았다.

`autoRestartOnWake` · `autoRestartOnSessionActive` · `autoRefreshKeyRemap` · `continuousKeyLoop` · `restartKeyListenOnWakeDelay` · `restartOnWakeDelay` · `wakeKeyboardDelay` · `keyboardConnectionDelay` · `touchListenerDelay` · `stopKeyListenOnSleep` · `keepExistingIohid` · `hyperAsCapsLockDisables` · `hyperAsCapsLockManualKeypress` · `longPressCapsLockTurnsItOff` · `capsArrowsOverrideModifiers` · `customKeyCycleSeek` · `enterSelectsMatch` · `persistMatches` · `persistPosition` · `highlightFirst` · `reloadMatchesDelay` · `disablePalmRejection` · `disableThumbRejection` · `filterLargeTouches` · `filterLightTouches` · `cornerTriggerThreshold` · `cornerFreezeThreshold` · `oneTopTriggerThreshold` · `oneTopCursorFreezeThreshold` · `doubleClickInterval` · `hyperDownTime` · `quickHyperKeycode` · `syncAfterEachWrite`

---

## 3. ⭐ 링크된 프레임워크와 사용 API (실측: 번들 심볼)

`otool -L Contents/MacOS/Superkey` · `nm -u Contents/MacOS/Superkey`

### 3.1 링크 판정표

| 프레임워크 | 링크 | 근거 |
| :--- | :---: | :--- |
| **Vision** | ✅ | `VNRecognizeTextRequest`, `VNRecognizedTextObservation`, `VNRecognizedText`, `VNImageRequestHandler`, `VNImageRectForNormalizedRect` |
| **ScreenCaptureKit** | ❌ | `SCStream`·`SCContentFilter`·`SCShareableContent` 심볼 **전무** |
| **CoreGraphics** | ✅ | `CGEventTapCreate/Enable/IsEnabled`, `CGEventCreateKeyboardEvent/MouseEvent`, `CGEventGet/SetFlags`, `CGEventPost`, `CGEventKeyboardSetUnicodeString`, `CGEventSourceCreate`, **`CGDisplayCreateImage`**, **`CGDisplayCreateImageForRect`**, `CGWindowListCopyWindowInfo`, `CGWindowListCreateDescriptionFromArray` |
| **ApplicationServices** (AX 포함) | ✅ | `AXIsProcessTrusted`, `AXUIElementCreateSystemWide/Application`, `AXUIElementCopyAttributeValue`, `AXUIElementCopyElementAtPosition`, `AXUIElementPerformAction`, `AXUIElementSetAttributeValue`, `AXUIElementIsAttributeSettable` |
| **Carbon (HIToolbox)** | ✅ | `UCKeyTranslate`, `LMGetKbdType`, `TISCopyCurrentKeyboardInputSource`, `TISCopyCurrentASCIICapableKeyboardLayoutInputSource`, `TISGetInputSourceProperty`, `kHISymbolicHotKey*`, `CarbonKeyboardShortcuts` |
| **CoreImage** | ✅ | `CILanczosScaleTransform`, `CIPhotoEffectMono`, `CIPhotoEffectNoir`, `CIMaximumComponent`, `CIMinimumComponent` |
| **IOKit** | ✅ | `IOHIDManagerCreate/Open/Close/ScheduleWithRunLoop`, `IOHIDManagerRegisterInputValueCallback`, `IOHIDManagerRegisterDeviceMatching/RemovalCallback`, `IOHIDDeviceGetProperty/Service`, `IOHIDEventSystemClientCreateSimpleClient`, `IOHIDServiceClientSetProperty`, **`IOHIDGet/SetModifierLockState`** |
| **QuartzCore** | ✅ | 직접 링크 |
| **MultitouchSupport** (⚠️ PrivateFramework) | ✅ | `MTDeviceCreateList`, `MTDeviceCreateFromDeviceID`, `MTRegisterContactFrameCallbackWithRefcon`, `MTDeviceStart`, `MTDeviceGetSensorSurfaceDimensions`, `MTDeviceIsBuiltIn`, `MTDeviceSupportsForce`, `MTDeviceGetFamilyID` |
| **ServiceManagement** | ✅ | `SMLoginItemSetEnabled` **및** `SMAppService` — 두 경로 모두 |
| **SwiftUI** / **AppKit** / **SpriteKit** | ✅ | UI 계층 |
| **Security** | ✅ | `SecStaticCodeCreateWithPath`, `SecCodeCopySigningInformation` 등 — 자체 코드 서명 검증(라이선스 보호) |
| **SystemConfiguration** | ✅ | `SCNetworkReachability` — 구매·라이선스 온라인 판정 |
| **Sparkle** 2.9.2 / **Paddle** 1.0.0 | ✅ | 번들 내 `@rpath` |
| Metal / MetalPerformanceShaders | ❌ | Swift 런타임 shim(`libswiftMetal.dylib`)만 weak, 실사용 근거 없음 |
| 그 외 PrivateFrameworks | — | **MultitouchSupport 하나뿐** |

Swift(강한 링크 `libswiftCore`·`libswiftVision`·`libswiftAppKit` 등) + Objective-C(`libobjc.A.dylib`) 혼용.

### 3.2 ⭐ 이 결과가 바꾸는 판정 4가지

1. **화면 캡처는 `CGDisplayCreateImage` 계열이다 — ScreenCaptureKit 이 아니다.**
   `LSMinimumSystemVersion = 12.0` 과 정합한다. `docs/spec/README.md` 의 제품 결정 **D1("최소 macOS 를 12.3 으로 올릴 것인가")은 근거를 잃는다** — 원본은 12.0 을 유지하면서 레거시 API 로 출하 중이다. `CGDisplayCreateImage` 는 macOS 14 에서 deprecated 이지만 v1.66(2026-06 빌드, SDK macosx26.5)에서 여전히 동작한다.

2. **`IOHIDGetModifierLockState` / `IOHIDSetModifierLockState` 를 쓴다.**
   caps lock 의 **실제 잠금 상태(대문자 고정 + LED)** 를 읽고 쓰는 API다. `Double tap shift = caps lock` 류 프리셋이 "진짜 caps lock 을 토글한다"는 명세의 서술이 이 심볼로 확정된다 — 키 이벤트 합성이 아니라 HID 잠금 상태 직접 조작이다.

3. **`hidutil` 을 셸로 호출한다.**
   실행 파일에 문자열 `hidutil property -g UserKeyMapping` 과 `HIDKeyboardModifierMappingSrc` / `HIDKeyboardModifierMappingDst` 가 있고, `Tools_Tools.bundle` 에 셸 실행 오류 문자열(`bash command error: %@`, `Error parsing plist response`)이 있다.
   ⭐ 즉 **caps lock 리매핑은 `CGEventTap` 만으로 하지 않는다.** 커널 레벨 HID modifier 매핑(`hidutil`)을 병용한다. 이는 Secure Input 구간에서도 유효하다는 중요한 차이를 만든다. 메뉴바 `Advanced ▸ Synthesize Caps Lock Remap` 항목(§6.4)이 "HID 매핑 대신 이벤트 합성으로 대체"하는 스위치로 보인다 `(미확정 — 라벨로부터의 해석)`.

4. **시스템 예약 단축키를 조회한다.**
   `kHISymbolicHotKeyCode` / `kHISymbolicHotKeyEnabled` / `kHISymbolicHotKeyModifiers` 심볼이 있다(`CopySymbolicHotKeys` 계열). `platform-constraints.md` 가 "시스템 예약 조합 전체를 열거할 공개 API 가 없다"고 판정했으나, 원본은 이 (문서화되지 않은) Carbon 경로를 쓴다.

### 3.3 권한 판정 — ⭐ 앱이 명시적으로 확인하는 권한은 Accessibility 하나뿐

| 권한 | 앱의 확인/요청 심볼 | 판정 |
| :--- | :--- | :--- |
| Accessibility | **`AXIsProcessTrusted`** (있음) | ✅ 명시적으로 확인한다. 권한 모달(`01-authorize-accessibility.png`)이 안내하는 유일한 권한 |
| Input Monitoring | `IOHIDCheckAccess` / `IOHIDRequestAccess` **없음** | ❌ 명시적으로 확인하지 않는다. 대신 실패를 붙잡고 재시도한다 — 문자열 `IOHIDManagerOpen failed with kIOReturnNotPermitted. Retrying... (Attempt ` |
| Screen Recording | `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` **없음** | ❌ 명시적으로 확인하지 않는다. `CGDisplayCreateImage` 호출 시 OS 가 암묵적으로 요구·프롬프트한다 |

`AXIsProcessTrustedWithOptions`(프롬프트 표시 변형) 도 없다 — 즉 **시스템 프롬프트를 띄우지 않고, 자체 모달로 사용자를 시스템 설정에 안내한다.** 이는 명세가 "3종 권한을 온보딩에서 함께 요청한다"고 기술한 것과 다르다.

---

## 4. 실행 파일 문자열에서 발굴한 기능 (실측: 번들 문자열)

스크린샷·AX 로는 보이지 않는 기능들. 인용은 전부 **SuperKey 원문**이다.

### 4.1 ⭐ 설정 충돌 감지 대화상자 — 명세에 전혀 없던 기능

```
Conflict: Caps lock already remapped
Conflict: Caps lock arrows
Conflict: Caps lock + home row
Disable other remappings and continue with this one?
Disable that setting and enable WASD arrows?
Disable that setting and enable caps + home row as
Remap caps lock to
```

`superkey-inventory.md` §6.2 와 `power-user-presets.md` §3.3 이 "충돌이 조용히 무시되어서는 안 된다"고 요구한 것이, 원본에서는 **사용자에게 묻고 상대 설정을 꺼주는 대화상자**로 구현되어 있다. 충돌 감지 대상 3종이 이름으로 확인된다: caps lock 리매핑 중복 / caps lock 방향키 프리셋 / caps lock home row 프리셋.

### 4.2 ⭐ 앱별 비활성화 — 명세에 전혀 없던 기능

```
Ignore frontmost.app          (템플릿 문자열)
disabledApps / enabledApps / disabledForApp / frontAppId / frontAppName
typeToSeekEnabledAppIDs / typeToSeekDisabledAppIDs
```

메뉴바 메뉴에서 실측 확인됨(§6.4): 현재 최전면 앱 이름이 들어간 **`Ignore <앱이름>`** 항목. `typeToSeek*` 키는 별도의 앱별 목록으로, 그 기능의 정체는 `(미확정)`.

### 4.3 ⭐ iCloud 설정 동기화 — 명세에 전혀 없던 기능

```
Do you want to import your existing iCloud configuration?
No, upload my current configuration
Incoming cloudConfigTimestamp:
overriding existing cloud config
syncAfterEachWrite
```

설정을 iCloud 로 동기화하고, 첫 실행 시 기존 클라우드 설정을 가져올지 묻는다. 저장소는 `NSUbiquitousKeyValueStore` 로 추정되나 심볼로 확인되지 않았다 `(미확정)`.

### 4.4 권한 실패·복구 흐름 (F-11 근거)

```
Superkey has insufficient privileges
Superkey is unable to listen to input from your devices.
Unable to listen to device input
It appears that the accessibility settings in macOS are out of sync and Superkey might not be able to
process your input. You can let Superkey try to reset privileges itself, or try to manually fix this by
performing the following:
Reset Priviliges & Restart Superkey          ← 원문 오타 그대로
Superkey will now close and attempt to open System Preferences for you.
Try removing Superkey from the list in System Preferences
Then relaunch Superkey and check the box in System Preferences again when prompted.
Accessibility, disable then remove Superkey.
Not Authorized to Control Your Computer
com-knollsoft-Superkey Failed to create event tap. Exiting program
```

⭐ **자체 TCC 리셋 시도**(`Reset Priviliges & Restart Superkey`)와 **수동 복구 안내**(목록에서 제거 → 재실행 → 재승인) 두 경로가 실재한다.

### 4.5 이벤트 탭 생명주기 감시 (F-07 근거)

```
Key loop starting.        Key loop started running
Checking key loop.        Key loop appears to not be running,
Key loop no longer exists. Resume key loop.
Received sleep notification / Received wake notification
Restarting keyboard listener on wake
Stopping listening
Time since last exit:     s ago, no need to restart.
Key remap cleared. Reloading key remapping
Detected keyboard:
```

`NSWorkspaceWillSleepNotification` / `DidWakeNotification` / `SessionDidBecomeActiveNotification` 심볼과 함께, **주기적 워치독 + 절전·깨어남·세션 전환 훅**이 실재함을 보여준다. `key-remapping-engine.md` 의 재활성화 설계가 실측으로 뒷받침된다.

### 4.6 트랙패드·마우스 인식 (F-06 근거)

```
Registered trackpad / Unregistered trackpad / Unregistered Magic Mouse
Touches are not being detected. Restarting.
Unable to obtain device dimensions / Unable to obtain deviceId / Unable to register device
MT callback with nil values
Apple Internal Keyboard / Trackpad
Viewing windows will be displayed for each device when touches are detected on that device
```

내부 타입: `MTTrackpadRegistrar`, `MTMouseRegistrar`, `MTTouchTracker`, `OneDrawGesture`, `OneDrawRecognizer`, `RestingThumbState`, `ForceRecognizer`, `TapRecognizer`, `ClickRecognizer`, `fourTapRecognizer`, `threeTapRecognizer`.
상태 키: `restingThumb`, `restingPalm`, `restingTopThumb`, `restingBottomThumb`, `wristTouches`, `tooLightTouches`, `largeTouchMajorAxis`, `zDensity`, `zPressure`, `magicMouseConnected`, `forceTouchConnected`, `trackpadConnected`, `hasBuiltInTrackpad`.

⭐ **Magic Mouse 도 등록 대상**이다. 그리고 손바닥·엄지 얹힘 거부(palm/thumb rejection)가 제스처 인식의 일부다 — 명세가 다루지 않은 영역이다.

### 4.7 Seek 내부 구조 (F-01~F-04 근거)

내부 타입: `EntryBarWindow`, `EntryBarViewController`, `EntryOutlineView`, `EntryBarTableRowView`, `EntrySearchButton`, `OverlayWindow`, `ScreenCapture`, `ScreenDetection`, `ScreenSettler`, `VisionManager`, `TextRecognition`, `RecognitionResult`, `PartialMatchData`, `SelectedMatch`, `CaptureFiltering`, `AccessibilityParsing`, `UnobscuredWindowElement`, `StageWindowAccessibilityElement`, `WindowObservation`.

⭐ 두 가지가 중요하다.

- **검색 바는 위치·크기가 저장되는 독립 창이다.** `defaults` 의 `NSWindow Frame EntryBarWindow = "1692 1147 400 40 …"` → **400 × 40 pt**, 사용자가 옮긴 위치가 보존된다(`persistPosition`). 오버레이 안에 그려지는 고정 요소가 아니다.
- **캡처 전처리 파이프라인이 존재한다.** `CILanczosScaleTransform`(스케일) → `CIPhotoEffectMono`/`CIPhotoEffectNoir`(흑백) → `CIMaximumComponent`/`CIMinimumComponent`(성분 추출). OCR 정확도를 위한 전처리이며, `CaptureFilterType` 로 선택된다.
- `StageWindowAccessibilityElement` — **Stage Manager** 대응 코드가 별도로 있다.

### 4.8 라이선스·구매 (F-12 근거)

Paddle **Classic** 프레임워크(`PADActivateWindow.nib`, `PADCheckoutWindow.nib`, `PADProductWindow.nib`, `PADProductNoTrialWindow.nib`)를 그대로 쓴다. 제품 ID `750314`.

SuperKey 자체 문자열:
```
Remove oldest license activation
Oldest license activation removed
Are you sure you want to remove this device from your activations?
If your license activations have all been utilized, you can remove the oldest.
Enter your license key for deactivation.
You can reactivate with your license key at any time.
License activation not found / Unable to deactivate license
Unable to initiate purchase without internet connection.
Thanks for trying Superkey!
```
Paddle 제공 문자열: `Activate License`, `Deactivate License`, `Recover your %@ license`, `Forgotten your license key?`, `You have %d days remaining of your trial period.`, `Continue Trial`, `Order Complete` 등.

⭐ `Remove Oldest Activation` 은 **자체 UI 가 아니라 Paddle 의 비활성화 API 를 라이선스 키 입력으로 호출하는 흐름**이다(안내 문구가 "Enter your license key for deactivation" 이다).

---

## 5. 리소스·현지화

### 5.1 ⭐ 현지화 — 앱 본체는 영어 단일이다

`find /Applications/Superkey.app -name '*.lproj'` 결과:

| 위치 | 로케일 |
| :--- | :--- |
| **`Contents/Resources/`** | **`Base.lproj` 하나뿐** |
| `Contents/Library/LoginItems/SuperkeyLauncher.app/…` | `Base.lproj` 하나뿐 |
| `Contents/Frameworks/Sparkle.framework/…` | 45개 (`ar` `ca` `cs` `da` `de` `el` `es` `fa` `fi` `fr` `he` `hr` `hu` `is` `it` `ja` `ko` `nb` `nl` `nn` `pl` `pt-BR` `pt-PT` `ro` `ru` `sk` `sl` `sv` `th` `tr` `uk` `vi` `zh_CN` `zh_HK` `zh_TW` …) |
| `Contents/Frameworks/Paddle.framework/…` | 12개 (`de` `en` `es` `fr` `it` `ja` `nl` `pl` `pt` `ru` `zh-Hans` `zh-TW`) |
| `Contents/Resources/KeyboardShortcuts_…bundle/…` | 15개 |

Info.plist 에 `CFBundleLocalizations` 키가 **없다**.

⭐ **`superkey-inventory.md` §2.3 이 appcast 의 `sparkle:deltaFromSparkleLocales="de,he,ar,el,ja,fa,uk"` 로부터 "앱이 8개 로케일을 번들한다"고 판정한 것은 오독이다.** 그 속성은 **델타 업데이트가 건드린 Sparkle 프레임워크의 로케일 파일 목록**이며, SuperKey 자신의 UI 문자열과 무관하다. 목록의 로케일이 전부 Sparkle 의 45개 안에 있다는 사실이 이를 뒷받침한다.

**결론: SuperKey v1.66 의 UI 는 영어 단일이고, 현지화 기능은 존재하지 않는다. RTL 대응도 없다.**

### 5.2 nib 리소스에서 판독한 UI 텍스트

Storyboard 는 `Base.lproj/Main.storyboardc`(환경설정 창·권한 모달·메뉴)와 `Info.storyboardc`(ⓘ 팝오버 4종)로 나뉜다.

**`Info.storyboardc` — ⓘ 팝오버 전량 (SuperKey 원문)**

`SeekInfoViewController` — Seek 탭 제목 옆 ⓘ (AX 로도 재확인):
> "Search on screen for text."
> "Use the arrow keys or tab to cycle through matches, and press enter to execute a mouse click."

`SeekAccessibilityViewController` — `Seek using macOS accessibility` 옆 ⓘ:
> "Seek using macOS Accessibility"
> "Enable this to let Seek potentially find more text items in the frontmost window."
> "It's not possible to determine precise locations of text within an Accessibility element, so the entire element will be highlighted by Seek."
> "Seek matches using Optical Character Recognition (OCR) will display in place of duplicate matches from Accessibility."
> "Sometimes text found in an accessibility element can be obscured, so you might get matches even if you can't see the text in the element."

⭐ **이슈 #3 의 질문("`Seek using macOS accessibility` 가 OCR 과 배타적인가")에 대한 확정 답: 배타적이지 않다.** OCR 이 항상 동작하는 기본 소스이고, AX 는 **최전면 창에 한해 후보를 추가**하는 보강 소스다. 중복되면 **OCR 매치가 AX 매치를 대체**한다. AX 매치는 텍스트의 정확한 위치를 알 수 없어 **요소 전체가 하이라이트**되고, 가려진 텍스트도 매치될 수 있다.

`ClickModesInfoViewController` — `Change click modes with modifier keys` 옆 ⓘ:
> "Seek Click Modes"
> "Hold the corresponding modifier keys when pressing enter to perform each type of click"
> 목록: "Just move cursor" · "Click at beginning of match" · "Click at end of match" · "Click and return cursor" · "Click, return, click" · "Double click and copy" · "Triple click and copy"

⭐ **클릭 모드는 7종**이다. 내부 식별자: `onlyMoveCursor` · `clickStartMatch` · `clickEndMatch` · `clickAndReturn` · `clickReturnClick` · `doubleClickCopy` · `tripleClickCopy`. 팝오버 아이콘은 SF Symbol `cursorarrow.motionlines` · `arrow.left.to.line` · `arrow.right.to.line` · `cursorarrow.motionlines.click` · `contextualmenu.and.cursorarrow` · `cursorarrow.click.2` · `doc.on.doc.fill`.
**어떤 modifier 가 어느 모드에 대응하는지는 nib 텍스트에도 AX 트리에도 없다 `(미확정)`.** 실행 파일에 `Record Modifiers` 와 `KeyboardShortcuts.RecorderModifierCocoa` 가 있어 **사용자가 모드별 modifier 를 직접 녹화**하는 구조로 보이나, 그 UI 는 4개 탭 어디에서도 관찰되지 않았다.

`HyperkeyInfoViewController` — Hyperkey 탭 제목 옆 ⓘ:
> "Convert your caps lock key or any of your modifier keys to the hyper key, all four modifiers combined: "
> "This modifier key combination is unlikely to exist in default shortcuts, so it acts as an extra modifier key."

**권한 모달 nib** — 스크린샷(`01`)에 보이는 것 외에 조건부 문구가 하나 더 있다:
> "If the checkbox is disabled, click the padlock and enter your password"

nib 에는 구형 문구(`Open System Preferences` / `Go to System Preferences`)가 저장되어 있고, 실행 중에는 `Open System Settings` / `Go to System Settings` 로 표시된다 — **OS 버전에 따라 런타임에 문구를 교체**한다.

---

## 6. ⭐ 실행 중 앱 UI 실측 (실측: AX 트리)

방법: 실행 중 `Superkey` 프로세스의 Accessibility 트리를 `System Events` 로 판독. 설정 변경은 §7 에 기록한 3건뿐이며 모두 복원했다.

### 6.1 `Seek` 탭 — 배치 순서 그대로

| # | 컨트롤 | 라벨 (SuperKey 원문) | 기본값 | 활성화 조건 |
| :--- | :--- | :--- | :--- | :--- |
| — | 제목 + ⓘ 버튼 | `Seek` | — | — |
| 1 | 단축키 레코더 (`AXTextField`) | `Toggle Seek with shortcut:` | **빈 값(미설정)** | 항상 |
| 2 | 팝업 (35항목) | `Remap key to Seek:` | **`-`** | 항상 |
| 3 | 체크박스 | `Only show while the remapped key is held` | ☐ | ⭐ **`Remap key to Seek:` ≠ `-` 일 때만 활성**. 아니면 dimmed |
| — | 부제 | `Release the remapped key to click` | — | 항목 3 에 종속 |
| — | 구분선 | | | |
| 4 | 체크박스 + ⓘ | `Seek using macOS accessibility` | ☐ | 항상 |
| 5 | 체크박스 (중첩) | `Match on more than one character` | **☑** | ⭐ **항목 4 가 ☑ 일 때만 존재**. ☐ 이면 dimmed 가 아니라 **완전히 숨겨진다**(AX 트리에서 사라진다) |
| 6 | 체크박스 | `Only Seek in the frontmost window` | ☐ | 항상 |
| 7 | 체크박스 | `Focus window before clicking` | ☐ | 항상 |
| 8 | 체크박스 | `Semicolon highlights next match` | ☐ | 항상 |
| 9 | 체크박스 + ⓘ | `Change click modes with modifier keys` | **☑** | 항상 — **Seek 탭에서 유일하게 기본 켜짐** |
| — | 부제 | `If this setting is disabled, modifiers will be applied to the click` | — | — |

⭐ **종속 표현이 두 가지로 다르다**: 항목 3 은 **비활성화(dimmed)**, 항목 5 는 **숨김(hidden)**. 클론 UI 도 이 구분을 재현해야 한다.

**`Remap key to Seek:` 팝업 선택지 35종** (표시 순서대로):
`-` · `caps lock` · `right option` · `right shift` · `right command` · `right control` · `left option` · `left shift` · `left command` · `left control` · `menu (PC)` · `F1`…`F24`
→ ⭐ **`globe` 이 없다.** Hyperkey 탭에는 있다(§6.2). 두 팝업의 열거형이 다르다.

### 6.2 `Hyperkey` 탭

| # | 컨트롤 | 라벨 (SuperKey 원문) | 기본값 | 비고 |
| :--- | :--- | :--- | :--- | :--- |
| — | 제목 + ⓘ | `Hyperkey` | — | |
| 1 | 키캡 일러스트 + 체크박스 + 팝업(35) | `Remap key to hyper key:` | **☐ / `caps lock`** | 일러스트는 팝업 값을 반영 |
| 2 | 조합 미리보기 + 체크박스 | `Include shift in hyper key` | **☑** | 왼쪽에 현재 조합 `⌃⌥⌘⇧` 를 텍스트로 표시 |
| — | 구분선 | | | |
| 3 | 키캡 + 체크박스 + 팝업(35) | `Remap key to meh key (⌃⌥⇧):` | **☐ / `caps lock`** | |
| 4 | 키캡 + 체크박스 + 팝업(35) | `Remap key to bleh key (⌃⌘⇧):` | **☐ / `caps lock`** | |
| — | 구분선 | | | |
| 5 | 라벨 + 체크박스 4개 | `Apply modifiers to keypress events and:` `Click` `Drag` `Move` `Scroll` | **`Click` ☑ · `Drag` ☐ · `Move` ☐ · `Scroll` ☐** | ⭐ 켜진 것은 `Click` 하나뿐 |
| — | 구분선 | | | |
| 6 | 제스처 일러스트 + 체크박스 + 팝업(5) | `Engage hyper key using trackpad:` | **☐ / `top right`** | 일러스트가 선택 영역을 반영 |
| — | 부제 | `Slide only one touch in from the selected area the trackpad a little. Remove the touch to release.` | — | |
| 7 | 체크박스 | `Change menu bar icon when engaged` | ☐ | ⭐ **항목 6 이 ☑ 일 때만 존재**(숨김) |
| 8 | 체크박스 | `Provide haptic feedback when triggered` | ☐ | ⭐ **항목 6 이 ☑ 일 때만 존재**(숨김) |

**hyper/meh/bleh 소스 키 팝업 선택지 35종** (셋 다 동일, 표시 순서대로):
`caps lock` · `right option` · `right shift` · `right command` · `right control` · `left option` · `left shift` · `left command` · `left control` · **`globe`** · `menu (PC)` · `F1`…`F24`

**트랙패드 영역 팝업 선택지 5종** (표시 순서대로):
`top left` · `top right` · `bottom left` · `bottom right` · **`top`**
→ `trackpad-hyper-gesture.md` 가 "4 코너 + 상단 가장자리" 로 추정한 것이 **정확히 맞았다**. `top` 이 상단 가장자리이며, 내부 이미지 이름 `hyperSlideDownTemplate` 과 defaults 키 `oneSwipeFromTop` 이 이를 뒷받침한다.

### 6.3 `Presets` 탭 — 전 항목 기본 ☐

| 그룹 | 라벨 (SuperKey 원문) | 기본값 |
| :--- | :--- | :--- |
| caps lock | `Remap caps lock to:` | ☐ / `left control` |
| caps lock | `Quick press caps lock to execute:` | ☐ / `caps lock` |
| caps lock | `Quick press duration` (슬라이더) | **1000 ms · 최소 250 · 최대 2000** |
| caps lock | `Caps lock + space = enter` | ☐ |
| caps lock | `Caps lock + W A S D = ▲ ◀︎ ▼ ▶︎` | ☐ |
| caps lock | `Caps lock +` [팝업] ` = ◀︎ ▼ ▲ ▶︎` | ☐ / `H J K L` |
| caps lock | `Caps lock + home row = ` [팝업] | ☐ / `symbol row (A = !)` |
| shift | `Double tap shift = caps lock` | ☐ |
| shift | `Left shift + right shift = caps lock` | ☐ |
| shift | `Shift + caps lock = caps lock` | ☐ |
| shift | `Quick press left or right shift to input corresponding:` | ☐ / `( )` |
| delete | `Hyper + delete = forward delete` | ☐ |
| delete | `Remap delete to forward delete` | ☐ |
| delete | `Shift + delete = forward delete` | ☐ |
| 기타 | `Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):` | ☐ / `Right ⌘` |
| 기타 | `Home & end operate on lines` | ☐ |

⭐ **`Quick press duration` 슬라이더: min 250 ms, max 2000 ms, 현재값 1000 ms.** `key-remapping-engine.md` 가 "눈금 8칸으로부터 200–1600ms, 간격 200ms" 로 역산한 추정은 **틀렸다**.

**팝업 선택지 전량**

| 팝업 | 선택지 |
| :--- | :--- |
| `Remap caps lock to:` (50종) | `esc` · **`nothing (disable it)`** · `left control` · `left shift` · `left option` · `left command` · `right control` · `right shift` · `right option` · `right command` · `return (enter)` · `delete (backspace)` · `delete forward` · `tab` · `spacebar` · `home` · `end` · `pageup` · `pagedown` · `left arrow` · `right arrow` · `up arrow` · `down arrow` · `mute` · `volume up` · `volume down` · `F1`…`F24` |
| `Quick press caps lock to execute:` (49종) | ⭐ **`Seek`** · (구분선) · `esc` · `caps lock` · `left control` · `left shift` · `left option` · `left command` · `right control` · `right shift` · `right option` · `right command` · `return (enter)` · `delete (backspace)` · `delete forward` · `tab` · `spacebar` · `home` · `end` · `pageup` · `pagedown` · `left arrow` · `right arrow` · `up arrow` · `down arrow` · `mute` · `volume up` · `volume down` · `F1`…`F20` · **`/`** |
| `Caps lock + [ ]` (2종) | `H J K L` · **`I J K L`** |
| `Caps lock + home row = [ ]` (2종) | `symbol row (A = !)` · **`function row (A = F1)`** |
| `Quick press left or right shift …` (4종) | `( )` · **`[ ]`** · **`{ }`** · **`< >`** |
| `Remap paste …` (4종) | `Right ⌘` · **`Left ⌘`** · **`Either ⌘`** · **`Hyper key`** |

⭐ **`Quick press caps lock to execute:` 의 첫 항목이 `Seek` 다** — quick press 로 Seek 세션을 여는 경로가 존재한다. 이는 `Seek` 탭의 두 활성화 경로(단축키·키 리맵) 외의 **세 번째 활성화 경로**이며, 명세에 없었다.

실행 파일에는 `hjklArrowColemak` · `hjklArrowDvorak` · `ijklArrowColemak` · `ijklArrowDvorak` · `wasdArrowColemak` · `wasdArrowDvorak` 키가 있으나 팝업에는 없다 → **레이아웃 변형은 사용자 선택이 아니라 감지된 키보드 레이아웃에 따라 자동 적용**되는 것으로 보인다 `(미확정 — 팝업 부재로부터의 해석)`.

**조건부로만 나타나는 Presets 항목** — nib 에는 있으나 기본 상태의 AX 트리에는 없다:
`Apply hyper to arrows` (`applyHyperToArrowsCheckbox`) — `Caps lock + W A S D` 만 켜서는 나타나지 않았다. 표시 조건 `(미확정)`. 관련 키 `applyHyperToCapsArrows` · `capsArrowsOverrideModifiers`.
Windows 키보드 리매핑 (`winKeyRemapCheckbox`) — 라벨·표시 조건 모두 `(미확정)`.

### 6.4 `General` 탭

| 컨트롤 | 라벨 (SuperKey 원문) | 기본값 |
| :--- | :--- | :--- |
| 체크박스 | `Launch on login` | ☐ |
| **버튼** | `v1.66 (66)` | ⭐ 정적 라벨이 아니라 **버튼**이다(About 창 추정 `(미확정)`). 첫 행 우측 |
| 체크박스 | `Check for updates automatically` | ☐ (`SUEnableAutomaticChecks = false` 와 일치) |
| 체크박스 | `Hide menu bar icon` | ☐ |
| 부제 | `When hidden, relaunch from Finder to open.` | — |
| 라벨 + 팝업 | `Menu bar icon` | **선택지 2종, 둘 다 라벨 없는 이미지 항목** |
| 버튼 | `Remove Oldest Activation` | — |
| 버튼 | `Purchase` | — (강조색) |

⭐ **명세 `preferences-ui.md` §4.4 가 추정한 항목 중 실재하지 않는 것**: 언어 선택 · 권한 상태 표시 · 라이선스 키 입력 필드 · `Reset to defaults`. **전부 없다.**
조건부 항목: `Relaunch on wake` (`wakeRelaunchCheckbox` / `wakeRelaunchStackView`) — nib 에는 있으나 기본 상태에서 보이지 않는다. 표시 조건 `(미확정)`.

### 6.5 ⭐ 메뉴바 메뉴 — 실측 전량

```
Purchase
────────
Ignore Ghostty              ← 현재 최전면 앱 이름이 동적으로 들어간다
────────
Settings…
Check for Updates…
About
Advanced ▸
    Show Logging…
    Launch Logger on Start
    Log to File
    Log Debug Messages
    Show Viewer…
    Launch Viewer on Start
    Synthesize Caps Lock Remap
    Relaunch After Wake
    Delay Relaunch After Wake
    Relaunch on Keyboard Connected
    Relaunch
Quit Superkey
```

권한이 없을 때는 별도 메뉴(`unauthorizedMenu`)로 교체되며, 항목은 `Not Authorized to Control Your Computer` 와 `Authorize` 다 (실측: 번들 문자열 — 권한이 부여된 상태라 직접 관찰하지는 못했다).

`menu-bar-and-lifecycle.md` 가 전부 `(추정)`으로 남겼던 부분이 여기서 확정된다. 특히 `Advanced` 하위 6개(`Relaunch After Wake`·`Delay Relaunch After Wake`·`Relaunch on Keyboard Connected`·`Synthesize Caps Lock Remap`·`Launch Logger/Viewer on Start`)는 §2.3 의 내부 키(`autoRestartOnWake`·`restartOnWakeDelay`·`keyboardAddedOrRemoved`·`launchLoggerOnStart`·`launchViewerOnStart`)와 짝을 이룬다.

---

## 7. ⭐ 변경했다가 복원한 SuperKey 설정 전량

관찰 전 baseline 을 `defaults read com.knollsoft.Superkey` 로 기록했다(§2.1).
**라이선스 관련 버튼(`Purchase`, `Remove Oldest Activation`)은 한 번도 누르지 않았다.**

| # | 탭 | 변경한 항목 | 변경 전 | 변경 후 | 복원 | 목적 |
| :--- | :--- | :--- | :--- | :--- | :---: | :--- |
| 1 | Seek | `Seek using macOS accessibility` | ☐ | ☑ → ☐ | ✅ | `Match on more than one character` 의 표시 조건·기본값 확인 |
| 2 | Seek | `Remap key to Seek:` | `-` | `F13` → `-` | ✅ | `Only show while the remapped key is held` 의 활성화 조건 확인. caps lock 대신 F13 을 골라 실사용 키에 영향이 없게 했다 |
| 3 | Hyperkey | `Engage hyper key using trackpad:` | ☐ | ☑ → ☐ | ✅ | `Change menu bar icon when engaged` / `Provide haptic feedback when triggered` 의 표시 조건 확인 |
| 4 | Presets | `Caps lock + W A S D = ▲ ◀︎ ▼ ▶︎` | ☐ | ☑ → ☐ | ✅ | `Apply hyper to arrows` 의 표시 조건 확인 (나타나지 않음) |

그 밖에: 팝업 6개를 열어 선택지를 읽고 **Esc 로 닫았다**(선택 변경 없음, 전후 값 동일 확인). 탭 4개를 순회했고 마지막에 최초 탭(`Seek`)으로 되돌렸다. 환경설정 창은 관찰 시작 시 닫혀 있었고 관찰 후 닫아 원상태로 두었다.

**최종 검증**: 4개 탭 전 컨트롤을 다시 판독해 체크 상태·팝업 값·슬라이더 값이 스크린샷 5장과 **전부 일치**함을 확인했다(§6.1~§6.4 표가 그 결과다).

### 7.1 ⚠️ 남은 잔여물 — plist 키 4개가 새로 생성되었다

토글 후 UI 값은 baseline 과 동일하지만, plist 에는 **원래 없던 키 4개가 기본값과 동등한 값으로 기록**되었다.

```
capsWasdArrows    = 2      (#4 로 생성)
oneSwipeFromTop   = 2      (#3 으로 생성)
seekOptions       = 1      (#1 로 생성)
seekRemapKeycode  = 0      (#2 로 생성)
```

- 네 키 모두 **UI 관찰상 baseline 과 동일한 상태**를 나타낸다(§6 최종 검증). `capsWasdArrows` · `oneSwipeFromTop` 의 `2` 는 "꺼짐" 을 뜻하는 열거값으로 보이고, `seekRemapKeycode = 0` 은 팝업이 `-` 를 표시하므로 "미설정" 센티널이다.
- **`defaults delete` 로 지우지 않았다.** 앱이 실행 중이며 이 값들을 메모리에 캐시하고 있어, 디스크만 지우면 메모리와 디스크가 어긋난 상태를 만든다 — 원상복구가 아니라 새로운 불일치를 만드는 행위다. 기능적으로 동등하고 사용자에게 관찰되지 않는 잔여물로 판단해 그대로 두었다.
- ⭐ 이 잔여물 자체가 하나의 실측 사실이다: **SuperKey 는 설정을 "값이 없으면 기본값" 방식으로 다루며, 한 번이라도 건드린 항목만 plist 에 기록한다.** 클론의 설정 저장 계층도 같은 성질(부재 = 기본값)을 가져야 §2.1 같은 기본값 확정이 가능하다.

---

## 8. 조사의 한계 — 확정하지 못한 것

| # | 항목 | 왜 확정하지 못했나 |
| :--- | :--- | :--- |
| 1 | 클릭 모드 7종 ↔ modifier 대응 | nib·AX 어디에도 매핑이 없다. `Record Modifiers` UI 가 어디서 열리는지 관찰하지 못했다 |
| 2 | `Menu bar icon` 팝업 2종의 정체 | 항목이 라벨 없는 이미지다. 에셋은 `Assets.car` 안이며 **추출하지 않았다**(저작권 경계) |
| 3 | Seek 오버레이의 실제 표시 형태 (라벨 문자 집합·배치·색) | Seek 을 실제로 발동시키지 않았다. 발동에는 Screen Recording 권한 프롬프트와 전체 화면 캡처가 따르고, 관찰 이득 대비 부작용이 크다고 판단했다 |
| 4 | quick press 임계 동작의 실제 체감 | 실제 키 입력 타이밍 실험은 하지 않았다. 대신 슬라이더의 정확한 범위(250–2000 ms)를 확정했다 |
| 5 | 트랙패드 제스처의 실제 반응 | 물리 제스처 입력이 필요하다. 대신 영역 5종·임계 파라미터 이름(`cornerTriggerThreshold` 등)을 확정했다 |
| 6 | `Apply hyper to arrows` / `Relaunch on wake` / Windows 키보드 리매핑의 표시 조건 | 조건을 재현하지 못했다 |
| 7 | 충돌 대화상자의 실제 모습·버튼 구성 | 충돌 조건을 만들려면 caps lock 을 실제로 리매핑해야 해 부작용이 크다고 판단했다. 문자열로 존재와 대상 3종만 확정 |
| 8 | iCloud 설정 동기화의 저장소·범위 | 문자열로 존재만 확인. 심볼로 `NSUbiquitousKeyValueStore` 를 확인하지 못했다 |
| 9 | `typeToSeekEnabledAppIDs` / `typeToSeekDisabledAppIDs` 의 기능 | 대응하는 UI 를 찾지 못했다 |
| 10 | `Screenshot.storyboardc` 의 용도 | 창을 띄우는 경로를 찾지 못했다 |
