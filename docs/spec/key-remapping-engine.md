# F-07 · 키 리매핑 엔진 (공통 기반)

> **한 줄 요약** — Seek(F-01)·Hyperkey/meh/bleh(F-05)·Power User Presets(F-08) 가 하나의 물리 키 이벤트 스트림을 공유하는 **단일 `CGEventTap` 기반 중재(arbitration) 엔진**. 기능별로 독립된 이벤트 탭을 설치하는 설계는 명시적으로 기각한다.
> **의존성** — Accessibility + Input Monitoring 권한(상세는 F-11)이 승인되어 있지 않으면 `CGEventTapCreate` 자체가 유효한 탭을 반환하지 않는다. 이 문서는 "권한이 있다"는 전제 위에서 엔진 내부 동작만 다룬다.
> **이 엔진을 소비하는 명세** — F-01(Seek 트리거의 키 리매핑 진입점), F-05(Hyperkey / meh / bleh), F-08(Power User Presets 전 항목). 세 명세 모두 이 문서가 정의하는 규칙 테이블 위에 자신의 "규칙"만 등록한다.

---

## 1. 개요

SuperKey 는 겉보기엔 세 가지 기능(Seek, Hyperkey, Power User Presets)이지만, 조사에서 확인된 실제 버그 두 건은 이들이 **UI 상으로만 분리되어 있을 뿐, 물리적으로는 정확히 같은 자원(키보드 이벤트 스트림, 특히 caps lock 같은 소수의 "쓸모없는" 키)을 두고 경쟁한다**는 사실을 증명한다.

- **v1.20** — "Fixed a bug where caps lock + keys being mapped to arrow keys wasn't working when caps lock was remapped as the hyper key." caps lock 이 Hyperkey 소스 키로 지정된 상태에서 Presets 의 "Caps lock + WASD = 화살표" 조합이 깨졌다. Hyperkey 기능과 Presets 기능이 caps lock 의 down/up 상태를 **각자 독립적으로** 추적했기 때문에 발생 가능한 부류의 버그다.
- **v1.62** — "\"Shift + caps lock = caps lock\" will no longer trigger a quick press caps lock keypress." 두 Presets 항목이 같은 caps lock 키 이벤트를 각자 해석하다 충돌했다.

이 문서는 이 두 버그가 **다시는 발생할 수 없는 구조**를 정의하는 것이 목적이다. 핵심 설계 원칙은 다음 하나로 요약된다.

> **물리 키 하나의 현재 상태(눌림 여부, 눌린 지속 시간, 동시에 눌린 다른 키)는 시스템 전체에서 단 하나의 "정본(canonical) 상태"로만 존재한다.** Seek/Hyperkey/Presets 는 이 정본 상태를 **읽기만** 하며, 각자 별도의 이벤트 탭이나 별도의 keyDown/keyUp 추적 로직을 갖지 않는다. "이 이벤트를 최종적으로 어떻게 바꿀 것인가"는 이 문서가 정의하는 단일 중재 우선순위 표(§3-b)가 매 이벤트마다 한 번만 결정한다.

엔진은 세 가지 책임을 진다.

1. **이벤트 탭 인프라** — 하나의 `CGEventTap` 을 열고, 죽지 않게 유지하고, 콜백 안에서 안전하게 판단을 내린다.
2. **중재** — 여러 규칙이 동시에 해당하는 물리 키 이벤트에 대해 결정론적으로 하나의 결과만 낸다.
3. **판정 원시 자료 제공** — quick press/hold 판정, 물리 키코드 기반 매칭, 레이아웃 독립적 문자 출력, Secure Input 인지라는 네 가지 "저수준 서비스"를 F-01/F-05/F-08 이 공통으로 가져다 쓸 수 있게 만든다.

---

## 2. 사용자 시나리오

이 엔진 자체는 사용자에게 직접 노출되는 UI 가 없다(설정 항목은 §4 참조, 대부분 F-05/F-08 소유). 사용자 시나리오는 모두 "엔진이 없다면 깨졌을 상호작용"의 형태를 띤다.

1. **caps lock 을 hyper 키로 쓰면서 동시에 Presets 의 방향키 프리셋도 쓰는 사용자** (v1.20 재현 시나리오)
   사용자는 Hyperkey 탭에서 `Remap key to hyper key: caps lock` 을 켜고, Presets 탭에서 `Caps lock + W A S D = ▲◀▼▶` 도 켠다. caps lock 을 누른 채로 W 를 누르면 — 엔진은 caps lock 이 "hold 로 확정된 hyper 소스 키" 이면서 동시에 "WASD 프리셋의 트리거 키"이기도 하다는 것을 하나의 상태로 인지하고, W 의 keyDown 이 도착하는 순간 우선순위 표에 따라 **화살표 키로 치환된 이벤트**를 낸다. 사용자는 어느 쪽 설정이 "이겼는지" 신경 쓸 필요가 없다 — 결과가 항상 예측 가능하다.

2. **`Shift + caps lock = caps lock` 과 `Quick press caps lock to execute` 를 동시에 켠 사용자** (v1.62 재현 시나리오)
   사용자가 shift 를 누른 채 caps lock 을 살짝 눌렀다 뗀다. 의도는 "shift+caps lock 조합"이지 "caps lock 단독 quick press"가 아니다. 엔진은 caps lock 의 quick press 상태 머신이 `PendingDown` 상태에 있는 동안 **다른 키(shift)가 이미 눌려 있었다는 사실**을 판정에 반영해, quick press 조건 자체가 성립하지 않도록 만든다(§3-c).

3. **개발자 권장 설정을 그대로 따라 하는 사용자**
   Caps Lock → Seek(hold 시 표시), `left shift + right shift = caps lock` 를 함께 켠 사용자가 Seek 세션을 여는 도중 실수로 shift 두 개를 동시에 누른다. Seek 세션이 활성 중이므로, 엔진은 shift 콤보 프리셋보다 **Seek 의 입력 독점을 우선**해 caps lock 합성을 만들지 않는다(§3-b 계층 1).

