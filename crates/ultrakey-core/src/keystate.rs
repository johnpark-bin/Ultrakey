//! ⭐ 정본(canonical) 공유 상태 — `key-remapping-engine.md` §3-b 가 요구하는 "물리 키 하나의
//! 현재 상태는 시스템 전체에서 단 하나로만 존재한다"는 원칙을 코드로 표현한 자리다.
//! Seek/Hyperkey/Presets 는 이 테이블을 **읽기만** 한다.
//!
//! 콜백 임계 경로에서 호출되므로 힙 할당이 없다: 눌림 여부는 고정 크기 비트셋
//! (`[u64; 4]` = 256비트)으로, quick press 상태 머신은 작은 고정 배열로 저장한다.

use crate::flags::EventFlags;
use crate::keycode::KeyCode;
use crate::quickpress::QuickPressState;
use crate::rules::{LanguageTrigger, ModifierKind, RuleAction, RuleId, RuleTable};

/// 동시에 추적 가능한 quick press 소스 키의 최대 개수. M1 은 hyper/meh/bleh 최대 3개뿐이라
/// 8개면 여유롭다 — 필요해지면 이 상수만 올리면 된다.
const MAX_TRACKED_KEYS: usize = 8;

/// ⭐ F-16 "치환 중" 래치의 최대 동시 개수(`docs/spec/korean-input.md` §3.3·§5#9, D-K6).
/// F-16 규칙은 4종(space/lang1/lang2/grave)뿐이라 4면 절대 넘치지 않는다.
const MAX_KOREAN_LATCH: usize = 4;

/// ⭐ F-19 "치환 중" 래치의 최대 동시 개수(`docs/spec/language-presets.md` §3, D-K6 동형).
/// F-19 의 KeyDown 치환 규칙(F-19.5·F-19.6)이 트리거로 주장하는 **고유 키코드** 수:
/// F-19.5 는 0x5D/0x2A(2 개), F-19.6 20행은 from 키 14개(0x13·0x16·0x1A·0x1C·0x19·0x1D·
/// 0x1B·0x18·0x5D·0x21·0x1E·0x29·0x27·0x2A). 합집합 14개 < 16. AloneTap 규칙(F-19.1~4·7)
/// 은 QuickPress 발화가 키 down+up 을 통째로 내므로 래치가 필요 없다.
const MAX_LANGUAGE_LATCH: usize = 16;

/// keyDown 이 치환을 발화시켰을 때 세우는 래치 하나 — 대응하는 keyUp 이 도착하면
/// **조건을 다시 평가하지 않고** 이 값 그대로 keyUp 을 합성한다(D-K6). 힙 할당 없이
/// 고정 크기 배열에 담는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KoreanLatch {
    pub trigger_key: KeyCode,
    pub out_keycode: KeyCode,
    pub out_flags: EventFlags,
    /// ⭐ F-18 Event Viewer(`docs/spec/event-viewer.md` §3.4) — 이 래치를 세운 규칙.
    /// keyUp 이 도착했을 때 `Outcome::rule()` 이 keyDown 때와 같은 값을 돌려주도록
    /// 래치에 실어 둔다(조건을 다시 평가하지 않는 D-K6 규약과 같은 이유).
    pub id: RuleId,
}

/// 비트셋이 표현 가능한 keycode 범위. macOS virtual keycode 는 실측 범위(§4 키코드 목록)가
/// 전부 이 안에 든다 — 그 이상 값은 (있을 수 없다고 보고) 조용히 무시한다.
const BITSET_BITS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MachineSlot {
    key: KeyCode,
    /// 이 소스 키가 hold 로 확정됐을 때 얹을 modifier flags. 등록 시점(`RuleTable`)에서
    /// 캐시해 둔다 — 콜백 임계 경로에서 `RuleTable` 을 다시 찾아보지 않기 위함.
    /// 이 키가 modifier 소스가 아니면(프리셋 액션만 있는 키) `EventFlags::NONE`.
    rule_flags: EventFlags,
    /// 이 키가 hyper/meh/bleh 소스이면 그 종류. 아니면 `None`(프리셋 전용 추적 키,
    /// M2/F-08).
    kind: Option<ModifierKind>,
    /// 이 키에 quick press 액션이 등록되어 있는가(M2/F-08 — `SourceKeyActions::quick_press`).
    has_quick_press: bool,
    /// 이 키에 double tap 액션이 등록되어 있는가(M2/F-08 — `SourceKeyActions::double_tap`).
    has_double_tap: bool,
    state: QuickPressState,
}

