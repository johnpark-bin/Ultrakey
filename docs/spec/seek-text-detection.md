# F-02 · Seek — 텍스트 후보 검출

> **한 줄 요약**: 화면 캡처에 대한 Vision OCR과, 활성 앱의 Accessibility 트리 파싱이라는 두 개의 독립 소스에서 "클릭 가능한 텍스트 후보" 목록을 만들고 하나로 병합한다.
> **의존성**: 후보 검출을 *언제* 시작하는지는 `F-01`(`seek-activation-and-session.md`)이 결정한다. 이 문서는 세션이 열려 질의 문자열이 주어진 상태에서 "화면 텍스트 후보 목록 + 질의 매칭 결과"를 산출하는 것까지만 다룬다. 소스 A(OCR)는 Screen Recording 권한에, 소스 B(AX)는 Accessibility 권한에 의존한다 — 권한 획득 절차 자체는 `F-11`(`permissions-onboarding.md`) 참조.
> **관련 명세**: 후보를 화면에 그리는 방법 → `F-03`(`seek-overlay-ui.md`). 선택된 후보를 클릭하는 방법 → `F-04`(`seek-click-execution.md`). 권한 획득 UX → `F-11`(`permissions-onboarding.md`). 플랫폼 역량 종합 판정 → `docs/spec/platform-constraints.md`(별도 문서, 이 문서 범위 밖).

---

## 1. 개요

Seek 는 사용자가 타이핑한 질의 문자열과 일치하는 텍스트를 화면에서 찾아 그 위치를 클릭 후보로 제시하는 기능이다. 이 문서가 다루는 범위는 "질의 문자열 + 현재 화면 상태" 를 입력으로 받아 "클릭 가능한 후보 목록(텍스트 + 화면 좌표)" 을 출력하는 검출·병합 파이프라인 하나뿐이다.

검출은 두 개의 독립 소스를 병행한다.

- **소스 A (주 경로) — Vision OCR**: 화면을 스크린샷으로 캡처해 Apple Vision 프레임워크로 문자를 인식한다. 항상 켜져 있으며 끌 수 없다 `(추정 — 조사 자료에 "OCR 끄기" 옵션이 없다. §9 참조)`.
- **소스 B (보조 경로) — Accessibility 파싱**: `Seek using macOS accessibility` 체크박스로 켜는 선택 기능이다. 대상 앱이 `AXUIElement` 트리로 노출하는 텍스트 요소를 직접 읽는다. v1.18 에서 "initial cut" 으로 추가됐으며 OCR 이 놓치는 후보를 보강하는 역할이다.

두 소스의 산출물은 **동일한 자료형(텍스트 + 전역 화면 좌표)** 으로 정규화된 뒤 병합되어 하나의 후보 목록이 된다.

---

## 2. 사용자 시나리오

1. 사용자가 Seek 세션을 연다(`F-01`). 이 시점에 화면 캡처와(설정이 켜져 있다면) AX 순회가 트리거된다.
2. 사용자가 질의 문자열을 한 글자씩 입력한다. 매 입력마다 이미 검출된 후보 목록에 대해 매칭 규칙을 재적용해 일치하는 후보를 좁혀 나간다 `(추정 — 원본은 Spotlight 유사 검색 바이므로 증분 필터링이 자연스럽다. 재검출 대 재필터링의 구분은 §5, §9 참조)`.
3. 후보가 여러 개면 정렬된 목록 중 첫 번째가 기본 선택되고, 사용자는 `F-01`/`F-03` 이 정의하는 방식으로 다음 후보로 이동한다.
4. 사용자가 확정하면 선택된 후보의 화면 좌표가 `F-04` 로 전달되어 클릭이 실행된다.
5. 화면 녹화 권한이 없거나, OCR 이 아무것도 찾지 못하거나, AX 가 응답하지 않는 등 실패 시에도 세션 자체는 유지되며 후보 0개인 상태로 사용자에게 보여진다(엔드투엔드 UX 는 `F-01`/`F-03`).

---

## 3. 동작 명세

### 3.1 파이프라인 개요

