# 이슈 #125 — Seek 인풋 박스 ⇧+Space "여러 번 눌러야 동작" — hyper 슬롯 잔류 수정 — 실측·확정·구현

> **성격**: `docs/plan/` 실측·계획 문서. P1(중급, 이 세션)의 코드 재확인 + 초안 →
> P2(상급 서브에이전트, opus, `claude` 에이전트로 기동 — `ultrakey-review` 는 opencode
> 전용이라 이 P2 판정이 그 리뷰를 대신함, §8 참고)의 조건부 통과 → P3(이 세션)
> 확정 → P4(이 세션)구현·검증까지 전 과정을 기록한다. **최종 채택안은 §8.**
>
> **선행**: `docs/plan/issue-121-seek-shiftspace-tap.md` (이슈 #121, PR #122 — 코드
> 변경 제로, 후보 E `(미확정)`). 이슈 #125 는 그 후보 E-② (hyper 슬롯 잔류)를
> 오케스트레이터가 코드로 확정한 뒤 위임한 후속이다.

---

## 1. 위임이 이미 확정한 원인 사슬 (재확인 대상)

이슈 #125 본문이 코드 근거와 함께 제시한 사슬:

1. `RIGHT_COMMAND` 는 hyper source(추적 modifier) — `HoldConfirmed` 전이 시
   `active_synth_flags()` 가 비어있지 않게 된다(`keystate.rs:383-391`).
2. D2 가드(`evaluate_korean_rules` KeyDown, `arbitration.rs:1252`):
   `if !self.state.active_synth_flags().is_empty() { return false }` — hyper 합성
   flags 가 잔류하면 F-16.1 이 조용히 미발화 → SPACE 리터럴 통과.
