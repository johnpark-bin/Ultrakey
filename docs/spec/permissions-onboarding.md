# F-11 · 권한 온보딩과 복구

> **한 줄 요약**: Accessibility·Screen Recording·Input Monitoring 3종 TCC 권한의 확인·요청·거부 후 복구·런타임 감시 흐름과, 권한 DB 불일치로 발생하는 `"Unable to initialize Superkey"` 상태의 감지·복구 UI, 그리고 개발 환경에서 권한을 안정적으로 유지하기 위한 코드 서명 워크플로를 정의한다.
> **의존성**: 없음(최상위 전제 조건) — 오히려 `F-07`(key-remapping-engine)이 `CGEventTap` 을 설치하려면 F-11 이 부여한 Accessibility·Input Monitoring 이 이미 있어야 하고, `F-01`~`F-04`(Seek 계열)가 Screen Recording 이 필요한 OCR 경로를 쓰려면 F-11 이 부여한 Screen Recording 이 있어야 한다. F-11 은 이 다른 명세들의 **선행 조건**이다.
> **관련 명세**: 앱 상태 머신·메뉴바 표시는 `F-10`(범위 밖 — 권한 상태를 메뉴바 아이콘에 어떻게 반영할지는 F-10 소관), event tap 설치 자체는 `F-07`(범위 밖 — F-11 은 권한만 부여하고 탭 설치는 하지 않는다), 환경설정 창 UI는 `F-09`(범위 밖 — F-11 은 §4 에서 권한 관련 설정 항목의 존재 여부만 명시한다).
> **근거 문서**: `docs/research/superkey-inventory.md` §1.3(FAQ), §6(횡단 관찰), §3.4/§7(General 탭) · `docs/research/rust-macos-capability-notes.md` §2.5(TCC 권한), §3.2(TCC 지속성), §3.3(tauri dev 함정)

---

## 1. 개요

이 앱은 macOS 의 세 가지 TCC(Transparency, Consent, and Control) 권한 없이는 핵심 기능이 전혀 동작하지 않는다. superkey-inventory.md §1.3 이 원문으로 확정한 근거는 다음 두 문장이다.

- "Accessibility permissions are **necessary to know when configured keystrokes are pressed, and to execute clicks**."
- "Screen recording permissions are **only used for Seek**, where the app needs to take a screenshot to find the text on your screen."

세 번째 권한인 Input Monitoring 은 랜딩 FAQ 본문에는 등장하지 않고, `"Unable to initialize Superkey"` 복구 절차 안에서만 드러난다 — "Also disable then remove it from System Preferences -> Privacy & Security -> **Input Monitoring** if it's there, too." (superkey-inventory.md §1.3). 즉 원본 앱은 Input Monitoring 을 사용자에게 명시적으로 설명한 적이 없고, 오직 문제가 생겼을 때의 복구 문서에서만 존재가 드러난다. 이는 이 권한이 온보딩 흐름에서 **조용히** 요청된다는 것을 시사한다 — 사용자가 "왜 이 권한이 필요한가"를 알아야 할 필요 없이, `CGEventTap` 설치의 전제 조건으로만 소비된다.

⭐ 이 문서의 핵심 명제는 두 가지다.

1. **권한 흐름의 품질이 제품 체감을 지배한다.** 세 권한 중 하나라도 없으면 앱은 "아무것도 하지 않는" 것처럼 보인다. 사용자는 이것을 버그로 인식하지, 권한 문제로 인식하지 못할 가능성이 높다. 따라서 온보딩은 권한을 요청하는 것 자체보다, 왜 필요한지·거부 시 무엇이 죽는지·어떻게 되돌리는지를 끊임없이 사용자에게 알려주는 것이 목표다.
2. **권한 DB 불일치(`"Unable to initialize Superkey"`)는 예외적 사고가 아니라 상시 발생하는 실패 모드**다(superkey-inventory.md §6 관찰 5). 특히 이 실패 모드는 개발자에게는 재빌드할 때마다(§3.2, ad-hoc 서명의 `cdhash` 변경), 일반 사용자에게는 앱 업데이트나 재설치 시 발생할 수 있다. 전용 복구 UI 를 갖추는 것이 부가 기능이 아니라 필수 기능이다.

## 2. 사용자 시나리오

### S1 — 최초 실행 정상 흐름

1. 사용자가 앱을 처음 실행한다. 세 권한 모두 미부여 상태다.
2. 앱은 §3.2 에서 정한 순서로 Accessibility → Input Monitoring 을 먼저 요청한다(핵심 리매핑 엔진의 전제 조건이므로). Screen Recording 은 아직 요청하지 않는다.
3. 시스템이 표준 TCC 프롬프트를 띄운다. 사용자가 "허용"을 누르고 시스템 설정 창에서 앱 항목을 체크한다.
4. 두 권한이 모두 부여되면 F-07 이 `CGEventTap` 설치를 시도할 수 있는 상태가 된다. 앱은 온보딩 화면에서 "핵심 기능 준비 완료" 를 표시하고 온보딩을 넘어간다.
5. 이후 사용자가 처음으로 Seek 를 트리거한다(`Toggle Seek with shortcut:` 또는 `Remap key to Seek:`). 이 시점에 Screen Recording 권한이 없으므로 온보딩 스타일의 안내(§3.4)와 함께 `CGRequestScreenCaptureAccess()` 를 호출한다.
6. 사용자가 허용하면 Seek 의 OCR 경로가 즉시 활성화되고 현재 세션에서 바로 사용 가능해진다 — Screen Recording 은 macOS 정책상 프롬프트 승인 직후 앱 재시작 없이 사용 가능한 경우가 많으나, 재시작이 필요한 macOS 버전도 있어 확정하지 않는다 `(추정)` → §9.

### S2 — 거부 후 재시도

