# F-09 · 환경설정 UI

> **한 줄 요약**: 좌측 사이드바(`Seek`·`Hyperkey`·`Presets`·`General` 4탭) + 우측 패널로 구성된 환경설정 창의 구조·컨트롤 종류·저장 방식·엔진으로의 즉시 반영 계약을 정의한다. 개별 설정의 의미는 다루지 않는다 — 이 문서는 "창 자체"의 명세다.
> **의존성**: 이 문서가 정의하는 저장·즉시반영 계약은 `F-07`(key-remapping-engine — 단일 리매핑 엔진·우선순위 중재)이 소비자다. `Seek` 탭의 개별 설정은 `F-01`~`F-04`, `Hyperkey`/`Presets` 탭은 각 소유 명세(§3.1 표), `General` 탭은 `F-10`~`F-14`(전부 `(추정)`)가 소유한다. 권한 상태 표시는 `F-11`(permissions-onboarding.md)의 온보딩 흐름과 연동된다.
> **관련 명세**: 개별 설정의 동작 의미 → 각 기능 명세(§3.1, §4 표에서 지시). 오버레이 창(Seek 검색 바) → `F-03`(seek-overlay-ui.md). 메뉴바 상주·환경설정 창을 여는 경로 → `F-10`. 리매핑 엔진 자체 → `F-07`(key-remapping-engine.md, 파일명 `(추정)`). 권한 획득·상태 표시 → `F-11`. 플랫폼 역량 종합 판정 → `platform-constraints.md`.

---

## 1. 개요

환경설정 창은 Superkey 의 모든 설정을 담는 유일한 UI 표면이다. 조사 자료(`superkey-inventory.md` §3)가 확보한 스크린샷 3장(`seekScreenshot.png`, `hyperScreenshot.png`, `presetScreenshot.png`)은 이 창을 원본 해상도로 담고 있으며, 창은 **좌측 사이드바(탭 목록) + 우측 패널(선택된 탭의 컨트롤들)** 구조다. 탭은 `Seek` · `Hyperkey` · `Presets` · `General` 4개이고 이 순서가 스크린샷 3장 모두에서 일관되게 확인된다.

이 문서가 다루는 것은 **창의 골격**이다 — 어떤 탭이 있고, 각 탭에 어떤 종류의 컨트롤(체크박스, 팝업, 슬라이더 등)이 등장하며, 컨트롤을 조작했을 때 값이 어디에 저장되고 언제 엔진에 반영되는가. 각 설정이 *무엇을 하는지*(예: `Match on more than one character` 가 AX 매칭을 어떻게 제한하는지)는 그 설정을 소유하는 개별 기능 명세의 몫이다.

개발자 본인이 명시한 설계 원칙(`superkey-inventory.md` §4.5)이 이 문서의 핵심 제약이다:

> "Superkey will follow the same **simple checkbox-centered configuration**"
> "There's definitely a tradeoff between offering complex entirely custom key remappings versus the canned presets here, but **I'll always prefer just checking a box over messing around with complicated preferences construction**"

즉 환경설정 UI 는 **고정된 항목의 열거**이지, 사용자가 임의 규칙을 조합·저장하는 규칙 편집기가 아니다. `Presets` 탭이 16개의 개별 체크박스로 이루어진 것 자체가 이 원칙의 직접적 결과이며, 클론도 "새 항목을 추가하는 규칙 빌더"를 만들지 않는다.

## 2. 사용자 시나리오

### 시나리오 A — 환경설정을 열고 Seek 리매핑 키를 바꾼다

1. 사용자가 메뉴바 아이콘(`F-10` 소유)을 클릭해 "Preferences…" 를 선택한다 `(추정 — §9)`.
2. 환경설정 창이 열리고, 이전에 마지막으로 보고 있던 탭(예: `Seek`)이 다시 선택된 상태로 나타난다 `(추정 — §9)`.
3. `Remap key to Seek:` 팝업 버튼을 클릭해 `caps lock` 대신 `right option` 을 선택한다.
4. 선택 즉시(별도 "적용" 버튼 없이) 값이 저장되고, `F-07`(리매핑 엔진)에 새 매핑이 반영된다(§3.7). 이전에 `caps lock` 을 누르고 있어도 더 이상 Seek 가 열리지 않는다.
5. 사용자가 창을 닫는다(빨간 stoplight 버튼 또는 `⌘W`). 엔진은 계속 백그라운드에서 동작한다(§5).

### 시나리오 B — 단축키 레코더에서 이미 점유된 조합을 입력한다

1. `Seek` 탭에서 `Toggle Seek with shortcut:` 필드를 클릭해 레코딩을 시작한다(§3.5).
2. 사용자가 `⌘Space`(macOS 기본 Spotlight 단축키)를 누른다.
3. 레코더가 이 조합이 시스템 단축키와 충돌함을 인지하고 경고를 표시한다(§3.5, §5) `(추정 — §9)`. 값은 저장되지 않고 이전 값(`⌥Space`)이 유지된다.
4. 사용자가 `Esc` 를 눌러 레코딩을 취소하거나, 다른 조합을 다시 눌러 레코딩을 이어간다.

### 시나리오 C — Presets 탭에서 서로 다른 그룹의 설정을 다수 조작한다

1. 사용자가 `Presets` 탭으로 전환한다. 좌측 키캡 일러스트(`caps lock`/`shift`/`delete`)로 구분된 4개 그룹이 보인다.
2. `Quick press duration` 슬라이더를 드래그해 값을 `1000 ms` 에서 `600 ms` 로 낮춘다. 드래그 중 값 라벨이 실시간으로 갱신된다.
3. `Caps lock + [H J K L] = ◀▼▲▶` 체크박스를 켠다. 라벨 문장 중간의 인라인 팝업(`H J K L`)은 그대로 유지된다(별도 변경 없음).
4. 각 변경은 개별적으로 즉시 저장·반영된다(§3.7) — 탭을 벗어나거나 창을 닫을 때 일괄 저장하는 방식이 아니다 `(추정 — §9, 근거: "적용" 버튼이 스크린샷에 없음)`.

## 3. 동작 명세

### 3.1 탭 구조

