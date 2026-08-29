# F-15 · 설정 저장 모델과 무결성 (충돌 감지 · 클라우드 동기화)

> 한 줄 요약: 설정값을 둘러싼 세 가지 무결성 문제 — (1) 저장소가 "부재 = 기본값"으로 동작하는 규약, (2) caps lock 을 두고 경쟁하는 설정 3종의 충돌 감지·대화형 해소, (3) 설정의 iCloud 동기화 — 를 다룬다. **(1)만 실측으로 확정됐고, (2)·(3)은 실행 파일 문자열로 존재만 확인했을 뿐 대응 UI 를 관찰하지 못했다.** 얇은 근거 위에 두꺼운 명세를 쌓지 않기 위해, 이 문서는 확정된 것과 `(미확정)`인 것을 각 절에서 냉정하게 구분한다.
> 의존성: `F-09`(`preferences-ui.md`) — §3.6 이 이미 "자체 JSON 저장소(`tauri-plugin-store`), `NSUserDefaults` 미사용"을 결정했다. 이 문서는 그 결정을 재론하지 않고, 그 위에 얹히는 **저장 규약**(부재=기본값)과 **저장값을 둘러싼 무결성 이벤트**(충돌 대화상자, 클라우드 동기화)만 다룬다. `F-08`(`power-user-presets.md`) §3.3 R8 — caps lock 충돌 대화상자의 존재를 F-08 이 먼저 기록했고, 상세는 이 문서(F-15) 소관으로 위임받았다. `F-05`(`hyperkey.md`) §5 — Seek/Hyperkey 소스 키 충돌 후보를 지적했다.
> 관련 명세: `F-07`(`key-remapping-engine.md`, event tap 중재 — 이 문서가 다루는 "설정 충돌"은 이벤트 중재 우선순위가 아니라 **설정을 저장하는 시점의 사용자 확인**이라는 점에서 F-07 의 런타임 중재와는 다른 층위다).

⭐ **근거의 한계**: 이 문서가 다루는 세 주제를 하나로 묶은 이유는 전부 "설정값을 둘러싼 무결성"이라는 하나의 관심사이기 때문이다 — 저장 규약(값이 없을 때 무엇이 진실인가), 저장 시점의 충돌(서로 다른 설정이 같은 자원을 요구할 때 무엇을 저장해야 하는가), 저장소의 다중 위치 정합성(로컬 파일과 클라우드 사본이 다를 때 무엇이 진실인가)은 모두 "지금 저장된 값을 믿을 수 있는가"라는 같은 질문의 변형이다. **기각한 대안**: 세 주제를 각각 독립 문서(예: `settings-storage.md`, `settings-conflicts.md`, `icloud-sync.md`)로 쪼개는 안을 고려했으나 기각했다 — (2)·(3)은 근거가 문자열 수준으로 얇아서, 각각을 독립 문서로 만들면 "9절 구조를 채워야 한다"는 압박 때문에 §2 사용자 시나리오·§4 설정 항목 같은 절을 추측으로 채우게 될 위험이 크다. 하나의 문서에서 세 주제를 나란히 놓으면 "이 주제는 이만큼만 안다"는 근거 수준의 차이가 한눈에 드러나고, 얇은 절은 얇은 채로 정직하게 남길 수 있다.

---

## 1. 개요

SuperKey 는 설정을 표준 `NSUserDefaults`(plist)에 저장한다. 이 저장소를 실측으로 조사하는 과정에서 세 가지가 드러났다.

**(1) 설정 저장 모델 — 실측 확정.** 사용자가 아무 설정도 건드리지 않은 상태의 plist 에는 키가 **7개**밖에 없다(app-bundle-analysis.md §2.1). 즉 SuperKey 의 설정 저장소는 "한 번이라도 건드린 항목만 기록하고, 나머지는 키 자체가 없다 — 없으면 기본값"이라는 규약으로 동작한다. 이 성질 자체가 실측 도구이기도 했다: 이번 조사에서 각 설정의 출고 기본값을 확정할 수 있었던 것은 정확히 이 성질(plist 를 비우고 UI 를 읽으면 기본값을 알 수 있다) 덕분이다.

