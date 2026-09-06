# 이슈 #139 — 입력 지연 1차: 경로 B 재적용을 탭 스레드 밖으로 · 탭 스레드 QoS · Seek 신호 무잠금 · 실측

> **성격**: `docs/plan/` 구현 위임 계획 문서. 이슈 #139 본문의 사실(F1~F4)·설계 방향(D1~D6)은
> 오케스트레이터 세션이 확정한 것이라 **재도출하지 않고 그대로 채택**한다. 이 문서는 그 위에서
> (a) 코드 재확인, (b) 각 결정의 근거·기각 대안 재검토, (c) 판정 조건, (d) 구현 단위, (e) 실측
> 결과 요약을 남긴다. 작성 2026-09-06, planner(위임 세션 본체).

---

## 0. 요약

- 원인 후보 3건 중 **F1(경로 B 재적용이 탭 런루프에서 동기 실행)** 이 유일하게 "입력이 통째로 멈추는"
  시간을 만든다 — 디바이스당 `hidutil` 3회(이 머신 실측 20~30 ms/회) + 원장 `persist` 2회(fsync).
  절전 복귀·동글 재삽입 뒤 1.5~2 s 시점에 수백 ms 급 정지가 가능하다.
- F2(QoS 없음)·F3(콜백 안 Mutex)은 상수 비용·규약 위반이며 단독으로 체감 지연을 만들 가능성은 낮다(추정).
  그래도 D2·D3 은 비용이 거의 없고 규약을 맞추므로 함께 적용한다.
- 판정은 **테스트 + 실측**으로 한다. 실측은 수정 전(main 과 동일 동작 + 계측만 추가)·수정 후 두 빌드로
  같은 매트릭스(①미실행 ②유휴 ③재적용 트리거 창)를 측정한다 — §5.

---

## 1. 코드 재확인 (P0 접지, 실측)

| # | 사실 | 근거 |
| :--- | :--- | :--- |
| C1 | `EngineCommand::ReapplyHidMapping(Option<DeviceId>)` 은 `drain_commands`(탭 스레드 `CommandSource` perform 콜백)에서 `apply_device`/`apply_all` 을 **동기 호출**한다. `None` 이면 `list_attached_keyboards()`(IOKit 열거)까지 그 스레드에서 돈다. | `crates/ultrakey-engine/src/engine.rs:991-1017` |
| C2 | 생산자는 `DelayScheduler::fire_job` 하나뿐 — `DidWake`(`wake_delay_ms`=2000 뒤 `None`)·핫플러그 `Attached`(`keyboard_connect_delay_ms`=1500 뒤 `Some(dev)`). | `system_hooks.rs:188-190, 347-360, 400-417` |
| C3 | `PathBManager` 는 `backend`/`ledger` 트레이트 객체 + `AtomicBool` 2개만 갖고 있고 **직렬화 장치가 없다**. `Engine::reconfigure`(메인)가 `apply_all` 을 동기 호출하므로 스케줄러 재적용과 동시 실행되면 커널 `UserKeyMapping` read-modify-write 가 겹칠 수 있다. | `path_b.rs:139-149, 203-266`, `engine.rs:285-295` |
| C4 | 탭 스레드는 `thread::Builder::new().name("ultrakey-tap")` 기본 QoS. 저장소 어디에도 `pthread_set_qos_class_self_np`·`thread_policy_set` 없음. `ffi.rs` 에 `libc` 의존 없이 `extern "C"` 직접 선언 관례. | `engine.rs:231`, explorer grep, `ffi.rs` |
| C5 | `AppState.seek_tx: Mutex<Option<Sender<SeekSignal>>>`. 쓰기 1곳(`main.rs:5478`, 엔진 기동), 읽기 1곳(`send_seek_signal`, `main.rs:5598`) — 읽기는 `on_engine_event` 즉 **탭 콜백 스택**에서 불린다. 앱 크레이트에 `arc-swap` 의존 있음. | `main.rs:1876, 4870, 5478, 5598` |
| C6 | 완료 로그 `"Path B (F-17) reapply completed"` 에는 `?device` 만 있고 소요 시간·디바이스 수가 없다. | `engine.rs:1014-1015` |
| C7 | `log_string_discipline.rs` 는 `crates/`·`apps/` 전체의 `tracing::*!` 매크로 안 한글을 위반으로 본다 — 신규 로그는 영어 리터럴로. | `apps/ultrakey-app/tests/log_string_discipline.rs:40, 275-310` |
| C8 | `hidutil property --get UserKeyMapping` 이 머신 3회: 0.02·0.02·0.03 s. 설치본 `/Applications/Ultrakey.app` 이 실행 중(pid 있음), `Ultrakey Dev` 자체 서명, 번들 ID `app.ultrakey.Ultrakey`. `~/Library/Logs/Ultrakey/` **부재 확인**(이슈 본문과 동일). | 이 세션 실측 |
| C9 | `latency_probe` 가 쓸 API 는 `objc2-core-graphics` 로 이미 크레이트에 있다(`key_poke.rs` 의 `CGEventSource::new(HIDSystemState)`·`CGEvent::post(HIDEventTap)`, `tap_listen.rs` 의 `tap_create(SessionEventTap, TailAppendEventTap, ListenOnly)`). `ULTRAKEY_MAGIC = 0x554B_4559`. | `examples/key_poke.rs:51-73`, `examples/tap_listen.rs`, `event.rs:18` |
| C10 | `manual-verification.md` §0: 검증은 `scripts/build-signed.sh`(universal release + `Ultrakey Dev` 서명) 산출 `.app` 을 **`open` 으로** 실행. 설치본과 서명 주체·번들 ID 가 같아 TCC 권한이 공유된다(추정 — 실측에서 확인). | `manual-verification.md:54-130` |

