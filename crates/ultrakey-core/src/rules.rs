//! 중재가 소비하는 규칙 테이블. F-05(hyper/meh/bleh)·F-08(Presets)이 여기 등록만 하고,
//! 실제 판정은 `arbitration.rs` 가 한다(`key-remapping-engine.md` §3-b).

use crate::flags::EventFlags;
use crate::keycode::KeyCode;

/// hyper/meh/bleh 세 조합 중 무엇인지(`hyperkey.md` §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModifierKind {
    Hyper,
    Meh,
    Bleh,
}

/// 소스 키 하나를 hyper/meh/bleh 로 바꾸는 규칙 하나(§3-b 계층 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModifierRule {
    pub source: KeyCode,
    pub kind: ModifierKind,
    pub flags: EventFlags,
}

/// Preset 다중 키 조합 규칙(§3-b 계층 3) — M2/F-08 자리.
///
/// M1 에는 이 규칙 타입을 실제로 채우는 곳이 없다 — `RuleTable::combo_rules` 는 항상
/// 빈 `Vec` 이다(`docs/dev/architecture.md` §5). 필드는 F-08 위임 시 채운다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComboRule {}

/// 소스 키 하나를 다른 키 하나로 바꾸는 1:1 리매핑(§3-b 계층 4) — M2/F-08 자리.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimpleRemap {
    pub from: KeyCode,
    pub to: KeyCode,
}

/// 이 엔진 인스턴스가 아는 규칙 전부. `Arbiter::arbitrate`/`reconfigure` 가 매 호출마다
/// 참조한다(할당 없이 순회할 수 있도록 `Vec` 이지만 항목 수는 작다고 가정한다).
#[derive(Debug, Clone, Default)]
pub struct RuleTable {
    /// F-05(M1). hyper/meh/bleh 규칙.
    pub modifier_rules: Vec<ModifierRule>,
    /// F-08(M2) 자리. M1 에서는 항상 비어 있다.
    pub combo_rules: Vec<ComboRule>,
    /// F-08(M2) 자리. M1 에서는 항상 비어 있다.
    pub simple_remaps: Vec<SimpleRemap>,
}

impl RuleTable {
    /// 주어진 물리 keycode 를 소스로 하는 modifier 규칙을 찾는다.
    ///
    /// ⭐ 동일 소스 키가 둘 이상의 `ModifierRule` 에 등록된 경우(§3-b "동일 소스 키 중복
    /// 배정 방지" 결정), 이 함수는 **처음 등록된 것**을 결정론적으로 반환한다 — 조용히
    /// 둘 다 무시되는 상태는 없다. UI 경고는 이 크레이트 바깥(F-05/F-08 소관)의 일이다.
    pub fn modifier_rule_for(&self, k: KeyCode) -> Option<&ModifierRule> {
        self.modifier_rules.iter().find(|r| r.source == k)
    }
}