**(2) 설정 충돌 감지와 해소 — 문자열로 존재만 확인.** 실행 파일에 `Conflict: Caps lock already remapped` 류의 문자열 7개가 있어, caps lock 을 두고 경쟁하는 설정 3종에 대해 "상대 설정을 끄고 이걸 켤까요?"라고 묻는 대화상자가 존재한다는 것은 확정된다. 그러나 이 조사는 caps lock 을 실제로 리매핑해 충돌 조건을 재현하지 않았으므로(부작용이 크다고 판단해 보류, app-bundle-analysis.md §8 한계 7), 대화상자의 실제 모습·버튼 구성·정확한 트리거 조건은 관찰하지 못했다.

**(3) iCloud 설정 동기화 — 문자열로 존재만 확인.** `Do you want to import your existing iCloud configuration?` 류의 문자열과 `syncAfterEachWrite` 등의 키로, 설정을 클라우드로 동기화하고 첫 실행 시 사용자에게 가져올지/올릴지 묻는 기능이 있다는 것은 확정된다. 그러나 이 기능을 켜고 끄는 UI 를 4개 탭 어디에서도 찾지 못했고, 저장소가 `NSUbiquitousKeyValueStore` 인지도 심볼로 확인되지 않았다.

**범위 밖**: 저장 기술 자체의 선택(JSON vs `NSUserDefaults` 등)은 `F-09`(preferences-ui.md §3.6)가 이미 결정했다 — 이 문서는 재론하지 않는다. `Presets`/`Hyperkey`/`Seek` 개별 프리셋의 동작은 각 소유 명세(F-05, F-08) 소관이며, 이 문서는 그 설정들이 충돌할 때의 **저장 시점 해소 절차**만 다룬다.

---

## 2. 사용자 시나리오

무리해서 세 주제 모두를 채우지 않는다. 근거가 있는 시나리오만 적는다.

### 시나리오 A — 출고 기본값을 확인하려는 QA (실측 방법론 그 자체)

QA 담당자가 "이 체크박스의 출고 기본값이 켜짐인지 꺼짐인지"를 확인하고 싶다. 클론의 설정 파일을 완전히 빈 상태(또는 파일 자체가 없는 상태)로 만들고 앱을 실행한 뒤, UI 에 표시되는 값을 읽는다. 저장소가 "부재 = 기본값" 규약을 지키는 한, 이 방법은 항상 정확하다 — 코드를 읽지 않고도 기본값을 검증할 수 있다. 반대로 저장소가 항상 모든 키를 파일에 쓰는 방식이었다면, 이 검증 방법 자체가 성립하지 않는다.

### 시나리오 B — 설정을 켰다 끄자 파일에 흔적이 남는다 (잔여물)

사용자가 `Seek using macOS accessibility` 를 켰다가 다시 끈다. UI 상으로는 원래 상태(꺼짐)로 완전히 돌아왔지만, 저장 파일에는 이전에 없던 `seekOptions = 1`(app-bundle-analysis.md §7.1 실측)이 새로 기록된다. 사용자는 이 차이를 인지하지 못하고, 기능적으로도 체감할 수 없다 — 하지만 "저장 파일을 열어 무엇을 건드렸는지 감사(audit)하려는" 사용자나 지원 담당자에게는 "껐다 켰다"의 흔적이 "한 번도 안 건드림"과 구분되지 않게 된다.

### 시나리오 C — caps lock 을 두고 설정이 경쟁한다 (충돌, 대화상자의 존재만 확정)

사용자가 `Presets` 탭에서 `Remap caps lock to: left control` 을 켠 상태에서, `Hyperkey` 탭의 `Remap key to hyper key:` 도 caps lock 으로 지정하려 한다. SuperKey 원본은 이 시도를 조용히 받아들이거나 조용히 실패시키지 않고, `Conflict: Caps lock already remapped` 류의 문구로 "기존 설정을 끄고 새 설정을 켤지" 사용자에게 확인을 구하는 것으로 보인다(실측: 번들 문자열). ⚠️ 이 시나리오의 "확인을 구하는 것으로 보인다"까지는 문자열 존재로 뒷받침되지만, 대화상자가 정확히 어떤 문구·버튼으로 뜨는지는 관찰하지 못했다 — §3.2, §9.

---

## 3. 동작 명세

### 3.1 ⭐ 설정 저장 모델 — 실측 확정

