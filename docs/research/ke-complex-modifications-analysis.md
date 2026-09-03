# KE 복합 변형 카탈로그 분석 — 언어별 프리셋 후보 분류

> **한 줄 요약** — Karabiner-Elements 복합 변형 카탈로그(https://ke-complex-modifications.pqrs.org/)를 전수 분석해, Ultrakey 가 언어 설정을 제공하는 언어(`ko`·`ja`·`zh`·`es`) 각각에 언어별 프리셋으로 제공할 패턴을 분류·웹 검증했다. 판정: `ko` 신규 2종 + F-16 참조, `ja` 4종 채택, `zh` 1종 채택(제3자 IME 한정) + 다언어 공통 후보 1종, `es` **추가 안 함**(사용도 증거 부족). 채택 근거에는 전부 웹 검증 등급과 출처를 붙였다.
> **성격** — 이 문서는 `docs/spec/language-presets.md`(F-19) 명세 초안의 **사실 토대**다. F-16(한국어 입력)이 "한국어 macOS 사용자가 실제로 운용 중인 Karabiner 규칙 4종"에서 도출된 것처럼, F-19 의 프리셋 세트는 이 문서의 분류·검증에서 도출한다.
> **조회일** — 2026-09-03. 사이트는 JavaScript(React) 앱이라 렌더링 HTML 이 아닌 **데이터 소스 저장소**(아래 §2)를 조회했다.

---

## 1. 조사 방법

- **데이터 소스**: 카탈로그 사이트의 실데이터는 GitHub 저장소 `pqrs-org/KE-complex_modifications` (기본 브랜치 `main`) 이다. 사이트는 컴파일된 React SPA(`/assets/index-DAI38KQG.js`)이며 런타임에 `dist.json` 을 fetch 한다 — 그 빌드 산출물은 Core 서브모듈(`pqrs-org/KE-complex_modifications-core`, `.gitmodules`·`Makefile` 로 확인)이 만든다. 이 저장소의 `public/json/`(규칙 원본) 과 `public/groups.json`(카테고리 인덱스) 이 사실의 1차 근거다. (실측: curl)
- **규칙 파일 형식**: 각 파일은 Karabiner 복합 변형 JSON — `{"title", "maintainers", "rules": [{"description", "manipulators": [...]}]}`. (실측)
- **수집**: 전수 디렉터리 조회(840 파일) + 그룹 인덱스 대조 + 국제 카테고리 72 파일 전체 목록 확보 후, **Ultrakey 의 대상 언어(ko·ja·zh·es)와 직접 관련된 파일 36개**(ko 10 · ja 12 · zh 7 · es 3 · 다언어·공통 4)의 manipulator 원문 분석 + 웹 검색으로 언어별 사용도 검증(출처 URL 포함, 등급 부여).
- ⭐ **대상 범위 명시**: International 72 파일 중 나머지 36 개는 de·fr·ru·키릴·nordic 등 **Ultrakey 가 언어 설정을 제공하지 않는 언어**(F-14 로케일 = en·ko·zh·es·ja 5종)의 파일이다 — 개별 manipulator 분석 대상은 아니고 목록 대조(§3.1~§3.4 의 테이블이 72 파일 중 대상 언어 관련분 전수)로 처리했다. `(실측 — 이슈 #113 작업 범위: 로케일 4종)`
- **등급 체계**: 이 문서의 "등급" 열은 `docs/spec/language-presets.md` §3 의 것과 동일하다 — ① 실사용(실제 사용자가 운용 중인 직접 증거) · ② 논의(포럼·이슈·블로그 추천) · ③ 간접·추정 · ④ 근거 부족.

---

## 2. 카탈로그 구조 (전수 파악)

### 2.1 파일 수와 인덱스 — 840 vs 615 불일치 해소 ⭐

| 구분 | 수 | 비고 |
| :--- | :--- | :--- |
| `public/json/` 실제 파일 | **840** | 전부 `.json` (저장소 트리 조회, 2026-09-03) |
| `groups.json` 인덱스 전체 | **615** | json 611 + js 4. **그룹 간 중복 등재 0건** (참조 615개 전부 고유 경로) |
| 인덱스 미등재 json | **229** | 840 − 611. 이하 귀속처 확인 |
| └ 빌드 시 `Others` 그룹(`dist.json` `others`)으로 편입 | **224** | 라이브 사이트는 이 파일들을 "Others" 탭으로 노출한다 |
| └ `example` 키로 편입 (인덱스 아님) | **5** | `example_device`·`example_halt`·`example_input_source`·`example_keyboard_type`·`example_select_input_source` — 기여자용 예시 |

> ⭐ **불일치의 설명은 "미등재 파일" 하나뿐이다.** 중복 등재는 인덱스 수를 파일 수보다 *부풀리지* 줄이지 않으므로(관측은 파일 840 > 인덱스 611) 이 관측을 설명할 수 없다. (실측)
> ⭐ **라이브 사이트는 `groups.json` 이 아니라 빌드 산출물 `dist.json` 을 쓴다**(사이트 번들에서 `fetch("dist.json")` 확인). `groups.json` 의 `others` 는 0 개지만 `dist.json` 의 `others` 는 224 개 — 그룹별 수가 다르다. 카탈로그 구조를 말할 때는 이 차이를 구분해야 한다.
> ⭐ **다운로드/임포트 횟수·star·사용 통계는 노출되지 않는다.** 사이트 번들에 downloads/imports/popular/stars/usage 계열 문자열이 0 매치다. (실측) 대신 있는 것은: ① 검색(자체 tokenizer, URL 파라미터 `q`, 검색 제안 23개 칩 + 결과 수 배지) ② 파일 카드의 **`maintainer` 칩**(`Author: X` GitHub 링크 — 필터는 아니고 표시·정렬 신호) ③ 정렬(자동 — `attributed(저자/설명 존재) > documented(추가 설명) > 설명 길이` 우선순위로 그룹 내 파일·검색 결과를 정렬. **사용자 선택형 정렬 UI 없음**) ④ 그룹별 파일 수 배지 ⑤ Karabiner 딥링크 임포트 버튼(`karabiner://…import?url=…`). (실측: 번들 문자열)

### 2.2 카테고리 13종

`groups.json` 은 **13개** 그룹이다(최초 조사 문건의 "14개"는 오기 — 실측 정정). 각 그룹 파일 수는 아래와 같고 합계 615 이다. (실측)

| id | name | 파일 수 |
| :--- | :--- | :--- |
| os-functionality | OS Functionality | 61 |
| application-specific | Application Specific | 79 |
| modifier-keys | Modifier Keys | 81 |
| emulation-modes | Emulation Modes | 76 |
| alternative-keyboard-layouts | Alternative Keyboard Layouts | 29 |
| international | International (Language Specific) | **72** |
| key-specific | Key Specific | 98 |
| device-specific | Device Specific | 51 |
| personal-settings | Personal Settings | 46 |
| miscellaneous | Miscellaneous | 16 |
| Command arrows and home end | Command arrows and home end | 5 |
| Trackball Tools | Trackball Tools | 1 |
| others | Others | 0* |

\* `groups.json` 상 0 개. 라이브 `dist.json` 의 `others` 는 **224 개**(§2.1) 이다.

⭐ **언어 관련 규칙은 `international` 한 곳에만 있지 않다.** `alternative-keyboard-layouts`(29 — JIS→US·스페인어 레이아웃 계열), `key-specific`, `emulation-modes` 등에도 언어·레이아웃 규칙이 섞여 있다. §3 의 각 언어 표의 "(카테고리)" 열이 이 사실을 좇는다 — ko 10 개 중 3 개(`nowage_eng_char_on_kor`·`korean-japanese-english`·`print_screen…` = `personal-settings`), ja 12 개 중 3 개(`jis_to_us_symbols`=alternative·`multi-layered-japanese`=emulation·`jis_pretend_remote_us`=device), zh 7 개 중 1 개(`double_tap_right_command…`=personal-settings) 가 international 밖이다.

### 2.3 규칙 수 추정

전수 규칙(manipulator) 수는 카탈로그가 공개하지 않으므로 표본 추정한다. seed 42 무작위 20 파일 직접 fetch → 규칙(rules) 수 총 55(평균 2.75/파일), manipulator 수 총 425(평균 21.25/파일, **중앙값 3.5**). 단일 outlier(`caps_lock_enhancement.json` 10 규칙·292 manipulator)가 표본의 69%를 차지해 평균이 크게 왜곡된다. 840 파일 전체 추정치: **규칙 ≈ 2,310 / manipulator ≈ 5,900~17,900** (outlier 제외 평균 7.0/파일 기준 ≈ 5,880 ~ 표본 평균 21.25 기준 ≈ 17,850). `(추정 — 표본 20/840, 관측된 극단 편향)`

---

## 3. 언어별 분류 — 후보 규칙 파일 전수 분석

> 36 개 파일의 manipulator 원문을 읽어 패턴 태그로 분류했다. 태그: **(a) 입력 소스 전환** · **(b) 문자 치환**(₩→`` ` ``, ¥↔\ 등) · **(c) dead keys/악센트** · **(d) 英数/かな(Eisu/Kana) 처리** · **(e) 전각/반각** · **(f) modifier 키 재할당**(단독 탭 이중화 포함) · **(g) 특정 IME 전용**.
> "Karabiner 고유 의존도" 열은 Ultrakey 가 흡수할 때 Karabiner 전용 기구를 우회해야 하는 정도다 — `select_input_source`/`input_source_if`/`set_variable`/`to_delayed_action`/`to_if_alone`(lazy)/`vk_none`/`hold_down_milliseconds`/`mouse_key`/`simultaneous` 류가 고유 기구고, 순수 `key_code` 치환·합성은 낮음이다.
> ⚠️ **패턴 (e) 전각/반각은 이번 후보 전수에서 파일이 0 개다** — "전각/반각 전환" 규칙이 유의미하게 존재하지 않았다. `(실측: 36 파일 전수 + international 72 파일 목록 대조)`

### 3.1 한국어 (ko)

F-16(이슈 #20)은 이미 사용자의 Karabiner 규칙 4종을 정본으로 명세 확정 — 그래서 아래는 **F-16 과의 관계**(흡수/참조/신규)를 함께 단다.

| 파일 (카테고리) | title | 무엇을 하는가 | 패턴 | Karabiner 고유 의존 | F-16 관계 |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `for_korean_keyboard.json` (international) | For Korean Keyboard | `Shift+Space`·`right_command`·`lang1` → `⌃Space` 합성(입력 소스 전환), `right_option`·`lang2` → `⌥Return`(한자 변환). 앱 제외 28종(터미널·RDP·VM) | a | 낮음~중간 (macOS 기본 단축키 합성) | **흡수됨** — F-16.1~F-16.3 의 1차 근거 (korean-input.md §1) |
| `korean_pc.json` (international) | For Korean PC Keyboard | `lang1` → `⌘Space`, `lang2` → `⌥Return`. 조건 없음 | a | 낮음 | **참조** — F-16.2·F-16.3 선례의 대안 구현 |
| `korean_won_to_backtick.json` (international) | For Korean | `₩` 키 → `⌥`+grave 합성(백틱). `input_source_if ko` | b | 중간~높음 | **흡수됨** — F-16.4 의 1차 근거 |
| `korean_won_to_backtick_with_shortcut_preserved.json` (international) | Korean Won to Backtick with shortcut preserved | 위와 같되 `optional: []` 으로 단독 탭만 치환 → `⌘+₩` 류 단축키 보존 | b | 중간~높음 | **이미 커버** — F-16.4 의 modifier 부재 조건이 동일 가치를 보장(korean-input.md §3.1). 신규 규칙 불필요 |
| `caps_lock_toggle_korean_english_include_parallels.json` (international) | CapLock to Korean <-> English | 캡스락 → `F19`(시스템 설정에서 한/영 전환 단축키로 위임), Parallels 안에서만 `right_option` | a·f | 중간 | ⭐ **신규 후보** (F-16 은 캡스락을 다루지 않는다) |
| `ChangeInputSourceDirectlyForKorean.json` (international) | Change input-source directly for korean | `right_command` 단독 → `select_input_source`(Gureum ↔ ABC / 2SetKorean ↔ ABC) | a·g | 높음 (select_input_source + IME ID 하드코딩) | ⭐ **신규 후보** — 단 아래 §4 의 CJK 버그 때문에 직접 전환 방식은 기각한다 |
| `left_shift_change_english_input.json` (international) | Left Shift → Change to/from English input | `left_shift` 단독 탭 → `caps_lock` — macOS 의 "캡스락으로 ABC 전환" 기능에 위임(설명에 macOS 설정 켜기 명시) | a | 낮음~중간 (macOS 기능 의존) | 신규 후보였으나 → §5 판정 참조 |
| `nowage_eng_char_on_kor.json` (personal-settings) | Eng Char on Kor | 한글 입력 중 영문 문자 자동 입력 — backspace 로 조합 취소 → en 전환 → 키업 시 ko 복귀, 대문자 모드 변수 | a·g | 높음 (변수·vk_none·hold_down 타임 해킹) | 검토 대상 — 타이밍 해킹 의존(§5 판정 참조) |
| `korean-japanese-english.json` (personal-settings) | 한/영/일 언어전환 쉽게 | JIS 키보드: `かな` 더블탭 → 한글, US: `⌘` 좌/우 단독 → en/ja · 우`⌘` 더블탭 → 한글 | a·d | 높음 (더블탭 상태기계) | 검토 대상 — 다언어 전환 |
| `print_screen-EN-scroll_lock-JP-pause-KR.json` (personal-settings) | Switch EN/JP/KR with print_screen… | `print_screen`→en · `scroll_lock`→ja · `pause`→ko (`select_input_source language`) | a | 높음 (select_input_source) | 검토 대상 — 다언어 전환 |

### 3.2 일본어 (ja)

| 파일 (카테고리) | title | 무엇을 하는가 | 패턴 | Karabiner 고유 의존 |
| :--- | :--- | :--- | :--- | :--- |
| `japanese.json` (international) | For Japanese (rev 6) | 13 규칙 — `⌘`·`⌃`·`⌥`·`⇧` 단독 탭 → 英数/かな 배분, `caps_lock` 을 input_source_if(ja/en)로 英数⇔かな 토글, eisuu/kana 동시 탭 → option, `Esc`·`⌃[` → 英数 첨가(vim) | a·d | 중간~높음 (to_if_alone/lazy/threshold) |
| `ei-kana-cmd.json` (international) | 英かな/⌘ for Japanese | US: `⌘` 단독 → 英数/かな · JIS: 英数/かな → `⌘` (단독이면 원 키). `⌘W/Q/⇧W` 더블탭 가드 | d·f | 중간~높음 (더블탭 상태기계) |
| `eisuufn.json` (international) | EisuuFN | 英数 키 → `fn` 레이어 스위치(단독이면 英数) | d·f | 중간 |
| `kanafn.json` (international) | KanaFN | かな 키 → `fn` 레이어: fn+ijkl 방향키·fn+h; ⌫/⌦·fn+op PgUp/Dn·fn+,. Home/End·fn+n Enter | d·f | 낮음~중간 (fn mandatory) |
| `jis_to_ascii.json` (international) | JIS配列をASCII配列風にする設定 | 英数→`⌘`·かな→`⌘`(단독이면 원 키), `⌘`↔`⌥` 스왑, ¥(`international3`) → `` ` `` | b·d·f | 낮음~중간 |
| `jis_to_us_symbols.json` (alternative-keyboard-layouts) | Japanese JIS to US Keyboard: Remap Symbol Keys | JIS 심볼 키 18 종 → US 배열(symbol row 대응) — ⚠️ **"18 종" 은 오기, 원문은 20 행**(2026-09-03 fetch 대조 — "18 종" 정정은 `docs/spec/language-presets.md` §3.2.1 참고) | b | 낮음~중간 (keyboard_type_if jis) |
| `jis_pretend_remote_us.json` (device-specific) | US (remote) ← JIS (local) | 원격(Chrome/AnyDesk) 접속 중에만 JIS → US 심볼 19 종 | b | 중간 (frontmost_application_if) |
| `swap_yen_and_backslash.json` (international) | Swap ¥ and \ … on US Keyboards | `\` ↔ `⌥\`(¥) 스왑 — US 키보드의 일본어 로마자 입력용 | b | 낮음 |
| `swap_yen_and_backslash_jis.json` (international) | Swap ¥ and \ always on JIS | JIS 판 — `international3`(¥) ↔ `⌥international3`(\) | b | 낮음 |
| `rdp-japanese-us.json` (international) | RDP for Japanese, US Keyboard | RDP 밖: `⌘` 단독 → 英数/かな · RDP 안: `⌘`↔`⌥` 스왑 | d·f | 중간 (frontmost 조건) |
| `multi-layered-japanese.json` (emulation-modes) | Multi-Layered Keymap for Japanese Keyboards | 英数 홀드 = Cursor/Mouse/Web/Desktop 레이어, かな 홀드 = NumPad 레이어(mouse_key·pointing_button) | d·f | 높음 (레이어·mouse_key) — **JIS 파워유저 도구, 언어 문제가 아니다** |
| `left_cmd-EN-right_cmd-CH-right_option-JA.json` (international) | Switch EN/CH/JA with left_cmd… | 좌`⌘` 단독 → en · 우`⌘` 단독 → zh-Hans · 우`⌥` 단독 → ja (`select_input_source language`) | a·f | 높음 (select_input_source) — 다언어 공통 |

### 3.3 중국어 (zh)

| 파일 (카테고리) | title | 무엇을 하는가 | 패턴 | Karabiner 고유 의존 |
| :--- | :--- | :--- | :--- | :--- |
| `caps_lock_toggle_chinese_english.json` (international) | Toggle Chinese English With caps_lock | 캡스락 단독 탭 → `⌃Space`, 길게 → 캡스락 본래 | a·f | 중간 (⌃Space 합성) |
| `CapslockChinese.json` (international) | Capslock for Chinese | 캡스락 단독 → `⌃⌥Space`, 길게 → 캡스락 | a·f | 중간 (시스템 단축키 의존) |
| `left_option_to_chinese.json` (international) | Switch to Simplified Chinese by pressing left-alt alone | 좌`⌥` 단독 탭 → zh-Hans (`select_input_source`) | a | 높음 (select_input_source) |
| `quickly_chinese_jis.json` (international) | Quickly switch to Chinese input on a JIS keyboard | JIS `かな` 더블탭 → zh·`かな`+英数 동시 → zh (`select_input_source zh*` 와일드카드) | a·d | 높음 (simultaneous·변수) |
| `double_tap_right_command_switch_doubaoime.json` (personal-settings) | …DoubaoIME voice input | 우`⌘` 더블탭 → DoubaoIME 음성 입력 ↔ Rime(Squirrel) 복귀 | **g** | 높음 (IME ID 하드코딩 + 타임 해킹) — **중국어 일반 문제가 아니다** |
| `windows_style_IME_switcher.json` (international) | windows_style_IME_switcher | `⌃`+`⇧` 단독 탭 → "다음 입력 소스" 합성 · `⌃Space`: en 이면 다음 소스로, 아니면 en 으로 | a | 높음 (select_input_source + 조건) |
| `left_cmd-EN-right_cmd-CH-right_option-JA.json` | (좌표는 §3.2) | 다언어 배분 | a·f | 높음 |

### 3.4 스페인어 (es)

| 파일 (카테고리) | title | 무엇을 하는가 | 패턴 | Karabiner 고유 의존 |
| :--- | :--- | :--- | :--- | :--- |
| `spanish_accents.json` (international) | Hold and tap for Spanish accents | 모음·n 더블탭 + backspace + `⌥`+모음 합성 → Á É Í Ó Ú Ñ | c | 높음 (변수·타이밍 상태기계) |
| `spanish_resurrect_dead_keys.json` (international) | …circumflex, tilde and backtick… on Spanish key layout | Spanish-ISO dead 키(ˆ ˜ \`) 뒤에 space 자동 첨가 → 단독 문자 | c | 중간 (input_source_if Spanish-ISO) |
| `hyperjis-accents-spanish.json` (international) | HyperJIS Accents: Spanish Extension | fn+n → ñ·fn+1 → ¡·fn+/ → ¿ (HyperJIS Core 확장) | c | 중간 |

> ⭐ **es 후보 전부가 패턴 (c) 악센트/dead keys 하나로 수렴한다** — 이것은 es 전용 문제가 아니라 **로마 문자 악센트 언어(fr·de·pt 등) 일반의 문제**이고, es 가 예시일 뿐이다. 계보 검증은 §4-4·§5-4.

---

## 4. 웹 검증 기록

> 각 주제: 확보한 증거(출처 URL + 원문 발췌 + 등급) → 판정 영향. `(추정)` 은 증거 부족 지점.

### 4-1. ko — 캡스락 한/영 · right command 한/영 · ₩→백틱

**캡스락 한/영 전환 — 문제 실재 ✅, "해법의 정본"은 카탈로그 토글 규칙이 아니라 "캡스락 → F17~F20 치환 + 시스템 단축키 연결"이다.**

| 출처 | 발췌 | 등급 |
| :--- | :--- | :--- |
| https://gist.github.com/bennlee/036e58d9f7a8e2f351e2a7cf6a92959b | "맥에서 `Caps Lock`을 한/영 전환키로 사용하면서부터 **딜레이가 생기거나**, **제대로 바뀌지 않는** 고질적인 문제가 꾸준히 발생해왔다" / "사용자들은 `karabiner-elements`등의 편법을 통해 `Caps Lock`키를 실제로 사용하지 않는 `F18` 등의 키로 매핑하여 해결해 왔다" | ① 실사용 |
| https://burn.eone.one/posts/2026/karabiner-elements-guide-2/ | "macOS 기본 한/영 전환(`Caps Lock`)은 눈에 띄는 딜레이가 있다. 빠르게 타이핑하다가 전환하면 글자가 씹히거나 밀리는 현상이 발생한다" / "macOS 입력기는 `F18`키를 한/영 전환 트리거로 인식한다" | ① 블로그 + ② 커뮤니티 룰 지칭 |
| https://mac.howso.kr/entry/%EB%A7%A5%EB%B6%81-macOS-%ED%95%9C%EC%98%81%ED%82%A4-%EC%9E%85%EB%A0%A5-%EC%A7%80%EC%97%B0-%EC%94%B9%ED%9E%98-%ED%95%B4%EA%B2%B0%EB%B0%A9%EB%B2%95 · https://change-words.tistory.com/entry/Mac-capslock-conversion-delay | "caps_lock 을 f19 키로 변경한 후 시스템 설정에서 다음 입력소스 선택의 단축키를 F19 로 변경" | ① 실사용 절차 |

**right command 한/영 — 검증 ✅** (윈도우 전환자의 우측 Alt/⌘ 위치 재현 수요):

| 출처 | 발췌 | 등급 |
| :--- | :--- | :--- |
| https://intwocave.com/318/ | "Mac의 키 레이아웃에서 우측 Alt 위치에 있는 키는 우측 Command 키인데, 나는 이 키에 한/영 기능을 바인딩하고자 하였다" — right_command → F18 + 시스템 단축키 | ① 실사용 |
| https://ratatou2.tistory.com/218 | "오른쪽 Command를 한영키로 바꾸고, Caps Lock 은 그저 대소문자에만 관여하게 할 것이다" / "여기까지하면 윈도우와 똑같은 키보드 환경 설정이 가능할 것이다!" | ① 실사용 |
| https://github.com/jaeyoi/hangeul-keymap-setup | "macOS에서 원하는 키를 한/영 전환 전용 키로 쓰기 위한 셸 스크립트" — 기본값 right command → F17, "수식키를 먼저 '단독으로 눌렀을 때 keydown 이 발생하는 일반 키'로 바꿔준 뒤 단축키로 등록하는 우회" | ① 구현 + 실측 |
| https://cho.sh/903D31 | "Press `Right Command` to set Mac's input method to Korean" — `select_input_source` 방식 JSON | ① 실사용 (단 아래 CJK 버그 주의) |

**₩→백틱 — 검증 ✅. F-16.4 와 동일 기능이라 "이미 다룸"이다.**

| 출처 | 발췌 | 등급 |
| :--- | :--- | :--- |
| https://burn.eone.one/posts/2026/karabiner-elements-guide-2/ | "한국어 입력 상태에서 백틱 키를 누르면 ₩(원화 기호)가 찍히는 문제는 Mac 한국어 사용자라면 누구나 겪는다" / "`input_source_if` 조건으로 한국어 입력 상태일 때만 동작하도록 제한" | ① 실사용 |

**⭐ CJK 입력 소스 직접 전환 버그 — 이 조사의 최대 발견.** `select_input_source` 로 한·중·일 입력기로 직접 전환하는 것은 macOS 버그로 실패한다 — 공식 문서의 경고다. 이 때문에 아래 §5 의 전환 프리셋은 전부 **`⌃Space` 재주입(F-16.1 과 같은 방식)** 을 쓰고 `select_input_source` 를 배제한다.

| 출처 | 발췌 | 등급 |
| :--- | :--- | :--- |
| https://karabiner-elements.pqrs.org/docs/json/complex-modifications-manipulator-definition/to/select-input-source/ | "Switching to input sources which have input_mode_id (Chinese, Japanese, Korean, Vietnamese) may be failed due to an macOS issue." / "For CJKV input sources, sending the input source switch shortcut (e.g., control-space) is better than using `select_input_source`." | 1차 공식 문서 |
| https://github.com/lawrence-lo/input-source-change | "Using `select_input_source` directly in Karabiner-Elements would sometimes fail on CJKV… it uses a deprecated macOS Carbon API that is not very reliable. The input icon changes but the actual input does not." | ① 실사용 |
| https://kage2kapp.org/inputswitcher-macos/ | "macOS 의 악명 높은 CJK 입력소스 전환 버그(전환해도 실제 타이핑은 이전 언어로 들어가는 문제)" / "TISSelectInputSource… macOS 26 에서도 여전히 존재" | ① 실사용 (macOS 26 실측) |
| https://www.v2ex.com/t/565667 | "select_input_source 基于 TISSelectInputSource, 切换多了会失效。输入法图标变了但是输入法不变" | ① 실사용 (zh 커뮤니티) |

### 4-2. ja — 英数/かな · ¥↔\ · JIS→US

**英数/かな 리매핑 — 검증 ✅✅ (가장 강한 증거).** JIS 키보드의 英数/かな 키 위치·US 배열 선호와 결합된 보편 문제다.

| 출처 | 발췌 | 등급 |
| :--- | :--- | :--- |
| https://qiita.com/sbtseiji/items/b06637dc4df34ced8464 | "caps lock キーを単独で押した場合に「ひらがな」モードと「U.S.」モードの切り替え" — japanese_kana/eisuu + input_source_if(ja) | ① 실사용 (JSON 공개) |
| https://qiita.com/Lil_Kurozukin/items/89206e7fcbd493e42850 | "英数かなキーみたいに「これをこうしたら必ずこうなれ！」と…Karabiner さんの力を借りることにしました" — US Magic Keyboard 의 caps lock → 英数 | ① 실사용 |
| https://qiita.com/ruemura3/items/dff3d8112e3da3ada41e | "USキーボードの日本語入力と英語入力の切り替えは、Macの場合デフォルトでは「Control + Space」という クソみたいな方法" | ① 실사용 |
| https://note.com/tigerwall/n/n5309474a2231 | "英語配列のキーボード…MacのCapsLockを使って英数とかなをトグル式で切り替えられるようにしたかった" — 카탈로그 "For Japanese (rev 6)" 사용 | ① 실사용 + 카탈로그 규칙 지칭 |
| https://qiita.com/k10i_/items/34f316eb1a5639b3e0ca | cmd 단독 탭 → 英数/かな (left_command → japanese_eisuu, right_command → japanese_kana) | ① 실사용 |
| https://karabiner-elements.pqrs.org/docs/json/complex-modifications-manipulator-definition/conditions/input-source/ | "Switching input source between Japanese and English at tapping the left command key" — 공식 예제 | 1차 공식 |

**¥↔backslash/백틱 — 검증 ✅, 단 "international3 함정" 을 반드시 기록해야 한다.**

| 출처 | 발췌 | 등급 |
| :--- | :--- | :--- |
| https://github.com/pqrs-org/KE-complex_modifications/pull/351 | "Please add the rule for also JIS keyboards. Differ from US keyboards, **¥** is mapped to the virtual key code 'international3' for JIS keyboard." | 1차 (카탈로그 PR) |
| https://github.com/pqrs-org/Karabiner-Elements/issues/982 | "I want to remap yen key (international 3) to backslash; however, when I map yen key to backslash, it prints close bracket ( ] )" — **international3 → \\ 매핑이 `]` 가 되는 함정**, jis_to_ascii.json 의 `international3 → option+international3` 수정 공유 | ① 실사용 (함정 실측) |
| https://github.com/pqrs-org/Karabiner-Elements/issues/1002 · #2819 · #3125 | international3 → backslash 가 JIS 에서 `]` 가 된다는 동일 증언 ("Keycodes are based on ANSI layout") | ① 실사용 |

**JIS → US 배열 — 검증 ✅✅.**

| 출처 | 발췌 | 등급 |
| :--- | :--- | :--- |
| https://japan-refactor.com/japanese-keyboard-layout/ | "Tired of fighting the JIS layout? … A commonly used configuration profile is titled 'For Japanese -> Use Japanese Keyboard as US Keyboard'" | ① 실사용 |
| https://www.keyboardista.com/en/articles/xekuaqcyel9zo37nfybo/ | "What settings do you recommend for making the Japanese keyboard look like an US keyboard? 1. For Japanese (JIS to ASCII) 2. Japanese JIS to US Keyboard: Remap Symbol Keys" — **카탈로그 규칙 2 개 직접 지칭** | ① 실사용 |
| https://github.com/crossly/mac-keyboard-map-jis-to-ansi | "Perfect for users who purchased a JIS Magic Keyboard but are accustomed to ANSI layout" | ① 구현 |

### 4-3. zh — 캡스락 중/영 · IME 전환 · 특정 IME

| 출처 | 발췌 | 등급 |
| :--- | :--- | :--- |
| https://blog.zhheo.com/p/5e08f6be.html | 카탈로그 규칙 **"Toggle Chinese English With caps_lock"** 임포트 절차 소개 — 목적: "使用中英文切换键切换 Mac 上的 ABC 输入法和搜狗输入法等第三方输入法" (**제3자 입력기에서 내장 캡스락 전환이 안 되기 때문**) | ① 실사용 + 규칙 지칭 |
| https://networm.me/2021/06/06/switch-ime-efficiently/ | "在 macOS 上搜索 Chinese 后导入 `Toggle Chinese English With caps_lock`" — JSON 원문 공개 | ① 실사용 |
| https://www.onejar99.com/mac-keyboard-en-zh-switch-by-karabiner-elements/ | "快捷鍵我習慣用 Caps Lock 鍵 (或稱「中/英」鍵)" | ① 실사용 |
| https://www.v2ex.com/t/565667 | "短按左 Command ⌘ → 切换到英文输入法, 短按右 Command ⌘ → 切换到中文输入法" + "切换多了会失效" (CJK 버그) | ① 실사용 |
| https://support.apple.com/zh-cn/guide/chinese-input-method/cim119a8d473/mac | 공식: 중국어 입력 소스에 "打开「使用大写锁定键切换」" — **Apple 중국어 입력기는 캡스락 중/영 전환이 네이티브** | 1차 공식 |

> ⭐ **기준 2(네이티브 대체)의 핵심 증거**: Apple 중국어 입력기에는 캡스락 중/영 토글이 **네이티브 내장**이다. 즉 "캡스락 → 중/영" 프리셋은 **Apple 입력기에는 불필요**하고, **제3자 입력기(搜狗 등) 대상으로만** 유효하다 — zh 커뮤니티 블로그의 동기가 정확히 그것이다(위 zhheo 발췌). `(실기기 확인 — 아래 §6 미해결 질문)`

**특정 IME(Doubao)** — `double_tap_right_command_switch_doubaoime.json` 은 DoubaoIME/Rime ID 를 하드코딩한 (g) 패턴이다. 범용성이 없어 canned preset 대상이 아니다. `(판정: 채택 안 함 — 증거 부족도 아닌, 성격상 부적합)`

### 4-4. es — dead key · 악센트

| 출처 | 발췌 | 등급 |
| :--- | :--- | :--- |
| https://discussions.apple.com/thread/254971869 | "I have a Spanish keyboard… Suddenly, these 2 dead keys were enabled and I can't write some vital passwords." | ① 실사용 (불편) |
| https://superuser.com/questions/1641710/ | "It's a feature of both the Spanish layouts… So you type the dead-key followed immediately by the letter… The only ways I know to eliminate them are… Ukelele" | ① 실사용 (해법이 KE 밖) |
| https://github.com/wezterm/wezterm/issues/300 | "With the Spanish ISO or the U.S. International keyboard layout pressing ´ + a should insert á. Instead a is inserted." | ① 실사용 (앱별 데드키) |
| https://otroespacioblog.wordpress.com/2025/05/03/teclado-en-espanol-con-idioma-ingles-o-teclado-ansi/ | "En Macos este idioma (US Intl. Alt Gr) no está disponible" → **커스텀 .keylayout 설치**, Karabiner 은 "mucho trabajo por hacer" 로 기각 | ① 실사용 (KE 회피 증언) |
| https://github.com/pqrs-org/Karabiner-Elements/issues/2186 | US 키보드에서 option+a → á — 사용자들이 `accents_with_option_key.json` 공유·다수 개선 댓글 "Right cmd + any vowel -> accented vowel" | ① 실사용 (소수지만 실제로 만듦) |
| https://github.com/smnrg/HyperJIS | hyperjis-accents-spanish.json — fn+n → ñ 등 | ① 구현 |

> ⭐ **es 판정의 핵심**: 불편 자체는 검증됐지만, **"KE 규칙으로 해결하는 수요" 의 증거가 압도적으로 약하다**. 스페인어 화자의 주류 해법은 Ukelele 커스텀 `.keylayout` 이고(위 otroespacio·superuser·Karabiner 이슈 #1205 "Create a keyboard layout according to your needs. That's the best solution"), 카탈로그의 `spanish_accents`(타이밍 해킹)·`spanish_resurrect_dead_keys` 의 직접 사용 증거는 수집되지 않았다. → **"추가 안 함" 판정**(§5-4).

### 4-5. 다언어 전환 (1키 = 1언어)

| 출처 | 발췌 | 등급 |
| :--- | :--- | :--- |
| https://github.com/lawrence-lo/input-source-change | F7/F8/F9 → en/zh/ja 직접 전환 — "Switching input sources among 3 languages… using ^Space or ^⌥Space has been a pain" / **앵커 select + ⌃⌥Space 전송으로 CJK 버그 우회한 원조** | ① 실사용 (trilingual) |
| https://www.v2ex.com/t/565667 · https://github.com/gxfxyz/e5f2ac1ce4f5053e9fb6608c10609837 | "短按左 Command ⌘ → 英文 / 短按右 Command ⌘ → 中文" | ① 실사용 |
| https://gist.github.com/sijiaoh/7793b439b7dcb1f7159d31031fffd561 | option+1/2/3/4 → en/zh-Hans/ja(히라가나)/ja(가타카나) | ① 실사용 |
| https://github.com/dongri/cmd-switch | "switch input sources… by pressing the left or right Command key alone" — **동일 관행의 독립 앱** | ① 실사용 |
| https://github.com/temeraire97/cmd-hanyoung | "Tap left ⌘ for English, right ⌘ for Korean — instant input-source switching… bypassing the CJKV bounce bug" | ① 실사용 |

> ⭐ **1키 = 1언어 배분은 ko·zh·ja 공통으로 검증된 관행이다.** 직접 할당은 대부분 좌/우 `⌘` 또는 F-키·숫자 키다 — **`⌥`(option) 단독 배분 사례는 수집되지 않았다** `(추정 — 카탈로그의 right_option→JA 할당을 뒷받침하는 웹 증거 부족)`. 다언어 전환은 모두 CJK 버그 때문에 "⌃Space 재주입" 또는 "앵커 + 단축키" 우회를 쓴다.

---

## 5. 채택 / 기각 권고 (언어별 최종 판정)

> 표기: **판정** — 채택(등급) / 기각(사유). 기각 사유는 "사용자 원칙: 굳이 설정할만한 패턴이 없는데 추가할 이유는 없다"(이슈 #113) 를 적용했다. 이 표는 `docs/spec/language-presets.md` §3 의 **정본**이다.

### 5-1. 한국어 ko — **F-16 참조 + 신규 2종 채택**

| 후보 | 판정 | 근거 (등급 · 출처) |
| :--- | :--- | :--- |
| F-16.1~F-16.4 (Shift+Space·한/영·한자·₩→`) | **참조** (신설 안 함) | korean-input.md 명세 확정. 카탈로그의 동일 패턴(`for_korean_keyboard`·`korean_won_to_backtick*`)은 흡수됨(§3.1) |
| `shortcut_preserved` 변형 | **기각 (이미 커버)** | F-16.4 의 modifier 부재 조건이 동일 가치(§3.1) |
| **캡스락 탭 → 한/영 전환** (caps_lock_toggle_korean_english_include_parallels 계열) | **채택 (①)** | 한국 커뮤니티 실사용 다수(§4-1). 단 정본은 "F-키 치환 + 시스템 단축키" 회로 — F-19 는 `⌃Space` 재주입으로 재현(§4-1 CJK 버그). ⚠️ macOS 한국어 입력기의 네이티브 캡스락 전환 옵션 여부는 실기기 미확인 → §6 |
| **오른쪽 command → 한/영 전환** (ChangeInputSourceDirectlyForKorean 계열) | **채택 (①)** | 윈도우 전환자 실사용(§4-1). 단 `select_input_source` 직접 전환은 CJK 버그로 기각 — F-16.1 의 `⌃Space` 재주입 방식을 쓴다 |
| left shift → 영어 (left_shift_change_english_input) | **기각 (③)** | macOS "캡스락으로 ABC 전환" 기능 위임 — Ultrakey 가 값을 더하는 부분이 없음(§3.1). macOS 네이티브와 동작이 겹침 |
| nowage_eng_char_on_kor, korean-japanese-english, print_screen… | **기각 (③~④)** | 타이밍 해킹 의존(답변 §3.1 의존도 높음) 또는 다언어 공통 문제(§5-5 로 이관) |

