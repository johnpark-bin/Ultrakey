# 이슈 #86 — Keyboards 탭에서 내장(built-in) 키보드가 목록에 보이지 않는다 — 계획 초안

> **성격**: `docs/plan/` 은 구현 위임 전 계획 문서를 두는 자리다. 이 문서는 중급(계획 초안)이 작성한 **초안**이며, 확정은 상급 리뷰(`ultrakey-review`) 후 호출 세션이 한다. 확정되면 결정 목록(D1~D5)이 정본이 되고, §5 의 명세 갱신 초안이 `docs/spec/per-device-settings.md` 에 반영된 뒤 구현이 위임된다.

---

## 1. 요약

**결론부터**: 이 이슈는 **UI 표시 문제가 아니라 열거(enumeration) 문제**다. `build_per_device_view()`(main.rs:692)는 `list_attached_keyboards()` 결과를 그대로 좌측 패인에 매핑할 뿐이므로, 내장 키보드가 목록에 안 보인다는 것은 곧 **`list_attached_keyboards()` 가 내장 키보드를 돌려주지 않는다**는 뜻이다. 그리고 이는 **기능적 결함**이기도 하다 — 엔진의 `reconcile_on_start`/`apply_all`(path_b.rs:197-203)이 `attached` 목록의 디바이스에만 D-1(`caps lock → F18`)과 디바이스별 설정을 쓰므로, 내장 키보드가 목록에서 빠지면 **내장 키보드의 caps lock 정규화(D-1)와 F-17 설정이 통째로 적용되지 않는다.**

**원인은 미확정이며, 실측으로 판정한다.** 코드 실측으로 확인된 사실 두 가지가 후보를 좁힌다:

1. **매칭 키 불일치(후보 a)**: 코드의 매칭 사전(`build_keyboard_matching_dict()`, hotplug.rs:96-129)은 `DeviceUsagePage`/`DeviceUsage` = 1/6 을 쓴다(ffi.rs:169-170). 반면 명세 §3.2 가 정본으로 인용하는 `hidutil list` 는 `PrimaryUsagePage`/`PrimaryUsage` = 1/6 을 쓴다(스파이크 §10). **두 키는 IOKit 의 서로 다른 프로퍼티다.** 외장 키보드(F108Pro)에서는 두 필터가 같은 결과를 줬지만(수동 검증 C-1 통과), 내장 키보드의 IOHIDDevice 서비스에서도 같은지는 실측되지 않았다.
2. **VID/PID 읽기 실패 시 항목 폐기(후보 b)**: `read_device_properties()`(hid_device.rs:79-92)는 `VendorID`/`ProductID` 를 못 읽으면 `None` 을 돌려 항목을 버린다.

`built_in` 속성은 읽어 `DeviceInfo.built_in`(perdevice/mod.rs:105)에 담기기만 하고 **어디서도 필터에 쓰이지 않는다** — 즉 "내장 제외" 로직은 존재하지 않으므로, 누락은 위 두 경로 중 하나다.

**수정 방향**: (A) 매칭 키를 `PrimaryUsagePage`/`PrimaryUsage` 로 정렬(명세 §3.2 가 이미 hidutil 과 "같은 매칭 사전"을 요구)을 1순위 권장. (B) VID/PID 폴백은 **기각** — 저장 키·경로 B 쓰기가 전부 VID/PID 를 전제하므로. (C) `built_in` 라벨 표시는 별도 결정(D3)으로 분리.

**실기기 부재 시**: 진단용 probe 확장(읽기 전용)을 먼저 머지하고, (A) 는 현재 기기에서 두 필터 결과가 동일함을 확인한 뒤 회귀 없음 판정 하에 머지하되 "내장 키보드 포함 여부는 실기기 미검증"으로 문서화한다. (C) 는 보류.

---

## 2. 결정 목록 D1~D5