- **저장소**: `~/Library/Preferences/com.knollsoft.Superkey.plist` (표준 `NSUserDefaults`). 비샌드박스(`com.apple.security.app-sandbox` 미선언, app-bundle-analysis.md §1.1).
- ⭐ **"부재 = 기본값" 모델**: 사용자가 아무 설정도 바꾸지 않은 상태의 plist 에는 **키가 7개뿐**이다(`hyperFlags`·`lastVersion`·`minAxCharCount`·`NSWindow Frame EntryBarWindow`·`Paddle-Superkey-750314-SD`·`SUEnableAutomaticChecks`·`SUHasLaunchedBefore`, app-bundle-analysis.md §2.1). 한 번이라도 건드린 항목만 plist 에 값이 생기고, 나머지 모든 설정(`Remap key to hyper key`, 16개 프리셋 등)은 키 자체가 없다 — 값이 없을 때의 기본값으로 동작한다.
- 첫 실행 시 시드되는 두 값: `hyperFlags = 1966080`(= `0x1E0000` = `kCGEventFlagMaskShift`\|`Control`\|`Alternate`\|`Command`, 즉 `⌃⌥⌘⇧` 4비트가 모두 켜진 `CGEventFlags` 마스크), `minAxCharCount = 2`. hyper 조합은 불리언 여러 개가 아니라 `CGEventFlags` 마스크 하나로, `Match on more than one character` 는 불리언이 아니라 최소 글자 수 정수로 저장된다.
- ⭐ **이 성질 자체가 검증 도구다**: 기본값을 확인하려면 plist 를 비우고 UI 를 읽으면 된다(§2 시나리오 A). 이번 조사의 다른 모든 F-0N 문서가 "출고 기본값"을 실측으로 확정할 수 있었던 것은 이 규약 덕분이다.
- ⭐ **잔여물 사례(§2 시나리오 B)**: 설정을 켰다 껐더니 UI 값은 원래대로 돌아왔지만, plist 에는 없던 키 4개가 기본값과 동등한 값으로 남았다 — `capsWasdArrows = 2` · `oneSwipeFromTop = 2` · `seekOptions = 1` · `seekRemapKeycode = 0`(app-bundle-analysis.md §7.1). 즉 **"기본값으로 되돌림"과 "키 삭제"가 SuperKey 원본에서는 구분된다** — 되돌리기는 키를 지우지 않고, 기본값과 동등한 값을 다시 쓸 뿐이다.
- 열거형 값이 정수로 저장된다. `seekOptions` 는 비트를 여러 개 담는 `OptionSet` 으로 보이고(`(미확정)`), `capsWasdArrows`/`oneSwipeFromTop` 은 `2` 가 "꺼짐"을 뜻하는 열거값으로 보인다(`(미확정)`) — 두 관찰 모두 값이 하나씩만 관찰되어(잔여물 사례 1건씩) 다른 값과 대조해 확정하지 못했다.

**클론의 결정: 저장 계층도 같은 성질(부재 = 기본값, 기본값은 파일에 쓰지 않음)을 가져야 한다.**

근거: (1) 이 성질 자체가 §2 시나리오 A 의 검증 방법론을 가능하게 한다 — 클론의 QA·지원 과정에서도 "설정 파일을 비우고 UI 를 읽으면 기본값을 알 수 있다"는 방법이 성립해야 디버깅·문서화 비용이 줄어든다. (2) 저장 파일이 사용자가 실제로 건드린 항목만 담으면, 파일 크기가 작고 스키마 마이그레이션(§5) 대상이 최소화된다 — 모든 키를 항상 쓰는 방식이었다면 새 버전마다 필드 전체를 순회하며 마이그레이션해야 하지만, 부재=기본값 모델에서는 "존재하는 키만" 마이그레이션하면 된다.

**"되돌리면 키를 지울 것인가, 남길 것인가"의 결정**: 클론은 **원본과 같이 지우지 않는다(write-through, 삭제 없음)**를 채택한다. 근거: (1) 구현 단순성 — "이 값이 기본값과 같아졌으니 키를 삭제한다"는 로직은 각 필드의 기본값을 저장 계층이 알고 있어야 하고, 매 쓰기마다 비교 연산이 필요하다. 반면 "바뀐 값을 그대로 쓴다"는 단순 write-through 는 그런 비교가 필요 없다. (2) 원본의 실측 사례(§2 시나리오 B)가 이미 "기능적으로 동등하고 사용자에게 관찰되지 않는 잔여물"임을 보여준다 — 즉 이 잔여물은 실질적 피해가 없는 부작용이다. (3) 반대급부로 감사 추적(audit trail) 정확도가 장기적으로 낮아지는 단점은 인정한다 — 이 트레이드오프를 §9 에 남긴다.

