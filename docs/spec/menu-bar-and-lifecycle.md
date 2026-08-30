# F-10 · 메뉴바 상주와 앱 수명주기

> **한 줄 요약**: SuperKey 클론을 Dock 아이콘 없는 메뉴바 상주 에이전트 앱으로 만드는 활성화 정책, `NSStatusItem` 메뉴 구성(⭐ 실측으로 전량 확정 — §3.3), 앱별 비활성화(`Ignore <앱이름>`, ⭐ 신규 — 이 문서가 소유, §3.4), 앱 상태 머신, 단일 인스턴스 보장, 로그인 시 자동 실행(⭐ 헬퍼 앱 + `ServiceManagement` 경로로 확정, §3.5), 절전·깨어남·로그인 시 엔진 재초기화 시점(⭐ event tap 재활성화가 아니라 **프로세스 재실행** 방식임이 확정, §3.2·§3.5), 정상 종료 시 정리, 권한 실패 시의 상태 전이(⭐ "지속 오류 상태"가 아니라 "진단 → 복구 시도 → (실패 시) 치명적 종료"로 정정, §2 시나리오 E)를 정의한다.
> **의존성**: `F-11`(permissions-onboarding)의 온보딩 플로우를 최초 실행 시 호출하고, 앱 상태 머신이 참조하는 권한 상태를 F-11 이 제공한다는 전제로 동작한다. `F-07`(key-remapping-engine)에게는 "지금 재초기화하라"는 신호만 보낸다 — `CGEventTap` 설치·재활성화 메커니즘 자체는 F-07 소관이다. 앱별 비활성화(`Ignore <앱이름>`)가 F-07 의 리매핑 적용 여부에 미치는 영향은 F-07 과 상호 참조한다(§3.4) — 단 이 실측 결과 자체는 이 문서(F-10)가 최초로 F-07 에 알리는 것이며, F-07 문서 자체의 갱신은 이 작업 범위 밖이다.
> **관련 명세**: 권한 요청·복구 절차의 상세는 `F-11`, 환경설정 창 UI 는 `F-09`, 업데이트 확인 로직은 `F-13`, 라이선스 상태 표시의 내용은 `F-12`, `CGEventTap` 자체의 설치·재활성화 메커니즘은 `F-07`, hyper 활성화 중 메뉴바 아이콘이 바뀌는 것은 `F-05`(hyperkey.md)·`F-06`(trackpad-hyper-gesture.md)과 상호 참조한다.
> **근거 문서**: `docs/research/app-bundle-analysis.md`(⭐ 1차 근거, 실측) · `docs/research/superkey-inventory.md`(기존 조사, 실측으로 대체되지 않은 부분만 유효)

---

## 1. 개요

SuperKey 클론은 **메뉴바 상주 앱(agent app)** 이다. Dock 아이콘이 없고 `⌘Tab` 앱 스위처에도 나타나지 않으며, 유일한 상시 접점은 메뉴바의 `NSStatusItem` 아이콘이다. ⭐ 이는 더 이상 추정이 아니다 — 원본 `Info.plist` 가 `LSUIElement = true` 를 선언하는 것이 실측으로 확인됐다(실측: 번들 심볼, app-bundle-analysis.md §1). macOS 에서 이는 `NSApplicationActivationPolicyAccessory` 와 동등한 효과를 내며, Tauri 에서는 `app.set_activation_policy(ActivationPolicy::Accessory)` 로 대응된다(rust-macos-capability-notes.md §2.6, §2.7).

메뉴 자체는 이번 조사에서 **실행 중 앱의 AX 트리를 직접 판독해 전량 확정**됐다(§3.3, 실측: AX 트리). 과거 명세가 appcast v1.62 changelog 문구("Menu item icons have been added to the menu bar menu.")만으로 항목 구성을 통째로 추정했던 것은 더 이상 필요하지 않다 — 다만 그 changelog 가 말하는 "메뉴 항목 아이콘"이 실제로 어느 항목에 붙어 있는지는 AX 트리 판독만으로는 아이콘 유무를 항상 명확히 드러내지 않아 이번 조사에서도 확인되지 않았다 `(미확정)` → §9.

F-10 이 다루는 두 번째 축은 **앱 수명주기**다. 에이전트 앱은 사용자가 명시적으로 실행하는 일이 드물고, 대부분 로그인 시 자동 실행되어 백그라운드에서 계속 떠 있다. 이 특성 때문에 절전에서 깨어남, 로그인, 키보드 재연결 같은 이벤트들이 모두 "리매핑 엔진이 살아 있는가"를 위협하는 실제 실패 지점이 된다. ⭐ 실측으로 확정된 중요한 사실 하나: 원본의 복구 수단은 `CGEventTap` 재설치가 아니라 **앱 프로세스 자체의 재실행(relaunch)**이다. 메뉴바 `Advanced ▸` 하위에 `Relaunch` · `Relaunch After Wake` · `Delay Relaunch After Wake` · `Relaunch on Keyboard Connected` 4개 항목이 그대로 존재하고(실측: AX 트리), 실행 파일에는 `AppRelauncher` 타입과 "Launching Superkey launcher" / "Successfully launched new application instance." 같은 원문 로그 문자열이 있다(실측: 번들 문자열). §3.2·§3.5 에서 상세를 다룬다.

세 번째 축은 권한 실패 상태다. **과거 명세가 채택했던 고정 실패 문구 `"Unable to initialize Superkey"`(웹사이트 FAQ 인용, superkey-inventory.md §1.3)는 이번 번들 문자열 실측에서 정확히 같은 형태로 발견되지 않았다.** 대신 다음이 실측으로 확정됐다(실측: 번들 문자열, app-bundle-analysis.md §4.4):

- 진단 제목류 원문: `Superkey has insufficient privileges` / `Unable to listen to device input`
- 본문 원문: `Superkey is unable to listen to input from your devices.`
- "권한이 macOS 와 어긋난(out of sync) 상태"라는 별도 진단이 있고, 두 갈래 복구 경로(자체 리셋 vs 수동 절차)를 제시한다 — 화면 문구와 절차의 상세는 `F-11` §3 이 갖는다.
- ⭐ **event tap 생성 실패 자체는 치명적이다.** 원문 `com-knollsoft-Superkey Failed to create event tap. Exiting program` 이 보여주듯, 이 실패는 앱을 살려둔 채 안내만 계속하는 지속 상태가 아니라 **프로세스 종료로 귀결될 수 있다.** 즉 "`Unable to initialize Superkey`" 라는 이름의 영속 오류 상태를 전제한 과거 설계는 정정이 필요하다 — F-10 은 이 절을 "진단(out-of-sync) → 복구 시도(자체 리셋/수동) → (실패 시) 치명적 종료" 흐름으로 다시 정의한다(§3.1, §2 시나리오 E). "Unable to initialize Superkey" 문구와 위 실측 문구들의 정확한 관계(같은 대화상자의 다른 버전 문구인지, 별개의 화면인지)는 확인하지 못했다 `(미확정)` → §9.
- 권한이 아예 없는 경우(Accessibility 미부여)는 이와 별개다 — 메뉴 전체가 `unauthorizedMenu` 로 교체된다(§3.3). 항목은 (SuperKey 원문) `Not Authorized to Control Your Computer` / `Authorize` 다. 이 메뉴는 **권한이 이미 부여된 상태에서 조사해 직접 관찰하지는 못했다** — 존재와 항목 이름은 번들 문자열로만 확인했다(실측: 번들 문자열). 실제 겉모습(비활성 라벨인지 클릭 가능한지, 아이콘 유무)은 `(미확정)` → §9.

이 세 갈래(unauthorizedMenu / out-of-sync 진단 / 정상 동작)의 정확한 진입·전이 조건은 F-10 의 상태 머신(§3.1)이 다루고, 복구 절차의 화면 문구·UI 상세는 `F-11` 이 갖는다.

## 2. 사용자 시나리오

### 시나리오 A — 최초 설치 후 첫 실행

