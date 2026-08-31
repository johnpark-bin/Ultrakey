# 이슈 #47 — General 탭 가시성 개선(섹션 구분) + 로그 폴더 열기 — 확정 계획 + 작업 분해

> **성격**: `docs/plan/` 은 구현 위임 전 **계획 문서**를 두는 자리다. 이 문서는 중급(계획 초안)이 작성하고 상급(호출 세션)이 리뷰·교정해 **확정**한 판이다. "결정 목록(D1~D5)"이 정본이고, §4 의 i18n 번역과 §5 의 구현 순서가 구현 위임(`ultrakey-implement`)의 지시서다.
>
> ⭐ **상급 리뷰 교정 2건 (2026-08-31)**:
> 1. **D4 실패 모드 재교정** — 초안은 "디렉터리 없음 → Err 반환 + 프런트 진단"이었으나, 위임 지시서의 명시 제약 **"경로가 없으면(로그 미생성) 조용히 무시하거나 비활성화"** 와 충돌한다(`showFatal` 은 셸 전체를 숨기는 무거운 반응이다). **디렉터리 부재는 `Ok(())` + `tracing::warn`(조용히 무시)** 으로, **`open` spawn 실패만 `Err` 반환**(진짜 예외 — 기존 커맨드 계약 유지)으로 확정한다.
> 2. **§4 ko 번역 공백** — `"Finder에서 로그 폴더 열기"` → `"Finder 에서 로그 폴더 열기"`. 기존 카탈로그가 영어 고유명사와 조사 사이에 공백을 둔다(`"메뉴바 아이콘 숨기기"` why 의 `"Finder 에서 Ultrakey 를 다시 실행해야 한다"` 선례).
>
> **범위 제약**: 항목 순서 재배열, 컨트롤 추가·삭제·타입 변경, General 탭 7개 컨트롤의 라벨·동작 변경, F-12(라이선싱)·F-13(자동 업데이트) 구현 — 전부 이 문서의 범위 밖이다(기각/불변으로만 언급한다). 이번 작업이 하는 일은 ① General 탭의 **그룹 구분선과 섹션 헤딩** 추가(마크업·i18n·테스트) ② **로그 폴더 열기 버튼** 1개 + 백엔드 커맨드 1개 ③ 버전 버튼 우측 정렬(CSS 1줄)뿐이다.

---

## 1. 요약

현재 General 탭(`settings.html` §`panel-general`, 823~916행)은 **평면 나열**이다 — 언어 선택부터 라이선스 구매까지 9개 블록이 구분선 없이 이어지고, `설정 파일`·`진단` 헤딩 2개만 존재한다(h2.group, hr 없음). 반면 Presets 탭은 `<hr />` + `<h2 class="group">` 로 4개 그룹을, Hyperkey 탭은 첫 그룹 무헤딩 + `<hr />` 로 구분한다(settings.html 98·143행, 346~421·431~행). 이번 작업은 **기존 UI 규약 안에서** General 탭을 5개 섹션(언어 / Startup & menu bar / License / 설정 파일 / 진단)으로 묶고, 진단 섹션에 **로그 폴더 열기 버튼**을 더한다. 로그 디렉터리 경로는 `main.rs` 의 두 함수(`open_log_file`·`log_file_path_display`)가 각자 하드코딩하고 있으므로(3237·1291행) **`log_dir()` 헬퍼 1개로 통합**하고, 그 위에 `open_log_folder` 커맨드(`open` CLI — `on_menu_relaunch` 4687행 선례)를 얹는다. 신규 i18n 키는 **3개 × 5언어**다. 저장 키는 **하나도 만들지 않는다**. 회귀 방지는 기존 `frontend_wiring.rs` 패턴(`settings.*` 리터럴 검사 · invoke/main.rs/`generate_handler!` 3종 정적 배선 · `NEW_KEYS_*` 5개 카탈로그 검사)을 그대로 따른다.

**이 문서가 선행 조사에서 바로잡은 것 2건** (상급 잠정 설계에 대한 수정, §2 D4·D5):
1. `open_log_folder` 의 실패 모드 — 잠정안의 "디렉터리 없으면 조용히 `Ok(())`" 는 기각. **`Result<(), String>` 로 Err 반환**하고 프런트가 진단 문구를 보여준다(기존 `open_event_viewer` 커맨드 패턴과 일치, §2 D4).
2. "체크박스 세로 정렬 가시성 개선" — 잠정안에서 제거. `.row`·`.checkbox-row` 는 이미 `align-items: center`(settings.html 87·88행)로 중앙 정렬이고 구체적 결함이 실측되지 않았다("지어내지 않기" 규약). UI 변경은 **`#version-btn { margin-left: auto; }` 단 1줄**로 한정한다(§2 D3).

---

## 2. 결정 목록 D1~D5

### D1 — 섹션 구성: **`<hr />` 4개 + `h2.group` 헤딩 4개(신규 2 + 기존 2), 항목 순서 불변**

**결정** — 변경 후 구성(왼쪽이 위, 아래로 이어짐):

