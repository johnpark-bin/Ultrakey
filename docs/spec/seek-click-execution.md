# F-04 · Seek — 클릭 실행

> 선택된 매치의 화면 좌표를 받아 실제 클릭을 합성하는 마지막 단계.
> 의존성: `F-01`(세션·확정 이벤트) → `F-02`(후보 검출·좌표) → `F-03`(오버레이 해제) → **F-04(본 문서)**. 전제 권한: `F-11`(Accessibility). 키 이벤트 가로채기 일반은 `F-07`.
> 관련 명세: [`F-01` 세션·선택 상태 관리](./seek-activation-and-session.md) · [`F-02` 후보 검출과 좌표 산출](./seek-text-detection.md) · [`F-03` 오버레이 렌더링·해제](./seek-overlay-ui.md) · [`F-07` CGEventTap 설치](./key-remapping-engine.md) · [`F-11` 권한](./permissions-onboarding.md)

---

## 1. 개요

Seek 의 확정 동작("Enter" 또는 hold 모드의 리매핑 키 해제, `F-01` 정의)이 발생하면, 선택된 매치 하나가 F-04 로 넘어온다. F-04 는 이 매치를 받아 다음을 수행하는 것이 전부다:

1. (설정에 따라) 클릭 대상 창을 포커스한다.
2. 오버레이를 해제한다.
3. 화면 좌표에 클릭(또는 우클릭·더블클릭·가운데클릭)을 합성한다.
4. (설계 결정에 따라) 커서 위치를 원래대로 되돌린다.

입력은 F-02 가 만든 매치 객체 하나이며, 최소한 다음을 포함한다고 가정한다: 출처(OCR 또는 AX), 화면상 bounding box(OCR) 또는 `AXUIElement` 핸들(AX), 소속 프로세스 pid. F-04 는 이 입력의 **생성 로직**(OCR 정확도, AX 트리 순회, 후보 랭킹)에는 관여하지 않는다 — 오직 "이미 확정된 매치를 어떻게 클릭으로 바꾸는가"만 다룬다.

이 기능은 SuperKey 의 핵심 약속("Match what you type, and click it — all with the keyboard and anywhere on the screen")을 물리적으로 실행하는 지점이며, 두 조사 문서(§Seek 탭 스크린샷, §역량 조사 2.1/2.2/2.6)에서 확정된 사실만을 근거로 한다.

---

## 2. 사용자 시나리오

**시나리오 A — 기본 클릭 (두 설정 모두 기본 동작)**
사용자가 Caps Lock 을 눌러 Seek 를 hold 모드로 띄우고(F-01) "Save" 를 타이핑해 버튼 텍스트를 매칭시킨 뒤, 손가락을 뗀다. 대상 앱 창이 이미 최전면이면 오버레이가 사라지고 "Save" 버튼 위치에 좌클릭이 합성된다.

**시나리오 B — 비활성 창의 버튼을 원클릭으로 활성화+클릭**
`Focus window before clicking` 이 켜진 상태에서, 사용자가 배경에 있는 다른 앱 창의 링크 텍스트를 Seek 로 찾아 확정한다. SuperKey 가 먼저 그 창을 활성화한 다음 같은 동작으로 클릭까지 완료한다 — macOS 기본 동작(첫 클릭이 활성화에만 소모되는 앱)과 달리 클릭 1회로 끝난다.

**시나리오 C — modifier 로 우클릭**
`Change click modes with modifier keys` 가 켜진 상태에서, 사용자가 Seek 확정 순간 Control 을 누르고 있다. 매치 위치에 좌클릭 대신 우클릭(컨텍스트 메뉴 트리거)이 합성된다.

**시나리오 D — modifier 를 클릭에 그대로 전달**
같은 설정이 꺼진 상태에서, 사용자가 ⌘ 을 누른 채 Seek 를 확정해 링크를 클릭한다. 클릭 종류는 항상 좌클릭이고, 대신 ⌘ 이 클릭 이벤트에 실려 브라우저가 "새 탭에서 열기"로 해석한다.

**시나리오 E — 확정 직전 대상이 사라짐**
사용자가 매치를 확정하는 순간 대상 앱이 그 창을 닫는다. 클릭은 실패하지만 SuperKey 는 크래시하거나 엉뚱한 창을 클릭하지 않는다(§5).

---

## 3. 동작 명세

