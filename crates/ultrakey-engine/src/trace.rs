//! ⭐ 진단 계측 — 이슈 #19(물리 caps lock 버그) 원인 확정용. 기본은 꺼져 있고
//! `ULTRAKEY_TRACE_TAP=1`(키보드만) 또는 `ULTRAKEY_TRACE_TAP=all`(마우스 포함)로 켠다.
//!
//! **탭 콜백 임계 경로에 락 없음, 힙 할당 없음, 동기 로깅 없음**(`docs/dev/
//! architecture.md` §2.1/§2.2)은 이 계측에도 그대로 적용된다 — 계측을 넣으려고
//! 그 불변식을 깨면 진단 도구 자체가 진단하려는 문제를 만들어낸다. 그래서:
//!
//! - 콜백(생산자, 탭 스레드)은 [`TapTrace`](POD·`Copy`, 힙 없음)를 만들어
//!   [`TraceRing::push`] 하나만 부른다. `tracing::*` 매크로도, `Mutex` 도,
//!   `Vec`/`String`/`format!` 도 여기 없다.
//! - 전용 드레인 스레드(소비자)가 200ms 마다 링을 비우고 그때 비로소
//!   `tracing::info!` 로 사람이 읽을 수 있는 한 줄을 남긴다 — 탭 스레드가
//!   아니므로 로깅 제약이 없다.
//!
//! ⛔ 링 버퍼 원시 자료구조(`unsafe impl Sync` 가 필요한 `UnsafeCell` 기반 SPSC)는
//! 이 파일에 없다 — 이 크레이트는 `#![forbid(unsafe_code)]` 다(`lib.rs`).
//! `docs/dev/architecture.md` §1 은 "이 저장소의 모든 `unsafe` 는
//! `ultrakey-platform` 에만 있다"를 크레이트 경계의 근거로 못박고 있고, 이는
//! FFI 뿐 아니라 이런 저수준 동시성 자료구조에도 그대로 적용해야 문서와 코드가
//! 어긋나지 않는다. 그래서 원시 자료구조는
//! [`ultrakey_platform::trace_ring::SpscRing`] 에 두고, 여기 [`TraceRing`] 은
//! `TapTrace` 전용으로 그 안전한 API 만 얇게 감싼다(위임 지시서가 준 설계를
//! 그대로 따르되 unsafe 배치만 이 저장소의 기존 경계에 맞췄다 — 자세한 근거는
//! 위임 완료 보고에 남긴다).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, RwLock};
use std::thread::{self, JoinHandle};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::Serialize;

use ultrakey_core::arbitration::{Disposition, Effect, Layer};
use ultrakey_core::event::{EventKind, InputEvent};
use ultrakey_core::flags::EventFlags;
use ultrakey_core::rules::RuleId;

/// 링 버퍼 용량. 2의 거듭제곱이어야 한다([`ultrakey_platform::trace_ring::SpscRing`]
/// 이 인덱스 계산에 비트마스크를 쓴다).
pub const TRACE_RING_CAP: usize = 512;

/// 판정 결과로 방출된 이벤트 하나(계측용 요약 — `ultrakey_core::arbitration::
/// SynthEvent` 를 그대로 담지 않는 이유는 `TapTrace` 를 POD 로 유지하기 위함이다).
#[derive(Debug, Clone, Copy, Default)]
pub struct TraceEmit {
    pub kind: u8,
    pub keycode: u16,
    pub flags: u64,
}

/// 한 이벤트의 계측 레코드. 전부 POD + `Copy` — 힙 할당이 없다.
///
/// `emitted`/`effects` 는 각각 최대 4개까지만 담는다. `Outcome` 자체는
/// `emitted()` 를 최대 8개까지 낼 수 있지만(`arbitration.rs` `MAX_EMIT`), 계측
/// 레코드는 진단용 요약이라 4개로 잘라도 충분하다 — 실제로 8개를 다 채우는
/// 경로는 P1(조합 발화) 뿐이고 caps lock 진단에는 관련이 없다.
#[derive(Debug, Clone, Copy, Default)]
pub struct TapTrace {
    pub seq: u64,
    /// 탭에 **도착한 그대로**의 값 — 이것이 이 계측의 핵심이다.
    pub raw_kind: u8,
    pub raw_keycode: u16,
    pub raw_flags: u64,
    pub autorepeat: bool,
    /// D-1 alias 환원 이후 판정에 쓰인 keycode.
    pub resolved_keycode: u16,
    pub alias_active: bool,
    pub layer: u8,
    /// ⭐ F-18 Event Viewer(`docs/spec/event-viewer.md` §3.4) — 발화한 규칙의 숫자
    /// 인코딩. 0=없음(규칙 없이 통과) 1=`RuleId::Preset` 2=`RuleId::Korean`
    /// 3=`RuleId::Hyperkey`. `rule_index` 는 `Preset`/`Korean` 의 페이로드
    /// (`Hyperkey` 는 페이로드가 없어 0 그대로). 콜백은 이 숫자만 다룬다 — 문자열
    /// 변환(`"preset:5"` 등)은 드레인 스레드([`rule_identifier`])가 한다.
    pub rule_kind: u8,
    pub rule_index: u8,
    pub disposition: u8,
    pub disposition_flags: u64,
    pub emitted: [TraceEmit; 4],
    pub emitted_len: u8,
    pub effects: [u8; 4],
    pub effects_len: u8,
    /// 경로 C(`ToggleCapsLock`) 실측 — 0=시도안함 1=성공 2=실패.
    pub path_c_result: u8,
    /// 0=off 1=on 2=읽기실패. 효과가 없었으면(시도 안 함) 기본값 0 그대로 남는다.
    pub path_c_before: u8,
    pub path_c_after: u8,
}

