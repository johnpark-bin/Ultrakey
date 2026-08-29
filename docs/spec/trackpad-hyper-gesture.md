# F-06 · 트랙패드 원터치 hyper 제스처

> **한 줄 요약**: 트랙패드의 지정된 모서리 또는 상단 가장자리에서 손가락 하나를 안쪽으로 짧게 슬라이드하면 hyper 키가 활성화되고, 손가락을 떼면 즉시 해제되는 — 물리 키를 소모하지 않는 hyper 키의 **대체 활성화 경로**다.
> **의존성**: `F-05`(hyperkey.md)가 정의하는 hyper/meh/bleh modifier 조합의 **소스 중 하나**로 동작한다 — F-06 은 modifier 조합 자체를 정의하지 않고, "hyper 를 활성화하라"는 신호만 F-05 방향으로 방출한다(§3.4). 그 신호가 실제 `CGEvent` modifier flag 로 합성·중재되는 경로는 `F-07`(이벤트 탭·중재) 소관이다. 환경설정 창의 `Hyperkey` 탭 UI 배치는 `F-09`, Accessibility·Input Monitoring 권한 획득은 `F-11` 소관이다.
> **관련 명세**: hyper/meh/bleh 조합 정의 → `F-05`(hyperkey.md). 이벤트 탭·중재 → `F-07`. 환경설정 창 UI → `F-09`. 권한 → `F-11`.

---

## 1. 개요

Superkey 는 hyper 키(⌃⌥⌘⇧)를 얻는 기본 경로로 물리 키 하나(Caps Lock 등)를 소모하는 리매핑을 쓴다. 트랙패드 원터치 제스처는 이 소모를 피하고 싶은 사용자를 위한 **대체 경로**다. 랜딩 페이지는 이를 3대 기능 소개에 파묻힌 별도 섹션으로 다룬다(superkey-inventory.md §1.2):

- "No need to sacrifice a key" / "Don't want to sacrifice a key?"
- "Use a simple one touch gesture built just for activating the hyper key"
- **"Slide one touch down from a corner or the top edge of the trackpad to activate and remove the touch to release"**

환경설정 `Hyperkey` 탭 스크린샷은 이 기능을 `Engage hyper key using trackpad:` 체크박스 + 영역 선택 팝업(스크린샷 값 `top right`)으로 노출하며, 부제는 랜딩 문구보다 더 구체적인 동작 설명을 준다: **"Slide only one touch in from the selected area the trackpad a little. Remove the touch to release."**(superkey-inventory.md §3.2)

설계 의도는 hyper 키를 물리 키 없이 즉시 손끝으로 트리거해, 같은 제작자의 다른 앱과 원터치로 연동하는 것이다: "Designed to enable one touch control of your windows with Rectangle Pro or app switching with Charmstone"(superkey-inventory.md §1.2). 즉 F-06 이 방출하는 hyper 활성 신호는 F-05/F-07 을 거쳐 다른 앱의 hyper-조합 단축키(예: Rectangle Pro 의 창 배치 단축키)를 트리거하는 데 쓰인다 — F-06 자신은 그 단축키가 무엇인지 알지 못한다.

이 기능은 **조사 근거가 이 프로젝트에서 가장 얇다.** 확인된 1차 원문은 위 인용 두 건과 체크박스/팝업 UI 뿐이며, 제스처 인식의 정량적 임계값(진입 영역 크기, 최소 이동 거리, 손가락 수 상한 등)은 어디에도 명시되어 있지 않다. 아래 전체 문서에서 `(추정)` 표시는 특히 신중하게 읽어야 한다.

## 2. 사용자 시나리오

### 시나리오 A — 상단 가장자리에서 슬라이드해 hyper 로 창 배치(Rectangle Pro 연동)

전제: `Engage hyper key using trackpad:` = ☑, 영역 = `top edge` `(추정 — §4 참조, 스크린샷 확인값은 top right)`.

1. 사용자가 아무 앱에서나 작업 중, 트랙패드 상단 가장자리에 손가락 하나를 대고 안쪽(화면 방향, 아래)으로 살짝 슬라이드한다.
2. 이동량이 임계값을 넘는 순간 hyper 가 활성화된다(F-05/F-07 로 신호 방출).
3. 손가락을 뗀 채로(터치는 유지) hyper 조합에 바인딩된 Rectangle Pro 단축키(예: hyper+←)를 눌러 창을 좌측 절반으로 배치한다.
4. 트랙패드에서 손가락을 뗀다. hyper 가 즉시 해제된다.

### 시나리오 B — 코너에서 슬라이드해 Charmstone 앱 전환

전제: `Engage hyper key using trackpad:` = ☑, 영역 = `top right`(스크린샷 확인값).

