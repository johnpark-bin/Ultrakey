# 이슈 #46 — 키보드 탭: 공통(모든 키보드) 룰 복사/상속 + 오버라이드 — 설계 초안 + 작업 분해

> **성격**: `docs/plan/` 은 구현 위임 전 **계획 문서**를 두는 자리다. 이 문서는 중급(계획 초안)이 작성한 **초안**이며, 확정은 상급(호출 세션)의 리뷰 후 이루어진다. 리뷰가 끝나면 "결정 목록(D1~D7)"이 정본이 되고, §7 의 명세 갱신 초안이 `docs/spec/per-device-settings.md` 에 반영된 뒤 구현이 위임된다.
>
> **범위 제약**: 저장 키 구조 변경, `Tri` 3상태 의미 변경, 기능 1 행 수준 병합(기본 채택), 이슈 #46 방향 재논의 — 전부 이 문서의 범위 밖(기각/불변으로만 언급한다).

---

## 1. 요약

현재 F-17(`Keyboards` 탭)의 3상태 폴백(`Tri`)은 이미 "상속 + 오버라이드"를 **저장 모델 수준**에서 구현하고 있다 — 디바이스 키 부재 = 공통 따름, 값 존재 = 오버라이드, `null` = 명시적 끔(`perdevice/mod.rs` `Tri`·`read_tri`·`resolved_*`). 이번 작업이 채우는 것은 그 위에 없는 **UI 층의 갭 셋**이다: ① 공통 룰을 디바이스로 물질화하는 **복사 버튼**(기능 1·기능 2 각 1개), ② 기능 1 의 **상속 복귀 컨트롤**(디바이스 오버라이드/끔을 버리고 키를 삭제), ③ **상속/오버라이드/끔 상태 표시**(기능 1 그룹 뱃지). 저장 키는 **하나도 새로 만들지 않는다** — 복사는 기존 키(`perDevice.<vid>:<pid>.keyRemap.rows`·`functionKeys.f1..f12`)에 기존 값 타입을 쓰는 것일 뿐이고, 상속 복귀는 이미 기능 2 가 쓰는 `settings_unset`(키 삭제)을 기능 1 의 `rows` 키에 처음 적용하는 것이다. 신규 백엔드 커맨드 1개(`settings_copy_common_to_device` — 복사 시점의 **희소 배치 쓰기**를 단일 store 락 + 단일 엔진 재합성으로 묶는다)와, ultrakey-core 의 **순수 복사 계획 헬퍼** 2개(어떤 키·값을 쓸지 판정), 프런트 변경, i18n 8키 × 5언어, 테스트가 전부다. 기능 1 의 "완전 대체" 병합 규약(명세 §3.4)은 **유지**하며, 행 수준 병합은 기각 대안으로만 검토한다.

---

## 2. 결정 목록 D1~D7

### D1 — 상속 모델의 범위 확정: **저장·해석 계층은 그대로 두고, UI 층(복사·복귀·표시)만 추가한다**

**결정**: 이번 작업이 바꾸는 것과 바꾸지 않는 것을 다음과 같이 긋는다.

| 바꾸지 않는다(기존 동작 유지) | 바꾼다(신규) |
| :--- | :--- |
| 저장 키 구조 — `perDevice.{scope}.keyRemap.rows` / `perDevice.{scope}.functionKeys.f1..f12` | ① 복사 버튼(공통 → 선택 디바이스, 기능별 1개) |
| `Tri` 3상태 의미 — 부재=상속, 값=오버라이드, `null`=끔 | ② 기능 1 상속 복귀 컨트롤 |
| 2계층 폴백 해석 — `resolved_key_remap_rows` / `resolved_function_key` (디바이스 → 공통 → 없음) | ③ 기능 1 그룹 상속/오버라이드/끔 상태 뱃지 |
| 기능 1 "완전 대체" — 디바이스 배열이 있으면 공통을 병합하지 않고 통째로 대체(§3.4) | ④ 순수 복사 계획 헬퍼(어떤 키·값을 복사할지 — ultrakey-core) |
| `compose()` 우선순위(D-1 > 기능 1 > 기능 2) · 원장(`_managed`) · `settings_set`/`settings_unset` 커맨드 표면 | ⑤ 배치 복사 커맨드 1개(앱 계층, 단일 재합성) |
| 기능 1 행 편집·삭제·`null` 저장 경로(`saveKeyRemapRows`), 기능 2 팝업 3상태 로직 | ⑥ i18n 8키 × 5언어 + 테스트 + 명세 갱신 |