// ============================================================================
// 링 버퍼 — 원시 자료구조는 ultrakey-platform 소유, 여기는 TapTrace 전용 래퍼.
// ============================================================================

/// 무잠금 SPSC 링 버퍼. 생산자 = 탭 스레드(콜백), 소비자 = 전용 드레인 스레드.
pub struct TraceRing(ultrakey_platform::trace_ring::SpscRing<TapTrace, TRACE_RING_CAP>);

impl TraceRing {
    pub fn new() -> Self {
        TraceRing(ultrakey_platform::trace_ring::SpscRing::new(TapTrace::default()))
    }

    /// 생산자(탭 스레드) 전용. 락 없음·힙 할당 없음 — 콜백에서 불러도 안전하다.
    /// 가득 찼으면 조용히 버린다(드롭 카운터만 오른다).
    pub fn push(&self, t: TapTrace) {
        self.0.push(t);
    }

    /// 소비자(드레인 스레드) 전용.
    pub fn pop(&self) -> Option<TapTrace> {
        self.0.pop()
    }

    /// 가득 찬 상태에서 버려진 누적 개수.
    pub fn dropped_count(&self) -> u64 {
        self.0.dropped_count()
    }
}

impl Default for TraceRing {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// 활성화 게이트
// ============================================================================

/// 계측 범위. 프로세스 시작 시 한 번만 환경변수를 읽어 캐시한다 — 콜백은 이 캐시만
/// 읽으므로 매 이벤트마다 환경변수 조회(락 가능성이 있는 `std::env` 내부 구현)를
/// 하지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TraceScope {
    /// 끔(기본).
    Off,
    /// 키보드 이벤트만 — `ULTRAKEY_TRACE_TAP=1`.
    KeysOnly,
    /// 마우스·스크롤까지 전부 — `ULTRAKEY_TRACE_TAP=all`.
    All,
}

static TRACE_SCOPE: OnceLock<TraceScope> = OnceLock::new();

/// ⭐ 기본이 **키보드만**인 이유(2026-08-30 실측). `all` 로 두면 마우스를 조금만
/// 움직여도 `MouseMoved` 레코드가 초당 수백 줄씩 쌓여 정작 봐야 할 키 이벤트가
/// 파묻힌다 — 이슈 #19 첫 계측 실행에서 실제로 그랬다. caps lock·shift 진단은
/// 전부 키보드 이벤트만 보면 되므로 그것을 기본으로 삼고, 마우스가 필요한 진단
/// (hyper 의 `Apply modifiers to …` 계열)은 `all` 로 명시적으로 켠다.
pub fn trace_scope() -> TraceScope {
    *TRACE_SCOPE.get_or_init(|| match std::env::var("ULTRAKEY_TRACE_TAP").as_deref() {
        Ok("1") => TraceScope::KeysOnly,
        Ok("all") => TraceScope::All,
        _ => TraceScope::Off,
    })
}

/// 계측이 켜져 있는가(범위 무관) — 드레인 스레드 기동 판정에 쓴다.
pub fn trace_enabled() -> bool {
    trace_scope() != TraceScope::Off
}

/// ⭐ F-18 Event Viewer 런타임 게이트(`docs/spec/event-viewer.md` §2.1) — 뷰어 창을
/// 여닫을 때만 갱신된다. `AtomicBool` 하나뿐인 이유는 그 문서 §2.1 이 기각한
/// `ArcSwap<Option<Sink>>` 대안 문서를 그대로 따른다 — 콜백이 뷰어가 꺼져 있을 때
/// 내는 비용을 `Relaxed` 로드 하나로 못박기 위함이다.
static VIEWER_ON: AtomicBool = AtomicBool::new(false);

/// 뷰어 창의 열림/닫힘과 1:1 로 호출된다(앱 계층). 콜백 스레드가 아니라 메인
/// (Tauri) 스레드에서 불리므로 순서 제약이 없다 — `Relaxed` 로 충분하다(§2.1: "뷰어를
/// 켜고 끄는 것은 사람이 창을 여닫는 빈도").
pub fn set_viewer_enabled(on: bool) {
    VIEWER_ON.store(on, Ordering::Relaxed);
}

/// 콜백 임계 경로에서 불린다 — `Relaxed` 로드 한 번뿐이다.
pub fn viewer_enabled() -> bool {
    VIEWER_ON.load(Ordering::Relaxed)
}

/// 이 이벤트를 기록할 것인가. 콜백 임계 경로에서 불린다 — 캐시된 값 비교뿐이다.
///
/// ⭐ F-18 — `TraceScope::Off` 에서도 뷰어가 켜져 있으면(그리고 키 이벤트에 한해)
/// 기록한다. 뷰어가 꺼져 있고 환경변수도 꺼져 있을 때만 정말로 `false` 다 — 그때
/// 콜백이 추가로 하는 일은 `viewer_enabled()` 의 `AtomicBool::load(Relaxed)` 한 번뿐
/// (`event-viewer.md` §2.1).
pub fn should_trace(kind: EventKind) -> bool {
    match trace_scope() {
        TraceScope::Off => viewer_enabled() && kind.is_key(),
        TraceScope::KeysOnly => kind.is_key(),
        TraceScope::All => true,
    }
}

// ============================================================================
// u8 판별자 ↔ enum 변환 — 콜백은 이 변환만 쓰고(할당 없음), 사람이 읽을 문자열은
// 드레인 스레드가 로그를 찍을 때만 계산한다.
// ============================================================================

