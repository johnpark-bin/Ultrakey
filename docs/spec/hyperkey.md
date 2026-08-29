# F-05 · Hyperkey (hyper · meh · bleh)

> 한 줄 요약: 지정한 물리 키 하나를 눌렀다 뗄 때마다 `⌃⌥⌘⇧`(hyper) · `⌃⌥⇧`(meh) · `⌃⌘⇧`(bleh) 중 하나의 합성 modifier 조합을 키/마우스 이벤트에 얹거나 벗기는 규칙을 정의한다. 이 기능은 이벤트를 가로채고 재주입하는 메커니즘을 스스로 갖지 않으며, 그 메커니즘은 `F-07` 이 제공한다.
> 의존성: `F-07`(`key-remapping-engine.md`) — `CGEventTap` 설치·재활성화·중재 우선순위·quick press 판정. 본 문서는 그 엔진 위에 얹히는 **규칙 정의**만 다룬다.
> 관련 명세: `F-06`(`trackpad-hyper-gesture.md`, 트랙패드로 hyper 활성화) · `F-08`(`presets.md`, `Hyper + delete = forward delete` 등 개별 프리셋) · `F-09`(환경설정 창 UI 자체)

---

## 1. 개요

Hyperkey 는 원래 동일 제작자(Ryan Hanson)의 별도 앱이었고, Superkey 환경설정 창의 `Hyperkey` 탭으로 흡수되었다(조사 §4.3). 핵심 개념은 "기본 단축키에서 잘 쓰이지 않는 modifier 조합을 하나 골라, 물리 키 하나를 그 조합으로 바꿔치기한다"는 것이다. 제작자 원문:

> "the concept is just to reassign a key to be cmd+control+shift+option, a key combination that is not typically used in any default shortcuts" — https://ryanhanson.dev/posts/superkey

Superkey 는 이 개념을 3종으로 확장했다.

| 조합 | 기호 | command 포함 | shift 포함 | 도입 시점 |
| :--- | :--- | :--- | :--- | :--- |
| hyper | `⌃⌥⌘⇧` (shift 제외 시 `⌃⌥⌘`) | ✅ | 설정에 따라 가변 | 초기 버전 (구 Hyperkey 앱부터) |
| meh | `⌃⌥⇧` | ❌ | ✅ (고정) | 초기 버전 |
| bleh | `⌃⌘⇧` | ✅ | ✅ (고정) | v1.62 |

hyper 는 다른 앱의 단축키에 "추가 modifier"로 그대로 쓰인다("The hyper key acts as an additional modifier key that you can use in all of your other apps that have keyboard shortcuts" — 랜딩 페이지). 즉 이 기능의 산출물은 Superkey 자체의 동작이 아니라, **다른 앱이 받는 키 이벤트의 modifier flag**다.

## 2. 사용자 시나리오

1. **런처 트리거로 사용** — `caps lock` 을 hyper 로 리매핑하고, Alfred 워크플로에 `Hyper+N` 을 등록해 NotePlan 을 실행한다(조사 §5, marvinolson.com 리뷰).
2. **손이 닿지 않는 물리 modifier 쌍 대체** — 4개 손가락을 동시에 눌러야 하는 `⌃⌥⌘⇧` 조합을 caps lock 한 키로 대체해 편안하게 입력한다.
3. **command 없는 조합이 필요한 경우** — meh(`⌃⌥⇧`)를 `right option` 에 배정해, hyper 와 별개로 command 를 포함하지 않는 단축키 체계를 만든다.
4. **좁은 조합이 필요한 경우** — `Include shift in hyper key` 를 끄고 `⌃⌥⌘`(shift 미포함)만 내보내, shift 를 요구하는 별도 단축키와 병용한다.
5. **세 번째 예비 조합이 필요한 경우** — bleh(`⌃⌘⇧`)를 세 번째 소스 키에 배정해, hyper·meh 와 겹치지 않는 세 번째 자동화 트리거 세트를 만든다.
6. **마우스 조작에도 modifier 를 얹는 경우** — hyper 소스 키를 누른 채 드래그하면(`Drag` ☑) 드래그 이벤트에도 `⌃⌥⌘⇧` 가 실려, 드래그 자체를 앱별 단축 동작(예: 특수 선택/복제 드래그)으로 재해석시킨다. 반대로 `Scroll` 은 기본 OFF 라, 자연 스크롤 도중 우연히 hyper 가 눌려 있어도 스크롤 확대/축소로 오작동하지 않는다.

