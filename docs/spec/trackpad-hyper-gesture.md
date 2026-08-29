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

이 기능은 웹 조사(superkey-inventory.md) 단계에서는 **근거가 이 프로젝트에서 가장 얇았다.** 이후 `/Applications/Superkey.app` 실측(app-bundle-analysis.md)으로 정성적 구조 — 영역 후보 5종·표시 순서, 사용 프레임워크(`MultitouchSupport.framework`)와 실제 링크된 심볼, 임계 파라미터의 **이름**, 2단계(트리거/프리즈) 구조, Magic Mouse 대상 포함, 팜/엄지 거부 로직의 존재 — 가 크게 보강되었다(아래 각 절 참조). 다만 **임계값의 실제 수치**와 **제스처의 실제 반응(체감)**은 물리적인 트랙패드 입력이 필요해 이번 조사에서도 관찰하지 못했다 — 이 두 가지는 여전히 이 문서에서 근거가 가장 얇은 영역이다. 아래 전체 문서에서 `(추정)`·`(미확정)` 표시는 여전히 신중하게 읽어야 한다.

## 2. 사용자 시나리오

### 시나리오 A — 상단 가장자리에서 슬라이드해 hyper 로 창 배치(Rectangle Pro 연동)

전제: `Engage hyper key using trackpad:` = ☑, 영역 = `top`(상단 가장자리 — 실측: AX 트리, app-bundle-analysis.md §6.2. 영역 팝업 5종 중 하나로 확정됨. §4 참조).

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

전제: 데스크톱 Mac(Mac mini/Mac Studio/iMac 등)에 일반 서드파티 마우스(멀티터치 표면이 없는 것) 또는 외장 키보드만 연결, 내장·외장 트랙패드도 Magic Mouse 도 없음. ⭐ Magic Mouse 는 §3.5 에서 확정된 대로 이 기능의 **대상 기기이므로 이 시나리오에서 제외**한다 — 이전 명세는 Magic Mouse 를 "트랙패드 없음"의 예시로 들었으나 이는 실측으로 정정되었다(§5 항목 1).

1. 환경설정 `Hyperkey` 탭을 연다.
2. `Engage hyper key using trackpad:` 체크박스는 활성화할 멀티터치 장치(트랙패드·Magic Mouse)가 없으므로 사용할 수 없는 상태로 표시된다(비활성화 또는 안내 문구 — 정확한 UI 처리는 `(추정)`, §5·§9 참조).
3. 사용자가 이후 Magic Trackpad 또는 Magic Mouse 를 블루투스로 연결하면, 체크박스가 사용 가능해진다(§5 참조).

## 3. 동작 명세

### 3.1 원칙

- 이 기능이 인식하는 유효 접촉은 **정확히 손가락 1개**여야 한다. 랜딩 페이지("one touch gesture")와 스크린샷 부제("Slide **only one** touch in...")가 모두 이를 직접 명시한다 — 이 부분은 `(추정)` 이 아니라 **확인된 원문 근거**다.
- ⭐ **"손가락 1개"는 단순히 감지된 접촉 개수가 1개라는 뜻이 아니라, 거부된 접촉을 제외한 "유효 접촉"이 1개라는 뜻으로 재정의한다(실측: 번들 문자열, app-bundle-analysis.md §4.6).** 실행 파일에 손바닥·엄지 얹힘 거부(palm/thumb rejection) 관련 키·타입이 다수 확인된다 — `restingThumb` · `restingPalm` · `restingTopThumb` · `restingBottomThumb` · `RestingThumbState` · `wristTouches` · `tooLightTouches` · `largeTouches` · `largeTouchMajorAxis` · `largeTouchMinorAxis` · `filterLargeTouches` · `filterLightTouches` · `disablePalmRejection` · `disableThumbRejection` · `lightTouchFilter`. 접촉 압력·면적을 재는 것으로 보이는 키도 함께 있다 — `zDensity` · `zPressure` · `zTotal` · `majorAxis` · `minorAxis`. 즉 원시 멀티터치 프레임에서 "너무 크거나(손바닥)", "너무 가볍거나(스치는 접촉)", "손목/엄지로 분류된" 접촉을 먼저 걸러낸 뒤에야 남은 접촉 수를 헤아려 `MAX_TOUCH_COUNT = 1` 판정을 한다는 구조로 읽힌다. 구체적인 판정 임계치와 필터 순서는 여전히 `(미확정)`이다.
- 제스처는 **진입 영역**(모서리 또는 상단 가장자리 중 선택된 하나)에서 **시작**해야 하고, **안쪽 방향**으로의 이동이어야 한다. 영역 밖에서 시작한 접촉이나 바깥쪽으로의 이동은 무시한다.
- hyper 활성화는 이동 임계값 충족 **즉시** 일어나고(정확한 타이밍은 §9), 해제는 접촉이 끝나는 순간(finger up) 발생한다 — 부제의 "Remove the touch to release" 는 지연 없는 즉시 해제로 읽는다.
- F-06 은 **손가락의 원시 위치·상태**를 프레임 단위로 알아야 하므로(§6), 공개 API 로는 표현할 수 없는 요구사항이다. 이 문서의 §6·§7 이 그 함의를 다룬다.
- ⭐ **인식기는 이 제스처(`OneDraw`) 하나만이 아니다.** 실행 파일에서 확인되는 인식기 계층: `OneDrawGesture` · `OneDrawRecognizer` · `TapRecognizer` · `ClickRecognizer` · `ForceRecognizer` · `threeTapRecognizer` · `fourTapRecognizer`(실측: 번들 문자열, app-bundle-analysis.md §4.6). 즉 트랙패드 원터치 hyper 제스처(`OneDraw`)는 SuperKey 가 같은 원시 멀티터치 스트림 위에서 동시에 돌리는 **여러 인식기 중 하나**이며, 탭·클릭·포스터치·3-4손가락 탭 인식기와 공존한다. 인식기 간 우선순위(예: `OneDraw` 판정 중에 `ForceRecognizer` 가 먼저 반응하면 어떻게 되는가)는 `(미확정)` — §9.

### 3.2 상태 머신

이름 붙인 파라미터: `ENTRY_ZONE`(진입 영역의 물리적 경계), `MIN_SLIDE_DISTANCE`(활성화에 필요한 최소 이동 거리), `MAX_TOUCH_COUNT = 1`(허용 동시 접촉 수 상한 — 확인된 값, §3.1 재정의대로 "유효 접촉" 기준), `GESTURE_TIMEOUT`(접촉 시작 후 임계 미도달 시 포기 시간).

