# 이슈 #140 — 입력 지연 2차: 이벤트 탭 마스크를 설정과 연동(마우스 이벤트가 필요 없는 구성에서는 마스크에서 제외)

> **성격**: `docs/plan/` 구현 위임 계획 문서. 이슈 #140 본문의 현행 사실·설계 방향 1~4 는
> 오케스트레이터 세션이 초안으로 적은 것이라, 이 문서가 P0 재확인 위에서 **확정·정정**한다.
> (a) 코드 재확인, (b) 결정과 근거·기각 대안, (c) 판정 조건, (d) 구현 단위, (e) 실측 결과 요약.
> 작성 2026-09-06, planner(위임 세션 본체). 선행 #139 는 PR #141(`d1333d1`)로 머지됐고 이
> 문서는 그 위에서 시작한다.

---

## 0. 요약

- 탭이 마우스 11종을 **항상** 받는 것이 구조적 상수 비용이다(커서 이동마다 WindowServer ⇄ 탭 스레드 동기 왕복).
  마스크를 `EngineConfig` 에서 도출해, 마우스 이벤트를 실제로 소비할 수 있는 구성에서만 넣는다.
- 마스크는 `CGEventTapCreate` 시점에 고정되므로 설정 변경으로 마스크가 달라지면 탭을 **재생성**한다.
  이슈 #65 가 굳힌 "재생성 금지" 는 `RecoverTap`(비활성화 통지·예산 소진·권한 상실) 트리거에 대한 것이고,
  사용자 설정 변경은 그 트리거가 아니다 — 이 구분을 `lifecycle.rs` 순수 판정 함수와 명세 §3-a 에 명시한다.
- 이슈 초안의 "기본 구성 = 트랙패드·`mouse_apply` 모두 off" 는 **사실과 다르다**(`MouseApply::default().click == true`).
  정확한 조건은 §1 C4·C5 — "합성 modifier flags 가 생길 수 있는 규칙이 하나라도 있는가" 다.

---

## 1. 코드 재확인 (P0 접지)

