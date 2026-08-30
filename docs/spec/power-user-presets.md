# F-08 · Power User Presets

> **한 줄 요약**: `Presets` 탭의 16개 "체크박스 하나로 켜는" 사전 정의 키 리매핑을 캡스락·시프트·삭제·기타 4개 그룹으로 전수 명세하고, 각 프리셋의 정확한 입력→출력 규칙과 서로·Hyperkey 간의 충돌·중재 규칙을 정의한다. **임의 커스텀 리매핑은 의도적으로 제공하지 않는다** — 이것이 이 기능의 설계 경계다.
> **의존성**: `F-07`(key-remapping-engine)이 설치·유지하는 단일 `CGEventTap` 위에서 동작한다. F-08 은 이 탭을 직접 설치하지 않고, F-07 이 물리 키코드 기준으로 판정해 올려주는 키다운/키업 이벤트와 **quick press 판정 결과**(참·거짓)를 소비할 뿐이다. `F-05`(hyper/meh/bleh 정의)가 정의하는 "hyper 활성" 상태를 F-08.12 가 구독한다.
> **관련 명세**: 이벤트 탭 설치·중재 우선순위·quick press 판정 메커니즘 자체는 `F-07`(key-remapping-engine.md) 참조. hyper/meh/bleh 의 정의와 소스 키 구성은 `F-05` 참조. 환경설정 창의 실제 렌더링(팝업 위젯, 슬라이더 UI)은 `F-09` 참조. Seek 의 `Remap key to Seek:` 는 `F-01`(seek-activation-and-session.md) 참조 — `Quick press caps lock to execute:` 가 `Seek` 를 선택지로 갖는 것도 F-01 의 활성화 경로 중 하나이므로 F-01 과 상호 참조한다. 캡스락 방향키/홈로우 프리셋의 레이아웃(Colemak/Dvorak) 변형 자동 적용 여부는 `F-14`(localization-and-input-sources.md) 참조. 설정 충돌 감지 대화상자의 상세 명세(버튼 구성 등)는 `F-15`(settings-store-and-integrity.md, 별도 담당) 참조.

---

## 1. 개요

`Presets` 탭은 superkey-inventory.md §3.3 이 스크린샷에서 판독한 16개 항목으로 구성되며, 좌측 키캡 일러스트(`caps lock` / `shift` / `delete`)로 4개 그룹(캡스락 7개, 시프트 4개, 삭제 3개, 기타 2개)이 구분된다. 각 항목은 "체크박스를 켜면 정해진 입력 조합이 정해진 출력을 낸다"는 **완전히 사전 정의된(canned)** 규칙이다.

이 설계는 우연이 아니라 제작자가 명시한 제품 철학이다(superkey-inventory.md §4.5, ryanhanson.dev):

> "There's definitely a tradeoff between offering complex entirely custom key remappings versus the **canned presets** here, but I'll always prefer just checking a box over messing around with complicated preferences construction."
> "Got remappings you want a checkbox for? Let me know!"

즉 Karabiner-Elements 류의 임의 규칙 엔진과 달리, **사용자가 새 규칙을 정의할 수단 자체가 없다.** 팝업 선택지(예: `Quick press caps lock to execute:` 의 출력 후보)조차 사용자가 자유 입력하는 것이 아니라, 개발자가 사용자 요청에 따라 하나씩 열거형에 추가하는 **고정 목록**이다(v1.62: "let me know if there are more desired keys for quick press caps lock"). F-08 의 클론 구현도 이 경계를 지킨다 — 커스텀 규칙 빌더는 F-08 의 범위가 아니며, 새 프리셋을 추가하려면 이 명세 자체를 개정해야 한다.

F-08 은 이 16개 프리셋 각각의 **입력 조건과 출력 이벤트**, 그리고 프리셋끼리·Hyperkey 와 물리 키를 두고 벌어지는 **충돌과 중재 규칙**을 정의한다. quick press 가 어떻게 판정되는지(타이밍 알고리즘)는 F-07 의 몫이며, F-08 은 "quick press 로 판정되면 무엇을 낸다"는 규칙만 쓴다.

## 2. 사용자 시나리오

### 시나리오 A — 캡스락 이중 용도 (Remap + Quick press)

전제 (사용자가 켠 구성 — 출고 기본값이 아니다. 출고 기본값은 16개 전부 ☐다, §4): `Remap caps lock to:` = ☑ `left control`, `Quick press caps lock to execute:` = ☑ `caps lock`, `Quick press duration` = `1000 ms`(슬라이더 실측 기본값과 우연히 같다, §3.2 F-08.3).

1. 사용자가 캡스락을 눌렀다가 900ms 안에 뗀다.
2. F-07 이 이를 quick press 로 판정한다(임계값 1000ms 이내).
3. F-08 은 `Quick press caps lock to execute:` 규칙에 따라 진짜 caps lock 키다운/키업을 합성해, OS 의 caps lock 잠금 상태(대문자 고정, LED)가 토글된다 — `left control` 로의 리매핑은 이번 입력에는 적용되지 않는다.
4. 다른 시점에 사용자가 캡스락을 1200ms 동안 누르고 있다가 뗀다.
5. F-07 이 이를 quick press 가 아니라고 판정한다.
6. F-08 은 `Remap caps lock to:` 규칙에 따라 눌려 있는 동안 계속 `left control` 키다운을 유지하고, 뗄 때 `left control` 키업을 낸다. OS 의 caps lock 잠금 상태는 전혀 건드리지 않는다.

### 시나리오 B — 캡스락 방향키 조합 (게임 스타일 이동)

전제 (사용자가 켠 구성 — 둘 다 출고 기본값은 ☐): `Caps lock + W A S D = ▲ ◀︎ ▼ ▶︎` = ☑, `Remap caps lock to:` = ☑ `left control`.

1. 사용자가 캡스락을 누른 채로 유지한다.
2. F-07 은 이 눌림을 **물리 키코드** 기준으로 계속 감시한다 — `Remap caps lock to:` 가 caps lock 을 `left control` 로 바꾸도록 설정되어 있어도, 조합 판정은 물리 캡스락 키다운 상태 자체를 본다(§3 "물리 키코드 우선" 규칙).
3. 캡스락을 누른 채로 `W` 를 누른다.
4. F-08 은 `▲`(위 화살표) 키다운/키업을 합성해 낸다. `W` 문자 자체나 `left control+W` 조합은 하위 앱에 전달되지 않는다.
5. 캡스락을 뗀다. 아무 다른 키도 함께 눌리지 않았으므로 §3.2 의 "단독 캡스락" 규칙에 따라 처리된다(시나리오 A 참조).

### 시나리오 C — Hyperkey 와 forward delete 의 결합

전제 (사용자가 켠 구성 — 둘 다 출고 기본값은 ☐): Hyperkey 탭 `Remap key to hyper key:` = ☑ `globe`, `Hyper + delete = forward delete` = ☑.

1. 사용자가 globe(🌐/Fn) 키를 누른다. F-05 의 정의에 따라 hyper 신호(⌃⌥⌘⇧ 합성)가 활성화된다.
2. hyper 가 활성인 상태에서 `delete`(backspace) 를 누른다.
3. F-08 은 "hyper 활성 + delete 다운" 조건이 충족됐음을 보고, forward delete(⌦) 키다운/키업을 합성해 낸다. F-08 은 hyper 의 소스 키가 globe 인지 caps lock 인지 알 필요가 없다 — F-05 가 올려주는 "hyper 활성" 불리언만 구독한다(v1.60 수정 사항, §3 상호작용 표 참조).

## 3. 동작 명세

### 3.1 원칙

- **모든 조합 판정은 물리 키코드(`CGKeyCode`) 기준이다.** `Remap caps lock to:`(F-08.1)로 캡스락이 다른 키로 리매핑되어 있어도, `Caps lock + …` 계열 조합(F-08.4~F-08.7)은 물리 캡스락 키다운/업 이벤트를 계속 감시한다. 이는 superkey-inventory.md §6.2 가 명시한 "단일 리매핑 엔진 + 명시적 우선순위" 요구와 v1.51/v1.52 의 "regardless of keyboard layout" 수정 계열의 연장이다.
- **quick press 판정은 F-07 의 결과를 그대로 받는다.** 어떤 키가 "quick press 되었다"는 판정 자체(타이밍 상태 머신)는 F-07 소관이다. F-08 은 판정 결과(참/거짓)와 눌린 키가 무엇인지만 받아 §3.2 표의 출력 규칙을 적용한다.
- **조합(chord) 프리셋은 modifier 역할 키의 다운을 유지 조건으로, 대상 키의 다운을 트리거로 삼는다.** 예: `Caps lock + space = enter` 는 캡스락이 눌려 있는 **동안** space 가 눌리는 순간 발화하며, 캡스락을 먼저 떼면 이후 space 입력은 조합에 걸리지 않는다.
- **출력은 항상 합성 키 이벤트다.** 프리셋은 원본 이벤트를 소비(consume)하고 대체 이벤트를 새로 발행한다 — 원본과 대체본이 동시에 하위 앱에 전달되지 않는다(F-07 의 `kCGEventTapOptionDefault` 소비 계약, rust-macos-capability-notes.md §2.1).
- **한 물리 키다운 이벤트는 최대 하나의 프리셋 규칙에만 소비된다.** 여러 프리셋이 같은 입력 조건에 반응할 수 있는 경우(§3.3 상호작용 표) 우선순위 규칙으로 정확히 하나만 발화하도록 중재한다.

