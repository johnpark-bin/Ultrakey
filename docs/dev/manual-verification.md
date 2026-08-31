# 수동 검증 절차 (M1 · M2 · M3)

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

#### 📌 재판정 (2026-08-31, 이슈 #39) — 🟡 **여전히 부분 통과. 다만 "왜 안 되는가"가 실측으로 좁혀졌다**

사용자 보고: *"로그인 시에 자동으로 시작하기 기능은 지금 현재 구현이 안 되어 있는 것 같아"*.

⭐ **가설을 채택하지 않고 실측부터 했다.** 앱에 진단 프로브(`ULTRAKEY_LOGIN_ITEM_PROBE`)를 넣어
`register → status → unregister → status` 를 돌리고, 매 단계의 **OS 정본**(`SMAppService.status`)과
BTM 데이터베이스(`sfltool dumpbtm`)를 함께 읽었다.

**✅ 실측으로 확정 — `SMAppService` 계층은 정상이다**

| 확인한 것 | 결과 |
| :--- | :--- |
| 등록 | ✅ `register` 성공 → `status = Enabled(1)` |
| BTM 기록 | ✅ `Type: app (0x2)` · `Disposition: [enabled, allowed, notified] (0xb)` · `URL` = 실행 중인 `.app` 절대 경로 |
| 해제 | ✅ `status = NotRegistered(0)`, BTM 은 `Disposition: [disabled, ...]` 묘비로 남는다(macOS 정상 동작) |
| 해제 후 재등록 | ✅ 다시 `Enabled` — 앱 자신의 해제는 `RequiresApproval` 을 유발하지 않는다 |
| ⭐ 번들 재빌드 후(cdhash 변경) | ✅ 여전히 `Enabled` — **"재빌드가 등록을 깬다"는 가설은 기각** |
| macOS 12 폴백 오발동 | ✅ 없음. `~/Library/LaunchAgents` 에 plist 가 생기지 않는다 |

⭐ **부수 발견**: 등록 레코드가 아예 없을 때 `status` 는 `NotRegistered(0)` 가 아니라 **`NotFound(3)`** 다.
기존 코드는 `status == 1` 이 아니면 전부 "꺼짐"으로 접어 이 구분을 잃고 있었다.

**⭐ 그래서 무엇이 문제였나 — 코드에서 확정한 결함 셋** (상세: [`../spec/menu-bar-and-lifecycle.md`](../spec/menu-bar-and-lifecycle.md) §3.5-a)

| # | 결함 | 사용자에게 어떻게 보이는가 |
| :--- | :--- | :--- |
| ① | UI 가 OS 정본이 아니라 저장된 "거울"을 읽는다 | OS 에서 항목이 사라져도 **체크박스는 계속 ☑** — "켜 놨는데 안 된다" |
| ② | 등록 실패해도 거울에 사용자 의도가 먼저 쓰인다 | 실패 상태가 `true` 로 굳어 ① 과 합쳐져 영구적 거짓 표시 |
| ③ | 기동 시 재조정이 없다 | ⭐ 이 앱은 **워크트리의 `target/` 빌드 디렉터리에서 실행**되고 BTM 에 그 절대 경로가 박힌다(실측). 워크트리가 정리되면 그 경로는 사라지고 **로그인 시 아무것도 뜨지 않는다.** 다른 위치의 빌드를 실행해도 아무도 다시 등록해 주지 않는다 |

셋 다 고쳤다 — 정본을 OS 로 바꾸고, 기동 시 재조정하고, 등록 결과를 `status()` 로 재검증한다.

**⬜ 그래도 판정은 🟡 다 — 로그아웃 → 로그인 왕복은 이번에도 수행하지 않았다.**

⭐ **PR #17 이 정확히 여기서 멈췄다가 이 사고가 났으므로, 이번에는 넘기더라도 명시적으로 넘긴다.**
사용자 세션을 끊는 되돌리기 어려운 조작이라 위임 경계 밖이다. **사용자가 직접 아래를 수행해야 이 항목이 닫힌다.**

```
1. General 탭에서 `Launch on login` 을 켠다
2. 체크박스 아래에 "승인 필요" 안내가 뜨는지 본다
   → 뜬다면: 시스템 설정 ▸ 일반 ▸ 로그인 항목에서 Ultrakey 를 켜고 다시 확인한다
3. 시스템 설정 ▸ 일반 ▸ 로그인 항목 에 Ultrakey 가 있는지 확인한다
4. ⭐ 로그아웃 → 로그인
5. 앱이 자동으로 떴는가? 수동 개입 없이 리매핑이 동작하는가?
6. ~/Library/Logs/Ultrakey/ultrakey.log 에 기동 로그가 있는지 본다
```

⚠️ **`RequiresApproval` 상태 자체는 실측하지 못했다.** 재현하려면 시스템 설정을 바꿔야 하는데
그것은 이 작업의 경계 밖이다(⛔ 시스템 설정 변경 금지). `SMAppServiceStatus` 헤더가 정의하는
값이라는 것과, 우리 코드가 그 값을 받으면 어떻게 행동하는지까지가 확정된 범위다.

⚠️ **검증 중 되돌린 것**: 프로브가 로그인 항목을 등록했다가 해제했다. 확인 시작 시점의 상태
(`general.launchOnLogin: false`, 등록 없음)로 되돌려 두었다. `~/Library/LaunchAgents` 에는
아무것도 만들지 않았다.

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

### ⭐ A-bis. D-1 매핑의 소유권 (2026-08-30 신설, 이슈 #19)

