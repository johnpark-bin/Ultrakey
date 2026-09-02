# 이슈 #87 — 트레이 메뉴 `About` 이 설정 창을 여는 문제 → 독립 정보(About) 창 신설 — 계획 초안

> **성격**: `docs/plan/` 은 구현 위임 전 계획 문서를 두는 자리다. 이 문서는 중급(계획 초안)이 작성하고 상급(호출 세션)이 리뷰·교정해 **확정**한 판이 된다.
>
> **범위 제약**: About 창 신설 + 정보 이동 + 메뉴 배선 + i18n. F-13 업데이트 로직·설정 저장 키·엔진 코드는 건드리지 않는다. ⚠️ **병행 이슈 #88**(창 파괴 후 재생성 부재)과 창 관리 코드 정합 필요.

---

## 1. 요약

현재 트레이 메뉴 `About`(`menu.about`)는 `main.rs:5292` 에서 `menu_ids::SETTINGS | menu_ids::ABOUT => show_settings_window(app)` 로 **설정 창을 연다**. 이는 `main.rs:4987-4994` 의 기존 결정("별도 About 창을 만들지 않는다")에 따른 것이며, **사용자 피드백(2026-09-02)이 이를 뒤집는다**: `About` 선택 시 정보성 데이터와 **업데이트 체크 버튼**이 노출되어야 한다. 설정 General 탭의 **앱 설치 위치·로그 위치**는 정보 페이지로 이동한다.

**핵심 실측 사실**:

1. About → 설정 창 경로는 **한 줄**이다 — `main.rs:5292` 매치 암. 상수·메뉴 항목은 유지하고 **라우팅만 분리**.
2. **별도 창 빌더 선례 존재** — Event Viewer(`main.rs:2792-2816`): `WebviewWindowBuilder` + "이미 열려 있으면 show/focus, 없으면 생성". `tauri.conf.json` `app.windows` 에는 settings·permissions 두 창만 정적.
3. **General 탭 About 행 데이터 소스 = `AppMeta`** — `build_app_meta`(`main.rs:1346-1354`). **설치 위치(앱 path)는 현재 `AppMeta` 에 없다** — `std::env::current_exe()`(`main.rs:3726` 선례)로 얻어야 함 `(추정)`.
4. **업데이트 체크는 `on_menu_check_for_updates`(`main.rs:2122-2138`)가 완결** — Sparkle 표준 UI 가 결과 표시. .app 밖에서는 `None` 조용히 무시.
5. i18n — `settings.general.about.*` 4키 · `settings.general.version` · `menu.about`.
6. `#version-btn` 토글(`settings.html:3121-3124`) — 이동 시 처리 방식 결정 필요(D3).
7. **#88 병행** — 설정 창 닫으면 파괴, 재생성 부재. #88 은 옵션 A(hide)/B(재생성)를 확정. **About 창도 같은 정책 따라야 함.**

**이 작업이 하는 일**: ① `about.html` 신설 ② `show_about_window` + 라우팅 분리 ③ General About 행(설치·로그 위치) 이동 ④ 업데이트 체크 버튼 ⑤ i18n 신규 키 5언어 ⑥ 명세 2건. **신규 저장 키 0 · 엔진 변경 0 · F-13 로직 변경 0.**

---

## 2. 결정 목록 D1~D6

### D1 — About 화면 형태: **별도 HTML(`about.html`) + 별도 창 (Event Viewer 빌더 패턴)**