1. S1 의 3단계에서 사용자가 Accessibility 프롬프트를 "허용 안 함"으로 거부한다.
2. macOS 는 이후 같은 세션에서 `AXIsProcessTrustedWithOptions` 를 다시 호출해도 **프롬프트를 다시 띄우지 않는다**(rust-macos-capability-notes.md §2.5: "권한 요청은 프롬프트를 한 번만 띄운다"). 앱이 이를 감지하지 못하고 반복 요청만 하면 사용자에게는 아무 일도 일어나지 않는 것처럼 보인다.
3. 앱은 `AXIsProcessTrusted()` 폴링(§3.5)으로 거부 상태를 감지하고, 온보딩 화면을 "시스템 설정에서 직접 켜야 합니다" 안내로 전환한다.
4. 안내 화면은 딥링크 버튼(`x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`)과 "설정 열기" 를 제공한다. 사용자가 클릭하면 시스템 설정의 손쉬운 사용 패널이 바로 열린다.
5. 사용자가 목록에서 앱을 찾아 토글을 켠다. 앱은 폴링으로 이 변경을 감지하고 자동으로 다음 단계(Input Monitoring)로 진행한다 — 사용자가 앱으로 돌아와 "확인" 버튼을 누를 필요가 없다.

### S3 — 런타임 중 권한 취소

1. 앱이 정상 동작 중이다. 세 권한 모두 부여된 상태로 F-07 의 `CGEventTap` 이 살아 있다.
2. 사용자가 시스템 설정 → 개인정보 보호 및 보안 → 손쉬운 사용에서 앱의 토글을 끈다(의도적이든, 다른 유틸리티 정리 중 실수든).
3. 이미 생성된 `CGEventTap` 은 즉시 죽지 않을 수 있으나(macOS 는 기존 탭을 언제 무효화하는지 문서화된 보장이 약함), 다음 폴링 주기(§3.5)에서 `AXIsProcessTrusted()` 가 `false` 를 반환한다.
4. 앱은 상태를 "권한 취소됨"으로 전환하고, 메뉴바 아이콘 상태 변경을 F-10 에 위임하며, 사용자가 다음에 앱을 조작할 때(메뉴바 클릭 등) 권한 재요청 안내를 노출한다. 이미 열려 있던 Seek 세션이 있다면 F-01 의 세션 취소 경로로 즉시 닫는다(F-01 §5 항목 9 참조).
5. 사용자가 재요청 안내에서 "설정 열기" 를 눌러 다시 토글을 켜면, 이번에는 목록에서 **제거되지 않고 토글만 꺼졌다가 켜진** 경우이므로 대부분 정상 재활성화되지만, 일부 macOS 버전에서는 이 경로도 `"Unable to initialize"` 로 이어질 수 있다 `(추정)` → S4.

### S4 — `Unable to initialize Superkey` 복구

1. 앱을 실행한다. Accessibility 권한이 시스템 설정 상에는 "켜짐"으로 표시되어 있다.
2. 그런데도 F-07 이 `CGEventTapCreate` 를 호출하면 `NULL` 이 반환된다 — 권한 DB 와 실제 커널 레벨 권한 사이의 불일치다.
3. 앱은 §3.7 의 판정 조건으로 이 불일치를 감지하고, 표준 온보딩 화면이 아니라 전용 오류 화면 `"Unable to initialize Superkey"` 를 띄운다(문구는 superkey-inventory.md §1.3 원문 유지).
4. 화면은 원문 그대로의 5단계 절차를 순서대로, 각 단계마다 실행 가능한 액션과 함께 안내한다(§3.7).
5. 사용자가 절차를 끝까지 따르고 Mac 을 재시작한 뒤 앱을 다시 실행하면, 최초 실행과 동일한 온보딩 흐름(S1)이 다시 시작된다 — 권한 DB 항목이 완전히 제거되었으므로 macOS 입장에서는 "새로 설치된 앱" 과 동일하게 취급한다.

## 3. 동작 명세

### 3.1 권한 3종 요약

| 권한 | 무엇에 쓰나 (조사 원문) | 확인 API | 요청 API | 없으면 죽는 기능 |
| :--- | :--- | :--- | :--- | :--- |
| Accessibility | "necessary to know when configured keystrokes are pressed, and to execute clicks" | `AXIsProcessTrusted()` | `AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt: true])` | 키 리매핑 전체(F-07), Seek 의 클릭 실행(F-04), Seek 의 AX 파싱 검출 경로(F-02) |
| Screen Recording | "only used for Seek, where the app needs to take a screenshot to find the text on your screen" | `CGPreflightScreenCaptureAccess()` | `CGRequestScreenCaptureAccess()` | Seek 의 OCR 검출 경로(F-02)만. AX 파싱 경로와 클릭 실행, 키 리매핑 자체는 영향 없음(§3.6) |
| Input Monitoring | 랜딩 FAQ 본문에는 없음. 복구 절차에서만 등장 — "disable then remove it from ... Input Monitoring if it's there, too" | `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)` | `IOHIDRequestAccess(kIOHIDRequestTypeListenEvent)` | 키 리매핑 전체(F-07 의 `CGEventTap` 설치 전제조건 중 하나) `(추정 — 아래 설명 참조)` |

Input Monitoring 이 `CGEventTap` 설치에 실제로 필수인지는 macOS 버전에 따라 달라질 수 있다는 보고가 있으나(전역 이벤트 탭이 Accessibility 만으로 동작하던 시기가 있었음), 원본 앱의 복구 절차가 Input Monitoring 을 Accessibility 와 나란히 취급한다는 사실 자체가 이 앱의 대상 macOS 버전에서는 두 권한이 함께 필요함을 강하게 시사한다. 이 문서는 **두 권한 모두 F-07 의 전제조건**으로 취급한다.

### 3.2 온보딩 흐름 — 결정과 근거