## 3. 동작 명세

### 3.1 modifier 조합 정의

| 이름 | 조합 | 기호 | command | option | control | shift | 출처 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| hyper (Include shift ☑, 기본) | control+option+command+shift | `⌃⌥⌘⇧` | ✅ | ✅ | ✅ | ✅ | 랜딩 페이지 "all four modifiers combined: ⌃⌥⌘⇧"; 조사 §3.2 |
| hyper (Include shift ☐) | control+option+command | `⌃⌥⌘` | ✅ | ✅ | ✅ | ❌ | 조사 §3.2 "해제 시 hyper 는 ⌃⌥⌘ (shift 제외)" |
| meh | control+option+shift | `⌃⌥⇧` | ❌ | ✅ | ✅ | ✅ | 조사 §3.2, 라벨 원문 `(^⌥⇧)` |
| bleh | control+command+shift | `⌃⌘⇧` | ✅ | ❌ | ✅ | ✅ | appcast v1.62 "The bleh key (⌃⌘⇧) can now be configured…"; v1.65 "Fixes a bug where the bleh key could also contain the option key" → option 미포함이 확정 사실 |

`Include shift in hyper key` 는 **hyper 에만** 적용된다. meh 는 라벨 자체가 `(^⌥⇧)` 로 shift 를 항상 포함하는 고정 조합으로 표기되어 있고, bleh 도 `(⌃⌘⇧)` 로 shift 고정 표기다. 즉 meh·bleh 는 "shift 포함 여부를 끌 수 있는 옵션"이 없다 — 조사 자료에 그런 체크박스가 없다.

### 3.2 키 down/up 상태 전이

hyper/meh/bleh 는 각각 독립적인 상태 기계를 갖되 형태는 동일하다. 아래는 하나의 소스 키(예: hyper 로 배정된 `right command`)에 대한 전이다.

| 현재 상태 | 이벤트 | 동작 | 다음 상태 |
| :--- | :--- | :--- | :--- |
| Idle | 소스 키 physical keyDown | 합성 `flagsChanged` 이벤트를 발생시켜 해당 조합의 modifier 비트를 ON 으로 내보낸다. 원본 keyDown 은 소비(억제)한다 | Active |
| Active | 다른 키의 physical keyDown/keyUp | 그 키 이벤트에 현재 조합의 modifier flag 를 OR 하여 그대로 전달 | Active (유지) |
| Active | (`Apply modifiers to ...`) 해당 종류가 ☑ 인 마우스 이벤트 | 그 마우스 이벤트에 현재 조합의 modifier flag 를 OR 하여 전달 | Active (유지) |
| Active | 소스 키 physical keyUp | 합성 `flagsChanged` 이벤트를 발생시켜 modifier 비트를 OFF 로 내보낸다. 원본 keyUp 은 소비 | Idle |
| Active | event tap 비활성화(`kCGEventTapDisabledByTimeout`/`UserInput`, F-07 소관) | F-07 이 탭을 재활성화할 때까지 이 상태 기계는 입력을 받지 못한다. 재활성화 시점에 소스 키가 여전히 물리적으로 눌려 있다면 **keyUp 을 못 받은 채 남을 위험**이 있다(§5 엣지 케이스 4) | (불확정 — F-07 재활성화 절차에 의존) |

**단독으로 눌렀다 뗀 경우(quick press)의 의미는 조사 자료로 확정되지 않는다.** hyper/meh/bleh 자체에 quick press 동작이 있는지, 아니면 항상 "누르는 동안만 modifier"로만 동작하는지는 §9 미해결 질문으로 승계한다. 이는 `Presets` 탭의 `Quick press caps lock to execute` 와는 별개의 메커니즘일 가능성이 높지만(그 항목은 caps lock 전용이고 Hyperkey 탭에는 quick press 라벨이 없음), caps lock 을 hyper 소스로 동시에 배정한 경우 두 규칙이 같은 물리 키 위에서 경쟁하게 된다는 점은 조사 §6 관찰 2("Hyperkey 와 Presets 는 동일한 물리 키를 두고 경쟁한다")로 확정된 사실이다. 이 경쟁의 중재는 F-07/F-08 소관이다.

