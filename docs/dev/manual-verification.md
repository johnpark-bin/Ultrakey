# 수동 검증 절차 (M1 · M2)

> **이 문서의 목적**: `docs/spec/README.md` 의 **"✅ M1 완료 판정" 5개 항목**(항목 1~5)과,
> **M2 2차가 더한 F-08 프리셋·F-10 메뉴바**(항목 6·7, 이슈 #15)를 각각 어떻게 확인하는지 적는다.
> 자동화할 수 있는 것은 이미 `cargo test` 에 들어가 있다(§0). 여기 남은 것은 **실제 이벤트 탭·실제 TCC 권한·실제 절전 복귀가 필요해 자동화할 수 없는 것들**이다.

---

## ⚠️ 먼저 — 이 문서를 쓴 세션이 검증하지 못한 것

**M1 구현 세션은 아래 항목 검증을 사실상 하나도 완료하지 못했다.** 정직하게 적는다.

- 이 머신에는 코드 서명 인증서가 없다 — `security find-identity -v -p codesigning` → **`0 valid identities found`**
- 서명 인증서 생성과 키체인 "항상 신뢰" 설정은 **키체인 변경**이라 위임 범위 밖이며, 사용자가 직접 하기로 결정되었다([`code-signing.md`](code-signing.md) §2)
- 서명되지 않은 빌드로는 TCC 권한이 재빌드마다 소실되므로([`../spec/platform-constraints.md`](../spec/platform-constraints.md) §3.2), **권한이 필요한 어떤 항목도 의미 있게 검증할 수 없다**

**다만 다음 두 가지는 실제로 확인했다** — 이것까지 "못 했다"고 뭉뚱그리면 그것도 부정확하다:

| 확인한 것 | 결과 |
| :--- | :--- |
| universal `.app` 번들이 실제로 만들어지는가 | ✅ `cargo tauri build --target universal-apple-darwin` 성공. `lipo -archs` → `x86_64 arm64`, `LSMinimumSystemVersion` → `12.0` |
| 앱이 기동해 배선이 끝까지 이어지는가 | ✅ 카탈로그 로드 → 권한 판정 → 엔진 시작 → `CGEventTap` 생성 → `Active` 전이. 종료 후 남은 프로세스도, 경로 B 잔존 매핑도 없다 |

⚠️ **그러나 그 기동 테스트는 위 "더 교묘한 함정"에 그대로 걸린 상태에서 이루어졌다** — ad-hoc 서명 바이너리를 권한 있는 터미널에서 직접 실행했고, 그래서 권한이 `Granted` 로 나왔다. **즉 증명된 것은 "배선이 끊기지 않았다"까지이고, 권한 온보딩 경로(모달 표시·폴링 자동 진행·런타임 취소 감지)는 한 번도 실행되지 않았다.**

따라서 아래 절차는 **명세와 구현으로부터 도출한 것이지, 실행해 통과를 확인한 것이 아니다.**
[`code-signing.md`](code-signing.md) 의 M0 절차를 먼저 마친 뒤 이 문서를 처음부터 수행해야 한다.

---

### 📌 갱신 (2026-08-30, 이슈 #8) — 항목 1-a 는 이제 실제로 통과했다

M0 서명 절차가 끝나(`Ultrakey Dev` 인증서 확인됨) 위 전제가 해소됐고, **항목 1-a #1~#4 를
실제로 수행해 통과시켰다**([결과](#-실제-통과-결과-2026-08-30-이슈-8--pr-fixonboarding-modal-not-shown)).

그 과정에서 이슈 #8 이 드러났다 — **권한 미부여 상태에서 온보딩 모달이 "안 보였다."**
실제로는 창이 정상적으로 최상단·포커스 상태로 떠 있었고, **웹뷰 내용만 완전히 백지**였다.
`tauri.conf.json` 에 `app.withGlobalTauri` 가 없어(기본값 `false`) Tauri v2 가
`window.__TAURI__` 를 주입하지 않았고, `ui/index.html` 의 모듈 첫 줄
`const { invoke } = window.__TAURI__.core;` 가 `TypeError` 를 던져 렌더 함수가 한 번도
실행되지 못한 것이 원인이다.

⭐ **이 버그의 진짜 비용은 버그 자체가 아니라 "진단할 수 없었다"는 것이었다.**
`open` 으로 띄운 앱의 로그를 볼 방법이 없어(§0 에 적혀 있던 `log stream` 명령이 동작하지 않았다)
TCC 데이터베이스와 AppleScript AX 트리를 뒤져야 했다. 그래서 **로그를 파일로도 남기도록
먼저 고친 뒤**(§0) 원인을 찾았다 — 파일 로그를 확보하자마자 `modal_copy 커맨드 호출됨` 이
한 줄도 없다는 사실이 즉시 원인을 가리켰다. **앞으로도 이 순서를 지켜라: 관찰 수단을
먼저 확보하고, 그다음에 원인을 찾는다.**

아래 "검증하지 못한 것" 서술은 **항목 1-b 이후에 대해서는 여전히 유효하다.**

---

## 0. 사전 준비

```sh
# 1) M0 서명 절차를 마쳤는지 확인 — `Ultrakey Dev` 가 보여야 한다
security find-identity -v -p codesigning

# 2) 서명된 universal .app 빌드
./scripts/build-signed.sh
./scripts/verify-signature.sh <출력된 경로>/Ultrakey.app

# 3) ⭐ 반드시 `open` 으로 실행한다 (아래 함정 참조)
open <경로>/Ultrakey.app

# 4) ⭐ 앱 로그는 파일로 남는다 — `open` 으로 띄워도 볼 수 있다
tail -f ~/Library/Logs/Ultrakey/ultrakey.log

# 5) 더 자세히 보려면 — `open --env` 로 로그 레벨을 넘긴다
open --env ULTRAKEY_LOG=debug <경로>/Ultrakey.app
```

> ⭐ **로그 매 줄에 스레드 이름이 찍힌다(이슈 #10).** `main` / `ultrakey-tap` /
> `ultrakey-permission-poll` / `ultrakey-hotplug` / `ultrakey-delay-scheduler` /
> `ultrakey-watchdog` 중 어느 스레드가 그 줄을 남겼는지 바로 보인다. 이슈 #10 은
> "어느 스레드가 시스템 훅을 등록했는가" 가 원인의 전부였는데 로그에 그 정보가 없어
> 좁힐 수 없었다 — 그래서 상시로 켜 두었다.
>
> ⭐ **재시작 디바운스 판정 로그는 `debug` 가 아니라 `info` 다(이슈 #10 에서 변경).**
> 항목 1-b #4·#5 가 "디바운스로 재시작을 건너뛴 판정이 로그에 남는다" 를 **통과 근거로
> 요구**하는데, `debug` 면 기본 레벨에서 보이지 않아 검증자가 "디바운스가 동작했다" 와
> "그런 로직이 아예 없다" 를 구분할 수 없었다. 빈도도 낮다 — 5초 안에 복구가 두 번
> 예약될 때만 찍힌다. 따라서 `ULTRAKEY_LOG=debug` 없이도 관찰된다.
>
> ⭐ **로그 확인 명령은 이슈 #8 에서 정정됐다.** 이전 판에 적혀 있던
> `log stream --predicate 'process == "ultrakey-app"'` 은 **동작하지 않는다** —
> 앱의 `tracing` 로그는 macOS 통합 로그(`OSLog`)가 아니라 stderr 로 나가므로, 그 명령으로는
> 시스템 프레임워크가 대신 남긴 로그만 보이고 앱 자신의 로그는 한 줄도 나오지 않는다.
> 게다가 `open` 으로 띄우면 stderr 자체가 사라져 **아무 데서도 로그를 볼 수 없었다.**
> 이것이 이슈 #8 의 진단을 크게 지연시킨 원인이었으므로,
> 앱이 stderr 와 **파일** 양쪽에 로그를 남기도록 바꿨다(`main.rs` 의 `init_logging`).
>
> | 경로 | 내용 |
> | :--- | :--- |
> | `~/Library/Logs/Ultrakey/ultrakey.log` | ⭐ 항상 기록된다. `open` 실행 시 유일하게 볼 수 있는 곳 |
> | `~/Library/Logs/Ultrakey/ultrakey.log.1` | 직전 세대. 파일이 2 MiB 를 넘으면 한 번 롤오버된다 |
> | stderr | 터미널에서 직접 실행했을 때만. `open --stderr <경로>` 로 파일에 받을 수도 있다 |
>
> 로그는 **append** 되므로 실행마다 `=== Ultrakey 기동 ===` 구분선(pid·실행 파일 경로 포함)이 찍힌다.
> 한 번의 실행만 보려면 그 구분선부터 읽으면 된다.
>
> ⚠️ `--env` 를 붙여도 TCC 판정 주체는 바뀌지 않는다(여전히 `launchd` 가 부모다) —
> 이슈 #8 검증에서 `open --env ULTRAKEY_LOG=debug` 로 띄운 앱이 정상적으로
> `PermissionTransition { from: Unknown, to: Denied }` 를 기록하는 것을 실측으로 확인했다.

**최소한 이것들이 로그에 보여야 한다** — 안 보이면 그 자체가 진단 신호다.

| 로그 줄 | 무엇을 확인해 주는가 |
| :--- | :--- |
| `=== Ultrakey 기동 ===` | 프로세스가 실제로 떴고, 어떤 실행 파일인지(`.app` 안인지) |
| `권한 상태 전이 transition=...` | 권한 판정과 이후 전이 |
| `on_permission_transition 진입 to=...` | 어느 분기를 탔는지 |
| `show_modal 호출 후 창 상태 is_visible=... is_focused=...` | 창이 실제로 떴는지 |
| ⭐ `modal_copy 커맨드 호출됨 kind=...` | **웹뷰가 살아서 Rust 를 호출했는지.** 이 줄이 없으면 창은 떠 있어도 내용이 백지다 — 이슈 #8 이 정확히 그 상태였다 |
| `엔진 시작됨 state=...` · `탭 상태 변경 state=...` | 엔진 상태 전이 |

> ⚠️ **`cargo tauri dev` 를 쓰지 마라.** TCC 가 부모 프로세스(터미널)의 권한으로 판정하므로 결과가 전부 무의미해진다([`../spec/platform-constraints.md`](../spec/platform-constraints.md) §3.4). 앱은 이 상태를 감지하면 `dev.not_app_bundle` 경고를 로그에 남긴다 — 그 경고가 보이면 검증을 중단하고 서명된 빌드로 다시 시작한다.

> ⛔ **더 교묘한 함정 — `.app` 을 빌드했어도 터미널에서 바이너리를 직접 실행하면 안 된다.**
> ```sh
> # ⛔ 이렇게 하지 마라
> ULTRAKEY_LOG=debug <경로>/Ultrakey.app/Contents/MacOS/ultrakey-app
> ```
> **M1 구현 세션이 실제로 이것을 관찰했다**: 정상적으로 빌드한 `.app` 의 바이너리를 Accessibility 권한이 있는 터미널에서 직접 실행했더니, `AXIsProcessTrusted()` 가 **`true` 를 반환**하고 이벤트 탭이 `Active` 까지 도달했다 — 앱 자신에게는 권한이 없는데도 그랬다. TCC 가 **책임 프로세스(responsible process)** 인 부모 터미널의 권한으로 판정하기 때문이다.
>
> 이것은 `tauri dev` 함정(§3.4)과 원인은 같지만, "`.app` 을 빌드했으니 괜찮다"고 생각하는 순간을 노린다는 점에서 더 위험하다. **반드시 `open <경로>/Ultrakey.app` 으로 실행하라** — 그래야 `launchd` 가 부모가 되어 TCC 가 앱 자신의 서명으로 판정한다.
>
> 판별법: 권한을 부여하지 않은 상태에서 실행했는데 **온보딩 모달이 뜨지 않고 곧바로 탭이 `Active` 가 되면**, 부모 권한을 물려받은 것이다.

### 관찰 도구 — 전부 macOS 기본 제공, 추가 설치 불필요

| 도구 | 여는 법 | 무엇을 보는가 |
| :--- | :--- | :--- |
| **키보드 뷰어(Keyboard Viewer)** | 시스템 설정 → 키보드 → 입력 소스 → "메뉴 막대에 입력 메뉴 표시" 켜기 → 메뉴 막대 입력 메뉴 → "키보드 뷰어 보기" | ⭐ **현재 눌린 것으로 인식되는 modifier 를 실시간으로 강조 표시한다.** 항목 2·3·5 의 1차 관찰 수단 |
| `ioreg` | `ioreg -l -w 0 \| grep -i SecureInput` | Secure Input 을 켠 프로세스의 PID. 항목 5 |
| `hidutil` | `hidutil property --get UserKeyMapping` | 경로 B 의 커널 매핑 잔존 여부 |
| **앱 로그 파일** | `tail -f ~/Library/Logs/Ultrakey/ultrakey.log` | ⭐ 앱 자신의 `tracing` 로그. `open` 으로 띄웠을 때 유일하게 볼 수 있는 곳(§0) |

---

## 0-ter. 재발 방지 — 이슈 #8 이 남긴 것

이슈 #8(온보딩 모달 백지)은 **컴파일러도 기존 테스트도 아무 말을 하지 않았다.** `tauri.conf.json`
과 `ui/index.html` 은 서로 다른 파일이고 둘 사이에 타입 관계가 없어서, 어긋나도 빌드가 통과한다.
그래서 그 어긋남을 정적으로 잡는 테스트를 추가했다.

### 자동화한 것 — `apps/ultrakey-app/tests/frontend_wiring.rs` (`cargo test` 에 포함)

| 테스트 | 무엇을 막는가 |
| :--- | :--- |
| `index_html_이_tauri_전역을_쓰면_with_global_tauri_가_켜져_있어야_한다` | ⭐ **이슈 #8 그 자체.** `ui/index.html` 이 `window.__TAURI__` 를 참조하는데 `app.withGlobalTauri` 가 `true` 가 아니면 실패한다 |
| `tauri_conf_에_permissions_라벨_창이_있어야_한다` | 창 라벨이 바뀌면 `show_modal()`/`hide_modal()` 이 조용히 아무 일도 하지 않는다 |
| `index_html_의_body_는_transparent_배경을_쓰지_않는다` | 창 설정에 `transparent: true` 가 없는데 body 를 투명하게 두는 조합(백지 실패 모드를 악화시킨다) |

### ⛔ 자동화하지 못한 것과 그 이유

**"모달이 실제로 눈에 보이고 문구가 그려졌는가"는 자동 테스트로 옮기지 못했다.** 이유:

1. **웹뷰 렌더 결과를 확인하려면 앱을 실제로 띄워야 한다.** 그런데 `cargo test` 가 띄운
   프로세스는 부모(터미널)의 TCC 권한을 물려받으므로(§0 의 두 함정), 애초에 온보딩 경로를
   타지 않는다 — 테스트가 통과해도 검증한 것이 없다.
2. **모달이 뜨는 조건 자체가 "Accessibility 미부여"** 다. 이를 자동으로 만들려면 테스트가
   시스템 TCC 데이터베이스를 조작해야 하는데, 그것은 사용자의 시스템 보안 상태를 CI 나
   `cargo test` 가 건드린다는 뜻이라 받아들일 수 없다.
3. **"백지인가 아닌가"의 판정에는 화면 캡처나 AX 트리 조회가 필요하고**, 둘 다 각각
   Screen Recording · Accessibility 권한을 **테스트 러너에게** 요구한다. 검증하려는 바로 그
   권한 체계에 의존하는 테스트다.

따라서 이 부분은 **항목 1-a #1 의 수동 확인이 계속 소유한다.** 대신 수동 확인의 비용을
낮추도록, 아래 두 가지를 로그로 판정할 수 있게 만들어 뒀다(§0).

| 로그 | 판정 |
| :--- | :--- |
| `show_modal 호출 후 창 상태 is_visible=Ok(true)` | 창이 떴는가 |
| ⭐ `modal_copy 커맨드 호출됨 kind="onboarding"` | **웹뷰가 살아서 문구를 가져갔는가.** 이 줄이 없는데 위 줄만 있으면 = 이슈 #8 재발이다 |

⭐ **이 두 줄의 조합이 사실상의 회귀 탐지기다.** 다음에 "모달이 안 보인다"는 보고가 오면,
TCC 데이터베이스를 뒤지기 전에 `~/Library/Logs/Ultrakey/ultrakey.log` 에서 이 두 줄부터 확인하라.

---

## 0-bis. 자동화된 것 — 여기서 다시 확인하지 않는다

아래는 `cargo test` 가 이미 검증한다. 수동 절차에서 중복 확인할 필요가 없다.

```sh
cargo test --workspace
```

| 자동 검증 대상 | 어디에 |
| :--- | :--- |
| 비트마스크 합성 — `hyperFlags == 1966080`, bleh 에 option 없음 | `ultrakey-core` `flags` 테스트 |
| quick press 타이밍 판정 상태 머신 전이 전량 | `ultrakey-core` `quickpress` 테스트 |
| **v1.20 / v1.62 회귀 시나리오** | `ultrakey-core` `arbitration` 테스트 |
| 중재 계층 short-circuit | `ultrakey-core` `arbitration` 테스트 |
| 키코드 매핑 · 소스 키 35종 | `ultrakey-core` `keycode` 테스트 |
| auto-repeat 이 `PendingDown` 으로 되돌리지 않음 | `ultrakey-core` 테스트 |
| 앱 게이트 판정 | `ultrakey-core` `gate` 테스트 |
| 레이아웃 역방향 테이블 구축 (가짜 레이아웃 주입) | `ultrakey-layout` 테스트 |
| 문자열 카탈로그 ko/en 키 집합 동일성 | `ultrakey-i18n` 테스트 |

---

## 항목 1 — 아무 리매핑도 없이 event tap 이 24시간 살아 있고, 절전 복귀·로그인·외장 키보드 연결 후에도 살아 있다

> 근거 명세: `key-remapping-engine.md` §3-a(생명주기 상태표) · §5 #1~#5, #10 · §8

### 1-a. 권한 온보딩 (F-11) — 먼저 통과해야 나머지가 성립한다

| # | 조작 | 기대 결과 |
| :--- | :--- | :--- |
| 1 | Accessibility 를 **부여하지 않은** 상태로 앱 실행 | 자체 모달이 뜬다. **macOS 표준 "허용/거부" 프롬프트가 뜨면 실패** — 원본도 클론도 시스템 프롬프트를 띄우지 않는다(F-11 §1.1) |
| 2 | 모달의 `시스템 설정 열기` 클릭 | 시스템 설정의 **손쉬운 사용** 패널이 직접 열린다 |
| 3 | 목록에서 Ultrakey 체크 → 앱으로 돌아옴 | ⭐ **별도의 "확인" 버튼 클릭 없이** 폴링만으로 모달이 사라지고 진행된다(F-11 §8) |
| 4 | 앱 종료 후 재실행 | 온보딩 모달이 **다시 나타나지 않는다** |
| 5 | 실행 중에 시스템 설정에서 Accessibility 를 **끈다** | 배경 폴링 주기(기본 5초) 이내에 앱이 감지한다. 로그에 권한 상실 전이가 남는다 |
| 6 | macOS 12 환경이 있다면 거기서도 실행 | 안내 문구가 "시스템 환경설정" 어휘로 바뀐다(F-11 §3.2, `get_os_variant`) |

#### ✅ 실제 통과 결과 (2026-08-30, 이슈 #8 / PR `fix/onboarding-modal-not-shown`)

**#1~#4 를 실제로 수행해 통과시켰다.** 환경: macOS 26.5.2, 3840×1600 + 2560×1440 듀얼,
`./scripts/build-signed.sh` 로 만든 서명된 universal `.app` 을 `open --env ULTRAKEY_LOG=debug` 로 실행.
권한은 매 시행 전에 `tccutil reset Accessibility app.ultrakey.Ultrakey` 로 미부여 상태로 되돌렸다.

| # | 결과 | 근거 |
| :--- | :---: | :--- |
| 1 | ✅ | 로그 `권한 상태 전이 transition=PermissionTransition { from: Unknown, to: Denied }` → `show_modal 호출 후 창 상태 is_visible=Ok(true) ... is_focused=Ok(true)` → `modal_copy 커맨드 호출됨 kind="onboarding"`. **스크린샷으로 모달 본문 전체를 눈으로 확인**했고, AX 트리에 제목·본문·경로·힌트·버튼 텍스트가 전부 노출된다. macOS 표준 프롬프트는 뜨지 않았다 |
| 2 | ✅ | 모달의 `Open System Settings` 버튼을 AX 로 눌렀더니 System Settings 가 최전면이 되고 **창 제목이 `Accessibility`** — 손쉬운 사용 패널이 직접 열렸다. 목록에 `Ultrakey.app` 항목이 보인다 |
| 3 | ✅ | 모달이 떠 있는 상태에서 목록의 체크박스를 켜자, **아무 버튼도 누르지 않았는데** 온보딩 폴링 주기 안에 `PermissionTransition { from: Denied, to: Granted }` → `hide_modal 호출 후 창 상태 is_visible=Ok(false)` → `엔진 시작됨 state=Active` 가 이어졌다. ⭐ 애초에 모달에 "확인" 버튼이 **존재하지 않는다**(버튼은 `Open System Settings` 하나뿐) — 폴링 자동 진행이 구조적으로 강제된다 |
| 4 | ✅ | 권한이 켜진 상태로 종료 후 재실행 → `PermissionTransition { from: Unknown, to: Granted }` → `hide_modal` → `엔진 시작됨 state=Active`. **AX 로 창 개수를 세면 `0`** — 모달이 뜨지 않았다 |

**증거 스크린샷**

| 파일 | 무엇 |
| :--- | :--- |
| [`screenshots/issue-8-before-blank-modal.png`](screenshots/issue-8-before-blank-modal.png) | ⛔ 수정 전 — 창은 떠 있으나 **완전한 백지**. 이슈 #8 의 증상 그 자체 |
| [`screenshots/issue-8-after-onboarding-modal.png`](screenshots/issue-8-after-onboarding-modal.png) | ✅ 수정 후 — 제목·본문·경로·체크박스 힌트·잠금 힌트·`Open System Settings` 버튼이 전부 그려진다(#1) |
| [`screenshots/issue-8-accessibility-pane.png`](screenshots/issue-8-accessibility-pane.png) | ✅ 버튼을 눌러 열린 손쉬운 사용 패널. 목록에 `Ultrakey.app` 이 보인다(#2) |

⚠️ **#5·#6 은 이번에도 수행하지 않았다.** #5(실행 중 권한 회수 감지)는 이번 범위 밖이고,
#6 은 macOS 12 환경이 없다. 통과했다고 적지 않는다.

⚠️ **검증을 위해 Accessibility 를 켰다가 되돌렸다.** 시작 상태는 `app.ultrakey.Ultrakey|0`(항목은
있으나 미부여)이었고, 종료 상태는 `tccutil reset Accessibility app.ultrakey.Ultrakey` 로
**항목 자체가 없는** 상태다. 둘 다 앱 입장에서는 `AXIsProcessTrusted() == false` — 동일하게
`Denied` 로 판정된다. 다른 앱 41개의 Accessibility 항목은 건드리지 않았다(리셋 대상을
`app.ultrakey.Ultrakey` 하나로 한정했다).

⛔ **앱 자신은 `tccutil` 을 호출하지 않는다** — 위 리셋은 검증자가 손으로 돌린 것이다.
자동 TCC 리셋을 제품 기능으로 넣지 않는다는 판정은 그대로다(F-11 §3.3·§7).

⚠️ **`AXIsProcessTrusted() == true` 인데 탭 생성이 실패하는 out-of-sync 상황(F-11 §3.3)은 의도적으로 재현하기 어렵다.** 재현되면 진단 화면이 뜨고 **수동 절차만** 안내해야 한다 — ⛔ 자동 TCC 리셋 버튼이 있으면 실패다(명세가 채택하지 않기로 판정했다).

### 1-b. 탭 생존

| # | 조작 | 기대 결과 | 판정 근거 |
| :--- | :--- | :--- | :--- |
| 1 | 앱 실행 후 아무 리매핑도 켜지 않은 채 **평소처럼 하루 사용** | 키 입력이 전혀 지연되거나 씹히지 않는다. 로그에 탭 재생성이 반복적으로 남지 않는다 | §3-a `Active` |
| 2 | 메뉴를 열어 둔 채(메뉴 트래킹 모드) 키를 입력 | 정상 동작한다 — ⭐ 전용 스레드 런루프 + `kCFRunLoopCommonModes` 결정의 검증 지점(§7 근거 1, §9 #9) | `architecture.md` §2.1 |
| 3 | 뚜껑을 닫아 절전 → 열어서 복귀 | 로그: 절전 알림 수신 → **지연(기본 2000 ms) 후** 탭 유효성 재확인. 복귀 직후 리매핑이 동작한다 | §3-a `SuspendedBySleepOrLock`, v1.58 회귀 |
| 4 | 화면 잠금(`⌃⌘Q`) → 잠금 해제 | 동일. ⭐ **직전 재시작으로부터 5초 이내면 디바운스로 재시작을 건너뛴다** — 로그에 그 판정이 남는다 | §3-a 디바운스 |
| 5 | 절전 → 즉시 복귀 → 즉시 다시 절전 → 복귀 (연달아) | 탭이 **반복 재생성되지 않는다.** 디바운스가 동작한 로그가 남는다 | §3-a |
| 6 | 로그아웃 → 로그인 | ⛔ **M1 에서는 검증 불가** — 아래 "실측 결과" 참조 | §5 #4 |
| 7 | 사용자 전환(Fast User Switching) 후 되돌아옴 | 동일 | §5 #5 |
| 8 | **외장 USB 키보드를 연결** | 로그에 키보드 감지가 남고, 지연(기본 1500 ms) 후 경로 B 재적용이 시도된다 | §5 #10 |
| 9 | 외장 키보드 **해제** | 동일하게 감지된다 | §5 #10 |
| 10 | 외장 키보드를 연결한 채 앱 재시작 | 정상 동작 | §5 #10 |

#### ✅ 실제 통과 결과 — #8·#9·#10 (2026-08-30, 이슈 #10 / PR `fix/hotplug-runloop-thread`)

환경: macOS 26.5.2, `./scripts/build-signed.sh` 산출물을 `open --env ULTRAKEY_LOG=debug` 로 실행.
외장 키보드는 **`F108Pro Dongle`(USB 무선 동글)**. 착탈은 사람이 물리적으로 수행했다.

⭐ **두 시작 경로를 각각 따로 검증했다** — 이슈 #10 이 경로에 따라 갈리는 버그였기 때문이다.

| 경로 | 엔진이 시작된 스레드 | #8 연결 | #9 해제 | #10 연결한 채 재시작 |
| :--- | :--- | :---: | :---: | :---: |
| **A** — 권한을 미리 부여하고 실행(`setup()` 에서 시작) | `main` | ✅ | ✅ | ✅ |
| **B** — 실행 중에 권한 부여(권한 전이 콜백에서 시작) | `ultrakey-permission-poll` | ✅ | ✅ | — |

경로 B 실측 로그(수정 전 이 조건에서 감지 로그가 **0건**이었다):

```
INFO ultrakey-hotplug  외장 키보드 해제 감지 — 지연 후 경로 B 재적용을 예약한다 delay_ms=1500
INFO ultrakey-tap      경로 B 재적용을 완료했다(M1: 등록된 규칙 0개)
INFO ultrakey-hotplug  외장 키보드 연결 감지 — 지연 후 경로 B 재적용을 예약한다 delay_ms=1500
INFO ultrakey-tap      경로 B 재적용을 완료했다(M1: 등록된 규칙 0개)
```

**예약(`ultrakey-hotplug`)과 실행(`ultrakey-tap`)이 모두 로그에 남는다** — 지연 1500 ms 뒤
`ReapplyHidMapping` 이 실제로 처리된 것이 확인된다.

⚠️ **착탈 1회에 감지 로그가 2건씩 남는 것은 정상이다.** `F108Pro Dongle` 이 키보드 클래스
HID 인터페이스를 2개 등록해서, 매칭 알림도 장치마다 하나씩 온다. 로그를 볼 때 중복으로
오해하지 마라.

⚠️ **#10 은 경로 A 실행이 겸한다** — 외장 키보드를 연결한 채 앱을 재시작한 것이 곧 경로 A
시나리오다. 재시작 후 `엔진 시작됨 state=Active` 가 남고 착탈이 정상 감지됐다.

#### ⭐ 스레드 친화성 위반은 이제 로그로 드러난다

로그 매 줄에 **스레드 이름**이 찍힌다(`main` / `ultrakey-tap` / `ultrakey-permission-poll` /
`ultrakey-hotplug` / `ultrakey-delay-scheduler` / `ultrakey-watchdog`). 경로 B 에서는 다음
경고가 실제로 관측된다 — 이 신호가 없어서 이슈 #10 이 조용히 죽어 있었다:

```
WARN ultrakey-permission-poll ⚠️ SystemHooks::start 가 메인 스레드가 아닌 곳에서 호출됐다 …
                              thread="ultrakey-permission-poll" thread_id=ThreadId(3)
```

#### ⛔ 이 검증 중에 발견해 함께 고친 것 — 권한 회수 시 시스템 전체 입력 정지

**항목 1-a #5(실행 중 권한 회수 감지)를 수행하다 훨씬 심각한 결함이 드러났다.** 권한을
회수하는 순간 **머신 전체의 키 입력과 마우스 클릭이 죽었다**(커서는 움직였다). 한 번은
강제 재시동까지 필요했다.

⭐ **판별은 대조 실험으로 했다** — Ultrakey 를 종료한 상태에서 같은 토글을 하면 멈추지
않았고, Ultrakey 가 떠 있을 때만 멈췄다. 즉 우리 결함이다.

| 증상 | 왜 그런가 |
| :--- | :--- |
| 커서는 움직인다 | WindowServer 가 커서를 직접 그린다 — 탭을 통과하지 않는다 |
| 키·클릭만 죽는다 | 이 둘은 활성 탭을 **동기적으로** 통과해야 앱에 전달된다 |
| **로그가 한 줄도 안 남는다** | 커맨드 소스(`drain_commands`)가 굶었다 — 런루프가 포화됐다는 뜻 |

원인은 `event_tap.rs` 트램폴린이 탭 비활성화 통지를 받으면 **조건 없이** `CGEventTapEnable(true)`
를 부른 것이다. 권한이 없으면 macOS 가 즉시 다시 끄고 또 통지해서, 재활성화⇄비활성화가
mach 메시지 속도로 무한 반복된다. 게다가 `tap.is_enabled()` 가 방금 켠 직후라 `true` 를
돌려주므로 실패 카운터가 매번 리셋되어 **재생성 에스컬레이션에 영원히 도달하지 못했다.**

수정 후 같은 조작의 실측 로그 — **전 과정 28 ms, 정지 없음**:

```
23:27:27.384 WARN ultrakey-watchdog 워치독: 탭이 비활성 상태로 감지됨 — 재활성화를 요청한다
23:27:27.403 WARN ultrakey-tap      Accessibility 권한이 없는 상태에서 탭 재활성화 요청이 왔다
                                    — 재활성화하지 않고 탭을 놓는다(재활성화 폭주 방지)
23:27:27.412 WARN ultrakey-tap      Accessibility 권한이 확인되지 않아 탭을 만들 수 없다
23:27:27.412 INFO ultrakey-tap      탭 상태 전이 s=NotInstalled
23:27:27.412 INFO main              permissions 창을 찾았다 what="show_modal"
23:27:27.441 INFO main              modal_copy 커맨드 호출됨 kind="onboarding"
```

⭐ **항목 1-a #5 는 이것으로 통과했다** — 권한 회수를 배경 폴링 주기 안에 감지하고 모달을
다시 띄운다. 워치독이 폴링보다 1초 가까이 먼저 탭 비활성화를 잡아낸 것도 함께 확인됐다.

⚠️ **`tccutil reset` 으로는 이 증상이 재현되지 않는다.** 실행 중인 프로세스의 활성 탭을
회수하지 않기 때문이다 — 재현하려면 **시스템 설정에서 토글을 꺼야** 한다. 진단 때 이걸
몰라 한 번 헛짚었으니 기록해 둔다.

⚠️ **검증을 위해 Accessibility 를 켰다 껐다.** 시작 상태와 종료 상태 모두
`Ultrakey.app` 항목이 **있으나 미부여**다. 다른 앱 16개의 항목은 건드리지 않았다.

⚠️ **1-b #4·#5(디바운스)는 이번 회차에서 수행하지 않았다.** 통과했다고 적지 않는다. 다만
그 판정 로그는 이제 `debug` 가 아니라 **`info`** 라 기본 레벨에서 보인다(아래 §0 참고).

### 📌 실측 결과 (2026-08-30, M1 검증)

사용자가 실제 하드웨어에서 수행한 결과다. 근거 로그는 `~/Library/Logs/Ultrakey/ultrakey.log`.

| # | 판정 | 근거 |
| :--- | :---: | :--- |
| 1-b #2 메뉴 트래킹 | ✅ | Ghostty 메뉴를 연 채 caps lock 입력 → 키 이벤트 로그가 그대로 남았다. `kCFRunLoopCommonModes` 결정이 실제로 먹는다 |
| 1-b #3 절전 → 복귀 | ✅ | `절전 복귀 알림 수신 … delay_ms=2000` → `탭 재활성화 성공 attempt=1` |
| 1-b #4 잠금 → 해제 | ✅ | `화면 잠금 해제 알림 수신 … delay_ms=1000` → `탭 재활성화 성공 attempt=1` |
| 1-b #5 디바운스 | 🟡 **간접 확인** | 아래 참조 |
| **1-b #6 로그아웃 → 로그인** | ⛔ **M1 범위 밖** | 아래 참조 |
| 1-b #7 사용자 전환 | ⬜ 미수행 | — |
| 1-b #8~#10 핫플러그 | ✅ | 이슈 #10 / PR #11 에서 경로 A·B 실측 |
| **1-c 워치독** | ✅ | 아래 참조 |
| **1-d 치명적 실패 경로** | ✅ | 아래 참조 |

#### 1-b #5 디바운스 — 간접 확인

직접 판정 로그(`직전 복구로부터 얼마 지나지 않아 재확인을 디바운스로 건너뛴다`)를 눈으로 보지는 못했다.
다만 로그에 **예약은 됐는데 재활성화가 일어나지 않은 구간**이 남았다.

```
22:14:43.265  탭 재활성화 성공 attempt=1
22:14:44.054  절전 복귀 알림 수신 — 지연 후 탭 재확인을 예약한다 delay_ms=2000
              → 22:14:46 에 실행됐어야 하나 `탭 재활성화` 로그가 없다
22:22:36      (다음 로그는 8분 뒤)
```

예약 실행 시점이 직전 재활성화로부터 약 2.7초 — 5초 임계값 안이다. **예약됐는데 재활성화가 없는 것이
곧 디바운스가 건너뛴 모습**이다. 직접 확인은 `open --env ULTRAKEY_LOG=debug <앱>` 으로 띄우고
`⌃⌘Q` 를 5초 안에 두 번 하면 된다(PR #11 이 이 로그를 `info` 로 올렸으므로 이후 빌드에서는 기본 레벨로 보인다).

#### ⛔ 1-b #6 은 M1 에서 원리적으로 통과할 수 없다

이 항목의 기대는 "리매핑이 **수동 개입 없이** 동작한다" 인데, 그것을 성립시키는 기능은
**`Launch on login`(F-10 / General 탭)이고 M2·M3 범위**다. M1 에는 없다.

실측에서도 로그아웃 시 앱이 종료됐고(종료 로그 없음 — macOS 가 그냥 죽인다) 로그인 후 재기동되지 않았다.
한 차례 재기동된 적이 있으나 그것은 macOS 의 "로그인 시 창 다시 열기" 였고 제품 기능이 아니다.

**M1 에서 검증 가능한 범위까지는 통과했다** — 재기동했을 때 카탈로그 로드 → 권한 판정 → 엔진 `Active`
복귀가 정상이다. `Launch on login` 구현 시 이 항목을 다시 판정한다.

#### 📌 재판정 (2026-08-30, M2 2차 / 이슈 #15) — 🟡 **부분 통과. "범위 밖" 은 해소됐다**

⭐ **이 항목을 "M1 범위 밖" 으로 만든 원인이 사라졌다.** `Launch on login`(F-10 / General 탭)이
구현되었고, 실기기에서 다음을 확인했다.

| 확인한 것 | 결과 |
| :--- | :--- |
| 출고 기본값 | **☐**(꺼짐) — §8 수용 기준대로 |
| 체크박스를 켜면 로그인 항목으로 등록되는가 | ✅ `System Events` 의 로그인 항목 목록에 `Ultrakey` 가 나타난다 |
| 껐을 때 해제되는가 | ✅ 목록에서 사라진다 |
| 어느 경로를 탔는가 | ✅ `SMAppService`(macOS 26). `~/Library/LaunchAgents` 에 plist 가 생기지 **않았다** — macOS 12 폴백이 잘못 발동하지 않았다는 뜻이다 |

⬜ **여전히 수행하지 않은 것: 실제 로그아웃 → 로그인 왕복.** 이 검증 회차는 사용자의 세션 안에서
진행됐고, 로그아웃은 사용자의 작업을 끊는 되돌리기 어려운 조작이라 수행하지 않았다.
**따라서 "재부팅 후 수동 개입 없이 리매핑이 동작한다" 는 여전히 미확인이다** — 등록까지만
확인했고 그다음은 확인하지 못했다. 다음 회차에서 사용자가 직접 로그아웃·로그인해 마무리해야 한다.

⚠️ macOS 12 의 LaunchAgent 폴백 경로는 **이 기기에서 확인할 수 없다**(macOS 26.5.2). 미검증으로 남긴다.

⚠️ 부수 관찰: 로그아웃 시 **graceful shutdown 경로가 실행되지 않는다**(`종료 요청` 로그 0건).
M1 은 등록 규칙이 0개라 `hidutil property --get UserKeyMapping` 이 `(null)` 로 깨끗했지만,
**M2 에서 실제 리매핑 규칙이 생기면 종료 정리가 아예 돌지 않는 경로가 된다.** 다음 기동 시 정리하는
설계가 필요한지 F-07·F-15 에서 판단해야 한다.

#### ✅ 1-c 워치독 · 1-d 치명적 실패 경로 — 한 번의 조작으로 함께 통과

시스템 설정에서 Accessibility 를 껐다가 약 35초 뒤 다시 켰다.

```
23:58:45  ultrakey-permission-poll  권한 상태 전이 { from: Granted, to: Denied }
23:58:45  main                      show_modal is_visible=Ok(true)
23:58:45  main                      modal_copy 커맨드 호출됨 kind="onboarding"

23:59:15  ultrakey-tap  WARN  Accessibility 권한이 없는 상태에서 탭 재활성화 요청이 왔다
                              — 재활성화하지 않고 탭을 놓는다
23:59:15  ultrakey-tap  탭 상태 전이 s=Installing
23:59:15  ultrakey-tap  WARN  Accessibility 권한이 확인되지 않아 탭을 만들 수 없다
                              — F-11 온보딩을 기다린다(재시도 루프를 돌지 않는다)
23:59:15  ultrakey-tap  탭 상태 전이 s=NotInstalled
23:59:15  ultrakey-tap  권한 없음 — 온보딩 모달로 넘긴다

23:59:20  ultrakey-permission-poll  권한 상태 전이 { from: Denied, to: Granted }
23:59:20  main                      hide_modal is_visible=Ok(false)
```

- **1-c** — 권한 상실로 탭이 무효화되자 워치독이 재활성화 요청을 받았고, 권한을 확인해 **올바르게 거절**한 뒤
  `NotInstalled` 로 내려갔다. 권한 복구 시 모달이 닫혔다
- **1-d** — 명세 §3-a 의 두 갈래가 로그에서 그대로 구분된다: 권한이 **없어서** 실패 → 종료하지 않고
  `NotInstalled` 로 온보딩 대기, 그리고 **`재시도 루프를 돌지 않는다`**
- ⭐ **PR #11 의 멈춤 수정이 실제로 작동한다.** 이 조작은 수정 전 시스템 전체 입력을 멈추게 했고
  그때는 로그가 한 줄도 남지 않았다. 이번에는 멈춤이 없었고 회수 이후로도 로그가 31줄 계속 흘렀다

---

### 1-c. 워치독 — 탭을 강제로 죽여 복구를 확인한다

가장 확실한 재현: **시스템 설정에서 Accessibility 토글을 껐다 곧바로 다시 켠다.** macOS 가 탭에 `kCGEventTapDisabledByUserInput` 을 보낸다.

기대: 로그에 비활성화 수신 → `CGEventTapEnable` 재활성화 성공이 남고, 리매핑이 계속 동작한다.
재활성화가 5회 연속 실패하면 탭 재생성으로, 재생성이 3회 실패하면 프로세스 재실행 신호로 에스컬레이션된다([`architecture.md`](architecture.md) §4 — **이 임계값들은 실측이 아니라 설계 판단이다**).

### 1-d. ⛔ 치명적 실패 경로 — 재시도가 아니라 종료여야 한다

`key-remapping-engine.md` §3-a 와 §5 #16 이 요구하는 동작이다.

| 상황 | 기대 |
| :--- | :--- |
| Accessibility 가 **없어서** 탭 생성 실패 | ⭕ 종료하지 않는다. `NotInstalled` 로 남아 F-11 온보딩 루프가 권한을 기다린다 |
| Accessibility 가 **있는데도** 탭 생성 실패 | ⛔ **재시도 루프를 돌지 않고 프로세스를 종료한다.** 로그에 치명적 실패가 남는다 |

의도적 재현은 어렵다. **최소한 코드 경로가 두 경우를 구분하는지 확인**하고, 무한 재시도 루프가 없음을 로그로 확인한다.

---

## ⚠️ 항목 2·3·**6** 사전 확인 — 다른 HID 계층 리매퍼가 있는가 (2026-08-30 신설, M2 2차에서 항목 6 으로 확대)

⭐ **이것을 먼저 보지 않으면 검증 결과를 오독한다.** M2 1차 검증에서 실제로 그랬다.

⭐ **M2 2차에서 적용 범위가 넓어졌다** — F-08 프리셋 16종 중 **7종이 caps lock 을 쓰므로**, 이 절은
항목 6(프리셋) 검증에도 **반드시** 선행한다. 프리셋 쪽의 대처 방법은 항목 2·3 과 방향이 다르다 — §6-0 참조.

`CGEventTap`(경로 A)은 **HID 드라이버보다 위**에 있다. 그래서 Karabiner-Elements 처럼 가상 HID 장치로 키를 바꾸는 도구가 깔려 있으면, **Ultrakey 는 원본 키코드를 볼 기회 자체가 없다** — 이미 바뀐 키가 도착한다.

실측(2026-08-30): 검증 기기에 Karabiner-Elements 가 `caps_lock ↔ left_control` 을 맞바꿔 두고 있었다. 그래서 `Remap key to hyper key: caps lock` 을 켜고 물리 caps lock 을 눌러도 hyper 가 발동하지 않았다 — 탭에 도착한 키코드가 `left control` 이었기 때문이다. **Ultrakey 의 결함이 아니라 경로 A 의 구조적 한계다.**

검증 전에 반드시 확인한다:

```sh
# 1) 다른 리매퍼가 도는가
ps aux | grep -iE "[k]arabiner|[b]etterTouch|[h]ammerspoon"
# 2) macOS 자체 modifier 리매핑이 있는가
defaults find modifiermapping
# 3) 커널 매핑(경로 B)이 남아 있는가
hidutil property --get UserKeyMapping
# 4) Karabiner 가 있다면 무엇을 바꾸는가
python3 -c "import json,os;d=json.load(open(os.path.expanduser('~/.config/karabiner/karabiner.json')));[print(p.get('name'),p.get('simple_modifications')) for p in d['profiles'] if p.get('selected')]"
```

**대처**: 리매퍼를 끄거나, **그 리매퍼가 실제로 내보내는 키코드**를 hyper 소스로 지정한다. 위 실측에서는 후자를 택했다 — hyper 소스를 `left control` 로 바꾸니(물리 caps lock 이 Karabiner 를 거쳐 `left control` 로 도착한다) 정상 동작했다. 리매퍼를 끄지 않아도 검증이 성립하고, 사용자 환경을 건드리지 않는다는 것이 이 선택의 근거다.

⛔ 이 조건은 **명세에도 없었고 이 문서에도 없었다.** `key-remapping-engine.md` §5 에 엣지 케이스로 함께 추가했다.

---

## 항목 2 — `caps lock` 을 hyper 로 지정하면 다른 앱이 `⌃⌥⌘⇧` 4개 modifier 를 인식한다

> 근거 명세: `hyperkey.md` §3.1 · §8
> ⭐ **절차 갱신 (2026-08-30, M2 1차 / 이슈 #13).** M1 에서는 이 항목을 수행할 수 없었다 — hyper 를 켜려면 `main.rs` 의 `HyperkeySettings::default()` 를 고쳐 재빌드해야 했기 때문이다(그래서 M1 검증에서 미수행으로 남았다). **F-09 환경설정 창이 생긴 지금은 UI 로 수행한다.**

### 2-0. 관찰 수단 — 왜 키보드 뷰어가 아니라 브라우저 프로브인가

이전 판은 **키보드 뷰어**를 관찰 수단으로 지정했다. 그것도 유효하지만 두 가지 문제가 있다.

1. 키보드 뷰어를 띄우려면 **시스템 설정에서 "메뉴 막대에 입력 메뉴 표시"를 켜야 한다** — 검증을 위해 시스템 설정을 바꾸게 된다(macOS 26 에서는 독립 앱으로 존재하지도 않고 `50onPaletteServer.app` 이 서빙한다).
2. 강조 표시는 스크린샷으로만 남고 **기계가 읽을 수 있는 기록이 남지 않는다.**

그래서 **`docs/dev/tools/modifier-probe.html`** 을 함께 둔다. 이 페이지는 브라우저(= Ultrakey 와 완전히 무관한 다른 앱)가 받은 `KeyboardEvent` 의 `ctrlKey`/`altKey`/`metaKey`/`shiftKey` 와 `getModifierState("CapsLock")` 을 그대로 표시하고 **콘솔에도 한 줄씩 남긴다.** 시스템 설정을 아무것도 바꾸지 않으며, 스크린샷·콘솔 로그 두 가지 증거가 동시에 남는다.

⚠️ 이것이 "다른 앱이 인식한다"를 증명하는 이유: 이 값은 Ultrakey 가 만들어 낸 것이 아니라 **WindowServer 가 브라우저에 전달한 이벤트**에서 읽은 것이다. 합성이 이벤트 스트림에 실제로 반영되지 않았다면 전부 `false` 로 나온다.

> 키보드 뷰어를 쓰고 싶다면 여전히 유효하다 — 입력 메뉴를 켰다면 **검증 후 반드시 되돌리고 그 사실을 PR 에 기록**한다.

### 2-a. hyper — `⌃⌥⌘⇧`

1. `scripts/build-signed.sh` 로 빌드·서명한 `.app` 을 `open` 으로 실행한다. ⛔ `cargo tauri dev` 금지, `.app` 안의 바이너리 직접 실행 금지(TCC 가 부모 프로세스 권한으로 판정된다).
2. **환경설정 창을 연다** — 권한 모달이 떠 있으면 Accessibility 를 먼저 허용한다.
3. `Hyperkey` 탭에서 **hyper 소스 키 체크박스를 켜고** 팝업을 `caps lock` 으로 둔다. shift 포함 토글은 **켠 상태**로 둔다(출고 기본값).
   ⭐ 재빌드도, 앱 재시작도 하지 않는다 — 설정 변경은 `ArcSwap` 교체로 **즉시** 엔진에 반영된다.
4. 브라우저에서 `docs/dev/tools/modifier-probe.html` 을 열고 페이지를 클릭해 포커스를 준다.
5. caps lock 을 **누른 채** 아무 글자 키(예: `A`)를 누른다.

| 확인 | 기대 |
| :--- | :--- |
| 프로브의 modifier 표시 | ⭐ **`⌃` `⌥` `⌘` `⇧` 4개 전부 파랗게 켜진다.** 판정 줄에 `✅ hyper — ⌃⌥⌘⇧ 4개 전부 인식됨` |
| 프로브 표의 그 행 | `ctrl/alt/meta/shift` 열이 전부 `T` |
| caps lock 의 실제 잠금 | ⇪ 칸이 **꺼져 있다**(`caps=F`) — 대문자 고정도, LED 점등도 없다. 원본 keyDown 이 소비되기 때문이다 |
| caps lock 을 뗀다 | 다음 키 입력에서 4개가 **전부 해제**된다 |
| 다른 앱의 단축키 | 물리적으로 `⌃⌥⌘⇧` 를 눌러 등록해 둔 단축키가 `caps lock + <키>` 로 정상 트리거된다 |

증거로 남길 것: 프로브 화면 **스크린샷** + 브라우저 콘솔의 `[probe] keydown … combo=⌃⌥⌘⇧` 줄.

**실전 확인 방법 하나** — 시스템 설정 → 키보드 → 키보드 단축키에서 아무 항목의 단축키를 물리 `⌃⌥⌘⇧ + <키>` 로 등록한 뒤, `caps lock + <키>` 로 발동하는지 본다.

⚠️ **알려진 한계 — 버그가 아니다.** Keyboard Maestro 류 shortcut recorder 에 hyper 소스 키로 단축키를 **등록(recording)** 하는 것은 실패할 수 있다. 그러나 물리 modifier 4개로 등록해 둔 단축키의 **트리거는 정상이어야 한다** — 이 비대칭이 명세(`hyperkey.md` §3.4, §8)가 요구하는 동작이다.

### ✅ 실측 결과 (2026-08-30, M2 1차 / 이슈 #13) — **통과**

사전 확인(위)에 따라 hyper 소스를 `left control` 로 지정하고(물리 caps lock 이 Karabiner 를 거쳐 그 키코드로 도착한다), `Include shift` 는 켠 채로 물리 caps lock 을 누른 상태에서 `A` 를 눌렀다. 관찰 수단은 `docs/dev/tools/modifier-probe.html`(Chrome).

| # | type | key | code | ⌃ | ⌥ | ⌘ | ⇧ | ⇪ | combo |
| ---: | :--- | :--- | :--- | :-: | :-: | :-: | :-: | :-: | :--- |
| 95 | keydown | Control | ControlLeft | T | T | T | T | F | `⌃⌥⌘⇧` |
| 96 | keydown | A | KeyA | T | T | T | T | F | `⌃⌥⌘⇧` |
| 97 | keyup | Control | ControlLeft | F | F | F | F | F | — |

- ⭐ **`KeyA` 의 keydown 이 `⌃⌥⌘⇧` 4개를 전부 싣고 도착했다.** 이 값은 Ultrakey 가 만든 것이 아니라 **WindowServer 가 브라우저에 전달한 이벤트**에서 읽은 것이므로, "다른 앱이 인식한다"가 그대로 뒷받침된다.
- ⭐ **caps lock 잠금이 걸리지 않았다**(⇪ 열 전부 `F`, 글자도 대문자 고정이 아니다) — 원본 keyDown 이 소비된다는 뜻이다.
- 소스 키를 떼자 다음 이벤트에서 4개가 전부 해제됐다.
- 증거: [`screenshots/issue-13-probe-hyper-4-modifiers.png`](screenshots/issue-13-probe-hyper-4-modifiers.png)

⛔ **이 항목을 처음 수행하면서 M1 의 엔진 결함 하나를 찾아 고쳤다.** macOS 는 modifier 키의 누름/뗌을 `KeyDown`/`KeyUp` 이 아니라 **`FlagsChanged` 하나로만** 보내는데, 중재기가 `FlagsChanged` 를 `_ => None` 으로 흘려보내고 있었다. 소스 키 35종 중 F1~F24 를 뺀 전부가 modifier 키이므로 **hyper/meh/bleh 는 어떤 소스 키로도 발동한 적이 없었다.** M1 의 자동 테스트가 전부 `KeyDown`/`KeyUp` 으로만 이벤트를 만들어 이 경로를 건드리지 않았고, 이 항목이 환경설정 UI 부재로 **한 번도 수행되지 않아** 드러나지 않았다. 수정 전 상태의 증거: [`screenshots/issue-13-probe-before-fix.png`](screenshots/issue-13-probe-before-fix.png) — 원본 `CapsLock` keydown 이 그대로 통과하고 잠금까지 걸렸다. **실기기 검증을 완료 조건에 두는 이유가 이것이다.**

⬜ **2-b(meh · bleh)와 2-c(마우스 이벤트)는 이번에 수행하지 않았다.** 같은 코드 경로(`handle_modifier_source_event`)를 타므로 회귀 테스트로는 덮여 있으나, 실기기 확인은 남아 있다.

### ⛔ caps lock 을 소스로 쓸 때의 알려진 한계 (2026-08-30 실측)

⭐ **caps lock 은 래칭 키라, 지금 구현(경로 A)에서는 hyper 가 홀드가 아니라 토글로 동작한다.**

실측 절차 — hyper 소스를 caps lock 으로 두고, 소스 키를 **톡 눌렀다 떼기**를 3회 반복하며 사이사이 `a` 를 눌렀다.

| 순서 | 관찰 |
| :--- | :--- |
| 1번째 누름 → `a` | `a` 가 `⌃⌥⌘` 를 달고 나갔다 (hyper **켜짐**) |
| 2번째 누름 → `a` | `a` 에 modifier 가 **없다** (hyper **꺼짐**) |
| 3번째 누름 → `a` | 다시 `⌃⌥⌘` 를 달고 나갔다 (hyper **켜짐**) |

증거: [`screenshots/issue-13-capslock-latching.png`](screenshots/issue-13-capslock-latching.png)

**원인**: caps lock 은 누를 때만 `flagsChanged` 를 보내고 뗄 때는 보내지 않는다. 그래서 `key-remapping-engine.md` §5 #18 의 down/up 환원이 "1번째 = 누름, 2번째 = 뗌"으로 해석한다. ⛔ **경로 A 만으로는 고칠 수 없다** — 오지 않는 이벤트를 만들어낼 방법이 없다.

**해법(예정)**: 경로 B 로 caps lock 을 사용되지 않는 모멘터리 키(`F18` 등)에 커널 매핑하고 hyper 규칙을 그 키에 건다. 경로 B 규칙 배정은 **F-08(M2 2차)** 소관이라 그때 함께 구현한다 — `key-remapping-engine.md` §5 #20.

**그때까지의 우회**: caps lock 이 아닌 **모멘터리 소스 키**를 쓰면 정상 동작한다 — `right command` · `right option` · `right control` · `F13` 등. 이 문서의 항목 2·3 실측도 모멘터리 키(`left control`)로 통과시킨 것이다.

⭐ 함께 고친 것: 합성 이벤트에 caps lock 의 **잠금 비트(`alphaShift`)가 따라붙어** 다른 앱이 caps lock 켜짐으로 인식하던 문제는 해소했다(§5 #21). 수정 후 프로브의 `⇪` 열이 전 행 `F` 다. ⚠️ 이때 하드웨어 잠금은 애초에 걸리지 않았다(`ioreg` 의 `HIDCapsLockState` = `No`) — 이벤트 flags 층위만의 문제였다.

### 2-b. meh · bleh

`Hyperkey` 탭에서 meh·bleh 체크박스를 켜고 **서로 다른 소스 키**를 고른다(같은 키를 고르면 hyper 가 이긴다 — UI 가 경고를 띄운다).

| 조합 | 프로브 표시 | 특히 확인할 것 |
| :--- | :--- | :--- |
| meh | `⌃` `⌥` `⇧` 3개 | ⛔ **`⌘` 가 켜지면 실패** |
| bleh | `⌃` `⌘` `⇧` 3개 | ⛔ **`⌥` 가 켜지면 실패** — v1.65 회귀 방지 대상(`hyperkey.md` §8) |

### 2-c. `Apply modifiers to keypress events and:`

출고 기본은 **`Click` 만 ON** 이다(실측, `hyperkey.md` §3.3).

| 조작 | 기대 |
| :--- | :--- |
| hyper 를 누른 채 클릭 | modifier 가 실린 클릭으로 전달된다 |
| hyper 를 누른 채 드래그 | `Drag` 가 꺼져 있으므로 **modifier 가 실리지 않는다** |
| `Scroll` 을 켜고 Safari 에서 hyper + 스크롤 | 확대/축소가 발동한다 — 이것이 명세가 이 항목을 기본 OFF 로 두는 이유다(§3.3) |

---

## 항목 3 — shift 포함 토글을 끄면 `⌃⌥⌘` 3개만 합성된다

> ⭐ **절차 갱신 (2026-08-30, M2 1차).** 항목 2 와 같은 이유로 이제 **UI 로** 수행한다.

1. 항목 2-a 의 상태(hyper = caps lock, shift 포함 ☑)에서 시작한다.
2. `Hyperkey` 탭의 **shift 포함 토글을 끈다.** ⭐ 앱을 재시작하지 않는다.
3. 프로브 페이지로 돌아가 caps lock 을 누른 채 `A` 를 누른다.

| 확인 | 기대 |
| :--- | :--- |
| 프로브 표시 | ⭐ **`⌃` `⌥` `⌘` 3개만.** `⇧` 는 켜지지 않는다. 판정 줄에 `✅ hyper (shift 제외)` |
| 저장 형태 | 단일 비트마스크의 shift 비트가 꺼진 것이지 별도 불리언 필드가 아니다(`hyperkey.md` §3.1 — `hyperFlags` 실측) |
| 저장 파일 | `~/Library/Application Support/app.ultrakey.Ultrakey/settings.json` 의 `values` 에 그 키가 **즉시** 생긴다(F-15 §3.1.1 결정 1) |
| meh · bleh | ⭐ **영향받지 않는다.** 이 토글은 hyper 에만 적용된다(§3.1) — meh/bleh 의 `⇧` 는 여전히 켜져야 한다 |

### ✅ 실측 결과 (2026-08-30, M2 1차 / 이슈 #13) — **통과**

항목 2 의 상태에서 환경설정 창의 shift 포함 토글을 껐다. **앱을 재시작하지 않았다.**

| # | type | key | code | ⌃ | ⌥ | ⌘ | ⇧ | combo |
| ---: | :--- | :--- | :--- | :-: | :-: | :-: | :-: | :--- |
| 38 | keydown | a | KeyA | T | T | T | **F** | `⌃⌥⌘` |
| 39 | keyup | a | KeyA | T | T | T | **F** | `⌃⌥⌘` |

- ⭐ **`⇧` 만 정확히 빠졌다.** 글자도 소문자 `a` 로 도착했다(shift 가 실제로 실리지 않았다는 독립적 방증).
- 창의 조합 미리보기가 `⌃⌥⌘⇧` → `⌃⌥⌘` 로 **즉시** 바뀌었고, 같은 순간 저장 파일에 `hyperkey.includeShiftInHyper: false` 가 생겼다.
- 증거: [`screenshots/issue-13-probe-hyper-no-shift.png`](screenshots/issue-13-probe-hyper-no-shift.png)

⭐ **`hyperkey.md` §9 #5 가 남긴 미해결 질문이 부분적으로 해소됐다.** "hyper 가 Active 인 도중 이 토글을 바꾸면 어떻게 되는가"에 대해 관찰된 것은: **토글은 즉시 엔진에 반영되고(로그에 `설정 변경을 반영해 Arbiter 를 재구성했다`), 그와 동시에 상태가 강제 리셋된다.** 따라서 "누른 채로 바꾸는" 경우 자체가 성립하지 않고 다음 누름부터 새 조합이 적용된다. 다만 **소스 키를 물리적으로 누른 채 토글을 바꾸는 상황은 재현하지 않았다**(환경설정 창을 클릭하려면 키를 놓아야 한다) — 그 경우는 여전히 `(미확정)`이다.

⬜ **meh · bleh 가 영향받지 않는지는 이번에 확인하지 않았다** — 두 슬롯을 켜지 않은 채로 검증했다. 단위 테스트는 `hyper_flags()`/`meh_flags()`/`bleh_flags()` 가 독립임을 덮는다.

---

## 항목 4 — QWERTZ·AZERTY·Dvorak·한글 입력기에서 동일하게 동작한다

> 근거 명세: `localization-and-input-sources.md` §3.2 · §8 (B) · `key-remapping-engine.md` §3-e

### 준비

시스템 설정 → 키보드 → 입력 소스에서 다음을 추가한다: **독일어(QWERTZ)** · **프랑스어(AZERTY)** · **Dvorak** · **한국어(2벌식)**.

### 각 입력 소스에서 반복

| # | 확인 | 기대 | 왜 |
| :--- | :--- | :--- | :--- |
| 1 | caps lock 을 hyper 로 눌렀을 때 | **모든 입력 소스에서 동일하게** `⌃⌥⌘⇧` 가 합성된다 | 판정이 물리 키코드 기준이라 레이아웃과 무관해야 한다(§3.2.1) |
| 2 | 입력 소스를 **전환하는 순간** | 로그에 `kTISNotifySelectedKeyboardInputSourceChanged` 수신과 캐시 재구축이 남는다 | §3.2.5, §8 (B) |
| 3 | **한글 입력기로 전환** | 로그에 ⭐ **`used_ascii_fallback = true`** 가 남는다 — `TISCopyCurrentASCIICapableKeyboardLayoutInputSource` 폴백이 발동했다는 뜻 | §3.2.6, `key-remapping-engine.md` §3-e |
| 4 | 한글 조합 중(예: "ㄱ" 입력 후 대기)에 hyper 를 누른다 | ⭐ **조합이 깨지지 않는다** | §3.2.6 — 명세가 실측 검증 필요로 남긴 지점 |
| 5 | 일본어 IME 변환 후보 표시 중 hyper | 변환이 취소되거나 엉뚱한 후보가 확정되지 않는다 | §5 항목 7 |

⚠️ **M1 의 한계를 정확히 이해할 것.** M1 에는 **문자를 출력하는 리매핑이 아직 없다**(quick press 괄호·슬래시·`symbol row` 는 전부 F-08/M2). 따라서 항목 4 에서 M1 이 실제로 검증하는 것은:
- ✅ **판정**이 레이아웃 독립적인가 (hyper 가 모든 레이아웃에서 동일하게 뜨는가)
- ✅ **레이아웃 테이블이 올바르게 구축·무효화되는가** (로그와 단위 테스트)
- ❌ **문자 출력**의 레이아웃 독립성은 M1 에서 검증할 수 없다 — 출력하는 규칙 자체가 없다. M2 로 넘어간다

이 구분을 흐리지 마라. "M1 에서 항목 4 를 통과했다"는 **판정 경로에 한정된 진술**이다.

---

## 항목 5 — Secure Input 구간에서 경로 A 가 무력화되어도 앱이 죽거나 stuck modifier 를 남기지 않는다

> 근거 명세: `key-remapping-engine.md` §5 #6, #9 · §8 · `hyperkey.md` §5 항목 6

### 5-a. Secure Input 구간 진입

1. `ioreg -l -w 0 | grep -i SecureInput` 로 현재 상태를 확인한다(보통 비어 있다).
2. **암호 입력 필드에 포커스를 준다** — 예: 시스템 설정의 잠금 해제 대화상자, `sudo` 를 기다리는 터미널, 로그인 항목의 암호 필드, 1Password 류의 잠금 해제 화면.
3. 다시 `ioreg` 로 확인하면 이제 Secure Input 을 켠 프로세스가 보인다.

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | 그 상태에서 caps lock(hyper 소스)을 누른다 | ⭕ **hyper 가 합성되지 않는다.** 이것은 macOS 의 의도된 보안 제약이며 우회하지 않는 것이 정답이다 |
| 2 | 앱 상태 | ⛔ **크래시하지 않는다.** 로그에 반복 오류가 쏟아지지 않는다 |
| 3 | 암호 필드에 실제로 타이핑 | 입력이 그대로 들어간다 — 원본 이벤트가 온전히 통과해야 한다 |

### 5-b. ⭐ stuck modifier — 가장 중요한 확인

`key-remapping-engine.md` §5 #9 가 정의하는 실패 모드다. **hyper 소스 키를 누른 채로 keyUp 을 영영 받지 못하는 상황**을 의도적으로 만든다.

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | caps lock(hyper)을 **누른 채** 암호 필드로 포커스 이동 → 놓는다 | 이후 일반 입력에 `⌃⌥⌘⇧` 가 계속 얹히지 **않는다**. 키보드 뷰어에 강조가 남아 있으면 **실패** |
| 2 | caps lock 을 **누른 채** 뚜껑을 닫아 절전 → 복귀 | 동일. 절전 진입 알림 수신 시 모든 상태가 `Idle` 로 강제 리셋되고 modifier off 가 방출되어야 한다 |
| 3 | caps lock 을 **누른 채** 화면 잠금 → 해제 | 동일 |
| 4 | caps lock 을 **누른 채** `⌘Tab` 으로 앱 전환 | ⭕ 물리적으로 키가 계속 눌려 있으므로 **modifier 는 유지되는 것이 정상**이다(§5 #8). 이것은 stuck 이 아니다 — 키를 떼면 해제되어야 한다 |

**복구되지 않는 stuck 을 만났다면**: 어떤 조작에서 발생했는지, `ioreg` 의 Secure Input 상태, 로그의 마지막 상태 전이를 함께 기록해 `key-remapping-engine.md` §5 에 새 실패 모드로 추가한다.

### 5-c. 경로 B/C 는 Secure Input 에 영향받지 않는가 — ⭐ 미확정, 검증 기회

`key-remapping-engine.md` §5 #6 과 §9 #8 이 `(미확정)` 으로 남긴 질문이다. 경로 B/C 는 커널 층에서 동작하므로 Secure Input 이 걸러내지 못할 **가능성**이 있으나 **명세가 단정하지 않았다.**

⚠️ **M1 에서는 확인할 수 없다** — 경로 B 로 배정된 규칙이 아직 0 개이고(§3-d, 배정은 F-08/M2), 경로 C 를 쓰는 규칙도 M2 다. **M2 에서 확인하고 그 결과를 명세 §9 #8 에 기록한다.**

---

## 부록 A — 경로 B(IOHID 커널 매핑) 잔존 정리 확인

> 근거 명세: `key-remapping-engine.md` §3-a2 · §5 #15 · §8

M1 에는 경로 B 규칙이 없지만 **인프라는 전부 들어가 있다.** 다음을 확인한다.

```sh
# 앱을 켠 상태와 끈 상태에서 각각
hidutil property --get UserKeyMapping
```

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | 앱 정상 종료 후 | Ultrakey 가 설치한 매핑이 남아 있지 않다 |
| 2 | 앱을 `kill -9` 로 강제 종료한 뒤 재실행 | 시작 시 잔존 매핑을 감지해 현재 설정과 재조정한 로그가 남는다 |
| 3 | Ultrakey 를 쓰기 **전부터** 사용자가 `hidutil` 로 직접 설정해 둔 매핑이 있는 경우 | ⛔ **그것을 지우지 않는다.** 남의 매핑을 치우는 것은 이 앱의 권한 밖이다 |

⚠️ 3번은 명세가 명시적으로 다루지 않은 지점이다(관련 키 `keepExistingIohid` 가 실측으로 존재하나 의미는 `(미확정)`, §3-a2). 관찰 결과를 명세에 기록한다.

## 부록 B — F-10 앱별 비활성화 게이트 (인터페이스만, UI 는 M3)

M1 에는 **메뉴바 UI 가 없다.** 게이트가 F-07 콜백의 계층 0 에서 읽히는 자리만 확정되어 있다([`architecture.md`](architecture.md) §2.3).

검증 가능한 것: 단위 테스트(`ultrakey-core` `gate`)와, 최전면 앱 전환 시 로그에 `frontAppId`/`frontAppName` 갱신이 남는지.
검증 불가능한 것: `Ignore <앱이름>` 을 실제로 켜고 끄는 것 — **M3 에서 UI 와 함께 검증한다.**

---

## 항목 6 — F-08 Power User Presets (M2 2차 / 이슈 #15 신설)

> 근거 명세: `../spec/power-user-presets.md` §8 · `architecture.md` §6

### ⚠️ 6-0. 프리셋 전용 사전 확인 — Karabiner 등 HID 계층 리매퍼

⭐ **위 "항목 2·3 사전 확인" 절을 프리셋에도 그대로 적용한다.** 오히려 여기가 더 중요하다 —
**16종 중 7종이 caps lock 을 쓰는데**(F-08.1·2·4·5·6·7·10), 검증 기기의 Karabiner-Elements 가
`caps_lock ↔ left_control` 을 맞바꿔 두고 있어 **물리 caps lock 을 눌러도 탭에는 `left control`
이 도착한다.** 사전 확인 명령 4개를 먼저 돌린다(위 절).

**프리셋 검증에서의 대처 — 항목 2·3 과 반대 방향이다.**

| 상황 | 항목 2·3(hyper) | 항목 6(프리셋) |
| :--- | :--- | :--- |
| 무엇을 바꾸는가 | **설정**을 바꾼다 — hyper 소스를 `left control` 로 지정 | ⛔ **바꿀 수 없다.** 프리셋의 소스 키는 caps lock 으로 **고정**이다(팝업이 없다) |
| 그래서 어떻게 하는가 | 물리 caps lock 을 그대로 누른다 | **물리 `left control` 을 누른다** — Karabiner 가 그것을 `caps lock` 키코드로 바꿔 보내므로, 탭에는 프리셋이 기다리는 caps lock 이 도착한다 |

즉 이 기기에서 `Caps lock + W A S D` 를 검증하려면 **물리 왼쪽 control 을 누른 채 `W`** 를 누른다.
⛔ **사용자의 Karabiner 설정을 끄거나 바꾸지 않는다** — 그럴 필요가 없다는 것이 이 방법의 요점이다.

⭐ **D-1(caps lock 모멘터리 정규화, `architecture.md` §6.1)과의 상호작용도 여기서 관찰한다.**
Ultrakey 가 설치하는 경로 B 매핑(`caps lock → F18`)은 `hidutil` 이므로 Karabiner 의 가상 HID
키보드에도 적용된다 — 따라서 물리 `left control` → (Karabiner) `caps lock` → (hidutil) `F18` 로
도착할 것으로 **예상**된다. 이것이 실제로 그런지는 아래 6-1 에서 확인한다.

```sh
# 프리셋을 켠 뒤 — Ultrakey 가 설치한 매핑이 보여야 한다
hidutil property --get UserKeyMapping
# 프리셋을 전부 끈 뒤 — (null) 로 돌아와야 한다
```

### 6-1. 대표 프리셋 — `Caps lock + W A S D = ▲ ◀︎ ▼ ▶︎` (F-08.5)

관찰 수단은 항목 2 와 같은 [`tools/modifier-probe.html`](tools/modifier-probe.html)(Chrome)이다 —
**다른 앱이 실제로 받은 이벤트**를 보여준다.

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | `Presets` 탭에서 `Caps lock + W A S D` 만 켠다 | `hidutil property --get UserKeyMapping` 에 `caps lock → F18` 매핑이 나타난다(D-1) |
| 2 | 프로브에서 caps lock 소스 키(이 기기에서는 물리 왼쪽 control)를 **누른 채** `W` | `ArrowUp` 이 도착한다. `w` 도, `Control+w` 도 아니다 |
| 3 | 같은 방식으로 `A`·`S`·`D` | `ArrowLeft`·`ArrowDown`·`ArrowRight` |
| 4 | 소스 키를 뗀 뒤 `W` | 평범한 `w` 가 도착한다(조합이 풀렸다) |
| 5 | ⭐ 소스 키를 **누른 채 유지**하고 `W` 를 세 번 | 세 번 다 `ArrowUp`. **한 번씩 걸러 나오면 D-1 이 동작하지 않은 것이다**(래칭, §5 #20) |
| 6 | 프리셋을 끄고 `hidutil property --get UserKeyMapping` | `(null)` — 매핑이 정리된다 |

### 6-2. `Double tap shift = caps lock` (F-08.8) — 경로 C

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | 프리셋을 켠 뒤 같은 shift 키를 300ms 안에 두 번 탭 | 키보드의 caps lock **LED 가 켜지고** 이후 타이핑이 대문자로 나간다 |
| 2 | `ioreg -c AppleHIDKeyboardEventDriverV2 \| grep -i HIDCapsLockState` 또는 `hidutil` 로 잠금 상태 확인 | `Yes` — **이벤트 flags 가 아니라 실제 HID 잠금 상태**가 바뀌어야 한다(경로 C) |
| 3 | 한 번 더 두 번 탭 | 다시 꺼진다 |
| 4 | shift 를 **한 번만** 탭 | 아무 일도 없다(double tap 간격 300ms 초과 시 유예된 quick press 도 없어야 한다 — F-08.11 이 꺼져 있다면) |

### 6-3. quick press 계열 — `Quick press left or right shift to input corresponding:` (F-08.11)

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | 프리셋을 켜고 팝업을 `( )` 로 둔 뒤, **왼쪽** shift 를 톡 눌렀다 뗀다 | `(` 가 입력된다 |
| 2 | **오른쪽** shift 를 톡 눌렀다 뗀다 | `)` 가 입력된다 |
| 3 | shift 를 누른 채 다른 글자를 친다 | 평범한 대문자가 나가고 괄호는 나오지 않는다(P2 — 다른 키가 눌리는 순간 quick press 후보에서 빠진다) |
| 4 | ⭐ 입력 소스를 한글(2벌식)로 바꾸고 1·2 반복 | **같은 괄호 문자**가 나온다(R5 / v1.51 회귀 방지 — 출력은 유니코드 문자로 고정) |

### 6-4. ⭐ 회귀 방지 2종 — v1.20 · v1.62

이것이 **이 검증에서 가장 중요한 두 항목**이다. 원본이 실제로 겪은 회귀다.

| # | 구성 | 조작 | 기대 |
| :--- | :--- | :--- | :--- |
| v1.20 | `Hyperkey` 탭에서 hyper 소스 = caps lock **그리고** `Caps lock + W A S D` ☑ | 소스 키를 누른 채 `W` | `ArrowUp` 이 나온다. ⭐ **`⌃⌥⌘⇧` 가 얹히지 않는다**(`architecture.md` §6.4 P5). 프로브의 modifier 열이 전부 `F` 여야 한다 |
| v1.62 | `Shift + caps lock = caps lock` ☑ **그리고** `Quick press caps lock to execute: caps lock` ☑ | shift 를 누른 채 caps lock 소스 키를 톡 눌렀다 뗀다 | caps lock 잠금이 **정확히 한 번** 토글된다. quick press 출력이 겹쳐 나와 두 번 토글되면 실패다 |

### 6-5. 충돌 감지 대화상자 3종 (`architecture.md` §6.5)

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | hyper 소스를 caps lock 으로 둔 채 `Remap caps lock to:` 를 켠다 | `CapsLockAlreadyRemapped` 대화상자. `계속` 을 누르면 hyper 소스 쪽이 꺼진다. `취소` 를 누르면 체크박스가 원래대로 돌아온다 |
| 2 | `Caps lock + W A S D` 를 켠 상태에서 `Caps lock + [H J K L]` 을 켠다 | `CapsLockArrows` 대화상자. `계속` 시 WASD 가 꺼진다 |
| 3 | 위 둘 중 하나가 켜진 상태에서 `Caps lock + home row` 를 켠다 | `CapsLockHomeRow` 대화상자 |
| 4 | ⭐ hyper 소스 = caps lock 인 상태에서 `Caps lock + W A S D` 를 켠다 | **대화상자가 뜨지 않는다.** 두 구성은 동시에 성립해야 한다(R2 수용 기준) |

### 6-6. Secure Input (F-08 §8 마지막 항목)

암호 필드(예: 시스템 설정의 잠금 해제 창)에 포커스를 준 채 프리셋을 발동시킨다.

| 경로 | 기대 |
| :--- | :--- |
| 경로 A 프리셋 15종 | 발화하지 않는다. 원본 키가 그대로 그 필드에 도달한다 |
| ⭐ 경로 B(D-1 의 `caps lock → F18` 커널 매핑) | **`(미확정)`** — `power-user-presets.md` §5 엣지 7 / §9 #10 이 남긴 질문이다. **관찰 결과를 그대로 기록하고, 관찰하지 못했으면 못했다고 적는다** |

---

## 항목 7 — F-10 메뉴바 · 수명주기 (M2 2차 / 이슈 #15 신설)

> 근거 명세: `../spec/menu-bar-and-lifecycle.md` §8 · `architecture.md` §6.7

### 7-a. 메뉴바 상주와 메뉴 구조

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | 앱 실행 | Dock 아이콘이 없고 `⌘Tab` 목록에도 없다. 메뉴바에 아이콘이 뜬다 |
| 2 | 아이콘 클릭 | `Ignore <최전면 앱>` · 구분선 · `Settings…` · `About` · `Advanced ▸` · `Quit Ultrakey` |
| 3 | 다른 앱을 최전면으로 바꾸고 메뉴를 다시 연다 | `Ignore …` 라벨이 **그 앱 이름으로 갱신**된다 |
| 4 | `Settings…` | 환경설정 창이 열린다 |
| 5 | `Advanced ▸` | `Synthesize Caps Lock Remap` · `Relaunch` 두 항목 |

### 7-b. 앱별 비활성화

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | 프리셋 하나를 켜고 그것이 동작함을 확인한다 | 동작한다 |
| 2 | 그 앱이 최전면인 상태에서 `Ignore <그 앱>` 클릭 | 항목에 체크가 들어간다 |
| 3 | 같은 프리셋을 다시 시도 | ⭐ **발화하지 않는다** — 원본 키가 그대로 나간다(계층 0 문지기) |
| 4 | 다른 앱으로 전환해 같은 프리셋 | 다시 발화한다 |
| 5 | `Ignore …` 를 다시 클릭 | 체크가 풀리고 1번 상태로 돌아온다 |
| 6 | 앱을 재시작 | 무시 목록이 **유지된다**(`general.disabledApps` 즉시 write-through) |

### 7-c. `unauthorizedMenu`

시스템 설정에서 Accessibility 권한을 껐다가 다시 켠다(⚠️ 껐다면 반드시 되돌리고 PR 에 기록한다).

| # | 상태 | 기대 |
| :--- | :--- | :--- |
| 1 | 권한 회수 직후 메뉴를 연다 | 메뉴 전체가 **2항목**(상태 안내 + 권한 허용)으로 교체된다 |
| 2 | 권한을 다시 부여 | 메뉴가 정상 구성으로 되돌아온다 |

### 7-d. ⭐ `Launch on login` — 1-b #6 을 다시 판정하는 항목

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | `General` 탭의 `Launch on login` 출고 기본값 | **☐**(꺼짐) — §8 수용 기준 |
| 2 | 체크박스를 켠다 | `~/Library/Application Support/.../settings.json` 에 `general.launchOnLogin: true` 가 **즉시** 나타난다 |
| 3 | 시스템 설정 ▸ 일반 ▸ 로그인 항목 | Ultrakey 가 목록에 나타난다 |
| 4 | ⭐ **로그아웃 → 로그인** | 앱이 **자동으로 기동**하고, 수동 개입 없이 리매핑이 동작한다 → **이것이 항목 1-b #6 의 재판정이다** |
| 5 | 체크박스를 끄고 3 을 다시 본다 | 목록에서 사라진다 |

⚠️ **macOS 12 경로(LaunchAgent plist 폴백)는 이 기기에서 확인할 수 없다** — 검증 기기는 macOS 26 이다.
`SMAppService` 경로만 실측하고, macOS 12 경로는 **미검증으로 남긴다.**

### 7-e. 단일 인스턴스

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | 앱이 뜬 상태에서 Finder 로 다시 실행 | 두 번째 프로세스가 즉시 종료된다. `ps aux \| grep Ultrakey` 에 하나만 남는다 |
| 2 | 키를 눌러 본다 | 이벤트가 **두 번 처리되지 않는다** |

---

## 📌 실측 결과 (2026-08-30, M2 2차 / 이슈 #15)

### ⚠️ 관찰 방법과 그 한계 — 먼저 읽어라

이 회차는 **사람이 물리 키를 누르는 대신** `crates/ultrakey-platform/examples/key_poke.rs` 로
키 이벤트를 합성해 세션에 넣었다. 그 도구는 자기 합성 마커(`ULTRAKEY_MAGIC`)를 **심지 않으므로**
Ultrakey 의 탭은 그것을 물리 입력과 구분하지 않는다 — 즉 **경로 A(탭 → 중재 → 합성 → 대상 앱)
전 구간이 실제 앱을 상대로 검증됐다.** modifier 키는 `fc:`(flagsChanged) 형태로 넣어
`key-remapping-engine.md` §5 #18 이 말하는 **실제 이벤트 형태**를 재현했다.

⛔ **그러나 이 방법으로 검증되지 않는 것이 있다. 통과했다고 적지 않는다.**

| 검증되지 않은 것 | 왜 |
| :--- | :--- |
| **경로 B 커널 매핑이 실제 물리 키에 적용되는가** | 합성 이벤트는 HID 층보다 위에서 주입되므로 `hidutil` 매핑을 지나지 않는다. 이 회차는 `caps lock → F18` 매핑이 **설치·정리되는 것**만 확인했고, 물리 caps lock 이 실제로 F18 로 도착하는지는 확인하지 못했다 |
| **caps lock 래칭이 실제로 해소되는가**(§5 #20) | 위와 같은 이유. D-1 의 핵심 효과가 바로 이것인데 **미확인이다** |
| 검증 기기 특유의 조건 | Karabiner-Elements 가 `caps_lock ↔ left_control` 을 **경로 B 보다 아래**에서 맞바꾸고 있어, 물리 caps lock 검증 자체가 이 기기에서는 성립하지 않는다(§6-0) |

⭐ 대신 **`F18` 을 직접 넣어** "커널 매핑이 이미 caps lock 을 F18 로 바꿔 보낸 뒤" 의 상태를
재현했다 — 그것이 D-1 이 만들려는 상태다. 아래 프리셋 결과는 전부 그 전제 위에 있다.

### ✅ 통과한 것

| 항목 | 결과 · 증거 |
| :--- | :--- |
| **D-1 경로 B 설치·정리** | `Caps lock + W A S D` 를 켜자 `hidutil property --get UserKeyMapping` 에 `Src=30064771129`(caps lock) → `Dst=30064771181`(F18) 가 나타났고, 앱을 정상 종료하자 비워졌다. 설정 파일에는 건드린 키 **하나만** 쓰였다("부재 = 기본값" 유지) |
| **6-1 F-08.5 `Caps lock + W A S D`** | 3행 문서의 3행 끝에서 `F18 + W` 를 두 번 → 마커가 **1행**에 들어갔다(`1111`). ▲ 가 실제로 두 번 합성됐다는 뜻이다. 소스 키를 뗀 뒤의 `W` 는 평범한 문자로 들어갔다 |
| **F-08.4 `Caps lock + space = enter`** | `AB` 뒤에서 `F18 + Space` → 문서가 `AB\n1` 이 됐다. space 가 소비되고 Enter 가 합성됐다 |
| **6-2 F-08.8 `Double tap shift = caps lock`** ⭐ | **`flagsChanged` 형태**로 좌 shift 를 두 번 탭 → `ioreg` 의 `HIDCapsLockState` 가 `No→Yes→No` 로 **정확히 한 번씩** 바뀌었다. 경로 C(`IOHIDSetModifierLockState`)가 실제로 동작한다 |
| **R1 (F-08.1 ↔ F-08.2 배타)** | 둘 다 켠 상태에서 caps lock 소스 키를 150ms 톡 누름 → 잠금 토글(quick press). 길게 눌러 다른 키와 조합 → 조합 발화. 같은 눌림이 두 규칙 중 하나에만 소비된다 |
| **6-4 v1.20 회귀** ⭐ | hyper 소스 = caps lock **이면서** `Caps lock + W A S D` ☑ 인 구성에서, 소스 키를 누른 채 `W` 두 번 → ▲ 가 두 번 났고 **hyper 조합이 얹히지 않았다**(P5). 원본이 v1.20 에서 깨졌던 바로 그 구성이다 |
| **6-4 v1.62 회귀** ⭐ | `Shift + caps lock = caps lock` 과 `Quick press caps lock` 을 둘 다 켠 상태에서 shift 를 누른 채 caps lock 을 톡 → 잠금이 **정확히 한 번** 토글됐다(`Yes→No`). 두 규칙이 겹쳐 발화했다면 두 번 토글돼 제자리로 돌아왔을 것이다 |
| **6-5 충돌 대화상자 3종** | 셋 다 실기기에서 떴다. `Caps Lock is already spoken for` → 배타 대상 `Hyper key source` / `Two Caps Lock arrow layouts at once` → `Caps Lock + W A S D…` / `The home row overlaps the arrow keys` → `Caps Lock + W A S D…`. `계속` 을 누르면 상대가 꺼지고 원래 값이 적용됐고, `취소` 는 아무것도 바꾸지 않았다 |
| **6-5 항목 4 (R2 예외)** | hyper 소스 = caps lock 인 상태에서 `Caps lock + W A S D` 를 켤 때 **대화상자가 뜨지 않았다**. 두 구성은 동시에 성립해야 한다 |
| **7-a 메뉴 구조** | AX 로 판독: `Ignore <앱>` · 구분선 · `Settings…` · `About` · `Advanced ▸ (Synthesize Caps Lock Remap · Relaunch)` · 구분선 · `Quit Ultrakey`. 최전면 앱을 바꾸자 라벨이 `Ignore Finder` 로 실시간 갱신됐다 |
| **7-b 앱별 비활성화** | TextEdit 이 최전면일 때 `Ignore TextEdit` 클릭 → `general.disabledApps` 에 즉시 기록. 그 상태에서 `F18 + W` 가 **발화하지 않고 원본 `w` 가 그대로 들어갔다**(계층 0 문지기). 다시 클릭해 해제하니 프리셋이 되살아났다 |
| **7-d `Launch on login`** | 위 1-b #6 재판정 참조(등록·해제 확인, 로그아웃 왕복 미수행) |
| **7-e 단일 인스턴스** | `open -n` 으로 두 번째 프로세스를 띄우자 `같은 번들 ID 로 이미 실행 중인 인스턴스가 있다 …` 로그와 함께 즉시 종료됐고, 남은 인스턴스는 1개였다 |
| **정상 종료** | 메뉴 `Quit Ultrakey` → 엔진 정리 후 프로세스 종료. **경로 B 매핑도 함께 비워졌다** |
| **탭별 창 리사이즈** | Presets 825×527 · Hyperkey 710×517 · General 613×273 — `preferences-ui.md` §3.1 실측값과 일치 |
| **종속 표현 ② 숨김** | 경로 B 매핑이 설치된 동안에만 "Caps Lock is remapped in the kernel right now…" 안내 행이 DOM 에 나타났다(매핑이 없을 때 찍은 스크린샷에는 없다) |

### 증거 스크린샷

| 파일 | 무엇 |
| :--- | :--- |
| [`screenshots/issue-15-presets-tab.png`](screenshots/issue-15-presets-tab.png) | Presets 탭 16종 4그룹. ② 숨김 안내(커널 매핑)와 ③ 문장 중간·끝 삽입 팝업이 함께 보인다 |
| [`screenshots/issue-15-conflict-caps-remapped.png`](screenshots/issue-15-conflict-caps-remapped.png) | 충돌 #1 — 배타 대상이 `Hyper key source` 로 **올바르게** 표시된다(수정 후) |
| [`screenshots/issue-15-conflict-caps-arrows.png`](screenshots/issue-15-conflict-caps-arrows.png) | 충돌 #2 — WASD ↔ HJKL |
| [`screenshots/issue-15-conflict-home-row.png`](screenshots/issue-15-conflict-home-row.png) | 충돌 #3 — home row ↔ 방향키 |
| [`screenshots/issue-15-general-launch-on-login.png`](screenshots/issue-15-general-launch-on-login.png) | General 탭 613×273. `Start Ultrakey at login` 이 활성이고, 아직 범위 밖인 자동 업데이트만 비활성이다 |

### ⬜ 수행하지 못한 것 — 통과했다고 적지 않는다

| 항목 | 왜 |
| :--- | :--- |
| **물리 caps lock 으로의 검증 전반** | §6-0 — 이 기기의 Karabiner 가 경로 B 보다 아래에서 caps lock 을 바꾼다. 합성 F18 로 우회했고, 그 한계는 위 표에 적었다 |
| **`unauthorizedMenu`(7-c)** | Accessibility 권한 회수·재부여가 필요한데, 재부여에 사용자 인증이 걸려 실패하면 앱이 동작 불능 상태로 남는다. **위험 대비 얻는 것이 작다고 판단해 수행하지 않았다** |
| **6-3 F-08.11 괄호 quick press** | 미수행 |
| **6-6 Secure Input** | 미수행. 경로 B 가 Secure Input 구간에서 어떻게 되는지는 `power-user-presets.md` §9.2 #10 의 `(미확정)` 그대로 남는다 |
| **나머지 프리셋 9종**(F-08.1·6·7·9·12·13·14·15·16)의 실동작 | 미수행. 판정 로직은 단위 테스트로 덮여 있으나 **실기기 확인은 하지 않았다** |
| **macOS 12 LaunchAgent 폴백** | 기기가 macOS 26.5.2 라 확인 불가 |

### 🐛 이 검증이 잡은 결함

⭐ **충돌 대화상자가 배타 대상을 잘못 가리켰다.** `Caps Lock is already spoken for` 대화상자가
"이걸 켜면 **`Send this key instead of Caps Lock:`** 를 끕니다" 라고, **지금 켜려는 설정 자신**을
꺼야 할 대상으로 표시했다. `conflicts.rs` 가 `to_disable` 에 `PRESETS_CAPS_LOCK_REMAP_ENABLED`
를 넣고 있었기 때문이다. 배타 대상은 **caps lock 을 이미 점유한 hyper/meh/bleh 슬롯**이어야 한다.

- 단위 테스트는 이 버그를 잡지 못했다 — 테스트가 코드와 같은 상수를 기대값으로 썼기 때문이다.
  **"무엇이 옳은가"가 아니라 "코드가 무엇을 하는가"를 검증하고 있었다.**
- 고친 뒤 `caps_modifier_slot_keys()` 를 도입해 실행 시점에 배타 대상을 계산하고, 반대 방향
  (hyper 슬롯을 켤 때 `Remap caps lock to:` 가 이미 켜져 있는 경우)도 대칭으로 처리했다.
- 회귀 테스트를 **동작 기준으로** 다시 썼다(`caps_lock_already_remapped_disables_the_modifier_slot_not_itself`).

### 검증 후 되돌린 것

- `settings.json` 을 **검증 전 상태로 복원**했다(hyperkey 3키 + `ui.lastTab` 만 존재).
- caps lock 잠금 상태를 `No` 로, `hidutil` 매핑을 빈 상태로 되돌렸다.
- 검증을 위해 잠시 종료했던 **기존 인스턴스(메인 체크아웃 빌드)를 다시 띄웠다.**
- ⛔ **키체인·시스템 설정·사용자의 Karabiner 설정은 건드리지 않았다.** Accessibility 권한도
  토글하지 않았다(위 7-c 참조).

---

## 결과 기록

각 항목을 수행한 뒤 **통과 여부와 관찰한 것**을 이슈 #5 또는 후속 이슈에 남긴다.
⭐ 특히 명세가 `(미확정)` 으로 남긴 값들 — 워치독 주기, 절전 복귀 지연, 재시작 디바운스 임계값, double tap 간격, Secure Input 확인 오버헤드 — 은 **이 절차가 그것을 실측으로 바꿀 수 있는 첫 기회**다([`architecture.md`](architecture.md) §4 표의 "설계 판단" 행 전부). 관찰값이 나오면 명세의 9절과 `architecture.md` §4 를 함께 갱신한다.
