# F-07 · 키 리매핑 엔진 (공통 기반)

> **한 줄 요약** — Seek(F-01)·Hyperkey/meh/bleh(F-05)·Power User Presets(F-08) 가 공유하는 키 리매핑 하부구조. ⭐ 실측으로 확정: 리매핑은 단일 `CGEventTap` 경로가 아니라 **물리적으로 다른 세 경로**(A: `CGEventTap` 이벤트 합성·중재, B: IOHID 커널 레벨 매핑, C: HID 잠금 상태 직접 조작)로 나뉜다(§1, §3-d). 경로 A 내부에서는 여전히 단일 중재(arbitration) 엔진이 규칙을 조정하며, 기능별로 독립된 이벤트 탭이나 독립된 IOHID 매핑을 설치하는 설계는 명시적으로 기각한다.
> **의존성** — Accessibility 권한(F-11)이 승인되어 있지 않으면 경로 A(`CGEventTapCreate`)가 유효한 탭을 반환하지 않는다. ⭐ 실측 정정: SuperKey 가 앱 시작 시 명시적으로 확인하는 권한은 `AXIsProcessTrusted` 하나뿐이다 — Input Monitoring 은 명시적으로 확인되지 않고 실패·재시도로 흡수된다(§3-a, §6). 이 문서는 "권한 절차 자체는 F-11 소관"이라는 전제 위에서 엔진 내부 동작만 다룬다.
> **이 엔진을 소비하는 명세** — F-01(Seek 트리거의 키 리매핑 진입점 — ⭐ quick press 로 Seek 를 여는 세 번째 활성화 경로가 실측으로 확인됨, §3-c), F-05(Hyperkey / meh / bleh), F-08(Power User Presets 전 항목). 세 명세 모두 이 문서가 정의하는 규칙 테이블 위에 자신의 "규칙"만 등록한다.
> **앱별 비활성화 게이트** — 최전면 앱을 추적해 비활성 목록에 든 앱에서는 리매핑을 통과시켜야 한다(§3-f). 게이트 자체의 판정 시점은 이 문서 소관이나, 메뉴바 UI·저장 형식의 상세는 `menu-bar-and-lifecycle.md`(F-10) 소관이다.

---

## 1. 개요

SuperKey 는 겉보기엔 세 가지 기능(Seek, Hyperkey, Power User Presets)이지만, 조사에서 확인된 실제 버그 두 건은 이들이 **UI 상으로만 분리되어 있을 뿐, 물리적으로는 정확히 같은 자원(키보드 이벤트 스트림, 특히 caps lock 같은 소수의 "쓸모없는" 키)을 두고 경쟁한다**는 사실을 증명한다.

- **v1.20** — "Fixed a bug where caps lock + keys being mapped to arrow keys wasn't working when caps lock was remapped as the hyper key." caps lock 이 Hyperkey 소스 키로 지정된 상태에서 Presets 의 "Caps lock + WASD = 화살표" 조합이 깨졌다. Hyperkey 기능과 Presets 기능이 caps lock 의 down/up 상태를 **각자 독립적으로** 추적했기 때문에 발생 가능한 부류의 버그다.
- **v1.62** — "\"Shift + caps lock = caps lock\" will no longer trigger a quick press caps lock keypress." 두 Presets 항목이 같은 caps lock 키 이벤트를 각자 해석하다 충돌했다.

이 문서는 이 두 버그가 **다시는 발생할 수 없는 구조**를 정의하는 것이 목적이다. 핵심 설계 원칙은 다음 하나로 요약된다.

> **물리 키 하나의 현재 상태(눌림 여부, 눌린 지속 시간, 동시에 눌린 다른 키)는 시스템 전체에서 단 하나의 "정본(canonical) 상태"로만 존재한다.** Seek/Hyperkey/Presets 는 이 정본 상태를 **읽기만** 하며, 각자 별도의 이벤트 탭이나 별도의 keyDown/keyUp 추적 로직을 갖지 않는다. "이 이벤트를 최종적으로 어떻게 바꿀 것인가"는 이 문서가 정의하는 단일 중재 우선순위 표(§3-b)가 매 이벤트마다 한 번만 결정한다.

⭐ **경로가 하나가 아니라 셋이다(실측: 번들 심볼·번들 문자열).** 위 원칙은 "정본 상태"를 어떻게 유지하느냐에 대한 것이고, 그 정본 상태를 바탕으로 **실제로 바깥 세계에 리매핑 결과를 내보내는 경로**는 셋으로 갈린다. 이는 기존 명세가 암묵적으로 전제했던 "모든 리매핑은 `CGEventTap` 하나를 거친다"는 가정을 무너뜨리는, 이번 실측의 가장 큰 발견이다.

| 경로 | 메커니즘 | 확인된 심볼/문자열 |
| :--- | :--- | :--- |
| **A — 이벤트 합성** | `CGEventTap` 으로 원본 이벤트를 가로채 다른 `CGEvent` 로 치환·방출 | `CGEventTapCreate`·`CGEventTapEnable`·`CGEventTapIsEnabled`·`CGEventCreateKeyboardEvent`·`CGEventCreateMouseEvent`·`CGEventGetFlags`·`CGEventSetFlags`·`CGEventSetType`·`CGEventSetIntegerValueField`·`CGEventPost`·`CGEventKeyboardSetUnicodeString`·`CGEventSourceCreate`·`CGEventGetLocation` |
| **B — IOHID 커널 레벨 매핑** | `hidutil` 이 쓰는 것과 동일한 `HIDKeyboardModifierMappingSrc`/`Dst` 키를 `IOHIDServiceClientSetProperty` 로 직접 쓰거나, `hidutil` 자체를 서브프로세스로 실행. 매핑은 커널 HID 드라이버 층에서 적용되며 개별 앱의 이벤트 스트림을 거치지 않는다 | 심볼 `IOHIDServiceClientSetProperty`·`IOHIDEventSystemClientCreateSimpleClient`·`IOHIDServiceClientConformsTo` / 문자열 `hidutil property -g UserKeyMapping`, `HIDKeyboardModifierMappingSrc`/`Dst`, `Unable to set keyboard mapping`(SuperKey 원문) / `Tools_Tools.bundle` 의 셸 실행 오류 `bash command error: %@`, `Error parsing plist response` |
| **C — HID 잠금 상태 직접 조작** | caps lock 의 실제 잠금(대문자 고정·LED)을 직접 읽고 쓴다 — 이벤트 합성이 아니다 | 심볼 `IOHIDGetModifierLockState`·`IOHIDSetModifierLockState` |

세 경로의 역할 분담과 선택 기준은 §3-d 에서 정의한다.

엔진은 다음 책임을 진다.

1. **리매핑 경로 인프라** — 경로 A(`CGEventTap`)를 열고 죽지 않게 유지하며, 경로 B(IOHID/`hidutil`)의 시스템 전역 상태를 설치·추적하고 **앱 종료 시 정리(cleanup)를 책임지며**(§3-a2), 경로 C(HID 잠금 상태)를 필요할 때 직접 읽고 쓴다.
2. **경로 선택** — 규칙마다 어느 경로로 구현할지 결정한다(§3-d). 사용자에게는 보통 노출되지 않지만, `Advanced ▸ Synthesize Caps Lock Remap` 처럼 경로를 전환하는 스위치가 실재한다(§3-d).
3. **중재** — 경로 A 내부에서, 여러 규칙이 동시에 해당하는 물리 키 이벤트에 대해 결정론적으로 하나의 결과만 낸다(§3-b).
4. **판정 원시 자료 제공** — quick press/hold 판정, 물리 키코드 기반 매칭, 레이아웃 독립적 문자 출력, Secure Input 인지, 최전면 앱 게이트(§3-f)라는 다섯 가지 "저수준 서비스"를 F-01/F-05/F-08 이 공통으로 가져다 쓸 수 있게 만든다.

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
   Secure Input 이 활성화된 창(예: 로그인 다이얼로그)에 포커스가 가 있는 동안 caps lock 을 눌러도 **경로 A(이벤트 합성) 기반** hyper 조합은 발동하지 않는다. 이는 macOS 의 의도된 보안 제약이며, 엔진은 이를 우회하지 않고 원본 이벤트를 그대로 통과시킨다. ⭐ 다만 caps lock 리매핑이 경로 B/C(커널 HID 매핑·HID 잠금 상태)로 구현된 규칙이라면, Secure Input 은 애플리케이션에 전달되는 이벤트 스트림만 걸러낼 뿐 그보다 아래 층(커널)의 HID 상태 자체를 바꾸지 않으므로 **영향을 받지 않을 가능성이 있다** `(미확정 — 실측으로 검증하지 않았다. 단정하지 않는다. §5·§9)`.

6. **SuperKey 가 강제 종료된 뒤 재실행한 사용자** (경로 B 정리(cleanup) 시나리오)
   사용자가 `Remap caps lock to: left control` 을 켜둔 채 Activity Monitor 등으로 SuperKey 프로세스를 강제 종료한다. 경로 B(IOHID 매핑)는 시스템 전역·영구 상태이므로 앱이 죽어도 caps lock 은 계속 left control 로 동작한다 — 문자열 "Key remap cleared. Reloading key remapping"(SuperKey 원문)이 이 정리 절차가 정상적으로는 존재함을 뒷받침한다. 사용자가 SuperKey 를 다시 실행하면, 엔진은 시작 시점에 기존 IOHID 매핑 잔존 여부를 감지하고 자신의 현재 설정과 재조정한다 — 그렇지 않으면 사용자가 설정을 끈 뒤에도 caps lock 이 영구히 left control 로 남는 사고가 난다(§3-a2, §5).

7. **외장 키보드를 연결/해제한 사용자**
   USB 키보드를 연결하면 macOS 는 그 장치에도 별도의 HID 서비스를 노출한다. 경로 B(IOHID 매핑)는 **장치 단위로 적용된다**(실측: `../research/per-device-hid-spike.md` S-1 — ⭐ 이전 판의 `(미확정 — 이유는 해석)` 은 이로써 해소되었다). 따라서 새로 연결된 키보드에는 기존에 설치해둔 매핑이 적용되어 있지 않을 수 있다. 엔진은 `IOHIDManagerRegisterDeviceMatchingCallback` 으로 새 장치 연결을 감지하고 경로 B 리매핑을 다시 적용한다 — 문자열 "Detected keyboard: "(SuperKey 원문)이 이 감지 이벤트가 로그로 남는다는 근거다(§3-a, §5).

---

## 3. 동작 명세

### 3-a. 이벤트 탭 생명주기 상태표 (경로 A)

