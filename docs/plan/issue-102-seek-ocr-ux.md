# 이슈 #102 — Seek 검색창이 OCR 동안 멈춘 듯 보이는 UX (창은 띄우고 검색어 입력은 받되 OCR 완료 시 트리거) — 설계 초안 + 작업 분해

> **성격**: `docs/plan/` 은 구현 위임 전 **계획 문서**를 두는 자리다. 이 문서는 중급(계획 초안)이 작성한 **초안**이며, 확정은 상급(호출 세션·`ultrakey-review`)의 리뷰 후 이루어진다. 리뷰가 끝나면 "결정 목록(D1~D5)"이 정본이 되고, §6 의 명세 갱신 초안이 `docs/spec/` 에 반영된 뒤 구현이 진행된다.
>
> **성격 2**: 이슈 #93(인풋 박스 모드)·#101(입력 소스 전환)이 만든 **다국어 OCR 지연의 UX 피드백 수정**이다. 사용자 제안("OCR 을 스레드로 풀어서 완료 시 트리거")의 **핵심 구조는 이미 구현되어 있음**이 코드 실측으로 확정되었다 — 이 계획은 그 사실을 명세에 확정 기록하고, 실제로 남아 있는 결함("검출 중 '없음' 오독" 피드백)을 고치는 일이다.
>
> ⭐ **상급 리뷰 반영 (2026-09-03, `ultrakey-review`)**: **조건부 통과** — 반영 항목 11건(#1~#11). 핵심 교정: ① 카운터·detecting 표시 규정의 정본을 **F-03(`seek-overlay-ui.md`)으로 이동**(#1) ② "F-01/F-03 이 필터링"이라는 경계 오류를 "F-02 의 `filter_by_query` 재적용"으로 정정(#2) ③ `seek-activation-and-session.md` §3.2 Opening 완료 행(Ready → Ready/Querying) 동시 갱신(#3) ④ **S-4 의 '지연 고지' 의무를 이번 범위에 포함**(#6, 5개 로케일 힌트). 전체 반영 내역은 §9.

---

## 1. 요약

**증상 (사용자 피드백 2026-09-02)**: 검색 언어를 영어+한글 모드(인풋 박스 모드, D10)로 설정하면, 검색창을 열었을 때 **OCR 에 시간이 걸려(한국어 약 3.1s) 동작하지 않는 것 같은 느낌**을 준다. 사용자 제안: **이 시점에 창은 띄워 놓고 검색어 입력은 받되, OCR 이 완료되면 입력된 키워드를 기준으로 검색을 트리거링** — "OCR 자체를 스레드로 풀어서, 스레드가 종료되면 인풋 창에 트리거링 받고 인풋 창에서 검색".

**원인 실측(코드, 요약)** — 사용자가 제안한 메커니즘은 **이미 구현되어 있다**:

| 사용자 제안 | 현재 구현 (실측) |
| :--- | :--- |
| "OCR 을 스레드로 풀어서" | ✅ **이미 전용 스레드** — `spawn_detection`(seek.rs)이 세션마다 `ultrakey-seek-detect` 스레드를 띄워 `detect_candidates` 를 돌린다. 워커/메인 스레드를 차단하지 않는다(S-1·S-6) |
| "창은 띄워 놓고" | ✅ **이미 즉시 표시** — `Opened` 효과에서 `renderer.show()` 가 후보 0개 상태로 검색 바를 즉시 띄운다(S-6). 검출 완료를 기다리지 않는다 |
| "검색어 입력은 받되" | ✅ **이미 OCR 중 입력 수용** — 인풋 박스 모드: 계층 1 이 문자/Backspace 를 원본 통과 → 웹뷰 `<input>` 이 IME 조합 → 100ms debounce/`compositionend` 로 `SetQuery`. 영어 모드: `handle_key` 가 쿼리 버퍼에 누적 |
| "OCR 완료되면 입력된 키워드로 트리거" | ✅ **이미 증분 트리거** — 검출 결과가 **디스플레이별로 도착할 때마다** `ingest_display` → `recompute_matching` 이 **현재 쿼리로 즉시** 필터링(F-02 의 `filter_by_query` 재적용). Opening 중 입력된 쿼리는 버퍼링되어(`set_query_external` — machine.rs) 도착 후보에 즉시 적용된다. 검출 완료를 기다리지 않는다 |
| "반응이 즉시 보이는 UX" | ⭐ **이것이 유일한 실제 결함** — 검출이 진행 중인데 카운터가 "없음"을 표시("검색 완료 + 0개" 오독). 진행 바로는 이 모순을 못 막는다 |

⭐ **실제로 남아 있는 결함 — 검출 중 "없음" 오독**: `overlay-searchbar.html` 카운터가 `total_matches <= 0` 이면 **"없음"** 을 표시한다. 검출이 아직 진행 중인데(한국어 OCR 최대 약 3.1s) 사용자가 쿼리를 입력해도 0 매치 = "없음"만 보인다 — **"검색이 끝났는데 0개"로 읽혀 "동작하지 않는다"는 인상을 준다.** 같은 파일 376행은 `payload.detecting` 으로 진행 바를 표시하는데 **카운터는 그 신호를 무시**한다 — 의도된 구분(S-6)을 스스로 깨고 있는 셈이다.

**결정 요지**:

- **D1 — 검출 중 카운터의 정직화**: `payload.detecting == true` 동안 카운터는 **"찾는 중…"**을 표시하고, 검출 완료 후 0 매치일 때만 확정 **"없음"**을 표시한다. 매치가 도착해도(증분 — 멀티 디스플레이 시 디스플레이 1 종료 후 반드시 발생) 검출 완료 전까지는 "찾는 중…"을 유지한다(도착한 매치 집합은 잠정적). 이슈의 "멈춘 듯" 인상이 사라진다(웹뷰 JS 3줄).
- **D2 — 트리거 구조 변경 없음**: "OCR 완료 시 트리거"는 이미 증분 도착 시점에 실현되어 있다. **스레드 추가/변경·검출 완료 대기 필터링은 하지 않는다** — S-6(첫 후보 335ms)·S-1(병렬 이득 0%) 실측을 기각 근거로 명세에 확정 기록한다.
- **D3 — 명세 갱신**: `seek-overlay-ui.md`(**카운터 표시 규정의 정본** — F-03 소관) · `seek-activation-and-session.md`(검출 중 버퍼링·트리거 계약, §3.2 Opening 완료 행 Ready/Querying 교정, §5 엣지 · 수용 기준) · `seek-text-detection.md`(스레드 구조·증분 트리거 확인, 필터 주체는 F-02).
- **D4 — S-4 지연 고지 이행**: `settings.seek.search_language.hint`(5개 로케일)에 "추가 언어 인식 시 초기 검색이 느려진다"는 지연 고지를 1문장 더한다 — S-4 가 결정했으나 미이행이던 의무이며, 사용자 불만("지연을 몰랐다")과 직접 연결된다.
- **D5 — 테스트**: (a) `machine.rs` — 이슈 #102 전 시나리오 순수 로직 재현 테스트(인풋 박스 래칭 + 검출 중 쿼리 + 후보 도착 **즉시** 필터 + `finish_detection` → detecting false + Querying 전이), (b) `frontend_wiring.rs` — 카운터 **분기 순서**(detecting 분기가 `total <= 0` 보다 앞)를 고정하는 정적 회귀 테스트.

**범위 제약**: ① ⛔ **검출 파이프라인 자체(OCR 스레드·증분 API·좌표 변환)는 건드리지 않는다** — 실측 결과 이미 요구된 구조다. ② 검출 지연(775ms→3.1s) 자체를 **줄이는** 작업도 아니다 — S-4 의 "지연 고지"만 이번에 이행한다(지연 값은 사용자가 선택한 대가다). ③ `detecting-track` 의 시각 강화(두께·색)는 **하지 않는다** — 카운터 문구가 본질이고, 지연 예산 P3 위의 추가 애니메이션은 지나친 변경이다(기각 근거 §3 D1).

---

## 2. 사실 확인 (조사 근거 — 실측)

| # | 사실 | 근거 (파일·행, 등급) |
| :--- | :--- | :--- |
| 1 | ⭐ **OCR 은 이미 전용 스레드에서 돈다** — `spawn_detection`(seek.rs L430-487)이 세션마다 `ultrakey-seek-detect` 스레드를 띄워 `detect_candidates` 를 실행하고, 결과는 `SeekSignal::Candidates`(디스플레이별)·`ExtraCandidates`(AX)·`DetectionFinished` 로 채널에 돌려보낸다. 워커는 단일 소유자라 잠금이 없다 | `apps/ultrakey-app/src/seek.rs:430-487, 774-831` + 모듈 문서 L9-27 (실측) |
| 2 | ⭐ **검색 바는 세션 열림 즉시 표시된다** — `SessionEffect::Opened` 에서 `renderer.show()` 가 호출되고, `OverlaySession::open` 은 **후보 0개·detecting=true** 로 연다(S-6). 검출 완료를 기다리지 않는다 | `seek.rs:312-314` · `crates/ultrakey-overlay/src/session.rs:67-86` (실측) |
| 3 | ⭐ **OCR 중 검색어 입력이 이미 가능하다** — 인풋 박스 모드(이슈 #93): 계층 1 이 `is_input_box_pass_key` 를 만족하는 문자/Backspace 키를 원본 통과 → 웹뷰 `<input>` 이 IME 조합 → `compositionend` 즉시 / 100ms idle debounce 로 `seek_set_query` → `SetQuery` 신호 | `arbitration.rs:407-434` · `overlay-searchbar.html:282-323` · `main.rs:4826-4827` (실측) |
| 4 | ⭐ **검색 트리거는 OCR 완료를 기다리지 않는다** — `SeekSignal::Candidates` → `ingest_display` → `overlay.ingest_display` → `recompute_matching` 이 **현재 질의로 즉시** 재필터한다. 필터의 주체는 `ultrakey_seek::filter_by_query`(**F-02 크레이트**)이며 오버레이가 새로 구현하지 않는다(seek-overlay-ui.md §3.6 계약 "오버레이 자신은 필터링을 수행하지 않는다"). Opening 중 도착한 `SetQuery` 는 `set_query_external` 이 `session.query` 에 버퍼링하고, 후보 도착 시 그 쿼리로 즉시 매치가 난다 | `machine.rs:412-427(버퍼링), 451-464(증분 소비자)` · `session.rs:26, 335-349(recompute_matching = filter_by_query 소비)` (실측) |
| 5 | 이슈 #93 의 T10-b 테스트가 **이미** "Opening 중 도착한 외부 쿼리는 버퍼에 누적되고, 후보가 도착하면 즉시 그 쿼리로 필터된다(유실 방지)"를 고정한다 | `machine.rs:1559-1577` `set_query_external_during_opening_buffers_and_filters` (실측) |
| 6 | ⭐ **"멈춘 듯" 의 실제 원인 — 검출 중 "없음" 오독**: 카운터가 `total_matches <= 0` 이면 "없음"을 표시한다. 검출 진행 중(한국어 OCR 약 3.1s — spike §3)에는 사용자가 쿼리를 입력해도 0 매치가 "없음"으로만 보인다 — "검색 완료 + 0개"라는 확정 판정으로 읽힌다. **같은 파일 376행은 `payload.detecting` 으로 진행 바를 표시하므로 두 신호가 이미 모순** | `overlay-searchbar.html:367-369(카운터), 376(detectingTrack)` · `seek-ocr-latency-spike.md` §3(ko-KR 775ms→3.1s) (실측) |
| 7 | `SearchBarFrame::detecting` 은 Rust(overlay)가 계산해 웹뷰에 이미 전달된다 — `present()` → `emit_to("overlay://searchbar")`. 검출 완료(`finish_detection`) 시 false 로 바뀌고 Repaint 된다 | `session.rs:110-112, 327` · `model.rs:78` · `overlay.rs:382-431` · `machine.rs:478-496` (실측) |
| 8 | 검출 완료 시 상태 전이: Opening → (쿼리 비었으면) **Ready** / (쿼리 있으면) **Querying**. 검출 중 입력했다면 Querying 으로 간다 — Ready 로 되돌아가지 않는다 | `machine.rs:488-494` (실측) |
| 9 | ⭐ **검출 중에도 먼저 도착한 매치로 Enter 확정이 가능하다** — `handle_key` 의 Confirm 분기(`machine.rs:330-334`)는 Opening 을 막지 않고(`Confirming` 만 차단), `confirm_selected`(570-590)는 `overlay.selected()`(=`matching.get(0)`)만 있으면 동작한다. 반면 순환(↑↓/Tab/`;`)은 Opening 중 무시(336-358). → "찾는 중…"인 동안 Enter 가 동작하는 상태는 **의도된 동작**으로 명세에 못박아야 한다 | `machine.rs:330-358, 570-590` (실측) |
| 10 | S-4 의 "켜면 느려진다는 것을 UI 에 밝힌다" 의무가 **미이행** — `settings.seek.search_language.hint`(5개 로케일)에 지연 고지 문구가 없다(IME 동작 설명만). 사용자 불만("지연을 몰랐다")과 직접 연결 | `resources/i18n/*.json:67` (실측) — literal scan |
| 11 | 검출 파이프라인은 **디스플레이별 순차 OCR + 끝나는 대로 하나씩 내보내기**(S-1)이고, 병렬은 이득 0%·첫 결과 지연(335→779ms)으로 반증됐다. 전처리 기본 `None`(S-3), 인식 언어 기본 영어(S-4) | `detect.rs:112-185` · `seek-ocr-latency-spike.md` §5 표 (실측) |
| 12 | 검색 바 HTML 의 문자열은 세션 UI 로 ko 하드코딩이 기존 관례다("없음" 등) — i18n 카탈로그가 아니다. "찾는 중…"도 같은 관례를 따른다 | `overlay-searchbar.html` 전반 (실측) |
| 13 | `frontend_wiring.rs` 는 오버레이 HTML 을 **정적으로 읽어** 재발을 검사하는 테스트 파일이다 — `read_searchbar_html()` 헬퍼 존재, 카운터 관련 기존 테스트는 **없음**(신규 추가 충돌 없음) | `apps/ultrakey-app/tests/frontend_wiring.rs:53-56, 2543-2546` (실측) |

---

## 3. 결정 목록 D1~D5

### D1 — 검출 중 카운터의 정직화: "찾는 중…" vs 확정 "없음"

**결정**: `overlay-searchbar.html` 의 카운터 분기를 `payload.detecting` 우선으로 고친다.

```js
const total = payload.total_matches || 0;
if (payload.detecting) {
    // ⭐(이슈 #102) — 검출 진행 중. 0 매치는 "검색 완료 + 0개"가 아니라
    // "아직 화면을 다 읽지 못했다"는 뜻 — "없음"으로 읽히면
    // "동작하지 않는다" 오독이 된다. 매치가 이미 도착했어도(증분—
    // 멀티 디스플레이) 검출 완료 전까지 이 문구를 유지한다(잠정 집합).
    counter.textContent = "찾는 중…";
} else if (total <= 0) {
    counter.textContent = "없음";   // 검출 완료 후의 확정 0
} else if (typeof payload.selected_index === "number") {
    counter.textContent = (payload.selected_index + 1) + " / " + total;
} else {
    counter.textContent = "- / " + total;
}
```

그 외의 변경은 **없다**: ① `detecting-track`(2px 진행 바)은 그대로 둔다 — 카운터가 이제 그 신호와 정합한다. 두꺼워지거나 색이 바뀌는 **시각 강화는 하지 않는다** (기각 아래). ② 매치 목록이 비어 있는 동안 목록 영역에 "검색 중" 안내 행을 추가하는 일도 하지 않는다 (기각 아래). ③ Rust 쪽 `SearchBarFrame` 에 새 필드(`status` enum 등)를 추가하지 않는다 (기각 아래).

**근거**: "멈춘 듯" 인상의 직접 원인은 사실 ⑥ — 검출이 진행 중인데 "없음"을 내보내는 모순. `detecting`(사실 ⑦)은 웹뷰가 이미 받는 값이므로 **표시만 고치면 결함이 닫히고**, Rust → JS 데이터면(Data flow)은 변경이 없다. 리뷰 #5 가 지적한 대로 **검출 중 + 매치 도착(증분) + Enter 확정 가능**(사실 ⑨) 엣지에서는 카운터를 매치 수와 무관하게 검출 완료까지 "찾는 중…"으로 유지한다 — 도착한 매치 집합은 검출이 덜 끝난 잠정 상태이기 때문이다. 이것이 의도된 동작임을 명세 §5·M-item 에 명시한다.

**기각한 대안**:

- **① 검출 완료 후 "한 번에" 필터링**(사용자 문구의 문자적 해석): 검출 결과를 모아뒀다가 `DetectionFinished` 에서 일괄 적용. — **기각.** 첫 후보 도착이 335ms → 775ms(전체 완료)로 늦어져 S-6(즉시 세션 + 증분)과 정면 충돌한다. 지금의 증분 필터링이 이미 "완료 시 트리거"의 상위 호환이다.
- **② OCR 을 스레드 풀로**(추가 병렬화): — **기각.** S-1 실측이 병렬 이득 0%(오히려 첫 결과 335→779ms 지연)임을 반증했고, 이미 전용 스레드(사실 ①)로 워커 차단은 없다.
- **③ `SearchBarFrame.status` enum 추가**(`Detecting | NoMatches | HasMatches`): `detecting` + `total_matches` 로 웹뷰가 전부 도출 가능 — 중복 상태를 두면 두 소스가 어긋날 회귀 경로가 생긴다. 기각.
- **④ 매치 목록에 "검색 중" 안내 행**: 검출 중 `matches.len()==0` 이면 목록이 `hidden` 이다. 안내 행을 넣으면 window height(행 수 추종, `overlay.rs` `present()` 의 `set_size`)가 검출 중 커졌다가 매치 도착 시 줄어드는 **점프**가 생긴다. 카운터 문구 + 진행 바로 충분. 기각.
- **⑤ `detecting-track` 시각 강화**(두께·밝기·텍스트 추가): "찾는 중…" 문구가 본질을 전달하고, 추가 애니메이션은 P3 지연 예산 위의 불필요한 변경이다. 기각.

### D2 — 트리거 구조 변경 없음 (증분 트리거를 명세로 확정)

**결정**: 검색 트리거(쿼리 필터링)는 **후보 도착 시점**(디스플레이별 `Candidates` 도착, 사실 ④)에 이미 즉시 실행된다. **코드 변경 없이**, 이 동작을 명세에 **확정 등급**으로 기록한다:
- `seek-text-detection.md` §5 #8(S-1/S-6 실측 기록)에 **"검출 결과 도착 시 F-02 의 `filter_by_query` 가 현재 질의로 재적용 — 'OCR 완료 시 트리거'는 증분 방식으로 실현"** 을 확인 문구로 추가(필터 주체 정정 포함, 리뷰 #2).
- `seek-activation-and-session.md` §3.2·§5 엣지에 **검출 중 입력 버퍼링·즉시 트리거 계약**과 **검출 중 카운터 규정(→ F-03 상호 참조)** 을 확정 문구로 추가.
- `seek-overlay-ui.md` — **§3.7 "검출 상태 표시"** 신설: 검출 중 카운터 "찾는 중…"·`detecting-track` 표시·검출 완료 후 확정 "없음"/"n / m"·잠정 매치 집합 규정(리뷰 #1 — F-03 이 정본).

**근거**: 변경할 것이 실측으로 없다. 사용자가 원한 UX(창 표시 + 입력 수용 + 완료 시 트리거)는 사실 ②③④가 이미 보장하고, 남은 결함은 피드백(D1)이다. **구현하지 않을 것을 행동으로 증명**하려면 명세에 못박는 것이 이 저장소의 관례(명세 = 완료 정의)에 맞는다.

### D3 — 명세 갱신 (초안은 §6)

- `seek-overlay-ui.md`: §3.7 "검출 상태 표시" 신설(카운터·진행 바·잠정 매치 — **정본**), §8 수용 기준 신규.
- `seek-activation-and-session.md`: §3.2 Opening 완료 행 **Ready/Querying 교정**(리뷰 #3 — 기존 "준비완료(Ready)" 무조건 행과 신규 테스트가 모순되지 않게), 검출 중 버퍼링·트리거 계약 문구, §5 엣지 신규 항목(검출 중 "없음" 오독 + 잠정 매치·Enter 확정 엣지), §5 #3 문구 정합(증분 — 리뷰 #10), §8 수용 기준 신규.
- `seek-text-detection.md`: §5 #8 보강(증분 트리거 실현 + 필터 주체 F-02), §8 수용 기준(선택).

### D4 — S-4 지연 고지 이행 (5개 로케일)

**결정**: `settings.seek.search_language.hint` 에 지연 고지 1문장을 덧붙인다(리뷰 #6):

- ko: "언어를 추가하면 화면을 읽고(OCR) 검색 결과를 찾는 시간이 길어질 수 있습니다."
- en: "Adding a language makes initial results slower, because the on-screen text must be recognized in more than one language."
- zh: "添加语言后,读取屏幕(OCR)并找到结果所需的时间会更长。"
- ja: "言語を追加すると、画面を読み取って(OCR)結果が見つかるまでの時間が長くなります。"
- es: "Añadir un idioma hace que los resultados tarden más en aparecer, porque el texto de la pantalla debe reconocerse en más de un idioma."

**근거**: S-4(사실 ⑩)가 이미 결정한 의무의 이행이다. 값만 바뀌고 키가 바뀌지 않으므로 i18n 카탈로그 커버리지 테스트는 그대로 통과한다. 지연 수치(3~4배 등)는 **적지 않는다** — 한국어만 실측이고(zh·ja·es 는 `(추정)`), 설정 글에 수치를 박으면 검증되지 않은 언어에 오정보가 된다(docs/spec 읽는 사람과 달리 UI 사용자는 `(추정)` 표기를 모른다).

**기각한 대안**: ① 별도 후속으로 분리 — 사용자 불만의 직접 원인("지연을 몰랐다")을 남겨두는 것이 된다. 기각. ② 카탈로그가 아닌 명세에만 기록 — UI 사용자에게 닿지 않는다. 기각.

### D5 — 테스트

순수 로직(트리거 조건) 단위 테스트 + 정적 회귀 테스트:

1. **`machine.rs`** — 이슈 #102 전 시나리오 재현(`issue102_query_during_detection_triggers_incrementally_and_finish_flips_detecting`):
   - `input_box_mode: true` 설정 변형 필요 — 기존 `open_global` 헬퍼(`machine.rs:1521-1533`)는 설정 하드코딩이므로, 테스트 안에서 `global_shortcut` + `input_box_mode: true` 설정을 직접 만들어 `activate` 한다(리뷰 #8).
   - 단언: ① 개시 → `state == Opening` + `search_bar_frame().detecting == true` + **`input_mode == true`**(래칭) ② 검출 중 `SetQuery("설정")` → Opening 유지 + detecting true + 0 매치 ③ 후보 도착 → **즉시** 쿼리로 필터(2매치) + detecting true 유지(잠정) ④ `finish_detection` → detecting false + `state == Querying` + 매치 유지.
2. **`frontend_wiring.rs`** — `searchbar_counter_distinguishes_detecting_from_no_matches`: 카운터 블록을 추출(`css_rule_body` 와 같은 방어적 스캔)해 **`payload.detecting` 분기가 `total <= 0` 분기보다 앞**인지와 "찾는 중…"·"없음" 문구 존재를 단언한다(리뷰 #7 — 존재만 검사하면 분기 순서 역전 회귀를 못 잡는다).
3. 기존 워크스페이스 테스트 `cargo test --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` 통과.

---

## 4. 구현 단위 (작업 분해)

| # | 단위 | 파일 | 변화 | 완료 정의 |
| :--- | :--- | :--- | :--- | :--- |
| 1 | 카운터 분기 | `apps/ultrakey-app/ui/overlay-searchbar.html` | `render()` 카운터: `payload.detecting` 우선 "찾는 중…" | 검출 중 "없음" 미표시, 검출 완료 후 확정 "없음"/"n / m" |
| 2 | 순수 로직 테스트 | `crates/ultrakey-seek-session/src/machine.rs` | D5-1 테스트 1개 추가 | 시나리오 전체 검증 (트리거 지연 없음·detecting·input_mode 래칭) |
| 3 | 정적 회귀 테스트 | `apps/ultrakey-app/tests/frontend_wiring.rs` | D5-2 테스트 1개 추가(분기 순서) | HTML 분기 순서·문구 유지 검증 |
| 4 | S-4 지연 고지 | `resources/i18n/{ko,en,zh,ja,es}.json` | `search_language.hint` 1문장 추가 | 5개 로케일 · 카탈로그 커버리지 테스트 통과 |
| 5 | 명세 — F-03 | `docs/spec/seek-overlay-ui.md` | §3.7 "검출 상태 표시" 신설 + §8 수용 기준 | 카운터·진행 바·잠정 매치 규정 정본 |
| 6 | 명세 — F-01 | `docs/spec/seek-activation-and-session.md` | §3.2 Opening 완료 행 Ready/Querying 교정 · 검출 중 버퍼링·트리거 계약 · §5 신규 엣지(+#3 정합) · §8 수용 기준 | 명세-테스트 모순 제거 |
| 7 | 명세 — F-02 | `docs/spec/seek-text-detection.md` | §5 #8 보강(증분 트리거 + 필터 주체 F-02) | 확정 등급 기재 |
| 8 | 수동 검증 항목 | `docs/dev/manual-verification.md` | 항목 19 표에 **M14(이슈 #102)** 추가(중간 상태 행 포함) | 실기기 절차·판정 기준 기록 |

---

## 5. 테스트 계획

| 테스트 | 대상 | 검증 내용 |
| :--- | :--- | :--- |
| `issue102_query_during_detection_triggers_incrementally_and_finish_flips_detecting` | `machine.rs` | ① 인풋 박스 모드 세션 개시 → `state == Opening` + `detecting == true` + `input_mode == true` ② 검출 중 `SetQuery` → Opening 유지·detecting true·0 매치 ③ 후보 도착 → **즉시** 쿼리로 필터(2매치) + detecting true 유지(잠정 — "찾는 중…" 분기 근거) ④ `finish_detection` → detecting false + `state == Querying` + 매치 유지 |
| `searchbar_counter_distinguishes_detecting_from_no_matches` | `frontend_wiring.rs` | 카운터 블록에서 `payload.detecting` 분기가 `total <= 0` **앞** + "찾는 중…"·"없음" 존재 |
| 기존 전체 | workspace | 회귀 없음 (catalog 값 변경 — 키 무변경이므로 커버리지 테스트 통과 확인) |

---

## 6. 명세 갱신 초안

### `seek-overlay-ui.md` — §3.7 신설 (카운터 표시의 정본, 리뷰 #1)

> **§3.7 검출 상태 표시 — 카운터·진행 바 · 잠정 매치 (⭐ 이슈 #102 확정)**
>
> 검색 바 카운터(`#counter`)는 세 가지 상태를 구분해 표시한다:
> - **검출 진행 중(`SearchBarFrame.detecting == true`)**: **"찾는 중…"** — 0 매치여도 "없음"을 내보내지 않는다("없음"은 검색 완료 + 0개라는 **확정 판정**으로 읽히며, 검출 중 사용자는 "동작하지 않는다"고 오독한다 — 이슈 #102).
> - **검출 완료 + 매치 0개**: **"없음"**(확정).
> - **검출 완료 + 매치 n 개**: **"k / n"**(선택 인덱스 k).
>
> ⭐ **잠정 매치**: 검출은 디스플레이별 증분(S-6)이므로 **검출 완료 전에도 먼저 도착한 디스플레이의 매치가 목록·하이라이트에 나타난다**. 이 매치 집합은 잠정적이며, 카운터는 매치 수와 무관하게 검출 완료까지 "찾는 중…"을 유지한다. 이 상태에서 Enter 는 도착한 첫 매치를 확정할 수 있다(의도된 동작 — F-01 §3.2 Opening 행). 진행 바(`detecting-track`)는 검출 중 표시되고 검출 완료 시 사라진다(S-6 의 UI 표현).
>
> **수용 기준**: 검출 중 카운터가 "찾는 중…"을 표시하고, 검출 완료 후 0 매치일 때만 "없음", 매치가 있을 때 "k / n" 을 표시한다. 검출 중 도착한 매치는 잠정적으로 표시되며 카운터 문구는 바뀌지 않는다.

### `seek-activation-and-session.md`

- **§3.2 상태 머신** — Opening 완료 행 교정(기존 무조건 "준비완료(Ready)" — 리뷰 #3):

| 현재 상태 | 입력 | 조건 | 다음 상태 | 부수효과 |
| :--- | :--- | :--- | :--- | :--- |
| Opening | F-02 로부터 "후보 생성 완료" 콜백 | 쿼리 버퍼 비어 있음 | 준비완료(Ready) | 후보 하이라이트 렌더 요청(F-03 위임) |
| Opening | F-02 로부터 "후보 생성 완료" 콜백 | 쿼리 버퍼에 문자가 있음(검출 중 입력됨) | 질의입력중(Querying) | 후보 하이라이트 렌더 요청 — 버퍼된 쿼리로 이미 필터된 상태(§5 #3) |

- **§3.1 원칙** (검출 중 카운터 — F-03 으로 상호 참조): "⭐(이슈 #102) 검출이 진행 중인 동안(`SearchBarFrame.detecting == true`) 검색 바 카운터는 **'찾는 중…'**을 표시한다 — 0 매치는 검출 완료 후에만 '없음'으로 확정된다(표시 규정의 정본은 F-03 §3.7). 검출 중 입력된 검색어는 후보가 도착하는 대로 즉시 필터링된다(OCR 완료 대기 없음 — S-1/S-6)."
- **§5 엣지 케이스·실패 모드**: 신규 항목 — "검출 중 '없음' 오독과 잠정 매치(이슈 #102)". 검출이 진행 중인데 카운터가 "없음"이면 사용자가 "동작하지 않는다"고 오독한다(검출 중엔 "찾는 중…"). 검출 중 도착한 매치는 잠정적이며(카운터는 검출 완료까지 "찾는 중…") Enter 로 확정할 수 있지만 ↑↓/Tab 순환은 Opening 중 무시된다(§3.2). 기존 §5 #3("캡처 지연 중 사용자 입력")은 "(실제로는 후보 도착마다 증분 필터링이 적용된다 — 이슈 #102 항목 참조)" 를 더해 증분 사실과 정합시킨다(리뷰 #10).
- **§8 수용 기준**: 신규 — "검출이 진행 중인 동안 카운터가 '찾는 중…'을 표시하고, 검출 완료 후 0 매치일 때만 '없음'을 표시한다(정본: F-03 §3.7). 검출 중 입력된 검색어가 후보 도착 시 즉시 필터링된다."

### `seek-text-detection.md` — §5 #8 보강 (리뷰 #2 필터 주체 정정 포함)

> **§5 #8 뒤 보강**: ⭐(이슈 #102) **"'OCR 완료 시 검색 트리거'는 이 파이프라인의 증분 전달(디스플레이별 `on_display`)이 이미 실현한다.** 후보가 도착하는 즉시 세션(F-01 머신 → F-03 오버레이 세션)이 **F-02 의 필터(`filter_by_query`)를 현재 질의로 재적용**한다 — 오버레이가 필터를 새로 구현하지 않는다(seek-overlay-ui.md §3.6 계약). OCR 총 완료를 기다리지 않으며, 별도 '완료 트리거' 경로를 추가하지 않는다."

> §8 수용 기준(선택): "디스플레이별 후보가 도착하는 즉시 현재 질의로 필터링된 후보가 표시된다(F-02 필터의 재적용)."

---

## 7. 수용 기준 (이 이슈의 완료 정의)

- [ ] `cargo test --workspace` 통과 (신규 2테스트 포함)
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 통과
- [ ] 검출이 진행 중인 동안 검색 바 카운터가 "찾는 중…"을 표시한다 (검출 완료 후 "없음"/"n / m"으로 전환)
- [ ] 검출 중 입력된 검색어(인풋 박스·영어 양쪽)가 후보 도착 시 즉시 필터링되어, OCR 완료를 기다리지 않고 매치가 나타난다 — 명세(F-01 §3.2/F-02 §5)에 확정 등급으로 기록됨
- [ ] `seek-overlay-ui.md` §3.7 에 카운터 표시 규정(정본)이 있다
- [ ] `settings.seek.search_language.hint` 가 5개 로케일에서 지연을 고지한다(S-4 이행)
- [ ] 검출 파이프라인(스레드·증분 API)은 변경 없음 — 회귀 제로
- [ ] 수동 검증 M-item(항목 19 M14)이 `manual-verification.md` 에 기록되어 있다

---

## 8. 수동 검증 (M-item — `manual-verification.md` 항목 19 표에 M14 로 추가)

**M14 (이슈 #102) — 영어+한글 모드에서 OCR 지연 상황의 검색창 응답**

| # | 시나리오 | 조작 | 기대 결과 | 판정 근거 |
| :--- | :--- | :--- | :--- | :--- |
| **M14-a** | 검색창 즉시 표시·입력 | `검색 언어` = `English + 한국어` 로 세션을 열고 **곧바로** 한글 쿼리 입력("설정") | 검색 바가 **즉시** 나타나고 `<input>` 에 포커스(쿼리 입력 가능). 카운터가 "찾는 중…"을 표시 — "없음"이 아니다 | Plan §3 D1 · §6 F-03 |
| **M14-b** | (중간 상태) 검출 완료 전 매치 도착 | 2개 디스플레이 환경에서 검출 종료 전에 먼저 도착한 디스플레이 결과를 관찰 | 매치가 목록·하이라이트에 나타나도 **카운터는 검출 완료 전까지 "찾는 중…"을 유지**한다(잠정 매치 — 정상, 결함이 아님). 이 상태에서 Enter 가 첫 매치를 확정한다 | Plan §2 사실 ⑨ · §6 F-01 §5 |
| **M14-c** | 검출 완료 후 확정 표시 | 검출 종료 후 카운터 확인 | "1 / n" 으로 전환되고 매치가 그대로 필터링된다. (대조) 검출 완료 후 매치 0개 쿼리 입력 → "없음"(확정) | Plan §3 D1 |

---

## 9. 상급 리뷰 반영 (2026-09-03, `ultrakey-review` — 판정: 조건부 통과)

| # | 지시 | 우선순위 | 반영 |
| :--- | :--- | :--- | :--- |
| **#1** | 카운터·detecting 표시 규정의 정본을 **F-03(`seek-overlay-ui.md`)** 에 두고 F-01 은 상호 참조 — `seek-overlay-ui.md` 에 detecting·카운터 언급이 0건인 것은 배선 공백 | 필수 | §3 D3 · §4 단위 5 · §6 F-03 §3.7 신설 |
| **#2** | "F-01/F-03 이 필터링" 문구는 경계 오류 — 필터는 **F-02 의 `filter_by_query`** 를 세션이 소비하는 것(seek-overlay-ui.md §3.6 계약) | 필수 | §2 사실 ④ 문구 정정 · §6 F-02 §5 #8 보강안의 주체 정정 |
| **#3** | `seek-activation-and-session.md` §3.2 Opening 완료 행(무조건 Ready)이 신규 테스트(Querying)와 모순 — 행 교정 | 필수 | §3 D3 · §6 F-01 §3.2 두 행으로 교정 |
| **#4** | 문구 위생 2건 — "검찰"→"검출", 정의 없는 "Elastic" 제거 | 필수 | 반영(본문 서술 전체 재작성 시 반영됨) |
| **#5** | 검출 중 + 매치 도착(증분) + Enter 확정 가능 엣지를 명세·M-item 에 명시 — 의도된 동작으로 못박기 | 필수 | §2 사실 ⑨ 신규 · §3 D1 문단 · §6 F-01 §5 · §8 M14-b |
| **#6** | S-4 의 "지연 고지" 의무(5개 로케일 힌트)가 미이행 — 포함 여부 명시 결정 | 필수 | **이번 범위에 포함** — §3 D4 (5개 로케일 문구 초안 + 카탈로그 커버리지 유지) |
| **#7** | `frontend_wiring` 정적 테스트는 **분기 순서**(detecting 이 `total <= 0` 보다 앞)를 단언 — 존재 검사만으론 역전 회귀를 못 잡는다 | 권장 | §3 D5-2 · §4 단위 3 · §5 |
| **#8** | `machine.rs` 신규 테스트에 `input_mode` 래칭 단언 추가 + `open_global` 헬퍼는 설정 하드코딩 — `input_box_mode: true` 변형 필요 | 권장 | §3 D5-1 · §5 (테스트 안에서 직접 설정 구성) |
| **#9** | M-item 에 중간 상태(매치 도착 + "찾는 중…" + Enter) 기대 행 추가 — 검증자의 오판 방지 | 권장 | §8 M14-b |
| **#10** | §5 #3 "후보 생성이 완료되면" 문구를 증분 사실과 정합 | 권장 | §6 F-01 §5 (정합 문구 추가) |
| #11 | S-5 Vision 예열은 미구현이지만 첫 세션 콜드 스타트 문제로 범위 밖 — 별도 추적 검토 | 정보 | 범위 제약 유지. 추적은 후속 이슈 후보로 남김 |
| #12 | `frontmost_window_rect` 는 이번 이슈와 무관 — 미건드림 적절 | 정보 | 반영 없음(확인만) |