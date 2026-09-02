#!/usr/bin/env bash
#
# build-signed.sh — universal 빌드 + 고정 자체 서명 인증서로 서명
#
# 개발 중 권한(Accessibility 등)이 재빌드마다 사라지지 않게 하려면, 매번 같은
# 서명 주체로 서명해야 한다(docs/dev/code-signing.md §1). 이 스크립트가 그
# `cargo tauri build → codesign → 검증` 루프를 자동화한다.
#
# ⚠️ `tauri dev` 로는 권한 기능을 테스트하지 않는다 — 이 스크립트로 만든
# `.app` 을 직접 실행해서 테스트한다. 자세한 근거는 docs/dev/code-signing.md 참조.
#
# ⚠️ `--deep` 은 Apple 이 권장하지 않는 플래그다(서명 대상 안의 각 코드 조각을
# 개별적으로 검증하지 않고 뭉뚱그려 서명한다). 지금은 Tauri 번들 안에 별도
# 프레임워크가 없어(entitlements.plist 에 App Sandbox 도 없고 임베디드
# 프레임워크도 없다) 문제가 되지 않지만, F-13(Sparkle) 이 들어와
# `Sparkle.framework` 를 번들에 넣는 순간부터는 `--deep` 을 버리고
# 프레임워크 → XPC 서비스 → 메인 실행 파일 순서로 **안쪽부터 개별 서명**해야
# 한다. 이 스크립트를 그때 반드시 갱신할 것.

set -euo pipefail

SIGN_IDENTITY="${ULTRAKEY_SIGN_IDENTITY:-Ultrakey Dev}"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
ENTITLEMENTS="${REPO_ROOT}/apps/ultrakey-app/entitlements.plist"

DOC="docs/dev/code-signing.md"

fail() {
	echo "❌ $1" >&2
	echo "   → 절차는 ${DOC} 를 참고한다." >&2
	exit 1
}

echo "== 1. 사전 확인 =="

if ! command -v cargo >/dev/null 2>&1; then
	fail "cargo 를 찾을 수 없다. Rust 툴체인을 설치한다(rustup.rs)."
fi

if ! cargo tauri --version >/dev/null 2>&1; then
	fail "cargo-tauri 가 설치되어 있지 않다. 'cargo install tauri-cli --version \"^2\"' 를 실행한다."
fi
echo "  ✅ cargo tauri: $(cargo tauri --version)"

INSTALLED_TARGETS="$(rustup target list --installed 2>/dev/null || true)"
for target in aarch64-apple-darwin x86_64-apple-darwin; do
	if ! grep -qx "${target}" <<<"${INSTALLED_TARGETS}"; then
		fail "rustup 타깃 '${target}' 이 설치되어 있지 않다. 'rustup target add ${target}' 를 실행한다."
	fi
done
echo "  ✅ rustup 타깃: aarch64-apple-darwin, x86_64-apple-darwin 모두 설치됨"

if ! security find-identity -v -p codesigning 2>/dev/null | grep -q "\"${SIGN_IDENTITY}\""; then
	fail "코드 서명 인증서 '${SIGN_IDENTITY}' 를 키체인에서 찾을 수 없다. 자체 서명 인증서를 만들고 '항상 신뢰' 로 설정한다(${DOC} §2)."
fi
echo "  ✅ 서명 인증서 '${SIGN_IDENTITY}' 확인됨"

if [ ! -f "${ENTITLEMENTS}" ]; then
	fail "entitlements 파일이 없다: ${ENTITLEMENTS}"
fi
echo "  ✅ entitlements 파일: ${ENTITLEMENTS}"

echo
echo "== 2. universal 빌드 =="
cd "${REPO_ROOT}"
# ⭐ F-13 — Sparkle 프레임워크가 없으면 먼저 내려받는다 (tauri-plugin-sparkle-updater
#   build.rs 가 컴파일 시점에 프레임워크를 요구한다). 멱등 — 이미 있으면 그대로 둔다.
./scripts/fetch-sparkle.sh
# ⭐ 이슈 #99 — 개발/자체 서명 빌드는 무-feature(기본)로 빌드한다. 라이선스 저장소가
#   파일(`~/Library/Application Support/Ultrakey/`)이 되어 키체인 로그인 프롬프트가
#   뜨지 않는다. Keychain 저장(`keychain-store` feature)은 정식 서명 릴리즈 전용 —
#   `.github/workflows/release.yml` 참조(docs/spec/licensing-and-trial.md §3.5 개정).
# ⭐ `--bundles app` — 이 스크립트가 필요한 산출물은 서명 대상 `.app` 하나뿐이다.
#   기본값(dmg 포함)으로 두면 번들러의 bundle_dmg.sh 가 Finder AppleScript 로
#   창을 꾸미다 macOS 26 에서 실패하고(`Can't set statusbar visible ... (-10006)`),
#   `set -e` 때문에 서명 단계까지 못 간다. 배포용 dmg 는 릴리스 워크플로가 만든다
#   (.github/workflows/release.yml).
cargo tauri build --target universal-apple-darwin --bundles app

echo
echo "== 3. 서명 =="

