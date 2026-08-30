#!/usr/bin/env bash
#
# generate-icons.sh — assets/app-icon/ultrakey.svg 하나에서 앱 아이콘 전량을 다시 만든다.
#
# 이 저장소에는 수작업으로 만든 PNG 가 없다. 아이콘을 손보고 싶으면 정본 SVG
# 또는 이 스크립트의 "디자인 상수"만 고치고 이 스크립트를 다시 돌린다.
#
#   ./scripts/generate-icons.sh
#
# ⭐ 외부 의존성이 없다 — macOS 기본 도구인 `sips`(SVG 래스터화)와
#    `iconutil`(.icns 조립), 그리고 POSIX `awk` 만 쓴다. 근거와 기각한 대안은
#    docs/dev/icons.md 에 있다.
#
# ⚠️ `sips` 는 SVG 를 **문서에 적힌 `width`/`height` 픽셀 크기로만** 벡터
#    래스터화한다. `--resampleWidth` 로 키우면 24×24 로 그린 뒤 비트맵을
#    확대해 뭉개진다(실측). 그래서 이 스크립트는 크기마다 `width`/`height`
#    를 박은 SVG 를 따로 합성한다.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

SRC="${REPO_ROOT}/assets/app-icon/ultrakey.svg"
OUT_DIR="${REPO_ROOT}/apps/ultrakey-app/icons"

# ─────────────────────────────────────────────────────────────────────────────
# 디자인 상수 — 판단 근거는 docs/dev/icons.md §2
# ─────────────────────────────────────────────────────────────────────────────

# macOS 아이콘 그리드: 1024 캔버스 안에 824 정사각 타일, 사방 100 여백.
CANVAS=1024
TILE=824
# 타일 대비 글리프 상자. 정본 선화의 실제 잉크는 이 상자의 83%(24 단위 중
# 2..22)를 차지하므로, 520 이면 잉크가 타일의 약 53% 다 — 큰 크기에서 선화가
# 타일 안에서 숨 쉬는 비율. `glyph_for_px()` 가 작은 크기에서만 이걸 키운다.
GLYPH=520

# 잉크빛 그라디언트 타일 + 거의 흰 글리프.
TILE_TOP="#2A2F3A"
TILE_BOTTOM="#12151C"
GLYPH_COLOR="#F2F4F8"
# 어두운 배경 위에서 타일 윤곽이 사라지지 않게 하는 안쪽 헤어라인.
RIM_COLOR="#FFFFFF"
RIM_OPACITY="0.12"
RIM_WIDTH=3

# 메뉴바 template 이미지. tray-icon 0.24.2 는 NSImage 크기를 **18pt 고정**으로
# 덮어쓰므로(tray-icon/src/platform_impl/macos/mod.rs `icon_height: f64 = 18.0`)
# Retina 1:1 이 되는 36px 이 정확한 크기다.
MENUBAR_PX=36
MENUBAR_GLYPH=32

fail() { echo "❌ $1" >&2; exit 1; }

for tool in sips iconutil awk; do
	command -v "${tool}" >/dev/null 2>&1 || fail "'${tool}' 를 찾을 수 없다."
done
[ -f "${SRC}" ] || fail "정본 SVG 가 없다: ${SRC}"

WORK="$(mktemp -d)"
trap 'rm -rf "${WORK}"' EXIT

