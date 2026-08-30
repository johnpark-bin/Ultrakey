//! ⭐ 정본(canonical) 공유 상태 — `key-remapping-engine.md` §3-b 가 요구하는 "물리 키 하나의
//! 현재 상태는 시스템 전체에서 단 하나로만 존재한다"는 원칙을 코드로 표현한 자리다.
//! Seek/Hyperkey/Presets 는 이 테이블을 **읽기만** 한다.
//!
//! 콜백 임계 경로에서 호출되므로 힙 할당이 없다: 눌림 여부는 고정 크기 비트셋
//! (`[u64; 4]` = 256비트)으로, quick press 상태 머신은 작은 고정 배열로 저장한다.

use crate::flags::EventFlags;
use crate::keycode::KeyCode;
use crate::quickpress::QuickPressState;
use crate::rules::{ModifierKind, RuleAction, RuleTable};

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
}