| 상태 | 진입 조건 | 엔진 동작 | 다음 상태로의 전이 | 비고 |
| :--- | :--- | :--- | :--- | :--- |
| `NotInstalled` | 앱 시작 직후, 또는 권한 상실 확정 후 | 탭 없음. 모든 키 이벤트는 macOS 기본 동작 그대로 | Accessibility 권한 확인 성공(`AXIsProcessTrusted`) → `Installing` — ⭐ 실측 정정: **Input Monitoring 은 앱이 명시적으로 확인하지 않는다**(`IOHIDCheckAccess`/`IOHIDRequestAccess` 링크 없음). 대신 뒤이은 `IOHIDManagerOpen` 실패를 재시도로 흡수한다(§6) | F-11 권한 상태와 1:1 대응. 상세는 F-11 소관 |
| `Installing` | 권한 확인 통과, `CGEventTapCreate` 호출 시도 중, 또는 ⭐ 설정 변경으로 도출된 이벤트 마스크가 현재 탭과 달라짐(이슈 #140 — `Active`/`Disabled` 에서 재생성) | `kCGSessionEventTap` / `kCGHeadInsertEventTap` / `kCGEventTapOptionDefault` 로 생성 시도. ⭐ 이벤트 마스크는 더 이상 고정이 아니라 `EngineConfig` 로부터 도출된다(`ultrakey_core::tap_mask::mouse_event_needs` → `ultrakey_platform::event_tap::build_event_mask`, 이슈 #140) | 성공(`NULL` 아닌 탭 반환) → `Active` / **권한 미승인으로 인한 실패** → `NotInstalled`(F-11 이 권한을 다시 부여할 때까지 대기, 재시도는 지수 백오프) / **권한이 확인된 상태에서도 생성 자체가 실패**(희귀 케이스) → `Terminated`(치명적 — 아래 "탭 생성 실패는 치명적이다" 참조; **단, 이슈 #140 의 설정 변경 재생성 트리거에서는 어느 실패든 `NotInstalled` + `TapLost` — §5 #27**) | `Default` 옵션이어야 이벤트 소비·치환 가능 — `ListenOnly` 는 관찰만 가능해 이 엔진의 목적과 맞지 않으므로 채택하지 않는다 |
| `Active` | 탭 생성 성공, `CFRunLoopSource` 가 런루프에 등록되어 콜백이 실제로 호출됨 | keyDown/keyUp/flagsChanged/마우스 이벤트는 마스크에 포함된 종류만 도착한다(이슈 #140 — 소비할 수 없는 구성에서는 마스크에서 제외). 콜백마다 §3-b 중재 규칙 실행. ⭐ **동시에 별도의 주기적 워치독이 탭 생존을 폴링한다**(실측: 번들 문자열 "Checking key loop.") — `kCGEventTapDisabledByTimeout` 콜백 통지만으로는 탭이 죽었는지 확정하기에 부족하다는 뜻이다. 폴링 주기는 `(미확정)`(§9) | 콜백이 `kCGEventTapDisabledByTimeout` 또는 `kCGEventTapDisabledByUserInput` 을 이벤트 타입으로 받음, **또는** 워치독이 부재("Key loop appears to not be running,")를 판정 → `Disabled` / 권한이 런타임에 취소됨(§5) → `NotInstalled` / 설정 변경으로 마스크가 달라짐 → `Installing`(재생성; 직전 `force_reset`, 이슈 #140) | 정상 상태. 대부분의 시간을 여기서 보낸다. 실측 문자열 "Key loop starting." → "Key loop started running" 이 이 상태 진입을 로그로 남기는 것으로 보인다 |
| `Disabled` | 콜백 자신이 타임아웃 또는 사용자 개입으로 탭이 꺼졌음을 통지받거나, 워치독이 부재를 판정("Key loop no longer exists.") | 즉시 `CGEventTapEnable(tap, true)` 호출로 재활성화 시도("Resume key loop.") — ⭐ **이슈 #65 갱신(2026-09-01, Phase 2)**: 이 시도는 **연속 유한 횟수**(`REENABLE_MAX_CONSECUTIVE`, 기본 5)로 제한된다. 리셋 트리거는 시간이 아니라 **트램폴린에 실제(비활성화 아닌) 이벤트가 도달했다는 사실**뿐이다 — 그 순간 탭이 살아서 이벤트를 통과시키고 있음이 증명되기 때문이다. 정상적인 `kCGEventTapDisabledByTimeout` 은 드물게 한 번씩 오고 그 사이 실제 이벤트가 예산을 리셋하므로 이 한도에 걸리지 않는다 | 재활성화 성공(예산 안에서 실제로 다시 이벤트가 통과함) → `Active` (같은 프레임에서 즉시 복귀 시도) / **예산 소진 또는 그 사이 `AXIsProcessTrusted()==false` 로 확인됨** → 재활성화도 재생성도 시도하지 않고 **탭을 해체**(`CFMachPortInvalidate` + 런루프 소스 제거 + drop) → `NotInstalled`, 권한 모니터에 복구를 이관한다(아래 "재시작(relaunch)" 문단 갱신 참고) / 설정 변경으로 마스크가 달라짐(예산 미소진일 때만) → `Installing`(이슈 #140) | v1.58 "key remapping was not working upon wake or login" 이 이 상태에서 재활성화 로직이 없었을 때 발생하는 실패 모드. `kCGEventTapDisabledByTimeout` 은 콜백이 너무 오래 걸렸다는 신호이기도 하므로, 재발 방지를 위해 §3-a 하단 "콜백 금지 사항"을 반드시 지킨다. ⛔ **이 예산이 시간 창으로 리셋되던 최초 구현(commit 7031351)은 권한 회수 시 재활성화⇄비활성화가 mach 메시지 속도로 무한 반복돼 시스템 전체 입력이 멎는 회귀를 냈다**(이슈 #65) — 원인·수정 근거는 `docs/dev/manual-verification.md` "1-c·1-d" 및 GitHub 이슈 #65 진단/리뷰 코멘트에 기록돼 있다 |
| `SuspendedBySleepOrLock` | `NSWorkspaceWillSleepNotification` / 화면 잠금(`com.apple.screenIsLocked` distributed notification) 수신(실측 문자열: "Received sleep notification", "Stopping listening") | 탭 자체는 유지하되, 깨어난 직후 한 차례 `CGEventTapEnable` 로 살아있는지 강제 확인(taps 는 절전 중 죽는 경우가 실측으로 확인되어 왔음 — v1.58 원인 추정 범주) | `NSWorkspaceDidWakeNotification`(실측 문자열: "Received wake notification", "Restarting keyboard listener on wake") / 화면 잠금 해제 → ⭐ **즉시 재확인하지 않는다.** 실측(설정 키)으로 확정: 재확인·재시작은 지연(delay) 뒤에 실행되며, 지연 파라미터가 트리거마다 별도로 존재한다(`restartOnWakeDelay`·`wakeKeyboardDelay`·`keyboardConnectionDelay`·`touchListenerDelay`) → 지연 경과 후 탭 상태 재확인 → `Active` 또는 `Installing`(탭이 완전히 무효화되어 있으면 재생성) | 로그인 시에도 동일 절차 — `NSWorkspaceSessionDidBecomeActiveNotification` 수신 시 탭 재확인. ⭐ **디바운스가 있다**: 문자열 "Time since last exit: …s ago, no need to restart."(SuperKey 원문)이 확정하듯, 직전 재시작/종료로부터 경과 시간이 임계값 미만이면 재시작 자체를 건너뛴다(임계값 `(미확정)`, §9) — 절전·잠금해제·세션전환이 짧은 간격으로 연달아 발생해도 탭을 반복 재생성하지 않기 위한 디바운스로 보인다 |
| `Terminated` | 앱 종료, 사용자가 시스템 설정에서 Accessibility 권한을 명시적으로 회수, **또는 `Installing` 상태에서 권한이 확인된 채로 탭 생성이 실패**(치명적) | `CFRunLoopSourceInvalidate` + 탭 참조 해제. 치명적 실패의 경우 프로세스 자체를 종료한다(실측 문자열: "com-knollsoft-Superkey Failed to create event tap. Exiting program") | 앱 재시작 또는 권한 재승인 시 `NotInstalled` 부터 재시작 | — |

⭐ **갱신(2026-09-06, 이슈 #140) — 설정 변경에 의한 재생성은 이슈 #65 의 재생성 금지와 다른 트리거다.** `Active`/`Disabled` 에서 `Installing` 으로 되돌아가는 위 전이는 탭 이벤트 마스크가 `EngineConfig` 로부터 다시 도출된 값과 달라졌을 때만 일어난다 — 이는 사람이 설정 화면에서 클릭한 속도로만 도착하는 `EngineCommand::Reconfigure` 트리거이지, 아래 문단이 금지하는 `RecoverTap` 트리거(탭 자신의 비활성화 통지·재활성화 예산 소진·권한 상실)가 아니다. 재생성은 재활성화 예산이 소진되어 폭주가 진행 중일 때는 허용되지 않고(`KeepTap`), 탭이 아예 없을 때도 허용되지 않는다(`NoTap` — 복구는 여전히 권한 모니터가 새 `Engine` 을 만드는 몫이다). `RecoverTap` 경로(`recover_tap_decision`·`handle_recover_tap`)는 이 갱신으로 바뀌지 않는다 — 재생성을 시도하지 않는다는 아래 문단의 결정 그대로다.

⭐ **탭 생성 실패는 치명적이다 — 재시도하지 않고 종료한다(실측: 번들 문자열).** 이전 버전은 `Installing` 실패 전부를 "지수 백오프로 재시도"로 다루었으나, 이는 **권한이 아직 없어 실패하는 흔한 경우**에만 맞는 서술이었다. SuperKey 원본은 문자열 "Failed to create event tap. Exiting program" 이 보여주듯, 권한이 확인된 상태에서 `CGEventTapCreate` 자체가 실패하는 (희귀한) 경우에는 재시도 루프를 돌리지 않고 **프로세스를 그대로 종료**한다. 이 구분을 명세에 반영한다: 권한 부재로 인한 실패는 F-11 온보딩 루프에 맡기고(`NotInstalled` 유지), 권한이 확인된 상태에서의 생성 실패는 치명적 실패로 별도 처리한다(§5·§8).

⭐ **재시작(relaunch)은 탭 재활성화가 아니라 프로세스 재실행이다(실측: AX).** 메뉴바 `Advanced` 에 `Relaunch` · `Relaunch After Wake` · `Delay Relaunch After Wake` · `Relaunch on Keyboard Connected` 항목이 실재한다. 이 라벨들의 동사가 "재활성화(re-enable)"가 아니라 "재실행(relaunch)"이라는 사실은, 원본이 궁극적인 복구 수단으로 `CGEventTapEnable` 재활성화보다 훨씬 무거운 **앱 프로세스 자체의 재시작**을 준비해두고 있다는 뜻이다 — 이는 `Disabled`→`Active` 전이(탭만 재활성화)가 항상 충분하지는 않다는 실측 방증이며, 기존 명세가 상정했던 것보다 훨씬 무거운 복구 수단이다. 원본이 왜 여기까지 필요로 했는지는 확정할 수 없다 `(미확정)`. 가능한 이유: (a) 경로 B(IOHID 매핑)의 시스템 상태가 탭과 독립적으로 어긋날 수 있고 프로세스 재시작이 그 상태를 재조정하는 가장 확실한 방법이거나, (b) 절전 복귀 후 `IOHIDManagerOpen`/`CGEventTapCreate` 가 요구하는 커널 리소스 자체가 일시적으로 불안정해 단순 재활성화보다 완전한 재초기화가 더 신뢰성 있게 동작하기 때문일 수 있다. 이 명세는 §3-a `Disabled`/`SuspendedBySleepOrLock` 전이(탭 재활성화)를 1차 복구 수단으로 유지한다.

⭐ **갱신(2026-09-01, 이슈 #65 Phase 2) — 반복 실패의 최종 수단은 프로세스 자동 재실행이 아니라 "탭 해체 + 권한 모델 이관"이다.** 위 문단은 SuperKey 원본이 프로세스 재실행 메뉴 항목을 갖고 있다는 **실측 사실**이라 그대로 남긴다. 다만 이 클론의 자체 설계 결정은 다르다: `handle_recover_tap` 재활성화 예산이 소진되면(§3-a `Disabled` 행) **재생성을 시도하지 않고** 탭을 해체해 `NotInstalled` 로 전이하고, 권한 모니터(`PermissionMonitor::report_tap_create_failed()`)에게 그 순간의 `AXIsProcessTrusted()` 로 `Denied`/`OutOfSync` 를 판정하게 한다 — "재생성 시도 → 실패 시 다시 재생성"을 반복하는 설계는 `AXIsProcessTrusted()` 가 stale `true` 를 돌려주는 상황에서 재생성마다 새 탭 = 새 예산이 다시 채워지는 **더 느린 폭주**가 된다는 것이 기각 사유다(이슈 #65 Phase 1 리뷰). §5#17·§8 의 "반복 실패 시 프로세스 재실행" 문구는 이 설계로 대체됐다 — `OutOfSync` 진단 화면(F-11, `permissions-onboarding.md` §3.3)이 그 UX 를 대신한다.

⭐ **외장 키보드 핫플러그 대응(실측: 번들 심볼·문자열, 신규 확정).** `IOHIDManagerRegisterDeviceMatchingCallback` / `IOHIDManagerRegisterDeviceRemovalCallback` / `IOHIDManagerSetDeviceMatching` / `IOHIDManagerScheduleWithRunLoop` 심볼, 문자열 "Detected keyboard: "(SuperKey 원문), 메뉴바 `Advanced ▸ Relaunch on Keyboard Connected`(실측: AX)가 이를 뒷받침한다. 키보드 장치가 연결·해제될 때 엔진은 리매핑을 다시 적용해야 한다 — 경로 B(IOHID 매핑)가 장치 단위로 적용되기 때문으로 보인다 `(미확정 — 이유는 해석)`. 기존 명세에 없던 실패 모드이며 §5 에 신규 항목으로 반영한다.

**콜백 안에서 해서는 안 되는 일**(모든 상태에 공통 적용): 블로킹 I/O(파일, 네트워크), AX 트리 순회, OCR, 뮤텍스 경합 가능성이 있는 잠금 획득, 로그 파일 동기 쓰기. macOS 는 이벤트 탭 콜백이 일정 시간(경험적으로 수백 ms 수준, Apple 은 정확한 값을 공개하지 않음) 안에 리턴하지 않으면 해당 탭을 `kCGEventTapDisabledByTimeout` 으로 강제 비활성화한다. 이는 §3-a `Disabled` 전이의 가장 흔한 원인이며, v1.58 부류 버그의 재발 방지는 "재활성화 로직"과 "애초에 타임아웃을 유발하지 않는 콜백 설계" 양쪽 모두를 요구한다. 무거운 작업이 필요한 판정(예: Seek 세션 시작)은 콜백 안에서는 **가벼운 상태 플래그 읽기/쓰기만** 수행하고, 실제 무거운 작업은 채널을 통해 다른 스레드로 위임한다.

### 3-a2. 경로 B/C 생명주기 — 설치·정리(cleanup) ⭐

경로 B(IOHID 커널 매핑)와 경로 C(HID 잠금 상태)는 경로 A 와 근본적으로 다른 생명주기를 가진다. **경로 A 는 앱 프로세스에 종속된다**(탭은 프로세스가 갖는 mach port 자원이라 프로세스가 죽으면 자동으로 사라진다). **경로 B 는 그렇지 않다** — `hidutil`/`IOHIDServiceClientSetProperty` 로 설정한 `UserKeyMapping` 은 커널 HID 드라이버 층의 전역·영구 상태이며, **앱이 비정상 종료되어도 남는다.**

| 항목 | 경로 A | 경로 B | 경로 C |
| :--- | :--- | :--- | :--- |
| 적용 범위 | 프로세스 내 이벤트 스트림 | 시스템 전역(커널 HID 층) | 시스템 전역(HID 잠금 상태 하나) |
| 앱 종료 시 자동 해제 | 예 | **아니오** — 명시적 정리 필요 | 해당 없음(마지막 값 유지) — 사용자가 실제로 caps lock 을 켜둔 것과 구분되지 않으므로 "정리 대상"이 아니라 "정상 상태" |
| 설치 시점 | 권한 확인 후 앱 시작 시(§3-a) | 해당 규칙이 활성화될 때 | 해당 규칙 평가 시점(이벤트 발생 시) |
| 정리(cleanup) 책임 | 프로세스 종료가 자동 처리 | ⭐ **엔진이 명시적으로 져야 한다** — 정상 종료 시 매핑을 비우거나, 다음 실행 시작 시 잔존 매핑을 감지해 자신의 현재 설정과 재조정한다. 문자열 "Key remap cleared. Reloading key remapping"(SuperKey 원문)이 이 정리 절차의 존재를 확정한다(실측: 번들 문자열) | 없음 |

⭐ **새 실패 모드 — 앱이 비정상 종료되어 경로 B 의 HID 매핑이 남는 경우**(§5 신규 항목). Activity Monitor 강제 종료, 크래시, `kill -9` 등으로 정상 종료 경로("Key remap cleared…")를 거치지 못하면 시스템 전역 키 매핑이 다음 실행까지, 혹은 재부팅까지 잔존한다. 관련 설정 키: `keepExistingIohid` · `currentMapping` · `axMapping` · `nonModMap` · `lastReadFromCommand` · `lastReadFromCommandDate` — 이름으로 미루어 엔진이 "마지막으로 커맨드(hidutil)에서 읽은 매핑"을 캐시해두고 시작 시 현재 상태와 비교하는 것으로 보인다 `(미확정 — 이름으로부터의 해석)`.

⭐ **경로 B 의 디바이스 매칭과 설치 확인 (2026-09-03 추가, 이슈 #110).** 경로 B 쓰기는 디바이스 한정이다(`per-device-settings.md` §3.6). 매칭 사전은 **4키 고정** `{"VendorID","ProductID","PrimaryUsagePage":1,"PrimaryUsage":6}` 다 — VID/PID 프로퍼티가 **없는** 내장 키보드(`hidutil list` 의 `0x0`/`0x0`, 스파이크 S-10 실측)를 `0:0` 으로 잡되, usage 두 키가 `IOHIDSystem`(usage 65280/23, 스파이크 S-3)을 원리적으로 배제한다. `--set` 은 매칭이 아무 서비스도 못 잡아도 exit 0 이므로 **쓰기 직후 `--get` 되읽기로 우리 항목이 그 디바이스에 실렸는지 확인**하고, 없으면 실패(`NotApplied`)로 다룬다 — 이 되읽기는 "실렸는가"만 답하지 "동작하는가"는 답하지 않는다(스파이크 S-7, `per-device-settings.md` §3.6 규칙 9 와 충돌 없음). 어느 붙어 있는 키보드에서든 확인이 안 되면 엔진은 그 상태를 게시하고(`SharedState::d1_confirmed`) 앱이 "caps lock 커널 매핑 미확인" 을 보인다. 중재기는 그것과 별개로 **이벤트 모양**으로 방어한다(§5 #26).

### 3-b. 중재(arbitration) 우선순위 표 ⭐

⭐ **이 표는 경로 A(이벤트 합성) 내부의 중재 규칙이다.** 경로 B/C 로 구현되는 규칙(§3-d)은 이 표를 거치지 않고 커널/HID 층에서 직접 적용된다 — 대신 경로 B/C 자체가 §3-d 의 선택 기준을 통해 경로 A 와 충돌하지 않도록 미리 배정된다. 즉 "정본 상태를 하나로 공유한다"는 원칙은 경로 A 안에서의 판정에 대한 것이고, 경로 선택 자체는 그보다 상위의 결정이다.

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
- *체감 지연을 최소화하는 설계*: 다만 이 보류는 "매번 `Quick press duration`(실측: 최소 250ms·최대 2000ms·현재값 1000ms, §4)을 전체 다 기다린다"는 뜻이 아니다. 상태표에서 보듯 판정은 **모호성이 해소되는 즉시** 끝난다 — (1) 다른 키가 눌리는 순간(hyper 조합의 일반적 사용 패턴, 보통 수십 ms 이내에 해소), (2) keyUp 이 오는 순간(quick press 그 자체 — 사용자가 키를 뗀 시점과 판정 시점이 사실상 동일해 체감 지연이 없다), (3) 타이머 만료(소스 키를 홀로 오래 누르고 있는 경우로, 이 경우는 정의상 "아직 아무 것도 할 필요가 없는" 대기 상태이므로 지연으로 느껴지지 않는다). 실제로 전체 `Quick press duration` 만큼 지연이 체감되는 경우는 "소스 키만 누르고 다른 키도 안 누르고 떼지도 않은 채 가만히 있는" 드문 상황뿐이며, 이 상황에서는 지연을 느낄 만한 출력 자체가 없다.
- 이 결정은 Karabiner-Elements/QMK 류의 tap-hold 알고리즘(“permissive hold”)과 원리가 같다 — 이미 검증된 패턴을 채택한다.

⭐ **F-05 §3.2 와의 충돌 해소 — 보류를 건너뛰는 조건 (2026-08-30, M1 구현 / 이슈 #5).**

`hyperkey.md` §3.2 의 상태 전이표는 "소스 키 physical keyDown → 곧바로 합성 `flagsChanged` 를 낸다"고 쓰여 있다. 반면 이 문서 §3-b 계층 2 의 조건은 "quick press 상태 머신이 그 키를 **`HoldConfirmed` 로 판정한 상태**" 이고, 위 표는 `Idle` → `PendingDown` 에서 원본 keyDown 을 **보류**한다. **두 서술은 충돌한다.**

**F-07 이 정본이다** — F-05 §1 이 "이 기능은 이벤트를 가로채고 재주입하는 메커니즘을 스스로 갖지 않으며, 그 메커니즘은 F-07 이 제공한다"고 이미 위임했다. 그 위에 다음 규칙을 추가해 충돌을 해소한다.

> **소스 키에 quick press 액션이 등록되어 있지 않으면, keyDown 즉시 `HoldConfirmed` 로 전이한다(보류하지 않는다).**

- *근거*: 보류는 **모호성을 해소하기 위한 비용**이다(하단 "보류 vs 낙관적 통과" 결정). 그 키에 quick press 액션이 아예 없으면 "탭인가 홀드인가"라는 질문 자체가 성립하지 않으므로, 기다릴 이유가 없다. 이 최적화 없이 구현하면 hyper 소스 키를 누를 때마다 최대 `Quick press duration`(기본 1000ms) 동안 modifier 가 나타나지 않아 기능이 사실상 쓸 수 없게 된다.
- *결과*: **M1 에서 hyper/meh/bleh 는 이 경로를 탄다.** hyper quick press(`quickHyperKeycode`/`executeQuickHyperKey`)는 존재가 확정됐을 뿐 노출 경로가 `(미확정)` 이라(§9 #5) M1 에서 액션을 등록하지 않기 때문이다. 따라서 M1 의 관측 가능한 동작은 F-05 §3.2 표의 서술과 정확히 일치한다.
- *caps lock 처럼 quick press 액션이 실제로 등록된 소스 키*(F-08/M2)에서는 보류 경로가 그대로 적용되며, 체감 지연은 하단 "체감 지연을 최소화하는 설계" 3개 항목이 흡수한다.
- *기각한 대안* — ① 항상 낙관적으로 통과시키고 나중에 보정: 하단 기각 사유(되돌릴 수 없는 부작용)가 그대로 적용된다. ② F-05 §3.2 를 정본으로 삼아 항상 즉시 합성: caps lock 이 hyper 소스이면서 동시에 quick press 대상인 구성(v1.20/v1.62 가 실제로 깨졌던 바로 그 구성)에서 quick press 가 영영 발동하지 않는다.

**Double tap 은 별도 차원.** `Double tap shift = caps lock` 은 단일 눌림의 지속 시간이 아니라 **연속된 두 번의 탭 사이 간격**을 판정해야 하므로, 위 표의 `WaitingSecondTap` 이라는 별도 상태와 별도 임계값(§4, 내부 상수로 추정)을 필요로 한다. `Quick press duration` 슬라이더와는 다른 값이며, AX 트리 실측으로도(4개 탭 전체 확인) 이 값을 조절하는 UI 컨트롤은 확인되지 않았다(§4, §9).

⭐ **quick press 판정 결과는 키 합성만이 아니다 — Seek 세션 열기로도 분기한다(실측: AX).** `Quick press caps lock to execute:` 팝업의 첫 항목이 `Seek` 다. 즉 이 상태 머신이 `QuickPressEmitted`/`DoubleTapConfirmed` 로 방출하는 "액션"은 키 이벤트 합성에 한정되지 않고, F-01 의 Seek 세션을 여는 신호일 수도 있다. 이는 F-01(`seek-activation-and-session.md`)이 정의하는 두 활성화 경로(전역 단축키 토글, 키 리매핑 hold 트리거)에 더해 **세 번째 활성화 경로**다 — F-01 은 이 상태 머신이 방출하는 "Seek 열기" 액션도 자신의 상태 머신 진입점으로 받아들여야 한다.

⭐ **hyper 소스 키에도 quick press 가 있다 — 존재가 확정된다(실측: 번들 문자열).** 키 `quickHyperKeycode` · `executeQuickHyperKey` · `hyperDownTime` 이 이를 뒷받침한다. 이 상태 머신은 caps lock quick press 전용이 아니라 hyper/meh/bleh 소스 키에도 적용되는 일반 메커니즘으로 보인다. 기존 §9 의 "hyper quick press 존재 여부 미확정"은 이로써 **존재 확정**으로 바뀐다. 다만 대응 UI 를 Seek/Hyperkey/Presets/General 4개 탭 어디에서도 찾지 못했다 — **노출 경로와 정확한 의미는 `(미확정)`**(§9).

키 `longPressCapsLockTurnsItOff` 의 존재는 별도 규칙을 시사한다 `(미확정 — 이름으로부터의 해석)`: caps lock 을 길게 누르면 (quick press 로 켜진) caps lock 잠금을 다시 해제하는 규칙일 가능성이 있다. 대응 UI 는 확인되지 않았다(§9). 그 밖에 실행 파일에서 확인되나 정확한 의미가 `(미확정)`인 타이밍 키: `debounceTime`(물리 키 디바운스로 추정) · `longPress`(quick press 와 별도의 홀드 판정 상수로 추정).

### 3-d. 경로 선택 기준 — 어떤 규칙이 어느 경로로 구현되는가 ⭐

세 경로는 상호 배타적 대안이 아니라 **역할이 다른 세 도구**다. 실측 심볼·문자열·설정 키만으로 "이 프리셋은 경로 X" 라고 1:1 확정할 수 있는 항목은 caps lock 잠금 상태(경로 C) 뿐이다. 나머지는 아래 원칙으로 판정하되, 개별 프리셋의 최종 배정은 F-08(`power-user-presets.md`) 소관이며 이 절은 **판정 기준**만 제공한다(개별 프리셋 전수 배정은 §9 미확정).

| 경로 | 적합한 경우 | 부적합한 경우 | 근거 |
| :--- | :--- | :--- | :--- |
| **A — 이벤트 합성** | 조건부 리매핑(다른 키와의 조합, quick press, hold 판정), 임의의 목표 키/문자로의 치환, Seek 소스 키, hyper/meh/bleh modifier 합성 — 즉 **"지금 이 순간의 판단"이 필요한 모든 것** | caps lock 의 실제 잠금 상태(LED·대문자 고정)를 바꿔야 하는 경우 — 이벤트 합성으로 caps lock keyDown/keyUp 을 내보내는 것과 실제 HID 잠금 상태를 바꾸는 것은 다르다(§3-a2) | `CGEventTap` 콜백은 매 이벤트마다 임의 판단을 실행할 수 있는 유일한 지점이다 |
| **B — IOHID 커널 매핑** | 무조건적인 1:1 키 치환으로, 앱이 죽어도 유지되길 원하거나 Secure Input 구간에서도 적용되어야 하는 경우로 보인다 `(미확정 — 실제 채택 이유는 해석)` | 조건부 판정이 필요한 모든 경우 — `hidutil`/`IOHIDServiceClientSetProperty` 는 "이 키코드는 항상 저 키코드" 라는 정적 테이블만 표현할 수 있고, quick press·hold·조합 같은 시간·문맥 조건은 표현할 수 없다 | `HIDKeyboardModifierMappingSrc`/`Dst` 자체가 정적 소스→대상 쌍의 배열이다(실측: 번들 문자열) |
| **C — HID 잠금 상태 직접 조작** | caps lock 의 "진짜 켜짐/꺼짐"이 의미를 갖는 규칙(`Double tap shift = caps lock`, `Left shift + right shift = caps lock`, `Shift + caps lock = caps lock` 등 — 결과물이 "caps lock 을 토글해야 한다"는 규칙 전부) | caps lock 을 다른 키로 치환하는 경우(`Remap caps lock to: left control` 류)는 대상이 caps lock 자체가 아니므로 해당 없음 | `IOHIDGetModifierLockState`/`SetModifierLockState` 는 잠금 상태 하나를 읽고 쓰는 API 이지 이벤트 치환 API 가 아니다 |

⭐ **`Advanced ▸ Synthesize Caps Lock Remap`(실측: AX)의 의미.** 메뉴바에 이 항목이 실재한다. 라벨이 "합성(synthesize)"이라는 단어를 쓰는 것으로 미루어, **기본은 경로 B/C(커널 매핑·HID 잠금 조작)이고 이 스위치를 켜면 경로 A(이벤트 합성)로 강제 전환**하는 것으로 해석된다 `(미확정 — 라벨로부터의 해석, 실제 동작을 관찰하지는 못했다)`. 이 해석이 맞다면 원본은 caps lock 리매핑에 대해 **두 구현을 모두 유지하며 사용자가 고를 수 있게 했다**는 뜻이다 — 아마도 경로 B/C 가 일부 환경(특정 커널 확장·보안 소프트웨어·가상 키보드 드라이버와의 충돌 등)에서 문제를 일으킬 때의 우회 수단으로 보인다. **클론 설계에 반영할 가치**: caps lock 관련 리매핑을 처음부터 경로 A 하나로만 구현하기보다, 경로 B/C 를 기본으로 하되 경로 A 폴백 스위치를 남겨두는 이중 구현 여지를 열어두는 편이 원본과의 기능 동등성에 유리하다. 이 문서는 그 스위치의 **존재와 방향성**만 확정하고, 실제 구현 여부·우선순위는 F-08/제품 결정 소관으로 넘긴다.

**경로 B 의 실행 방식 — FFI 직접 호출과 서브프로세스 실행이 공존한다(실측: 번들 심볼·`Tools_Tools.bundle` 문자열).** `IOHIDServiceClientSetProperty` 를 통한 직접 FFI 호출 경로와, `hidutil` 바이너리를 셸 서브프로세스로 실행하는 경로가 둘 다 심볼/문자열로 확인된다. 이 둘이 같은 결과에 도달하는 두 구현(하나가 폴백)인지, 서로 다른 설정 항목에 쓰이는 별개 경로인지는 `(미확정)`. 이 구분이 §7 의 구현 접근 판정(FFI 대 프로세스 실행)에 직접 영향을 준다.

### 3-e. 레이아웃 독립 판정 — 확정 API 조합 ⭐

기존 명세가 `(추정)`으로 남겼던 API 조합이 실측으로 확정된다(실측: 번들 심볼): `TISCopyCurrentKeyboardInputSource` · `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` · `TISGetInputSourceProperty` · `UCKeyTranslate` · `LMGetKbdType`. 관련 설정 키: `selectedKeyboardInputSource` · `checksStandardANSI` · `DynamicKeyCodes` · `ignoredKeycodes` · `handledKeys` · `cmdOptionKeycodes`.

⭐ **`TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 의 존재가 판정 절차를 바꾼다.** 이는 **비-ASCII 입력기(한글·일본어·중국어 등 CJK IME)가 현재 활성 입력 소스일 때 ASCII 가능한 레이아웃으로 폴백해 물리 키코드를 해석**하는 표준 macOS 기법이다 — `TISCopyCurrentKeyboardInputSource` 가 반환하는 소스를 그대로 `UCKeyTranslate` 에 넘기면, IME 가 활성인 동안 keycode→문자 변환 자체가 실패하거나 무의미한 결과를 낼 수 있다. 판정 절차를 다음으로 확정한다.

1. `TISCopyCurrentKeyboardInputSource()` 로 현재 입력 소스를 얻는다.
2. 그 소스가 ASCII 가능한지 판정한다(정확한 프로퍼티 키는 `(미확정)`).
3. ASCII 가능하지 않으면(CJK IME 등), `TISCopyCurrentASCIICapableKeyboardLayoutInputSource()` 가 반환하는 대체 레이아웃으로 **교체**해 이후 `UCKeyTranslate` 조회에 사용한다.
4. `LMGetKbdType()` 으로 물리 키보드 하드웨어 타입을 얻어 `UCKeyTranslate` 의 `iKeyboardType` 인자로 넘긴다(레이아웃 데이터가 키보드 타입에 따라 달라지는 배열을 포함하기 때문).
5. `TISGetInputSourceProperty(kTISPropertyUnicodeKeyLayoutData)` 로 얻은 레이아웃 데이터와 물리 keycode 를 `UCKeyTranslate` 에 넣어 목표 문자 또는 문자→keycode 역방향 테이블을 계산한다.

이 절차의 상세(정방향/역방향 테이블 구축, 캐시 무효화 시점, `CGEventKeyboardSetUnicodeString` 폴백)는 `localization-and-input-sources.md`(F-14) 소관이다 — F-14 는 아직 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 를 반영하지 않았으므로, 이 문서가 실측으로 확정한 API 조합을 F-14 갱신 시 반영해야 한다.

### 3-f. 앱별 비활성화 게이트 ⭐

메뉴바에 `Ignore <최전면앱이름>` 항목이 실재한다(실측: AX — 관찰 당시 "Ignore Ghostty"). 관련 키: `disabledApps` · `enabledApps` · `disabledForApp` · `frontAppId` · `frontAppName` · `frontmostAppToggle` · `ApplicationToggle`. 심볼 `NSWorkspaceDidActivateApplicationNotification`.

**엔진은 최전면 앱을 추적하고, 매 keyDown/keyUp/flagsChanged 이벤트 판정 이전에 이 게이트를 확인해야 한다.** 최전면 앱이 비활성화 목록에 있으면 §3-b 의 5개 계층을 전혀 평가하지 않고 원본 이벤트를 통과시킨다 — 개념적으로는 §3-b 계층 1 보다도 먼저 평가되는 계층 0(문지기)에 해당한다. `NSWorkspaceDidActivateApplicationNotification` 수신 시 `frontAppId`/`frontAppName` 상태를 갱신한다.

상세 정의(메뉴 항목의 정확한 라벨 생성 규칙, `disabledApps`/`enabledApps` 저장 형식, `typeToSeekEnabledAppIDs`/`typeToSeekDisabledAppIDs` 와의 관계)는 `menu-bar-and-lifecycle.md`(F-10) 소관이며, 이 문서는 엔진이 이 상태를 **참조해야 한다는 사실과 평가 시점**만 정의한다.

---

## 4. 설정 항목

이 엔진이 **직접 소유**하는 설정만 기록한다. 소스 키 선택 팝업(`Remap key to hyper key:` 등)이나 프리셋 활성화 체크박스 자체는 F-05/F-08 소유이며, 엔진은 그 결과로 만들어지는 규칙 테이블만 소비한다.

| 설정 항목 | UI 위치 | 컨트롤 | 관측/제안 값 | 근거 |
| :--- | :--- | :--- | :--- | :--- |
| `Quick press duration` | Presets 탭, caps lock 그룹 | 슬라이더 + 값 라벨 | ⭐ **실측(AX 트리): 최소 250 ms · 최대 2000 ms · 현재값(기본) 1000 ms.** 저장 키 `quickPressTimeout`. step 간격은 여전히 `(미확정)`(§9) | 이전 버전은 "눈금 8칸으로부터 최소 200ms·최대 1600ms·간격 200ms" 를 역산해 제안했으나 **틀렸다** — 실측과 어긋난다. **교훈**: 슬라이더의 시각적 눈금 개수만으로 min/max/step 을 역산하는 방법은 신뢰할 수 없다 — 반드시 슬라이더 자체의 최소/최대 값 속성(AX)이나 실제 드래그 관찰로 직접 확정해야 한다 |
| Double tap 최대 간격 | (UI 노출 없음, 내부 상수로 추정) | — | `(추정)` **300 ms** — 여전히 `(미확정)`, 값 자체는 근거 없는 추정치 | 저장 키 `doubleClickInterval` 로 확인됨(실측: 번들 문자열). **"사용자 조절 UI 없는 내부값"이라는 기존 추정은 승격 확정된다**: 4개 탭 전체를 AX 트리로 재확인해도 `Double tap shift = caps lock` 옆에 슬라이더·값 표시가 없다(실측: AX 트리). 300ms 자체(OS 표준 더블클릭 간격에서 유추)는 여전히 근거 없는 추정치다. §9 로 승계 |
| hyper quick press(`quickHyperKeycode` / `executeQuickHyperKey` / `hyperDownTime`) | `(미확정)` — 4개 탭 어디에서도 대응 컨트롤을 찾지 못했다 | — | 존재는 확정(실측: 번들 문자열). 노출 경로·정확한 의미는 `(미확정)` | §3-c 참조. `Quick press caps lock to execute:` 와 유사한 메커니즘이 hyper/meh/bleh 소스 키에도 있는 것으로 보인다(§9) |

이 엔진 자체는 위 값들을 제외하면 사용자에게 노출되는 설정이 없다. 이벤트 탭 인프라, 중재 우선순위, 경로 선택(§3-d), 레이아웃 독립 판정, Secure Input 대응, 앱별 비활성화 게이트는 모두 내부 동작이며 설정 항목이 아니다.

---

## 5. 엣지 케이스와 실패 모드

| # | 상황 | 엔진 대응 |
| :--- | :--- | :--- |
| 1 | **탭 타임아웃**(`kCGEventTapDisabledByTimeout`) | §3-a `Disabled` → 즉시 `CGEventTapEnable` 재활성화. 반복 발생 시 콜백 내부에서 시간이 걸리는 코드가 있는지 원인 조사가 필요하다는 내부 로그 남김(사용자 노출 없음) |
| 2 | **탭 비활성화**(`kCGEventTapDisabledByUserInput`, 예: 시스템 설정에서 접근성 권한을 토글) | ⭐ **갱신(이슈 #65 Phase 2)**: 동일하게 재활성화 시도하되 연속 유한 예산(§3-a `Disabled` 행) 안에서만. 예산이 소진되거나 그 사이 권한이 실제로 취소된 것으로 확인되면 **재생성을 시도하지 않고** 탭을 해체해 `NotInstalled` 로 전이하고 F-11 의 권한 상태 UI(권한 모니터 경유)와 연동 |
| 3 | **절전 복귀**(v1.58 실패 모드) | `NSWorkspaceDidWakeNotification` 수신 시 탭 유효성 강제 재확인. 죽어 있으면 처음부터 재생성 |
| 4 | **로그인 / 화면 잠금 해제** | `NSWorkspaceSessionDidBecomeActiveNotification`, 화면 잠금 해제 알림 수신 시 동일하게 재확인 |
| 5 | **사용자 전환(Fast User Switching)** | 세션이 비활성화되는 동안(`NSWorkspaceSessionDidResignActiveNotification`) 탭은 유지하되 이벤트가 오지 않는 것이 정상. 세션이 다시 활성화되면 #4 와 동일 절차로 재확인 |
| 6 | **Secure Input 활성화**(암호 필드 포커스) | 콜백 진입 시 `IsSecureEventInputEnabled()` 확인. `true` 면 §3-b/§3-c 어떤 규칙도 평가하지 않고 원본 이벤트를 그대로 통과(§6 참조) — **단, 이는 경로 A 에만 해당한다.** ⭐ 경로 B(커널 HID 매핑)·경로 C(HID 잠금 상태)는 애플리케이션 이벤트 스트림보다 아래 층(커널)에서 동작하므로, Secure Input 이 걸러내는 것은 경로 A 의 결과물뿐이고 **경로 B/C 는 영향을 받지 않을 가능성이 있다** `(미확정 — 실측으로 검증하지 않았다. 단정하지 않는다. §9)`. 이 상태에서 quick-press 상태 머신이 `PendingDown` 등 중간 상태에 있었다면, 다음 정상 이벤트가 왔을 때 상태를 `Idle` 로 강제 리셋하여 stale 상태가 남지 않게 한다 |
| 7 | **권한 취소가 런타임 중 일어남**(앱 실행 중 시스템 설정에서 Accessibility 를 끔) | ⭐ **갱신(이슈 #65 Phase 2)**: 1차 방어선은 탭 자신의 FSM 이다 — §3-a `Disabled` 행의 연속 재활성화 예산이 소진되면(수 초가 아니라 즉시, mach 메시지 왕복 수 회 안에) 탭을 해체하고 `NotInstalled` 로 전이한다. 배경 폴링(`permission_poll_background_ms`, 기본 5초)으로 `AXIsProcessTrusted()` 를 확인하는 것은 **2차·독립 방어선**이다 — 탭 스레드 FSM 이 어떤 이유로든 이 사건을 놓쳐도, 폴링이 별도로 `Denied` 를 감지해 앱 전체(`Engine::shutdown()`, 탭·워치독·시스템 훅·지연 스케줄러 전부)를 정지시킨다(`apps/ultrakey-app` 의 `on_permission_transition`/`stop_engine`). F-11 UI 와 연동 |
| 8 | **키를 누른 채 앱 전환**(⌘Tab 등으로 포커스가 바뀜) | 소스 키 상태 머신은 앱 포커스와 무관하게 물리 키 상태만 추적하므로 영향 없음. 단, hold-confirmed 된 modifier 를 전환된 새 앱에도 계속 적용할지는 규칙에 달림 — 물리적으로 키가 계속 눌려 있다면 modifier 도 계속 유지 |
| 9 | **키를 누른 채 세션 종료 / 절전 진입(stuck modifier)** | 소스 키의 keyUp 이벤트를 영영 받지 못하는 경우(예: 화면이 잠기며 keyUp 이 소실). 절전/잠금 진입 알림 수신 시 모든 `PendingDown`/`HoldConfirmed` 상태를 `Idle` 로 강제 리셋하고, 필요 시 모든 activee modifier 에 대해 합성 flagsChanged(off)를 방출해 stuck modifier 를 예방 |
| 10 | **외장 키보드 연결/해제**(실측: 번들 심볼·문자열 — 신규 확정, 이전 명세에 없던 실패 모드) | `CGEventTap`(경로 A)은 특정 키보드 장치가 아니라 세션 전체의 HID 이벤트를 받으므로 장치 목록 변경 자체는 경로 A 에 직접 영향이 없다. ⭐ 그러나 **경로 B(IOHID 매핑)는 장치 단위로 적용된다**(실측 확정 — [`../research/per-device-hid-spike.md`](../research/per-device-hid-spike.md) S-1. `--matching '{"VendorID":…,"ProductID":…}'` 로 건 매핑이 그 디바이스에만 적용되고 다른 디바이스에는 다른 배열이 동시에 존재함을 확인했다. **이전 판의 `(미확정 — 이유는 해석)` 은 해소되었다**). 따라서 새로 연결된 키보드에는 기존 매핑이 적용되어 있지 않다. 엔진은 `IOHIDManagerRegisterDeviceMatchingCallback`/`RemovalCallback` 으로 연결·해제를 감지해(문자열 "Detected keyboard: ") 경로 B 리매핑을 재적용한다. 메뉴바 `Advanced ▸ Relaunch on Keyboard Connected` 가 이 대응의 수동 트리거로 보인다. ⚠️ **같은 스파이크가 새 제약 셋을 확정했다**(같은 문서 S-2·S-3·S-6): ① 물리 디바이스 1개가 IOHID 서비스 여러 개라 `--get` 은 여러 행을 준다 — 잔존 감지는 집계해야 한다, ② `VendorID` 단독 매칭은 `IOHIDSystem` 을 함께 잡아 **사실상 전역 쓰기**가 되므로 매칭 사전에 `ProductID` 를 반드시 함께 넣는다, ③ `--set` 은 병합이 아니라 **배열 통째 교체**라 매칭 없는 전역 쓰기는 디바이스별 매핑을 파괴한다 — 디바이스별 설정(F-17)과 공존하려면 **D-1 을 포함한 모든 경로 B 쓰기가 디바이스 한정이어야 한다**(`per-device-settings.md`). 외장(비-Apple) 키보드에서 caps lock 등 주요 소스 키의 물리 keycode 가 Apple 내장 키보드와 다를 가능성은 여전히 §9 미해결 질문으로 남긴다 |
| 11 | **입력 소스(키보드 레이아웃) 변경** | `kTISNotifySelectedKeyboardInputSourceChanged` 수신 시, §6 의 레이아웃→문자 역산 캐시를 무효화하고 다음 요청 시 재계산 |
| 12 | **다른 리매퍼(Karabiner-Elements 등)와 공존** | Karabiner-Elements 는 자체적으로 가상 HID 드라이버(Karabiner VirtualHIDDevice)를 통해 이벤트를 재주입하는 방식을 쓰므로, 이 엔진의 탭에는 Karabiner 가 이미 가공한 이벤트가 도착할 수 있다. 엔진은 이를 구분하지 않고 "도착한 그대로의 물리 이벤트"로 처리한다 — 두 리매퍼가 같은 키를 다르게 재정의하면 사용자에게 예측 불가능한 결과가 나올 수 있으나, 이는 macOS 이벤트 탭 체인의 구조적 한계이며 이 엔진이 해결할 수 있는 범위 밖이다. 최소한 자기 자신이 만든 합성 이벤트를 자기 탭이 다시 가로채 무한 루프에 빠지지 않도록, 합성 이벤트에는 식별 가능한 마커(예: `CGEventSetIntegerValueField` 로 커스텀 필드에 엔진 고유 태그 삽입)를 남기고 콜백 최초 진입 시 이 마커를 확인해 자기 자신의 합성 이벤트는 즉시 통과시킨다 |
| 13 | **키 반복(auto-repeat)** | 물리 키를 길게 누르고 있으면 OS 가 반복 keyDown 을 보낸다(`kCGKeyboardEventAutorepeat` 필드로 구분 가능). quick-press 상태 머신은 이미 `HoldConfirmed` 상태이므로 반복 keyDown 은 상태 전이를 유발하지 않고 무시(또는 필요 시 반복 keyDown 자체를 소비)한다 — 반복 keyDown 을 매번 새 `PendingDown` 으로 오인하면 안 된다 |
| 14 | **동일 키 중복 배정** | §3-b 하단 "동일 소스 키 중복 배정 방지" 결정에 따라 런타임은 항상 §3-b 우선순위 표로 결정론적으로 처리하고, UI 는 저장 시점에 경고를 표시(차단하지 않음) |
| 15 | ⭐ **앱이 비정상 종료되어 경로 B 의 IOHID 매핑이 남는 경우**(신규 확정, §3-a2) | 정상 종료 경로("Key remap cleared. Reloading key remapping")를 거치지 못하면 시스템 전역 키 매핑이 잔존한다. 엔진은 시작 시점에 기존 IOHID 매핑을 감지해 현재 설정과 재조정한다(비교에 쓰이는 것으로 보이는 캐시 키: `currentMapping`/`lastReadFromCommand`/`lastReadFromCommandDate`) |
| 16 | ⭐ **탭 생성이 권한 확인 통과 후에도 실패하는 경우**(신규 확정, §3-a) | 원본은 재시도하지 않고 프로세스를 즉시 종료한다(문자열 "Failed to create event tap. Exiting program"). 이 명세는 권한 부재로 인한 실패(F-11 온보딩으로 위임)와 이 치명적 실패를 구분해 처리한다 |
| 17 | ⭐ **탭 재활성화만으로 복구되지 않는 경우**(신규 확정, §3-a) | 메뉴바 `Advanced ▸ Relaunch` 류 항목이 실재한다는 사실은, 원본이 `CGEventTapEnable` 재활성화보다 무거운 프로세스 재실행을 최종 복구 수단으로 두고 있다는 방증이다(원본의 실측 사실, 그대로 유지). ⭐ **갱신(이슈 #65 Phase 2) — 이 클론의 자체 설계는 자동 프로세스 재실행이 아니다.** 재활성화 예산이 소진되면 탭을 해체하고 권한 모델에 이관한다 — 권한이 여전히 `Granted` 인데 탭을 못 살리는 상태는 `OutOfSync` 진단 화면(F-11)으로 사용자에게 보여준다. 권한을 다시 `Denied`→`Granted` 로 왕복시키거나(시스템 설정에서 토글) 앱을 재시작하면 회복된다 — 자동 재실행은 하지 않는다 |

---

⭐ **2026-08-30 추가 (M2 1차 / 이슈 #13) — 실기기 검증이 새로 드러낸 두 가지.**

| # | 상황 | 엔진 대응 |
| :--- | :--- | :--- |
| 18 ⭐ | **modifier 소스 키는 `KeyDown`/`KeyUp` 으로 오지 않는다** — macOS 는 caps lock·shift·control·option·command·globe 의 누름/뗌을 **`kCGEventFlagsChanged` 하나로만** 전달한다. 소스 키 팝업 35종 중 F1~F24 를 뺀 **전부**가 여기 해당한다 | 중재기가 `FlagsChanged` 를 **down/up 으로 환원**한 뒤 계층 2 에 넣는다. 판정은 **우리가 소유한 정본 눌림 테이블**로 한다 — 그 keycode 가 눌린 기록이 없으면 누름, 있으면 뗌. ⛔ `ev.flags` 의 비트로 판정하지 않는다: (a) 좌/우 shift 가 공개 `CGEventFlags` 에서 **같은 비트**(`0x20000`)를 공유해 두 소스 키를 구분할 수 없고, (b) caps lock 의 `alphaShift` 비트는 눌림이 아니라 **잠금 상태**를 나타낸다. 상태가 어긋날 위험은 기존 `ForceResetState` 경로(#9)가 흡수한다 |
| 19 ⭐ | D-1 우회 폴백(§5 #26 (c))에서 F-08.1 이 합성하는 control `flagsChanged` 에 원본 caps 의 `alphaShift` 비트가 실린다 — `strip_caps_lock_bit` 는 hyper/meh/bleh `modifier_rules` 만 보고 hold_remap 전용 구성은 보지 않는다(#21 층위) | `(미확정 — 이슈 #110 P2 리뷰 지적, 실측 안 함)`. 폴백은 D-1 미설치 키보드에서만 밟히고 그 잠금 표시는 다음 이벤트의 실제 flags 로 덮이므로 이번에는 고치지 않았다 | 폴백 상태(예: `Synthesize Caps Lock Remap` 이 아니라 D-1 되읽기 실패)에서 브라우저 프로브의 `getModifierState("CapsLock")` 을 관찰; 재현되면 `strip_caps_lock_bit` 조건을 `caps_lock_alias.is_some()` 으로 넓힌다 |
| 19 ⭐ | **HID 계층 리매퍼가 먼저 키를 바꾼다** — Karabiner-Elements 처럼 가상 HID 장치로 키를 바꾸는 도구는 **경로 A 보다 아래**에 있다. 그런 도구가 `caps_lock → left_control` 을 걸어 두면 탭에는 `left control` 이 도착하고, `caps lock` 을 소스로 한 규칙은 **영영 발동하지 않는다** | ⛔ **엔진이 할 수 있는 일이 없다** — 원본 키코드가 프로세스에 도달하지 않는다. 이것은 버그가 아니라 경로 A 의 구조적 한계이므로, **진단으로만 다룬다**: 검증 절차(`../dev/manual-verification.md` "항목 2·3 사전 확인")가 리매퍼 존재를 먼저 확인하게 하고, 사용자는 그 리매퍼가 실제로 내보내는 키코드를 소스로 지정하면 된다. 경로 B(IOHID 커널 매핑)는 같은 계층이라 충돌 양상이 다르며, 그 조정은 §3-a2 소관이다 |

| 20 ⭐⛔ | **caps lock 은 래칭 키다 — 뗌 이벤트가 오지 않는다** | 실측(2026-08-30, 브라우저 프로브): caps lock 을 hyper 소스로 두고 소스 키를 톡 눌렀다 떼기를 3회 반복하며 사이사이 `a` 를 눌렀더니, **1·3번째 `a` 만 hyper 조합을 달고 2번째는 달지 않았다.** 즉 누름마다 `flagsChanged` 가 한 번씩만 오고 뗌에는 오지 않아, #18 의 down/up 환원이 "1번째=누름, 2번째=뗌"으로 해석한다 → **hyper 가 홀드가 아니라 토글로 동작한다.** ⛔ **경로 A 만으로는 고칠 수 없다** — 오지 않는 이벤트를 만들어낼 방법이 없다. 시간 기반 추정(일정 시간 뒤 자동 해제)은 "누른 채 유지"와 구분이 안 되므로 기각한다. **해법은 경로 B** — caps lock 을 사용되지 않는 모멘터리 키(예: `F18`)로 커널 매핑한 뒤 hyper 규칙을 그 키에 걸면 정상적인 down/up 쌍을 받는다. 이것이 §3-d 가 기록한 `Advanced ▸ Synthesize Caps Lock Remap` 스위치의 존재 이유로 보인다. **경로 B 규칙 배정은 F-08(M2 2차) 소관이므로 그때 함께 구현한다** — 그때까지 caps lock 이 아닌 모멘터리 소스 키(`right command`·`right option`·`F13` 등)를 쓰면 정상 동작한다. ⭐ **2026-08-30 갱신(M2 2차 / 이슈 #15): 구현했다.** caps lock 에 의존하는 규칙이 하나라도 켜지면 `caps lock → F18` 커널 매핑을 설치하고, 중재기가 진입 즉시 F18 을 caps lock 으로 되돌려 판정한다. 설계 근거·기각한 대안·되돌리는 수단(`Advanced ▸ Synthesize Caps Lock Remap`)은 `../dev/architecture.md` §6.1 결정 D-1 에 있다 |
| 21 ⭐ | **caps lock 의 잠금 비트(`alphaShift`)가 합성 이벤트에 따라붙는다** | 원본 caps lock 이벤트에는 `alphaShift`(0x10000)가 실려 오는데, 합성·통과 이벤트를 만들 때 `ev.flags` 를 그대로 물려주면 그 비트가 우리가 내보내는 모든 이벤트에 따라붙어 **다른 앱이 caps lock 켜짐으로 인식한다**(실측: 브라우저의 `getModifierState("CapsLock")` 이 참). ⭐ 이때 **하드웨어 잠금은 걸리지 않았다**(`ioreg` 의 `HIDCapsLockState` = `No`) — 즉 경로 C 로는 고칠 수 없는, **이벤트 flags 층위만의** 문제다. 대응: caps lock 이 modifier 소스로 등록되어 있는 동안 이 엔진이 만지는 이벤트에서 `alphaShift` 를 지운다. 그 키를 소스로 배정한 순간부터 사용자에게 caps lock 은 더 이상 잠금 키가 아니므로 잠금 비트가 남는 것은 되돌릴 수단이 없는 상태다. ⚠️ 범위는 **이미 손대는 이벤트**로 한정한다 — 통과 이벤트까지 건드리면 매 이벤트에 `CGEventSetFlags` 를 부르게 되어 §2.2 의 임계 경로 비용이 늘어난다 |

| 22 ⭐⛔ | **#18 의 출력 쪽 대칭 — modifier 를 `KeyDown`/`KeyUp` 으로 내보내면 아무도 눌린 것으로 보지 않는다** (2026-08-30 신설, 이슈 #19) | #18 은 **입력**만 다뤘다. 같은 사실이 **출력**에도 그대로 적용된다: macOS 가 modifier 의 눌림을 `flagsChanged` + flags 비트로만 표현하므로, 받는 앱도 그 모양으로만 인식한다. 그런데 `Remap caps lock to: left control`(F-08.1)의 대상 50종 중 **8종이 modifier**(`left/right control`·`shift`·`option`·`command`)인데, 구현은 그것을 `KeyDown`(flags 0)으로 합성해 내보냈다 → **받는 앱은 control 이 눌린 것으로 보지 않는다.** 실측(사용자 보고): 브라우저 프로브에서 눌림·뗌이 **둘 다 `keyup` 으로만** 찍혔다. 대응: 대상이 modifier 키면 (a) `flagsChanged` 로, (b) 일반 마스크와 **좌/우 구분 비트**(`NX_DEVICE*KEYMASK`)를 함께 실어 낸다. ⭐ (c) 나아가 **누르고 있는 동안 뒤따르는 다른 키 이벤트에도** 그 비트를 얹어야 한다 — 합성 `flagsChanged` 한 번만으로는 `⌃C` 가 만들어지지 않는다. hyper/meh/bleh 가 이미 쓰는 기구(정본 상태 테이블의 활성 flags 합산 → 통과 이벤트에 얹기)에 그대로 태운다. ⚠️ **caps lock 자신은 예외** — `alphaShift` 는 눌림이 아니라 잠금이므로(#18 (b), #21) 눌림 표현으로 쓰지 않는다 |
| 23 ⭐⛔ | **보류(§3-c)한 원본을 끝내 내보내지 않으면 그 키가 죽는다** (2026-08-30 신설, 이슈 #19) | §3-c 는 소스 키의 원본 keyDown 을 `PendingDown` 동안 **보류**하고, `HoldConfirmed` 로 넘어가는 순간 "확정하여 계층 2/3 평가에 즉시 반영" 하라고 한다. hyper/meh/bleh 소스나 F-08.1 대상이 있는 키는 그 "반영"이 곧 **대체 출력**이라 자연스럽게 처리된다. ⛔ **대체 출력이 없는 키가 구멍이었다** — `Double tap shift = caps lock`(F-08.8)이나 `Quick press … shift`(F-08.11)만 켜면 shift 는 추적 키가 되지만 대체 출력이 하나도 없다. 구현이 그 경우 아무것도 내지 않아 **보류가 아니라 폐기**가 됐고, 그 프리셋을 켜는 순간 **shift 키가 통째로 죽었다**(`Shift+a` 가 소문자). 대응: 대체 출력이 없으면 **보류했던 원본을 그 자리에서 방출한다** — hold 확정 시 down, 뗄 때 up, double tap 대기 타임아웃 시 down+up. ⚠️ **caps lock 은 예외** — D-1 이 켜져 있으면 원본은 물리적으로 `F18` 이라 되살려도 의미가 없고, caps lock 의 원래 기능(잠금 토글)은 §3-c 의 보류 결정이 "되돌릴 수 없는 부작용" 으로 든 바로 그 동작이다 |
| 24 ⭐ | **눌림 테이블 stale 로 인한 caps lock 하드웨어 잠금 오발화**(2026-09-03 신설, 이슈 #108) | 사용자 보고: caps lock 이 재현 조건 불명·저빈도로 하드웨어 잠금(LED·대문자 고정) 상태에 빠진다. 원인 판정 — **(b) 눌림 테이블 stale 이 근본 원인이다.** 0-c/0-d 게이트(F-10 앱 게이트·Secure Input)가 걸린 동안 매 콜백이 `force_reset` 으로 눌림 테이블을 무조건 지운다. 사용자가 shift 를 **누른 채** 그 구간을 드나들면 테이블은 거짓인데 물리 shift 는 여전히 눌린 상태가 되고, 그 뒤 오는 **진짜** 뗌을 `normalize_kind`(#18)가 눌림 테이블만 보고 판정하면 "안 눌려 있었으니 이건 누름"으로 오판해 `set_pressed(true)` 로 영구 고착시켰다. `Left/right shift`(F-08.9)·`Shift + caps lock = caps lock`(F-08.10)이 그 고착값으로 hold 조건(`EitherShift`/`Key`)을 오판해 `Effect::ToggleCapsLock` 을 오발화한다. 대응: `normalize_kind` 가 이벤트 **자신의** flags 를 먼저 본다 — 그 키의 좌우 공용 family 비트(`SHIFT`/`CONTROL`/`ALTERNATE`/`COMMAND`/`SECONDARY_FN`, caps lock 은 해당 없음)가 없으면 눌림 테이블과 무관하게 뗌으로 확정한다. ⛔ 좌우 구분 비트(`NX_DEVICE*KEYMASK`, #22 가 다루는 합성 출력 전용 비트)는 이 판정에 **재사용하지 않는다** — 실제 입력 이벤트에는 실리지 않아 모든 진짜 keyDown 이 뗌으로 오판된다(잔여 구멍: 양쪽 shift 를 다 든 채 한쪽만 stale 이면 반대쪽이 여전히 family 비트를 얹고 있어 이 판정이 개입하지 못한다 — 아래 자동 복구 안전망이 최종 방어선이다). **(a) D-1 부재 구간**은 기동 시 재조정이 탭 생성보다 먼저 동기 실행되고 핫플러그도 배선돼 있어 새 코드 없이 이미 좁아져 있었다 — 절전 복귀만 검증 불가능한 가설로 남아 `DidWake` 에 `ReapplyHidMapping(None)` 재적용 한 줄을 더해 닫았다(멱등, 새 경로 아님). **(c) 절전/잠금 중 외부 토글**은 아래 자동 복구 안전망이 그대로 흡수한다. ⭐ **자동 복구(안전망)** — caps lock 이 modifier 소스(D-1, `caps_lock_alias.is_some()`)로 배정된 동안 `caps_lock_state()==true` 가 관측되면 `set_caps_lock_state(false)` 로 되돌린다(기존 워치독 폴링에 편승, 새 타이머 없음). D-1 활성 중에는 물리 caps lock 이 커널에서 F18 로 이미 바뀌어 있어, 사용자가 키보드로 진짜 caps lock 을 잠글 수 있는 유일한 수단이 우리 자신의 경로 C(`Effect::ToggleCapsLock`)뿐이다 — 그래서 "직전 우리 효과가 캡스락을 켰는가"(`SharedState::caps_lock_owned_lock`, 불리언, 시간 제한 없음) 하나만으로 의도된 잠금(`Double tap shift`·`Left/right shift`·`Shift + caps lock = caps lock`)과 그 밖의 모든 잠금을 안전하게 나눌 수 있다. 시간 기반 유효기간은 두지 않는다 — #20 이 이미 기각한 "시간 기반 자동 해제"와 같은 문제(누른 채 유지와 구분 불가)가 생긴다 |
| 25 | **macOS 자체 keyboard·입력기 설정과의 상호작용**(2026-09-03, 이슈 #108 추가 관점) | (d) macOS 의 "Caps Lock 키로 입력 소스 전환"(비-라틴 입력기가 하나라도 있으면 옵션이 나타나고, 짧게 누르면 전환·길게 누르면 잠금으로 다룬다)은 **독립 원인이 아니다** — D-1 이 활성이면 물리 caps lock 은 커널에서 F18 로 바뀌어 도착하므로, 이 macOS 자체 기능(물리 caps lock keycode 에만 반응)도 `normalize_kind` 의 stale 극성 문제도 함께 비껴간다. #24 의 (a)(e)(안전망)가 D-1 의 커버리지를 넓힐수록 이 기능이 발동할 표면도 함께 좁아지는 **종속 현상**이다. (e) D-1 은 `list_attached_keyboards()`(usage page 1/usage 6)로 잡히는 **모든** 디바이스에 무차별 설치한다(`path_b.rs::apply_all_inner`) — Karabiner-DriverKit-VirtualHIDDevice 가 매칭 대상으로 잡혀도 자기 몫의 D-1 을 자동으로 받으므로 별도 코드가 필요 없다. 그 dext 의 검출·D-1 설치 확인 절차는 `../dev/manual-verification.md` §6-0-bis 를 그대로 쓴다(새 절차를 만들지 않는다 — VID/PID 를 몰라도 그 절차가 성립한다). (f) 하드웨어 잠금(`ioreg` `HIDCapsLockState`)과 이벤트 flags 층위만의 잠금 표시(#21)는 서로 다른 층이다 — 이 이슈의 증상은 전자다. (g) 시스템 설정 `키보드 ▸ 보조 키` 의 사용자 remap 은 hidutil `UserKeyMapping` 과 같은 커널 프로퍼티를 공유하는 것으로 보인다 `(추정)` — D-17-6(`../dev/architecture.md` §7.2, "같은 src 충돌 시 우리가 이긴다")이 저장 위치와 무관하게 이미 이 상호작용을 흡수한다: caps-lock 의존 프리셋이 켜지는 순간 D-1 이 그 자리를 가져가는 것이 의도된 설계다 |
| 26 ⭐⛔ | **내장 키보드에 D-1 이 설치되지 않아 caps lock 단독 입력이 control 고착을 낸다**(2026-09-03 신설, 이슈 #110) | 사용자 보고(MacBook Air `Mac17,4`): caps lock 을 짧게 누르면 이후 control 이 눌린 것처럼 동작(HJKL 이 방향키). 인과 사슬(실측, 스파이크 S-10): ⓪ 내장 키보드의 IOHIDDevice 노드에 `VendorID`/`ProductID` 프로퍼티가 **없어** `read_device_properties` 가 항목을 버렸고 `list_attached_keyboards()` 가 0대를 돌려줘 **D-1 설치 시도 자체가 없었다**(로그 `count=0 devices=[]`, 이슈 #86 원인 (b) 확정) ① 설령 열거됐어도 D-17-4 규칙 2(`product_id == 0` 전체 거부)와 2키 매칭 사전이 `0:0` 에 쓰는 것을 막았다 ② 그래서 물리 caps 가 커널에 caps 로 도달해 래칭 키의 `flagsChanged` 1회(#20)만 오고, #18 환원이 이를 눌림으로 기록해 F-08.1 이 합성한 control(#22)의 뗌이 다음 누름까지 오지 않는다. **이슈 #108 의 잠금 증상과 같은 원인이다**(PR #109 안전망이 잠금은 되돌리므로 남은 증상이 고착). 대응 셋: **(a) 근본** — VID/PID 부재는 0 으로 폴백해 열거하고, 매칭 사전에 usage 1/6 을 항상 넣고, 규칙 2 를 `0x5ac:0x0`(IOHIDSystem) 정확 일치로 좁혀 `0:0` 에 D-1 을 설치한다(§3-a2). **(b) 확인** — 쓰기 후 되읽기(§3-a2) + 미확인 상태 표시. **(c) 방어선** — `caps_lock_alias` 가 켜져 있는데 keycode `0x39` 의 `flagsChanged` 가 도착하면 그 이벤트를 낸 키보드에 D-1 이 없다는 것이 이벤트로 증명된다(D-1 정상이면 F18 KeyDown/KeyUp 으로 온다) → 중재기가 그 이벤트 하나를 **down+up 쌍(탭)** 으로 판정한다. 시간 기반 추정이 아닌 결정론적 하강이라 #20 의 기각 사유에 걸리지 않고, D-1 정상 환경에서는 조건이 참이 될 수 없어 홀드를 건드리지 않으며, 전역 플래그와 달리 외장 정상·내장 미설치 혼합 상태에서도 디바이스별로 옳다. 이 분기는 `Effect::ToggleCapsLock` 을 떨어뜨린다 — 하드웨어 잠금은 커널이 물리 caps 로 이미 토글했고 경로 C 를 다시 부르면 읽고-반전이라 도로 뒤집힌다. 실측(`caps_lock_toggle_probe`+`tap_listen`): 경로 C 가 세션에 싣는 `flagsChanged` 는 keycode **`0xFF`** 라 이 분기를 다시 밟지 않는다. ⚠️ 폴백의 한계(사실): 그 키보드에서 caps 는 **탭으로만** 동작한다 — 홀드(hyper·`caps+hjkl`)는 없고, caps 에 quick press/double tap 액션이 있으면 같은 시각의 down/up 이라 그 액션이 발화하며, 커널이 건 하드웨어 잠금은 우리 소유가 아니라 #24 안전망이 폴링 주기(기본 1초) 안에 되돌린다(`Shift + caps lock = caps lock` 도 잠금이 잠깐 켜졌다 꺼진다). 폴백의 목적은 고착 방지이고 잠금·홀드까지 살리는 길은 (a) 뿐이다 |
| 27 ⭐ | **설정 변경으로 탭 마스크가 달라짐**(2026-09-06 신설, 이슈 #140) | `mouse_apply`·규칙·트랙패드 설정을 바꿔 `ultrakey_core::tap_mask::mouse_event_needs` 가 도출하는 값이 지금 탭의 마스크와 달라지면, 엔진은 `force_reset` 을 먼저 방출한 뒤(합성 modifier 즉시 해제) 탭을 재생성한다(§3-a `Installing` 재진입). 재생성 창(옛 탭 drop ~ 새 `CGEventTapCreate`) 동안 도착하는 이벤트는 탭 없이 그대로 통과한다(코드 근거에 의한 추론, 실기기 미검증). 직전 `force_reset` 으로 stuck modifier 는 남지 않을 것으로 본다(추정 — 미검증, `docs/dev/input-latency-spike.md` §8 체크리스트). ⭐ **재생성 실패는 최초 설치와 다르다** — 살아 있던 탭을 사용자 클릭 하나로 해체한 뒤이므로 `NotTrusted`/`CreateFailed` 어느 쪽이든 치명(`Terminated`)으로 굳히지 않고 `NotInstalled` + `TapLost` 로 권한 모델(#17 의 `OutOfSync` 진단)에 이관한다. 새 탭은 재활성화 예산(§3-a `Disabled` 행)을 새로 받는다 — 폭주 중(`reenable_budget_exhausted`)에는 재생성하지 않으므로 #65 의 해체가 무효화되지는 않지만, 사람 속도의 반복 토글이 그 해체를 늦출 수는 있다(허용) |

> ⭐ **#18 은 M1 이 안고 머지된 실제 결함이었다.** M1 의 자동 테스트가 전부 `KeyDown`/`KeyUp` 으로만 이벤트를 만들어 이 경로를 건드리지 않았고, 이를 잡아냈어야 할 수동 검증 항목(`../dev/manual-verification.md` 항목 2·3)은 **환경설정 UI 가 없어 수행 자체가 불가능**했다. M2 1차에서 UI 가 생기자마자 첫 실행에서 드러났다 — 검증 절차의 공백이 코드의 공백과 정확히 겹쳐 있었다는 기록으로 남긴다.

## 6. 필요한 플랫폼 API

### 경로 A — CGEventTap

| API / 알림 | 용도 |
| :--- | :--- |
| `CGEventTapCreate(kCGSessionEventTap, kCGHeadInsertEventTap, kCGEventTapOptionDefault, mask, callback, userInfo)` | 탭 생성. `Default` 옵션 필수(소비·치환을 위해) |
| `CGEventTapEnable(tap, enable)` | 탭 재활성화(§3-a `Disabled` 상태 복구) |
| `CGEventTapIsEnabled` | 탭 생존 여부 조회 — 워치독 폴링에 쓰이는 것으로 보임(§3-a) |
| `CFMachPortCreateRunLoopSource` / `CFRunLoopAddSource` / `CFRunLoopRun` | 탭을 런루프에 등록하고 구동(§7 런루프 소유권 결정과 직결) |
| `CGEventGetIntegerValueField(event, kCGKeyboardEventKeycode)` | 물리 키코드 판독 — 모든 판정의 입력 (§3-b, §3-c, 레이아웃 독립성의 근거) |
| `CGEventGetIntegerValueField(event, kCGKeyboardEventAutorepeat)` | 키 반복 판별(엣지 케이스 13) |
| `CGEventCreateKeyboardEvent` / `CGEventCreateMouseEvent` / `CGEventSetType` / `CGEventGetFlags` / `CGEventSetFlags` / `CGEventSetIntegerValueField` / `CGEventPost` / `CGEventSourceCreate` / `CGEventGetLocation` | 치환 이벤트 합성 전반(단순 리매핑, 합성 modifier, 마우스 이벤트에 modifier 얹기 등) — 실측으로 전량 확인된 경로 A 심볼 |
| `CGEventKeyboardSetUnicodeString` | 물리 키코드로 표현 불가능한 문자를 강제로 얹는 폴백 경로(§3-b 계층 3/4 의 문자 출력, §7 결정 참조) |
| `IsSecureEventInputEnabled()` | Secure Input 상태 판별(§5 엣지 케이스 6) — 폴링 전용, 상태 변경을 알리는 공개 알림 API 는 확인되지 않음. **경로 A 에만 적용됨**(§5 엣지 케이스 6, §9) |
| `NSWorkspaceDidWakeNotification` / `NSWorkspaceWillSleepNotification` / `NSWorkspaceSessionDidBecomeActiveNotification` / `NSWorkspaceSessionDidResignActiveNotification` / `NSWorkspaceDidActivateApplicationNotification` | 절전·로그인·사용자 전환 시 탭 재확인 트리거, 마지막 알림은 §3-f 앱별 비활성화 게이트의 최전면 앱 추적용 |

### 경로 B — IOHID 커널 매핑

| API / 문자열 | 용도 |
| :--- | :--- |
| `IOHIDServiceClientSetProperty` | `HIDKeyboardModifierMappingSrc`/`Dst` 를 커널 HID 서비스에 직접 기록(§3-d) |
| `IOHIDEventSystemClientCreateSimpleClient` | HID 이벤트 시스템 클라이언트 생성 — 대상 서비스 열거의 전제 |
| `IOHIDServiceClientConformsTo` | 대상 HID 서비스(키보드) 판별 |
| `hidutil property -g/-set UserKeyMapping`(서브프로세스 실행, FFI 아님) | 경로 B 의 대체/병행 경로(§3-d, §7) — `Tools_Tools.bundle` 의 셸 실행 오류 문자열이 이 경로를 뒷받침 |
| ~~`IOHIDManagerRegisterDeviceMatchingCallback` / `RegisterDeviceRemovalCallback` / `SetDeviceMatching` / `ScheduleWithRunLoop`~~ | ~~키보드 핫플러그 감지 → 경로 B 재적용(§3-a, §5 엣지 케이스 10)~~ — ⭐ **대체됨, 아래 참조** |
| ⭐ `IOServiceAddMatchingNotification`(`kIOMatchedNotification` / `kIOTerminatedNotification`) + `IOServiceMatching(kIOHIDDeviceKey)` | **키보드 핫플러그 감지의 채택 수단**(§3-a, §5 엣지 케이스 10). 아래 판정 참조 |

⭐ **핫플러그 감지 수단 변경 (2026-08-30, M1 구현 / 이슈 #5).** 원본이 `IOHIDManager*` 계열을 링크한다는 실측(§3-a)은 그대로 유효하지만, **클론은 `IOServiceAddMatchingNotification` 을 쓴다.** 근거:

- `IOHIDManager` 계열은 **감지만 하려 해도 `IOHIDManagerOpen` 이 필요하고, 그것이 곧 Input Monitoring(TCC) 권한 요구**다. 그런데 이 문서와 F-11 이 함께 확정한 사실은 "원본은 Input Monitoring 을 명시적으로 확인하지 않고 `IOHIDManagerOpen` 실패를 재시도로 흡수한다"(§6 권한 표, F-11 §3.1)이다. 즉 명세대로 구현하면 **핫플러그 감지 하나를 위해 권한이 끝내 없을 때 영영 동작하지 않는 경로**를 만들게 되고, 그 사실이 사용자에게 보이지도 않는다.
- `IOServiceAddMatchingNotification` 은 **TCC 권한을 요구하지 않으면서** 같은 정보(키보드 HID 장치의 등장·소멸)를 준다. 매칭 딕셔너리는 `IOServiceMatching(kIOHIDDeviceKey)` 에 `kIOHIDPrimaryUsagePageKey = 1`(GenericDesktop) · `kIOHIDPrimaryUsageKey = 6`(Keyboard) 를 더해 좁힌다 — ⚠️ **이슈 #86 정렬 후 `DeviceUsagePage`/`DeviceUsage` 가 아니다**(`F-17` §3.2 hidutil list 정본과 같은 필터).
- **바뀌지 않는 것**: 감지 *이후*의 동작(경로 B 재적용, `keyboardConnectionDelay` 지연, 재시작 디바운스)은 §3-a·§5 #10 그대로다. 바뀌는 것은 감지 수단 하나뿐이다.
- ⚠️ 구현 함정: `IOServiceAddMatchingNotification` 은 첫 등록 시 **기존 장치 전부에 대해 즉시 콜백이 온다**. 반환된 이터레이터를 끝까지 비워야 이후 알림이 도착한다.

**기각한 대안** — `IOHIDManagerOpen` 을 재시도 루프로 흡수하며 명세대로 구현: 권한이 없으면 감지가 조용히 죽고, 그 상태가 관측되지 않는다. 상세는 `docs/dev/architecture.md` §3 "결정 2".

### 경로 C — HID 잠금 상태

| API | 용도 |
| :--- | :--- |
| `IOHIDGetModifierLockState` / `IOHIDSetModifierLockState` | caps lock 의 실제 잠금(대문자 고정·LED) 읽기/쓰기(§3-d) |

### 레이아웃 독립 판정

| API / 알림 | 용도 |
| :--- | :--- |
| `TISCopyCurrentKeyboardInputSource` | 현재 입력 소스 조회 |
| `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` | ⭐ 비-ASCII 입력기(CJK IME 등) 활성 시 ASCII 가능 레이아웃으로 폴백(§3-e, 신규 확정) |
| `TISGetInputSourceProperty(kTISPropertyUnicodeKeyLayoutData)` | 레이아웃 데이터 조회 |
| `UCKeyTranslate` | keycode+modifier → 문자 정방향 변환, 역방향 탐색 테이블의 기반 |
| `LMGetKbdType` | ⭐ 물리 키보드 하드웨어 타입 조회 — `UCKeyTranslate` 의 `iKeyboardType` 인자(§3-e, 신규 확정) |
| `kTISNotifySelectedKeyboardInputSourceChanged`(Distributed Notification, `CFNotificationCenterAddObserver`) | 입력 소스 변경 감지 → 레이아웃 역산 캐시 무효화 |

### 권한 확인 (F-11 소유, 참조만)

| API | 용도 |
| :--- | :--- |
| `AXIsProcessTrusted()` | ⭐ **실측: SuperKey 가 명시적으로 확인하는 유일한 권한.** 런타임 권한 취소 감지용 폴링(엣지 케이스 7)에도 쓰임. 권한 요청 자체는 F-11 소유 |
| ~~`IOHIDCheckAccess` / `IOHIDRequestAccess`~~ | ⭐ **실측 정정: 이 심볼은 링크되어 있지 않다.** Input Monitoring 권한을 명시적으로 확인하지 않는다. 대신 `IOHIDManagerOpen` 실패(`kIOReturnNotPermitted`)를 재시도로 흡수한다 — 문자열 "IOHIDManagerOpen failed with kIOReturnNotPermitted. Retrying... (Attempt "(SuperKey 원문) |
| ~~`AXIsProcessTrustedWithOptions`~~ | ⭐ **실측 정정: 링크되어 있지 않다.** 즉 시스템 프롬프트를 띄우지 않고, 자체 모달로 시스템 설정을 안내한다. 상세 온보딩 흐름은 F-11(`permissions-onboarding.md`) 소관이며, 이 문서는 "엔진이 의존하는 권한 상태"만 인지한다 |

---

## 7. 구현 접근

구현 접근은 프로젝트 공통 3분류로 판정한다 ([`README.md`](README.md#구현-접근-3분류) 참조). ⭐ 세 리매핑 경로가 서로 다른 판정을 받는다 — 아래 경로별로 나누어 판정한다.

- **순수 Rust** — 기존 안전(safe) 래퍼 크레이트만으로 완전히 커버되어 `unsafe` FFI 도 네이티브 shim 도 불필요
- **Rust 바인딩** — 크레이트는 존재하나 `unsafe` FFI(`objc2-*` 계열의 헤더 자동 생성 바인딩 또는 수기 `extern "C"` 선언) 직접 호출이 필요. 별도로 빌드하는 Swift/Objective-C 소스 파일은 없다
- **네이티브 shim 불가피** — Rust 에서 호출할 방법이 없어 Swift/Objective-C 소스를 별도로 빌드해 링크해야 함

### 경로 A — CGEventTap: Rust 바인딩

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

### 경로 B — IOHID 커널 매핑: 혼합(Rust 바인딩 + 프로세스 실행) ⭐

경로 B 는 두 구현 방식이 실측으로 공존한다(§3-d).

- **FFI 직접 호출** — `IOHIDServiceClientSetProperty` / `IOHIDEventSystemClientCreateSimpleClient` / `IOHIDServiceClientConformsTo` / `IOHIDManagerRegisterDeviceMatchingCallback` 류는 C ABI 함수이며 `IOKit.framework` 에 링크해 `extern "C"` 선언(또는 `objc2-io-kit` 류 바인딩, 전용 크레이트 존재 여부는 별도 확인 필요)으로 호출 가능하다 → **분류 B(Rust 바인딩)**.
- **`hidutil` 서브프로세스 실행** — ⭐ **판정: 이것은 FFI 가 아니라 프로세스 실행이며, README.md 의 3분류 어디에도 자리가 없다.** 3분류는 "Rust 에서 네이티브 API 를 어떻게 호출하는가"(라이브러리 호출의 안전성 정도)를 축으로 하는 분류다. `std::process::Command` 로 `hidutil` 을 실행하고 stdout/plist 를 파싱하는 것은 `unsafe` FFI 도, 별도로 빌드하는 네이티브 shim 도 필요 없다는 점에서 표면적으로는 순수 Rust 에 가장 가깝다. 그러나 "라이브러리를 안전하게 감싸 호출한다"는 3분류의 전제 자체가 성립하지 않는다 — 대신 **외부 실행 파일의 존재·경로·버전·출력 형식에 의존**하는 별개의 리스크(macOS 버전 간 `hidutil` 동작 차이, 셸 인용·파싱 오류, 서브프로세스 실행 자체의 권한/샌드박스 제약)를 진다. `Tools_Tools.bundle` 의 `bash command error: %@` / `Error parsing plist response` 문자열이 원본에서도 이 리스크가 실제로 존재해 에러 핸들링을 별도로 두었음을 보여준다. 이 문서의 판정: **프로세스 실행은 3분류 바깥의 별도 범주로 다루고, 클론 설계는 README.md 3분류표에 이를 각주로 남길 것을 제안한다**(§9).
- **원본이 왜 `hidutil` 서브프로세스를 병용하는가는 `(미확정)`.** 가능성: FFI 경로가 macOS 버전마다 불안정해 Apple 이 유지보수하는 CLI 를 폴백으로 쓰거나, 반대로 `hidutil` 이 1차 경로이고 FFI 는 상태 조회·확인용일 수 있다. ~~클론 설계는 **FFI 직접 호출을 1차로 채택**하되(에러 처리·테스트가 프로세스 실행보다 결정론적이다), `hidutil` 서브프로세스를 진단·폴백 경로로 남겨두는 것을 권장한다.~~

⭐ **판정 반전 (2026-08-30, M1 구현 / 이슈 #5).** 위 "FFI 1차" 권장을 **뒤집어 `hidutil` 서브프로세스를 1차 경로로 채택한다.** 근거:

1. **시그니처를 검증할 수 없다.** `IOHIDEventSystemClientCreateSimpleClient` 와 `IOHIDServiceClientSetProperty` 는 macOS SDK 의 **공개 헤더에 없는 심볼**이다(SDK 헤더 검색으로 확인). `platform-constraints.md` P6 이 `MultitouchSupport` 에 대해 남긴 경고 — "**시그니처를 추측으로 쓰지 말 것**" — 이 그대로 적용된다. 잘못된 시그니처의 `extern "C"` 호출은 컴파일과 테스트를 통과한 뒤 임의 시점에 UB 를 낸다.
2. **원래 판정의 근거가 조건부였다.** "FFI 가 더 결정론적" 이라는 선호는 **시그니처가 확정된 경우에만** 성립한다. 그리고 §9 #15 가 확정하듯 이 문서는 원본이 어느 쪽을 1차로 쓰는지 알지 못한다 — 즉 "FFI 1차"는 실측이 아니라 설계 선호였다.
3. **전환 비용이 지금 가장 싸다.** M1 에는 경로 B 로 배정된 규칙이 0 개다(개별 배정은 F-08/M2 소관, §3-d). 인터페이스(`HidMappingBackend` 트레이트)만 고정해 두면, 시그니처를 오픈소스 구현체로 대조 검증한 뒤 M2 에서 구현체만 갈아끼울 수 있다.

**기각한 대안** — ① 추측 시그니처로 FFI 를 지금 구현: 위 근거 1. ② 경로 B 자체를 M2 로 미룸: **정리(cleanup)와 잔존 매핑 감지는 앱이 처음 출하되는 순간부터 있어야 한다**(§3-a2, §5 #15). M2 에서 처음 켜면 그 전 버전이 남긴 시스템 전역 매핑을 아무도 치우지 않는다.

**완화 조치**: 셸을 거치지 않고 `Command::new("hidutil")` 로 인자를 직접 넘겨 원본이 겪은 셸 인용 오류(`bash command error: %@`)를 구조적으로 배제한다. 반전의 상세 근거는 `docs/dev/architecture.md` §3 "결정 1" 에 있다.

### 경로 C — HID 잠금 상태 직접 조작: Rust 바인딩

`IOHIDGetModifierLockState` / `IOHIDSetModifierLockState` 는 C ABI 함수로, `IOKit.framework` 링크 후 `extern "C"` 선언으로 호출 가능하다. 전용 안전 래퍼 크레이트는 확인되지 않았다 `(미확정)` → **분류 B(Rust 바인딩)**. 경로 A 와 별도의 코드 경로이지만 요구되는 unsafe 표면 자체는 작다(함수 2개).

### 종합 판정

이 엔진 전체의 구현 접근은 **Rust 바인딩(분류 B)** 이 중심이되, 경로 B 한정으로 **프로세스 실행(3분류 바깥의 범주)** 이 보조적으로 관여한다. 네이티브 shim(분류 C)은 세 경로 어디에도 필요하지 않다 — 모든 API 가 C ABI 로 노출되어 Rust 에서 직접 FFI 로 해결 가능하기 때문이다.

---

## 8. 수용 기준

- [ ] `CGEventTapCreate` 가 `kCGSessionEventTap` / `kCGHeadInsertEventTap` / `kCGEventTapOptionDefault` 로 생성되며, keyDown/keyUp/flagsChanged 및 Hyperkey 마우스 옵션에 해당하는 이벤트 타입을 마스크에 포함한다.
- [ ] 탭이 `kCGEventTapDisabledByTimeout` 또는 `kCGEventTapDisabledByUserInput` 을 수신하면 비활성화 통지에 재활성화를 시도한다 — 단 **연속 유한 횟수까지**이며, 카운터는 실제 이벤트 통과로만 리셋된다(시간으로 리셋되지 않는다). 소진되면 재활성화를 멈추고 탭을 해체해 권한 모델에 넘긴다(재생성을 시도하지 않는다) — 이슈 #65 Phase 1 리뷰 결정 1, `event_tap.rs` 의 `ReenableBudget` 참고.
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
- [ ] ⭐ 경로 B(IOHID 커널 매핑) 규칙은 앱 정상 종료 시 명시적으로 정리(cleanup)되고, 비정상 종료로 정리가 되지 않은 경우 다음 실행 시작 시 잔존 매핑을 감지해 현재 설정과 재조정한다(§3-a2).
- [ ] ⭐ 경로 C(HID 잠금 상태)로 구현되는 규칙(`Double tap shift = caps lock` 류)은 이벤트 합성이 아니라 `IOHIDSetModifierLockState` 로 실제 caps lock 잠금을 토글한다(§3-d).
- [ ] ⭐ 권한 확인이 확실히 통과한 상태에서 `CGEventTapCreate` 자체가 실패하는 경우, 무한 재시도 루프에 빠지지 않고 명시적인 치명적 실패 경로로 처리된다(§3-a, §5#16).
- [ ] ⭐ 절전/로그인/세션전환 복귀 시 탭 재확인은 즉시 실행되지 않고 지연(delay) 뒤에 실행되며, 직전 재시작으로부터 짧은 시간 안의 중복 재확인은 디바운스로 억제된다(§3-a).
- [ ] ⭐ 외장 키보드 연결/해제 감지 시 경로 B 리매핑이 재적용된다(§3-a, §5#10).
- [ ] ⭐ 최전면 앱이 비활성화 목록에 있는 동안에는 §3-b 의 어떤 계층도 평가되지 않고 원본 이벤트가 그대로 통과한다(§3-f).
- [ ] ⭐ 레이아웃 독립 판정은 현재 입력 소스가 ASCII 가능하지 않을 때 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 로 폴백한 레이아웃을 사용한다(§3-e).
- [ ] ⭐ **(이슈 #65 Phase 2, 신규)** 권한이 `Denied`/`OutOfSync`/`Unknown` 으로 전이하면 앱은 엔진 전체(탭 스레드·워치독·시스템 훅·지연 스케줄러)를 정지시킨다 — 살아있는 탭이 있어도 예외 없이. 이후 `Granted` 로 전이하면 완전히 새 `Engine` 을 기동해 탭을 재생성한다(`apps/ultrakey-app` 의 `on_permission_transition`/`start_engine_if_needed`/`stop_engine`). 권한을 회수했다가 재부여했을 때 **앱을 재시작하지 않아도** 리매핑이 실제로 복귀해야 한다(§3-a `Disabled` 행, `docs/dev/manual-verification.md` 신규 항목).

---

## 9. 미해결 질문

⭐ **이번 실측으로 해소된 질문** — 이전 버전의 다음 두 항목은 실측으로 해소되어 표에서 제거한다: "`Quick press duration` 슬라이더의 정확한 최소/최대/간격"(→ §4 에서 최소 250ms·최대 2000ms·현재값 1000ms 로 확정, step 간격만 아래 #1 로 승계), "hyper quick press 존재 여부"(→ §3-c 에서 존재 확정, 노출 경로만 아래 #5 로 승계).

| # | 질문 | 현재 처리 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | `Quick press duration` 슬라이더의 정확한 step 간격 | 최소 250ms·최대 2000ms·현재값 1000ms 는 실측으로 확정(§4, AX 트리). **step 간격만 미확정.** 이전 버전이 "8칸 눈금 역산"으로 200ms 간격을 추정했던 것은 **틀렸다** — 눈금 개수로부터의 역산은 신뢰할 수 없다는 교훈을 남긴다(§4) | 슬라이더를 실제로 드래그하며 각 정지점의 라벨 값을 읽는다(AX 의 min/max 값 속성만으로는 연속 슬라이더의 스텝을 알 수 없다) |
| 2 | Double tap 최대 간격의 실제 ms 값 | UI 노출이 없다는 것은 확정(실측: AX, 4개 탭 전 컨트롤 확인). 정확한 내부 상수값은 여전히 `(미확정)`, 300ms 는 근거 없는 추정치로 유지 | 실제 키 입력 타이밍 실험(다양한 간격으로 double tap 반복해 임계점 이분 탐색) |
| 3 | 세 리매핑 경로(A/B/C)의 개별 프리셋별 정확한 배정 | §3-d 가 판정 **기준**은 확정했으나, F-08 16종 프리셋 각각이 실제로 어느 경로로 구현되는지 전수 확인은 못 했다. caps lock 잠금 토글 계열(경로 C)만 심볼로 확정 | 동적 트레이싱(`dtrace`, API 호출 로깅) — 이번 조사는 정적 분석과 AX 관찰만 했다(디컴파일 금지 원칙) |
| 4 | `Advanced ▸ Synthesize Caps Lock Remap` 의 정확한 의미·동작 | `(미확정 — 라벨로부터의 해석)`. "기본은 경로 B/C, 스위치를 켜면 경로 A로 강제 전환"으로 해석했으나 토글 전후 실제 동작을 관찰하지 못했다(§3-d) | 토글 전후 `hidutil property -g UserKeyMapping` 출력 비교, caps lock 프리셋 동작 재관찰 |
| 5 | hyper quick press(`quickHyperKeycode`/`executeQuickHyperKey`/`hyperDownTime`)의 UI 노출 경로와 정확한 의미 | 존재는 확정(실측: 번들 문자열, §3-c). 4개 탭 어디에도 대응 컨트롤을 찾지 못했다 — 숨은 설정이거나 조건부 표시일 가능성 | 조건부 표시 후보(다른 설정 조합 시 나타나는지) 재현, 또는 이 키들을 직접 조작 후 UI 재관찰 |
| 6 | `longPressCapsLockTurnsItOff` 의 정확한 규칙 | `(미확정 — 이름으로부터의 해석)`. "caps lock 을 길게 누르면 잠금 해제"로 추정(§3-c) | 대응 UI 탐색, 또는 실제 caps lock 을 길게 눌러 거동 관찰 |
| 7 | 워치독("Checking key loop.")의 폴링 주기, 재시작 디바운스("Time since last exit…")의 정확한 임계값(초) | `(미확정)`. 존재와 메커니즘만 문자열로 확정(§3-a) | 로그 활성화(메뉴바 `Show Logging…`/`Log to File`) 후 타임스탬프 간격 측정 |
| 8 | 경로 B(IOHID 커널 매핑)·경로 C(HID 잠금 상태)가 Secure Input 구간에서도 유효한가 | `(미확정)`. §5 엣지 케이스 6 에서 "영향받지 않을 가능성"으로만 서술, 단정하지 않는다 | 로그인 다이얼로그 등 Secure Input 활성 창에 포커스를 둔 채 caps lock 프리셋의 실제 동작 관찰 |
| 9 | `CFRunLoopSource` 를 전용 스레드에 두는 결정(§7)의 실측 검증 | 설계 근거는 있으나 실제 타임아웃 발생률·지연시간 미측정 | 프로토타입 구현 후 트래킹 모드 상황(메뉴 열기 등)에서 hyper 키 반응성 측정 |
| 10 | 동일 키 중복 배정 UI 경고의 정확한 문구·배지 디자인 | 결정은 §3-b 에서 내렸으나(런타임 우선순위 + 비차단 경고) 구체적 UI 카피는 미정 | F-05/F-08 UI 설계 시점에 확정 |
| 11 | Karabiner-Elements 등 타 리매퍼와 동시에 이벤트 탭이 설치되었을 때 macOS 가 실제로 어떤 순서로 콜백 체인을 호출하는지 | 엣지 케이스 12 에서 "구조적 한계로 통제 불가"로만 서술 | 실제 두 앱을 함께 실행해 관찰 |
| 12 | `IsSecureEventInputEnabled()` 를 매 콜백마다 확인할지, 별도 주기로 폴링할지 | 이 문서는 매 콜백 확인을 전제로 서술(오버헤드가 작은 syscall 이라는 가정) | 실측으로 오버헤드 확인, 필요 시 캐시+주기 폴링으로 전환 |
| 13 | 외장(비-Apple) 키보드에서 caps lock 등 주요 소스 키의 물리 keycode 가 Apple 내장 키보드와 다를 가능성 | 엣지 케이스 10 에서 미해결로 남김. 핫플러그 시 경로 B 재적용 로직 자체는 이번에 확정됨(§3-a) | 실제 외장 키보드 다수로 keycode 로그 비교 |
| 14 | 콜백이 강제 종료되기까지의 정확한 타임아웃 값(Apple 비공개) | §3-a 에서 "경험적으로 수백 ms 수준"으로만 서술 | 실측(의도적으로 콜백을 지연시켜 타임아웃 발생 시점 측정) |
| 15 | `hidutil` 서브프로세스 실행과 `IOHIDServiceClientSetProperty` FFI 직접 호출 중 **원본이** 실제로 어느 쪽을 1차 경로로 쓰는가 | `(미확정)` — 원본에 대한 질문으로는 그대로 남는다. ⭐ **다만 클론의 선택은 확정됐다**: `hidutil` 1차(§7 판정 반전). 원본이 무엇을 쓰는지와 무관하게 클론은 검증 가능한 경로를 택했다 | 동적 트레이싱, 또는 `hidutil` 프로세스 스폰 여부를 로그로 관찰 |
| 16 ⭐ | `IOHIDEventSystemClientCreateSimpleClient` / `IOHIDServiceClientSetProperty` 의 **정확한 시그니처** | `(미확정)`. macOS SDK 공개 헤더에 없다 — 이것이 §7 판정 반전의 직접 근거다. 클론은 시그니처를 추측해 선언하지 않았다 | 오픈소스 구현체(예: `hidutil` 을 대체하는 공개 프로젝트) 대조. `platform-constraints.md` P6 과 같은 방법론이며, **확인 전까지 선언하지 않는다** |
| 17 ⭐ | `IOServiceAddMatchingNotification` 기반 핫플러그 감지가 `IOHIDManager` 계열과 **동등한 이벤트를 주는가** | 클론이 §6 에서 채택한 대체 수단. 원리상 같은 IOKit 레지스트리 이벤트이지만, 외장 키보드·블루투스·KVM 스위치 등에서 실제로 동일하게 발화하는지는 미검증 | 실제 외장 키보드(USB·Bluetooth)를 연결·해제하며 콜백 발화 로그 비교 |
| 18 ⭐ | 절전/세션/키보드연결 지연값과 재시작 디바운스 임계값의 **실제 적정치** | #7 이 "원본의 값"을 묻는 반면 이것은 "우리 값이 맞는가"를 묻는다. M1 은 설계 판단으로 2000/1000/1500/5000 ms 를 채택했다(`docs/dev/architecture.md` §4) — **실측이 아니다** | 절전 복귀·잠금 해제·핫플러그 직후 리매핑 동작 여부를 지연값을 바꿔가며 관찰 |
