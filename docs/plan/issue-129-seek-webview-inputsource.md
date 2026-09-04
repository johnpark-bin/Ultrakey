# 이슈 #129 — Seek 인풋 박스 합성 ⌃Space 웹뷰 흡수 — P1 조사·후보 비교 (초안, 확정 전)

> **성격**: `docs/plan/` 은 구현 위임 전 계획 문서. 이 문서는 P1(중급, herdr-dev-delegate)이
> 작성한 **조사·후보 비교 초안**이다 — 확정은 P2(상급 서브에이전트, `model: opus`)가 한다.
> 이슈 #129 본문의 실기기 트레이스(6:1)는 그대로 받아들이고(재도출하지 않는다), 이 문서는
> 그 사실 위에서 코드 근거를 재확인하고 두 후보(+ 조사 중 발견한 세 번째 후보)를 비교한다.

---

## 0. 요약

- 이슈 #129 본문의 실기기 트레이스(`layer=KoreanInput disp=Consume emitted=[...]` 6회 vs
  `kTISNotifySelectedKeyboardInputSourceChanged` 1회)는 코드 구조와 모순되지 않는다 —
  D2(`evaluate_korean_rules`)는 매 탭 정상 발화해 합성 `⌃Space`(SPACE keycode + control
  flag)를 `post_to_tap`(`CGEventTapPostEvent`)으로 방출한다(F13, 아래).
- **⭐ 새로 확인한 사실 — 후보 ①(`TISSelectInputSource` 직접 전환)을 재검토할 때 반드시
  봐야 하는 기존 프로젝트 결정**: 이 저장소는 이미 `docs/spec/language-presets.md` §8 에서
  **"select_input_source 류(TISSelectInputSource 직접 호출)가 코드에 존재하지 않는다"를
  부정형 수용 기준으로 못박고 있다** — 근거는 `docs/research/ke-complex-modifications-analysis.md`
  §4-1 이 인용한 **macOS 의 문서화된 CJK(중·일·한·베트남) 입력 소스 직접 전환 버그**다:
  Karabiner-Elements 공식 문서 "Switching to input sources which have input_mode_id
  (Chinese, Japanese, Korean, Vietnamese) may be failed due to an macOS issue… sending
  the input source switch shortcut (e.g., control-space) is better than using
  `select_input_source`" + 독립된 실사용 보고 3건("아이콘은 바뀌는데 실제 입력은 안 바뀐다",
  macOS 26 에서도 재현). **F-16.1 이 애초에 `⌃Space` 재주입 방식을 택한 이유가 바로 이
  버그다** — 즉 "select_input_source 직접 호출 금지"는 임의의 보수적 관례가 아니라, **이번에
  후보 ①이 targeting 하려는 바로 그 케이스(한국어 IME 전환)에서 이미 검증된 macOS 결함을
  피하기 위한 결정**이다(P1 판단, 아래 §2 근거).
- 이 사실 하나가 후보 비교의 무게 중심을 옮긴다 — 아래 §2·§3.

---

## 1. 코드 재확인 (이슈 본문 트레이스와의 정합성)

| # | 사실 | 근거 |
| :--- | :--- | :--- |
| F13 | D2 의 SPACE KeyDown 발화는 `SynthEvent { kind: KeyDown, keycode: rule.out_keycode, flags: rule.out_flags }` 를 만들고(`arbitration.rs:1299-1306`), F-16.1 규칙의 `out_keycode`/`out_flags` 는 "같은 `space` keycode 에 control 플래그만"(`korean-input.md` §3.3 표) — 즉 `⌃Space`. KeyUp 도 래치 짝으로 동일 치환을 낸다(`arbitration.rs:1225-1239`). | `arbitration.rs` (실측) |
| F14 | 그 `SynthEvent` 는 `on_tap_event` 안에서 `apply_outcome_in_tap` → `make_synth_event` → `SyntheticEvent::keyboard(keycode, down, flags)` → `post_to_tap(proxy)` = `CGEventTapPostEvent` 로 방출된다 — **탭 콜백 스택 프레임 안에서 동기 실행**(`engine.rs:390-397,690`). | `engine.rs` (실측) |
| F15 | 검색 바는 인풋 박스 세션 개시 시 `make_search_bar_key_for_input` 이 `set_can_become_key(true)` + `activate_pid` + `make_key_and_order_front` 로 **실제 macOS 키 윈도우로 승격**된다(이슈 본문의 "search bar promoted to key window for input-box session" 로그가 바로 이 지점) — WKWebView `<input>` 이 포커스를 갖고 IME 조합이 가능한 상태가 된다. | `overlay.rs:489-519` (실측) |
| F16 | `text_input_source.rs` 는 현재 **조회/관측 전용**이다 — `current_layout()`(레이아웃 스냅샷 읽기)·`translate()`(uchr 역산)·`observe_input_source_changes()`(`kTISNotifySelectedKeyboardInputSourceChanged` 구독)만 있고, **`TISSelectInputSource` 바인딩·호출자는 이 크레이트에도 저장소 어디에도 없다**(§0 요약의 부정형 수용 기준과 일치). | `text_input_source.rs` 전수 열람 (실측) |
| F17 | "웹뷰가 흡수한다"는 트레이스 관찰이지 코드로 재현·반증할 수 있는 지점이 아니다 — `post_to_tap` 이후의 경로(WindowServer 의 시스템 심볼릭 핫키 매칭 단계)는 이 저장소 코드 밖이다. **정확한 흡수 메커니즘은 여전히 `(미확정)`** — M-절차(실기기)로만 확인 가능하다. | 구조적 한계(코드 열람으로 확인 불가) |

---

## 2. 후보 ① — D2 발화 시 `TISSelectInputSource` 직접 전환

### 구현하려면 필요한 것 (신규 범위)

1. `crates/ultrakey-platform/src/ffi.rs` 에 `TISSelectInputSource` extern 선언 추가(현재 없음).
2. `text_input_source.rs` 에 wrapper 함수 신설.
3. **목표 소스를 고르는 로직이 새로 필요하다** — 이게 후보 ①의 숨은 범위다. 현재
   F-16.1 은 "OS 가 이미 등록한 `⌃Space` 토글"을 재주입만 할 뿐, 어떤 두 소스를 토글하는지
   **우리 설정에 전혀 없다**(`korean-input.md` §3.3 — 조건·출력 모두 "space+control 재주입"
   뿐, source id 개념 없음). 직접 전환을 구현하려면: 현재 소스가 한국어인지(`korean.rs` 의
   `classify_input_source_languages` 재사용 가능, F16 근거) 판정 → 아니면 "인풋 박스가
   기대하는 한국어 소스"로, 맞으면 "폴백 ASCII 소스"로 전환하는 새 결정 로직을 추가해야 한다.
   재사용 가능한 조각(F16 판정)은 있지만, "정확히 이 두 소스 사이를 토글"이라는 새 개념은
   프로젝트에 없다.