```
                         ┌─────────────────────────┐
                         │  세션 열림 (F-01 트리거)  │
                         └─────────────┬────────────┘
                     ┌─────────────────┴─────────────────┐
                     ▼                                     ▼
        [소스 A: Vision OCR]                    [소스 B: Accessibility] (설정 켜짐 시)
       A1 화면 캡처                                B1 AX 루트 획득
       A2 VNRecognizeTextRequest 실행              B2 트리 순회 + 텍스트 요소 수집
       A3 좌표 변환 (정규화·좌하단→전역·좌상단)      B3 범위 필터 적용
       A4 TextCandidate 목록 A                     B4 TextCandidate 목록 B
                     └─────────────────┬─────────────────┘
                                        ▼
                              M. 병합 (M1~M3)
                                        ▼
                         전역 후보 목록 CandidateSet
                                        ▼
                         질의 문자열 매칭 → 정렬된 부분집합
                                        ▼
                         F-03(오버레이) / F-04(클릭) 로 전달
```

### 3.2 소스 A — Vision OCR

| 단계 | 입력 | 처리 | 출력 |
| :--- | :--- | :--- | :--- |
| A1. 화면 캡처 | (트리거 신호) | 디스플레이별로 `CGImage` 캡처 | `[(displayID, CGImage, imgW, imgH, backingScaleFactor)]` |
| A2. OCR 실행 | 디스플레이별 `CGImage` | `VNImageRequestHandler(cgImage:)` 생성 → `VNRecognizeTextRequest` 실행 | `[VNRecognizedTextObservation]` (디스플레이별) |
| A3. 좌표 변환 | `VNRecognizedTextObservation.boundingBox`(정규화, 좌하단 원점) | §3.2.2 수식 적용 | 전역 화면 좌표 `CGRect`(포인트 단위, 좌상단 원점) |
| A4. 후보 생성 | 변환된 좌표 + `topCandidates(1)` 문자열 | `TextCandidate { text, frame, source: OCR, confidence }` 구성 | `[TextCandidate]` (목록 A) |

#### 3.2.1 인식 파라미터와 트레이드오프

| 파라미터 | 선택 | 근거·트레이드오프 |
| :--- | :--- | :--- |
| `recognitionLevel` | `.accurate` | `.fast` 는 지연이 낮지만 작은 텍스트·저대비 텍스트에서 인식률이 더 떨어진다. 조사에서 확인된 실패 사례(극소 텍스트, 저대비 색상)를 줄이려면 `.accurate` 가 필요하다. 대가는 지연 — 큰 화면·다중 디스플레이에서 수백 ms 단위로 늘어날 수 있다(§5 참조, `platform-constraints.md` P3). |
| `usesLanguageCorrection` | `false` `(추정)` | Seek 가 찾는 대상은 UI 라벨·버튼 텍스트 등 자연어 문장이 아닌 경우가 많다. 언어 모델 기반 보정은 "OK", "확인" 같은 짧은 UI 텍스트를 실제와 다른 단어로 교정할 위험이 있다. 근거 자료 없음 — §9 미해결 질문으로 승계. |
| `recognitionLanguages` | 시스템 로케일 + 영어를 기본으로, 사용자가 늘릴 수 있게 `(추정)` | 조사에서 앱이 en 외 7개 로케일(de, he, ar, el, ja, fa, uk)을 번들함이 appcast 에서 확인됐다(`superkey-inventory.md` §2.3). UI 로케일과 OCR 인식 언어가 반드시 같을 필요는 없으므로 별도 설정일 가능성이 있으나 조사 자료로 확정되지 않는다. |
| `minimumTextHeight` | 명시적으로 낮게 설정하거나 미설정(기본값) `(추정)` | "extra small text" 가 확인된 실패 사례이므로, 이 파라미터를 부주의하게 높게 두면 그 실패를 자초하게 된다. 구체적 수치는 조사 자료에 없다. |

#### 3.2.2 좌표 변환 (수식)

`VNRecognizedTextObservation.boundingBox` 는 **해당 디스플레이의 캡처 이미지 기준으로 정규화된 좌표**이며 **원점이 좌하단**이다. 이를 AX 좌표(§3.3)와 같은 좌표계, 즉 **전역 화면 좌표(포인트 단위, 원점 좌상단, Quartz/CoreGraphics 관례)** 로 변환해야 두 소스를 병합할 수 있다.

주어진 값:
- `(bx, by, bw, bh)` — `boundingBox`, 각 값 `[0, 1]`, 원점 좌하단
- `(imgW, imgH)` — 해당 디스플레이 캡처 이미지의 **픽셀** 크기
- `scale` — 해당 디스플레이의 `backingScaleFactor` (Retina 는 보통 2.0)
- `(originX, originY)` — 해당 디스플레이의 전역 원점(`CGDisplayBounds(displayID).origin`, 이미 좌상단 원점 전역 좌표계)

