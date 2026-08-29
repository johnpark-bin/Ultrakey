# F-13 · 자동 업데이트

> 한 줄 요약: Sparkle 2.x 호환 appcast(`updates.xml`)를 주기적/수동으로 조회해 신버전을 찾고, EdDSA(ed25519) 서명을 검증한 아티팩트만 `.dmg` 교체 방식으로 설치한 뒤 앱을 재시작한다. 검증 실패 시 절대 설치하지 않는 것이 이 기능의 보안 핵심이다.
> 의존성: `F-11`(`permissions-onboarding.md`, 권한·코드 서명 일반론 — 여기서는 "업데이트가 TCC 권한을 깨지 않을 조건"만 다룬다) · `F-10`(`app-lifecycle-and-menubar.md`, 종료 전 리매핑 엔진 정리 계약 — 재시작이 그 계약을 트리거하는 지점만 다룬다)
> 관련 명세: `F-12`(`license-validation.md`, 라이선스 검증 — 네트워크를 쓰는 나머지 한 가지, 이 문서의 appcast 조회와 함께 "네트워크는 라이선스 검증 + 업데이트뿐"이라는 제품 약속(조사 §1.3)을 구성한다) · `F-10`(메뉴바 메뉴 항목 배치, 앱 종료·재시작 수명주기 자체) · `F-11`(Accessibility/Screen Recording/Input Monitoring 권한 획득·복구 UI)

---

## 1. 개요

Superkey 는 macOS 데스크톱 앱의 표준 자동 업데이트 프레임워크인 **Sparkle**(현재 2.x 세대)을 그대로 쓴다. 이는 이번 조사 전체에서 **원본 인프라를 XML 원문으로 직접 확인한 유일한 영역**이다 — 랜딩 페이지의 마케팅 문구가 아니라, `https://superkey.app/downloads/updates.xml` 을 실제로 fetch 해서 얻은 사실이다(조사 §0, §2.3). 따라서 이 명세는 다른 F-문서들보다 "추정"의 비중이 낮고, appcast 스키마·서명 방식·델타 전략 대부분을 확정으로 서술할 수 있다.

핵심 사실 3가지:

1. **appcast 는 Sparkle 2.x 포맷**이다. 네임스페이스 `http://www.andymatuschak.org/xml-namespaces/sparkle` 로 확인된다(조사 §2.3).
2. **서명은 EdDSA(ed25519)**다. `sparkle:edSignature` 속성이 XML 에 직접 등장하며, 구형 DSA 방식이 아니다.
3. **델타 업데이트를 제공**한다 — 직전 5개 버전까지, 전체 `.dmg`(4,969,453 bytes) 대비 델타(182,658 bytes)는 약 1/27 크기다.

네트워크 경계: 랜딩 FAQ 는 "only reaches out to the network for license validation or updates **as configured**"(조사 §1.3)라고 명시한다. "as configured" 는 사용자가 자동 확인을 끌 수 있음을 시사했고, 이는 이제 실측으로 확정되었다 — `SUEnableAutomaticChecks` 가 defaults 에 `false` 로 저장되어 있고(실측: defaults), `General` 탭의 `Check for updates automatically` 체크박스가 기본 ☐ 다(실측: AX 트리, §4). 이 기능은 `F-12`(라이선스 검증)와 함께 Superkey 의 네트워크 접근 전부를 구성하며, 그 외에는 어떤 원격 호출도 없어야 한다(조사 §1.3 "None of the data that Superkey processes is stored on your disk... none of my apps use any kind of telemetry or tracking"). `F-12` 가 실측으로 확인한 **자체 코드 서명 검증**(`SecStaticCodeCreateWithPath` 등, `docs/spec/licensing-and-trial.md` §3.9)은 이 문서의 §5-9(TCC 권한 유지 조건)에 직접 제약을 더한다 — 상호 참조는 §5-9 에서 다룬다.

appcast 는 **롤링 윈도우**다 — 현재 파일에는 `<item>` 10개만 있고, 웹페이지(`/versions`)는 그중 최근 3개만 렌더링한다(`parseAppcast.js` 의 `items.slice(0, 3)`, 조사 §0). 이 사실은 클라이언트 구현에도 영향을 준다: appcast 를 "전체 히스토리"로 취급하면 안 되고, "현재 설치 버전보다 새로운 항목 중 appcast 에 실려 있는 것"만 후보로 삼아야 한다.

## 2. 사용자 시나리오

1. **백그라운드 자동 확인** — 앱이 실행 중인 상태로 **48시간**(`SUScheduledCheckInterval = 172800`, 실측: Info.plist)이 지나면, 사용자 개입 없이 appcast 를 조회한다. 단, ⭐ **출고 기본값은 이 자동 확인 자체가 꺼져 있다**(`SUEnableAutomaticChecks = false`, 실측: defaults·AX 트리, §4) — 사용자가 `General` 탭에서 직접 켜야 이 시나리오가 발동한다. 신버전이 없으면 아무 UI 도 나타나지 않는다.
2. **신버전 발견 → 알림** — 신버전이 발견되면 릴리스 노트(appcast 의 `<description>`)와 함께 업데이트 안내가 표시되고, 사용자는 설치 여부를 선택한다. Sparkle 표준 UI 문자열로 `A new version of %@ is available!` / `A new version of %@ is ready to install!` 가 확인된다(Sparkle 프레임워크 원문, §3.4). ⭐ 이때 사용자에게는 단순 예/아니오가 아니라 **`Skip This Version`(이 버전 건너뛰기) · `Remind Me Later`(나중에 알림)** 두 선택지가 실재한다(Sparkle 프레임워크 원문, 실측: 번들 문자열, §3.4) — 즉시 설치 외의 경로가 최소 2가지 더 있다.
3. **수동 확인** — ⭐ **메뉴바 메뉴의 `Check for Updates…` 항목**(실측: AX 트리, `app-bundle-analysis.md` §6.5)을 클릭해 즉시 appcast 를 재조회한다. `General` 탭에는 이 기능에 대응하는 컨트롤이 없다(§4) — 수동 확인의 소재는 메뉴바뿐이다. 최신 버전이면 "최신 버전입니다" 류의 확인 메시지가 뜬다.
4. **앱 시작 시 확인** — 앱을 실행할 때 자동 확인이 켜져 있으면 즉시(또는 짧은 지연 후) 한 번 조회한다.
5. **자동 확인 끄기** — 사용자가 `General` 탭의 `Check for updates automatically` 체크박스를 끈다(출고 기본값이 이미 ☐ 이므로, 대부분의 사용자에게는 "끄는" 행위가 아니라 "그대로 두는" 행위다). 이후 백그라운드 조회는 멈추지만, 수동 확인(메뉴바) 경로는 계속 동작한다.
6. **델타로 빠르게 업데이트** — 사용자가 직전 5개 버전 이내를 쓰고 있다면, 183KB 안팎의 델타만 받아 빠르게 업데이트를 마친다. 오래된 버전이면 알아채지 못한 채 자동으로 전체 `.dmg` 를 받는다.
7. **서명 검증 실패를 목격** — (정상 시나리오는 아니지만) 손상되었거나 위조된 아티팩트가 다운로드된 경우, 사용자는 설치가 진행되지 않고 오류 메시지만 보게 된다. 이 기능의 보안 핵심이 여기서 드러난다.
8. **재시작을 통한 설치 완료** — 사용자가 설치를 승인하면 `.dmg` 마운트 → `.app` 교체 → 앱 재시작이 이어진다. 리매핑 엔진이 event tap 을 쥐고 있던 상태였다면, 재시작 전에 정리(modifier 해제)가 선행되어야 한다(`F-10` 계약).
9. **오프라인 상태의 확인 시도** — 네트워크가 없을 때 백그라운드 확인은 조용히 실패하고 다음 주기를 기다리지만, 수동 확인은 사용자에게 실패를 알린다.