**F-09 와의 경계**: `preferences-ui.md` §3.6 은 저장 **기술**(JSON, `tauri-plugin-store`)을 결정했다. 이 문서는 그 파일이 지켜야 할 **규약**(부재=기본값, 삭제 없는 write-through)만 정한다 — 저장 포맷·경로·마이그레이션 실행 메커니즘의 상세는 F-09 소관이며 중복 명세를 만들지 않는다.

### 3.2 ⭐ 설정 충돌 감지와 해소 — 문자열로 존재 확정

실행 파일 문자열(SuperKey 원문, 실측: 번들 문자열, app-bundle-analysis.md §4.1):

```
Conflict: Caps lock already remapped
Conflict: Caps lock arrows
Conflict: Caps lock + home row
Disable other remappings and continue with this one?
Disable that setting and enable WASD arrows?
Disable that setting and enable caps + home row as
Remap caps lock to
```

**확정되는 것**:
- 충돌 감지 대상은 **3종**이다 — caps lock 리매핑 중복(`Remap caps lock to:` 류가 서로 겹침) / caps lock 방향키 프리셋(`Caps lock + W A S D`, `Caps lock + [H J K L]`) / caps lock home row 프리셋(`Caps lock + home row =`).
- 해소 방식은 **"상대 설정을 끄고 이걸 켤까요?"라고 사용자에게 묻는 대화형 배타 선택**이다 — 자동 우선순위 규칙이 아니다.
- ⭐ 이것은 `superkey-inventory.md` §6.2 와 `power-user-presets.md` §3.3 이 **요구사항으로만("충돌이 조용히 무시되어서는 안 된다")** 적었던 것의 **실제 구현**이다.

**충돌 규칙 표 — 실측된 3종만 확정으로 표기**:

| # | 트리거 (경쟁하는 설정 쌍) | 원문(SuperKey) | 해소 방식 | 근거 |
| :--- | :--- | :--- | :--- | :--- |
| C1 | caps lock 리매핑 중복 — `Remap caps lock to:` 등 caps lock 을 소스로 지정하는 설정끼리 겹침 | `Conflict: Caps lock already remapped` / `Disable other remappings and continue with this one?` | 사용자 확인 후 기존 설정 비활성화, 새 설정 활성화 | app-bundle-analysis.md §4.1 |
| C2 | caps lock 방향키 프리셋 충돌 — `Caps lock + W A S D`, `Caps lock + [H J K L]` 계열 | `Conflict: Caps lock arrows` / `Disable that setting and enable WASD arrows?` | 상동 | 상동 |
| C3 | caps lock home row 프리셋 충돌 — `Caps lock + home row =` | `Conflict: Caps lock + home row` / `Disable that setting and enable caps + home row as` | 상동 | 상동 |

**`(미확정)` — 위 표에 넣지 않은 것들**:
- 대화상자의 실제 모습·버튼 구성·문구 전문. 충돌 조건을 만들려면 caps lock 을 실제로 리매핑해야 해 이번 조사에서는 재현하지 않았다.
- 감지가 **저장 시점**(설정을 켜는 순간)인지 **토글 시점**(체크박스를 누르는 즉시)인지 구분되지 않는다.
- `Conflict:` 3종 외의 충돌 — 예를 들어 `hyperkey.md` §5 가 지적한 **Seek 소스 키와 Hyperkey 소스 키의 중복**, `power-user-presets.md` §3.3 R2 가 지적한 **캡스락 방향키 프리셋 ↔ Hyperkey 소스 caps lock** 조합 — 이 대화상자로 처리되는지, 아니면 별도 메커니즘(또는 처리되지 않음)인지 확정하지 못했다.

**클론이 추가로 다뤄야 할 것(원본에서 실측되지 않은 충돌 후보)** — 아래는 이 문서가 새로 정의하는 것이 아니라, 다른 명세가 이미 지적한 후보를 여기 모아 "F-15 가 상세를 다뤄야 할 목록"으로만 표기한다:
- Seek 탭 `Remap key to Seek:` ↔ Hyperkey 탭 hyper/meh/bleh 소스 키 중복(`hyperkey.md` §5 엣지 케이스 2).
- 캡스락 방향키/home row 프리셋 ↔ Hyperkey `Remap key to hyper key: = caps lock` 조합(`power-user-presets.md` §3.3 R2, §5 엣지 케이스 2).