| 현재 상태 | 입력 | 조건 | 다음 상태 | 부수효과 |
| :--- | :--- | :--- | :--- | :--- |
| 대기(Idle) | 새 접촉(finger down) 감지 | `Engage hyper key using trackpad:` = ☑ · 접촉 좌표가 `ENTRY_ZONE`(선택된 코너/가장자리) 내 `(추정)` · 유효 접촉 수 = 1 | 진입접촉감지(ZoneContact) | 접촉 ID·시작 좌표·시작 시각 기록 |
| Idle | 새 접촉 감지 | 접촉 좌표가 `ENTRY_ZONE` 밖 | Idle 유지 | 부수효과 없음(무시) |
| Idle | 새 접촉 감지 | `Engage hyper key using trackpad:` = ☐, 또는 이 트랙패드에 대해 아직 `MTDeviceStart` 미실행 | Idle 유지 | 부수효과 없음 |
| ZoneContact | 동일 접촉 ID 가 이후 프레임에서 안쪽 방향으로 누적 이동량이 **프리즈 임계**(`cornerFreezeThreshold`/`oneTopCursorFreezeThreshold`, §3.2.1) 이상 | 유효 접촉 수 여전히 1 | 커서고정(CursorFrozen) | 커서 이동을 이 접촉으로부터 분리(freeze) — hyper 는 아직 미활성 |
| ZoneContact | 임계 도달 전 접촉 소멸(finger up) | — | Idle | 부수효과 없음 — §5 "제스처 미완성" |
| ZoneContact | 임계 도달 전 두 번째 접촉 추가 | — | Idle(취소) | 부수효과 없음 — §5 "다른 제스처와의 오인" |
| ZoneContact | `GESTURE_TIMEOUT`(추정) 경과, 임계 미도달 | — | Idle | 접촉 상태 폐기 |
| CursorFrozen | 동일 접촉 ID 가 안쪽 방향으로 누적 이동량이 **트리거 임계**(`cornerTriggerThreshold`/`oneTopTriggerThreshold`, §3.2.1) 이상 | 유효 접촉 수 여전히 1 | hyper 활성(Engaged) | **hyper 활성 신호 방출**(§3.4). 커서 고정은 유지 `(추정)` |
| CursorFrozen | 트리거 임계 도달 전 접촉 소멸(finger up) | — | Idle | 커서 고정 해제 — §5 "제스처 미완성" |
| Engaged | 동일 접촉 ID 유지(터치가 표면에 계속 있음) | — | Engaged 유지 | 부수효과 없음(hyper 유지, 커서 고정 유지 `(추정)`) |
| Engaged | 접촉 소멸(finger up) 감지 | — | 해제됨(Released) | **hyper 해제 신호 방출**(§3.4), 커서 고정 해제 |
| Engaged | 두 번째 접촉 추가 감지(예: 다른 손가락이 우발적으로 트랙패드에 닿음) | — | Engaged 유지 `(추정)` 또는 즉시 강제 해제 `(추정)` | 정책 미확정 — §9 |
| Engaged | 트랙패드 장치 핸들 무효화(절전 복귀, 외장 트랙패드 분리 등) | — | Released(강제) | hyper 강제 해제 신호 방출 — §5 |
| Released | (내부 전이) | — | Idle | 접촉 ID 바인딩 해제 |

#### 3.2.1 ⭐ 2단계 임계 구조 (실측: 번들 문자열, app-bundle-analysis.md §4.6)

이전 명세는 임계값을 `MIN_SLIDE_DISTANCE` 하나로 단순화했으나, 실행 파일 문자열에서 **파라미터 이름 자체가 실재함**이 확인되어 구조를 다시 짠다. 확인된 이름: `cornerTriggerThreshold` · `cornerFreezeThreshold` · `oneTopTriggerThreshold` · `oneTopCursorFreezeThreshold`. 여기서 읽어낼 수 있는 구조:

- **코너 계열과 상단(`top`) 계열이 서로 다른 임계값 쌍을 쓴다** — `corner*` 접두어와 `oneTop*` 접두어가 별도로 존재한다. 즉 진입 영역이 4개 코너 중 하나냐 상단 가장자리(`top`)냐에 따라 임계값이 다르게 설정되어 있을 가능성이 높다. 두 계열을 하나의 `ENTRY_ZONE`/`MIN_SLIDE_DISTANCE` 로 뭉뚱그린 이전 설계는 이 구조를 놓치고 있었다.
- **각 계열마다 "트리거 임계"와 "커서 고정(freeze) 임계" 두 개가 있다** — 이는 이전 명세가 예상하지 못한 구조다. 즉 제스처 도중 **커서 이동을 일시적으로 얼어붙게(freeze) 하는 별도 단계**가 hyper 활성화(trigger)와 분리되어 존재한다. 관련 키 `cursorFreeze` 도 확인된다.
- 이 문서는 "freeze 임계가 trigger 임계보다 먼저(더 작은 이동량에서) 도달한다"는 순서를 **논리적으로 타당한 추정**으로 채택했다 — 손가락이 조금만 움직여도 그 이동이 일반 트랙패드 커서 이동으로 오인되지 않도록 먼저 커서를 고정한 뒤, 충분히 더 미끄러지면 실제로 hyper 를 트리거하는 2단계 UX 로 읽힌다. 이 순서 자체, 그리고 freeze 가 Engaged 상태 동안 계속 유지되는지는 `(미확정)` — §9.
- 수치 자체(각 임계값이 실제로 몇 mm/px 인지)는 여전히 `(미확정)`이다. 파라미터 **이름의 실재**만 승격되었다.

### 3.3 진입 영역과 슬라이드 방향의 해석

랜딩 페이지와 스크린샷 부제가 서로 다른 표현을 쓴다는 점에 유의해야 한다:

- 랜딩: "Slide one touch **down** from a corner or the top edge" — "아래로"
- 스크린샷 부제: "Slide only one touch **in** from the selected area ... **a little**" — "안쪽으로, 조금"

상단 가장자리에서는 "아래로" = "안쪽으로"가 같은 방향이지만, **코너**에서는 "아래로"만으로는 코너에서 트랙패드 중앙을 향한 대각선 이동을 설명하지 못한다. 이 문서는 코너의 경우 "안쪽(대각선)"으로, 상단 가장자리의 경우 "아래(수직)"으로 해석한다 `(추정)` — 두 원문을 조화시킨 것이며 확정된 근거는 아니다(§9).

