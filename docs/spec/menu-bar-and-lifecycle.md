# F-10 · 메뉴바 상주와 앱 수명주기

> **한 줄 요약**: SuperKey 클론을 Dock 아이콘 없는 메뉴바 상주 에이전트 앱으로 만드는 활성화 정책, `NSStatusItem` 메뉴 구성, 앱 상태 머신(미초기화~종료중), 단일 인스턴스 보장, 로그인 시 자동 실행, 절전·잠금해제·사용자전환 시 엔진 재초기화 시점, 정상 종료 시 정리, `"Unable to initialize Superkey"` 상태로의 진입·이탈을 정의한다.
> **의존성**: `F-11`(permissions-onboarding)의 온보딩 플로우를 최초 실행 시 호출하고, 앱 상태 머신이 참조하는 권한 상태를 F-11 이 제공한다는 전제로 동작한다. `F-07`(key-remapping-engine)에게는 "지금 재초기화하라"는 신호만 보낸다 — `CGEventTap` 설치·재활성화 메커니즘 자체는 F-07 소관이다.
> **관련 명세**: 권한 요청·복구 절차의 상세는 `F-11`, 환경설정 창 UI 는 `F-09`, 업데이트 확인 로직은 `F-13`, 라이선스 상태 표시의 내용은 `F-12`, `CGEventTap` 자체의 설치·재활성화 메커니즘은 `F-07` 을 참조한다.

---

## 1. 개요

SuperKey 클론은 **메뉴바 상주 앱(agent app)** 이다. Dock 아이콘이 없고 `⌘Tab` 앱 스위처에도 나타나지 않으며, 유일한 상시 접점은 메뉴바의 `NSStatusItem` 아이콘이다. 이는 macOS 에서 `NSApplicationActivationPolicyAccessory`(또는 `Info.plist` 의 `LSUIElement = true`)로 구현되는 표준 패턴이며, Tauri 에서는 `app.set_activation_policy(ActivationPolicy::Accessory)` 로 대응된다(rust-macos-capability-notes.md §2.6, §2.7).

메뉴바 아이콘 애셋은 조사에서 실제로 확인됐다 — `/assets/images/superkey menu icon.png`(superkey-inventory.md §7 Q15). 또한 appcast v1.62 는 `"Menu item icons have been added to the menu bar menu."` 라고 명시한다(superkey-inventory.md §2.1) — 즉 메뉴 자체의 아이콘 1개가 아니라 **메뉴를 펼쳤을 때 각 항목 옆에도 아이콘이 붙는 메뉴**다. 다만 그 메뉴가 정확히 어떤 항목들로 구성되는지는 조사에서 확인되지 않았다(superkey-inventory.md §7 Q15) — F-10 은 다른 기능 명세가 요구하는 항목들을 근거로 구성을 `(추정)` 하여 제안한다(§3.3).

F-10 이 다루는 두 번째 축은 **앱 수명주기**다. 에이전트 앱은 사용자가 명시적으로 실행하는 일이 드물고, 대부분 로그인 시 자동 실행되어 백그라운드에서 계속 떠 있다. 이 특성 때문에 일반 앱이라면 신경 쓰지 않아도 될 이벤트들 — 절전에서 깨어남, 화면 잠금 해제, 빠른 사용자 전환, 디스플레이 구성 변경 — 이 모두 "리매핑 엔진이 살아 있는가"를 위협하는 실제 실패 지점이 된다. appcast v1.58 은 `"In certain scenarios, Superkey's key remapping was not working upon wake or login."` 이라고 명시하는데(superkey-inventory.md §2.1), 이는 추정이 아니라 **실제로 발생했던 회귀**다. F-10 은 이런 수명주기 이벤트가 발생했을 때 "언제 F-07 에 재초기화를 요청하는가"를 못박는다 — 재초기화 자체를 어떻게 수행하는지는 F-07 의 몫이다.

세 번째 축은 **`"Unable to initialize Superkey"`** 상태다. 이는 조사에서 확인된 고정 실패 문구로(superkey-inventory.md §1.3), 권한이 macOS TCC 데이터베이스와 어긋났을 때 앱이 들어가는 상태다. F-10 은 이 상태에 "언제 진입하고, 그 상태에서 메뉴바가 무엇을 보여주며, 언제 빠져나오는가"라는 수명주기 관점만 명세한다 — 복구 절차(권한을 어떻게 다시 부여받는가)는 F-11 이 갖는다.

## 2. 사용자 시나리오

### 시나리오 A — 최초 설치 후 첫 실행

