# F-11 · 권한 온보딩과 복구

> **한 줄 요약**: ⭐ 앱이 명시적으로 확인·요청하는 TCC 권한은 **Accessibility 하나뿐**이다(app-bundle-analysis.md §3.3, 실측: 번들 심볼). Input Monitoring 은 확인 없이 실패를 붙잡아 재시도하고, Screen Recording 은 첫 캡처(`CGDisplayCreateImage`) 시점에 macOS 가 암묵적으로 프롬프트한다. 이 문서는 Accessibility 온보딩의 자체 모달(시스템 프롬프트가 아니다) 흐름, 권한 DB 불일치("out of sync") 진단·복구 흐름, event tap 생성 실패의 치명적 종료(F-10 과 연결), 그리고 개발 환경에서 권한을 안정적으로 유지하기 위한 코드 서명 워크플로를 정의한다.
> **의존성**: 없음(최상위 전제 조건) — `F-07`(key-remapping-engine)이 `CGEventTap` 을 설치하려면 F-11 이 부여한 Accessibility 가 이미 있어야 한다. `F-01`~`F-04`(Seek 계열)가 OCR 경로를 쓰려면 Screen Recording 이 필요하지만, 그 권한은 **F-11 의 온보딩이 아니라 첫 캡처 시점에 OS 가 직접 요구**한다(§1, §3.1) — 이는 이전 버전 명세의 가장 큰 정정이다.
> **관련 명세**: 앱 상태 머신·메뉴바 표시는 `F-10`(범위 밖 — 권한 없음 상태를 메뉴바 `unauthorizedMenu` 로 어떻게 반영할지는 F-10 소관, 이 문서와 상호 참조), event tap 설치 자체와 그 실패가 치명적 종료로 이어지는 흐름은 `F-07`/`F-10`(범위 밖 — F-11 은 권한만 부여하고 탭 설치·종료 판단은 하지 않는다), 환경설정 창 UI는 `F-09`(범위 밖).
> **근거 문서**: `docs/research/app-bundle-analysis.md`(⭐ 1차 근거, §3.3·§4.4·§5.2) · `docs/research/superkey-inventory.md` §1.3(FAQ, 실측으로 대체되지 않은 부분만 유효) · `docs/research/rust-macos-capability-notes.md` §2.5(TCC 권한), §3.2(TCC 지속성), §3.3(tauri dev 함정)

---

## 1. 개요

### 1.1 ⭐⭐ 가장 큰 정정 — 앱이 명시적으로 확인하는 권한은 Accessibility 하나뿐이다

이전 버전은 "Accessibility · Input Monitoring · Screen Recording 3종을 온보딩에서 함께 요청한다"는 전제로 이 문서 전체를 설계했다. 실측(app-bundle-analysis.md §3.3, `otool -L`/`nm -u` 심볼 판정)은 다르다.

| 권한 | 확인/요청 심볼 | 판정 |
| :--- | :--- | :--- |
| Accessibility | **`AXIsProcessTrusted`** 있음 | ✅ 명시적으로 확인한다. 자체 모달(§3.2)이 안내하는 유일한 권한 |
| Input Monitoring | `IOHIDCheckAccess` / `IOHIDRequestAccess` **없음** | ❌ 확인하지 않는다. 대신 실패를 붙잡고 재시도한다 — 원문(SuperKey) "IOHIDManagerOpen failed with kIOReturnNotPermitted. Retrying... (Attempt " |
| Screen Recording | `CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` **없음** | ❌ 확인하지 않는다. `CGDisplayCreateImage` 호출 시 OS 가 암묵적으로 요구·프롬프트한다 |

⭐ `AXIsProcessTrustedWithOptions`(프롬프트를 동반하는 변형)도 **없다.** 즉 원본은 **macOS 표준 TCC 시스템 프롬프트를 한 번도 띄우지 않는다** — 대신 자체 모달로 사용자를 시스템 설정으로 안내한다(§3.2). 이는 §2·§3 전체의 설계 전제를 바꾼다: "프롬프트를 한 번만 띄울 수 있다"는 macOS 의 제약(rust-macos-capability-notes.md §2.5) 자체가 원본에는 애초에 적용되지 않는다 — 원본은 처음부터 끝까지 자체 UI + 시스템 설정 딥링크로만 동작한다.

⭐ **entitlement 로도 이를 판정할 수 없다.** 원본의 entitlement 는 `com.apple.security.cs.allow-jit` 단 하나이고 App Sandbox 도 선언하지 않는다(실측: `codesign -dv --entitlements`, app-bundle-analysis.md §1.1). **TCC 권한(Accessibility·Input Monitoring·Screen Recording)은 entitlement 로 선언되지 않는다** — 런타임에 시스템이 부여하는 것이며 번들 메타데이터에 흔적이 남지 않는다. 즉 "이 앱이 어떤 권한을 요구하는가"는 Info.plist 나 entitlements 조회로는 절대 판정할 수 없고, **링크된 API(위 표)로만** 판정할 수 있다. 클론을 검증할 때도 이 원칙이 그대로 적용된다.

Screen Recording 이 Seek(`F-02`) 사용 시 필요하다는 사실 자체는 바뀌지 않는다 — 바뀐 것은 **확인 시점**이다. 이전 명세는 이를 온보딩 단계의 명시적 요청으로 설계했으나, 실측은 "요청조차 하지 않고, OS 가 첫 캡처 API 호출 시점에 알아서 프롬프트한다"는 훨씬 얇은 설계를 보여준다.

### 1.2 이 문서의 핵심 명제