### 3.1 확정 입력부터 클릭 완료까지 — 순서도 수준 단계

```
[F-01: 확정 이벤트 수신]
        │  (Enter, 또는 hold 모드에서 리매핑 키 해제)
        ▼
[1] 확정 시점의 눌린 modifier 키 스냅샷을 캡처
        │
        ▼
[2] 확정된 매치의 출처 확인 — OCR(bounding box만) / AX(AXUIElement 핸들 보유)
        │
        ▼
[3] `Change click modes with modifier keys` 판정
        ├─ ON  → 스냅샷 modifier 조합을 "클릭 종류"로 해석 (§3.3, §9 Q10)
        └─ OFF → 클릭 종류는 항상 기본 좌클릭. modifier 는 4단계에서
                  클릭 이벤트의 CGEventFlags 로 그대로 실을 값으로 보류
        ▼
[4] 오버레이 해제 (F-03 위임, 동기 완료 보장 — §5, §6)
        ▼
[5] `Focus window before clicking` 판정
        ├─ ON  → 대상 지점의 AX 윈도우 요소 조회 → pid 확보
        │         → NSRunningApplication.activate → 그 윈도우에 kAXRaiseAction
        └─ OFF → 아무 동작 없음
        ▼
[6] 클릭 좌표 계산 — bounding box/AX frame 중심점, 픽셀→포인트 환산,
        전역(다중 디스플레이) 좌표계로 정렬 (§3.4)
        ▼
[7] 클릭 합성 전략 선택 — 후보 출처 × 클릭 종류 (§3.3 표)
        ├─ AX 후보 + 기본 좌클릭 + kAXPressAction 지원
        │        → AXUIElementPerformAction(kAXPressAction)
        └─ 그 외 전부(OCR 후보 / 우·중간·더블클릭 / Press 미지원)
                 → CGEventCreateMouseEvent + CGEventPost (좌표 기반),
                   OFF 상태였다면 3단계에서 보류한 modifier 를 CGEventFlags 로 부여
        ▼
[8] 클릭 완료 후 커서 위치 복구 — CGWarpMouseCursorPosition (§3.5)
        ▼
[9] 세션 상태 정리 → F-01 로 위임 (본 문서 범위 밖)
```

### 3.2 `Focus window before clicking` × `Change click modes with modifier keys` 조합별 동작

| | `Change click modes` **ON** | `Change click modes` **OFF** |
| :--- | :--- | :--- |
| `Focus window before clicking` **ON** | 대상 창을 먼저 활성화한 뒤, 확정 시점 modifier 조합이 가리키는 클릭 종류(좌/우/중간/더블)를 그 창에 합성. "활성화 + 우클릭" 등이 한 번의 확정으로 끝난다. | 대상 창을 먼저 활성화한 뒤, 항상 좌클릭을 합성하되 modifier 를 클릭 이벤트에 얹는다. 활성화 자체는 modifier 와 무관하게 항상 수행. |
| `Focus window before clicking` **OFF** | 창 활성화 없이 클릭 종류만 modifier 로 전환. 대상이 비활성 창이면 macOS 기본 동작을 따른다 — 앱에 따라 첫 클릭이 활성화에만 소모되고 우클릭/더블클릭이 무시될 수 있음(§5). | 창 활성화 없이 modifier 를 얹은 좌클릭만 합성. 두 설정 모두 SuperKey 개입이 최소인 조합. |

### 3.3 후보 출처(OCR/AX) × 클릭 종류별 합성 전략

| 후보 출처 | 클릭 종류 | 전략 | 근거 |
| :--- | :--- | :--- | :--- |
| OCR (bounding box만 보유) | 좌클릭 | `CGEventCreateMouseEvent` + `CGEventPost` | AX 요소 핸들이 없으므로 좌표 기반 합성만 가능 |
| OCR | 우/중간/더블클릭 | `CGEventCreateMouseEvent` + `CGEventPost` (이벤트 타입·`kCGMouseEventClickState` 조정) | 동일 |
| AX (핸들 보유) | 좌클릭, `kAXPressAction` 지원 확인됨 | `AXUIElementPerformAction(kAXPressAction)` | 요소 핸들이 있으므로 좌표·화면 밖·다중 디스플레이 문제를 완전히 우회하는 요소 기반 경로를 우선 사용. 스크롤되어 화면 일부만 보이거나 좌표가 살짝 어긋나도 요소 자체를 누르므로 더 견고 |
| AX | 좌클릭, `kAXPressAction` **미지원**(`kAXErrorActionUnsupported`) | `CGEventCreateMouseEvent` + `CGEventPost` (AX frame 중심 좌표 사용) | Press 액션이 없는 순수 텍스트 요소 등은 좌표 기반으로 폴백 |
| AX | 우/중간/더블클릭 | `CGEventCreateMouseEvent` + `CGEventPost` | `kAXPressAction` 은 의미상 "기본 동작 1회 실행"일 뿐 우클릭·더블클릭에 대응하는 AX 액션이 표준화되어 있지 않다. modifier 로 클릭 종류가 바뀌는 순간부터는 출처와 무관하게 항상 좌표 기반 경로로 통일 |

