# ⭐ 이슈 #139 입력 지연 스파이크 — 경로 B 재적용의 탭 스레드 점유 실측

> **무엇인가**: `docs/plan/issue-139-input-latency.md` U1·U5 가 요구한 실측 문서다.
> 재현: `cargo run -p ultrakey-platform --release --example latency_probe -- --interval-ms 10 --duration-s 30 --spike-ms 20`
> (③ 은 `--duration-s 180`, `--duration-s 60` 등 절차에 맞게 조정)

---

## 0. 측정 환경

| 항목 | 값 |
| :--- | :--- |
| 기기 | Apple **M4 Pro** |
| OS | macOS **26.5.2** (25F84) |
| 빌드 | aarch64 `--release` `.app`, `Ultrakey Dev` 자체 서명, `open` 으로 실행(설치본과 번들 ID·서명 주체 동일 → TCC 권한 공유됨 — 온보딩 없이 탭 `Active`, 실측) |
| 도구 | `latency_probe` (`--interval-ms 10 --spike-ms 20`, release 빌드) |
| 디바이스 | F108Pro 동글 `5ac:24f`, 무선 마우스 동글 `373b:11d9` (attached=2) |
| `hidutil property --get UserKeyMapping` | 3회: 0.02 / 0.02 / 0.03 s |
| 날짜 | 2026-09-06 |
| 비고 | 측정 중 설치본(`/Applications/Ultrakey.app`)은 종료(사용자 동의), 측정 뒤 `open -a Ultrakey` 로 복원 |

---

## 1. ⭐ 한 줄 결론

**수정 전**, 동글 재삽입 뒤 경로 B 재적용이 탭 스레드에서 `elapsed_ms=179`(디바이스 1대) 걸렸고,
그 완료 순간 probe 샘플 14개가 한꺼번에 풀리며 **최대 180.88 ms** 지연이 관측됐다 —
곧 그 179 ms 동안 키보드·마우스 입력이 전부 정지했다는 뜻이다.

**수정 후**, 재적용은 지연 스케줄러 스레드(`ultrakey-delay-scheduler`)로 옮겨졌다. 수정 후 ③(재삽입 창)은
**[대기]** — 사용자 외출로 재삽입 불가, 복귀 후 측정 예정. 이 칸은 다음 실측 세션(planner)이 채운다.

---

## 2. 측정 방법

### probe 원리 (`latency_probe`)

- 별도 스레드가 `--interval-ms` 간격으로 **현재 커서 위치의 `MouseMoved`** 이벤트를 `CGEventSource(HIDSystemState)` 로 만들어
  `EventSourceUserData` 에 `(TAG<<56) | ns_since_start` 를 실어 `CGEvent::post(HIDEventTap)` 으로 발행한다.
- 마커에 `ULTRAKEY_MAGIC` 을 쓰지 않는다 — 그 값을 쓰면 Ultrakey 엔진의 0-a(자기 합성 이벤트 마커 확인) 분기가
  probe 이벤트를 즉시 통과시켜 버려 탭 통과 경로를 실측하지 못한다.
- 발행된 이벤트는 `HIDEventTap` → **Ultrakey 세션 탭**(`SessionEventTap`) 을 실제로 통과한 뒤 → probe 의 수신 탭
  (`TailAppendEventTap` + `ListenOnly`, `MouseMoved` 만 마스킹)에 도착한다. 수신 시각(`now_ns`)에서 인코딩된
  발행 시각(`encoded_ns`)을 빼 지연을 계산한다.
- TAG 가 다른(즉 실제 사용자 마우스 움직임) 이벤트는 무시한다.

### ③ 절차

1. probe 를 지정 시간(180 s 또는 60 s) 실행 상태로 둔다.
2. 사용자에게 동글 재삽입을 요청한다.
3. 재삽입(attach) 약 1.5 s 뒤 재적용(`ReapplyHidMapping`/스케줄러 재적용)이 실행된다(`keyboard_connect_delay_ms=1500`).
4. 앱 로그의 재적용 완료 시각·`elapsed_ms` 와 probe 가 출력한 스파이크 벽시계 시각을 대조한다 — **둘 다 UTC** 다.

---

## 3. 결과

### 수정 전 (U1 계측 빌드 = main 과 동일 동작, `elapsed_ms` 계측만 추가)

