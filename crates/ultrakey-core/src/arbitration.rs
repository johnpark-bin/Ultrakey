//! ⭐ 이 크레이트의 본체 — `key-remapping-engine.md` §3-b 중재 우선순위 표의 구현.
//!
//! `Arbiter::arbitrate` 는 힙 할당을 하지 않는다. `Outcome` 의 emit/effect 버퍼는 고정 크기
//! 인라인 배열이다(`docs/dev/architecture.md` §2.2 "콜백 안에서 절대 하지 않는 것").
//!
//! 계층 0(앱 게이트, `gate.rs`)과 Secure Input 확인은 호출자(`ultrakey-engine`)가
//! `arbitrate` 를 부르기 **이전에** 이미 걸렀다는 전제다(`docs/dev/architecture.md` §2.3).
//! 이 모듈은 §3-b 의 계층 1~5 만 다룬다.
//!
//! ⭐ M2/F-08 확장 — `docs/dev/architecture.md` §6 이 이 세션에서 확정한 중재 설계
//! (D-1, P1~P12)를 그대로 구현한다. 스스로 재설계하지 않는다 — 그 문서가 정본이다.

use crate::event::{EventKind, InputEvent};
use crate::flags::EventFlags;
use crate::keycode::KeyCode;
use crate::keystate::{KeyStateTable, KoreanLatch};
use crate::korean::{self, KoreanImeState, KoreanTrigger};
use crate::quickpress::{QuickPressConfig, QuickPressEvent, QuickPressState};
use crate::rules::{ComboRule, HoldCondition, ModifierKind, RuleAction, RuleId};
use crate::settings::EngineConfig;
use crate::time::Millis;

/// ⭐ 콜백이 매 이벤트 진입 시 원자값에서 읽어 넘기는 게이트 스냅샷(D-K4).
/// `Arbiter::arbitrate` 가 예전에 받던 `seek_active: bool` 파라미터를 이 구조체가
/// 흡수한다 — 게이트가 하나에서 셋으로 늘어난 것을 시그니처 파라미터 나열이 아니라
/// 값 하나로 표현한다.
///
/// `Default` 가 전부 "무해한" 값(모든 게이트 비활성, IME 상태는 `Unknown`)이므로
/// **기존 테스트는 전부 `GateSnapshot::default()` 로 옮겨도 F-16 을 발화시키지 않는다** —
/// 회귀 표면이 자동으로 좁아진다.
#[derive(Debug, Clone, Copy, Default)]
pub struct GateSnapshot {
    /// 계층 1(Seek 세션 활성, F-01/M3).
    pub seek_active: bool,
    /// ⭐(이슈 #93) — 세션이 **인풋 박스 모드**(검색 언어가 명시적 비영어)인가.
    /// 세션 열림 시점에 래칭된다(`false` = 영어 단일 또는 로케일 폴백).
    /// 켜져 있으면 계층 1 이 [`is_input_box_pass_key`](crate::seek_input_box::
    /// is_input_box_pass_key) 를 만족하는 키를 원본 그대로 통과시켜 웹뷰
    /// `<input>` 이 macOS IME 로 조합하게 한다(Plan §3 D4).
    pub seek_input_box: bool,
    /// ⭐(이슈 #93) — `seek.semicolonCycle`(저장 키 `seek.semicolonCycle`)
    /// 의 현재 값. 인풋 박스 모드에서 `;` 의 통과/소비를 가른다(`seek_input_box.rs`
    /// — 설정 켜짐+무modifier 면 순환, 아니면 통과).
    pub seek_semicolon_cycles: bool,
    /// ⭐(이슈 #93) — `Toggle Seek with shortcut:` 의 물리 keycode(`0` = 미설정).
    /// 인풋 박스 모드에서 문자 키 통과 판정 **앞**에 이 조합을 걸러 세션 재입력
    /// 토글(정확히는 machine 의 `matches_global_shortcut`)이 살아 있게 한다.
    pub seek_shortcut_keycode: u16,
    /// ⭐(이슈 #93) — 같은 단축키의 modifier 비트마스크(`EventFlags` 관례).
    pub seek_shortcut_mods: u64,
    /// ⭐ D-K3 — 한국어 전용 앱 제외 비트(`docs/spec/korean-input.md` §3.5).
    /// `true` 면 F-16 규칙을 **평가하지 않는다.** F-10 전역 게이트(`gate.rs`)와 달리
    /// hyper/meh/bleh 등 계층 2~5 의 다른 규칙에는 영향을 주지 않는다.
    pub korean_app_excluded: bool,
    /// ⭐ D-K2 — 한국어 입력기 활성 판정(`korean.rs`). `Unknown` 이 기본값이자
    /// fail-closed 다: 판정 불가 상태에서 한국어 전용 규칙은 발화하지 않는다.
    pub korean_ime: KoreanImeState,
    /// ⭐ F-06 — 트랙패드 제스처가 hyper 를 요청 중인가(`trackpad-hyper-gesture.md` §3.4).
    /// 트랙패드 리스너가 `Engaged` 로 전이하는 동안만 `true` 다. 물리 키 소스와는
    /// 별개 소스이며, 이 값은 "hyper 요청"만 담당하고 실제 `EventFlags` 합성은
    /// 기존 계층 2 경로(`active_synth_flags` 에 hyper 규칙 flags 를 OR)로 흡수된다.
    /// `Default` 는 `false` — 리스너가 없어도(비공개 API 격하) 기존 동작과 완전히 같다.
    pub trackpad_hyper_active: bool,
    /// ⭐ F-06 — 트랙패드 제스처가 진행 중(프리즈 임계 도달, `trackpad-hyper-gesture.md`
    /// §3.2.1)이어서 **마우스 이동 이벤트를 소비해야 하는가**. 제스처 접촉이 낸 커서 이동이
    /// 일반 커서 이동으로 오인되는 것을 막는 §8 마지막 수용 기준의 구현이다.
    /// ⚠️ 소비 대상은 `MouseMoved` 뿐이다 — 클릭·드래그·휠은 소비하지 않는다.
    pub trackpad_freeze_cursor: bool,
}

/// 이 이벤트를 최종적으로 어떻게 흘려보낼 것인가.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    /// 원본 이벤트를 그대로 통과시킨다.
    Pass,
    /// 원본 이벤트의 flags 만 바꿔서 통과시킨다(원본 keycode/kind 유지).
    PassWithFlags(EventFlags),
    /// 원본 이벤트를 소비(억제)한다. 대신 낼 이벤트가 있다면 `Outcome::emitted()` 에 있다.
    Consume,
}

/// 판정 결과로 새로 합성해 내보낼 이벤트 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SynthEvent {
    pub kind: EventKind,
    pub keycode: KeyCode,
    pub flags: EventFlags,
}

/// core 는 macOS 를 모르므로 경로 C(caps lock 잠금 토글)·Seek 세션 열기·유니코드 문자
/// 입력을 직접 실행할 수 없다 — 호출자(`ultrakey-engine`)가 이 효과를 받아 실제 플랫폼
/// API 로 옮긴다(`docs/dev/architecture.md` §6.4 P9·P11, A-6 #2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    ToggleCapsLock,
    OpenSeek,
    TypeChar(char),
    /// ⭐ F-01 — `Remap key to Seek:` 키가 눌렸다(활성화 경로 2). 세션 모드
    /// (toggle/hold)는 F-01 이 자기 설정으로 판정한다 — core 는 모른다.
    SeekTriggerDown,
    /// ⭐ F-01 — 같은 키가 떼졌다. hold 모드의 "Release the remapped key to click"
    /// 이 이 이벤트로 성립한다(명세 §3.2).
    ///
    /// ⭐ **실린 `EventFlags` 는 키를 뗀 그 순간의 modifier 스냅샷이다.** F-04
    /// (`seek-click-execution.md`) §5 #10 이 "hold 모드 확정 시 클릭 모드 판정에
    /// 쓰이는 modifier 는 리매핑 키를 떼는 그 순간의 상태를 반영한다" 고 요구하는데,
    /// 그 순간을 아는 것은 이 콜백뿐이다 — F-04 가 자기 시점에 다시 조회하면
    /// 사용자는 이미 손을 뗀 뒤다. 그래서 값을 **여기서 떠서** 실어 보낸다.
    ///
    /// ⚠️ 원본 `ev.flags` 를 가공하지 않고 그대로 싣는다. caps lock 트리거라면
    /// `alphaShift` 비트도 함께 온다 — 어떤 비트를 클릭 모드 판정에 쓸지는
    /// F-04 의 판단이지 이 크레이트의 판단이 아니다.
    SeekTriggerUp(EventFlags),
    /// ⭐ F-01 — 세션이 열려 있는 동안 Seek 으로 라우팅되는 키 하나(계층 1).
    /// `kind` 는 **정규화된 값**이다 — `FlagsChanged` 는 이미 down/up 으로 환원돼
    /// 있다(`normalize_kind`). 소비자가 그 환원을 다시 하면 안 된다.
    SeekKey(InputEvent),
}

/// 어느 계층이 이 결과를 결정했는지 — short-circuit 을 테스트에서 검증하기 위한 표식.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    /// 계층 0(문지기). 이 크레이트는 만들지 않는다 — 호출자가 이미 걸렀기 때문(§3-f).
    /// 호출자가 게이트에서 막았을 때 자신의 기록용으로 이 값을 붙일 수 있도록 열어 둔다.
    AppGate,
    SeekSession,
    HyperModifier,
    PresetCombo,
    /// F-16 한국어 입력 지원(`docs/spec/korean-input.md` §3.4) — `PresetCombo` 다음,
    /// `SimpleRemap` 앞. 계층 3 안에서 `RuleId::Preset` 이 `RuleId::Korean` 보다 먼저
    /// 평가되므로(`rules.rs`), F-08.4 같은 Preset 조합이 F-16 규칙보다 항상 이긴다.
    KoreanInput,
    /// F-06 트랙패드 제스처 프리즈(`trackpad-hyper-gesture.md` §3.2.1) — 제스처 진행
    /// 중 마우스 이동 이벤트를 소비하는 자리. 계층 1 앞의 게이트성 소비다.
    TrackpadFreeze,
    SimpleRemap,
    Passthrough,
}

/// ⭐ M2/F-08 — P1 이 한 패스에서 flagsChanged + remap down + 조합 down/up 을 함께 낼 수
/// 있어(`docs/dev/architecture.md` §6.4 A-6 #1) 4에서 8로 올렸다.
const MAX_EMIT: usize = 8;
const MAX_EFFECTS: usize = 4;

/// 중재 결과. 최대 8개 인라인 이벤트 버퍼 + 4개 효과 버퍼 + disposition — 힙 할당이 없다.
#[derive(Debug, Clone, Copy)]
pub struct Outcome {
    buf: [SynthEvent; MAX_EMIT],
    len: usize,
    effects: [Effect; MAX_EFFECTS],
    effect_len: usize,
    disposition: Disposition,
    layer: Layer,
    /// ⭐ F-18 Event Viewer(`docs/spec/event-viewer.md` §3.4) — `layer` 하나로는
    /// `Layer::PresetCombo` 안의 16종 프리셋을 구분할 수 없다. 규칙이 실제로 발화한
    /// 지점에서만 채운다 — 규칙 없이 통과하는 경로(`Layer::Passthrough` 등)는 `None`.
    /// `RuleId` 는 2바이트 POD 라 `Outcome` 의 `Copy`+무할당 성질을 깨지 않는다.
    rule: Option<RuleId>,
}

impl Outcome {
    fn new(layer: Layer, disposition: Disposition) -> Self {
        // 플레이스홀더 값. len/effect_len 범위 밖은 절대 읽지 않으므로 내용 자체는 의미가 없다.
        let placeholder = SynthEvent {
            kind: EventKind::FlagsChanged,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
        };
        Outcome {
            buf: [placeholder; MAX_EMIT],
            len: 0,
            effects: [Effect::OpenSeek; MAX_EFFECTS],
            effect_len: 0,
            disposition,
            layer,
            rule: None,
        }
    }

    fn pass(layer: Layer) -> Self {
        Self::new(layer, Disposition::Pass)
    }

    fn consume(layer: Layer) -> Self {
        Self::new(layer, Disposition::Consume)
    }

    fn push(&mut self, ev: SynthEvent) {
        debug_assert!(self.len < MAX_EMIT, "Outcome 버퍼 초과 — MAX_EMIT 를 늘려야 한다");
        if self.len < MAX_EMIT {
            self.buf[self.len] = ev;
            self.len += 1;
        }
    }

    fn push_effect(&mut self, e: Effect) {
        debug_assert!(self.effect_len < MAX_EFFECTS, "Outcome 효과 버퍼 초과 — MAX_EFFECTS 를 늘려야 한다");
        if self.effect_len < MAX_EFFECTS {
            self.effects[self.effect_len] = e;
            self.effect_len += 1;
        }
    }

    pub fn emitted(&self) -> &[SynthEvent] {
        &self.buf[..self.len]
    }

    pub fn effects(&self) -> &[Effect] {
        &self.effects[..self.effect_len]
    }

    pub fn disposition(&self) -> Disposition {
        self.disposition
    }

    pub fn layer(&self) -> Layer {
        self.layer
    }

    /// 어느 규칙이 이 결과를 발화시켰는가(§3.4). 규칙 없이 통과한 이벤트는 `None`.
    pub fn rule(&self) -> Option<RuleId> {
        self.rule
    }
}

/// ⭐ D-1 caps lock 모멘터리 정규화(architecture.md §6.1). `cfg.caps_lock_alias` 가
/// 설정돼 있고 이 keycode 가 그 alias(보통 F18, 경로 B `hidutil` 매핑의 대상)와
/// 일치하면 `KeyCode::CAPS_LOCK` 을 반환한다.
///
/// ⚠️ 이렇게 되돌린 keycode 는 **판정**(눌림 테이블 갱신, modifier/조합/quick press
/// 매칭)에만 쓰인다. 이후 로직이 방출하는 합성 이벤트의 keycode 는 각 규칙이 스스로
/// 정한다 — 예를 들어 조합·hold_remap 이 합성하는 이벤트의 keycode 는 대상 키(target
/// key) 자신이지 caps lock 이 아니다. 유일하게 caps lock 자신의 keycode 로 나가는
/// 것은 hyper/meh/bleh modifier 합성 `FlagsChanged`(`rule.source == KeyCode::
/// CAPS_LOCK`)뿐이며, 이는 D-1 이전에도 이미 그랬던 동작이라 alias 유무와 무관하게
/// 일관적이다.
///
/// 모듈 수준 자유 함수로 뽑아 둔 이유 — 이슈 #19 진단 계측(`ultrakey-engine::trace`)이
/// 판정과 **같은 함수**로 resolved keycode 를 계산해야 한다(아래
/// [`resolve_caps_lock_alias_for_trace`]). `Arbiter::resolve_caps_lock_alias` 안에
/// 로직을 묻어 두면 계측이 자기 사본을 새로 만들게 되고, 그러면 계측이 실제 판정과
/// 다른 답을 낼 수 있다 — 계측이 거짓말을 하게 된다.
fn resolve_caps_lock_alias_keycode(cfg: &EngineConfig, keycode: KeyCode) -> KeyCode {
    match cfg.caps_lock_alias {
        Some(alias) if keycode == alias => KeyCode::CAPS_LOCK,
        _ => keycode,
    }
}

/// ⭐ 계측 전용 공개 헬퍼(이슈 #19 물리 caps lock 버그 진단, `ultrakey-engine::trace`).
/// [`resolve_caps_lock_alias_keycode`] 를 그대로 노출할 뿐 새 로직을 만들지 않는다 —
/// 이 함수가 판정이 실제로 쓰는 것과 다른 값을 내면 계측 자체가 거짓말이 된다.
pub fn resolve_caps_lock_alias_for_trace(cfg: &EngineConfig, keycode: KeyCode) -> KeyCode {
    resolve_caps_lock_alias_keycode(cfg, keycode)
}

/// 중재 엔진 하나의 인스턴스. 탭 스레드가 배타 소유한다(`docs/dev/architecture.md` §2.2).
pub struct Arbiter {
    pub state: KeyStateTable,
}

impl Arbiter {
    pub fn new(cfg: &EngineConfig) -> Self {
        let mut state = KeyStateTable::new();
        state.register_sources(&cfg.rules);
        Arbiter { state }
    }

    /// 규칙/설정이 바뀔 때 호출 — 추적 슬롯 구성을 다시 계산한다. 기존 활성 상태는
    /// 가능한 한 보존된다(`KeyStateTable::register_sources` 참고).
    pub fn reconfigure(&mut self, cfg: &EngineConfig) {
        self.state.register_sources(&cfg.rules);
    }

    /// `key` 하나에 대한 quick press 판정 설정. M1 은 규칙 전체에 하나의 설정을 썼지만,
    /// M2 는 키마다 `has_quick_press_action`/`has_double_tap_action` 이 다를 수 있어
    /// (예: caps lock 은 quick press 만, shift 는 double tap 만) 키 단위로 조회한다.
    fn quick_press_config_for(&self, cfg: &EngineConfig, key: KeyCode) -> QuickPressConfig {
        QuickPressConfig {
            quick_press_duration: Millis(cfg.timings.quick_press_duration_ms),
            double_tap_interval: Millis(cfg.timings.double_tap_interval_ms),
            has_quick_press_action: self.state.has_quick_press(key),
            has_double_tap_action: self.state.has_double_tap(key),
        }
    }

    fn resolve_caps_lock_alias(cfg: &EngineConfig, ev: &InputEvent) -> InputEvent {
        InputEvent {
            keycode: resolve_caps_lock_alias_keycode(cfg, ev.keycode),
            ..*ev
        }
    }

