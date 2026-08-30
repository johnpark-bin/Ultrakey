# 코드 서명 워크플로 — M0

> ⚠️ **이 문서가 만들어진 세션은 서명 인증서를 생성하지 않았고, 서명된 빌드를 실행해 검증하지도 않았다.**
> 아래 절차는 `docs/spec/platform-constraints.md` §3(TCC 권한과 코드 서명) · `docs/spec/permissions-onboarding.md` §7(개발 워크플로 요구사항) · Apple 공식 문서에 근거해 작성했다. 키체인·시스템 설정을 바꾸는 단계는 **사용자가 직접 실행**해야 하며(§2 참조), 이 세션은 읽기 전용 확인 명령만 실행했다.
> 확인된 환경(2026-08-30, 이 워크트리): macOS 26.5.2 / Apple Silicon, Xcode 26.6 전체 설치(`xcode-select -p` → `/Applications/Xcode.app/Contents/Developer`), `rustc 1.96.1` / `cargo 1.96.1`, `security find-identity -v -p codesigning` → **`0 valid identities found`**(서명 인증서 없음), `cargo tauri` 미설치. `rustup target list --installed` 는 이 워크트리에서 `aarch64-apple-darwin` 과 `x86_64-apple-darwin` **둘 다** 확인됐다 — 다른 환경에서는 `x86_64-apple-darwin` 이 빠져 있을 수 있으므로 §3 의 확인 절차를 그대로 따른다.

---

## 1. 왜 이게 1일차에 필요한가

`docs/spec/platform-constraints.md` §3.2 는 TCC(권한) 데이터베이스가 권한을 **`(번들 ID, 코드 서명 요구사항 csreq)`** 쌍으로 키잉한다고 확정한다.

| 서명 상태 | 권한 부여 | 재빌드 후 유지 | 다른 기기에서 유지 | 이 프로젝트에서의 용도 |
| :--- | :---: | :---: | :---: | :--- |
| 미서명 | ❌ 사실상 불가 | — | — | 쓰지 않는다 |
| ad-hoc (`--sign -`) | ✅ | ❌ **매 빌드 소실** | ❌ | 쓰지 않는다 |
| **자체 서명 인증서 (고정)** | ✅ | ✅ | ❌ | ⭐ **개발 중 표준** |
| **Developer ID** | ✅ | ✅ | ✅ | ⭐ **배포용** |

ad-hoc 서명(`codesign --sign -`)은 안정적인 서명 주체가 없어 `csreq` 가 바이너리의 **`cdhash`** 에 묶인다. 소스를 한 줄만 고쳐도 `cdhash` 가 바뀌고, macOS 는 이를 다른 앱으로 인식한다 → **재빌드할 때마다 Accessibility·Screen Recording 등 이미 부여받은 권한이 전부 사라진다.**

또한 §3.4 는 `tauri dev` 의 함정을 지적한다. `tauri dev` 는 `.app` 번들이 아니라 `target/debug/<binary>` 를 직접 실행하므로, 번들 ID 가 없거나 달라 TCC 가 **부모 프로세스(터미널·IDE)의 권한**으로 판정한다. 터미널에 접근성 권한을 준 상태에서 `tauri dev` 로 테스트하면 "동작하는 것처럼 보이지만" 실제 배포 조건(서명된 `.app` 단위 권한 판정)과 다르다.

이 두 가지를 1일차에 세팅하지 않으면, F-07(키 리매핑 엔진)·F-11(권한 온보딩) 등 권한이 필요한 기능을 구현하는 내내 "권한이 됐다 안 됐다" 를 서명 문제 때문에 오판하게 되고, **그 전까지 쌓은 검증이 전부 무효**가 된다.

---

## 2. ⭐ 사용자가 직접 해야 하는 단계

**여기서부터는 키체인·시스템 설정을 변경하는 작업이라, 위임받은 세션이 대신 실행하지 않는다.** 아래를 사용자가 직접 수행해야 한다.

### 2.1 자체 서명 코드 서명 인증서 생성 (GUI, 권장 경로)

1. **키체인 접근(Keychain Access)** 앱을 연다(`/Applications/Utilities/Keychain Access.app`, Spotlight 검색: "키체인 접근").
2. 메뉴에서 **키체인 접근 → 인증서 지원(Certificate Assistant) → 인증서 생성...(Create a Certificate...)** 을 선택한다.
3. 다음과 같이 입력한다:
   - **이름(Name)**: `Ultrakey Dev`
   - **신원 유형(Identity Type)**: **자체 서명 루트(Self Signed Root)**
   - **인증서 유형(Certificate Type)**: 드롭다운에서 **코드 서명(Code Signing)** 선택 — ⚠️ 기본값인 "SSL 클라이언트" 등이 아니라 반드시 "코드 서명" 이어야 한다.
   - "이 인증서에 대해 기본값 재정의(Let me override defaults)" 는 체크하지 않아도 된다(기본 유효기간으로 충분).