⭐ 이 해석은 부분적으로 실측 뒷받침을 얻었다(실측: 번들 문자열, app-bundle-analysis.md §6.2). 영역 팝업 각 항목에 대응하는 내부 이미지 이름이 `hyperSlideTopLeftTemplate` · `hyperSlideTopRightTemplate` · `hyperSlideBottomLeftTemplate` · `hyperSlideBottomRightTemplate` · `hyperSlideDownTemplate`(상단 가장자리 `top` 항목용)로 확인된다. 코너 4종은 이름에 "Top/BottomLeft/Right"로 **위치**만 표기되어 방향을 직접 말하지 않지만, 상단 가장자리 항목만 유일하게 `...Down` 으로 **방향**을 명시한다 — 이는 상단 가장자리에서는 "아래로"가 곧 유일한 자연스러운 방향(코너처럼 여러 대각선이 있을 수 없음)이라는 §3.3 의 해석과 정합한다. 다만 코너 4종의 정확한 허용 각도·대각선 여부는 여전히 `(미확정)`이다.

### 3.4 hyper 활성 신호 방출 계약

F-06 은 `CGEvent` 의 modifier flag 를 직접 합성하지 **않는다**. F-06 이 하는 일은 상태 머신이 `Engaged`/`Released` 로 전이할 때 F-05 방향으로 **"트랙패드 소스의 hyper 요청 상태"**(불리언: 활성/비활성)를 알리는 것뿐이다.

- F-05(hyperkey.md)는 hyper 조합(⌃⌥⌘⇧, meh, bleh)을 정의하는 소비자다. F-05 관점에서 "hyper 를 켜라"는 요청은 물리 키 소스(Caps Lock 등 리매핑된 키)와 트랙패드 소스 **두 곳에서 올 수 있다** — F-06 은 그중 하나다.
- 두 소스가 동시에 활성(물리 키를 누른 채 트랙패드 제스처도 활성)인 경우의 병합 규칙(OR 로 합쳐 하나의 hyper 상태로 취급하는 것이 자연스러운 기본값이나, 확정된 근거는 없다)은 F-05/F-07 의 몫이며 F-06 은 자신의 소스 상태만 정직하게 보고한다 `(추정)`.
- 실제 `CGEvent` modifier flag 삽입·이벤트 탭 우선순위 중재는 F-07 소관이다. F-06 은 F-07 이 노출하는 "가상 modifier 소스 등록" 인터페이스를 통해 신호를 전달한다고 가정한다 — 그 인터페이스의 구체 형태는 F-07 명세에서 정의되어야 하며, 이 문서에서 API 시그니처를 짓지 않는다.
- hyper 가 F-06 경로로 활성화된 상태에서 마우스/키보드 이벤트에 modifier 를 얹을지 여부는 `Apply modifiers to keypress events and: Click/Drag/Move/Scroll` 체크박스(superkey-inventory.md §3.2, F-05 범위)의 일반 규칙을 그대로 따른다고 가정한다 `(추정)` — F-06 특유의 예외는 조사 자료에 없다.

### 3.5 ⭐ 신규 확인 — Magic Mouse 도 대상 기기다

이전 명세는 "트랙패드"만 전제했으나, 실행 파일에는 트랙패드와 마우스를 **별도 타입으로 나누어 등록**하는 구조가 있다(실측: 번들 문자열, app-bundle-analysis.md §4.6): 내부 타입 `MTTrackpadRegistrar` / `MTMouseRegistrar` 가 나뉘어 있고, 로그 문자열에 `Unregistered Magic Mouse`(SuperKey 원문)가 있다. 상태 키도 기기 종류별로 분리되어 있다: `magicMouseConnected` · `forceTouchConnected` · `trackpadConnected` · `hasBuiltInTrackpad` · `internalTrackpad` · `magicTrackpad`.

- Magic Mouse 도 표면 멀티터치가 가능한 기기이므로, 원터치 hyper 제스처(`OneDraw`, §3.1)가 Magic Mouse 표면에서도 동작 대상이라는 것이 이 구조로 뒷받침된다. 다만 Magic Mouse 는 트랙패드보다 표면이 훨씬 좁아 코너/상단 가장자리라는 개념이 그대로 적용되는지, 아니면 Magic Mouse 전용 판정 로직(별도 임계값 등)이 있는지는 `(미확정)`이다.
- `forceTouchConnected` 는 Force Touch 트랙패드(내장) 여부를, `hasBuiltInTrackpad`/`internalTrackpad` 는 내장 트랙패드 유무를, `magicTrackpad` 는 외장 Magic Trackpad 를 각각 가리키는 것으로 보이나 정확한 의미 구분은 `(미확정)`이다.
- §5 에 기기 종류별 등록/해제 실패 모드를 추가한다.

### 3.6 관찰·디버깅 수단 — 트랙패드 뷰어(실측: 메뉴바 AX 트리 + 번들 문자열)

메뉴바 메뉴 `Advanced ▸ Show Viewer…` / `Launch Viewer on Start`(app-bundle-analysis.md §6.5)는 연결된 각 멀티터치 장치의 원시 접촉을 시각화하는 **디버그 뷰어**다. 툴팁 원문(SuperKey 원문): "Viewing windows will be displayed for each device when touches are detected on that device". 즉 장치마다 별도의 뷰잉 창이 접촉 발생 시에만 뜨는 구조로 보인다. 이는 사용자 대상 기능이 아니라 개발·디버깅 수단이지만, 클론이 같은 종류의 원시 멀티터치 파이프라인을 구현한다면 개발 단계에서 상응하는 디버그 뷰어를 두는 것이 실측(코너/프리즈/트리거 임계값을 눈으로 확인할 유일한 방법)에 유용하다는 점을 기록해 둔다.

## 4. 설정 항목

| 이름(원문 라벨) | 타입 | 확인된 값 / 기본값 | 유효 범위 | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| `Engage hyper key using trackpad:` | 체크박스 | **☐(꺼짐) — 출고 기본값 확정** | ☑ / ☐ | 실측: defaults + AX 트리, app-bundle-analysis.md §2.1, §6.2. `~/Library/Preferences/com.knollsoft.Superkey.plist` 에 관련 키가 신규 설치 상태에서 아예 없고, 미가공 AX 실측도 ☐ — 이중 근거로 확정(hyperkey.md §4 와 동일 판정 근거) |
| (영역 선택 팝업, 명시적 라벨 없음 — 체크박스 옆 팝업 버튼) | 팝업 버튼 | **☐/`top right`(실측 기본값)** | ⭐ **확인된 값은 5종 전량, 표시 순서까지 확정**: `top left` · `top right` · `bottom left` · `bottom right` · `top` — §4 이전 버전의 "4 코너 + 상단 가장자리" 추정이 **정확히 맞았다**. `top` 이 상단 가장자리다 | 실측: AX 트리, app-bundle-analysis.md §6.2. 내부 이미지 이름 `hyperSlideTopLeftTemplate`/`hyperSlideTopRightTemplate`/`hyperSlideBottomLeftTemplate`/`hyperSlideBottomRightTemplate`/`hyperSlideDownTemplate`, defaults 키 `oneSwipeFromTop` 이 이를 뒷받침한다 |

