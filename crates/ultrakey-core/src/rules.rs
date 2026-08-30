//! 중재가 소비하는 규칙 테이블. F-05(hyper/meh/bleh)·F-08(Presets)이 여기 등록만 하고,
//! 실제 판정은 `arbitration.rs` 가 한다(`key-remapping-engine.md` §3-b).

use crate::flags::EventFlags;
use crate::keycode::KeyCode;
use crate::korean::KoreanTrigger;

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

/// 어느 프리셋(또는 기능)이 만든 규칙인지 — 로그·테스트·충돌 진단용
/// (`docs/dev/architecture.md` §6 A-3). `Preset(1)` = F-08.1 … `Preset(16)` = F-08.16.
/// `Korean(13)`..`Korean(16)` = F-16.1..F-16.4(`docs/spec/korean-input.md` §3.4, D-K5).
///
/// ⭐ 순서는 선언 순서 그대로 `Preset(_) < Korean(_) < Hyperkey` 다(derive `Ord` —
/// enum 의 판별값(discriminant) 다음 payload 로 비교). 이것이 명세 §3.4 "계층 3 안의
/// 규칙 ID 순서로 결정론적" 의 코드 표현이다 — F-08.4(`Preset(4)`)가 F-16.1
/// (`Korean(13)`)보다 항상 먼저 평가되어 이긴다. `combo_rules` 와 `korean_rules` 는
/// F-08/F-16 이 각자 채우고 `Hyperkey` 는 계층 2 modifier 규칙만 만들기 때문에
/// `Korean(_)` 대 `Hyperkey` 의 상대 순서는 실제로는 쓰이지 않지만, `RuleId` 자체의
/// 전순서(total order)를 명확히 하기 위해 derive 로 정의해 둔다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleId {
    Preset(u8),
    Korean(u8),
    Hyperkey,
}

/// 조합의 "유지" 조건(`docs/dev/architecture.md` §6.4 P3 — 정본 눌림 테이블로만 판정한다.
/// `ev.flags` 의 modifier 비트로 판정하지 않는다 — 좌/우 shift 가 같은 비트를 공유하고,
/// caps lock 의 `alphaShift` 는 눌림이 아니라 잠금을 뜻하기 때문이다).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoldCondition {
    Key(KeyCode),
    EitherShift,
    EitherCommand,
    /// 논리 hyper 신호 — `ModifierKind::Hyper` 슬롯이 `HoldConfirmed` 인가(R4,
    /// architecture.md §6.4 P8). 소스 키 종류(caps lock·globe 등)와 무관하다.
    HyperActive,
}

/// 규칙 발화 시의 결과.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleAction {
    /// 키 하나의 down+up 을 합성한다.
    Key { keycode: KeyCode, flags: EventFlags },
    /// 유니코드 문자 하나를 그대로 입력한다(레이아웃 독립, architecture.md §6.4 P9).
    Text(char),
    /// 경로 C 로 실제 caps lock 잠금을 토글한다(P11).
    ToggleCapsLock,
    /// Seek 세션을 연다 — M3/F-01. 지금은 효과만 발행한다.
    OpenSeek,
    /// 아무것도 내지 않는다(`nothing (disable it)`).
    Nothing,
}

/// Preset 다중 키 조합 규칙(§3-b 계층 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComboRule {
    pub id: RuleId,
    pub hold: HoldCondition,
    pub trigger: KeyCode,
    pub action: RuleAction,
}

/// F-16 한국어 입력 규칙 하나(`docs/spec/korean-input.md` §3.1, D-K5).
///
/// ⛔ `ComboRule` 을 재사용하지 않는다 — `HoldCondition` 은 "modifier 부재"·"한국어
/// IME 활성"·"앱 제외" 를 표현할 수 없다. 넓히면 범용 기구가 F-16 전용 개념으로
/// 오염된다(D-K5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KoreanRule {
    /// `RuleId::Korean(13)`..`Korean(16)`.
    pub id: RuleId,
    /// 물리 트리거 키(`space`(0x31) / `lang1`(`KeyCode::JIS_KANA`, 0x68) /
    /// `lang2`(`KeyCode::JIS_EISU`, 0x66) / `grave`(0x32)).
    pub trigger_key: KeyCode,
    pub trigger: KoreanTrigger,
    /// 참이면 [`crate::korean::KoreanImeState::Active`] 일 때만 발화한다(fail-closed).
    pub requires_korean_ime: bool,
    /// 같은 keycode 재주입 — 명세 결정: 유니코드 주입 금지(§3.1).
    pub out_keycode: KeyCode,
    /// 얹을 modifier 비트. ⭐ 원본 `ev.flags` 를 물려받지 않는다 — 이 값 그대로 방출한다
    /// (D-K7, `docs/dev/architecture.md` §6.4 P5).
    pub out_flags: EventFlags,
}

