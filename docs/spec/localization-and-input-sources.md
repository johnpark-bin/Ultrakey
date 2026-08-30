# F-14 · 현지화와 키보드 입력 소스 독립성

> 한 줄 요약: ⭐ **정정**: 이전 판이 "앱이 8개 로케일(영어 포함, 그중 3개 RTL)을 번들한다"고 판정한 것은 간접 증거를 직접 증거로 오독한 것이었다(§1). 실측 결과 SuperKey v1.66 의 UI 는 **영어 단일**이며 현지화 기능이 없다. 따라서 (A) 는 이제 "원본을 따라가는 명세"가 아니라 **클론이 채택할지 말지를 스스로 결정해야 하는 선택지**이며, 채택할 경우를 대비한 문자열 카탈로그·런타임 로케일 결정·RTL 레이아웃 규칙의 설계 원칙만 정의한다. (B) 모든 키 입력 판정을 물리 키코드 기준으로 통일해 QWERTZ·AZERTY·Dvorak·CJK 입력기에서도 깨지지 않도록 하는 원칙은 원본의 실제 회귀 이력(v1.51/v1.52)과 이번 실측(Carbon 심볼 확정)으로 뒷받침되며 그대로 유효하다. 둘 다 나중에 손대면 UI·판정 로직을 전면 재작성해야 하는 횡단 관심사라 명세 단계에서 미리 설계해 둔다.
> 의존성: `F-07`(`key-remapping-engine.md`) — event tap 설치·중재·quick press 판정 메커니즘 자체. 본 문서는 그 엔진이 **무엇을 기준으로 판정해야 하는가**의 원칙만 제공한다. `F-09`(환경설정 창의 컨트롤 구조) — 본 문서가 정의하는 문자열·RTL 규칙이 실제로 배치되는 대상.
> 관련 명세: `F-08`(`power-user-presets.md`, 개별 프리셋의 동작 정의 — `home row`/`symbol row` 등 레이아웃 의존 프리셋의 **개별 동작**은 F-08 소관, 본 문서는 그 프리셋이 비-QWERTY 에서 "무엇을 의미하는가"의 판정 원칙만 제공. `Caps lock + W A S D`/`[H J K L]` 의 Colemak/Dvorak 변형 자동 적용 여부는 §3.2.7, F-08 §3.2·§9 항목 3 과 상호 참조) · `F-05`(`hyperkey.md`, modifier 합성 — 본 문서 §3.2 의 키코드 기반 판정 원칙을 그대로 따르는 선행 사례)

---

## 1. 개요

이 문서는 서로 다른 두 문제를 다루지만, 둘 다 같은 성격의 리스크를 공유한다 — **초기 설계에서 놓치면 나중에 UI·판정 로직을 전면 재작성해야 한다**는 점이다.

**(A) 현지화 — ⭐⭐ 이전 판의 가장 큰 오기 정정.** 이전 판은 Sparkle 델타 appcast 의 `sparkle:deltaFromSparkleLocales="de,he,ar,el,ja,fa,uk"` 속성(`superkey-inventory.md` §2.3)에서 "앱이 de·he·ar·el·ja·fa·uk + en, 총 8개 로케일을 번들하며 그중 3개가 RTL"이라고 판정했다. **이것은 오독이었다.** 실측(app-bundle-analysis.md §5.1)은 다음을 확정한다:

- 앱 본체 `Contents/Resources/` 에는 `Base.lproj` 하나뿐이다.
- Info.plist 에 `CFBundleLocalizations` 키가 **없다**.
- appcast 속성이 나열한 `de` `he` `ar` `el` `ja` `fa` `uk` 는 SuperKey 자신의 로케일이 아니라, 번들에 동봉된 **`Sparkle.framework` 가 자체적으로 갖는 45개 로케일 중 이번 델타 업데이트가 건드린 파일 목록**이다 — 목록 7개가 전부 Sparkle 의 45개 안에 들어 있다는 사실이 이를 뒷받침한다. `Paddle.framework` 도 12개, `KeyboardShortcuts` 리소스 번들도 15개를 별도로 갖지만 이 역시 SuperKey 자신의 UI 로케일과 무관하다.

⭐ **결론: SuperKey v1.66 의 UI 는 영어 단일이고, 현지화 기능이 존재하지 않는다. RTL 대응도 없다.**

이 정정이 "클론도 현지화를 포기해야 한다"는 뜻은 아니다. **원본에 없는 기능을 클론이 가질지는 별개의 제품 결정이다.** 그래서 이 문서의 (A) 는 이제 "원본이 이미 하는 것을 따라가는 명세"가 아니라 **"클론 고유의 선택지"**로 성격이 바뀐다 — 채택 여부는 열어 두되, 채택하기로 결정했을 때 나중에 손대면 비싼 부분(문자열 카탈로그 구조, RTL 레이아웃 미러링, 단일 소스 결정)만 미리 설계해 둔다는 논리는 원본이 실제로 그렇게 했든 아니든 그대로 유효하다 — 로케일·RTL 대응은 UI 전반에 걸친 횡단 관심사이므로, 문자열을 하드코딩하고 레이아웃을 LTR 전제로 짜 둔 뒤 나중에 채택을 결정하면 오버레이·환경설정 창의 레이아웃 로직을 전면 재설계해야 하기 때문이다. **다만 구체적인 로케일 목록(몇 개, 어떤 언어)은 근거를 잃었으므로 확정 사실로 쓰지 않는다** — 어떤 로케일을 실제로 지원할지는 이 문서가 정할 문제가 아니라 클론이 별도로 정할 제품 결정 사항으로 되돌린다(§3.1.1, §8, §9).

**(B) 키보드 입력 소스 독립성.** 조사(`superkey-inventory.md` §2.1, §6 관찰 3)는 원본이 **v1.51/v1.52 에서 세 건**을 "regardless of keyboard layout" 으로 수정한 이력을 확정했다 — 괄호 quick press, Seek 세미콜론, paste w/o formatting 리매핑. 세 건 모두 원인이 같다: **문자 기반 판정이 QWERTY 이외 레이아웃에서 깨진다.** 이는 F-07 이 구현할 이벤트 판정 엔진 전체에 적용되어야 하는 원칙이므로, 개별 프리셋(F-08)이나 엔진 자체(F-07)에 흩어 두지 않고 여기서 한 번에 못박는다. 이번 실측(app-bundle-analysis.md §3.1)으로 `TISCopyCurrentKeyboardInputSource`·`TISCopyCurrentASCIICapableKeyboardLayoutInputSource`·`TISGetInputSourceProperty`·`UCKeyTranslate`·`LMGetKbdType` 심볼과 Carbon 링크가 확정되어, 이전 판이 "(추정)"으로 남겼던 CJK 입력기 대응(§3.2.6)의 근거가 크게 보강됐다.

두 주제를 하나의 문서로 묶은 이유는 조사 브리프가 명시한 대로 **"나중에 하면 전부 다시 만들어야 하는" 성격**을 공유하기 때문이다 — (A)는 채택 시 UI 레이아웃 전면 재작업, (B)는 판정 로직 전면 재작업이라는 형태로 나타난다.

**범위 밖**: 개별 프리셋의 동작 정의(`F-08`), event tap·중재·quick press 판정 메커니즘 자체(`F-07` — 본 문서는 그 엔진이 따라야 할 판정 원칙만 제공), 환경설정 창의 컨트롤 구조(`F-09`).

---

## 2. 사용자 시나리오

⚠️ 시나리오 1~3 은 **클론이 현지화를 채택하기로 결정했다는 가정 위의 시나리오**다(§1). SuperKey 원본에는 대응하는 실제 동작이 없으므로 "원본이 이렇게 동작한다"는 근거가 아니라 "채택한다면 이렇게 설계한다"는 예시로 읽어야 한다. 시나리오 4~7 은 (B) 입력 소스 독립성 원칙에 속하며, 원본의 실제 회귀 이력과 이번 실측으로 뒷받침된다.