1. **권한 흐름의 품질이 제품 체감을 지배한다.** Accessibility 가 없으면 앱은 "아무것도 하지 않는" 것처럼 보인다. 사용자는 이것을 버그로 인식하지, 권한 문제로 인식하지 못할 가능성이 높다.
2. **권한 DB 불일치("out of sync")는 예외적 사고가 아니라 상시 발생하는 실패 모드**다. 개발자에게는 재빌드할 때마다(§7, ad-hoc 서명의 `cdhash` 변경), 일반 사용자에게는 앱 업데이트나 재설치 시 발생할 수 있다. 실측으로 이 진단·복구 흐름 전량이 확정됐다(§3.3) — 전용 복구 UI 는 부가 기능이 아니라 필수 기능이다.
3. ⭐ **event tap 생성 실패는 복구 시도의 대상일 뿐 아니라, 끝내 해소되지 않으면 치명적이다.** 원문 "Failed to create event tap. Exiting program" 이 보여주듯 앱은 종료한다. 이 부분은 `F-10`(menu-bar-and-lifecycle.md) §2 시나리오 E 와 상호 참조한다 — F-11 은 진단·복구 UI 의 내용을, F-10 은 그로 인한 앱 상태·프로세스 생명주기 전이를 각각 소유한다.

## 2. 사용자 시나리오

### S1 — 최초 실행 정상 흐름 (⭐ 정정)

1. 사용자가 앱을 처음 실행한다. Accessibility 가 미부여 상태다.
2. 앱은 `AXIsProcessTrusted()` 로 확인 후, **시스템 프롬프트가 아니라 자체 모달** `Authorize Superkey` 를 띄운다(§3.2, 실측: `01-authorize-accessibility.png`). "허용/허용 안 함" 을 고르는 macOS 표준 대화상자가 아니다 — 사용자가 직접 시스템 설정으로 이동해 체크박스를 켜야 한다.
3. 사용자가 `Open System Settings` 버튼을 누르면 시스템 설정의 손쉬운 사용 패널이 열린다. 사용자가 `Superkey.app` 항목을 체크한다.
4. Accessibility 가 부여되면 F-07 이 `CGEventTap` 설치를 시도할 수 있는 상태가 된다. Input Monitoring 은 **명시적으로 확인·요청되지 않는다** — F-07 이 `IOHIDManagerOpen` 등을 실제로 호출했을 때 실패하면 원문 "IOHIDManagerOpen failed with kIOReturnNotPermitted. Retrying... (Attempt " 로 재시도하며, 이 재시도가 반복되는 동안 macOS 는 (앱이 명시적으로 요청하지 않아도) Input Monitoring 프롬프트를 자체적으로 띄울 수 있다 — 정확한 트리거 시점은 `(미확정)` → §9.
5. 이후 사용자가 처음으로 Seek 를 트리거하고 OCR 경로가 실행되면, `CGDisplayCreateImage` 호출 시점에 Screen Recording 권한이 없으므로 **macOS 표준 프롬프트가 암묵적으로 뜬다.** 앱은 이를 직접 요청하지 않는다(§1.1) — 프롬프트를 관장하는 것은 전적으로 OS 다.
6. 사용자가 허용하면 Seek 의 OCR 경로가 활성화된다. Screen Recording 은 macOS 정책상 프롬프트 승인 직후 앱 재시작 없이 사용 가능한 경우가 많으나, 재시작이 필요한 macOS 버전도 있어 확정하지 않는다 `(추정)` → §9.

### S2 — Accessibility 거부 후 재시도

1. S1 의 3단계에서 사용자가 시스템 설정에서 체크박스를 켜지 않고 모달을 닫는다(원본에는 애초에 "허용/거부" 버튼이 있는 시스템 프롬프트가 없으므로, "거부"라는 개념은 모달을 닫거나 무시하는 것에 가깝다).
2. 앱은 폴링(§3.6 이 다루던 감시 개념 유지)으로 `AXIsProcessTrusted()` 를 계속 확인한다. `true` 가 될 때까지 모달을 다시 보여주거나 유지한다.
3. nib 에는 조건부 문구가 하나 더 있다 — (SuperKey 원문) "If the checkbox is disabled, click the padlock and enter your password". 관리형 기기나 잠긴 패널에서 체크박스가 회색 처리된 경우를 위한 안내다.
4. ⭐ nib 에 저장된 원문은 구형(`Open System Preferences` / `Go to System Preferences`)이고, 실행 중에는 `Open System Settings` / `Go to System Settings` 로 표시된다 — **OS 버전에 따라 런타임에 문구를 교체**한다(macOS 13 Ventura 에서 "시스템 환경설정"이 "시스템 설정"으로 개명된 것에 대응). 클론도 같은 대응이 필요하다.

### S3 — 런타임 중 Accessibility 취소

1. 앱이 정상 동작 중이다. Accessibility 가 부여된 상태로 F-07 의 `CGEventTap` 이 살아 있다.
2. 사용자가 시스템 설정에서 앱의 토글을 끈다.
3. 다음 폴링 주기에서 `AXIsProcessTrusted()` 가 `false` 를 반환한다.
4. 앱은 상태를 "권한 없음"으로 전환하고, 메뉴바 아이콘·메뉴 변경을 `F-10` 에 위임한다(F-10 의 `unauthorizedMenu` 전이). 이미 열려 있던 Seek 세션이 있다면 F-01 의 세션 취소 경로로 즉시 닫는다.

### S4 — ⭐ 권한 어긋남("out of sync") 진단과 복구 (실측으로 전면 재작성)

