# F-02 · Seek — 텍스트 후보 검출

> **한 줄 요약**: 화면 캡처에 대한 Vision OCR(항상 켜지는 기본 소스)과, 최전면 창에 한해 후보를 보강하는 Accessibility 트리 파싱, 그리고 화면에 남아 있는(가려진 것 포함) 창의 제목을 후보로 넣는 `CGWindowList` 창 제목 검색(소스 C — ⭐ 이슈 #133, D12)이라는 **세 소스**에서 "클릭 가능한 텍스트 후보" 목록을 만들고, 중복 시 OCR 을 우선해 병합한다. ⭐ 화면 캡처는 **ScreenCaptureKit 이 아니라 `CGDisplayCreateImage` 계열**이다(실측: 번들 심볼).
> **의존성**: 후보 검출을 *언제* 시작하는지는 `F-01`(`seek-activation-and-session.md`)이 결정한다. 이 문서는 세션이 열려 질의 문자열이 주어진 상태에서 "화면 텍스트 후보 목록 + 질의 매칭 결과"를 산출하는 것까지만 다룬다. 소스 A(OCR)·⭐(이슈 #133) 소스 C(창 제목)는 Screen Recording 권한에, 소스 B(AX)는 Accessibility 권한에 의존한다 — 권한 획득 절차 자체는 `F-11`(`permissions-onboarding.md`) 참조.
> **관련 명세**: 후보를 화면에 그리는 방법 → `F-03`(`seek-overlay-ui.md`, AX 매치의 "요소 전체 하이라이트"는 여기서 다룬다). 선택된 후보를 클릭하는 방법 → `F-04`(`seek-click-execution.md`). 권한 획득 UX → `F-11`(`permissions-onboarding.md`). 플랫폼 역량 종합 판정 → `docs/spec/platform-constraints.md`(별도 문서, 이 문서 범위 밖).
> **1차 근거**: `docs/research/app-bundle-analysis.md`(실측: AX 트리·`defaults`·번들 심볼·번들 문자열). 근거 표기 정의는 그 문서 §0.

---

## 1. 개요

Seek 는 사용자가 타이핑한 질의 문자열과 일치하는 텍스트를 화면에서 찾아 그 위치를 클릭 후보로 제시하는 기능이다. 이 문서가 다루는 범위는 "질의 문자열 + 현재 화면 상태" 를 입력으로 받아 "클릭 가능한 후보 목록(텍스트 + 화면 좌표)" 을 출력하는 검출·병합 파이프라인 하나뿐이다.

검출은 세 개의 독립 소스를 병행한다.

- **소스 A (항상 켜지는 기본 소스) — Vision OCR**: 화면을 스크린샷으로 캡처해 Apple Vision 프레임워크로 문자를 인식한다. ⭐ **끌 수 있는 UI 컨트롤이 없다는 사실이 확정**됐다 — `Seek using macOS accessibility` 옆 ⓘ 팝오버 원문(§3.1)이 AX 를 "잠재적으로 더 많은 텍스트 항목을 찾게 해주는" 보강 소스로 서술하는 반면 OCR 을 끄는 스위치는 어디에도 없다(실측: AX 트리 — Seek 탭 8개 컨트롤 전수 확인, app-bundle-analysis.md §6.1).
- **소스 B (최전면 창 한정 보강 경로) — Accessibility 파싱**: `Seek using macOS accessibility` 체크박스로 켜는 선택 기능이다. 대상 앱이 `AXUIElement` 트리로 노출하는 텍스트 요소를 직접 읽는다. v1.18 에서 "initial cut" 으로 추가됐으며 OCR 이 놓치는 후보를 보강하는 역할이다. ⭐ **AX 소스는 이 체크박스와 무관하게 애초에 최전면 창으로 범위가 고정된다** — ⓘ 팝오버 원문이 "in the frontmost window" 라고 명시한다(§3.1, §3.3.2).
- **소스 C (제3 소스, ⭐ 이슈 #133 · D12) — 창 제목 검색**: `Search window titles` 체크박스(기본 ☑, §4)로 제어하는 보강 소스다. `CGWindowListCopyWindowInfo(OptionOnScreenOnly)` 로 화면에 남아 있는 창(가려진 것 포함, 최소화 제외)의 제목(`kCGWindowName`)을 읽어 후보로 넣는다 — **화면에 안 보이는(가려진) 창의 제목을 검색 대상에 넣는 보강 소스**다. Accessibility(소스 B)가 최전면 창 하나로 한정되는 것과 달리 범위 제한이 없고, 같은 가시(OCR/AX) 텍스트가 그 창 bounds 안에 있으면 중복 제거된다(§3.4 M4). 좌표는 창 bounds 전체라 정밀 위치가 없어, 확정 동작도 **클릭이 아니라 창 전면화**다(`seek-click-execution.md` §3.7).

세 소스의 산출물은 **동일한 자료형(텍스트 + 전역 화면 좌표)** 으로 정규화된 뒤 병합되어 하나의 후보 목록이 된다. ⭐ **AX·OCR 은 배타적이지 않다.** 둘 다 동시에 후보를 낼 수 있고, 겹치는 경우 **OCR 매치가 AX 매치를 대체**한다(우선순위 확정, §3.4). ⭐(이슈 #133) **세 소스는 배타적이지 않다** — 창 제목 후보(소스 C)는 위 둘과 겹치면 §3.4 M4 규칙(같은 정규화 텍스트 + 창 bounds 안)으로 제거되고, 소스 C 끼리(같은 제목의 다른 창)는 유지된다.

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
        [소스 A: Vision OCR, 항상 동작]              [소스 B: Accessibility, 항상 최전면 창 한정]
       A1 화면 캡처 (CGDisplayCreateImage 계열)       (설정 켜짐 시에만 동작)
       A1.5 캡처 이미지 전처리 (CoreImage, ⭐ 신규)     B1 AX 루트 획득 (항상 최전면 창)
       A2 VNRecognizeTextRequest 실행                 B2 트리 순회 + 텍스트 요소 수집
       A3 좌표 변환 (정규화·좌하단→전역·좌상단)         B3 범위 필터 적용 (minAxCharCount)
       A4 TextCandidate 목록 A                        B4 TextCandidate 목록 B (요소 전체 하이라이트)
                     └─────────────────┬─────────────────┘
                                        ▼
                        M. 병합 — 중복 시 OCR 이 AX 를 대체 (M1~M3) · ⭐ 소스 C 중복 제거 (§3.4 M4, 이슈 #133)
                                        ▼
                         전역 후보 목록 CandidateSet
                                        ▼
                         질의 문자열 매칭 → 정렬된 부분집합
                                        ▼
                         F-03(오버레이) / F-04(클릭) 로 전달
```

⭐ **소스 B 의 범위는 `Only Seek in the frontmost window` 설정과 무관하게 항상 최전면 창으로 고정된다.** 그 설정은 소스 A(OCR/캡처)의 범위만 좁힌다. 두 설정의 역할 분리는 §3.3.2 참조.

⭐(이슈 #133) **소스 C(창 제목)는 위 그림 이후 병합 계층에 합류한 세 번째 소스다** — CGWindowList 로 화면에 남아 있는(가려진 것 포함) 창 제목을 읽어 `merge_candidates`(A∪B) 출력에 붙이고(§3.3.4), §3.4 M4 규칙으로 가시 텍스트와 겹치는 제목 후보를 거른 뒤 읽기 순서로 정렬한다. 별도 분기로 그리지 않았을 뿐, 병합 지점(▼ M) 이후의 흐름은 세 소스가 동일하다.

### 3.2 소스 A — Vision OCR

⭐⭐ **화면 캡처 API 정정 — ScreenCaptureKit 이 아니다.** 실측(`nm -u`): 링크된 심볼은 **`CGDisplayCreateImage`** · **`CGDisplayCreateImageForRect`** · `CGWindowListCopyWindowInfo` · `CGWindowListCreateDescriptionFromArray` 이고, `SCStream`·`SCContentFilter`·`SCShareableContent` 등 ScreenCaptureKit 심볼은 **전무**하다(app-bundle-analysis.md §3.1). `LSMinimumSystemVersion = 12.0` 과 정합한다 — 원본은 macOS 12.0 을 유지한 채 `CGDisplayCreateImage` 계열 레거시 API 로 출하 중이다. ⭐ **`CGWindowListCopyWindowInfo` 의 용도가 확정됐다(이슈 #133)** — **소스 C: 창 제목 후보 획득**(§3.3.4, §6). 기존 추정("`Only Seek in the frontmost window` 의 창 범위 판정")은 §3.3.2 의 추정으로 유보해 둔다 — 같은 함수가 두 용도에 쓰일 가능성을 배제하지 않는다(소스 C 용도는 실측·확정, 범위 판정 용도는 미확정).

| 단계 | 입력 | 처리 | 출력 |
| :--- | :--- | :--- | :--- |
| A1. 화면 캡처 | (트리거 신호) | 디스플레이별로 `CGDisplayCreateImage(displayID)` 또는 특정 영역만 `CGDisplayCreateImageForRect(displayID, rect)` 캡처(실측: 번들 심볼) | `[(displayID, CGImage, imgW, imgH, backingScaleFactor)]` |
| A1.5. ⭐ 전처리 (신규) | 디스플레이별 `CGImage` | §3.2.1 의 CoreImage 필터 파이프라인 적용 | 전처리된 `CGImage` |
| A2. OCR 실행 | 전처리된 `CGImage` | `VNImageRequestHandler(cgImage:)` 생성 → `VNRecognizeTextRequest` 실행 | `[VNRecognizedTextObservation]` (디스플레이별) |
| A3. 좌표 변환 | `VNRecognizedTextObservation.boundingBox`(정규화, 좌하단 원점) | §3.2.3 수식 적용. `VNImageRectForNormalizedRect` 심볼의 존재가 이 정규화 좌표 → 화면 좌표 변환 단계 자체가 필요함을 확정한다(실측: 번들 심볼) | 전역 화면 좌표 `CGRect`(포인트 단위, 좌상단 원점) |
| A4. 후보 생성 | 변환된 좌표 + `topCandidates(1)` 문자열 | `TextCandidate { text, frame, source: OCR, confidence }` 구성 | `[TextCandidate]` (목록 A) |

#### 3.2.1 ⭐ 캡처 이미지 전처리 파이프라인 (신규 확정)

기존 명세에 전혀 없던 단계다. CoreImage 가 링크되어 있고 다음 필터 이름이 실행 파일에 있다(실측: 번들 심볼, app-bundle-analysis.md §3.1, §4.7):

- **`CILanczosScaleTransform`** — 스케일 변환. 캡처 이미지를 OCR 에 유리한 해상도로 정규화하는 용도로 추정된다.
- **`CIPhotoEffectMono`** / **`CIPhotoEffectNoir`** — 흑백 변환. 대비를 강화해 저대비 텍스트(§4.5 확인된 실패 사례)의 인식률을 높이는 용도로 추정된다.
- **`CIMaximumComponent`** / **`CIMinimumComponent`** — RGB 성분 중 최대/최소값 추출. 특정 색상 채널의 텍스트(예: 검정 배경 위 특정 파란색, §5 #12)를 두드러지게 하는 용도로 추정된다.

내부 타입 `CaptureFiltering` · `CaptureFilterType` · `FilterFactory` 계열(`CompareFilter` · `MessageFilterFactory` · `PathFilterFactory`)이 이 필터들을 선택·적용하는 구조로 보인다.

⭐ **어떤 조건에서 어떤 필터를 고르는지는 `(미확정)`이다** — 화면 콘텐츠(밝기·대비)에 따라 동적으로 고르는 적응형 파이프라인인지, 항상 고정된 순서로 전부 적용하는지 조사 자료로 확정되지 않았다. §9 미해결 질문으로 승계한다.

#### 3.2.2 인식 파라미터와 트레이드오프

| 파라미터 | 선택 | 근거·트레이드오프 |
| :--- | :--- | :--- |
| `recognitionLevel` | `.accurate` | `.fast` 는 지연이 낮지만 작은 텍스트·저대비 텍스트에서 인식률이 더 떨어진다. 조사에서 확인된 실패 사례(극소 텍스트, 저대비 색상)를 줄이려면 `.accurate` 가 필요하다. 대가는 지연 — 큰 화면·다중 디스플레이에서 수백 ms 단위로 늘어날 수 있다(§5 참조, `platform-constraints.md` P3). |
| `usesLanguageCorrection` | `false` `(추정)` | Seek 가 찾는 대상은 UI 라벨·버튼 텍스트 등 자연어 문장이 아닌 경우가 많다. 언어 모델 기반 보정은 "OK", "확인" 같은 짧은 UI 텍스트를 실제와 다른 단어로 교정할 위험이 있다. 근거 자료 없음 — §9 미해결 질문으로 승계. |
| `recognitionLanguages` | ⚠️ **정본이 이슈 #93 으로 바뀌었다 — 검색 언어 설정(`seek.searchLanguage`, Plan D6)이 정본**이다: 명시 비영어(`ko`·`zh`·`ja`·`es`) → `["<lang>-<region>", "en-US"]`, 명시 `"en"` → `[]`(영어 강제). ⭐ **설정 부재 = 영어 고정(이슈 #131, D11)** — 이슈 #48 의 로케일 자동 매핑은 **폐기**됐다(부재 시 `Locale::En` → `recognitionLanguages` 빈 목록 = Vision 기본 영어). ⭐ 결정 사슬: **S-4**(기본 미설정/영어, 실측) → **이슈 #48**(UI 로케일 자동 매핑, 설정 부재 시 임시 정본) → **이슈 #93**(검색 언어 설정 정본 + 로케일 폴백) → **이슈 #131**(2026-09-05 — 설정 부재 = 영어 고정. 이슈 #48 의 로케일 폴백은 폐기, D11). 위 실측 대가(전체 화면 3.1s)가 그대로 유지되므로 **기본은 영어 단일**을 유지한다 | ⭐⭐ **실측**(`docs/dev/seek-ocr-latency-spike.md` §3): ① Vision 의 기본 인식 언어는 **영어뿐**이다 — 화면이 한글로 가득해도 후보가 **0개**다(조용한 실패). ② `"ko-KR"` 은 목록의 **첫 번째**여야 동작한다: `["en-US", "ko-KR"]` 의 결과는 `["en-US"]` 와 관측 수·시간·신뢰도까지 **완전히 동일**했다. ③ 대가가 크다 — `["ko-KR", "en-US"]` 는 전체 화면 775 ms → **약 3.1 s**(3~4배), 평균 신뢰도 0.83 → 0.59. 그래서 기본으로 켜지 않는다(결정 **S-4**). 기존의 "8개 로케일 번들" 근거는 §5 #10 이 이미 오독으로 정정했다. ⚠️ **비영어 3종(zh·ja·es)의 지연·신뢰도는 한국어만 실측**(`(추정)` — manual-verification.md M-item 으로 각 1회 재현 측정). ⭐ **이슈 #131 (D11) — 부재 = 영어 고정을 고른 근거**: 영어 기본 = Vision 기본(성능·신뢰도 실측상 최적), 이슈 #93 의 선택 언어 UI 로 원하면 명시적으로 켠다, 부재 폴백이 사용자가 선택했다는 암시를 만들어 사용자 피드백("선택했을 때만")과 어긋남. **기각**: (a) 현행 유지 — 한국어 UI 사용자가 설정을 건드리지 않아도 3~4배 지연을 무의식적으로 짐, (b) 기본값 명시 저장 — "부재=기본값" 규약(F-15) 위반, (c) 다국어 복수 확장 — 범위 밖(별도 이슈). |
| `minimumTextHeight` | 명시적으로 낮게 설정하거나 미설정(기본값) `(추정)` | "extra small text" 가 확인된 실패 사례이므로, 이 파라미터를 부주의하게 높게 두면 그 실패를 자초하게 된다. 구체적 수치는 조사 자료에 없다. |

#### 3.2.3 좌표 변환 (수식)

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

⭐⭐ **가장 중요한 정정 — AX 는 언제나 최전면 창 하나로 한정된다.** `Seek using macOS accessibility` 옆 ⓘ 팝오버 원문(SuperKey 원문, 실측: AX 트리 + nib, `SeekAccessibilityViewController`):
> "Seek using macOS Accessibility"
> "Enable this to let Seek potentially find more text items in the frontmost window."
> "It's not possible to determine precise locations of text within an Accessibility element, so the entire element will be highlighted by Seek."
> "Seek matches using Optical Character Recognition (OCR) will display in place of duplicate matches from Accessibility."
> "Sometimes text found in an accessibility element can be obscured, so you might get matches even if you can't see the text in the element."

이 원문으로 확정되는 것:
- AX 는 **최전면 창에 한해서만** 후보를 추가하는 보강 소스다 — `Only Seek in the frontmost window` 설정과 무관하게 원래부터 최전면 창으로 범위가 고정되어 있다(끄고 켜는 것은 AX 자체일 뿐 순회 범위가 아니다).
- AX 매치는 **텍스트의 정확한 위치를 알 수 없어 요소 전체가 하이라이트**된다 — OCR 매치(문자 단위로 정밀한 `boundingBox`)와 하이라이트 형태가 다르다(`F-03` 소관).
- **중복 시 OCR 매치가 AX 매치를 대체**한다(우선순위 확정, §3.4 M3).
- AX 매치는 **가려진(obscured) 텍스트도 잡힐 수 있다** — 사용자 눈에 보이지 않는 텍스트가 매치되는 알려진 한계다(§5). 관련 타입 `UnobscuredWindowElement` 가 실재한다(실측: 번들 심볼).

| 단계 | 입력 | 처리 | 출력 |
| :--- | :--- | :--- | :--- |
| B1. 루트 획득 | (항상 최전면 창) | `kAXFocusedWindowAttribute` 로 얻은 최전면 창 하나만 루트로 사용 — `Only Seek in the frontmost window` 와 무관 | AX 루트 요소 1개 |
| B2. 트리 순회 | AX 루트 | 깊이 제한을 두고 `kAXChildrenAttribute` 로 재귀 순회. 각 요소에서 `kAXValueAttribute`/`kAXTitleAttribute`/`kAXDescriptionAttribute` 중 텍스트가 있는 것과, `kAXPositionAttribute`/`kAXSizeAttribute`(둘 다 `AXValue` → `CGPoint`/`CGSize` 로 언패킹) 를 읽는다 | `[(text, frame)]` 원시 목록 |
| B3. 범위 필터 | 원시 목록 + `minAxCharCount`(정수, §3.3.1) | `text.count >= minAxCharCount` 인 요소만 통과 | 필터링된 목록 |
| B4. 후보 생성 | 필터링된 목록 | `TextCandidate { text, frame(요소 전체), source: AX, confidence: None }` 구성 | `[TextCandidate]` (목록 B) |

`kAXPositionAttribute`/`kAXSizeAttribute` 는 이미 **전역 화면 좌표, 좌상단 원점, 포인트 단위**로 반환되므로(CoreGraphics 관례), 소스 A 와 달리 추가 좌표 변환이 필요 없다.

#### 3.3.1 ⭐ `Match on more than one character` — 숨겨진 것이지 삭제된 것이 아니고, 불리언이 아니다

기존 명세는 이 항목의 의미를 추정으로만 서술했다. 실측으로 다음이 확정된다(app-bundle-analysis.md §6.1, §2.1):

- **v1.66 스크린샷에 이 항목이 안 보이는 이유는 삭제가 아니라 숨김이다.** `Seek using macOS accessibility` 가 꺼져 있으면 이 체크박스는 AX 트리에서 완전히 사라진다(dimmed 가 아니라 hidden). AX 로 상위 체크박스를 켜자 나타났고, 끄자 다시 사라졌다(실측: AX 트리).
- **상위를 켰을 때의 기본값은 ☑ 다**(실측: AX 트리).
- **저장 형태가 불리언이 아니다.** `defaults` 에 **`minAxCharCount = 2`** 라는 **정수**로 저장된다(실측: `defaults`). 즉 "1글자 초과" 라는 이진 스위치가 아니라 **AX 요소 텍스트의 최소 글자 수 파라미터**이며, 체크박스는 이 정수를 2 와 1 사이에서 토글하는 UI 로 보인다 `(미확정 — 정수를 2/1 외의 값으로 설정하는 경로는 찾지 못했다. §9)`.
- ⭐ `minAxCharCount` 는 **아무 설정도 건드리지 않은 plist 에 이미 존재하는 단 두 개의 키 중 하나**다(다른 하나는 `NSWindow Frame EntryBarWindow`, app-bundle-analysis.md §2.1). 즉 앱이 첫 실행 시 이 값을 `2` 로 시딩(seed)해 두는 것으로 보인다 — 다른 Seek 설정들이 "값이 없으면 곧 기본값"인 것과 달리 이 키는 예외적으로 항상 존재한다.

이유(왜 최소 글자 수를 두는가)는 조사 자료에 직접 서술되어 있지 않으나 다음 근거로 여전히 추정한다: OCR 은 화면에 보이는 "덩어리 텍스트"만 관측하므로 1글자 후보가 드물게만 나오는 반면, AX 트리는 버튼 아이콘의 accessibility label, 구분자, 단축키 힌트 등 **화면에 시각적으로 드러나지 않는 1글자 텍스트 요소**까지 그대로 노출한다. 이런 요소를 그대로 후보에 포함시키면 사용자가 한 글자만 입력해도 후보가 폭증해 목록이 사실상 무의미해진다.

#### 3.3.2 ⭐ `Only Seek in the frontmost window` 은 OCR(소스 A) 범위만 좁힌다 — AX 범위와는 별개

기존 명세는 이 설정이 두 소스 모두에 적용된다고 추정했으나, §3.3 도입부에 인용한 AX ⓘ 팝오버 원문("in the frontmost window")으로 **AX 는 이 설정과 무관하게 원래부터 최전면 창 하나로 고정**됨이 확정됐다. 따라서 두 설정의 역할은 다음과 같이 분리된다:

- **AX(소스 B)**: 항상 최전면 창 한정. `Only Seek in the frontmost window` 를 꺼도 AX 의 순회 루트는 넓어지지 않는다.
- **`Only Seek in the frontmost window`**: OCR(소스 A) 의 캡처/인식 범위를 좁힌다 — 꺼짐이면 전체 화면(모든 디스플레이)을 캡처, 켜짐이면 최전면 창 프레임으로 캡처 영역을 잘라낸다(A1). 이 범위 축소 자체의 구체적 구현(창 프레임을 어떻게 얻는지 — `CGWindowListCopyWindowInfo` 로 추정, app-bundle-analysis.md §3.2)은 여전히 `(미확정)`이다.

#### 3.3.3 성능·블로킹 위험

AX 트리 순회는 **프로세스 간 동기 IPC** 다. 대상 앱이 응답하지 않으면 순회 호출이 블로킹되어 Seek 전체가 멈춘다. 이는 v1.19 "potential crash with Seek using the Accessibility API" 로 실제 발생한 이력이 있다(`superkey-inventory.md` §2.2). 대응책:

- **메시징 타임아웃**: 각 대상 프로세스의 `AXUIElementSetMessagingTimeout` 을 짧게(수백 ms 단위) 설정해 응답 없는 앱이 전체 파이프라인을 무기한 블로킹하지 못하게 한다.
- **깊이 제한**: 트리 순회 깊이에 상한을 둬 병적으로 깊은 트리(예: 브라우저의 DOM 미러 트리)에서 순회 시간이 폭증하지 않게 한다.
- **격리 실행**: 소스 B 전체를 별도 스레드(또는 프로세스별 타임아웃이 걸린 태스크)에서 수행해, 한 앱이 멈춰도 소스 A 의 결과나 세션 UI 자체는 살아있게 한다.

구체적 타임아웃 값·깊이 상한은 조사 자료에 없다 — §9 미해결 질문.

#### 3.3.4 ⭐ 소스 C — CGWindowList 창 제목 파싱 (이슈 #133 · D12, 신규)

⭐ 원본 SuperKey 에는 없는 **클론 고유 소스**다(갈라짐 표 D12). 사용자 명시 요구(이슈 #133: "중첩되어 있는 윈도우에서 뒤쪽에 있는 윈도우에 있는 문자열 탐색이 안 된다")에서 출발해, **화면에 안 보이는(가려진) 창의 제목**을 검색 대상에 넣는 보강 소스로 설계됐다. Accessibility(소스 B)가 최전면 창 하나로 한정되는 것과 달리 이 소스는 범위 제한이 없다.

| 단계 | 입력 | 처리 | 출력 |
| :--- | :--- | :--- | :--- |
| C1. 목록 획득 | (트리거 신호) | `CGWindowListCopyWindowInfo(kCGWindowListOptionOnScreenOnly, kCGNullWindowID)` 로 온스크린 창 딕셔너리 배열 획득 | `[CFDictionary]` (창 1개당 1항목) |
| C2. 필터 | 딕셔너리 배열 | ① `kCGWindowLayer == 0` 인 창만(일반 앱 창 — 메뉴 막대(26)·트루퍼(1000) 등 다른 레이어는 후보 불가) ② `kCGWindowName`(제목)이 **빈 문자열**이면 항목 제외 — 제목 없는 창은 검색 대상이 없다 | 필터링된 목록 |
| C3. 후보 생성 | 필터링된 목록 | `kCGWindowName` = 제목, `kCGWindowBounds` = 창 bounds 전체(전역 rect — §3.3.4-2), `kCGWindowNumber`/`kCGWindowOwnerPID` = 창 식별자(확정 동작의 창 전면화에 사용, `seek-click-execution.md` §3.7) | `[TextCandidate]` (목록 C, source: WindowTitle) |

**좌표계(§3.3.4-2)**: `kCGWindowBounds` 는 `X`/`Y`/`Width`/`Height` 키의 CFDictionary 로 돌아오며(키 이름은 2026-09-05 이 기기 전수 출력 실측으로 확정), 이미 **전역 화면 좌표, 포인트 단위, 좌상단 원점**이다 — 소스 A(§3.2.3 변환 결과)·소스 B(`kAXPositionAttribute`/`kAXSizeAttribute`)와 **동일한 좌표계**라 추가 좌표 변환이 필요 없다. `frame` 은 창 bounds **전체**(가시 텍스트의 정밀한 위치가 아니라 창 사각형)다 — 그래서 이 소스의 후보는 클릭 지점을 가질 수 없고, 확정 시 창 전면화로 실행된다(F-04 §3.7).

**`OptionOnScreenOnly` 의 의미**: 이 소스는 **화면에 나타나 있는 창**만 본다. "안 보이는 창"이란 위 §3.3.4 도입부에서 말한 대로 **가려진(occluded) 창**을 뜻하며, 최소화된 창은 온스크린 목록에서 빠진다.

⭐ **자기 창 제외(이슈 #133 REVISE 반영)**: Ultrakey 본인 프로세스의 창(설정 창·Event Viewer·About 창)은 세션 중 열려 있어도 후보에서 제외한다 — 이 기능의 목적은 "다른 앱의 가려진 창 검색"이므로 자기 창은 노이즈일 뿐이며, 오버레이 창은 항상 `always_on_top`(플로팅 레벨)이라 레이어 0 필터(C2 ①)로 이미 걸러진다.

**⭐ 권한 제약**: `CGWindowListCopyWindowInfo` 자체는 Screen Recording 권한이 없어도 호출에 **성공**한다. 다만 권한이 없으면 `kCGWindowName`(창 제목)이 통상 **빈 문자열**로 돌아오고, C2 의 빈 제목 필터가 그 항목을 제외하므로 이 소스는 **권한 없이 조용히 빈 결과(후보 0개)** 가 된다 — 실패로 드러나지 않는다. F-02 는 애초에 Screen Recording 권한을 요구·보유하므로(§1, `F-11`), 실제 사용 경로에서는 제목이 읽힌다.

### 3.4 병합 계층

두 목록을 하나의 `CandidateSet` 으로 합친다. 전제: 3.2.3/3.3 절에서 이미 동일 좌표계(전역, 좌상단 원점, 포인트)로 정규화되어 있다. 단, AX 매치는 요소 전체가 `frame` 이므로(§3.3) OCR 매치보다 넓은 사각형인 경우가 흔하다.

| 단계 | 규칙 | 근거 / 상태 |
| :--- | :--- | :--- |
| M1. 합치기 | 목록 A ∪ 목록 B | 사실 — 두 소스가 병행 동작하고 결과가 하나의 목록으로 사용자에게 보인다는 점은 벤더 FAQ 와 ⓘ 팝오버 원문(§3.3) 양쪽에서 확정. |
| M2. 중복 판정 | 두 후보의 `frame` 이 겹치고(예: IoU 임계값 초과, 또는 중심점 간 거리가 임계값 이하) 텍스트가 대소문자 무시 후 동일/포함 관계이면 같은 실체로 판정 | `(추정)` — 정확한 임계값과 텍스트 비교 방식은 조사 자료에 없다. |
| M3. ⭐ 충돌 시 우선순위 — 정정(승격) | 겹치는 한 쌍에서 **OCR 쪽이 AX 쪽을 대체**한다 | **확정** — `Seek using macOS accessibility` ⓘ 팝오버 원문(§3.3): "Seek matches using Optical Character Recognition (OCR) will display in place of duplicate matches from Accessibility." 기존 명세가 "AX 프레임을 채택한다"고 추정했던 것은 **정반대로 틀렸다.** OCR 이 항상 켜지는 기본 소스이고 AX 는 보강 소스라는 §1 의 구도와도 정합한다. |
| M4. ⭐(이슈 #133 · D12, 신규) 창 제목 후보(소스 C)의 중복 제거 | 창 제목 후보는, **같은 정규화 텍스트(소문자·공백 축약, `normalize_for_match`)를 가진 OCR/AX 후보가 그 창 bounds 안에 있으면** 버린다(구현: 가시 후보 프레임의 **중심점이 창 bounds 안** — 닫힌 구간 포함 판정). ⚠️ **창 제목 후보끼리(같은 제목의 다른 창)는 중복 제거하지 않는다** — 서로 다른 창이다 | **확정** — 이슈 #133 결정(comment-5547504824 3절). 가시 텍스트가 이미 그 제목을 대표하고 있으면 제목 후보는 잡음이므로 버리고, 정밀 위치가 없는 제목 후보가 가시 후보를 밀어내는 것을 막는다 |
| 정렬 순서 | 읽기 순서(화면 좌표 `y` 오름차순, 동률이면 `x` 오름차순)를 기본으로 제안 | `(추정)` — 개발자 포스트는 "up/down arrows or tab/shift+tab 로 후보 순환"만 언급하고 순서 기준은 밝히지 않는다. 커서 거리 기준일 가능성도 배제할 수 없다 — §9. |

⭐(이슈 #133) **소스 C 는 M1~M3 을 거친 뒤(OCR/AX 병합 출력에) 추가 병합된다** — `merge_candidates`(A∪B) 결과에 창 제목 목록(소스 C)을 붙이고(구현: `merge_window_titles`), 위 M4 규칙으로 가시 중복을 거른 뒤 읽기 순서로 다시 정렬한다. 분리한 이유는 두 병합의 성격이 다르기 때문이다 — M1~M3 은 "같은 실체를 하나로"이고, M4 는 "가시 후보를 대표하는 제목 잡음 제거"다.

### 3.5 ⭐ 다중 디스플레이·Stage Manager (신규)

내부 타입 `ScreenDetection` · `ScreenSettler` · 저장 키 `screenMatches` · `screenObservations`, 그리고 심볼 `SDySo8NSScreenCSaySo27VNRecognizedTextObservationCGGG`(Swift 맹글드 이름 = `[NSScreen: [VNRecognizedTextObservation]]`, 즉 **화면(`NSScreen`)을 키로 한 OCR 결과 딕셔너리**)가 실측된다(실측: 번들 심볼, app-bundle-analysis.md §4.7·§3.1). → **OCR 결과를 화면별로 따로 보관한다.** §3.2.3 의 디스플레이별 독립 좌표 변환과 정합하는 구조다.

⭐ `StageWindowAccessibilityElement` — **Stage Manager 전용 AX 처리**가 별도로 존재한다(실측: 번들 심볼). 기존 명세는 Stage Manager 를 다루지 않았다. Stage Manager 에서는 창이 그룹으로 축소·전환되는 방식이 일반 창과 달라 AX 트리 접근 경로가 특수 취급될 수 있다는 정도만 확정되고, 구체적으로 무엇이 다른지는 `(미확정)` — §9.

`WindowObservation` · `frontWindowObservations` · `closedWindows` · `cachedWindowId` · `cachedChildren` → **창 상태를 관찰·캐시**하는 구조가 있다. AX 트리 재탐색 비용을 줄이기 위한 것으로 보인다 `(미확정)`.

### 3.6 질의 매칭 규칙

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

⭐ **오기 정정.** 기존 명세는 세 항목 모두를 스크린샷(홍보용 구성) 값 그대로 `(추정)`으로 표시했다. 실측(AX 트리 + `defaults`)으로 아래처럼 확정된다. 판정 논리는 F-01 문서와 동일 — plist 에 키가 없으면 그 항목의 기본값은 OFF/미설정이다(`Match on more than one character` 는 예외적으로 plist 에 항상 존재하는 시딩 값, 아래 참조).

| 이름 (원문) | 타입 | 기본값 (실측) | 표시/활성화 조건 | 저장 키 | 출처 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `Seek using macOS accessibility` | 체크박스 (bool) | **☐** (실측: AX 트리 + `defaults` 부재) | 항상 표시 | `seekOptions` 비트(추정) | app-bundle-analysis.md §6.1, §2.2 |
| ↳ `Match on more than one character` | 체크박스 UI, 저장은 **정수**(§3.3.1) | **상위 켰을 때 기본 ☑**(정수값 `2`) (실측: AX 트리 + `defaults`) | ⭐ **`Seek using macOS accessibility` 가 ☑ 일 때만 존재 — ☐ 이면 dimmed 가 아니라 완전히 숨겨진다**(hidden, 실측: AX 트리) | `minAxCharCount`(정수, `2`/`1`) — ⭐ 아무 설정도 안 건드린 plist 에 이미 존재하는 두 키 중 하나(시딩값) | app-bundle-analysis.md §6.1, §2.1 |
| `Only Seek in the frontmost window` | 체크박스 (bool) | **☐** (실측: AX 트리) — 기존 명세와 값 일치, 실측으로 확인 완료 | 항상 표시. ⭐ AX(소스 B)의 범위와는 무관 — OCR(소스 A)의 캡처 범위만 좁힌다(§3.3.2) | `seekFrontmostOnly` | app-bundle-analysis.md §6.1, §2.2 |
| ⭐(이슈 #133 · D12) `Search window titles` | 체크박스 (bool) | **☑ (부재 = 참)** — ⚠️ **원본에 없는 신규 항목이라 실측 출고 기본값이 없다** — ☑ 는 **클론 설계 결정**(이슈 #133)이다. 라벨 ko: `창 제목 검색` | 항상 표시 | `seek.includeWindowTitles` | 이슈 #133 (원본에 없는 D12 신규 기능) |

> ⭐(이슈 #133) **`Search window titles` 는 위 표의 나머지 행들과 근거 등급이 다르다.** 나머지는 전부 원본 v1.66 실측(부재 = 원본 기본값)이지만, 이 행은 **원본에 없는 클론 고유 항목**(D12 — 갈라짐 표)이라 실측값이 존재하지 않는다. **부재 = 참(☑)** 은 클론 설계 결정이며, 그 근거·기각 대안은 이슈 #133 결정 코멘트(comment-5547504824)에 기록돼 있다 — 여기서는 결정만 확정으로 반영한다. 힌트 원문: en "Search window titles even when they are obscured by other windows" · ko "다른 창에 가려 보이지 않아도 창 제목을 검색 대상에 넣는다".

> `Match on more than one character` 를 제외한 나머지는 사용자가 건드리지 않는 한 plist 에 키 자체가 없다 — "부재 = 기본값" 판정 논리(app-bundle-analysis.md §2.1)를 그대로 따른다.

---

## 5. 엣지 케이스와 실패 모드

| # | 케이스 | 기대 동작 |
| :--- | :--- | :--- |
| 1 | ⭐⭐ **정정 — 실측 완료.** Screen Recording 권한 거부/미부여 | **"소스 A 전체가 죽는다(캡처 자체가 실패)" 는 틀렸다.** 서명된 앱으로 확인한 실제 동작(`docs/dev/manual-verification.md` 항목 8): `CGDisplayCreateImage` 는 **실패하지 않고**(8 ms 만에 반환) 이미지를 돌려주며, 그 안에는 데스크톱 배경 **그리고 메뉴 막대**가 들어 있다 — 메뉴 막대는 화면 기록 보호 대상이 아니다. 그래서 OCR 후보가 **0개가 아니라 몇 개 나온다**(실측: `Gitkraken`, `window Help`, `view` 등 5개). ⚠️ **따라서 "후보가 0개인가" 로 권한 부재를 판정하면 안 된다** — `CGPreflightScreenCaptureAccess()` 로 권한 상태를 직접 봐야 #1(권한 없음)과 #2(텍스트 없는 화면)가 구분된다. ⭐ 원본은 preflight 심볼이 없지만 **이 클론은 쓴다** — 근거는 `F-11` §1.3. |
| 2 | OCR 이 후보 0개 반환(예: 화면이 전부 이미지/영상이거나 텍스트가 없음) | 소스 A 목록이 빈 배열. 소스 B 결과와 병합해 그대로 진행. 둘 다 0개면 세션은 열려 있되 후보 없음 상태. |
| 3 | 다중 디스플레이, 해상도·배율이 서로 다름 | §3.2.3 의 디스플레이별 독립 변환(`originX/Y`, `scale` 을 디스플레이마다 별도 적용)이 정확히 이뤄져야 함. v1.55 에서 실제로 깨졌던 영역이므로 회귀 테스트 필수. §3.5 의 `[NSScreen: [VNRecognizedTextObservation]]` 화면별 저장 구조와 정합해야 한다. |
| 4 | Retina 배율(`backingScaleFactor` = 2.0 등) | §3.2.3 3단계의 픽셀→포인트 나눗셈을 누락하면 후보 프레임이 실제 텍스트보다 커 보이거나 좌표가 밀린다. |
| 5 | 대상 창이 전체화면(Full Screen) 스페이스에 있음 | 전체화면 앱은 별도 Space 에서 렌더링되며 창 레벨 취급이 다르다. 캡처 대상에 포함되는지, 오버레이가 그 위에 뜨는지는 `F-03`/캡처 API 선택에 좌우된다. `platform-constraints.md` 의 오버레이 창 레벨 판정과 함께 검증 필요. |
| 6 | 대상 앱이 Accessibility 를 지원하지 않음(예: 일부 게임, 커스텀 렌더링 엔진 앱) | 소스 B 에서 해당 앱은 요소 0개. 소스 A(OCR)만으로 그 앱 위의 텍스트를 커버해야 한다 — 이것이 AX 를 보조로 두는 이유이기도 하다. |
| 7 | 대상 앱이 AX 요청에 응답하지 않음(행업, 정지) | §3.3.3 의 `AXUIElementSetMessagingTimeout` + 깊이 제한이 없으면 Seek 전체가 블로킹된다. 타임아웃 초과 시 해당 앱은 건너뛰고 나머지 소스로 계속 진행. |
| 8 | ⭐⭐ **정정 — 실측 완료** (`docs/dev/seek-ocr-latency-spike.md`) | **① 실측치**: 전체 화면(3840×1600 + 2560×1440) `.accurate` 합계 **약 775 ms**(캡처 31 ms + OCR 744 ms). 프로세스 첫 호출은 약 120 ms 더 비싸다. **② "화면 크기에 비례" 는 틀렸다** — 6.1 Mpx 화면이 335 ms, 3.7 Mpx 화면이 448 ms 였다. 비용은 픽셀 수가 아니라 **인식된 텍스트 양**을 따른다. **③ "디스플레이별 병렬 처리로 완화 가능" 은 반증됐다** — 병렬 이득이 두 실험 모두 **정확히 0 %** 다(774→779 ms, 605→607 ms). `VNRecognizeTextRequest` 는 직렬 실행된다. 병렬은 오히려 첫 결과 도착을 335 ms → 779 ms 로 **늦춘다**. **④ 그래서 완화책은 병렬이 아니라 증분 전달이다**: 디스플레이별로 순차 OCR 하되 끝나는 대로 하나씩 내보내고, 세션 자체는 검출을 기다리지 않고 즉시 연다(결정 **S-1**·**S-6**). ⭐(이슈 #102) **"'OCR 완료 시 검색 트리거'는 이 증분 전달이 이미 실현한다** — 후보가 디스플레이별로 도착하는 즉시 세션(F-01 머신 → F-03 오버레이 세션)이 **F-02 의 필터(`filter_by_query`)를 현재 질의로 재적용**한다. 오버레이가 필터를 새로 구현하지 않는다(seek-overlay-ui.md §3.6 계약) — OCR 총 완료를 기다리지 않으며, 별도 '완료 트리거' 경로를 추가하지 않는다. |
| 9 | 세션이 열린 채로 화면 내용이 바뀜(애니메이션, 스크롤, 알림 팝업 등) | 검출은 세션 열림 시점의 단일 스냅샷을 기준으로 하는지, 주기적으로 재검출하는지가 조사 자료로 확정되지 않음 `❓미확인`. 재검출하지 않는다면 화면과 후보 위치가 어긋날 수 있다. |
| 10 | RTL 텍스트(아랍어/히브리어/페르시아어) | ⭐ 정정 — 기존 명세가 근거로 든 "번들이 8개 로케일을 지원한다"(appcast `deltaFromSparkleLocales`)는 오독이었다 — 그 속성은 Sparkle 프레임워크 자신의 로케일 파일 목록일 뿐, SuperKey UI/OCR 언어와 무관하다(app-bundle-analysis.md §5.1, §7 D1 참조). SuperKey 본체(`Base.lproj`)는 영어 단일이다. OCR `recognitionLanguages` 에 해당 언어가 없으면 인식 자체가 실패한다는 일반론과, 인식되더라도 §3.4 의 "읽기 순서" 정렬이 LTR 전제라면 RTL 언어에서 순서가 부자연스러울 수 있다는 우려는 여전히 유효하되, 그 필요성의 실측 근거는 없다. |
| 11 | 회전된 텍스트(세로쓰기, 기울어진 UI 요소) | `VNRecognizedTextObservation.boundingBox` 는 기본적으로 축 정렬(axis-aligned) 사각형을 전제한다. 회전된 텍스트는 인식률이 낮거나 프레임이 실제 텍스트를 느슨하게만 감싼다. |
| 12 | 극소 텍스트 / 특정 색상 대비(검정 배경 위 특정 파란색) / 인접 줄과 너무 가까운 텍스트 | 개발자 본인이 명시한 확인된 OCR 실패 사례 3종(§4.5 인용). v1.19 에서 "font colors/backgrounds" 처리가 개선됐으나 완전히 해소됐다는 근거는 없다. §3.2.1 의 전처리 필터(흑백·성분 추출)가 이 사례들을 겨냥한 것으로 추정된다. |
| 13 | ⭐ 정정 — 최소 지원 OS 와 캡처 API 요구 버전의 "간극"은 애초에 존재하지 않는다 | 기존 명세는 ScreenCaptureKit(12.3+) 전제로 12.0–12.2 사용자를 배제하는 결정을 내렸으나, 원본은 ScreenCaptureKit 을 쓰지 않는다(§3.2). `LSMinimumSystemVersion = 12.0` 과 실제 사용 API(`CGDisplayCreateImage`)가 정합하므로 이 간극 자체가 근거를 잃는다 — §7 D1 재검토 참조. |
| 14 | ⭐ (신규) AX 매치가 가려진(obscured) 텍스트를 잡는 경우 | AX ⓘ 팝오버 원문(§3.3)이 명시하는 알려진 한계다: "you might get matches even if you can't see the text in the element." 사용자에게는 화면에 없는 텍스트가 매치로 뜨는 것처럼 보일 수 있다. 관련 타입 `UnobscuredWindowElement`. F-03 은 이런 매치도 정상적으로 하이라이트(요소 전체)해야 한다. |
| 15 | ⭐ (신규) Stage Manager 에서 창을 순회 | `StageWindowAccessibilityElement` 가 별도로 존재하는 것으로 보아 Stage Manager 의 AX 트리 접근이 일반 창과 다르게 취급될 가능성이 있다(§3.5). 구체적으로 무엇이 다른지는 `(미확정)` — §9. |
| 16 | ⭐ (신규, 이슈 #133) 소스 C(창 제목)와 Screen Recording 권한 | `CGWindowListCopyWindowInfo` 자체는 권한 없이도 호출에 성공하지만, 권한이 없으면 `kCGWindowName`(창 제목)이 빈 문자열이 되어(§3.3.4) 이 소스는 **실질적으로 빈 결과(후보 0개)** 가 된다 — 조용한 실패. F-02 는 애초에 Screen Recording 권한을 요구·보유하므로(§1) 실제 경로에서는 제목이 읽힌다 |

---

## 6. 필요한 플랫폼 API

**소스 A (OCR·캡처)** — ⭐ 아래 캡처 API 목록은 ScreenCaptureKit 을 전제한 기존 명세를 정정한 것이다.
- `CGImage` — 캡처 결과 프레임 표현
- ⭐ **`CGDisplayCreateImage(displayID)`** / **`CGDisplayCreateImageForRect(displayID, rect)`** — 화면 캡처 본체(실측: 번들 심볼, `nm -u`). ⭐ **`CGWindowListCopyWindowInfo` 의 용도가 확정됐다(이슈 #133)** — **소스 C: 창 제목 후보 획득**(§3.3.4, `crates/ultrakey-platform/src/window_list.rs`). 기존 추정("`Only Seek in the frontmost window` 의 창 범위 판정")은 소스 C 용도가 확정된 뒤에도 §3.3.2 의 추정으로 유보해 둔다 — 같은 함수가 두 용도에 쓰일 가능성을 배제하지 않는다(전자는 실측·확정, 후자는 미확정). `CGWindowListCreateDescriptionFromArray` 는 여전히 용도 미확정.
- **CoreImage** — `CILanczosScaleTransform`(스케일) · `CIPhotoEffectMono` / `CIPhotoEffectNoir`(흑백) · `CIMaximumComponent` / `CIMinimumComponent`(성분 추출) — §3.2.1 의 전처리 파이프라인(실측: 번들 심볼)
- **Vision — 실측 확정**(승격, `(추정)` 아님): `VNImageRequestHandler(cgImage:)` · `VNRecognizeTextRequest` · `VNRecognizedTextObservation`(`boundingBox`, `topCandidates(_:)`) · `VNRecognizedText` · `VNImageRectForNormalizedRect`(정규화 좌표 → 화면 좌표 변환, §3.2.3). Swift 래퍼 `libswiftVision.dylib`. 내부 타입 `VisionManager` · `TextRecognition` · `TextRecognitionDelegate` · `RecognitionResult`(실측: 번들 심볼)
- `CGDisplayBounds(_:)` / `NSScreen.screens` — 디스플레이별 전역 원점, `backingScaleFactor`
- ⭐ **Screen Recording 권한을 명시적으로 확인하는 심볼이 없다.** `CGPreflightScreenCaptureAccess()` / `CGRequestScreenCaptureAccess()` 는 실측(`nm -u`)에서 **발견되지 않았다** — `CGDisplayCreateImage` 호출 시 OS 가 암묵적으로 요구·프롬프트하는 방식에 의존한다. 상세는 `F-11` 참조.

**소스 B (AX)**
- `AXUIElementCreateSystemWide()`, `AXUIElementCreateApplication(pid:)`
- `AXUIElementCopyAttributeValue` — `kAXRoleAttribute`, `kAXValueAttribute`, `kAXTitleAttribute`, `kAXDescriptionAttribute`, `kAXChildrenAttribute`, `kAXPositionAttribute`, `kAXSizeAttribute`, `kAXFocusedWindowAttribute`(최전면 창 판정용 — §3.3 확정에 따라 AX 는 이 속성으로 얻은 창 하나로 항상 고정)
- `AXValueGetValue` — `AXValue` → `CGPoint`/`CGSize` 언패킹
- `AXUIElementSetMessagingTimeout` — 블로킹 방지
- (권한 확인만, 상세는 `F-11`) `AXIsProcessTrusted()` — 실측으로 확인된 유일한 명시적 권한 확인 심볼(app-bundle-analysis.md §3.3)

---

## 7. 구현 접근

### ⭐⭐ 화면 캡처 API 정정 — ScreenCaptureKit 전제가 사라졌다

기존 명세는 소스 A 의 캡처 계층을 ScreenCaptureKit 기반으로 설계했다. 실측(§3.2)으로 **원본은 ScreenCaptureKit 을 전혀 쓰지 않고 `CGDisplayCreateImage`/`CGDisplayCreateImageForRect` 만 쓴다**는 것이 확정됐다. 이것이 바꾸는 것:

- **소스 A — Vision OCR**: `objc2-vision`(0.3.2)이 `VNImageRequestHandler`/`VNRecognizeTextRequest`/`VNRecognizedTextObservation` 을 헤더 자동 생성 바인딩으로 그대로 노출한다(`rust-macos-capability-notes.md` §1.1, §2.4) — 이 부분은 캡처 API 선택과 무관하므로 그대로 유효하다.
- ⭐⭐ **캡처 크레이트 커버리지 — `(미확정)` 해소, 원시 FFI 불필요.** **`objc2-core-graphics`(0.3.2, 이 저장소가 이미 쓰는 크레이트)가 `CGDisplayCreateImage` 와 `CGDisplayCreateImageForRect` 를 그대로 노출한다.** 둘 다 기본 기능 `CGDirectDisplay` + `CGImage` 에 들어 있고 `Option<CFRetained<CGImage>>` 를 돌려주는 안전한 래퍼다(`#[deprecated]` 는 붙어 있다 — `platform-constraints.md` P9). 함께 필요한 `CGGetActiveDisplayList` · `CGDisplayBounds` 도 같은 크레이트에 있다. ⭐ **`CGPreflightScreenCaptureAccess` / `CGRequestScreenCaptureAccess` 도 노출된다**(`CGWindow` 기능) — F-11 영향은 `permissions-onboarding.md` §1.4 참조. 즉 `core-graphics`/`core-graphics-sys` 를 새로 들이거나 원시 `extern "C"` 를 선언할 이유가 **전혀 없다**. 확인 방법: 크레이트 소스 대조 + **실제 컴파일**(`crates/ultrakey-platform/src/screen_capture.rs`).
- ⭐⭐ **CoreImage 크레이트 커버리지 — `(미확정)` 해소.** **`objc2-core-image`(0.3.2)** 로 전부 커버된다: `CIImage::initWithCGImage` → `CIImage::imageByApplyingFilter(_withInputParameters)` → (필요 시) `CIContext::createCGImage_fromRect`. §3.2.1 의 필터 5종은 전부 CoreImage **내장 필터 이름 문자열**이므로 필터별 바인딩이 따로 필요 없다. ⭐ 게다가 `VNImageRequestHandler` 에 **`initWithCIImage:options:`** 가 있어 `CIContext` 왕복(GPU→CPU 래스터화)을 통째로 건너뛸 수 있다 — 이 구현은 그 경로를 쓴다(`crates/ultrakey-platform/src/image_preprocess.rs`). 확인 방법: 크레이트 소스 대조 + 실제 컴파일.
- `CGImage` ↔ Vision 연결과 좌표 변환(§3.2.3)은 여전히 순수 Rust 로직으로 작성 가능하고, 네이티브 shim 은 필요 없다는 결론 자체는 유지된다.
- **소스 B — Accessibility 파싱**: 이 절은 캡처 API 선택과 무관하므로 정정 대상이 아니다. `axuielement`(0.9.1, "현재 가장 활발")가 `AXUIElement`/`AXValue`/`AXAttribute`/`ProcessTrust` 를 안전한 고수준 API 로 감싼다(`rust-macos-capability-notes.md` §1.2, §2.2). 필요한 속성(`kAXPositionAttribute` 등) 조회와 `AXUIElementPerformAction` 은 커버된다.
- ⭐⭐ **`AXUIElementSetMessagingTimeout` — `❓미확인` 해소(Q-f). `axuielement` 0.9.1 이 이미 노출한다.** 안전 래퍼 `AXUIElement::set_timeout(&self, timeout_seconds: f32) -> Result<(), AXError>` 가 그대로 `AXUIElementSetMessagingTimeout` 을 감싸고(`src/ax_ui_element.rs`), 기본 기능 `raw-ffi` 를 켜면 원시 선언(`ffi::AXUIElementSetMessagingTimeout`)도 함께 노출된다. ⭐ **따라서 `accessibility-sys`(0.2.0)를 들일 이유가 없다** — 의존성 하나를 아꼈다. 확인 방법: 크레이트 소스 대조 + 실제 컴파일.
- **⭐ 신규: 캡처 이미지 전처리(CoreImage)** — 위에서 확정. `objc2-core-image`(0.3.2) 하나로 끝난다.

> ⭐ **세 `(미확정)` 이 모두 "이미 이 저장소가 쓰거나 objc2 계열 크레이트가 커버한다" 로 닫혔다.** 새로 들인 의존성은 `objc2-core-image` · `objc2-vision` · `axuielement` **셋뿐**이고, 원시 `extern "C"` 선언은 **한 줄도 늘지 않았다**(`platform-constraints.md` P6 의 시그니처 추측 위험에 노출되지 않는다).

### ⭐⭐ 최소 macOS 버전 결정(D1) — 근거가 사라졌다, 재검토 필요

기존 명세(§7 구판)는 "제품 공식 최소 버전은 12.0 이지만 채택한 캡처 API(ScreenCaptureKit)는 12.3+ 에서만 동작한다"는 간극을 근거로 **최소 버전을 12.3 으로 상향**하기로 결정했고, 이 결정은 `docs/spec/README.md` 의 제품 결정 **D1** 으로 기록되어 있다.

⭐ **이 간극 자체가 존재하지 않는다.** 원본이 실제로 쓰는 API 는 `CGDisplayCreateImage` 계열이고, `LSMinimumSystemVersion = 12.0` 과 정확히 정합한다(app-bundle-analysis.md §1, §3). 즉 **D1 이 상정한 전제(ScreenCaptureKit 필요 → 12.3 상향 불가피)가 사실이 아니었다.** D1 은 근거를 잃는다 — README 자체의 수정은 다른 담당 소관이지만, 이 사실은 여기 명시한다.

**트레이드오프 재평가 — 클론이 어떻게 볼 것인가.** 확인된 사실:
- `CGDisplayCreateImage` 는 **macOS 14 에서 deprecated** 다(Apple 공식 문서 기준, 이 조사가 직접 실측한 것은 아니고 일반적으로 알려진 사실).
- 그럼에도 **v1.66(2026-06 빌드, SDK macosx26.5 로 빌드)이 지금도 이 API 를 그대로 쓰고 있다** — 즉 벤더 자신이 최신 SDK 로 빌드하면서도 deprecated API 교체를 미루고 있다(실측: 번들 심볼).

이 사실로부터 두 갈래 판단이 가능하다:
1. **원본과 동일하게 `CGDisplayCreateImage` 를 채택하고 최소 버전을 12.0 으로 유지한다.** 근거: 벤더가 실제로 이 API 로 5년 가까이(v1.66 은 2026년 빌드) 문제없이 배포해왔다는 것 자체가 "deprecated 여도 당장 제거되지는 않는다"는 실증이다. 클론의 목표가 원본과의 행동 동등성이라면, 굳이 더 넓은 OS 지원 범위(12.0)를 포기하면서까지 더 복잡한 API(ScreenCaptureKit, 비동기 델리게이트 기반)로 미리 옮겨갈 이유가 약하다.
2. **ScreenCaptureKit 으로 미리 옮기고 최소 버전을 12.3 으로 올린다.** 근거: `CGDisplayCreateImage` 가 실제로 제거되는 시점이 오면 그때 가서 마이그레이션하는 비용이, 지금 한 번에 옮기는 비용보다 클 수 있다. 특히 macOS 26(2026년 현재 최신 메이저)까지 나온 시점에서 12.0–12.2 사용자 비율은 극히 작을 것으로 추정된다(추정, 실측 데이터 없음).

**이 문서는 1안(원본과 동일하게 `CGDisplayCreateImage` 채택, 최소 버전 12.0 유지)쪽으로 판단을 기운다** — 원본이 이미 이 트레이드오프를 감수하고 실제 배포로 검증했다는 사실이 가장 강한 근거이기 때문이다. 다만 이는 **이 명세 저자의 판단이며 제품 요구사항 확정이 아니다.** 최종 결정은 README.md D1 담당자의 재검토가 필요하다 — §9.

---

## 8. 수용 기준

- [ ] Screen Recording 권한이 있을 때, 화면에 보이는 텍스트에 대해 소스 A(OCR)가 텍스트+화면 좌표 후보를 생성한다. `Seek using macOS accessibility` 를 끄는 것과 무관하게 소스 A 는 항상 동작한다(끄는 UI 컨트롤이 없다).
- [ ] `Seek using macOS accessibility` 가 켜져 있을 때, 소스 B(AX)가 **최전면 창의** 텍스트 요소에서 텍스트+화면 좌표(요소 전체) 후보를 생성한다. `Only Seek in the frontmost window` 값과 무관하게 이 범위는 항상 최전면 창이다.
- [ ] `Seek using macOS accessibility` 가 꺼져 있을 때, 소스 B 는 후보를 생성하지 않고, `Match on more than one character` 체크박스도 UI 에서 완전히 사라진다(hidden).
- [ ] 소스 A 의 `VNRecognizedTextObservation.boundingBox`(정규화, 좌하단 원점)가 §3.2.3 의 수식을 거쳐 소스 B 와 동일한 좌표계(전역, 좌상단 원점, 포인트)로 정확히 변환된다.
- [ ] 서로 다른 해상도·배율(`backingScaleFactor`)을 가진 2개 이상의 디스플레이에서, 각 디스플레이의 후보 좌표가 올바른 전역 위치를 가리킨다(디스플레이별 원점·배율이 독립적으로 적용됨).
- [ ] `Match on more than one character` 가 켜져 있을 때(정수 `minAxCharCount = 2`), 소스 B 의 1글자 텍스트 요소가 후보 목록에서 제외된다.
- [ ] `Only Seek in the frontmost window` 가 켜져 있을 때, 소스 A(OCR)의 캡처·인식 범위가 최전면 창으로 좁혀진다. 소스 B(AX)의 범위는 이 설정과 무관하게 항상 최전면 창이었으므로 변화가 없다.
- [ ] 소스 A 와 소스 B 가 겹치는 위치에서 동일 텍스트를 각각 검출했을 때, 병합 결과에서 **OCR(소스 A) 매치가 AX(소스 B) 매치를 대체**하며 단일 후보만 남는다(M2/M3 규칙).
- [ ] AX 전용(OCR 과 겹치지 않는) 매치는 요소 전체 사각형으로 하이라이트되고, 가려진(obscured) 텍스트라도 후보로 나타날 수 있다(§5 #14).
- [ ] Screen Recording 권한이 없을 때 소스 A 결과가 빈 목록이 되며, 세션 자체는 크래시 없이 유지된다.
- [ ] 응답하지 않는 앱이 대상 화면에 있을 때, 소스 B 순회가 전체 파이프라인을 무기한 블로킹하지 않고 타임아웃 후 진행된다.
- [ ] OCR 결과가 0개인 화면(순수 이미지 등)에서도 세션이 오류 없이 후보 0개 상태를 반환한다.
- [ ] 병합된 `CandidateSet` 에 질의 문자열을 적용했을 때, §3.6 의 매칭 규칙(대소문자 무시·부분 문자열·공백 정규화)에 따라 일치하는 후보만 남는다.
- [ ] ⭐(이슈 #102) 디스플레이별 후보가 도착하는 즉시 현재 질의로 필터링된 후보가 표시된다(F-02 `filter_by_query` 의 재적용 — OCR 총 완료를 기다리지 않는다, §5 #8).
- [ ] ⭐(이슈 #133, 소스 C) `Search window titles` 가 켜져 있고 Screen Recording 권한이 있을 때, 화면에 **가려진(obscured) 창**의 제목이 후보 목록에 나타난다(§3.3.4).
- [ ] ⭐(이슈 #133, §3.4 M4) 같은 창 bounds 안에 같은 정규화 텍스트의 가시(OCR/AX) 후보가 있으면 창 제목 후보는 중복 제거되고, 제목이 같은 **서로 다른 창**은 둘 다 유지된다.
- [ ] ⭐(이슈 #133, 소스 C) `Search window titles` 가 꺼져 있으면 소스 C 는 후보를 하나도 생성하지 않는다.

---

## 9. 미해결 질문

| # | 질문 | 상태 / 왜 확정 못 했는가 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| ~~Q-a~~ | ~~OCR 을 사용자가 끌 수 있는가~~ — ⭐ **해소**. 끌 수 있는 UI 컨트롤이 없다(실측: AX 트리, §1·§3.1) | app-bundle-analysis.md §6.1 | 해소됨 |
| ~~Q-b~~ | ~~병합 충돌 시 우선순위~~ — ⭐ **해소**. **OCR 이 AX 를 대체**한다(ⓘ 팝오버 원문, §3.3·§3.4 M3). 기존의 "AX 우선" 추정은 정반대로 틀렸었다 | app-bundle-analysis.md §5.2 | 해소됨 |
| Q-c | 후보 정렬 순서가 읽기 순서인지 커서 거리 기준인지(§3.4) | 개발자 포스트는 순환 방향(↑↓/Tab)만 언급, 정렬 기준 언급 없음. AX/OCR 관계와 달리 이 부분은 이번 실측 대상이 아니었다 | 실제 앱에서 다중 매치 화면 관찰 |
| Q-d | 질의 매칭 규칙(대소문자·부분/접두사·공백) 전체(§3.6) | 조사 자료에 매칭 알고리즘 서술 없음 | 실제 앱 동작 관찰 |
| ~~Q-e~~ | ~~`Only Seek in the frontmost window` 의 적용 범위~~ — ⭐ **해소**. AX(소스 B)는 이 설정과 무관하게 항상 최전면 창 한정, 이 설정은 OCR(소스 A)의 캡처 범위만 좁힌다(ⓘ 팝오버 원문, §3.3.2) | app-bundle-analysis.md §5.2 | 해소됨 |
| ~~Q-f~~ | ~~`AXUIElementSetMessagingTimeout` 이 `axuielement` 0.9.1 에 노출되는가~~ — ⭐ **해소. 노출된다.** 안전 래퍼 `AXUIElement::set_timeout(f32)` 가 그대로 그 함수를 감싼다(`src/ax_ui_element.rs`), 기본 기능 `raw-ffi` 로 원시 선언도 함께 나온다. `accessibility-sys` 는 불필요 — 의존성 하나를 아꼈다(§7) | 크레이트 소스 대조 + 실제 컴파일 | 해소됨 |
| Q-g | 세션이 열려 있는 동안 화면 변화에 대해 재검출을 수행하는가, 최초 스냅샷 고정인가(§5 #9) | 조사 자료에 언급 없음 | 실제 앱에서 세션 도중 화면 변화 관찰 |
| Q-h | `usesLanguageCorrection`, `minimumTextHeight` 의 **원본이 쓴 값** | 벤더가 공개하지 않음 — 심볼 존재는 확정됐으나(§3.2.2, §6) 원본이 고른 값은 여전히 확인 못했다. ⭐ **`recognitionLanguages` 부분은 이 항목에서 빠진다** — 원본이 무엇을 골랐는지와 무관하게 **이 클론이 골라야 할 값**이 실측으로 확정됐다(§3.2.2, 결정 S-4) | 소스 코드 확인 |
| Q-i | ⭐ 정정된 형태로 재상정 — **최소 macOS 버전을 12.0 으로 유지할지 12.3 으로 올릴지**, README.md D1 의 재검토 | 기존 D1 의 전제(ScreenCaptureKit 필요)가 근거를 잃었다(§7). 이 문서는 12.0 유지(원본과 동일 API 채택) 쪽으로 판단을 기울였으나 최종 결정은 아니다 | README.md D1 담당자 재검토 |
| Q-j | 번들 로케일과 `recognitionLanguages` 매핑 여부, RTL 언어에서의 후보 정렬 | ⭐ 정정 — "번들이 8개 로케일을 지원한다"는 기존 근거(appcast `deltaFromSparkleLocales`)는 오독이었다(§5 #10, app-bundle-analysis.md §5.1). SuperKey 본체는 영어 단일이므로 애초에 UI 로케일과 OCR 인식 언어를 연결할 근거가 사라졌다 — OCR 인식 언어 자체를 별도로 어떻게 정하는지는 여전히 `(미확정)` | 소스 코드 확인 또는 다국어 화면 실측 |
| Q-k | ⭐ **부분 해소** — §3.2.1 전처리 필터의 선택 조건 | **원본의 선택 로직은 여전히 미확인**(내부 타입 이름만 실측). 다만 **이 클론의 기본값은 실측으로 정해졌다**: 일반적인 데스크톱 화면에서 프리셋별 후보 집합의 차이가 거의 전부 **같은 텍스트의 다른 오인식**이었고 평균 신뢰도는 Noir 가 오히려 가장 낮았다 → 기본값 `None`(결정 **S-3**, `docs/dev/seek-ocr-latency-spike.md` §4). ⚠️ 그 표본에 §5 #12 의 실패 사례(극소 텍스트·저대비 색상)가 **없었으므로**, 프리셋이 그 사례에서 유효한지는 여전히 미확인이다 | 저대비·극소 텍스트를 의도적으로 만든 화면으로 재측정 |
| Q-l | ⭐ (신규) `minAxCharCount` 를 `2`/`1` 외의 값으로 설정하는 경로가 있는가 (§3.3.1) | UI 는 체크박스 하나뿐이라 두 값만 관찰됐고, 다른 값을 만드는 경로(설정 파일 직접 편집 외)를 찾지 못했다 | 소스 코드 확인, 또는 `defaults write` 로 임의 값을 넣었을 때의 UI/동작 관찰 |
| ~~Q-m~~ | ~~전체 화면 OCR 지연의 실측치~~ — ⭐⭐ **해소. `docs/dev/seek-ocr-latency-spike.md`** | **전체 화면(3840×1600 + 2560×1440) `.accurate` = 약 775 ms**(캡처 31 ms + OCR 744 ms), 콜드 첫 호출 +120 ms. `.fast` 는 241 ms. 함께 확정된 것: 비용은 화면 크기가 아니라 **텍스트 밀도**를 따르고, **Vision 은 병렬화되지 않는다**(이득 0 %). ⚠️ **Retina(`scale=2.0`)와 저사양 기기는 측정하지 못했다** — 그 두 가지는 스파이크 문서 §6 에 미확인으로 남았다 | 해소됨 (M4 Pro / macOS 26.5.2 / 1× 디스플레이 2대) |
| Q-n | ⭐ (신규) Stage Manager 에서 `StageWindowAccessibilityElement` 가 일반 AX 처리와 정확히 무엇이 다른가 (§3.5, §5 #15) | 타입 이름만 실측, 동작 상세는 확인 못함 | Stage Manager 를 켠 상태에서 실제 앱 동작 관찰, 또는 소스 코드 확인 |
| Q-o | ⭐ (신규, 이슈 #133) 소스 C — 창 제목 후보의 **획득 경로·중복 제거 규칙·확정 동작** (§3.3.4·§3.4 M4 · `seek-click-execution.md` §3.7) | **결정됨(이슈 #133)** — 획득 경로 a(`CGWindowListCopyWindowInfo(OptionOnScreenOnly)`), 중복 규칙(같은 정규화 텍스트의 가시 후보가 창 bounds 안에 있으면 제거 — 소스 C 끼리는 유지), 확정 시 **창 전면화**(클릭 합성 없음). 근거·기각 대안: 이슈 #133 결정 코멘트(comment-5547504824) | 해소됨 |