### 3.3 `Apply modifiers to keypress events and:` — 이벤트 타입 대응

이 설정은 hyper/meh/bleh 어느 조합이 Active 상태든 공통으로 적용되는 **전역 스위치**다(조사 §3.2 는 이 4개 체크박스를 Hyperkey 탭 전체에 하나만 배치했고, 조합별로 따로 두지 않았다).

| 체크박스 | 스크린샷 기본값 | 대응하는 이벤트 유형 (추정) | 개별 토글이 필요한 이유 |
| :--- | :--- | :--- | :--- |
| `Click` | ☑ | `kCGEventLeftMouseDown` / `Up`, `kCGEventRightMouseDown` / `Up`, `kCGEventOtherMouseDown` / `Up` (추정 — 버튼별 세분화 여부는 원문 미명시, §9) | 클릭에 modifier 를 얹어야 "Cmd+클릭 = 새 탭" 류의 앱 단축 클릭을 hyper 소스 키만으로 재현할 수 있다 |
| `Drag` | ☑ | `kCGEventLeftMouseDragged` / `RightMouseDragged` / `OtherMouseDragged` (추정) | 드래그 중간에 눌린 modifier 로 동작이 바뀌는 앱(예: 사각 선택 vs 자유 선택)을 위해 필요 |
| `Move` | ☑ | `kCGEventMouseMoved` (추정) | modifier + 마우스 이동만으로 반응하는 앱(예: 특정 오버레이 UI)을 위해 필요 |
| `Scroll` | ☐ | `kCGEventScrollWheel` (추정) | ⭐ `⌘+스크롤` 이 확대/축소로 예약된 앱(Safari, Maps, Photos 등)이 흔해, hyper 를 누른 채 무심코 스크롤하면 원치 않는 확대/축소가 발동한다. 그래서 이 항목만 스크린샷 기본값이 OFF 다 |

각 항목은 독립 체크박스이므로 임의 조합으로 켜고 끌 수 있다. 이 표의 `CGEventType` 상수 매핑은 조사 원문에 직접 명시되지 않았으며, `rust-macos-capability-notes.md` §2.1 이 확인한 `CGEventTap` 이 `kCGHeadInsertEventTap` 마스크로 임의 이벤트 유형을 가로챌 수 있다는 사실에서 타당하게 추론한 것이다 — 구현 전 반드시 실측 확인이 필요하다(§9).

### 3.4 호환성 함정 — Keyboard Maestro 류 shortcut recorder 비대칭

조사 원문(랜딩 FAQ):

> "Keyboard Maestro's shortcut recorder works a little differently than most, BUT if you just record your shortcut physically pressing all the modifiers, then the Hyperkey configured in Superkey (or Hyperkey) will properly trigger what you have configured in Keyboard Maestro."

즉 다음 두 동작은 **비대칭**이다.

- **등록(recording)**: 사용자가 hyper 소스 키를 눌러 단축키를 "녹화"하려 하면, 일부 recorder(Keyboard Maestro 등)는 이를 정상적인 물리 modifier 조합으로 인식하지 못해 실패할 수 있다.
- **트리거(triggering)**: 반대로, 사용자가 물리적으로 `⌃⌥⌘⇧` 를 직접 눌러 등록해 둔 단축키는, 이후 hyper 소스 키를 통한 합성 이벤트로 정상 트리거된다.

이 비대칭은 합성 `flagsChanged` 이벤트가 물리 modifier 키 4개를 동시에 누른 것과 **이벤트 스트림 형태가 다를 수 있다**는 것을 시사한다(예: 하나의 `flagsChanged` 로 4비트가 한 번에 바뀌는지, 4번의 개별 `flagsChanged` 로 순차적으로 바뀌는지). 정확한 이벤트 스트림 형태는 조사 자료로 확정되지 않으므로 §9 미해결 질문으로 남긴다. 명세상 반드시 지켜야 할 것은: **recorder 등록 실패는 버그가 아니라 알려진 한계이며, 트리거 자체는 정상 동작해야 한다.**

## 4. 설정 항목

조사 §3.2 `Hyperkey` 탭 스크린샷 기준 6개 행. 마지막 행(트랙패드 활성화)은 이 탭에 있지만 상세 동작은 `F-06` 소관이라 존재만 기록한다.