| 탭 | 아이콘 | 담는 설정 (개수) | 소유 명세 ID |
| :--- | :--- | :--- | :--- |
| `Seek` | 존재 확인, 구체적 모양은 조사 자료에 서술 없음 → ❓미확인 | 상위 컨트롤 8개 + 중첩 체크박스 1개 = 9개 항목(§4) | `F-01`·`F-02`·`F-03`·`F-04` |
| `Hyperkey` | ❓미확인 | 6개 항목(§4) | `F-05`(추정)·`F-07`·`F-08`(추정) |
| `Presets` | ❓미확인 | 16개 항목, 4개 그룹(caps lock / shift / delete / 기타)(§4) | `F-06`(추정)·`F-07` |
| `General` | ❓미확인 | 패널 내용 자체가 조사에서 미확인(`superkey-inventory.md` §3.4, §7 Q2). 로그인 시 실행·자동 업데이트·라이선스·권한 상태·언어가 모일 것으로 추론(§4, 전부 `(추정)`) | `F-10`·`F-11`·`F-12`·`F-13`·`F-14`(전부 추정) |

> `F-05`(Hyperkey 변환)·`F-06`(Presets)·`F-08`(트랙패드 원터치 제스처)의 ID 배정은 이 문서 작성 시점의 **추정**이다. `F-01`~`F-04`(Seek 4분할), `F-07`(key-remapping-engine)은 기존 명세(`seek-activation-and-session.md`, `seek-text-detection.md`)와 그 문서들의 의존성 선언에서 이미 확정되어 있다. `F-10`~`F-14` 는 이 작업 지시에서 명시적으로 배정됐다. 실제 각 명세 문서가 작성되는 시점에 번호가 확정되면 이 표를 갱신해야 한다.

패널 상단에는 탭 이름과 ⓘ 정보 버튼이 함께 표시된다 — 스크린샷에서 직접 확인된 것은 `Seek ⓘ`, `Hyperkey ⓘ` 뿐이다(`superkey-inventory.md` §3.1, §3.2). `Presets`·`General` 패널 제목에도 같은 ⓘ 버튼이 있는지는 조사 자료로 확인되지 않았다 `(추정 — 일관성상 있을 가능성이 높음, §9)`.

### 3.2 컨트롤 타입 카탈로그

| 타입 | 등장 위치 | 동작 규칙 |
| :--- | :--- | :--- |
| 체크박스 | 전 탭에 다수(§4 대부분) | 클릭 시 즉시 토글, 별도 확인 없이 §3.7 의 즉시 반영 계약을 따른다 |
| 중첩 체크박스 | Seek: `Match on more than one character`(`Seek using macOS accessibility` 하위) | 시각적으로 들여쓰기되어 상위 체크박스에 종속됨을 표시. 상위가 꺼지면 비활성화(dimmed)되어 조작 불가하나, 저장된 값 자체는 보존된다 `(추정)` — 상위를 다시 켜면 이전 하위 값이 복원되는 편이 사용자 기대에 맞는다 |
| 팝업 버튼(열거형 선택) | Seek: `Remap key to Seek:`; Hyperkey: 4개(소스 키 3종 + 트랙패드 영역); Presets: 다수 | 클릭 시 드롭다운이 열리고 선택 즉시 반영. 각 팝업의 **선택지 전체 목록은 대부분 미확인**이다 — 확인된 값만 스크린샷 1개 선택 상태로 노출됨(`superkey-inventory.md` §7 Q4·Q5·Q7·Q8) |
| **단축키 레코더** | Seek: `Toggle Seek with shortcut:` | 값 `⌥Space` + 지우기 ✕ 버튼. 상세 동작은 §3.5 |
| 슬라이더 + 값 라벨 | Presets: `Quick press duration` | 눈금 8칸, 스크린샷 값 `1000 ms`. 최소/최대/간격 미확인(§9 Q6). 드래그 중 값 라벨 실시간 갱신 |
| 인라인 팝업(라벨 문장 중간에 삽입) | Presets: `Caps lock + [H J K L] = ◀▼▲▶`, `Caps lock + home row = [symbol row (A = !)]` | 라벨 문장 자체가 현재 선택값을 포함해 렌더링된다. **로케일 번역 시 문장 어순이 깨지지 않게 재구성해야 한다**(§5, RTL 포함) |
| 부제/설명 텍스트 | Seek 다수 항목("Release the remapped key to click" 등), Hyperkey 트랙패드("Slide only one touch…") | 컨트롤 바로 아래 보조 텍스트(회색, 작은 글자로 추정). 상호작용 없음 |
| 키캡 일러스트 | Hyperkey 좌측(선택된 소스 키), Presets 그룹 구분(`caps lock`/`shift`/`delete`) | 현재 선택된 물리 키를 시각적으로 표시하는 순수 표시 요소. 클릭 상호작용 없음(추정) |
| 인라인 ⓘ 툴팁 | 패널 제목(`Seek ⓘ`, `Hyperkey ⓘ`), 일부 체크박스 라벨 옆(`Seek using macOS accessibility` ⓘ, `Change click modes with modifier keys` ⓘ) | 클릭 또는 hover 시 설명 팝오버. 내용 대부분 미공개(§9 Q10) |

### 3.3 창 생명주기

- **열기 경로**: 메뉴바 아이콘 메뉴(`F-10` 소유)에서 "Preferences…" 류 항목을 선택하는 것이 유력한 경로다 `(추정)`. 최초 실행 시 권한 온보딩(`F-11`)과 함께 자동으로 열리는지, 아니면 별도 온보딩 창이 있고 환경설정은 메뉴바 경로로만 여는지는 조사 자료로 확인되지 않았다 `(추정 — §9)`. `⌘,` 표준 단축키가 지원되는지도 미확인이다.
- **닫기**: 표준 macOS 창 닫기(빨간 stoplight 버튼, `⌘W`)로 닫힌다고 가정한다 `(추정)`. 창을 닫아도 앱 자체는 종료되지 않는다(§5 참조) — 메뉴바 상주 앱의 표준 동작이다.
- **재열기 시 마지막 탭 기억**: 조사 자료로 확인되지 않았다 `(추정 — §9)`. macOS 환경설정류 앱(예: System Settings)의 일반적 관례를 따라 마지막으로 본 탭을 기억하는 쪽을 기본 정책으로 제안하되, 확정되지 않았음을 명시한다.
- **다중 인스턴스 방지**: 이미 열려 있는 상태에서 메뉴바에서 다시 "Preferences…" 를 선택하면 새 창을 열지 않고 기존 창을 최전면으로 가져온다 `(추정)` — macOS 단일 설정 창 관례.

### 3.4 접근성