- `ui/about.html` 신설 + `WebviewWindowBuilder` 동적 생성, "이미 열려 있으면 show/focus, 없으면 생성". `tauri.conf.json` 정적 선언 안 함.
- 근거: 사용자 원문 "정보 창 열리지 않음"은 별도 화면을 기대. Event Viewer 선례가 완결적. 설정 창(825×821)에 탭 추가는 용량 대비 과함. `#88` 정합(옵션 A/B 어느 쪽이든 동일 적용).
- **기각**: 설정 창 안 정보 탭(사용자 요구와 어긋남)·설정 창 안 모달(상시 접근 화면에 부적합)·`tauri.conf.json` 정적 선언(#88 결정에 따라 재작업).

### D2 — 정보 목록: **앱 이름 · 버전 · 제작자 · 웹사이트 · 연락 · 이슈 등록 경로 · 업데이트 체크 버튼**

| 항목 | 값 | 출처 |
| :--- | :--- | :--- |
| 앱 이름 | `Ultrakey` | 하드코딩(브랜드 고유명사) |
| 버전 | `v{0}` | `app.package_info().version` |
| 제작자 | `John Park` | `authors` 확인 `(추정)` — 실패 시 하드코딩 |
| 웹사이트 | URL 결정은 **호출 세션에 남김** | — |
| 연락 방법 | 결정 필요 | — |
| 이슈 등록 경로 | GitHub Issues URL — 결정 필요 | — |
| 업데이트 체크 | `Check for Updates…` | `on_menu_check_for_updates` 재사용 |

- ⚠️ **URL 3종(웹사이트·연락·이슈)은 이 계획이 지어낼 수 없는 사실** — 호출 세션이 결정, 미결정 상태로 구현 시작하지 않음.
- **기각**: 라이선스 상태 포함(F-12 소관, General 에 이미 있음 — 표면 2개 어긋남 위험).

### D3 — 이동 항목 처리: **General 탭 About 행 제거 → About 창으로 이동 (복제 아님)**

- **이동**: `#about` 블록(`settings.html:1011-1015`)의 **로그 위치** → About 창. **설치 위치는 신규** — `AppMeta.app_path` 필드(`std::env::current_exe()` 기반, .app 번들 경로 유도).
- **제거**: `#about` 블록·`#version-btn`·클릭 토글 제거(About 행 없어지면 버튼 존재 이유 소실).
- **유지 이동**: 번들 ID·설정 파일 경로 행은 **General 탭 Advanced 섹션**(이슈 #77) 진단 버튼 행 아래로 이동(진단 성격). `settings.general.about.bundle_id`·`settings_path`·`settings_absent` 키 유지, `log_path` 키는 About 창 키로 재배치.
- **기각**: General 유지 + About 복제(표면 2개 어긋남), `#version-btn` 유지 + About 열기(중복 진입점 — 상급 판단).

### D4 — 업데이트 체크 버튼: **`on_menu_check_for_updates` 재사용, 신규 로직 없음, Sparkle 표준 UI 에 위임**

- 진행·결과 표시는 Sparkle 네이티브 대화상자. .app 밖에서는 버튼 **비활성**(메뉴 항목과 같은 `bundle::is_running_from_app_bundle()` 기준).
- 얇은 `#[tauri::command]` 래퍼 1개 허용 `(추정 — 구현 시 판단)`.

### D5 — 메뉴 배선: **`main.rs:5292` 라우팅 분리 + `show_about_window` 신설**

- `menu_ids::SETTINGS => show_settings_window(app)` / `menu_ids::ABOUT => show_about_window(app)`.
- `show_about_window` — Event Viewer 패턴. `ABOUT_WINDOW_LABEL: &str = "about"` 상수 신설.
- `main.rs:4987-4994` 기존 결정 주석을 지우지 않고 **"이슈 #87 로 뒤집힘"** 붙이기.
- #88 정합 — show_about_window 는 #88 정책을 따름.

### D6 — i18n: **신규 키 ~11개 × 5언어 + `log_path` 재배치**

- 신규 `about.` 네임스페이스: `title`·`app_name`·`version`·`author`·`website`·`contact`·`issues`·`check_for_updates`(`menu.check_for_updates` 재사용 가능)·`app_path`·`log_path`·`outside_bundle`.
- 재배치: `settings.general.about.log_path` → `about.log_path`. 유지: `bundle_id`·`settings_path`·`settings_absent`·`version`·`menu.about`.
- ⚠️ `frontend_wiring.rs` 의 `settings.general.about.*` 참조 테스트 존재 여부 구현 시 확인.

---

## 3. 작업 분해 (파일 단위)

### 3.1 `apps/ultrakey-app/ui/about.html` — 신규

- `eventviewer.html` 구조 + settings.html CSS 규약(`:root { color-scheme }`·`Canvas`/`CanvasText`·`-apple-system`). 정보 행(§2 표) + 업데이트 버튼. 링크는 `bundle::open_url`(`main.rs:2353` 선례) 재사용 — 커맨드 래퍼 필요 시 신설 `(추정)`.
- 부트스트랩: `about_bootstrap` 신규 또는 `settings_bootstrap` 재사용 `(추정)`.

### 3.2 `apps/ultrakey-app/src/main.rs`

1. `handle_menu_event` 라우팅 분리(5292)
2. `show_about_window` 신규(Event Viewer 패턴) + `ABOUT_WINDOW_LABEL`
3. `AppMeta.app_path` 필드(`build_app_meta`)
4. 기존 결정 주석 뒤집기(4987-4994)
5. `check_for_updates` 래퍼 + 필요 시 `open_url` 래퍼

### 3.3 `apps/ultrakey-app/ui/settings.html`

- `#about` 블록·`#version-btn`·토글 리스너 제거. 번들 ID·설정 파일 경로 행을 Advanced(`#general-advanced`)로 이동. `renderGeneral`·`applyStrings` 배선 갱신.

### 3.4 i18n 5개 — 신규 키 + `log_path` 재배치 (§4 번역 초안)

### 3.5 `tests/frontend_wiring.rs`

- 신규: About 라우팅 분리·`show_about_window` 존재·`about.html` 존재·`about.*` 키 5언어 집합·settings.html `#about`/`#version-btn` 부재·`AppMeta.app_path`. 기존 테스트 회귀 확인.

### 3.6 명세 2건 — `menu-bar-and-lifecycle.md` §3.3(About 행 → 별도 About 창)·`preferences-ui.md` §4.4(About 행 제거·이동)·README 갈라짐 표 등재 여부(원본도 About 창 있음 — 재현이므로 등재 불필요 가능, 상급 판단)

### 3.7 `manual-verification.md` — About 창 검증 절차

---

## 4. i18n 번역 초안 (5개 언어 — 초안, 상급 리뷰 교정)

| 키 | en | ko | zh | es | ja |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `about.title` | `About Ultrakey` | `Ultrakey 정보` | `关于 Ultrakey` | `Acerca de Ultrakey` | `Ultrakey について` |
| `about.app_name` | `Application` | `앱 이름` | `应用名称` | `Aplicación` | `アプリ名` |
| `about.version` | `Version` | `버전` | `版本` | `Versión` | `バージョン` |
| `about.author` | `Developer` | `제작자` | `开发者` | `Desarrollador` | `開発者` |
| `about.website` | `Website` | `웹사이트` | `网站` | `Sitio web` | `ウェブサイト` |
| `about.contact` | `Contact` | `연락 방법` | `联系方式` | `Contacto` | `連絡先` |
| `about.issues` | `Report an issue` | `이슈 등록` | `报告问题` | `Informar de un problema` | `問題を報告` |
| `about.check_for_updates` | `Check for Updates…` | `업데이트 확인…` | `检查更新…` | `Buscar actualizaciones…` | `アップデートを確認…` |
| `about.app_path` | `App location` | `앱 설치 위치` | `应用安装位置` | `Ubicación de la aplicación` | `アプリの場所` |
| `about.log_path` | `Log file location` | `로그 파일 위치` | `日志文件位置` | `Ubicación del archivo de registro` | `ログファイルの場所` |
| `about.outside_bundle` | `Running outside an app bundle` | `앱 번들 밖에서 실행 중` | `在应用包外运行` | `Ejecutándose fuera de un paquete de aplicación` | `アプリバンドルの外で実行中` |

⚠️ URL 3종(웹사이트·연락·이슈) 결정 후 라벨 값 반영.

---

## 5. 테스트 계획

### 5.1 자동화

1. 신규 — About 라우팅 분리 / `show_about_window` 존재
2. 신규 — `about.html` 존재·커맨드
3. 신규 — `about.*` 키 5언어 집합
4. 신규 — settings.html `#about`·`#version-btn` 부재(제거 시)
5. 신규 — `AppMeta.app_path`
6~8. 기존 회귀 — 필수 메뉴 id·General 섹션·F-13

### 5.2 수동

1. 트레이 `About` → 별도 About 창
2. 정보 행 표시
3. `Check for Updates…` 동작(.app 번들)
4. General 탭 About 행 부재, Advanced 에 번들 ID·설정 경로
5. About 창 닫고 다시 열기(#88 정합)
6. 5언어 표시

---

## 6. 리스크 / 미확인

| # | 항목 | 성격 | 대응 |
| :--- | :--- | :--- | :--- |
| 1 | #88 창 정책과 정합 | 사실(병행) | show_about_window 는 #88 옵션 A/B 에 종속. #88 먼저 확정 시 동일 적용. 같은 PR 여부는 호출 세션 판단 |
| 2 | 설치 위치 획득 | `(추정)` | `current_exe()` → .app 경로 유도. 번들 밖은 `outside_bundle` |
| 3 | **URL 3종 결정** | 사실 | **호출 세션이 결정. 구현 위임 전 확정 필수** |
| 4 | `about.html` 부트스트랩 | `(추정)` | `about_bootstrap` 신규 권장(AppMeta+문자열만) |
| 5 | `check_for_updates` 래퍼 | `(추정)` | `#[tauri::command]` 래퍼 필요 |
| 6 | `open_url` 래퍼 | `(추정)` | 기존 `bundle::open_url` 커맨드 여부 확인 |
| 7 | `settings.general.about.*` 참조 테스트 | 미확인 | 구현 시 grep |
| 8 | `#version-btn` | 설계 | 초안은 제거 권장 — 상급 판단 |
| 9 | 번역 품질 | 미확인 | 상급 리뷰 교정 |
| 10 | About 창 크기 | `(추정)` | 420×360pt + `resizable(false)` 권장 — 상급 판단 |

---

## 7. 구현 순서 (위임 지시서)

1. **호출 세션: URL 3종 결정** — 선행 조건
2. i18n 5개 — 신규 키 + log_path 재배치
3. `about.html` 신규
4. `main.rs` — 라우팅 분리 + show_about_window + AppMeta.app_path + 래퍼
5. `settings.html` — About 행 제거 + Advanced 이동
6. `frontend_wiring.rs` — 신규 테스트 + 회귀
7. 명세 2건 + manual-verification.md
8. `cargo test` + 수동 검증

---

## 8. 상급 리뷰 요청 사항

1. D1 별도 HTML+창 채택
2. D2 정보 목록 + **URL 3종 결정(호출 세션)**
3. D3 About 행 제거 + 번들 ID/설정 경로 Advanced 이동
4. D3-보조 `#version-btn` 제거 vs 유지+About 열기 (초안: 제거)
5. D4 업데이트 버튼 재사용 + Sparkle 위임
6. D6 `about.` 네임스페이스 + `menu.check_for_updates` 재사용 여부
7. #88 정합 · 같은 PR/별도 PR
8. README 갈라짐 표 등재 여부(재현이므로 불필요 가능)
9. About 창 크기·resizable(false)

---

## 9. ⭐ 상급 리뷰 반영 (2026-09-02, `ultrakey-review`)

> 판정: **조건부 통과** — URL 3종 확정 전 위임 금지(계획이 이미 게이트). 추가 재작업 없음.

### 판정 요지
- **D1(별도 `about.html` + Event Viewer 빌더 패턴) 승인.** **D3(이동: 로그 위치→About, 설치 위치 신규, 번들 ID·설정 경로→Advanced) 승인.** **D4 승인.** 창 크기 420×360·`resizable(false)` 승인(`(추정)` 유지).
- ⭐ **핵심 발견**: `menu-bar-and-lifecycle.md:155` 이 이미 "`About` — **재현.** `About.storyboardc` 존재로 About 창이 있음을 확인(실측)… 클론은 최소 정보 창으로 재현"이라 규정 — 이 작업은 **갈라짐이 아니라 명세에 뒤처진 구현을 맞추는 정정**이다. 4987 결정이 명세에서 이탈해 있던 것.
- **README 갈라짐 표 등재 불필요 확인** — 재현이다. 정정 기록으로 남기고 원본 실측(`About.storyboardc`)은 보존.

### 반영 수정 목록 (호출 세션 확정 사항 포함)
| # | 위치 | 수정 |
| :--- | :--- | :--- |
| 1 | **URL 3종 (웹사이트·연락·이슈)** | ✅ **확정 (2026-09-02, 호출 세션)**: **레포 URL 사용** — 웹사이트·연락·이슈 등록 전부 `https://github.com/johnpark-bin/Ultrakey` 로 연결. 연락 방법 라벨은 "GitHub Issues" 로 표시(별도 이메일 없음). 초안 §4 번역 표의 갱신할 라벨 값: `about.website` → `GitHub` / `about.contact` → `GitHub Issues`(번역 없이 고유 브랜드 유지) |
| 2 | **D3 `#version-btn`** | ⭐ **상급 리뷰 권장으로 변경: 제거 → "유지 + About 창 열기"** — ① 원본 버전 버튼은 실측 버튼이고 `preferences-ui.md` §9 Q12 가 "About 창 오픈 추정" — About 창 신설이 Q12 를 원본 추정 방향으로 해소 ② 원본도 트레이 `About` + 버전 버튼 이중 진입점 ③ 기각 근거 "중복 진입점" 약함. **확정 주체는 호출 세션**(본 계획은 상급 권장을 정본으로 반영) |
| 3 | D6 ⚠️ | `settings.general.about.*` 참조 테스트 — **부재 확인**(frontend_wiring.rs 실측, 참조는 settings.html:1387-1389·2922 뿐). 기록하고 종료 |
| 4 | D2 `authors` | `(추정)` 유지 — `package_info().authors` 구현 시 확인, 실패 시 하드코딩 폴백 |

---

## 10. ⭐ 이슈 #96 부록 (2026-09-03) — About 창 UI 개선

> 이 계획으로 만든 About 창에 대한 사용자 피드백("너무 못생겼다 · 스크롤바 없이 본문
> 전체가 보여야 한다 · 앱 아이콘이 노출돼야 한다")을 반영한 후속 작업의 기록.
> 본문의 D1~D6(별도 창 · 정보 구성 · 이동 · 업데이트 버튼 · 라우팅 · #88 상주)은
> **그대로 유지**되고, 표면만 다시 만들었다.

| 항목 | 결정 | 근거 · 기각한 대안 |
| :--- | :--- | :--- |
| **앱 아이콘 노출** | 히어로 상단에 **정본 `assets/app-icon/ultrakey.svg` 의 인라인 사본**(`currentColor` 선화, 64px) | `docs/dev/icons.md` §2.4 관례(온보딩 모달과 동일) — 라이트/다크 자동 대응 · 네트워크/CSP 경로 없음 · 로드 실패로 아이콘만 조용히 사라지는 경로 없음. ⛔ 기각: ① `icons/*.png` 를 `ui/` 에 복사해 `<img>` 참조 — 정본 파이프라인(`generate-icons.sh`) 밖 래스터 사본의 이중 관리 ② Tauri asset 프로토콜 — capability·CSP 배선 증가, 새로고침 경로 취약 ③ CSS 타일 재현 — 정본 파이프라인에 없는 시각 어휘 발명 |
| **레이아웃** | 히어로(아이콘 · 앱 이름 · 버전) + 정보 행 6종 + 하단 중앙 버튼 | 앱 이름·버전이 히어로로 올라가며 중복되던 라벨 행 2종 제거 — 카탈로그 키 `about.app_name`·`about.version` 도 함께 제거(미사용 키 방치 금지, D6 의 log_path 재배치 선례). 회귀 아님: 정보 자체(이름·버전 포함)는 전부 계속 표시된다 |
| **창 크기** | **540×480pt 고정 · `resizable(false)` 유지** — 5개 언어 × 번들 내·외 2상태(총 10조합)을 헤드리스 렌더로 실측, 가장 높은 조합(에스파냐어 + 번들 밖 안내, 466px)에 여백을 더함 | 본문은 세로 중앙 정렬(`margin: auto` — 넘칠 때 상단이 잘리는 `justify-content: center` 대신). ⛔ 기각: ① JS 콘텐츠 측정 후 `setSize` — 로딩 시 창 크기 깜빡임, #88 상주 창의 재측정 복잡도. 내용은 언어·번들 여부만 타는 고정 행 구성이라 정적 실측으로 충분 ② 옛 420×360 유지 — 피드백의 원인(스크롤바) 자체가 해결되지 않음 |
| **정합 가드** | `frontend_wiring.rs` 신규: `about_html_의_아이콘_path는_정본_svg_와_같다`(인라인 사본 정합) · `about_창은_스크롤바_없는_고정_크기로_열린다`(540×480 + `resizable(false)`) | 기존 #87 배선 테스트 8종 전부 유지(회귀 없음) |