| # | 섹션(변경 전) | 섹션(변경 후) | 소속 항목(순서 그대로) | 헤딩 |
| :--- | :--- | :--- | :--- | :--- |
| 1 | 언어(헤딩 없음) | **언어 블록 — 헤딩 없음 (첫 블록)** | 언어 select + hint | — |
| 2 | — | `<hr />` + **Startup & menu bar** | 로그인 시 실행(+버전 버튼 + why + requires-approval + about 패널) · 자동 업데이트 · 메뉴바 아이콘 숨기기 · 메뉴바 아이콘 모양 | `settings.general.startup` (신규) |
| 3 | — | `<hr />` + **License** | 가장 오래된 활성화 해제 · 라이선스 구매 | `settings.general.license` (신규) |
| 4 | 설정 파일(헤딩만) | `<hr />` + 설정 파일 | 내보내기/가져오기 + hint + 결과 행 | `settings.general.transfer` (기존) |
| 5 | 진단(헤딩만) | `<hr />` + 진단 | Event Viewer + **로그 폴더 열기(신규)** + hint | `settings.general.diagnostics` (기존) |

**근거**:
- **구분선 방식은 Presets 탭의 정본 패턴**(`<hr />` 직후 `h2.group`)이다. General 탭의 기존 transfer/diagnostics 헤딩은 hr 없이 쓰였는데, 그것이 이번에 모순이 되는 것이 아니라 **다른 탭과 정합하도록 보강**된다 — 헤딩이 "이 그룹의 제목"이라면 그 앞에 경계가 있어야 그룹 경계가 시각적으로 선명하다(가시성 개선의 본 목적).
- **헤딩 개수는 4개**(신규 2 + 기존 2), `<hr />` 은 4개. 헤딩이 없는 블록은 첫 언어 블록뿐이다. 그룹 크기가 1(언어)·4(startup)·2(license)·2+버튼 2(transfer)·2+버튼 2(diagnostics) 로 어느 쪽도 공허하지 않다.
- **항목 순서·기존 id·기존 라벨을 하나도 바꾸지 않는다** — F-09 §4.4 의 원본 실측 구성("7개 컨트롤 + 조건부 1개", preferences-ui.md §4.4)과 F-09 §8 수용 기준("모두 존재하고")을 그대로 지킨다. 헤딩·hr 은 F-09 §3.2 컨트롤 카탈로그 바깥의 **레이아웃 요소**다(카탈로그에 "타입"으로 등재되어 있지 않고, Presets · Hyperkey 가 이미 자유롭게 쓴다) — 새 컨트롤 타입을 만들지 않는다는 제약 위반이 없다.
- **저장 영향 없음**: 헤딩·구분선은 저장 키와 무관(F-15 "부재 = 기본값" 무영향). 신규 저장 키 0개.
- **접근성**: 헤딩 계층은 `h1`(탭 제목) → `h2.group`(섹션) 으로 **Presets 탭과 동일한 구조**(438행 `presets-group-caps` 등) — 새 헤딩 레벨을 만들지 않는다. 첫 블록(언어)의 무헤딩은 Hyperkey 탭 선례(346행) 그대로. 신규 헤딩·버튼 라벨은 전부 카탈로그 문자열이 채우므로 화면 판독기 라벨이 자동으로 제공된다.

**기각한 대안**:
- **헤딩만 추가(hr 없음)**: 현재 transfer/diagnostics 헤딩이 정확히 이 상태인데 그룹 경계가 시각적으로 약하다 — 이슈의 "가시성 개선" 목적에 어긋난다.
- **`<hr />` 만 추가(헤딩 없음, Hyperkey 탭 스타일)**: Hyperkey 는 그룹이 1~3줄이라 hr 만으로 충분하지만, General 의 startup 그룹은 항목 4개 + why 문구 4개라 "무엇의 그룹인가"가 헤딩 없이는 흐릿하다. 진단·설정 파일 헤딩이 이미 존재하므로 그것과 새 그룹들을 같은 수준(헤딩 + 경계)으로 맞추는 것이 일관적이다.
- **언어 블록에도 헤딩("언어"/"General")**: 행 라벨이 이미 "언어"(`settings.general.language` = Language/언어/语言/Idioma/言語)인데 헤딩으로 또 "언어"를 반복하면 시각·판독기 양쪽에서 중복이다. "General" 헤딩은 탭 제목(`settings.general.heading`)과 중복. **Hyperkey 탭이 첫 그룹을 무헤딩으로 시작하는 선례**(346~357행 — hyper 슬롯이 헤딩 없이 바로 시작)를 그대로 따른다.
- **startup 을 "시작"·"메뉴바" 2개 그룹으로 분할**: 자동 업데이트가 "시작"에 어울리지 않는다는 관찰에서 나온 대안. 그러나 헤딩·hr 이 5~6개로 늘어 그룹 1·2 항목이 1~2줄밖에 안 되어 조각난다. "Startup & menu bar" 한 헤딩이 이 4개 항목을 자연스럽게 포괄한다(로그인 시 실행 = 시작, 메뉴바 아이콘 2종 = 메뉴바, 자동 업데이트 = 앱 수명주기 설정).

### D2 — 헤딩 키 이름·id: **평평한 키 2개(`settings.general.startup`·`settings.general.license`) + 기존 키 2개 재사용, id 는 기존 패턴 `general-*-heading`**

**결정**:
- 신규 i18n 키: `settings.general.startup` · `settings.general.license` (기존 `settings.general.transfer`·`settings.general.diagnostics` 와 **같은 평평한 형태**).
- 신규 헤딩 id: `general-startup-heading` · `general-license-heading` (기존 `general-transfer-heading`·`general-diagnostics-heading` 와 같은 패턴).
- `applyStrings()`(settings.html 991행)에 헤딩 텍스트 2줄 추가 — 기존 transfer(1126)·diagnostics(1132) 배선과 같은 방식.