| # | 사실 | 근거 |
| :--- | :--- | :--- |
| C1 | 마스크는 `macos_impl::build_event_mask()` 가 고정 생성(키 3종 + 마우스 11종). `EventTap::create(callback)` 은 마스크 인자를 받지 않고 보관하지도 않는다. | `crates/ultrakey-platform/src/event_tap.rs:207-236, 370-383` |
| C2 | `handle_recreate_tap` 은 `tap_thread_main` 의 최초 설치 1회만 부른다. `RecoverTap` 은 `recover_tap_decision(trusted, exhausted)` 로 **해체 또는 관찰**만 한다 — 재생성 없음. 기각 근거: stale `AXIsProcessTrusted()` 아래서 재생성마다 새 `ReenableBudget`(5회)이 채워져 "더 느린 폭주" 가 된다. | `engine.rs:847-957`, `lifecycle.rs:100-135` |
| C3 | `drain_commands` 는 `CommandChannel` 을 갖고 있지 않다(#65 교정 2 로 필요가 사라졌음). 탭 재생성은 콜백을 다시 만들어야 하므로(`build_callback(cell, commands, start)`) 채널이 다시 필요하다 — 닭과 달걀(채널은 `CommandSource` 가 만들어진 뒤에야 생긴다). | `engine.rs:959-1018, 1073-1091` |
| C4 | `MouseMoved` 소비처는 `gates.trackpad_freeze_cursor && kind == MouseMoved`(계층 `TrackpadFreeze`) 하나. 게이트는 리스너 스레드가 게시하고, 리스너는 앱이 `hyperkey.trackpad.enabled && hyperkey.hyper.enabled` 일 때만 띄운다(`reconfigure_trackpad`). | `arbitration.rs:567-574`, `engine.rs:707-711`, `apps/ultrakey-app/src/main.rs:2203-2233` |
| C5 | 클릭·드래그·이동·스크롤의 `PassWithFlags` 는 `active`(= `active_synth_flags()` ∪ 트랙패드 hyper flags) 가 비어 있지 않을 때만, 그리고 `mouse_apply.{click,drag,move,scroll}` 이 켜진 종류만. `active_synth_flags()` 의 원천은 두 가지뿐 — ① `modifier_rules`(hyper/meh/bleh) ② `source_actions[].hold_remap == Some(RuleAction::Key{keycode})` 이면서 `keycode.modifier_flags()` 가 있는 프리셋(예: `Remap caps lock to: left control`). 트랙패드 hyper flags 도 `ModifierKind::Hyper` 규칙이 있어야 비어 있지 않다. | `arbitration.rs:645-668, 1142-1149, 1547-1552`, `keystate.rs:274-330, 383-391` |
| C6 | `MouseApply::default()` 는 `click: true`. `HyperkeySettings::default()` 는 hyper/meh/bleh 전부 비활성 → `to_modifier_rules()` 빈 목록. 프리셋 크레이트(`ultrakey-presets`·`ultrakey-language-presets`)는 마우스 이벤트를 직접 다루지 않는다(비테스트 grep 0건). | `settings/mod.rs:29-37`, `hyperkey/src/lib.rs:291-309, 420-423`, explorer grep |
| C7 | `EngineConfig` 에 트랙패드 정보가 없다. 앱의 `build_engine_config` 가 `rules`·`mouse_apply` 를 채운다. 설정 변경 경로: `settings_set` → `apply_setting` → `reconfigure_engine`(`Engine::reconfigure` → `force_reset_state`(옵션) → `reconfigure_trackpad`). | `settings/mod.rs:83-110`, `main.rs:1635-1684, 2160-2188` |
| C8 | 워치독은 `health_probe_slot`(`ArcSwapOption<TapHealthProbe>`)을 1 s 주기로 읽어 `is_enabled()==false` 면 `RecoverTap` 을 보낸다. 슬롯이 `None` 이면 건너뛴다. `handle_recreate_tap` 은 슬롯을 비운 뒤 새 probe 를 채운다. | `watchdog.rs:60-78`, `engine.rs:895-916` |
| C9 | 앱은 `TapStateChanged(Active)` 에 `PermissionMonitor::report_tap_created()`(이미 `Granted` 면 전이 없음 — 멱등), `TapLost` 에 `report_tap_create_failed()` 를 부른다. `Installing` 등 다른 상태는 로그만. | `main.rs:5546-5590`, `permissions/src/monitor.rs:141-149` |
| C10 | `log_string_discipline.rs` — `crates/`·`apps/` 의 `tracing::*!` 안 한글 금지, 카탈로그 호출 금지. 신규 로그는 영어 리터럴. | `apps/ultrakey-app/tests/log_string_discipline.rs:40, 275-334` |
| C11 | #139 실측 도구·기준선: `cargo run -p ultrakey-platform --release --example latency_probe -- --interval-ms 10 --duration-s 30 --spike-ms 20`. 수정 후 ② 유휴 p50 0.83 ms · max 11.11 ms · 스파이크 0, ① 미실행 p50 0.53 ms · max 15.90 ms. 검증 빌드는 `scripts/build-signed.sh` 산출 `.app` 을 `open` 으로 실행, 로그는 `~/Library/Logs/Ultrakey/ultrakey.log`. | `docs/dev/input-latency-spike.md`, `manual-verification.md:54-130` |

---

## 2. 결정과 근거

### D1. 마스크 도출은 두 층으로 나눈다 — core 가 "필요한 종류", platform 이 "비트"

- `ultrakey-core::tap_mask::MouseEventNeeds { click, drag, r#move, scroll }` 와 순수 함수
  `mouse_event_needs(&EngineConfig) -> MouseEventNeeds`:
  - `synth_flags_possible` = `rules.modifier_rules` 중 flags 가 비어 있지 않은 규칙이 있거나,
    `rules.source_actions` 중 `hold_remap` 이 modifier 키(`KeyCode::modifier_flags().is_some()`)인 것이 있음(C5 와 동일 판정).
  - `click/drag/scroll` = `synth_flags_possible && mouse_apply.<종류>`.
  - `r#move` = `cfg.trackpad_gesture_enabled || (synth_flags_possible && mouse_apply.r#move)`.
- `EngineConfig` 에 `trackpad_gesture_enabled: bool`(기본 `false`)을 추가한다. 앱은 리스너 기동 조건과
  **같은 식**(`hyperkey.trackpad.enabled && hyperkey.hyper.enabled`)으로 채운다 — 프리즈 게이트가 켜질 수
  있는 조건과 마스크 조건을 하나로 맞추기 위함이다(C4).
- `ultrakey-platform::event_tap::build_event_mask(&MouseEventNeeds) -> u64` 는 `#[cfg(target_os = "macos")]`
  **밖**의 순수 함수로 옮긴다. `EventKind → CGEventType 비트` 표는 숫자 상수로 두고, macOS 테스트가
  `CGEventType::X.0` 와 일치함을 고정한다(비-macOS 에서도 마스크 단위 테스트가 돈다).
- 기각: (a) 마스크를 앱에서 계산해 넘기기 — 판정 근거(C5)가 `ultrakey-core` 의 중재 규칙과 결합돼 있으므로
  같은 크레이트에 두어야 규칙이 늘 때 함께 고쳐진다. (b) `!modifier_rules.is_empty()` 만 보기 — 프리셋
  `hold_remap` 경로(C5 ②)를 놓쳐 `caps lock → left control` + `⌃클릭` 이 깨진다.

### D2. 탭 재생성은 `Installing` 재사용 — 새 상태를 추가하지 않는다 (추론 강도 high 사용 지점)

- 전이: `Active | Disabled → Installing → Active / NotInstalled / Terminated`. `Installing` 은 명세 §3-a 가
  이미 "최초 설치 또는 재생성 시도" 로 정의한 상태다(`CreateAttemptResult` 문서). 새 상태 `Recreating` 은
  `AtomicTapState` u8 매핑·앱의 `TapStateChanged` 핸들러·명세 표·Event Viewer 까지 번지지만 판정에
  새 정보를 주지 않는다 — 기각.
- **#65 와의 경계**를 순수 함수로 고정한다: `lifecycle::reconfigure_tap_decision(has_tap, fatal,
  budget_exhausted, mask_changed) -> ReconfigureTapDecision { KeepTap | Recreate | NoTap }`.
  - `!has_tap`(이미 해체됐거나 미설치) → `NoTap`: 재생성하지 않는다. 복구는 여전히 권한 모니터가
    새 `Engine` 을 만드는 것으로만 일어난다(#65 교정 2 유지). 다음 `Engine::start` 가 갱신된 설정으로
    마스크를 도출하므로 변경이 유실되지 않는다.
  - `fatal` → `NoTap`. `budget_exhausted` → `KeepTap`: 폭주 중(권한 회수 직후 stale `true`)에는 새 탭 = 새
    예산을 주지 않는다. `RecoverTap` 이 곧 해체한다.
  - 나머지에서 `mask_changed` 일 때만 `Recreate`. 사용자 설정 변경은 사람의 클릭 속도로만 오므로
    폭주 원인이 아니다 — 이것이 #65 의 금지와 이 전이가 충돌하지 않는 이유다.
- 재생성 절차(탭 스레드, `drain_commands` 의 `Reconfigure` 분기 안): `arbiter.reconfigure(&cfg)` →
  판정 → `Recreate` 면 `arbiter.force_reset(&cfg)` 결과를 `apply_outcome_outside_tap`/`apply_effects_outside_tap`
  으로 방출(stuck modifier 방지, §5 #9) → `handle_recreate_tap(cell, commands)`(기존 함수 그대로:
  `tap=None`·probe 슬롯 비움·`Installing`·`EventTap::create(callback, mask)`·probe 슬롯 채움·전이).
  실패 정책은 기존 그대로(`NotTrusted → NotInstalled`, `CreateFailed → Terminated` 치명).
- C3 의 닭과 달걀은 `TapThreadState.commands: Option<CommandChannel>` 을 **최초 설치 전에 채워** 푼다
  (`tap_thread_main` 이 `CommandChannel::new` 직후 `cell.borrow_mut().commands = Some(..)`).
  기각: perform 클로저에 `RunLoopConfined<Option<CommandChannel>>` 늦은 채움 — #65 가 걷어낸 구조를
  되살리는 셈이고, 상태 셀에 넣는 편이 접근 지점이 하나다.
- 재생성 창의 이벤트: 옛 탭 drop(런루프 소스 제거·`CFMachPortInvalidate`) 과 새 `CGEventTapCreate` 가 같은
  스레드에서 연속 실행되므로, 그 사이 이벤트는 탭이 없는 상태로 **그대로 통과**한다(유실이 아니다).
  직전 `force_reset` 으로 합성 modifier 는 이미 내려가 있다.
- `handle_recreate_tap` 문서 주석의 "생애주기당 정확히 한 번" 은 "최초 설치 1회 + 설정 마스크 변경 시" 로
  정정한다. `RecoverTap` 경로에서 부르지 않는다는 금지는 그대로다.

### D3. 기각한 대안(이슈 3항 재확인)

- (a) 마우스용 `ListenOnly` 탭 분리 — `PassWithFlags` 는 이벤트를 제자리에서 바꿔야 하므로 `Default` 탭이어야 한다. 불가.
- (b) 마스크 고정 유지 — 이 이슈의 대상.
- (c) 앱 재시작 요구 — UX 열화. 재생성 비용은 `CGEventTapCreate` 1회로 사용자가 체감하지 못한다.

---

## 3. 판정 조건 (테스트·동작으로 확인)

| # | 조건 | 확인 수단 | 귀속 |
| :--- | :--- | :--- | :--- |
| A1 | 기본 구성(modifier 규칙 없음·modifier `hold_remap` 없음·트랙패드 off)에서 마스크가 키 3종뿐이다. | core 단위 테스트(`mouse_event_needs(EngineConfig::default())` 전부 false), platform 단위 테스트(마스크 == 키 3종 비트), 실기기 `tap created` 로그의 마스크 필드 | U1·U2·U4 |
| A2 | 트랙패드 제스처를 켜면(hyper 활성 + trackpad 활성) 앱 재시작 없이 `MouseMoved` 가 마스크에 들어가 커서 프리즈가 동작하고, 끄면 다시 빠진다. 재생성 로그 + `Installing → Active` 전이가 남는다. | core 테스트(`trackpad_gesture_enabled` → `r#move`), 실기기 로그·수동 확인 | U1·U3·U4 |
| A3 | hyper + `mouse_apply.click`(기본) 구성에서 클릭 6종이 마스크에 있고 hyper-클릭 flags 덮어쓰기가 유지된다. `hold_remap` 이 modifier 키인 프리셋만 있어도 같다. | core 테스트 2건, 실기기 수동 1건 | U1·U4 |
| A4 | 재생성 직전 `force_reset` 이 실행돼 stuck modifier 가 남지 않는다. | 코드 경로(리뷰) + 실기기: hyper 소스를 누른 채 토글해도 뗀 뒤 modifier 잔류 없음 | U3·U4 |
| A5 | FSM 경계: `reconfigure_tap_decision` 이 (탭 없음→`NoTap`, fatal→`NoTap`, 예산 소진→`KeepTap`, 마스크 동일→`KeepTap`, 그 외 변경→`Recreate`) 를 만족하고, `RecoverTap` 경로(`recover_tap_decision`·`handle_recover_tap`)는 무변경이다. | lifecycle 단위 테스트, 리뷰어 diff 대조 | U3 |
| A6 | 키 이벤트 경로 무변화 — 마스크 축소 뒤에도 keyDown 이 탭을 통과하고 리매핑이 동작한다. | 실기기: hyper 조합 동작 확인(또는 `ULTRAKEY_TRACE_TAP=1`) | U4 |
| A7 | ② 유휴 지연이 ①(미실행) 수준으로 수렴한다(기본 구성). | `latency_probe` 30 s, #139 기준선과 표로 대조 | U4 |
| A8 | `cargo test --workspace`·`cargo clippy --workspace --all-targets -- -D warnings`·`log_string_discipline`·`cargo check -p ultrakey-app --features keychain-store` 통과. 문서 3종 갱신. | CI 4단계 로컬 실행 | U1~U5 |