1. 사용자가 트랙패드 우측 상단 모서리에 손가락 하나를 대고 안쪽으로 슬라이드한다.
2. hyper 활성화. 이어서 hyper+Tab 등 Charmstone 에 바인딩된 조합을 눌러 앱을 전환한다.
3. 손가락을 떼 hyper 를 해제한다.

### 시나리오 C — 제스처 미완성(짧은 탭)

전제: 시나리오 A/B 와 동일 설정.

1. 사용자가 지정 영역에 손가락을 댔다가, 안쪽으로 충분히 밀기 전에 바로 뗀다(우발적 접촉, 또는 트랙패드를 짚고 지나가는 손).
2. 이동 임계값에 도달하지 못했으므로 hyper 는 활성화되지 않는다 — 아무 일도 일어나지 않는다.

### 시나리오 D — 트랙패드가 없는 기기

전제: 데스크톱 Mac(Mac mini/Mac Studio/iMac 등)에 Magic Mouse 또는 외장 키보드만 연결, 내장·외장 트랙패드 없음.

1. 환경설정 `Hyperkey` 탭을 연다.
2. `Engage hyper key using trackpad:` 체크박스는 활성화할 트랙패드 장치가 없으므로 사용할 수 없는 상태로 표시된다(비활성화 또는 안내 문구 — 정확한 UI 처리는 `(추정)`, §5·§9 참조).
3. 사용자가 이후 Magic Trackpad 를 블루투스로 연결하면, 체크박스가 사용 가능해진다(§5 참조).

## 3. 동작 명세

### 3.1 원칙

- 이 기능이 인식하는 유효 접촉은 **정확히 손가락 1개**여야 한다. 랜딩 페이지("one touch gesture")와 스크린샷 부제("Slide **only one** touch in...")가 모두 이를 직접 명시한다 — 이 부분은 `(추정)` 이 아니라 **확인된 원문 근거**다.
- 제스처는 **진입 영역**(모서리 또는 상단 가장자리 중 선택된 하나)에서 **시작**해야 하고, **안쪽 방향**으로의 이동이어야 한다. 영역 밖에서 시작한 접촉이나 바깥쪽으로의 이동은 무시한다.
- hyper 활성화는 이동 임계값 충족 **즉시** 일어나고(정확한 타이밍은 §9), 해제는 접촉이 끝나는 순간(finger up) 발생한다 — 부제의 "Remove the touch to release" 는 지연 없는 즉시 해제로 읽는다.
- F-06 은 **손가락의 원시 위치·상태**를 프레임 단위로 알아야 하므로(§6), 공개 API 로는 표현할 수 없는 요구사항이다. 이 문서의 §6·§7 이 그 함의를 다룬다.

### 3.2 상태 머신

이름 붙인 파라미터: `ENTRY_ZONE`(진입 영역의 물리적 경계), `MIN_SLIDE_DISTANCE`(활성화에 필요한 최소 이동 거리), `MAX_TOUCH_COUNT = 1`(허용 동시 접촉 수 상한 — 확인된 값), `GESTURE_TIMEOUT`(접촉 시작 후 임계 미도달 시 포기 시간).

| 현재 상태 | 입력 | 조건 | 다음 상태 | 부수효과 |
| :--- | :--- | :--- | :--- | :--- |
| 대기(Idle) | 새 접촉(finger down) 감지 | `Engage hyper key using trackpad:` = ☑ · 접촉 좌표가 `ENTRY_ZONE`(선택된 코너/가장자리) 내 `(추정)` · 동시 접촉 수 = 1 | 진입접촉감지(ZoneContact) | 접촉 ID·시작 좌표·시작 시각 기록 |
| Idle | 새 접촉 감지 | 접촉 좌표가 `ENTRY_ZONE` 밖 | Idle 유지 | 부수효과 없음(무시) |
| Idle | 새 접촉 감지 | `Engage hyper key using trackpad:` = ☐, 또는 이 트랙패드에 대해 아직 `MTDeviceStart` 미실행 | Idle 유지 | 부수효과 없음 |
| ZoneContact | 동일 접촉 ID 가 이후 프레임에서 안쪽 방향으로 누적 `MIN_SLIDE_DISTANCE`(추정) 이상 이동 | 접촉 수 여전히 1(추가 접촉 없음) | hyper 활성(Engaged) | **hyper 활성 신호 방출**(§3.4) |
| ZoneContact | 임계 도달 전 접촉 소멸(finger up) | — | Idle | 부수효과 없음 — §5 "제스처 미완성" |
| ZoneContact | 임계 도달 전 두 번째 접촉 추가 | — | Idle(취소) | 부수효과 없음 — §5 "다른 제스처와의 오인" |
| ZoneContact | `GESTURE_TIMEOUT`(추정) 경과, 임계 미도달 | — | Idle | 접촉 상태 폐기 |
| Engaged | 동일 접촉 ID 유지(터치가 표면에 계속 있음) | — | Engaged 유지 | 부수효과 없음(hyper 유지) |
| Engaged | 접촉 소멸(finger up) 감지 | — | 해제됨(Released) | **hyper 해제 신호 방출**(§3.4) |
| Engaged | 두 번째 접촉 추가 감지(예: 다른 손가락이 우발적으로 트랙패드에 닿음) | — | Engaged 유지 `(추정)` 또는 즉시 강제 해제 `(추정)` | 정책 미확정 — §9 |
| Engaged | 트랙패드 장치 핸들 무효화(절전 복귀, 외장 트랙패드 분리 등) | — | Released(강제) | hyper 강제 해제 신호 방출 — §5 |
| Released | (내부 전이) | — | Idle | 접촉 ID 바인딩 해제 |