1. 사용자가 `.dmg` 에서 앱을 `/Applications` 로 드래그하고 최초 실행한다.
2. 앱은 즉시 Dock 아이콘 없이 메뉴바에만 아이콘을 표시하며, 상태 머신은 **미초기화(Uninitialized)** 로 시작한다.
3. 필요한 권한(Accessibility, Screen Recording, Input Monitoring)이 아직 없으므로 F-11 의 온보딩 플로우가 트리거되고, 상태는 **권한 대기(AwaitingPermissions)** 로 전이한다.
4. 사용자가 온보딩을 따라 시스템 설정에서 권한을 모두 부여한다.
5. F-10 은 권한 상태 변화를 감지하고 F-07 에 엔진 기동을 요청한다. 엔진이 정상 기동하면 상태는 **정상 동작(Running)** 으로 전이한다.

### 시나리오 B — 로그인 시 자동 실행

1. 사용자가 이전 세션에서 `Launch at login` `(추정)` 을 켜 두었다.
2. 맥을 재시작하고 로그인한다.
3. macOS 13 이상이면 `SMAppService.mainApp` 이 등록해 둔 로그인 항목을 통해, macOS 12 면 LaunchAgent 를 통해 앱이 자동 실행된다(§6, §7).
4. 이미 권한이 부여되어 있으므로 온보딩 없이 곧바로 **정상 동작(Running)** 상태로 진입하고, F-07 에 엔진 기동을 요청한다.

### 시나리오 C — 절전 복귀

1. 앱이 정상 동작 중 맥이 잠들었다가 사용자가 뚜껑을 열어 깨운다.
2. `NSWorkspace` 의 `didWakeNotification` 이 발화한다.
3. F-10 은 이 알림을 받아 F-07 에 "재초기화하라"는 신호를 보낸다 — appcast v1.58 이 명시한 "절전 복귀 시 리매핑 미동작" 회귀를 재발시키지 않기 위한 조치다.
4. 사용자는 별도 조작 없이 리매핑이 정상 동작함을 확인한다.

### 시나리오 D — 이중 실행 시도

1. 앱이 이미 메뉴바에 상주 중인데, 사용자가 Finder 에서 앱 아이콘을 실수로 다시 더블클릭한다(또는 로그인 항목과 수동 실행이 겹친다).
2. 두 번째 프로세스는 시작 직후 이미 같은 번들 ID 로 실행 중인 인스턴스가 있음을 감지한다.
3. 두 번째 프로세스는 새로운 `CGEventTap` 을 설치하지 않고 즉시 종료한다 — 그렇지 않으면 키 입력이 두 번 처리된다(§5 항목 1).

### 시나리오 E — 권한 DB 불일치로 초기화 실패

1. 사용자가 개발 중 앱을 재서명하거나(rust-macos-capability-notes.md §3.2), 시스템 설정에서 Accessibility 항목을 수동으로 건드리는 등, TCC 데이터베이스가 앱이 실제로 부여받은 권한과 어긋나는 상태가 된다.
2. 앱을 실행하면 F-07 의 엔진 기동 시도가 실패하고, 상태 머신은 **`Unable to initialize Superkey`** 로 전이한다.
3. 메뉴바 아이콘은 오류 상태를 나타내는 형태로 바뀌고(§3.1), 메뉴에는 이 상태를 알리는 문구가 원문 그대로 표시된다.
4. 사용자는 F-11 이 정의하는 복구 절차(앱 종료 → 권한 제거 → 재부팅 → 재실행 → 재승인)를 수행한다. 절차 완료 후 앱을 다시 실행하면 상태 머신은 미초기화부터 다시 시작해 정상 동작으로 도달한다.

### 시나리오 F — 메뉴에서 정상 종료

1. 사용자가 메뉴바 아이콘을 클릭해 메뉴를 열고 `Quit` `(추정)` 항목을 선택한다.
2. 상태는 **종료 중(Quitting)** 으로 전이한다.
3. F-10 은 F-07 에 event tap 해제를 요청하고, 현재 합성 상태로 눌려 있는 modifier 가 있으면 이를 해소(release 이벤트 재생)하도록 요청한다 — 눌린 채로 종료되면 시스템 전체의 modifier 상태가 어긋나 다른 앱에까지 영향을 준다.
4. 열려 있던 Seek 오버레이 창이 있으면 파괴를 요청한다.
5. 정리가 끝나면 프로세스가 종료된다.

## 3. 동작 명세

### 3.1 앱 상태 머신