3. `is_tracked`(FSM 진입, `arbitration.rs:572-576`)는 세션 게이트
   (`arbitration.rs:491`) **뒤**다 — 세션 중 RIGHT_COMMAND 해제가 계층 1 에서
   `SeekKey` 로 소비되어 `handle_tracked_key_event` 에 도달하지 못한다 → 세션이
   열린 동안 `HoldConfirmed` 가 해제되지 않는다(이슈 #121 F5 의 "양날").
4. 세션 열림 경로(`apps/ultrakey-app/src/seek.rs` `SessionEffect::Opened`,
   :283-)는 `seek_session_active` 원자값만 게시하고 `force_reset_state()` 를
   부르지 않는다.

## 2. P1 재확인 — 코드 재실측

### 2-1. 사슬 1~4는 코드로 재확인됨

- `keystate.rs:383-391` `active_synth_flags()` — 그대로.
- `arbitration.rs:1252` D2 가드 — 그대로. T8(`input_box_f16_1_does_not_fire_while_hyper_active`,
  :2642)이 "진짜 hyper 활성이면 미발화"를 인코딩하는 것도 그대로.
- `arbitration.rs:572-576` `is_tracked` 진입이 `gates.seek_active` 분기
  (:491-556) **뒤** — 그대로.
- `apps/ultrakey-app/src/seek.rs:283-341` `SessionEffect::Opened` 핸들러 —
  `env.shared.seek_session_active.store(true, ..)` 등 원자값 게시만 하고
  `force_reset_state` 호출 없음 — 그대로.

### 2-2. ⭐ 추가로 재확인한 사실 — 정본 눌림 테이블은 세션과 무관하게 갱신된다

`arbitrate_with_kind`(`arbitration.rs:446-460`) 최상단:

```rust
match kind {
    EventKind::KeyDown => self.state.set_pressed(ev.keycode, true),
    EventKind::KeyUp => self.state.set_pressed(ev.keycode, false),
    _ => {}
}
```

이 갱신은 **세션 게이트(:491)보다 앞**에서, 모든 이벤트에 대해 무조건 실행된다.
또한 modifier 키의 `FlagsChanged` → `KeyDown`/`KeyUp` 환원(`normalize_kind`,
:726-766)은 **그 이벤트 자신의 flags 비트만으로** 뗌을 판정한다(정본 테이블과
무관 — 오히려 정본 테이블이 stale 할 때 사실을 바로잡기 위한 설계, 주석
:734-744, 이슈 #108).

**결론**: RIGHT_COMMAND 를 실제로 뗄 때, `is_pressed(RIGHT_COMMAND)` 는 세션이
열려 있어도(그 이벤트가 계층 1 에서 `SeekKey` 로 소비되어도) **정확히 `false`
가 된다**. 잘못된 채로 남는 것은 FSM 슬롯(`slots[i].state`)뿐이다 —
`handle_tracked_key_event` 가 불리지 않아 `HoldConfirmed` → `Idle` 전이가 나지
않는다.

즉 stale 상태는 **"물리적으로는 이미 뗐는데(`is_pressed==false`) FSM 슬롯만
`HoldConfirmed` 로 남은" 것**이지, "물리적으로 계속 눌려 있는" 것이 아니다. 이
둘은 이미 존재하는 정본 데이터(`is_pressed`)로 코드상 구분 가능하다.

### 2-3. 기준선 테스트로 고정 (신규, `arbitration.rs`)

`issue125_stale_hyper_after_session_release_no_longer_blocks_f16_1` 추가:

1. 세션 밖에서 F13(hyper) hold 확정 (T8 과 동일 준비).
2. 인풋 박스 세션 오픈.
3. 세션 중 F13 KeyUp → `release.layer() == Layer::SeekSession`(F5 재확인 — FSM
   미도달, `SeekKey` 로 소비).
4. `arb.state.is_pressed(F13) == false` — **정본 눌림 테이블은 세션 중에도
   정확히 갱신됨**(§2-2 주장의 재현).
5. `arb.state.active_synth_flags().is_empty()` — **현재 코드에서는 실패**
   (stale 슬롯이 남아 비어있지 않음). 이것이 이슈 사슬의 코드 재현이다.
6. ⇧+Space → 기대: `Layer::KoreanInput` + `Consume`(1회 탭 발화) — 현재
   코드에서는 도달 안 함(D2 가 막음).

`cargo test -p ultrakey-core --lib issue125_stale_hyper` 결과: **실패**,
정확히 5번 어서션(`active_synth_flags 가 stale 하게 남으면 안 된다`)에서 —
가설과 일치.

---

## 3. 수정 후보 두 가지 (P2 확정 대상)

### 후보 ① — 세션 열림 시 `force_reset_state()` (이슈 본문 1순위)

`SessionEffect::Opened` 핸들러(`seek.rs:283`)에서 `Engine::force_reset_state()`
(기존 `ForceResetState` 명령, 새 기구 아님)를 호출 — 세션은 항상 깨끗한
modifier baseline 에서 시작.

**필요한 배선**: `WorkerEnv`(`seek.rs:184`)는 현재 `Arc<SharedState>`(원자값
전용)만 들고 있고 `Engine`/`CommandChannel` 핸들이 없다. `Engine::commands()`
접근자를 새로 추가(`system_hooks.rs` 의 `DelaySchedulerHandle::commands()` 와
같은 기존 패턴)하고, `seek::spawn()` 시그니처에 `CommandChannel` 을 추가로
꿰어야 한다(`main.rs:5441` 호출부도 수정).

**장점**: 이슈가 요구한 "1순위 후보"와 정확히 일치, "세션은 클린 상태에서
시작"이라는 단순하고 이해하기 쉬운 불변식.

**단점(⭐ P2 가 판단할 긴장)**:
- **모든** 세션 오픈마다(인풋 박스가 아니어도) 전체 FSM 을 리셋한다 — 문제가
  D2(인풋 박스 한정) 하나인데 영향 범위가 전체 modifier/프리셋 상태로 넓다.
- `force_reset` 은 `HoldConfirmed` 인 슬롯에 대해 "off" `FlagsChanged` 를
  합성해 낸다(`arbitration.rs:1538-1577`). **세션이 열리는 순간 사용자가
  진짜로 어떤 modifier 를 물리적으로 누르고 있는 채였다면**(예: hyper 키를
  누른 채로 다른 키로 Seek 를 열었다면), 그 modifier 를 강제로 "뗀 것"으로
  처리해 버린다 — 이후 실제 물리 릴리즈가 왔을 때 이미 뗀 것으로 처리된
  뒤라 아무 일도 안 하지만, **세션이 열린 순간부터 정당한 hold 가 임의로
  끊긴다.** T8 이 지키려는 "진짜 hold 는 존중" 원칙과 정면으로 부딪힌다 —
  T8 자체는 세션 오픈 **이후의** 판정(D2)만 보므로 깨지지 않겠지만, "hold
  하던 modifier 가 세션 오픈 시점에 이유 없이 끊긴다"는 **새로운** 사용자
  가시 회귀를 만들 수 있다.
- `is_kind_active`/`slot_flags_for` 등 다른 소비처(§2-2 계층 2 "유지" 로직,
  :640)에도 영향 — 세션 밖에서는 어차피 이 슬롯들이 무관하지만, 세션이
  끝나고 다시 세션 밖으로 돌아왔을 때 진짜 hold 가 사라져 있는 게 맞는지는
  "그 사용자가 여전히 물리적으로 누르고 있었는가"에 달렸다 — 알 수 없다.

### 후보 ② — D2 가드를 "물리적으로 안 눌려 있으면 무시"로 좁힘 (stale 감지) — ⭐ P1 권고

§2-2 에서 확인했듯 **정본 눌림 테이블은 세션과 무관하게 항상 정확하다.** D2
가드가 보는 값을, 물리적으로 눌려 있는 `HoldConfirmed` 슬롯만 세도록 좁힌다.

⚠️ **구현 위치 — `active_synth_flags()` 자체를 고치지 않는다.** 처음에는
`keystate.rs:383-391` `active_synth_flags()` 를 직접 고치는 안을 그려봤으나
두 가지 이유로 기각하고, **D2 전용의 새 메서드**로 좁혔다:

1. **기존 화이트박스 테스트가 `pressed`/`state` 를 의도적으로 독립시켜
   시험한다.** `keystate.rs` 의 `active_synth_flags_ors_multiple_active_rules`
   (:442-462)와 `register_sources_modifier_hold_remap_carries_target_flags`
   (:541-561)는 `set_machine(.., HoldConfirmed)` 만 부르고 `set_pressed` 를
   전혀 호출하지 않은 채로 `active_synth_flags()` 가 그 flags 를 포함하길
   기대한다 — `KeyStateTable` 은 "눌림 비트셋"과 "FSM 슬롯 상태"를 원래
   독립된 필드로 문서화한다(모듈 문서 :1-6). `active_synth_flags()` 의
   계약을 여기서 바꾸면 이 두 테스트가 깨진다 — 실제 결함과 무관한 단위
   테스트를 고쳐야 하는 것 자체가, 이 계약 변경이 이 함수의 본래 층위보다
   한 단 위(호출자 쪽 판단)에 속한다는 신호다.
2. **`active_synth_flags()` 의 다른 소비처는 세션 중 도달 불가능하다** —
   재확인 결과 `arbitration.rs:640,909,925,951,968`(계층 2 유지·hold
   시작/종료)과 `:1418,1470`(F-19 언어 규칙)은 전부 `handle_tracked_key_event`/
   `evaluate_language_rules`/`fire_language_alone_tap` 를 거치는데, 이들은
   `is_tracked` 게이트(:572-576) 뒤 — **세션 게이트(:491) 뒤에만** 도달한다.
   즉 **D2(:1252)가 `gates.seek_active` 블록 안에서 `active_synth_flags()` 를
   부르는 유일한 자리**다. stale 문제는 세션이 열려 있는 동안에만 발생하므로,
   고칠 자리도 D2 하나로 좁힐 수 있다.

`keystate.rs` 에 D2 전용 메서드를 추가한다:

```rust
/// `active_synth_flags()` 와 달리 **물리적으로 지금 눌려 있는** 소스 키의
/// `HoldConfirmed` 슬롯만 합산한다 — 이슈 #125. Seek 세션 게이트가
/// `is_tracked`(FSM 진입)를 가려, 세션 중 소스 키를 실제로 뗐는데도 FSM
/// 슬롯이 `HoldConfirmed` 로 잔류하는 stale 상태를 걸러낸다. 정본 눌림
/// 테이블(`pressed`)은 세션과 무관하게 항상 정확하다(`arbitrate_with_kind`
/// 최상단이 세션 게이트보다 앞에서 갱신 — arbitration.rs 문서 참고).
pub fn active_synth_flags_of_pressed_slots(&self) -> EventFlags {
    let mut flags = EventFlags::NONE;
    for slot in self.slots.iter().flatten() {
        if matches!(slot.state, QuickPressState::HoldConfirmed) && self.is_pressed(slot.key) {
            flags |= slot.rule_flags;
        }
    }
    flags
}
```

`Arbiter` 에 얇은 위임 추가(기존 `pub fn active_synth_flags(&self)`,
:345-350 바로 옆에), D2 가드(:1252)만 이 새 메서드로 교체한다. 다른 20여
개 기존 테스트·다른 호출부는 전부 `active_synth_flags()` 그대로 — 영향
없음.

**정합성(왜 다른 곳을 깨지 않는가)**: 위 2번 근거대로 세션 밖에서는
`HoldConfirmed` 전이·해제가 항상 FSM 을 통해서만 일어나므로, **정상 동작에서는
`HoldConfirmed` ⇒ `is_pressed(key)==true` 가 항상 성립한다**(같은 이벤트 처리
안에서 눌림 테이블 갱신과 FSM 전이가 함께 일어나므로). D2 는 세션 안에서만
불리므로 이 새 메서드가 실제로 갈라지는 것은 정확히 stale 케이스뿐이다.

**T8 과의 정합**: T8 은 세션 오픈 **전**에 F13 을 hold 확정하고, 세션 중에도
F13 을 **떼지 않은 채** ⇧+Space 를 누른다 — 즉 `is_pressed(F13)==true` 인 채로
D2 를 시험한다. 후보 ②는 이 경우 `active_synth_flags()` 가 여전히 F13 의 flags
를 포함하므로 **T8 은 그대로 통과한다**(비회귀). 반대로 §2-3 신규 테스트처럼
F13 을 세션 중에 뗀 뒤(stale)라면 `is_pressed(F13)==false` 가 되어 flags 에서
빠지고, D2 가 정상 발화한다.

**장점**:
- 배선 변경 없음(`WorkerEnv`/`Engine::commands()` 신설 불필요) — `keystate.rs`
  에 메서드 하나 추가 + D2 가드 한 줄 교체.
- 영향 범위가 D2 하나 — 이슈가 지목한 정확한 결함(HoldConfirmed 인데
  물리적으로 안 눌려 있음 = 모순 상태)만 겨냥하고, 다른 20여 개
  `active_synth_flags()` 소비처·테스트는 원래 계약 그대로 유지된다(§ 위
  "구현 위치" 참고).
- 세션 열림 시점에 진짜 hold 를 임의로 끊는 부작용이 없다(후보 ①의 최대
  단점 회피) — 물리적으로 눌려 있는 한 `is_pressed` 는 계속 `true`.
- `force_reset` 이 "off" `FlagsChanged` 를 내보내는 부작용(후보 ①의 무해성
  판단이 필요했던 지점)이 아예 없다 — 아무 이벤트도 새로 합성하지 않는다.

**FSM 슬롯 잔류는 안전한가 — P1 이 `quickpress.rs` 전이표로 확인함**:
후보 ②는 판정만 고치고 `slots[i].state` 자체는 `HoldConfirmed` 로 남긴다.
이 잔류가 세션이 닫힌 뒤 문제를 일으키지 않는지 `quickpress.rs` 의 상태
전이 함수 3개를 전수 확인했다:

- `on_key_down`(:98) — `HoldConfirmed` 에서 그 키 자신의 다음 keyDown(세션
  밖, 진짜 재입력)은 `(HoldConfirmed, None)` — 이미 확정된 상태를 유지할
  뿐 새 `HoldStart` 를 내지 않는다(autorepeat 과 같은 취급, §5 엣지 13
  설계 그대로). 이 시점에 `arbitrate_with_kind` 최상단이 `set_pressed(key,
  true)` 를 이미 했으므로, `active_synth_flags_of_pressed_slots()` 는 이
  키를 다시 "물리적으로 눌림"으로 정확히 센다 — 결과적으로 옳다.
- `on_key_up`(:129) — `HoldConfirmed` 에서 그 키 자신의 다음 keyUp(세션
  밖, 진짜 릴리즈)은 `(Idle, Some(HoldEnd))` — **잔류가 여기서 스스로
  청소된다.** 이것이 세션 안에서 원래 오지 못했던 바로 그 전이다 — 세션
  밖에서 같은 키가 한 번이라도 다시 눌렸다 떼어지면 FSM 슬롯이 완전히
  정상 상태로 돌아온다(자기 치유). `HoldEnd` 는 정상적인 "off"
  `FlagsChanged` 를 낸다 — 실제 물리 릴리즈에 대응하는 정당한 이벤트라
  후보 ①의 "세션 오픈 시 강제 off" 와 달리 의심할 이유가 없다.
- `on_other_key_down`(:141) — `HoldConfirmed` 는 매치되지 않고 `other`
  분기로 떨어져 `(HoldConfirmed, None)` — 다른 키가 눌려도 잔류 슬롯에
  대해 아무 것도 새로 하지 않는다(`dispatch_other_key_down` 이 이 슬롯의
  `HoldStart` 를 재방출하지 않는다는 뜻). 세션 밖에서 잔류 슬롯이 다른
  키 이벤트에 영향을 주는 유일한 경로는 `active_synth_flags()` 를 읽는
  자리들인데(§2-1 목록), 그 자리들은 D2 와 달리 세션과 무관하게 항상
  `HoldConfirmed ⇒ is_pressed==true` 불변식이 성립하는 경로에서만
  불린다(§2-1) — 세션이 닫힌 직후 그 키가 아직 눌린 적 없는(즉
  `is_pressed==false`) 잔류 상태에서 **다른 키**가 눌려 `active_synth_flags()`
  가 불리면(예: `emit_hold_start`, :909), 잔류 슬롯의 flags 가 여전히
  섞여 들어간다 — **이 경로는 후보 ②가 고치지 않는다.** 다만 이것은
  이슈 #125 가 겨냥한 "인풋 박스 D2 미발화" 증상과는 무관한 별도 표면
  (세션 밖·다른 키의 hyper flags 오염, 매우 좁은 창)이라 이번 수정
  범위 밖으로 판단했다 — P2 가 이 잔여 표면도 닫을지(예: D2 와 같은
  `_of_pressed_slots` 변형을 §2-1 의 세션-도달-불가 자리에도 적용) 판단할
  수 있다.

**단점(⭐ P2 가 판단할 지점)**:
- 위에서 짚은 "세션 밖·잔류 슬롯이 다른 키의 `active_synth_flags()` 읽기를
  오염시키는" 좁은 잔여 표면 — 후보 ①(전면 `force_reset`)은 이것까지
  청소하지만, 발생 조건이 좁고(세션 중 hyper 릴리즈가 유실된 뒤, 세션이
  닫히고, 그 hyper 키 자신은 아직 다시 눌리지 않은 채, 다른 키가 눌리는
  순간) 이슈 #125 의 재현 증상과 직접 관련이 없다.
- "판정만 고치고 상태는 안 고친다"가 개념적으로 절반짜리 수정처럼 보일 수
  있다 — 그러나 `active_synth_flags()` 자체가 원래도 "판정용 도출값"이지
  FSM 의 정본 상태가 아니므로(문서 주석 :378 "현재 HoldConfirmed 인 모든
  modifier 규칙의 flags 를 OR 로 합산한 값" — 파생값), 도출 시점에 눌림
  테이블과 대조하는 것은 도출 로직을 더 정확하게 만드는 것이지 임시방편이
  아니다.

### 후보 비교 요약

| | ① 세션 오픈 시 force_reset | ② D2 전용 `active_synth_flags_of_pressed_slots` |
| :--- | :--- | :--- |
| 배선 변경 | `Engine::commands()` 신설 + `WorkerEnv`/`spawn()` 시그니처 변경 | 없음(`keystate.rs` 메서드 1개 + D2 한 줄) |
| 영향 범위 | 모든 세션 오픈(인풋 박스 무관) · 전체 FSM/래치 | D2 판정 한 자리만 |
| 진짜 hold 보존(T8) | 판정 자체는 유지되나 **세션 오픈 순간 hold 가 끊길 위험** | 물리적으로 눌려 있는 한 항상 보존 |
| FSM 슬롯 잔류 | 세션마다 전면 리셋(청소됨) | 세션 밖 자기 치유(다음 press/release 사이클) — 확인 완료(위) |
| 세션 밖 다른 키 오염(좁은 잔여 표면) | 닫음 | 안 닫음(범위 밖으로 판단, P2 확인 필요) |
| "off" 합성 이벤트 부작용 판단 필요 | 필요(세션 오픈 시점 무해성) | 불필요 |
| 기존 단위 테스트 영향 | 없음 | 없음(D2 전용 신규 메서드라 `active_synth_flags()` 계약 불변) |

**P1 권고**: 후보 ② — 배선이 가장 좁고, T8 이 지키려는 "진짜 hold 존중"을
가장 직접적으로 만족시키며(물리 눌림 여부로 정확히 구분), 새 부작용
표면(세션 오픈 시 강제 "off" 이벤트)이 없고, FSM 잔류의 자기 치유를
`quickpress.rs` 전이표로 확인했다. 남는 것은 "세션 밖 다른 키 오염"이라는
좁고 별개인 잔여 표면뿐 — 이슈 #125 범위 밖으로 보되, P2 가 원하면 같은
패턴(§2-1 의 세션-도달-불가 자리에도 `_of_pressed_slots` 적용)으로 마저
닫을 수 있다.

---

## 4. `(미확定)` — 유지

- 정확한 "세 번째" 주기 수치는 여전히 `(미확정)` — 런타임 의존, 실기기 절차로.
- hyper 슬롯이 애초에 왜 stale HoldConfirmed 가 되는지(정상 해제가 실기기에서
  유실되는 트리거)도 `(미확정)` — 이번 수정은 트리거를 없애지 않고, **트리거가
  발생해도 D2 판정이 stale 을 무해하게 걸러내도록** 한다.

## 5. 금지 승계 (#121)

새 이벤트 탭·새 레이어·새 FSM 금지, `select_input_source` 직접 호출 금지,
인풋 박스 밖 동작 변경 금지, #106/#120/#122 테스트·동작 무효화 금지.

## 6. P2 에게 요청하는 판정

1. **방향 적절성** — §3 두 후보 중 어느 쪽을 채택할지. P1 은 후보 ②
   (D2 전용 `active_synth_flags_of_pressed_slots`)를 권고한다 — 배선 없음,
   T8 비회귀 자명, FSM 잔류 자기 치유를 `quickpress.rs` 전이표로 이미
   확인함(§3 "FSM 슬롯 잔류는 안전한가"). 절충안(예: 후보 ②를 채택하되
   "세션 밖 다른 키 오염" 잔여 표면도 같은 패턴으로 닫기)도 검토 가능.
2. **후보 ①을 판단한다면**(P1 권고를 뒤집는 경우): `Engine::commands()`
   신설 + `WorkerEnv`/`seek::spawn()` 시그니처 변경의 배선 비용과, "세션
   오픈 시점에 진짜 물리 hold 를 강제로 끊을 수 있다"는 §3 의 새 회귀
   표면을 감수할 근거를 명시해 달라.
3. **초안 타당성** — §2-3 기준선 테스트(`issue125_stale_hyper_after_session_release_no_longer_blocks_f16_1`)
   가 실제로 실패함을 확인했다(`cargo test -p ultrakey-core --lib
   issue125_stale_hyper` → FAILED, 정확히 예상 어서션에서). 이 재현이
   사슬의 코드 검증으로 충분한지.
4. §3 이 표로 정리한 평가 기준 확정 — P4 가 이 기준으로 완료 판정.
5. 가능하면 `ultrakey-review` 서브에이전트/스킬 리뷰도 함께.

## 7. P1 이 이미 만들어 둔 것 (P4 가 이어받을 상태)

- 기준선 테스트 `issue125_stale_hyper_after_session_release_no_longer_blocks_f16_1`
  이 `crates/ultrakey-core/src/arbitration.rs` 의 T8 바로 뒤에 이미
  추가돼 있고, **수정 전 상태에서 실패를 확인했다**(위 §2-3). P2 확정
  후보를 구현하면 이 테스트가 통과해야 한다 — P4 는 이 테스트를 그대로
  완료 판정 (b)로 쓸 수 있다.
- 이 문서는 아직 워킹 브랜치에 커밋되지 않았다 — P4 가 구현 커밋에 함께
  포함하거나, 별도 문서 커밋으로 먼저 올릴 수 있다.

---

## 8. P2 판정 요약 + P3 확정 + P4 구현·검증 결과

### 8-1. P2(opus, `claude` 에이전트로 기동) 판정 — 조건부 통과

**채택**: 후보 ②(D2 전용 `active_synth_flags_of_pressed_slots`). P1 의 권고와
결론은 같지만 **근거를 교체**했다 — P2 가 파일을 직접 읽어 검증한 결정적
사실: `force_reset`(후보 ①)은 `KeyStateTable::reset_all()`(`keystate.rs:366-376`)
을 부르고, 이는 FSM 슬롯뿐 아니라 **정본 눌림 비트셋(`pressed`)까지 통째로
지운다.** `korean_trigger_met`(`arbitration.rs:1281-1298`)과 이슈 #121 이 이미
고정한 `issue121_stale_modifier_blocks_shift_only_characterization`
(`arbitration.rs:3349`)은 바로 그 `pressed` 테이블이 정확하다는 전제 위에
서 있다 — 후보 ①은 **세션 오픈 시점에 진짜로 눌려 있는 modifier 를 "안
눌림"으로 만들어**, #121 이 "여러 번 눌러야 함"의 뿌리로 특성화한 stale
상태의 부호만 뒤집은 쌍둥이를 새로 만든다. 또한 후보 ①은 모든 Seek 세션
오픈마다(인풋 박스 무관) 전역 상태를 건드려 `docs/plan/issue-121-seek-shiftspace-tap.md:63`
의 "인풋 박스 밖 동작 변경 금지"에 저촉된다. (배선 비용은 판단 근거가
아니다 — 서브에이전트 실측으로 `Engine::force_reset_state()` 는 이미
`pub` 이라 후보 ①의 배선도 P1 초안보다 훨씬 싸다는 것이 확인됐다; 기각
사유는 순전히 위 두 가지 정합성 문제다.)

**P1 재실측 검증**: §2-2·quickpress 3전이 전부 파일 재대조로 동의. 단
이의 2건:
- **(A) — 기준선 테스트 결함**: P1 의 §2-3 기준선 테스트가
  `arb.state.active_synth_flags().is_empty()` 를 어서션했는데, 이는 후보
  ①의 의미론(전면 리셋 → 원본 값도 비워짐)이다. 후보 ②는 원본
  `active_synth_flags()` 를 의도적으로 안 건드리므로, 후보 ②를 올바르게
  구현해도 이 어서션은 계속 실패한다. **→ §8-3 에서 수정.**
- **(B) — "D2 가 세션 중 유일한 소비처"는 사실이 아님**: `on_timer_tick`
  (`engine.rs`)은 `GateSnapshot` 을 받지 않고 `arbiter.on_tick` 을 세션
  게이트와 무관하게 부른다 — `emit_hold_start`(`arbitration.rs:909`)가 세션
  중에도 원리적으로 도달 가능하다(좁은 조건: 세션 오픈 **전**부터 다른
  추적 슬롯이 `PendingDown` 이던 타이머가 세션 중 만료). 판정은 바꾸지
  않는다(여전히 후보 ②) — 이 경로는 인풋 박스 밖이라 #121 금지 조항상
  이번 범위 밖으로 남기고 후속 이슈로 분리한다.

**quickpress 자기 치유 검증 통과** — 단, 그 소스 키의 다음 press/release
사이클이 **세션 밖에서** 일어나야 청소된다(세션 안에서는 그 사이클도
계층 1 이 삼킨다). `pressed` 를 진실 원천으로 쓰는 후보 ②는 이 창의 길이와
무관하게 항상 옳다(강화 근거로 채택).

**`ultrakey-review` 유무**: 존재하나(`.opencode/agent/ultrakey-review.md`)
opencode 전용 — `AGENTS.md:53` 이 "Claude Code·Codex 에서 상급(평가자)은
호출 세션의 모델이어서 별도 에이전트 파일이 없다"고 명시한다. 이 P2 판정
자체가 그 리뷰를 대신한다.

### 8-2. P3 확정 (이 세션)

P2 판정 전부 반영. 미반영 지적 없음. §3 의 후보 비교·"장점/단점" 서술 중
"배선이 비싸서 후보 ①을 기각한다"는 취지의 문장들은 **틀린 근거**였음을
여기 기록해 둔다 — 실제 기각 사유는 §8-1 의 `reset_all()` 정합성 문제다
(§3 본문은 실측 기록 보존을 위해 원문 그대로 두고, 이 절에서 정정한다).

### 8-3. P4 구현 (이 세션)

1. **`crates/ultrakey-core/src/keystate.rs`** — `active_synth_flags_of_pressed_slots()`
   추가(`HoldConfirmed && is_pressed(slot.key)` 인 슬롯만 OR). 기존
   `active_synth_flags()` 는 한 글자도 안 건드림.
2. **`crates/ultrakey-core/src/arbitration.rs`**:
   - D2 가드(원 :1252, 새 위치는 주석 추가로 이동) 한 줄을 새 메서드
     호출로 교체.
   - `Arbiter` 에 `active_synth_flags_of_pressed_slots()` 얇은 위임 추가
     (기존 `active_synth_flags()` 위임 바로 옆 — 트레이스 계측이 필요로
     해서 추가함, §8-4).
   - 기준선 테스트를 P2 이의 (A) 대로 수정: `active_synth_flags()` 는
     **여전히 non-empty** 임을 확인하는 어서션(후보 ②가 슬롯 상태를 안
     건드린다는 것을 명시) + `active_synth_flags_of_pressed_slots()` 가
     비어있음을 확인하는 어서션으로 교체.
3. **트레이스 계측(P2 §4 항목 (l), SHOULD)** — `crates/ultrakey-engine/src/trace.rs`
   에 `TapTrace::synth_flags_active_pressed`(POD 필드, #121 `pressed_mods_other`
   와 같은 관례) + `ViewerRecord::synth_active_pressed` 추가, `log_trace`/
   `decode_viewer_record` 에 배선. `engine.rs` 의 레코드 생성부(#121 이
   `pressed_mods_other`/`synth_flags_active` 를 채우던 자리 바로 옆)에서
   같은 스레드 메모리 읽기 한 번 더로 채운다 — `should_trace` 게이트 안,
   진단 전용, 런타임 동작 무변경. 기존 `synth_flags_active` 필드는
   그대로 둔다(#121 과의 비교 가능성 보존) — 두 필드가 갈리면(1/0) stale
   hyper 슬롯 확정.

### 8-4. 검증 결과 (완료 기준 — P2 §4 전부 충족)

| 기준 | 결과 |
| :--- | :--- |
| (a) 신규 메서드가 `active_synth_flags()` 는 안 건드림 | ✅ |
| (b) D2 가드 한 줄만 교체, 다른 소비처 그대로 | ✅ |
| (c) `Arbiter` 위임은 계측이 필요로 해 추가(공개 표면 최소) | ✅ |
| (d) 기준선 테스트(수정된 어서션) 통과 | ✅ `cargo test -p ultrakey-core --lib issue125_stale_hyper` → ok |
| (e) T8(`input_box_f16_1_does_not_fire_while_hyper_active`) 무수정 통과 | ✅ |
| (f) `keystate.rs` 기존 단위 테스트 2건(`active_synth_flags_ors_multiple_active_rules`·`register_sources_modifier_hold_remap_carries_target_flags`) 무수정 통과 | ✅ (`active_synth_flags()` 를 안 건드렸으므로 자명) |
| (g) T-118·T-121 시리즈·F-19·force_reset 계열 비회귀 | ✅ `cargo test -p ultrakey-core --lib` → 329 passed / 0 failed |
| (h) `cargo test -p ultrakey-core -p ultrakey-engine` | ✅ 전부 통과 |
| (i) `cargo test --workspace`(`bash scripts/fetch-sparkle.sh` 선행 실행 후) | ✅ 전 크레이트 통과, 실패 0 |
| (j) `cargo clippy --workspace --all-targets -- -D warnings` | ✅ 경고 0 |
| (k) 새 이벤트 탭·레이어·FSM 없음 · `select_input_source` 직접 호출 없음 · 인풋 박스 밖 동작 변경 없음 | ✅ (변경 파일: `keystate.rs`·`arbitration.rs`·`trace.rs`·`engine.rs` 뿐 — 전부 판정·계측 계층) |
| (l) 트레이스 pressed-필터 비트 추가(SHOULD) | ✅ `synth_flags_active_pressed` |

### 8-5. `(미확定)` 재확인 (§4 그대로 유지)

이번 수정이 닫는 것은 **hyper 합성 flags 잔류가 D2 를 막는 경로 하나**다.
"정확히 세 번째" 주기 수치와 "hyper 슬롯이 애초에 왜 stale 이 되는가"(정상
해제 유실의 실기기 트리거)는 여전히 `(미확정)` — 실기기 절차(§8-6, #121
M0~M9 확장)로 확인한다. P2 가 지적한 대로, 실기기에서 "여러 번 눌러야
함"이 **다른 경로로**(예: `korean_trigger_met` 을 깨는 stale `pressed` —
#121 이 특성화한 별개 경로) 남아 있다면 이번 수정으로 사라지지 않을 수
있다 — PR 본문에 이 기대치를 명시한다.

### 8-6. 실기기 검증 절차 추가 — M10 (#121 M0~M9 확장)

| # | 절차 | 기대 |
| :--- | :--- | :--- |
| M10 ⭐ | `ULTRAKEY_TRACE_TAP=1` 로 인풋 박스 ⇧+Space 가 미발화하는 탭을 잡은 뒤, `synth_active`(기존)와 `synth_active_pressed`(신규) 를 대조 | ① 둘 다 `true` → **진짜 hyper hold 중**(T8 케이스, 정상 — 사용자가 실제로 hyper 키를 누른 채임) ② `synth_active=true, synth_active_pressed=false` → **stale 확정**(이번 수정이 여기서 D2 를 풀어준다 — 이 조합이 실기기에서 관측되면 수정 후 재현 시도) ③ 둘 다 `false`인데도 미발화 → 이번 수정이 겨냥한 경로가 아님(§8-5 다른 경로 — 후속 조사 필요) |
