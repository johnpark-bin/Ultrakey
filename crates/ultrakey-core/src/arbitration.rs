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
use crate::keystate::KeyStateTable;
use crate::quickpress::{QuickPressConfig, QuickPressEvent, QuickPressState};
use crate::rules::{ComboRule, HoldCondition, ModifierKind, RuleAction};
use crate::settings::EngineConfig;
use crate::time::Millis;

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

    /// ⭐ D-1 caps lock 모멘터리 정규화(architecture.md §6.1). `cfg.caps_lock_alias` 가
    /// 설정돼 있고 이 이벤트의 keycode 가 그 alias(보통 F18, 경로 B `hidutil` 매핑의
    /// 대상)와 일치하면 `KeyCode::CAPS_LOCK` 으로 되돌린 사본을 반환한다.
    ///
    /// ⚠️ 이렇게 되돌린 keycode 는 **판정**(눌림 테이블 갱신, modifier/조합/quick press
    /// 매칭)에만 쓰인다. 이 함수가 반환하는 이벤트를 기준으로 이후 로직이 방출하는
    /// 합성 이벤트의 keycode 는 각 규칙이 스스로 정한다 — 예를 들어 조합·hold_remap 이
    /// 합성하는 이벤트의 keycode 는 대상 키(target key) 자신이지 caps lock 이 아니다.
    /// 유일하게 caps lock 자신의 keycode 로 나가는 것은 hyper/meh/bleh modifier 합성
    /// `FlagsChanged`(`rule.source == KeyCode::CAPS_LOCK`)뿐이며, 이는 D-1 이전에도
    /// 이미 그랬던 동작이라 alias 유무와 무관하게 일관적이다.
    fn resolve_caps_lock_alias(cfg: &EngineConfig, ev: &InputEvent) -> InputEvent {
        match cfg.caps_lock_alias {
            Some(alias) if ev.keycode == alias => InputEvent {
                keycode: KeyCode::CAPS_LOCK,
                ..*ev
            },
            _ => *ev,
        }
    }

    /// §3-b 계층 1~5. 계층 0(앱 게이트)·Secure Input 은 호출자가 이미 걸렀다.
    pub fn arbitrate(
        &mut self,
        cfg: &EngineConfig,
        ev_in: &InputEvent,
        seek_active: bool,
        now: Millis,
    ) -> Outcome {
        // 계층 1: Seek 세션 활성. M1 은 seek_active 가 항상 false 이지만, 이 분기 자체는
        // M3(F-01)를 위해 존재해야 한다(architecture.md §5).
        if seek_active {
            return Outcome::consume(Layer::SeekSession);
        }

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

        // 계층 2/3 통합 소스 키 핸들러(architecture.md §6.4 P1) — 추적 키(hyper/meh/bleh
        // 소스 또는 프리셋 액션 소스) 자신의 이벤트는 이 핸들러 하나가 결정한다.
        let is_tracked = cfg.rules.modifier_rule_for(ev.keycode).is_some()
            || cfg.rules.source_actions_for(ev.keycode).is_some();
        if is_tracked && (kind == EventKind::KeyDown || kind == EventKind::KeyUp) {
            return self.handle_tracked_key_event(cfg, ev, kind, now);
        }

        // 이하는 추적 대상이 아닌 키(또는 마우스 이벤트)의 경로다.
        let mut out = Outcome::pass(Layer::Passthrough);

        // P2 — 추적 대상이 아닌 키의 keyDown 은 모든 슬롯에 on_other_key_down 을 돌린다
        // (v1.62 예방의 핵심, §3-c 표 3행). M1 은 이 호출을 한 번도 하지 않았다.
        if kind == EventKind::KeyDown {
            self.dispatch_other_key_down(cfg, &mut out);
        }

        // 계층 3: Preset 조합. 트리거가 이 키이고 hold 조건이 성립하면 발화.
        if self.evaluate_combo_rules(cfg, ev, kind, &mut out) {
            return out;
        }

        // 계층 4: 단순 리매핑.
        if self.evaluate_simple_remap(cfg, ev, &mut out) {
            return out;
        }

        // 계층 2 의 "유지" 효과: hyper/meh/bleh 가 Active 인 동안 다른 키/마우스 이벤트에
        // modifier 를 얹는다(§3-b 계층 2 행, hyperkey.md §3.2 Active 행).
        let active = self.state.active_synth_flags();
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
        now: Millis,
    ) -> Outcome {
        match kind {
            EventKind::KeyDown => {
                // P6 — 이 키가 조합의 트리거이기도 하면 조합이 먼저다. FSM 을 건드리기 전에
                // 먼저 확인한다: 성립하면 그것을 발화하고 이 키의 FSM 을 Suppressed 로 만든
                // 뒤 Consume 한다. F-08.10(`Shift + caps lock = caps lock`)이 F-08.2(quick
                // press caps lock)를 무효화하는 것이 이 규칙의 구현이다(R3/v1.62).
                let mut out = Outcome::consume(Layer::PresetCombo);
                if let Some(rule) = self.find_matching_combo(&cfg.rules.combo_rules, ev.keycode) {
                    self.fire_combo(cfg, &rule, ev, &mut out);
                    return out;
                }

                let source_actions = cfg.rules.source_actions_for(ev.keycode).copied();
                let qp_cfg = self.quick_press_config_for(cfg, ev.keycode);
                let prev = self.state.machine(ev.keycode);
                let (next, event) = prev.on_key_down(now, ev.autorepeat, &qp_cfg);
                self.state.set_machine(ev.keycode, next);

                let mut out = Outcome::consume(Layer::HyperModifier);
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

                let mut out = Outcome::consume(Layer::HyperModifier);
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
        if let Some(rule) = cfg.rules.modifier_rule_for(key) {
            let flags = Self::strip_caps_lock_bit(cfg, base_flags | self.state.active_synth_flags());
            out.push(SynthEvent {
                kind: EventKind::FlagsChanged,
                keycode: rule.source,
                flags,
            });
            out.layer = Layer::HyperModifier;
        }
        if let Some(action) = cfg.rules.source_actions_for(key).and_then(|sa| sa.hold_remap) {
            Self::push_key_down(out, action);
            out.layer = Layer::PresetCombo;
        }
    }

    /// `key` 의 hold 가 해제됐을 때의 효과 — `emit_hold_start` 의 대응.
    fn emit_hold_end(&self, cfg: &EngineConfig, key: KeyCode, base_flags: EventFlags, out: &mut Outcome) {
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
        }
        if let Some(action) = cfg.rules.source_actions_for(key).and_then(|sa| sa.hold_remap) {
            Self::push_key_up(out, action);
            out.layer = Layer::PresetCombo;
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
            RuleAction::Key { keycode, flags } => {
                out.push(SynthEvent { kind: EventKind::KeyDown, keycode, flags });
                out.push(SynthEvent { kind: EventKind::KeyUp, keycode, flags });
            }
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
        out: &mut Outcome,
    ) -> bool {
        if kind != EventKind::KeyDown {
            return false;
        }
        if let Some(rule) = self.find_matching_combo(&cfg.rules.combo_rules, ev.keycode) {
            self.fire_combo(cfg, &rule, ev, out);
            true
        } else {
            false
        }
    }

    /// `rules`(이미 `RuleId` 오름차순으로 정렬돼 주어진다고 전제 — architecture.md §6.4
    /// 서두) 중 트리거가 `key` 이고 hold 조건이 성립하는 첫 규칙을 찾는다.
    fn find_matching_combo(&self, rules: &[ComboRule], key: KeyCode) -> Option<ComboRule> {
        rules
            .iter()
            .copied()
            .find(|r| r.trigger == key && self.hold_condition_met(r.hold))
    }

    /// P3 — 정본 눌림 테이블(`is_pressed`)·논리 hyper 신호(`is_kind_active`)로만 판정한다.
    /// `ev.flags` 의 modifier 비트로 판정하지 않는다.
    fn hold_condition_met(&self, cond: HoldCondition) -> bool {
        match cond {
            HoldCondition::Key(k) => self.state.is_pressed(k),
            HoldCondition::EitherShift => {
                self.state.is_pressed(KeyCode::LEFT_SHIFT) || self.state.is_pressed(KeyCode::RIGHT_SHIFT)
            }
            HoldCondition::EitherCommand => {
                self.state.is_pressed(KeyCode::LEFT_COMMAND) || self.state.is_pressed(KeyCode::RIGHT_COMMAND)
            }
            HoldCondition::HyperActive => self.state.is_kind_active(ModifierKind::Hyper),
        }
    }

    /// 조합 하나를 발화한다 — 액션을 `out` 에 반영하고, 트리거 키(그리고 P7 이 요구하는
    /// 경우 hold 키까지)의 FSM 을 `Suppressed` 로 전이시킨다.
    fn fire_combo(&mut self, _cfg: &EngineConfig, rule: &ComboRule, ev: &InputEvent, out: &mut Outcome) {
        out.layer = Layer::PresetCombo;
        out.disposition = Disposition::Consume;

        match rule.action {
            RuleAction::Key { keycode, flags: action_flags } => {
                // P5 — 조합이 낸 출력에는 hyper 합성 flags 와 caps lock 잠금 비트를 얹지
                // 않는다. `Caps lock + W = ▲` 의 의도는 방향키이지 `⌃⌥⌘⇧▲` 가 아니다.
                let base = ev.flags & !self.state.active_synth_flags() & !EventFlags::CAPS_LOCK;
                let flags = base | action_flags;
                out.push(SynthEvent { kind: EventKind::KeyDown, keycode, flags });
                out.push(SynthEvent { kind: EventKind::KeyUp, keycode, flags });
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
                if cfg.rules.modifier_rule_for(key).is_some() {
                    out.push(SynthEvent {
                        kind: EventKind::FlagsChanged,
                        keycode: key,
                        flags: EventFlags::NONE,
                    });
                    out.layer = Layer::HyperModifier;
                }
                if let Some(action) = cfg.rules.source_actions_for(key).and_then(|sa| sa.hold_remap) {
                    Self::push_key_up(&mut out, action);
                    out.layer = Layer::PresetCombo;
                }
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
    use crate::rules::{ModifierKind, ModifierRule, RuleId, SimpleRemap, SourceKeyActions};

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

        let caps_down = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        assert_eq!(caps_down.layer(), Layer::HyperModifier);
        assert_eq!(caps_down.disposition(), Disposition::Consume);
        assert_eq!(caps_down.emitted().len(), 1);
        assert_eq!(caps_down.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);

        let w_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_W, EventFlags::NONE), false, Millis(10));
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

        let shift_down = arb.arbitrate(&cfg, &key_down(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), false, Millis(0));
        assert_eq!(shift_down.layer(), Layer::Passthrough);

        let caps_down = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::SHIFT), false, Millis(5));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
        assert_eq!(caps_down.disposition(), Disposition::Consume);

        let caps_up = arb.arbitrate(&cfg, &key_up(KeyCode::CAPS_LOCK, EventFlags::SHIFT), false, Millis(20));
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

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
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
        let meh_out = meh_arb.arbitrate(&meh_cfg, &key_down(KeyCode::RIGHT_OPTION, EventFlags::NONE), false, Millis(0));
        assert_eq!(meh_out.emitted()[0].flags, EventFlags::MEH);

        let mut bleh_cfg = EngineConfig::default();
        bleh_cfg.rules.modifier_rules.push(ModifierRule {
            source: KeyCode::RIGHT_COMMAND,
            kind: ModifierKind::Bleh,
            flags: EventFlags::BLEH,
        });
        let mut bleh_arb = Arbiter::new(&bleh_cfg);
        let bleh_out = bleh_arb.arbitrate(&bleh_cfg, &key_down(KeyCode::RIGHT_COMMAND, EventFlags::NONE), false, Millis(0));
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
        arb.arbitrate(&cfg, &key_down(KeyCode::RIGHT_OPTION, EventFlags::NONE), false, Millis(0));
        let bleh_out = arb.arbitrate(&cfg, &key_down(KeyCode::RIGHT_COMMAND, EventFlags::NONE), false, Millis(1));

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

        let first = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        assert_eq!(first.emitted().len(), 1);

        let mut repeat_ev = key_down(KeyCode::CAPS_LOCK, EventFlags::NONE);
        repeat_ev.autorepeat = true;
        let repeat = arb.arbitrate(&cfg, &repeat_ev, false, Millis(50));
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

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        assert_eq!(out.layer(), Layer::HyperModifier);
        assert_eq!(out.emitted()[0].keycode, KeyCode::CAPS_LOCK);
        assert_ne!(out.emitted()[0].keycode, KeyCode::LEFT_CONTROL);
    }

    /// 계층 1(Seek)이 성립하면 계층 2 도 평가되지 않는다.
    #[test]
    fn seek_layer_short_circuits_everything_below() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), true, Millis(0));
        assert_eq!(out.layer(), Layer::SeekSession);
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle);
    }

    /// 테스트 #8 — force_reset 이 합성 중이던 modifier 에 대해 off flagsChanged 를 방출.
    #[test]
    fn force_reset_emits_off_for_active_modifiers() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);
        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
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
        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));

        let click_ev = InputEvent {
            kind: EventKind::LeftMouseDown,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        let click_out = arb.arbitrate(&cfg, &click_ev, false, Millis(10));
        assert_eq!(click_out.disposition(), Disposition::PassWithFlags(EventFlags::HYPER_WITH_SHIFT));

        let drag_ev = InputEvent {
            kind: EventKind::LeftMouseDragged,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        let drag_out = arb.arbitrate(&cfg, &drag_ev, false, Millis(11));
        assert_eq!(drag_out.disposition(), Disposition::Pass);
    }

    /// 테스트 #11 — quick press 액션이 등록되지 않은 소스 키는 keyDown 즉시 HoldConfirmed.
    #[test]
    fn modifier_source_without_quick_press_action_confirms_immediately() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle);

        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
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

        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted().len(), 1);
        assert_eq!(out.emitted()[0].kind, EventKind::FlagsChanged);
        assert_eq!(out.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);
    }

    #[test]
    fn flags_changed_second_occurrence_normalizes_to_key_up() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(10));

        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted().len(), 1);
        assert_eq!(out.emitted()[0].kind, EventKind::FlagsChanged);
        assert!(!out.emitted()[0].flags.contains(EventFlags::HYPER_WITH_SHIFT));
    }

    #[test]
    fn flags_changed_hold_lets_other_key_down_carry_all_four_hyper_flags() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let caps = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        assert_eq!(caps.disposition(), Disposition::Consume);

        let a_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), false, Millis(10));
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

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        let a_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), false, Millis(10));

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

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        assert!(arb.state.is_pressed(KeyCode::CAPS_LOCK), "첫 FlagsChanged 이후 정본 눌림 테이블은 눌림으로 기록돼야 한다");

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(10));
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

        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), false, Millis(0));
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

        let f13_down = arb.arbitrate(&cfg, &key_down(KeyCode::F13, EventFlags::NONE), false, Millis(0));
        assert_eq!(f13_down.disposition(), Disposition::Consume);
        assert_eq!(f13_down.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);

        let a_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), false, Millis(10));
        assert_eq!(a_down.disposition(), Disposition::PassWithFlags(EventFlags::HYPER_WITH_SHIFT));

        let f13_up = arb.arbitrate(&cfg, &key_up(KeyCode::F13, EventFlags::NONE), false, Millis(20));
        assert_eq!(f13_up.disposition(), Disposition::Consume);
        assert!(!f13_up.emitted()[0].flags.contains(EventFlags::HYPER_WITH_SHIFT));
    }

    #[test]
    fn force_reset_then_flags_changed_normalizes_to_key_down_again() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        assert!(arb.state.is_pressed(KeyCode::CAPS_LOCK));

        arb.force_reset(&cfg);
        assert!(!arb.state.is_pressed(KeyCode::CAPS_LOCK), "force_reset 은 정본 눌림 테이블도 지운다");

        let out = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(100));
        assert_eq!(out.disposition(), Disposition::Consume);
        assert_eq!(out.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);
    }

    #[test]
    fn flags_changed_activated_hyper_applies_to_mouse_click_but_not_drag() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);
        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));

        let click_ev = InputEvent {
            kind: EventKind::LeftMouseDown,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        let click_out = arb.arbitrate(&cfg, &click_ev, false, Millis(10));
        assert_eq!(click_out.disposition(), Disposition::PassWithFlags(EventFlags::HYPER_WITH_SHIFT));

        let drag_ev = InputEvent {
            kind: EventKind::LeftMouseDragged,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
            autorepeat: false,
        };
        let drag_out = arb.arbitrate(&cfg, &drag_ev, false, Millis(11));
        assert_eq!(drag_out.disposition(), Disposition::Pass);
    }

    #[test]
    fn caps_lock_source_strips_alpha_shift_from_synthesized_event() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let ev = flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK);
        let out = arb.arbitrate(&cfg, &ev, false, Millis(0));

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
            false,
            Millis(0),
        );

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode(0x00), EventFlags::CAPS_LOCK),
            false,
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
            false,
            Millis(0),
        );

        let out = arb.arbitrate(
            &cfg,
            &key_down(KeyCode(0x00), EventFlags::CAPS_LOCK),
            false,
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

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), false, Millis(0));
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

        let out = arb.arbitrate(&cfg, &key_down(KeyCode::F18, EventFlags::NONE), false, Millis(0));
        assert_eq!(out.layer(), Layer::Passthrough);
        assert_eq!(out.disposition(), Disposition::Pass);
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

            arb.arbitrate(&cfg, &key_down(source, EventFlags::NONE), false, Millis(0));
            let out = arb.arbitrate(&cfg, &key_down(KeyCode::DELETE, EventFlags::NONE), false, Millis(10));

            assert_eq!(out.layer(), Layer::PresetCombo, "source={source:?}");
            assert_eq!(out.disposition(), Disposition::Consume);
            assert_eq!(out.emitted().len(), 2);
            assert_eq!(out.emitted()[0].kind, EventKind::KeyDown);
            assert_eq!(out.emitted()[0].keycode, KeyCode::FORWARD_DELETE);
            assert_eq!(out.emitted()[1].kind, EventKind::KeyUp);
        }
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

        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        let out = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_W, EventFlags::NONE), false, Millis(10));

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
        arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), false, Millis(0));

        // caps lock 을 톡 눌렀다 뗀다(quick press 조건을 만족할 만큼 짧게).
        let down = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::SHIFT), false, Millis(5));
        assert_eq!(down.disposition(), Disposition::Consume);
        assert_eq!(down.layer(), Layer::PresetCombo);
        assert_eq!(down.effects(), &[Effect::ToggleCapsLock], "P6: shift+caps 조합만 발화해야 한다");
        assert!(down.emitted().is_empty(), "조합은 캡스락 키 이벤트를 합성하지 않는다(ToggleCapsLock 은 effect)");

        let up = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::SHIFT), false, Millis(50));
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
        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        let up = arb.arbitrate(&cfg, &key_up(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(100));
        assert_eq!(up.emitted().len(), 2);
        assert_eq!(up.emitted()[0].keycode, KeyCode::ESCAPE);
        assert!(!up.emitted().iter().any(|e| e.keycode == KeyCode::LEFT_CONTROL));
    }

    /// P2 — quick press 액션이 등록된 키가 `PendingDown` 인 동안 다른 키가 눌리면 즉시
    /// `HoldConfirmed` 가 되고, hold_remap 대상 키의 `KeyDown` 이 방출된다.
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

        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::PendingDown { since: Millis(0) });

        // 다른 키(A)의 keyDown — P2 에 의해 즉시 HoldConfirmed 로 확정돼야 한다.
        let a_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_A, EventFlags::NONE), false, Millis(10));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
        // hold_remap 대상(LEFT_CONTROL)의 KeyDown 이 A 의 처리에 실려 나와야 한다.
        assert!(
            a_down.emitted().iter().any(|e| e.keycode == KeyCode::LEFT_CONTROL && e.kind == EventKind::KeyDown),
            "P2 로 확정된 hold_remap 의 KeyDown 이 없다: {:?}",
            a_down.emitted()
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
        let left_down = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), false, Millis(0));
        assert!(left_down.effects().is_empty());
        assert_eq!(arb.state.machine(KeyCode::LEFT_SHIFT), QuickPressState::PendingDown { since: Millis(0) });

        // 오른쪽 shift 다운 — P6 조합 우선 체크가 오른쪽 자신의 FSM 진입보다 먼저 발화해야 한다.
        let right_down = arb.arbitrate(&cfg, &flags_changed(KeyCode::RIGHT_SHIFT, EventFlags::SHIFT), false, Millis(5));
        assert_eq!(right_down.effects(), &[Effect::ToggleCapsLock]);
        assert_eq!(arb.state.machine(KeyCode::LEFT_SHIFT), QuickPressState::Suppressed, "P7: hold 키도 Suppressed 여야 한다");
        assert_eq!(arb.state.machine(KeyCode::RIGHT_SHIFT), QuickPressState::Suppressed);

        // 두 shift 를 뗀다 — 토글이 추가로 나오면 안 된다.
        let left_up = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), false, Millis(10));
        assert!(left_up.effects().is_empty());
        let right_up = arb.arbitrate(&cfg, &flags_changed(KeyCode::RIGHT_SHIFT, EventFlags::SHIFT), false, Millis(10));
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

        arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), false, Millis(0));
        let left_up = arb.arbitrate(&cfg, &flags_changed(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), false, Millis(50));
        assert_eq!(left_up.effects(), &[Effect::TypeChar('(')]);

        arb.arbitrate(&cfg, &flags_changed(KeyCode::RIGHT_SHIFT, EventFlags::SHIFT), false, Millis(100));
        let right_up = arb.arbitrate(&cfg, &flags_changed(KeyCode::RIGHT_SHIFT, EventFlags::SHIFT), false, Millis(150));
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

        let down = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK), false, Millis(0));
        assert!(down.effects().is_empty());
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::PendingDown { since: Millis(0) });

        let up = arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(300));
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

        arb.arbitrate(&cfg, &flags_changed(KeyCode::CAPS_LOCK, EventFlags::CAPS_LOCK), false, Millis(0));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::PendingDown { since: Millis(0) });

        let tick = arb.on_tick(&cfg, Millis(1200));
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
        // 이 키는 modifier 소스가 아니므로 flagsChanged 는 나오지 않는다.
        assert!(tick.emitted().is_empty());

        let w_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_W, EventFlags::NONE), false, Millis(1210));
        assert_eq!(w_down.layer(), Layer::PresetCombo);
        assert_eq!(w_down.emitted()[0].keycode, KeyCode::UP_ARROW);
    }
}