| 상태 | 진입 조건 | 메뉴바 아이콘 표시 | 가능한 조작 |
| :--- | :--- | :--- | :--- |
| **미초기화(Uninitialized)** | 프로세스 시작 직후, 아직 권한·엔진 상태를 확인하기 전 | 기본 아이콘, 활성화 애니메이션 없음 `(추정)` | 없음 — 밀리초 단위로 즉시 다음 상태로 전이 |
| **권한 대기(AwaitingPermissions)** | 필요한 권한(Accessibility·Screen Recording·Input Monitoring) 중 하나 이상이 없음 | 경고를 나타내는 형태 `(추정)` — 상세는 F-11 | 메뉴 클릭 → F-11 온보딩 창 열기. `Quit` |
| **정상 동작(Running)** | 권한 모두 부여됨 + F-07 엔진 기동 성공 | 기본 아이콘 | 메뉴의 모든 항목 활성 — §3.3 |
| **`Unable to initialize Superkey`** | 권한은 시스템 설정상 부여된 것처럼 보이나 TCC DB 와 앱 서명이 어긋나 F-07 엔진 기동이 실패 | 오류를 나타내는 형태 `(추정)` | 메뉴 클릭 → 고정 문구 `"Unable to initialize Superkey"` 표시 및 F-11 복구 절차 안내로 연결. `Quit` |
| **트라이얼 만료(TrialExpired)** | 20일 체험 기간이 지났고 유효 라이선스가 없음(만료 후 동작은 F-12 소관, 여기서는 상태 전이만) | 트라이얼 만료를 나타내는 형태 `(추정)` | 메뉴 클릭 → 라이선스 입력/구매 유도(F-12). 기능 제한 범위는 F-12 소관 |
| **종료 중(Quitting)** | 사용자가 `Quit` 선택, 또는 시스템 로그아웃/재시작으로 인한 종료 신호 수신 | 아이콘이 사라지는 과정(순간적) | 없음 — 정리 완료 후 프로세스 종료 |

> 상태 간 전이는 원칙적으로 미초기화 → (권한대기 ↔ 정상동작 ↔ Unable to initialize) → 종료중 의 흐름을 따른다. 트라이얼 만료는 정상 동작 상태와 독립적인 축(라이선스 유효성)이며, 두 축이 동시에 참일 수 있다 — 정확한 우선순위 규칙(권한 문제와 트라이얼 만료가 동시에 발생하면 어느 쪽 아이콘을 우선 표시하는가)은 조사로 확정되지 않았다 `(추정)` → §9.

### 3.2 수명주기 이벤트 → 대응 동작

| 이벤트 | 구독 대상 | 무엇을 재초기화하는가 | 근거 |
| :--- | :--- | :--- | :--- |
| 절전에서 깨어남 | `NSWorkspace.didWakeNotification` | F-07 에 event tap 재활성화(또는 재설치) 요청 | superkey-inventory.md §2.1 v1.58 "key remapping was not working upon wake or login" |
| 로그인(사용자 세션 시작) | `NSWorkspace.sessionDidBecomeActiveNotification` (최초 로그인 경로) 또는 프로세스 최초 기동 시점 자체 | F-07 에 엔진 최초 기동 요청 | 위와 동일 — v1.58 은 "wake or login" 두 경로 모두를 실패 이력으로 명시 |
| 화면 잠금 해제 | `NSWorkspace.sessionDidBecomeActiveNotification`(잠금 해제 시에도 발화) | F-07 에 event tap 재활성화 요청 `(추정)` — wake 와 동일한 실패 계열로 간주 | v1.58 계열 실패를 예방적으로 넓게 적용한 것 `(추정)` → §9 |
| 사용자 전환(fast user switching) 복귀 | `NSWorkspace.sessionDidBecomeActiveNotification` | F-07 에 event tap 재활성화 요청 `(추정)` — 세션 전환도 잠금 해제와 동일한 계열로 간주 | §9 |
| 디스플레이 구성 변경(모니터 연결/해제, 해상도 변경) | `NSApplication.didChangeScreenParametersNotification` | F-03(오버레이 좌표계) 재계산을 요청 — event tap 자체는 영향 없음(F-10 이 F-07 에 보내는 신호는 없음) | superkey-inventory.md §2.1 v1.55 "Fixed broken Seek behavior on additional displays" — 다만 이는 F-03/F-02 의 좌표계 버그이지 event tap 생존 문제가 아니므로 F-10 은 이벤트 구독 지점만 제공하고 재계산 자체는 위임 |

이 표는 "언제 신호를 보내는가"만 정의한다. `CGEventTap` 이 실제로 `kCGEventTapDisabledByTimeout`/`kCGEventTapDisabledByUserInput` 상태인지 판별하고 `CGEventTapEnable` 을 호출하는 로직은 F-07 소관이다(rust-macos-capability-notes.md §2.1).

### 3.3 메뉴 항목 구성 (전체 `(추정)`)