**근거**:
- ⚠️ **상급 잠정 설계의 확인 요청 — `settings.general.license` 와 기존 `settings.general.license.why` 의 공존**: **가능하다. 확인 완료.** `resources/i18n/*.json` 은 **평평한 키-값 맵**이고(`all_catalogs_are_flat_string_maps` 테스트, `crates/ultrakey-i18n/src/lib.rs` 355행이 이 구조를 강제), `flatten_catalog` 은 키를 그대로 1:1 로 꺼낸다. 즉 `"settings.general.license": "License"` 와 `"settings.general.license.why": "…"` 는 **별개의 키**로 공존한다. `localization-and-input-sources.md` §3.1.5 의 "단일 카탈로그 — 평평한 키-값 파일"(72행)과도 일치. (JSON 이 중첩 객체였다면 같은 위치에 스칼라와 객체를 둘 수 없어 재구성이 필요했지만, 그 경우가 아니다.)
- 헤딩 id 패턴을 기존과 맞추면 frontend_wiring 의 정적 id 검사 문법(`id="{id}"`)에 그대로 들어간다.

**기각한 대안**:
- `settings.general.groups.startup` 류 **중첩형 네임스페이스 도입**: 기존 transfer/diagnostics 가 평평한 형태라 3중 그룹 prefix 는 기존 카탈로그 어휘와 어긋난다. 확장할 이유가 없다.
- `license` 대신 `licensing`·`activation`: `license.why` 키가 이미 `license` 접두사를 쓰므로 같은 어휘로 통일하는 것이 자연스럽다.

### D3 — UI 다듬기: **`#version-btn { margin-left: auto; }` 단 1줄 — 그 외 레이아웃 변경 없음**

**결정**: 로그인 시 실행 행(settings.html 839~847)에서 버전 버튼을 행 우측으로 정렬한다. `.row` 가 `display: flex`(87행)이므로 `margin-left: auto` 한 줄로 동작한다. 이슈의 나머지 UI 변경 없음.

**근거**: 원본 SuperKey 실측 — `Launch on login` 체크박스는 "첫 행 좌측", `v1.66 (66)` 버튼은 "**첫 행 우측(항목 1 과 같은 행)**"(preferences-ui.md §4.4, 291~292행). 현재 클론은 버튼이 체크박스 바로 옆에 붙어 있어 원본 배치와 어긋나고, 체크박스 라벨이 길어질 때(번역) 버튼이 중간에 끼는 모양이 된다. 원본 배치로 맞추는 것은 가시성 개선이고 라벨·동작 변경이 아니다.

**기각한 대안**:
- **잠정안의 "체크박스 세로 정렬 가시성 개선"** — **범위에서 제거한다.** `.row`·`.checkbox-row` 는 이미 `align-items: center` 로 세로 중앙 정렬이고(87·88행), 구체적으로 무엇이 어긋난다는 실측이 없었다. 정의되지 않은 개선을 계획에 넣으면 구현이 헤맨다 — 이 저장소의 "확인하지 못한 동작을 지어내지 않는다" 규약에 따른다. 구현 단계에서 실제로 비뚤림이 관찰되면 별도 이슈로 분리한다.
- 버튼을 별도 행으로 이동: 항목 순서·구성 변경(§2 D1 범위 제약)에 걸리고, 원본 실측("같은 행")과도 어긋난다.

### D4 — 로그 폴더 열기 커맨드: **`log_dir()` 헬퍼로 경로 단일화 + `open_log_folder() -> Result<(), String>`(open CLI), 디렉터리 부재는 조용히 무시·spawn 실패만 Err**

**결정** (⭐ 상급 리뷰에서 실패 모드를 재교정했다 — 문서 머리 주석 참고):
- `main.rs` 에 `fn log_dir() -> Option<std::path::PathBuf>` 헬퍼를 신설하고, `open_log_file()`(3233행)·`log_file_path_display()`(1288행)이 이 헬퍼를 쓰도록 재작성한다 — **동작은 그대로**(경로 값이 같음을 §6 테스트·리뷰로 확인). 두 함수의 `Library/Logs/Ultrakey` 경로 조각 **하드코딩 2곳이 1곳으로** 수렴하고, 신규 커맨드가 세 번째 사본을 만들지 않는다.
- `#[tauri::command] fn open_log_folder() -> Result<(), String>`:
  1. `log_dir()` 이 `None`(HOME 없음) → `tracing::warn` 남기고 **`Ok(())`**(조용히 무시).
  2. 디렉터리가 실제로 없으면(`!dir.is_dir()`) → `tracing::warn` 남기고 **`Ok(())`**(조용히 무시) — **만들지도 않는다**(커맨드는 읽기 전용).
  3. `std::process::Command::new("open").arg(&dir).spawn()` — **실패 시에만 `Err(e.to_string())`**, 성공 시 `tracing::info` + `Ok(())`.
  - 커맨드는 인자·상태가 없어 `AppHandle`·`State` 를 받지 않는다(tauri 커맨드의 무인자 시그니처 허용). 프런트는 `invoke("open_log_folder")`.
- `generate_handler!`(3579행 근처)에 `open_log_folder,` 추가.
- 프런트: `open-log-folder-btn` 클릭 리스너 — `invoke("open_log_folder").catch(err => showFatal("Failed to open the log folder: " + String(err)))`. **기존 `open-event-viewer-btn` 리스너(2501~2505)와 같은 패턴**. ⚠️ 이 진단 문구는 3번(spawn 실패) 경로 전용이다.