### 5-2. 일본어 ja — **4종 채택**

| 후보 | 판정 | 근거 (등급 · 출처) |
| :--- | :--- | :--- |
| **캡스락 → 英数/かな 토글** (japanese.json 방식) | **채택 (①)** | Qiita 다수·블로그·공식 예제(§4-2). JIS 사용자의 범용 문제 — F-16 의 한/영 키(`0x68`)와 같은 물리 키 계열 ⚠️ 중재 필요(§6) |
| **⌘ 단독 탭 → 英数/かな** (ei-kana-cmd·rdp-japanese-us 계열) | **채택 (①)** | Qiita k10i_·카탈로그 규칙(§4-2). US 키보드 + JIS 키보드 양쪽 배리언트 |
| **¥↔\ / ¥→백틱** (swap_yen*·jis_to_ascii 의 international3) | **채택 (①)** | PR #351·이슈 #982 등(§4-2). ⚠️ international3 함정(`]` 가 됨) 을 명세에 반드시 기록 |
| **JIS 키보드를 US 처럼** (jis_to_us_symbols 계열) | **채택 (①)** | 카탈로그 규칙 직접 지칭 블로그·crossly(§4-2). 18 심볼 치환 세트 — 캔디드(preset 후보) 1개로 그룹핑 |
| multi-layered-japanese (레이어·마우스) | 기각 (③) | JIS 파워유저 도구 — 언어 입력 문제가 아니라 일반 키맵 도구(§3.2). F-19 범위 밖 |
| kanafn/eisuufn (英数/かな → fn 레이어) | 기각 (③) | fn 레이어는 F-08 검토 대상이지 언어 프리셋 대상이 아님(§3.2) |