4. **생성(Create)** 을 눌러 완료한다. 완료 후 "로그인(login)" 키체인의 "내 인증서" 분류에 `Ultrakey Dev` 가 나타난다.

### 2.2 인증서를 "항상 신뢰" 로 설정

1. 키체인 접근에서 방금 만든 `Ultrakey Dev` 인증서를 더블클릭한다.
2. 펼쳐지는 상세 창에서 **신뢰(Trust)** 섹션을 연다.
3. **이 인증서를 사용할 때(When using this certificate)** 드롭다운을 **항상 신뢰(Always Trust)** 로 바꾼다.
4. 창을 닫으면 관리자 암호 확인 창이 뜬다 — 암호를 입력해 변경 사항을 저장한다.

이 단계를 건너뛰면 서명 시 `errSecInternalComponent` 류의 오류가 나거나, 서명은 되어도 codesign 이 신뢰 체인 경고를 낸다(§4).

### 2.3 ⚠️ GUI 대신 CLI 를 쓰고 싶다면 — 참고용, 이 세션은 실행하지 않았다

GUI 조작을 CLI 로 대체하려면 대략 아래와 같은 명령이 필요하다(자체 서명 인증서를 만들고 키체인에 넣고 "항상 신뢰" 로 설정):

```bash
# 1) 자체 서명 코드 서명 인증서를 openssl 로 만든다
openssl req -x509 -newkey rsa:2048 -keyout ultrakey-dev.key -out ultrakey-dev.crt \
  -days 3650 -nodes -subj "/CN=Ultrakey Dev"
openssl pkcs12 -export -out ultrakey-dev.p12 -inkey ultrakey-dev.key -in ultrakey-dev.crt -passout pass:

# 2) 로그인 키체인에 개인 키+인증서로 임포트한다
security import ultrakey-dev.p12 -k ~/Library/Keychains/login.keychain-db -T /usr/bin/codesign -P ""

# 3) 코드 서명 확장(EKU)을 인증서에 붙이고, 항상 신뢰로 설정한다
#    (openssl 로 만든 인증서는 기본적으로 코드 서명 EKU 가 없으므로 GUI 경로보다 손이 더 간다)
security add-trusted-cert -d -r trustAsRoot -k ~/Library/Keychains/login.keychain-db ultrakey-dev.crt
security set-key-partition-list -S apple-tool:,apple: -k "" ~/Library/Keychains/login.keychain-db
```

⚠️ **이 명령들은 키체인을 직접 변경하므로 이 세션은 실행하지 않았고, 검증도 하지 않았다.** `openssl` 로 만든 인증서는 "코드 서명" 확장이 GUI 경로처럼 자동으로 붙지 않아 추가 작업이 필요할 수 있다 — 대부분의 경우 **2.1의 GUI 경로가 더 간단하고 확실하다.** CLI 경로를 쓰기로 했다면 §3.1 의 검증 명령으로 반드시 결과를 확인한다.

---

## 3. 각 단계의 검증 방법

| # | 확인할 것 | 실행 명령 | 성공 기준 |
| :--- | :--- | :--- | :--- |
| 3.1 | 서명 인증서가 키체인에 있고 코드 서명 용도로 쓸 수 있다 | `security find-identity -v -p codesigning` | 목록에 `"Ultrakey Dev"` 가 보인다. ⭐ 이 문서 작성 시점(2026-08-30)에는 이 워크트리에서 `0 valid identities found` — 즉 아직 없다 |
| 3.2 | universal 빌드에 필요한 두 rustup 타깃이 설치되어 있다 | `rustup target add x86_64-apple-darwin` 실행 후 `rustup target list --installed` | 출력에 `aarch64-apple-darwin` 과 `x86_64-apple-darwin` 이 **둘 다** 보인다 |
| 3.3 | `tauri-cli` 가 설치되어 있다 | `cargo install tauri-cli --version "^2"` 실행 후 `cargo tauri --version` | 버전 문자열이 출력된다(에러 없이 종료) |
| 3.4 | 빌드된 `.app` 이 올바르게 서명되었다 | `codesign -dv --entitlements :- Ultrakey.app` | 출력에서 다음을 확인: `flags=0x10000(runtime)` (Hardened Runtime 활성), `Authority=Ultrakey Dev` (서명 주체가 우리 인증서), `Identifier=<고정 번들 ID>` (번들 ID 가 고정값과 일치) |
| 3.5 | universal 바이너리다 | `lipo -archs Ultrakey.app/Contents/MacOS/<실행파일명>` | 출력이 `x86_64 arm64` (순서 무관, 두 아키텍처 모두 포함) |
| 3.6 | 서명 무결성이 깨지지 않았다 | `codesign --verify --deep --strict --verbose=2 Ultrakey.app` | 오류 없이 종료(`valid on disk` / `satisfies its Designated Requirement` 류의 메시지) |

