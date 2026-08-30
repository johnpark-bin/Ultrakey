# Ultrakey — 에이전트 규약

Ultrakey 는 macOS 유틸리티 **SuperKey**(https://superkey.app/)의 Rust/Tauri 클론이다.
이 문서는 어떤 하네스(Claude Code · Codex · opencode)로 작업하든 **동일하게 적용되는 규약**이다.

## 1. 현재 단계

**명세 단계**다. `docs/spec/` 의 기능별 명세가 확정된 뒤 기능 단위로 구현을 위임한다.

| 문서 | 내용 |
| :--- | :--- |
| [`docs/research/superkey-inventory.md`](docs/research/superkey-inventory.md) | SuperKey 전수 조사. 출처 URL + 원문 인용. **모든 명세의 사실 근거** |
| [`docs/research/rust-macos-capability-notes.md`](docs/research/rust-macos-capability-notes.md) | Rust/Tauri × macOS 역량 조사. 크레이트 버전은 crates.io 로 검증됨 |
| [`docs/spec/README.md`](docs/spec/README.md) | 기능 색인 · 구현 접근 분류 · 구현 순서 |
| [`docs/spec/platform-constraints.md`](docs/spec/platform-constraints.md) | Rust/Tauri 로 커버되는 경계와 네이티브 shim 경계 |
| `docs/spec/<기능>.md` | 기능별 명세 9개 절 |

## 2. ⭐ 모델 라우팅 규약

모델을 **상·중·하 세 등급**으로 나누고, 하네스마다 대응 모델을 고정한다.
운용의 핵심은 **속도 우선**이다 — **기본 작업은 빠른 중급 모델부터 시작**하고, 상급 모델은 그 결과의 **평가자**로만 쓴다.

| 등급 | 역할 | Claude Code | Codex | opencode |
| :--- | :--- | :--- | :--- | :--- |
| **상** | **평가자(호출 세션)** — 중급 모델이 낸 **계획 초안·구현 산출물을 리뷰·교정**. 갈림길의 **최종 확정만** 직접 | Fable 5 > Opus 5 | Sol | DeepSeek V4 Pro 0813 > Qwen3.8 Max |
| **중** | **계획 초안 + 구현** — 분석·설계·계획 **초안** 작성, 명세를 코드로, 테스트, 반복 수정. **기본 작업 주체. 가장 많이 돌린다** | Sonnet 5 | Terra | DeepSeek V4 Flash 0731 (Effort High) > Qwen3.8 Flash (Effort High) |
| **하** | **탐색** — 코드·파일 검색, 사실 확인, 단순 자료조사 | Haiku | Luna | DeepSeek V4 Flash 0731 (Effort Low) > Qwen3.8 Flash (Effort Low) |

**운용 원칙**

- ⭐ **기본 작업은 중급 모델부터 시작한다.** 분석·설계·계획 **초안**을 잡는 것도 중급 모델이 한다.
- 상급 모델은 **평가자(Evaluator)** 로만 동작한다 — 중급이 낸 계획 초안과 구현 산출물을 리뷰하고, 부족하거나 추가로 고민할 지점을 검토·교정한다.
- ⭐ **리뷰가 반영된 계획·산출물을 바탕으로 작업을 시작한다.** 이것이 정본 흐름이다 — 초안(중급) → 리뷰(상급) → 리뷰 반영 → 작업 진행.
- 단순 탐색·검색·자료조사는 **하급 모델**로 내려 보낸다. 넓게 훑는 검색을 메인 세션이 직접 하면 컨텍스트가 결과 덤프로 오염된다.
- ⭐ 이렇게 쓰는 목적은 **비용이 아니라 속도**다. 빠른 중급 모델이 반복해서 결과물을 뽑아내고, 상급 모델의 사용량은 최소로 제한한다.
- 갈림길의 **최종 판단과 확정만 상급(호출 세션)이 직접** 한다. 초안·반복 작업은 서브에이전트(중급)에 넘긴다.
- 구현은 기능 단위로 위임한다. `docs/spec/` 의 파일 1개가 위임 1건의 단위다.
- 독립적인 서브에이전트는 **한 메시지에서 병렬로** 띄운다.

### 각 하네스의 실제 설정 파일

| 하네스 | 파일 | 형식 |
| :--- | :--- | :--- |
| Claude Code | `.claude/agents/ultrakey-{plan,implement,explore}.md` | Markdown + YAML frontmatter (`model:` 키) |
| Codex | `.codex/config.toml` + `.codex/agents/ultrakey-{plan,implement,explore}.toml` | TOML |
| opencode | `.opencode/agent/ultrakey-{plan,implement,explore}.md` | Markdown + YAML frontmatter (`model: provider/id` · `options.reasoningEffort`) |

모델 식별자의 근거와 확인 방법은 각 설정 파일의 주석에 적어 두었다.
표의 약칭은 이러하다 — `Sol`/`Terra`/`Luna` 는 Codex 모델 ID `gpt-5.6-{sol,terra,luna}`, opencode 의 `Effort High/Low` 는 `options.reasoningEffort`(`high`/`low`), Claude Code 의 `Sonnet 5`/`Haiku` 는 `sonnet`/`haiku` 에일리어스다. `A > B` 는 1순위가 A, 2순위(폴백)가 B 다 — 설정 파일은 1순위 모델을 고정한다.
⭐ 상급(평가자)은 **호출 세션의 모델**이어서 별도 에이전트 파일이 없다 — `ultrakey-plan` 은 초안을 잡는 **중급** 에이전트다.

## 3. 작업 규약

### 문서

- **문서는 한국어로 쓴다.** 기능 이름·설정 라벨·API 이름 등 고유명사는 **원문 표기를 유지**한다 — `Seek`, `Only show while the remapped key is held`, `CGEventTap`, `AXUIElement`.
- ⭐ **사실과 추정을 반드시 구분한다.** 조사 문서에 근거가 있으면 인용과 출처를 달고, 없으면 `(추정)` 을 붙이고 근거를 쓴다. **확인하지 못한 동작을 지어내지 않는다.** 확정 불가한 것은 해당 문서의 "미해결 질문" 절에 남긴다.
- 갈림길에서는 **스스로 결정하고, 근거와 기각한 대안을 문서에 남긴다.**

### 코드 (구현 단계에 들어가면 적용)

- 가능한 한 **Rust**. UI 가 필요하면 **Tauri**. 전체 Xcode 설치는 요구하지 않는다 — Xcode Command Line Tools 만으로 빌드되어야 한다.
- Rust/Tauri 로 커버되지 않는 영역에는 언어 제약을 두지 않는다. 불가피하면 Objective-C/Swift shim 을 허용하되, **경계를 명세에 먼저 적고** 들어간다.
- 기능을 구현하기 전에 해당 `docs/spec/<기능>.md` 의 **수용 기준**을 읽는다. 그것이 완료 정의다.

### 커밋

- 커밋 메시지는 **한국어 + Gitmoji 이모지 문자**로 쓴다. 예: `📝 SuperKey 기능 전수 조사 인벤토리 추가`
- `CHANGELOG.md` 는 만들지도 고치지도 않는다.

## 4. 제품 제약 (원본에서 승계)

원본 SuperKey 가 사용자에게 한 약속이며, 클론도 지킨다.

- **네트워크 접근은 라이선스 검증과 업데이트 두 가지뿐이다.** 원문: "only reaches out to the network for license validation or updates as configured"
- **텔레메트리·트래킹 없음.** 원문: "none of my apps use any kind of telemetry or tracking"
- **처리한 화면 데이터를 디스크에 저장하지 않는다.** 원문: "None of the data that Superkey processes is stored on your disk"
  (설정과 라이선스 상태는 예외 — 이것은 *처리 데이터*가 아니다)
- 최소 지원: macOS 12+ · Intel 및 Apple Silicon 유니버설
- **임의 커스텀 리매핑을 제공하지 않는다.** 사전 정의된 체크박스 프리셋만 제공하는 것이 의도된 설계다. 개발자 원문: "I'll always prefer just checking a box over messing around with complicated preferences construction"

## 5. 작업 경계

**승인 없이 진행**: 워크트리 안의 파일 읽기·수정·생성·삭제, 커밋, 작업 브랜치 푸시, PR 생성, 이슈 본문·코멘트 갱신.

⛔ **하지 않는다**: 워크트리 밖 경로 수정·삭제, `push --force`, history 재작성, PR 머지, 워크트리·브랜치 삭제, 외부 발송(메일·메신저·웹훅), 패키지 배포, 전역 에이전트 설정 변경, 시크릿 열람·기록.