### 5-3. 중국어 zh — **1종 채택 + 다언어 공통 후보 이관**

| 후보 | 판정 | 근거 (등급 · 출처) |
| :--- | :--- | :--- |
| **캡스락 → 중/영 전환** (caps_lock_toggle_chinese_english 계열) | **채택 (①) — 단 제3자 입력기 대상 한정** | zh 커뮤니티 실사용(§4-3). Apple 중국어 입력기는 캡스락 전환 **네이티브 내장**(Apple 공식) — 프리셋 가치는 제3자 입력기(搜狗 등)에서만 발휘. `⌃Space` 재주입 구현. ⚠️ 실기기 확인(§6) |
| 좌⌘=en·우⌘=zh (다언어 배분) | **→ §5-5 공통 프리셋 후보로 이관** | 1키=1언어 관행(§4-5). zh 단독이 아니라 ko·ja 까지 걸치는 횡단 문제 |
| left_option → zh (left_option_to_chinese) | 기각 (③) | `select_input_source` + option 단독 배분 사례의 웹 증거 부족(§4-5 `(추정)`) |
| Doubao IME 전환 | **기각 (성격상 부적합)** | IME ID 하드코딩 (g) — 범용 canned preset 대상이 아님(§3.3) |
| windows_style_IME_switcher | 기각 (③) | "Windows 스타일 단축키 재현"은 언어 프리셋이 아니라 별개 관심사(§3.3) |