⭐ **추정이 맞았다는 사실 자체를 기록한다.** 이전 버전 §4 는 랜딩 페이지의 "corner **or** the top edge" 문구만으로 후보를 `top left`/`top right`/`bottom left`/`bottom right`/`top edge`(상단 가장자리 전체를 가리키는 가상의 이름)로 추정했다. 실측 결과 코너 4종의 이름은 추정과 정확히 일치했고, "상단 가장자리"에 대응하는 실제 UI 라벨은 추정했던 `top edge` 가 아니라 단순히 `top` 이었다 — 개념은 맞았고 정확한 문자열만 달랐다.

### 내부 인식 임계 파라미터 — 이름은 확정, 수치는 여전히 `(미확정)` (실측: 번들 문자열, app-bundle-analysis.md §4.6, §6.2)

⭐ 이전 버전은 `ENTRY_ZONE`/`MIN_SLIDE_DISTANCE`/`GESTURE_TIMEOUT` 이라는 **이 문서가 지어낸 가상의 파라미터 이름**으로 수치까지 추정했다. 실측으로 SuperKey 내부의 **실제 파라미터 이름**이 확인되어, 아래 표로 교체한다. §3.2.1 의 2단계(트리거/프리즈) × 2계열(코너/상단) 구조가 이 이름들의 근거다.

| 파라미터(실제 내부 이름) | 의미 | 계열 | 확정 상태 |
| :--- | :--- | :--- | :--- |
| `cornerTriggerThreshold` | 코너 진입 시 hyper 를 실제로 트리거하는 누적 이동 임계값 | 코너 | 이름 실재 확정(실측: 번들 문자열). **수치는 `(미확정)`** |
| `cornerFreezeThreshold` | 코너 진입 시 커서 이동을 고정(freeze)하는 누적 이동 임계값 | 코너 | 이름 실재 확정. **수치는 `(미확정)`** |
| `oneTopTriggerThreshold` | 상단(`top`) 진입 시 hyper 를 실제로 트리거하는 누적 이동 임계값 | 상단 | 이름 실재 확정. **수치는 `(미확정)`** |
| `oneTopCursorFreezeThreshold` | 상단(`top`) 진입 시 커서 이동을 고정하는 누적 이동 임계값 | 상단 | 이름 실재 확정. **수치는 `(미확정)`** |
| `cursorFreeze` | 위 프리즈 임계값들이 실제로 적용되는 상태를 나타내는 것으로 보이는 별도 키 | 공통 `(추정)` | 이름 실재 확정. 정확한 역할은 `(미확정)` |
| `MAX_TOUCH_COUNT` | 인식 유지에 허용되는 유효 접촉 수 | 공통 | **1** — 확인된 값(원문 직접 인용, §3.1) |
| `GESTURE_TIMEOUT` | 접촉 시작 후 임계 미도달 시 포기까지의 시간 | 공통 | 대응하는 내부 이름 확인 안 됨. 개념·수치 모두 `(추정)`(약 300–500ms 제안, 근거 없음, 유사 macOS 시스템 제스처 관례로부터의 추론) |
| 방향 허용 각도(각도 허용치) | "안쪽" 판정의 각도 허용 범위 | 코너 | 대응하는 내부 이름 확인 안 됨. §3.3 의 "down" vs "in" 불일치를 조화시키기 위한 제안(시작점 기준 트랙패드 중심 방향 ±30°). 근거 없음, `(추정)` |

### 의미 미확정인 관련 키 (실측: 번들 문자열 — 존재만 기록)

`trackpadClickSupport` · `MouseClickSupport` · `TwoButtonSwapped` · `MouseButtonDivision` 이 실행 파일에 존재한다(app-bundle-analysis.md §4.6). 클릭·버튼 관련 설정으로 보이나 대응하는 UI 를 찾지 못했고, 원터치 hyper 제스처(`OneDraw`)와 직접 관련이 있는지도 확인되지 않는다. 의미는 전부 `(미확정)` — §9.

## 5. 엣지 케이스와 실패 모드

