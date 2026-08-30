# F-01 · Seek — 활성화와 세션 상태 머신

> **한 줄 요약**: Seek 세션을 여는 세 경로(전역 단축키 토글, 키 리매핑 트리거, `Presets` 탭 quick press caps lock)와 hold/toggle 두 모드, 그리고 세션의 유휴→시작→후보준비→질의→선택→확정/취소 생명주기와 세션 중 키 입력 라우팅 규칙을 정의한다. ⭐ 출고 기본 설치 상태에서는 두 주된 활성화 경로(단축키·키 리매핑)가 모두 미설정이라 **Seek 를 발동할 방법이 전혀 없다** — 사용자가 반드시 하나 이상을 직접 설정해야 한다(실측: AX 트리 + `defaults`).
> **의존성**: `F-07`(key-remapping-engine — `CGEventTap` 설치·재활성화·우선순위 중재)의 **소비자**로서 리매핑 키 다운/업 이벤트와 세션 중 키 라우팅 계약만 사용한다. `F-11`(권한 획득)이 부여한 Accessibility·Input Monitoring·Screen Recording 권한이 이미 있다고 가정한다.
> **관련 명세**: 후보 생성(OCR·AX·화면 캡처)은 `F-02`(seek-text-detection.md), 검색 바·하이라이트·연결선 렌더링과 창 관리는 `F-03`(seek-overlay-ui.md), 실제 클릭 합성과 창 포커스는 `F-04`(seek-click-execution.md), `CGEventTap` 자체의 설치·재활성화는 `F-07`(key-remapping-engine.md), 권한 획득 흐름은 `F-11`, quick press caps lock 판정 메커니즘과 출력 후보 열거형은 `power-user-presets.md`(F-08.2)를 참조한다.
> **1차 근거**: `docs/research/app-bundle-analysis.md`(실측: AX 트리·`defaults`·번들 심볼·번들 문자열). 근거 표기 정의는 그 문서 §0.

---

## 1. 개요

Seek 는 마우스나 트랙패드 없이, 화면에 보이는 텍스트를 타이핑해서 그 위치를 클릭하는 기능이다(`superkey-inventory.md` §1.1: "Match what you type, and click it ― all with the keyboard and anywhere on the screen"). 사용자는 **Spotlight 유사 검색 바**(§4.5 개발자 포스트)에 문자열을 입력하고, 화면에서 매치되는 후보 중 하나를 키보드만으로 선택·확정한다.

F-01 은 이 기능의 **활성화 트리거, 세션의 생명주기, 세션 중 키 입력의 라우팅 규칙**을 다룬다. Seek 는 별개의 두 하위 시스템(화면에서 후보를 찾는 F-02, 후보를 화면에 그리는 F-03, 클릭을 실제로 실행하는 F-04)을 조율하는 **상태 머신**이며, F-01 은 그 조율 로직 자체다. 후보를 어떻게 찾는지, 어떻게 그리는지, 클릭을 어떻게 합성하는지는 F-01 의 관심사가 아니다 — F-01 은 "언제 세션이 열리고, 언제 어떤 키가 무엇을 하며, 언제 닫히는가"만 정의한다.

⭐ **온보딩 함의.** 출고 상태의 `Seek` 탭을 실측하면 `Toggle Seek with shortcut:` 는 빈 값(버튼 라벨 `Record Shortcut`), `Remap key to Seek:` 는 `-`(미설정)다(실측: AX 트리 + `defaults` 부재, §4). 두 경로 모두 미설정이므로 **아무것도 설정하지 않은 사용자는 Seek 를 단 한 번도 발동할 수 없다.** 세 번째 경로인 `Presets` 탭의 `Quick press caps lock to execute:` 도 기본 ☐(§4)다. 클론의 온보딩 설계는 이 사실을 전제해야 한다 — Seek 를 홍보하려면 최초 실행 흐름에서 활성화 경로 중 하나를 설정하도록 유도하거나, 최소한 미설정 상태를 사용자에게 알려야 한다.

## 2. 사용자 시나리오

### 시나리오 A — toggle 모드 (전역 단축키)

전제: 사용자가 `Toggle Seek with shortcut:` 를 직접 설정했다(예: `⌥Space`). ⭐ 이 값은 **출고 기본값이 아니다** — 실측 기본값은 빈 값(미설정, 버튼 라벨 `Record Shortcut`)이며 `⌥Space` 는 홍보 스크린샷에만 등장한다(실측: AX 트리 + `defaults` 부재, §4). `Remap key to Seek:` 는 설정되어 있지 않거나(`-`) `Only show while the remapped key is held` 가 꺼져 있음.

1. 사용자가 아무 앱에서나 (자신이 설정한 단축키, 예시로) `⌥Space` 를 누른다.
2. Seek 세션이 열린다 — 오버레이가 나타나고(F-03 위임), 화면 캡처와 후보 생성이 트리거된다(F-02 위임).
3. 사용자가 `set` 을 타이핑한다. 각 글자는 Seek 검색 바에만 들어가고, 방금까지 포커스를 갖고 있던 앱에는 전달되지 않는다.
4. 화면에 "Settings", "Set as default", "Preset" 세 후보가 매치되어 하이라이트된다(F-03 위임).
5. 사용자가 `;` 를 눌러 다음 매치로 순환하거나 `↓`/`Tab` 으로 다음 매치로 이동한다.
6. 원하는 매치("Settings")가 선택된 상태에서 `Enter` 를 누른다.
7. F-01 은 선택된 매치의 좌표를 F-04(클릭 실행)에 넘기고, 클릭 실행 완료 콜백을 받으면 세션을 닫는다 — 오버레이가 사라지고 원래 앱으로 키 입력 라우팅이 복귀한다.