> 원칙: **AX 요소 기반 경로(`kAXPressAction`)는 "기본 좌클릭 + AX 후보 + Press 지원"이라는 좁은 조건에서만** 쓰고, 그 외 모든 조합은 좌표 기반 `CGEvent` 경로로 통일한다. 두 경로를 넓게 섞으면 클릭 종류·좌표계 처리 로직이 출처마다 갈라져 유지보수 비용이 커진다.

### 3.4 클릭 좌표 계산

- **점 선택**: bounding box(OCR) 또는 AX `kAXPositionAttribute`/`kAXSizeAttribute` 로 구한 frame(AX) 의 **중심점**을 클릭 지점으로 삼는다. 좌측 기준점은 채택하지 않는다 — 버튼 테두리나 아이콘 여백에 걸려 클릭이 무효화될 위험이 중심점보다 크다.
- **좌표계 정합**: OCR 은 Vision/`CGImage` 캡처 결과이므로 **픽셀 단위**(Retina 배율 반영)로 나온다. `CGEventCreateMouseEvent`/AX API 는 **포인트 단위** 전역 좌표를 기대한다. 캡처에 사용한 디스플레이의 `backingScaleFactor` 로 나눠 포인트로 환산한 뒤, 해당 디스플레이의 전역 원점 오프셋을 더해 합성한다.
- **다중 디스플레이**: 모든 좌표는 macOS 전역 좌표계(주 디스플레이 좌상단을 기준으로 하는 `CG`/AX 공통 원점)로 정렬한다. `NSScreen.screens` 는 좌하단 원점(Cocoa 좌표계)이므로 이를 그대로 `CGEvent` 좌표에 섞으면 Y 축이 뒤집힌다 — 오버레이(F-03)와 검출(F-02)이 이미 전역 좌표를 넘겨준다는 전제 하에, F-04 는 **좌표계 변환을 다시 수행하지 않고 입력값을 그대로 신뢰**하되, 디스플레이마다 배율이 다를 수 있다는 점만 클릭 지점 계산 시 재확인한다(§5 엣지 케이스).

### 3.5 커서 위치 처리 — 결정: 클릭 후 원래 위치로 복구한다

- **결정**: 클릭 직전 실제 커서의 전역 좌표를 기억해 두고, 클릭 이벤트 합성이 끝난 뒤 `CGWarpMouseCursorPosition` 으로 그 좌표로 되돌린다.
- **근거**: `CGEventPost` 로 내보낸 마우스 이벤트는 HID 이벤트 스트림에 실제로 합류하므로, 이벤트에 실린 위치로 **화면상 커서가 실제로 이동한다** — 클릭 위치와 커서 표시를 분리해서 "안 보이게" 클릭하는 방법은 없다. 따라서 커서 이동 자체는 막을 수 없고, 사용자 체감을 결정하는 것은 "이동 후 그대로 둘 것인가, 원위치로 되돌릴 것인가"뿐이다. SuperKey 는 "마우스 없이 화면을 조작한다"는 제품 철학을 갖고 있으므로(조사 §1.1 "Navigate your screen without a mouse or trackpad"), Seek 사용 시점의 커서는 대개 사용자가 신경 쓰지 않는 위치에 놓여 있다고 보는 편이 안전하다. 클릭 후 그 위치를 보존해 두면 사용자가 다시 트랙패드/마우스를 잡았을 때 이전 컨텍스트(예: 다른 문서의 커서 근처)를 잃지 않는다.
- **구현 유의점**: `CGWarpMouseCursorPosition` 직후 실제 물리 마우스가 큐에 남긴 이동 델타와 충돌해 커서가 다시 튈 수 있다. 워프 전후로 `CGAssociateMouseAndMouseCursorPosition(false)` → 워프 → `CGAssociateMouseAndMouseCursorPosition(true)` 로 감싸는 처리가 필요하다.