- **VoiceOver**: 창이 Tauri WebView(WKWebView) 기반이라면, 표준 HTML 폼 컨트롤(체크박스=`<input type="checkbox">`, 팝업=`<select>` 또는 ARIA `role="listbox"`)은 WebKit 이 기본적으로 macOS 접근성 트리에 노출한다. 다만 **비표준 커스텀 컨트롤**(단축키 레코더, 인라인 팝업이 낀 라벨 문장, 슬라이더의 눈금·값 라벨 연동)은 ARIA 속성(`aria-label`, `aria-valuenow`, `aria-live` 등)을 명시적으로 부여하지 않으면 VoiceOver 가 의미를 읽어주지 못한다. 이는 조사 자료에 근거가 없는 구현 요구사항이며 `(추정)` 표시한다.
- **키보드 내비게이션**: 탭 사이드바는 방향키로 이동 가능해야 하고(`Seek`↔`Hyperkey`↔`Presets`↔`General`), 패널 내부는 `Tab`/`⇧Tab` 으로 컨트롤 간 이동이 가능해야 한다. 단축키 레코더가 포커스를 가진 상태에서 `Tab` 이 "다음 컨트롤로 이동"과 "레코딩 대상 키"로 이중 해석될 위험이 있다 — 레코더는 레코딩 모드가 아닐 때만 `Tab` 을 포커스 이동으로 처리해야 한다 `(추정)`.
- **다크모드**: macOS 시스템 외관(라이트/다크)을 따라야 한다. Tauri 는 `prefers-color-scheme` CSS 미디어쿼리와 `NSApp.effectiveAppearance` 변경 이벤트 구독을 통해 자동/수동 전환 모두 지원 가능하다(§6). 조사 자료(스크린샷)는 라이트 모드로만 캡처되어 다크모드 시각 사양(대비, 아이콘 톤) 자체는 확인할 수 없다 `(추정)`.

### 3.5 단축키 레코더 동작 명세

대상: `Toggle Seek with shortcut:`. 스크린샷에서 값 `⌥Space` 와 지우기 ✕ 버튼이 확인된다(`superkey-inventory.md` §3.1). 구체적 동작은 조사 자료에 근거가 없어 아래는 업계 표준(Karabiner-Elements, Alfred, Raycast 등 동종 앱의 단축키 레코더 관례)에서 채택한 합리적 기본값이며 전부 `(추정)`이다 → §9.

- **녹화 시작**: 필드를 클릭하면 레코딩 모드로 전환된다(시각적으로 필드가 강조 표시되고, placeholder 가 "키를 누르세요" 류 안내로 바뀐다).
- **녹화 중 입력 처리**: 레코딩 모드에서는 이후 눌리는 키 이벤트(modifier 조합 포함)가 앱 전체가 아니라 이 필드로만 전달된다 — 이는 F-07 의 `CGEventTap` 이 아니라 창이 포커스를 가진 상태의 **로컬 이벤트 캡처**로 처리하는 편이 안전하다(전역 탭을 새로 여는 것은 과함).
- **취소**: `Esc` 를 누르면 레코딩을 취소하고 이전 값으로 되돌린다. 필드 밖을 클릭(blur)해도 레코딩을 취소한다 `(추정)`.
- **이미 점유된 조합 처리**: 같은 조합이 이미 다른 Superkey 설정(예: `Remap key to Seek:` 와 물리적으로 겹치는 단일 키, 또는 다른 전역 단축키 필드)에 쓰이고 있으면 경고를 표시한다. 저장을 막을지(하드 차단), 경고만 하고 저장을 허용할지(소프트 경고 — 나중에 F-07 의 우선순위 규칙으로 해소)는 미확정이다 `(추정)` → §9. §6.2 의 횡단 관찰("Hyperkey 와 Presets 는 동일한 물리 키를 두고 경쟁한다")을 고려하면 소프트 경고 쪽이 원본의 실제 동작(엔진이 우선순위로 해소)과 더 부합할 가능성이 있다.
- **시스템 단축키와의 충돌 경고**: `⌘Space`(Spotlight), `⌘Tab`(App Switcher) 등 macOS 가 자체적으로 선점하는 조합을 누르면, macOS 가 그 이벤트를 애초에 서드파티 앱에 전달하지 않을 수 있다. 이 경우 레코더는 "아무 반응 없음"으로 보이거나, 별도로 알려진 위험 조합의 하드코딩 목록과 대조해 사전 경고를 표시해야 한다 `(추정)` — macOS 는 전체 시스템 단축키 표를 조회하는 공개 API 를 제공하지 않으므로(§6), 완전한 자동 감지는 불가능하고 알려진 목록 기반의 부분 대응만 가능하다.
- **지우기**: ✕ 버튼을 누르면 값이 빈 상태로 저장되고, 해당 트리거는 비활성 상태가 된다(전역 단축키로는 Seek 를 열 수 없음 — `Remap key to Seek:` 경로는 별개로 계속 동작).

### 3.6 설정 저장

**결정: 자체 JSON 저장소(`tauri-plugin-store` 기반), `com.knollsoft.Superkey` 의 `NSUserDefaults` 도메인은 사용하지 않는다.**

원본 Superkey 는 `defaults` 계열 `NSUserDefaults`(번들 ID `com.knollsoft.Superkey`, `macupdater.net` 교차 확인 — `superkey-inventory.md` §5)를 쓸 것으로 보인다 `(추정)` — Q3("각 설정의 출고 기본값")의 확인 방법으로 `defaults read com.knollsoft.Superkey` 가 제시된 것 자체가 이 추정의 근거다.

클론에서 `NSUserDefaults` 를 직접 바인딩하지 않기로 결정한 근거:

1. **다른 앱의 도메인을 재사용할 이유가 없다.** 클론은 별도 번들 ID를 가질 것이므로 애초에 `com.knollsoft.Superkey` 도메인과 무관하다. `NSUserDefaults` 자체를 저장 계층으로 쓰는 것과 원본의 도메인을 흉내 내는 것은 별개 문제이며, 후자는 시도할 이유가 없다.
2. **`rust-macos-capability-notes.md` 에 `NSUserDefaults` 전용 고수준 Rust 크레이트가 없다.** `objc2-foundation` 을 통해 `NSUserDefaults` 를 다루는 것은 가능하지만(§1.1), 이는 매 설정 변경마다 `unsafe` Objective-C 호출을 거쳐야 함을 의미한다. 반면 `tauri-plugin-autostart`, `tauri-plugin-updater` 등 이 프로젝트가 이미 채택 방향으로 삼는 Tauri 공식/준공식 플러그인 생태계(§2.8, §2.9)와 궤를 같이하는 `tauri-plugin-store` 는 안전한 Rust API 로 충분하다.
3. **스키마 마이그레이션을 직접 제어해야 한다.** `NSUserDefaults` 는 키-값 저장소일 뿐 버전 필드나 마이그레이션 개념이 없다. 어차피 자체적으로 스키마 버전 필드와 마이그레이션 로직을 얹어야 하므로(§5), `NSUserDefaults` 위에 얹으나 JSON 파일 위에 얹으나 추가 설계 비용은 동일하고, JSON 쪽이 검사·백업·디버깅이 쉽다(사람이 읽을 수 있는 파일을 직접 열어볼 수 있다).
4. **트레이드오프를 명시한다.** 이 결정은 macOS 파워유저가 기대할 수 있는 `defaults read/write <bundle-id>` 상호운용성을 포기한다. 이는 §5 의 "General 탭 항목이 평문 JSON 에 남는 문제"와 함께 재검토 대상으로 §9 에 남긴다.

**저장 위치**: Tauri 앱 데이터 디렉토리(예: `~/Library/Application Support/<bundle-id>/settings.json`) `(추정 — 정확한 경로는 tauri-plugin-store 의 기본 동작에 따름)`.

**"처리 데이터"와 "설정"의 구분**: 원문 "None of the data that Superkey processes is stored on your disk"(`superkey-inventory.md` §1.3)는 Seek 가 캡처한 화면 스크린샷, OCR 결과, AX 트리에서 읽은 텍스트 등 **런타임에 처리되는 데이터**를 말하는 것이지, 환경설정 자체를 말하는 것이 아니다. 설정값은 재실행 후에도 유지되어야 하므로 **반드시 디스크에 저장된다** — 이 문서가 정의하는 `settings.json`(또는 등가물)이 그 저장소다. 이 구분을 명시하지 않으면 "디스크에 아무것도 저장하지 않는다"는 원문을 오독해 설정 영속성 요구사항을 놓칠 위험이 있다.

### 3.7 설정 변경의 즉시 반영

- **적용 버튼 없음.** 스크린샷 어디에도 "Apply"/"OK" 류 버튼이 없다 — 모든 컨트롤은 값이 바뀌는 즉시(체크박스 클릭, 팝업 선택, 슬라이더 드래그 종료, 레코더 확정) **저장과 엔진 반영이 함께 일어난다**고 가정한다 `(추정 — 근거: 스크린샷에 확인 버튼 부재, §9)`.
- **엔진으로의 계약**: 환경설정 UI 는 값이 바뀔 때마다 F-07(key-remapping-engine)에 갱신된 설정 스냅샷(또는 변경분 델타)을 전달한다. F-07 은 이를 받아 `CGEventTap` 콜백이 참조하는 매핑 테이블을 원자적으로 교체한다. 이 계약의 정확한 형태(전체 스냅샷 재전송 vs. 필드 단위 델타, 동기/비동기)는 F-07 명세가 정의할 몫이며, F-09 는 "체크박스를 끄면 다음 키 입력부터 즉시 반영되어야 한다"는 **사용자 관찰 가능한 요구사항**만 못박는다.
- **슬라이더의 "즉시"의 의미**: 드래그 도중 매 픽셀마다 엔진에 반영하는 것은 낭비이므로, 드래그가 끝나는 시점(mouse up) 또는 debounce 이후에 반영하는 것이 합리적이다 `(추정)`. 값 라벨 자체는 드래그 중 실시간으로 갱신된다(시각적 피드백과 엔진 반영 시점은 분리).
- **저장 실패와 반영의 관계**: 저장(디스크 쓰기)이 실패해도 엔진 반영(메모리 내 상태 갱신)은 별도로 성공할 수 있다 — 즉시 반영을 저장 성공에 의존시키면 디스크 오류가 있을 때 설정이 전혀 동작하지 않는 나쁜 실패 모드가 된다. 엔진 반영은 즉시, 디스크 저장은 비동기로 처리하고 실패 시 사용자에게 알리는 쪽을 채택한다(§5).

## 4. 설정 항목

⭐ 전체 설정의 색인 표다. 기본값은 각 소유 명세에 위임하고 여기서는 반복하지 않는다.

### 4.1 `Seek` 탭 (8개 상위 + 중첩 1개 = 9개 항목)

| 설정 이름 (원문) | 탭 | 컨트롤 타입 | 소유 명세 ID | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| `Toggle Seek with shortcut:` | Seek | 단축키 레코더 | F-01 | `superkey-inventory.md` §3.1 |
| `Remap key to Seek:` | Seek | 팝업 버튼 | F-01 | §3.1 |
| `Only show while the remapped key is held` | Seek | 체크박스 | F-01 | §3.1 |
| `Seek using macOS accessibility` ⓘ | Seek | 체크박스 | F-02 | §3.1 |
| ↳ `Match on more than one character` | Seek | 중첩 체크박스 | F-02 | §3.1 |
| `Only Seek in the frontmost window` | Seek | 체크박스 | F-02 | §3.1 |
| `Focus window before clicking` | Seek | 체크박스 | F-04 `(추정)` | §3.1 |
| `Semicolon highlights next match` | Seek | 체크박스 | F-01 | §3.1 |
| `Change click modes with modifier keys` ⓘ | Seek | 체크박스 | F-04 `(추정)` | §3.1 |

> `Focus window before clicking`·`Change click modes with modifier keys` 의 F-04 귀속은 `seek-activation-and-session.md` §4 가 "각각 F-02·F-03·F-04 범위"라고만 밝히고 개별 대응을 명시하지 않아 이 문서에서 추론한 것이다 `(추정)` → §9.

### 4.2 `Hyperkey` 탭 (6개 항목)