---

## 4. 구현 단위

| 단위 | 내용 | 판정 |
| :--- | :--- | :--- |
| U1 | `EngineConfig.trackpad_gesture_enabled` 추가, `ultrakey-core::tap_mask`(`MouseEventNeeds`·`mouse_event_needs`) + 테스트, `ultrakey-platform::event_tap::build_event_mask(&MouseEventNeeds)` 를 cfg 밖 순수 함수로 + 테스트(macOS 에서 `CGEventType` 상수 대조 포함). | A1·A2·A3 |
| U2 | `EventTap::create(callback, mask: u64)` + `EventTap::mask()` 보관(스텁 포함). `handle_recreate_tap` 이 현재 `cfg` 로 마스크를 도출해 넘기고 `tap created` 로그에 마스크(마우스 종류 포함 여부)를 남긴다. 앱 `build_engine_config` 가 `trackpad_gesture_enabled` 를 채운다. | A1 |
| U3 | `lifecycle::reconfigure_tap_decision` + 테스트. `TapThreadState.commands` 늦은 채움. `Reconfigure` 분기에서 판정 → `force_reset` 방출 → `handle_recreate_tap`. 문서 주석 정정. | A4·A5 |
| U4 | 실측: 기본 구성 ② 유휴 `latency_probe` vs #139 기준선·①. 트랙패드 토글 재생성 로그·프리즈 동작. hyper-클릭·stuck modifier·키 경로 확인. | A2·A3·A4·A6·A7 |
| U5 | `architecture.md` §2(마스크 도출·재생성 전이·닭과 달걀 해법), `key-remapping-engine.md` §3-a(`Active/Disabled → Installing` 트리거 추가·`Active` 행 "설정된 마우스 이벤트" 구체화), `input-latency-spike.md` 수치 추가, 이 문서 §5. | A8 |

---

## 5. 실측 결과 요약

(U4 뒤 기록)