| # | 라벨 (원문) | 타입 | 기본값 | 유효범위 | 출처 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | `Remap key to hyper key:` | 체크박스 + 소스 키 선택 팝업 | 스크린샷 값 ☑ / `right command` — **실제 출고 기본값은 미확인** `(추정)` | 소스 키 enum. 확인된 후보: `caps lock`, `right command`, `right option`, `globe`. 전체 목록 미확정(조사 Q4) | 조사 §3.2 |
| 2 | `Include shift in hyper key` | 체크박스 | ☑ (스크린샷) `(추정)` | boolean | 조사 §3.2 |
| 3 | `Remap key to meh key (^⌥⇧):` | 체크박스 + 소스 키 선택 팝업 | 스크린샷 값 ☑ / `right option` `(추정)` | 소스 키 enum (항목 1 과 동일 후보군으로 추정) | 조사 §3.2 |
| 4 | `bleh key (⌃⌘⇧)`(라벨 형태 추정) | 체크박스 + 소스 키 선택 팝업 `(추정)` — meh 와 동형 UI로 추정, 실제 스크린샷 미공개 | 미확인. `(추정)` 기본 OFF(새로 추가된 3번째 조합이라 기존 사용자 설정을 건드리지 않기 위함으로 추정) | 소스 키 enum `(추정)` | appcast v1.62 / v1.65 |
| 5 | `Apply modifiers to keypress events and:` (`Click` `Drag` `Move` `Scroll`) | 체크박스 4개 (독립 토글) | `Click` ☑ · `Drag` ☑ · `Move` ☑ · `Scroll` ☐ (스크린샷) | boolean × 4 | 조사 §3.2 |
| 6 | `Engage hyper key using trackpad:` | 체크박스 + 팝업 (트랙패드 영역 선택) | ☐ / `top right` (스크린샷) | → 상세는 **`F-06` 참조**. 본 문서에서는 Hyperkey 탭 내 존재만 기록 | 조사 §3.2 |

## 5. 엣지 케이스와 실패 모드