1. Accessibility 가 시스템 설정 상에는 "켜짐"으로 표시되어 있다(`AXIsProcessTrusted() == true`).
2. 그런데도 F-07 이 `CGEventTapCreate` 를 호출하면 `NULL` 이 반환된다 — 권한 DB 와 실제 커널 레벨 권한 사이의 불일치다.
3. 앱은 원문 `Superkey has insufficient privileges` / `Superkey is unable to listen to input from your devices.`(또는 `Unable to listen to device input`) 진단 화면을 띄운다. 본문은 "It appears that the accessibility settings in macOS are out of sync and Superkey might not be able to process your input. You can let Superkey try to reset privileges itself, or try to manually fix this by performing the following:" 로 이어진다.
4. 화면은 두 갈래를 제시한다.
   - **(a) 자체 리셋**: 버튼 원문 `Reset Priviliges & Restart Superkey`[sic — 원문 오타("Priviliges") 그대로다]. 누르면 "Superkey will now close and attempt to open System Preferences for you." 대로 앱이 스스로 종료하고 시스템 설정을 열려 시도한다.
   - **(b) 수동 절차**: "Try removing Superkey from the list in System Preferences" → "Then relaunch Superkey and check the box in System Preferences again when prompted." → "Accessibility, disable then remove Superkey." 순서로 안내한다.
5. 사용자가 절차를 마치고(또는 (a)의 자동 리셋이 성공하고) 앱을 다시 실행하면, 권한 DB 항목이 완전히 제거되었으므로 macOS 입장에서는 "새로 설치된 앱"과 동일하게 취급 — S1 이 처음부터 다시 시작된다.
6. 어느 경로로도 문제가 해소되지 않고 F-07 이 다시 `CGEventTapCreate` 를 시도해 또 실패하면, 원문 `com-knollsoft-Superkey Failed to create event tap. Exiting program` 로그와 함께 **프로세스가 종료된다.** 이는 F-10 의 프로세스 생명주기 관점 — F-11 은 "이 진단·복구 UI 를 보여준다"까지가 소관이고, 종료 여부·시점은 F-10 이 정의한다.
7. 권한이 완전히 없는 경우(사용자가 처음부터 Accessibility 를 부여하지 않음)는 이 진단과 별개다 — 메뉴바 메뉴 전체가 `unauthorizedMenu` 로 교체되며 항목은 (SuperKey 원문) `Not Authorized to Control Your Computer` / `Authorize` 다(F-10 소관, 실측: 번들 문자열 — 권한이 부여된 상태로 조사해 메뉴 자체를 직접 관찰하지는 못했다).

## 3. 동작 명세

### 3.1 권한 3종 — 확인 방식 요약 (⭐ 전면 재작성)

| 권한 | 무엇에 쓰나 | 앱이 명시적으로 확인/요청하는가 | 확인/요청에 준하는 실제 메커니즘 | 없으면 죽는 기능 |
| :--- | :--- | :---: | :--- | :--- |
| Accessibility | 키 이벤트 감지·클릭 실행 | ✅ `AXIsProcessTrusted()` | 자체 모달 `Authorize Superkey`(§3.2) — 시스템 프롬프트 아님 | 키 리매핑 전체(F-07), Seek 의 클릭 실행(F-04), AX 파싱 검출 경로(F-02) |
| Input Monitoring | `CGEventTap`/HID 이벤트 수신의 전제 조건으로 추정 | ❌ | `IOHIDManagerOpen` 실패 시 원문 "IOHIDManagerOpen failed with kIOReturnNotPermitted. Retrying... (Attempt " 로 붙잡고 재시도. 앱이 명시적으로 요청하지 않아도 OS 가 이 반복 시도 과정에서 자체 프롬프트를 띄울 가능성이 있으나 확정하지 못함 `(미확정)` | 키 리매핑(F-07) — 정확히 어떤 조건에서 필수인지는 `(미확정)` → §9 |
| Screen Recording | Seek 의 OCR 캡처(`CGDisplayCreateImage`) | ❌ | `CGDisplayCreateImage` 호출 시 macOS 가 표준 프롬프트를 암묵적으로 띄움. 앱은 관여하지 않는다 | Seek 의 OCR 검출 경로(F-02)만. AX 파싱 경로·클릭 실행·키 리매핑 자체는 영향 없음 |

⭐ Input Monitoring 이 실제로 `CGEventTap` 설치의 전제 조건인지는 macOS 버전에 따라 다를 수 있으나, 이 문서는 확인 API 부재라는 실측 사실만 명시하고 "필수 여부"는 `(미확정)` 으로 남긴다 — 추측으로 메우지 않는다.

### 3.2 Accessibility 온보딩 모달 실측 (실측: `01-authorize-accessibility.png` + nib)

원문(SuperKey):

```
Authorize Superkey
Superkey needs your permission to read events from your devices.
Go to System Settings → Privacy & Security → Accessibility
[Open System Settings]
Check Superkey.app
```

조건부 문구(nib, 체크박스가 잠겨 있을 때만):

```
If the checkbox is disabled, click the padlock and enter your password
```

⭐ nib 에 저장된 문구는 구형(`Open System Preferences` / `Go to System Preferences`)이고, **실행 중에는 `System Settings` 로 표시된다** — OS 버전에 따라 런타임에 문구를 교체한다는 뜻이다. 클론도 동일한 대응(OS 버전 판별 → "시스템 환경설정" 대 "시스템 설정" 어휘 교체)이 필요하다.

컨트롤러·아웃렛(실측: 번들 심볼): `AccessibilityWindowController` · `AccessibilityViewController` · `AccessibilityAuthorization` · `openSysPrefsButton` · `openSystemPrefs:` · `sysPrefsPathField` · `SystemPrefsUtil`.