### 3.6 오버레이 해제와 클릭의 순서

오버레이(F-03)는 **클릭 좌표 계산·포커스 전환보다 먼저, 동기적으로** 해제한다(§3.1 4단계). 이유:

1. 오버레이가 입력 투명(`ignoresMouseEvents`)이라 해도, 이를 유일한 방어선으로 삼지 않는다 — 상태 전환 중 일시적으로 플래그가 어긋날 가능성에 대비한 방어적 순서다.
2. `Focus window before clicking` 이 켜져 있으면 대상 창이 새로 최전면으로 올라오는데, 이때 오버레이가 여전히 `always_on_top` 이면 시각적으로 오버레이가 새로 올라온 창 위에 남아있는 것처럼 보인다. 활성화 전에 오버레이를 먼저 치워야 자연스럽다.
3. 오버레이 윈도우의 order-out 은 AppKit 호출 자체는 동기지만, 윈도우 서버 합성(compositing)까지는 다음 프레임(약 16ms, 60Hz 기준)이 걸릴 수 있다. 클릭 합성까지 포함한 전체 파이프라인(포커스 전환 대기 등, §5)이 이보다 오래 걸리는 것이 일반적이므로 별도의 강제 대기는 두지 않되, 최소 한 프레임의 여유가 자연스럽게 확보되도록 순서를 오버레이 해제 → 포커스 → 클릭으로 고정한다.

### 3.7 Focus 전환 방법 — 결정: `NSRunningApplication.activate` + 대상 윈도우 `kAXRaiseAction` 병행

세 후보(`AXUIElementPerformAction(kAXRaiseAction)` / `AXUIElementSetAttributeValue(kAXFrontmostAttribute)` / `NSRunningApplication.activate`)를 검토한 결과:

- `kAXFrontmostAttribute` 는 **앱 전체**를 최전면으로 만들 뿐, 그 앱에 창이 여러 개일 때 어느 창이 함께 올라오는지 보장하지 않는다.
- `kAXRaiseAction` 은 **특정 창**을 올리지만(Apple 문서상 "앱을 활성화하지는 않는다"는 취지의 동작 — 창만 올라오고 다른 앱이 여전히 키보드 포커스를 쥔 상태로 남을 수 있음), 단독으로는 클릭이 실제로 그 창에 도달한다는 보장이 약하다.
- `NSRunningApplication.activate(options:)` 는 **앱 활성화**(키보드 포커스 이전 포함)를 보장하지만 어느 창이 앞으로 오는지는 앱 자체의 재량이다.

→ **둘을 병행**한다: 먼저 `NSRunningApplication.activate` 로 앱을 활성화해 키보드/클릭 포커스가 실제로 그 앱으로 넘어가게 하고, 이어서 클릭 대상이 속한 **특정 창**에 `kAXRaiseAction` 을 수행해 같은 앱의 다른 창이 대신 앞에 오는 경우를 방지한다. 대상 윈도우 요소는 클릭 지점에 대해 `AXUIElementCopyElementAtPosition`(시스템 전역 요소 기준)으로 조회한다 — 이 조회는 OCR/AX 두 후보 출처 모두에 동일하게 적용 가능하므로(둘 다 최종적으로는 화면 좌표를 가지므로), 포커스 전환 로직은 후보 출처에 따라 분기하지 않는다.

---

## 4. 설정 항목

| 이름 (원문) | 타입 | 기본값 | 유효범위 | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| `Focus window before clicking` | 체크박스(불리언) | ☑ (스크린샷 관찰값 — 실제 출고 기본값은 미확정, 조사 §3.4 "홍보용 구성" 주의 및 Q3 참조) | ON / OFF | `docs/research/superkey-inventory.md` §3.1, 스크린샷 `seekScreenshot.png` |
| `Change click modes with modifier keys` | 체크박스(불리언), ⓘ 툴팁 보유 | ☑ (동일 사유로 미확정) | ON / OFF | 동일. 부제 원문: "If this setting is disabled, modifiers will be applied to the click" |

