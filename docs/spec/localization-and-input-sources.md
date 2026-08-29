# F-14 · 현지화와 키보드 입력 소스 독립성

> 한 줄 요약: (A) 앱이 번들하는 8개 로케일(영어 포함, 그중 3개 RTL)의 문자열 카탈로그·런타임 로케일 결정·RTL 레이아웃 규칙을 정의하고, (B) 모든 키 입력 판정을 물리 키코드 기준으로 통일해 QWERTZ·AZERTY·Dvorak·CJK 입력기에서도 깨지지 않도록 하는 원칙을 정의한다. 둘 다 나중에 손대면 UI·판정 로직을 전면 재작성해야 하는 횡단 관심사라 명세 단계에서 확정한다.
> 의존성: `F-07`(`key-remapping-engine.md`) — event tap 설치·중재·quick press 판정 메커니즘 자체. 본 문서는 그 엔진이 **무엇을 기준으로 판정해야 하는가**의 원칙만 제공한다. `F-09`(환경설정 창의 컨트롤 구조) — 본 문서가 정의하는 문자열·RTL 규칙이 실제로 배치되는 대상.
> 관련 명세: `F-08`(`presets.md`, 개별 프리셋의 동작 정의 — `home row`/`symbol row` 등 레이아웃 의존 프리셋의 **개별 동작**은 F-08 소관, 본 문서는 그 프리셋이 비-QWERTY 에서 "무엇을 의미하는가"의 판정 원칙만 제공) · `F-05`(`hyperkey.md`, modifier 합성 — 본 문서 §3.2 의 키코드 기반 판정 원칙을 그대로 따르는 선행 사례)

---

## 1. 개요

이 문서는 서로 다른 두 문제를 다루지만, 둘 다 같은 성격의 리스크를 공유한다 — **초기 설계에서 놓치면 나중에 UI·판정 로직을 전면 재작성해야 한다**는 점이다.

**(A) 현지화.** 조사(`superkey-inventory.md` §2.3)는 Sparkle 델타 appcast 의 `sparkle:deltaFromSparkleLocales="de,he,ar,el,ja,fa,uk"` 속성에서 앱이 번들하는 로케일을 확정했다. 즉 **de·he·ar·el·ja·fa·uk + en, 총 8개**이며, 이 중 **he·ar·fa 3개가 RTL** 이다. 이는 원본 앱이 "언젠가 다국어를 지원하겠다"가 아니라 **이미 8개 로케일로 배포 중인 실제 기능**이라는 뜻이다. 클론이 처음부터 문자열을 하드코딩하고 레이아웃을 LTR 전제로 짜면, RTL 3개 로케일 추가 시점에 오버레이·환경설정 창의 레이아웃 로직을 다시 설계해야 한다.

**(B) 키보드 입력 소스 독립성.** 조사(`superkey-inventory.md` §2.1, §6 관찰 3)는 원본이 **v1.51/v1.52 에서 세 건**을 "regardless of keyboard layout" 으로 수정한 이력을 확정했다 — 괄호 quick press, Seek 세미콜론, paste w/o formatting 리매핑. 세 건 모두 원인이 같다: **문자 기반 판정이 QWERTY 이외 레이아웃에서 깨진다.** 이는 F-07 이 구현할 이벤트 판정 엔진 전체에 적용되어야 하는 원칙이므로, 개별 프리셋(F-08)이나 엔진 자체(F-07)에 흩어 두지 않고 여기서 한 번에 못박는다.

두 주제를 하나의 문서로 묶은 이유는 조사 브리프가 명시한 대로 **"나중에 하면 전부 다시 만들어야 하는" 성격**을 공유하기 때문이다 — (A)는 UI 레이아웃 전면 재작업, (B)는 판정 로직 전면 재작업이라는 형태로 나타난다.

**범위 밖**: 개별 프리셋의 동작 정의(`F-08`), event tap·중재·quick press 판정 메커니즘 자체(`F-07` — 본 문서는 그 엔진이 따라야 할 판정 원칙만 제공), 환경설정 창의 컨트롤 구조(`F-09`).

---

## 2. 사용자 시나리오

1. **독일어 macOS 사용자** — 시스템 언어가 독일어인 사용자가 Superkey 를 처음 실행하면, 환경설정 창·메뉴바 메뉴·시스템 알림이 모두 독일어로 표시된다. 키 이름(`Caps Lock`, `Befehlstaste` 등)은 macOS 시스템 관례를 따르지만 modifier 기호(`⌃⌥⌘⇧`)는 번역되지 않고 그대로 표기된다.
2. **히브리어 사용자가 Seek 를 사용** — RTL 로케일에서 Seek 오버레이의 Spotlight 유사 검색 바가 우측 정렬로 표시되고, 검색 바에서 선택된 매치까지 잇는 연결선의 시작점도 그에 맞게 조정된다.
3. **아랍어 사용자가 환경설정 창을 연다** — `Presets` 탭의 좌측 키캡 일러스트(`caps lock`/`shift`/`delete`)와 우측 체크박스 목록의 좌우 배치가 미러링되어, 사이드바가 우측에, 컨트롤이 좌측에 온다.
4. **AZERTY 프랑스어 키보드 사용자** — `Quick press left or right shift to input corresponding: ( )` 을 사용할 때, 물리적으로 `Shift` 키 위치를 판정하는 로직은 keycode 기준이라 레이아웃과 무관하게 동작하고, 출력되는 문자 `(`/`)` 는 현재 AZERTY 입력 소스에서 실제로 그 문자를 낼 수 있는 조합을 찾아 합성한다.
5. **Dvorak 사용자** — `Caps lock + W A S D = ▲◀▼▶` 프리셋을 사용할 때, 판정은 물리적으로 QWERTY 배열의 W/A/S/D 위치에 해당하는 keycode(`kVK_ANSI_W` 등)를 기준으로 하므로, Dvorak 배열에서 그 물리 위치에 있는 실제 문자(`,`/`a`/`o`/`e` 등)와 무관하게 항상 같은 물리 키 4개가 방향키로 동작한다.
6. **한글 입력기로 문서를 작성하던 중** — 한글 조합이 진행 중(예: "ㄱ" 입력 후 조합 대기)인 상태에서 quick press caps lock 이 트리거되면, 조합 중인 텍스트가 깨지지 않도록 리매핑 개입이 유예된다(§3.2, 정책은 `(추정)`).
7. **세션 도중 입력 소스를 전환** — 사용자가 `⌘+Space`(또는 메뉴바)로 미국 영어 입력 소스에서 프랑스어(AZERTY) 입력 소스로 전환하면, 앱은 `kTISNotifySelectedKeyboardInputSourceChanged` 를 받아 캐시된 keycode↔문자 매핑을 즉시 무효화하고, 이후 발생하는 quick press 출력은 새 입력 소스 기준으로 재계산된다.

