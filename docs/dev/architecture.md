# Ultrakey 아키텍처 — 모듈 경계와 동시성 설계

> 이 문서는 **M1 위임(이슈 #5)에서 확정한 코드 구조의 근거**를 남긴다.
> 명세(`docs/spec/`)는 "무엇을 만드는가"를 정의하고, 이 문서는 "그것을 어떤 경계로 쪼개는가"를 정의한다.
> M2·M3 위임은 여기서 정한 경계 위에 올라탄다 — 경계를 바꿔야 한다면 이 문서를 먼저 고친다.

---

## 1. 크레이트 경계와 F-xx 대응

경계를 정하는 기준은 하나다: **`unsafe` FFI 와 순수 판정 로직을 물리적으로 분리하고, 명세 파일 1개가 크레이트(또는 크레이트 안의 모듈) 1개에 대응하게 만든다.**
후자는 이후 기능 단위 병렬 위임에서 같은 파일을 동시에 고치는 충돌을 없애기 위한 것이다(`docs/spec/README.md` "기능당 파일 1개로 분할" 결정과 같은 논리).

| 크레이트 | 대응 명세 | 역할 | `unsafe` | macOS 필요 |
| :--- | :--- | :--- | :---: | :---: |
| `ultrakey-core` | F-07 의 **판정 부분**, F-10 게이트 상태, F-15 선행 | 정본 키 상태 테이블, quick press 상태 머신, 중재 우선순위 표, 규칙 테이블, 설정 타입, 키코드 enum | ❌ 없음 | ❌ 불필요 |
| `ultrakey-platform` | 전 기능의 플랫폼 경계 | **모든 `unsafe` FFI 가 여기에만 있다.** CGEvent/EventTap, IOKit(경로 B·C·핫플러그), Carbon HIToolbox(TIS/UCKeyTranslate), AX, NSWorkspace, Secure Input, CFRunLoop | ✅ 전량 | ✅ |
| `ultrakey-engine` | F-07 의 **인프라 부분** | 전용 스레드 런루프, 탭 생명주기 FSM, 워치독, 절전·깨어남·세션 훅, 핫플러그 재적용, 경로 B/C 관리자 | 간접 | ✅ |
| `ultrakey-layout` | F-14 (B) | 입력 소스 독립 판정 — ASCII 폴백, 정/역방향 테이블, 캐시 무효화 | 간접 | ✅ (테스트는 ❌) |
| `ultrakey-hyperkey` | F-05 | hyper·meh·bleh 규칙 정의와 비트마스크 합성. **엔진에 규칙만 등록한다** | ❌ 없음 | ❌ 불필요 |
| `ultrakey-permissions` | F-11 | `AXIsProcessTrusted` 폴링, 권한 상태 머신, out-of-sync 진단, 시스템 설정 딥링크 | 간접 | ✅ |
| `ultrakey-i18n` | F-14 (A) / D4 | 문자열 카탈로그(ko + en), 로케일 결정, OS 버전별 어휘 교체 | ❌ 없음 | ❌ 불필요 |
| `apps/ultrakey-app` | F-09·F-10 의 껍데기 | Tauri 앱. M1 에서는 권한 안내 모달 + 배선(wiring)만 | 간접 | ✅ |

### 왜 이 경계인가 — 기각한 대안

| 결정 | 근거 | 기각한 대안 |
| :--- | :--- | :--- |
| **`unsafe` 를 `ultrakey-platform` 한 크레이트에 전량 격리** | 명세가 요구하는 FFI 표면이 크다(CGEvent 14종, IOKit 3계열, Carbon 5종, AX, NSWorkspace). 흩어 두면 "이 `unsafe` 가 지키는 불변식이 무엇인가"를 검토할 지점이 파일마다 생긴다. 한곳에 모으면 **안전 래퍼의 계약을 한 파일에서 검토**할 수 있고, 나머지 크레이트 전부에 `#![forbid(unsafe_code)]` 를 걸 수 있다 | 기능 크레이트마다 필요한 FFI 를 직접 선언 — 같은 `extern "C"` 선언이 중복되고, 시그니처가 갈리면 UB 가 조용히 들어온다 |
| **판정 로직(`ultrakey-core`)을 macOS 비의존 순수 크레이트로 분리** | 이것이 Phase 7("자동화 가능한 것은 자동화한다")을 성립시키는 유일한 구조다. 중재 우선순위 표·quick press 상태 머신·비트마스크 합성은 전부 `InputEvent → Outcome` 순수 함수로 쓸 수 있고, 그러면 실제 키보드 없이 v1.20/v1.62 회귀 시나리오를 단위 테스트로 재현할 수 있다(F-07 §8) | 엔진 안에 판정을 함께 두기 — 탭이 살아 있어야만 테스트가 되고, 그러면 CI 에서 아무것도 검증할 수 없다 |
| **F-05 를 별도 크레이트로** | 규칙 제공자(rule provider) 자리를 M1 에서 하나 만들어 두면, M2 의 F-08(Presets 16종)이 `ultrakey-presets` 로 **같은 자리에 나란히** 들어온다. 형태가 이미 있으면 위임 지시가 짧아진다 | `ultrakey-core` 안의 모듈로 흡수 — F-08 이 들어올 때 core 가 비대해지고, 두 위임이 같은 크레이트를 고치게 된다 |
| **F-14 (B) 를 `ultrakey-engine` 이 아니라 별도 크레이트로** | 레이아웃 테이블 구축은 `UCKeyTranslate` 호출을 트레이트로 주입하면 **가짜 레이아웃으로 단위 테스트가 된다**. 엔진 안에 두면 그 이음매가 사라진다 | 엔진의 모듈로 — 명세가 별도 파일인데 코드만 합치면 M5 의 F-14(A) 위임이 갈 곳이 애매해진다 |
| **Tauri 를 앱 크레이트 하나에만 의존시킨다** | 엔진·판정 로직이 Tauri 를 모르면 `cargo test -p ultrakey-core` 가 webview 툴체인 없이 돈다. 또한 F-03(오버레이)이 렌더링 계층을 교체 가능하게 분리해야 한다는 `platform-constraints.md` P3 요구와도 일관된다 | 엔진이 `AppHandle` 을 직접 들고 emit — 테스트 불가, 그리고 콜백 스레드가 Tauri 상태를 만지게 되어 §2 의 동시성 규약이 깨진다 |

---

## 2. ⭐ `CGEventTap` 콜백의 동시성·수명 설계

**이 절이 이 문서에서 가장 중요하다.** `key-remapping-engine.md` §3-a 는 콜백 안에서 블로킹 I/O·AX 순회·**뮤텍스 경합 가능성이 있는 잠금 획득**을 금지한다. 그 금지를 "조심해서 지키는 규약"이 아니라 **구조적으로 위반할 수 없는 형태**로 만드는 것이 목표다.

### 2.1 스레드 배치

```
                    ┌──────────────────────────────────────────┐
 메인 스레드         │ Tauri / NSApplication 런루프             │
 (Tauri 소유)        │  · 권한 모달, 설정 UI (M2~)              │
                    │  · NSWorkspace 알림 구독 (절전/깨어남/    │
                    │    세션/최전면 앱)                        │
                    │  · TIS 입력 소스 변경 알림 구독           │
                    └───────┬──────────────────────┬───────────┘
                            │ ArcSwap 원자적 교체    │ 커맨드 큐 + CFRunLoopSource0 시그널
                            ▼                      ▼
                    ┌──────────────────────────────────────────┐
 이벤트 탭 전용      │ 독립 CFRunLoop (kCFRunLoopCommonModes)   │
 스레드              │  · CGEventTap 콜백  ← 임계 경로          │
 ("ultrakey-tap")   │  · quick press 타이머 (CFRunLoopTimer)   │
                    │  · 커맨드 소스 (CFRunLoopSource v0)      │
                    │  · 정본 키 상태 테이블 (배타 소유)        │
                    └──────────────────────────────────────────┘
                            ▲
                            │ Recover 커맨드 / 치명적 실패 시 직접 에스컬레이션
                    ┌───────┴──────────────────────────────────┐
 워치독 스레드       │ 1초 주기 CGEventTapIsEnabled 폴링        │
                    └──────────────────────────────────────────┘
```

전용 스레드 결정 자체는 `key-remapping-engine.md` §7 과 `platform-constraints.md` §5.2 가 이미 내렸다. 이 문서가 더하는 것은 **"그 대가로 생기는 상태 공유를 어떻게 락 없이 해결하는가"** 다.

### 2.2 락을 쓰지 않는 세 가지 장치

| 공유 대상 | 방향 | 장치 | 근거 |
| :--- | :--- | :--- | :--- |
| **정본 키 상태 테이블**(눌림 여부, 눌린 시각, quick press FSM) | 탭 스레드 전용 | **공유하지 않는다.** 탭 스레드가 배타 소유하며 `&mut` 로만 접근 | 이 테이블은 콜백과 타이머만 만진다. 둘 다 같은 런루프 위에서 직렬화되므로 애초에 경합이 없다. `Mutex` 를 쓰면 "경합이 없는데 락을 잡는" 비용만 남는다 |
| **설정·규칙 테이블**, **레이아웃 역산 테이블** | 메인 → 탭 (드묾), 탭에서 매 이벤트 읽음 | `ArcSwap<T>` — 읽기는 대기 없음(wait-free), 쓰기는 원자적 포인터 교체 | 읽기:쓰기 비율이 극단적으로 읽기 우세다. `RwLock` 은 writer 가 끼어들면 reader 가 블로킹될 수 있고, 그 순간이 곧 §3-a 의 타임아웃이다 |
| **앱별 비활성화 게이트**(F-10), **Seek 세션 활성**(F-01) | 메인 → 탭 | `AtomicBool` — 판정 결과를 **메인 스레드에서 미리 계산해** 부울 하나로 게시 | 명세 §3-b 계층 1 이 "전역 플래그 1개로 O(1) 확인"을 요구한다. 콜백이 번들 ID 문자열을 목록과 비교하면 O(n) 문자열 비교 + 할당이 임계 경로에 들어온다 |
| **탭 스레드로 보내는 명령**(상태 리셋, 경로 B 재적용, 탭 복구, 종료) | 메인·워치독 → 탭 | 무잠금 MPSC 큐 + **`CFRunLoopSource` (version 0)** 시그널 | 탭 스레드는 런루프 안에서 잠들어 있다. 큐에 넣고 `CFRunLoopSourceSignal` + `CFRunLoopWakeUp` 하면 런루프가 깨어나 perform 콜백에서 큐를 비운다 — **콜백과 같은 스레드에서 직렬 실행되므로 상태 테이블에 락 없이 접근할 수 있다** |

> ⛔ **콜백 안에서 절대 하지 않는 것** (컴파일러가 막아주지 못하므로 리뷰 체크리스트로 남긴다)
> `Mutex`/`RwLock` 획득 · 힙 할당(`Vec::new`, `String`, `format!`) · 파일·네트워크 I/O · AX 트리 순회 · `println!`/동기 로깅 · 채널 `send` 의 블로킹 변형 · Tauri `emit`.
> 필요한 산출물은 고정 크기 배열(`SmallVec` 상당의 인라인 버퍼)에 담아 반환한다.

### 2.3 콜백 진입 순서 — ⭐ F-10 게이트를 읽는 자리

콜백은 **아래 순서를 반드시 지킨다.** 이 순서가 곧 `key-remapping-engine.md` §3-f("계층 0 문지기")와 §3-b(계층 1~5)의 구현이다.

```
0-a. 자기 합성 이벤트 마커 확인      → 마커면 즉시 통과 (무한 루프 방지, §5 엣지 12)
0-b. 탭 비활성화 이벤트 타입 확인    → Disabled 처리 후 통과 (§3-a)
0-c. ⭐ 앱별 비활성화 게이트 (F-10)  → 비활성이면 상태 리셋 후 통과 (§3-f, 계층 0)
0-d. Secure Input 확인               → true 면 FSM 전부 Idle 로 리셋 후 통과 (§5 엣지 6)
─────────── 여기부터 ultrakey-core 의 순수 함수 ───────────
1.   Seek 세션 활성                  (계층 1, M3)
2.   hyper/meh/bleh modifier 상태    (계층 2, F-05)
3.   Preset 조합                     (계층 3, M2)
4.   단순 리매핑                     (계층 4, M2)
5.   통과                            (계층 5)
```

**게이트를 읽는 위치를 M1 에서 확정하는 이유**: 명세 README 가 "F-07 → F-10 의 역방향 의존"으로 지목한 지점이 정확히 이 0-c 다. 이 자리를 나중에 만들면, 계층 1~5 가 이미 각자 앞에서 게이트를 확인하는 코드를 갖게 되어 **평가 시점이 규칙마다 갈린다**. 지금 계층 0 하나로 못박는다.

게이트 인터페이스는 `ultrakey-core::gate` 가 소유한다.

```rust
/// F-07 이 매 이벤트마다 읽는 쪽. 반드시 O(1)·무할당이어야 한다.
pub trait AppGate: Send + Sync {
    fn is_remapping_disabled(&self) -> bool;
}

/// F-10 이 갱신하는 쪽. M1 은 구현만 두고, 목록 관리 UI 는 M3.
pub struct AppGateController { /* … */ }
impl AppGateController {
    pub fn set_front_app(&self, ident: Option<AppIdentity>);   // NSWorkspaceDidActivateApplication
    pub fn set_disabled_apps(&self, bundle_ids: Vec<String>);  // 설정 변경
    pub fn toggle_front_app(&self) -> bool;                    // 메뉴바 `Ignore <앱>` (M3)
}
```

판정(`bundle_id ∈ disabledApps`)은 **`set_front_app`/`set_disabled_apps` 시점에 메인 스레드가 수행**해 `AtomicBool` 에 게시한다. 콜백은 그 부울만 읽는다.

`menu-bar-and-lifecycle.md` 가 `disabledApps` 와 `enabledApps` 를 둘 다 발견하고 블랙리스트/화이트리스트 두 모드 가능성을 `(미확정)` 으로 남겼으므로, 인터페이스는 **모드 개념을 밖으로 노출하지 않는다** — 컨트롤러 내부가 어떤 모드로 판정하든 F-07 이 보는 것은 부울 하나다. 모드가 확정되면 컨트롤러만 고친다.

### 2.4 수명(lifetime) 규약

| 자원 | 소유자 | 해제 순서 |
| :--- | :--- | :--- |
| 콜백 컨텍스트(`*mut CallbackContext`) | 탭 스레드. `Box::into_raw` 로 생성 | 런루프 정지 → 소스 제거 → `CFMachPortInvalidate` → 참조 해제 → **마지막에** `Box::from_raw` 로 드롭 |
| `CFMachPort`(탭) · `CFRunLoopSource` | 탭 스레드 | 위와 동일 순서 |
| 경로 B(IOHID `UserKeyMapping`) | 프로세스 **밖**(커널 HID 층) | ⭐ 프로세스가 죽어도 남는다. 정상 종료 시 명시 정리, 비정상 종료 대비로 **시작 시 잔존 매핑 감지·재조정**(§3-a2) |
| 경로 C(HID 잠금 상태) | 없음 | 정리 대상이 아니다 — 마지막 값이 곧 "사용자가 caps lock 을 켜둔 상태"와 구분되지 않는다(§3-a2) |

콜백 컨텍스트는 **탭 스레드 핸들을 절대 들지 않는다.** 들면 참조 순환이 되어 스레드가 자기 자신을 조인하는 교착이 생긴다. 컨텍스트가 드는 것은 `Arc<SharedState>`(원자값 + `ArcSwap` 뿐)와 상태 테이블의 배타 소유권뿐이다.

---

## 3. 리매핑 경로 3종의 배치

| 경로 | 크레이트 위치 | M1 구현 범위 |
| :--- | :--- | :--- |
| **A — `CGEventTap` 이벤트 합성** | `ultrakey-platform::event_tap` + `ultrakey-engine::tap` + `ultrakey-core::arbitration` | **전량.** M1 의 본체 |
| **B — IOHID 커널 매핑** | `ultrakey-platform::hid_mapping` + `ultrakey-engine::path_b` | **인프라 전량**(설치·조회·정리·잔존 감지·핫플러그 재적용). ⭐ M1 에는 경로 B 로 배정된 **규칙이 아직 없다** — 규칙 배정은 F-08(M2) 소관(§3-d) |
| **C — HID 잠금 상태** | `ultrakey-platform::hid_lock` | **전량**(읽기/쓰기 API). 이를 쓰는 규칙(`Double tap shift = caps lock` 류)은 F-08(M2) |

### ⭐ 명세 §7 판정을 뒤집는 결정 1 — 경로 B 의 1차 경로

`key-remapping-engine.md` §7 은 "**FFI 직접 호출을 1차로 채택**하되 `hidutil` 서브프로세스를 진단·폴백으로" 를 권장했다. **이 위임은 이를 뒤집어 `hidutil` 서브프로세스를 1차로 채택한다.**

- **근거 1 — 시그니처를 검증할 수 없다.** `IOHIDEventSystemClientCreateSimpleClient` 는 공개 헤더에 없는 심볼이다. `IOHIDServiceClientSetProperty` 도 마찬가지다. `platform-constraints.md` P6 이 `MultitouchSupport` 에 대해 남긴 경고 — "**시그니처를 추측으로 쓰지 말 것**" — 이 그대로 적용된다. 잘못된 시그니처의 `extern "C"` 호출은 컴파일도 통과하고 테스트도 통과하다가 임의 시점에 UB 를 낸다.
- **근거 2 — 명세 자신이 어느 쪽이 1차인지 모른다.** §9 #15 가 "원본이 실제로 어느 쪽을 1차로 쓰는가"를 `(미확정)` 으로 남겼다. 즉 §7 의 "FFI 1차" 는 실측이 아니라 "에러 처리·테스트가 더 결정론적"이라는 설계 선호였다. 그 선호는 **시그니처가 확정된 경우에만** 성립한다.
- **근거 3 — M1 에서 경로 B 규칙이 0 개다.** 지금 이 선택의 비용이 가장 싸다. FFI 시그니처를 오픈소스 구현체로 대조 검증한 뒤 M2 에서 바꿔도 인터페이스(`HidMappingBackend` 트레이트)는 그대로다.
- **기각한 대안** — ① FFI 를 추측 시그니처로 지금 구현: 위 근거 1. ② 경로 B 자체를 M2 로 미룸: 정리(cleanup)·잔존 감지는 **앱이 처음 출하되는 순간부터** 있어야 한다. M2 에서 처음 켜면 그 전 버전이 남긴 매핑을 아무도 치우지 않는다.

→ `key-remapping-engine.md` §7·§9 에 이 반전을 반영했다.

### ⭐ 명세 §6 API 목록을 벗어나는 결정 2 — 키보드 핫플러그 감지 수단

명세 §6 은 핫플러그 감지에 `IOHIDManagerRegisterDeviceMatchingCallback` / `RemovalCallback` / `SetDeviceMatching` / `ScheduleWithRunLoop` 를 나열한다. **이 위임은 대신 IOKit 의 `IOServiceAddMatchingNotification`(`kIOMatchedNotification` / `kIOTerminatedNotification`)을 쓴다.**

- **근거** — `IOHIDManager` 계열은 감지만 하려 해도 `IOHIDManagerOpen` 이 필요하고, 그것이 곧 **Input Monitoring(TCC) 권한 요구**다. 명세 자신이 확정한 사실은 "원본은 Input Monitoring 을 명시적으로 확인하지 않고 `IOHIDManagerOpen` 실패를 재시도로 흡수한다"(§6, F-11 §3.1)이다. 즉 **핫플러그 감지를 위해 권한 하나를 더 요구하는 구조가 된다.** `IOServiceAddMatchingNotification` 은 TCC 권한을 요구하지 않고 같은 정보(키보드 HID 장치의 등장·소멸)를 준다.
- **바뀌지 않는 것** — 감지 후의 동작(경로 B 재적용, `keyboardConnectionDelay` 지연, 디바운스)은 명세 그대로다. 바뀌는 것은 감지 수단 하나뿐이다.
- **기각한 대안** — `IOHIDManagerOpen` 을 재시도 루프로 흡수하며 명세대로 구현: 권한이 끝내 없으면 핫플러그 감지가 영영 동작하지 않고, 그 사실이 사용자에게 보이지 않는다.

→ `key-remapping-engine.md` §6·§9 에 반영했다.

---

## 4. 미확정 값에 대해 이 구현이 고른 기본값

명세가 `(미확정)` 으로 남긴 타이밍 값들이다. **실측이 아니라 이 구현의 설계 판단이며, 전부 설정으로 노출되어 나중에 바꿀 수 있다.** 근거가 실측인 값과 섞이지 않게 여기 모아 둔다.

| 값 | 채택값 | 명세의 상태 | 고른 근거 |
| :--- | ---: | :--- | :--- |
| 워치독 폴링 주기 | 1000 ms | §9 #7 `(미확정)` | 탭이 죽은 뒤 사용자가 "안 되네"라고 느끼기까지의 시간. 1초면 대개 첫 키 입력 전에 복구된다. `CGEventTapIsEnabled` 는 mach port 조회라 1초 주기 비용이 무시할 만하다 |
| 깨어남 후 재확인 지연(`restartOnWakeDelay`) | 2000 ms | §3-a `(미확정)` | 깨어난 직후는 커널 HID·WindowServer 가 아직 안정화 전이다. 즉시 재생성하면 그 재생성이 실패한다 |
| 세션 활성화 후 지연 | 1000 ms | §3-a `(미확정)` | 위와 같은 이유. 잠금 해제는 절전 복귀보다 가볍다 |
| 키보드 연결 후 지연(`keyboardConnectionDelay`) | 1500 ms | §3-a `(미확정)` | 장치 열거가 끝나기 전에 경로 B 를 재적용하면 새 장치를 놓친다 |
| 재시작 디바운스 임계값 | 5000 ms | §9 #7 `(미확정)` | 절전→잠금해제→세션전환이 연달아 오는 흔한 시퀀스를 한 번으로 합치기에 충분하고, 진짜 두 번째 사건을 삼킬 만큼 길지는 않다 |
| 탭 재활성화 재시도 상한 | 5 회 | §5 #17 `(미확정)` | 초과 시 탭 재생성으로, 재생성도 3회 실패하면 프로세스 재실행 신호로 에스컬레이션 |
| Double tap 최대 간격 | 300 ms | §4 `(추정)` — 명세가 이미 300ms 를 근거 없는 추정치로 표기 | 명세의 추정치를 그대로 쓴다. **M1 에는 이 값을 쓰는 규칙이 없다**(F-08/M2) |
| `Quick press duration` 기본값 | 1000 ms | **실측 확정**(§4, AX 트리: 최소 250·최대 2000·현재 1000) | 추정 아님 |
| 권한 폴링 주기 | 온보딩 중 500 ms / 배경 5000 ms | F-11 §9 #8 `(미확정)` | 온보딩 중에는 사용자가 시스템 설정에서 돌아온 직후를 기다리므로 짧게. 배경에서는 권한 회수 감지용이라 길어도 된다 |
| `IsSecureEventInputEnabled()` 확인 시점 | **매 콜백** | §9 #12 `(미확정)` | 명세가 "매 콜백 확인을 전제로 서술(오버헤드가 작다는 가정)". 이 가정을 실측하지 못했으므로 **캐시로 전환 가능한 트레이트 뒤에 두었다** — 측정 후 교체 지점이 한 곳이다 |

---

## 5. M1 에서 만들지 않은 자리

경계만 뚫어 두고 구현은 다음 마일스톤으로 넘긴 지점이다. **"나중에 여기에 들어온다"가 코드에 보이도록** 트레이트/enum variant 로 남겼다.

| 자리 | 형태 | 언제 |
| :--- | :--- | :--- |
| 계층 1 — Seek 세션 | `SharedState::seek_session_active: AtomicBool` (항상 false) | M3 / F-01 |
| 계층 3 — Preset 조합 | `RuleTable::combo_rules: Vec<ComboRule>` (항상 비어 있음) | M2 / F-08 |
| 계층 4 — 단순 리매핑 | `RuleTable::simple_remaps` (항상 비어 있음) | M2 / F-08 |
| 경로 B 규칙 배정 | `HidMappingBackend` 트레이트 + `hidutil` 구현체 | M2 / F-08 |
| 경로 C 를 쓰는 규칙 | `hid_lock::{get,set}_caps_lock_state` 만 존재 | M2 / F-08 |
| 설정 영속화("부재 = 기본값") | `ultrakey-core::settings` 의 타입만. 파일 저장 없음 | M2 / F-15 |
| 메뉴바 UI | `AppGateController` 만. `NSStatusItem` 없음 | M2·M3 / F-10 |
| 문자 출력형 리매핑 | `ultrakey-layout` 의 역방향 테이블은 완성. 이를 쓰는 규칙이 없음 | M2 / F-08 |