### 3.2 프리셋 전수 표

> 표기: 팝업 선택지 열에서 **볼드**는 조사 자료로 확인된 값, `(추정)`은 근거 있는 추론 후보(명세를 위해 만들어낸 것이 아니라 인접 UI 패턴에서 유추), `(미확정)`은 후보 자체가 조사로 드러나지 않았거나 해석이 확정되지 않은 항목이다. 확정 불가 항목은 모두 §9 로 승계한다.
> ⭐ **기본값 열은 이번 실측으로 전면 정정되었다.** app-bundle-analysis.md §2.1(defaults 전량 — 프리셋 관련 키가 하나도 없음)과 §6.3(AX 트리 — 16개 전부 ☐)이 서로를 뒷받침한다: 사용자가 손대지 않은 키는 plist 에 존재하지 않고, 존재하지 않는 키는 "부재 = 기본값"으로 동작하며, 그 값이 AX 로 전부 ☐ 로 관찰됐다. 이전 판(F-08.1·2·4·5·9·11·12·14·15·16 을 ☑ 로 기재)은 홍보용 스크린샷 구성을 기본값으로 오인한 것이었다. `superkey-inventory.md` §7 Q3 는 이로써 해소된다.

| ID | 원문 라벨 | 그룹 | 입력 조건 | 출력 | 팝업/슬라이더 선택지 | 기본값 (실측: defaults + AX 트리) | 출처 |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| F-08.1 | `Remap caps lock to:` | caps lock | 물리 caps lock 키다운, 단 F-07 이 quick press 로 판정하지 **않은** 입력(§3.3 규칙 R1) | 선택된 키의 키다운을 눌려 있는 동안 유지, 뗄 때 키업. OS caps lock 잠금 상태(LED·대문자 고정)는 발생하지 않음 — 완전 대체. ⭐ **이벤트 합성이 아니라 커널 레벨 `hidutil` HID `UserKeyMapping`(`HIDKeyboardModifierMappingSrc`/`Dst`, `IOHIDServiceClientSetProperty`)으로 구현될 수 있다**(실측: 번들 문자열·심볼, app-bundle-analysis.md §3.2-3). 메뉴바 `Advanced ▸ Synthesize Caps Lock Remap` 이 이 HID 매핑 대신 이벤트 합성으로 대체하는 스위치로 보인다 `(미확정 — 라벨로부터의 해석)` | **50종 전량 확인**(표시 순서, 실측: AX 트리): `esc` · `nothing (disable it)` · `left control` · `left shift` · `left option` · `left command` · `right control` · `right shift` · `right option` · `right command` · `return (enter)` · `delete (backspace)` · `delete forward` · `tab` · `spacebar` · `home` · `end` · `pageup` · `pagedown` · `left arrow` · `right arrow` · `up arrow` · `down arrow` · `mute` · `volume up` · `volume down` · `F1`~`F24`. ⭐ 이 팝업이 **출력 키 선택**(대상 키)이며 Hyperkey/Seek 탭의 **소스 키 선택** 팝업과 열거형 자체가 다르다는 이전 판의 추정이 **맞았다** — 확정. `nothing (disable it)` 이라는 명시적 무효화 선택지가 있다(캡스락을 완전히 죽이는 용도) | **☐**(전체) / 팝업 표시값 `left control` | app-bundle-analysis.md §2.1, §3.2-3, §6.3; superkey-inventory.md §3.3 그룹1 |
| F-08.2 | `Quick press caps lock to execute:` | caps lock | F-07 이 caps lock 을 quick press 로 판정(§3.3 규칙 R1) | 선택된 키의 키다운+키업을 합성해 발행. ⭐ **단, 선택지가 `Seek` 인 경우 키 합성이 아니라 F-01(seek-activation-and-session.md) 이 정의하는 Seek 세션을 연다** — `Seek` 탭의 단축키·키 리맵 두 활성화 경로 외의 **세 번째 활성화 경로** | **49종 전량 확인**(표시 순서, 실측: AX 트리): ⭐ **`Seek`**(첫 항목) · (구분선) · `esc` · `caps lock` · `left control` · `left shift` · `left option` · `left command` · `right control` · `right shift` · `right option` · `right command` · `return (enter)` · `delete (backspace)` · `delete forward` · `tab` · `spacebar` · `home` · `end` · `pageup` · `pagedown` · `left arrow` · `right arrow` · `up arrow` · `down arrow` · `mute` · `volume up` · `volume down` · `F1`~`F20` · `/`(v1.62 추가) | **☐**(전체) / 팝업 표시값 `caps lock` | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §3.3 그룹1, §2.1 |
| F-08.3 | `Quick press duration` | caps lock | (설정값, 트리거 없음) | F-08.2 의 quick press 판정 임계값을 F-07 에 제공 | 슬라이더. ⭐ **최소 250 ms · 최대 2000 ms 확정**(`AXSlider min=250 max=2000`, 실측: AX 트리). 눈금 간격(step)은 `(미확정)` — `key-remapping-engine.md` 가 "8칸, 200–1600ms, 간격 200ms" 로 역산한 이전 추정은 **틀렸다**, 정정 | **☐**(체크박스 없음, 값 자체는) `1000 ms`(현재값, 실측: AX `AXSlider value=1000`) | app-bundle-analysis.md §6.3; superkey-inventory.md §3.3 그룹1, §7 |
| F-08.4 | `Caps lock + space = enter` | caps lock | 물리 caps lock 다운 유지 중 `space` 다운 | `Enter`(⏎) 키다운/키업 | 없음(단순 체크박스) | **☐** | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §3.3 그룹1 |
| F-08.5 | `Caps lock + W A S D = ▲ ◀︎ ▼ ▶︎` | caps lock | 캡스락 유지 중 `W`/`A`/`S`/`D` 각각 다운 | `W`→▲, `A`→◀, `S`→▼, `D`→▶ 방향키 키다운/키업. ⭐ v1.20 에서 "caps lock 이 hyper 키로 리매핑된 상태에서 동작 안 함" 버그 이력(§3.3 상호작용 표 R2) | **없음**(실측: AX 트리 — WASD 항목에는 팝업이 아예 없다). 그러나 실행 파일에는 `wasdArrowColemak`·`wasdArrowDvorak` 키가 있다 → 레이아웃 변형은 팝업 선택이 아니라 **감지된 키보드 레이아웃에 따라 자동 적용**되는 것으로 보인다 `(미확정 — 팝업 부재로부터의 해석)`. F-14(입력 소스 독립성) 참조 | **☐**(라벨 원문도 정정: 화살표 사이 공백·variation selector 포함, 실측: AX 트리) | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §3.3 그룹1, §2.2 |
| F-08.6 | `Caps lock +` [팝업] ` = ◀︎ ▼ ▲ ▶︎` | caps lock | 캡스락 유지 중 `H`/`J`/`K`/`L` (또는 `I`/`J`/`K`/`L`) 각각 다운 | `H`(또는 `I`)→◀, `J`→▼, `K`→▲, `L`→▶ (vim 방향 관례) | ⭐ **팝업이 문장 중간에 낀다**(`Caps lock +` [팝업] ` = ◀︎ ▼ ▲ ▶︎`, 실측: AX 트리 — 라벨 원문 정정). 선택지 **2종 확정**: `H J K L` · `I J K L`. 이전 판의 "대체 키셋 후보 미확인" 이 이로써 승격된다. 실행 파일에는 `hjklArrowColemak`·`hjklArrowDvorak`·`ijklArrowColemak`·`ijklArrowDvorak` 키가 있으나 팝업에는 없다 → 레이아웃 변형은 감지된 키보드 레이아웃에 따라 자동 적용되는 것으로 보인다 `(미확정 — 팝업 부재로부터의 해석)`. F-14 참조 | **☐** / `H J K L` | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §3.3 그룹1 |
| F-08.7 | `Caps lock + home row = ` [팝업] | caps lock | 캡스락 유지 중 home row 키(`A S D F G H J K L ; '`) 각각 다운 | 확인된 두 스킴의 예시: `symbol row` 스킴에서 `A`→`!`, `function row` 스킴에서 `A`→`F1`. ⭐ 두 스킴의 존재로 F-08.7 의 출력 규칙이 **"홈로우 키를 순서대로 다른 행에 매핑한다"는 일반 규칙**임이 드러난다. 그러나 `A` 외 나머지 홈로우 키(`S D F G H J K L ; '`)의 정확한 매핑은 여전히 **`(미확정)`** — §9 | 팝업이 라벨 뒤에 붙는다(실측: AX 트리 — 라벨 원문 정정). 선택지 **2종 확정**: `symbol row (A = !)` · `function row (A = F1)`. 내부 키 `capsHomeSymbol` / `capsHomeFunction`. 이전 판의 "대체 스킴 존재 여부 미확인" 이 이로써 해소된다 | **☐** / `symbol row (A = !)` | app-bundle-analysis.md §2.1, §2.2, §6.3; superkey-inventory.md §3.3 그룹1 |
| F-08.8 | `Double tap shift = caps lock` | shift | 동일 shift 키(좌/우 특정 여부 미확인)를 F-07 판정 임계 내 두 번 탭 | 진짜 caps lock 키다운/키업 합성 → OS caps lock 잠금 상태 토글. ⭐ **`IOHIDSetModifierLockState` 로 확정**(실측: 번들 심볼, app-bundle-analysis.md §3.2-2) — 단순 키 이벤트 합성이 아니라 HID 잠금 상태를 직접 조작한다 | 없음 | **☐** | app-bundle-analysis.md §2.1, §3.2-2, §6.3; superkey-inventory.md §3.3 그룹2 |
| F-08.9 | `Left shift + right shift = caps lock` | shift | 좌 shift 와 우 shift 가 겹치는 시간 내에 함께 다운 | 진짜 caps lock 키다운/키업 합성 → 잠금 상태 토글(`IOHIDSetModifierLockState` 확정, 위와 동일 근거). 개발자 본인이 상시 사용한다고 밝힌 설정이나(§1.4), **출고 기본값은 아니다** — 정정 | 없음 | **☐**(이전 판은 ☑ 로 오기 — 정정) | app-bundle-analysis.md §2.1, §3.2-2, §6.3; superkey-inventory.md §3.3 그룹2, §1.4 |
| F-08.10 | `Shift + caps lock = caps lock` | shift | shift(좌/우 무관 추정) 유지 중 caps lock 다운 | 진짜 caps lock 키다운/키업 합성 → 잠금 상태 토글(`IOHIDSetModifierLockState` 확정, 위와 동일 근거). ⭐ v1.62 에서 F-08.2 의 quick press 오발 버그와 얽혀 있었음(§3.3 상호작용 표 R3) | 없음 | **☐** | app-bundle-analysis.md §2.1, §3.2-2, §6.3; superkey-inventory.md §3.3 그룹2, §2.1 |
| F-08.11 | `Quick press left or right shift to input corresponding:` | shift | F-07 이 좌 shift 또는 우 shift 각각을 quick press 로 판정 | 좌 shift quick press → 팝업 쌍의 왼쪽 문자, 우 shift quick press → 오른쪽 문자를 합성 입력. 레이아웃 독립(v1.51: "regardless of keyboard layout") | **4종 전량 확인**(실측: AX 트리): `( )` · `[ ]` · `{ }` · `< >`. 이전 판의 "다른 쌍 후보 미확인" 이 이로써 해소된다 | **☐**(이전 판은 ☑ 로 오기 — 정정) / `( )` | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §3.3 그룹2, §2.1 |
| F-08.12 | `Hyper + delete = forward delete` | delete | F-05 정의 hyper 신호 활성 유지 중 `delete`(backspace) 다운 | forward delete(⌦) 키다운/키업. ⭐ v1.60 이전엔 hyper 소스가 globe 키일 때 동작 안 함 — 소스 키에 독립적이어야 함(§3.3 상호작용 표 R4) | 없음 | **☐**(이전 판은 ☑ 로 오기 — 정정) | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §3.3 그룹3, §2.1 |
| F-08.13 | `Remap delete to forward delete` | delete | 물리 `delete`(backspace) 단독 다운(modifier 없음) | forward delete(⌦) 키다운/키업 | 없음 | **☐** | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §3.3 그룹3 |
| F-08.14 | `Shift + delete = forward delete` | delete | shift 유지 중 `delete`(backspace) 다운 | forward delete(⌦) 키다운/키업. Windows 의 "완전 삭제" 관례와 무관 — 순수 키코드 치환일 뿐, 실제 파일 영구삭제 여부는 대상 앱 소관 | 없음 | **☐**(이전 판은 ☑ 로 오기 — 정정) | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §3.3 그룹3 |
| F-08.15 | `Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):` | 기타 | ⌘ 유지 중 `V` 다운, 단 팝업이 지정한 ⌘ 변형일 때만(단, `Hyper key` 선택 시는 아래 참조) | ⌘⌥⇧+V 합성 키 이벤트("서식 없이 붙여넣기"). 레이아웃 독립(v1.52) | **4종 전량 확인**(실측: AX 트리): `Right ⌘` · `Left ⌘` · `Either ⌘` · **`Hyper key`**. ⭐ 이전 판이 "어느 물리 ⌘ 가 트리거인가를 고르는 위젯" 이라고 추정한 것이 **맞았고**, 여기에 `Hyper key` 라는 네 번째 선택지가 더 있다 — hyper 활성 중 `V` 로 서식 없이 붙여넣기. F-08.12 에 이어 **F-05 의 hyper 신호를 구독하는 두 번째 프리셋**이다(§3.3 R4 와 나란히 다룸) | **☐**(이전 판은 ☑ 로 오기 — 정정) / `Right ⌘` | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §3.3 그룹4, §2.1 |
| F-08.16 | `Home & end operate on lines` | 기타 | 물리 `Home`/`End` 단독 다운 | "줄" 단위로 커서 이동(제목의 "lines" 명시). 정확히 어떤 키 이벤트로 합성되는지(예: ⌘←/⌘→ 대체, 또는 다른 방식) `(미확정)` — §9 | 없음 | **☐**(이전 판은 ☑ 로 오기 — 정정) | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §3.3 그룹4 |