1. **트랙패드도 Magic Mouse 도 없는 기기.** ⭐ **정정(§3.5): Magic Mouse 는 이 기능의 대상 기기다** — 이전 명세는 "데스크톱 Mac + Magic Mouse"를 트랙패드 없음의 예시로 들었으나, 실측으로 Magic Mouse 도 멀티터치 등록 대상(`MTMouseRegistrar`)임이 확인되어 이는 틀린 예시였다. 이 항목은 **일반 서드파티 마우스(멀티터치 표면 없음) 또는 외장 키보드만 있고 트랙패드·Magic Mouse 모두 없는 구성**으로 좁혀 다시 정의한다. 이런 구성에서는 `MTDeviceCreateList()`(§6)가 반환하는 장치 목록이 비어 있다. `Engage hyper key using trackpad:` 체크박스는 비활성화되거나(선택 불가) 켜져 있어도 아무 접촉 프레임도 들어오지 않아 항상 무효과 상태가 되어야 한다. 정확한 UI 처리(비활성화 vs 조용히 무시)는 F-09 소관이나, F-06 로직은 "장치 목록이 비어 있으면 이 기능은 절대 hyper 를 활성화하지 않는다"는 불변식을 지켜야 한다.
2. **외장 Magic Trackpad 연결.** 세션 도중 블루투스 Magic Trackpad 가 새로 연결되면, `MTDeviceCreateList()` 를 다시 호출해 장치 목록을 갱신하고 새 장치에도 콜백을 등록해야 한다. 연결 이벤트를 감시하는 구체 메커니즘은 조사 자료에 없다 `(추정)` — §9. ⭐ 연결·해제 자체는 로깅되는 것으로 확인된다 — 로그 문자열 `Registered trackpad` / `Unregistered trackpad`(SuperKey 원문, 실측: 번들 문자열, app-bundle-analysis.md §4.6).
3. **외장 트랙패드 연결 해제(도중 포함).** hyper 가 `Engaged` 상태인 도중 사용하던 트랙패드가 물리적으로 분리되면(배터리 소진, 블루투스 끊김), 해당 장치의 접촉 스트림이 그냥 끊길 뿐 명시적 finger-up 이벤트가 오지 않을 수 있다. 이 경우 강제 해제 처리가 없으면 hyper 가 **stuck**(항목 6) 상태로 남는다 — 장치 무효화 감지 즉시 강제 `Released` 로 전이해야 한다(§3.2 표). 로그 문자열 `Unregistered trackpad`(위 항목 2)가 이 경로에서도 남는 것으로 보인다.
4. **다른 트랙패드 제스처·시스템 제스처와의 오인.** 사용자가 코너/가장자리 근처에서 스와이프(예: 4손가락 Mission Control 스와이프, 3-finger drag)를 시작하는 경우, 첫 접촉이 진입 영역 안에서 시작될 수 있다. `MAX_TOUCH_COUNT = 1` 규칙(§3.2, §4)이 이 상황의 1차 방어선이다 — 두 번째 이상의 접촉이 감지되면 즉시 취소한다. 그러나 시스템 제스처가 먼저 macOS 자체에 의해 소비되어(트랙패드 제스처 인식은 OS 커널/드라이버 레벨에서도 동시에 일어남) F-06 이 애초에 원시 프레임을 못 받을 가능성도 있다 `(추정)` — §9.
5. **3-finger drag 등 시스템 제스처가 활성화되어 있는 상태.** 사용자가 시스템 설정에서 3-finger drag(트랙패드로 창 드래그)를 켜 놓은 경우, 코너 진입 접촉이 드래그의 일부로 소비될 가능성이 있다. F-06 은 이 충돌을 해소할 권한이 없다(둘 다 같은 원시 멀티터치 스트림을 관찰하는 별개 소비자) — 사용자에게 영역 선택을 바꾸도록 안내하는 것이 유일한 완화책이다 `(추정)`.
6. **손가락을 뗐는데 이벤트를 놓쳐 hyper 가 stuck.** `MTRegisterContactFrameCallbackWithRefcon` 콜백이 프레임을 유실하거나(시스템 부하, 콜백 스레드 지연) finger-up 프레임 자체가 전달되지 않으면 상태 머신이 영원히 `Engaged` 에 머물러 hyper 가 눌린 채로 고착된다. 워치독이 필요하다: 마지막 유효 프레임 수신 후 일정 시간(`(추정)`, 예: 2초) 동안 새 프레임이 없으면 강제로 `Released` 로 전이한다. ⭐ **이 워치독이 SuperKey 원본에 실재함이 확정되었다**(실측: 번들 문자열, app-bundle-analysis.md §4.6) — 로그 문자열 `Touches are not being detected. Restarting.`(SuperKey 원문)이 접촉 미검출 시 **자동 재시작**을 수행한다는 직접 증거다. 함께 확인되는 실패 문자열: `Unable to obtain device dimensions` / `Unable to obtain deviceId` / `Unable to register device` / `MT callback with nil values`. 이 실패 경로들은 §8 수용 기준에도 반영한다.
7. **절전 복귀 후 장치 핸들 만료.** superkey-inventory.md §2.1(v1.58)이 `CGEventTap` 이 절전 복귀·로그인 시 재설치가 필요하다고 확정한 것과 같은 계열 문제가 `MTDevice` 핸들에도 적용될 가능성이 높다 `(추정)` — MultitouchSupport 는 비공개 프레임워크라 이 동작이 문서화되어 있지 않다. 절전 복귀(`NSWorkspace.didWakeNotification` 등 공개 알림) 수신 시 `MTDeviceCreateList()`/`MTRegisterContactFrameCallbackWithRefcon()`/`MTDeviceStart()` 시퀀스를 전부 재실행하는 방어적 설계가 필요하다.
8. **여러 트랙패드 동시 연결.** 내장 트랙패드 + 외장 Magic Trackpad 가 동시에 연결된 구성에서, `MTDeviceCreateList()` 는 둘 다 반환할 가능성이 높다 `(추정)`. F-06 은 어느 한 장치가 아니라 **연결된 모든 트랙패드 장치**에 콜백을 등록해야 어느 쪽에서 제스처를 해도 반응한다. 두 장치에서 동시에 진입 영역 접촉이 발생하는 경우(사실상 2인 이상 동시 사용 등 희귀 상황)의 처리 정책은 조사 자료에 없다 `(추정)`.
9. **OS 업데이트로 비공개 API 시그니처가 바뀜.** MultitouchSupport 는 공개 헤더가 없는 비공개 프레임워크이므로, macOS 버전이 올라가며 함수 시그니처·콜백 구조체 레이아웃·심지어 함수 존재 자체가 예고 없이 바뀔 수 있다. 최소한 앱 시작 시 심벌 로드(`dlsym` 또는 링크 타임 심벌 해석) 실패를 감지해, 실패 시 이 기능 전체를 조용히 비활성화(체크박스 비활성화 안내)하는 방어 코드가 **필수**다 — 크래시로 이어지면 안 된다.
10. **화면 잠금.** 화면이 잠긴 동안(로그인 화면, 화면 보호기)에는 hyper 를 활성화해도 어떤 앱에도 전달할 대상이 없다. F-06 자체가 잠금 상태를 감지해 제스처 인식을 끌지, 그냥 신호만 방출하고 F-07 이 잠금 중 이벤트 주입을 억제할지는 F-07 의 책임 경계와 겹친다 — F-06 은 잠금 여부와 무관하게 원시 제스처 인식만 계속하고, 억제는 F-07 이 한다고 가정한다 `(추정)`.
11. **팜 리젝션(palm rejection) — ⭐ SuperKey 자체 필터링 로직이 확정됨.** 타이핑 중 손바닥 아랫부분이 트랙패드 상단 가장자리에 스치는 경우가 실제로 흔하다. §3.1 에서 확정한 대로 SuperKey 는 드라이버 레벨과 별개로 **자체 팜/엄지 거부 로직**을 갖고 있다(`restingThumb`/`restingPalm`/`largeTouches`/`filterLargeTouches`/`disablePalmRejection` 등, 실측: 번들 문자열). 다만 `MultitouchSupport` 콜백이 macOS 드라이버 레벨 팜 리젝션 **이전** 원시 데이터를 주는지 **이후** 데이터를 주는지는 여전히 확인되지 않았다 `(추정)`. 이후 데이터라면 SuperKey 의 자체 필터는 드라이버가 놓친 나머지를 보강하는 2차 방어선이고, 이전 데이터라면 SuperKey 의 자체 필터가 사실상 유일한 방어선이라는 뜻이 된다 — 어느 쪽이든 §4 의 `filterLargeTouches`/`filterLightTouches`/`disablePalmRejection`/`disableThumbRejection` 류 설정(대응 UI 는 4개 탭 어디에도 없어 사용자 노출 여부 `(미확정)`)이 오탐 완화의 핵심이라는 점은 실측으로 확정됐다 — §9.
12. **`Engage hyper key using trackpad:` 를 끈 상태에서 진행 중이던 제스처.** 사용자가 설정 창에서 이 체크박스를 실시간으로 끄는 순간 이미 `Engaged` 상태였다면, 즉시 강제 `Released` 로 전이해 hyper 를 해제해야 한다(설정 변경이 상태 머신에 즉시 반영되어야 stuck 상태를 막을 수 있다).
13. **Magic Mouse 관련 오탐·미탐(신규, §3.5).** Magic Mouse 표면은 트랙패드보다 훨씬 좁다. "코너"/"상단 가장자리"라는 진입 영역 개념을 트랙패드와 동일한 절대 좌표·거리 기준으로 그대로 적용하면 Magic Mouse 에서는 오탐(어디를 잡아도 진입 영역)이나 미탐(진입 영역이 사실상 없음)이 발생할 수 있다. Magic Mouse 전용 임계값이 별도로 존재하는지는 `(미확정)` — §9.
14. **Magic Mouse 연결 해제.** `Unregistered Magic Mouse`(SuperKey 원문, 실측: 번들 문자열, app-bundle-analysis.md §4.6) 로그가 확인된다 — 트랙패드와 마찬가지로 연결 해제가 감지·로깅되는 것으로 보이며, hyper 가 Magic Mouse 경로로 `Engaged` 상태였다면 항목 3 과 동일하게 강제 `Released` 처리가 필요하다.