/// 정본 키 상태 테이블. `Arbiter` 가 배타 소유한다(`docs/dev/architecture.md` §2.2 —
/// 탭 스레드 전용, 락 없이 `&mut` 로만 접근).
pub struct KeyStateTable {
    /// 눌림 여부 비트셋. 인덱스 = keycode 값.
    pressed: [u64; 4],
    /// quick press 상태 머신 슬롯. 항상 앞에서부터 빈틈없이(index 0..slot_count) 채운다 —
    /// `register_sources` 가 매번 전체를 다시 구성하기 때문에 이 불변식이 성립한다.
    slots: [Option<MachineSlot>; MAX_TRACKED_KEYS],
    /// F-16 "치환 중" 래치(D-K6). 빈틈이 있을 수 있다 — `korean_rules` 는 최대 4종뿐이라
    /// 슬롯처럼 앞에서부터 채우는 불변식을 둘 필요가 없다.
    korean_latches: [Option<KoreanLatch>; MAX_KOREAN_LATCH],
    /// ⭐ F-19 "치환 중" 래치(명세 §3, D-K6 동형). F-16 래치와 **별도 배열**인 이유:
    /// 트리거 키코드가 겹치는 구성(예: F-19.6 과 F-16 이 같은 물리 키를 소스로 주장 — 충돌
    /// 대화상자가 막지만)이어도 두 래치가 서로 덮어쓰지 않도록 안전하게 독립시킨다.
    language_latches: [Option<KoreanLatch>; MAX_LANGUAGE_LATCH],
}

impl KeyStateTable {
    pub fn new() -> Self {
        KeyStateTable {
            pressed: [0; 4],
            slots: [None; MAX_TRACKED_KEYS],
            korean_latches: [None; MAX_KOREAN_LATCH],
            language_latches: [None; MAX_LANGUAGE_LATCH],
        }
    }

    /// `trigger_key` 에 걸린 "치환 중" 래치가 있으면 그 값.
    pub fn korean_latch(&self, trigger_key: KeyCode) -> Option<KoreanLatch> {
        self.korean_latches
            .iter()
            .flatten()
            .find(|l| l.trigger_key == trigger_key)
            .copied()
    }

    /// 래치를 세운다. 같은 `trigger_key` 항목이 이미 있으면 덮어쓴다(자동 반복 대비,
    /// §5#7). 빈 슬롯이 없으면(설계상 발생할 수 없다 — `MAX_KOREAN_LATCH` 는 F-16
    /// 규칙 4종에 딱 맞다) 조용히 무시한다.
    pub(crate) fn set_korean_latch(&mut self, latch: KoreanLatch) {
        if let Some(existing) = self
            .korean_latches
            .iter_mut()
            .flatten()
            .find(|l| l.trigger_key == latch.trigger_key)
        {
            *existing = latch;
            return;
        }
        if let Some(slot) = self.korean_latches.iter_mut().find(|s| s.is_none()) {
            *slot = Some(latch);
        }
    }

    /// `trigger_key` 의 래치를 지운다. keyUp 을 치환해 내보낸 뒤 호출한다.
    pub(crate) fn clear_korean_latch(&mut self, trigger_key: KeyCode) {
        for slot in self.korean_latches.iter_mut() {
            if slot.is_some_and(|l| l.trigger_key == trigger_key) {
                *slot = None;
            }
        }
    }

    /// ⭐ F-19 — `trigger_key` 에 걸린 언어 규칙 "치환 중" 래치가 있으면 그 값.
    /// `KoreanLatch` 구조체를 재사용한다(트리거 키·출력 키코드·플래그·id 로 모양이 동일).
    pub fn language_latch(&self, trigger_key: KeyCode) -> Option<KoreanLatch> {
        self.language_latches
            .iter()
            .flatten()
            .find(|l| l.trigger_key == trigger_key)
            .copied()
    }

