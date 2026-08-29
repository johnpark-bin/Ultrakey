# Ultrakey 기능 명세 색인

Ultrakey 는 macOS 유틸리티 **SuperKey**(https://superkey.app/)의 Rust/Tauri 클론이다.
이 디렉터리는 **기능별 구현 명세**를 담는다. 명세가 확정된 뒤 기능 단위로 구현을 위임한다.

## 문서 지도

| 문서 | 역할 |
| :--- | :--- |
| **이 파일** | 기능 색인 · 구현 접근 분류 · 의존 관계 · 구현 순서 |
| [`platform-constraints.md`](platform-constraints.md) | ⭐ Rust/Tauri 경계 판정 — Xcode 요구사항, TCC 와 코드 서명, 오버레이, `CGEventTap` 과 이벤트 루프 |
| [`../research/superkey-inventory.md`](../research/superkey-inventory.md) | SuperKey 전수 조사 (출처 URL + 원문 인용). **모든 명세의 사실 근거** |
| [`../research/rust-macos-capability-notes.md`](../research/rust-macos-capability-notes.md) | Rust/Tauri × macOS 역량 조사. 크레이트 버전은 crates.io 로 검증됨 |
| `<기능>.md` | 기능별 명세. 아래 9개 절을 모두 갖는다 |

각 명세의 절 구성: **1 개요 · 2 사용자 시나리오 · 3 동작 명세 · 4 설정 항목 · 5 엣지 케이스와 실패 모드 · 6 필요한 플랫폼 API · 7 구현 접근 · 8 수용 기준 · 9 미해결 질문**

> 구현을 시작할 때 **8절 수용 기준이 완료 정의**다. 7절의 구현 접근 판정을 벗어나야 한다면 먼저 근거를 보고하고 명세를 고친 뒤 진행한다.

## 구현 접근 3분류

| 분류 | 정의 |
| :--- | :--- |
| **순수 Rust** | 안전(safe) 래퍼 크레이트만으로 커버. `unsafe` 도 네이티브 소스도 없다 |
| **Rust 바인딩** | `unsafe` FFI 직접 호출이 필요. `objc2-*` 자동 생성 바인딩이거나 수기 `extern "C"` 선언. **별도로 빌드하는 `.m`/`.swift` 파일은 없다** |
| **네이티브 shim 불가피** | Rust 에서 호출할 방법이 없어 Swift/Objective-C 소스를 별도 컴파일해 링크해야 함 |

⭐ **13개 기능 중 "네이티브 shim 불가피" 는 없다.** 근거와 그럼에도 남는 위험은 [`platform-constraints.md` §6](platform-constraints.md) 에 있다.

---

## 기능 전체 표

| ID | 기능 | 명세 | 구현 접근 | 의존 | 난이도 |
| :--- | :--- | :--- | :--- | :--- | :---: |
| **F-01** | Seek — 활성화와 세션 상태 머신 | [`seek-activation-and-session.md`](seek-activation-and-session.md) | Rust 바인딩 | F-07, F-02, F-03, F-04 | 중 |
| **F-02** | Seek — 텍스트 후보 검출 (Vision OCR + AX) | [`seek-text-detection.md`](seek-text-detection.md) | Rust 바인딩 | F-11 | **상** |
| **F-03** | Seek — 화면 오버레이 UI | [`seek-overlay-ui.md`](seek-overlay-ui.md) | Rust 바인딩 (`ns_window()` 필수) | F-02 | **상** |
| **F-04** | Seek — 클릭 실행 | [`seek-click-execution.md`](seek-click-execution.md) | Rust 바인딩 | F-02, F-03, F-11 | 중 |
| **F-05** | Hyperkey (hyper · meh · bleh) | [`hyperkey.md`](hyperkey.md) | Rust 바인딩 | F-07 | 중 |
| **F-06** | 트랙패드 원터치 hyper 제스처 | [`trackpad-hyper-gesture.md`](trackpad-hyper-gesture.md) | Rust 바인딩 ⚠️ **비공개 API** | F-05, F-07 | **상** |
| **F-07** | 키 리매핑 엔진 (공통 기반) | [`key-remapping-engine.md`](key-remapping-engine.md) | Rust 바인딩 | F-11 | **상** |
| **F-08** | Power User Presets (16종) | [`power-user-presets.md`](power-user-presets.md) | Rust 바인딩 | F-07, F-05 | 중 |
| **F-09** | 환경설정 UI | [`preferences-ui.md`](preferences-ui.md) | Rust 바인딩 | 전 기능의 설정 소유 | 중 |
| **F-10** | 메뉴바 상주와 앱 수명주기 | [`menu-bar-and-lifecycle.md`](menu-bar-and-lifecycle.md) | Rust 바인딩 | F-11, F-07 | 하 |
| **F-11** | 권한 온보딩과 복구 | [`permissions-onboarding.md`](permissions-onboarding.md) | Rust 바인딩 | — | 중 |
| **F-12** | 라이선싱과 트라이얼 | [`licensing-and-trial.md`](licensing-and-trial.md) | 순수 Rust (+ Keychain 바인딩) | — | 하 |
| **F-13** | 자동 업데이트 (Sparkle) | [`auto-update.md`](auto-update.md) | Rust 바인딩 (`Sparkle.framework`) | F-10 | 중 |
| **F-14** | 현지화와 키보드 입력 소스 독립성 | [`localization-and-input-sources.md`](localization-and-input-sources.md) | Rust 바인딩 | F-07 (입력 소스), 전 UI (현지화) | 중 |

## 의존 관계

```
F-11 권한 ──┬─► F-07 리매핑 엔진 ──┬─► F-05 Hyperkey ──► F-06 트랙패드 제스처
            │                      │
            │                      ├─► F-08 Presets
            │                      │
            │                      └─► F-14(B) 입력 소스 독립성
            │
            ├─► F-02 텍스트 검출 ──► F-03 오버레이 ──► F-04 클릭 실행
            │                                              │
            │        F-01 세션 상태 머신 ◄─────────────────┘
            │           (F-07 · F-02 · F-03 · F-04 를 모두 소비)
            │
            └─► F-10 메뉴바·수명주기 ──► F-13 자동 업데이트

F-09 환경설정 UI  ── 모든 기능의 설정을 소유. 각 기능과 병행
F-12 라이선싱     ── 독립. 언제든 착수 가능
F-14(A) 현지화    ── 횡단. UI 가 어느 정도 굳은 뒤
```

---

## ⭐ 구현 순서 제안

### M0 — 개발 워크플로 세팅 (명세 이전, **1일차에 반드시**)

명세 문서가 아니라 환경 작업이지만, **이걸 나중에 하면 그 전까지의 모든 테스트가 헛것이 된다.**

- Xcode Command Line Tools 설치 (`xcode-select --install`). 전체 Xcode 는 필요 없다
- **고정 자체 서명 인증서** 생성 + 키체인 "항상 신뢰" + 번들 ID 고정
- `tauri build` → `codesign` → 실행 을 개발 루프로 확립. **`tauri dev` 로는 권한 기능을 테스트하지 않는다**

근거: [`platform-constraints.md` §3](platform-constraints.md). ad-hoc 서명은 재빌드마다 TCC 권한을 잃는다.

### M1 — 기반 (F-11 → F-10 → F-07)

**왜 먼저인가**: 권한이 없으면 아무것도 동작하지 않고, 리매핑 엔진은 F-05·F-08·F-01 이 **모두 올라타는 단일 토대**다. 엔진을 나중에 만들면 기능마다 event tap 을 따로 붙이게 되고, 그것이 원본이 v1.20·v1.62 에서 실제로 겪은 버그의 원인이다.

1. **F-11 권한 온보딩** — 3종 권한 확인·요청·복구. `Unable to initialize` 상태 포함
2. **F-10 메뉴바·수명주기** — 상주 앱 형태, 단일 인스턴스, 깨어남·로그인 재초기화 훅
3. **F-07 리매핑 엔진** — 전용 스레드 런루프, 탭 재활성화, **중재 우선순위**, quick press 판정
4. **F-14(B) 입력 소스 독립성** — F-07 과 **함께** 한다. 나중에 하면 판정 로직을 전부 다시 쓴다

> ✅ M1 완료 판정: 아무 리매핑도 없이 event tap 이 24시간 살아 있고, 절전 복귀·로그인 후에도 살아 있다.

### M2 — 리매핑 제품화 (F-05 → F-08 → F-09)

**왜 여기인가**: Seek 보다 먼저다. 리매핑은 원본에서도 별도 앱(Hyperkey)으로 완결됐던 기능이라 **단독으로 출하 가능한 가치**를 만든다. Seek 은 훨씬 크고 위험하다.

5. **F-05 Hyperkey** — hyper/meh/bleh, `Apply modifiers to ...` 4종
6. **F-08 Power User Presets** — 16종 전수 + 상호작용 중재
7. **F-09 환경설정 UI** — Seek 탭은 비워둔 채 Hyperkey·Presets·General 부터

> ✅ M2 완료 판정: 원본 Hyperkey 앱과 동등한 기능을 환경설정으로 켜고 끌 수 있다.

### M3 — Seek (F-02 → F-03 → F-01 → F-04)

**왜 이 순서인가**: 검출이 없으면 그릴 게 없고, 그릴 게 없으면 세션을 만들 이유가 없고, 세션이 없으면 클릭할 대상이 없다. 데이터가 흐르는 방향 그대로다.

8. **F-02 텍스트 검출** — 먼저 **실측 스파이크**: 전체 화면 OCR 지연을 잰다. 여기서 예산이 안 나오면 아래 설계가 전부 바뀐다
9. **F-03 오버레이** — `ns_window()` 로 창 속성 제어. 렌더링 계층은 **교체 가능하게 분리**(WKWebView 지연 미실측 — `platform-constraints.md` P3)
10. **F-01 세션 상태 머신** — toggle/hold 모드, 질의 입력 라우팅, 매치 순환
11. **F-04 클릭 실행** — 포커스·클릭 모드·좌표 합성

> ✅ M3 완료 판정: 개발자 권장 설정(Caps Lock → Seek, hold 모드, `;` 순환)이 다중 디스플레이에서 동작한다.

### M4 — 배포 (F-13 → F-12)

12. **F-13 자동 업데이트** — Sparkle appcast·EdDSA. ⚠️ 신·구 앱의 **서명 주체가 같아야 TCC 권한이 유지**된다
13. **F-12 라이선싱** — 상태 머신과 추상 인터페이스까지. 실제 Paddle 연동은 사양 확인 후

### M5 — 마무리와 선택 기능 (F-14(A) → F-06)

14. **F-14(A) 현지화** — 8개 로케일, RTL 3종. UI 가 굳은 뒤에 한다
15. **F-06 트랙패드 제스처** — ⚠️ **최후순위**. 비공개 `MultitouchSupport` 프레임워크에 의존해 OS 업데이트로 조용히 깨질 수 있다. hyper 활성화의 **대체 경로**일 뿐이므로(F-05 물리 키가 주 경로), 실패 시 격하 가능한 선택 기능으로 설계한다

---

## 순서를 이렇게 정한 근거와 기각한 대안

| 결정 | 근거 | 기각한 대안 |
| :--- | :--- | :--- |
| **F-07 리매핑 엔진을 단일 토대로 먼저 만든다** | Hyperkey·Presets·Seek 트리거가 **같은 키 이벤트 스트림을 두고 경쟁**한다. 원본의 v1.20("caps lock 이 hyper 일 때 `Caps lock + WASD` 프리셋 파손")과 v1.62("`Shift + caps lock` 이 quick press 오발")가 이 충돌의 실증이다 | 기능마다 event tap 을 따로 설치 — 같은 버그를 재생산하고, 중재 규칙을 나중에 소급 적용해야 한다 |
| **Seek(M3)보다 리매핑(M2)을 먼저** | 리매핑은 원본에서도 독립 앱으로 완결됐던 기능이라 단독 출하 가치가 있다. Seek 은 OCR·AX·오버레이·클릭 4계층이라 위험이 훨씬 크다 | Seek 우선 — 대표 기능이지만 미실측 지연 예산(P3)에 물려 있어 초반에 진척이 안 보일 위험 |
| **F-06 을 최후순위로** | 유일하게 비공개 API 에 의존한다. 기능적으로는 F-05 의 대체 입력 경로일 뿐이라, 빠져도 제품이 성립한다 | 랜딩 페이지에 크게 노출된 기능이니 먼저 — 위험 대비 가치가 맞지 않는다 |
| **F-14(B) 입력 소스 독립성을 F-07 과 동시에** | 원본이 v1.51·v1.52 에서 **세 건**을 "regardless of keyboard layout" 으로 고쳤다. 문자 기반으로 먼저 만들면 전부 다시 쓴다 | 나중에 대응 — 원본이 그렇게 하다 세 번 데였다 |
| **M0 서명 워크플로를 1일차에** | ad-hoc 서명은 재빌드마다 TCC 권한이 날아간다. `tauri dev` 는 부모 프로세스 권한으로 판정한다. 늦게 알면 그 전까지의 검증이 전부 무효 | 나중에 서명 — 잘못된 가정 위에 코드가 쌓인다 |
| **기능당 파일 1개로 분할** | 이후 기능 단위 병렬 구현 위임 시 같은 파일을 동시에 고쳐 충돌이 확정적이다 | 단일 `SPEC.md` |
| **Seek 을 4개 명세로 분할** | 활성화·검출·렌더링·클릭은 구현 영역과 실패 모드가 전혀 다르다. 한 파일이면 병렬 위임이 불가능하다 | Seek 단일 명세 — 문서가 비대해지고 위임 단위가 커진다 |
| **ADR 을 따로 두지 않음** | 명세 확정 전이라 결정을 굳히기 이르다. 기술 선택 근거는 각 명세 7절과 `platform-constraints.md` 에 인라인으로 남긴다 | 별도 ADR 세트 |

---

## ⚠️ 착수 전에 답해야 하는 제품 결정

| # | 질문 | 왜 지금 답해야 하는가 | 관련 |
| :--- | :--- | :--- | :--- |
| D1 | **최소 macOS 버전을 12.0 으로 둘 것인가, 12.3 으로 올릴 것인가** | ScreenCaptureKit 이 12.3+ 다. 12.0 을 유지하려면 `CGWindowListCreateImage` 폴백 경로를 따로 유지해야 하고, 그건 macOS 14 에서 deprecated 다. F-02 명세는 **12.3 상향**을 제안했다 | F-02, `platform-constraints.md` P2 |
| D2 | **라이선싱을 실제로 구현할 것인가** | F-12 명세는 상태 머신과 추상 인터페이스만 만들고 실제 Paddle 연동은 보류하기로 제안했다 | F-12 |
| D3 | **F-06 트랙패드 제스처를 범위에 넣을 것인가** | 비공개 API 위험을 감수할지의 판단. 빼도 제품은 성립한다 | F-06 |

## 미해결 질문의 소재

웹 조사만으로 확정하지 못한 것은 각 명세의 **9절**과 다음 두 곳에 모여 있다.

- 제품 사양 관련 — [`../research/superkey-inventory.md` §7](../research/superkey-inventory.md) (Q1~Q15)
- 플랫폼 관련 — [`platform-constraints.md` §7](platform-constraints.md) (P1~P7)

가장 큰 공백은 **앱을 설치하지 않고는 알 수 없는 것들**이다: 각 설정의 출고 기본값, 팝업 선택지 전체 목록, `General` 탭의 실제 구성, 메뉴바 메뉴 항목. 이번 조사는 웹 기반이라는 전제였으므로 그대로 남긴다.
