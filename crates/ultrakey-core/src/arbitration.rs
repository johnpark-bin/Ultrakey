//! ⭐ 이 크레이트의 본체 — `key-remapping-engine.md` §3-b 중재 우선순위 표의 구현.
//!
//! `Arbiter::arbitrate` 는 힙 할당을 하지 않는다. `Outcome` 의 emit 버퍼는 고정 크기
//! 인라인 배열이다(`docs/dev/architecture.md` §2.2 "콜백 안에서 절대 하지 않는 것").
//!
//! 계층 0(앱 게이트, `gate.rs`)과 Secure Input 확인은 호출자(`ultrakey-engine`)가
//! `arbitrate` 를 부르기 **이전에** 이미 걸렀다는 전제다(`docs/dev/architecture.md` §2.3).
//! 이 모듈은 §3-b 의 계층 1~5 만 다룬다.

use crate::event::{EventKind, InputEvent};
use crate::flags::EventFlags;
use crate::keycode::KeyCode;
use crate::keystate::KeyStateTable;
use crate::quickpress::{QuickPressConfig, QuickPressEvent, QuickPressState};
use crate::rules::ModifierRule;
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

const MAX_EMIT: usize = 4;

/// 중재 결과. 최대 4개 인라인 버퍼 + disposition — 힙 할당이 없다.
#[derive(Debug, Clone, Copy)]
pub struct Outcome {
    buf: [SynthEvent; MAX_EMIT],
    len: usize,
    disposition: Disposition,
    layer: Layer,
}

impl Outcome {
    fn new(layer: Layer, disposition: Disposition) -> Self {
        // 플레이스홀더 값. len 범위 밖은 절대 읽지 않으므로 내용 자체는 의미가 없다.
        let placeholder = SynthEvent {
            kind: EventKind::FlagsChanged,
            keycode: KeyCode(0),
            flags: EventFlags::NONE,
        };
        Outcome {
            buf: [placeholder; MAX_EMIT],
            len: 0,
            disposition,
            layer,
        }
    }

    fn pass(layer: Layer) -> Self {
        Self::new(layer, Disposition::Pass)
    }