### 3.3 진입 영역과 슬라이드 방향의 해석

랜딩 페이지와 스크린샷 부제가 서로 다른 표현을 쓴다는 점에 유의해야 한다:

- 랜딩: "Slide one touch **down** from a corner or the top edge" — "아래로"
- 스크린샷 부제: "Slide only one touch **in** from the selected area ... **a little**" — "안쪽으로, 조금"

상단 가장자리에서는 "아래로" = "안쪽으로"가 같은 방향이지만, **코너**에서는 "아래로"만으로는 코너에서 트랙패드 중앙을 향한 대각선 이동을 설명하지 못한다. 이 문서는 코너의 경우 "안쪽(대각선)"으로, 상단 가장자리의 경우 "아래(수직)"으로 해석한다 `(추정)` — 두 원문을 조화시킨 것이며 확정된 근거는 아니다(§9).

### 3.4 hyper 활성 신호 방출 계약

F-06 은 `CGEvent` 의 modifier flag 를 직접 합성하지 **않는다**. F-06 이 하는 일은 상태 머신이 `Engaged`/`Released` 로 전이할 때 F-05 방향으로 **"트랙패드 소스의 hyper 요청 상태"**(불리언: 활성/비활성)를 알리는 것뿐이다.

- F-05(hyperkey.md)는 hyper 조합(⌃⌥⌘⇧, meh, bleh)을 정의하는 소비자다. F-05 관점에서 "hyper 를 켜라"는 요청은 물리 키 소스(Caps Lock 등 리매핑된 키)와 트랙패드 소스 **두 곳에서 올 수 있다** — F-06 은 그중 하나다.
- 두 소스가 동시에 활성(물리 키를 누른 채 트랙패드 제스처도 활성)인 경우의 병합 규칙(OR 로 합쳐 하나의 hyper 상태로 취급하는 것이 자연스러운 기본값이나, 확정된 근거는 없다)은 F-05/F-07 의 몫이며 F-06 은 자신의 소스 상태만 정직하게 보고한다 `(추정)`.
- 실제 `CGEvent` modifier flag 삽입·이벤트 탭 우선순위 중재는 F-07 소관이다. F-06 은 F-07 이 노출하는 "가상 modifier 소스 등록" 인터페이스를 통해 신호를 전달한다고 가정한다 — 그 인터페이스의 구체 형태는 F-07 명세에서 정의되어야 하며, 이 문서에서 API 시그니처를 짓지 않는다.
- hyper 가 F-06 경로로 활성화된 상태에서 마우스/키보드 이벤트에 modifier 를 얹을지 여부는 `Apply modifiers to keypress events and: Click/Drag/Move/Scroll` 체크박스(superkey-inventory.md §3.2, F-05 범위)의 일반 규칙을 그대로 따른다고 가정한다 `(추정)` — F-06 특유의 예외는 조사 자료에 없다.

## 4. 설정 항목

| 이름(원문 라벨) | 타입 | 확인된 값 / 기본값 | 유효 범위 | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| `Engage hyper key using trackpad:` | 체크박스 | 스크린샷 값 ☐(꺼짐). 출고 기본값 미확인(스크린샷은 홍보용 구성일 수 있음, superkey-inventory.md §3.4) | ☑ / ☐ | superkey-inventory.md §3.2 |
| (영역 선택 팝업, 명시적 라벨 없음 — 체크박스 옆 팝업 버튼) | 팝업 버튼 | 스크린샷 값 `top right` | 확인된 값은 `top right` 하나뿐. 랜딩 문구 "from a corner or the top edge" 로부터 후보군을 `(추정)` 으로 제안: `top left` · `top right` · `bottom left` · `bottom right`(4 코너) + `top edge`(상단 가장자리 전체) — 근거: "corner **or** the top edge" 가 코너류와 가장자리류를 별개 항목으로 구분해 말하고 있음 | superkey-inventory.md §3.2 |

### 내부 인식 임계 파라미터(모두 `(추정)`, 조사 자료에 수치 없음)