pub fn event_kind_to_code(kind: EventKind) -> u8 {
    match kind {
        EventKind::KeyDown => 0,
        EventKind::KeyUp => 1,
        EventKind::FlagsChanged => 2,
        EventKind::LeftMouseDown => 3,
        EventKind::LeftMouseUp => 4,
        EventKind::RightMouseDown => 5,
        EventKind::RightMouseUp => 6,
        EventKind::OtherMouseDown => 7,
        EventKind::OtherMouseUp => 8,
        EventKind::LeftMouseDragged => 9,
        EventKind::RightMouseDragged => 10,
        EventKind::OtherMouseDragged => 11,
        EventKind::MouseMoved => 12,
        EventKind::ScrollWheel => 13,
        EventKind::TapDisabledByTimeout => 14,
        EventKind::TapDisabledByUserInput => 15,
    }
}

pub fn event_kind_from_code(code: u8) -> Option<EventKind> {
    Some(match code {
        0 => EventKind::KeyDown,
        1 => EventKind::KeyUp,
        2 => EventKind::FlagsChanged,
        3 => EventKind::LeftMouseDown,
        4 => EventKind::LeftMouseUp,
        5 => EventKind::RightMouseDown,
        6 => EventKind::RightMouseUp,
        7 => EventKind::OtherMouseDown,
        8 => EventKind::OtherMouseUp,
        9 => EventKind::LeftMouseDragged,
        10 => EventKind::RightMouseDragged,
        11 => EventKind::OtherMouseDragged,
        12 => EventKind::MouseMoved,
        13 => EventKind::ScrollWheel,
        14 => EventKind::TapDisabledByTimeout,
        15 => EventKind::TapDisabledByUserInput,
        _ => return None,
    })
}

/// 사람이 읽을 짧은 이름 — 로그 전용(드레인 스레드에서만 호출).
fn event_kind_name(code: u8) -> &'static str {
    match event_kind_from_code(code) {
        Some(EventKind::KeyDown) => "KeyDown",
        Some(EventKind::KeyUp) => "KeyUp",
        Some(EventKind::FlagsChanged) => "FlagsChanged",
        Some(EventKind::LeftMouseDown) => "LeftMouseDown",
        Some(EventKind::LeftMouseUp) => "LeftMouseUp",
        Some(EventKind::RightMouseDown) => "RightMouseDown",
        Some(EventKind::RightMouseUp) => "RightMouseUp",
        Some(EventKind::OtherMouseDown) => "OtherMouseDown",
        Some(EventKind::OtherMouseUp) => "OtherMouseUp",
        Some(EventKind::LeftMouseDragged) => "LeftMouseDragged",
        Some(EventKind::RightMouseDragged) => "RightMouseDragged",
        Some(EventKind::OtherMouseDragged) => "OtherMouseDragged",
        Some(EventKind::MouseMoved) => "MouseMoved",
        Some(EventKind::ScrollWheel) => "ScrollWheel",
        Some(EventKind::TapDisabledByTimeout) => "TapDisabledByTimeout",
        Some(EventKind::TapDisabledByUserInput) => "TapDisabledByUserInput",
        None => "Unknown",
    }
}

pub fn layer_to_code(layer: Layer) -> u8 {
    match layer {
        Layer::AppGate => 0,
        Layer::SeekSession => 1,
        Layer::HyperModifier => 2,
        Layer::PresetCombo => 3,
        Layer::SimpleRemap => 4,
        Layer::Passthrough => 5,
        // TODO(F-16 배선): F-16 한국어 입력 지원(`docs/spec/korean-input.md`)이
        // `ultrakey-core::arbitration::Layer` 에 `KoreanInput` 을 추가해 이 매치가
        // 깨졌다 — 기존 5개 코드를 재배치하지 않기 위해 새 코드 6을 뒤에 붙인다.
        // 다음 작업(F-16 배선)이 실제 게이트를 연결한다.
        Layer::KoreanInput => 6,
        // ⭐ F-06 — 트랙패드 프리즈 소비 자리(`trackpad-hyper-gesture.md` §3.2.1).
        // 코드 7은 F-16(6) 뒤에 붙인다 — 기존 코드 재배치 금지(위 주석과 같은 이유).
        Layer::TrackpadFreeze => 7,
    }
}

pub fn layer_from_code(code: u8) -> Option<Layer> {
    Some(match code {
        0 => Layer::AppGate,
        1 => Layer::SeekSession,
        2 => Layer::HyperModifier,
        3 => Layer::PresetCombo,
        4 => Layer::SimpleRemap,
        5 => Layer::Passthrough,
        6 => Layer::KoreanInput,
        7 => Layer::TrackpadFreeze,
        _ => return None,
    })
}

fn layer_name(code: u8) -> &'static str {
    match layer_from_code(code) {
        Some(Layer::AppGate) => "AppGate",
        Some(Layer::SeekSession) => "SeekSession",
        Some(Layer::HyperModifier) => "HyperModifier",
        Some(Layer::PresetCombo) => "PresetCombo",
        Some(Layer::SimpleRemap) => "SimpleRemap",
        Some(Layer::Passthrough) => "Passthrough",
        Some(Layer::KoreanInput) => "KoreanInput",
        Some(Layer::TrackpadFreeze) => "TrackpadFreeze",
        None => "Unknown",
    }
}

/// ⭐ F-18 — `Outcome::rule()` 을 콜백이 다룰 수 있는 숫자 한 쌍으로 옮긴다. 콜백은
/// 이 변환만 쓴다(할당 없음) — 사람이 읽을 식별자([`rule_identifier`])는 드레인
/// 스레드가 만든다.
pub fn rule_to_code(rule: Option<RuleId>) -> (u8, u8) {
    match rule {
        None => (0, 0),
        Some(RuleId::Preset(n)) => (1, n),
        Some(RuleId::Korean(n)) => (2, n),
        Some(RuleId::Hyperkey) => (3, 0),
    }
}

