# 경로 B 디바이스 한정 매핑 — 실현 가능성 스파이크 (이슈 #21)

> **한 줄 요약** — ⭐ **경로 B(IOHID `UserKeyMapping`)의 디바이스 한정 매핑은 실제로 된다**(S-1, 실측). 그리고 **Consumer Page(0x0C) 목적지도 실제로 동작한다**(S-8, 실측 — 키 입력으로 확인). 그러나 세 가지 함정이 실측으로 드러났다: VID 단독 매칭은 `IOHIDSystem` 을 함께 잡아 사실상 전역 쓰기가 되고(S-3), IOHID 는 배열을 **전혀 검증하지 않으며**(S-7), 배열은 **병합이 아니라 통째 교체**라 마지막에 쓴 쪽이 이긴다(S-6).
> **이 문서의 역할** — `../spec/per-device-settings.md`(F-17)의 사실 토대. 명세의 판단은 전부 여기의 실측 결과 위에 서 있다.
> **⛔ 제품 코드는 만들지 않았다** — 전부 셸 명령(`hidutil` · `ioreg` · `ps`)과 사용자 키 입력 확인이다.

---

## 0. 실험 환경과 시스템 상태 복원

| 항목 | 값 |
| :--- | :--- |
| 기기 | `Mac16,11` (`sysctl -n hw.model`) |
| OS | macOS 26.5.2 (빌드 25F84) |
| 실험 일시 | 2026-08-30 15:15 ~ 15:29 (KST) |
| 대상 디바이스 A | `F108Pro Dongle` — VID `0x5ac`(1452) · PID `0x24f`(591) · USB · Built-In `0` |
| 대상 디바이스 B | `Wireless mouse 8k dongle-L` — VID `0x373b`(14139) · PID `0x11d9`(4569) · USB · Built-In `0` |

> 두 디바이스의 VID/PID 는 **이슈 #21 에 이미 공개된 범위**다. 그 밖의 개인 식별 정보는 싣지 않는다.

### ⭐ 시스템 상태 복원 — 확인함

| 시각 | 상태 |
| :--- | :--- |
| **15:15 (실험 전 baseline, 실측)** | `hidutil property --get UserKeyMapping` → `( )` · 디바이스 A/B 모두 전 서비스 빈 배열. **즉 실험 시작 시점에 설치된 매핑은 없었다** |
| 15:29 (실험 후 복원, 실측) | global `()` · 디바이스 A 3개 서비스 전부 `( )` · 디바이스 B 3개 서비스 전부 `( )` |
| 15:40 (사후 재확인, 실측) | global 및 두 디바이스에 `caps_lock → F18` 한 규칙. ⭐ **이것은 내 잔재가 아니다** — 내가 빈 배열로 복원한 뒤 이슈 #19 워크트리의 Ultrakey 앱이 재기동하며 **스스로 설치한 D-1** 이다(그 앱의 정상 동작). 내 실험 값은 한 건도 없다 |

**복원 완료.** 실험 중 사용한 값(`0x700000004`='a' · `0xC000000E9`=volume_up · `0x70000006A`=F15 · `0x700000069`=F14 · 엉터리 값 `0x9999999999`)은 **한 건도 남지 않았다**. ⛔ 사용자의 Karabiner 설정은 읽지도 바꾸지도 않았다.

### ⚠️ 실험 중 관측된 외부 간섭 (그리고 그것이 곧 발견이었다)

실험 도중 `caps_lock → F18` 매핑이 스스로 나타났다 사라지기를 반복했다. 원인을 추적한 결과:

```
ps ax → /Users/…/worktrees/Ultrakey/fix-physical-caps-lock-events/…/Ultrakey.app/…/ultrakey-app
        (PID 82665 → 이후 15102 로 교체 — 재빌드·재기동 중)
```

**이슈 #19 작업 워크트리의 Ultrakey 빌드가 동시에 실행 중이었고**, 그것이 D-1(`caps_lock → F18`)을 **전역으로**(`--matching` 없이) 설치하고 있었다. 이 간섭은 실험을 두 번 오염시켰지만(§S-6, §부록 A), 그 자체가 **F-17 이 반드시 풀어야 할 자원 경쟁 문제의 살아 있는 재현**이었다.