**결정: Accessibility·Input Monitoring 은 최초 실행 시 선요청, Screen Recording 은 Seek 최초 사용 시 지연 요청(hybrid).**

근거:

- Accessibility 와 Input Monitoring 은 앱의 **존재 이유**(키 리매핑, F-07)의 전제 조건이다. 이 둘 없이는 메뉴바에 떠 있는 것 말고는 할 수 있는 일이 없다. 최초 실행에서 바로 요청하지 않으면, 사용자는 앱을 켜놓고도 아무 키도 리매핑되지 않는 상태로 방치되며 왜 그런지 알 방법이 없다.
- Screen Recording 은 Seek 라는 **단일 기능의 단일 경로(OCR)** 에만 관여한다(§3.1). 게다가 Screen Recording 권한 프롬프트는 macOS 에서 사용자에게 가장 거부감이 큰 권한 중 하나로 알려져 있어("이 앱이 내 화면을 다 볼 수 있다니"), 앱을 켜보지도 않은 시점에 요청하면 온보딩 이탈률을 높일 위험이 있다. Seek 를 처음 실제로 쓰려는 순간 — 즉 "이 기능을 쓰려면 이 권한이 필요하다" 는 인과관계가 화면에 분명히 보이는 순간 — 요청하는 것이 사용자 신뢰를 얻기에 자연스럽다.
- 이 판단은 조사 자료에 명시된 순서 지침이 아니라, "Screen recording permissions are only used for Seek" 라는 FAQ 원문이 부여하는 기능 범위와, 세 권한의 영향 범위 차이(1개 vs 앱 전체)로부터 도출한 설계 결정이다 `(추정 — 설계 판단, 사실 아님)`.

전부 미리 받는 대안(모두 최초 실행에 일괄 요청)을 기각한 이유: 세 프롬프트를 연달아 띄우면 macOS 프롬프트 자체가 앱 사이 전환·다른 창 뒤에 숨는 문제(§5 항목 10)가 겹칠 위험이 커지고, 아직 써보지도 않은 기능(Seek)의 권한을 미리 요구하는 것은 "필요할 때 요청" 이라는 iOS/macOS 권한 UX 관례에도 어긋난다.

### 3.3 온보딩 상태 머신

| 상태 | 진입 조건 | 화면/동작 | 다음 상태 |
| :--- | :--- | :--- | :--- |
| 시작(Start) | 앱 최초 실행, 또는 §3.5 폴링에서 핵심 권한 미부여 감지 | 온보딩 화면 표시, "손쉬운 사용 권한이 필요합니다" 설명 | AX 요청중 |
| AX 요청중 | 사용자가 "권한 요청" 버튼 클릭 | `AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt: true])` 호출 | AX 대기중 |
| AX 대기중 | — | 폴링으로 `AXIsProcessTrusted()` 결과 대기(§3.5) | AX 부여됨 또는 AX 거부됨 |
| AX 거부됨 | 폴링에서 일정 시간 경과 후에도 `false` | "시스템 설정에서 직접 켜세요" 안내 + 딥링크 버튼(§3.4) | AX 대기중(딥링크 후 재폴링) |
| AX 부여됨 | 폴링에서 `true` 확인 | 다음 권한으로 자동 진행 | IM 요청중 |
| IM 요청중 | AX 부여됨 진입 직후 자동 | `IOHIDRequestAccess(kIOHIDRequestTypeListenEvent)` 호출 | IM 대기중 |
| IM 대기중 | — | 폴링으로 `IOHIDCheckAccess()` 결과 대기 | IM 부여됨 또는 IM 거부됨 |
| IM 거부됨 | 폴링에서 일정 시간 경과 후에도 거부 상태 | "시스템 설정에서 직접 켜세요" 안내 + 딥링크 버튼(§3.4) | IM 대기중(딥링크 후 재폴링) |
| IM 부여됨 | 폴링에서 부여 확인 | F-07 에 탭 설치 시도 신호 전달. 온보딩 화면 "핵심 기능 준비 완료" | 핵심 완료 |
| 핵심 완료 | — | 온보딩 종료, 일반 사용 화면으로 전환. Screen Recording 은 아직 요청하지 않음 | (일반 사용) |
| SR 요청중 | 일반 사용 중 Seek 최초 트리거 & `CGPreflightScreenCaptureAccess()` = false | Seek 오버레이 대신 "화면 기록 권한이 필요합니다" 인라인 안내 + `CGRequestScreenCaptureAccess()` 호출 | SR 대기중 |
| SR 대기중 | — | 폴링으로 `CGPreflightScreenCaptureAccess()` 결과 대기 | SR 부여됨 또는 SR 거부됨 |
| SR 거부됨 | 폴링에서 일정 시간 경과 후에도 `false` | "시스템 설정에서 직접 켜세요" 안내 + 딥링크 버튼. 이후 Seek 는 AX 파싱 경로로만 동작(§3.6) | SR 대기중(딥링크 후 재폴링) |
| SR 부여됨 | 폴링에서 `true` 확인 | Seek OCR 경로 활성화, 현재 트리거를 이어서 처리 | (일반 사용, 전체 기능) |
| 불일치 감지 | 임의 상태에서 §3.7 판정 조건 충족 | `"Unable to initialize Superkey"` 화면으로 강제 전환 | 복구 절차중 |
| 복구 절차중 | — | §3.7 의 5단계 안내 UI | Mac 재시작 후 앱 재실행 → 시작(Start) |

### 3.4 거부 후 복구 — 시스템 설정 딥링크