| 파라미터 | 의미 | 제안값 `(추정)` | 근거 |
| :--- | :--- | :--- | :--- |
| `ENTRY_ZONE` | 진입 영역의 물리적 경계(코너: 정사각형, 가장자리: 띠) | 코너: 약 15mm × 15mm 정사각형 / 상단 가장자리: 트랙패드 상단을 따라 폭 약 15mm 띠 | 부제 "a little"(작은 이동)에 맞춰 진입 판정 영역도 좁게 잡아야 손가락을 트랙패드 중앙에서 쓸 때 오작동하지 않는다는 추론. 수치 자체는 근거 없음 |
| `MIN_SLIDE_DISTANCE` | 활성화로 판정하는 최소 누적 이동 거리 | 약 5–10mm | 부제 "Slide ... in ... a little" — "조금"이라는 표현이 큰 이동을 요구하지 않음을 시사할 뿐, 정량값은 없음 |
| `MAX_TOUCH_COUNT` | 인식 유지에 허용되는 동시 접촉 수 | **1** (확인됨) | "one touch gesture" / "Slide only one touch" — 원문 직접 인용, `(추정)` 아님 |
| `GESTURE_TIMEOUT` | 접촉 시작 후 임계 미도달 시 포기까지의 시간 | 약 300–500ms | 조사 자료에 근거 없음. 유사 트랙패드 제스처(macOS 시스템 제스처) 일반 관례로부터의 추론 |
| 방향 허용 각도(각도 허용치) | "안쪽" 판정의 각도 허용 범위 | 시작점 기준 트랙패드 중심 방향 ±30° `(추정)` | §3.3 의 "down" vs "in" 불일치를 조화시키기 위한 제안. 근거 없음 |

## 5. 엣지 케이스와 실패 모드