---

## 1. 실측 결과 요약

| # | 질문 | 판정 | 등급 |
| :--- | :--- | :--- | :--- |
| **S-1** | `--matching {VID,PID}` 가 그 디바이스에만 적용되는가 | ⭕ **된다** | **확정 (실측)** |
| **S-2** | 물리 디바이스 1개 = IOHID 서비스 몇 개인가 | **여러 개** (A 는 3개) | **확정 (실측)** |
| **S-3** | VID 단독 매칭으로 충분한가 | ⛔ **위험** — `IOHIDSystem` 을 함께 잡는다 | **확정 (실측)** |
| **S-4** | VID+PID 가 Apple 내장 키보드와 구분해 주는가 | — | **(미확정)** — 이 기기엔 내장 키보드가 없다. ⭐ 2026-09-03 **S-10** 이 MacBook Air 에서 부분 해소: 내장 키보드는 VID/PID 가 **없다**(`0:0`) |
| **S-5** | 한 디바이스 배열에 규칙 여러 개가 공존하는가 | ⭕ **공존한다** | **확정 (실측)** |
| **S-6** | 배열은 병합인가 교체인가 | **통째 교체** — 마지막에 쓴 쪽이 이긴다 | **확정 (실측, 비의도 재현 2회)** |
| **S-7** | IOHID 가 값을 검증하는가 | ⛔ **전혀 안 한다** — 엉터리 값도 그대로 저장 | **확정 (실측, 통제 실험)** |
| **S-8** | Consumer Page(0x0C) 목적지가 동작하는가 | ⭕ **동작한다** | **확정 (실측, 키 입력 확인)** |
| **S-9** | 리매핑 위에 macOS 의 `fn` 반전이 여전히 걸리는가 | — | **(미확정)** — 관측 수단이 없었다 |

---

## 2. S-1 ⭐ 디바이스 한정 매핑은 된다 — 이 명세의 토대

**방법.** 디바이스 B(마우스 동글)에만 `F13 → F15` 를 설치한 뒤, 같은 명령 안에서 A 와 B 를 번갈아 5회 연속 읽었다. 이때 A 에는 (외부 앱이 설치한) `caps_lock → F18` 이 들어 있었으므로, **서로 다른 두 배열이 동시에 존재하는지**를 볼 수 있었다.

```
hidutil property --matching '{"VendorID":14139,"ProductID":4569}' \
  --set '{"UserKeyMapping":[{"HIDKeyboardModifierMappingSrc":0x700000068,
                             "HIDKeyboardModifierMappingDst":0x70000006A}]}'
```

**결과** (15:17:05.044 ~ 15:17:05.179, 5회 전부 동일):

| 디바이스 | 배열 내용 |
| :--- | :--- |
| B (마우스 동글) | `Src=0x700000068`(F13) → `Dst=0x70000006A`(F15) ← **내가 넣은 것만** |
| A (F108Pro) | `Src=0x700000039`(caps_lock) → `Dst=0x70000006D`(F18) ← **다른 것만** |

⭐ **판정: `hidutil property --matching '{"VendorID":…,"ProductID":…}' --set '{"UserKeyMapping":…}'` 는 매칭된 디바이스에만 적용된다.** 서로 다른 두 배열이 두 디바이스에 동시에, 안정적으로 존재했다. `key-remapping-engine.md` §5 #10 이 `(미확정 — 이유는 해석)` 으로 남겨 둔 "경로 B 는 장치 단위로 적용되는 것으로 보인다" 는 **이로써 확정된다.**

---

## 3. S-2 물리 디바이스 1개 = IOHID 서비스 여러 개

`--matching` 으로 읽으면 디바이스 하나에 대해 **RegistryID 가 여러 줄** 나온다.