---

## 3. 동작 명세

### 3.1 (A) 로케일

#### 3.1.1 로케일 표

| 코드 | 언어 | 방향 | 출처 |
| :--- | :--- | :--- | :--- |
| `en` | 영어 | LTR | 기본 로케일 — 번들의 기준(base) 로케일이며 `sparkle:deltaFromSparkleLocales` 목록에는 나타나지 않는다(그 속성은 en 을 기준으로 한 **차이** 로케일만 나열하므로) |
| `de` | 독일어(Deutsch) | LTR | `superkey-inventory.md` §2.3, `sparkle:deltaFromSparkleLocales="de,he,ar,el,ja,fa,uk"` |
| `he` | 히브리어(עברית) | **RTL** | 상동 |
| `ar` | 아랍어(العربية) | **RTL** | 상동 |
| `el` | 그리스어(Ελληνικά) | LTR | 상동 |
| `ja` | 일본어(日本語) | LTR | 상동 |
| `fa` | 페르시아어(فارسی) | **RTL** | 상동 |
| `uk` | 우크라이나어(Українська) | LTR | 상동 |

#### 3.1.2 로케일 결정 순서

1. macOS 는 앱 번들의 `CFBundleLocalizations`(Info.plist 에 위 8개 코드를 선언)와 시스템 설정(`시스템 설정 > 일반 > 언어 및 지역`, 앱별 언어 재정의 포함)의 사용자 선호 언어 목록을 대조해 최적 로케일을 결정한다.
2. 네이티브 측은 이 결정 결과를 `NSBundle`/`NSLocale` 계열 API 로 읽어(§6) 문자열 카탈로그의 로케일 키를 선택한다.
3. 정확히 일치하는 지역 변형이 없으면(예: 시스템이 `de-CH`) 언어 코드만으로 폴백한다(`de`).
4. 번들 로케일 8종 어디에도 해당하지 않으면 `en` 으로 폴백한다.
5. 앱 내에 시스템 설정을 오버라이드하는 언어 선택 UI가 있는지는 조사로 확정되지 않았다 — §4, §9.
6. `(추정)` 로케일 값은 앱 시작 시 1회 결정되며, 실행 중 시스템 언어를 바꿔도 재시작 전까지 UI 에 반영되지 않을 가능성이 있다. 이는 macOS 네이티브 앱의 일반적 관례이나 조사 자료에 직접 근거는 없다 — §9.

#### 3.1.3 번역 대상 / 비대상 분류

| 항목 | 번역 대상 | 정책·근거 |
| :--- | :--- | :--- |
| 환경설정 창 라벨·버튼·툴팁(ⓘ)·체크박스 설명, 메뉴바 메뉴 항목, 시스템 알림 텍스트, 오류 메시지(`"Unable to initialize Superkey"` 등) | **예** | 표준 UI 문자열. 문자열 카탈로그의 1차 대상 |
| 앱·기능 고유 명칭(`Superkey`, `Seek`, `Hyperkey`, `hyper`/`meh`/`bleh`) | **아니오** | 브랜드·기능 고유명사. 조사 자료 어디에도 로케일별 명칭 변형의 근거가 없고, 랜딩 페이지·appcast 는 로케일 불문 항상 영어 원문을 쓴다 |
| modifier 기호(`⌃` `⌥` `⌘` `⇧`) | **아니오(고정)** | macOS 는 메뉴 단축키 표기 등에서 이 기호를 언어와 무관하게 동일한 유니코드 글리프로 쓰는 것이 시스템 관례다. 로케일별로 문자로 풀어 쓰지 않는다 |
| ⚠️ 물리 키 이름(`Caps Lock`, `Command`, `Option`, `Control`, `Shift`, `Delete`, `Tab`, `Return`/`Enter`, `Escape`, `Home`, `End`, `Space`) | **예 — 번역한다(정책 결정)** | macOS 자체가 시스템 설정의 키보드·단축키 편집기에서 이 명칭들을 완전히 현지화해 표시하는 것이 플랫폼 관례라는 것은 일반적으로 알려져 있으나, 이 조사 자료(§0 "웹 기반 조사, 앱 미설치")에는 macOS 가 로케일별로 실제 어떤 문자열을 쓰는지에 대한 직접 근거가 없다 `(추정)`. 사용자가 이미 자국어 macOS 환경에서 보는 이름과 앱 안의 이름을 일치시켜야 혼란이 없다는 것이 채택 근거다. 다만 정확한 로케일별 번역어 문자열을 앱이 자체 관리할지, macOS 가 제공하는 시스템 문자열을 그대로 재사용할지는 §9 미해결 질문 |
| 레이아웃 의존 프리셋 라벨(`home row`, `symbol row (A = !)`) | 설명 문구는 **예**, 산출되는 실제 심볼(`!` 등)은 **번역이 아니라 키보드 레이아웃 종속 계산** | 문구는 UI 텍스트이지만, 괄호 안 예시 문자는 §3.2.4 의 키코드 기반 산출 로직이 현재 입력 소스 기준으로 매번 다시 계산해야 하는 데이터다 |
| 숫자(`1000` 등 슬라이더 값) | 로케일 자릿수 표기 규칙 적용 가능 `(추정)` | 값이 3~4자리 정수뿐이라(조사 Q6, 눈금 8칸) 로케일 간 차이가 실질적으로 드러날 상황이 적다. 서양 아라비아 숫자(0-9)로 고정 표기하고 아랍어/페르시아어의 동양 아라비아 숫자(٠١٢…)로는 전환하지 않는 것을 기본 정책으로 한다 — 기술 설정값의 혼동을 막기 위한 흔한 실무 관례 `(추정)` |
| 단위(`ms`) | **아니오** | 기술 단위 기호는 SI 표기를 따르며 로케일 번역 대상이 아니다 |