# ─────────────────────────────────────────────────────────────────────────────
# 정본에서 path 의 `d` 만 뽑아 온다. 색·굵기는 정본이 아니라 여기서 정한다 —
# 도형만 정본을 따르고 나머지는 이 스크립트가 소유한다는 뜻이다.
# ─────────────────────────────────────────────────────────────────────────────
PATH_DATA="$(grep -o 'd="[^"]*"' "${SRC}" | sed 's/^d="//; s/"$//')"
PATH_COUNT="$(printf '%s\n' "${PATH_DATA}" | grep -c .)"
[ "${PATH_COUNT}" -eq 3 ] || fail "정본에서 path 3개를 기대했는데 ${PATH_COUNT}개를 찾았다: ${SRC}"

# 글리프 <path> 들을 지정한 색·굵기로 다시 찍어낸다.
emit_paths() {
	local color="$1" width="$2"
	printf '%s\n' "${PATH_DATA}" | while IFS= read -r d; do
		[ -n "${d}" ] || continue
		printf '  <path d="%s" fill="none" stroke="%s" stroke-width="%s" stroke-linecap="round" stroke-linejoin="round"/>\n' \
			"${d}" "${color}" "${width}"
	done
}

# ─────────────────────────────────────────────────────────────────────────────
# 렌더 픽셀 크기별 선 굵기(정본의 24 단위 기준).
#
# 정본은 stroke-width 2 다. 그대로 두면 32px 에서 1.2px, 16px 에서 0.6px 가 되어
# 안티에일리어싱에 녹아 없어진다. 도형은 그대로 두고 굵기만 작은 크기에서
# 올린다 — 이슈 #16 이 명시적으로 허용한 조정이다.
#
#   렌더 stroke px = 굵기 × (GLYPH/24) × (px/CANVAS)
# ─────────────────────────────────────────────────────────────────────────────
stroke_for_px() {
	case "$1" in
		16) echo "3.6" ;;   # → 1.2px
		32) echo "2.6" ;;   # → 1.7px
		*)  echo "2.0" ;;   # 64px 이상은 정본 그대로 (64 → 2.7px, 128 → 5.4px)
	esac
}

# 크기별 글리프 상자. 16·32px 에서는 타일 대비 글리프를 키운다 — 그 해상도에서
# 여백은 사치이고, 실루엣이 무엇인지 알아볼 수 있는 쪽이 낫다. Apple 도 작은
# 크기용 아트워크를 따로 그린다.
glyph_for_px() {
	case "$1" in
		16) echo "640" ;;
		32) echo "580" ;;
		*)  echo "${GLYPH}" ;;
	esac
}

# ─────────────────────────────────────────────────────────────────────────────
# macOS 앱 타일 모양 — 원호 라운드 사각형이 아니라 **초타원(squircle)** 이다.
# |x/a|^n + |y/a|^n = 1, n=5 로 폴리곤을 떠서 path 로 만든다.
# ─────────────────────────────────────────────────────────────────────────────
squircle_path() {
	awk -v a="$((TILE / 2))" -v c="$((CANVAS / 2))" -v n=5 -v steps=512 '
		function sgn(v) { return v < 0 ? -1 : 1 }
		function sp(v,   m) { m = (v < 0 ? -v : v); return m == 0 ? 0 : sgn(v) * exp((2.0 / n) * log(m)) }
		BEGIN {
			pi = 3.141592653589793
			for (i = 0; i < steps; i++) {
				t = 2 * pi * i / steps
				printf "%s%.3f %.3f", (i == 0 ? "M" : "L"), c + a * sp(cos(t)), c + a * sp(sin(t))
			}
			printf "Z"
		}'
}
SQUIRCLE="$(squircle_path)"

# ─────────────────────────────────────────────────────────────────────────────
# 앱 번들 아이콘 한 장 — 초타원 타일 + 글리프.
# ─────────────────────────────────────────────────────────────────────────────
render_app_icon() {
	local px="$1" dest="$2"
	local sw glyph scale
	sw="$(stroke_for_px "${px}")"
	glyph="$(glyph_for_px "${px}")"
	scale="$(awk -v g="${glyph}" 'BEGIN { printf "%.6f", g / 24 }')"

	{
		printf '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 %s %s" width="%s" height="%s">\n' \
			"${CANVAS}" "${CANVAS}" "${px}" "${px}"
		printf '  <defs><linearGradient id="tile" x1="0" y1="0" x2="0" y2="1">'
		printf '<stop offset="0" stop-color="%s"/><stop offset="1" stop-color="%s"/>' "${TILE_TOP}" "${TILE_BOTTOM}"
		printf '</linearGradient></defs>\n'
		printf '  <path d="%s" fill="url(#tile)"/>\n' "${SQUIRCLE}"
		printf '  <path d="%s" fill="none" stroke="%s" stroke-opacity="%s" stroke-width="%s"/>\n' \
			"${SQUIRCLE}" "${RIM_COLOR}" "${RIM_OPACITY}" "${RIM_WIDTH}"
		printf '  <g transform="translate(%s,%s) scale(%s) translate(-12,-12)">\n' \
			"$((CANVAS / 2))" "$((CANVAS / 2))" "${scale}"
		emit_paths "${GLYPH_COLOR}" "${sw}"
		printf '  </g>\n</svg>\n'
	} > "${WORK}/app-${px}.svg"

	sips -s format png "${WORK}/app-${px}.svg" --out "${dest}" >/dev/null
}