변환:

```
# 1) 좌하단 원점 → 좌상단 원점 (정규화 좌표 내에서 y만 뒤집는다)
ty = 1 - by - bh
tx = bx
tw = bw
th = bh

# 2) 정규화 좌표 → 이 디스플레이 이미지의 픽셀 좌표
px = tx * imgW
py = ty * imgH
pw = tw * imgW
ph = th * imgH

# 3) 픽셀 → 포인트 (Retina 배율 보정)
qx = px / scale
qy = py / scale
qw = pw / scale
qh = ph / scale

# 4) 디스플레이 로컬 포인트 → 전역 포인트 (다중 디스플레이 매핑)
gx = originX + qx
gy = originY + qy
gw = qw
gh = qh
```

결과 `CGRect(gx, gy, gw, gh)` 가 `TextCandidate.frame` 이 되며, 이는 `kAXPositionAttribute`/`kAXSizeAttribute` 가 반환하는 좌표계와 동일해야 병합이 가능하다(§3.4).

> ⚠️ 4단계가 v1.55 에서 "다중 디스플레이에서 Seek 동작이 깨짐" 으로 실제 버그가 난 지점이다(`superkey-inventory.md` §2.1). 디스플레이별로 별도 캡처·별도 OCR 요청을 수행하고, 이 4단계 오프셋 보정을 반드시 디스플레이마다 독립적으로 적용해야 한다. 개발자 포스트가 언급한 "다른 디스플레이의 매치로 선을 못 그리던" 결함도 같은 원인 계열이다(§4.5).

### 3.3 소스 B — Accessibility 파싱

| 단계 | 입력 | 처리 | 출력 |
| :--- | :--- | :--- | :--- |
| B1. 루트 획득 | (설정: `Only Seek in the frontmost window`) | 꺼짐 → `AXUIElementCreateSystemWide()` 또는 화면상 보이는 모든 앱의 `AXUIElementCreateApplication(pid)` 순회. 켜짐 → 최전면 앱의 `kAXFocusedWindowAttribute` 로 얻은 창 하나만 루트로 사용 | AX 루트 요소 1개 이상 |
| B2. 트리 순회 | AX 루트 | 깊이 제한을 두고 `kAXChildrenAttribute` 로 재귀 순회. 각 요소에서 `kAXValueAttribute`/`kAXTitleAttribute`/`kAXDescriptionAttribute` 중 텍스트가 있는 것과, `kAXPositionAttribute`/`kAXSizeAttribute`(둘 다 `AXValue` → `CGPoint`/`CGSize` 로 언패킹) 를 읽는다 | `[(text, frame)]` 원시 목록 |
| B3. 범위 필터 | 원시 목록 + (설정: `Match on more than one character`) | 켜짐 → `text.count >= 2` 인 요소만 통과 | 필터링된 목록 |
| B4. 후보 생성 | 필터링된 목록 | `TextCandidate { text, frame, source: AX, confidence: None }` 구성 | `[TextCandidate]` (목록 B) |

`kAXPositionAttribute`/`kAXSizeAttribute` 는 이미 **전역 화면 좌표, 좌상단 원점, 포인트 단위**로 반환되므로(CoreGraphics 관례), 소스 A 와 달리 추가 좌표 변환이 필요 없다.

#### 3.3.1 `Match on more than one character` 의 의미 `(추정)`

이 체크박스는 AX 매칭을 2글자 이상인 요소로 제한한다. 이유는 조사 자료에 직접 서술되어 있지 않으나 다음 근거로 추정한다: OCR 은 화면에 보이는 "덩어리 텍스트"만 관측하므로 1글자 후보가 드물게만 나오는 반면, AX 트리는 버튼 아이콘의 accessibility label, 구분자, 단축키 힌트 등 **화면에 시각적으로 드러나지 않는 1글자 텍스트 요소**까지 그대로 노출한다. 이런 요소를 그대로 후보에 포함시키면 사용자가 한 글자만 입력해도 후보가 폭증해 목록이 사실상 무의미해진다. 그래서 AX 소스에만 적용되는 노이즈 억제 스위치로 별도 존재한다고 본다. OCR 소스에는 이런 옵션이 없다 — OCR 은 애초에 시각적으로 존재하는 텍스트만 잡으므로 1글자 노이즈 문제가 AX 만큼 심하지 않기 때문으로 추정한다.