| 권한 | 딥링크 URL | 확인 상태 |
| :--- | :--- | :--- |
| Accessibility | `x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility` | 사실 — rust-macos-capability-notes.md §2.5 원문에 그대로 인용됨 |
| Screen Recording | `x-apple.systempreferences:com.apple.preference.security?Privacy_ScreenCapture` | `(추정)` — Accessibility 와 동일한 URL 스킴 패턴을 따른다고 가정. 실기에서 시스템 설정을 열어 앵커가 실제로 Screen Recording 패널로 스크롤되는지 확인 필요 → §9 |
| Input Monitoring | `x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent` | `(추정)` — Accessibility 와 동일한 URL 스킴 패턴을 따른다고 가정. 위와 동일하게 실기 확인 필요 → §9 |

공통 UI 요소:

- **딥링크 버튼**: 클릭 시 위 URL 을 시스템에 위임해 시스템 설정 앱의 해당 개인정보 보호 패널을 직접 연다.
- **복사 가능한 경로 텍스트**: 딥링크가 실패하는 macOS 버전(URL 스킴이 macOS 메이저 버전 사이에서 바뀐 이력이 있음)에 대비해 "시스템 설정 → 개인정보 보호 및 보안 → [권한명]" 형태의 텍스트를 클릭 한 번으로 클립보드에 복사할 수 있게 한다.
- **확인 버튼 없음, 자동 폴링만**: 사용자가 시스템 설정에서 토글을 켠 뒤 앱으로 직접 돌아와 "확인"을 누르게 하지 않는다. §3.5 의 폴링이 상태 변화를 스스로 감지해 다음 단계로 진행한다 — 수동 확인 버튼은 사용자가 실제로 토글을 켜지 않고도 누를 수 있어 오탐 소지가 있다.

### 3.5 권한 상태 실시간 감시 — 폴링 채택

**결정: 폴링. 알림 구독은 채택하지 않는다.**

근거:

- rust-macos-capability-notes.md §2.5 에 열거된 세 확인 API(`AXIsProcessTrusted()`, `CGPreflightScreenCaptureAccess()`, `IOHIDCheckAccess()`) 는 모두 **동기 조회형** API 이며, 조사 노트 어디에도 이 세 권한의 변경을 알려주는 Darwin 알림·KVO·콜백 메커니즘이 확인되지 않았다. 없는 것을 있다고 가정하고 설계할 수 없다.
- 따라서 유일하게 근거가 있는 방법은 주기적 폴링이다. 폴링 주기는 조사 자료에 근거가 없어 설계 판단으로 정한다 `(추정)`: 온보딩 진행 중(§3.3 의 "대기중" 상태들)에는 짧은 주기(예: 1초)로, 온보딩이 끝난 뒤 백그라운드 감시 중에는 배터리·CPU 부담을 줄이기 위한 긴 주기(예: 10~30초)로 이원화한다. 정확한 값은 실측 후 조정 대상 → §9.
- 보조 트리거로 **앱이 포그라운드로 돌아올 때**(사용자가 시스템 설정 창에서 앱으로 다시 전환하는 macOS 표준 동선과 일치)와 **F-07 이 탭 설치·재활성화를 시도하기 직전**에 즉시 1회 확인을 추가한다. 이는 폴링 주기를 기다리지 않고 사용자가 방금 시스템 설정에서 바꾼 값을 최대한 빨리 반영하기 위함이다.

### 3.6 권한 조합별 기능 가용성

| Accessibility | Input Monitoring | Screen Recording | 결과 |
| :---: | :---: | :---: | :--- |
| ✅ | ✅ | ✅ | 전체 기능 정상. Seek 는 OCR + AX 파싱 병합 검출(F-02) |
| ✅ | ✅ | ❌ | 키 리매핑·Hyperkey·Presets 정상. Seek 는 AX 파싱 경로만 동작(`Seek using macOS accessibility` 옵션이 켜져 있는 경우) — OCR 대상(AX 트리에 노출되지 않는 텍스트, 예: 캔버스 렌더링된 텍스트)은 검출 불가 |
| ✅ | ❌ | ✅ | F-07 이 `CGEventTap` 을 설치하지 못하거나 이벤트를 받지 못함(§3.1 각주) → 키 리매핑·Seek 트리거(`Remap key to Seek:`) 전부 무력화. `Toggle Seek with shortcut:` 전역 단축키 경로만 남을 가능성 `(추정, F-01 §5 항목 9와 동일 근거)` |
| ❌ | ✅/❌ | ✅/❌ | Accessibility 가 없으면 클릭 실행(F-04) 자체가 불가능하고 키 리매핑도 불가능 — 사실상 앱 전체가 무력화. Screen Recording 만 있어도 의미 없음(클릭할 수 없으므로 Seek 도 무의미) |
| ✅ | ✅ | ✅였다가 런타임 취소 | 이미 열려 있던 Seek 세션은 다음 OCR 캡처 시도에서 실패 → F-02 가 AX 전용 폴백으로 자동 전환하거나 세션을 취소해야 한다(F-02 소관, F-11 은 권한 상태만 알림) |

### 3.7 `"Unable to initialize Superkey"` — 복구 안내 UI

**판정 조건(감지 방법)** `(추정)`: Accessibility 확인 API(`AXIsProcessTrusted()`)가 `true` 를 반환하는데도 F-07 이 `CGEventTapCreate` 호출 시 `NULL` 을 반환하는 상태를 불일치로 판정한다. 이 판정은 조사 자료에 명시적으로 서술되어 있지 않다 — FAQ 원문은 "even when the app has the necessary Accessibility settings enabled" 라고만 말해 증상만 서술할 뿐 앱 내부에서 어떻게 이를 프로그래밍적으로 감지하는지는 밝히지 않는다. `AXIsProcessTrusted() == true` 이면서 `CGEventTapCreate() == NULL` 인 조합은 권한 DB 와 커널 권한 사이의 불일치를 관찰할 수 있는 유일한 지점(F-07 이 실제로 탭을 설치하려 시도하는 지점)이라는 점에서 합리적인 판정 조건으로 채택했다. 최종 확인은 §9 로 승계한다.

**복구 안내 UI 명세** — superkey-inventory.md §1.3 원문 5단계를 그대로 화면 단계로 옮긴다:

| 단계 | 원문 절차 | UI 요소 |
| :---: | :--- | :--- |
| 1 | Close Superkey if it's running | 안내 텍스트만 표시("이 화면을 닫지 말고 계속 진행하세요" — 이 화면 자체는 앱의 일부이므로 "종료" 는 다음 단계들을 마친 뒤 사용자가 직접 Dock/메뉴바에서 종료하도록 안내). 확인 버튼 없이 다음 단계로 자동 진행 |
| 2 | System Settings → Privacy & Security → Accessibility 에서 앱을 **비활성화 후 목록에서 제거** | 딥링크 버튼(`x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility`) + 복사 가능한 절차 텍스트("먼저 토글을 끄고, `-` 버튼으로 목록에서 제거하세요") + "완료했습니다" 확인 버튼(폴링으로 자동 감지가 불가능한 단계이므로 — 목록에서 제거되었는지는 앱이 스스로 확인할 안전한 방법이 없다 `(추정)`) |
| 3 | System Preferences → Privacy & Security → Input Monitoring 에서도 동일 | 딥링크 버튼(`x-apple.systempreferences:com.apple.preference.security?Privacy_ListenEvent`, §3.4 `(추정)`) + 동일한 복사 텍스트 + "완료했습니다" 확인 버튼 |
| 4 | Restart your mac | 재시작 안내 텍스트 + "지금 재시작" 버튼(macOS 재시작 다이얼로그 트리거) 또는 "나중에 직접 재시작" 안내. 재시작을 건너뛰고 앱을 계속 쓸 수 없다는 점을 명확히 경고 — 원문 절차에 재시작이 필수 단계로 포함되어 있으므로 이를 생략하면 복구가 보장되지 않는다 |
| 5 | Launch Superkey and enable settings for it as prompted | 재시작 후 앱이 자동 실행되도록 로그인 항목에 등록되어 있다면(§4) 별도 안내 불필요. 그렇지 않다면 "재시작 후 Superkey 를 다시 실행하세요" 안내. 앱이 다시 실행되면 §3.3 의 시작(Start) 상태부터 온보딩이 처음부터 다시 진행된다 |

이 5단계 전체를 하나의 화면에서 순차 진행형 체크리스트로 보여주며, 각 단계는 이전 단계가 완료(자동 감지 또는 사용자의 "완료했습니다" 확인)되기 전까지 비활성화 상태로 둔다.

## 4. 설정 항목

권한 자체에 대한 **전용 설정 항목은 없다.** superkey-inventory.md §3.4 는 `General` 탭에 "로그인 시 실행, 메뉴바 아이콘 표시/숨김, 자동 업데이트 확인, 라이선스 등록·상태, 버전 정보, 환경설정 초기화" 가 있다고 확인했으나 권한 관련 항목은 그 목록에 없고, §7 Q2 가 `General` 탭 전체를 미확인 상태로 남겨두었다.

`(추정)`: `General` 탭(또는 그에 준하는 위치)에 세 권한의 현재 부여 상태를 읽기 전용으로 보여주는 상태 표시(예: Accessibility ✅ / Screen Recording ❌ / Input Monitoring ✅ 형태의 목록)와, 각 항목 옆에 "시스템 설정 열기" 딥링크 버튼이 있을 가능성이 높다 — 권한이 이 앱의 핵심 전제 조건이라는 성격상, 사용자가 문제가 생겼을 때 온보딩 흐름을 다시 겪지 않고도 상태를 확인할 곳이 있어야 자연스럽기 때문이다. 다만 이는 조사 자료로 확정되지 않았으므로 §9 로 승계한다. 이 상태 표시 UI 자체의 배치·디자인은 `F-09`(환경설정 창 UI) 소관이며, F-11 은 표시할 상태 데이터(3종 권한의 boolean)만 제공한다.

## 5. 엣지 케이스와 실패 모드