## 3. 동작 명세

### 3.1 appcast 스키마

아래 표는 `https://superkey.app/downloads/updates.xml` 원문에서 직접 확인된 요소·속성만 담는다. 확인되지 않은 필드는 추가하지 않는다.

| 요소 / 속성 | 의미 | 필수 여부 | 원본 실제값 예시 | 출처 |
| :--- | :--- | :--- | :--- | :--- |
| `<rss xmlns:sparkle="...">` | Sparkle 2.x 네임스페이스 선언 — appcast 포맷 식별자 | 필수 | `http://www.andymatuschak.org/xml-namespaces/sparkle` | 조사 §2.3 |
| `<item>` | 릴리스 1건 | 필수(항목당 1개), 현행 파일에 10개 | — | 조사 §0 |
| `<pubDate>` | 릴리스 일시 | 필수(정렬 기준으로 추정) | `Tue, 23 Jun 2026`(요일·일·월·연도만 확인됨. 시각·타임존 포함 여부·RFC822 전체 형식은 미확인 `(추정)`) | 조사 §2.1 |
| `<description>` (CDATA) | 릴리스 노트 HTML | 필수 | `<ul><li>Fixes a crash introduced in v1.65.</li></ul>` | 조사 §2.1 |
| `<sparkle:version>` | `CFBundleVersion` — 단조 증가 정수 | 필수 | `66` | 조사 §2.3 |
| `<sparkle:shortVersionString>` | `CFBundleShortVersionString` | 필수 | `1.66` | 조사 §2.3 |
| `<sparkle:minimumSystemVersion>` | 이 버전을 설치 후보로 삼기 위한 최소 macOS 버전 게이트 | 필수(게이트 목적상) | `12.0` | 조사 §1.5, §2.3 |
| `<enclosure url="...">` | 배포 아티팩트 다운로드 URL | 필수 | `.../Superkey1.66.dmg`(전체 URL 은 truncated 인용, 도메인/경로 패턴만 확정) | 조사 §1.5 |
| `<enclosure length="...">` | 아티팩트 바이트 크기 | 필수 | `4969453` | 조사 §1.5 |
| `<enclosure type="...">` | MIME 타입 — 배포 형식이 `.dmg` 임을 뒷받침 | 필수 | `application/octet-stream` | 범위 확정 표 |
| `sparkle:edSignature` (`<enclosure>` 속성) | EdDSA(ed25519) 서명값 | 필수 | `BGoKibR3nvMA...`(절단 인용) | 조사 §2.3 |
| `<sparkle:deltas>` | 이 버전으로 가는 델타 목록 컨테이너 | 선택 — 델타 커버리지 밖의 구버전에서 온 항목은 자식이 없거나 존재하지 않을 수 있음 `(추정)` | 자식 `<enclosure>` 5~6개 | 조사 §2.3 |
| 델타 파일명 패턴 | `Superkey<신버전>-<구버전>.delta` | — | `Superkey66-65.delta` … `Superkey66-60.delta` | 조사 §2.3 |
| `sparkle:deltaFromSparkleLocales` | 번들 로케일 목록(정확한 소속 요소·의미는 파일명에서 추정) | 확인됨(값), 위치·정확한 의미 `(추정)` | `de,he,ar,el,ja,fa,uk`(+en 기본) | 조사 §2.3 |

델타 항목의 크기 비교는 실측값이다: 전체 `.dmg` 4,969,453 bytes 대 델타 182,658 bytes — 약 96% 절감.

참고용 골격(확인된 필드만으로 구성, 델타 자식의 정확한 속성명 일부는 Sparkle 표준 관례를 따른 것으로 `(추정)` 표기):

```xml
<rss version="2.0" xmlns:sparkle="http://www.andymatuschak.org/xml-namespaces/sparkle">
  <channel>
    <item>
      <pubDate>Tue, 23 Jun 2026</pubDate>
      <sparkle:version>66</sparkle:version>
      <sparkle:shortVersionString>1.66</sparkle:shortVersionString>
      <sparkle:minimumSystemVersion>12.0</sparkle:minimumSystemVersion>
      <description><![CDATA[
        <ul><li>Fixes a crash introduced in v1.65.</li></ul>
      ]]></description>
      <enclosure
        url=".../Superkey1.66.dmg"
        length="4969453"
        type="application/octet-stream"
        sparkle:edSignature="BGoKibR3nvMA..." />
      <!-- 아래 sparkle:deltaFrom 속성명·구조는 Sparkle 표준 관례를 따른 것으로 (추정) -->
      <sparkle:deltas>
        <enclosure url=".../Superkey66-65.delta" sparkle:deltaFrom="65"
          type="application/octet-stream" sparkle:edSignature="..." />
        <!-- ... Superkey66-60.delta 까지 -->
      </sparkle:deltas>
    </item>
  </channel>
</rss>
```