#### 3.3.2 `Only Seek in the frontmost window` 의 순회 범위 축소

이 설정은 스크린샷상 `Seek using macOS accessibility` 의 하위 체크박스가 아니라 **독립된 상위 체크박스**로 배치되어 있다(`superkey-inventory.md` §3.1). 따라서 이 설정은 소스 B(AX 순회 루트)뿐 아니라 소스 A(화면 캡처 범위)에도 함께 적용되는 것이 자연스럽다 `(추정)`: 꺼짐 상태에서는 전체 화면(모든 디스플레이)을 캡처·순회하고, 켜짐 상태에서는 최전면 창의 프레임으로 캡처 영역을 잘라내고(A1) AX 순회 루트도 그 창으로 제한한다(B1). 조사 자료는 이 설정이 "Seek" 전체에 적용된다고만 말할 뿐 소스별로 분리해 서술하지 않으므로, 두 소스에 동일하게 적용하는 쪽을 채택하고 §9 에 확인 필요 항목으로 남긴다.

#### 3.3.3 성능·블로킹 위험

AX 트리 순회는 **프로세스 간 동기 IPC** 다. 대상 앱이 응답하지 않으면 순회 호출이 블로킹되어 Seek 전체가 멈춘다. 이는 v1.19 "potential crash with Seek using the Accessibility API" 로 실제 발생한 이력이 있다(`superkey-inventory.md` §2.2). 대응책:

- **메시징 타임아웃**: 각 대상 프로세스의 `AXUIElementSetMessagingTimeout` 을 짧게(수백 ms 단위) 설정해 응답 없는 앱이 전체 파이프라인을 무기한 블로킹하지 못하게 한다.
- **깊이 제한**: 트리 순회 깊이에 상한을 둬 병적으로 깊은 트리(예: 브라우저의 DOM 미러 트리)에서 순회 시간이 폭증하지 않게 한다.
- **격리 실행**: 소스 B 전체를 별도 스레드(또는 프로세스별 타임아웃이 걸린 태스크)에서 수행해, 한 앱이 멈춰도 소스 A 의 결과나 세션 UI 자체는 살아있게 한다.

구체적 타임아웃 값·깊이 상한은 조사 자료에 없다 — §9 미해결 질문.

### 3.4 병합 계층

두 목록을 하나의 `CandidateSet` 으로 합친다. 전제: 3.2.2/3.3 절에서 이미 동일 좌표계(전역, 좌상단 원점, 포인트)로 정규화되어 있다.

| 단계 | 규칙 | 근거 / 상태 |
| :--- | :--- | :--- |
| M1. 합치기 | 목록 A ∪ 목록 B | 사실 — 두 소스가 병행 동작하고 결과가 하나의 목록으로 사용자에게 보인다는 점은 벤더 FAQ("also parse accessibility information ... Enable this by checking a box")에서 확정. |
| M2. 중복 판정 | 두 후보의 `frame` 이 겹치고(예: IoU 임계값 초과, 또는 중심점 간 거리가 임계값 이하) 텍스트가 대소문자 무시 후 동일/포함 관계이면 같은 실체로 판정 | `(추정)` — 정확한 임계값과 텍스트 비교 방식은 조사 자료에 없다. |
| M3. 충돌 시 우선순위 | 겹치는 한 쌍에서 **AX 쪽 `frame`을 채택**하고, 텍스트는 더 긴/완전한 쪽(대개 AX의 `kAXValueAttribute` 전체 문자열)을 채택한다 | `(추정)` — 근거: AX 프레임은 앱이 직접 보고하는 값이라 OCR 의 이미지 인식 오차(줄 경계 오분할, 안티앨리어싱 등)가 없다. 다만 제품이 "OCR 이 주, AX 가 부" 라는 서술과 이 규칙이 상충하지 않는지는 확정되지 않았다 — §9. |
| 정렬 순서 | 읽기 순서(화면 좌표 `y` 오름차순, 동률이면 `x` 오름차순)를 기본으로 제안 | `(추정)` — 개발자 포스트는 "up/down arrows or tab/shift+tab 로 후보 순환"만 언급하고 순서 기준은 밝히지 않는다. 커서 거리 기준일 가능성도 배제할 수 없다 — §9. |

### 3.5 질의 매칭 규칙

