# 명세 검증 보고서 — SuperKey v1.66 (66) 실측 대조

> 검증일: 2026-08-30 · 대상 명세: `docs/spec/*.md` (PR #2 에서 **웹 조사만으로** 작성) · 이슈 #3
> 1차 근거: [`app-bundle-analysis.md`](app-bundle-analysis.md) · [`screenshots/`](screenshots/)

이 문서는 **판정 결과의 요약**이다. 각 사실의 근거는 `app-bundle-analysis.md` 에, 반영된 명세는 `docs/spec/*.md` 에 있다.

---

## 0. 한눈에

| 분류 | 뜻 | 건수 |
| :--- | :--- | :---: |
| ✅ **확인** | 기존 명세가 맞았다. `(추정)` 을 실측으로 승격 | **31** |
| ⚠️ **수정** | 기존 명세가 틀렸다. 고쳤다 | **19** |
| ⭐ **신규** | 기존 명세에 없던 것. 추가했다 | **24** |
| ❓ **미확정** | 확인하지 못했다. **추측으로 메우지 않고 남겼다** | **21** |

검증 방법 3종:
1. **스크린샷 5장 정독** — 이 세션이 직접 판독 (이슈 #3 전사본의 요약을 넘어 비활성 상태·배치·아이콘까지)
2. **번들 정적 분석** — `plutil`·`codesign`·`otool -L`·`nm -u`·`strings`·nib 리소스. ⛔ 디컴파일·역어셈블 없음, ⛔ 에셋 추출 없음
3. **실행 중 앱의 Accessibility 트리 판독** — 4개 탭 전 컨트롤, 팝업 선택지 전량, 슬라이더 범위, 메뉴바 메뉴. 설정 변경은 §5 의 4건뿐이며 전부 복원

⭐ **가장 큰 성과 하나**: `superkey-inventory.md` §7 Q3("각 설정의 출고 기본값")이 완전히 해소되었다. 아무 설정도 바꾸지 않은 상태의 `~/Library/Preferences/com.knollsoft.Superkey.plist` 에 **키가 7개밖에 없다** — 즉 **부재 = 기본값**이고, AX 실측이 거의 전부 ☐ 로 나왔다. 이 하나가 아래 "수정" 19건 중 6건의 근거다.

---

## 1. ✅ 확인 — 기존 명세가 맞았다

`(추정)` → 실측 승격. 웹 조사가 어디까지 옳았는지의 기록이기도 하다.

| # | 항목 | 명세 | 근거 |
| :--- | :--- | :--- | :--- |
| C-01 | 번들 ID `com.knollsoft.Superkey` | F-09 | Info.plist |
| C-02 | Vision OCR 사용 | F-02 | `VNRecognizeTextRequest` 등 심볼 6종 |
| C-03 | `CGEventTap` 계열 사용 | F-05·F-07·F-08 | `CGEventTapCreate`/`Enable`/`IsEnabled` 등 |
| C-04 | AXUIElement 사용 | F-02·F-04·F-11 | `AXUIElementCopyAttributeValue` 등 9종 |
| C-05 | Carbon `UCKeyTranslate`·`TIS*` 로 레이아웃 독립 판정 | F-14(B) | 심볼 확정 |
| C-06 | **`MultitouchSupport` 비공개 프레임워크 의존** | F-06 | 번들에서 **유일한 PrivateFramework**, 함수 9종 |
| C-07 | **트랙패드 영역 후보 = 4 코너 + 상단 가장자리** | F-06 | ⭐ 추정이 **정확히 맞았다**. 실측 5종 |
| C-08 | Sparkle 2.x + EdDSA 서명 | F-13 | `Sparkle.framework` 2.9.2, `SUPublicEDKey` |
| C-09 | **Paddle 사용** | F-12 | `Paddle.framework` 1.0.0, Classic 계열, 제품 ID `750314` |
| C-10 | `LSUIElement` 상주 앱 | F-10 | Info.plist |
| C-11 | 비샌드박스 | F-10·`platform-constraints` | entitlement 에 app-sandbox 없음 |
| C-12 | Developer ID + Hardened Runtime | `platform-constraints` §3 | Team ID `XSYZ3E4B7D` |
| C-13 | Universal (`x86_64` + `arm64`) | `platform-constraints` | `file` |
| C-14 | 최소 macOS **12.0** | `platform-constraints` | `LSMinimumSystemVersion` |
| C-15 | 환경설정에 Apply/OK 버튼 없음, 즉시 반영 | F-09 | AX 트리 |
| C-16 | `Change click modes with modifier keys` 기본 ☑ | F-04 | ⭐ Seek 탭에서 **유일한** 기본 켜짐 |
| C-17 | `Include shift in hyper key` 기본 ☑ | F-05 | AX + `hyperFlags` |
| C-18 | `Only Seek in the frontmost window` 기본 ☐ | F-02 | AX |
| C-19 | bleh 는 option 을 포함하지 않는다 (`⌃⌘⇧`) | F-05 | 라벨 원문 |
| C-20 | meh 는 command 를 포함하지 않는다 (`⌃⌥⇧`) | F-05 | 라벨 원문 |
| C-21 | `Remap caps lock to:` 팝업은 **출력** 키 열거형이며 소스 키 열거형과 다르다 | F-08 | ⭐ 추정이 맞았다. 50종 vs 35종 |
| C-22 | `Remap paste …` 팝업은 어느 물리 ⌘ 가 트리거인지 고르는 위젯 | F-08 | ⭐ 추정이 맞았다 (+ `Hyper key` 선택지 추가) |
| C-23 | `Caps lock + [H J K L]` 인라인 팝업이 대체 키셋 선택 | F-08 | ⭐ 추정이 맞았다. `H J K L` / `I J K L` |
| C-24 | Quick press 임계값이 caps lock·shift 양쪽에 쓰인다 | F-07·F-08 | 슬라이더 하나가 두 프리셋을 지배 |
| C-25 | 이벤트 탭 재활성화·깨어남 재초기화가 필요하다 | F-07 | ⭐ 문자열로 워치독·절전/깨어남 훅 실재 확인 |
| C-26 | 충돌이 조용히 무시되어서는 안 된다 | F-08 | ⭐ 원본이 **대화상자**로 구현 — 요구가 옳았다 |
| C-27 | `NSUserDefaults` 를 설정 저장에 쓴다 | F-09 | plist 실재 |
| C-28 | 앱은 Storyboard 기반 AppKit 앱 | F-09 | `NSMainStoryboardFile` |
| C-29 | 사이드바 4탭 (`Seek`·`Hyperkey`·`Presets`·`General`) | F-09 | AX |
| C-30 | 권한 모달이 Accessibility 를 시스템 설정에서 켜도록 안내 | F-11 | 스크린샷 + nib |
| C-31 | `Semicolon highlights next match` 가 물리 키코드 기준 | F-01·F-03 | 키 `semicolonCycleSeek` |

---

## 2. ⚠️ 수정 — 기존 명세가 틀렸다

| # | 기존 명세의 서술 | 실측 | 고친 문서 |
| :--- | :--- | :--- | :--- |
| **F-01** | ⭐ **출고 기본값**: 여러 항목을 ☑ 로 기술 (랜딩 홍보 스크린샷을 그대로 옮김) | **사실상 전부 ☐.** 기본 ON 은 `Include shift in hyper key` · `Change click modes with modifier keys` · (조건부) `Match on more than one character` **셋뿐** | F-01·F-02·F-03·F-04·F-05·F-08·F-09·F-10·F-13 전부 |
| **F-02** | ⭐ **8개 로케일 번들, 3개 RTL** — `sparkle:deltaFromSparkleLocales` 근거 | **오독.** 앱 본체는 `Base.lproj` 하나, `CFBundleLocalizations` 없음 → **영어 단일, 현지화 없음.** 그 7개는 Sparkle 프레임워크의 45개 로케일 중 일부다 | F-14, `superkey-inventory.md`, README (D4 신설) |
| **F-03** | ⭐ **ScreenCaptureKit 으로 화면 캡처** (F-02 는 최소 OS 12.3 상향을 제안) | **쓰지 않는다.** `CGDisplayCreateImage` / `CGDisplayCreateImageForRect` + `CGWindowListCopyWindowInfo`. `LSMinimumSystemVersion = 12.0` 유지 → **제품 결정 D1 해소, 12.0 유지** | F-02, `platform-constraints` §0.2(a)·P2, README |
| **F-04** | ⭐ **`Apply modifiers to …` Click·Drag·Move ☑, Scroll ☐** | **`Click` 하나만 ☑**, 나머지 셋 ☐ | F-05 |
| **F-05** | ⭐ **권한 3종(Accessibility·Input Monitoring·Screen Recording)을 온보딩에서 함께 요청** | 명시적으로 확인하는 것은 **`AXIsProcessTrusted` 하나뿐.** `IOHIDCheckAccess`/`IOHIDRequestAccess`·`CGPreflightScreenCaptureAccess`·`AXIsProcessTrustedWithOptions` **전부 없음**. Input Monitoring 은 실패-재시도로, Screen Recording 은 첫 캡처 시 OS 프롬프트로 흡수 | F-11, F-02, F-07, `platform-constraints` §0.2(c) |
| **F-06** | ⭐ **`Quick press duration` 최소 200 · 최대 1600 · 간격 200ms** (눈금 8칸 역산) | **최소 250 · 최대 2000 · 현재 1000ms.** 눈금 개수로부터의 역산은 신뢰할 수 없다 | F-07, F-08 |
| **F-07** | 중첩 체크박스는 상위가 꺼지면 **비활성화(dimmed)** | 종속 표현이 **두 가지**다 — `Only show while the remapped key is held` 는 dimmed, `Match on more than one character` 는 **완전히 숨김**(AX 트리에서 사라짐) | F-09, F-01, F-02 |
| **F-08** | `Match on more than one character` 는 불리언 체크박스 | **정수 `minAxCharCount = 2`** 로 저장된다. 최소 글자 수 파라미터다 | F-02, F-09 |
| **F-09** | AX 와 OCR 의 병합 우선순위 (기존 서술이 반대) | **OCR 이 기본 소스, AX 는 최전면 창 한정 보강.** 중복 시 **OCR 이 AX 를 대체**한다 (ⓘ 팝오버 원문) | F-02, F-03 |
| **F-10** | Seek 클릭은 매치 **중심점**을 클릭 / modifier 로 좌·우·중간 클릭을 고른다 | 클릭 모드 **7종**이고 우클릭·중간클릭이 없다. `Click at beginning of match` / `at end of match` — **시작/끝** 지점이다 | F-04 |
| **F-11** | Seek 검색 바는 화면 중앙 고정 요소 | **위치·크기가 저장되는 독립 창** (400×40pt, `NSWindow Frame EntryBarWindow`) | F-01, F-03 |
| **F-12** | 오버레이는 단일 창 | `overlayWindows` **복수형** — 디스플레이별 창으로 보인다 | F-03 |
| **F-13** | 업데이트 확인 주기 24시간 (Sparkle 기본값 추정) | **`SUScheduledCheckInterval = 172800` = 48시간**이 명시적으로 설정되어 있다 | F-13 |
| **F-14** | 자동 업데이트 확인 기본 ON 추정 | **기본 OFF** (`SUEnableAutomaticChecks = false`) | F-13 |
| **F-15** | `Launch at login` 기본 ON 추정 ("이런 유틸리티는 흔히 ON") | **기본 OFF.** 라벨도 `Launch on login` | F-10 |
| **F-16** | 메뉴바 항목은 `Show icon in menu bar` (표시 극성) | `Hide menu bar icon` (**숨김 극성**) + 부제 | F-10, F-09 |
| **F-17** | 시스템 예약 단축키를 열거할 **공개 API 가 없어** 부분 대응만 가능 | 원본은 `kHISymbolicHotKeyCode`/`Enabled`/`Modifiers`(Carbon `CopySymbolicHotKeys`)를 쓴다 — 문서화되지 않았을 뿐 경로가 있다 | F-09, `platform-constraints` §0.3 |
| **F-18** | caps lock 리매핑·토글은 전부 `CGEventTap` 이벤트 합성 | **경로가 3종이다** — A `CGEventTap`, B IOHID `UserKeyMapping`(`hidutil` 병용), C `IOHIDSet/GetModifierLockState`. caps lock 잠금 토글은 이벤트 합성이 **아니다** | F-07, F-08, `platform-constraints` §0.3 |
| **F-19** | 권한 복구는 사용자 안내뿐 | **"권한이 어긋난(out of sync)" 진단 상태**와 **앱이 스스로 TCC 를 리셋하고 재시작하는 경로**가 실재한다. 그리고 event tap 생성 실패는 재시도가 아니라 **프로세스 종료**다 | F-11, F-07, F-10 |

### 2.1 "추정했으나 실재하지 않음" 으로 확인된 것

명세가 있을 것으로 추정했으나 **실제 UI 에 없는 것**들이다. 조용히 지우지 않고 "부재 확인" 으로 기록했다 — 그것도 검증의 산출물이다.

| 추정했던 항목 | 어디에 있을 것으로 추정했나 | 실측 |
| :--- | :--- | :--- |
| 언어 선택 | `General` 탭 | **없음** (현지화 자체가 없다) |
| 권한 상태 표시 (3종 배지 + 딥링크) | `General` 탭 | **없음** |
| 라이선스 키 입력란 + 활성화 버튼 | `General` 탭 | **없음** — Paddle 별도 창 소관 |
| 활성화된 기기 목록 / "이 기기 비활성화" | `General` 탭 | **없음** — `Remove Oldest Activation` 은 라이선스 키를 넣으면 서버가 가장 오래된 것을 회수하는 흐름이다 |
| `Reset preferences` / `Reset to defaults` | `General` 탭 | **없음** |
| 자동 다운로드 · 베타 채널 · 마지막 확인 일시 | `General` 탭 | **없음** ("지금 확인" 은 메뉴바의 `Check for Updates…` 로 실재) |

---

## 3. ⭐ 신규 — 기존 명세에 없던 것

| # | 발견 | 근거 | 반영 문서 |
| :--- | :--- | :--- | :--- |
| N-01 | **Seek 클릭 모드 7종** — `Just move cursor` · `Click at beginning of match` · `Click at end of match` · `Click and return cursor` · `Click, return, click` · `Double click and copy` · `Triple click and copy`. modifier 를 누른 채 enter 로 선택 | ⓘ 팝오버 nib | F-04 |
| N-02 | **Seek 활성화 경로가 3종** — `Quick press caps lock to execute:` 팝업의 **첫 항목이 `Seek`** 다 | AX 팝업 | F-01, F-07, F-08 |
| N-03 | **설정 충돌 감지 대화상자** 3종 — caps lock 리매핑 중복 / 방향키 프리셋 / home row 프리셋. "상대 설정을 끄고 이걸 켤까요?" 라고 **묻는다** | 실행 파일 문자열 | **F-15**(신규), F-08, F-09 |
| N-04 | **앱별 비활성화** — 메뉴바에 `Ignore <최전면앱이름>` | AX 메뉴 | F-10, F-07 |
| N-05 | **iCloud 설정 동기화** — 첫 실행 시 "기존 클라우드 설정을 가져올까요?" | 실행 파일 문자열 | **F-15**(신규) |
| N-06 | **캡처 전처리 파이프라인** — `CILanczosScaleTransform` → `CIPhotoEffectMono`/`Noir` → `CIMaximumComponent`/`Minimum` | 실행 파일 문자열 + CoreImage 링크 | F-02 |
| N-07 | **`hidutil` 커널 레벨 키 매핑** — `hidutil property -g UserKeyMapping`, `HIDKeyboardModifierMappingSrc`/`Dst` | 실행 파일 문자열 | F-07, F-08 |
| N-08 | **`IOHIDGet`/`SetModifierLockState`** — caps lock 실제 잠금·LED 직접 조작 | 심볼 | F-07, F-08 |
| N-09 | **키보드 핫플러그 대응** — `IOHIDManagerRegisterDeviceMatching/RemovalCallback`, 메뉴바 `Relaunch on Keyboard Connected` | 심볼 + AX 메뉴 | F-07, F-10 |
| N-10 | **이벤트 탭 워치독 + 재시작 디바운스** — "Checking key loop." / "Time since last exit: … s ago, no need to restart." | 실행 파일 문자열 | F-07, F-10 |
| N-11 | **복구 수단이 프로세스 재실행** — `Relaunch` · `Relaunch After Wake` · `Delay Relaunch After Wake` | AX 메뉴 | F-10, F-07 |
| N-12 | **자체 TCC 리셋 경로** — `Reset Priviliges & Restart Superkey` [sic] | 실행 파일 문자열 | F-11 |
| N-13 | **`unauthorizedMenu`** — 권한 없을 때 메뉴가 통째로 교체된다 | 실행 파일 문자열 | F-10, F-11 |
| N-14 | **Hyperkey 탭 조건부 체크박스 2개** — `Change menu bar icon when engaged` · `Provide haptic feedback when triggered` (트랙패드 항목에 종속, 숨김) | AX (토글 후 관찰) | F-05, F-06, F-10 |
| N-15 | **Magic Mouse 도 제스처 대상** — `MTMouseRegistrar` vs `MTTrackpadRegistrar`, "Unregistered Magic Mouse" | 실행 파일 문자열 | F-06 |
| N-16 | **손바닥·엄지 얹힘 거부**가 "one touch" 판정의 일부 — `restingThumb`·`restingPalm`·`filterLargeTouches`·`disablePalmRejection` 등 | 실행 파일 문자열 | F-06 |
| N-17 | **제스처 임계가 2단계** — 계열별로 `*TriggerThreshold` 와 `*CursorFreezeThreshold` 가 쌍을 이룬다 | 실행 파일 문자열 | F-06 |
| N-18 | **Stage Manager 전용 AX 처리** — `StageWindowAccessibilityElement` | 실행 파일 문자열 | F-02 |
| N-19 | **검색 바에 매치 목록 UI** — `EntryOutlineView` · `EntryBarTableRowView` · `matchesList` | 실행 파일 문자열 | F-01, F-03 |
| N-20 | **합성 클릭 되돌이 차단 장치** — `blockClicksUntil` · `stopNextMouseDown`/`Up` · `humanClickDownUp` vs `programmaticClickDownUp` | 실행 파일 문자열 | F-04 |
| N-21 | **자체 코드 서명 검증** — `SecStaticCodeCreateWithPath`, `anchor apple generic`, MAC 주소 기반 기기 식별 | 심볼 + 문자열 | F-12, `platform-constraints` §3.6 |
| N-22 | **로그인 항목 0.1초 재시도 루프** — "Unable to set launch on login to %{public}@. Retrying in 0.1s" + 헬퍼 핑퐁 | 실행 파일 문자열 | F-10 |
| N-23 | **`SMAppService` 와 `SMLoginItemSetEnabled` 이중 경로** — OS 버전 분기 | 심볼 | F-10 |
| N-24 | **`v1.66 (66)` 은 정적 라벨이 아니라 버튼**, 그리고 탭마다 창 크기가 다르다 (555×378 / 710×517 / 825×527 / 613×273 pt) | AX | F-09, F-10 |

---

## 4. ❓ 미확정 — 확인하지 못한 것

**추측으로 메우지 않았다.** 각 항목이 왜 미확정인지가 함께 적혀 있다. 전체 목록과 상세 사유는 [`app-bundle-analysis.md` §8](app-bundle-analysis.md), 기능별 상세는 각 명세의 **9절**에 있다.

| # | 항목 | 왜 확인하지 못했나 | 명세 |
| :--- | :--- | :--- | :--- |
| U-01 | **클릭 모드 7종 ↔ modifier 대응** | nib·AX 어디에도 매핑이 없다. `Record Modifiers`(`RecorderModifierCocoa`)로 사용자가 직접 녹화하는 구조로 보이나 그 UI 를 4개 탭에서 찾지 못했다 | F-04 |
| U-02 | **Seek 오버레이의 실제 표시 형태** (라벨 문자 집합·배치·색) | Seek 을 실제로 발동시키지 않았다 — Screen Recording 권한 프롬프트와 전체 화면 캡처가 따르고, 관찰 이득 대비 부작용이 크다고 판단했다 | F-03 |
| U-03 | **트랙패드 제스처의 실제 반응** | 물리 제스처 입력이 필요하다. 대신 영역 5종과 임계 파라미터 **이름**을 확정했다 | F-06 |
| U-04 | 제스처 임계 **수치** (`cornerTriggerThreshold` 등 4종) | 값이 코드 안에 있다. 디컴파일은 하지 않았다 | F-06 |
| U-05 | **quick press 임계 동작의 실제 체감** | 키 입력 타이밍 실험은 하지 않았다. 대신 슬라이더 범위(250–2000ms)를 확정했다 | F-07 |
| U-06 | `Quick press duration` 슬라이더의 **step 간격** | AX 가 min/max/value 만 노출한다 | F-07, F-08 |
| U-07 | **충돌 대화상자의 실제 모습·버튼 구성** | 충돌을 만들려면 caps lock 을 실제로 리매핑해야 해 부작용이 크다고 판단했다. 문자열로 존재와 대상 3종만 확정 | F-15 |
| U-08 | **`Menu bar icon` 팝업 2종의 정체** | 항목이 라벨 없는 이미지다. 에셋(`Assets.car`)은 **저작권 경계상 추출하지 않았다** | F-09, F-10 |
| U-09 | `Apply hyper to arrows` 의 **표시 조건** | `Caps lock + W A S D` 만 켜서는 나타나지 않았다. 조건을 재현하지 못했다 | F-08 |
| U-10 | `Relaunch on wake` 의 **표시 조건** | nib 에는 있으나 기본 상태 AX 트리에 없다 | F-10 |
| U-11 | Windows 키보드 리매핑(`winKeyRemapCheckbox`)의 **라벨·표시 조건** | Windows 키보드가 연결되어 있지 않다 | F-08 |
| U-12 | **iCloud 동기화의 저장소·범위** | 문자열로 존재만 확인. `NSUbiquitousKeyValueStore` 심볼을 확인하지 못했고, ⚠️ **대응 entitlement 도 없다** — 기능이 비활성이거나 다른 저장소일 수 있다 | F-15 |
| U-13 | `typeToSeekEnabledAppIDs` / `typeToSeekDisabledAppIDs` 의 기능 | 대응 UI 를 찾지 못했다 | F-10 |
| U-14 | hyper **quick press** 의 노출 경로와 의미 | 키(`quickHyperKeycode`·`executeQuickHyperKey`·`hyperDownTime`)로 **존재는 확정**, UI 는 4개 탭 어디에도 없다 | F-05, F-07 |
| U-15 | `Synthesize Caps Lock Remap` 메뉴 항목의 정확한 의미 | 라벨로부터 "경로 B 대신 A 강제" 로 해석되나 확인하지 못했다 | F-07 |
| U-16 | `longPressCapsLockTurnsItOff` 등 UI 없는 내부 키 30여 개 | 대응 컨트롤이 없다. 숨은 설정인지 내부 상수인지 알 수 없다 | F-07, `app-bundle-analysis.md` §2.3 |
| U-17 | OCR **전처리 필터 선택 조건** | 필터 이름은 확정, 어떤 조건에서 무엇을 고르는지는 알 수 없다 | F-02, `platform-constraints` P10 |
| U-18 | **전체 화면 OCR 지연** | 여전히 미실측. 클론의 실측 스파이크로만 알 수 있다 | F-02, `platform-constraints` P3 |
| U-19 | 홈로우 프리셋의 **각 키 매핑 전량** | 팝업 라벨이 `A = !` / `A = F1` 두 예시만 보여준다 | F-08 |
| U-20 | 레이아웃 변형(Colemak/Dvorak)의 **자동 적용 여부** | 키는 실재하나 팝업에 없다 — 자동 적용으로 보이나 확인하지 못했다 | F-08, F-14 |
| U-21 | 라이선스 API 사양·활성화 대수·트라이얼 기간·판매가 | ⚠️ **`Purchase` 와 `Remove Oldest Activation` 버튼은 누르지 않았다** (작업 경계) | F-12, `platform-constraints` P7 |

---

## 5. ⭐ 상호작용 확인에서 변경했다가 복원한 SuperKey 설정

관찰 전 baseline 을 `defaults read com.knollsoft.Superkey` 로 기록했다(`app-bundle-analysis.md` §2.1).
⛔ **라이선스 관련 버튼(`Purchase`, `Remove Oldest Activation`)은 한 번도 누르지 않았다.**

| # | 탭 | 변경한 항목 | 변경 전 → 변경 후 → 복원 후 | 복원 | 왜 바꿨나 |
| :--- | :--- | :--- | :--- | :---: | :--- |
| 1 | Seek | `Seek using macOS accessibility` | ☐ → ☑ → **☐** | ✅ | `Match on more than one character` 의 표시 조건과 기본값 확인 |
| 2 | Seek | `Remap key to Seek:` | `-` → `F13` → **`-`** | ✅ | `Only show while the remapped key is held` 의 활성화 조건 확인. ⭐ caps lock 대신 **F13** 을 골라 실사용 키에 영향이 없게 했다 |
| 3 | Hyperkey | `Engage hyper key using trackpad:` | ☐ → ☑ → **☐** | ✅ | `Change menu bar icon when engaged` / `Provide haptic feedback when triggered` 의 표시 조건 확인 |
| 4 | Presets | `Caps lock + W A S D = ▲ ◀︎ ▼ ▶︎` | ☐ → ☑ → **☐** | ✅ | `Apply hyper to arrows` 의 표시 조건 확인 (나타나지 않았다) |

**그 밖의 조작 (설정을 바꾸지 않은 것)**
- 팝업 **6개**를 열어 선택지를 읽고 **Esc 로 닫았다.** 전후 값이 동일함을 각각 확인했다.
- 4개 탭을 순회했고 마지막에 최초 탭(`Seek`)으로 되돌렸다.
- 환경설정 창은 관찰 시작 시 **닫혀 있었고**, 관찰 후 **닫아** 원상태로 두었다.
- 메뉴바 메뉴를 열어 항목을 읽고 **Esc 로 닫았다.** 어떤 항목도 클릭하지 않았다 (`Settings…` 제외).
- ⓘ 팝오버를 열어 내용을 읽고 Esc 로 닫았다.

**최종 검증**: 4개 탭의 전 컨트롤을 다시 판독해 **체크 상태·팝업 값·슬라이더 값이 스크린샷 5장과 전부 일치**함을 확인했다.

### 5.1 ⚠️ 남은 잔여물 — plist 키 4개

UI 값은 baseline 과 동일하지만, plist 에는 **원래 없던 키 4개**가 기본값과 동등한 값으로 기록되었다.

```
capsWasdArrows    = 2      (#4 로 생성)
oneSwipeFromTop   = 2      (#3 으로 생성)
seekOptions       = 1      (#1 로 생성)
seekRemapKeycode  = 0      (#2 로 생성)
```

**왜 지우지 않았나**: 앱이 실행 중이며 이 값들을 메모리에 캐시하고 있다. 디스크만 `defaults delete` 로 지우면 메모리와 디스크가 어긋난 상태를 만든다 — 원상복구가 아니라 **새로운 불일치를 만드는 행위**다. 네 값 모두 UI 관찰상 baseline 과 동일한 상태를 나타내므로(`2` = 꺼짐, `seekRemapKeycode = 0` 은 팝업이 `-` 를 표시하는 미설정 센티널) 기능적으로 동등한 잔여물로 판단해 그대로 두었다.

⭐ **이 잔여물 자체가 하나의 실측 사실이다**: SuperKey 는 **"기본값으로 되돌림" 과 "키 삭제" 를 구분하지 않는다.** 한 번 건드린 항목은 기본값으로 돌아가도 키가 남는다. 클론이 이를 어떻게 다룰지는 `settings-store-and-integrity.md` (F-15) 가 결정한다.

---

## 6. 검증 방법에 대한 회고 — 다음에 반복하지 않을 실수

| 무엇이 빗나갔나 | 왜 빗나갔나 | 교훈 |
| :--- | :--- | :--- |
| "8개 로케일 번들, 3개 RTL" | **간접 증거를 직접 증거로 취급했다.** appcast 의 델타 업데이트 부수 속성(`sparkle:deltaFromSparkleLocales`)을 앱의 로케일 목록으로 읽었다 | 부수 속성은 그 속성이 원래 무엇을 위한 것인지 먼저 확인한다 |
| 기본값을 홍보 스크린샷에서 읽음 | 근거가 스크린샷밖에 없었다. ⭐ 다만 `superkey-inventory.md` §3.4 와 Q3 가 **"출고 기본값이라는 보장은 없다" 고 명시적으로 유보했다** — 유보는 옳았고, 그 유보를 명세 본문이 충분히 전파하지 못한 것이 문제였다 | 유보는 질문 목록이 아니라 **그 값을 쓰는 자리마다** 붙여야 한다 |
| 슬라이더 범위를 눈금 개수로 역산 | 8칸 눈금과 관측값 1000ms 로 등간격 배열을 역산했다. 실제는 250–2000 | 시각적 요소 개수로부터의 역산은 근거가 아니다. 모르면 `(미확정)` |
| 권한 3종을 온보딩에서 함께 요청한다고 가정 | FAQ 의 복구 절차에 세 권한이 등장한 것을 "앱이 셋 다 확인한다" 로 읽었다 | "언급된다" 와 "앱이 확인한다" 는 다르다. 심볼 테이블이 이 구분의 답이었다 |
| ScreenCaptureKit 을 전제로 최소 OS 상향을 제안 | 최신 API 가 정답이라고 가정했다. 원본은 12.0 을 유지하며 레거시 API 로 출하 중이다 | 원본이 무엇을 쓰는지는 "가능한가" 가 아니라 **"충분한가"** 의 답이다 |

⭐ 반대로 **웹 조사가 옳게 짚은 것도 많다** — 트랙패드 영역 후보(4 코너 + 상단 가장자리), 출력 키 열거형과 소스 키 열거형의 분리, paste 팝업의 성격, HJKL 인라인 팝업의 용도, 이벤트 탭 재활성화의 필요성, 충돌이 조용히 무시되면 안 된다는 요구. §1 의 31건이 그 목록이다.

---

## 7. 남은 작업

이 PR 이후:

1. **키 이벤트 탭 + 리매핑 엔진 (M1)** — 이미 다음 위임 대상으로 결정되어 있다. 범위와 완료 판정은 [`../spec/README.md` M1](../spec/README.md) 참조
2. **제품 결정 D2·D3·D4** — 라이선싱 구현 범위 · F-06 범위 포함 여부 · 현지화 채택 여부. D1(최소 OS)은 이번 검증으로 해소되었다
3. §4 의 미확정 21건 중 실행 중 관찰로 풀 수 있는 것(U-01·U-02·U-05·U-07)은 **구현 중 필요한 시점에** 다시 관찰한다 — 지금 무리해서 확인하는 것보다, 그 명세를 구현할 때 확인하는 편이 관찰 목적이 분명하다