### 3.2 업데이트 상태 머신

| 상태 | 진입 조건 | 수행 동작 | 성공 시 다음 상태 | 실패 시 분기 |
| :--- | :--- | :--- | :--- | :--- |
| 유휴(Idle) | 앱 시작, 이전 사이클 종료, 사용자가 "자동 확인 끔" | 다음 예정 확인 대기(자동) 또는 사용자 액션 대기(수동) | 확인 중 | — |
| 확인 중(Checking) | 주기 도달 / 앱 시작 / 사용자가 "지금 확인" 클릭 | HTTPS 로 `updates.xml` GET | 사용 가능 또는 유휴("최신 버전") | **appcast 파싱 실패**(네트워크 오류·XML 오류·서버 다운) → 유휴 복귀. 자동 확인이면 조용히, 수동 확인이면 오류 노출(§5-7) |
| 사용 가능(Available) | 파싱 성공 + 후보 선정 규칙(§3.3) 통과 항목 존재 | 릴리스 노트·버전 표시, 사용자 승인 대기(자동 확인 경로) 또는 즉시 다음 단계(정책 미확정 `(추정)`) | 다운로드 | **모든 항목이 `minimumSystemVersion` 미달**(§5-8) → 유휴("업데이트 없음") |
| 다운로드(Downloading) | 사용자 승인 (또는 자동 다운로드 정책이 켜진 경우 즉시) | 델타 판정(§3.3) 결과에 따라 델타 또는 전체 `.dmg` GET | 검증 | **다운로드 중단**(§5-2) → 재시도 또는 유휴. **디스크 공간 부족**(§5-3) → 유휴, 원본 앱 유지 |
| 검증(Verifying) | 다운로드 완료 | 임베드된 EdDSA 공개키로 `sparkle:edSignature` 검증 | 설치 | **서명 검증 실패**(§5-1) → **다운로드 파일 폐기, 설치로 절대 진행하지 않음**, 유휴 + 오류 노출 |
| 설치(Installing) | 검증 통과 | `.dmg` 마운트 → 신규 `.app` 을 `/Applications` 의 기존 `.app` 과 교체 | 재시작 대기 | **델타 적용 실패**(§5-6) → 전체 `.dmg` 재다운로드로 폴백(다운로드 상태로 복귀). **쓰기 불가 위치/권한 부족**(§5-4, §5-5) → 유휴, 원본 앱 유지, 오류 노출 |
| 재시작 대기(PendingRestart) | 교체 완료 | `F-10` 종료 전 정리 계약 실행(리매핑 엔진 event tap 해제, 눌려 있는 modifier 강제 해제) 후 프로세스 종료·신규 `.app` 실행 | (프로세스 재기동으로 상태 초기화) | 정리 계약이 완료되지 않은 채 종료되면 **modifier stuck**(§5-10) 위험 |

### 3.3 후보 선정과 델타 적용 판정

1. **`minimumSystemVersion` 게이트** — appcast 의 각 `<item>` 을 최신 버전부터 순회하며, 현재 macOS 버전이 그 항목의 `sparkle:minimumSystemVersion` 이상인 첫 항목을 후보로 채택한다. 모든 항목이 게이트에 걸리면 "업데이트 없음"으로 처리한다(§5-8).
2. **버전 비교** — 채택된 항목의 `sparkle:version`(정수) 이 현재 설치본의 `CFBundleVersion` 보다 큰 경우에만 "사용 가능" 상태로 전이한다. 비교는 `sparkle:shortVersionString`(`1.66` 형태) 이 아니라 정수 `sparkle:version` 기준으로 한다 — 문자열 버전은 표시용이다.
3. **델타 적용 가능 여부** — 선정된 신버전 항목의 `<sparkle:deltas>` 자식 중, 현재 설치본의 `sparkle:version` 과 일치하는 "구버전" 을 소스로 하는 델타가 있는지 확인한다. 있으면 델타를 우선 시도한다. appcast 는 직전 5개 버전까지만 델타를 유지하므로, 현재 버전이 그보다 오래되었으면 델타 후보가 애초에 존재하지 않고 전체 `.dmg` 로 진행한다.
4. **델타 검증·적용 실패 시 전체 폴백** — 델타 다운로드 후 서명 검증 또는 바이너리 패치 적용에 실패하면, 전체 `.dmg` 다운로드로 자동 폴백한다(§3.2 "설치" 상태의 실패 분기). 이 폴백은 사용자에게 별도 승인을 요구하지 않는다 `(추정)` — 최초 승인이 이미 "이 버전으로 업데이트"를 의미했다고 보기 때문이다.
5. **서명은 델타·전체 아티팩트 모두에 개별 적용** — 델타 자체도 `sparkle:edSignature` 를 가지므로(§3.1), 델타를 적용한 결과물이 아니라 **델타 파일 자체의 다운로드 시점**에 서명을 검증한다. 적용 후 최종 산출물의 무결성은 Sparkle 표준 흐름상 델타 적용 알고리즘의 체크섬으로 보장되는 것으로 본다 `(추정)`.

### 3.4 ⭐ Sparkle 표준 UI 상태·문자열 (Sparkle 프레임워크 원문 — SuperKey 자체 문구가 아니다, 실측: 번들 문자열)

번들 문자열에서 Sparkle 2.9.2 가 내장한 업데이트 흐름의 UI 상태 문자열 전량이 확인된다:

```
Checking for updates…
Downloading update…
Extracting update…
Installing update…
A new version of %@ is available!
A new version of %@ is ready to install!
Skip This Version
Remind Me Later
Ready to Install
An error occurred in retrieving update information. Please try again later.
An error occurred while downloading the update. Please try again later.
An error occurred while extracting the archive. Please try again later.
The update is improperly signed and could not be validated.
```