| 조건 | posted | received | lost | p50 (ms) | p95 (ms) | p99 (ms) | max (ms) | spikes≥20 |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 설치본(0.1.2) 실행·유휴 (참고) | 2487 | 2487 | 0 | 0.44 | 2.96 | 5.62 | 15.39 | 0 |
| ① Ultrakey 미실행 | 2511 | 2511 | 0 | 0.35 | 3.87 | 7.98 | 12.03 | 0 |
| ② 계측 빌드 실행·유휴 (기동 직후 30 s) | 2517 | 2517 | 0 | 0.75 | 5.23 | 9.32 | 33.24 | 4 |
| ③ 동글 재삽입 창 (180 s, 재삽입 1회) | 14979 | 14979 | 0 | 0.57 | 3.22 | 6.70 | 180.88 | 24 |

### 수정 후 (브랜치 빌드: U1~U4 전부)

| 조건 | posted | received | lost | p50 (ms) | p95 (ms) | p99 (ms) | max (ms) | spikes≥20 |
| :--- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| ① Ultrakey 미실행 | 2176 | 2176 | 0 | 0.53 | 1.26 | 2.45 | 15.90 | 0 |
| ② 수정 후 빌드 실행·유휴 (기동 직후 30 s) | 2202 | 2202 | 0 | 0.83 | 1.52 | 2.45 | 11.11 | 0 |
| ③ 재삽입 창 | 대기 | 대기 | 대기 | 대기 | 대기 | 대기 | 대기 | 대기 |

**③(수정 후) 미측정 — 사유: 사용자 외출로 재삽입 불가, 복귀 후 측정 예정.**

> 수정 후 ①②에서 참고로 실행한 180 s 창(재삽입 없이)은 스파이크 0건·max 13.25 ms 였으나
> 재삽입 이벤트가 없어 ③ 표본으로 인정하지 않는다.

---

## 4. ③ 상관 분석

수정 전 ③ 의 실제 재삽입 1회에 대해:

- `attach 23:09:23.034` → `completed 23:09:24.720 thread=ultrakey-tap elapsed_ms=179 devices=1`
- probe: `23:09:24.720~.721` 구간에 14 샘플이 한꺼번에 도착. 지연값이
  `180.88, 169.56, 157.05, …, 33.76, 21.24 ms` 로 **12.5 ms 간격 계단**을 이룬다(12.5 ms ≈ probe 발행 간격) —
  정지 구간(179 ms) 동안 큐에 쌓였던 이벤트가 정지 해제 순간 일괄 방출된 모양으로 해석된다.
- 직전 `23:09:24.454~.493` 구간의 6 샘플(25~49 ms)은 **원인 미확인 (추정: 핫플러그 직후 IOKit 재열거 또는 시스템 부하)**.
- 첫 시도(`23:06:47` attach, `elapsed_ms=114`)는 probe 측정 창 밖이라 표본에서 **제외**했다.
- 수정 전 ② 의 4개 스파이크(max 33.24 ms, 기동 직후 30 s 구간)는 **원인 미확인**.

---

## 5. QoS 확인 (A3)

기동 로그:

```
tap thread QoS set to USER_INTERACTIVE qos_before=Some(Default) qos_after=Some(UserInteractive) thread=ultrakey-tap
```

`ps -M` 은 스레드 이름을 보여주지 않아 `ultrakey-tap` 스레드로 귀속할 수 없다 — 이번 판정 근거로 쓰지 않았다.

---

## 6. 함께 관찰된 것

- `~/Library/Logs/Ultrakey/` 는 계측 빌드를 `open` 하자 즉시 생성됐다 — `init_logging` 자체는 정상 동작이다.
  설치본(0.1.2) 실행 중에는 이 디렉터리가 없었다. **원인 미확인 (추정: 설치본 실행 환경 차이)** — 이 문서의
  범위 밖이라 별도 이슈로 제안한다.
- posted 수 차이(수정 전 ~2500 vs 수정 후 ~2200, 30 s 창)는 probe 발행 스레드의 sleep 정밀도 차이로 보인다
  (추정). 각 샘플이 자기 발행 시각을 실어 지연을 계산하므로 이 차이는 지연값 자체에는 영향이 없다.

---

## 7. 한계

- 표본이 1회다(③ 수정 전 재삽입 1회만 관측).
- 유휴 조건(②)은 "기동 직후 30 s" 로만 정의했다.
- probe 자체가 `MouseMoved` 를 100 Hz(=`--interval-ms 10`)로 주입해 측정 대상 시스템에 부하를 더한다.
- 절전→깨어남 트리거(전체 재적용, 대상 디바이스 2대)는 **미측정** — 사유: 세션 연결을 유지하기 위해
  이번 세션에서는 절전 트리거를 걸지 않았다.
- 수정 후 ③(재삽입 창)은 위 §3·§1 대로 **미측정**이다.