## 6. 필요한 플랫폼 API

⭐ 이 기능의 핵심 기술 문제는 **개별 손가락의 원시 위치·상태에 대한 접근**이다. macOS 는 이를 공개 API 로 제공하지 않는다.

### 6.1 검토하고 기각한 공개 API 경로

- **`NSEvent` 의 `touchesMatchingPhase:inView:`.** 개별 터치 좌표·phase 를 준다는 점에서 후보로 보이지만, **앱이 포커스를 가진 창(key window)에서만** 동작한다. F-06 은 어떤 앱이 포커스를 갖고 있든 트랙패드 제스처에 반응해야 하는 **전역** 기능이므로 이 경로는 요구사항을 충족하지 못한다. **기각.**
- **`NSEvent.addGlobalMonitorForEvents(matching: .gesture/.magnify/.rotate/.swipe)`.** 전역으로 동작한다는 점은 맞지만, 이 API 가 주는 것은 OS 가 이미 하나의 제스처로 **해석·요약한** 값(확대/축소 배율, 회전각, 스와이프 방향)뿐이다. "지정된 코너에서 손가락 1개가 안쪽으로 얼마나 이동했는가"라는 개별 접촉점 좌표는 제공하지 않는다. **기각.**

이 두 경로 모두 macOS 가 멀티터치 원시 데이터를 서드파티에 공개하지 않는다는 동일한 근본 제약에서 나온다.

### 6.2 ⭐ 실무 경로 — 비공개 MultitouchSupport 프레임워크, 이제 확정됨

이전 버전은 "같은 제작자의 다른 앱(Multitouch/Scroll)이 이 계열 프레임워크를 쓸 가능성이 높다"는 성격 유추에 의존했다. **이제는 SuperKey 자신의 바이너리에서 직접 확인되었다(실측: 번들 심볼, app-bundle-analysis.md §3.1).** `otool -L Contents/MacOS/Superkey` 결과 `/System/Library/PrivateFrameworks/MultitouchSupport.framework` 가 **직접 링크**되어 있고, 이는 SuperKey 번들 전체에서 **유일한 PrivateFramework**다(다른 모든 링크 프레임워크는 공개 프레임워크). `nm -u` 로 실제 사용 심볼도 확인된다:

`MTDeviceCreateList` · `MTDeviceCreateFromDeviceID` · `MTRegisterContactFrameCallbackWithRefcon` · `MTDeviceStart` · `MTDeviceGetSensorSurfaceDimensions` · `MTDeviceIsBuiltIn` · `MTDeviceSupportsForce` · `MTDeviceGetFamilyID`

| 항목 | 내용 | 확인 상태 |
| :--- | :--- | :--- |
| 프레임워크 | `MultitouchSupport.framework` (`/System/Library/PrivateFrameworks/` 아래에 위치, 공개 헤더 없음) | ⭐ **`otool -L` 로 직접 링크 확정**(실측: 번들 심볼) — SuperKey 번들에서 유일한 PrivateFramework |
| `MTDeviceCreateList` | 연결된 멀티터치 장치 목록을 얻는 함수 | 심볼 사용 확정(실측: 번들 심볼). 정확한 파라미터·반환 타입 시그니처(공개 헤더 없음)는 이 문서가 **짓지 않는다** `(추정)` |
| `MTDeviceCreateFromDeviceID` | 특정 장치 ID 로부터 장치 핸들을 얻는 함수로 추정 — ⭐ **이전 명세에 없던 심볼**, 여러 장치(트랙패드 + Magic Mouse, §3.5)를 식별해 개별 핸들을 얻는 데 쓰이는 것으로 보인다 | 심볼 사용 확정. 시그니처 `(추정)` |
| `MTRegisterContactFrameCallbackWithRefcon` | 특정 장치에 대해 접촉 프레임(손가락별 위치·상태) 콜백을 등록하는 함수 — ⭐ **이전 명세가 가정한 이름 `MTRegisterContactFrameCallback` 과 다르다.** 실제 심볼은 `refcon`(콜백에 사용자 컨텍스트 포인터를 함께 전달하는 관례) 포함 변형이다 — 여러 장치를 동시에 다뤄야 하는 SuperKey 의 요구(§3.5)와 정합적이다 | 심볼 사용 확정(실측: 번들 심볼). 콜백 함수 포인터 타입, 프레임 데이터 구조체(개별 손가락 레코드)의 정확한 메모리 레이아웃은 공개 헤더가 없어 **확인 불가** — 커뮤니티 역공학 자료에 의존해야 하며 이 문서는 그 세부값을 단정하지 않는다 `(추정)` |
| `MTDeviceStart` | 등록된 장치에서 실제로 프레임 스트림 수신을 시작하는 함수 | 심볼 사용 확정. 시그니처 `(추정)` |
| `MTDeviceGetSensorSurfaceDimensions` | ⭐ **이전 명세에 없던 심볼.** 트랙패드/Magic Mouse 표면의 물리적 치수를 얻는 함수로 추정 — §4 의 `ENTRY_ZONE`·임계값이 절대 좌표가 아니라 표면 크기에 대한 상대값으로 계산될 가능성을 시사한다 `(추정)` | 심볼 사용 확정. 시그니처 `(추정)` |
| `MTDeviceIsBuiltIn` | ⭐ **이전 명세에 없던 심볼.** 내장 트랙패드인지 판별 — §3.5 의 `hasBuiltInTrackpad`/`internalTrackpad` 키와 대응되는 것으로 보인다 `(추정)` | 심볼 사용 확정. 시그니처 `(추정)` |
| `MTDeviceSupportsForce` | ⭐ **이전 명세에 없던 심볼.** Force Touch 지원 여부 판별 — §3.5 의 `forceTouchConnected` 키와 대응 `(추정)` | 심볼 사용 확정. 시그니처 `(추정)` |
| `MTDeviceGetFamilyID` | ⭐ **이전 명세에 없던 심볼.** 장치 세대/모델군 식별자를 얻는 함수로 추정 — 트랙패드 세대별로 판정 로직을 분기할 가능성 `(추정)` | 심볼 사용 확정. 시그니처 `(추정)` |
| 대칭 API(정지·해제) | `MTDeviceStop`, `MTUnregisterContactFrameCallback`, `MTDeviceRelease` 류의 정리 함수가 합리적으로 존재할 것으로 추정되나, `nm -u` 목록에서 이 이름들은 확인되지 않았다 | `(미확정)` — 함수명 자체가 여전히 불명. §9 |
| TCC 권한 | app-bundle-analysis.md §3.3 은 SuperKey 가 명시적으로 확인하는 권한이 Accessibility 하나뿐이라고 확정했다 — MultitouchSupport 호출 자체에 대한 별도 TCC 프리플라이트 심볼(`IOHIDCheckAccess` 류)은 발견되지 않았다 | 부분 승격: "앱이 명시적으로 확인하지는 않는다"는 정황까지는 실측됐으나, OS 가 암묵적으로 권한을 요구하는지는 여전히 ❓미확인 — §9 |

