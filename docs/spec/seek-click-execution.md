# F-04 · Seek — 클릭 실행

> 선택된 매치의 화면 좌표를 받아 실제 클릭을 합성하는 마지막 단계.
> 의존성: `F-01`(세션·확정 이벤트) → `F-02`(후보 검출·좌표) → `F-03`(오버레이 해제) → **F-04(본 문서)**. 전제 권한: `F-11`(Accessibility). 키 이벤트 가로채기 일반은 `F-07`.
> 관련 명세: [`F-01` 세션·선택 상태 관리](./seek-activation-and-session.md) · [`F-02` 후보 검출과 좌표 산출](./seek-text-detection.md) · [`F-03` 오버레이 렌더링·해제](./seek-overlay-ui.md) · [`F-07` CGEventTap 설치](./key-remapping-engine.md) · [`F-11` 권한](./permissions-onboarding.md)
> **근거 문서**: `docs/research/superkey-inventory.md` §3.1, §7 Q3 · `docs/research/rust-macos-capability-notes.md` §2.1, §2.2, §2.6 · `docs/research/app-bundle-analysis.md` §2.1(defaults 실측), §3.1·§3.3(링크 심볼·권한 판정), §5.2(ⓘ 팝오버 원문), §6.1(Seek 탭 AX 실측)

---

## 1. 개요

Seek 의 확정 동작("Enter" 또는 hold 모드의 리매핑 키 해제, `F-01` 정의)이 발생하면, 선택된 매치 하나가 F-04 로 넘어온다. F-04 는 이 매치를 받아 다음을 수행하는 것이 전부다:

1. (설정에 따라) 클릭 대상 창을 포커스한다.
2. 오버레이를 해제한다.
3. **⭐ 확정된 클릭 모드(7종 중 하나, §3.3)를 실행한다** — 클릭하지 않고 커서만 이동, 매치 시작/끝 지점 클릭, 클릭 후 커서 복귀, 클릭→복귀→재클릭, 더블/트리플 클릭+복사 중 하나다. "클릭(또는 우클릭·더블클릭·가운데클릭)을 합성한다"는 기존 서술은 **부정확했다** — 실측 결과 SuperKey 의 클릭 모드는 마우스 버튼 종류(좌/우/중간)가 아니라 **클릭 지점·횟수·커서 복귀 여부의 조합**으로 구성된 7종 프리셋이다(§3.3). 커서 복귀는 모드 하나(`Click and return cursor`)의 정의일 뿐, 모든 클릭에 공통 적용되는 후처리 단계가 아니다.

입력은 F-02 가 만든 매치 객체 하나이며, 최소한 다음을 포함한다고 가정한다: 출처(OCR 또는 AX), 화면상 bounding box(OCR) 또는 `AXUIElement` 핸들(AX), 소속 프로세스 pid. F-04 는 이 입력의 **생성 로직**(OCR 정확도, AX 트리 순회, 후보 랭킹)에는 관여하지 않는다 — 오직 "이미 확정된 매치를 어떻게 클릭으로 바꾸는가"만 다룬다.

이 기능은 SuperKey 의 핵심 약속("Match what you type, and click it — all with the keyboard and anywhere on the screen")을 물리적으로 실행하는 지점이며, 조사 문서와 번들 실측(`app-bundle-analysis.md`)에서 확정된 사실만을 근거로 한다.

### 1.1 ⭐⭐ 가장 큰 신규 확정 — 클릭 모드는 7종이며, `Change click modes with modifier keys` 의 실체다 (실측: 번들 문자열 nib)

`Change click modes with modifier keys` 옆 ⓘ 팝오버 원문(SuperKey 원문, `ClickModesInfoViewController`):
> "Seek Click Modes"
> "Hold the corresponding modifier keys when pressing enter to perform each type of click"
> 목록: "Just move cursor" · "Click at beginning of match" · "Click at end of match" · "Click and return cursor" · "Click, return, click" · "Double click and copy" · "Triple click and copy"

내부 식별자 대응: `onlyMoveCursor` · `clickStartMatch` · `clickEndMatch` · `clickAndReturn` · `clickReturnClick` · `doubleClickCopy` · `tripleClickCopy`. 팝오버 아이콘(SF Symbol): `cursorarrow.motionlines` · `arrow.left.to.line` · `arrow.right.to.line` · `cursorarrow.motionlines.click` · `contextualmenu.and.cursorarrow` · `cursorarrow.click.2` · `doc.on.doc.fill`.

실행 파일에는 팝오버 목록과 다른 표기의 문자열도 있다(SuperKey 원문): "Click and Return" · "Click End of Match" · "Click Start of Match" · "Double Click and Copy" · "Triple Click and Copy" — **같은 모드의 다른 표기**로 보인다 `(미확정)`.

이 발견이 기존 명세를 어떻게 바꾸는가: 기존 명세는 "modifier 가 좌/우/중간/더블클릭 중 클릭 **종류**를 바꾼다"고 전제했다(§3.2, §3.3 구버전). 실측 결과는 다르다 — modifier 가 바꾸는 것은 마우스 버튼 종류가 아니라 **"클릭할지, 어디를 클릭할지, 몇 번 클릭할지, 클릭 후 커서를 되돌릴지"의 조합인 7종 프리셋**이다. 우클릭·가운데클릭이 이 7종에 아예 없다는 점도 중요하다 — 우클릭 시나리오(§2 시나리오 C)를 이 문서가 서술했다면, 그 시나리오의 "우클릭"이라는 결과 자체가 실측과 배치되므로 정정한다(§2, §3.3).

---

## 2. 사용자 시나리오

**시나리오 A — 기본 클릭 (두 설정 모두 기본 동작)**
사용자가 Caps Lock 을 눌러 Seek 를 hold 모드로 띄우고(F-01) "Save" 를 타이핑해 버튼 텍스트를 매칭시킨 뒤, 손가락을 뗀다. 대상 앱 창이 이미 최전면이면 오버레이가 사라지고 "Save" 버튼 위치에 좌클릭이 합성된다.

**시나리오 B — 비활성 창의 버튼을 원클릭으로 활성화+클릭**
`Focus window before clicking` 이 켜진 상태에서, 사용자가 배경에 있는 다른 앱 창의 링크 텍스트를 Seek 로 찾아 확정한다. SuperKey 가 먼저 그 창을 활성화한 다음 같은 동작으로 클릭까지 완료한다 — macOS 기본 동작(첫 클릭이 활성화에만 소모되는 앱)과 달리 클릭 1회로 끝난다.