이 후보들이 실제로 C1~C3 대화상자로 처리되는지, 별도 대화상자가 있는지, 아니면 처리되지 않는지는 §9 미해결 질문이다.

### 3.3 ⭐ iCloud 설정 동기화 — 문자열로 존재 확정

실행 파일 문자열(SuperKey 원문, 실측: 번들 문자열, app-bundle-analysis.md §4.3):

```
Do you want to import your existing iCloud configuration?
No, upload my current configuration
Incoming cloudConfigTimestamp:
overriding existing cloud config
```

관련 키: `syncAfterEachWrite` · `config` · `customUUID` · `GlobalDefaults` · `CodableDefault` · `IntEnumDefault` · `DoubleDefault` · `StringDefault` · `OptionalBoolDefault` · `OptionSetDefault`.

**확정되는 것**:
- 설정을 클라우드로 동기화하는 기능이 존재한다.
- 기존 클라우드 설정이 있으면 **가져올지**(`Do you want to import your existing iCloud configuration?`) 또는 **현재 설정을 올릴지**(`No, upload my current configuration`) 사용자에게 묻는다.
- 타임스탬프(`Incoming cloudConfigTimestamp:`)로 최신성을 판정하며, 로컬이 최신이면 `overriding existing cloud config` 로 클라우드를 덮어쓰는 경로가 있다.
- 쓰기마다 즉시 동기화하는 옵션(`syncAfterEachWrite`)이 존재한다 — 이는 "매번 동기화할지, 별도 트리거(예: 앱 종료·주기적)로 동기화할지"를 사용자가 고를 수 있다는 뜻으로 보인다 `(미확정 — 옵션명으로부터의 해석, 반대 옵션이 무엇인지는 관찰 못함)`.
- `CodableDefault`·`IntEnumDefault`·`DoubleDefault`·`StringDefault`·`OptionalBoolDefault`·`OptionSetDefault` 라는 이름들은 특정 설정 키라기보다 **타입별 프로퍼티 래퍼(property wrapper)의 이름**으로 보인다 `(미확정 — 이름 패턴으로부터의 해석)` — 맞다면 SuperKey 내부는 값의 타입(정수 열거형, 실수, 문자열, 선택적 불리언, 비트 OptionSet)마다 다른 래퍼로 `NSUserDefaults` 값을 읽고 쓰며, 그 래퍼가 클라우드 동기화 여부까지 함께 처리하는 것으로 추측된다. 이 구조 자체가 §3.1 의 "부재=기본값" 모델과 iCloud 동기화가 **같은 값 읽기/쓰기 계층**을 공유할 가능성을 시사하지만, 확정할 근거는 없다.

**`(미확정)`**:
- 저장소가 `NSUbiquitousKeyValueStore` 인지 — 그 심볼(`NSUbiquitousKeyValueStore` 클래스, `synchronize`, `didChangeExternallyNotification` 등)을 확인하지 못했다.
- 동기화 대상 범위(전체 설정인지 일부인지).
- 충돌 해소(가져오기/올리기 선택)가 첫 실행 1회뿐인지, 이후에도 클라우드-로컬 불일치가 생길 때마다 다시 묻는지.
- 이 기능을 켜고 끄는 UI. 4개 탭(`Seek`/`Hyperkey`/`Presets`/`General`) 어디에도 없다(app-bundle-analysis.md §6).

**클론이 이 기능을 범위에 넣을지의 판단**: 이 문서는 채택 여부를 결정하지 않는다 — 근거가 문자열 수준으로 얇아 UI·정확한 동작을 모르는 상태에서 구현 범위를 정하는 것은 추측으로 명세를 채우는 것과 같다. 다만 판단에 필요한 사실 하나는 짚어 둔다: **이 기능이 애플의 iCloud Key-Value Store 를 쓴다면 Apple Developer 계정과 `com.apple.developer.ubiquity-kvstore-identifier` entitlement 가 필요한데, SuperKey 의 entitlement 에는 그런 키가 없다**(`com.apple.security.cs.allow-jit` 하나뿐, app-bundle-analysis.md §1.1). 문자열은 있는데 entitlement 가 없다는 것은 (a) 이 기능이 v1.66 시점에는 비활성이거나, (b) `NSUbiquitousKeyValueStore` 가 아닌 다른 저장소(자체 백엔드 등)를 쓴다는 뜻일 수 있다 — **어느 쪽인지 단정하지 않는다.** §9 미해결 질문으로 남긴다.

