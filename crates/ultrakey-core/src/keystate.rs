//! ⭐ 정본(canonical) 공유 상태 — `key-remapping-engine.md` §3-b 가 요구하는 "물리 키 하나의
//! 현재 상태는 시스템 전체에서 단 하나로만 존재한다"는 원칙을 코드로 표현한 자리다.
//! Seek/Hyperkey/Presets 는 이 테이블을 **읽기만** 한다.
//!
//! 콜백 임계 경로에서 호출되므로 힙 할당이 없다: 눌림 여부는 고정 크기 비트셋
//! (`[u64; 4]` = 256비트)으로, quick press 상태 머신은 작은 고정 배열로 저장한다.

use crate::flags::EventFlags;
use crate::keycode::KeyCode;
use crate::quickpress::QuickPressState;
use crate::rules::RuleTable;

/// 동시에 추적 가능한 quick press 소스 키의 최대 개수. M1 은 hyper/meh/bleh 최대 3개뿐이라
/// 8개면 여유롭다 — 필요해지면 이 상수만 올리면 된다.
const MAX_TRACKED_KEYS: usize = 8;

/// 비트셋이 표현 가능한 keycode 범위. macOS virtual keycode 는 실측 범위(§4 키코드 목록)가
/// 전부 이 안에 든다 — 그 이상 값은 (있을 수 없다고 보고) 조용히 무시한다.
const BITSET_BITS: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct MachineSlot {
    key: KeyCode,
    /// 이 소스 키가 hold 로 확정됐을 때 얹을 modifier flags. 등록 시점(`RuleTable`)에서
    /// 캐시해 둔다 — 콜백 임계 경로에서 `RuleTable` 을 다시 찾아보지 않기 위함.
    rule_flags: EventFlags,
    state: QuickPressState,
}

/// 정본 키 상태 테이블. `Arbiter` 가 배타 소유한다(`docs/dev/architecture.md` §2.2 —
/// 탭 스레드 전용, 락 없이 `&mut` 로만 접근).
pub struct KeyStateTable {
    /// 눌림 여부 비트셋. 인덱스 = keycode 값.
    pressed: [u64; 4],
    /// quick press 상태 머신 슬롯. 항상 앞에서부터 빈틈없이(index 0..slot_count) 채운다 —
    /// `register_modifier_sources` 가 매번 전체를 다시 구성하기 때문에 이 불변식이 성립한다.
    slots: [Option<MachineSlot>; MAX_TRACKED_KEYS],
}

impl KeyStateTable {
    pub fn new() -> Self {
        KeyStateTable {
            pressed: [0; 4],
            slots: [None; MAX_TRACKED_KEYS],
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

    /// `RuleTable::modifier_rules` 로부터 quick press 슬롯을 (재)구성한다. 이미 등록돼
    /// 있던 소스 키는 현재 상태를 보존한다(설정을 바꾸는 도중 활성 상태를 잃지 않기 위함).
    pub(crate) fn register_modifier_sources(&mut self, rules: &RuleTable) {
        let mut new_slots: [Option<MachineSlot>; MAX_TRACKED_KEYS] = [None; MAX_TRACKED_KEYS];
        for (n, rule) in rules.modifier_rules.iter().enumerate() {
            if n >= MAX_TRACKED_KEYS {
                // 설계 한도 초과 — M1 최대치(hyper/meh/bleh 3개)를 훨씬 넘는 비정상 설정.
                // 초과분은 등록하지 않는다(패닉하지 않는다).
                break;
            }
            let preserved_state = self.machine(rule.source);
            new_slots[n] = Some(MachineSlot {
                key: rule.source,
                rule_flags: rule.flags,
                state: preserved_state,
            });
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
}

impl Default for KeyStateTable {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{ModifierKind, ModifierRule};

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
        t.register_modifier_sources(&rules);
        t.set_machine(KeyCode::CAPS_LOCK, QuickPressState::HoldConfirmed);

        // 같은 규칙으로 다시 등록해도 활성 상태가 유지되어야 한다.
        t.register_modifier_sources(&rules);
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
        t.register_modifier_sources(&rules);
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
        t.register_modifier_sources(&rules);
        t.set_pressed(KeyCode::CAPS_LOCK, true);
        t.set_machine(KeyCode::CAPS_LOCK, QuickPressState::HoldConfirmed);

        t.reset_all();

        assert!(!t.is_pressed(KeyCode::CAPS_LOCK));
        assert_eq!(t.machine(KeyCode::CAPS_LOCK), QuickPressState::Idle);
        assert!(t.active_synth_flags().is_empty());
    }
}