**근거**:
- ⭐ **위임 지시서의 명시 제약이 실패 모드를 정한다**: "로그 폴더 열기는 경로가 없으면(로그 미생성) **조용히 무시하거나 비활성화**". `showFatal` 은 셸 전체를 숨기고 진단 화면으로 교체한다(settings.html 975~980행) — "경로 없음"처럼 사용자가 조치할 것이 없는 상태에서 설정 창 전체를 가리는 것은 제약의 의도(조용한 처리)와 반대다. 그래서 1·2번은 `Ok(())` + 로그 경고로 조용히 넘긴다.
- **진짜 예외는 기존 커맨드 계약으로 알린다**: `open` spawn 실패는 환경 자체가 깨진 드문 상태라 기존 계약(`Result<(), String>` + 프런트 진단 — `open_event_viewer`(main.rs 2479행) + `ALLOWED_DIAGNOSTIC_LITERALS` "Failed to open the Event Viewer: " 선례)을 그대로 따른다. 즉 **"조용히 무시"는 지시서가 지정한 경로 없음 케이스에만 적용**하고, 그 외 실패는 기존 패턴이다.
- **"로그 디렉터리가 없다"는 정상 기동에서 일어날 수 없는 상태다** — `open_log_file` 이 기동 시 `create_dir_all` 로 이미 만든다(3238행). 도달했다면 로그 하위시스템이 실패한 것인데, 그 사실은 `tracing::warn`(stderr 폴백 포함)으로 운영 증거를 남기되 사용자 UI 는 건드리지 않는다.
- **`open` CLI 채택**: `on_menu_relaunch`(4687행)가 이미 `std::process::Command::new("open")` 을 쓴다. 앱 크레이트에 새 플랫폼 의존(`objc2`/NSWorkspace FFI)을 들이지 않는다 — `open <디렉터리>` 는 Finder 로 그 폴더를 여는 macOS 표준 동작이다.
- **읽기 전용 커맨드**: 디렉터리를 만들지 않는다 — "열기 버튼 클릭이 디스크에 쓰기 부작용을 일으킨다"를 막는다. 디렉터리 생성은 `open_log_file` 의 고유 책임으로 남긴다.

**기각한 대안**:
- **초안 — 디렉터리 없음도 `Err` 반환 + 프런트 진단**: 로직 자체는 정직하지만("로그 하위시스템 실패를 알릴 유일한 지점") 위임 지시서의 명시 제약(경로 없으면 조용히 무시)과 충돌하고, `showFatal` 의 무게(셸 전체 교체)가 이 케이스에 과하다. 제약이 있는 한 제약이 우선한다.
- **`create_dir_all` 후 성공 보장**: 버튼 클릭이 디스크 쓰기를 야기한다. 로그 디렉터리가 없다는 것 자체가 "만들어야 한다"가 아니라 "이미 실패 상태다"를 뜻한다.
- **버튼 비활성화(경로 없을 때)**: 프런트가 디렉터리 존재를 알려면 `AppMeta` 에 필드를 추가해야 한다 — 실제로는 항상 존재하는 경로(기동 시 생성)를 위해 부트스트랩 표면을 넓힐 실익이 없다. "조용히 무시" 쪽을 택한다.
- **NSWorkspace(`objc2-appkit`) FFI**: `open` CLI 로 충분한 단순 동작에, 이 앱의 유일한 unsafe 크레이트인 `ultrakey-platform` 밖에 새 FFI 전선을 깐다. 플랫폼 계층 변경·테스트 범위가 커지는 대가 없이 동작은 같다.

### D5 — 신규 i18n 키: **3개 × 5언어, 위치 인자 없음, 버튼 라벨에 목적지(Finder) 명시**

**결정**: §4 표의 3개 키(`settings.general.startup`·`settings.general.license`·`settings.general.open_log_folder`)를 5개 카탈로그에 추가한다. 모두 위치 인자(`{0}`)가 없다. 버튼 라벨은 **"Open Log Folder in Finder"** 로 확정.

**근거**:
- 버튼 라벨이 목적지를 명시하는 이유: `Open Event Viewer` 는 자체 창이 열리므로 라벨이 그 자체로 목적지지만, 로그 폴더는 **Finder(OS 창)** 가 열리므로 "Finder에서"가 사용자 예측에 도움을 준다. 보조 hint 텍스트를 추가하는 대신 라벨 하나로 정보를 전달한다 — 이슈 범위를 과하게 키우지 않는다(진단 섹션에는 이미 `settings.general.event_viewer.hint` 가 있고, 신규 로그 폴더 버튼은 그 hint 아래 같은 행에 들어가므로 설명이 아예 없는 것이 아니다).
- 위치 인자 없음 → `all_catalogs_preserve_positional_placeholders`(ultrakey-i18n 375행) 영향 없음.

**기각한 대안**:
- `Open Log Folder`(짧은 형태): Event Viewer 버튼과의 대칭만 보고 택하면 "어디서 열리지?"가 사용자에게 남는다. 위 근거.
- 힌트 문구 신설(`settings.general.open_log_folder.hint`): 버튼 라벨이 이미 목적지를 담고, 기존 Event Viewer hint 가 그 아래에 함께 보이므로 4번째 키를 추가할 실익이 없다 — 키가 늘면 5개 카탈로그 × 4키로 검사 표면이 커진다.

---

## 3. 현재 구성 조사 — 검증 완료 (이 문서 작성자가 소스를 직접 재확인)