조사에서 메뉴 항목 구성 자체는 확인되지 않았다(superkey-inventory.md §7 Q15). 아래는 다른 기능 명세가 요구하는 진입점을 근거로 제안하는 추정 구성이며, 실제 순서·라벨·아이콘은 앱 설치 후 확인이 필요하다.

| 항목(추정 라벨) | 아이콘 | 동작 | 활성 조건 | 근거 |
| :--- | :--- | :--- | :--- | :--- |
| 상태 표시줄 (비클릭) `(추정)` | 없음 또는 상태 아이콘 | 없음(정보 표시 전용) | `Unable to initialize Superkey` 또는 트라이얼 만료 상태에서만 표시 `(추정)` | superkey-inventory.md §1.3 고정 문구 |
| `Seek` `(추정)` | Seek 아이콘 `(추정)` | Seek 세션 즉시 실행 (F-01 트리거) | 정상 동작 상태에서만 활성 | F-01(seek-activation-and-session.md)이 요구하는 진입점 — 전역 단축키·리매핑 키 외의 3번째 활성화 경로로 자연스러움 |
| `Preferences…` `(추정)` | 톱니바퀴 아이콘 `(추정)` | 환경설정 창 열기(F-09) | 모든 상태에서 활성(권한 대기·오류 상태에서도 설정 확인 필요) | F-09(환경설정 창)가 진입점을 요구함 |
| `Check for Updates…` `(추정)` | 업데이트 아이콘 `(추정)` | 업데이트 확인 트리거(F-13) | 정상 동작 상태에서 활성 | F-13(업데이트 확인)이 요구하는 진입점 |
| `License…` `(추정)` | 라이선스/열쇠 아이콘 `(추정)` | 라이선스 상태·입력 창 열기(F-12) | 모든 상태에서 활성 | F-12(라이선스 상태 표시)가 요구하는 진입점 |
| 구분선 | — | — | — | 표준 메뉴 관례 `(추정)` |
| `Quit Superkey` `(추정)` | 종료 아이콘 `(추정)` | 앱 정상 종료(§2 시나리오 F, §3.1 종료중 상태) | 모든 상태에서 항상 활성 | 모든 메뉴바 앱의 표준 최소 요구사항 |

v1.62 의 "menu item icons" 변경이 위 표의 모든 항목에 아이콘을 붙이는 것인지, 일부(예: 상태를 나타내는 항목만)에만 붙이는 것인지는 확인되지 않았다 `(추정)` → §9.

## 4. 설정 항목

메뉴바·수명주기와 직접 관련된 설정은 스크린샷이 공개되지 않은 `General` 탭 소속으로 추정된다(superkey-inventory.md §3.4: "`General` 탭 ... ❓미확인"). 아래는 모두 `(추정)` 이다.

| 이름(추정 라벨) | 타입 | 기본값 | 유효 범위 | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| `Launch at login` `(추정)` | 체크박스 | `(추정)` — 통상 이런 유틸리티는 기본 ON 이 흔하나 확인 불가 | ☑ / ☐ | superkey-inventory.md §3.4, §7 Q2 |
| `Show icon in menu bar` `(추정)` | 체크박스 | `(추정)` — 기본 ☑ 로 추정(메뉴바가 유일한 접점이므로 끄면 접근 불가 위험, §5 항목 2) | ☑ / ☐ | 동일 |
| `Reset preferences` `(추정)` | 버튼 | 해당 없음(동작) | — | superkey-inventory.md §3.4 (General 탭 추정 항목 나열 중 "환경설정 초기화") |
| `Check for updates automatically` `(추정)` | 체크박스 | `(추정)` — 세부 로직은 F-13 소관, F-10 은 존재 여부만 언급 | ☑ / ☐ | superkey-inventory.md §3.4, F-13 참조 |

> 이 절의 항목들은 F-09(환경설정 창 UI)가 실제 렌더링을 소유한다. F-10 은 이 값들이 앱 수명주기(자동 실행, 메뉴바 표시)에 미치는 영향만 다룬다.

## 5. 엣지 케이스와 실패 모드

