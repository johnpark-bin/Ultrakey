# 아이콘 — 정본·생성 절차·디자인 결정

> **한 줄 요약**: 앱 아이콘은 `assets/app-icon/ultrakey.svg` 하나에서 나온다. `./scripts/generate-icons.sh` 가 macOS 기본 도구(`sips`·`iconutil`·`awk`)만으로 앱 번들 6종과 메뉴바 template 을 다시 만든다. 수작업 PNG 는 저장소에 하나도 없다.

관련: 이슈 #16 · [`code-signing.md`](code-signing.md)(재빌드·재서명) · [`../spec/menu-bar-and-lifecycle.md`](../spec/menu-bar-and-lifecycle.md)(F-10) · [`../spec/permissions-onboarding.md`](../spec/permissions-onboarding.md)(F-11)

---

## 1. 정본과 산출물

| | 경로 |
| :--- | :--- |
| **정본 SVG** | `assets/app-icon/ultrakey.svg` — 이슈 #16 본문의 SVG 그대로. 빈 래퍼 `<g>` 만 걷어냈고 **`path` 의 `d` 는 한 글자도 바꾸지 않았다** |
| 생성 스크립트 | `scripts/generate-icons.sh` |
| 앱 번들 아이콘 | `apps/ultrakey-app/icons/{32x32,64x64,128x128,128x128@2x,icon}.png` · `icon.icns` |
| 메뉴바 template | `apps/ultrakey-app/icons/menubar-template.png` (36×36) |
| 온보딩 모달 | `apps/ultrakey-app/ui/index.html` 의 인라인 `<svg class="app-icon">` |
| README | 위 `128x128@2x.png` 를 그대로 참조한다 |

```sh
./scripts/generate-icons.sh     # 전량 재생성
cargo test -p ultrakey-app --test frontend_wiring   # 정본과 사용처 정합 검사
```

⚠️ **디렉터리 이름이 `assets/icon/` 이 아니라 `assets/app-icon/` 인 데는 이유가 있다.** macOS 사용자의 전역 `.gitignore` 에는 관례적으로 클래식 Mac OS 아이콘 리소스 파일용 `Icon` 패턴이 들어 있다. git 은 macOS 에서 기본적으로 대소문자를 구분하지 않으므로(`core.ignorecase`), 그 패턴이 **`assets/icon/` 디렉터리를 통째로 무시해** 정본 SVG 가 조용히 커밋되지 않는다(실측). 이름을 되돌리지 마라.

⭐ **아이콘을 바꾸고 싶으면 PNG 를 직접 편집하지 말고** 정본 SVG 또는 스크립트 상단의 "디자인 상수"를 고치고 스크립트를 다시 돌린다.

정합은 테스트가 지킨다 — `index_html_의_아이콘_path는_정본_svg_와_같다`(온보딩 인라인 사본이 정본과 갈라지는 것), `setup_tray_는_전용_메뉴바_template_자산을_쓴다`, `bundle_icon_목록의_파일이_전부_존재한다`.

---

## 2. 디자인 결정

정본은 **배경 없는 검은 선화**다(`fill="none"` · `stroke="#000000"` · `stroke-width="2"` · `viewBox 24×24`). 그대로는 macOS 앱 번들 아이콘이 될 수 없다. 이슈 #16 이 형태 변경을 금지하므로 **색·배경·여백·굵기만** 조정했다.

### 2.1 앱 번들 — 잉크빛 초타원 타일 + 밝은 글리프

- **캔버스 1024, 타일 824, 사방 여백 100.** Apple 의 macOS 아이콘 그리드 그대로다. 아트워크가 캔버스를 꽉 채우지 않는다.
- **모양은 원호 라운드 사각형이 아니라 초타원(squircle)** 이다. `|x/a|⁵ + |y/a|⁵ = 1` 을 512 점 폴리곤으로 떠서 path 로 만든다(`squircle_path()`). `rx="185.4"` 짜리 라운드 사각형은 큰 크기에서 네이티브 아이콘 옆에 두면 모서리가 어색하다.
- **타일은 잉크빛 세로 그라디언트**(`#2A2F3A` → `#12151C`), **글리프는 거의 흰색**(`#F2F4F8`). 어두운 배경 위에서 타일 윤곽이 사라지지 않게 안쪽에 흰색 12% 헤어라인을 둘렀다.
- **글리프 상자는 타일의 63%**(1024 기준 520). 정본 선화의 실제 잉크는 상자의 83%(24 단위 중 2..22)뿐이라 **잉크는 타일의 약 53%** 를 차지한다.