/// 사람이 읽을 **안정적인** 규칙 식별자(`docs/spec/event-viewer.md` §3.4) — 사람이
/// 읽을 이름(예: 프리셋 라벨)은 앱 계층이 이 식별자를 보고 붙인다. 드레인 스레드
/// 전용(할당해도 되는 경로).
fn rule_identifier(kind: u8, index: u8) -> Option<String> {
    match kind {
        1 => Some(format!("preset:{index}")),
        2 => Some(format!("korean:{index}")),
        3 => Some("hyperkey".to_string()),
        _ => None,
    }
}

/// `Disposition::PassWithFlags` 는 `EventFlags` 를 함께 들고 있어 u8 하나로는
/// 왕복이 안 된다 — `TapTrace` 가 `disposition_flags` 를 별도 필드로 갖는 이유다.
/// 그래서 이 변환의 "왕복"은 **판별자(어느 variant인가)** 기준이다: flags 는
/// 호출자가 갖고 있다가 [`disposition_from_code`] 에 함께 넘긴다.
pub fn disposition_to_code(d: Disposition) -> u8 {
    match d {
        Disposition::Pass => 0,
        Disposition::PassWithFlags(_) => 1,
        Disposition::Consume => 2,
    }
}

pub fn disposition_flags_of(d: Disposition) -> u64 {
    match d {
        Disposition::PassWithFlags(f) => f.0,
        _ => 0,
    }
}

pub fn disposition_from_code(code: u8, flags: ultrakey_core::flags::EventFlags) -> Option<Disposition> {
    Some(match code {
        0 => Disposition::Pass,
        1 => Disposition::PassWithFlags(flags),
        2 => Disposition::Consume,
        _ => return None,
    })
}

fn disposition_name(code: u8) -> &'static str {
    match code {
        0 => "Pass",
        1 => "PassWithFlags",
        2 => "Consume",
        _ => "Unknown",
    }
}

/// `Effect::TypeChar` 도 위와 같은 이유로 판별자만 왕복한다 — `char` 는 계측
/// 레코드에 담지 않는다(caps lock 진단에 필요 없다).
///
/// ⭐ F-01 배선 — `Effect::SeekKey(InputEvent)` 도 같은 이유로 판별자만 담는다.
/// `InputEvent` 는 `TapTrace` 를 POD 로 유지하는 이 파일의 규약(모듈 문서 §서두)에
/// 맞지 않는 필드 조합(keycode/flags/kind/autorepeat)이라 계측 레코드에 그대로
/// 넣지 않는다 — 어차피 `emitted`/`raw_keycode` 등 기존 필드에 이미 같은 정보(어느
/// keycode 가 도착했는지)가 담겨 있어 caps lock 진단 목적에는 판별자만으로 충분하다.
pub fn effect_to_code(e: Effect) -> u8 {
    match e {
        Effect::ToggleCapsLock => 0,
        Effect::OpenSeek => 1,
        Effect::TypeChar(_) => 2,
        Effect::SeekTriggerDown => 3,
        Effect::SeekTriggerUp(_) => 4,
        Effect::SeekKey(_) => 5,
    }
}

/// `placeholder_char`/`placeholder_seek_key` — `TypeChar`/`SeekKey` 는 판별자만
/// 왕복하므로(위 `effect_to_code` 문서 참고) 실제 payload 를 복원할 수 없다. 호출자가
/// 아무 값이나 채워 넣어 돌려받는다 — 왕복 테스트가 "이 코드가 어느 variant 인가"만
/// 확인하면 되기 때문이다.
pub fn effect_from_code(code: u8, placeholder_char: char, placeholder_seek_key: InputEvent) -> Option<Effect> {
    Some(match code {
        0 => Effect::ToggleCapsLock,
        1 => Effect::OpenSeek,
        2 => Effect::TypeChar(placeholder_char),
        3 => Effect::SeekTriggerDown,
        4 => Effect::SeekTriggerUp(ultrakey_core::flags::EventFlags::NONE),
        5 => Effect::SeekKey(placeholder_seek_key),
        _ => return None,
    })
}

fn effect_name(code: u8) -> &'static str {
    match code {
        0 => "ToggleCapsLock",
        1 => "OpenSeek",
        2 => "TypeChar",
        3 => "SeekTriggerDown",
        4 => "SeekTriggerUp",
        5 => "SeekKey",
        _ => "Unknown",
    }
}

fn path_c_result_name(code: u8) -> &'static str {
    match code {
        0 => "not-attempted",
        1 => "ok",
        2 => "failed",
        _ => "Unknown",
    }
}

fn caps_lock_state_name(code: u8) -> &'static str {
    match code {
        0 => "off",
        1 => "on",
        2 => "read-failed",
        _ => "Unknown",
    }
}

// ============================================================================
// F-18 Event Viewer — 디코드된 레코드와 싱크(`docs/spec/event-viewer.md` §3.5).
// 여기서부터는 전부 드레인 스레드 전용이다 — 콜백은 이 타입들을 만들지 않는다.
// ============================================================================

/// 방출된 이벤트 하나(뷰어 표시용, 디코드됨). `TraceEmit` 의 사람이 읽을 버전.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerEmit {
    pub kind: String,
    pub keycode: u16,
    pub flags: u64,
    pub modifiers: Vec<String>,
}