4. **절전에서 깨어난 사용자** (v1.58 재현 시나리오)
   맥북을 열자마자 caps lock 을 눌러 hyper 단축키를 쓰려 했지만 반응이 없다. 엔진은 `NSWorkspaceDidWakeNotification` 수신 시 탭 상태를 점검하고, 죽어 있으면 재설치한다 — 사용자는 이 과정을 인지하지 못하고 그냥 "항상 동작한다"고 느껴야 한다.

5. **암호 입력 필드에 포커스를 둔 사용자**
   Secure Input 이 활성화된 창(예: 로그인 다이얼로그)에 포커스가 가 있는 동안 caps lock 을 눌러도 hyper 조합이 발동하지 않는다. 이는 macOS 의 의도된 보안 제약이며, 엔진은 이를 우회하지 않고 원본 이벤트를 그대로 통과시킨다.

---

## 3. 동작 명세

### 3-a. 이벤트 탭 생명주기 상태표

| 상태 | 진입 조건 | 엔진 동작 | 다음 상태로의 전이 | 비고 |
| :--- | :--- | :--- | :--- | :--- |
| `NotInstalled` | 앱 시작 직후, 또는 권한 상실 확정 후 | 탭 없음. 모든 키 이벤트는 macOS 기본 동작 그대로 | Accessibility+Input Monitoring 권한 확인 성공 → `Installing` | F-11 권한 상태와 1:1 대응 |
| `Installing` | 권한 확인 통과, `CGEventTapCreate` 호출 시도 중 | `kCGSessionEventTap` / `kCGHeadInsertEventTap` / `kCGEventTapOptionDefault` 로 생성 시도 | 성공(`NULL` 아닌 탭 반환) → `Active` / 실패 → `NotInstalled` (재시도는 지수 백오프) | `Default` 옵션이어야 이벤트 소비·치환 가능 — `ListenOnly` 는 관찰만 가능해 이 엔진의 목적과 맞지 않으므로 채택하지 않는다 |
| `Active` | 탭 생성 성공, `CFRunLoopSource` 가 런루프에 등록되어 콜백이 실제로 호출됨 | keyDown/keyUp/flagsChanged/(설정된 마우스 이벤트) 콜백마다 §3-b 중재 규칙 실행 | 콜백이 `kCGEventTapDisabledByTimeout` 또는 `kCGEventTapDisabledByUserInput` 을 이벤트 타입으로 받음 → `Disabled` / 권한이 런타임에 취소됨(§5) → `NotInstalled` | 정상 상태. 대부분의 시간을 여기서 보낸다 |
| `Disabled` | 콜백 자신이 타임아웃 또는 사용자 개입으로 탭이 꺼졌음을 통지받음 | 즉시 `CGEventTapEnable(tap, true)` 호출로 재활성화 시도 | 재활성화 성공 → `Active` (같은 프레임에서 즉시 복귀 시도, 실패 시 짧은 지수 백오프로 재시도) | v1.58 "key remapping was not working upon wake or login" 이 이 상태에서 재활성화 로직이 없었을 때 발생하는 실패 모드. `kCGEventTapDisabledByTimeout` 은 콜백이 너무 오래 걸렸다는 신호이기도 하므로, 재발 방지를 위해 §3-a 하단 "콜백 금지 사항"을 반드시 지킨다 |
| `SuspendedBySleepOrLock` | `NSWorkspaceWillSleepNotification` / 화면 잠금(`com.apple.screenIsLocked` distributed notification) 수신 | 탭 자체는 유지하되, 깨어난 직후 한 차례 `CGEventTapEnable` 로 살아있는지 강제 확인(taps 는 절전 중 죽는 경우가 실측으로 확인되어 왔음 — v1.58 원인 추정 범주) | `NSWorkspaceDidWakeNotification` / 화면 잠금 해제(`com.apple.screenIsUnlocked`) 수신 → 탭 상태 재확인 후 `Active` 또는 `Installing` (탭이 완전히 무효화되어 있으면 재생성) | 로그인 시에도 동일 절차 — `NSWorkspaceSessionDidBecomeActiveNotification` 수신 시 탭 재확인 |
| `Terminated` | 앱 종료, 또는 사용자가 시스템 설정에서 Accessibility 권한을 명시적으로 회수 | `CFRunLoopSourceInvalidate` + 탭 참조 해제 | 앱 재시작 또는 권한 재승인 시 `NotInstalled` 부터 재시작 | — |

**콜백 안에서 해서는 안 되는 일**(모든 상태에 공통 적용): 블로킹 I/O(파일, 네트워크), AX 트리 순회, OCR, 뮤텍스 경합 가능성이 있는 잠금 획득, 로그 파일 동기 쓰기. macOS 는 이벤트 탭 콜백이 일정 시간(경험적으로 수백 ms 수준, Apple 은 정확한 값을 공개하지 않음) 안에 리턴하지 않으면 해당 탭을 `kCGEventTapDisabledByTimeout` 으로 강제 비활성화한다. 이는 §3-a `Disabled` 전이의 가장 흔한 원인이며, v1.58 부류 버그의 재발 방지는 "재활성화 로직"과 "애초에 타임아웃을 유발하지 않는 콜백 설계" 양쪽 모두를 요구한다. 무거운 작업이 필요한 판정(예: Seek 세션 시작)은 콜백 안에서는 **가벼운 상태 플래그 읽기/쓰기만** 수행하고, 실제 무거운 작업은 채널을 통해 다른 스레드로 위임한다.

### 3-b. 중재(arbitration) 우선순위 표 ⭐

