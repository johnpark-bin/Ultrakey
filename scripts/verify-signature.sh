#!/usr/bin/env bash
#
# verify-signature.sh — 서명·권한 관련 상태를 진단만 하는 읽기 전용 스크립트
#
# 키체인·서명·번들 내용 중 무엇도 바꾸지 않는다. 오직 확인하고 ✅/❌ 로 보고한다.
# 절차·기대값의 근거는 docs/dev/code-signing.md 참조.
#
# 사용법:
#   scripts/verify-signature.sh <.app 경로> [기대 서명 주체] [기대 번들 ID]
#
#   기대 서명 주체 기본값: "Ultrakey Dev" (ULTRAKEY_SIGN_IDENTITY 로도 지정 가능)
#   기대 번들 ID 는 선택 — 주면 Identifier 와 비교하고, 안 주면 값만 출력한다.

set -uo pipefail

APP_PATH="${1:-}"
EXPECTED_IDENTITY="${2:-${ULTRAKEY_SIGN_IDENTITY:-Ultrakey Dev}}"
EXPECTED_BUNDLE_ID="${3:-}"

if [ -z "${APP_PATH}" ]; then
	echo "사용법: $0 <.app 경로> [기대 서명 주체] [기대 번들 ID]" >&2
	exit 2
fi

if [ ! -d "${APP_PATH}" ]; then
	echo "❌ .app 번들을 찾을 수 없다: ${APP_PATH}" >&2
	exit 2
fi

PASS_COUNT=0
FAIL_COUNT=0

mark() {
	# $1: 0(성공) 또는 그 외(실패), $2: 메시지
	if [ "$1" -eq 0 ]; then
		echo "  ✅ $2"
		PASS_COUNT=$((PASS_COUNT + 1))
	else
		echo "  ❌ $2"
		FAIL_COUNT=$((FAIL_COUNT + 1))
	fi
}

echo "대상: ${APP_PATH}"
echo

echo "== 1. codesign --verify --deep --strict --verbose=2 =="
VERIFY_OUTPUT="$(codesign --verify --deep --strict --verbose=2 "${APP_PATH}" 2>&1)"
VERIFY_STATUS=$?
echo "${VERIFY_OUTPUT}" | sed 's/^/  /'
mark "${VERIFY_STATUS}" "서명 무결성 검증 통과"
echo

echo "== 2. codesign -dv --entitlements :- =="
DV_OUTPUT="$(codesign -dv --entitlements :- "${APP_PATH}" 2>&1)"
echo "${DV_OUTPUT}" | sed 's/^/  /'
echo

if echo "${DV_OUTPUT}" | grep -q 'flags=0x10000(runtime)'; then
	mark 0 "Hardened Runtime 활성 (flags=0x10000(runtime))"
else
	mark 1 "Hardened Runtime 미확인 — flags=0x10000(runtime) 을 찾지 못했다"
fi

AUTHORITY_LINE="$(echo "${DV_OUTPUT}" | grep -m1 '^Authority=' || true)"
if [ -n "${AUTHORITY_LINE}" ] && echo "${AUTHORITY_LINE}" | grep -q "${EXPECTED_IDENTITY}"; then
	mark 0 "서명 주체가 기대값과 일치: ${AUTHORITY_LINE}"
else
	mark 1 "서명 주체가 기대값('${EXPECTED_IDENTITY}')과 다르거나 없음: ${AUTHORITY_LINE:-<없음>}"
fi

IDENTIFIER_LINE="$(echo "${DV_OUTPUT}" | grep -m1 '^Identifier=' || true)"
if [ -n "${EXPECTED_BUNDLE_ID}" ]; then
	if echo "${IDENTIFIER_LINE}" | grep -q "${EXPECTED_BUNDLE_ID}"; then
		mark 0 "번들 ID 가 기대값과 일치: ${IDENTIFIER_LINE}"
	else
		mark 1 "번들 ID 가 기대값('${EXPECTED_BUNDLE_ID}')과 다름: ${IDENTIFIER_LINE:-<없음>}"
	fi
else
	echo "  ℹ️  ${IDENTIFIER_LINE:-<Identifier 없음>} (기대 번들 ID 미지정 — 비교 생략)"
fi
echo

echo "== 3. lipo -archs (universal 아키텍처) =="
EXECUTABLE_NAME="$(basename "${APP_PATH}" .app)"
EXECUTABLE_PATH="${APP_PATH}/Contents/MacOS/${EXECUTABLE_NAME}"
if [ ! -x "${EXECUTABLE_PATH}" ]; then
	EXECUTABLE_PATH="$(find "${APP_PATH}/Contents/MacOS" -type f -perm +111 2>/dev/null | head -n 1)"
fi

if [ -z "${EXECUTABLE_PATH}" ] || [ ! -f "${EXECUTABLE_PATH}" ]; then
	mark 1 "실행 파일을 ${APP_PATH}/Contents/MacOS 에서 찾지 못했다"
else
	LIPO_OUTPUT="$(lipo -archs "${EXECUTABLE_PATH}" 2>&1)"
	echo "  실행 파일: ${EXECUTABLE_PATH}"
	echo "  archs: ${LIPO_OUTPUT}"
	if echo "${LIPO_OUTPUT}" | grep -q 'x86_64' && echo "${LIPO_OUTPUT}" | grep -q 'arm64'; then
		mark 0 "universal 바이너리 확인 (x86_64 + arm64)"
	else
		mark 1 "universal 바이너리가 아니다: ${LIPO_OUTPUT}"
	fi
fi
echo

echo "== 요약 =="
echo "  통과 ${PASS_COUNT} / 실패 ${FAIL_COUNT}"

if [ "${FAIL_COUNT}" -gt 0 ]; then
	echo "  실패 항목의 원인·대처는 docs/dev/code-signing.md §4 를 참고한다."
	exit 1
fi

exit 0