1. **프롬프트는 한 번만 뜬다.** `AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt: true])` 를 거부 후 재호출해도 시스템 프롬프트가 다시 뜨지 않는다(rust-macos-capability-notes.md §2.5). 앱은 이 사실을 알고 두 번째 호출부터는 프롬프트 대신 딥링크 안내로 전환해야 한다 — 반복 호출로 사용자가 "왜 아무 반응이 없지" 하고 느끼게 하면 안 된다.
2. **앱을 옮기면 TCC 경로가 바뀐다.** TCC 는 번들 ID + `csreq` 로 권한을 키잉하지만(rust-macos-capability-notes.md §3.2), 일부 TCC 검사는 실행 파일의 경로도 함께 기록하는 것으로 알려져 있어 `.app` 을 `/Applications` 밖의 다른 경로로 옮기거나 이름을 바꾸면 권한이 재확인을 요구할 수 있다 `(추정)`. 온보딩 상태 머신(§3.3)이 "이미 온보딩을 마친 사용자" 도 시작(Start) 상태로 재진입할 수 있는 구조여야 하는 이유가 이것이다.
3. **재빌드로 권한이 소실된다(개발 환경).** ad-hoc 서명은 빌드마다 `cdhash` 가 바뀌어 TCC 가 매번 "새 앱" 으로 취급한다(rust-macos-capability-notes.md §3.2). §7 의 개발 워크플로 요구사항으로 대응한다.
4. **`tauri dev` 에서는 권한이 터미널(부모 프로세스)에 귀속된다.** `.app` 번들이 아니라 `target/debug/<binary>` 를 직접 실행하므로 TCC 가 이 실행 파일이 아니라 부모 프로세스의 권한으로 판정한다(rust-macos-capability-notes.md §3.3). 권한이 필요한 기능은 `tauri dev` 로는 애초에 검증이 불가능하다 — 이는 버그가 아니라 구조적 한계다.
5. **MDM 으로 권한이 강제된 기업 환경.** 기업이 MDM 프로파일로 특정 앱의 TCC 권한을 사전 승인하거나(`PPPC` 프로파일), 반대로 특정 권한을 조직 정책으로 영구 차단할 수 있다. 이 경우 `AXIsProcessTrustedWithOptions` 의 프롬프트 자체가 뜨지 않거나, 사용자가 시스템 설정에서 토글을 조작할 수 없는(회색 처리된) 상태로 보일 수 있다. 앱은 이 상태에서 딥링크로 유도해도 사용자가 할 수 있는 일이 없다는 것을 인지하고, "관리자에게 문의하세요" 류의 문구로 분기해야 한다 `(추정 — 조사 자료에 근거 없음)`.
6. **사용자 전환(Fast User Switching).** TCC 는 사용자별로 별도의 DB(`~/Library/Application Support/com.apple.TCC/TCC.db`)를 갖는다. 관리자 계정에서 권한을 부여해도 다른 사용자 계정으로 전환하면 그 계정에서는 처음부터 다시 온보딩을 거쳐야 한다. 특히 관리자 권한이 필요한 시스템 전역 TCC.db 항목과 사용자별 항목의 우선순위 차이는 조사 자료로 확정되지 않았다 `(추정)`.
7. **macOS 메이저 업그레이드 후 권한 리셋.** Apple 은 메이저 OS 업그레이드 시 일부 또는 전체 TCC 권한을 초기화하는 경우가 있다(알려진 관례이나 이 앱에 대한 직접 검증은 없음 `(추정)`). 앱은 업데이트 직후 첫 실행에서 §3.3 의 시작(Start) 상태로 정상적으로 재진입할 수 있어야 하며, 별도의 "OS 업그레이드 감지" 로직 없이도 폴링·상태 머신만으로 이를 커버해야 한다.
8. **권한은 있는데 탭 생성이 실패한다.** §3.7 이 다루는 `"Unable to initialize Superkey"` 상태 그 자체 — 확인 API 는 `true`, 실제 커널 레벨 검사는 실패.
9. **Screen Recording 만 거부된 상태로 장기간 사용.** §3.6 의 두 번째 행. Seek 는 죽지 않지만 OCR 검출 대상을 놓치는 사례가 누적된다. 사용자가 "왜 이 텍스트는 안 잡히지" 라고 느낄 때마다 원인이 권한 때문인지 OCR 정확도 한계 때문인지 구분하기 어렵다 — Seek 오버레이 UI(F-03) 쪽에서 Screen Recording 미부여 상태를 시각적으로 표시할 필요가 있다는 시사점을 남긴다(F-03/F-09 승계 사항).
10. **권한 프롬프트가 다른 창 뒤에 뜬다.** macOS 의 TCC 프롬프트는 항상 최상위로 뜨는 것이 보장되지 않으며, 특히 앱이 백그라운드 유틸리티(Dock 아이콘 없는 Accessory 앱, rust-macos-capability-notes.md §2.6)로 동작할 때 시스템 프롬프트가 전체화면 앱이나 다른 창 뒤에 가려질 수 있다. 온보딩 화면 자체가 "권한 창이 안 보이면 `⌘Tab` 으로 시스템 설정을 확인해보세요" 같은 보조 안내를 상시 노출해야 한다 `(추정)`.

## 6. 필요한 플랫폼 API

- **Accessibility**: `AXIsProcessTrusted()`(확인), `AXIsProcessTrustedWithOptions([kAXTrustedCheckOptionPrompt: true])`(요청 — 프롬프트 동반). rust-macos-capability-notes.md §2.5. Info.plist 에 별도 사용 설명 키 없음(같은 절 표).
- **Screen Recording**: `CGPreflightScreenCaptureAccess()`(확인, 프롬프트 없이 조용히 조회), `CGRequestScreenCaptureAccess()`(요청 — 프롬프트 동반). 같은 절. Info.plist 사용 설명 키 없음.
- **Input Monitoring**: `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)`(확인), `IOHIDRequestAccess(kIOHIDRequestTypeListenEvent)`(요청). 같은 절. ⭐ **전용 Rust 크레이트가 없다** — `#[link(name="IOKit", kind="framework")]` 로 IOKit 프레임워크를 직접 링크하고 `extern "C"` 로 두 함수를 손으로 선언해야 한다(rust-macos-capability-notes.md §2.5: "Input Monitoring 의 `IOHIDCheckAccess`/`IOHIDRequestAccess` 는 전용 크레이트가 없어 IOKit 을 직접 링크하고 extern "C" 선언을 손으로 써야 한다").
- **`CGEventTapCreate` (판정용, 설치 자체는 F-07 소관)**: F-11 은 §3.7 의 불일치 판정을 위해 F-07 이 노출하는 "탭 생성 성공/실패" 신호를 구독한다. `CGEventTapCreate` 호출 자체는 F-07 의 책임이다(rust-macos-capability-notes.md §2.1).
- **시스템 설정 딥링크**: `x-apple.systempreferences:` URL 스킴을 macOS 표준 URL 오픈 메커니즘(`NSWorkspace.open` 또는 Tauri 의 shell-open 계열)으로 호출. Accessibility 앵커(`Privacy_Accessibility`)는 rust-macos-capability-notes.md §2.5 원문에 직접 인용된 사실이고, Screen Recording(`Privacy_ScreenCapture`)·Input Monitoring(`Privacy_ListenEvent`) 앵커는 `(추정)`(§3.4, §9).
- **재시작 트리거**(§3.7 단계 4): macOS 표준 재시작 다이얼로그를 여는 AppleScript(`tell application "System Events" to restart`) 또는 이에 준하는 호출. 조사 자료에 이 앱이 실제로 재시작 버튼을 제공하는지에 대한 근거는 없다 `(추정)` → §9.

## 7. 구현 접근

