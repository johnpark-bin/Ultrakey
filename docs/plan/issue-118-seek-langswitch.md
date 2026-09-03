# 이슈 #118 — Seek 입력창에서 언어 전환 키 미동작 (F-19 arbitration 변경 회귀 추정) — 실측 계획 + 작업 분해

> **성격**: `docs/plan/` 은 구현 위임 전 **계획 문서**를 두는 자리다. 이 문서는 중급(실측·초안)이
> 진행한 **실측 결과 + 계획 초안**이며, 확정은 상급(`ultrakey-review`)의 리뷰 후 이루어진다.
> 이번 위임은 "회귀 수정"이지만 **첫 단계가 회귀 실측**이다 — 원인이 확정되지 않으면
> 지어내지 않고 `(미확정)` 으로 남긴다(위임 계약).
>
> **성격 2**: 이슈 #101(닫힘, PR #106)의 **같은 증상 재발** (Ver.2) 위임이다. #106 의 실측 방법
> (테스트 재현 → diff 대조 → 소비 경로 후보 검증 → 원인 1줄)을 그대로 이번에 적용했다.
>
> ⭐ **상급 리뷰 반영 (2026-09-03, `ultrakey-review`)**: **조건부 통과** — 반영 항목 6건
> (G1~G6), 반영 안 해도 되는 사항 4건(N1~N4) 중 N2(기각 대안 ⑥)·N3(FSM Idle 단언)은
> 채택, N1(슬롯 포화)은 후보로 기록. 전체 반영 내역은 §10.

---

## 0. 요약 — 실측 결론 (리뷰 확정 전 초안)

**① #116(F-19) 은 코드·테스트 수준에서 인풋 박스 언어 전환 경로를 건드리지 않았다 — 무죄.**

**② 회귀의 원인 지점은 코드·테스트 수준에서 확정 불가 — `(미확정)`.** 단위 테스트 42개 전부
통과(⌃Space D1 Pass 포함), #116 diff 는 세션 게이트·인풋 박스 분기·이벤트 전달 경로
미변경, F-19 규칙 트리거 테이블에 SPACE/SHIFT 자체가 없다.

