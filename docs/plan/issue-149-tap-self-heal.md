# 이슈 #149 — 탭 사망 시 자가복구 교착: OutOfSync 유한 백오프 자동 재창착 + 워치독 활성 재활성화 + 로그 보존

> **성격**: `docs/plan/` 구현 위임 계획 문서. 이슈 #149 본문은 오케스트레이터 세션이 사용자 보고(2026-09-29)와
> 실기기 unified log 조사로 적은 초안이고, 이 문서가 P0 코드 재확인 위에서 **확정·정정**한다.
> (a) 발생 경위, (b) 코드 재확인, (c) 결정과 근거·기각 대안, (d) 판정 조건, (e) 구현 단위, (f) 수용 기준, (g) 미해결.
> 선행 이슈: #65(재활성화 예산·권한 모델 이관) · PR #66(`OutOfSync` 가드) · #108(워치독 안전망) · #140(마스크 재생성).

---

## 0. 요약

- 증상: `Shift+Space`(F-16.1)·`right command`(hyperkey) 등 **탭 기반 기능이 전부** 먹통이 되고 재기동으로만 해소된다. 실패 세션의 앱 로그는 유실되어 원인 경로를 직접 확정하지 못했으나(§1), 코드에 **복구가 구조적으로 불가능한 종단 상태 둘**이 존재함을 확인했다(§2).
- **교착 A (`OutOfSync` 종단)**: 재활성화 예산 소진 → 탭 해체 → `TapLost` → `OutOfSync`. PR #66 가드가 `observe_trusted(true)` 의 `Granted` 복귀를 차단하는데, 유일한 탈출구 `observe_tap_created` 는 "새 엔진이 탭을 만든 성공"을 요구하고 새 엔진은 `Granted` 전이에서만 뜬다 — **권한이 계속 true 면 순환 대기.** #65 Phase 2 의 이관 설계는 "수동 절차(권한 토글/재시작)를 사용자가 한다"를 암묵 전제로 했고, 그 전제가 실사용에서 무너졌다.
- **교착 B (통지 없는 비활성화)**: `CGEventTapEnable` 재시도는 트램폴린(통지 수령 시) 전용이고 `handle_recover_tap` KeepAlive 는 관찰만 한다. 비활성화 통지 없이 탭이 꺼지면 워치독이 매초 `RecoverTap` 을 보내도 아무 주체가 `enable()` 을 부르지 않는다 — 코드 주석이 스스로 인정한 구멍(`engine.rs:968-975`).
- 해법은 #65 의 "RecoverTap 경로에서 재생성 금지" 를 **건드리지 않는다** — #65 문서가 이미 정한 회복 경로("권한 모니터가 `Granted` 를 재확인해 완전히 새 `Engine` 을 만든다")를 유한 횟수·장시간 백오프로 **스스로 트리거**할 뿐이다(D1), 그리고 통지 기반 예산과 분리된 별도 상한으로 워치독 경로에서 재활성화를 시도한다(D2).

---

## 1. 발생 경위 (실기기 관찰, 2026-09-29)

| 시각(KST) | 관찰 | 출처 |
| :--- | :--- | :--- |
| 09-26 22:06 | v0.1.4 바이너리가 `/Applications/Ultrakey.app` 을 교체 | 파일 mtime |
| 09-26 23:00:15 | WindowServer `Sender lacks privileges to install non-process event taps` — 실행 중 번들이 교체되어 stale 권한이 실재함을 보여주는 정황. 이 로그의 발화 주체를 프로세스 단위로 확정하지는 못함 `(추정)` | `log show` unified log |
| 09-28 01:55 → 09-29 00:10 | 실패 세션 pid 4397. TIS 갱신 동반 `invalid CGEventSource` 로그가 **23:39 를 마지막으로 소멸**(그 전엔 분당 2~12회 정상 발생), 00:10 재기동(pid 45358) 후 즉시 복구 | unified log 분 히스토그램 |
| 09-29 00:10 | 재기동 이후 이 구간 sleep/lock 이벤트 없음 — 이번 실패는 절전·잠금 전이로 좁혀지지 않음 | `pmset -g log` |

**포렌식 공백**: 실패 세션의 `~/Library/Logs/Ultrakey/ultrakey.log` 내용이 현재 파일에 없다(현 파일은 45358 부팅 배너부터 시작, `ultrakey.log.1` 없음 — 부팅 시점 파일 생성/이동 흔적). 그래서 이번 실패가 **교착 A(Teardown) 였는지 교착 B(KeepAlive 무한 관찰) 였는지 확정하지 못한다**. 두 교착 모두 증상을 똑같이 설명한다. D4(로그 보존)는 다음 발증의 원인 확정 자체를 산출물로 한다.