### 시나리오 B — hold 모드 (키 리매핑, 개발자 권장 설정)

전제: 사용자가 `Remap key to Seek:` = `caps lock` 로, `Only show while the remapped key is held` = ☑ 로 직접 설정했다(superkey-inventory.md §1.4: 개발자 본인이 권장하는 구성). ⭐ 이 역시 **출고 기본값이 아니다** — 실측 기본값은 `Remap key to Seek:` = `-`(미설정), `Only show while the remapped key is held` = ☐ 이고, 후자는 전자가 `-` 인 동안 비활성(dimmed) 상태다(실측: AX 트리, §4). `-` 가 아닌 값을 고르는 순간 비로소 이 체크박스가 활성화된다.

1. 사용자가 `caps lock` 을 누른 채로 유지한다. 누르는 순간 Seek 세션이 열린다.
2. `caps lock` 을 계속 누른 채로 `err` 를 타이핑한다(§4.5: "requires you to be able to type words with a pinky down on the caps lock key" — 새끼손가락으로 caps lock 을 누른 채 나머지 손가락으로 타이핑).
3. "Error message", "Terraform" 두 후보가 하이라이트된다.
4. 사용자가 `caps lock` 을 뗀다. 부제 "Release the remapped key to click" 그대로, **키를 떼는 순간이 곧 확정**이다 — 이 시점에 선택되어 있던 매치가 그대로 확정되어 클릭이 실행된다.
5. 세션이 닫히고 원래 앱으로 포커스와 키 라우팅이 복귀한다.

### 시나리오 C — 취소

전제: 시나리오 A 와 동일한 toggle 모드.

1. `⌥Space` 로 세션을 연다.
2. `che` 까지 타이핑했으나 원하는 매치가 없거나, 사용자가 다른 작업으로 전환하고 싶어졌다.
3. `Esc` 를 누른다 `(추정 — §9 참조)`.
4. 세션이 클릭 실행 없이 즉시 닫힌다. 검색 바에 입력했던 `che` 는 폐기되고, 원래 앱으로 포커스와 키 라우팅이 복귀한다.

### 시나리오 D — quick press caps lock 경유 (⭐ 신규 확정, 세 번째 활성화 경로)

전제: `Presets` 탭 `Quick press caps lock to execute:` = ☑ / `Seek`(팝업 49항목 중 첫 항목, 실측: AX 트리, §4).

1. 사용자가 `caps lock` 을 짧게 눌렀다 뗀다(quick press 판정 — 임계값·메커니즘은 F-07/`power-user-presets.md` F-08.3 소관).
2. F-07 이 이를 quick press 로 판정하면 Seek 세션이 열린다.
3. 이후 절차(타이핑·순환·확정)는 시나리오 A/B 와 동일하다.

⭐ quick press 는 정의상 **키를 뗀 뒤에야 판정이 성립**하므로(길게 누르고 있는 동작이 아니라 "짧게 누르고 뗌"이 트리거), 시나리오 B 의 hold 모드(누르고 있는 동안 세션 유지, 떼는 순간 확정)와 같은 방식으로 동작하기는 구조적으로 어렵다는 추론은 가능하다. 그러나 이 경로가 실제로 hold 모드로 취급되는지 toggle 모드로 취급되는지는 조사 자료로 확정되지 않았다 `(미확정)` → §9.

## 3. 동작 명세

### 3.1 원칙

- 세션은 항상 **정확히 0개 또는 1개**만 존재한다. 새 활성화 시도는 §3.3 "중복 활성화" 규칙을 따른다.
- 세션이 열려 있는 동안, `CGEventTap` 을 통해 들어오는 문자 키 이벤트는 F-07 의 리매핑 엔진 계약에 따라 **Seek 검색 바로만 라우팅**되고 하위(포커스를 잃은) 앱으로는 전달되지 않는다. 반대로 세션이 닫혀 있으면 F-01 은 어떤 키 이벤트도 소비하지 않는다(리매핑 키 자체의 감시는 F-07 이 항상 수행).
- **검색어 키**(문자·숫자·기호 대부분)와 **제어 키**(↑ ↓ Tab ⇧Tab `;` Enter Esc 및 hold 모드의 리매핑 키 자체)는 분리된 채널로 처리된다. 제어 키는 검색어 버퍼에 추가되지 않는다.
- 모든 키 판정은 **물리 키코드(virtual keycode) 기준**이다. `;` 순환이 v1.51 에서 "regardless of keyboard layout" 으로 수정된 사실(superkey-inventory.md §2.1)이 이 원칙의 근거이며, F-01 의 모든 제어 키 판정(↑↓Tab⇧Tab, Enter, Esc 포함)에 동일하게 적용해야 나중에 같은 버그를 재생산하지 않는다.
- ⭐ **승격 — 화살표 키/Tab 순환, Enter 확정.** Seek 탭 제목 옆 ⓘ 팝오버 원문(SuperKey 원문, 실측: AX 트리 + nib, `SeekInfoViewController`):
  > "Search on screen for text."
  > "Use the arrow keys or tab to cycle through matches, and press enter to execute a mouse click."

  이로써 **화살표 키 또는 Tab 으로 매치를 순환하고, Enter 로 클릭을 실행한다**는 서술은 더 이상 `(추정)`이 아니라 확정된 사실이다. 다만 원문은 "arrow keys or tab" 만 언급할 뿐 **`⇧Tab`(역방향)을 명시하지 않는다** — 아래 §3.2 표의 `⇧Tab` 행은 방향키 대칭성으로부터의 합리적 설계이지 원문 근거는 아니므로 `(미확정)`으로 남긴다(§9). `Esc` 취소 역시 원문이 언급하지 않아 `(추정)`으로 유지한다.