```
$ hidutil property --matching '{"VendorID":1452,"ProductID":591}' --get UserKeyMapping
RegistryID  Key              Value
100001251   UserKeyMapping   ( … )     ← UsagePage 12 / Usage 1  (Consumer)
10000129f   UserKeyMapping   ( … )     ← UsagePage 1  / Usage 2
100001292   UserKeyMapping   ( … )     ← UsagePage 1  / Usage 6  (Keyboard)
```

`hidutil list` 상 F108Pro 는 usage 조합 6종(`1/2`, `1/6`, `12/1`, `65369/97`, `65376/97`)으로 등장하며, 그중 `UserKeyMapping` 을 갖는 서비스가 3개다.

**함의 (F-17 이 지켜야 할 것):**
- **쓰기**는 `--matching` 한 번으로 그 디바이스의 전 서비스에 도달한다 — 서비스별로 나눠 쓸 필요가 없다.
- **읽기**는 서비스 수만큼 행이 나온다. 잔존 매핑 감지(§3-a2)는 이 행들을 **집계**해야 하고, 서비스마다 값이 다를 가능성(부분 실패)도 다뤄야 한다.
- UI 의 "디바이스 1개"는 IOHID 서비스 N개의 묶음이다. 디바이스 식별자는 서비스 단위가 아니라 **VID/PID 단위**여야 한다.

---

## 4. S-3 ⭐ VID 단독 매칭은 위험하다 — Apple VID 문제의 실제 모습

이슈 #21 은 "VID 1452 = 0x05AC 는 Apple 의 Vendor ID 라 내장 키보드와 구분되지 않을 수 있다" 고 경고했다. 실측 결과 **위험은 예상과 다른 곳에 있었다.**

```
$ hidutil list --matching '{"VendorID":1452}'
0x5ac  0x0    65280  23  IOHIDSystem                  (null)           Built-In 0
0x5ac  0x24f  1      2   AppleUserHIDEventService      F108Pro Dongle
0x5ac  0x24f  1      6   AppleHIDKeyboardEventDriver   F108Pro Dongle
0x5ac  0x24f  12     1   AppleHIDKeyboardEventDriver   F108Pro Dongle
…
```

⭐ **VID `0x5ac` 단독 매칭은 `IOHIDSystem`(VID `0x5ac` / PID `0x0` / UsagePage 65280 / Usage 23)을 함께 잡는다.** `IOHIDSystem` 은 개별 디바이스가 아니라 **시스템 전역 매핑의 대상 노드**다. 즉 VID 만으로 매칭해 쓰면 **디바이스 한정이 아니라 사실상 전역 쓰기**가 되어, 그 순간 모든 디바이스의 매핑을 갈아엎는다.

**규칙 (F-17 필수):** 매칭 사전에는 **반드시 `VendorID` 와 `ProductID` 를 함께** 넣는다. `VendorID` 단독 매칭과 매칭 없는 `--set` 은 **금지**한다.

---

## 5. S-4 (미확정) 내장 키보드와의 구분은 이 기기에서 확인 불가

```
$ hidutil list | awk '(built-in == 1) && (keyboard usage)'   → 0 건
$ sysctl -n hw.model                                          → Mac16,11
```

**이 기기에는 내장 키보드가 없다.** 따라서 "Apple VID 를 보고하는 서드파티 키보드"와 "진짜 Apple 내장 키보드"가 PID 로 구분되는지는 **실측할 수 없었다** — `(미확정)`.

**함께 남는 (미확정):**
- **동일 모델 2대**(같은 VID+PID)를 붙이면 두 대가 하나로 보인다. VID/PID 는 원리상 *모델* 식별자이지 *개체* 식별자가 아니다. 실측하지 않았다.
- `RegistryID` · `LocationID` 는 개체를 구분하지만 재부팅·포트 변경에 걸쳐 안정적인지 확인하지 않았다 — 설정 파일의 키로 쓸 수 있는지 `(미확정)`.

**재현 절차 (내장 키보드가 있는 Mac 에서):**
1. `hidutil list --matching '{"PrimaryUsagePage":1,"PrimaryUsage":6}'` 로 내장 키보드의 VID/PID 를 기록한다.
2. 외장 키보드에만 `--matching {VID,PID} --set` 으로 눈에 띄는 매핑을 건다.
3. 내장 키보드에서 같은 키를 눌러 **바뀌지 않는지** 확인한다.
4. 빈 배열로 복원한다.