---

## 4. 설정 항목

이 문서가 소유하는 **사용자 노출 설정은 관찰된 것이 없다.**

- 충돌 대화상자(§3.2)는 설정 항목이 아니라 **이벤트**다 — 사용자가 값을 저장·조회하는 대상이 아니라, 저장을 시도할 때 조건부로 나타나는 확인 절차다.
- iCloud 동기화(§3.3)를 켜고 끄는 UI 는 찾지 못했다 — 있다면 어느 탭에 속하는지도 확정할 수 없다.
- §3.1 의 "부재=기본값" 규약은 저장 계층의 내부 동작이지, 사용자가 조작하는 설정이 아니다.

따라서 이 절은 향후 UI 를 관찰하기 전까지 빈 상태로 둔다.

---

## 5. 엣지 케이스와 실패 모드

1. **설정 파일 손상** — JSON(클론) 또는 plist(원본)가 손상되면 파싱이 실패한다. §3.1 의 "부재=기본값" 모델을 따르는 저장 계층이라면, 손상된 파일을 통째로 버리고 빈 상태(=전부 기본값)로 시작하는 것이 자연스러운 복구 경로다 — 이는 `preferences-ui.md` §5 항목 3 이 이미 다룬 마이그레이션 실패 대응과 같은 패턴이다.
2. **스키마 마이그레이션** — `lastVersion` 키가 실재한다(app-bundle-analysis.md §2.1, 값 `"66"`). 이는 SuperKey 가 버전 간 마이그레이션 판별에 이 값을 쓴다는 방증이다. §3.1 의 "부재=기본값" 모델과 결합하면, 마이그레이션 로직은 "이번 버전에서 새로 추가된 키가 옛 버전 파일에 아예 없다"는 것을 "기본값을 채워 넣어야 한다"는 신호로 자연스럽게 해석할 수 있다 — 필드 삭제·이름 변경의 경우는 이 신호만으로 부족하며 별도 마이그레이션 규칙이 필요하다.
3. **동기화 충돌** — 두 기기에서 각각 설정을 바꾼 뒤 양쪽이 동기화를 시도하면, §3.3 의 타임스탬프 비교로는 "동시에 다른 값으로 바뀐 두 설정"이 하나를 완전히 덮어쓰는 결과가 된다(마지막 쓰기 우선, last-write-wins). 필드 단위 병합이 있는지는 `(미확정)`.
4. **잔여 키** — §3.1 의 잔여물 사례(§2 시나리오 B)가 보여주듯, "값이 기본값과 같아짐"과 "키가 삭제됨"이 다르다. 클론이 §3.1 의 결정(삭제 없는 write-through)을 따르면, 저장 파일을 직접 읽어 "사용자가 이 설정을 건드린 적이 있는가"를 판정하려는 시도(감사·지원 목적)는 신뢰할 수 없다 — 값이 있다고 해서 사용자가 "의도를 갖고" 그 값을 골랐다는 뜻은 아니다.
5. **여러 기기의 동시 수정** — §3.3 이 확정한 "가져올지/올릴지" 선택이 첫 실행 1회뿐이라면(§9 미확정), 이후 여러 기기에서 각각 설정을 바꾼 경우 어느 쪽이 "진실"인지 사용자가 알 방법이 없어질 수 있다.

---

## 6. 필요한 플랫폼 API

| API / 구성요소 | 용도 | 확정 여부 |
| :--- | :--- | :--- |
| `NSUserDefaults`(SuperKey 원본이 실제로 쓰는 저장소) | §3.1 이 참고하는 원본의 저장 메커니즘 — 클론은 이미 다른 기술(`tauri-plugin-store`, `preferences-ui.md` §3.6)을 채택했으므로 클론에는 불필요 | **실측 확정**(원본 기준) — app-bundle-analysis.md §2 |
| `CGEventFlags` 비트마스크 인코딩(`hyperFlags` 값의 해석) | §3.1 이 참고하는 원본의 값 인코딩 방식. 클론이 유사한 비트마스크 인코딩을 채택할지는 F-05(hyperkey.md) 소관 | **실측 확정** — app-bundle-analysis.md §2.1 |
| `NSUbiquitousKeyValueStore`(추정 저장소) | §3.3 iCloud 동기화 — 클론이 채택할 경우에만 필요 | **`(미확정)`** — 심볼로 확인되지 않았다 |
| 대화상자 표시(Tauri dialog / WebView 모달) | §3.2 충돌 대화상자 — 이미 F-09 가 다루는 환경설정 창 렌더링 인프라 재사용으로 충분할 것으로 보임 | 플랫폼 API 신규 요구 없음(F-09 인프라 재사용) |