    /// §3-b 계층 1~5. 계층 0(앱 게이트)·Secure Input 은 호출자가 이미 걸렀다.
    ///
    /// ⚠️ **순서가 버그 수정이다(2026-08-30, F-01 배선).** 예전 코드는 `gates.seek_active`
    /// 를 함수 맨 앞, D-1 alias 되돌리기·`normalize_kind`·눌림 테이블 갱신보다도 앞에서
    /// 검사했다. `normalize_kind` 는 `FlagsChanged` 를 눌림 테이블로 down/up 으로
    /// 환원하는데, 세션이 열린 동안 그 테이블을 갱신하지 않으면 (1) caps lock 을 떼는
    /// `FlagsChanged` 를 "릴리즈"로 볼 수 없고 (2) 세션이 닫힌 뒤에도 테이블이 어긋난
    /// 채 남아 down/up 판정이 통째로 뒤집힌다 — hold 모드가 원리적으로 동작하지
    /// 않는다. 그래서 이제 alias 되돌리기 → `normalize_kind` → 눌림 테이블 갱신 →
    /// Seek 트리거 판정 → 계층 1(세션 활성) 순으로 평가한다.
    pub fn arbitrate(
        &mut self,
        cfg: &EngineConfig,
        ev_in: &InputEvent,
        gates: GateSnapshot,
        now: Millis,
    ) -> Outcome {
        // D-1 — 진입 즉시 caps lock alias 를 되돌린다. 이후 모든 판정은 이 사본 기준이다.
        let resolved = Self::resolve_caps_lock_alias(cfg, ev_in);
        let ev = &resolved;

        // 정본 상태 갱신 — 이후 모든 계층이 "이 갱신 이후의" 상태를 공유해서 읽는다.
        // v1.20 예방의 핵심: hyper 판정과 preset 판정이 서로 다른 사본을 보지 않는다.
        // ⭐ `FlagsChanged` 는 여기서 down/up 으로 환원한다 — 그 근거는
        // [`Self::flags_changed_is_press`] 문서에 있다.
        let kind = self.normalize_kind(ev);

        match kind {
            EventKind::KeyDown => self.state.set_pressed(ev.keycode, true),
            EventKind::KeyUp => self.state.set_pressed(ev.keycode, false),
            _ => {}
        }

        // ⭐ F-01 활성화 경로 2 — `Remap key to Seek:`. 계층 1 바로 아래, 계층 2 위다.
        // 이 위치의 근거는 `key-remapping-engine.md` §3-b 의 "Seek 은 다른 어떤
        // 리매핑보다 명백히 상위 의도" 다. caps lock 이 동시에 hyper 소스이거나
        // quick press 대상이어도 Seek 트리거가 결정론적으로 이긴다(§5 #7).
        //
        // ⭐ 세션이 열려 있든 아니든 이 검사가 먼저다 — 그래야 트리거 키의 릴리즈가
        // 항상 `SeekTriggerUp` 으로 올라간다. 계층 1(SeekKey)로 흘려보내면 세션이
        // 열리는 시점과 `seek_session_active` 게시 시점 사이의 경합에서 릴리즈를
        // 통째로 잃을 수 있다.
        if cfg.rules.seek_trigger == Some(ev.keycode)
            && (kind == EventKind::KeyDown || kind == EventKind::KeyUp)
        {
            let mut out = Outcome::consume(Layer::SeekSession);
            match kind {
                // 명세 §5 #10 · §3.3 — 누르고 있는 동안 오는 반복 다운은 새 활성화
                // 시도가 아니라 눌림 유지의 연속이다. 소비만 하고 효과를 내지 않는다.
                EventKind::KeyDown if ev.autorepeat => {}
                EventKind::KeyDown => out.push_effect(Effect::SeekTriggerDown),
                _ => out.push_effect(Effect::SeekTriggerUp(ev.flags)),
            }
            return out;
        }

        // 계층 1: Seek 세션 활성(§3-b). 키 이벤트는 전부 소비해 Seek 으로 라우팅한다 —
        // "세션이 열려 있는 동안 타이핑한 문자는 하위 앱에 도달하지 않는다"(§8).
        //
        // ⭐ 마우스 이벤트는 통과시킨다. 명세 §3.1 이 소비 대상으로 지목한 것은 **키**
        // 이벤트뿐이고, 마우스까지 삼키면 세션이 열린 동안 포인터가 통째로 멈춘다 —
        // 그리고 F-04 가 합성할 클릭 자체를 우리가 다시 삼키게 된다.
        if gates.seek_active {
            if !ev.kind.is_key() {
                return Outcome::pass(Layer::SeekSession);
            }
            // ⭐ 이슈 #76 — 한/영 키(`0x68` = `JIS_KANA`)만 예외로 원본 그대로 통과시킨다
            // (`seek-activation-and-session.md` §3.1 한/영 키 예외). 입력 소스 전환을
            // macOS 기본 동작에 맡기기 위해서다. KeyDown/KeyUp/FlagsChanged 어느 모양으로
            // 도착해도, modifier 동반 여부와 무관하게 통과시킨다. 이 예외는 F-16.2(`한/영
            // 키로 입력 소스 변경`) 설정·앱 제외 게이트와 무관하다(D5) — F-16.2 는 계층
            // 3 에 있어 세션 중에는 계층 1 이 먼저다. 한자 키(`0x66` = `JIS_EISU`)는
            // 예외가 아니다 — 계속 소비한다(D4).
            //
            // ⚠️ 분기 위치는 `ev.kind.is_key()` 통과 판정 **뒤**에 둔다 — 한/영 예외와
            // 마우스 통과가 겹치지 않게. 나머지 키 소비는 그대로 유지한다.
            if ev.keycode == KeyCode::JIS_KANA {
                return Outcome::pass(Layer::SeekSession);
            }
            // ⭐(이슈 #93) — 인풋 박스 모드(다국어 검색 언어)에서 문자·backspace
            // 키(및 ⌥+문자=데드키)를 **원본 그대로 통과**시켜 웹뷰 `<input>` 이
            // macOS IME 로 조합하게 한다. 나머지(Enter·Esc·↑↓·Tab·`;` 순환·⌘⌃
            // 조합)는 계속 소비한다 — 세션 컨트롤(순환·확정·취소)이 웹뷰에
            // 빼앗기지 않고, 다국어 세션 중 우리 앱이 활성이므로 ⌘Q 같은 앱
            // 단축키가 통과되어 프로세스가 종료되지 않게 한다. 한/영 키 예외
            // (위)와 마찬가지로 **원본 이벤트 그대로**(환원된 kind 가 아니라)
            // `Outcome::pass` 한다 — IME 는 KeyDown/KeyUp/FlagsChanged 어느 쪽이든
            // 받아야 조합·상태가 깨지지 않는다(Plan §9 #4).
            if gates.seek_input_box
                && crate::seek_input_box::is_input_box_pass_key(ev, gates.seek_semicolon_cycles)
            {
                // ⭐ 전역 단축키(`Toggle Seek with shortcut:`) 조합은 통과시키지
                // 않는다 — 문자로 새어 들어가면 세션을 단축키로 다시 닫을 수
                // 없다. 아래로 떨어져 Consume → `SeekKey` → machine 의 재입력
                // 판정(`matches_global_shortcut`)을 탄다.
                const SHORTCUT_MOD_MASK: u64 = EventFlags::SHIFT.0
                    | EventFlags::CONTROL.0
                    | EventFlags::ALTERNATE.0
                    | EventFlags::COMMAND.0;
                let is_shortcut = gates.seek_shortcut_keycode != 0
                    && ev.keycode.0 == gates.seek_shortcut_keycode
                    && (ev.flags.0 & SHORTCUT_MOD_MASK)
                        == (gates.seek_shortcut_mods & SHORTCUT_MOD_MASK);
                if !is_shortcut {
                    return Outcome::pass(Layer::SeekSession);
                }
            }
            let mut out = Outcome::consume(Layer::SeekSession);
            out.push_effect(Effect::SeekKey(InputEvent { kind, ..*ev }));
            return out;
        }

        // ⭐ F-06 프리즈(`trackpad-hyper-gesture.md` §3.2.1·§8 마지막 항목) — 트랙패드
        // 제스처가 진행 중(프리즈 임계 도달)이면 마우스 이동 이벤트를 소비한다. 접촉
        // 하나만 있는 상태에서 시스템이 내는 `MouseMoved` 는 사실상 그 접촉이 낸 커서
        // 이동뿐이므로, 이 소비가 다른 입력을 막지 않는다. 클릭·드래그·휠은 소비하지
        // 않는다 — 제스처 중에도 클릭은 의도일 수 있다(§3.4 `Click/Drag` 체크박스 계열).
        if gates.trackpad_freeze_cursor && kind == EventKind::MouseMoved {
            return Outcome::consume(Layer::TrackpadFreeze);
        }

        // 계층 2/3 통합 소스 키 핸들러(architecture.md §6.4 P1) — 추적 키(hyper/meh/bleh
        // 소스 또는 프리셋 액션 소스) 자신의 이벤트는 이 핸들러 하나가 결정한다.
        let is_tracked = cfg.rules.modifier_rule_for(ev.keycode).is_some()
            || cfg.rules.source_actions_for(ev.keycode).is_some();
        if is_tracked && (kind == EventKind::KeyDown || kind == EventKind::KeyUp) {
            return self.handle_tracked_key_event(cfg, ev, kind, gates, now);
        }

        // 이하는 추적 대상이 아닌 키(또는 마우스 이벤트)의 경로다.
        let mut out = Outcome::pass(Layer::Passthrough);

        // P2 — 추적 대상이 아닌 키의 keyDown 은 모든 슬롯에 on_other_key_down 을 돌린다
        // (v1.62 예방의 핵심, §3-c 표 3행). M1 은 이 호출을 한 번도 하지 않았다.
        if kind == EventKind::KeyDown {
            self.dispatch_other_key_down(cfg, &mut out);
        }

        // ⭐ K9(이슈 #73, D-K18) — `modifier 키와 함께 누른 문자 키를 영어 소문자로
        // 입력` 옵션이 켜져 있으면 문자 키의 keyDown 을 먼저 판정한다. 위치: 계층 3
        // Preset **앞** — 근거: 이 옵션이 켜졌다는 것은 "⌥+문자가 한글로 깨진다"는
        // 문제를 겪은 사용자이고, 그 modifier+문자 조합을 프리셋이나 F-16 규칙이 가로
        // 채는 모양은 없다(K9 의 트리거는 문자 키 자체라 preset combo 의 hold 키·F-16
        // 규칙의 트리거 키와 겹치지 않는다). 다만 **command·control·caps lock 조합은
        // 발화하지 않는다**(`korean::lowercase_action_for` 내부) — 앱 단축키(⌘C 등)와
        // 대문자 의도를 보호한다. 그래서 Preset 앞에 두어도 앱 단축키를 삼키지 않는다.
        // 끄면(기본) 아무 일도 하지 않는다 — 기존 동작과 완전히 같다.
        if cfg.korean_modifier_lowercase
            && kind == EventKind::KeyDown
            && !gates.korean_app_excluded
        {
            let pressed = |k: KeyCode| self.state.is_pressed(k);
            if let Some(lower) = korean::lowercase_action_for(ev.keycode, &pressed) {
                out.layer = Layer::KoreanInput;
                out.disposition = Disposition::Consume;
                out.push_effect(Effect::TypeChar(lower));
                return out;
            }
        }

        // 계층 3: Preset 조합. 트리거가 이 키이고 hold 조건이 성립하면 발화.
        if self.evaluate_combo_rules(cfg, ev, kind, gates, &mut out) {
            return out;
        }

        // ⭐ 계층 3 안의 F-16 한국어 입력 규칙(`docs/spec/korean-input.md` §3.4,
        // D-K4·D-K5) — `evaluate_combo_rules` 다음, `evaluate_simple_remap` 앞.
        // Preset 이 먼저 평가되므로 `RuleId::Preset` 소속 규칙(예: F-08.4)이 항상
        // F-16 규칙보다 결정론적으로 이긴다(§3.4 표).
        if self.evaluate_korean_rules(cfg, ev, kind, gates, &mut out) {
            return out;
        }

        // 계층 4: 단순 리매핑.
        if self.evaluate_simple_remap(cfg, ev, &mut out) {
            return out;
        }

        // 계층 2 의 "유지" 효과: hyper/meh/bleh 가 Active 인 동안 다른 키/마우스 이벤트에
        // modifier 를 얹는다(§3-b 계층 2 행, hyperkey.md §3.2 Active 행).
        // ⭐ F-06 — 트랙패드 제스처 소스(`gates.trackpad_hyper_active`)도 같은 자리에서
        // hyper 규칙 flags 를 OR 한다. 두 소스(물리 키 + 트랙패드)가 동시에 활성이어도
        // 비트마스크 OR 이므로 중복·모순이 생기지 않는다(hyperkey.md §3.4 병합).
        let mut active = self.state.active_synth_flags();
        if gates.trackpad_hyper_active {
            active |= Self::trackpad_hyper_flags(cfg);
        }
        if !active.is_empty() {
            if ev.kind.is_key() {
                out.layer = Layer::HyperModifier;
                out.disposition =
                    Disposition::PassWithFlags(Self::strip_caps_lock_bit(cfg, ev.flags | active));
                return out;
            }
            if ev.kind.is_click() || ev.kind.is_drag() || ev.kind.is_move() || ev.kind.is_scroll() {
                out.layer = Layer::HyperModifier;
                if self.mouse_should_apply(cfg, ev.kind) {
                    out.disposition =
                        Disposition::PassWithFlags(Self::strip_caps_lock_bit(cfg, ev.flags | active));
                }
                return out;
            }
        }

        // 계층 5: 통과.
        out
    }

    /// ⭐ caps lock 이 modifier 소스로 배정돼 있으면 잠금 비트(`alphaShift`)를 지운다
    /// (2026-08-30, M2 1차 / 이슈 #13 — 실측으로 발견한 결함).
    ///
    /// **문제**: caps lock 을 hyper 소스로 쓰면 hyper 자체는 정상 동작하지만, 다른 앱이
    /// **caps lock 이 켜진 것으로 인식**한다. 실측(브라우저 프로브): 합성된 이벤트의
    /// `getModifierState("CapsLock")` 이 계속 `true` 였고, 그래서 글자가 대문자로 나갔다.
    ///
    /// **원인은 우리 쪽이다.** 합성·통과 이벤트를 만들 때 `ev.flags` 를 그대로 물려주는데,
    /// 원본 caps lock 이벤트에는 이미 `alphaShift` 비트가 실려 온다. 그 비트가 우리가
    /// 내보내는 모든 이벤트에 그대로 따라붙는다.
    ///
    /// **결정**: caps lock 이 hyper/meh/bleh 소스로 **등록되어 있는 동안**, 이 엔진이
    /// 만지는 이벤트에서 `alphaShift` 를 지운다. 근거 — 그 키를 modifier 소스로 배정한
    /// 순간부터 사용자에게 caps lock 은 **더 이상 잠금 키가 아니다.** 잠금을 토글할 수단이
    /// 없으므로 잠금 비트가 남아 있는 것은 사용자가 되돌릴 수 없는 상태이고, 그것은
    /// `hyperkey.md` §8 이 요구하는 "caps lock 의 실제 잠금이 켜지지 않는다"와도 어긋난다.
    ///
    /// ⚠️ **범위를 좁게 잡았다** — 이 엔진이 **이미 손대는 이벤트**(합성 `flagsChanged`,
    /// 조합이 활성인 동안 flags 를 얹어 통과시키는 이벤트)에만 적용한다. 아무것도 하지
    /// 않고 통과시키는 이벤트까지 건드리려면 매 이벤트에 `CGEventSetFlags` 를 부르게 되어
    /// 임계 경로 비용이 늘고(`architecture.md` §2.2), 얻는 것은 "이미 잠겨 있던 상태"의
    /// 표시뿐이다. 그 경우는 경로 C(`hid_lock::set_caps_lock_state`)가 다룰 몫이다.
    ///
    /// 기각한 대안 — **경로 C 로 잠금을 끄는 것만으로 해결한다**: 이번 실측에서
    /// `ioreg` 의 `HIDCapsLockState` 는 계속 `No` 였다. 즉 **하드웨어 잠금은 애초에
    /// 걸리지 않았고** 이벤트 flags 에만 비트가 실려 있었다 — 경로 C 로는 이 경우를
    /// 고칠 수 없다. 두 층위가 다르다는 것을 실측이 보여준 셈이라 여기 남긴다.
    fn strip_caps_lock_bit(cfg: &EngineConfig, flags: EventFlags) -> EventFlags {
        let caps_is_source = cfg
            .rules
            .modifier_rules
            .iter()
            .any(|r| r.source == KeyCode::CAPS_LOCK);
        if caps_is_source {
            flags & !EventFlags::CAPS_LOCK
        } else {
            flags
        }
    }

    /// ⭐ `FlagsChanged` 를 `KeyDown`/`KeyUp` 으로 환원한다 (2026-08-30, M2 1차 / 이슈 #13).
    ///
    /// **왜 필요한가 — 이것이 없으면 hyper 가 아예 동작하지 않는다.** macOS 는 modifier
    /// 키(caps lock · shift · control · option · command · globe)의 눌림/뗌을
    /// `kCGEventKeyDown`/`KeyUp` 이 아니라 **`kCGEventFlagsChanged` 하나로만** 전달한다.
    /// 그런데 소스 키 팝업 35종 중 F1~F24 를 뺀 **전부가 modifier 키**다 — 즉 이 환원이
    /// 없으면 원본 이벤트가 그대로 통과하고, hyper/meh/bleh 는 **어떤 소스 키로도
    /// 발동하지 않는다.**
    ///
    /// **판정 방법: 우리가 소유한 정본 눌림 테이블을 본다.** 그 keycode 가 지금 눌린
    /// 것으로 기록돼 있지 않으면 이번 `FlagsChanged` 는 누름이고, 눌린 것으로 기록돼
    /// 있으면 뗌이다.
    ///
    /// ⭐ D-1 이후에도 이 함수는 그대로 필요하다 — caps lock 에 의존하는 규칙이 하나도
    /// 없어 `caps_lock_alias` 가 `None` 인 구성(순수 hyper/meh/bleh 만 쓰는 M1 스타일
    /// 설정)에서는 caps lock 이 여전히 물리 `FlagsChanged` 로만 도착하기 때문이다.
    ///
    /// 기각한 대안 — **`ev.flags` 의 비트를 보고 판정한다**: 두 곳에서 깨진다.
    /// (1) 좌/우 shift 는 공개 `CGEventFlags` 상수에서 **같은 비트**(`0x20000`)를 공유해
    /// `left shift` 와 `right shift` 를 구분할 수 없다. (2) caps lock 의 `alphaShift`
    /// 비트는 **키의 눌림이 아니라 잠금(lock) 상태**를 나타낸다 — 눌림 판정에 쓸 수 없다.
    fn normalize_kind(&self, ev: &InputEvent) -> EventKind {
        if ev.kind != EventKind::FlagsChanged {
            return ev.kind;
        }
        if self.state.is_pressed(ev.keycode) {
            EventKind::KeyUp
        } else {
            EventKind::KeyDown
        }
    }

    /// ⭐ M2/F-08 — 계층 2/3 통합 소스 키 핸들러(architecture.md §6.4 P1). 추적 키(hyper/
    /// meh/bleh 소스이거나 프리셋 액션이 등록된 키) 자신의 down/up 은 이 함수 하나가
    /// 결정한다. `kind` 는 항상 `KeyDown`/`KeyUp` 이다 — 호출자(`arbitrate`)가 보장한다.
    fn handle_tracked_key_event(
        &mut self,
        cfg: &EngineConfig,
        ev: &InputEvent,
        kind: EventKind,
        gates: GateSnapshot,
        now: Millis,
    ) -> Outcome {
        // ⭐ 원본을 소비할 것인가, 통과시킬 것인가 (2026-08-30, 이슈 #19 증상 C).
        // 대체 출력이 없는 추적 키(F-08.8/F-08.11 만 켠 shift)는 원본을 그대로
        // 흘려보내야 한다 — 근거는 [`Self::has_substitute_output`] 문서에 있다.
        let base_disposition = if Self::has_substitute_output(cfg, ev.keycode) {
            Disposition::Consume
        } else {
            Disposition::Pass
        };

        match kind {
            EventKind::KeyDown => {
                // P6 — 이 키가 조합의 트리거이기도 하면 조합이 먼저다. FSM 을 건드리기 전에
                // 먼저 확인한다: 성립하면 그것을 발화하고 이 키의 FSM 을 Suppressed 로 만든
                // 뒤 Consume 한다. F-08.10(`Shift + caps lock = caps lock`)이 F-08.2(quick
                // press caps lock)를 무효화하는 것이 이 규칙의 구현이다(R3/v1.62).
                let mut out = Outcome::consume(Layer::PresetCombo);
                if let Some(rule) =
                    self.find_matching_combo(&cfg.rules.combo_rules, ev.keycode, gates.trackpad_hyper_active)
                {
                    self.fire_combo(cfg, &rule, ev, &mut out);
                    return out;
                }

                let source_actions = cfg.rules.source_actions_for(ev.keycode).copied();
                let qp_cfg = self.quick_press_config_for(cfg, ev.keycode);
                let prev = self.state.machine(ev.keycode);
                let (next, event) = prev.on_key_down(now, ev.autorepeat, &qp_cfg);
                self.state.set_machine(ev.keycode, next);

                let mut out = Outcome::new(Layer::HyperModifier, base_disposition);
                match event {
                    Some(QuickPressEvent::HoldStart) => {
                        self.emit_hold_start(cfg, ev.keycode, ev.flags, &mut out);
                    }
                    Some(QuickPressEvent::QuickPress) => {
                        out.layer = Layer::PresetCombo;
                        if let Some(action) = source_actions.and_then(|sa| sa.quick_press) {
                            Self::push_full_action(&mut out, action);
                        }
                    }
                    Some(QuickPressEvent::DoubleTap) => {
                        out.layer = Layer::PresetCombo;
                        if let Some(action) = source_actions.and_then(|sa| sa.double_tap) {
                            Self::push_full_action(&mut out, action);
                        }
                    }
                    // HoldEnd 는 keyDown 에서 나올 수 없다. None(PendingDown 유지 등)은
                    // 원본을 소비만 하고 끝난다.
                    _ => {}
                }
                out
            }
            EventKind::KeyUp => {
                let source_actions = cfg.rules.source_actions_for(ev.keycode).copied();
                let qp_cfg = self.quick_press_config_for(cfg, ev.keycode);
                let prev = self.state.machine(ev.keycode);
                let (next, event) = prev.on_key_up(now, &qp_cfg);
                self.state.set_machine(ev.keycode, next);

                let mut out = Outcome::new(Layer::HyperModifier, base_disposition);
                match event {
                    Some(QuickPressEvent::HoldEnd) => {
                        self.emit_hold_end(cfg, ev.keycode, ev.flags, &mut out);
                    }
                    Some(QuickPressEvent::QuickPress) => {
                        // PendingDown 에서 double tap 액션 없이 짧게 뗀 즉시 quick press.
                        out.layer = Layer::PresetCombo;
                        if let Some(action) = source_actions.and_then(|sa| sa.quick_press) {
                            Self::push_full_action(&mut out, action);
                        }
                    }
                    // WaitingSecondTap 진입(대기 중, 이벤트 없음)과 Suppressed → Idle
                    // (조용히 소비만, R3)이 여기로 들어온다.
                    _ => {}
                }
                out
            }
            _ => unreachable!("handle_tracked_key_event 는 KeyDown/KeyUp 에서만 호출된다"),
        }
    }

    /// P2 — 추적 대상이 아닌 키의 keyDown 도착 시 모든 슬롯에 `on_other_key_down` 을
    /// 돌린다. `HoldStart` 가 나온 슬롯마다 그 효과(modifier 합성/hold_remap 다운)를
    /// `out` 에 밀어 넣는다 — 이 함수가 반환한 뒤 `out` 은 계층 3/4/5 로 그대로 이어진다.
    fn dispatch_other_key_down(&mut self, cfg: &EngineConfig, out: &mut Outcome) {
        for idx in 0..self.state.slot_count() {
            let (key, _rule_flags, state) = self.state.slot_at(idx);
            let (next, event) = state.on_other_key_down();
            if next != state {
                self.state.set_machine(key, next);
            }
            if matches!(event, Some(QuickPressEvent::HoldStart)) {
                self.emit_hold_start(cfg, key, EventFlags::NONE, out);
            }
        }
    }

    /// `key` 가 hold 로 확정됐을 때의 효과를 `out` 에 밀어 넣는다: (a) modifier 소스이면
    /// 합성 `FlagsChanged`, (b) 프리셋 hold_remap 대상이 있으면 그 키의 `KeyDown`.
    /// 둘 다 있을 수 있다(한 키가 동시에 modifier 소스이자 hold_remap 대상인 구성은
    /// 설정 UI 가 충돌로 막지만, 이 함수 자체는 어느 조합이든 안전하게 처리한다).
    fn emit_hold_start(&self, cfg: &EngineConfig, key: KeyCode, base_flags: EventFlags, out: &mut Outcome) {
        let mut substituted = false;

        if let Some(rule) = cfg.rules.modifier_rule_for(key) {
            let flags = Self::strip_caps_lock_bit(cfg, base_flags | self.state.active_synth_flags());
            out.push(SynthEvent {
                kind: EventKind::FlagsChanged,
                keycode: rule.source,
                flags,
            });
            out.layer = Layer::HyperModifier;
            substituted = true;
        }

        if let Some(action) = cfg.rules.source_actions_for(key).and_then(|sa| sa.hold_remap) {
            match Self::modifier_target_of(action) {
                // ⭐ 대상이 modifier 키다 — `FlagsChanged` + 해당 비트로 낸다(이슈 #19 증상 A).
                Some((target, mf)) => {
                    let flags = Self::strip_caps_lock_bit(
                        cfg,
                        base_flags | self.state.active_synth_flags() | mf,
                    );
                    out.push(SynthEvent { kind: EventKind::FlagsChanged, keycode: target, flags });
                }
                None => Self::push_key_down(out, action),
            }
            out.layer = Layer::PresetCombo;
            // `RuleAction::Nothing`(`nothing (disable it)`)도 "대체됨"이다 — 사용자가
            // 그 키를 명시적으로 죽이기로 고른 것이므로 원본을 되살리면 안 된다.
            substituted = true;
        }

        // 대체 출력이 없는 키(F-08.8/F-08.11 만 켠 shift)는 애초에 원본을 소비하지
        // 않았으므로(`has_substitute_output`) 여기서 낼 것이 없다 — hold 확정은 FSM
        // 상태 전이일 뿐이고 사용자에게는 그냥 shift 를 누르고 있는 것이다.
        let _ = substituted;
    }

    /// `key` 의 hold 가 해제됐을 때의 효과 — `emit_hold_start` 의 대응.
    fn emit_hold_end(&self, cfg: &EngineConfig, key: KeyCode, base_flags: EventFlags, out: &mut Outcome) {
        let mut substituted = false;

        if let Some(rule) = cfg.rules.modifier_rule_for(key) {
            // 이 규칙의 flags 는 벗기되, 다른 조합이 여전히 active 라면 그 flags 는
            // 유지한다(공유 비트가 있는 hyper/meh/bleh 조합 중 하나만 놓아도 나머지가
            // 살아 있어야 하므로).
            let remaining = self.state.active_synth_flags();
            let flags = Self::strip_caps_lock_bit(cfg, (base_flags & !rule.flags) | remaining);
            out.push(SynthEvent {
                kind: EventKind::FlagsChanged,
                keycode: rule.source,
                flags,
            });
            out.layer = Layer::HyperModifier;
            substituted = true;
        }

        if let Some(action) = cfg.rules.source_actions_for(key).and_then(|sa| sa.hold_remap) {
            match Self::modifier_target_of(action) {
                Some((target, mf)) => {
                    // 이 규칙의 비트만 벗기되, 여전히 active 인 다른 슬롯이 같은
                    // modifier 를 들고 있으면(예: 두 키가 모두 left control 로 리매핑된
                    // 구성) 그 비트는 살려 둔다 — 위 modifier_rule 분기와 같은 규약이다.
                    let remaining = self.state.active_synth_flags();
                    let flags = Self::strip_caps_lock_bit(cfg, (base_flags & !mf) | remaining);
                    out.push(SynthEvent { kind: EventKind::FlagsChanged, keycode: target, flags });
                }
                None => Self::push_key_up(out, action),
            }
            out.layer = Layer::PresetCombo;
            substituted = true;
        }

        let _ = substituted;
    }

