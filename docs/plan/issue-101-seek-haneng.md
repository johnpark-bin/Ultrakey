# 이슈 #101 — Seek 입력창에서 한/영 전환 키가 동작하지 않는다 (인풋 소스 전환 회귀) — 설계 초안 + 작업 분해

> **성격**: `docs/plan/` 은 구현 위임 전 **계획 문서**를 두는 자리다. 이 문서는 중급(계획 초안)이 작성한 **초안**이며, 확정은 상급(호출 세션·`ultrakey-review`)의 리뷰 후 이루어진다. 리뷰가 끝나면 "결정 목록(D1~D4)"이 정본이 되고, §6 의 명세 갱신 초안이 `docs/spec/` 에 반영된 뒤 구현이 진행된다.
>
> **성격 2**: 이슈 #76(한/영 키 통과)·#93(인풋 박스 모드)이 만든 구조의 **회귀 수정**이다. 새 기능이 아니라 **누락된 통과 경로 복구** + **세션 중 F-16.1 발화 예외**다.
>
> ⭐ **상급 리뷰 반영 (2026-09-03, `ultrakey-review`)**: 조건부 통과 — 반영 항목 7건(#1~#7). **핵심 교정: D2 를 D1 보다 앞에 배치**(#3-a — 래치 잔존 엣지 제거). 전체 반영 내역은 §9.

---

## 1. 요약

**증상 (사용자 피드백 2026-09-02)**: 검색 언어를 영어+한글 모드(인풋 박스 모드)로 설정하면, 입력창 안에서 **⌃Space 도 ⇧+Space 도 입력 소스 전환이 동작하지 않는다**. 단, **전환된 상태에서의 입력은 정상** — 한글 상태로 열면 한글, 영어 상태로 열면 영어가 그대로 입력된다. 즉 **인풋 소스 전환 단축키만 세션 게이트가 삼키거나 우회시키고 있고, 전환 그 자체가 웹뷰/시스템에 전달되지 않는다**.

**원인 실측(코드, 3경로 대조)**:

| 경로 | 인풋 박스 세션 중 실제 동작 | 원인 |
| :--- | :--- | :--- |
| **(a) ⌃Space** (macOS 기본 입력 소스 전환 단축키) | ❌ **Consume** → 이벤트 소멸 | `is_input_box_pass_key`(`seek_input_box.rs:37-63`)가 ⌘/⌃ 조합을 전부 소비 판정(⌘Q·앱 단축키 보호, #93). ⌃Space 는 CONTROL 을 실어 **시스템 입력 소스 전환 단축키임에도 소비**된다 — 시스템 단축키가 발화할 기회 자체가 없다 |
| **(b) ⇧+Space** (F-16.1, `korean.shiftSpaceSwitchesInputSource`) | ❌ **Pass 되지만 무동작** | F-16 규칙은 **계층 3**(`evaluate_korean_rules`)이라 세션 중 계층 1 이 short-circuit → 발화하지 않는다(설계 — korean-input.md §3.2). 인풋 박스 모드에서는 ⇧+Space 가 프린터블(`SPACE`)로 **통과**되어 웹뷰 `<input>` 에 도달하지만, 웹뷰는 ⇧+Space 로 입력 소스를 바꾸지 않는다 — 스페이스가 입력되거나 무시될 뿐 |
| **(c) 한/영 키(`0x68`)** | ✅ 통과(변화 없음) | 이슈 #76 예외(`arbitration.rs:370-372`)가 인풋 박스 분기보다 **앞**에 있어 계속 원본 통과. 회귀 없음. ⚠️ 웹뷰가 키 윈도우일 때 0x68 이 실제 입력 소스를 바꾸는지는 실기기 검증 필요(§8) |

**결정 요지**:

- **D1 — 시스템 입력 소스 전환 단축키(⌃Space·⌃⌥Space)를 인풋 박스 모드에서 원본 통과**한다. `is_input_box_pass_key` 의 ⌘/⌃ 소비 판정 **앞**에서 가른다. 단, 설정된 세션 토글 단축키와 일치하면 예외(재입력 토글 = 세션 닫기 유지, 기존 가드 재사용). ⌘Space(Spotlight)·기타 ⌘⌃ 조합은 **계속 소비**(기존 보호 유지).
- **D2 — F-16.1(⇧+Space)을 인풋 박스 모드 세션 중 발화**시킨다. SPACE 키에 한해 계층 3 의 `evaluate_korean_rules` 를 **같은 자리에서 재평가**(규칙·래치·조건 전부 재사용 — 중복 로직 없음). F-16.1 이 꺼져 있으면(기본) ⇧+Space 는 종전대로 통과(검색어 공백). ⭐ **D2 를 D1 보다 앞에 평가한다**(상급 리뷰 #3-a — 래치 잔존 엣지 제거).
- **D3 — 한/영 키(0x68)**: 코드 변경 없음(이미 통과). 실기기 검증 항목으로만 남긴다.
- **D4 — 명세 갱신**: `seek-activation-and-session.md`(한/영 예외 · 인풋 박스 모드 · 상태 머신 · §5 엣지 · 수용 기준) · `seek-overlay-ui.md`(§1 canBecomeKey + IME + 입력 소스 전환) · `korean-input.md`(F-16.1 세션 중 발화 예외 — **F-16.2 관련 확정 문구는 한 글자도 건드리지 않는다**, §9 #6-a).

**범위 제약**: ② ⛔ **영어 단일(기본, 부재) 세션은 현행 동작 그대로** — ⌃Space·⇧+Space 모두 종전대로 소비/라우팅된다(회귀 제로). ③ caps lock 입력 소스 전환(`Use Caps Lock to switch…`)은 **이번 범위 밖**(D-1 alias·hyper FSM·quick press FSM 과 깊게 얽혀 있고 사용자 피드백에 없음 — §8 후속 항목). ④ globe(🌐) 키 전환도 **이번 범위 밖**(§8 #2 — `(미확인)` 으로만 기록).

---

## 2. 사실 확인 (조사 근거)

| # | 사실 | 근거 (파일·행, 등급) |
| :--- | :--- | :--- |
| 1 | 세션 중 계층 1 게이트는 `seek_active` 면 (마우스 → Pass) → (한/영 `0x68` → Pass) → (인풋 박스 + `is_input_box_pass_key` → Pass, 아니면) **전부 Consume + `SeekKey`** | `arbitration.rs:356-404` (실측) |
| 2 | `is_input_box_pass_key`: **⌘/⌃ 중 하나라도 실리면 무조건 소비**(`has_browser_command_modifier`) — ⌃Space 도 여기 걸린다 | `seek_input_box.rs:37-63, 66-68` (실측) |
| 3 | 탭은 `CGEventTapLocation::SessionEventTap` + `CGEventTapOptions::Default`(삭제 가능) + `HeadInsertEventTap` — **콜백이 Consume 을 반환하면 이벤트가 삭제되어 시스템 단축키(입력 소스 전환)까지 도달하지 못한다** | `ultrakey-platform/src/event_tap.rs:387-396` · `engine.rs:714-721` (실측) |
| 4 | F-16 규칙은 계층 3(`evaluate_korean_rules`) — 세션 중 계층 1 이 short-circuit 하여 **평가되지 않는다**(korean-input.md §3.2 가 "세션 중 F-16.2 가 발화하지 않는 것은 설계된 동작"으로 못박음) | `arbitration.rs:463, 989-1056` · `korean-input.md:142` (실측) |
| 5 | 인풋 박스 모드 + `SPACE`(modifier 없음·⇧ 붙음)는 프린터블로 **통과** — 웹뷰 `<input>` 이 받는다(`space_passes`·`shift_letter_passes` 테스트) | `seek_input_box.rs:56-62, 179-183` (실측) |
| 6 | F-16.1 규칙: `Korean(13)` · trigger `SPACE` + `KoreanTrigger::ShiftOnly` · requires_korean_ime `false` · out `SPACE` + `CONTROL\|NX_DEVICELCTLKEYMASK(0x0004_0001)` — ⇧+Space 를 ⌃+Space 로 치환하는 규칙 | `ultrakey-korean/src/settings.rs:99-109` · `rules.rs:85-99` (실측) |
| 7 | 한/영 키(`0x68`) 예외는 인풋 박스 분기보다 앞(`arbitration.rs:370-372`) — 모든 세션 모드에서 원본 통과 | `arbitration.rs:370-372` · #76 T1~T8 (실측) |
| 8 | 인풋 박스 분기에는 "설정된 전역 단축키 조합은 통과 금지" 가드가 있다(`seek_shortcut_keycode`/`seek_shortcut_mods`) — 재입력 토글(세션 닫기)을 살린다 | `arbitration.rs:385-399` (실측) |
| 9 | 인풋 박스 모드에서 `⇧+Space` 를 누르면 **웹뷰가 스페이스 1개를 입력**할 수 있다(무시되거나). 사용자 보고의 "전환 안 됨"과 정합 | `overlay-searchbar.html` `<input>` (#93, 실측) |
| 10 | 사용자 관찰 "한글 상태로 열면 한글 입력, 영어 상태로 열면 영어 입력" — IME 입력 자체는 정상, **전환 단축키만 죽어 있다**는 것과 정확히 일치 | 이슈 #101 본문 (사용자 관찰) |
| 11 | ⭐ **선례 — 합성 ⌃Space 가 시스템 입력 소스 전환까지 도달함이 실기기 검증됨**: 세션 밖 F-16.1 이 내보낸 ⌃Space(flags `0x0004_0001`)가 실제로 입력 소스를 바꾼다는 것은 M2 3차 완료 판정("`Shift + Space` 로 입력 소스가 바뀌며")으로 확인된 사실이다. D1 의 원본 ⌃Space 통과는 이와 디스패치 경로가 동일(세션 레벨 탭 → 시스템 핫키) | `docs/spec/README.md` M2 3차 완료 판정 · `event_tap.rs:334-342`(엔진 마커) (실기기 확인) |
| 12 | `overlay-searchbar.html` 에 JS keydown/keyup 핸들러가 **없다** — 통과 키는 네이티브 `<input>`(composition event + input event + 100ms debounce)로만 간다. 웹뷰가 전환 키를 가로챌 경로가 없다 | `overlay-searchbar.html:248-491` (실측) |
| 13 | 웹뷰 네이티브 `<input>` 에서 ⌃Space 의 동작(`setMarkedText:` 관례)은 미확인 — 코코아 텍스트 입력의 ⌃Space = "marked text 삼입"이며 WKWebView 에서는 대개 가시 동작 없음. 시스템 단축키가 발화하면 그 전에 시스템이 가로챈다 | WebKit 동작 `(추정)` — M1 실기기 확인 |

---

## 3. 결정 목록 D1~D4

### D1 — 시스템 입력 소스 전환 단축키(⌃Space·⌃⌥Space)를 인풋 박스 모드에서 원본 통과

**결정**: `seek_input_box.rs` 에 판정 헬퍼 `is_input_source_switch_shortcut(ev)` 를 추가하고, `arbitration.rs` 계층 1 인풋 박스 분기에서 이 조합을 `Outcome::pass` 시킨다. **⭐ D2(⇧+Space 재평가) 다음, `is_input_box_pass_key` 앞에 평가한다**(상급 리뷰 #3-a — 순서 근거는 D1 항목 끝 참조).

```
if gates.seek_input_box {
    …(D2 — SPACE 재평가, §D2)…
    if is_input_source_switch_shortcut(ev) && !is_seek_shortcut_combo(ev, gates) {
        return Outcome::pass(Layer::SeekSession);   // 시스템이 입력 소스 전환 처리
    }
    if is_input_box_pass_key(…) { …기존 가드·통과… }
}
```

- **판정**: `ev.keycode == SPACE && ev.flags.contains(CONTROL) && !ev.flags.contains(COMMAND)`.
  - ⌃Space(이전 입력 소스)·⌃⌥Space(입력 메뉴의 다음 소스) — macOS 기본 단축키 2종 + 사용자 지정 ⌃계열 단축키를 커버.
  - ⛔ COMMAND 가 실리면 제외 — ⌘Space(Spotlight)는 지금처럼 소비한다(앱 단축키 보호 원칙 유지).
  - ⇧ 는 가리지 않는다(제한을 새로 만들지 않는다 — 시스템이 처리하지 않는 ⌃⇧Space 는 웹뷰에 도달해 무해. 키 윈도우는 우리 오버레이 HTML 일 뿐이라 제3자 콘텐츠 위험이 없다).
- **세션 토글 단축키 예외**: `is_seek_shortcut_combo`(기존 가드 로직, `arbitration.rs:389-396` 을 헬퍼로 추출)가 참이면 **통과하지 않고** 아래로 떨어져 Consume → `SeekKey` → machine 의 재입력 토글(세션 닫기)을 살린다. ⚠️ 오늘(수정 전)은 ⌃Space 가 가드 검사 없이 무조건 Consume+SeekKey 라 같은 효과 — D1 은 그 동작을 정확히 보존한다.
- **영어 단일(기본) 세션은 이 분기 자체가 없다** — `gates.seek_input_box == false` 이므로 ⌃Space 는 종전대로 Consume. **회귀 제로**.

**근거**: 시스템 입력 소스 전환 단축키는 앱이 아니라 **시스템(Carbon HIToolbox 단축키)** 이 처리한다. 탭이 Consume 하면 그 단축키가 발화할 기회 자체가 사라진다(사실 ③). ⭐ **선례(사실 ⑪)**: 이 저장소는 이미 세션 밖 F-16.1 의 합성 ⌃Space 가 시스템 입력 소스 전환까지 도달함을 실기기로 확인했다 — 마커를 단 합성 이벤트가 자기 탭을 통과한 뒤 시스템 핫키 디스패치에서 처리된 것이다. D1 의 원본 ⌃Space 통과는 같은 디스패치 경로다(등급: `(실기기 검증된 경로와 동일)`). 인풋 박스 모드는 우리 앱이 활성 앱이므로, ⌃Space 를 통과시키면 평범한 앱(Safari 의 텍스트 필드 등)과 똑같이 시스템이 입력 소스를 바꾼다 — "전환된 상태에서의 입력은 정상"(사실 ⑩)이 그 전제를 뒷받침한다. 남은 불확실성은 "우리 앱이 활성·웹뷰가 키 윈도우인 상태에서도 동일한가" 하나뿐이며, 그것이 M1/M2 가 검증할 정확한 범위다.

**기각한 대안** (⭐ #4-a 반영 — 문구 정정):

| 대안 | 기각 사유 |
| :--- | :--- |
| ① 전 모드(영어 포함)에서 ⌃Space 통과 | ② **시스템 단축키가 비활성화된 구성**에서는 통과된 ⌃Space 가 포커스 앱(영어 단일 세션에서는 하위 앱)에 그대로 가 그 앱의 자체 ⌃Space 바인딩을 건드릴 수 있다. ③ #93 의 "영어 단일 = 현행 동작 보존" 불변식에 어긋난다. — 결정은 유지, 사유 문구만 정정 |
| ② ⌃Space 를 `is_input_box_pass_key` 안에서 예외 처리 | 그 함수의 계약("⌘/⌃ = 소비")이 흐려진다. 시스템 단축키 통과는 **게이트 차원의 결정**이지 "인풋 박스가 처리할 키" 판정이 아니다 — 분리된 헬퍼가 Event Viewer·테스트에서 경로를 명확히 드러낸다 |
| ③ 웹뷰 JS 가 keydown 을 감지해 처리 | ⌃Space 는 게이트가 이미 Consume 하여 웹뷰에 **도달조차 안 한다**(사실 12 — JS keydown 핸들러도 없다). JS 에서 처리할 이벤트가 없다. 근본 원인(탭 단계 소비)을 건드리지 않는 꼼수일 뿐 |

**⭐ D2-우선 순서의 근거 (상급 리뷰 #3-a)**: D1 를 먼저 두면 다음 시퀀스에서 래치가 잔존한다 — ① F-16.1 발화(⇧+Space down — 래치 세움, ⌃Space down 합성) → ② 사용자가 space 를 누른 채 **⌃ 를 추가** 후 릴리즈 → ③ 그 이벤트는 `SPACE+CONTROL` KeyUp 인데 D1 이 먼저라 래치 소비 없이 통과 → ④ 이후 평범한 공백의 up 이 잔존 래치에 잡혀 짝 없는 ⌃Space up 합성. 사용자 가시 영향은 거의 0 이지만, **D-K6 "down 이 세운 래치는 대응 up 이 조건 재평가 없이 소비한다"는 불변식을 위반**한다. D2 를 먼저 두면: ⌃Space up 은 control 눌림 때문에 `ShiftOnly` 가 항상 거짓 → F-16.1 은 발화하지 않고 평가만 지나가며(None — 부수효과 없음), **래치가 있으면 그것을 소비**(KeyUp 분기)한다. 두 순서의 동작 차이는 이 한 케이스뿐이다 — D2-우선이 정확히 그 케이스를 해소한다.

### D2 — F-16.1(⇧+Space)을 인풋 박스 세션 중 발화 — 계층 3 규칙을 그 자리에서 재평가

**결정**: `arbitration.rs` 계층 1 의 인풋 박스 분기에서, **`ev.keycode == KeyCode::SPACE` 이고 `kind` 가 KeyDown/KeyUp** 이고 **설정된 세션 토글 단축키와 일치하지 않을 때**, `self.evaluate_korean_rules(cfg, ev, kind, gates, &mut korean_outcome)` 를 호출하고, 참이면 그 `Outcome`(Consume + `SPACE`+⌃ 치환 + 래치)을 반환한다. F-16.1 이 꺼져 있으면(기본) 규칙을 찾지 못해 `false` 가 나오고, 기존 경로로 이어진다.

```
if gates.seek_input_box {
    // ⭐(이슈 #101) F-16.1 — SPACE 키에 한해 계층 3 규칙을 이 자리에서 재평가한다.
    // (계층 1 short-circuit 때문에 세션 중 계층 3 가 평가되지 않는다 — 사실 ④)
    // ⚠️ D1 보다 먼저 평가 — ⌃Space up + 잔존 래치 케이스(리뷰 #3-a)에서 래치를 소비한다.
    if ev.keycode == KeyCode::SPACE
        && (kind == EventKind::KeyDown || kind == EventKind::KeyUp)
        && !is_seek_shortcut_combo(ev, gates)   // ⭐ 세션 토글 보존(모든 조합) — T4·T7-c 정합
    {
        let mut ko = Outcome::consume(Layer::KoreanInput);
        if self.evaluate_korean_rules(cfg, ev, kind, gates, &mut ko) {
            return ko;
        }
    }
    if is_input_source_switch_shortcut(ev) && !is_seek_shortcut_combo(ev, gates) { …D1 통과… }
    if is_input_box_pass_key(…) { …기존… }
}
```

- **왜 `evaluate_korean_rules` 통째로 재사용하는가**: 규칙 조회(`trigger_key == SPACE && shift-only`), 앱 제외 게이트·IME 조건·`active_synth_flags`(hyper 가 쥔 채로는 발화 금지) 조건 3종, **래치(D-K6 — down/up 짝맞춤)** 까지 전부 그대로 쓴다. 새 판정 로직을 만들면 이슈 #73 이 겪은 "테스트가 코드의 사본을 고정하는" 사고의 재발 자리가 된다. 함수는 `cfg.rules.korean_rules`·게이트·눌림 테이블·래치만 읽고 그 외 문맥에 의존하지 않으므로(`arbitration.rs:989-1056`) 계층 1 자리에서 불러도 안전하다. 눌림 테이블은 계층 1 **앞**에서 갱신되므로(`arbitration.rs:321-325`) 세션 중에도 `ShiftOnly` 판정은 정확하다.
- **SPACE 로 한정하는 이유**: 한/영 키(`0x68`)는 §D3 처럼 **원본 통과**가 옳은 처리이고(#76 — 치환하면 통과 의미가 사라진다), 한자(`0x66`)·백틱(`0x32`)은 세션 중 발화해서는 안 되는 기존 설계다. F-16.1 하나만이 "세션 중 발화해야 하는 사용자 기대"가 있는 규칙이다. (참고: 인풋 박스에서 한자 키는 계속 Consume, 백틱은 프린터블로 통과 — 둘 다 이번 변경 없음.)
- **순서**: **D1(⌃Space 통과)보다 앞**. ⇧+Space 는 `is_input_box_pass_key` 가 프린터블로 통과시키므로, 발화 판정이 그보다 앞서야 한다(발화가 없으면 이후 통과 판정을 그대로 탄다 — F-16.1 꺼짐 + ⇧+Space = 검색어 공백 유지).
- **세션 토글 보존**: `!is_seek_shortcut_combo` 를 재평가 조건에 넣는다 — 토글=⇧+Space 같은 작위적 구성에서도 **오늘의 동작(토글 = 세션 닫기)이 정확히 유지**된다(토글로 설정된 조합은 D2 가 건너뛰고, `is_input_box_pass_key` 분기의 기존 가드가 Consume+SeekKey 로 처리).
- **래치 세션 경계 안전성 (리뷰 #3 확인)**: 세션 중 세운 래치는 ① 세션 중이면 D2 가, ② 그 사이 세션이 닫혔으면 계층 3 의 **같은 함수**가 소비한다 — KeyUp 분기는 조건을 재평가하지 않고 래치만 본다(D-K6), 래치는 `trigger_key` 기준 조회라 다른 keycode 를 오염시키지 않는다(`keystate.rs:79-85`). `seek_trigger == SPACE` 구성은 SPACE 가 계층 1 앞 단계(`arbitration.rs:336-348`)에서 항상 가로채이므로 **래치가 아예 생기지 않아**(F-16.1 영영 미발화) 안전하다.
- **영어 단일 세션**: 이 분기 자체가 없다 — ⇧+Space 는 종전대로 Consume → `SeekKey`(검색어 공백). **회귀 제로**.

**근거**: 사용자가 F-16.1 을 켠 것은 "⇧+Space = 입력 소스 변경"을 선언한 것이다. 세션 중(특히 인풋 박스) 그 선언이 조용히 무시되어 지금은 **스페이스 1개가 입력되거나 무시**된다(사실 ⑨). "입력 소스 전환 단축키가 살아 있어야 전환된 상태로 입력을 이어갈 수 있다"는 #76 의 정신(한/영 예외)과 같은 방향이다 — 다만 ⇧+Space 는 macOS 네이티브 단축키가 아니어서 통과만으로는 안 되고 **치환이 필요**하다.

**기각한 대안**:

| 대안 | 기각 사유 |
| :--- | :--- |
| ① F-16 규칙 전체를 세션 중 평가(계층 1 이 하위 계층으로 넘기는 구조 개편) | 한/영·한자·백틱 규칙은 세션 중 발화되어서는 안 되는 기존 확정(한/영은 원본 통과가 정답, 한자는 `return` 출력이라 세션 중 오발 위험). 구조 개편은 회귀 표면을 전 계층으로 넓힌다 — "SPACE 키 한정 재평가"가 원인 자리에서의 최소 수정이다 |
| ② ⇧+Space 를 웹뷰 JS 에서 감지해 `seek_set_query` 대신 별도 커맨드로 | 이벤트가 게이트를 통과해 웹뷰에 도달한 뒤의 처리 — 게이트 단계에서 끝내는 편이 경로가 하나다. 웹뷰 처리는 조합 중(`compositionstart`) 상태와 겹쳐 타이밍 버그가 생길 여지가 있다 |
| ③ 영어 세션에서도 F-16.1 발화 | 영어 단일 = 한 언어 검색이므로 세션 중 입력 소스 변경의 실익이 없고, ⇧+Space 가 지금 검색어 공백(SeekKey→Text)으로 쓰이는 동작을 바꾼다. #93 의 "영어 단일 현행 유지" 원칙에 어긋난다 |

### D3 — 한/영 키(`0x68`): 코드 변경 없음, 실기기 검증 항목으로만 관리

**결정**: 코드 변경을 하지 않는다. `arbitration.rs:370-372` 의 JIS_KANA 예외가 인풋 박스 분기보다 **앞**에 있어 **이미 통과**이며, 이번 회귀의 원인이 아니다(§2 사실 ⑦). §8 에 "인풋 박스(키 윈도우) 상태에서 0x68 이 실제로 입력 소스를 전환하는가"를 실기기 검증 항목으로 남긴다.

- **이유**: 0x68 은 앱 입력 컨텍스트(`NSTextInputContext`/IMK)가 처리하는 키다. 키 윈도우가 웹뷰(WKWebView)일 때도 IMK 계층이 0x68 을 가로채 전환할 가능성이 높지만(Chromium 의 KANJI_MODE 매핑이 그 전제 위에 서 있다), **이 코드베이스는 실기기 확인 없는 동작을 명세에 적지 않는다** — 등급을 `(실기기 미검증)` 으로 두고 문헌에 남긴다.

### D4 — 명세 갱신

| 문서 | 갱신 내용 |
| :--- | :--- |
| `seek-activation-and-session.md` §3.1 | 한/영 키 예외 문단을 **입력 소스 전환 3경로**로 확장: ① 한/영 키(0x68) 원본 통과(기존) ② 시스템 단축키 ⌃Space·⌃⌥Space — **인풋 박스 모드에서 원본 통과**(단, 세션 토글 단축키와 일치하면 소비 = 재입력 토글) ③ ⇧+Space(F-16.1) — 인풋 박스 모드에서 **세션 중 재평가·치환 발화**(F-16.1 꺼짐이면 검색어 공백 통과). 영어 단일 세션은 셋 다 종전 동작. ⭐ **기존 확정 문구 "F-16.2 는 계층 3 에 있어 세션 중에는 계층 1 이 먼저다" 는 반드시 개정한다** — 인풋 박스 + SPACE 한정으로 계층 3 규칙이 세션 중 재평가되므로 더 이상 무조건 참이 아니다(§9 #6-b) |
| `seek-activation-and-session.md` §3.2 | 상태 머신 표에 인풋 박스 모드 행 추가: ⌃Space/⌃⌥Space(토글 단축키 제외) → 통과(시스템 입력 소스 전환), ⇧+Space(F-16.1 켬) → 통과 대신 ⌃Space 치환 |
| `seek-activation-and-session.md` §5 | 엣지 케이스 추가: ⑥ 세션 중 입력 소스 전환 시 조합 상태 처리(전환 직후 webview `compositionend` flush — M8), ⑦ 래치 잔존 비대칭 없음(리뷰 #3-a 반영 후 — D2-우선이 ⌃Space up 래치를 소비) |
| `seek-activation-and-session.md` §8 | 수용 기준 추가: ① 인풋 박스 세션 중 ⌃Space 로 입력 소스 전환 ② 인풋 박스 세션 중 F-16.1 켬 상태 ⇧+Space 로 전환 ③ 영어 단일 세션에서 ⌃Space·⇧+Space 가 검색어/라우팅에 영향 없음(회귀 없음) |
| `seek-overlay-ui.md` §1 | IME 조합 문단에 "인풋 박스 모드에서 입력 소스 전환은 **게이트가 시스템·F-16.1 경로로 통과**시킨다 — 전환 단축키는 웹뷰가 처리하는 게 아니라 시스템이 처리하므로 `<input>` 구현과 무관" 명시 |
| `korean-input.md` §3.1/F-16.1 | F-16.1 행·시나리오 A 에 "**인풋 박스(다국어) Seek 세션 중에는 세션이 계층 3 을 short-circuit 하므로, 세션 게이트가 같은 규칙을 그 자리에서 재평가**한다(F-16.1 한정, 이슈 #101)" 명시 |
| `korean-input.md` §3.2 상호참조 | F-16.1 예외를 **별도 문장**으로 추가(§9 #6-a) — ⛔ **F-16.2 관련 확정 문구("다국어 세션에서도 한/영 키(0x68)는 여전히 원본 통과라 F-16.2 는 세션 중 발화하지 않는다")는 한 글자도 건드리지 않는다.** 이슈 본문이 ⇧Space 를 "F-16.2"로 오기한 것에 끌려다니면 정면 모순이 생긴다 |
| `korean-input.md` §9 | 다음 질문 추가: 인풋 박스(키 윈도우) 상태에서 한/영 키(0x68) 원본 통과가 실제 전환을 일으키는지 — 실기기 미검증 |

---

## 4. 테스트 계획 (T#)

### `crates/ultrakey-core/src/seek_input_box.rs` — S1~S6 (+#2-a/b/c 반영)

| # | 테스트 | 기대 |
| :--- | :--- | :--- |
| S1 | `is_input_source_switch_shortcut` — ⌃Space(0x0004_0001) | true |
| S2 | — ⌃⌥Space(0x000C_0001·0x0004_0020 조합) | true |
| S2-b | — **⌃⇧Space** | true(⇧ 미가림 결정 고정 — 리뷰 #2-a) |
| S2-c | — **우측 control**(0x40000 \| 0x2000, `flags.rs:57`) | true(리뷰 #2-c) |
| S2-d | — ⌃ + caps lock 잠금 비트(0x10000) | true(잠금 비트 무영향) |
| S3 | — ⌘Space | false(COMMAND 가드) |
| S4 | — ⌘⌃Space | false |
| S5 | — Space(무modifier)·⇧+Space·⌥+Space | false |
| S6 | — 비-Space 키 + ⌃(예: ⌃A) | false |

### `crates/ultrakey-core/src/arbitration.rs` — T1~T11 + 리뷰 테스트 ①~⑧

| # | 테스트 | 기대 |
| :--- | :--- | :--- |
| T1 | 인풋 박스 + ⌃Space **KeyDown** | **Pass**(시스템 전환 — SeekKey 없음) |
| T1-b | 인풋 박스 + ⌃Space **KeyUp** | **Pass**(키 페어 통과 — 리뷰 #2-b) |
| T2 | 인풋 박스 + ⌃⌥Space | **Pass** |
| T2-b | 인풋 박스 + ⌃⇧Space | **Pass**(리뷰 #2-a) |
| T3 | 인풋 박스 + ⌘Space / ⌘⌃Space | **Consume + SeekKey**(Spotlight·앱 보호 유지) |
| T4 | 인풋 박스 + ⌃Space == **설정된 세션 토글** | **Consume + SeekKey**(재입력 토글 — 기존 T7-c 동작 보존) |
| T4-b | **토글=⌥Space 설정** + ⌃Space | **Pass**(가드 과매칭 없음 — 리뷰 ⑦, 기존 `input_box_consumes_configured_global_shortcut_combo` 의 역방향) |
| T5 | **영어 단일** + ⌃Space | **Consume + SeekKey**(현행 유지 — 회귀 없음) |
| T5-b | **영어 단일 + F-16.1 켬** + ⇧+Space | **Consume + SeekKey**(D2 분기 자체가 없음 고정 — 리뷰 ⑥) |
| T6 | 인풋 박스 + **F-16.1 켬** + shift 다운 → ⇧+Space KeyDown → ⇧+Space KeyUp → shift 업 | Consume + `SPACE`+⌃(0x0004_0001) KeyDown/KeyUp 짝 방출(래치) — 시나리오 A |
| T7 | 인풋 박스 + **F-16.1 꺼짐(기본)** + ⇧+Space | **Pass**(검색어 공백 — 회귀 없음) |
| T8 | 인풋 박스 + F-16.1 켬 + **hyper 활성**(active_synth_flags) + ⇧+Space | 미발화 — 통과(`evaluate_korean_rules` 조건 3 재사용 확인) |
| T9 | 인풋 박스 + ⌃A·⌘A | 계속 Consume(기존 `input_box_consumes_command_and_control_combos` 재확인) |
| T10 | 인풋 박스 + F-16.1 켬 + **한/영 키(0x68)** | 계속 **Pass**(#76 예외가 재평가 분기보다 앞 — 치환되지 않음) |
| T11 | 인풋 박스 + F-16.1 켬 + **한자(0x66)** | 계속 Consume + SeekKey(재평가 분기 SPACE 한정) |
| T-① | **래치 세션 경계**: 세션 + F-16.1 켬 → ⇧+Space down(발화·래치) → 게이트 `seek_active=false` → space up | **Consume + ⌃Space KeyUp 합성**(계층 3 래치 경로 — 리뷰 ①) |
| T-② | **래치 pending + ⌃Space up** (리뷰 #3-a 시퀀스, D2-우선 기준) | **Consume + ⌃Space up 합성 + 래치 clear**(통과가 아님) |
| T-③ | **autorepeat**: 세션 + F-16.1 켬 + ⇧+Space 반복 down ×N → up | 전부 Consume + 치환(래치 덮어쓰기 — 리뷰 ③) |
| T-④ | **무래치 KeyUp 통과**: 세션 + F-16.1 켬 + 무modifier space down(통과) → space up | **Pass**(D2 KeyUp 이 래치 없이 소비하지 않음 고정 — 리뷰 ④) |
| T-⑤ | **D2→D1 순서**: 세션 + F-16.1 켬 + ⌃Space | **Pass**(D2 미발화 — control 로 ShiftOnly 거짓 → D1 통과, 순서 고정 — 리뷰 ⑤) |

### 기존 테스트 영향 검토

- `input_box_consumes_command_and_control_combos` — `ANSI_A`+⌘/⌃. D1 은 SPACE 한정이라 영향 없음. ✅
- `input_box_consumes_configured_global_shortcut_combo` — `⌥Space`(CONTROL 없음). D1 통과 조건(CONTROL) 불충족이라 영향 없음. ✅
- `space_passes`(`seek_input_box`) — Space 무modifier. D1 은 ⌃ 실림만. ✅
- F-16.1 기존 테스트(`f16_1_shift_only_space_is_substituted_with_control` 등) — 세션 비활성(`GateSnapshot::default()`)이라 계층 3 경로 그대로. 영향 없음. ✅

---

## 5. 작업 분해

| # | 작업 | 산출물 |
| :--- | :--- | :--- |
| 1 | `seek_input_box.rs`: `is_input_source_switch_shortcut` 추가 + S1~S6 | 헬퍼 + 테스트 |
| 2 | `arbitration.rs`: 인풋 박스 분기 — ① D2 재평가 분기(SPACE 한정·토글 가드 공유) ② `is_seek_shortcut_combo` 헬퍼 추출(기존 `arbitration.rs:389-396` 로직) ③ D1 통과 분기 | 계층 1 수정 |
| 3 | `arbitration.rs` 테스트 T1~T11 + T-①~⑤ | 회귀 + 신규 동작 고정 |
| 4 | `cargo test -p ultrakey-core` → `cargo test --workspace` → `cargo clippy --workspace --all-targets -- -D warnings` | 전부 통과 |
| 5 | 명세 갱신(D4 + §9 #6-a~c 반영) — 3개 문서 | `docs/spec/*.md` |
| 6 | 커밋(한국어+Gitmoji, `-c commit.gpgsign=false`) → 푸시 → PR `Closes #101`(원인 실측·수정·기각 대안·상급 리뷰 결과·수동 검증 절차) | PR |

---

## 6. 실기기 검증 (수동 — 자동화 불가)

> 이 프로젝트 규약(manual-verification.md 기조): **실기기 검증을 완료 조건에 두는 이유는 자동 테스트가 실제 macOS 이벤트 모양을 재현하지 못하기 때문**이다. 아래 절차를 PR 본문·이슈 코멘트에 남긴다. **M1~M4 는 한국어 입력 소스 2개 이상 설정 필요.**

| # | 절차 | 기대 |
| :--- | :--- | :--- |
| M1 | 검색 언어 = `English + 한국어` → 텍스트 필드에 커서(한글 상태) → ⌃Space | 입력 소스가 영어(ABC)로 전환 — 이후 타이핑이 영어로 입력. **시스템 단축키 비활성화 구성에서도 ⌃Space 가 웹뷰를 깨지 않음을 함께 확인**(사실 13 의 `(추정)` 판정) |
| M2 | 같은 세션 · 영어 상태 → ⌃Space | 한글로 재전환 — 한글 조합 입력 + 검색 동작 |
| M3 | F-16.1(`⇧+Space 로 입력 소스 변경`) 켬 → 세션 중 ⇧+Space | 입력 소스 전환(F-16.1 발화) — 스페이스가 검색어에 들어가지 않아야 함 |
| M4 | F-16.1 꺼짐(기본) → 세션 중 ⇧+Space | 검색어에 공백 입력(기존 동작) |
| M5 | 영어 단일 세션 → ⌃Space·⇧+Space | 검색어/세션 라우팅에 영향 없음(회귀 없음) |
| M6 | (106키 키보드 보유 시) 세션 중 한/영 키 | 입력 소스 전환(#76 예외가 키 윈도우에서도 동작하는지 확인) |
| M7 | `ULTRAKEY_TRACE_TAP=1` → 세션 중 ⌃Space | 트레이스에 `layer=SeekSession disposition=Pass`(소비 아님) 기록 확인 |
| M8 ⭐ | **한글 조합 도중** ⌃Space(또는 F-16.1 ⇧Space)로 전환 → 전환 후 계속 입력 | ① 미완성 조합이 쿼리에 반영되거나(compositionend flush) 깨끗이 사라짐 ② 전환 후 타이핑이 **계속 쿼리에 반영됨**(`composing` 플래그 stuck 아님 — 리뷰 #7 이 지적한 잠재 장애 경로: `overlay-searchbar.html:293-295, 315-318` 의 `compositionend` 의존) |

**기기 제약**: 인풋 소스 전환은 한국어 입력기 설정이 있는 macOS 가 필요하다. M6 은 한국어 106키 물리 키보드가 필요(없으면 `(실기기 미검증)` 으로 유지).

---

## 7. 리스크

| 리스크 | 완화 |
| :--- | :--- |
| ⌃Space 통과로 인해 인풋 박스 세션 중 사용자 앱에 ⌃ 단축키가 새어 들어감 | 우리 앱이 활성 앱이라 새는 대상이 없다(키 윈도우 = 우리 웹뷰). ⌘⌃ 는 계속 소비. 웹뷰의 ⌃Space 처리 능력은 `(추정)`(사실 13) — M1 이 함께 확인 |
| D1/D2 의 pass 가 설정된 토글 단축키와 충돌 | `is_seek_shortcut_combo` 가드 — 토글로 설정된 조합은 **D2(재평가 skip)·D1(통과 skip) 둘 다** 건너뛰고 Consume(재입력 토글 유지) |
| D2 의 재평가가 `compositionstart` 중 ⇧+Space 와 겹침 | IME 조합 중 ⇧+Space 가 오면 F-16.1 이 소비하고 ⌃Space 를 방출 — 조합 버퍼는 키 이벤트와 독립. 전환 직후 `compositionend` flush 가 쿼리 반영을 보장해야 하며 **그것이 M8 의 판정 기준**(리뷰 #7) |
| `seek_trigger`(= `Remap key to Seek:`)가 SPACE 인 구성 | SPACE 가 계층 1 앞 단계(`arbitration.rs:336-348`)에서 항상 가로채이므로 D2·D1 모두 도달 못함 — F-16.1 세션 발화·⌃Space 통과 모두 대상 아님. §3-b 의 "Seek 은 모든 리매핑보다 상위 의도" 그대로(리뷰 #3-b) |
| caps lock / globe 입력 소스 전환은 이번 범위 밖 | §8 후속 항목으로 남기고, PR 본문에 한계로 명시 |

---

## 8. 미해결 질문 / 후속

| # | 질문 | 상태 |
| :--- | :--- | :--- |
| 1 | 키 윈도우(웹뷰) 상태에서 0x68 원본 통과가 실제 입력 소스 전환을 일으키는가 | `(실기기 미검증)` — M6. 106키 키보드 필요 |
| 2 | **globe(🌐) 키** 입력 소스 전환은 세션 중 동작하는가 — 탭보다 아래(WindowServer)에서 처리되면 이미 동작하고, 탭을 지나면 계층 1 에서 소비 중 | `(미확인)` — 리뷰 #1-b. 범위 확대 없이 사실만 기록. 후속 이슈 후보 |
| 3 | caps lock(`Use Caps Lock to switch…` 설정) 입력 소스 전환은 이번에 건드리지 않는다 — 세션 중 계속 소비된다 | 후속 이슈 후보. D-1·hyper FSM·quick press FSM 과 얽힘 |

---

## 9. 상급 리뷰 반영 (2026-09-03, `ultrakey-review` — 조건부 통과)

> 리뷰 판정: **조건부 통과**. 방향(D1~D4)은 정합하나 확정 전 반영 7건. 아래 표가 반영 내역이다. 리뷰 원문 지적 번호(#1~#7)와 초안 조치가 1:1 대응한다.

| 리뷰 지적 | 반영 |
| :--- | :--- |
| #1-a 원인 실측 보강 — F-16.1 선례(합성 ⌃Space → 시스템 전환, M2 3차 완료 판정)를 인용해 D1 근거 등급 승격 | §2 사실 ⑪ 추가 · §3 D1 근거 문단에 인용·등급 명시 |
| #1-b 미언급 경로 — globe(🌐) 키 전환 | §8 #2 로 기록(범위 확대 없음) |
| #2-a ⌃⇧Space 판정을 고정하는 테스트 부재 | S2-b·T2-b 추가(⇧ 미가림 결정 고정) |
| #2-b ⌃Space KeyUp 통과 미고정 | T1-b 추가 |
| #2-c 우측 control 변형 부재 | S2-c 추가(`flags.rs:57` DEVICE_RIGHT_CONTROL) |
| **#3-a (핵심) D1→D2 순서가 래치 잔존 엣지를 만든다 — D2 를 앞으로** | ⭐ 채택: **D2(SPACE 재평가)를 D1(⌃Space 통과)보다 앞에 평가**. §3 D1 "D2-우선 순서의 근거" 참조 — 유일한 동작 차이(⌃Space up + 잔존 래치)를 D2 가 정확히 소비. T-② 로 고정 |
| #3-b seek_trigger==SPACE 경계 미언급 | §7 리스크 행 추가(SPACE 가 계층 1 앞에서 항상 가로채임 — F-16.1 세션 발화 대상 아님) |
| #4-a D1 기각 대안 ① 의 "하위 앱 입력 소스" 표현 부정확 | 사유 정정 — "시스템 단축키 비활성화 구성에서 하위 앱의 자체 ⌃Space 바인딩" + "#93 현행 유지 불변식" (입력 소스는 시스템 전역 상태) |
| #5 테스트 8종 공백 | §4 에 T-①~⑤·S2-b/c/d·T1-b·T2-b·T4-b·T5-b 추가 (우선순 ① 래치 세션 경계 ② 래치 pending+⌃Space up ③ autorepeat ④ 무래치 KeyUp ⑤ D1 우위) |
| #6-a 이슈 본문이 ⇧Space 를 "F-16.2"로 오기 — F-16.2 확정 문구 보존 지시 | §3 D4·§1 에 "F-16.2 관련 문구 한 글자도 건드리지 않음" 명시, F-16.1 예외는 별도 문장으로 |
| #6-b "세션 중 계층 3 은 평가되지 않는다" 메타 문구 개정 필요 | §3 D4 표 첫 행에 기존 확정 문구 개정 명시 |
| #6-c §5 엣지 케이스 누락 | §3 D4 §5 행 추가(전환 시 조합 상태 · 래치 잔존 없음) |
| #7 조합 중 전환 — `composing` 플래그 stuck 잠재 장애 경로 + 웹뷰 ⌃Space 동작 `(추정)` 강등 | §6 M8 추가(판정 기준 포함) · §2 사실 13 `(추정)` · §7 리스크 행 |