1. **이중 실행.** 두 프로세스가 동시에 뜨면 각자 `CGEventTap` 을 설치하려 시도해 키 이벤트가 두 번 처리(리매핑이 두 번 적용되거나 이중 클릭 등)된다. §3.2 의 단일 인스턴스 보장으로 두 번째 프로세스가 즉시 종료돼야 한다(§6, §7). 종료 대신 기존 인스턴스를 활성화(메뉴 열기)하는 UX 가 더 나을 수 있으나 조사로 확정되지 않았다 `(추정)` → §9.
2. **메뉴바 공간 부족(노치 Mac, 아이콘 과다).** macOS 가 공간이 부족하면 상태 아이템을 자동으로 숨긴다. 이 경우 사용자가 앱에 접근할 유일한 통로가 사라진다 — `Show icon in menu bar` 를 끄지 못하게 하거나, 최소한 숨겨졌을 때의 복구 경로(예: Dock 아이콘 임시 표시, 전역 단축키로 메뉴 강제 노출)를 고려해야 한다.
3. **로그인 항목 등록 실패.** `SMAppService.register()` 는 사용자가 시스템 설정에서 명시적으로 거부하거나 관리형 기기 정책으로 차단된 경우 실패할 수 있다. 실패 시 `Launch at login` 체크박스는 UI 상 켜진 것처럼 보이나 실제로는 등록되지 않은 상태가 될 수 있다 — 등록 결과를 확인하고 실패 시 사용자에게 알리는 처리가 필요하다.
4. **macOS 12 에서 `SMAppService` 부재.** `SMAppService` 는 macOS 13 이상에서만 존재한다(rust-macos-capability-notes.md §2.9). 최소 지원 버전이 12.0(superkey-inventory.md §1.5, appcast `minimumSystemVersion=12.0`)이므로, macOS 12 런타임에서 `SMAppService` API 를 호출하면 링크 타임이 아닌 **런타임**에 실패한다. OS 버전 분기가 반드시 필요하다(§6, §7).
5. **종료 시 modifier stuck.** 사용자가 hyper/meh/bleh 키를 누른 채로 강제 종료(force quit)하거나 시스템이 크래시하면, `CGEventTap` 이 사라진 뒤에도 OS 레벨에서 modifier 가 눌린 것으로 남을 수 있다. 정상 종료 경로(§3.1 종료중 상태)에서는 modifier 해소를 시도할 수 있지만, 강제 종료·크래시 경로에서는 F-10 이 개입할 기회 자체가 없다 — 다음 실행 시점에 잔여 상태를 점검·초기화하는 것이 유일한 보완책이다 `(추정)`.
6. **절전 복귀 후 탭 사망.** v1.58 이 실제로 겪은 회귀다(§3.2). `didWakeNotification` 수신과 F-07 재초기화 요청 사이에 타이밍 문제(알림이 너무 이르게 와서 아직 시스템이 완전히 깨어나지 않은 상태)가 있을 수 있어 재시도·지연 로직이 필요할 수 있다 `(추정)`.
7. **사용자 전환(fast user switching).** 다른 사용자 세션으로 전환되면 원래 사용자의 앱은 백그라운드로 밀려나지만 프로세스는 계속 존재한다. `CGEventTap` 이 전환된 세션의 키 입력까지 받아버리면 안 되고, 원래 세션으로 복귀했을 때만 정상 동작해야 한다 — 세션 경계를 넘는 이벤트 탭의 동작은 조사로 확정되지 않았다 `(추정)` → §9.
8. **권한이 런타임에 취소됨.** 앱이 정상 동작 중에 사용자가 시스템 설정에서 Accessibility 권한을 끄면, 진행 중이던 `CGEventTap` 은 즉시 무효화될 수 있다. 이 경우 앱은 정상 동작 상태에서 곧바로 권한 대기 또는 `Unable to initialize Superkey` 상태로 전이해야 하며, 이를 감지하는 방법(폴링 vs 이벤트 기반)은 F-11 과 조율이 필요하다.
9. **앱이 `/Applications` 밖에서 실행됨(TCC 경로 변경).** 앱을 `/Applications` 가 아닌 다른 경로(예: `~/Downloads`)에서 직접 실행하면 TCC 는 그 경로의 바이너리를 별개의 신원으로 취급할 수 있다. 이전에 부여된 권한이 적용되지 않아 `Unable to initialize Superkey` 상태로 직행할 수 있다(rust-macos-capability-notes.md §3.2 의 `csreq`/서명 논의와 같은 계열). 이상적으로는 앱이 실행 위치를 감지해 `/Applications` 로 이동을 권유해야 한다 `(추정)`.
10. **업데이트 후 재시작.** Sparkle(또는 대체 업데이터)이 업데이트를 설치한 뒤 앱을 재시작시키면, 이는 사실상 §2 시나리오 F(정상 종료)와 시나리오 A/B(재기동) 가 연쇄로 일어나는 것이다. 재시작 직후 event tap 재설치, 권한 재확인, 메뉴바 아이콘 재표시가 새 바이너리 기준으로 다시 이뤄져야 한다. 업데이트 자체의 다운로드·적용 로직은 F-13 소관이나, 재시작 후 F-10 의 상태 머신이 미초기화부터 다시 시작한다는 점은 F-10 의 책임이다.
11. **크래시 후 재기동.** 프로세스가 예기치 않게 종료된 경우(크래시), 사용자가 수동으로 재실행하기 전까지 리매핑 기능 전체가 조용히 죽어 있다. 자동 재기동(예: `launchd` `KeepAlive`)을 걸지, 조용히 죽게 둘지는 조사로 확정되지 않았다 `(추정)` → §9. 자동 재기동을 건다면 짧은 시간 내 반복 크래시(크래시 루프)를 감지해 무한 재시작을 막는 안전장치가 필요하다.
12. **트라이얼 만료 시점에 앱이 실행 중.** 앱이 정상 동작 상태로 떠 있는 도중 20일 체험 기간의 만료 시각을 넘기는 경우, 재시작 없이도 상태 머신이 정상 동작에서 트라이얼 만료로 전이해야 한다. 이 감지를 폴링(예: 매시 정각 확인)으로 할지, 다음 실행 시점에만 확인할지는 조사로 확정되지 않았다 `(추정)` → §9(만료 이후 정확한 기능 제한 범위 자체는 F-12 소관).