이 모달은 시스템 프롬프트가 아니므로 **닫아도 다시 열 수 있고, 몇 번이든 다시 띄울 수 있다** — macOS 의 "프롬프트는 한 번만 뜬다"는 제약(`AXIsProcessTrustedWithOptions` 를 쓸 때만 해당)이 원본에는 적용되지 않는다(§1.1).

### 3.3 ⭐ 신규 — 권한 실패·복구 흐름 전량

`S4`(§2)에서 서술한 흐름을 상태·절차로 정리한다. 이 절이 기존 명세의 `(추정)` 대부분을 대체한다.

**확정되는 복구 설계:**

1. **"권한이 어긋난(out of sync) 상태"라는 별도 진단 상태가 존재한다.** `AXIsProcessTrusted() == true` 인데 `CGEventTapCreate` 가 실패하는 조합으로 감지되는 것으로 추정한다(§2 S4 항목 2) — 이 판정 조건 자체가 원문에 프로그래밍적으로 명시된 것은 아니고, 증상("even when the app has the necessary Accessibility settings enabled" 류 서술)으로부터의 합리적 추정이다 `(미확정 — 판정 조건의 정확한 구현)`.
2. 그 진단 화면은 두 갈래를 제시한다 — **(a) 앱이 스스로 권한을 리셋하고 재시작**(`Reset Priviliges & Restart Superkey`[sic]), **(b) 수동 절차**(목록에서 제거 → 재실행 → 재승인).
3. **event tap 생성 실패는 궁극적으로 치명적이다** — 재시도를 무한히 계속하는 것이 아니라, 실패가 해소되지 않으면 프로세스가 종료된다(`Failed to create event tap. Exiting program`). 종료의 정확한 트리거 조건(몇 번 재시도 후 포기하는지)은 `(미확정)` → §9.
4. 권한이 없으면(위 진단이 아니라 애초에 미부여) 메뉴바 메뉴가 `unauthorizedMenu` 로 교체된다(F-10 §3.1·§3.3 참조).

**⭐ "(a) 스스로 리셋"의 구체적 수단은 `(미확정)` 이다.** `tccutil reset` 계열 셸 호출로 보이는 정황은 있다 — 번들 안에 셸 실행 경로가 있고(`Tools_Tools.bundle` 의 원문 "bash command error: %@"), `hidutil` 을 셸로 호출하는 사례가 이미 다른 곳(caps lock 리매핑)에서 확인됐다(app-bundle-analysis.md §3.2 항목 3). 그러나 이 리셋 버튼이 실제로 `tccutil reset Accessibility com.knollsoft.Superkey` 류 명령을 실행하는지는 **직접 확인하지 못했다.**

⭐ **클론이 이를 재현할지에 대한 판단**: 재현하지 않는 쪽을 권장한다. 근거:

- `tccutil reset` 은 **해당 서비스(Accessibility)에 대해 그 앱 하나만이 아니라, 경우에 따라 서비스 전체 또는 광범위한 항목을 초기화할 수 있는 명령**이다(정확한 스코프는 `tccutil` 인자에 따라 다르며, 잘못 쓰면 사용자가 다른 앱에 부여했던 권한까지 건드릴 위험이 있다).
- 사용자의 명시적 승인 없이(또는 "리셋" 버튼 클릭만으로) 시스템 보안 데이터베이스를 앱이 스스로 조작하는 것은, 이 앱이 이미 요구하는 권한(Accessibility)보다 더 강한 신뢰를 전제로 한다.
- 원본이 이 기능을 가진 이유(권한 DB 불일치가 실제로 상시 발생하는 문제, §1.2)는 이해할 수 있으나, 클론은 **(b) 수동 절차 안내를 기본으로 하고, 자동 리셋은 넣더라도 사용자에게 정확히 무엇이 지워지는지 고지하는 확인 대화상자를 추가로 거치는 것이 안전하다.** 이는 원본과의 의도적 차이다 — §7·§9 로 승계.

### 3.4 권한 상태 실시간 감시

원본이 폴링을 쓰는지 알림 구독을 쓰는지는 이번 실측 범위(번들 심볼·문자열, AX 트리)에서 직접 확인되지 않았다. `AXIsProcessTrusted()` 는 동기 조회형 API 이고, 이 값의 변경을 알려주는 Darwin 알림·KVO·콜백이 시스템에 존재한다는 근거가 없다(rust-macos-capability-notes.md §2.5). 따라서 **클론의 설계로서 폴링을 유지한다**(기존 판단 유지) — 온보딩 진행 중에는 짧은 주기, 온보딩 이후 백그라운드 감시 중에는 긴 주기로 이원화한다. 정확한 주기 값은 조사 근거가 없어 설계 판단이다 `(추정)` → §9.

보조 트리거로 **앱이 포그라운드로 돌아올 때**와 **F-07 이 탭 설치·재활성화를 시도하기 직전**에 즉시 1회 확인을 추가한다(기존 판단 유지).

### 3.5 권한 조합별 기능 가용성 (⭐ 갱신 — 확인 가능한 축이 실제로는 하나뿐임을 반영)

Accessibility 만 앱이 직접 확인할 수 있는 값이고, Input Monitoring·Screen Recording 은 "확인"이 아니라 "실패의 관찰"로만 상태를 유추할 수 있다는 점에 유의해서 읽는다.