1. **3종을 같은 소스 키에 배정** — 예를 들어 `right command` 를 hyper 와 meh 양쪽에 동시에 배정하려는 시도. UI/엔진 레벨에서 금지해야 하는지, 아니면 나중에 설정한 쪽이 이기는지 조사 자료로 확정되지 않는다. 최소한 **조용히 둘 다 무시되는 상태**는 없어야 한다.
2. **소스 키가 `Remap key to Seek` 트리거와 충돌** — `Seek` 탭의 `Remap key to Seek:` 도 같은 소스 키 enum 을 공유한다(예: `caps lock`). 두 탭에서 같은 키를 서로 다른 기능에 배정하면 물리 키 하나를 두고 기능 간 경쟁이 생긴다(조사 §6 관찰 2, v1.20/v1.62 버그 사례).
3. **소스 키를 누른 채 물리 modifier 키도 동시에 누름** — 예: hyper 소스 키(`right command`)를 누른 상태에서 물리 `shift` 도 누른다. 합성 `⌃⌥⌘⇧` 위에 물리 shift 가 중복으로 얹히는 형태가 되는데, 이 경우 flag 가 단순 OR 라면 문제없어야 하나, 일부 앱은 동일 modifier 의 중복 `flagsChanged` 를 다른 키로 오인할 수 있다.
4. **키를 누른 채 앱 전환(stuck modifier)** — hyper 소스 키를 누른 상태에서 `Cmd+Tab` 등으로 포커스가 다른 앱/Space 로 이동하면, event tap 이 keyUp 이벤트를 놓칠 수 있다. 이 경우 Active 상태가 해제되지 않아 이후 모든 키 입력에 hyper modifier 가 계속 얹히는 "stuck modifier" 상태가 된다. 복구 경로(예: 소스 키 재입력, 타임아웃)가 필요하다.
5. **전체화면 앱으로 전환** — event tap 재설치/재활성화 지연 구간(F-07 소관, v1.58 "절전 복귀·로그인 시 리매핑 미동작"과 동일 계열 실패 모드)과 겹치면, 전체화면 전환 직후 hyper 가 일시적으로 동작하지 않을 수 있다.
6. **Secure Input 활성 시(암호 필드)** — "Password text fields in macOS are secure and prevent 3rd party applications from knowing which keystrokes are pressed"(조사 §1.3). hyper/meh/bleh 를 포함한 모든 키 리매핑이 이 구간에서 완전히 무력화되며, 이는 macOS 제약이라 우회 불가능하다.
7. **globe(🌐/Fn) 키의 특수성** — v1.60 "Hyper + delete = forward delete will now work if you have the hyper key set to the globe key"(조사 §1.3 인용부)는 globe 키가 한때 hyper 소스로서 제대로 동작하지 않았다가 v1.60 에서 수정되었음을 시사한다. macOS 에서 `fn`/globe 키는 표준 `flagsChanged` 와는 다른 이벤트 경로(NX 이벤트 등)를 탈 수 있다는 것이 널리 알려진 특이사항이며 `(추정)`, 이 때문에 별도의 감지 로직이 필요했을 가능성이 있다.
8. **외장 키보드에 없는 키를 소스로 배정** — `right command`/`right option`/globe 키가 물리적으로 없는 키보드(Windows 키보드, 일부 서드파티 키보드)에서는 설정이 저장되어도 실질적으로 트리거할 방법이 없다.
9. **Keyboard Maestro 류 shortcut recorder 와의 공존** — §3.4 참조. 등록은 실패할 수 있으나 트리거는 정상이어야 한다는 비대칭을 사용자에게 문서화하지 않으면 "안 된다"는 오인 버그 리포트로 이어진다.
10. **다른 리매퍼(Karabiner-Elements, BetterTouchTool, 구 Hyperkey 앱 등)와 동시 실행** — 동일한 `CGEventTap` 자원 계층을 두고 경쟁한다. 다른 리매퍼가 먼저 같은 소스 키의 keyDown 을 소비하면 Superkey 의 hyper 규칙은 그 이벤트를 아예 받지 못할 수 있다.
11. **`Include shift in hyper key` 를 hyper Active 상태 도중 변경** — 설정 변경이 즉시 반영되어 현재 눌려 있는 hyper 의 modifier 구성이 바뀌는지, 다음 keyDown 부터 반영되는지 불명확하다.
12. **소스 키 단독 눌렀다 뗀 경우(quick press 여부 미정)** — §3.2 에서 다룬 대로, hyper 를 quick press 로 다루는 별도 규칙이 있는지, 아니면 항상 무동작인지 확정되지 않는다. caps lock 을 hyper 소스이자 `Quick press caps lock to execute` 대상으로 동시에 설정한 경우 결과가 불명확하다.

## 6. 필요한 플랫폼 API

`rust-macos-capability-notes.md` §2.1, §2.5 기준. F-05 는 이벤트 탭 설치·재활성화 자체를 새로 구현하지 않고 F-07 이 노출하는 콜백/훅에 규칙으로 참여한다.

| API | 용도 | 비고 |
| :--- | :--- | :--- |
| `CGEventTapCreate` (`kCGSessionEventTap`, `kCGHeadInsertEventTap`, `kCGEventTapOptionDefault`) | 소스 키의 `keyDown`/`keyUp`, 그리고 `Apply modifiers to ...` 대상 마우스 이벤트를 가로채는 콜백 등록 | 설치·재활성화 로직 자체는 **F-07 소관**. F-05 는 이 콜백 안에서 실행되는 규칙만 정의 |
| `CGEventGetFlags` / `CGEventSetFlags` | 눌려 있는 소스 키에 대응하는 modifier 비트마스크(`kCGEventFlagMaskControl` \| `Alternate` \| `Command` \| `Shift`)를 이벤트에 OR/AND 하여 얹거나 벗김 | hyper/meh/bleh 조합별 상수 마스크를 미리 정의해 둔다 |
| `CGEventCreateKeyboardEvent` + `CGEventSetType`(`kCGEventFlagsChanged`) | 소스 키 down/up 시점에 합성 `flagsChanged` 이벤트를 생성해 콜백 반환값으로 내보냄 | 원본 keyDown/keyUp 은 콜백에서 `None` 반환으로 소비 |
| `kCGEventType` 상수군 — `kCGEventLeftMouseDown/Up`, `RightMouseDown/Up`, `OtherMouseDown/Up`, `LeftMouseDragged` 등, `kCGEventMouseMoved`, `kCGEventScrollWheel` | `Click`/`Drag`/`Move`/`Scroll` 각 토글이 감시할 이벤트 마스크 결정 (§3.3, 매핑은 추정) | F-07 의 이벤트 탭 마스크에 이 유형들을 포함시켜야 함 |
| `AXIsProcessTrusted()` / `AXIsProcessTrustedWithOptions` | Accessibility 권한 확인 — 리매핑 실행의 전제 조건 | F-07·F-09 소관, F-05 는 소비만 |
| `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)` / `IOHIDRequestAccess` | Input Monitoring 권한 — 조사 §1.3 FAQ 복구 절차에서 관여가 확인된 3번째 권한 | F-07·F-09 소관 |