1. **트랙패드가 없는 기기.** 데스크톱 Mac + Magic Mouse 또는 외장 키보드만 있는 구성에서는 `MTDeviceCreateList()`(§6)가 반환하는 장치 목록이 비어 있다. `Engage hyper key using trackpad:` 체크박스는 비활성화되거나(선택 불가) 켜져 있어도 아무 접촉 프레임도 들어오지 않아 항상 무효과 상태가 되어야 한다. 정확한 UI 처리(비활성화 vs 조용히 무시)는 F-09 소관이나, F-06 로직은 "장치 목록이 비어 있으면 이 기능은 절대 hyper 를 활성화하지 않는다"는 불변식을 지켜야 한다.
2. **외장 Magic Trackpad 연결.** 세션 도중 블루투스 Magic Trackpad 가 새로 연결되면, `MTDeviceCreateList()` 를 다시 호출해 장치 목록을 갱신하고 새 장치에도 콜백을 등록해야 한다. 연결 이벤트를 감시하는 구체 메커니즘은 조사 자료에 없다 `(추정)` — §9.
3. **외장 트랙패드 연결 해제(도중 포함).** hyper 가 `Engaged` 상태인 도중 사용하던 트랙패드가 물리적으로 분리되면(배터리 소진, 블루투스 끊김), 해당 장치의 접촉 스트림이 그냥 끊길 뿐 명시적 finger-up 이벤트가 오지 않을 수 있다. 이 경우 강제 해제 처리가 없으면 hyper 가 **stuck**(항목 6) 상태로 남는다 — 장치 무효화 감지 즉시 강제 `Released` 로 전이해야 한다(§3.2 표).
4. **다른 트랙패드 제스처·시스템 제스처와의 오인.** 사용자가 코너/가장자리 근처에서 스와이프(예: 4손가락 Mission Control 스와이프, 3-finger drag)를 시작하는 경우, 첫 접촉이 진입 영역 안에서 시작될 수 있다. `MAX_TOUCH_COUNT = 1` 규칙(§3.2, §4)이 이 상황의 1차 방어선이다 — 두 번째 이상의 접촉이 감지되면 즉시 취소한다. 그러나 시스템 제스처가 먼저 macOS 자체에 의해 소비되어(트랙패드 제스처 인식은 OS 커널/드라이버 레벨에서도 동시에 일어남) F-06 이 애초에 원시 프레임을 못 받을 가능성도 있다 `(추정)` — §9.
5. **3-finger drag 등 시스템 제스처가 활성화되어 있는 상태.** 사용자가 시스템 설정에서 3-finger drag(트랙패드로 창 드래그)를 켜 놓은 경우, 코너 진입 접촉이 드래그의 일부로 소비될 가능성이 있다. F-06 은 이 충돌을 해소할 권한이 없다(둘 다 같은 원시 멀티터치 스트림을 관찰하는 별개 소비자) — 사용자에게 영역 선택을 바꾸도록 안내하는 것이 유일한 완화책이다 `(추정)`.
6. **손가락을 뗐는데 이벤트를 놓쳐 hyper 가 stuck.** `MTRegisterContactFrameCallback` 콜백이 프레임을 유실하거나(시스템 부하, 콜백 스레드 지연) finger-up 프레임 자체가 전달되지 않으면 상태 머신이 영원히 `Engaged` 에 머물러 hyper 가 눌린 채로 고착된다. 워치독이 필요하다: 마지막 유효 프레임 수신 후 일정 시간(`(추정)`, 예: 2초) 동안 새 프레임이 없으면 강제로 `Released` 로 전이한다.
7. **절전 복귀 후 장치 핸들 만료.** superkey-inventory.md §2.1(v1.58)이 `CGEventTap` 이 절전 복귀·로그인 시 재설치가 필요하다고 확정한 것과 같은 계열 문제가 `MTDevice` 핸들에도 적용될 가능성이 높다 `(추정)` — MultitouchSupport 는 비공개 프레임워크라 이 동작이 문서화되어 있지 않다. 절전 복귀(`NSWorkspace.didWakeNotification` 등 공개 알림) 수신 시 `MTDeviceCreateList()`/`MTRegisterContactFrameCallback()`/`MTDeviceStart()` 시퀀스를 전부 재실행하는 방어적 설계가 필요하다.
8. **여러 트랙패드 동시 연결.** 내장 트랙패드 + 외장 Magic Trackpad 가 동시에 연결된 구성에서, `MTDeviceCreateList()` 는 둘 다 반환할 가능성이 높다 `(추정)`. F-06 은 어느 한 장치가 아니라 **연결된 모든 트랙패드 장치**에 콜백을 등록해야 어느 쪽에서 제스처를 해도 반응한다. 두 장치에서 동시에 진입 영역 접촉이 발생하는 경우(사실상 2인 이상 동시 사용 등 희귀 상황)의 처리 정책은 조사 자료에 없다 `(추정)`.
9. **OS 업데이트로 비공개 API 시그니처가 바뀜.** MultitouchSupport 는 공개 헤더가 없는 비공개 프레임워크이므로, macOS 버전이 올라가며 함수 시그니처·콜백 구조체 레이아웃·심지어 함수 존재 자체가 예고 없이 바뀔 수 있다. 최소한 앱 시작 시 심벌 로드(`dlsym` 또는 링크 타임 심벌 해석) 실패를 감지해, 실패 시 이 기능 전체를 조용히 비활성화(체크박스 비활성화 안내)하는 방어 코드가 **필수**다 — 크래시로 이어지면 안 된다.
10. **화면 잠금.** 화면이 잠긴 동안(로그인 화면, 화면 보호기)에는 hyper 를 활성화해도 어떤 앱에도 전달할 대상이 없다. F-06 자체가 잠금 상태를 감지해 제스처 인식을 끌지, 그냥 신호만 방출하고 F-07 이 잠금 중 이벤트 주입을 억제할지는 F-07 의 책임 경계와 겹친다 — F-06 은 잠금 여부와 무관하게 원시 제스처 인식만 계속하고, 억제는 F-07 이 한다고 가정한다 `(추정)`.
11. **팜 리젝션(palm rejection).** 타이핑 중 손바닥 아랫부분이 트랙패드 상단 가장자리에 스치는 경우가 실제로 흔하다. macOS 드라이버 레벨의 팜 리젝션이 이런 접촉을 이미 걸러낼 수도 있으나, `MultitouchSupport` 콜백이 드라이버의 팜 리젝션 **이전** 원시 데이터를 주는지 **이후** 데이터를 주는지는 확인되지 않았다 `(추정)`. 이후 데이터라면 F-06 은 추가 조치가 필요 없고, 이전 데이터라면 오작동(의도치 않은 hyper 활성화) 가능성이 실사용에서 두드러질 것이다 — §9.
12. **`Engage hyper key using trackpad:` 를 끈 상태에서 진행 중이던 제스처.** 사용자가 설정 창에서 이 체크박스를 실시간으로 끄는 순간 이미 `Engaged` 상태였다면, 즉시 강제 `Released` 로 전이해 hyper 를 해제해야 한다(설정 변경이 상태 머신에 즉시 반영되어야 stuck 상태를 막을 수 있다).

## 6. 필요한 플랫폼 API

⭐ 이 기능의 핵심 기술 문제는 **개별 손가락의 원시 위치·상태에 대한 접근**이다. macOS 는 이를 공개 API 로 제공하지 않는다.

### 6.1 검토하고 기각한 공개 API 경로

- **`NSEvent` 의 `touchesMatchingPhase:inView:`.** 개별 터치 좌표·phase 를 준다는 점에서 후보로 보이지만, **앱이 포커스를 가진 창(key window)에서만** 동작한다. F-06 은 어떤 앱이 포커스를 갖고 있든 트랙패드 제스처에 반응해야 하는 **전역** 기능이므로 이 경로는 요구사항을 충족하지 못한다. **기각.**
- **`NSEvent.addGlobalMonitorForEvents(matching: .gesture/.magnify/.rotate/.swipe)`.** 전역으로 동작한다는 점은 맞지만, 이 API 가 주는 것은 OS 가 이미 하나의 제스처로 **해석·요약한** 값(확대/축소 배율, 회전각, 스와이프 방향)뿐이다. "지정된 코너에서 손가락 1개가 안쪽으로 얼마나 이동했는가"라는 개별 접촉점 좌표는 제공하지 않는다. **기각.**