---

## 2. 결정과 근거 (D1~D6 채택 + 세부 결정)

### D1. 재적용을 지연 스케줄러 스레드에서 직접 실행

- `DelayScheduler::spawn(shared, commands, path_b: Arc<PathBManager>)` / `SystemHooks::start(shared, commands, path_b)` 로 시그니처 확장. `fire_job` 의 `ReapplyHidMapping` 분기가 `apply_device`/`list_attached_keyboards()+apply_all` 을 **그 자리에서** 실행하고 `shared.d1_confirmed` 를 게시하고 `elapsed_ms`·디바이스 수를 로그한다(D4).
- `EngineCommand::ReapplyHidMapping` **변형 삭제**. `drain_commands` 분기 삭제. `command.rs`·`system_hooks.rs` 문서 주석 갱신.
- 근거: 스케줄러가 유일 생산자이고 이미 전용 스레드다. 새 스레드 없이 탭 런루프에서 블로킹 작업이 완전히 빠진다.
- 기각: (a) 탭 유지 — 원인. (b) 전용 워커 — 이득 없음. (c) 재적용 제거 — #108 대응 상실. (d) **`spawn_blocking` 식 일회성 스레드** — 이벤트마다 스레드 생성, 모듈 문서의 "지연은 스레드를 새로 띄우지 않는다" 원칙과 충돌.

### D1-b. `PathBManager` 직렬화 Mutex

- `PathBManager { serial: Mutex<()>, .. }`. **공개 진입점**(`reconcile_on_start`·`apply_all`·`apply_device`·`cleanup`)에서 잡는다 — `write_device` 안에서 잡으면 `apply_all` 한 번이 디바이스 사이에서 끊겨 `reconfigure` 와 인터리브될 수 있다. 진입점 단위가 "커널을 한 번 read-modify-write 하는 논리 단위"다.
- 이 Mutex 는 메인 스레드(`reconfigure`·`shutdown`)와 스케줄러 스레드만 만진다. 탭 스레드는 D1 이후 `PathBManager` 를 참조하지 않으므로(`TapThreadState.path_b` 필드도 제거) §2.2 와 무관.
- `d1_confirmed` 게시 순서: 두 스레드가 각자 apply 뒤 `path_b.d1_confirmed()` 를 읽어 `SharedState` 에 store 한다. 인터리브되어도 두 store 는 모두 "마지막 apply 이후" 값이라 수렴한다(추정 — 값이 단조롭지 않아도 마지막 게시가 최신 apply 결과를 반영).
- 테스트: 가짜 백엔드가 `apply` 안에서 진입 중 카운터를 올리고 짧게 sleep 한 뒤 내린다. 두 스레드에서 `apply_all` 동시 호출 → 관측된 최대 동시 진입 수가 1 이어야 한다. `Mutex` 를 빼면 2 가 관측된다(테스트가 실제로 결함을 잡는지 구현 중 확인).
- 기각: `RwLock` — 읽기 경로가 없다. `parking_lot` — 새 의존성.