이 문자열 집합이 §3.2 상태 머신의 각 단계(확인 중·다운로드·검증·설치)에 정확히 대응한다 — "Checking for updates…" = 확인 중, "Downloading update…" = 다운로드, "Extracting update…"/"Installing update…" = 설치, "The update is improperly signed and could not be validated." = §3.2 "검증" 실패 분기(§5-1)의 실제 문구다. ⭐ **`Skip This Version` / `Remind Me Later` 라는 사용자 선택지가 실재**한다는 점이 중요하다 — §3.2 "사용 가능(Available)" 상태의 "사용자 승인 대기"는 단순 설치/취소 이분법이 아니라 이 두 경로를 포함한 최소 3지 선택으로 정정한다(§2 시나리오 2).

SuperKey 자체 키(값이 아니라 defaults/코드 상의 식별자): `updater` · `appVersionTooNew` · `DefaultUpdate` · `ProductVersion` · `versionField` · `copyrightField` · `versionButton`.

⭐ `appVersionTooNew` 는 **설치된 버전이 appcast 보다 최신인 경우**의 처리가 있다는 뜻이다 `(미확정 — 이름으로부터의 해석)`. §5-15 에서 실패 모드로 다룬다.

### 3.5 ⭐ `lastVersion` 키 — 마지막 실행 버전 기록 (실측: defaults)

조사 시점의 defaults 전량(§2.1, `app-bundle-analysis.md`)에 `lastVersion = "66"` 이 있다 — **앱이 마지막으로 실행된 버전을 기록**한다. 업데이트 직후 최초 실행에서 `CFBundleVersion`(신버전)과 `lastVersion`(이전 실행 버전)을 비교해 다르면, 1회성 마이그레이션이나 "무엇이 바뀌었는지" 안내를 트리거하는 용도로 보인다 `(미확정 — 용도는 해석)`. 비교 후에는 `lastVersion` 을 현재 버전으로 갱신하는 것이 자연스러운 설계다 `(추정)`.

## 4. 설정 항목

### 4.1 ⭐ Info.plist 로 확정된 Sparkle 설정 — §4 를 실측으로 교체

`General` 탭 스크린샷과 `Info.plist`(실측) 로 아래가 전부 확정되었다. 이전 버전이 조사 Q13 으로 남겨둔 "확인 주기"와 "자동 확인 기본값" 두 추정은 **오기였다** — 아래 두 건이 정정이다.

| 키 | 실측값 | 의미 |
| :--- | :--- | :--- |
| `SUFeedURL` | `https://superkey.app/downloads/updates.xml` | appcast URL(§3.1 과 일치) |
| `SUPublicEDKey` | `lpt9M3PhocbZ3MZiLH+crEqRfU11kfoNzGxSqiEIdvM=` | EdDSA(ed25519) 공개키 — 서명 검증 방식이 EdDSA 라는 §1·§3.3 의 서술이 이 키의 실존으로 확정된다(`(추정)` → 승격) |
| `SUScheduledCheckInterval` | **`172800`**(초) = **48시간** | ⭐ **오기 수정 1**: 이전 §4 항목 2 "확인 주기 24시간 `(추정, Sparkle 기본 SUScheduledCheckInterval)`" 는 틀렸다. Sparkle 기본값을 쓰는 게 아니라 **48시간이 명시적으로 설정**되어 있다 |

번들 프레임워크는 **Sparkle 2.9.2**(`Contents/Frameworks/Sparkle.framework`)이며, 내부에 `Autoupdate` · `Updater.app` · `XPCServices`(`Downloader.xpc` · `Installer.xpc`)를 포함한다. Sparkle 이 WebKit 을 링크한다(릴리스 노트 HTML 렌더링용으로 추정 `(추정)`).

사용자 설정 실측: `SUEnableAutomaticChecks` = **`false`**(실측: defaults). `General` 탭의 `Check for updates automatically` 체크박스와 일치하며 **기본값이 ☐** 다. ⭐ **오기 수정 2**: 이전 §4 항목 1 "자동으로 업데이트 확인 기본 ON `(추정)`" 는 틀렸다. **기본은 OFF 다** — 근거: 아무 설정도 바꾸지 않은 상태에서 `SUEnableAutomaticChecks = false` 이고 체크박스가 ☐ 다(실측: defaults·AX 트리).

### 4.2 §4 의 나머지 추정 항목 정리

이전 §4 가 나열했던 6개 항목 중, `General` 탭에 **실재가 확인된 것은 "지금 확인" 하나뿐**이며 그마저 `General` 탭이 아니라 **메뉴바**에 있다.

| # | 항목 | 실측 결과 |
| :--- | :--- | :--- |
| 1 | 자동으로 업데이트 확인 | `General` 탭에 실재 — `Check for updates automatically` 체크박스, 기본 ☐(§4.1) |
| 2 | 확인 주기 | `General` 탭에 노출되는 컨트롤 **없음**(실측: AX 트리) — 값은 `SUScheduledCheckInterval`(48시간)로 고정, 사용자가 바꿀 수 없는 것으로 보인다 `(미확정)` |
| 3 | 자동으로 다운로드 | `General` 탭에 **없음이 확인됨**(실측: AX 트리, `app-bundle-analysis.md` §6.4) | 
| 4 | 프리릴리스/베타 채널 구독 | `General` 탭에 **없음이 확인됨**(실측: AX 트리) |
| 5 | 마지막 확인 일시 표시 | `General` 탭에 **없음이 확인됨**(실측: AX 트리) |
| 6 | 지금 확인 버튼 | ⭐ **`General` 탭에는 없다.** 대신 **메뉴바에 `Check for Updates…` 항목**으로 실재한다(실측: AX 트리, `app-bundle-analysis.md` §6.5) |

3~5번 항목이 Sparkle 자체 UI(별도 창) 안에 있을 가능성은 `(미확정)` 으로 남긴다 — `General` 탭에는 없다는 것만 실측으로 확정됐을 뿐, Sparkle 이 내부적으로 이 옵션들을 노출하는 다른 화면이 있는지는 관찰하지 못했다.

`General` 탭의 실측 전량(항목 순서 그대로, `app-bundle-analysis.md` §6.4): `Launch on login` 체크박스 · `v1.66 (66)` 버튼 · `Check for updates automatically` 체크박스(☐) · `Hide menu bar icon` 체크박스 · 부제 `When hidden, relaunch from Finder to open.` · `Menu bar icon` 팝업 · `Remove Oldest Activation` 버튼(`F-12` 소관) · `Purchase` 버튼(`F-12` 소관). 업데이트 관련 컨트롤은 이 중 `Check for updates automatically` 하나뿐이다.