비공개 API 사용의 함의:

- **App Store 배포 불가** — 하지만 SuperKey 는 직접 배포(`.dmg`, Sparkle)이므로 이 제약은 해당 없음(superkey-inventory.md §1.5).
- **OS 업데이트로 깨질 위험** — 공개 계약이 없으므로 Apple 이 언제든 함수를 제거·변경해도 사전 고지가 없다. §5 항목 9 의 방어 코드가 필수.
- **헤더가 없어 함수 시그니처를 직접 선언해야 함** — 컴파일러가 시그니처를 검증해 주지 않으므로, 잘못된 타입 선언은 조용한 메모리 손상으로 이어질 수 있다.

## 7. 구현 접근

**판정: Rust 바인딩(단, 손으로 작성한 비공개 API 바인딩 — 다른 F-0N 명세의 "Rust 바인딩"과 위험 등급이 다르다).**

rust-macos-capability-notes.md §1.1 이 정리한 `objc2-*` 계열은 모두 **공개 프레임워크의 헤더**로부터 자동 생성된 바인딩이다. `MultitouchSupport.framework` 는 공개 헤더가 없으므로 이 생성 파이프라인에 들어갈 수 없다 — `objc2` 자동 생성 바인딩 경로는 원천적으로 쓸 수 없다.

⭐ `otool -L`/`nm -u` 로 SuperKey 자신이 이 프레임워크를 **직접 링크**해 쓴다는 것이 확정되었으므로(§6.2), "SuperKey 와 같은 경로가 실제로 존재하고 동작한다"는 전제 자체는 더 이상 유추가 아니라 실측 사실이다. 그러나 `MTDeviceCreateList`/`MTRegisterContactFrameCallbackWithRefcon`/`MTDeviceStart` 등이 (이름과 `nm -u` 심볼 테이블에 C 심볼 형태로 나타난다는 관례로 볼 때) **C ABI 로 노출된 일반 함수**이지, Objective-C 클래스 메서드가 아니라는 것은 여전히 `(추정)`이다 — `nm` 은 심볼 존재를 보여줄 뿐 호출 규약까지 증명하지 않는다. 이 전제가 맞다면:

- Rust 쪽에서 `extern "C" { fn MTDeviceCreateList() -> ...; ... }` 형태로 함수 시그니처를 **손으로 선언**하고, 빌드 스크립트(`build.rs`)에서 `#[link(name = "MultitouchSupport", kind = "framework")]` 와 `PrivateFrameworks` 검색 경로(`-F /System/Library/PrivateFrameworks`)를 지정해 **직접 링크**하는 것만으로 호출이 가능하다.
- 콜백 함수(`MTRegisterContactFrameCallbackWithRefcon` 에 넘기는 함수 포인터)도 Rust 의 `extern "C" fn(...)` 로 정의한 함수를 그대로 C 함수 포인터로 넘길 수 있어, 별도로 컴파일된 Swift/Objective-C 트램폴린 파일이 **필요하지 않다**. `WithRefcon` 변형은 콜백에 사용자 컨텍스트(예: 어느 장치·어느 진입 영역인지)를 함께 실어 보낼 수 있어, 여러 장치(트랙패드+Magic Mouse, §3.5)를 동일한 콜백 함수 하나로 처리하기에도 유리하다.

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
- [ ] 트랙패드도 Magic Mouse 도 없는 기기(일반 서드파티 마우스/외장 키보드만)에서는 이 기능이 어떤 조건에서도 hyper 를 활성화하지 않으며, 크래시 없이 안전하게 무효 상태를 유지한다(§5 항목 1 — Magic Mouse 는 대상 기기이므로 이 조건에서 제외).
- [ ] `Engage hyper key using trackpad:` 가 ☑ 이고 **Magic Mouse** 가 연결된 기기에서도, 선택된 영역에서 손가락 1개를 안쪽으로 슬라이드하면 hyper 활성 신호가 방출된다(§3.5). Magic Mouse 연결 해제 시 hyper 가 `Engaged` 상태였다면 강제 해제된다(§5 항목 14).
- [ ] 세션 도중 외장 Magic Trackpad 를 연결하면 재시작 없이 새 장치에서도 제스처가 인식된다.
- [ ] 세션 도중 사용 중이던 외장 트랙패드가 연결 해제되면, hyper 가 `Engaged` 상태였을 경우 강제로 해제되고 stuck 상태가 되지 않는다.
- [ ] 절전 복귀 직후에도 (재접속 지연을 허용하는 합리적 시간 내) 트랙패드 제스처 인식이 정상 동작한다.
- [ ] `Engage hyper key using trackpad:` 를 끄면, 이미 `Engaged` 상태였던 hyper 가 즉시 강제 해제된다.
- [ ] MultitouchSupport 심벌 로드나 콜백 데이터 해석이 실패하는 경우, 앱이 크래시하지 않고 이 기능만 비활성화된 채로 나머지 기능(물리 키 hyper 리매핑 등)이 정상 동작한다.
- [ ] hyper 가 트랙패드 경로로 활성화된 상태에서 물리 키를 통한 hyper 활성화가 동시에 일어나도(F-05 소관 병합 규칙 하에) 중복되거나 모순된 modifier 상태가 발생하지 않는다.
- [ ] ⭐ 접촉 프레임이 일정 시간 이상 수신되지 않으면(watchdog) 자동으로 트랙패드 리스너를 재시작한다 — SuperKey 원본의 `Touches are not being detected. Restarting.` 로직과 동등한 동작(§5 항목 6).
- [ ] ⭐ 진입 영역 접촉이 트리거 임계값에 도달하기 전, 프리즈 임계값(§3.2.1)을 넘는 시점부터 시스템 커서 이동이 그 접촉으로부터 분리(freeze)된다 — 손가락을 밀어 넣는 동작이 일반 트랙패드 커서 이동으로 오인되지 않는다.

