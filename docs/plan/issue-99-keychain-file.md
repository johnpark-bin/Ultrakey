# 이슈 #99 — 시스템 키체인 로그인 프롬프트 제거: 빌드 분기 (개발=파일 저장, 정식 서명=Keychain) — 계획 초안

> 저장 위치: `docs/plan/issue-99-keychain-file.md`
> 상태: **상급 리뷰 반영 완료 — 구현 진행** (리뷰 내역은 §5)
> 결론부터: **컴파일 타임 Cargo feature `keychain-store`(기본 OFF)로 분기한다.** 개발/자체 서명 빌드는 `FileStore`(`~/Library/Application Support/Ultrakey/` JSON 파일), 정식 Apple 서명 빌드만 기존 `KeychainStore`를 주입한다. `TrialStore`/`CacheStore` 트레이트는 불변.

---

## 1. 요약

자체 서명/개발 빌드에서 최초 실행 시 시스템 로그인(키체인 접근) 프롬프트가 뜨는 문제(이슈 #99 사용자 피드백)를 **빌드 분기**로 해결한다. 프롬프트의 근본 원인은 `kSecAttrAccessControl`(ACL) 미지정 + 자체 서명 신원 변화로 추정된다(이슈 #99, 스크린샷 실측은 미실시). 확정 방향은 이슈에 이미 기록돼 있다: **개발 빌드는 파일 저장으로 프롬프트를 원천 차단하고, 정식 서명 릴리즈 빌드만 F-12 §3.5 의 Keychain 결정(트라이얼 무한 반복 방지)을 유지한다.**

구현은 ① `FileStore` 신설(경로 주입 가능, 원자적 쓰기) ② `ultrakey-app` 에 `keychain-store` feature(기본 OFF) ③ `LicenseController::boot()` 이 feature 에 따라 구현체 선택 ④ `release.yml` 이 **서명 시크릿이 있을 때만** feature 활성화 ⑤ 명세 §3.5·§5·§6·§7·§8 갱신(원본 결정 보존 + 개정 첨부)이다.

## 2. 확정된 사실 (코드 실측)

- `crates/ultrakey-platform/src/keychain.rs` — `KeychainStore` 가 generic password 2종(`com.ultrakey.license.trial` · `com.ultrakey.license.cache`)에 JSON 바이트를 `kSecValueData` 로 저장. `kSecAttrAccessibleAfterFirstUnlock`, `kSecAttrSynchronizable=false`. **ACL 미지정.** 자체 단위 테스트 없음(`#[cfg(test)]` 부재 — 회귀는 컴파일 + 상태 머신 테스트로 커버).
- `crates/ultrakey-license/src/store.rs` — `TrialStore`·`CacheStore` 트레이트 + `InMemoryStore`. 저장 값은 `TrialRecord { clock: TrialClock { trial_started_at, last_seen_at }, trial_consumed }`, `LicenseCache{license_key, device_id, cache_token, cache_issued_at, activations_used, activations_limit}` — 전부 `serde` derive 완료.
- 시계 조작 완화(`last_seen_at`)는 **저장소 위 계층**(`decision.rs::next_last_seen` + `machine.rs`)에 있다 — 저장소 구현체와 무관하게 유지된다. 상태 머신의 모든 저장은 `Mutex<LicenseMachine>` 아래서 일어난다.
- `apps/ultrakey-app/src/license.rs:37` — `LicenseController::boot()` 이 `Arc<KeychainStore>` 를 만들어 두 트레이트 객체로 주입. `store` 필드(`Arc<KeychainStore>`)는 `deactivate_device` 의 `read_cache()` 에만 쓰인다. **`KeychainStore` 참조는 이 파일이 유일하다.**
- 저장소 경로 관례: `std::env::var_os("HOME")` 기반(`main.rs:1340` 로그 디렉터리, `login_item.rs:277`, `overlay_spike.rs:344`). `dirs` 크레이트 없음. F-15 설정은 tauri `app_data_dir()`(`~/Library/Application Support/app.ultrakey.Ultrakey`) — 라이선스 파일 저장소는 이슈 확정 방향대로 **제품 이름 디렉터리** `~/Library/Application Support/Ultrakey/` 를 쓴다(원본 SuperKey 의 `~/Library/Application Support/Superkey/` 관례와 동일 계열, 실측: `superkey-inventory.md` §1).
- `scripts/build-signed.sh` — `cargo tauri build --target universal-apple-darwin`(feature 없음). `.github/workflows/release.yml` — `tauri build --target universal-apple-darwin`, **서명 시크릿 유무를 `SIGNED` 로 판정하고 그 뒤에 빌드**(92-101행) → feature 조건부 활성화가 구조적으로 가능하다.
- `ultrakey-license` 의존은 `serde`·`thiserror` 뿐 — JSON 파일 저장을 위해 `serde_json`(워크스페이스 1.0.151, 2026-08-30 검증) 추가가 필요하다.
- `docs/dev/manual-verification.md` §15-2 가 Keychain 항목 실기기 검증 절차(`security find-generic-password`)를 갖고 있다 — 빌드 분기에 맞춰 갱신 대상.

## 3. 결정 목록

### D1 — 분기 방식: 컴파일 타임 Cargo feature `keychain-store` (기본 OFF) ⭐ 핵심

이슈 초안 권고를 채택한다. 개발/자체 서명 바이너리에는 Keychain 배선이 **컴파일 단계에서 사라지고**, 릴리즈 바이너리에는 파일 배선이 사라진다 — 런타임에 "잘못된 저장소"가 선택될 가능성이 없다.

**기각한 대안**:
- **런타임 감지**(`is_running_from_app_bundle` + 서명 주체 확인) — 이슈 초안 자체가 "배포 검증이 컴파일 타임보다 약함"으로 덜 권장. 게다가 서명 주체 확인 코드는 `SecCode*` FFI 를 새로 요구하는데, 자체 코드 서명 검증은 명세 §3.9 에서 "Paddle 실연동 시점까지 구현하지 않는다"고 이미 결정한 영역이다. 기각.
- **build.rs 환경변수**(`ULTRAKEY_LICENSE_STORE=file|keychain`) — 카고 생태계 표준이 아니고, `cargo metadata`/IDE/문서 어디에도 의도가 드러나지 않는다. feature 가 같은 일을 표준 방식으로 한다. 기각.

### D2 — feature 위치: `ultrakey-app` 전용. `ultrakey-platform::keychain` 은 항상 컴파일

`keychain.rs` 모듈 자체는 feature 로 게이트하지 않는다. 근거: **`cargo test --workspace` 와 clippy 가 기본(무-feature) 빌드에서도 `keychain.rs` 를 계속 컴파일**해 배포 전용 코드가 조용히 썩는 것을 막는다. feature 는 `license.rs` 의 주입 지점(`cfg`)에서만 분기한다.

**기각한 대안**: `ultrakey-platform` 에 feature 를 두고 `pub mod keychain` 을 게이트 — 기본 `cargo test --workspace`·CI 가 Keychain 어댑터를 아예 컴파일하지 않게 되어 회귀 커버리지가 사라진다. 기각.

### D3 — FileStore 위치: `crates/ultrakey-license/src/file_store.rs` 신설

`FileStore` 는 순수 표준 라이브러리(`std::fs` + `serde_json`)다 — FFI 없음, `unsafe` 없음, macOS 비의존. `#![forbid(unsafe_code)]` 인 라이선스 크레이트에 맞고, 트레이트·`InMemoryStore` 와 같은 크레이트에 모여 저장소 추상 전반이 한곳에 보인다. Sparkle 프레임워크 획득 없이 `cargo test -p ultrakey-license` 로 검증된다.

**기각한 대안**:
- `ultrakey-platform` — 이 크레이트의 정체는 "macOS FFI 경계, 모든 `unsafe` 가 여기만 있다"다. FFI 가 없는 파일 저장이 들어가면 경계 서술이 희석된다. (선행 사례 `login_item.rs` 는 관심사 자체가 플랫폼 종속이라 경우를 달리한다.)
- `ultrakey-app` — 영속 로직이 앱 껍데기에 섞이고, 앱 크레이트 밖에서 테스트하려면 의존을 거꾸로 걸어야 한다. 기각.

### D4 — FileStore 설계

```rust
pub struct FileStore { dir: PathBuf, _guard: Mutex<()> }
impl FileStore {
    pub fn new(dir: PathBuf) -> Self;        // 경로 주입 — 테스트는 임시 디렉터리
    pub fn default_dir() -> Option<Self>;     // $HOME/Library/Application Support/Ultrakey — HOME 없으면 None
}
```

- **파일 2종**: `trial.json`(`TrialRecord`) · `license-cache.json`(`LicenseCache`) — Keychain 의 항목 2종과 1:1 대응. 직렬화는 **Keychain 저장소와 동일한 `serde_json` 바이트** → 두 저장소 간 바이트 호환(향후 이전·백업이 자명).
- **원자적 쓰기**: 같은 디렉터리의 임시 파일(`<이름>.tmp`)에 쓴 뒤 `fs::rename` — 같은 볼륨 보장이므로 APFS 에서 원자적. 쓰다 죽어도 최종 파일은 손상되지 않는다. ⭐ 고정 tmp 파일명·충돌 안전은 **단일 작성자**일 때만 성립한다 — 그 전제는 F-10 §5 의 단일 인스턴스 보장(`crates/ultrakey-platform/src/single_instance.rs`, 두 번째 프로세스는 Tauri 기동 전 종료)으로 세워진다.
- **디렉터리 생성 시점**: `create_dir_all` + `0o700` 적용은 **쓰기 경로에서**(첫 쓰기 시) 수행한다 — "설정을 안 건드리면 파일이 안 생긴다"는 F-15 선례(설정 저장소 지연 생성, `main.rs:4261-4263`)와 동일하게, 상태를 한 번도 안 쓴 프로세스는 디렉터리도 만들지 않는다. 이미 존재하는 디렉터리에는 `set_permissions` 로 `0o700` 을 재적용한다(멱등).
- **권한**(unix): 디렉터리 `0o700` · 파일 `0o600`(`std::os::unix::fs::PermissionsExt`, 신규 의존 없음). 파일에 `license_key` + `cache_token` 이 담긴다.
- **읽기 의미론**: 파일 없음 → `None`, 손상 JSON → `None` — `KeychainStore` 의 `.ok()` 의미론과 동일.
- `clear_cache`: `remove_file`, `NotFound` 무시.
- **직렬화 가드**: `Mutex<()>` 가 전 연산을 직렬화(`KeychainStore._guard` 와 대칭 — 같은 tmp 파일명에 대한 동시 쓰기 방지). 상태 머신이 이미 `Mutex<LicenseMachine>` 으로 직렬화하지만, 어댑터 자체의 계약으로 둔다.
- 쓰기 실패는 조용히 무시(`KeychainStore` 의 `SecItemAdd` 실패 무시와 대칭).
- **`last_seen_at` 시계 조작 완화 로직은 손대지 않는다** — 그 로직은 `machine.rs`/`decision.rs`(저장소 위 계층)에 있고 `TrialRecord` 를 그대로 영속하는 것으로 유지된다.
- **HOME 부재 폴백**: `boot()` 가 `InMemoryStore` 로 강등 + `tracing::error`. 라이선스 상태 머신은 프로세스 내에서 계속 동작(체험은 매 실행 재시작) — macOS GUI 세션에서 사실상 불가능한 상황의 방어선.

**기각한 대안**:
- 파일 1개에 두 기록 병합 — `clear_cache` 가 트라이얼 기록을 재작성해야 하고, Keychain 의 2항목 모델과도 어긋난다.
- plist — 이득 없음. JSON 이어야 Keychain 저장소와 바이트 호환.

### D5 — `LicenseController` 구현체 무관화

`store: Arc<KeychainStore>` 필드를 `store: Arc<dyn CacheStore + Send + Sync>` 로 바꾼다. `boot()` 는 `cfg` 분기로 구체 구현체를 만들고 두 트레이트 객체(`TrialStore` · `CacheStore`)로 강제 변환해 주입한다. `deactivate_device` 는 트레이트 경유 `read_cache()` 그대로 — 동작 불변.

⭐ **import 문도 동일 feature 로 게이트한다** — `use ultrakey_platform::keychain::KeychainStore;`(license.rs:19)를 `#[cfg(feature = "keychain-store")]` 로 감싸지 않으면, 기본(무-feature) 빌드에서 unused import 경고가 나 `cargo clippy --all-targets -- -D warnings` 게이트가 실패한다. `FileStore` import 는 `#[cfg(not(feature = "keychain-store"))]` 로 게이트.

### D6 — 빌드 파이프라인 배선

| 위치 | 변경 |
| :--- | :--- |
| `apps/ultrakey-app/Cargo.toml` | `[features] keychain-store = []` (기본 OFF) 추가 |
| `scripts/build-signed.sh` | 빌드 명령 무변경(무-feature = 파일 저장). 분기 설명 주석만 추가 |
| `ci.yml` check job | ⭐ `cargo check -p ultrakey-app --features keychain-store` + `cargo clippy -p ultrakey-app --features keychain-store -- -D warnings` 추가 — **PR·main 푸시 게이트**가 릴리즈 분기를 컴파일·린트하게 한다 |
| `release.yml` build job | `SIGNED=true` 일 때만 `tauri build --target universal-apple-darwin --features keychain-store`. 미서명 릴리즈는 파일 저장(아래 근거) |
| `release.yml` check job | (선택) `ci.yml` 과 동일한 feature 컴파일 스텝 — 태그 푸시 게이트에서도 재확인 |

⭐ **`ci.yml` 에도 게이트를 두는 근거(상급 리뷰 반영)**: `release.yml` 은 `v*` 태그 푸시에만 돈다. feature 분기 컴파일 검증을 릴리즈 게이트에만 두면 **릴리즈 분기가 처음으로 컴파일되는 시점이 릴리즈 태그 푸시 순간**이 되어, 계획의 "두 분기 모두 CI 컴파일 검증" 근거가 PR 단계에서 성립하지 않는다. 따라서 기본(무-feature) 빌드는 기존 `cargo test --workspace` + clippy 가, 릴리즈 분기(feature ON)는 `ci.yml` 의 신규 스텝이 커버한다.

⭐ **미서명 릴리즈 빌드가 파일 저장을 쓰는 근거**: 저장소 선택 규칙표에서 파일/Keychain 을 가르는 기준은 "서명 주체가 정식 Apple(Developer ID)인가"다. 시크릿 미설정 릴리즈 빌드는 자체 서명도 없는 빌드라 프롬프트 문제가 재현될 수 있다 — 이슈가 고치려는 바로 그 버그를 릴리즈 산출물에 남길 수 없다.

⭐ **`SIGNED=true` ⟺ Developer ID 전제(상급 리뷰 반영)**: `release.yml` 의 `SIGNED` 는 기계적으로 **시크릿 존재**만 확인한다. 이것이 Developer ID 서명을 의미하는 근거는 `code-signing.md` §8 의 시크릿 계약 — `APPLE_CERTIFICATE` 는 **Developer ID Application .p12** 다. 이 계약과 다른 인증서가 설정되면 릴리즈에 프롬프트 문제가 재현될 수 있다(설정 오류이며 워크플로가 탐지할 수 있는 범위가 아니다).

### D7 — Security FFI 유지

`ffi.rs` 의 `SecItem*` 4함수 + `kSec*` 상수 9종은 **배포 빌드가 계속 쓰므로 유지**한다. 이슈 초기 방향의 "쓸 곳 없어지면 제거"는 빌드 분기 결정으로 폐기 — 제거하지 않는다.

### D8 — 마이그레이션 정책: **없음 — 1회 리셋 수용** (상급 리뷰로 추가된 결정)

이전 빌드(Keychain 저장)를 쓰다가 파일 저장 빌드로 올리면 트라이얼/캐시 상태가 **1회 리셋**되고, 옛 `com.ultrakey.license.trial`·`com.ultrakey.license.cache` 항목은 로그인 키체인에 고아로 남는다. 역방향(미래의 미서명 릴리즈(파일) → 정식 서명 릴리즈(Keychain))도 동일한 1회 리셋. 이 전환을 **마이그레이션 없이 수용**한다.

**근거**:
1. 파일 저장이 나가는 빌드는 개발/자체 서명/미서명이며, 이슈가 이미 "개발 빌드는 트라이얼 리셋 가능" 리스크를 수용했다(이슈 #99 확정 방향). 상용 보호 대상인 정식 서명 릴리즈는 Keychain 을 그대로 유지하므로 상용 사용자의 트라이얼은 리셋되지 않는다.
2. 파일 빌드가 옛 Keychain 항목을 읽거나 지우려면 그 자체가 Keychain 접근이다 — 서명 주체가 바뀐 항목에 접근하면 이 이슈에서 제거하려는 바로 그 시스템 프롬프트가 다시 뜰 수 있다 `(추정 — 마이그레이션이 프롬프트를 유발할 수 있다는 판단은 실측 미실시)`. 따라서 **"파일 빌드는 Keychain 을 일절 건드리지 않는다"** 가 일관된 설계다.
3. 마이그레이션 코드는 두 저장소 포맷·순서·부분 실패를 다루는 영속 계층 복잡도를 추가하지만, 얻는 것은 개발 빌드의 1회 트라이얼 연속성뿐이라 비용 대비 가치가 없다.

이 결정은 명세 §5 갱신(전환 노트)에도 함께 기록한다.

### D9 — 문서 갱신 범위 (원본 보존 + 결정 첨부 — README 갈라짐 표 절차)

| 문서 | 갱신 |
| :--- | :--- |
| `docs/spec/licensing-and-trial.md` §3.5 | 원본 "Keychain 채택" 결정·표·근거를 **지우지 않고** 그 뒤에 ⭐ 개정 블록(2026-09-03, 이슈 #99) 첨부: 분기 규칙표 · 왜 뒤집는가(프롬프트) · 개발 빌드 한정 리스크(원본 기각 근거 인용) · FileStore 세부 |
| 같은 문서 §5 | #6 재설치(파일도 앱 번들 밖이라 재설치에 생존 — 양 빌드 동일, 명시) · #10 파일 삭제(개발 빌드 = 리셋 수용 / 정식 서명 = 현행 유지) · **전환 노트**(저장소 전환 시 1회 리셋 — D8 마이그레이션 없음 결정 기록) |
| 같은 문서 §6 | Keychain 항목 → "정식 서명 빌드 한정" 명시 + 파일 저장(표준 라이브러리 `std::fs`, 원자적 쓰기) 항목 추가 |
| 같은 문서 §7 | "Keychain 에 저장" → 분기 서술로 갱신. "Rust 바인딩" 판정은 유지(정식 서명 빌드가 여전히 Security FFI 를 쓴다) |
| 같은 문서 §8 | #1 저장소 중립 문구로 갱신 · **#3 는 저장소 중립이 아니라 빌드별로 쓴다**(정식 서명: 현행 유지 / 파일 빌드: 일반 재설치에는 생존하나 `~/Library/Application Support` 삭제 시 리셋 — §5 #10 교차 참조) + 신규 3항: ① 자체 서명/개발 빌드 최초 실행 프롬프트 없음 + 파일 기록 ② 정식 서명 빌드 Keychain 유지 ③ 쓰다 죽어도 JSON 손상 없음(원자적 쓰기) |
| `docs/dev/manual-verification.md` **§15 전체** | ⭐ Keychain 전제가 §15 전체에 퍼져 있다(실측: §15 서두 2815행 · §15-2 #4·#5 · §15-4 #2 2857행 · §15-5 2867행 · 통과 판정 요약표 2880·2882행). 빌드별 절차 분리 원칙을 전부 적용 — 개발 빌드: `cat ~/Library/Application Support/Ultrakey/trial.json` · 정식 서명 빌드: 기존 `security find-generic-password` 절차. §15-2 #4 의 "재설치 생존"은 파일 빌드에서도 성립(파일은 앱 번들 밖) |
| `docs/spec/README.md` F-12 행 | 구현 접근 열 "순수 Rust (+ Keychain·Security 바인딩)" → §3.5 개정 블록을 가리키는 분기 안내 1구 추가(분류 자체는 유지 — 정식 서명 빌드가 FFI 를 쓴다) |
| `crates/ultrakey-license/src/store.rs` 독 주석 | "§3.5 는 … **Keychain** 에 저장"(2-10행)·"요구사항 … (Keychain)"(37-38행)을 저장소 중립 문구로 갱신 |
| `docs/spec/platform-constraints.md` §0 F-12 행 | "순수 Rust (+ Keychain 은 바인딩)" → 분기 한 줄 명시(최소 수정) |

## 4. 테스트 계획

1. **`file_store.rs` 인라인 테스트**(신규 의존 없음 — `std::env::temp_dir()` + PID·카운터 조합의 고유 디렉터리):
   - 쓰기→읽기 왕복(트라이얼 + 캐시) · 파일 없음 → `None` · 손상 JSON → `None`
   - `clear_cache` 후 캐시 `None` + 트라이얼 기록 무손상
   - 원자적 쓰기: 쓰기 후 `*.tmp` 잔존 없음, 최종 파일 존재
   - 권한: `0o600` 파일 · `0o700` 디렉터리 (unix)
   - `default_dir()` 경로 형태(`~/Library/Application Support/Ultrakey` 로 끝남)
   - ⭐ **다중 스레드 직렬화 1건** — `Mutex<()>` 가드 계약(`&self` 로 동시 쓰기)을 문서 이상으로 검증
2. **회귀**: `cargo test --workspace`(상태 머신 인라인 테스트 + Keychain 어댑터 컴파일) · `cargo clippy --workspace --all-targets -- -D warnings`.
3. **feature 분기 컴파일 검증**: `cargo check -p ultrakey-app --features keychain-store` + clippy(기본 빌드와 분리 실행).
4. **실기기 — ⭐ 머지 전 필수(상급 리뷰로 격상)**: `build-signed.sh` 산출물을 키체인 상태가 깨끗한 조건에서 최초 실행해 ① 시스템 로그인 프롬프트 부재 ② `trial.json` 생성을 확인하고 결과를 PR 에 기록한다. 프롬프트의 근본 원인이 여전히 `(추정 — 스크린샷 실측 미실시)`(이슈 #99) 상태라 실기기 확인이 유일한 종결 수단이다. 자동 실행은 키 리매핑 앱 특성상 입력 가로챔 위험이 있어 수동 절차를 남기되, **이 검증을 통과하기 전에는 머지하지 않는다.**

## 5. 리뷰 반영 (상급 리뷰 완료)

`ultrakey-review`(상급) 리뷰 판정: **조건부 승인 — 블로커 없음, 중요 4건 + 제안 6건**. 전부 반영했다.

| # | 등급 | 지적 | 반영 |
| :--- | :--- | :--- | :--- |
| 1 | 중요 | `release.yml` 은 태그 푸시에만 도는데 분기 컴파일 검증을 그곳에만 두면 PR 단계에서 분기가 컴파일되지 않는다 | D6 — `ci.yml` check job 에 feature 분기 `cargo check` + `clippy` 추가. 근거 문단 신설 |
| 2 | 중요 | `manual-verification.md` 갱신 범위가 §15-2 만으로 좁다 | D9 — 대상을 **§15 전체**(서두·§15-2·§15-4·§15-5·요약표)로 확대 |
| 3 | 중요 | 기존 Keychain 항목의 전환(마이그레이션) 결정 누락 | **D8 신설** — 마이그레이션 없음 · 1회 리셋 수용 · 파일 빌드는 Keychain 미접촉. 명세 §5 전환 노트에도 기록 |
| 4 | 중요 | 이슈 수용 기준(프롬프트 없음) 검증이 "선택"으로 밀려 있음 | §4.4 — **머지 전 필수 실기기 검증**으로 격상 |
| 5 | 제안 | `SIGNED=true` ⟺ Developer ID 전제 미인용 | D6 — `code-signing.md` §8 시크릿 계약 인용 |
| 6 | 제안 | 직렬화·충돌 안전 주장의 전거 미특정 + 디렉터리 생성 시점 미특정 | D4 — F-10 단일 인스턴스 인용 · 쓰기 경로 지연 생성 · 멱등 재적용 명시 + §4.1 직렬화 테스트 추가 |
| 7 | 제안 | `license.rs:19` import 도 feature 게이트 필요 | D5 — import 게이팅 명시 |
| 8 | 제안 | `docs/spec/README.md` F-12 행 + `store.rs` 독 주석 누락 | D9 — 두 대상 추가 |
| 9 | 제안 | `TrialRecord` 형태 정밀도 | §2 — 중첩 `TrialClock` 구조로 정정 |
| 10 | 제안 | §8 #3 "저장소 중립 문구"가 빌드별 실제 차이를 가림 | D9 — §8 #3 를 **빌드별** 문구로 명시(파일 빌드는 Application Support 삭제 시 리셋) |

리뷰어가 타당 확인한 것: D1~D3·D7(근거 실측 일치) · D4 원자성·권한·바이트 호환 · D6 `tauri build --features` 플래그 실존 · 샌드박스 없음으로 경로 리다이렉트 부재 · 갈라짐 표 등재 불필요(원본 자체가 파일 저장).