**왜 검은 선화를 흰 선화로 뒤집었나 — 기각한 대안**

- **기각 ① 흰 타일 + 검은 선화(정본 색 유지).** 검정-흰색은 대비가 가장 높아 32px 에서 유리하다. 그러나 (a) 밝은 바탕화면·Finder 위에서 타일 자체의 경계가 녹아 실루엣을 잃고, (b) 얇은 검은 선은 작은 크기에서 안티에일리어싱에 회색으로 뭉개지는 반면 어두운 바탕의 밝은 선은 시각적으로 살짝 번져(irradiation) 오히려 버틴다. 라이트/다크 양쪽 Dock 에서 안정적인 쪽을 골랐다.
- **기각 ② 컬러 그라디언트 타일(파랑/보라 등).** 사용자가 준 것은 단색 선화뿐이고 브랜드 색은 지정되지 않았다. 없는 브랜드 색을 발명하지 않았다.
- **기각 ③ 타일 없이 선화만(투명 배경).** macOS 앱 번들 아이콘 관습에서 벗어나고, Dock·Finder 배경에 따라 검은 선이 그대로 묻힌다. 이슈 #16 이 지적한 바로 그 문제다.

### 2.2 작은 크기 — 굵기와 여백을 크기별로 바꾼다

정본의 `stroke-width: 2` 를 그대로 쓰면 32px 에서 1.4px, 16px 에서 0.7px 가 되어 사라진다. **형태는 그대로 두고 굵기만** 올리고, 16·32px 에서는 글리프를 키운다(그 해상도에서 여백은 사치고, 실루엣을 알아보는 쪽이 낫다 — Apple 도 작은 크기용 아트워크를 따로 그린다).

| 렌더 픽셀 | 글리프 상자 | `stroke-width`(24 단위) | 실제 선 두께 |
| ---: | ---: | ---: | ---: |
| 16 | 640 | 3.6 | 약 1.2px |
| 32 | 580 | 2.6 | 약 1.7px |
| 64 | 520 | 2.0 (정본) | 약 2.7px |
| 128 이상 | 520 | 2.0 (정본) | 5.4px 이상 |

### 2.3 메뉴바 — template 이미지

⭐ **이 SVG 가 가장 잘 맞는 자리다.** macOS 는 template 이미지의 **알파 채널만** 남기고 현재 외관(라이트/다크·메뉴 강조 상태)에 맞는 단색으로 다시 칠한다. 색이 아니라 **알파 모양**이 보이는 것 전부다. 따라서 타일도 색도 없이 선화만, 순수 검정으로 그린다.

- **36×36 px.** `tray-icon` 0.24.2 가 `NSImage` 크기를 **18pt 로 고정**하므로(`platform_impl/macos/mod.rs` 의 `icon_height: f64 = 18.0`) Retina 에서 1:1 이 되는 픽셀 크기가 36 이다. `@2x` 파일을 따로 둘 필요가 없다.
- 캔버스 36 안에서 글리프 상자 32 → 실제 잉크 약 26.7px = 13.3pt. 18pt 자리에 SF Symbols 를 놓았을 때의 광학 크기와 맞는다. 선 두께는 약 2.7px = 1.33pt.
- **이전 상태**: `setup_tray` 가 앱 번들의 `32x32.png` 를 그대로 template 로 넘겼다. 불투명한 타일 전체가 알파 1 이라 메뉴바에서 **둥근 사각형 덩어리**로만 보였다. 이번에 전용 자산으로 교체했다.

**`Menu bar icon` 팝업(F-10 §3)은 이번 범위 밖이다.** 환경설정 창의 그 팝업은 지금 `disabled` 인 자리표시자이고(`ui/settings.html`), 원본 SuperKey 의 선택지 2종이 무엇인지는 `menu-bar-and-lifecycle.md` §9 항목 2 에서 아직 `(미확정)` 이다. 선택지 체계가 확정되기 전에 아이콘 변형을 미리 만들 근거가 없어, **선택 없는 단일 template** 으로 두었다. 팝업이 살아나면 이 스크립트에 변형을 추가하는 자리는 `render_menubar_template()` 이다.