### D1 — 원인 판정: 확장 probe 를 정본으로 확정, 판정 기준 3단계로 고정

기존 `keyboard_list_probe.rs` 는 중복 제거 후 최종 목록만 출력하므로(58행) 판정 불가다. probe 를 **진단 모드**로 확장한다(읽기 전용, `UserKeyMapping` 불변):

- **출력 1 — 원시 서비스 덤프**(중복 제거 전): 현재 매칭 사전으로 매칭된 모든 IOHID 서비스에 대해 `RegistryID`·`VendorID`·`ProductID`·`Product`·`Transport`·`Built-In`·`DeviceUsagePage`·`DeviceUsage`·`PrimaryUsagePage`·`PrimaryUsage`.
- **출력 2 — 이중 매칭 비교**: `PrimaryUsagePage`/`PrimaryUsage` 사전으로 같은 열거를 한 번 더 돌려 결과 집합 차이 출력.
- **출력 3 — 기존 dedup 회귀 체크 유지**.

**판정 기준**:

| 관측 | 판정 |
| :--- | :--- |
| 내장 키보드 서비스가 출력 1 에 **없다** | 원인 (a) — 매칭 필터 문제 → (A) 채택 |
| 출력 1 에는 있는데 VID/PID 가 `(못 읽음)` | 원인 (b) — `None` 폐기 → (B) 검토 |
| 출력 1 에는 있는데 dedup 후 사라짐 | 원인 (c) — dedup 충돌 → 별도 대응 |
| 출력 1·2 모두에 없음 | `ioreg -l` 덤프로 재판정 |

### D2 — 수정 방향: (A) 매칭 키 정렬 1순위, (B) VID/PID 폴백 기각, (C) 는 D3

**(A) 권장**: `build_keyboard_matching_dict()` 의 필터 키를 `PrimaryUsagePage`/`PrimaryUsage` 로 바꾼다(ffi.rs 에 상수 추가). 이 함수는 열거(`hid_device.rs`)와 핫플러그(`hotplug.rs`)가 **공유**하므로 한 곳만 고치면 두 경로가 함께 정렬된다.

- 근거: 명세 §3.2 는 "구현은 **같은 매칭 사전**을 IOKit 으로 쓴다"고 규정, 정본은 `hidutil list --matching '{"PrimaryUsagePage":1,"PrimaryUsage":6}'`. 코드가 쓰는 `DeviceUsagePage`/`DeviceUsage` 는 그 정본과 **다른 IOKit 프로퍼티 키**. `hidutil list`(PrimaryUsage)가 내장 키보드를 보여 주는 것은 일반적으로 알려진 동작 `(추정 — 이 레포 검증기기엔 내장 키보드 없음)`.

**(B) 기각 — VID/PID 폴백**: 저장 키 `<vid>:<pid>` 체계·`DeviceId::parse`·`to_match()`·`--matching {VID,PID}` 쓰기 전부가 VID/PID 를 전제. VID/PID 가 정말 없으면 per-device 설정·D-1 설치가 원리적으로 불가능 → 목록에 보이기만 하고 설정이 안 되는 거짓 약속. 대체안: probe 가 원인 (b) 를 확인하면 **부모 IORegistry 엔트리에서 VID/PID 폴백**을 먼저 시도하고, 그것도 실패하면 "per-device 설정 불가"로 문서화·제외 유지.

**(C) 를 주 수정으로 삼는 것 기각**: `built_in` 필터는 이미 없으므로 무의미. "명시 포함"은 곧 (A)이고, "라벨 표시"는 D3.

### D3 — built-in 라벨 표시: 권장하되 실기기 검증 후로 미룬다

`PerDeviceDeviceView`(main.rs:618)에 `built_in: Option<bool>` 추가 + `populateDeviceList`(settings.html:1537-1557)에서 `(내장)` 라벨. i18n `preferences.keyboards.devicePicker.builtIn` 1키 × 5언어.