두 설정 모두 **Seek 탭**에 위치하며, 팝업이나 슬라이더 같은 부가 파라미터는 없는 단순 불리언이다(조사 문서에서 추가 세부 컨트롤이 확인되지 않음).

---

## 5. 엣지 케이스와 실패 모드

| # | 케이스 | 처리 방침 |
| :--- | :--- | :--- |
| 1 | 클릭 직전 대상 창이 닫힘 | AX 조회(`AXUIElementCopyElementAtPosition` 또는 저장해 둔 핸들 접근)가 `kAXErrorInvalidUIElement`/`kAXErrorCannotComplete` 로 실패 → 클릭을 조용히 취소하고 세션 종료(F-01). 사용자에게 별도 오류 UI는 띄우지 않는다(원본 제품이 이런 실패에 대해 오류 다이얼로그를 노출한다는 근거 없음) |
| 2 | 대상 창이 확정 직전에 이동함 | 좌표 기반(OCR) 매치는 이동을 반영하지 못해 빈 공간을 클릭할 수 있음. AX 기반 매치는 클릭 직전 `kAXPositionAttribute` 를 재조회해 최신 좌표를 반영. OCR 경로는 이 재조회 수단이 없다는 근본적 한계를 인정하고 넘어간다(F-02 범위, 여기서는 완화책 없음) |
| 3 | 대상이 다른 Space(가상 데스크톱) | `Focus window before clicking` ON 이면 `activate` 가 Space 전환까지 유발할 수 있음(대상 앱의 `NSWindowCollectionBehavior` 에 의존). OFF 면 클릭이 현재 Space 의 좌표에 합성되어 엉뚱한 화면을 클릭할 위험 — 이 경우는 사용자 설정(OFF)의 명시적 트레이드오프로 취급하고 별도 감지·경고는 하지 않는다 |
| 4 | 전체화면 앱이 대상 | 전체화면 앱은 자신만의 Space 를 갖는다. 오버레이(F-03)가 전체화면 Space 위에 뜨는 것 자체가 별도 과제이며, 클릭 합성 자체는 좌표만 맞으면 정상 동작 — 좌표계 이슈는 F-02/F-03 의존 |
| 5 | 대상 요소가 비활성/disabled 상태 | `kAXPressAction` 수행 시 `kAXErrorActionUnsupported` 또는 액션은 성공해도 UI 반응 없음. 전자는 좌표 기반 폴백(§3.3)으로 전환, 후자(비활성 버튼에 조용히 씹히는 클릭)는 SuperKey 도 감지 수단이 없으므로 그대로 둔다 |
| 6 | AX 요소 핸들이 확정 시점에 만료(stale) | `AXUIElementPerformAction` 호출이 `kAXErrorInvalidUIElement` 반환 → 좌표 기반(bounding box 재계산 불가 시 마지막으로 알려진 AX frame)으로 1회 폴백 시도, 그것도 실패하면 클릭 포기 |
| 7 | 계산된 좌표가 화면 밖(디스플레이 구성 변경 등) | 클릭 전 전역 좌표가 현재 `NSScreen.screens` 유니온 프레임 안에 있는지 검증하고, 벗어나면 클릭을 실행하지 않고 세션을 취소한다 (v1.55 다중 디스플레이 이력에 대한 방어) |
| 8 | 클릭이 드래그로 오인됨 | `CGEventCreateMouseEvent` 의 mouseDown/mouseUp 을 같은 좌표로, 앱이 드래그로 해석하지 않을 만큼 짧은 간격으로 발행한다. 정확한 임계값은 미실측(§9) |
| 9 | 포커스 전환이 느린 앱(activate 후 실제 키 창이 되기까지 지연) | 고정 딜레이 삽입은 지어내지 않는다 — 대신 AX 관찰(`kAXFocusedWindowChangedNotification` 구독 등)로 전환 완료를 확인하는 방식을 후보로 남기고 §9 미해결 질문으로 승계 |
| 10 | 확정 시점에 modifier 가 아직 눌려 있음(hold 모드) | hold 모드 확정은 리매핑 키를 떼는 순간(F-01)이며, 클릭 종류/전달용으로 쓰이는 modifier 는 **그 순간의 스냅샷**(§3.1 1단계)으로 고정한다. 리매핑 키 자신이 modifier 후보이기도 한 경우(예: Caps Lock 이 Seek 트리거이면서 동시에 클릭 종류 전환 modifier 로 쓰이길 기대하는 상황)는 F-07 의 이벤트 탭이 그 키 자체를 이미 소비하므로 애초에 modifier 로 관측될 수 없다 — 이 조합은 구조적으로 성립하지 않음을 문서화만 해 둔다 |
| 11 | Secure Input 활성(암호 필드에 포커스) | 조사 문서에 따르면 Secure Input 은 키 리매핑 전면을 무력화하는 macOS 제약이다. Seek 세션 자체가 F-01/F-07 단계에서 이미 성립하지 않을 가능성이 높지만, 만에 하나 세션이 성립해 F-04 까지 도달했다면 클릭 합성 자체(마우스 이벤트)는 Secure Input 의 영향을 받지 않으므로 정상 수행한다 — 단, 클릭 결과로 열리는 것이 암호 필드라면 그 앱 안에서의 이후 키 입력은 별개 문제(F-07 범위) |
| 12 | 클릭 후 원래 커서 위치 복구 실패 | `CGWarpMouseCursorPosition` 호출 자체가 실패하는 사례는 조사에서 확인되지 않았으나, 반환값 확인 없이 방치하면 커서가 클릭 지점에 남는다. 복구 실패를 별도로 사용자에게 알리지 않고, 커서가 클릭 지점에 남는 것을 안전한 열화(fallback) 상태로 받아들인다 |
| 13 | 다중 디스플레이 간 배율(Retina/비-Retina) 상이 | 클릭 지점이 속한 디스플레이의 `backingScaleFactor` 를 그 디스플레이 기준으로 개별 조회해야 한다 — 캡처 시점 디스플레이와 클릭 시점 디스플레이가 다르면(§케이스 2, 7) 잘못된 배율을 적용해 좌표가 틀어질 수 있음 |
| 14 | Accessibility 권한이 클릭 실행 도중 회수됨 | `AXUIElementPerformAction`/`CGEventPost` 가 조용히 실패(권한 없이는 애초에 `CGEventTap` 도 F-07 에서 죽어 있을 가능성이 높음). 권한 상태 감지·복구는 F-11 범위, 여기서는 "Accessibility 권한이 전제"라는 의존 사실만 남긴다 |