## 5. 엣지 케이스와 실패 모드

1. **서명 검증 실패** — 손상되었거나 위조된 아티팩트. **어떤 경우에도 설치를 진행하지 않는다.** 다운로드 파일은 폐기하고, 사용자에게 오류를 노출한다 — 실제 문구는 "The update is improperly signed and could not be validated."(Sparkle 프레임워크 원문, 실측: 번들 문자열, §3.4). 자동 재시도 여부는 정책 미확정이나, 동일 항목을 무한 재시도하지 않아야 한다 `(추정)`.
2. **다운로드 중단** — 네트워크 단절, 사용자의 취소, 절전 진입 등으로 다운로드가 끊긴다. 부분 파일은 폐기하고 처음부터 재시도한다(Sparkle 표준 흐름상 다운로드 재개는 지원되지 않는 것으로 본다 `(추정)`). 실제 오류 문구는 "An error occurred while downloading the update. Please try again later."(Sparkle 프레임워크 원문, §3.4).
3. **디스크 공간 부족** — `.dmg` 다운로드 또는 마운트 후 `.app` 복사 단계에서 공간이 부족하면 설치를 취소하고 기존 `.app` 을 그대로 유지한다. 사용자에게 원인을 알린다.
4. **`.app` 이 쓰기 불가 위치에 있음(`/Applications` 권한 부족)** — 앱이 관리자 권한 없이 쓸 수 없는 위치에 설치되어 있으면 교체가 실패한다. Sparkle 표준은 필요 시 관리자 자격 증명 프롬프트를 띄우는 경로를 갖고 있는 것으로 알려져 있다 `(추정)`. 실패 시 원본 앱을 유지한다.
5. **앱이 다른 계정 소유** — 다중 사용자 환경에서 설치 소유자와 현재 실행 계정이 다르면 교체 권한이 없어 실패한다. 사용자에게 수동 개입(관리자에게 요청 등)을 안내해야 한다.
6. **델타 적용 실패 → 전체 폴백** — 델타 다운로드는 성공했으나 서명 검증 또는 바이너리 패치 적용이 실패하면, 전체 `.dmg` 다운로드로 자동 전환한다(§3.3-4). 사용자에게는 "업데이트 방식이 바뀌었다"는 사실보다 "업데이트가 계속 진행 중"이라는 정보만 노출하면 충분하다 `(추정)`.
7. **appcast 파싱 실패** — `updates.xml` 요청이 네트워크 오류로 실패하거나, 응답이 잘못된 XML/HTML(예: 서버 오류 페이지)이면 파싱 실패로 처리한다. 자동 확인이면 조용히 유휴로 복귀해 다음 주기를 기다리고, 수동 확인이면 사용자에게 실패를 알린다 — 실제 문구는 "An error occurred in retrieving update information. Please try again later."(Sparkle 프레임워크 원문, §3.4). 압축 해제 단계의 별도 오류도 있다: "An error occurred while extracting the archive. Please try again later."
8. **`minimumSystemVersion` 미달** — 현재 macOS 버전이 appcast 의 모든 항목의 게이트를 통과하지 못하면 "업데이트 없음"으로 처리한다. 이는 실제로 조사 시점 appcast 가 이미 `12.0` 게이트를 걸어 두었다는 사실(§1.5)로 뒷받침된다.
9. **업데이트 후 TCC 권한 소실** — 신규 `.app` 이 기존과 **다른 코드 서명 주체**(다른 인증서, 또는 ad-hoc 서명)로 서명되어 있으면 `csreq` 가 달라져 Accessibility/Screen Recording/Input Monitoring 권한이 전부 초기화된다(조사 노트 §3.2). 사용자는 FAQ 에 문서화된 "Unable to initialize Superkey" 증상(조사 §1.3)을 겪게 된다. **업데이트가 권한을 깨지 않을 조건은 "신·구 `.app` 이 동일한 Developer ID 인증서로 서명되어 있을 것"** 하나뿐이다. 이 조건이 깨지는 경우(예: 릴리스 파이프라인에서 잘못된 인증서로 서명)는 배포 사고이지 정상 실패 모드가 아니므로, CI/릴리스 체크리스트에서 사전에 차단해야 한다.

   ⭐ **실측 근거 보강**: 조사된 v1.66 번들의 서명 정보(`app-bundle-analysis.md` §1.1, 실측: `codesign -dv --entitlements :-`)는 Team ID **`XSYZ3E4B7D`**, Hardened Runtime **활성**, entitlement 는 **`com.apple.security.cs.allow-jit` 단 하나**다. 이 조합(같은 Team ID + 같은 entitlement 세트)이 유지되는 한 `csreq` 가 안정적이라는 전제가 성립한다 — 릴리스 파이프라인은 매 빌드마다 이 세 값(Team ID, Hardened Runtime 플래그, entitlement 목록)이 이전 릴리스와 동일한지 검증해야 한다.

   ⭐ **`F-12`(라이선싱)와의 상호 참조**: `docs/spec/licensing-and-trial.md` §3.9 가 실측으로 확인한 **자체 코드 서명 검증**(`SecStaticCodeCreateWithPath` · `SecCodeCopySigningInformation` 등, 요구사항 문자열 `anchor apple generic`)은 이 실패 모드에 새로운 제약을 더한다 — 업데이트로 교체된 신규 `.app` 은 TCC 권한을 유지하기 위해 **동일 서명 주체**를 만족해야 할 뿐 아니라, `anchor apple generic` 요구사항(Apple 발급 인증서 체인)도 계속 통과해야 **라이선스 자체 검증도 함께 통과**한다. 즉 서명 사고 하나가 TCC 재승인 요구와 라이선스 무효화를 **동시에** 일으킬 수 있다 — 두 문서(`F-12` §3.9, `F-13` 본 항목)는 같은 서명 사고를 서로 다른 증상으로 관찰하는 셈이다.