---

## 6. S-5 · S-6 ⭐ 규칙은 공존하지만, 배열은 통째 교체된다

### S-5 — 한 배열에 규칙 여러 개는 공존한다 (실측)

```
--set '{"UserKeyMapping":[{Src:caps_lock, Dst:F18},
                          {Src:F13,       Dst:F15}]}'
→ 두 항목 모두 3개 서비스 전부에 저장되어 되읽힌다
```

즉 **D-1 과 F-17 의 규칙이 같은 배열에 함께 들어갈 수 있다.** 자원 공존은 원리적으로 가능하다.

### S-6 ⭐ — 그러나 쓰기는 병합이 아니라 통째 교체다 (실측, 비의도 재현 2회)

`--set` 은 배열 전체를 갈아치운다. 그래서 **여러 주체가 각자 쓰면 마지막에 쓴 쪽이 앞의 것을 전부 지운다.**

이것은 가정이 아니라 이번 실험에서 **두 번 실제로 일어났다**:

| 회차 | 관측 |
| :--- | :--- |
| 1 | 15:16 경 — A 에 걸어 둔 `F13 → F14` 가 사라지고 `caps_lock → F18` 로 대체됨 |
| 2 | 15:20:39 에 A 에 설치한 3규칙 배열이 15:23:51 에는 `caps_lock → F18` 한 줄만 남음. **그 사이에 사용자가 누른 키 입력 실험이 통째로 무효화되었다**(부록 A) |

범인은 **이슈 #19 워크트리의 Ultrakey 빌드가 D-1 을 전역으로(`--matching` 없이) 설치한 것**이었다.

⚠️ **이 스파이크 이후 #19 는 머지되었으나, 전역 쓰기는 그대로 남아 있다** — `crates/ultrakey-platform/src/hid_mapping.rs` 의 `HidutilBackend::apply()` 는 여전히 매칭 없이 `--set` 을 부른다. 즉 여기서 관측한 파괴는 **지금도 재현된다.**

⭐ **F-17 이 반드시 답해야 할 결론:**
1. Ultrakey 는 자신이 관리하는 디바이스의 `UserKeyMapping` 에 대해 **유일한 기록자**여야 한다.
2. **D-1 을 포함해 모든 경로 B 쓰기를 디바이스 한정으로 바꿔야 한다.** 지금처럼 전역으로 쓰면 자기 자신의 디바이스별 설정을 파괴한다 — 이번에 실제로 그랬다.
3. 엔진은 한 디바이스에 대해 **모든 규칙 출처를 합성해 완성된 배열 하나를 단일 `--set` 으로** 쓴다. 증분 쓰기는 금지한다.

---

## 7. S-7 ⭐ IOHID 는 값을 전혀 검증하지 않는다 — "받아들여졌다"는 근거가 아니다

능력 판정을 하기 전에 **통제 실험**을 먼저 했다. 결과가 이후 모든 해석을 바꿨다.

| 넣은 값 | 결과 |
| :--- | :--- |
| `Dst: 0x9999999999` (아무 의미 없는 값) | **그대로 저장되고 그대로 되읽힌다** |
| `Src: 0xFF00000003` (Apple `fn`) | 그대로 저장됨 |
| `Src: 0xC000000E9` (Consumer Page 를 소스로) | 그대로 저장됨 |
| `{"NotAKey":1, …}` (없는 키 이름) | 오류 없이 통과 |

⭐ **판정: `hidutil` 이 값을 받아들이고 되읽어 준다는 사실은, 그 매핑이 실제로 동작한다는 근거가 전혀 되지 않는다.** 커널은 배열을 그대로 보관할 뿐이다.

**이 프로젝트에 대한 함의:** 경로 B 의 능력 주장은 **키 입력으로 확인하지 않은 한 `(미확정)`** 이다. 되읽기 성공을 성공 판정으로 쓰는 구현·검증 절차는 전부 무효다.

---