---

## 6. 필요한 플랫폼 API

| API | 용도 | 크레이트 |
| :--- | :--- | :--- |
| `CGEventCreateMouseEvent` | 좌표 기반 클릭(모든 클릭 종류) 이벤트 생성 | `core-graphics` 0.25.0 또는 `objc2-core-graphics` 0.3.2 |
| `CGEventPost(kCGHIDEventTap, event)` | 생성한 마우스 이벤트를 HID 스트림에 주입 | 동일 |
| `CGEventSetFlags` | OFF 상태일 때 modifier 를 클릭 이벤트에 얹기 | 동일 |
| `CGEventSetIntegerValueField(.mouseEventClickState)` | 더블클릭(연속 클릭 카운트) 표현 | 동일 |
| `kCGEventRightMouseDown/Up`, `kCGEventOtherMouseDown/Up` | 우클릭/가운데클릭 이벤트 타입 | 동일 |
| `AXUIElementPerformAction(kAXPressAction)` | AX 후보 + 기본 좌클릭 경로의 요소 기반 클릭 | `axuielement` 0.9.1 |
| `AXUIElementCopyActionNames` | `kAXPressAction` 지원 여부 사전 확인 | `axuielement` 0.9.1 |
| `AXUIElementCopyElementAtPosition` | 클릭 지점의 AX 윈도우 요소 조회(포커스 전환용, 출처 무관 공통 경로) | `axuielement` 0.9.1 또는 `accessibility-sys` 0.2.0 |
| `AXUIElementPerformAction(kAXRaiseAction)` | 대상 특정 윈도우 올리기 | `axuielement` 0.9.1 |
| `AXUIElementSetMessagingTimeout` | AX 호출이 응답 없는 대상 앱에 블로킹되는 것 방지 | `axuielement` 0.9.1 |
| `NSRunningApplication.activate(options:)` | 대상 앱 활성화(키 포커스 이전) | `objc2-app-kit` 0.3.2 |
| `CGWarpMouseCursorPosition` | 클릭 후 커서 원위치 복구 | `core-graphics` 0.25.0 또는 `objc2-core-graphics` 0.3.2 |
| `CGAssociateMouseAndMouseCursorPosition` | 워프 직후 물리 마우스 이동과의 충돌 방지 | 동일 |
| `NSScreen.screens` | 다중 디스플레이 union frame, 좌표 범위 검증 | `objc2-app-kit` 0.3.2 |
| `AXIsProcessTrusted` (전제 확인만, 요청·온보딩은 F-11) | 클릭 합성 가능 여부의 선행 조건 | `accessibility-sys` 0.2.0 |