---

## 4. 실패 시 증상과 대처

| 증상 | 원인 | 대처 |
| :--- | :--- | :--- |
| **재빌드할 때마다 이미 부여한 권한(Accessibility 등)이 사라진다** | ad-hoc 서명(`--sign -`)을 쓰고 있다. `cdhash` 가 빌드마다 바뀐다 | `scripts/build-signed.sh` 로 고정된 `Ultrakey Dev` 인증서로 서명한다. §1 참조 |
| **권한을 켰는데 리매핑이 동작하지 않는다** | 권한 DB 가 어긋난 상태(out-of-sync) — `AXIsProcessTrusted()==true` 인데 실제 API 호출은 실패하는 경우 | `docs/spec/permissions-onboarding.md` §3.3(F-11) 의 수동 절차를 따른다: 시스템 설정의 권한 목록에서 앱을 **제거** → 앱 재실행 → 프롬프트에서 **다시 승인** |
| **서명할 때 `errSecInternalComponent` 가 나거나 인증서를 못 찾는다** | 인증서가 "코드 서명(Code Signing)" 유형이 아니거나, "항상 신뢰(Always Trust)" 로 설정되지 않았다 | §2.1 에서 인증서 유형을 다시 확인하고, §2.2 의 신뢰 설정을 다시 한다 |
| ⭐ **`open` 이 `kLSNoExecutableErr: The executable is missing` 로 실패한다** (실행 파일은 멀쩡히 있는데도) | **메시지가 원인을 오도한다.** 실제 원인은 실행 파일 부재가 아니라 **서명을 신뢰할 수 없는 것**이다 — 인증서가 키체인에서 사라졌거나, 로그인 키체인이 잠겼거나, "항상 신뢰" 가 풀렸다. LaunchServices 는 서명을 신뢰하지 못하면 이 오류를 낸다 | **진단 순서** ① `security find-identity -v -p codesigning` — `Ultrakey Dev` 가 보이는가 ② `codesign --verify --deep --strict <앱>` — `CSSMERR_TP_NOT_TRUSTED` 가 나오면 신뢰 문제가 확정이다. 대처는 §2.1·§2.2 를 다시 수행하고 **재빌드**한다(없어진 인증서로 서명된 번들은 되살릴 수 없다). ⚠️ **`security find-identity` 는 실행하는 셸의 보안 세션에 따라 결과가 다를 수 있다** — 다른 세션(예: 자동화 도구)에서 `0 valid identities found` 가 나와도 사용자 터미널에서는 보일 수 있으니, 사용자 터미널에서 직접 확인한다 |
| ⭐ **로그아웃/로그인 뒤부터 `open` 이 위 오류로 실패한다** | 실측(2026-08-30)에서 로그아웃·로그인을 거친 뒤 이 증상이 나타났다. 정확한 인과는 확정하지 못했다 — 키체인 잠금 또는 신뢰 설정 소실로 추정한다 `(미확정)` | 세션 전환 뒤에는 **검증을 시작하기 전에 §3.1 을 한 번 확인**한다. 실패하면 §2 를 다시 수행하고 재빌드한다 |
| **`The application "Ultrakey" cannot be opened`** | Gatekeeper 가 자체 서명(비-Developer ID) 앱을 차단한다 | 개발 중에는 Finder 에서 앱을 **우클릭 → 열기(Open)** 로 첫 실행을 허용하거나, `xattr -d com.apple.quarantine Ultrakey.app` 으로 quarantine 속성을 제거한다 |
| **`tauri dev` 로 테스트했을 땐 되던 권한 기능이 `tauri build` 산출물에서는 안 된다** | §3.4(원본 문서 §3.4) 그대로다. `tauri dev` 는 `.app` 번들이 아니라 `target/debug/<binary>` 를 직접 실행해 **부모 프로세스(터미널)의 권한**으로 판정된다 | `tauri dev` 로는 권한 기능을 애초에 테스트하지 않는다. 반드시 `tauri build` → 서명 → 실행 루프를 쓴다(§5) |