### ⚠️ 치명적 리스크 (§0 의 macOS CJK 버그)

- 우리가 전환하려는 목표가 **정확히 한국어 IME**다 — Karabiner 공식 문서·독립 보고 3건이
  경고하는 "input_mode_id 가 있는 CJK(V) 소스로 `select_input_source`" 케이스에 **정확히
  해당**한다. 보고된 실패 양상("아이콘은 바뀌는데 실제 입력은 안 바뀐다")은 **이번 이슈의
  증상(여러 번 눌러야 한다)보다 사용자 경험상 더 나쁠 수 있다** — 지금은 "안 눌린 것처럼
  보이는" 실패지만, 후보 ①의 실패 양상은 "전환된 것처럼 보이는데 실제로는 이전 언어로
  타이핑되는" 조용한 오분류다. 유닛 테스트로 이 macOS 버그를 재현·검증할 수 없다(순수
  플랫폼 동작) — 채택 시 실기기에서만 드러나는 리스크를 새로 들이는 셈이다.
- `language-presets.md` §8 의 부정형 수용 기준("select_input_source 류가 코드에 존재하지
  않는다")은 **Seek 인풋 박스에 국한된 결정이 아니라 F-16/F-19 전체(한국어·언어별 프리셋)에
  걸친 저장소 전역 결정**이다. 이슈 #129 위임의 "예외"는 "select_input_source 직접 호출
  금지"를 **이 위임 범위(인풋 박스 세션)에 한해 재검토**할 권한만 준다 — 채택 시
  `language-presets.md` §8 자체를 인풋 박스 한정 예외로 다시 쓰는 명세 작업이 추가로
  필요하고, 그 문서가 지목하는 근본 버그(CJK 전환 버그)는 이번 이슈로 사라지지 않는다.

### 성능/안정성 리스크 (탭 콜백 예산)

- `TISSelectInputSource` 는 입력기 프로세스에 분산 알림을 보내는 **무거운 동기 호출**이다.
  이 저장소는 이미 `engine.rs` 의 caps lock 경로(`toggle_caps_lock_via_path_c`)에서 "콜백
  안에서 직접 실행 가능"의 기준을 **"mach 메시지 한 번, 마이크로초 단위"**로 명시하고
  워커 스레드 큐잉조차 기각한 전례(`architecture.md` §6.6)가 있다 — `TISSelectInputSource`
  는 이 기준을 만족한다는 근거가 없다(입력기 IPC 왕복은 마이크로초가 아니라 밀리초
  단위일 수 있다). 탭 콜백(`on_tap_event`) 안에서 동기 호출하면 macOS 의 탭 콜백 시간
  예산을 넘겨 `TapDisabledByTimeout` 을 유발할 위험이 있다 — **이 경우 인풋 박스뿐 아니라
  전체 리매핑이 일시 중단되는, 지금보다 범위가 넓은 회귀**다.
- 안전하게 하려면 기존 `CommandChannel`/`EngineCommand`(`RecoverTap` 이 쓰는 "콜백 안에서는
  큐잉만, 실제 처리는 콜백이 아닌 자리에서" 패턴)를 재사용해 탭 콜백 밖에서 호출해야 한다
  — 이는 `Effect` 모델에 새 변형 추가 + core→engine→platform 3개 크레이트에 걸친 배선이라,
  "인풋 박스 세션 한정 수정"치고는 손이 크다.

### 판정 (P1 권고, P2 확정 대상)

**채택 비권고.** 근거: (a) 목표가 이미 문서화된 macOS CJK 버그의 정확한 트리거 조건과
일치하고, 그 버그의 실패 양상이 현재 증상보다 조용히 더 나쁠 수 있으며 유닛 테스트로
막을 수 없다 (b) 프로젝트 전역 결정(`language-presets.md` §8)과 정면 충돌해 이슈 #129
범위를 넘는 명세 정정이 필요하다 (c) 안전한 구현(탭 콜백 밖 디스패치)은 범위가 "새 이벤트
탭/레이어/FSM 금지" 제약과 긴장 관계에 있는 새 배선을 요구한다. **P2 가 "select_input_source
금지"를 재검토한다면, 이 근거(§0·§2)를 뒤집을 새 증거(예: 우리 표적 macOS 버전에서 그
CJK 버그가 실측으로 재현되지 않음을 확인)가 있어야 한다** — 이 P1 조사는 그 반증을 찾지
못했다.

---

## 3. 후보 ② — 합성 `⌃Space` 의 방출 경로/타이밍 변경

### 현재 경로가 이미 "물리 키 입력과 같은 급"이라는 점

`post_to_tap`(`CGEventTapPostEvent`)은 "우리 탭보다 뒤에 이벤트를 주입"한다(`event.rs:193-196`
문서). 우리 탭이 설치되는 위치(HID 근접부, `architecture.md` §6.1 계열)를 감안하면, 이
합성 이벤트는 이미 **시스템 전역 단축키 매칭을 포함해 물리 키 입력과 거의 동일한 하류
파이프라인**을 통과한다. `post()`(`CGEventPost` at `SessionEventTap`)로 바꿔도 이 파이프라인
구간 자체는 달라지지 않는다 — **탭 위치를 바꾸는 것만으로는 이론적 근거가 약하다.**

### 그래서 실제로 바뀔 수 있는 변수는 "그 순간의 포커스/윈도우 상태"

이슈 본문이 특정한 흡수 지점("검색 바 WKWebView `<input>` 이 키 윈도우일 때")이 맞다면,
개입 지점은 이벤트 자체가 아니라 **그 순간 어떤 윈도우가 키 윈도우인가**다. 후보:

- **②-a: 합성 방출 직전 검색 바의 키 윈도우 자격을 일시 해제 후 복원.**
  `overlay.rs` 의 `make_search_bar_key_for_input`/`release_search_bar_input` 패턴이 이미
  메인 스레드 디스패치로 `set_can_become_key`/`make_key_and_order_front` 를 수행한다 —
  구조적으로 재사용 가능한 자리는 있다. 다만 **탭 콜백은 메인 스레드가 아니고, 콜백
  안에서 메인 스레드 왕복을 기다릴 수 없다**(락·블로킹 IPC 금지, `architecture.md` §2.2)
  — 합성 이벤트 방출과 "키 윈도우 해제→복원"의 순서를 콜백 밖에서 비동기로 조율해야 한다.
  타이밍이 틀어지면(예: 웹뷰가 아직 키 윈도우인 채로 이벤트가 도착) 아무 효과가 없고,
  반대로 과하면(너무 오래 키 윈도우를 뺏으면) 사용자가 다음 문자를 입력할 때 포커스가
  없어 `is_input_box_pass_key` 문자 통과가 깨질 수 있다 — **인풋 박스 밖 동작은 안
  건드리지만, 인풋 박스 안의 "문자 통과" 경로(§4 회귀 대상)와 시간 경합**이 생긴다.
- **②-b: 웹뷰 자체가 조합 상태(`compositionstart`)일 때만 개입.** 이슈 본문의 (미확정)
  M7 이 "조합 중 vs 조합 없음 전환의 동작 차이"를 실기기로 확인하도록 이미 설계돼 있다 —
  만약 M7 이 "조합 중에만 흡수된다"를 확정하면, 개입을 조합 종료 신호(`compositionend`)
  이후로 지연하는 좁은 수정이 가능해진다. **이 결정은 실기기 M7 없이는 코드만으로 내릴 수
  없다.**

### 판정

②는 ①과 달리 **기존 프로젝트 결정(⌃Space 재주입)의 틀 안에서 개선**하는 방향이라 원칙적
충돌이 없고, macOS CJK 버그도 트리거하지 않는다. 하지만 **정확한 개입 지점(②-a 의 타이밍
vs ②-b 의 조합 상태)은 실기기 M-절차 없이 이 세션에서 확정할 수 없다** — §0 요약의 F17과
같은 한계다. 구현 리스크(②-a 의 메인 스레드 왕복 조율)도 작지 않다.

---

## 4. 조사 중 발견한 제3의 후보 — 관측 기반 유계 재시도 (신규 제안, P2 판단 요청)

기존 두 후보 모두 "왜 흡수되는가"를 알아야 정밀하게 고칠 수 있는데, 그 메커니즘은 이
세션에서 `(미확정)`이다. 반면 트레이스 자체(6:1, 세션 밖은 1:1)는 이미 **통계적 실패
패턴**을 보여준다 — 대부분 흡수되지만 이따금 통과한다. 왜 흡수되는지 몰라도 **"발화 후
전환이 실제로 일어났는지 관측하고, 안 일어났으면 다시 낸다"**는 보정은 가능하다.

- 이미 존재하는 조각: `text_input_source::observe_input_source_changes`(`kTISNotify…` 구독,
  §1 F16) + `RepeatingTimer`(quick press 타이머가 이미 쓰는 패턴, `engine.rs:1061`) +
  `CommandChannel`(콜백 밖 처리 패턴, `RecoverTap` 이 이미 씀).
- 아이디어: D2 발화(합성 ⌃Space 방출) 직후 짧은 유계 창(예: 수십~1~2백 ms, 실측 필요)
  안에 `kTISNotifySelectedKeyboardInputSourceChanged` 가 오지 않으면, **콜백 밖에서**
  (`apply_outcome_outside_tap` 이 이미 쓰는 `post()` 경로로) 합성 ⌃Space 를 한 번 더
  낸다 — 관측될 때까지 유계 재시도(예: 최대 2~3회, 그 이상은 포기하고 정상 실패로 둔다).
  `TISSelectInputSource` 를 전혀 호출하지 않으므로 §2 의 CJK 버그·명세 충돌을 모두
  피한다.
- ⚠️ 자체 인지 리스크: (a) "유계 재시도 타이머+관측"은 "새 이벤트 탭/레이어/FSM 금지"
  제약에 저촉되는지 경계선에 있다 — 이것이 새 FSM 인지, 아니면 기존 P11(경로 C 콜백 밖
  처리)·이슈 #19 트레이스처럼 "진단/보정 유틸리티"로 볼 수 있는지는 P2 가 판단해야 한다.
  (b) 세션 밖(1:1 로 이미 정상 동작하는 경로)에는 이 재시도를 켜면 안 된다 — 인풋 박스
  세션 한정 게이트가 반드시 필요하다(위임 계약과 일치는 하지만 구현 실수 여지). (c) 사용자가
  전환 직후 빠르게 타이핑을 재개하면, 지연된 재시도 ⌃Space 가 뒤늦게 나가 **의도치 않은
  재전환**(전환→전환)을 일으킬 수 있다 — 재시도 시작 전 "이미 전환됐는지"를 다시 확인하는
  가드가 필요하다.
- 이 후보는 이슈 #129 델리게이션 문서가 제시한 두 후보(①/②) 목록에는 없다 — P1 이 조사
  중 발견한 제3안이다. P2 가 ①/②/③ 중 선택하거나, 서로 조합(예: ②-b 로 좁히되 안전망으로
  ③ 병행)하는 방향도 열려 있다.

---

## 5. P1 권고 (확정 아님 — P2 대상)

1. **후보 ①(직접 전환)은 채택 비권고** — §2 의 macOS CJK 버그 근거가 강하고, 실패 시
   사용자 경험이 지금보다 나빠질 수 있으며 유닛 테스트로 방어 불가.
2. **후보 ②(방출 경로/타이밍)와 후보 ③(관측 기반 유계 재시도)이 유력** — 둘 다
   `select_input_source` 금지를 건드리지 않는다. ②는 정밀하지만 실기기 확정 없이는 정확한
   개입 지점을 모른다(§3). ③은 메커니즘을 몰라도 통계적으로 동작할 수 있지만 "새 FSM
   금지"와의 경계·재전환 레이스 리스크가 있다(§4).
3. P2 가 확정할 것: (a) ①을 완전히 배제할지, 배제한다면 그 판단을 명세(`language-presets.md`
   §8 또는 이 문서)에 남길지 (b) ②/③ 중 하나 또는 조합을 목표 메커니즘으로 확정 (c) "인풋
   박스 세션 D2 발화 = 정확히 1 전환"을 무엇으로(유닛/통합 가능 범위 + 실기기 절차)
   판정할지 수용 기준을 못박기 (d) 저장소에 `ultrakey-review` 서브에이전트/스킬이 있으면
   그 리뷰도 받을 것(위임 계약).

---

## 6. 기각·보류 메모 (초안)

| 항목 | 처리 |
| :--- | :--- |
| `TISSelectInputSource` 직접 호출(후보 ①) | **비권고** — §2. 최종 판단은 P2. |
| 새 이벤트 탭 설치 | 검토 안 함 — 위임 계약 금지, ②/③ 모두 기존 탭·콜백·타이머 재사용으로 충분 |
| 탭 위치(`SessionEventTap`↔다른 위치)만 바꾸는 안 | 기각 — §3, 파이프라인 구간이 이미 동일해 이론적 근거 약함 |

---

## 7. P2 확정 (상급 리뷰)

> **성격**: 이 절은 P2(상급)의 **확정**이다 — §0~§6 의 P1 초안 권고를 검증·정정하고,
> P1·P3 가 그대로 이어받아 구현에 들어갈 방향·수용 기준을 못박는다. 저장소에
> `ultrakey-review` 서브에이전트는 **이 하네스에서 호출 불가**하므로(`.claude/agents/` 에는
> `ultrakey-explore`·`ultrakey-implement`·`ultrakey-plan` 뿐이고 `ultrakey-review.md` 는
> `.opencode/agent/` 전용), P2 가 그 비판적 리뷰 역할을 겸했다 — 아래 §7.2 가 그 결과다.
> 형식 선례: `docs/plan/issue-121-seek-shiftspace-tap.md` §11.

### 7.1 판정 요약

| # | 확정 사항 | 판정 |
| :--- | :--- | :--- |
| ① | `TISSelectInputSource` 직접 전환 | ⛔ **기각** — 채택하지 않는다(§7.3). 재개 조건은 §7.6 에 못박는다 |
| ② | 합성 `⌃Space` 의 **방출 경로/타이밍** 변경 | ✅ **확정 — 유일한 주력 방향**. 구체안 순위는 §7.4 |
| ③ | 관측 기반 유계 재시도 | ⛔ **비채택**(안전망으로도 병행하지 않는다) — §7.3 |
| — | "`select_input_source` 직접 호출 금지" | ✅ **유지**. 재검토는 수행했고, 트레이스 증거는 이 금지의 근거를 **건드리지 않는다**(§7.3-A). `docs/spec/language-presets.md` §8 은 **무수정** |

**한 줄 확정**: 인풋 박스 세션의 D2 **판정 로직은 한 줄도 건드리지 않고**(#125·#127 가드·래치
그대로), 그 결과로 나온 `SynthEvent` 의 **전송(transport)만** 인풋 박스 세션 게이트 안에서
바꾼다. 그리고 그 전에 **M-129-1(물리 `⌃Space` 실기기 확인)을 먼저 실행한다** — 이 한 번의
관측이 ②가 유효한지 무의미한지를 결정적으로 가르기 때문이다(§7.5).

### 7.2 상급 리뷰 — P1 초안에 대한 반증·정정 (근거 재검증 결과)

P1 의 두 인용은 **원문 대조로 정확함이 확인**됐다(오히려 과소 인용이다):

- `docs/spec/language-presets.md` §8 에 문자 그대로 존재한다 — "**CJK 직접 전환 금지**: 코드에서
  `select_input_source` 류 구현(또는 `TISSelectInputSource` 직접 호출)이 **존재하지 않는다**
  (부정형 — 연구 §4-1 버그 근거)".
- `docs/research/ke-complex-modifications-analysis.md` §4-1 의 CJK 버그 표(Karabiner 1차 공식
  문서 + 독립 실사용 보고 3건: `lawrence-lo/input-source-change`, `kage2kapp.org`(macOS 26
  실측), `v2ex.com/t/565667`(zh 커뮤니티))도 정확하다. **P1 이 인용하지 않은 결정적 한 줄**:
  같은 절이 "이 때문에 아래 §5 의 전환 프리셋은 전부 **`⌃Space` 재주입(F-16.1 과 같은 방식)**
  을 쓰고 `select_input_source` 를 배제한다"고 명시한다 — 즉 **F-16.1 이 `⌃Space` 재주입을
  쓰는 것 자체가 이 버그의 산물**이며, 후보 ①은 그 결정을 정확히 되돌리는 것이다.

그 위에서 P2 가 **새로 찾은 사실 3건**이 후보 비교를 바꾼다.

| # | 리뷰 발견 | 근거 | P1 초안에 대한 영향 |
| :--- | :--- | :--- | :--- |
| **R1** ⭐ | **"웹뷰가 키 윈도우"만으로 `⌃Space` 가 흡수되는 것이 아니다** — 저장소는 **같은 상태에서 동작한다고 명세된 `⌃Space` 경로를 이미 갖고 있다**: 인풋 박스 세션 중 **물리** `⌃Space` 는 D1 이 원본 통과시키고 시스템(Carbon HIToolbox)이 전환한다(이슈 #101 경로 ②). 즉 흡수는 **합성 이벤트에 고유한** 현상일 가능성이 크다 | `docs/spec/seek-activation-and-session.md` §3.1 ②·§4 표 116행·수용 기준 218행 · `arbitration.rs:552-554`(D2 **뒤**에 D1 통과) · `seek_input_box.rs:87-91` `is_input_source_switch_shortcut` = `SPACE && CONTROL && !COMMAND`(⇧ 는 가리지 않음) | **§3 판정 정정.** P1 은 ②를 "이론적 근거가 약하다"고 낮췄으나, R1 은 ②에 **구체적 표적**을 준다: "합성 방출을, 같은 상태에서 이미 통하는 물리 경로에 최대한 가깝게 만든다". ②가 주력이 되는 근거 |
| **R2** ⭐ | **P1 의 ②-a(합성 직전 키 윈도우 자격 일시 해제)는 하드 기각이다** — 검색 바가 키 윈도우 자격을 잃으면(`didResignKey`) 인풋 박스 세션은 `CloseReason::Defocused` 로 **자동으로 닫힌다**(이슈 #93 설계). ②-a 는 고치려는 그 세션을 죽인다 | `docs/spec/seek-activation-and-session.md` §4 표 119행 · §5 시나리오 12 · `overlay.rs:525-543` `release_search_bar_input`(`set_can_become_key(false)`) | §3 ②-a **기각 확정**. P1 이 "타이밍이 어렵다"로 남긴 것을 "구조적으로 불가"로 승격 |
| **R3** | **후보 ③의 실패 양상은 후보 ①과 같은 부류다.** 연구 §4-1 의 1차 출처가 한국어 입력 소스 전환에 **가변 지연이 문서화**돼 있음을 보인다("딜레이가 생기거나, 제대로 바뀌지 않는 고질적인 문제"). 재시도 창이 그 지연보다 짧으면 **되토글**(전환→재전환 = 원래 언어)이 나고, 이는 이슈 #127 이 실측한 실패 양상(208ms 뒤 Korean→ABC 되돌아감)과 같다 — 즉 ③은 지금의 **큰 실패**("안 눌린다")를 **조용한 실패**("전환된 줄 알았는데 이전 언어로 타이핑")로 바꾼다 | `docs/research/…§4-1` 첫 표 1행 · `docs/spec/korean-input.md:95`(이슈 #127 실측) | §4 의 "안전망으로 병행" 여지를 **닫는다**. ①을 기각하는 바로 그 논거(조용한 오분류)가 ③에도 그대로 적용된다 |

부수 확인(코드 실측, P1 §1 F13~F16 은 전부 사실로 재확인):

- F-16.1 의 출력은 `EventFlags(0x0004_0001)` = `CONTROL | NX_DEVICELCTLKEYMASK`
  (`crates/ultrakey-korean/src/settings.rs:118-129`) — **이미 "왼쪽 control 디바이스 비트"까지
  실어 물리 이벤트에 가깝게 만든 형태**다. 그리고 **이 동일한 shape 가 세션 밖에서는 1:1 로
  동작한다**(이슈 본문 대조군). ⇒ **이벤트 shape 는 시스템 핫키 단계에 이미 충분하다**는
  반증이 서 있으므로, "shape 를 더 물리답게 바꾼다"(FlagsChanged 동반 등)는 안은 **근거가
  약하다** — §7.4 에서 후순위로 내린다. 남는 변수는 shape 가 아니라 **수신 맥락·주입 시점**이다.
- `apply_outcome_outside_tap`(`engine.rs:401-409`, `post()` = `CGEventPost` at
  `SessionEventTap`) · `RepeatingTimer`(`engine.rs:1059-1066`, 탭 스레드) ·
  `CommandChannel`/`EngineCommand`(`RecoverTap` 선례, `engine.rs:605-611` → `drain_commands`
  `engine.rs:920`) 이 **이미 존재하고 검증돼 있다**. 탭 콜백·타이머 콜백·커맨드 perform 은
  **같은 탭 스레드에서 직렬로만** 호출된다(`command.rs:3-11`) — 콜백 밖 방출로 옮겨도
  KeyDown→KeyUp 순서가 스레드 경합으로 뒤집히지 않는다.
- ⚠️ **테스트 지형 (P3 가 반드시 알아야 할 제약)**: `ultrakey-engine` 크레이트에는
  `#[cfg(test)]` 모듈이 **하나도 없다**. F-16.1·인풋 박스 커버리지 **전량**(16종 + D1 통과 7종)이
  `crates/ultrakey-core/src/arbitration.rs` 인라인 테스트에 있고, 전부 `Outcome`/`SynthEvent`
  **수준에서 멈춘다**. 즉 **"어떤 SynthEvent 를 내는가"는 CI 로 고정되지만 "그것을 어떻게
  방출하는가"는 CI 로 고정할 수단이 현재 없다** — 이것이 §7.5 에서 M-절차의 몫을 키우는 이유다.

### 7.3 각 후보 판정과 근거

**A. `select_input_source` 금지 — 유지 (재검토 수행함).**

위임 계약이 허용한 재검토를 실제로 수행했고, 결론은 **유지**다. 근거:

1. **증거의 사정거리**: 6:1 트레이스는 "**현재의 방출이 실패한다**"는 증거이지 "`TISSelectInputSource`
   가 동작한다"는 증거가 **아니다**. 금지의 근거(연구 §4-1 의 CJK 전환 버그)는 이 트레이스로
   조금도 약해지지 않는다 — 두 사실은 서로 독립이다. "증거 이전의 결정"이라는 사유만으로
   뒤집을 수 있는 결정이 아니다.
2. **실패 양상의 방향이 나쁘다**: 지금은 **큰 실패**(아무 일도 안 일어남 → 사용자가 다시 누름).
   ①의 문서화된 실패는 **조용한 실패**(아이콘은 바뀌고 실제 입력은 이전 언어) — 그것도
   **텍스트 입력 표면**에서. 검색어가 조용히 엉뚱한 언어로 들어가는 쪽이 UX 상 더 나쁘고,
   순수 플랫폼 동작이라 **CI 로 방어할 수단이 없다**.
3. **부분 채택도 불가(P2 신규 분석)**: KE 문서가 경고하는 트리거는 "**`input_mode_id` 가 있는
   CJKV 소스로** 전환"이다. 한국어→ABC 방향은 그 트리거 **밖**이지만 ABC→한국어 방향은
   정확히 **안**이다. 인풋 박스는 양방향이 다 필요하므로 "한 방향만 TIS" 하이브리드는 성립하지
   않는다 — 방향마다 메커니즘이 다르면 신뢰도가 더 낮아진다.
4. **제재된 메커니즘을 아직 소진하지 않았다(R1)**: 같은 실패 상태에서 동작한다고 명세된
   `⌃Space` 경로가 남아 있다. 그것을 소진하기 전에 금지된 API 로 가는 것은 이르다.
5. **범위**: 채택 시 `language-presets.md` §8 의 **전역 부정형 수용 기준을 인풋 박스 한정
   예외로 다시 쓰는 명세 작업** + 목표 소스 선택 로직 신설(명세에 개념 자체가 없다) +
   콜백 밖 디스패치 배선(`Effect` 신규 변형, 3개 크레이트)이 따라온다. "인풋 박스 세션 한정
   수정"의 범위를 넘는다.

⇒ **`docs/spec/language-presets.md` §8 은 이번 이슈에서 수정하지 않는다.** 이 P2 판정은
§8 을 다시 쓸 권한을 **행사하지 않았고**, P3 에게도 주지 않는다. (뒤집었다면 §8 정정이
필수였다는 점을 위임 계약대로 여기에 명시한다.)

**B. 후보 ③ — 비채택.** R3 이 근거다. 추가로: (a) 재시도 창을 정하려면 전환 지연 분포를
실측해야 하는데(M-129-3) 그 실측 없이 창을 고르는 것은 추측이다 (b) "타이머 + 알림 관측 +
재확인 가드"는 외부 지연이 **무계**인 대상에 대한 상태 기계라 CI 로 정합성을 고정할 수 없고
(§7.2 테스트 지형), 위임 계약의 "새 FSM 금지" 경계에서 **금지 쪽**이라고 판정한다 — P1 이
P2 판단을 요청한 지점에 대한 답이다. (c) 메커니즘을 모른 채 증상을 덮는 안인데, M-129-1 이
메커니즘을 **한 번의 키 입력으로** 좁혀 주므로 그 전에 채택할 이유가 없다.

**C. 후보 ② — 확정.** 위임 계약과 충돌하는 지점이 없고(새 탭/레이어/FSM 없음, 인풋 박스
게이트 안, 판정 로직 무변경 ⇒ #106/#120/#122/#126/#128 테스트 전량 무변경 통과), CJK 버그를
트리거하지 않으며, R1 이 표적을 준다.

### 7.4 확정 메커니즘 — P3 구현 계약

**불변식 (전부 필수)**

1. `evaluate_korean_rules`(D2·세션 밖 공용)의 **판정 로직·조건·래치는 무변경** — #125 의
   `active_synth_flags_of_pressed_slots` 가드, #127 의 autorepeat 억제 그대로.
2. 변경은 **`gates.seek_input_box == true` 게이트 안**에서만. 세션 밖 F-16.1·F-16.2 경로,
   영어 단일 세션은 **한 바이트도 바뀌지 않는다**.
3. **금지**: 키 윈도우 자격 조작(R2) · `TISSelectInputSource`/`select_input_source` 도입(§7.3-A) ·
   새 이벤트 탭/레이어/FSM · `CHANGELOG` 수정 · `docs/spec/language-presets.md` §8 수정.
4. 탭 콜백 임계 경로 불변식 유지 — 락·힙 할당·블로킹 IPC 없음(`architecture.md` §2.2).

**구체안 순위** (각 안은 **독립적으로 실기기 측정**한다 — 쌓아서 한 번에 재지 말 것)

| 순위 | 안 | 내용 | 근거 / 리스크 |
| :--- | :--- | :--- | :--- |
| **1** | **②-c1 — 콜백 밖 방출** | 인풋 박스 세션에서 D2 가 낸 `SynthEvent` 를 `apply_outcome_in_tap`(`post_to_tap`, 콜백 안) 대신 **기존 콜백 밖 경로**(`apply_outcome_outside_tap` = `post()`, 커맨드 perform/타이머 — `RecoverTap` 선례)로 방출. 물리 space 이벤트의 `TapAction` 은 종전대로 Consume | R1 이 남긴 유일한 변수(**수신 맥락·주입 시점**)를 직접 건드리는 안. 현재는 물리 shift+space 를 처리하는 **바로 그 콜백 스택 프레임 안에서** 합성 이벤트를 밀어 넣는다 — 물리 `⌃Space`(단독 코드, 동시 처리 중인 물리 space 없음)와 다른 유일한 조건이다. **기존·검증된 인프라 재사용**(신규 개념 0). ⚠️ KeyDown→KeyUp 순서 보존이 필수 불변식(탭 스레드 단일 직렬화가 근거 — §7.2) |
| **2** | **②-c2 — 주입 위치** | `post()` 의 `CGEventTapLocation::SessionEventTap` 을 **`HIDEventTap`** 으로(인풋 박스 경로 한정). 1줄 변형, ②-c1 위에 직교로 얹힌다 | P1 이 §3·§6 에서 "탭 위치만 바꾸는 안"으로 **기각**한 것을 **부분 복원**한다 — P1 이 검토한 것은 `SessionEventTap`↔우리 탭 위치(동일 구간)였고, **HID 레벨(세션 파이프라인 전체보다 상류) 재주입은 검토되지 않았다**. `is_ultrakey_synthetic` 마커가 재진입을 막는다(`event.rs:204-207`) |
| **3** | **②-d — 이벤트 shape** | 합성 space 앞뒤에 control `FlagsChanged` 를 동반(`make_synth_event` 가 `EventKind::FlagsChanged` → `SyntheticEvent::flags_changed` 를 이미 지원) | ⚠️ **근거 약함** — 동일 shape 가 세션 밖에서 1:1 로 동작한다(§7.2). 게다가 `arbitration.rs:2641` 등이 현재 shape 를 단언하므로 **기존 테스트 수정이 발생**한다. 1·2 가 실패한 뒤에만 |
| **4** | **②-b — 조합 상태 게이팅** | 웹뷰 `compositionend` 이후로 방출 지연 | **M-129-4(=#121 M7) 결과에 종속** — 그 실측 없이 착수 금지 |
| ⛔ | ②-a — 키 윈도우 일시 해제 | — | **기각**(R2 — 세션 자동 닫힘) |

### 7.5 목표점 · 수용 기준

**목표점**: 인풋 박스 세션에서 F-16.1 이 발화할 때 **방출 : 실제 입력 소스 전환 = 1:1**
(현재 6:1). 세션 밖·기본 Seek 는 비회귀.

| # | 수용 기준 | 고정 수단 |
| :--- | :--- | :--- |
| **A1** ⭐ | 인풋 박스 세션에서 ⇧+Space 를 연속 N회(N ≥ 6) 눌렀을 때 `ULTRAKEY_TRACE_TAP=1` 의 `emitted=[KeyDown 0x31 0x40001]` 횟수와 `kTISNotifySelectedKeyboardInputSourceChanged` 횟수가 **같다**(N:N) | **실기기 M-129-2** `(미확정)` — CI 불가 |
| **A2** | 전환 직후 타이핑이 **실제로 전환된 언어로** 들어간다(아이콘만 바뀌는 조용한 실패 부재) | **실기기 M-129-6** `(미확정)` |
| **A3** | 세션 중 ⇧+Space 로 검색어에 **공백이 들어가지 않는다** | 실기기 M-129-5 + `arbitration.rs` 기존 테스트(Consume 단언) |
| **A4** | `arbitration.rs` 의 F-16.1·인풋 박스 테스트 16종과 D1 통과 테스트 7종이 **무변경으로 통과**한다(순위 1·2 안은 `SynthEvent` shape 를 바꾸지 않으므로 성립해야 한다). 대표: `input_box_f16_1_shift_space_substitutes_control_pair`·`issue125_stale_hyper_after_session_release_no_longer_blocks_f16_1`·`issue127_input_box_shift_space_autorepeat_synthesizes_keydown_exactly_once`·`issue121_user_settings_shift_space_fires_on_every_tap_in_input_box`·`input_box_f16_1_control_space_still_passes_d1` | **CI** — `cargo test --workspace` |
| **A5** | 세션 **밖** ⇧+Space·한/영 키(F-16.1/F-16.2)는 1:1 그대로 | CI(계층 3 테스트) + 실기기 M-129-5 |
| **A6** | 인풋 박스 문자 통과(`is_input_box_pass_key`)·물리 `⌃Space` D1 통과·세션 토글 소비가 비회귀 | CI + 실기기 M-129-5 |
| **A7** | 수정으로 인해 세션이 **자동으로 닫히지 않는다**(`CloseReason::Defocused` 미발생) — R2 대응 | 실기기 M-129-5 (로그) `(미확정)` |
| **A8** | `TapDisabledByTimeout`(탭 비활성화) 이 발생하지 않는다 | 실기기 M-129-5 (로그) `(미확정)` |
| **A9** | 순위 1 안 채택 시 **KeyDown→KeyUp 방출 순서가 보존**된다 | ⚠️ `ultrakey-engine` 에 테스트 모듈이 없어 **CI 고정 불가**(§7.2). 코드 리뷰 불변식 + M-129-2 트레이스 순서 확인 `(미확정)`. P3 가 엔진 계층에 최소 테스트를 신설할 수 있으면 신설이 바람직하나 **필수 요건은 아니다** |
| **A10** | 코드에 `TISSelectInputSource`·`select_input_source` 심볼이 **계속 존재하지 않는다**(§8 부정형 기준 유지) | **CI/grep 으로 고정 가능** — 현재 코드 히트 0(실측) |
| **A11** | `cargo test --workspace` · `cargo clippy --workspace --all-targets -- -D warnings` 통과 | CI |
| **A12** | `docs/spec/seek-activation-and-session.md` §3.1 ③ 과 `docs/spec/korean-input.md` §3.1 에 이번 결정 기록(전송 변경 + 6:1 실측 + ①/③ 기각 근거)을 남긴다. **`language-presets.md` §8 은 무수정** | 문서 리뷰 |

**실기기 절차 (M-129) — 이슈 본문 체크리스트의 "실기기 절차" 항목을 대체·구체화**

| # | 절차 | 판정 |
| :--- | :--- | :--- |
| **M-129-1** ⭐⭐ **선행·결정적** | 인풋 박스 세션(검색 언어 ko) 중 **물리 `⌃Space`** 를 직접 1회 누른다. (= 이슈 #121 §8 **M6 — 아직 실행되지 않았다**) | **전환됨** → 흡수는 **합성 고유** ⇒ R1 성립 ⇒ §7.4 순위 1·2 로 진행. **전환 안 됨** → 이 상태에서 `⌃Space` 메커니즘 **자체가 죽어 있다** ⇒ ② 전량·③ 모두 무의미 ⇒ §7.6 의 ① 재개 조건 (i) 충족 **동시에** 이슈 #101 수용 기준(`seek-activation-and-session.md` 218행)이 **불성립**임을 별도 결함으로 기록 |
| **M-129-2** | 수정 적용 후 `ULTRAKEY_TRACE_TAP=1` 로 ⇧+Space 연속 6회 이상 | A1(6:1 → N:N). 트레이스에서 KeyDown→KeyUp 순서도 함께 확인(A9) |
| **M-129-3** | (③ 를 재개할 때만) 방출→`kTISNotify…` 지연 분포 측정 | ③ 의 재시도 창 상한 근거. 지금은 실행 불필요 |
| **M-129-4** | 한글 IME **조합 중**(`compositionstart` 직후) vs 조합 없음 상태의 ⇧+Space 차이 (= #121 M7) | ②-b 착수 여부의 유일한 입력 |
| **M-129-5** | 비회귀 묶음 — 세션 밖 ⇧+Space·한/영 키, 인풋 박스 문자 입력, 물리 `⌃Space`, 세션 토글 단축키, 세션 자동 닫힘 로그, 탭 비활성화 로그 | A3·A5·A6·A7·A8 |
| **M-129-6** | 전환 후 실제로 한글/영문이 **입력되는지**(아이콘 아닌 실입력) | A2 |

⚠️ **코드 변경 제로 결말도 유효한 결말이다**: M-129-1 이 "물리 `⌃Space` 도 전환 안 됨"으로
나오면 이 PR 은 **측정·기록 PR** 로 마무리한다 — 이슈 #118→#120, #121→#122 의 선례가 그대로
적용된다. 지어내서 고치지 말 것.

### 7.6 후보 ① 재개 조건 (구속력 있음)

아래 **전부**가 충족되기 전에는 `TISSelectInputSource` 를 도입하지 않는다.

1. M-129-1 이 "인풋 박스 세션에서는 물리 `⌃Space` 도 전환되지 않는다"를 보인다.
2. §7.4 순위 1·2 안을 **각각** 실기기에서 측정했고 A1 이 달성되지 않았다.
3. 표적 macOS 에서 `TISSelectInputSource` → 한국어 소스 전환을 **20회 이상 반복 실측**해,
   연구 §4-1 이 보고한 실패 양상(아이콘만 전환 / "切换多了会失效")이 **재현되지 않음**을
   확인한다 — P1 §2 가 요구한 "반증"의 구체적 형태다.
4. 채택 시 `docs/spec/language-presets.md` §8 의 부정형 수용 기준을 **인풋 박스 한정 예외로
   재작성**하고 `docs/research/ke-complex-modifications-analysis.md` §4-1 에 실측 주석을 단다.
   **이 P2 판정은 그 재작성을 승인하지 않았다** — 별도 위임이 필요하다.

### 7.7 반영 / 기각 내역

| P1 초안 항목 | P2 처리 |
| :--- | :--- | 
| §0·§2 — 후보 ① 비권고, `language-presets.md` §8 · 연구 §4-1 인용 | **반영(인용 정확성 원문 대조 확인) + 강화** — §7.3-A 에 근거 5종으로 확정. 연구 §4-1 의 "그래서 프리셋은 전부 ⌃Space 재주입을 쓴다" 한 줄과 "방향 비대칭(부분 채택 불가)" 논거를 추가 |
| §1 F13~F16 코드 근거 | **반영** — 전부 실측 재확인. F-16.1 출력이 `EventFlags(0x0004_0001)`(control + 좌control 디바이스 비트)임을 보강 |
| §1 F17 — 흡수 메커니즘 `(미확정)` | **반영, 단 범위 축소** — R1 로 "웹뷰 키 윈도우 ⇒ 흡수"는 성립하지 않게 됐다. 남은 `(미확정)`은 "합성 고유의 흡수인가"이며 M-129-1 이 그것을 가른다 |
| §3 — 후보 ② "이론적 근거 약함" 판정 | ⛔ **기각(정정)** — R1. ②는 유일한 주력 방향으로 승격 |
| §3 ②-a — 키 윈도우 일시 해제 | ⛔ **기각(승격)** — R2. "타이밍이 어렵다"가 아니라 **세션이 닫힌다**(구조적 불가) |
| §3 ②-b — 조합 상태 게이팅 | **반영, 순위 4** — M-129-4 종속. 착수 조건 명시 |
| §6 — "탭 위치만 바꾸는 안" 기각 | **부분 복원** — `SessionEventTap`↔우리 탭 위치 비교로는 기각이 옳으나, **HID 레벨 재주입**은 미검토였다. ②-c2(순위 2)로 되살림 |
| §4 — 후보 ③ 및 "새 FSM 경계선" P2 판단 요청 | ⛔ **비채택 확정** — R3(되토글 = 조용한 실패, ①을 기각한 논거와 동일) + 무계 외부 지연 대상의 상태 기계는 CI 고정 불가 ⇒ "새 FSM 금지"의 **금지 쪽**으로 판정 |
| §5-3(d) — `ultrakey-review` 리뷰 받기 | **반영(대체 수행)** — 이 하네스에서 호출 불가(`.claude/agents/` 미등재)하여 P2 가 겸했다. 산출물이 §7.2 의 R1~R3 |
| (신규) 테스트 지형 — `ultrakey-engine` 테스트 부재 | **추가** — A9 를 CI 로 고정할 수 없다는 제약을 수용 기준에 명시 |

---

## 8. P3 반영 확인 (구현 전 검증)

P2 §7 을 반영하기 전, P2 가 새로 주장한 두 핵심 근거(R1·R2)를 코드로 직접 재확인했다 —
P2 를 그대로 믿지 않고 검증한다는 원칙(§7.2 가 P1 에게 한 것과 같은 절차).

- **R1 재확인**: `arbitration.rs:552-554` — D2(F-16.1 재평가) 판정 **뒤**, 인풋 박스 세션
  안에서 `is_input_source_switch_shortcut` 이 성립하면(물리 `⌃Space`) `Outcome::pass`
  로 **원본 그대로 통과**시킨다. `seek-activation-and-session.md` §3.1 ②·수용 기준
  218행이 이 경로가 "실제 전환됨"으로 명세돼 있음을 확인 — R1 성립.
- **R2 재확인**: `overlay.rs:525-543` `release_search_bar_input` 이 `set_can_become_key(false)`
  를 하고, `seek-activation-and-session.md` §4 표 119행·§5 시나리오 12 가 검색 바
  `didResignKey` → `CloseReason::Defocused` **자동 종료**를 명세한다 — R2 성립(②-a 는
  구조적으로 불가).

두 근거 모두 원문 대조로 성립 확인 — **P2 §7 을 그대로 반영한다.** 반영 안 한 항목 없음.

## 9. P4 구현 (완료)

**적용한 안**: §7.4 순위 1(②-c1, 콜백 밖 방출)만 구현했다. 순위 2(②-c2, HID 레벨
재주입)는 순위 1 이 실기기에서 A1 을 달성하지 못할 때의 다음 단계로 남겨 둔다(§7.4 —
"각 안은 독립적으로 실기기 측정한다").

- **`ultrakey-core` 변경 없음** — `evaluate_korean_rules`·D2 판정·래치·가드 전부 그대로
  (§7.4 불변식 1). `arbitration.rs` 333개 테스트(F-16.1·인풋 박스 16종·D1 통과 7종·
  #121/#125/#127 포함) 전부 **무변경으로 통과**(A4).
- **`ultrakey-engine` 변경**:
  - `crates/ultrakey-engine/src/command.rs` — `EngineCommand::PostSynthEvent(SynthEvent)`
    추가(`RecoverTap` 과 같은 "큐잉만, 처리는 콜백 밖" 패턴).
  - `crates/ultrakey-engine/src/engine.rs` — `on_tap_event` 의 `apply_outcome_in_tap`
    직접 호출을 `emit_outcome`(신설)으로 교체. `emit_outcome` 은
    `should_defer_synth_to_command_queue(gates, outcome.layer())`(신설, 순수 함수)가
    참이면 `commands.send(EngineCommand::PostSynthEvent(*ev))`(콜백 밖 큐잉)로,
    아니면 종전대로 `apply_outcome_in_tap`(콜백 안 `post_to_tap`)으로 보낸다. 게이트는
    `gates.seek_active && gates.seek_input_box && outcome.layer() == Layer::KoreanInput`
    — §7.4 가 요구한 "인풋 박스 게이트 안에서만" 을 그대로 구현한다(불변식 2).
    `drain_commands` 에 `EngineCommand::PostSynthEvent(ev)` 처리 추가 — `make_synth_event`
    (기존 함수, 무변경) 로 변환 후 `.post()`(`apply_outcome_outside_tap` 과 같은 경로).
  - ⭐ **`ultrakey-engine` 최초의 `#[cfg(test)]` 모듈 신설**(§7.2 가 지적한 테스트 지형
    공백을 메운다, A9 처럼 필수는 아니었으나 P2 가 "신설이 바람직하다"고 명시) —
    `should_defer_synth_to_command_queue` 게이트 불변식 4종(인풋 박스 세션 D2 만 defer ·
    세션 밖 동일 레이어는 defer 안 함 · 세션은 열렸지만 인풋 박스 아니면 defer 안 함 ·
    인풋 박스 세션이라도 다른 레이어는 defer 안 함). `TapProxy`(탭 콜백 전용 불투명
    타입)가 필요 없는 순수 게이트 조건만 뽑아 테스트 가능하게 만들었다 — `emit_outcome`
    자체(전송)는 여전히 실기기로만 검증된다(A9 그대로 `(미확정)`).
- **검증**: `cargo test --workspace` 전체 그린(신규 4종 포함, 기존 스위트 전량 무변경
  통과) · `cargo clippy --workspace --all-targets -- -D warnings` 0 경고.
  `TISSelectInputSource`/`select_input_source` 코드 히트 0(A10, `grep` 재확인).
- **명세 갱신**: `seek-activation-and-session.md` §3.1(이슈 #129 항목, ①/③ 기각 근거
  포함) · `korean-input.md` §3.1(같은 결정을 F-16 규칙 문맥에서 요약, 상호 참조).
  `language-presets.md` §8 은 **무수정**(P2 판정 그대로).
- **실기기로만 확인 가능(`(미확정)`, M-129 절차로 넘김)**: A1(6:1→N:N 실제 전환)·
  A2(조용한 오분류 부재)·A7(세션 자동 닫힘 없음)·A8(탭 타임아웃 없음)·A9(콜백 밖
  방출에서도 KeyDown→KeyUp 순서 보존 — 코드 구조상 성립해야 하나 CI 로 고정 불가).
  **M-129-1(물리 `⌃Space` 가 인풋 박스 세션에서 실제로 전환되는지)이 선행·결정적
  절차다** — 이 결과가 "전환 안 됨"이면 이 PR 의 전송 경로 변경 자체가 무의미해지고
  별도 결함(§7.5 M-129-1)으로 이어진다. PR 본문에 절차를 그대로 옮긴다.