## 8. S-8 ⭐ Consumer Page(0x0C) 목적지는 실제로 동작한다 — 기능 2 의 출력 측 확정

S-7 때문에 이 질문은 **키 입력으로만** 답할 수 있었다. 그래서 **대조군을 둔 통제 실험**으로 설계했다.

디바이스 A(F108Pro)에**만** 설치한 배열:

| 소스 | 목적지 | 역할 |
| :--- | :--- | :--- |
| `0x700000039` caps_lock | `0x70000006D` F18 | D-1 유지 |
| `0x700000043` **F10** | `0x700000004` `'a'` | **대조군** — Keyboard Page(0x07) |
| `0x700000042` **F9** | `0xC000000E9` volume_increment | **시험 대상** — Consumer Page(0x0C) |

> 대조군을 둔 이유: "아무 일도 없음" 이 *표현 불가* 때문인지 *매핑이 덮어써짐*(S-6) 때문인지 구분하기 위해서다. 실제로 1차 시도는 S-6 으로 무효가 되었고(부록 A), 대조군이 없었다면 이를 "Consumer Page 불가" 로 **오판했을 것이다.**
>
> 2차 시도에서는 S-6 을 이기기 위해 매핑을 **150초 동안 매초 재설치**하는 셸 루프를 돌린 상태에서 키를 눌렀다.

**결과 (사용자 키 입력 실측, 2차 시도):**

| 누른 키 | 관측 |
| :--- | :--- |
| **F10 단독** (대조군) | **`a` 가 입력됨** ✓ |
| **F9 단독** (시험 대상) | **볼륨이 올라감** ✓ |

⭐ **판정: `UserKeyMapping` 의 목적지로 Consumer Page(0x0C) usage 를 쓸 수 있고, 실제로 미디어/시스템 키로 동작한다.** 대조군이 함께 동작했으므로 매핑 자체가 살아 있었음도 확인된다.

즉 **기능 2 의 "F-키 → 시스템 기능 12종" 출력은 경로 B 로 표현 가능하다.** `NSSystemDefined` 이벤트 계층을 따로 다룰 필요가 없다.

> ⚠️ 12종 전부를 각각 확인한 것은 아니다. `volume_increment`(`0xC000000E9`) 한 종을 확인했다. 나머지 11종의 usage 값 대응(특히 `mission_control` · `spotlight` · `dictation` · `do_not_disturb` 처럼 표준 Consumer Page 에 대응이 자명하지 않은 것들)은 **`(미확정)`** 이다 — §10.

---

## 9. S-9 (미확정) `fn` 반전이 리매핑 위에 걸리는지

시험 대상 매핑이 살아 있는 동안 `Fn + F9` 를 눌러 보았으나, 사용자가 **그 결과를 관측할 수단이 없었다**(무엇이 일어났는지 판별 불가). → `(미확정)`.

**다만 구조적으로 확정된 것이 하나 있다.** `UserKeyMapping` 배열 항목은 `HIDKeyboardModifierMappingSrc` / `Dst` 쌍뿐이며 **modifier 조건 필드가 없다.** 이는 `key-remapping-engine.md` §3-d 가 이미 확정해 둔 것과 같다:

> 조건부 판정이 필요한 모든 경우 — `hidutil`/`IOHIDServiceClientSetProperty` 는 "이 키코드는 항상 저 키코드" 라는 정적 테이블만 표현할 수 있고, quick press·hold·조합 같은 시간·문맥 조건은 표현할 수 없다

⭐ **따라서 Karabiner 가 쓰는 형태 그대로의 `fn + F1 → 기능` 은 경로 B 로 표현할 수 없다.** 표현할 수 있는 것은 **조건 없는 `F1 → 기능`** 뿐이다. F-17 기능 2 는 이 제약 위에서 다시 정의되어야 한다(→ 명세 §3).

> S-7 때문에 "modifier 필드가 없다"를 실험으로 증명할 수는 없다(없는 키 이름도 조용히 삼켜지므로). 위 판정은 **되읽힌 스키마 관찰 + 기존 명세 §3-d** 에 근거한다.