**조건부로만 나타나는 Presets 항목** (nib 에는 있으나 기본 상태 AX 트리에는 없음 — 실측: 번들 문자열 + AX 트리):
- `Apply hyper to arrows` (`applyHyperToArrowsCheckbox`, 키 `applyHyperToCapsArrows`·`capsArrowsOverrideModifiers`) — `Caps lock + W A S D` 만 켜서는 나타나지 않았다(§7 변경 이력 #4). 표시 조건 `(미확정)`. 이름으로 보아 "caps lock 방향키 프리셋 사용 중에도 hyper modifier 를 함께 얹을지"의 스위치로 보이며, 이는 §3.3 R2(v1.20 버그)가 다룬 바로 그 충돌 지점이다.
- Windows 키보드 리매핑(`winKeyRemapCheckbox`) — 라벨·표시 조건 모두 `(미확정)`.

### 3.3 상호작용·충돌 표

| # | 충돌하는 프리셋/기능 쌍 | 충돌 유형 | 중재 규칙 | 근거(버전) |
| :--- | :--- | :--- | :--- | :--- |
| R1 | F-08.1 (`Remap caps lock to:`) ↔ F-08.2 (`Quick press caps lock to execute:`) | 같은 물리 키(caps lock)의 같은 다운/업 이벤트를 두 규칙이 동시에 후보로 삼음 | F-07 의 quick press 판정이 **먼저** 실행된다. 판정이 참이면 F-08.2 가 발화하고 F-08.1 은 적용되지 않는다(리매핑된 키다운을 애초에 내보내지 않는다 — hold 도중 출력이 났다가 취소되는 게 아니라 처음부터 F-08.2 경로로만 처리). 판정이 거짓(quick press 임계 초과)이면 F-08.1 이 발화한다. 둘 다 꺼져 있으면 캡스락은 원래 기능(잠금 토글)을 그대로 낸다 | 설계상 필연 — 두 프리셋이 같은 그룹에 나란히 존재하는 이유 |
| R2 | F-08.5/F-08.6/F-08.7 (캡스락 조합 계열) ↔ Hyperkey `Remap key to hyper key:` = caps lock, `Apply hyper to arrows` | 캡스락이 Hyperkey 의 소스 키로도 지정되어 있으면, 캡스락 다운이 "hyper 활성화"와 "조합 modifier 유지" 두 의미를 동시에 요구받음 | v1.20 은 이 조합에서 캡스락 조합 프리셋이 아예 발화하지 않는 버그였다(원문: "caps lock + keys being mapped to arrow keys wasn't working when caps lock was remapped as the hyper key"). ⭐ **정정**: 이 자리에서 이전 판이 "명시적 우선순위 규칙(자동)이 있어야 한다"고 추정했던 것은, 이번 실측으로 밝혀진 §3.3 R8(설정 충돌 감지 대화상자)에 비추어 보면 **적어도 일부는 자동 우선순위가 아니라 대화형 배타 선택으로 구현되어 있을 가능성이 높다** — 실행 파일 문자열에 `Conflict: Caps lock arrows` / `Disable that setting and enable WASD arrows?` 가 그대로 존재한다(app-bundle-analysis.md §4.1). 다만 이 대화상자가 정확히 "캡스락 조합 계열 ↔ Hyperkey 소스"쌍을 다루는지, 아니면 "WASD ↔ HJKL"처럼 조합 프리셋끼리의 충돌만 다루는지는 실측으로 재현하지 못했다 `(미확정)`. `Apply hyper to arrows`(§3.2 조건부 항목)의 이름도 이 지점의 절충안으로 보이나 표시 조건이 `(미확정)`이라 확정하지 못한다. R8·F-15(settings-store-and-integrity.md) 참조 | v1.20 (2023-07-14); 대화상자 실재는 app-bundle-analysis.md §4.1 |
| R3 | F-08.10 (`Shift + caps lock = caps lock`) ↔ F-08.2 (`Quick press caps lock to execute:`) | `Shift + caps lock = caps lock` 발화의 캡스락 다운/업이 동시에 quick press 판정 창 안에 들어가면, F-08.2 도 오발할 수 있었음(버그) | v1.62 수정 후: F-08.10 의 조합이 성립한 것으로 판정된 캡스락 다운/업 이벤트는 F-08.2 의 quick press 후보에서 **제외**된다(원문: "\"Shift + caps lock = caps lock\" will no longer trigger a quick press caps lock keypress"). 즉 shift 가 함께 눌려 있었다는 사실이 quick press 판정을 무효화하는 조건으로 F-07 에 전달되어야 한다 | v1.62 (2026-06-03) |
| R4 | F-08.12 (`Hyper + delete = forward delete`) ↔ Hyperkey 소스 키 종류 | F-08.12 가 "hyper 신호"가 아니라 특정 소스 키(예: 특정 modifier)의 물리 상태에 잘못 결합되어 있으면, 소스 키가 globe 처럼 modifier 가 아닌 키일 때 동작하지 않음 | 수정 후: F-08.12 는 F-05 가 노출하는 **"hyper 활성"이라는 논리 신호**만 구독하고, 그 신호를 만든 물리 소스 키가 무엇인지는 알지 못한다(원문: "will now work if you have the hyper key set to the globe key"). 이 설계라야 향후 hyper 소스 후보가 늘어나도 F-08.12 를 수정할 필요가 없다 | v1.60 (2026-03-05) |
| R5 | F-08.15 (paste w/o formatting) / F-08.11 (bracket quick press) ↔ 키보드 레이아웃 | 문자 기반(예: `V` 라는 글자, `(` 라는 기호)으로 판정하면 비-QWERTY·비영문 레이아웃에서 다른 물리 키가 눌려야 같은 출력이 남 | v1.51/v1.52 수정 후: 입력 판정은 물리 키코드로, 출력 합성은 목표 **문자**(유니코드)로 고정한다 — "타이핑되는 것처럼 보이는 글자"가 레이아웃과 무관하게 항상 동일해야 한다는 것이 수정의 의미다 | v1.51, v1.52 (2025-07) |
| R6 | F-08.13 (`Remap delete to forward delete`) ↔ F-08.12 / F-08.14 | 세 프리셋 모두 delete 키를 최종적으로 forward delete 로 바꾸는 목적이 겹침 | 배타적이지 않다 — 입력 조건이 다르므로(F-08.13: 단독, F-08.12: hyper+, F-08.14: shift+) 셋을 동시에 켜도 오류는 아니다. 다만 F-08.13 이 켜져 있으면 delete 키는 modifier 유무와 무관하게 이미 항상 forward delete 만 내므로, F-08.12/F-08.14 는 **관측 가능한 차이를 만들지 않는 중복 규칙**이 된다(§5 엣지케이스 10) | 설계상 필연 |
| R7 | F-08.8/F-08.9/F-08.10 (캡스락 토글 3계열) ↔ 서로 | 셋 다 최종 출력(진짜 caps lock 토글)이 동일하지만 입력 조건(더블탭 / 양쪽 동시 / shift+caps lock)이 서로 다름 | 배타적이지 않다 — 세 조건이 겹치지 않는 물리 제스처이므로 동시에 켜도 각자 독립적으로 발화한다. 단, 한 제스처가 우연히 두 조건을 동시에 만족시킬 가능성(예: 양쪽 shift 를 빠르게 두 번 두드림)은 조사로 확정되지 않았다(§9) | `(추정)` — 조사 자료 직접 근거 없음 |
| R8 | ⭐ **설정 충돌 감지 대화상자** — caps lock 을 두고 경쟁하는 설정 3종 | `Remap caps lock to:` 류의 caps lock 리매핑 중복 / caps lock 방향키 프리셋(F-08.5·F-08.6) 간 충돌 / caps lock home row 프리셋(F-08.7) 충돌 — 이전 판이 "조용히 무시되는 상태는 없어야 한다"고 **요구사항으로만** 적었던 것이, 원본에서는 **실제 대화상자**로 구현되어 있다(실측: 번들 문자열, app-bundle-analysis.md §4.1) | 원문: `Conflict: Caps lock already remapped` / `Conflict: Caps lock arrows` / `Conflict: Caps lock + home row` / `Disable other remappings and continue with this one?` / `Disable that setting and enable WASD arrows?` / `Disable that setting and enable caps + home row as`. 해소 방식은 **"상대 설정을 끄고 이걸 켤까요?" 라고 사용자에게 묻는 대화형 배타 선택**이다 — 자동 우선순위가 아니다. 대화상자 자체의 상세 명세(정확한 트리거 쌍, 버튼 구성, 문구 전체)는 새 문서 `F-15`(settings-store-and-integrity.md, 별도 담당)가 다룬다 — 여기서는 이 메커니즘이 실재한다는 사실과 F-08 프리셋과의 연관만 기록한다. 실제 버튼 구성은 관찰하지 못했다(§9) | app-bundle-analysis.md §4.1 |

**클론이 이 대화형 해소를 채택해야 하는가 — 채택 권고.** F-08 관점에서 세 가지 근거로 채택을 권한다. (1) caps lock 은 F-08(Presets)·Hyperkey·Seek 세 탭이 동시에 소스로 지정할 수 있는 유일한 물리 키이고(§5 엣지케이스 2), 이 경쟁이 조용히 한쪽만 이기면 사용자는 "왜 안 먹지"를 스스로 디버깅해야 한다 — 원본이 굳이 전용 대화상자 3종을 만들어 이 마찰을 없앤 것은 우연이 아니다. (2) 자동 우선순위 규칙(예: "나중에 켠 설정이 이긴다")은 §3.3 R2 가 지적했듯 규칙 자체가 명세에 없으면 구현마다 달라질 위험이 있는데, 대화형 배타 선택은 그 규칙을 사용자의 마지막 선택으로 명시적으로 고정한다. (3) 세 대상(리매핑 중복·방향키·home row)이 전부 "caps lock 을 modifier 로 쓰는 프리셋들끼리"로 좁혀져 있어 구현 범위가 작다. 다만 대화상자의 정확한 트리거 조건(예: R2 에서 남긴 "Hyperkey 소스 caps lock ↔ 방향키 프리셋"도 포함되는지)과 버튼 구성은 F-15 가 실측을 더 하기 전까지 F-08 쪽에서 확정할 수 없다 — 이 판단은 채택 여부에 대한 것이지 구현 세부에 대한 것이 아니다.

## 4. 설정 항목

⭐ **정정.** 이전 판은 스크린샷의 체크 상태를 "출고 기본값이라는 보장이 없다"며 전부 `(추정)`으로 남겼는데, 이번 실측으로 **출고 기본값이 확정**되었다: `~/Library/Preferences/com.knollsoft.Superkey.plist` 를 아무 설정도 바꾸지 않은 채로 읽으면 프리셋 관련 키가 **하나도 없고**(`hyperFlags`·`minAxCharCount`·업데이트/라이선스 관련 키 7개만 존재, app-bundle-analysis.md §2.1), AX 트리 실측도 16개 항목 **전부 ☐**로 나왔다(§6.3). "설정 키 부재 = 기본값 = OFF" 이므로 두 근거가 서로를 뒷받침하며, 이로써 스크린샷 값(F-08.1·2·4·5·9·11·12·14·15·16 을 ☑ 로 보여준 것)은 **홍보용으로 켜 둔 구성**이었음이 확정된다. `superkey-inventory.md` §7 Q3 는 이로써 해소되었다. 아래 표의 "기본값" 열은 실측값으로 교체한다.

| 이름(원문 라벨) | 타입 | 기본값 (실측: defaults + AX 트리) | 유효 범위 | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| `Remap caps lock to:` | 체크박스 + 팝업 | **☐** / 팝업 표시값 `left control` | 출력 키 열거형 **50종 확정**(§3.2 F-08.1) | app-bundle-analysis.md §2.1, §6.3 |
| `Quick press caps lock to execute:` | 체크박스 + 팝업 | **☐** / 팝업 표시값 `caps lock` | 출력 후보 **49종 확정**, 첫 항목 `Seek`(§3.2 F-08.2) | app-bundle-analysis.md §2.1, §6.3 |
| `Quick press duration` | 슬라이더 | 현재값 `1000 ms`, **최소 250 ms · 최대 2000 ms 확정** | 간격(step) `(미확정)`(§9) | app-bundle-analysis.md §6.3 |
| `Caps lock + space = enter` | 체크박스 | **☐** | ☑/☐ | app-bundle-analysis.md §2.1, §6.3 |
| `Caps lock + W A S D = ▲ ◀︎ ▼ ▶︎` | 체크박스 | **☐** | ☑/☐. 팝업 없음(§3.2 F-08.5) | app-bundle-analysis.md §2.1, §6.3 |
| `Caps lock +` [팝업] ` = ◀︎ ▼ ▲ ▶︎` | 체크박스 + 인라인 팝업 | **☐** / `H J K L` | 팝업 **2종 확정**: `H J K L` · `I J K L` | app-bundle-analysis.md §2.1, §6.3 |
| `Caps lock + home row = ` [팝업] | 체크박스 + 인라인 팝업 | **☐** / `symbol row (A = !)` | 팝업 **2종 확정**: `symbol row (A = !)` · `function row (A = F1)` | app-bundle-analysis.md §2.1, §6.3 |
| `Double tap shift = caps lock` | 체크박스 | **☐** | ☑/☐ | app-bundle-analysis.md §2.1, §6.3 |
| `Left shift + right shift = caps lock` | 체크박스 | **☐**(이전 판은 ☑ 로 오기 — 정정) | ☑/☐. 개발자 본인 상시 사용 설정이나 출고 기본값은 아니다(§1.4) | app-bundle-analysis.md §2.1, §6.3; superkey-inventory.md §1.4 |
| `Shift + caps lock = caps lock` | 체크박스 | **☐** | ☑/☐ | app-bundle-analysis.md §2.1, §6.3 |
| `Quick press left or right shift to input corresponding:` | 체크박스 + 팝업 | **☐**(이전 판은 ☑ 로 오기 — 정정) / `( )` | 문자 쌍 **4종 확정**: `( )` · `[ ]` · `{ }` · `< >` | app-bundle-analysis.md §2.1, §6.3 |
| `Hyper + delete = forward delete` | 체크박스 | **☐**(이전 판은 ☑ 로 오기 — 정정) | ☑/☐ | app-bundle-analysis.md §2.1, §6.3 |
| `Remap delete to forward delete` | 체크박스 | **☐** | ☑/☐ | app-bundle-analysis.md §2.1, §6.3 |
| `Shift + delete = forward delete` | 체크박스 | **☐**(이전 판은 ☑ 로 오기 — 정정) | ☑/☐ | app-bundle-analysis.md §2.1, §6.3 |
| `Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):` | 체크박스 + 팝업 | **☐**(이전 판은 ☑ 로 오기 — 정정) / `Right ⌘` | 팝업 **4종 확정**: `Right ⌘` · `Left ⌘` · `Either ⌘` · `Hyper key` | app-bundle-analysis.md §2.1, §6.3 |
| `Home & end operate on lines` | 체크박스 | **☐**(이전 판은 ☑ 로 오기 — 정정) | ☑/☐ | app-bundle-analysis.md §2.1, §6.3 |

조건부로만 나타나는 항목 2개(`Apply hyper to arrows`, Windows 키보드 리매핑)는 16개 기본 목록에 포함되지 않으며 표시 조건이 `(미확정)`이라 별도 표로 두지 않는다 — §3.2 조건부 항목, §9 참조.

## 5. 엣지 케이스와 실패 모드

1. **서로 배타적이지 않은 프리셋의 동시 활성 — 캡스락 토글 3계열 중복 발화.** F-08.8/F-08.9/F-08.10 이 모두 켜진 상태에서, 양쪽 shift 를 거의 동시에 두 번 빠르게 두드리는 제스처는 "더블탭"(F-08.8)과 "좌+우 동시"(F-08.9) 조건을 함께 만족시킬 수 있다. 이 경우 캡스락 토글이 두 번 발화해 결과적으로 원래 상태로 되돌아가 버리는 "무효화" 증상이 관측 가능하다. 두 규칙 중 어느 쪽이 우선하는지, 또는 짧은 시간 내 중복 토글을 F-07 이 흡수(디바운스)하는지는 조사로 확정되지 않았다(§9).
2. **`Remap caps lock to:` 와 Seek/Hyperkey 의 caps lock 배정 충돌.** `Remap caps lock to:`(F-08.1), Hyperkey 탭 `Remap key to hyper key:`, Seek 탭 `Remap key to Seek:` 세 곳 모두 캡스락을 소스로 지정할 수 있다. 셋 중 둘 이상이 동시에 캡스락을 지정하면 하나의 물리 키다운 이벤트를 세 기능이 경쟁한다. ⭐ **정정**: F-08.1 이 이 경쟁의 당사자인 경우는 §3.3 R8 이 확인한 `Conflict: Caps lock already remapped` 대화상자가 "조용한 패배"가 아니라 **사용자에게 묻고 상대 설정을 끄는 방식**으로 해소한다(실측: 번들 문자열, app-bundle-analysis.md §4.1). F-08 은 이 대화상자 자체를 정의하지 않는다 — 상세는 F-15 소관이며, F-08 관점에서는 "F-08.1 이 이 경쟁에 관여하면 조용히 지는 것이 아니라 확인 대화상자가 뜬다"는 사실만 기록한다. Seek/Hyperkey 두 소스가 서로 겹치는 경우까지 이 대화상자가 다루는지는 `(미확정)`(§9).
3. **quick press 임계 경계.** `Quick press duration`(F-08.3) 이 `1000 ms` 일 때 정확히 1000ms 에 뗀 입력이 quick press 로 판정되는지(경계 포함 여부)는 F-07 소관이나, F-08.1/F-08.2 R1 규칙이 이 판정에 전적으로 의존하므로 경계 오차 1ms 차이로 완전히 다른 출력(리매핑된 키 vs quick press 키)이 나는 것이 사용자 관점에서는 "가끔 안 먹는다"는 체감으로 나타난다.
4. **키 반복(auto-repeat) 중 조합 성립.** 캡스락을 길게 누르고 있으면 OS 가 caps lock 다운 이벤트를 반복 발행할 수 있다(설정에 따라). F-08.4~F-08.7 의 조합 판정이 "캡스락 다운 시점"이 아니라 "캡스락 다운 상태" 를 modifier 조건으로 삼도록 설계되지 않으면, auto-repeat 다운마다 조합이 재평가되어 의도치 않은 재발화가 생길 수 있다. F-08 은 "다운 상태 유지"를 조건으로 명시했으므로(§3.1), auto-repeat 다운 이벤트 자체는 새로운 트리거로 취급하지 않아야 한다 — 단 이 억제 로직이 F-07 의 물리 키 상태 추적에 있는지 F-08 쪽에 있는지는 구현 시 확정 필요.
5. **비-QWERTY 레이아웃 — WASD/HJKL 은 위치 기반, 다른 프리셋은 문자 기반.** F-08.5(`W A S D`)와 F-08.6(`H J K L`)은 **물리 키 위치**(게임 이동 키 관례, vim 방향 관례)로 의미가 고정되어 있어 레이아웑이 바뀌어도 물리 위치 그대로 판정해야 한다(예: AZERTY 에서도 물리적으로 QWERTY 의 W 위치에 해당하는 키가 트리거). 반대로 F-08.11(괄호)·F-08.7(기호행)·F-08.15(paste)는 **출력 문자**가 레이아웃과 무관하게 고정돼야 한다(v1.51/v1.52, R5). 두 원칙이 프리셋마다 다르게 적용된다는 점 자체가 실수하기 쉬운 지점이다.
6. **`home row`/`symbol row` 가 레이아웃마다 다름.** F-08.7 의 "home row" 는 QWERTY 기준 `A S D F G H J K L ; '` 이지만, 물리 키 위치 기준으로 다른 레이아웃(예: Dvorak, AZERTY)에서는 이 문자 집합 자체가 다른 물리 키에 배치되어 있다. F-08.7 이 "물리 위치의 홈로우"를 의미하는지 "논리 문자로서의 A S D F…"를 의미하는지 조사로 확정되지 않았다(§9) — 전자라면 출력 기호도 레이아웃마다 달라질 수 있고, 후자라면 물리 키 위치가 사용자 기대(홈로우에 손가락을 얹은 상태)와 어긋날 수 있다.
7. **Secure Input.** 암호 필드가 Secure Input 을 활성화하면 F-07 의 이벤트 탭 자체가 키 입력 내용을 알 수 없다(superkey-inventory.md §1.3). ⭐ **재검토 필요 — 정정.** 이전 판은 "16개 프리셋 전부가 이 구간에서 완전히 무력화된다 — 예외 없음"이라고 단정했으나, F-08.1(`Remap caps lock to:`)이 `CGEventTap` 이벤트 합성이 아니라 커널 레벨 `hidutil` HID `UserKeyMapping` 으로 구현될 수 있다는 사실(§3.2 F-08.1, app-bundle-analysis.md §3.2-3)이 이 단정을 흔든다 — **HID 레벨 매핑 경로는 Secure Input 구간에서도 영향을 받지 않을 가능성이 있다** `(미확정 — 단정하지 않는다. `CGEventTap`으로 구현된 나머지 15개 프리셋과 hidutil로 구현될 수 있는 F-08.1 은 이 구간에서 서로 다르게 동작할 수 있다는 뜻일 뿐, 실측으로 검증되지 않았다)`. 캡스락 조합(F-08.4~7)을 누른 채로 Secure Input 이 걸리는 필드에 포커스가 넘어가는 경계 상황(예: 캡스락을 누르고 있는 도중 사용자가 클릭으로 암호 필드에 포커스를 옮김)에서 F-07 이 "눌려 있던 캡스락"을 뗄 때까지 계속 조합 후보로 유지할지, 즉시 무효화할지도 확정되지 않았다.
8. **외장 키보드에 right ⌘ 가 없음.** F-08.15 의 팝업 값이 `Right ⌘`(오른쪽 커맨드 키 물리 변형)을 트리거 조건으로 삼는다는 해석이 맞다면, 오른쪽 커맨드가 물리적으로 없거나(일부 컴팩트 키보드) OS 가 좌우를 구분하지 않고 같은 keycode 로 보고하는 외장 키보드에서는 이 프리셋이 발화 자체가 불가능하다. 이 경우 사용자에게 "이 키보드에서는 이 프리셋이 동작하지 않는다"는 신호를 줄 수단이 명세에 없다(F-09 의 UI 소관이나 F-08 이 이 사실 자체는 규정해야 한다).
9. **`Home & end operate on lines` 가 앱마다 다르게 해석됨.** 많은 macOS 앱(터미널, 코드 에디터, 브라우저)이 `Home`/`End` 를 자체 단축키 핸들러로 가로채거나, F-08.16 이 합성해 내는 대체 키 이벤트(예: ⌘←/⌘→ 계열로 추정, §9)를 그 앱이 다른 의미(예: 브라우저 뒤로가기)로 처리할 수 있다. F-08 은 OS 레벨에서 합성 이벤트를 발행하는 것까지만 책임지며, 하위 앱이 그 합성 이벤트를 어떻게 소비하는지는 F-08 의 통제 밖이다 — "합성된 이벤트가 원래 앱에서 다른 동작으로 해석될 수 있다"는 사실 자체가 이 프리셋의 근본적 한계다.
10. **forward delete 3종(F-08.12/13/14)의 동시 활성.** 셋 다 켜져 있으면 R6 에서 서술했듯 delete 키는 modifier 조합과 무관하게 항상 forward delete 만 낸다. 이 상태에서 사용자가 진짜 backspace(뒤로 삭제)를 낼 수단이 전혀 남지 않는다 — F-08.13 단독으로 이미 이 상태가 되며, 나머지 두 프리셋은 관측 가능한 차이를 만들지 않는다. 세 프리셋을 동시에 켜는 것을 막을지, 경고할지는 F-09(설정 UI) 소관이나 F-08 은 이 결과(진짜 backspace 소실 가능)를 사실로 기록해 둔다.
11. **`Quick press duration` 슬라이더를 세션 중 변경.** 캡스락을 누르고 있는 도중(아직 떼지 않은 상태에서) 사용자가 설정 창에서 슬라이더 값을 바꾸면, 이미 진행 중인 눌림에 새 임계값이 소급 적용되는지 다음 눌림부터 적용되는지는 F-07/F-09 의 구현 세부에 달려 있으며 조사로 확정되지 않았다.
12. ⭐ **해소됨 — caps lock 잠금 상태(LED) 토글 API.** F-08.8/F-08.9/F-08.10(캡스락 토글 3계열)이 "진짜 caps lock 토글"을 만드는 방법이 단순 `CGEvent` 키다운/업 합성이 아니라 **`IOHIDGetModifierLockState`/`IOHIDSetModifierLockState`(IOKit)로 확정**되었다(실측: 번들 심볼, app-bundle-analysis.md §3.2-2) — HID 잠금 상태(대문자 고정 + LED)를 직접 읽고 쓴다. 이전 판이 §9 로 남겼던 미해결 질문(§9 옛 Q11)은 이로써 해소되며, 이 사실은 §3.2 F-08.8/9/10 행과 §6 API 표에도 반영했다.

## 6. 필요한 플랫폼 API

F-08 은 F-07 이 설치·유지하는 `CGEventTap` 콜백 위에서, 물리 키다운/업 이벤트와 quick press 판정 결과를 소비해 **대체 키 이벤트를 합성**하는 소비자다. F-08 이 직접 요구하는 것은 다음과 같다.

- **키 이벤트 소비 + 합성 발행** — `CGEventTapCreate` 콜백이 원본 이벤트를 `NULL` 반환(소비)하고 `CGEventCreateKeyboardEvent`/`CGEventPost` 로 대체 이벤트를 발행하는 표준 패턴. 근거: `core-graphics` 0.25.0 의 `CGEventTap` 안전 래퍼, 또는 `objc2-core-graphics` 0.3.2 의 원시 바인딩(rust-macos-capability-notes.md §1.1, §2.1). 방향키·Enter·forward delete·control 등은 고정 `CGKeyCode` 로 합성 가능하다.
- **레이아웃 독립적 문자 출력** — F-08.7(기호행), F-08.11(괄호), F-08.15(paste 합성 자체는 modifier 조합이라 문제 없음) 처럼 **특정 문자**가 항상 나와야 하는 프리셋은 `CGKeyCode` 대신 `CGEventKeyboardSetUnicodeString` 로 목표 유니코드 문자열을 직접 얹는 방식이 v1.51/v1.52 의 "regardless of keyboard layout" 요구를 만족시키는 유일한 방법으로 보인다 `(추정 — rust-macos-capability-notes.md 가 이 API 를 직접 언급하지 않음, §9)`. `objc2-core-graphics` 가 이 함수를 노출하는지는 확인이 더 필요하다.
- **hyper 활성 신호 구독** — F-08.12 는 F-05 가 정의하는 hyper modifier 합성 상태(⌃⌥⌘⇧)를 소스 키 종류와 무관하게 구독해야 한다(R4). 이는 새 플랫폼 API 가 아니라 F-05/F-07 이 노출하는 내부 인터페이스 계약의 문제다.
- **caps lock 잠금 상태 토글** — ⭐ **확정(실측: 번들 심볼, app-bundle-analysis.md §3.2-2)**. F-08.8/F-08.9/F-08.10 이 "진짜 caps lock 토글"을 내는 방법은 `CGEvent` 키다운/업 합성이 아니라 IOKit `IOHIDGetModifierLockState`/`IOHIDSetModifierLockState` 다. 더 이상 커버리지 공백이 아니다 — Rust 에서 `#[link(name = "IOKit", kind = "framework")]` 로 링크한 뒤 `extern "C"` 로 직접 선언해야 하는 API 로 확정한다(§7).
- **caps lock 리매핑의 HID 커널 매핑 경로** — ⭐ **신규(실측: 번들 문자열·심볼, app-bundle-analysis.md §3.2-3)**. F-08.1(`Remap caps lock to:`)은 `CGEventTap` 이벤트 합성이 아니라 커널 레벨 `hidutil` HID `UserKeyMapping`(`HIDKeyboardModifierMappingSrc`/`HIDKeyboardModifierMappingDst`, `IOHIDServiceClientSetProperty`)으로 구현될 수 있다. 셸로 `hidutil property -g UserKeyMapping` 을 호출하는 경로와 `IOHIDServiceClientSetProperty` 를 직접 호출하는 경로 둘 다 문자열·심볼로 확인된다 — 어느 쪽이 실제 활성 경로인지, 또는 메뉴바 `Advanced ▸ Synthesize Caps Lock Remap` 스위치로 전환되는 두 경로인지는 `(미확정)`.
- **Input Monitoring / Accessibility 권한 전제** — F-07 이 `CGEventTap` 을 설치하기 위한 전제 조건(rust-macos-capability-notes.md §2.5)이며 F-08 은 권한이 이미 있다고 가정한다. 권한 흐름은 F-11(범위 밖으로 추정) 소관.

## 7. 구현 접근

**판정: Rust 바인딩.**

F-08 의 16개 규칙 각각은 "물리 키코드 조합을 감지하면 정해진 이벤트를 합성한다"는 순수 조건-반응 로직으로, 애플리케이션 계층에서는 플랫폼 API 를 직접 호출하지 않는 것처럼 보인다. 그러나 실제로는 두 지점에서 플랫폼 타입·API 에 직접 맞닿는다.

1. **입력 판정과 출력 합성 모두 `CGEvent`/`CGKeyCode` 타입에 의존한다.** `core-graphics` 0.25.0 과 `objc2-core-graphics` 0.3.2 가 이 타입과 `CGEventCreateKeyboardEvent`/`CGEventPost` 를 이미 제공하므로(rust-macos-capability-notes.md §1.1, §2.1), 대부분의 프리셋(F-08.1~F-08.6, F-08.8~F-08.10, F-08.12~F-08.16)은 이 크레이트만으로 구현 가능하다.
2. **레이아웃 독립적 문자 합성(F-08.7, F-08.11)은 조사 문서가 커버리지를 확인하지 못한 API(`CGEventKeyboardSetUnicodeString`)에 의존할 가능성이 있다.** caps lock 잠금 상태 토글(F-08.8/9/10)은 더 이상 가능성이 아니라 **`IOHIDGetModifierLockState`/`IOHIDSetModifierLockState`(IOKit)로 확정**되었다(실측: 번들 심볼, app-bundle-analysis.md §3.2-2). 나아가 F-08.1(`Remap caps lock to:`)도 이벤트 합성이 아니라 커널 레벨 `hidutil` HID `UserKeyMapping`(`IOHIDServiceClientSetProperty`)으로 구현될 수 있다는 사실이 새로 드러났다(app-bundle-analysis.md §3.2-3, §6 참조). 이는 rust-macos-capability-notes.md §2.5 가 Input Monitoring TCC 에 대해 내린 판단(전용 크레이트가 없어 IOKit 을 직접 링크하고 `extern "C"` 선언을 손으로 써야 한다)과 같은 패턴이다 — Swift/Objective-C 로 별도 shim 프로세스나 바이너리를 작성할 필요는 없고, `#[link(name = "...", kind = "framework")]` 로 프레임워크를 링크한 뒤 필요한 함수를 Rust 에서 직접 `extern "C"` 선언하면 된다. 따라서 이 부분도 "네이티브 shim 불가피"가 아니라 **"Rust 바인딩(수기 FFI 선언 포함)"** 범주에 든다.

- **기각한 대안 1 — "순수 Rust".** 조건 판정 로직 자체는 플랫폼 독립적으로 보이지만, 입력 판정의 최소 단위가 `CGKeyCode`/`CGEventFlags` 이고 출력 합성이 `CGEvent` 구조체를 직접 다루므로 F-08 의 코드는 시작부터 플랫폼 타입 위에서 동작한다. "순수 Rust" 로 분류하면 F-07 과의 경계(어디까지가 F-07 이 제공하는 이벤트 타입이고 어디부터 F-08 이 합성하는 새 이벤트인지)가 흐려진다.
- **기각한 대안 2 — "네이티브 shim 불가피".** §6 에서 표시한 커버리지 공백(유니코드 문자 합성)과 확정된 API(caps lock 잠금 상태, hidutil HID 매핑)는 모두 macOS 프레임워크의 C API 이며, Objective-C 런타임 메시지 전송이 필요한 것이 아니라 순수 C 함수 호출이거나 셸 호출(`hidutil`)이다. `objc2` 생태계나 별도 Swift 코드 없이 Rust `extern "C"` 로 직접 링크(또는 `std::process::Command` 로 `hidutil` 셸 호출)하는 방식이 가능하다는 점에서 F-01 의 판단과 같은 논리로 shim 이 불가피하다고 볼 근거가 없다.

## 8. 수용 기준

- [ ] `Remap caps lock to:` 가 켜져 있고 quick press 로 판정되지 않은 캡스락 누름/뗌에서, 물리 캡스락 대신 팝업에 지정된 키의 다운/업이 합성되고 OS caps lock 잠금 상태는 변하지 않는다.
- [ ] `Quick press caps lock to execute:` 가 켜져 있고 `Quick press duration` 이내에 캡스락을 눌렀다 떼면, 팝업에 지정된 키(예: `caps lock` 또는 `/`)의 다운+업이 합성된다.
- [ ] `Remap caps lock to:` 와 `Quick press caps lock to execute:` 가 동시에 켜져 있을 때, 같은 캡스락 다운/업 이벤트가 두 규칙 중 정확히 하나에만 소비된다(R1) — quick press 판정 결과에 따라 배타적으로 갈린다.
- [ ] `Caps lock + space = enter` 가 켜진 상태에서 캡스락을 누른 채 space 를 누르면 Enter 가 합성되고, space 문자 자체나 두 키의 원본 조합은 하위 앱에 전달되지 않는다.
- [ ] `Caps lock + W A S D = ▲ ◀︎ ▼ ▶︎` 가 켜진 상태에서 캡스락을 누른 채 W/A/S/D 를 각각 누르면 대응하는 방향키(▲◀▼▶)가 각각 합성된다.
- [ ] Hyperkey 탭에서 caps lock 이 hyper 소스 키로 지정된 상태에서도, `Caps lock + W A S D`/`Caps lock + [H J K L]`/`Caps lock + home row` 세 프리셋이 정상 발화한다(R2, v1.20 회귀 방지).
- [ ] `Left shift + right shift = caps lock` 이 켜진 상태에서 좌우 shift 를 겹치는 시간 내에 함께 누르면 진짜 caps lock 토글이 발생한다(잠금 상태가 실제로 바뀐다).
- [ ] `Shift + caps lock = caps lock` 과 `Quick press caps lock to execute:` 가 동시에 켜져 있을 때, shift 를 누른 채 캡스락을 눌렀다 떼는 입력이 caps lock 토글만 발생시키고 quick press 출력(F-08.2)은 발화하지 않는다(R3, v1.62 회귀 방지).
- [ ] `Quick press left or right shift to input corresponding:` 이 켜진 상태에서 좌/우 shift 를 각각 quick press 하면 팝업 쌍의 왼쪽/오른쪽 문자가 각각 정확히 합성되며, 비-QWERTY 키보드 레이아웃에서도 동일한 문자가 나온다(v1.51 회귀 방지).
- [ ] `Hyper + delete = forward delete` 가 켜진 상태에서, hyper 소스 키가 caps lock 이든 globe 든 관계없이 hyper 활성 + delete 입력이 forward delete 를 합성한다(R4, v1.60 회귀 방지).
- [ ] `Remap delete to forward delete` 가 켜진 상태에서 delete(backspace) 단독 입력이 항상 forward delete 로 치환되고, 어떤 modifier 조합으로도 원래의 backspace 를 낼 수 없다(R6/엣지케이스 10 을 알려진 제약으로 문서화하는 것이 목적이며, 이 기준은 실제 그렇게 동작함을 확인한다).
- [ ] `Remap paste (⌘+V) to paste w/o formatting (⌘⌥⇧+V):` 가 켜진 상태에서 ⌘+V 입력이 키보드 레이아웃과 무관하게 항상 서식 없는 붙여넣기 단축키(⌘⌥⇧+V)로 치환된다(v1.52 회귀 방지).
- [ ] `CGEventTap` 경로로 구현되는 15개 프리셋은 Secure Input 이 활성화된 입력 필드에 포커스가 있는 동안 발화하지 않는다(원본 키 입력이 그대로 그 필드에 도달한다). `Remap caps lock to:` 가 HID `UserKeyMapping` 경로로 구현될 경우 이 구간에서의 동작은 §5 엣지케이스 7 의 `(미확정)` 표시를 그대로 따르며, 이 기준은 그 경로가 실제로 결정된 뒤 별도로 확정한다.

## 9. 미해결 질문

### 9.1 이번 실측으로 해소됨

- ~~16개 프리셋의 출고 기본값~~ → **전부 ☐ 로 확정**. app-bundle-analysis.md §2.1(defaults 전량) + §6.3(AX 트리) — 본 문서 §3.2, §4.
- ~~`Remap caps lock to:` 팝업의 출력 키 전체 열거형~~ → **50종 확정**. app-bundle-analysis.md §6.3 — 본 문서 §3.2 F-08.1.
- ~~`Quick press caps lock to execute:` 출력 후보 전체 목록~~ → **49종 확정**(첫 항목 `Seek`, 끝 항목 `/`). app-bundle-analysis.md §6.3 — 본 문서 §3.2 F-08.2.
- ~~`Quick press duration` 슬라이더의 최소·최대~~ → **250 ms / 2000 ms 확정**(간격은 §9.2 로 존속). app-bundle-analysis.md §6.3 — 본 문서 §3.2 F-08.3.
- ~~`Caps lock + [H J K L]` 인라인 팝업의 대체 후보~~ → **`H J K L` · `I J K L` 2종 확정**. app-bundle-analysis.md §6.3 — 본 문서 §3.2 F-08.6.
- ~~`Caps lock + home row` 인라인 팝업의 대체 스킴~~ → **`symbol row (A = !)` · `function row (A = F1)` 2종 확정**(각 키 정확한 매핑은 §9.2 로 존속). app-bundle-analysis.md §6.3 — 본 문서 §3.2 F-08.7.
- ~~`Quick press left or right shift` 출력 문자 쌍 후보~~ → **`( )`·`[ ]`·`{ }`·`< >` 4종 확정**. app-bundle-analysis.md §6.3 — 본 문서 §3.2 F-08.11.
- ~~`Remap paste (⌘+V)` 팝업의 의미~~ → **물리 ⌘ 선택 위젯이라는 추정이 확정되었고, `Hyper key` 라는 4번째 선택지도 확인됨**. app-bundle-analysis.md §6.3 — 본 문서 §3.2 F-08.15.
- ~~caps lock 잠금 상태(LED) 토글이 `IOHIDSetModifierLockState` 류 API 를 필요로 하는지~~ → **필요함, `IOHIDGetModifierLockState`/`IOHIDSetModifierLockState` 로 확정**. app-bundle-analysis.md §3.2-2 — 본 문서 §3.2 F-08.8/9/10, §6, §7.

### 9.1-bis ⭐ M2 2차 구현이 내린 결정 (2026-08-30, 이슈 #15)

아래는 **해소(사실 확정)가 아니라 결정**이다 — 원본을 더 관찰해서 알아낸 것이 아니라, 이 클론이
근거를 대고 고른 값이다. 그래서 §9.1(실측으로 해소됨)과 분리해 적는다. 전문은
[`../dev/architecture.md` §6](../dev/architecture.md) 에 있다.

| 질문(§9.2) | 이 구현이 고른 것 | 근거 |
| :--- | :--- | :--- |
| #2 `Caps lock + home row` 의 `A` 외 매핑 | 홈로우 11키(`A S D F G H J K L ; '`)를 **숫자행 시프트 기호에 순서대로** — `! @ # $ % ^ & * ( ) _` / 함수행에 순서대로 — `F1`…`F11` | §3.2 F-08.7 이 확정한 **"홈로우를 다른 행에 순서대로 매핑"이라는 일반 규칙**에, 실측된 두 점(`A = !`·`A = F1`)을 얹으면 이 배열이 유일한 자연스러운 보간이다. 지어낸 것이 아니다. ⚠️ **원본이 실제로 같은 배열인지는 여전히 미확인** |
| #7 `Home & end operate on lines` 의 출력 | `Home → ⌘←` · `End → ⌘→` | macOS 에서 `Home`/`End` 는 **문서** 처음/끝, `⌘←`/`⌘→` 는 **줄** 처음/끝이다. 라벨의 "operate on lines" 가 정확히 이 차이를 가리킨다 |
| #1 `Quick press duration` 슬라이더 step | 50 ms | 실측된 것은 범위(250~2000)와 현재값(1000)뿐이다. 기본값 1000 이 눈금에 정확히 떨어지는 값 중 가장 세밀한 것을 골랐다 |
| #8 캡스락 토글 3계열의 겹치는 제스처 | **F-08.9(좌우 동시)가 F-08.8(double tap)보다 우선**하고, 발화 시 좌·우 shift 상태 머신을 **둘 다 억제**해 중복 토글을 원천 차단한다 | 동시성(서로 다른 두 키가 겹쳐 눌림)이 반복성(한 키를 두 번)보다 **더 구체적인 조건**이다. **기각한 대안 — 시간 디바운스**: 임계값이 또 하나의 `(미확정)` 상수가 되는데, 억제만으로 이미 해소된다 |
| #3 Colemak/Dvorak 변형 | **구현하지 않는다** | WASD·HJKL 은 §5 엣지 5 가 확정했듯 **물리 위치 기반**이다. "레이아웃에 따라 자동 적용"은 팝업 부재로부터의 해석일 뿐이고, 물리 위치 기반 판정과 정면으로 어긋난다 |
| #4 `Apply hyper to arrows` | **구현하지 않는다** | 표시 조건 자체가 `(미확정)`. 이 구현의 기본 동작(조합 출력에 hyper flags 를 얹지 않음, `architecture.md` §6.4 P5)이 그 체크박스가 꺼진 상태에 해당한다 |
| #5 Windows 키보드 리매핑 | **구현하지 않는다** | 라벨·표시 조건 둘 다 `(미확정)` |
| #6 충돌 대화상자의 트리거 쌍·버튼 구성 | 트리거 쌍 3종을 `architecture.md` §6.5 로 확정하고, 문구는 **우리 것을 쓴다**(원본 문자열을 옮기지 않는다). 버튼은 2개(`계속`·`취소`) | 원본의 버튼 구성을 관찰하지 못했으므로 재현이 아니라 설계다. ⭐ **caps lock 조합 프리셋과 "hyper 소스 = caps lock" 은 충돌로 다루지 않는다** — §8 이 "동시에 성립해야 한다"를 수용 기준으로 못박았기 때문이다(R2/v1.20) |

⭐ **§5 #20(caps lock 래칭)이 이 문서의 전제를 바꿨다.** M2 1차 실기기 검증에서 **caps lock 은
누를 때만 `flagsChanged` 를 보내고 뗄 때는 보내지 않는다**는 것이 실측됐다
(`key-remapping-engine.md` §5 #20). 이 문서의 캡스락 그룹 7종은 전부 "caps lock 이 눌린 채
유지" 를 조건으로 하므로, 경로 A 만으로는 **7종 전부가 실기기에서 성립하지 않는다.**
이 구현은 `architecture.md` §6.1 **결정 D-1** 로 이를 푼다 — caps lock 의존 규칙이 하나라도
켜지면 경로 B 로 `caps lock → F18` 커널 매핑을 설치하고, 중재기가 F18 을 caps lock 으로
되돌려 판정한다. **되돌리는 수단**은 메뉴바 `Advanced ▸ Synthesize Caps Lock Remap` 이다.

### 9.2 남은 질문

| # | 질문 | 조사 문서 연결 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | `Quick press duration` 슬라이더의 눈금 간격(step) — 최소 250 ms·최대 2000 ms 는 확정, 간격만 미확정 | app-bundle-analysis.md §6.3 (범위만 확정) | 앱 설치 후 슬라이더를 드래그하며 값 변화 폭 관찰, 또는 AX `AXSlider` 의 `AXStepSize` 유사 속성 조회 |
| 2 | `Caps lock + home row` 의 `A` 외 나머지 홈로우 키(`S D F G H J K L ; '`)의 정확한 매핑 — `symbol row`/`function row` 두 스킴 존재와 "홈로우를 다른 행에 순서대로 매핑"이라는 일반 규칙은 확정됐으나 각 키별 대응은 미확정 | app-bundle-analysis.md §6.3 (예시 1개만 제공) | 앱 설치 후 프리셋을 켜고 홈로우 각 키 실동작 확인 |
| 3 | `Caps lock + W A S D`/`Caps lock + [H J K L]` 의 Colemak/Dvorak 레이아웃 변형(`wasdArrowColemak` 등 내부 키)이 감지된 키보드 레이아웃에 따라 정말 자동 적용되는지, 아니면 다른 트리거가 있는지 | app-bundle-analysis.md §6.3 (팝업 부재로부터의 해석일 뿐) | Colemak/Dvorak 입력 소스로 전환 후 실동작 비교. F-14(localization-and-input-sources.md) 참조 |
| 4 | `Apply hyper to arrows`(`applyHyperToArrowsCheckbox`)의 표시 조건 — `Caps lock + W A S D` 단독으로는 나타나지 않았다 | app-bundle-analysis.md §6.3, §7 변경 이력 #4 | 다른 프리셋 조합(예: Hyperkey 소스를 caps lock 으로 지정 + WASD 동시 활성)으로 재현 시도 |
| 5 | Windows 키보드 리매핑(`winKeyRemapCheckbox`)의 라벨·표시 조건 | app-bundle-analysis.md §2.2, §6.3 (nib 존재만 확인) | Windows 키보드(PC 106/109 키 배열) 연결 후 Presets 탭 재관찰 |
| 6 | 설정 충돌 감지 대화상자(§3.3 R8)의 실제 버튼 구성·문구 전체, 그리고 정확히 어느 설정 쌍이 각 대화상자를 트리거하는지(예: R2 의 "캡스락 조합 계열 ↔ Hyperkey 소스" 가 포함되는지) | app-bundle-analysis.md §4.1 (문자열 존재만 확인, §8 한계 7) | caps lock 을 실제로 리매핑해 충돌 조건을 재현(부작용 있음 — 실측 시 주의). 상세 명세는 F-15(settings-store-and-integrity.md) 소관 |
| 7 | `Home & end operate on lines`(F-08.16)이 정확히 어떤 키 이벤트로 합성되는지(⌘←/⌘→ 대체 등) | 조사 문서에 직접 근거 없음 | 앱 설치 후 텍스트 에디터에서 실동작 확인, 키 이벤트 로깅 |
| 8 | `Double tap shift`/`Left+right shift`/`Shift+caps lock` 세 캡스락 토글 계열이 동시에 켜져 있을 때 겹치는 제스처의 우선순위·디바운스 여부(엣지케이스 1) | 조사 문서에 직접 근거 없음 | 앱 설치 후 실동작 확인 |
| 9 | Seek/Hyperkey/Presets 세 탭이 동시에 caps lock 을 소스로 지정했을 때의 전역 우선순위 규칙(엣지케이스 2) — R8 의 충돌 대화상자가 이 경쟁의 전부 또는 일부를 다룰 가능성이 있으나 정확한 범위는 미확정 | app-bundle-analysis.md §4.1 (대화상자 실재는 확인, 정확한 트리거 쌍은 미확인) | F-07/F-15 명세에서 확정 필요 — F-08 은 이 표를 그 결정에 종속시킨다 |
| 10 | HID `UserKeyMapping` 경로(F-08.1)가 Secure Input 구간에서 실제로 영향을 받지 않는지 — §5 엣지케이스 7 참조 | app-bundle-analysis.md §3.2-3 (API 존재만 확인, Secure Input 과의 상호작용은 미측정) | Secure Input 이 걸리는 필드(예: 로그인 암호창)에서 `Remap caps lock to:` 실동작 확인 |