| 항목 | 확인 결과 (라인 근거) |
| :--- | :--- |
| panel-general DOM | settings.html 823~916행. ① 언어 select + hint(831~837) ② 로그인 시 실행 + version-btn + why + requires-approval + about(839~857) ③ 자동 업데이트(859~866) ④ 메뉴바 아이콘 숨기기(868~874) ⑤ 메뉴바 아이콘 모양(876~883) ⑥ 활성화 해제(885~889) ⑦ 구매(891~895) ⑧ transfer 헤딩 + export/import + hint + result(900~906) ⑨ diagnostics 헤딩 + Event Viewer(910~914). **구분선 0개, 헤딩 2개(h2.group, hr 없음)** — 선행 조사와 정확히 일치 |
| `hr` 정의 | settings.html 98행 — 정확히 조사 내용과 동일 |
| `h2.group` 정의 | settings.html 143~146행 — 정확히 조사 내용과 동일 |
| Hyperkey 무헤딩 선례 | 346~357행 — hyper 슬롯이 헤딩 없이 시작, 369/389/402행 `<hr />` 로 구분 |
| Presets hr+헤딩 패턴 | 497행 `<hr />` → 500행 `<h2 class="group" id="presets-group-shift">` |
| 버전 버튼 배치 실측 | preferences-ui.md §4.4 291~292행 — "첫 행 우측(항목 1 과 같은 행)" ⭐ 클론 현재는 좌측 붙음(839~847행) |
| 컨트롤 카탈로그 | preferences-ui.md §3.2 110행 — `버튼` 타입이 General 행으로 등재(`v1.66 (66)`·`Remove Oldest Activation`·`Purchase`). 신규 컨트롤 타입 불필요 확인 |
| `open_log_file()` | main.rs 3233~3252행 — `HOME/Library/Logs/Ultrakey` create_dir_all, 2 MiB 초과 시 `ultrakey.log.1` 롤오버 |
| `log_file_path_display()` | main.rs 1288~1296행 — `HOME/Library/Logs/Ultrakey/ultrakey.log` 계산만. **경로 조각 하드코딩 2곳 확인** |
| `open` CLI 선례 | main.rs 4687~4695행 — `on_menu_relaunch` 가 `Command::new("open")` 사용. spawn 실패 시 tracing::error 만 남기고 계속(메뉴 경로 관용, §2 D4 참조) |
| `open_event_viewer` 커맨드 | main.rs 2478~2502행 — `Result<(), String>` + 프런트 `.catch` → `showFatal("Failed to open the Event Viewer: ")`(settings.html 2501~2505). **신규 커맨드의 패턴 정본** |
| `generate_handler!` | main.rs 3579행 `open_event_viewer,` 등록 |
| General 헤딩 채우는 경로 | settings.html `applyStrings()` 1126·1132행 — transfer/diagnostics 헤딩 텍스트를 여기서 채운다. 신규 헤딩도 여기 추가 |
| General 렌더·리스너 | `renderGeneral(meta)`(2333) + `version-btn` 클릭 리스너(2460~2463, about 토글) — 신규 버튼 리스너는 open-event-viewer-btn 리스너 옆에 추가 |
| i18n 저장 구조 | `resources/i18n/*.json` — **평평한 키-값 맵** 확인(`"settings.general.transfer": "…"` 형태). ultrakey-i18n `all_catalogs_are_flat_string_maps`(355행)가 구조 강제므로 신규 키도 평평하게 |
| 기존 general 키 현황 | 5개 카탈로그 동일: `transfer`·`diagnostics`·`license.why` 존재, `startup`·`license`·`open_log_folder` 부재. `settings.general.license` 추가가 `license.why` 와 충돌하지 않음(§2 D2) |

**frontend_wiring.rs 재확인 (회귀 장치)**:

| 장치 | 라인 | 이번 변경과의 관계 |
| :--- | :--- | :--- |
| `LOCALES = ["en","ko","zh","es","ja"]` | 68 | 신규 키 검사에 사용 |
| `settings_html_의_settings_점_리터럴은_전부_카탈로그_키다` | 320 | en 카탈로그 기준으로 settings.html 의 `"settings.*"` 리터럴을 전수 검사. **신규 헤딩 키는 먼저 카탈로그에 추가되어야 이 테스트가 통과한다** — 변경 순서 제약(§5) |
| `settings_html_에_하드코딩된_영어_문장이_없다` | 371 | `ALLOWED_DIAGNOSTIC_LITERALS` 에 "Failed to open the Event Viewer: "(393) 선례. **신규 "Failed to open the log folder: " 를 여기에 추가 필요**(근거 주석 포함) |
| `키보드_복사_커맨드가_invoke_배선되어_있다` | 1143 | invoke(html) + `fn` 선언(main.rs) + `generate_handler!` 등록 3종 정적 검사 — **신규 로그 폴더 커맨드 테스트의 패턴** |
| `settings_html_에_export_import_이벤트뷰어_버튼이_있고_커맨드를_invoke_한다` | 1690 | General 버튼 id + 커맨드 invoke 배선 — 신규 `open-log-folder-btn`·`open_log_folder` 를 여기에 추가 |
| `NEW_KEYS_FOR_ISSUE_39_PHASE_2_3` | 1654 | 5개 카탈로그 존재 검사. transfer·diagnostics 가 이미 이 목록에 있음 — `NEW_KEYS_FOR_ISSUE_47` 는 별도 목록으로 신설 |
| `main_rs_의_menu_점_리터럴은_다섯_카탈로그_모두에_있다` | 671 | main.rs 리터럴을 5개 카탈로그 전수 검사하는 선례(스캔식) — 신규 커맨드는 카탈로그 키를 직접 쓰지 않으므로 무관 |

