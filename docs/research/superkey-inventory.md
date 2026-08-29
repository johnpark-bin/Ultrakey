# SuperKey 전수 조사 인벤토리

> 조사일: 2026-08-30 · 조사 방식: **웹 기반** (앱 미설치)
> 대상: [superkey.app](https://superkey.app/) 전 페이지 + Sparkle 체인지로그 + 제품 스크린샷 + 제3자 자료
> 조사 시점 최신 버전: **1.66** (2026-06-23)

## 이 문서를 읽는 법

- **사실(확정)** — 출처 URL 과 원문 인용이 함께 붙은 항목. 그대로 명세의 근거로 쓴다.
- **`(추정)`** — 조사 자료에서 직접 확인되지 않은 합리적 추론. 근거를 함께 적었다. 명세에서는 반드시 "미해결 질문" 으로 승계한다.
- **`❓미확인`** — 확인하려 했으나 웹 조사만으로는 확정 불가한 항목.

---

## 0. 조사 방법 메모 — 체인지로그를 어떻게 확보했는가

`https://superkey.app/versions` 는 **정적 HTML 에 릴리스 노트를 담고 있지 않다.** 페이지는 빈 `<div id="versions"></div>` 만 갖고 있고, `/assets/js/parseAppcast.js` 가 런타임에 Sparkle appcast 를 가져와 렌더링한다. 게다가 그 스크립트는 **최근 3개 버전만** 화면에 그린다(`items = items.slice(0, 3)`).

```js
// https://superkey.app/assets/js/parseAppcast.js (원문 발췌)
$.ajax({ type: "GET", url: "/downloads/updates.xml", dataType: "xml", ...
  // cut it to first 3
  items = items.slice(0, 3)
```

따라서 실제 전체 체인지로그는 **Sparkle appcast XML 원본**에 있다:

| 출처 URL | 확인한 사실 |
| :--- | :--- |
| `https://superkey.app/downloads/updates.xml` | HTTP 200, 28,509 bytes, `<item>` 10개. 현행 appcast 는 **롤링 윈도우**라 과거 버전은 빠져 있다. |
| `https://web.archive.org/web/20230815082813id_/https://superkey.app/downloads/updates.xml` | 2023-08-15 스냅샷. v1.18 / 1.19 / 1.20 릴리스 노트 확보. |

`web.archive.org` 의 `updates.xml` 스냅샷은 **2건뿐**(2023-08-15, 2026-06-19)이라, **v1.21 ~ v1.50 구간의 릴리스 노트는 1차 출처로는 복원 불가**다. → §7 미해결 질문.

또한 랜딩 페이지의 제품 스크린샷 3장은 **환경설정 창의 실제 UI 전체**를 담고 있어, 텍스트로 공개되지 않은 설정 항목 대부분을 여기서 확정했다(§3).

---

## 1. 랜딩 페이지 (`https://superkey.app/`)

정적 HTML 로 완결된 **단일 페이지 사이트**다. `/features`, `/docs`, `/pricing`, `/download`, `/faq` 등은 존재하지 않는다(404).

### 1.1 제품 포지셔닝 · 3대 기능

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| https://superkey.app/ | "Superkey — Simple and powerful keyboard enhancement on macOS" | 제품 한 줄 정의: macOS 키보드 강화 유틸리티 |
| https://superkey.app/ | "Seek & click" / "Match what you type, and click it ― all with the keyboard and anywhere on the screen" | **기능 1 — Seek & Click.** 타이핑한 문자열을 화면에서 찾아 키보드만으로 클릭 |
| https://superkey.app/ | "Navigate your screen without a mouse or trackpad" | Seek 의 목적: 마우스·트랙패드 없이 화면 조작 |
| https://superkey.app/ | "Hyperkey included" / "Convert your caps lock key **or** any of your modifier keys to the hyper key, all four modifiers combined: ⌃⌥⌘⇧" | **기능 2 — Hyperkey.** Caps Lock 또는 임의의 modifier 키를 4-modifier 조합(⌃⌥⌘⇧)으로 변환 |
| https://superkey.app/ | "The hyper key acts as an additional modifier key that you can use in all of your other apps that have keyboard shortcuts" | Hyper 키는 타 앱의 단축키에 그대로 쓰이는 추가 modifier 로 동작 |
| https://superkey.app/ | "Or just remap caps lock to a more useful key" | Hyper 로 만들지 않고 일반 키로 리매핑하는 것도 지원 |
| https://superkey.app/ | "☑︎ Power user" / "Maximize keyboard efficiency with just a few checkboxes" | **기능 3 — Power User Presets.** 체크박스 몇 개로 켜는 사전 정의 리매핑 모음 |
| https://superkey.app/ | "Got remappings you want a checkbox for? Let me know!" | 프리셋은 **고정 목록**이며 사용자가 임의 정의하는 구조가 아니다 (사용자 요청 → 개발자가 추가) |
| https://superkey.app/ | "Created by Ryan Hanson" / "ryanhanson.dev" | 제작자. Rectangle · Rectangle Pro · Multitouch · Hyperkey · Charmstone 등의 저자 |

### 1.2 트랙패드 원터치 제스처 — 랜딩 페이지에만 있는 기능

⭐ 이 기능은 3대 기능 소개에 묻혀 있어 놓치기 쉽다. **Hyperkey 의 물리 키를 소모하지 않는 대체 활성화 경로**다.

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| https://superkey.app/ | "No need to sacrifice a key" / "Don't want to sacrifice a key?" | 키를 희생하지 않고 hyper 키를 쓰는 대안 |
| https://superkey.app/ | "Use a simple one touch gesture built just for activating the hyper key" | hyper 키 활성화 전용 원터치 제스처 |
| https://superkey.app/ | "Slide one touch down from a corner or the top edge of the trackpad to activate and remove the touch to release" | 트랙패드 **모서리 또는 상단 가장자리**에서 손가락 하나를 아래로 슬라이드 → 활성화, 손가락을 떼면 해제 |
| https://superkey.app/ | "Designed to enable one touch control of your windows with [Rectangle Pro](https://rectangleapp.com/pro) or app switching with [Charmstone](https://charmstone.app)" | 설계 의도: Rectangle Pro 창 제어 / Charmstone 앱 전환의 원터치 트리거 |

### 1.3 FAQ — 동작·제약의 1차 출처

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| https://superkey.app/ | "Seek uses Optical Character Recognition (OCR) to find your text, and there are currently some limitations to this. Small text, and text close to lines can be difficult to pick up. I'm continuously working on improving this." | Seek 의 1차 검출 수단은 **OCR**. 작은 글자, 선에 인접한 글자는 검출 실패 가능 |
| https://superkey.app/ | "Seek can also parse accessibility information for apps that expose their user interface elements to the macOS Accessibility API. Enable this by checking a box in the preferences window." | Seek 의 2차 검출 수단은 **Accessibility API 파싱**. 환경설정 체크박스로 켠다 (OCR 과 병용) |
| https://superkey.app/ | "Password text fields in macOS are secure and prevent 3rd party applications from knowing which keystrokes are pressed." | **Secure Input** 활성 시(암호 필드) 키 리매핑 전면 무력화. macOS 제약이라 우회 불가 |
| https://superkey.app/ | "Screen recording permissions are only used for Seek, where the app needs to take a screenshot to find the text on your screen." | 화면 녹화 권한 = Seek 스크린샷 전용 |
| https://superkey.app/ | "Accessibility permissions are necessary to know when configured keystrokes are pressed, and to execute clicks." | 접근성 권한 = 키 입력 감지 + 클릭 실행 |
| https://superkey.app/ | "Superkey does not do anything further with any of the information it processes, and only reaches out to the network for license validation or updates as configured. None of the data that Superkey processes is stored on your disk. I take privacy seriously and none of my apps use any kind of telemetry or tracking." | **네트워크는 라이선스 검증 + 업데이트뿐. 디스크 저장 없음. 텔레메트리·트래킹 없음.** 클론의 설계 제약으로 승계할 것 |
| https://superkey.app/ | "When I launch Superkey, I get a warning that says \"Unable to initialize Superkey\", even when the app has the necessary Accessibility settings enabled." | 권한 DB 불일치 시의 고정 실패 문구: **"Unable to initialize Superkey"** |
| https://superkey.app/ | "This indicates that the necessary permissions are out of sync in macOS. Try the following: Close Superkey if it's running / In System Settings -> Privacy & Security -> Accessibility, first disable Superkey, then remove it. / Also disable then remove it from System Preferences -> Privacy & Security -> Input Monitoring if it's there, too. / Restart your mac. / Launch Superkey and enable settings for it as prompted." | 복구 절차 5단계. ⭐ **Input Monitoring** 권한도 관여함이 여기서 드러난다 (권한 3종) |
| https://superkey.app/ | "Keyboard Maestro's shortcut recorder works a little differently than most, BUT if you just record your shortcut physically pressing all the modifiers, then the Hyperkey configured in Superkey (or Hyperkey) will properly trigger what you have configured in Keyboard Maestro." | 합성 modifier 이벤트를 recorder 가 못 잡는 앱이 있다. 물리 입력으로 등록하면 트리거는 정상 |

### 1.4 개발자 권장 설정 ("My Seek Setup") — 기본값 설계의 힌트

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| https://superkey.app/ | "The Caps Lock key is basically a key that is meant to be remapped to something else. Useful real estate." | Caps Lock 을 리매핑 대상으로 보는 제품 철학 |
| https://superkey.app/ | "I remap it to Seek, and check the box to only show while the remapped key is held. This feels like executing a keyboard shortcut to click on text." | 권장: Caps Lock → Seek, "only show while the remapped key is held" 체크 |
| https://superkey.app/ | "I also use semicolon to cycle the results, and in the Presets tab I enable left shift + right shift = Caps Lock." | 권장: 세미콜론으로 결과 순환 + Presets 탭의 "left shift + right shift = Caps Lock" |

### 1.5 시스템 요구사항 · 라이선스 · 배포

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| https://superkey.app/ | "Supports macOS 12+, Intel and Apple Silicon" | macOS 12 (Monterey) 이상, 유니버설 바이너리 |
| https://superkey.app/ | "Free for 20 days, purchase for ..." | **20일 무료 체험** 후 구매 |
| https://superkey.app/ | "One license can be active on 3 devices at a time" | 1 라이선스 = **동시 활성 3대** |
| https://superkey.app/ | "The order process is conducted by an online reseller, Paddle.com, the Merchant of Record for all our orders." | 결제·MoR = **Paddle** |
| https://superkey.app/ (HTML) | `<script src="https://cdn.paddle.com/paddle/paddle.js">` | Paddle **Classic** JS SDK 를 사이트에 임베드 (Paddle Billing 이 아닌 Classic 계열 `paddle.js`) |
| https://superkey.app/ (HTML) | `href="downloads/Superkey1.66.dmg"` | 배포 형식 **`.dmg`**. 조사 시점 최신 1.66 |
| https://superkey.app/downloads/updates.xml | `<sparkle:minimumSystemVersion>12.0</sparkle:minimumSystemVersion>` | appcast 도 최소 시스템 12.0 을 명시 |
| https://superkey.app/downloads/updates.xml | `<enclosure url=".../Superkey1.66.dmg" length="4969453" ...>` | 앱 크기 약 **4.97 MB** |
| https://superkey.app/ | "superkey@ryanhanson.dev" | 지원 연락처 (웹 기반 지원 포털 없음) |

### 1.6 내부 링크 전수

| 링크 | 상태 | 비고 |
| :--- | :--- | :--- |
| `/` | 200 | 랜딩 (단일 페이지) |
| `/versions` | 200 | 체인지로그 — JS 로 appcast 렌더링, 최근 3개만 표시 |
| `/terms` | 200 | Privacy Policy **와** Terms of Service 가 한 페이지에 합쳐져 있음 |
| `/refund` | 200 | 환불 정책 |
| `/hyperkey` | 200 | 구 Hyperkey 앱 구매자 마이그레이션 안내 |
| `/downloads/Superkey1.66.dmg` | 200 | 배포 바이너리 |
| `/downloads/updates.xml` | 200 | ⭐ Sparkle appcast |
| `mailto:superkey@ryanhanson.dev` | — | 지원 메일 |
| `https://ryanhanson.dev` | 200 | 제작자 사이트 (외부) |
| `/features` `/docs` `/pricing` `/download` `/faq` `/support` `/help` `/blog` `/changelog` | **404** | 존재하지 않음 |

---

## 2. 체인지로그 (Sparkle appcast)

### 2.1 현행 appcast — `https://superkey.app/downloads/updates.xml`

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| `.../updates.xml` — v1.66, Tue, 23 Jun 2026 | "Fixes a crash introduced in v1.65." | 1.65 회귀 크래시 수정 |
| `.../updates.xml` — v1.65, Mon, 15 Jun 2026 | "Fixes a bug where the bleh key could also contain the option key." | **bleh 키에 option 이 섞여 들어가는 버그** 수정 → bleh 는 option 을 포함하지 않음이 확정 |
| `.../updates.xml` — v1.62, Wed, 03 Jun 2026 | "The bleh key (⌃⌘⇧) can now be configured in the Hyperkey tab." | ⭐ **bleh 키(⌃⌘⇧) 신설**, Hyperkey 탭에서 설정 |
| `.../updates.xml` — v1.62 | "Menu item icons have been added to the menu bar menu." | 메뉴바 메뉴 항목에 아이콘 추가 |
| `.../updates.xml` — v1.62 | "\"Quick press caps lock\" can now execute a slash keypress (let me know if there are more desired keys for quick press caps lock)." | **Quick press caps lock 의 출력 키 목록에 `/` 추가.** 출력 후보가 열거형(고정 목록)임이 드러남 |
| `.../updates.xml` — v1.62 | "\"Shift + caps lock = caps lock\" will no longer trigger a quick press caps lock keypress." | 두 프리셋의 상호작용 버그 수정 → **프리셋 간 우선순위 규칙이 존재**함을 시사 |
| `.../updates.xml` — v1.60, Thu, 05 Mar 2026 | "Seek should now be matching some accessibility elements that it wasn't before." | Seek 의 접근성 요소 매칭 범위 확대 |
| `.../updates.xml` — v1.60 | "Hyper + delete = forward delete will now work if you have the hyper key set to the globe key." | ⭐ hyper 키의 소스로 **globe(🌐/Fn) 키**를 쓸 수 있음이 확정 |
| `.../updates.xml` — v1.59, Sat, 22 Nov 2025 | "Minor bug fixes." | — |
| `.../updates.xml` — v1.58, Tue, 04 Nov 2025 | "In certain scenarios, Superkey's key remapping was not working upon wake or login. This should now be fixed." | ⭐ **절전 복귀·로그인 시 event tap 재설치**가 필요한 알려진 실패 모드 |
| `.../updates.xml` — v1.56, Wed, 08 Oct 2025 | "Fixed a handful of minor issues." | — |
| `.../updates.xml` — v1.55, Mon, 15 Sep 2025 | "Fixed issues with accessibility parsing for Seek." | — |
| `.../updates.xml` — v1.55 | "Fixed broken Seek behavior on additional displays." | ⭐ **다중 디스플레이는 명시적 실패 이력이 있는 영역** |
| `.../updates.xml` — v1.52, Wed, 16 Jul 2025 | "Now remapping paste to paste w/o formatting will work regardless of keyboard layout." | 키보드 레이아웃 독립성 (paste 프리셋) |
| `.../updates.xml` — v1.51, Sat, 12 Jul 2025 | "Now quick press shift to execute braces will input the correct character, regardless of keyboard layout." | 키보드 레이아웃 독립성 (괄호 프리셋) |
| `.../updates.xml` — v1.51 | "Now in Seek, using semicolon to select the next match will work regardless of keyboard layout." | 키보드 레이아웃 독립성 (Seek 세미콜론) |

> ⭐ **횡단 주제 — 키보드 레이아웃 독립성.** 1.51/1.52 세 항목이 모두 같은 원인이다. 문자 기반이 아니라 **물리 키코드(virtual keycode) 기반**으로 판정해야 QWERTY 이외 레이아웃에서 깨지지 않는다.

### 2.2 아카이브 appcast (2023-08-15 스냅샷)

출처: `https://web.archive.org/web/20230815082813id_/https://superkey.app/downloads/updates.xml`

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| (위 스냅샷) — v1.20, Fri, 14 Jul 2023 | "Fixed a bug where caps lock + keys being mapped to arrow keys wasn't working when caps lock was remapped as the hyper key" | Caps Lock 이 hyper 인 상태에서 "Caps lock + WASD/HJKL = 화살표" 프리셋이 깨지던 버그 → ⭐ **Hyperkey 와 Presets 는 같은 키를 두고 충돌한다** |
| (위 스냅샷) — v1.20 | "Adjusted how Superkey handles error scenarios with macOS accessibility settings getting out of sync" | 권한 DB 불일치 처리 개선 (FAQ 의 "Unable to initialize" 와 같은 계열) |
| (위 스냅샷) — v1.19, Sat, 06 May 2023 | "Seek will now pick up more varieties of font colors/backgrounds" | OCR 전처리에 **글자색/배경 대비 처리**가 들어 있음 |
| (위 스냅샷) — v1.19 | "Fix for a potential crash with Seek using the Accessibility API" | AX 트리 순회 중 크래시 이력 |
| (위 스냅샷) — v1.18, Wed, 03 May 2023 | "Added a checkbox to the Seek preferences for the initial cut at letting Seek use the macOS accessibility API to find more text matches. There will be more improvements to the accessibility parsing and OCR parsing in upcoming releases." | ⭐ **Accessibility 파싱은 OCR 보다 나중에 추가된 보조 경로**. 즉 OCR 이 주 경로, AX 가 증강 |
| (위 스냅샷) — v1.18 | "Fixed some other small issues with Seek" | — |

### 2.3 Sparkle 업데이트 인프라에서 읽어낸 사실

| 출처 | 확인한 사실 | 클론에 대한 함의 |
| :--- | :--- | :--- |
| `updates.xml` 의 `xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle"` | **Sparkle 2.x** appcast 포맷 | 자동 업데이트를 Sparkle 호환으로 만들면 인프라를 그대로 재현 가능 |
| `sparkle:edSignature="BGoKibR3nvMA..."` | **EdDSA(ed25519) 서명** 사용 (구형 DSA 아님) | 업데이트 무결성 검증 방식 |
| `<sparkle:deltas>` 에 `Superkey66-65.delta` … `Superkey66-60.delta` | **델타 업데이트** 제공, 직전 5개 버전까지 | 전체 4.97MB vs 델타 183KB |
| `sparkle:deltaFromSparkleLocales="de,he,ar,el,ja,fa,uk"` | ⭐ 앱이 **de·he·ar·el·ja·fa·uk (+en)** 8개 로케일을 번들 | 현지화가 실제 기능. he/ar/fa 는 **RTL** |
| `<sparkle:version>66</sparkle:version>` vs `<sparkle:shortVersionString>1.66</...>` | `CFBundleVersion` = 단조 증가 정수, `CFBundleShortVersionString` = `1.NN` | 버전 체계 |
| `sparkle:minimumSystemVersion` = `12.0` | 최소 macOS 12.0 | — |

---

## 3. ⭐ 제품 스크린샷에서 확정한 환경설정 UI 전체

랜딩 페이지가 제공하는 3장의 스크린샷은 환경설정 창을 원본 해상도로 담고 있다. **아래 항목은 모두 스크린샷에서 직접 판독한 것이며, 라벨 문자열은 화면에 보이는 그대로다.**

환경설정 창은 좌측 사이드바 + 우측 패널 구조이며, 탭은 **`Seek` · `Hyperkey` · `Presets` · `General`** 4개다.

### 3.1 `Seek` 탭 — 출처: `https://superkey.app/assets/images/seekScreenshot.png`

| 라벨 (원문) | 컨트롤 | 스크린샷의 값 | 비고 |
| :--- | :--- | :--- | :--- |
| `Toggle Seek with shortcut:` | 단축키 레코더 (지우기 ✕ 버튼 포함) | `⌥Space` | 전역 단축키로 Seek 토글 |
| `Remap key to Seek:` | 팝업 버튼 | `caps lock` | 단일 키를 Seek 트리거로 리매핑 |
| `Only show while the remapped key is held` | 체크박스 | ☑ | 부제: **"Release the remapped key to click"** — 키를 떼는 순간 클릭 |
| `Seek using macOS accessibility` ⓘ | 체크박스 | ☑ | AX 파싱 병용 (v1.18 에서 추가된 그 체크박스) |
| ↳ `Match on more than one character` | 중첩 체크박스 | ☑ | AX 매칭을 2글자 이상으로 제한 |
| `Only Seek in the frontmost window` | 체크박스 | ☐ | 검색 범위를 최전면 창으로 한정 |
| `Focus window before clicking` | 체크박스 | ☑ | 클릭 전 대상 창을 먼저 활성화 |
| `Semicolon highlights next match` | 체크박스 | ☑ | `;` 로 다음 매치 순환 |
| `Change click modes with modifier keys` ⓘ | 체크박스 | ☑ | 부제: **"If this setting is disabled, modifiers will be applied to the click"** — modifier 를 *클릭 종류 전환*에 쓸지, *클릭에 얹을지*의 이중 의미 스위치 |

### 3.2 `Hyperkey` 탭 — 출처: `https://superkey.app/assets/images/hyperScreenshot.png`

| 라벨 (원문) | 컨트롤 | 스크린샷의 값 | 비고 |
| :--- | :--- | :--- | :--- |
| `Remap key to hyper key:` | 체크박스 + 팝업 버튼 | ☑ / `right command` | 소스 키 선택 |
| `Include shift in hyper key` | 체크박스 | ☑ | 해제 시 hyper 는 ⌃⌥⌘ (shift 제외) |
| `Remap key to meh key (^⌥⇧):` | 체크박스 + 팝업 버튼 | ☑ / `right option` | ⭐ **meh 키 = ⌃⌥⇧** (command 없음) |
| *(v1.62 신설, 스크린샷에는 없음)* `bleh key (⌃⌘⇧)` | 체크박스 + 팝업 버튼 `(추정)` | — | 출처: appcast v1.62 "The bleh key (⌃⌘⇧) can now be configured in the Hyperkey tab." meh 와 동형 UI 로 추정 |
| `Apply modifiers to keypress events and:` | 체크박스 4개 | ☑ `Click` ☑ `Drag` ☑ `Move` ☐ `Scroll` | ⭐ 합성 modifier 를 **키 이벤트 외에 마우스 이벤트에도** 얹을지 개별 선택 |
| `Engage hyper key using trackpad:` | 체크박스 + 팝업 버튼 | ☐ / `top right` | 트랙패드 제스처. 부제: **"Slide only one touch in from the selected area the trackpad a little. Remove the touch to release."** |

패널 상단에 `Hyperkey ⓘ` 제목과 정보 버튼이 있다. 좌측에 현재 선택된 소스 키를 그린 **키캡 일러스트**(`⌘ command`, `⌥ option`)와 `^⌥⌘⇧` modifier 미리보기가 붙는다.

### 3.3 `Presets` 탭 — 출처: `https://superkey.app/assets/images/presetScreenshot.png`

패널은 좌측의 **키캡 일러스트(`caps lock` / `shift` / `delete`)** 로 4개 그룹이 구분된다.

**그룹 1 — caps lock**

| 라벨 (원문) | 컨트롤 | 스크린샷의 값 |
| :--- | :--- | :--- |
| `Remap caps lock to:` | 체크박스 + 팝업 | ☑ / `left control` |
| `Quick press caps lock to execute:` | 체크박스 + 팝업 | ☑ / `caps lock` |
| `Quick press duration` | 슬라이더 (눈금 8칸) + 값 라벨 | `1000 ms` |
| `Caps lock + space = enter` | 체크박스 | ☑ |
| `Caps lock + W A S D = ▲◀▼▶` | 체크박스 | ☑ |
| `Caps lock + [H J K L] = ◀▼▲▶` | 체크박스 + 인라인 팝업(`H J K L`) | ☐ |
| `Caps lock + home row = [symbol row (A = !)]` | 체크박스 + 인라인 팝업 | ☐ |

**그룹 2 — shift**

| 라벨 (원문) | 컨트롤 | 스크린샷의 값 |
| :--- | :--- | :--- |
| `Double tap shift = caps lock` | 체크박스 | ☐ |
| `Left shift + right shift = caps lock` | 체크박스 | ☑ |
| `Shift + caps lock = caps lock` | 체크박스 | ☐ |
| `Quick press left or right shift to input corresponding:` | 체크박스 + 팝업 | ☑ / `( )` |

**그룹 3 — delete**

| 라벨 (원문) | 컨트롤 | 스크린샷의 값 |
| :--- | :--- | :--- |
| `Hyper + delete = forward delete` | 체크박스 | ☑ |
| `Remap delete to forward delete` | 체크박스 | ☐ |
| `Shift + delete = forward delete` | 체크박스 | ☑ |

**그룹 4 — 기타**

| 라벨 (원문) | 컨트롤 | 스크린샷의 값 |
| :--- | :--- | :--- |
| `Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):` | 체크박스 + 팝업 | ☑ / `Right ⌘` |
| `Home & end operate on lines` | 체크박스 | ☑ |

> 스크린샷의 체크 상태는 **홍보용 구성**이지 출고 기본값이라는 보장은 없다 `(추정)`. → §7 미해결 질문.

### 3.4 `General` 탭

사이드바에 존재하는 것은 스크린샷 3장 모두에서 확인된다. **패널 내용을 담은 스크린샷은 공개되어 있지 않다.** ❓미확인

동종 앱(Rectangle 등 같은 제작자) 관례와 다른 기능의 요구사항으로부터 추론하면 다음이 들어갈 것으로 본다 `(추정)`:
로그인 시 실행, 메뉴바 아이콘 표시/숨김, 자동 업데이트 확인, 라이선스 등록·상태, 버전 정보, 환경설정 초기화. → §7 미해결 질문.

---

## 4. 법적 문서 · 가격 · 지원

### 4.1 Privacy Policy + Terms of Service (`https://superkey.app/terms`)

한 페이지에 두 문서가 합쳐져 있다. `Last updated: 1 March 2021`.

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| https://superkey.app/terms | "**Privacy Policy** — No user data is gathered by Superkey." | ⭐ 개인정보 정책 본문이 **한 문장**이다. 수집 자체가 없다 |
| https://superkey.app/terms | "Superkey uses APIs provided by Apple and Ryan Hanson makes no guarantees that Superkey will work properly on every single device or every version of macOS." | Apple API 의존 → 기기·OS 버전별 동작 보장 없음 |
| https://superkey.app/terms | "Superkey and the materials on this web site are provided \"as is\"." | 무보증 |
| https://superkey.app/terms | "Neither Ryan Hanson nor any person associated with Ryan Hanson makes any warranty or representation with respect to the completeness, security, reliability, quality, accuracy, or availability of the services." | 무보증 (확장) |
| https://superkey.app/terms | "In no event shall Ryan Hanson be liable for any damages (including, without limitation, damages for loss of data or profit, or due to business interruption) arising out of the use or inability to use Superkey" | 책임 제한 |
| https://superkey.app/terms | "if there is liability found on the part of Ryan Hanson, it will be limited to the amount paid for the products and/or services" | 책임 상한 = 지불액 |

### 4.2 Refund Policy (`https://superkey.app/refund`)

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| https://superkey.app/refund | "If you're unhappy with Superkey, a refund can be issued within 14 days of purchase. Send an email to superkey@ryanhanson.dev." | **환불 14일**, 메일 신청. (체험 20일 > 환불 14일 — 비대칭에 유의) |

### 4.3 Hyperkey 마이그레이션 (`https://superkey.app/hyperkey`)

Superkey 는 같은 제작자의 별도 앱 **Hyperkey** 를 흡수한 제품이며, 구매자 이관 경로가 따로 있다.

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| https://superkey.app/hyperkey | "Migrating from Hyperkey — How to get your free license for Superkey" | 구 Hyperkey 구매자 → Superkey 무료 라이선스 |
| https://superkey.app/hyperkey | "In the latest version of Hyperkey, you should have been prompted to obtain a coupon code. Your email address here is the coupon code." | 쿠폰 코드 = 이메일 주소 |
| https://superkey.app/hyperkey | "Go through the purchase process for Superkey. When you get to this screen, add your coupon code." | Paddle 체크아웃에서 100% 쿠폰 적용 |
| https://superkey.app/hyperkey | "You get a new license instead of reusing your old one, which takes care of any used up activations you might have for old devices." | ⭐ **라이선스에는 소진되는 activation 카운트가 있다.** 새 키 발급으로 초기화 |
| https://superkey.app/hyperkey | "The upgrade path us not as prone to abuse since I'm not generating new license keys on the fly." | 라이선스 키는 사전 발급 풀 방식 `(추정)` |
| https://superkey.app/ (배너) | "Purchased Hyperkey? [Follow these steps](https://superkey.app/hyperkey)" | 랜딩 상단 배너 (HTML 주석 처리된 상태로 소스에 존재) |

### 4.4 가격 — ❓ 1차 출처 확인 불가

랜딩 페이지의 가격 문구는 `"Free for 20 days, purchase for ..."` 에서 끊기고, **실제 금액은 Paddle JS SDK 가 런타임에 주입**한다(정적 HTML 에 금액 문자열이 없다). curl 로 받은 HTML 에는 `$` 로 시작하는 금액이 존재하지 않음을 확인했다.

| 출처 | 값 | 신뢰도 |
| :--- | :--- | :--- |
| 제3자 집계 사이트 (웹 검색) | `$15.99` 1회 구매 | `[3차]` — 1차 출처 미확인. 명세에서 금액에 의존하지 말 것 |

가격은 클론 명세에 영향을 주지 않으므로 미확인으로 남긴다.

---

## 4.5 ⭐ 개발자 본인의 기술 포스트 — `https://ryanhanson.dev/posts/superkey`

제목: **"Match what you type and click it: Superkey"**. Seek 의 UI 형태와 구현 기반을 제작자가 직접 서술한 **1차 출처**이며, 스크린샷만으로는 알 수 없던 항목을 다수 확정해 준다.

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| https://ryanhanson.dev/posts/superkey | "The main feature is called Seek. It's a way to click text that you type in a **Spotlight-like search bar**." | ⭐ Seek 의 UI 는 **Spotlight 유사 검색 바**다. vimium 식 힌트 문자 라벨 방식이 아니다 |
| https://ryanhanson.dev/posts/superkey | "You can tie it to a keyboard shortcut, much like Spotlight or any launcher app, or you can remap a key to show it." | 활성화 경로 2종: 전역 단축키 / 키 리매핑 (환경설정의 두 항목과 대응) |
| https://ryanhanson.dev/posts/superkey | "After typing in the Seek bar, you can change the selected match with the **up and down arrows or tab and shift+tab**, and **hit enter to click**." | ⭐ 매치 순환 = ↑/↓ 또는 Tab/⇧Tab. 확정 = **Enter**. (세미콜론 순환은 이후 버전에서 추가된 별도 경로) |
| https://ryanhanson.dev/posts/superkey | "The Seek functionality is powered by **Apple's Vision framework** that uses OCR to match the text on your screen." | ⭐ OCR 엔진 = **Apple Vision**. 제작자 직접 확인. 서드파티 OCR 아님 |
| https://ryanhanson.dev/posts/superkey | "Examples of text that doesn't match include **extra small text, certain shades of blue text on black background, and text that is too close to other lines**." | OCR 실패 사례 3종 (랜딩 FAQ 보다 구체적). v1.19 의 "font colors/backgrounds" 수정과 대응 |
| https://ryanhanson.dev/posts/superkey | "There are still a couple more shortcomings that will get ironed out in upcoming releases, like **drawing the line to the selected match on a different display**" | ⭐⭐ Seek 오버레이는 **검색 바에서 선택된 매치까지 선(line)을 그린다.** 초기 버전은 다른 디스플레이의 매치로는 선을 못 그렸다 (v1.55 의 다중 디스플레이 수정과 대응) |
| https://ryanhanson.dev/posts/superkey | "My preferred way to use Seek is to remap caps lock to it, and check the box for \"Only show while the remapped key is held\". … but requires you to be able to type words with a pinky down on the caps lock key." | hold 모드의 실사용 제약: Caps Lock 을 누른 채로 타이핑해야 함 |
| https://ryanhanson.dev/posts/superkey | "**My Hyperkey app is entirely included in Superkey.** … the concept is just to reassign a key to be cmd+control+shift+option, a key combination that is not typically used in any default shortcuts." | Superkey ⊃ Hyperkey. hyper 조합의 설계 근거: 기본 단축키와 충돌하지 않는 조합 |
| https://ryanhanson.dev/posts/superkey | "Superkey will follow the same **simple checkbox-centered configuration**" | 환경설정 설계 원칙: 체크박스 중심 |
| https://ryanhanson.dev/posts/superkey | "There's definitely a tradeoff between offering complex entirely custom key remappings versus the **canned presets** here, but I'll always prefer just checking a box over messing around with complicated preferences construction" | ⭐ **의도적 설계 결정**: 임의 커스텀 리매핑을 제공하지 않고 사전 정의 프리셋만 제공한다 (Karabiner-Elements 와의 차별점) |
| https://ryanhanson.dev/ | "**Rectangle** — The new standard in window management for macOS. **Free and open source.**" | 같은 제작자의 Rectangle 은 오픈소스다. AX·이벤트 탭·권한 온보딩의 실제 구현 패턴을 참고할 수 있다 |

> **Q9 해소.** Seek 오버레이의 형태는 이 포스트로 확정됐다: **Spotlight 유사 검색 바 + 화면상 매치 하이라이트 + 검색 바에서 선택 매치까지 잇는 선.**

---

## 5. 제3자 자료 (교차 검증용)

> ⚠️ 아래는 1차 출처가 아니다. **벤더 문서와 충돌하면 벤더 문서를 채택**한다.

| 출처 URL | 원문 인용 | 요약 |
| :--- | :--- | :--- |
| https://notes.nicolasdeville.com/apps/superkey/ | "Uses OCR technology to identify and click text visible on screen without mouse interaction. The tool can also leverage accessibility API data from compatible applications." | `[3차]` 벤더 설명과 일치 |
| https://marvinolson.com/2024/10/16/superkey-mac-application/ | "Remaps the Caps Lock key to a Shift+Control+Option+Command combination, creating 'a gazillion set of hot key combinations'" | `[3차]` 실사용자 리뷰. Hyperkey 조합 설명 일치 |
| https://marvinolson.com/2024/10/16/superkey-mac-application/ | Alfred Workflow 연동 예: `Caps Lock+N` → NotePlan 실행 | `[3차]` hyper 키가 런처/자동화 앱의 트리거로 쓰이는 실사용 패턴 |
| https://macupdater.net/app_updates/appinfo/com.knollsoft.Superkey/index.html | 번들 ID `com.knollsoft.Superkey` | ⭐ **번들 ID 확정**. 제작자의 다른 앱들과 같은 `com.knollsoft.*` 네임스페이스 |
| https://talk.macpowerusers.com/t/hyperkey-is-now-free-and-if-you-bought-it-you-can-get-a-free-superkey-license/30937 | "if you purchased it in the past, updating to the newest version will give you a 100% coupon off Superkey." | `[3차]` §4.3 마이그레이션 경로와 일치 |

### 모순되는 주장

| 항목 | 벤더(1차) | 제3자 | 판정 |
| :--- | :--- | :--- | :--- |
| 최소 macOS | `macOS 12+` (랜딩 + appcast `minimumSystemVersion=12.0`) | insmac.org: "macOS 10.12 or later" | **벤더 채택.** 제3자는 v1.19(2023) 시절 정보를 방치한 것으로 보임 `(추정)` |
| Seek 커버리지 | "Small text, and text close to lines can be difficult to pick up" | "Seek anything you can display with just the keyboard" | **벤더 채택.** 제3자는 마케팅 어조. OCR 은 원리상 전수 검출이 불가능 |

---

## 6. 조사에서 도출된 횡단 관찰 (구현 설계에 직결)

1. **Seek 은 OCR 이 주(主), Accessibility 가 부(副)다.** v1.18 릴리스 노트가 AX 파싱을 "initial cut" 으로 나중에 추가했다고 명시한다. 두 소스는 **병합**되며(중복 매치 처리 필요), AX 는 체크박스로 끌 수 있다.
2. **Hyperkey 와 Presets 는 동일한 물리 키를 두고 경쟁한다.** v1.20 버그(Caps Lock 이 hyper 일 때 `Caps lock + WASD` 프리셋 파손), v1.62 버그(`Shift + caps lock = caps lock` 이 quick press 를 오발) 두 건 모두 이 충돌이다. → **단일 리매핑 엔진 + 명시적 우선순위 규칙**이 필요하다. 기능을 따로 구현하면 같은 버그를 재생산한다.
3. **모든 판정은 물리 키코드 기준이어야 한다.** v1.51/1.52 세 건의 수정이 전부 "regardless of keyboard layout" 이다.
4. **권한은 3종이다** — Accessibility, Screen Recording, **Input Monitoring**. 세 번째는 FAQ 의 복구 절차에서만 드러난다.
5. **권한 DB 불일치는 상시 발생하는 실패 모드**이며, 전용 오류 상태와 안내 UI(`"Unable to initialize Superkey"`)를 갖는다.
6. **절전 복귀·로그인 시 event tap 이 죽는다** (v1.58). 재설치 로직이 필수다.
7. **다중 디스플레이는 별도로 깨진다** (v1.55). Seek 의 좌표계 처리를 처음부터 다중 디스플레이 전제로 설계해야 한다.
8. **네트워크 접근은 라이선스 검증 + 업데이트 두 가지로 한정**되며, 이것이 제품의 명시적 약속이다. 클론도 이 경계를 지켜야 한다.
9. **8개 로케일을 번들**하며 그중 3개(he/ar/fa)가 RTL 이다. i18n 을 후행 과제로 미루면 레이아웃을 다시 만들어야 한다.

---

## 7. 미해결 질문 (웹 조사로 확정 불가)

| # | 질문 | 왜 확정 못 했는가 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| Q1 | v1.21 ~ v1.50 의 릴리스 노트 | 현행 appcast 는 최근 10개만 유지. Wayback 의 `updates.xml` 스냅샷은 2건뿐 | 개발자 문의, 또는 각 버전 `.dmg` 내 릴리스 노트 확인 |
| Q2 | `General` 탭의 실제 항목 | 해당 탭 스크린샷이 공개되지 않음 | 앱 설치 후 확인 (이번 범위 밖) |
| Q3 | 각 설정의 **출고 기본값** | 스크린샷은 홍보용 구성값 | 앱 최초 실행 후 `defaults read com.knollsoft.Superkey` |
| Q4 | `Remap key to hyper key` / `Remap key to Seek` 등 팝업의 **선택지 전체 목록** | 스크린샷은 선택된 값 1개만 노출. `caps lock`, `left control`, `right command`, `right option`, `globe`(v1.60) 는 확인됨 | 앱 설치 후 팝업 열기 |
| Q5 | `Quick press caps lock to execute` 의 출력 후보 전체 | `caps lock`, `/`(v1.62) 만 확인 | 동일 |
| Q6 | `Quick press duration` 슬라이더의 최소·최대·간격 | 스크린샷에서 값 `1000 ms` 와 눈금 8칸만 판독 | 동일 |
| Q7 | `Caps lock + home row` 팝업의 대안 (`symbol row (A = !)` 외) | 동일 | 동일 |
| Q8 | `Quick press left or right shift` 의 출력 후보 (`( )` 외 `[ ]`, `{ }` 등) | 동일 | 동일 |
| ~~Q9~~ | ~~Seek 오버레이의 시각적 형태~~ | ✅ **해소** — §4.5 개발자 포스트에서 확정: Spotlight 유사 검색 바 + 매치 하이라이트 + 선택 매치까지 잇는 선 | — |
| Q9' | 검색 바의 **화면상 위치** (화면 중앙 고정인지, 커서/활성 창 기준인지) 와 다중 디스플레이에서 어느 화면에 뜨는지 | 포스트는 선을 그린다는 사실만 언급 | 앱 실행 화면 확인 |
| Q10 | `Change click modes with modifier keys` 의 modifier ↔ 클릭 종류 매핑 | ⓘ 툴팁 내용 미공개 | 앱 내 ⓘ 확인 |
| Q11 | 라이선스 키 포맷, 활성화·비활성화 API, 오프라인 유예 정책 | 공개 문서 없음 | Paddle Classic License API 문서 + 앱 동작 관찰 |
| Q12 | 체험 20일의 기산점(최초 실행/설치)과 만료 후 동작(전면 차단/기능 제한) | 공개 문서 없음 | 동일 |
| Q13 | 업데이트 확인 주기 | 공개 문서 없음. Sparkle 기본은 24시간 `(추정)` | 동일 |
| Q14 | 판매가 | Paddle JS 런타임 주입이라 정적 HTML 에 없음 | 실제 체크아웃 화면 |
| Q15 | 메뉴바 메뉴의 항목 구성 (v1.62 에서 아이콘 추가된 그 메뉴) | 스크린샷 미공개. `/assets/images/superkey menu icon.png` 로 아이콘만 존재 확인 | 앱 설치 후 확인 |
