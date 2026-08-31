#!/usr/bin/env bash
#
# fetch-sparkle.sh — Sparkle.framework 2.9.2 + 명령행 도구 다운로드 (F-13)
#
# tauri-plugin-sparkle-updater(npm: spa 업데이터) 는 빌드 시점에
# `Sparkle.framework` 를 필요로 한다. 플러그인 build.rs 의 프레임워크 탐색은
# `SPARKLE_FRAMEWORK_PATH`(이 프로젝트는 .cargo/config.toml [env] 로 고정) 또는
# OUT_DIR 조상에서 `tauri.conf.json` 을 찾는 방식인데, 이 워크스페이스 레이아웃
# (앱이 apps/ultrakey-app/ — target 의 조상이 아님)에서는 후자가 실패하므로
# 반드시 `SPARKLE_FRAMEWORK_PATH` 가 필요하다(docs/dev/auto-update-server.md).
#
# 산출물:
#   apps/ultrakey-app/Sparkle.framework   — 번들에 포함(tauri.conf.json
#                                           bundle.macOS.frameworks) + 링크 타깃
#   apps/ultrakey-app/sparkle-bin/        — generate_keys · sign_update · BinaryDelta
# 두 경로 모두 .gitignore 대상(이진 산출물 — 커밋하지 않는다).
#
# ⚠️ 이 스크립트는 프레임워크(재배포 가능 이진)와 스파크 서명 도구를 받아오는
# 유일한 지점이다. sha256 고정 — 릴리즈 태그를 바꿀 때 여기 값도 함께 바꾼다.
#
# 사용법: ./scripts/fetch-sparkle.sh  (멱등 — 이미 있으면 건너뛴다)

set -euo pipefail

VERSION="2.9.2"
EXPECTED_SHA256="1cb340cbbef04c6c0d162078610c25e2221031d794a3449d89f2f56f4df77c95"
DOWNLOAD_URL="https://github.com/sparkle-project/Sparkle/releases/download/${VERSION}/Sparkle-${VERSION}.tar.xz"

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DEST_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)/apps/ultrakey-app"

FRAMEWORK="${DEST_DIR}/Sparkle.framework"
BIN_DIR="${DEST_DIR}/sparkle-bin"

if [ -d "${FRAMEWORK}" ] && [ -d "${BIN_DIR}" ]; then
	echo "✅ Sparkle.framework ${VERSION} 와 sparkle-bin 이 이미 있다 — 건너뛴다"
	exit 0
fi

TMP="$(mktemp -d)"
trap 'rm -rf "${TMP}"' EXIT

echo "== Sparkle ${VERSION} 다운로드 =="
curl -fsSL -o "${TMP}/sparkle.tar.xz" "${DOWNLOAD_URL}"

echo "== sha256 검증 =="
echo "${EXPECTED_SHA256}  ${TMP}/sparkle.tar.xz" | shasum -a 256 -c -

echo "== 해제 + 배치 =="
tar -xf "${TMP}/sparkle.tar.xz" -C "${TMP}"
rm -rf "${FRAMEWORK}" "${BIN_DIR}"
cp -R "${TMP}/Sparkle.framework" "${FRAMEWORK}"
cp -R "${TMP}/bin" "${BIN_DIR}"
chmod +x "${BIN_DIR}"/*

echo "✅ 완료:"
echo "   ${FRAMEWORK}"
echo "   ${BIN_DIR}"
file "${FRAMEWORK}/Versions/B/Sparkle" | sed 's/^/   /'