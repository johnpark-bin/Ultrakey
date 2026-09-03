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
| ad-hoc (`--sign -`) | ✅ | ❌ **매 빌드 소실** | ❌ | 로컬 개발에서는 쓰지 않는다. ⚠️ **예외**: CI 릴리즈 파이프라인(§8)은 서명 시크릿이 없을 때 미서명 배포(TCC 영구 인가 실패, 이슈 #123) 대신 이 방식으로 **폴백**한다 — "쓰지 않는다"가 아니라 "임시 완화(최후 수단)"다 |
| **자체 서명 인증서 (고정)** | ✅ | ✅ | ❌ | ⭐ **개발 중 표준**. §8 의 CI 시크릿으로 등록하면 릴리즈에서도 같은 이점(재인가 불필요)을 얻는다 — ad-hoc 은 임시 완화일 뿐 이쪽이 **항구적 해결**이다 |
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
| **`The application "Ultrakey" cannot be opened`** | Gatekeeper 가 자체 서명(비-Developer ID)·ad-hoc 앱을 차단한다 | ⚠️ **macOS 15+ 에서는 Finder 우클릭 → 열기로 우회할 수 없다**(Sequoia 부터 이 경로가 제거됨). 차단 화면이 뜬 뒤 **시스템 설정 → 개인정보 보호 및 보안 → 하단의 "그래도 열기"** 버튼을 눌러 허용하거나, `xattr -dr com.apple.quarantine Ultrakey.app` 으로 quarantine 속성을 제거한 뒤 다시 실행한다 |
| ⭐ **다른 기기에 설치된 미서명 빌드에서 Accessibility 등 권한이 몇 번을 다시 켜도 계속 실패한다** (이슈 #123) | 이슈 #123 수정 전 릴리즈(미서명 배포)를 설치한 기기다. TCC 가 저장한 code requirement 와 실행 중 프로세스의 실제 identity 가 구조적으로 영원히 어긋난다 — 재설치·`tccutil reset` 으로도 해결 불가 | **권장(영구 해결)**: ① 이슈 #123 수정 이후 릴리즈(서명됨)를 재설치 → ② **시스템 설정 → 개인정보 보호 및 보안 → 손쉬운 사용(Accessibility)** 목록에서 Ultrakey 항목을 **제거**(단순 토글 끄기 아님) → ③ 앱 재실행 → 프롬프트에서 **다시 승인**. **임시(급할 때만)**: 로컬에서 `codesign --force --deep --sign - /Applications/Ultrakey.app` 으로 ad-hoc 재서명 후 `tccutil reset Accessibility app.ultrakey.Ultrakey` → 재실행 후 재승인(다음 자동 업데이트에서 다시 깨진다 — ad-hoc 은 재빌드마다 identity 가 바뀐다) |
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
| Entitlements | **`com.apple.security.cs.allow-jit` 하나뿐** | ⚠️ 개발 중(자체 서명)에는 `com.apple.security.cs.disable-library-validation` 도 함께 선언한다 — 이유는 §9 참조. Developer ID 배포로 넘어가면 이 키는 다시 뺄 수 있다(§9) |
| 아키텍처 | Universal (`x86_64` + `arm64`) | ✅ 동일 — `cargo tauri build --target universal-apple-darwin` + `lipo` |

⭐ 여기서 확인해야 할 것 하나: **TCC 권한은 entitlement 로 선언되지 않는다.** Accessibility·Screen Recording·Input Monitoring 은 `entitlements.plist` 어디에도 나타나지 않으며, "어떤 권한이 필요한가" 는 번들 메타데이터가 아니라 **실제로 링크·호출하는 API** 로만 판정된다(`platform-constraints.md` §0.1). `com.apple.security.cs.allow-jit` 은 WebKit(Sparkle·Paddle 이 링크) 이 요구하는 것으로 보이나 확정되지 않았다(`(미확정)`).

---

## 8. 릴리즈 파이프라인 시크릿 (CI)

`.github/workflows/release.yml`(이슈 #50, 서명 폴백은 이슈 #123)은 `v*` 태그 푸시를 트리거로 유니버설 DMG 빌드를 실행한다. 서명은 Tauri 번들러에게 맡기지 않는다 — build-signed.sh(§5)와 동일하게 `tauri build`(번들만) 뒤 워크플로가 **명시적으로 codesign** 한다. 그 결과물로 드래프트 릴리즈를 만든다.

⭐ **이슈 #123 — 미서명 배포는 없앴다.** 이전에는 서명 시크릿이 없으면 미서명 DMG 를 그대로 배포했다(경고만, 실패하지 않음). 미서명 번들은 TCC 가 저장하는 code requirement 와 실행 중 프로세스의 실제 identity 가 구조적으로 영원히 어긋나 **Accessibility 등 권한 인가가 영구히 실패**한다(실측 근거: 이슈 #123 본문 — cdhash 불일치 → `SecStaticCodeCheckValidity` `-67050`). 지금은 시크릿 유무와 무관하게 **항상 서명**한다:

- 서명 시크릿(`APPLE_CERTIFICATE` 그룹)이 있으면 그 인증서로 서명.
- 없으면 **번들 전체 ad-hoc 서명**(`codesign --sign -`)으로 폴백 — Sparkle.framework 포함, 안쪽부터 중첩 서명 → 메인 앱 서명(entitlements 적용, identifier `app.ultrakey.Ultrakey` 고정) → 서명된 `.app` 으로 DMG 재생성. 서명 방식과 무관하게 최종 `.app` 에 `codesign --verify --deep --strict` 가 **배포 게이트**로 걸려 있다(실패 시 워크플로 실패). `spctl` 은 자체 서명·ad-hoc 에서 rejected 가 정상 동작이므로 게이트가 아니라 정보성 출력만 남긴다.

ad-hoc 은 §1 표에서 보듯 **cdhash 기반**이라 재빌드(=자동 업데이트 포함 다음 릴리즈)마다 identity 가 바뀐다 — 인가는 매번 되지만 **사용자가 업데이트할 때마다 손쉬운 사용 등 권한을 다시 승인**해야 한다. 자체 서명 인증서(`.p12`, §8.1)를 CI 시크릿으로 등록하면 **certificate leaf 기반**(§1 표)이 되어 이 문제가 사라진다 — ad-hoc 은 임시 완화, 자체 서명 등록이 항구적 해결이다.

서명·공증 시크릿은 워크플로가 환경/시크릿 컨텍스트에서 직접 읽어 사용한다. 설정해야 하는 시크릿은 다섯 개다.

| 시크릿 | 용도 | 준비 절차 |
| :--- | :--- | :--- |
| `APPLE_CERTIFICATE` | **Developer ID Application** `.p12`(인증서 + 개인 키) 의 base64 | 키체인 접근 → 내 인증서 → Developer ID Application 인증서(+개인 키) 우클릭 → 내보내기(`.p12`, 비밀번호 지정) → `openssl base64 -in cert.p12 -out cert.b64`(Tauri 공식 문서 절차) → 파일 내용 전체를 시크릿으로 |
| `APPLE_CERTIFICATE_PASSWORD` | 위 단계에서 지정한 `.p12` 내보내기 비밀번호 | 위 절차의 그 비밀번호 그대로 |
| `APPLE_ID` | 공증에 쓰는 Apple 계정 이메일 | Apple ID 이메일 그대로 |
| `APPLE_PASSWORD` | ⚠️ **앱 전용 암호(app-specific password)** — 계정 암호나 2FA 코드가 아니다 | https://support.apple.com/HT204397 에서 생성 |
| `APPLE_TEAM_ID` | 개발자 멤버십 페이지의 Team ID | https://developer.apple.com/account#MembershipDetailsCard |

⭐ **그룹 규칙: 두 시크릿 그룹은 각각 전부 아니면 전무(all-or-nothing)다.** 서명 그룹은 `APPLE_CERTIFICATE` + `APPLE_CERTIFICATE_PASSWORD` 이고, 공증 그룹은 `APPLE_ID` + `APPLE_PASSWORD` + `APPLE_TEAM_ID` 다. 파이프라인은 불완전한 그룹을 감지해 **경고와 함께 환경에서 제거**한다(실패하지 않는다). 결과 상태는 세 가지다(이슈 #123 로 개정 — 미서명 상태는 더 이상 없다):

- **다섯 개 모두 설정** → 실제 인증서로 서명, 공증 시크릿도 모두 존재(단 아래 참고 — 이 워크플로는 실제 공증 제출을 수행하지 않는다)
- **서명 그룹만 설정** → 자체 서명 인증서로 서명(항구적 — §1) — 공증은 없으므로 다른 기기의 첫 실행에서 Gatekeeper 가 차단할 수 있다(§4)
- **하나도 설정 안 함** → **ad-hoc 폴백**(임시 완화 — §1). 업데이트마다 권한 재승인 필요

이 그룹 판정이 필요한 이유: 빈 문자열도 "존재"로 잡히면 빈 인증서 임포트 시도로 실패하거나, 공증 자격이 부분적으로만 있으면(`APPLE_ID` + `APPLE_PASSWORD` 는 있고 `APPLE_TEAM_ID` 가 없음) 하드 실패하기 때문이다.

⭐ **"다섯 개 모두 설정"이 곧 "실제로 공증됐다"를 뜻하지 않는다.** 이 워크플로는 `xcrun notarytool submit`/`xcrun stapler staple` 을 호출하지 않는다 — `NOTARIZED` job 출력은 "공증 시크릿 3종이 모두 존재한다"만 의미하는 그룹 판정 플래그다(이슈 #99 이래의 기존 동작이며, 실제 notarytool 제출 구현은 이슈 #123 범위 밖이다 — P2 검토 결론). 릴리즈 노트가 이 플래그만으로 "공증됨, 조치 불필요"를 잘못 표시하지 않도록, 워크플로는 서명 직후 `xcrun stapler validate "$APP_PATH"` 를 실제로 실행해 그 결과를 **`notarized_verified`** 라는 별도 출력에 담는다 — 릴리즈 노트의 3단계 경고 문구(공증/자체 서명/ad-hoc)는 `notarized_verified` 를 기준으로 나뉜다. 현재는 notarytool 제출이 없으므로 `notarized_verified` 는 항상 `false` 다 — 즉 릴리즈 노트는 (실제 Developer ID 공증 자동화가 별도로 추가되기 전까지) 공증 시크릿이 있어도 자체 서명 경고를 보여준다. 이는 의도된 안전한 기본값이다(거짓 "공증됨" 표시보다 과잉 경고가 낫다).

⭐ **`keychain-store` feature 게이트도 `NOTARIZED`(그룹 판정) 기준이다** — 자체 서명 `.p12` 만 등록한 상태(서명 그룹만 설정)는 `NOTARIZED=false` 이므로 keychain-store 가 켜지지 않고 개발 빌드와 같은 파일 저장을 유지한다(이슈 #99 의 키체인 프롬프트 방지 의도 보존). Developer ID + 공증 시크릿까지 모두 등록해야 keychain-store 로 전환된다.

### 8.1 자체 서명 `.p12` 등록 절차 (사용자 수동 단계 — 이 세션은 실행하지 않는다)

§2 에서 만든 로컬 `Ultrakey Dev` 인증서를 CI 시크릿으로 등록하면, 서명 시크릿이 없을 때의 ad-hoc 폴백(§1, §8 상단) 대신 **항구적 서명**(certificate leaf 기반, 재빌드해도 identity 불변)으로 릴리즈된다. 절차:

1. **키체인 접근**에서 `Ultrakey Dev` 인증서(§2.1)를 찾는다 — 개인 키가 같이 있어야 한다(인증서 옆 삼각형을 펼쳐 개인 키가 보이는지 확인).
2. 인증서와 개인 키를 함께 선택 → 우클릭 → **내보내기(Export)** → 형식 **개인 정보 교환(.p12)** → 내보내기 비밀번호를 지정한다(이 비밀번호가 `APPLE_CERTIFICATE_PASSWORD` 시크릿 값이 된다).
3. `.p12` 파일을 base64 로 인코딩한다: `openssl base64 -in ultrakey-dev.p12 -out ultrakey-dev.b64 -A`
4. 시크릿을 등록한다: `gh secret set APPLE_CERTIFICATE < ultrakey-dev.b64` 그리고 `gh secret set APPLE_CERTIFICATE_PASSWORD`(비밀번호를 프롬프트에 입력) — 또는 GitHub Settings → Secrets and variables → Actions 에서 GUI 로.
5. ⛔ 내보낸 `.p12` 파일과 base64 파일은 등록 후 **로컬에서 삭제**한다. 저장소 어디에도 커밋하지 않는다.
6. 확인: 다음 릴리즈 태그 푸시 후 워크플로 로그에서 `::notice::서명 신원: Ultrakey Dev` 가 보이는지, 릴리즈 노트가 ad-hoc 경고 대신 자체 서명 경고로 바뀌었는지 확인한다.

⚠️ 자체 서명 등록만으로는 §8 의 "공증 그룹"이 채워지지 않으므로 `keychain-store` feature 는 켜지지 않는다(위 참고) — 이는 의도된 동작이다.

**필요 없는 시크릿** (명시적으로):
- `APPLE_SIGNING_IDENTITY` — 인증서에서 자동으로 추론된다. 문제가 생겨 신원을 고정하고 싶을 때만 이 값(또는 `bundle.macOS.signingIdentity`)을 지정한다.
- `KEYCHAIN_PASSWORD` — 워크플로가 매 실행마다 `openssl rand -hex 16` 으로 임시 키체인 비밀번호를 직접 생성하고, 서명이 끝나면(`trap ... EXIT`) 그 임시 키체인을 스스로 지운다. 시크릿으로 고정할 필요가 없다.

⭐ **Team ID 불변 조건**: §6 과 같다 — 인증서를 갱신(만료 등)할 때 **같은 Team ID** 로 재발급한다. Team ID 가 바뀌면 모든 기존 사용자가 TCC 권한을 잃는다(F-13 자동 업데이트 전제 조건, `docs/spec/platform-constraints.md` §3.5).

**등록 절차**: `gh secret set APPLE_CERTIFICATE < cert.b64`(시크릿마다 반복) 또는 Settings → Secrets and variables → Actions. ⛔ **시크릿 값을 어떤 저장소 파일에도 절대 써넣지 않는다** — 이 문서는 절차만 다룬다.

**첫 릴리즈 / ad-hoc 폴백 교체 절차**: ad-hoc 폴백 드래프트 릴리즈로 파이프라인 자체를 검증하는 것은 문제없다(단, 배포용으로 그대로 공개하지 않는다 — §1·§4). 시크릿을 설정한 뒤 서명된 빌드로 교체하려면 → 드래프트 릴리즈 삭제 → 원격 태그 삭제 → **같은 태그를 다시 푸시**(워크플로 재실행).

---

## 9. Sparkle 라이브러리 검증(library validation) 예외 — 이슈 #61

### 증상

자체 서명 빌드(로컬 `build-signed.sh` · CI release 모두)로 만든 `Ultrakey.app` 이 실행 즉시 dyld 크래시로 종료됐다.

```
Termination Reason: Namespace DYLD, Code 1, Library missing
Library not loaded: @rpath/Sparkle.framework/Versions/B/Sparkle
Reason: ... code signature ... not valid for use in process:
        mapping process and mapped file (non-platform) have different Team IDs
```

### 원인

메인 앱과 번들된 `Sparkle.framework` 모두 `Authority=Ultrakey Dev`, `TeamIdentifier=not set`, `flags=0x10000(runtime)` — 즉 F-13(Sparkle) 의 중첩 재서명(`build-signed.sh` §3)은 서명 주체를 정확히 맞추고 있었다. 그런데도 크래시가 난 이유는 서명 주체 불일치가 아니라 **하드닝 런타임의 library validation** 때문이다. `--options runtime` 이 켜지면 dyld 는 로드하는 각 라이브러리에 대해 (a) Apple 플랫폼 바이너리이거나 (b) 메인 실행 파일과 **동일한 비어 있지 않은(non-empty) Team ID** 로 서명됐을 것을 요구한다. 자체 서명 인증서(§2)는 Apple 발급이 아니므로 Team ID 가 애초에 없고, **빈 Team ID 끼리는 "같은 팀" 으로 인정되지 않는다** — 따라서 재서명 절차를 아무리 정교하게 맞춰도 이 조합은 구조적으로 통과할 수 없다.

SuperKey v1.66(§7)이 같은 구성(Hardened Runtime, entitlement 은 `allow-jit` 뿐)으로도 문제가 없는 이유는 **Developer ID**(Team ID `XSYZ3E4B7D`, 비어 있지 않음)로 서명하기 때문이다. 개발 중 자체 서명에만 해당하는 문제다.

### 결정

`apps/ultrakey-app/entitlements.plist` 에 `com.apple.security.cs.disable-library-validation` (`true`) 를 추가했다. 로컬 `build-signed.sh` 와 CI `release.yml` 모두 이 파일 하나를 참조하므로(경로: `apps/ultrakey-app/entitlements.plist`) 수정 한 곳으로 양쪽이 고쳐진다. `build-signed.sh` §4 검증에도 서명된 앱의 entitlements 에 이 키가 실제로 들어갔는지 확인하는 단계를 추가해 회귀를 막는다.

### 기각한 대안

- **`--options runtime` 제거(하드닝 런타임 끄기)** — library validation 자체가 사라져 크래시는 멎지만, 향후 Developer ID + 공증 구성(하드닝 런타임 필수, §8)과 서명 구성이 갈라진다. `com.apple.security.cs.allow-jit` entitlement 도 하드닝 런타임 플래그 하에서만 의미가 있다. 기각.
- **Sparkle 원본 서명 유지(재서명하지 않음)** — 원본은 Sparkle Project 의 Developer ID Team ID 를 갖고 있어, 빈 Team ID 인 메인 앱과 오히려 더 확실하게 불일치한다. 기각.

### Developer ID 배포로 넘어갈 때

Developer ID(비어 있지 않은 Team ID)로 서명하면 앱과 Sparkle 이 같은 실제 Team ID 를 공유하게 되어 library validation 이 원래 방식대로 통과한다 — 이 entitlement 은 **개발 중 자체 서명 전용 예외**다. §8 의 CI 서명 그룹이 활성화된 배포 빌드에서 이 키를 유지할지 뺄지는 별도로 검토한다(유지해도 동작에는 영향 없음 — library validation 을 요구가 아니라 예외로 만드는 키이므로 이미 통과하는 경로를 막지 않는다).