#### 3.1.4 RTL 레이아웃 미러링 규칙

RTL 로케일(`he`/`ar`/`fa`) 활성 시:

- **환경설정 창**: 좌측 사이드바(탭 4개) ↔ 우측 패널 배치가 좌우 반전된다. 각 패널 내 "체크박스 + 팝업" 행의 좌우 순서도 반전되어, 라벨 텍스트가 우측 정렬로 시작한다.
- **`Presets` 탭 키캡 일러스트**: `caps lock`/`shift`/`delete` 그룹 구분용 일러스트의 화면상 위치가 미러링된다. 단, modifier 기호 표기 순서(`⌃⌥⌘⇧`)는 §3.1.3 에 따라 언어·방향과 무관하게 macOS 표준 순서를 유지한다 — 아이콘의 **배치**는 미러링하되 기호 **조합 표기 순서**는 고정한다.
- **Seek 오버레이 검색 바**: Spotlight 유사 검색 바 내부 텍스트 입력·커서 시작 위치가 RTL 로 전환된다. 검색 바에서 선택된 매치까지 잇는 연결선(`superkey-inventory.md` §4.5 확정 사실)의 기준점은 여전히 검색 바의 화면상 실제 좌표이므로, 검색 바 자체가 화면의 어느 쪽에 배치되든 선은 그 실제 위치에서 그려진다 — 미러링은 검색 바 **내부 요소**의 정렬에 적용되고, 화면 좌표계 자체는 절대 좌표라 미러링 대상이 아니다.
- 이 규칙들의 정확한 세부(정렬 값, 여백 반전 여부 등)는 실제 UI 구현체(`F-09`, `F-03`)가 결정하며, 본 문서는 "무엇이 미러링 대상인가"라는 분류 원칙만 제공한다.

#### 3.1.5 문자열 소스 통합 — Tauri(WebView) ↔ 네이티브

원본 조사에는 없는 이 클론 고유의 아키텍처 문제다. Superkey 클론은 환경설정 창·Seek 오버레이를 Tauri(WKWebView)로 그리고, 메뉴바 메뉴·시스템 알림은 네이티브(AppKit) API 로 그린다. 두 렌더링 경로가 각자 별도의 문자열 카탈로그(예: 네이티브는 `.strings`, 웹 쪽은 JS i18n JSON)를 가지면, 로케일 하나에 대해 어느 한쪽에만 문자열이 추가되는 **분기(drift)**가 발생하기 쉽다 — 특히 §3.1의 번역 범위 자체가 확정되지 않은 상태(§9)에서는 이 위험이 더 크다.

**결정: 단일 로케일별 문자열 카탈로그 파일(로케일 코드 8개 × 파일 1개, 키-값 형식)을 앱 리소스로 하나만 두고, 네이티브 Rust 코드와 WebView 양쪽이 같은 파일을 읽는다.**

- 네이티브 측(트레이 메뉴, 알림): Rust 백엔드가 앱 시작 시 해당 로케일 파일을 표준 직렬화 방식으로 파싱해 메모리에 올리고, 메뉴 아이템·알림 문자열 생성 시 조회한다. 이는 플랫폼 API 호출이 아니라 순수 파일 I/O + 파싱이므로 §6/§7 의 3분류 판정 대상이 아니다.
- WebView 측: Tauri 의 정적 리소스 서빙(또는 IPC 커맨드)으로 같은 파일을 JS 에서 `fetch` 해 동일한 키로 조회한다.
- **근거**: 카탈로그가 두 벌이면 "번역 누락 시 어느 쪽만 en 으로 남는가"라는 실패 모드(§5)가 검증·테스트 대상에서 두 배로 늘어난다. 하나의 파일을 두 소비자가 읽는 구조는 키 목록의 완전성(coverage)을 한 곳에서만 검증하면 되므로, drift 위험을 구조적으로 없앤다.
- 로케일 결정(§3.1.2)은 네이티브 측에서 1회 계산해 WebView 초기화 시 전달하는 것으로 한다 — 두 쪽이 각자 로케일을 따로 판정하면 판정 로직 자체가 갈릴 위험이 있다.

### 3.2 (B) 입력 판정 · 출력 생성 규칙

#### 3.2.1 원칙

> **입력 판정은 항상 물리 키코드(`CGKeyCode`/virtual keycode)로 한다.** 문자로 판정하면 QWERTZ·AZERTY·Dvorak, 그리고 한글/일본어 입력기 활성 시 판정 자체가 성립하지 않거나 엉뚱한 키에서 트리거된다.
> **출력은 상황에 따라 문자여야 할 수도, 물리 키+modifier 조합(단축키)이어야 할 수도 있다.** 어느 쪽이든 "현재 입력 소스에서 무엇을 누르면 이 결과가 나오는가"를 매번 다시 계산해야 하며, 하드코딩된 US-QWERTY 매핑을 쓰면 안 된다.

이 원칙은 조사에서 확정된 3건의 실제 회귀(v1.51 ×2, v1.52 ×1, `superkey-inventory.md` §2.1)에서 직접 도출됐으며, F-07 이 구현하는 event tap 판정 로직 전체가 이를 준수해야 한다.

#### 3.2.2 (i) vs (ii) — 문자 출력을 만드는 두 방식