**파손 위험 검토 결과**: 기존 General 탭 id(`version-btn`·`general-language`·`settings-export-btn`·`open-event-viewer-btn` 등)와 호출을 하나도 바꾸지 않으므로, panel-general 마크업에 의존하는 기존 테스트(`settings_html_에_general_language_select가_있다` 1589 · `settings_html_에_export_import_…` 1690)는 전부 그대로 통과한다. `<hr />`·`h2.group` 삽입 위치는 기존 블록 **사이**이므로 DOM 순서 기반 검사가 없다(hr 총 개수를 세는 기존 테스트도 없다 — 확인됨).

---

## 4. 신규 i18n 키 — 3종 × 5언어 (번역 확정)

> ⚠️ 3개 키 모두 **위치 인자 없음**. `all_catalogs_preserve_positional_placeholders` 테스트 영향 없음. 번역 톤은 기존 카탈로그 어휘를 그대로 따른다 — 참고: `settings.general.event_viewer`(en "Open Event Viewer" / ko "Event Viewer 열기" / zh "打开 Event Viewer" / es "Abrir el Event Viewer" / ja "Event Viewer を開く"), `settings.general.launch_on_login`(ko "로그인할 때 Ultrakey 시작"), `settings.general.menu_bar_icon`(ko "메뉴바 아이콘 모양" / zh "菜单栏图标样式" / es "Estilo del icono de la barra de menús" / ja "メニューバーアイコンのスタイル"), `settings.general.purchase`(ko "라이선스 구매" / zh "购买许可证" / es "Comprar una licencia" / ja "ライセンスを購入"). Finder 는 전 언어에서 **고유명사 유지**(macOS 현지화 관례).

| 키 | en | ko | zh | es | ja |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `settings.general.startup` | `Startup & menu bar` | `시작 및 메뉴바` | `启动与菜单栏` | `Inicio y barra de menús` | `起動とメニューバー` |
| `settings.general.license` | `License` | `라이선스` | `许可证` | `Licencia` | `ライセンス` |
| `settings.general.open_log_folder` | `Open Log Folder in Finder` | `Finder 에서 로그 폴더 열기` | `在 Finder 中打开日志文件夹` | `Abrir la carpeta de registros en Finder` | `Finder でログフォルダを開く` |

- zh 는 "打开 Event Viewer" 짧은 톤이 있으나, "在 Finder 中打开..." 은 Finder 앞에 전치사를 두는 해당 카탈로그의 자연스러운 표현이다(`在 Finder 中隐藏…` 류와 어휘가 일치).
- ko `Finder 에서 로그 폴더 열기`: 기존 "Event Viewer 열기" 형식(동사로 끝)을 따르되 목적어를 밝혔다. ⭐ 영어 고유명사와 조사 사이 공백은 기존 카탈로그 규약이다(`"메뉴바 아이콘 숨기기"` why: `"Finder 에서 Ultrakey 를 다시 실행해야 한다"`).

---

## 5. 구현 순서 체크리스트 (파일 단위 · 실행 순서)

> ⚠️ (2)와 (3)의 순서가 회귀 테스트 통과에 필수다 — `settings_html_의_settings_점_리터럴은_전부_카탈로그_키다`(en)와 `NEW_KEYS_*`(5개)가 키 누락을 즉시 잡으므로 **카탈로그를 먼저** 넣는다.

1. **main.rs — `log_dir()` 헬퍼 추출 + 기존 2함수 재작성 (동작 무변경)**
   - `fn log_dir() -> Option<std::path::PathBuf>` — `std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Logs/Ultrakey"))` (1288행 근처에 배치, 두 함수가 보이는 위치).
   - `log_file_path_display()` → `log_dir().map(|d| d.join("ultrakey.log").display().to_string()).unwrap_or_default()` (**기존 None→`String::new()` 동작 보존** — build_app_meta 가 빈 문자열로 전달받던 경우를 바꾸지 않는다).
   - `open_log_file()` → `let dir = log_dir()?;` 로 시작하도록 치환(3236~3237행 대체). create_dir_all·롤오버·append 로직은 그대로.
   - ⚠️ 이 단계에서 `cargo test -p ultrakey-app` — 기존 로그 경로 테스트가 있으면 통과 확인(없다면 경로 문자열 산출이 동일함을 리뷰로 확인).

2. **resources/i18n/{en,ko,zh,es,ja}.json** — §4 의 키 3개를 5개 파일에 **같은 키이름으로** 추가(값은 언어별). ⚠️ en.json 을 먼저 넣어야 (3)의 HTML 리터럴이 `settings_…리터럴은_전부_카탈로그_키다` 를 통과할 수 있다.

3. **main.rs — `open_log_folder` 커맨드 + 등록**
   - §2 D4 의 시그니처·실패 모드(디렉터리 부재 = `Ok(())` + warn, spawn 실패만 `Err`). `open_event_viewer`(2478) 근처에 배치(진단 커맨드군).
   - `generate_handler!`(3579 부근)에 `open_log_folder,` 추가.