질의 문자열이 주어지면 `CandidateSet` 에서 일치하는 항목만 남긴다.

| 항목 | 제안 규칙 | 상태 |
| :--- | :--- | :--- |
| 대소문자 구분 | 구분하지 않음(대소문자 무시 비교) | `(추정)` — Spotlight 류 빠른 검색 UI 의 일반적 관례. 확정 근거 없음. |
| 매치 방식 | 부분 문자열 매치(포함 여부) | `(추정)` — 접두사 매치만이라면 버튼 중간 단어를 찾기 어려워 UX 상 부자연스럽다. 확정 근거 없음. |
| 공백 처리 | 후보 텍스트·질의 모두 앞뒤 공백 트림, 연속 공백은 단일 공백으로 정규화 후 비교 | `(추정)` |
| 빈 질의 | 후보 없음(0개) 상태로 취급하거나 전체 후보를 그대로 노출 | `❓미확인` — 두 동작 모두 가능하며 조사 자료로 판정 불가 |

이 절의 모든 규칙은 §9 미해결 질문으로 승계한다.

---

## 4. 설정 항목

| 이름 (원문) | 타입 | 기본값 | 유효 범위 | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| `Seek using macOS accessibility` | 체크박스 (bool) | ☑ (스크린샷 기준) `(추정 — 출고 기본값 미확정, Q3)` | on/off | `superkey-inventory.md` §3.1 (스크린샷), v1.18 appcast 도입 노트(§2.2) |
| ↳ `Match on more than one character` | 체크박스 (bool), `Seek using macOS accessibility` 켜짐일 때만 유효 | ☑ (스크린샷 기준) `(추정 — 출고 기본값 미확정, Q3)` | on/off | `superkey-inventory.md` §3.1 (스크린샷) |
| `Only Seek in the frontmost window` | 체크박스 (bool) | ☐ (스크린샷 기준) `(추정 — 출고 기본값 미확정, Q3)` | on/off | `superkey-inventory.md` §3.1 (스크린샷) |

> 세 항목 모두 스크린샷에서 판독한 값이며, 조사 문서는 "스크린샷의 체크 상태는 홍보용 구성이지 출고 기본값이라는 보장은 없다"고 명시적으로 유보한다(`superkey-inventory.md` Q3). 클론 구현 시 실제 출고 기본값은 별도 확인이 필요하다.

---

## 5. 엣지 케이스와 실패 모드