    /// ⭐ 이 액션의 대상이 **modifier 키**이면 `(대상 keycode, 그 키의 눌림 flags)`
    /// (2026-08-30, 이슈 #19 증상 A).
    ///
    /// **왜 필요한가.** macOS 는 modifier 키의 눌림/뗌을 `KeyDown`/`KeyUp` 이 아니라
    /// `kCGEventFlagsChanged` 와 그 이벤트의 flags 비트로만 표현한다 —
    /// `key-remapping-engine.md` §5 #18 이 **입력 쪽에서** 이미 확정한 사실이고,
    /// [`Arbiter::normalize_kind`] 가 그것을 근거로 존재한다. 그런데 **출력 쪽**은 그
    /// 대칭을 지키지 않고 있었다: `Remap caps lock to: left control`(F-08.1)이
    /// `left control` 의 `KeyDown`(flags 0)을 합성해 내보내면, 받는 앱 입장에서는
    /// control 비트가 한 번도 켜진 적이 없으므로 **control 이 눌린 것으로 보지 않는다.**
    /// 사용자 실측의 "키 다운·키 업이 모두 키 업으로 잡힌다"가 이 모양이다.
    ///
    /// 대상이 modifier 가 아니면(`esc`·`tab`·방향키 등) `None` — 그쪽은 종전대로
    /// `KeyDown`/`KeyUp` 이 옳다.
    fn modifier_target_of(action: RuleAction) -> Option<(KeyCode, EventFlags)> {
        match action {
            RuleAction::Key { keycode, .. } => keycode.modifier_flags().map(|f| (keycode, f)),
            _ => None,
        }
    }

    /// ⭐ 이 추적 키의 **원본 이벤트를 대체할 출력이 있는가** (2026-08-30, 이슈 #19 증상 C).
    ///
    /// `key-remapping-engine.md` §3-c 는 소스 키의 원본 keyDown 을 `PendingDown` 동안
    /// **보류**하라고 정한다. 그런데 구현은 그 보류를 **모든** 추적 키에 적용하면서,
    /// 대체 출력이 없는 키에서는 보류한 것을 끝내 내보내지 않았다 — 즉 보류가 아니라
    /// **폐기**였다. 그 결과 `Double tap shift = caps lock`(F-08.8)이나
    /// `Quick press … shift`(F-08.11)를 켜는 순간 **shift 키가 통째로 죽었다**
    /// (`Shift+a` 가 소문자로 나간다). 명세 어디에도 그 프리셋이 shift 를 무력화한다는
    /// 서술이 없다 — `power-user-presets.md` §3.2 F-08.8 행의 "부가 효과" 열은 비어 있다.
    ///
    /// **판정 규칙과 그 근거.** §3-c 가 낙관적 통과를 기각한 이유는 **오직 하나**,
    /// "이미 도착한 keyDown(예: 실제 caps lock 토글)을 되돌릴 방법이 없다" 는 것이다 —
    /// 즉 **되돌릴 수 없는 부작용**이 보류의 유일한 존재 이유다. 그러니 원본을 대체할
    /// 출력도 없고 원본 자체에 부작용도 없는 키(shift)는 보류할 이유가 없다. 같은 문서가
    /// 이미 같은 형태의 예외를 두고 있다 — "소스 키에 quick press 액션이 등록되어 있지
    /// 않으면 keyDown 즉시 `HoldConfirmed` 로 전이한다(보류하지 않는다). 근거: 보류는
    /// 모호성을 해소하기 위한 **비용**이다." 여기서도 같다: 보류가 사는 것이 없다.
    ///
    /// 그래서 **대체 출력이 있는 키만 원본을 소비하고, 없는 키는 원본을 그대로 통과**
    /// 시킨다. 프리셋은 제스처를 **더하는** 것이지 그 키를 **빼앗는** 것이 아니다.
    ///
    /// - hyper/meh/bleh 소스 → 합성 `flagsChanged` 가 원본을 대신한다.
    /// - `hold_remap` 이 있는 키(F-08.1) → 리매핑된 키가 원본을 대신한다.
    ///   `RuleAction::Nothing`(`nothing (disable it)`)도 여기 든다 — 사용자가 그 키를
    ///   명시적으로 죽이기로 고른 것이다.
    /// - **caps lock 은 언제나 여기 든다.** 근거 셋. (1) D-1 이 켜져 있으면 원본은
    ///   물리적으로 `F18` 이라(`architecture.md` §6.1) 통과시켜도 어떤 앱에도 의미가
    ///   없고 쓰레기 이벤트만 늘어난다. (2) caps lock 의 원래 기능은 **잠금 토글**이고,
    ///   그것이 §3-c 가 보류의 근거로 든 바로 그 "되돌릴 수 없는 부작용"이다.
    ///   (3) 사용자가 caps lock 프리셋을 켠 시점에 그 키는 이미 용도가 바뀌었다.
    ///
    /// 기각한 대안 — *보류했다가 확정 시점에 원본의 사본을 합성해 방출한다*: 문면상
    /// "보류" 에 더 가깝지만, (a) 합성 사본은 원본과 동일한 이벤트가 아니고(디바이스
    /// 정보·자동반복 플래그 등이 빠진다), (b) `CGEventTapPostEvent` 로 낸 사본과 그
    /// 뒤에 통과시키는 키 이벤트의 **순서 보장이 명확하지 않다**. 원본을 그대로
    /// 흘려보내면 두 문제가 모두 없다.
    fn has_substitute_output(cfg: &EngineConfig, key: KeyCode) -> bool {
        key == KeyCode::CAPS_LOCK
            || cfg.rules.modifier_rule_for(key).is_some()
            || cfg
                .rules
                .source_actions_for(key)
                .is_some_and(|sa| sa.hold_remap.is_some())
    }

    /// `RuleAction::Key` 하나를 "눌렀다 뗌" 한 쌍으로 방출한다 — 대상이 modifier 면
    /// `FlagsChanged` 쌍으로, 아니면 `KeyDown`/`KeyUp` 쌍으로.
    /// quick press·double tap·조합이 모두 이 함수 하나를 쓴다(모양이 갈라지지 않게).
    fn push_key_action(out: &mut Outcome, keycode: KeyCode, flags: EventFlags) {
        match keycode.modifier_flags() {
            Some(mf) => {
                out.push(SynthEvent { kind: EventKind::FlagsChanged, keycode, flags: flags | mf });
                out.push(SynthEvent { kind: EventKind::FlagsChanged, keycode, flags: flags & !mf });
            }
            None => {
                out.push(SynthEvent { kind: EventKind::KeyDown, keycode, flags });
                out.push(SynthEvent { kind: EventKind::KeyUp, keycode, flags });
            }
        }
    }

    /// hold_remap 의 `KeyDown` 절반. `RuleAction::Key` 가 아니면(`Nothing` 등) 아무것도
    /// 내지 않는다 — "소비만 하고 아무것도 내지 않음"(F-08.1 `nothing (disable it)`).
    fn push_key_down(out: &mut Outcome, action: RuleAction) {
        if let RuleAction::Key { keycode, flags } = action {
            out.push(SynthEvent { kind: EventKind::KeyDown, keycode, flags });
        }
    }

    /// hold_remap 의 `KeyUp` 절반.
    fn push_key_up(out: &mut Outcome, action: RuleAction) {
        if let RuleAction::Key { keycode, flags } = action {
            out.push(SynthEvent { kind: EventKind::KeyUp, keycode, flags });
        }
    }

    /// quick press/double tap/조합 액션의 완결된 발화 — `RuleAction` 의 4가지 변형을
    /// 각자의 방출 형태로 옮긴다(architecture.md §6.4 P9·P11).
    fn push_full_action(out: &mut Outcome, action: RuleAction) {
        match action {
            RuleAction::Key { keycode, flags } => Self::push_key_action(out, keycode, flags),
            RuleAction::Text(c) => out.push_effect(Effect::TypeChar(c)),
            RuleAction::ToggleCapsLock => out.push_effect(Effect::ToggleCapsLock),
            RuleAction::OpenSeek => out.push_effect(Effect::OpenSeek),
            RuleAction::Nothing => {}
        }
    }

    /// 계층 3 — Preset 조합. `kind` 가 `KeyDown` 이 아니면 조합은 트리거되지 않는다
    /// (조합은 트리거 키의 down 에서만 발화한다, §3.1).
    fn evaluate_combo_rules(
        &mut self,
        cfg: &EngineConfig,
        ev: &InputEvent,
        kind: EventKind,
        gates: GateSnapshot,
        out: &mut Outcome,
    ) -> bool {
        if kind != EventKind::KeyDown {
            return false;
        }
        if let Some(rule) =
            self.find_matching_combo(&cfg.rules.combo_rules, ev.keycode, gates.trackpad_hyper_active)
        {
            self.fire_combo(cfg, &rule, ev, out);
            true
        } else {
            false
        }
    }

    /// `rules`(이미 `RuleId` 오름차순으로 정렬돼 주어진다고 전제 — architecture.md §6.4
    /// 서두) 중 트리거가 `key` 이고 hold 조건이 성립하는 첫 규칙을 찾는다.
    /// `rules`(이미 `RuleId` 오름차순으로 정렬돼 주어진다고 전제 — architecture.md §6.4
    /// 서두) 중 트리거가 `key` 이고 hold 조건이 성립하는 첫 규칙을 찾는다.
    fn find_matching_combo(
        &self,
        rules: &[ComboRule],
        key: KeyCode,
        trackpad_hyper: bool,
    ) -> Option<ComboRule> {
        rules
            .iter()
            .copied()
            .find(|r| r.trigger == key && self.hold_condition_met(r.hold, trackpad_hyper))
    }

    /// ⭐ F-06 — 트랙패드 소스가 합성할 hyper 조합 flags. 규칙 테이블의 `ModifierKind::
    /// Hyper` 슬롯의 flags(`include_shift_in_hyper` 가 반영된 값)를 그대로 쓴다 —
    /// 트랙패드 소스가 내는 hyper 는 물리 키 소스의 hyper 와 **항상 같은 비트마스크**다.
    /// hyper 규칙이 없으면(하이퍼 슬롯이 꺼져 있으면) `NONE` — fail-closed 다.
    fn trackpad_hyper_flags(cfg: &EngineConfig) -> EventFlags {
        cfg.rules
            .modifier_rules
            .iter()
            .find(|r| r.kind == ModifierKind::Hyper)
            .map(|r| r.flags)
            .unwrap_or(EventFlags::NONE)
    }

    /// P3 — 정본 눌림 테이블(`is_pressed`)·논리 hyper 신호(`is_kind_active`)로만 판정한다.
    /// `ev.flags` 의 modifier 비트로 판정하지 않는다.
    ///
    /// ⭐ F-06 — `HyperActive` 는 트랙패드 제스처 소스도 OR 한다(R4/P8 논리 hyper 신호의
    /// 두 번째 소스). 소스 키가 눌려 있지 않아도 트랙패드 제스처가 hyper 를 들고 있으면
    /// `Hyper + delete = forward delete`(F-08.12) 같은 조합이 발화해야 한다.
    fn hold_condition_met(&self, cond: HoldCondition, trackpad_hyper: bool) -> bool {
        match cond {
            HoldCondition::Key(k) => self.state.is_pressed(k),
            HoldCondition::EitherShift => {
                self.state.is_pressed(KeyCode::LEFT_SHIFT) || self.state.is_pressed(KeyCode::RIGHT_SHIFT)
            }
            HoldCondition::EitherCommand => {
                self.state.is_pressed(KeyCode::LEFT_COMMAND) || self.state.is_pressed(KeyCode::RIGHT_COMMAND)
            }
            HoldCondition::HyperActive => {
                self.state.is_kind_active(ModifierKind::Hyper) || trackpad_hyper
            }
        }
    }

    /// 조합 하나를 발화한다 — 액션을 `out` 에 반영하고, 트리거 키(그리고 P7 이 요구하는
    /// 경우 hold 키까지)의 FSM 을 `Suppressed` 로 전이시킨다.
    fn fire_combo(&mut self, _cfg: &EngineConfig, rule: &ComboRule, ev: &InputEvent, out: &mut Outcome) {
        out.layer = Layer::PresetCombo;
        out.disposition = Disposition::Consume;
        out.rule = Some(rule.id);

        match rule.action {
            RuleAction::Key { keycode, flags: action_flags } => {
                // P5 — 조합이 낸 출력에는 hyper 합성 flags 와 caps lock 잠금 비트를 얹지
                // 않는다. `Caps lock + W = ▲` 의 의도는 방향키이지 `⌃⌥⌘⇧▲` 가 아니다.
                let base = ev.flags & !self.state.active_synth_flags() & !EventFlags::CAPS_LOCK;
                Self::push_key_action(out, keycode, base | action_flags);
            }
            RuleAction::Text(c) => out.push_effect(Effect::TypeChar(c)),
            RuleAction::ToggleCapsLock => out.push_effect(Effect::ToggleCapsLock),
            RuleAction::OpenSeek => out.push_effect(Effect::OpenSeek),
            RuleAction::Nothing => {}
        }

        // P6 — 트리거가 된 추적 키는 Suppressed 로(등록돼 있지 않으면 조용히 무시된다).
        self.state.set_machine(rule.trigger, QuickPressState::Suppressed);

        // P7 — F-08.9(좌우 shift 동시)는 trigger 도 shift, hold 도 shift 다. 발화 시
        // 좌·우 shift FSM 을 둘 다 Suppressed 로 만들어, "양쪽 shift 를 빠르게 두 번"
        // 제스처가 토글을 두 번 내 서로 상쇄되는 문제(§5 엣지 1)를 원천 차단한다.
        if let HoldCondition::Key(hold_key) = rule.hold {
            let is_shift_pair = matches!(rule.trigger, KeyCode::LEFT_SHIFT | KeyCode::RIGHT_SHIFT)
                && matches!(hold_key, KeyCode::LEFT_SHIFT | KeyCode::RIGHT_SHIFT);
            if is_shift_pair {
                self.state.set_machine(hold_key, QuickPressState::Suppressed);
            }
        }
    }

    /// ⭐ F-16 한국어 입력 규칙(`docs/spec/korean-input.md` §3.1·§3.3, D-K5·D-K6·D-K7).
    ///
    /// `kind` 는 호출자(`arbitrate`)가 이미 [`Self::normalize_kind`] 로 환원한 값이다
    /// — **`ev.kind` 를 직접 보지 않는다**(D-K13). 이 프로젝트는 "실제 macOS 가 보내는
    /// 이벤트 모양"을 가정해 세 번 데였다(M1 의 `FlagsChanged` 누락, PR #23 의 modifier
    /// 출력 문제). 환원된 `kind` 를 쓰면 `lang1`/`lang2` 같은 트리거 키가 `KeyDown` 으로
    /// 오든 `FlagsChanged` 로 오든 같은 경로를 탄다.
    ///
    /// `KeyUp` 은 D-K6 의 "치환 중" 래치 규약을 그대로 따른다 — 조건을 다시 평가하지
    /// 않고, keyDown 이 세운 래치가 정한 keycode/flags 로 `KeyUp` 을 합성한다.
    fn evaluate_korean_rules(
        &mut self,
        cfg: &EngineConfig,
        ev: &InputEvent,
        kind: EventKind,
        gates: GateSnapshot,
        out: &mut Outcome,
    ) -> bool {
        match kind {
            EventKind::KeyUp => {
                let Some(latch) = self.state.korean_latch(ev.keycode) else {
                    return false;
                };
                out.layer = Layer::KoreanInput;
                out.disposition = Disposition::Consume;
                out.rule = Some(latch.id);
                out.push(SynthEvent {
                    kind: EventKind::KeyUp,
                    keycode: latch.out_keycode,
                    flags: latch.out_flags,
                });
                self.state.clear_korean_latch(ev.keycode);
                true
            }
            EventKind::KeyDown => {
                let Some(rule) = cfg
                    .rules
                    .korean_rules
                    .iter()
                    .find(|r| r.trigger_key == ev.keycode && self.korean_trigger_met(r.trigger))
                else {
                    return false;
                };

                // 추가 조건 3가지(D-K5) — 전부 성립해야 발화한다. 하나라도 깨지면 이
                // keyDown 은 통과시키되(원본을 그대로 흘려보낸다), **래치는 건드리지
                // 않는다** — 자동 반복 도중 조건이 깨져도 이미 나간 down 에 대응하는
                // up 이 짝을 잃지 않아야 한다(§5#7, D-K6).
                if gates.korean_app_excluded {
                    return false;
                }
                if rule.requires_korean_ime && gates.korean_ime != KoreanImeState::Active {
                    return false;
                }
                if !self.state.active_synth_flags().is_empty() {
                    return false;
                }

                out.layer = Layer::KoreanInput;
                out.disposition = Disposition::Consume;
                out.rule = Some(rule.id);
                out.push(SynthEvent {
                    kind: EventKind::KeyDown,
                    keycode: rule.out_keycode,
                    flags: rule.out_flags,
                });
                // 같은 trigger_key 항목은 덮어쓴다 — 자동 반복(§5#7) 대비.
                self.state.set_korean_latch(KoreanLatch {
                    trigger_key: rule.trigger_key,
                    out_keycode: rule.out_keycode,
                    out_flags: rule.out_flags,
                    id: rule.id,
                });
                true
            }
            _ => false,
        }
    }

    /// `KoreanTrigger` 성립 조건(D-K5) — 정본 눌림 테이블(`is_pressed`)로만 판정한다.
    /// `ev.flags` 를 보지 않는다(§3.1 — 좌/우 shift 가 같은 비트를 공유해 구분 불가,
    /// architecture.md §6.4 P3). caps lock 은 `korean::MODIFIER_KEYS` 에 포함되므로
    /// "modifier 부재" 판정에서 자동으로 걸러진다.
    fn korean_trigger_met(&self, trigger: KoreanTrigger) -> bool {
        use crate::korean::MODIFIER_KEYS;
        match trigger {
            KoreanTrigger::NoModifier => !MODIFIER_KEYS.iter().any(|k| self.state.is_pressed(*k)),
            KoreanTrigger::ShiftOnly => {
                let shift_pressed = self.state.is_pressed(KeyCode::LEFT_SHIFT)
                    || self.state.is_pressed(KeyCode::RIGHT_SHIFT);
                let other_modifier_pressed = MODIFIER_KEYS.iter().any(|k| {
                    *k != KeyCode::LEFT_SHIFT && *k != KeyCode::RIGHT_SHIFT && self.state.is_pressed(*k)
                });
                shift_pressed && !other_modifier_pressed
            }
        }
    }

    /// 계층 4 — 단순 리매핑.
    fn evaluate_simple_remap(&self, cfg: &EngineConfig, ev: &InputEvent, out: &mut Outcome) -> bool {
        if !(ev.kind == EventKind::KeyDown || ev.kind == EventKind::KeyUp) {
            return false;
        }
        let Some(remap) = cfg.rules.simple_remaps.iter().find(|r| r.from == ev.keycode) else {
            return false;
        };
        out.layer = Layer::SimpleRemap;
        out.disposition = Disposition::Consume;
        out.rule = Some(remap.id);
        out.push(SynthEvent {
            kind: ev.kind,
            keycode: remap.to,
            flags: ev.flags | remap.add_flags,
        });
        true
    }