매 keyDown/keyUp/flagsChanged 이벤트는 도착 즉시 아래 계층을 **위에서부터 순서대로** 평가하며, 처음으로 조건이 성립하는 계층에서 멈춘다. 낮은 계층의 규칙은 평가조차 되지 않는다(short-circuit). 단, 계층 판정에 필요한 "이 키가 지금 눌려 있는가" 류의 원시 상태는 계층과 무관하게 **하나의 공유 상태 테이블**(물리 키코드 → 눌림 여부/눌린 시각/quick-press 상태 머신 상태)에서 읽는다 — 이 공유가 v1.20 류의 버그를 구조적으로 막는 지점이다(하단 설명 참조).

| 계층 | 조건 | 이벤트 소비 여부 | 예시 |
| :--- | :--- | :--- | :--- |
| **1. Seek 세션 활성** | F-01 의 Seek 세션이 열려 있음(공유 상태의 전역 플래그 1개로 판정, AX/OCR 호출 없이 O(1) 확인) | **소비.** 이후 계층은 전혀 평가하지 않고 이벤트를 Seek 세션 핸들러로 라우팅 | Seek 검색 바가 열린 상태에서 임의의 문자 키, 화살표, Enter, `;` 는 전부 Seek 이 처리. `caps lock + WASD` 프리셋조차 Seek 세션 중에는 개입하지 않는다 |
| **2. Hyperkey/meh/bleh 소스 키의 modifier 상태** | 이벤트의 소스 키가 F-05 에 hyper/meh/bleh 소스로 등록되어 있고, §3-c quick press 상태 머신이 그 키를 `HoldConfirmed` 로 판정한 상태 | **소비.** 원본 keyDown/keyUp 은 방출하지 않고, 합성 `flagsChanged`(⌃⌥⌘⇧ 등)를 방출. 설정에 따라 마우스 이벤트에도 동일 modifier 를 얹음 | caps lock 이 hyper 소스로 등록된 상태에서 caps lock 을 누른 채 유지하면 ⌃⌥⌘⇧ 가 합성되어 다른 앱의 단축키에 그대로 전달됨 |
| **3. Preset 조합** | 이벤트가 F-08 에 등록된 다중 키 조합 조건(트리거 키 + 동시 눌림 상태)을 만족 | **소비.** 조합의 결과 이벤트(예: 화살표 키, forward delete, `⌘⌥⇧V`)로 치환 | `Caps lock + W A S D = ▲◀▼▶`, `Hyper + delete = forward delete`, `Left shift + right shift = caps lock` |
| **4. 단순 리매핑** | 이벤트의 소스 키가 F-08/F-05 에 1:1 리매핑으로 등록됨(조합 조건 없음) | **소비.** 치환된 단일 키 이벤트를 방출 | `Remap caps lock to: left control`, `Remap delete to forward delete` |
| **5. 통과(passthrough)** | 위 어느 계층에도 해당하지 않음 | 소비하지 않음. 원본 이벤트를 그대로 반환 | 등록되지 않은 임의의 키 입력 |

**계층 2 와 계층 3 이 "충돌"이 아니라 "합성"으로 작동해야 하는 이유(v1.20 예방).** 계층 순서만 보면 "hyper 가 항상 preset 을 이긴다"로 오해하기 쉽지만, 그렇지 않다. 계층 2 는 **"이 물리 키 자체가 다음 키 이벤트에 어떤 modifier 를 얹을 것인가"** 만 결정하고, 계층 3 은 **그렇게 만들어진 최신 modifier 상태를 포함해** 자신의 조합 조건을 평가한다. 즉 caps lock 이 hyper 소스이면서 동시에 WASD 프리셋의 트리거 키인 구성에서, "caps lock 을 누르고 있다"는 사실은 하나의 공유 상태로만 존재하고, W 의 keyDown 이 도착하는 순간 엔진은 (a) caps lock 이 hold-confirmed 상태다 → (b) WASD 프리셋 조건(트리거 키 held + W)도 동시에 성립한다는 것을 **같은 판정 패스 안에서** 본다. v1.20 버그는 이 두 사실이 서로 다른 독립 추적 경로에 있어 하나가 다른 하나를 보지 못했을 때 발생한다. 이 엔진은 단일 상태 테이블을 강제함으로써 이 클래스의 버그 자체가 발생할 수 없게 만든다.

**v1.62 예방.** `Shift + caps lock = caps lock`(계층 3, 조합 프리셋)과 `Quick press caps lock to execute`(계층 3, quick-press 프리셋)는 같은 계층 안에서 같은 소스 키(caps lock)를 두고 경쟁하는 두 규칙이다. 이 충돌은 §3-c 상태 머신의 "다른 키 입력 = 홀드로 확정" 전이가 해소한다: shift 가 이미 눌려 있는 상태에서 caps lock 의 keyDown 이 도착하면, caps lock 의 quick-press 상태 머신은 **곧바로** `HoldConfirmed` 로 전이하며 quick-press 타이머/판정 자체를 취소한다. 따라서 이후 caps lock 이 짧게 눌렸다 떼어져도 quick press 이벤트는 애초에 발생 후보에서 제외되고, `Shift + caps lock = caps lock` 조합만 평가된다.

**동일 소스 키 중복 배정 방지 — 결정.** `Remap key to Seek: caps lock` / `Remap caps lock to: left control` / `Remap key to hyper key: caps lock` 이 동시에 설정될 수 있는가라는 질문에 대해, 이 엔진은 **런타임 결정론적 우선순위(위 표)를 1차 방어선으로 채택**하고, **UI 경고를 2차 방어선으로 병행**한다(UI 단독 차단은 채택하지 않는다).