# ─────────────────────────────────────────────────────────────────────────────
# 메뉴바 template 이미지 — 타일도 색도 없다. 알파만 남기고 macOS 가 현재
# 외관(라이트/다크·강조 상태)에 맞춰 다시 칠한다. 그래서 단색 검정으로 그린다.
# ─────────────────────────────────────────────────────────────────────────────
render_menubar_template() {
	local dest="$1"
	local scale
	scale="$(awk -v g="${MENUBAR_GLYPH}" 'BEGIN { printf "%.6f", g / 24 }')"
	{
		printf '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 %s %s" width="%s" height="%s">\n' \
			"${MENUBAR_PX}" "${MENUBAR_PX}" "${MENUBAR_PX}" "${MENUBAR_PX}"
		printf '  <g transform="translate(%s,%s) scale(%s) translate(-12,-12)">\n' \
			"$((MENUBAR_PX / 2))" "$((MENUBAR_PX / 2))" "${scale}"
		emit_paths "#000000" "2"
		printf '  </g>\n</svg>\n'
	} > "${WORK}/menubar.svg"
	sips -s format png "${WORK}/menubar.svg" --out "${dest}" >/dev/null
}

# ─────────────────────────────────────────────────────────────────────────────
echo "== 1. 앱 번들 PNG =="
# tauri.conf.json 의 bundle.icon 이 참조하는 4종 + Tauri 관례상 함께 두는 icon.png.
render_app_icon 32   "${OUT_DIR}/32x32.png";       echo "  ✅ 32x32.png"
render_app_icon 64   "${OUT_DIR}/64x64.png";       echo "  ✅ 64x64.png"
render_app_icon 128  "${OUT_DIR}/128x128.png";     echo "  ✅ 128x128.png"
render_app_icon 256  "${OUT_DIR}/128x128@2x.png";  echo "  ✅ 128x128@2x.png"
render_app_icon 512  "${OUT_DIR}/icon.png";        echo "  ✅ icon.png"

echo
echo "== 2. icon.icns =="
ICONSET="${WORK}/Ultrakey.iconset"
mkdir -p "${ICONSET}"
# iconutil 이 요구하는 파일명 규칙 — 이름의 숫자는 **포인트**, @2x 는 픽셀이 2배다.
for px in 16 32 64 128 256 512 1024; do
	render_app_icon "${px}" "${WORK}/px-${px}.png"
done
cp "${WORK}/px-16.png"   "${ICONSET}/icon_16x16.png"
cp "${WORK}/px-32.png"   "${ICONSET}/icon_16x16@2x.png"
cp "${WORK}/px-32.png"   "${ICONSET}/icon_32x32.png"
cp "${WORK}/px-64.png"   "${ICONSET}/icon_32x32@2x.png"
cp "${WORK}/px-128.png"  "${ICONSET}/icon_128x128.png"
cp "${WORK}/px-256.png"  "${ICONSET}/icon_128x128@2x.png"
cp "${WORK}/px-256.png"  "${ICONSET}/icon_256x256.png"
cp "${WORK}/px-512.png"  "${ICONSET}/icon_256x256@2x.png"
cp "${WORK}/px-512.png"  "${ICONSET}/icon_512x512.png"
cp "${WORK}/px-1024.png" "${ICONSET}/icon_512x512@2x.png"
iconutil -c icns "${ICONSET}" -o "${OUT_DIR}/icon.icns"
echo "  ✅ icon.icns"

echo
echo "== 3. 메뉴바 template =="
render_menubar_template "${OUT_DIR}/menubar-template.png"
echo "  ✅ menubar-template.png (${MENUBAR_PX}×${MENUBAR_PX})"

echo
echo "== 4. 완료 =="
echo "산출물: ${OUT_DIR}"
echo "⚠️ macOS 가 아이콘을 캐시한다 — 재빌드·재서명 뒤에도 옛 아이콘이 보이면 docs/dev/icons.md §4 를 본다."