M1 에는 이 엔진이 설치하는 경로 B 규칙이 **0개**여서 위 1·3 이 서로 충돌하지 않았다.
**D-1 이 그 전제를 바꿨다** — 이제 우리가 설치하는 매핑이 정확히 하나 있고
(`caps lock 0x700000039 → F18 0x70000006D`), 그 값을 우리가 안다. 그래서 잔존 매핑을
**우리 것 / 남의 것**으로 나눠 다뤄야 한다.

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | caps lock 프리셋을 하나 켠 뒤 `hidutil property --get UserKeyMapping` | `Src=30064771129`, `Dst=30064771181` 이 **나타난다** |
| 2 | ⭐ 메뉴바 `Advanced ▸ Synthesize Caps Lock Remap` 을 **켠다**, 그리고 다시 조회 | ⭐ **그 매핑이 즉시 사라진다.** 앱을 다시 띄울 필요가 없다 |
| 3 | 다시 **끈다**, 조회 | 매핑이 다시 나타난다 |
| 4 | 매핑이 걸린 채로 앱을 `kill -9` → `Synthesize Caps Lock Remap` 이 켜진 상태로 재실행 | 시작 시 재조정이 **우리 매핑을 걷어낸다**(부록 A #1). 로그: `우리가 설치한 D-1 매핑만 제거했다` |
| 5 | ⭐ 사용자가 직접 건 **다른** 매핑(예: `hidutil property --set` 로 넣은 임의 쌍)이 함께 있는 상태에서 4 를 반복 | ⛔ **그 매핑은 그대로 남아 있어야 한다**(부록 A #3). 우리 서명과 일치하는 항목만 사라진다 |
| 6 | 앱 정상 종료 후 조회 | 우리 매핑은 없고 남의 매핑은 남아 있다 |

⛔ **2·4 가 이슈 #19 증상 B 의 회귀 방지 지점이다.** 이 둘이 깨지면 `caps lock → F18` 이
커널에 남은 채 중재기는 그것을 caps lock 으로 되돌릴 alias 를 잃어, **물리 caps lock 이
아무 규칙에도 걸리지 않고 앱 안에서 되돌릴 수단도 없다.** 사용자가 보고한 "아예 캡스락
이벤트 자체가 캡처가 안됨" 이 그 상태다.

⚠️ 5 를 실제로 해 보려면 사용자 시스템 전역 상태를 건드리게 된다 — **검증 후 반드시
원래대로 돌려놓고, 무엇을 바꿨는지 기록**한다.

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

---

### ⭐ 6-0-bis. 어느 키를 눌러야 하는가 — Karabiner 상태 × D-1 (2026-08-30 신설, 이슈 #19)

⛔ **위 표만 보고 검증하면 이슈 #19 를 다시 놓친다.** 위 표는 "Karabiner 가 켜져 있다"를
전제로 쓰였는데, 사용자가 Karabiner 를 **끄면 눌러야 할 키가 정반대로 바뀐다.** 실제로
이슈 #19 는 사용자가 Karabiner 를 끄고 물리 caps lock 을 직접 눌러 발견한 것이다.

**먼저 Karabiner 가 실제로 도는지 확정한다.** 프로세스 목록만으로는 부족하다 — Karabiner 를
꺼도 `Karabiner-Core-Service`·`Karabiner-VirtualHIDDevice-Daemon`·DriverKit 확장은 root
데몬으로 **계속 살아 있다.** 판별 기준은 **가상 키보드가 HID 장치로 등록돼 있는가** 다.

```sh
# 1 이상이면 Karabiner 가 실제로 키를 가로채고 있다. 0 이면 꺼져 있다.
ioreg -c IOHIDDevice -r -d1 | grep -c "Karabiner DriverKit VirtualHIDKeyboard"
# 사용자 세션 프로세스(있으면 켜져 있다)
pgrep -lf "Karabiner-Elements.app|console_user_server"
```

| Karabiner | 물리 caps lock 이 탭에 도착하는 모양 | 검증 시 눌러야 할 키 |
| :--- | :--- | :--- |
| **켜짐**(`caps_lock ↔ left_control`) | caps lock → (Karabiner) `left control` → 탭. **caps lock 은 영영 오지 않는다** | **물리 왼쪽 control** (그것이 `caps lock` 으로 바뀌어 도착) |
| **꺼짐** + D-1 켜짐 | caps lock → (`hidutil`) **`F18`** → 탭. 중재기가 alias 로 caps lock 으로 되돌린다 | **물리 caps lock** |
| **꺼짐** + D-1 꺼짐(`Synthesize Caps Lock Remap` ☑) | caps lock 이 `flagsChanged` 로만, **뗄 때는 오지 않는다**(래칭, §5 #20) | **물리 caps lock**. 단 홀드 계열은 원리적으로 토글처럼 동작한다 |

⭐ **추측하지 말고 계측으로 확정한다 (이슈 #19 에서 신설).** 탭에 실제로 도착한 원본
이벤트를 로그로 볼 수 있다:

```sh
open --env ULTRAKEY_TRACE_TAP=1 -n target/universal-apple-darwin/release/bundle/macos/Ultrakey.app
tail -f ~/Library/Logs/Ultrakey/ultrakey.log | grep "탭 계측"
```

이 로그는 `raw_keycode`(도착한 그대로)·`raw_kind`·`raw_flags`·`resolved`(alias 환원 뒤)·
`layer`·`disp`·`emitted`·`path_c` 를 한 줄로 남긴다. **caps lock 을 한 번 눌러 보고
`raw_keycode` 가 `0x39`(caps lock)인지 `0x4F`(F18)인지 `0x3B`(left control)인지 먼저 확인한 뒤**
아래 절차를 시작하라. 이 확인 없이 "동작하지 않는다"고 적으면 안 된다.

⛔ **`cargo tauri dev` 나 `.app` 안의 바이너리 직접 실행으로는 안 된다** — TCC 가 부모
프로세스 권한으로 판정한다. 반드시 `open` 으로 띄운다.

⭐ **D-1(caps lock 모멘터리 정규화, `architecture.md` §6.1)과의 상호작용도 여기서 관찰한다.**
Ultrakey 가 설치하는 경로 B 매핑(`caps lock → F18`)은 `hidutil` 이므로 Karabiner 의 가상 HID
키보드에도 적용된다 — 따라서 물리 `left control` → (Karabiner) `caps lock` → (hidutil) `F18` 로
도착할 것으로 **예상**된다. 이것이 실제로 그런지는 아래 6-1 에서 확인한다.

```sh
# 프리셋을 켠 뒤 — Ultrakey 가 설치한 매핑이 보여야 한다
hidutil property --get UserKeyMapping
# 프리셋을 전부 끈 뒤 — (null) 로 돌아와야 한다
```

### ⭐ 6-1-0. `Remap caps lock to:` — **대상이 modifier 키인 경우** (F-08.1, 이슈 #19 신설)

⛔ **이 절이 없어서 이슈 #19 증상 A 를 놓쳤다.** `Remap caps lock to:` 의 대상 50종 중
**8종이 modifier 키**다(`left/right control`·`shift`·`option`·`command`). macOS 는 modifier 의
눌림을 `KeyDown` 이 아니라 **`flagsChanged` + flags 비트**로만 전달하므로
(`../spec/key-remapping-engine.md` §5 #18), 대상이 modifier 일 때는 **다른 대상과 검증
포인트가 다르다.** 방향키·`esc` 로만 확인하면 이 결함이 드러나지 않는다.

관찰 수단은 [`tools/modifier-probe.html`](tools/modifier-probe.html)(Chrome).

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | `Remap caps lock to:` ☑ `left control` 로 두고, 6-0-bis 표가 지정한 키를 **누른다** | 프로브에 `keydown` / `ControlLeft` 행이 찍히고 **`⌃` 열이 `T`** 다 |
| 2 | 그대로 **뗀다** | `keyup` / `ControlLeft` 행이 찍히고 `⌃` 열이 `F` 로 돌아간다 |
| 3 | ⭐ 눌림·뗌이 **둘 다 `keyup` 으로만** 찍히면 **실패다** | 이것이 이슈 #19 증상 A 다 — `KeyDown` 으로 합성해 control 비트가 한 번도 켜지지 않은 상태 |
| 4 | ⭐ 그 키를 **누른 채 `C`** | `⌃C` 가 성립한다(프로브의 `⌃` 열이 `T` 인 `keydown c`). 합성 `flagsChanged` 한 번만으로는 여기서 실패한다 — 뒤따르는 키 이벤트에도 비트가 실려야 한다 |
| 5 | 대상을 `esc` 로 바꾸고 1~2 반복 | `keydown`/`keyup` 의 `Escape` — modifier 가 아닌 대상은 종전대로 키 이벤트로 나가야 한다(과잉 교정 방지) |
| 6 | 그 키를 누르는 동안 caps lock **잠금이 켜지지 않는다** | LED 가 꺼진 채이고 타이핑이 소문자다(F-08.1 "OS caps lock 잠금 상태는 발생하지 않음") |

⭐ 계측 로그(`ULTRAKEY_TRACE_TAP=1`)로도 같은 것을 본다 — `emitted` 열이
`FlagsChanged 0x3B 0x00040001` 이어야 하고, `KeyDown 0x3B 0x00000000` 이면 실패다.

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
| 4 | shift 를 **한 번만** 탭 | caps lock 은 토글되지 않는다. ⚠️ 아래 5·6 을 함께 본다 |
| 5 | ⭐ **`Shift + a` 를 친다**(shift 를 누른 채 `a`) | **대문자 `A`** 가 나온다 |
| 6 | ⭐ shift 를 한 번만 톡 누른 **직후** `a` 를 친다 | 소문자 `a` 가 나온다(앞의 탭이 뒤 글자에 영향을 주지 않는다) |

⛔ **5·6 이 이 절의 핵심이다 — 이슈 #19 증상 C.** 이전 판에는 이 두 행이 없었고, 그래서
"이 프리셋을 켜면 **shift 키가 통째로 죽는다**"는 결함이 검증을 통과했다. 명세는 F-08.8 이
shift 를 무력화한다고 쓴 적이 없다(`../spec/power-user-presets.md` §3.2 F-08.8 행의 "부가
효과" 열은 비어 있다). 프리셋이 **더하는** 제스처이지 **빼앗는** 것이 아니다.
`Quick press … shift`(F-08.11, 6-3)에도 같은 확인이 필요하며 그쪽은 6-3 #3 이 이미 담당한다.

### 6-3. quick press 계열 — `Quick press left or right shift to input corresponding:` (F-08.11)

⭐ **관찰 수단**: 2-0 의 `docs/dev/tools/modifier-probe.html` 을 그대로 쓴다 — 이 절은 "무슨 글자가 들어갔는가" 뿐 아니라 **"그 이벤트가 어떤 키로 보이는가"**(`code`·`keyCode`)까지 봐야 하는데, 그 두 가지를 한 화면에 함께 남기는 것이 이 프로브다. 텍스트가 실제로 삽입되는지도 함께 보려면 프로브 페이지의 입력 필드(또는 아무 텍스트 필드)에 포커스를 준 채 친다.

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | 프리셋을 켜고 팝업을 `( )` 로 둔 뒤, **왼쪽** shift 를 톡 눌렀다 뗀다 | `(` 가 입력된다 |
| 2 | **오른쪽** shift 를 톡 눌렀다 뗀다 | `)` 가 입력된다 |
| 3 | shift 를 누른 채 다른 글자를 친다 | 평범한 대문자가 나가고 괄호는 나오지 않는다(P2 — 다른 키가 눌리는 순간 quick press 후보에서 빠진다) |
| 4 | ⭐ 입력 소스를 한글(2벌식)로 바꾸고 1·2 반복 | **같은 괄호 문자**가 나온다(R5 / v1.51 회귀 방지 — 출력은 레이아웃과 무관하게 고정) |
| 5 | ⭐ shift 를 **1초 넘게** 눌렀다 뗀다(다른 키는 누르지 않는다) | 괄호가 나오지 **않는다** — `Quick press duration` 기본값 1000 ms 를 넘겼으므로 hold 다 |
| 6 | ⭐ **프로브의 `code`·`keyCode` 열을 본다** | `code=Digit9` · `keyCode=57` · `shift=T` (US 배열 기준 `(` 를 내는 진짜 키). ⛔ `code=KeyA` · `keyCode=65` · `shift=F` 로 보이면 `localization-and-input-sources.md` §3.2.2 의 **(ii) 폴백이 1순위로 잘못 쓰이고 있다** — 아래 결과 절이 그 결함이다 |

#### 📌 실제 통과 결과 (2026-08-30, 이슈 #32) — ⚠️ **사용자 보고를 재현하지 못했다**

사용자가 "이 기능이 정상동작하지 않는 것 같음"이라고 보고해 진단했다. **결론부터: 이 절의 1~5 는 수정 전에도 전부 통과했다.** 재현하지 못한 것을 재현했다고 적지 않는다.

**확인 방법.** `scripts/build-signed.sh` 로 만든 `.app` 을 `open` 으로 띄우고(사용자의 실제 `settings.json` 그대로 — `presets.shiftQuickPressBrackets.enabled: true`), `ULTRAKEY_TRACE_TAP=1` 로 계측을 켠 뒤, `cargo run -p ultrakey-platform --example key_poke` 로 **`flagsChanged` 모양 그대로** 좌/우 shift 를 주입했다. 받는 쪽은 `WKWebView` 의 `<input>` (브라우저·Electron 계열과 같은 텍스트 입력 경로)과 AppKit `NSTextField` 두 가지로 각각 확인했다.

| 확인한 것 | 결과 |
| :--- | :--- |
| 엔진이 quick press 를 발화하는가 | ✅ 좌/우 모두 `effects=[TypeChar]`. 계측 원문: `탭 계측 seq=1 raw_kind=FlagsChanged raw_keycode=0x38 … layer=PresetCombo disp=Pass emitted=[] effects=[TypeChar]`, `seq=3 … raw_keycode=0x3C … effects=[TypeChar]` |
| 합성 이벤트가 실제 이벤트 스트림에 실리는가 | ✅ 세션 탭 **꼬리**에 붙인 수동 탭(`examples/tap_listen`)이 그대로 관측했다: `type=KeyDown keycode=0x00 flags=0x00000000 srcUD=0x554B4559 unicode="("` |
| 1·2 (좌 `(` · 우 `)`) | ✅ 입력됨 |
| 3 (shift + 글자) | ✅ `B` 만 나가고 괄호 없음 |
| 4 (한글 2벌식 입력 소스) | ✅ `INPUTSOURCE=com.apple.inputmethod.Korean.2SetKorean` 상태에서 `()` 그대로 입력됨 |
| 5 (1.2초 홀드) | ✅ 괄호 나오지 않음 |
| 25 ms 짧은 탭 · 연타 2회 · 글자 직후 탭 | ✅ 전부 정상 |

**🐛 그런데 이 진단이 다른 결함 하나를 잡았다 — 6번 항목이다.**

프로브가 받은 keydown 이 이렇다:

```
kd key="(" code=KeyA kc=65 shift=false
```

문자는 `(` 가 맞지만 **키는 `A`(keycode 0) 로 보인다.** `localization-and-input-sources.md` §3.2.2 는 문자 출력에 **(i) `UCKeyTranslate` 역방향 탐색을 1순위**로, `CGEventKeyboardSetUnicodeString` 은 **(ii) 폴백**으로 정해 두었는데, 구현은 (ii)만 하고 있었다. (i)에 필요한 역방향 표(`ultrakey-layout::LayoutTable::keycode_for_char`)는 `(`·`)` 에 대한 레이아웃 독립 테스트까지 갖춘 채 **부르는 곳이 한 군데도 없었다** — 이 저장소가 이미 세 번 겪은 *입력 쪽 대칭이 출력 쪽에 없다*(PR #23) 계열의 네 번째다.

실사용 영향: 텍스트를 **삽입**하는 경로만 쓰는 앱에서는 티가 나지 않지만(위 1~5 가 통과한 이유다), keydown 의 `code`/`keyCode` 로 키를 판정하는 앱 — 에디터의 키 바인딩, 브라우저·Electron 앱의 단축키 처리, 게임 — 에서는 괄호가 아니라 `A` 로 인식된다.

⬜ **확인하지 못한 것**: 사용자가 어느 앱에서 어떻게 시험했는지 모른다. 위 (ii)-only 결함이 사용자가 본 증상의 원인인지는 **확인하지 못했다** — 이 수정이 그 증상을 없앤다고 단정하지 않는다. 이슈 #32 에 어떤 앱에서 시험했는지 질문을 남겼다.

#### ✅ 수정 후 재확인 (2026-08-30, 이슈 #32)

`plan_text_output` 이 §3.2.2 (i)를 먼저 쓰도록 고친 뒤, 같은 방법으로 다시 쟀다.

| # | 결과 |
| :--- | :--- |
| 1·2 | ✅ 좌 shift → `(`, 우 shift → `)`. 입력 필드 내용 `()` |
| 5 | ✅ 1.2초 홀드 — 괄호 나오지 않음 |
| 3 | ✅ shift + `b` → `B` 만, 괄호 없음 |
| **6** | ✅ **고쳐졌다.** `kd key="(" code=Digit9 kc=57 shift=true` · `kd key=")" code=Digit0 kc=48 shift=true` — 수정 전의 `code=KeyA kc=65 shift=false` 가 이제 실제 괄호 키를 가리킨다 |

⬜ **수정 후에 확인하지 못한 것 — 4번(한글 2벌식 입력 소스)**. 수정 **전**에는 통과를 확인했다(`INPUTSOURCE=com.apple.inputmethod.Korean.2SetKorean` 에서 `()` 입력됨). 수정 후에는 재현하지 못했다 — macOS 입력 소스는 앱별로 유지되어 검증용 프로브 앱의 입력 소스를 외부에서 한글로 바꿀 수 없었고, 사용자 수준 설정을 바꾸지 않기로 한 제약 안에서는 여기까지가 한계다. 근거상 통과할 것으로 본다 `(추정)`: 한글 IME 는 `kTISPropertyUnicodeKeyLayoutData` 를 주지 않아 레이아웃 표가 ASCII 로 폴백하고(로그 `used_ascii_fallback=true`), 그 표의 `(` 는 `shift+9` 이며 2벌식은 그 조합을 그대로 `(` 로 통과시킨다. **실측으로 확정한 것이 아니다 — 사용자 확인이 필요하다.**

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

### 7-f. ⭐ 언어 선택 (D6 / 이슈 #39)

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | `General` 탭의 `Language` 팝업 | `System` · `English` · `한국어` · `中文` · `Español` · `日本語` 6개 항목. ⭐ **각 언어 이름이 그 언어로** 쓰여 있다(현재 UI 언어로 번역되어 있으면 안 된다) |
| 2 | `한국어` 를 고른다 | **재시작 없이 즉시** 환경설정 창 전체가 한국어로 바뀐다 |
| 3 | 메뉴바 아이콘을 클릭한다 | 메뉴 항목도 한국어다(창만 바뀌면 안 된다) |
| 4 | `中文` → `Español` → `日本語` 로 차례로 바꾼다 | 매번 즉시 바뀌고, 라벨이 잘리거나 빈 문자열이 되지 않는다 |
| 5 | `settings.json` 을 본다 | `general.language` 키가 있다 |
| 6 | `System` 을 고른다 | 시스템 언어로 돌아가고, ⭐ `settings.json` 에서 **키가 사라진다**("부재 = 기본값") |
| 7 | ⭐⭐ **`~/Library/Logs/Ultrakey/ultrakey.log` 를 본다** | ⭐ **어느 언어를 골랐든 로그는 전부 영어다.** 한글이 한 줄도 없어야 한다 |

### 7-g. ⭐ 설정 export / import (D7 / 이슈 #39)

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | `General` 탭의 `Export…` | 저장 대화상자가 뜨고 `.json` 을 쓴다 |
| 2 | 그 파일을 연다 | `kind: "ultrakey.settings-export"` · ⭐ **`values` 에 "건드린 키만"** 들어 있다(수백 줄이면 안 된다). `perDevice._managed` 와 `ui.*` 는 **없다** |
| 3 | 설정 두어 개를 바꾼다 | 바뀐다 |
| 4 | `Import…` 로 1의 파일을 고른다 | ⭐ **교체**된다 — 3에서 바꾼 것이 파일의 상태로 되돌아간다. 파일에 없던 키는 **기본값으로** 돌아간다 |
| 5 | 저장 디렉터리를 본다 | `settings.json.pre-import-<epoch>` 백업이 있다 |
| 6 | 연결되지 않은 키보드 설정이 든 파일을 import | 거절하지 않는다. **보존하되** "지금 연결되어 있지 않은 키보드 N종" 안내가 뜬다 |
| 7 | 엉뚱한 JSON(예: `package.json`)을 import | 거절한다. ⭐ **저장소가 전혀 바뀌지 않는다** |
| 8 | import 직후 | 재시작 없이 엔진·메뉴·UI 에 반영된다 |

### 7-h. ⭐ Event Viewer (D8 / F-18 / 이슈 #39)

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | `General` 탭의 `Open Event Viewer` | 별도 창이 뜬다(설정 창 안의 탭이 아니다) |
| 2 | 다른 앱(편집기 등)에 포커스를 주고 키를 누른다 | ⭐ 뷰어 창이 **항상 위**에 있어 타이핑하면서 볼 수 있다. 줄이 실시간으로 쌓인다 |
| 3 | 평범한 글자 키를 누른다 | `Pass` · 규칙 없음 · 출력 없음 |
| 4 | ⭐ 켜 둔 프리셋의 조합을 누른다 | `Consume` 로 표시되고 색 막대가 붙는다. **어느 프리셋인지 이름이 보인다**(계층 이름만 보이면 안 된다) |
| 5 | `Pause` | 화면이 멈춘다. 다시 `Resume` 하면 이어진다 |
| 6 | `Clear` | 비워진다 |
| 7 | 창을 닫는다 | 계측이 멈춘다 |
| 8 | ⭐ **권한이 없는 상태에서 연다** | 안내가 뜬다(빈 화면으로 두지 않는다) |

⚠️ **"뷰어가 꺼져 있을 때 비용 0"은 단위 테스트가 지킨다** — `trace.rs` 의 `viewer_off_and_env_off_means_no_trace`.
실기기에서 "느려지지 않았다"를 눈으로 확인하는 것은 의미 있는 측정이 아니므로 그렇게 적지 않는다.

## 📌 실측 결과 (2026-08-31, 이슈 #39 / General 탭 확장)

빌드: `./scripts/build-signed.sh` → `target/universal-apple-darwin/release/bundle/macos/Ultrakey.app` 를 `open` 으로 실행.
조작은 AX(`System Events`)로 구동했고, 판정은 **스크린샷 · `settings.json` · 로그 파일**로 했다.

### ✅ 통과한 것

| 항목 | 결과 · 증거 |
| :--- | :--- |
| **7-f #1 언어 팝업** | ✅ `General` 탭에 `언어` 팝업이 있고 항목이 각 언어의 자기 이름이다([스크린샷](screenshots/39-general-tab-en.png)) |
| **7-f #2 즉시 적용** | ✅ `한국어` 선택 → **재시작 없이** 창 전체가 한국어로 바뀌었다([스크린샷](screenshots/39-general-tab-ko.png)) |
| **7-f #3 메뉴바도 함께** | ✅ 트레이 메뉴가 `[Firefox Developer Edition 무시하기][설정…][정보][고급][Ultrakey 종료]` 로 바뀌었다 — 창만이 아니라 **네이티브 표면도** 갱신된다 |
| **7-f #5 저장** | ✅ `settings.json` 에 `general.language: "ko"` 가 즉시 나타났다 |
| **7-f #6 System 복귀** | ✅ 키를 지우면 시스템 언어로 돌아간다("부재 = 기본값") |
| ⭐⭐ **7-f #7 로그는 항상 영어** | ✅ **UI 가 한국어인 상태로 이번 실행 로그 전체를 검사해 한글이 0줄이었다.** 언어를 바꾼 그 줄조차 영어다 — `general.language set locale="ko"` |
| **7-g #1·#2 export** | ✅ 저장 대화상자가 뜨고 파일이 쓰였다. ⭐ **키 28개만** 담겼고(수백 개가 아니다), `perDevice._managed` 와 `ui.*` 가 **없다.** `perDevice.5ac:24f.*`·`perDevice.all.*`(실제 사용자 설정)는 들어 있다 |
| ⭐ **7-g #4 import 는 교체다** | ✅ 파일에 **없는** 키(`korean.hanjaKeyConvertsHanja`)를 먼저 켠 뒤 import → **그 키가 사라졌다**(32개 → 31개). 병합이었다면 살아남았을 것이다. 로그: `settings imported … applied=31 removed=1 absent_devices=0` |
| **7-g #5 백업** | ✅ `settings.json.pre-import-1788105992` 가 생겼다 |
| **7-g #8 즉시 반영** | ✅ 재시작 없이 엔진 재구성 로그가 이어졌다(`reconfigured the Arbiter to reflect the settings change`) |
| ⭐ **기기 상태 키 보존** | ✅ import 후에도 `perDevice._managed` 와 `ui.lastTab` 이 **이 기기의 값 그대로** 남았다 |
| **7-h #1 별도 창** | ✅ `Event Viewer 열기` → 독립 창이 떴다(`windows = [Ultrakey — Event Viewer][Ultrakey]`) |
| ⭐ **7-h #3·#4 입력 → 해석 대조** | ✅ [스크린샷](screenshots/39-event-viewer.png). 평범한 키는 `Passthrough / Pass`, `⇧`·`⌘` 조합은 수정자가 표시된다. ⭐ **`KeyDown 0x4F fn → 0x39`** 로 D-1 alias 환원이 보이고, 그 줄이 `HyperModifier / Consume` 로 소비된 뒤 KeyUp 에서 `PresetCombo / Consume` + **`ToggleCapsLock ok off->on`** 효과가 찍혔다. 소비된 줄은 색 막대로 구분된다 |
| **7-d 로그인 항목 기동 재조정** | ✅ 기동 로그에 `reconciling login item mirror against OS state at boot mirror=false status="not-found"` — 새 재조정 경로가 실제로 돈다 |

### ⬜ 확인하지 못한 것 — 통과했다고 적지 않는다

| 항목 | 왜 |
| :--- | :--- |
| ⭐ **7-d #4 로그아웃 → 로그인 왕복 (= 1-b #6)** | 사용자 세션을 끊는 되돌리기 어려운 조작이라 수행하지 않았다. **여전히 미확인이다** — 절차는 1-b #6 절에 적어 두었고 사용자가 직접 마쳐야 한다 |
| `RequiresApproval` 상태 | 재현하려면 **시스템 설정을 바꿔야** 하는데 이 작업의 경계 밖이다(⛔ 시스템 설정 변경 금지) |
| macOS 12 LaunchAgent 폴백 | 검증 기기가 macOS 26 이다. 이전 회차와 같은 이유로 미검증 |
| 7-f #4 `中文`·`Español`·`日本語` 실제 전환 | `한국어` 전환만 실기기로 확인했다. 나머지 셋은 **카탈로그 키 집합 동일성·플레이스홀더 일치 단위 테스트**로만 담보된다 |
| 7-h #8 권한 없는 상태의 뷰어 안내 | 권한이 이미 부여된 기기라 그 상태를 만들지 못했다 |
| 뷰어 링 오버플로 표시 | 링(512칸)을 채울 만큼 빠른 입력을 만들지 못했다. 배선(`Engine::trace_dropped_count`)과 표시 코드는 있으나 **실제로 넘치는 것을 보지는 못했다** |

### ⚠️ 이 회차에서 새로 발견한 버그 (이 이슈 범위 밖 — 별도 처리가 필요하다)

⭐ **설정 창을 닫으면 다시 열 수 없다.**

```
ERROR ultrakey_app: window not found window_label="settings" what="show_settings"
```

메뉴바 `설정…` 을 눌러도 창이 뜨지 않는다. Tauri 는 창을 닫으면 기본적으로 **파괴**하는데,
`show_settings` 는 라벨로 기존 창을 찾기만 하고 없으면 새로 만들지 않는다.

- **이 회차의 변경이 원인이 아니다.** Event Viewer 의 `CloseRequested` 핸들러는 그 창 하나에만
  걸려 있고(`window.on_window_event`), 설정 창에는 아무것도 걸지 않았다. **기존 버그다** —
  7-a 는 메뉴 구조만 봤고 "닫았다가 다시 열기"를 한 적이 없다.
- 영향은 크다: 설정 창이 **모든 설정과 Event Viewer 로 가는 유일한 입구**다. 한 번 닫으면
  앱을 재시작해야 한다.
- 고치는 방법은 작다(라벨로 못 찾으면 `WebviewWindowBuilder` 로 다시 만든다). 이 이슈의
  범위 밖이라 **고치지 않고 기록만 한다.**

### ⚠️ 검증 중 바꿨다가 되돌린 것

| 바꾼 것 | 되돌림 |
| :--- | :--- |
| 로그인 항목 등록(프로브) | ✅ 해제. `sfltool dumpbtm` 에 `disabled` 묘비만 남는다(macOS 정상 동작) |
| `general.language = ko` | ✅ 키를 지웠다(= 시작 시점의 "부재") |
| `korean.hanjaKeyConvertsHanja` | ✅ import 가 지웠다(원래 부재였다) |
| `settings.json.pre-import-*` 백업 | ✅ 삭제 |
| caps lock 잠금(합성 F18 이 토글) | ✅ 다시 꺼서 `HIDCapsLockState=No` 확인 |
| 메인 체크아웃에서 돌던 인스턴스 종료 | ✅ 검증 후 다시 실행해 두었다 |

시작 시점 `settings.json` 과 최종 상태의 차이는 **없다**(`ui.lastTab` 까지 되돌렸다).
`~/Library/LaunchAgents` 에는 아무것도 만들지 않았고, `hidutil property --get UserKeyMapping` 은 비어 있다.

---

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

## 항목 9 — F-09 환경설정 창 크기 (이슈 #32 신설)

> 근거 명세: `../spec/preferences-ui.md` §3.1·§3.3 (⭐ **원본과 갈라지는 지점 D5** — 원본은 탭마다 창을 리사이즈하지만 클론은 하지 않는다)

⚠️ **9-b 를 하기 전에 현재 `settings.json` 을 백업하라.** 이 항목은 저장 파일에 `ui.windowWidth`/`ui.windowHeight` 를 실제로 만든다.

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 9-a #1 | 환경설정 창을 열고 왼쪽 메뉴에서 6개 탭을 차례로 누른다 | ⭐ **창 크기가 전혀 변하지 않는다.** 어느 탭에서도 내용이 세로로 잘리지 않는다 |
| 9-a #2 | 로그(`~/Library/Logs/Ultrakey/ultrakey.log`)에서 탭을 누른 시점을 본다 | `what="resize_settings"` 줄이 **없다**(복원 시 한 번을 빼면 이 문자열은 더 이상 나오지 않아야 한다) |
| 9-b #1 | 설정을 한 번도 건드리지 않은 상태(저장 파일 없음)에서 창을 열기만 하고 닫는다 | ⭐ `settings.json` 이 **생기지 않는다**(F-15 §8). 창을 여는 것만으로는 `ui.windowWidth` 도 생기지 않는다 |
| 9-b #2 | 창 모서리를 드래그해 크기를 바꾼다 | 드래그가 끝나고 약 0.4초 뒤 `settings.json` 의 `ui.windowWidth`·`ui.windowHeight` 가 새 값이 된다. ⭐ **드래그 중에 파일이 수십 번 쓰이지 않는다** — 값이 한 번만 갱신된다 |
| 9-b #3 | 앱을 종료하고 다시 띄운 뒤 창을 연다 | ⭐ **바꿔 둔 크기 그대로** 열린다 |
| 9-b #4 | ⛔ 종료 경로가 실행되지 않는 방식으로 죽인다(`kill -9`) — 크기를 바꾸고 1초 뒤 | 그래도 크기가 살아 있다(F-15 §3.1.1 결정 1 — 종료 시점 flush 에 기대지 않는다) |

### 📌 실제 통과 결과 (2026-08-30, 이슈 #32) — **전부 통과**

`scripts/build-signed.sh` 로 만든 `.app` 을 `open` 으로 띄워 확인했다. 탭 클릭과 창 크기 조회는 `System Events`(AX)로 했다 — 손으로 클릭한 것과 같은 경로이고, 창 크기를 **숫자로** 기록에 남길 수 있어서다.

| # | 결과 | 증거 |
| :--- | :--- | :--- |
| 9-a #1 | ✅ | 사이드바 6개 탭을 차례로 클릭하며 매번 `size of window 1` 을 읽었다 — `tab0..tab5` 전부 **825x821**. `ui.lastTab` 이 `general`(마지막으로 누른 탭)로 바뀐 것으로 클릭이 실제로 탭을 바꿨음을 확인했다 |
| 9-a #2 | ✅ | 탭 전환 구간 로그에 `resize_settings` **0건**(`grep -c` = 0) |
| 9-b #1 | ✅ | 저장 파일을 지우고 앱을 띄운 뒤 창을 열었다 — `ui.*` 키가 **하나도 생기지 않았다**. 창은 저장값이 없을 때의 기본 크기 **825x821** 로 열렸다 |
| 9-b #2 | ✅ | 50 ms 간격으로 10번 크기를 바꿨다(드래그 흉내). 직후에는 `ui.window*` 키가 **없었고**, 디바운스(400 ms) 뒤 **마지막 값 하나만** 기록됐다 — `ui.windowWidth: 740`, `ui.windowHeight: 630`. 중간값은 하나도 파일에 남지 않았다 |
| 9-b #3·#4 | ✅ | `kill -9` 로 종료 경로를 건너뛰고 죽인 뒤 다시 띄웠다 — 창이 **740 x 630** 으로 열렸다. 기동 로그에 복원 호출(`what="resize_settings"`)이 정확히 한 번 있다 |

⬜ **하지 않은 것**: 마우스로 창 모서리를 **물리적으로 드래그**하지는 않았다. AX 로 크기를 바꾸는 경로도 `WindowEvent::Resized` 를 같은 방식으로 발생시키지만, 물리 드래그가 만드는 이벤트 밀도(초당 수십 건)를 그대로 재현했다고는 말할 수 없다 `(추정 — 10회/0.05초 간격으로 근사했다)`.

### 🐛 이 검증이 잡은 것 — ⚠️ **이 이슈 범위 밖, F-17 소관**

저장 파일을 지우고 앱을 띄우면 **환경설정 창을 한 번도 열지 않았는데도** `settings.json` 이 만들어진다:

```json
{ "schemaVersion": 1, "values": { "perDevice._managed": {} } }
```

F-15 §8 의 "설정을 한 번도 건드리지 않으면 저장 파일 자체가 생기지 않는다"가 **기동만으로 이미 깨져 있다.** 쓰는 주체는 F-17 경로 B 원장(`perDevice._managed`)이고 창 크기 저장과는 무관하다(위 9-b #1 이 보여주듯 `ui.*` 키는 하나도 생기지 않는다). ⛔ 이슈 #32 범위 밖이라 여기서 고치지 않는다 — F-17 후속(이슈 #31) 소관이다.

## 항목 8 — 앱 아이콘 (이슈 #16)

아이콘 교체는 자동 테스트로 "보인다"를 증명할 수 없다. `frontend_wiring.rs` 가 지키는 것은
**정합**(정본 SVG ↔ 온보딩 인라인 사본, `setup_tray` 가 template 자산을 쓰는지,
`bundle.icon` 이 실재 파일을 가리키는지)까지이고, 실제로 화면에 그려진 모양은 눈으로 봐야 한다.
설계 근거와 재생성 절차는 [`icons.md`](icons.md).

| # | 절차 | 기대 |
| :--- | :--- | :--- |
| 1 | `./scripts/generate-icons.sh` → `./scripts/build-signed.sh` → `open …/Ultrakey.app` | 번들 `Contents/Resources/icon.icns` 가 `apps/ultrakey-app/icons/icon.icns` 와 같은 해시다 |
| 2 | Finder 에서 `Ultrakey.app` 을 아이콘 뷰로 본다 · `Get Info` 를 연다 | 새 아이콘이 보인다. 옛 아이콘이 보이면 캐시 문제다 → [`icons.md`](icons.md) §4 |
| 3 | 메뉴바 아이콘을 본다 | 배경 없는 선화 실루엣이다. **둥근 사각형 덩어리로 보이면 template 이 아니라 앱 타일을 쓰고 있는 것이다** |
| 4 | 권한 미부여 상태로 온보딩 모달을 띄운다 | 제목 위에 아이콘이 있고, 라이트에서 검은 선화 / 다크에서 흰 선화다 |

### 📌 실측 결과 (2026-08-30, 이슈 #16 / `feat/app-icon`)

#### ⚠️ 관찰 방법과 그 한계 — 먼저 읽어라

- **Dock 아이콘은 이 앱에 존재하지 않는다.** `set_activation_policy(ActivationPolicy::Accessory)`
  로 상주 에이전트 앱이기 때문이다(F-10 §3). 그래서 번들 아이콘 증거는 **Finder 아이콘 뷰와
  `Get Info`** 로 남겼다 — 같은 `.icns` 를 같은 Launch Services 경로로 그리는 자리다.
- ⭐ **온보딩 모달은 Accessibility 권한이 없어야 뜬다.** 이 기기에는 이미 권한이 부여돼 있어
  그대로는 뜨지 않는다. **`tccutil reset` 으로 권한을 회수하지 않았다** — 재부여 실패 시 개발
  환경이 망가지는 위험 대비 얻는 것이 작다(§7-c 와 같은 판단). 대신 **서명된 `.app` 을
  스크래치패드에 복사해 번들 ID 만 `app.ultrakey.IconShot` 으로 바꿔 재서명**하고 그것을 띄웠다.
  권한이 없는 신원이라 온보딩 경로가 그대로 실행된다. **저장소의 번들 ID 와 실제 앱의 TCC
  기록은 건드리지 않았다**(확인: 원본 `Info.plist` 의 `CFBundleIdentifier` 는 그대로
  `app.ultrakey.Ultrakey`). 촬영 후 사본은 종료·삭제했다.
- ⚠️ **이 기기의 두 디스플레이는 모두 1x(비Retina)** 다(3840×1600 · 2560×1440, `UI Looks like`
  가 해상도와 같다). 즉 메뉴바 아이콘은 36px 자산을 18pt 로 **축소**해 그린 결과를 본 것이다.
  Retina 에서의 1:1(36px) 렌더는 **확인하지 못했다.**
- ⚠️ **메뉴바의 라이트/다크 대비는 확인하지 못했다.** 이 기기의 배경화면이 어두워 시스템 외관을
  라이트로 두어도 메뉴바가 어둡게 합성된다 — 두 외관 모두에서 **흰 실루엣**만 관찰됐다.
  검은 실루엣(밝은 메뉴바) 경우는 관찰되지 않았다.

#### ✅ 통과한 것

| # | 결과 |
| :--- | :--- |
| 1 | ✅ 빌드·서명 통과. 번들 ID `app.ultrakey.Ultrakey` 유지, universal(x86_64+arm64). 번들 안 `Contents/Resources/icon.icns` 와 저장소의 `icons/icon.icns` 가 **같은 SHA-1**(`684dcce3…`) — 번들에 실제로 새 아이콘이 들어갔다 |
| 2 | ✅ Finder 아이콘 뷰와 `Get Info` 모두 새 아이콘. **캐시 조작을 전혀 하지 않고** 바로 갱신됐다(`lsregister`·`killall` 미실행) |
| 3 | ✅ 메뉴바에 배경 없는 선화 실루엣. 이전(앱 타일 재사용) 의 사각 덩어리가 아니다 |
| 4 | ✅ 온보딩 모달 제목 위에 아이콘. **라이트에서 검은 선화, 다크에서 흰 선화**로 `currentColor` 가 외관을 따라간다 |

#### 증거 스크린샷

| 파일 | 무엇 |
| :--- | :--- |
| [`screenshots/issue-16-app-icon-getinfo.png`](screenshots/issue-16-app-icon-getinfo.png) | `Get Info` — 제목줄의 16pt 아이콘과 Preview 의 대형 아이콘(#2) |
| [`screenshots/issue-16-app-icon-finder.png`](screenshots/issue-16-app-icon-finder.png) | Finder 아이콘 뷰의 `Ultrakey.app`(#2) |
| [`screenshots/issue-16-menubar-icon.png`](screenshots/issue-16-menubar-icon.png) | 메뉴바 — 원본 배율과 6배 확대(#3) |
| [`screenshots/issue-16-onboarding-light-dark.png`](screenshots/issue-16-onboarding-light-dark.png) | 온보딩 모달 라이트/다크 나란히(#4) |
| [`screenshots/issue-16-icon-sizes.png`](screenshots/issue-16-icon-sizes.png) | 저장소에 커밋된 512·256·128·64·32px 자산을 **원본 배율로** 나란히 — 크기별 선 굵기 조정의 결과 |

#### ⬜ 수행하지 못한 것 — 통과했다고 적지 않는다

| 항목 | 이유 |
| :--- | :--- |
| **Retina(2x) 메뉴바 렌더** | 이 기기의 디스플레이가 둘 다 1x 다. 36px 자산이 18pt 에 1:1 로 떨어지는 것은 계산으로만 맞췄다 |
| **밝은 메뉴바에서의 검은 실루엣** | 배경화면이 어두워 두 외관 모두 메뉴바가 어둡게 합성됐다 |
| **아이콘 캐시 강제 갱신 경로** | 캐시 문제가 **발생하지 않아서** `lsregister -f`·`killall Finder` 를 실행할 일이 없었다. [`icons.md`](icons.md) §4 의 절차는 **미검증**이다 |
| **Dock 아이콘** | 이 앱에는 존재하지 않는다(위 참조) |
| **`.icns` 안의 16pt 표현** | `Get Info` 제목줄과 Finder 목록에서 작게 보이는 것까지는 확인했으나, 각 표현(`icon_16x16` 등)을 하나씩 뜯어 본 것은 아니다 |

#### 검증 후 되돌린 것

- 시스템 외관을 다크로 **잠시** 바꿨다가 **라이트로 복구**했다(복구 확인: `dark mode = false`).
- 온보딩 촬영용 사본(`IconShot.app`, 번들 ID `app.ultrakey.IconShot`)을 **종료·삭제**했다.
  이 사본은 권한을 요구하지 않으므로 TCC 목록에 항목을 남기지 않는다.
- 촬영을 위해 잠시 종료했던 **기존 인스턴스(메인 체크아웃 빌드)가 다시 떠 있는 것을 확인**했다.
- ⛔ **키체인·TCC·Launch Services 데이터베이스는 건드리지 않았다.**

---

## 📌 실측 결과 (2026-08-30, 이슈 #19 — 물리 caps lock 3증상)

> 대상: `fix/physical-caps-lock-events`. 사용자가 Karabiner 를 끄고 물리 caps lock 을 직접
> 눌러 보고한 세 증상(A 리매핑 down/up 오판정 · B quick press 미동작 · C double tap shift
> 미동작)의 원인 확정과 수정 검증.

### ⚠️ 관찰 방법과 그 한계 — 먼저 읽는다

이번 회차는 **사람이 물리 키를 누르지 못하는 조건**에서 수행했다. 그래서 두 층으로 나눠
검증하고, **각각이 무엇을 증명하고 무엇을 증명하지 못하는지** 분명히 적는다.

| 층 | 수단 | 증명하는 것 | ⛔ 증명하지 **못**하는 것 |
| :--- | :--- | :--- | :--- |
| 커널 매핑(경로 B) | `hidutil property --get UserKeyMapping` + 기동 로그 | `caps lock → F18` 매핑의 설치·제거가 설정과 일치한다 | 물리 키를 눌렀을 때 커널이 실제로 F18 을 낸다는 것 |
| 탭 이후 전 구간 | `examples/key_poke` 로 **F18 을 직접 주입** + `ULTRAKEY_TRACE_TAP=1` 계측 | 탭 도착 → alias 환원 → 중재 → 합성/방출 → 경로 C 까지 전부 | 위와 같음 — 주입은 HID 층 **위**에서 들어가므로 커널 매핑을 지나지 않는다 |

⭐ 이 분해 자체는 `examples/key_poke` 모듈 문서가 이미 정한 방법이다 — "`caps lock → F18`
매핑이 걸린 상태를 흉내 내려면 이 도구에 **F18 을 직접** 넣어야 한다(그것이 커널 매핑이
하는 일이다)."

⛔ **그러므로 "물리 caps lock 키를 실제로 눌러서 확인했다"고는 쓰지 않는다.** 그 한 단계는
아래 "⬜ 수행하지 못한 것" 에 남긴다.

⚠️ **Karabiner-Elements 는 이 회차 중에 켜졌다 꺼졌다 했다.** 조사 시작 시점에는 가상 HID
키보드가 등록돼 있었고(켜짐), 몇 분 뒤 사라졌다가(꺼짐), 검증 도중 다시 등록됐다.
⛔ **사용자의 Karabiner 설정은 읽기만 했고 바꾸지 않았다.** 주입 방식 검증은 Karabiner
계층을 지나지 않으므로 이 변동에 영향받지 않는다.

### ✅ 통과한 것 — 증상별 원인과 실측

**증상 B — 경로 B 커널 매핑이 엔진 설정과 어긋난 채 남는다.**

고장 상태를 이 기기에서 **그대로 발견**했다: `settings.json` 은 `synthesizeCapsLockRemap:
true`(= D-1 꺼짐, `caps_lock_alias = None`)인데 커널에는 `caps lock → F18` 이 남아 있었다.
계측이 그 결과를 문자 그대로 보여준다 — 물리 caps lock 이 되었을 F18 이 아무에게도
해석되지 않고 통과만 한다:

```
raw_kind=KeyDown raw_keycode=0x4F ... resolved=0x4F alias_active=false
layer=Passthrough disp=Pass emitted=[] effects=[]
```

원인 둘. (1) `Advanced ▸ Synthesize Caps Lock Remap` 메뉴 토글이 `SettingsStore` 에만 쓰고
`AppState::presets`·엔진을 갱신하지 않았다 — 이전 회차 로그가 증거다(`05:39:52` 토글
`value=true` → `05:40:49` 경로 B 재적용이 여전히 `count=1`). (2) `reconcile_on_start` 가
`desired` 가 비었을 때 잔존 매핑을 지우지 않았다(M1 의 "출처를 판별할 수 없다" 전제가
D-1 이후로는 거짓이다).

수정 후 기동 로그와 조회:

```
경로 B 시작 시 재조정 — 이번 설정은 매핑을 요구하지 않으므로
우리가 설치한 D-1 매핑만 제거했다(남의 매핑은 그대로 둔다) ours=1 foreign=0
$ hidutil property --get UserKeyMapping
( )
```

그리고 `Quick press caps lock to execute: caps lock`(F-08.2) 기능 자체:

```
seq=45 raw_kind=KeyUp raw_keycode=0x4F ... resolved=0x39 alias_active=true
layer=PresetCombo disp=Consume effects=[ToggleCapsLock] path_c=성공(전:off 후:on)
```

**증상 A — modifier 대상 리매핑을 `KeyDown`/`KeyUp` 으로 내보냈다.**

`Remap caps lock to: left control`, D-1 켬. F18 주입:

```
seq=409 KeyDown 0x4F → resolved=0x39 alias_active=true
        emitted=[FlagsChanged 0x3B 0x20840001]     ← control(0x40000) + 좌측 구분(0x1)
seq=410 KeyDown 0x50(F19) → disp=PassWithFlags disp_flags=0x20840001
seq=411 KeyUp   0x50      → disp=PassWithFlags disp_flags=0x20840001
seq=412 KeyUp   0x4F → emitted=[FlagsChanged 0x3B 0x20800000]   ← control 해제
```

수정 전이라면 `emitted=[KeyDown 0x3B 0x00000000]` 이고 `disp_flags` 는 `0x0` 이었다 —
control 비트가 한 번도 켜지지 않으므로 받는 앱은 눌림을 보지 못한다(사용자 보고의
"다운·업이 전부 업으로 잡힌다"). ⭐ `seq=410/411` 이 핵심이다: **유지 중 다른 키에도
비트가 실려야** `⌃C` 가 성립한다. 합성 `flagsChanged` 한 번만으로는 부족하다.

**증상 C — 보류한 원본을 끝내 내보내지 않아 shift 가 죽었다.**

`Double tap shift = caps lock` 만 켠 구성. shift 의 `flagsChanged` 주입:

```
seq=9  FlagsChanged 0x38 → disp=Pass        ← shift 가 통과한다(수정 전이면 Consume)
seq=15 FlagsChanged 0x38 → disp=Pass
seq=28 FlagsChanged 0x38 → disp=Pass effects=[ToggleCapsLock] path_c=성공(전:off 후:on)
seq=36 FlagsChanged 0x38 → disp=Pass effects=[ToggleCapsLock] path_c=성공(전:on 후:off)
```

`disp=Pass` 와 `effects=[ToggleCapsLock]` 이 **한 줄에 함께** 있는 것이 이 수정의 요점이다 —
프리셋은 제스처를 **더하는** 것이지 그 키를 **빼앗는** 것이 아니다.

**경로 C 자체** — `IOHIDGet/SetModifierLockState`(selector `1`)가 이 기기에서 정상 동작한다.
독립 확인: selector `0`·`4`~`8` 은 `kIOReturnUnsupported`(`0xE00002C2`)로 거부되고 `1`·`2`·`3`
만 받아들여진다. 위 `path_c=성공(전:off 후:on)` / `(전:on 후:off)` 가 양방향을 보여준다.

**계측이 잡아낸 명세대로의 동작 하나.** 검증 도중 quick press 가 발화하지 않은 회차가
있었는데, 계측을 보니 F18 의 down 과 up 사이에 **다른 키의 keyDown 이 끼어들어 있었다**
(사용자가 그때 키보드를 쓰고 있었다). §3-c 표 3행·P2 가 요구하는 대로 quick press 후보에서
정확히 빠진 것이다 — 결함이 아니다. **계측이 없었다면 이것을 "가끔 안 먹는다"로 오진했을
것이다.**

### ⬜ 수행하지 못한 것 — 그대로 적는다

- ⛔ **물리 caps lock·shift 키를 사람이 실제로 눌러 확인하지 못했다.** 이 회차를 수행한
  주체가 물리 키를 누를 수 없다. 주입(`key_poke`)은 HID 층 **위**로 들어가므로
  **"물리 caps lock 을 누르면 커널이 F18 을 낸다"는 한 단계만은 검증되지 않았다.**
  나머지 전 구간(탭 도착 이후)은 위 계측으로 검증했다.
- ⛔ **브라우저 프로브(`tools/modifier-probe.html`)로 "다른 앱이 받은 이벤트"를 보지
  못했다.** 브라우저 자동화 확장이 연결돼 있지 않았다. 대신 `disp=PassWithFlags
  disp_flags=0x20840001`(seq=410/411)로 **탭이 다음 앱에 넘기는 flags** 를 직접 확인했다.
- ⬜ 부록 A-bis #5(사용자가 직접 건 **다른** 매핑을 보존하는가)는 **자동 테스트로만**
  확인했다(`path_b.rs` 의 `cleanup_removes_only_our_d1_mapping_and_keeps_foreign_ones` ·
  `reconcile_with_desired_preserves_foreign_mappings`). 사용자 시스템 전역 상태를 실제로
  더럽히지 않기 위해 실기기에서는 수행하지 않았다.
- ⬜ 6-6(Secure Input 구간에서 경로 B 가 어떻게 동작하는가)은 이번에도 확인하지 않았다 —
  `power-user-presets.md` §9 #10 의 `(미확정)` 이 그대로 남는다.

### 검증 후 되돌린 것

| 무엇 | 어떻게 |
| :--- | :--- |
| `settings.json` | 시작 전 원본을 복사해 두고, 검증 뒤 **바이트 그대로 복원**했다(`capsLockRemap=true` · `capsQuickPress=true` · `doubleTapShiftToCaps=false` · `synthesizeCapsLockRemap=true`) |
| `hidutil` 커널 매핑 | 복원한 설정(`synthesize=true` → alias 없음)에 맞춰 앱이 스스로 비웠다 — 최종 `( )`. ⚠️ **검증 시작 시점에 남아 있던 잔존 `caps lock → F18` 은 일부러 되돌리지 않았다.** 그것이 이번에 고친 결함 그 자체이고, 되살리면 물리 caps lock 이 다시 죽는다 |
| caps lock 잠금 상태 | 검증 중 경로 C 로 토글했으나 **꺼짐(원래 상태)으로 되돌려 놓았다** |
| 실행 중이던 Ultrakey | 검증 조건(잔존 매핑 보존)을 위해 `kill -9` 로 종료했다. 지금은 **이 브랜치의 서명 빌드**가 대신 실행 중이다 |
| Karabiner-Elements | ⛔ **읽기만 했다. 아무것도 바꾸지 않았다** |
| 키체인·시스템 설정·Accessibility | ⛔ **건드리지 않았다** (기존 서명 주체가 같아 권한이 그대로 승계됐다) |

---

## 항목 7 — F-16 한국어 입력 지원 1·2단계 (M2 3차 / 이슈 #25 신설)

> 근거 명세: `../spec/korean-input.md` §8 · §3.3 · §3.5

### 7-0. 사전 확인 — 이 항목만의 전제 3가지

```sh
# 1) ⭐ 한국어 입력기가 설치돼 있는가 — 없으면 F-16.4 를 확인할 수 없다.
#    ⛔ 없다고 해서 시스템 설정에서 임의로 추가하지 마라. 확인 못 했다고 적고 넘긴다.
swift docs/dev/tools/tis-language-probe.swift korean

# 2) ⭐ "입력 소스 선택" 단축키가 ⌃Space 인가 — F-16.1 의 전제다(명세 §5 #4).
#    parameters = (32, 49, 262144) 이면 space(49) + control(0x40000) 이다.
/usr/libexec/PlistBuddy -c "Print :AppleSymbolicHotKeys:60" \
  ~/Library/Preferences/com.apple.symbolichotkeys.plist

# 3) HID 계층 리매퍼 — 항목 2·3 사전 확인과 같다. Karabiner 가 켜져 있으면
#    명세 §3.6 표대로 F-16 은 **전부 무동작**한다(그것이 정상이다).
ioreg -c IOHIDDevice -r -d1 | grep -c "Karabiner DriverKit VirtualHIDKeyboard"
```

⚠️ **다른 Ultrakey 인스턴스가 떠 있으면 새 빌드가 즉시 종료된다** — 번들 ID 단일
인스턴스 가드(`menu-bar-and-lifecycle.md` §5 항목 1) 때문이다. 먼저 확인하고 종료한다.

```sh
pgrep -lf 'Ultrakey.app/Contents/MacOS'
osascript -e 'tell application id "app.ultrakey.Ultrakey" to quit'
```

⛔ **설정 파일을 손대기 전에 반드시 백업한다.** 이 항목은 `korean.*` 설정을 켜야
하는데, 그 파일에는 사용자의 실제 설정이 들어 있다.

```sh
D=~/Library/Application\ Support/app.ultrakey.Ultrakey
cp "$D/settings.json" /tmp/settings.json.backup
# … 검증 후 …
cp /tmp/settings.json.backup "$D/settings.json"
shasum "$D/settings.json" /tmp/settings.json.backup   # ⭐ 두 해시가 같아야 한다
```

### ⭐ 7-1. F-14 선행 결함이 실제로 고쳐졌는가 — **로그 한 줄로 판정된다**

이것을 **가장 먼저** 확인한다. 여기가 틀리면 아래 전부가 의미 없다(명세 §3.3).

```sh
open --env ULTRAKEY_TRACE_TAP=1 -n target/universal-apple-darwin/release/bundle/macos/Ultrakey.app
grep '입력 소스 갱신' ~/Library/Logs/Ultrakey/ultrakey.log
```

**한국어 입력기를 켠 상태**에서 다음 모양이 나와야 한다:

```
used_ascii_fallback=true
source_id="com.apple.keylayout.ABC"                          ← 폴백 교체 *이후* (기존 필드)
original_source_id="com.apple.inputmethod.Korean.2SetKorean" ← 교체 *이전* 원본 (신규 필드)
first_language="ko"  korean_ime=Active
```

⭐ **`source_id` 와 `original_source_id` 가 서로 다르다는 것이 곧 결함과 그 수정의
증거다.** F-16 이 `source_id` 로 판정했다면 `ABC` 를 보고 항상 거짓을 냈을 것이다.

⭐ **이 줄이 앱 기동 직후에 나오는지도 함께 본다.** 입력 소스를 바꾸지 않았는데도
나와야 한다 — 기존 구현은 **알림이 올 때만** 재구축해서, 기동 직후에는 게이트가
`Unknown` 인 채로 fail-closed 되어 F-16.4 가 영영 발화하지 않았다.

#### ✅ 실측 결과 (2026-08-30, 이슈 #25 / PR) — **통과**

위 모양 그대로 관찰됐다. 기동 직후 1회 게시도 확인했다(로그 시각이 `엔진 시작됨` 보다 앞선다).

### 7-2. 규칙 발화 — ⭐ 탭 계측으로 판정한다

⭐ **사람이 키를 누르지 않고도 확인할 수 있다.** `key_poke` 가 마커 없는 이벤트를
합성해 세션에 넣으면 경로 A 가 물리 입력과 구분하지 않고 받는다(그 도구의 한계는
`crates/ultrakey-platform/examples/key_poke.rs` 모듈 문서 참고).

```sh
cargo build -p ultrakey-platform --example key_poke
# ⭐ modifier 는 반드시 `fc:`(flagsChanged)로 만든다 — KeyDown 으로 흉내 내면
#    M1 이 안고 머지됐던 바로 그 구멍을 재현한다.
./target/debug/examples/key_poke tap:0x32 sleep:300                                  # ① ₩
./target/debug/examples/key_poke fc:0x37:0x100008 sleep:80 tap:0x32 sleep:80 fc:0x37:0x0  # ② ⌘+`
./target/debug/examples/key_poke fc:0x38:0x20002 sleep:80 tap:0x31 sleep:80 fc:0x38:0x0   # ③ ⇧+space
tail -f ~/Library/Logs/Ultrakey/ultrakey.log | grep '탭 계측'
```

| # | 입력 | 기대 `layer` / `disp` / `emitted` |
| :--- | :--- | :--- |
| ① | `KeyDown 0x32` (한국어 IME 활성, modifier 없음) | `KoreanInput` / `Consume` / `KeyDown 0x32 0x80020` |
| ② | `KeyDown 0x32` + command | `Passthrough` / `Pass` / `[]` — ⭐ **개입하지 않는다** |
| ③ | `KeyDown 0x31` + shift | `KoreanInput` / `Consume` / `KeyDown 0x31 0x40001` — ⭐ **shift 가 빠지고 control 만 남는다** |
| ④ | `KeyDown 0x32` (**영문** 입력기) | `Passthrough` / `Pass` / `[]` |

`emitted` 의 flags 는 macOS 헤더 값이다: `0x80020` = `kCGEventFlagMaskAlternate`(0x80000) |
`NX_DEVICELALTKEYMASK`(0x20), `0x40001` = `kCGEventFlagMaskControl`(0x40000) |
`NX_DEVICELCTLKEYMASK`(0x1).

⭐ **keyUp 도 함께 본다** — ①·③ 은 `KeyUp` 도 같은 keycode/flags 로 치환되어야 한다
(명세 §3.3·§5 #9 의 "치환 중" 래치). 짝이 맞지 않으면 modifier 가 걸린 채 남는다.

#### ✅ 실측 결과 (2026-08-30) — **①②③④ 전부 통과**

keyUp 치환(래치)도 ①·③ 양쪽에서 확인했다.

### ⭐ 7-3. 실제로 무슨 글자가 나오는가 — **대조군이 있어야 증명된다**

7-2 는 "우리가 무엇을 내보냈는가" 까지만 증명한다. **받는 앱에 실제로 무엇이
찍히는가**는 별개다 — 명세 §5 #5·§9 #2 가 `⌥`+grave 가 정말 백틱인지를 `(추정)` 으로
남겨 두었다.

```sh
osascript -e 'tell application "TextEdit" to activate' \
          -e 'tell application "TextEdit" to set text of document 1 to ""'
./target/debug/examples/key_poke tap:0x32 sleep:600
osascript -e 'tell application "TextEdit" to get text of document 1' | od -c
```

| # | 조건 | 기대 결과 |
| :--- | :--- | :--- |
| A | 한국어 IME + 기능 **ON** + modifier 없음 | `` ` `` (U+0060) |
| B | 한국어 IME + 기능 **ON** + `⇧` | `~` — 개입하지 않는다 |
| C | ⭐ **대조군**: 한국어 IME + 기능 **OFF** + modifier 없음 | `₩` (U+20A9) |

⛔ **C 를 건너뛰지 마라.** C 가 없으면 A 의 백틱이 우리 치환의 결과인지, 아니면 그
입력기가 원래 백틱을 내는지 구분할 수 없다 — 그것은 검증이 아니라 관찰일 뿐이다.

#### ✅ 실측 결과 (2026-08-30) — **A `` ` `` · B `~` · C `₩`, 전부 기대대로**

⭐ 이로써 명세 §9 #2 가 `(추정)` 으로 남겼던 "한국어 입력 소스에서 `⌥`+grave 가
백틱인가" 가 **실측으로 해소됐다.**

### 7-4. `Shift + Space` 가 실제로 입력 소스를 바꾸는가

7-2 ③ 이 `⌃Space` 를 내보낸 것까지는 증명했다. macOS 가 그것을 **단축키로 인식했는지**는
입력 소스가 실제로 바뀌었는지로 판정한다 — 로그가 그것을 보여준다.

```sh
grep '입력 소스 갱신' ~/Library/Logs/Ultrakey/ultrakey.log
```

③ 직후에 새 줄이 나오고 `original_source_id`·`korean_ime` 가 뒤집혀야 한다.
⭐ **양방향으로 확인한다** — 한 번 더 보내 원래 입력 소스로 돌아오는지까지 본다.

#### ✅ 실측 결과 (2026-08-30) — **통과, 양방향**

`Korean.2SetKorean`(`korean_ime=Active`) ↔ `keylayout.ABC`(`Inactive`) 전환을 확인했다.
게이트가 전환마다 다시 게시되는 것도 같은 로그로 확인된다.

### 7-5. `Korean` 탭 UI

⭐ **탭이 실제로 그려졌는지는 `ui.lastTab` 로 확인할 수 있다.** `settings.json` 의
`ui.lastTab` 을 `"korean"` 으로 두고 앱을 띄우면, 프런트엔드가 부팅 중
`activateTab("korean")` → `settings_set_tab` 을 부른다. 탭이 `TABS` 배열이나 DOM 에
없으면 그 자리에서 예외가 나 `showFatal` 로 빠지고 **리사이즈 로그가 아예 남지 않는다.**

```sh
grep -E 'settings_bootstrap|resize_settings|알 수 없는 탭' ~/Library/Logs/Ultrakey/ultrakey.log
```

⚠️ **`창 조작 후 상태` 의 `outer_size` 는 판정에 쓰지 마라** — 그 값은 리사이즈가
반영되기 전 상태를 찍는다(`ui.lastTab=seek` 로 띄워도 710×517 이 찍힌다). 판정은
"`resize_settings` 줄이 남았는가" 로 한다.

**눈으로 볼 것**(메뉴바 → `설정…`):

- [ ] 탭 순서가 `Seek · Hyperkey · Presets · **Korean** · General` 이다
- [ ] ⭐ **정정(2단계).** `한/영`·`한자` 두 항목이 **활성이고(dimmed 아님) 배지가 없으며**,
      각각 부제(`.hint`)가 붙어 있다 — 1단계 때의 "dimmed + `아직` 배지 + 사유 문구"는
      §3.2 의 키코드가 근거 3중으로 해소되면서 걷혔다(D-K14). ⛔ **"실기기 미검증"
      배지를 UI 에서 찾지 마라 — 두지 않기로 결정했다**(명세 §4.2 의 ⭐, D-K16):
      증거 등급은 개발자용 메타 정보이지 사용자가 조작할 수 있는 사실이 아니다.
      부제가 말해야 하는 것은 "이 키가 있는 키보드에서만 동작한다" 쪽이다
- [ ] `원격 데스크톱 …` 항목이 **기본으로 켜져 있다** — F-16 중 유일하게 기본 ☑ 다
- [ ] 내용이 창에 다 들어간다(잘리지 않는다)

#### ✅ 실측 결과 (2026-08-30) — **통과. 단, 관찰 경로에 한계가 있다**

`ui.lastTab="korean"` 부팅에서 `settings_bootstrap` → `resize_settings` 가 오류 없이
남았다 — 탭 버튼·패널·`settings_set_tab("korean")` 이 전부 배선되어 있다는 뜻이다.

⚠️ **환경설정 창 자체를 앱 안에서 눈으로 보지는 못했다** — 그 창은 메뉴바에서만 열리고,
자동으로 열려면 System Events 자동화 권한이 필요해 **TCC 상태를 바꾸게 된다**(금지).
대신 `ui/settings.html` 을 브라우저에서 같은 부트스트랩 페이로드로 렌더해 위 4개 항목을
전부 눈으로 확인했다. **같은 마크업·같은 CSS 지만 앱의 웹뷰는 아니다** — 이 구분을
지운 채 "확인했다" 고 적지 않는다.

⭐ 그 렌더로 **창 크기를 실측했다**: 내용 416px + `.panel` 상하 패딩 40px + 타이틀바
28px = **613 × 484**. 명세 §9 #5 가 남긴 자리를 이 값으로 채웠다.

### ⛔ 7-6. 이 항목으로 확인할 수 **없는** 것

- **2단계(한/영 `0x68` · 한자 `0x66`)** — ⭐ **한국어 106키 물리 키보드가 없다.**
  keycode 는 근거 3중으로 확정됐지만(명세 §3.2) 실기기 확인은 불가능하다.
  키보드가 확보되면 명세 §3.2 "해소 이전의 기록" 의 절차 1~6 을 수행한다.
  ⚠️ `key_poke` 로 `0x68`/`0x66` 을 합성하면 **우리 쪽 경로**(탭 → 규칙 → 방출)는
  확인되지만, **실제 하드웨어가 그 keycode 를 보내는가**는 확인되지 않는다 — 이 둘을
  구분해 적어라.
- **원격 데스크톱 제외 게이트의 실제 앱 동작** — 목록의 클라이언트가 이 기기에 설치돼
  있지 않다(`com.apple.ScreenSharing` 만 있다). 게이트 판정 자체는 단위 테스트가 덮는다.
- **서드파티 한국어 입력기**(구름 입력기 등)의 `languages[0]` — 설치돼 있지 않다.
- **IME 조합 중 개입의 영향**(명세 §5 #8·§9 #9) — 자음만 입력한 조합 중 상태에서
  `` ` `` 를 눌러 조합이 깨지는지는 사람이 직접 타이핑해야 관찰할 수 있다.

---

## 결과 기록

각 항목을 수행한 뒤 **통과 여부와 관찰한 것**을 이슈 #5 또는 후속 이슈에 남긴다.
⭐ 특히 명세가 `(미확정)` 으로 남긴 값들 — 워치독 주기, 절전 복귀 지연, 재시작 디바운스 임계값, double tap 간격, Secure Input 확인 오버헤드 — 은 **이 절차가 그것을 실측으로 바꿀 수 있는 첫 기회**다([`architecture.md`](architecture.md) §4 표의 "설계 판단" 행 전부). 관찰값이 나오면 명세의 9절과 `architecture.md` §4 를 함께 갱신한다.

---

## 부록 C — F-17 키보드별 설정 (이슈 #28)

> 근거 명세: `../spec/per-device-settings.md` §8 수용 기준 · §3.6 규칙 1~9
> 설계 결정: `architecture.md` §7(D-17-1~6)
> 사실 토대: `../research/per-device-hid-spike.md`

### ⛔ C-0. 먼저 — 이 검증이 지켜야 할 안전 절차

⭐ **스파이크(§부록 A)가 확립한 절차를 그대로 따른다.** 되읽기 성공은 동작 확인이 아니다(S-7).

① 실험 전 baseline 기록 → ② 설치 → ③ **설치 직후 되읽기** → ④ 키 입력 → ⑤ **직후 되읽기로 생존 확인** → ⑥ 복원 → ⑦ 복원 검증

```sh
# ① baseline — 반드시 먼저. 전역과 디바이스별을 모두 기록한다.
hidutil property --get UserKeyMapping
hidutil list --matching '{"PrimaryUsagePage":1,"PrimaryUsage":6}'
for d in '{"VendorID":<vid>,"ProductID":<pid>}'; do
  hidutil property --matching "$d" --get UserKeyMapping
done
```

⚠️ **다른 Ultrakey 인스턴스가 돌고 있지 않은지 반드시 확인한다.**

```sh
ps ax | grep ultrakey-app | grep -v grep
```

⭐ **스파이크를 두 번 오염시킨 원인이 바로 이것이다** — 다른 워크트리의 빌드가 D-1 을
**전역으로** 설치해 디바이스별 배열을 통째로 갈아치웠다(스파이크 §6, 부록 A). 검증 중에는
**정확히 하나의 Ultrakey 만** 살아 있어야 한다(§3.6 규칙 1 — 단일 기록자).

⛔ **사용자의 Karabiner 설정을 읽지도 바꾸지도 않는다.**

### C-1. 디바이스 열거 (§3.2, S-2)

```sh
cargo run -p ultrakey-platform --example keyboard_list_probe
hidutil list --matching '{"PrimaryUsagePage":1,"PrimaryUsage":6}'   # 대조군
```

| # | 기대 |
| :--- | :--- |
| 1 | 프로브가 돌려주는 키보드 대수 = `hidutil list` 의 **물리 디바이스** 대수 |
| 2 | ⭐ **`(VID,PID)` 중복이 없다** — `hidutil list` 는 같은 키보드를 서비스 수만큼 여러 줄로 보여주지만(S-2), 팝업에는 1대당 한 항목만 떠야 한다 |
| 3 | VID·PID·제품명이 `hidutil list` 와 일치한다 |

### C-2. ⭐ 디바이스 격리 — 이 기능의 핵심 (§8 수용 기준 2, S-1 재현)

**이것이 통과하지 못하면 F-17 은 실패다.** S-3 의 함정(VID 단독 매칭 → `IOHIDSystem` 오염 →
사실상 전역 쓰기)에 빠지지 않았는지가 여기서 판가름난다.

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | `Keyboards` 탭에서 **외장 키보드 A** 를 골라 눈에 띄는 변환 1행을 등록 | A 의 배열에만 그 매핑이 나타난다 |
| 2 | 같은 시점에 **디바이스 B** 조회 | ⛔ **B 의 배열에는 그 매핑이 없다** |
| 3 | 같은 시점에 **전역** 조회(`--matching` 없이) | ⛔ 그 매핑이 전역에 **없다** |
| 4 | ⭐ **키를 실제로 눌러 본다** — A 에서 그 키, 그리고 B(또는 다른 입력 장치)에서 같은 키 | A 에서만 바뀐 동작이 나고 **B 는 영향받지 않는다** |
| 5 | 되읽기로 매핑 생존 확인(④ 직후) | 매핑이 그대로 있다 — 없으면 4의 관측은 **무효**다(부록 A 의 교훈) |

### C-3. D-1 과의 공존 (§8 수용 기준 3, §3.6 규칙 2·4)

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | caps lock 의존 프리셋을 하나 켠 상태에서 디바이스 A 에 기능 1/2 설정 | A 의 배열에 `caps lock → F18`(D-1) **과** 기능 1/2 매핑이 **함께** 있다 |
| 2 | 붙어 있는 **모든** 키보드 조회 | D-1 이 **모든** 키보드에 있다(D-17-1 — 경로 A 가 디바이스를 구분하지 못하므로) |
| 3 | 전역(`--matching` 없이) 조회 | ⭐ 전역 배열에 우리 D-1 이 **없다**(D-17-5 이관이 걷어냈다) |

### C-4. 잔존 정리·재조정 (§8 수용 기준 10, §3.6 규칙 6)

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | 앱 정상 종료 후 조회 | 우리 매핑이 없다. ⛔ 남의 매핑은 그대로 |
| 2 | `kill -9` 후 재실행 | 원장에 있던 디바이스의 배열이 현재 설정 기준으로 재조정된다(잔존물을 신뢰하지 않고 덮어쓴다) |
| 3 | 사용자가 직접 건 매핑이 있는 상태에서 1·2 반복 | ⛔ **그 매핑은 남아 있다**(PR #23 소유권 모델) |
| 4 | 디바이스를 뽑았다 다시 꽂는다 | `(연결 안 됨)` 배지 → 재연결 시 배열 자동 재적용(§5 항목 1, 규칙 8) |

### C-5. 기능 2 — Consumer Page 실동작 (§8 수용 기준 6, S-8 재현)

⛔ **되읽기 성공만으로 판정하지 않는다. 키를 눌러 확인한다.**

| # | 조작 | 기대 |
| :--- | :--- | :--- |
| 1 | 어떤 F-키에 `Volume Up` 을 지정 | 그 키를 누르면 **실제로 볼륨이 올라간다**(S-8 이 `0xC000000E9` 로 확인한 값) |
| 2 | ⭐ 대조군 — 같은 배열에 Keyboard Page 매핑도 함께 둔다 | 대조군도 동작해야 한다. 그래야 "아무 일 없음" 이 *표현 불가* 때문인지 *매핑 소멸*(S-6) 때문인지 구분된다 |
| 3 | ①② 직후 되읽기 | 매핑이 살아 있었음을 확인 — 없으면 관측 무효 |

⚠️ **전제**: macOS `Use F1, F2, etc. keys as standard function keys` 가 **켜져 있어야** 한다
(§3.5). 꺼져 있을 때의 동작은 `(미확정)` 이며 보증하지 않는다.

### C-6. ⭐ 복원 (⑥⑦)

```sh
# 검증용으로 넣은 매핑을 전부 걷어낸 뒤, baseline 과 글자 그대로 일치하는지 대조한다.
hidutil property --get UserKeyMapping
for d in …; do hidutil property --matching "$d" --get UserKeyMapping; done
```

⛔ **baseline 과 다르면 복원이 끝난 것이 아니다.** 검증 시작 시점에 돌고 있던 Ultrakey
인스턴스가 있었다면 **그것도 원래대로 되돌린다**(다시 띄운다).

### ✅ 실측 결과 (2026-08-30, 이슈 #28 / 브랜치 `feat/per-device-settings`)

**환경**: `Mac16,11` · macOS 26.5.2(빌드 25F84) · 서명 빌드를 `open` 으로 실행.
**대상 디바이스**: A = `F108Pro Dongle`(`0x5ac:0x24f`) · B = `Wireless mouse 8k dongle-L`(`0x373b:0x11d9`).
두 VID/PID 는 이슈 #21·스파이크에 이미 공개된 범위다.

⚠️ **검증 시작 시점에 다른 워크트리(메인 체크아웃)의 Ultrakey 가 돌고 있었다** — 스파이크를
오염시킨 것과 같은 조건이다. C-0 절차대로 **먼저 종료**시키고 단일 기록자를 확보한 뒤 진행했다.

#### ⭐ C-2 디바이스 격리 — **통과**. 이 기능의 핵심 판정이다

디바이스 A 에만 `keyRemap.rows = [F13→F15]` 와 `functionKeys.f9 = VolumeUp` 을 설정한 결과:

| 대상 | 배열 | 판정 |
| :--- | :--- | :--- |
| **디바이스 A** (서비스 3개 **전부**) | `caps_lock→F18`(D-1) · `F13(0x700000068)→F15(0x70000006A)`(기능 1) · `F9(0x700000042)→0xC000000E9`(기능 2, Consumer Page) | ⭕ 3개가 **한 배열에 공존**(§3.6 규칙 4) |
| **디바이스 B** | `caps_lock→F18` **만** | ⭕ A 의 설정이 **전혀 새지 않았다** |
| **전역**(`--matching` 없이) | `( )` **빈 배열** | ⭕ S-3 의 함정(전역 오염)에 빠지지 않았다 |

⭐ **§8 수용 기준 2·3 과 브리프의 핵심 확인 항목("외장 키보드에만 적용되고 다른 입력 장치는
영향받지 않는다")이 이것으로 충족된다.** 한 번의 `--matching {VID,PID} --set` 이 그 디바이스의
IOHID 서비스 3개 전부에 도달했다(S-2 확인).

#### ⭐ D-17-5 전역 D-1 잔재 이관 — **통과**

검증 직전 상태가 마침 실제 이관 대상이었다 — 구버전 앱이 종료된 뒤에도 **전역** `caps_lock→F18`
이 남아 있었다. 새 빌드 기동 로그:

```
INFO ultrakey_engine::path_b: 전역 D-1 잔재(구버전이 매칭 없이 설치한 caps lock → F18)를
                              발견해 제거했다(D-17-5)
```

직후 전역 배열이 `( )` 로 비었고, D-1 은 **붙어 있는 두 키보드 각각에** 디바이스 한정으로
다시 설치됐다(D-17-1). 두 번째 기동에서는 `전역 D-1 잔재가 없다 — 이관할 것이 없다` 로
**멱등**하게 동작했다.

#### ⭐ 소유권 모델(PR #23 · D-17-2) — 양방향 **통과**

| # | 조작 | 결과 |
| :--- | :--- | :--- |
| 1 | 디바이스 B 에 "사용자가 직접 건" 매핑(`F14→F16`)을 심고 앱 재기동 | ⭕ **그 매핑이 살아남았고**, 우리 D-1 이 그 옆에 함께 설치됐다 |
| 2 | 그때의 원장(`perDevice._managed`) 확인 | ⭕ 우리 매핑만 들어 있고 **남의 매핑은 원장에 없다** |
| 3 | 디바이스 A 의 F-17 설정을 **전부 지우고** 재기동 | ⭕ 기능 1·2 매핑이 **사라졌다**(이슈 #19 회귀 방지). D-1 은 여전히 필요하므로 남았다 |
| 4 | 같은 시점 디바이스 B | ⭕ 남의 매핑 `F14→F16` 이 **그대로 살아 있다** |

#### C-1 디바이스 열거 — **통과**

```
$ cargo run -p ultrakey-platform --example keyboard_list_probe
열거된 키보드 2대 (중복 제거 후)
0x5ac    0x24f    F108Pro Dongle               USB   (못 읽음)
0x373b   0x11d9   Wireless mouse 8k dongle-L   USB   (못 읽음)
✅ (VID,PID) 중복 없음 — S-2 중복 제거가 동작한다.
```
`hidutil list` 의 물리 디바이스 목록과 정확히 일치한다. ⚠️ `Built-In` 은 읽지 못했다
(명세 §9 #7 은 그대로 `(미확정)`) — 이 구현은 식별에 VID+PID 만 쓰므로 기능에 영향이 없다.

#### `Keyboards` 탭 UI — **통과**(스크린샷으로 확인)

디바이스 팝업이 **실제 키보드 2대를 제품명으로**(서비스 3개씩이지만 **각 1항목**) 보여줬고,
`For all devices` 가 첫 항목이다. macOS 상태 표시줄이 `On` 배지를 띄웠다 — `fn_state.rs` 의
`CFPreferencesCopyValue` 가 실기기에서 실제 값을 읽는다는 확인이다. F1~F12 팝업이 **정확히
12개**(접근성 트리에서 팝업 총 13개 = 디바이스 1 + F-키 12, §3.1.5 와 일치). 기능 1 의
from 팝업에 F-05 소스 키 35종이 명세 순서로 떴다. **종속 표현 ①** — 빈 행이 있는 동안
`+ Add item` 이 dimmed 됐다. **종속 표현 ②(b)** — `For all devices` 에서 F-키 팝업에
`공통 설정을 따름` 항목이 나타나지 않았다.

#### ⛔ 이 검증에서 **발견해 고친 결함** — 실기기 검증이 아니면 못 잡았다

`settings.html` 이 `Keyboards` 탭을 `TABS` 에 넣었는데 `main.rs` 의 `tab_window_size()` 에
대응 항목이 없어, `settings_set_tab` 이 `알 수 없는 탭: keyboards` 로 거부하고 **설정 창
전체가 오류 화면으로 죽었다.** `tests/frontend_wiring.rs` 는 HTML **텍스트**만 보므로
(한쪽은 JS 배열, 다른 쪽은 Rust `match`) 이 어긋남을 원리적으로 잡을 수 없다.
→ `main.rs` 에 두 목록을 실제로 대조하는 테스트
`every_tab_in_settings_html_has_a_window_size` 를 추가했고, **결함을 되살려 실제로
실패하는 것까지 확인**한 뒤 다시 고쳤다.

#### 📌 부수 관측 — 정상 종료 시 정리는 **실행되지 않았다**(3회 모두)

앱을 정상 종료(`quit` 이벤트)시켜도 우리 매핑이 디바이스에 그대로 남았다. 3회 관측 모두
같았다. ⭐ 이것은 F-15 §3.1.1 결정 2("정리의 정본은 종료가 아니라 기동")를 **다시 실측으로
뒷받침한다** — 그래서 이 구현의 실제 정리 경로는 `reconcile_on_start` 이고, 위 검증들도
전부 그 경로가 동작함을 보였다.

#### ⛔ 이 세션이 **확인하지 못한 것** — 통과했다고 적지 않는다

| 항목 | 왜 못 했는가 |
| :--- | :--- |
| ⭐ **§8 수용 기준 6 — F-키를 실제로 눌러 볼륨이 올라가는지** | **물리 키를 누를 수 없다.** `CGEvent` 합성은 경로 B(HID 드라이버 계층) **위**에서 일어나므로 리매핑을 거치지 않아 유효한 대체 수단이 되지 못한다. 배열에 `F9 → 0xC000000E9` 가 들어간 것까지만 확인했다(⚠️ S-7 — 되읽기는 동작 근거가 아니다). 사람이 F108Pro 의 F9 를 눌러 확인해야 한다 |
| 핫플러그 재적용(§5 항목 1, 규칙 8) | 키보드를 물리적으로 뽑았다 꽂아야 한다 |
| 기능 2 의 나머지 11종 usage(명세 §9 #5·#6) | 범위 밖. 그래서 usage 가 확정된 8종만 팝업에 낸다 |
| §9 #1 Apple 내장 키보드와의 구분 | 이 기기에 **내장 키보드가 없다** — 스파이크 S-4 의 한계를 그대로 승계 |

#### ⭐ 복원 — 확인함

검증용으로 넣은 값(`F13→F15` · `F9→volume_up` · 디바이스 B 에 심은 `F14→F16`)을 전부
걷어내고, baseline 기록과 **줄 단위로 대조해 완전히 동일함**을 확인했다(전역 1 + A 3서비스 +
B 3서비스 = `caps_lock→F18` 7건, `fnState=1`). 검증 시작 시점에 돌고 있던 메인 체크아웃의
Ultrakey 도 **다시 띄워** 원래대로 되돌렸다.
⛔ 사용자의 Karabiner 설정은 읽지도 바꾸지도 않았다. 시스템 설정·키체인도 건드리지 않았다.

---

## 항목 8 — F-02 Seek 텍스트 후보 검출 (M3 1차 / 이슈 #30 신설)

> **무엇을 확인하는가**: 명세 `docs/spec/seek-text-detection.md` §8 수용 기준 중
> **실제 화면·실제 TCC 권한이 있어야만 확인 가능한 것들**. 순수 로직(좌표 변환 수식,
> 병합 M1~M3, 질의 매칭)은 `cargo test -p ultrakey-seek` 28개가 이미 덮으므로 여기서
> 다시 확인하지 않는다(§0-bis 의 원칙 그대로).
>
> ⛔ **F-01(세션)·F-03(오버레이)·F-04(클릭)은 아직 없다.** 그래서 이 항목은 단축키로
> Seek 을 여는 것이 아니라, 앱에 심어 둔 **검출 프로브**를 환경변수로 한 번 돌려
> 그 산출물을 로그와 덤프 파일로 확인한다.

### ⚠️ 8-0. 왜 터미널 실행으로는 이 항목을 검증할 수 없는가

Screen Recording 권한은 TCC 가 **부모 프로세스** 기준으로 판정한다
([`../spec/platform-constraints.md`](../spec/platform-constraints.md) §3.2).
**이 함정은 Accessibility 보다 Screen Recording 에서 훨씬 잘 드러난다** — 실제로
이 세션이 측정한 대비가 그것이다:

| 실행 방식 | `CGPreflightScreenCaptureAccess()` | OCR 후보 |
| :--- | :--- | ---: |
| 터미널에서 `cargo run --example seek_ocr_spike` | `true` (**터미널의** 권한) | 45 + 102 개 |
| 서명된 `.app` 을 `open` 으로 기동 | `false` | **0 개** |

⛔ **따라서 반드시 `./scripts/build-signed.sh` → `open …/Ultrakey.app` 경로로만 확인한다.**
`cargo tauri dev` 도, `.app` 안의 바이너리 직접 실행도 안 된다.

### 8-a. 준비

```sh
./scripts/build-signed.sh
rm -f ~/Library/Logs/Ultrakey/seek-candidates.json
```

프로브를 켜는 환경변수(전부 선택):

| 변수 | 뜻 |
| :--- | :--- |
| `ULTRAKEY_SEEK_DETECT_DUMP=1` | 기동 직후 검출을 **한 번** 돌린다 (이것만 필수) |
| `ULTRAKEY_SEEK_DETECT_AX=1` | 소스 B(Accessibility)도 켠다 |
| `ULTRAKEY_SEEK_DETECT_LANGS=ko-KR,en-US` | `recognitionLanguages` 를 지정한다 |
| `ULTRAKEY_SEEK_DETECT_REQUEST=1` | 권한이 없으면 시스템 프롬프트를 한 번 띄운다 |

```sh
open -n target/universal-apple-darwin/release/bundle/macos/Ultrakey.app \
  --env ULTRAKEY_SEEK_DETECT_DUMP=1
```

관찰: `~/Library/Logs/Ultrakey/ultrakey.log` 의 `F-02` 로 시작하는 줄,
그리고 후보 전량이 담긴 `~/Library/Logs/Ultrakey/seek-candidates.json`.

### ⭐ 8-b. 권한 **없을** 때 — 조용한 실패를 감지하는가 (명세 §8, F-11 §1.3)

⭐ **이것을 먼저 한다.** 권한을 부여하기 전 상태가 그대로 이 케이스이기 때문이다.

1. Screen Recording 목록에서 Ultrakey 가 꺼져 있는(또는 없는) 상태로 위 명령을 실행한다.
2. 로그에서 확인:
   - `F-02 검출 프로브 시작 … screen_recording=Denied`
   - `F-02 검출 프로브 완료 total=0 ocr=0 … capture_ms=<0 이 아닌 값>`
   - ⭐ `ERROR … ⚠️ 소스 A 후보 0개 + Screen Recording 권한 없음 — 조용한 실패다.`

⭐ **`capture_ms` 가 0 이 아니라는 점이 핵심이다** — `CGDisplayCreateImage` 가 **실패하지
않고 이미지를 돌려줬는데** OCR 후보가 0개다. 이것이 명세가 말한 조용한 실패이고, 권한
상태를 따로 보지 않으면 "텍스트 없는 화면"(§5 #2)과 구분되지 않는다.

### 8-c. 권한 **있을** 때 — 실제 후보가 잡히는가 (명세 §8 첫 항목)

1. 시스템 설정 ▸ 개인정보 보호 및 보안 ▸ **화면 기록** 에서 Ultrakey 를 켠다.
   (`ULTRAKEY_SEEK_DETECT_REQUEST=1` 로 프롬프트를 띄울 수도 있다.)
2. ⚠️ **앱을 완전히 종료했다가 다시 띄운다** — Screen Recording 은 이 요구가
   Accessibility 보다 엄격하다.
3. 화면에 **텍스트가 많은 창**(에디터·브라우저)을 띄워 두고 8-a 명령을 실행한다.
4. 확인:
   - `screen_recording=Granted`
   - `F-02 디스플레이 OCR 완료 (증분 전달)` 가 **디스플레이 수만큼** 찍히는가 —
     이것이 결정 S-1(디스플레이별 증분 전달)이 실제로 동작한다는 증거다
   - `F-02 후보` 줄들의 `text` 가 화면에 실제로 보이는 문자열과 일치하는가
   - ⭐ `x`/`y` 가 **그 텍스트가 실제로 있는 화면 위치**를 가리키는가.
     주 디스플레이가 아닌 화면의 후보는 `x` 가 음수여야 한다(전역 원점이 `(-2560, 0)` 인
     경우) — 이것이 §5 #3 의 v1.55 다중 디스플레이 회귀를 막는 지점이다

### 8-d. 소스 B(AX)와 병합 (명세 §8 2·3·8번 항목)

```sh
open -n …/Ultrakey.app --env ULTRAKEY_SEEK_DETECT_DUMP=1 --env ULTRAKEY_SEEK_DETECT_AX=1
```

- `ax=<0 이 아닌 값>` 이 찍히는가 (최전면 창에서 AX 요소를 읽었는가)
- `merged_away=<0 이 아닌 값>` — 겹친 후보가 실제로 제거됐는가
- 덤프 JSON 에서 같은 위치에 `"Ocr"` 과 `"Accessibility"` 가 **함께** 남아 있지
  않은가. ⭐ 남아 있다면 M3(OCR 이 AX 를 대체)가 깨진 것이다
- ⚠️ AX 를 끄고(`ULTRAKEY_SEEK_DETECT_AX` 없이) 돌리면 `ax=0` 이어야 한다

### 8-e. 한국어 인식 (스파이크 §3, 결정 S-4)

한글이 많은 화면을 띄워 두고:

```sh
open -n …/Ultrakey.app --env ULTRAKEY_SEEK_DETECT_DUMP=1 \
  --env ULTRAKEY_SEEK_DETECT_LANGS=ko-KR,en-US
```

- 기본값(변수 없음)에서는 한글 후보가 **0개**여야 한다
- `ko-KR` 을 **첫 번째**로 넣으면 한글 후보가 나와야 한다
- ⚠️ `en-US,ko-KR` 순서로는 **나오지 않는다** — 실측된 동작이다
- `total_ms` 가 3~4배로 늘어나는 것도 함께 확인한다 (S-4 의 대가)

### 8-e-2. 로케일 기반 자동 구성 (이슈 #48, 2026-08-31 신설)

> 위 8-e 가 검출 프로브 환경변수(`ULTRAKEY_SEEK_DETECT_LANGS`)를 직접 지정하는
> 것이라면, 이 절은 **실제 앱 세션**에서 `general.language`(UI 로케일)가
> `recognitionLanguages` 로 자동 반영되는 것을 확인한다. 프로브는 카탈로그를
> 읽지 않으므로 이 절은 프로브로 대체할 수 없다.

1. `General` 탭 언어 선택에서 **한국어**를 고른다(또는 [`defaults`](../spec/settings-store-and-integrity.md) 로
   `general.language = "ko"` 를 직접 넣는다). 앱을 재시작해 UI 가 한국어로
   뜨는지 확인한다.
2. 화면에 **한글이 많은 창**(한국어 웹 페이지·문서)을 띄워 둔다.
3. 서명된 `.app` 을 `open` 으로 기동하고(`./scripts/build-signed.sh`),
   Seek 세션을 **실제 단축키/리맵으로 연 뒤** 한글 후보가 나타나는지 확인한다:
   - 로그의 `F-02` 줄에 한국어 문자열이 OCR 후보로 잡히는가
   - `seek-candidates.json`(또는 `ULTRAKEY_SEEK_TRACE=1` 로그)에 한글이 있는가
4. **영어 폴백 회귀 확인**: `General` 탭 언어를 **System**(또는 English)으로
   되돌리고 재시작 → 이전 동작(영어 단일)과 동일하게 한글 후보가 **0개**여야
   한다(회귀 금지).
5. 시스템 언어가 한국어인 macOS 에서 `general.language` 를 설정하지 않은 채
   (키 부재) 기동하면, UI 가 한국어 카탈로그인 것과 일치하게 한글 OCR 도
   동작해야 한다(이슈 #48 해석: 미설정 = 시스템 언어 폴백 결과를 따른다).
6. 일본어·중국어(간체)도 같은 절차로 확인 가능 — `general.language = ja`
   또는 `zh` 로 바꿔 같은 화면의 일본어/중국어 텍스트 후보를 본다.

⚠️ **이 절은 실기기에서 아직 실행되지 않았다** — 서명된 `.app` 과 화면 기록
권한, 한국어 화면 콘텐츠가 필요해서다. 구현 PR 의 "실기기 미검증" 항목으로
남기고, 실행 결과를 기록할 자리로 이 절을 쓴다.

### ⛔ 8-f. 이 절차로도 확인할 수 없는 것

| 항목 | 왜 |
| :--- | :--- |
| **Retina(`scale = 2.0`)** | 이 기기에 붙은 두 디스플레이가 모두 `scale = 1.00` 이고 Retina 패널이 없다. 해상도(시스템 설정) 변경은 위임 범위 밖이라 하지 않았다. §3.2.3 3단계는 `ultrakey-seek` **단위 테스트로만** 검증된다 |
| `Only Seek in the frontmost window` 의 실제 창 프레임 획득 | 창 프레임을 어디서 얻는지는 명세가 `(미확정)` 으로 남겼고(§3.3.2), 그 획득 책임은 F-01 이다. 이 구현은 호출자가 넘긴 사각형을 쓴다 |
| 응답 없는 앱에서 AX 타임아웃이 실제로 걸리는가 (§8, §5 #7) | 앱을 인위적으로 멈추게 하려면 `SIGSTOP` 을 남의 프로세스에 보내야 한다. 코드상 `set_timeout(0.5)` 이 시스템 와이드·대상 앱 양쪽에 걸리는 것까지만 확인했다 |
| Stage Manager · 전체화면 스페이스 (§5 #5, #15) | 범위 밖 |

### 📌 실측 결과 (2026-08-30, 이슈 #30)

전부 **서명된 `.app` 을 `open` 으로 기동**해서 얻은 결과다(터미널 실행 결과가 아니다).
화면 기록 권한은 검증 목적으로 부여했다가 **끝난 뒤 `tccutil reset ScreenCapture
app.ultrakey.Ultrakey` 로 되돌렸고, 되돌아간 것까지 확인**했다.

| 절차 | 결과 |
| :--- | :--- |
| 8-0 TCC 함정 | ✅ **관측함.** 같은 코드가 터미널에서는 147개 후보, 서명된 `.app` 에서는 0개(권한 부여 전) |
| 8-b 권한 없을 때 조용한 실패 감지 | ✅ **통과.** `screen_recording=Denied`, `capture_ms=8.05`(**캡처 자체는 성공**), `ocr=0`, `ERROR … 조용한 실패다` |
| 8-c 실제 후보 검출 | ✅ **통과.** `screen_recording=Granted`, **후보 174개**(OCR 169 + AX 5). 로그의 `text` 가 화면에 실제로 보이던 문자열과 일치했다 |
| 8-c 증분 전달 (S-1) | ✅ **통과.** `F-02 디스플레이 OCR 완료` 가 디스플레이마다 따로 찍혔다 — id=3 66개 @ 583 ms, id=2 103개 @ 391 ms. 전체 완료(1109 ms)를 기다리지 않는다 |
| ⭐ 8-c 다중 디스플레이 좌표 (§5 #3) | ✅ **통과.** 전역 원점이 `(-2560, 0)` 인 디스플레이 2 의 후보 x 범위가 **[-2556.3, -100.5]**, 원점 `(0,0)` 인 디스플레이 3 은 **[94.9, 3667.0]** 였다. 각 화면의 실제 범위 안에 정확히 들어간다 |
| 8-d 소스 B(AX) | ✅ **통과.** Safari 를 최전면에 두고 `ax=60` — `Start Page` · `Go back` · `Make Safari your default browser?` 등 최전면 창의 요소를 전역 좌표로 읽었다 |
| ⭐ 8-d 병합 M3 | ✅ **통과.** `ocr=156, ax=60, merged_away=13 → total=203`. 덤프를 다시 대조해 **겹치면서 텍스트가 같은 OCR/AX 쌍이 둘 다 남은 경우가 0건**임을 확인했다 |
| 8-d 정렬 | ✅ **통과.** 덤프 203개의 `y` 가 단조 증가 |
| 8-e 한국어 인식 | ✅ **통과.** 기본값에서는 한글 후보 0개, `ULTRAKEY_SEEK_DETECT_LANGS=ko-KR,en-US` 로는 **87개**. 대가도 확인 — `total_ms` 가 892 ms → **2417 ms** |
| 권한 복원 | ✅ **확인함.** `tccutil reset` 후 다시 띄우니 `screen_recording=Denied` 로 돌아갔다 |

#### ⭐ 이 검증이 **찾아낸 버그** — 후보 수로 권한 부재를 판정하면 안 된다

권한을 되돌린 뒤 마지막으로 한 번 더 돌렸더니 `ocr=5` 가 나왔다. 후보는
`Gitkraken` · `window Help` · `view` · `coll` · `` — 전부 **메뉴 막대** 문자열이다.

⭐ **권한 없이 캡처하면 데스크톱 배경만 오는 것이 아니라 메뉴 막대까지 함께 온다**
(메뉴 막대는 화면 기록 보호 대상이 아니다). 처음 구현한 판정은
`ocr_count == 0 && 권한 없음` 이었으므로 **이 상황에서 발화하지 않았을 것이다.**

→ `DetectionOutcome::ocr_blocked_by_permission()` 을 **후보 수를 보지 않고 권한
상태만 보도록** 고쳤다. 권한이 없으면 후보가 몇 개 나오든 소스 A 는 화면의 실제
내용을 보고 있지 않기 때문이다. **실기기 검증이 아니었으면 이 결함은 그대로
남았다** — 단위 테스트로는 "권한 없이 캡처한 이미지에 메뉴 막대가 들어 있다" 는
사실 자체를 알 수 없다.

---

## 항목 9 — F-03 Seek 오버레이 UI (M3 2차 / 이슈 #34 신설)

> 대상 명세: `docs/spec/seek-overlay-ui.md` · 실측 근거: `docs/dev/seek-overlay-render-spike.md`
> 이 항목이 확인하는 것은 **화면에 실제로 그려지는가**와 **포커스를 뺏지 않는가** 둘이다.
> 두 가지 다 단위 테스트로는 원리적으로 확인할 수 없다.

### ⚠️ 9-0. 왜 이 항목이 필요한가 — 실제로 세 가지를 잡았다

이 절차를 처음 수행하면서 **단위 테스트가 전부 통과하는 상태에서 세 개의 결함**을
찾았다. 셋 다 "코드가 옳은데 화면이 틀린" 종류라 자동 테스트로는 잡히지 않는다.

| # | 증상 | 원인 | 어떻게 잡았나 |
| :--- | :--- | :--- | :--- |
| 1 | 창 설정은 성공했다고 나오는데 `can_become_key=true` | **tao(Tauri 창 백엔드)가 `canBecomeKeyWindow` 를 무조건 `YES` 로 오버라이드**한다. `NSWindowStyleMaskBorderless` 로 바꿔도 AppKit 기본 구현이 애초에 호출되지 않는다 | 설정 뒤 **값을 되읽어 로그로 남긴** 덕에 즉시 보였다 |
| 2 | 앱이 조용히 죽음(크래시 리포트도 Rust 패닉도 없음) | 1번을 고치려 `object_setClass` 로 클래스를 바꿔 끼웠는데(`tauri-nspanel` 기법), Tauri 창은 생성 시점에 이미 KVO 로 스위즐돼 있어(`NSKVONotifying_TaoWindow`) isa 재변경이 KVO 와 충돌한다 | 킬 스위치(`ULTRAKEY_OVERLAY_NO_SWAP`)로 원인을 갈랐다 |
| 3 | ⭐ **연결선이 디스플레이 경계에서 끊긴 것처럼 보임** — v1.55 회귀와 똑같은 증상 | 좌표 계산은 **정확했다**. 진짜 원인은 **JS 의 `tauri.event.listen` 이 리스너를 `target: Any` 로 등록**해서, Rust 가 `emit_to` 로 대상을 지정해도 **모든 창이 모든 프레임을 받은 것**이다 → 디스플레이 3 의 창이 디스플레이 2 의 프레임을 그렸다 | 스크린샷 픽셀을 직접 세어 "선이 있는 곳"과 "있어야 할 곳"이 다름을 확인하고, 캔버스에 임시 배지(창 이름·`display_id`)를 그려 확정 |

### 9-a. 준비

```sh
./scripts/build-signed.sh
```

⛔ `cargo tauri dev` 금지, `.app` 안 바이너리 직접 실행 금지(§0 과 같은 이유).

⭐ **F-01(세션 상태 머신)이 아직 없으므로 오버레이를 열 호출자가 없다.** 그래서
검증 전용 하네스를 둔다 — `ULTRAKEY_OVERLAY_DEMO=1`. 이 모드는 **엔진(CGEventTap)을
켜지 않고**, `setup()` 안에서 곧바로 반환한 뒤 오버레이만 구동한다(그래서 이미 떠
있는 다른 인스턴스와 나란히 돌릴 수 있다).

```sh
open -n target/universal-apple-darwin/release/bundle/macos/Ultrakey.app \
  --env ULTRAKEY_OVERLAY_DEMO=1
# 선택: 타이핑을 흉내 낼 문자열(기본 "se")
#       --env ULTRAKEY_OVERLAY_DEMO_QUERY=set
# 선택: AX 소스도 켜서 §3.5 의 OCR/AX 하이라이트 구분을 본다
#       --env ULTRAKEY_OVERLAY_DEMO_AX=1
```

⭐ **기동 후 6초 뒤에** 오버레이가 뜬다. **그 사이에 다른 앱(텍스트 편집기 등)을
클릭해 커서를 깜빡이게 두어라** — 그것이 9-c 의 판정 기준이다.

### 9-b. 창 네이티브 속성 — 로그로 **값을 대조**한다

```sh
grep '네이티브 설정 완료' ~/Library/Logs/Ultrakey/ultrakey.log
```

창마다 한 줄씩(디스플레이 수 + 검색 바 1) 나와야 하고, 각 줄이 다음을 만족해야 한다:

| 필드 | 기대값 | 무엇을 뜻하는가 |
| :--- | :--- | :--- |
| `can_become_key` | **`Some(false)`** | ⭐ 활성화하지 않고 표시 — 클릭해도 키 윈도우가 되지 않는다 |
| `non_activating` | `true` | 표적 스위즐이 걸렸다 |
| `level` | `Some(1000)` | `NSScreenSaverWindowLevel` — 전체화면 앱 위 |
| `collection_behavior` | `Some(337)` | `CanJoinAllSpaces(1) + Stationary(16) + IgnoresCycle(64) + FullScreenAuxiliary(256)` |
| `outer_position` / `outer_size` | `requested` 와 일치 | 창이 정말 그 디스플레이를 덮는가 |

⚠️ `can_become_key` 가 `Some(true)` 면 **9-0 의 결함 1이 재발한 것**이다.

### ⭐ 9-c. 포커스를 뺏지 않는가 (§8 수용 기준 1)

오버레이가 뜬 **뒤에** 확인한다:

1. 9-a 에서 클릭해 둔 앱의 **텍스트 커서가 계속 깜빡이는가**.
2. 메뉴 막대의 앱 이름이 **그대로인가**(Ultrakey 로 바뀌면 실패).
3. 값으로도 대조:

```sh
osascript -e 'tell application "System Events" to get name of first application process whose frontmost is true'
```

### 9-d. 증분 수신 (S-6, §8 — 이 기능의 핵심)

```sh
grep -E '오버레이를 열었다|검출 전에 질의를 걸었다|증분 수신|검출 완료' ~/Library/Logs/Ultrakey/ultrakey.log
```

- `오버레이를 열었다 … elapsed_ms=0.x` — ⭐ **검출을 기다리지 않고 즉시 열린다**
- `증분 수신 … display_id=… arrived_at_ms=…` 이 **디스플레이마다 따로** 찍힌다
- 눈으로: 검색 바가 먼저 뜨고(하단에 얇은 진행 표시), 그 뒤 **하이라이트가 화면에
  단계적으로 늘어난다**. 한 번에 다 뜨면 S-6 이 깨진 것이다

### ⭐ 9-e. 연결선이 디스플레이 경계를 넘는가 (§8 — v1.55 회귀 방지)

눈으로만 보면 애매하다. **두 화면을 동시에 캡처해 경계 픽셀을 센다**:

```sh
screencapture -x a.png b.png   # 인자 하나당 디스플레이 하나
```

그다음 한쪽 화면의 **안쪽 끝 열**과 다른 쪽 화면의 **바깥쪽 끝 열**에서 강조색
(`#0A84FF`) 픽셀의 y 좌표를 비교한다. **두 y 가 같아야** 선이 이어진 것이다.

⚠️ 두 창의 `requestAnimationFrame` 이 독립적이라, 선택이 막 바뀐 직후에는 한쪽만
갱신된 프레임이 잡힐 수 있다. **여러 번 캡처해 한 번이라도 일치하면 통과**로 본다.

### 9-f. 전체화면 앱 위 표시 (§8)

```sh
osascript -e 'tell application "TextEdit" to activate' \
          -e 'tell application "TextEdit" to make new document with properties {text:"F-03 fullscreen verification"}'
osascript -e 'tell application "System Events" to tell process "TextEdit" to set value of attribute "AXFullScreen" of front window to true'
```

전체화면이 된 디스플레이를 캡처해 **하이라이트가 그 위에 그려져 있는지** 본다.
⛔ 끝나면 전체화면을 풀고 문서를 저장하지 않고 닫는다.

### 9-g. 검색 바 위치 저장 (§3.4, §8)

```sh
grep -E '검색 바 위치를 저장했다|ULTRAKEY_OVERLAY_DEMO —' ~/Library/Logs/Ultrakey/ultrakey.log
```

- 첫 실행: `stored=None` → `저장된 검색 바 위치가 없다 — 기본 위치를 쓴다`
- 이후 실행: `stored=Some((x, y))` — **부재 = 기본값**(F-15 §3.6) 규약대로다

### ⛔ 9-h. 이 절차로도 확인할 수 **없는** 것

| 항목 | 왜 |
| :--- | :--- |
| **마우스로 실제 드래그**해 검색 바를 옮기는 것 | 사람이 없으면 못 한다. 합성 마우스 이벤트를 쓰려면 `CGEvent` 마우스 경로를 만들어야 하는데 **그것이 F-04 의 범위**라 손대지 않았다. 대신 드래그의 **전제**(클릭 통과 꺼짐 · `setMovableByWindowBackground(true)` · `canBecomeKey=false`)와 **결과**(`Moved` → 저장 → 재시작 후 복원)는 각각 확인했다 |
| **디스플레이 핫플러그** | 케이블을 뽑을 사람이 없다. `didChangeScreenParameters` 구독과 재배치 로직은 배선돼 있고, 재배치 판정 자체는 `ultrakey-overlay` 단위 테스트가 덮는다 |
| **Retina(배율 2.0)** | 이 기기의 두 디스플레이가 모두 `backing_scale = 1.0` 이다(F-02 스파이크와 같은 한계). `devicePixelRatio` 보정 코드는 있으나 실측되지 않았다 |
| **Mission Control / Stage Manager** (§5 #5, #6) | 명세가 `(추정)` 으로 남긴 항목이고 이번 범위 밖 |
| **다크/라이트 전환** | 시스템 외관을 바꾸는 것은 사용자 설정 변경이라 하지 않았다. 조회 자체(`dark=true`)는 로그로 확인 |

### 📌 실측 결과 (2026-08-30, 이슈 #34 / 브랜치 `feat/seek-overlay`)

기기·배치는 F-02 스파이크와 같다(M4 Pro / 3840×1600 @ `(0,0)` + 2560×1440 @ `(-2560,0)`,
둘 다 `backing_scale=1.0`, 60 Hz + 144 Hz). 전부 **서명된 `.app` 을 `open` 으로 기동**한 결과다.

| 절차 | 결과 |
| :--- | :--- |
| 9-b 창 속성 | ✅ **통과.** 창 3개(`seek-bar` · `overlay-3` · `overlay-2`) 모두 `can_become_key=Some(false)` · `non_activating=true` · `level=Some(1000)` · `collection_behavior=Some(337)` |
| 9-b 창 프레임 | ✅ **통과.** `overlay-3` = `(0,0) 3840×1600`, `overlay-2` = `(-2560,0) 2560×1440` — 요청값과 실제값이 일치 |
| ⭐ 9-c 포커스 미탈취 | ✅ **통과.** 오버레이가 뜬 뒤에도 `frontmost = firefox` 그대로였고, 메뉴 막대도 바뀌지 않았다 |
| ⭐ 9-d 증분 수신 | ✅ **통과.** `오버레이를 열었다 … elapsed_ms=0.23` (검출 전), 그 뒤 `display_id=3 candidates=42 arrived_at_ms=478` → `display_id=2 candidates=118 arrived_at_ms=896`. 검출 완료는 966 ms — **세션은 그것을 기다리지 않았다** |
| 9-d 키 입력 재렌더링 | ✅ **통과.** `키 입력 — 재렌더링 query=se matches=25` |
| ⭐ 9-e 연결선 경계 통과 | ✅ **통과.** 동시 캡처 8쌍 중 2쌍에서 **디스플레이 2 의 오른쪽 끝 열과 디스플레이 3 의 왼쪽 끝 열이 모두 `y=423`** — 같은 y 에서 이어진다. (나머지 쌍은 두 창의 rAF 가 어긋난 순간이거나 선택 매치가 한 화면 안에 있던 순간이다) |
| 9-e 창별 프레임 격리 | ✅ **통과.** 임시 배지로 `win=overlay-3 id=3` · `win=overlay-2 id=2` 확인(수정 전에는 `win=overlay-3 id=2` 였다 — 9-0 결함 3) |
| ⭐ 9-f 전체화면 위 표시 | ✅ **통과.** TextEdit 을 디스플레이 2 에 전체화면(`AXFullScreen=true`, 프레임 `(-2560,0,2560,1440)`)으로 두고 캡처했을 때, **그 위에 하이라이트 10개와 라벨(4·9·13·14·16·18·20·21·23·24)이 그려져 있었다** |
| 9-g 검색 바 위치 저장 | ✅ **통과.** 첫 실행 `stored=None` → 기본 위치 계산, 이후 실행 `stored=Some((1720.0, 320.0))` 로 복원 |
| §3.5 OCR/AX 구분 | ✅ **통과.** `ULTRAKEY_OVERLAY_DEMO_AX=1` 로 `AX 후보를 넣는다 … count=10` — AX 매치는 파선 테두리로 그려진다 |
| §3.5 외관·모션 | ✅ **조회 확인.** `외관·모션 설정 dark=true reduce_motion=false` — 팔레트가 시스템 외관을 따른다 |
| 상한 정책(§4.2) | ✅ 로그 `omitted=0`(후보가 상한 미만이었다). 상한 동작 자체는 단위 테스트가 덮는다 |

#### 검증 중 변경한 시스템 상태와 복원

| 항목 | 상태 |
| :--- | :--- |
| **화면 기록 권한** | ⚠️ **이 검증은 권한을 바꾸지 않았다.** 시작 시점에 이미 `screen_recording=Granted` 였고(로그), 끝날 때도 그대로다. 부여도 회수도 하지 않았으므로 되돌릴 것이 없다 |
| TextEdit | 검증용으로 새 문서를 만들어 전체화면으로 두었다가 **전체화면 해제 → 저장하지 않고 닫기 → 종료**까지 완료 |
| 검증용 앱 프로세스 | `ULTRAKEY_OVERLAY_DEMO`/`ULTRAKEY_OVERLAY_SPIKE` 로 띄운 인스턴스는 전부 종료했다 |
| 키체인 · 시스템 설정 · Karabiner | **건드리지 않았다** |

---

## 항목 10 — F-01 Seek 활성화·세션 상태 머신 (M3 3차 / 이슈 #38 신설)

> 대상 명세: `docs/spec/seek-activation-and-session.md` · 실측 근거:
> `docs/dev/seek-ocr-latency-spike.md`(S-1·S-6) · `docs/dev/seek-overlay-render-spike.md`(R-4)
> 이 항목이 확인하는 것은 **세션이 실제로 열리고 닫히는가**와 **S-6 대로 검출을
> 기다리지 않는가** 둘이다. 둘 다 단위 테스트로는 원리적으로 확인할 수 없다.

### ⚠️ 10-0. 사전 확인 — 어느 키를 눌러야 하는가

⭐ **항목 6 의 `6-0-bis` 표를 그대로 적용한다.** 이 기능도 caps lock 을 쓰므로
Karabiner 상태에 따라 눌러야 할 물리 키가 정반대로 바뀐다.

```sh
# 1 이상이면 Karabiner 가 실제로 키를 가로채고 있다.
ioreg -c IOHIDDevice -r -d1 | grep -c "Karabiner DriverKit VirtualHIDKeyboard"
# 사용자 Karabiner 매핑 (⛔ 읽기만 한다. 절대 바꾸지 않는다)
python3 -c "import json,os;d=json.load(open(os.path.expanduser('~/.config/karabiner/karabiner.json')));print(d['profiles'][0]['simple_modifications'])"
hidutil property --get UserKeyMapping     # 경로 B(D-1) 잔재
```

### ⭐ 10-0-bis. 사람이 없을 때 — `key_poke` 로 주입한다

⛔ **`key_poke` 는 `CGEventPost(HIDEventTap)` 으로 주입한다 — 즉 Karabiner 의 가상
HID 계층보다 **위**다.** 그래서 `0x39`(caps lock)를 주입하면 Karabiner 의
`caps_lock ↔ left_control` 맞바꿈과 **무관하게** 탭에 caps lock 으로 도착한다.
이것이 이 항목을 자동으로 수행할 수 있게 해 주는 열쇠다.

⛔ **그 대신 이 도구로는 확인되지 않는 것**(항목 6 과 같다): HID 층 아래는 지나지
않으므로 **물리 caps lock 의 래칭 동작**(누를 때만 `flagsChanged` 가 오고 뗄 때는
오지 않는 것, §5 #20)은 재현되지 않는다. 아래 결과의 hold 모드는 **주입된 두 번의
`flagsChanged`** 로 얻은 것이지, 물리 caps lock 을 실제로 누르고 뗀 것이 아니다.

```sh
./scripts/build-signed.sh
open -n target/universal-apple-darwin/release/bundle/macos/Ultrakey.app \
  --env ULTRAKEY_SEEK_TRACE=1
cargo build -p ultrakey-platform --release --example key_poke
```

⚠️ **세션이 열린 채로 두지 마라.** 세션 중에는 계층 1 이 **모든 키 이벤트를
소비**하므로(§3.1) 키보드가 통째로 먹통이 된다. 모든 주입 시퀀스는 반드시
`tap:0x35`(Esc)나 확정으로 끝나야 하고, 막히면 앱을 죽이면(`pkill -f Ultrakey.app`)
탭이 함께 풀린다.

### 10-a. 활성화 경로 3종

설정은 `Seek` 탭(+`Presets` 탭)에서 하거나, 검증용으로 저장소를 직접 편집한다
(⛔ **끝나면 baseline 으로 되돌리고 대조까지 한다** — 아래 "검증 후 되돌린 것").

| 경로 | 설정 | 주입 |
| :--- | :--- | :--- |
| 1 전역 단축키 | `seek.toggleShortcut.code=Space` · `.modifiers=0x80000`(⌥) | `fc:0x3A:0x80000 tap:0x31 fc:0x3A:0x0` |
| 2 키 리매핑 | `seek.remapKey=CapsLock` · `seek.executeOnClose` | `fc:0x39:0x10000` (다운), 다시 `fc:0x39:0x10000` (업) |
| 3 quick press | `seek.remapKey=-` · `presets.capsQuickPress.enabled=true` · `.action=Seek` | `fc:0x39:0x10000 sleep:120 fc:0x39:0x0` |

### 10-b. 판정은 로그로 한다

```sh
grep "ultrakey_app::seek" ~/Library/Logs/Ultrakey/ultrakey.log
```

| 로그 줄 | 무엇을 증명하는가 |
| :--- | :--- |
| `⭐ Seek 세션을 열었다 path=… mode=… elapsed_ms=… matches=0` | ⭐ **S-6** — 검출을 기다리지 않고 **후보 0개로 즉시** 열린다 |
| `⭐ 증분 수신 display_id=… candidates=… arrived_at_ms=…` | ⭐ **S-1** — 디스플레이별로 **하나씩** 도착한다 |
| `키 입력 query=… matches=… state=…` | 타이핑으로 좁혀진다 |
| `선택 이동 index=… text=…` | ↑↓Tab⇧Tab`;` 순환 |
| `⭐ 확정 text=… point=… modifiers=… query=…` | F-04 로 넘길 `ConfirmedMatch`(modifiers 스냅샷 포함) |
| `세션 종료 reason=…` | `Confirmed`/`Cancelled`/`Toggled`/`ReleasedWithoutMatch` |

### ⛔ 10-c. 이 절차로도 확인할 수 **없는** 것

| 항목 | 왜 |
| :--- | :--- |
| **물리 caps lock 의 래칭** | `key_poke` 가 HID 층 위에서 주입한다(10-0-bis). 이 기기는 Karabiner 가 caps lock 을 가로채고 있어 물리 caps lock 자체가 탭에 도달하지 않는다 — ⛔ 사용자 설정을 바꾸지 않는다 |
| **하위 앱에 문자가 도달하지 않는가** | 텍스트 필드를 띄우고 사람이 확인해야 한다. 중재기가 `Disposition::Consume` 을 낸다는 것은 단위 테스트(`session_active_consumes_key_events_and_routes_them`)가 덮는다 |
| **F-04 실제 클릭** | 범위 밖. `NullClickExecutor` 가 요청만 기록한다 |
| **Secure Input 중 동작**(§5 #1) · **세션 중 앱/Space 전환**(§5 #5) | 명세가 `(추정)` 으로 남긴 항목이고 이번 범위 밖 |
| **Retina(배율 2.0)** | 이 기기의 두 디스플레이가 모두 `backing_scale=1.0`(F-02·F-03 스파이크와 같은 한계) |

### 📌 실측 결과 (2026-08-31, 이슈 #38 / 브랜치 `feat/seek-session`)

기기·배치는 F-02·F-03 스파이크와 같다(M4 Pro / 3840×1600 @ `(0,0)` + 2560×1440 @
`(-2560,0)`, 둘 다 `backing_scale=1.0`). 전부 **서명된 `.app` 을 `open` 으로 기동**한
결과다. 키는 `key_poke` 로 주입했다(10-0-bis 의 한계가 그대로 적용된다).

#### ⚠️ 관찰 방법과 그 한계 — 먼저 읽어라

- 사전 확인 결과 **Karabiner 가 켜져 있었다**(`VirtualHIDKeyboard` = 1, 프로필이
  `caps_lock ↔ left_control` 을 맞바꾼다). ⛔ **사용자 설정은 건드리지 않았다.**
  대신 `key_poke` 가 HID 층 **위**에서 주입하므로 caps lock 이 그대로 탭에 도착했다.
- `hidutil property --get UserKeyMapping` 은 검증 전·중·후 모두 `( )` 였다.

#### ✅ 통과한 것

| 절차 | 결과 |
| :--- | :--- |
| 10-a 경로 1 전역 단축키 | ✅ **통과.** `path=GlobalShortcut mode=Toggle elapsed_ms=0.354` |
| 10-a 경로 2 키 리매핑 (hold) | ✅ **통과.** `path=RemapKey mode=Hold elapsed_ms=0.149` |
| 10-a 경로 2 키 리매핑 (toggle) | ✅ **통과.** `path=RemapKey mode=Toggle`, 재입력 시 `reason=Toggled` |
| 10-a 경로 3 quick press caps lock | ✅ **통과.** `path=QuickPressCapsLock mode=Toggle elapsed_ms=0.162` |
| ⭐ **S-6 즉시 열림** | ✅ **통과.** 네 경로 전부 `elapsed_ms < 0.4`, `matches=0` — 검출 완료(≈780~1120 ms)를 **기다리지 않는다** |
| ⭐ **S-1 증분 수신** | ✅ **통과.** `display_id=3 candidates=84 arrived_at_ms=553` → `display_id=2 candidates=106 arrived_at_ms=901`. 디스플레이별로 따로 도착한다 |
| 타이핑으로 좁혀짐 | ✅ **통과.** `query=s matches=85` → `query=se matches=24` |
| ↓ 순환 | ✅ **통과.** `선택 이동 index=Some(1)` → `Some(2)` |
| `;` 순환(`Semicolon highlights next match` ☑) | ✅ **통과.** `Some(2)` → `Some(3)` |
| Tab 정방향 / ⇧Tab 역방향 | ✅ **통과.** `Some(1)` → `Some(2)` → (⇧Tab) `Some(1)` |
| hold 릴리즈 = 확정 | ✅ **통과.** `⭐ 확정 text=… modifiers=536936448 query=se` → `세션 종료 reason=Confirmed` |
| Esc 취소 | ✅ **통과.** `세션 종료 reason=Cancelled`, 확정 없음 |
| toggle 재입력이 닫는다(§3.3) | ✅ **통과.** 리매핑 키·전역 단축키 양쪽 모두 `reason=Toggled` |
| ⭐ 오버레이가 실제로 그려진다 | ✅ **통과.** 스크린샷에 매치 하이라이트와 검색 바 → 선택 매치 **연결선**이 보인다 |
| 오버레이가 포커스를 뺏지 않는다 | ✅ **통과.** 창 3개 전부 `can_become_key=Some(false)` · `level=Some(1000)` · `collection_behavior=Some(337)` |
| ⭐ F-04 경계 | ✅ **통과.** `ConfirmedMatch` 가 좌표(`point_x/point_y`)와 **확정 순간 modifier 스냅샷**(F-04 §5 #10)을 싣고 나온다. 실제 클릭은 `NullClickExecutor` 가 받는다(F-04 범위) |

증거 스크린샷: [`screenshots/issue-38-seek-session-overlay.png`](screenshots/issue-38-seek-session-overlay.png)

#### 🐛 이 검증이 **찾아낸 결함 3건** — 전부 단위 테스트가 통과하는 상태에서 나왔다

| # | 증상 | 원인 | 고친 것 |
| :--- | :--- | :--- | :--- |
| 1 | ⭐ **전역 단축키로 연 세션이 같은 단축키로 닫히지 않는다** — 열린 채 남아 **키보드 전체가 먹통**이 된다 | 세션 중에는 계층 1 이 모든 키를 소비하므로(§3.1) 그 조합이 **윈도 서버의 핫키 디스패치까지 도달하지 못한다.** `global-hotkey` 는 두 번째 입력을 영영 보지 못한다. 리매핑 키 경로는 트리거 판정이 계층 1 **앞**이라 이 문제가 없었다 | `SeekConfig::global_shortcut` 에 조합을 들고, 세션 중 키 라우팅에서 직접 재입력을 알아본다 |
| 2 | ⭐ 고치고 나니 이번엔 **세션이 열리는 순간 스스로 닫혔다** | 같은 물리 누름 하나가 **두 경로로** 관측된다 — ① 시스템 핫키(세션을 연다) ② 계층 1 이 넘겨 주는 같은 키 이벤트. ②를 재입력으로 오인했다 | `awaiting_shortcut_release` 빗장 — 그 키가 **한 번 떼어질 때까지** 재입력 판정을 잠근다. ⛔ 시간 창으로 막지 않았다(임계값이 또 하나의 `(미확정)` 상수가 되고 느린 기기에서 다시 깨진다) |
| 3 | 검증 로그가 한 칸씩 밀려 읽힌다 — 첫 글자를 친 줄에 `query=`(빈 값) | 계측이 `handle_key` **앞**에서 찍혔다 | 뒤로 옮겼다 |

⭐ **1·2 는 실기기 검증이 아니면 찾지 못했을 종류다** — 상태 머신 단독으로는 "핫키
이벤트가 오지 않는다"도 "같은 누름이 두 경로로 온다"도 표현되지 않기 때문이다.
셋 다 회귀 테스트를 함께 넣었다(`global_shortcut_re_entry_closes_toggle_session` 외 2종).

#### ⬜ 수행하지 못한 것 — 통과했다고 적지 않는다

10-c 표 그대로다. 특히 **물리 caps lock 을 사람이 실제로 누른 검증은 하지 않았다** —
이 기기에서는 Karabiner 가 그 키를 가로채고 있고, 그 설정을 바꾸는 것은 작업 경계
밖이다.

#### 검증 후 되돌린 것

| 항목 | 상태 |
| :--- | :--- |
| **앱 설정 저장소**(`settings.json`) | ⭐ 검증용으로 `seek.*` 5개 키와 `presets.capsQuickPress.action` 을 넣었다가, **baseline 사본으로 되돌리고 `diff` 로 글자 그대로 일치함을 확인**했다. 앱을 다시 기동·종료한 뒤에도 그대로였다 |
| **화면 기록 권한** | ⚠️ **바꾸지 않았다.** 시작 시점에 이미 부여돼 있었고(F-03 검증 때부터) 끝날 때도 그대로다 — 부여도 회수도 하지 않았으므로 되돌릴 것이 없다 |
| **경로 B(`hidutil`)** | 검증 전·중·후 모두 `( )`. 잔재 없음 |
| **Karabiner** | ⛔ **건드리지 않았다.** 설정 파일을 읽기만 했다(수정 시각이 검증 전과 같다) |
| 검증용 앱 프로세스 | 전부 종료했다 |

---

## 항목 11 — F-04 Seek 클릭 실행 · 클릭 모드 7종 (M3 4차 / 이슈 #44 신설)

> 대상 명세: `docs/spec/seek-click-execution.md` · 결정 기록: 이슈 #44
> (`docs/dev/f04-plan-draft.md` Part 1 — **클론의 설계 결정, 원본 미검증 항목 포함**).
> 이 항목이 확인하는 것은 **합성 클릭이 실제로 대상에 도달하는가**와 **7종 모드가
> 계획 표(§2.2)대로 실행되는가**다. 자동화된 것(모드 해석·지점·계획·경로·화면 밖
> 판정 — `cargo test -p ultrakey-click`)은 여기서 다시 확인하지 않는다.
>
> ⚠️ **클릭 실행은 워커 스레드에서 일어난다** — 앱 로그(`tail -f
> ~/Library/Logs/Ultrakey/ultrakey.log`)의 `synthesized click:`(debug)·
> `performed AX press on element at`(debug)·`AX press action failed`(warn)·
> `click point ... off-screen`(warn) 라인이 실행 경로의 직접 증거다.
> debug 레벨이 안 보이면 `open --env ULTRAKEY_LOG=ultrakey=debug` 로 켠다(§0-5).

### 11-0. 사전 준비

```sh
# 1) 서명된 .app 을 `open` 으로 (권한이 TCC 에 유지되는 유일한 방법 — §0)
./scripts/build-signed.sh
open <경로>/Ultrakey.app

# 2) Seek 발동 준비 — `Remap key to Seek:` 를 Caps Lock 으로 설정하고
#    `Only show while the remapped key is held`(hold 모드)를 켠다(항목 10 과 동일).
#    설정 UI 가 없으면 settings.json 에 아래 값을 넣었다가 삭제한다(항목 10 의
#    "되돌린 것" 표 참고):
#   seek.remapKey = "CapsLock" · seek.executeOnClose = true

# 3) Safari(또는 시스템 설정)에서 "Save"·"Settings" 식 검색어와 일치하는 보기 좋은
#    버튼/링크를 화면에 띄워 둔다. 매치 프레임이 화면 안에 확실히 보여야 한다.
```

### 11-1. 실제 AX 클릭 (Q19 · D9 — press 가 좌표 클릭과 같은 결과를 내는가)

**준비물**: 항목 11-0 + 두 설정 모두 **OFF**(`seek.focusWindowBeforeClicking =
false`, `seek.changeClickModesWithModifiers = false` — 단, changeModes 는 ☑ 가 출고
기본이므로 명시적으로 끈다).

| 단계 | 조작 | 판정 기준 |
| :--- | :--- | :--- |
| a | Safari 의 버튼 텍스트를 매치로 선택하고 hold 릴리즈로 확정 | 로그에 `performed AX press on element at (x, y)` — 대상 버튼이 눌리고 실제 액션이 일어난다(예: 새 창/화면 이동). 좌표 기반 클릭과 구분이 안 될 만큼 같은 결과면 통과 |
| b | 컨트롤이 **없는** 순수 텍스트(본문 단락의 단어)를 대상으로 같은 조작 | 로그에 `AX press action failed (...); falling back to coordinate click` → 그러고 나서 `synthesized click:` 로 **좌표 폴백**이 실제 클릭을 완성한다. 텍스트에 클릭 결과(커서 위치 이동)가 있으면 통과 |
| c | 대상 앱을 종료시킨 직후(창 닫힘, §5 #1) 확정 | 크래시 없음 · `failed to get AX element at click point; canceling click` 로 **조용히 취소**되고 엉뚱한 창에 클릭이 가지 않는다 |

### 11-2. 포커스 전환 (Focus window before clicking — §8 수용 기준 10·11)

**준비물**: Safari 뒤에 다른 창을 최전면으로 둔다(대상 Safari 창은 **비활성**).

| 단계 | 설정 | 조작·판정 기준 |
| :--- | :--- | :--- |
| a | `focusWindowBeforeClicking` **ON** | Safari 창의 링크를 매치로 확정 → **클릭 1회로** Safari 가 최전면이 되고 클릭까지 도달한다(첫 클릭이 활성화에만 소모되지 않는다). Safari 가 최전면이 됐는지·클릭 결과가 일어났는지 눈으로 확인 |
| b | 상동 | 로그에서 `focus activation timed out`(느린 앱, D8 열화)이 없어야 하고, 있어도 **클릭은 진행**되어야 한다(상한 초과 = 안전판) |
| c | 상동 **OFF** | 같은 조작 → 로그에 `NSRunningApplication` 관련 활성화/`isActive` 흔적이 **전혀** 없어야 한다(§8 수용 기준 11: 활성화 API 미호출). macOS 기본 동작대로 첫 클릭이 활성화에 소모될 수 있음 — 그것이 정상 |

### 11-3. 클립보드 복사 (doubleClickCopy / tripleClickCopy — D11)

**준비물**: 클립보드를 비운다(`pbcopy < /dev/null`), Safari 의 텍스트 본문.

| 단계 | 조작 | 판정 기준 |
| :--- | :--- | :--- |
| a | 매치로 단어 하나를 잡고 **⌘** 를 누른 채 확정(`doubleClickCopy`) | 로그에 `synthesized click:` click_state=1 → 2 → `⌘C`(키 합성은 로그 없음 — 키보드 이벤트) → `pbpaste` 로 그 단어가 그대로 나온다(따옴표 등 macOS 단어 선택 경계 기준). 단어가 **선택되고 복사**됐으면 통과 |
| b | **⌘⌥** 를 누른 채 확정(`tripleClickCopy`) | `pbpaste` 로 줄/문단이 나온다. 통과 |
| c | ⚠️ D11 한계(로그 아님) — `focusWindowBeforeClicking` OFF + **비활성 창** 대상 | 첫 클릭의 click-through 가 없으면 ⌘C 가 최전면 앱으로 갈 수 있다. 이 경우 `pbpaste` 에 대상 텍스트가 안 들어와도 **알려진 열화**로 기록하고 통과 처리 — 이후 원본 대조 항목(11-8)으로 넘긴다 |

### 11-4. 워프 체감 (clickAndReturn / clickReturnClick / onlyMoveCursor — D7·엣지 12)

**준비물**: 매치 프레임과 먼 곳에 커서를 둔다. 단일 디스플레이부터, 가능하면 다중 디스플레이로 반복.

| 단계 | 조작 | 판정 기준 |
| :--- | :--- | :--- |
| a | **⌃** 확정(`clickAndReturn`) | 커서가 매치 지점으로 갔다가 클릭 후 **원래 위치로 돌아온다**. 클릭 자체(창 액션)도 발생한다 |
| b | **⇧** 확정(`clickReturnClick`) | 클릭 → 커서 원위치 복귀 → **원위치에서 재클릭**. 대상 앱에 두 번째 클릭 결과(예: 원래 위치 요소의 액션)가 보인다 |
| c | **⌃⌥** 확정(`onlyMoveCursor`) | 클릭 이벤트가 **전혀 없고**(로그에 `synthesized click` 없음) 커서만 매치 지점으로 이동한다 |
| d | 워프 중 물리 마우스를 흔든다 | 커서가 워프 뒤 물리 마우스 델타로 튀지 않는다(`CGAssociate…` 감싸기). 튀면 로그의 `failed to warp cursor back` 여부와 함께 기록 |
| e | (엣지 12) — 워프 실패를 인위적으로 내기 어려우므로 **코드 리뷰로 확인**한다 | `warp_cursor` 가 실패 시 warn 로그만 남기고 커서가 클릭 지점에 남는 열화를 수용하는지 |

### 11-5. Self-click — 합성 클릭이 탭에 되돌아오지 않는다 (§8 수용 기준 18 · D6)

| 단계 | 조작 | 판정 기준 |
| :--- | :--- | :--- |
| a | 더블클릭(⌘)·트리플클릭(⌘⌥)·클릭-복사 조합을 각각 실행 | 클릭이 **한 번만** 일어난다(대상 앱이 이중 처리하지 않는다 — 예: 링크가 두 번 열리지 않는다). 세션 로그에 합성 이벤트가 콜백에 재진입한 흔적(`seek session opened` 재등장 등)이 없다 |
| b | 클릭 직후 곧바로 사용자 클릭을 한다 | 사용자 클릭이 정상 처리된다(마커가 사라질 상태가 없으므로 영구 소비 상태에 빠지지 않는다 — D6) |

### 11-6. 엣지 — 창 닫힘·화면 밖·드래그 오인·hold 스냅샷

| 단계 | 케이스 | 조작·판정 기준 |
| :--- | :--- | :--- |
| a | 창 닫힘(§5 #1) | 11-1-c 와 동일 — 재확인 불필요(앞서 통과) |
| b | 화면 밖(§5 #7) | 디스플레이 구성을 바꾼 뒤(미러 해제 등) **옛 화면 밖 좌표의 매치를 그대로 확정**하거나, 화면 밖을 가리키도록 검색 바를 옮긴 뒤 확정 → `click point ... off-screen; canceling click` 로 클릭 미실행·크래시 없음. 화면 구성 변경 후엔 검출 좌표도 새 구성 기준이므로 자연 재현이 어려우면 **코드 리뷰**(`point_visible` 단위 테스트)로 갈음하고 기록 |
| c | 드래그 오인(§5 #8) | 11-3-a(더블클릭) 중 커서가 매치를 **드래그**하거나 텍스트를 끌지 않는다(같은 좌표 down/up 이므로 원리적으로 없음 — 이상이면 기록) |
| d | hold 확정 스냅샷(§5 #10) | hold 모드에서 **확정 순간(릴리즈)에 modifier 를 누른 채** 놓는다 → 그 modifier 의 모드가 실행된다. 릴리즈 후에 modifier 를 눌러도 아무 일 없음(`ConfirmedMatch.modifiers` = 릴리즈 순간 스냅샷, F-01 이 전달) |
| e | 느린 포커스 앱(§5 #9) | (가능하면) 앱 정지를 만들어 `focus activation timed out` → 클릭이 진행되는지(11-2-b 와 동일) |

### 11-7. 설정 4조합 — `Focus window...` × `Change click modes...`

| 조합 | 판정 기준 |
| :--- | :--- |
| focus OFF × changeModes ON(출고 기본) | modifier 로 7종 전환이 동작한다(11-3·11-4 의 각 조합) |
| focus OFF × changeModes OFF | 기본 클릭 + modifier 가 클릭 이벤트에 실린다 — **브라우저 링크에서 ⌘+클릭 = 새 탭**으로 열리면 ⌘ 플래그 전달 확인(§8 수용 기준 5) |
| focus ON × changeModes ON | 11-2-a 후 그 modifier 의 모드가 대상 창에 실행된다 |
| focus ON × changeModes OFF | 활성화 + modifier 를 실은 기본 클릭(§3.2 표) |

### 11-8. (원본 대조 — 저장소 필수 검증은 아님) 원본 SuperKey 실기

슈퍼키를 설치한 기기에서 **D1 매핑표(이슈 #44)** 와 **복사 방식(⌘C 합성인지)** 을
실제로 눌러 기록한다. 이 기록이 Q10·D11 의 **최종 교정 수단**이다 — clon 의
매핑표가 원본과 다르면 `docs/spec/seek-click-execution.md` §3.3·
`ultrakey-click/src/mode.rs` 만 고치면 된다(판정·계획 로직과 분리돼 있다).

### ✅ 통과 판정 요약

| 항목 | 확인 수단 |
| :--- | :--- |
| 11-1 a·b·c (AX press·폴백·취소) | 로그 3종 + 실제 액션 |
| 11-2 a·b·c (포커스 ON/OFF) | 클릭 1회 달성 + 로그 |
| 11-3 a·b (⌘C 복사) | `pbpaste` |
| 11-4 a·b·c (워프 3모드) | 커서·클릭 결과 |
| 11-5 (self-click) | 이중 처리 없음 |
| 11-6 b (화면 밖) | 로그 + 크래시 없음 |
| 11-7 (설정 4조합) | 브라우저 새 탭 등 |
| 11-8 (원본 대조) | 📝 기록(필수 검증 아님) |

---

## 14. F-13 자동 업데이트(Sparkle) — 수동 검증

> **검증 전 필수**: Sparkle 은 `.app` 번들이 있어야 동작한다(명세 §7, `tauri-plugin-sparkle-updater` 는
> `NSBundle.mainBundle().bundleIdentifier` 가 비면 `tauri dev` 에서 비활성 — `sparkle_updater()` 가 `None`).
> 반드시 `./scripts/build-signed.sh` 로 서명된 universal `.app` 을 만들어 `open` 으로 실행한다
> (`docs/dev/code-signing.md §5` 개발 루프). 빌드 전 `./scripts/fetch-sparkle.sh`(멱등)가 프레임워크를 받아 둔 상태여야 컴파일이 된다.

사전 준비: `security find-identity -v -p codesigning` 에 `Ultrakey Dev` 가 보이는지, 그리고 `open <경로>/Ultrakey.app` 로 띄운다. 로그는 `tail -f ~/Library/Logs/Ultrakey/ultrakey.log` 로 본다(§0).

### 14-1. 번들에 Sparkle 프레임워크가 실려 있다

| # | 조작 | 기대 결과 |
| :--- | :--- | :--- |
| 1 | `ls <...>/Ultrakey.app/Contents/Frameworks/` | `Sparkle.framework` 디렉터리 존재 |
| 2 | `<...>/Sparkle.framework/Versions/B/Sparkle` 로 `file` | `Mach-O universal binary ... [x86_64] [arm64]` |
| 3 | `codesign --verify --strict --verbose=2 <...>/Sparkle.framework` | 오류 없이 통과(우리 주체로 서명됨) |

### 14-2. Info.plist 에 SU 키가 박혀 있다 (tauri-cli 자동 병합)

| # | 조작 | 기대 결과 |
| :--- | :--- | :--- |
| 1 | `/usr/libexec/PlistBuddy -c 'Print' <...>/Ultrakey.app/Contents/Info.plist` | `SUFeedURL`(자리표시자 https 값) · `SUPublicEDKey`(베이스64 43자) · `SUEnableAutomaticChecks`=`false` · `SUScheduledCheckInterval`=`172800` |
| 2 | CFBundle* 키가 Tauri 가 만든 값 | `CFBundleVersion` 가 `tauri.conf.json version` 과 일치 |

### 14-3. 메뉴바 `Check for Updates…` 동작 (수동 확인)

| # | 조작 | 기대 결과 |
| :--- | :--- | :--- |
| 1 | 앱 로드 후 메뉴바 아이콘 → 메뉴 | `Settings…` 와 `About` 사이에 `Check for Updates…` 항목이 보이고 **활성**(`.app` 이므로) |
| 2 | `Check for Updates…` 클릭 | Sparkle 표준 창이 뜨며 "업데이트 확인 중…" 표시. 자리표시자 URL 이므로 이내 **오류 문구**(retrieve 오류) 또는 "최신 버전" — ⚠️ 서버 없이는 "최신"이 진짜 갱신 유무와 무관 |
| 3 | `.app` 이 아닌 `tauri dev` 로 실행 | 항목이 **비활성**(`bundle::is_running_from_app_bundle` 로 판단) — `.app` 만 활성 |

- 로그: `~/.herdr/.../ultrakey.log` 에 `manual check for updates started`(영어) 남는지 확인.
- Sparkle 자체 로그: `log stream --predicate 'subsystem == "org.sparkle-project.Sparkle"' --info`(명세 §6; `.app` 실행 시 콘솔 확인용).

### 14-4. General 탭 자동 확인 체크박스 (출고 기본 OFF)

| # | 조작 | 기대 결과 |
| :--- | :--- | :--- |
| 1 | 환경설정 General 탭 | `Check for updates automatically` 체크박스가 **활성**(비활성·배지 아님)이고 **☐**(꺼짐) 가 기본 |
| 2 | 체크하면 | `defaults read app.ultrakey.Ultrakey SUEnableAutomaticChecks` → `1`(UserDefaults 로 저장, Sparkle 이 실제로 읽는 값) |
| 3 | 재실행 | 체크 상태가 ☑ 로 유지(Sparkle UserDefaults 영속) |
| 4 | 다시 해제 | `defaults read ... SUEnableAutomaticChecks` → `0` |

> ⚠️ **F-15 와의 관계**: 이 체크박스는 `SettingsStore`(settings.json)에 **저장하지 않는다** — 정본은
> Sparkle 의 UserDefaults 이고, `settings.json` 에 이 키를 쓰지 않아 "설정을 안 건드리면 파일이 안 생긴다"
> (F-15 §8)도 지켜진다.

### 14-5. 자동 확인이 실제 48h 로 동작하는가 (배경 주기)

서버가 실재할 때만 확인 가능한 항목이다(명세 §3.2). 자동 확인이 켜진 상태에서 배경 조회가
`SUScheduledCheckInterval`(172800s) 만큼 떨어져 발생하는지 로그/Sparkle os_log 로 관찰한다.
⚠️ 서버 미구성이므로 **이번 위임에서는 미검증 항목**이다 — `docs/dev/auto-update-server.md` 의 호스팅 절차 뒤에 수행할 것.

### 14-6. 실기기 전체 왕복 (다운로드→서명 검증→설치→재시작)

명세 §8 의 핵심(서명 검증 실패 시 설치 금지 · 신·구 서명 주체 일치 · 재시작 전 modifier 해제)은
**실제 appcast + DMG + 개인키가 있어야** 확인된다. 서버 준비(`docs/dev/auto-update-server.md`) 후:

| # | 조작 | 기대 결과 |
| :--- | :--- | :--- |
| 1 | 신버전 appcast + DMG 를 https 호스트에 배포, `SUFeedURL` 을 그 URL 로 교체해 빌드 | `Check for Updates…` 에 "신버전 있음 + 릴리스 노트" |
| 2 | 정상 경로로 설치 | 설치 후 재시작. 신규 `.app` 이 이전과 **같은 서명 주체** — Accessibility 등 **권한 재승인 없이 유지** |
| 3 | ⛔ 서명을 **위조**한 DMG(appcast edSignature 뒤섞기) | `"The update is improperly signed..."` 오류, **설치 거부**, 기존 `.app` 보존 |
| 4 | 재시작 직전 눌린 modifier | 정리 로그(`will-relaunch` → `engine cleaned up before Sparkle update relaunch`) — stuck 없음 |
| 5 | 오프라인 앱cast 파싱 실패 | 크래시 없이 다음 주기/오류만(§8) |

> ⚠️ **14-6 전체는 실기기 미검증** — 서버가 없어 이번 위임에서 실행하지 못했다. 절차만 확정된 상태다.

## 15. F-12 라이선싱과 트라이얼 — 수동 검증 (이슈 #59)

> ⭐ 명세 §7 결정 — 실제 Paddle 은 붙이지 않고 **no-op `LicenseProvider`(항상 라이선스
> 활성)** 를 기본으로 한다. 그래서 자동화 가능한 검증( 상태 머신·오프라인 유예·시계
> 조작 완화·기산점 계산)은 전부 `ultrakey-license` 의 단위 테스트로 이미 통과했고,
> 아래는 그걸로 검증할 수 없는 **실기기 전용** 항목이다 — Keychain(`Security.framework`)
> 저장 진짜 살아남는가, IOPlatformUUID 가 실제로 읽히는가, General 탭 UI 가 진짜
> 그려지는가.

### 15-1. 사전 준비

- 서명된 `.app` 번들로 실행한다(`./scripts/build-signed.sh`). `tauri dev` 로는 부모
  프로세스 권한 문제가 있어 General 탭이 온전히 동작하지 않을 수 있다.
- `ULTRAKEY_LOG=debug` 로 실행해 라이선스 판정 로그(`license state evaluated`)를 본다.

### 15-2. 체험 기산점이 Keychain 에 기록되는가(§3.5, §5-10)

| # | 조작 | 기대 결과 |
| :--- | :--- | :--- |
| 1 | 최초 실행(Keychain 에 항목 없음) | 로그에 `license state evaluated state=trial days_remaining=20` |
| 2 | General 탭에서 라이선스 상태 확인 | "체험판 — N일 남음" 표시(no-op 활성이라 상태가 라이선스 활성이면 no-op 경로 정상 — 아래 15-3 참고) |
| 3 | ⭐ Keychain 항목 존재 확인 | `security find-generic-password -s com.ultrakey.license.trial` — `trial_record` JSON 이 담겨 있다 |
| 4 | **앱을 완전히 삭제(`~/Library/Application Support` 포함) 후 재설치** | 같은 기기에서 기산점이 초기화되지 않는다 — Keychain 에 살아 있음(§8-3). `security find-generic-password` 로 같은 `trial_started_at` 확인 |
| 5 | `security delete-generic-password -s com.ultrakey.license.trial` 후 재실행 | 체험이 리셋된다(받아들이는 잔여 위험, §3.5) |

> ⚠️ **no-op 경로 주의점**: 현재 빌드는 항상 라이선스 활성이므로 `evaluate_on_start` 가
> Keychain 에 캐시가 없어도 체험 판정에 도달하지만, `NoopLicenseProvider` 를 주입해
> 라이선스 활성이 우선한다(§3.2 "최우선"). 그래서 15-2 는 **Keychain 에 trial 항목이
> 기록되는가**가 핵심이지, 화면에 "체험판"이 보이는가는 아니다 — 화면은 no-op 이라
> 항상 라이선스 활성으로 표시될 수 있다. 체험 UI 배선을 눈으로 보려면 no-op 을 끄는
> 진단 플래그(현재 없음)가 필요하다 — 그 전까지는 General 탭 상태 표시가
> "License active" 로 그려지는지가 배선 검증이다.

### 15-3. General 탭 라이선스 UI 배선(§4.2)

| # | 조작 | 기대 결과 |
| :--- | :--- | :--- |
| 1 | 서명된 앱 실행 → General 탭 | "License active" 표시 + "Licensed" 배지 + "이 기기 비활성화" 버튼 활성 |
| 2 | "이 기기 비활성화" 클릭 | 확인 대화상자("이 기기를 비활성화할까요?")가 먼저 뜨고, `Deactivate` 를 눌러야만 실행(§5-13) |
| 3 | 확인 후 | no-op: `Deactivated` → 라이선스 상태가 재평가됨(no-op 은 다시 활성으로) — 로그에 `license state evaluated state=licensed` |
| 4 | 라이선스 키 입력란에 아무 키/빈 값 후 "Activate" | 빈 값이면 "Enter a license key first."; 키 있으면 no-op `activated` |

### 15-4. IOPlatformUUID 가 실제로 읽히는가(§3.6, §8 실기기)

| # | 조작 | 기대 결과 |
| :--- | :--- | :--- |
| 1 | 로그에 device_id 확인 | `activate`/`deactivate` 시 no-op 이 device_id 를 받는다. 활성화 성공 로그에 기기 슬롯 `0/3` |
| 2 | (원본 대조 — 저장소 필수 검증 아님) `ioreg -rd1 -c IOPlatformExpertDevice` | `IOPlatformUUID` 값이 로그/Keychain 의 device_id 와 일치해야 한다 |

> ⚠️ **IOPlatformUUID 는 부팅마다 바뀔 수 있다**(애플이 문서화하지 않은 동작). 이게
> 재부팅 후에도 안정적인지는 이 위임에서 검증하지 못했다 — 하드웨어 교체 = 새 슬롯
> 소모라는 §3.6 동작이 성립하려면 부팅 간 안정이 필요한데, `(미확정)` 이다.

### 15-5. 이 절차로도 확인할 수 **없는** 것

- **오프라인 유예(§3.4)가 실제 캐시에서 동작하는가** — no-op 은 항상 활성이라 유예
  경계를 재현할 방법이 없다. 이는 `ultrakey-license::machine` 단위 테스트로 검증됐다.
- **시계 조작 완화(§3.5/§3.6)가 실제 Keychain `last_seen_at` 으로 동작하는가** — 같은
  이유. 단위 테스트로 검증됐다.
- **`limit_reached`(3대) — 4번째 기기에서의 활성화 거부** — no-op 이 항상 `activated` 라
  도달 불가. §8-6 은 no-op 하에서 구조적으로 성립이 불가능하며, Paddle 실연동
  시점에 재검증한다(명세 §7).
- **refunded/revoked 즉시 무효(§8-8)의 실제 서버 응답 경로** — 서버가 없어 도달 불가.
  `ultrakey-license` 단위 테스트(`refunded_revalidation_clears_cache_and_invalidates`)로
  검증됐다.

### ✅ 통과 판정 요약

| 항목 | 자동화 | 실기기 |
| :--- | :--- | :--- |
| 기산점 최초 기록 → 체험 중 | ✅ 단위 테스트 | ✅ 15-2 (Keychain 기록) |
| 20일 경과 → 체험 만료 | ✅ 단위 테스트 | ⚠️ 키보드를 20일 돌릴 수 없어 미검증 |
| 재설치에도 기산점 유지 | — | ✅ 15-2 #4 (실기기 전용) |
| 유효키 활성화 → 라이선스 활성 + 캐시 저장 | ✅ 단위 테스트 | ⚠️ no-op 라 서버 검증 불가 |
| 오프라인 유예 유지 | ✅ 단위 테스트 | (no-op) |
| refunded/revoked 즉시 무효 | ✅ 단위 테스트 | (no-op) |
| 무효 → 체험만료 재전이(체험중 복귀 금지) | ✅ 단위 테스트 | — |
| 시계 과거 되돌려도 남은 일수 증가 금지 | ✅ 단위 테스트 | ⚠️ 실기기로 시계 조작 재현 어려움 |
| 최소 식별자(검증 페이로드 필드 제한) | ✅ 구조 검증 | — |
| 비활성화 확인 대화상자 | ✅ 로직 | ✅ 15-3 #2 |
| IOPlatformUUID 취득 | — | ⚠️ 15-4 (로그로 확인) |
| `limit_reached`·3대 제한 | ⛔ no-op 하 도달 불가 | Paddle 연동 시점에 재검증 |