### D2. 탭 스레드 QoS `USER_INTERACTIVE`

- `ffi.rs` 에 `pthread_set_qos_class_self_np`·`pthread_get_qos_class_np`·`pthread_self` 를 `extern "C"` 직접 선언(기존 관례, `libc` 추가 없음). `QOS_CLASS_USER_INTERACTIVE = 0x21`(`sys/qos.h`).
- 새 모듈 `crates/ultrakey-platform/src/thread_qos.rs`: `pub fn set_current_thread_user_interactive() -> Result<(), i32>`, `pub fn current_thread_qos_class() -> Option<u32>`(되읽기). `unsafe` 는 이 모듈 안에만.
- `tap_thread_main` 진입 직후 호출. 결과(rc·되읽은 값)를 `tracing::info!` 한 줄 — 실패해도 계속(콜백 밖이라 로깅 허용).
- 실측: 로그 되읽기 값 + `ps -M <pid>` 의 `ultrakey-tap` 스레드 PRI(기본 31 → UI 47, 추정 — 실측으로 확인).
- 기각: `thread_policy_set(TIME_CONSTRAINT)` 실시간 — 튜닝·오남용 부담. QoS 가 표준.

### D3. `seek_tx` → `ArcSwapOption<Sender<SeekSignal>>`

- `send_seek_signal` 은 `load()` 가드로 참조만 얻어 `send`(crossbeam unbounded `send` 는 블로킹 아님). 기동 시 `store(Some(Arc::new(tx)))`.
- 기각: 유지 — 규약 위반을 남길 이유 없음. `OnceLock` — 재시작(엔진 재기동) 경로에서 재설정 불가.

### D4. 계측

- 재적용 완료/실패 로그에 `elapsed_ms`(진입~완료), `devices`(대상 수; `Some` 이면 1, `None` 이면 `attached.len()`), `thread` 는 기존 포맷터가 찍는다.
- **U1 에서 먼저** 탭 드레인 분기에 넣어 "수정 전 입력 정지 시간"을 잡고, U2 에서 같은 필드를 스케줄러 분기로 옮긴다.

### D5. `latency_probe` 예제

- `crates/ultrakey-platform/examples/latency_probe.rs`. 별도 스레드가 `interval_ms` 간격으로 **현재 커서 위치의 `MouseMoved`** 를 `CGEventSource(HIDSystemState)` 로 만들어 `EventSourceUserData` 에 `(TAG<<56) | ns_since_start` 를 싣고 `CGEvent::post(HIDEventTap)`. 메인 스레드는 `SessionEventTap`+`TailAppendEventTap`+`ListenOnly`, 마스크 `MouseMoved` 만인 탭으로 도착을 받아 TAG 가 맞는 이벤트만 `now_ns - encoded_ns` 로 지연을 계산한다. 마커에 `ULTRAKEY_MAGIC` 을 쓰지 않는다(0-a 즉시 통과 방지). TAG 는 상위 바이트 `0x4C`, 하위 56비트 ns(≈2.3 년 여유).
- 출력: 종료 시 `n / p50 / p95 / p99 / max (ms)` + 임계값(기본 20 ms) 초과 샘플을 **벽시계 시각(HH:MM:SS.mmm)** 과 함께 즉시 출력(③ 에서 앱 로그의 reapply 시각과 상관 확인용). 옵션: `--interval-ms`(기본 10) `--duration-s`(기본 30) `--spike-ms`(기본 20) `--csv <path>`.
- 커서 이동은 "현재 위치로 이동"이라 무해. 실제 마우스 움직임은 TAG 불일치로 무시된다.

### D6. 문서