| # | 케이스 | 기대 동작 |
| :--- | :--- | :--- |
| 1 | Screen Recording 권한 거부/미부여 | 소스 A 전체가 죽는다(캡처 자체가 실패). 소스 B 가 켜져 있으면 그 결과만으로 세션이 계속된다. 권한 유도 UX 자체는 `F-11`. |
| 2 | OCR 이 후보 0개 반환(예: 화면이 전부 이미지/영상이거나 텍스트가 없음) | 소스 A 목록이 빈 배열. 소스 B 결과와 병합해 그대로 진행. 둘 다 0개면 세션은 열려 있되 후보 없음 상태. |
| 3 | 다중 디스플레이, 해상도·배율이 서로 다름 | §3.2.2 의 디스플레이별 독립 변환(`originX/Y`, `scale` 을 디스플레이마다 별도 적용)이 정확히 이뤄져야 함. v1.55 에서 실제로 깨졌던 영역이므로 회귀 테스트 필수. |
| 4 | Retina 배율(`backingScaleFactor` = 2.0 등) | §3.2.2 3단계의 픽셀→포인트 나눗셈을 누락하면 후보 프레임이 실제 텍스트보다 커 보이거나 좌표가 밀린다. |
| 5 | 대상 창이 전체화면(Full Screen) 스페이스에 있음 | 전체화면 앱은 별도 Space 에서 렌더링되며 창 레벨 취급이 다르다. 캡처 대상에 포함되는지, 오버레이가 그 위에 뜨는지는 `F-03`/캡처 API 선택에 좌우된다. `platform-constraints.md` 의 오버레이 창 레벨 판정과 함께 검증 필요. |
| 6 | 대상 앱이 Accessibility 를 지원하지 않음(예: 일부 게임, 커스텀 렌더링 엔진 앱) | 소스 B 에서 해당 앱은 요소 0개. 소스 A(OCR)만으로 그 앱 위의 텍스트를 커버해야 한다 — 이것이 AX 를 보조로 두는 이유이기도 하다. |
| 7 | 대상 앱이 AX 요청에 응답하지 않음(행업, 정지) | §3.3.3 의 `AXUIElementSetMessagingTimeout` + 깊이 제한이 없으면 Seek 전체가 블로킹된다. 타임아웃 초과 시 해당 앱은 건너뛰고 나머지 소스로 계속 진행. |
| 8 | 매우 큰 화면(고해상도 단일 또는 다중 디스플레이 합산)에서 OCR 지연 | `.accurate` 레벨의 비용이 화면 크기에 비례해 커진다. 세션 열림 → 후보 표시까지의 지연 예산 초과 위험(`platform-constraints.md` P3). 디스플레이별 병렬 처리로 완화 가능. |
| 9 | 세션이 열린 채로 화면 내용이 바뀜(애니메이션, 스크롤, 알림 팝업 등) | 검출은 세션 열림 시점의 단일 스냅샷을 기준으로 하는지, 주기적으로 재검출하는지가 조사 자료로 확정되지 않음 `❓미확인`. 재검출하지 않는다면 화면과 후보 위치가 어긋날 수 있다. |
| 10 | RTL 텍스트(아랍어/히브리어/페르시아어 — 번들 로케일에 포함) | OCR `recognitionLanguages` 에 해당 언어가 없으면 인식 자체가 실패한다. 인식되더라도 §3.4 의 "읽기 순서" 정렬이 LTR 전제라면 RTL 언어에서 순서가 부자연스러울 수 있다. |
| 11 | 회전된 텍스트(세로쓰기, 기울어진 UI 요소) | `VNRecognizedTextObservation.boundingBox` 는 기본적으로 축 정렬(axis-aligned) 사각형을 전제한다. 회전된 텍스트는 인식률이 낮거나 프레임이 실제 텍스트를 느슨하게만 감싼다. |
| 12 | 극소 텍스트 / 특정 색상 대비(검정 배경 위 특정 파란색) / 인접 줄과 너무 가까운 텍스트 | 개발자 본인이 명시한 확인된 OCR 실패 사례 3종(§4.5 인용). v1.19 에서 "font colors/backgrounds" 처리가 개선됐으나 완전히 해소됐다는 근거는 없다. |
| 13 | 최소 지원 OS(12.0)와 ScreenCaptureKit 요구 버전(12.3+)의 간극 | §7 에서 최소 버전을 12.3 으로 상향하는 결정을 내린다. 12.0–12.2 사용자는 애초에 이 기능(및 앱 전체)을 설치할 수 없게 된다 — 근거는 §7. |

---

## 6. 필요한 플랫폼 API

**소스 A (OCR·캡처)**
- `CGImage` — 캡처 결과 프레임 표현
- ScreenCaptureKit: macOS 14+ 는 `SCScreenshotManager`(단발 캡처), 12.3–13.x 는 `SCStream` 기반 델리게이트 캡처에서 1프레임만 취함
- `VNImageRequestHandler(cgImage:)`, `VNRecognizeTextRequest`, `VNRecognizedTextObservation`(`boundingBox`, `topCandidates(_:)`)
- `CGDisplayBounds(_:)` / `NSScreen.screens` — 디스플레이별 전역 원점, `backingScaleFactor`
- (권한 확인만, 상세는 `F-11`) `CGPreflightScreenCaptureAccess()`

**소스 B (AX)**
- `AXUIElementCreateSystemWide()`, `AXUIElementCreateApplication(pid:)`
- `AXUIElementCopyAttributeValue` — `kAXRoleAttribute`, `kAXValueAttribute`, `kAXTitleAttribute`, `kAXDescriptionAttribute`, `kAXChildrenAttribute`, `kAXPositionAttribute`, `kAXSizeAttribute`, `kAXFocusedWindowAttribute`(최전면 창 판정용)
- `AXValueGetValue` — `AXValue` → `CGPoint`/`CGSize` 언패킹
- `AXUIElementSetMessagingTimeout` — 블로킹 방지
- (권한 확인만, 상세는 `F-11`) `AXIsProcessTrusted()`

---

## 7. 구현 접근

### 소스 A — Vision OCR: **Rust 바인딩**

