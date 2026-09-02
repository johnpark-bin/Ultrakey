# F-09 · 환경설정 UI

> **한 줄 요약**: 좌측 사이드바(`Seek`·`Hyperkey`·`Presets`·`General` 4탭, 각각 AX 상 토글 버튼) + 우측 패널로 구성된 환경설정 창의 구조·컨트롤 종류·저장 방식·엔진으로의 즉시 반영 계약을 정의한다. 개별 설정의 의미는 다루지 않는다 — 이 문서는 "창 자체"의 명세다.
> **의존성**: 이 문서가 정의하는 저장·즉시반영 계약은 `F-07`(key-remapping-engine — 단일 리매핑 엔진·우선순위 중재)이 소비자다. `Seek` 탭의 개별 설정은 `F-01`~`F-04`, `Hyperkey` 탭(hyper/meh/bleh + 트랙패드 제스처)은 `F-05`·`F-06`, `Presets` 탭(16종)은 `F-08`, `General` 탭은 `F-10`~`F-13`이 소유한다. 권한 상태 표시는 `F-11`(permissions-onboarding.md)의 온보딩 흐름과 연동된다. 설정 충돌 감지 대화상자·저장소 무결성의 상세 명세는 `F-15`(settings-store-and-integrity.md, 별도 담당)가 소유한다 — 이 문서는 그 대화상자가 UI 표면(4개 탭 중 어디서 뜨는지)으로서 실재한다는 사실만 다룬다.
> **관련 명세**: 개별 설정의 동작 의미 → 각 기능 명세(§3.1, §4 표에서 지시). 오버레이 창(Seek 검색 바) → `F-03`(seek-overlay-ui.md). 메뉴바 상주·환경설정 창을 여는 경로 → `F-10`. 리매핑 엔진 자체 → `F-07`(key-remapping-engine.md). 권한 획득·상태 표시 → `F-11`. 현지화 → `F-14`(localization-and-input-sources.md). 플랫폼 역량 종합 판정 → `platform-constraints.md`.
> **1차 근거**: [`../research/app-bundle-analysis.md`](../research/app-bundle-analysis.md) — 실제 설치된 SuperKey v1.66(66)의 번들 정적 분석 + 실행 중 AX 트리 실측. 이 문서의 `(실측: ...)` 표기 전부가 그 문서를 가리킨다(등급 정의는 그 문서 §0).

---

## 1. 개요

환경설정 창은 Superkey 의 모든 설정을 담는 유일한 UI 표면이다. `NSMainStoryboardFile = Main`(`AppKit` + Storyboard, WebView 아님 — 실측: 번들 문자열, app-bundle-analysis.md §1)로 구현되어 있으며, 창은 **좌측 사이드바(탭 목록) + 우측 패널(선택된 탭의 컨트롤들)** 구조다. 탭은 `Seek` · `Hyperkey` · `Presets` · `General` 4개이고, AX 트리 상 각 탭은 **토글형 `AXCheckBox`**(체크박스가 아니라 라디오처럼 동작하는 선택 버튼)로 구현되어 있으며 이 순서가 AX 실측·스크린샷 4장 모두에서 일관되게 확인된다(실측: AX 트리, 02~05-*.png).

⭐ **창 크기는 탭마다 다르며, 탭 전환 시 창이 내용에 맞게 리사이즈된다**(실측: AX 트리) — `Seek` 555×378pt · `Hyperkey` 710×517pt · `Presets` 825×527pt · `General` 613×273pt. 고정 크기 창이 아니다. 이는 §3.3·§5·§6 전반에 영향을 준다(다중 디스플레이 위치 복원 로직이 창 크기 변화를 전제해야 한다).

