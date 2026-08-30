# Ultrakey 기능 명세 색인

Ultrakey 는 macOS 유틸리티 **SuperKey**(https://superkey.app/)의 Rust/Tauri 클론이다.
이 디렉터리는 **기능별 구현 명세**를 담는다. 명세가 확정된 뒤 기능 단위로 구현을 위임한다.

> ⭐ **명세 상태: 확정 (2026-08-30).**
> 이 명세들은 처음에 **웹 조사만으로** 작성되었고(PR #2), 이후 **SuperKey v1.66 (66) 을 실제로 설치·실행해 전수 검증**했다(이슈 #3). 검증 결과는 [`../research/spec-verification-report.md`](../research/spec-verification-report.md) 에 있고, 1차 근거는 [`../research/app-bundle-analysis.md`](../research/app-bundle-analysis.md) 와 [`../research/screenshots/`](../research/screenshots/) 에 있다.
> 각 명세 안의 사실에는 근거 등급이 붙어 있다 — `(실측: …)` 은 v1.66 에서 직접 확인한 것, `(미확정)` 은 확인하지 못해 **추측으로 메우지 않고 남겨둔 것**이다. `(추정)` 은 근거 있는 추론이되 확인되지 않은 것이다.

## 문서 지도

| 문서 | 역할 |
| :--- | :--- |
| **이 파일** | 기능 색인 · 구현 접근 분류 · 의존 관계 · 구현 순서 |
| [`platform-constraints.md`](platform-constraints.md) | ⭐ Rust/Tauri 경계 판정 — Xcode 요구사항, TCC 와 코드 서명, 오버레이, `CGEventTap` 과 이벤트 루프. **§0 에 v1.66 실측 대조표** |
| [`../research/app-bundle-analysis.md`](../research/app-bundle-analysis.md) | ⭐ **v1.66 실측 1차 근거** — 번들 메타데이터, 링크 프레임워크와 심볼, UI 문자열, 실행 중 앱의 AX 트리, 복원한 설정 목록 |
| [`../research/spec-verification-report.md`](../research/spec-verification-report.md) | ⭐ **검증 결과** — 확인 / 수정 / 신규 / 미확정 4분류 |
| [`../research/screenshots/`](../research/screenshots/) | 환경설정 UI 스크린샷 5장 (v1.66) |
| [`../research/superkey-inventory.md`](../research/superkey-inventory.md) | SuperKey 웹 조사 (출처 URL + 원문 인용). ⚠️ **v1.66 실측이 이 문서의 일부를 뒤집었다** — 충돌 시 `app-bundle-analysis.md` 가 우선한다 |
| [`../research/rust-macos-capability-notes.md`](../research/rust-macos-capability-notes.md) | Rust/Tauri × macOS 역량 조사. 크레이트 버전은 crates.io 로 검증됨 |
| `<기능>.md` | 기능별 명세. 아래 9개 절을 모두 갖는다 |
| [`../dev/architecture.md`](../dev/architecture.md) | ⭐ **구현 구조** — 크레이트 경계와 F-xx 대응, `CGEventTap` 콜백의 동시성·수명 설계, 명세를 벗어난 결정과 근거, 미확정 값에 대해 고른 기본값 |
| [`../dev/code-signing.md`](../dev/code-signing.md) | ⭐ **M0 절차** — 고정 자체 서명 인증서 생성(사용자가 직접 실행), 검증 방법, 실패 시 증상과 대처 |
| [`../dev/manual-verification.md`](../dev/manual-verification.md) | ⭐ **자동화 불가능한 검증 절차** — "M1 완료 판정" 5개 항목을 각각 어떻게 확인하는가 |

각 명세의 절 구성: **1 개요 · 2 사용자 시나리오 · 3 동작 명세 · 4 설정 항목 · 5 엣지 케이스와 실패 모드 · 6 필요한 플랫폼 API · 7 구현 접근 · 8 수용 기준 · 9 미해결 질문**

> 구현을 시작할 때 **8절 수용 기준이 완료 정의**다. 7절의 구현 접근 판정을 벗어나야 한다면 먼저 근거를 보고하고 명세를 고친 뒤 진행한다.

## 구현 접근 3분류

| 분류 | 정의 |
| :--- | :--- |
| **순수 Rust** | 안전(safe) 래퍼 크레이트만으로 커버. `unsafe` 도 네이티브 소스도 없다 |
| **Rust 바인딩** | `unsafe` FFI 직접 호출이 필요. `objc2-*` 자동 생성 바인딩이거나 수기 `extern "C"` 선언. **별도로 빌드하는 `.m`/`.swift` 파일은 없다** |
| **네이티브 shim 불가피** | Rust 에서 호출할 방법이 없어 Swift/Objective-C 소스를 별도 컴파일해 링크해야 함 |

⭐ **15개 기능 중 "네이티브 shim 불가피" 는 없다.** v1.66 실측이 이 판정을 뒤집지 않았다 — 원본이 쓰는 프레임워크는 전부 C ABI 또는 Objective-C 로 노출된다. 근거와 그럼에도 남는 위험은 [`platform-constraints.md` §0·§6](platform-constraints.md) 에 있다.

---

## 기능 전체 표

| ID | 기능 | 명세 | 구현 접근 | 의존 | 난이도 |
| :--- | :--- | :--- | :--- | :--- | :---: |
| **F-01** | Seek — 활성화와 세션 상태 머신 | [`seek-activation-and-session.md`](seek-activation-and-session.md) | Rust 바인딩 | F-07, F-02, F-03, F-04 | 중 |
| **F-02** | Seek — 텍스트 후보 검출 (Vision OCR + AX) | [`seek-text-detection.md`](seek-text-detection.md) | Rust 바인딩 | F-11 | **상** |
| **F-03** | Seek — 화면 오버레이 UI | [`seek-overlay-ui.md`](seek-overlay-ui.md) | Rust 바인딩 (`ns_window()` 필수) | F-02 | **상** |
| **F-04** | Seek — 클릭 실행 (**클릭 모드 7종**) | [`seek-click-execution.md`](seek-click-execution.md) | Rust 바인딩 | F-02, F-03, F-11 | 중 |
| **F-05** | Hyperkey (hyper · meh · bleh) | [`hyperkey.md`](hyperkey.md) | Rust 바인딩 | F-07 | 중 |
| **F-06** | 트랙패드·Magic Mouse hyper 제스처 | [`trackpad-hyper-gesture.md`](trackpad-hyper-gesture.md) | Rust 바인딩 ⚠️ **비공개 API** | F-05, F-07 | **상** |
| **F-07** | 키 리매핑 엔진 (공통 기반, **경로 3종**) | [`key-remapping-engine.md`](key-remapping-engine.md) | Rust 바인딩 (+ `hidutil` 폴백은 순수 Rust) | F-11 | **상** |
| **F-08** | Power User Presets (16종) | [`power-user-presets.md`](power-user-presets.md) | Rust 바인딩 | F-07, F-05 | 중 |
| **F-09** | 환경설정 UI | [`preferences-ui.md`](preferences-ui.md) | Rust 바인딩 | F-15, 전 기능의 설정 소유 | 중 |
| **F-10** | 메뉴바 상주 · 앱 수명주기 · **앱별 비활성화** | [`menu-bar-and-lifecycle.md`](menu-bar-and-lifecycle.md) | Rust 바인딩 | F-11, F-07 | 하 |
| **F-11** | 권한 온보딩과 복구 | [`permissions-onboarding.md`](permissions-onboarding.md) | Rust 바인딩 | — | 중 |
| **F-12** | 라이선싱과 트라이얼 (Paddle Classic) | [`licensing-and-trial.md`](licensing-and-trial.md) | 순수 Rust (+ Keychain·Security 바인딩) | — | 하 |
| **F-13** | 자동 업데이트 (Sparkle 2.x) | [`auto-update.md`](auto-update.md) | Rust 바인딩 (`Sparkle.framework`) | F-10 | 중 |
| **F-14** | 현지화와 키보드 입력 소스 독립성 | [`localization-and-input-sources.md`](localization-and-input-sources.md) | Rust 바인딩 | F-07 (입력 소스), 전 UI (현지화) | 중 |
| **F-15** ⭐ | 설정 저장 모델과 무결성 (충돌 감지 · 클라우드 동기화) | [`settings-store-and-integrity.md`](settings-store-and-integrity.md) | 순수 Rust | — | 하 |

> ⭐ **v1.66 검증이 만든 변화**
> - **F-15 신설** — 설정 저장 모델("부재 = 기본값")이 실측으로 확정되었고, 명세에 없던 **설정 충돌 감지 대화상자**와 **iCloud 설정 동기화**가 실재함이 드러났다. 셋 다 "설정값을 둘러싼 무결성" 이라는 하나의 관심사라 한 문서로 묶었다. 기각한 대안: 세 개의 얇은 문서로 분리 — 근거가 얇아(대부분 실행 파일 문자열) 각각을 독립 문서로 만들면 추측으로 분량을 채우게 된다.
> - **F-10 이 앱별 비활성화를 흡수** — 메뉴바의 `Ignore <앱이름>` 항목이 그 UI 이고 F-10 이 이미 앱 수명주기를 소유하므로 독립 문서를 만들지 않았다. F-07 은 이 상태를 게이트로 구독만 한다.
> - **meh/bleh 는 F-05 에 유지** — 상태 기계가 hyper 와 동일하고 modifier 마스크만 다르다. 분리하면 같은 표를 세 번 쓰게 된다.
> - **quick press 판정은 F-07 에 유지** — caps lock·shift·hyper 세 곳이 같은 타이밍 판정을 공유하므로 엔진이 소유하는 것이 맞다.

## 의존 관계

```
F-11 권한 ──┬─► F-07 리매핑 엔진 ──┬─► F-05 Hyperkey ──► F-06 트랙패드·Magic Mouse 제스처
            │       (경로 A/B/C)   │
            │                      ├─► F-08 Presets
            │                      │
            │                      └─► F-14(B) 입력 소스 독립성
            │
            ├─► F-02 텍스트 검출 ──► F-03 오버레이 ──► F-04 클릭 실행
            │   (Vision OCR +AX)                          │
            │        F-01 세션 상태 머신 ◄────────────────┘
            │           (F-07 · F-02 · F-03 · F-04 를 모두 소비)
            │
            └─► F-10 메뉴바·수명주기·앱별 비활성화 ──► F-13 자동 업데이트
                        │
                        └─► (앱별 비활성화 상태를 F-07 이 게이트로 구독)

F-15 설정 저장·무결성 ── F-09 가 올라타는 토대. 다른 기능의 설정이 생기기 전에 있어야 한다
F-09 환경설정 UI      ── F-15 위에서 모든 기능의 설정을 소유. 각 기능과 병행
F-12 라이선싱         ── 독립. 언제든 착수 가능
F-14(A) 현지화        ── 횡단. 원본에 없는 클론 고유 선택지 (D4)
```

⭐ **v1.66 검증이 의존 관계에 더한 것**
- **F-07 → F-10 의 역방향 의존이 생겼다.** 앱별 비활성화(`Ignore <앱이름>`)는 F-10 이 소유하지만, 그 상태를 **F-07 의 이벤트 탭 콜백이 최우선 게이트로 읽어야 한다.** 두 기능을 완전히 순서대로 만들 수 없다 — F-10 이 상태를 노출하는 인터페이스만 F-07 보다 먼저 정해두고, 실제 목록 관리 UI 는 나중에 붙인다.
- **F-15 가 F-09 앞에 온다.** "부재 = 기본값" 저장 모델은 나중에 얹을 수 없다 — 기본값을 파일에 미리 써버리는 구조로 시작하면 모든 기본값 검증이 무의미해진다.
- ⭐ **F-09 가 F-08 앞에 온다**(2026-08-30, 이슈 #13). 원래 M2 순서는 F-15 → F-08 → F-09 였으나, 환경설정 UI 가 없으면 M1 이 만든 hyper 동작을 **눈으로 검증할 방법 자체가 없다**는 것이 M1 검증에서 드러났다. 근거와 기각한 대안은 아래 "순서를 이렇게 정한 근거" 표에 있다.

---

## ⭐ 구현 순서

### M0 — 개발 워크플로 세팅 (**1일차에 반드시**)

명세 문서가 아니라 환경 작업이지만, **이걸 나중에 하면 그 전까지의 모든 테스트가 헛것이 된다.**

- Xcode Command Line Tools 설치 (`xcode-select --install`). 전체 Xcode 는 필요 없다
- **고정 자체 서명 인증서** 생성 + 키체인 "항상 신뢰" + 번들 ID 고정
- `tauri build` → `codesign` → 실행 을 개발 루프로 확립. **`tauri dev` 로는 권한 기능을 테스트하지 않는다**
- 타깃은 **`x86_64` + `arm64` universal** (원본이 그렇다 — `platform-constraints.md` §3.6)

근거: [`platform-constraints.md` §3](platform-constraints.md). ad-hoc 서명은 재빌드마다 TCC 권한을 잃는다.

---

### ⭐ M1 — 키 이벤트 탭 + 리매핑 엔진 (**다음 위임의 대상**)

> **이 마일스톤이 이 PR 이 머지된 뒤 곧바로 별도 위임될 범위다.** Hyperkey 의 최소 동작(hyper 소스 키 하나가 `⌃⌥⌘⇧` 를 합성)까지 포함한다.

**왜 이것이 첫 단계인가**: 리매핑 엔진은 F-05·F-08·F-01 이 **모두 올라타는 단일 토대**다. 엔진을 나중에 만들면 기능마다 event tap 을 따로 붙이게 되고, 그것이 원본이 v1.20·v1.62 에서 실제로 겪은 버그의 원인이다. 그리고 v1.66 실측이 엔진의 요구를 크게 늘렸다(경로 3종, 워치독, 핫플러그) — 이 복잡도를 다른 기능과 섞어서 만들 수 없다.

| # | 범위 | 명세 | 이번 검증이 더한 것 |
| :--- | :--- | :--- | :--- |
| 1 | **F-11 권한 온보딩** (최소) | [`permissions-onboarding.md`](permissions-onboarding.md) | ⭐ 확인할 권한은 `AXIsProcessTrusted` **하나**다. Input Monitoring·Screen Recording 은 사전 확인하지 않고 **실패-재시도**로 흡수한다. 자체 모달로 안내하고 시스템 프롬프트를 띄우지 않는다 |
| 2 | **F-07 리매핑 엔진** | [`key-remapping-engine.md`](key-remapping-engine.md) | ⭐ **경로 A(`CGEventTap`) / B(IOHID `UserKeyMapping`) / C(`IOHIDSetModifierLockState`) 3종**. 워치독 + 절전·깨어남·세션 훅 + 재시작 디바운스. **키보드 핫플러그 재적용**. 탭 생성 실패는 재시도가 아니라 종료 |
| 3 | **F-14(B) 입력 소스 독립성** | [`localization-and-input-sources.md`](localization-and-input-sources.md) | ⭐ `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 폴백. **F-07 과 함께** 한다 — 나중에 하면 판정 로직을 전부 다시 쓴다 |
| 4 | **F-05 Hyperkey 최소 동작** | [`hyperkey.md`](hyperkey.md) | ⭐ 조합은 **단일 `CGEventFlags` 비트마스크**로 저장한다(`hyperFlags = 1966080` 실측). 소스 키 enum 35종. `Apply modifiers to …` 는 **`Click` 만 기본 ON** |
| 5 | **F-10 인터페이스만** | [`menu-bar-and-lifecycle.md`](menu-bar-and-lifecycle.md) | 앱별 비활성화 상태를 F-07 이 게이트로 읽을 **인터페이스**만 먼저 정한다. 메뉴 UI 는 M3 |

> ✅ **M1 완료 판정**
> - 아무 리매핑도 없이 event tap 이 24시간 살아 있고, 절전 복귀·로그인·외장 키보드 연결 후에도 살아 있다
> - `caps lock` 을 hyper 로 지정하면 다른 앱이 `⌃⌥⌘⇧` 4개 modifier 를 인식한다
> - `Include shift in hyper key` 를 끄면 `⌃⌥⌘` 3개만 합성된다
> - QWERTZ·AZERTY·Dvorak·한글 입력기에서 동일하게 동작한다
> - Secure Input 구간에서 경로 A 가 무력화되어도 앱이 죽거나 stuck modifier 를 남기지 않는다

---

### M2 — 설정 토대와 리매핑 제품화 (F-15 → F-09 → F-08 → F-10)

**왜 여기인가**: Seek 보다 먼저다. 리매핑은 원본에서도 별도 앱(Hyperkey)으로 완결됐던 기능이라 **단독으로 출하 가능한 가치**를 만든다. Seek 은 훨씬 크고 위험하다.

⭐ **M2 를 두 차수로 나눈다(이슈 #13).** 1차가 설정 토대와 환경설정 창을, 2차가 프리셋과 메뉴바를 맡는다.

#### M2 1차 — 설정 토대와 환경설정 창 (이슈 #13)

6. **F-15 설정 저장 모델** — ⭐ **"부재 = 기본값"** 규약. 나중에 얹을 수 없다. ⭐ **저장 시점 정책과 종료 유실 대응**도 여기서 정한다(아래 표)
7. **F-09 환경설정 UI** — `Hyperkey`·`General` 탭만. `Seek`·`Presets` 탭은 자리만 만들고 비운다. ⭐ 종속 표현 3종(dimmed / 숨김 / 문장 중간 삽입), 탭별 창 리사이즈, 팝업 선택지 전량
8. **F-05 Hyperkey 배선** — UI 에서 켜고 끈 값이 실제로 엔진에 반영된다. `main.rs` 의 하드코딩된 `HyperkeySettings::default()` 를 없앤다

#### M2 2차 — 프리셋과 메뉴바 (이슈 #15) — ✅ **완료 (2026-08-30)**

9. **F-08 Power User Presets** — 16종 전수 + 상호작용 중재 + ⭐ **충돌 감지 대화상자 3종**
10. **F-10 메뉴바·수명주기** — 상주 앱, 메뉴 구조, ⭐ **앱별 비활성화 UI**, `unauthorizedMenu`, `Launch on login`

> ✅ M2 1차 완료 판정: 환경설정 창에서 hyper 키를 켜면 재빌드 없이 `⌃⌥⌘⇧` 가 합성되고, 설정을 한 번도 건드리지 않은 상태에서 **저장 파일이 아예 생기지 않는다.**
> ✅ M2 완료 판정: 원본 Hyperkey 앱과 동등한 기능을 환경설정으로 켜고 끌 수 있다.

⭐ **M2 2차가 명세를 바꾼 것 — 반드시 함께 읽어라**

| 무엇이 바뀌었나 | 어디에 |
| :--- | :--- |
| ⭐ **결정 D-1 — caps lock 모멘터리 정규화.** caps lock 은 래칭 키라 뗌 이벤트가 오지 않고, 그래서 캡스락 그룹 7종이 경로 A 만으로는 성립하지 않는다. caps lock 의존 규칙이 하나라도 켜지면 경로 B 로 `caps lock → F18` 커널 매핑을 설치하고 중재기가 F18 을 caps lock 으로 되돌려 판정한다 | [`../dev/architecture.md` §6.1](../dev/architecture.md) · `key-remapping-engine.md` §5 #20 |
| ⭐ **중재 규칙표 P1~P12** — R1~R8 을 코드가 따를 수 있는 형태로 확정. v1.20·v1.62 회귀 방지의 실제 구현 지점 | [`../dev/architecture.md` §6.4](../dev/architecture.md) |
| **`Advanced ▸ Synthesize Caps Lock Remap` 을 "기각" 에서 "재현" 으로 뒤집었다** — D-1 이 커널 매핑을 기본 경로로 올렸으므로 끄는 수단이 반드시 함께 있어야 한다 | `menu-bar-and-lifecycle.md` §3.3 |
| **팝업 계수 정정** — `Quick press caps lock to execute:` 는 49종이 아니라 **48종 + 구분선 1** 이다(근거 문서가 구분선을 함께 셌다) | `power-user-presets.md` §9.1-bis · `preferences-ui.md` §4.3 |
| **`(미확정)` 8개 자리에서 이 구현이 고른 값과 근거** — 홈로우 11키 매핑, `Home`/`End` 출력, 슬라이더 step, 토글 3계열 우선순위, Colemak/Dvorak·`Apply hyper to arrows`·Windows 키보드 미구현 결정 | `power-user-presets.md` §9.1-bis |

⚠️ **실기기 검증의 한계도 함께 기록되어 있다** — 검증 기기의 Karabiner-Elements 가 경로 B 보다 아래에서 caps lock 을 바꾸고 있어 **물리 caps lock 으로의 검증 자체가 성립하지 않는다.** 무엇을 확인했고 무엇을 확인하지 못했는지는 [`../dev/manual-verification.md` "실측 결과 (M2 2차)"](../dev/manual-verification.md) 에 항목별로 구분해 적었다.

### M3 — Seek (F-02 → F-03 → F-01 → F-04)

**왜 이 순서인가**: 검출이 없으면 그릴 게 없고, 그릴 게 없으면 세션을 만들 이유가 없고, 세션이 없으면 클릭할 대상이 없다. 데이터가 흐르는 방향 그대로다.

11. **F-02 텍스트 검출** — 먼저 **실측 스파이크**: 전체 화면 OCR 지연을 잰다. ⭐ 캡처는 `CGDisplayCreateImage`(ScreenCaptureKit 아님), **CoreImage 전처리 파이프라인** 포함
12. **F-03 오버레이** — ⭐ **창이 여러 개**다(디스플레이별 하이라이트 + 독립 검색 바 400×40pt, 위치 저장). 렌더링 계층은 **교체 가능하게 분리**(`platform-constraints.md` P3)
13. **F-01 세션 상태 머신** — ⭐ **활성화 경로 3종**(단축키 / 키 리맵 / quick press caps lock → Seek). 화살표·tab 순환, enter 확정
14. **F-04 클릭 실행** — ⭐ **클릭 모드 7종**. 합성 클릭이 자기 탭으로 되돌아오지 않게 차단

> ✅ M3 완료 판정: 개발자 권장 설정(Caps Lock → Seek, hold 모드, `;` 순환)이 다중 디스플레이에서 동작한다.

### M4 — 배포 (F-13 → F-12)

15. **F-13 자동 업데이트** — Sparkle appcast·EdDSA. ⚠️ 신·구 앱의 **서명 주체가 같아야 TCC 권한이 유지**된다
16. **F-12 라이선싱** — 상태 머신과 추상 인터페이스까지. ⭐ 원본은 **Paddle Classic** 이며 자체 코드 서명 검증을 병용한다 — 개발 중 자체 서명에서 막히지 않게 우회 가능한 형태로 설계한다

### M5 — 마무리와 선택 기능 (F-14(A) → F-06)

17. **F-14(A) 현지화** — ⭐ **원본에는 없는 기능이다**(영어 단일). 클론이 할지 말지는 제품 결정 D4
18. **F-06 트랙패드·Magic Mouse 제스처** — ⚠️ **최후순위**. 비공개 `MultitouchSupport` 에 의존해 OS 업데이트로 조용히 깨질 수 있다. hyper 활성화의 **대체 경로**일 뿐이므로 실패 시 격하 가능한 선택 기능으로 설계한다

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
| ⭐ **M1 을 "권한 + 엔진 + Hyperkey 최소 동작" 으로 묶어 한 번에 위임** | v1.66 실측이 엔진의 요구를 크게 늘렸다 — 리매핑 경로 3종, 워치독, 절전·깨어남 훅, 키보드 핫플러그. 이 복잡도는 **동작하는 것을 눈으로 확인할 수 있는 최소 기능**(hyper 키 하나)이 함께 있어야 검증 가능하다. 권한 없이는 탭이 뜨지 않으므로 F-11 도 뗄 수 없다 | ① 엔진만 먼저 — 아무것도 관찰되지 않아 "탭이 살아 있다" 외에 검증할 것이 없다. ② M1 에 Presets 16종까지 포함 — 증분이 커져 리뷰가 불가능하고, 충돌 중재 규칙이 엔진 설계를 오염시킨다 |
| ⭐ **M2 안에서 F-09 를 F-08 앞으로 당긴다** (2026-08-30, 이슈 #13) | M1 완료 판정 5개 중 2~5번(hyper `⌃⌥⌘⇧` 합성, `Include shift` 토글, meh/bleh)이 **환경설정 UI 가 없어서 검증 불가** 상태로 남았다 — `main.rs` 의 `HyperkeySettings::default()` 를 고쳐 재빌드해야만 hyper 를 켤 수 있었기 때문이다. F-09 를 먼저 내면 **M1 의 미검증 항목이 즉시 풀린다.** 의존 관계도 이를 허용한다: F-09 는 F-15 에만 의존하고 F-08 은 F-07·F-05 에 의존하므로 서로 막지 않는다 | ① 원래 순서(F-08 먼저) 유지 — M1 검증 부채가 M2 끝까지 남고, 그 부채 위에 프리셋 16종이 쌓인다. ② M2 를 쪼개지 않고 한 번에 위임 — 증분이 커져 리뷰가 불가능하고, M1 이 1시간 35분 만에 세션 한도로 끊긴 전례가 있다 |
| ⭐ **F-15(설정 저장 모델)를 F-09 앞에** | "부재 = 기본값" 은 나중에 얹을 수 없다. 기본값을 파일에 미리 써버리는 구조로 시작하면 이번 검증이 쓴 방법(plist 를 보고 기본값을 확정)이 클론에서는 통하지 않게 된다 | F-09 안에 저장 계층을 흡수 — 설정 소유자와 저장 규약이 뒤섞여 마이그레이션 설계가 늦어진다 |
| ⭐ **앱별 비활성화를 F-10 에 흡수, 독립 문서를 만들지 않음** | UI 가 메뉴바 항목 하나(`Ignore <앱이름>`)뿐이고 F-10 이 이미 최전면 앱 추적·수명주기를 소유한다. F-07 은 상태를 게이트로 구독만 한다 | 독립 문서 F-16 — 9절을 채울 근거가 없어 절반이 `(미확정)` 이 된다 |
| ⭐ **충돌 감지·클라우드 동기화·저장 모델을 F-15 한 문서로** | 셋 다 "설정값을 둘러싼 무결성" 이고, 뒤 둘은 근거가 얇다(실행 파일 문자열뿐, UI 미관찰). 한 문서에 모아야 그 불확실성이 한곳에 보인다 | 세 개의 얇은 독립 문서 — 근거 대비 분량을 맞추려다 추측으로 메우게 된다 |
| ⭐ **meh/bleh 를 F-05 에 유지, quick press 를 F-07 에 유지** | meh/bleh 는 hyper 와 상태 기계가 동일하고 modifier 마스크만 다르다. quick press 는 caps lock·shift·hyper 세 곳이 같은 타이밍 판정을 공유한다 | 각각 독립 문서 — 같은 표를 세 번 쓰거나, 판정 로직이 세 곳에 복제된다 |

---

## ✅ 착수 전 제품 결정 — 전부 확정됨

> **상태: D1~D4 전부 확정 (2026-08-30, 이슈 #5).** 이 표는 더 이상 "답해야 하는 질문"이 아니라 **확정된 답과 그 근거**다.
> 결정의 원문은 [이슈 #5](https://github.com/johnpark-bin/Ultrakey/issues/5) 본문 "착수 전 제품 결정 — 확정됨" 절에 있다.

| # | 질문 | ✅ 확정된 답 | 근거 · 범위 영향 | 관련 |
| :--- | :--- | :--- | :--- | :--- |
| **D1** | 최소 macOS 버전 12.0 vs 12.3 | **12.0 을 유지한다** | 이미 해소된 것을 재확인. SuperKey v1.66 이 `LSMinimumSystemVersion = 12.0` 을 유지하며 ScreenCaptureKit 없이 `CGDisplayCreateImage` 로 출하 중이다 — 12.3 상향의 유일한 근거가 사라졌다. 구현 반영: `apps/ultrakey-app/tauri.conf.json` 의 `bundle.macOS.minimumSystemVersion` | F-02, `platform-constraints.md` §0.2 (a) |
| **D2** | 라이선싱을 실제로 구현할 것인가 | **상태 머신 + 추상 인터페이스까지만. 실제 Paddle 연동은 보류** | F-12 명세의 제안을 그대로 채택한다. 대상 자체는 실측으로 확정되어 있다(**Paddle Classic**, 제품 ID `750314`, 자체 코드 서명 검증 병용). **M4 범위이며 M1 과 무관하다.** 다만 원본이 자기 서명을 검증하므로(`SecStaticCodeCreateWithPath`, `anchor apple generic`), 개발 중 자체 서명 인증서에서 이 검증이 실패해 **개발 루프를 막지 않도록 우회 경로를 F-12 명세에 명시해 둔다** | F-12, `platform-constraints.md` §3.6 |
| **D3** | F-06 트랙패드 제스처를 범위에 넣을 것인가 | **범위에 두되 최후순위(M5). 실패 시 격하 가능한 선택 기능으로 설계** | 위험이 선명하다 — `MultitouchSupport` 는 번들에서 **유일한 PrivateFramework** 이고, 부를 함수 9개는 확정됐으나 **시그니처는 여전히 비공개**다(P6). Magic Mouse 분기와 손바닥·엄지 거부까지 필요해 범위도 예상보다 크다. 그러나 이것은 **hyper 활성화의 대체 경로일 뿐**이라 빠져도 제품이 성립한다 — 그래서 "빼기"가 아니라 "최후순위 + 격하 가능"으로 정했다 | F-06, `platform-constraints.md` §0.2 (b), P6 |
| **D4** ⭐ | 현지화를 할 것인가 | **한다 — 한국어 + 영어** | ⚠️ **원본에 없는 기능이다.** 실측 결과 SuperKey 는 영어 단일이며(`Base.lproj` 하나, `CFBundleLocalizations` 없음), 기존 명세가 근거로 삼았던 "8개 로케일"은 Sparkle 프레임워크의 로케일을 오독한 것이었다. 따라서 이것은 **원본 추종이 아니라 클론 고유의 선택**이다. ⭐ **M1 에 영향이 있다** — 권한 안내 모달 등 사용자 대면 문자열이 M1 에서 이미 생기므로, UI 가 굳기 전에 **카탈로그 구조를 M1 에서 잡았다**(`ultrakey-i18n` + `resources/i18n/{en,ko}.json`, `localization-and-input-sources.md` §3.1.5 의 "단일 카탈로그" 결정을 따름). F-14(A) 의 나머지(RTL 등)는 M5 | F-14, `app-bundle-analysis.md` §5.1 |

### 이 결정들이 구현 순서에 미친 영향

- **D4 만 M1 을 건드렸다.** D2·D3 는 각각 M4·M5 범위라 M1 착수를 막지 않는다.
- D4 채택으로 **로케일 목록이 확정**되었으므로, `localization-and-input-sources.md` §3.1.1 이 "클론이 정할 문제"로 되돌려 두었던 로케일 집합은 이제 **`en` + `ko` 2종**이다. 다만 §3.1.4 의 **RTL 미러링 규칙은 M1 범위 밖이다** — `ko`·`en` 둘 다 LTR 이라 M1 에서는 적용 대상이 없다. 규칙 자체는 나중에 RTL 로케일을 추가할 때를 위해 명세에 남겨 둔다.
- D3 를 "빼기"가 아니라 "최후순위"로 정했으므로 `trackpad-hyper-gesture.md` 는 폐기하지 않는다. `hyperkey.md` §3.5 의 조건부 표시 2종(`Change menu bar icon when engaged` · `Provide haptic feedback when triggered`)도 F-05 에 그대로 남는다.

## 미해결 질문의 소재

⭐ **v1.66 검증으로 대부분이 해소되었다.** 남은 것은 각 명세의 **9절**과 다음 세 곳에 모여 있다.

- **검증 결과 요약** — [`../research/spec-verification-report.md`](../research/spec-verification-report.md) 의 "미확정" 분류
- **관찰의 한계** — [`../research/app-bundle-analysis.md` §8](../research/app-bundle-analysis.md) (10건). 왜 확인하지 못했는지가 함께 적혀 있다
- **플랫폼 관련** — [`platform-constraints.md` §7](platform-constraints.md) (P1·P3~P11)

이전에 "앱을 설치하지 않고는 알 수 없다" 고 남겨두었던 것 — **각 설정의 출고 기본값 · 팝업 선택지 전체 목록 · `General` 탭의 실제 구성 · 메뉴바 메뉴 항목** — 은 **전부 해소되었다**.

⚠️ 반대로, [`../research/superkey-inventory.md`](../research/superkey-inventory.md) 는 이제 **일부가 사실이 아닌 문서**다(로케일 수, 홍보 스크린샷의 체크 상태를 기본값으로 읽은 것). 웹 조사의 기록으로서 그대로 보존하되, **실측과 충돌하면 `app-bundle-analysis.md` 가 우선한다.**