`objc2-vision`(0.3.2)이 `VNImageRequestHandler`/`VNRecognizeTextRequest`/`VNRecognizedTextObservation` 을 헤더 자동 생성 바인딩으로 그대로 노출하며, `screencapturekit`(8.0.1, 고수준) 또는 `objc2-screen-capture-kit`(0.3.2, 원시)으로 캡처가 가능하다(`rust-macos-capability-notes.md` §1.1, §2.3, §2.4). `CGImage` ↔ Vision 연결과 좌표 변환(§3.2.2)은 순수 Rust 로직으로 작성 가능하다. 별도의 Swift/Objective-C 브리징 코드(네이티브 shim)를 새로 작성할 필요는 없다 — 다만 호출부 전체가 `unsafe` FFI 라는 점에서 "순수 Rust" 는 아니다. ScreenCaptureKit 의 비동기 델리게이트 기반 API 를 "단발 캡처" 용도로 다루는 것은 다소 번거롭지만(`platform-constraints.md` P3 참조), 이는 구현 난이도 문제이지 언어·바인딩 가능성 문제는 아니다.

### 소스 B — Accessibility 파싱: **Rust 바인딩**

`axuielement`(0.9.1, "현재 가장 활발")가 `AXUIElement`/`AXValue`/`AXAttribute`/`ProcessTrust` 를 안전한 고수준 API 로 감싼다(`rust-macos-capability-notes.md` §1.2, §2.2). 필요한 속성(`kAXPositionAttribute` 등) 조회와 `AXUIElementPerformAction` 은 커버된다. 단, `AXUIElementSetMessagingTimeout` 이 `axuielement` 크레이트에 노출되는지는 조사에서 확인되지 않았다 `❓미확인`(§9) — 노출되지 않는다면 `accessibility-sys`(0.2.0, 원시 FFI)로 그 한 호출만 직접 선언해도 여전히 "Rust 바인딩" 범주이며 네이티브 shim 은 필요 없다.

### ⭐ 최소 macOS 버전 결정: **12.3 으로 상향**

조사에서 확인된 간극: 제품 공식 최소 버전은 **macOS 12.0** 이지만, 채택한 캡처 API 인 **ScreenCaptureKit 은 macOS 12.3+** 에서만 동작한다(`rust-macos-capability-notes.md` §2.3, P2). 대안은 두 가지였다.

1. **폴백**: 12.0–12.2 에서는 `CGWindowListCreateImage` 로 캡처.
2. **최소 버전 상향**: 지원 최소 버전을 12.3 으로 올리고 폴백 경로 자체를 두지 않는다.

**2안(상향)을 채택한다.** 근거:
- `CGWindowListCreateImage` 는 macOS 14 에서 **deprecated** 이며 향후 제거 위험이 있는 API 에 신규로 의존하는 것은 유지보수 부채다.
- 소스 A 전체(캡처+변환 경로)를 이중으로 유지하는 비용이, 12.0–12.2 라는 매우 좁은 패치 버전 구간의 지원 이득보다 크다고 판단한다.
- 12.0(2021 출시)은 이 조사 시점(2026)에서 이미 노후 OS 이며, 12.0–12.2 로 좁혀지는 실사용자 비율은 극히 작을 것으로 추정된다 `(추정 — 실측 데이터 없음)`.

이 결정은 **명세 저자의 판단이며 제품 요구사항 확정이 아니다.** 실제 채택 여부는 §9 미해결 질문으로 다시 올린다.

---

## 8. 수용 기준