**근거**:
- "상속 + 오버라이드 모델"(이슈 #46 ②)은 이미 저장 모델에 존재하는 사실이다. 저장 계층을 손대면 기존 저장 파일 호환성(F-15 무결성)을 처음부터 다시 논증해야 한다 — 이번 작업이 주는 가치는 UI 갭 3개(복사·복귀·표시)뿐이므로, 저장 계층을 건드릴 근거가 없다.
- §3.4 의 "완전 대체"는 명세·구현·테스트(`feature1_device_value_overrides_common` 등) 셋이 함께 확정한 규약이다. 그것을 행 수준 병합으로 바꾸면 **"이 디바이스에서 실제로 무엇이 적용되는가"를 사용자가 머릿속에서 합산**해야 하게 되고(공통의 일부 + 디바이스의 일부), 경로 B 가 배열을 원자적으로 통째 교체하는 성질(스파이크 S-6 — `--set` 은 병합이 아니라 교체)과도 어긋난다. 바꿀 이유가 없다.

**기각한 대안**:
- **기능 1 을 행 수준 병합으로 재설계(오버라이드는 행 단위)**: 위 근거와 동일. 저장 계층에 연산이 하나 더 생기고(부분 병합), 사용자 모델이 합산을 요구하고, 경로 B 의 통째 교체 성질과 대칭이 깨진다. 작업량도 3~4배(해석·저장·합성·UI·테스트 전면 변경). 이 문서는 이 대안을 **기각 대안으로만** 취급한다.
- **상속을 "복사 후 수동 동기화"로 대체(복사만 두고 상속 제거)**: 부재=상속 폴백을 없애면 기능 2 의 `--- (공통 설정을 따름)` 팝업 선택지와 명세 §3.3 전체가 무너진다. 이미 구현·테스트된 동작을 지우는 것이 이슈 #46 방향(상속 + 오버라이드)과 모순된다.

### D2 — 복사 버튼: **그룹 제목 행 1개씩(기능별 2개), 공통에 복사할 명시 값이 있을 때만 활성, 대상 디바이스에 값이 있으면 "실제로 바뀌는 것만" 확인 대화상자**

**결정** (세부 위치·라벨·활성 조건은 §4 에서 규격화):
- **위치**: 우측 상세 패인의 두 그룹 제목 행——기능 1(`키 변환 세트`)과 기능 2(`Function Keys`) 각각에 버튼 1개. 방향은 "공통(`For all devices`) → 현재 선택된 디바이스"로 고정이다(버튼이 선택 디바이스의 상세 패인 안에 있으므로 방향이 문맥상 자명하다).
- **개수**: 기능 전체 1개가 아니라 **기능별 2개**. 이유는 §4 에서 보듯 두 기능의 복사 의미론이 다르다(기능 1 = 배열 1개 통째, 기능 2 = 희소 F-키 집합) — 하나의 "전체 복사" 버튼으로 묶으면 확인 대화상자의 범위(어느 기능을 덮는가)와 활성 조건(어느 기능이라도 복사할 것이 있는가)이 모호해진다.
- **활성 조건(① dimmed)**: `currentDevice !== "all"` 인 동안 표시되되, 그 기능에 복사할 것이 없으면 `disabled`.
  - 기능 1: `perDevice.all.keyRemap.rows` 가 **명시적 비어 있지 않은 배열**일 때만. (부재·`null`(끔)·빈 배열 — 빈 배열은 이론상 수기 파일에만 존재 — 은 복사할 것이 없다.)
  - 기능 2: 공통 계층에 **명시적 목적지 id 문자열(Value)인 F-키가 1개 이상**일 때만. (공통의 `null`(끔)·부재 키는 복사하지 않는다 — D3.)
- **대상 디바이스에 이미 오버라이드가 있을 때**: **"그 복사가 실제로 무엇인가를 바꿀 때만" 확인 대화상자를 띄운다.** "무엇인가를 바꾼다"의 기준은 **기존 디바이스 전용 값(키)이 새 값과 다른 값으로 교체될 때**다 — 같은 값이면 확인을 띄울 이유가 없다(기능 1: 디바이스 키가 존재하고 값이 공통 배열과 다를 때 / 기능 2: 복사할 키 ∩ {값이 다른 디바이스 키} ≠ ∅). 확인 후의 쓰기 자체는 **무조건 교체**다 — 병합하지 않는다(기능 1 전체 대체, 기능 2 희소 교체). ⚠️ 디바이스 키가 **부재**(상속 중)인 채로 복사하면 값은 같지만 키가 새로 생겨 이후 공통 변경이 흐르지 않는 스냅샷이 된다 — 파괴되는 데이터가 없으므로 확인 없이 실행한다(편집으로 물질화하는 것과 동등한 동작).
  - **근거**: 이 코드베이스가 이미 이슈 #31 ① 에서 "아무것도 바뀌지 않으면 저장하지 않는다"는 억제 철학을 채택했다(`saveKeyRemapRows` 의 `hadKey && rowsEqual` 분기). "바뀌는 것만 확인"은 그 철학의 연장이다. 반대로 **파괴적 조작**(사용자가 만든 디바이스 전용 값이 공통 값으로 덮임)에는 F-15 충돌 대화상자 패턴의 정신을 적용해 반드시 확인을 받는다. 확인 없는 자동 덮어쓰기는 이 저장소의 "데이터를 조용히 지우지 않는다"는 관례와 어긋난다.
- **복사 후 즉시 저장 반영**: 전용 배치 커맨드 1개가 store 락 한 번 → 저장 전부 → `reconfigure_engine` 한 번 → `SettingsState` 한 번 반환하므로, 응답 1회로 UI 가 즉시 갱신된다. 기능 1 만 보면 기존 `settings_set` 1회로도 충분하지만, 기능 2 희소 복사는 최대 12회 키 쓰기라 기존 경로로는 12회 왕복 + 12회 엔진 재합성이 된다 — 그래서 커맨드를 하나 묶는 쪽을 택한다(아래 기각 대안).

**기각한 대안**:
- **복사 버튼 1개(전체 기능)**: 위 근거. 확인 대화상자는 "복사할 것이 있는 기능" 단위로 띄워야 하는데 전체 복사는 그 단위가 애매하다. 또 기능 1 만 필요한 사용자에게 기능 2 의 오버라이드까지 함께 덮는 위험을 확인 문구로 설명해야 해서 문구가 길어진다.
- **디바이스가 오버라이드를 갖고 있으면 항상 확인**: 복사 결과가 기존 값과 동일한 경우(no-op 복사)에도 대화상자가 뜬다. no-op 은 데이터를 파괴하지 않으므로 확인의 근거가 없다. 이슈 #31 ① 의 저장 억제 철학에 어긋난다.
- **확인 없이 항상 덮어쓰기(조용한 교체)**: Karabiner 식 단순함이지만, 이 앱은 "공통은 켜 두고 이 디바이스만 끄기"(시나리오 D)처럼 디바이스 전용 값에 사용자 의도가 실려 있고, 그 값을 복사 한 번으로 지우는 것은 되돌릴 수 없다. F-15 의 충돌 대화상자 패턴(파괴적 설정 조작은 확인)과 정면으로 어긋난다.
- **기능 2 를 프런트 루프로 `settings_set` 12회**: 왕복·엔진 재합성 12회(각각 `reconfigure_engine`) + 중간 상태 렌더링으로 화면 깜빡임 + 도중 오류 시 부분 복사 상태. 희소 복사라 결과적으로는 안전하게 수렴하지만, 이미 있는 `settings_set_per_device` 의 "저장 → 재합성 → 상태 반환" 구조를 그대로 두고 배치만 얇게 묶으면 되는 상황에서 12회 재합성은 낭비다.

### D3 — 기능 2 복사 의미: **희소 복사(공통 계층의 명시적 Value F-키만 물질화)**

**결정**: 기능 2 복사는 공통 계층에서 **명시적 목적지 id 문자열(Value 상태)인 F-키만** 디바이스 키로 쓴다. 공통의 `null`(끔)이나 부재 키는 쓰지 않는다. 복사 후:
- 복사된 키(예: f1, f3)는 공통 변경을 **따라가지 않는다**(스냅샷).
- 복사되지 않은 키(f2, f4…)는 디바이스 키가 부재인 채로 남아 **계속 상속**한다 — 복사 시점 이후 공통에 새로 값이 생긴 키도 자동으로 상속된다.

**근거**:
- F-15 "부재 = 기본값" / "한 번이라도 건드린 항목만 저장" 철학과 일치한다. 복사는 사용자가 그 키를 "건드린" 것과 같으므로 건드린 키만 저장하고, 안 건드린 키는 저장하지 않는 것이 모델과 정확히 맞는다.
- **공통의 `null`(끔) 키는 복사할 필요가 없다** — 디바이스 키가 부재면 이미 공통의 `null` 을 상속한다(같은 결과). 쓰는 값이 결과를 바꾸지 않는 쓰기를 할 이유가 없다.
- 부분 상속이 남는 것은 버그가 아니라 **문서화된 의미론**이다. "복사"의 단위는 "복사 시점에 공통에 명시적으로 존재했던 값"이다(§3 저장 모델 영향).

**MUST DO — "복사하지 않으면 상속"과 "복사하면 독립 스냅샷"의 관계 한 문장 정의**:
> **복사는 공통 계층의 명시적 값들을 복사 시점 그대로 디바이스 계층에 물질화하는 것이고, 일단 복사된 값은 공통이 나중에 바뀌어도 따라가지 않는다(독립 스냅샷); 복사되지 않은 키는 계속 상속한다 — 즉 "스냅샷"의 단위는 '복사 시점에 공통에 값이 있던 항목'이고, 그 밖은 상속이 유지된다.**

**기각한 대안 — 12개 전부 물질화(완전 스냅샷)**: 복사하면 디바이스가 공통에서 완전히 분리된다는 점에서 사용자 모델이 단순해 보이지만, ① 공통에 값이 2개뿐인데 12키를 저장하면 F-15 "건드린 항목만 저장" 위반(사용자가 건드리지 않은 키 10개를 파일에 쓴다), ② 공통 끔(`null`)이 명시적 `null` 12개로 물질화되어 저장 파일이 지저분, ③ 나중에 공통에 F 키가 추가되면 이 디바이스만 조용히 낡은 상태로 떨어져 나가고 그것을 알릴 UI 가 없다. 희소 복사의 "새로 생긴 공통 값은 계속 상속"이 오히려 사용자에게 예측 가능하다.

### D4 — 기능 1 상속 복귀 수단: **기능 1 그룹에 "공통 설정 따르기" 버튼 1개(디바이스 키 삭제 = `settings_unset`)**

**결정**: 기능 1 목록 영역(빈 목록 힌트 옆)에 `#keyboards-keyremap-revert-btn` 버튼을 두고, 클릭 시 `commitUnset("perDevice.<id>.keyRemap.rows")` 를 호출한다(기능 2 의 `--- (공통 설정을 따름)` 선택지가 이미 쓰는 `settings_unset` 경로 — 백엔드 변경 0). 라벨: `preferences.keyboards.keyRemap.revertToCommon` = "공통 설정 따르기". 표시 규칙: `currentDevice !== "all"` **이고** 디바이스 계층에 `keyRemap.rows` 키가 존재할 때만(② 숨김) — 부재(이미 상속 중)면 이 버튼은 개념이 성립하지 않으므로 만든다.
- **어느 상태에서든** 키 삭제로 상속 복귀: 오버라이드(배열 값)든 끔(`null`)이든 `remove_setting_key` 는 값과 무관하게 키만 지운다(`main.rs:874` — `values.remove(key)`). 복귀 후 디바이스는 공통 값을 따르고, 공통도 없으면 "매핑 없음"이 된다.
- **기능 2 에는 새 컨트롤을 만들지 않는다** — 팝업의 `--- (공통 설정을 따름)` 선택지(`KEYBOARDS_FOLLOW_COMMON` → `commitUnset`)가 이미 상속 복귀의 완전한 수단이다.

**근거**: 기능 1 의 현재 갭은 "행을 전부 지우면 `null`(끔)이 될 뿐 상속으로 돌아가는 길이 없다"는 것 이슈 #46 조사에서 확인된 사실이다. 돌아가는 길의 자연스러운 표현은 **디바이스 키 삭제**다(값 변경이 아니라 "내가 만든 것 없애기" — F-15 의 부재=상속의 정면 활용). `settings_unset`·`remove_setting_key` 는 이미 구현·테스트되어 있고 `validate_per_device_key` 도 `keyRemap.rows` 를 허용하므로, 이 컨트롤은 **순수 프런트 연결**이다 — 백엔드 위험이 없다. 기능 2 와 수단이 같아지므로(`settings_unset`) 사용자 모델이 두 기능에서 일관된다.

**기각한 대안**:
- **기능 1 목록에 "상속 복귀" 행/체크박스 추가**: 오버라이드는 배열 단위라 "행 하나만 상속 복귀"가 개념적으로 성립하지 않는다(§3.4 완전 대체). 행 단위 복귀는 D1 에서 기각한 행 수준 병합을 전제로 한다.
- **복사 버튼을 "상속/복사 토글"로 겸용**: 상태가 두 개의 서로 다른 조작(공통→디바이스, 디바이스→공통)을 한 버튼에 실어 라벨·활성 조건이 난해해진다. 조작은 각각 한 방향으로 명확한 버튼 2개(복사·복귀)가 낫다.

### D5 — 상속/오버라이드 표시: **기능 1 그룹 전용 뱃지(3상태) + 상속 중 행은 편집 가능 유지(기존 동작) + 기능 2 는 팝업이 이미 표시**

**결정**:
- **(a) 기능 1 그룹 제목 옆 뱃지** `#keyboards-keyremap-status-badge` — 3값 텍스트: `status.inherited`(디바이스 키 부재) / `status.override`(배열 값) / `status.off`(`null`=끔). `status.inherited` 일 때의 상세 문구는 기존 `keyboards-keyremap-inherited` 힌트(행을 고치면 물질화된다는 안내)가 그대로 담당하고, 뱃지는 **압축 상태 라벨**로만 동작한다.
- **(b) 상속 중인 행의 표시**: **읽기 전용으로 만들지 않는다** — 행은 그대로 편집 가능하고, 아무 행을 고치는 순간 지금처럼 "이 디바이스 전용 배열로 굳는" 오버라이드 시작이 된다(기존 동작 그대로, 명세 §3.4 완전 대체의 UI 표현). 뱃지 + 기존 힌트가 "지금은 공통을 보여주고 있다"는 사실을 알리는 역할을 한다.
- **(c) 기능 2 행별 구분**: **추가 표시를 만들지 않는다** — 각 행의 팝업이 상속 중이면 `--- (공통 설정을 따름)` 을, 오버라이드면 목적지 id 를, 끔이면 `표준 F-키로 사용` 을 선택지로 이미 보여준다. 이것이 곧 행별 상태 표시다. 기능 2 그룹 전체를 요약하는 뱃지도 만들지 않는다(12키가 뒤섞인 혼합 상태를 어떤 문구로 요약해야 하는지가 정의 불가능).
- **종속 표현 3종 분류**: 뱃지는 **③ 문장 중간 삽입(inline)** 계열(라벨 옆 정적 텍스트)로 분류한다 — dimmed(조작 불가)도 hidden(노드 부재)도 아니고, 상태를 보여주는 정적 문구다. 복사 버튼은 **① dimmed**(복사할 것 없을 때 disabled 유지 — `+ Add item` 과 같은 패턴, 헤더 레이아웃이 유지된다), 상속 복귀 버튼은 **② 숨김**(상속 중에는 애초에 노드를 만들지 않음 — `For all devices` 선택 시 기능 2 팝업의 `공통 따름` 옵션을 숨기는 §3.1.3 (b) 와 같은 논리: 조작할 대상이 개념적으로 없으면 숨긴다). `For all devices` 선택 중에는 복사·복귀 버튼 둘 다 ② 숨김이다(공통 위에 복사 원천도 복귀 목적지도 없다).

**근거**: 상속 중 행을 읽기 전용으로 만들면 "이 디바이스만 다르게 하고 싶다"는 가장 흔한 조작이 **2단계**(복사 버튼 → 편집)가 된다. 지금 동작(편집 즉시 물질화)은 명세 §3.4 가 이미 확정한 완전 대체의 자연스러운 UI 이고, 복사 버튼은 "편집 없이 통째로 가져오기"의 지름길로 충분하다. 표시의 목적은 *조회*이지 *조작 제한*이 아니다.

**기각한 대안**:
- **상속 중 행을 dimmed(회색)로**: dimmed 는 F-09 에서 "지금 조작하면 안 된다"는 의미로 쓰인다. 실제로는 조작해도 되고(그게 오버라이드 시작) 의도된 진입점이다 — 뜻이 거꾸로 전달된다.
- **상속 중 행에 행마다 뱃지**: 상속은 배열 단위라 행별 뱃지는 전부 같은 문구가 되고, 행이 0개인 상속(공통 비어 있음)은 보일 뱃지가 없다. 그룹 뱃지 하나가 전 상태를 덮는다.
- **기능 2 그룹 요약 뱃지**: 12키가 혼합(일부 상속·일부 오버라이드)일 때 문구를 정할 수 없다.

### D6 — 저장 호환성: **새 저장 키 없음 — 복사는 기존 키에 기존 값 타입을 쓸 뿐이고, 기존 설정 파일은 그대로 읽힌다**

**결정**: 저장 키·값 타입·sentinel 의미를 **하나도 바꾸지 않는다**. 이 작업이 저장소에 하는 일은 오직 두 가지다: ① 복사 = `store.set("perDevice.<id>.keyRemap.rows", 공통 배열)` 또는 `store.set("perDevice.<id>.functionKeys.fN", 목적지 id)` (기존 키, 기존 JSON 타입), ② 상속 복귀 = `remove_setting_key` 로 키 삭제. 둘 다 이미 구현·검증된 연산이다.

**근거(호환성 논증)**:
- **읽기 경로 불변**: `PerDeviceSettings::new` + `read_tri` + `resolved_*` 는 손대지 않는다. 기존 파일의 어떤 값도 해석이 달라지지 않는다.
- **쓰기 경로는 기존 값만**: 복사가 쓰는 값은 그때 공통 계층에 이미 저장되어 있던 값의 복사본이다. 값 자체는 "이미 이 버전이 이해하는 형식"이다.
- **삭제 연산은 키 무관**: `remove_setting_key` 는 `values.remove(key)` 후 파일 재작성 — 특정 키에 대한 특수 처리가 없다(확인: `main.rs:874~906`, 임시 파일 → sync → rename 의 원자적 쓰기). 기능 2 의 f-키에 이미 쓰이고 있는 경로를 기능 1 의 rows 키에 처음 쓰는 것뿐이다.
- **스키마 버전**: `{schemaVersion, values}` 봉투(`settings/store.rs`)도 그대로. 복사·복귀는 values 의 내용만 바꾸므로 버전 의미에 영향이 없다.
- 결론: **마이그레이션 필요 없음**. 이 버전이 저장한 파일은 이전 버전과 상호 교환 가능하다(값이 전부 기존 형태).

**기각한 대안**:
- **복사 추적용 별도 키(예: `perDevice.<id>._copiedFrom`)**: 복사 여부를 저장하면 "스냅샷"을 메타데이터로 굳히는 셈이 되는데, 의미론은 이미 값 존재 자체로 결정된다(디바이스 키 존재 = 오버라이드 = 그 값은 스냅샷). 메타데이터 키는 F-15 희소 맵에 잉여 상태를 추가하고, `collect_per_device_values` 가 실어 나르는 값들과 원장(`_managed`) 사이에 또 하나의 특수 키를 만든다. 필요 없다.
- **`settings_export` 처럼 복사 이력 버전 관리**: 요구사항에 없는 것으로, 범위 초과.

### D7 — 모달/확인 대화상자: **기존 `#conflict-overlay` 패턴의 CSS/구조를 재사용하되, 전용 `#confirm-overlay` 를 새로 만들고 상태는 순수 프런트**

**결정**: 복사 확인은 **프런트 전용 확인 오버레이**(새 DOM 블록 `#confirm-overlay` — 기존 `#conflict-overlay` 와 같은 `.dialog` CSS 클래스·버튼 구성 재사용)로 구현한다. 서버 왕복이 없다: 확인 판정(복사가 무엇인가를 바꾸는가)은 프런트가 이미 가진 `currentPerDevice.values` 원본 맵으로 계산할 수 있고, 취소/확인 모두 저장 커맨드를 부르기 **전**에 끝난다.

**근거**: 기존 `#conflict-overlay`(architecture.md §6.5)는 **Presets 전용 페이로드**로 설계됐다 — `PendingConflictView { kind, key, value, disableLabelKeys }`, 해소 커맨드 `settings_resolve_conflict` 는 불리언 값만 받는다(`value.as_bool()` — `main.rs:2967` "only supports a boolean value"). 복사 확인은 (키, 불리언) 형태가 아니라 "커맨드 1개를 실행할지 말지"의 2지선다라 페이로드가 완전히 다르다. 같은 오버레이를 확장하려면 `PendingConflictView` 에 경우를 추가하고 해소 커맨드의 값 타입을 바꿔야 하는데, 그것은 Presets 충돌 흐름에 회귀 위험을 끼치는 대가다. 대신 **확인 필요 조건을 계산하는 로직과 확인 UI를 분리**한다 — 확인은 프런트가, "무조건 교체" 쓰기는 백엔드 커맨드가 맡는다. 이 구분은 "충돌 감지(서버) vs 확정 의지 확인(UI)"이라는 두 관심사의 분리이기도 하다: 복사 덮어쓰기는 의미론적 충돌이 아니라 사용자가 명시적으로 선택한 교체라, 서버의 pending-conflict 상태 기계(반쯤 적용된 상태)가 필요 없다.

**기각한 대안**:
- **`#conflict-overlay` 재사용(페이로드 확장)**: 위 근거. Presets 흐름(`pendingConflict` 상태·`settings_resolve_conflict`·`disableLabelKeys` 목록)에 복사 케이스를 얹으면 한 오버레이가 두 계약을 담게 되고, 불리언 전용 해소 커맨드의 시그니처를 바꿔야 한다.
- **네이티브(OS) 확인 시트/`confirm()`**: 이 앱은 환경설정 창 안의 오버레이를 "설정 창이 상태를 소유하고 자동화 테스트가 DOM 으로 확인할 수 있다"는 근거로 이미 선택했다(`settings.html` 880행 주석). 같은 이유가 복사 확인에도 그대로 적용된다. `window.confirm()` 은 카탈로그 문구·DOM 테스트에서 빠진다.

---

## 3. 저장 모델 영향 — 새 키 없음 논증 + 복사 연산의 정확한 저장 효과

**이 작업이 저장소에 일으키는 모든 쓰기는 아래 표가 전부다.** 그 외의 키 경로는 존재하지 않는다(프런트가 보내는 키는 `validate_per_device_key` 의 두 모양 검증을 그대로 통과해야 한다).

| 동작 | 쓰는 저장 키 | 쓰는 값 | 기존 키와의 관계 | 결과 상태(`Tri`) |
| :--- | :--- | :--- | :--- | :--- |
| 기능 1 복사 | `perDevice.<id>.keyRemap.rows` | 공통 계층의 `rows` 배열 **복사본**(기존 값 타입) | 없던 키 → 신설(상속 → 오버라이드로 전환). 있던 키 → 값 교체(완전 대체, §3.4) | `Value(rows)` |
| 기능 2 복사(희소, D3) | `perDevice.<id>.functionKeys.fN` (공통에 명시적 Value 인 N 만) | 해당 F-키의 목적지 id 문자열(기존 값 타입) | 없던 키 → 신설. 있던 키 → 같은 키만 교체(나머지 디바이스 키는 보존) | 각 키 `Value(id)` |
| 기능 1 상속 복귀 | — (**아무 키도 쓰지 않는다**) | — | `remove_setting_key` 로 `perDevice.<id>.keyRemap.rows` 삭제 | 키 부재 = `Inherit` |
| (기능 2 상속 복귀) | — | — | 기존 팝업 `--- (공통 설정을 따름)` → `settings_unset` — **변경 없음** | 키 부재 = `Inherit` |

**주의해야 할 두 가지**:
1. **기능 1 복사 후 공통을 바꿔도 디바이스 복사본은 안 바뀐다**(§2 D3 의 정의 그대로 — 복사된 배열은 저장 당시 공통 배열과 값이 같지만, 이후 읽기는 디바이스 키가 우선하므로 완전 독립이다).
2. **기능 2 복사는 부분 스냅샷이다**: 복사된 키만 독립이고, 복사되지 않은 키는 계속 상속한다(공통에 **나중에** 추가된 F-키 값은 이 디바이스에 그대로 흘러든다). 이것은 의도된 의미론이며 명세·UI 카피에 명시한다.

**레거시 값 호환**: 기능 2 복사가 쓰는 값은 공통 계층에 이미 저장되어 있던 값 그대로다. 옛 `SystemFunction` variant 이름(마이그레이션 대상)이 공통에 있다면 그것도 그대로 복사된다 — 해석은 기존 `destinations::resolve_stored` 가 읽는 시점에 처리한다(저장 표현을 바꾸지 않는다).

---

## 4. UI 변경 명세

### 4.1 변경 전 DOM 구조 (Keyboards 탭, `settings.html`)

기능 1 그룹 현재:
```html
<h2 class="group" id="keyboards-keyremap-heading"></h2>
<div id="keyboards-keyremap-list"></div>
<p class="hint" id="keyboards-keyremap-empty"></p>
<p class="hint" id="keyboards-keyremap-inherited" hidden></p>
<button id="keyboards-keyremap-add-btn"></button>
```
기능 2 그룹 현재: `<h2 class="group" id="keyboards-functionkeys-heading"></h2>` + `<div id="keyboards-functionkeys-list">`(F1~F12 행).

### 4.2 변경 후 DOM (추가분 — 기존 id·구조는 전부 유지, 회귀 없음)

```html
<!-- 기능 1 그룹 제목 행: flex 로 감싸 복사 버튼 + 상태 뱃지를 붙인다 -->
<div class="group-title-row">
  <h2 class="group" id="keyboards-keyremap-heading"></h2>
  <span class="badge" id="keyboards-keyremap-status-badge" hidden></span>
  <button id="keyboards-keyremap-copy-btn" type="button" disabled></button>
</div>
<div id="keyboards-keyremap-list"></div>
<p class="hint" id="keyboards-keyremap-empty"></p>
<p class="hint" id="keyboards-keyremap-inherited" hidden></p>
<button id="keyboards-keyremap-revert-btn" type="button" hidden></button>   <!-- ② 숨김 -->
<button id="keyboards-keyremap-add-btn"></button>

<!-- 기능 2 그룹 제목 행 -->
<div class="group-title-row">
  <h2 class="group" id="keyboards-functionkeys-heading"></h2>
  <button id="keyboards-functionkeys-copy-btn" type="button" disabled></button>
</div>
<div id="keyboards-functionkeys-list">
  … (F1~F12 행, 기존 그대로)
</div>

<!-- 복사 확인 오버레이 — D7. conflict-overlay 와 같은 .dialog CSS/버튼 구성. -->
<div id="confirm-overlay" hidden>
  <div class="dialog" role="alertdialog" aria-modal="true" aria-labelledby="confirm-title">
    <h2 id="confirm-title"></h2>
    <p id="confirm-body"></p>
    <div class="dialog-buttons">
      <button id="confirm-cancel"></button>
      <button id="confirm-continue" class="accent"></button>
    </div>
  </div>
</div>
```

### 4.3 컨트롤 규격

| 컨트롤 | 위치 | 라벨(i18n) | 활성/표시 조건 | 동작 |
| :--- | :--- | :--- | :--- | :--- |
| `#keyboards-keyremap-copy-btn` | 기능 1 그룹 제목 행 | `preferences.keyboards.copy.button` | `currentDevice === "all"` → ② 숨김. 그 외: 공통 `keyRemap.rows` 가 명시적 비어 있지 않은 배열일 때만 활성(① dimmed) | 복사 확인 판정 → (필요 시) `#confirm-overlay` → `invoke("settings_copy_common_to_device", { device, feature: "keyRemap" })` |
| `#keyboards-functionkeys-copy-btn` | 기능 2 그룹 제목 행 | `preferences.keyboards.copy.button` | `currentDevice === "all"` → ② 숨김. 그 외: 공통에 Value 인 F-키 ≥ 1 개일 때만 활성 | 위와 같음(`feature: "functionKeys"`) |
| `#keyboards-keyremap-revert-btn` | 기능 1 목록 아래(`+ Add item` 옆/앞) | `preferences.keyboards.keyRemap.revertToCommon` | ② 숨김 — `currentDevice !== "all"` **이고** `perDevice.<id>.keyRemap.rows` 키 존재 시에만 생성 | `commitUnset(keyRemapRowsKey(currentDevice))` → `renderState` |
| `#keyboards-keyremap-status-badge` | 기능 1 그룹 제목 행 뱃지 | `status.inherited` / `status.override` / `status.off` | `currentDevice === "all"` → 숨김(공통 계층에는 상태 개념 없음). 그 외 3상태 텍스트 중 하나 | 읽기 전용 |
| `#confirm-overlay` | 창 오버레이(conflict-overlay 와 같은 위치·스타일) | `copy.confirmTitle` / `copy.confirmBody`(`{0}` = 그룹 제목) / `common.cancel`(재사용) / `copy.confirmContinue` | 복사가 실제로 무언가를 바꿀 때만 | 취소 → 아무것도 안 함(화면 그대로). 계속 → 복사 커맨드 1회 → `renderState` |

### 4.4 JS 동작 요약 (구현은 ultrakey-implement)

1. `renderKeyRemapGroup` 확장: 뱃지 3상태 텍스트·숨김 설정, 복사 버튼 활성/숨김 판정, 복귀 버튼 생성/숨김 판정을 같은 패스에 추가. 기존 `keyboards-keyremap-inherited` 힌트 로직은 **그대로 둔다**(뱃지와 역할이 다르다 — 뱃지는 상태 라벨, 힌트는 "편집하면 물질화된다"는 설명).
2. 복사 흐름:
   - 확인 필요 판정(프런트, `currentPerDevice.values` 사용): 기능 1 = `values[key]` 존재 **이고** 기존 값이 공통 배열과 다를 때. 기능 2 = 복사 대상 키(공통 Value 키) 중에서 `values[deviceKey]` 가 존재 **하고** 값이 공통 값과 다른 키가 1개 이상일 때. (`keyRemapRowsEqual` 재사용 가능.)
   - 필요 시 `#confirm-overlay` 표시 → `confirm-continue` 클릭 시 실행. 확인이 불필요하면 즉시 실행.
   - **쓰기 억제**: 이미 디바이스 키에 새 값과 같은 값이 있으면 그 키의 쓰기를 스킵한다(`saveKeyRemapRows` 의 이슈 #31 ① 억제 철학과 동일). 전부 스킵되면 커맨드 호출 자체를 생략한다. 디바이스 키가 부재인(상속 중) 키는 스냅샷 물질화 목적의 쓰기이므로 **스킵하지 않는다** — 그것이 복사의 존재 이유다.
   - `invoke("settings_copy_common_to_device", { device: currentDevice, feature })` — 응답 `SettingsState` 로 `renderState(state)`.
3. 복귀: 기존 `commitUnset` 그대로 재사용(키만 `keyRemapRowsKey(currentDevice)`).
4. `applyStrings()` 에 새 5개 라벨(`copy.button`·`revertToCommon`·`status.*` 3종 + `confirm-*` 2종) 배선.

**기존 동작 회귀 없음 논증**(UI 층): 기능 1 행 편집/삭제/`null` 저장 경로, 기능 2 팝업 3상태, `For all devices` 선택 시 팝업의 `공통 따름` 숨김(§3.1.3 (b)) — 전부 손대지 않는다. 추가분은 새 DOM 노드와 새 이벤트 핸들러뿐이고, 기존 id·함수는 이름 그대로 유지된다(§3.1.5 의 고정 컨트롤 개수는 명세 쪽 표만 갱신 — §7).

### 4.5 신규 i18n 키 (5개 언어, `preferences.keyboards.*` 네임스페이스)

> en/ko 는 완전 번역 제안, zh/es/ja 는 **초안 제안**(최종 검수는 구현 단계 — 기존 카탈로그가 이미 쓰는 어휘를 그대로 쓴다: "For all devices" = zh `适用于所有设备` / es `Para todos los dispositivos` / ja `すべてのデバイ스`).

| 키 | en | ko | zh | es | ja |
| :--- | :--- | :--- | :--- | :--- | :--- |
| `preferences.keyboards.copy.button` | `Copy from “For all devices”` | `“모든 키보드” 설정 복사` | `从“适用于所有设备”复制` | `Copiar desde “Para todos los dispositivos”` | `「すべてのデバイス」からコピー` |
| `preferences.keyboards.copy.confirmTitle` | `Overwrite this keyboard's settings?` | `이 키보드의 설정을 교체할까요?` | `覆盖此键盘的设置吗？` | `¿Sobrescribir la configuración de este teclado?` | `このキーボードの設定を上書きしますか？` |
| `preferences.keyboards.copy.confirmBody` | `This will replace this keyboard's {0} with the settings from “For all devices”.` | `이 키보드의 {0}(을)를 “모든 키보드” 설정으로 교체합니다.` | `此键盘的{0}将替换为“适用于所有设备”的设置。` | `El {0} de este teclado se reemplazará con la configuración de “Para todos los dispositivos”.` | `このキーボードの{0}は「すべてのデバイス」の設定に置き換えられます。` |
| `preferences.keyboards.copy.confirmContinue` | `Overwrite` | `교체` | `覆盖` | `Sobrescribir` | `上書き` |
| `preferences.keyboards.keyRemap.revertToCommon` | `Revert to common setting` | `공통 설정 따르기` | `恢复为通用设置` | `Volver a la configuración común` | `共通設定に戻す` |
| `preferences.keyboards.status.inherited` | `Inherited from “For all devices”` | `“모든 키보드” 설정을 따름` | `继承自“适用于所有设备”` | `Heredado de “Para todos los dispositivos”` | `「すべてのデバイス」から継承中` |
| `preferences.keyboards.status.override` | `Custom for this keyboard` | `이 키보드 전용 설정` | `此键盘专属设置` | `Configuración propia de este teclado` | `このキーボード専用の設定` |
| `preferences.keyboards.status.off` | `Off for this keyboard` | `이 키보드에서 끔` | `已为此键盘关闭` | `Desactivado para este teclado` | `このキーボードではオフ` |

⚠️ `confirmBody` 는 위치 인자 `{0}` 하나를 쓴다 — `all_catalogs_preserve_positional_placeholders` 테스트가 5개 언어의 인자 집합 일치를 강제하므로 어느 언어에서도 `{0}` 을 빼면 안 된다. 문법상 "이 키보드의 {0}(을)를" 같은 (을)를 중립 표기는 한국어 UI 에서 흔한 관례이고, `{0}` 위치에 오는 값은 `preferences.keyboards.group.keyRemap.title`("키 변환 세트"·"Key Conversions", zh `按键转换`, es `Conversiones de teclas`, ja `キー変換`)과 `group.functionKeys.title`("Function Keys" — 4언어 동일 표기)이다.

**테스트 영향(하드코딩 키 목록)**: `frontend_wiring.rs` 의 `keyboards_탭_신규_i18n_키_29개가_en_ko_양쪽에_있다`(1154행)는 키 목록을 하드코딩하고 개수 단언(`KEYS.len() == 29`)을 건다. 이 목록은 **이미 4개를 빠뜨리고 있다**(`keyRemap.inheritedFromCommon`·`macosStatus.on/off/unknown` — 카탈로그·settings.html 에는 존재, 이 테스트 목록에만 부재). 이 작업에서 목록에 **신규 8개를 더하며, 누락돼 있던 4개도 같은 패스에 보충**한다 → 총 **41개**로 갱신(`29 + 8 + 4`). `settings_html_의_preferences_keyboards_점_리터럴이_…`(1249행)는 settings.html 리터럴과 카탈로그의 일치를 스캔으로 강제하므로(서브셋이 아니라 전체), 새 리터럴을 카탈로그 없이 추가하면 즉시 실패한다 — 이것이 이번 변경의 첫 번째 회귀 방어선이다.

---

## 5. 구현 작업 분해 (파일 단위 · 실행 순서)

### (a) ultrakey-core — 순수 복사 계획 헬퍼 (신규)

- **파일**: `crates/ultrakey-core/src/perdevice/inherit.rs` (신규 모듈; `perdevice/mod.rs` 의 `pub mod inherit;` 1행 추가 — `usage.rs`/`destinations.rs` 와 같은 패턴).
- **내용(제안 시그니처 — 순수 함수, macOS 비의존, `#[forbid(unsafe_code)]` 유지)**:
  - `pub fn plan_key_remap_copy(settings: &PerDeviceSettings<'_>) -> Option<Vec<KeyRemapRow>>` — `key_remap_rows_common()` 이 `Tri::Value(rows)` 이고 `!rows.is_empty()` 일 때만 `Some(rows)`. `Inherit`/`Off`/빈 배열 → `None`(복사할 것 없음).
  - `pub fn plan_function_keys_copy(settings: &PerDeviceSettings<'_>) -> Vec<(FKey, String)>` — `FKey::all()` 을 순회하며 `function_key_common(f)` 가 `Tri::Value(id)` 인 것만 `(f, id)` 로 수집(희소 — D3). `Inherit`/`Off` 는 건너뛴다.
- **이유**: "무엇이 복사 가능한가 / 무엇을 복사하는가"는 제품 의미론(D3)이라 테스트로 못박아야 하고, 두 함수 모두 `PerDeviceSettings` 위의 순수 판정이라 core 의 관례(인라인 `#[cfg(test)]`) 그대로 검증할 수 있다. 앱 계층은 여기서 나온 계획을 저장소에 쓰기만 하면 된다.
- 코어의 기존 해석·합성·원장에는 **변경 없음**.

### (b) ultrakey-app — 배치 복사 커맨드 (신규 1개) + 상속 복귀는 0줄

- **파일**: `apps/ultrakey-app/src/main.rs`.
- **신규 커맨드** `settings_copy_common_to_device(state, device: String, feature: String) -> Result<SettingsState, String>`:
  1. `device == PER_DEVICE_COMMON_SCOPE`("all") 거부, `DeviceId::parse` 실패 거부, `feature` ∈ {`"keyRemap"`, `"functionKeys"`} 화이트리스트.
  2. `state.store` 락 1회 — `PerDeviceSettings::new(&store.values)` 로 계획 계산(`plan_key_remap_copy` 또는 `plan_function_keys_copy`). 계획은 소유 타입(`Vec`)이므로 borrow 가 여기서 끝난다.
  3. 같은 락 안에서 쓰기: 기능 1 → `store.set(per_device_key_remap_rows(&device), rows)` 1회. 기능 2 → `store.set(per_device_function_key(&device, f), id)` 를 계획 수만큼(≤ 12).
  4. 락 해제 → `reconfigure_engine(...)`(기존 `settings_set_per_device` 와 동일한 스냅샷·인자·순서) → `build_settings_state(...)` 반환.
  - write-through 실패 관용(`D-B` 근거 포함)은 `settings_set_per_device` 그대로 따른다.
  - **상속 복귀는 백엔드 변경 0줄** — `settings_unset`(기존 커맨드)가 `keyRemap.rows` 키에 이미 동작한다(deterministic: `validate_per_device_key` 통과 확인됨).
- `invoke_handler` 등록 목록(`main.rs` 마지막 `generate_handler!`)에 커맨드 추가.

### (c) settings.html — 프런트 변경

- **파일**: `apps/ultrakey-app/ui/settings.html`.
- §4.2 의 DOM 추가(그룹 제목 행 래퍼 2개·복사 2·복귀 1·뱃지 1·`#confirm-overlay`) + §4.3~4.4 의 JS(상태 뱃지, 버튼 활성/숨김, 복사 흐름, 확인 오버레이, `applyStrings()` 배선).
- 기존 함수·id·이벤트 **무변경**(`renderKeyRemapGroup`·`renderFunctionKeysGroup`·`saveKeyRemapRows`·`commitUnset`·F-키 change 리스너).

### (d) i18n + 하드코딩 테스트 목록 갱신

- **파일**: `resources/i18n/{en,ko,zh,es,ja}.json` — §4.5 의 8키를 5개 파일에 정확히 같은 키로 추가(값은 언어별). 위치 인자 `{0}` 집합 일치 준수.
- **파일**: `apps/ultrakey-app/tests/frontend_wiring.rs` —
  - `keyboards_탭_신규_i18n_키_29개가_…` 목록 갱신: 8개 신규 + 4개 기존 누락 보충 → 41개, 개수 단언 동시 갱신.
  - 신규 정적 배선 테스트(§6).

### (e) 명세 갱신

- **파일**: `docs/spec/per-device-settings.md` — §7 의 초안을 반영(신규 절 삽입·§3.1.5 표 갱신·§4.1 카탈로그 행 추가·§8 수용 기준 추가). 구현 코드와 같은 위임에 포함한다(수용 기준이 완료 정의이므로).
- ⚠️ `docs/plan/issue-46-rule-inherit.md` **외의 파일은 이 계획 단계에서는 수정하지 않는다** — 명세 반영은 리뷰 확정 후 구현 위임 시점에 수행.

### (f) 테스트 목록

§6 참조 — 구현 순서상 (a)의 인라인 테스트 → (b)의 main.rs 테스트 → (d)의 frontend_wiring 갱신 순.

---

## 6. 테스트 계획

### 6.1 ultrakey-core 단위 테스트 — `perdevice/inherit.rs` 인라인 `#[cfg(test)]` (perdevice/mod.rs 관례)

| 테스트 | 검증 내용 |
| :--- | :--- |
| `plan_key_remap_copy_returns_common_rows_when_value_and_non_empty` | 공통 `Value([…])` → `Some(rows)` — 값이 그대로 복사 계획에 실린다 |
| `plan_key_remap_copy_none_when_common_absent` | 공통 키 부재(`Inherit`) → `None` |
| `plan_key_remap_copy_none_when_common_off` | 공통 `null`(끔) → `None` |
| `plan_key_remap_copy_none_when_common_empty_array` | 수기 파일의 `[]` → `None` |
| `plan_function_keys_copy_only_includes_value_keys` | 공통 f1=Value, f2=Off(null), f3=부재 → `[(F1, id)]` 만 — **희소 복사 확정** |
| `plan_function_keys_copy_preserves_id_strings_from_common` | 옛 `SystemFunction` 저장값도 plan 에 그대로 실린다(저장 표현 무변경 — 복사는 해석이 아니라 이동) |
| (기존 테스트 전부 그대로 통과) | 해석·합성·원장·검증 변경 없음 회귀 방지 |

### 6.2 ultrakey-app 테스트 — main.rs 인라인 테스트

| 테스트 | 검증 내용 |
| :--- | :--- |
| `settings_copy_common_refuses_device_all` | `device == "all"` → Err |
| `settings_copy_common_refuses_bad_device_and_feature` | `DeviceId::parse` 실패·알 수 없는 `feature` → Err |
| 커맨드 등록 여부 | `generate_handler!` 목록에 이름 존재(frontend_wiring 의 정적 검사와 중복이지만 main.rs 쪽에서도 1줄) |

### 6.3 프런트 정적 배선 테스트 — `frontend_wiring.rs` (정적 문자열 분석, 기존 패턴)

| 테스트(제안 이름) | 검증 내용 |
| :--- | :--- |
| `settings_html_에_keyboards_탭_복사_복귀_상태_컨트롤이_있다` | `id="keyboards-keyremap-copy-btn"`, `id="keyboards-functionkeys-copy-btn"`, `id="keyboards-keyremap-revert-btn"`, `id="keyboards-keyremap-status-badge"`, `id="confirm-overlay"` 존재 |
| `키보드_복사_커맨드가_invoke_배선되어_있다` | settings.html 이 `invoke("settings_copy_common_to_device"` 호출 + main.rs 에 커맨드 이름·`generate_handler!` 등록 존재 (기존 1100행 부근 카탈로그 방식 재사용) |
| `키보드_상속_복귀는_settings_unset을_쓴다` | revert 클릭 처리기가 `commitUnset(keyRemapRowsKey(currentDevice))` 경로 사용(주석/코드 문자열 검사) |
| `keyboards_탭_신규_i18n_키_41개가_en_ko_양쪽에_있다` | 기존 29개 테스트 갱신(신규 8 + 누락 4 보충, 개수 단언 41) |
| (기존) `settings_html_의_preferences_keyboards_점_리터럴이_…` | 자동 적용 — 새 리터럴 ↔ 카탈로그 누락 시 즉시 실패 |

### 6.4 수동 검증(자동화 불가) — `docs/dev/manual-verification.md` 후속 갱신

- 실제 키보드 2대(또는 내장+외장)에서: 공통에 `left_command ↔ left_option` 등록 → 디바이스 A 로 복사 → 공통을 수정 → A 의 복사본이 안 바뀌는지(독립 스냅샷), B 는 공통을 따르는지(상속 유지) 확인.
- 기능 2 희소 복사 후 공통에 새 F-키 값을 추가 → 복사받은 디바이스가 그 키만 상속하는지 확인.
- 오버라이드가 있는 디바이스에 복사 → 확인 대화상자가 뜨고, 취소 시 아무것도 안 바뀌고, 계속 시 교체되는지 확인.

---

## 7. 명세 갱신 초안 — `docs/spec/per-device-settings.md` 에 추가할 내용

> 초안(문서 한국어). 실제 반영은 리뷰 확정 후. 번호는 현재 문서 구조 기준 제안이며, 반영 시 기존 절 번호와 조정한다.

### 7.1 신규 절 "§3.7 공통 설정 복사 · 상속 복귀 · 상태 표시" (기존 §3.6 다음)

> **§3.7 ⭐ 공통 설정 복사 · 상속 복귀 · 상태 표시** (이슈 #46)
>
> §§3.3~3.6 의 2계층 폴백이 곧 "상속 + 오버라이드" 모델이다. 이 절은 그 위의 UI 조작을 정의한다. 저장 키·값 타입·`Tri` 의미는 §3.3 그대로 — 복사는 기존 키에 기존 값 타입을 쓰는 것뿐이고 새 저장 키가 없다.
>
> **3.7.1 복사 버튼 — 여기서 말하는 "복사"의 의미.** 선택된 디바이스의 우측 상세 패인, 두 그룹(기능 1 · 기능 2) 제목 행에 각각 하나씩 둔다. 방향은 "공통(`For all devices`) → 선택 디바이스"로 고정이다. 기능 1 복사는 공통의 `rows` 배열 전체를 디바이스 키로 통째 기록한다(§3.4 완전 대체의 쓰기 판본). 기능 2 복사는 **희소**다 — 공통에 명시적 목적지가 있는 F-키만 디바이스 키로 기록한다(공통의 `null`·부재 키는 기록하지 않는다 — 부재면 어차피 상속되므로 결과가 같다). ⭐ **복사는 스냅샷이다**: 복사된 값은 복사 시점의 공통 값 그대로 디바이스에 고정되며, 이후 공통이 바뀌어도 따라가지 않는다. 복사되지 않은 키는 계속 상속하고, 이후 공통에 **새로** 생긴 값도 상속한다 — 스냅샷의 단위는 "복사 시점에 공통에 값이 있던 항목"이다.
>
> **활성 조건**: `For all devices` 선택 중에는 복사 버튼이 나타나지 않는다(② 숨김 — 공통 위에 복사 원천이 없다). 디바이스 선택 중에는 복사할 것이 있을 때만 활성이다(① dimmed — 기능 1: 공통 `rows` 가 명시적 비어 있지 않은 배열일 때. 기능 2: 공통에 Value 인 F-키가 1개 이상일 때).
>
> **오버라이드 확인**: 복사가 **실제로 무엇인가를 바꿀 때만** 확인 대화상자를 띄운다(이미 같은 값이면 그냥 실행). 확인 후의 쓰기는 무조건 교체다 — 병합하지 않는다(기능 1: 디바이스 배열 전체가 공통 배열로 대체. 기능 2: 겹치는 F-키만 교체, 겹치지 않는 디바이스 전용 값은 보존). 확인 UI 는 프런트가 소유한다(충돌 대화상자와 달리 서버 상태가 필요 없다 — D7).
>
> **3.7.2 상속 복귀.** 오버라이드(값)든 끔(`null`)이든 디바이스 계층의 명시적 상태를 버리고 상속으로 돌아가는 조작은 **디바이스 키 삭제**(`settings_unset`)다. 기능 2 는 F-키 팝업의 `--- (공통 설정을 따름)` 선택지가 이미 이 수단이다. 기능 1 은 그룹에 `공통 설정 따르기` 버튼을 두어 같은 수단(디바이스의 `keyRemap.rows` 키 삭제)에 연결한다 — 행을 "전부 지우는"(=`null` 끔) 것과 구별된다: 끔은 이 디바이스에서만 공통을 무시하고 꺼버리는 것이고, 상속 복귀는 공통을 다시 따르는 것이다.
>
> **3.7.3 상태 표시.** 기능 1 그룹 제목에 상태 뱃지를 둔다 — `상속됨`(디바이스 키 부재) / `이 키보드 전용 설정`(배열 값) / `이 키보드에서 끔`(`null`). 상속 중 행은 편집 가능한 상태로 그대로 보여준다(어느 행을 고치든 그 순간 §3.4 의 완전 대체로 이 디바이스 전용 배열이 된다 — 기존 동작). 기능 2 는 행별 팝업의 선택지(공통 따름/목적지/표준 F-키)가 곧 상태 표시라 추가 UI 를 두지 않는다. 종속 표현 분류: 뱃지 = ③ inline, 복사 버튼 = ① dimmed, 복귀 버튼 = ② 숨김(§3.1.3 확장).

### 7.2 §3.1.5 표 갱신(고정 컨트롤 개수)

- 기존: 고정 4개 — 패인 1 + 그룹 제목 2 + 상태 표시줄 1.
- 수정안: 고정 **8개** — 패인 1 + 그룹 제목 2(이제 두 그룹 제목 각각 복사 버튼 1개씩을 품는 행) + **복사 버튼 2** + **상속 복귀 버튼 1(기능 1, ② 숨김)** + **상태 뱃지 1(기능 1, ③ inline)** + macOS 상태 표시줄 1. (② 숨김 컨트롤도 "설계상 고정 슬롯"으로 센다 — `For all devices` 선택 시 DOM 에서 사라질 뿐.)

### 7.3 §4.1 카탈로그 표 추가 행

§4.5 의 8개 키 행을 같은 표 형식(키 제안 / en / ko)으로 추가하고, zh/es/ja 는 D6 로케일 확장 규약에 따라 동일 키 집합을 유지한다고 명시.

### 7.4 §8 수용 기준 추가 항

- [ ] `For all devices` 에 등록한 기능 1/2 설정을 선택 디바이스로 복사할 수 있고(복사 버튼 ×2), 복사 후 공통 설정을 바꿔도 복사된 디바이스 값은 바뀌지 않는다(독립 스냅샷).
- [ ] 기능 2 복사는 공통에 명시적 값이 있는 F-키만 디바이스로 기록하고(희소), 공통에 새로 추가된 F-키 값은 복사받지 않은 디바이스에 계속 상속된다.
- [ ] 복사가 기존 디바이스 전용 값을 실제로 바꿀 때만 확인 대화상자가 뜨고, 취소하면 아무것도 저장되지 않으며, 같은 값이면 대화상자 없이 진행된다.
- [ ] 기능 1 에서 오버라이드/끔 상태의 디바이스를 `공통 설정 따르기` 로 복귀시키면 디바이스 키가 삭제되어(저장 파일에서 키 부재 확인) 공통 값을 따른다.
- [ ] 기능 1 그룹의 상속/오버라이드/끔 상태 뱃지가 선택 디바이스에 맞게 표시된다.
- [ ] 새 저장 키가 없다 — 복사·복귀 전후로 설정 파일에 `perDevice.<id>.*` 외 키가 생기지 않는다(정적/동적 검토).

---

## 8. 리스크 / 미확인 항목

| # | 항목 | 등급 | 내용과 완화 |
| :--- | :--- | :--- | :--- |
| 1 | 기능 2 희소 복사의 **부분 스냅샷**이 사용자 기대와 어긋날 수 있다 — "복사했는데 일부 키는 여전히 공통을 따라간다"는 인상 | 알려진 설계(의도) | §3.7.1 에 의미론을 명시하고, 복사 완료 후 팝업이 "복사된 키는 목적지, 복사 안 된 키는 공통 따름"을 그대로 표현하므로 UI 가 사실을 알린다. 사용자 피드백이 오면 **전부 물질화** 대안(§2 D3 기각)을 재검토할 수 있다 — 이 문서가 그 판을 남겨 둔다 |
| 2 | `settings_unset` 을 기능 1 의 `rows` 키에 처음 쓴다 | 낮음(확인됨) | `validate_per_device_key` 가 `keyRemap.rows` 를 허용하고 `remove_setting_key` 는 키 무관임을 코드로 확인했다. 테스트로 고정한다 |
| 3 | 복사 커맨드의 `feature` 문자열 + 디바이스 유효성 — 프런트 버그/수동 조작 | 낮음 | 화이트리스트 + `DeviceId::parse` + `validate_per_device_*` 재사용으로 저장 전 거부. 기존 `validate_per_device_key` 와 같은 정신 |
| 4 | 복사 버튼 **이중 클릭** → 커맨드 2회 | 낮음 | 복사는 멱등(같은 값을 두 번 쓰면 두 번째는 no-op)이라 안전. 필요하면 구현 시 버튼 disabled 처리로 완화 |
| 5 | 프런트 확인 판정(무엇이 바뀌는가)은 런타임 로직이라 정적 배선 테스트로 못 잡는다 | 알려진 한계 | 판정 자체의 단위 테스트는 JS 프레임워크가 없어 기존과 동일하게 수동 검증(§6.4)에 맡긴다. 다만 판정에 쓰는 헬퍼(`keyRemapRowsEqual`)는 이미 존재·검증됨 |
| 6 | i18n 하드코딩 29개 목록의 기존 누락 4개 | 확인된 사실 | 같은 패스에서 41개로 보충(§4.5). 명세 §4.1 표에도 `inheritedFromCommon`·`macosStatus.on/off/unknown` 행이 원래 없었던 것인지 확인 필요 — **(미확정)**: §4.1 표는 29키만 적고 있어서 명세 문서 쪽도 함께 갱신할지 구현 시 판단 |
| 7 | zh/es/ja 번역은 초안 — `(을)를` 중립 표기 등 한국어 copy 의 자연스러움 | 초안(제안) | 최종 copy 는 구현 단계에서 리뷰한다. 핵심은 **키 집합 일치**(테스트 강제)와 위치 인자 `{0}` 일치 |
| 8 | 복사 커맨드가 `reconfigure_engine` 을 한 번에 수행: 기능 2 최대 12키 쓰기가 한 락 안이라 파일 쓰기 실패 시 부분 저장 | 낮음 | `SettingsStore::set` 의 실패 관용(D-B — 메모리 값은 갱신, saveError 로 UI 알림)을 그대로 따른다. 부분 저장되더라도 희소 복사라 안전하게 수렴한다 |
| 9 | `settings_copy_common_to_device` 가 반환하는 `SettingsState` 는 전체 설정 상태(seek·presets 포함) — 복사 응답이 화면 전체 재렌더 | 낮음 | `renderState` 전면 재렌더는 `commit()` 의 기존 계약 그대로다 — 새 패턴이 아니다 |
| 10 | 명세 §3.7 번호·§3.1.5 개수 표 병합 충돌 | 낮음 | 반영 시 기존 절 번호와 대조한다. 다른 위임이 같은 문서를 동시에 고치고 있지 않은지 확인하고 진행(현재 `docs/spec/` 은 이 작업 외 변경 없음) |