- 근거: Apple VID(0x5ac)는 서드파티 키보드와 내장이 **공유** — 제품명만으로 구분 불가할 수 있다.
- **단, 실기기 검증 선행**: 수동 검증 C-1 에서 `Built-In` 읽기가 실패("(못 읽음)"). 내장 키보드에서 `Built-In` = 1 이 읽히는지는 실측된 바 없다(명세 §9 Q7). 확인 전 라벨 도입은 무의미.

### D4 — 회귀 보호: dedup 유지 + hidutil 대조 게이트 + 핫플러그 공유 영향 검증

1. **(VID,PID) dedup 유지**(hid_device.rs:53-61) — probe 의 "중복 시 exit 1"이 게이트.
2. **hidutil 대조 게이트** — 수정 후 probe 결과가 `hidutil list --matching '{"PrimaryUsagePage":1,"PrimaryUsage":6}'` 의 물리 디바이스 대수와 일치(수동 검증 C-1 기준).
3. **핫플러그 공유 영향 검증** — 공유 dict 를 고치므로 핫플러그 통지 집합도 바뀐다. 기존 핫플러그 테스트 통과 확인.

### D5 — 실기기 부재 시: 문서화 후 연기 (코드 가드 추가 기각)

1. **진단 probe 확장 먼저 머지**(읽기 전용, 무해).
2. **(A) 은 현재 기기에서 두 필터 결과 동일 확인 후 머지** — no-op 이면 회귀 없음, 명세 §3.2 정렬이라는 독립 정당성. "내장 키보드 포함 여부는 실기기 미검증"으로 문서화.
3. **(C) 보류**.
4. 이슈를 "실기기 검증 대기"로 유지.

**기각 — 코드 가드 추가**: "내장 키보드가 목록에 있어야 한다" 불변식은 현재 항상 실패 → 가드 추가는 항상 오류. 원인 미상 상태에서의 가드는 추측으로 명세를 채우는 것.

---

## 3. 작업 분해 (파일 단위)

### 3-1. 진단 단계 (실기기 필요)

| # | 파일 | 작업 |
| :--- | :--- | :--- |
| 1 | `crates/ultrakey-platform/examples/keyboard_list_probe.rs` | 진단 모드 확장 — 원시 서비스 덤프 + 이중 매칭 비교. dedup 회귀 체크 유지. **실기기 없이도 머지 가능** |
| 2 | (실기기) | 확장 probe + hidutil 대조 + `ioreg -l` 덤프 → 판정 |

### 3-2. 수정 단계 (판정 기반)

| # | 파일 | 작업 | 채택 조건 |
| :--- | :--- | :--- | :--- |
| 3 | `crates/ultrakey-platform/src/ffi.rs` | `K_IOHID_PRIMARY_USAGE_PAGE_KEY`·`K_IOHID_PRIMARY_USAGE_KEY` 상수 추가 | D2 (A) |
| 4 | `crates/ultrakey-platform/src/hotplug.rs` | `build_keyboard_matching_dict()` 키 교체 + doc 주석 | D2 (A) |
| 5 | `crates/ultrakey-platform/src/hid_device.rs` | (원인 b 시) VID/PID 부모 엔트리 폴백 | D2 (B) 대체안 |
| 6 | `apps/ultrakey-app/src/main.rs` | `PerDeviceDeviceView.built_in` 필드 추가 | D3 |
| 7 | `apps/ultrakey-app/ui/settings.html` | 내장 라벨 표시 | D3 |
| 8 | `resources/i18n/{en,ko,zh,ja,es}.json` | `preferences.keyboards.devicePicker.builtIn` 키 | D3 |
| 9 | `apps/ultrakey-app/tests/frontend_wiring.rs` | i18n 키 개수 갱신 + 배선 테스트 | D3 |

### 3-3. 문서 단계