---

## 5. 개발 루프

```
cargo tauri build --target universal-apple-darwin
        ↓
codesign --force --deep --options runtime \
  --entitlements apps/ultrakey-app/entitlements.plist \
  --sign "Ultrakey Dev" <산출 .app>
        ↓
open <산출 .app>   (또는 Finder 에서 직접 실행)
```

`scripts/build-signed.sh` 가 이 세 단계를 자동화한다. `scripts/verify-signature.sh <.app 경로>` 로 서명·권한 관련 상태를 언제든 다시 진단할 수 있다.

**⚠️ `tauri dev` 로는 권한 기능(F-01~F-08, F-11, F-13)을 테스트하지 않는다.** `tauri dev` 는 `.app` 번들을 만들지 않고 서명도 거치지 않으므로, 권한 판정이 실제 배포 조건과 다르게 나온다(§1, §4). 권한과 무관한 UI 레이아웃·상태 관리 등을 빠르게 반복할 때만 `tauri dev` 를 쓴다.

---

## 6. 불변 조건

- **번들 ID(Bundle Identifier)를 개발 중에도 고정한다.** 아직 `apps/ultrakey-app/tauri.conf.json` 이 만들어지지 않았으므로(이 문서 작성 시점), 그 파일을 만드는 위임 건에서 `identifier` 값을 한 번 정하면 이후 절대 바꾸지 않는다. 값이 바뀌면 macOS 는 새 앱으로 취급해 이미 부여된 모든 권한이 사라진다.
- **배포 시 Developer ID 로 교체할 때도 같은 Team ID 를 유지해야 한다.** `docs/spec/platform-constraints.md` §3.5 — 자동 업데이트(F-13)가 `.app` 을 새 버전으로 교체할 때, 신·구 앱의 서명 주체(Team ID)와 번들 ID 가 같아야 TCC 권한이 유지된다. 서명 인증서를 갱신(만료 등)할 때 Team ID 가 바뀌면 **모든 사용자가 권한을 다시 부여**해야 하므로, 이것은 릴리스 파이프라인의 불변 조건으로 못박아 둔다.

---

## 7. 원본 실측과의 대조 — 우리가 맞춰야 할 구성

`docs/spec/platform-constraints.md` §3.6 은 실제 배포 중인 SuperKey v1.66 을 `codesign -dv --entitlements :-` 로 실측한 결과다. 클론도 이 구성을 따른다.

| 항목 | SuperKey v1.66 실측 | 우리가 맞출 값 |
| :--- | :--- | :--- |
| 서명 | Developer ID (Team ID `XSYZ3E4B7D`) | 개발 중: 고정 자체 서명 인증서(`Ultrakey Dev`). 배포 시: Developer ID |
| Hardened Runtime | 활성 (`flags=0x10000(runtime)`) | ✅ 활성 — `codesign --options runtime` |
| App Sandbox | **없음** | ✅ 없음 — `CGEventTap`·`AXUIElement`·IOHID·비공개 프레임워크는 샌드박스에서 쓸 수 없다. 이 때문에 Mac App Store 배포가 불가능하고, 직접 배포(+ Sparkle)가 유일한 경로다 |
| Entitlements | **`com.apple.security.cs.allow-jit` 하나뿐** | ✅ 동일 — `apps/ultrakey-app/entitlements.plist` 에 이 키 하나만 선언한다 |
| 아키텍처 | Universal (`x86_64` + `arm64`) | ✅ 동일 — `cargo tauri build --target universal-apple-darwin` + `lipo` |

⭐ 여기서 확인해야 할 것 하나: **TCC 권한은 entitlement 로 선언되지 않는다.** Accessibility·Screen Recording·Input Monitoring 은 `entitlements.plist` 어디에도 나타나지 않으며, "어떤 권한이 필요한가" 는 번들 메타데이터가 아니라 **실제로 링크·호출하는 API** 로만 판정된다(`platform-constraints.md` §0.1). `com.apple.security.cs.allow-jit` 은 WebKit(Sparkle·Paddle 이 링크) 이 요구하는 것으로 보이나 확정되지 않았다(`(미확정)`).

---

## 8. 릴리즈 파이프라인 시크릿 (CI)