## 6. 필요한 플랫폼 API

- **`NSApplicationActivationPolicyAccessory` / `LSUIElement`** — Dock 아이콘 제거, 앱 스위처 미노출. Tauri 대응: `app.set_activation_policy(ActivationPolicy::Accessory)`(rust-macos-capability-notes.md §2.6 "Dock 아이콘 없음" 행).
- **`NSStatusItem`** — 메뉴바 아이콘·메뉴. `tray-icon` 0.24.2 가 macOS 에서 `ns_status_item() -> Option<Retained<NSStatusItem>>` 로 직접 접근을 제공한다(rust-macos-capability-notes.md §2.7). 메뉴 항목별 아이콘(`NSMenuItem.image`)이 `tray-icon` 고수준 API 로 되는지는 미확인이며(rust-macos-capability-notes.md §4 P4), 필요 시 `ns_status_item()` 이 반환하는 `NSStatusItem` 을 통해 `objc2-app-kit` 0.3.2 로 `NSMenu`/`NSMenuItem` 을 직접 조작해야 할 수 있다.
- **`SMAppService.mainApp`(macOS 13+)** — 로그인 시 실행 등록. `smappservice-rs` 0.1.3 또는 `objc2-service-management` 0.3.2(rust-macos-capability-notes.md §1.2, §2.9). **macOS 13+ 전용**임이 크레이트 설명 자체에 명시되어 있다.
- **LaunchAgent plist(macOS 12 폴백)** — `SMAppService` 부재 시 대안. `auto-launch` 0.6.0 의 AppleScript/LaunchAgent 모드로 커버 가능(rust-macos-capability-notes.md §1.2, §2.9).
- **`NSWorkspace` 알림** — `didWakeNotification`(절전 복귀), `sessionDidBecomeActiveNotification`(로그인·잠금해제·사용자전환 복귀), `screensDidWakeNotification`(디스플레이 절전 복귀). `objc2-app-kit` 0.3.2 를 통해 구독(§3.2). 이 알림들에 대한 전용 상위 크레이트는 조사에서 확인되지 않았다 — `objc2-app-kit` 원시 바인딩 사용이 기본 경로다.
- **`NSApplication.didChangeScreenParametersNotification`** — 디스플레이 구성 변경 감지. `objc2-app-kit` 0.3.2.
- **단일 인스턴스 감지** — `NSWorkspace.shared.runningApplications` 를 번들 식별자로 필터링해 기존 인스턴스 존재 여부를 확인하는 방식이 `objc2-app-kit` 0.3.2 만으로 가능하다(추가 크레이트 불필요, §7). Tauri 생태계의 전용 단일 인스턴스 플러그인은 rust-macos-capability-notes.md 의 크레이트 실사 표에 없어 버전을 확정할 수 없다 → §9.
- **종료 시 정리 신호** — event tap 해제·modifier 해소는 F-07 이 노출하는 인터페이스를 F-10 이 호출하는 것으로, 이 문서 범위에서는 API 가 아니라 **호출 시점**(§3.1 종료중 상태 진입)만 정의한다.
- **`AXIsProcessTrusted()` 등 TCC 확인** — `Unable to initialize Superkey` 진입 판정에 필요한 권한 상태 조회는 F-11 이 제공하는 것을 F-10 이 소비한다(rust-macos-capability-notes.md §2.5). F-10 자체가 TCC API 를 직접 호출하지는 않는다.

## 7. 구현 접근

**판정: Rust 바인딩.**