| # | 파일 | 작업 |
| :--- | :--- | :--- |
| 10 | `docs/spec/per-device-settings.md` | §3.2(매칭 키·내장 포함)·§3.1.2(라벨)·§8(수용 기준)·§9(Q1·Q7) |
| 11 | `docs/dev/manual-verification.md` | 부록 C-1 에 내장 키보드 절차 추가 |
| 12 | `docs/research/per-device-hid-spike.md` | 실기기 확보 시 실측 기록 |

**엔진 변경 없음** — `path_b.rs`·`engine.rs` 는 `attached` 목록을 주입받아 순회할 뿐.

---

## 4. 테스트 / 실기기 계획

### 4-1. 자동 (실기기 불필요)

- `hotplug.rs` 기존 테스트(IOKit 환경 실행 패턴) 통과
- `frontend_wiring.rs` i18n 키 개수 46→47 · 내장 라벨 배선
- probe dedup 회귀 체크

### 4-2. 실기기 (내장 키보드 있는 MacBook)

1. 확장 probe 실행 → 원시 덤프에서 내장 키보드 존재/누락 단계 판정
2. `ioreg -l` 대조 → `Built-In` 프로퍼티 키·타입 확인(§9 Q7 해소)
3. 수정 후 probe → 내장 포함·dedup 1대당 1항목
4. 앱 실행 Keyboards 탭 → 내장 키보드 좌측 패인 표시(라벨 포함)
5. 내장 키보드에 기능 1 등록 → 그 키보드에서만 동작
6. caps lock 프리셋 → 내장 키보드 D-1 정규화 동작
7. 내장 키보드 `--matching {VID,PID} --set` 되읽기 + 키 입력
8. 외장 키보드 목록 대조 → 수정 전과 동일·중복 없음
9. 재기동 → 재조정이 내장에도 적용
10. 내장 설정 후 뽑기/재연결 → `(연결 안 됨)`·재적용

### 4-3. 실기기 부재 시