- `architecture.md` §2.1 도식에 지연 스케줄러 스레드(경로 B 재적용 실행 주체)를 넣고, §2.2 아래에 규칙 추가: "커맨드 드레인·타이머 콜백도 탭 런루프 시간이다 — 서브프로세스·fsync·네트워크 등 블로킹 작업을 넣지 않는다. 탭 스레드는 `QOS_CLASS_USER_INTERACTIVE`". `manual-verification.md:293` 근처에 "이슈 #139 이후 실행 스레드는 `ultrakey-delay-scheduler`" 주석.
- `docs/dev/input-latency-spike.md` 신설(§0 환경, §1 한 줄 결론, §2 매트릭스, §3 상관, §4 한계·미측정).

### 추론 강도 메모
- P1 에 실제 설계 갈림길은 없었다(D1~D6 확정 입력). Mutex 위치(진입점 vs `write_device`)와 probe 의 시각 인코딩 방식만 세부 결정이라 medium 으로 진행.

---

## 3. 판정 조건 (이슈 "완료 조건" ↔ 확인 수단)

| # | 조건 | 확인 수단 | 귀속 단위 |
| :--- | :--- | :--- | :--- |
| A1 | `EngineCommand::ReapplyHidMapping` 부재, 재적용은 스케줄러 스레드, 탭 코드 경로에 `hidutil`·`persist` 없음 | `grep -rn "EngineCommand::ReapplyHidMapping" crates apps` = 0건(`DelayedJob::ReapplyHidMapping` 은 스케줄러 내부 job 이름으로 존속 — 의도된 잔존). `engine.rs` 의 `drain_commands`/`tap_thread_main`/`TapThreadState` 에 `path_b` 참조 0건. 실기기 로그의 reapply 줄 스레드가 `ultrakey-delay-scheduler` | U2 |
| A2 | `PathBManager` 직렬화 | 단위 테스트(동시 `apply_all` 최대 진입 1) 통과 | U2 |
| A3 | 탭 스레드 QoS UI | 기동 로그 `tap thread QoS` 줄의 되읽기 값 = 0x21 + `ps -M` PRI 기록 | U3 |
| A4 | `seek_tx` Mutex 없음, Seek 동작 유지 | `grep seek_tx main.rs` 에 `Mutex` 없음. 수동: 세션 열기·타이핑·한/영·ESC | U4 |
| A5 | reapply 로그 `elapsed_ms` | 로그 실물(수정 전·후) | U1→U2 |
| A6 | `latency_probe` 빌드·실행, 스파이크 문서에 전후 ①②③ | `cargo build -p ultrakey-platform --example latency_probe`; 문서 표. ③ 수정 전 max ↔ `elapsed_ms` 상관, 수정 후 ② 수준(또는 그 반대 사실을 그대로) | U1·U5 |
| A7 | `cargo test --workspace`·`clippy` 신규 경고 0·`log_string_discipline` 통과 | CI 동일 명령 실행 결과 | 전 단위 |
| A8 | 절전 복귀·동글 재삽입 뒤 D-1·per-device 매핑 유지 | 실기기: ③ 측정 뒤 `hidutil property --get UserKeyMapping` 에 D-1(0x39→F18) 확인 + 로그 `Path B applied to attached devices` | U5 |

---

## 4. 구현 단위

| 단위 | 내용 | 조건 | 담당 |
| :--- | :--- | :--- | :--- |
| U1 | `latency_probe.rs` + 탭 드레인 분기의 `elapsed_ms`/`devices` 계측(코드 변경은 이것뿐, 동작은 main 과 동일). 이어서 **수정 전 실측** ①②③(release aarch64 `.app`, `Ultrakey Dev` 서명, `open`) | A5·A6(전) | implementer → planner(실측, 사용자 협조) |
| U2 | D1 + D1-b + 변형 제거 + 테스트 + 문서 주석 | A1·A2·A5 | implementer |
| U3 | D2 FFI·모듈·호출·로그 | A3 | implementer(U2 뒤 — `engine.rs` 공유)(implementer 세션 한도로 중단 → planner 인수) |
| U4 | D3 | A4 | implementer(U2 와 병렬 가능 — `main.rs` 만) |
| U5 | 수정 후 실측 ①②③ + A3·A8 실기기 확인 → `input-latency-spike.md`·`architecture.md` §2·이 문서 §5 | A3·A6·A8 | planner(실측) → implementer(문서) |