F-10 이 다루는 기능(활성화 정책 설정, `NSStatusItem` 생성과 메뉴 구성, 로그인 항목 등록, `NSWorkspace` 알림 구독, 실행 중인 앱 목록 조회를 통한 단일 인스턴스 판정, 상태 머신 자체의 로직)은 전부 rust-macos-capability-notes.md 가 실사한 크레이트로 커버된다.

- `app.set_activation_policy(ActivationPolicy::Accessory)` — Tauri v2 API 자체가 제공(rust-macos-capability-notes.md §2.6).
- `tray-icon` 0.24.2 — `NSStatusItem` 생성·기본 메뉴 구성, `ns_status_item()` 으로 필요 시 AppKit 직접 접근(rust-macos-capability-notes.md §1.2, §2.7).
- `smappservice-rs` 0.1.3(macOS 13+) / `auto-launch` 0.6.0(macOS 12 폴백) — 로그인 시 실행. 두 크레이트 모두 필요하며 **OS 버전 분기가 필수**다(rust-macos-capability-notes.md §1.2, §2.9).
- `objc2-app-kit` 0.3.2 — `NSWorkspace` 알림 구독, `NSMenuItem` 아이콘 직접 설정(필요 시), `runningApplications` 조회를 통한 단일 인스턴스 판정. `objc2-service-management` 0.3.2 도 `SMAppService` 의 대안 경로로 쓸 수 있다.

네이티브 Swift/Objective-C shim 을 별도로 작성할 필요가 **없다** — 상태 머신 로직은 순수 애플리케이션 로직이고, 플랫폼 접점(활성화 정책, 상태 아이템, 로그인 항목, 수명주기 알림)은 모두 위 크레이트들이 노출하는 API 표면 안에 있다.

- **기각한 대안 1 — "순수 Rust" 판정.** 상태 머신 자체는 플랫폼 비의존적으로 보이지만, F-10 의 모든 진입 조건(권한 상태, 절전/잠금 이벤트, 로그인 항목 등록 성공 여부)이 AppKit/ServiceManagement 타입에 직접 묶여 있어, "순수 Rust" 로 분류하면 어디서부터 바인딩이 필요한지가 모호해진다.
- **기각한 대안 2 — "네이티브 shim 불가피".** 메뉴 항목별 아이콘(P4, §6)이 `tray-icon` 고수준 API 로 되는지는 미확인이지만, `ns_status_item()` 이 `NSStatusItem`(따라서 그 `NSMenu`)에 대한 직접 접근을 제공하므로 `objc2-app-kit` 으로 `NSMenuItem.image` 를 설정하는 경로가 이미 열려 있다(rust-macos-capability-notes.md §2.7: "메뉴 항목마다 아이콘을 넣는 것은 NSMenuItem.image 직접 설정이 필요할 수 있다"). 즉 API 커버리지 공백이 아니라 **어느 경로(고수준 vs 저수준)를 쓰느냐의 확인 필요 사항**이다 → §9. Swift/Objective-C 로 별도 프로세스나 프레임워크를 작성해야 하는 진짜 공백은 발견되지 않았다.
- **기각한 대안 3 — Tauri 전용 단일 인스턴스 플러그인 채택.** Tauri 커뮤니티에는 단일 인스턴스 플러그인이 존재하는 것으로 알려져 있으나, rust-macos-capability-notes.md 의 크레이트 실사 표(§1)에 포함되지 않아 버전을 검증할 수 없었다. 검증된 `objc2-app-kit` 0.3.2 의 `runningApplications` 조회만으로 동일한 목적을 달성할 수 있으므로, 미검증 크레이트 대신 이 경로를 채택한다 → 정확한 크레이트 선정은 §9 로 승계.

## 8. 수용 기준