10. **재시작 시 modifier stuck** — 리매핑 엔진이 `CGEventTap` 을 쥔 채, 그리고 소스 키(hyper/meh/bleh 또는 프리셋)가 물리적으로 눌려 있는 상태에서 업데이트 재시작이 트리거되면, 정리 없이 종료 시 modifier 가 시스템에 눌린 채로 남을 위험이 있다(`F-05`/`F-08` 이 정의하는 Active 상태와 동일 계열 문제). **재시작 직전 반드시 F-10 의 종료 전 정리 계약을 거쳐, 눌려 있는 모든 합성 modifier 에 대해 keyUp/flagsChanged-off 를 강제로 내보낸 뒤에만 프로세스를 종료**해야 한다.
11. **오프라인** — 네트워크 자체가 없는 상태. 자동 확인은 실패를 삼키고 다음 주기를 기다리며 사용자에게 알리지 않는다. 수동 확인은 실패를 알린다.
12. **중간자 공격 시도** — appcast 요청과 아티팩트 다운로드 모두 HTTPS 로만 이루어져야 하며 평문 다운그레이드를 허용하지 않는다. 설령 공격자가 appcast XML 자체(또는 `enclosure url`)를 통째로 변조하더라도, 앱에 임베드된 EdDSA 공개키로 서명 불일치가 검출되어 설치가 거부된다 — appcast 무결성이 아니라 **최종 아티팩트 서명**이 신뢰의 근원이다.
13. **롤링 윈도우로 인한 원거리 버전 스킵** — 매우 오래된 버전(현재 appcast 의 델타 커버리지 5~6개 버전보다 더 뒤처진 경우)에서 확인하면 델타 후보가 애초에 존재하지 않아 항상 전체 `.dmg` 로 업데이트된다. 이는 실패가 아니라 정상 동작이지만, 사용자에게는 "왜 매번 전체 다운로드인지"가 설명되지 않을 수 있다.
14. **확인이 중복 트리거됨** — 백그라운드 주기 확인이 진행 중인 동안 사용자가 수동으로 "지금 확인"을 누르는 경우, 두 요청이 경쟁하지 않도록 이미 "확인 중" 상태면 새 요청을 무시하거나 기존 요청에 합류시켜야 한다.
15. **⭐ 설치된 버전이 appcast 최신 버전보다 새로운 경우(`appVersionTooNew`)** — 실행 파일에서 확인되는 SuperKey 자체 키(§3.4)로, 사용자가 베타·수동 설치 등으로 appcast 가 광고하는 최신 버전보다 더 새로운 빌드를 갖고 있는 상태를 SuperKey 가 별도로 인지하고 처리한다는 뜻이다 `(미확정 — 이름으로부터의 해석, 실제 UI 문구·동작은 확인하지 못했다)`. 합리적 기본값으로는 "업데이트 없음"과 동일하게 조용히 처리하는 것을 제안하되, 원본이 실제로 경고를 띄우는지는 확인되지 않았다.

## 6. 필요한 플랫폼 API

`rust-macos-capability-notes.md` §2.8, §3 기준.

| API / 구성요소 | 용도 | 비고 |
| :--- | :--- | :--- |
| `Sparkle.framework` **2.9.2**(`SPUStandardUpdaterController` 계열, 실측: 번들 버전) | appcast 조회·파싱, EdDSA 서명 검증, 델타 판정·적용, `.dmg` 마운트, `.app` 교체, 재시작 트리거 | 이 기능의 실질 로직 대부분이 이 프레임워크 **내부**에서 수행된다. Rust 코드가 appcast XML 을 직접 파싱하거나 EdDSA 를 직접 검증하지 않는다. 프레임워크 내부에 `Autoupdate` · `Updater.app` · `XPCServices`(`Downloader.xpc` · `Installer.xpc`)가 포함되어 있고 WebKit 을 링크한다(실측: 번들 구조) |
| Sparkle 의 종료 전 훅(재시작 직전 콜백) | 앱이 스스로 종료되기 전 정리할 기회를 얻는 지점 | `F-10` 의 event tap 해제·modifier 강제 해제 로직을 이 훅에 연결해야 한다(§5-10) |
| `codesign` / Developer ID 인증서 | 릴리스 아티팩트 서명 — 신·구 버전이 동일 서명 주체를 유지해야 TCC 권한이 보존됨(조사 노트 §3.2, §5-9) | 클라이언트 코드가 아니라 **릴리스 파이프라인**의 요구사항. 실측 기준값: Team ID `XSYZ3E4B7D`, Hardened Runtime 활성, entitlement `com.apple.security.cs.allow-jit` 단 하나(§5-9) |
| (서버 측) EdDSA 키 페어 생성·서명 도구, `BinaryDelta` | appcast 서명, 델타 아티팩트 생성 | §7 "서버 측 산출물" 참조 |

Rust 애플리케이션 코드 레벨에서 직접 호출할 저수준 macOS API(`CGEventTap`, `AXUIElement` 등)는 이 기능에 없다 — 있다면 그것은 `F-10`(재시작 훅과 정리 로직)의 몫이다.

## 7. 구현 접근

**판정: Rust 바인딩** — `tauri-plugin-sparkle-updater` 0.2.5 로 `Sparkle.framework` 를 그대로 쓴다.

사전 빌드된 `.framework` 를 번들에 넣고 Rust 크레이트로 호출할 뿐, Swift/Objective-C 소스를 별도로 컴파일하지 않으므로 `네이티브 shim 불가피` 가 아니다. 다만 다른 기능과 달리 **번들에 서드파티 프레임워크가 하나 추가**되고 그것도 코드 서명 대상이 된다는 점에서 배포 파이프라인의 비용이 다르다.

### 7.1 3안 비교

| 선택지 | appcast 재현 | EdDSA 서명 | 델타 | 재시작 | 성숙도 | `tauri dev` |
| :--- | :--- | :--- | :--- | :--- | :--- | :--- |
| ① `tauri-plugin-updater` 2.10.1 | ❌ — Tauri 자체 JSON 포맷, Sparkle appcast 아님 | Tauri 자체 서명 방식(minisign 계열) | ❌ 없음 | 지원 | 공식·크로스플랫폼, 안정 | 동작 |
| ② `tauri-plugin-sparkle-updater` 0.2.5 | ✅ — Sparkle.framework 를 그대로 감쌈 | ✅ EdDSA 그대로 | ✅ 있음 | ✅ 자동 | 0.x 커뮤니티, `Sparkle.framework` 번들 필요 | ❌ 미동작(`.app` 번들 필요) |
| ③ `objc2` 로 Sparkle 직접 바인딩 | ✅ (직접 구현) | ✅ (Sparkle 위임) | ✅ (Sparkle 위임) | ✅ (Sparkle 위임) | 완전한 제어, 비용 큼 | `.app` 번들 필요는 동일 |