## 7. 구현 접근

**판정: Rust 바인딩.**

- 근거: F-05 가 필요로 하는 모든 동작(합성 `flagsChanged` 생성, 이벤트 flag 마스크 조작, 특정 `CGEventType` 감시)은 `rust-macos-capability-notes.md` §2.1 이 "✅"로 판정한 `CGEventTap` 계열 API 로 전부 커버된다. 이 기능은 §2.6(오버레이 창)처럼 `NSWindow` 레벨·컬렉션 비헤이비어 등 AppKit 을 직접 다뤄야 하는 부분이 없다 — modifier 비트 연산과 이벤트 유형 판별이라는 **순수 이벤트 레벨 로직**이기 때문이다. 따라서 네이티브 Objective-C shim 을 별도로 작성할 필요가 없다.
- 사용 크레이트: `core-graphics` **0.25.0** — `CGEventTap` 안전 래퍼와 `CGEvent`/`CGEventFlags`/`CGEventType` 바인딩을 제공(조사 노트 §1.2). F-07 이 이 크레이트(또는 `objc2-core-graphics` 0.3.2)로 탭 인프라를 구축하면, F-05 는 그 콜백 시그니처 위에서 flag 마스크 표(§3.1)와 이벤트 타입 표(§3.3)를 순수 Rust 상태 기계로 구현한다.
- 기각한 대안:
  - **`rdev` 0.5.3** — 조사 노트가 "3년간 릴리스 없음, 이벤트 소비 제어 제한적, 신규 의존 비권장"으로 명시. hyper 규칙은 원본 이벤트를 **소비하고 합성 이벤트로 치환**해야 하는데 `rdev` 는 이 제어가 약하다.
  - **네이티브 Swift/ObjC shim** — modifier 비트 연산에는 AppKit 이 전혀 필요 없어 shim 을 둘 이유가 없다. shim 은 F-07 이 다루는 `NSWindow`/`NSStatusItem` 류의 AppKit 전용 기능(§2.6, §2.7)에서만 정당화된다.
  - **`objc2-core-graphics` 단독(원시 FFI)** — 가능은 하지만 `core-graphics` 크레이트가 이미 이 용도에 맞는 얇은 안전 래퍼를 제공하므로, 굳이 헤더 자동 생성 바인딩을 직접 `unsafe` 로 호출할 이유가 적다. 다만 F-07 이 `objc2-core-graphics` 를 택할 경우 F-05 의 상태 기계는 그 타입에 맞춰 그대로 이식 가능하다(두 크레이트 모두 동일한 Core Graphics C API 를 감싸므로 로직 자체는 크레이트 독립적).

## 8. 수용 기준