    fn mouse_should_apply(&self, cfg: &EngineConfig, kind: EventKind) -> bool {
        (kind.is_click() && cfg.mouse_apply.click)
            || (kind.is_drag() && cfg.mouse_apply.drag)
            || (kind.is_move() && cfg.mouse_apply.r#move)
            || (kind.is_scroll() && cfg.mouse_apply.scroll)
    }

    /// quick press 타이머 만료 처리(§3-c). 실제 시계 대신 호출자가 `now` 를 준다.
    pub fn on_tick(&mut self, cfg: &EngineConfig, now: Millis) -> Outcome {
        let mut out = Outcome::pass(Layer::Passthrough);

        for idx in 0..self.state.slot_count() {
            let (key, _rule_flags, state) = self.state.slot_at(idx);
            let qp_cfg = self.quick_press_config_for(cfg, key);
            let (next, event) = state.on_tick(now, &qp_cfg);
            if next != state {
                self.state.set_machine(key, next);
            }
            match event {
                Some(QuickPressEvent::HoldStart) => {
                    self.emit_hold_start(cfg, key, EventFlags::NONE, &mut out);
                }
                Some(QuickPressEvent::QuickPress) => {
                    // ⭐ M2 — `WaitingSecondTap` 타임아웃으로 유예됐던 quick press 를 이제
                    // 실제로 방출한다. M1 은 이 액션 자체가 없어 무시했었다.
                    out.layer = Layer::PresetCombo;
                    if let Some(action) = cfg.rules.source_actions_for(key).and_then(|sa| sa.quick_press) {
                        Self::push_full_action(&mut out, action);
                    }
                }
                _ => {}
            }
        }
        out
    }

    /// 절전/잠금/Secure Input 진입 — stuck modifier 방지(§5 엣지 6·9).
    /// 합성 중이던 modifier/hold_remap 을 전부 off 로 방출한 뒤 내부 상태를 리셋한다.
    pub fn force_reset(&mut self, cfg: &EngineConfig) -> Outcome {
        let mut out = Outcome::pass(Layer::Passthrough);

        for idx in 0..self.state.slot_count() {
            let (key, _rule_flags, state) = self.state.slot_at(idx);
            if matches!(state, QuickPressState::HoldConfirmed) {
                let mut substituted = false;
                if cfg.rules.modifier_rule_for(key).is_some() {
                    out.push(SynthEvent {
                        kind: EventKind::FlagsChanged,
                        keycode: key,
                        flags: EventFlags::NONE,
                    });
                    out.layer = Layer::HyperModifier;
                    substituted = true;
                }
                if let Some(action) = cfg.rules.source_actions_for(key).and_then(|sa| sa.hold_remap) {
                    // ⭐ 대상이 modifier 면 "비어 있는 flags 의 flagsChanged" 로 놓아야
                    // 한다 — `KeyUp` 을 내면 받는 앱은 애초에 눌린 적이 없다고 보므로
                    // stuck modifier 가 그대로 남는다(이슈 #19 증상 A 와 같은 뿌리).
                    match Self::modifier_target_of(action) {
                        Some((target, _mf)) => out.push(SynthEvent {
                            kind: EventKind::FlagsChanged,
                            keycode: target,
                            flags: EventFlags::NONE,
                        }),
                        None => Self::push_key_up(&mut out, action),
                    }
                    out.layer = Layer::PresetCombo;
                    substituted = true;
                }
                // 대체 출력이 없는 키는 원본을 통과시켜 왔으므로(§`has_substitute_output`)
                // 여기서 되돌릴 합성 이벤트가 없다 — 물리 키의 실제 뗌이 그 역할을 한다.
                let _ = substituted;
            }
        }

        self.state.reset_all();
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventKind;
    use crate::korean::{KoreanImeState, KoreanTrigger};
    use crate::rules::{ComboRule, HoldCondition, KoreanRule, ModifierKind, ModifierRule, RuleId, SimpleRemap, SourceKeyActions};

    fn hyper_config() -> EngineConfig {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        cfg
    }

    fn key_down(keycode: KeyCode, flags: EventFlags) -> InputEvent {
        InputEvent {
            kind: EventKind::KeyDown,
            keycode,
            flags,
            autorepeat: false,
        }
    }

    fn key_up(keycode: KeyCode, flags: EventFlags) -> InputEvent {
        InputEvent {
            kind: EventKind::KeyUp,
            keycode,
            flags,
            autorepeat: false,
        }
    }

    /// `Disposition::PassWithFlags` 에서 flags 를 꺼낸다 — 통과 이벤트에 무엇이 얹혔는지
    /// 보는 테스트가 여러 개라 헬퍼로 뽑는다.
    fn passed_flags(out: &Outcome) -> EventFlags {
        match out.disposition() {
            Disposition::PassWithFlags(f) => f,
            other => panic!("flags 가 얹힌 통과가 아니다: {other:?}"),
        }
    }

    /// macOS 가 modifier 키의 눌림/뗌을 실제로 보내는 형태 — `KeyDown`/`KeyUp` 이 아니라
    /// `FlagsChanged` 하나다. `normalize_kind` 회귀 테스트들이 이 헬퍼로 그 형태를 재현한다.
    fn flags_changed(keycode: KeyCode, flags: EventFlags) -> InputEvent {
        InputEvent {
            kind: EventKind::FlagsChanged,
            keycode,
            flags,
            autorepeat: false,
        }
    }

    /// 테스트 #2 — v1.20 재현: caps lock 이 hyper 소스이면서 동시에 계층 3 트리거일 수
    /// 있어야 한다.
    #[test]
    fn v1_20_caps_lock_hold_lets_w_pass_with_hyper_flags() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let caps_down = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(caps_down.layer(), Layer::HyperModifier);
        assert_eq!(caps_down.disposition(), Disposition::Consume);
        assert_eq!(caps_down.emitted().len(), 1);
        assert_eq!(caps_down.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);

        let w_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_W, EventFlags::NONE), GateSnapshot::default(), Millis(10));
        assert_eq!(w_down.layer(), Layer::HyperModifier);
        assert_eq!(w_down.disposition(), Disposition::PassWithFlags(EventFlags::HYPER_WITH_SHIFT));
    }

    /// 테스트 #3 — v1.62 재현: shift 를 누른 채 caps lock keyDown → quick press 머신이
    /// 즉시 HoldConfirmed 가 되어 quick press(짧게 눌렀다 뗀 것으로 오인) 가 절대 발생하지
    /// 않는다.
    #[test]
    fn v1_62_shift_then_caps_lock_never_quick_presses() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let shift_down = arb.arbitrate(&cfg, &key_down(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(0));
        assert_eq!(shift_down.layer(), Layer::Passthrough);

        let caps_down = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::SHIFT), GateSnapshot::default(), Millis(5));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
        assert_eq!(caps_down.disposition(), Disposition::Consume);

        let caps_up = arb.arbitrate(&cfg, &key_up(KeyCode::CAPS_LOCK, EventFlags::SHIFT), GateSnapshot::default(), Millis(20));
        assert_eq!(caps_up.emitted().len(), 1);
        assert_eq!(caps_up.emitted()[0].kind, EventKind::FlagsChanged);
        assert!(!caps_up.emitted()[0].flags.contains(EventFlags::HYPER_WITH_SHIFT));
    }

    /// 테스트 #4 — `Include shift in hyper key` OFF → HYPER_NO_SHIFT 만 합성.
    #[test]
    fn hyper_without_shift_only_synthesizes_three_modifiers() {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_NO_SHIFT,
        });
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(out.emitted()[0].flags, EventFlags::HYPER_NO_SHIFT);
        assert!(!out.emitted()[0].flags.contains(EventFlags::SHIFT));
    }

    /// 테스트 #5 — meh → ⌃⌥⇧, bleh → ⌃⌘⇧(option 은 어떤 경우에도 포함되지 않음, v1.65 회귀
    /// 방지).
    #[test]
    fn meh_and_bleh_synthesize_correct_masks() {
        let mut meh_cfg = EngineConfig::default();
        meh_cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::RIGHT_OPTION,
            kind: ModifierKind::Meh,
            flags: EventFlags::MEH,
        });
        let mut meh_arb = Arbiter::new(&meh_cfg);
        let meh_out = meh_arb.arbitrate(&meh_cfg, &key_down(KeyCode::RIGHT_OPTION, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(meh_out.emitted()[0].flags, EventFlags::MEH);

        let mut bleh_cfg = EngineConfig::default();
        bleh_cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::RIGHT_COMMAND,
            kind: ModifierKind::Bleh,
            flags: EventFlags::BLEH,
        });
        let mut bleh_arb = Arbiter::new(&bleh_cfg);
        let bleh_out = bleh_arb.arbitrate(&bleh_cfg, &key_down(KeyCode::RIGHT_COMMAND, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert!(bleh_out.emitted()[0].flags.contains(EventFlags::BLEH));
        assert!(!bleh_out.emitted()[0].flags.contains(EventFlags::ALTERNATE));
    }

    /// 동시에 활성화된 여러 조합은 OR 로 합산된다.
    #[test]
    fn simultaneous_combos_or_their_flags() {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::RIGHT_OPTION,
            kind: ModifierKind::Meh,
            flags: EventFlags::MEH,
        });
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::RIGHT_COMMAND,
            kind: ModifierKind::Bleh,
            flags: EventFlags::BLEH,
        });
        let mut arb = Arbiter::new(&cfg);
        arb.arbitrate(&cfg, &key_down(KeyCode::RIGHT_OPTION, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let bleh_out = arb.arbitrate(&cfg, &key_down(KeyCode::RIGHT_COMMAND, EventFlags::NONE), GateSnapshot::default(), Millis(1));

        let flags = bleh_out.emitted()[0].flags;
        assert!(flags.contains(EventFlags::MEH));
        assert!(flags.contains(EventFlags::BLEH));
        assert!(flags.contains(EventFlags::ALTERNATE));
    }

    /// 테스트 #6 — auto-repeat keyDown 이 PendingDown 으로 되돌리지 않는다(§5 엣지 13).
    #[test]
    fn autorepeat_does_not_re_emit_hold_start() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let first = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(first.emitted().len(), 1);

        let mut repeat_ev = key_down(KeyCode::CAPS_LOCK, EventFlags::NONE);
        repeat_ev.autorepeat = true;
        let repeat = arb.arbitrate(&cfg, &repeat_ev, GateSnapshot::default(), Millis(50));
        assert_eq!(repeat.disposition(), Disposition::Consume);
        assert_eq!(repeat.emitted().len(), 0, "autorepeat 은 HoldStart 를 다시 내면 안 된다");
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
    }

    /// 테스트 #7 — short-circuit: 상위 계층이 성립하면 하위 계층이 평가되지 않는다.
    #[test]
    fn higher_layer_short_circuits_lower_layer() {
        let mut cfg = hyper_config();
        cfg.rules.simple_remaps.push(SimpleRemap {
            id: RuleId::Preset(13),
            from: KeyCode::CAPS_LOCK,
            to: KeyCode::LEFT_CONTROL,
            add_flags: EventFlags::NONE,
        });
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(out.layer(), Layer::HyperModifier);
        assert_eq!(out.emitted()[0].keycode, KeyCode::CAPS_LOCK);
        assert_ne!(out.emitted()[0].keycode, KeyCode::LEFT_CONTROL);
    }

    /// 계층 1(Seek)이 성립하면 계층 2 도 평가되지 않는다.
    #[test]
    fn seek_layer_short_circuits_everything_below() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot { seek_active: true, ..Default::default() }, Millis(0));
        assert_eq!(out.layer(), Layer::SeekSession);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle);
        // ⭐ F-01 배선 — 계층 2(hyper) 는 평가되지 않으므로 hyper 합성 `flagsChanged` 는
        // 없고, 대신 `Effect::SeekKey` 하나가 실려 나간다(§3.1 "세션이 열려 있는 동안
        // …CGEventTap 을 통해 들어오는 문자 키 이벤트는 Seek 검색 바로만 라우팅").
        assert!(out.emitted().is_empty());
        assert_eq!(out.effects().len(), 1);
        assert!(matches!(
            out.effects()[0],
            Effect::SeekKey(ev) if ev.keycode == KeyCode::CAPS_LOCK && ev.kind == EventKind::KeyDown
        ));
    }

    // ── ⭐ F-01 — Seek 트리거·세션 라우팅 배선(`seek-activation-and-session.md` §3.1~3.3) ──

    /// #1 — `seek_trigger_capslock_flags_changed_emits_down_then_up`.
    ///
    /// caps lock 은 물리적으로 `FlagsChanged` 하나로만 도착한다(§3.2 표, "리매핑 키
    /// 다운"). `KeyDown`/`KeyUp` 으로 흉내 내면 M1 hyper 미발동 사고(이 크레이트 문서
    /// 서두 인용)를 그대로 재현한다 — 그래서 이 테스트가 그 재발 방지 자리다. 같은
    /// keycode 로 `FlagsChanged` 를 두 번 보내면 첫 번째가 `SeekTriggerDown`(§3.2 "리매핑
    /// 키 다운"), 두 번째가 `SeekTriggerUp`(hold 모드 "Release the remapped key to
    /// click", §3.2)이어야 한다.
    #[test]
    fn seek_trigger_capslock_flags_changed_emits_down_then_up() {
        let mut cfg = EngineConfig::default();
        cfg.rules.seek_trigger = Some(KeyCode::CAPS_LOCK);
        let mut arb = Arbiter::new(&cfg);

        let down = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK),
            GateSnapshot::default(),
            Millis(0),
        );
        assert_eq!(down.layer(), Layer::SeekSession);
        assert_eq!(down.disposition(), Disposition::Consume);
        assert_eq!(down.effects(), [Effect::SeekTriggerDown]);

        // ⭐ 릴리즈에 ⌘ 을 얹는다 — F-04 §5 #10 이 요구하는 "리매핑 키를 떼는 그
        // 순간의 modifier 스냅샷" 이 `SeekTriggerUp` 에 실려 나가야 한다. 그 순간을
        // 아는 것은 이 콜백뿐이므로 여기서 뜨지 않으면 F-04 가 영영 알 수 없다.
        let release_flags = EventFlags::CAPS_LOCK | EventFlags::COMMAND;
        let up = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::CAPS_LOCK, release_flags),
            GateSnapshot::default(),
            Millis(50),
        );
        assert_eq!(up.layer(), Layer::SeekSession);
        assert_eq!(up.disposition(), Disposition::Consume);
        assert_eq!(up.effects(), [Effect::SeekTriggerUp(release_flags)]);
    }

    /// #2 — `seek_trigger_momentary_key_emits_down_then_up`. `F13` 같은 모멘터리
    /// F-키는 물리적으로 `KeyDown`/`KeyUp` 쌍으로 도착한다(caps lock 과 달리 modifier
    /// 가 아니다) — `key-remapping-engine.md` §3-a 근거. `Remap key to Seek:` 팝업
    /// 35종에 `F13` 이 포함된다(§4).
    #[test]
    fn seek_trigger_momentary_key_emits_down_then_up() {
        let mut cfg = EngineConfig::default();
        cfg.rules.seek_trigger = Some(KeyCode::F13);
        let mut arb = Arbiter::new(&cfg);

        let down = arb.arbitrate(&cfg, &key_down(KeyCode::F13, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(down.layer(), Layer::SeekSession);
        assert_eq!(down.effects(), [Effect::SeekTriggerDown]);

        // 릴리즈 시점의 modifier 스냅샷(F-04 §5 #10) — ⌥ 를 누른 채 뗀 상황.
        let up = arb.arbitrate(&cfg, &key_up(KeyCode::F13, EventFlags::ALTERNATE), GateSnapshot::default(), Millis(20));
        assert_eq!(up.layer(), Layer::SeekSession);
        assert_eq!(up.effects(), [Effect::SeekTriggerUp(EventFlags::ALTERNATE)]);
    }

    /// #3 — `seek_trigger_autorepeat_down_is_consumed_without_effect`(§5 #10 · §3.3
    /// "같은 경로로 열린 세션에 hold 모드에서 동일 리매핑 키의 반복(autorepeat) 다운
    /// 이벤트가 들어오는 경우, 이는 새 활성화 시도가 아니라 눌림 유지의 연속이므로
    /// 무시한다"). 반복 다운은 소비만 하고 두 번째 `SeekTriggerDown` 을 내면 안 된다.
    #[test]
    fn seek_trigger_autorepeat_down_is_consumed_without_effect() {
        let mut cfg = EngineConfig::default();
        cfg.rules.seek_trigger = Some(KeyCode::F13);
        let mut arb = Arbiter::new(&cfg);

        let first = arb.arbitrate(&cfg, &key_down(KeyCode::F13, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(first.effects(), [Effect::SeekTriggerDown]);

        let repeat = arb.arbitrate(&cfg, &key_down_repeat(KeyCode::F13, EventFlags::NONE), GateSnapshot::default(), Millis(50));
        assert_eq!(repeat.layer(), Layer::SeekSession);
        assert_eq!(repeat.disposition(), Disposition::Consume);
        assert!(repeat.effects().is_empty(), "autorepeat 다운은 SeekTriggerDown 을 다시 내면 안 된다");
    }

    /// #4 — `seek_trigger_beats_hyper_source_on_same_key`. `key-remapping-engine.md`
    /// §3-b "동일 소스 키 중복 배정 방지" 근거 2: "Seek(화면 탐색 전용 모드 진입)은
    /// 다른 어떤 리매핑보다 명백히 상위 의도" — caps lock 이 hyper 소스로도 등록돼
    /// 있어도 hyper 합성 `flagsChanged` 는 나가지 않고 Seek 만 발화한다.
    #[test]
    fn seek_trigger_beats_hyper_source_on_same_key() {
        let mut cfg = hyper_config(); // caps lock 을 hyper 소스로 등록
        cfg.rules.seek_trigger = Some(KeyCode::CAPS_LOCK);
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK),
            GateSnapshot::default(),
            Millis(0),
        );
        assert_eq!(out.layer(), Layer::SeekSession);
        assert_eq!(out.effects(), [Effect::SeekTriggerDown]);
        assert!(out.emitted().is_empty(), "hyper 합성 flagsChanged 가 나가면 안 된다");
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle, "hyper FSM 은 건드리지 않는다");
    }

    /// #5 — `seek_trigger_beats_quick_press_action_on_same_key`. 같은 근거(§3-b) —
    /// caps lock 에 quick press 액션(`ToggleCapsLock`)이 등록돼 있어도 Seek 트리거가
    /// 이겨서 quick press FSM 자체가 전혀 진행되지 않는다.
    #[test]
    fn seek_trigger_beats_quick_press_action_on_same_key() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: Some(RuleAction::ToggleCapsLock),
            double_tap: None,
            hold_remap: None,
        });
        cfg.rules.seek_trigger = Some(KeyCode::CAPS_LOCK);
        let mut arb = Arbiter::new(&cfg);

        let down = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK),
            GateSnapshot::default(),
            Millis(0),
        );
        assert_eq!(down.effects(), [Effect::SeekTriggerDown]);

        let up = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK),
            GateSnapshot::default(),
            Millis(10),
        );
        assert_eq!(up.effects(), [Effect::SeekTriggerUp(EventFlags::CAPS_LOCK)]);
        assert!(!up.effects().contains(&Effect::ToggleCapsLock), "quick press 액션이 발화하면 안 된다");
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle, "quick press FSM 은 건드리지 않는다");
    }

    /// #6 — `seek_trigger_resolves_d1_alias`(D-1, `architecture.md` §6.1). caps lock 이
    /// 경로 B 로 `F18` 에 매핑돼 있으면(`caps_lock_alias = Some(F18)`) 탭에는 물리
    /// `F18` 의 `KeyDown`/`KeyUp` 이 도착한다. `seek_trigger = Some(CAPS_LOCK)` 이면
    /// alias 를 되돌린 뒤 판정해야 하므로, F18 의 `KeyDown` 이 `SeekTriggerDown` 을
    /// 내야 한다.
    #[test]
    fn seek_trigger_resolves_d1_alias() {
        let mut cfg = d1_cfg();
        cfg.rules.seek_trigger = Some(KeyCode::CAPS_LOCK);
        let mut arb = Arbiter::new(&cfg);

        let down = arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(down.layer(), Layer::SeekSession);
        assert_eq!(down.effects(), [Effect::SeekTriggerDown]);
    }

    /// #7 — `session_active_consumes_key_events_and_routes_them`(§8 수용 기준 "세션이
    /// 열려 있는 동안 타이핑한 문자는 Seek 검색 바에만 나타나고… 하위 앱의 텍스트
    /// 필드에는 어떤 문자도 도달하지 않는다").
    #[test]
    fn session_active_consumes_key_events_and_routes_them() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_A, EventFlags::NONE),
            GateSnapshot { seek_active: true, ..Default::default() },
            Millis(0),
        );
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.effects().len(), 1);
        match out.effects()[0] {
            Effect::SeekKey(ev) => {
                assert_eq!(ev.kind, EventKind::KeyDown);
                assert_eq!(ev.keycode, KeyCode::ANSI_A);
            }
            other => panic!("SeekKey 가 아니다: {other:?}"),
        }
    }

    /// #8 — `session_active_normalizes_flags_changed_before_routing`. F-01(소비자)이
    /// `FlagsChanged` → down/up 환원을 다시 하지 않아도 되게 하는 계약(`Effect::SeekKey`
    /// 문서 주석) — 세션 중 modifier 키(예: shift)를 눌렀다 떼도 `SeekKey.kind` 가 이미
    /// `KeyDown`→`KeyUp` 으로 정규화돼 있어야 한다.
    #[test]
    fn session_active_normalizes_flags_changed_before_routing() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot { seek_active: true, ..Default::default() };

        let down = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), gates, Millis(0));
        match down.effects()[0] {
            Effect::SeekKey(ev) => assert_eq!(ev.kind, EventKind::KeyDown),
            other => panic!("SeekKey 가 아니다: {other:?}"),
        }

        let up = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), gates, Millis(10));
        match up.effects()[0] {
            Effect::SeekKey(ev) => assert_eq!(ev.kind, EventKind::KeyUp),
            other => panic!("SeekKey 가 아니다: {other:?}"),
        }
    }

    /// #9 — `session_active_passes_mouse_events_through`. §3.1 이 소비 대상으로 지목한
    /// 것은 **키** 이벤트뿐이다 — 마우스까지 삼키면 세션 중 포인터가 멈추고 F-04 가
    /// 합성할 클릭 자체를 다시 삼키게 된다.
    #[test]
    fn session_active_passes_mouse_events_through() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        let click = InputEvent {
            kind: EventKind::LeftMouseDown,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        let out = arb.arbitrate(&cfg, &click, GateSnapshot { seek_active: true, ..Default::default() }, Millis(0));
        assert_eq!(out.disposition(), Disposition::Pass);
        assert!(out.effects().is_empty());
    }

    /// #10 — ⭐ `session_active_keeps_pressed_table_in_sync` — **회귀 방지**. 예전
    /// 코드는 `gates.seek_active` 를 `normalize_kind`/`set_pressed` 보다 **앞**에서
    /// 검사해 즉시 리턴했다 — 세션이 열려 있는 동안 정본 눌림 테이블이 전혀 갱신되지
    /// 않았다는 뜻이다. 이 테스트는 그 갱신이 세션 여부와 무관하게 항상 일어남을
    /// 직접 확인한다: 세션이 열린 동안 left shift 를 `FlagsChanged` 로 눌렀다(→
    /// `is_pressed` 가 `true`) 뗀 뒤(→ `false`), 세션을 닫고 다시 `FlagsChanged` 를
    /// 보내면 `normalize_kind` 가 그것을 (테이블이 `false` 이므로) `KeyDown` 으로
    /// 환원해야 한다 — `is_pressed` 가 다시 `true` 로 바뀌는 것으로 확인한다.
    #[test]
    fn session_active_keeps_pressed_table_in_sync() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);
        let session_open = GateSnapshot { seek_active: true, ..Default::default() };

        arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), session_open, Millis(0));
        assert!(arb.state.is_pressed(KeyCode::LEFT_SHIFT), "세션 중 down 이 테이블에 반영돼야 한다");

        arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), session_open, Millis(10));
        assert!(!arb.state.is_pressed(KeyCode::LEFT_SHIFT), "세션 중 up 도 테이블에 반영돼야 한다");

        // 세션을 닫는다 — 실제로는 F-01 이 `seek_session_active` 를 false 로 내린다.
        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(20));
        assert!(
            arb.state.is_pressed(KeyCode::LEFT_SHIFT),
            "테이블이 어긋나 있었다면(수정 전 코드) 이 이벤트가 KeyUp 으로 오판돼 false 로 남았을 것이다"
        );
        // 세션이 닫혀 있고 등록된 규칙이 없으므로 정상적으로 통과한다(Seek 이 삼키지 않는다).
        assert_eq!(out.layer(), Layer::Passthrough);
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// #11 — `seek_trigger_up_is_emitted_even_while_session_active`. hold 모드 확정이
    /// 이 계약 위에 서 있다(§3.2 "Selected(hold 모드) | 리매핑 키 업(release) … Confirming
    /// … 'Release the remapped key to click'") — 세션이 이미 열려 있는 상태에서도
    /// 트리거 키 자신의 릴리즈는 계층 1(`SeekKey`)이 아니라 `SeekTriggerUp` 으로
    /// 나가야 한다.
    #[test]
    fn seek_trigger_up_is_emitted_even_while_session_active() {
        let mut cfg = EngineConfig::default();
        cfg.rules.seek_trigger = Some(KeyCode::CAPS_LOCK);
        let mut arb = Arbiter::new(&cfg);

        // 트리거 다운 — 세션이 아직 열리지 않은 시점(F-01 이 이제 막 세션을 연다).
        let down = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK),
            GateSnapshot::default(),
            Millis(0),
        );
        assert_eq!(down.effects(), [Effect::SeekTriggerDown]);

        // 세션이 이제 열려 있다(F-01 이 `seek_session_active` 를 true 로 게시했다) —
        // 그 상태에서 같은 트리거 키를 뗀다.
        let up = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK),
            GateSnapshot { seek_active: true, ..Default::default() },
            Millis(50),
        );
        assert_eq!(up.layer(), Layer::SeekSession);
        assert_eq!(up.disposition(), Disposition::Consume);
        assert_eq!(
            up.effects(),
            [Effect::SeekTriggerUp(EventFlags::CAPS_LOCK)],
            "SeekKey 가 아니라 SeekTriggerUp 이어야 한다"
        );
    }

    // ── ⭐ 이슈 #76 — 세션 중 한/영 키(`0x68`) 원본 통과 (T1~T8) ────────────────────────
    // `docs/plan/issue-76-seek-searchbar-ime.md` §4 의 T1~T8 을 그대로 고정한다.

    /// T1 — 세션 활성 + `JIS_KANA` KeyDown(modifier 없음) → `Pass`. 소비하지 않고
    /// `SeekKey` 효과도 내지 않는다 — 원본 이벤트가 그대로 하위 앱으로 전달된다.
    #[test]
    fn session_active_passes_jis_kana_key_down() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::JIS_KANA, EventFlags::NONE),
            GateSnapshot { seek_active: true, ..Default::default() },
            Millis(0),
        );
        assert_eq!(out.layer(), Layer::SeekSession);
        assert_eq!(out.disposition(), Disposition::Pass);
        assert!(out.emitted().is_empty(), "합성 이벤트를 내면 안 된다");
        assert!(out.effects().is_empty(), "SeekKey 효과를 내면 안 된다");
    }

    /// T2 — 세션 활성 + `JIS_KANA` KeyUp → `Pass`. KeyDown 과 같은 경로 — 한/영 키는
    /// 모멘터리 키라 down/up 이 한 쌍으로 통과되어야 한다(D3).
    #[test]
    fn session_active_passes_jis_kana_key_up() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(
            &cfg,
            &key_up(KeyCode::JIS_KANA, EventFlags::NONE),
            GateSnapshot { seek_active: true, ..Default::default() },
            Millis(0),
        );
        assert_eq!(out.disposition(), Disposition::Pass);
        assert!(out.effects().is_empty());
    }

    /// T3 — 세션 활성 + `JIS_KANA` **FlagsChanged** → `Pass`. korean-input.md §3.2 의
    /// "도착 이벤트 종류를 가정하지 않는다"/"두 모양 모두 재현" 절차 — macOS 가 한/영
    /// 키를 KeyDown 이 아니라 FlagsChanged 로 보내는 환경이어도 원본 그대로 통과한다.
    #[test]
    fn session_active_passes_jis_kana_flags_changed() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::JIS_KANA, EventFlags::NONE),
            GateSnapshot { seek_active: true, ..Default::default() },
            Millis(0),
        );
        assert_eq!(out.disposition(), Disposition::Pass);
        assert!(out.effects().is_empty());
    }

    /// T4 — 회귀 방지: 세션 활성 + `ANSI_A` KeyDown 은 **여전히** `Consume` + `SeekKey`.
    /// 예외는 한/영 키 하나뿐이다.
    #[test]
    fn session_active_still_consumes_other_keys() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_A, EventFlags::NONE),
            GateSnapshot { seek_active: true, ..Default::default() },
            Millis(0),
        );
        assert_eq!(out.layer(), Layer::SeekSession);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.effects().len(), 1);
        match out.effects()[0] {
            Effect::SeekKey(ev) => {
                assert_eq!(ev.kind, EventKind::KeyDown);
                assert_eq!(ev.keycode, KeyCode::ANSI_A);
            }
            other => panic!("SeekKey 가 아니다: {other:?}"),
        }
    }

    /// T5 — 한자 키(`0x66` = `JIS_EISU`)는 **통과하지 않는다** — 계속 `Consume` +
    /// `SeekKey`. 예외는 한/영 키 하나뿐이다(D4).
    #[test]
    fn session_active_consumes_jis_eisu_key() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::JIS_EISU, EventFlags::NONE),
            GateSnapshot { seek_active: true, ..Default::default() },
            Millis(0),
        );
        assert_eq!(out.layer(), Layer::SeekSession);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.effects().len(), 1);
        match out.effects()[0] {
            Effect::SeekKey(ev) => {
                assert_eq!(ev.kind, EventKind::KeyDown);
                assert_eq!(ev.keycode, KeyCode::JIS_EISU);
            }
            other => panic!("SeekKey 가 아니다: {other:?}"),
        }
    }

    /// T6 — 세션 활성 + 마우스 이벤트 → `Pass`. 기존 동작 유지 — 한/영 예외 분기가
    /// `ev.kind.is_key()` 통과 판정 **뒤**에 있어 마우스 통과와 겹치지 않는다.
    #[test]
    fn session_active_still_passes_mouse_events() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        let click = InputEvent {
            kind: EventKind::LeftMouseDown,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        let out = arb.arbitrate(&cfg, &click, GateSnapshot { seek_active: true, ..Default::default() }, Millis(0));
        assert_eq!(out.disposition(), Disposition::Pass);
        assert!(out.effects().is_empty());
    }

    /// T7 — 세션 **비활성** + `JIS_KANA` KeyDown → 기존 경로(D5). F-16.2 가 꺼져 있으면
    /// (기본) 통과, 켜져 있으면 계층 3 발화 — 이 함수는 세션 게이트만 본다.
    #[test]
    fn session_inactive_jis_kana_takes_existing_path() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_KANA, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(out.layer(), Layer::Passthrough, "세션 밖에서는 통과 경로여야 한다");
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// T8 — 세션 활성 + `JIS_KANA` + modifier(⌘) 실림 → `Pass`. 상급 리뷰 확정(D3):
    /// modifier 동반 여부와 무관하게 통과한다(§5.1 명세 문구).
    #[test]
    fn session_active_passes_jis_kana_with_modifier() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::JIS_KANA, EventFlags::COMMAND),
            GateSnapshot { seek_active: true, ..Default::default() },
            Millis(0),
        );
        assert_eq!(out.disposition(), Disposition::Pass);
        assert!(out.effects().is_empty());

        // FlagsChanged 모양 + shift modifier 로도 동일한지 함께 고정한다.
        let fc_out = arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::JIS_KANA, EventFlags::SHIFT),
            GateSnapshot { seek_active: true, ..Default::default() },
            Millis(10),
        );
        assert_eq!(fc_out.disposition(), Disposition::Pass);
        assert!(fc_out.effects().is_empty());
    }

    // ── ⭐(이슈 #93) — 인풋 박스 모드의 계층 1 문자 통과 (Plan §6 T4~T9·T7-b·T7-c) ──

    fn input_box_gates(semicolon_cycles: bool) -> GateSnapshot {
        GateSnapshot {
            seek_active: true,
            seek_input_box: true,
            seek_semicolon_cycles: semicolon_cycles,
            ..Default::default()
        }
    }

    /// 세션 비활성이면 인풋 박스 플래그를 켜도 통과 분기가 평가되지 않는다(회귀 없음).
    #[test]
    fn inactive_session_ignores_input_box_flag() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);
        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_A, EventFlags::NONE),
            GateSnapshot {
                seek_input_box: true,
                ..Default::default()
            },
            Millis(0),
        );
        assert_eq!(out.layer(), Layer::Passthrough);
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// T4 — 인풋 박스 모드 + 문자 키(modifier 없음) → **Pass**(웹뷰 `<input>` 이
    /// IME 로 처리).
    #[test]
    fn input_box_passes_plain_letter() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);
        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_A, EventFlags::NONE),
            input_box_gates(false),
            Millis(0),
        );
        assert_eq!(out.layer(), Layer::SeekSession);
        assert_eq!(out.disposition(), Disposition::Pass);
        assert!(out.effects().is_empty(), "SeekKey 를 내면 안 된다");
    }

    /// T5 — 인풋 박스 모드 + backspace(`DELETE`) → **Pass**(입력 요소가 조합 포함
    /// 처리).
    #[test]
    fn input_box_passes_backspace() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);
        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::DELETE, EventFlags::NONE),
            input_box_gates(false),
            Millis(0),
        );
        assert_eq!(out.disposition(), Disposition::Pass);
        assert!(out.effects().is_empty());
    }

    /// T6 — 인풋 박스 모드 + `RETURN` → **여전히 Consume + SeekKey**(세션 컨트롤).
    #[test]
    fn input_box_still_consumes_control_keys() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);
        for (kc, _label) in [
            (KeyCode::RETURN, "Enter"),
            (KeyCode::ESCAPE, "Esc"),
            (KeyCode::DOWN_ARROW, "Down"),
            (KeyCode::UP_ARROW, "Up"),
            (KeyCode::TAB, "Tab"),
        ] {
            let out = arb.arbitrate(
                &cfg,
                &key_down(kc, EventFlags::NONE),
                input_box_gates(false),
                Millis(0),
            );
            assert_eq!(out.disposition(), Disposition::Consume, "{_label}");
            assert_eq!(out.effects().len(), 1, "{_label}");
            assert!(matches!(out.effects()[0], Effect::SeekKey(_)), "{_label}");
        }
    }

    /// T7 — 인풋 박스 모드 + `⌘A`·`⌃A` → **여전히 Consume**(⌘⌃ 조합 보호 —
    /// 다국어 세션 중 우리 앱이 활성이므로 ⌘Q 로 프로세스가 종료되는 것을 막는다).
    #[test]
    fn input_box_consumes_command_and_control_combos() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);
        for flags in [EventFlags::COMMAND, EventFlags::CONTROL] {
            let out = arb.arbitrate(
                &cfg,
                &key_down(KeyCode::ANSI_A, flags),
                input_box_gates(false),
                Millis(0),
            );
            assert_eq!(out.disposition(), Disposition::Consume, "{flags:?}");
            assert_eq!(out.effects().len(), 1);
        }
    }

    /// T7-b — 인풋 박스 모드 + `⌥E`(option, 데드키) → **Pass**(스페인어 악센트
    /// 입력에 필요한 ⌥+문자의 네이티브 의미론 보존).
    #[test]
    fn input_box_passes_option_letter_dead_key() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);
        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_E, EventFlags::ALTERNATE),
            input_box_gates(false),
            Millis(0),
        );
        assert_eq!(out.disposition(), Disposition::Pass, "⌥+문자는 데드키로 통과");
        assert!(out.effects().is_empty());
    }

    /// T7-c — 인풋 박스 모드 + **설정된 전역 단축키 조합**(예: ⌥Space 트리거) →
    /// **여전히 Consume**(machine 의 재입력 토글 판정을 살린다 — 정확히는
    /// `matches_global_shortcut` 가 닫기로 처리).
    #[test]
    fn input_box_consumes_configured_global_shortcut_combo() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot {
            seek_active: true,
            seek_input_box: true,
            seek_semicolon_cycles: false,
            seek_shortcut_keycode: KeyCode::SPACE.0,
            seek_shortcut_mods: EventFlags::ALTERNATE.0,
            ..Default::default()
        };
        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::SPACE, EventFlags::ALTERNATE),
            gates,
            Millis(0),
        );
        assert_eq!(out.disposition(), Disposition::Consume, "단축키 조합은 통과 금지");
        assert_eq!(out.effects().len(), 1);

        // 같은 ⌥ 조합이지만 **다른** keycode 는 통과한다(문자).
        let other = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_B, EventFlags::ALTERNATE),
            gates,
            Millis(10),
        );
        assert_eq!(other.disposition(), Disposition::Pass);
    }

    /// ⭐ `;` — 인풋 박스 모드에서 설정 켬(= 순환 소비) vs 끔(= 검색어 통과).
    #[test]
    fn input_box_semicolon_follows_cycle_setting() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        // 켬: `;` 는 다음 매치 순환 키로 Consume.
        let cycle = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_SEMICOLON, EventFlags::NONE),
            input_box_gates(true),
            Millis(0),
        );
        assert_eq!(cycle.disposition(), Disposition::Consume);
        assert_eq!(cycle.effects().len(), 1);

        // 끔: `;` 는 그냥 문자로 통과.
        let text = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_SEMICOLON, EventFlags::NONE),
            input_box_gates(false),
            Millis(10),
        );
        assert_eq!(text.disposition(), Disposition::Pass);
        assert!(text.effects().is_empty());
    }

    /// T8 — 세션 활성 + 인풋 박스 **꺼짐**(영어 단일·`"en"` 강제) + `ANSI_A` →
    /// **여전히 Consume + SeekKey** — 설정 분기가 영어 단일 동작을 바꾸지 않는다.
    #[test]
    fn english_only_still_consumes_letters() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);
        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_A, EventFlags::NONE),
            GateSnapshot { seek_active: true, ..Default::default() },
            Millis(0),
        );
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.effects().len(), 1);
        assert!(matches!(out.effects()[0], Effect::SeekKey(_)));
    }

    /// 테스트 #8 — force_reset 이 합성 중이던 modifier 에 대해 off flagsChanged 를 방출.
    #[test]
    fn force_reset_emits_off_for_active_modifiers() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);
        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert!(!arb.state.active_synth_flags().is_empty());

        let out = arb.force_reset(&cfg);
        assert_eq!(out.emitted().len(), 1);
        assert_eq!(out.emitted()[0].keycode, KeyCode::CAPS_LOCK);
        assert_eq!(out.emitted()[0].flags, EventFlags::NONE);

        assert!(arb.state.active_synth_flags().is_empty());
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle);
    }

    /// 테스트 #9 — MouseApply: click:true, drag:false 일 때 클릭엔 flags 가 얹히고
    /// 드래그엔 얹히지 않는다.
    #[test]
    fn mouse_apply_respects_per_kind_toggles() {
        let cfg = hyper_config(); // 기본 MouseApply: click=true, drag=false
        let mut arb = Arbiter::new(&cfg);
        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));

        let click_ev = InputEvent {
            kind: EventKind::LeftMouseDown,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        let click_out = arb.arbitrate(&cfg, &click_ev, GateSnapshot::default(), Millis(10));
        assert_eq!(click_out.disposition(), Disposition::PassWithFlags(EventFlags::HYPER_WITH_SHIFT));

        let drag_ev = InputEvent {
            kind: EventKind::LeftMouseDragged,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        let drag_out = arb.arbitrate(&cfg, &drag_ev, GateSnapshot::default(), Millis(11));
        assert_eq!(drag_out.disposition(), Disposition::Pass);
    }

    /// 테스트 #11 — quick press 액션이 등록되지 않은 소스 키는 keyDown 즉시 HoldConfirmed.
    #[test]
    fn modifier_source_without_quick_press_action_confirms_immediately() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle);

        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
    }

    #[test]
    fn on_tick_is_noop_when_nothing_pending() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);
        let out = arb.on_tick(&cfg, Millis(100));
        assert_eq!(out.emitted().len(), 0);
        assert_eq!(out.layer(), Layer::Passthrough);
    }

    // ── `normalize_kind` 회귀 테스트 (2026-08-30, M2 1차 / 이슈 #13) ──────────────────────

    #[test]
    fn flags_changed_first_occurrence_normalizes_to_key_down() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted().len(), 1);
        assert_eq!(out.emitted()[0].kind, EventKind::FlagsChanged);
        assert_eq!(out.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);
    }

    #[test]
    fn flags_changed_second_occurrence_normalizes_to_key_up() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(10));

        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted().len(), 1);
        assert_eq!(out.emitted()[0].kind, EventKind::FlagsChanged);
        assert!(!out.emitted()[0].flags.contains(EventFlags::HYPER_WITH_SHIFT));
    }

    #[test]
    fn flags_changed_hold_lets_other_key_down_carry_all_four_hyper_flags() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let caps = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(caps.disposition(), Disposition::Consume);

        let a_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), GateSnapshot::default(), Millis(10));
        assert_eq!(a_down.layer(), Layer::HyperModifier);
        assert_eq!(a_down.disposition(), Disposition::PassWithFlags(EventFlags::HYPER_WITH_SHIFT));
        if let Disposition::PassWithFlags(flags) = a_down.disposition() {
            assert!(flags.contains(EventFlags::CONTROL));
            assert!(flags.contains(EventFlags::ALTERNATE));
            assert!(flags.contains(EventFlags::COMMAND));
            assert!(flags.contains(EventFlags::SHIFT));
        }
    }

    #[test]
    fn flags_changed_hold_without_shift_carries_three_modifiers_only() {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_NO_SHIFT,
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let a_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), GateSnapshot::default(), Millis(10));

        assert_eq!(a_down.disposition(), Disposition::PassWithFlags(EventFlags::HYPER_NO_SHIFT));
        if let Disposition::PassWithFlags(flags) = a_down.disposition() {
            assert!(flags.contains(EventFlags::CONTROL));
            assert!(flags.contains(EventFlags::ALTERNATE));
            assert!(flags.contains(EventFlags::COMMAND));
            assert!(!flags.contains(EventFlags::SHIFT));
        }
    }

    #[test]
    fn flags_changed_updates_canonical_pressed_table() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);
        assert!(!arb.state.is_pressed(KeyCode::CAPS_LOCK));

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert!(arb.state.is_pressed(KeyCode::CAPS_LOCK), "첫 FlagsChanged 이후 정본 눌림 테이블은 눌림으로 기록돼야 한다");

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(10));
        assert!(!arb.state.is_pressed(KeyCode::CAPS_LOCK), "두 번째 FlagsChanged 이후에는 뗌으로 기록돼야 한다");
    }

    #[test]
    fn flags_changed_distinguishes_left_and_right_shift_by_keycode() {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::RIGHT_SHIFT,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(0));
        assert_eq!(out.disposition(), Disposition::Pass);
        assert_eq!(out.layer(), Layer::Passthrough);
        assert!(arb.state.active_synth_flags().is_empty());
    }

    #[test]
    fn f_key_source_still_uses_key_down_key_up_path() {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::F13,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        let mut arb = Arbiter::new(&cfg);

        let f13_down = arb.arbitrate(&cfg, &key_down(KeyCode::F13, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(f13_down.disposition(), Disposition::Consume);
        assert_eq!(f13_down.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);

        let a_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), GateSnapshot::default(), Millis(10));
        assert_eq!(a_down.disposition(), Disposition::PassWithFlags(EventFlags::HYPER_WITH_SHIFT));

        let f13_up = arb.arbitrate(&cfg, &key_up(KeyCode::F13, EventFlags::NONE), GateSnapshot::default(), Millis(20));
        assert_eq!(f13_up.disposition(), Disposition::Consume);
        assert!(!f13_up.emitted()[0].flags.contains(EventFlags::HYPER_WITH_SHIFT));
    }

    #[test]
    fn force_reset_then_flags_changed_normalizes_to_key_down_again() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert!(arb.state.is_pressed(KeyCode::CAPS_LOCK));

        arb.force_reset(&cfg);
        assert!(!arb.state.is_pressed(KeyCode::CAPS_LOCK), "force_reset 은 정본 눌림 테이블도 지운다");

        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(100));
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);
    }

    #[test]
    fn flags_changed_activated_hyper_applies_to_mouse_click_but_not_drag() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);
        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));

        let click_ev = InputEvent {
            kind: EventKind::LeftMouseDown,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        let click_out = arb.arbitrate(&cfg, &click_ev, GateSnapshot::default(), Millis(10));
        assert_eq!(click_out.disposition(), Disposition::PassWithFlags(EventFlags::HYPER_WITH_SHIFT));

        let drag_ev = InputEvent {
            kind: EventKind::LeftMouseDragged,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        let drag_out = arb.arbitrate(&cfg, &drag_ev, GateSnapshot::default(), Millis(11));
        assert_eq!(drag_out.disposition(), Disposition::Pass);
    }

    #[test]
    fn caps_lock_source_strips_alpha_shift_from_synthesized_event() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let ev = flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK);
        let out = arb.arbitrate(&cfg, &ev, GateSnapshot::default(), Millis(0));

        assert_eq!(out.disposition(), Disposition::Consume);
        let synth = out.emitted();
        assert_eq!(synth.len(), 1, "합성 flagsChanged 하나가 나와야 한다");
        assert!(
            (synth[0].flags & EventFlags::CAPS_LOCK) == EventFlags::NONE,
            "합성 이벤트에 alphaShift 가 남아 있다: {:?}",
            synth[0].flags
        );
        assert_eq!(
            synth[0].flags & EventFlags::HYPER_WITH_SHIFT,
            EventFlags::HYPER_WITH_SHIFT,
            "hyper 4비트는 그대로 실려야 한다"
        );
    }

    #[test]
    fn caps_lock_source_strips_alpha_shift_from_passed_through_keys() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK),
            GateSnapshot::default(),
            Millis(0),
        );

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode(0x00), EventFlags::CAPS_LOCK),
            GateSnapshot::default(),
            Millis(10),
        );
        let flags = passed_flags(&out);
        assert!(
            (flags & EventFlags::CAPS_LOCK) == EventFlags::NONE,
            "통과 이벤트에 alphaShift 가 남아 있다: {flags:?}"
        );
        assert_eq!(flags & EventFlags::HYPER_WITH_SHIFT, EventFlags::HYPER_WITH_SHIFT);
    }

    #[test]
    fn alpha_shift_is_preserved_when_caps_lock_is_not_a_source() {
        let mut cfg = EngineConfig::default();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::RIGHT_COMMAND,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(
            &cfg,
            &flags_changed(KeyCode::RIGHT_COMMAND, EventFlags::CAPS_LOCK),
            GateSnapshot::default(),
            Millis(0),
        );

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode(0x00), EventFlags::CAPS_LOCK),
            GateSnapshot::default(),
            Millis(10),
        );
        let flags = passed_flags(&out);
        assert_eq!(
            flags & EventFlags::CAPS_LOCK,
            EventFlags::CAPS_LOCK,
            "사용자가 켜 둔 caps lock 을 지워서는 안 된다"
        );
    }

    // ── M2/F-08 — D-1 caps lock alias ────────────────────────────────────────────────────

    /// D-1 — `caps_lock_alias` 가 설정돼 있으면 그 keycode(F18)의 이벤트가 caps lock 으로
    /// 판정돼야 한다. F18 은 물리적으로 진짜 down/up 쌍을 보내므로(FlagsChanged 아님),
    /// 이 테스트는 KeyDown/KeyUp 을 그대로 쓴다.
    #[test]
    fn caps_lock_alias_resolves_aliased_keycode_to_caps_lock() {
        let mut cfg = hyper_config();
        cfg.caps_lock_alias = Some(KeyCode::F18);
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(out.layer(), Layer::HyperModifier);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
    }

    /// alias 가 없으면(caps_lock_alias: None) F18 은 그냥 평범한 F-키다 — 별도 규칙이
    /// 없으면 아무 영향 없이 통과한다.
    #[test]
    fn no_alias_leaves_f18_untouched() {
        let cfg = hyper_config(); // caps_lock_alias: None (기본값)
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(out.layer(), Layer::Passthrough);
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// 이슈 #19 계측용 공개 헬퍼가 내부 판정과 같은 답을 낸다는 것을 못박는 테스트 —
    /// 이 헬퍼가 흔들리면 `ultrakey-engine::trace` 의 `resolved_keycode` 필드가 실제
    /// 판정과 다른 값을 찍어 계측 자체가 거짓말을 하게 된다.
    #[test]
    fn resolve_caps_lock_alias_for_trace_matches_internal_resolution() {
        let mut cfg = hyper_config();
        cfg.caps_lock_alias = Some(KeyCode::F18);

        assert_eq!(
            resolve_caps_lock_alias_for_trace(&cfg, KeyCode::F18),
            KeyCode::CAPS_LOCK
        );
        // alias 가 아닌 keycode 는 그대로다.
        assert_eq!(
            resolve_caps_lock_alias_for_trace(&cfg, KeyCode::ANSI_A),
            KeyCode::ANSI_A
        );

        cfg.caps_lock_alias = None;
        assert_eq!(
            resolve_caps_lock_alias_for_trace(&cfg, KeyCode::F18),
            KeyCode::F18
        );
    }

    // ── M2/F-08 — 계층 3 조합(ComboRule), R1~R4, P5~P9 ───────────────────────────────────

    /// R4/P8 — `Hyper + delete = forward delete`: hyper 소스가 caps lock 이든 globe 든
    /// 동일하게 발화한다(v1.60 회귀 방지).
    #[test]
    fn r4_hyper_plus_delete_fires_regardless_of_hyper_source() {
        for source in [KeyCode::CAPS_LOCK, KeyCode::FUNCTION] {
            let mut cfg = EngineConfig::default();
            cfg.rules.modifier_rules.push(ModifierRule {
                source,
                kind: ModifierKind::Hyper,
                flags: EventFlags::HYPER_WITH_SHIFT,
            });
            cfg.rules.combo_rules.push(ComboRule {
                id: RuleId::Preset(12),
                hold: HoldCondition::HyperActive,
                trigger: KeyCode::DELETE,
                action: RuleAction::Key { keycode: KeyCode::FORWARD_DELETE, flags: EventFlags::NONE },
            });
            let mut arb = Arbiter::new(&cfg);

            arb.arbitrate(&cfg, &key_down(source, EventFlags::NONE), GateSnapshot::default(), Millis(0));
            let out = arb.arbitrate(&cfg, &key_down(KeyCode::DELETE, EventFlags::NONE), GateSnapshot::default(), Millis(10));

            assert_eq!(out.layer(), Layer::PresetCombo, "source={source:?}");
            assert_eq!(out.disposition(), Disposition::Consume);
            assert_eq!(out.emitted().len(), 2);
            assert_eq!(out.emitted()[0].kind, EventKind::KeyDown);
            assert_eq!(out.emitted()[0].keycode, KeyCode::FORWARD_DELETE);
            assert_eq!(out.emitted()[1].kind, EventKind::KeyUp);
        }
    }

    /// ⭐ F-18 Event Viewer(`docs/spec/event-viewer.md` §3.4) — 프리셋 조합이 발화하면
    /// `Outcome::rule()` 이 그 프리셋의 `RuleId` 를 돌려준다. `Layer::PresetCombo` 하나로는
    /// 프리셋 16종을 구분할 수 없다는 것이 이 필드를 더한 이유이므로, "어느 프리셋인지"가
    /// 실제로 드러나는지를 직접 확인한다.
    #[test]
    fn outcome_rule_reports_firing_preset_id() {
        let mut cfg = hyper_config();
        cfg.rules.combo_rules.push(ComboRule {
            id: RuleId::Preset(12),
            hold: HoldCondition::HyperActive,
            trigger: KeyCode::DELETE,
            action: RuleAction::Key { keycode: KeyCode::FORWARD_DELETE, flags: EventFlags::NONE },
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let out = arb.arbitrate(&cfg, &key_down(KeyCode::DELETE, EventFlags::NONE), GateSnapshot::default(), Millis(10));

        assert_eq!(out.rule(), Some(RuleId::Preset(12)));
    }

    /// 규칙 없이 통과하는 이벤트는 `rule()` 이 `None` 이다 — "적용된 규칙 없음"을
    /// 뷰어가 `(없음 — 통과)` 로 보여줄 수 있으려면 이 구분이 있어야 한다.
    #[test]
    fn outcome_rule_is_none_on_plain_passthrough() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), GateSnapshot::default(), Millis(0));

        assert_eq!(out.layer(), Layer::Passthrough);
        assert_eq!(out.rule(), None);
    }

    /// P5 — 조합 출력에는 hyper 합성 flags 가 얹히지 않는다: `Caps lock + W = ▲` 는
    /// 순수 방향키이지 `⌃⌥⌘⇧▲` 가 아니다(v1.20 확장 검증).
    #[test]
    fn p5_combo_output_does_not_carry_hyper_flags() {
        let mut cfg = hyper_config();
        cfg.rules.combo_rules.push(ComboRule {
            id: RuleId::Preset(5),
            hold: HoldCondition::Key(KeyCode::CAPS_LOCK),
            trigger: KeyCode::ANSI_W,
            action: RuleAction::Key { keycode: KeyCode::UP_ARROW, flags: EventFlags::NONE },
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let out = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_W, EventFlags::NONE), GateSnapshot::default(), Millis(10));

        assert_eq!(out.layer(), Layer::PresetCombo);
        assert_eq!(out.emitted().len(), 2);
        assert_eq!(out.emitted()[0].keycode, KeyCode::UP_ARROW);
        assert!(!out.emitted()[0].flags.contains(EventFlags::HYPER_WITH_SHIFT), "hyper flags 가 새어 나왔다: {:?}", out.emitted()[0].flags);
    }

    /// R3/v1.62 — `Shift + caps lock = caps lock` 과 `Quick press caps lock` 동시 활성.
    /// shift 를 누른 채 caps lock 을 톡 눌렀다 떼면 `Effect::ToggleCapsLock` 하나만
    /// 나오고 quick press 액션은 나오지 않는다(P6).
    #[test]
    fn r3_shift_plus_caps_lock_suppresses_quick_press() {
        let mut cfg = EngineConfig::default();
        cfg.rules.combo_rules.push(ComboRule {
            id: RuleId::Preset(10),
            hold: HoldCondition::EitherShift,
            trigger: KeyCode::CAPS_LOCK,
            action: RuleAction::ToggleCapsLock,
        });
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: Some(RuleAction::Key { keycode: KeyCode::CAPS_LOCK, flags: EventFlags::NONE }),
            double_tap: None,
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        // shift 를 먼저 누른다 — shift 는 추적 대상이 아니므로 그냥 눌림 테이블에만 기록.
        arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(0));

        // caps lock 을 톡 눌렀다 뗀다(quick press 조건을 만족할 만큼 짧게).
        let down = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::SHIFT), GateSnapshot::default(), Millis(5));
        assert_eq!(down.disposition(), Disposition::Consume);
        assert_eq!(down.layer(), Layer::PresetCombo);
        assert_eq!(down.effects(), &[Effect::ToggleCapsLock], "P6: shift+caps 조합만 발화해야 한다");
        assert!(down.emitted().is_empty(), "조합은 캡스락 키 이벤트를 합성하지 않는다(ToggleCapsLock 은 effect)");

        let up = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::SHIFT), GateSnapshot::default(), Millis(50));
        assert!(up.emitted().is_empty(), "Suppressed → Idle 은 quick press 를 내면 안 된다");
        assert!(up.effects().is_empty());
    }

    /// R1 — `Remap caps lock to:` 와 `Quick press caps lock to execute:` 동시 활성에서
    /// 같은 눌림이 정확히 하나에만 소비된다.
    #[test]
    fn r1_hold_remap_and_quick_press_are_mutually_exclusive_by_timing() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: Some(RuleAction::Key { keycode: KeyCode::ESCAPE, flags: EventFlags::NONE }),
            double_tap: None,
            hold_remap: Some(RuleAction::Key { keycode: KeyCode::LEFT_CONTROL, flags: EventFlags::NONE }),
        });
        let mut arb = Arbiter::new(&cfg);

        // 짧게 눌렀다 뗌 — quick press(ESC) 만 나온다, LEFT_CONTROL 은 나오지 않는다.
        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let up = arb.arbitrate(&cfg, &key_up(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(100));
        assert_eq!(up.emitted().len(), 2);
        assert_eq!(up.emitted()[0].keycode, KeyCode::ESCAPE);
        assert!(!up.emitted().iter().any(|e| e.keycode == KeyCode::LEFT_CONTROL));
    }

    /// P2 — quick press 액션이 등록된 키가 `PendingDown` 인 동안 다른 키가 눌리면 즉시
    /// `HoldConfirmed` 가 되고, hold_remap 대상이 눌린 것으로 표현된다.
    ///
    /// ⭐ **기대값을 정정했다 (2026-08-30, 이슈 #19).** 이전 판은
    /// `LEFT_CONTROL` 의 `EventKind::KeyDown` 을 기대했다 — 그것은 **코드가 무엇을
    /// 하는가**를 그대로 옮겨 적은 것이지 **무엇이 옳은가**가 아니었다(PR #17 이
    /// 지적한 함정). macOS 는 modifier 키의 눌림을 `KeyDown` 으로 전달하지 않는다
    /// (`key-remapping-engine.md` §5 #18) — 그래서 `KeyDown` 을 내보내면 받는 앱은
    /// control 이 눌린 것으로 보지 않고, 사용자 실측의 "다운·업이 전부 업으로 잡힌다"가
    /// 나온다. 옳은 기대값은 **control 비트를 실은 `FlagsChanged`** 다.
    #[test]
    fn p2_other_key_down_confirms_pending_hold_and_emits_hold_remap() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: Some(RuleAction::Key { keycode: KeyCode::ESCAPE, flags: EventFlags::NONE }),
            double_tap: None,
            hold_remap: Some(RuleAction::Key { keycode: KeyCode::LEFT_CONTROL, flags: EventFlags::NONE }),
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::PendingDown { since: Millis(0) });

        // 다른 키(A)의 keyDown — P2 에 의해 즉시 HoldConfirmed 로 확정돼야 한다.
        let a_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), GateSnapshot::default(), Millis(10));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
        // hold_remap 대상(LEFT_CONTROL)이 "눌린 것"으로 A 의 처리에 실려 나와야 한다.
        // ⭐ 기대값의 출처는 구현 상수가 아니라 macOS 헤더다:
        //   kCGEventFlagMaskControl = 0x00040000 (`CGEventTypes.h`)
        //   NX_DEVICELCTLKEYMASK    = 0x00000001 (`IOKit/hidsystem/IOLLEvent.h`)
        const LEFT_CONTROL_HELD: u64 = 0x0004_0001;
        let held = a_down
            .emitted()
            .iter()
            .find(|e| e.keycode == KeyCode::LEFT_CONTROL)
            .unwrap_or_else(|| panic!("hold_remap 대상이 방출되지 않았다: {:?}", a_down.emitted()));
        assert_eq!(
            held.kind,
            EventKind::FlagsChanged,
            "modifier 대상은 FlagsChanged 로 나가야 한다(§5 #18) — KeyDown 은 눌림으로 인식되지 않는다"
        );
        assert_eq!(
            held.flags.0 & LEFT_CONTROL_HELD,
            LEFT_CONTROL_HELD,
            "control 비트(일반 + 좌측 구분)가 실려 있어야 한다: {:#010X}",
            held.flags.0
        );
    }

    /// P7/엣지1 — 좌우 shift 동시 → 토글 1회, 그 직후 double tap 조건이 성립해도 추가
    /// 토글이 없다.
    #[test]
    fn p7_left_right_shift_together_toggles_once_and_suppresses_double_tap() {
        let mut cfg = EngineConfig::default();
        cfg.rules.combo_rules.push(ComboRule {
            id: RuleId::Preset(9),
            hold: HoldCondition::Key(KeyCode::RIGHT_SHIFT),
            trigger: KeyCode::LEFT_SHIFT,
            action: RuleAction::ToggleCapsLock,
        });
        cfg.rules.combo_rules.push(ComboRule {
            id: RuleId::Preset(9),
            hold: HoldCondition::Key(KeyCode::LEFT_SHIFT),
            trigger: KeyCode::RIGHT_SHIFT,
            action: RuleAction::ToggleCapsLock,
        });
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::LEFT_SHIFT,
            quick_press: None,
            double_tap: Some(RuleAction::ToggleCapsLock),
            hold_remap: None,
        });
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::RIGHT_SHIFT,
            quick_press: None,
            double_tap: Some(RuleAction::ToggleCapsLock),
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        // 왼쪽부터 눌러 PendingDown 진입(quick press 없이 double tap 만 있으니 PendingDown 을 거친다).
        let left_down = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(0));
        assert!(left_down.effects().is_empty());
        assert_eq!(arb.state.machine(KeyCode::LEFT_SHIFT), QuickPressState::PendingDown { since: Millis(0) });

        // 오른쪽 shift 다운 — P6 조합 우선 체크가 오른쪽 자신의 FSM 진입보다 먼저 발화해야 한다.
        let right_down = arb.arbitrate(&cfg, &flags_changed(KeyCode::RIGHT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(5));
        assert_eq!(right_down.effects(), &[Effect::ToggleCapsLock]);
        assert_eq!(arb.state.machine(KeyCode::LEFT_SHIFT), QuickPressState::Suppressed, "P7: hold 키도 Suppressed 여야 한다");
        assert_eq!(arb.state.machine(KeyCode::RIGHT_SHIFT), QuickPressState::Suppressed);

        // 두 shift 를 뗀다 — 토글이 추가로 나오면 안 된다.
        let left_up = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(10));
        assert!(left_up.effects().is_empty());
        let right_up = arb.arbitrate(&cfg, &flags_changed(KeyCode::RIGHT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(10));
        assert!(right_up.effects().is_empty(), "P7: Suppressed → Idle 은 double tap 을 내면 안 된다");

        assert_eq!(arb.state.machine(KeyCode::LEFT_SHIFT), QuickPressState::Idle);
        assert_eq!(arb.state.machine(KeyCode::RIGHT_SHIFT), QuickPressState::Idle);
    }

    /// 좌/우 shift 는 `FlagsChanged` 로도 keycode 로 정확히 구분돼 각자의 quick press
    /// 액션이 독립적으로 발화한다.
    #[test]
    fn left_and_right_shift_quick_press_are_independent_via_flags_changed() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::LEFT_SHIFT,
            quick_press: Some(RuleAction::Text('(')),
            double_tap: None,
            hold_remap: None,
        });
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::RIGHT_SHIFT,
            quick_press: Some(RuleAction::Text(')')),
            double_tap: None,
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(0));
        let left_up = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(50));
        assert_eq!(left_up.effects(), &[Effect::TypeChar('(')]);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::RIGHT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(100));
        let right_up = arb.arbitrate(&cfg, &flags_changed(KeyCode::RIGHT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(150));
        assert_eq!(right_up.effects(), &[Effect::TypeChar(')')]);
    }

    // ── M2/F-08 — caps lock quick press/hold, FlagsChanged 로만 구성 ────────────────────

    /// caps lock quick press — `FlagsChanged`(누름) → 300ms 후 `FlagsChanged`(뗌) → quick
    /// press 액션 방출. `KeyDown`/`KeyUp` 이 아니라 `FlagsChanged` 로 구성한다(M1 결함
    /// 재생산 방지).
    #[test]
    fn caps_lock_quick_press_via_flags_changed_only() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: Some(RuleAction::ToggleCapsLock),
            double_tap: None,
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        let down = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK), GateSnapshot::default(), Millis(0));
        assert!(down.effects().is_empty());
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::PendingDown { since: Millis(0) });

        let up = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(300));
        assert_eq!(up.effects(), &[Effect::ToggleCapsLock]);
    }

    /// caps lock hold — `FlagsChanged`(누름) → 1200ms tick → HoldConfirmed → 다른 키 →
    /// 조합 발화(F-08.5 스타일: caps lock + W = 위 방향키).
    #[test]
    fn caps_lock_hold_via_flags_changed_then_tick_then_combo() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: Some(RuleAction::ToggleCapsLock),
            double_tap: None,
            hold_remap: None,
        });
        cfg.rules.combo_rules.push(ComboRule {
            id: RuleId::Preset(5),
            hold: HoldCondition::Key(KeyCode::CAPS_LOCK),
            trigger: KeyCode::ANSI_W,
            action: RuleAction::Key { keycode: KeyCode::UP_ARROW, flags: EventFlags::NONE },
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK), GateSnapshot::default(), Millis(0));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::PendingDown { since: Millis(0) });

        let tick = arb.on_tick(&cfg, Millis(1200));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
        // 이 키는 modifier 소스가 아니므로 flagsChanged 는 나오지 않는다.
        assert!(tick.emitted().is_empty());

        let w_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_W, EventFlags::NONE), GateSnapshot::default(), Millis(1210));
        assert_eq!(w_down.layer(), Layer::PresetCombo);
        assert_eq!(w_down.emitted()[0].keycode, KeyCode::UP_ARROW);
    }

    // ── ⭐ D-1(caps lock → F18)이 켜진 구성 — 이슈 #19 ────────────────────────────────────
    //
    // `docs/dev/architecture.md` §6.1(D-1): caps lock 에 의존하는 프리셋이 하나라도 켜지면
    // 경로 B 가 `caps lock → F18` 커널 매핑을 설치한다. 그래서 탭에는 물리 caps lock 이
    // 아니라 **F18 이, 그것도 `KeyDown`/`KeyUp` 정상 쌍으로** 도착한다. 아래 테스트들은
    // 전부 이 모양(F18 의 KeyDown/KeyUp)을 입력으로 재현한다 — `caps_lock_alias: None` 에
    // `KeyCode::CAPS_LOCK` 을 직접 넣는 이전 테스트들은 이 경로를 한 번도 통과하지 않았고,
    // 그것이 이슈 #19 가 테스트를 뚫고 나간 이유다.

    /// D-1 이 켜진 최소 구성 — `caps_lock_alias: Some(F18)`, 규칙은 각 테스트가 채운다.
    fn d1_cfg() -> EngineConfig {
        EngineConfig { caps_lock_alias: Some(KeyCode::F18), ..EngineConfig::default() }
    }

    /// `kVK_ANSI_C = 0x08`(`Carbon/HIToolbox/Events.h`). `keycode.rs` 의 35종 소스 키
    /// 목록 밖이라 이름 상수가 없다 — ⌃C 조합의 예시로 A 가 아닌 다른 문자 키를 고른
    /// 것뿐이고, 판정 로직에서 이 키 자체가 특별한 의미를 갖지는 않는다.
    const ANSI_C: KeyCode = KeyCode(0x08);

    /// `Remap caps lock to: left control` 이 D-1 아래에서 실제로 도착하는 F18 의
    /// KeyDown/KeyUp 을 옳게 처리한다: 대상이 modifier(`left control`)이므로 `FlagsChanged`
    /// 로 나가야 한다(§5 #18) — `KeyDown` 으로 내면 받는 앱은 control 이 눌린 것으로 보지
    /// 않는다(사용자 실측 "다운·업이 전부 업으로 잡힌다").
    #[test]
    fn d1_f18_arrives_as_keydown_and_remaps_to_left_control_as_flags_changed() {
        let mut cfg = d1_cfg();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: None,
            double_tap: None,
            hold_remap: Some(RuleAction::Key { keycode: KeyCode::LEFT_CONTROL, flags: EventFlags::NONE }),
        });
        let mut arb = Arbiter::new(&cfg);

        // quick press 액션이 없으므로 keyDown 즉시 hold 확정된다.
        let down = arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(down.disposition(), Disposition::Consume);
        assert_eq!(down.emitted().len(), 1);
        assert_eq!(down.emitted()[0].keycode, KeyCode::LEFT_CONTROL);
        // ⭐ macOS 는 modifier 의 눌림을 `KeyDown` 으로 전달하지 않는다
        // (`key-remapping-engine.md` §5 #18) — `KeyDown` 으로 내면 받는 앱은 control 이
        // 눌린 것으로 보지 않고, 그것이 사용자 실측의 "다운·업이 전부 업으로 잡힌다" 다.
        assert_eq!(down.emitted()[0].kind, EventKind::FlagsChanged);
        // 기대값의 출처는 macOS 헤더다: kCGEventFlagMaskControl = 0x00040000
        // (`CGEventTypes.h`), NX_DEVICELCTLKEYMASK = 0x00000001 (`IOKit/hidsystem/IOLLEvent.h`).
        const LEFT_CONTROL_HELD: u64 = 0x0004_0001;
        assert_eq!(down.emitted()[0].flags.0 & LEFT_CONTROL_HELD, LEFT_CONTROL_HELD);

        let up = arb.arbitrate(&cfg, &key_up(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(50));
        assert_eq!(up.emitted().len(), 1);
        assert_eq!(up.emitted()[0].kind, EventKind::FlagsChanged);
        assert_eq!(up.emitted()[0].keycode, KeyCode::LEFT_CONTROL);
        assert_eq!(up.emitted()[0].flags.0 & LEFT_CONTROL_HELD, 0);
    }

    /// 합성 `FlagsChanged` 한 번만으로는 `⌃C` 가 만들어지지 않는다 — 뒤따르는 다른 키
    /// 이벤트에도 control 비트가 실려야 한다(`keystate.rs` 의 `register_sources` 가
    /// hold_remap 대상의 flags 를 슬롯에 실어 `active_synth_flags()` 로 흘려보내는 이유).
    #[test]
    fn d1_left_control_remap_is_carried_onto_following_key_events() {
        let mut cfg = d1_cfg();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: None,
            double_tap: None,
            hold_remap: Some(RuleAction::Key { keycode: KeyCode::LEFT_CONTROL, flags: EventFlags::NONE }),
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let c_down = arb.arbitrate(&cfg, &key_down(ANSI_C, EventFlags::NONE), GateSnapshot::default(), Millis(10));

        const LEFT_CONTROL_HELD: u64 = 0x0004_0001;
        match c_down.disposition() {
            Disposition::PassWithFlags(f) => {
                assert_eq!(
                    f.0 & LEFT_CONTROL_HELD,
                    LEFT_CONTROL_HELD,
                    "합성 flagsChanged 한 번만으로는 ⌃C 가 만들어지지 않는다 — 뒤따르는 키 이벤트에도 비트가 실려야 한다"
                );
            }
            other => panic!("PassWithFlags 가 아니다: {other:?}"),
        }
    }

    /// F-08.2 — quick press caps lock 은 경로 C(`Effect::ToggleCapsLock`)로 나가고, 키
    /// 이벤트는 합성하지 않는다(`power-user-presets.md` §8: "Quick press duration 이내에
    /// 캡스락을 눌렀다 떼면 …").
    #[test]
    fn d1_quick_press_caps_lock_toggles_lock_via_path_c() {
        let mut cfg = d1_cfg();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: Some(RuleAction::ToggleCapsLock),
            double_tap: None,
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let up = arb.arbitrate(&cfg, &key_up(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(100));

        assert_eq!(up.effects(), &[Effect::ToggleCapsLock]);
        assert!(up.emitted().is_empty());
        assert_eq!(up.disposition(), Disposition::Consume);
    }

    /// D-1 이 켜진 구성에서 hold 확정이 원본 F18 을 되살리지 않는다. 근거 — (1) 원본은
    /// 물리적으로 F18 이라 되살려도 어떤 앱에도 의미가 없다(`architecture.md` §6.1).
    /// (2) caps lock 의 원래 기능(잠금 토글)은 `key-remapping-engine.md` §3-c 가 "되돌릴
    /// 수 없는 부작용"의 대표 예로 든 바로 그 동작이다 — 되살리면 안 된다.
    #[test]
    fn d1_long_press_caps_lock_does_not_revive_the_original_f18() {
        let mut cfg = d1_cfg();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: Some(RuleAction::ToggleCapsLock),
            double_tap: None,
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let tick = arb.on_tick(&cfg, Millis(1100));

        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
        assert!(
            tick.emitted().is_empty(),
            "hold 확정이 원본 F18 을 되살렸다 — D-1 구성에서 그것은 아무 앱에도 의미가 없다"
        );
    }

    /// P3·`architecture.md` §6.4 — caps lock 조합은 F18 도착 시에도 정상 발화하고, 방출
    /// flags 에는 caps lock 잠금 비트(`alphaShift`, P5)가 실리지 않는다.
    #[test]
    fn d1_caps_lock_combo_matches_on_f18_arrival() {
        let mut cfg = d1_cfg();
        cfg.rules.combo_rules.push(ComboRule {
            id: RuleId::Preset(5),
            hold: HoldCondition::Key(KeyCode::CAPS_LOCK),
            trigger: KeyCode::ANSI_W,
            action: RuleAction::Key { keycode: KeyCode::UP_ARROW, flags: EventFlags::NONE },
        });
        // CAPS_LOCK 을 추적 키로 만들어야 F18 도착이 `handle_tracked_key_event` 를 탄다.
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: None,
            double_tap: None,
            hold_remap: Some(RuleAction::Nothing),
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let w_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_W, EventFlags::NONE), GateSnapshot::default(), Millis(10));

        assert_eq!(w_down.disposition(), Disposition::Consume);
        assert_eq!(w_down.emitted().len(), 2);
        assert_eq!(w_down.emitted()[0].kind, EventKind::KeyDown);
        assert_eq!(w_down.emitted()[0].keycode, KeyCode::UP_ARROW);
        assert_eq!(w_down.emitted()[1].kind, EventKind::KeyUp);
        // P5 — caps lock 잠금 비트(kCGEventFlagMaskAlphaShift = 0x00010000)가 새어 나오면
        // 안 된다.
        const CAPS_LOCK_BIT: u64 = 0x0001_0000;
        for e in w_down.emitted() {
            assert_eq!(e.flags.0 & CAPS_LOCK_BIT, 0, "조합 출력에 caps lock 잠금 비트가 실렸다");
        }
    }

    /// `architecture.md` §6.1 — "중재기는 진입 즉시 이 keycode 를 caps lock 으로 되돌려
    /// 이후 모든 판정을 물리 caps lock 기준으로 수행한다": D-1 이 켜진 경로(F18 의
    /// KeyDown/KeyUp)와 꺼진 경로(물리 caps lock 의 FlagsChanged 두 번)는 같은 제스처에
    /// 대해 완전히 같은 결과를 내야 한다.
    #[test]
    fn d1_and_non_d1_paths_agree_on_the_same_gesture() {
        let mut cfg_d1 = hyper_config();
        cfg_d1.caps_lock_alias = Some(KeyCode::F18);
        let mut arb_d1 = Arbiter::new(&cfg_d1);
        let d1_down = arb_d1.arbitrate(&cfg_d1, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let d1_up = arb_d1.arbitrate(&cfg_d1, &key_up(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(50));

        let cfg_no_d1 = hyper_config(); // caps_lock_alias: None (기본값)
        let mut arb_no_d1 = Arbiter::new(&cfg_no_d1);
        let no_d1_down =
            arb_no_d1.arbitrate(&cfg_no_d1, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let no_d1_up =
            arb_no_d1.arbitrate(&cfg_no_d1, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), GateSnapshot::default(), Millis(50));

        assert_eq!(d1_down.disposition(), no_d1_down.disposition());
        assert_eq!(d1_down.emitted(), no_d1_down.emitted());
        assert_eq!(d1_down.effects(), no_d1_down.effects());

        assert_eq!(d1_up.disposition(), no_d1_up.disposition());
        assert_eq!(d1_up.emitted(), no_d1_up.emitted());
        assert_eq!(d1_up.effects(), no_d1_up.effects());
    }

    /// 과잉 교정 방지 — 대상이 modifier 가 아니면(`esc`) 종전대로 `KeyDown` 으로 나가야
    /// 한다. 이슈 #19 수정이 "modifier 대상만 FlagsChanged" 조건을 놓쳐 전부 FlagsChanged
    /// 로 바꿔버리는 회귀를 막는다.
    #[test]
    fn non_modifier_remap_target_still_uses_key_down_and_key_up() {
        let mut cfg = d1_cfg();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: None,
            double_tap: None,
            hold_remap: Some(RuleAction::Key { keycode: KeyCode::ESCAPE, flags: EventFlags::NONE }),
        });
        let mut arb = Arbiter::new(&cfg);

        let down = arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(down.emitted().len(), 1);
        assert_eq!(down.emitted()[0].kind, EventKind::KeyDown);
        assert_eq!(down.emitted()[0].keycode, KeyCode::ESCAPE);
    }

    /// `KeyUp` 을 내면 stuck modifier 가 남는다(증상 A 와 같은 뿌리) — `force_reset` 도
    /// modifier 대상은 `FlagsChanged`(빈 flags)로 놓아야 한다.
    #[test]
    fn force_reset_releases_modifier_hold_remap_as_flags_changed() {
        let mut cfg = d1_cfg();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: None,
            double_tap: None,
            hold_remap: Some(RuleAction::Key { keycode: KeyCode::LEFT_CONTROL, flags: EventFlags::NONE }),
        });
        let mut arb = Arbiter::new(&cfg);
        arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);

        let out = arb.force_reset(&cfg);
        const LEFT_CONTROL_HELD: u64 = 0x0004_0001;
        let released = out
            .emitted()
            .iter()
            .find(|e| e.keycode == KeyCode::LEFT_CONTROL)
            .unwrap_or_else(|| panic!("hold_remap 해제 이벤트가 방출되지 않았다: {:?}", out.emitted()));
        assert_eq!(
            released.kind,
            EventKind::FlagsChanged,
            "KeyUp 을 내면 stuck modifier 가 남는다(이슈 #19 증상 A 와 같은 뿌리)"
        );
        assert_eq!(released.flags.0 & LEFT_CONTROL_HELD, 0);
    }

    /// quick press 액션의 대상이 modifier 키(`left command`)면 down+up 이 둘 다
    /// `FlagsChanged` 쌍으로 나가야 한다.
    /// (`kCGEventFlagMaskCommand = 0x00100000`, `NX_DEVICELCMDKEYMASK = 0x00000008`)
    #[test]
    fn quick_press_target_that_is_a_modifier_is_emitted_as_flags_changed_pair() {
        let mut cfg = d1_cfg();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: Some(RuleAction::Key { keycode: KeyCode::LEFT_COMMAND, flags: EventFlags::NONE }),
            double_tap: None,
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        let up = arb.arbitrate(&cfg, &key_up(KeyCode::F18, EventFlags::NONE), GateSnapshot::default(), Millis(100));

        const LEFT_COMMAND_HELD: u64 = 0x0010_0008;
        assert_eq!(up.emitted().len(), 2);
        for e in up.emitted() {
            assert_eq!(e.kind, EventKind::FlagsChanged);
            assert_eq!(e.keycode, KeyCode::LEFT_COMMAND);
        }
        assert_eq!(up.emitted()[0].flags.0 & LEFT_COMMAND_HELD, LEFT_COMMAND_HELD);
        assert_eq!(up.emitted()[1].flags.0 & LEFT_COMMAND_HELD, 0);
    }

    // ── 증상 C — shift 가 죽지 않는다 ─────────────────────────────────────────────────────
    //
    // ⚠️ shift 는 modifier 이므로 입력은 `FlagsChanged` 로 만든다(`KeyDown` 이 아니다) —
    // 그것이 M1 이 놓쳤던 바로 그 구멍이다.

    /// `double tap shift = caps lock` — 두 번의 `FlagsChanged`(down→up) 뒤 300ms 안에
    /// 다시 down 이 오면 double tap 이 성립한다(기본 `double_tap_interval_ms = 300`).
    #[test]
    fn double_tap_shift_toggles_caps_lock() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::LEFT_SHIFT,
            quick_press: None,
            double_tap: Some(RuleAction::ToggleCapsLock),
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(0));
        arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(50));
        let second_down =
            arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(100));

        assert_eq!(second_down.effects(), &[Effect::ToggleCapsLock]);
    }

    /// ⭐ 이번 버그의 핵심 회귀 방지 — **설계 정정판**(2026-08-30). `Arbiter::
    /// has_substitute_output` 의 판정을 따른다: shift 는 caps lock 이 아니고, modifier
    /// 소스도 아니고, hold_remap 대상도 없으므로 대체 출력이 없다 — 그런 키는 원본을
    /// **애초에 소비하지 않고 그대로 통과**시킨다(`Disposition::Pass`). `key-
    /// remapping-engine.md` §3-c 가 보류(소비 후 재방출)를 요구하는 유일한 근거는
    /// "되돌릴 수 없는 부작용"(caps lock 잠금 토글이 그 예)인데, shift 홀로는 그런
    /// 부작용이 없다 — 그러니 보류할 이유가 없다(`has_substitute_output` 문서 참고).
    /// `power-user-presets.md` §3.2 F-08.8 행의 "부가 효과" 열도 비어 있다(shift 를
    /// 무력화한다는 서술이 없다).
    #[test]
    fn double_tap_shift_preset_does_not_kill_the_shift_key() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::LEFT_SHIFT,
            quick_press: None,
            double_tap: Some(RuleAction::ToggleCapsLock),
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        let shift_down = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(0));
        assert_eq!(
            shift_down.disposition(),
            Disposition::Pass,
            "shift 가 통째로 삼켜진다 — `Double tap shift = caps lock` 을 켜면 Shift+a 가 소문자로 나간다"
        );

        // 뒤따르는 다른 키(A)의 keyDown 도 shift 를 막지 않아야 한다 — dispatch_other_key_down
        // 이 대체 출력 없는 슬롯을 HoldConfirmed 로 전이시키더라도 아무것도 합성하지 않는다.
        let a_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), GateSnapshot::default(), Millis(10));
        assert_eq!(
            a_down.disposition(),
            Disposition::Pass,
            "shift 가 통째로 삼켜진다 — `Double tap shift = caps lock` 을 켜면 Shift+a 가 소문자로 나간다"
        );
    }

    /// §3-c 표 7행 — "유예됐던 단일 quick press 액션을 지금 방출"이라 했는데 방출할
    /// 액션이 없다(double tap 전용 키). ⭐ **설계 정정판**: 원본을 애초에 통과시켜
    /// 왔으므로(`has_substitute_output` == false) 되살릴 것 자체가 없다 — down/up 모두
    /// `Disposition::Pass` 였고, 타임아웃 시점에도 `on_tick` 은 아무것도 합성하지 않는다.
    #[test]
    fn single_shift_tap_passes_through_and_emits_nothing_on_timeout() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::LEFT_SHIFT,
            quick_press: None,
            double_tap: Some(RuleAction::ToggleCapsLock),
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        let down = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(0));
        assert_eq!(down.disposition(), Disposition::Pass);
        let up = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(50));
        assert_eq!(up.disposition(), Disposition::Pass);

        let tick = arb.on_tick(&cfg, Millis(400)); // double tap 간격(300ms) 초과
        assert!(tick.emitted().is_empty(), "원본이 이미 통과했으므로 되살릴 것이 없다: {:?}", tick.emitted());
        assert!(tick.effects().is_empty());
    }

    /// `force_reset` 은 대체 출력이 없는 키(shift)에 대해서는 아무것도 방출하지 않는다 —
    /// 원본을 통과시켜 왔으므로 물리 키의 실제 뗌이 해제를 담당하고, 우리가 합성할
    /// "해제 이벤트"가 애초에 존재하지 않는다(`has_substitute_output` 문서 참고).
    #[test]
    fn force_reset_emits_nothing_for_keys_without_substitute_output() {
        let mut cfg = EngineConfig::default();
        cfg.rules.source_actions.push(SourceKeyActions {
            key: KeyCode::LEFT_SHIFT,
            quick_press: None,
            double_tap: Some(RuleAction::ToggleCapsLock),
            hold_remap: None,
        });
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), GateSnapshot::default(), Millis(0));
        arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), GateSnapshot::default(), Millis(10));
        assert_eq!(arb.state.machine(KeyCode::LEFT_SHIFT), QuickPressState::HoldConfirmed);

        let out = arb.force_reset(&cfg);
        assert!(
            !out.emitted().iter().any(|e| e.keycode == KeyCode::LEFT_SHIFT),
            "원본을 통과시켜 온 키인데 force_reset 이 LEFT_SHIFT 이벤트를 합성해 냈다: {:?}",
            out.emitted()
        );
    }

    // ══════════════════════════════════════════════════════════════════════════
    // F-16 한국어 입력 지원 — `docs/spec/korean-input.md` §3.1·§3.3·§3.4, D-K4~D-K7
    // ══════════════════════════════════════════════════════════════════════════
    //
    // ⭐ 이 프로젝트는 같은 계열의 사고가 세 번 있었다: M1(FlagsChanged 를 흘려 hyper 가
    // 발동하지 않음), M2-2(테스트가 코드와 같은 상수를 기대값으로 써서 "코드가 무엇을
    // 하는가" 만 검증), PR #23(modifier 출력을 KeyDown(flags 0)으로 합성). 아래 헬퍼는
    // 이 세 실패를 재발시키지 않도록 설계됐다:
    //   1. 트리거를 누르는 표현은 `press_modifier`/`release_modifier` 로 만든다 —
    //      실제 macOS 가 보내는 `FlagsChanged` + 일반 마스크 + device 비트 모양이다.
    //   2. 기대값은 구현 상수(`EventFlags::CONTROL` 등)에서 가져오지 않고 macOS 헤더
    //      리터럴을 직접 적는다.

    /// F-16.1(`Korean(13)`) — `docs/spec/korean-input.md` §3.1 표 그대로.
    fn korean_rule_shift_space() -> KoreanRule {
        KoreanRule {
            id: RuleId::Korean(13),
            trigger_key: KeyCode::SPACE,
            trigger: KoreanTrigger::ShiftOnly,
            requires_korean_ime: false,
            out_keycode: KeyCode::SPACE,
            out_flags: EventFlags(0x0004_0001), // CONTROL(0x40000) | NX_DEVICELCTLKEYMASK(0x1)
        }
    }

    /// F-16.4(`Korean(16)`) — `docs/spec/korean-input.md` §3.1 표 그대로.
    fn korean_rule_won_grave() -> KoreanRule {
        KoreanRule {
            id: RuleId::Korean(16),
            trigger_key: KeyCode::ANSI_GRAVE,
            trigger: KoreanTrigger::NoModifier,
            requires_korean_ime: true,
            out_keycode: KeyCode::ANSI_GRAVE,
            out_flags: EventFlags(0x0008_0020), // ALTERNATE(0x80000) | NX_DEVICELALTKEYMASK(0x20)
        }
    }

    /// F-16.2(`Korean(14)`) 한/영 — `docs/spec/korean-input.md` §3.2·D-K14 표 그대로.
    fn korean_rule_han_eng() -> KoreanRule {
        KoreanRule {
            id: RuleId::Korean(14),
            trigger_key: KeyCode::JIS_KANA,
            trigger: KoreanTrigger::NoModifier,
            requires_korean_ime: false,
            out_keycode: KeyCode::SPACE,
            out_flags: EventFlags(0x0004_0001), // CONTROL(0x40000) | NX_DEVICELCTLKEYMASK(0x1)
        }
    }

    /// F-16.3(`Korean(15)`) 한자 — `docs/spec/korean-input.md` §3.2·D-K14 표 그대로.
    fn korean_rule_hanja() -> KoreanRule {
        KoreanRule {
            id: RuleId::Korean(15),
            trigger_key: KeyCode::JIS_EISU,
            trigger: KoreanTrigger::NoModifier,
            requires_korean_ime: true,
            out_keycode: KeyCode::RETURN,
            out_flags: EventFlags(0x0008_0040), // ALTERNATE(0x80000) | NX_DEVICERALTKEYMASK(0x40)
        }
    }

    fn korean_config() -> EngineConfig {
        let mut cfg = EngineConfig::default();
        cfg.rules.korean_rules = vec![
            korean_rule_shift_space(),
            korean_rule_han_eng(),
            korean_rule_hanja(),
            korean_rule_won_grave(),
        ];
        cfg
    }

    /// macOS 가 modifier 키의 눌림을 실제로 보내는 형태: `FlagsChanged` + 일반 마스크 +
    /// device 비트. ⭐ 값은 macOS 헤더 리터럴이다(구현 상수에서 역산하지 않는다) —
    /// 출처는 `keycode.rs::modifier_flags_match_macos_header_values` 와 동일:
    ///   `CoreGraphics/CGEventTypes.h`
    ///     kCGEventFlagMaskShift      = 0x00020000
    ///     kCGEventFlagMaskControl    = 0x00040000
    ///     kCGEventFlagMaskAlternate  = 0x00080000
    ///     kCGEventFlagMaskCommand    = 0x00100000
    ///   `IOKit/hidsystem/IOLLEvent.h`
    ///     NX_DEVICELCTLKEYMASK   = 0x00000001   NX_DEVICELSHIFTKEYMASK = 0x00000002
    ///     NX_DEVICERSHIFTKEYMASK = 0x00000004   NX_DEVICELCMDKEYMASK   = 0x00000008
    ///     NX_DEVICELALTKEYMASK   = 0x00000020   NX_DEVICERCTLKEYMASK   = 0x00002000
    fn press_modifier(keycode: KeyCode, header_literal_flags: u64) -> InputEvent {
        InputEvent {
            kind: EventKind::FlagsChanged,
            keycode,
            flags: EventFlags(header_literal_flags),
            autorepeat: false,
        }
    }

    /// modifier 를 뗀다 — 실제 macOS 는 그 modifier 의 비트를 지운 `FlagsChanged` 를 보낸다.
    /// 우리 판정은 `is_pressed` 정본 테이블만 보므로(P3) `flags` 값 자체는 판정에
    /// 관여하지 않지만, 이벤트 모양의 사실성을 위해 0 을 싣는다(다른 modifier 가 동시에
    /// 눌려 있지 않은 이 테스트들의 시나리오에서는 이것이 실제 값과도 일치한다).
    fn release_modifier(keycode: KeyCode) -> InputEvent {
        InputEvent {
            kind: EventKind::FlagsChanged,
            keycode,
            flags: EventFlags::NONE,
            autorepeat: false,
        }
    }

    fn key_down_repeat(keycode: KeyCode, flags: EventFlags) -> InputEvent {
        InputEvent {
            kind: EventKind::KeyDown,
            keycode,
            flags,
            autorepeat: true,
        }
    }

    /// 시나리오 1 — shift 만 눌린 채 space keyDown → `space` + control 로 치환, shift 제거.
    #[test]
    fn f16_1_shift_only_space_is_substituted_with_control() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &press_modifier(KeyCode::LEFT_SHIFT, 0x0002_0002), GateSnapshot::default(), Millis(0));
        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::SPACE, EventFlags(0x0002_0002)),
            GateSnapshot::default(),
            Millis(10),
        );

        assert_eq!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted().len(), 1);
        assert_eq!(out.emitted()[0].kind, EventKind::KeyDown);
        assert_eq!(out.emitted()[0].keycode, KeyCode::SPACE);
        // ⭐ shift 는 제거되고 control 만 남는다 — 기대값은 macOS 헤더 리터럴.
        assert_eq!(out.emitted()[0].flags, EventFlags(0x0004_0001));

        // 사용자가 space 를 뗀 뒤 shift 도 뗀다 — 래치(D-K6)가 keyUp 도 같은 치환으로
        // 짝을 맞춘다. `release_modifier` 로 실제 `FlagsChanged`(off) 모양을 재현한다.
        let space_up = arb.arbitrate(&cfg, &key_up(KeyCode::SPACE, EventFlags(0x0002_0002)), GateSnapshot::default(), Millis(15));
        assert_eq!(space_up.layer(), Layer::KoreanInput);
        assert_eq!(space_up.emitted()[0].kind, EventKind::KeyUp);
        assert_eq!(space_up.emitted()[0].flags, EventFlags(0x0004_0001));

        arb.arbitrate(&cfg, &release_modifier(KeyCode::LEFT_SHIFT), GateSnapshot::default(), Millis(20));
        assert!(!arb.state.is_pressed(KeyCode::LEFT_SHIFT));
    }

    /// 시나리오 2 — shift + command 가 눌린 채 space → 개입하지 않는다.
    #[test]
    fn f16_1_shift_plus_command_does_not_intervene() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &press_modifier(KeyCode::LEFT_SHIFT, 0x0002_0002), GateSnapshot::default(), Millis(0));
        arb.arbitrate(
            &cfg,
            &press_modifier(KeyCode::LEFT_COMMAND, 0x0012_000A),
            GateSnapshot::default(),
            Millis(5),
        );
        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::SPACE, EventFlags(0x0012_000A)),
            GateSnapshot::default(),
            Millis(10),
        );

        assert_ne!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// 시나리오 3 — 아무 modifier 없이 space → 개입하지 않는다.
    #[test]
    fn f16_1_no_modifier_space_does_not_intervene() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::SPACE, EventFlags::NONE), GateSnapshot::default(), Millis(0));

        assert_ne!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// 시나리오 4 — ⭐ 한국어 IME 활성, modifier 없음 → `grave` + option 으로 치환.
    #[test]
    fn f16_4_ime_active_no_modifier_grave_is_substituted_with_option() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_GRAVE, EventFlags::NONE), gates, Millis(0));

        assert_eq!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted().len(), 1);
        assert_eq!(out.emitted()[0].keycode, KeyCode::ANSI_GRAVE);
        assert_eq!(out.emitted()[0].flags, EventFlags(0x0008_0020));
    }

    /// 시나리오 5 — ⭐ 한국어 IME 비활성 → 개입하지 않는다(`` ` `` 가 그대로 나간다).
    #[test]
    fn f16_4_ime_inactive_does_not_intervene() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Inactive,
            ..Default::default()
        };

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_GRAVE, EventFlags::NONE), gates, Millis(0));

        assert_ne!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// 시나리오 6 — ⭐ 한국어 IME `Unknown`(판정 불가) → 개입하지 않는다(fail-closed, §3.3).
    #[test]
    fn f16_4_ime_unknown_fails_closed() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        // GateSnapshot::default() 의 korean_ime 는 Unknown 이다 — 명시적으로 재확인한다.
        let gates = GateSnapshot::default();
        assert_eq!(gates.korean_ime, KoreanImeState::Unknown);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_GRAVE, EventFlags::NONE), gates, Millis(0));

        assert_ne!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// 시나리오 7 — ⭐ `⌘`+`` ` ``·`⌃`+`` ` ``·`⇧`+`` ` `` 각각 → 개입하지 않는다.
    /// 명세 §8: "이 부정형 기준이 §3.1 원문 의도의 정본이다."
    #[test]
    fn f16_4_modifier_plus_grave_never_intervenes() {
        let cfg = korean_config();
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };

        let cases: &[(KeyCode, u64, u64)] = &[
            // (modifier keycode, press flags 리터럴, grave 이벤트에 실릴 flags 리터럴)
            (KeyCode::LEFT_COMMAND, 0x0010_0008, 0x0010_0008), // COMMAND | NX_DEVICELCMDKEYMASK
            (KeyCode::LEFT_CONTROL, 0x0004_0001, 0x0004_0001), // CONTROL | NX_DEVICELCTLKEYMASK
            (KeyCode::LEFT_SHIFT, 0x0002_0002, 0x0002_0002),   // SHIFT | NX_DEVICELSHIFTKEYMASK
        ];

        for (modifier, press_flags, grave_flags) in cases {
            let mut arb = Arbiter::new(&cfg);
            arb.arbitrate(&cfg, &press_modifier(*modifier, *press_flags), gates, Millis(0));
            let out = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_GRAVE, EventFlags(*grave_flags)), gates, Millis(10));

            assert_ne!(
                out.layer(),
                Layer::KoreanInput,
                "modifier {:#04X} 와 함께 눌렀는데 F-16.4 가 개입했다",
                modifier.0
            );
            assert_eq!(out.disposition(), Disposition::Pass);
        }
    }

    /// 시나리오 8 — ⭐ hyper 소스가 `right shift` 인 구성에서 hyper 합성이 활성인 채
    /// space → F-16.1 이 개입하지 않는다(명세 §8·§3.4 "여기가 진짜 위험 지점이다").
    #[test]
    fn f16_1_does_not_intervene_when_right_shift_is_hyper_source() {
        let mut cfg = korean_config();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::RIGHT_SHIFT,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        let mut arb = Arbiter::new(&cfg);

        let hold = arb.arbitrate(
            &cfg,
            &press_modifier(KeyCode::RIGHT_SHIFT, 0x0002_0004),
            GateSnapshot::default(),
            Millis(0),
        );
        assert_eq!(arb.state.machine(KeyCode::RIGHT_SHIFT), QuickPressState::HoldConfirmed);
        assert_eq!(hold.layer(), Layer::HyperModifier);

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::SPACE, EventFlags(0x0002_0004)),
            GateSnapshot::default(),
            Millis(10),
        );

        // F-16.1 이 개입하지 않았으므로 hyper 의 "유지" 효과가 그대로 적용된다 —
        // space 가 hyper 4비트를 실은 채 통과한다(hyper + space 가 정상 동작).
        assert_ne!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.layer(), Layer::HyperModifier);
        let flags = passed_flags(&out);
        assert_eq!(flags & EventFlags::HYPER_WITH_SHIFT, EventFlags::HYPER_WITH_SHIFT);
    }

    /// 시나리오 9 — ⭐ 앱 제외 게이트가 켜진 상태 → F-16 규칙이 전부 통과되지만
    /// hyper 는 계속 동작한다(F-10 전역 게이트와 구분되는 지점, 명세 §8).
    #[test]
    fn f16_app_excluded_gate_passes_korean_rules_but_hyper_still_works() {
        let mut cfg = korean_config();
        cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        let mut arb = Arbiter::new(&cfg);
        let excluded_gates = GateSnapshot {
            korean_app_excluded: true,
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };

        // F-16.4 — modifier 없이 grave 를 눌러도 개입하지 않는다(앱 제외).
        let grave_out = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_GRAVE, EventFlags::NONE), excluded_gates, Millis(0));
        assert_ne!(grave_out.layer(), Layer::KoreanInput);
        assert_eq!(grave_out.disposition(), Disposition::Pass);

        // hyper 는 같은 게이트 스냅샷 아래서도 정상 동작한다 — korean_app_excluded 는
        // 계층 3 안의 F-16 규칙 평가에만 영향을 준다.
        let caps_down = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), excluded_gates, Millis(10));
        assert_eq!(caps_down.layer(), Layer::HyperModifier);
        assert_eq!(caps_down.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);
    }

    /// 시나리오 10 — `Caps lock + space = enter`(F-08.4, `Preset(4)`)와 F-16.1 을
    /// 동시에 켜고 caps lock+shift+space 를 누르면 F-08.4 가 결정론적으로 이긴다
    /// (`Preset(4) < Korean(13)`, 명세 §3.4 표).
    #[test]
    fn f08_4_wins_deterministically_over_f16_1() {
        let mut cfg = korean_config();
        cfg.rules.combo_rules.push(ComboRule {
            id: RuleId::Preset(4),
            hold: HoldCondition::Key(KeyCode::CAPS_LOCK),
            trigger: KeyCode::SPACE,
            action: RuleAction::Key {
                keycode: KeyCode::RETURN,
                flags: EventFlags::NONE,
            },
        });
        let mut arb = Arbiter::new(&cfg);

        // caps lock 은 modifier 소스로 등록돼 있지 않다 — 이 테스트는 순수하게
        // "눌려 있음" 상태만 필요하다. FlagsChanged 로 눌러 정본 테이블을 채운다.
        arb.arbitrate(&cfg, &press_modifier(KeyCode::CAPS_LOCK, 0x0001_0000), GateSnapshot::default(), Millis(0));
        arb.arbitrate(&cfg, &press_modifier(KeyCode::LEFT_SHIFT, 0x0002_0002), GateSnapshot::default(), Millis(5));

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::SPACE, EventFlags(0x0002_0002)),
            GateSnapshot::default(),
            Millis(10),
        );

        assert_eq!(out.layer(), Layer::PresetCombo);
        assert_eq!(out.emitted()[0].keycode, KeyCode::RETURN);
        assert_ne!(out.emitted()[0].keycode, KeyCode::SPACE);
    }

    /// 시나리오 11 — keyDown/keyUp 짝. 치환된 keyDown 뒤의 keyUp 도 같은 keycode/flags
    /// 로 치환된다(래치, D-K6).
    #[test]
    fn f16_4_key_up_replays_the_latched_substitution() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };

        let down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_GRAVE, EventFlags::NONE), gates, Millis(0));
        assert_eq!(down.emitted()[0].flags, EventFlags(0x0008_0020));

        // keyUp 이 도착할 때는 IME 가 이미 비활성으로 바뀌었다고 하더라도 — 래치는
        // "조건을 다시 평가하지 않는다"(D-K6) — 여전히 같은 치환이 나가야 한다.
        let up_gates = GateSnapshot {
            korean_ime: KoreanImeState::Inactive,
            ..Default::default()
        };
        let up = arb.arbitrate(&cfg, &key_up(KeyCode::ANSI_GRAVE, EventFlags::NONE), up_gates, Millis(20));

        assert_eq!(up.layer(), Layer::KoreanInput);
        assert_eq!(up.disposition(), Disposition::Consume);
        assert_eq!(up.emitted().len(), 1);
        assert_eq!(up.emitted()[0].kind, EventKind::KeyUp);
        assert_eq!(up.emitted()[0].keycode, KeyCode::ANSI_GRAVE);
        assert_eq!(up.emitted()[0].flags, EventFlags(0x0008_0020));

        // 래치는 소비된 뒤 지워진다 — 다음 keyUp(대응하는 keyDown 없는)은 통과한다.
        let stray_up = arb.arbitrate(&cfg, &key_up(KeyCode::ANSI_GRAVE, EventFlags::NONE), up_gates, Millis(30));
        assert_ne!(stray_up.layer(), Layer::KoreanInput);
    }

    /// 시나리오 12 — 자동 반복(`autorepeat: true`) keyDown 도 매번 치환된다(§5#7).
    #[test]
    fn f16_4_autorepeat_key_down_is_substituted_every_time() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };

        let first = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_GRAVE, EventFlags::NONE), gates, Millis(0));
        assert_eq!(first.layer(), Layer::KoreanInput);

        for i in 1..=3u64 {
            let repeat = arb.arbitrate(
                &cfg,
                &key_down_repeat(KeyCode::ANSI_GRAVE, EventFlags::NONE),
                gates,
                Millis(10 * i),
            );
            assert_eq!(repeat.layer(), Layer::KoreanInput, "반복 {i} 회차에서 치환되지 않았다");
            assert_eq!(repeat.disposition(), Disposition::Consume);
            assert_eq!(repeat.emitted()[0].flags, EventFlags(0x0008_0020));
        }
    }

    /// 시나리오 13 — ⭐ 트리거 이벤트가 `FlagsChanged` 로 도착해도 같은 결과가 나온다
    /// (D-K13). `arbitrate` 는 진입 즉시 `normalize_kind()` 로 환원하므로, 트리거 키가
    /// `KeyDown` 으로 오든 `FlagsChanged` 로 오든 같은 경로를 탄다 — 이 프로젝트가
    /// 세 번 데인 "실제 이벤트 모양을 가정하지 않는다" 원칙의 회귀 테스트다.
    #[test]
    fn f16_4_fires_the_same_way_when_trigger_arrives_as_flags_changed() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };

        // grave 가 KeyDown 이 아니라 FlagsChanged 로 도착한다 — normalize_kind 가
        // is_pressed(GRAVE)==false 이므로 이것을 KeyDown 으로 환원해야 한다.
        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::ANSI_GRAVE, EventFlags::NONE), gates, Millis(0));

        assert_eq!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted()[0].kind, EventKind::KeyDown);
        assert_eq!(out.emitted()[0].keycode, KeyCode::ANSI_GRAVE);
        assert_eq!(out.emitted()[0].flags, EventFlags(0x0008_0020));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // F-16 2단계 — 한/영(F-16.2)·한자(F-16.3) — `docs/spec/korean-input.md` §3.2·D-K14
    // ══════════════════════════════════════════════════════════════════════════

    /// 시나리오 1 — F-16.2: modifier 없는 `0x68`(`JIS_KANA`) keyDown → `space` + control 로 치환.
    #[test]
    fn f16_2_no_modifier_han_eng_is_substituted_with_space_control() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_KANA, EventFlags::NONE), GateSnapshot::default(), Millis(0));

        assert_eq!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted().len(), 1);
        assert_eq!(out.emitted()[0].kind, EventKind::KeyDown);
        assert_eq!(out.emitted()[0].keycode, KeyCode::SPACE);
        assert_eq!(out.emitted()[0].flags, EventFlags(0x0004_0001));
    }

    /// 시나리오 2 — ⭐ F-16.2: 한국어 IME 가 `Inactive` 여도 발화한다.
    ///
    /// **왜 이것이 옳은가.** 한/영 키는 *입력 소스를 바꾸는* 키다 — 한국어 상태에서
    /// 영문으로 넘어갈 때도, 영문 상태에서 한국어로 되돌아올 때도 똑같이 눌린다.
    /// `requires_korean_ime` 조건을 걸면 "한국어일 때만 발화" 가 되어, 영문 상태에서는
    /// 게이트가 거짓이라 발화하지 않는다 — **한국어→영문 전환만 가능하고 되돌아올 수
    /// 없는 편도 키**가 된다. 이것이 D-K14 가 F-16.2 를 `requires_korean_ime = false`
    /// 로 정한 이유이자, 이 프로젝트에서 뒤집히기 가장 쉬운 지점이다(명세 §3.1).
    #[test]
    fn f16_2_fires_even_when_korean_ime_is_inactive_to_avoid_a_one_way_key() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Inactive,
            ..Default::default()
        };

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_KANA, EventFlags::NONE), gates, Millis(0));

        assert_eq!(
            out.layer(),
            Layer::KoreanInput,
            "IME 가 Inactive 여도 한/영은 발화해야 한다 — 그렇지 않으면 영문→한국어 전환이 막힌다"
        );
        assert_eq!(out.emitted()[0].keycode, KeyCode::SPACE);
        assert_eq!(out.emitted()[0].flags, EventFlags(0x0004_0001));
    }

    /// 시나리오 3 — F-16.2: 한국어 IME 가 `Unknown`(판정 불가) 이어도 발화한다 —
    /// `requires_korean_ime = false` 라 애초에 IME 조건을 보지 않는다.
    #[test]
    fn f16_2_fires_even_when_korean_ime_is_unknown() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot::default();
        assert_eq!(gates.korean_ime, KoreanImeState::Unknown);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_KANA, EventFlags::NONE), gates, Millis(0));

        assert_eq!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.emitted()[0].keycode, KeyCode::SPACE);
    }

    /// 시나리오 4 — F-16.3: IME `Active` + modifier 없는 `0x66`(`JIS_EISU`) → `return`
    /// + right option 으로 치환.
    #[test]
    fn f16_3_ime_active_no_modifier_hanja_is_substituted_with_return_right_option() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_EISU, EventFlags::NONE), gates, Millis(0));

        assert_eq!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted().len(), 1);
        assert_eq!(out.emitted()[0].kind, EventKind::KeyDown);
        assert_eq!(out.emitted()[0].keycode, KeyCode::RETURN);
        assert_eq!(out.emitted()[0].flags, EventFlags(0x0008_0040));
    }

    /// 시나리오 5 — ⭐ F-16.3: IME `Inactive` → 발화하지 않는다. 출력이 `return` 이라
    /// 오발하면 폼 제출 같은 되돌리기 어려운 부작용이 나므로 조건을 엄격히 요구한다.
    #[test]
    fn f16_3_ime_inactive_does_not_intervene() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Inactive,
            ..Default::default()
        };

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_EISU, EventFlags::NONE), gates, Millis(0));

        assert_ne!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// 시나리오 6 — ⭐ F-16.3: IME `Unknown`(판정 불가) → 발화하지 않는다(fail-closed,
    /// 명세 §3.3). `return` 을 내는 규칙이라 판정 불가 상태에서 발화하는 쪽보다
    /// 발화하지 않는 쪽이 안전하다.
    #[test]
    fn f16_3_ime_unknown_fails_closed() {
        let cfg = korean_config();
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot::default();
        assert_eq!(gates.korean_ime, KoreanImeState::Unknown);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_EISU, EventFlags::NONE), gates, Millis(0));

        assert_ne!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// 시나리오 7 — 두 규칙 모두: modifier 가 하나라도 눌려 있으면 개입하지 않는다
    /// (`⌘`·`⇧` 각각으로 확인).
    #[test]
    fn f16_2_and_f16_3_do_not_intervene_when_a_modifier_is_held() {
        let cfg = korean_config();
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };

        let cases: &[(KeyCode, u64)] = &[
            (KeyCode::LEFT_COMMAND, 0x0010_0008), // COMMAND | NX_DEVICELCMDKEYMASK
            (KeyCode::LEFT_SHIFT, 0x0002_0002),   // SHIFT | NX_DEVICELSHIFTKEYMASK
        ];

        for (modifier, press_flags) in cases {
            for (trigger_key, expect_msg) in
                [(KeyCode::JIS_KANA, "한/영"), (KeyCode::JIS_EISU, "한자")]
            {
                let mut arb = Arbiter::new(&cfg);
                arb.arbitrate(&cfg, &press_modifier(*modifier, *press_flags), gates, Millis(0));
                let out = arb.arbitrate(&cfg, &key_down(trigger_key, EventFlags(*press_flags)), gates, Millis(10));

                assert_ne!(
                    out.layer(),
                    Layer::KoreanInput,
                    "{expect_msg} 규칙이 modifier {:#04X} 와 함께 눌렀는데 개입했다",
                    modifier.0
                );
                assert_eq!(out.disposition(), Disposition::Pass);
            }
        }
    }

    /// 시나리오 8 — ⭐⭐ F-16.2·F-16.3: 트리거가 `FlagsChanged` 로 도착해도 같은
    /// 결과가 나온다(D-K13).
    ///
    /// **근거**: Chromium `keyboard_code_conversion_mac.mm` 이 남긴 단서 —
    /// "macOS Eisu/Kana key events have a space symbol as `event.characters`, but
    /// the symbol is not generated for users and the event is just used for
    /// enabling/disabling an IME." 즉 이 두 키는 **문자를 내지 않는 IME 토글 전용
    /// 이벤트**이고, `CGEventTap` 에 `KeyDown` 으로 도착하는지 `FlagsChanged` 로
    /// 도착하는지는 실기기 없이 확인되지 않았다(§3.2). 이 프로젝트는 정확히 이
    /// "이벤트 모양을 가정한" 지점에서 세 번 데였다 — M1 이 `FlagsChanged` 를 흘려
    /// hyper 가 발동하지 않았고, PR #23 은 modifier 출력을 `KeyDown`(flags 0)으로
    /// 합성했다. 그래서 구현은 도착 종류를 가정하지 않고 `normalize_kind()` 로 환원한
    /// `kind` 만 본다(D-K13) — 이 테스트가 그 불변식을 검증한다.
    #[test]
    fn f16_2_and_f16_3_fire_the_same_way_when_trigger_arrives_as_flags_changed() {
        let cfg = korean_config();

        // 한/영 — FlagsChanged 로 도착.
        let mut arb = Arbiter::new(&cfg);
        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::JIS_KANA, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.emitted()[0].kind, EventKind::KeyDown);
        assert_eq!(out.emitted()[0].keycode, KeyCode::SPACE);
        assert_eq!(out.emitted()[0].flags, EventFlags(0x0004_0001));

        // 한자 — FlagsChanged 로 도착. IME Active 필요.
        let mut arb = Arbiter::new(&cfg);
        let gates = GateSnapshot {
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };
        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::JIS_EISU, EventFlags::NONE), gates, Millis(0));
        assert_eq!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.emitted()[0].kind, EventKind::KeyDown);
        assert_eq!(out.emitted()[0].keycode, KeyCode::RETURN);
        assert_eq!(out.emitted()[0].flags, EventFlags(0x0008_0040));
    }

    /// 시나리오 9 — 앱 제외 게이트가 켜지면 두 규칙 다 통과된다.
    #[test]
    fn f16_2_and_f16_3_pass_through_when_app_excluded() {
        let cfg = korean_config();
        let excluded_gates = GateSnapshot {
            korean_app_excluded: true,
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };

        let mut arb = Arbiter::new(&cfg);
        let han_eng_out = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_KANA, EventFlags::NONE), excluded_gates, Millis(0));
        assert_ne!(han_eng_out.layer(), Layer::KoreanInput);
        assert_eq!(han_eng_out.disposition(), Disposition::Pass);

        let hanja_out = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_EISU, EventFlags::NONE), excluded_gates, Millis(10));
        assert_ne!(hanja_out.layer(), Layer::KoreanInput);
        assert_eq!(hanja_out.disposition(), Disposition::Pass);
    }

    /// 시나리오 10 — keyUp 래치(D-K6)가 두 규칙에도 적용된다: 치환된 keyDown 뒤의
    /// keyUp 도 같은 keycode/flags 로 치환된다.
    #[test]
    fn f16_2_and_f16_3_key_up_replays_the_latched_substitution() {
        let cfg = korean_config();

        // 한/영.
        let mut arb = Arbiter::new(&cfg);
        let down = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_KANA, EventFlags::NONE), GateSnapshot::default(), Millis(0));
        assert_eq!(down.emitted()[0].flags, EventFlags(0x0004_0001));
        let up = arb.arbitrate(&cfg, &key_up(KeyCode::JIS_KANA, EventFlags::NONE), GateSnapshot::default(), Millis(10));
        assert_eq!(up.layer(), Layer::KoreanInput);
        assert_eq!(up.emitted()[0].kind, EventKind::KeyUp);
        assert_eq!(up.emitted()[0].keycode, KeyCode::SPACE);
        assert_eq!(up.emitted()[0].flags, EventFlags(0x0004_0001));

        // 한자 — IME Active 로 keyDown, keyUp 시점엔 Inactive 로 바뀌어도(D-K6:
        // "조건을 다시 평가하지 않는다") 여전히 같은 치환이 나가야 한다.
        let mut arb = Arbiter::new(&cfg);
        let active_gates = GateSnapshot {
            korean_ime: KoreanImeState::Active,
            ..Default::default()
        };
        let down = arb.arbitrate(&cfg, &key_down(KeyCode::JIS_EISU, EventFlags::NONE), active_gates, Millis(0));
        assert_eq!(down.emitted()[0].flags, EventFlags(0x0008_0040));

        let inactive_gates = GateSnapshot {
            korean_ime: KoreanImeState::Inactive,
            ..Default::default()
        };
        let up = arb.arbitrate(&cfg, &key_up(KeyCode::JIS_EISU, EventFlags::NONE), inactive_gates, Millis(20));
        assert_eq!(up.layer(), Layer::KoreanInput);
        assert_eq!(up.emitted()[0].kind, EventKind::KeyUp);
        assert_eq!(up.emitted()[0].keycode, KeyCode::RETURN);
        assert_eq!(up.emitted()[0].flags, EventFlags(0x0008_0040));
    }

    // ══════════════════════════════════════════════════════════════════════════
    // ⭐ K9 — `modifier 키와 함께 누른 문자 키를 영어 소문자로 입력`(이슈 #73,
    // D-K18). ⭐ 원본 SuperKey 에 없는 클론 고유 확장이다(명세 §3.7).
    // ══════════════════════════════════════════════════════════════════════════

    fn k9_config() -> EngineConfig {
        EngineConfig {
            korean_modifier_lowercase: true,
            ..EngineConfig::default()
        }
    }

    /// 시나리오 K9-1 — 옵션 ON + shift 눌린 상태에서 `A` keyDown → `Effect::TypeChar('a')`.
    /// 대문자 조합이 소문자로 인식된다(명세 §3.7).
    #[test]
    fn k9_option_shift_letter_emits_type_char_lowercase() {
        let cfg = k9_config();
        let mut arb = Arbiter::new(&cfg);

        // shift 를 실제 모양(FlagsChanged + device 비트)으로 누른다.
        arb.arbitrate(&cfg, &press_modifier(KeyCode::LEFT_SHIFT, 0x0002_0002), GateSnapshot::default(), Millis(0));

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_A, EventFlags(0x0002_0002)),
            GateSnapshot::default(),
            Millis(10),
        );

        assert_eq!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.effects(), &[Effect::TypeChar('a')]);
    }

    /// 시나리오 K9-2 — ⛔ command+A → 발화하지 않는다(앱 단축키 보호, §3.7). 통과다.
    #[test]
    fn k9_command_letter_passes_through_for_app_shortcuts() {
        let cfg = k9_config();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &press_modifier(KeyCode::LEFT_COMMAND, 0x0010_0008), GateSnapshot::default(), Millis(0));
        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_C, EventFlags(0x0010_0008)),
            GateSnapshot::default(),
            Millis(10),
        );

        assert_ne!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Pass);
    }

    /// 시나리오 K9-3 — 옵션 OFF(기본) → 아무 일도 하지 않는다. 기존 동작 보존.
    #[test]
    fn k9_option_off_changes_nothing() {
        let cfg = EngineConfig::default();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &press_modifier(KeyCode::LEFT_SHIFT, 0x0002_0002), GateSnapshot::default(), Millis(0));
        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode::ANSI_A, EventFlags(0x0002_0002)),
            GateSnapshot::default(),
            Millis(10),
        );

        assert_ne!(out.layer(), Layer::KoreanInput);
        assert!(out.effects().is_empty());
    }

    /// 시나리오 K9-4 — modifier 없이 문자 키만 누르면 발화하지 않는다 — 그 키의 원래
    /// 문자(한국어 입력기면 한글 자모)가 나가야 하기 때문이다(§3.7).
    #[test]
    fn k9_no_modifier_letter_passes_through() {
        let cfg = k9_config();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), GateSnapshot::default(), Millis(0));

        assert_ne!(out.layer(), Layer::KoreanInput);
        assert_eq!(out.disposition(), Disposition::Pass);
    }
}
