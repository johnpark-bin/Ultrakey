# F-13 자동 업데이트 — 서버(릴리즈) 측 절차

> **이 문서는 F-13(`docs/spec/auto-update.md`)의 "서버 측 산출물"(§7.4)을 절차로 정리한 것이다.**
> 클라이언트(앱) 쪽은 `apps/ultrakey-app` 의 `Info.plist` SU 키 · `tauri-plugin-sparkle-updater` 배선으로
> 이미 구현되어 있다. 이 문서는 **실제 업데이트를 배포**할 때 필요한 EdDSA 키 관리 · DMG 서명 ·
> appcast 생성 · 호스팅을 다룬다.
>
> **범위 결정 (이슈 #56 Phase 1)**: 릴리즈 워크플로우(`release.yml`)는 이번 위임에서 **확장하지 않았다.**
> 호스팅 엔드포인트가 아직 정해지지 않았고, 실제 `v*` 릴리즈가 없어 검증할 수 없는 CI 코드를
> 집어넣지 않기 위함이다. 대신 이 절차와 로컬 도구(`scripts/fetch-sparkle.sh` 가 내려주는 `sparkle-bin/`)
> 로 발행을 준비해 둔다. 호스팅을 정하면 `release.yml` 에 appcast 생성·업로드 잡을 얹으면 된다(추후 작업).

---

## 1. 빌드 전 준비물 — Sparkle.framework

`scripts/fetch-sparkle.sh` 는 `apps/ultrakey-app/Sparkle.framework`(링크+번들 타깃)와
`apps/ultrakey-app/sparkle-bin/`(서명·키 도구)을 내려받는다(버전·sha256 고정, 멱등).

- **링크**: `tauri-plugin-sparkle-updater` build.rs 가 `SPARKLE_FRAMEWORK_PATH`(`.cargo/config.toml` 의 `[env]`)로
  이 디렉터리를 찾아 컴파일 시점에 `-framework Sparkle` 을 링크한다. 없는 채로 `cargo build` 를 하면 **panic** — 반드시 이 스크립트를 먼저 돈다.
- **번들**: `tauri.conf.json` 의 `bundle.macOS.frameworks: ["Sparkle.framework"]` 가 `tauri build` 때
  `Contents/Frameworks/` 로 복사·서명하고 rpath(`@executable_path/../Frameworks`)를 잡아 준다(tauri-build).
- `.gitignore` 대상 — 프레임워크(이진)와 `sparkle-bin`(키 생성·서명 도구)은 커밋하지 않는다.
- CI: `ci.yml`·`release.yml` 의 check/build 잡이 테스트·`tauri build` 전에 이 스크립트를 실행한다.

> Sparkle 2.9.2 는 universal(`x86_64 arm64`)이므로 `--target universal-apple-darwin` 에 추가 lipo 가 필요 없다.
> `sparkle-bin/generate_keys`·`sign_update` 도 네이티브 universal 바이너리다.

---

## 2. EdDSA 키 관리 (서명 검증의 신뢰 근원)

Sparkle 은 **Ed25519** 로 DMG를 서명하고, 앱에 임베드된 공개키로 검증한다(명세 §1·§3.3·§5-1).
클라이언트 코드가 appcast·EdDSA 를 직접 다루지 않고 **Sparkle.framework 내부**가 처리한다(명세 §6).

### 2.1 개발용 키 — 생성 (이미 완료)

```sh
apps/ultrakey-app/sparkle-bin/generate_keys
```

- 개인키는 **이 머신의 macOS 키체인**(이름 `Sparkle Update Keys`)에 저장된다 — **어떤 파일에도 쓰지 않는다.**
- 출력으로 표시되는 베이스64 공개키를 `apps/ultrakey-app/Info.plist` 의 `SUPublicEDKey` 에 넣는다.
  (이 프로젝트의 개발용 공개키: `hEBYABNXGExw6rB6QUURW8X6+HwcRwXdkO00fvkl/2A=`)

### 2.2 CI/서버용 키

`release.yml` 이 appcast 를 **자동 서명**하게 하려면 개인키가 CI 시크릿이어야 한다. 개인키를 키체인에서
내보내 시크릿(`SPARKLE_EDDSA_PRIVATE_KEY` 등)으로 둔다:

```sh
# 시크릿을 만드는 사람(관리자)이 로컬에서 키체인 개인키를 내보낸 뒤 base64 로 감싼다.
sparkle-bin/generate_keys -x > sparkle-ed25519-private-key.txt   # ⛔ 절대 커밋 금지
gh secret set SPARKLE_EDDSA_PRIVATE_KEY < sparkle-ed25519-private-key.txt
```

> ⛔ **시크릿 값을 어떤 저장소 파일에도 넣지 않는다.** 이 문서는 절차만 다룬다(`docs/dev/code-signing.md §8` 과 같은 규율).

### 2.3 ⚠️ 키 교체·유출 시

명세 §9 질문 9: 개인키가 유출되면 **즉시 새 키 페어**로 교체하고, 구버전 앱(구 공개키 임베드)은 더 이상
서명을 신뢰하지 않게 된다 — appcast 항목을 새 서명으로 다시 만들고 롤링 윈도우에서 이전 항목을 제거한다.
개인키는 **한 번 커밋된 순간 신뢰 불가**이므로, 키 교체 정책은 "신규 키로 재서명 후 재배포"가 기본이다.

---

## 3. 발행 절차

`cargo tauri build`(또는 `scripts/build-signed.sh`)로 서명된 universal `.app` 과 DMG(내부에
`Contents/Frameworks/Sparkle.framework`)까지 만들어졌다고 전제한다. 그 DMG를 appcast 후보로 올리는 절차:

### 3.1 DMG 를 Sparkle 로 서명

```sh
DMG=target/universal-apple-darwin/release/bundle/dmg/Ultrakey_0.1.0_universal.dmg
apps/ultrakey-app/sparkle-bin/sign_update "${DMG}"
# → sparkle:edSignature="..." sparkle:length="..."
```

- `sign_update` 는 로컬 키체인의 개인키(개발)로 서명하거나, 시크릿에서 내보낸 파일(`-s <path>`)로 서명한다.
- ⛔ **DMG 는 반드시 코드 서명된 `.app` 을 담아야** Sparkle 의 설치 검증이 통과한다 — 미서명/ad-hoc 서명 번들은
  업데이트가 거부된다("The update is improperly signed and could not be validated.", 명세 §5-1).
- ⚠️ **신·구 `.app` 의 서명 주체(Team ID)·번들 ID·entitlement 세트가 같아야** 업데이트 후 TCC 권한
  (Accessibility 등)이 유지된다(명세 §5-9, `docs/dev/code-signing.md §6`). 릴리즈 파이프라인은 매 번들마다
  `codesign -dv --entitlements :-` 로 `runtime` 활성·같은 Team ID·`com.apple.security.cs.allow-jit` 단일을 확인한다.

### 3.2 appcast(updates.xml) 생성·갱신

`sparkle-bin/generate_appcast` 로 디렉터리를 스캔해 `<item>` 을 만들 수 있으나, 보통은 `sign_update` 가
출력한 `edSignature`·`length` 를 직접 `<enclosure>` 에 넣어 관리한다. 골격(명세 §3.1 참고 — 전부 실측 근거):

```xml
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <item>
      <title>Ultrakey 0.2.0</title>
      <pubDate>Tue, 01 Sep 2026 00:00:00 +0900</pubDate>
      <sparkle:version>2</sparkle:version>
      <sparkle:shortVersionString>0.2.0</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>12.0</sparkle:minimumSystemVersion>
      <description><![CDATA[<ul><li>새 기능 추가</li></ul>]]></description>
      <enclosure url="https://<호스트>/ultrakey/Ultrakey_0.2.0_universal.dmg"
                 sparkle:edSignature="<sign_update 출력>"
                 length="<sign_update 출력>" type="application/octet-stream" />
    </item>
  </channel>
</rss>
```

- **롤링 윈도우**(명세 §1·조사 §0): 예전 항목을 정리해 최근 10개 내외로 유지한다.
- `<sparkle:deltas>` 는 **초기 범위 제외**(명세 §7.3) — 서버에서 BinaryDelta 를 만들지 않는 한 항목을 싣지 않는다.
  `generate_appcast` 는 전체 `.dmg` 만 다루므로 델타는 필요해지면 추가 작업(직전 5개 버전 각각 BinaryDelta)이다.
- `sparkle:version`(정수)는 `CFBundleVersion`(tauri.conf.json `version` 의 빌드 번호)와 맞춘다 — 앱이
  `sparkle:version`을 비교해 후보를 가른다(명세 §3.3). 문자열 표시용 `shortVersionString` 은 `0.2.0`.

### 3.3 호스팅

명세 §5-12: **HTTPS 로만** 서빙한다(HTTP 다운그레이드 금지). DMG 와 `updates.xml` 을 같은 https 오리진에 둔다.
- 앱의 `SUFeedURL` 은 `apps/ultrakey-app/Info.plist` 에 자리표시자로 들어 있다 — 호스팅 확정 후 실제 URL 로 교체한다.

---

## 4. 실기기 전 검증 체크리스트

`docs/dev/manual-verification.md` 의 F-13 절 기준. 전체 왕복(다운로드→서명 검증→설치→재시작)은 서버가 실제로
appcast 를 서빙해야 돌릴 수 있다 — 먼저 절차 문서를 준비해 두고, 호스팅이 정해지면 그 절차대로 검증한다.

| 항목 | 검증 방법 |
| :--- | :--- |
| DMG 서명이 Sparkle 공개키로 검증된다 | `sign_update` 로 만든 서명으로 appcast 를 조회했을 때 설치가 진행됨(검증 실패 시 거부) |
| 신·구 `.app` 서명 주체 동일 | 릴리즈 파이프라인에서 `codesign -dv` 로 Team ID·runtime·entitlement 대조 |
| 자동 확인 ON 시 48h 주기 | `SUScheduledCheckInterval` 기본값과 배경 조회 로그 |
| HTTPS 전용 | HTTP URL 로 바꾸면 Sparkle 이 조회를 거부함 |