    fn pass_with_flags(layer: Layer, flags: EventFlags) -> Self {
        Self::new(layer, Disposition::PassWithFlags(flags))
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

    pub fn emitted(&self) -> &[SynthEvent] {
        &self.buf[..self.len]
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
        state.register_modifier_sources(&cfg.rules);
        Arbiter { state }
    }

    /// 규칙/설정이 바뀔 때 호출 — quick press 슬롯 구성을 다시 계산한다. 기존 활성 상태는
    /// 가능한 한 보존된다(`KeyStateTable::register_modifier_sources` 참고).
    pub fn reconfigure(&mut self, cfg: &EngineConfig) {
        self.state.register_modifier_sources(&cfg.rules);
    }

    fn quick_press_config(&self, cfg: &EngineConfig) -> QuickPressConfig {
        QuickPressConfig {
            quick_press_duration: Millis(cfg.timings.quick_press_duration_ms),
            double_tap_interval: Millis(cfg.timings.double_tap_interval_ms),
            // ⭐ M1 재해석(quickpress.rs 모듈 문서 참고): modifier 규칙에는 아직 quick press
            // 액션 슬롯 자체가 없으므로 항상 false 다. M2 가 이 값을 실제로 조회해 채운다.
            has_quick_press_action: false,
            has_double_tap_action: false,
        }
    }

    /// §3-b 계층 1~5. 계층 0(앱 게이트)·Secure Input 은 호출자가 이미 걸렀다.
    pub fn arbitrate(
        &mut self,
        cfg: &EngineConfig,
        ev: &InputEvent,
        seek_active: bool,
        now: Millis,
    ) -> Outcome {
        // 계층 1: Seek 세션 활성. M1 은 seek_active 가 항상 false 이지만, 이 분기 자체는
        // M3(F-01)를 위해 존재해야 한다(architecture.md §5).
        if seek_active {
            return Outcome::consume(Layer::SeekSession);
        }

        // 정본 상태 갱신 — 이후 모든 계층이 "이 갱신 이후의" 상태를 공유해서 읽는다.
        // v1.20 예방의 핵심: hyper 판정과 preset 판정이 서로 다른 사본을 보지 않는다.
        match ev.kind {
            EventKind::KeyDown => self.state.set_pressed(ev.keycode, true),
            EventKind::KeyUp => self.state.set_pressed(ev.keycode, false),
            _ => {}
        }

        // 계층 2: hyper/meh/bleh 소스 키 자체의 이벤트.
        if let Some(rule) = cfg.rules.modifier_rule_for(ev.keycode).copied() {
            if let Some(out) = self.handle_modifier_source_event(&rule, ev, now, cfg) {
                return out;
            }
        }

        // 계층 3: Preset 조합(M2/F-08 자리) — M1 에는 규칙이 없어 항상 None 이지만,
        // 분기 자체는 존재해야 한다(architecture.md §5).
        if let Some(out) = self.evaluate_combo_rules(cfg, ev) {
            return out;
        }

        // 계층 4: 단순 리매핑(M2/F-08 자리) — M1 에는 규칙이 없어 항상 None.
        if let Some(out) = self.evaluate_simple_remap(cfg, ev) {
            return out;
        }

        // 계층 2 의 "유지" 효과: hyper/meh/bleh 가 Active 인 동안 다른 키/마우스 이벤트에
        // modifier 를 얹는다(§3-b 계층 2 행, hyperkey.md §3.2 Active 행).
        let active = self.state.active_synth_flags();
        if !active.is_empty() {
            if ev.kind.is_key() {
                return Outcome::pass_with_flags(Layer::HyperModifier, ev.flags | active);
            }
            if ev.kind.is_click() || ev.kind.is_drag() || ev.kind.is_move() || ev.kind.is_scroll() {
                return if self.mouse_should_apply(cfg, ev.kind) {
                    Outcome::pass_with_flags(Layer::HyperModifier, ev.flags | active)
                } else {
                    Outcome::pass(Layer::HyperModifier)
                };
            }
        }

        // 계층 5: 통과.
        Outcome::pass(Layer::Passthrough)
    }

    /// 이벤트의 keycode 가 hyper/meh/bleh 소스 키 자신일 때의 처리(§3-b 계층 2, §3-c).
    fn handle_modifier_source_event(
        &mut self,
        rule: &ModifierRule,
        ev: &InputEvent,
        now: Millis,
        cfg: &EngineConfig,
    ) -> Option<Outcome> {
        let qp_cfg = self.quick_press_config(cfg);

        match ev.kind {
            EventKind::KeyDown => {
                let prev = self.state.machine(rule.source);
                let (next, event) = prev.on_key_down(now, ev.autorepeat, &qp_cfg);
                self.state.set_machine(rule.source, next);

                let mut out = Outcome::consume(Layer::HyperModifier);
                if matches!(event, Some(QuickPressEvent::HoldStart)) {
                    // ⭐ 여러 조합이 동시에 활성이면 OR 로 합산한다(keystate.rs 문서 참고).
                    // active_synth_flags() 는 방금 반영한 next 상태를 포함해 계산된다.
                    let flags = ev.flags | self.state.active_synth_flags();
                    out.push(SynthEvent {
                        kind: EventKind::FlagsChanged,
                        keycode: rule.source,
                        flags,
                    });
                }
                // PendingDown 유지 등 그 외 결과는 원본 keyDown 을 보류(소비)만 한다 — emit 없음.
                Some(out)
            }
            EventKind::KeyUp => {
                let prev = self.state.machine(rule.source);
                let (next, event) = prev.on_key_up(now, &qp_cfg);
                self.state.set_machine(rule.source, next);

                let mut out = Outcome::consume(Layer::HyperModifier);
                if matches!(event, Some(QuickPressEvent::HoldEnd)) {
                    // 이 규칙의 flags 는 벗기되, 다른 조합이 여전히 active 라면 그 flags 는
                    // 유지한다(공유 비트가 있는 hyper/meh/bleh 조합 중 하나만 놓아도 나머지가
                    // 살아 있어야 하므로).
                    let remaining = self.state.active_synth_flags();
                    let flags = (ev.flags & !rule.flags) | remaining;
                    out.push(SynthEvent {
                        kind: EventKind::FlagsChanged,
                        keycode: rule.source,
                        flags,
                    });
                }
                // QuickPress/DoubleTap 방출은 M1 에 해당 액션이 없어 도달하지 않는다.
                Some(out)
            }
            _ => None,
        }
    }

    /// 계층 3 — M2/F-08 자리. `ComboRule` 에 아직 필드가 없어 평가할 것이 없다.
    fn evaluate_combo_rules(&mut self, cfg: &EngineConfig, _ev: &InputEvent) -> Option<Outcome> {
        for _combo in &cfg.rules.combo_rules {
            // TODO(M2/F-08): ComboRule 매칭 로직. M1 에서는 항상 비어 있어 실행되지 않는다.
        }
        None
    }

    /// 계층 4 — M2/F-08 자리. M1 에는 등록된 `SimpleRemap` 이 없어 항상 `None` 이지만,
    /// 로직 자체는 미리 구현해 둔다(들어올 때 이 함수만 그대로 쓰면 된다).
    fn evaluate_simple_remap(&self, cfg: &EngineConfig, ev: &InputEvent) -> Option<Outcome> {
        if !(ev.kind == EventKind::KeyDown || ev.kind == EventKind::KeyUp) {
            return None;
        }
        let remap = cfg.rules.simple_remaps.iter().find(|r| r.from == ev.keycode)?;
        let mut out = Outcome::consume(Layer::SimpleRemap);
        out.push(SynthEvent {
            kind: ev.kind,
            keycode: remap.to,
            flags: ev.flags,
        });
        Some(out)
    }

    fn mouse_should_apply(&self, cfg: &EngineConfig, kind: EventKind) -> bool {
        (kind.is_click() && cfg.mouse_apply.click)
            || (kind.is_drag() && cfg.mouse_apply.drag)
            || (kind.is_move() && cfg.mouse_apply.r#move)
            || (kind.is_scroll() && cfg.mouse_apply.scroll)
    }

    /// quick press 타이머 만료 처리(§3-c). 실제 시계 대신 호출자가 `now` 를 준다.
    pub fn on_tick(&mut self, cfg: &EngineConfig, now: Millis) -> Outcome {
        let qp_cfg = self.quick_press_config(cfg);
        let mut out = Outcome::pass(Layer::Passthrough);

        for idx in 0..self.state.slot_count() {
            let (key, _rule_flags, state) = self.state.slot_at(idx);
            let (next, event) = state.on_tick(now, &qp_cfg);
            if next != state {
                self.state.set_machine(key, next);
            }
            if matches!(event, Some(QuickPressEvent::HoldStart)) {
                let flags = self.state.active_synth_flags();
                out.layer = Layer::HyperModifier;
                out.push(SynthEvent {
                    kind: EventKind::FlagsChanged,
                    keycode: key,
                    flags,
                });
            }
            // WaitingSecondTap 타임아웃으로 유예된 QuickPress 방출은 M1 에 해당 액션이 없어
            // 무시한다(M2 가 quick press 액션을 등록하면 이 자리에서 처리한다).
        }
        out
    }

    /// 절전/잠금/Secure Input 진입 — stuck modifier 방지(§5 엣지 6·9).
    /// 합성 중이던 modifier 를 전부 off 로 방출한 뒤 내부 상태를 리셋한다.
    pub fn force_reset(&mut self) -> Outcome {
        let mut out = Outcome::pass(Layer::Passthrough);

        for idx in 0..self.state.slot_count() {
            let (key, rule_flags, state) = self.state.slot_at(idx);
            if matches!(state, QuickPressState::HoldConfirmed) {
                let _ = rule_flags; // 참고용 — off 이벤트는 flags 없이(전부 해제) 내보낸다.
                out.layer = Layer::HyperModifier;
                out.push(SynthEvent {
                    kind: EventKind::FlagsChanged,
                    keycode: key,
                    flags: EventFlags::NONE,
                });
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
    use crate::rules::{ModifierKind, ModifierRule, SimpleRemap};

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

    /// 테스트 #2 — v1.20 재현: caps lock 이 hyper 소스이면서 동시에 계층 3 트리거일 수
    /// 있어야 한다. M1 에는 계층 3 규칙이 없지만, "계층 2 가 계층 3 평가를 막지 않는다"는
    /// 구조 자체(공유 상태 하나에서 두 계층이 동시에 사실을 읽는다)를 검증한다: caps lock
    /// hold 확정 이후 W 의 keyDown 이 hyper flags 를 얹은 채 통과해야 한다.
    #[test]
    fn v1_20_caps_lock_hold_lets_w_pass_with_hyper_flags() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        let caps_down = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        assert_eq!(caps_down.layer(), Layer::HyperModifier);
        assert_eq!(caps_down.disposition(), Disposition::Consume);
        assert_eq!(caps_down.emitted().len(), 1);
        assert_eq!(caps_down.emitted()[0].flags, EventFlags::HYPER_WITH_SHIFT);

        // W 는 hyper 소스도 아니고(계층 2 자기 이벤트 아님), combo/simple 규칙도 없다(계층
        // 3·4 는 평가되어 None 을 반환) — 그래도 hyper Active 상태가 공유 상태에 남아 있으므로
        // "유지" 경로(여전히 계층 2 의 효과)가 W 에 hyper flags 를 얹어 통과시킨다.
        let w_down = arb.arbitrate(&cfg, &key_down(KeyCode::ANSI_W, EventFlags::NONE), false, Millis(10));
        assert_eq!(w_down.layer(), Layer::HyperModifier);
        assert_eq!(w_down.disposition(), Disposition::PassWithFlags(EventFlags::HYPER_WITH_SHIFT));
    }

    /// 테스트 #3 — v1.62 재현: shift 를 누른 채 caps lock keyDown → quick press 머신이
    /// 즉시 HoldConfirmed 가 되어 quick press(짧게 눌렀다 뗀 것으로 오인) 가 절대 발생하지
    /// 않는다. M1 재해석(quick press 액션 없음)에 의해 caps lock 은 항상 즉시 확정된다.
    #[test]
    fn v1_62_shift_then_caps_lock_never_quick_presses() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);

        // 물리 shift 는 이 크레이트 관점에서는 그냥 눌린 키 하나 — hyper 소스가 아니므로
        // 정본 pressed 비트셋에만 반영된다.
        let shift_down = arb.arbitrate(&cfg, &key_down(KeyCode::LEFT_SHIFT, EventFlags::SHIFT), false, Millis(0));
        assert_eq!(shift_down.layer(), Layer::Passthrough);

        let caps_down = arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::SHIFT), false, Millis(5));
        // 즉시 HoldConfirmed — PendingDown 을 거치지 않는다.
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
        assert_eq!(caps_down.disposition(), Disposition::Consume);

        // 아주 짧게 뗐다 — quick press 라면 여기서 QuickPress 류 이벤트가 나야 하지만,
        // 이미 HoldConfirmed 이므로 HoldEnd 만 나오고 quick press 는 발생하지 않는다.
        let caps_up = arb.arbitrate(&cfg, &key_up(KeyCode::CAPS_LOCK, EventFlags::SHIFT), false, Millis(20));
        assert_eq!(caps_up.emitted().len(), 1);
        assert_eq!(caps_up.emitted()[0].kind, EventKind::FlagsChanged);
        // HoldEnd 로 flags 가 hyper 비트를 벗겨낸 값이어야 한다(quick press 액션 이벤트가 아님).
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
    /// 방지). meh 와 bleh 를 동시에 누르면 OR 합산으로 meh 의 ALTERNATE 비트가 결과에 섞여
    /// 들어오므로, "bleh 단독일 때 option 이 없다"는 각각 독립된 Arbiter 로 검증한다.
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

    /// 동시에 활성화된 여러 조합은 OR 로 합산된다(`keystate.rs` 문서의 설계 결정) —
    /// meh 가 여전히 눌려 있는 동안 bleh 를 누르면 두 조합의 비트가 모두 실린다.
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

        // meh(⌃⌥⇧) 와 bleh(⌃⌘⇧) 가 동시에 활성 — 합산 결과는 ⌃⌥⌘⇧(옵션 포함) 이 된다.
        let flags = bleh_out.emitted()[0].flags;
        assert!(flags.contains(EventFlags::MEH));
        assert!(flags.contains(EventFlags::BLEH));
        assert!(flags.contains(EventFlags::ALTERNATE));
    }