순서: U1 → (U2 ∥ U4) → U3 → 빌드·실측 → U5 → P3 심사(reviewer, 새 컨텍스트) → P4.

### 실측 절차 (공통)
1. `cargo tauri build --target aarch64-apple-darwin --bundles app` 후 `build-signed.sh` §3 과 같은 순서로 `Ultrakey Dev` 서명(Sparkle 중첩 → 프레임워크 → 메인). universal 대신 단일 아키텍처 — 측정 목적에 충분하고 빌드 시간 절반(기각: universal — 이 머신은 arm64 이고 배포물이 아니다).
2. 설치본 종료는 **사용자 동의 뒤**(`osascript -e 'quit app "Ultrakey"'`), 끝나면 `open -a Ultrakey` 로 복원.
3. ① probe 만(Ultrakey 전부 종료) 30 s. ② 측정 빌드 `open` 뒤 탭 `Active` 확인, 30 s. ③ probe 60 s 실행 중 사용자에게 동글 재삽입 요청 → 1.5 s 뒤 reapply → 로그의 `elapsed_ms`·시각과 probe 스파이크 시각 대조.
4. 미측정 칸은 "미측정 — 사유".

---

## 5. 실측 결과 요약

> 상세·상관 분석·측정 환경은 [`docs/dev/input-latency-spike.md`](../dev/input-latency-spike.md) 참조.

### 한 줄 결론

수정 전 동글 재삽입 뒤 경로 B 재적용이 탭 스레드에서 `elapsed_ms=179`(디바이스 1대) 걸렸고, 그 완료
순간 probe 샘플 14개가 한꺼번에 풀리며 최대 **180.88 ms** 지연이 관측됐다(= 그 시간 동안 키보드·마우스
입력 전부 정지). 수정 후 재적용은 지연 스케줄러 스레드(`ultrakey-delay-scheduler`)로 옮겨졌다.
수정 후 ③(재삽입 창)은 **대기** — 사용자 외출로 재삽입 불가, 복귀 후 측정 예정.

### 전후 ①②③

| 조건 | 수정 전 max (ms) | 수정 전 spikes≥20 | 수정 후 max (ms) | 수정 후 spikes≥20 |
| :--- | ---: | ---: | ---: | ---: |
| ① Ultrakey 미실행 | 12.03 | 0 | 15.90 | 0 |
| ② 실행·유휴(기동 직후 30 s) | 33.24 | 4 | 11.11 | 0 |
| ③ 동글 재삽입 창 | 180.88 | 24 | **대기** | **대기** |

### A1~A8 판정 현황

| # | 조건 | 현황 |
| :--- | :--- | :--- |
| A1 | `EngineCommand::ReapplyHidMapping` 부재, 재적용은 스케줄러 스레드 | `grep -rn "EngineCommand::ReapplyHidMapping" crates apps` 0건, `engine.rs` 탭 경로에 `path_b` 참조 0건(P3 reviewer 재현 확인). 남은 6건은 전부 `system_hooks.rs` 의 `DelayedJob::ReapplyHidMapping`(의도된 잔존) |
| A2 | `PathBManager` 직렬화 | 단위 테스트 통과 |
| A3 | 탭 스레드 QoS UI | 기동 로그 확인(`qos_before=Some(Default) qos_after=Some(UserInteractive)`). `ps -M` 은 스레드 이름 미표시 — 귀속 불가, 판정 근거로 쓰지 않음 |
| A4 | `seek_tx` Mutex 없음 | grep 확인 완료. 수동(세션 열기·타이핑·한/영·ESC) 확인은 **대기** |
| A5 | reapply 로그 `elapsed_ms` | 수정 전·후 로그 실물로 확인 |
| A6 | `latency_probe` 빌드·실행, 전후 ①②③ 문서화 | 수정 후 ③ **대기** |
| A7 | `cargo test --workspace`·`clippy` 신규 경고 0·`log_string_discipline` | 통과 |
| A8 | 절전 복귀·동글 재삽입 뒤 D-1·per-device 매핑 유지 | 수정 후 ③ 실측과 함께 **대기** |