| 설정 이름 (원문) | 탭 | 컨트롤 타입 | 소유 명세 ID | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| `Remap key to hyper key:` | Hyperkey | 체크박스 + 팝업 버튼 | F-05 `(추정)` | `superkey-inventory.md` §3.2 |
| `Include shift in hyper key` | Hyperkey | 체크박스 | F-05 `(추정)` | §3.2 |
| `Remap key to meh key (^⌥⇧):` | Hyperkey | 체크박스 + 팝업 버튼 | F-05 `(추정)` | §3.2 |
| `bleh key (⌃⌘⇧)` (v1.62 신설, 스크린샷 미공개) | Hyperkey | 체크박스 + 팝업 버튼 `(추정)` | F-05 `(추정)` | §3.2, appcast v1.62 |
| `Apply modifiers to keypress events and:` (Click/Drag/Move/Scroll) | Hyperkey | 체크박스 4개 | F-05 `(추정)` | §3.2 |
| `Engage hyper key using trackpad:` | Hyperkey | 체크박스 + 팝업 버튼 | F-08 `(추정)` | §3.2 |

### 4.3 `Presets` 탭 (16개 항목, 4개 그룹)

| 설정 이름 (원문) | 탭 | 컨트롤 타입 | 소유 명세 ID | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| `Remap caps lock to:` | Presets(caps lock) | 체크박스 + 팝업 | F-06 `(추정)` | `superkey-inventory.md` §3.3 |
| `Quick press caps lock to execute:` | Presets(caps lock) | 체크박스 + 팝업 | F-06 `(추정)` | §3.3 |
| `Quick press duration` | Presets(caps lock) | 슬라이더 + 값 라벨 | F-06 `(추정)` | §3.3 |
| `Caps lock + space = enter` | Presets(caps lock) | 체크박스 | F-06 `(추정)` | §3.3 |
| `Caps lock + W A S D = ▲◀▼▶` | Presets(caps lock) | 체크박스 | F-06 `(추정)` | §3.3 |
| `Caps lock + [H J K L] = ◀▼▲▶` | Presets(caps lock) | 체크박스 + 인라인 팝업 | F-06 `(추정)` | §3.3 |
| `Caps lock + home row = [symbol row (A = !)]` | Presets(caps lock) | 체크박스 + 인라인 팝업 | F-06 `(추정)` | §3.3 |
| `Double tap shift = caps lock` | Presets(shift) | 체크박스 | F-06 `(추정)` | §3.3 |
| `Left shift + right shift = caps lock` | Presets(shift) | 체크박스 | F-06 `(추정)` | §3.3 |
| `Shift + caps lock = caps lock` | Presets(shift) | 체크박스 | F-06 `(추정)` | §3.3 |
| `Quick press left or right shift to input corresponding:` | Presets(shift) | 체크박스 + 팝업 | F-06 `(추정)` | §3.3 |
| `Hyper + delete = forward delete` | Presets(delete) | 체크박스 | F-06 `(추정)` | §3.3 |
| `Remap delete to forward delete` | Presets(delete) | 체크박스 | F-06 `(추정)` | §3.3 |
| `Shift + delete = forward delete` | Presets(delete) | 체크박스 | F-06 `(추정)` | §3.3 |
| `Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):` | Presets(기타) | 체크박스 + 팝업 | F-06 `(추정)` | §3.3 |
| `Home & end operate on lines` | Presets(기타) | 체크박스 | F-06 `(추정)` | §3.3 |

### 4.4 `General` 탭 — ⚠️ 전부 `(추정)`

패널 내용 자체가 조사에서 확인되지 않았다(`superkey-inventory.md` §3.4, §7 Q2). 아래는 다른 기능 명세가 요구하는 항목으로부터의 추론이며, 실제 존재 여부·정확한 라벨 문구·컨트롤 타입 전부 확정되지 않았다.

| 설정 이름 (추정 라벨) | 탭 | 컨트롤 타입 (추정) | 소유 명세 ID | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| 로그인 시 실행 (`Launch at login` 류) | General | 체크박스 `(추정)` | F-10 `(추정)` | 작업 지시 + `superkey-inventory.md` §3.4 |
| 자동 업데이트 확인 (`Automatically check for updates` 류) | General | 체크박스 `(추정)` | F-13 `(추정)` | 작업 지시 + §3.4, §2.3(Sparkle) |
| 라이선스 등록·상태 (`License key` 입력·`Activate` 류) | General | 텍스트 필드 + 버튼 `(추정)` | F-12 `(추정)` | 작업 지시 + §3.4, §4.3(activation 카운트) |
| 권한 상태 표시 (Accessibility/Screen Recording/Input Monitoring 상태 배지) | General | 상태 표시 + "시스템 설정 열기" 버튼 `(추정)` | F-11 `(추정)` | 작업 지시 + §3.4, §1.3(권한 3종) |
| 언어 선택 (`Language` 류) | General | 팝업 버튼 `(추정)` | F-14 `(추정)` | 작업 지시 + §2.3(8개 로케일 번들) |
| 메뉴바 아이콘 표시/숨김 `(추정)` | General | 체크박스 `(추정)` | F-10 `(추정)` | §3.4(추론 목록) |
| 버전 정보 | General | 텍스트 표시 `(추정)` | 소유 명세 미정 `(추정)` | §3.4(추론 목록) |
| 환경설정 초기화(`Reset to defaults` 류) | General | 버튼 `(추정)` | F-09(이 문서, 창 자체 동작) | §3.4(추론 목록) |

## 5. 엣지 케이스와 실패 모드