---

## 7. 구현 접근

이 문서의 판정은 아래 3분류를 기준으로 한다(다른 F-0N 문서들과 동일한 척도).

- **순수 Rust** — 안전(safe) 래퍼 크레이트만으로 커버되어 `unsafe` 도 네이티브 소스도 없음
- **Rust 바인딩** — `unsafe` FFI 직접 호출이 필요하다. `objc2-*` 계열 헤더 자동생성 바인딩을 직접 호출하거나, 대응 크레이트가 없으면 `extern "C"` 선언을 손으로 작성해 프레임워크에 직접 링크한다
- **네이티브 shim 불가피** — Rust 에서 호출할 방법이 없어 Swift/Objective-C 소스를 별도 컴파일해 링크해야 함

### (1) 설정 저장 모델(부재=기본값 규약) — 판정: **순수 Rust**

`tauri-plugin-store`(F-09 §3.6 이 이미 결정) 위에 "필드가 없으면 기본값을 쓴다"는 역직렬화 규칙과 "값이 바뀔 때만 쓴다"는 직렬화 규칙을 얹는 것은 플랫폼 API 호출이 아니라 순수 Rust 로직(구조체 `Option<T>` 필드 + `#[serde(default)]` 류 패턴)이다. 네이티브 바인딩이 전혀 필요 없다.

### (2) 설정 충돌 감지·해소 대화상자 — 판정: **순수 Rust** (근거는 얇음)

충돌 감지 자체(설정 저장 시 caps lock 소스 키 중복을 검사하는 로직)는 순수 Rust 상태 비교 로직이다. 대화상자 표시는 F-09 가 이미 채택한 Tauri/WebView 렌더링 인프라를 재사용하면 되므로 이 문서가 새로 판정할 플랫폼 API 가 없다. 다만 이 판정은 "무엇을 만들지" 는 확정하지 못한 채 "어떻게 만들지" 만 판정한 것이다 — §3.2 의 `(미확정)` 목록(정확한 트리거 조건, 다른 충돌 후보의 처리 여부)이 해소되지 않으면 실제 구현 범위(충돌 검사 규칙이 3종인지, 더 많은지)를 확정할 수 없다.

### (3) iCloud 설정 동기화 — 판정: **보류(범위 판단이 §7 의 선결 과제)**

이 기능을 클론이 요구사항에 넣을지부터 결정되지 않았다(§3.3). 판단을 미룬 근거: (a) 대응 UI 를 하나도 관찰하지 못해 사용자 경험을 설계할 근거가 없다. (b) 원본의 entitlement 에 iCloud KV 스토어에 필요한 키가 없다는 모순(§3.3, §9)이 있어, 원본조차 이 기능이 활성 상태인지 불확실하다. (c) 클론은 애초에 원본과 다른 Apple Developer 계정·번들 ID를 쓰므로, 설령 채택하더라도 원본과 같은 iCloud 컨테이너를 공유할 수 없다 — "원본을 따라간다"는 근거 자체가 이 항목에는 성립하지 않는다.

**만약 채택한다면**의 판정만 남겨 둔다: Apple 의 `NSUbiquitousKeyValueStore` 를 쓴다면 `objc2-foundation` 헤더 자동생성 바인딩 경로가 될 것으로 보이나(`rust-macos-capability-notes.md` 의 일반적인 Foundation 바인딩 패턴), 이 클래스를 전용으로 다루는 크레이트가 있는지는 조사되지 않았다 — `Rust 바인딩` 후보. 자체 백엔드(예: 클론이 이미 갖췄을 계정/동기화 서비스)를 쓴다면 플랫폼 API 판정 대상이 아니라 순수 네트워킹 문제가 된다. 둘 중 무엇을 택할지는 이 문서의 범위를 넘는 제품 결정이다.

---

## 8. 수용 기준

확정된 부분만 검증 가능한 형태로 적는다.