- *근거 1 — UI 단독 차단이 구조적으로 불완전하다.* Seek 소스 키(Seek 탭), hyper 소스 키(Hyperkey 탭), caps lock 리매핑 대상(Presets 탭)은 서로 다른 탭의 서로 다른 팝업 컨트롤이다. 저장 시점에 세 탭을 가로질러 검증하는 로직을 만들 수는 있지만, 설정 마이그레이션·향후 설정 가져오기 기능·수동 설정 파일 편집 등 UI 를 거치지 않는 경로로도 충돌 상태는 발생할 수 있다. 런타임이 이런 상태를 만나도 항상 결정론적으로 동작해야 한다.
- *근거 2 — 표의 계층 순서 자체가 이미 "가장 그럴듯한 사용자 의도"를 반영한다.* Seek(화면 탐색 전용 모드 진입)은 다른 어떤 리매핑보다 명백히 상위 의도이고, hyper 화(modifier 화)는 preset 조합보다, 그리고 preset 조합은 단순 1:1 리매핑보다 "더 많은 조건을 요구하는, 더 구체적인 규칙"이므로 우선한다는 일반 원칙과도 일치한다.
- *UI 경고(2차 방어선)*: 저장 시점에 세 탭에 걸쳐 동일 소스 키가 둘 이상 활성화되어 있음을 감지하면, 뒤늦게(우선순위가 낮은) 등록된 규칙 옆에 인라인 경고를 표시한다(예: "이 키는 이미 Hyperkey 에서 사용 중입니다"). 저장 자체는 막지 않는다 — 사용자가 의도적으로 계층적 조합(계층 2+3 합성처럼)을 구성하는 정당한 사용도 있기 때문이다. 경고 문구·배지 디자인의 세부는 미정(§9).

### 3-c. Quick press(탭 vs 홀드) 판정 상태 머신 ⭐

`Quick press caps lock to execute:`, `Quick press left or right shift to input corresponding:`, `Double tap shift = caps lock`, 그리고 §3-b 계층 2 의 hyper/meh/bleh hold 판정이 모두 이 상태 머신 하나를 공유한다. 소스 키(물리 키코드) 하나당 독립된 인스턴스가 존재한다.

| 현재 상태 | 입력 | 조건 | 다음 상태 | 방출 이벤트 |
| :--- | :--- | :--- | :--- | :--- |
| `Idle` | 등록된 소스 키의 keyDown | — | `PendingDown` | 없음. 타이머 시작(임계값 = `Quick press duration`). 원본 keyDown 은 **보류**(§ 하단 "보류 vs 낙관적 통과" 결정 참조) |
| `PendingDown` | 동일 소스 키의 keyUp | 경과 시간 `<` `Quick press duration` **그리고** 그동안 다른 키 입력 없음 | `Double tap shift` 류 설정이 켜져 있으면 `WaitingSecondTap`, 아니면 `QuickPressEmitted`(즉시 `Idle` 로 환원) | quick press 액션(예: `Quick press caps lock to execute: caps lock` 이면 실제 caps lock 토글 이벤트, `/` 이면 슬래시 문자) — `Double tap` 대기 중이면 아직 방출하지 않고 유예 |
| `PendingDown` | 임의의 **다른** 키의 keyDown 수신 | — | `HoldConfirmed` (즉시) | 보류 중이던 소스 키 keyDown 을 이제 "modifier 로 확정"하여 §3-b 계층 2/3 평가에 즉시 반영. v1.62 예방 지점 — 다른 키가 도착한 순간 quick-press 후보에서 완전히 배제 |
| `PendingDown` | 타이머 만료(경과 시간 `≥` `Quick press duration`), 소스 키는 아직 눌린 채 | — | `HoldConfirmed` | 보류 중이던 소스 키 keyDown 을 modifier 로 확정, 이후 도착하는 이벤트부터 §3-b 계층 2 평가에 반영 시작 |
| `HoldConfirmed` | 소스 키의 keyUp | — | `Idle` | 없음(단순 modifier 해제 — 합성 flagsChanged 로 modifier off 를 방출) |
| `WaitingSecondTap` | 동일 소스 키의 두 번째 keyDown | 첫 keyUp 이후 경과 시간 `<` double tap 최대 간격(§4, 내부 상수 추정치) | `DoubleTapConfirmed` → 즉시 `Idle` | double tap 액션 방출(예: `Double tap shift = caps lock` → caps lock keyDown+keyUp 합성) |
| `WaitingSecondTap` | double tap 최대 간격 초과(두 번째 keyDown 없음) | — | `Idle` | 유예됐던 단일 quick press 액션을 지금 방출(지연 emit) |
| `Idle` | 등록되지 않은 키/조건 불일치 | — | `Idle` | 원본 이벤트 그대로 통과(계층 5) |

**핵심 난점 — 보류(버퍼링) vs 낙관적 통과, 그리고 결정.** 이 엔진은 **보류(버퍼링) 방식을 채택**하며, 순수 낙관적 통과-후-보정 방식은 기각한다.

- *기각 사유*: `CGEventTap` 이 한 번 방출한 keyDown 은 대상 앱에 실제로 전달되며, macOS 에는 "방금 보낸 keyDown 을 취소"하는 API 가 없다. 낙관적으로 caps lock 의 원본 keyDown 을 그대로 통과시켰다가, 나중에 "사실은 quick press 였다"고 판명되면 이미 도착한 keyDown(예: 실제 caps lock 토글)을 되돌릴 방법이 없다 — 잘못된 낙관적 방출은 **되돌릴 수 없는 부작용**을 만든다. 정확성을 latency 보다 우선한다.
- *체감 지연을 최소화하는 설계*: 다만 이 보류는 "매번 `Quick press duration`(추정 1000ms) 전체를 기다린다"는 뜻이 아니다. 상태표에서 보듯 판정은 **모호성이 해소되는 즉시** 끝난다 — (1) 다른 키가 눌리는 순간(hyper 조합의 일반적 사용 패턴, 보통 수십 ms 이내에 해소), (2) keyUp 이 오는 순간(quick press 그 자체 — 사용자가 키를 뗀 시점과 판정 시점이 사실상 동일해 체감 지연이 없다), (3) 타이머 만료(소스 키를 홀로 오래 누르고 있는 경우로, 이 경우는 정의상 "아직 아무 것도 할 필요가 없는" 대기 상태이므로 지연으로 느껴지지 않는다). 실제로 전체 `Quick press duration` 만큼 지연이 체감되는 경우는 "소스 키만 누르고 다른 키도 안 누르고 떼지도 않은 채 가만히 있는" 드문 상황뿐이며, 이 상황에서는 지연을 느낄 만한 출력 자체가 없다.
- 이 결정은 Karabiner-Elements/QMK 류의 tap-hold 알고리즘(“permissive hold”)과 원리가 같다 — 이미 검증된 패턴을 채택한다.