1. **설정 저장 실패.** 디스크 쓰기 실패(권한 문제, 디스크 풀, 샌드박스 경로 문제)가 발생하면, §3.7 의 원칙대로 엔진 반영(메모리 상태)은 계속 성공시키되 사용자에게 저장 실패를 알리고 재시도하거나 마지막 성공 상태로 롤백할지 선택하게 한다 `(추정)`. 조용히 무시하면 재실행 후 설정이 사라지는 조용한 데이터 손실이 된다.
2. **손상된 설정 파일.** `settings.json` 이 JSON 파싱에 실패하는 경우(비정상 종료 중 쓰기 등), 손상된 파일을 `settings.json.bak` 류로 보존한 뒤 기본값으로 폴백하고 사용자에게 알린다 `(추정)`. 조용히 기본값으로 덮어쓰면 사용자가 공들여 만든 설정이 통보 없이 사라진다.
3. **이전 버전 스키마 마이그레이션.** 새 버전에서 필드가 추가·삭제·이름 변경되는 경우(v1.62 의 `bleh key` 신설이 실제 사례), 스키마 버전 필드로 판별해 마이그레이션 함수를 순차 적용한다. 마이그레이션 자체가 실패하면 원본 파일을 보존한 채 기본값으로 시작하고 사용자에게 알린다 `(추정)` — `NSUserDefaults` 를 쓰지 않기로 한 결정(§3.6)의 직접적 근거이기도 하다.
4. **서로 배타적인 설정의 동시 선택 방지.** `Remap key to Seek:` 의 대상 키가 동시에 `Hyperkey` 탭의 소스 키이거나 `Presets` 탭의 리매핑 대상(예: `caps lock`)으로 지정되는 경우, UI 레벨에서 저장을 막을지(하드 차단) 저장은 허용하고 F-07 의 우선순위 규칙으로 해소할지 결정되지 않았다 `(추정)` → §9. `superkey-inventory.md` §6.2 가 이 충돌이 실제 버그 이력(v1.20, v1.62)임을 보여주므로 UI 차원의 최소한의 경고(하드 차단이 아니더라도)는 필요해 보인다.
5. **권한 없는 상태에서 기능 토글.** Screen Recording 권한이 없는데 Seek 관련 체크박스(`Seek using macOS accessibility` 등)를 켜는 것 자체는 UI 레벨에서 막지 않는다 `(추정)` — 체크박스는 "이 기능을 원한다"는 사용자 의사이며, 실제 동작 가능 여부는 F-11 의 권한 상태 배너가 별도로 알린다. 두 UI(설정 체크박스, 권한 배너)의 불일치를 사용자가 오인하지 않도록 General 탭의 권한 상태 항목이 명확히 보여야 한다.
6. **단축키 충돌.** §3.5 참조. 이미 점유된 조합이거나 시스템 예약 조합인 경우 경고를 표시하되, 시스템 예약 조합 전체를 프로그램적으로 열거할 공개 API 가 없어(§6) 알려진 위험 목록 기반의 부분 대응만 가능하다.
7. **창을 닫아도 엔진은 계속 동작.** 환경설정 창의 close 는 앱 종료(quit)가 아니다 — 메뉴바 상주 앱(`F-10`)의 표준 동작이며, `CGEventTap` 과 리매핑 엔진(`F-07`)은 창 상태와 무관하게 계속 실행된다. 창을 닫는 것과 "일시정지"를 혼동하는 UI(예: 창을 닫으면 리매핑이 꺼진다는 오해)를 만들지 않아야 한다.
8. **다중 디스플레이에서 창 위치 복원.** 마지막으로 창이 있던 디스플레이가 연결 해제된 상태로 재실행되면, 저장된 좌표가 화면 밖을 가리킬 수 있다. 사용 가능한 디스플레이 목록(`available_monitors()`, §6)과 대조해 좌표가 어떤 화면에도 속하지 않으면 주 디스플레이 중앙으로 클램프한다 `(추정)`. `superkey-inventory.md` §2.1 이 v1.55 에서 "다중 디스플레이에서 Seek 오버레이가 깨졌다"는 이력을 보여주므로, 환경설정 창도 같은 부류의 취약점을 안고 시작한다고 가정하고 설계해야 한다.
9. **로케일별 라벨 길이(RTL 포함).** 번들 8개 로케일 중 `he`/`ar`/`fa` 는 RTL 이다(`superkey-inventory.md` §2.3). 인라인 팝업이 낀 라벨 문장(`Caps lock + [H J K L] = ◀▼▲▶`, `Caps lock + home row = [...]`)은 번역 시 어순이 완전히 달라질 수 있어 고정 문자열 템플릿(`"{prefix} {popup} {suffix}"`)으로 구현하면 RTL 언어에서 문법이 깨진다. 팝업 위치가 문장 내에서 가변적임을 전제로 한 i18n 템플릿 설계가 필요하다. 긴 번역 문자열이 체크박스 라벨을 다단으로 줄바꿈시키는 레이아웃 붕괴도 함께 고려해야 한다.
10. **단축키 레코딩 중 창/앱 포커스 이동.** 레코딩 모드에서 사용자가 `⌘Tab` 등으로 다른 앱으로 전환하면, 레코딩은 자동으로 취소되고 이전 값으로 되돌아가야 한다 `(추정)` — 포커스를 잃은 채로 레코딩 모드가 남아 있으면 이후 그 앱에서의 키 입력이 의도치 않게 레코더로 흘러들어갈 위험이 있다(단, 로컬 이벤트 캡처이므로 실제로는 포커스 상실과 함께 자연히 입력이 끊길 가능성이 높다 — 그래도 UI 상태는 명시적으로 되돌려야 한다).
11. **General 탭 민감정보의 저장 방식.** 라이선스 키 등 상대적으로 민감한 값이 §3.6 에서 결정한 평문 JSON 파일에 그대로 남는 것이 적절한지는 별도 검토가 필요하다 `(추정)` — 라이선스 활성화 자체의 보안 모델은 F-12 소관이지만, 저장 위치가 이 문서(F-09)의 결정에 종속되므로 여기서 리스크만 표기한다.

## 6. 필요한 플랫폼 API