이 두 경로 모두 macOS 가 멀티터치 원시 데이터를 서드파티에 공개하지 않는다는 동일한 근본 제약에서 나온다.

### 6.2 실무 경로 — 비공개 MultitouchSupport 프레임워크

같은 제작자(Ryan Hanson)의 다른 앱인 **Multitouch**와 **Scroll**이 트랙패드/멀티터치 원시 데이터를 다루는 유틸리티라는 점에서, 이 계열 앱들이 동일한 비공개 프레임워크 경로를 쓸 가능성이 높다고 본다 `(추정, 근거: 두 앱의 기능 성격 — 별도 1차 확인 없음)`.

| 항목 | 내용 | 확인 상태 |
| :--- | :--- | :--- |
| 프레임워크 | `MultitouchSupport.framework` (`/System/Library/PrivateFrameworks/` 아래에 위치, 공개 헤더 없음) | 비공개 프레임워크라는 사실 자체는 업계에 널리 알려진 사실이나, 이 문서의 1차 조사(superkey-inventory.md, rust-macos-capability-notes.md)는 이를 직접 검증하지 않았다 — 확인 방법은 §9 |
| `MTDeviceCreateList` | 연결된 멀티터치 장치(내장 트랙패드 + 외장 Magic Trackpad) 목록을 얻는 함수로 추정 | 함수 **이름**만 신뢰. 정확한 파라미터·반환 타입 시그니처는 이 문서가 **짓지 않는다** `(추정)` |
| `MTRegisterContactFrameCallback` | 특정 장치에 대해 접촉 프레임(손가락별 위치·상태) 콜백을 등록하는 함수로 추정 | 함수 **이름**만 신뢰. 콜백 함수 포인터 타입, 프레임 데이터 구조체(개별 손가락 레코드)의 정확한 메모리 레이아웃은 공개 헤더가 없어 **확인 불가** — 커뮤니티 역공학 자료에 의존해야 하며 이 문서는 그 세부값을 단정하지 않는다 `(추정)` |
| `MTDeviceStart` | 등록된 장치에서 실제로 프레임 스트림 수신을 시작하는 함수로 추정 | 함수 이름만 신뢰 |
| 대칭 API(정지·해제) | `MTDeviceStop`, `MTUnregisterContactFrameCallback`, `MTDeviceRelease` 류의 정리 함수가 합리적으로 존재할 것으로 추정되나, 함수명 자체가 조사로 확인되지 않았다 | `(추정)`, 함수명 불명 — §9 |
| TCC 권한 | rust-macos-capability-notes.md 는 이 프레임워크에 대한 TCC 요구사항을 조사하지 않았다. 트랙패드 원시 데이터 접근에 별도 권한이 필요한지 불명 | ❓미확인 — §9 |

비공개 API 사용의 함의:

- **App Store 배포 불가** — 하지만 SuperKey 는 직접 배포(`.dmg`, Sparkle)이므로 이 제약은 해당 없음(superkey-inventory.md §1.5).
- **OS 업데이트로 깨질 위험** — 공개 계약이 없으므로 Apple 이 언제든 함수를 제거·변경해도 사전 고지가 없다. §5 항목 9 의 방어 코드가 필수.
- **헤더가 없어 함수 시그니처를 직접 선언해야 함** — 컴파일러가 시그니처를 검증해 주지 않으므로, 잘못된 타입 선언은 조용한 메모리 손상으로 이어질 수 있다.

## 7. 구현 접근

**판정: Rust 바인딩(단, 손으로 작성한 비공개 API 바인딩 — 다른 F-0N 명세의 "Rust 바인딩"과 위험 등급이 다르다).**

rust-macos-capability-notes.md §1.1 이 정리한 `objc2-*` 계열은 모두 **공개 프레임워크의 헤더**로부터 자동 생성된 바인딩이다. `MultitouchSupport.framework` 는 공개 헤더가 없으므로 이 생성 파이프라인에 들어갈 수 없다 — `objc2` 자동 생성 바인딩 경로는 원천적으로 쓸 수 없다.

그러나 `MTDeviceCreateList`/`MTRegisterContactFrameCallback`/`MTDeviceStart` 는 (이름과 통상적인 이런 계열 프레임워크의 관례로 볼 때) **C ABI 로 노출된 일반 함수**이지, Objective-C 클래스 메서드가 아니다 `(추정)`. 이 전제가 맞다면:

- Rust 쪽에서 `extern "C" { fn MTDeviceCreateList() -> ...; ... }` 형태로 함수 시그니처를 **손으로 선언**하고, 빌드 스크립트(`build.rs`)에서 `#[link(name = "MultitouchSupport", kind = "framework")]` 와 `PrivateFrameworks` 검색 경로(`-F /System/Library/PrivateFrameworks`)를 지정해 **직접 링크**하는 것만으로 호출이 가능하다.
- 콜백 함수(`MTRegisterContactFrameCallback` 에 넘기는 함수 포인터)도 Rust 의 `extern "C" fn(...)` 로 정의한 함수를 그대로 C 함수 포인터로 넘길 수 있어, 별도로 컴파일된 Swift/Objective-C 트램폴린 파일이 **필요하지 않다**.

따라서 이 경로는 "네이티브 shim 불가피"가 아니라 **"Rust 바인딩"** 으로 판정한다 — 단, `objc2-*` 류의 안전하고 시그니처가 헤더로 보증된 바인딩과는 질적으로 다르다: 이것은 **비공개 API 를 대상으로 한 손수 작성 FFI 선언**이며, 구조체 레이아웃(특히 콜백이 넘겨주는 손가락별 프레임 데이터)이 틀리면 정의되지 않은 동작(크래시, 메모리 손상)으로 직결된다. 이 위험은 컴파일러가 잡아주지 못한다.

- **기각한 대안 1 — `NSEvent.touchesMatchingPhase:inView:`.** §6.1 참조. 포커스 창에서만 동작해 전역 제스처 요구사항을 충족하지 못한다.
- **기각한 대안 2 — `NSEvent.addGlobalMonitorForEvents(matching: .gesture/.magnify/...)`.** §6.1 참조. 개별 접촉점을 주지 않고 이미 해석된 제스처 요약값만 준다.
- **기각한 대안 3 — "순수 Rust" 판정.** 순수 Rust 로 표현 가능한 부분(상태 머신, 임계값 판정 로직)은 F-01 의 상태 머신과 성격이 같지만, 이 기능의 입력 경계 전체가 비공개 프레임워크의 원시 콜백 데이터에 의존하므로 "순수 Rust"로 판정하면 이 기능의 실제 위험(비공개 API 의존)을 은폐하게 된다.

### ⚠️ 이 기능은 다른 기능과 위험도가 다르다 — 구현 순서 의견

F-01(Seek 활성화)·F-05(hyperkey 조합)·F-07(이벤트 탭)이 의존하는 `CGEventTap`·`AXUIElement`·`CGEventTap` 등은 모두 **공개**, **문서화**, **활발히 유지되는 Rust 크레이트로 커버**되는 API 다(rust-macos-capability-notes.md §1–2). F-06 은 이 모든 성질이 반대다: 비공개, 무헤더, 무크레이트, 무보증.

게다가 F-06 은 정의상 hyper 키 활성화의 **필수 경로가 아니라 대체 경로**다 — 물리 키 리매핑(F-05 의 기본 경로)만으로 hyper 키 기능은 완결된다. 이 두 사실을 합치면:

- F-06 은 **후순위로 구현하는 것이 타당하다**고 본다. F-05/F-07 이 안정화된 뒤, 별도의 기능 플래그로 분리해 추가하는 편이 전체 일정의 리스크를 낮춘다.
- 릴리스 시에도 F-06 은 **선택적·격하 가능(degradable)** 기능으로 설계해야 한다 — 심벌 로드 실패나 예상과 다른 데이터 레이아웃을 감지하면 이 기능만 조용히 비활성화하고, hyper 키의 물리 키 경로(F-05)는 영향받지 않아야 한다.
- macOS 메이저 버전이 올라갈 때마다 이 프레임워크의 동작을 회귀 테스트하는 것을 QA 절차에 명시적으로 포함해야 한다(§5 항목 9).

## 8. 수용 기준