### 3.2 상태 머신

| 현재 상태 | 입력 | 조건 | 다음 상태 | 부수효과 |
| :--- | :--- | :--- | :--- | :--- |
| 유휴(Idle) | 전역 단축키 다운 (`Toggle Seek with shortcut:`) | 활성 세션 없음 | 세션열림·후보대기(Opening) | 오버레이 표시 요청(F-03 위임) · 화면 캡처·후보 생성 트리거(F-02 위임) · 세션 모드 = toggle 로 기록 · 이후 문자 키를 Seek 로 라우팅 시작(F-07 계약) |
| 유휴(Idle) | 리매핑 키 다운 (`Remap key to Seek:`), `Only show while the remapped key is held` = ☐ | 활성 세션 없음 | 세션열림·후보대기(Opening) | 위와 동일, 세션 모드 = toggle 로 기록 |
| 유휴(Idle) | 리매핑 키 다운 (`Remap key to Seek:`), `Only show while the remapped key is held` = ☑ | 활성 세션 없음 | 세션열림·후보대기(Opening) | 위와 동일, 세션 모드 = hold 로 기록 · 리매핑 키의 업(release) 이벤트 감시 시작 |
| 유휴(Idle) | quick press caps lock 판정 (`Presets` 탭 `Quick press caps lock to execute:` = `Seek`) `(⭐ 신규 — 실측: AX 트리, §4)` | 활성 세션 없음 · F-07 이 quick press 로 판정 | 세션열림·후보대기(Opening) | 위와 동일, 세션 모드 `(미확정 — hold 로 성립하기 어렵다는 추론은 가능하나 단정하지 않음. §2 시나리오 D·§9 참조)` |
| Opening | F-02 로부터 "후보 생성 완료" 콜백 | — | 준비완료(Ready) | 후보 하이라이트 렌더 요청(F-03 위임) |
| Opening | 문자 키 입력 | 캡처·후보 생성 아직 미완료 | Opening 유지 | 입력 문자를 쿼리 버퍼에 누적(하위 앱에는 전달 안 함) — §5 "캡처 지연 중 사용자 입력" |
| 준비완료(Ready) | 문자 키 입력 | — | 질의입력중(Querying) | 쿼리 버퍼에 문자 추가 · 후보를 쿼리로 필터링(F-02 위임) — ⭐ F-02 쪽 `reloadMatchesDelay` 로 디바운스됨(§4·app-bundle-analysis.md §2.3) |
| Querying | 문자 키 입력 / Backspace | — | Querying 유지 | 쿼리 버퍼 갱신 · 후보 재필터링(디바운스 적용, 위와 동일) |
| Ready / Querying | `↓` 또는 `Tab` `(실측: AX + nib, §3.1)` | 필터링된 후보 ≥ 1 | 매치선택됨(Selected) | 다음 후보를 선택 상태로 지정 · 하이라이트 갱신 요청(F-03 위임) |
| Ready / Querying | `↑` 또는 `⇧Tab` `(↑ 는 실측: AX + nib, §3.1. ⇧Tab 은 원문 미언급 — (미확정))` | 필터링된 후보 ≥ 1 | Selected | 이전 후보를 선택 상태로 지정 · 하이라이트 갱신 요청 |
| Ready / Querying / Selected | `;` (세미콜론, 물리 키코드) | `Semicolon highlights next match` = ☑ | Selected | 다음 후보로 순환 이동 · 하이라이트 갱신 요청 |
| Selected | `↓`/`Tab`/`↑`/`;` `(실측)` · `⇧Tab` `(미확정)` | 필터링된 후보 ≥ 1 | Selected | 지정된 방향으로 선택 이동 |
| Selected | `Enter` `(실측: AX + nib, §3.1)` | — | 확정처리중(Confirming) | 선택된 매치 좌표를 F-04(클릭 실행)에 위임 |
| Opening / Ready / Querying (hold 모드) | 리매핑 키 업(release) | 선택된 매치 없음 | 취소됨(Cancelled) | 클릭 실행 없이 즉시 종료 절차 진행 — §5 "hold 모드에서 트리거 키를 너무 빨리 뗌" |
| Selected (hold 모드) | 리매핑 키 업(release) | 선택된 매치 있음 | Confirming | 현재 선택 매치 좌표를 F-04 에 위임 — "Release the remapped key to click" |
| Confirming | F-04 로부터 "클릭 실행 완료" 콜백 | — | 종료됨(Closed) | 오버레이 닫기 요청(F-03 위임) · 원래 앱으로 포커스·키 라우팅 복귀(F-07 계약 해제) |
| Opening / Ready / Querying / Selected | `Esc` `(추정 — §9)` | — | Cancelled | 클릭 실행 없이 종료 절차 진행 |
| Opening / Ready / Querying / Selected (toggle 모드) | 동일한 전역 단축키 또는 동일한 리매핑 키의 재입력 | 세션 모드 = toggle | Cancelled | 토글 의미상 재입력은 닫기로 처리 — §5 "세션 재진입·중복 활성화" |
| Cancelled | (내부 전이) | — | Closed | 오버레이 닫기 요청 · 원래 앱으로 포커스·키 라우팅 복귀 |
| 임의 활성 상태 | 활성화 트리거(전역 단축키 다운 또는 리매핑 키 다운) | 이미 세션이 열려 있고, 이번 트리거가 **다른** 경로(예: 세션은 리매핑 키로 열렸는데 전역 단축키가 눌림) | 상태 유지(입력 무시) | 부수효과 없음 — §5 "세션 재진입·중복 활성화" |