/// 이벤트 뷰어 창이 한 줄로 그리는 자료(§3.3). 드레인 스레드가 [`TapTrace`] 하나당
/// 하나 만든다 — 이 파일에서 **처음** 할당이 일어나는 지점이다(탭 스레드가 아니므로
/// 허용된다, §3.5 1단계).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewerRecord {
    pub seq: u64,
    /// 드레인 스레드가 찍는 유닉스 밀리초(§3.3 "드레인 시각").
    pub at_ms: u64,
    pub kind: String,
    pub keycode: u16,
    pub flags: u64,
    pub modifiers: Vec<String>,
    pub autorepeat: bool,
    pub resolved_keycode: u16,
    pub alias_active: bool,
    pub layer: String,
    /// ⭐ 안정적인 규칙 식별자(`"preset:5"`·`"korean:13"`·`"hyperkey"`) — 사람이 읽을
    /// 이름은 앱 계층이 이 식별자를 보고 붙인다(§3.4).
    pub rule: Option<String>,
    pub disposition: String,
    pub disposition_flags: u64,
    pub emitted: Vec<ViewerEmit>,
    pub effects: Vec<String>,
    /// 예: `"ToggleCapsLock ok off->on"`. 경로 C 를 시도하지 않았으면 `None`.
    pub path_c: Option<String>,
}

/// `flags` 비트마스크에서 켜진 modifier 이름들(§3.3 표 — `⇧⌃⌥⌘` + caps lock/fn).
/// 드레인 스레드 전용(할당한다).
fn modifiers_from_flags(flags: u64) -> Vec<String> {
    let f = EventFlags(flags);
    let mut mods = Vec::new();
    if f.contains(EventFlags::SHIFT) {
        mods.push("shift".to_string());
    }
    if f.contains(EventFlags::CONTROL) {
        mods.push("control".to_string());
    }
    if f.contains(EventFlags::ALTERNATE) {
        mods.push("option".to_string());
    }
    if f.contains(EventFlags::COMMAND) {
        mods.push("command".to_string());
    }
    if f.contains(EventFlags::CAPS_LOCK) {
        mods.push("capsLock".to_string());
    }
    if f.contains(EventFlags::SECONDARY_FN) {
        mods.push("fn".to_string());
    }
    mods
}

/// 경로 C 실측을 사람이 읽을 한 줄로(§3.3 "효과" 열 예시 `"ToggleCapsLock (off→on)"`
/// 을 뷰어 자료용으로 압축한 형태). 시도하지 않았으면(`path_c_result == 0`) `None`.
fn viewer_path_c(t: &TapTrace) -> Option<String> {
    if t.path_c_result == 0 {
        return None;
    }
    Some(format!(
        "ToggleCapsLock {} {}->{}",
        path_c_result_name(t.path_c_result),
        caps_lock_state_name(t.path_c_before),
        caps_lock_state_name(t.path_c_after),
    ))
}

/// [`TapTrace`](POD) 하나를 [`ViewerRecord`](디코드됨)로 바꾼다. 드레인 스레드
/// 전용 — 탭 스레드는 이 함수를 절대 부르지 않는다.
fn decode_viewer_record(t: &TapTrace, at_ms: u64) -> ViewerRecord {
    let emitted = t.emitted[..t.emitted_len as usize]
        .iter()
        .map(|e| ViewerEmit {
            kind: event_kind_name(e.kind).to_string(),
            keycode: e.keycode,
            flags: e.flags,
            modifiers: modifiers_from_flags(e.flags),
        })
        .collect();
    let effects = t.effects[..t.effects_len as usize]
        .iter()
        .map(|e| effect_name(*e).to_string())
        .collect();

    ViewerRecord {
        seq: t.seq,
        at_ms,
        kind: event_kind_name(t.raw_kind).to_string(),
        keycode: t.raw_keycode,
        flags: t.raw_flags,
        modifiers: modifiers_from_flags(t.raw_flags),
        autorepeat: t.autorepeat,
        resolved_keycode: t.resolved_keycode,
        alias_active: t.alias_active,
        layer: layer_name(t.layer).to_string(),
        rule: rule_identifier(t.rule_kind, t.rule_index),
        disposition: disposition_name(t.disposition).to_string(),
        disposition_flags: t.disposition_flags,
        emitted,
        effects,
        path_c: viewer_path_c(t),
    }
}

/// 뷰어 창이 등록하는 싱크. 드레인 스레드가 디코드한 [`ViewerRecord`] 를 받아
/// 자신의 `Mutex<VecDeque<ViewerRecord>>`(상한 500, §3.5)에 넣는다.
pub type ViewerSink = Arc<dyn Fn(ViewerRecord) + Send + Sync>;

/// ⛔ **드레인 스레드만 읽는다 — 콜백은 절대 읽지 않는다**(§2.1 기각한 대안 항목
/// 참고). `RwLock` 이 콜백 임계 경로에 등장하지 않는 것이 이 파일 전체 설계의
/// 핵심이다.
static VIEWER_SINK: OnceLock<RwLock<Option<ViewerSink>>> = OnceLock::new();

fn viewer_sink_lock() -> &'static RwLock<Option<ViewerSink>> {
    VIEWER_SINK.get_or_init(|| RwLock::new(None))
}

/// 뷰어 창의 열림(`Some`)/닫힘(`None`)과 1:1 로 호출된다(앱 계층, 메인 스레드).
pub fn set_viewer_sink(sink: Option<ViewerSink>) {
    if let Ok(mut guard) = viewer_sink_lock().write() {
        *guard = sink;
    }
}

fn call_viewer_sink(record: ViewerRecord) {
    if let Ok(guard) = viewer_sink_lock().read() {
        if let Some(sink) = guard.as_ref() {
            sink(record);
        }
    }
}

// ============================================================================
// 드레인 스레드
// ============================================================================

const DRAIN_INTERVAL: Duration = Duration::from_millis(200);

/// [`spawn_drain_thread`] 가 돌려주는 손잡이. 정지 플래그(`AtomicBool`)로 드레인
/// 스레드를 끝낸다 — `shutdown()` 이 플래그를 세우고 스레드가 마지막 드레인을
/// 한 번 더 돈 뒤 리턴할 때까지 join 한다.
pub struct TraceDrainHandle {
    stop: Arc<AtomicBool>,
    join: JoinHandle<()>,
}

