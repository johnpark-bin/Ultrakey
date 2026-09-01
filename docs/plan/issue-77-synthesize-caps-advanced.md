# 이슈 #77 — Synthesize Caps Lock Remap + 진단 툴을 설정 Advanced 섹션으로 이동 (위험 고지 포함) — 계획 초안

> **성격**: `docs/plan/` 은 구현 위임 전 **계획 문서**를 두는 자리다. 이 문서는 중급(계획 초안)이 작성하고 상급(호출 세션)이 리뷰·교정해 **확정**한 판이 된다. "결정 목록(D1~Dn)"이 정본이고, §3 의 작업 분해와 §4 의 i18n 번역이 구현 위임(`ultrakey-implement`)의 지시서다.
>
> **상태**: ⬜ **초안 — 상급 리뷰 대기.** 이 문서의 모든 결정은 초안이며, 상급(호출 세션) 리뷰로 확정된다. 리뷰가 반영된 뒤 구현 위임이 시작된다.
>
> **범위 제약**: 이 작업은 **UI 재배치 + 위험 고지 + i18n** 이다. 저장 키(`presets.synthesizeCapsLockRemap`)·엔진 동작(`reconfigure_engine`·경로 B 설치/해제)·`detect_conflict` 충돌 감지 로직은 **건드리지 않는다**(D5). 트레이 메뉴의 `Advanced ▸ Relaunch` 항목도 이 작업의 범위 밖이다(유지).

---

## 1. 요약

현재 `Synthesize Caps Lock Remap` 토글은 **트레이 메뉴 `Advanced ▸` 서브메뉴에만** 있다(`main.rs:4997-5002` CheckMenuItem 생성, `main.rs:5230` 이벤트 라우팅, `main.rs:5277-5354` 토글 처리). 사용자 요구는 ① 이 옵션을 **일반 설정 항목**으로 옮기고 ② 위험성 기능이므로 **주의 고지 문구**를 설정 화면에 넣고 ③ **기본 숨김 → 펼치면 나타나는 advanced 항목** 형태로 표현하며 ④ **진단 툴(Event Viewer 등)도 같은 Advanced 항목 안으로** 위치시키는 것이다.

**핵심 실측 사실 (이 계획의 토대)**:

1. **설정 화면 체크박스 추가만으로 엔진 반영까지 기존 경로가 완결된다.** `settings_set`(`main.rs:1936`) → `settings_set_preset`(`main.rs:3088`) → `apply_preset_setting`(`main.rs:1608` — `PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP` 분기 존재) → `reconfigure_engine(force_reset=true)`(`main.rs:3131`) 경로가 이미 synthesize 키를 처리한다. 프런트의 data-key 위임 리스너(`settings.html:2948`)가 체크박스 → `settings_set` → `renderState` 로 이어진다. 즉 **백엔드 커맨드·엔진 반영 코드는 한 줄도 고칠 필요가 없다** — `settings.html` 에 체크박스 하나와 i18n 키만 추가하면 된다.
2. **트레이 메뉴 토글(`on_menu_toggle_synthesize_caps_lock_remap`, `main.rs:5277`)은 메뉴 전용 경로다.** 이 함수는 `state.presets` 메모리 정본 갱신 → `reconfigure_engine` → 저장 순서로 동작하며, `settings_set_preset` 과 **동일한 결과**를 낸다(둘 다 `presets.synthesize_caps_lock_remap` 을 바꾸고 `force_reset=true` 로 엔진 재구성). 즉 트레이 항목을 제거해도 기능 자체는 설정 화면 경로로 완전히 대체된다.
3. **`detect_conflict`(`crates/ultrakey-presets/src/conflicts.rs:56`)는 synthesize 키를 충돌 후보로 다루지 않는다** — caps lock 그룹 7종(리매핑·quick press·WASD·HJKL·home row 등)만 다룬다. synthesize 는 "경로 B 를 끄는 스위치"라 다른 프리셋과 상호 배타 관계가 아니므로, 설정 화면 체크박스가 `settings_set_preset` 을 거쳐도 충돌 대화상자가 뜰 일이 없다(기존 트레이 토글도 충돌 감지를 거치지 않았다 — 동작 동등).
4. **General 탭의 기존 "진단" 섹션**(`settings.html:1029-1039`)은 `#general-diagnostics-heading` + Event Viewer 버튼 + 로그 폴더 버튼 한 행이다. 이슈 #47 이 만든 `<hr />` + `h2.group` 섹션 패턴 안에 있다.
5. **`<details>` 요소는 현재 settings.html 어디에도 없다** — 이 저장소의 접이식 UI 선례는 `#about`(`settings.html:948`, `hidden` 속성 + 버전 버튼 클릭으로 토글)와 `renderHiddenNote`(② 숨김 패턴)뿐이다. `<details>` 는 **새 컨트롤 타입**이 된다.

**이 작업이 하는 일**: ① General 탭에 "Advanced" 섹션(`<details>` 접이식) 신설 ② 그 안에 Synthesize Caps Lock Remap 체크박스 + 위험 고지 문구 ③ 기존 진단 섹션(Event Viewer·로그 폴더 버튼)을 Advanced 안으로 이동 ④ 트레이 메뉴에서 Synthesize 항목 제거 ⑤ i18n 신규 키 5개 언어 ⑥ 명세 3건 갱신. **신규 저장 키 0개, 백엔드 커맨드 변경 0개, 엔진 코드 변경 0개.**

---

## 2. 결정 목록 D1~Dn

### D1 — Advanced 섹션의 UI 형태: **`<details>`/`<summary>` 접이식 (기본 접힘)**

**결정**: General 탭 최하단(진단 섹션 자리)에 `<details id="general-advanced">` + `<summary>` 헤딩 형태의 접이식 섹션을 신설한다. 기본 상태는 **접힘**(`open` 속성 없음). 펼치면 그 안에 ① Synthesize Caps Lock Remap 체크박스 + 위험 고지 ② 진단 버튼 행(Event Viewer·로그 폴더)이 나타난다.