    /// 래치를 세운다. 같은 `trigger_key` 항목이 이미 있으면 덮어쓴다(자동 반복 대비,
    /// F-16 §5#7). 빈 슬롯이 없으면(설계상 발생할 수 없다 — `MAX_LANGUAGE_LATCH` 는
    /// F-19 트리거 키 14개에 여유가 있다) 조용히 무시한다.
    pub(crate) fn set_language_latch(&mut self, latch: KoreanLatch) {
        if let Some(existing) = self
            .language_latches
            .iter_mut()
            .flatten()
            .find(|l| l.trigger_key == latch.trigger_key)
        {
            *existing = latch;
            return;
        }
        if let Some(slot) = self.language_latches.iter_mut().find(|s| s.is_none()) {
            *slot = Some(latch);
        }
    }

    /// `trigger_key` 의 언어 래치를 지운다.
    pub(crate) fn clear_language_latch(&mut self, trigger_key: KeyCode) {
        for slot in self.language_latches.iter_mut() {
            if slot.is_some_and(|l| l.trigger_key == trigger_key) {
                *slot = None;
            }
        }
    }

    fn bit_index(k: KeyCode) -> Option<(usize, u32)> {
        let idx = k.0 as usize;
        if idx >= BITSET_BITS {
            return None;
        }
        Some((idx / 64, (idx % 64) as u32))
    }

    pub fn is_pressed(&self, k: KeyCode) -> bool {
        match Self::bit_index(k) {
            Some((word, bit)) => (self.pressed[word] >> bit) & 1 == 1,
            None => false,
        }
    }

    pub(crate) fn set_pressed(&mut self, k: KeyCode, pressed: bool) {
        if let Some((word, bit)) = Self::bit_index(k) {
            if pressed {
                self.pressed[word] |= 1 << bit;
            } else {
                self.pressed[word] &= !(1u64 << bit);
            }
        }
    }