- [ ] 앱 실행 시 Dock 아이콘이 표시되지 않고, `⌘Tab` 앱 스위처 목록에도 나타나지 않는다.
- [ ] 정상 동작 상태에서 메뉴바에 아이콘이 표시되고, 클릭하면 메뉴가 펼쳐진다.
- [ ] 이미 인스턴스가 실행 중인 상태에서 앱을 다시 실행하면, 두 번째 프로세스는 새 `CGEventTap` 을 설치하지 않고 종료(또는 기존 인스턴스로 위임)한다.
- [ ] `Launch at login` 을 켠 상태로 재부팅하면, macOS 13 이상과 macOS 12 양쪽 모두에서 로그인 후 앱이 자동 실행된다.
- [ ] macOS 12 에서 `SMAppService` API 를 호출하지 않고 LaunchAgent 경로로만 로그인 항목이 등록된다(런타임 크래시 없음).
- [ ] 절전에서 깨어난 직후 리매핑이 정상 동작한다(수동 재실행 없이).
- [ ] 화면 잠금을 해제한 직후 리매핑이 정상 동작한다.
- [ ] 사용자 전환으로 세션을 벗어났다가 원래 세션으로 복귀하면 리매핑이 정상 동작하고, 벗어나 있는 동안에는 다른 사용자의 키 입력을 처리하지 않는다.
- [ ] 메뉴에서 `Quit` 을 선택하면, 그 시점에 합성 상태로 눌려 있던 modifier 가 모두 해소된 후 프로세스가 종료된다.
- [ ] 메뉴에서 `Quit` 을 선택하면, event tap 해제와 열려 있던 오버레이 창 파괴가 모두 완료된 후 프로세스가 종료된다.
- [ ] 권한이 시스템 설정상 부여된 것처럼 보이나 TCC DB 와 어긋난 상태에서 앱을 실행하면, 상태 머신이 `Unable to initialize Superkey` 로 전이하고 그 고정 문구가 원문 그대로(번역·의역 없이) 표시된다.
- [ ] `Unable to initialize Superkey` 상태에서 F-11 의 복구 절차를 완료하고 재실행하면, 상태 머신이 미초기화부터 다시 시작해 정상 동작에 도달한다.
- [ ] 트라이얼 만료 시각을 넘기면 상태 머신이 정상 동작에서 트라이얼 만료 상태로 전이한다(재실행 여부와 무관하게, 또는 최소한 다음 실행 시점에는 반드시).

## 9. 미해결 질문

| # | 질문 | 조사 문서 연결 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | 메뉴바 메뉴의 실제 항목 구성·순서·라벨(§3.3 은 전부 `(추정)`) | superkey-inventory.md §7 Q15 | 앱 설치 후 확인 |
| 2 | v1.62 "menu item icons" 가 모든 항목에 붙는지, 일부에만 붙는지 | superkey-inventory.md §2.1 v1.62 | 앱 설치 후 확인 |
| 3 | `General` 탭의 `Launch at login` / `Show icon in menu bar` 등 실제 라벨과 기본값 | superkey-inventory.md §3.4, §7 Q2 | 앱 설치 후 확인, `defaults read com.knollsoft.Superkey` |
| 4 | 이중 실행 시 두 번째 프로세스가 조용히 종료되는지, 기존 인스턴스의 메뉴를 여는지 | 조사 자료에 근거 없음 | 앱 설치 후 이중 실행 재현 |
| 5 | 화면 잠금 해제·사용자 전환도 v1.58 계열 실패(§3.2 표의 "추정" 행)에 해당하는지, 아니면 wake 만 해당하는지 | superkey-inventory.md §2.1 v1.58 은 "wake or login" 만 명시 | 실기 재현 테스트 |
| 6 | 세션 전환 중(다른 사용자로 전환된 동안) `CGEventTap` 이 실제로 이벤트를 계속 받는지 여부 | 조사 자료에 근거 없음 | 실기 재현 테스트, Apple 문서 확인 |
| 7 | 크래시 후 자동 재기동 정책(launchd KeepAlive 여부, 크래시 루프 방지 임계값) | 조사 자료에 근거 없음 | 개발자 문의 또는 정책 자체 설계 결정 |
| 8 | 트라이얼 만료 감지를 폴링으로 할지, 다음 실행 시점에만 확인할지 | superkey-inventory.md §7 Q12(체험 기산점·만료 후 동작)와 연결되나 F-10 관점의 감지 시점은 별도 미확인 | 개발자 문의 또는 정책 자체 설계 결정 |
| 9 | 권한 대기와 트라이얼 만료가 동시에 참일 때 메뉴바 아이콘·메뉴가 어느 것을 우선하는지 | 조사 자료에 근거 없음 | 실기 재현 테스트(원본 앱에서 재현 가능하다면) |
| 10 | 메뉴 항목별 아이콘을 `tray-icon` 고수준 API 로 설정할 수 있는지, `objc2-app-kit` 저수준 경로가 필요한지 | rust-macos-capability-notes.md §4 P4 | `tray-icon` 0.24.2 소스·문서 확인, 실측 |
| 11 | 단일 인스턴스 감지에 쓸 정확한 구현(Tauri 커뮤니티 플러그인 vs `objc2-app-kit` `runningApplications` 직접 조회)과 그 크레이트의 검증된 버전 | rust-macos-capability-notes.md §1 크레이트 표에 전용 플러그인 없음 | crates.io 재조회, 실측 |
| 12 | `/Applications` 밖에서 실행된 경우를 앱이 능동적으로 감지해 안내할지 여부 | rust-macos-capability-notes.md §3.2 (TCC 는 경로가 아니라 서명·번들ID 로 키잉되지만 실무에서 경로 이동을 권장하는 관례가 흔함) | 정책 설계 결정 |