| 방식 | 메커니즘 | 장점 | 함정 |
| :--- | :--- | :--- | :--- |
| **(i) `UCKeyTranslate` 역방향 탐색** | 현재 입력 소스의 키보드 레이아웃 데이터(`kTISPropertyUnicodeKeyLayoutData`)를 이용해, 가능한 모든 (keycode, modifier) 조합에 대해 `UCKeyTranslate` 를 정방향 실행하고 목표 문자와 일치하는 조합을 찾아 캐싱한 역방향 테이블을 만든다. 그 keycode+modifier 로 합성 `keyDown`/`keyUp` `CGEvent` 를 생성해 전송한다 | 수신 앱이 실제 물리 입력과 구분할 수 없는 "진짜" 키 이벤트라, **단축키로 해석되어야 하는 출력**(예: `⌘⌥⇧V`)에도 쓸 수 있고 텍스트 서비스(TSM) 파이프라인에도 자연스럽게 들어간다 | 현재 레이아웃에 그 문자를 낼 수 있는 조합이 아예 없으면(일부 완성형 IME·극단적 배열) 실패한다. 역방향 테이블 계산 비용이 있고, 입력 소스가 바뀔 때마다 재계산·캐시 무효화가 필요하다(§3.2.5) |
| **(ii) `CGEventKeyboardSetUnicodeString`** | 임의의 유니코드 문자열을 이벤트에 물리 keycode·modifier 없이 직접 실어 보낸다 | 레이아웃에 존재하지 않는 문자도 낼 수 있는 **레이아웃 완전 독립** 방식. 구현이 단순하다 | 이벤트의 keycode 필드는 실제 키를 반영하지 않는 임의값이라, **수신 측이 keycode/modifier 조합으로 단축키를 인식하는 경우 트리거되지 않을 수 있다.** 순수 텍스트 삽입에는 적합하지만 modifier 조합 형태의 "단축키" 출력에는 부적합 |

**채택 원칙**: 출력이 **문자 그 자체**(`(`, `)`, `/`)라면 (i)를 우선 시도하고, 현재 레이아웃에서 그 문자를 만드는 조합을 찾지 못하면 (ii)로 폴백한다(§5 엣지 케이스). 출력이 **동일 키에 다른 modifier 를 얹은 단축키 합성**(예: paste → paste w/o formatting)이라면, 애초에 역방향 탐색이 필요 없다 — 이미 알고 있는 물리 키(예: `V`, `kVK_ANSI_V`)에 `CGEventSetFlags` 로 목표 modifier 비트를 추가해 재주입하면 되며, 이는 `F-05`(Hyperkey)의 modifier 합성 방식과 동일한 패턴이다. 즉 (ii)는 **단축키 출력에는 원칙적으로 쓰지 않는다.**

#### 3.2.3 입력 판정·출력 생성 규칙 표

| 상황 | 키코드로 판정 | 문자로 출력? | 사용 API | 근거 |
| :--- | :--- | :--- | :--- | :--- |
| Seek: `;` 로 다음 매치 선택 | `kVK_ANSI_Semicolon`(0x29) 물리 keycode | 아니오 — 내부 커맨드 트리거, 문자 산출 없음 | `CGEventGetIntegerValueField(kCGKeyboardEventKeycode)` | `superkey-inventory.md` §2.1 v1.51 "regardless of keyboard layout" |
| quick press shift → 괄호 실행(`( )`) | `kVK_Shift`(좌 0x38 / 우 0x3C) + quick press 판정(F-07 소관, 지속시간은 `Quick press duration` 값) | 예 — `(` 또는 `)` | §3.2.2 (i) 채택, 실패 시 (ii) 폴백. `UCKeyTranslate` + `TISCopyCurrentKeyboardInputSource` | 조사 v1.51 "input the correct character, regardless of keyboard layout" |
| quick press caps lock → `/` 실행(v1.62 추가) | `kVK_CapsLock`(0x39) + quick press 판정 | 예 — `/` | §3.2.2 (i) 채택, 실패 시 (ii) 폴백 | appcast v1.62 "can now execute a slash keypress" |
| remap paste → paste w/o formatting(`⌘⌥⇧V`) | `⌘`(Command) + `kVK_ANSI_V`(0x09) 조합의 keyDown | 아니오 — 단축키 합성. 동일 keycode `V` 에 modifier 비트만 추가 | `CGEventSetFlags` 로 Option+Shift 비트를 추가해 동일 keycode 로 재주입(`F-05` 패턴과 동일) | 조사 v1.52 "regardless of keyboard layout". 단축키는 keycode+flags 로 인식되므로 (ii) Unicode 직접 주입은 부적합(§3.2.2) |
| `Caps lock + W A S D = ▲◀▼▶` | `kVK_ANSI_W`/`A`/`S`/`D`(물리 키보드의 고정된 4개 위치, US 배열 기준 keycode 값 자체를 물리 위치 식별자로 사용) | 아니오 — 방향키(`kVK_UpArrow` 등) 합성 | `CGEventGetIntegerValueField` 판정 + `CGEventCreateKeyboardEvent` 로 방향키 keycode 합성 | F-08 소관(개별 동작)이나 판정 원칙은 본 문서 소관. Dvorak 등에서 "W/A/S/D 문자"가 아니라 "그 물리 위치"가 방향키가 됨(§2 시나리오 5) |
| `Caps lock + home row = symbol row (A = !)` | home row 물리 keycode 집합(`kVK_ANSI_A/S/D/F/G/H/J/K/L/Semicolon` 등, US 배열 기준 물리 위치) | 예 — 현재 입력 소스에서 대응 심볼 산출 | `UCKeyTranslate` 로 "숫자 줄 + shift" 에 해당하는 문자를 **현재 레이아웃 기준으로** 조회 (`A = !` 는 US-QWERTY 한정 예시) | F-08 소관(개별 매핑표)이나, 비-QWERTY 에서 "symbol row" 자체가 무엇을 의미하는지는 §3.2.4 |

#### 3.2.4 `home row`/`symbol row` 프리셋의 레이아웃 의존성

`Caps lock + home row = symbol row (A = !)` (조사 §3.3)는 US-QWERTY 를 전제로 "숫자 줄 + Shift = 기호"라는 매핑을 홈 로우 각 키에 대응시킨 것이다. 이 프리셋을 비-QWERTY 에서 그대로 재현하려면 두 층위를 분리해야 한다.