    pub fn pressed_count(&self) -> usize {
        self.pressed.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// `k` 의 quick press 상태 머신 현재 상태. 등록되지 않은 키는 `Idle` 로 취급한다.
    pub fn machine(&self, k: KeyCode) -> QuickPressState {
        self.slots
            .iter()
            .flatten()
            .find(|s| s.key == k)
            .map(|s| s.state)
            .unwrap_or(QuickPressState::Idle)
    }

    pub(crate) fn set_machine(&mut self, k: KeyCode, state: QuickPressState) {
        for slot in self.slots.iter_mut().flatten() {
            if slot.key == k {
                slot.state = state;
                return;
            }
        }
    }

    /// 등록된 슬롯 개수(빈틈없이 앞에서부터 채워져 있다는 불변식을 전제로 한다).
    pub(crate) fn slot_count(&self) -> usize {
        self.slots.iter().take_while(|s| s.is_some()).count()
    }

    /// `idx` 번째 슬롯의 (keycode, rule flags, 현재 상태). `idx < slot_count()` 여야 한다.
    pub(crate) fn slot_at(&self, idx: usize) -> (KeyCode, EventFlags, QuickPressState) {
        let slot = self.slots[idx].expect("slot_at: idx 는 slot_count() 범위 안이어야 한다");
        (slot.key, slot.rule_flags, slot.state)
    }

    /// 이 키가 hyper/meh/bleh 소스로 등록돼 있을 때 그 합성 flags. 등록돼 있지 않거나
    /// 프리셋 전용 추적 키(modifier 아님)면 `EventFlags::NONE`.
    pub fn slot_flags_for(&self, k: KeyCode) -> EventFlags {
        self.slots
            .iter()
            .flatten()
            .find(|s| s.key == k)
            .map(|s| s.rule_flags)
            .unwrap_or(EventFlags::NONE)
    }

    /// 이 키에 quick press 액션이 등록돼 있는가(M2/F-08).
    pub fn has_quick_press(&self, k: KeyCode) -> bool {
        self.slots
            .iter()
            .flatten()
            .find(|s| s.key == k)
            .map(|s| s.has_quick_press)
            .unwrap_or(false)
    }

    /// 이 키에 double tap 액션이 등록돼 있는가(M2/F-08).
    pub fn has_double_tap(&self, k: KeyCode) -> bool {
        self.slots
            .iter()
            .flatten()
            .find(|s| s.key == k)
            .map(|s| s.has_double_tap)
            .unwrap_or(false)
    }

    /// `kind` 종류의 modifier 슬롯 중 현재 `HoldConfirmed` 인 것이 하나라도 있는가
    /// — 논리 hyper 신호(R4/P8, `docs/dev/architecture.md` §6.4). 소스 키가 caps lock 이든
    /// globe 든 관계없이 동일하게 참을 반환한다.
    pub fn is_kind_active(&self, kind: ModifierKind) -> bool {
        self.slots
            .iter()
            .flatten()
            .any(|s| s.kind == Some(kind) && matches!(s.state, QuickPressState::HoldConfirmed))
    }

    /// `RuleTable::modifier_rules` 와 `RuleTable::source_actions` 의 소스 키 **합집합**으로
    /// 추적 슬롯을 (재)구성한다(`docs/dev/architecture.md` §6.2). 같은 키가 둘 다에 등록돼
    /// 있으면 한 슬롯으로 합쳐 modifier 정보와 액션 정보를 함께 담는다. 이미 등록돼 있던
    /// 소스 키는 현재 상태를 보존한다(설정을 바꾸는 도중 활성 상태를 잃지 않기 위함).
    ///
    /// ⭐ M1 의 `register_modifier_sources` 를 대체한다 — 이름을 바꾼 것은 이제
    /// modifier 뿐 아니라 프리셋 액션 소스 키도 함께 등록하기 때문이다.
    ///
    /// ⭐ **결합 지점**(이슈 #140). 여기서 `rule_flags` 로 등록하는 두 원천
    /// (`modifier_rules`, `source_actions.hold_remap` 의 modifier 대상)이 곧
    /// `active_synth_flags()` 가 낼 수 있는 flags 의 전부다.
    /// `tap_mask::synthetic_modifier_flags_possible` 이 이 두 원천을 그대로
    /// 다시 판정해 탭 마스크에 마우스 이벤트를 넣을지 정한다 — 여기 원천이
    /// 늘면 그쪽도 함께 고쳐야 한다.
    pub(crate) fn register_sources(&mut self, rules: &RuleTable) {
        let mut new_slots: [Option<MachineSlot>; MAX_TRACKED_KEYS] = [None; MAX_TRACKED_KEYS];
        let mut n = 0usize;

        for rule in &rules.modifier_rules {
            if let Some(existing) = new_slots[..n].iter_mut().flatten().find(|s| s.key == rule.source) {
                existing.kind = Some(rule.kind);
                existing.rule_flags = rule.flags;
            } else if n < MAX_TRACKED_KEYS {
                let preserved_state = self.machine(rule.source);
                new_slots[n] = Some(MachineSlot {
                    key: rule.source,
                    rule_flags: rule.flags,
                    kind: Some(rule.kind),
                    has_quick_press: false,
                    has_double_tap: false,
                    state: preserved_state,
                });
                n += 1;
            }
            // 설계 한도 초과(n >= MAX_TRACKED_KEYS) — 초과분은 등록하지 않는다(패닉하지 않는다).
        }

        for sa in &rules.source_actions {
            // ⭐ hold_remap 대상이 **modifier 키**면 그 키의 flags 를 이 슬롯의
            // `rule_flags` 로 삼는다 (2026-08-30, 이슈 #19 증상 A).
            //
            // `Remap caps lock to: left control` 은 "caps lock 을 누르고 있는 동안
            // left control 이 눌려 있다"는 뜻이다. 합성 `flagsChanged` 를 한 번 내는
            // 것만으로는 부족하다 — 그 뒤에 지나가는 **다른 키 이벤트에도** control
            // 비트가 실려 있어야 앱이 `⌃C` 를 본다. hyper/meh/bleh 가 이미 그 일을
            // `active_synth_flags()` → `Disposition::PassWithFlags` 로 하고 있으므로,
            // hold_remap 도 같은 기구에 태운다. 이것이 없으면 조합키로서 쓸모가 없다.
            let hold_remap_flags = match sa.hold_remap {
                Some(RuleAction::Key { keycode, .. }) => {
                    keycode.modifier_flags().unwrap_or(EventFlags::NONE)
                }
                _ => EventFlags::NONE,
            };

            if let Some(existing) = new_slots[..n].iter_mut().flatten().find(|s| s.key == sa.key) {
                existing.has_quick_press = sa.quick_press.is_some();
                existing.has_double_tap = sa.double_tap.is_some();
                // 같은 키가 hyper/meh/bleh 소스이기도 하면 그쪽 flags 가 우선한다 —
                // 그 조합은 UI 충돌 감지가 막지만, 여기서도 조용히 덮어쓰지 않는다.
                if existing.kind.is_none() {
                    existing.rule_flags = hold_remap_flags;
                }
            } else if n < MAX_TRACKED_KEYS {
                let preserved_state = self.machine(sa.key);
                new_slots[n] = Some(MachineSlot {
                    key: sa.key,
                    rule_flags: hold_remap_flags,
                    kind: None,
                    has_quick_press: sa.quick_press.is_some(),
                    has_double_tap: sa.double_tap.is_some(),
                    state: preserved_state,
                });
                n += 1;
            }
        }

        // ⭐ F-19 — 언어 규칙의 `AloneTap` 트리거 키(caps lock·좌/우⌘)도 같은 FSM 을 탄다.
        // F-16 의 "modifier 부재" 판정은 keyDown 도착 시점에 트리거 키 자신까지 함께
        // 눌렸는지로 판정했지만, F-19 의 단독 탭("이 키만, 다른 modifier 없음")은
        // **탭/홀드 경계**가 필요하다 — ⌘+W 를 누르면 홀드로 확정돼 우⌘가 modifier 로
        // 남아야 하고(명세 §3.6, H5), 단독 탭일 때만 발화해야 한다. 그래서 F-08.2 와
        // 같은 QuickPress FSM 슬롯으로 등록하고, 발화는 `handle_tracked_key_event` 의
        // QuickPress 이벤트가 담당한다. `rule_flags` 는 비우고(합성 flags 를 얹지 않는다,
        // P5, F-19.2 의 우⌘는 물리 modifier 가 이미 native flags 를 낸다) `has_quick_press`
        // 만 등록한다.
        for rule in &rules.language_rules {
            if let LanguageTrigger::AloneTap { key } = rule.trigger {
                if let Some(existing) = new_slots[..n].iter_mut().flatten().find(|s| s.key == key) {
                    existing.has_quick_press = true;
                } else if n < MAX_TRACKED_KEYS {
                    let preserved_state = self.machine(key);
                    new_slots[n] = Some(MachineSlot {
                        key,
                        rule_flags: EventFlags::NONE,
                        kind: None,
                        has_quick_press: true,
                        has_double_tap: false,
                        state: preserved_state,
                    });
                    n += 1;
                }
            }
        }

        self.slots = new_slots;
    }

    /// 절전/잠금/Secure Input 진입 등 stuck modifier 방지를 위한 전면 리셋(§5 엣지 6·9).
    /// 실제 "off" flagsChanged 방출은 `Arbiter::force_reset` 이 이 호출 **이전에** 담당한다 —
    /// 이 함수는 순수하게 상태만 지운다.
    pub fn reset_all(&mut self) {
        self.pressed = [0; 4];
        for slot in self.slots.iter_mut().flatten() {
            slot.state = QuickPressState::Idle;
        }
        // D-K6 — 래치도 전부 지운다. 리셋 이후 도착하는 keyUp 은 더 이상 대응하는
        // keyDown 치환이 없으므로 원본 그대로 흘려보내야 한다.
        self.korean_latches = [None; MAX_KOREAN_LATCH];
        // ⭐ F-19 — 언어 규칙 래치도 같은 규약으로 전부 지운다.
        self.language_latches = [None; MAX_LANGUAGE_LATCH];
    }

    /// 현재 `HoldConfirmed` 인 모든 modifier 규칙의 flags 를 OR 로 합산한 값.
    ///
    /// ⭐ 여러 조합이 동시에 활성이면 OR 로 합산한다 — `hyperkey.md` §9 #2 가 미확정으로
    /// 남긴 질문에 대해 이 크레이트가 채택한 답이다. 근거: (1) 비트마스크 표현과 정합적이고,
    /// (2) "덮어쓰기"를 택하면 어느 쪽이 이기는지가 입력 순서에 의존해 비결정적이 된다.
    pub fn active_synth_flags(&self) -> EventFlags {
        let mut flags = EventFlags::NONE;
        for slot in self.slots.iter().flatten() {
            if matches!(slot.state, QuickPressState::HoldConfirmed) {
                flags |= slot.rule_flags;
            }
        }
        flags
    }

    /// ⭐ 이슈 #125 — `active_synth_flags()` 와 달리 **물리적으로 지금 눌려 있는**
    /// 소스 키의 `HoldConfirmed` 슬롯만 합산한다. Seek 세션 게이트가 `is_tracked`
    /// (FSM 진입)를 가려, 세션 중 소스 키를 실제로 뗐는데도 FSM 슬롯이
    /// `HoldConfirmed` 로 잔류하는 stale 상태가 생길 수 있다(`arbitration.rs` D2
    /// 가드 문서 참고) — 정본 눌림 비트셋(`pressed`)은 세션과 무관하게 항상
    /// 정확하므로(`arbitrate_with_kind` 최상단이 세션 게이트보다 앞서 갱신), 이
    /// 메서드로 그 stale 슬롯을 걸러낸다.
    ///
    /// ⚠️ `active_synth_flags()` 자체는 고치지 않는다 — 이 테이블은 "눌림
    /// 비트셋"과 "FSM 슬롯 상태"를 독립된 필드로 둔다(모듈 문서). 세션 밖에서는
    /// `HoldConfirmed` 전이·해제가 항상 FSM 을 통해서만 일어나 `HoldConfirmed`
    /// ⇒ `is_pressed(key)==true` 가 이미 성립하므로, 이 메서드는 그 경로에서
    /// `active_synth_flags()` 와 동일한 값을 낸다 — 실제로 갈라지는 것은 stale
    /// 케이스뿐이다.
    pub fn active_synth_flags_of_pressed_slots(&self) -> EventFlags {
        let mut flags = EventFlags::NONE;
        for slot in self.slots.iter().flatten() {
            if matches!(slot.state, QuickPressState::HoldConfirmed) && self.is_pressed(slot.key) {
                flags |= slot.rule_flags;
            }
        }
        flags
    }
}

impl Default for KeyStateTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{ModifierKind, ModifierRule, RuleAction, SourceKeyActions};