### 3.3 세션 재진입·중복 활성화 요약

- **같은 경로**로 열린 세션에 **toggle 모드**로 같은 트리거가 다시 들어오면 세션을 닫는다(위 표의 토글 재입력 행).
- **같은 경로**로 열린 세션에 **hold 모드**에서 동일 리매핑 키의 반복(autorepeat) 다운 이벤트가 들어오는 경우, 이는 새 활성화 시도가 아니라 눌림 유지의 연속이므로 무시한다(상태 불변).
- **다른 경로**의 트리거(세션은 리매핑 키로 열려 있는데 전역 단축키가 눌리는 경우 등)는 무시한다 — 두 번째 세션을 열지 않는다. 이 정책은 조사 자료로 확정되지 않았으며 합리적 기본 정책으로 채택한 것이다 `(추정)`.

### 3.4 ⭐ 검색 바의 실체 — 오버레이에 그려지는 요소가 아니라 독립 창

내부 타입 `EntryBarWindow` · `EntryBarWindowController` · `EntryBarViewController` · `EntryOutlineView` · `EntryBarTableRowView` · `EntrySearchButton` · `EntryBarMatches` · `EntryBarOptions` · `ClickablePlaceholderView`(실측: 번들 심볼, app-bundle-analysis.md §4.7).

**검색 바는 위치·크기가 저장되는 독립 창이다.** 조사 시점의 `defaults` 에 `NSWindow Frame EntryBarWindow = "1692 1147 400 40 0 0 3840 1570 "` 가 있다 → **400 × 40 pt** 크기이고, 사용자가 옮긴 위치가 그대로 보존된다(저장 키 `persistPosition`, 실측: `defaults`). 즉 세션이 열릴 때마다 화면 중앙 등 고정 좌표에 다시 그려지는 오버레이 내부 요소가 아니라, 자기 자신의 프레임을 기억하는 `NSWindow` 다. F-01 은 이 창의 생성·해제 시점(세션 Opening ↔ Closed)만 책임지고, 실제 렌더링·배치 로직은 `F-03`(seek-overlay-ui.md) 소관이다.

`EntryOutlineView` / `EntryBarTableRowView` 타입 이름은 **검색 바에 매치 목록이 표(outline/table) 형태로 함께 붙어 있다**는 것을 시사한다 — 화면 위 하이라이트만으로 매치를 보여주는 게 아니라는 뜻이다. `(미확정 — 타입 이름으로부터의 해석, 실제 렌더링 형태는 관찰하지 못함. §9, app-bundle-analysis.md §8 #3 참조)`.

관련 내부 키(실측: 번들 문자열, 전부 UI 미노출·`(미확정)`): `enterSelectsMatch` · `customKeyCycleSeek` · `semicolonCycleSeek` · `highlightFirst` · `persistMatches` · `matchesShown` · `totalMatches` · `SelectedMatch` · `PartialMatchData` · `reloadMatchesDelay` · `reloadMatchesWorkItem` · `lastReloadMatches` · `closeWorkItem` · `clearOnShow`.

⭐ 이 중 두 키가 기존 명세에 없던 동작을 시사한다 — 둘 다 존재만 기록하고 의미는 `(미확정)`으로 둔다:
- `highlightFirst` — 세션을 열거나 쿼리를 바꿨을 때 **첫 매치가 자동으로 선택 상태가 될 수 있음**을 시사한다. 사실이라면 §3.2 의 Ready 진입 시 "선택된 매치 없음" 전제가 조건부로 바뀐다.
- `clearOnShow` — 세션을 **열 때 이전 쿼리를 비울 수 있음**을 시사한다.

⭐ `reloadMatchesDelay` 의 존재는 **쿼리 입력 시 후보 재계산에 디바운스가 있다**는 뜻이다 — §3.2 의 Querying 전이(문자 키 입력 → 후보 재필터링)에 이미 반영했다.

## 4. 설정 항목

아래는 F-01(활성화·세션·매치 이동/확정)에 직접 관련된 `Seek` 탭 설정만 다룬다. `Seek using macOS accessibility`, `Match on more than one character`, `Only Seek in the frontmost window`, `Focus window before clicking`, `Change click modes with modifier keys` 는 각각 F-02·F-03·F-04 범위이므로 이 문서에서 다루지 않는다.

⭐ **오기 정정.** 기존 명세는 이 네 항목의 기본값을 스크린샷(홍보용 구성)에서 그대로 가져와 `(추정)`으로 표시했으나, 전부 틀렸다. 근거 논리: 사용자가 어떤 설정도 바꾸지 않은 상태의 `defaults`(§3.4, app-bundle-analysis.md §2.1)에는 Seek 관련 키가 **전혀 없고**, 같은 상태의 AX 트리 실측도 전부 꺼짐/미설정이었다. 즉 **"plist 에 키가 없다" = "기본값이 곧 OFF/미설정"** 이라는 것이 이 문서 전체가 채택하는 판정 논리다(app-bundle-analysis.md §2.1). `superkey-inventory.md` §7 Q3(출고 기본값 미확인)가 이로써 해소된다.