1. 사용자가 `.dmg` 에서 앱을 `/Applications` 로 드래그하고 최초 실행한다.
2. 앱은 즉시 Dock 아이콘 없이 메뉴바에만 아이콘을 표시하며, 상태 머신은 **미초기화(Uninitialized)** 로 시작한다.
3. Accessibility 권한(⭐ 앱이 명시적으로 확인하는 유일한 권한 — 상세는 `F-11`)이 아직 없으므로 메뉴는 `unauthorizedMenu` 로 교체되고, 상태는 **권한 없음(Unauthorized)** 으로 전이한다.
4. 사용자가 `Authorize` 를 누르면 F-11 의 안내를 따라 시스템 설정에서 Accessibility 를 부여한다.
5. F-10 은 권한 상태 변화를 감지하고 F-07 에 엔진 기동을 요청한다. `CGEventTapCreate` 가 성공하면 상태는 **정상 동작(Running)** 으로 전이하고, 메뉴는 정상 메뉴(§3.3)로 되돌아온다.

### 시나리오 B — 로그인 시 자동 실행

1. 사용자가 `General` 탭의 `Launch on login` 을 켰다 — ⭐ 이 체크박스의 출고 기본값은 **☐(꺼짐)** 이다(실측: AX 트리, §4). 과거 명세가 "이런 유틸리티는 기본 ON 이 흔하다"고 추정한 것은 틀렸다.
2. 맥을 재시작하고 로그인한다.
3. `Contents/Library/LoginItems/SuperkeyLauncher.app` 헬퍼가 로그인 항목으로 등록되어 있다면 macOS 가 이를 실행하고, 헬퍼가 본체 `Superkey.app` 을 기동한다(§3.5). `SMLoginItemSetEnabled`·`SMAppService` 심볼이 둘 다 링크되어 있어(실측: 번들 심볼) macOS 버전에 따라 두 경로 중 하나가 쓰이는 것으로 보인다 `(미확정 — 분기 조건은 해석)`.
4. ⭐ **로그인 항목 등록 자체가 실패할 수 있다.** 원문 "Unable to set launch on login to %{public}@. Retrying in 0.1s" / "Still unable to set launch on login" 이 확인됐다(실측: 번들 문자열) — 0.1초 간격의 재시도 루프와, 그래도 실패하면 포기하는 메시지가 있다. §5·§8 에 반영한다.
5. 이미 권한이 부여되어 있으므로 `unauthorizedMenu` 를 거치지 않고 곧바로 **정상 동작(Running)** 상태로 진입하고, F-07 에 엔진 기동을 요청한다.

### 시나리오 C — 절전 복귀

1. 앱이 정상 동작 중 맥이 잠들었다가 사용자가 뚜껑을 열어 깨운다.
2. `NSWorkspaceWillSleepNotification` 발화 시 원문 "Received sleep notification" 이, `NSWorkspaceDidWakeNotification` 발화 시 "Received wake notification" 이 로그로 남는다(실측: 번들 문자열·심볼).
3. F-10 은 깨어남 알림을 받아 재초기화 필요 여부를 판단한다. ⭐ 여기에 **재시작 디바운스**가 있다는 것이 실측으로 확정됐다 — 원문 "Time since last exit: … s ago, no need to restart." 는 마지막 종료(절전 진입) 시점이 충분히 최근이면 재시작을 건너뛴다는 것을 보여준다. 반대로 재시작이 필요하다고 판단되면 원문 "Restarting keyboard listener on wake" 로그와 함께 재개된다. 관련 키: `lastSleepDate` · `appLaunchedTime` · `systemUptime` · `restartOnWakeDelay` · `wakeKeyboardDelay`(깨어남 후 지연 재개, §5).
4. §1 에서 확정했듯, 이 "재개"는 **`CGEventTapEnable` 같은 탭 내부 재활성화가 아니라 앱 프로세스 재실행일 가능성이 높다**(메뉴바 `Advanced ▸ Relaunch After Wake`/`Delay Relaunch After Wake` 항목이 그 사용자 노출 스위치로 보인다). 재초기화의 정확한 구현(탭 재활성화 vs 프로세스 재실행)은 F-07 소관이며, F-10 은 "언제 신호를 보내는가"만 정의한다.
5. 사용자는 별도 조작 없이 리매핑이 정상 동작함을 확인한다.

### 시나리오 D — 이중 실행 시도

1. 앱이 이미 메뉴바에 상주 중인데, 사용자가 Finder 에서 앱 아이콘을 실수로 다시 더블클릭한다(또는 로그인 항목과 수동 실행이 겹친다).
2. 두 번째 프로세스는 시작 직후 이미 같은 번들 ID 로 실행 중인 인스턴스가 있음을 감지한다.
3. 두 번째 프로세스는 새로운 `CGEventTap` 을 설치하지 않고 즉시 종료한다 — 그렇지 않으면 키 입력이 두 번 처리된다(§5 항목 1). ⚠️ 이 시나리오의 구체적 감지·처리 메커니즘은 이번 실측에서 별도로 확인되지 않았다 `(추정)` — §9 로 승계.

### 시나리오 E — ⭐ 권한 어긋남(out-of-sync) 진단과 그 이후 (정정)

이전 버전은 이 시나리오를 "`Unable to initialize Superkey` 라는 지속 오류 상태에 머무르며 사용자가 F-11 의 복구 절차를 밟는다"로 서술했다. 실측 문자열은 더 구체적인 그림을 보여준다 — 아래로 대체한다.

1. 사용자가 개발 중 앱을 재서명하거나, 시스템 설정에서 Accessibility 항목을 수동으로 건드리는 등, TCC 데이터베이스가 앱이 실제로 부여받은 권한과 어긋나는 상태가 된다.
2. 앱을 실행하면 `AXIsProcessTrusted()` 는 `true` 를 반환하지만(시스템 설정상 "켜짐"으로 보임) F-07 의 `CGEventTapCreate` 시도가 실패한다.
3. 앱은 이를 out-of-sync 로 판정하고, 원문 `Superkey has insufficient privileges` / `Superkey is unable to listen to input from your devices.` 대화상자를 띄운다. 이 대화상자는 두 갈래를 제시한다 — **(a) `Reset Priviliges & Restart Superkey`**[sic, 원문 오타 그대로] 버튼으로 앱이 스스로 권한을 리셋하고 재시작을 시도하거나, **(b) 수동 절차**(시스템 설정에서 목록 제거 → 재실행 → 재승인)를 따른다. 화면 문구·절차 상세는 `F-11` §3 이 갖는다 — F-10 은 이 대화상자가 뜨는 시점(§3.1 상태 진입 조건)만 정의한다.
4. **(a) 를 선택한 경우**: 원문 "Superkey will now close and attempt to open System Preferences for you." 가 보여주듯 앱이 스스로 종료 후 시스템 설정을 열려 시도한다 — 즉 이 경로는 §3.1 의 "종료 중" 과 사실상 합류한다.
5. **(b) 를 선택하거나, 어느 경로로도 문제가 해소되지 않아 F-07 이 다시 `CGEventTapCreate` 를 시도해 또 실패하는 경우**: 원문 `com-knollsoft-Superkey Failed to create event tap. Exiting program` 로그와 함께 **프로세스가 종료된다.** 이는 지속 오류 상태가 아니라 **치명적 종료**다.
6. 사용자가 F-11 이 정의하는 복구 절차(목록에서 제거 → 재부팅 → 재실행 → 재승인)를 완료하고 앱을 다시 실행하면, 상태 머신은 미초기화부터 다시 시작해 정상 동작으로 도달한다.

### 시나리오 F — 메뉴에서 정상 종료

1. 사용자가 메뉴바 아이콘을 클릭해 메뉴를 열고 **`Quit Superkey`**(실측: AX 트리 — 더 이상 `(추정)` 이 아니다)를 선택한다.
2. 상태는 **종료 중(Quitting)** 으로 전이한다.
3. F-10 은 F-07 에 event tap 해제를 요청하고, 현재 합성 상태로 눌려 있는 modifier 가 있으면 이를 해소(release 이벤트 재생)하도록 요청한다 — 눌린 채로 종료되면 시스템 전체의 modifier 상태가 어긋나 다른 앱에까지 영향을 준다.
4. 열려 있던 Seek 오버레이 창이 있으면 파괴를 요청한다.
5. 정리가 끝나면 프로세스가 종료된다.

메뉴의 `Advanced ▸ Relaunch` 는 이와 다른 동작이다 — 종료가 아니라 §1·§3.2 의 재실행(relaunch) 경로를 사용자가 수동으로 트리거하는 항목으로 보인다(§3.3).

### 시나리오 G — ⭐ 신규: 앱별 비활성화(`Ignore <앱이름>`) 사용

