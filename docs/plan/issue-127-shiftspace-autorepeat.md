# 이슈 #127 — ⇧+Space(F-16.1) 합성 ⌃Space 가 Space 자동 반복마다 재발화 — 실측·계획·구현

> **성격**: `docs/plan/` 실측·계획 문서. P1(중급, 이 세션)의 재현 테스트 + 원인 확인 +
> 수정 초안 → P2(상급 서브에이전트, opus) 조건부 판정 → P3(이 세션) 확정 → P4(이 세션)
> 구현 완료·검증까지 전 과정을 기록한다.
>
> **선행**: `docs/plan/issue-121-seek-shiftspace-tap.md`(이슈 #121, PR #122 — 코드 변경
> 제로, 후보 E `(미확정)`) · `docs/plan/issue-125-seek-shiftspace-hyper-stale.md`(이슈
> #125, PR #126 — hyper 슬롯 stale 이 D2 를 막는 경로 수정). 이슈 #127 은 #125 포함
> 빌드에서도 재발한 별개 경로 — Space **자동 반복**이 합성 ⌃Space 를 반복마다 새로
> 방출하는 결함이다.

---

## 1. 위임이 이미 확정한 사실 (재확인 완료)

이슈 #127 본문·herdr-dev-delegate 위임이 실기기 로그와 함께 제시한 사슬을 코드로
전부 재확인했다:

1. **실기기 로그**(`~/Library/Logs/Ultrakey/ultrakey.log`, UTC 18:13): 인풋 박스 세션
   2 중 입력 소스가 Korean 으로 바뀐 뒤 **208ms 후 ABC 로 되돌아감**(18:13:30.343 →
   18:13:30.551). 사용자 macOS `InitialKeyRepeat=15`(225ms) · `KeyRepeat=2`(30ms).
2. **`evaluate_korean_rules` KeyDown 은 `ev.autorepeat` 를 보지 않는다**
   (`arbitration.rs:1233`선, 수정 전) — 반복 down 마다 규칙이 다시 성립해
   `SynthEvent{KeyDown, SPACE, 0x0004_0001}` 을 매번 새로 방출한다(래치는 "자동
   반복 대비" 덮어쓰기만 함). D2(인풋 박스 세션, `:543`)·계층 3(세션 밖, `:627`)
   둘 다 같은 함수를 부른다.
3. `SynthEvent`(`arbitration.rs:99-103`)에 autorepeat 필드가 없고, 플랫폼 합성
   (`crates/ultrakey-platform/src/event.rs` `SyntheticEvent::keyboard`)도
   `kCGKeyboardEventAutorepeat` 를 설정하지 않는다 — macOS 는 반복마다 독립된
   ⌃Space 누름으로 본다(물리 ⌃Space 의 자동 반복은 그 이벤트 자신의 autorepeat
   플래그로 심볼릭 핫키 엔진이 무시하지만, 우리 합성만 예외).
4. 자기 되돌림 대안 배제: `refresh_input_source`/`LayoutResolver::rebuild` 는 입력
   소스를 선택하지 않는다(`TISSelectInputSource` 호출자 없음) — 재확인, 변경 없음.
5. **결과 = 토글 횟수 = 1 + 반복 수.** 225~255ms 홀드 → 2회 토글 → 되돌아감(세션 2
   실측). 225ms 미만 탭 → 1회 → 전환. "됐다/안 됐다"가 누름 길이에 좌우된다.

### `(미확정)` — 유지

- **정확히 "세 번째"** 수치는 누름 길이 패리티 + 알림 합침의 산물로 보이나
  결정론적 수치로 확정하지 않는다.
- **세션 1(전환 알림 0회)** 은 이 사슬로 설명되지 않는다 — 별도 후보(첫 검색 바
  창 생성 직후 키 윈도우/핫키 전달 레이스). 이번 PR 범위 밖, 실기기 절차만
  남긴다(§7 M11).

---

## 2. P1 — 기준선 재현 테스트 (수정 전 실패 확인 완료)

`crates/ultrakey-core/src/arbitration.rs`:

- `issue127_input_box_shift_space_autorepeat_synthesizes_keydown_exactly_once`
  (인풋 박스 D2 경로) · `issue127_layer3_shift_space_autorepeat_synthesizes_keydown_exactly_once`
  (계층 3, 세션 밖) — 둘 다 "⇧ down → SPACE down → autorepeat down ×3 → up" 시퀀스에서
  **합성 KeyDown 총 개수가 정확히 1** 임을 단언한다.

**수정 전 실측**: `cargo test -p ultrakey-core --lib issue127` → **FAILED** 양쪽 다
`left: 4, right: 1`(초기 1회 + autorepeat 3회 = 4회 합성 KeyDown) — 실기기 로그가
보인 "반복마다 재발화"를 코드 수준에서 정확히 재현했다.

또한 기존 테스트 `input_box_f16_1_autorepeat_substitutes_each_tick`(이슈 #121 도입,
"T-③")가 **바로 이 버그를 "기대값"으로 인코딩하고 있었다** — `autorepeat 도 ⌃Space
down 으로 치환(래치 덮어쓰기)` 을 정상으로 단언했다. 이 테스트 자체가 #127 결함의
회귀 방지 장벽이 아니라 결함을 고정하고 있었다는 뜻이다. §4 에서 기대값을 고친다
(`input_box_f16_1_autorepeat_is_consumed_without_re_synthesizing` 로 개명).

---

## 3. 수정 방향 — 두 후보와 ⭐ 핵심 제약(기존 테스트 발견)

### 3-1. 위임이 제시한 1순위: "autorepeat + 래치 있음 → 소비만"

`evaluate_korean_rules` KeyDown 에서 `ev.autorepeat` 이고 같은 `trigger_key` 래치가
이미 서 있으면 원본도 하류로 안 보내고 합성도 안 내고 소비만 한다 — D-K6 규약
(최초 down=합성 down, up=합성 up) 유지.

### 3-2. ⭐ 이 규칙을 **F-16 전체**에 무조건 적용하면 기존 테스트가 깨진다

`evaluate_korean_rules` 는 F-16 규칙 4종(`Korean(13)`~`Korean(16)`)을 **전부** 이
함수 하나로 처리한다. 그런데 이미 존재하는 재발 방지 테스트
`f16_4_autorepeat_key_down_is_substituted_every_time`(시나리오 12, F-16.4 `Korean(16)`
= won→backtick)은 **"autorepeat KeyDown 도 매번 치환된다"를 의도적으로 고정**한다.

이유는 두 F-16 규칙군의 출력 성격이 다르기 때문이다:

| 규칙 | trigger→out | 출력 성격 | 물리 키를 누르고 있을 때 기대 동작 |
| :--- | :--- | :--- | :--- |
| F-16.1 `Korean(13)` | SPACE(⇧only) → SPACE+CONTROL(0x0004_0001) | **시스템 "입력 소스 전환" 토글 핫키**(⌃Space) | **1 press = 정확히 1 토글** — 반복 발화하면 되토글(#127 그 자체) |
| F-16.2 `Korean(14)` | JIS_KANA(무mod) → SPACE+CONTROL(0x0004_0001) | 동일(F-16.1 과 완전히 같은 출력) | 동일 |
| F-16.3 `Korean(15)` | JIS_EISU(무mod) → RETURN+ALT(0x0008_0040) | **실제 RETURN 키 출력**(한자 변환 확정) | 물리 RETURN 을 계속 누르면 계속 나가는 게 정상 — 반복 억제 대상 아님 |
| F-16.4 `Korean(16)` | ANSI_GRAVE(무mod) → ANSI_GRAVE+ALT(0x0008_0020) | **실제 문자 출력**(₩→백틱, 레이아웃 경유 합성) | 물리 키를 누르고 있으면 문자가 반복 타이핑되는 게 정상 — 반복 억제 대상 아님 |

F-16.3·F-16.4 는 "토글"이 아니라 **정상적으로 반복 타이핑되는 키/문자**다. 여기에
"autorepeat 는 재발화 금지"를 적용하면 사용자가 백틱 키를 눌러 잡고 있어도 문자가
한 번만 나가는 **새 회귀**를 만든다 — F-16.1/F-16.2 의 결함과 반대 방향의 결함이다.

### 3-3. F-19(`evaluate_language_rules`) — 같은 결함이 있는가? **없다(확인 완료)**

이슈 체크리스트가 요구한 "F-19 언어 규칙 래치 경로 동일 결함 여부 확인"을 코드로
확인했다:

- `evaluate_language_rules` 의 KeyDown 분기는 `LanguageTrigger::AloneTap` 을
  **명시적으로 걸러낸다**(`!matches!(r.trigger, LanguageTrigger::AloneTap { .. })`,
  `arbitration.rs:1426`) — 즉 이 분기가 다루는 규칙은 `NoModifier`/`ShiftOnly`/
  `OptionOnly` 트리거뿐이다.
- 실제로 이 트리거로 등록되는 F-19 규칙은 **F-19.5(¥↔\)·F-19.6(JIS→US 20행)** 뿐이고,
  전부 **실제 문자 치환**(레이아웃 경유 합성) — F-16.3/F-16.4 와 같은 성격이다.
  물리 키를 누르고 있으면 반복 타이핑되는 게 맞다.
- **토글형 F-19 규칙**(F-19.1·F-19.2·F-19.3·F-19.4·F-19.7 — 캡스락/⌘ 단독 탭 →
  ⌃Space 류)은 전부 `LanguageTrigger::AloneTap` 이라 **이 함수를 아예 타지 않는다**
  — `handle_tracked_key_event`(quick press FSM, `arbitration.rs:828`)를 통해서만
  발화한다.
- FSM 의 `on_key_down`(`quickpress.rs:65`)은 **이미** autorepeat 를 방어적으로
  처리한다 — `HoldConfirmed` 에서 그 키 자신의 다음 keyDown(autorepeat 포함)은
  `(HoldConfirmed, None)`: 상태를 유지할 뿐 이벤트를 다시 내지 않는다(§ 모듈 문서,
  "autorepeat 과 같은 취급"). 즉 토글형 F-19 는 **애초에 #127 의 결함 경로에 들어간
  적이 없다**(별도 FSM 경로).

**결론**: F-19 는 수정 불필요 — `evaluate_language_rules` 의 KeyDown-래치 경로가
다루는 규칙은 전부 F-16.3/F-16.4 와 같은 "실제 문자 출력"류이고, F-19 의 토글형
규칙은 애초에 다른(안전한) 경로를 탄다.

### 3-4. 채택안 — 규칙 단위 플래그로 좁힌 소비 억제

`KoreanRule` 에 `is_input_source_toggle: bool` 필드를 추가한다(F-16.1·F-16.2 =
`true`, F-16.3·F-16.4 = `false`). `evaluate_korean_rules` KeyDown 분기 최상단
(규칙 매칭 직후, 게이트 검사 앞)에서:

```rust
if ev.autorepeat && rule.is_input_source_toggle {
    if let Some(latch) = self.state.korean_latch(rule.trigger_key) {
        out.layer = Layer::KoreanInput;
        out.disposition = Disposition::Consume;
        out.rule = Some(latch.id);
        return true; // 합성 없음 — 소비만
    }
}
```

- **래치가 있으면**(최초 down 이 이미 발화해 세워둔 것) 소비만 하고 반환 —
  게이트(앱 제외·IME·hyper 활성)를 다시 평가하지 않는다. KeyUp 래치 소비가
  "조건을 다시 평가하지 않는다"(D-K6)는 것과 대칭이다.
- **래치가 없으면**(예: 세션 열림 직후 첫 이벤트가 우연히 autorepeat 플래그를
  달고 온 경우, 또는 래치가 다른 경로로 지워진 뒤 도착한 경우 — 위임이 짚은
  "래치 없이 autorepeat 만 도착하는 경계") 아래로 흘러 **최초 down 과 동일하게**
  게이트 검사 → 발화 → 래치 생성을 거친다. 이 자기 치유는 새 상태를 도입하지
  않고 기존 래치 유무만으로 판단하므로 별도 분기가 필요 없다.
- **F-16.3·F-16.4**(`is_input_source_toggle == false`)는 이 분기를 타지 않고
  기존 로직 그대로 매 tick 재발화한다 — `f16_4_autorepeat_key_down_is_substituted_every_time`
  비회귀.
- ⭐ 방어 강화(P2 리뷰 선택-2): `korean_latch(rule.trigger_key)` 뒤에
  `.filter(|l| l.id == rule.id)` 를 붙여, 발견한 래치가 **지금 매칭된 그 규칙이
  세운 것인지** 확인한다. 지금은 F-16 규칙 4종의 `trigger_key` 가 전부 유일해
  항상 자기 자신의 래치만 발견되지만, 장차 같은 `trigger_key` 를 다른 `trigger`
  로 등록하는 규칙이 추가되면 이 확인이 없으면 교차 오염(다른 규칙의 래치가
  억제를 잘못 트리거)이 가능하다.

⚠️ **한정어(P2 리뷰 선택-3)**: 위 "게이트를 다시 평가하지 않는다"는 앱 제외·IME
활성·hyper 활성 3가지 **게이트**(`gates.korean_app_excluded` 등)에만 해당한다.
**트리거 조건**(`korean_trigger_met` — 물리 modifier 상태)은 이 분기보다 **앞**의
`find()` 단계에서 매 tick 다시 평가된다 — 예를 들어 ⇧+Space 를 누르고 있다가
autorepeat 이 오기 전에 Shift 를 먼저 떼면 `ShiftOnly` 가 거짓이 되어 이 함수
전체가 `false` 를 반환하고, 그 SPACE autorepeat down 은 D2 아래 `Effect::SeekKey`
로 흘러 검색어에 공백이 들어간다(래치는 남아 KeyUp 짝은 유지). 이것은 **수정
전과 완전히 동일한 사전 존재 동작**이며 #127 결함과는 무관하다(원인·수정 범위
밖 — 실사용 빈도가 낮고, 고치려면 D-K6 의 "게이트가 깨져도 래치는 건드리지
않는다" 규약과 별도로 정합을 설계해야 한다).

⚠️ **부작용 기록(P2 리뷰 필수-1)**: 이 분기가 만드는 `Consume + 무방출 KeyDown`
Outcome 은 `ultrakey-engine/src/engine.rs:677` 의 `outcome_is_pending`("소비했지만
아무것도 방출하지 않음 ⇒ quick press 판정 대기 중") 휴리스틱에 걸린다 —
`crates/ultrakey-engine/src/lifecycle.rs:178` `quick_press_tick_delay_hint` 가 이
조합을 "여전히 대기 중"으로 해석해, F-16.1/F-16.2 의 autorepeat tick(약 30ms)
마다 quick press 타이머를 재예약한다. **기능 영향은 없다** — `on_tick`
(`engine.rs:774`)은 대기 중인 FSM 슬롯이 없으면 no-op Outcome 을 내고, 타이머는
"다음 발화 시각" 하나를 밀어내는 방식이라 누적되지 않는다(누름 1회당 여분의
런루프 웨이크업 1회 수준). 코드 수정은 하지 않는다 — `engine.rs` 를 건드리는
것은 이 위임의 범위를 넘고, 이 휴리스틱을 "Consume+무방출=pending" 에서
세분화하는 것은 이득 대비 회귀 위험이 크다. 여기 기록해 두는 이유는, 후속
세션이 quick press 타이머를 손볼 때 "왜 F-16 토글 규칙을 누르고 있는 동안
타이머가 재예약되는가"로 시간을 잃지 않게 하기 위함이다.

### 3-5. 기각한 대안 — `SynthEvent` 에 autorepeat 전파 → 플랫폼 `kCGKeyboardEventAutorepeat` 설정

위임이 제시한 대안: `SynthEvent`/`Effect::PushKey` 류에 `autorepeat: bool` 필드를
추가해 원본 이벤트의 autorepeat 를 그대로 실어 보내고, 플랫폼 합성
(`ultrakey-platform::event::SyntheticEvent::keyboard`)이 `kCGKeyboardEventAutorepeat`
를 설정하게 해서 **macOS 자신의 심볼릭 핫키 엔진이 반복을 무시**하게 만드는 안.

**기각 사유**:

1. **영향 범위가 이 결함의 범위를 넘는다.** `SynthEvent`/합성 이벤트 생성 경로는
   F-16 뿐 아니라 `RuleAction::Key`(계층 2/3 프리셋 리매핑 전체 — `Self::push_key_action`,
   `arbitration.rs:1183`)·`Effect::TypeChar`(유니코드 문자 합성)·D-1 커널 리매핑 등
   **모든** 합성 키 이벤트 소비처가 공유한다. autorepeat 전파를 도입하면 이 모든
   경로의 출력 모양이 바뀐다 — 예를 들어 일반 리매핑(`a`→`b`)을 길게 누르고 있을
   때 지금은 매 tick `b` 의 KeyDown 이 나가 macOS 표준 문자 반복(누르고 있으면
   문자가 반복 입력됨)을 재현하는데, autorepeat 플래그를 얹으면 macOS 가 이걸
   "이미 처리된 반복"으로 취급해 **문자 반복 입력 자체가 멈출 위험**이 있다(F-16.3/
   F-16.4 가 지키는 것과 같은 종류의 회귀, 그러나 훨씬 넓은 표면 — 일반 타이핑
   리매핑 전체).
2. **결함의 원인 진단과 어긋난다.** 이 결함은 "F-16.1/F-16.2 의 출력이 토글 핫키인데
   반복 발화된다"는 **좁은 의미론적 문제**다. `SynthEvent` 라는 범용 기구에 새 필드를
   얹어 플랫폼까지 배선하는 것은 위임 계약의 "새 이벤트 탭·새 레이어·새 FSM 금지"에
   직접 저촉하지는 않지만, **범용 데이터 구조를 F-16 전용 의미론으로 오염**시키는
   패턴이다(`rules.rs` 의 기존 주석 D-K5 가 `KoreanRule` 이 `ComboRule` 을 재사용하지
   않는 이유로 든 것과 같은 논리 — "넓히면 범용 기구가 특정 규칙 전용 개념으로
   오염된다").
3. **채택안이 이미 문제를 완전히 닫는다.** §3-4 는 배선 변경 없이(신규 필드 1개 +
   함수 내부 분기 1개) F-16.1/F-16.2 만 정확히 겨냥해서 고친다 — 플랫폼 계층까지
   내려갈 필요가 없다.

이 대안은 P2 가 뒤집을 수 있는 판단 대상으로 남긴다(§5).

---

## 4. 테스트 갱신

- **신규(수정 전 실패 확인)**:
  `issue127_input_box_shift_space_autorepeat_synthesizes_keydown_exactly_once`,
  `issue127_layer3_shift_space_autorepeat_synthesizes_keydown_exactly_once`.
- **개명 + 기대값 정정**: `input_box_f16_1_autorepeat_substitutes_each_tick` →
  `input_box_f16_1_autorepeat_is_consumed_without_re_synthesizing` — "autorepeat 도
  매번 치환된다"(#127 결함 그 자체를 인코딩)에서 "래치가 서 있으면 autorepeat 은
  재발화하지 않는다"로 기대값을 고친다. 문서 주석에 왜 원래 기대값이 틀렸는지
  명시했다.
- **비회귀(무수정 통과 확인)**: `f16_4_autorepeat_key_down_is_substituted_every_time`
  (F-16.4 는 매 tick 재발화가 옳다) · T-118/T-121/#125 시리즈 전체 · `plain_space_up_without_latch_passes`
  등 기존 331개 테스트 전부.
- **신규(P2 리뷰 선택-1 — 대칭 축 고정)**:
  `issue127_layer3_han_eng_autorepeat_synthesizes_keydown_exactly_once`(F-16.2 도
  F-16.1 과 같은 억제가 적용됨 — 세션 중엔 JIS_KANA 가 계층 1 예외로 D2 에 도달
  안 하므로 계층 3 로만 확인) · `f16_3_hanja_autorepeat_key_down_is_substituted_every_time`
  (F-16.3 도 F-16.4 처럼 토글이 아니라 매 tick 재발화 — §3-2 표 4행 전부 테스트로
  고정 완료).

---

## 5. P2 에게 요청하는 판정

1. **방향 적절성** — §3-4(규칙 단위 `is_input_source_toggle` 플래그로 좁힌 소비
   억제) 채택 여부. §3-5(SynthEvent autorepeat 전파)를 P1 은 영향 범위·의미론
   오염을 근거로 기각했다 — 이 판단에 동의하는지, 아니면 두 안을 병행(예: 우선
   §3-4 를 적용하고 §3-5 는 후속 이슈로 분리)할지.
2. **래치 없이 autorepeat 만 도착하는 경계** — §3-4 의 "자기 치유"(래치가 없으면
   최초 down 처럼 재평가) 판단이 안전한지. 특히: 래치가 다른 경로(예: 향후
   `force_reset` 류)로 지워진 직후 도착한 진짜 autorepeat 이 새 토글을 한 번 더
   내보내는 것이 허용 가능한 트레이드오프인지(위임 계약이 "1 press = 정확히 1
   chord" 를 요구하지만, 이 경계는 "래치가 이미 지워진" 비정상 상태이므로 완전한
   보장은 원래도 불가능하다는 게 P1 판단).
3. **F-19 판단**(§3-3) — "F-19 는 이 결함 경로에 들어간 적이 없다"는 결론에
   동의하는지. 동의하면 F-19 쪽 코드 변경은 없음이 맞다.
4. **채택 후보 초안 검증** — §3-4 diff(`crates/ultrakey-core/src/{rules,arbitration}.rs`,
   `crates/ultrakey-korean/src/settings.rs`)를 직접 읽고 실측 검증. 기준선 테스트
   2종(수정 전 FAILED → 수정 후 ok)·개명된 테스트·`f16_4_...every_time` 비회귀·
   `cargo test -p ultrakey-core --lib`(331개) 통과를 재실행해 확인.
5. 가능하면 `ultrakey-review` 서브에이전트/스킬 리뷰도 함께(단, #125 §8-1 이 확인한
   대로 `ultrakey-review` 는 opencode 전용이라 이 P2 판정 자체가 그 리뷰를
   대신한다).

## 6. 금지 승계 (#121·#125)

새 이벤트 탭·새 레이어·새 FSM 금지, `select_input_source` 직접 호출 금지,
#106/#120/#122/#126 테스트·동작 무효화 금지, CHANGELOG 미변경.

## 7. 실기기 검증 절차 — M11 (#121 M0~M9, #125 M10 확장)

| # | 절차 | 기대 |
| :--- | :--- | :--- |
| M11 ⭐ | `pkill -x ultrakey-app; open --env ULTRAKEY_TRACE_TAP=1 -n /Applications/Ultrakey.app` → F-16.1 켬 → 인풋 박스 세션 열고 ⇧+Space 를 **225ms 이상**(자동 반복이 시작될 만큼) 눌렀다 뗀다 → `~/Library/Logs/Ultrakey/ultrakey.log` 의 "탭 계측" 레코드에서 SPACE `autorepeat=true` 도착 수(N) vs `layer=KoreanInput`·`emitted=[KeyDown ...]` 로 기록된 합성 ⌃Space **KeyDown** 방출 수 대조 | 수정 전: 방출 수 = 1+N(반복마다 재발화, 세션 2 재현). 수정 후: 방출 수 = **1**(최초 down 뿐, autorepeat 은 `disposition=Consume, emitted=[]` 으로 기록) — 입력 소스가 208ms 뒤 되돌아가지 않아야 한다 |
| M12 | M11 과 같은 세션에서 F-16.4(₩→백틱, 한국어 IME 활성 필요)를 길게 눌러 문자가 정상적으로 반복 타이핑되는지 확인 — §3-2 가 F-16.3/F-16.4 를 억제 대상에서 제외한 결정의 실기기 확인 | 백틱 문자가 누르고 있는 동안 계속 입력됨(비회귀) |

⭐(P2 리뷰 선택-4) F-16.2(한/영 키→⌃Space)는 JIS 키보드가 있어야 실기기로 직접
확인할 수 있다 — 이번 세션은 JIS 실기기를 보유하지 않아 별도 M 절차를 두지 않는다.
F-16.2 는 F-16.1 과 완전히 동일한 코드 경로(`is_input_source_toggle = true`, 같은
출력 `SPACE+CONTROL`)를 타므로 **M11 이 대표 검증한다** — 코드 수준 대칭 테스트는
`issue127_layer3_han_eng_autorepeat_synthesizes_keydown_exactly_once`(§4)로 이미
고정했다.

이슈 #121 §8 의 M0~M9(전제 확인·후보 E/B/C/D 분기)와 #125 §8-6 의 M10(stale hyper
판정)은 그대로 유효하다 — 이번 수정은 그 사슬과 독립된 별개 결함(자동 반복 재발화)을
닫는다. 세션 1(첫 세션, 전환 알림 0회)의 매커니즘은 여전히 `(미확정)` — 별도 이슈
후보(검색 바 창 최초 생성 직후 키 윈도우 레이스)로 남긴다.

---

## 8. P2(opus) 판정 요약 + P3 확정

### 8-1. P2 판정 — 조건부 통과

**동의한 것 전부**: §3-4(규칙 단위 `is_input_source_toggle` 플래그) 채택·§3-5
(`SynthEvent` autorepeat 전파) 기각·§3-4 "래치 없이 autorepeat" 경계의 자기 치유
처리(래치를 지우는 경로는 `clear_korean_latch`(KeyUp 소비)와 `force_reset`뿐이라,
실제 경계는 "절전/잠금 진입 중 물리 키를 누르고 있는" 극단 상황으로 좁다는 것을
코드로 재확인) · §3-3 F-19 무변경 결론(`evaluate_language_rules` 의 `AloneTap`
배제·`language_trigger_met` 의 `unreachable!()`·`quickpress.rs` 의 `on_key_down`
이 `HoldConfirmed` 에서 이벤트를 다시 안 내는 것을 코드로 독립 재확인) · 위임
계약 준수(새 기구·직접 호출·CHANGELOG 없음) · 기존 테스트 기대값 반전이 "#122
테스트 무효화 금지" 위반이 아니라는 판정(원래 기대값이 결함 그 자체를 기술하고
있었고, 검색어 공백 차단·KeyUp 단일 방출 단언은 그대로 유지·강화됐다는 근거).

**P2 자체 재검증**: `cargo test -p ultrakey-core --lib` 331/331 · 신규 4종 개별
통과 · clippy 0 경고를 독립 재실행했고, ⭐ 새 분기를 `if false &&` 로 무력화한 뒤
재실행해 기준선 실패(`left: 4, right: 1` 등)가 P1 §2 기록과 정확히 일치함을
재현으로 확인(검증 후 원상 복원 확인).

**반영 필수 1건**: `Consume + 무방출 KeyDown` 이 `ultrakey-engine::engine::outcome_is_pending`
휴리스틱에 걸려 autorepeat tick 마다 quick press 타이머를 재예약하는 부작용(기능
영향 없음, 여분 웨이크업 수준) — 코드 수정은 요구하지 않되 계획 문서에 기록 요구.
**반영**: §3-4 끝에 "부작용 기록" 문단 추가.

**선택 지적 4건**: ① F-16.2/F-16.3 대칭 테스트 부재 ② `latch.id == rule.id` 미확인
(교차 오염 방어) ③ "게이트 재평가 안 함" 서술이 트리거 조건(매 tick 재평가)과
나란히 읽히면 오해 소지 ④ M11/M12 표에 F-16.2 항목 없음.

### 8-2. P3 확정 (이 세션)

**필수 1건 + 선택 4건 전부 반영**:

1. §3-4 부작용 문단 추가(위 §7 절차 앞).
2. §3-4 에 "게이트 vs 트리거 조건" 한정어 문단 추가.
3. §7 에 F-16.2 실기기 대체 검증 근거(M11 이 대표) 문단 추가.
4. 테스트 2종 추가: `issue127_layer3_han_eng_autorepeat_synthesizes_keydown_exactly_once`
   (F-16.2) · `f16_3_hanja_autorepeat_key_down_is_substituted_every_time`(F-16.3) —
   §3-2 표의 4행 전부가 이제 테스트로 고정됐다.
5. `evaluate_korean_rules` 의 새 분기에 `.filter(|l| l.id == rule.id)` 추가(방어
   강화, 실동작 변경 없음 — 지금은 trigger_key 유일성으로 항상 자기 자신의 래치만
   발견됨).

미반영 지적 없음. `cargo test -p ultrakey-core --lib` → **333 passed / 0 failed**
(신규 2종 추가 반영), `cargo clippy -p ultrakey-core -p ultrakey-korean --all-targets
-- -D warnings` → 0 경고, 재확인 완료.