    #[test]
    fn pressed_bitset_tracks_independently_per_key() {
        let mut t = KeyStateTable::new();
        assert!(!t.is_pressed(KeyCode::CAPS_LOCK));
        t.set_pressed(KeyCode::CAPS_LOCK, true);
        assert!(t.is_pressed(KeyCode::CAPS_LOCK));
        assert!(!t.is_pressed(KeyCode::LEFT_SHIFT));
        assert_eq!(t.pressed_count(), 1);
        t.set_pressed(KeyCode::CAPS_LOCK, false);
        assert!(!t.is_pressed(KeyCode::CAPS_LOCK));
        assert_eq!(t.pressed_count(), 0);
    }

    #[test]
    fn unregistered_key_machine_is_idle() {
        let t = KeyStateTable::new();
        assert_eq!(t.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle);
    }

    #[test]
    fn register_preserves_state_across_reconfigure() {
        let mut t = KeyStateTable::new();
        let mut rules = RuleTable::default();
        rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        t.register_sources(&rules);
        t.set_machine(KeyCode::CAPS_LOCK, QuickPressState::HoldConfirmed);

        // 같은 규칙으로 다시 등록해도 활성 상태가 유지되어야 한다.
        t.register_sources(&rules);
        assert_eq!(t.machine(KeyCode::CAPS_LOCK), QuickPressState::HoldConfirmed);
    }