### 5-4. 스페인어 es — **추가 안 함** ⭐

| 후보 | 판정 | 근거 |
| :--- | :--- | :--- |
| spanish_accents | **추가 안 함** | ①~② 미달 — 더블탭+backspace 타이밍 해킹이고, 사용도 증거 부족(§4-4). 주류 해법은 Ukelele 커스텀 `.keylayout` |
| spanish_resurrect_dead_keys | **추가 안 함** | 불편 자체는 검증(①: Apple 커뮤니티 등) 이나 규칙 사용 증거 ③ 이하. "Spanish-ISO 레이아웃 전용" 으로 사용자 풀이 좁음 |
| hyperjis-accents-spanish | **추가 안 함** | 단독 프리셋으로 성립할 만한 사용도를 뒷받침할 증거 없음(③) — HyperJIS Core 전제(망가질 것 같은 선행 레이아웃 구조) |

**근거 정리**: "macOS 스페인어 키보드의 dead keys 불편"은 실재(①) 하지만, 그 해결의 **주류 경로가 Karabiner 가 아니다**(Ukelele·`.keylayout`·US Intl AltGr). 카탈로그에 es 규칙이 3 개뿐이라는 사실(§3.4)과, 그중 2 개가 타이밍 해킹이거나 특정 레이아웃 한정이라는 점(§3.4)이 이를 뒷받침한다. **사용자 원칙("굳이 설정할만한 패턴이 없는데 추가할 이유는 없다")에 따라 추가하지 않는다.** 재검토 여지는 명세 §9 에 남긴다.