- probe 확장 + 매칭 정렬(#3·#4)만 머지, 현재 기기 두 필터 결과 동일 확인
- 내장 포함 여부 `(미확정)` 문서화, 이슈 "실기기 검증 대기"
- 라벨(#6~#9) 보류

---

## 5. 명세 갱신 초안 — `docs/spec/per-device-settings.md`

### §3.2 열거 — 매칭 키 명시

> **열거**: `hidutil list --matching '{"PrimaryUsagePage":1,"PrimaryUsage":6}'` 로 VID·PID·제품명·Transport·Built-In·RegistryID 를 얻을 수 있다(실측, 스파이크 §10). 구현은 **같은 매칭 사전**을 IOKit(`IOServiceMatching(kIOHIDDeviceKey)` + **`PrimaryUsagePage`/`PrimaryUsage`** 필터)으로 쓴다 — ⚠️ `DeviceUsagePage`/`DeviceUsage` 가 아니다(이슈 #86). **내장 키보드도 목록에 포함된다.**

### §3.1.2 항목 구성 표 — 내장 라벨

> `Built-In` = 1 인 항목은 제품명 뒤에 `(내장)` 라벨 — Apple VID(0x5ac)는 서드파티 키보드와 공유되므로 제품명만으로 구분이 안 될 수 있다.

### §8 수용 기준 — 추가

- [ ] 내장 키보드가 있는 Mac 에서 Keyboards 탭 좌측 패인에 내장 키보드가 나타나고(제품명 + `(내장)` 라벨), 서드파티 키보드 목록은 수정 전과 동일하다.
- [ ] 내장 키보드에 기능 1/2 설정을 등록하면 그 키보드에서만 동작하고 외장 키보드에는 영향이 없다.
- [ ] caps lock 의존 프리셋이 켜진 상태에서 내장 키보드의 caps lock 이 정규화된다.

### §9 미해결 질문 — 갱신

- Q1(Apple VID 구분)·Q7(`Built-In` 컬럼 소스) — 실측 결과 반영(내장 키보드 기기 확보 시).

---

## 6. 리스크 / 미확인

| # | 리스크 | 완화 |
| :--- | :--- | :--- |
| 1 | 매칭 키 변경이 핫플러그에도 영향(공유 dict) | 같은 집합으로 정렬되므로 정합적. 핫플러그 테스트 + 실기기 #8 |
| 2 | 내장 키보드 `--matching {VID,PID}` 쓰기 미검증 | 실기기 #7(S-7 절차)로 확정 |
| 3 | 내장 서비스가 서로 다른 (VID,PID) 를 보고 → dedup 다중 항목 | probe 원시 덤프로 확인, 별도 대응 |
| 4 | `Built-In` 읽기 현재 기기 실패 | 내장 키보드에서 읽히는지 확인 후에만 라벨(D3) |
| 5 | 내장 등장 시 `_managed` 원장 처리 | 의도된 동작(§3.6 규칙 6). 실기기 #9 |
| 6 | 내장·외장 같은 (VID,PID) | 희박 `(추정)`. probe 덤프 확인 |

**미확인**: 내장 키보드 IOHID 서비스 구조 · `Built-In` 프로퍼티 키·타입 · 내장에 대한 `--matching` 쓰기 동작 · `PrimaryUsagePage` 필터가 내장 키보드를 매칭한다는 것 `(추정)`

---

## 7. ⭐ 상급 리뷰 요청 사항

1. **D2 의 (A) — 매칭 키 정렬을 실기기 확인 없이 머지할지**: 현재 기기에서 no-op 이면 회귀 없음 판정 하에 머지 가능하다고 보지만, "수정이 실제로 이슈를 고치는지"는 실기기 전까지 미검증. 머지 vs 전량 보류.
2. **D3 의 라벨을 이번 이슈에 포함할지**: 범위가 커지지만 이슈 본질에 부합. `Built-In` 읽기 실기기 검증 선행 조건.
3. **원인 (b) 확인 시 대응 방향**: 부모 엔트리 폴백 → 실패 시 "per-device 설정 불가" 문서화 — D-1 이 내장 키보드에 설치 불가능해지는 경우의 제품 판단.
---

## 8. ⭐ 상급 리뷰 반영 (2026-09-02, `ultrakey-review`)

> 판정: **조건부 통과** — 문서 정정 3건 반영 후 확정. 모든 코드 인용 실측 대조 완료.

### 판정 요지
- **(A) 실기기 없는 머지 — 조건부 승인.** 게이트(현재 기기 두 필터 일치 + hidutil 대조 + "실기기 미검증" 문서화) 충분. `hidutil` 자체가 `PrimaryUsagePage` 필터라 정렬 방향이 정본 앵커. 내장이 그래도 안 보이면 probe 가 (b)/(c) 판정.
- **D3 라벨 — 연기 승인.** `Built-In` 읽기가 C-1 에서 실패했으므로 미검증 데이터 위 라벨은 "지어내지 않는다" 위반.
- **원인 (b) 제품 판단 — 폴백 체인 승인** + "설정 불가"를 **조용한 제외 아닌 명시 노출**로 명세화할 조건.

### 반영 수정 목록
| # | 위치 | 수정 |
| :--- | :--- | :--- |
| 1 | §1 요약 | "`build_per_device_view()` 가 결과를 그대로 매핑" → 실제는 `list_attached_keyboards()` **∪ 설정 키 존재 디바이스** 합집합(`main.rs:692` 주석). 결론(내장은 열거로만 진입)은 불변 |
| 2 | D3 | `populateDeviceList` 라인 정정: **1568행**(1537-1557 은 `buildDeviceOption` 영역) |
| 3 | §5 | 명세 갱신 대상에 `per-device-settings.md:534` **§6 플랫폼 API 표**(`IOServiceMatching` + `UsagePage`/`Usage` 필터 행) 추가 |