> ⛔ **클론은 이 동작을 따르지 않는다 (2026-08-30, 이슈 #32).** 위 문단은 **원본의 실측 사실**이므로 그대로 둔다 — 클론이 다르게 가기로 한 것이지, 실측이 틀린 것이 아니다. 클론은 **단일 고정 크기 + 사용자가 조절한 크기 영속**을 택했다. 근거와 기각한 대안은 §3.3 의 "탭 전환 시 리사이즈" 항목에 있고, 갈라짐 자체는 [원본과 갈라지는 지점](README.md#-원본과-갈라지는-지점-divergence) **D5** 로 등재했다.

이 문서가 다루는 것은 **창의 골격**이다 — 어떤 탭이 있고, 각 탭에 어떤 종류의 컨트롤(체크박스, 팝업, 슬라이더 등)이 등장하며, 컨트롤을 조작했을 때 값이 어디에 저장되고 언제 엔진에 반영되는가. 각 설정이 *무엇을 하는지*는 그 설정을 소유하는 개별 기능 명세의 몫이다.

개발자 본인이 명시한 설계 원칙(`superkey-inventory.md` §4.5)이 이 문서의 핵심 제약이다:

> "Superkey will follow the same **simple checkbox-centered configuration**"
> "There's definitely a tradeoff between offering complex entirely custom key remappings versus the canned presets here, but **I'll always prefer just checking a box over messing around with complicated preferences construction**"

즉 환경설정 UI 는 **고정된 항목의 열거**이지, 사용자가 임의 규칙을 조합·저장하는 규칙 편집기가 아니다. 이번 실측(app-bundle-analysis.md §6)이 이 원칙을 정확히 확인해 준다 — `Presets` 탭 16개 항목 전부가 체크박스(+ 최대 팝업 하나)로 끝나며, 자유 텍스트 규칙 입력 필드는 어디에도 없다. 클론도 "새 항목을 추가하는 규칙 빌더"를 만들지 않는다.

## 2. 사용자 시나리오

### 시나리오 A — 환경설정을 열고 Seek 리매핑 키를 바꾼다

1. 사용자가 메뉴바 아이콘(`F-10` 소유)을 클릭해 `Settings…` 를 선택한다(실측: 번들 문자열 — 메뉴바 메뉴 전량, app-bundle-analysis.md §6.5. ⭐ 항목 라벨이 `Settings…` 로 확정됐다 — 기존 명세가 `(추정)` 으로 썼던 `Preferences…` 가 아니다. `menu-bar-and-lifecycle.md`(F-10)가 별도로 갱신해야 할 대상이다).
2. 환경설정 창이 열리고 `Seek` 탭(555×378pt)이 선택된 상태로 나타난다. 이전에 마지막으로 보고 있던 탭이 다시 열리는지는 관찰 중 항상 같은 탭이었다는 정황뿐이라 `(미확정)` → §9.
3. `Remap key to Seek:` 팝업 버튼을 클릭해 `-`(미설정) 대신 `right option` 을 선택한다(35종 선택지 중 하나, §4.1).
4. 선택 즉시(별도 "적용" 버튼 없이 — 실측: AX 트리, 4개 탭 어디에도 Apply/OK 버튼 없음이 확인됨, §3.7) 값이 저장되고, `F-07`(리매핑 엔진)에 새 매핑이 반영된다.
5. 사용자가 창을 닫는다(빨간 stoplight 버튼 또는 `⌘W`). 엔진은 계속 백그라운드에서 동작한다(§5).

### 시나리오 B — 단축키 레코더에서 이미 점유된 조합을 입력한다

1. `Seek` 탭에서 `Toggle Seek with shortcut:` 필드를 클릭해 레코딩을 시작한다(§3.5). 미설정 상태의 기본 표시는 `Record Shortcut`(실측: 번들 문자열 — `KeyboardShortcuts` 패키지의 표준 placeholder)이다.
2. 사용자가 `⌘Space`(macOS 기본 Spotlight 단축키)를 누른다.
3. 레코더가 이 조합이 시스템 단축키와 충돌함을 인지하고 경고를 표시하는지는 조사로 확인되지 않았다 `(미확정)` → §9. `KeyboardShortcuts` 패키지 자체는 시스템 예약 단축키 감지 기능을 제공하지만, SuperKey 가 이를 활성화했는지는 관찰하지 못했다(실측 부작용이 커 재현하지 않음, app-bundle-analysis.md §8 항목 7 참조).
4. 사용자가 `Esc` 를 눌러 레코딩을 취소하거나, 다른 조합을 다시 눌러 레코딩을 이어간다.

### 시나리오 C — Presets 탭에서 서로 다른 그룹의 설정을 다수 조작한다

1. 사용자가 `Presets` 탭으로 전환한다. 창이 825×527pt 로 리사이즈된다(실측: AX 트리). 캡스락/시프트/삭제/기타 4개 그룹이 순서대로 배치되어 있다(§4.3).
2. `Quick press duration` 슬라이더를 드래그해 값을 `1000 ms`(출고 기본값, 실측: AX 트리)에서 `600 ms` 로 낮춘다. 슬라이더 범위는 **최소 250ms · 최대 2000ms** 로 확정됐다(실측: AX 트리) — 눈금 간격(step)은 여전히 `(미확정)` → §9.
3. `Caps lock +` [팝업: `H J K L` / `I J K L`] ` = ◀▼▲▶` 체크박스를 켠다. 팝업은 라벨 문장 중간에 삽입되어 있다(§3.8 "문장 중간 삽입" 규약).
4. 각 변경은 개별적으로 즉시 저장·반영된다(§3.7) — 탭을 벗어나거나 창을 닫을 때 일괄 저장하는 방식이 아니다. 이는 이제 `(추정)` 이 아니라 **확인**이다: AX 트리에 Apply 류 버튼이 없고, 실측 중 개별 토글마다 plist 에 해당 키가 즉시 기록되는 것을 확인했다(실측: defaults, app-bundle-analysis.md §7.1).

### 시나리오 D — 서로 충돌하는 caps lock 설정을 동시에 켠다 `(F-15 소관, 배치만 확인)`

1. 사용자가 `Presets` 탭에서 `Caps lock + W A S D` 를 켠 상태에서 `Caps lock + [H J K L]` 도 켜려 한다.
2. 대화상자 "Conflict: Caps lock arrows"·"Disable that setting and enable WASD arrows?" 류 문구가 뜬다(SuperKey 원문 — 실측: 번들 문자열, app-bundle-analysis.md §4.1). 정확한 버튼 구성·문구 전체·재현 조건은 `F-15`(settings-store-and-integrity.md) 소관이며, 이 문서는 "그런 대화상자가 실재한다"는 사실만 확인한다.
3. 사용자가 확인하면 상대 설정이 꺼지고 새 설정이 켜진다 — 즉 원본은 **UI 레벨 하드 차단이 아니라 사용자에게 묻고 상대를 꺼주는 대화형 해소**를 택했다.

## 3. 동작 명세

### 3.1 탭 구조

| 탭 | 컨트롤 개수(§4) | 창 크기(실측: AX 트리) | 소유 명세 ID |
| :--- | :--- | :--- | :--- |
| `Seek` | 상위 8개 + 조건부 1개 = 9개 | 555 × 378 pt | `F-01`·`F-02`·`F-03`·`F-04` |
| `Hyperkey` | 상위 6개 + 조건부 2개 = 8개 | 710 × 517 pt | `F-05`·`F-06` |
| `Presets` | 16개 + 조건부 2개 = 18개 | 825 × 527 pt | `F-08` |
| `General` | 7개 컨트롤 + 조건부 1개 | 613 × 273 pt | `F-10`·`F-11`·`F-12`·`F-13` |

⚠️ **이 표의 창 크기 열은 원본의 실측 기록이며, 클론의 창 크기가 아니다 (2026-08-30, 이슈 #32).** 클론은 탭마다 창을 바꾸지 않고 **825 × 821 pt 하나**로 연다(§3.3). 그 값은 클론의 마크업을 실제로 렌더해 잰 것이지 위 표에서 고른 것이 아니다 — 가장 큰 탭(`Presets`)의 패널 내용이 749 px 이고, 여기에 패널 상하 패딩 40 px 과 창 크롬 32 px 을 더했다(실측: 2026-08-30, 창 너비 825 에서 `#panel-presets` 의 `scrollHeight`). 원본의 825 × 527 을 그대로 쓰면 이 탭의 내용이 잘린다.

⭐ **F-05/F-06/F-08 의 소유 배정이 정정됐다.** 이전 판(이 문서의 구판)은 `Hyperkey` 탭의 `Engage hyper key using trackpad:` 를 `F-08(추정)` 으로, `Presets` 탭 전체를 `F-06(추정)` 으로 잘못 배정했다. 실제로는 `trackpad-hyper-gesture.md` 가 `F-06`, `power-user-presets.md` 가 `F-08` 로 이미 확정되어 있다(두 문서 모두 자체 헤더에서 이 ID 로 서로를 참조한다) — 이 문서의 구판에 있던 ID 혼동을 여기서 바로잡는다.

각 탭 버튼은 AX 상 `AXCheckBox`(라디오 그룹처럼 동작하는 토글)이며, 탭마다 라벨 **왼쪽에 아이콘**이 붙는다.

#### ⭐ 탭 아이콘 (이슈 #40 ②)

**원본 실측** — 구판은 "아이콘 존재는 AX 로 확인, 구체적 모양은 확인하지 못했다 `(미확정)`" 라고 썼으나, 이미 저장소에 있던 v1.66 스크린샷 4장이 그 모양을 그대로 담고 있었다. 눈으로 읽어 확정한다(실측: `docs/research/screenshots/02-seek-tab.png`·`03-hyperkey-tab.png`·`04-presets-tab.png`·`05-general-tab.png`):

| 탭 | 원본 아이콘 모양(실측) |
| :--- | :--- |
| `Seek` | 십자선 조준경 — 원 하나에 상하좌우 눈금 |
| `Hyperkey` | 구체 + 안쪽 초승달 하이라이트 |
| `Presets` | 원반(디스크) 더미를 위에서 비스듬히 본 모양 |
| `General` | 4각 반짝임(sparkle) — 큰 것 하나 + 작은 것 하나 |

전부 **단색 선화**이고 라벨 왼쪽에 놓이며, 선택된 탭에서 라벨과 함께 강조색으로 물든다.

**클론의 구현 방식 — 확정: 이모지가 아니라 인라인 SVG 선화.** 사용자 요청은 "최소한 이모지 형태로라도 … 흑백 형태로라도"였으므로 이모지도 허용 범위였지만, 이 저장소에서는 SVG 가 더 싸고 더 정확하다:

1. **`stroke: currentColor` 하나로 다크/라이트가 끝난다.** 이 창은 이미 `color-scheme: light dark` + `Canvas`/`CanvasText` 시스템 색만 쓴다(§3.4). 아이콘도 같은 전경색을 상속하면 테마별 자산이 필요 없다. 같은 규약을 `index.html` 의 `<svg class="app-icon">` 과 `overlay-searchbar.html` 의 돋보기 아이콘이 이미 쓰고 있다 — **새 규약이 아니라 있는 규약의 적용**이다.
2. **PR #18 의 아이콘 자산 규약(`docs/dev/icons.md`)과 충돌하지 않는다.** 그 규약이 금지하는 것은 **수작업 PNG** 이고, 요구하는 것은 "래스터는 정본 SVG 에서 스크립트로 생성한다"이다. 탭 아이콘은 WebView 안에서 벡터 그대로 그려지므로 **래스터가 아예 만들어지지 않는다** — `generate-icons.sh` 에 넣을 산출물이 없다. 대신 형태 어휘는 정본과 맞춘다: `viewBox` 24×24 · `fill="none"` · `stroke-width="2"`(`icons.md` §2 가 정본 SVG 에 대해 기술한 값 그대로).
3. **이모지는 흑백이 되지 않는다.** macOS 의 Apple Color Emoji 는 컬러 폰트라 `color` 로 단색화할 수 없다. 사용자가 "흑백 형태로라도"라고 한 요구를 이모지로는 오히려 만족시키지 못하고, 원본의 단색 선화와도 갈라진다. 자소 폭·베이스라인도 글리프마다 달라 6개가 나란히 섰을 때 정렬이 흔들린다.
4. **번역 대상이 되지 않는다.** 이모지를 라벨 문자열에 넣으면 `resources/i18n/{en,ko}.json` 의 값에 섞여 들어가 카탈로그가 표현이 아니라 장식을 나르게 된다. SVG 는 마크업에 있으므로 문자열 카탈로그를 건드리지 않는다 — 이 변경으로 **새 i18n 키가 하나도 생기지 않는다**.

**기각한 대안 — SF Symbols**: macOS 네이티브 앱이라면 첫 선택지지만, 이 창은 WebView 다. SF Symbols 는 시스템 폰트 스택으로 노출되지 않아 웹 콘텐츠에서 쓸 수 없고, 라이선스도 Apple 플랫폼 UI 밖 사용을 제한한다.

`Korean`·`Keyboards` 두 탭은 원본에 없는 클론 고유 탭이므로(F-16·F-17, `../spec/README.md` 갈라짐 표) 원본 실측이 존재하지 않는다 — 위 네 개와 같은 어휘로 새로 그렸다: `Korean` 은 한글 글자 `가` 의 획, `Keyboards` 는 키보드 외곽선 + 키 점. ⛔ **이 절은 `../spec/README.md` 의 갈라짐 표에 새 행을 만들지 않는다** — 원본에도 탭 아이콘이 있으므로 이것은 갈라짐이 아니라 **원본에 맞추는 것**이다. 패널 상단에는 탭 이름과 ⓘ 정보 버튼이 함께 표시되는 것은 `Seek`·`Hyperkey` 뿐이다(실측: AX 트리 — `SeekInfoViewController`·`HyperkeyInfoViewController` 두 개만 패널 제목 옆에 있고, `Presets`·`General` 패널 제목에는 ⓘ 가 없다. 기존 판의 `(추정 — 일관성상 있을 가능성)` 을 정정한다).

⭐⭐ **현대화 — 선택 상태·탭 전환 규약 (이슈 #79, UXR-01·UXR-07·UXR-04)**:

- **선택 상태의 색 온도 (UXR-01)**: 사이드바 선택 탭(`button[aria-selected="true"]`)과 Keyboards 디바이스 선택 행(`[role="option"][aria-selected="true"]`)의 채움을 `currentColor 14%` → **`color-mix(in srgb, Highlight 12~16%, transparent)`** 로 바꾼다 — hover(8% currentColor)와 선택이 **색 온도로도 구분**된다. `Highlight` 는 시스템 강조색 키워드라 테마별 자산이 없고 라이트/다크에서 accent 색이 자동으로 따라온다(기존 `color-scheme: light dark` 골격 그대로). `aria-selected`·`data-tab`·컨트롤 id·`font-weight` 는 **무변경** — CSS 선택기 값만 바뀐다.
- **탭 전환 패널 페이드 (UXR-07)**: 탭을 바꾸면 패널이 **90~110ms opacity 페이드 인**으로 나타난다. `[hidden]`(display:none) 에서는 transition 이 켜지지 않으므로, `hidden` 해제 직후 패널에 인라인 `opacity: 0` 을 세팅하고 reflow 를 강제한 뒤 `requestAnimationFrame` 1프레임에서 인라인 opacity 를 제거해 `transition: opacity 110ms` 가 페이드 인되도록 한다. `prefers-reduced-motion` 시 페이드 0ms(즉시) — 오버레이의 감소 모션 관례와 같은 취지. ⚠️ 정적 HTML 폴백 유지(기본 표시 상태 = opacity 1). `activateTab` 의 탭 화이트리스트·persist·`settings_set_tab` invoke·hidden 토글 로직은 **그대로** — 표시 타이밍의 인라인 처리만 추가한다.
- **섹션/그룹 헤딩 대비 (UXR-04)**: 준수 세부는 §3.2 의 "대비 정책" 참조 — `h2.group` 은 대문자 변환 없이 캡션 헤딩으로, `color-mix(in srgb, currentColor 55%, transparent)` 대비(§3.2).

### 3.2 컨트롤 타입 카탈로그

| 타입 | 등장 위치 | 동작 규칙 |
| :--- | :--- | :--- |
| 체크박스 | 전 탭에 다수(§4 대부분) | 클릭 시 즉시 토글, 별도 확인 없이 §3.7 의 즉시 반영 계약을 따른다 |
| 팝업 버튼(열거형 선택) | Seek 1개(35종); Hyperkey 4개(35종×3 + 5종×1); Presets 다수(§4.3); General 1개(2종) | 클릭 시 드롭다운이 열리고 선택 즉시 반영. **모든 팝업의 선택지 전량이 이번 실측으로 확정됐다**(app-bundle-analysis.md §6.1~§6.3, 아래 §4 에 전재) |
| 인라인 팝업(라벨 문장 중간에 삽입) | Presets: `Caps lock +` [팝업] ` = ◀▼▲▶` | §3.8 "문장 중간 삽입" 규약 참조. `Caps lock + home row = ` [팝업] 처럼 문장 끝에 붙는 경우도 있어 삽입 위치가 항목마다 다르다 |
| **단축키 레코더** (`AXTextField`) | Seek: `Toggle Seek with shortcut:` | Sindre Sorhus 의 오픈소스 `KeyboardShortcuts` 패키지(`RecorderCocoa`)로 구현됨이 확정됐다. 상세 동작은 §3.5 |
| 슬라이더 + 값 라벨 | Presets: `Quick press duration` | **최소 250 ms · 최대 2000 ms**(실측: AX 트리), 기본값 1000 ms. 눈금 간격(step) `(미확정)`. 드래그 중 값 라벨 실시간 갱신(§3.7) |
| 부제/설명 텍스트(정적) | Seek·Hyperkey·General 다수 | 컨트롤 바로 아래 보조 텍스트. 상호작용 없음. 예: `Release the remapped key to click`, `When hidden, relaunch from Finder to open.` |
| 키캡/제스처 일러스트 | Hyperkey(선택된 소스 키, 트랙패드 영역), General(좌측 앱 로고) | 현재 선택 값을 반영하는 순수 표시 요소. 클릭 상호작용 없음 |
| **ⓘ 정보 팝오버 버튼** | Seek 3개(제목 옆·`Seek using macOS accessibility` 옆·`Change click modes with modifier keys` 옆), Hyperkey 1개(제목 옆) | ⭐ 실재하는 UI 컴포넌트다(실측: AX 트리 + `Info.storyboardc`). 컨트롤러: `SeekInfoViewController`·`SeekAccessibilityViewController`·`ClickModesInfoViewController`·`HyperkeyInfoViewController`. **F-09 는 팝오버의 배치(어느 항목 옆에 붙는가)와 컴포넌트 종류만 소유한다. 팝오버 안의 설명 문구 자체는 각 항목을 소유하는 명세(F-01~F-04, F-05)가 소유한다** — 원문 전문은 app-bundle-analysis.md §5.2 참조 |
| **버튼** | General: `v1.66 (66)`·`Remove Oldest Activation`·`Purchase` | ⭐ `v1.66 (66)` 은 정적 텍스트가 아니라 **버튼**임이 확정됐다(실측: AX 트리) — 클릭 시 동작은 About 창 오픈으로 추정되나 `(미확정)`. `Purchase` 는 강조색(accent color) 버튼 |
| **접이식 섹션** (`<details>`/`<summary>`) 🧩 | General 탭 최하단: `Advanced`(이슈 #77) | ⭐ **신규 구조 요소 타입.** `<details>` 는 저장소에서 첫 사용이다 — 기존 선례는 `#about`(`hidden` 속성 + 버튼 토글, 설정 창)뿐이었다. 동작 규칙: ① **기본 접힘**(`open` 속성 부재)으로 출하한다 ② **접힘 상태는 저장하지 않는다**(세션 상태 — F-15 "부재 = 기본값", 신규 저장 키 0개) ③ `<summary>` 가 그룹 헤딩을 겸한다(접이식 안에 `h2.group` 을 다시 두어 이중 헤딩을 만들지 않는다). ⚠️ 이 타입은 **한 컨트롤의 값이 다른 컨트롤의 노출을 결정**하는 §3.8 종속 표현 3종(① dimmed·② 숨김·③ 문장 중간 삽입)과 **다르다** — Advanced 는 어떤 설정 값에도 종속하지 않는 **사용자 조작 disclosure 위젯**이므로 §3.8 에 추가하지 않고 이 카탈로그의 구조 요소로 등재한다 |

⭐⭐ **컨트롤 시각 속성 확장 (이슈 #79, UXR-03·UXR-05·UXR-06·UXR-08·UXR-09)** — P0 현대화 인라인 반영. 컨트롤 종류·id·저장 키·카탈로그 키·invoke 배선은 **무변경**, 아래는 시각 속성만 추가로 정의한다:

- **액션 버튼 (UXR-03)**: `button.accent`(설정 창)와 온보딩 `button.primary`(`#open`)를 **시스템 강조색 채움**으로 그린다 — `background: color-mix(in srgb, Highlight 85~95%, Canvas)`, `color: CanvasText`(강조색 위 대비 보정 농도), hover 시 한 단계 진하게. ⛔ `.danger`(비활성화 확인)는 기존 붉은 스타일을 유지하고 accent 와 섞지 않는다. `disabled` 는 기존 opacity `.5` 규약 유지. (온보딩 화면에 같은 어휘를 적용 — `index.html`.)
- **온보딩 화면 공용 규약 (UXR-09)**: 온보딩(`index.html`)의 primary 버튼은 위 "액션 버튼"과 같은 어휘로 그린다. 수동 절차 블록(`pre.steps`)은 줄 간격 1.6 이상으로 읽기 흐름을 주고, 힌트(`locked-hint`)는 대비 55% `color-mix` 로 맞춘다. 문구·`modal_copy` 계약·버튼 배선 무변경(F-11 참조).
- **간격 공통 속성 (UXR-05)**: 힌트(부제)가 붙는 행은 아래 간격 12px, 힌트 없는 행 10px 유지. 섹션 구분선(`hr`) 상하 여백 18~20px. DOM 구조·순서 불변.
- **대비 정책 (UXR-04)**: 섹션/그룹 헤딩(`h2.group`·Event Viewer `thead th`)은 `text-transform` 없이 캡션 헤딩으로, `color: color-mix(in srgb, currentColor 55%, transparent)` — 라이트/다크 같은 대비.
- **경고 힌트 (UXR-06)**: "활성화 경로 없음" 같은 상태 알림은 `.warn` 규약(시스템 오렌지 `color-mix(in srgb, orange 70%, CanvasText)`)을 쓴다 — 일반 부제(`.hint`)와 구분. (예: `#seek-not-configured` 는 `hint warn`.)
- **단축키 레코더 — 필드형 캡슐 (UXR-08)**: `#seek-shortcut-record` 를 일반 버튼과 구분되는 **필드형 캡슐**로 그린다(둥근 radius 6~7px, `currentColor` 4~6% 배경 + 25% 테두리). 레코딩 중(`aria-pressed="true"`)엔 accent 테두리/채움으로 "입력 대기"를 알린다. 요소 종류·id·이벤트·라벨·조회/취소/commit 동작(§3.5)은 **무변경**.
- **focus-visible 공용 규약 (UXR-02)**: §3.4(접근성) 참조.
- **Event Viewer 소비 행 대비 (UXR-10)**: `event-viewer.md` §3.3 참조 — `tr.consumed` 는 `Highlight 12%` 배경 + 좌측 `inset 3px` 막대, hover `currentColor 7%`.

### 3.3 창 생명주기

- **열기 경로**: 메뉴바 아이콘 메뉴에서 `Settings…` 항목을 선택하는 것이 확인된 경로다(실측: 번들 문자열, app-bundle-analysis.md §6.5). 최초 실행 시 권한 온보딩(`F-11`)과 함께 자동으로 열리는지, `⌘,` 표준 단축키가 지원되는지는 여전히 `(미확정)` → §9.
- **탭 전환 시 리사이즈**: 원본은 탭을 바꾸면 창이 §3.1 표의 크기로 애니메이션과 함께(또는 즉시 — 애니메이션 유무는 `(미확정)`) 리사이즈된다(실측: AX 트리).
  ⛔ **클론은 리사이즈하지 않는다 (2026-08-30, 이슈 #32 — 사용자 결정).** 사용자 원문: *"왼쪽 메뉴를 선택할때마다 설정창 크기가 계속 변하는데 단일 크기로 고정돼면 좋겠고 / 사용자가 설정한 크기가 계속 유지돼면 좋겠음 / 가능하면 기본크기는 가장 큰 크기에 맞춰서 동작하는게 유리해보임"*.
  - **단일 고정 크기** — 탭을 바꿔도 창 크기를 건드리지 않는다. `settings_set_tab` 은 `ui.lastTab` 저장과 탭 이름 검증만 한다.
  - **기본 크기 = 가장 큰 탭 기준** — 825 × 821 pt(§3.1 아래 주석의 실측 근거). 어느 탭에서도 내용이 잘리지 않는다.
  - **사용자가 조절한 크기 영속** — 창을 드래그해 바꾼 크기를 `ui.windowWidth`·`ui.windowHeight` 로 저장하고 다음 기동에 복원한다. F-15 규약을 따른다: **두 키가 둘 다 있을 때만** 복원하고(부재 = 기본값), 저장은 컨트롤 단위 즉시 write-through 와 같은 성질로 **디바운스 400 ms 뒤 자기 스레드가 직접 쓴다** — 종료 시점 flush 에 기대지 않는다(로그아웃 때 종료 경로가 실행되지 않음이 실측됐다, `settings-store-and-integrity.md` §3.1.1 결정 1). 우리가 프로그램적으로 넣은 크기와 창이 아직 보이지 않을 때의 `Resized` 는 저장하지 않는다 — 그러지 않으면 "환경설정 창을 열기만 해도 `settings.json` 이 생긴다"(F-15 §8)가 깨진다.
  - **기각한 대안 ①** *원본대로 두고 각 탭 크기만 키운다*: 사용자가 지적한 것은 크기가 모자라다는 것이 아니라 **크기가 계속 변한다는 것**이다. 크기를 키워도 변하는 것은 그대로다.
  - **기각한 대안 ②** *탭별 크기를 유지하되 사용자가 조절하면 그 탭만 기억한다*: 탭 6개 × 크기 2값 = 저장 키 12개가 생기고, "내가 지금 보는 창 크기가 왜 아까와 다른가"라는 원래 불만이 부분적으로 남는다.
  - ⚠️ 이 갈라짐은 §3.1 표의 크기 열과 §5·§6(다중 디스플레이 위치 복원)의 전제를 바꾼다 — 클론에서는 창 크기가 탭에 따라 변하지 않으므로 위치 복원 로직이 크기 변화를 전제할 필요가 없다.
  - ⚠️ **알려진 한계**: 기본 높이 821 pt 는 세로 해상도가 아주 낮은 디스플레이(작업 영역 높이 ≈ 850 pt 미만)에서는 화면을 넘칠 수 있다 `(추정 — 그런 디스플레이에서 실측하지 못했다)`. 창은 `resizable` 이고 사용자가 줄인 크기는 영속하므로 한 번 줄이면 그 뒤로는 유지된다. 모니터 작업 영역에 맞춰 자동으로 줄이는 처리는 넣지 않았다 — 다중 디스플레이·해상도 변경 시의 경계 조건을 새로 만들 뿐이고, 실제로 겪은 문제가 아니기 때문이다(기각한 대안).
- **닫기**: 표준 macOS 창 닫기(빨간 stoplight 버튼, `⌘W`)로 닫힌다고 가정한다 `(추정)`. 창을 닫아도 앱 자체는 종료되지 않는다(§5) — 메뉴바 상주 앱의 표준 동작이며, 메뉴에 `Quit Superkey` 가 별도로 있다(실측: 번들 문자열).
  ⛔ **클론은 닫기를 숨김(hide)으로 전환한다 (2026-09-02, 이슈 #88 — 클론 자체 결정. 원본의 닫기 동작이 숨김인지 파괴인지는 여전히 미확인 → §9).** 위 문장은 **원본의 실측 기록**이므로 그대로 둔다 — 클론이 다르게 가기로 한 것이지, 실측이 틀린 것이 아니다(위 "탭 전환 시 리사이즈" ⛔ 블록과 같은 형식 선례). 클론은 `CloseRequested` 에서 `api.prevent_close()` + `hide()` 로 닫는 즉시 창을 숨기고, 설정 창 인스턴스를 **앱 종료까지 상주**시킨다(파괴·재생성하지 않는다).
  - 근거: `show_settings_window`(main.rs) 가 창 **존재**를 전제로 재열림 로직이 완결된다 — 파괴되면 `on_main_thread` 가 `None` 을 받아 에러 로그만 남긴다(이슈 #88 이 드러낸 결함). 크기 영속 배선(`wire_window_size_persistence`)은 setup() 1회 배선이라 파괴-재생성 시 재설치·크기 복원·디바운스 스레드 중복 방지 재설계가 필요하다. Seek 오버레이의 "재생성 비용 회피 상주" 관례(F-03 §3.1)와도 일치한다.
  - **기각: 파괴 후 재생성(builder)** — Event Viewer 의 `CloseRequested` 는 부수 동작(계측 종료)이 있는 창이라 이식에 재설계가 필요하다. 닫기 직후 400ms 디바운스 창 안 크기 변경 손실 가능(저장은 디바운스 스레드가 400ms 뒤). 재생성 시 `wire_window_size_persistence` 재호출이 디바운스 스레드 중복 스폰 버그를 유발하기 쉬운 구조다.
  - **공통 정책(#87 참조)**: 메뉴가 여는 주 창(설정)은 닫아도 **숨김 유지(상주)**. ⭐ **이슈 #87 의 About 창도 이 정책을 따른다** — `show_about_window`(main.rs) 가 `CloseRequested` 에서 `prevent_close()` + `hide()` 로 숨겨 상주한다(파괴-재생성 없음, #88 정합).
- **재열기 시 마지막 탭 기억**: 관찰 중 항상 `Seek` 탭으로 열렸다는 정황은 있으나, 이는 관찰 시작 시 탭을 그렇게 두고 시작했을 가능성과 구분되지 않는다 → `(미확정)` → §9.
- **다중 인스턴스 방지**: 이미 열려 있는 상태에서 메뉴바에서 다시 `Settings…` 를 선택하면 새 창을 열지 않고 기존 창을 최전면으로 가져온다 `(추정 — 원본 행동, 이번 실측에서 직접 검증하지 않았다 — macOS 단일 설정 창 관례)`. ⭐ **클론 동작 구현 확정 (2026-09-02, 이슈 #88)**: `show_settings_window`(main.rs) 가 **이미 보이면 `show()` 하지 않고 `set_focus()` 만** 호출한다 — 정확히 위 서술의 동작이다. 두 번째 프로세스의 재실행 신호(이슈 #68)도 같은 함수를 타므로 숨김 상태면 `show()`, 표시 상태면 포커스다. 이는 **클론 구현 확정**이지 원본의 `(추정)` 을 해소한 것이 아니다 — 원본 관찰은 여전히 미확정으로 남는다.

### 3.4 접근성

- **VoiceOver**: 원본은 AppKit 네이티브 창(`NSMainStoryboardFile`)이므로 표준 `NSButton`/`NSPopUpButton`/`NSSlider` 는 VoiceOver 를 "공짜로" 지원한다. 클론은 Tauri WebView 로 이 창을 구현하기로 결정했으므로(§7), 이 무료 지원은 클론에는 적용되지 않는다 — 표준 HTML 폼 컨트롤(체크박스=`<input type="checkbox">`, 팝업=`<select>`)은 WebKit 이 접근성 트리에 노출하지만, **비표준 커스텀 컨트롤**(단축키 레코더, 인라인 팝업이 낀 라벨 문장, 슬라이더-값 라벨 연동, ⓘ 팝오버)은 ARIA 속성을 명시적으로 부여해야 한다 `(추정)`.
- **키보드 내비게이션**: 탭 사이드바는 방향키로 이동 가능해야 하고, 패널 내부는 `Tab`/`⇧Tab` 으로 컨트롤 간 이동이 가능해야 한다. 단축키 레코더가 레코딩 모드일 때만 `Tab` 을 "다음 키 조합의 일부"로 먹고, 그 외엔 포커스 이동으로 처리해야 한다 `(추정)`.
- **다크모드**: macOS 시스템 외관을 따라야 한다. 스크린샷 5장은 전부 라이트 모드로 캡처되어(실측: 02~05-*.png) 다크모드 시각 사양 자체는 이번 조사로도 확인되지 않았다 `(추정)`.
- ⭐ **공용 `:focus-visible` 규약 (UXR-02, 이슈 #79)**: 키보드 사용자가 Tab 순회할 때 포커스 위치를 일관되게 보여야 한다. **버튼·탭·목록 행·개별 속성 없는 요소**에 `:focus-visible { outline: 2px solid Highlight; outline-offset: 1~2px; }` 를 공용 규칙으로 둔다. **체크박스·`select`·`input[type=range]` 는 WebKit 네이티브 포커스 링을 유지한다** — 커스텀 링으로 재구현하면 macOS 포커스 링 표현(시스템 설정)과 어긋나고 회귀를 만든다. `:focus`(마우스 클릭 시)에는 보이지 않게 하려고 `:focus-visible` 로만 건다.

### 3.5 단축키 레코더 동작 명세

대상: `Toggle Seek with shortcut:`. AX 상 역할은 `AXTextField`(검색 텍스트 필드와 동일 역할)이고, 미설정 시 표시 라벨은 `Record Shortcut` 이다(실측: AX 트리 + 번들 문자열).

⭐ **정체가 확정됐다.** 번들에 `KeyboardShortcuts_KeyboardShortcuts.bundle`(SPM 리소스 번들)이 있고, 실행 파일 심볼에 `KeyboardShortcuts.RecorderCocoa`·`RecorderModifierCocoa`·`RecorderForPlayback`·`CarbonKeyboardShortcuts` 가 확인된다(실측: 번들 심볼, app-bundle-analysis.md §1.2·§3.1). 이는 Sindre Sorhus 의 오픈소스 `KeyboardShortcuts` Swift 패키지다.

- **클론 구현 접근에 대한 판단**: `KeyboardShortcuts` 는 **Swift 전용 패키지**이며 Rust 에서 직접 링크할 수 없다. 그러나 이 사실이 클론의 구현 접근 판정(§7, Rust 바인딩)을 바꾸지는 않는다 — 원본이 이 패키지를 쓴 것은 원본이 Swift/AppKit 네이티브 앱이기 때문일 뿐, 클론이 동일 패키지를 써야 할 이유는 없다. 클론은 이미 계획된 대로 `objc2-app-kit` 을 통한 `NSEvent.addLocalMonitorForEvents(matching:handler:)` 직접 호출(§6)로 동등한 레코딩 UX 를 구현하면 된다. 즉 **원본의 라이브러리 선택은 참고 사실일 뿐, 클론에 새 네이티브 shim 의무를 부과하지 않는다** — 이 판단이 서지 않으면 "Swift 패키지가 있으니 Swift shim 이 필요하다"는 오판으로 M0(순수 Rust/Rust 바인딩 경계 설계)가 흔들릴 위험이 있어 명시적으로 기록한다.
- **`Record Modifiers` / `RecorderModifierCocoa`**: 실행 파일에 이 별도 레코더의 심볼이 존재한다 — **modifier 조합만 녹화하는 전용 레코더**의 존재를 뜻한다. Seek 클릭 모드 7종(§3.2, `ClickModesInfoViewController` 팝오버 내용, app-bundle-analysis.md §5.2)의 modifier 지정에 쓰이는 것으로 추정되나, **이 UI 는 4개 탭 AX 트리 어디에서도 관찰되지 않았다** `(미확정)` → §9. 별도 창이거나, 조건부로만 나타나거나, 이번 조사가 놓쳤을 가능성이 모두 남아 있다.
- **녹화 시작·취소·지우기**: 필드를 클릭하면 레코딩 모드로 전환되고(placeholder 가 `Type Shortcut` 류로 바뀌는 것이 `KeyboardShortcuts` 패키지의 표준 동작이나 SuperKey 실제 문구는 확인하지 못했다 `(미확정)`), `Esc` 로 취소, 지우기 ✕ 버튼으로 빈 값 저장은 동종 패키지의 표준 관례를 따른다고 가정한다 `(추정)`.
- **시스템 예약 단축키와의 충돌**: `kHISymbolicHotKeyCode`·`kHISymbolicHotKeyEnabled`·`kHISymbolicHotKeyModifiers` 심볼이 실행 파일에 확인된다(실측: 번들 심볼, app-bundle-analysis.md §3.2 항목 4) — ⭐ 이는 **시스템 예약 단축키 전체를 열거하는, 문서화되지 않은 Carbon API 경로**다. 기존 명세가 "공개 API 가 없어 부분 대응만 가능하다"고 판정한 것은 정정되어야 한다: 공개 API 는 없지만, **원본은 비공개(undocumented) Carbon 경로로 이를 우회한다.** 클론이 같은 경로를 택할지는 별도 판단이 필요하다 — §7·§9 참조.

### 3.6 설정 저장

**결정: 자체 JSON 저장소, `com.knollsoft.Superkey` 의 `NSUserDefaults` 도메인은 사용하지 않는다.** 이 결정 자체는 유지한다(근거는 기존 §3.6 그대로 아래에 보존).

⭐ **구현 크레이트 정정 (2026-08-30, M2 1차 / 이슈 #13).** 이전 판은 저장소를 `tauri-plugin-store` 로 구현한다고 적었다. **구현하면서 두 가지가 드러나 정정한다** — 상세 근거는 `settings-store-and-integrity.md` §7 (1) 에 있다.

1. ⛔ `#[serde(default)]` 구조체를 통째로 직렬화하면 **모든 필드가 파일에 쓰여** "부재 = 기본값"이 쓰기 쪽에서 깨진다. 디스크 표현은 타입 직렬화가 아니라 **희소 키-값 맵**(`{"schemaVersion":1,"values":{"<평평한.키>":<json>}}`)이어야 하고, 타입 구조체는 그 맵에서 필드별로 조립한다.
2. 저장 계층을 Tauri 에 의존시키면 `cargo test` 가 webview 툴체인을 요구하게 되어 `../dev/architecture.md` 의 "Tauri 는 앱 크레이트에만" 결정과 충돌한다. 그래서 저장 계층은 **`ultrakey-core::settings::store` 의 순수 Rust 구현**이다.

**바뀌지 않는 것**: JSON 파일을 앱 데이터 디렉터리에 두고 `NSUserDefaults` 를 쓰지 않는다는 결정, 그리고 아래 4개 근거.

⭐ **"부재 = 기본값" 규약을 명시적으로 추가한다.** 원본을 아무 설정도 바꾸지 않은 상태로 실행했을 때 `~/Library/Preferences/com.knollsoft.Superkey.plist` 에는 키가 **7개뿐**이었다(실측: defaults, app-bundle-analysis.md §2.1) — `SUEnableAutomaticChecks`·`SUHasLaunchedBefore`·`lastVersion`·창 위치·Paddle 캐시 두 건, 그리고 `hyperFlags`·`minAxCharCount` 뿐이다. `Remap key to hyper key`, 16개 프리셋 등 나머지 **모든 설정 키는 존재하지 않는다.** 즉 원본의 저장 계층은 **"키가 없으면 그 설정의 하드코딩된 기본값으로 동작한다"** 는 규약이며, 사용자가 한 번이라도 건드린 항목만 디스크에 기록된다(실측 뒷받침: 토글 후 원상복구했음에도 `capsWasdArrows`·`oneSwipeFromTop`·`seekOptions`·`seekRemapKeycode` 4개 키가 "기본값과 동등한 값"으로 새로 생겼다 — app-bundle-analysis.md §7.1).

- **클론도 같은 성질을 가져야 하는 이유**: 이 문서(F-09)와 각 소유 명세(F-01~F-08 등)가 §4 에서 못박는 "출고 기본값"은 **정의(definition)** 가 아니라 **원본을 관찰해 확정한 사실**이다. 이 사실 확정 자체가 "키 부재 = 기본값" 이라는 원본의 저장 모델을 전제로만 성립했다 — plist 에 값이 없다는 것을 "AX 로 관찰한 실제 화면 상태"와 짝지어야 비로소 기본값을 확정할 수 있었다(§2.1 의 방법론 자체가 그렇다). 클론의 `settings.json` 이 반대로 "모든 필드를 항상 명시적으로 쓴다"는 모델을 택하면, 향후 클론 자체의 출고 기본값 검증(새 버전에서 필드가 늘었을 때 "이 필드는 사용자가 안 건드렸다"를 구분하는 것)이 원본과 같은 방식으로는 불가능해진다. 따라서 클론의 `tauri-plugin-store` 스키마도 **"필드가 없으면 애플리케이션 코드의 기본값 상수를 쓴다"** 는 규약을 명시적으로 채택한다 — `settings.json` 에 모든 필드를 기본값으로 미리 채워 쓰지 않는다. 이는 §5(엣지 케이스, 스키마 마이그레이션)와도 직결된다: 새 버전에서 필드가 추가돼도 기존 JSON 파일은 그 필드가 없는 채로 유효하며, 로더가 기본값을 채워 넣는다.

원본 결정 근거(기존 판 보존):

1. **다른 앱의 도메인을 재사용할 이유가 없다.** 클론은 별도 번들 ID를 가질 것이므로 애초에 `com.knollsoft.Superkey` 도메인과 무관하다.
2. **`NSUserDefaults` 전용 고수준 Rust 크레이트가 없다.** `objc2-foundation` 으로 접근 가능하나 매 설정 변경마다 `unsafe` 호출을 거쳐야 한다. `tauri-plugin-store` 는 안전한 Rust API 로 충분하다.
3. **스키마 마이그레이션을 직접 제어해야 한다.** `NSUserDefaults` 는 버전 필드·마이그레이션 개념이 없다. JSON 쪽이 검사·백업·디버깅이 쉽다.
4. **트레이드오프를 명시한다.** `defaults read/write <bundle-id>` 상호운용성을 포기한다. §9 재검토 대상.

**저장 위치**: Tauri 앱 데이터 디렉토리(예: `~/Library/Application Support/<bundle-id>/settings.json`) `(추정)`. 참고로 원본은 `~/Library/Preferences/com.knollsoft.Superkey.plist`(`NSUserDefaults`) + `~/Library/Application Support/Superkey/750314.padl`(Paddle 라이선스 캐시, 바이너리 plist)를 쓴다(실측: defaults).

**"처리 데이터"와 "설정"의 구분**: 기존 판의 서술을 유지한다 — "None of the data that Superkey processes is stored on your disk" 는 런타임 캡처 데이터를 말하는 것이지 설정 자체를 말하는 것이 아니다. 설정값은 반드시 디스크에 저장된다.

**iCloud 설정 동기화(범위 밖)**: 실행 파일 문자열에 iCloud 설정 동기화 기능이 실재함이 확인됐다("Do you want to import your existing iCloud configuration?" 등, 실측: 번들 문자열, app-bundle-analysis.md §4.3). 저장소는 `NSUbiquitousKeyValueStore` 로 추정되나 심볼로 확인되지 않았다 `(미확정)`. 이 기능을 클론 범위에 넣을지는 이 문서가 결정할 사안이 아니며, 넣는다면 `F-15`(settings-store-and-integrity.md) 나 별도 명세가 다뤄야 한다 — 여기서는 존재만 기록한다.

### 3.7 설정 변경의 즉시 반영

- **적용 버튼 없음 — ⭐ 확인으로 승격.** 기존 판은 "스크린샷에 확인 버튼이 안 보인다"는 `(추정)` 이었다. 이번 실측은 4개 탭 전체의 AX 트리를 판독해 **Apply/OK 류 버튼이 어디에도 없음을 직접 확인했다**(실측: AX 트리, app-bundle-analysis.md §6.1~§6.4 전체 어디에도 그런 버튼이 열거되지 않는다). 모든 컨트롤은 값이 바뀌는 즉시(체크박스 클릭, 팝업 선택, 슬라이더 조작 종료, 레코더 확정) 저장과 엔진 반영이 함께 일어난다.
- **개별 컨트롤 단위로 즉시 반영됨 — 승격.** 실측 중 항목 하나를 토글했다가 되돌렸을 때, 그 항목에 대응하는 plist 키 하나만 정확히 새로 생겼다(§3.6, app-bundle-analysis.md §7.1) — 탭 단위나 창 닫기 시점의 일괄 저장이 아니라 **컨트롤 단위 즉시 저장**임을 실측이 뒷받침한다.
- **엔진으로의 계약**: 환경설정 UI 는 값이 바뀔 때마다 F-07(key-remapping-engine)에 갱신된 설정 스냅샷(또는 변경분 델타)을 전달한다. 이 계약의 정확한 형태(전체 스냅샷 재전송 vs. 필드 단위 델타)는 F-07 명세가 정의할 몫이다.
- **슬라이더의 "즉시"의 의미**: `(미확정)` 로 유지한다 — 드래그 종료(mouse up) 시점 반영인지 debounce 인지는 이번 실측으로도 확인하지 못했다. 슬라이더 범위(250~2000ms)는 확정됐으나 반영 타이밍은 별개 문제다.
- **저장 실패와 반영의 관계**: 기존 판 유지 — 엔진 반영은 즉시, 디스크 저장은 비동기로 처리하고 실패 시 알린다(§5).

### 3.8 ⭐ 설정 종속 표현 3종 규약

이번 실측에서 확인된 가장 중요한 UI 설계 사실이다. 종속된 하위 설정이 상위 설정 상태에 따라 UI 상 표현되는 방식이 **세 가지로 다르며, 서로 혼동해서는 안 된다.**

| 유형 | 정의 | 실측 사례 |
| :--- | :--- | :--- |
| **① 비활성화(dimmed)** | 자리를 계속 차지하지만 회색으로 흐려지고 조작 불가 | `Only show while the remapped key is held` — `Remap key to Seek:` 가 `-`(미설정)이면 dimmed |
| **② 숨김(hidden)** | AX 트리에서 완전히 사라짐. 자리 자체가 없어짐 | `Match on more than one character` — `Seek using macOS accessibility` 가 ☐ 이면 AX 트리에서 사라진다. `Change menu bar icon when engaged`·`Provide haptic feedback when triggered` 도 `Engage hyper key using trackpad:` 에 대해 같은 방식 |
| **③ 문장 중간 삽입(inline)** | 팝업이 라벨 문장 사이에 낀다(항상 존재, 종속 관계가 아니라 렌더링 방식) | `Caps lock +` [팝업] ` = ◀︎ ▼ ▲ ▶︎`(중간 삽입) vs. `Caps lock + home row = ` [팝업](끝에 부착) |

⭐ **기존 판 정정.** 이전 판 §3.2 는 "중첩 체크박스는 상위가 꺼지면 비활성화(dimmed)되나 저장값은 보존된다"고 `(추정)` 했다 — `Match on more than one character` 를 예로 들었다. **이 추정은 틀렸다.** 실측 결과 그 항목은 dimmed 가 아니라 **숨김**이다. 클론 UI 도 이 구분을 재현해야 한다 — ①은 CSS `disabled` 속성으로, ②는 조건부 렌더링(DOM 자체를 제거/삽입)으로 구현해야 하며 이 둘을 같은 방식으로 처리하면 원본과 다른 체감을 준다.

## 4. 설정 항목

⭐ 4개 탭의 전 항목을 실제 UI 기준으로 재작성한다. 각 항목에 라벨 원문·컨트롤 종류·선택지 전량·출고 기본값·활성화/표시 조건·배치 순서와 구분선 위치·소유 명세·저장 키(알려진 경우)를 표기한다. 저장 키가 `—` 인 항목은 IB 아웃렛만 확인되고 `NSUserDefaults` 키는 확인되지 않은 것이다(app-bundle-analysis.md §2.2).

### 4.1 `Seek` 탭 (상위 8개 + 조건부 1개 = 9개 항목, 555×378pt)

배치 순서 그대로 (실측: AX 트리, 02-seek-tab.png):

| # | 라벨 원문 | 컨트롤 | 기본값 | 활성화·표시 조건 | 소유 명세 | 저장 키 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| — | `Seek` + ⓘ(제목 옆) | 정적 텍스트 + 정보 팝오버 | — | — | F-01 | — |
| 1 | `Toggle Seek with shortcut:` | 단축키 레코더(`AXTextField`) | **빈 값(미설정)**, 표시 라벨 `Record Shortcut` | 항상 | F-01 | — |
| 2 | `Remap key to Seek:` | 팝업 버튼(35종, §4.1.1) | **`-`**(미설정) | 항상 | F-01 | `seekRemapKeycode` |
| 3 | `Only show while the remapped key is held` | 체크박스 | ☐ | ② 아님, **① 비활성화(dimmed)**: `Remap key to Seek:` = `-` 이면 dimmed. 부제 `Release the remapped key to click` 이 종속 | F-01 | `seekExecuteOnClose` |
| — | (구분선) | | | | | |
| 4 | `Seek using macOS accessibility` + ⓘ | 체크박스 + 정보 팝오버 | ☐ | 항상 | F-02 | `seekOptions` 비트 |
| 5 | `Match on more than one character` | 체크박스(중첩) | **☑**(값 `minAxCharCount = 2`) | **② 숨김**: 항목 4 가 ☐ 이면 AX 트리에서 사라진다 | F-02 | `minAxCharCount`(정수, 불리언 아님) |
| 6 | `Only Seek in the frontmost window` | 체크박스 | ☐ | 항상 | F-02 | `seekFrontmostOnly` |
| 7 | `Focus window before clicking` | 체크박스 | ☐ | 항상 | F-04 `(추정 — 소유 배정은 기존과 동일하게 유지, 이번 실측이 F-03/F-04 경계를 확정하지 못함)` | — |
| 8 | `Semicolon highlights next match` | 체크박스 | ☐ | 항상 | F-01 | `semicolonCycleSeek` |
| 9 | `Change click modes with modifier keys` + ⓘ | 체크박스 + 정보 팝오버 | **☑** — Seek 탭에서 유일하게 기본 켜짐 | 항상. 부제 `If this setting is disabled, modifiers will be applied to the click` 종속 | F-04 `(추정)` | `seekOptions` 비트 |

**`Remap key to Seek:` 팝업 선택지 35종(표시 순서대로, 실측: AX 트리)**: `-` · `caps lock` · `right option` · `right shift` · `right command` · `right control` · `left option` · `left shift` · `left command` · `left control` · `menu (PC)` · `F1`…`F24`. **`globe` 는 없다** — Hyperkey 탭 팝업(§4.2)에는 있어 두 열거형이 다르다.

**ⓘ 팝오버 내용의 소유**: 3개 팝오버(제목·항목4·항목9) 의 문구 자체는 각각 F-01/F-02/F-04 가 소유한다. F-09 는 "그 항목 옆에 ⓘ 가 있다"는 배치 사실만 소유한다(§3.2).

> ⭐(이슈 #93) — **클론 고유 항목 `검색 언어`** 는 위 "실측: AX 트리" 표에서 **제외**한다. 원본 SuperKey 의 Seek 탭에 이런 항목이 없기 때문(원본은 영어 단일 — 계획 §9 #9 참조)이고, 위 표는 원본 실측의 정본이므로 지우지 않고 **측면 주석**으로만 남긴다. 클론 구현의 항목 위치·저장 키·동작은 `seek-activation-and-session.md` §4 계열(저장 키 `seek.searchLanguage`)과 `settings.html` 이 정본이다 — 컨트롤은 팝업(5종: `en`·`ko`·`zh`·`ja`·`es`), 부재 시 로케일 파생 값이 선택된 것으로 표시되고 팝업 조작 시 명시 값으로 굳는다(Plan D1·D6 §9 #2).

### 4.2 `Hyperkey` 탭 (상위 6개 + 조건부 2개 = 8개 항목, 710×517pt)

| # | 라벨 원문 | 컨트롤 | 기본값 | 활성화·표시 조건 | 소유 명세 | 저장 키 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| — | `Hyperkey` + ⓘ(제목 옆) | 정적 텍스트 + 정보 팝오버 | — | — | F-05 | — |
| 1 | `Remap key to hyper key:` | 키캡 일러스트 + 체크박스 + 팝업(35종, §4.2.1) | ☐ / `caps lock` | 항상. 일러스트가 팝업 값을 반영 | F-05 | `hyperFlags` + 키코드 |
| 2 | `Include shift in hyper key` | 조합 미리보기(`⌃⌥⌘⇧` 텍스트) + 체크박스 | **☑**(`hyperFlags = 1966080` 로 시드됨 — `0x1E0000` = Shift\|Control\|Alternate\|Command) | 항상 | F-05 | `hyperFlags` 의 shift 비트 |
| — | (구분선) | | | | | |
| 3 | `Remap key to meh key (⌃⌥⇧):` | 키캡 + 체크박스 + 팝업(35종) | ☐ / `caps lock` | 항상 | F-05 | — |
| 4 | `Remap key to bleh key (⌃⌘⇧):` | 키캡 + 체크박스 + 팝업(35종) | ☐ / `caps lock` | 항상 | F-05 | — |
| — | (구분선) | | | | | |
| 5 | `Apply modifiers to keypress events and:` + `Click`/`Drag`/`Move`/`Scroll` | 라벨 + 체크박스 4개(그룹) | **`Click` ☑ · `Drag`/`Move`/`Scroll` ☐** — 켜진 것은 `Click` 하나뿐 | 항상 | F-05 | `clickEventsCheckbox` 등 4종 IB 아웃렛(키 미확인) |
| — | (구분선) | | | | | |
| 6 | `Engage hyper key using trackpad:` | 제스처 일러스트 + 체크박스 + 팝업(5종, §4.2.2) | ☐ / `top right` | 항상. 부제 `Slide only one touch in from the selected area the trackpad a little. Remove the touch to release.` 종속 | **F-06** | `oneSwipeFromTop` |
| 7 | `Change menu bar icon when engaged` | 체크박스 | ☐ | **② 숨김**: 항목 6 이 ☐ 이면 사라진다 | F-06 | `changeMenuBarIcon` |
| 8 | `Provide haptic feedback when triggered` | 체크박스 | ☐ | **② 숨김**: 항목 6 이 ☐ 이면 사라진다 | F-06 | — |

**hyper/meh/bleh 소스 키 팝업 선택지 35종(셋 다 동일, 표시 순서대로)**: `caps lock` · `right option` · `right shift` · `right command` · `right control` · `left option` · `left shift` · `left command` · `left control` · **`globe`** · `menu (PC)` · `F1`…`F24`.

**트랙패드 영역 팝업 선택지 5종(표시 순서대로)**: `top left` · `top right` · `bottom left` · `bottom right` · **`top`**.

### 4.3 `Presets` 탭 (16개 + 조건부 2개, 825×527pt)

전 항목 기본 ☐(캡스락/시프트/삭제/기타 4개 그룹, 배치 순서대로):

| # | 그룹 | 라벨 원문 | 컨트롤 | 선택지 | 기본값 | 소유 | 저장 키 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| 1 | caps lock | `Remap caps lock to:` | 체크박스 + 팝업(50종, §4.3.1) | 아래 표 | ☐ / `left control` | F-08 | `capsLockRemapped` |
| 2 | caps lock | `Quick press caps lock to execute:` | 체크박스 + 팝업(48종 + 구분선, §4.3.1) | 아래 표 | ☐ / `caps lock` | F-08 | — |
| 3 | caps lock | `Quick press duration` | 슬라이더 + 값 라벨 | 250~2000ms | 1000ms | F-08 | `quickPressTimeout` |
| 4 | caps lock | `Caps lock + space = enter` | 체크박스 | — | ☐ | F-08 | `capsSpaceEnter` |
| 5 | caps lock | `Caps lock + W A S D = ▲◀▼▶` | 체크박스 | — | ☐ | F-08 | `capsWasdArrows` |
| 6 | caps lock | `Caps lock +` [팝업] ` = ◀▼▲▶`(③ 문장 중간 삽입) | 체크박스 + 인라인 팝업(2종) | `H J K L`·`I J K L` | ☐ / `H J K L` | F-08 | `capsHjklArrows`/`capsIjklArrows` |
| 7 | caps lock | `Caps lock + home row = ` [팝업](③ 문장 끝 부착) | 체크박스 + 인라인 팝업(2종) | `symbol row (A = !)`·`function row (A = F1)` | ☐ / `symbol row (A = !)` | F-08 | `capsHomeSymbol`/`capsHomeFunction` |
| — | (구분선) | | | | | | |
| 8 | shift | `Double tap shift = caps lock` | 체크박스 | — | ☐ | F-08 | `doubleShiftToCaps` |
| 9 | shift | `Left shift + right shift = caps lock` | 체크박스 | — | ☐ | F-08 | `leftRightShiftToCaps` |
| 10 | shift | `Shift + caps lock = caps lock` | 체크박스 | — | ☐ | F-08 | `shiftPlusCapsToCaps` |
| 11 | shift | `Quick press left or right shift to input corresponding:` | 체크박스 + 팝업(4종) | `( )`·`[ ]`·`{ }`·`< >` | ☐ / `( )` | F-08 | `shiftToBraceEntersString`/`shiftToBraceSelection` |
| — | (구분선) | | | | | | |
| 12 | delete | `Hyper + delete = forward delete` | 체크박스 | — | ☐ | F-08 | `hyperDeleteToForward` |
| 13 | delete | `Remap delete to forward delete` | 체크박스 | — | ☐ | F-08 | — |
| 14 | delete | `Shift + delete = forward delete` | 체크박스 | — | ☐ | F-08 | `shiftDeleteToForward` |
| — | (구분선) | | | | | | |
| 15 | 기타 | `Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):` | 체크박스 + 팝업(4종) | `Right ⌘`·`Left ⌘`·`Either ⌘`·`Hyper key` | ☐ / `Right ⌘` | F-08 | — |
| 16 | 기타 | `Home & end operate on lines` | 체크박스 | — | ☐ | F-08 | — |

**조건부 항목(nib 에는 있으나 기본 상태 AX 트리에는 없음)**:

| 항목 | 표시 조건 | 관련 키 |
| :--- | :--- | :--- |
| `Apply hyper to arrows` | `(미확정)` — `Caps lock + W A S D` 만 켜서는 나타나지 않았다(실측으로 재현 시도했으나 실패) | `applyHyperToCapsArrows`·`capsArrowsOverrideModifiers` |
| Windows 키보드 리매핑(라벨 미확정) | `(미확정)` | `winKeyRemapCheckbox` |

**팝업 선택지 전량**

| 팝업 | 선택지(표시 순서) |
| :--- | :--- |
| `Remap caps lock to:` (50종) | `esc` · `nothing (disable it)` · `left control` · `left shift` · `left option` · `left command` · `right control` · `right shift` · `right option` · `right command` · `return (enter)` · `delete (backspace)` · `delete forward` · `tab` · `spacebar` · `home` · `end` · `pageup` · `pagedown` · `left arrow` · `right arrow` · `up arrow` · `down arrow` · `mute` · `volume up` · `volume down` · `F1`…`F24` |
| `Quick press caps lock to execute:` (48종 + 구분선 1) | `Seek` · (구분선) · `esc` · `caps lock` · `left control` · `left shift` · `left option` · `left command` · `right control` · `right shift` · `right option` · `right command` · `return (enter)` · `delete (backspace)` · `delete forward` · `tab` · `spacebar` · `home` · `end` · `pageup` · `pagedown` · `left arrow` · `right arrow` · `up arrow` · `down arrow` · `mute` · `volume up` · `volume down` · `F1`…`F20` · `/` |

⭐ `Quick press caps lock to execute:` 의 첫 항목이 `Seek` 다 — quick press 로 Seek 세션을 여는 **세 번째 활성화 경로**(단축키 레코더·`Remap key to Seek:` 리매핑에 이어)가 존재한다. F-01 이 이 사실을 §3(활성화 경로)에 반영해야 한다 — F-09 는 여기서 팝업 선택지로서만 기록한다.

레이아웃 변형 키(`hjklArrowColemak`·`wasdArrowDvorak` 등, 실행 파일에는 있으나 팝업에는 없음)는 사용자 선택이 아니라 감지된 키보드 레이아웃에 따라 자동 적용되는 것으로 보인다 `(미확정)` — `F-14`(localization-and-input-sources.md) 소관.

### 4.4 `General` 탭 (7개 컨트롤 + 조건부 1개, 613×273pt) — ⭐ 전면 교체

⭐ **이전 판 §4.4 는 전부 `(추정)`이었고 실제와 크게 다르다.** 실제 구성은 다음과 같다(실측: AX 트리, 05-general-tab.png, 배치 순서대로):

| # | 라벨 원문 | 컨트롤 | 기본값 | 위치 | 소유 명세 | 저장 키 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| — | 앱 로고 이미지 | 정적 이미지 | — | 좌측 | — | — |
| 1 | `Launch on login` | 체크박스 | ☐ | 첫 행 좌측 | F-10 | — |
| — | `v1.66 (66)` | **버튼**(정적 텍스트 아님) | — | 첫 행 우측(항목 1 과 같은 행) | ⭐ **이슈 #87 로 해소.** 버튼 클릭 → About 창 오픈(§9 Q12 추정 방향). 클론 `#version-btn` 도 About 창을 연다(`open_about_window` 커맨드 — `plan/issue-87` §9 #2 유지 결정) | — |
| 2 | `Check for updates automatically` | 체크박스 | ☐(`SUEnableAutomaticChecks = false` 와 일치) | | F-13 | `SUEnableAutomaticChecks` |
| 3 | `Hide menu bar icon` | 체크박스 | ☐ | 부제 `When hidden, relaunch from Finder to open.` 종속 | F-10 | — |
| 4 | `Menu bar icon` | 라벨 + 팝업(2종, 둘 다 **라벨 없는 이미지 항목**) | 기본 선택 `(미확정)` — 에셋 미추출로 어느 쪽이 기본인지 구분 못함 | | F-10 | — |
| 5 | `Remove Oldest Activation` | 버튼 | — | | F-12 | — |
| 6 | `Purchase` | 버튼(강조색) | — | | F-12 | — |

⭐ **"실재하지 않음"이 확인된 항목 — 이전 판이 추정했으나 전부 없다**: **언어 선택 팝업 · 권한 상태 표시(Accessibility/Screen Recording/Input Monitoring 배지) · 라이선스 키 입력 텍스트 필드 · `Reset to defaults` 버튼.** 이전 판 §4.4 표의 8개 행 중 이 4개는 AX 트리에 대응 항목이 없음을 직접 확인했다(실측: AX 트리, app-bundle-analysis.md §6.4) — 조용히 삭제하지 않고 "확인 결과 부재"로 여기 기록한다. 특히 **권한 상태 표시가 General 탭에 없다**는 것은 `F-11`(permissions-onboarding.md)이 짚어야 할 사실이다 — 권한 상태는 온보딩 모달이나 다른 경로로만 노출되고, 환경설정 창에는 상시 표시되지 않는다.

**조건부 항목**: `Relaunch on wake`(`wakeRelaunchCheckbox` / `wakeRelaunchStackView`) — nib 에는 있으나 기본 상태에서 보이지 않는다. 표시 조건 `(미확정)` → §9.

**⭐ 클론의 `Advanced` 접이식 섹션 (이슈 #77 신설 — 원본에 없는 클론 구성)**: 위 표는 **원본의 실측**이다. 클론의 General 탭은 그 위에 **최하단 `Advanced`** 를 하나 더 둔다(배치 순서: … → 설정 파일 → **Advanced**). 구성:

- **`<details id="general-advanced">`**(§3.2 "접이식 섹션" 구조 요소) — 기본 접힘, `open` 속성 없이 출하, 접힘 상태 비영속(세션 상태).
- 안에는 ① **`Synthesize Caps Lock Remap` 체크박스**(`presets.synthesizeCapsLockRemap` — 저장 키·엔진 계약은 불변, D5) + 그 아래 **위험 고지 paragraph**(`class="hint warn"`). 이 체크박스는 트레이 메뉴 `Advanced ▸ Synthesize Caps Lock Remap` 을 **제거한 뒤** 이곳이 유일한 표면이다(README 갈라짐 표 이탈 D9). 위험 고지의 방향: **기본(OFF) 상태에서 캡스락 의존 기능 활성 시 커널 HID 매핑(caps lock→F18)이 전역 설치**되고, 이 옵션을 켜면 그 매핑을 제거하고 이벤트 합성(경로 A)만 쓴다. 켬의 트레이드오프는 caps lock 래칭 한계(`key-remapping-engine.md` §5 #20)로 캡스락 기반 기능이 불안정해질 수 있다는 것.
- ② **기존 진단 툴**(`#open-event-viewer-btn`·`#open-log-folder-btn` + `general-event-viewer-hint`, 이슈 #39 Phase 3·#47)을 **Advanced 안으로 이동**. 버튼·커맨드(`open_event_viewer`·`open_log_folder`)·이벤트 리스너·i18n 키는 그대로 두고 DOM 위치만 옮긴다. 기존 `#general-diagnostics-heading`(h2.group)은 제거 — `<summary>` 가 헤딩을 겸한다(`settings.general.diagnostics` i18n 키 삭제).
- ③ **⭐ 이슈 #87 — About 정보 이동**: General 탭 상단에 있던 `#about` 블록이 제거되고(About 창 `ui/about.html` 신설 — `plan/issue-87` D1), **번들 ID·설정 파일 경로 행**(`#about-bundle-id`·`#about-settings-path` + 라벨 `settings.general.about.bundle_id`·`settings_path`·`settings_absent`)이 진단 성격이라 이 Advanced 섹션 **진단 버튼 아래**(`#general-path-info`)로 **이동**했다(제거 아님 — 값 채우기·라벨 배선은 불변). **로그 위치**는 About 창으로 이동(키 `settings.general.about.log_path` → `about.log_path` 재배치), **설치 위치**(`AppMeta.app_path`)는 About 창에 신규. **버전 버튼 `#version-btn` 은 유지**하되 클릭 동작이 "About 정보 펼치기"에서 "About 창 열기"로 바뀌었다(§9 Q12 해소, `plan/issue-87` §9 #2).

## 5. 엣지 케이스와 실패 모드

1. **설정 저장 실패.** §3.7 의 원칙대로 엔진 반영(메모리 상태)은 계속 성공시키되 사용자에게 저장 실패를 알리고 재시도하거나 마지막 성공 상태로 롤백할지 선택하게 한다 `(추정)`.
2. **손상된 설정 파일.** `settings.json` 이 파싱에 실패하면, 손상된 파일을 `settings.json.bak` 류로 보존한 뒤 §3.6 의 "부재 = 기본값" 규약에 따라 **필드별 기본값으로 채워 시작**하고 사용자에게 알린다 `(추정)`.
3. **이전 버전 스키마 마이그레이션.** §3.6 의 "부재 = 기본값" 규약 덕분에, 새 버전에서 필드가 추가되는 흔한 경우는 마이그레이션 함수 없이도 자동으로 해결된다(로더가 없는 필드는 기본값으로 채운다) — 필드 **이름 변경·삭제**만 명시적 마이그레이션 함수가 필요하다. 마이그레이션 자체가 실패하면 원본 파일을 보존한 채 기본값으로 시작한다 `(추정)`.
4. **설정 충돌.** ⭐ **해소됨.** 이전 판은 "UI 레벨에서 막을지 F-07 우선순위로 해소할지 미결"이라 썼으나, 실행 파일 문자열로 확인된 원본의 대화상자("Conflict: Caps lock arrows", "Disable that setting and enable WASD arrows?" 등, 실측: 번들 문자열)가 **사용자에게 묻고 상대 설정을 꺼주는 대화형 해소**임을 보여준다. 이 대화상자의 정확한 버튼 구성·전체 문구·정확한 재현 조건은 `F-15`(settings-store-and-integrity.md, 별도 담당) 가 상세 명세한다 — F-09 는 시나리오 D(§2)로 배치만 기록한다.
5. **권한 없는 상태에서 기능 토글.** 기존 판 유지 — 체크박스는 "이 기능을 원한다"는 사용자 의사이며, 실제 동작 가능 여부는 F-11 이 별도로 알린다. 다만 **General 탭에 권한 상태 표시 자체가 없음이 확인됐으므로**(§4.4), 이 불일치를 사용자에게 알릴 UI 표면이 원본에는 아예 없다는 뜻이다 — 클론이 이를 개선할지는 F-11 소관의 제품 결정이다.
6. **단축키 충돌.** §3.5 참조. 원본은 `kHISymbolicHotKeyCode` 등 **비공개 Carbon API 로 시스템 예약 단축키 전체를 조회하는 경로를 실제로 갖고 있다**(실측: 번들 심볼). 기존 판의 "공개 API 가 없어 부분 대응만 가능하다"는 서술은 "원본이 그렇게 한다"는 뜻은 아니었음이 드러났다 — 정정한다. 클론이 같은 비공개 경로를 쓸지의 판단은 §7·§9 참조.
7. **창을 닫아도 엔진은 계속 동작.** 기존 판 유지.
8. **다중 디스플레이에서 창 위치 복원.** ⭐ 창 크기가 탭마다 다르다(§3.1)는 사실이 이 로직에 직접 영향을 준다 — 저장된 좌표만으로는 복원할 수 없고, **어느 탭으로 열릴지(§3.3, 미확정)에 따라 창 크기가 달라지므로, 클램프 판정은 "열릴 탭의 크기"를 알고 나서 계산해야 한다.** 원본의 `NSWindow Frame EntryBarWindow` 저장 관례(실측: defaults, app-bundle-analysis.md §2.1)로 미루어, 환경설정 창도 `NSWindow Frame` 류로 위치만(크기는 탭에 종속) 저장할 가능성이 높다 `(추정)`.
9. **현지화 — ⭐ 성격이 바뀐 항목.** app-bundle-analysis.md §5.1 이 확정한 바, **SuperKey 앱 본체는 현지화가 전혀 없다** — `Contents/Resources/` 에 `Base.lproj` 하나뿐이고 `Info.plist` 에 `CFBundleLocalizations` 키가 없다. 이전 명세가 인용한 "8개 로케일 번들"은 appcast 의 `sparkle:deltaFromSparkleLocales` 오독으로, 실제로는 Sparkle 프레임워크 자체의 로케일 파일 목록이었다(app-bundle-analysis.md §5.1). **즉 로케일별 라벨 길이·RTL 대응은 원본에는 해당하지 않으며, 클론이 현지화를 제품 목표로 추가한다면 그건 원본을 재현하는 문제가 아니라 클론 고유의 신규 과제다.** 삭제하지 않고 이렇게 재규정한다 — 상세는 `F-14`(localization-and-input-sources.md) 소관.
10. **단축키 레코딩 중 창/앱 포커스 이동.** 기존 판 유지 `(추정)`.
11. **General 탭 민감정보의 저장 방식.** 기존 판 유지 — 다만 §4.4 확인 결과 General 탭에 **라이선스 키 입력 필드 자체가 없다**(라이선스 활성화는 Paddle Classic 프레임워크의 별도 창을 통한다, app-bundle-analysis.md §4.8). 따라서 이 항목의 실제 리스크 표면은 "환경설정 JSON 에 라이선스 키가 평문으로 남는가"가 아니라 "Paddle 활성화 캐시(`750314.padl`/`.spadl`)의 저장 형식"으로 좁혀진다 — F-12 소관.
12. **⭐ 토글 후 원상복구해도 저장소에 잔여 키가 남을 수 있다.** §3.6 이 인용한 실측 잔여물(`capsWasdArrows`·`oneSwipeFromTop`·`seekOptions`·`seekRemapKeycode`, app-bundle-analysis.md §7.1)이 보여주듯, 원본은 "값을 기본값으로 되돌려도 키 자체는 지우지 않는다." 클론도 같은 동작(불필요한 `delete` 호출을 하지 않음)을 채택할지, 아니면 "값이 기본값과 같아지면 키를 지워 파일을 더 작게 유지"할지는 결정되지 않았다 `(추정)` — 기능적으로 동등하므로 급하지 않은 결정이나, `settings.json` 사람이 읽고 디버깅한다는 §3.6 의 장점을 살리려면 후자가 더 깔끔하다.

## 6. 필요한 플랫폼 API

- **창 생성·관리(안전 Rust)**: Tauri `WebviewWindow`/`WindowBuilder` — `decorations(true)`(표준 타이틀바). ⭐ **탭마다 크기가 다르므로 고정 크기가 아니라 `set_size()` 를 탭 전환 시 호출하는 리사이즈 가능 창이어야 한다**(§3.1·§3.3, 실측: AX 트리로 확정). 오버레이(F-03) 전용 요구사항(특수 window level·클릭 통과)은 이 창에 해당하지 않는다.
- **창 위치 저장·복원**: Tauri `window.outer_position()` / `set_position()` + `window.available_monitors()`. §5 항목 8 의 클램프 로직은 "열릴 탭의 크기"를 먼저 결정한 뒤 계산해야 한다.
- **단축키 레코더의 로컬 키 캡처**: `global-hotkey` 크레이트는 "가로채기가 아니라 등록"이라 레코딩 UI 용도로는 맞지 않는다. macOS 표준 패턴은 `NSEvent.addLocalMonitorForEvents(matching:handler:)` 이며, `objc2-app-kit` 을 통해 접근 가능하나 커버리지가 명시적으로 검증되지 않았다 → §9. §3.5 에서 정리했듯, 원본이 쓰는 `KeyboardShortcuts`(Swift 전용 패키지)를 클론이 따라야 할 이유는 없다 — 이 API 경로로 충분하다는 판단이다.
- **시스템 예약 단축키 조회**: ⭐ 공개 API 는 없으나, 원본이 실제로 쓰는 경로가 확정됐다 — `kHISymbolicHotKeyCode`/`kHISymbolicHotKeyEnabled`/`kHISymbolicHotKeyModifiers`(`CopySymbolicHotKeys` 계열, Carbon HIToolbox, 실측: 번들 심볼). **클론이 이 경로를 택할지의 판단**: 이 API 는 공식 문서화되지 않았고 향후 macOS 버전에서 제거될 위험이 있다(Carbon 은 수년간 deprecated 상태). 그러나 `objc2`/`core-foundation` 바인딩으로 접근 가능한 C 심볼이므로 새 네이티브 shim 은 필요 없다(Rust 바인딩 판정 유지). **권장**: 완전한 커버리지가 필요하지 않다면(§5 항목 6 의 "알려진 고위험 조합 하드코딩 목록" 대응으로 충분하다면) 굳이 비공개 API 위험을 감수하지 않는 쪽을, 원본과 동등한 완전성을 목표로 한다면 이 경로를 채택하되 macOS 버전별 동작 확인을 CI 에 넣는 쪽을 제안한다 — 최종 채택 여부는 `(미확정)`, 제품 결정 사항으로 §9 에 남긴다.
- **설정 저장**: 플랫폼 API 요구 없음 — `ultrakey-core::settings::store` 의 순수 Rust JSON 저장소. 경로는 Tauri `app.path().app_data_dir()` 로 얻는다. §3.6 의 "부재 = 기본값" 규약은 **희소 키-값 맵** 으로 구현한다(⛔ `#[serde(default)]` 구조체 통째 직렬화로는 지킬 수 없다 — §3.6 정정 참조).
- **VoiceOver**: §3.4 참조 — 커스텀 컨트롤에는 ARIA 속성을 HTML/CSS/JS 레벨에서 직접 부여한다.
- **다크모드**: CSS `prefers-color-scheme` + `objc2-app-kit` 의 `NSApp.appearance` `(추정)`.
- **권한 상태 조회**: `AXIsProcessTrusted()`, `CGPreflightScreenCaptureAccess()`, `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)` — 단, §4.4 확인 결과 **원본은 이를 General 탭에 표시하지 않는다.** 클론이 표시할지는 F-11 의 제품 결정이다.

## 7. 구현 접근

**판정: Rust 바인딩.** 이번 실측으로도 이 판정은 바뀌지 않는다 — 오히려 근거가 보강됐다.

환경설정 창은 오버레이(F-03)와 요구사항이 근본적으로 다르다:

- **지연 예산이 느슨하다.** 체크박스 클릭, 팝업 선택, 슬라이더 드래그 모두 사람이 조작하는 폼 상호작용이다.
- **폼 컨트롤이 많다.** 4개 탭 합계 **41개 컨트롤**(Seek 9 + Hyperkey 8 + Presets 18 + General 8, §4 의 조건부 포함)이 체크박스·팝업·슬라이더·텍스트 필드·버튼으로 구성되며, HTML `<form>` 이 잘하는 일이다.
- **원본이 리사이즈 가능한 표준 창이라는 점이 실측으로 확정됐다(§3.1).** `NSScreenSaverWindowLevel`·`ignoresMouseEvents` 같은 오버레이 특수 처리가 여기엔 필요 없다는 판단을 오히려 강화한다.

⭐ **원본이 완전히 네이티브(AppKit + Storyboard, WebView 아님)라는 사실**(실측: 번들 문자열, `NSMainStoryboardFile = Main`)이 확정됐다. 이는 클론의 Tauri WebView 채택 결정을 뒤집을 근거는 아니다 — 원본의 구현 언어 선택은 클론 아키텍처의 제약이 아니라 참고 사실일 뿐이며, 클론이 이미 다른 명세들에서 확립한 Tauri 기반 아키텍처(`platform-constraints.md`)와 별개로 판단한다.

1. **네이티브 룩앤필 재현 비용.** WKWebView 는 macOS System Settings 특유의 시각 요소를 "공짜로" 제공하지 않는다 — 특히 원본의 **탭 전환 시 창 리사이즈**(§3.1)는 WebView 창에서도 재현 가능하지만(Tauri `set_size` 애니메이션), AppKit 이 기본 제공하는 부드러움과 동일한 체감을 내려면 추가 튜닝이 필요하다. 디자인 투자 비용의 문제다.
2. **단축키 레코더만 예외적으로 네이티브에 가깝다.** `NSEvent.addLocalMonitorForEvents` 류 API 가 필요하며 이는 `objc2-app-kit` 을 통한 `unsafe` 호출이다(Rust 바인딩 판정의 근거). §3.5 에서 정리했듯, 원본이 Swift 전용 `KeyboardShortcuts` 패키지를 쓴다는 사실은 클론에 새 shim 의무를 지우지 않는다 — 이 창 안의 **한 컨트롤**만 네이티브 API 를 호출하는 것으로 충분하다.
3. **시스템 예약 단축키 조회의 비공개 API 위험.** §6 에서 다룬 대로, 완전한 커버리지를 원한다면 `kHISymbolicHotKey*` 경로(비공개, Carbon)를 감수해야 한다. 이는 오버레이(F-03)의 `MultitouchSupport`(F-06) 위험과 같은 종류이므로, 채택한다면 F-06 과 동일하게 "실패 시 격하 가능한 선택 기능"으로 설계하는 편이 안전하다 — 알려진 고위험 조합 하드코딩 목록으로 최소 기능을 유지한 채, 완전 열거는 부가 기능으로 둔다.

- **기각한 대안 1 — "순수 Rust".** 창 위치 저장/복원, 단축키 레코더, 시스템 예약 단축키 조회가 모두 플랫폼 타입에 직접 의존한다.
- **기각한 대안 2 — "네이티브 shim 불가피".** `KeyboardShortcuts` 가 Swift 전용이라는 사실을 근거로 이 판정을 택할 수도 있었으나(§3.5), 그 패키지 자체를 이식할 필요가 없고 `objc2-app-kit` 의 `NSEvent` 로컬 모니터로 동등한 UX 를 구현할 수 있다는 판단에 따라 기각한다.

## 8. 수용 기준

- [ ] 환경설정 창을 열면 좌측 사이드바에 `Seek`·`Hyperkey`·`Presets`·`General` 4개 탭이 이 순서로, 각각 토글형 버튼으로 표시된다.
- [ ] 각 탭을 클릭하면 우측 패널이 전환되고 창 크기가 §3.1 의 값(Seek 555×378 · Hyperkey 710×517 · Presets 825×527 · General 613×273, pt)으로 리사이즈된다.
- [ ] 좌측 사이드바의 탭 6개 전부가 라벨 왼쪽에 아이콘을 갖고, 라이트/다크 양쪽에서 배경과 충분한 대비로 보인다(§3.1 탭 아이콘, 이슈 #40 ②).
- [ ] `Seek` 탭에 §4.1 의 상위 8개 항목이 모두 존재하고, `Match on more than one character` 는 `Seek using macOS accessibility` 가 ☐ 인 상태에서 **DOM/AX 트리에서 완전히 사라진다**(dimmed 가 아니라 hidden). `Only show while the remapped key is held` 는 `Remap key to Seek:` 가 `-` 인 상태에서 **자리를 차지한 채 dimmed** 로 표시된다.
- [ ] `Hyperkey` 탭에 §4.2 의 상위 6개 항목이 모두 존재하고, `Engage hyper key using trackpad:` 가 ☐ 이면 `Change menu bar icon when engaged`·`Provide haptic feedback when triggered` 2개가 숨김 처리된다.
- [ ] `Presets` 탭에 §4.3 의 16개 항목이 4개 그룹(캡스락 7·시프트 4·삭제 3·기타 2)으로 구분선과 함께 표시된다. `Quick press duration` 슬라이더는 250~2000ms 범위이고 기본값 1000ms 다.
- [ ] `General` 탭에 §4.4 의 7개 컨트롤(`Launch on login`·버전 버튼·`Check for updates automatically`·`Hide menu bar icon`·`Menu bar icon` 팝업·`Remove Oldest Activation`·`Purchase`)이 모두 존재하고, 언어 선택·권한 상태 표시·라이선스 키 입력 필드·`Reset to defaults` 는 **의도적으로 없다**.
- [ ] 신규 설치 상태(설정을 아무것도 바꾸지 않음)에서 §4 의 모든 항목이 표기된 출고 기본값과 정확히 일치한다 — 특히 `Include shift in hyper key`·`Change click modes with modifier keys`·`Match on more than one character` **3개만 ☑**, 나머지는 전부 ☐ 이다.
- [ ] 임의의 체크박스를 토글하면 별도의 "적용"/"확인" 조작 없이(그런 버튼이 UI 어디에도 없다) 다음 키 입력부터 F-07 리매핑 엔진에 새 값이 반영된다.
- [ ] 단축키 레코더 필드는 미설정 시 `Record Shortcut` 을 표시하고, 클릭하면 레코딩 모드로 전환되며, `Esc` 를 누르면 레코딩이 취소되고 이전 값이 유지된다.
- [ ] 앱을 재시작해도 이전에 설정한 모든 값이 그대로 유지되며, **한 번도 건드리지 않은 항목은 저장 파일에 키 자체가 없어도 올바른 기본값으로 동작한다**(§3.6 "부재 = 기본값").
- [ ] `settings.json` 이 파싱 불가능한 상태로 손상되어 있을 때 앱이 크래시하지 않고 필드별 기본값으로 시작하며, 손상된 파일이 별도 백업으로 보존된다.
- [ ] 환경설정 창을 닫아도 앱은 종료되지 않고, 직전까지 활성화되어 있던 리매핑은 계속 동작한다.
- [ ] 서로 충돌하는 caps lock 프리셋 두 개를 순서대로 켜면 대화상자가 뜨고, 확인 시 먼저 켠 설정이 꺼진다(F-15 상세 명세 필요, 여기서는 존재만 검증).
- [ ] 설정 창을 닫았다(빨간 stoplight 버튼 또는 `⌘W`) 다시 열면(메뉴 `Settings…`, #68 앱 재실행) 정상적으로 다시 열리고, 직전 탭·크기가 유지된다(창은 파괴되지 않고 숨김 상주, 이슈 #88). ⭐ **이슈 #87 — 메뉴 `About` 과 설정 창의 버전 버튼은 설정 창이 아니라 독립 About 창을 연다**(`About` 닫았다 다시 열기 상주는 About 창 절이 소유, `plan/issue-87` §5.2·`manual-verification.md`).

> ⚠️ **이슈 #88 자동 테스트 없는 근거**: 위 닫기 → 숨김 배선의 핵심은 `CloseRequested` 이벤트 수신 뒤 `prevent_close()`·`hide()` 호출인데, 둘 다 **Tauri 창 라이브 타입**(`tauri::WebviewWindow`)에 붙는 이벤트·호출이라 단위 테스트로 흉내 낼 수 없다. `tauri.conf.json` 이 설정 창을 `visible:false` 로 선언하는 자체(상주 전제)는 기존 `settings_window_default_matches_tauri_conf` 가 지킨다. 런타임 판정은 `docs/dev/manual-verification.md` 항목 18(M1~M9)이 소유한다.

## 9. 미해결 질문

⭐ 해소된 것은 어디서 해소됐는지 한 줄로 남기고 이 표에서 지운다. 아래는 여전히 남은 것과 이번에 새로 발견된 것이다.

| # | 질문 | 상태 |
| :--- | :--- | :--- |
| 1 | 환경설정 창을 최초 실행 시 자동으로 여는지, `⌘,` 단축키를 지원하는지 | `(미확정)` — 메뉴 항목 라벨(`Settings…`)은 확정됐으나(§3.3) 이 두 가지는 확인 못함 |
| 2 | 재열기 시 마지막으로 보던 탭을 기억하는지 | `(미확정)` — 관찰 중 항상 같은 탭이었다는 정황뿐, 관찰 시작 조건과 구분 안 됨 |
| 4 | `Focus window before clicking`·`Change click modes with modifier keys` 의 정확한 소유(F-03 vs F-04) | `(미확정)` — 이번 실측은 UI 배치만 다뤘고 소유 경계는 F-03/F-04 명세가 확정할 몫 |
| 5 | `Record Modifiers` / `RecorderModifierCocoa` 레코더 UI 의 위치 | `(미확정)` — 실행 파일에 심볼은 있으나 4개 탭 AX 트리 어디에도 없다. 클릭 모드 7종(F-04 영역)의 modifier 지정용으로 추정 |
| 6 | `Menu bar icon` 팝업 2종의 정체 | `(미확정)` — 라벨 없는 이미지 항목. `Assets.car` 추출은 저작권 경계상 하지 않았다 |
| 7 | `Apply hyper to arrows`·`Relaunch on wake`·Windows 키보드 리매핑의 표시 조건 | `(미확정)` — 재현 시도했으나 조건을 특정하지 못했다(app-bundle-analysis.md §7 항목 4·6) |
| 8 | `Quick press duration` 슬라이더의 눈금 간격(step) | `(미확정)` — 범위(250~2000ms)와 기본값(1000ms)만 확정 |
| 9 | 단축키 레코더의 정확한 취소/충돌 처리(blur 시 취소 여부, 시스템 예약 조합 충돌 시 동작) | `(미확정)` — `KeyboardShortcuts` 패키지 표준 동작을 참고할 뿐 SuperKey 의 실제 동작을 재현 관찰하지 못함 |
| 10 | 시스템 예약 단축키 조회에 비공개 `kHISymbolicHotKey*` 경로를 클론이 실제로 채택할지 | 제품 결정 사항 `(미확정)` — 위험과 대안은 §6·§7 에 판단 근거를 남겼다 |
| 11 | `NSUserDefaults` 미사용 결정(§3.6)이 파워유저의 `defaults` 명령 상호운용성 기대에 미치는 영향을 받아들일지 | 제품 결정 사항, 이번 실측이 "부재 = 기본값" 규약의 중요성을 더 명확히 했을 뿐 결론은 내지 않았다 |
| 12 | `v1.66 (66)` 버튼의 클릭 동작과 소유 명세 | ✅ **해소 (2026-09-02, 이슈 #87)** — 클론의 독립 About 창(`ui/about.html`)이 신설되고 `#version-btn` 클릭이 그 창을 연다(`open_about_window`). Q12 의 "About 창 오픈 추정"이 원본 추정 방향 그대로 클론에서 실현됐다. **원본 버튼을 직접 눌러 About 창이 열리는 것은 여전히 미관찰**이다 — §3.3 `About.storyboardc` 실측이 창 존재를 보증할 뿐(§9 해소 목록 참조). 소유 명세는 About 창 내용(F-10 주관, 정보 행 값은 F-09·F-13 과 공유) |
| 13 | `NSEvent.addLocalMonitorForEvents` 가 `objc2-app-kit` 에 실제로 안정적으로 노출되어 있는지 | `(미확정)` — 크레이트 문서·프로토타입으로 별도 검증 필요, 이번 실측 범위 밖 |
| 14 | 원본 SuperKey 의 설정 창 닫기 동작이 **숨김**인지 **파괴**인지 | `(미확정)` — 이슈 #88 의 클론 결정(닫기 → 숨김 상주, §3.3 ⛔ 클론 결정 블록)은 **클론 자체 결정**이지 원본 실측이 아니다. 원본 관찰(원격 실측 또는 개발자 문의)은 후속 실측 대상으로 남는다 |

**해소되어 제거된 항목(이전 판 §9)과 해소 근거**:
- `General` 탭의 실제 항목·라벨·컨트롤 타입 → §4.4(실측: AX 트리)로 확정.
- 환경설정 창을 여는 메뉴 항목 라벨 → `Settings…` 로 확정(실측: 번들 문자열, §3.3).
- `Presets`·`General` 패널 제목에 ⓘ 가 있는지 → 없음으로 확정(§3.1, 정보 팝오버 컨트롤러가 4개뿐임을 실측).
- 각 팝업 버튼의 선택지 전체 목록 → §4.1~§4.3 에 전량 확정.
- 각 설정의 출고 기본값 → §2.1(plist) + §6(AX) 조합으로 확정.
- `F-05`/`F-06`/`F-08` 의 실제 명세 ID 배정 → 각 명세 문서 자체의 헤더에서 이미 확정되어 있었음을 확인, 이 문서의 ID 혼동을 정정(§3.1).
- 서로 다른 탭 간 동일 물리 키 충돌 시 UI 차단 여부 → 대화형 해소(사용자에게 묻고 상대를 끔)로 확정, 상세는 F-15 소관(§5 항목 4).
- 설정 변경이 컨트롤 단위로 즉시 반영되는지 → 확정(§3.7, 잔여 plist 키 실측 근거).
- 메뉴바 메뉴 항목 구성 → 전량 확정(app-bundle-analysis.md §6.5, F-10 소관 문서가 갱신 필요).
- 4개 탭 아이콘의 구체적 모양(구판 §9 질문 3) → 저장소의 v1.66 스크린샷 4장을 눈으로 읽어 확정, 클론의 구현 방식(인라인 SVG)까지 §3.1 에 기록(이슈 #40 ②).
