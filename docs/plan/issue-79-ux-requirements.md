# 이슈 #79 — UI/UX 현대화 요구사항 (ultrakey-uxreview 산출 문서)

> **성격**: 이 문서는 `ultrakey-uxreview`(UI/UX 리뷰 전용 중급 에이전트)의 **산출물 자리**다. 현재 상태는 **초안 골격** — 구현 역할(ultrakey-implement, 이번 위임)이 리뷰 대상 체크리스트(§1)·요구사항 형식과 우선순위 태그 규칙(§0)·실행 명령(§5)을 먼저 잡아 두었고, 실제 전수 리뷰 결과는 §5 실행 명령으로 `ultrakey-uxreview` 를 호출해 §2~§4 에 채운다.
>
> **정본 흐름**: `ultrakey-uxreview` 산출(초안) → `ultrakey-review`(상급) 리뷰 → 반영 → **호출 세션 확정** → 명세 반영(3단계) → 구현 위임(4단계). 확정 주체는 호출 세션이다(AGENTS.md §2, 계획 issue-79 §2 D3·D4).
>
> **⚠️ 산출 메커니즘 (계획 §9 리뷰 #1)**: `ultrakey-uxreview` 는 `edit: deny` 로 **파일을 쓸 수 없다**. 관례는 에이전트가 요구사항 본문(§2~§4)을 **최종 응답으로 전문 반환** → 호출 세션이 이 문서를 작성·커밋한다. 리뷰 실행도 §5 의 올바른 호출 방식(서브에이전트 Task 위임)을 쓴다.

---

## 0. 산출 규칙

### 0.1 우선순위 태그 규칙 (P0~P2)

| 태그 | 의미 | 반영 시점 |
| :--- | :--- | :--- |
| **P0** | 1차 현대화 — 시각·인터랙션 체감(컬러·타이포·간격·포커스·애니메이션·컨트롤 스타일). 컨트롤 id·저장 키·카탈로그 키·invoke 배선 **무변경**이 전제 | 이슈 #79 PR(4단계) |
| **P1** | 1차 범위 경계 — 명세 갱신/소유 판정이 선행돼야 반영 가능 | 명세 갱신(3단계) 후 4단계 |
| **P2** | 제품 개선 — 설정 항목 추가·삭제·재배열·문구 변경·창 크기 재결정 | 별도 후속 이슈(5단계) |

- P0·P1 은 `docs/spec/preferences-ui.md` §3.1·§3.2·§3.4·§3.8 에 **인라인 반영**(계획 D3). P2 는 기존 기능 명세(F-01~F-17)에 귀속 + 후속 이슈로 분리.
- "현대화/제품/유지" 3분류와의 대응: **1현대화 → P0·P1**, **2제품 → P2**, **3유지(불변) → 본문 요구사항에 싣지 않고 §2.4 재검토 보고로만** 남긴다(계획 D2·D6).

### 0.2 요구사항 형식 템플릿

각 요구사항은 아래 형식으로 1개씩 쓴다. 요구사항 ID 는 `UXR-<번호>`, 추적 가능하게 붙인다.

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-01` … |
| `화면` | 등장 화면(탭 이름 또는 HTML 파일·트레이 메뉴) |
| `분류` | 1현대화 / 2제품 / 3유지 |
| `우선순위` | P0 / P1 / P2 |
| `요구사항` | **위치·현재 표현·권장 표현** 순서로 구체적으로("이 화면의 (위치)가 (무엇) 때문에 부족하고 (이렇게) 바라면 된다") |
| `수용 기준` | **눈으로 확인 가능**한 항목(라이트/다크 구분 포함) |
| `명세 연동` | `preferences-ui.md §3.x` 인라인 / `F-xx` 귀속 / 없음(순수 스타일) |

---

## 1. 리뷰 대상 체크리스트 (전수 실측 · 계획 D8 순서)

> `ultrakey-uxreview` 가 화면별로 아래 항목을 전수 실측한다 — (a) DOM·CSS 구조 (b) 인터랙션(포커스·호버·transition·애니메이션) (c) 문구(카탈로그 키 기준). 스크린샷은 읽지 않는다(⛔ — `docs/research/screenshots/` 참조 금지, 계획 §9 리뷰 #2).

- [x] **`settings.html` 6탭 전체** — `Seek`·`Hyperkey`·`Presets`·`Korean`·`Keyboards`·`General`(탭 id `tab-{seek,hyperkey,presets,korean,keyboards,general}`, ARIA `role="tab"`, 좌측 사이드바 + 우측 패널). 각 탭의 (a)(b)(c)를 따로 적는다
- [x] **`index.html`** — 권한 온보딩(`#onboarding`)·`#out-of-sync` 섹션. 신규 사용자가 처음 보는 화면
- [x] **`overlay-searchbar.html`** — 검색바 + `#detecting-track`/`#detecting-thumb` + 매치 리스트. Material 검색 UX 기준 대조(계획 D2 2단계)
- [x] **`overlay-highlight.html`** — 현재 DOM·CSS (계획 §8 리스크 #4 미조사 항목 — 이 체크리스트에서 확정)
- [x] **`eventviewer.html`** — 툴바(listening 상태·Clear) + 로그 스크롤 + 상태바
- [x] **트레이 메뉴** — `main.rs` `setup_tray` / `build_normal_menu`·`build_unauthorized_menu`. 네이티브라 CSS 적용 불가 → 항목 구성·라벨·아이콘만 리뷰(계획 D8·§8 리스크 #6)
- [x] **i18n 5언어** — `resources/i18n/{en,ko,zh,es,ja}.json` 에서 **탭·화면별 사용 키** 추출("현재 어떤 문구가 어느 화면에 있는가")
- [x] **번역 품질 정조사(2차 예비)** — 현존 문자열 중 문맥·정확도 이슈는 §3 에만 기록(계획 D7 — 1차는 카탈로그 무변경)

> ⚠️ **실측 수치 — 상급 리뷰로 정정 (2026-09-02)**: 계획 issue-79 §3 조사 표의 "en 251행·165 keys" 와, 이 문서 1차 채움 당시 §4 Q1 이 잡은 "251행·247 keys" 는 **전부 재현 불가 오기**였다. 정본 실측은 **`en.json` = 252행 · 250 출현 · 248 유니크 키** 이고 **5개 카탈로그가 동일**하다(중복 키 2건이 출현 수를 늘린다 — §3). 에이전트는 이 값을 기준으로 삼는다.

---

## 2. 리뷰 결과 — 요구사항

> 아래 2.1~2.4 는 `ultrakey-uxreview` 가 §0 형식·태그 규칙에 따라 채운다. 요구사항만 남긴다 — 조사 과정·방법론은 이 문서에 나열하지 않는다(계획 D2 4단계: "결론을 먼저 쓴다").

### 2.1 P0 — 1차 현대화 (이슈 #79 PR 대상)

> P0 는 전부 "1현대화" 분류다. 현재 UI 의 시각 토대(`:root { color-scheme: light dark }` · Canvas/CanvasText · `color-mix(currentColor …)` 3요소)는 유지한다 — 아래 항목은 그 토대 위의 **표현 계층**을 바꾼다.

> 모든 요구사항은 §0.2 형식(위치·현재 표현·권장 표현)을 따른다. **P0 전제(컨트롤 id·저장 키·카탈로그 키·invoke 배선 무변경)는 각 항목이 지킨다** — 언급이 없는 항목은 배선 무변경이다.

---

#### UXR-01 — 시스템 강조색 기반 선택 상태 규약 (사이드바 탭 · Keyboards 디바이스 목록)

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-01` |
| `화면` | `settings.html` — 사이드바(`.sidebar button[aria-selected="true"]`) · Keyboards 탭 디바이스 패인(`.keyboards-device-pane [role="option"][aria-selected="true"]`) |
| `분류` | 1현대화 |
| `우선순위` | P0 |
| `요구사항` | 현재 선택 표현이 `currentColor` 농도뿐이다 — hover(8%)와 선택(14%)의 차이가 약하고 라이트/다크에서 같은 회색 톤이라 "지금 어느 탭을 보고 있는가"가 덜 또렷하다. macOS 설정 UI 의 선택 관례(시스템 강조색 tint)와도 다르다. → 선택 상태를 `color-mix(in srgb, Highlight <농도>, transparent)` 기반으로 바꾼다(약 12~16% 채움 + `font-weight` 유지). hover(8% currentColor)는 그대로 두어 **hover 와 선택이 색 온도로도 구분**되게 한다. `Highlight` 는 시스템 강조색 키워드라 테마별 자산이 필요 없고(현재 규약과 같은 이유), 라이트/다크에서 accent 색이 자동으로 따라온다. **`aria-selected`·`data-tab`·컨트롤 id 전부 무변경** — CSS 선택기 값만 바뀐다. |
| `수용 기준` | ① 라이트/다크 각각에서 선택된 탭·디바이스가 시스템 강조색 계열 채움으로 보이고 hover 와 색상·강도가 명확히 구분된다. ② 마우스를 올린 비선택 항목(8% 회색)과 선택 항목이 동시에 보이는 상태에서도 어느 쪽이 선택인지 즉시 판별된다. ③ 탭 전환·디바이스 선택 동작이 변경 전과 동일하다. |
| `명세 연동` | `preferences-ui.md` §3.1(탭)·§3.2(카탈로그 "선택 상태" 속성) 인라인 |

---

#### UXR-02 — focus-visible 공통 규약 (설정 창)

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-02` |
| `화면` | `settings.html` — 전체 컨트롤(체크박스·select·슬라이더·버튼·단축키 레코더·탭·Keyboards 목록 행) |
| `분류` | 1현대화 |
| `우선순위` | P0 |
| `요구사항` | `:focus-visible` 스타일이 사이드바 버튼(`outline: 2px solid Highlight; offset -2px`)에만 정의되어 있고, 나머지 컨트롤은 WebKit 기본 포커스 링에 의존해 **표현이 제각각**이다(체크박스·select·슬라이더는 시스템 링, 버튼은 링 + 이미지 자산 없음). 키보드 사용자가 Tab 순회할 때 어디에 있는지 일관되게 보여야 한다. → 공용 규칙을 추가한다: ① 컨트롤(버튼·탭·목록 행·개별 속성 없는 요소)에 `:focus-visible { outline: 2px solid Highlight; outline-offset: 1~2px; border-radius 보정 }` ② **체크박스·select·`input[type=range]` 는 네이티브 WebKit 포커스 링을 유지한다** — 커스텀 링으로 재구현하면 macOS 접근성(포커스 링 모양이 시스템 설정과 다름)과 회귀를 만든다. `:focus`(마우스 클릭 시)에는 안 보이도록 `:focus-visible` 로만 건다. |
| `수용 기준` | ① Tab/⇧Tab 으로 설정 창을 순회할 때 모든 포커스 가능 요소가 동일한 형태(2px Highlight 링)로 포커스를 표시한다(키보드 사용 시에만). ② 마우스로 클릭한 컨트롤에는 포커스 링이 나타나지 않는다. ③ 다크/라이트 양쪽에서 링이 배경과 대비된다. |
| `명세 연동` | `preferences-ui.md` §3.4(접근성) 인라인 |

---

#### UXR-03 — accent 버튼: 시스템 강조색 채움 + 글자 대비 자동 (설정 창 · 온보딩 공용)

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-03` |
| `화면` | `settings.html` — `button.accent`(`purchase-btn`·`license-activate-btn`·`conflict-continue`·`confirm-continue`) · `index.html` — `#open`(primary, 온보딩의 유일한 1차 동작) |
| `분류` | 1현대화 |
| `우선순위` | P0 |
| `요구사항` | 현재 `.accent` 는 `font-weight: 600` 외에 배경이 없어 일반 버튼과 구분이 약하고, 온보딩의 `Open System Settings`(primary)도 `font-weight: 500` 뿐이라 **첫 화면의 1차 행동이 시각 1순위로 안 보인다**. macOS 기본(action) 버튼 관례는 accent 채움이다. → 공용 액션 버튼 스타일을 추가한다: `background: color-mix(in srgb, Highlight 85~95%, Canvas); border: 1px solid color-mix(in srgb, Highlight 60%, CanvasText)` + hover 시 Highlight 100% 로 한 단계 진하게. ⭐ **구현 확정 (2026-09-02)**: 글자색은 `color-mix(in srgb, CanvasText 10%, white)` — 라이트/다크 모두 **흰색 계열**(요구사항 초안의 "다크에서 진한 글자" 추정과 다르지만, 강조색 채움 위 대비 수용 기준을 흰색 계열이 충족 — `settings.html` 버튼 채움이 정본). ⚠️ **버튼 id·텍스트·클릭 배선·`disabled` 규약(구매·비활성화 버튼의 `opacity: .5`) 무변경.** `.danger`(비활성화 확인)는 기존 붉은 스타일 유지 — accent 와 섞지 않는다. |
| `수용 기준` | ① 라이트/다크 각각에서 accent 버튼이 시스템 강조색으로 채워져 일반 버튼과 즉시 구분되고, 버튼 라벨이 채움 위에서 대비된다(WCAG 대비 체크 가능). ② 온보딩 `Open System Settings` 가 설정 창 accent 버튼과 같은 어휘로 보인다. ③ hover 시 한 단계 진해지고, disabled 상태는 기존 opacity 규약 그대로다. |
| `명세 연동` | `preferences-ui.md` §3.2(버튼 카탈로그 — "액션 버튼" 속성 확장, 온보딩 화면 공용으로 명시) 인라인 |

---

#### UXR-04 — 섹션·그룹 헤딩 대비 정책 (대문자 변환 제거)

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-04` |
| `화면` | `settings.html` — `h2.group`(Presets 4그룹 · Korean 제외 목록 헤딩 · Keyboards 탭 2곳(settings.html:814 · 843 — `#keyboards-keyremap-heading`·`#keyboards-functionkeys-heading`) · General `Startup & menu bar`·`License`·`Settings file`) · `eventviewer.html` — `thead th`(동일 스타일 사용) |
| `분류` | 1현대화 |
| `우선순위` | P0 |
| `요구사항` | `h2.group` 이 `text-transform: uppercase` + `opacity: .6` 인데, ① 한글 UI(`프리셋`·`시작 및 메뉴바`)에서 대문자 변환은 무효한 규칙일 뿐이고 ② `opacity .6` 은 다크에서 흰 글자가 회색으로 뭉개져 대비가 약하다. 최신 macOS 설정 UI 는 대문자 강조 없는 캡션 헤딩을 쓴다. → `text-transform` 제거(영문 "Caps Lock"·"Shift" 그룹명도 원문 표기 유지 — CSS 로 변형하지 않음), `opacity` 대신 `color-mix(in srgb, currentColor 55%, transparent)` 로 바꿔 라이트/다크에서 같은 대비를 보장한다. **문구·키 구조 무변경 — CSS 속성만.** `eventviewer.html` 의 `thead th`(11px · uppercase · opacity .6)도 같은 정책으로 일관시킨다(진단 창이므로 대문자 유지 여부는 이 항목 범위에서 CSS 속성만 — 헤딩 텍스트는 테이블 컬럼 제목이라 변환 제거 권장). |
| `수용 기준` | ① 라이트/다크 각각에서 섹션 헤딩이 같은 강도로 보인다(라이트에서 연회색, 다크에서 연회색 — 대비가 같다). ② 한글 헤딩과 영문 그룹명("Caps Lock"·"Delete")이 대문자로 왜곡되지 않는다. ③ Event Viewer 컬럼 헤딩에도 같은 대비가 적용된다. |
| `명세 연동` | `preferences-ui.md` §3.1(그룹 구조)·§3.2(부제/헤딩 속성) 인라인 |

---

#### UXR-05 — 행 간격·섹션 구분 체계의 최소 보정

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-05` |
| `화면` | `settings.html` — 전 탭의 `.row`(margin 10px 단일) · `hr`(16px) |
| `분류` | 1현대화 |
| `우선순위` | P0 |
| `요구사항` | 모든 행이 `margin: 10px 0` 단일이라 ① 힌트(부제)가 붙은 항목과 다음 항목 사이가 힌트 없는 항목과 같은 간격이라 읽기 흐름이 끊기고 ② `hr` 상하 16px 은 섹션 경계가 약하다. **재배치·순서 변경은 하지 않는다.** → ① 힌트가 붙는 행(`.row` + 다음 `.hint`)의 하단 간격을 12px 로, 힌트 없는 행 간격 10px 유지 ② `hr` 상하 여백을 18~20px 로 늘리고 ③ 선택된 탭이 아닌 탭의 라벨 판독성은 UXR-01 의 선택 강화로 해결(별도 조치 없음). 변경은 CSS margin 수치뿐이다 — DOM 구조·순서 불변. |
| `수용 기준` | ① 힌트가 있는 항목("…키를 눌러야 한다" 류 부제)과 다음 항목 사이에 힌트 없는 항목보다 넓은 간격이 보인다. ② 구분선(hr) 위아래가 변경 전보다 여유 있어 보이고 섹션 단위로 묶여 읽힌다. ③ 탭 내 항목 순서가 변경 전과 같다. |
| `명세 연동` | `preferences-ui.md` §3.2(카탈로그 공통 속성 — 간격) 인라인 |

---

#### UXR-06 — `Seek` 탭 활성화 경로 부재 경고를 경고 표현으로 격상

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-06` |
| `화면` | `settings.html` — Seek 탭 `#seek-not-configured` |
| `분류` | 1현대화 |
| `우선순위` | P0 |
| `요구사항` | "Seek has no way to open right now…" 는 **세 활성화 경로가 전부 비어 기능을 단 한 번도 발동할 수 없는 상태**의 알림인데 `p.hint`(opacity .65·12px)로만 렌더되어 일반 부제와 구분이 안 된다. → 기존 `.warn` 규약(시스템 오렌지 `color-mix(in srgb, orange 70%, CanvasText)`)을 적용하고, 탭 최상단에 이미 있는 위치는 유지한다. **카탈로그 키·문구·표시 조건(anyActivationConfigured) 무변경 — 클래스만 `hint` → `hint warn`.** |
| `수용 기준` | ① 출고 기본값(아무 설정도 안 건드린 상태)에서 Seek 탭에 오렌지색 경고가 다른 힌트와 구분되어 보인다. ② 단축키·리매핑·quick press 중 하나라도 설정하면 사라진다. ③ 라이트/다크 양쪽에서 오렌지가 대비된다. |
| `명세 연동` | `preferences-ui.md` §3.2(warn 힌트 속성) 인라인 |

---

#### UXR-07 — 탭 전환 패널 페이드(감소 모션 대응 포함)

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-07` |
| `화면` | `settings.html` — 탭 전환(`activateTab()` 의 `panel-*` hidden 토글) |
| `분류` | 1현대화 |
| `우선순위` | P0 |
| `요구사항` | 탭을 바꾸면 패널이 즉시 스냅 교체된다. macOS 설정 UI 를 포함한 현대 앱은 콘텐츠 전환에 가벼운 페이드(100ms 안팎)를 쓴다. → 패널 표시 시 90~110ms opacity 페이드 인을 넣는다. 구현 주의: `[hidden]`(display:none) 상태에서는 transition 이 동작하지 않으므로, `hidden` 해제 직후 패널에 인라인 `opacity: 0` 을 세팅하고 reflow 로 시작값을 고정한 뒤 `requestAnimationFrame` 1프레임에서 인라인 opacity 를 제거해 CSS 기본(opacity 1)으로의 페이드 인을 만든다(⚠️ 초안의 `.tabpanel-visible` **클래스** 방식은 CSS 기본 opacity 를 0 으로 만들어 정적 렌더·기본 표시 상태 요구("opacity 1")를 위반하므로 **인라인 방식으로 구현** — 계획 §6 지시서 반영). **`prefers-reduced-motion: reduce`(또는 `accessibilityDisplayShouldReduceMotion`) 시 페이드 0ms(즉시)** — 오버레이의 기존 감소 모션 관례와 같은 취지. ⚠️ 유일하게 JS 변경이 들어가는 P0 항목이다 — 그러나 `activateTab` 의 탭 화이트리스트·persist·`settings_set_tab` invoke·hidden 토글 로직 자체는 **통째로 무변경**, 표시 타이밍의 인라인 처리만 추가한다. 정적 HTML 폴백(JS 죽어도 현행대로 동작)을 해치지 않는다 — 인라인 처리는 표시 보강이라 hidden 토글에 실패해도 기능 무영향이어야 한다(부드러운 전환이 아니라 즉시 표시로 폴백). |
| `수용 기준` | ① 탭을 바꾸면 패널이 약 100ms 페이드로 나타난다(첫 페인트). ② 시스템에서 "동작 줄이기"를 켠 상태에서는 즉시 전환된다. ③ 전환 애니메이션 후의 최종 렌더는 변경 전과 동일하다(컨트롤·배선 무영향). ④ JS 없는 정적 렌더에서도 패널이 즉시 보인다(기본 표시 상태 = opacity 1 — JS 사망 시 투명 폴백 없음) |
| `명세 연동` | `preferences-ui.md` §3.1(탭 구조 — 전환 동작) 인라인 |

---

#### UXR-08 — 단축키 레코더를 필드형(캡슐) 컨트롤로

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-08` |
| `화면` | `settings.html` — Seek 탭 `#seek-shortcut-record`(뒤에 `#seek-shortcut-clear` "Clear" 버튼) |
| `분류` | 1현대화 |
| `우선순위` | P0 |
| `요구사항` | 현재 레코더가 일반 `button` 과 완전히 같은 스타일이라 "클릭하면 단축키를 받는 필드"로 안 보인다(마우스 사용자는 잘 발견하나, "Record Shortcut" 라벨과 창 밖의 설정 관례를 모르는 사용자는 버튼으로 오독). macOS 단축키 레코더 관례는 테두리 캡슐/필드 형태다. → 버튼 요소·id·이벤트(`startSeekRecording`)·라벨(카탈로그 키)은 그대로 두고 **표면 스타일만 필드형**으로 바꾼다: 약간 넓은 좌우 패딩, 둥근 캡슐(radius 6~7px), `background: color-mix(in srgb, currentColor 4~6%, transparent)` + `border 1px currentColor 25%`(일반 버튼보다 테두리 또렷하게), 레코딩 중 상태(`aria-pressed="true"` · "Recording…")는 accent 테두리/채움으로 전환해 "지금 입력을 받는 중"을 즉시 알린다. **녹음 로직·Escape 취소·commit 무변경.** |
| `수용 기준` | ① 레코더가 일반 버튼과 시각적으로 구분되는 필드형 캡슐로 보인다. ② 클릭하면 "Recording…" 상태에서 캡슐이 강조되어 입력 대기가 시각적으로 드러난다. ③ 키 입력·Esc 취소·Clear 동작이 변경 전과 동일하다. |
| `명세 연동` | `preferences-ui.md` §3.2(단축키 레코더 — 스타일 속성)·§3.5(동작 명세는 무변경) 인라인 |

---

#### UXR-09 — 온보딩(권한) 화면의 1차 액션 강조 + 상태 문구 대비

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-09` |
| `화면` | `index.html` — `#onboarding`(`#open` 버튼은 UXR-03 과 공용) · `#out-of-sync`(`pre.steps`) |
| `분류` | 1현대화 |
| `우선순위` | P0 |
| `요구사항` | ① `#open` primary 버튼은 UXR-03 의 공용 액션 규약으로 해결된다(중복 기술 생략). ② `#out-of-sync` 의 `pre.steps`(3단계 수동 절차)는 `color-mix(currentColor 8%)` 배경에 **이미 `font: inherit`**(index.html:47 — 본문과 같은 폰트를 쓰고 있음)지만, 단계 번호(1. 2. 3.)가 평문이라 계층이 약하다. → 단계 줄 간격을 6px 로 벌리고(현재 `pre.steps` padding 10px 12px 유지) 줄 간격 1.6 이상으로 읽기 흐름을 준다. ③ `locked-hint`(자물쇠 안내)의 opacity .65 는 UXR-04 와 같은 대비 정책(55% color-mix)으로 맞춘다. **문구·`modal_copy` 계약·버튼 배선 무변경 — CSS·간격만.** |
| `수용 기준` | ① 온보딩 화면(라이트/다크)에서 primary 버튼이 accent 채움으로 보이고, ② out-of-sync 3단계 절차가 줄 간격이 벌어진 읽기 쉬운 블록으로 보이며, ③ 모든 힌트 문구가 라이트/다크에서 같은 대비로 보인다. |
| `명세 연동` | `preferences-ui.md` §3.2(공용 버튼·힌트 규약 — 온보딩 화면에 적용) 인라인 + `permissions-onboarding.md`(F-11) 참조 |

---

#### UXR-10 — Event Viewer 소비 행 강조의 다크 대비 보정

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-10` |
| `화면` | `eventviewer.html` — `tr.consumed`(`Highlight 9%` 배경 + inset 3px 막대) · `tbody tr:hover`(5%) |
| `분류` | 1현대화 |
| `우선순위` | P0 |
| `요구사항` | 이 창의 핵심 질문("내가 누른 것이 그대로 나갔는가, 아니면 Ultrakey 가 먹었는가")을 담당하는 `tr.consumed` 가 라이트에서 9% Highlight 배경이라 옅고, 다크에서는 더 약해진다. hover(5%)와의 구분도 작다. → ① consumed 배경을 `Highlight 12%` 로, 좌측 막대는 `inset 3px` 유지하되 색을 `Highlight` 그대로 두어 테마 양쪽에서 또렷하게 ② hover 를 `currentColor 5%` → `7%` 로 한 단계 보강. **표 마크업·렌더 로직·문자열 무변경 — CSS 값만.** |
| `수용 기준` | ① 라이트/다크 각각에서 키가 소비(Consume)된 행이 다른 행과 배경색으로 즉시 구분된다. ② 행 hover 시 배경이 consumed 와 섞이지 않고 구분된다(consumed 위를 지나가도 consumed 로 보인다). ③ "소비된 행 = 파란 막대"의 의미가 테마와 무관하게 유지된다. |
| `명세 연동` | `event-viewer.md` §3.3(색 표현) 인라인 + `preferences-ui.md` §3.2(공용 대비 규약 참조) |

### 2.2 P1 — 명세 갱신 선행 후 반영

> P1 은 전부 "1현대화" 성격이되 **소유 명세의 미확정 값이 갱신·확정돼야** 반영 가능하다(계획 D3·D5 — 명세 없는 CSS 변경 금지).

---

#### UXR-11 — 오버레이 팔레트 명세 따라잡기: 출고값 소스를 구현으로 확정 (라이트 팔레트 · 테마 전환 계약)

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-11` |
| `화면` | `overlay-searchbar.html` · `overlay-highlight.html` — 팔레트(`:root` 폴백 CSS 변수 · `PALETTE_FALLBACK`) |
| `분류` | 1현대화 |
| `우선순위` | P1 |
| `요구사항` | (상급 리뷰 반영 — 새 값 제안을 없애고 **명세 따라잡기로 재규정**) 오버레이 팔레트는 **이미 구현이 확정 상태**다 — 출고값 정본은 `crates/ultrakey-overlay/src/palette.rs:66-80`(라이트 팔레트, 이슈 #67 `bar_fg` 포함) 이고 회귀 테스트 4종이 그 값을 고정한다. 반면 명세가 이를 따라가지 못해 `docs/spec/seek-overlay-ui.md` §4.2 는 전부 `(추정)`, §5 엣지 11 은 다크↔라이트 전환을 "다음 재렌더"로 기술한다. → ① **새 팔레트 값을 제안하지 않고**, `palette.rs` 의 라이트·다크 출고값(highlight·selected·line·label·bar·border·animation 전부)을 §4.2 의 확정 기본값으로 승격할 것을 요구한다. ② 외관 판정은 **세션 시작 시** `is_dark_appearance()`(apps/ultrakey-app/src/seek.rs:144) 이며, 한 세션 안에서 고정된다(`ultrakey-overlay/src/session.rs:48,67` — `OverlaySession` 이 appearance 를 보유) → §5 엣지 11 계약을 **"다음 재렌더"가 아니라 "다음 Seek 세션부터"** 로 정정할 것을 요구한다. ③ 두 HTML 의 폴백 값은 그대로 두되, 명세·폴백이 구현(정본)을 가리키도록 주석으로 연결한다. P1 분류는 유지(명세 갱신 선행). |
| `수용 기준` | ① `seek-overlay-ui.md` §4.2 에 라이트/다크 출고 팔레트가 `palette.rs` 값 그대로 확정되어 있다(라이트 `bar_fg = #1A1A1A` 포함). ② §5 엣지 11 이 "세션 시작 시 고정 · 다음 Seek 세션부터 반영" 계약으로 갱신되어 있다. ③ 시스템 라이트 테마에서 Seek 를 발동했을 때 검색 바·하이라이트·선택·연결선이 `palette.rs` Light 값 그대로 보인다. |
| `명세 연동` | `seek-overlay-ui.md` §4.2·§5(엣지 11) 갱신 — F-03 귀속 |

### 2.3 P2 — 제품 개선 (후속·별도 이슈)

> P2 는 전부 "2제품" 분류다 — 설정 항목·문구·구조 변경이라 카탈로그/명세 갱신이 선행돼야 한다(계획 D5·D7). 여기서는 요구사항만 확정한다.

---

#### UXR-12 — 사용자 대상 문구에 남은 개발 노트 제거 (General 탭 · 트레이 메뉴)

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-12` |
| `화면` | `settings.html` General 탭 · 트레이 메뉴(툴팁 없음 — 라벨만) |
| `분류` | 2제품 |
| `우선순위` | P2 |
| `요구사항` | 현재 카탈로그의 **4개 문구가 내부 구현 단계를 그대로 사용자에게 말한다** — ① `settings.general.menu_bar_icon.why` = "There's no menu bar icon yet — F-10, M2 phase 2." ② `settings.general.license.key.hint` = "…Licensing is scoped to F-12; the current build always treats a license as active." ③ `settings.general.license.why` = "Licensing is scoped to F-12, M4." ④ `settings.general.launch_on_login.why` = "…On macOS 12 it falls back to a LaunchAgent — that path has not been tested on real hardware." — 전부 개발자 메모가 UI 문구로 샌 것이다. → 기능 상태에 맞는 **사용자 대상 문구로 재작성**(① 메뉴바 아이콘 스타일 선택 기능이 없는 현재 상태를 안내하는 중립 문구 ② 라이선스 상태·활성화 경로 안내 ③ 라이선스 구매 경로 안내 ④ macOS 12 폴백 동작이 확정된 뒤 사실만). 상태가 변하면 문구도 함께 갱신한다(구현 단계 문구 금지 규약을 카탈로그에 명문화). 5언어 동일 키·같은 톤으로 번역. |
| `수용 기준` | ① General 탭에서 "F-12"·"M2"·"M4"·"scoped to" 등 내부 식별자가 문구에서 사라진다. ② 각 문구가 현재 기능 상태를 설명하는 문장으로 바뀐다(예: 메뉴바 아이콘 항목 비활성 이유). ③ 5개 카탈로그 키 집합이 불변인 채 값만 교체된다. |
| `명세 연동` | `F-10`(`menu_bar_icon`)·`F-12`(라이선스)·`F-13`(로그인 항목 — `launch_on_login.why`) 귀속 + `F-14`(문구 톤 규약) |

---

#### UXR-13 — 트레이 `Advanced ▸` 서브메뉴의 항목 1개 문제 정리

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-13` |
| `화면` | 트레이 메뉴 — `build_normal_menu()` 의 `Advanced ▸ Relaunch` |
| `분류` | 2제품 |
| `우선순위` | P2 |
| `요구사항` | D9(이슈 #77)가 `Synthesize Caps Lock Remap` 항목을 설정 화면으로 옮기면서 `Advanced` 서브메뉴에 `Relaunch` **하나만** 남았다. 항목 1개짜리 서브메뉴는 macOS 메뉴 관례상 부풀림이다(펼침 동작 없이 클릭 한 번 더 요구). → `Advanced ▸ Relaunch` 를 **최상위 `Relaunch` 항목으로 승격**하고 `Advanced` 서브메뉴를 제거한다(향후 Advanced 가 다시 항목을 얻으면 그때 복원). ⚠️ 결정이 필요한 지점: 원본 실측 기록(원본 Advanced 서브메뉴 구성)은 지우지 않고 "클론은 단일 항목이 되어 최상위로 올렸다"는 이탈 기록을 `menu-bar-and-lifecycle.md` §3.3 옆에 붙인다(README 갈라짐 판정 기준 1 — D5·D9 선례의 "사용자 요구"가 아니라 **자체 정리**이므로, 이탈 판정을 받으려면 이 항목 자체가 갈림길 리뷰를 거쳐야 한다). `menu.advanced.*` 카탈로그 키는 승격 후에도 남는다(라벨 재사용 — `menu.advanced.relaunch` → `menu.relaunch` 이동 권장, 키 변경은 F-14 규약에 따라 5언어 동시). |
| `수용 기준` | ① 트레이 메뉴에 `Relaunch` 가 최상위 항목으로 보이고 `Advanced ▸` 가 없다. ② `.app` 번들 밖 실행 시 Relaunch 가 disabled 이던 기존 동작이 유지된다. ③ 5개 언어 카탈로그에서 메뉴 라벨이 일치한다. |
| `명세 연동` | `F-10`(`menu-bar-and-lifecycle.md` §3.3) 귀속 — 이탈 판정 리뷰 선행 |

---

#### UXR-14 — 오버레이 하드코딩 한국어 → 문자열 카탈로그 이전

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-14` |
| `화면` | `overlay-searchbar.html`(`<html lang="ko">` · `counter` 의 `"없음"` · diag 문구) · `overlay-highlight.html`(`"+N개 더"` 우하단) |
| `분류` | 2제품 |
| `우선순위` | P2 |
| `요구사항` | 오버레이 2개 창이 카탈로그(F-14 키 구조)를 거치지 않고 한국어를 하드코딩한다 — ① 검색 바 매치 0건 표시 `"없음"` ② 하이라이트 상한 생략 표시 `"+N개 더"` ③ 두 창의 `showDiag` 진단 문구 ④ `lang="ko"` 문서 선언. 이것은 "i18n 5언어 키 구조" 불변 전제의 **문서화되지 않은 예외**다(설정 창과 인라인 규약이 다르다). → 새 카탈로그 키를 추가하고(제안: `overlay.searchbar.no_matches`·`overlay.searchbar.diag_*`·`overlay.highlight.more_omitted` 등 — 5언어 동일 키 세트), 팔레트·문구를 Rust 가 함께 보내거나 bootstrap 시 카탈로그를 주입하는 계약으로 이전한다. **값이 아니라 키 사용처가 바뀌는 변경**이라 새 키 추가 = 카탈로그 변경 = 1차 금지, 후속(F-03 명세에 IPC 계약 명시)에서만. |
| `수용 기준` | ① 오버레이 표시 문자열 중 하드코딩된 `"없음"`·`"+N개 더"` 가 카탈로그 키 경유로 바뀐다. ② 5개 언어 각각에서 같은 자리에 번역된 문구가 보인다. ③ `lang` 선언이 언어 상태와 일치한다. |
| `명세 연동` | `F-03`(seek-overlay-ui IPC)·`F-14`(키 구조) 귀속 — 새 키 목록은 §3 에 |

---

#### UXR-15 — General 탭 "구현 안 됨" 배지·비활성 항목의 완결 처리

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-15` |
| `화면` | `settings.html` General 탭 — `menu-bar-icon`(비활성 select + "Not yet" 배지) · 라이선스 버튼 disabled 배지 |
| `분류` | 2제품 |
| `우선순위` | P2 |
| `요구사항` | `Menu bar icon` 항목이 `disabled` select + `Not yet`(배지) + 개발 노트 why 문구(UXR-12 로 교체 예정)로 **구현 중임을 그대로 보여준다**. 제품 관점에서 ① 기능이 준비되면 활성화(현재 궤도: M5 이후) ② 아니면 항목을 **안내 문구 1줄로 접어서** 비활성 select 를 없앤다. 어느 쪽이든 "Not yet" 배지의 반복 사용(menu-bar-icon·purchase 2곳)을 줄이고, 기능별 완료 상태 표시 규약을 정한다. UXR-12 와 함께 진행(문구 교체 + 컨트롤 노출 결정이 한 묶음). |
| `수용 기준` | ① General 탭에서 "Not yet" 류 배지가 기능별로 일관된 규약으로만 보인다(또는 안내 문구로 대체). ② 비활성 select 가 사라지고 항목이 제품 문구로 설명된다. ③ 저장 키·컨트롤 id 는 유지(활성화 시 복원). |
| `명세 연동` | `F-10`(`menu_bar_icon`)·`F-12`(purchase) 귀속 |

---

#### UXR-16 — `ko` 카탈로그 문체·용어 통일

| 필드 | 내용 |
| :--- | :--- |
| `ID` | `UXR-16` |
| `화면` | `resources/i18n/ko.json` 전역(문체) + 개별 키(용어) |
| `분류` | 2제품 |
| `우선순위` | P2 |
| `요구사항` | §3 에 상세 기록한 대로 `ko` 번역에 **한다체·명령조·존댓말이 혼재**한다(예: `settings.seek.not_configured` "…설정하라.", `menu.launch_on_login.requires_approval` "…켜야 한다.", `settings.korean.not_working.hint` "…다시 시도한다." vs 나머지 "~세요"·"~합니다"). → 문체를 **"~합니다/하세요" 존댓말 평서형 1종으로 통일**하는 1회성 작업을 별도 이슈로 진행한다(키 구조 불변 — 값만 교체는 5언어 동시가 필요하고, ko 단독 문체 교정은 en 톤 규약과 짝을 이뤄야 한다). |
| `수용 기준` | ① `ko.json` 전 키에서 문장 종결 표현이 한 문체로 통일된다. ② 사용자에게 지시하는 문구와 상태를 알리는 문구의 톤 구분이 일관된다. ③ 키 집합은 불변이다. |
| `명세 연동` | `F-14`(현지화 — 톤 규약 §3) 귀속 |

### 2.4 유지(불변) 재검토 보고

전면 고정 제약 4종(D3·D6·D7)에 대해 이 리뷰가 **"바꾸고 싶다"로 분류한 항목만** 다음과 같다. 바꾸는 판정은 하지 않는다 — 후속 갈림길/별도 이슈의 몫이다.

- **D5 단일 고정 크기 창 (825×821pt)** — "바꾸고 싶다" **유보적 의견 1건**: 명세 §3.3 의 알려진 한계대로 높이 821pt 는 작업 영역 ≈ 850pt 미만 디스플레이(구형 13인치 맥북 등)에서 화면을 넘칠 수 있다. 다만 **리뷰 중 측정한 현재 6탭 중 어느 탭에서도 내용 잘림이 관찰되지 않았고**(패널 스크롤로 흡수), 탭 전환 시 크기 변화가 없는 것이 이슈 #32 사용자 결정이므로 **전면 유지가 옳다**고 판단한다. 후속에서 원하면 "최대 높이를 작업 영역으로 클램프하지 않는다"는 기존 결정 재확인만 남긴다 — 요구사항으로 승격하지 않는다.
- **종속 표현 3종 (§3.2·§3.8)** — "바꾸고 싶다" 항목 **없음**. ① dimmed(disabled·opacity .45) ② 숨김(DOM 생성/삭제) ③ 문장 중간 삽입(inline-row) 구현이 실측 확정 표현과 일치하고, 이번 리뷰가 발견한 것은 표현 **스타일**(UXR-01·UXR-04)뿐이다 — 3종의 **구현 방식**은 유지가 옳다.
- **저장 규약 (F-15 "부재 = 기본값")** — "바꾸고 싶다" 항목 **없음**. 희소 키-값 맵이 1차 현대화 무영향 전제(P0 = 저장 키 0 변경)와 export/import·"설정을 안 건드리면 파일이 없다" 동작의 원인이다 — 유지.
- **i18n 5언어 키 구조 (F-14)** — "바꾸고 싶다" **유보적 의견 1건**: 키 구조 자체는 아니지만, **구조의 예외가 이미 2곳에 문서화 없이 존재한다** — ① `overlay-searchbar.html`·`overlay-highlight.html` 의 하드코딩 한국어(UXR-14) ② `eventviewer.html` 의 영어 고정 표현 — 상급 리뷰 반영으로 완화: 이것은 "명시적 결정"이 아니라 **근거가 문서화된 판단**이다(`event-viewer.md` §9 항목 3 — "기술 어휘라 번역하지 않는 편이 진단에 유리하다"고 한 판단이지만 **아직 미해결·확정 아님**). ①의 예외는 키 구조를 바꾸지 않고 **예외 목록을 F-14 문서에 명문화**(후속)하는 것으로 충분하다 — 구조 자체는 유지하며, **예외 명문화 대상은 ①(오버레이)뿐**이다.

---

## 3. 번역 품질 / 새 문자열 (2차 이후 · 계획 D7)

> 1차는 문자열 카탈로그를 건드리지 않는다(5개 파일·키 집합 일치 검사 회귀 장치). 여기 기록된 항목은 **2차(제품 개선) 이후**에만 반영한다.

**카탈로그 구조 이슈 (전 언어 공통)**

- `menu.check_for_updates` 와 `menu.check_for_updates.not_ready` 가 **5개 카탈로그 전부에 중복 정의**되어 있다(en.json 28·32행). **유래 (상급 리뷰 반영 — git 실측)**: `b84627c`(08-31 21:40)가 **"Automatic updates aren't available…"(현 둘째 정의)** 을 먼저 추가했고, `e1c597a`(23:33)가 **"Can't check for updates…"(현 첫 정의)** 를 나중에 중복 추가했다 — 즉 앞에 써 있는 첫 정의가 **더 늦게 추가된 문구**다. JSON 파서는 나중 값(둘째 정의)이 이기므로 런타임 표시 문구에는 안전하고, "첫 정의(잔재) 삭제" 방향 자체는 유지하되, 2차 정리 시 **어느 문구를 남길지** b84627c/e1c597a 의 의도를 근거로 재선택할 것을 명시한다(첫 정의가 나중에 추가된 의도 확인이 선행돼야 삭제를 확정할 수 있다). **중복 키 방지 검사**도 회귀 장치에 추가한다(2차 — `all_catalogs_*` 테스트 확장).

**ko 문체·용어 (UXR-16 과 짝 — 여기서는 실측 예시만)**

| 키 | 현재 ko | 문제 | 제안 방향 |
| :--- | :--- | :--- | :--- |
| `settings.seek.not_configured` | "…설정하라." | 명령조 — 다른 문구와 톤 불일치 | "…설정하세요." |
| `menu.launch_on_login.requires_approval` | "…직접 켜야 한다." | 한다체 서술 | "…직접 켜야 합니다." |
| `settings.korean.not_working.hint` | "…다시 시도한다." | 한다체 서술 | "…다시 시도해 주세요." |
| `settings.general.license.status.licensed.badge` | "라이선스됨" | "라이선스 활성"과 의미 중복·어색 | "활성" 또는 제거 |
| `settings.presets.group.other` | "그 밖에" | 표준 용어 아님 | "기타" |
| `settings.presets.option.nothing` | "아무것도 — 키를 끈다" | 팝업 선택지로 길어 잘릴 수 있음 | "끔 (키 비활성화)" 로 축약 검토 |
| `permissions.out_of_sync.manual_steps` | "{0} 을(를) 제거하세요" | 조사 괄호식이 노출 | 조사 처리는 문장형으로 완결("{0} 을 제거하세요") |

**새로 필요한 문구 (UXR-14 — 오버레이 카탈로그 이전)**

- `overlay.searchbar.no_matches` — 현재 하드코딩 `"없음"`(값은 5언어)
- `overlay.highlight.more_omitted` — 현재 하드코딩 `"+N개 더"`(위치 인자 포함: `"+{0}개 더"`)
- 두 창의 `showDiag` 진단 문구 — 개발자용이나 로그 성격(영어)으로 통일할지 카탈로그로 둘지 F-03 에서 결정
- `en.json` 기준 신규 키 추가 시 5개 카탈로그 동시 추가(키 집합 일치 강제)

**zh·es·ja** — 이번 리뷰에서는 키 집합 동일성·값 존재만 확인했다(전부 en 과 키 일치, 248 keys). **번역 품질 정조사는 2차 예비로 남긴다**(ko 심층만 1차 실측 — 체크리스트 "번역 품질 정조사(2차 예비)" 항목의 일부 완료).

---

## 4. 미해결 질문

(에이전트 실측으로 확정하지 못한 것 — 사실과 추정을 구분해)

| # | 질문 | 상태 |
| :--- | :--- | :--- |
| 1 | ~~요구사항 문서 §1 주석의 "en.json 251행·247 keys" 와, 이번 실측의 json 파서 결과(248 유니크 키, 중복 키 2건 때문에 출현 250)가 어긋났던 건~~ → **마감 (상급 리뷰 #3)**: 실측 **252행 · 250 출현 · 248 유니크 키**(5개 카탈로그 동일) 가 정본으로 확정됐고, §1 의 "251행·247 keys" 는 **재현 불가 오기** 였다(§1 주석이 이미 정정됨). 중복 키 2건 제거는 여전히 2차 몫(§3). | **확정 — 248 유니크 키가 정본** (§1 주석과 동일 · 1차 채움 방향 역전) |
| 2 | ~~라이트 테마에서 오버레이가 실제로 출하되는 팔레트 값 — Rust 쪽이 보내는 기본 팔레트 정의 위치를 이번 실측(HTML·명세)에서는 확정하지 못했다. HTML 폴백은 다크 단일이다~~ → **해소 (상급 리뷰 #1, UXR-11 재규정)**: 출고값 정본은 `crates/ultrakey-overlay/src/palette.rs:66-80`(라이트 팔레트, 이슈 #67 `bar_fg` 포함, 회귀 테스트 4종). 외관은 세션 시작 시 `is_dark_appearance()`(apps/ultrakey-app/src/seek.rs:144) 로 판정되고 세션 내 고정(overlay/session.rs:48,67) — HTML 폴백은 다크 단일이지만 Rust 가 항상 팔레트를 보낸다. | **확정 — palette.rs 값이 정본** (UXR-11 이 명세 따라잡기로 반영) |
| 3 | 온보딩 창이 `resizable: false`(tauri.conf.json — 520×400 고정)로 열리는 이유와, 설정 창(D5)과 달리 크기 영속이 없는 것이 의도인지. | (추정 — F-11 온보딩의 의도된 단순성일 가능성 높음) — 2차 판정 |
| 4 | 트레이 정상 메뉴에 권한 상태 표시가 없다(unauthorized menu → normal menu 전환만). 온보딩 폴링이 처리하므로 기능 결함은 아니나, **장기간 사용자가 권한이 풀렸을 때** 메뉴에서 알 방법이 없다. | (추정 — 원본도 유사하다는 점은 검증 안 함) — F-11/F-10 제품 결정 |
| 5 | `eventviewer.html` 의 "번역하지 않는다" 결의와, 오버레이의 하드코딩 한국어(UXR-14)가 같은 예외 성격인지 — **상급 리뷰 #5 반영으로 완화**: 전자는 "명시적 결정"이 아니라 명세 `event-viewer.md` §9 항목 3 의 **근거가 문서화된 판단**(미해결·확정 아님)이고, 후자(오버레이)는 완전히 묵시적이다. | 사실 — **예외 명문화 대상은 후자(오버레이)뿐**. 전자는 판단이므로 확정 시 별도 승격 |

---

## 5. 실행 명령 (이 문서를 채우는 방법)

> ⚠️ **1단계에서 확인된 사실 (2026-09-02, ultrakey-implement)**: `opencode run --agent <이름>` 은 **primary 에이전트 전용**이다 — 서브에이전트(`ultrakey-uxreview` 등)를 지목하면 *"is a subagent, not a primary agent. Falling back to default agent"* 경고 후 기본 에이전트로 떨어진다. 따라서 계획 issue-79 §6 2단계에 적힌 `opencode run --agent ultrakey-uxreview "…"` 는 **그대로 쓰면 동작하지 않는다**. 올바른 호출은 아래 ① 이다. (보조 확인: `opencode agent list` — 배치 직후 새 에이전트가 등재·권한 파싱되는지 확인하는 스캔. `ultrakey-uxreview (subagent)`·`edit deny`·`webfetch deny` 확인 완료.)

### ① 정본 — Task 도구로 서브에이전트 위임 (권장)

`edit: deny` 라 파일을 못 쓴다. 새 세션(워크트리 분리, AGENTS.md §2)을 열고 그 세션이 Task 도구로 `ultrakey-uxreview` 를 spawn → 에이전트가 본문을 최종 응답으로 반환 → 호출 세션이 이 문서에 작성·커밋한다.

```bash
opencode run --agent build "이 문서(docs/plan/issue-79-ux-requirements.md)의 §1 체크리스트를 따라 전수 리뷰해 달라고
Task 도구(subagent_type=ultrakey-uxreview)로 위임하고, 반환된 §2.1~§2.4·§3·§4 본문을 그대로 보고하라. 문서를 수정하지 마라."
```

### ② 우선순위 태깅 유의

리뷰 결과를 채울 때 §0.1 규칙(우선순위 ≠ 분류, P0 은 컨트롤 id·저장 키·카탈로그 키·invoke 배선 무변경 전제)을 따른다. 확정 전에 반영 우선순위를 뒤집지 않는다 — 상급 리뷰 + 호출 세션이 판정한다.

---

## 6. 확정 후 절차 (계획 D4·§9 리뷰 #5)

1. `ultrakey-uxreview` 가 §2~§4 를 채운다(위 ① 실행, **2단계 완료 조건**).
2. `ultrakey-review`(상급) 이 요구사항 문서를 리뷰 — "요구사항이 아닌 것(조사 노트)"이 섞였는지, 불변 제약(§2.4)을 건드렸는지 점검.
3. 반영 → **호출 세션 확정**.
4. ⭐ 계획 초안 `issue-79-ux-review-and-modernization.md` 의 **§6 4단계에 구현 순서 지시서를 보강**한다(리뷰 :#5 — 현재는 "요구사항 확정 후 갱신"에 걸려 있다). 이 보강도 2단계 완료 조건에 포함된다.
5. 3단계(명세 갱신 — P0~P1 만 `preferences-ui.md` 인라인 반영) → 4단계(1차 현대화 구현 `ultrakey-implement` 위임) → 5단계(P2 제품 개선·창 틀 교체는 별도 이슈). 이 계획의 §7 수용 기준(6탭 라이트/다크 일관성·인터랙션 표현·회귀 테스트 통과·i18n 키 집합 불변·사용자 "이전보다 현대적이다" 판정)이 4단계 완료 정의다.