`.github/workflows/release.yml`(이슈 #50)은 `v*` 태그 푸시를 트리거로 유니버설 DMG 빌드를 실행한다. 서명·공증 시크릿이 있으면 Tauri 번들러가 **임시 키체인을 스스로 생성·서명·공증·스테이플**까지 처리하므로 §2 같은 수동 키체인 단계가 필요 없다. 시크릿이 없으면 **미서명 빌드로 통과한다(경고만, 실패하지 않는다)**. 그 결과물로 드래프트 릴리즈를 만든다.

서명·공증은 전부 Tauri 번들러가 **환경 변수에서 읽어** 수행하고, 워크플로는 시크릿을 환경에 주입하고 불완전한 그룹을 걸러내는 일만 한다. 설정해야 하는 시크릿은 다섯 개다.

| 시크릿 | 용도 | 준비 절차 |
| :--- | :--- | :--- |
| `APPLE_CERTIFICATE` | **Developer ID Application** `.p12`(인증서 + 개인 키) 의 base64 | 키체인 접근 → 내 인증서 → Developer ID Application 인증서(+개인 키) 우클릭 → 내보내기(`.p12`, 비밀번호 지정) → `openssl base64 -in cert.p12 -out cert.b64`(Tauri 공식 문서 절차) → 파일 내용 전체를 시크릿으로 |
| `APPLE_CERTIFICATE_PASSWORD` | 위 단계에서 지정한 `.p12` 내보내기 비밀번호 | 위 절차의 그 비밀번호 그대로 |
| `APPLE_ID` | 공증에 쓰는 Apple 계정 이메일 | Apple ID 이메일 그대로 |
| `APPLE_PASSWORD` | ⚠️ **앱 전용 암호(app-specific password)** — 계정 암호나 2FA 코드가 아니다 | https://support.apple.com/HT204397 에서 생성 |
| `APPLE_TEAM_ID` | 개발자 멤버십 페이지의 Team ID | https://developer.apple.com/account#MembershipDetailsCard |

⭐ **그룹 규칙: 두 시크릿 그룹은 각각 전부 아니면 전무(all-or-nothing)다.** 서명 그룹은 `APPLE_CERTIFICATE` + `APPLE_CERTIFICATE_PASSWORD` 이고, 공증 그룹은 `APPLE_ID` + `APPLE_PASSWORD` + `APPLE_TEAM_ID` 다. 파이프라인은 불완전한 그룹을 감지해 **경고와 함께 환경에서 제거**한다(실패하지 않는다). 결과 상태는 세 가지다:

- **다섯 개 모두 설정** → 서명 + 공증
- **서명 그룹만 설정** → 서명만 — ⚠️ 공증이 없으면 다른 기기의 첫 실행에서 Gatekeeper 가 경고한다 (공증 전까지)
- **하나도 설정 안 함** → 미서명

이 게이트가 필요한 이유: 번들러는 환경 변수의 **존재**(`var_os`)로 서명 여부를 판정하므로, **빈 문자열도 "존재"로 잡혀** 빈 인증서 임포트를 시도해 실패한다. 또 공증 자격이 부분적으로만 있으면(`APPLE_ID` + `APPLE_PASSWORD` 는 있고 `APPLE_TEAM_ID` 가 없음) `MissingTeamId` 로 하드 실패한다 — 근거: `crates/tauri-bundler/src/bundle/macos/{sign,app}.rs`.

**필요 없는 시크릿** (명시적으로):
- `APPLE_SIGNING_IDENTITY` — 인증서에서 자동으로 추론된다. 문제가 생겨 신원을 고정하고 싶을 때만 이 값(또는 `bundle.macOS.signingIdentity`)을 지정한다.
- `KEYCHAIN_PASSWORD` — 번들러가 임시 키체인을 직접 만들고 지우므로 필요 없다. 수동 키체인 임포트가 들어간 옛 문서 예시는 레거시 흐름용이다.

⭐ **Team ID 불변 조건**: §6 과 같다 — 인증서를 갱신(만료 등)할 때 **같은 Team ID** 로 재발급한다. Team ID 가 바뀌면 모든 기존 사용자가 TCC 권한을 잃는다(F-13 자동 업데이트 전제 조건, `docs/spec/platform-constraints.md` §3.5).

**등록 절차**: `gh secret set APPLE_CERTIFICATE < cert.b64`(시크릿마다 반복) 또는 Settings → Secrets and variables → Actions. ⛔ **시크릿 값을 어떤 저장소 파일에도 절대 써넣지 않는다** — 이 문서는 절차만 다룬다.

**첫 릴리즈 / 미서명 교체 절차**: 미서명 드래프트 릴리즈로 파이프라인 자체를 검증하는 것은 문제없다. 시크릿을 설정한 뒤 서명된 빌드로 교체하려면 → 드래프트 릴리즈 삭제 → 원격 태그 삭제 → **같은 태그를 다시 푸시**(워크플로 재실행).