**판정: Rust 바인딩(Accessibility·Screen Recording) + Input Monitoring 은 크레이트 부재로 인한 수동 FFI 선언(그래도 "Rust 바인딩" 범주 — 네이티브 shim 은 아님).**

- **Accessibility**: `axuielement` 0.9.1 이 `ProcessTrust` 래퍼를 제공하며(rust-macos-capability-notes.md §1.2), 이것으로 `AXIsProcessTrusted()`/`AXIsProcessTrustedWithOptions()` 호출이 커버된다. 더 얇은 대안으로 `macos-accessibility-client` 0.0.2(`application_is_trusted()`/`application_is_trusted_with_prompt()` 전용 초소형 크레이트)도 있다. `tauri-plugin-macos-permissions` 2.3.0 도 Accessibility 확인·요청을 제공한다(같은 절: "Accessibility / Full Disk / 마이크 / 카메라 TCC 확인·요청") — 단 이 플러그인은 **Screen Recording 과 Input Monitoring 은 다루지 않는다**는 점에 주의. F-11 은 이 플러그인 또는 `axuielement` 중 하나를 Accessibility 전용으로 채택할 수 있으나, 어느 쪽도 세 권한을 통합 제공하지 않으므로 세 권한을 하나의 내부 모듈로 감싸는 것은 이 앱이 직접 해야 한다.
- **Screen Recording**: `core-graphics` 0.25.0 이 `CGPreflightScreenCaptureAccess`/`CGRequestScreenCaptureAccess` 안전 래퍼를 제공한다(rust-macos-capability-notes.md §1.2).
- **Input Monitoring**: 위 어떤 크레이트도 `IOHIDCheckAccess`/`IOHIDRequestAccess` 를 노출하지 않는다(같은 절, "존재하지 않는 크레이트는 이 표에 없다" 원칙과 §2.5 의 명시적 확인). IOKit 프레임워크 자체는 macOS SDK 에 포함되어 있고(rust-macos-capability-notes.md §3.1, CLT 로 충분) 별도 네이티브 코드 작성 없이 `extern "C"` 선언만으로 링크할 수 있다. 이는 Swift/Objective-C 로 별도 shim 앱이나 브리지를 작성해야 하는 "네이티브 shim 불가피" 범주와는 다르다 — 함수 시그니처를 손으로 선언하는 것은 여전히 순수 Rust 코드 안에서 끝난다.
- **딥링크·재시작 트리거**: Tauri 의 shell-open API 또는 표준 라이브러리 `Command` 로 `open` / `osascript` 를 호출하는 것으로 충분하다. 별도 크레이트 불필요.
- **결론적으로 F-11 전체에 네이티브 Swift/Objective-C shim 은 불필요하다.** 유일한 특이점은 Input Monitoring 의 수동 FFI 선언이며, 이는 여전히 "Rust 바인딩" 판정 범주 안에 있다.

### 개발 워크플로 요구사항 ⭐

rust-macos-capability-notes.md §3.2·§3.3 이 이 프로젝트의 개발 루프에 직접 영향을 주는 사실을 확정했으므로, F-11 구현과 무관하게 프로젝트 전체의 개발 워크플로 요구사항으로 명시한다.

1. **TCC 는 번들 ID + `csreq` 로 권한을 키잉한다.** ad-hoc 서명(`codesign --sign -`)은 안정적 서명 주체가 없어 `csreq` 가 바이너리의 `cdhash` 에 묶이고, 코드가 1바이트만 바뀌어도 `cdhash` 가 달라져 macOS 가 다른 앱으로 취급한다 — **매 빌드마다 권한이 소실된다.**
2. **공증(notarization)은 TCC 와 무관하다.** 공증은 Gatekeeper 가 요구하는 배포용 요건이며, 로컬 개발 중 권한을 받기 위해 필요한 것이 아니다. 개발 중 공증을 신경 쓸 필요가 없다.
3. **개발 중 우회책**: 로컬에서 자체 서명 인증서를 하나 생성해 키체인에 "항상 신뢰" 로 등록하고, **모든 개발 빌드를 이 동일한 인증서로 서명**한다. 서명 주체가 고정되므로 `csreq` 가 안정되고 재빌드해도 권한이 유지된다. 배포 단계에서만 Developer ID 인증서 + 공증으로 교체한다.
4. **`tauri dev` 는 이 워크플로에서 배제한다.** `tauri dev` 는 `.app` 번들이 아니라 `target/debug/<binary>` 를 직접 실행하고, TCC 는 이를 부모 프로세스(터미널)의 권한으로 판정한다. **권한이 필요한 기능(F-07 의 리매핑 엔진, F-01~F-04 의 Seek, F-11 의 권한 흐름 자체)은 반드시 `tauri build` 산출 `.app` 을 위 3번의 고정 자체 서명 인증서로 서명한 뒤 그 번들을 직접 실행해서 테스트한다.** UI 로직·순수 애플리케이션 로직만 검증할 때는 `tauri dev` 를 계속 사용해도 무방하다.

## 8. 수용 기준