- [ ] Screen Recording 권한이 있을 때, 화면에 보이는 텍스트에 대해 소스 A(OCR)가 텍스트+화면 좌표 후보를 생성한다.
- [ ] `Seek using macOS accessibility` 가 켜져 있을 때, 소스 B(AX)가 대상 앱의 텍스트 요소에서 텍스트+화면 좌표 후보를 생성한다.
- [ ] `Seek using macOS accessibility` 가 꺼져 있을 때, 소스 B 는 후보를 생성하지 않고 소스 A 만으로 세션이 정상 동작한다.
- [ ] 소스 A 의 `VNRecognizedTextObservation.boundingBox`(정규화, 좌하단 원점)가 §3.2.2 의 수식을 거쳐 소스 B 와 동일한 좌표계(전역, 좌상단 원점, 포인트)로 정확히 변환된다.
- [ ] 서로 다른 해상도·배율(`backingScaleFactor`)을 가진 2개 이상의 디스플레이에서, 각 디스플레이의 후보 좌표가 올바른 전역 위치를 가리킨다(디스플레이별 원점·배율이 독립적으로 적용됨).
- [ ] `Match on more than one character` 가 켜져 있을 때, 소스 B 의 1글자 텍스트 요소가 후보 목록에서 제외된다.
- [ ] `Only Seek in the frontmost window` 가 켜져 있을 때, 최전면 창 밖의 텍스트는 어느 소스에서도 후보로 나타나지 않는다.
- [ ] 소스 A 와 소스 B 가 겹치는 위치에서 동일 텍스트를 각각 검출했을 때, 병합 결과에 중복 후보가 아닌 단일 후보만 남는다(M2/M3 규칙).
- [ ] Screen Recording 권한이 없을 때 소스 A 결과가 빈 목록이 되며, 세션 자체는 크래시 없이 유지된다.
- [ ] 응답하지 않는 앱이 대상 화면에 있을 때, 소스 B 순회가 전체 파이프라인을 무기한 블로킹하지 않고 타임아웃 후 진행된다.
- [ ] OCR 결과가 0개인 화면(순수 이미지 등)에서도 세션이 오류 없이 후보 0개 상태를 반환한다.
- [ ] 병합된 `CandidateSet` 에 질의 문자열을 적용했을 때, §3.5 의 매칭 규칙(대소문자 무시·부분 문자열·공백 정규화)에 따라 일치하는 후보만 남는다.

---

## 9. 미해결 질문

| # | 질문 | 왜 확정 못 했는가 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| Q-a | OCR(소스 A)을 사용자가 끌 수 있는가, 아니면 항상 켜져 있는가 | 스크린샷·FAQ 어디에도 "OCR 끄기" 옵션이 보이지 않는다 | 앱 설치 후 환경설정 전수 확인 |
| Q-b | 병합 시 소스 A/B 충돌에서 실제로 어느 쪽 좌표·텍스트를 우선하는가(§3.4 M3) | 조사 자료에 병합 알고리즘 서술 없음. 본 문서의 "AX 우선" 은 추정 | 소스 코드 또는 실제 앱에서 겹치는 텍스트에 대한 관찰 실험 |
| Q-c | 후보 정렬 순서가 읽기 순서인지 커서 거리 기준인지(§3.4) | 개발자 포스트는 순환 방향(↑↓/Tab)만 언급, 정렬 기준 언급 없음 | 실제 앱에서 다중 매치 화면 관찰 |
| Q-d | 질의 매칭 규칙(대소문자·부분/접두사·공백) 전체(§3.5) | 조사 자료에 매칭 알고리즘 서술 없음 | 실제 앱 동작 관찰 |
| Q-e | `Only Seek in the frontmost window` 가 소스 A(캡처 영역)까지 좁히는지, 소스 B(AX 루트)에만 적용되는지(§3.3.2) | 설정이 두 소스 중 어디에 속하는지 UI 상 명확히 구분되어 있지 않음 | 실제 앱에서 다른 창의 텍스트가 프론트모스트 창 제한 시 캡처/AX 각각에서 사라지는지 관찰 |
| Q-f | `AXUIElementSetMessagingTimeout` 이 `axuielement` 0.9.1 크레이트에 노출되는가 | 크레이트 문서를 정밀 대조하지 않음(조사 범위 밖) | 크레이트 API 문서·소스 확인 |
| Q-g | 세션이 열려 있는 동안 화면 변화에 대해 재검출을 수행하는가, 최초 스냅샷 고정인가(§5 #9) | 조사 자료에 언급 없음 | 실제 앱에서 세션 도중 화면 변화 관찰 |
| Q-h | `usesLanguageCorrection`, `recognitionLanguages`, `minimumTextHeight` 의 실제 값 | Vision 파라미터 선택은 벤더가 공개하지 않음 | 소스 코드 확인 또는 실측(스캔 텍스트 조합 테스트) |
| Q-i | 최소 macOS 버전을 12.3 으로 상향하는 §7 의 결정에 대한 제품 승인 | 이 문서 저자의 기술적 판단이며 비즈니스 결정(구버전 OS 사용자 배제)은 별도 승인 필요 | 제품 오너 확인 |
| Q-j | 번들 로케일(de/he/ar/el/ja/fa/uk)과 `recognitionLanguages` 매핑 여부, RTL 언어에서의 후보 정렬 | 조사 자료는 UI 로케일 번들만 확인, OCR 인식 언어와의 연결 여부는 불명 | 소스 코드 확인 또는 다국어 화면 실측 |