**Double tap 은 별도 차원.** `Double tap shift = caps lock` 은 단일 눌림의 지속 시간이 아니라 **연속된 두 번의 탭 사이 간격**을 판정해야 하므로, 위 표의 `WaitingSecondTap` 이라는 별도 상태와 별도 임계값(§4, 내부 상수로 추정)을 필요로 한다. `Quick press duration` 슬라이더와는 다른 값이며, 스크린샷에 이 값을 조절하는 UI 컨트롤은 확인되지 않았다(§4, §9).

---

## 4. 설정 항목

이 엔진이 **직접 소유**하는 설정만 기록한다. 소스 키 선택 팝업(`Remap key to hyper key:` 등)이나 프리셋 활성화 체크박스 자체는 F-05/F-08 소유이며, 엔진은 그 결과로 만들어지는 규칙 테이블만 소비한다.

| 설정 항목 | UI 위치 | 컨트롤 | 관측/제안 값 | 근거 |
| :--- | :--- | :--- | :--- | :--- |
| `Quick press duration` | Presets 탭, caps lock 그룹 | 슬라이더(눈금 8칸) + 값 라벨 | 스크린샷 관측값 **1000 ms**. 최소/최대/간격은 미확정 → `(추정)` **최소 200ms · 최대 1600ms · 간격 200ms**(8개 눈금: 200/400/600/800/1000/1200/1400/1600, 1000 이 5번째 눈금과 정확히 일치) | 조사 원문에 수치 범위 없음(연구노트 Q6). 8칸 눈금과 관측값 1000ms 을 동시에 만족하는 등간격 배열을 역산해 제안. §9 로 승계 |
| Double tap 최대 간격 | (UI 노출 없음, 내부 상수로 추정) | — | `(추정)` **300 ms** | 스크린샷에 `Double tap shift = caps lock` 옆에 슬라이더나 값 표시가 없다 — 사용자 조절 UI 없이 내부 고정값일 가능성. 300ms 는 OS 표준 더블클릭 간격(시스템 환경설정 기본값 대역)에서 유추한 추정치. §9 로 승계 |

이 엔진 자체는 위 두 값을 제외하면 사용자에게 노출되는 설정이 없다. 이벤트 탭 인프라, 중재 우선순위, 레이아웃 독립 판정, Secure Input 대응은 모두 내부 동작이며 설정 항목이 아니다.

---

## 5. 엣지 케이스와 실패 모드

| # | 상황 | 엔진 대응 |
| :--- | :--- | :--- |
| 1 | **탭 타임아웃**(`kCGEventTapDisabledByTimeout`) | §3-a `Disabled` → 즉시 `CGEventTapEnable` 재활성화. 반복 발생 시 콜백 내부에서 시간이 걸리는 코드가 있는지 원인 조사가 필요하다는 내부 로그 남김(사용자 노출 없음) |
| 2 | **탭 비활성화**(`kCGEventTapDisabledByUserInput`, 예: 시스템 설정에서 접근성 권한을 토글) | 동일하게 재활성화 시도. 재활성화가 계속 실패하면(권한이 실제로 취소된 경우) `NotInstalled` 로 전이하고 F-11 의 권한 상태 UI 와 연동 |
| 3 | **절전 복귀**(v1.58 실패 모드) | `NSWorkspaceDidWakeNotification` 수신 시 탭 유효성 강제 재확인. 죽어 있으면 처음부터 재생성 |
| 4 | **로그인 / 화면 잠금 해제** | `NSWorkspaceSessionDidBecomeActiveNotification`, 화면 잠금 해제 알림 수신 시 동일하게 재확인 |
| 5 | **사용자 전환(Fast User Switching)** | 세션이 비활성화되는 동안(`NSWorkspaceSessionDidResignActiveNotification`) 탭은 유지하되 이벤트가 오지 않는 것이 정상. 세션이 다시 활성화되면 #4 와 동일 절차로 재확인 |
| 6 | **Secure Input 활성화**(암호 필드 포커스) | 콜백 진입 시 `IsSecureEventInputEnabled()` 확인. `true` 면 §3-b/§3-c 어떤 규칙도 평가하지 않고 원본 이벤트를 그대로 통과(§6 참조). 이 상태에서 quick-press 상태 머신이 `PendingDown` 등 중간 상태에 있었다면, 다음 정상 이벤트가 왔을 때 상태를 `Idle` 로 강제 리셋하여 stale 상태가 남지 않게 한다 |
| 7 | **권한 취소가 런타임 중 일어남**(앱 실행 중 시스템 설정에서 Accessibility 를 끔) | 다음 콜백 호출 자체가 오지 않거나 탭이 무효화된다. 주기적 폴링(예: 수 초 간격)으로 `AXIsProcessTrusted()` 를 확인해 취소를 감지하고 `Terminated`/`NotInstalled` 로 전이, F-11 UI 와 연동 |
| 8 | **키를 누른 채 앱 전환**(⌘Tab 등으로 포커스가 바뀜) | 소스 키 상태 머신은 앱 포커스와 무관하게 물리 키 상태만 추적하므로 영향 없음. 단, hold-confirmed 된 modifier 를 전환된 새 앱에도 계속 적용할지는 규칙에 달림 — 물리적으로 키가 계속 눌려 있다면 modifier 도 계속 유지 |
| 9 | **키를 누른 채 세션 종료 / 절전 진입(stuck modifier)** | 소스 키의 keyUp 이벤트를 영영 받지 못하는 경우(예: 화면이 잠기며 keyUp 이 소실). 절전/잠금 진입 알림 수신 시 모든 `PendingDown`/`HoldConfirmed` 상태를 `Idle` 로 강제 리셋하고, 필요 시 모든 activee modifier 에 대해 합성 flagsChanged(off)를 방출해 stuck modifier 를 예방 |
| 10 | **외장 키보드 연결/해제** | `CGEventTap` 은 특정 키보드 장치가 아니라 세션 전체의 HID 이벤트를 받으므로 장치 목록 변경 자체는 엔진에 직접 영향이 없다. 다만 외장 키보드가 non-Apple 배열이라 caps lock 등 특정 키의 물리 keycode 매핑이 다를 가능성은 §9 미해결 질문으로 남긴다 |
| 11 | **입력 소스(키보드 레이아웃) 변경** | `kTISNotifySelectedKeyboardInputSourceChanged` 수신 시, §6 의 레이아웃→문자 역산 캐시를 무효화하고 다음 요청 시 재계산 |
| 12 | **다른 리매퍼(Karabiner-Elements 등)와 공존** | Karabiner-Elements 는 자체적으로 가상 HID 드라이버(Karabiner VirtualHIDDevice)를 통해 이벤트를 재주입하는 방식을 쓰므로, 이 엔진의 탭에는 Karabiner 가 이미 가공한 이벤트가 도착할 수 있다. 엔진은 이를 구분하지 않고 "도착한 그대로의 물리 이벤트"로 처리한다 — 두 리매퍼가 같은 키를 다르게 재정의하면 사용자에게 예측 불가능한 결과가 나올 수 있으나, 이는 macOS 이벤트 탭 체인의 구조적 한계이며 이 엔진이 해결할 수 있는 범위 밖이다. 최소한 자기 자신이 만든 합성 이벤트를 자기 탭이 다시 가로채 무한 루프에 빠지지 않도록, 합성 이벤트에는 식별 가능한 마커(예: `CGEventSetIntegerValueField` 로 커스텀 필드에 엔진 고유 태그 삽입)를 남기고 콜백 최초 진입 시 이 마커를 확인해 자기 자신의 합성 이벤트는 즉시 통과시킨다 |
| 13 | **키 반복(auto-repeat)** | 물리 키를 길게 누르고 있으면 OS 가 반복 keyDown 을 보낸다(`kCGKeyboardEventAutorepeat` 필드로 구분 가능). quick-press 상태 머신은 이미 `HoldConfirmed` 상태이므로 반복 keyDown 은 상태 전이를 유발하지 않고 무시(또는 필요 시 반복 keyDown 자체를 소비)한다 — 반복 keyDown 을 매번 새 `PendingDown` 으로 오인하면 안 된다 |
| 14 | **동일 키 중복 배정** | §3-b 하단 "동일 소스 키 중복 배정 방지" 결정에 따라 런타임은 항상 §3-b 우선순위 표로 결정론적으로 처리하고, UI 는 저장 시점에 경고를 표시(차단하지 않음) |