---

## 7. 구현 접근

**판정: Rust 바인딩** — `objc2` 계열과 `axuielement` 의 `unsafe` FFI 로 전부 호출 가능하며, 별도로 빌드하는 Swift/Objective-C shim 은 불필요하다.

**근거**: §6 의 모든 API가 `objc2-core-graphics`(0.3.2), `axuielement`(0.9.1, 현재 가장 활발히 유지되는 AX 고수준 크레이트), `objc2-app-kit`(0.3.2), 또는 `core-graphics`(0.25.0, `CGEventTap` 안전 래퍼와 동일 생태계) 로 이미 노출되어 있다(`docs/research/rust-macos-capability-notes.md` §1.1, §1.2, §2.1, §2.2). F-03(오버레이)에서 이미 확인된 패턴과 동일하게 — `WebviewWindow::ns_window()` 로 얻은 포인터를 캐스팅해 AppKit 전체에 접근할 수 있었던 것처럼 — 클릭 실행도 네이티브 shim 없이 Rust 프로세스 안에서 전부 해결된다. 다만 `AXUIElement*` 원시 호출과 `CGEvent*` 필드 조작은 안전한 상위 래퍼가 없는 `unsafe` FFI 수준이므로 ①(크레이트만으로 충분)이 아니라 ②로 판정한다.

**채택 크레이트/버전**: `core-graphics` 0.25.0, `axuielement` 0.9.1, `objc2-app-kit` 0.3.2, `objc2-core-graphics` 0.3.2, `accessibility-sys` 0.2.0(보완용 원시 FFI, `axuielement` 가 커버하지 못하는 개별 상수/함수 필요 시).

**기각한 대안**:
- `rdev` 0.5.3 — 전역 키/마우스 이벤트 크레이트지만 **3년간 릴리스 없음**(조사 §2.1), 이벤트 소비·필드 제어가 세밀하지 않아 클릭 종류별 필드 조작(`kCGMouseEventClickState` 등)에 부적합. 신규 의존으로 채택하지 않는다.
- 전용 네이티브(Swift/ObjC) shim 작성 — F-03 오버레이 사례에서 이미 "shim 없이 해결 가능"이 입증된 패턴을 클릭 실행에도 그대로 적용할 수 있어 불필요한 복잡도로 판단, 기각.
- `AXUIElementPerformAction(kAXPressAction)` 을 모든 클릭에 일괄 적용 — §3.3 에서 서술한 대로 우클릭/더블클릭/OCR 후보를 표현할 수 없어 기각. 좁은 조건(AX+기본좌클릭+Press지원)에서만 사용.

---

## 8. 수용 기준

