# M1 수동 검증 절차

> **이 문서의 목적**: `docs/spec/README.md` 의 **"✅ M1 완료 판정" 5개 항목**을 각각 어떻게 확인하는지 적는다.
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
| 6 | 로그아웃 → 로그인 | 리매핑이 수동 개입 없이 동작한다 | §5 #4 |
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

## 항목 2 — `caps lock` 을 hyper 로 지정하면 다른 앱이 `⌃⌥⌘⇧` 4개 modifier 를 인식한다

> 근거 명세: `hyperkey.md` §3.1 · §8

1. 설정에서 `Remap key to hyper key: caps lock` 을 켜고 `Include shift in hyper key` 를 **켠 상태**로 둔다.
2. **키보드 뷰어**를 띄운다.
3. caps lock 을 **누른 채 유지**한다.

| 확인 | 기대 |
| :--- | :--- |
| 키보드 뷰어의 modifier 강조 | ⭐ **`⌃` `⌥` `⌘` `⇧` 4개가 전부 강조**된다 |
| caps lock 의 실제 잠금 | **켜지지 않는다** — 대문자 고정도, LED 점등도 없다. 원본 keyDown 이 소비되기 때문이다 |
| caps lock 을 뗀다 | 4개 강조가 **즉시 전부 해제**된다 |
| 다른 앱의 단축키 | 물리적으로 `⌃⌥⌘⇧` 를 눌러 등록해 둔 단축키가 `caps lock + <키>` 로 정상 트리거된다 |

**실전 확인 방법 하나** — 시스템 설정 → 키보드 → 키보드 단축키에서 아무 항목의 단축키를 물리 `⌃⌥⌘⇧ + <키>` 로 등록한 뒤, `caps lock + <키>` 로 발동하는지 본다.

⚠️ **알려진 한계 — 버그가 아니다.** Keyboard Maestro 류 shortcut recorder 에 hyper 소스 키로 단축키를 **등록(recording)** 하는 것은 실패할 수 있다. 그러나 물리 modifier 4개로 등록해 둔 단축키의 **트리거는 정상이어야 한다** — 이 비대칭이 명세(`hyperkey.md` §3.4, §8)가 요구하는 동작이다.

### 2-b. meh · bleh

| 조합 | 소스 키를 눌렀을 때 키보드 뷰어 | 특히 확인할 것 |
| :--- | :--- | :--- |
| meh | `⌃` `⌥` `⇧` 3개 | ⛔ **`⌘` 가 강조되면 실패** |
| bleh | `⌃` `⌘` `⇧` 3개 | ⛔ **`⌥` 가 강조되면 실패** — v1.65 회귀 방지 대상(`hyperkey.md` §8) |

### 2-c. `Apply modifiers to keypress events and:`

출고 기본은 **`Click` 만 ON** 이다(실측, `hyperkey.md` §3.3).

| 조작 | 기대 |
| :--- | :--- |
| hyper 를 누른 채 클릭 | modifier 가 실린 클릭으로 전달된다 |
| hyper 를 누른 채 드래그 | `Drag` 가 꺼져 있으므로 **modifier 가 실리지 않는다** |
| `Scroll` 을 켜고 Safari 에서 hyper + 스크롤 | 확대/축소가 발동한다 — 이것이 명세가 이 항목을 기본 OFF 로 두는 이유다(§3.3) |

---

## 항목 3 — `Include shift in hyper key` 를 끄면 `⌃⌥⌘` 3개만 합성된다

1. `Include shift in hyper key` 를 **끈다**.
2. 키보드 뷰어를 띄우고 caps lock 을 누른 채 유지한다.

| 확인 | 기대 |
| :--- | :--- |
| modifier 강조 | ⭐ **`⌃` `⌥` `⌘` 3개만.** `⇧` 는 강조되지 않는다 |
| 저장 형태 | 단일 비트마스크의 shift 비트가 꺼진 것이지 별도 불리언 필드가 아니다(`hyperkey.md` §3.1 — `hyperFlags` 실측) |
| meh · bleh | ⭐ **영향받지 않는다.** 이 토글은 hyper 에만 적용된다(§3.1) — meh/bleh 의 `⇧` 는 여전히 강조되어야 한다 |

⚠️ `hyperkey.md` §9 #5 가 남긴 미해결 질문: hyper 가 **Active 인 도중** 이 토글을 바꾸면 즉시 반영되는지 다음 keyDown 부터인지 확정되지 않았다. 관찰되면 그 결과를 명세 §9 에 기록한다.

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

## 결과 기록

각 항목을 수행한 뒤 **통과 여부와 관찰한 것**을 이슈 #5 또는 후속 이슈에 남긴다.
⭐ 특히 명세가 `(미확정)` 으로 남긴 값들 — 워치독 주기, 절전 복귀 지연, 재시작 디바운스 임계값, double tap 간격, Secure Input 확인 오버헤드 — 은 **이 절차가 그것을 실측으로 바꿀 수 있는 첫 기회**다([`architecture.md`](architecture.md) §4 표의 "설계 판단" 행 전부). 관찰값이 나오면 명세의 9절과 `architecture.md` §4 를 함께 갱신한다.