1. **"home row" 의 정의는 물리 위치다.** 즉 keycode 집합(§3.2.3 표)으로 고정되며, 어떤 레이아웃에서도 항상 같은 물리 키 10개(또는 그 이하)를 가리킨다. 이 층위는 레이아웃 독립적이다.
2. **"symbol row" 가 산출하는 실제 문자는 레이아웃 종속이다.** US-QWERTY 에서는 숫자 줄 + Shift 가 `!@#$%^&*()` 를 낸다는 전제(`A = !`) 자체가, AZERTY 에서는 성립하지 않는다 — AZERTY 는 숫자 줄이 Shift **없이** 기호를 내고 Shift 를 눌러야 숫자가 나오는 반전된 배치다. 따라서 "symbol row" 가 무엇을 의미하는지는 하드코딩된 US 심볼 표를 재사용하면 안 되고, **현재 입력 소스에 대해 매번 `UCKeyTranslate` 로 다시 계산**해야 한다.
3. 계산 결과가 레이아웃마다 다르므로, 프리셋 UI(F-09)가 인라인 팝업(조사 Q7)에 보여주는 예시 문자(`A = !` 같은)도 로케일·입력 소스에 따라 달라져야 한다는 것이 이 문서의 판정이다.

#### 3.2.5 입력 소스 변경 감지·무효화 절차

1. 앱 시작 시 `TISCopyCurrentKeyboardInputSource()` 로 현재 입력 소스를 얻고, `TISGetInputSourceProperty(kTISPropertyUnicodeKeyLayoutData)` 로 레이아웃 데이터를 읽어 (a) 정방향 `UCKeyTranslate` 조회 캐시와 (b) §3.2.2 (i) 가 쓰는 역방향(문자→keycode+modifier) 캐시 테이블을 구축한다.
2. `CFNotificationCenterGetDistributedCenter()` 에 `kTISNotifySelectedKeyboardInputSourceChanged` 옵저버를 등록한다.
3. 알림 수신 시: (a) 새 입력 소스를 `TISCopyCurrentKeyboardInputSource()` 로 다시 얻고, (b) 1의 두 캐시 테이블을 **즉시 폐기하고 재구축**한다(지연 재구축이 아니라 즉시 — 폐기와 재구축 사이에 quick press 등이 트리거되면 stale 매핑으로 잘못된 문자가 출력될 위험이 있다).
4. 재구축이 완료되기 전에 §3.2.3 표의 문자 출력이 필요한 이벤트가 도착하면, 재구축 완료까지 대기하거나(짧은 지연 허용) 해당 1회는 무시한다 — 어느 쪽을 택할지는 §9 미해결 질문.
5. CJK 계열 "입력 방식(input method)" 로케일(한글·日本語·中文 등)로 전환된 경우, §3.2.6 의 조합 중 정책이 함께 활성화된다.

#### 3.2.6 CJK 입력기 활성 중의 동작 `(추정)`

조사 자료에는 이 상황에 대한 직접 근거가 없다. 한글/일본어/중국어 입력기가 조합 중(marked text 존재)일 때 리매핑이 개입하면 조합이 깨진다는 것은 일반적으로 알려진 문제이나(예: 세미콜론이 특정 IME 의 조합 키로 쓰이는 경우, 그 키를 Seek 판정이 먼저 가로채면 조합이 끊긴다), 정확한 감지 방법과 정책은 실측이 필요하다.

**채택 정책(추정)**: 현재 입력 소스가 "키보드 입력 방식(Input Method)" 범주(한글 2벌식/3벌식, 日本語 かな入力/ローマ字入力, 中文 拼音 등)이고 **marked text(조합 중 미확정 텍스트)가 존재하는 것으로 판단되면**, 문자를 새로 산출·출력해야 하는 리매핑(quick press 괄호/슬래시 실행, `symbol row` 등)은 해당 keyDown 을 소비하지 않고 원본 그대로 통과시킨다. 반대로 문자를 산출하지 않는 리매핑(Hyperkey modifier 합성, Seek 활성화 자체, 방향키 프리셋)은 조합 여부와 무관하게 계속 동작한다 — 이들은 IME 의 조합 버퍼에 개입하지 않기 때문이다.

- 감지 방법은 미정 `(추정)`. 후보: 텍스트 서비스 관리(TSM) 계열 API 로 활성 문서의 조합 상태를 조회하거나, 포커스된 요소의 Accessibility 속성(`kAXSelectedTextRangeAttribute` 등 marked text 관련)을 확인하는 방식이 있으나, 조사 노트(`rust-macos-capability-notes.md`)에는 이 용도의 크레이트나 API 목록이 없다.
- 이 정책은 §9 미해결 질문으로 승계하며, 실제 구현 전 앱 설치 후 한글 입력기로 실측 검증이 필요하다.

---

## 4. 설정 항목

| # | 항목 | 존재 여부 | 비고 |
| :--- | :--- | :--- | :--- |
| 1 | 앱 내 언어 선택(시스템 설정을 오버라이드) | **미확정** `(추정 — 없을 가능성이 높음)` | 조사된 `General` 탭 스크린샷이 공개되지 않아(`superkey-inventory.md` §3.4, Q2) 확정 불가. 동종 macOS 유틸리티 관례상 시스템 언어를 그대로 따르고 별도 언어 선택 UI를 두지 않는 경우가 흔하다는 것이 추정 근거이나 직접 증거는 없다 → §9 |
| 2 | 입력 소스 변경에 대한 사용자 노출 설정(알림·로그 등) | **없음** `(추정)` | §3.2.5 절차는 백그라운드에서 투명하게 동작하는 것으로 설계한다. 조사 자료에 이런 설정 항목의 근거가 없다 |
| 3 | RTL 강제/해제 토글(시스템 로케일 방향과 무관하게) | **없음** `(추정)` | RTL 여부는 §3.1.2 로케일 결정 결과에 종속되며, 별도 스위치가 있다는 근거가 없다 |
| 4 | `Caps lock + home row = symbol row` 인라인 팝업의 대안 목록 | **일부만 확인** | `symbol row (A = !)` 외 대안 존재 여부는 조사 Q7 로 미확정 |
| 5 | `Quick press left or right shift` 출력 후보 전체 | **일부만 확인** | `( )` 외 `[ ]`, `{ }` 등 존재 여부는 조사 Q8 로 미확정 |

---

## 5. 엣지 케이스와 실패 모드