- **창 생성·관리(안전 Rust)**: Tauri `WebviewWindow`/`WindowBuilder` — `decorations(true)`(표준 타이틀바), 고정 크기 또는 리사이즈 가능 여부는 미확인 `(추정)`. `rust-macos-capability-notes.md` §2.6 이 다룬 특수 window level·클릭 통과 등은 오버레이(F-03) 전용 요구사항이며 이 창에는 해당하지 않는다.
- **창 위치 저장·복원**: Tauri `window.outer_position()` / `set_position()` (안전 Rust) + `window.available_monitors()` 로 디스플레이 목록을 조회해 §5 항목 8 의 클램프 로직을 구현한다.
- **단축키 레코더의 로컬 키 캡처**: `global-hotkey` 0.8.0(§1.2)은 "가로채기가 아니라 등록"이라 임의 조합을 실시간으로 캡처하는 레코딩 UI 용도로는 맞지 않는다. macOS 표준 패턴은 `NSEvent.addLocalMonitorForEvents(matching:handler:)`(창이 키 창일 때만 동작) 이며, 이는 `objc2-app-kit` 을 통해 접근 가능하나 `rust-macos-capability-notes.md` 는 이 특정 API 의 커버리지를 명시적으로 확인하지 않았다 → §9.
- **시스템 예약 단축키 조회**: macOS 는 System Settings > Keyboard Shortcuts 에 등록된 전체 단축키 표를 조회하는 공개 API 를 제공하지 않는다(조사 자료에 근거 없음, 일반적으로 알려진 macOS 제약) → 완전 자동 충돌 감지는 불가능하고, 알려진 고위험 조합(`⌘Space`, `⌘Tab`, `⌘⇧3/4/5` 등)의 하드코딩 목록 대조만 가능하다 `(추정)`.
- **설정 저장**: `tauri-plugin-store`(§1.3) — JSON 기반, 앱 데이터 디렉토리에 저장. 스키마 버전 필드와 마이그레이션은 자체 로직으로 얹는다(플러그인 자체는 마이그레이션을 제공하지 않음).
- **VoiceOver**: WKWebView 가 표준 HTML 컨트롤을 자동으로 접근성 트리에 노출하므로 별도 네이티브 API 호출이 원칙적으로 불필요하다. 커스텀 컨트롤(단축키 레코더, 인라인 팝업 라벨)에는 ARIA 속성을 HTML/CSS/JS 레벨에서 직접 부여한다 — 플랫폼 API 라기보다 프론트엔드 구현 책임이다.
- **다크모드**: CSS `prefers-color-scheme` 로 대부분 커버되며, 앱 전체의 강제 라이트/다크 전환(시스템과 무관하게)을 지원하려면 Tauri 의 테마 API 또는 `objc2-app-kit` 의 `NSApp.appearance` 설정이 필요하다 `(추정)`.
- **권한 상태 조회(General 탭 표시용)**: `rust-macos-capability-notes.md` §2.5 — `AXIsProcessTrusted()`, `CGPreflightScreenCaptureAccess()`, `IOHIDCheckAccess(kIOHIDRequestTypeListenEvent)`. 이 문서(F-09)는 상태를 **표시**만 하고, 요청·온보딩 흐름 자체는 F-11 소관이다.

## 7. 구현 접근

**판정: Rust 바인딩.**

환경설정 창은 오버레이(F-03)와 요구사항이 근본적으로 다르다. 오버레이는 지연 예산이 빡빡하고(키 입력마다 재렌더링) 특수 window level·클릭 통과·다중 디스플레이 union frame 계산 등 AppKit 저수준 제어가 필수였다(`rust-macos-capability-notes.md` §2.6). 반면 환경설정 창은:

- **지연 예산이 느슨하다.** 체크박스 클릭, 팝업 선택, 슬라이더 드래그 모두 사람이 조작하는 폼 상호작용이며, 프레임 단위 응답성이 요구되지 않는다.
- **폼 컨트롤이 많다.** 31개 이상의 개별 설정(§4)이 체크박스·팝업·슬라이더·텍스트 필드로 구성되며, 이는 HTML `<form>` 이 원래 잘하는 일이다. 각 컨트롤을 네이티브 `NSButton`/`NSPopUpButton`/`NSSlider` 로 하나하나 만드는 것보다, HTML/CSS/JS 로 만들고 필요한 곳에만 네이티브 이스케이프(§6)를 쓰는 것이 개발 비용 면에서 합리적이다.
- **표준 창 동작이면 충분하다.** `decorations: true` 인 평범한 타이틀바 창으로, 오버레이가 요구했던 `NSScreenSaverWindowLevel`·`ignoresMouseEvents`·전체화면 위 표시 같은 특수 처리가 필요 없다.

따라서 **Tauri WebView 로 이 창을 만드는 것이 적절하다** — 오버레이와 달리 여기서는 "WebView 왕복 비용"이 문제가 되는 지점이 없다. 다만 두 가지는 명시적으로 짚어야 한다.

1. **네이티브 룩앤필 재현 비용.** WKWebView 는 macOS System Settings 특유의 시각 요소(사이드바 리스트 스타일, `NSPopUpButton` 의 셰브론과 눌림 효과, `NSSlider` 의 눈금 렌더링, 창 전체의 vibrancy)를 "공짜로" 제공하지 않는다. 이들을 CSS/HTML/JS 로 픽셀 단위까지 재현하려면 상당한 프론트엔드 작업이 필요하며, 완벽히 동일한 룩앤필을 포기하고 "네이티브에 가까운" 수준으로 타협하는 것이 현실적이다. 이는 기능적 공백이 아니라 **디자인 투자 비용**의 문제다.
2. **단축키 레코더만 예외적으로 네이티브에 가깝다.** §6 에서 짚었듯 `NSEvent.addLocalMonitorForEvents` 류 API 가 필요할 가능성이 높고, 이는 `objc2-app-kit` 을 통한 `unsafe` 호출이다(Rust 바인딩 판정의 근거). 그러나 이는 창 전체를 네이티브로 만들어야 한다는 뜻이 아니라, 이 창 안의 **한 컨트롤**만 네이티브 API 를 호출하는 것으로 충분하다.

- **기각한 대안 1 — "순수 Rust".** 창 위치 저장/복원, 단축키 레코더, 권한 상태 조회(General 탭)가 모두 플랫폼 타입(`NSEvent`, TCC 함수)에 직접 의존해 순수 애플리케이션 로직만으로 완결되지 않는다.
- **기각한 대안 2 — "네이티브 shim 불가피".** 오버레이(F-03)와 달리 `rust-macos-capability-notes.md` 가 이 창의 요구사항(표준 창, 폼 컨트롤, 위치 복원)에 대해 커버리지 공백을 보고하지 않았다. 유일하게 불확실한 지점(로컬 이벤트 모니터)도 기존 `objc2-app-kit` 바인딩으로 해결 가능할 가능성이 높고, 별도의 Swift/Objective-C 소스 파일을 프로젝트에 추가해야 할 근거는 없다.

## 8. 수용 기준