- [ ] 설정 파일(또는 그 파일이 빈 상태)로 앱을 실행하면, 모든 항목이 각 F-0N 문서가 정의한 출고 기본값으로 표시된다(§3.1 "부재=기본값").
- [ ] 사용자가 건드리지 않은 설정 항목은 저장 파일에 키 자체가 생기지 않는다.
- [ ] 설정을 변경했다가 원래 값으로 되돌리면, 저장 파일에는 해당 키가 남되(삭제되지 않음, §3.1 결정) 값은 기본값과 동등하다.
- [ ] `Remap caps lock to:` 를 사용 중인 상태에서 다른 caps lock 리매핑(예: Hyperkey 소스)을 추가로 시도하면, 조용히 실패하거나 조용히 덮어쓰지 않고 확인을 구하는 절차가 발생한다(C1, §3.2 — 정확한 UI 는 F-09/F-08 후속 설계 대상).
- [ ] `Caps lock + W A S D`/`[H J K L]` 방향키 프리셋 사이에서도 동일한 확인 절차가 발생한다(C2).
- [ ] `Caps lock + home row =` 프리셋과의 충돌에서도 동일한 확인 절차가 발생한다(C3).

---

## 9. 미해결 질문

1. **충돌 대화상자의 실제 모습·버튼 구성·문구 전문** — caps lock 을 실제로 리매핑해 충돌 조건을 재현해야 확인 가능하다(부작용 있음, app-bundle-analysis.md §8 한계 7).
2. **충돌 감지 시점** — 설정 저장 시점인지 토글(체크박스 클릭) 즉시인지.
3. **`Conflict:` 3종 외의 충돌 처리 여부** — Seek 소스 키 ↔ Hyperkey 소스 키 중복(`hyperkey.md` §5), 캡스락 방향키 프리셋 ↔ Hyperkey 소스 caps lock(`power-user-presets.md` §3.3 R2)이 이 대화상자로 처리되는지, 별도 메커니즘인지, 처리되지 않는지.
4. **iCloud 저장소의 정체** — `NSUbiquitousKeyValueStore` 인지 다른 저장소인지 심볼로 확인되지 않았다.
5. **⭐ entitlement 모순** — SuperKey 의 문자열에는 iCloud 동기화 기능이 뚜렷하지만, entitlement 에는 iCloud KV 스토어에 필요한 `com.apple.developer.ubiquity-kvstore-identifier` 가 없다(`com.apple.security.cs.allow-jit` 하나뿐). 문자열이 있는데 entitlement 가 없다는 것은 기능이 비활성이거나 다른 저장소를 쓴다는 뜻일 수 있으나, 이번 조사로는 어느 쪽인지 단정할 수 없다.
6. **iCloud 동기화 대상 범위** — 전체 설정인지 일부 설정만인지.
7. **iCloud 충돌 해소가 첫 실행 1회뿐인지 상시인지**.
8. **iCloud 동기화를 켜고 끄는 UI 위치** — 4개 탭 어디에도 없었다. 숨겨진 조건부 UI 이거나, 애초에 사용자에게 노출되지 않는 기능(예: 항상 켜짐)일 수 있다.
9. **`syncAfterEachWrite` 의 대응 옵션** — "매번 동기화"의 반대(주기적/수동/앱 종료 시)가 실제로 무엇인지 관찰하지 못했다.
10. **`CodableDefault` 등 타입별 프로퍼티 래퍼 추측의 정확성** — 이름 패턴으로부터의 해석일 뿐이며, 실제 SuperKey 내부 구조를 반영하는지 확인할 방법이 없다(디컴파일은 조사 범위 밖).
11. **`seekOptions`/`capsWasdArrows`/`oneSwipeFromTop` 의 정확한 열거값 의미** — 각각 값 하나만 관찰되어(잔여물 사례) 다른 값과 대조해 확정하지 못했다. `seekOptions` 가 비트 `OptionSet` 이라는 것도, `2` 가 "꺼짐"이라는 것도 `(미확정)`.
12. **저장 파일 손상 시 실제 복구 정책** — §5 항목 1 은 "부재=기본값 모델과 자연스럽게 어울린다"는 설계 판단일 뿐, 원본이 실제로 이렇게 동작하는지는 관찰하지 못했다.
13. **다중 기기 동시 수정 시 병합 정책** — last-write-wins 인지 필드 단위 병합인지(§5 항목 3).