# 이슈 #93 — Seek 다국어 검색 (영어 + 선택 언어 ko·zh·ja·es 1개, IME/문자 입력 포함) — 설계 초안 + 작업 분해

> **성격**: `docs/plan/` 은 구현 위임 전 **계획 문서**를 두는 자리다. 이 문서는 중급(계획 초안)이 작성한 **초안**이며, 확정은 상급(호출 세션)의 리뷰 후 이루어진다. 리뷰가 끝나면 "결정 목록(D1~D8)"이 정본이 되고, §8 의 명세 갱신 초안이 `docs/spec/` 에 반영된 뒤 구현이 위임된다.
>
> **범위 제약**: Rust 쪽 한글 자모 결합 엔진(Issue #76 의 기각 대안 B' 계열)은 이 문서의 **범위 밖**(기각·후순위로만 언급한다). "영어 + 1개 언어" 이외의 다중 언어(2개+) 구성, OCR `recognitionLanguages` 의 시스템 로케일 무관 설치 언어 자동 감지도 범위 밖이다.

---

## 1. 요약

이슈 #76 에서 "한/영 키 전환은 복구했으나 **한글 조합 입력(IME) 자체는 범위 밖**으로 남겼다"(#76 D2). 이번 이슈는 그 남은 반쪽 — **한국어 뿐 아니라 중국어·일본어·스페인어까지, IME/문자 입력으로 검색어를 입력하고** 해당 언어를 OCR 로 검출하는 **다국어 검색** — 을 연다.

**결정 요지**:

- **설정 모델(D1)**: `Seek` 탭에 `검색 언어`(영어 + 선택 언어 1개) 팝업을 추가한다. 저장 키 `seek.searchLanguage`, 기본값 = 영어 단일(부재). 선택지: `English only`(기본)·`English + 한국어`·`English + 中文`·`English + 日本語`·`English + Español`.
- **이벤트 루프 처리 방식(D2)**: ⭐ **사용자 제안(인풋 박스 + debounce)을 채택하되, "영어 단일 설정일 때는 현행 동작 그대로"로 범위를 한정한다.**
  - **영어 단일(기본)**: 현행 유지 — 글자 즉시 처리, `canBecomeKey = false`, `<span>` 검색어 표시, 포커스 무변화. **회귀 제로.**
  - **다국어 선택 시**: 오버레이 검색 바를 **실제 `<input>`** 으로 전환해 macOS IME 가 조합을 수행하게 하고, **입력 멈춤 100ms 후(debounce)** 조합된 값을 검색어로 반영한다.
- **canBecomeKey(D3)**: 다국어 세션 중에만 검색 바 창을 `NON_ACTIVATING` 등록부에서 **일시 해제**해 키 윈도우가 될 수 있게 하고, 세션 종료 시 다시 등록한다. 영어 단일 세션은 전혀 건드리지 않는다.
- **세션 중 키 라우팅(D4)**: 다국어 세션에서 **`Text`/`Backspace` 로 분류되는 키만 원본 그대로 통과**시키고(키 윈도우인 검색 바 입력이 받는다), 제어 키(Enter·Esc·↑↓·Tab·`;`)·⌘⌃⌥ 조합은 기존대로 소비해 세션 컨트롤을 유지한다.
- **debounce 반영 경로(D5)**: 웹뷰 `<input>` 의 `input` 이벤트를 100ms 디바운스해 Rust 로 보내고, 상태 머신이 그 값을 쿼리로 반영한다(F-02 검출 재실행은 없다 — 이미 세션 시작에 캡처된 후보를 **필터링**만 갱신).
- **OCR 언어(D6)**: `seek.searchLanguage` 가 `recognitionLanguages` 를 결정한다(비영어 선택 시 `["<lang>-<region>", "en-US"]`, 영어 단일은 빈 목록 = Vision 기본). ⚠️ 실측 대가: 한국어 `["ko-KR","en-US"]` 는 전체 화면 OCR 775 ms → **약 3.1 s**(`seek-ocr-latency-spike.md` §3) — **기본을 영어 단일로 유지하는 근거**이기도 하다.
- **포커스 복원(D7)**: 세션 열릴 때 이전 최전면 앱의 pid 를 기억하고, **클릭 없이 닫혔을 때(Cancelled 등)만** 이전 앱을 재활성화한다. 클릭 확정 경로는 F-04 가 이미 대상 앱을 활성화하므로 복원하지 않는다.
- **명세·문서 갱신(D8)**: `seek-overlay-ui.md`(§1 범위 밖 문구 정정) · `seek-activation-and-session.md`(상태 머신 · 수용 기준) · `seek-text-detection.md`(F-02 `recognitionLanguages` 결정) · `korean-input.md`(#76 D2 "후속 작업"과의 연결) · `preferences-ui.md`(Seek 탭 설정 항목).

**관찰한 사실(코드 실측)**: ① 한국어 입력기가 활성인 동안 `current_layout()` 은 `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 로 **US 폴백 레이아웃으로 교체**한다(`text_input_source.rs:100-111`) → `resolve_typed_char`(`seek.rs:411`)가 **자모를 낼 수 없다** — 검색어에 한글이 오는 경로가 현재는 원리적으로 없다(#76 D2 의 "자모가 안 나온다"를 코드로 재확인). ② `overlay-searchbar.html:90-96` 주석이 `canBecomeKey=false` 라 `<input>` 을 둬도 타이핑이 씹힌다고 못박았다. ③ 검색 바 창은 `overlay.rs` 가 `force_non_activating()` 으로 `NON_ACTIVATING` 등록부에 넣고, 스위즐된 `canBecomeKeyWindow` 가 **등록부에 있는 창에 대해서만 NO** 를 반환한다(`overlay_window.rs:241-257`) — 즉 **런타임에 등록부에서 빼고 넣는 것으로 키 윈도우 가능/불가를 토글할 수 있는 구조**다. ④ 계층 1 게이트(`arbitration.rs:340-360`)는 세션 중 모든 키를 Consume → `SeekKey` 로 라우팅하고 마우스·한/영 키만 통과시킨다 — 여기에 다국어 통과 분기를 추가한다.

---

## 2. 사실 확인 (조사 근거)

| # | 사실 | 근거 (파일·행, 등급) |
| :--- | :--- | :--- |
| 1 | 검색어 문자 해석은 `LayoutTable::char_for`(`ultrakey-layout`) → 폴백은 `ascii_fallback`(US 전용). 한국어 IME 활성 시 `used_ascii_fallback = true` 가 되고 **US 폴백 레이아웃의 표**가 게시된다 | `seek.rs:411-491` · `layout/lib.rs:206-210` · `platform/text_input_source.rs:100-111` (실측) |
| 2 | 한국어 입력기가 활성이면 검색어에는 **영/숫자만** 들어간다 — 한글 자모가 나오는 경로가 없다 | 위 + `docs/plan/issue-76-seek-searchbar-ime.md` D2 (실측 요약) |
| 3 | 검색 바는 `canBecomeKey = false` 이고 `<input>` 이 없음 — `<span id="query-text">` 에 Rust 가 밀어 넣는 쿼리를 그림 | `overlay-searchbar.html:90-96` (§3.2, 실측) |
| 4 | 검색 바 창은 `force_non_activating()` 로 `NON_ACTIVATING` 등록부에 등록되고, 스위즐된 `canBecomeKeyWindow` 는 **등록부의 창만 NO**, 그 밖은 원래 구현으로 넘김 | `overlay_window.rs:232-257, 310-353` · `overlay.rs:190-198` (실측) |
| 5 | 계층 1 게이트: 세션 중 마우스·한/영 키(`0x68`)만 통과, 나머지 키는 전부 Consume → `Effect::SeekKey` | `arbitration.rs:340-360` (실측) |
| 6 | 쿼리 필터링은 세션 시작에 캡처된 후보에 대해 매 입력마다 즉시 적용됨(`OverlaySession::set_query` → `recompute_matching`) — **재검출은 없음** | `ultrakey-seek-session/machine.rs:349-379` · `ultrakey-overlay` (실측) |
| 7 | OCR `recognitionLanguages` 는 현재 UI 로케일(`general.language`)에서 계산됨 — `["ko-KR","en-US"]`(한국어 UI) 등 | `main.rs:4676-4690` · `i18n/lib.rs:114-132` (이슈 #48, 실측) |
| 8 | `["ko-KR","en-US"]` 로 전체 화면 OCR 을 돌리면 **약 3.1 s** (775 ms 의 3~4배), 신뢰도 0.83→0.59. 첫 원소가 인식 언어를 가른다 | `docs/dev/seek-ocr-latency-spike.md` §3 · `seek-text-detection.md` §3.2.2 (실측) |
| 9 | 세션 중 제어 키 분류 — Enter/Esc/↑↓/Tab/⇧Tab/`;`/⌘⌃⌥ 조합 ⇒ `Ignore` | `ultrakey-seek-session/keys.rs:44-114` (실측) |
| 10 | `NsWindowHandle`·`NSRunningApplication.activateWithOptions`(주 전면 앱 재활성화용) 는 이미 `objc2-app-kit` 바인딩으로 존재 | `platform/overlay_window.rs` · `click_executor.rs:388-409` (실측) |
| 11 | 설정 UI 는 `data-key` + `change` 위임 → `settings_set` Tauri 커맨드로 저장·재렌더 | `settings.html:3052-3077` (실측) |
| 12 | 스위즐은 **프로세스당 한 번**(`OnceLock`) — 등록부만 토글하면 같은 스위즐을 재실행하지 않아도 된다 | `overlay_window.rs:269-308` (실측) |

---

## 3. 결정 목록 D1~D8

### D1 — 설정 모델: **`Seek` 탭 `검색 언어` 팝업, 저장 키 `seek.searchLanguage`, 기본 = 영어 단일**

**결정**: `Seek` 탭에 `검색 언어` 항목을 둔다. 컨트롤은 **팝업(select)** — "영어 + 1개 언어"라는 이슈 본문의 구성 요구를 옵션 5개로 직접 표현하는 것이 가장 단순하다.

| 값 | 라벨(ko) | 라벨(en) | OCR 언어 | `input_box_mode` |
| :--- | :--- | :--- | :--- | :--- |
| **부재**(키 없음) | (로케일 파생 행이 선택된 것으로 표시) | — | **기존 동작 유지** — `general.language` 로케일에서 계산(D6 폴백, 이슈 #48) | `false` |
| `"en"`(명시) | `영어만` | `English only` | `[]` (Vision 기본, 실측) — 로케일을 명시적으로 이김 | `false` |
| `"ko"` | `영어 + 한국어` | `English + 한국어` | `["ko-KR", "en-US"]` | `true` |
| `"zh"` | `영어 + 中文` | `English + 中文` | `["zh-Hans", "en-US"]` | `true` |
| `"ja"` | `영어 + 日本語` | `English + 日本語` | `["ja-JP", "en-US"]` | `true` |
| `"es"` | `영어 + Español` | `English + Español` | `["es-ES", "en-US"]` | `true` |

> ⚠️ **상급 리뷰 반영(#1·#2)** — 위 표가 D1~D6 의 **최종 정본**이다. 초안 원판이 "빈 값과 `"en"` 을 같은 상태로" 두던 것은 한국어 UI 사용자에 대해 모순이었고, 부재 = 로케일 폴백, `"en"` = 명시 오버라이드로 분리했다(§9 #1~#2). 부재 시 팝업 표시 계약도 §9 #2 참고.

- **저장 키**: `seek.searchLanguage`(부재 = 기본값 영어 단일, F-15 §3.1). 값은 `Option<String>` — `None`/빈 문자열 = 영어 단일. `settings_store` 커맨드의 문자열 유형으로 저장한다.
- **표현**: `SeekSettings` 에 `search_language: Option<String>` 필드 추가. `to_config` 로 `SeekConfig` 에 넘길 때는 다국어 여부(부울)와 OCR 언어를 파생하는 **두 개의 값**으로 옮긴다 — `input_box_mode: bool`(세션 키 라우팅·UI 전환 판정)과, 세션 열 때 검출 스레드에 넘길 언어 목록. 언어 코드 → OCR 태그 매핑은 `ultrakey-i18n::Locale::ocr_recognition_languages()` 를 **재사용**한다(값 `"ko"` ↔ `Locale::Ko` — 이슈 #48 이 만든 단일 매핑을 복제하지 않는다).
- **기본값 어긋남 방지**: 부재 = 영어 단일. `unwrap_or_default()` 로 처리하면 `"ko"` 가 저장되기 전엔 전부 영어 단일로 읽힌다 — 기존 동작과 정확히 같다.
- **UI 위치**: `Seek` 탭 설정 표의 기존 항목들 사이(F-01 설정 뒤)에 둔다. `Semicolon highlights next match`(저장 키 `seek.semicolonCycle`) 아래가 자연스럽다.

**기각한 대안**:

| 대안 | 기각 사유 |
| :--- | :--- |
| ① 저장 키를 배열(`seek.searchLanguages = ["ko"]`)로 | 스칼라 하나면 충분한데 배열은 직렬화·검증·부재 의미가 복잡해진다. "영어 + 1개" 구성은 스칼라 옵션 5개가 정확히 표현한다. |
| ② `General` 탭 로케일과 통합 | 로케일은 **UI 언어**이고 검색 언어는 **OCR·입력 대상**이다 — F-14 §3.1 의 "로케일 결정과 입력 소스는 서로 독립된 축".한국어 UI 를 쓰면서 영문만 검색하거나(기본), 영어 UI 를 쓰면서 한국어를 검색하는 구성이 모두 자연스럽다. |
| ③ OCR 언어 목록을 `recognitionLanguages` UI 로 직접 편집 | 이슈 제품 제약 §4 "사전 정의된 체크박스 프리셋만 제공하는 것이 의도된 설계"와 어긋남. 5단계 옵션이 요구를 정확히 커버한다. |

### D2 — 이벤트 루프 처리 방식: **사용자 제안(인풋 박스 + debounce) 채택. 단 "다국어 선택 시에만" 인풋 박스로 전환, 영어 단일은 현행 유지**

**결정**: 이슈 본문의 제안 — *"영어만 선택 → 현행 동작(즉시 문자). 영어+한국어 선택 → 입력창을 인풋 박스처럼 동작하게 하고, 일정 시간(예: 100ms) 입력이 없으면 입력된 값 기준으로 OCR 탐색"* — 을 그 구조 그대로 채택한다.

| 설정 | 검색어 입력 방식 |
| :--- | :--- |
| 영어 단일(기본) | 현행 그대로 — Rust(계층 1)가 `resolve_typed_char` 로 문자를 해석해 쿼리로 밀어 넣음. `canBecomeKey = false` 유지, 포커스 무변화, `<span>` 표시 |
| 다국어 선택 | 검색 바를 실제 `<input>` 으로 표시하고, macOS IME 가 조합 → 조합 완료·입력 중단(100ms) → Rust 로 쿼리 반영 |

**근거 (이 세 가지가 이 결정의 전부다)**:

1. ⭐ **Rust 쪽 자모 결합 엔진(이슈 옵션 d)은 네 언어 전부를 커버할 수 없다.** 한국어는 자모→음절 결합이 **알고리즘적**(초성 19·중성 21·종성 28 → 11172 음절, 표준 공식)이라 구현 가능하지만, 중국어(병음→한자)·일본어(가나·문자 변환)는 **문장 단위 후보 선택이 필요한 IME** 라 Rust 가 재현할 수 없다. 스페인어는 악센트(á/é/ñ)가 `⌥+키` 조합이라 결합 엔진 없이는 "두 글자 분리 입력"이 된다. 결합 엔진 하나로 네 언어가 모두 안 되는 판정을 **하나의 일관된 메커니즘**(실제 IME)으로 대체하는 것이 이 결정의 핵심이다.
2. ⭐ **`canBecomeKey = false` 와의 충돌은 "다국어 세션 중에 한해" 부분 해제로 풀 수 있다.** 기존 제약의 목적은 "**오버레이가 열렸다고 하위 앱 포커스를 뺏지 않는다**"이다. 다국어를 명시적으로 선택한 사용자는 그 대가(임시 포커스 이동)를 받아들인 구성이고, 세션은 되돌릴 수 있다(§D3·D7). **영어 단일이면(기본) 제약이 그대로 지켜진다.** 이것은 "상호 배타적"이 아니라 "설정에 따른 분기"다.
3. **재검출이 아니라 필터링에 debounce 를 적용한다.** `recognitionLanguages` 변경으로 비영어 후보를 뽑는 것은 **세션 시작 검출** 때 확정된다(이슈 #48 — 언어 목록을 세션 열기마다 읽어 넘긴다). 인풋 박스에서 조합된 검색어는 이미 있는 후보를 **필터링**하는 쿼리가 될 뿐이라, debounce 후 Rust 로 보내 `set_query` 만 갱신하면 된다 — 이슈 제안 문구의 "OCR 탐색"은 사실 "새 쿼리로 필터"이며 구현이 단순해진다.

**기각한 대안** — 자세한 표는 §4.

### D3 — canBecomeKey: **다국어 세션에 한해 검색 바 창을 `NON_ACTIVATING` 등록부에서 일시 해제 + 키 윈도우로 승격, 세션 종료 시 재등록**

**결정**: `ultrakey-platform::overlay_window` 에 `NsWindowHandle::set_can_become_key(bool)`(또는 `register_non_activating(bool)`)를 추가한다 — 기존 스위즐(프로세스당 1회, `OnceLock`)은 그대로 두고 **등록부(`NON_ACTIVATING: RwLock<Vec<usize>>`)에서 추가/제거**만 한다(실측 ④⑫).

- **다국어 세션 열림**: 검색 바 창을 등록부에서 제거(`canBecomeKeyWindow` → 원래 구현 YES) → `NSApp.activate` + `makeKeyAndOrderFront` → 웹뷰 `<input>` 에 `focus()`(JS). 이전 **최전면 앱 pid 를 기억**(F-01 활성화 스냅샷에 포함).
- **세션 닫힘(모든 경로)**: `<input>` 값 초기화 → 등록부에 재추가(다시 NON-activating) → `orderOut`. **클릭 확정 여부에 따라** 이전 앱 재활성화/미복원(§D7).
- ⚠️ **이 토글은 메인 스레드에서만**(`NsWindowHandle` 의 기존 계약) — 창 조작은 전부 `run_on_main_thread` 디스패치.

**근거**: 등록부 토글이 동작 방식은 이미 코드로 확정돼 있다(실측 ④). 새 스위즐·새 서브클래싱이 없다 — `object_setClass` 는 실기기 하드 크래시로 반증된 경로(korea-input 계열 조사든 여기든), 이 구현은 그 위험을 다시 걸지 않는다.

**기각한 대안**:

| 대안 | 기각 사유 |
| :--- | :--- |
| ① 전체 세션 내내 검색 바 창을 키 윈도우로 유지 | 영어 단일 세션까지 포커스를 뺏는 회귀가 생긴다 — 기본 동작(현행) 보존 원칙 위배. |
| ② 오버레이 창 전체(하이라이트 포함)를 키 윈도우로 승격 | 검색 바 창 하나만 입력을 받으면 된다. 하이라이트 창은 클릭 통과라 키 윈도우가 되어도 의미가 없다. |
| ③ 부모가 아닌 별도 입력 창을 띄움 | 하이라이트·검색 바·입력 창이 복합으로 늘어나 구조가 커진다. 검색 바 창 자체가 입력을 받는 편이 창 수가 줄고 `persistPosition`(위치 저장)과도 정합한다. |

### D4 — 세션 중 키 라우팅: **다국어 세션에서 `Text`/`Backspace` 분류 키만 원본 통과, 나머지는 현행대로 소비**

**결정**: 계층 1 게이트(`arbitration.rs`)에 다국어(`input_box_mode`) 분기를 추가한다.

```
if gates.seek_active {
    마우스 → Pass (기존)
    한/영 키(0x68) → Pass (기존, 이슈 #76)
    if input_box_mode && is_input_box_pass_key(ev) → Pass   // ⭐ 신규
    else → Consume + Effect::SeekKey                          // 기존
}
```

`is_input_box_pass_key(ev, semicolon_cycles)` 는 **현행 `classify`(세션 키 분류, `keys.rs:44-114`)가 `Text` 또는 `Backspace` 로 볼 키**와 같은 집합을 물리 keycode+flags로 판정한다 — `;` 는 `semicolonCycle` 설정이 켜져 있으면 소비(순환), 꺼져 있으면 통과(`keys.rs:86`):

- `Delete`(backspace) → Pass (입력 요소가 IME 조합 단계를 포함해 올바르게 지운다).
- 문자·숫자·공백·기호 keycode 로서 **⌘/⌃ modifier 가 안 실려 있는 것** → Pass (웹뷰 `<input>` 이 실제 IME 조합으로 처리). ⭐ **상급 리뷰 반영(#3, 최종 확정): `⌥` 는 차단하지 않는다** — `⌥+문자`(US 배열 데드키 → 스페인어 악센트)도 통과. 단 **설정된 전역 단축키 조합은 예외로 Consume**(토글 닫기 조합이 검색어로 새지 않게).
- Enter·Esc·↑↓·Tab·`;`(순환 켬)·⌘⌃ 조합·modifier 키·한자 키(`0x66`) → **기존대로 Consume** (세션 컨트롤 유지 — Enter 확정, Esc 취소, 순환이 웹뷰에 빼앗기지 않는다).
- `Shift`(대문자) 는 대문자 입력에 필요하므로 필터링 대상이 아니다(기존 `keys.rs` 와 같은 규칙).
- ⭐ **통과 키의 KeyUp·FlagsChanged 도 통과**로 정의한다(반대면 입력 필드가 down-only 상태가 되고, 시프트·데드키 상태가 깨진다).

**근거**:
- 입력 **역할 분담**이 명확해진다 — 글자 입력은 웹뷰(IME 가 조합), 세션 제어는 Rust(상태 머신). 두 쪽이 같은 키를 두고 경합하지 않도록 "어느 쪽 소관인가"를 계층 1 이 물리 keycode 로 결정한다.
- **범위가 영어 단일 세션에 닿지 않는다** — `input_box_mode` 가 false 인 동안 기존 분기와 코드가 1비트도 바뀌지 않는다(회귀 제로).
- ⌘⌃⌥ 조합은 절대 통과시키지 않아 **Ultrakey 프로세스 단축키(⌘Q·⌘W 등)가 다국어 세션 중에도 눌리지 않게** 막는 것이 그대로 유지된다.

**이슈 #76 과의 관계**: 한/영 키 통과 예외는 그대로 유지된다. 다만 다국어 세션(키 윈도우 승격)에서는 한/영 키가 **Ultrakey 앱의** 입력 소스를 전환한다 — 이는 의도된 동작이나, 입력 소스가 원래 앱과 다르게 남는 경계는 §7 리스크로 기록한다.

### D5 — debounce: **100ms, 웹뷰 → Rust 쿼리 반영 경로. F-02 재검출 없음**

**결정**: 다국어 세션에서 `<input>` 의 `input` 이벤트를 프런트가 100ms 디바운스해 `invoke("seek_set_query", { query })` 로 Rust(워커)에 보낸다. 워커는 새 `SeekSignal::SetQuery(String)` 로 상태 머신의 새 `SeekSessionMachine::set_query_external(query)` 를 부르고, 그 결과 쿼리에 따라 후보를 필터링·재렌더한다.

- **100ms** 는 이슈 본문이 제안한 값이자 일반 UX 관행(체감 지연 100ms 상한, `seek-overlay-ui.md` §3.6)과 일치한다. ⭐ **상급 리뷰 반영(#5)**: `compositionend` 시 **즉시 전송**한다 — 디바운스는 **비조합(직접) 입력**의 idle 상한으로만 작동한다. 이렇게 하면 한글 음절이 굳는 즉시 쿼리가 도착하고, "마지막 입력 후 Enter 를 곧바로 눌러 직전 필터로 확정되는" 경쟁(§7 리스크 9)이 조합 입력에서는 사라진다.
- 검출 스레드는 세션 열림 때 이미 언어 목록을 받아 한 번 돈다 — **쿼리 반영은 재검출·재캡처를 유발하지 않는다**(실측 ⑥). 이는 이슈 제안 문구의 "OCR 탐색"을 단순화한 해석이며 §2 사실 6·8 에 근거한다.
- **쿼리 소유권**: 다국어 세션에서는 `<input>` 이 소스 오브트루스다. Rust → 웹뷰 렌더(`overlay://searchbar`)는 검색어 텍스트를 `<input>` 에 **되쓰지 않는다**(초점·커서 위치를 건드리지 않게) — 대신 함수 입력 모드 플래그(`input_mode: true`)를 실어 보내고, JS 가 렌더 시 `<input>` 은 값을 건드리지 않고 그대로 둔다. 영어 단일은 종전처럼 `<span>` 에 반영.
- **조합 중(compositionstart)**: 조합 중에는 쿼리를 보내지 않는다(`compositionend` 후의 최신 값만 전송). 이렇게 해야 자음 `ㄱㄴㄷ` 이 음절로 굳기 전에 중간 상태가 검색어에 새지 않는다. ⭐ 파생 엣지(#5): 다국어 세션에서 **Enter 로 한글 조합 확정이 불가**하다(계층 1 이 Enter 를 소비) — 조합은 Space·다음 문자로도 확정되며, 실제 동작은 M13 으로 확인한다.

**기각한 대안**:

| 대안 | 기각 사유 |
| :--- | :--- |
| ① 매 키 입력마다 즉시 전송 | IME 조합 중간 상태(자모)가 그대로 쿼리에 실려 사용자가 "ㄱ" 단계에서 매치 0 개를 보는 혼란. 디바운스가 없다면 키 반복·빠른 타이핑에서 IPC 도 폭주한다. |
| ② Rust 쪽에서 debounce 타이머를 관리 | 타이머를 관리할 워커 상태가 늘고, 웹뷰가 이미 디바운스를 하면 이중이다. 합성 결과를 아는 쪽(웹뷰)이 타이머를 갖는 것이 값이 가장 정확하다. |

### D6 — OCR `recognitionLanguages`: **설정이 결정, UI 로케일 자동 연결은 "설정이 비면"만 폴백**

**결정**: 세션 열 때 검출 스레드에 넘기는 언어 목록을 `seek.searchLanguage` 에서 계산한다.

- `searchLanguage` 가 비면 → **기존 동작 유지**(현행: `general.language` UI 로케일에서 계산 — 이슈 #48). 이것은 기존 사용자의 회귀를 막는 폴백이다.
- `searchLanguage` 가 지정되면 → `Locale` 매핑(`i18n/lib.rs:124`)으로 `["ko-KR","en-US"]` 등을 명시 사용 — UI 로케일과 무관.
- **스페인어·중국어·일본어의 실측 대가는 한국어와 같은 계열** [`<lang>-<region>` 첫 원소] 로 추정한다 `(추정 — 한국어만 실측, `seek-ocr-latency-spike.md` §3. 다른 비영어 언어의 지연은 미측정. §7 리스크 3)`.

**근거**: 검색 언어는 OCR 인식 언어의 **직접적인 지정**이다. 현재 이슈 #48 이 임시로 "UI 가 곧 OCR 언어"로 연결한 것은 **설정이 없었기 때문**이고, 이번에 검색 언어 설정이 생기면 그것이 정본이 된다. 다만 D1 의 폴백(비면 → 기존)으로 #48 의 동작을 여전히 살린다.

**기각한 대안**:

| 대안 | 기각 사유 |
| :--- | :--- |
| ① 다국어일 때 전 언어 전부(`["ko-KR","zh-Hans","ja-JP","es-ES","en-US"]`) | 인식 시간·신뢰도가 통제 불능으로 늘어난다. "영어 + 1개"라는 이슈 요구가 목록 폭발을 원천 봉쇄한다. |
| ② 시스템에 설치된 입력 소스 언어로 자동 추적 | 로케일 실측이 입력 소스 설치 상태에 의존해 결정적이지 않다(코드 실측 불가). 범위 밖으로 남긴다(이슈 본문 요구가 설정 선택형이므로). |

### D7 — 포커스 복원 + **키 상실 자동 닫힘**: **이전 최전면 앱 pid 를 기록, "클릭 없이 닫혔을 때만" 재활성화. 다국어 세션 중 검색 바가 키를 잃으면 자동 닫힘**

**결정**: 다국어 세션이 **열릴 때**(`snapshot_activation_context` 를 이미 매 세션 호출하므로 — `seek.rs:140-161`) 그 시점의 **최전면 앱의 pid** 를 함께 스냅샷에 담는다(메인 스레드 `NSWorkspace.shared.frontmostApplication.processIdentifier`).

- **클릭 없이 닫힘**(Cancelled·Toggled·ReleasedWithoutMatch·DisplaysChanged): 기억한 pid 를 `NSRunningApplication.activateWithOptions(ActivateIgnoringOtherApps)` 로 재활성화한다(`click_executor.rs:388-409` 가 이미 만든 `activate_application` 패턴 재사용).
- **클릭 확정(Confirmed)**: **재활성화하지 않는다** — F-04 가 이미 클릭 대상 앱을 활성화했다(`Focus window before clicking` 설정도 F-04 소관). 여기서 이전 앱을 다시 활성화하면 **클릭 직후 포커스가 이전 앱으로 되돌아가는** 오작동이다.
- ⭐ **상급 리뷰 반영(#11, 최종 확정 — (ii) 키 상실 자동 닫힘)**: 다국어 세션 중 검색 바 창이 키를 잃으면(`NSWindow.didResignKeyNotification` — 예: 사용자가 타 앱 클릭) **복원 없이 세션을 자동으로 닫는다**. 근거 — (i) 타 앱 클릭을 수용하면 세션 중 통과되는 문자 키가 **클릭된 앱에 그대로 새어** (암호 필드 오타 등) 누출 위험이 생기고, 이는 "세션 중 키는 하위 앱에 닿지 않는다"는 계층 1 의 안전 계약(§3.1)과 어긋난다. ⚠️ **Confirming 상태(클릭 확정 처리 중)에서는 닫지 않는다** — F-04 가 대상 앱을 활성화하며 낸 `resignKey` 를 취소로 오인하지 않도록.
- **영어 단일 세션**: 아무 변화가 없다(포커스가 애초에 안 움직였으므로). 키 상실 자동 닫힘도 다국어 세션 전용 — 영어 세션은 오늘처럼 키 전부가 소비되어 누출이 없어 자동 닫힘이 불필요하다.

**근거**: "포커스를 뺏는다"의 대가는 "되돌려 준다"로 상쇄되어야 한다. 확인 시점 구분(Confirmed vs 그 외)은 F-04 와의 충돌을 피하는 비대칭 처리로, F-04 §5 #10(확정 시 modifier 스냅샷)과 같은 "경로별 비대칭" 선례를 따른다.

### D8 — 명세·문서·UI 갱신 범위

`docs/plan/` 관례(issue-76 등)대로 명세 갱신은 구현 위에 얹는다. 갱신 파일과 내용:

| 파일 | 갱신 내용 |
| :--- | :--- |
| `docs/spec/README.md` | ⭐ **갈라짐 표 D10 행 신규**(§9 #9) — "Seek 다국어 검색 + IME 입력(검색 언어 설정)". 성격: **신규 기능**. 근거: 원본은 영어 단일(`seek-text-detection.md` §5 #10 — `Base.lproj`), 사용자 명시 요구(이슈 #93). 기준 1 통과 |
| `docs/spec/seek-overlay-ui.md` | §1 "한글 조합은 범위 밖"을 **정정** — "영어 단일은 범위 밖 유지, 검색 언어 설정이 비영어면 검색 바가 실제 `<input>` 이 되어 IME 조합을 수용(다국어 세션 한정 포커스 핸드오프)"로. §3.2 표의 `canBecomeKey=false` 요구에 "다국어 세션 예외" 주석. §7 (A)·`overlay_window.rs` 모듈 문서의 `makeKeyAndOrderFront` 금지 문구에 다국어 세션 예외 정정(§9 #10). 다국어 세션의 영문 타이핑이 100ms 갱신 단위라는 트레이드오프 엣지 기록 |
| `docs/spec/seek-activation-and-session.md` | §3.1 세션 중 키 라우팅 규칙에 다국어 `Text`/`Backspace` 통과 행 추가(⌘/⌃ 조합은 계속 소비, ⌥+문자는 통과) · §3.2 상태 머신 표에 다국어 키 행 + **키 상실(자동 닫힘) 행**(§9 #10) · §5 엣지에 키 상실·다국어 트레이드오프 기록 · §8 수용 기준 추가 |
| `docs/spec/seek-text-detection.md` | F-02 §3.2.2 `recognitionLanguages` 행 갱신 — "UI 로케일 자동 매핑(`(추정)` 상태)이 아니라 `seek.searchLanguage` 가 정본"으로, 결정 사슬 **S-4 → 이슈 #48 → 이슈 #93** 한 행(§9 #10), 실측 대가(3.1 s) 재확인 |
| `docs/spec/korean-input.md` | §9 또는 #76 상호 참조 — 이슈 #76 D2 의 "후속 작업"이 **이슈 #93 으로 해소**되었음을 한 줄 연결 |
| `docs/spec/preferences-ui.md` | Seek 탭 항목 표에 `검색 언어` 항목(`seek.searchLanguage`) 추가 |
| `docs/dev/manual-verification.md` | §6 의 M1~M13 수동 검증 항목 추가(한글 IME 조합·debounce·포커스 복원·키 상실 자동 닫힘·⌥ 데드키·`;` 순환 설정·타 앱 클릭) |

---

## 4. 기각한 대안 상세 (이슈 옵션 a·c·d 정리)

| # | 대안 | 판정 | 기각 사유 |
| :--- | :--- | :--- | :--- |
| (a) | **비영문 입력 즉시 처리(현행 영문만 그대로)** — 설정 자체를 두지 않음 | **부분 채택** | "영어 단일 = 현행"은 D2 의 기본 분기로 채택. 그러나 이슈 본문 요구("원하는 경우 특정 국가 언어로 탐색 가능해야 한다")를 전혀 만족하지 못해 **단독으로는 불충분**. |
| (b) | **오버레이를 실제 인풋 박스로 전환(IME 조합)** — 모든 세션에서 | **부분 채택 → 설정 분기** | 사용자 제안 요지. 다만 **전 세션 전환은 회귀**(영어 단일 = 현행 보존 원칙 위배)라 "다국어 선택 시에만"으로 한정. 이게 §2.2 의 정본이다. |
| (c) | **인풋 박스 + debounce(100ms)** | **채택 (D2·D5)** | 사용자 제안 그대로. "debounce 후 입력된 값으로 OCR 탐색"을 "재검출 없는 필터 갱신"으로 단순화(실측 ⑥). |
| (d) | **Rust 쪽 자모 조합 엔진** | **기각** | §D2 근거 1 — 한국어 외(중국어·일본어)는 IME 재생산이 사실상 불가. 한국어만 해당하는 반쪽 엔진으로 네 언어 요구를 못 푼다. + `used_ascii_fallback` 때문에 **원본 자모를 읽는 새 플랫폼 경로**(`current_layout` 폴백 교체 이전 `uchr_data`)부터 만들어야 하는 선결 작업. 대형·후순위. |
| (e) | **숨은 `NSTextInputContext` + `interpretKeyEvents:` 로 IME 만 떼어냄** | **기각** | 키 윈도우 없이 IME 를 부르는 방식. `NSTextInputClient` 의 marked-text 콜백·후보 창 배치(`firstRectForCharacterRange`)를 전부 손으로 재현해야 하고, 실제 자모 조합 동작이 macOS 버전별로 다를 위험 — 실기기 검증 난이도가 키 윈도우 방식보다 훨씬 높다. 채택 이득(포커스 유지)은 이미 "설정 분기"로 영어 단일에서 지켜진다. |
| (f) | **검색어에 유니코드 문자 직접 주입** (`CGEventKeyboardSetUnicodeString`) | **기각** | IME 조합 버퍼를 우회해 **조합 중인 글자를 깨뜨리고**(F-16.4 §3.1 이 이미 같은 이유로 기각), 하위 앱에 문자가 새어 나간다. |

---

## 5. 작업 분해 (파일 단위)

### 작업 1 — 설정 모델 + 저장 (crates)

- `crates/ultrakey-core/src/settings/keys.rs`: `SEEK_SEARCH_LANGUAGE: &str = "seek.searchLanguage"` 추가, `all()` 목록에 등록.
- `crates/ultrakey-seek-session/src/settings.rs`: `SeekSettings.search_language: Option<String>` (+ from_store/to_config, `input_box_mode` 파생).
- `crates/ultrakey-seek-session/src/config.rs`: `SeekConfig` 에 `input_box_mode: bool` 추가(기본 false — `Copy` 파생 유지).

### 작업 2 — 세션 키 라우팅 (arbitration)

- `crates/ultrakey-core/src/arbitration.rs` 계층 1 게이트(`340-360`): `input_box_mode` 이면서 `is_input_box_pass_key` 인 키는 `Outcome::pass(Layer::SeekSession)` — 분기 위치는 **마우스 통과·한/영 통과 뒤**(기존 두 예외와 겹치지 않게).
- `input_box_mode` 는 `EngineConfig`/게이트 스냅샷 어디로 들어가는지 결정 — 계층 1 은 `GateSnapshot` 을 받으므로, `seek_session_active` 를 게시하는 곳(워커)이 **같은 원자값 근처에 `seek_input_box` 게이트를 함께 게시**하는 패턴이 기존 구조와 가장 어울린다(`SharedState.seek_session_active` 주석 참조).
- `is_input_box_pass_key(ev)`: `keys.rs` 의 분류와 동일한 물리 keycode 집합을 `ultrakey-core` 에서 판정 — ⚠️ `SessionKey` 분류는 `ultrakey-seek-session` 크레이트에 있어 `ultrakey-core` 는 그걸 의존하지 않으므로 **core 쪽에 최소 판정 헬퍼**(`is_input_box_pass_key`)를 두고, 테스트로 `keys.rs` 분류 결과와 대조한다.

### 작업 3 — 검색 바 웹뷰 인풋 (UI)

- `apps/ultrakey-app/ui/overlay-searchbar.html`: `<span id="query-text">` 병렬로 `<input id="query-input" hidden>` 추가(옵션 접근성 라벨 포함). `render(payload)` 가 `payload.input_mode` 플래그를 보고 display 전환 + input/Debounce 전송(`compositionstart/end` 처리) + 커서·값 겹치기 방지. 검색어 소스는 다국어에서는 `<input>`, 영어에서는 `<span>`.

### 작업 4 — 워커·렌더러·플랫폼 (app + platform)

- `apps/ultrakey-app/src/seek.rs`: `SeekSignal::SetQuery(String)` 추가 · `set_query_external` 호출 · `input_box_mode` 에 따른 렌더/포커스/복원 흐름 · 세션 열릴 때 스냅샷에 최전면 pid 추가 · `spawn_detection` 에 언어 목록 넘기기(기존 `ocr_languages` 클로저를 설정 우선·로케일 폴백으로 교체).
- `apps/ultrakey-app/src/overlay.rs`: `searchbar` 렌더에 `input_mode` 실어 보냄 · 새 Tauri 커맨드 `seek_set_query` 배선(워커 채널로).
- `crates/ultrakey-platform/src/overlay_window.rs`: `NsWindowHandle::set_can_become_key(bool)` — 등록부 추가/제거만(테스트 포함).
- `apps/ultrakey-app/src/main.rs`: `ocr_languages` 클로저를 설정 기준으로 교체.

### 작업 5 — 설정 UI + i18n

- `apps/ultrakey-app/ui/settings.html`: Seek 탭에 `검색 언어` 팝업(`data-key="seek.searchLanguage"`, `settings.seek.*` 라벨) 추가.
- `resources/i18n/{en,ko,zh,ja,es}.json`: `settings.seek.search_language`(+ 선택지 문구) 추가 — ⚠️ `frontend_wiring.rs` 의 `settings.seek.*` 자동 검사가 **5언어 전부** 통과해야 한다.
- popup 옵션은 백엔드가 내려주는 협약을 재사용하거나 프런트 상수로 — `General` 탭 언어 팝업처럼 endonym(선택지 자기 이름) 은 번역하지 않는다.

### 작업 6 — 명세·문서·수동 검증

- §D8 표의 파일 갱신 + `docs/dev/manual-verification.md` 항목 추가.

---

## 6. 테스트 계획

### 단위 테스트 (새로 추가)

| # | 모듈 | 시나리오 | 기대 |
| :--- | :--- | :--- | :--- |
| T1 | `ultrakey-seek-session/settings.rs` | `search_language` 부재 → `input_box_mode=false`, 언어 목록 없음(영어) | 기본값 = 영어 단일 |
| T2 | 동일 | `search_language="ko"` → `input_box_mode=true`, `["ko-KR","en-US"]` | 설정 반영 |
| T3 | `ultrakey-seek-session/config.rs` | `input_box_mode` 기본값 false, Copy 유지 | 회귀 없음 |
| T4 | `ultrakey-core/arbitration.rs` | 세션 활성 + `input_box_mode` + `ANSI_A`(modifier 없음) → **Pass** | 다국어 통과 |
| T5 | 동일 | 세션 활성 + `input_box_mode` + `DELETE` → **Pass** | backspace 통과 |
| T6 | 동일 | 세션 활성 + `input_box_mode` + `RETURN` → **여전히 Consume + SeekKey** | 제어 키 소비 유지 |
| T7 | 동일 | 세션 활성 + `input_box_mode` + `⌘A` · `⌃A` → **여전히 Consume** | ⌘⌃ 조합 보호(⌘Q 프로세스 종료 방지) |
| T7-b | 동일(§9 #3) | 세션 활성 + `input_box_mode` + `⌥E`(modifier option, 데드키) → **Pass** | ⌥+문자 통과 — 스페인어 악센트 |
| T7-c | 동일(§9 #3) | 세션 활성 + `input_box_mode` + **설정된 전역 단축키 조합**(예: ⌥Space 트리거) → **여전히 Consume** | 전역 단축키 가드 |
| T8 | 동일 | 세션 활성 + `input_box_mode=false` + `ANSI_A` → **여전히 Consume + SeekKey** | 영어 단일 회귀 제로 |
| T9 | 동일 | 세션 비활성 → 기존 경로(다국어 플래그 무관) | 회귀 없음 |
| T10 | `ultrakey-seek-session/machine.rs` | `set_query_external("한글")` → 상태 Querying·쿼리 반영·Repaint, 후보 필터 적용 | 외부 쿼리 경로 |
| T10-b | 동일(§9 #8) | **Opening 중** `set_query_external("한글")` → 쿼리 버퍼에 누적, `ingest_display` 가 도착하면 그 쿼리로 즉시 필터(한국어 OCR 3.1s 동안의 입력 유실 방지 — `machine.rs:360-369·853` 대조) | Opening 버퍼 |
| T11 | `ultrakey-seek-session/machine.rs` | `set_query_external("")` → Ready 로 복귀 | 빈 쿼리 처리 |
| T12 | `ultrakey-platform/overlay_window.rs` | 등록부 추가/제거 토글 — `can_become_key` 되읽기 | 토글 반영 |
| T13 | `keys.rs`(보조)(§9 #4) | `is_input_box_pass_key(ev, semicolon_cycles)` 와 `classify` 가 Text/Backspace 로 보는 키 집합이 일치 — **설정 on/off 파라미터화** | 판정 대조 |
| T14 | `frontend_wiring.rs`(기존) | 새 `settings.seek.*` 문자열 5언어 카탈로그 존재 | 자동 검사 통과 |

### 수동 검증 (`docs/dev/manual-verification.md`)

| # | 절차 | 기대 |
| :--- | :--- | :--- |
| M1 | 검색 언어 = `영어+한국어`, 한글 IME 활성, 캡스락 hold 로 세션 | 검색 바가 인풋 박스로 표시, 한글 조합·확정이 검색어에 음절 단위로 반영 |
| M2 | "한글" 입력 후 100ms 멈춤 | 쿼리가 화면 후보(한글 텍스트) 필터에 반영, 매치·카운터 갱신 |
| M3 | 조합 도중(자음만) 화면 | 중간 자모 상태가 검색어에 새지 않음(debounce·compositionend) |
| M4 | Enter 확정 → 클릭 → 하이라이트 해제 | 클릭 후 포커스가 F-04 가 활성화한 대상 앱에 남음(이전 앱으로 되돌아가지 않음) |
| M5 | Esc 취소 | 이전 최전면 앱으로 포커스 복원, 검색 바가 다시 NON-activating |
| M6 | 검색 언어 = 영어 단일 | 기존과 100% 동일(인풋 미표시, 포커스 무변화) |
| M7 | 다국어 세션 중 ⌘A·⌘Q | 소비(⌘Q 로 앱이 안 죽음), 검색어에 'a' 안 들어감 |
| M8 | 한국어 OCR 지연(약 3.1 s) 실측 | 검출 동안 입력이 유실되지 않고 후보 도착 후 필터 적용(S-6) |
| M9 | 한글 후보가 실제 화면에 있을 때(예: 한글 버튼) 매치 | `["ko-KR","en-US"]` 로 검출되어 매치됨 |
| M10 | 다국어 세션 중 `;` 순환 설정 켬/끔 각각에서 `;` 입력 | 켬 → 다음 매치 순환(소비) · 끔 → 검색어 `;`(통과) |
| M11 | 다국어 세션 중 다른 앱 창 클릭 | 키 상실 감지 → 세션 자동 닫힘, 클릭한 앱에 문자 누출 없음(§9 #11) |
| M12 | US 배열에서 `⌥+e`(데드키) → `e` | 웹뷰 인풋이 `é` 조합 — 스페인어 악센트 입력 확인(§9 #3) |
| M13 | 조합 도중(자음만) 스페이스/다음 문자 입력, 그리고 "마지막 입력 후 곧바로 Enter" | 조합이 음절로 굳은 뒤 쿼리 반영. Enter-디바운스 경쟁(리스크 9)의 실제 확인 |

---

## 7. 리스크 / 미확인

| # | 리스크 | 등급 | 대응 |
| :--- | :--- | :--- | :--- |
| 1 | **`canBecomeKey` 토글이 실제 키 윈도우 승격으로 이어지는가** — 스위즐 등록부 해제 + `makeKeyAndOrderFront` 실기기 검증 필요 | `(미확정)` | §4 D3 — 실기기 M1~M7. 토글이 안 되면 폴백은 "다국어 세션을 이번에 못 낸다"(기능 축소) 이지 회귀는 아니다 |
| 2 | **입력 소스(In: Ultrakey 앱이 키 윈도우를 잡는 순간 `.Accessory` 앱의 입력 소스가 마지막 사용 소스로 바뀔 가능성** | `(미확정)` | 한/영 키가 세션 중 통과하므로 사용자가 직접 전환 가능. M1 에서 실측·(필요시) 원래 앱 소스로의 복원 로직을 후속으로 |
| 3 | **중국어·일본어·스페인어 OCR 의 실측 지연·신뢰도** — 한국어(3.1 s)만 실측 | `(추정)` | `seek-ocr-latency-spike.md` 의 방법을 각 언어로 한 번씩 재현해 값을 기록. 기본값이 영어 단일이라 기본 경험은 보호됨 |
| 4 | **`set_query_external` 이 Entry Bar 입력과 마찰** — 다국어 세션에서 Rust 와 웹뷰가 쿼리를 두 소유자로 둔다 | 중 | D5 의 "다국어에서는 `<input>` 이 소스 오브트루스", 웹뷰 렌더는 값을 되쓰지 않음. T10·M2 로 고정 |
| 5 | **`compositionstart` 중 Enter/Esc** | 낮음 | IME 조합 중 Enter 는 조합 확정(웹뷰) — 계층 1 은 Enter 를 소비하므로 조합 확정에 개입. 실제로는 조합이 Enter 로 확정되지 않을 수 있으므로 M3 추가 검증 |
| 6 | **IME/webview 포커스 타이밍** — 키 윈도우가 된 직후 `input.focus()` 가 안 받는 순간이 있으면 첫 타자 유실 | `(미확정)` | 세션 열림 이벤트에서 `focused` 플래그 + `input.focus()` 재호출. M1 로 확인 |
| 7 | **재활성화된 이전 앱과 F-04 확인 경로의 경합** | 낮음 | D7 의 "클릭 확정 시 미복원" 비대칭이 구조적으로 막음. M4 로 확인 |
| 8 | **`input_box_mode` 게이트와 `seek_session_active` 의 게시 타이밍** | 중 | 워커가 `Opened` 효과를 얻는 시점에 두 게이트를 같은 원자값 근처에 함께 Release 게시. `input_box_mode` 는 **세션 열림 시점에 래칭**(열린 세션 중 변경은 다음 세션 적용 — `set_config` 계약 확장, §9 #6). T4~T9 로 고정 |
| 9 | ⭐ **Enter-디바운스 경쟁(#5)** — 마지막 입력 후 100ms 안에 Enter 를 누르면 웹뷰 flush 전에 계층 1 이 Enter 를 소비해 **직전 필터의 선택 인덱스로 확정**될 수 있음 | 중 | `compositionend` **즉시 전송**으로 조합 입력에서는 해소(디바운스는 비조합 idle 상한). 잔존하는 비조합 빠른 타이핑 + 곧바로 Enter 경쟁은 기록·수용(M13 으로 실측, 필요 시 다음 반복에서 Enter 를 웹뷰에 통과시키는 무거운 대안 검토) |
| 10 | ⭐ **다국어 세션 중 타 앱 클릭(#11)** — 검색 바가 키를 잃고, 통과되는 문자 키가 클릭된 앱에 새는 위험 | 중 | **키 상실 → 자동 닫힘(복원 없음)** 확정(D7). Confirming 은 예외. M11 로 확인 |

---

## 8. 상급 리뷰 요청 사항

1. **D2 — 사용자 제안(인풋 박스 + debounce)의 "유려함" 판정.** 특히 (a) "영어 단일 = 현행 유지, 다국어 = 인풋 박스"라는 **설정 분기**가 한결같은 UX 인가, (b) debounce 100ms 값의 적정성, (c) "OCR 탐색"을 "재검출 없는 필터 갱신"으로 단순화한 해석이 사용자 의도와 부합하는가.
2. **D3 — `canBecomeKey` 의 다국어 세션 한정 부분 해제.** 영어 단일 세션의 회귀 없음 + 세션 종료 시 재등록·포커스 복원(D7)의 대칭성. "기존 제약(canBecomeKey)·세션 라우팅·OCR 언어"가 모두 설정 분기로 보존되는가.
3. **D4 — 키 라우팅 역할 분담** (글자=웹뷰, 제어=맨틀). ⌘⌃⌥ 조합을 계속 소비하는 안전 판단. `is_input_box_pass_key` 를 `ultrakey-core` 에 두는 경계(크레이트 의존 방향)가 적절한가.
4. **D6 — OCR 언어의 설정 우선·로케일 폴백.** 이슈 #48 의 UI 로케일 자동 매핑을 "설정이 비면" 폴백으로 유지하는 것이 기존 사용자 회귀를 막는 최선인가, 아니면 완전히 분리(독립 축)가 맞는가.
5. **작업 분해 단위(6개)** 와 명세 갱신 범위(D8)가 위임·검증 단위로 적절한가.

---

## 9. ⭐ 상급 리뷰 반영 (2026-09-02, `ultrakey-review`)

> 판정: **조건부 통과.** §2 사실 표 인용 12건 전수 대조 ✅, 골격(설정 분기·등록부 토글·역할 분담 라우팅·필터 전용 debounce·설정 우선/로케일 폴백·비대칭 포커스 복원) 전부 승인. 아래 반영 수정 목록 #1~#5 는 **차단**(반영 전 구현 위임 금지), #6~ 는 반영 항목이다. #3(⌥)과 #11((i)/(ii))은 호출 세션이 **최종 확정**한 항목이다.

### 리뷰 판정 요지

- **D1 × D6 모순(차단 #1)**: 초안 D1 표 1행("빈 값/`"en"` → OCR `[]`")과 D6("빈 값 → `general.language` 로케일 폴백")이 한국어 UI 사용자에 대해 양립 불가. 해결: **부재 ≠ `"en"` 명시**. 부재 = 로케일 폴백(현행 유지, `input_box_mode=false`), `"en"` 명시 = `[]` 강제. `input_box_mode` 는 **명시적 비영어 값에서만** 파생.
- **D4 ⌥ 모순(차단 #3, 최종 확정 — ⌘/⌃만 차단):** "⌘/⌃/⌥ 가 안 실려 있으면 통과" 문안대로면 US 배열 스페인어 데드키(`⌥+e` → ´)가 웹뷰에 도달하지 못한다. **호출 세션 판정: 리뷰 권장 채택** — `input_box_mode` 에서 차단 대상은 **⌘/⌃ 만**, ⌥+문자는 통과(네이티브 입력 의미론의 일부). 단 **설정된 전역 단축키 조합(`matches_global_shortcut`)은 예외로 Consume** 가드를 둔다(토글 닫기 등 파악 불가한 조합이 검색어로 새지 않게). 이로써 D2 근거 1 의 "스페인어 악센트" 문구는 그대로 정합.
- **D4 `is_input_box_pass_key` 파라미터화(차단 #4):** `;` 통과/소비는 `semicolonCycle` 설정에 의존(`keys.rs:86`) — 판정 입력에 이 비트를 명시. 통과 키의 **KeyUp·FlagsChanged 도 통과**로 정의. `input_box_mode` 는 **세션 열림 시점에 래칭**(열린 세션 중 변경은 다음 세션 적용 — `seek.rs:116-120` 계약 확장).
- **D5 Enter-디바운스 경쟁(차단 #5, 최종 확정 — 리뷰 권장 채택):** 마지막 입력 후 100ms 안에 Enter 를 누르면 웹뷰가 flush 전에 계층 1 이 Enter 를 소비해 **직전 필터 결과로 확정**될 수 있다. **호출 세션 판정**: ① `compositionend` 시 **즉시 전송**(디바운스는 비조합 입력의 idle 상한으로만 — 조합 완성 즉시 쿼리를 먼저 보냄), ② 잔존 경쟁(비조합 빠른 타이핑 + 곧바로 Enter)은 §7 리스크 + 명세 엣지로 기록, ③ 다국어 세션에서 **Enter 로 한글 조합 확정이 불가**(계층 1 이 소비)라는 파생 엣지도 기록 — 조합은 Space·다음 문자로도 확정된다는 가정은 M-항목으로 실기 확인.
- **D7** 초안 열거는 `CloseReason` 전수로 완전. **#11 최종 확정 — (ii) 키 상실 시 자동 닫힘 채택**(리뷰 권장 (i) 수용을 기각). 근거: (i)은 다국어 세션에서 통과되는 문자 키가 **클릭된 타 앱에 새어** 암호 필드에 오타 입력 등 누출 위험을 만드는데, 이는 "세션 중 키는 하위 앱에 닿지 않는다"는 계층 1 의 기존 안전 계약과 어긋나는 새 해자드다. (ii)는 키 상실(`didResignKey`) 시 복원 없이 세션을 닫아 누출을 구조적으로 차단한다. ⚠️ **Confirming 상태(클릭 확정 처리 중)에서는 닫지 않는다** — F-04 가 대상 앱을 활성화하며 낸 `resignKey` 를 세션 취소로 오인하지 않도록.
- **D8 누락(차단 #9):** `docs/spec/README.md` 갈라짐 표에 **D10 행** 등재 필요 — 다국어 검색 + IME 입력은 원본에 없는 기능(근거: 원본 `Base.lproj` 영어 단일, `seek-text-detection.md` §5 #10 / 사용자 요구: 이슈 #93).
- **D6** — 폴백 유지가 맞다(#48 로 한국어 OCR 을 쓰던 사용자의 업그레이드 회귀 방지). 결정 사슬 **S-4 → 이슈 #48 → 이슈 #93** 을 명세에 한 행으로.
- **D8 오타** — "이슈 #8"→**#48**, 리스크 "SEO"→"IME/webview", D1 저장 키 `semicolonCycleSeek`(원본 plist 키) → **`seek.semicolonCycle`**(클론 키, `keys.rs:161`).

### 반영 수정 목록 (구현 위임 지시서 — 정본)

| # | 위치 | 수정 |
| :--- | :--- | :--- |
| 1 | §3 D1 표 + D6 | 표 분리: **부재 = 로케일 폴백**(현행, `input_box_mode=false`) / **`"en"` = `[]` 강제**(명시 오버라이드). `input_box_mode` 는 명시 비영어 값에서만 파생 |
| 2 | §3 D1 | 부재 시 팝업 계약: 로케일 파생 행이 선택된 것으로 표시. 사용자가 팝업을 조작해 값을 확정하는 순간 명시 값 저장(같은 값을 다시 골라도 — 선택 변경이 아니라 **팝업 조작 자체**를 트리거). 한국어 UI 사용자에게 `English only` 가 no-op 이 아님(명시 `"en"` = `[]` 강제) |
| 3 | §3 D4 + 작업 2 + T7 | **⌘/⌃/⌥ 판정 확정 — ⌘/⌃ 만 차단, ⌥+문자 통과(데드키)**. `input_box_mode` 에서 설정된 전역 단축키 조합은 예외 Consume 가드. T7 유지(`⌘A` → Consume) + **`⌥A` → Pass** 테스트 추가 + 전역 단축키 조합 가드 테스트 |
| 4 | §3 D4 + 작업 2 + T13 | `is_input_box_pass_key(ev, semicolon_cycles_gate)` — `semicolonCycle` 비트 입력 명시(`keys.rs:86`). T13 을 설정 on/off 파라미터화. **통과 키의 KeyUp·FlagsChanged 도 통과**로 정의(시프트·데드키 상태 보존) |
| 5 | §3 D5 + §7 | Enter-디바운스 경쟁 등재 + 완화(이상): `compositionend` 즉시 전송(디바운스 = 비조합 idle 상한). 기각 대안 ①의 "IPC 폭주" 논거 → "조합 중간 상태 유출"로 정정. 잔존 경쟁 + "Enter 로 조합 확정 불가" 엣지를 §7·명세에 기록, M-항목 추가 |
| 6 | §3 D4/D5 + 작업 2 | `input_box_mode` **세션 열림 래칭** 명시 — 열린 세션 중 변경은 다음 세션 적용(기존 `set_config` 계약 확장). 리스크 8 에 포함 |
| 7 | §3 D3 + 작업 4 | 재등록은 **등록부 전용 API**(`force_non_activating` 미경유 — 스위즐 재설치·`ULTRAKEY_OVERLAY_NO_SWAP` 킬 스위치 우회 방지). 등록부가 `canBecomeMainWindow` 도 함께 토글함을 주석 |
| 8 | §6 T10 옆 | `set_query_external` 의 **Opening 상태 버퍼** + 배치 도착 자동 필터 테스트 추가(`machine.rs:360-369`·`853` 대조) — M8(한국어 OCR 3.1s) 이 의존 |
| 9 | §3 D8 + `docs/spec/README.md` | 갈라짐 표 **D10 행** — "Seek 다국어 검색 + IME 입력"(성격: 신규 기능, 근거: 원본 영어 단일 + 이슈 #93) |
| 10 | §3 D8 + 명세 | `seek-text-detection.md` §3.2.2 에 결정 사슬 **S-4 → #48 → #93** 한 행. D7(키 상실 자동 닫힘)을 `seek-activation-and-session.md` §3.2 상태 머신 + §5 엣지에 행 추가 |
| 11 | §3 D7 + §6 + D8 | **다국어 세션 중 타 앱 클릭 = 키 상실 → 자동 닫힘(복원 없음, Confirming 예외)** 확정. 수동 검증 3건 추가(아래). 다국어 세션의 영문 타이핑 100ms 갱신 트레이드오프 명세 엣지 기록 |
| 12 | 본문 오타 | "이슈 #8"→#48 · "SEO"→"IME/webview" · D1 저장 키 → `seek.semicolonCycle` · 사실 6 인용 → `ultrakey-overlay/src/session.rs:112·323` · D2 근거 3 "이슈 #8"→#48 |

### 최종 확정 항목 (호출 세션 판정)

| # | 항목 | 확정 |
| :--- | :--- | :--- |
| 3 | ⌥+문자 통과 | ⭕ 채택 — `input_box_mode` 에서 ⌘/⌃ 만 차단, ⌥+문자/데드키 통과 + 전역 단축키 조합 Consume 가드. (스페인어 악센트 입력 가능) |
| 5 | Enter-디바운스 경쟁 | ⭕ 채택(리뷰 권장) — `compositionend` 즉시 전송 + 잔존 경쟁 기록. 더 무거운 대안(Enter/Esc 를 웹뷰에 통과시켜 flush 후 확정)은 이번 범위 기각 — 제어 키 소관이 둘로 갈라지는 표면 증가가 리뷰·검증 비용을 키운다 |
| 11 | 타 앱 클릭 엣지 | ⭕ (ii) 키 상실 자동 닫힘 채택 (리뷰 권장 (i) 기각 — 문단 위 근거). `didResignKey` + Confirming 예외 |

### 수동 검증 추가 (§6 테스트 계획에 병합)

| # | 절차 | 기대 |
| :--- | :--- | :--- |
| M10 | 다국어 세션 중 `;` 순환 설정 켬/끔 각각에서 `;` 입력 | 켬 → 다음 매치 순환(소비) · 끔 → 검색어 `;`(통과) |
| M11 | 다국어 세션 중 다른 앱 창 클릭 | 키 상실 감지 → 세션 자동 닫힘, 클릭한 앱에 문자 누출 없음 |
| M12 | US 배열에서 ⌥+e (데드키) → e | 웹뷰 인풋이 `é` 조합 — 스페인어 악센트 입력 확인 |
| M13 | 조합 도중(자음만) 스페이스/다음 문자 | 조합이 확정되어 음절로 굳은 뒤 쿼리 반영(Enter 로 조합 확정 불가 엣지 확인) |