**③ 그래서 "수정"의 내용은**: ㉠ **F-19 규칙이 전부 켜진 상태에서도 인풋 박스 전환 키가
안전함을 고정하는 회귀 재발 방지 테스트 신규** (기존 테스트는 `EngineConfig::default()` 로
F-19 미로드 상태만 고정 — 커버리지 갭), ㉡ 실측 문서(본 문서) + 명세에 결정 기록,
㉢ PR 본문에 **실기기 검증 절차**(#106 M1~M8 재선언 + 기대 로그)를 남겨 "반복 리포트"를
막는다. **새 기구·새 레이어·새 FSM 금지** — 기존 구조만.

**④ 실기기 확정이 필요한 유일한 지점** (사용자/검증자가 실행): 웹뷰 키 윈도우 상태에서
시스템 입력 소스 hotkey(⌃Space)가 실제로 발화하는가. 이것만은 이 세션(CLI·무권한)에서
측정 불가 — 절차와 기대값을 §8 M1~M8 로 남긴다.

---

## 1. 배경 — 증상과 위임 범위

### 증상 (사용자 보고, 이슈 #118)

- Seek 를 단축키로 띄운 **텍스트 입력 창(인풋 박스 모드)** 안에서 `Shift+Space`·`Ctrl+Space`
  언어 전환 키가 **동작하지 않음** (영어↔한국어 양방향 모두).
- 창을 띄우기 **전에** 한국어로 전환해 두면 한국어 입력은 정상 — 전환 자체만 죽어 있다.
- **범위 결정(사용자)**: 인풋 박스 모드만 수정. 세션 밖·일반 타이핑은 건드리지 않는다.

### 위임 계약 (herdr-dev-delegate)

- 브랜치 `fix/issue-118-seek-langswitch` (생성됨) · 워크트리 `/Users/daphne/.herdr/worktrees/Ultrakey/fix-issue-118-seek-langswitch`
- 산출물: 실측 기록(`docs/plan/issue-118-…md`) · 수정 · 회귀 재발 방지 테스트 · 명세 갱신(필요 시) · PR `Closes #118`
- ⛔ 인풋 박스 밖 동작 변경 금지 · 새 이벤트 탭/레이어/FSM 금지 · `select_input_source` 직접 호출 금지 · #117 라벨 누락 별도 위임
- ⛔ 원인 미확정 시 지어내지 말고 `(미확정)` 기록 (본 문서 §3·§8 에 반영)

### 선례 — 이슈 #101 → PR #106 (닫힘)

- #101(2026-09-02): 이번과 **정확히 같은 증상**. PR #106(`29fef71`, 커밋 `046169f`)이 수정·머지.
- #106 원인 실측 3경로:
  - **⌃Space**: `is_input_box_pass_key` 의 ⌘/⌃ 소비 판정이 시스템 단축키를 삼킴 → 수정 D1:
    `is_input_source_switch_shortcut`(SPACE + ⌃ + ⌘ 미실림)를 인풋 박스 모드에서 **원본 통과**.
  - **⇧+Space**(F-16.1): F-16 규칙은 계층 3 → 세션 중 계층 1 short-circuit 으로 발화 안 함 →
    수정 D2: SPACE 키에 한해 계층 1 자리에서 `evaluate_korean_rules` **재평가**(계층 1 분기
    `arbitration.rs:522-530`, 평가 순서 D2 → D1 = 리뷰 #3-a).
  - **한/영 키(0x68)**: #76 예외로 통과 — 회귀 없음.
- ⚠️ **PR #106 의 실기기 검증(M1~M8)** 이 PR 본문에 절차로 선언됐지만 **결과 기록이 없다**
  (§2 F6) — "동작했었다"는 실측 근거가 없다.

---

## 2. 실측 결과 — 확정된 사실 (이번 세션에서 측정)

| # | 사실 | 근거 (파일·행·명령, 등급) |
| :--- | :--- | :--- |
| F1 | main(`f9782d7` = #116 머지)에서 인풋 박스 관련 **테스트 42개 전부 통과** — ⌃Space D1 Pass(`input_box_passes_system_input_source_shortcut`), ⇧+Space D2 치환(`input_box_f16_1_shift_space_substitutes_control_pair`), ⌃⌥Space·⌃⇧Space·토글 가드·영어 회귀 포함 | `cargo test -p ultrakey-core --lib input_box` — 42 passed / 0 failed (실측) |
| F2 | #116(`54b1b9f`)의 `arbitration.rs` 변경은 ①`GateSnapshot` 필드 4개 추가 ②`Layer::LanguageInput` 신설 ③`is_tracked` 에 `has_language_alone_tap` 병합(:566) ④계층 3 `evaluate_language_rules` 호출(:618) ⑤quick press FSM AloneTap 발화 2곳(:821-830·857-866) ⑥새 헬퍼 7종. **세션 게이트(:483-548)·인풋 박스 D1/D2(:516-544)·이벤트 전달 경로는 미변경** | `git show 54b1b9f -- crates/ultrakey-core/src/arbitration.rs` (실측) |
| F3 | #116 의 엔진 변경은 게이트 스냅샷 필드 추가뿐(`engine.rs` +6) — 탭 디스패치·Pass 전달 경로 미변경. `korean.rs` 변경은 `classify_input_source_languages_for` 일반화(추가 함수)뿐, F-16 판정 무영향. `ultrakey-korean/settings.rs` +60 은 F-19.1/F-19.2 규칙 추가 — **F-16.1 정의(`to_rules`)는 무수정** | `git show 54b1b9f -- …` 4개 파일 diff (실측) |
| F4 | F-19 언어 규칙(Language 17~23)의 트리거 키 집합: `CAPS_LOCK`·`RIGHT_COMMAND`·`LEFT_COMMAND`·`JIS_YEN`·`ANSI_BACKSLASH`·`ANSI_2/6/7/8/9/0`·`ANSI_MINUS`·`ANSI_EQUAL`·`ANSI_LEFT/RIGHT_BRACKET`·`ANSI_SEMICOLON`·`ANSI_QUOTE` — **SPACE 도 SHIFT 도 없다** → `evaluate_language_rules`(KeyDown `find`: `language_trigger_key(r) == ev.keycode`)가 SPACE 이벤트를 소비할 수 있는 조합이 존재하지 않음 | `ultrakey-language-presets/src/settings.rs` (실측) · `arbitration.rs:1376-1383` |
| F5 | `has_language_alone_tap(SPACE) == false` — AloneTap 등록 키는 `CAPS_LOCK`·`LEFT_COMMAND`·`RIGHT_COMMAND` 3종뿐. 또한 `is_tracked`(:564-566)는 **세션 게이트 뒤**라 세션 중 도달 불가 — AloneTap FSM 이 세션 중 ⌃Space/⇧+Space 를 볼 수 없다 | `keystate.rs:329-362` · `arbitration.rs:559-569` (실측) |
| F6 | PR #106 본문에 **실기기 검증 M1~M8 절차가 문서화돼 있으나 결과 기록이 없다** (M1 = 인풋 박스 + ⌃Space → ABC 전환, M7 = `ULTRAKEY_TRACE_TAP=1` → `layer=SeekSession disposition=Pass`). 이슈 #101·#118 모두 코멘트 0. → "#106 이후 실제로 동작했다"는 **문서 근거 없음** | `gh pr view 106` (실측) |
| F7 | **중간 PR 전수 대조(리뷰 G1 보강)**: $#106$ 머지($29fef71$)~$f9782d7$ 사이 arbitration·웹뷰 경로 변경 — ①**#107($b45f558$)** 이 인풋 박스 키 윈도우인 `apps/ultrakey-app/ui/overlay-searchbar.html` 을 변경했으나 diff 는 카운터 문구 분기(`payload.detecting` → "찾는 중…")뿐, **키 이벤트 핸들러 추가 없음** ②**#108($4df236d$)** `normalize_kind` 수정($arbitration.rs:718-758$, $arbitrate$ 진입부 :394 — 세션 게이트 **앞**)은 `FlagsChanged` 를 KeyDown/KeyUp 으로 환원할 때의 family 비트 판정만 바꿈 — SPACE KeyDown/KeyUp(:719-721)은 무변경 통과 ③**#110($3aab238$)** 이중 중재 분기 `d1_bypassed`(:386-390, 조건 :430-434)는 `caps_lock_alias Some + FlagsChanged + CAPS_LOCK` 에 한정 — SPACE 이벤트는 구조적으로 진입 불가 ④**#112·#114** 는 arbitration 미변경(#114 는 crates 미변경, pure docs) | `git log 29fef71..f9782d7 -- crates/ultrakey-core/src/arbitration.rs …` · 각 커밋 diff (실측) |
| F8 | 엔진 Pass 전달: `Disposition::Pass → TapAction::Pass → 탭 콜백이 원본 이벤트 포인터 반환` — 이벤트가 **정상 디스패치**(시스템 hotkey·앱 입력)를 계속 탄다. `Consume` 은 `null_mut()`(삭제). D1 의 `Outcome::pass` 가 시스템 입력 소스 hotkey 에 도달하는 경로는 코드로 유효 | `engine.rs:750-757` · `event_tap.rs:354-355` (실측) |
| F9 | **현재 HEAD 에서 웹뷰 키 이벤트 핸들러 부재** — `overlay-searchbar.html` 에 keydown/keyup 리스너가 없다(#106 사실⑫의 HEAD 재확인). focus·compositionstart/end·input·click 리스너뿐(:307-437) → 통과 키가 게이트→시스템 경로만 타고 웹뷰가 가로챌 지점이 없음 | `overlay-searchbar.html` (실측 — 리뷰 G1) |
| F10 | **래치 배열 물리 분리**: D2 가 재사용하는 `evaluate_korean_rules` 는 `cfg.rules.korean_rules`·`korean_latches` **만** 읽는다($arbitration.rs:1210, 1225-1229, 1257$) — `language_rules`·`language_latches` 참조가 구조적으로 없다. 두 래치 배열은 별도 슬롯($keystate.rs:73-77$)이라 F-16 래치와 F-19 래치가 서로 덮어쓰거나 소비하지 않는다 | (실측 — 리뷰 G4) |
| F11 | **F-19 AloneTap 은 `active_synth_flags` 에 무영향**: AloneTap 슬롯은 `rule_flags: EventFlags::NONE` 으로 등록되어($keystate.rs:347-354$) HoldConfirmed 가 돼도 `active_synth_flags()`($keystate.rs:383-391$)에 비트를 보태지 않는다 → D2 의 발화 조건 3(hyper 활성 금지, $arbitration.rs:1244$)을 흔들 수 없다 | (실측 — 리뷰 G4) |

### 사실 해석

- **F1+F2+F4+F5+F8 을 합치면**: #116 이 인풋 박스 ⌃Space/⇧+Space 경로에 개입할 수 있는
  코드 지점이 존재하지 않는다. "F-19 arbitration 변경 회귀"라는 이슈 추정은 **코드·테스트
  수준에서 기각**된다.
- **F6 은 함정**: "회귀 재발"은 사용자가 **#106 이후 동작했었다**는 사실을 전제로 한다. 그러나
  그 전제의 문서 근거가 없다 — #118 은 "#106 수정 자체가 실기기에서 미검증·동작 안 한 상태의
  재보고"일 가능성과 "실제 회귀"일 가능성이 동률이다.

---

## 3. 원인 판정 (초안 — 리뷰 확정 전)

### 확정된 것

> **"#116 이 원인이다"는 코드·테스트 수준 근거가 없다.** 근거: F1~F5·F8.

### 미확정 (실기기 전용 — 이 환경에서 측정 불가)

> **"왜 실기기에서는 언어 전환 키가 안 되는가"의 최종 매커니즘은 `(미확정)`.**

후보 (가능성 순 정렬, 전부 실기기 단계):

| 후보 | 내용 | 가능성 근거 |
| :--- | :--- | :--- |
| (A) #106 수정의 실기기 미검증 지속 | D1 pass + D2 치환이 **시스템에 도달은 하나**, 이후 **웹뷰 키 윈도우 상태에서 입력 소스 hotkey 발화/IME 반영**은 #106 이 이미 "실기기 검증 필요"로 남겼던 범위(M1/M2. 미기록) | F6 · #106 플랜: "남은 불확실성은 우리 앱이 활성·웹뷰가 키 윈도우인 상태에서도 동일한가 하나뿐" |
| (B) 환경·설정 의존 | 시스템 입력 소스가 2개 미만이거나 "Select previous input source"(⌃Space) 단축키가 미등록 | manual-verification.md §9 #3 `(미확정)` (3 소스 이상 동작) |
| (C) #116 간접 회귀 | 사용자가 **F-19.1/2/4/7(캡스락·우⌘ AloneTap)을 켠 뒤** 캡스락 D-1 커널 매핑 설치·`resolve_caps_lock_alias` 영향으로 세션 중 무언가가 바뀜 | 배제 근거를 둘로 나눈다(리뷰 G6): ① 규칙·FSM 교차 없음 = F4·F5·F10·F11. ② **커널 매핑 교차 없음** = D-1 매핑은 캡스락 키코드(0x39→F18)만 바꾸며 ⌃Space/⇧+Space 이벤트는 캡스락 키코드를 포함하지 않는다. `d1_bypassed` 이중 중재도 `CAPS_LOCK + FlagsChanged` 한정($arbitration.rs:430-434$)이라 SPACE 이벤트는 구조적으로 진입 불가(F7-③) |

**문서 원칙 (위임 계약)**: 이 `(미확정)` 판정을 PR 본문·§8 에 그대로 남긴다. 지어낸 원인을
"수정"하지 않는다.

---

## 4. 회귀 재발 방지 — 커버리지 갭과 신규 테스트 설계

### 갭

기존 인풋 박스 테스트(`input_box_gates`, `arbitration.rs:2241`)는 `EngineConfig::default()`
를 쓴다 — **`language_rules` 가 빈 상태**(F-19 전부 꺼짐)만 고정한다. F-19 프리셋이 켜진
상태(캡스락·우⌘·⌘ AloneTap + JIS 규칙 로드)에서도 인풋 박스 전환 키가 안전하다는 단언은
없다. F4/F5 의 "안전함"을 **테스트로 고정**하는 것이 이번 수정의 핵심 산출물이다.

### 신규 테스트 (T-118 시리즈, `arbitration.rs` `input_box_*` 계열 위치)

셋업: `f19_all_presets_config()` — F-19.1~7 규칙을 전부 로드한 `EngineConfig` (`KoreanSettings::
to_language_rules()` + `JapaneseSettings::to_language_rules()` + `ChineseSettings::
to_language_rules()` 를 합친 것과 동형). ⛔ 새 판정 로직 없음 — 오직 "이미 존재하는 규칙
테이블을 로드한 상태"에서의 단언만.

| # | 테스트 | 기대 |
| :--- | :--- | :--- |
| T-118-1 | **F-19 전체 규칙 로드** + 인풋 박스 + **⌃Space·⌃⌥Space·⌃⇧Space 루프**(D1 계열 전부) KeyDown/KeyUp | 전부 **Pass**(SeekSession · effects 없음 — D1. F-19 래치·FSM 소비 없음) — 기존 T1/T2/T2-b 가 F-19 미로드 상태만 고정하므로 확장 필요(리뷰 G3) |
| T-118-2 | 같은 cfg + F-16.1 켬 + 인풋 박스 + **⇧+Space** down/up | **Consume + ⌃Space 치환 짝**(D2 — F-19 가 가로채지 않음) |
| T-118-2b | 같은 cfg + **F-16.1 꺼짐(기본 구성)** + 인풋 박스 + ⇧+Space | **Pass**(검색어 공백 — F-19 가 SPACE 를 소비하지 않음의 직접 고정). ⭐ 사용자 기본 구성(리뷰 G3 — 빠지면 보고 증상과 가장 가까운 구성이 미고정) |
| T-118-3 | 같은 cfg + **`is_jis = JisState::Jis` 게이트**(⛔ 반드시 명시 — `language_jis_met` 는 Unknown 에서 fail-closed, $arbitration.rs:1304-1312$) + **F-19.6(JIS→US)** — JIS_YEN + ⇧ | 세션 밖: **LanguageInput Consume 치환**(F-19 정상 발화). 세션 중(인풋 박스): **Consume + SeekKey** — JIS_YEN 은 `is_printable_keycode` 부재($seek_input_box.rs:112-162$)라 계층 1 소비, F-19 평가 자체가 안 됨 |
| T-118-4 | 같은 cfg + **캡스락 AloneTap**(F-19.1) + 인풋 박스 + 캡스락 down/up | 세션 중: **Consume + SeekKey**, AloneTap FSM 발화 없음(세션 게이트가 `is_tracked` 앞 — 기존 세션 동작 고정) + FSM `Idle` 유지 단언(리뷰 N3 채택 — $arb.state$ 기계 슬롯 상태 확인). 캡스락은 세션 밖 동작과 무관함을 추가 고정 |
| T-118-5 | 같은 cfg + **세션 밖** ⌃Space | **Pass**(Passthrough — 기존 행동 고정, F-19 간섭 없음) |
| T-118-6 | 같은 cfg + F-16.1 켬 + **세션 밖** ⇧+Space | **Consume + ⌃Space 치환**(계층 3 F-16.1 정상 발화 — F-19 공존 하 세션 밖 비회귀 고정, 리뷰 G3) |

---

## 5. 명세·문서 갱신 (결정 기록 — 기존 기록은 남긴다)

| 문서 | 갱신 내용 |
| :--- | :--- |
| `docs/plan/issue-118-seek-langswitch.md` (본 문서) | 실측 F1~F8 · 원인 판정(미확정) · T-118 설계 · 기각 대안 |
| `docs/spec/language-presets.md` | ⭐ 결정 기록을 **기존 기록 옆에** 추가: "F-19 규칙 트리거 집합은 SPACE/SHIFT 를 포함하지 않으며, 인풋 박스 모드 D1(⌃Space 원본 통과)·D2(F-16.1 ⇧+Space 재평가) 경로와 교차하지 않음 — #118 실측으로 확인. 단 실기기 매커니즘은 (미확정, #118 §8 M1~M8)" |
| `docs/spec/seek-activation-and-session.md` §3.1 | 인풋 박스 입력 소스 전환 3경로 옆에 결정을 붙인다: "#118 실측 — #116 이 3경로를 변경하지 않았음(테스트 42개 + diff 대조). 실기기 재현 시 M1~M8 기록" |
| 수동 검증 문서 (`manual-verification.md` 는 **건드리지 않는다** — #106 이미 절차 보유) | — |

---

## 6. 수용 기준 (⭐ P2 확정 — 상급 리뷰 확정판)

- [ ] **실측 기록 확정**: 본 문서가 리뷰 반영 후 확정 — F1~F11 + 원인 판정("#116 은 코드·테스트 수준 무죄" / 실기기 매커니즘 `(미확정)`)
- [ ] **프로덕션 코드 변경 제로**: 이 PR 의 diff 는 **테스트 코드(`arbitration.rs` `#[cfg(test)]` 모듈)와 문서(`docs/plan/`, `docs/spec/`)로만** 구성된다 — 런타임 동작 변경 없음. 이것이 범위 준수("인풋 박스 모드만")의 최강 보증이다
- [ ] **T-118 시리즈 7종**(T-118-1~6 + 2b) — 전부 `f19_all_presets_config()`(F-19.1~7 전체 로드) 상태에서 §4 표의 기대값 달성
- [ ] 기존 테스트 무효화 없음 — `cargo test --workspace` 전체 통과(input_box 42개 + 신규 포함)
- [ ] F-19 전체 테스트 통과(`ultrakey-language-presets` 크레이트) — 라벨·규칙 7종·AloneTap·충돌
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 0 경고
- [ ] 명세 결정 기록: `language-presets.md`·`seek-activation-and-session.md` §3.1 에 기존 기록 보존 + 결정 추가. ⛔ `korean-input.md` 의 F-16.2 관련 확정 문구(#106 리뷰 #6-a 보존 지시)·#106 이 개정한 "계층 1 우선" 메타 문구를 건드리지 않는다. `manual-verification.md` **미변경**
- [ ] **라벨 미변경** — 이슈 라벨 추가·삭제 금지(#117 라벨 누락은 별도 위임)
- [ ] PR 본문: 실측 근거(F1~F11) · 결정 · 기각 대안 · **`(미확정)` 명시** · 실기기 절차 M1~M9 · 이슈 체크리스트 항목별 실제 충족 내역 매핑("동작 복구" 항목은 "코드 수준 무죄 확인 + 테스트 고정 + 절차 이관으로 충족"으로 명시) · **"이 PR 은 런타임 동작을 바꾸지 않으며, 실기기 증상이 지속되면 M1~M8 결과가 후속 이슈의 출발점이다"** 문구 · 커밋/제목 한국어+Gitmoji · 끝 `Closes #118`

---

## 7. 작업 분해

1. (완료) 실측 1차 — 이슈·#106 diff·#116 diff·소스·테스트 상태 확인 (§2 F1~F8)
2. (완료) 본 계획 초안 작성 — P2 리뷰 대기
3. P2 — `ultrakey-review` 리뷰 → 반영(반영 안 한 지적은 사유 기록) → §6 수용 기준 확정
4. T-118 테스트 구현 (`f19_all_presets_config` + T-118-1~5)
5. 명세·계획 문서 결정 기록 반영
6. `cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings` 통과 확인
7. 커밋(한국어+Gitmoji) · 브랜치 푸시 · PR 생성 (`Closes #118` + 실측 근거 + (미확정) + M1~M8)
8. PR 감시(`watch` → 성공) / 실패 시 `gh pr view` 직접 판정 → 정리 직전 예고 → 자기 정리 → `herdr workspace close w24`

---

## 8. 실기기 검증 절차 (사용자/검증자용 — PR 본문에도 동일 게시)

| # | 절차 | 기대 | #106 대비 |
| :--- | :--- | :--- | :--- |
| M1 | 검색 언어 `English + 한국어`(인풋 박스) → 세션 중 **⌃Space** | 입력 소스 ABC 전환, 이후 영어 입력 | 동일 |
| M2 | 같은 세션 · 영어 상태 → **⌃Space** | 한글로 재전환, 조합 + 검색 동작 | 동일 |
| M3 | F-16.1 켬 · 세션 중 **⇧+Space** | 입력 소스 전환, 스페이스가 검색어에 안 들어감 | 동일 |
| M4 | F-16.1 꺼짐(기본) · 세션 중 ⇧+Space | 검색어 공백 입력(기존 동작) | 동일 |
| M5 | 영어 단일 세션 · ⌃Space·⇧+Space | 영향 없음(회귀 없음) | 동일 |
| M6 | (106키 키보드 보유 시) 세션 중 한/영 키 | #76 예외가 키 윈도우에서 동작 여부 | 동일 |
| M7 | `ULTRAKEY_TRACE_TAP=1` · 세션 중 ⌃Space | `layer=SeekSession disposition=Pass` 기록 (F8 경로) | 동일 |
| M8 | 한글 조합 중 전환 → 계속 입력 | `compositionend` flush — composing stuck 없음 | 동일 |
| M9 | ⭐ **신규 — F-19.1(캡스락 탭=한/영) 켬 + 인풋 박스** | 캡스락 탭이 인풋 박스 안에서는 **금지**(세션 컨트롤), 밖에서는 전환 — F-19 세션 간섭 없음 | 신규 |

> ⚠️ M1/M2 결과가 **안 나오는 경우**(전환 미발생): "시스템 hotkey 발화 자체"가 문제일 수 있다
> (우리 코드 경로 밖). 그때 수집 자료: `ULTRAKEY_TRACE_TAP=1` 로그(⌃Space 가 Pass 로 찍히는가) +
> 시스템 설정 ⌃Space 단축키 등록 여부.

---

## 9. 기각한 대안

| 대안 | 기각 사유 |
| :--- | :--- |
| ① #116 추가분(`evaluate_language_rules`)을 인풋 박스 분기에서도 재평가/가드 | 원인이 #116 코드로 확정되지 않았다 — 없는 회귀를 고치려 구조를 더하는 것. 새 기구 금지에도 위배 |
| ② D1/D2 재배치(순서 변경) | #106 리뷰 #3-a 가 잠근 D2→D1 순서 — 래치 잔존 엣지가 그 순서로 해소됨. 테스트가 이미 고정(T-②·T-⑤) |
| ③ `is_input_source_switch_shortcut` 판정 확대(⇧ 허용 등) | #106 이 잠근 판정. 테스트 고정 완료. 확대할 근거 없음 |
| ④ Pass 를 합성·재주입으로 바꾸기 | F8 경로가 이미 시스템 hotkey 까지 도달하는 유일한 원본 경로 — 합성은 마커·루프 위험 |
| ⑤ "코드 수정 제로"로 그냥 닫기 | T-118 커버리지 갭(F-19 로드 상태 미고정)이 남는다 — 재발 방지 테스트는 가치가 있다 |
| ⑥ 실기기 검증(M1~M8)을 **먼저** 돌리고 PR 없이 닫기 | 이 세션은 실기기 실행 불가(CLI·무권한·GUI 앱 미기동) — 절차는 **PR 이 검증자에게 전달하는 수단**이며, 코드 수준 재발 방지 테스트는 실기기 결과와 무관하게 독립 가치가 있다(리뷰 N2 채택) |

---

## 10. 상급 리뷰 반영 내역

**리뷰 (`ultrakey-review`, Qwen3.8 Max): 제목 없음 — 조건부 통과.** 핵심 실측(F1~F8) 전부
재검증되어 사실로 확인("42 passed / 0 failed 재실행", diff 대조, 규칙 테이블 리드, 웹뷰
핸들러 grep).

| 구분 | 항목 | 반영 |
| :--- | :--- | :--- |
| G1 | #106~#116 사이 중간 PR 전수 대조(#107 웹뷰·#108 normalize_kind·#110 d1_bypassed) + 웹뷰 무핸들러 사실 | **반영** — F7 확장·F9 신설(§2) |
| G2 | T-118-3 `is_jis` 게이트 명시 + 오타 | **반영** — §4 |
| G3 | F-16.1 꺼짐(기본 구성) T-118-2b + D1 계열 루프 확장 + 세션 밖 ⇧+Space T-118-6 | **반영** — §4 |
| G4 | 래치 물리 분리·AloneTap `rule_flags NONE` 무영향 코드 근거 | **반영** — F10·F11 신설(§2) |
| G5 | 수용 기준 확정안(프로덕션 제로·라벨·인박스 밖·증상 지속 안내·테스트 목록) | **반영** — §6 전체 교체 |
| G6 | 후보 (C) 배제 근거 정정(커널 매핑 교차 = F4/F5 가 아님) | **반영** — §3 (C) 행 |
| N1 | MAX_TRACKED_KEYS=8 슬롯 포화 시 F-19 먼저 탈락(등록 순서) | **미반영(기록)** — 인풋 박스 경로와 무관(#118 증상 벡터 아님). 후속 후보: F-19 AloneTap 3종 동시 활성 시 슬롯 보호 확인 |
| N2 | 기각 대안 ⑥ 추가 | **채택** — §9 |
| N3 | T-118-4 FSM Idle 단언 | **채택** — §4 T-118-4 |
| N4 | F8 엔진 행 인용 미재리드 — 계획 인용 수용 | **수용** — §2 F8 유지 |