- [ ] `Engage hyper key using trackpad:` 가 ☑ 이고 트랙패드가 있는 기기에서, 선택된 영역(예: `top right`)에서 손가락 1개를 안쪽으로 슬라이드하면 hyper 활성 신호가 F-05 방향으로 방출된다.
- [ ] hyper 활성 상태에서 트랙패드 접촉이 끝나면(finger up) 다음 프레임 내 hyper 해제 신호가 방출된다.
- [ ] 진입 영역 밖에서 시작한 접촉이나, 이동 임계값에 도달하지 못한 접촉은 hyper 를 활성화하지 않는다.
- [ ] 이동 임계값 도달 전에 두 번째 손가락이 추가로 감지되면 제스처가 취소되고 hyper 는 활성화되지 않는다.
- [ ] 트랙패드가 없는 기기(데스크톱 Mac + Magic Mouse/외장 키보드만)에서는 이 기능이 어떤 조건에서도 hyper 를 활성화하지 않으며, 크래시 없이 안전하게 무효 상태를 유지한다.
- [ ] 세션 도중 외장 Magic Trackpad 를 연결하면 재시작 없이 새 장치에서도 제스처가 인식된다.
- [ ] 세션 도중 사용 중이던 외장 트랙패드가 연결 해제되면, hyper 가 `Engaged` 상태였을 경우 강제로 해제되고 stuck 상태가 되지 않는다.
- [ ] 절전 복귀 직후에도 (재접속 지연을 허용하는 합리적 시간 내) 트랙패드 제스처 인식이 정상 동작한다.
- [ ] `Engage hyper key using trackpad:` 를 끄면, 이미 `Engaged` 상태였던 hyper 가 즉시 강제 해제된다.
- [ ] MultitouchSupport 심벌 로드나 콜백 데이터 해석이 실패하는 경우, 앱이 크래시하지 않고 이 기능만 비활성화된 채로 나머지 기능(물리 키 hyper 리매핑 등)이 정상 동작한다.
- [ ] hyper 가 트랙패드 경로로 활성화된 상태에서 물리 키를 통한 hyper 활성화가 동시에 일어나도(F-05 소관 병합 규칙 하에) 중복되거나 모순된 modifier 상태가 발생하지 않는다.

## 9. 미해결 질문

| # | 질문 | 왜 확정 못 했는가 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | 영역 선택 팝업의 **전체 후보 목록** (확인값은 `top right` 하나뿐. §4 는 4코너+상단가장자리를 `(추정)` 으로 제안) | 스크린샷은 선택된 값 1개만 노출 | 앱 설치 후 팝업 열기 |
| 2 | `ENTRY_ZONE`(진입 영역 크기), `MIN_SLIDE_DISTANCE`(최소 이동 거리), `GESTURE_TIMEOUT`(제스처 포기 시간)의 실제 수치 | 조사 자료에 정량값 없음 | 앱 설치 후 실측(트랙패드 로깅), 또는 개발자 문의 |
| 3 | 코너에서의 "안쪽" 방향이 정확히 대각선인지, 허용 각도 범위는 얼마인지 (§3.3) | 랜딩("down")과 스크린샷 부제("in")의 표현이 상이하고 정량 근거 없음 | 앱 설치 후 실동작 확인 |
| 4 | `MultitouchSupport.framework` 가 실제로 Superkey 의 구현 경로인지 (§6.2 는 같은 제작자의 Multitouch/Scroll 앱 성격으로부터의 추정) | Superkey 바이너리 자체를 조사하지 않음(웹 조사만 수행) | Superkey `.app` 바이너리의 링크된 프레임워크 목록 확인(`otool -L`), 또는 개발자 문의 |
| 5 | `MTDeviceCreateList`/`MTRegisterContactFrameCallback`/`MTDeviceStart` 의 정확한 함수 시그니처와 콜백 데이터 구조체 레이아웃 | 공개 헤더 없음. 이 문서는 함수 이름 외 세부값을 짓지 않기로 결정 | 커뮤니티 역공학 헤더 확보(공개 GitHub 등) 후 실기 검증, `nm`/`class-dump` 류 도구로 심벌 확인 |
| 6 | 트랙패드 원시 데이터 접근에 TCC 권한(Accessibility/Input Monitoring 외 별도 권한)이 필요한가 | rust-macos-capability-notes.md 가 이 프레임워크의 권한 요구사항을 조사하지 않음 | 실기에서 권한 프롬프트 발생 여부 관찰 |
| 7 | 콜백이 macOS 드라이버 레벨 팜 리젝션 **이전** 데이터를 주는지 **이후** 데이터를 주는지 (§5 항목 11) | 비공개 API 라 동작 문서 없음 | 실기 테스트(의도적 손바닥 접촉으로 재현) |
| 8 | 두 번째 손가락이 `Engaged` 상태 도중 추가되는 경우 유지할지 강제 해제할지 (§3.2 표) | 조사 자료에 근거 없음 | 앱 설치 후 실동작 확인, 또는 개발자 문의 |
| 9 | 절전 복귀 시 `MTDevice` 핸들이 `CGEventTap` 과 같은 계열로 무효화되는지 (§5 항목 7) | MultitouchSupport 문서 없음. `CGEventTap` 사례(v1.58)로부터의 유비 추론일 뿐 | 실기에서 절전 복귀 반복 테스트 |
| 10 | `Engage hyper key using trackpad:` 와 영역 팝업의 **출고 기본값** | 스크린샷은 홍보용 구성일 수 있음(superkey-inventory.md §3.4) | 앱 최초 실행 후 `defaults read com.knollsoft.Superkey` |
| 11 | 화면 잠금 중 제스처 인식 억제를 F-06 자신이 할지 F-07 이 할지의 책임 경계 (§5 항목 10) | 조사 자료에 근거 없음 | F-07 명세 확정 시 함께 결정 |
