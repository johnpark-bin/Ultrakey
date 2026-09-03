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
| `ultrakey-platform` | 전 기능의 플랫폼 경계 | **모든 `unsafe` FFI 가 여기에만 있다.** CGEvent/EventTap, IOKit(경로 B·C·핫플러그), Carbon HIToolbox(TIS/UCKeyTranslate), AX, NSWorkspace, Secure Input, CFRunLoop. ⭐ **F-02 추가(이슈 #30)**: `screen_capture`(`CGDisplayCreateImage`) · `image_preprocess`(CoreImage) · `vision_ocr`(Vision) · `ax_text`(AX 트리 순회) · `screen_recording`(권한). ⭐ **F-03 추가(이슈 #34)**: `overlay_window`(`ns_window()` 경유 window level · collection behavior · ⭐ 키 윈도우가 될 수 없게 만드는 표적 스위즐) · `screens`(`NSScreen` 열거 · `backingScaleFactor` · 핫플러그 알림 · 외관/축소된 모션). ⭐ **F-04 추가(이슈 #44)**: `click_synthesis`(마우스 클릭·커서 워프·디스플레이 경계) | ✅ 전량 | ✅ |
| `ultrakey-engine` | F-07 의 **인프라 부분** | 전용 스레드 런루프, 탭 생명주기 FSM, 워치독, 절전·깨어남·세션 훅, 핫플러그 재적용, 경로 B/C 관리자 | 간접 | ✅ |
| `ultrakey-layout` | F-14 (B) | 입력 소스 독립 판정 — ASCII 폴백, 정/역방향 테이블, 캐시 무효화 | 간접 | ✅ (테스트는 ❌) |
| `ultrakey-hyperkey` | F-05 | hyper·meh·bleh 규칙 정의와 비트마스크 합성. **엔진에 규칙만 등록한다** | ❌ 없음 | ❌ 불필요 |
| `ultrakey-seek` | **F-02** | Seek 텍스트 후보 검출의 **순수 로직** — 좌표 변환(§3.2.3), 후보 정규화, 두 소스 병합(§3.4), 질의 매칭(§3.6). macOS 의존은 `detect` 모듈 하나에 `#[cfg]` 로 가둬 두어 나머지는 어디서나 테스트된다 | ❌ 없음 | 부분 (`detect` 만) |
| `ultrakey-overlay` | **F-03** | Seek 오버레이 UI 의 **순수 로직** — 다중 디스플레이 좌표 변환, ⭐ **연결선의 디스플레이 경계 클리핑**(Liang–Barsky), 렌더 모델 산출, 상한 정책(§4.2), ⭐ **증분 수신 세션**(F-02 결정 S-6 의 소비자). 렌더링 계층은 `OverlayRenderer` 트레이트로 분리해 교체 가능하다(`platform-constraints.md` §4.3) | ❌ 없음 | ❌ 불필요 |
| `ultrakey-seek-session` | **F-01** | Seek **활성화·세션 상태 머신**의 순수 로직 — 활성화 경로 3종의 모드 판정, 세션 생명주기(Opening→Ready→Querying→Selected→Confirming), 세션 중 키 라우팅(물리 키코드 기준), F-04 로 넘길 `ConfirmedMatch`/`ClickExecutor`/`ClickSettings` 경계(F-04 추가, 이슈 #44). ⭐ 질의 버퍼·필터링·순환·200개 상한은 **다시 만들지 않고** `ultrakey-overlay::OverlaySession` 을 소유(compose)한다 | ❌ 없음 | ❌ 불필요 |
| `ultrakey-click` | **F-04** | Seek 클릭 실행의 **순수 로직**(이슈 #44) — 클릭 모드 7종 해석(modifier 스냅샷 → 모드), 모드별 클릭 지점 계산, 프리미티브 시퀀스 계획(클릭 수·clickState·워프 복귀·⌘C), AX 경로 판정(좁은 조건 + 실패 3분기), 화면 밖 판정. macOS 비의존 → 전부 단위 테스트 가능 | ❌ 없음 | ❌ 불필요 |
| `ultrakey-permissions` | F-11 | `AXIsProcessTrusted` 폴링, 권한 상태 머신, out-of-sync 진단, 시스템 설정 딥링크 | 간접 | ✅ |
| `ultrakey-i18n` | F-14 (A) / D4 · ⭐ **D6 추가(이슈 #39)** | 문자열 카탈로그(**`en`·`ko`·`zh`·`es`·`ja` 5종**), 로케일 결정, OS 버전별 어휘 교체, 언어 선택 UI 용 endonym. ⭐ **UI 문자열 전용이다** — 로그 문구는 이 크레이트를 타지 않고 코드 안의 영어 리터럴로 남는다(`localization-and-input-sources.md` §3.1.6). 그 분리는 `apps/ultrakey-app/tests/log_string_discipline.rs` 가 소스를 읽어 강제한다 | ❌ 없음 | ❌ 불필요 |
| `apps/ultrakey-app` | F-09·F-10 의 껍데기 | Tauri 앱. M1 에서는 권한 안내 모달 + 배선(wiring)만 | 간접 | ✅ |

### 왜 이 경계인가 — 기각한 대안

| 결정 | 근거 | 기각한 대안 |
| :--- | :--- | :--- |
| **`unsafe` 를 `ultrakey-platform` 한 크레이트에 전량 격리** | 명세가 요구하는 FFI 표면이 크다(CGEvent 14종, IOKit 3계열, Carbon 5종, AX, NSWorkspace). 흩어 두면 "이 `unsafe` 가 지키는 불변식이 무엇인가"를 검토할 지점이 파일마다 생긴다. 한곳에 모으면 **안전 래퍼의 계약을 한 파일에서 검토**할 수 있고, 나머지 크레이트 전부에 `#![forbid(unsafe_code)]` 를 걸 수 있다 | 기능 크레이트마다 필요한 FFI 를 직접 선언 — 같은 `extern "C"` 선언이 중복되고, 시그니처가 갈리면 UB 가 조용히 들어온다 |
| **판정 로직(`ultrakey-core`)을 macOS 비의존 순수 크레이트로 분리** | 이것이 Phase 7("자동화 가능한 것은 자동화한다")을 성립시키는 유일한 구조다. 중재 우선순위 표·quick press 상태 머신·비트마스크 합성은 전부 `InputEvent → Outcome` 순수 함수로 쓸 수 있고, 그러면 실제 키보드 없이 v1.20/v1.62 회귀 시나리오를 단위 테스트로 재현할 수 있다(F-07 §8) | 엔진 안에 판정을 함께 두기 — 탭이 살아 있어야만 테스트가 되고, 그러면 CI 에서 아무것도 검증할 수 없다 |
| **F-05 를 별도 크레이트로** | 규칙 제공자(rule provider) 자리를 M1 에서 하나 만들어 두면, M2 의 F-08(Presets 16종)이 `ultrakey-presets` 로 **같은 자리에 나란히** 들어온다. 형태가 이미 있으면 위임 지시가 짧아진다 | `ultrakey-core` 안의 모듈로 흡수 — F-08 이 들어올 때 core 가 비대해지고, 두 위임이 같은 크레이트를 고치게 된다 |
| **F-14 (B) 를 `ultrakey-engine` 이 아니라 별도 크레이트로** | 레이아웃 테이블 구축은 `UCKeyTranslate` 호출을 트레이트로 주입하면 **가짜 레이아웃으로 단위 테스트가 된다**. 엔진 안에 두면 그 이음매가 사라진다 | 엔진의 모듈로 — 명세가 별도 파일인데 코드만 합치면 M5 의 F-14(A) 위임이 갈 곳이 애매해진다 |
| **Tauri 를 앱 크레이트 하나에만 의존시킨다** | 엔진·판정 로직이 Tauri 를 모르면 `cargo test -p ultrakey-core` 가 webview 툴체인 없이 돈다. 또한 F-03(오버레이)이 렌더링 계층을 교체 가능하게 분리해야 한다는 `platform-constraints.md` P3 요구와도 일관된다 | 엔진이 `AppHandle` 을 직접 들고 emit — 테스트 불가, 그리고 콜백 스레드가 Tauri 상태를 만지게 되어 §2 의 동시성 규약이 깨진다 |
| **F-04 를 순수 결과 크레이트(`ultrakey-click`) + platform FFI 모듈(`click_synthesis`) + 앱 오케스트레이션(`click_executor`) 셋으로 나눈다** (이슈 #44) | F-02(`ultrakey-seek` + platform/ax_text·screen_capture)·F-03(`ultrakey-overlay` + platform/overlay_window)·F-01(`ultrakey-seek-session` + app/seek.rs) 와 **동형의 정착 패턴**이다 — 명세 1개 = 크레이트(또는 크레이트 안 모듈) 1개 규약의 실질("한 위임이 순수 로직 크레이트를 배타 소유")을 지킨다. 순수 로직을 독립 크레이트로 분리해야 다른 위임(F-01/F-02/F-03 후속)과 병렬 수정 충돌이 없다. **기각한 대안**: ① `ultrakey-seek-session` 흡수 — F-01 크레이트가 F-04 판정까지 소유하면 위임이 묶이고 seek-session 은 ClickExecutor **호출자**라 로직을 되먹이는 의존이 생긴다. ② `ultrakey-overlay` 흡수 — 좌표 기하만으로는 성립하나 오버레이는 F-03 렌더 모델이 핵심이고 클릭 모드 7종은 별개 도메인. ③ `ultrakey-core` 흡수 — 이미 설정·판정이 모인 크레이트라 더 키우지 않는다. ④ `event.rs` 에 마우스 합성 추가 — 키보드 합성에 하중이 걸린 파일이라 F-04 의 unsafe 표면을 한 파일에서 검토할 수 있도록 `click_synthesis.rs` 로 분리 |

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
| F-04 `Focus window before clicking` — `isActive` 폴링 간격 | 20 ms | §5 #9 `(미확정)` — 이슈 #44(D8) | 폴링은 사건 발생 시 조기 종료하는 **조건부 대기**라 고정 딜레이 금지(§5 #9)를 지킨다. ±1프레임 이내로 오버헤드 무시 가능 |
| 상동 — 폴링 상한 | 300 ms (20ms × 15회) | 상동 — 이슈 #44(D8) | 상한 초과 시 폴링을 멈추고 클릭을 그대로 진행(첫 클릭이 활성화에 소모되는 macOS 기본 동작으로 열화). 안전판이지 클릭의 전제 조건이 아니다 |
| F-04 `AXWindow` 조상 탐색 깊이 상한 | 10 | 상동 — 이슈 #44(J4) | 버튼→도구모음→창이 보통 2~3 단계. 브라우저 DOM 미러 같은 병적 트리 방어값 |

---

## 5. M1 에서 만들지 않은 자리

경계만 뚫어 두고 구현은 다음 마일스톤으로 넘긴 지점이다. **"나중에 여기에 들어온다"가 코드에 보이도록** 트레이트/enum variant 로 남겼다.

| 자리 | 형태 | 언제 |
| :--- | :--- | :--- |
| ~~계층 1 — Seek 세션~~ | ⭐ **해소(M3 / 이슈 #38).** `SharedState::seek_session_active` 를 `apps/ultrakey-app/src/seek.rs` 의 워커가 세션 개폐 때마다 게시하고, 중재기가 그 값으로 계층 1 을 판정해 키를 `Effect::SeekKey` 로 F-01 에 라우팅한다 | ✅ M3 |
| ~~F-04 — Seek 클릭 실행~~ | ⭐ **해소(M3 / 이슈 #44).** `ultrakey-click`(순수 판정) + `ultrakey-platform::click_synthesis`(unsafe FFI) + `apps/ultrakey-app/src/click_executor.rs`(오케스트레이션 — Focus 전환·AX press·좌표 폴백)로 F-04 경계가 채워졌다. `seek.rs` 의 `NullClickExecutor` 자리를 실 구현이 교체했다 | ✅ M3 |
| 계층 3 — Preset 조합 | `RuleTable::combo_rules: Vec<ComboRule>` (항상 비어 있음) | M2 / F-08 |
| 계층 4 — 단순 리매핑 | `RuleTable::simple_remaps` (항상 비어 있음) | M2 / F-08 |
| 경로 B 규칙 배정 | `HidMappingBackend` 트레이트 + `hidutil` 구현체 | M2 / F-08 |
| 경로 C 를 쓰는 규칙 | `hid_lock::{get,set}_caps_lock_state` 만 존재 | M2 / F-08 |
| 설정 영속화("부재 = 기본값") | `ultrakey-core::settings` 의 타입만. 파일 저장 없음 | M2 / F-15 |
| 메뉴바 UI | `AppGateController` 만. `NSStatusItem` 없음 | M2·M3 / F-10 |
| 문자 출력형 리매핑 | `ultrakey-layout` 의 역방향 테이블은 완성. 이를 쓰는 규칙이 없음 | M2 / F-08 |

---

## 6. ⭐ F-08 프리셋 중재 설계 (M2 2차 / 이슈 #15)

> 이 절은 **이 세션이 직접 설계한 것**이다. `power-user-presets.md` §3.3 이 R1~R8 로 남긴 상호작용
> 규칙을 실제 코드가 따를 수 있는 형태로 확정하고, 명세가 `(미확정)` 으로 남긴 자리에서 이 구현이
> 무엇을 골랐는지와 기각한 대안을 기록한다. 원본이 v1.20·v1.62 에서 실제로 겪은 회귀가 전부
> 이 영역이므로, 규칙을 코드에 흩어 두지 않고 여기 한 장에 모은다.

### 6.1 ⭐ 선결 문제 — caps lock 은 래칭 키다 (§5 #20 의 해법)

M2 1차 실측이 확정한 사실: **caps lock 은 누를 때만 `flagsChanged` 를 보내고 뗄 때는 보내지
않는다.** 그래서 `key-remapping-engine.md` §5 #18 의 down/up 환원이 "1번째 누름 = down,
2번째 누름 = up" 으로 해석해 hyper 가 홀드가 아니라 **토글**로 동작한다.

**프리셋 16종 중 7종이 caps lock 을 "눌린 채 유지" 조건으로 쓴다**(F-08.1·2·4·5·6·7·10).
즉 이 문제를 풀지 않으면 F-08 수용 기준의 절반이 실기기에서 성립할 수 없다. §5 #20 이
"해법은 경로 B, 배정은 F-08(M2 2차) 소관" 이라고 넘긴 자리가 여기다.

> **결정 D-1 — caps lock 모멘터리 정규화.**
> caps lock 에 의존하는 규칙이 **하나라도** 켜져 있으면, 경로 B(`hidutil` HID `UserKeyMapping`)로
> `caps lock → F18` 커널 매핑을 설치한다. 그러면 탭에는 **정상적인 down/up 쌍을 갖는 F18** 이
> 도착한다. 중재기는 진입 즉시 이 keycode 를 caps lock 으로 되돌려(`EngineConfig::caps_lock_alias`)
> 이후 모든 판정을 물리 caps lock 기준으로 수행한다.
>
> - **왜 F18 인가**: `SourceKey` 35종에 들어 있어 keycode 가 확정돼 있고(`0x4F`), 실물 키보드
>   대부분에 물리 키가 없어 사용자의 기존 입력과 충돌할 여지가 가장 작다. Karabiner 류가 같은
>   목적으로 관용적으로 쓰는 키이기도 하다.
> - **어떤 규칙이 이 조건을 켜는가**: F-08.1·F-08.2·F-08.4·F-08.5·F-08.6·F-08.7·F-08.10 중
>   하나라도 켜짐, 또는 hyper/meh/bleh 소스 키가 caps lock.
> - **경로 C 와 충돌하지 않는다**: "진짜 caps lock 토글"(F-08.8/9/10)은 키 합성이 아니라
>   `IOHIDSetModifierLockState` 로 내므로, caps lock 키가 F18 로 리매핑돼 있어도 그대로 동작한다.
> - **되돌릴 수단을 남긴다**: 설정 화면 `General` 탭 `Advanced` 섹션의 `Synthesize Caps
>   Lock Remap` 체크박스(⭐ 이슈 #77 — 메뉴바 `Advanced ▸` 에서 설정 화면으로 이동.
>   상태는 [이탈 D9](spec/README.md#-원본과-갈라지는-지점-divergence)). 켜면 경로 B 를
>   설치하지 않고 경로 A 만 쓴다 — 래칭 문제는 남지만, 다른 HID 계층
>   도구와 충돌하는 환경에서 사용자가 커널 매핑을 끌 수 있어야 한다.
>   ⭐ `menu-bar-and-lifecycle.md` §3.3 이 이 항목을 "기각(1차 릴리스 범위 밖)" 으로 판정하며
>   "필요해지면 F-08 이 재검토" 라고 남겼는데, D-1 이 커널 매핑을 제품 기본 경로로 올렸으므로
>   **재검토 결과 재현으로 뒤집는다.** 사용자 시스템 전역에 남는 변경에는 반드시 끄는 수단이
>   함께 있어야 한다는 것이 근거다.
>
> **기각한 대안**
> - ① *경로 A 에서 시간 기반으로 뗌을 추정한다* — "누른 채 유지"와 "톡 누르고 뗌"을 구분할 수
>   없다. §5 #20 이 이미 같은 이유로 기각했다.
> - ② *caps lock 프리셋을 포기하고 모멘터리 키만 지원한다* — 16종 중 7종을 버리는 것이고,
>   원본의 캡스락 그룹 전체가 사라진다. F-08 §8 수용 기준을 충족할 수 없다.
> - ③ *`Remap caps lock to:`(F-08.1)만 경로 B 로 직접 매핑한다*(caps lock → left control 등) —
>   Secure Input 구간에서도 동작한다는 장점이 있으나(§5 엣지 7), F-08.1 은 **quick press 판정에
>   종속**된다(R1). 정적 커널 매핑은 시간 조건을 표현할 수 없으므로 R1 을 구현할 수 없다.
>   따라서 caps lock 관련 매핑은 **한 종류(→F18)로 통일하고 조건 판정은 전부 경로 A** 에서 한다.

> ⭐ **D-1 보칙 — 커널 매핑의 소유권과 수명 (2026-08-30 추가, 이슈 #19 실측).**
>
> D-1 을 켜는 것만으로는 부족하다. **끄는 쪽이 반쪽이면 caps lock 이 복구 불가능하게 죽는다.**
> 실측으로 드러난 상태: `Advanced ▸ Synthesize Caps Lock Remap` 을 켜서 `caps_lock_alias` 가
> `None` 이 됐는데 커널에는 `caps lock → F18` 이 그대로 남아, **물리 caps lock 이 F18 로
> 도착하지만 그것을 caps lock 으로 되돌릴 alias 가 없다.** 어떤 규칙에도 매칭되지 않고
> 앱 안에서 되돌릴 수단도 없다 — 사용자가 보고한 "아예 캡스락 이벤트 자체가 캡처가
> 안됨"이 이 상태다. 두 규칙을 못박는다.
>
> - **B-1. 설정이 매핑을 요구하지 않게 되면 우리 매핑을 즉시 걷어낸다.** M1 은 잔존 매핑을
>   보면 "우리 것인지 남의 것인지 구분할 수 없다"며 손대지 않았고, 당시엔 그 전제가 참이었다
>   (설치하는 규칙이 0개였다). D-1 이후로는 거짓이다 — 우리가 설치하는 매핑은 정확히
>   하나이고 그 값(`0x700000039 → 0x70000006D`)이 **서명** 역할을 한다.
>   ⛔ 서명이 다른 항목은 여전히 손대지 않는다(`manual-verification.md` 부록 A #3).
>   ⛔ `hidutil` 의 `UserKeyMapping` 은 **전체 교체**이므로, 무엇을 적용하든 남의 매핑을
>   함께 실어 보내야 한다. 정상 종료 시의 정리도 "전부 비우기"가 아니라 "우리 것만 빼기"다.
> - **B-2. `Synthesize Caps Lock Remap` 토글은 저장으로 끝나지 않는다.** 이 스위치는
>   `EngineConfig::caps_lock_alias` 를 바꾸므로, 누른 즉시 메모리 정본(`AppState::presets`)과
>   엔진(`Engine::reconfigure` → 경로 B 재적용)에 반영되어야 한다. 저장만 하면 재실행
>   전까지 스위치가 아무 효과도 없고, 위 상태로 곧장 들어간다.
>
> ⭐ **D-1 보칙 2 — 설치는 확인돼야 설치다 (2026-09-03 추가, 이슈 #110).**
>
> - **B-3. 되읽기로 확인한다.** `hidutil --set` 은 매칭 사전이 아무 서비스도 못 잡아도 exit 0
>   이다. MacBook Air 내장 키보드는 VID/PID 프로퍼티가 없어 열거에서 빠졌고(`count=0`),
>   원장만 "설치됨" 이던 상태가 caps lock 단독 입력의 control 고착으로 드러났다. 이제
>   `PathBManager::write_device` 는 `--set` 직후 같은 사전으로 되읽어 우리 항목이 실렸는지
>   확인하고(`NotApplied`), 전체 재조정의 결과를 `d1_confirmed` 로 게시한다(0대 = 미확인).
>   앱은 `caps_lock_alias.is_some() && !d1_confirmed` 일 때 "caps lock 커널 매핑 미확인" 을
>   보인다. 되읽기는 "실렸는가"만 답한다(S-7 — 동작 확인은 여전히 수동 검증).
> - **B-4. 중재기는 이벤트 모양으로 방어한다.** `caps_lock_alias` 가 켜져 있는데 keycode `0x39`
>   의 `flagsChanged` 가 오면 그 키보드에 D-1 이 없다는 것이 이벤트로 증명된다(정상이면 F18
>   KeyDown/KeyUp) → 그 이벤트를 down+up 탭으로 판정한다(`Arbiter::d1_bypassed`,
>   `key-remapping-engine.md` §5 #26). 전역 플래그로 게이트하지 않는 이유: 혼합 상태(외장
>   정상·내장 미설치)에서 외장의 홀드까지 죽이고, 핫플러그 직후 stale 하다.

⚠️ **검증 기기에서는 이 경로를 실측할 수 없다.** Karabiner-Elements 가 `caps_lock ↔ left_control`
을 **경로 B 보다도 아래**(가상 HID 장치)에서 맞바꾸고 있어, 우리의 `hidutil` 매핑이 볼 caps lock
자체가 도착하지 않는다. `manual-verification.md` 의 사전 확인 절이 이 조건을 먼저 확인하게 한다.

### 6.2 정본 상태 확장 — 어떤 키가 추적되는가

M1 은 `KeyStateTable` 의 quick press 슬롯을 **`modifier_rules` 의 소스 키로만** 구성했다.
M2 는 여기에 **프리셋이 FSM 을 필요로 하는 키**를 합집합으로 더한다.

| 슬롯에 등록되는 키 | 왜 |
| :--- | :--- |
| hyper/meh/bleh 소스 키 | 계층 2 hold 판정(M1 그대로) |
| caps lock (F-08.1 또는 F-08.2 가 켜짐) | quick press ↔ hold 리매핑 배타 판정(R1) |
| left shift / right shift (F-08.8 또는 F-08.11 이 켜짐) | quick press 문자 출력, double tap 판정 |

`MAX_TRACKED_KEYS = 8` 은 최악의 경우(hyper·meh·bleh 3 + caps lock + 좌우 shift = 6)에도 여유가 있다.

⭐ **M1 이 뚫어만 두고 배선하지 않은 구멍을 여기서 막는다.** `QuickPressState::on_other_key_down`
(§3-c 표 3행, **v1.62 예방의 핵심**)은 M1 에서 **한 번도 호출되지 않았다** — `has_quick_press_action`
이 항상 `false` 여서 `PendingDown` 상태 자체가 생기지 않았기 때문에 드러나지 않았을 뿐이다.
M2 는 `has_quick_press_action` 을 실제로 채우므로, **추적 대상이 아닌 키의 keyDown 이 도착할 때마다
모든 슬롯에 `on_other_key_down` 을 돌리는 배선**을 반드시 함께 넣는다.

### 6.3 프리셋 16종의 경로·계층 배정

| ID | 프리셋 | 계층 | 경로 | 비고 |
| :--- | :--- | :---: | :---: | :--- |
| F-08.1 | `Remap caps lock to:` | 2/3 (FSM hold) | A | R1 로 F-08.2 와 배타. `nothing (disable it)` 은 소비만 하고 아무것도 내지 않는다 |
| F-08.2 | `Quick press caps lock to execute:` | 2/3 (FSM quick press) | A (+C) | 출력이 `caps lock` 이면 경로 C 로 실제 잠금 토글. `Seek` 는 M3 — 지금은 효과만 발행하고 로그 |
| F-08.3 | `Quick press duration` | — | — | `Timings::quick_press_duration_ms` |
| F-08.4 | `Caps lock + space = enter` | 3 | A | |
| F-08.5 | `Caps lock + W A S D` | 3 | A | |
| F-08.6 | `Caps lock + [H J K L / I J K L]` | 3 | A | |
| F-08.7 | `Caps lock + home row` | 3 | A (+유니코드) | symbol row 는 문자 출력, function row 는 키코드 출력 |
| F-08.8 | `Double tap shift = caps lock` | 2/3 (FSM double tap) | C | |
| F-08.9 | `Left shift + right shift = caps lock` | 3 | C | |
| F-08.10 | `Shift + caps lock = caps lock` | 3 | C | R3 로 F-08.2 를 무효화 |
| F-08.11 | `Quick press left or right shift …` | 2/3 (FSM quick press) | A (유니코드) | |
| F-08.12 | `Hyper + delete = forward delete` | 3 | A | **논리 hyper 신호**를 구독(R4) |
| F-08.13 | `Remap delete to forward delete` | 4 | A | |
| F-08.14 | `Shift + delete = forward delete` | 3 | A | |
| F-08.15 | `Remap paste (⌘+V) …` | 3 | A | `Hyper key` 선택 시 논리 hyper 신호 구독 |
| F-08.16 | `Home & end operate on lines` | 4 | A | |

### 6.4 ⭐ 중재 규칙표 P1~P12 — 이 구현이 확정한 것

계층 3 안에서 여러 규칙이 같은 이벤트에 반응할 수 있다. **평가 순서를 규칙 ID 로 고정**해
입력 순서에 의존하지 않게 한다(`RuleTable` 을 정렬된 상태로 만들어 넘긴다).

| # | 규칙 | 명세 근거 |
| :--- | :--- | :--- |
| **P1** | **추적 키 자신의 이벤트는 FSM 하나가 결정한다.** `HoldStart` → 계층 2 modifier 합성 + F-08.1 대상 키 down. `HoldEnd` → modifier off + 대상 키 up. `QuickPress` → F-08.2/F-08.11 액션. `DoubleTap` → F-08.8 액션. FSM 이 이 넷 중 **정확히 하나**만 방출하므로 R1 이 자동으로 성립한다 | R1 |
| **P2** | **추적 대상이 아닌 키의 keyDown 이 오면, 모든 슬롯에 `on_other_key_down` 을 돌린다.** `PendingDown` 이던 슬롯은 즉시 `HoldConfirmed` 가 되고 quick press 후보에서 빠진다 | R1·v1.62, §3-c 표 3행 |
| **P3** | **조합 판정은 정본 눌림 테이블(`is_pressed`)만 본다.** `ev.flags` 의 modifier 비트로 판정하지 않는다 — 좌/우 shift 가 같은 비트(`0x20000`)를 공유해 구분이 불가능하고, caps lock 의 `alphaShift` 는 눌림이 아니라 잠금을 뜻한다 | §5 #18·§8 |
| **P4** | **계층 2 는 계층 3 을 막지 않는다.** caps lock 이 hyper 소스이면서 동시에 조합 트리거인 구성에서, caps lock 의 hold 확정과 조합 조건 성립은 **같은 판정 패스의 같은 상태 테이블**에서 읽힌다. 계층 2 가 소비하는 것은 **소스 키 자신의 이벤트**뿐이고, 트리거 키(W 등)의 이벤트는 계층 3 에 그대로 도달한다 | R2·v1.20 |
| **P5** | **조합이 낸 출력에는 hyper 합성 flags 를 얹지 않는다.** 방출 이벤트의 flags = `ev.flags` − `active_synth_flags()` − `alphaShift`. 근거: `Caps lock + W = ▲` 의 의도는 방향키이지 `⌃⌥⌘⇧▲` 가 아니다. `Apply hyper to arrows` 체크박스의 존재가 "기본은 안 얹는다"를 방증한다 — 그 체크박스는 **표시 조건이 `(미확정)`** 이므로 구현하지 않고 §9 로 승계한다 | R2, §3.2 조건부 항목 |
| **P6** | **조합의 트리거가 된 추적 키는 `Suppressed` 로 전이한다.** 그 눌림에서는 quick press·double tap 을 내지 않는다. F-08.10(shift+caps lock)이 F-08.2 를 무효화하는 것이 이 규칙의 구현이다 | R3·v1.62 |
| **P7** | **F-08.9(좌우 shift 동시)가 F-08.8(double tap shift)보다 우선한다.** 발화 시 좌·우 shift FSM 을 **둘 다** `Suppressed` 로 만들어, "양쪽 shift 를 빠르게 두 번" 제스처가 토글을 두 번 내 서로 상쇄되는 §5 엣지 1 을 원천 차단한다. **기각한 대안 — 시간 디바운스**: 임계값이 또 하나의 `(미확정)` 상수가 되고, 억제로 이미 해소되는 문제에 타이머를 더할 이유가 없다 | §5 엣지 1, §9 #8 |
| **P8** | **F-08.12·F-08.15(`Hyper key`)는 논리 hyper 신호를 구독한다.** 소스 키 종류를 보지 않고 `ModifierKind::Hyper` 인 슬롯이 `HoldConfirmed` 인지만 본다. globe 든 caps lock 이든 동일하게 발화한다 | R4·v1.60 |
| **P9** | **문자 출력형(F-08.7 symbol row, F-08.11 괄호)은 유니코드 문자로 낸다.** `CGEventKeyboardSetUnicodeString` 으로 목표 문자를 직접 얹어 레이아웃과 무관하게 같은 글자가 나가게 한다. 반대로 **입력 판정은 언제나 물리 키코드**다 — 두 원칙이 프리셋마다 다르게 적용되는 것이 §5 엣지 5 가 경고한 지점이다 | R5·v1.51/v1.52 |
| **P10** | **forward delete 3종(F-08.12/13/14)은 배타적이지 않다.** 셋 다 켜지면 F-08.13 만으로 이미 delete 가 항상 forward delete 이므로 나머지 둘은 관측 가능한 차이를 만들지 않는다. 이것을 막지 않고 **사실로 문서화**한다 — 진짜 backspace 를 낼 수단이 사라지는 것은 F-08.13 단독의 결과다 | R6, §5 엣지 10 |
| **P11** | **caps lock 토글 3종(F-08.8/9/10)의 출력은 전부 경로 C** (`IOHIDSetModifierLockState`)다. 키 합성이 아니다 — D-1 로 caps lock 키가 F18 로 리매핑돼 있어도 이 경로는 영향받지 않는다 | §3.2 F-08.8/9/10, §6 |
| **P12** | **설정 충돌은 대화형 배타 선택으로 해소한다**(자동 우선순위가 아니다). §6.5 | R8 |

### 6.5 충돌 감지 대화상자 3종

| # | 내부 ID | 트리거 | 배타 대상 |
| :--- | :--- | :--- | :--- |
| 1 | `CapsLockAlreadyRemapped` | `Remap caps lock to:` 를 켜려는데 hyper/meh/bleh 소스가 이미 caps lock (또는 그 반대) | 상대 설정을 끈다 |
| 2 | `CapsLockArrows` | `Caps lock + W A S D` 와 `Caps lock + [H J K L]` 중 꺼진 쪽을 켜려 함 | 켜져 있던 쪽을 끈다 |
| 3 | `CapsLockHomeRow` | `Caps lock + home row` 와 방향키 프리셋(F-08.5·F-08.6) 중 한쪽을 켜려 함 | 상대를 끈다 |

**#3 의 근거**: home row 집합(`A S D F G H J K L ; '`)은 WASD 의 `A S D` 와 HJKL 의 `H J K L` 을
**실제로 포함**한다. 같은 물리 키가 두 규칙의 트리거가 되므로 조용히 한쪽만 이기면 사용자는
"왜 안 먹지"를 스스로 디버깅해야 한다 — 원본이 전용 대화상자를 둔 이유가 이것이다.

⭐ **F-08.4(`Caps lock + space`)는 어느 그룹과도 충돌하지 않는다** — `space` 는 위 세 집합 어디에도
없다. 그래서 배타 대상에서 뺀다.

⭐ **caps lock 조합 프리셋(F-08.4~7)과 "hyper 소스 = caps lock" 은 충돌로 다루지 않는다.**
F-08 §8 이 "두 구성이 동시에 성립해야 한다"를 **수용 기준으로 명시**했기 때문이다(R2/v1.20 회귀
방지). 대화상자 #1 의 배타 대상은 `Remap caps lock to:`(F-08.1) 쪽뿐이다.

⚠️ **버튼 구성·문구는 원본을 관찰하지 못했다**(F-08 §9 #6). 이 구현은 우리 문구로 2버튼
(`계속`·`취소`)을 쓰고, 원본 문구를 그대로 옮기지 않는다. 정확한 트리거 쌍이 원본과 같은지는
`(미확정)` 으로 남긴다.

### 6.6 명세가 `(미확정)` 으로 남긴 자리에서 이 구현이 고른 값

| 항목 | 채택값 | 근거 | 명세 상태 |
| :--- | :--- | :--- | :--- |
| `Caps lock + home row` 의 `A` 외 매핑 | 홈로우 11키(`A S D F G H J K L ; '`)를 숫자행 시프트 기호에 **순서대로** — `! @ # $ % ^ & * ( ) _` / 함수행에 순서대로 — `F1`…`F11` | §3.2 F-08.7 이 확정한 "홈로우를 다른 행에 순서대로 매핑" 일반 규칙 + 실측 예시 두 개(`A = !`, `A = F1`)를 그대로 연장한 것이다. 지어낸 것이 아니라 **두 실측점을 잇는 유일한 자연스러운 보간**이다 | F-08 §9 #2 |
| `Home & end operate on lines` 의 출력 | `Home → ⌘←`, `End → ⌘→` | macOS 에서 `Home`/`End` 는 문서 처음/끝, `⌘←`/`⌘→` 는 **줄** 처음/끝이다. 라벨의 "operate on lines" 가 정확히 이 차이를 가리킨다 | F-08 §9 #7 `(추정)` |
| `Quick press duration` 슬라이더 step | 50 ms | 250~2000 범위를 35칸으로 나눈다. 실측된 것은 범위와 현재값(1000)뿐이고 step 은 미확정이므로, **기본값 1000 이 눈금에 정확히 떨어지는** 값 중 가장 세밀한 것을 골랐다 | F-08 §9 #1 |
| double tap 최대 간격 | 300 ms | M1 이 이미 고른 값을 그대로 쓴다(§4) | §4 `(추정)` |
| Colemak/Dvorak 변형 | **구현하지 않는다** | WASD·HJKL 은 §5 엣지 5 가 확정했듯 **물리 위치 기반**이다. 레이아웃 변형 키(`wasdArrowColemak` 등)가 실행 파일에 있다는 사실만으로 "자동 적용"을 단정할 수 없고(팝업 부재로부터의 해석), 물리 위치 기반 판정과 정면으로 어긋난다 | F-08 §9 #3 |
| `Apply hyper to arrows` | **구현하지 않는다** | 표시 조건 자체가 `(미확정)` 이다. P5 가 정한 기본 동작(hyper flags 를 얹지 않음)이 이 체크박스가 꺼진 상태에 해당한다 | F-08 §9 #4 |
| Windows 키보드 리매핑 | **구현하지 않는다** | 라벨·표시 조건 둘 다 `(미확정)` | F-08 §9 #5 |
| 경로 C 토글을 콜백 안에서 직접 실행 | **한다** | `IOHIDSetModifierLockState` 는 mach 메시지 한 번이라 마이크로초 단위이고, **사용자가 실제로 제스처를 했을 때만** 실행된다(매 이벤트가 아니다). 탭 타임아웃 예산 대비 무시할 만하다. **기각한 대안 — 워커 스레드로 큐잉**: 무한 큐 `send` 가 오히려 콜백 안에서 할당을 유발할 수 있어(§2.2 금지 목록) 얻는 것보다 잃는 것이 크다. 실측에서 타임아웃이 관측되면 이 자리 하나만 바꾸면 된다 | 신규 |

### 6.7 F-10 이 M2 2차에서 채우는 것

| 항목 | 결정 |
| :--- | :--- |
| 메뉴 구성 | `Ignore <앱>` · (구분선) · `Settings…` · `About` · `Advanced ▸ (Relaunch)` · `Quit Ultrakey`. ⭐ 이슈 #77 — `Synthesize Caps Lock Remap` 은 설정 화면 `General` 탭 `Advanced` 섹션으로 이동했다(트레이 제거, [이탈 D9](spec/README.md#-원본과-갈라지는-지점-divergence)). `Purchase`(F-12)·`Check for Updates…`(F-13)는 **범위 밖이라 넣지 않는다** — 자리만 비운다 |
| `Advanced` 하위 | 로깅 뷰어 6종은 명세대로 **기각**. `Relaunch After Wake`/`Delay …`/`Relaunch on Keyboard Connected` 는 이미 엔진이 자동으로 하는 일이라 사용자 노출 스위치를 두지 않는다 — 수동 `Relaunch` 하나만 자가 진단 수단으로 남긴다(§3.3 판단 그대로) |
| `unauthorizedMenu` | 권한 없음 상태에서 메뉴 전체를 2항목(`상태 안내`(비활성) · `권한 허용…`)으로 교체 |
| `Launch on login` | macOS 13+ `SMAppService.mainApp`. macOS 12 는 `~/Library/LaunchAgents` plist 폴백. **원본의 헬퍼 앱(`SuperkeyLauncher.app`) + `SMLoginItemSetEnabled` 패턴은 기각**(§7 기각한 대안 3 이 이미 그 방향을 제시했다) — 헬퍼 번들을 하나 더 서명·배포·핑퐁 관리해야 하는데 얻는 것이 없다. 등록 실패는 **상한 있는 재시도**(0.2s 간격 5회)로 흡수하고 실패를 사용자에게 알린다 |
| 앱별 비활성화 | M1 의 `AppGateController` 를 그대로 쓴다. `NSWorkspaceDidActivateApplicationNotification` 은 이미 `workspace.rs` 가 `SystemEvent::FrontAppChanged` 로 올려주고 있다 — **새 인터페이스를 만들지 않는다.** 목록은 `general.disabledApps` 로 영속화하고, 콜백 임계 경로는 `AtomicBool` 하나만 읽는다(§2.2 불변 조건 유지) |

---

## 7. ⭐ F-17 키보드별 설정 — 경로 B 의 디바이스 한정 재설계 (이슈 #28)

`docs/spec/per-device-settings.md`(F-17)가 요구하는 것을 이 구현이 어떻게 배치했는가.
사실 토대는 전부 `docs/research/per-device-hid-spike.md` 의 실측이다.

### 7.1 ⭐ 결정 D-17-1 — 모든 경로 B 쓰기를 디바이스 한정으로 바꾼다. **D-1 도 예외가 아니다.**

> **결정.** `HidMappingBackend` 의 모든 메서드가 `DeviceMatch { vendor_id, product_id }` 를
> **필수 인자로** 받는다. 그 결과 두 가지가 **타입 수준에서 표현 불가능**해진다 —
> ① 매칭 없는 전역 `--set`, ② VID 단독 매칭(S-3 의 함정).
> 한 디바이스의 최종 배열은 **D-1 + 기능 1 + 기능 2 를 합성한 하나**이고, 그것을
> `--matching {VID,PID} --set` **한 번**으로 쓴다(명세 §3.6 규칙 4).

**근거.** S-6(실측, 스파이크 중 비의도 재현 2회) — 매칭 없는 `--set` 은 per-device 배열까지
포함해 **전부** 갈아치운다. S-1(실측) — `--matching {VID,PID}` 쓰기는 그 디바이스에만 닿는다.
따라서 **전역 기록자와 디바이스별 기록자가 공존할 수 있는 구성은 존재하지 않는다.**
F-17 이 디바이스 한정을 요구하는 이상 D-1 도 디바이스 한정이어야 한다.

⭐ **D-1 은 붙어 있는 *모든* 키보드에 설치한다.** D-1 이 "필요"한 조건은 전역이지만
(caps lock 의존 규칙이 하나라도 켜짐), 그 **설치**는 디바이스마다 이루어져야 한다 —
경로 A(`CGEventTap`)는 세션 전체의 병합 스트림을 받아 어느 키보드가 caps lock 을 보냈는지
구분하지 못하므로(F-07 §1), 어느 한 키보드에 D-1 이 빠지면 그 키보드의 caps lock 이
정규화되지 않은 채 도착해 중재기의 caps-lock 의존 규칙이 오동작한다.

**기각한 대안**

1. **D-1 은 전역으로 두고 F-17 은 D-1 이 닿지 않는 디바이스에만 쓴다** — 기각.
   전역 쓰기는 정의상 모든 디바이스에 닿는다(S-6). 그런 디바이스는 존재하지 않는다.
2. **D-1 을 전역으로 두되 F-17 쓰기 뒤에 항상 다시 설치한다(마지막 기록자 우선)** — 기각.
   D-1 재설치는 기동·`reconfigure`·핫플러그 세 경로에서 우리가 통제하지 못하는 시점에
   발생한다. 정확성 속성을 경합으로 바꾸는 설계이며, ⭐ **스파이크를 두 번 오염시킨 것이
   정확히 이 동작이다**(스파이크 §6, 부록 A).
3. **D-1 을 `IOHIDSystem`(VID `0x5ac` / PID `0x0`)에 명시적으로 쓰고 F-17 은 실제
   디바이스에 쓴다** — 기각. ⓐ `IOHIDSystem` 배열이 per-device 배열과 *합성*되는지
   *가리는지* 측정된 바 없고 S-6 의 관측은 오히려 후자를 시사한다. ⓑ 문서화되지 않은
   의사 디바이스를 제품 의존성으로 만든다. ⓒ 성립하더라도 D-1 은 여전히 사실상 전역이라,
   사용자가 특정 키보드에서만 D-1 을 끄는 것이 원리적으로 불가능해진다.
4. **caps lock 키가 실제로 달린 키보드에만 D-1 을 설치한다** — 기각. 위에서 밝힌 대로
   경로 A 가 디바이스를 구분하지 못한다. 어느 키보드에 물리 caps lock 이 있는지는
   HID 서술자로 알 수 있을지 모르나, 그 정보가 맞더라도 얻는 것이 없다(설치 비용이
   디바이스당 배열 항목 하나다).
5. **F-17 이 엔진과 별개의 기록자를 갖는다** — 기각. 한 배열에 기록자 둘 = 명세 §3.6
   규칙 1 위반이고 S-6 이 그 결과를 이미 보여줬다. **합성기 하나, 기록자 하나.**

### 7.2 ⭐ 결정 D-17-2 — 소유권 판정은 "디바이스별 마지막으로 쓴 배열" 원장으로 한다

> **결정.** 설정 저장소에 `perDevice._managed` 를 두고 값은
> `{ "<vid>:<pid>": [{"src":…,"dst":…}, …] }` — 그 디바이스에 **우리가 마지막으로 쓴 배열**이다.
> "우리 것" 은 이 원장과의 비교로 판정하고, 원장에 없는 항목은 **전부 남의 것으로 보존**한다.

**근거.** PR #23 이 확립한 소유권 모델("우리가 설치한 매핑만 제거한다")은 우리 출력이
**고정 상수 한 개**(`caps lock → F18`)였기 때문에 *서명 비교*로 성립했다. F-17 부터 출력은
**사용자가 정의**한다 — 사용자가 기능 1 의 행을 지우면 커널의 잔존 매핑은 더 이상 "우리가
지금 계산하는 집합" 에 없으므로, 서명 방식은 그것을 영원히 남의 것으로 오판하고 매핑이
지워지지 않는다. 즉 **서명은 직전에 우리가 실제로 쓴 집합이어야 하고, 그것은 기록해야만
알 수 있다.** 명세 §3.6 규칙 6 이 요구한 원장(디바이스 목록)을 **디바이스 → 배열**로
강화한 것이며, PR #23 의 모델을 깨는 것이 아니라 **일반화**한다(고정 상수 한 개짜리
원장이 곧 PR #23 이다).

**크래시 안전 — 2단계 영속화.**

```
① claimed = ledger[dev] ∪ composed_new ;  persist(ledger[dev] = claimed)   # 쓰기 전: 상위집합
② current = read(dev) ; foreign = current − claimed
   write(dev, foreign ++ composed_new)                                      # 커널
③ persist(ledger[dev] = composed_new)                                      # 쓰기 후: 정확집합
```

①이 있어 **어느 시점에 죽어도 원장은 커널에 있을 수 있는 우리 것의 상위집합**이다 —
우리 매핑이 영구히 새는 일(이슈 #19 증상 B: caps lock 이 복구 불가능하게 죽는 상태)이 없다.
③이 있어 정상 경로에서 원장은 정확하고, 남의 매핑을 잘못 지우지 않는다(PR #23 보존).
`composed_new` 와 `foreign` 이 모두 비면 빈 배열을 쓰고 그 디바이스를 원장에서 제거한다.

**기각한 대안**
- **"관리 중인 디바이스의 모든 매핑은 우리 것"** — 기각. PR #23 위반. 사용자가 같은
  디바이스에 직접 걸어 둔 매핑을 파괴한다.
- **"소스 키가 우리 어휘(35종)에 있으면 우리 것"** — 기각. 사용자의 `caps lock → escape`
  를 삼킨다. 우리 어휘가 넓을수록 더 많이 삼킨다.

### 7.3 결정 D-17-3 — 읽기 집계는 서비스 전체의 **합집합**, 불일치 시 `partial`

S-2(실측) — 물리 디바이스 1개가 IOHID 서비스 여러 개다(이 기기에서 3개). `--matching --get`
은 서비스 수만큼 행을 준다(명세 §3.6 규칙 7). 소유권 뺄셈과 남의 매핑 보존 양쪽에서
"어딘가에 존재하는 것" 을 전부 봐야 하므로 **합집합**이 보수적으로 옳다 — 한 서비스에만
있는 매핑도 존재하는 매핑이고 보존 대상이다. 서비스마다 값이 다르면(쓰기가 디바이스 분리와
경합한 부분 실패, 명세 §5 항목 7) `partial = true` 로 올리고, 다음 재조정이 흡수한다.

### 7.4 결정 D-17-4 — 값 검증은 우리가 한다 (S-7)

S-7(실측, 통제 실험) — IOHID 는 값을 **전혀** 검증하지 않는다. 엉터리 값도 조용히 저장된다.
그래서 **쓰기 직전 게이트**를 우리가 둔다. `apply` 진입 즉시 검증하고, 실패하면 **쓰지 않고**
타입화된 오류로 거부한다(조용히 넘기지 않는다).

| # | 규칙 | 막는 것 |
| :--- | :--- | :--- |
| 1 | `DeviceMatch` 는 `vendor_id`·`product_id` 둘 다 필수. ⭐ 이슈 #110 — 매칭 사전은 여기에 `PrimaryUsagePage:1`/`PrimaryUsage:6` 을 항상 더한 **4키 고정**이다(`matching_json`) | VID 단독 매칭(S-3) — 타입 수준에서 불가능. usage 두 키가 `IOHIDSystem`(usage 65280/23)을 PID 와 무관하게 배제한다 |
| 2 | ⛔ `0x5ac:0x0`(IOHIDSystem) **정확 일치**인 쓰기를 거부(`perdevice::is_iohidsystem`). ⭐ 이슈 #110 — 옛 형태 `product_id == 0` 전체 거부는 VID/PID 프로퍼티가 없어 `0:0` 으로 열거되는 내장 키보드(S-10)까지 막았다 | `IOHIDSystem`(`0x5ac:0x0`)에 대한 하드 가드 — S-3 의 실제 함정 |
| 3 | 모든 `src`/`dst` 가 닫힌 어휘에서 나온 값: `page ∈ {0x07, 0x0C, 0xFF, 0xFF01}`, `usage ≤ 0xFFFF` | 스파이크의 `0x9999999999` 류가 저장되는 사고 |
| 4 | 배열 안 **중복 `src` 금지** | 규칙 5 중재가 보장하지만 마지막에 assert 한다 |
| 5 | 배열 길이 ≤ 64 | 폭주 방어 |

⛔ **되읽기 성공을 동작 확인으로 쓰지 않는다**(명세 §3.6 규칙 9). 자동 테스트는 "쓴 값이
되읽힌다" 까지만 확인할 수 있고, "실제로 그 키를 눌렀을 때 의도한 동작이 난다" 는
`manual-verification.md` 의 수동 검증이 담당한다.

⭐ **규칙 6 — 쓰기 후 되읽기 확인(2026-09-03, 이슈 #110).** 위와 모순되지 않는다: `--set` 직후
같은 사전으로 되읽어 **"그 디바이스에 실렸는가"** 만 확인한다(매칭 서비스 ≥ 1, 모든 서비스
배열 ⊇ 합성 배열). 실패는 `HidMappingError::NotApplied` 로 올리고 원장 ③(정확집합)을 건너뛴다.
`hidutil --set` 이 매칭 실패에도 exit 0 이라 생긴 구멍(내장 키보드 미설치가 조용히 "성공")을
막는 것이 목적이다. 디바이스당 서브프로세스가 2→3회로 늘어난다(기동 시 동기 실행 — 1~2대에서
체감 없음, 기록만 한다).

### 7.5 결정 D-17-5 — 전역 D-1 잔재의 1회 이관

구버전(이슈 #19 머지 시점까지의 빌드)은 D-1 을 **전역**으로 설치했다. 그 잔재가 커널에
남은 채 새 빌드가 디바이스 한정 쓰기를 시작하면, 다음 전역 쓰기가 모든 per-device 배열을
지운다. 그래서:

- `reconcile_on_start` 의 **가장 첫 단계**에서 딱 한 번, 전역 배열을 읽어 **우리 D-1 서명만
  제거**한 나머지를 전역에 쓴다. 전역 배열에 우리 서명이 없으면 **아무것도 하지 않는다**
  (남의 전역 매핑을 건드리지 않는다 — PR #23).
- 이것이 코드베이스에서 **유일한 매칭 없는 `--set`** 이며, 이름부터 그렇게 못박는다:
  `HidutilBackend::migrate_clear_global_d1()`. `HidMappingBackend` **트레이트에는 넣지 않는다**
  — 기능 쓰기 경로에서 호출할 수 없게 하기 위해서다.
- **순서가 안전성을 보장한다**: 이 호출은 어떤 per-device 쓰기보다 먼저 일어나고, 그 뒤
  재조정이 모든 디바이스의 배열을 다시 계산해 쓴다.

### 7.6 이 구현이 실측으로 확정한 것 — 명세 §9 의 `(미확정)` 세 건을 좁힌다

⭐ 아래는 이 작업 중 이 기기(`Mac16,11` · macOS 26.5.2 빌드 25F84)에서 **직접 실행해 확인**한
것이다. 명세 §9 의 해당 항목을 이 근거로 갱신했다.

| 명세 §9 # | 질문 | 이 작업의 실측 결과 |
| :--- | :--- | :--- |
| 8 | `com.apple.keyboard.fnState` 가 실제 저장 키 이름·도메인이 맞는가 | ⭕ **맞다.** `defaults read -g com.apple.keyboard.fnState` → `1`. `NSGlobalDomain`(`kCFPreferencesAnyApplication`)의 `com.apple.keyboard.fnState` 로 확정 |
| 9 | Karabiner 등의 가상 디바이스가 `Keyboards` 탭 목록에 나타나는가 | ⭕ **나타나지 않는다.** Karabiner-Elements 가 **실행 중인 상태**(`Karabiner-Core-Service` · `Karabiner-VirtualHIDDevice-Daemon` · DriverKit `dext` 셋 다 살아 있음)에서 `hidutil list` 전체 253행 중 `karabiner`/`pqrs`/`virtual` 에 걸리는 행이 **0건**. 즉 제품 결정이 필요 없다 — 목록에 애초에 오지 않는다 |
| 7 | `hidutil list` 의 `Built-In` 컬럼의 원천 IOKit 프로퍼티 | 여전히 `(미확정)`. 이 구현은 `Built-In` 을 **식별에 쓰지 않으므로**(식별자는 VID+PID 다) 기능에 영향이 없다 — 읽히면 표시에 쓰고, 못 읽으면 `None` 으로 둔다 |

⚠️ ~~§9 #1(Apple VID 서드파티 키보드 vs 진짜 Apple 내장 키보드)은 그대로 `(미확정)` 이다~~ →
⭐ **2026-09-03 부분 해소(이슈 #110, 스파이크 S-10)** — MacBook Air `Mac17,4` 의 내장 키보드는
VID/PID 프로퍼티가 **없어** `0:0` 으로 열거된다(Apple VID `0x5ac` 를 보고하지 않는다). §9 #7
(`Built-In` 원천)도 같은 실측으로 해소됐다 — 노드 프로퍼티 `Built-In` 그대로.