---

## 2. 코드 재확인 (P0 접지)

| # | 사실 | 근거 |
| :--- | :--- | :--- |
| C1 | 트램폴린은 `TapDisabledByTimeout/UserInput` 통지 도달 시에만 `CGEvent::tap_enable(port, true)` 를 호출하고, 연속 예산(5회, 리셋은 실제 이벤트 통과 시)을 소비한다. `RecoverTap`/워치독 경로는 `enable()` 을 부르지 않는다 | `crates/ultrakey-platform/src/event_tap.rs:404-444, 197-228` |
| C2 | `handle_recover_tap`: `Teardown`(`!trusted ∥ exhausted`) 시 탭·프로브 슬롯 해체 + `TapLost` 발신, 그 외 `KeepAlive` 는 `tap_state_after_reenable(enabled)` 설정과 로그뿐 — **관찰 전용**(#65 교정 2) | `crates/ultrakey-engine/src/engine.rs:867-903`, `lifecycle.rs:150-156` |
| C3 | 워치독은 매 `watchdog_poll_ms`(기본 1000) 동안 `probe.is_enabled()` 이 거짓이면 `RecoverTap` 을 보낸다. `ReenableBudget` 은 통지에서만 소비되므로 **통지 없는 비활성화에서는 예산이 영구 미소진 → C2 의 KeepAlive 무한 루프**. 코드 주석: "실기기에서 관찰된 적은 없다 — 관찰되면 트램폴린 쪽에 별도 신호가 필요하다" | `crates/ultrakey-engine/src/watchdog.rs:60-106`, `engine.rs:968-975` |
| C4 | `TapLost` → `report_tap_create_failed()` → `observe_tap_create_failed(trusted)`: trusted 면 `OutOfSync`. `observe_trusted(true)` 는 `OutOfSync` 에서 가드로 막혀 `Granted` 로 전이하지 않는다(PR #66). 탈출구는 `observe_tap_created` 뿐이고, 그 호출자는 `TapState::Active` 전이 콜백(`on_engine_event`) — 엔진이 멈은 뒤엔 도달 불가 | `crates/ultrakey-permissions/src/model.rs:79-111`, `apps/ultrakey-app/src/main.rs:5623-5653, 5692-5698` |
| C5 | `OutOfSync` 는 `should_stop_engine_for` → `queue_engine_stop` → 엔진 전체 종료·슬롯 비움. 다음 `Granted` 전이가 있어야 `start_engine_if_needed` 가 새 엔진을 만든다 — C4 의 가드 때문에 그 전이가 안 온다. **종단 확인** | `main.rs:5330-5343, 5427-5441, 6846-6851` |
| C6 | 엔진 기동 후 최초 탭 생성이 `NotTrusted` 로 실패하면 `EngineEvent::NotTrusted` → 모달만 띄우고 **권한 모델에는 알리지 않는다**(`report_tap_*` 미호출). D1 의 재시도가 이 경로를 밟으면 모델은 `Granted` 인데 엔진 슬롯은 실패한 엔진으로 `Some` — 이후 전이가 없어 **시도 자체가 소음 없이 유실된다**(별도의 세 번째 구멍, D1 에 포함해 막는다) | `engine.rs:980-1033`, `main.rs:5634-5638` |
| C7 | 권한 폴링 주기: 온보딩 500ms/배경 5000ms. `OutOfSync` 는 `poll_mode` 가 Onboarding 이라 폴링은 계속 돈다 — 백오프 타이머를 폴링 루프에 얹을 수 있다(새 스레드 불필요). `model` 은 락을 놓은 뒤 콜백을 부르는 규약 준수 필수 | `crates/ultrakey-core/src/settings/mod.rs:70-76`, `monitor.rs:59-91` |
| C8 | `OutOfSync` 진단 모달 문구는 실재한다(`modal_copy` 분기) — 사용자는 죽은 상태에서 "권한 어긋남" 화면을 본다. 이번 사용자 보고에 모달 언급이 없는 것은 모달이 열려도 `LSUIElement`/가려진 창 때문일 수 있다 `(추정)` | `main.rs:2005` |
| C9 | `keyboard_type::current_keyboard_is_jis()` 가 `CGEventSourceGetKeyboardType(NULL)` 을 부르고, 실측에서 TIS 갱신마다 SkyLight `invalid CGEventSource: 0x0` (에러 레벨, 2회/호출) 이 남는다. 기능 결함은 아니나 이번 조사의 시간 축 신호로 쓰인 로그이며 소음 자체는 제거 가능 | `crates/ultrakey-platform/src/keyboard_type.rs:18-27`, unified log 실측 |
| C10 | 합성 이벤트용 프로세스 공유 소스가 이미 있다(`event::shared_event_source()`) — C9 대체 소스로 재사용 가능 | `crates/ultrakey-platform/src/event.rs:55` |

---

## 3. 결정과 근거

### D1. `OutOfSync` 에 유한 백오프 자동 재창착 — 권한 모니터 폴링 루프 안에서 판정

- `PermissionModel` 이 시간(`now_ms`)을 받아들인다. `observe_trusted(trusted, now_ms)` (시그니처 확장)는 `OutOfSync` && `trusted == true` 일 때 무조건 차단하던 것을, **`last_out_of_sync_ms + backoff(attempts) ≤ now_ms` && `attempts < 3`** 이면 `attempts += 1` 하고 `OutOfSync → Granted` 전이를 반환한다. backoff = **[30_000, 120_000, 600_000] ms**.
- 반환된 `Granted` 전이는 기존 배선 그대로 `on_permission_transition` → `start_engine_if_needed` → **완전히 새 `Engine`** 을 탔다. #65 Phase 2 가 금지한 것은 *한 엔진 안에서* RecoverTap 트리거로 `handle_recreate_tap` 을 부르는 것이고(#65 문서가 명시한 회복 경로 그 자체: "권한 모니터가 `Granted` 를 재확인해 완전히 새 Engine 을 만드는 것"), D1 은 그 명시된 경로를 타이머로 깨우는 것이다 — 금지 위반이 아니다.
- `attempts` 리셋: `observe_tap_created`(Active 도달)와 `trusted == false` 관측(사용자 토글이 곧 새 시작)에서 0. `Granted` 도달 실패(재시도가 `NotTrusted`/예산 소진으로 다시 `OutOfSync`)는 `last_out_of_sync_ms` 를 갱신해 다음 백오프 단계를 탄다.
- 3회 실패 후에는 **조용히 멈추지 않는다**: 마지막 `OutOfSync` 복귀 시점에 ERROR 로그 + 트레이 알림(`TrayNotification` 또는 기존 모달 상주 유지)으로 사용자 절차가 남았음을 고지한다. `(구현 세부 — C8 모달 노이즈 문제와 함께 P3 에서 정리)`
- C6 의 세 번째 구멍을 함께 막는다: `EngineEvent::NotTrusted` 도 `report_tap_create_failed()` 를 부르게 배선한다(모달 표시는 유지). 안 그러면 D1 재시도가 실패한 엔진으로 슬롯을 채운 채 모델은 `Granted` 에 고착해 시도가 유실된다.

**기각한 대안**

- (i) **폴링마다 `observe_trusted(true)` 를 통과시켜 매 5초 엔진 재기동** — PR #66 검수가 정확히 기각한 "저속 순환"(생성 → 예산 소진 → 해체 → 생성)이 그대로 재발. 기각 유지. D1 은 이걸 **유한 횟수 + 장시간 백오프**로 좁힌 것이다.
- (ii) **프로세스 자동 재실행**(원본 `Relaunch` 류) — 명세 §3-a "재시작" 문단이 채택 보류한 무거운 수단. 새 엔진이 탭·예산·경로를 전부 리셋하므로 재실행과 같은 효과를 프로세스 생존 하에 낸다. stale TCC 가 **진짜 권한 무효**인 경우는 어차피 재실행으로도 못 고치므로(D1 3회 실패 후 수동 고지가 최종 방어), 자동 재실행은 추가 커버리지가 없다.
- (iii) `tccutil reset` 자동 실행 — `permissions-onboarding.md` §3.3 이 이미 기각한 이유(보안 DB 광범위 초기화 위험) 그대로 유지.

### D2. 워치독 경로에 재활성화 시도 부여 — 통지 예산과 **분리된** 상한

- `EngineCommand::RecoverTap` 에 이유를 싣는다: `RecoverTap { reason: DisableReason }` — `NotificationTimeout | NotificationUserInput | WatchdogSilent`(콜백은 C1 을 통해 kind 를 이미 안다; 워치독은 자체 생성).
- `EventTap` 에 `silent_reenable: Cell<u32>` 신설. `handle_recover_tap` 의 KeepAlive 분기에서 `!enabled && reason == WatchdogSilent && silent_reenable < 3` 이면 `tap.enable(true)` 호출 + 카운트 증가 + WARN 로그("watchdog-initiated re-enable attempt N/3"). 실제 이벤트가 트램폴린에 도달하면(`note_real_event` 를 공유) **두 카운터 모두 리셋** — 폭주 정의("실제 이벤트 없이 즉시 재질림")가 통지 경로와 동일하게 성립한다.
- 3회로도 안 살아나면 `recover_tap_decision` 의 해체 조건을 `!trusted || budget_exhausted || silent_budget_exhausted` 로 확장해 **Teardown → D1 경로로 이관**. 즉 교착 B 는 더 이상 종단이 아니라 교착 A(그리고 D1)로 합류한다.
- #65 교정 2 가 `handle_recover_tap` 의 enable 을 제거한 근거(같은 프레임 `is_enabled()` 이 항상 stale true → 에스컬레이션 불능)는 여기서 성립하지 않는다: 워치독 경로는 **1초 간격 폴링**이고 stale true 는 다음 폴링에서 식별되며, 설령 stale true 가 반복돼도 상한 3회에서 Teardown 으로 확정된다. 통지 기반 `ReenableBudget`(mach 속도 폭주 방지)는 그대로 두고, 그보다 1000배 느린 별도 예산을 두 층으로 쓰는 것이 폭주 재점화 없이 교착 B 를 소진한다.
- silent disable 임에도 `is_enabled()` 이 true 를 반환하는 상태(프로브가嘘를 보는 케이스)는 이번 설계로도 잡히지 않는다 — 이벤트 heartbeat 확인이 필요하나 범위 밖(§7).

### D3. `handle_recreate_tap` "created but disabled" 엣지의 관찰 로그를 승격

- C2/교착 B 문서화가 "log+관찰" 이었으므로, `CreateAttemptResult::CreatedButDisabled` 도 D2 의 silent 예산을 쓰도록 한다(같은 카운터 재사용 — created-but-disabled 는 곧 `!enabled && WatchdogSilent` 와 동일한 구별 절차). 별도 상태머신 추가 없음.

### D4. 포렌식: 로그 보존과 사인(dump)

- `open_log_file`: 단세대 rename → **3세대 순환**(`.1`→`.2` 로 밀기)으로 확장. 부팅 배너는 유지하되 직전 세대 유실을 막는다.
- 탭 상태 전이를 `TapThreadState` 에 링 버퍼(최근 32개: 시각·전이·트리거)로 누적하고, **`Teardown` 즉시 전체를 WARN 로그로 덤프**한다. "왜 죽었나"가 재기동 없이 로그 한 덩어리로 남는 것이 목표.
- 워치독 `RecoverTap` 발송은 로그가 이미 WARN으로 남기므로 덤프에 포함 가능(전이를 커맨드 원인으로 기록).
- `WatchdogSilent` 판정 자체가 이번 실측 공백의 해답 열쇠다 — 다음 발증에서 "watchdog: tap detected disabled" 반복 + attempt 로그가 보이면 교착 B, "giving up on this tap" 가 보이면 교착 A 이다.

### D5. C9 소음 제거(부수)

- `current_keyboard_is_jis()` 를 `event::shared_event_source()`(C10)로 조회한다. SkyLight 에러 소거 — 부재 시 `(추정)` 이던 "NULL 허용" 전제를 실측으로 대체하지 않아도 되는 방향. 동작 변화가 없는지 검증 항목에 넣는다(JIS gate 로그 값 동일).

---

## 4. 판정 조건 (순수 함수·테스트)

| 대상 | 서명 | 필수 테스트 |
| :--- | :--- | :--- |
| `model::observe_trusted` | `(trusted, now_ms) -> Option<Transition>` + 백오프 판정 내부 | (1) `OutOfSync` 에서 backoff 미만 시각 → `None`(PR #66 가드 유지 확인) (2) 도달 → `Some(OutOfSync→Granted)` + `attempts` 증가 (3) 4회째 `None` (4) `observe_tap_created` 후 attempts 0 리셋 (5) `trusted=false` 관측 시 attempts 0 리셋 + `Denied` 전이 열림 |
| `lifecycle::recover_tap_decision` | `(trusted, budget_exhausted, silent_exhausted)` | 기존 2축 테스트를 3축으로 확장 + `KeepAlive∧!enabled∧!silent_exhausted` 는 시도, 소진은 `Teardown` |
| `event_tap::SilentReenableBudget` | `note_watchdog_attempt() -> bool`, `note_real_event()` 리셋 | 통지 `ReenableBudget` 과 독립 카운터 확인 — 통지 소진이 silent 를 소진시키지 않고 그 반대도 아님 |
| `lifecycle::tap_state_after_reenable` | 유지 | unchanged |

- 폴링 루프에 시간을 넣는다: `monitor.rs` 는 `Instant` baseline 으로 ms 를 계산해 모델에 전달(순수 모델은 시스템 시계 무관). `check_now` 도 같은 인자 경유.

## 5. 구현 단위 (위임 4건)

| 단위 | 파일 | 내용 |
| :--- | :--- | :--- |
| P1 | `crates/ultrakey-permissions/{model,monitor}.rs` + `apps/ultrakey-app/src/main.rs`(`NotTrusted` 배선·재시도 소진 고지) | D1·C6. 명세 연동: `permissions-onboarding.md` S4·§3.3 개정본 |
| P2 | `crates/ultrakey-platform/src/event_tap.rs` + `crates/ultrakey-engine/{engine,lifecycle,watchdog}.rs` | D2·D3. `EngineCommand::RecoverTap { reason }`·silent 예산·decision 3축. 명세 연동: `key-remapping-engine.md` §3-a·§5 #29/#30 |
| P3 | `apps/ultrakey-app/src/main.rs`(init_logging/open_log_file·`TapThreadState` 링 버퍼는 engine 측) | D4. 로그 덤프 포맷은 `architecture.md` 갱신 불필요(진단 로그 규약은 로그 자체) |
| P4 | `crates/ultrakey-platform/src/keyboard_type.rs` | D5. |

- 순서: P2 → P1 → P3 → P4 (P2 가 교착 B 를 A 로 합류시키고, P1 이 A 를 소진한다; P3 은 독립).
- 각 단위는 기존 `cargo test` 전체 + 수동 검증(`docs/dev/manual-verification.md`)로 마감.

## 6. 수용 기준

1. 통지 없는 비활성화 시뮬레이션(단위 테스트: `WatchdogSilent` RecoverTap 3회 → `enable` 시도 3회 로그 → Teardown → `TapLost`) 후, 백오프 30초에 새 엔진이 뜬다 — **탭 생성 성공 시 키 기능이 ≤30초 + 1사이클 안에 자동 복구**되고 로그에 전 과정이 남는다.
2. `OutOfSync` 에서 재창착 3회 실패 후에도 폴링이 조용히 순환만 계속하지 않는다 — ERROR 로그 1회와 사용자 고지가 남고, 이후 `trusted` 의 true→false→true 왕복 없이도 **모달이 계속 상주**한다(C8 점검: 설정 창이 가려져 있어도 모달이 실제로 보이는지 실기기 확인).
3. 기존 #65 폭주 시나리오(권한 회수 후 재활성화⇄비활성화 mach 핑퐁)는 회귀 없다 — 통지 예산 소진 → Teardown → D1 백오프(30초)가 첫 재시도이고, 재시도가 다시 핑퐁을 만들면 새 통지 예산 5회에서 해체 후 다음 백오프. 폴링이 Granted 로 새 엔진을 띄우는 빈도는 **최소 30초 간격 상한**으로 구속됨을 테스트로 고정.
4. D5: `log show --process ultrakey-app` 에서 `invalid CGEventSource` 가 사라지고, `is_jis` 게시값이 이전과 동일하다.
5. 재기동 외 회복 수단이 전무했던 원 증상이 자동 복구로 대체된다: 실기기에서 탭을 꺼뜨리는 재현 절차(권한 실회수 포함, §7)로 ≤10분 내 복구 또는 명시적 고지 관찰.

## 7. 미해결 질문

| # | 질문 | 확인 방법 |
| :--- | :--- | :--- |
| 1 | 이번(09-28/29) 실패가 교착 A 였는지 B 였는지 — 로그 유실로 미결정 | D4 도입 후 다음 발증 시 덤프로 확정 |
| 2 | WindowServer 09-26 23:00 거부 로그의 발화 주체와 ultrakey 세션의 대응 | 동일 상황 재현(실행 중 .app 교체) 후 unified log + 앱 로그 대조 |
| 3 | silent disable 임에도 `CGEventTapIsEnabled` 가 true 를 반환하는 "프로브嘘" 상태의 실재 여부 — D2 로는 못 잡는다 | 탭 콜백 진입 자체를 세는 heartbeat(합성 이벤트/스누프) 신설 필요. 범위 밖, 재현 관찰 시 후속 이슈 |
| 4 | `Secure Event Input` 활성화/해제가 탭을 통지 없이 끄는가(#144 실측과 겹침) | password field 진입 전후 `CGEventTapIsEnabled`+통지 관찰 |