- [ ] `Focus window before clicking` OFF, `Change click modes with modifier keys` OFF 상태에서, AX 후보의 기본 좌클릭이 `kAXPressAction` 으로 합성된다.
- [ ] 같은 조건에서 OCR 후보는 항상 `CGEventCreateMouseEvent`+`CGEventPost` 로 합성된다(AX 경로를 타지 않는다).
- [ ] `Change click modes with modifier keys` ON 이고 확정 시점 Control 이 눌려 있으면, 매치 위치에 우클릭이 합성된다(매핑은 §9 의 `(추정)`을 따르되, 최소한 "무언가 다른 클릭 종류로 전환된다"는 동작 자체는 검증한다).
- [ ] 동일 설정 OFF 상태에서 ⌘ 을 누른 채 확정하면, 클릭 종류는 항상 좌클릭이고 ⌘ 플래그가 클릭 이벤트에 실려 있다.
- [ ] `Focus window before clicking` ON 이고 대상이 비활성 창일 때, 클릭 1회(확정 1회)로 창이 활성화되고 클릭까지 도달한다.
- [ ] `Focus window before clicking` OFF 일 때는 SuperKey 가 별도의 활성화 API 를 호출하지 않는다(활성화 여부는 순수히 macOS/대상 앱의 기본 동작에 위임).
- [ ] 오버레이는 포커스 전환·클릭 좌표 계산보다 항상 먼저 해제된다(§3.6 순서를 어기지 않는다).
- [ ] 클릭 전 대상 창이 닫히면(§5 #1) 클릭이 조용히 취소되고 크래시하지 않는다.
- [ ] 클릭 지점이 현재 디스플레이 구성의 화면 밖으로 계산되면(§5 #7) 클릭을 실행하지 않는다.
- [ ] 클릭 완료 후 커서가 클릭 직전의 원래 좌표로 복귀한다(정상 경로에서).
- [ ] hold 모드 확정 시 클릭 종류 판정에 쓰이는 modifier 스냅샷은 리매핑 키를 떼는 그 순간의 상태를 반영한다(§5 #10).
- [ ] AX 후보에서 `kAXPressAction` 이 `kAXErrorActionUnsupported` 로 실패하면 좌표 기반 경로로 폴백해 클릭이 완료된다.

---

## 9. 미해결 질문

| # | 질문 | 상태 |
| :--- | :--- | :--- |
| Q10(승계, 조사 문서 원출처) | `Change click modes with modifier keys` 의 modifier ↔ 클릭 종류 매핑 | **미확정.** 조사 문서(⓪ ⓘ 툴팁 내용 미공개)에서도 확정하지 못했다. 본 문서는 다음을 `(추정)` 으로만 제안한다 — 근거: ⌃(Control)-클릭은 macOS 의 오랜 보조 클릭(컨텍스트 메뉴) 관례와 일치하므로 우클릭에 대응시키는 것이 가장 자연스럽다. ⌥(Option)은 트랙패드에 물리적 가운데 버튼이 없어 대체 경로가 필요하다는 점에서 가운데클릭 후보로 제안한다. ⇧(Shift)은 남은 후보 중 배정 근거가 가장 약하지만 더블클릭에 임시 배정한다. ⌘(Command)은 SuperKey 자체가 hyper/meh/bleh 조합에 이미 이 키를 깊이 쓰고 있어(조사 §2.1 hyper 조합) 단독 클릭-종류 전환자로 쓰면 조합 충돌 여지가 있다고 보아 배정하지 않는 것을 제안한다. **이 넷 모두 앱 내 ⓘ 툴팁 실측 전까지는 추정이며, 구현 전 반드시 확인 필요.** |
| Q10-a | 위 매핑에서 **복수 modifier 조합**(예: ⌃⇧ 동시)이 눌렸을 때의 동작 — 우선순위 규칙이 있는지, 혹은 정의되지 않은 조합은 무시하는지 | 미확정 |
| Q16 | OCR 캡처 픽셀 좌표 → `CGEvent` 포인트 좌표 변환 시, 캡처 시점과 클릭 시점 사이 디스플레이 구성이 바뀌지 않았다는 전제가 실제로 F-02/F-03 파이프라인에서 얼마나 촘촘히 보장되는지 | 실측 필요(§5 #2, #7, #13 과 연결) |
| Q17 | 클릭이 드래그로 오인되지 않기 위한 mouseDown-mouseUp 간격의 구체적 임계값(ms) | SuperKey 실측값 없음. 지어내지 않음 — 프로토타입 단계에서 실측 필요 |
| Q18 | `Focus window before clicking` 의 "활성화 완료" 판정 방법 — 고정 딜레이 / `kAXFocusedWindowChangedNotification` 구독 / 폴링 중 무엇을 쓸지 | 조사 문서에 근거 없음. §5 #9 에서 후보만 나열, 결정은 보류 |
| Q19 | `kAXPressAction` 지원 여부를 `AXUIElementCopyActionNames` 로 사전 질의하는 것이 모든 대상 앱에서 신뢰할 수 있는지(일부 앱이 액션 이름은 보고하되 실제 수행은 실패하는 사례가 있는지) | 실측 필요 |
| Q20 | 다중 modifier 스냅샷을 "누른 시점"이 아니라 "확정 시점"으로 고정하는 것이 사용자 기대와 맞는지(예: Control 을 먼저 누르고 이후 확정하는 동안 손을 뗀 경우) | UX 검증 필요, 본 문서는 확정 시점 스냅샷을 설계 결정으로 채택했으나 재검토 여지 있음 |