## 9. 미해결 질문

**이번 실측으로 해소된 구 질문 (app-bundle-analysis.md 근거, 한 줄 요약):**

- ~~영역 선택 팝업의 전체 후보 목록~~ → §4 에서 5종 전량·표시 순서 확정, "4코너+상단" 추정이 정확히 맞았음(실측: AX 트리, §6.2).
- ~~`MultitouchSupport.framework` 가 실제로 Superkey 의 구현 경로인지~~ → `otool -L` 로 직접 링크 확정, SuperKey 유일의 PrivateFramework(실측: 번들 심볼, §3.1).
- ~~`Engage hyper key using trackpad:` 와 영역 팝업의 출고 기본값~~ → defaults 부재 + AX 실측 이중 근거로 ☐/`top right` 확정(실측: defaults + AX 트리, §2.1, §6.2).

**남은 질문 (재정리, 일부는 부분 해소):**

| # | 질문 | 왜 확정 못 했는가 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | `cornerTriggerThreshold`/`cornerFreezeThreshold`/`oneTopTriggerThreshold`/`oneTopCursorFreezeThreshold`(§3.2.1)와 `GESTURE_TIMEOUT` 의 실제 수치(부분 해소 — **이름**은 확정) | 조사 자료에 정량값 없음. 공개 헤더 없는 비공개 프레임워크라 리버스 엔지니어링 없이는 값을 읽을 수 없다 | 앱 설치 후 실측(트랙패드 로깅), 또는 개발자 문의 |
| 2 | 코너에서의 "안쪽" 방향이 정확히 대각선인지, 허용 각도 범위는 얼마인지 (§3.3) — `hyperSlideDownTemplate` 이름이 상단 가장자리 쪽 해석만 부분적으로 뒷받침 | 랜딩("down")과 스크린샷 부제("in")의 표현이 상이하고 정량 근거 없음. 코너 4종 이미지 이름은 위치만 표기해 방향 정보가 없음 | 앱 설치 후 실동작 확인 |
| 3 | `MTDeviceCreateList`/`MTRegisterContactFrameCallbackWithRefcon`/`MTDeviceStart`/`MTDeviceCreateFromDeviceID`/`MTDeviceGetSensorSurfaceDimensions` 등의 정확한 함수 시그니처와 콜백 데이터 구조체 레이아웃(부분 해소 — **심볼명**은 `nm -u` 로 확정) | 공개 헤더 없음. 이 문서는 함수 이름 외 세부값을 짓지 않기로 결정 | 커뮤니티 역공학 헤더 확보(공개 GitHub 등) 후 실기 검증 |
| 4 | 트랙패드 원시 데이터 접근에 TCC 권한(Accessibility/Input Monitoring 외 별도 권한)이 필요한가(부분 해소 — 앱이 **명시적으로 프리플라이트하지는 않는다**는 것은 확정, §6.2) | OS 가 암묵적으로 권한을 요구하는지는 실기 확인 필요 | 실기에서 권한 프롬프트 발생 여부 관찰 |
| 5 | 콜백이 macOS 드라이버 레벨 팜 리젝션 **이전** 데이터를 주는지 **이후** 데이터를 주는지 (§5 항목 11) — SuperKey 자체 필터 로직(`restingThumb` 등)의 존재는 확정됐으나 이 질문 자체는 해소되지 않음 | 비공개 API 라 동작 문서 없음 | 실기 테스트(의도적 손바닥 접촉으로 재현) |
| 6 | 두 번째 손가락이 `Engaged` 상태 도중 추가되는 경우 유지할지 강제 해제할지 (§3.2 표) | 조사 자료에 근거 없음 | 앱 설치 후 실동작 확인, 또는 개발자 문의 |
| 7 | 절전 복귀 시 `MTDevice` 핸들이 `CGEventTap` 과 같은 계열로 무효화되는지 (§5 항목 7) | MultitouchSupport 문서 없음. `CGEventTap` 사례(v1.58)로부터의 유비 추론일 뿐 | 실기에서 절전 복귀 반복 테스트 |
| 8 | 화면 잠금 중 제스처 인식 억제를 F-06 자신이 할지 F-07 이 할지의 책임 경계 (§5 항목 10) | 조사 자료에 근거 없음 | F-07 명세 확정 시 함께 결정 |
| 9 | ⭐ 프리즈 임계값 도달 시점과 트리거 임계값 도달 시점의 정확한 순서·중첩 여부, 그리고 Engaged 상태 동안 커서 고정이 계속 유지되는지 (§3.2.1, 신규) | 파라미터 이름만 확인되고 동작 순서는 문서화되어 있지 않음 | 앱 설치 후 실동작 확인(커서가 실제로 멈추는지 관찰), 또는 개발자 문의 |
| 10 | ⭐ Magic Mouse 전용 임계값·판정 로직이 트랙패드와 별도로 존재하는지 (§3.5, §5 항목 13, 신규) | `corner*`/`oneTop*` 파라미터가 트랙패드 영역 기준으로만 이름 붙어 있고 Magic Mouse 전용 파라미터 이름은 발견되지 않음 | 앱 설치 후 Magic Mouse 로 실동작 확인, 또는 개발자 문의 |
| 11 | ⭐ `trackpadClickSupport`/`MouseClickSupport`/`TwoButtonSwapped`/`MouseButtonDivision` 의 의미와 `OneDraw` 제스처와의 관련성 (§4, 신규) | 대응 UI 를 찾지 못함 | 앱 설치 후 관련 설정 변경 실험, 또는 개발자 문의 |
| 12 | ⭐ `OneDraw` 외 다른 인식기(`TapRecognizer`/`ClickRecognizer`/`ForceRecognizer`/`threeTapRecognizer`/`fourTapRecognizer`)와의 우선순위·충돌 해소 규칙 (§3.1, 신규) | 인식기 이름만 확인되고 중재 로직은 문서화되어 있지 않음 | 앱 설치 후 여러 제스처를 동시에 유발해 실동작 확인 |