    /// 테스트 #6 — auto-repeat keyDown 이 PendingDown 으로 되돌리지 않는다(§5 엣지 13).
    /// M1 은 has_quick_press_action=false 라 애초에 PendingDown 이 없지만, HoldConfirmed
    /// 상태에서 autorepeat 가 반복돼도 HoldStart 를 다시 emit 하지 않아야 한다.
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
    /// 계층 2(hyper 소스 자기 자신 이벤트)가 성립하면 계층 4(단순 리매핑)가 같은 keycode 에
    /// 등록돼 있어도 무시되어야 한다.
    #[test]
    fn higher_layer_short_circuits_lower_layer() {
        let mut cfg = hyper_config();
        // 같은 caps lock 에 단순 리매핑도 등록해 둔다 — 계층 2 가 이겨야 한다.
        cfg.rules.simple_remaps.push(SimpleRemap {
            from: KeyCode::CAPS_LOCK,
            to: KeyCode::LEFT_CONTROL,
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
        // Seek 가 소비했으므로 caps lock 의 quick press 머신은 전혀 건드려지지 않는다.
        assert_eq!(arb.state.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle);
    }

    /// 테스트 #8 — force_reset 이 합성 중이던 modifier 에 대해 off flagsChanged 를 방출.
    #[test]
    fn force_reset_emits_off_for_active_modifiers() {
        let cfg = hyper_config();
        let mut arb = Arbiter::new(&cfg);
        arb.arbitrate(&cfg, &key_down(KeyCode::CAPS_LOCK, EventFlags::NONE), false, Millis(0));
        assert!(!arb.state.active_synth_flags().is_empty());

        let out = arb.force_reset();
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
}