    #[test]
    fn active_synth_flags_ors_multiple_active_rules() {
        let mut t = KeyStateTable::new();
        let mut rules = RuleTable::default();
        rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        rules.modifier_rules.push(ModifierRule {
            source: KeyCode::RIGHT_OPTION,
            kind: ModifierKind::Meh,
            flags: EventFlags::MEH,
        });
        t.register_sources(&rules);
        t.set_machine(KeyCode::CAPS_LOCK, QuickPressState::HoldConfirmed);
        t.set_machine(KeyCode::RIGHT_OPTION, QuickPressState::HoldConfirmed);

        let active = t.active_synth_flags();
        assert!(active.contains(EventFlags::HYPER_WITH_SHIFT));
        assert!(active.contains(EventFlags::MEH));
    }

    #[test]
    fn reset_all_clears_pressed_and_machines() {
        let mut t = KeyStateTable::new();
        let mut rules = RuleTable::default();
        rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        t.register_sources(&rules);
        t.set_pressed(KeyCode::CAPS_LOCK, true);
        t.set_machine(KeyCode::CAPS_LOCK, QuickPressState::HoldConfirmed);

        t.reset_all();

        assert!(!t.is_pressed(KeyCode::CAPS_LOCK));
        assert_eq!(t.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle);
        assert!(t.active_synth_flags().is_empty());
    }