1. **독일어 macOS 사용자** — 시스템 언어가 독일어인 사용자가 Superkey 를 처음 실행하면, 환경설정 창·메뉴바 메뉴·시스템 알림이 모두 독일어로 표시된다. 키 이름(`Caps Lock`, `Befehlstaste` 등)은 macOS 시스템 관례를 따르지만 modifier 기호(`⌃⌥⌘⇧`)는 번역되지 않고 그대로 표기된다.
2. **히브리어 사용자가 Seek 를 사용** — RTL 로케일에서 Seek 오버레이의 Spotlight 유사 검색 바가 우측 정렬로 표시되고, 검색 바에서 선택된 매치까지 잇는 연결선의 시작점도 그에 맞게 조정된다.
3. **아랍어 사용자가 환경설정 창을 연다** — `Presets` 탭의 좌측 키캡 일러스트(`caps lock`/`shift`/`delete`)와 우측 체크박스 목록의 좌우 배치가 미러링되어, 사이드바가 우측에, 컨트롤이 좌측에 온다.
4. **AZERTY 프랑스어 키보드 사용자** — `Quick press left or right shift to input corresponding: ( )` 을 사용할 때, 물리적으로 `Shift` 키 위치를 판정하는 로직은 keycode 기준이라 레이아웃과 무관하게 동작하고, 출력되는 문자 `(`/`)` 는 현재 AZERTY 입력 소스에서 실제로 그 문자를 낼 수 있는 조합을 찾아 합성한다.
5. **Dvorak 사용자** — `Caps lock + W A S D = ▲◀▼▶` 프리셋을 사용할 때, 판정은 물리적으로 QWERTY 배열의 W/A/S/D 위치에 해당하는 keycode(`kVK_ANSI_W` 등)를 기준으로 하므로, Dvorak 배열에서 그 물리 위치에 있는 실제 문자(`,`/`a`/`o`/`e` 등)와 무관하게 항상 같은 물리 키 4개가 방향키로 동작한다. ⭐ **주의**: 이 시나리오가 전제하는 "항상 같은 물리 키 4개 고정" 은 §3.2.7 의 실측(실행 파일에 `wasdArrowDvorak` 등 레이아웃별 별도 키 집합이 존재)으로 재검토가 필요하다 — SuperKey 는 이 프리셋에 한해 레이아웃마다 **다른** keycode 집합을 쓰는 것으로 보인다(§3.2.7 참조).
6. **한글 입력기로 문서를 작성하던 중** — 한글 조합이 진행 중(예: "ㄱ" 입력 후 조합 대기)인 상태에서 quick press caps lock 이 트리거되면, 조합 중인 텍스트가 깨지지 않도록 리매핑 개입이 유예된다(§3.2, 정책은 `(추정)`).
7. **세션 도중 입력 소스를 전환** — 사용자가 `⌘+Space`(또는 메뉴바)로 미국 영어 입력 소스에서 프랑스어(AZERTY) 입력 소스로 전환하면, 앱은 `kTISNotifySelectedKeyboardInputSourceChanged` 를 받아 캐시된 keycode↔문자 매핑을 즉시 무효화하고, 이후 발생하는 quick press 출력은 새 입력 소스 기준으로 재계산된다.

---

## 3. 동작 명세

### 3.1 (A) 로케일

#### 3.1.1 로케일 목록 — ✅ 확정: `en` · `ko` · `zh` · `es` · `ja` 5종 (제품 결정 D4, 2026-08-30 확장)

이전 판은 여기에 "8개 로케일 표"(en/de/he/ar/el/ja/fa/uk, RTL 3종)를 확정 사실로 적었다. §1 에서 정정했듯 이 목록은 SuperKey 자신의 로케일이 아니라 Sparkle 프레임워크의 로케일을 오독한 것이었으므로, **원본으로부터 도출되는 로케일 목록은 존재하지 않는다.**