- [ ] hyper 소스 키를 누르면 물리 keyDown 시점에 합성 `flagsChanged` 가 발생하고, `Include shift in hyper key` ☑ 상태에서는 대상 앱이 `⌃⌥⌘⇧` 4개 modifier 가 모두 눌린 것으로 인식한다.
- [ ] `Include shift in hyper key` 를 끄면 동일 소스 키 down 시 `⌃⌥⌘`(shift 제외) 3개 modifier 만 합성된다.
- [ ] meh 소스 키를 누르면 `⌃⌥⇧`(command 제외)가 합성된다.
- [ ] bleh 소스 키를 누르면 `⌃⌘⇧` 가 합성되며, option 은 어떤 경우에도 포함되지 않는다(v1.65 회귀 방지 대상).
- [ ] hyper/meh/bleh Active 상태에서 다른 키를 누르면, 그 키 이벤트에 현재 조합의 modifier flag 가 얹혀 대상 앱으로 전달된다.
- [ ] 소스 키를 떼면 즉시 합성 `flagsChanged` 로 modifier 가 해제되고, 이후 키 입력에는 더 이상 영향을 주지 않는다.
- [ ] `Apply modifiers to ... Click` 이 ☑ 인 상태에서 hyper Active 중 마우스 클릭에 합성 modifier flag 가 실려 전달된다. ☐ 이면 클릭 이벤트는 영향받지 않는다.
- [ ] `Drag`/`Move`/`Scroll` 각각 독립적으로 켜고 끌 수 있으며, 꺼진 이벤트 유형에는 modifier 가 얹히지 않는다(스크린샷 기본 구성: `Scroll` 만 OFF).
- [ ] Secure Input 이 활성인 필드에 포커스가 있는 동안에는 hyper/meh/bleh 리매핑이 전혀 동작하지 않는다(합성 이벤트가 생성되지 않는다).
- [ ] Keyboard Maestro 류 shortcut recorder 로 hyper 조합을 "기록"하는 것은 실패할 수 있지만, 물리 modifier 4개를 직접 눌러 등록해 둔 단축키는 Superkey 가 보낸 합성 hyper 이벤트로 정상 트리거된다.
- [ ] 동일 물리 키를 hyper/meh/bleh 중 둘 이상에 동시에 배정하려는 시도에 대해, 조용히 무시되지 않고 명시적인 우선순위 규칙 또는 차단이 적용된다.
- [ ] globe 키를 hyper 소스로 설정한 경우에도 동일하게 동작한다(v1.60 회귀 방지: `Hyper + delete = forward delete` 프리셋과의 연동은 `F-08` 검증 대상이나, hyper 자체의 flag 합성은 여기서 검증).

## 9. 미해결 질문

1. **quick press 의미론** — hyper/meh/bleh 를 단독으로 눌렀다 뗐을 때 별도 동작(quick press)이 있는지, 아니면 항상 "누르는 동안만 modifier"인지 조사 자료로 확정 불가. `F-07`/`F-08` 의 quick press 판정 메커니즘과 통합되는지도 불명확 — 앱 설치 후 실측 필요.
2. **소스 키 팝업의 선택지 전체 목록** — `caps lock`, `right command`, `right option`, `globe` 만 확인됨(조사 Q4). `left control`, `left option`, `left command` 등 나머지 후보 존재 여부 미확정.
3. **bleh key 의 실제 UI 형태** — meh 와 동형(체크박스+팝업)이라는 것은 추정이며, v1.62 이후 스크린샷이 공개되지 않아 실제 라벨 문구·배치 미확인.
4. **각 설정의 출고 기본값** — 스크린샷은 홍보용으로 구성된 값일 수 있어(조사 Q3), 신규 설치 시 실제 기본값(특히 `Remap key to hyper key` 의 ☑/☐ 초기 상태)은 `defaults read com.knollsoft.Superkey` 로 확인이 필요.
5. **여러 조합을 동시에 활성화했을 때의 상호작용** — hyper 와 meh 를 서로 다른 소스 키에 각각 배정해 두고 두 키를 동시에 누르면, modifier 가 합산되는지(`⌃⌥⌘⇧` ∪ `⌃⌥⇧` = `⌃⌥⌘⇧`) 아니면 나중에 눌린 쪽이 이전 것을 덮어쓰는지 불명확.
6. **globe/fn 키의 이벤트 경로** — `fn`/globe 키가 표준 `flagsChanged` 와 다른 이벤트 체계(NX 이벤트 등)를 타는지, 그로 인해 hyper 소스로서 별도 처리가 필요한지는 `(추정)`이며 v1.60 수정 커밋의 정확한 원인은 확인 불가.
7. **`Apply modifiers to ...` 4항목의 정확한 `CGEventType` 매핑** — 특히 `Click`/`Drag` 가 좌/우/기타 버튼(Left/Right/Other MouseDown 등)을 모두 포함하는지 원문에 명시되어 있지 않아 §3.3 매핑은 추정이다.
8. **런타임 설정 변경의 즉시 반영 여부** — hyper 가 Active 상태인 도중 `Include shift in hyper key` 등을 변경했을 때 그 순간부터 반영되는지, 다음 keyDown 부터인지 불명확.