1. **RTL 에서 Seek 오버레이 연결선 방향** — 검색 바가 화면 우측에 미러링 배치된 상태에서, 검색 바 → 매치 하이라이트로 잇는 연결선의 시작점 계산이 LTR 전제로 하드코딩되어 있으면 엉뚱한 방향에서 선이 시작된다.
2. **RTL 에서 키캡 일러스트 좌우 배치** — `Presets`/`Hyperkey` 탭의 키캡 일러스트가 미러링 대상인지, 아니면 물리 키보드 그림이라 항상 고정(물리 키보드 자체는 RTL 이 아니므로) 이어야 하는지 상충하는 두 직관이 있다 — §3.1.4 는 배치는 미러링, 기호 순서는 고정으로 정했으나 일러스트 자체의 좌우 반전 여부는 실제 그래픽 자산 기준으로 재확인 필요.
3. **긴 독일어 라벨로 인한 레이아웃 파손** — `Only show while the remapped key is held` 류의 문장형 라벨이 독일어에서 원문보다 길어지면, 체크박스와 팝업 버튼이 같은 행에 있는 UI(조사 §3.1)에서 줄바꿈되거나 팝업이 화면 밖으로 밀릴 수 있다.
4. **Dvorak 에서 `WASD` 프리셋** — §3.2.3 판정대로 물리 위치 기준이면 Dvorak 사용자에게는 그 4개 키가 `,`/`a`/`o`/`e` 로 보여 "W/A/S/D" 라는 라벨과 실제 키가 불일치한다. 라벨을 현재 레이아웃에 맞춰 동적으로 다시 표기해야 하는지는 UI(F-09) 결정 사항.
5. **AZERTY 에서 `symbol row` 프리셋** — §3.2.4 에서 다룬 대로, US 하드코딩 심볼 표(`A = !`)를 그대로 쓰면 AZERTY 의 실제 숫자/기호 배치(Shift 관계가 반전됨)와 맞지 않아 엉뚱한 문자가 출력된다.
6. **한글 입력기 조합 중** — §3.2.6 정책이 없으면 조합 중인 텍스트가 리매핑 개입으로 깨진다.
7. **일본어 IME 변환 확정 대기 중** — 변환 후보가 표시된 상태에서 quick press 가 개입하면 변환이 취소되거나 의도치 않은 후보가 확정될 수 있다. §3.2.6 과 동일 계열이나 IME 종류별 조합 상태 판별 방식이 다를 수 있다.
8. **세션 중 입력 소스 전환** — §3.2.5 절차가 없으면, US 배열로 캐싱된 역방향 매핑 테이블을 그대로 쓴 채 AZERTY 로 전환된 상태에서 quick press 를 실행해 잘못된 문자가 출력된다.
9. **외장 키보드가 다른 물리 레이아웃(JIS 외장 키보드 + ANSI 시스템 설정 등)** — keycode 자체가 물리적으로 다른 배열을 가리킬 수 있어, "물리 위치 기준 판정"이라는 원칙이 흔들릴 수 있는 유일한 경우다. macOS 가 이를 어떻게 정규화하는지는 조사 범위 밖.
10. **`symbol row` 개념이 없는 레이아웃** — 완성형 CJK 입력 방식처럼 "숫자 줄 + Shift = 기호"라는 US 관례 자체가 성립하지 않는 입력 소스에서, 이 프리셋 UI 를 그대로 노출할지 숨길지 미정.
11. **번역 누락 시 폴백** — 특정 로케일의 문자열 카탈로그에 일부 키가 비어 있으면(§9, 번역 범위 미확정) `en` 문자열로 개별 폴백되어, 한 화면 안에 두 언어가 섞여 보이는 상태가 될 수 있다.
12. **`UCKeyTranslate` 역방향 탐색 실패** — 현재 레이아웃에 목표 문자(`(` 등)를 낼 수 있는 keycode+modifier 조합이 전혀 없는 경우, §3.2.2 의 (ii) 폴백이 필요하다. 이 폴백이 없으면 quick press 자체가 조용히 무동작한다.
13. **RTL 로케일에서 숫자·단위 표기의 bidi 혼합** — `1000 ms` 같은 LTR 숫자+단위 문자열이 RTL 문장 흐름 안에 놓이면, 양방향(bidi) 알고리즘이 순서를 뒤섞을 수 있어 격리 처리(예: bidi isolate 문자)가 필요할 수 있다.

---

## 6. 필요한 플랫폼 API

`rust-macos-capability-notes.md` 기준. F-14 는 §2.1(`CGEventTap`)·§2.5(TCC)의 기존 판정을 재사용하고, 이 문서에서 새로 필요한 API 는 아래와 같다.

| API | 용도 | 비고 |
| :--- | :--- | :--- |
| `TISCopyCurrentKeyboardInputSource` | 현재 활성 입력 소스 조회 | Carbon `HIToolbox` 소속 |
| `TISGetInputSourceProperty`(`kTISPropertyUnicodeKeyLayoutData`, `kTISPropertyInputSourceID`, `kTISPropertyInputSourceType`) | 레이아웃 데이터 및 입력 소스 종류(키보드 배열 vs 입력 방식/IME) 조회 | 상동. §3.2.6 CJK IME 판별에도 사용 |
| `kTISNotifySelectedKeyboardInputSourceChanged` | 입력 소스 변경 알림 | `CFNotificationCenterGetDistributedCenter()` 에 옵저버 등록 |
| `UCKeyTranslate` | keycode+modifier → 문자 정방향 변환. §3.2.2 (i) 의 역방향 탐색 테이블을 만드는 기반 | Carbon `HIToolbox` 소속 |
| `CGEventGetIntegerValueField`(`kCGKeyboardEventKeycode`) | 물리 keycode 판정(§3.2 원칙의 핵심) | Core Graphics — F-07 이 이미 설치한 event tap 콜백 안에서 호출 |
| `CGEventCreateKeyboardEvent` / `CGEventSetFlags` | 합성 keyDown/keyUp 이벤트 생성, modifier 비트 추가(§3.2.3 paste 사례) | Core Graphics |
| `CGEventKeyboardSetUnicodeString` | §3.2.2 (ii) — 레이아웃 독립 문자 직접 주입(폴백 경로) | Core Graphics |
| `NSBundle`/`NSLocale`(`preferredLocalizations`, `CFBundleLocalizations`) | §3.1.2 로케일 결정 | Foundation |
| `NSApplication.userInterfaceLayoutDirection` / `NSLocale.characterDirection(forLanguage:)` | RTL 여부 판정(§3.1.4) | AppKit / Foundation |
| CJK 조합 상태(marked text) 조회 | §3.2.6 정책의 감지 메커니즘 | 조사 노트에 대응 크레이트·API 목록 없음. 후보만 있고 확정 API 미상 — §9 |

