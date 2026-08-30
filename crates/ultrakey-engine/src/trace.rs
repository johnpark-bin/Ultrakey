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
use std::sync::{Arc, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use ultrakey_core::arbitration::{Disposition, Effect, Layer};
use ultrakey_core::event::EventKind;

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

/// 이 이벤트를 기록할 것인가. 콜백 임계 경로에서 불린다 — 캐시된 값 비교뿐이다.
pub fn should_trace(kind: EventKind) -> bool {
    match trace_scope() {
        TraceScope::Off => false,
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
        None => "Unknown",
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
pub fn effect_to_code(e: Effect) -> u8 {
    match e {
        Effect::ToggleCapsLock => 0,
        Effect::OpenSeek => 1,
        Effect::TypeChar(_) => 2,
    }
}

pub fn effect_from_code(code: u8, placeholder_char: char) -> Option<Effect> {
    Some(match code {
        0 => Effect::ToggleCapsLock,
        1 => Effect::OpenSeek,
        2 => Effect::TypeChar(placeholder_char),
        _ => return None,
    })
}

fn effect_name(code: u8) -> &'static str {
    match code {
        0 => "ToggleCapsLock",
        1 => "OpenSeek",
        2 => "TypeChar",
        _ => "Unknown",
    }
}

fn path_c_result_name(code: u8) -> &'static str {
    match code {
        0 => "시도안함",
        1 => "성공",
        2 => "실패",
        _ => "Unknown",
    }
}

fn caps_lock_state_name(code: u8) -> &'static str {
    match code {
        0 => "off",
        1 => "on",
        2 => "읽기실패",
        _ => "Unknown",
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

/// 전용 스레드를 띄워 200ms 마다 링을 비우고 `tracing::info!` 로 한 줄씩 남긴다.
/// 탭 스레드가 아니므로 로깅 제약이 없다. [`trace_enabled`] 가 `false` 면 스레드를
/// 만들지 않는다.
pub fn spawn_drain_thread(ring: Arc<TraceRing>) -> Option<TraceDrainHandle> {
    if !trace_enabled() {
        return None;
    }

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

fn drain_once(ring: &TraceRing, last_dropped: &mut u64) {
    while let Some(t) = ring.pop() {
        log_trace(&t);
    }

    let dropped = ring.dropped_count();
    if dropped > *last_dropped {
        tracing::warn!(
            dropped = dropped - *last_dropped,
            total_dropped = dropped,
            "탭 계측 링 버퍼가 가득 차 레코드가 드롭됐다"
        );
        *last_dropped = dropped;
    }
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
            "{}(전:{} 후:{})",
            path_c_result_name(t.path_c_result),
            caps_lock_state_name(t.path_c_before),
            caps_lock_state_name(t.path_c_after)
        ),
        "탭 계측"
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

    #[test]
    fn effect_round_trips_for_every_variant() {
        let ch = 'x';
        let all = [Effect::ToggleCapsLock, Effect::OpenSeek, Effect::TypeChar(ch)];
        for e in all {
            let code = effect_to_code(e);
            let recovered = effect_from_code(code, ch).unwrap();
            assert_eq!(recovered, e, "effect={e:?}");
        }
    }

    #[test]
    fn unknown_codes_return_none() {
        assert_eq!(event_kind_from_code(255), None);
        assert_eq!(layer_from_code(255), None);
        assert_eq!(disposition_from_code(255, EventFlags::NONE), None);
        assert_eq!(effect_from_code(255, 'x'), None);
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
}