| Accessibility(확인 가능) | Input Monitoring(추론) | Screen Recording(추론) | 결과 |
| :---: | :---: | :---: | :--- |
| ✅ | ✅(재시도 성공) | ✅(캡처 성공) | 전체 기능 정상. Seek 는 OCR + AX 파싱 병합 검출(F-02) |
| ✅ | ✅ | ❌(캡처 실패 또는 미승인) | 키 리매핑·Hyperkey·Presets 정상. Seek 는 AX 파싱 경로만 동작(`Seek using macOS accessibility` 옵션이 켜져 있는 경우) |
| ✅ | ❌(`IOHIDManagerOpen` 계속 실패) | — | F-07 이 이벤트를 받지 못함 → 키 리매핑·Seek 트리거 전부 무력화 가능성. 정확한 영향 범위는 §3.1 각주와 같이 `(미확정)` |
| ❌ | — | — | 클릭 실행(F-04) 자체가 불가능 — 사실상 앱 전체가 무력화. `unauthorizedMenu`(F-10)로 전이 |
| ✅였다가 진단(out of sync) | — | — | §3.3 의 진단·복구 흐름으로 전이. F-10 과 연동해 최종적으로 종료될 수 있음 |

## 4. 설정 항목

⭐ 기존 §4 의 "`General` 탭에 권한 상태 표시가 있을 가능성이 높다"는 `(추정)`은 **틀렸다.** 실측 결과 `General` 탭에 권한 관련 항목이 **없다**(app-bundle-analysis.md §6.4). 확인된 `General` 탭 전체 항목(순서대로)은:

| 라벨 원문 | 컨트롤 |
| :--- | :--- |
| `Launch on login` | 체크박스 |
| `v1.66 (66)` | 버튼 |
| `Check for updates automatically` | 체크박스 |
| `Hide menu bar icon` | 체크박스 |
| `Menu bar icon` | 팝업 |
| `Remove Oldest Activation` | 버튼 |
| `Purchase` | 버튼 |

권한 상태 표시·딥링크 버튼은 이 목록 어디에도 없다. **"확인 결과 부재"로 명시 기록한다** — 조용히 삭제하지 않는다.

**클론이 그럼에도 권한 상태 표시를 두는 것이 나은가에 대한 판단**: 두는 쪽을 권장한다. 근거:

- 원본은 대신 §3.2·§3.3 의 전용 모달·진단 화면으로 권한 문제를 상시 알리는 구조를 갖고 있다 — "General 탭에 상태 배지가 없다"는 것이 "권한 상태를 어디에도 보여주지 않는다"는 뜻은 아니다. 원본 나름의 방식(모달 우선)으로 이미 그 역할을 하고 있다.
- 그럼에도 `General` 탭(또는 그에 준하는 위치)에 최소한의 읽기 전용 상태 표시(Accessibility 부여 여부)를 두는 것은, 사용자가 문제가 생겼을 때 진단 모달이 다시 뜨기를 기다리지 않고도 스스로 확인할 수 있게 해 UX 를 개선한다.
- **이는 원본과의 의도적 차이다.** 원본을 그대로 복제할 필요는 없으며, 이 판단은 `F-09`(환경설정 창 UI)로 승계해 실제 배치·디자인을 결정하게 한다.

## 5. 엣지 케이스와 실패 모드

1. **자체 모달은 macOS 의 "프롬프트 한 번" 제약을 받지 않는다.** `AXIsProcessTrustedWithOptions` 를 쓰지 않으므로, 원본은 몇 번이든 자체 모달을 다시 띄울 수 있다(§1.1, §3.2). 클론이 `AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt: true])` 를 쓰기로 한다면(§7 의 설계 선택), 이 제약이 다시 적용되므로 두 번째 호출부터는 프롬프트 대신 자체 안내로 전환하는 로직이 필요하다 — 원본과 클론의 구현 선택에 따라 이 항목의 적용 여부가 달라진다.
2. **Input Monitoring 재시도 루프가 무한할 수 있다.** 원문 "Retrying... (Attempt " 는 시도 횟수를 로그에 남기지만, 최대 횟수·중단 조건은 확인되지 않았다 `(미확정)` → §9. 클론은 상한을 두고, 상한 도달 시 사용자에게 명시적으로 알리는 편이 안전하다 — 원본을 그대로 흉내 내 무한 재시도로 두는 것은 권장하지 않는다(디버깅 어려움·배터리 소모).
3. **Screen Recording 요청 실패/거부 시 Seek 는 죽지 않는다.** `CGDisplayCreateImage` 실패는 OCR 경로만 막고, AX 파싱 경로·클릭 실행·키 리매핑에는 영향이 없다(§3.5). Seek 오버레이 UI(F-03) 쪽에서 이 상태를 시각적으로 표시할 필요가 있다는 시사점을 남긴다(F-03/F-09 승계 사항).
4. **앱을 옮기면 TCC 경로가 바뀐다.** TCC 는 번들 ID + `csreq` 로 권한을 키잉하지만(rust-macos-capability-notes.md §3.2), 실행 파일 경로도 함께 기록하는 것으로 알려져 있어 `.app` 을 다른 경로로 옮기거나 이름을 바꾸면 권한 재확인을 요구할 수 있다 `(추정)`.
5. **재빌드로 권한이 소실된다(개발 환경).** ad-hoc 서명은 빌드마다 `cdhash` 가 바뀌어 TCC 가 매번 "새 앱"으로 취급한다(§7).
6. **`tauri dev` 에서는 권한이 터미널(부모 프로세스)에 귀속된다.** `.app` 번들이 아니라 `target/debug/<binary>` 를 직접 실행하므로 TCC 가 부모 프로세스의 권한으로 판정한다(rust-macos-capability-notes.md §3.3).
7. **MDM 으로 권한이 강제된 기업 환경.** PPPC 프로파일로 사전 승인되거나 조직 정책으로 영구 차단될 수 있다. 클론이 `AXIsProcessTrustedWithOptions` 를 쓴다면 프롬프트 자체가 뜨지 않을 수 있고, 원본처럼 자체 모달만 쓴다면 시스템 설정의 회색 처리된 토글을 사용자가 조작할 수 없는 상태로 남는다 — "관리자에게 문의하세요" 류 분기가 필요하다 `(추정)`.
8. **사용자 전환(Fast User Switching).** TCC 는 사용자별로 별도의 DB(`~/Library/Application Support/com.apple.TCC/TCC.db`)를 갖는다. 계정을 전환하면 그 계정에서는 처음부터 다시 온보딩을 거쳐야 한다.
9. **자체 리셋(§3.3)의 위험.** 클론이 만약 자동 리셋 기능을 넣기로 한다면, 원본과 달리 반드시 "무엇이 지워지는지" 사용자에게 고지하고 확인을 받아야 한다(§3.3의 권고).
10. **권한 모달이 다른 창 뒤에 뜬다.** 앱이 Dock 아이콘 없는 Accessory 앱(F-10)으로 동작할 때, 자체 모달이라도 다른 앱의 전체화면 창 뒤에 가려질 수 있다. `⌘Tab` 으로는 애초에 앱 자체가 노출되지 않으므로(F-10), 메뉴바 아이콘 클릭으로 모달을 다시 불러올 수 있는 경로가 항상 있어야 한다.