---

## 7. 구현 접근

이 문서의 판정은 아래 3분류를 기준으로 한다(다른 F-0N 문서들과 동일한 척도).

- **순수 Rust** — 안전(safe) 래퍼 크레이트만으로 커버되어 `unsafe` 도 네이티브 소스도 없음
- **Rust 바인딩** — `unsafe` FFI 직접 호출이 필요하다. `objc2-*` 계열 헤더 자동생성 바인딩을 직접 호출하거나, 대응 크레이트가 없으면 `extern "C"` 선언을 손으로 작성해 프레임워크에 직접 링크한다. 어느 쪽이든 Swift/Objective-C 로 별도 컴파일 산출물(shim 바이너리)을 두지는 않는다
- **네이티브 shim 불가피** — Rust 에서 호출할 방법이 없어 Swift/Objective-C 소스를 별도 컴파일해 링크해야 함

### (A) 현지화 — 판정: **Rust 바인딩**

- 로케일 결정(§3.1.2)과 RTL 판정(§3.1.4)에 필요한 `NSBundle`/`NSLocale`/`NSApplication.userInterfaceLayoutDirection` 은 `objc2-foundation` 0.3.2, `objc2-app-kit` 0.3.2 로 노출된다(`rust-macos-capability-notes.md` §1.1). 다만 이 크레이트들은 "헤더 자동생성 바인딩이며 안전한 상위 래퍼는 없고 호출부는 `unsafe`"라고 조사 노트 §1.1 이 명시하므로 `순수 Rust` 가 아니라 `Rust 바인딩` 이다.
- 문자열 카탈로그 자체(§3.1.5 의 파일 파싱·조회)는 플랫폼 API 호출이 아니라 순수 Rust 로직(직렬화 포맷 파싱)이라 3분류 판정 대상 밖이다.
- 기각한 대안: 문자열 카탈로그를 네이티브 `.strings`(macOS 표준 현지화 포맷)로 관리 — macOS 관례에는 맞지만, §3.1.5 의 "단일 소스" 결정과 정면으로 충돌한다. WebView 쪽이 `.strings` 를 직접 파싱할 표준 경로가 없어(웹 기술 스택과 이질적), 결국 빌드 시 JSON 으로 변환하는 이중 관리가 되어 drift 위험이 되살아난다. 기각.

### (B) 키보드 입력 소스 독립성 — 판정: **Rust 바인딩**

- `CGEventTap`·`CGEventGetIntegerValueField`·`CGEventSetFlags`·`CGEventCreateKeyboardEvent`·`CGEventKeyboardSetUnicodeString` 은 `core-graphics` 0.25.0(안전 래퍼) 또는 `objc2-core-graphics` 0.3.2(헤더 자동생성)로 커버된다(조사 노트 §1.1, §1.2, §2.1). `core-graphics` 가 `CGEventTap` 안전 래퍼를 이미 제공한다는 점에서 이 부분만 보면 `순수 Rust` 에 가깝지만, `CGEventKeyboardSetUnicodeString` 처럼 조사 노트가 명시적으로 나열하지 않은 개별 함수는 `objc2-core-graphics` 의 헤더 자동생성 경로(`unsafe` 수준)로 보완해야 할 가능성이 있어, 전체를 `Rust 바인딩` 으로 판정한다.
- ⭐ **`TISCopyCurrentKeyboardInputSource`/`TISGetInputSourceProperty`/`kTISNotifySelectedKeyboardInputSourceChanged`/`UCKeyTranslate` 는 Carbon `HIToolbox` 소속이며, `rust-macos-capability-notes.md` 의 크레이트 표(§1.1, §1.2) 어디에도 `HIToolbox`/`Carbon`/`TIS`/`UCKeyTranslate` 를 다루는 크레이트가 등장하지 않는다.** 정직하게 말하면: **전용 크레이트 미확인, `extern "C"` 직접 선언 필요 `(추정)`.** `Carbon.framework`(또는 그 하위 `HIToolbox.framework`)에 `#[link(name = "Carbon", kind = "framework")]` 로 직접 링크하고, 필요한 함수 시그니처를 손으로 선언하는 접근이 될 것으로 본다. 이는 여전히 "Swift/ObjC shim 바이너리"가 아니라 Rust 안에서의 `unsafe extern` 선언이므로 `Rust 바인딩` 으로 분류하되, 같은 분류 안에서도 "크레이트가 있어 감싸기만 하면 되는 경우"보다 비용이 크다는 점을 §9 에 남긴다.
- §3.2.6 의 CJK 조합 상태 조회 API 는 조사 노트에 아예 언급이 없어 이 판정 자체를 내릴 수 없다 — §9 미해결 질문.
- 기각한 대안:
  - **`rdev` 0.5.3** — 조사 노트가 "3년간 릴리스 없음, 이벤트 소비 제어 제한적"으로 명시(§1.2). §3.2.3 의 정밀한 이벤트 소비·치환·flag 조작 요구를 충족하지 못한다.
  - **문자 기반 판정으로 되돌아가기(비교 대안)** — 구현이 단순해 보이지만, 조사에서 확정된 v1.51/v1.52 세 건의 회귀를 그대로 재현하는 접근이라 원천적으로 기각. §3.2.1 원칙과 정면으로 배치된다.
  - **네이티브 Swift shim 으로 `HIToolbox` 호출부만 분리** — Carbon API 자체는 C ABI 라 Rust `extern "C"` 로 직접 호출 가능하고 별도 프로세스 경계나 Swift 런타임이 필요 없다. shim 을 두면 오히려 IPC 오버헤드와 배포 복잡도만 늘어나 기각.