**근거**:
- 사용자 요구 ③ "기본 숨김 → 사용자가 펼치면 나타나는 advanced 항목"이 `<details>` 의 기본 동작과 정확히 일치한다. `<details>` 는 WebKit 네이티브 접이식으로, `toggle` 이벤트·`open` 속성·ARIA(`role="group"` 자동)가 기본 제공되어 접근성 구현 비용이 가장 낮다.
- F-09 §3.8 종속 표현 3종 분류상 이 항목은 **② 숨김(hidden)** 의 확장이다 — "항상 존재하되 접혀 있다"는 것은 DOM 에는 있지만 시각·AX 트리에서 내용이 드러나지 않는 ② 의 변형으로 볼 수 있다. ① dimmed(자리 차지 + 흐려짐)와 ③ 문장 중간 삽입은 해당하지 않는다. `<details>` 는 ② 를 "조건부 렌더링(DOM 제거/삽입)" 대신 "CSS/네이티브 접기"로 구현하는 것이며, 기존 `renderHiddenNote`(② 숨김)와 같은 목적(고급 항목을 기본 화면에서 감춤)을 더 싸게 달성한다.
- **기존 선례**: `#about`(`settings.html:948`)가 `hidden` 속성 + 버튼 토글로 접이식을 이미 구현했다. `<details>` 는 그 패턴의 표준화된 형태다. `renderHiddenNote` 도 ② 숨김의 선례다.
- **저장 영향 없음**: 접힘 상태는 저장하지 않는다(세션 상태). F-15 "부재 = 기본값" 규약에 새 키를 만들지 않는다. (접힘 상태 영속은 D1 기각 대안 참고.)

**기각한 대안**:
- **일반 토글 + 조건부 노출(체크박스로 펼침)**: "Advanced 항목 보기"라는 토글 체크박스가 하나 더 생기고, 그 값이 저장 키가 된다. 사용자 요구는 "펼치면 나타나는" 것이지 "설정으로 켜야 보이는" 것이 아니므로, 저장 키를 만들지 않는 `<details>` 가 요구에 더 정확하다. 또한 이 토글은 F-09 §3.8 3종 어디에도 속하지 않는 **새 종속 표현**을 만든다.
- **기존 충돌/확인 오버레이 패턴 활용**: `#conflict-overlay`·`#confirm-overlay`(`settings.html:1049·1067`)는 **모달 대화상자**다. Advanced 섹션은 상시 존재하는 영역이므로 모달은 부적합 — 오버레이는 "일회성 확인"에만 쓰고, 상시 노출 영역에는 쓰지 않는다(위험 고지의 역할 구분은 D3).
- **접힘 상태를 `ui.advancedExpanded` 로 영속**: 저장 키 1개가 늘고, F-15 §3.1.1 "컨트롤 단위 즉시 write-through" 대상이 하나 더 생긴다. 접힘 상태는 사용자 세션 편의일 뿐 설정이 아니므로 영속할 가치가 없다. (상급이 영속을 원하면 별도 결정으로 추가 — 초안은 비영속 권장.)

### D2 — 트레이 메뉴 항목: **제거**

**결정**: `main.rs` 의 `SYNTHESIZE_CAPS_REMAP` CheckMenuItem(`main.rs:4997-5004`)과 `advanced_menu` 구성(`main.rs:5015-5021`)에서 synthesize 항목을 **제거**한다. `Advanced ▸` 서브메뉴에는 `Relaunch` 만 남는다. `menu_ids::SYNTHESIZE_CAPS_REMAP` 상수·`handle_menu_event` 의 라우팅(`main.rs:5230`)·`on_menu_toggle_synthesize_caps_lock_remap`(`main.rs:5277-5354`)도 함께 제거한다. i18n 키 `menu.advanced.synthesize_caps_remap`(5개 언어)도 삭제한다.

**근거**:
- 사용자 요구 ① "트레이 메뉴에서 일반 설정 항목으로 이동"은 **이동**이지 복제가 아니다. 트레이에 유지하면 같은 설정이 두 표면에 존재해 **두 경로가 어긋날 위험**이 생긴다 — 이 프로젝트는 이미 이슈 #19 에서 "메뉴 토글이 엔진에 반영되지 않는" 어긋남을 겪었고, `on_menu_toggle_synthesize_caps_lock_remap` 의 주석(`main.rs:5283-5294`)이 그 교훈을 담고 있다. 표면을 하나로 줄이는 것이 어긋남 자체를 구조적으로 제거한다.
- **회귀 분석**: 트레이 항목 제거로 잃는 것은 "메뉴바에서 바로 토글"뿐이다. 기능 자체(경로 B 설치/해제)는 설정 화면 체크박스가 `settings_set_preset` 경로로 동일하게 수행한다(§1 실측 1·2). `Advanced ▸` 서브메뉴는 `Relaunch` 하나만 남아도 유효하다 — `menu-bar-and-lifecycle.md` §3.3 의 실측 메뉴 구성은 **원본**의 것이고, 클론은 이미 로깅·뷰어 항목을 기각해 축소된 구성을 쓰고 있다(§3.3 표 "기각" 행). synthesize 제거는 그 축소의 연장이다.
- **원본과의 갈라짐**: 원본은 이 항목을 메뉴에 둔다(실측: AX 트리). 클론이 설정 화면으로 옮기는 것은 **의도된 갈라짐**이며, `README.md` 의 "원본과 갈라지는 지점" 표에 등재해야 한다(§5 명세 갱신).

**기각한 대안**:
- **트레이 유지 + 설정 화면 추가(복제)**: 두 표면이 같은 저장 키를 읽고 쓰므로 동작은 같지만, ① 사용자 요구("접근성 불량으로 설정으로 이동")를 반만 충족 ② 표면 2개 = 동기화 유지 비용 ③ 이슈 #19 류 어긋남 재발 위험. 유지할 근거(예: "메뉴바에서 빠른 토글이 필요하다")가 사용자 요구보다 우선하지 않는다.
- **트레이 항목을 비활성화(disabled)로 남기고 설정 화면으로 안내**: "이동"의 중간 단계로 보이지만, 비활성 항목은 클릭해도 아무 일도 안 일어나 UX 가 나쁘고, 다음 릴리스에서 제거할 때 또 한 번의 변경이 필요하다. 한 번에 제거가 싸다.