1. 사용자가 다른 앱(예: 터미널 에뮬레이터)에서 SuperKey 의 리매핑과 충돌하는 단축키를 자주 쓴다.
2. 사용자가 그 앱을 최전면으로 전환한 상태에서 SuperKey 메뉴를 연다. 메뉴 두 번째 섹션에 `Ignore <그 앱 이름>` 항목이 동적으로 표시된다(실측: AX 트리, "Ignore Ghostty" 로 관찰됨). 템플릿 문자열은 (SuperKey 원문) "Ignore frontmost.app" 다.
3. 사용자가 클릭하면 그 앱의 번들 ID 가 비활성 목록(`disabledApps` 로 추정)에 추가된다.
4. 이후 그 앱이 최전면일 때는 F-07 의 리매핑 엔진이 이벤트를 가로채지 않고 통과시킨다 — 정확한 통과 범위(리매핑만인지 Seek 트리거까지인지)는 `(미확정)` → §3.4, §9.
5. 사용자가 같은 항목을 다시 클릭하면(또는 별도 UI 로) 목록에서 제거해 리매핑을 재개할 수 있을 것으로 추정되나, 그 UI 가 메뉴 항목 재클릭인지 별도 화면인지는 확인하지 못했다 `(미확정)`.

## 3. 동작 명세

### 3.1 앱 상태 머신

| 상태 | 진입 조건 | 메뉴바 표시 | 가능한 조작 |
| :--- | :--- | :--- | :--- |
| **미초기화(Uninitialized)** | 프로세스 시작 직후, `AXIsProcessTrusted()` 를 아직 확인하기 전 | 기본 아이콘, 활성화 애니메이션 없음 `(추정)` | 없음 — 밀리초 단위로 즉시 다음 상태로 전이 |
| **권한 없음(Unauthorized)** | `AXIsProcessTrusted() == false` (F-11 §3.1) | ⭐ 메뉴 전체가 `unauthorizedMenu` 로 교체(실측: 번들 문자열 — 항목 존재만 확인, 겉모습은 직접 관찰 못함 `(미확정)`) — 항목 `Not Authorized to Control Your Computer` / `Authorize` | `Authorize` 클릭 → F-11 온보딩(시스템 설정 안내) |
| **정상 동작(Running)** | Accessibility 부여됨 + F-07 `CGEventTapCreate` 성공 | 정상 메뉴 전량(§3.3) | 메뉴의 모든 항목 활성 |
| **권한 어긋남 진단(PermissionsOutOfSync)** ⭐(이름·정의 정정) | `AXIsProcessTrusted() == true` 인데 F-07 `CGEventTapCreate` 가 실패 | 대화상자 `Superkey has insufficient privileges` (실측: 번들 문자열). 이 상태에서의 메뉴바 아이콘 형태는 관찰되지 않음 `(미확정)` | 대화상자의 두 갈래(§2 시나리오 E) — (a) 자체 리셋+재시작(사실상 종료중으로 합류) 또는 (b) 수동 절차 안내. 어느 쪽도 해소하지 못하면 **치명적 종료**로 귀결 |
| **종료 중(Quitting)** | `Quit Superkey` 선택(실측: AX 트리), 시스템 로그아웃/재시작 신호, 또는 위 진단 상태의 치명적 종료 경로 | 아이콘이 사라지는 과정(순간적) | 없음 — 정리 완료 후 프로세스 종료 |
| **트라이얼 만료(TrialExpired)** | 트라이얼 기간이 지났고 유효 라이선스가 없음(만료 후 동작은 F-12 소관) | `Purchase` 메뉴 항목이 항상 최상단에 존재하는 것이 실측으로 확인됐다(§3.3) — 이 항목이 트라이얼/라이선스 상태에 따라 조건부인지 항상 노출인지는 `(미확정)` | 메뉴 클릭 → 라이선스 입력/구매 유도(F-12) |

> 상태 간 전이는 원칙적으로 미초기화 → (권한없음 ↔ 정상동작 ↔ 권한어긋남진단) → 종료중 의 흐름을 따른다. 권한어긋남진단은 §2 시나리오 E 가 보여주듯 종료중으로 직접 합류할 수 있는 유일한 상태다(다른 상태는 사용자의 명시적 `Quit` 을 거친다). 트라이얼 만료는 독립적인 축이며, 정확한 우선순위 규칙(권한 문제와 트라이얼 만료가 동시에 발생하면 어느 쪽을 우선 표시하는가)은 조사로 확정되지 않았다 `(추정)` → §9.

### 3.2 수명주기 이벤트 → 대응 동작

| 이벤트 | 구독 대상(실측: 번들 심볼) | 무엇을 하는가 | 근거 |
| :--- | :--- | :--- | :--- |
| 절전 진입 | `NSWorkspaceWillSleepNotification` | 원문 "Received sleep notification" 로깅. `lastSleepDate` 기록 | 실측: 번들 문자열·심볼 |
| 절전에서 깨어남 | `NSWorkspaceDidWakeNotification` | 원문 "Received wake notification". 디바운스 판정 후 필요하면 원문 "Restarting keyboard listener on wake" 와 함께 재개 | 실측: 번들 문자열·심볼 |
| ⭐ 재시작 디바운스 | (내부 판정) | `lastSleepDate`·`appLaunchedTime`·`systemUptime` 을 비교해 "충분히 최근"이면 재시작을 건너뜀 — 원문 "Time since last exit: … s ago, no need to restart." | 실측: 번들 문자열. 정확한 임계값은 `(미확정)` → §9 |
| ⭐ 깨어남 후 지연 재개 | (내부 판정) | `restartOnWakeDelay` · `wakeKeyboardDelay` 키가 존재 — 깨어난 직후 곧바로가 아니라 일정 지연 후 키보드 리스너를 재개하는 것으로 보인다. 메뉴바 `Advanced ▸ Delay Relaunch After Wake` 가 사용자 노출 스위치 | 실측: 번들 문자열(키 이름). 지연 값·조건은 `(미확정)` |
| 세션 활성화(로그인·잠금해제·사용자전환) | `NSWorkspaceSessionDidBecomeActiveNotification` | 심볼 존재 확인 — 정확히 어떤 처리를 트리거하는지는 문자열로 세분화되지 않았다 | 실측: 번들 심볼. 처리 세부는 `(미확정)` |
| 최전면 앱 전환 | `NSWorkspaceDidActivateApplicationNotification` | 메뉴의 `Ignore <앱이름>` 라벨 갱신 + F-07 에 "이 앱은 비활성 목록에 있는가" 판정 근거 제공(§3.4) | 실측: 번들 심볼(§3.4) |
| 키보드 연결 | (HID 디바이스 매칭 콜백, IOKit) | 메뉴바 `Advanced ▸ Relaunch on Keyboard Connected` 가 사용자 노출 스위치 — 키보드가 새로 연결되면 재실행(또는 재초기화)을 트리거하는 것으로 보인다 | 실측: AX 트리(메뉴 항목 존재). 트리거 조건·재초기화 방식은 `(미확정)` |

⭐ 이 표가 "언제 무엇을 하는가"를 정의하되, 중요한 정정이 있다 — 원본의 재개 메커니즘은 F-07 이 `CGEventTapEnable` 로 기존 탭을 되살리는 것이 아니라, `AppRelauncher`(§3.5)를 통한 **프로세스 자체의 재실행**일 가능성이 실측으로 뒷받침된다(메뉴의 `Relanch`/`Relaunch After Wake`/`Delay Relaunch After Wake`/`Relaunch on Keyboard Connected` 4개 항목이 전부 "재실행" 어휘를 쓴다). `CGEventTap` 이 실제로 `kCGEventTapDisabledByTimeout`/`kCGEventTapDisabledByUserInput` 상태인지 판별하고 어느 전략(탭 내부 재활성화 vs 프로세스 재실행)을 쓸지는 F-07 소관이다(rust-macos-capability-notes.md §2.1) — 이 실측 사실은 F-07 의 설계 재검토를 위한 입력으로 §9 에도 남긴다.

### 3.3 ⭐ 메뉴 항목 구성 — 실측 전량 확정

실측(AX 트리 — 실제로 메뉴를 열어 판독)으로 확정된 메뉴 전체 구조:

```
Purchase
────────
Ignore <최전면 앱 이름>        ← 관찰 당시 "Ignore Ghostty"
────────
Settings…
Check for Updates…
About
Advanced ▸
    Show Logging…
    Launch Logger on Start
    Log to File
    Log Debug Messages
    Show Viewer…
    Launch Viewer on Start
    Synthesize Caps Lock Remap
    Relaunch After Wake
    Delay Relaunch After Wake
    Relaunch on Keyboard Connected
    Relaunch
Quit Superkey
```