**관련 실측 사실 하나.** 사용자의 macOS 설정은 `Use F1, F2, etc. keys as standard function keys` = **On** 이다(스크린샷). 이 상태에서 **F9 단독 입력이 Keyboard Page usage `0x42` 로 도착했다**(내 리매핑이 걸렸으므로 확정). 즉 이 토글이 켜져 있으면 F-키의 원시 usage 는 F-키 usage 이고, 우리는 그것을 갈아탈 수 있다. 토글이 **꺼져 있을 때** 무엇이 도착하는지는 시험하지 않았다 — `(미확정)`.

---

## 10. 디바이스 열거 — 무엇을 쓸 수 있는가

**셸에서는 `hidutil list` 로 필요한 정보가 전부 나온다** (실측):

```
$ hidutil list --matching '{"PrimaryUsagePage":1,"PrimaryUsage":6}'
VendorID ProductID LocationID UsagePage Usage RegistryID  Transport Class                       Product         Built-In
0x373b   0x11d9    0x2123220  1         6     0x100001259 USB       AppleUserHIDEventService     Wireless mouse… 0
0x5ac    0x24f     0x2123240  1         6     0x100001292 USB       AppleHIDKeyboardEventDriver  F108Pro Dongle  0
```

VID · PID · 제품명 · Transport · Built-In · RegistryID 를 모두 얻을 수 있다.

**기존 자산의 재사용 가능성** (`ultrakey-platform/src/hotplug.rs`):

| 항목 | 현황 |
| :--- | :--- |
| 감지 기구 | `IOServiceAddMatchingNotification`, 매칭 필터 `UsagePage 1 / Usage 6`(키보드만) — **F-17 이 필요로 하는 것과 정확히 같은 필터** |
| 권한 | Input Monitoring(TCC) **불필요** — 그대로 유지된다 |
| 공개 API | `pub enum HotplugEvent { Attached, Detached }` · `pub fn watch_keyboards(cb) -> Option<KeyboardHotplugWatcher>` |
| ⛔ 부족한 것 | **콜백이 이벤트 종류만 넘긴다 — VID/PID/제품명이 없다.** 열거 API 도 없다 |

⭐ **판정: 감지 기구는 재사용할 수 있으나, 그대로는 쓸 수 없다.** 콜백이 디바이스 속성을 실어 나르도록 확장하고(`io_iterator_t` 에서 `IORegistryEntryCreateCFProperty` 로 `VendorID`·`ProductID`·`Product` 를 읽으면 된다), 현재 붙어 있는 키보드를 **열거**하는 API 를 새로 더해야 한다. `drain_iterator` 가 이미 이터레이터를 순회하므로 확장 지점은 명확하다.

---

## 10-bis. S-10 ⭐ 내장 키보드는 VID/PID 가 **없다** — 이슈 #110 실측 (2026-09-03)

> **기기**: MacBook Air `Mac17,4` · macOS 26.6.2 (25G83). 이 스파이크의 원래 기기(`Mac16,11`)에는 내장 키보드가 없어 S-4 가 `(미확정)` 으로 남았던 자리를 이 실측이 채운다.
> **⛔ 이 절의 실측은 전부 읽기 전용**(`hidutil list` · `hidutil property --get` · `ioreg` · `keyboard_list_probe`)이다. `--set` 되읽기 결과는 이슈 #110 PR 의 실기기 검증 절(`../dev/manual-verification.md` 항목 21)에 있다.