| 이름(원문 라벨) | 타입 | 기본값 (실측) | 활성화 조건 | 유효 범위 | 저장 키 | 출처 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| `Toggle Seek with shortcut:` | 전역 단축키 레코더(`KeyboardShortcuts.RecorderCocoa`, 지우기 가능) | ⭐ **빈 값(미설정)**. 버튼 라벨이 `Record Shortcut` (실측: AX 트리 + `defaults` 부재) — 기존 명세의 `⌥Space` 는 **홍보 스크린샷 값**이며 출고 기본값이 아니다 | 항상 | 임의의 modifier+키 조합. 미설정(빈 값) 가능 — 지우기 버튼 존재 | (KeyboardShortcuts 패키지 자체 저장소로 추정, `com.knollsoft.Superkey` plist 최상위 키는 아님 `(미확정)`) | app-bundle-analysis.md §6.1, §1.2 |
| `Remap key to Seek:` | 팝업 버튼(35항목) | ⭐ **`-`(미설정)** (실측: AX 트리 + `defaults` 부재) — 기존 명세의 `caps lock` 은 틀렸다 | 항상 | **35종 확정**, 표시 순서: `-` · `caps lock` · `right option` · `right shift` · `right command` · `right control` · `left option` · `left shift` · `left command` · `left control` · `menu (PC)` · `F1`~`F24`. ⭐ **`globe` 이 없다** — Hyperkey 탭의 동일 성격 팝업(hyper/meh/bleh 소스 키, 35종)에는 `globe` 이 있고 `-` 가 없어 **두 팝업의 열거형이 다르다**(§5 참조) | `seekRemapKeycode` (`-` 는 `0`) | app-bundle-analysis.md §6.1, §2.2 |
| `Only show while the remapped key is held` | 체크박스 | ⭐ **☐** (실측: AX 트리 + `defaults` 부재) — 기존 명세의 ☑ 는 틀렸다 | ⭐ **`Remap key to Seek:` ≠ `-` 일 때만 활성화**, `-` 이면 비활성(dimmed)(실측: 팝업을 `F13` 으로 바꾸자 활성화, `-` 로 되돌리자 다시 비활성 — app-bundle-analysis.md §7 #2) | ☑(hold 모드) / ☐(toggle 모드) | `seekExecuteOnClose` | app-bundle-analysis.md §6.1, §2.2, §7 |
| `Semicolon highlights next match` | 체크박스 | ⭐ **☐** (실측: AX 트리 + `defaults` 부재) — 기존 명세의 ☑ 는 틀렸다 | 항상 | ☑ / ☐. v1.51 부터 물리 키코드 기준으로 판정(레이아웃 독립) | `semicolonCycleSeek` | app-bundle-analysis.md §6.1, §2.2 |
| (`Presets` 탭) `Quick press caps lock to execute:` = `Seek` | 팝업 선택지(48종 중 첫 항목, 그 아래 구분선) | ⭐ **신규 확정된 세 번째 활성화 경로**. 그룹 자체 기본값은 ☐ / `caps lock`(§7의 Presets 탭 기본값 참조) — 선택지에 `Seek` 를 고르면 그 경로가 활성화됨 | `Presets` 탭에서 이 팝업이 `Seek` 로 설정되어 있을 때 | — | — (저장 키는 `power-user-presets.md` F-08.2 소관) | app-bundle-analysis.md §6.3, §2.3 |

## 5. 엣지 케이스와 실패 모드

1. **Secure Input(암호 필드) 활성 중 트리거.** macOS 가 Secure Input 을 켜면 3rd-party 앱은 키 입력을 알 수 없다(superkey-inventory.md §1.3: "Password text fields in macOS are secure and prevent 3rd party applications from knowing which keystrokes are pressed"). `Remap key to Seek:` 트리거는 F-07 의 `CGEventTap` 에 의존하므로 Secure Input 중에는 리매핑 키 다운 자체가 감지되지 않아 세션이 열리지 않는다. `Toggle Seek with shortcut:` 경로는 `global-hotkey` 크레이트가 이벤트 탭이 아니라 시스템 핫키 **등록** 방식이라(rust-macos-capability-notes.md §1.2: "가로채기가 아니라 등록") Secure Input 의 영향을 받지 않을 가능성이 있으나 확정되지 않았다 `(추정)` → §9.
2. **후보 0개.** 쿼리와 매치되는 후보가 없으면 Ready/Querying 상태에 머무른다. `↑`/`↓`/`Tab`/`;` 는 부수효과 없이 무시되고(표의 "필터링된 후보 ≥ 1" 조건 불충족), `Enter` 도 선택된 매치가 없으므로 부수효과 없이 무시된다(상태 불변). 세션은 사용자가 `Esc` 로 취소하거나 쿼리를 바꿔 매치가 생길 때까지 열려 있다.
3. **캡처 지연 중 사용자 입력.** Opening 상태에서 F-02 의 후보 생성이 아직 끝나지 않았는데 사용자가 타이핑하면, 입력 문자는 폐기되지 않고 쿼리 버퍼에 누적된다(표 참조). 후보 생성이 완료되면 누적된 쿼리로 즉시 필터링을 적용한다. 이 버퍼링이 없으면 빠르게 타이핑하는 사용자의 앞부분 입력이 유실된다.
4. **hold 모드에서 트리거 키를 너무 빨리 뗌.** 아직 선택된 매치가 없는 상태(Opening/Ready/Querying)에서 리매핑 키가 릴리즈되면, "Release the remapped key to click" 의 의미상 확정할 대상이 없으므로 클릭을 실행하지 않고 취소로 처리한다(표 참조). 매치가 하나라도 있지만 아직 선택되지 않은 경우(Ready/Querying, `Selected` 상태 진입 전) 어떤 매치가 암묵적으로 "선택된 것"으로 간주되는지(예: 첫 번째 후보 자동 선택 여부)는 조사 자료로 확정되지 않았다 `(추정)` → §9.
5. **세션 중 앱 전환·Space 전환.** 사용자가 세션이 열린 채로 `⌘Tab` 등으로 다른 앱이나 Space 로 전환하는 경우의 동작(세션을 유지할지, 자동 취소할지)은 조사 자료에 근거가 없다 `(추정)` → §9. 안전한 기본값으로는 자동 취소(클릭 대상이 더 이상 화면에 없을 수 있으므로)를 제안하되, 최종 정책은 확인이 필요하다.
6. **세션 중 디스플레이 구성 변경.** 모니터 연결·해제나 해상도 변경이 세션 중 발생하는 경우다. superkey-inventory.md §2.1 은 v1.55 에서 "Fixed broken Seek behavior on additional displays" 를 명시해 다중 디스플레이가 실제로 깨진 이력이 있는 영역임을 보여준다. F-01 관점에서는 최소한 진행 중인 세션을 취소하고 재활성화를 요구하는 것이 안전하다 `(추정)` — 좌표계 재계산 자체는 F-02/F-03 소관.
7. **리매핑 키가 Hyperkey/Presets 와 충돌.** `Remap key to Seek:` 에 지정한 키가 동시에 `Hyperkey` 탭의 hyper 소스 키이거나 `Presets` 탭의 리매핑 대상(예: `caps lock`)이면 물리 키 하나를 여러 기능이 두고 경쟁한다. superkey-inventory.md §6.2 는 이것이 실제 버그 이력(v1.20, v1.62)이 있는 영역이라고 명시한다. F-01 은 이 충돌을 직접 해소하지 않는다 — F-07(key-remapping-engine)이 단일 리매핑 엔진과 명시적 우선순위 규칙으로 어떤 키가 Seek 트리거로 판정될지를 결정하고, F-01 은 F-07 이 "Seek 트리거"로 판정해 올려보낸 이벤트만 소비한다.
   ⭐ **소스 키 열거형의 비대칭(실측: AX 트리, §4).** `Remap key to Seek:` 팝업(35종)에는 `globe` 이 없지만 `Hyperkey` 탭의 hyper/meh/bleh 소스 키 팝업(동일하게 35종)에는 `globe` 이 있고 대신 `-`(미설정)가 없다. 결과적으로 **`globe` 키는 Seek 의 트리거로 지정할 수 없다** — 그 키를 둘러싼 F-07 충돌 판정에서 Seek 는 애초에 경쟁자가 될 수 없는 유일한 소스 키다. 반대로 `Remap key to Seek:` 의 다른 34종(`caps lock` 부터 `F24` 까지)은 전부 `Hyperkey`/`Presets` 팝업에도 나타나므로 위 문단의 충돌 시나리오가 성립한다. 클론 UI 도 두 팝업의 선택지 열거형을 동일하게 두어서는 안 된다.
8. **전체화면 앱.** 전체화면 앱 위에서 세션을 열 수 있는지, 오버레이가 보이는지는 F-03(오버레이 창 레벨) 소관이다. F-01 의 관심사는 활성화 트리거(전역 단축키·리매핑 키)가 전체화면 앱 포커스 중에도 F-07 의 `CGEventTap` 을 통해 정상 수신되는가인데, 이는 CGEventTap 자체가 앱 전체화면 여부와 무관하게 시스템 레벨에서 동작하므로 별도 처리가 불필요하다 `(추정)`.
9. **권한 거부 상태.** Accessibility 또는 Input Monitoring 권한이 거부되어 있으면 F-07 의 `CGEventTap` 이 설치되지 않거나 키 이벤트를 받지 못해 `Remap key to Seek:` 트리거가 전혀 작동하지 않는다(권한 요청·복구 흐름은 F-11 참조). `Toggle Seek with shortcut:` 전역 단축키 경로가 이 상태에서도 동작하는지는 `global-hotkey` 가 별도 권한 체계를 쓰는지에 달려 있으며 조사로 확정되지 않았다 `(추정)`.
10. **auto-repeat 키다운.** 리매핑 키를 누르고 있으면 OS 가 반복 키다운 이벤트를 보낼 수 있다. hold 모드에서 이를 새로운 활성화 시도로 오인하면 안 되므로, 이미 세션이 열려 있는 동안의 같은 키의 반복 다운 이벤트는 무시한다(§3.3).

## 6. 필요한 플랫폼 API

F-01 자체는 화면 캡처·OCR·AX 파싱·클릭 합성 API 를 직접 호출하지 않는다. F-01 이 직접 소비하는 것은 다음 두 가지 이벤트 소스뿐이다.

- **리매핑 키 다운/업 이벤트** — F-07(key-remapping-engine)이 설치·유지하는 `CGEventTap` 콜백에서 물리 키코드로 판정해 F-01 에 전달하는 이벤트. 근거 크레이트: `core-graphics` 0.25.0(`CGEventTap` 안전 래퍼) 또는 `objc2-core-graphics` 0.3.2(`CGEventTapCreate` 원시 바인딩) — rust-macos-capability-notes.md §1.1, §1.2, §2.1. F-01 은 이 탭을 직접 설치하지 않고 F-07 이 제공하는 구독 인터페이스만 쓴다.
- **세션 중 문자/제어 키 이벤트의 소비(consume)** — 같은 `CGEventTap` 콜백이 `kCGEventTapOptionDefault` 로 설치되어 있어야 이벤트를 하위 앱에 전달하지 않고 가로챌 수 있다(rust-macos-capability-notes.md §2.1: "`Default` 옵션이어야 이벤트를 소비·치환할 수 있다"). 이 설치·재활성화(`kCGEventTapDisabledByTimeout`/`ByUserInput` 대응 포함)는 F-07 의 책임이며, F-01 은 "세션이 열려 있는 동안 문자 키는 하위 앱에 전달되지 않는다"는 **계약**만 의존한다.
- **전역 단축키 등록** — `Toggle Seek with shortcut:` 은 `CGEventTap` 가로채기가 아니라 시스템 레벨 핫키 **등록**이다. 근거 크레이트: `global-hotkey` 0.8.0(rust-macos-capability-notes.md §1.2: "전역 단축키 등록(Tauri 팀 관리). 가로채기가 아니라 등록").
- **TCC 권한 전제** — Accessibility(`AXIsProcessTrusted()`)와 Input Monitoring(`IOHIDCheckAccess`)가 F-07 이 `CGEventTap` 을 설치하기 위한 전제 조건이다(rust-macos-capability-notes.md §2.5). 권한 요청·상태 확인 UI 는 F-11 소관이며, F-01 은 권한이 이미 부여되어 있다고 가정하고 부여되지 않은 경우의 증상(§5 항목 9)만 서술한다.

## 7. 구현 접근

**판정: Rust 바인딩.**

F-01 의 로직(상태 머신 자체, 쿼리 버퍼 관리, 매치 선택 인덱스 이동, 모드 판별)은 순수한 애플리케이션 로직으로 플랫폼 API 를 직접 호출하지 않는다. 그러나 F-01 이 소비하는 입력(리매핑 키 다운/업, 제어 키 여부 판정)은 물리 키코드(`CGKeyCode`) 와 `CGEventType` 이라는 플랫폼 타입에 직접 의존하며, 이 타입들은 `core-graphics` 0.25.0 / `objc2-core-graphics` 0.3.2 가 제공하는 바인딩을 통해서만 얻을 수 있다. 전역 단축키 등록도 `global-hotkey` 0.8.0 크레이트에 의존한다. 두 크레이트 모두 rust-macos-capability-notes.md 가 crates.io 로 검증한 안정 버전이며, 필요한 API 표면(이벤트 타입, 키코드, 핫키 등록)이 이미 노출되어 있어 **네이티브 Swift/Objective-C shim 을 별도로 작성할 필요가 없다**.

- **기각한 대안 1 — `rdev`.** 전역 키 이벤트 크레이트지만 rust-macos-capability-notes.md §1.2 가 "3년간 릴리스 없음 — 신규 의존 비권장"이라고 명시한다. 이벤트 소비(consume) 제어도 제한적이라 F-07 이 요구하는 `kCGEventTapOptionDefault` 수준의 치환·소비가 어렵다.
- **기각한 대안 2 — "순수 Rust" 판정.** 상태 머신 로직만 보면 순수 Rust 로 보이지만, F-01 의 입력 경계(어떤 키가 제어 키인지 판정하는 지점)가 `CGKeyCode`/`CGEventType` 같은 바인딩 타입과 맞닿아 있어 이 판정을 채택하면 F-01 과 F-07 사이의 계약이 모호해진다. 따라서 "Rust 바인딩"으로 명시해 F-01 이 사용하는 타입이 어디서 오는지 분명히 한다.
- **기각한 대안 3 — "네이티브 shim 불가피".** rust-macos-capability-notes.md 는 F-01 이 필요로 하는 이벤트 타입·전역 단축키 등록 어느 쪽에도 커버리지 공백을 보고하지 않았다(공백이 확인된 영역은 F-03 의 window level 제어, F-11 의 Input Monitoring TCC 등 다른 명세의 몫이다). 따라서 F-01 자체에는 shim 이 불가피하다고 볼 근거가 없다.

## 8. 수용 기준

- [ ] `Toggle Seek with shortcut:` 에 설정된 단축키를 누르면 세션이 열리고, 활성 세션이 없는 상태에서 200ms 이내(관찰 가능한 프레임 단위)에 오버레이 표시 요청과 캡처 트리거가 모두 발생한다.
- [ ] `Remap key to Seek:` 에 설정된 키를 누르면 `Only show while the remapped key is held` 값에 따라 세션 모드가 toggle 또는 hold 로 정확히 기록된다.
- [ ] hold 모드에서 리매핑 키를 누르고 있는 동안 세션이 유지되고, 키를 떼는 즉시(다음 이벤트 틱 내) 그 시점에 선택되어 있던 매치가 확정되어 F-04 에 위임된다.
- [ ] hold 모드에서 매치가 하나도 선택되지 않은 채 리매핑 키를 떼면 클릭이 실행되지 않고 세션이 즉시 닫힌다.
- [ ] 세션이 열려 있는 동안 타이핑한 문자는 Seek 검색 바에만 나타나고, 포커스를 잃은 하위 앱의 텍스트 필드에는 어떤 문자도 도달하지 않는다.
- [ ] `↑`/`↓`/`Tab`/`⇧Tab` 입력 후 다음 프레임 내 선택된 매치의 하이라이트가 인접 후보로 이동한다.
- [ ] `Semicolon highlights next match` 가 켜진 상태에서 물리적으로 세미콜론 위치의 키를 누르면, 활성 키보드 레이아웃과 무관하게 다음 매치로 순환한다(비-QWERTY 레이아웃에서도 동일하게 동작).
- [ ] `Enter` 를 누르면 현재 선택된 매치 좌표가 F-04 에 위임되고, F-04 의 완료 콜백 수신 후 세션이 닫히며 원래 앱으로 키 라우팅이 복귀한다.
- [ ] `Esc` 를 누르면 어떤 상태에서든 클릭 실행 없이 세션이 즉시 닫힌다.
- [ ] 세션이 열려 있는 동안 동일한 활성화 트리거(같은 전역 단축키 또는 같은 리매핑 키)가 다시 들어와도 두 번째 세션이 생성되지 않는다(toggle 모드에서는 기존 세션이 닫힌다).
- [ ] 캡처·후보 생성이 완료되기 전에 입력된 문자가 유실되지 않고, 완료 즉시 누적된 쿼리로 필터링된 결과가 표시된다.
- [ ] 후보가 0개인 상태에서 `↑`/`↓`/`Tab`/`⇧Tab`/`;`/`Enter` 를 눌러도 예외 없이 상태가 유지된다(크래시·행 없음).

## 9. 미해결 질문

| # | 질문 | 조사 문서 연결 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| ~~1~~ | ~~출고 기본값~~ — ⭐ **해소**. §4 표 참조: `Toggle Seek with shortcut:` = 빈 값, `Remap key to Seek:` = `-`, `Only show while the remapped key is held` = ☐(비활성), `Semicolon highlights next match` = ☐ | app-bundle-analysis.md §2.1, §6.1 | 해소됨 |
| ~~2~~ | ~~`Remap key to Seek:` 팝업 선택지 전체 목록~~ — ⭐ **해소**. 35종, §4 표 참조. `globe` 없음이 확정, Hyperkey 팝업과 비대칭(§5) | app-bundle-analysis.md §6.1 | 해소됨 |
| 3 | `Esc` 가 실제로 취소 키인가 (§2 시나리오 C, §3.2 표에서 `(추정)`으로 표시) | ⓘ 팝오버 원문(§3.1)이 "arrow keys or tab" 과 "enter" 만 언급, `Esc` 는 언급 없음 — macOS 표준 관례(Spotlight 등)로부터의 추정만 유지 | 앱 설치 후 실동작 확인, 또는 개발자 문의 |
| 3b | `⇧Tab`(역방향 순환)이 실제로 지원되는가 (§3.1·§3.2 표에서 `(미확정)`으로 표시) | ⓘ 팝오버 원문이 "tab" 만 언급, 방향 대칭성으로부터의 추정일 뿐 | 앱 설치 후 실동작 확인 |
| 4 | hold 모드에서 매치가 있지만 아직 명시적으로 `Selected` 상태로 진입하기 전에 키를 떼면 첫 번째 후보가 암묵적으로 확정되는가, 아니면 취소되는가 (§5 항목 4) — `highlightFirst` 키(§3.4)의 존재가 "첫 매치 자동 선택"을 시사하나 조건·범위 불명 | 조사 자료에 근거 없음 | 앱 설치 후 실동작 확인 |
| 4b | `highlightFirst` / `clearOnShow` / `persistMatches` 의 정확한 의미와 조건 (§3.4) | 번들 문자열에 키 이름만 있고 UI 노출 없음 | 소스 코드 확인 또는 실동작 관찰 |
| 5 | 세션 중 앱 전환·Space 전환 시 세션을 유지할지 자동 취소할지의 정책 (§5 항목 5) | 조사 자료에 근거 없음 | 앱 설치 후 실동작 확인 |
| 6 | `Toggle Seek with shortcut:`(전역 단축키) 경로가 Secure Input 중에도 동작하는지, 그리고 Accessibility/Input Monitoring 권한 거부 상태에서도 동작하는지 (§5 항목 1, 9) | rust-macos-capability-notes.md §1.2("가로채기가 아니라 등록")에서 추론했으나 미확정 | `global-hotkey` 0.8.0 의 macOS 구현이 Carbon `RegisterEventHotKey` 계열인지 소스 확인, 실기 테스트 |
| 7 | `CGEventTap` 의 `CFRunLoopSource` 를 Tauri 메인 런루프에 붙일지 전용 스레드 런루프를 쓸지 — F-01 이 F-07 로부터 이벤트를 받는 지연 시간에 영향 | rust-macos-capability-notes.md §4 P1 | 실측(전용 스레드 vs 메인 런루프 벤치마크) |
| 8 | quick press caps lock 경유 Seek 활성화(§2 시나리오 D, §3.2)가 hold 모드인지 toggle 모드인지 | quick press 는 키를 뗀 뒤 판정되므로 hold 가 구조적으로 성립하기 어렵다는 추론은 가능하나 조사 자료로 확정되지 않음 | 앱 설치 후 실동작 확인 — `Presets` 탭에서 `Quick press caps lock to execute:` 를 `Seek` 로 설정하고 caps lock 을 짧게 눌러 관찰 |
| 9 | `EntryOutlineView`/`EntryBarTableRowView` 가 시사하는 "검색 바에 표 형태로 붙은 매치 목록"의 실제 형태, 그리고 Seek 오버레이 자체의 표시 형태(라벨 문자 집합·배치·색) | Seek 을 실제로 발동시키지 않았다 — 발동에는 Screen Recording 권한 프롬프트와 전체 화면 캡처가 따르고 관찰 이득 대비 부작용이 크다고 판단(app-bundle-analysis.md §8 #3) | 실제로 세션을 열어 오버레이·검색 바를 관찰(부작용 검토 후) |