### 7.2 근거

이 명세서의 §3 전체(appcast 스키마, EdDSA 서명, 델타)는 **원본 SuperKey 가 실제로 배포 중인 Sparkle 인프라를 그대로 재현**하겠다는 전제 위에 서 있다. 조사에서 appcast XML 원문을 직접 확보했다는 것 자체가, 클론도 같은 포맷으로 서비스할 수 있고 서비스해야 한다는 근거가 된다.

- **① `tauri-plugin-updater` 를 기각한 이유**: 공식 지원과 크로스플랫폼이라는 장점은 이 제품(macOS 전용 유틸리티)에서 의미가 작다. 결정적으로 appcast 포맷과 서명 방식이 다르므로, 이 문서 §3 에서 확정한 스키마 전체를 버리고 처음부터 다른 릴리스 파이프라인을 설계해야 한다 — 원본 재현이라는 이번 클론 프로젝트의 원칙과 어긋난다. 델타가 없다는 점도 원본 대비 명백한 기능 축소다.
- **② `tauri-plugin-sparkle-updater` 를 채택한 이유**: appcast·EdDSA·델타·자동 재시작을 **전부 그대로** 얻는다. 이는 §3 의 상태 머신과 델타 판정 규칙을 클라이언트 코드로 재발명할 필요가 없다는 뜻이다 — Sparkle.framework 내부에 이미 구현되어 있다. 단점은 명확히 인지한다: 0.x 버전대의 커뮤니티 유지보수 플러그인이라 안정성 리스크가 있고, `Sparkle.framework` 를 앱 번들에 포함해야 하며(코드 서명 대상이 하나 늘어남), `tauri dev` 로는 테스트할 수 없다(`.app` 번들이 있어야 동작 — 이는 rust-macos-capability-notes.md §3.3 이 지적하는 "권한이 필요한 기능은 `tauri build` 산출물로 테스트해야 한다"는 제약과 정확히 같은 계열의 개발 루프 문제다).
- **③ `objc2` 직접 바인딩을 기각한 이유**: ②가 이미 Sparkle.framework 를 통째로 감싸 필요한 기능을 전부 제공하는 상황에서, Sparkle 의 Objective-C API 표면(`SPUUpdater`, `SPUUpdaterDelegate` 등)을 손으로 바인딩하는 비용을 정당화할 근거가 없다. ②의 0.x 리스크가 실제로 프로덕션에서 감당 불가능하다고 판명되는 경우에만 재검토할 대안으로 남겨둔다.

### 7.3 델타 업데이트를 초기 범위에 넣을지

**결정: 초기 범위에서 제외한다.** 클라이언트 측에서는 ②가 델타를 "공짜로" 지원하므로 코드 비용이 없지만, 델타를 실제로 동작시키려면 **서버 측**에서 매 릴리스마다 직전 5개 버전 각각에 대해 `BinaryDelta` 를 생성·서명·호스팅하고, 5~6개의 델타 조합을 실제로 테스트해야 한다(§7.4). 초기 사용자 규모가 작은 단계에서는 이 운영 비용이 183KB 절감이라는 이득에 비해 과하다고 판단한다. appcast 스키마 자체는 `<sparkle:deltas>` 를 이미 지원하도록 설계해 두므로(§3.1), 필요해지면 서버 파이프라인만 추가하면 되고 클라이언트 재작업은 없다.

### 7.4 서버 측 산출물 — 릴리스 파이프라인 개괄

클론이 재현해야 할 서버(릴리스) 측 작업:

1. **`.dmg` 빌드·서명** — `tauri build` 산출 `.app` 을 Developer ID 인증서로 서명(§5-9 의 TCC 보존 조건)하고 공증(notarization) 후 `.dmg` 로 패키징.
2. **EdDSA 키 페어 관리** — 개인키는 릴리스 서버/CI 시크릿에만 존재, 공개키는 앱 바이너리에 컴파일 타임 임베드. 개인키 유출 시 즉시 키 교체와 구버전 신뢰 철회 절차가 필요하다 `(추정, 미해결 질문 승계)`.
3. **appcast 서명·XML 생성** — 새 `.dmg` 의 EdDSA 서명(`sparkle:edSignature`)을 계산하고, `<item>` 을 appcast 맨 앞에 추가한다. 롤링 윈도우 정책(현재 10개 유지, 조사 §0)에 맞춰 오래된 항목을 제거한다.
4. **(초기 범위 제외) 델타 생성** — 직전 5개 버전 각각에 대해 `BinaryDelta` 로 `.delta` 파일을 생성·서명하고 `<sparkle:deltas>` 에 추가.
5. **호스팅** — `updates.xml`, `.dmg`, (해당 시) `.delta` 파일을 HTTPS 로 서빙. §5-12 의 중간자 공격 방어는 전송 계층(HTTPS)과 아티팩트 서명(EdDSA) 이중 방어로 구성된다.

## 8. 수용 기준