## 6. 필요한 플랫폼 API

- **Accessibility**: `AXIsProcessTrusted()`(확인 — 원본이 실제로 쓰는 유일한 명시적 API). `AXIsProcessTrustedWithOptions()` 는 **원본에 없다**(§1.1) — 클론이 이를 채택할지는 §7 의 설계 선택. rust-macos-capability-notes.md §2.5. Info.plist 에 별도 사용 설명 키 없음(usage description 키 자체가 전무 — app-bundle-analysis.md §1).
- **Screen Recording**: 원본은 `CGPreflightScreenCaptureAccess`/`CGRequestScreenCaptureAccess` 를 **쓰지 않는다**(실측: 링크 심볼 부재) — `CGDisplayCreateImage`/`CGDisplayCreateImageForRect` 호출 시 OS 가 암묵적으로 프롬프트한다(app-bundle-analysis.md §3.1). 클론이 명시적 확인/요청 API 를 쓸지는 §7 의 설계 선택.
- **Input Monitoring**: 원본은 `IOHIDCheckAccess`/`IOHIDRequestAccess` 를 **쓰지 않는다** — `IOHIDManagerOpen` 등을 직접 호출해 `kIOReturnNotPermitted` 실패를 붙잡아 재시도한다(§3.1). 이 두 확인/요청 함수는 macOS SDK 에는 존재하지만 원본이 링크하지 않았을 뿐이다 — 클론이 쓰기로 하면 전용 Rust 크레이트가 없어 `#[link(name="IOKit", kind="framework")]` 로 직접 링크하고 `extern "C"` 로 선언해야 한다(rust-macos-capability-notes.md §2.5).
- **`CGEventTapCreate` (판정용, 설치 자체는 F-07 소관)**: F-11 은 §3.3 의 out-of-sync 판정을 위해 F-07 이 노출하는 "탭 생성 성공/실패" 신호를 구독한다.
- **시스템 설정 딥링크**: `x-apple.systempreferences:` URL 스킴. 원본의 자체 모달(§3.2)은 버튼(`Open System Settings`)으로 이를 호출하는 것으로 보이나, 정확한 URL(앵커 이름)은 이번 실측(nib 텍스트·AX 트리) 범위에서 문자열 자체로는 확인되지 않았다 `(미확정)` → §9. Accessibility 앵커(`Privacy_Accessibility`)는 이전 조사(rust-macos-capability-notes.md §2.5)에서 원문으로 확인된 값을 유지한다.
- **재시작 트리거**(§3.3 (a) 경로): 자체 리셋이 실제로 앱 재시작까지 수행한다는 것이 원문("Superkey will now close and attempt to open System Preferences for you.")으로 확인됐다. 정확한 구현(AppleScript, `NSWorkspace.launchApplication`, 또는 `AppRelauncher`(F-10 §3.5) 재사용)은 `(미확정)`.

## 7. 구현 접근

**판정: Rust 바인딩.** (기존 판정 유지)

- **Accessibility**: `axuielement` 0.9.1 이 `ProcessTrust` 래퍼를 제공한다(rust-macos-capability-notes.md §1.2). 더 얇은 대안으로 `macos-accessibility-client` 0.0.2 도 있다.
- **⭐ 설계 결정 — Input Monitoring·Screen Recording 은 원본과 다르게, 플랫폼이 제공하는 명시적 확인/요청 API 를 쓴다.** 원본은 이 둘을 확인하지 않고 실패-재시도(Input Monitoring)·OS 암묵 프롬프트(Screen Recording)에만 의존한다(§3.1). 클론은 대신:
  - Screen Recording: `core-graphics` 0.25.0 의 `CGPreflightScreenCaptureAccess`/`CGRequestScreenCaptureAccess` 안전 래퍼를 그대로 쓴다(rust-macos-capability-notes.md §1.2). 이렇게 하면 Seek 최초 사용 시 "화면 기록 권한이 필요합니다" 라는 명확한 인라인 안내를 미리 보여줄 수 있어, 원본처럼 OS 프롬프트가 갑자기 뜨는 것보다 사용자 경험이 낫다.
  - Input Monitoring: 전용 크레이트가 없으므로 IOKit 을 직접 링크하고 `IOHIDCheckAccess`/`IOHIDRequestAccess` 를 `extern "C"` 로 선언해 쓴다. 원본의 "실패 후 재시도" 패턴보다 사전에 상태를 알 수 있어 더 견고하다.
  - ⭐ **이는 원본과의 의도적 차이다.** 원본이 이 API 들을 쓰지 않는 이유(어쩌면 앱이 대상으로 하는 최소 macOS 12 에서 이 API 들의 가용성·신뢰성 문제, 혹은 단순 설계 누락)는 조사로 확인되지 않았다 `(미확정)` → §9. 더 견고한 명시적 API 가 플랫폼에 존재하는데도 원본이 안 쓴다는 사실 자체가 클론이 이를 채택하지 말아야 할 이유는 아니라고 판단했다.