⭐ **제품 결정 D4 로 클론의 목록이 확정되었다 (2026-08-30, 이슈 #5): 한국어 + 영어 2종.** 이는 원본 추종이 아니라 **클론 고유의 선택**이며, `README.md` 의 제품 결정 표가 정본이다.

⭐ **확장 (2026-08-30, 이슈 #39): 5종으로 넓힌다.** 사용자 요구다. D4 의 성격(원본 추종이 아닌 클론 고유의 제품 결정)은 그대로이고 목록만 늘어난다.

| 로케일 코드 | 방향 | 역할 | 카탈로그 |
| :--- | :--- | :--- | :--- |
| `en` | LTR | 기본(fallback). 지원하지 않는 시스템 언어는 전부 여기로 폴백한다 | `resources/i18n/en.json` |
| `ko` | LTR | 클론 고유 추가 (D4 1차) | `resources/i18n/ko.json` |
| `zh` | LTR | 간체 중국어. `zh-Hans`·`zh-CN`·`zh-SG` 등 지역 변형은 전부 여기로 접힌다 | `resources/i18n/zh.json` |
| `es` | LTR | 스페인어(중립) | `resources/i18n/es.json` |
| `ja` | LTR | 일본어 | `resources/i18n/ja.json` |

⚠️ **다섯 개 전부 LTR 이다** — §3.1.4 의 RTL 미러링 규칙에는 여전히 **적용 대상이 없다.** RTL 은 M5 승계 그대로다.

⚠️ **번체 중국어(`zh-Hant`·`zh-TW`·`zh-HK`)는 지원 목록에 없다.** 간체 카탈로그로 대신 채우지 않는다 — 두 표기 체계는 어휘까지 다르고, 간체를 보여주는 것이 영어를 보여주는 것보다 낫다는 근거가 없다. §3.1.2 의 폴백 규칙대로 `en` 으로 간다. 필요해지면 `zh-Hant` 를 **별도 카탈로그로** 더한다.

⚠️ **`settings.tab.*` 라벨의 번역 범위가 로케일마다 다르다.** `ko` 는 `Presets`·`General`·`Korean` 을 영어로 두고 `Keyboards` 만 번역했으나, `zh`·`es`·`ja` 는 브랜드가 아닌 일반 명사를 전부 번역했다. 후자가 §3.1.3 의 분류표(브랜드·기능 고유명사만 비번역)에 맞고, `ko` 쪽이 그 표와 어긋나 있다 — **정정 대상은 `ko` 이지만 이번 회차에서 고치지 않았다**(같은 파일을 병렬 작업 #40 이 건드리고 있어 충돌을 만들지 않기로 했다). §9 미해결 질문에 남긴다. ⭐ **해소 (2026-08-31, 이슈 #45)** — 메뉴 탭 다국어화 가이드라인(`Seek`·`Hyperkey` 원문 유지, 나머지 전부 i18n)에 따라 `ko` 탭 라벨을 번역(`프리셋`·`한국어`·`일반`)하고 전 로케일에서 **탭 라벨 ↔ 상세 헤딩 값을 일치**시켰다. 함께 정한 것: `Korean` 탭의 라벨·헤딩은 "언어와 관련된 입력 설정은 언어 이름만 표기한다"는 같은 이슈의 규칙을 따라 `한국어`(`한국어 입력` 이 아님) — 이 규칙의 유일한 적용 대상이 그 헤딩이었다. 재발 방지: `crates/ultrakey-i18n` 테스트 `seek과_hyperkey_탭_라벨과_헤딩은_모든_로케일에서_원문_그대로다`·`탭_라벨과_상세_헤딩_값은_모든_로케일에서_같다`.

**M1 이 실제로 구현한 범위**(권한 안내 모달 등 사용자 대면 문자열이 M1 에서 이미 생기므로 카탈로그 구조를 M1 에서 잡았다):
- `resources/i18n/{en,ko}.json` — §3.1.5 의 "단일 카탈로그" 결정을 따르는 평평한 키-값 파일
- `crates/ultrakey-i18n` — 로케일 결정(§3.1.2), 키 조회, 누락 시 `en` 개별 폴백(§5 항목 11), ⭐ OS 버전별 어휘 교체(System Preferences ↔ System Settings, F-11 §3.2)
- 두 파일의 **키 집합 동일성을 단위 테스트로 강제**한다 — §8 (A) 의 "두 경로 사이에 번역 키 누락 차이가 없다"를 자동 검증 장치로 만든 것이다

⭐ **RTL 은 M1 범위 밖이다.** `en`·`ko` 둘 다 LTR 이라 §3.1.4 의 미러링 규칙에 **현재 적용 대상이 없다.** 규칙 자체는 나중에 RTL 로케일을 추가할 때를 위해 그대로 남겨 두고, `(A)` 의 나머지(RTL 레이아웃, 키 이름 번역 정책 확정)는 M5 로 승계한다.

아래는 로케일 집합이 늘어나도 유지되는 설계 원칙이다:

- 로케일은 `en`(기본) + 클론이 선택한 로케일 코드 N개로 구성된다.
- 각 로케일은 LTR/RTL 둘 중 하나로 분류되며(macOS `NSLocale.characterDirection(forLanguage:)` 로 판정), RTL 로 분류된 로케일에는 §3.1.4 의 미러링 규칙이 적용된다.
- 로케일 코드 집합 자체는 Info.plist 의 `CFBundleLocalizations` 키에 선언한다(§3.1.2).

번들에 동봉된 서드파티 프레임워크(Sparkle 45개, Paddle 12개, KeyboardShortcuts 15개)의 로케일 개수는 SuperKey 자신의 UI 로케일 지원 범위를 **전혀 보장하지 않는다** — 이번 오독이 정확히 그 착각에서 비롯됐다. 클론이 이 서드파티 프레임워크들과 동등한 것(예: 자동 업데이트 UI, 라이선싱 UI, 단축키 레코더)을 채택하더라도, 그 구성요소가 갖는 로케일 개수를 클론 자신의 지원 로케일 목록과 혼동하지 않아야 한다.

#### 3.1.2 로케일 결정 순서

1. macOS 는 앱 번들의 `CFBundleLocalizations`(Info.plist 에 클론이 지원하기로 결정한 로케일 코드를 선언)와 시스템 설정(`시스템 설정 > 일반 > 언어 및 지역`, 앱별 언어 재정의 포함)의 사용자 선호 언어 목록을 대조해 최적 로케일을 결정한다.
2. 네이티브 측은 이 결정 결과를 `NSBundle`/`NSLocale` 계열 API 로 읽어(§6) 문자열 카탈로그의 로케일 키를 선택한다.
3. 정확히 일치하는 지역 변형이 없으면(예: 시스템이 `de-CH`) 언어 코드만으로 폴백한다(`de`).
4. 클론이 지원하기로 선언한 로케일 어디에도 해당하지 않으면 `en` 으로 폴백한다.
5. 앱 내에 시스템 설정을 오버라이드하는 언어 선택 UI가 있는지는 원본 기준으로 **실측 확정됐다 — 없다**(app-bundle-analysis.md §6.4, `General` 탭에 언어 선택 항목이 없음). ⭐ **클론은 둔다 (2026-08-30, 이슈 #39 — 사용자 요구).** 원본에 없는 것을 더하는 갈라짐이므로 [`README.md` 원본과 갈라지는 지점](README.md#-원본과-갈라지는-지점-divergence) **D6** 로 등재했다. 동작은 §3.1.2-a.
6. `(추정)` 로케일 값은 앱 시작 시 1회 결정되며, 실행 중 시스템 언어를 바꿔도 재시작 전까지 UI 에 반영되지 않을 가능성이 있다. 이는 macOS 네이티브 앱의 일반적 관례이나 원본에 현지화 기능 자체가 없어 직접 근거는 없다 — §9.

#### 3.1.2-a ⭐ `General` 탭 언어 선택 UI (2026-08-30 신설, 이슈 #39)

**저장 키**: `general.language`. **부재 = 시스템 언어를 따른다**(§3.1.2 의 1~4 순서). F-15 의 "부재 = 기본값" 규약을 그대로 따른다 — "시스템 따름"이 기본값이므로 그것을 고른 사용자는 파일에 키를 남기지 않는다.

**선택지**: `System` · `English` · `한국어` · `中文` · `Español` · `日本語`.

⭐ **각 언어의 이름은 그 언어로 쓴다(endonym).** `Korean`·`Chinese` 처럼 현재 UI 언어로 쓰면, 잘못된 언어로 앱이 떠서 아무것도 못 읽는 사용자가 **자기 언어를 찾지 못한다.** 언어 선택 팝업은 "지금 UI 를 읽을 수 없는 사람"이 쓰는 유일한 컨트롤이므로, 그 목록만은 현재 로케일과 무관해야 한다. (`System` 항목만은 번역한다 — 그것을 고르는 사람은 이미 읽을 수 있는 상태다.)

**적용 시점**: ⭐ **즉시. 재시작을 요구하지 않는다.** 근거: 재시작을 요구하면 사용자가 언어를 잘못 골랐을 때 되돌리는 비용이 "읽을 수 없는 UI 에서 재시작하고 다시 찾기"가 된다. 즉시 적용이면 잘못 고른 것을 그 자리에서 되돌린다.

즉시 적용이 건드려야 하는 표면은 셋이다.

| 표면 | 어떻게 |
| :--- | :--- |
| 환경설정 창(WebView) | `settings_set` 이 새 카탈로그 전량을 응답에 실어 보내고, 프런트엔드가 이미 갖고 있는 `t(key)` 렌더 경로를 다시 돌린다 |
| 메뉴바 메뉴(네이티브) | 카탈로그 교체 후 `rebuild_tray_menu()` — 권한 전이 때 이미 하는 일과 같은 경로다 |
| 권한 온보딩 모달 | 다음에 열릴 때 새 카탈로그를 읽는다. 열려 있는 동안 바꾸는 경로는 만들지 않는다(그 상태에서 언어를 바꿀 방법이 없다 — 설정 창에 도달하지 못한 상태이기 때문) |

**카탈로그 보관 방식**: `AppState.catalog` 를 `Catalog` 에서 `ArcSwap<Catalog>` 로 바꾼다. 읽는 쪽(커맨드·트레이 갱신)이 여럿이고 쓰기는 사람이 언어를 바꿀 때뿐이라, `architecture.md` §2.2 가 설정 테이블에 쓴 것과 같은 이유로 `ArcSwap` 이 맞다. ⛔ **탭 콜백은 카탈로그를 읽지 않는다** — 문자열은 UI 표면에만 있다(§3.1.6).

#### 3.1.6 ⭐⭐ UI 문자열 경로와 로그 문자열 경로의 분리 (2026-08-30 신설, 이슈 #39)

사용자 원문:

> 추가로 로깅을 할 때에는 모두 영어로 표기를 했으면 좋겠음.
> 이것은 언어의 선택과 무관하게 추후 문제 상황을 분석함에 있어서 편리함을 제공하기 때문임.

**요구의 핵심은 문구 치환이 아니라 경로 분리다.** 지금까지 이 저장소에는 "이 문자열은 사용자에게 가는가, 로그로 가는가"라는 구분이 **아예 없었다.** 그래서 로그 문구가 한국어로 쓰였고(문서를 한국어로 쓰는 규약이 코드 안까지 흘러들어왔다), 반대 방향의 사고도 이미 한 번 일어났다 — `main.rs` 가 온보딩 모달의 **UI 문구를 그대로 로그에 찍고 있었다**. 그 줄은 사용자가 언어를 바꾸면 **로그의 내용이 따라 바뀐다.** 정확히 사용자가 없애 달라고 한 상황이다.

문구만 영어로 고치면 6개월 뒤 같은 일이 다시 생긴다. 그래서 규약과 **그 규약을 깨면 빌드가 깨지는 장치**를 함께 둔다.

##### 규약

| | UI 문자열 | 로그 문자열 |
| :--- | :--- | :--- |
| **어디에 있는가** | `resources/i18n/<locale>.json` — 코드 안에 리터럴로 두지 않는다 | **코드 안의 영어 리터럴.** 카탈로그에 들어가지 않는다 |
| **누가 읽는가** | `ultrakey-i18n::Catalog` | `tracing` 매크로 |
| **언어** | 사용자가 고른 로케일 5종 | ⭐ **항상 영어. 예외 없다** |
| **누가 보는가** | 사용자 | 개발자·이슈에 로그를 붙이는 사용자 |
| **번역하는가** | 한다 | ⛔ **하지 않는다.** 번역되면 다른 언어 사용자의 로그를 읽을 수 없고, 문구 검색이 로케일마다 갈린다 |

⭐ **두 경로는 서로를 부르지 않는다.** 카탈로그 값이 로그로 흘러가서는 안 되고(로그가 로케일에 따라 바뀐다), 로그 리터럴이 UI 로 가서도 안 된다(번역되지 않은 문자열이 사용자에게 보인다).

⭐ **동적 값은 예외가 아니다.** 사용자에게 보여줄 오류를 로그에도 남겨야 할 때는 **문구가 아니라 키를 남긴다** — `tracing::warn!(copy_key = "permissions.accessibility.title", ...)`. 키는 로케일과 무관하고, 오히려 문구보다 검색하기 좋다.

##### 규약을 깨면 빌드가 깨지게 하는 장치

규약을 문서에만 두면 지켜지지 않는다. 이 저장소는 이미 **소스를 읽어 규약을 강제하는 테스트**를 쓰고 있다(`apps/ultrakey-app/tests/frontend_wiring.rs` 가 `settings.html`·`tauri.conf.json` 의 내용을 직접 검사한다). 같은 형태로 둘을 더한다.

1. **`logs_are_english_only`** — `crates/`·`apps/` 의 모든 `.rs` 를 훑어 `tracing::{trace,debug,info,warn,error}!` 호출 안에 **한글이 있으면 실패**한다. 위치와 문구를 함께 보고해 고칠 자리를 바로 알려 준다.
2. **`log_macros_do_not_read_the_ui_catalog`** — 같은 매크로 호출 안에 `catalog.get(`·`catalog.format(`·`.entries()` 가 있으면 실패한다. 이것이 위에서 실제로 일어났던 드리프트를 막는 장치다.

⛔ **기각한 대안 ① — `Catalog::get` 이 `Display` 없는 newtype 을 돌려주게 한다**: `tracing::warn!(%title)` 이 컴파일되지 않아 구조적으로 완벽해 보이지만, `.as_str()` 한 번이면 조용히 우회되고 그 우회는 리뷰에서 눈에 띄지 않는다. 그러면서 카탈로그를 쓰는 모든 호출부의 타입이 바뀐다 — 비용은 크고 보장은 새는 쪽이다.
⛔ **기각한 대안 ② — 로그 문구도 카탈로그에 넣고 항상 `en` 으로 조회한다**: 카탈로그가 두 배로 커지고, "이 키는 UI 용인가 로그 용인가"라는 구분이 다시 사라진다. 무엇보다 **로그 문구는 번역 대상이 아니므로 애초에 카탈로그에 있을 이유가 없다.**
⛔ **기각한 대안 ③ — 규약만 문서에 적고 리뷰로 지킨다**: 이 저장소가 이미 그렇게 하다가 실패한 방식이다(한국어 로그 120건).

##### 이번 회차의 범위

- `tracing` 매크로 안의 한국어 문구 **전량**을 영어로 옮긴다. ⚠️ 기계적 치환이 아니라 **의미가 통하는 영어**로 쓴다.
- ⛔ **주석·문서·커밋 메시지는 그대로 한국어다.** 이 규약은 **실행 시 밖으로 나가는 문자열**에만 적용된다. 저장소의 문서 규약(AGENTS.md §3)은 바뀌지 않는다.
- `expect()`·`panic!`·테스트 assert 메시지의 한국어는 **이번 범위가 아니다** — 그것은 개발자가 개발 중에만 보는 것이고 사용자의 로그 파일에 나타나지 않는다. 다만 `Display for LoginItemError` 처럼 **오류 값이 로그로 흘러가는 것**은 로그 경로이므로 영어로 옮긴다.

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

⚠️ 아래 규칙은 클론이 RTL 로 분류되는 로케일을 채택하기로 결정한 경우에 적용되는 **설계 원칙**이다(§3.1.1). 어떤 로케일이 실제로 RTL 목록에 들어갈지는 클론의 제품 결정이며, 여기서는 임의의 RTL 로케일이 활성화됐을 때를 가정한다:

- **환경설정 창**: 좌측 사이드바(탭 4개) ↔ 우측 패널 배치가 좌우 반전된다. 각 패널 내 "체크박스 + 팝업" 행의 좌우 순서도 반전되어, 라벨 텍스트가 우측 정렬로 시작한다.
- **`Presets` 탭 키캡 일러스트**: `caps lock`/`shift`/`delete` 그룹 구분용 일러스트의 화면상 위치가 미러링된다. 단, modifier 기호 표기 순서(`⌃⌥⌘⇧`)는 §3.1.3 에 따라 언어·방향과 무관하게 macOS 표준 순서를 유지한다 — 아이콘의 **배치**는 미러링하되 기호 **조합 표기 순서**는 고정한다.
- **Seek 오버레이 검색 바**: Spotlight 유사 검색 바 내부 텍스트 입력·커서 시작 위치가 RTL 로 전환된다. 검색 바에서 선택된 매치까지 잇는 연결선(`superkey-inventory.md` §4.5 확정 사실)의 기준점은 여전히 검색 바의 화면상 실제 좌표이므로, 검색 바 자체가 화면의 어느 쪽에 배치되든 선은 그 실제 위치에서 그려진다 — 미러링은 검색 바 **내부 요소**의 정렬에 적용되고, 화면 좌표계 자체는 절대 좌표라 미러링 대상이 아니다.
- 이 규칙들의 정확한 세부(정렬 값, 여백 반전 여부 등)는 실제 UI 구현체(`F-09`, `F-03`)가 결정하며, 본 문서는 "무엇이 미러링 대상인가"라는 분류 원칙만 제공한다.

#### 3.1.5 문자열 소스 통합 — Tauri(WebView) ↔ 네이티브

원본 조사에는 없는 이 클론 고유의 아키텍처 문제다. Superkey 클론은 환경설정 창·Seek 오버레이를 Tauri(WKWebView)로 그리고, 메뉴바 메뉴·시스템 알림은 네이티브(AppKit) API 로 그린다. 두 렌더링 경로가 각자 별도의 문자열 카탈로그(예: 네이티브는 `.strings`, 웹 쪽은 JS i18n JSON)를 가지면, 로케일 하나에 대해 어느 한쪽에만 문자열이 추가되는 **분기(drift)**가 발생하기 쉽다 — 특히 §3.1의 번역 범위 자체가 확정되지 않은 상태(§9)에서는 이 위험이 더 크다.

**결정: 단일 로케일별 문자열 카탈로그 파일(클론이 지원하기로 결정한 로케일 코드 수만큼 × 파일 1개, 키-값 형식)을 앱 리소스로 하나만 두고, 네이티브 Rust 코드와 WebView 양쪽이 같은 파일을 읽는다.**

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

> 📌 **구현 갱신 (2026-08-30, 이슈 #32) — 이 채택 원칙이 한동안 지켜지지 않았다.**
> M2-2(PR #17)가 넣은 `Effect::TypeChar` 구현은 **(ii)만** 했다 — `CGEventKeyboardSetUnicodeString` 에 keycode 0 짜리 이벤트. (i)에 필요한 역방향 테이블(`ultrakey-layout::LayoutTable::keycode_for_char`)은 **만들어져 있고 `(`·`)` 에 대한 레이아웃 독립 테스트까지 있었는데, 부르는 곳이 한 군데도 없었다.** 이 저장소가 이미 세 번 겪은 결함 계열 — *입력 쪽에 있는 대칭이 출력 쪽에 없다*(PR #23) — 의 네 번째다.
> F-08.11(`Quick press left or right shift to input corresponding:`)이 실기기에서 동작하지 않는다는 사용자 보고가 이것을 드러냈다. 실기기 로그로 확정한 것: 중재 엔진은 `Effect::TypeChar` 를 정확히 냈고(`ULTRAKEY_TRACE_TAP=1` 계측), 합성된 `CGEvent` 도 유니코드 `(`/`)` 를 싣고 세션 이벤트 스트림에 실제로 흘렀다(`examples/tap_listen`). 즉 결함은 **무엇을 내보내는가**가 아니라 **어떤 모양으로 내보내는가**였다.
> 이제 `ultrakey-engine::text_output::plan_text_output` 이 (i)를 먼저 시도하고 실패할 때만 (ii)로 떨어진다 — §8 수용 기준의 마지막 항목이 요구하는 그대로다.

#### 3.2.3 입력 판정·출력 생성 규칙 표

| 상황 | 키코드로 판정 | 문자로 출력? | 사용 API | 근거 |
| :--- | :--- | :--- | :--- | :--- |
| Seek: `;` 로 다음 매치 선택 | `kVK_ANSI_Semicolon`(0x29) 물리 keycode | 아니오 — 내부 커맨드 트리거, 문자 산출 없음 | `CGEventGetIntegerValueField(kCGKeyboardEventKeycode)` | `superkey-inventory.md` §2.1 v1.51 "regardless of keyboard layout" |
| quick press shift → 괄호 실행(`( )`) | `kVK_Shift`(좌 0x38 / 우 0x3C) + quick press 판정(F-07 소관, 지속시간은 `Quick press duration` 값) | 예 — `(` 또는 `)` | §3.2.2 (i) 채택, 실패 시 (ii) 폴백. `UCKeyTranslate` + `TISCopyCurrentKeyboardInputSource` | 조사 v1.51 "input the correct character, regardless of keyboard layout" |
| quick press caps lock → `/` 실행(v1.62 추가) | `kVK_CapsLock`(0x39) + quick press 판정 | 예 — `/` | §3.2.2 (i) 채택, 실패 시 (ii) 폴백 | appcast v1.62 "can now execute a slash keypress" |
| remap paste → paste w/o formatting(`⌘⌥⇧V`) | `⌘`(Command) + `kVK_ANSI_V`(0x09) 조합의 keyDown | 아니오 — 단축키 합성. 동일 keycode `V` 에 modifier 비트만 추가 | `CGEventSetFlags` 로 Option+Shift 비트를 추가해 동일 keycode 로 재주입(`F-05` 패턴과 동일) | 조사 v1.52 "regardless of keyboard layout". 단축키는 keycode+flags 로 인식되므로 (ii) Unicode 직접 주입은 부적합(§3.2.2) |
| `Caps lock + W A S D = ▲◀▼▶` | ⭐ **정정(§3.2.7)**: `kVK_ANSI_W`/`A`/`S`/`D` 고정이 아니라, 감지된 키보드 레이아웃(QWERTY/Colemak/Dvorak)에 따라 **레이아웃별로 다른 keycode 집합**을 쓰는 것으로 보인다 `(미확정 — 팝업 부재로부터의 해석, §3.2.7)` | 아니오 — 방향키(`kVK_UpArrow` 등) 합성 | `CGEventGetIntegerValueField` 판정 + `CGEventCreateKeyboardEvent` 로 방향키 keycode 합성 | F-08 소관(개별 동작)이나 판정 원칙은 본 문서 소관. §3.2.7 참조 — "물리 키코드 고정" 원칙만으로는 이 프리셋의 실제 동작을 설명하지 못한다 |
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

#### 3.2.6 CJK 입력기 활성 중의 동작 — ⭐ 실측으로 API 확정, 정책은 정정

이전 판은 이 상황을 통째로 `(추정)`으로 남겼다. 한글/일본어/중국어 입력기가 조합 중(marked text 존재)일 때 리매핑이 개입하면 조합이 깨진다는 것은 일반적으로 알려진 문제이며, "marked text 감지 → 문자 출력형 리매핑 pass-through" 라는 정책을 가설로 제시했었다. 이번 실측(app-bundle-analysis.md §3.1, `nm -u`)으로 실행 파일이 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 를 링크하는 것이 확정되어, 정책의 근거가 바뀐다.

⭐ **`TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 는 macOS 의 표준 기법이다** — 현재 활성 입력 소스가 한글/日本語/中文 같은 비-ASCII 입력 방식(IME)일 때, keycode→문자 해석을 그 IME 로 하지 않고 **사용자가 최근에 선택했던 "ASCII 가능(ASCII-capable)" 키보드 레이아웃으로 폴백**해 수행하는 API 다. 물리 키 기반 단축키 판정을 CJK IME 활성 중에도 깨지지 않게 하려는 목적으로 널리 쓰이는 패턴이다.

**정정된 이해**: SuperKey 는 marked text(조합 중 텍스트) 상태를 직접 감지해 개입을 유예하는 방식이 아니라, **애초에 문자 출력이 필요한 리매핑(quick press 괄호/슬래시 실행, `symbol row` 등)의 keycode→문자 해석 단계에서 이 API 로 얻은 ASCII 가능 레이아웃을 조회 기준으로 쓰는 것으로 보인다** `(미확정 — 심볼 링크는 확정, 정확한 호출 지점과 실제 동작은 미관찰)`. 이 방식이 맞다면, §3.2.1 의 물리 키코드 판정 원칙과 문자 출력 계산이 IME 조합 상태와 무관하게 항상 같은 절차로 동작하므로, marked text 감지·조합 상태 판별이라는 별도 메커니즘 자체가 필요 없어진다.

- **실측 확정**: 심볼 링크(`TISCopyCurrentASCIICapableKeyboardLayoutInputSource`), 그 API 의 표준 용도(ASCII 폴백 keycode 해석).
- **`(미확정)`으로 남는 것**: 이 API 를 정확히 어느 리매핑 경로에 쓰는지(quick press 전용인지, 모든 문자 출력형 리매핑에 공통인지), 실제로 한글 입력기 조합 중 이 폴백이 부작용 없이 동작하는지(실측 시 Screen Recording 권한·조합 방해 부작용을 감수해야 해 이번 조사에서는 검증하지 않았다), 그리고 조합 버퍼 자체를 건드리지 않는 리매핑(Hyperkey modifier 합성, Seek 활성화, 방향키 프리셋)이 여전히 조합 여부와 무관하게 동작하는지.
- 이 정책은 §9 미해결 질문으로 승계하며, 실제 구현 전 한글/일본어 입력기로 실측 검증이 필요하다.

#### 3.2.7 ⭐ 신규 — 레이아웃별 프리셋 변형이 실재한다

실행 파일 문자열(실측: 번들 문자열)에서 다음 키가 확인된다:

```
hjklArrow / hjklArrowColemak / hjklArrowDvorak
ijklArrow / ijklArrowColemak / ijklArrowDvorak
wasdArrow / wasdArrowColemak / wasdArrowDvorak
```

그런데 실측(AX 트리, app-bundle-analysis.md §6.3)에서 `Caps lock + [ ]` 팝업의 선택지는 **`H J K L` 과 `I J K L` 두 개뿐**이고 Colemak/Dvorak 변형은 팝업에 없다. `Caps lock + W A S D` 항목에는 애초에 팝업이 없다(체크박스뿐).

→ **레이아웃 변형은 사용자가 팝업에서 고르는 것이 아니라, 감지된 키보드 레이아웃에 따라 자동 적용되는 것으로 보인다** `(미확정 — 팝업 부재로부터의 해석)`. `power-user-presets.md`(F-08) §3.2 F-08.5·F-08.6, §9 항목 3 과 상호 참조.

⭐ **이 발견이 중요한 이유**: §3.2.1 의 "물리 키코드로 판정한다" 원칙은 대부분의 리매핑(quick press 문자 출력, semicolon, paste 등)에는 충분하지만, **이 프리셋 계열(방향키 매핑)에는 그것만으로 부족하다는 증거다.** Colemak/Dvorak 에서는 QWERTY 기준 물리 키코드(`kVK_ANSI_W`/`A`/`S`/`D` 등)가 여전히 존재하지만, 그 물리 위치에 손가락을 놓았을 때의 **인체공학적 의미(어느 손가락이 어디에 있는가)** 가 달라진다. "WASD 로 위/왼쪽/아래/오른쪽을 이동한다"는 프리셋의 의도가 QWERTY 사용자에게는 자연스러운 손가락 배치를 전제하므로, Dvorak/Colemak 에서 그 의도를 그대로 유지하려면 **레이아웃마다 다른 keycode 집합**(그 레이아웃에서 인체공학적으로 동등한 위치의 키)을 써야 한다는 것이 `wasdArrowColemak`/`wasdArrowDvorak` 등 별도 키가 존재하는 이유로 보인다.

**§3.2.1 원칙의 보강**: "입력 판정은 물리 키코드로 한다" 는 원칙은 유지하되, 이 프리셋 계열에서는 **"어떤 물리 키코드 집합을 쓸지 자체가 감지된 레이아웃에 따라 달라진다"** 는 예외를 명시해야 한다. §2 시나리오 5, §3.2.3 WASD 행, §5 엣지 케이스 4 는 이전 판이 "항상 같은 물리 키 4개 고정" 이라고 서술했던 부분이며, 이 실측으로 재검토가 필요하다는 점을 표기해 두었다.

#### 3.2.8 ⭐ 신규 — 폴백 교체 이전 원본 입력 소스의 언어 노출 (F-16 요구)

`korean-input.md`(F-16) §3.3·§9 항목 4 가 이 문서에 낸 요구를 여기서 반영한다.

**무엇이 문제였는가.** `current_layout()`(`crates/ultrakey-platform/src/text_input_source.rs`)은 현재 입력 소스가 ASCII 불가일 때(CJK IME 가 정확히 그렇다) `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 로 소스를 **교체한 뒤** `source_id` 를 읽는다(§3.2.6). 그 결과 `LayoutSnapshot.source_id` 는 한국어 입력기가 활성이어도 **폴백 레이아웃(예: US)의 ID** 를 담는다. F-16 이 이 필드로 "한국어 입력기가 활성인가"를 판정하면 **항상 거짓**이 나온다 — 이 필드는 애초에 그런 판정을 위해 설계되지 않았다.

**결정**: `LayoutSnapshot` 에 두 필드를 더해, **폴백 교체 이전** 원본 소스에서 읽은 값을 그대로 노출한다.

- `original_source_id: String` — 교체 이전 원본 소스의 `kTISPropertyInputSourceID`.
- `original_languages: Vec<String>` — 같은 원본 소스의 `kTISPropertyInputSourceLanguages`(BCP-47 언어 태그, `CFArray<CFString>` 를 원소 순서 그대로 옮긴 것). 조회 실패면 빈 `Vec`.

⚠️ **`is_ascii_capable == false` 만으로는 불충분하다.** 일본어·중국어 IME 도 ASCII 불가이므로, 이 불리언 하나로는 "한국어인가"를 구분할 수 없다 — 반드시 언어 태그 자체를 봐야 한다.

**판정 기준**은 `kTISPropertyInputSourceLanguages` 의 **첫 원소**다 — Karabiner-Elements 의 `input_source_if { language: "ko" }` 조건과 같은 기준이며, 이 문서(F-14)는 값을 노출만 하고 "한국어인가"의 실제 **판정 자체는 소유하지 않는다** — 그 판정은 `ultrakey-core::korean`(`classify_input_source_languages`) 소관이다. 이 크레이트 분리는 `ultrakey-layout`(F-14 소비자)이 `used_ascii_fallback` 을 그대로 나르기만 하고 판정을 다시 하지 않는 기존 규약과 같다.

⛔ **읽는 순서가 F-16 판정의 전제다.** `original_source_id`/`original_languages` 는 반드시 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 로 소스를 교체하는 코드 **이전**에 읽어야 한다 — 그 아래에서 읽으면 이미 폴백된 소스를 보게 되어 이번 절이 고치려는 결함이 그대로 재발한다.

⛔ **기존 `source_id` 의 의미는 바꾸지 않는다.** 교체 **이후** 소스의 ID 라는 의미 그대로 `ultrakey-layout` 이 레이아웃 역산 테이블의 캐시 키로 계속 쓴다.

⭐ **실측(2026-08-30, 이 저장소 세션)** — 판정 기준이 실제로 성립하는지 직접 확인했다. 도구: [`../dev/tools/tis-language-probe.swift`](../dev/tools/tis-language-probe.swift). 이 도구는 `TISCreateInputSourceList` 로 **설치된** 소스를 열거해 프로퍼티를 읽기만 하므로 **시스템 설정을 전혀 바꾸지 않는다**(입력 소스를 전환할 필요조차 없다).

| 입력 소스 | `isASCIICapable` | `languages[0]` | 전체 |
| :--- | :---: | :--- | :--- |
| `com.apple.inputmethod.Korean` 및 그 하위 5종(2-set·3-set·390 세벌식·공진청 로마자·HNC 로마자) | `false` | **`ko`** | `["ko"]` |
| `com.apple.inputmethod.Kotoeri.*` (일본어 8종) | `false` | `ja` | `["ja"]` |
| `com.apple.inputmethod.SCIM.ITABC` (중국어 간체) | `false` | `zh-Hans` | `["zh-Hans"]` |
| `com.apple.keylayout.Anjal` (타밀) | `false` | `ta` | `["ta"]` |
| `com.apple.keylayout.ABC` | `true` | `en` | **96개** |
| `com.apple.keylayout.ABC-AZERTY` | `true` | `fr` | 95개 |

이 실측이 확정하는 것 셋:

1. Apple 기본 한국어 입력기 **6종 전부**가 `languages[0] == "ko"` 다 — 이 절의 판정 기준이 성립한다.
2. ⚠️ **`is_ascii_capable == false` 만으로는 불충분하다는 것이 사실로 확인됐다** — 일본어·중국어·타밀 입력기도 전부 `false` 다.
3. ⭐ **"첫 원소만 본다" 가 옳다.** `ABC` 레이아웃 하나가 언어 태그를 **96개** 싣고 있다 — "목록에 `ko` 가 들어 있는가" 로 판정하는 설계였다면 임계 경로 비용과 오판 위험 양쪽에서 나빴을 형태다.

⚠️ **이 실측이 확정하지 못하는 것**: 위는 `TISCreateInputSourceList`(열거) 경로이고, 우리 코드가 쓰는 것은 `TISCopyCurrentKeyboardInputSource`(현재 활성) 경로다. 같은 프로퍼티지만 **경로가 다르므로**, 실행 중인 앱이 실제로 이 값을 담아 오는지는 실기기 확인이 따로 필요하다(`../dev/manual-verification.md`).

상호 참조: `korean-input.md` §3.3(문제 발견), §9 항목 4(소유권 승계).

---

## 4. 설정 항목

| # | 항목 | 존재 여부 | 비고 |
| :--- | :--- | :--- | :--- |
| 1 | 앱 내 언어 선택(시스템 설정을 오버라이드) | ⭐ **없음 — 실측 확정으로 승격** | 이전 판은 `(추정 — 없을 가능성이 높음)` 이었다. 실측(app-bundle-analysis.md §6.4)으로 확정됨: `General` 탭에 언어 선택 항목이 **없다**. SuperKey 원본은 애초에 현지화 기능이 없으므로(§1) 이 UI 도 없는 것이 당연하다. 클론이 §3.1.1 대로 현지화를 채택하기로 하면, 이런 UI 를 새로 둘지는 별도 결정 사항 |
| 2 | 입력 소스 변경에 대한 사용자 노출 설정(알림·로그 등) | **없음** `(추정)` | §3.2.5 절차는 백그라운드에서 투명하게 동작하는 것으로 설계한다. 조사 자료에 이런 설정 항목의 근거가 없다 |
| 3 | RTL 강제/해제 토글(시스템 로케일 방향과 무관하게) | **없음** `(추정)` | RTL 여부는 §3.1.2 로케일 결정 결과에 종속되며, 별도 스위치가 있다는 근거가 없다 |
| 4 | `Caps lock + home row = ` [팝업] 의 대안 목록 | ⭐ **2종 확정 — 해소** | 실측(AX 트리, app-bundle-analysis.md §6.3): `symbol row (A = !)` · `function row (A = F1)`. 이전 판이 미확정으로 남겼던 것이 해소됨. 개별 프리셋 동작은 `power-user-presets.md`(F-08) §3.2 F-08.7 참조 |
| 5 | `Quick press left or right shift` 출력 후보 전체 | ⭐ **4종 확정 — 해소** | 실측(AX 트리, app-bundle-analysis.md §6.3): `( )` · `[ ]` · `{ }` · `< >`. 이전 판이 미확정으로 남겼던 것이 해소됨. 개별 프리셋 동작은 `power-user-presets.md`(F-08) §3.2 F-08.11 참조 |

---

## 5. 엣지 케이스와 실패 모드

⚠️ 1~3, 11, 13 은 **클론이 현지화(및 그중에서도 RTL 로케일)를 채택할 경우 대비해야 하는 과제**다 — SuperKey 원본에는 해당 기능 자체가 없으므로 실제로 관찰된 결함이 아니라, 채택 시 미리 대비해 두지 않으면 겪게 될 것으로 예상되는 실패 모드다.

1. **RTL 채택 시 — Seek 오버레이 연결선 방향** — 검색 바가 화면 우측에 미러링 배치된 상태에서, 검색 바 → 매치 하이라이트로 잇는 연결선의 시작점 계산이 LTR 전제로 하드코딩되어 있으면 엉뚱한 방향에서 선이 시작된다.
2. **RTL 채택 시 — 키캡 일러스트 좌우 배치** — `Presets`/`Hyperkey` 탭의 키캡 일러스트가 미러링 대상인지, 아니면 물리 키보드 그림이라 항상 고정(물리 키보드 자체는 RTL 이 아니므로) 이어야 하는지 상충하는 두 직관이 있다 — §3.1.4 는 배치는 미러링, 기호 순서는 고정으로 정했으나 일러스트 자체의 좌우 반전 여부는 실제 그래픽 자산 기준으로 재확인 필요.
3. **현지화 채택 시 — 긴 번역 라벨로 인한 레이아웃 파손** — `Only show while the remapped key is held` 류의 문장형 라벨이 특정 언어(예: 독일어)에서 원문보다 길어지면, 체크박스와 팝업 버튼이 같은 행에 있는 UI(`superkey-inventory.md` §3.1)에서 줄바꿈되거나 팝업이 화면 밖으로 밀릴 수 있다.
4. **Dvorak/Colemak 에서 `WASD`/`HJKL` 프리셋** — ⭐ **정정(§3.2.7)**: 이전 판은 "물리 위치 고정이라 라벨과 실제 키가 불일치한다"고 서술했으나, 실행 파일에 `wasdArrowDvorak` 등 레이아웃별 별도 keycode 집합이 존재하는 것으로 보아 SuperKey 는 애초에 **레이아웃마다 다른 물리 키 집합**을 써서 인체공학적 의미를 보존하려는 것으로 보인다 `(미확정)`. 이 해석이 맞다면 라벨-실제 키 불일치 문제 자체가 원본에서는 발생하지 않을 수 있다 — 다만 자동 감지가 정확히 언제·어떻게 트리거되는지는 미확정이므로, 감지 실패 시(예: 레이아웃 전환 미인식) 여전히 불일치가 남을 수 있다는 점은 과제로 남는다.
5. **AZERTY 에서 `symbol row` 프리셋** — §3.2.4 에서 다룬 대로, US 하드코딩 심볼 표(`A = !`)를 그대로 쓰면 AZERTY 의 실제 숫자/기호 배치(Shift 관계가 반전됨)와 맞지 않아 엉뚱한 문자가 출력된다.
6. **한글 입력기 조합 중** — §3.2.6 정정된 이해(ASCII 가능 레이아웃 폴백)가 실제로 조합을 깨뜨리지 않는지는 실측 검증이 필요하다.
7. **일본어 IME 변환 확정 대기 중** — 변환 후보가 표시된 상태에서 quick press 가 개입하면 변환이 취소되거나 의도치 않은 후보가 확정될 수 있다. §3.2.6 과 동일 계열이나 IME 종류별 조합 상태 판별 방식이 다를 수 있다.
8. **세션 중 입력 소스 전환** — §3.2.5 절차가 없으면, US 배열로 캐싱된 역방향 매핑 테이블을 그대로 쓴 채 AZERTY 로 전환된 상태에서 quick press 를 실행해 잘못된 문자가 출력된다.
9. **외장 키보드가 다른 물리 레이아웃(JIS 외장 키보드 + ANSI 시스템 설정 등)** — keycode 자체가 물리적으로 다른 배열을 가리킬 수 있어, "물리 위치 기준 판정"이라는 원칙이 흔들릴 수 있는 유일한 경우다. macOS 가 이를 어떻게 정규화하는지는 조사 범위 밖.
10. **`symbol row` 개념이 없는 레이아웃** — 완성형 CJK 입력 방식처럼 "숫자 줄 + Shift = 기호"라는 US 관례 자체가 성립하지 않는 입력 소스에서, 이 프리셋 UI 를 그대로 노출할지 숨길지 미정.
11. **현지화 채택 시 — 번역 누락 시 폴백** — 특정 로케일의 문자열 카탈로그에 일부 키가 비어 있으면(§9, 번역 범위 미확정) `en` 문자열로 개별 폴백되어, 한 화면 안에 두 언어가 섞여 보이는 상태가 될 수 있다.
12. **`UCKeyTranslate` 역방향 탐색 실패** — 현재 레이아웃에 목표 문자(`(` 등)를 낼 수 있는 keycode+modifier 조합이 전혀 없는 경우, §3.2.2 의 (ii) 폴백이 필요하다. 이 폴백이 없으면 quick press 자체가 조용히 무동작한다.
13. **RTL 채택 시 — 숫자·단위 표기의 bidi 혼합** — `1000 ms` 같은 LTR 숫자+단위 문자열이 RTL 문장 흐름 안에 놓이면, 양방향(bidi) 알고리즘이 순서를 뒤섞을 수 있어 격리 처리(예: bidi isolate 문자)가 필요할 수 있다.

---

## 6. 필요한 플랫폼 API

`rust-macos-capability-notes.md` 기준. F-14 는 §2.1(`CGEventTap`)·§2.5(TCC)의 기존 판정을 재사용하고, 이 문서에서 새로 필요한 API 는 아래와 같다.

| API | 용도 | 비고 |
| :--- | :--- | :--- |
| `TISCopyCurrentKeyboardInputSource` | 현재 활성 입력 소스 조회 | Carbon `HIToolbox` 소속. **실측: 번들 심볼**(app-bundle-analysis.md §3.1) |
| ⭐ `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` | 비-ASCII 입력 방식(CJK IME 등) 활성 시 ASCII 가능 레이아웃으로 폴백해 keycode→문자 해석. §3.2.6 정정된 이해의 핵심 근거 | Carbon `HIToolbox` 소속. **실측: 번들 심볼**(app-bundle-analysis.md §3.1) — 이전 판에는 없던 행 |
| `TISGetInputSourceProperty`(`kTISPropertyUnicodeKeyLayoutData`, `kTISPropertyInputSourceID`, `kTISPropertyInputSourceType`) | 레이아웃 데이터 및 입력 소스 종류(키보드 배열 vs 입력 방식/IME) 조회 | 상동. **실측: 번들 심볼** |
| ⭐ `kTISPropertyInputSourceLanguages`(F-16 요구, §3.2.8) | 폴백 교체 **이전** 원본 입력 소스의 언어 판정 — `korean-input.md` §3.3 이 낸 요구 | `TextInputSources.h`. `CFArrayRef`(원소 `CFString`), Get 규칙 |
| `kTISNotifySelectedKeyboardInputSourceChanged` | 입력 소스 변경 알림 | `CFNotificationCenterGetDistributedCenter()` 에 옵저버 등록 |
| `UCKeyTranslate` | keycode+modifier → 문자 정방향 변환. §3.2.2 (i) 의 역방향 탐색 테이블을 만드는 기반 | Carbon `HIToolbox` 소속. **실측: 번들 심볼** |
| `LMGetKbdType` | 현재 키보드 하드웨어 타입 조회 | Carbon 소속. **실측: 번들 심볼**(app-bundle-analysis.md §3.1) — 이전 판에는 없던 행 |
| `CGEventGetIntegerValueField`(`kCGKeyboardEventKeycode`) | 물리 keycode 판정(§3.2 원칙의 핵심) | Core Graphics — F-07 이 이미 설치한 event tap 콜백 안에서 호출 |
| `CGEventCreateKeyboardEvent` / `CGEventSetFlags` | 합성 keyDown/keyUp 이벤트 생성, modifier 비트 추가(§3.2.3 paste 사례) | Core Graphics |
| `CGEventKeyboardSetUnicodeString` | §3.2.2 (ii) — 레이아웃 독립 문자 직접 주입(폴백 경로) | Core Graphics |
| `NSBundle`/`NSLocale`(`preferredLocalizations`, `CFBundleLocalizations`) | §3.1.2 로케일 결정 — **클론이 현지화를 채택하기로 결정한 경우에만 필요** | Foundation |
| `NSApplication.userInterfaceLayoutDirection` / `NSLocale.characterDirection(forLanguage:)` | RTL 여부 판정(§3.1.4) — **클론이 RTL 로케일을 채택하기로 결정한 경우에만 필요** | AppKit / Foundation |
| CJK 조합 상태(marked text) 조회 | ⭐ **판정 변경**: §3.2.6 이 marked text 직접 감지 대신 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 폴백을 쓰는 것으로 보인다는 실측이 나오면서, 이 행의 필요성 자체가 낮아졌다. 다만 그 해석이 틀렸을 경우를 대비해 후보로만 남긴다 `(미확정)` | 조사 노트에 대응 크레이트·API 목록 없음 — §9 |

---

## 7. 구현 접근

이 문서의 판정은 아래 3분류를 기준으로 한다(다른 F-0N 문서들과 동일한 척도).

- **순수 Rust** — 안전(safe) 래퍼 크레이트만으로 커버되어 `unsafe` 도 네이티브 소스도 없음
- **Rust 바인딩** — `unsafe` FFI 직접 호출이 필요하다. `objc2-*` 계열 헤더 자동생성 바인딩을 직접 호출하거나, 대응 크레이트가 없으면 `extern "C"` 선언을 손으로 작성해 프레임워크에 직접 링크한다. 어느 쪽이든 Swift/Objective-C 로 별도 컴파일 산출물(shim 바이너리)을 두지는 않는다
- **네이티브 shim 불가피** — Rust 에서 호출할 방법이 없어 Swift/Objective-C 소스를 별도 컴파일해 링크해야 함

### (A) 현지화 — 판정: **Rust 바인딩** (클론이 채택하기로 결정한 경우)

- 로케일 결정(§3.1.2)과 RTL 판정(§3.1.4)에 필요한 `NSBundle`/`NSLocale`/`NSApplication.userInterfaceLayoutDirection` 은 `objc2-foundation` 0.3.2, `objc2-app-kit` 0.3.2 로 노출된다(`rust-macos-capability-notes.md` §1.1). 다만 이 크레이트들은 "헤더 자동생성 바인딩이며 안전한 상위 래퍼는 없고 호출부는 `unsafe`"라고 조사 노트 §1.1 이 명시하므로 `순수 Rust` 가 아니라 `Rust 바인딩` 이다.
- 문자열 카탈로그 자체(§3.1.5 의 파일 파싱·조회)는 플랫폼 API 호출이 아니라 순수 Rust 로직(직렬화 포맷 파싱)이라 3분류 판정 대상 밖이다.
- 기각한 대안: 문자열 카탈로그를 네이티브 `.strings`(macOS 표준 현지화 포맷)로 관리 — macOS 관례에는 맞지만, §3.1.5 의 "단일 소스" 결정과 정면으로 충돌한다. WebView 쪽이 `.strings` 를 직접 파싱할 표준 경로가 없어(웹 기술 스택과 이질적), 결국 빌드 시 JSON 으로 변환하는 이중 관리가 되어 drift 위험이 되살아난다. 기각.

### (B) 키보드 입력 소스 독립성 — 판정: **Rust 바인딩**

- `CGEventTap`·`CGEventGetIntegerValueField`·`CGEventSetFlags`·`CGEventCreateKeyboardEvent`·`CGEventKeyboardSetUnicodeString` 은 `core-graphics` 0.25.0(안전 래퍼) 또는 `objc2-core-graphics` 0.3.2(헤더 자동생성)로 커버된다(조사 노트 §1.1, §1.2, §2.1). `core-graphics` 가 `CGEventTap` 안전 래퍼를 이미 제공한다는 점에서 이 부분만 보면 `순수 Rust` 에 가깝지만, `CGEventKeyboardSetUnicodeString` 처럼 조사 노트가 명시적으로 나열하지 않은 개별 함수는 `objc2-core-graphics` 의 헤더 자동생성 경로(`unsafe` 수준)로 보완해야 할 가능성이 있어, 전체를 `Rust 바인딩` 으로 판정한다.
- ⭐ **`TISCopyCurrentKeyboardInputSource`/`TISCopyCurrentASCIICapableKeyboardLayoutInputSource`/`TISGetInputSourceProperty`/`kTISNotifySelectedKeyboardInputSourceChanged`/`UCKeyTranslate`/`LMGetKbdType` 는 Carbon `HIToolbox` 소속이며, `rust-macos-capability-notes.md` 의 크레이트 표(§1.1, §1.2) 어디에도 `HIToolbox`/`Carbon`/`TIS`/`UCKeyTranslate` 를 다루는 크레이트가 등장하지 않는다.** 정직하게 말하면: **전용 크레이트 미확인, `extern "C"` 직접 선언 필요 `(추정)`.** `Carbon.framework`(또는 그 하위 `HIToolbox.framework`)에 `#[link(name = "Carbon", kind = "framework")]` 로 직접 링크하고, 필요한 함수 시그니처를 손으로 선언하는 접근이 될 것으로 본다. 이는 여전히 "Swift/ObjC shim 바이너리"가 아니라 Rust 안에서의 `unsafe extern` 선언이므로 `Rust 바인딩` 으로 분류하되, 같은 분류 안에서도 "크레이트가 있어 감싸기만 하면 되는 경우"보다 비용이 크다는 점을 §9 에 남긴다. ⭐ **정정**: 이전 판은 이 목록에 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource`·`LMGetKbdType` 이 빠져 있었다 — 실측(app-bundle-analysis.md §3.1)으로 추가됐다. 이 API 들도 같은 Carbon `HIToolbox` 소속이라 판정은 바뀌지 않는다.
- §3.2.6 의 CJK 대응 판정에서 `(추정)` 을 지운다: 이전 판은 "CJK 조합 상태 조회 API 가 조사 노트에 아예 언급이 없어 판정을 내릴 수 없다"고 했으나, 실측으로 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 링크가 확정되어 위 Carbon `HIToolbox` 그룹과 동일하게 `Rust 바인딩`(수기 `extern "C"` 선언)으로 판정한다. marked text 를 별도로 조회하는 API 는 이 해석이 맞다면 애초에 필요 없다(§3.2.6).
- 기각한 대안:
  - **`rdev` 0.5.3** — 조사 노트가 "3년간 릴리스 없음, 이벤트 소비 제어 제한적"으로 명시(§1.2). §3.2.3 의 정밀한 이벤트 소비·치환·flag 조작 요구를 충족하지 못한다.
  - **문자 기반 판정으로 되돌아가기(비교 대안)** — 구현이 단순해 보이지만, 조사에서 확정된 v1.51/v1.52 세 건의 회귀를 그대로 재현하는 접근이라 원천적으로 기각. §3.2.1 원칙과 정면으로 배치된다.
  - **네이티브 Swift shim 으로 `HIToolbox` 호출부만 분리** — Carbon API 자체는 C ABI 라 Rust `extern "C"` 로 직접 호출 가능하고 별도 프로세스 경계나 Swift 런타임이 필요 없다. shim 을 두면 오히려 IPC 오버헤드와 배포 복잡도만 늘어나 기각.

---

## 8. 수용 기준

### (A) 현지화 — ⚠️ 클론이 현지화를 채택하기로 결정한 경우에만 적용

⭐ **정정**: 이전 판은 "8개 번들 로케일"을 확정 사실로 전제한 기준을 실었다. 이는 오독에 근거한 것이었으므로(§1) 삭제하고, 로케일 집합을 클론이 정하는 것으로 되돌린 아래 조건부 기준으로 대체한다.

- [ ] (채택 시) 앱이 시스템 언어를 클론이 지원하기로 선언한 로케일 집합 중 하나로 정확히 매핑해 실행하며, 지원하지 않는 언어는 `en` 으로 폴백한다.
- [ ] (채택 시, RTL 로케일 포함) RTL 로 분류된 로케일에서 환경설정 창의 사이드바·패널 좌우 배치가 미러링되고, modifier 기호(`⌃⌥⌘⇧`) 표기 순서는 로케일과 무관하게 고정된다.
- [ ] (채택 시, RTL 로케일 포함) Seek 오버레이 검색 바가 RTL 로케일에서 우측 정렬로 표시되며, 검색 바 → 매치 연결선은 화면 절대 좌표 기준으로 정확히 그려진다.
- [ ] (채택 시) 메뉴바 메뉴와 시스템 알림, 환경설정 창(WebView)의 문자열이 동일한 로케일 카탈로그 파일 하나에서 나오며, 두 경로 사이에 번역 키 누락 차이가 없다.
- [ ] (미채택 시) `General` 탭 등 어디에도 언어 선택 UI 가 없다 — 실측 확정 사실이므로(§4 항목 1) 클론이 현지화를 채택하지 않기로 했다면 이 상태를 그대로 유지한다.

### (B) 키보드 입력 소스 독립성 — 상시 적용

- [ ] `Quick press left or right shift to input corresponding: ( )` 이 AZERTY·Dvorak·QWERTZ 각 입력 소스에서 물리 keycode 기준으로 동일하게 트리거되고, 출력 문자는 각 입력 소스에서 실제로 `(`/`)` 로 나타난다.
- [ ] Seek 에서 세미콜론으로 다음 매치를 선택하는 동작이 물리 `kVK_ANSI_Semicolon` keycode 기준으로 판정되어, 해당 위치에 다른 문자가 배정된 레이아웃(AZERTY 등)에서도 동일한 물리 키로 트리거된다.
- [ ] `Remap paste to paste w/o formatting` 이 keycode+modifier 조합(`⌘V` 감지, `⌘⌥⇧V` 합성)으로 구현되어 있으며, 이 합성 이벤트가 대상 앱에서 실제 단축키로 인식된다(`CGEventKeyboardSetUnicodeString` 을 쓰지 않는다).
- [ ] `UCKeyTranslate` 역방향 탐색으로 목표 문자를 낼 수 없는 레이아웃에서, quick press 출력이 조용히 무동작하지 않고 `CGEventKeyboardSetUnicodeString` 폴백으로 대체된다.
- [ ] `kTISNotifySelectedKeyboardInputSourceChanged` 수신 시 캐시된 keycode↔문자 매핑이 즉시 무효화·재구축되며, 재구축 완료 전 도착한 quick press 이벤트가 stale 매핑으로 잘못된 문자를 출력하지 않는다.
- [ ] `home row = symbol row` 프리셋이 산출하는 실제 심볼이 US-QWERTY 하드코딩 표가 아니라 현재 입력 소스 기준으로 계산된다.
- [ ] ⭐ **신규**: `Caps lock + W A S D`/`[H J K L]` 방향키 프리셋이 감지된 키보드 레이아웃(QWERTY/Colemak/Dvorak)에 따라 인체공학적으로 동등한 물리 키 집합에서 트리거된다(§3.2.7) — 단, 자동 감지가 실패하는 경우의 폴백 동작은 실측이 더 필요하다(§9).
- [ ] 한글 또는 일본어 입력기 활성 중 문자 출력형 리매핑이 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 폴백으로 정상 동작하고 조합을 깨뜨리지 않는다(§3.2.6 검증).

---

## 9. 미해결 질문

### 9.1 이번 실측으로 해소됨

1. ~~로케일 목록(8개, RTL 3개)~~ — **해소가 아니라 오독 정정**: 원본은 현지화 기능이 없다(§1). 이전 판이 "번들 로케일 8개" 를 확정 사실로 다뤘던 것 자체가 잘못이었다.
2. ~~앱 내 언어 선택 UI 존재 여부~~ — **해소**: 실측으로 **없음**이 확정됐다(app-bundle-analysis.md §6.4, `General` 탭).
3. ~~`home row`/`symbol row` 팝업의 전체 대안 목록~~ — **해소**: `symbol row (A = !)` · `function row (A = F1)` 2종 확정(§4 항목 4).
4. ~~`Quick press left or right shift` 출력 후보 전체~~ — **해소**: `( )` · `[ ]` · `{ }` · `< >` 4종 확정(§4 항목 5).
5. ~~CJK 대응의 API 근거~~ — **부분 해소**: 이전 판은 "조사 노트에 대응 API 가 전혀 언급되지 않는다"고 했으나, 실측으로 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 심볼 링크가 확정됐다(§3.2.6). 다만 이 API 의 정확한 호출 지점과 실제 동작(한글 입력기로 실측 검증)은 여전히 미확정이므로 §9.2 로 승계한다.

⭐ **이번 조사에서 얻은 교훈 — 왜 로케일 오독이 생겼는가**: 이전 판은 Sparkle 델타 appcast 의 `sparkle:deltaFromSparkleLocales` 속성(업데이트 인프라가 건드린 파일 목록이라는 **간접 증거**)을 앱 자신의 UI 로케일 지원(**직접 증거**가 필요한 주장)의 근거로 삼았다. 간접 증거는 그것이 무엇을 직접 가리키는지(이 경우 "Sparkle 프레임워크의 로케일 파일") 검증하지 않으면 엉뚱한 결론(이 경우 "SuperKey 자신의 로케일")으로 이어질 수 있다는 것이 이번 사례의 교훈이다. 앞으로 번들 구조·프레임워크 경계를 넘나드는 간접 증거를 쓸 때는, 그 증거가 정확히 어느 컴포넌트에 속하는지(`Contents/Resources/` 인지 `Contents/Frameworks/*/Resources/` 인지 등, app-bundle-analysis.md §5.1 의 구분)를 먼저 확인해야 한다.

### 9.2 남은 질문

1. ~~**클론이 현지화를 채택할지의 제품 결정**~~ — ✅ **해소.** 제품 결정 D4 로 **채택 확정, 로케일은 `en` + `ko` 2종**(§3.1.1, `README.md` 제품 결정 표, 이슈 #5). 카탈로그 구조는 M1 에서 구현되었다. 남은 것은 아래 #10 으로 승계한다.
2. **키 이름 번역 정책의 근거 보강** — §3.1.3 에서 "물리 키 이름을 번역한다"고 정책 결정했으나, macOS 가 실제로 로케일별 어떤 문자열을 쓰는지(예: 독일어 키 이름의 정확한 표기)는 근거가 없는 `(추정)`이다. 현지화 채택이 확정되면 macOS 시스템 설정의 실제 로케일별 문자열을 확인해 보강 필요.
3. **로케일 변경의 실행 중 반영 여부** — (현지화 채택 시) 시스템 언어를 실행 중 바꿨을 때 재시작 없이 UI 에 반영되는지 `(추정)` 상태이며 확정 필요.
4. **레이아웃 변형(Colemak/Dvorak)의 자동 적용 여부와 감지 시점** — §3.2.7 에서 새로 발견된 `wasdArrowColemak` 등 내부 키가 실제로 언제·어떻게 감지된 레이아웃에 자동 적용되는지는 팝업 부재로부터의 해석일 뿐이다. Colemak/Dvorak 입력 소스로 전환 후 실동작 비교가 필요하다. `power-user-presets.md`(F-08) §9 항목 3 과 동일 질문.
5. **CJK 조합 상태와 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 폴백의 실제 동작** — 심볼 링크는 확정됐으나(§3.2.6), 정확한 호출 지점과 한글/일본어 입력기 조합 중 실제 동작은 실측 검증이 필요하다(Screen Recording 권한·조합 방해 부작용 때문에 이번 조사에서는 하지 않았다).
6. **`Apply modifiers`류 이벤트와 IME 상호작용** — CJK 입력기 활성 중 `F-05` 의 Hyperkey modifier 합성이 IME 자체의 단축키(예: 한글/영어 전환 키)와 충돌하는지는 조사 범위 밖.
7. **`UCKeyTranslate`/`TIS*` 의 Rust FFI 세부** — 전용 크레이트가 없다는 것은 확정이나(§7), `Carbon.framework` 링크가 Tauri 빌드 파이프라인(코드 서명·번들링)과 충돌 없이 동작하는지는 실측 필요.
8. **입력 소스 재구축 대기 중 이벤트 처리 정책** — §3.2.5의 4번 항목, 재구축 완료까지 대기할지 해당 1회를 무시할지 미결정.
9. **RTL 키캡 일러스트의 실제 그래픽 자산 미러링 여부** — (현지화·RTL 채택 시) §3.1.4/§5 의 상충하는 직관을 실제 디자인 자산 확보 후 재확인 필요.
10. ⭐ **`ko` 번역의 품질·용어 일관성** (D4 채택으로 새로 생긴 질문) — M1 이 만든 카탈로그는 권한 온보딩·치명적 실패·메뉴바 인터페이스에 해당하는 최소 키 집합뿐이다. UI 가 커지는 M2(F-09 환경설정 창)에서 **§3.1.3 의 "물리 키 이름을 번역한다" 정책**이 실제로 어떤 한국어 표기를 쓸지(예: `Caps Lock` → "Caps Lock" 유지인가 "고정" 인가)는 미확정이다. macOS 한국어 시스템 설정의 실제 표기를 확인해 맞추는 것이 §3.1.3 의 채택 근거("사용자가 이미 자국어 macOS 환경에서 보는 이름과 일치")와 정합적이다.
11. ⭐ **RTL 로케일을 언제 추가할 것인가** — D4 는 `en`+`ko` 만 확정했고 둘 다 LTR 이라 §3.1.4 미러링 규칙에 현재 적용 대상이 없다. 규칙은 명세에 남아 있으나 **검증된 적이 없다** — RTL 로케일을 실제로 추가하는 시점에 §5 항목 1·2·13(연결선 방향, 키캡 일러스트 미러링, bidi 숫자 혼합)을 처음부터 재검토해야 한다.
12. **서드파티 한국어 입력기에서도 `kTISPropertyInputSourceLanguages` 첫 원소가 `"ko"` 인가** (F-16 요구, §3.2.8) — ⭐ **Apple 기본 입력기에 대해서는 해소됐다**(§3.2.8 실측 표: 한국어 6종 전부 `["ko"]`). 남은 것은 구름 입력기 등 **서드파티** 한국어 입력기이며, 이 기기에 설치돼 있지 않아 확인하지 못했다. `korean-input.md` §9 항목 3 과 동일 질문 — 해당 입력기를 설치한 뒤 `docs/dev/tools/tis-language-probe.swift` 를 다시 돌리면 된다.