**시나리오 C — modifier 로 클릭 모드 전환 (⭐ 정정: 우클릭이 아니다)**
`Change click modes with modifier keys` 가 **기본 켜짐**(§4)인 상태에서, 사용자가 Seek 확정 순간 특정 modifier 를 누르고 있다. 매치 위치에 기본 클릭 대신 7종 모드(§1.1, §3.3) 중 그 modifier 에 대응하는 모드가 실행된다 — 예를 들어 `Double click and copy` 라면 매치 텍스트를 더블클릭해 단어를 선택하고 복사한다. 기존 명세가 이 시나리오를 "우클릭이 합성된다"로 서술했다면 정정한다 — 실측된 7종 모드에는 우클릭·중간클릭이 없다(§1.1). **어떤 modifier 가 어느 모드에 대응하는지는 `(미확정)`이다** — §3.3, §9 Q10 참조.

**시나리오 D — modifier 를 클릭에 그대로 전달 (설정이 꺼졌을 때의 이중 의미)**
`Change click modes with modifier keys` 를 사용자가 **직접 꺼둔** 상태에서, 사용자가 ⌘ 을 누른 채 Seek 를 확정해 링크를 클릭한다. `Change click modes...` 부제 원문(SuperKey 원문): "If this setting is disabled, modifiers will be applied to the click" — 즉 이 설정이 꺼지면 modifier 는 클릭 **모드 선택**에 쓰이지 않고 클릭 **이벤트 자체**에 실린다. 클릭 종류는 기본 동작(보통 좌클릭)이고, ⌘ 이 클릭 이벤트의 `CGEventFlags` 로 그대로 실려 브라우저가 "새 탭에서 열기"로 해석하는 식이다. 이 설정은 **Seek 탭에서 유일하게 출고 기본값이 켜짐(☑)** 인 항목이므로(§4), 이 시나리오를 재현하려면 사용자가 명시적으로 꺼야 한다.

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
[3] `Change click modes with modifier keys` 판정 (⭐ 정정 — "클릭 종류" 가 아니라 "클릭 모드")
        ├─ ON(기본값, §4)
        │     → 스냅샷 modifier 조합을 7종 클릭 모드(§1.1, §3.3) 중 하나로 해석
        │        onlyMoveCursor / clickStartMatch / clickEndMatch /
        │        clickAndReturn / clickReturnClick / doubleClickCopy / tripleClickCopy
        │        modifier↔모드 대응표는 `(미확정)` — §9 Q10
        └─ OFF → 클릭 모드는 항상 기본 동작(단일 좌표 클릭). modifier 는 4단계에서
                  클릭 이벤트의 CGEventFlags 로 그대로 실을 값으로 보류
                  (부제 원문: "If this setting is disabled, modifiers will be
                  applied to the click")
        ▼
[4] 오버레이 해제 (F-03 위임, 동기 완료 보장 — §5, §6)
        ▼
[5] `Focus window before clicking` 판정 — 출고 기본값 ☐(§4)
        ├─ ON  → 대상 지점의 AX 윈도우 요소 조회(`AXUIElementCopyElementAtPosition`) → pid 확보
        │         → NSRunningApplication.activate → 그 윈도우에 kAXRaiseAction
        └─ OFF → 아무 동작 없음
        ▼
[6] 클릭 지점 계산 — ⭐ 정정: 모드에 따라 매치 bounding box/AX frame 의
        **시작점 또는 끝점**(clickStartMatch/clickEndMatch) 또는 그 외 모드의
        기준점(§3.3, §3.4). 중심점을 항상 쓴다는 기존 서술은 틀렸다.
        픽셀→포인트 환산, 전역(다중 디스플레이) 좌표계로 정렬
        ▼
[7] 클릭 모드 실행 — 후보 출처 × 클릭 모드 (§3.3 표)
        ├─ onlyMoveCursor → 클릭 없음. CGWarpMouseCursorPosition(또는 programmaticMove)
        │        으로 커서만 매치 지점으로 이동
        ├─ AX 후보 + clickStartMatch/clickEndMatch 상당(단일 클릭) + kAXPressAction 지원
        │        → AXUIElementPerformAction(kAXPressAction) (§3.3 좁은 조건)
        └─ 그 외 전부(OCR 후보 / doubleClickCopy·tripleClickCopy / clickAndReturn /
                 clickReturnClick / Press 미지원)
                 → CGEventCreateMouseEvent + CGEventPost (좌표 기반),
                   OFF 상태였다면 3단계에서 보류한 modifier 를 CGEventFlags 로 부여.
                   합성 클릭이 자기 이벤트 탭으로 되돌아오는 것을 막기 위해
                   blockClicksUntil / stopNextMouseDown / stopNextMouseUp 을
                   병행한다(§3.8, ⭐ 신규)
        ▼
[8] 모드가 요구하면 커서 위치 복구 또는 재클릭 — clickAndReturn: 복구만 /
        clickReturnClick: 복구 후 재클릭 / 그 외 모드: 해당 없음 (§3.5, ⭐ 정정 —
        커서 복구는 전체 공통 후처리가 아니라 모드별 정의다)
        ▼
[9] 세션 상태 정리 → F-01 로 위임 (본 문서 범위 밖)
```

### 3.2 `Focus window before clicking` × `Change click modes with modifier keys` — 이중 의미와 출고 기본값

⭐ **출고 기본값(실측: AX 트리 · defaults, `superkey-inventory.md` §7 Q3 해소)**: `Focus window before clicking` = **☐**, `Change click modes with modifier keys` = **☑**. 스크린샷이 둘 다 ☑ 로 보였다면 그 스크린샷은 홍보용 구성값이지 출고 기본값이 아니다. `Change click modes with modifier keys` 는 **Seek 탭 전체에서 유일하게 출고 기본값이 켜진(☑) 항목**이다 — 나머지 Seek 탭 설정은 전부 ☐(§3.1, F-03 근거).

⭐ **`Change click modes with modifier keys` 의 이중 의미**(부제 원문, SuperKey 원문): "If this setting is disabled, modifiers will be applied to the click". 이 설정은 단순 on/off 가 아니라 **modifier 키의 용도 자체를 바꾼다**:

| | `Change click modes` **ON**(기본값) | `Change click modes` **OFF** |
| :--- | :--- | :--- |
| modifier 의 역할 | 확정 시점 modifier 조합이 **7종 클릭 모드**(§1.1, §3.3) 중 하나를 선택한다. modifier↔모드 대응은 `(미확정)`(§9 Q10) | modifier 는 클릭 모드 선택에 관여하지 않는다. 대신 **클릭 이벤트 자체에 그대로 실린다**(`CGEventFlags`) — 예: ⌘+클릭 → 브라우저가 "새 탭에서 열기"로 해석 |
| `Focus window before clicking` **ON** 과 조합 | 대상 창을 먼저 활성화한 뒤, modifier 가 가리키는 클릭 모드를 그 창에 실행. "활성화 + 특정 모드"가 한 번의 확정으로 끝난다 | 대상 창을 먼저 활성화한 뒤, 기본 클릭 모드를 실행하되 modifier 를 클릭 이벤트에 얹는다. 활성화 자체는 modifier 와 무관하게 항상 수행 |
| `Focus window before clicking` **OFF** 와 조합 | 창 활성화 없이 클릭 모드만 modifier 로 전환. 대상이 비활성 창이면 macOS 기본 동작을 따른다 — 앱에 따라 첫 클릭이 활성화에만 소모되고 특정 모드(더블/트리플클릭 등)가 무시될 수 있음(§5) | 창 활성화 없이 modifier 를 얹은 기본 클릭 모드만 합성. 두 설정 모두 SuperKey 개입이 최소인 조합 |

### 3.3 ⭐⭐ 클릭 모드 7종 — 각 동작의 명세

`Change click modes with modifier keys` 가 가리키는 실체는 7종 클릭 모드다(§1.1). 각 모드의 동작:

| 모드 (SuperKey 원문) | 내부 식별자 | 동작 |
| :--- | :--- | :--- |
| `Just move cursor` | `onlyMoveCursor` | 클릭하지 않는다. 커서만 매치 위치로 이동한다 |
| `Click at beginning of match` | `clickStartMatch` | 매치 영역(bounding box/AX frame)의 **시작 지점**을 클릭한다. ⭐ 정정: 매치 **중심**이 아니라 **시작점**이다 — 기존 명세가 항상 중심점을 클릭한다고 전제했다면 §3.4 에서 정정한다 |
| `Click at end of match` | `clickEndMatch` | 매치 영역의 **끝 지점**을 클릭한다 |
| `Click and return cursor` | `clickAndReturn` | 클릭한 뒤 커서를 원래 위치로 되돌린다. §3.5 의 "항상 커서를 복구한다"는 기존 결정은 **이 모드 하나의 정의**였다 — 다른 모드에는 적용되지 않는다 |
| `Click, return, click` | `clickReturnClick` | 클릭 → 커서 원위치 복귀 → 다시 클릭(원래 위치에서). 두 번째 클릭의 좌표는 커서를 되돌린 원래 위치다 |
| `Double click and copy` | `doubleClickCopy` | 매치 위치를 더블클릭해 단어를 선택하고 복사한다(macOS 기본 더블클릭 단어 선택 동작에 의존) |
| `Triple click and copy` | `tripleClickCopy` | 매치 위치를 트리플클릭해 줄/문단을 선택하고 복사한다(macOS 기본 트리플클릭 동작에 의존) |

관련 저장 키(존재만 확인, 의미는 이름으로부터의 해석 `(미확정)`): `justClick` · `clickCount` · `clickNumber` · `humanClickDownUp` · `programmaticClickDown` · `programmaticClickDownUp` · `onProgrammaticClickDown` · `onProgrammaticClickDownUp` · `programmaticMove` · `blockClicksUntil` · `stopNextMouseDown` · `stopNextMouseUp` · `cursorFreeze` · `middleClick` · `rightClick` · `modifierPassthrough` · `ModifierSeekMode` · `MouseButtonDivision` · `TwoButtonSwapped`.

`middleClick`/`rightClick` 키가 존재한다는 사실은 우클릭·중간클릭 자체가 SuperKey 어딘가에서 쓰인다는 근거는 되지만, 위 7종 팝오버 목록에는 우클릭·중간클릭 항목이 없다 — 이 키들이 클릭 모드 7종과 무관한 다른 기능(예: 트랙패드 제스처, F-06)에 속할 가능성이 있다 `(미확정)`.

### 3.3.1 후보 출처(OCR/AX) × 클릭 모드별 합성 전략

| 후보 출처 | 클릭 모드 | 전략 | 근거 |
| :--- | :--- | :--- | :--- |
| 무관 | `onlyMoveCursor` | `CGWarpMouseCursorPosition`(§3.5 워프 절차) 또는 `programmaticMove` | 클릭 이벤트 자체가 없으므로 출처와 무관 |
| OCR (bounding box만 보유) | `clickStartMatch`/`clickEndMatch`(기본 클릭 상당) | `CGEventCreateMouseEvent` + `CGEventPost` | AX 요소 핸들이 없으므로 좌표 기반 합성만 가능 |
| OCR | `clickAndReturn`/`clickReturnClick`/`doubleClickCopy`/`tripleClickCopy` | `CGEventCreateMouseEvent` + `CGEventPost` (이벤트 타입·`kCGMouseEventClickState` 조정) | 동일 |
| AX (핸들 보유) | `clickStartMatch`/`clickEndMatch` 상당의 단일 클릭, `kAXPressAction` 지원 확인됨 | `AXUIElementPerformAction(kAXPressAction)` | 요소 핸들이 있으므로 좌표·화면 밖·다중 디스플레이 문제를 완전히 우회하는 요소 기반 경로를 우선 사용. 스크롤되어 화면 일부만 보이거나 좌표가 살짝 어긋나도 요소 자체를 누르므로 더 견고. 다만 시작/끝 지점 구분(§3.3)은 요소 기반 경로에는 의미가 없다는 점에 유의 — Press 액션은 좌표를 받지 않는다 |
| AX | 단일 클릭, `kAXPressAction` **미지원**(`kAXErrorActionUnsupported`) | `CGEventCreateMouseEvent` + `CGEventPost` (AX frame 의 시작/끝 지점 좌표 사용) | Press 액션이 없는 순수 텍스트 요소 등은 좌표 기반으로 폴백 |
| AX | `clickAndReturn`/`clickReturnClick`/`doubleClickCopy`/`tripleClickCopy` | `CGEventCreateMouseEvent` + `CGEventPost` | `kAXPressAction` 은 의미상 "기본 동작 1회 실행"일 뿐 더블/트리플클릭·클릭-복귀류에 대응하는 AX 액션이 표준화되어 있지 않다. 이 모드들부터는 출처와 무관하게 항상 좌표 기반 경로로 통일 |

> 원칙: **AX 요소 기반 경로(`kAXPressAction`)는 "AX 후보 + 시작/끝 지점 단일 클릭 상당 + Press 지원"이라는 좁은 조건에서만** 쓰고, 그 외 모든 조합(다른 6종 모드, OCR 후보, Press 미지원)은 좌표 기반 `CGEvent` 경로로 통일한다. 두 경로를 넓게 섞으면 클릭 모드·좌표계 처리 로직이 출처마다 갈라져 유지보수 비용이 커진다.

### 3.4 클릭 좌표 계산 — ⭐ 정정: 중심점이 아니라 모드별 기준점

- **점 선택**: ⭐ 기존 명세는 "bounding box/AX frame 의 중심점을 항상 클릭 지점으로 삼는다"고 전제했다. 실측 결과 이는 **`clickStartMatch`/`clickEndMatch` 두 모드에는 맞지 않는다** — 이 두 모드는 이름 그대로 매치 영역의 **시작 지점**(bounding box 좌상단 또는 텍스트 시작 위치) 또는 **끝 지점**(우하단 또는 텍스트 끝 위치)을 클릭 지점으로 삼는다. 시작/끝 지점의 정확한 정의(가로 방향 텍스트 기준 좌/우 가장자리인지, bounding box 모서리인지)는 SuperKey 원문 팝오버에 상세가 없어 `(미확정)` — 텍스트 진행 방향(LTR)의 좌/우 가장자리로 해석하는 것을 구현 시작점으로 제안한다. `onlyMoveCursor`/`clickAndReturn`/`clickReturnClick`/`doubleClickCopy`/`tripleClickCopy` 등 나머지 모드가 시작/끝 중 무엇을 쓰는지, 혹은 별도로 중심점을 쓰는지도 `(미확정)`이다 — 팝오버 원문에 언급이 없다. 구현 시작점으로는 이들 모드도 `clickStartMatch` 와 동일한 시작 지점 규칙을 재사용할 것을 제안하되, §9 로 확인 필요 사항을 승계한다.
- **좌표계 정합**: OCR 은 Vision/`CGImage` 캡처 결과이므로 **픽셀 단위**(Retina 배율 반영)로 나온다. `CGEventCreateMouseEvent`/AX API 는 **포인트 단위** 전역 좌표를 기대한다. 캡처에 사용한 디스플레이의 `backingScaleFactor` 로 나눠 포인트로 환산한 뒤, 해당 디스플레이의 전역 원점 오프셋을 더해 합성한다.
- **다중 디스플레이**: 모든 좌표는 macOS 전역 좌표계(주 디스플레이 좌상단을 기준으로 하는 `CG`/AX 공통 원점)로 정렬한다. `NSScreen.screens` 는 좌하단 원점(Cocoa 좌표계)이므로 이를 그대로 `CGEvent` 좌표에 섞으면 Y 축이 뒤집힌다 — 오버레이(F-03)와 검출(F-02)이 이미 전역 좌표를 넘겨준다는 전제 하에, F-04 는 **좌표계 변환을 다시 수행하지 않고 입력값을 그대로 신뢰**하되, 디스플레이마다 배율이 다를 수 있다는 점만 클릭 지점 계산 시 재확인한다(§5 엣지 케이스). 실제 다중 디스플레이 좌표 변환 규칙은 실기 관찰되지 않았다 — §9 로 승계.

### 3.5 커서 위치 처리 — ⭐ 정정: 전체 공통이 아니라 모드별 정의

- **정정**: 기존 명세는 "클릭 후 항상 원래 위치로 커서를 복구한다"를 F-04 전체의 결정으로 서술했다. 실측(§1.1, §3.3) 결과 커서 복구는 **`Click and return cursor`(`clickAndReturn`) 모드 하나의 정의**이며, `Click, return, click`(`clickReturnClick`) 은 복구 후 재클릭까지 포함한다. 나머지 5종 모드(`onlyMoveCursor`/`clickStartMatch`/`clickEndMatch`/`doubleClickCopy`/`tripleClickCopy`)는 커서를 복구한다는 근거가 팝오버 원문에 없다 — 클릭·이동 후 커서가 그 자리에 남는 것이 기본 동작일 가능성이 있다 `(미확정)`.
- **워프 절차 자체는 유지**: `clickAndReturn`/`clickReturnClick` 모드에서 커서를 복구할 때는 클릭 직전 실제 커서의 전역 좌표를 기억해 두고, 클릭 이벤트 합성이 끝난 뒤 `CGWarpMouseCursorPosition` 으로 그 좌표로 되돌린다. `CGEventPost` 로 내보낸 마우스 이벤트는 HID 이벤트 스트림에 실제로 합류하므로, 이벤트에 실린 위치로 **화면상 커서가 실제로 이동한다** — 클릭 위치와 커서 표시를 분리해서 "안 보이게" 클릭하는 방법은 없다.
- **구현 유의점**: `CGWarpMouseCursorPosition` 직후 실제 물리 마우스가 큐에 남긴 이동 델타와 충돌해 커서가 다시 튈 수 있다. 워프 전후로 `CGAssociateMouseAndMouseCursorPosition(false)` → 워프 → `CGAssociateMouseAndMouseCursorPosition(true)` 로 감싸는 처리가 필요하다. `cursorFreeze` 키(§3.3, 존재만 확인)가 이 억제 처리와 관련 있을 가능성이 있다 `(미확정)`.

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

→ **둘을 병행**한다: 먼저 `NSRunningApplication.activate` 로 앱을 활성화해 키보드/클릭 포커스가 실제로 그 앱으로 넘어가게 하고, 이어서 클릭 대상이 속한 **특정 창**에 `kAXRaiseAction` 을 수행해 같은 앱의 다른 창이 대신 앞에 오는 경우를 방지한다. 대상 윈도우 요소는 클릭 지점에 대해 `AXUIElementCopyElementAtPosition`(시스템 전역 요소 기준)으로 조회한다 — 이 조회는 OCR/AX 두 후보 출처 모두에 동일하게 적용 가능하므로(둘 다 최종적으로는 화면 좌표를 가지므로), 포커스 전환 로직은 후보 출처에 따라 분기하지 않는다. ⭐ `AXUIElementCopyElementAtPosition` 이 실제로 `Focus window before clicking` 구현에 쓰인다는 추정은 이제 **심볼 링크가 실측으로 확정**됐다(실측: 번들 심볼, §6) — 다만 그 용도가 이 절이 서술한 대로인지는 여전히 해석이다 `(미확정 — 용도는 해석)`.

### 3.8 ⭐ 신규 — 합성 클릭이 자기 이벤트 탭으로 되돌아오는 것을 막는 장치

F-07(`key-remapping-engine.md`)이 설치하는 `CGEventTap` 은 키보드 이벤트뿐 아니라, tap 의 이벤트 마스크에 마우스 이벤트가 포함돼 있다면 F-04 가 `CGEventPost` 로 내보낸 **합성 클릭 이벤트도 같은 탭으로 다시 들어올 수 있다.** 이를 무한 루프나 오탐(자신이 만든 클릭을 사용자 클릭으로 오인)으로 처리하지 않으려면 별도의 차단 장치가 필요하다 — 기존 명세에는 이 문제 자체가 없었다.

실행 파일에서 확인된 관련 키·구분이 이 장치의 실체를 보여준다:

- **`humanClickDownUp` vs `programmaticClickDownUp`**: 실제 사용자가 누른 클릭과 SuperKey 가 합성한 클릭을 **별도 상태로 구분**해서 추적한다. `programmaticClickDown` / `onProgrammaticClickDown` / `onProgrammaticClickDownUp` 도 같은 구분선 위에 있다 — SuperKey 내부적으로 "이 mouseDown/mouseUp 이벤트는 내가 만들었다"는 표식을 어딘가(이벤트 필드의 사용자 정의 값, 또는 별도 상태 플래그)에 남긴다는 뜻으로 읽힌다.
- **`blockClicksUntil`**: 특정 시각까지 들어오는 클릭 이벤트를 걸러낸다 — 합성 클릭이 이벤트 스트림에 합류한 직후의 짧은 창(window) 동안, 그 결과로 되돌아오는 이벤트를 자기 자신이 만든 것으로 판정해 무시하는 데 쓰이는 것으로 보인다.
- **`stopNextMouseDown` / `stopNextMouseUp`**: 다음에 들어오는 mouseDown/mouseUp 이벤트 하나를 명시적으로 소비(탭에서 흡수)하도록 예약하는 플래그로 보인다 — 시간 기반(`blockClicksUntil`)이 아니라 **이벤트 카운트 기반**의 차단 방식이다.

⭐ **이 구분이 필수인 이유**: F-07 이 전역 `CGEventTap` 으로 마우스 이벤트까지 관찰한다면, `CGEventPost` 로 주입한 합성 클릭이 그 탭에 다시 도달해 SuperKey 자신의 다른 로직(예: "클릭이 발생하면 무언가를 한다"는 별개 기능)을 오작동시키거나, 최악의 경우 클릭→탭 감지→재처리→재클릭의 루프를 만들 수 있다. `humanClickDownUp`/`programmaticClickDownUp` 의 이원화와 `blockClicksUntil`/`stopNextMouseDown`/`stopNextMouseUp` 의 소비 예약이 이를 막는 설계 요소로 보인다. 클론 구현도 동일한 문제를 가지므로(F-07 이 마우스 이벤트를 관찰하는 범위에 따라), 합성 클릭을 발행하기 직전 "이 좌표·이 시각의 클릭은 내가 만든 것"이라는 표식을 남기고, F-07 의 이벤트 탭 콜백에서 그 표식을 검사해 되돌아온 합성 이벤트를 통과시키는 장치를 두어야 한다. 정확한 구현 방식(시간 창 vs 카운트 기반, 표식을 이벤트의 어느 필드에 남기는지)은 실측되지 않았다 — §9 로 승계.

---

## 4. 설정 항목

| 이름 (원문) | 타입 | 기본값 | 저장 키 | 유효범위 | 출처 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `Focus window before clicking` | 체크박스(불리언) | **☐** (실측: AX 트리 · defaults) | `frontmostFirstCheckbox` 대응 | ON / OFF | `app-bundle-analysis.md` §2.1(defaults), §6.1(AX 실측) |
| `Change click modes with modifier keys` | 체크박스(불리언), ⓘ 툴팁 보유 | **☑** (실측: AX 트리 — 스크린샷과 일치) | (`seekOptions` 비트) | ON / OFF | 동일. 부제 원문: "If this setting is disabled, modifiers will be applied to the click" |

⭐ **기본값 오기 정정**: `Focus window before clicking` 이 ☑ 로 서술돼 있었다면 그것은 홍보 스크린샷 값이다. 아무 설정도 바꾸지 않은 상태의 `defaults` 전량에 이 항목에 대응하는 키가 없고, AX 트리 실측도 ☐ 다 — 부재가 기본값이므로 **출고 기본값은 ☐(꺼짐)** 이다. `Change click modes with modifier keys` 는 기존 서술(☑)과 실측이 **일치**한다 — ⭐ **Seek 탭 전체에서 유일하게 출고 기본값이 켜진 항목**이라는 점을 명시한다(§3.2). (`superkey-inventory.md` §7 Q3 해소.)

두 설정 모두 **Seek 탭**에 위치하며, 팝업이나 슬라이더 같은 부가 파라미터는 없는 단순 불리언이다(조사 문서에서 추가 세부 컨트롤이 확인되지 않음). **클릭 모드 7종(§1.1, §3.3)을 고르는 UI(`Record Modifiers`)는 이 두 체크박스와 별개이며, 4개 탭 어디에서도 관찰되지 않았다** `(미확정)` — §9 참조.

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
| 10 | 확정 시점에 modifier 가 아직 눌려 있음(hold 모드) | hold 모드 확정은 리매핑 키를 떼는 순간(F-01)이며, 클릭 모드 판정에 쓰이는 modifier 는 **그 순간의 스냅샷**(§3.1 1단계)으로 고정한다. 리매핑 키 자신이 modifier 후보이기도 한 경우(예: Caps Lock 이 Seek 트리거이면서 동시에 클릭 모드 전환 modifier 로 쓰이길 기대하는 상황)는 F-07 의 이벤트 탭이 그 키 자체를 이미 소비하므로 애초에 modifier 로 관측될 수 없다 — 이 조합은 구조적으로 성립하지 않음을 문서화만 해 둔다 |
| 11 | Secure Input 활성(암호 필드에 포커스) | 조사 문서에 따르면 Secure Input 은 키 리매핑 전면을 무력화하는 macOS 제약이다. Seek 세션 자체가 F-01/F-07 단계에서 이미 성립하지 않을 가능성이 높지만, 만에 하나 세션이 성립해 F-04 까지 도달했다면 클릭 합성 자체(마우스 이벤트)는 Secure Input 의 영향을 받지 않으므로 정상 수행한다 — 단, 클릭 결과로 열리는 것이 암호 필드라면 그 앱 안에서의 이후 키 입력은 별개 문제(F-07 범위) |
| 12 | 클릭 후 원래 커서 위치 복구 실패 | `CGWarpMouseCursorPosition` 호출 자체가 실패하는 사례는 조사에서 확인되지 않았으나, 반환값 확인 없이 방치하면 커서가 클릭 지점에 남는다. 복구 실패를 별도로 사용자에게 알리지 않고, 커서가 클릭 지점에 남는 것을 안전한 열화(fallback) 상태로 받아들인다 |
| 13 | 다중 디스플레이 간 배율(Retina/비-Retina) 상이 | 클릭 지점이 속한 디스플레이의 `backingScaleFactor` 를 그 디스플레이 기준으로 개별 조회해야 한다 — 캡처 시점 디스플레이와 클릭 시점 디스플레이가 다르면(§케이스 2, 7) 잘못된 배율을 적용해 좌표가 틀어질 수 있음 |
| 14 | Accessibility 권한이 클릭 실행 도중 회수됨 | `AXUIElementPerformAction`/`CGEventPost` 가 조용히 실패(권한 없이는 애초에 `CGEventTap` 도 F-07 에서 죽어 있을 가능성이 높음). 권한 상태 감지·복구는 F-11 범위, 여기서는 "Accessibility 권한이 전제"라는 의존 사실만 남긴다 |

---

## 6. 필요한 플랫폼 API

⭐ **아래 API 목록은 이제 대부분 실측으로 확정됐다** (실측: 번들 심볼, `app-bundle-analysis.md` §3.1). 클론이 Rust 로 호출할 크레이트/버전은 이 문서의 설계 결정이며, API 자체(함수 이름)는 원본 SuperKey 실행 파일의 링크 심볼로 확인된 것이다.

| API | 용도 | 근거 | 크레이트 |
| :--- | :--- | :--- | :--- |
| `CGEventCreateMouseEvent` | 좌표 기반 클릭(모든 클릭 모드) 이벤트 생성 | 실측: 번들 심볼 | `core-graphics` 0.25.0 또는 `objc2-core-graphics` 0.3.2 |
| `CGEventPost` | 생성한 마우스 이벤트를 HID 스트림에 주입 | 실측: 번들 심볼 | 동일 |
| `CGEventSetFlags` | OFF 상태일 때 modifier 를 클릭 이벤트에 얹기 | 실측: 번들 심볼 | 동일 |
| `CGEventSetIntegerValueField` | 클릭 카운트(더블/트리플클릭) 설정용으로 보임 `(미확정 — 용도는 해석)` | 실측: 번들 심볼 | 동일 |
| `CGEventGetLocation` | 클릭 이벤트/현재 커서의 좌표 조회 | 실측: 번들 심볼 | 동일 |
| `CGEventSourceCreate` | 합성 이벤트의 이벤트 소스 생성 | 실측: 번들 심볼 | 동일 |
| `kCGEventRightMouseDown/Up`, `kCGEventOtherMouseDown/Up` | 우클릭/가운데클릭 이벤트 타입(§3.3 의 7종 모드에는 직접 대응하지 않지만, `middleClick`/`rightClick` 키(§3.3)가 시사하는 별도 용도에 필요할 수 있음) | 명세 설계값(원본 대응 여부 `(미확정)`) | 동일 |
| `AXUIElementPerformAction` (`kAXPressAction`) | AX 후보 + 좁은 조건(§3.3.1)의 요소 기반 클릭 | 실측: 번들 심볼 | `axuielement` 0.9.1 |
| `AXUIElementSetAttributeValue` | AX 요소 속성 설정(포커스·값 변경 등 — 정확한 사용처는 `(미확정)`) | 실측: 번들 심볼 | `axuielement` 0.9.1 |
| `AXUIElementCopyActionNames` | `kAXPressAction` 지원 여부 사전 확인 | 명세 설계값 | `axuielement` 0.9.1 |
| `AXUIElementCopyElementAtPosition` | ⭐ 클릭 대상 좌표의 AX 요소를 역조회 — `Focus window before clicking` 구현에 쓰이는 것으로 보인다 `(미확정 — 용도는 해석)` | 실측: 번들 심볼 | `axuielement` 0.9.1 또는 `accessibility-sys` 0.2.0 |
| `AXUIElementPerformAction` (`kAXRaiseAction`) | 대상 특정 윈도우 올리기 | 실측: 번들 심볼(액션 존재는 확인, `kAXRaiseAction` 특정은 `(미확정)`) | `axuielement` 0.9.1 |
| `AXUIElementCreateApplication` | pid 로부터 앱의 AX 최상위 요소 획득 | 실측: 번들 심볼 | `axuielement` 0.9.1 |
| `AXUIElementGetPid` | AX 요소로부터 소속 프로세스 pid 조회 | 실측: 번들 심볼 | `axuielement` 0.9.1 |
| `AXUIElementSetMessagingTimeout` | AX 호출이 응답 없는 대상 앱에 블로킹되는 것 방지 | 명세 설계값 | `axuielement` 0.9.1 |
| `NSRunningApplication.activate(options:)` / `runningApplications` | 대상 앱 활성화(키 포커스 이전), 실행 중 앱 열거 | 실측: 번들 심볼(`NSWorkspace` 계열) | `objc2-app-kit` 0.3.2 |
| `NSWorkspace` / `NSWorkspaceOpenConfiguration` / `URLForApplicationWithBundleIdentifier:` | 앱 활성화·조회에 쓰이는 `NSWorkspace` API 군 — 정확히 어느 호출이 F-04 경로에 쓰이는지는 `(미확정)` | 실측: 번들 심볼 | `objc2-app-kit` 0.3.2 |
| `CGWarpMouseCursorPosition` | 클릭 후 커서 원위치 복구(`clickAndReturn`/`clickReturnClick` 모드, §3.5) | 명세 설계값 | `core-graphics` 0.25.0 또는 `objc2-core-graphics` 0.3.2 |
| `CGAssociateMouseAndMouseCursorPosition` | 워프 직후 물리 마우스 이동과의 충돌 방지 | 명세 설계값 | 동일 |
| `NSScreen.screens` | 다중 디스플레이 union frame, 좌표 범위 검증 | 명세 설계값 | `objc2-app-kit` 0.3.2 |
| `AXIsProcessTrusted` (전제 확인만, 요청·온보딩은 F-11) | 클릭 합성 가능 여부의 선행 조건 | 실측: 번들 심볼(`app-bundle-analysis.md` §3.3) | `accessibility-sys` 0.2.0 |

---

## 7. 구현 접근

**판정: Rust 바인딩** — `objc2` 계열과 `axuielement` 의 `unsafe` FFI 로 전부 호출 가능하며, 별도로 빌드하는 Swift/Objective-C shim 은 불필요하다.

**근거**: §6 의 모든 API가 `objc2-core-graphics`(0.3.2), `axuielement`(0.9.1, 현재 가장 활발히 유지되는 AX 고수준 크레이트), `objc2-app-kit`(0.3.2), 또는 `core-graphics`(0.25.0, `CGEventTap` 안전 래퍼와 동일 생태계) 로 이미 노출되어 있다(`docs/research/rust-macos-capability-notes.md` §1.1, §1.2, §2.1, §2.2). F-03(오버레이)에서 이미 확인된 패턴과 동일하게 — `WebviewWindow::ns_window()` 로 얻은 포인터를 캐스팅해 AppKit 전체에 접근할 수 있었던 것처럼 — 클릭 실행도 네이티브 shim 없이 Rust 프로세스 안에서 전부 해결된다. 다만 `AXUIElement*` 원시 호출과 `CGEvent*` 필드 조작은 안전한 상위 래퍼가 없는 `unsafe` FFI 수준이므로 ①(크레이트만으로 충분)이 아니라 ②로 판정한다.

**채택 크레이트/버전**: `core-graphics` 0.25.0, `axuielement` 0.9.1, `objc2-app-kit` 0.3.2, `objc2-core-graphics` 0.3.2, `accessibility-sys` 0.2.0(보완용 원시 FFI, `axuielement` 가 커버하지 못하는 개별 상수/함수 필요 시).

**기각한 대안**:
- `rdev` 0.5.3 — 전역 키/마우스 이벤트 크레이트지만 **3년간 릴리스 없음**(조사 §2.1), 이벤트 소비·필드 제어가 세밀하지 않아 클릭 모드별 필드 조작(`kCGMouseEventClickState` 등)에 부적합. 신규 의존으로 채택하지 않는다.
- 전용 네이티브(Swift/ObjC) shim 작성 — F-03 오버레이 사례에서 이미 "shim 없이 해결 가능"이 입증된 패턴을 클릭 실행에도 그대로 적용할 수 있어 불필요한 복잡도로 판단, 기각.
- `AXUIElementPerformAction(kAXPressAction)` 을 모든 클릭 모드에 일괄 적용 — §3.3.1 에서 서술한 대로 `onlyMoveCursor`/`clickAndReturn`/`clickReturnClick`/`doubleClickCopy`/`tripleClickCopy`/OCR 후보를 표현할 수 없어 기각. 좁은 조건(AX+시작/끝 지점 단일 클릭+Press지원)에서만 사용.

---

## 8. 수용 기준

- [ ] `Focus window before clicking` OFF, `Change click modes with modifier keys` OFF 상태(둘 다 켠 채 테스트하려면 후자를 의도적으로 끔 — 출고 기본값은 후자가 ☑ 이므로 §4 확인)에서, AX 후보의 기본 클릭이 `kAXPressAction` 으로 합성된다.
- [ ] 같은 조건에서 OCR 후보는 항상 `CGEventCreateMouseEvent`+`CGEventPost` 로 합성된다(AX 경로를 타지 않는다).
- [ ] 출고 기본값(`Change click modes with modifier keys` ☑)에서 확정 시점에 아무 modifier 도 눌려 있지 않으면, 7종 중 SuperKey 가 "기본 클릭"으로 취급하는 모드가 실행된다(어느 모드가 무-modifier 기본인지는 §9 Q10 확인 후 구체화).
- [ ] `Change click modes with modifier keys` ON 이고 확정 시점 특정 modifier 가 눌려 있으면, 매치 위치에 무-modifier 시와 **다른 클릭 모드**(7종 중 하나, §3.3)가 실행된다 — 정확한 modifier↔모드 매핑은 §9 Q10 확정 전까지는 "다른 모드로 전환된다"는 동작 자체만 검증한다.
- [ ] `Change click modes with modifier keys` 를 사용자가 명시적으로 끈 상태에서 ⌘ 을 누른 채 확정하면, 클릭 모드는 항상 기본 동작이고 ⌘ 플래그가 클릭 이벤트에 실려 있다.
- [ ] `Just move cursor` 모드에서는 클릭 이벤트가 전혀 발생하지 않고 커서만 매치 위치로 이동한다.
- [ ] `Click at beginning of match` / `Click at end of match` 모드는 매치 영역의 중심이 아니라 시작/끝 지점을 클릭한다(§3.4).
- [ ] `Click and return cursor` 모드는 클릭 후 커서가 원래 위치로 복귀하고, `Click, return, click` 모드는 복귀 후 같은 지점에서 다시 클릭한다.
- [ ] `Double click and copy` / `Triple click and copy` 모드는 각각 더블/트리플클릭을 합성해 단어/줄을 선택하고 클립보드에 복사한다.
- [ ] `Focus window before clicking` ON 이고 대상이 비활성 창일 때, 클릭 1회(확정 1회)로 창이 활성화되고 클릭까지 도달한다.
- [ ] `Focus window before clicking` OFF 일 때는 SuperKey 가 별도의 활성화 API 를 호출하지 않는다(활성화 여부는 순수히 macOS/대상 앱의 기본 동작에 위임).
- [ ] 오버레이는 포커스 전환·클릭 좌표 계산보다 항상 먼저 해제된다(§3.6 순서를 어기지 않는다).
- [ ] 클릭 전 대상 창이 닫히면(§5 #1) 클릭이 조용히 취소되고 크래시하지 않는다.
- [ ] 클릭 지점이 현재 디스플레이 구성의 화면 밖으로 계산되면(§5 #7) 클릭을 실행하지 않는다.
- [ ] `clickAndReturn`/`clickReturnClick` 모드에서 클릭 완료 후 커서가 클릭 직전의 원래 좌표로 복귀한다(정상 경로에서).
- [ ] hold 모드 확정 시 클릭 모드 판정에 쓰이는 modifier 스냅샷은 리매핑 키를 떼는 그 순간의 상태를 반영한다(§5 #10).
- [ ] AX 후보에서 `kAXPressAction` 이 `kAXErrorActionUnsupported` 로 실패하면 좌표 기반 경로로 폴백해 클릭이 완료된다.
- [ ] F-04 가 합성한 클릭이 F-07 의 `CGEventTap` 콜백에 되돌아왔을 때, SuperKey 자신의 클릭으로 식별되어 이중 처리·루프가 발생하지 않는다(§3.8).

---

## 9. 미해결 질문

### 해소된 질문 (참고용 기록)

이번 실측으로 다음이 확정되어 더 이상 열린 질문이 아니다: 클릭 모드의 존재와 7종 목록(§1.1) · 클릭 지점이 중심이 아니라 시작/끝 지점이라는 것(§3.4) · `Change click modes with modifier keys` 의 이중 의미(모드 선택 vs modifier 그대로 전달, §3.2) · 두 체크박스의 출고 기본값(§4) · 클릭 합성 API 목록(§6).

### 열린 질문

| # | 질문 | 상태 |
| :--- | :--- | :--- |
| Q10 | ⭐ `Change click modes with modifier keys` 의 modifier ↔ 클릭 모드(7종) 매핑 | **미확정.** ⓘ 팝오버 원문(§1.1)은 "Hold the corresponding modifier keys when pressing enter"라고만 말할 뿐 어떤 modifier 가 어느 모드인지 명시하지 않는다. nib 정적 판독·AX 트리 어디에도 매핑표가 없다. **추측으로 매핑표를 만들지 않는다** — 실행 파일에 `Record Modifiers` 문자열과 `KeyboardShortcuts.RecorderModifierCocoa` 타입이 있어 **사용자가 모드별 modifier 를 직접 녹화**하는 구조로 보이나, 그 UI 는 4개 탭 어디에서도 관찰되지 않았다. 구현 전 반드시 확인 필요 |
| Q10-a | `Record Modifiers` UI 가 실제로 어디에 있는지(어느 탭·어느 진입점) | 관찰되지 않음. 조건부 표시(예: `Change click modes with modifier keys` ⓘ 팝오버 내부, 또는 별도 서브패널)일 가능성 `(미확정)` |
| Q10-b | 7종 모드 중 **무-modifier 기본값**이 무엇인지(예: `clickStartMatch` 가 기본이고 나머지가 modifier 로 전환되는 구조인지) | 팝오버 원문에 언급 없음. `justClick`/`clickNumber` 등 관련 키(§3.3)의 정확한 의미도 미확정 |
| Q10-c | 위 매핑에서 **복수 modifier 조합**(예: ⌃⇧ 동시)이 눌렸을 때의 동작 — 우선순위 규칙이 있는지, 혹은 정의되지 않은 조합은 무시하는지 | 미확정 |
| Q10-d | 두 가지 다른 표기(팝오버 "Just move cursor" 류 vs 실행 파일 문자열 "Click and Return" 류, §1.1)가 같은 모드의 다른 UI 노출(팝오버 vs 다른 화면)인지, 아니면 실제로 다른 개념인지 | `(미확정)` — 표기 차이만 확인, 의미 차이 여부는 확인 못 함 |
| Q11 | `EntrySearchButton`/`ClickablePlaceholderView`(F-03 §1.1)와 F-04 의 클릭 실행 로직이 공유되는 부분이 있는지(검색 바 안의 클릭 가능 요소도 같은 클릭 합성 경로를 쓰는지) | F-03 범위와 겹침, 확인 안 됨 |
| Q12 | `clickStartMatch`/`clickEndMatch` 의 "시작/끝" 이 텍스트 진행 방향 기준인지 bounding box 모서리 기준인지(§3.4) | 팝오버 원문에 상세 없음. 구현 시작점만 제안, 확정 아님 |
| Q13 | `onlyMoveCursor`/`doubleClickCopy`/`tripleClickCopy` 모드가 클릭 지점을 시작/끝/중심 중 무엇으로 삼는지(§3.4) | 팝오버 원문에 언급 없음 |
| Q14 | `blockClicksUntil`/`stopNextMouseDown`/`stopNextMouseUp`(§3.8)이 시간 창 기반인지 이벤트 카운트 기반인지, 정확한 임계값·구현 방식 | 존재만 확인, 구현 상세는 실측 안 됨 |
| Q16 | OCR 캡처 픽셀 좌표 → `CGEvent` 포인트 좌표 변환 시, 캡처 시점과 클릭 시점 사이 디스플레이 구성이 바뀌지 않았다는 전제가 실제로 F-02/F-03 파이프라인에서 얼마나 촘촘히 보장되는지 | 실측 필요(§5 #2, #7, #13 과 연결) |
| Q16-a | 다중 디스플레이에서의 실제 클릭 좌표 변환 규칙(배율이 다른 디스플레이 간 전환 등) | 실기 관찰 못 함 — F-03 §3.3 의 창 구조 논의와 연결 |
| Q17 | 클릭이 드래그로 오인되지 않기 위한 mouseDown-mouseUp 간격의 구체적 임계값(ms) | SuperKey 실측값 없음. 지어내지 않음 — 프로토타입 단계에서 실측 필요 |
| Q18 | `Focus window before clicking` 의 "활성화 완료" 판정 방법 — 고정 딜레이 / `kAXFocusedWindowChangedNotification` 구독 / 폴링 중 무엇을 쓸지 | 조사 문서에 근거 없음. §5 #9 에서 후보만 나열, 결정은 보류 |
| Q19 | `kAXPressAction` 지원 여부를 `AXUIElementCopyActionNames` 로 사전 질의하는 것이 모든 대상 앱에서 신뢰할 수 있는지(일부 앱이 액션 이름은 보고하되 실제 수행은 실패하는 사례가 있는지) | 실측 필요 |
| Q20 | 다중 modifier 스냅샷을 "누른 시점"이 아니라 "확정 시점"으로 고정하는 것이 사용자 기대와 맞는지(예: Control 을 먼저 누르고 이후 확정하는 동안 손을 뗀 경우) | UX 검증 필요, 본 문서는 확정 시점 스냅샷을 설계 결정으로 채택했으나 재검토 여지 있음 |