- [ ] 환경설정 창을 열면 좌측 사이드바에 `Seek`·`Hyperkey`·`Presets`·`General` 4개 탭이 이 순서로 표시된다.
- [ ] 각 탭을 클릭하면 우측 패널이 해당 탭의 설정으로 전환되고, 패널 상단에 탭 이름이 표시된다(`Seek`·`Hyperkey` 는 ⓘ 정보 버튼과 함께).
- [ ] `Seek` 탭에 §4.1 의 9개 항목(중첩 체크박스 포함)이 모두 존재하고, `Match on more than one character` 는 `Seek using macOS accessibility` 가 꺼진 상태에서 비활성화(조작 불가)로 표시된다.
- [ ] `Hyperkey` 탭에 §4.2 의 6개 항목이 모두 존재하고, `Apply modifiers to keypress events and:` 아래 `Click`/`Drag`/`Move`/`Scroll` 4개 체크박스가 개별적으로 토글 가능하다.
- [ ] `Presets` 탭에 §4.3 의 16개 항목이 4개 그룹(caps lock/shift/delete/기타)으로 시각적으로 구분되어 모두 존재한다.
- [ ] 임의의 체크박스를 토글하면 별도의 "적용"/"확인" 조작 없이 다음 키 입력부터 F-07 리매핑 엔진에 새 값이 반영된다(관찰 가능한 지연 1초 이내).
- [ ] 단축키 레코더 필드를 클릭하면 레코딩 모드로 전환되고, 조합을 입력하면 그 조합이 필드에 표시되며 저장된다. `Esc` 를 누르면 레코딩이 취소되고 이전 값이 유지된다.
- [ ] 단축키 레코더의 ✕ 버튼을 누르면 값이 비워지고, 저장된 상태에서 해당 트리거가 비활성화된다.
- [ ] 앱을 재시작해도 이전에 설정한 모든 값(4개 탭 전체)이 그대로 유지된다(§3.6 저장소에서 로드).
- [ ] `settings.json`(또는 등가 저장 파일)이 파싱 불가능한 상태로 손상되어 있을 때 앱이 크래시하지 않고 기본값으로 시작하며, 손상된 파일이 별도 백업으로 보존된다.
- [ ] 환경설정 창을 닫아도(빨간 stoplight 또는 `⌘W`) 앱은 종료되지 않고, 직전까지 활성화되어 있던 리매핑(예: hyper 키)은 계속 동작한다.
- [ ] VoiceOver 가 켜진 상태에서 `Tab` 키만으로 각 탭과 패널 내 모든 컨트롤에 순서대로 접근할 수 있고, 각 컨트롤의 현재 값이 음성으로 안내된다.
- [ ] 시스템 외관을 다크 모드로 전환하면 환경설정 창도 별도 재실행 없이 다크 모드로 전환된다.

## 9. 미해결 질문

| # | 질문 | 조사 문서 연결 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | `General` 탭의 실제 항목·라벨·컨트롤 타입 전체 | `superkey-inventory.md` §3.4, §7 Q2 | 앱 설치 후 확인 |
| 2 | 환경설정 창을 여는 정확한 경로(메뉴바 항목 클릭인지, 최초 실행 시 자동으로 뜨는지, `⌘,` 지원 여부) | 조사 자료에 직접 근거 없음 | 앱 설치 후 확인, 또는 §7 Q15(메뉴바 메뉴 구성)와 함께 확인 |
| 3 | 재열기 시 마지막으로 보던 탭을 기억하는지 | 조사 자료에 근거 없음 | 앱 설치 후 확인 |
| 4 | 각 탭의 아이콘 구체적 모양 | 조사 자료가 탭 존재만 확인, 아이콘 모양은 서술하지 않음 | 스크린샷 재확인 또는 앱 설치 |
| 5 | `Presets`·`General` 패널 제목에도 `Seek ⓘ`/`Hyperkey ⓘ` 와 같은 인라인 ⓘ 버튼이 있는지 | `superkey-inventory.md` §3.1, §3.2 는 Seek·Hyperkey 만 명시 | 스크린샷 재확인(Presets 스크린샷 재검토) 또는 앱 설치 |
| 6 | `Focus window before clicking`, `Change click modes with modifier keys` 의 정확한 소유 명세(F-03 vs F-04) | `seek-activation-and-session.md` §4 가 F-02·F-03·F-04 로만 뭉뚱그려 서술 | F-03/F-04 명세 작성 시 확정 |
| 7 | `F-05`(Hyperkey)·`F-06`(Presets)·`F-08`(트랙패드 제스처)의 실제 명세 ID 배정 | 이 문서에서 잔여 번호로 추정 배정 | 프로젝트 명세 ID 부여 규칙 확인, 각 명세 작성 시 확정 |
| 8 | 단축키 레코더의 정확한 동작(취소 조건, 충돌 시 하드 차단 vs 소프트 경고, blur 시 취소 여부) | 조사 자료에 근거 없음, 동종 앱 관례로 추정 | 앱 설치 후 실동작 확인 |
| 9 | 서로 다른 탭 간 같은 물리 키를 지정할 때 UI 가 저장을 차단하는지, 아니면 F-07 의 우선순위로만 해소하는지 | `superkey-inventory.md` §6.2(v1.20, v1.62 버그 이력)에서 충돌 자체는 확인되나 UI 차원 처리는 불명 | 앱 설치 후 실동작 확인 |
| 10 | 설정 변경이 "탭 단위"로 즉시 반영되는지, 컨트롤 단위로 즉시 반영되는지(적용 버튼 부재로 추정한 것일 뿐 명시적 근거 없음) | 스크린샷에 확인 버튼 부재라는 간접 근거만 있음 | 앱 설치 후 실동작 확인 |
| 11 | `Quick press duration` 슬라이더의 최소/최대/간격 | `superkey-inventory.md` §7 Q6 | 앱 설치 후 슬라이더 조작 |
| 12 | 각 팝업 버튼의 선택지 전체 목록(`Remap key to hyper key:` 등) | §7 Q4·Q5·Q7·Q8 | 앱 설치 후 팝업 열기 |
| 13 | 각 설정의 출고 기본값 | §7 Q3 | 앱 최초 실행 후 `defaults read com.knollsoft.Superkey`(원본 기준) — 클론은 자체 기본값을 설계 시점에 정의 |
| 14 | `NSEvent.addLocalMonitorForEvents` 류 로컬 키 캡처 API 가 `objc2-app-kit` 0.3.2 에 실제로 노출되어 있고 안정적으로 동작하는지 | `rust-macos-capability-notes.md` 가 이 특정 API 를 명시적으로 검증하지 않음 | crates.io 문서 및 실제 바인딩 확인, 프로토타입 구현 |
| 15 | 메뉴바 메뉴 항목 구성(환경설정을 여는 정확한 메뉴 항목 라벨 포함) | §7 Q15 | 앱 설치 후 확인 |
| 16 | `NSUserDefaults` 미사용 결정(§3.6)이 실제로 사용자 기대(파워유저의 `defaults` 명령 상호운용성)에 미치는 영향이 받아들여질 수 있는 트레이드오프인지 | 이 문서의 설계 결정, 조사 자료에 직접 근거 없음 | 제품 방향 결정(개발자/기획 판단) |