- [ ] 앱 최초 실행 시 Accessibility 권한 요청 프롬프트가 자동으로 표시된다(사용자가 별도 버튼을 찾아 누르지 않아도 온보딩 화면에서 명확히 유도됨).
- [ ] Accessibility 가 부여되면 Input Monitoring 요청이 자동으로 이어지며, 두 권한이 모두 부여되기 전까지 Screen Recording 요청은 트리거되지 않는다.
- [ ] Accessibility 프롬프트를 거부한 뒤 앱 내에서 재시도 버튼을 눌러도 시스템 프롬프트가 다시 뜨지 않고, 대신 시스템 설정 딥링크 안내로 전환된다.
- [ ] 시스템 설정에서 사용자가 Accessibility 토글을 켜면, 앱으로 돌아왔을 때 별도의 "확인" 버튼 클릭 없이 폴링만으로 다음 단계(Input Monitoring)가 자동 진행된다.
- [ ] Seek 를 최초로 트리거하는 시점(Screen Recording 미부여 상태)에 Screen Recording 요청 프롬프트가 표시되고, 이전까지는 표시되지 않는다.
- [ ] 세 권한이 모두 부여된 상태에서 앱을 재실행하면 온보딩 화면이 다시 나타나지 않고 즉시 일반 사용 화면으로 진입한다.
- [ ] 런타임 중 시스템 설정에서 Accessibility 를 끄면, 다음 폴링 주기(§3.5 정의 값) 이내에 앱이 이를 감지하고 권한 재요청 안내를 노출한다.
- [ ] Screen Recording 만 거부된 상태에서 키 리매핑과 Seek 의 AX 파싱 경로는 정상 동작하고, OCR 경로만 비활성화된다(크래시·행 없음).
- [ ] Accessibility 확인 API 가 `true` 를 반환하는데 F-07 의 탭 생성이 실패하는 상황이 재현되면, 표준 온보딩 화면이 아니라 `"Unable to initialize Superkey"` 화면으로 전환된다.
- [ ] `"Unable to initialize Superkey"` 화면은 원문 5단계(종료·Accessibility 제거·Input Monitoring 제거·재시작·재실행)를 순서대로, 각 단계에 딥링크 또는 복사 가능한 안내 텍스트와 함께 표시한다.
- [ ] 개발 빌드를 고정된 자체 서명 인증서로 서명한 뒤 연속으로 재빌드해도(코드 수정 → 재빌드 → 재실행) 이미 부여된 권한이 유지되어 재승인 프롬프트가 다시 뜨지 않는다.
- [ ] `tauri dev` 로 실행한 프로세스에서는 권한 요청 시도 시 이것이 `tauri build` 산출물이 아니라는 경고(로그 또는 UI)가 표시된다 — 터미널 권한으로 오판되는 상황을 개발자가 인지할 수 있어야 한다.
- [ ] 딥링크 버튼 클릭 시 시스템 설정 앱이 해당 개인정보 보호 패널로 열린다(세 권한 각각에 대해).

## 9. 미해결 질문

| # | 질문 | 조사 문서 연결 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | Screen Recording(`Privacy_ScreenCapture`)과 Input Monitoring(`Privacy_ListenEvent`) 딥링크 URL 의 정확한 앵커 이름과, macOS 버전별 URL 스킴 안정성 | §3.4, §6 — Accessibility 앵커만 조사 노트에 직접 인용됨 | 실기(각 최신 macOS 버전)에서 URL 을 직접 열어 올바른 패널로 이동하는지 확인 |
| 2 | Input Monitoring 이 이 앱이 대상으로 하는 모든 macOS 버전에서 `CGEventTap` 설치에 실제로 필수인지, 아니면 Accessibility 만으로 충분한 버전이 있는지 | §3.1 각주, §3.6 | Apple 문서 및 각 macOS 버전에서 Input Monitoring 을 거부한 채 `CGEventTapCreate` 를 호출하는 실기 테스트 |
| 3 | `"Unable to initialize Superkey"` 불일치를 코드로 판정하는 정확한 조건(`AXIsProcessTrusted() == true` && `CGEventTapCreate() == NULL`)이 실제 증상과 일치하는지 | §3.7 | 원본 앱 개발자 문의, 또는 재빌드 반복으로 불일치 상태를 재현해 F-07 의 실제 반환값 관찰 |
| 4 | Screen Recording 권한을 프롬프트에서 승인한 직후, 앱 재시작 없이 같은 세션에서 바로 사용 가능한지(macOS 버전별 차이) | §2 시나리오 S1 | 각 macOS 버전에서 `CGRequestScreenCaptureAccess()` 승인 직후 곧바로 캡처 API 호출 테스트 |
| 5 | `General` 탭(또는 그에 준하는 위치)에 권한 상태 표시 UI 가 실제로 존재하는지, 존재한다면 정확한 레이아웃 | §4, superkey-inventory.md §7 Q2 | 앱 설치 후 확인(이번 조사 범위 밖) |
| 6 | 앱 번들을 다른 경로로 옮기거나 이름을 바꿀 때 TCC 권한이 실제로 재확인을 요구하는지 | §5 항목 2 | 실기 테스트(권한 부여 후 `.app` 을 이동/개명하고 재실행) |
| 7 | MDM(PPPC 프로파일)으로 사전 승인되거나 차단된 환경에서 확인 API 들이 어떤 값을 반환하는지, 그리고 UI 가 회색 처리된 토글을 사용자에게 어떻게 안내해야 하는지 | §5 항목 5 | 기업 MDM 테스트 환경에서 실기 검증(이번 조사 범위 밖) |
| 8 | 폴링 주기의 구체적 값(온보딩 중 vs 백그라운드 감시 중) | §3.5 | 배터리·CPU 부담과 반응 속도를 함께 측정하는 실기 벤치마크 |
| 9 | `"Unable to initialize Superkey"` 복구 화면의 4단계에서 앱이 실제로 "지금 재시작" 버튼(AppleScript 트리거)을 제공하는지, 아니면 텍스트 안내로 그치는지 | §6, §3.7 | 원본 앱 실기 확인(범위 밖) 또는 개발자 문의 |
| 10 | Accessibility 확인·요청에 `axuielement` 0.9.1 을 쓸지, 더 가벼운 `macos-accessibility-client` 0.0.2 또는 `tauri-plugin-macos-permissions` 2.3.0 을 쓸지 — F-07 등 다른 명세가 이미 `axuielement` 를 AX 트리 순회용으로 채택한다면 중복 의존을 피하기 위해 같은 크레이트로 통일하는 편이 나음 | §7 | F-02/F-07 명세의 최종 크레이트 선택과 대조 |