- [ ] 자동 확인이 켜져 있으면 앱은 **48시간**(`SUScheduledCheckInterval = 172800`, 실측: Info.plist)마다 백그라운드에서 `updates.xml` 을 조회한다. 자동 확인의 **출고 기본값은 OFF** 다(실측: defaults·AX 트리, §4.1).
- [ ] 메뉴바 메뉴의 `Check for Updates…` 항목(실측: AX 트리)에서 수동으로 업데이트를 확인하면 즉시 appcast 를 재조회하고, 결과("업데이트 있음"/"최신 버전")를 사용자에게 표시한다. `General` 탭에는 이 컨트롤이 없다(§4.2).
- [ ] appcast 의 `sparkle:minimumSystemVersion` 이 현재 macOS 버전보다 높은 `<item>` 은 업데이트 후보에서 제외된다.
- [ ] **EdDSA 서명 검증에 실패한 아티팩트(전체 `.dmg`, 델타 모두)는 어떤 경우에도 설치되지 않는다.** 검증 실패 시 다운로드 파일은 폐기되고, 사용자에게 오류가 노출된다.
- [ ] 서명 검증을 통과한 아티팩트만 `.dmg` 마운트 → `.app` 교체 절차로 진행한다 — 검증과 설치 사이에 우회 경로가 없다.
- [ ] 현재 버전에서 appcast 상의 델타 커버리지(직전 5~6개 버전) 안에 있으면 델타를 우선 시도하고, 델타 다운로드·적용·검증 중 하나라도 실패하면 전체 `.dmg` 다운로드로 자동 폴백한다.
- [ ] 사용자가 자동 확인을 끄면 이후 백그라운드 조회가 발생하지 않으며, 수동 확인 경로는 계속 동작한다.
- [ ] 업데이트 설치 후 재시작된 신규 `.app` 은 이전 `.app` 과 동일한 코드 서명 주체(Developer ID)로 서명되어 있으며, Accessibility/Screen Recording/Input Monitoring 권한이 재승인 절차 없이 유지된다.
- [ ] 리매핑 엔진이 event tap 을 쥐고 있거나 modifier 가 Active 상태인 도중 업데이트 재시작이 트리거되면, `F-10` 의 종료 전 정리 계약(눌려 있는 modifier 해제)이 완료된 뒤에만 프로세스가 종료된다.
- [ ] appcast 파싱 실패(네트워크 오류·잘못된 XML)가 발생해도 앱은 크래시하지 않는다 — 자동 확인이면 다음 주기까지 조용히 대기하고, 수동 확인이면 오류를 노출한다.
- [ ] appcast 조회와 아티팩트 다운로드는 HTTPS 로만 이루어지며, 평문 HTTP 로의 다운그레이드를 허용하지 않는다.
- [ ] 디스크 공간 부족, `/Applications` 쓰기 권한 부족, 다른 계정 소유 등 설치 단계 실패 시 원래 설치된 `.app` 은 그대로 유지되고 실패 사유가 사용자에게 표시된다.

## 9. 미해결 질문

### 9.0 ⭐ 이번 실측으로 해소된 질문

`app-bundle-analysis.md` 실측(Info.plist·defaults·AX 트리·번들 심볼/문자열)으로 다음이 해소되어 아래 목록에서 제외했다: **확인 주기**(48시간, `SUScheduledCheckInterval = 172800`, §4.1) · **자동 확인의 기본값**(OFF, §4.1) · **EdDSA 서명 검증**(`SUPublicEDKey` 실존으로 확정, §4.1) · **Sparkle 버전**(2.9.2, §6) · **`General` 탭에서 업데이트 UI 가 차지하는 범위**(`Check for updates automatically` 체크박스 하나뿐, §4.2) · **"지금 확인"의 소재**(메뉴바 `Check for Updates…`, `General` 탭 아님, §2·§4.2). 아래는 **여전히 남은** 질문만 추린다.

1. **자동 다운로드(승인 전 백그라운드 선다운로드) 옵션의 존재 여부** — Sparkle 은 이 옵션을 지원할 수 있으나, `General` 탭에는 없음이 확인되었다(§4.2). Sparkle 자체 UI(별도 창) 안에 있을 가능성은 `(미확정)`.
2. **프리릴리스/베타 채널의 존재 여부** — appcast 원문에 채널 분기를 시사하는 필드가 확인되지 않았고, `General` 탭에도 없음이 확인되었다(§4.2). 존재하지 않을 가능성이 높으나 단정할 근거는 없다.
3. **델타 업데이트의 실제 동작** — appcast 스키마(§3.1)와 크기 비교는 확인됐으나, 델타 다운로드·적용·서명 검증의 실제 실행 흐름은 관찰하지 못했다(구매·업데이트 버튼을 실제로 눌러야 해 이번 조사 범위 밖).
4. **`appVersionTooNew` 의 정확한 처리** — 번들 문자열 키의 존재는 확인됐으나(§3.4, §5-15), 실제 UI 문구·동작은 이름으로부터의 해석일 뿐이다.
5. **관리자 권한이 필요한 설치 시나리오의 정확한 UX** — `/Applications` 쓰기 권한이 없을 때 Sparkle 이 관리자 자격 증명 프롬프트를 띄우는지, 아니면 단순 실패로 끝나는지 원본에서 확인되지 않았다.
6. **`sparkle:deltaFromSparkleLocales` 의 정확한 소속 요소와 의미** — 어느 XML 요소에 붙는 속성인지, "델타가 커버하는 로케일" 인지 "해당 버전이 번들한 로케일" 인지 원문 truncated 인용만으로는 확정할 수 없다.
7. **`pubDate` 의 정확한 형식** — 요일·일·월·연도까지는 여러 항목에서 확인되나(예: `Tue, 23 Jun 2026`), 시각·타임존을 포함한 완전한 RFC822 형식인지는 확인되지 않았다.
8. **v1.21~v1.50 구간의 릴리스가 델타·서명 관행에서 현재와 동일했는지** — 롤링 윈도우로 과거 appcast 항목이 소실되어 있어(조사 §0, Q1), Sparkle 2.x/EdDSA 채택 시점을 특정할 수 없다. 이 클론 명세는 조사 시점(v1.66)의 관행만을 확정으로 다룬다.
9. **EdDSA 개인키 유출·교체 시 구버전 신뢰를 철회하는 절차** — 원본에 공개된 사고 대응 절차가 없어, §7.4 의 서버 파이프라인 항목은 일반적인 모범 사례를 적용한 설계 제안이지 원본 재현이 아니다.
10. **델타 미적용 시 사용자에게 그 사실을 알리는지** — §3.3-4 의 자동 폴백이 조용히 일어나는지, 아니면 "전체 다운로드로 전환됨"을 노출하는지 원본에서 확인할 수 없었다.
11. **실제 업데이트 화면의 레이아웃** — Sparkle 표준 문자열(§3.4)의 존재는 확인했으나, 실제 신버전 발견 시 뜨는 창의 배치·버튼 구성은 관찰하지 못했다(신버전이 없는 상태에서 조사했다).
12. **`lastVersion` 의 실제 용도** — defaults 에 값이 있다는 사실은 확인됐으나(§3.5), 실제로 어떤 마이그레이션·안내를 트리거하는지는 이름으로부터의 해석일 뿐이다.