/// 소스 키 하나를 다른 키 하나로 바꾸는 1:1 리매핑(§3-b 계층 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimpleRemap {
    pub id: RuleId,
    pub from: KeyCode,
    pub to: KeyCode,
    pub add_flags: EventFlags,
}

/// 추적 키 자신의 이벤트로 발화하는 프리셋 액션(계층 2·3, `docs/dev/architecture.md` §6.4 P1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceKeyActions {
    pub key: KeyCode,
    pub quick_press: Option<RuleAction>,
    pub double_tap: Option<RuleAction>,
    /// F-08.1 — hold 로 확정된 동안 이 키의 down 을 유지, 뗄 때 up.
    /// `None` 이면 리매핑 없음. `Some(RuleAction::Nothing)` 은 소비만 하고 아무것도 내지 않음.
    pub hold_remap: Option<RuleAction>,
}

/// 이 엔진 인스턴스가 아는 규칙 전부. `Arbiter::arbitrate`/`reconfigure` 가 매 호출마다
/// 참조한다(할당 없이 순회할 수 있도록 `Vec` 이지만 항목 수는 작다고 가정한다).
#[derive(Debug, Clone, Default)]
pub struct RuleTable {
    /// F-05(M1). hyper/meh/bleh 규칙.
    pub modifier_rules: Vec<ModifierRule>,
    /// F-08(M2). `RuleId` 오름차순으로 정렬된 상태로 주어진다고 전제한다
    /// (`PresetRules::to_rules` 가 정렬해서 반환한다 — architecture.md §6.4 서두).
    pub combo_rules: Vec<ComboRule>,
    /// F-08(M2).
    pub simple_remaps: Vec<SimpleRemap>,
    /// F-08(M2). 추적 키(caps lock·좌우 shift 등) 자신의 이벤트로 발화하는 액션.
    pub source_actions: Vec<SourceKeyActions>,
    /// F-16(M?). `RuleId` 오름차순으로 정렬된 상태로 주어진다고 전제한다
    /// (`ultrakey-korean::KoreanSettings::to_rules` 가 정렬해서 반환한다).
    pub korean_rules: Vec<KoreanRule>,
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

    /// 주어진 물리 keycode 를 추적 대상으로 삼는 소스 키 액션을 찾는다.
    pub fn source_actions_for(&self, k: KeyCode) -> Option<&SourceKeyActions> {
        self.source_actions.iter().find(|s| s.key == k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rule_id_preset_variants_sort_before_hyperkey() {
        assert!(RuleId::Preset(1) < RuleId::Hyperkey);
        assert!(RuleId::Preset(1) < RuleId::Preset(2));
        assert!(RuleId::Preset(16) < RuleId::Hyperkey);
    }

    /// D-K5 — `Preset(_) < Korean(_) < Hyperkey`. 계층 3 안의 평가 순서(§3.4)의
    /// 코드 표현이다: F-08.4(`Preset(4)`)가 F-16.1(`Korean(13)`)보다 먼저 평가된다.
    #[test]
    fn rule_id_korean_variants_sort_between_preset_and_hyperkey() {
        assert!(RuleId::Preset(16) < RuleId::Korean(13));
        assert!(RuleId::Korean(13) < RuleId::Korean(16));
        assert!(RuleId::Korean(16) < RuleId::Hyperkey);
        assert!(RuleId::Preset(4) < RuleId::Korean(13));
    }

    #[test]
    fn source_actions_for_finds_registered_key() {
        let table = RuleTable {
            source_actions: vec![SourceKeyActions {
                key: KeyCode::CAPS_LOCK,
                quick_press: Some(RuleAction::ToggleCapsLock),
                double_tap: None,
                hold_remap: None,
            }],
            ..RuleTable::default()
        };
        assert!(table.source_actions_for(KeyCode::CAPS_LOCK).is_some());
        assert!(table.source_actions_for(KeyCode::LEFT_SHIFT).is_none());
    }
}