---

## 8. 수용 기준

- [ ] 앱이 시스템 언어를 8개 번들 로케일(`en`, `de`, `he`, `ar`, `el`, `ja`, `fa`, `uk`) 중 하나로 정확히 매핑해 실행하며, 번들에 없는 언어는 `en` 으로 폴백한다.
- [ ] `he`/`ar`/`fa` 로케일에서 환경설정 창의 사이드바·패널 좌우 배치가 미러링되고, modifier 기호(`⌃⌥⌘⇧`) 표기 순서는 로케일과 무관하게 고정된다.
- [ ] Seek 오버레이 검색 바가 RTL 로케일에서 우측 정렬로 표시되며, 검색 바 → 매치 연결선은 화면 절대 좌표 기준으로 정확히 그려진다.
- [ ] 메뉴바 메뉴와 시스템 알림, 환경설정 창(WebView)의 문자열이 동일한 로케일 카탈로그 파일 하나에서 나오며, 두 경로 사이에 번역 키 누락 차이가 없다.
- [ ] `Quick press left or right shift to input corresponding: ( )` 이 AZERTY·Dvorak·QWERTZ 각 입력 소스에서 물리 keycode 기준으로 동일하게 트리거되고, 출력 문자는 각 입력 소스에서 실제로 `(`/`)` 로 나타난다.
- [ ] Seek 에서 세미콜론으로 다음 매치를 선택하는 동작이 물리 `kVK_ANSI_Semicolon` keycode 기준으로 판정되어, 해당 위치에 다른 문자가 배정된 레이아웃(AZERTY 등)에서도 동일한 물리 키로 트리거된다.
- [ ] `Remap paste to paste w/o formatting` 이 keycode+modifier 조합(`⌘V` 감지, `⌘⌥⇧V` 합성)으로 구현되어 있으며, 이 합성 이벤트가 대상 앱에서 실제 단축키로 인식된다(`CGEventKeyboardSetUnicodeString` 을 쓰지 않는다).
- [ ] `UCKeyTranslate` 역방향 탐색으로 목표 문자를 낼 수 없는 레이아웃에서, quick press 출력이 조용히 무동작하지 않고 `CGEventKeyboardSetUnicodeString` 폴백으로 대체된다.
- [ ] `kTISNotifySelectedKeyboardInputSourceChanged` 수신 시 캐시된 keycode↔문자 매핑이 즉시 무효화·재구축되며, 재구축 완료 전 도착한 quick press 이벤트가 stale 매핑으로 잘못된 문자를 출력하지 않는다.
- [ ] `Caps lock + W A S D = ▲◀▼▶` 프리셋이 Dvorak 배열에서도 물리적으로 같은 4개 키 위치에서 트리거된다.
- [ ] `home row = symbol row` 프리셋이 산출하는 실제 심볼이 US-QWERTY 하드코딩 표가 아니라 현재 입력 소스 기준으로 계산된다.
- [ ] 한글 또는 일본어 입력기 조합 중(marked text 존재) 상태에서 문자 출력형 리매핑이 개입해 조합을 깨뜨리지 않는다(§3.2.6 정책 검증).

---

## 9. 미해결 질문

1. **번역 범위** — 로케일 목록(8개)은 확정이지만, 각 로케일에서 전체 UI 가 번역되는지 일부만 번역되는지는 조사로 확인 불가. §3.1.3 의 대상/비대상 분류는 이 문서의 정책 제안이며 실제 번역 완성도와 다를 수 있다.
2. **키 이름 번역 정책의 근거 보강** — §3.1.3 에서 "물리 키 이름을 번역한다"고 정책 결정했으나, macOS 가 실제로 로케일별 어떤 문자열을 쓰는지(예: 독일어 키 이름의 정확한 표기)는 조사 자료에 근거가 없는 `(추정)`이다. 앱 설치 후 macOS 시스템 설정의 실제 로케일별 문자열을 확인해 보강 필요.
3. **앱 내 언어 선택 UI 존재 여부** — `General` 탭 스크린샷 미공개로 확정 불가(조사 Q2). 있다면 §3.1.2 로케일 결정 순서에 최우선 단계로 추가되어야 한다.
4. **로케일 변경의 실행 중 반영 여부** — 시스템 언어를 실행 중 바꿨을 때 재시작 없이 UI 에 반영되는지 `(추정)` 상태이며 확정 필요.
5. **CJK 조합 상태 감지 API** — §3.2.6 정책의 감지 메커니즘(TSM 계열 API 인지, Accessibility 속성 조회인지)이 조사 노트에 전혀 언급되지 않아 확정 불가. 앱 설치 후 한글/일본어 입력기로 실측 검증 필요.
6. **`Apply modifiers`류 이벤트와 IME 상호작용** — CJK 입력기 활성 중 `F-05` 의 Hyperkey modifier 합성이 IME 자체의 단축키(예: 한글/영어 전환 키)와 충돌하는지는 조사 범위 밖.
7. **`UCKeyTranslate`/`TIS*` 의 Rust FFI 세부** — 전용 크레이트가 없다는 것은 확정이나(§7), `Carbon.framework` 링크가 Tauri 빌드 파이프라인(코드 서명·번들링)과 충돌 없이 동작하는지는 실측 필요.
8. **`home row`/`symbol row` 팝업의 전체 대안 목록** — 조사 Q7 로 미확정. `A = !` 외 다른 프리셋 변형이 있는지 앱 설치 후 확인 필요.
9. **`Quick press left or right shift` 출력 후보 전체** — 조사 Q8 로 미확정. `( )` 외 `[ ]`/`{ }` 등이 있다면 §3.2.3 표에 행을 추가해야 한다.
10. **입력 소스 재구축 대기 중 이벤트 처리 정책** — §3.2.5의 4번 항목, 재구축 완료까지 대기할지 해당 1회를 무시할지 미결정.
11. **RTL 키캡 일러스트의 실제 그래픽 자산 미러링 여부** — §3.1.4/§5 의 상충하는 직관을 실제 디자인 자산 확보 후 재확인 필요.