### 5-5. 다언어 공통 — **후보로만, 채택은 명세 초안에서 열어 둠**

| 후보 | 판정 | 근거 |
| :--- | :--- | :--- |
| **1키 = 1언어 배분** (좌⌘=en·우⌘=zh·우⌥=ja 류) | **후보 (①)** — 채택 여부는 명세 초안의 미해결 질문으로 승계 | 검증된 관행(§4-5) 이나 ⓵ 범위(ko·ja·zh)와 en(기준선 제외) 의 관계, ⓶ `select_input_source` 배제 시 N-언어 직접 전환의 구현(⌃Space 는 2-소스 토글 전용) ⓷ option 단독 사례 부족 — 셋 다 열린 문제 |

---

## 6. 미해결 질문 (명세 §9 로 승계)

| # | 질문 | 왜 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | **macOS 한국어 입력기에 "캡스락으로 입력 소스 전환" 네이티브 옵션이 있는가** — 있으면 ko 캡스락 프리셋은 기준 2 에서 개별 재심 | 중국어 입력기에는 공식 문서로 존재 확인(§4-3). 한국어 입력기 옵션은 실기기 미확인 | 실기기 시스템 설정 ▸ 키보드 ▸ 입력 소스 편집 화면에서 한국어 소스의 옵션 확인 |
| 2 | **Apple 중국어 입력기의 캡스락 네이티브 토글 실동작** — 프리셋 채택 시 제3자 입력기 한정 범위의 실체적 확인 | 공식 문서 존재, 실동작은 미확인 | Apple 입력기 vs 제3자 입력기(搜狗 등) 각각에서 캡스락 탭 실측 |
| 3 | 106키 한/영·한자 키 실기기 미검증 (F-16 §9 #1 승계) | 한국어 106키 키보드 부재 | F-16 §3.2 "해소 이전의 기록" 절차 |
| 4 | 다언어(3+ 소스)에서 `⌃Space` 재주입으로 특정 언어 직접 전환이 되는가 | `⌃Space` 는 "이전 입력 소스" 토글 — 3 소스 이상에서 순환 의미가 달라진다 | 3 입력 소스 환경 실측 |
| 5 | es "추가 안 함" 판정의 재검토 시점 — 규칙 사용 증거가 쌓이는가 | 현재 ③ 이하 | KE 이슈·커뮤니티 모니터링 |

---

## 7. 출처 요약

- 카탈로그 데이터: `https://github.com/pqrs-org/KE-complex_modifications` (`main` 브랜치, `public/groups.json` · `public/json/` · `dist.json` 라이브 산출물 `https://ke-complex-modifications.pqrs.org/dist.json` · 사이트 번들 `https://ke-complex-modifications.pqrs.org/assets/index-DAI38KQG.js`), 조회 2026-09-03
- 규칙 파일 원문: `https://raw.githubusercontent.com/pqrs-org/KE-complex_modifications/main/public/json/<파일명>`
- Karabiner 공식 문서: `https://karabiner-elements.pqrs.org/docs/json/complex-modifications-manipulator-definition/to/select-input-source/` · `…/conditions/input-source/`
- 웹 검증 출처: §4 각 행 참조 (접근일 2026-09-03)