# universal 타깃의 번들 산출 위치는 워크스페이스 공유 target 디렉터리 기준으로
# target/universal-apple-darwin/release/bundle/macos/*.app 이다(Tauri v2 관례).
# 아직 apps/ultrakey-app/tauri.conf.json 이 없는 시점에는 정확한 앱 이름을
# 미리 알 수 없으므로 glob 으로 찾는다.
BUNDLE_DIR="${REPO_ROOT}/target/universal-apple-darwin/release/bundle/macos"
if [ ! -d "${BUNDLE_DIR}" ]; then
	fail "번들 디렉터리를 찾을 수 없다: ${BUNDLE_DIR} (빌드 산출 경로가 바뀌었다면 이 스크립트를 갱신한다)"
fi

APP_PATH=""
for candidate in "${BUNDLE_DIR}"/*.app; do
	if [ -d "${candidate}" ]; then
		APP_PATH="${candidate}"
		break
	fi
done

if [ -z "${APP_PATH}" ]; then
	fail "${BUNDLE_DIR} 에서 .app 번들을 찾지 못했다."
fi
echo "  대상: ${APP_PATH}"

codesign --force --options runtime \
	--entitlements "${ENTITLEMENTS}" \
	--sign "${SIGN_IDENTITY}" \
	"${APP_PATH}"

# ⭐ F-13 — Sparkle 프레임워크가 번들에 들어갔다면 `--deep` 대신 **안쪽부터 개별
# 서명**한다(docs/dev/code-signing.md §7). Tauri 번들러가 프레임워크를
# Contents/Frameworks 에 복사해 두고, 이 스크립트는 그 위에 다시 Ultrakey Dev
# 주체로 서명해 번들 전체를 같은 주체로 맞춘다. 순서: XPC/헬퍼 → 프레임워크 →
# 메인 앱(마지막 메인 재서명으로 프레임워크를 중첩 코드로 포함).
if [ -d "${APP_PATH}/Contents/Frameworks/Sparkle.framework" ]; then
	SINGED_SPARKLE=1
	echo "  -- Sparkle.framework 중첩 코드(XPC·헬퍼 앱) 개별 서명 --"
	# find 의 -o 는 -print0 보다 우선순위가 낮아 두 조건을 괄호로 묶어야 한다.
	find "${APP_PATH}/Contents/Frameworks/Sparkle.framework" \
		\( -name "*.xpc" -o -name "*.app" \) -print0 |
		while IFS= read -r -d '' nested; do
			echo "    ${nested##*/}"
			codesign --force --options runtime --sign "${SIGN_IDENTITY}" "${nested}"
		done
	echo "  -- Sparkle.framework 번들 자체 서명 --"
	codesign --force --options runtime --sign "${SIGN_IDENTITY}" \
		"${APP_PATH}/Contents/Frameworks/Sparkle.framework"
fi

# 프레임워크를 포함해 메인 앱을 다시 서명한다 — 메인 서명이 중첩 참조(cdhash)를
# 기록해야 하므로 프레임워크를 먼저 서명한 뒤 이 단계를 거친다. `--deep` 없이
# 메인 번들만 서명하면 이미 서명된 프레임워크는 그대로 보존된다.
if [ "${SINGED_SPARKLE:-0}" = "1" ]; then
	codesign --force --options runtime \
		--entitlements "${ENTITLEMENTS}" \
		--sign "${SIGN_IDENTITY}" \
		"${APP_PATH}"
fi

echo
echo "== 4. 검증 =="

echo "  -- codesign --verify --deep --strict --verbose=2 --"
codesign --verify --deep --strict --verbose=2 "${APP_PATH}"

echo
echo "  -- codesign -dv --entitlements :- --"
codesign -dv --entitlements :- "${APP_PATH}"

echo
echo "  -- entitlements: disable-library-validation --"
# F-13(Sparkle) 은 Team ID 없는 자체 서명이라 하드닝 런타임의 library validation 을
# 구조적으로 통과할 수 없다(docs/dev/code-signing.md §9, 이슈 #61). 이 키가 빠지면
# 서명된 결과물은 만들어져도 실행 즉시 dyld 크래시가 나므로 여기서 반드시 걸러낸다.
if codesign -d --entitlements :- --xml "${APP_PATH}" 2>/dev/null |
	grep -q '<key>com.apple.security.cs.disable-library-validation</key>'; then
	echo "  ✅ com.apple.security.cs.disable-library-validation 존재"
else
	fail "서명된 앱의 entitlements 에 com.apple.security.cs.disable-library-validation 이 없다. ${ENTITLEMENTS} 를 확인한다(이슈 #61)."
fi

EXECUTABLE_NAME="$(basename "${APP_PATH}" .app)"
EXECUTABLE_PATH="${APP_PATH}/Contents/MacOS/${EXECUTABLE_NAME}"
if [ ! -x "${EXECUTABLE_PATH}" ]; then
	# Info.plist 의 CFBundleExecutable 이 앱 이름과 다를 수 있으므로 폴백으로 찾는다.
	EXECUTABLE_PATH="$(find "${APP_PATH}/Contents/MacOS" -type f -perm +111 | head -n 1)"
fi

echo
echo "  -- lipo -archs ${EXECUTABLE_PATH} --"
lipo -archs "${EXECUTABLE_PATH}"

echo
echo "== 5. 완료 =="
echo "산출물: ${APP_PATH}"