    // ── M2/F-08 — register_sources 합집합 등록, is_kind_active, 접근자 ──────────────────

    /// 같은 키가 `modifier_rules` 와 `source_actions` 양쪽에 등록돼 있으면 한 슬롯으로
    /// 합쳐진다 — 예: caps lock 이 hyper 소스이면서 동시에 quick press 대상(F-08.2)인 구성.
    #[test]
    fn register_sources_merges_same_key_from_modifier_and_source_actions() {
        let mut t = KeyStateTable::new();
        let mut rules = RuleTable::default();
        rules.modifier_rules.push(ModifierRule {
            source: KeyCode::CAPS_LOCK,
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: Some(RuleAction::ToggleCapsLock),
            double_tap: None,
            hold_remap: None,
        });
        t.register_sources(&rules);

        // 슬롯이 하나로 합쳐졌다 — modifier 정보와 액션 정보를 동시에 담는다.
        assert_eq!(t.slot_count(), 1);
        assert_eq!(t.slot_flags_for(KeyCode::CAPS_LOCK), EventFlags::HYPER_WITH_SHIFT);
        assert!(t.has_quick_press(KeyCode::CAPS_LOCK));
        assert!(!t.has_double_tap(KeyCode::CAPS_LOCK));
    }

    /// hold_remap 대상이 **modifier 가 아닌** 키(`esc` 등)면 `slot_flags_for` 는 `NONE`.
    #[test]
    fn register_sources_non_modifier_hold_remap_has_no_flags() {
        let mut t = KeyStateTable::new();
        let mut rules = RuleTable::default();
        rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: None,
            double_tap: None,
            hold_remap: Some(RuleAction::Key {
                keycode: KeyCode::ESCAPE,
                flags: EventFlags::NONE,
            }),
        });
        t.register_sources(&rules);

        assert_eq!(t.slot_flags_for(KeyCode::CAPS_LOCK), EventFlags::NONE);
        assert!(!t.has_quick_press(KeyCode::CAPS_LOCK));
        assert!(!t.has_double_tap(KeyCode::CAPS_LOCK));
    }

    /// ⭐ hold_remap 대상이 **modifier 키**면 그 flags 를 슬롯이 들고 있어야 한다
    /// (2026-08-30, 이슈 #19 증상 A). 그래야 `active_synth_flags()` → `PassWithFlags`
    /// 로 **뒤따르는 다른 키 이벤트에도** control 비트가 실린다 — 그것이 없으면
    /// `Remap caps lock to: left control` 로 `⌃C` 를 만들 수 없다.
    ///
    /// 기대값의 출처는 구현 상수가 아니라 macOS 헤더다:
    ///   kCGEventFlagMaskControl = 0x00040000 (`CGEventTypes.h`)
    ///   NX_DEVICELCTLKEYMASK    = 0x00000001 (`IOKit/hidsystem/IOLLEvent.h`)
    #[test]
    fn register_sources_modifier_hold_remap_carries_target_flags() {
        let mut t = KeyStateTable::new();
        let mut rules = RuleTable::default();
        rules.source_actions.push(SourceKeyActions {
            key: KeyCode::CAPS_LOCK,
            quick_press: None,
            double_tap: None,
            hold_remap: Some(RuleAction::Key {
                keycode: KeyCode::LEFT_CONTROL,
                flags: EventFlags::NONE,
            }),
        });
        t.register_sources(&rules);

        assert_eq!(t.slot_flags_for(KeyCode::CAPS_LOCK).0, 0x0004_0001);

        // 그리고 hold 가 확정되면 그 flags 가 실제로 active 로 합산돼야 한다.
        t.set_machine(KeyCode::CAPS_LOCK, QuickPressState::HoldConfirmed);
        assert_eq!(t.active_synth_flags().0, 0x0004_0001);
    }

    /// `is_kind_active` — 소스 키 종류(caps lock/globe)와 무관하게 `HoldConfirmed` 인
    /// hyper 슬롯이 있으면 참(R4/P8).
    #[test]
    fn is_kind_active_is_true_regardless_of_source_key() {
        let mut t = KeyStateTable::new();
        let mut rules = RuleTable::default();
        rules.modifier_rules.push(ModifierRule {
            source: KeyCode::FUNCTION, // globe
            kind: ModifierKind::Hyper,
            flags: EventFlags::HYPER_WITH_SHIFT,
        });
        t.register_sources(&rules);
        assert!(!t.is_kind_active(ModifierKind::Hyper));

        t.set_machine(KeyCode::FUNCTION, QuickPressState::HoldConfirmed);
        assert!(t.is_kind_active(ModifierKind::Hyper));
        assert!(!t.is_kind_active(ModifierKind::Meh));
    }

    /// 등록되지 않은 키의 접근자들은 안전한 기본값을 낸다(패닉하지 않는다).
    #[test]
    fn accessors_on_unregistered_key_return_safe_defaults() {
        let t = KeyStateTable::new();
        assert_eq!(t.slot_flags_for(KeyCode::CAPS_LOCK), EventFlags::NONE);
        assert!(!t.has_quick_press(KeyCode::CAPS_LOCK));
        assert!(!t.has_double_tap(KeyCode::CAPS_LOCK));
        assert!(!t.is_kind_active(ModifierKind::Hyper));
    }

    // ── F-16 korean_latch — D-K6 ────────────────────────────────────────────────

    #[test]
    fn korean_latch_roundtrip_and_clear() {
        let mut t = KeyStateTable::new();
        assert_eq!(t.korean_latch(KeyCode::SPACE), None);

        let latch = KoreanLatch {
            trigger_key: KeyCode::SPACE,
            out_keycode: KeyCode::SPACE,
            out_flags: EventFlags::CONTROL,
            id: RuleId::Korean(13),
        };
        t.set_korean_latch(latch);
        assert_eq!(t.korean_latch(KeyCode::SPACE), Some(latch));

        t.clear_korean_latch(KeyCode::SPACE);
        assert_eq!(t.korean_latch(KeyCode::SPACE), None);
    }

    /// 같은 trigger_key 로 다시 세우면 덮어쓴다 — 자동 반복 대비(§5#7).
    #[test]
    fn korean_latch_overwrites_same_trigger_key() {
        let mut t = KeyStateTable::new();
        t.set_korean_latch(KoreanLatch {
            trigger_key: KeyCode::SPACE,
            out_keycode: KeyCode::SPACE,
            out_flags: EventFlags::CONTROL,
            id: RuleId::Korean(13),
        });
        t.set_korean_latch(KoreanLatch {
            trigger_key: KeyCode::SPACE,
            out_keycode: KeyCode::SPACE,
            out_flags: EventFlags::ALTERNATE,
            id: RuleId::Korean(13),
        });
        assert_eq!(
            t.korean_latch(KeyCode::SPACE).map(|l| l.out_flags),
            Some(EventFlags::ALTERNATE)
        );
    }

    /// 여러 trigger_key 의 래치가 독립적으로 공존한다(space/grave/lang1/lang2).
    #[test]
    fn korean_latch_tracks_multiple_trigger_keys_independently() {
        let mut t = KeyStateTable::new();
        t.set_korean_latch(KoreanLatch {
            trigger_key: KeyCode::SPACE,
            out_keycode: KeyCode::SPACE,
            out_flags: EventFlags::CONTROL,
            id: RuleId::Korean(13),
        });
        t.set_korean_latch(KoreanLatch {
            trigger_key: KeyCode(0x32), // grave
            out_keycode: KeyCode(0x32),
            out_flags: EventFlags::ALTERNATE,
            id: RuleId::Korean(13),
        });
        assert!(t.korean_latch(KeyCode::SPACE).is_some());
        assert!(t.korean_latch(KeyCode(0x32)).is_some());

        t.clear_korean_latch(KeyCode::SPACE);
        assert!(t.korean_latch(KeyCode::SPACE).is_none());
        assert!(t.korean_latch(KeyCode(0x32)).is_some());
    }

    /// `reset_all()` 이 래치를 전부 지운다(D-K6).
    #[test]
    fn reset_all_clears_korean_latches() {
        let mut t = KeyStateTable::new();
        t.set_korean_latch(KoreanLatch {
            trigger_key: KeyCode::SPACE,
            out_keycode: KeyCode::SPACE,
            out_flags: EventFlags::CONTROL,
            id: RuleId::Korean(13),
        });
        t.reset_all();
        assert_eq!(t.korean_latch(KeyCode::SPACE), None);
    }
}