- **딥링크·재시작 트리거**: Tauri 의 shell-open API 또는 표준 라이브러리 `Command` 로 `open` / `osascript` 를 호출하는 것으로 충분하다.
- **자체 TCC 리셋(§3.3의 (a) 경로)**: **채택하지 않는 것을 권장한다**(§3.3, §5 항목 9). 채택한다면 `tccutil` 셸 호출이 유력한 구현이나, 스코프가 넓어 위험하다 — 정확한 인자·확인 대화상자 설계는 별도 안전성 검토가 필요하다.
- **결론적으로 F-11 전체에 네이티브 Swift/Objective-C shim 은 불필요하다.** Input Monitoring 의 수동 FFI 선언이 유일한 특이점이며, 여전히 "Rust 바인딩" 판정 범주 안에 있다.

⭐ **M1 이 실제로 구현한 범위 (2026-08-30, 이슈 #5).** 위 "명시적 확인 API 를 쓴다"는 설계 결정은 **유효하되, M1 에서는 적용 대상이 아직 없다.**

| 권한 | M1 에서의 상태 | 이유 |
| :--- | :--- | :--- |
| Accessibility | ✅ **구현됨** — `AXIsProcessTrusted()` 폴링(온보딩 500 ms / 배경 5000 ms), 자체 모달, 시스템 설정 딥링크, 런타임 취소 감지 | M1 의 유일한 필수 권한. `CGEventTapCreate` 의 전제 조건 |
| Screen Recording | ❌ **구현 안 함** | M1 에 Seek(F-02)가 없어 `CGDisplayCreateImage` 를 **호출하는 코드 자체가 없다.** 확인할 것이 없으므로 확인하지 않는다. 위 설계 결정(`CGPreflightScreenCaptureAccess` 채택)은 **M3 에서 F-02 와 함께** 적용한다 |
| Input Monitoring | ❌ **구현 안 함** | ⭐ M1 은 `IOHIDManagerOpen` 을 **호출하지 않는다.** 키보드 핫플러그 감지를 `IOHIDManager` 계열이 아니라 `IOServiceAddMatchingNotification` 으로 구현했기 때문이다(`key-remapping-engine.md` §6 판정 변경) — 그 결정 자체가 "이 권한을 요구하지 않기 위한" 것이었다. 따라서 M1 에서는 확인할 것도, 실패-재시도로 흡수할 것도 없다. 위 설계 결정(`IOHIDCheckAccess` 채택)은 **경로 B 의 FFI 구현이 들어오는 시점(M2 이후)** 에 재검토한다 |

즉 **M1 의 관측 가능한 동작은 "확인하는 권한은 `AXIsProcessTrusted` 하나"** 이며, 이는 §1.1 의 실측(원본도 그렇다)과 일치한다. 원본과의 의도적 차이(§7 의 명시적 API 채택)는 **없어진 것이 아니라 아직 적용 지점이 오지 않은 것**이다 — 조용히 지우지 않고 여기에 기록해 둔다.

⚠️ **자체 TCC 리셋은 구현하지 않았다.** §3.3·§5 항목 9·§7 이 "채택하지 않는 것을 권장"으로 판정한 그대로다. M1 의 out-of-sync 진단 화면은 **(b) 수동 절차 안내만** 제공한다. §8 의 "(a) 자체 리셋 경로를 클론이 구현한다면…" 수용 기준은 **구현하지 않음으로써 충족**된다.

### 개발 워크플로 요구사항 ⭐ (기존 내용 유지 — 실측으로 보강)

rust-macos-capability-notes.md §3.2·§3.3 이 확정한 사실에, app-bundle-analysis.md §1.1 이 확인한 사실(entitlement 는 `com.apple.security.cs.allow-jit` 하나뿐, App Sandbox 없음, Hardened Runtime 활성)을 더해 개발 워크플로 요구사항을 명시한다.

1. **TCC 는 번들 ID + `csreq` 로 권한을 키잉한다.** ad-hoc 서명은 `cdhash` 가 매 빌드 바뀌어 권한이 소실된다.
2. **공증(notarization)은 TCC 와 무관하다.** Gatekeeper 요건이며 로컬 개발 권한과 무관하다.
3. **개발 중 우회책**: 고정된 자체 서명 인증서로 모든 개발 빌드를 서명해 `csreq` 를 안정시킨다.
4. **`tauri dev` 는 이 워크플로에서 배제한다.** 권한이 필요한 기능은 반드시 `tauri build` 산출 `.app` 을 위 인증서로 서명한 뒤 테스트한다.
5. ⭐ **Hardened Runtime 은 활성 상태여야 한다** — 원본이 그렇게 배포되고 있음이 실측으로 확인됐다(`flags=0x10000(runtime)`). 클론도 동일하게 Hardened Runtime 을 켜고 배포해야 TCC·공증 요건을 함께 만족한다.

## 8. 수용 기준

- [ ] 앱 최초 실행 시 Accessibility 미부여 상태면 `Authorize Superkey` 류의 자체 안내(모달 또는 온보딩 화면)가 표시되고, 시스템 설정으로 이동하는 버튼이 있다.
- [ ] 시스템 설정에서 사용자가 Accessibility 토글을 켜면, 앱으로 돌아왔을 때 별도의 "확인" 버튼 클릭 없이 폴링만으로 자동 진행된다.
- [ ] Input Monitoring·Screen Recording 은 온보딩 단계에서 함께 요청되지 않는다 — Screen Recording 은 Seek 최초 트리거 시점(또는 클론이 명시적 API 를 채택했다면 그 호출 시점)에만 요청/프롬프트된다.
- [ ] Accessibility 가 부여된 상태에서 앱을 재실행하면 온보딩 화면이 다시 나타나지 않고 즉시 일반 사용 화면으로 진입한다.
- [ ] 런타임 중 시스템 설정에서 Accessibility 를 끄면, 다음 폴링 주기 이내에 앱이 이를 감지하고 F-10 의 `unauthorizedMenu` 전이가 트리거된다.
- [ ] Screen Recording 만 거부된 상태에서 키 리매핑과 Seek 의 AX 파싱 경로는 정상 동작하고, OCR 경로만 비활성화된다.
- [ ] `AXIsProcessTrusted() == true` 인데 F-07 의 탭 생성이 실패하는 상황이 재현되면, out-of-sync 진단 화면(§3.3)이 표시되고 (a)/(b) 두 경로가 모두 제공된다.
- [ ] 진단 화면의 (a) 자체 리셋 경로를 클론이 구현한다면, 무엇이 지워지는지 사용자에게 고지하는 확인 단계를 원본에 없더라도 추가한다.
- [ ] 진단이 해소되지 않고 event tap 생성이 계속 실패하면, 앱은 (F-10 이 정의하는 절차에 따라) 로그를 남기고 정상적으로 종료한다 — 무한정 조용히 죽어있는 상태로 남지 않는다.
- [ ] `General` 탭에는 권한 관련 항목이 없거나(원본 그대로 따를 경우), 있다면 그것이 원본과의 의도적 차이임이 설계 문서에 명시되어 있다(§4).
- [ ] 개발 빌드를 고정된 자체 서명 인증서로 서명한 뒤 연속으로 재빌드해도 이미 부여된 권한이 유지된다.
- [ ] `tauri dev` 로 실행한 프로세스에서는 권한 요청 시도 시 이것이 `tauri build` 산출물이 아니라는 경고가 표시된다.
- [ ] OS 버전(시스템 환경설정 vs 시스템 설정)에 따라 안내 문구가 올바르게 교체된다(§3.2).

## 9. 미해결 질문

이번 실측으로 **해소된 항목**: 권한 종류(3종 명시 확인 → Accessibility 1종만 명시 확인, 나머지는 암묵/추론) · Accessibility 온보딩 모달의 정확한 문구 · out-of-sync 진단·복구 흐름의 존재와 대체적 절차 · `General` 탭에 권한 상태 표시가 없다는 사실.

남은 미해결 질문:

| # | 질문 | 근거 문서 연결 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | "(a) 스스로 리셋"이 실제로 `tccutil reset` 을 호출하는지, 호출한다면 정확한 인자(스코프) | §3.3, app-bundle-analysis.md §4.4 | 리셋 버튼 클릭 전후 `sqlite3 ~/Library/Application\ Support/com.apple.TCC/TCC.db` 스냅샷 비교(위험 — 실기 재현 시 다른 앱 권한에 영향 없는지 사전 백업 필요) |
| 2 | Input Monitoring 이 이 앱이 대상으로 하는 macOS 버전에서 `CGEventTap` 설치에 실제로 필수인지 | §3.1, §3.5 | Input Monitoring 을 거부한 채 `CGEventTapCreate` 를 호출하는 실기 테스트 |
| 3 | `unauthorizedMenu` 의 실제 겉모습(F-10 §9 항목 1 과 동일 질문) | app-bundle-analysis.md §4.4 — 권한이 있는 상태라 직접 관찰 못함 | Accessibility 권한을 실제로 회수한 뒤 재관찰 |
| 4 | Input Monitoring 재시도 루프의 최대 횟수·중단 조건 | §5 항목 2 | 재시도 로그(`Attempt N`)의 상한을 실기에서 관찰 |
| 5 | out-of-sync 판정 조건(`AXIsProcessTrusted()==true && CGEventTapCreate()==NULL`)이 원본 내부 구현과 정확히 일치하는지 | §3.3 | 원본 개발자 문의, 또는 재빌드 반복으로 불일치 상태를 재현해 관찰 |
| 6 | Screen Recording 권한을 프롬프트에서 승인한 직후, 앱 재시작 없이 같은 세션에서 바로 사용 가능한지(macOS 버전별 차이) | §2 S1 | 각 macOS 버전에서 승인 직후 곧바로 캡처 API 호출 테스트 |
| 7 | Screen Recording·Input Monitoring 시스템 설정 딥링크의 정확한 앵커(`Privacy_ScreenCapture`/`Privacy_ListenEvent`)가 원본이 실제로 쓰는 값과 같은지 | §6 | 실기에서 URL 을 직접 열어 올바른 패널로 이동하는지 확인 |
| 8 | 폴링 주기의 구체적 값 | §3.4 | 배터리·CPU 부담과 반응 속도를 함께 측정하는 실기 벤치마크 |
| 9 | 원본이 명시적 확인 API(`IOHIDCheckAccess` 등)를 쓰지 않은 이유(설계 누락인지 의도인지) | §7 | 원본 개발자 문의 |
| 10 | MDM(PPPC 프로파일) 환경에서 자체 모달이 어떻게 동작하는지 | §5 항목 7 | 기업 MDM 테스트 환경에서 실기 검증(이번 조사 범위 밖) |