---

## 6. 필요한 플랫폼 API

| API / 알림 | 용도 |
| :--- | :--- |
| `CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, callback, userInfo)` | 탭 생성. `Default` 옵션 필수(소비·치환을 위해) |
| `CGEventTapEnable(tap, enable)` | 탭 재활성화(§3-a `Disabled` 상태 복구) |
| `CFMachPortCreateRunLoopSource` / `CFRunLoopAddSource` / `CFRunLoopRun` | 탭을 런루프에 등록하고 구동(§7 런루프 소유권 결정과 직결) |
| `CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode)` | 물리 키코드 판독 — 모든 판정의 입력 (§3-b, §3-c, 레이아웃 독립성의 근거) |
| `CGEventGetIntegerValueField(event, kCGKeyboardEventAutorepeat)` | 키 반복 판별(엣지 케이스 13) |
| `CGEventCreateKeyboardEvent` / `CGEventSetFlags` | 치환 이벤트 합성(단순 리매핑, 합성 modifier) |
| `CGEventKeyboardSetUnicodeString` | 물리 키코드로 표현 불가능한 문자를 강제로 얹는 폴백 경로(§3-b 계층 3/4 의 문자 출력, §7 결정 참조) |
| `TISCopyCurrentKeyboardInputSource`, `TISGetInputSourceProperty(kTISPropertyUnicodeKeyLayoutData)`, `UCKeyTranslate` | 현재 입력 소스에서 "원하는 문자를 내는 물리 키코드"를 역산(레이아웃 독립 출력의 주 경로) |
| `kTISNotifySelectedKeyboardInputSourceChanged`(Distributed Notification, `CFNotificationCenterAddObserver`) | 입력 소스 변경 감지 → 레이아웃 역산 캐시 무효화 |
| `IsSecureEventInputEnabled()` | Secure Input 상태 판별(§5 엣지 케이스 6) — 폴링 전용, 상태 변경을 알리는 공개 알림 API 는 확인되지 않음 |
| `NSWorkspaceDidWakeNotification` / `NSWorkspaceWillSleepNotification` / `NSWorkspaceSessionDidBecomeActiveNotification` / `NSWorkspaceSessionDidResignActiveNotification` | 절전·로그인·사용자 전환 시 탭 재확인 트리거 |
| `AXIsProcessTrusted()` | 런타임 권한 취소 감지용 폴링(엣지 케이스 7). 권한 요청 자체는 F-11 소유 |
| *(참조만, F-11 소유)* `IOHIDCheckAccess` / `IOHIDRequestAccess` | Input Monitoring 권한 — 이 문서는 "권한 없으면 탭이 안 열린다"는 의존 사실만 인지 |

---

## 7. 구현 접근

