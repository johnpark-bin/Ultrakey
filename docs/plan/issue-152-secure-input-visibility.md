# 이슈 #152 — Secure Input 활성 구간 리매핑 무표식 정지에 관측성 부여

> **성격**: `docs/plan/` 구현 위임 계획 문서. 설계상 의도된 일시중단(`key-remapping-engine.md` §5 엣지 6/#28)이지만
> **표식이 없다**는 것이 결함이라는 판단 아래, 복구가 아니라 **가시성**(로그 + 트레이)만 부여한다.
> 사실과 추정을 구분한다(`AGENTS.md` §3 문서 규약).

---

## 1. 발생 경위 (실측 인용)

| 관찰 | 출처 |
| :--- | :--- |
| 실패 창에서 `IsSecureEventInputEnabled()==true` 실측 | 실기기 조사 (호출 세션 제공, 사실) |
| `sudo` 대기고 없이 유지 = 보유자 누수 정황 | 실기기 조사 (호출 세션 제공, 사실) |
| 1Password 상주 | 실기기 조사 (호출 세션 제공, 사실) |

Secure Input 이 켜지면 세션 탭에 키 이벤트가 아예 전달되지 않는다(탭은 Active 유지, 0-d 게이트는 콜백 안에 있으므로
실행조차 안 됨) → 로그 0줄, UI 0신호, 사용자는 앱 고장으로 오인한다. 보유자 해제 시 이벤트별 게이트라 자동 재개된다 —
따라서 고칠 것은 복구가 아니라 **가시성**이다.

## 2. 코드 재확인

| # | 사실 | 근거 |
| :--- | :--- | :--- |
| C1 | 0-d 게이트가 콜백 안에서 `secure_input.is_enabled()` 를 보고 원본 통과 + `force_reset` 한다 | `crates/ultrakey-engine/src/engine.rs` 0-d 분기 |
| C2 | 탭 스레드 밖에서 상태를 바꾸는 유일한 통로는 명령 큐 + `CommandSignaller` 다 | `crates/ultrakey-engine/src/command.rs` 모듈 문서 |
| C3 | 워치독은 기존 폴링 주기(`watchdog_poll_ms`, 기본 1000ms)에서 탭 프로브 폴링 + caps-lock 안전망(#108/#144)을 얹는다 — 새 타이머 없음 | `crates/ultrakey-engine/src/watchdog.rs` |
| C4 | `drain_commands` 의 `RecoverTap` 핸들러 규약: `TapThreadState.fatal` 이면 early-return 한다 | `crates/ultrakey-engine/src/engine.rs` `handle_recover_tap` |
| C5 | 앱은 정상/`unauthorized` 두 벌 메뉴를 미리 조립해 두고 권한 전이 때 갈아 끼운다 | `apps/ultrakey-app/src/main.rs` `setup_tray`/`apply_tray_menu_for_permission` |
| C6 | 카탈로그 5종 parity를 검사하는 기존 테스트가 있다 | `crates/ultrakey-i18n/src/lib.rs` `all_catalogs_have_identical_key_sets` |

## 3. 결정과 근거

### D1. `EngineCommand::SecureInputChanged(bool)` + `EngineEvent::SecureInputChanged(bool)` 신설

- 워치독이 기존 폴링 주기 안에서 `SecureInputProbe::is_enabled`(`DirectSecureInputProbe` 는 단위형이라
  `Engine::start` 배선에서 값으로 넘긴다)를 읽고, **전이에서만** commands 채널로 발송한다.
- `drain_commands` 핸들러: WARN 로그(영어 리터럴) + `(st.on_event)(EngineEvent::SecureInputChanged(..))`.
  ON: `secure input is active — path A remapping is suspended by design (issue #152); this is not a tap fault`.
  OFF: `secure input released — path A remapping resumes`.
  `fatal` 일 때는 다른 `RecoverTap` 핸들러 규약과 동일하게 early-return 한다.

**기각한 대안**: 콜백 안에서 직접 게시 — §2.2 가 콜백 임계 경로의 동기 로깅·할당을 금지하므로 기각.
보유자 추적·해제 강제(`tccutil` 류) — Non-goals 로 제외.

### D2. 앱 트레이 경고 항목

- `on_engine_event` 분기: true → 트레이 메뉴 최상단에 **비활성 경고 항목**
  `menu.status.secure_input_suspended` 를 얹어 교체(reason=`secure-input`), false → 기존 `normal_menu` 로 원복.
  기존 `apply_tray_menu_for_permission` 패턴과 같은 `run_on_main_thread` 큐잉, 락 규약 준수.
- 메뉴 조립은 `build_normal_menu` 가 맡고, 조립 순서는 테스트 가능한 순수 함수
  `tray_menu_items_for_secure_input` 로 고정한다 (`main.rs` tests 모듈에 회귀 테스트).
- `AppState` 에 `secure_input_suspended: AtomicBool` 게이지 + `secure_input_menu_shown: AtomicBool` 표시 게이지를 두고
  메뉴 재조립 시 읽는다(중복 이벤트 안전).
- i18n: 새 라벨은 전부 카탈로그에 동일 키(`menu.status.secure_input_suspended`)로 추가한다(하드코딩 한국어 문자열 금지).

## 4. 구현 단위

1. `command.rs` — `SecureInputChanged(bool)` 신설.
2. `engine.rs` — `EngineEvent::SecureInputChanged(bool)` 신설, `drain_commands` 핸들러, `Watchdog::spawn` 호출부에
   `DirectSecureInputProbe` 추가 인수.
3. `watchdog.rs` — 새 스레드 없이 기존 폴링 주기 안 전이 감지 발송 + 순수 함수 `secure_input_transition` + 회귀 테스트
   (ON→OFF→ON 3회만 발송).
4. `main.rs` — `on_engine_event` 분기, `build_normal_menu(…, secure_input_suspended)` 경고 항목(최상단),
   `apply_secure_input_tray_state` 헬퍼, `AppState` 게이지 2종, 순수 함수 회귀 테스트.
5. 카탈로그 5종에 동일 키 추가(기존 parity 테스트 통과).
6. `docs/spec/key-remapping-engine.md` §5 엣지 6 행에 관측성 한 줄 갱신 + 본 계획 문서 신설.

## 5. 수용 기준

- 변경이 위 파일로 한정되고, 새 스레드 없음, rustfmt 실행 없음, 무관 코드 재포맷 없음(diff 로 확인).
- 회귀 테스트: (a) 전이 함수 발송 로직 ON→OFF→ON 3회, (b) 메뉴 조립 함수 suspended=true 최상단 경고 / false 없음,
  (c) 카탈로그 parity 기존 테스트 통과.

## 6. 미해결 (Non-goals)

- Secure Input 보유자 추적·해제 강제(`tccutil` 류) — 하지 않는다.
- 명세 §8 수용기준 대량 갱신, other 탭 UI — 하지 않는다.