impl TraceDrainHandle {
    pub fn shutdown(self) {
        self.stop.store(true, Ordering::Release);
        let _ = self.join.join();
    }
}

/// 전용 스레드를 띄워 200ms 마다 링을 비운다. 탭 스레드가 아니므로 로깅·할당
/// 제약이 없다.
///
/// ⭐ F-18 Event Viewer(`docs/spec/event-viewer.md` §2.2) — **항상** 스레드를
/// 띄운다. 예전에는 [`trace_enabled`]가 `false`(환경변수 꺼짐)면 스레드 자체를
/// 만들지 않았지만, 뷰어는 **실행 중에** 켜지므로 그 시점에 드레인 스레드가 이미
/// 있어야 한다. 링이 비어 있으면 200ms 마다 즉시 다시 잠든다 — 초당 5회의 빈
/// 깨어남이고 그 비용은 측정 가능한 수준이 아니다(§2.2 가 기각한 대안: 뷰어를
/// 켤 때 스레드를 만들고 끌 때 join — 수명주기 관리가 늘어나는 데 비해 얻는
/// 것이 작다).
///
/// `None` 은 이제 "계측이 꺼져 있다"가 아니라 스레드 생성 자체가 실패한
/// (`thread::Builder::spawn` 이 OS 자원 부족 등으로 `Err` 를 낸) 드문 경우만 뜻한다.
pub fn spawn_drain_thread(ring: Arc<TraceRing>) -> Option<TraceDrainHandle> {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);

    let join = thread::Builder::new()
        .name("ultrakey-trace-drain".to_string())
        .spawn(move || {
            let mut last_dropped = 0u64;
            while !thread_stop.load(Ordering::Acquire) {
                thread::sleep(DRAIN_INTERVAL);
                drain_once(&ring, &mut last_dropped);
            }
            // 종료 직전 남은 레코드를 마지막으로 한 번 더 회수한다.
            drain_once(&ring, &mut last_dropped);
        })
        .ok()?;

    Some(TraceDrainHandle { stop, join })
}

/// ⭐ F-18 — 환경변수 계측(`log_trace`)과 뷰어 싱크(`ViewerRecord`)는 서로 독립이다
/// (`event-viewer.md` §3.2 — "둘 중 하나라도 켜져 있으면 계측이 돈다"). 둘 다 꺼져
/// 있으면 레코드를 디코드하지 않고 링을 비우기만 한다.
fn drain_once(ring: &TraceRing, last_dropped: &mut u64) {
    let log_on = trace_enabled();
    let has_sink = viewer_sink_lock().read().is_ok_and(|guard| guard.is_some());

    while let Some(t) = ring.pop() {
        if log_on {
            log_trace(&t);
        }
        if has_sink {
            let record = decode_viewer_record(&t, now_unix_ms());
            call_viewer_sink(record);
        }
    }

    let dropped = ring.dropped_count();
    if dropped > *last_dropped {
        tracing::warn!(
            dropped = dropped - *last_dropped,
            total_dropped = dropped,
            "tap trace ring buffer full; dropping records"
        );
        *last_dropped = dropped;
    }
}

fn now_unix_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// `emitted` 를 `[FlagsChanged 0x3B 0x40000]` 형태로 최대 4개까지 찍는다. 드레인
/// 스레드(탭 스레드가 아니다)에서만 부르므로 할당해도 된다.
fn format_emitted(t: &TapTrace) -> String {
    let mut out = String::from("[");
    for (i, e) in t.emitted[..t.emitted_len as usize].iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(&format!(
            "{} 0x{:02X} 0x{:X}",
            event_kind_name(e.kind),
            e.keycode,
            e.flags
        ));
    }
    out.push(']');
    out
}

fn format_effects(t: &TapTrace) -> String {
    let mut out = String::from("[");
    for (i, e) in t.effects[..t.effects_len as usize].iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        out.push_str(effect_name(*e));
    }
    out.push(']');
    out
}