### 2.4 온보딩 모달 — 인라인 SVG + `currentColor`

`ui/index.html` 은 `color-scheme: light dark` 로 시스템 외관을 따라간다. 아이콘도 같이 따라가야 하므로 `stroke: currentColor` 를 쓴다 — 라이트에서 검은 선화, 다크에서 흰 선화가 된다.

`<img src="icon.svg">` 를 **기각**한 이유: `<img>` 안의 SVG 는 문서의 `currentColor` 를 상속하지 못해 다크 모드에서 검은 선이 묻힌다. CSS `mask-image` 로 우회할 수 있지만, 마스크 로드가 실패하면 **아무것도 그려지지 않고 조용히 사라진다** — 이 파일이 백지 사고 이후 세운 "실패해도 화면에 보이게" 원칙과 정면으로 어긋난다. 인라인 SVG 는 추가 요청도 CSP 경계도 타지 않는다.

---

## 3. 생성 파이프라인 — 왜 macOS 기본 도구만 쓰나

| 도구 | 역할 |
| :--- | :--- |
| `sips` | SVG → PNG **벡터 래스터화** |
| `iconutil` | `.iconset` → `.icns` |
| `awk` | 초타원 path 계산 |

전부 macOS 에 기본 탑재라 **외부 의존성이 0** 이다. 새 기여자가 `brew install` 없이 바로 재생성할 수 있다.

⚠️ **`sips` 의 함정 (실측)**: `sips` 는 SVG 를 **문서에 적힌 `width`/`height` 픽셀 크기로만** 벡터 래스터화한다. `--resampleWidth 1024` 로 키우면 24×24 로 그린 뒤 **비트맵을 확대**해 완전히 뭉개진다. 그래서 스크립트는 크기마다 `width`/`height` 를 박은 SVG 를 따로 합성한다.

**기각한 대안**

- **`rsvg-convert`(librsvg) / Inkscape** — 렌더 품질은 좋지만 Homebrew 설치가 필요하다. `sips` 로 충분한 품질이 나오는 것을 실측으로 확인해서 의존성을 추가할 근거가 없어졌다.
- **ImageMagick(`magick`)** — 이 기기에는 있지만 역시 Homebrew 의존이고, SVG 는 `rsvg-convert` 델리게이트로 넘긴다(`magick -list delegate`). 델리게이트가 없으면 내장 MSVG 렌더러로 떨어져 `stroke-linejoin` 처리가 부정확하다. 즉 ImageMagick 을 넣어도 결국 librsvg 를 같이 넣어야 한다.
- **`qlmanage -t`** — Quick Look 썸네일은 크기·여백을 우리가 통제할 수 없고 산출 파일명도 규약에 묶인다.
- **`resvg` 크레이트로 작은 Rust 도구 작성** — 저장소가 이미 Rust 워크스페이스라 자연스럽지만, 아이콘 한 번 만들자고 렌더러 의존성 트리를 `Cargo.lock` 에 영구히 얹는 값이 크다.

---

## 4. macOS 아이콘 캐시

재빌드·재서명했는데도 Finder/Dock 에 옛 아이콘이 보이면 대부분 캐시 문제다. **덜 침습적인 것부터** 시도한다.

1. `.app` 을 다른 폴더로 옮겼다가 되돌린다 (또는 Finder 창을 닫았다 연다).
2. Launch Services 에 다시 등록한다:
   ```sh
   /System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister \
     -f /path/to/Ultrakey.app
   ```
3. 그래도 안 되면 Finder 만 재시작한다: `killall Finder`.

⛔ `killall Dock` · `lsregister -kill -r -domain local -domain system -domain user` 같은 **시스템 전역 조작은 신중히** 한다. 전자는 Dock 상태를, 후자는 기기 전체의 Launch Services 데이터베이스를 날린다(재구축까지 수십 초~수 분, 그동안 기본 앱 연결이 흔들린다). 실행했다면 무엇을 왜 했는지 PR 에 기록한다.

⚠️ **번들 ID 는 `app.ultrakey.Ultrakey` 고정이다.** 아이콘 작업 중에도 바꾸지 않는다 — 바뀌면 TCC 권한이 전부 날아간다([`code-signing.md`](code-signing.md) §6).