### D3 — 위험 고지 문구의 위치·형태: **펼침 시 notice (인라인 경고 문구) — 확인 대화상자는 쓰지 않는다**

**결정**: 위험 고지는 **Advanced 섹션을 펼쳤을 때 보이는 인라인 경고 문구**로 구현한다. Synthesize 체크박스 **바로 아래**에 `class="hint warn"`(기존 `renderHiddenNote` 의 `"warn"` 스타일, `settings.html:2447-2448` 선례) 문구를 둔다. 체크박스 토글 시 **추가 확인 대화상자는 띄우지 않는다**.

**역할 구분** (세 가지 고지 수단이 각자 다른 일을 한다):
| 수단 | 역할 | 채택 |
| :--- | :--- | :--- |
| 펼침 시 인라인 notice | **상시 고지** — "이 기능이 무엇을 하고 왜 위험한지"를 기능을 조작하기 전에 읽게 한다 | ✅ 채택 |
| 토글 시 확인 대화상자 | **일회성 경고** — "지금 이 동작을 실행할 것인가"를 매번 묻는다 | ❌ 기각 (아래) |
| 옵션 옆 인라인(체크박스 라벨 옆) | **축약 고지** — 한 줄로 요약 | ❌ 기각 (아래) |

**근거**:
- 이 기능의 위험은 **토글 순간**이 아니라 **켜진 상태가 지속되는 동안** 존재한다(경로 B 커널 매핑이 전역에 설치·유지됨). 확인 대화상자는 "켜는 순간"만 막고, 켜둔 채로 며칠 지난 사용자에게는 아무것도 알리지 못한다. **상시 보이는 인라인 notice** 가 지속 위험에 맞는 고지 형태다.
- 기존 선례: `renderHiddenNote("presets-delete-warning", …, "warn")`(`settings.html:2447`)이 "진짜 backspace 를 낼 수단이 사라진 상태"를 `warn` 스타일로 상시 고지한다 — **같은 패턴**이다. `#launch-on-login-requires-approval`(`settings.html:946`)도 조건부 안내 행의 선례다.
- **확인 대화상자 기각 근거**: ① 이 저장소의 확인 오버레이(`#confirm-overlay`)는 "커맨드 1개를 실행할지 말지"의 2지선다에만 쓰인다(이슈 #46 D7 주석, `settings.html:1061-1066`) — 체크박스 토글은 그 성격이 아니다. ② `settings_set_preset` 경로는 충돌 감지가 없어(§1 실측 3) 대화상자를 끼우려면 **새 프런트 상태 기계 + 새 커맨드**가 필요해 이 작업의 "UI 재배치" 범위를 넘는다. ③ 원본도 이 토글에 확인 대화상자를 두지 않는다(실측 근거 없음 — `(추정)`).
- **옵션 옆 인라인 기각 근거**: 체크박스 라벨 옆에 위험 문구를 붙이면 라벨이 길어지고, F-09 §3.8 ③ "문장 중간 삽입"과 혼동될 수 있다. notice 는 체크박스 **아래** hint 자리에 두는 것이 이 저장소의 기존 패턴(`hint why` 문구들)과 일치한다.

**문구 초안 (en, 최종 번역은 §4)**:
> "When enabled, Ultrakey installs a system-wide kernel-level HID mapping (caps lock → F18) instead of synthesizing events. This affects every keyboard and persists until disabled or Ultrakey restarts. Only enable this if you understand the trade-off."

### D4 — 진단 툴의 위치: **기존 진단 섹션을 Advanced 안으로 이동 (버튼·커맨드·i18n 키는 그대로)**

**결정**: `settings.html:1029-1039` 의 진단 섹션(`#general-diagnostics-heading` + Event Viewer 버튼 + 로그 폴더 버튼 + hint)을 **Advanced `<details>` 안으로 통째로 이동**한다. 버튼 id·커맨드(`open_event_viewer`·`open_log_folder`)·이벤트 리스너(`settings.html:3055·3064`)·i18n 키(`settings.general.diagnostics`·`event_viewer`·`open_log_folder`·`event_viewer.hint`)는 **하나도 바꾸지 않는다** — DOM 위치만 옮긴다.

**근거**:
- 사용자 요구 ④ "진단 툴(Event Viewer 등)도 이 Advanced 항목 안으로 위치"가 그대로다. 진단 툴은 일반 사용자가 매일 쓰는 기능이 아니므로 "고급" 분류가 자연스럽다.
- **이동 방식**: `<details>` 블록 안에 기존 마크업을 그대로 옮겨 넣는다. `applyStrings()`(`settings.html:1278-1286`)의 배선은 id 기반이라 위치와 무관하게 그대로 동작한다. `renderState`·`renderHiddenNote` 도 id 기반이라 무영향.
- **섹션 헤딩 처리**: 기존 `#general-diagnostics-heading`(`h2.group`)은 `<details>` 안에서 `<summary>` 와 중복되므로 **제거**하고, `<summary>` 가 "Advanced" 헤딩을 겸한다. 진단 버튼 행은 `<details>` 안에서 `h2.group` 없이 바로 시작한다(Advanced 안의 하위 그룹이 아니라 단순 나열). `settings.general.diagnostics` i18n 키는 삭제한다(§4).
- **배치 순서**: Advanced 섹션은 General 탭 **최하단**(기존 진단 섹션 자리)에 둔다. 탭 내부 순서는 언어 → Startup & menu bar → License → 설정 파일 → **Advanced** 가 된다. (기존 진단이 마지막이었으므로 순서 변화가 없다.)

**기각한 대안**:
- **진단 섹션은 그대로 두고 Advanced 에 synthesize 만 넣기**: 사용자 요구 ④를 충족하지 못한다.
- **진단을 Advanced 안에 넣되 헤딩 유지**: `<details>` 안에 `h2.group` 헤딩이 또 있으면 헤딩 2중(Advanced + 진단)이 되어 시각·판독기 양쪽에서 중복이다. `<summary>` 가 그룹 제목을 겸하는 것이 `<details>` 의 표준 사용법이다.
- **진단 버튼을 Advanced 밖에 남기고 synthesize 만 Advanced 로**: 요구 ④와 어긋난다.

### D5 — 저장 키·엔진 동작: **변경하지 않는다 (UI 재배치에 국한)**

**결정**: `presets.synthesizeCapsLockRemap` 저장 키(`keys.rs:103`)·`PresetSettings::synthesize_caps_lock_remap` 필드(`ultrakey-presets/src/settings.rs:187`)·`reconfigure_engine`·경로 B 설치/해제 로직·`detect_conflict` — 전부 **그대로 둔다**. 이 작업은 UI 표면 이동만 한다.

**근거**:
- §1 실측 1·2 가 보여주듯, 설정 화면 체크박스는 기존 `settings_set_preset` 경로로 **이미 완결된** 엔진 반영을 얻는다. 저장 키를 바꾸면 마이그레이션(F-15 §5)이 필요해지고, `manual-verification.md` 의 기존 검증 절차(§1466·§1559 — `synthesizeCapsLockRemap` 값 확인)가 깨진다.
- **이동 task 의 본질**: 사용자 요구는 "접근성 불량"의 해소다. 저장·엔진 계약을 건드리면 이 작업의 리스크가 "UI 재배치"에서 "엔진 동작 변경"으로 커진다. 별도 이슈로 분리하는 것이 맞다.
- **경로 B 전역 설치의 위험성 자체는 이 작업이 해결하지 않는다** — D-1 디바이스 한정화(`README.md` M6 절, `per-device-settings.md` §3.6 규칙 2)는 별도 작업이다. 이 작업은 그 위험을 **고지**만 하고, 해결은 후속 이슈로 남긴다(§6).

**기각한 대안**:
- **저장 키를 `general.synthesizeCapsLockRemap` 으로 이동**: 키 이름 변경 = F-15 마이그레이션 함수 + `keys::all()` 화이트리스트 갱신 + `manual-verification.md` 갱신 + 기존 사용자 설정 무효화. UI 이동에 비해 비용이 과도하다. synthesize 는 **Presets(캡스락 프리셋)의 동작을 바꾸는 스위치**이므로 `presets.*` 네임스페이스가 의미상으로도 맞다.
- **체크박스 대신 별도 커맨드 경로 신설**: `settings_set_preset` 이 이미 처리하므로 새 커맨드는 중복 코드다.

### D6 — i18n: **신규 키 4개 × 5언어 + 삭제 키 2개 × 5언어**

**결정**:
- **신규**: `settings.general.advanced`(섹션 제목) · `settings.general.advanced.synthesize_caps_remap`(체크박스 라벨) · `settings.general.advanced.synthesize_caps_remap.why`(위험 고지) · `settings.general.advanced.diagnostics`(Advanced 안 진단 그룹 라벨 — D4 의 헤딩 제거에 따른 대체) — **4개 × 5언어(en/ko/zh/es/ja)**.
- **삭제**: `menu.advanced.synthesize_caps_remap`(D2) · `settings.general.diagnostics`(D4 — 헤딩이 `<summary>` 로 대체됨) — **2개 × 5언어**.
- **유지**: `settings.general.event_viewer`·`open_log_folder`·`event_viewer.hint`(D4 — 버튼·hint 는 그대로).

**근거**:
- 키 네이밍은 기존 평평한 카탈로그 어휘(`settings.general.*`)를 따른다. `settings.general.advanced.*` 접두사는 새 섹션의 네임스페이스다.
- `menu.advanced.synthesize_caps_remap` 삭제는 `frontend_wiring.rs` 의 메뉴 키 검사(`main_rs_의_menu_점_리터럴은_다섯_카탈로그_모두에_있다`, `main.rs:803`)와 **필수 항목 id 검사**(`main.rs:824-842` — `"menu.advanced.synthesize_caps_remap"` 포함)를 함께 갱신해야 한다(§3 작업 4).
- ⚠️ **`settings.general.advanced.diagnostics` 의 필요성 재검토 필요(상급 확인 요청)**: D4 는 진단 헤딩을 제거하고 `<summary>` 가 겸한다고 정했다. 그렇다면 이 키는 불필요할 수 있다. 초안은 **두 가지 옵션**을 남긴다 — (a) `<summary>` = "Advanced" 하나로 통합(키 3개만 신규), (b) Advanced 안에 "진단" 소헤딩 유지(키 4개). **초안 권장은 (a)** — `<details>` 안에 헤딩을 또 두면 2중 헤딩이 되고, 진단 버튼 2개 + hint 1줄은 헤딩 없이도 자명하다. 상급이 (b) 를 원하면 D4 의 "헤딩 제거"를 뒤집고 이 키를 추가한다.

**기각한 대안**:
- **`menu.advanced.synthesize_caps_remap` 키를 재사용**: 키 이름이 `menu.` 접두사라 설정 화면에서 쓰면 네임스페이스가 어긋난다. `settings.general.advanced.*` 로 새로 만든다.
- **`settings.general.advanced` 대신 `settings.general.advanced_heading` 류**: 기존 헤딩 키(`settings.general.startup` 등)는 접미사 없는 평평한 형태다. 같은 어휘를 따른다.

---

## 3. 작업 분해 (파일 단위)

> 구현 순서는 §5 의 "구현 순서" 를 따른다. 각 파일의 변경은 **이 문서의 결정(D1~D6)이 정본**이다.

### 3.1 `apps/ultrakey-app/ui/settings.html` — 마크업 + 배선

1. **General 탭 최하단**(기존 진단 섹션 자리, `settings.html:1026-1039`)에 `<details id="general-advanced">` 블록 신설:
   - `<summary id="general-advanced-summary"></summary>` — `applyStrings()` 에서 `settings.general.advanced` 로 채움.
   - 안에 ① Synthesize 체크박스 행: `<label class="checkbox-row"><input type="checkbox" id="synthesize-caps-remap" data-key="presets.synthesizeCapsLockRemap" /><span id="synthesize-caps-remap-label"></span></label>` + `<p class="hint warn" id="synthesize-caps-remap-why"></p>` ② 진단 버튼 행(기존 `#open-event-viewer-btn`·`#open-log-folder-btn` + `#general-event-viewer-hint` 를 **그대로 이동**).
   - 기존 `#general-diagnostics-heading`(`h2.group`)은 **제거**.
2. **`applyStrings()`**(`settings.html:1278-1286`) 갱신:
   - `general-diagnostics-heading` 배선 제거.
   - `general-advanced-summary`·`synthesize-caps-remap-label`·`synthesize-caps-remap-why` 배선 추가.
   - `open-event-viewer-btn`·`open-log-folder-btn`·`general-event-viewer-hint` 배선은 **유지**(id 기반이라 위치 무관).
3. **`renderState()`**(`settings.html:2761`)에 synthesize 체크박스 렌더 1줄 추가: `$("synthesize-caps-remap").checked = state.presets.synthesizeCapsLockRemap;` — `renderPresets()`(`settings.html:2407`)에 넣을지 `renderState` 의 General 블록(`settings.html:2805-2808`)에 넣을지는 구현 판단. **권장: `renderPresets()`** — 저장 키가 `presets.*` 이므로 상태 소스가 `state.presets` 다.
4. **이벤트 리스너**: 신규 체크박스는 `data-key` 속성만으로 공용 위임 리스너(`settings.html:2948`)가 처리한다 — **별도 리스너 불필요**. 기존 `open-event-viewer-btn`·`open-log-folder-btn` 리스너(`settings.html:3055·3064`)는 그대로.
5. **위험 고지 문구**: `synthesize-caps-remap-why` 는 `applyStrings()` 에서 `settings.general.advanced.synthesize_caps_remap.why` 로 채운다. `class="hint warn"` — `renderHiddenNote` 의 `"warn"` 스타일(`settings.html:2447-2448`)과 같은 시각.

### 3.2 `apps/ultrakey-app/src/main.rs` — 트레이 메뉴에서 제거

1. `build_normal_menu`(`main.rs:4997-5004`)에서 `synth_caps_item` 생성 제거, `advanced_menu`(`main.rs:5015-5021`)를 `&[&relaunch_item]` 로 축소.
2. `menu_ids::SYNTHESIZE_CAPS_REMAP` 상수(`main.rs:106`) 제거.
3. `handle_menu_event` 의 라우팅(`main.rs:5230`) 제거.
4. `on_menu_toggle_synthesize_caps_lock_remap`(`main.rs:5277-5354`) **전체 제거** — 이 함수는 메뉴 전용 경로이며 설정 화면 경로(`settings_set_preset`)가 대체한다(§1 실측 2).
5. `synthesize_caps_lock_remap_enabled` 헬퍼(`main.rs:5073-5077`) 제거 — 호출처가 사라진다.
6. ⚠️ **`main.rs:6976` 의 테스트 코드**(`store.set(keys::PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP, &true)`)는 **유지** — 저장 키 자체는 살아있으므로(§1 실측 1) 이 테스트는 여전히 유효하다. 다만 그 테스트가 메뉴 토글을 거쳐 호출되는지 확인 필요(구현 시 판단 — 메뉴 함수 제거로 깨지면 `settings_set_preset` 경로로 대체).

### 3.3 `resources/i18n/{en,ko,zh,es,ja}.json` — 5개 언어

- **삭제**: `menu.advanced.synthesize_caps_remap`(각 파일 31행) · `settings.general.diagnostics`(각 파일 138행).
- **신규**: `settings.general.advanced` · `settings.general.advanced.synthesize_caps_remap` · `settings.general.advanced.synthesize_caps_remap.why` (+ D6 옵션 (b) 채택 시 `settings.general.advanced.diagnostics`).
- 번역 초안은 §4.

### 3.4 `apps/ultrakey-app/tests/frontend_wiring.rs` — 정적 배선 검사 갱신

1. `main_rs_가_정상_메뉴의_필수_항목_id를_전부_선언한다`(`main.rs:824-842`)에서 `"menu.advanced.synthesize_caps_remap"` 제거.
2. `main_rs_의_menu_점_리터럴은_다섯_카탈로그_모두에_있다`(`main.rs:803`)는 자동 — `main.rs` 에서 `menu.advanced.synthesize_caps_remap` 리터럴이 사라지므로 카탈로그에서도 삭제하면 통과.
3. **신규 테스트 추가** (기존 `settings_html_의_자동_업데이트_체크박스는_활성이다`(`main.rs:849`) 패턴):
   - `settings.html` 에 `id="synthesize-caps-remap"` + `data-key="presets.synthesizeCapsLockRemap"` 가 있고 disabled 가 아님.
   - `<details id="general-advanced">` 가 존재하고 `open` 속성이 없음(기본 접힘).
   - `settings.general.advanced.*` 신규 키 3개(또는 4개)가 5개 카탈로그 모두에 있고, 삭제 키 2개가 5개 카탈로그 모두에 없음.
   - `#general-diagnostics-heading` 이 settings.html 에 더 이상 없음.
   - `menu.advanced.synthesize_caps_remap` 이 main.rs 에 더 이상 없음.

### 3.5 명세 갱신 — 3건

1. **`docs/spec/menu-bar-and-lifecycle.md`** §3.3: `Synthesize Caps Lock Remap` 행의 "재현" 판단을 **"설정 화면으로 이동(이슈 #77)"** 으로 갱신. §3.3 메뉴 구성도 `Advanced ▸ (Relaunch)` 로 축소. §9 미해결 질문에 "원본의 실제 동작 미관찰" 항목은 유지.
2. **`docs/spec/preferences-ui.md`** §4.4(General 탭): Advanced 섹션(`<details>`) 추가, synthesize 체크박스 + 위험 고지, 진단 섹션 이동을 반영. §3.8 종속 표현 3종에 **"④ 접이식(advanced)"** 를 추가할지 상급 판단 필요 — 초안은 **추가 권장**(`<details>` 는 ② 숨김의 변형이지만 "사용자가 펼치면 나타난다"는 상호작용이 ② 와 다르다).
3. **`docs/spec/README.md`** "원본과 갈라지는 지점" 표: **"`Advanced ▸ Synthesize Caps Lock Remap` 을 설정 화면 Advanced 섹션으로 이동"** 행 추가(원본은 메뉴에 유지 — 실측).

### 3.6 `docs/dev/manual-verification.md` — 검증 절차 갱신

- synthesize 토글 검증 절차를 **설정 화면 경로**(General 탭 → Advanced 펼침 → 체크박스)로 갱신. `settings.json` 의 `synthesizeCapsLockRemap` 값 확인 절차는 유지(저장 키 불변).

---

## 4. i18n 번역 초안 (5개 언어)

> ⚠️ **번역은 초안이다.** 상급 리뷰에서 교정한다(이슈 #47 계획의 §4 ko 번역 교정 선례). 특히 위험 고지 문구는 **기능의 위험성을 정확히 전달**해야 하므로, 번역보다 원문(en)의 정확성이 우선이다.

| 키 | en | ko | zh | es | ja |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `settings.general.advanced` | `Advanced` | `고급` | `高级` | `Avanzado` | `詳細` |
| `settings.general.advanced.synthesize_caps_remap` | `Synthesize Caps Lock Remap` | `Caps Lock 리매핑 합성` | `合成大写锁定重映射` | `Sintetizar reasignación de Bloq Mayús` | `Caps Lock リマップを合成` |
| `settings.general.advanced.synthesize_caps_remap.why` | `When enabled, Ultrakey installs a system-wide kernel-level HID mapping (caps lock → F18) instead of synthesizing events. This affects every keyboard and persists until disabled or Ultrakey restarts. Only enable this if you understand the trade-off.` | `켜면 Ultrakey 가 이벤트 합성 대신 시스템 전역 커널 HID 매핑(caps lock → F18)을 설치합니다. 모든 키보드에 적용되며, 끄거나 Ultrakey 를 재시작할 때까지 유지됩니다. 이 트레이드오프를 이해한 경우에만 켜세요.` | `启用后，Ultrakey 将安装系统级内核 HID 映射（大写锁定 → F18），而不是合成事件。这会影响所有键盘，并持续到禁用或重启 Ultrakey。仅在您理解此权衡时启用。` | `Cuando está activado, Ultrakey instala una asignación HID a nivel de kernel en todo el sistema (Bloq Mayús → F18) en lugar de sintetizar eventos. Afecta a todos los teclados y persiste hasta que se desactive o se reinicie Ultrakey. Actívelo solo si comprende esta compensación.` | `有効にすると、Ultrakey はイベント合成の代わりにシステム全体のカーネルレベル HID マッピング（Caps Lock → F18）をインストールします。すべてのキーボードに影響し、無効化するか Ultrakey を再起動するまで持続します。このトレードオフを理解している場合のみ有効にしてください。` |
| `settings.general.advanced.diagnostics` (D6 옵션 (b) 채택 시) | `Diagnostics` | `진단` | `诊断` | `Diagnóstico` | `診断` |

**삭제 키**: `menu.advanced.synthesize_caps_remap`(기존 값: en `Synthesize Caps Lock Remap` / ko `Caps Lock 리매핑 합성` / zh `合成大写锁定重映射` / es `Sintetizar reasignación de Bloq Mayús` / ja `Caps Lock リマップを合成`) · `settings.general.diagnostics`(기존 값: en `Diagnostics` / ko `진단` / zh `诊断` / es `Diagnóstico` / ja `診断`).

---

## 5. 테스트 계획

### 5.1 자동화 테스트 (`cargo test`)

| # | 테스트 | 위치 | 검증 내용 |
| :--- | :--- | :--- | :--- |
| 1 | 메뉴 키 카탈로그 일치 | `frontend_wiring.rs` `main_rs_의_menu_점_리터럴은_다섯_카탈로그_모두에_있다` | `menu.advanced.synthesize_caps_remap` 이 main.rs 와 카탈로그 양쪽에서 사라져도 통과 |
| 2 | 필수 메뉴 항목 id | `frontend_wiring.rs` `main_rs_가_정상_메뉴의_필수_항목_id를_전부_선언한다` | 목록에서 synthesize 제거 후 통과 |
| 3 | **신규** — synthesize 체크박스 존재·활성 | `frontend_wiring.rs` 신규 | `id="synthesize-caps-remap"` + `data-key="presets.synthesizeCapsLockRemap"` + disabled 아님 |
| 4 | **신규** — Advanced `<details>` 기본 접힘 | `frontend_wiring.rs` 신규 | `<details id="general-advanced">` 존재 + `open` 속성 부재 |
| 5 | **신규** — 진단 헤딩 제거 | `frontend_wiring.rs` 신규 | `#general-diagnostics-heading` 부재, `#open-event-viewer-btn`·`#open-log-folder-btn` 존재 |
| 6 | **신규** — i18n 키 집합 | `frontend_wiring.rs` 신규 | 신규 키 3~4개가 5개 카탈로그 모두에 존재, 삭제 키 2개가 모두에 부재 |
| 7 | 기존 — `settings_set_preset` 경로 | `main.rs` 기존 테스트 | synthesize 키가 `apply_preset_setting` 분기(`main.rs:1608`)에 남아있음 — **변경 없음, 회귀 확인용** |
| 8 | 기존 — `ultrakey-presets` 단위 테스트 | `crates/ultrakey-presets/src/settings.rs` | `synthesize_caps_lock_remap` 필드·기본값·store 로드 테스트 — **변경 없음, 회귀 확인용** |

### 5.2 수동 검증 (`docs/dev/manual-verification.md` 갱신)

1. 설정 창 → General 탭 → Advanced 가 접혀 있고, 펼치면 synthesize 체크박스 + 위험 고지 + 진단 버튼 2개가 보인다.
2. synthesize 체크박스 켬 → `settings.json` 의 `presets.synthesizeCapsLockRemap` 이 `true` 로 즉시 기록되고, `hidutil property --get UserKeyMapping` 에 `caps lock → F18` 매핑이 설치된다(경로 B). 끔 → 매핑이 제거된다.
3. 트레이 메뉴 `Advanced ▸` 에 synthesize 항목이 없고 `Relaunch` 만 있다.
4. Event Viewer·로그 폴더 버튼이 Advanced 안에서도 기존과 동일하게 동작한다.
5. 5개 언어 전환 시 신규 문구가 모두 표시되고, 삭제 키가 남아있지 않다(카탈로그 무결성).

---

## 6. 리스크 / 미확인

| # | 항목 | 성격 | 대응 |
| :--- | :--- | :--- | :--- |
| 1 | **경로 B 전역 설치의 위험 자체는 이 작업이 해결하지 않는다** | 사실(실측) | D-1 디바이스 한정화(`README.md` M6, `per-device-settings.md` §3.6 규칙 2)는 별도 작업. 이 작업은 위험을 **고지**만 한다. 후속 이슈로 남긴다 |
| 2 | **`main.rs:6976` 테스트가 메뉴 토글 함수를 거치는지** | 미확인 | 구현 시 확인. 메뉴 함수 제거로 깨지면 `settings_set_preset` 경로로 대체(저장 키는 불변이므로 테스트 의도는 유지) |
| 3 | **`settings.general.advanced.diagnostics` 키 필요 여부** | 미확인(설계) | D6 옵션 (a)/(b) — 초안 권장 (a)(키 불필요). 상급 확정 필요 |
| 4 | **`<details>` 가 이 저장소의 새 컨트롤 타입** | 사실 | F-09 §3.2 컨트롤 카탈로그에 "접이식" 타입 추가 필요(§3.5 명세 갱신). `#about`(`hidden` 토글) 선례가 있으나 `<details>` 는 첫 사용 |
| 5 | **원본의 synthesize 토글에 확인 대화상자가 있는지** | 미확인 `(추정 — 없음)` | 원본 실측 근거 없음. 클론은 인라인 notice 로 결정(D3). 원본에 대화상자가 있었다면 갈라짐 표에 추가 |
| 6 | **`<details>` 접힘 상태의 AX/스크린리더 동작** | 미확인 `(추정)` | WebKit 은 `<details>`/`<summary>` 를 표준 접이식으로 노출. 실기기 VoiceOver 확인은 수동 검증에 포함 |
| 7 | **트레이 메뉴에서 synthesize 제거 시 `Advanced ▸` 서브메뉴가 1항목만 남음** | 사실 | `Relaunch` 만 남는다. 서브메뉴 자체를 없앨지는 이 작업 범위 밖(기각 — `Relaunch` 는 유지 가치, `menu-bar-and-lifecycle.md` §3.3) |
| 8 | **i18n 번역 품질** | 미확인 | §4 번역 초안은 상급 리뷰에서 교정(이슈 #47 선례) |

---

## 7. 구현 순서 (위임 지시서)

1. **i18n 5개 파일** — 삭제 2키 + 신규 3~4키(§4). 이 단계에서 `frontend_wiring` 의 카탈로그 검사가 깨지지만, 2~4 단계가 함께 머지되므로 순서상 문제없음.
2. **`settings.html`** — `<details>` 블록 + 체크박스 + notice + 진단 이동 + `applyStrings`/`renderState` 배선(§3.1).
3. **`main.rs`** — 메뉴 항목·상수·라우팅·토글 함수 제거(§3.2).
4. **`frontend_wiring.rs`** — 기존 검사 갱신 + 신규 테스트 4개(§3.4).
5. **명세 3건 + `manual-verification.md`** — 갱신(§3.5·§3.6).
6. `cargo test` 전체 + 수동 검증(§5).

---

## 8. 상급 리뷰 요청 사항

1. **D1** — `<details>` 채택과 "접힘 상태 비영속" 결정 확인.
2. **D2** — 트레이 항목 **제거**(복제 아님) 확인. 원본과의 갈라짐 등재 동의.
3. **D3** — 위험 고지를 "펼침 시 인라인 notice"로 하고 **확인 대화상자를 만들지 않는** 결정 확인. 문구 초안(§4) 교정.
4. **D6 옵션 (a)/(b)** — `settings.general.advanced.diagnostics` 키를 만들지(진단 소헤딩 유지) vs 만들지 않을지(Advanced 하나로 통합). **초안 권장 (a)**.
5. **F-09 §3.8** — 종속 표현 3종에 "④ 접이식(advanced)" 추가 여부. 초안은 추가 권장.
6. **리스크 #2** — `main.rs:6976` 테스트의 메뉴 함수 의존 여부는 구현 시 확인하되, 초안은 "저장 키 불변" 원칙으로 테스트 의도 유지 권장.

---

## 9. ⭐ 상급 리뷰 반영 (2026-09-02, `ultrakey-review`)

> 판정: **조건부 통과.** 아래 P1(반영 필수) 3건과 P2·P3 를 반영한다.

### 리뷰 판정 요지

**D1(`<details>` + 접힘 비영속) — 승인.** F-15 "부재 = 기본값"에 새 키를 만들지 않는다.

**D2(트레이 항목 제거) — 승인.** "이동이지 복제가 아니다"가 사용자 요구와 일치한다. ⚠️ 단 **등재 형식 수정**: 원본에 있는 항목의 위치를 다르게 만드는 것은 README 기준 1 문언상 갈라짐이 아니라 **이탈**이다 → D5 선례대로 "이탈로 등재 + 사용자 실경험 허가 근거(이슈 #77: '접근성이 매우 나쁨')"를 붙인다. 번호는 다음 순번 **D9**. 원본 실측 기록(menu-bar-and-lifecycle.md §3.3 행)은 지우지 않고 옆에 결정을 붙인다.

**D3(인라인 notice) — 수단은 승인, ⛔ 문구 방향은 역전.** 문구 초안은 "켜면 커널 HID 매핑 설치"로 썼지만 **사실이 반대다**: `main.rs:1088` — `if needs_alias && !presets.synthesize_caps_lock_remap { Some(KeyCode::F18) }` 즉 **켜면 커널 매핑을 제거하고 경로 A(이벤트 합성)만 쓰고, 꺼져 있어야(기본) 커널 매핑이 설치**된다. menu-bar-and-lifecycle.md §3.3 도 "켜면 경로 B 를 설치하지 않고 경로 A 만 쓴다"로 일치.

**D6 옵션 — (a) 확정.** `<summary>` 가 헤딩을 겸하고 진단 버튼 2개 + hint 는 라벨 없이 자명하다.

**§3.8 "④ 접이식" 추가 — 기각.** F-09 §3.8 종속 표현 3종은 "한 컨트롤의 값이 다른 컨트롤의 노출/활성 결정" 관계다. Advanced 접이식은 아무 설정 값에도 종속하지 않는 **사용자 조작 disclosure 위젯**이다. → §3.8 에 ④ 를 추가하지 않고 **§3.2 컨트롤 카탈로그에 "접이식 섹션(`<details>`/`<summary>`) — 저장 없음, 기본 접힘"을 새 구조 요소 타입으로 등재**.

### 9.1 반영 필수 (P1 — 위임 전 필수)

| # | 위치 | 수정 |
| :--- | :--- | :--- |
| 1 | **위험 고지 문구 방향 역전** (D3 문구 + §4 번역 표 5개 언어 전체) | 문구를 다음 방향으로 전면 재작성: ① **기본(OFF) 상태에서** caps lock 의존 기능이 활성일 때 시스템 전역 커널 HID 매핑(caps lock → F18)이 설치된다 ② 이 옵션을 켜면 그 매핑을 제거하고 이벤트 합성으로 대체한다 ③ **켬의 트레이드오프**: 커널 매핑 없이 caps lock 래칭 한계(뗌 이벤트 미도착, `key-remapping-engine.md` §5 #20, `main.rs:1066-1081` 주석)로 caps lock 기반 기능(hold 모드 Seek 트리거, 캡스락 프리셋)이 불안정해질 수 있다 ④ 부수 정정: 매핑은 "항상"이 아니라 **caps lock 의존 기능이 활성인 동안** 설치된다(`compute_caps_lock_alias` 의 `needs_alias` 조건) |
| 2 | **누락된 기존 테스트 3건** (안 하면 빌드 파괴) | (a) `synthesize_caps_lock_remap_토글이_엔진에도_반영된다`(`frontend_wiring.rs:993-1010`) — 메뉴 토글 함수를 메인 소스에서 grep 하는 정적 테스트. 이 함수를 제거하면 `expect` 패닉 → **삭제하되 의도(토글이 엔진에 반영)를 설정 경로로 재고정**: `apply_preset_setting` 의 `PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP` 분기가 `reconfigure_engine` 에 도달함을 단언하는 테스트로 대체. (b) `NEW_KEYS_FOR_ISSUE_39_PHASE_2_3` 상수(`frontend_wiring.rs:2051`)에 `"settings.general.diagnostics"` 포함 — D6 키 삭제 시 실패 → 상수 목록에서 제외. (c) General 패널 헤딩 테스트(`frontend_wiring.rs:2155-2173`)가 `"general-diagnostics-heading"` **존재** 단언 — 헤딩 제거 시 실패 → 기존 헤딩 목록에서 제외. `<hr />` 개수 4 단언은 details 가 hr 자리에 들어가면 그대로 성립 — 유지 |
| 3 | **헬퍼 삭제 × 테스트 모순** | `main.rs:6967-6979` 의 두 테스트(`synthesize_caps_lock_remap_enabled_defaults_to_false`·`_reflects_stored_value`)는 **헬퍼 `synthesize_caps_lock_remap_enabled` 를 직접 호출**한다(6969·6978행). 헬퍼 삭제 시 컴파일 깨짐 → 두 테스트를 `store.get(keys::PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP)` 직접 단언(부재 = 기본값)으로 바꿔 저장 키 불변 의도를 보존. 리스크 #2 는 "해소됨"으로 마감 |

### 9.2 반영 (P2·P3)

| # | 위치 | 수정 |
| :--- | :--- | :--- |
| 4 | §3.2 항목 1 | `build_normal_menu` 의 `synth_caps_checked` 파라미터와 호출 지점 2곳(`main.rs:5103` setup_tray · `5180` rebuild_tray_menu) 정리를 작업 목록에 명시: 시그니처 변경 + 호출 지점의 `synthesize_caps_lock_remap_enabled` 호출 제거 |
| 5 | §3.5 항목 2 | §3.8 에 "④ 접이식" 추가 **하지 않는다** — §3.2 컨트롤 카탈로그에 구조 요소로 등재(범주 오류 수정) |
| 6 | §2 D2 + §3.5 항목 3 | README 등재는 갈라짐이 아니라 **이탈 D9** 로 — D5 선례(사용자 실경험 + 명시 요구 근거)를 붙인다 |
| 7 | §4 번역 표 | 문구(`.why`)만 1 번 방향으로 재작성. 라벨 자체(`Synthesize Caps Lock Remap`)는 5언어 현행 유지 |

### 9.3 핵심 실측 사실 (문구 재작성의 근거, 리뷰가 확정)

```
main.rs:1088  if needs_alias && !presets.synthesize_caps_lock_remap { Some(KeyCode::F18) }
              즉 synthesize 를 켜면 alias(caps lock → F18 커널 매핑)를 만들지 않는다.
              꺼져 있으면(기본) 경로 B 커널 매핑이 설치된다. 켬 = 커널 매핑에서 벗어나는 탈출구.
```