| # | 확인 | 결과 | 등급 |
| :--- | :--- | :--- | :--- |
| S-10-1 | `hidutil list --matching '{"PrimaryUsagePage":1,"PrimaryUsage":6}'` | Services 1행: `AppleHIDKeyboardEventDriverV2` RegistryID `0x100000b4a`, **VendorID `0x0` · ProductID `0x0`**, LocationID `0xe8`(232), Transport `FIFO`, Built-In `1`, Product "Apple Internal Keyboard / Trackpad". Devices 1행: `AppleHIDTransportHIDDevice` RegistryID `0x100000b34`, 같은 값 | **확정 (실측)** |
| S-10-2 | `ioreg -c AppleHIDTransportHIDDevice -r -d1` — 그 IOHIDDevice 노드(`0x100000b34`, usage 1/6)의 프로퍼티 | `PrimaryUsagePage=1`·`PrimaryUsage=6`·`Built-In=Yes`·`LocationID=232`·`Transport="FIFO"`·`Product=…` 는 있으나 **`VendorID`/`ProductID` 키가 존재하지 않는다.** 같은 클래스의 나머지 5개 노드(usage 1/2·65280/3·65280/11·65280/13·65280/95)도 동일. 즉 `hidutil list` 의 `0x0` 은 **부재를 0 으로 표시한 것**이다 | **확정 (실측)** |
| S-10-3 | `cargo run -p ultrakey-platform --example keyboard_list_probe`(IOKit 직접 열거, 수정 전 코드) | 두 필터(`DeviceUsage*`·`PrimaryUsage*`) 모두 `0x100000b34` 1개 매칭, `VID=(못 읽음) PID=(못 읽음)` → `list_attached_keyboards()` **0대**. `read_device_properties` 가 VID/PID 부재를 항목 폐기로 다뤘기 때문 — 이슈 #86 원인 후보 (b) 가 여기서 확정된다 | **확정 (실측)** |
| S-10-4 | 앱 로그(`~/Library/Logs/Ultrakey/ultrakey.log`) | 기동 재조정 3회 모두 `Path B applied to attached devices count=0 d1_active=true devices=[]` — **D-1 설치 시도 자체가 없었다**(규칙 2 가 거부한 것이 아니다) | **확정 (실측)** |
| S-10-5 | `hidutil property --matching '{"VendorID":0,"ProductID":0,"PrimaryUsagePage":1,"PrimaryUsage":6}' --get UserKeyMapping` | `0x100000b4a` 1행 `(null)` | **확정 (실측)** |
| S-10-6 | `--matching '{"Built-In":1,"PrimaryUsagePage":1,"PrimaryUsage":6}'` / `'{"LocationID":232,"PrimaryUsagePage":1,"PrimaryUsage":6}'` | 각각 `0x100000b4a` 1행 | **확정 (실측, 이슈 #110 본문)** |
| S-10-7 | `--matching '{"LocationID":232}'` 단독 / `'{"RegistryID":…}'` | 3행(트랙패드·관리 서비스 포함) / 0행(`RegistryID` 는 매칭 키로 지원되지 않음) | **확정 (실측, 이슈 #110 본문)** |
| S-10-8 | 전역 `hidutil property --get UserKeyMapping` | `(null)` — 전역 잔재 없음 | **확정 (실측)** |
| S-10-9 | 수정 후 `keyboard_list_probe`(VID/PID 부재 → 0 폴백) | 열거 1대 `VID=0x0 PID=0x0 Product="Apple Internal Keyboard / Trackpad" Transport=FIFO Built-In=1` | **확정 (실측)** |
| S-10-10 | 경로 C(`IOHIDSetModifierLockState`, `caps_lock_toggle_probe`)로 잠금을 켰다 되돌리는 동안 세션 스트림(`tap_listen`, `TailAppendEventTap`) 관찰 | `kCGEventFlagsChanged`(type 12) 2건 — **keycode `0xFF`**, flags `0x00010100`(alphaShift on) → `0x00000100`(off). keycode `0x39`(caps lock) 는 **0건**. 즉 경로 C 는 `FlagsChanged` 를 싣지만 물리 caps lock 과 keycode 로 구분된다 — 이슈 #110 방어선(`Arbiter::d1_bypassed`, 조건 `0x39`)이 경로 C 에 되먹이지 않는다 | **확정 (실측)** |

**S-4 에 대한 답(부분).** 이 기기의 내장 키보드는 Apple VID `0x5ac` 를 보고하지 **않는다** — VID/PID 자체가 없다. 따라서 "Apple VID 서드파티 키보드 vs 내장 키보드" 구분 문제는 이 기기에서는 발생하지 않는다(내장은 `0:0`, 서드파티는 `5ac:<pid≠0>`). 다른 Mac 모델(USB/SPI 내장 키보드가 VID/PID 를 보고하는 세대)에서도 같은지는 `(미확정)`.

**§9 #7 에 대한 답.** `Built-In` 은 IOHIDDevice 노드의 프로퍼티 `Built-In`(CFBoolean `Yes`)으로 직접 읽힌다(S-10-2, S-10-9 의 `Built-In=1`). `hid_device.rs::read_bool_property` 가 이미 CFNumber/CFBoolean 둘 다 받으므로 코드 변경 없이 읽혔다.

**⚠️ 함의.** `IOHIDSystem`(S-3)은 VID `0x5ac`/PID `0x0` 이고 내장 키보드는 VID `0x0`/PID `0x0` 이다 — 두 경우를 "PID 가 0" 하나로 묶어 거부하면(D-17-4 규칙 2 의 옛 형태) 내장 키보드에 영원히 쓸 수 없다. usage 1/6 을 매칭 사전에 넣으면 `IOHIDSystem`(usage 65280/23)은 원리적으로 배제된다.

---

## 11. 이 스파이크가 F-17 명세에 강제하는 것

| # | 강제되는 규칙 | 근거 |
| :--- | :--- | :--- |
| 1 | 매칭 사전에 **VID+PID 를 항상 함께** 넣는다. VID 단독·매칭 없는 `--set` 금지 | S-3 |
| 2 | 경로 B 쓰기는 **전부 디바이스 한정으로** 바꾼다 — **D-1 포함** | S-1, S-6 |
| 3 | 디바이스별로 **완성된 배열 하나를 단일 `--set`** 으로 쓴다. 증분 쓰기 금지 | S-5, S-6 |
| 4 | 잔존 매핑 감지·정리는 디바이스 단위로, 서비스 여러 행을 **집계**해 판정한다 | S-2, S-6 |
| 5 | 되읽기 성공을 **동작 확인으로 쓰지 않는다** | S-7 |
| 6 | 기능 2 의 출력(미디어 키)은 경로 B 로 **표현 가능**하다 | S-8 |
| 7 | 기능 2 를 `fn + F1` 이 아니라 **F-키 자체의 재정의**로 다시 정의한다 | S-9 |
| 8 | 디바이스 식별자는 VID/PID 기반. 동일 모델 2대는 **한계로 명시**한다 | S-4 |
| 9 | `hotplug.rs` 를 **디바이스 속성을 싣도록 확장**하고 열거 API 를 더한다 | §10 |

---

## 부록 A — 1차 키 입력 시도가 무효가 된 경위 (S-6 의 실물 증거)

기록으로 남길 가치가 있다. **대조군이 없었다면 오판했을 사례**다.

| 시각 | 사건 |
| :--- | :--- |
| 15:20:39 | 디바이스 A 에 3규칙 배열 설치. 되읽기로 3규칙 전부 확인 |
| (그 사이) | 사용자가 키를 누름 — "단독 기능키는 기능키로 동작함, Fn+F10 은 무음, Fn+F11/F12 는 볼륨 다운/업" |
| 15:23:51 | 되읽기 → **`caps_lock → F18` 한 줄만 남음.** F9·F10 규칙 소멸 |

사용자가 관측한 것은 **전부 macOS 기본 동작**이었다 — 매핑이 이미 지워져 있었기 때문이다. 되읽기를 하지 않았다면 이를 "Consumer Page 표현 불가" 로 확정했을 것이고, **F-17 기능 2 는 근거 없이 기각되었을 것이다.**

2차 시도는 (a) 매핑을 매초 재설치해 S-6 을 이기고, (b) 대조군을 함께 두고, (c) 키를 누른 **직후** 매핑 생존을 되읽기로 확인해 결과의 유효성을 보장했다.

**교훈 — 경로 B 실험의 필수 절차:** ① 실험 전 baseline 기록 → ② 대조군 포함 설치 → ③ **설치 직후 되읽기** → ④ 키 입력 → ⑤ **직후 되읽기로 생존 확인** → ⑥ 복원 → ⑦ 복원 검증.