4. **settings.html — 마크업 + CSS + JS**
   - `panel-general`(823~916) 안에, 기존 블록 사이에만 삽입:
     - 837행(언어 hint) 뒤: `<hr />` + `<h2 class="group" id="general-startup-heading"></h2>`
     - 895행(구매 why) 뒤: `<hr />` + `<h2 class="group" id="general-license-heading"></h2>`
     - 899행(transfer 주석) 앞: `<hr />` (기존 `#general-transfer-heading` 는 그대로)
     - 909행(diagnostics 주석) 앞: `<hr />` (기존 `#general-diagnostics-heading` 는 그대로)
     - 912행 Event Viewer 버튼 근처: `<button id="open-log-folder-btn"></button>` — **같은 `.row`(911~913) 안, `open-event-viewer-btn` 옆에 추가**(transfer 의 export/import 가 한 `.row` 에 둘인 선례와 동일). 기존 `general-event-viewer-hint`(914)는 그대로.
   - CSS: `<style>` 안에 `#version-btn { margin-left: auto; }` 1줄(88행 `.checkbox-row` 근처 또는 98행 hr 근처).
   - JS:
     - `applyStrings()`(1126 근처): `$("general-startup-heading").textContent = t("settings.general.startup");`, `$("general-license-heading").textContent = t("settings.general.license");`, `$("open-log-folder-btn").textContent = t("settings.general.open_log_folder");`
     - `open-event-viewer-btn` 리스너(2501) 옆: `$("open-log-folder-btn").addEventListener("click", () => { invoke("open_log_folder").catch((err) => { showFatal("Failed to open the log folder: " + String(err)); }); });`
   - ⚠️ 기존 id·함수·주석 **일절 무변경**. 창 크기·레이아웃 클래스·컨트롤 타입 무변경.

5. **frontend_wiring.rs — 기존 확장 + 신규 테스트** (§6.2)

6. **검증**: `cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings` · §7 실기기 확인.

---

## 6. 테스트 · 검증 계획

### 6.1 기존 테스트가 자동으로 잡는 것 (변경 순서가 틀리면 여기서 실패)

| 테스트 | 걸리는 시점 |
| :--- | :--- |
| `settings_html_의_settings_점_리터럴은_전부_카탈로그_키다` (320) | (4)에서 HTML 에 `t("settings.general.startup")` 등을 넣었는데 (2) 카탈로그가 먼저 안 들어갔으면 즉시 실패 |
| `settings_html_에_하드코딩된_영어_문장이_없다` (371) | (4)의 showFatal 문구가 `ALLOWED_DIAGNOSTIC_LITERALS` 에 없으면 실패 → §6.2 (a) 와 함께 |
| `settings_html_에_export_import_이벤트뷰어_버튼이_있고_커맨드를_invoke_한다` (1690) | 확장 후: `open-log-folder-btn` id + `open_log_folder` invoke 누락 시 실패 |
| `이슈_39_phase_2_3_신규_카탈로그_키가_…` (1671) | transfer·diagnostics 재사용이므로 무변경 통과(기존 키 유지 확인) |
| ultrakey-i18n: `all_catalogs_have_identical_key_sets`·`all_catalogs_are_flat_string_maps`·`all_catalogs_preserve_positional_placeholders` | (2)에서 5개 중 하나라도 키를 빼먽거나 평평하지 않거나 위치 인자가 어긋나면 실패 |

### 6.2 frontend_wiring.rs 에 추가/수정할 테스트 (구현 진행과 같은 위임에 포함)

| 테스트(제안 이름) | 종류 | 검증 내용 |
| :--- | :--- | :--- |
| (a) `settings_html_에_하드코딩된_영어_문장이_없다` **수정** | 허용 목록 추가 | `ALLOWED_DIAGNOSTIC_LITERALS` 에 `"Failed to open the log folder: "` 추가 + 주석(open_event_viewer 항목과 같은 근거 — 진단 전용, 카탈로그를 못 믿을 수도 있는 경로) |
| (b) `settings_html_에_export_import_이벤트뷰어_버튼이_있고_커맨드를_invoke_한다` **수정** | 기존 확장 | id 배열에 `"open-log-folder-btn"`, 커맨드 배열에 `"open_log_folder"` 추가 |
| (c) `로그_폴더_커맨드가_invoke_배선되어_있다` **신규** | 이슈 #46 3종 정적 배선 패턴(`키보드_복사_커맨드가_invoke_배선되어_있다` 1143행 모방) | ① settings.html 이 `invoke("open_log_folder"` 호출 ② main.rs 에 `fn open_log_folder(` 선언 ③ main.rs 의 `generate_handler!` 목록에 `open_log_folder,` 존재 |
| (d) `general_탭에_섹션_헤딩과_구분선이_있다` **신규** | 정적 구조 | `panel-general` 블록을 `<section id="panel-general"` 시작부터 `</section>` 까지 잘라내고(HTML 주석 제거 후) 그 안의 `<hr />` 가 **4개**, 그리고 `id="general-startup-heading"`·`id="general-license-heading"`(신규)·`id="general-transfer-heading"`·`id="general-diagnostics-heading"`(기존) 4개가 모두 존재. ⚠️ hr 카운트는 panel-general 스코프로 한정한다(전체 HTML 에서 세면 다른 탭 수정에 취약). |
| (e) `이슈_47_신규_카탈로그_키가_다섯_카탈로그_모두에_있다` **신규** | `NEW_KEYS_FOR_ISSUE_39_PHASE_2_3` (1654) 모방 | `const NEW_KEYS_FOR_ISSUE_47: &[&str] = &["settings.general.startup", "settings.general.license", "settings.general.open_log_folder"];` 를 `LOCALES` 5개 카탈로그 전부에서 `flatten_catalog` 로 확인(누락 키 보고). |

### 6.3 main.rs 인라인 테스트