구현 접근은 프로젝트 공통 3분류로 판정한다 ([`README.md`](README.md#구현-접근-3분류) 참조).

- **순수 Rust** — 기존 안전(safe) 래퍼 크레이트만으로 완전히 커버되어 `unsafe` FFI 도 네이티브 shim 도 불필요
- **Rust 바인딩** — 크레이트는 존재하나 `unsafe` FFI(`objc2-*` 계열의 헤더 자동 생성 바인딩 또는 수기 `extern "C"` 선언) 직접 호출이 필요. 별도로 빌드하는 Swift/Objective-C 소스 파일은 없다
- **네이티브 shim 불가피** — Rust 에서 호출할 방법이 없어 Swift/Objective-C 소스를 별도로 빌드해 링크해야 함

**판정: Rust 바인딩.**

- 이벤트 탭 자체(생성/활성화/콜백 등록)는 `core-graphics` 0.25.0 이 `CGEventTap` 에 대한 비교적 안전한 래퍼를 제공해 분류 A 수준으로 커버된다.
- 그러나 이 엔진이 요구하는 다음 기능들은 `core-graphics` 래퍼 범위 밖이라 `objc2-core-graphics` / `objc2-core-foundation` / `objc2-application-services` 의 `unsafe` FFI 를 직접 호출해야 한다.
  - `UCKeyTranslate` 기반 레이아웃 역산(레이아웃 독립 출력, §3-b/§6)
  - `CGEventKeyboardSetUnicodeString` 폴백 경로
  - `TIS*` 계열 입력 소스 조회·알림 구독
  - `CFRunLoopSource` 를 전용 스레드의 런루프에 수동으로 등록하는 절차(아래 런루프 결정 참조)
  - `IsSecureEventInputEnabled()` FFI 선언(전용 크레이트 없음, `ApplicationServices` 프레임워크 직접 링크)

  이 unsafe 표면이 엔진의 핵심 로직(중재, 상태 머신) 자체보다 크지는 않지만, 없앨 수 없으므로 전체 판정은 **분류 B**로 내린다. Swift/ObjC 네이티브 shim(분류 C)까지 필요하다고 보지는 않는다 — 위 API 들은 모두 C ABI 로 노출되어 `objc2-*` 바인딩 또는 직접 `extern "C"` 선언으로 Rust 안에서 해결 가능하기 때문이다.

**기각한 대안 — `rdev`.** 연구 노트가 지적하듯 `rdev` 0.5.3 은 2023-06 이후 릴리스가 없고(3년 이상 미유지), 무엇보다 **이벤트 소비·치환 제어가 제한적**이다. 이 엔진의 존재 이유 자체가 "이벤트를 가로채 다른 이벤트로 바꿔치기"이므로, 관찰만 가능하거나 치환 제어가 불완전한 라이브러리는 요구사항을 충족하지 못한다. `CGEventTapCreate` 를 `kCGEventTapOptionDefault` 로 직접 호출하는 경로(즉 `core-graphics`/`objc2-core-graphics`)만이 소비·치환을 보장한다.

**런루프 소유권 — 결정 (연구노트 P1 해소).** **전용 스레드에서 독립된 `CFRunLoop` 를 돌리고, `CFRunLoopSource` 를 그 위에 `kCFRunLoopCommonModes` 로 등록한다.** Tauri 의 메인 런루프(`NSApplication`/webview 이벤트 루프)에 얹지 않는다.

- *근거 1 — 트래킹 모드(tracking mode) 문제.* macOS 의 메인 런루프는 사용자가 메뉴를 열거나 창을 드래그하는 동안 `NSEventTrackingRunLoopMode` 로 전환된다. 소스가 `kCFRunLoopDefaultMode` 로만 등록되어 있으면, 바로 이 순간(메뉴가 열려 있는 동안) 이벤트 탭 콜백이 아예 스케줄되지 않는다. 리매핑 엔진은 사용자가 메뉴를 조작 중일 때도 hyper 키/quick press 등이 계속 동작해야 하므로, 이 취약점을 안게 되는 메인 런루프 결합은 받아들이기 어렵다. `kCFRunLoopCommonModes` 등록으로 이 문제는 완화할 수 있지만, 메인 스레드가 다른 이유로 바쁘거나(웹뷰 렌더링, IPC 처리) 블로킹되면 어차피 탭 콜백도 지연되어 §3-a 의 타임아웃 위험이 커진다.
- *근거 2 — 콜백 지연시간(latency) 예산.* §3-a 는 콜백이 절대 블로킹되어서는 안 된다고 규정한다. Tauri 메인 스레드는 webview IPC, 메뉴바 이벤트, 윈도우 이벤트 등 다른 책임을 함께 지므로 "항상 가볍다"는 보장이 없다. 전용 스레드는 오직 이 탭 콜백만 서비스하도록 격리되어 지연 예산을 독립적으로 통제할 수 있다.
- *비용*: 전용 스레드의 상태(공유 상태 테이블, §3-b/§3-c)를 메인 스레드(설정 UI, Seek 세션 상태)와 동기화해야 한다. 이는 락(경합이 적은 스핀락 또는 `parking_lot::Mutex`) 또는 원자적 플래그(`AtomicBool`/`AtomicU8`) + 채널로 해결하며, 콜백 안에서의 락 획득은 경합이 사실상 없는 짧은 임계 구역으로 제한한다(§3-a 의 "콜백 금지 사항"과 일관).
- 이 결정은 실측(연구노트가 "실측 필요"로 남긴 P1)을 대체하지 않는다 — 설계 시점의 근거 있는 판단이며, 구현 후 실제 지연시간·타임아웃 발생률을 측정해 재검토할 것을 §9 에 남긴다.

**주요 크레이트/버전**: `core-graphics` 0.25.0(탭 생성·기본 이벤트 조작), `objc2-core-graphics` 0.3.2 + `objc2-core-foundation` 0.3.2(런루프 수동 관리, 탭 래퍼가 커버 못 하는 필드 접근), `objc2-application-services` 0.3.2(`TIS*`, `IsSecureEventInputEnabled` 등 Application Services 네임스페이스), `objc2` 0.6.4(런타임 기반).

---

## 8. 수용 기준

- [ ] `CGEventTapCreate` 가 `kCGSessionEventTap` / `kCGHeadInsertEventTap` / `kCGEventTapOptionDefault` 로 생성되며, keyDown/keyUp/flagsChanged 및 Hyperkey 마우스 옵션에 해당하는 이벤트 타입을 마스크에 포함한다.
- [ ] 탭이 `kCGEventTapDisabledByTimeout` 또는 `kCGEventTapDisabledByUserInput` 을 수신하면 예외 없이 `CGEventTapEnable` 로 재활성화를 시도한다.
- [ ] 절전 복귀(`NSWorkspaceDidWakeNotification`), 로그인(`NSWorkspaceSessionDidBecomeActiveNotification`), 화면 잠금 해제 시 탭 유효성을 재확인하고 필요 시 재생성한다.
- [ ] 이벤트 탭 콜백은 블로킹 I/O, AX 트리 순회, OCR 등 무거운 연산을 직접 수행하지 않는다(무거운 작업은 채널로 위임).
- [ ] §3-b 중재 우선순위 표의 5개 계층이 구현되어 있고, 상위 계층이 성립하면 하위 계층은 평가되지 않는다(short-circuit 이 코드/테스트로 확인 가능).
- [ ] v1.20 시나리오(caps lock 이 hyper 소스이면서 동시에 `Caps lock + WASD` 트리거 키) 재현 테스트에서 WASD 가 정상적으로 화살표로 치환된다.
- [ ] v1.62 시나리오(`Shift + caps lock = caps lock` 활성 + `Quick press caps lock to execute` 활성) 재현 테스트에서 shift+caps lock 조합 시 quick press 오발이 발생하지 않는다.
- [ ] §3-c quick press 상태 머신이 표에 정의된 모든 전이(다른 키 입력에 의한 즉시 홀드 확정 포함)를 구현하고, 소스 키 keyDown 은 판정이 끝나기 전까지 대상 앱에 전달되지 않는다(보류 방식).
- [ ] 물리 키코드(virtual keycode) 기반으로 판정하며, 문자 기반(예: 현재 눌린 문자가 무엇인지)으로 소스 키를 식별하는 코드 경로가 없다.
- [ ] 레이아웃 독립적 문자 출력이 필요한 경우, 현재 입력 소스에서 목표 문자를 내는 물리 키코드를 역산하는 경로를 우선 사용하고, 역산 실패 시에만 `CGEventKeyboardSetUnicodeString` 폴백을 사용한다.
- [ ] `kTISNotifySelectedKeyboardInputSourceChanged` 수신 시 레이아웃 역산 캐시가 무효화된다.
- [ ] `IsSecureEventInputEnabled()` 가 `true` 인 동안 어떤 리매핑/치환도 수행하지 않고 원본 이벤트를 그대로 통과시킨다.
- [ ] 동일 소스 키가 둘 이상의 규칙에 등록된 경우에도 런타임은 항상 §3-b 우선순위에 따라 결정론적으로 하나의 결과만 낸다(비결정적 동작이나 크래시가 없다).
- [ ] 키 반복(auto-repeat) 이벤트가 quick-press 상태 머신을 다시 `PendingDown` 으로 되돌리지 않는다.
- [ ] 절전 진입/화면 잠금 등으로 keyUp 을 영영 받지 못할 수 있는 상황에서, 관련 알림 수신 시 모든 진행 중 상태를 강제로 `Idle` 로 리셋해 stuck modifier 를 방지한다.

---

## 9. 미해결 질문

| # | 질문 | 현재 처리 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | `Quick press duration` 슬라이더의 정확한 최소/최대/간격 | `(추정)` 200–1600ms, 200ms 간격으로 제안(§4). 8개 눈금과 관측값 1000ms 을 만족하도록 역산한 값일 뿐, 실측 근거 없음 | 앱 설치 후 슬라이더 드래그하며 값 읽기(연구노트 Q6과 동일 항목) |
| 2 | Double tap 최대 간격의 실제 값과, 사용자 조절 UI 존재 여부 | `(추정)` 내부 상수 300ms. UI 노출 없다고 가정 | 앱 설치 후 `General` 탭 등에 숨겨진 컨트롤이 있는지 확인 |
| 3 | `CFRunLoopSource` 를 전용 스레드에 두는 결정(§7)의 실측 검증 | 설계 근거는 있으나 실제 타임아웃 발생률·지연시간 미측정 | 프로토타입 구현 후 트래킹 모드 상황(메뉴 열기 등)에서 hyper 키 반응성 측정 |
| 4 | 동일 키 중복 배정 UI 경고의 정확한 문구·배지 디자인 | 결정은 §3-b 에서 내렸으나(런타임 우선순위 + 비차단 경고) 구체적 UI 카피는 미정 | F-05/F-08 UI 설계 시점에 확정 |
| 5 | Karabiner-Elements 등 타 리매퍼와 동시에 이벤트 탭이 설치되었을 때 macOS 가 실제로 어떤 순서로 콜백 체인을 호출하는지 | 엣지 케이스 12 에서 "구조적 한계로 통제 불가"로만 서술 | 실제 두 앱을 함께 실행해 관찰 |
| 6 | `IsSecureEventInputEnabled()` 를 매 콜백마다 확인할지, 별도 주기로 폴링할지 | 이 문서는 매 콜백 확인을 전제로 서술(오버헤드가 작은 syscall 이라는 가정) | 실측으로 오버헤드 확인, 필요 시 캐시+주기 폴링으로 전환 |
| 7 | 외장(비-Apple) 키보드에서 caps lock 등 주요 소스 키의 물리 keycode 가 Apple 내장 키보드와 다를 가능성 | 엣지 케이스 10 에서 미해결로 남김 | 실제 외장 키보드 다수로 keycode 로그 비교 |
| 8 | 콜백이 강제 종료되기까지의 정확한 타임아웃 값(Apple 비공개) | §3-a 에서 "경험적으로 수백 ms 수준"으로만 서술 | 실측(의도적으로 콜백을 지연시켜 타임아웃 발생 시점 측정) |