권한이 없을 때는 이 전체가 `unauthorizedMenu`(§3.1)로 교체된다.

**항목별 소유 명세와 클론 재현 판단:**

| 항목 (SuperKey 원문) | 소유 명세 | 클론 재현 판단과 근거 |
| :--- | :--- | :--- |
| `Purchase` | F-12(licensing-and-trial) | **재현.** 항상 최상단에 있는 것으로 관찰됨. 조건부 노출 여부는 F-12 소관 |
| `Ignore <앱이름>` | F-10(이 문서) | **재현.** §3.4 |
| `Settings…` | F-09(preferences-ui) | **재현.** 환경설정 창을 여는 표준 진입점 |
| `Check for Updates…` | F-13(auto-update) | **재현.** 업데이트 확인 트리거 |
| `About` | F-10 | **재현(단순화 가능).** `About.storyboardc` 존재로 About 창이 있음을 확인(실측: 번들 구조). 클론은 최소한의 버전·저작권 정보 창으로 재현 |
| `Advanced ▸ Show Logging…` / `Launch Logger on Start` / `Log to File` / `Log Debug Messages` / `Show Viewer…` / `Launch Viewer on Start` | F-10 | **기각.** 별도 SPM 로깅 번들(`Logging_Logging.bundle`)에 딸린 개발자·지원용 로그 뷰어·토글로 판단된다. 최종 사용자 경험에 필요하지 않고, 클론은 표준 OS 로그(또는 파일 로그)로 충분하다고 판단한다. 구조화 로그 자체(파일 기록)는 유지할 수 있으나, 사용자 노출 뷰어 UI 는 재현하지 않는다 |
| `Synthesize Caps Lock Remap` | F-07/F-08 관련 | **기각(1차 릴리스 범위 밖).** caps lock 리매핑을 HID 매핑(`hidutil`) 대신 이벤트 합성으로 대체하는 스위치로 추정된다(app-bundle-analysis.md §3.2 항목 3, `(미확정 — 라벨로부터의 해석)`). 디버깅/호환성 대응용 스위치로 보여 일반 사용자에게 노출할 필요가 낮다고 판단, Advanced 서브메뉴와 함께 기각. ⭐ **2026-08-30 정정(M2 2차 / 이슈 #15) — F-08 이 재검토해 "재현"으로 뒤집었다.** `../dev/architecture.md` §6.1 결정 D-1 이 caps lock 래칭 문제(`key-remapping-engine.md` §5 #20)를 풀기 위해 **커널 HID 매핑을 제품 기본 경로로 올렸기 때문**이다. 사용자 시스템 전역에 남는 변경에는 반드시 끄는 수단이 함께 있어야 한다 — 이 항목이 그 수단이다(켜면 경로 B 를 설치하지 않고 경로 A 만 쓴다). ⚠️ 라벨의 의미는 원본에서 관찰한 것이 아니라 이 클론이 정한 것이다 `(미확정 — 원본의 실제 동작은 여전히 관찰하지 못했다)` |
| `Relaunch After Wake` / `Delay Relaunch After Wake` / `Relaunch on Keyboard Connected` / `Relaunch` | F-10 | **재현 여부 검토 필요.** 절전 복귀 실패에 대한 사용자 셀프서비스 복구 수단으로 실질적 가치가 있다. 다만 "재실행"이라는 로우레벨 개념을 그대로 메뉴에 노출하는 것이 일반 사용자 UX 에 맞는지는 설계 판단이 필요하다 — 최소한 수동 `Relaunch` 하나는 "리매핑이 안 될 때" 자가 진단 수단으로 유지할 가치가 있다고 판단한다 → §9 |
| `Quit Superkey` | F-10 | **재현.** 표준 종료(§2 시나리오 F) |

v1.62 changelog 의 "메뉴 항목 아이콘"이 위 표의 어느 항목에 실제로 붙어 있는지는 AX 트리만으로 확인되지 않았다 `(미확정)` → §9.

### 3.4 ⭐ 신규 — 앱별 비활성화(`Ignore <앱이름>`)

이 기능은 이 문서(F-10)가 소유한다. 메뉴바에서 실측 확인됐다(§3.3): 현재 최전면 앱 이름이 들어간 `Ignore <앱이름>` 항목이 있다(관찰 당시 "Ignore Ghostty"). 템플릿 문자열(SuperKey 원문): "Ignore frontmost.app".

**확정된 것:**

- 최전면 앱을 `NSWorkspaceDidActivateApplicationNotification` 로 추적하고, `runningApplications` / `processIdentifier` / `bundleIdentifier` 로 신원(번들 ID)을 얻는다(실측: 번들 심볼).
- 관련 키(실측: 번들 문자열): `disabledApps` · `enabledApps` · `disabledForApp` · `userDisabled` · `userEnabled` · `frontAppId` · `frontAppName` · `frontmostApplication` · `frontmostAppToggle` · `ApplicationToggle` · `ignoreMenuItem` · `loggableBundleId` · `loginWindowBundleID`.
- 개념적으로: 최전면 앱의 번들 ID 가 비활성 목록에 들면, F-07 의 리매핑 엔진이 그 앱에 대해서는 리매핑을 통과시킨다(패스스루). **이 판정 로직 자체가 실제로 F-07 어디에 붙는지는 이 조사 범위 밖이다 — F-07(key-remapping-engine.md)과 상호 참조가 필요하며, F-07 문서 자체의 갱신은 이번 작업 범위에 포함되지 않는다.**

**미확정으로 남기는 것(추측으로 메우지 않음):**

- `disabledApps` 와 `enabledApps` 가 **둘 다 존재**한다 → 블랙리스트 모드(기본 전체 활성, 특정 앱만 끔)와 화이트리스트 모드(기본 전체 비활성, 특정 앱만 켬) 두 가지가 있을 가능성 `(미확정)`.
- ⭐ 별도로 `typeToSeekEnabledAppIDs` / `typeToSeekDisabledAppIDs` 키가 있다 — Seek 전용의 또 다른 앱별 목록으로 보이나, **대응하는 UI 를 찾지 못했다** `(미확정)` → §9.
- 이 기능(`Ignore <앱이름>`)이 Seek 트리거까지 끄는지, 키 리매핑만 끄는지는 `(미확정)`.
- `loginWindowBundleID` 의 존재는 로그인 창(`loginwindow.app`)에서의 특수 처리(예: 로그인 화면에서는 리매핑을 적용하지 않도록 항상 비활성 목록에 포함)를 시사하나 확정하지 못했다 `(미확정)`.
- 목록에서 제거(재활성화)하는 UI 가 무엇인지 `(미확정)`(§2 시나리오 G 항목 5).

### 3.5 ⭐ 로그인 실행 방식 — 확정

- **`LSUIElement = true`**(실측: Info.plist) → Dock 아이콘 없는 상주 앱. `(추정)` 이었던 것을 승격한다.
- **LaunchAgent plist 는 없다** — `/Library/LaunchAgents` · `~/Library/LaunchAgents` 양쪽 모두 SuperKey 항목이 확인되지 않았다(실측: 번들 구조 조사).
- 대신 번들에 **`Contents/Library/LoginItems/SuperkeyLauncher.app`** 헬퍼가 있고, `ServiceManagement.framework` 가 링크되어 있으며, **`SMLoginItemSetEnabled` 와 `SMAppService` 심볼이 둘 다** 있다(실측: 번들 심볼) → macOS 버전에 따라 두 경로를 분기하는 것으로 보인다. `LSMinimumSystemVersion = 12.0` 이고 `SMAppService` 는 13+ 전용이므로, macOS 12 에서는 `SMLoginItemSetEnabled`(레거시), 13+ 에서는 `SMAppService`(신규) 를 쓰는 분기로 추정한다 `(미확정 — 분기 조건은 해석)`.
- ⭐ **로그인 항목 설정이 실패할 수 있고, 0.1초 간격 재시도 루프가 있다** — 원문 "Unable to set launch on login to %{public}@. Retrying in 0.1s" / "Still unable to set launch on login"(실측: 번들 문자열). 기존 명세에 없던 실패 모드다 → §5·§8.
- 헬퍼 실행 관련 원문: "Launching Superkey launcher" · "Successfully launched Superkey launcher" · "Received terminate message from helper" · "Successfully launched new application instance." · `AppRelauncher`(내부 타입명, 실측: 번들 문자열).
- **헬퍼와의 통신**: `terminateOnHelperPong` · `receivePong` · `delayRelaunchItem` 키·메서드명이 존재 → **핑퐁(ping-pong) 프로토콜로 헬퍼 생존을 확인**하는 것으로 보인다(실측: 번들 문자열) `(미확정 — 정확한 프로토콜 상세)` → §9.

기존 §6·§7 의 "macOS 13+ 는 `SMAppService`, macOS 12 는 LaunchAgent 폴백" 이중 경로 판정은 이 실측으로 방향이 뒷받침된다 — 다만 원본은 "LaunchAgent plist" 가 아니라 "로그인 항목 헬퍼 앱(`SuperkeyLauncher.app`) + `SMLoginItemSetEnabled`" 조합을 macOS 12 경로로 쓰는 것으로 보여, 순수 LaunchAgent plist 폴백과는 구현 형태가 다르다(§6·§7 갱신).

### 3.5-a ⭐ 클론의 로그인 항목 — 실측과 정정 (2026-08-30 신설, 이슈 #39)

사용자 보고: *"로그인 시에 자동으로 시작하기 기능은 지금 현재 구현이 안 되어 있는 것 같아"*.

PR #17 이 이 기능을 구현했다고 보고하면서 **"등록·해제까지만 확인했고 로그아웃→로그인 왕복은 수행하지 않았다"** 고 명시했다. 그래서 이번 회차는 **가설을 채택하지 않고 실측부터 했다.**

#### 실측 방법

앱에 진단 프로브를 넣었다(`ULTRAKEY_LOGIN_ITEM_PROBE`, `apps/ultrakey-app/src/main.rs`). `register → status → unregister → status` 를 돌면서 매 단계의 **OS 정본**(`SMAppService.status`)을 로그로 남긴다. 판정은 앱이 스스로 기억하는 값이 아니라 OS 가 돌려주는 값으로만 했고, BTM(Background Task Management) 데이터베이스를 `sfltool dumpbtm` 으로 함께 대조했다.

#### ✅ 실측으로 확정된 것 — `SMAppService` 계층은 정상이다

| 확인한 것 | 결과 |
| :--- | :--- |
| 등록 | ✅ `register` 성공 → `status = Enabled(1)` |
| BTM 기록 | ✅ `Type: app (0x2)` · `Disposition: [enabled, allowed, notified] (0xb)` · `URL` 은 실행 중인 `.app` 의 절대 경로 |
| 해제 | ✅ `unregister` 성공 → `status = NotRegistered(0)`, BTM 레코드는 `Disposition: [disabled, ...]` 묘비로 남는다(macOS 의 정상 동작) |
| 해제 후 **재등록** | ✅ 다시 `Enabled` — 앱 자신의 해제는 `RequiresApproval` 을 유발하지 않는다 |
| ⭐ **번들을 재빌드한 뒤**(cdhash 변경) | ✅ `status` 가 여전히 `Enabled` — **"재빌드가 등록을 깬다"는 가설은 기각된다** |
| macOS 12 폴백 오발동 | ✅ 없음. `~/Library/LaunchAgents` 에 plist 가 생기지 않는다 |

⭐ **부수 발견**: 등록 레코드가 아예 없을 때 `status` 는 `NotRegistered(0)` 가 아니라 **`NotFound(3)`** 를 돌려준다. 두 값은 사용자가 해야 할 일이 다른데, 기존 코드는 `status == 1` 이 아니면 전부 "꺼짐"으로 접어 이 구분을 잃고 있었다.

#### ⭐ 그래서 무엇이 문제였나 — 코드에서 확정된 결함 셋

`SMAppService` 호출 자체는 멀쩡했다. 문제는 **그 결과를 다루는 방식**에 있었다.

| # | 결함 | 사용자에게 어떻게 보이는가 |
| :--- | :--- | :--- |
| **①** | ⭐ **UI 가 OS 정본이 아니라 "거울"을 읽는다.** `general_view()` 는 저장소의 `general.launchOnLogin`(사용자가 마지막에 고른 값)만 읽고 `login_item::status()` 를 보지 않는다 | 사용자가 시스템 설정에서 항목을 끄거나, 등록해 둔 `.app` 이 사라지거나, 상태가 `RequiresApproval` 이 되어도 **체크박스는 계속 ☑ 로 보인다.** "켜 놨는데 안 된다"의 가장 직접적인 설명이다 |
| **②** | **등록에 실패해도 거울에는 사용자 의도가 먼저 쓰인다.** `store.set(...)` 이 `result` 판정보다 앞에 있다 | 실패한 상태가 `true` 로 굳고, ① 과 합쳐져 **영구적으로 거짓을 표시**한다 |
| **③** | **기동 시 재조정이 없다.** 거울과 OS 가 어긋나도 아무도 맞추지 않는다 | ⭐ 이 프로젝트에서 실제로 일어나는 시나리오: 앱을 **워크트리의 `target/` 빌드 디렉터리에서 실행**하므로 등록된 절대 경로가 그 디렉터리를 가리킨다(실측: BTM `URL` 이 정확히 그랬다). 워크트리가 정리되면 그 경로는 사라지고 **로그인 시 아무것도 뜨지 않는다.** 나중에 다른 위치의 빌드를 실행해도 아무도 다시 등록해 주지 않는다 |

#### 결정 — 정본은 OS 이고, 앱은 그것을 읽고 맞춘다

1. **`general_view` 는 `login_item::status()` 를 읽는다.** 저장된 값은 판정이 불가능할 때(`Unsupported`)의 폴백으로만 쓴다. **정본은 언제나 OS 다.**
2. **기동 시 재조정한다.** 거울이 `true` 인데 OS 가 `NotRegistered`/`NotFound` 면 **다시 등록**하고 영어 로그를 남긴다. ⭐ 이것이 결함 ③ 을 실제로 고친다 — 앱이 새 위치에서 실행되면 그 위치로 스스로 다시 등록한다.
3. **등록/해제 뒤에 `status()` 를 다시 읽어 검증한다.** API 가 `Ok` 를 돌려줬다는 것과 로그인 시 실제로 뜬다는 것은 다르다. `Enabled` 가 아니면 실패로 취급한다.
4. ⭐ **`RequiresApproval` 을 별도로 다룬다.** macOS 13+ 는 사용자가 `시스템 설정 ▸ 일반 ▸ 로그인 항목` 에서 항목을 끄면 이 상태가 되고, **앱이 `register` 를 다시 불러도 이 상태 그대로다 — 사용자만 되돌릴 수 있다.** 그러므로 재등록으로 해결하려 하지 말고 **사용자에게 무엇을 해야 하는지 알린다.**
   ⚠️ 이 상태는 **이번 회차에서 실측하지 못했다** — 재현하려면 시스템 설정을 바꿔야 하는데 그것은 이 작업의 경계 밖이다. `SMAppServiceStatus` 헤더가 정의하는 값이라는 것과, 우리 코드가 그 값을 받으면 어떻게 행동하는지까지가 확정된 범위다 `(동작은 실측, 이 상태로의 진입 조건은 문서 근거)`.
5. **거울은 OS 조작의 결과를 확인한 뒤에 쓴다.**

⛔ **기각한 대안 — 거울을 없애고 매번 OS 만 읽는다**: `status()` 는 `SMAppService` 객체 생성 + Objective-C 메시지 전송이라 UI 렌더마다 부르기엔 무겁지 않지만, `Unsupported`(판정 불가) 상황에서 보여줄 값이 없어진다. 정본을 OS 로 하되 폴백을 남기는 편이 낫다.

#### ⬜ 여전히 확인하지 못한 것

⭐ **실제 로그아웃 → 로그인 왕복은 이번에도 수행하지 않았다.** 사용자의 세션을 끊는 조작이라 위임 경계 밖이다. **따라서 "로그인 후 수동 개입 없이 리매핑이 동작한다"는 여전히 미확인이다** — 위 결함 셋은 코드와 실측으로 확정했고 고쳤지만, 그것이 왕복 시나리오를 통과시킨다는 것까지 증명하지는 못했다. `docs/dev/manual-verification.md` 7-d 와 1-b #6 에 절차를 남겼다.

### 3.6 단일 인스턴스·기타 관찰

다음은 존재만 확인됐고, 클론 범위 판단을 함께 적는다.

| 항목 | 관찰 | 클론 범위 판단 |
| :--- | :--- | :--- |
| `isRunningOnDevice` | 내부 키 존재(실측: 번들 문자열) | 라이선스 활성화가 기기에 결속되는지와 관련된 것으로 보임 — F-12 소관, F-10 은 존재만 기록 |
| `AppRelauncher` | 재실행 담당 내부 타입(§3.5) | F-10 소관, §3.2·§3.5 에 반영 |
| `relaunchOpensMenu` | 재실행 후 메뉴를 자동으로 여는지를 암시하는 키 이름 | 재현 여부는 UX 판단 필요, 확정 안 됨 `(미확정)` → §9 |
| `Notification Center` | 내부 참조 문자열 존재 | 시스템 알림(예: 업데이트 완료, 권한 문제)을 macOS 알림 센터로 보내는 경로로 추정 — F-13/F-11 과 겹칠 수 있음, F-10 은 존재만 기록 |
| `Visit superkey.app` / `About Superkey` / `AboutWindowController` | About 관련 문자열·컨트롤러 | §3.3 `About` 항목과 연결. 클론은 최소 정보 창으로 재현 |
| 로깅 하위 시스템(`Logging_Logging.bundle`, `LogWindowController`, `WindowDestination`, `ConsoleDestination`, `FileDestination`, `useMacConsole`, `useNSLog`, `rotateFile`) | 별도 SPM 로깅 번들 | §3.3 에서 이미 사용자 노출 로깅 UI 는 기각했다. 내부 구조화 로그(파일 로테이션 등)는 개발/지원 목적으로 클론에도 있을 수 있으나 사용자에게 노출하지 않는다는 판단은 유지 |

## 4. 설정 항목 — `General` 탭 실측 (전면 교체)

기존 §4 는 전부 `(추정)` 이었고 실제와 달랐다. AX 트리 실측 결과, 배치 순서대로:

| 라벨 원문 | 컨트롤 | 기본값 |
| :--- | :--- | :--- |
| `Launch on login` | 체크박스 | ☐ |
| `v1.66 (66)` | ⭐ **버튼**(정적 라벨이 아니다). 첫 행 우측 | — |
| `Check for updates automatically` | 체크박스 | ☐ (`SUEnableAutomaticChecks = false`) |
| `Hide menu bar icon` | 체크박스 | ☐ |
| 부제 `When hidden, relaunch from Finder to open.` | 정적 텍스트 | — |
| `Menu bar icon` | 라벨 + 팝업(선택지 2종, 둘 다 **라벨 없는 이미지 항목**) | — |
| `Remove Oldest Activation` | 버튼 | — |
| `Purchase` | 버튼(강조색) | — |

⭐ **기본값이 ☐ 인 것이 중요하다**: `Launch on login` 이 기본 꺼짐이다. 과거 명세가 "이런 유틸리티는 기본 ON 이 흔하다"고 추정한 것은 **틀렸다**.

⭐ **기존 명세가 추정했으나 실재하지 않는 것 — "확인 결과 부재"로 명시 기록한다(조용히 지우지 않는다)**:

- `Show icon in menu bar` — 실제는 반대 극성의 `Hide menu bar icon` 이다. 항목 자체는 있으나 이름·극성이 달랐다.
- `Reset preferences` — `General` 탭에 없다.
- 권한 상태 표시(Accessibility/Screen Recording/Input Monitoring 배지 등) — `General` 탭에 없다. (F-11 §4 와 동일한 결론.)
- 언어 선택 — `General` 탭에 없다. 앱 자체가 현지화 자원을 갖지 않는다(app-bundle-analysis.md §5.1).

**조건부 항목**: `Relaunch on wake`(내부 이름 `wakeRelaunchCheckbox` / `wakeRelaunchStackView`) — nib 리소스에는 존재하나 기본 상태의 AX 트리에는 나타나지 않았다. 표시 조건은 `(미확정)` → §9.

**아이콘 관련 내부 이름**(실측: 번들 문자열): `StatusTemplate` · `HyperStatusTemplate` · `StatusIcon` · `superkeyLogoTemplate` · `menuBarIconSelect` · `selectMenuBarIcon:`. ⭐ `Hyperkey` 탭의 조건부 항목 `Change menu bar icon when engaged` 와 이름이 연결된다 — **hyper 가 활성 중일 때 메뉴바 아이콘이 바뀐다.** `F-05`(hyperkey.md)·`F-06`(trackpad-hyper-gesture.md)과 상호 참조.

※ `Menu bar icon` 팝업 2종의 정체는 `(미확정)` — 항목이 라벨 없는 이미지이고, 에셋(`Assets.car`)은 저작권 경계상 추출하지 않았다 → §9.

> 이 절의 항목들은 `F-09`(환경설정 창 UI)가 실제 렌더링을 소유한다. F-10 은 이 값들이 앱 수명주기(자동 실행, 메뉴바 표시)에 미치는 영향만 다룬다.

## 5. 엣지 케이스와 실패 모드

1. **이중 실행.** 두 프로세스가 동시에 뜨면 각자 `CGEventTap` 을 설치하려 시도해 키 이벤트가 두 번 처리된다. §3.2 의 단일 인스턴스 보장으로 두 번째 프로세스가 즉시 종료돼야 한다(§6, §7). 종료 대신 기존 인스턴스를 활성화(메뉴 열기)하는 UX 가 더 나을 수 있으나 조사로 확정되지 않았다 `(추정)` → §9.
2. **메뉴바 공간 부족(노치 Mac, 아이콘 과다).** macOS 가 공간이 부족하면 상태 아이템을 자동으로 숨긴다. `Hide menu bar icon` 을 켜지 않은 상태에서도 이런 일이 벌어질 수 있어, 최소한 숨겨졌을 때의 복구 경로(원본은 부제로 "Finder 에서 재실행"을 안내한다 — `When hidden, relaunch from Finder to open.`)가 필요하다는 점을 원본 스스로도 인지하고 있다(§4).
3. ⭐ **로그인 항목 등록 실패와 재시도 루프.** `SMLoginItemSetEnabled`/`SMAppService` 등록이 실패하면 원본은 0.1초 간격으로 재시도하고, 그래도 안 되면 "Still unable to set launch on login" 로 포기한다(§3.5, 실측: 번들 문자열). 클론도 무한 재시도 대신 상한을 두고 실패를 사용자에게 알리는 처리가 필요하다.
4. **macOS 12 에서 `SMAppService` 부재.** `SMAppService` 는 macOS 13 이상에서만 존재한다. 최소 지원 버전이 12.0(실측: Info.plist `LSMinimumSystemVersion`)이므로, macOS 12 런타임에서 `SMAppService` API 를 호출하면 런타임에 실패한다. OS 버전 분기가 반드시 필요하다(§3.5, §6, §7).
5. **종료 시 modifier stuck.** 사용자가 hyper/meh/bleh 키를 누른 채로 강제 종료(force quit)하거나 시스템이 크래시하면, `CGEventTap` 이 사라진 뒤에도 OS 레벨에서 modifier 가 눌린 것으로 남을 수 있다. 정상 종료 경로(§3.1 종료중 상태)에서는 modifier 해소를 시도할 수 있지만, 강제 종료·크래시·§2 시나리오 E 의 치명적 종료 경로에서는 F-10 이 개입할 기회가 제한적이다 — 다음 실행 시점에 잔여 상태를 점검·초기화하는 것이 유일한 보완책이다 `(추정)`.
6. **절전 복귀 후 탭 사망.** §3.2 의 디바운스·지연 로직이 실측으로 확정됐으나, 정확한 임계값·지연 시간은 `(미확정)` → §9. 클론은 최소한 "최근에 재시작했으면 건너뛴다"는 디바운스 개념 자체는 채택할 가치가 있다(불필요한 재초기화로 인한 깜빡임·지연을 줄임).
7. **사용자 전환(fast user switching).** 다른 사용자 세션으로 전환되면 원래 사용자의 앱은 백그라운드로 밀려나지만 프로세스는 계속 존재한다. `CGEventTap` 이 전환된 세션의 키 입력까지 받아버리면 안 된다 — 세션 경계를 넘는 이벤트 탭의 동작은 조사로 확정되지 않았다 `(추정)` → §9.
8. **권한이 런타임에 취소됨.** 앱이 정상 동작 중에 사용자가 시스템 설정에서 Accessibility 권한을 끄면, 진행 중이던 `CGEventTap` 은 즉시 무효화될 수 있다. 이 경우 앱은 정상 동작 상태에서 곧바로 권한 없음 상태로 전이해야 하며, 감지 방법은 F-11 과 조율이 필요하다.
9. **앱이 `/Applications` 밖에서 실행됨(TCC 경로 변경).** 앱을 다른 경로에서 직접 실행하면 TCC 는 그 경로의 바이너리를 별개의 신원으로 취급할 수 있다. 이전에 부여된 권한이 적용되지 않아 권한 없음 또는 권한 어긋남 진단 상태로 직행할 수 있다.
10. **업데이트 후 재시작.** Sparkle 이 업데이트를 설치한 뒤 앱을 재시작시키면, 이는 사실상 §2 시나리오 F(정상 종료)와 시나리오 A/B(재기동)가 연쇄로 일어나는 것이다. 재시작 후 F-10 의 상태 머신이 미초기화부터 다시 시작한다는 점은 F-10 의 책임이다.
11. **크래시 후 재기동.** 프로세스가 예기치 않게 종료된 경우, 자동 재기동을 걸지 조용히 죽게 둘지는 조사로 확정되지 않았다 `(추정)` → §9. `AppRelauncher`/헬퍼 핑퐁 프로토콜(§3.5)이 이와 관련될 가능성이 있으나 상세는 `(미확정)`.
12. **트라이얼 만료 시점에 앱이 실행 중.** 재시작 없이도 상태 머신이 정상 동작에서 트라이얼 만료로 전이해야 하는지, 폴링으로 할지 다음 실행 시점에만 확인할지는 조사로 확정되지 않았다 `(추정)` → §9(만료 이후 정확한 기능 제한 범위는 F-12 소관).
13. ⭐ **헬퍼 핑퐁 실패.** `SuperkeyLauncher.app` 헬퍼가 "pong" 응답을 보내지 않으면(`receivePong` 대기 초과) 앱이 어떻게 판단하는지는 `(미확정)`. 헬퍼가 죽었는데 로그인 항목 등록은 성공한 것으로 남아있는 불일치 가능성이 있다 → §9.

## 6. 필요한 플랫폼 API

- **`NSApplicationActivationPolicyAccessory` / `LSUIElement`**(⭐ 실측 확정) — Dock 아이콘 제거, 앱 스위처 미노출. Tauri 대응: `app.set_activation_policy(ActivationPolicy::Accessory)`(rust-macos-capability-notes.md §2.6, §2.7).
- **`NSStatusItem`** — 메뉴바 아이콘·메뉴. `tray-icon` 0.24.2 가 macOS 에서 `ns_status_item() -> Option<Retained<NSStatusItem>>` 로 직접 접근을 제공한다(rust-macos-capability-notes.md §2.7). 메뉴 항목별 아이콘이 `tray-icon` 고수준 API 로 되는지는 미확인이며, 필요 시 `objc2-app-kit` 0.3.2 로 `NSMenu`/`NSMenuItem` 을 직접 조작해야 할 수 있다.
- **`SMAppService.mainApp`(macOS 13+)** — 로그인 시 실행 등록. `smappservice-rs` 0.1.3 또는 `objc2-service-management` 0.3.2(rust-macos-capability-notes.md §1.2, §2.9). ⭐ 원본이 `SMLoginItemSetEnabled` 와 `SMAppService` 를 **둘 다** 링크한다는 사실이 이 이중 경로 판정을 실측으로 뒷받침한다(§3.5).
- **`SMLoginItemSetEnabled`(macOS 12 폴백)** — 원본이 실제로 쓰는 레거시 API(§3.5). ⭐ 정정: 기존 §6 이 제안했던 "LaunchAgent plist 폴백"은 원본의 실제 구현(헬퍼 앱 + `SMLoginItemSetEnabled`)과 형태가 다르다 — `auto-launch` 0.6.0 이 이 API 를 감싸는지, 직접 `extern "C"` 선언이 필요한지 확인 필요 → §9.
- **`NSWorkspace` 알림** — `willSleepNotification`(절전 진입) · `didWakeNotification`(절전 복귀) · `sessionDidBecomeActiveNotification`(로그인·잠금해제·사용자전환 복귀) · `didActivateApplicationNotification`(최전면 앱 추적, §3.4 신규 요구사항). `objc2-app-kit` 0.3.2 를 통해 구독.
- **`NSWorkspace.shared.runningApplications`** — 단일 인스턴스 판정(§3.6)과 §3.4 앱별 비활성화의 최전면 앱 신원(`bundleIdentifier`) 조회 양쪽에 쓰인다.
- **`NSApplication.didChangeScreenParametersNotification`** — 디스플레이 구성 변경 감지(기존 §6 내용 유지).
- **종료 시 정리 신호** — event tap 해제·modifier 해소는 F-07 이 노출하는 인터페이스를 F-10 이 호출하는 것으로, 이 문서 범위에서는 호출 시점만 정의한다.
- **`AXIsProcessTrusted()` 등 TCC 확인** — 권한 상태 조회는 F-11 이 제공하는 것을 F-10 이 소비한다. F-10 자체가 TCC API 를 직접 호출하지는 않는다.

## 7. 구현 접근

**판정: Rust 바인딩.** (기존 판정 유지 — 실측으로 방향이 뒷받침됐다)

F-10 이 다루는 기능(활성화 정책 설정, `NSStatusItem` 생성과 메뉴 구성, 로그인 항목 등록, `NSWorkspace` 알림 구독, 실행 중인 앱 목록 조회를 통한 단일 인스턴스·앱별 비활성화 판정, 상태 머신 자체의 로직)은 rust-macos-capability-notes.md 가 실사한 크레이트로 대부분 커버된다.

- `app.set_activation_policy(ActivationPolicy::Accessory)` — Tauri v2 API 자체가 제공(rust-macos-capability-notes.md §2.6).
- `tray-icon` 0.24.2 — `NSStatusItem` 생성·기본 메뉴 구성, `ns_status_item()` 으로 필요 시 AppKit 직접 접근(rust-macos-capability-notes.md §1.2, §2.7).
- `smappservice-rs` 0.1.3(macOS 13+) / macOS 12 레거시 경로 — 이중 경로가 필요하다는 판정은 실측으로 확정됐으나(§3.5, §6), macOS 12 쪽이 원본처럼 `SMLoginItemSetEnabled` 기반 헬퍼 앱 방식인지, 순수 LaunchAgent plist 로 대체할지는 클론의 설계 선택이다 — 원본을 그대로 따를 필요는 없다. `auto-launch` 0.6.0 은 LaunchAgent/AppleScript 경로를 제공하므로, 헬퍼 앱 없이 더 단순하게 구현하는 것도 대안이다.
- `objc2-app-kit` 0.3.2 — `NSWorkspace` 알림 구독(§3.2·§3.4 신규 알림 포함), `NSMenuItem` 아이콘 직접 설정, `runningApplications` 조회.
- ⭐ **원본의 0.1초 재시도 루프·헬퍼 핑퐁 프로토콜(§3.5)은 위 크레이트들이 기본 제공하지 않는다** — 이 부분은 애플리케이션 로직으로 직접 구현해야 하며, 원본을 그대로 재현할지 더 단순한 재시도 정책(예: 지수 백오프, 상한 있는 재시도)으로 대체할지는 설계 판단이다. 어느 쪽이든 네이티브 shim 은 필요 없다.

네이티브 Swift/Objective-C shim 을 별도로 작성할 필요가 **없다** — 상태 머신 로직은 순수 애플리케이션 로직이고, 플랫폼 접점은 모두 위 크레이트들이 노출하는 API 표면 안에 있다.

- **기각한 대안 1 — "순수 Rust" 판정.** 상태 머신 자체는 플랫폼 비의존적으로 보이지만, F-10 의 모든 진입 조건(권한 상태, 절전/잠금 이벤트, 로그인 항목 등록 성공 여부, 최전면 앱 신원)이 AppKit/ServiceManagement 타입에 직접 묶여 있어 "순수 Rust" 로 분류하면 모호해진다.
- **기각한 대안 2 — "네이티브 shim 불가피".** 메뉴 항목별 아이콘이 `tray-icon` 고수준 API 로 되는지는 미확인이지만, `ns_status_item()` 이 `NSStatusItem`(따라서 그 `NSMenu`)에 대한 직접 접근을 제공하므로 `objc2-app-kit` 경로가 이미 열려 있다. Swift/Objective-C 로 별도 프로세스나 프레임워크를 작성해야 하는 진짜 공백은 발견되지 않았다.
- **기각한 대안 3 — 원본의 헬퍼 앱(`SuperkeyLauncher.app`) 방식을 그대로 재현.** `SMLoginItemSetEnabled` 는 별도 헬퍼 번들이 `Contents/Library/LoginItems/` 에 있을 것을 요구하는 구식 패턴이다. macOS 13+ 전용인 `SMAppService.mainApp`(메인 앱 자신을 로그인 항목으로 등록, 헬퍼 불필요)만으로 13+ 를 커버하고, macOS 12 지원이 필수적이지 않다면 헬퍼 앱 패턴 자체를 생략하는 것도 대안이다 → 최소 지원 버전 정책 자체는 이 문서 범위 밖의 제품 결정.

## 8. 수용 기준

- [ ] 앱 실행 시 Dock 아이콘이 표시되지 않고, `⌘Tab` 앱 스위처 목록에도 나타나지 않는다.
- [ ] Accessibility 권한이 없는 상태에서 메뉴를 열면 `unauthorizedMenu` 에 준하는 대체 메뉴(권한 상태 안내 + 권한 요청 진입점)가 표시된다.
- [ ] 정상 동작 상태에서 메뉴바에 §3.3 이 정한 항목(재현 대상으로 판정된 것들)이 순서대로 표시되고, 최전면 앱 전환 시 `Ignore <앱이름>` 라벨이 실시간으로 갱신된다.
- [ ] `Ignore <앱이름>` 을 선택하면 그 앱이 최전면일 때 F-07 의 리매핑이 통과(패스스루)된다.
- [ ] 이미 인스턴스가 실행 중인 상태에서 앱을 다시 실행하면, 두 번째 프로세스는 새 `CGEventTap` 을 설치하지 않고 종료(또는 기존 인스턴스로 위임)한다.
- [ ] `Launch on login` 의 출고 기본값은 ☐(꺼짐)이다.
- [ ] `Launch on login` 을 켠 상태로 재부팅하면, macOS 13 이상과 macOS 12 양쪽 모두에서 로그인 후 앱이 자동 실행된다. 등록이 실패하면 상한 있는 재시도 후 사용자에게 실패를 알린다(무한 재시도 금지, §5 항목 3).
- [ ] macOS 12 에서 `SMAppService` API 를 호출하지 않는다(런타임 크래시 없음).
- [ ] 절전에서 깨어난 직후 리매핑이 정상 동작한다(수동 재실행 없이). 최근에 이미 재시작했다면 불필요한 재초기화를 건너뛴다(디바운스).
- [ ] `AXIsProcessTrusted() == true` 인데 event tap 생성이 계속 실패하는 상황을 재현하면, 앱은 권한 어긋남 진단 화면을 띄우고(F-11 §3 UI), 사용자가 문제를 해소하지 못하면 상태를 알리는 로그와 함께 정상 종료 절차를 거쳐 프로세스가 끝난다(원본처럼 무한정 조용히 죽어있는 상태로 남지 않는다).
- [ ] 메뉴에서 `Quit Superkey` 을 선택하면, 그 시점에 합성 상태로 눌려 있던 modifier 가 모두 해소되고 event tap 해제·오버레이 창 파괴가 완료된 후 프로세스가 종료된다.
- [ ] `Hide menu bar icon` 을 켠 상태에서 메뉴바 아이콘 접근이 사라지면, `When hidden, relaunch from Finder to open.` 에 준하는 복구 안내가 제공된다.

## 9. 미해결 질문

이번 실측으로 **해소된 항목**(더 이상 §9 에 남지 않음): 메뉴바 메뉴 전체 구성·순서·라벨, `General` 탭의 실제 항목·기본값, 로그인 실행 방식(헬퍼 앱 + `SMLoginItemSetEnabled`/`SMAppService` 이중 경로), `LSUIElement`.

남은 미해결 질문:

| # | 질문 | 근거 문서 연결 | 확인 방법 |
| :--- | :--- | :--- | :--- |
| 1 | `unauthorizedMenu` 의 실제 겉모습(비활성 라벨인지 클릭 가능한지, 아이콘 유무) | app-bundle-analysis.md §4.4 — 문자열로만 존재 확인, 권한이 있는 상태라 직접 관찰 못함 | Accessibility 권한을 실제로 회수한 뒤 재관찰 |
| 2 | `Menu bar icon` 팝업 2종의 정체 | app-bundle-analysis.md §6.4, §8 항목 2 — `Assets.car` 는 저작권 경계상 추출하지 않음 | 실기에서 각 옵션을 선택해 실제 아이콘 모양 스크린샷으로 비교(에셋 추출 없이) |
| 3 | `Ignore <앱이름>` 이 Seek 트리거까지 끄는지, 리매핑만 끄는지 | app-bundle-analysis.md §4.2 | 실기에서 앱을 무시 목록에 넣고 그 앱이 최전면일 때 Seek 단축키가 동작하는지 확인 |
| 4 | `typeToSeekEnabledAppIDs` / `typeToSeekDisabledAppIDs` 의 기능과 대응 UI | app-bundle-analysis.md §2.3, §8 항목 9 | 4개 탭을 다시 정밀 스캔하거나 개발자 문의 |
| 5 | 헬퍼(`SuperkeyLauncher.app`) 핑퐁 프로토콜(`terminateOnHelperPong`/`receivePong`)의 정확한 절차와 타임아웃 | app-bundle-analysis.md §3.2(로그인 실행) | 헬퍼 프로세스를 강제 종료해 앱의 반응을 관찰 |
| 6 | `Relaunch on wake` 체크박스의 표시 조건 | app-bundle-analysis.md §6.4 | nib 조건부 바인딩을 유발하는 상태를 재현(다른 설정 조합 시도) |
| 7 | `Screenshot.storyboardc` 의 용도 | app-bundle-analysis.md §8 항목 10 | 이 스토리보드를 여는 코드 경로를 실기에서 유발할 방법 탐색 |
| 8 | v1.62 "menu item icons" 가 §3.3 의 어느 항목에 실제로 붙는지 | superkey-inventory.md §2.1 v1.62 | 메뉴를 열어 각 항목 왼쪽 아이콘 유무를 스크린샷으로 직접 확인(AX 트리 대신) |
| 9 | 재시작 디바운스·깨어남 후 지연의 정확한 임계값 | §3.2 | `lastSleepDate`/`restartOnWakeDelay`/`wakeKeyboardDelay` 관련 동작을 실측 타이밍 테스트로 재현 |
| 10 | 이중 실행 시 두 번째 프로세스가 조용히 종료되는지, 기존 인스턴스의 메뉴를 여는지 | 조사 자료에 근거 없음 | 앱 설치 후 이중 실행 재현 |
| 11 | "Unable to initialize Superkey"(웹사이트 FAQ 문구)와 실측된 `Superkey has insufficient privileges`/`Unable to listen to device input` 문구의 정확한 관계(구버전 문구인지, 다른 화면인지) | app-bundle-analysis.md §4.4 vs superkey-inventory.md §1.3 | 원본 개발자 문의, 또는 여러 버전의 바이너리 문자열 비교 |
| 12 | 크래시 후 자동 재기동 정책(헬퍼 핑퐁과의 연관 여부 포함) | §5 항목 11 | 개발자 문의 또는 정책 자체 설계 결정 |
| 13 | 권한 없음과 트라이얼 만료가 동시에 참일 때 메뉴바가 어느 것을 우선하는지 | §3.1 각주 | 실기 재현 테스트(원본 앱에서 재현 가능하다면) |
| 14 | `SMLoginItemSetEnabled` 를 감싸는 검증된 Rust 크레이트가 있는지, 없다면 수동 FFI 선언이 필요한지 | §6, §7 | crates.io 재조회 |
| 15 | `relaunchOpensMenu` 가 실제로 재실행 후 메뉴를 자동으로 여는 동작인지 | §3.6 | 실기에서 `Advanced ▸ Relaunch` 를 눌러 재실행 직후 거동 관찰 |
| 16 | `/Applications` 밖에서 실행된 경우를 앱이 능동적으로 감지해 안내할지 여부 | 기존 §9 승계 | 정책 설계 결정 |