- `log_dir()`·`open_log_folder()` 는 **HOME 환경변수에 의존**해 단위 테스트로 고정하기 어렵다(기존 `open_log_file` 도 테스트가 없다). 경로 산출 동일성은 **리뷰 단계**에서 `log_dir` → `join("ultrakey.log")` 가 기존 두 함수와 같은 경로임을 확인하고, §7 실기기로 동작을 확인한다. 커맨드 등록 여부는 6.2 (c) 가 정적으로 커버한다.
- ⚠️ 구현 시 커맨드 오류 문자열("cannot resolve log directory" 등)은 **tracing 매크로가 아니다** — `log_string_discipline.rs`(tracing 안 한국어·카탈로그 금지)는 tracing 호출만 검사하므로 이 문자열에는 영향이 없다. 다만 이 문자열들은 프런트 `showFatal` 로 합성되는 진단 문구이므로 영어를 그대로 유지한다(카탈로그를 거치지 않는다 — `ALLOWED_DIAGNOSTIC_LITERALS`(a) 와 정합).

---

## 7. 실기기 확인 항목 (서명된 .app, `docs/dev/manual-verification.md` 후속 갱신 대상)

1. 로그가 실제로 기록된 상태에서 `Open Log Folder in Finder` 클릭 → Finder 가 `~/Library/Logs/Ultrakey` 폴더를 연다(`ultrakey.log` 파일 자체가 아니라 폴더).
2. 롤오버 상태(로그가 2 MiB 초과해 `ultrakey.log.1` 이 존재)에서도 같은 동작 — 폴더 진입 후 두 파일이 모두 보인다.
3. 언어를 `General > Language` 로 바꾼 뒤 ① 새 섹션 헤딩 2개 ② 로그 폴더 버튼 라벨 ③ 기존 transfer/diagnostics 헤딩이 모두 번역된다.
4. 버전 버튼이 로그인 시 실행 행의 **우측 끝**에 정렬되고, 체크박스 라벨 길이가 달라져도(번역) 우측 정렬이 유지된다.
5. (드문 경로) 로그 디렉터리를 수동으로 지운 뒤(앱 재기동 전) 버튼 클릭 → **아무 일도 일어나지 않는다**(조용히 무시 — 위임 지시서 제약). 로그(stderr 포함)에 경고가 남고 앱은 정상 동작한다. 이후 앱 재기동 시 `open_log_file` 이 디렉터리를 다시 만들고 로그가 재개된다.
6. General 탭 전체 스크롤: 5개 섹션(언어 — Startup & menu bar — License — 설정 파일 — 진단) 경계가 구분선·헤딩으로 선명하게 나뉜다. 기존 컨트롤 동작(언어 전환·launch on login·내보내기/가져오기·Event Viewer)에 회귀가 없다.

---

## 8. 리스크 / 미확인 항목

| # | 항목 | 등급 | 내용과 완화 |
| :--- | :--- | :--- | :--- |
| 1 | "Startup & menu bar" 헤딩 아래 자동 업데이트가 묶이는 의미론 | 낮음(의견) | 자동 업데이트는 "시작"이나 "메뉴바" 어느 쪽에도 딱 맞지 않는다. 그러나 그것을 분리할 만한 그룹 크기가 안 되고(항목 1개), 이번 범위는 "헤딩을 붙여 시각 경계를 만든다"가 핵심이라 어울리는 그룹명으로 포괄했다 — 사용자 피드백이 오면 startup 을 "앱 동작" 계열로 개명하는 것을 재검토할 수 있다(§2 D1 기각한 대안에 기록됨). |
| 2 | `log_dir()` 헬퍼로 두 함수를 재작성할 때 경로 산출의 불변 | 낮음(확인 필요) | 리뷰 단계에서 `join("Library/Logs/Ultrakey")` → `join("ultrakey.log")` 합성이 기존 문자열과 동일함을 확인한다. 단위 테스트가 env 에 의존해 자동화 불가라 §7 확인 항목 1·2 가 최종 근거가 된다. |
| 3 | panel-general 스코프의 hr 카운트 검사(6.2 d)가 HTML 주석 안의 `<hr` 에 오탐 | 낮음 | settings.html 의 panel-general 주석에 `<hr` 이 없다(확인됨). 그래도 테스트 구현은 스코프 추출 후 HTML 주석을 제거하고 카운트하도록 지시한다(이 저장소의 테스트는 문자열을 못 믿고 스스로 방어한다는 기존 정신). |
| 4 | `open_log_folder` Err 문자열이 사용자 대면이 된다(카탈로그 미경유) | 알려진 설계(기존과 동일) | `open_event_viewer` 의 "Failed to open the Event Viewer: " 와 같은 관례 — 진단 전용. `ALLOWED_DIAGNOSTIC_LITERALS` (a) 로 허용 목록에 명시하고 근거를 남긴다. |
| 5 | i18n 5개 카탈로그의 키 집합 동일성 위반 | 낮음(장치 있음) | (2) 단계에서 5개 파일을 전부 고치지 않으면 `ultrakey-i18n` 테스트 + 6.2 (e) 가 이중으로 잡는다. |
| 6 | 이슈 #47 의 "가시성 개선"에 사용자 기대의 다른 부분이 있을 수 있다(예: 항목 재배열) | 미확인 | 초안 작성 시점에 이슈 본문의 재배열 요구는 확인되지 않았다 — **이 문서는 재배열을 범위 밖으로 명시**하고, 상급(호출 세션)이 이슈 #47 본문과 대조해 범위 차이가 있으면 이 문서를 고친다. |