fn log_trace(t: &TapTrace) {
    tracing::info!(
        seq = t.seq,
        raw_kind = %event_kind_name(t.raw_kind),
        raw_keycode = %format_args!("0x{:02X}", t.raw_keycode),
        raw_flags = %format_args!("0x{:08X}", t.raw_flags),
        autorepeat = t.autorepeat,
        resolved = %format_args!("0x{:02X}", t.resolved_keycode),
        alias_active = t.alias_active,
        layer = %layer_name(t.layer),
        disp = %disposition_name(t.disposition),
        disp_flags = %format_args!("0x{:X}", t.disposition_flags),
        emitted = %format_emitted(t),
        effects = %format_effects(t),
        path_c = %format_args!(
            "{} before={} after={}",
            path_c_result_name(t.path_c_result),
            caps_lock_state_name(t.path_c_before),
            caps_lock_state_name(t.path_c_after)
        ),
        "tap trace"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultrakey_core::flags::EventFlags;

    // ── u8 ↔ enum 왕복 ─────────────────────────────────────────────────────

    #[test]
    fn event_kind_round_trips_for_every_variant() {
        let all = [
            EventKind::KeyDown,
            EventKind::KeyUp,
            EventKind::FlagsChanged,
            EventKind::LeftMouseDown,
            EventKind::LeftMouseUp,
            EventKind::RightMouseDown,
            EventKind::RightMouseUp,
            EventKind::OtherMouseDown,
            EventKind::OtherMouseUp,
            EventKind::LeftMouseDragged,
            EventKind::RightMouseDragged,
            EventKind::OtherMouseDragged,
            EventKind::MouseMoved,
            EventKind::ScrollWheel,
            EventKind::TapDisabledByTimeout,
            EventKind::TapDisabledByUserInput,
        ];
        for kind in all {
            let code = event_kind_to_code(kind);
            assert_eq!(event_kind_from_code(code), Some(kind), "kind={kind:?}");
        }
    }

    #[test]
    fn layer_round_trips_for_every_variant() {
        let all = [
            Layer::AppGate,
            Layer::SeekSession,
            Layer::HyperModifier,
            Layer::PresetCombo,
            Layer::SimpleRemap,
            Layer::Passthrough,
            Layer::KoreanInput,
        ];
        for layer in all {
            let code = layer_to_code(layer);
            assert_eq!(layer_from_code(code), Some(layer), "layer={layer:?}");
        }
    }

    #[test]
    fn disposition_round_trips_for_every_variant() {
        let flags = EventFlags(0x40000);
        let all = [
            Disposition::Pass,
            Disposition::PassWithFlags(flags),
            Disposition::Consume,
        ];
        for d in all {
            let code = disposition_to_code(d);
            let recovered = disposition_from_code(code, flags).unwrap();
            assert_eq!(recovered, d, "disposition={d:?}");
        }
    }

    /// ⭐ F-01 배선 — `InputEvent` 는 macOS 가 실제로 보내는 이벤트 모양을 그대로 써야
    /// 한다는 이 저장소의 반복된 교훈(PR #35 "출력이 keycode 0 으로 나감")을 따라
    /// keycode 0 이 아닌 값을 쓴다.
    fn seek_key_placeholder() -> InputEvent {
        InputEvent {
            kind: EventKind::KeyDown,
            keycode: ultrakey_core::keycode::KeyCode::ANSI_A,
            flags: EventFlags::NONE,
            autorepeat: false,
        }
    }

    #[test]
    fn effect_round_trips_for_every_variant() {
        let ch = 'x';
        let seek_key = seek_key_placeholder();
        let all = [
            Effect::ToggleCapsLock,
            Effect::OpenSeek,
            Effect::TypeChar(ch),
            Effect::SeekTriggerDown,
            Effect::SeekTriggerUp(ultrakey_core::flags::EventFlags::NONE),
            Effect::SeekKey(seek_key),
        ];
        for e in all {
            let code = effect_to_code(e);
            let recovered = effect_from_code(code, ch, seek_key).unwrap();
            assert_eq!(recovered, e, "effect={e:?}");
        }
    }

    #[test]
    fn unknown_codes_return_none() {
        assert_eq!(event_kind_from_code(255), None);
        assert_eq!(layer_from_code(255), None);
        assert_eq!(disposition_from_code(255, EventFlags::NONE), None);
        assert_eq!(effect_from_code(255, 'x', seek_key_placeholder()), None);
    }

    // ── 링 버퍼 — TapTrace 전용 래퍼 배선 확인 ────────────────────────────────

    fn sample(seq: u64) -> TapTrace {
        TapTrace {
            seq,
            raw_kind: event_kind_to_code(EventKind::KeyDown),
            raw_keycode: 0x39,
            ..Default::default()
        }
    }

    #[test]
    fn ring_pop_on_empty_is_none() {
        let ring = TraceRing::new();
        assert!(ring.pop().is_none());
    }

    #[test]
    fn ring_push_pop_is_fifo() {
        let ring = TraceRing::new();
        ring.push(sample(1));
        ring.push(sample(2));
        ring.push(sample(3));

        assert_eq!(ring.pop().unwrap().seq, 1);
        assert_eq!(ring.pop().unwrap().seq, 2);
        assert_eq!(ring.pop().unwrap().seq, 3);
        assert!(ring.pop().is_none());
    }

    #[test]
    fn ring_drops_and_counts_when_full() {
        let ring = TraceRing::new();
        for i in 0..TRACE_RING_CAP as u64 {
            ring.push(sample(i));
        }
        assert_eq!(ring.dropped_count(), 0);

        // 가득 찬 상태에서 더 push 하면 조용히 버려지고 카운터만 오른다.
        ring.push(sample(9999));
        ring.push(sample(10000));
        assert_eq!(ring.dropped_count(), 2);

        // 기존 내용은 덮어써지지 않고 그대로 FIFO 순서로 남아 있다.
        assert_eq!(ring.pop().unwrap().seq, 0);
    }

    // ── 활성화 게이트 ──────────────────────────────────────────────────────

    /// `OnceLock` 캐시 특성상 이 프로세스 안에서 `trace_enabled()` 를 한 번이라도
    /// 부르면 이후 값이 고정된다 — 다른 테스트가 먼저 호출해 캐시를 굳혔을 수
    /// 있으므로, 이 테스트는 "호출이 패닉 없이 안정적인 bool 을 돌려준다"만
    /// 검증한다(환경변수 값 자체를 단정하지 않는다).
    #[test]
    fn trace_enabled_is_stable_across_calls() {
        let first = trace_enabled();
        let second = trace_enabled();
        assert_eq!(first, second);
    }

    // ── F-18 Event Viewer 런타임 게이트(§2.1, §3.6) ───────────────────────────
    //
    // ⚠️ 아래 세 테스트는 `trace_scope()` 가 `Off` 라는 전제 위에 있다. `trace_scope()`
    // 도 `trace_enabled()` 와 같은 `OnceLock` 을 쓰므로, 이 테스트 바이너리 안에서
    // `ULTRAKEY_TRACE_TAP` 을 실제로 설정해 실행하지 않는 한 항상 `Off` 로 고정된다
    // (`trace_enabled_is_stable_across_calls` 문서와 같은 전제). 그 전제가 깨지는
    // 실행 환경(환경변수를 설정하고 테스트를 돌리는 경우)에서는 조용히 건너뛴다 —
    // 이 게이트 자체의 로직이 아니라 테스트 격리 문제이기 때문이다.
    //
    // ⚠️ 그리고 `VIEWER_ON` 은 **프로세스 전역 `AtomicBool`** 이다 — 아래 세 테스트가
    // 병렬 실행되면 서로의 `set_viewer_enabled` 호출을 덮어쓴다. `viewer_off_*` 가
    // `set(false)` 뒤 루프로 `!should_trace` 를 검증하는 사이 `viewer_on_*` 의
    // `set(true)` 가 끼어들면, env 가 `Off` 인데도 `should_trace(KeyDown)` 이 true 로
    // 오염되어 실패한다 — CI(병렬 스케줄링이 로컬과 다름)에서 실제로 재현됐다
    // (이슈 #101 PR — panic: kind=KeyDown). 그래서 셋 다 `VIEWER_TEST_LOCK` 으로
    // 직렬화한다. 프로덕션 쪽 `AtomicBool` 은 콜백 임계 경로라 손대지 않는다.
    static VIEWER_TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 수용 기준(§8) — 뷰어 off + 환경변수 off 에서 `should_trace()` 가 **모든**
    /// `EventKind` 에 대해 거짓이다. 이것이 "콜백이 추가로 하는 일은 `AtomicBool`
    /// 로드 한 번뿐" 이라는 §2.1 주장의 직접 증거다.
    #[test]
    fn viewer_off_and_env_off_means_no_trace() {
        let _gate = VIEWER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if trace_scope() != TraceScope::Off {
            return;
        }
        set_viewer_enabled(false);

        let all = [
            EventKind::KeyDown,
            EventKind::KeyUp,
            EventKind::FlagsChanged,
            EventKind::LeftMouseDown,
            EventKind::LeftMouseUp,
            EventKind::RightMouseDown,
            EventKind::RightMouseUp,
            EventKind::OtherMouseDown,
            EventKind::OtherMouseUp,
            EventKind::LeftMouseDragged,
            EventKind::RightMouseDragged,
            EventKind::OtherMouseDragged,
            EventKind::MouseMoved,
            EventKind::ScrollWheel,
            EventKind::TapDisabledByTimeout,
            EventKind::TapDisabledByUserInput,
        ];
        for kind in all {
            assert!(!should_trace(kind), "kind={kind:?}");
        }
    }

    /// §3.6 — 뷰어가 켜지면 키 이벤트만 기록한다(`KeysOnly` 와 같은 판정). 마우스는
    /// 여전히 거짓이어야 한다 — 마우스 이벤트가 파묻는 문제(모듈 문서)를 뷰어도
    /// 피해야 한다.
    #[test]
    fn viewer_on_traces_key_events_only() {
        let _gate = VIEWER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        if trace_scope() != TraceScope::Off {
            return;
        }
        set_viewer_enabled(true);

        assert!(should_trace(EventKind::KeyDown));
        assert!(should_trace(EventKind::KeyUp));
        assert!(should_trace(EventKind::FlagsChanged));
        assert!(!should_trace(EventKind::MouseMoved));
        assert!(!should_trace(EventKind::LeftMouseDown));
        assert!(!should_trace(EventKind::ScrollWheel));

        set_viewer_enabled(false); // 다른 테스트에 영향을 주지 않게 원복한다.
    }

    #[test]
    fn set_viewer_enabled_round_trips() {
        let _gate = VIEWER_TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        set_viewer_enabled(true);
        assert!(viewer_enabled());
        set_viewer_enabled(false);
        assert!(!viewer_enabled());
    }

    // ── F-18 Event Viewer — ViewerRecord 디코드 ───────────────────────────────

    /// §3.4 — `TapTrace::rule_kind`/`rule_index` 가 `rule_identifier` 를 거쳐
    /// `"preset:5"`/`"korean:13"`/`"hyperkey"` 로 디코드된다. 규칙 없음(0)은 `None`.
    #[test]
    fn viewer_record_decodes_rule_identifier() {
        assert_eq!(rule_identifier(0, 0), None);
        assert_eq!(rule_identifier(1, 5), Some("preset:5".to_string()));
        assert_eq!(rule_identifier(2, 13), Some("korean:13".to_string()));
        assert_eq!(rule_identifier(3, 0), Some("hyperkey".to_string()));

        let mut t = sample(1);
        t.rule_kind = 1;
        t.rule_index = 5;
        let record = decode_viewer_record(&t, 0);
        assert_eq!(record.rule, Some("preset:5".to_string()));

        let passthrough = sample(2);
        let record = decode_viewer_record(&passthrough, 0);
        assert_eq!(record.rule, None);
    }

    /// §3.5 — `ViewerRecord` 는 뷰어 프런트엔드(웹뷰)가 소비할 JSON 이라 필드 이름이
    /// camelCase 여야 한다(`#[serde(rename_all = "camelCase")]`).
    #[test]
    fn viewer_record_serializes_to_camel_case_json() {
        let mut t = sample(42);
        t.rule_kind = 1;
        t.rule_index = 5;
        t.raw_flags = EventFlags::SHIFT.0;
        let record = decode_viewer_record(&t, 1_000);

        let json = serde_json::to_value(&record).expect("ViewerRecord 직렬화는 실패하지 않는다");
        let obj = json.as_object().expect("객체여야 한다");

        assert!(obj.contains_key("atMs"), "at_ms 가 camelCase 로 나오지 않았다: {obj:?}");
        assert!(obj.contains_key("resolvedKeycode"));
        assert!(obj.contains_key("aliasActive"));
        assert!(obj.contains_key("dispositionFlags"));
        assert!(obj.contains_key("pathC"));
        assert!(!obj.contains_key("raw_kind"), "snake_case 필드가 남아 있다: {obj:?}");
        assert_eq!(obj.get("rule").and_then(|v| v.as_str()), Some("preset:5"));
        assert_eq!(
            obj.get("modifiers").and_then(|v| v.as_array()).map(|a| a.len()),
            Some(1)
        );
    }
}
