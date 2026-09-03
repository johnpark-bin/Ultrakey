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
/// `Language(17)`..`Language(23)` = F-19.1..F-19.7(`docs/spec/language-presets.md` §3 —
/// 규칙표 P17~P23, `docs/dev/architecture.md` §6.4). payload 는 규칙표 번호를 그대로 쓴다.
///
/// ⭐ 순서는 선언 순서 그대로 `Preset(_) < Korean(_) < Language(_) < Hyperkey` 다(derive
/// `Ord` — enum 의 판별값(discriminant) 다음 payload 로 비교). 이것이 명세 §3.4 "계층 3 안의
/// 규칙 ID 순서로 결정론적" 의 코드 표현이다 — F-08.4(`Preset(4)`)가 F-16.1
/// (`Korean(13)`)보다 항상 먼저 평가되어 이기고, F-16 은 F-19(`Language(_)`) 보다 먼저
/// 평가된다(0x66/0x68 키코드 공유 시 결정론적 순서 — ⛔ 실제 동시 활성은 충돌 대화상자가
/// 막지만, 순서는 명시한다). `combo_rules` 와 `korean_rules` · `language_rules` 는
/// 각 기능이 채우고 `Hyperkey` 는 계층 2 modifier 규칙만 만들기 때문에 `Korean(_)` 대
/// `Hyperkey` 의 상대 순서는 실제로는 쓰이지 않지만, `RuleId` 자체의 전순서(total order)를
/// 명확히 하기 위해 derive 로 정의해 둔다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleId {
    Preset(u8),
    Korean(u8),
    Language(u8),
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
    /// ⭐ 이슈 #127 — 이 규칙의 출력이 시스템 "입력 소스 전환" 토글 핫키(⌃Space 류)인가.
    /// 참이면 물리 트리거 키를 누르고 있는 동안 오는 autorepeat KeyDown 은 같은
    /// `trigger_key` 래치가 이미 서 있을 때 **재발화하지 않고 소비만** 한다(원본도
    /// 하류로 안 흘려보낸다) — D-K6 규약(최초 down=합성 down, up=합성 up)을 "1 press
    /// = 정확히 1 chord" 로 유지한다. macOS 는 물리 ⌃Space 의 autorepeat 을 그
    /// 이벤트의 autorepeat 플래그로 무시하지만, `SynthEvent`/플랫폼 합성에는 그
    /// 플래그가 없어(§`SynthEvent` 문서) 매 반복이 독립된 토글로 오인된다(실기기
    /// 로그 실측 — 208ms 뒤 Korean→ABC 되돌아감).
    ///
    /// 거짓이면(F-16.3 한자·F-16.4 backtick) 기존과 동일하게 매 autorepeat tick 마다
    /// 재발화한다 — 이 규칙들의 출력은 시스템 토글이 아니라 **실제로 반복 타이핑되는
    /// 문자/제어 키**라, 물리 키를 계속 누르고 있으면 계속 나가는 것이 맞는 동작이다
    /// (`f16_4_autorepeat_key_down_is_substituted_every_time`).
    pub is_input_source_toggle: bool,
}

/// F-19 규칙이 참조하는 앱 제외 게이트 — F-16 §3.5 의 기능군 단위 게이트 패턴을
/// 언어별로 확장한 자리(`docs/spec/language-presets.md` §3.5, D-7).
///
/// `ultrakey-core` 는 게이트의 실제 비트를 소유하지 않는다(`gate.rs`)?
/// 아니다 — `AtomicAppGate` 가 언어별 비트를 가진다. 이 열거는 그 비트 중 무엇을
/// 읽을지 규칙에 실어 주는 선택자다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageGate {
    /// F-19.1·F-19.2 — `korean.*` 네임스페이스 규칙이라 F-16 과 같은 한국어 게이트를
    /// 읽는다(`korean_disabled`).
    Korean,
    /// F-19.3·F-19.4 — `japanese.disabled`.
    Japanese,
    /// F-19.7 — `chinese.disabled`.
    Chinese,
}

/// F-19 언어별 프리셋 규칙 하나(`docs/spec/language-presets.md` §3, P17~P23).
///
/// ⛔ `KoreanRule` 을 재사용하지 않는다 — 그 트리거(`KoreanTrigger`)는 "modifier 부재/
/// shift 만" 두 값뿐이라 캡스락·⌘ 의 **단독 탭(quick press)** 과 "option 만" 을 표현할
/// 수 없다. D-K5 가 `ComboRule` 에 대해 내린 판정(범용 기구 오염)을 그대로 따른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LanguageRule {
    /// `RuleId::Language(17)`..`Language(23)` — 규칙표 P 번호와 일치한다.
    pub id: RuleId,
    pub trigger: LanguageTrigger,
    /// 앱 게이트 선택자. `None` = 게이트 없음(F-19.5·F-19.6 — 타이핑 문자 문제라
    /// 원격 데스크톱에서도 동작해야 한다, 명세 §3.5).
    pub app_gate: Option<LanguageGate>,
    /// 참이면 JIS 키보드에서만, 거짓이면 JIS 가 아닐 때만 발화(키보드 타입 게이트).
    pub requires_jis: Option<bool>,
    pub out: LanguageOut,
}

/// F-19 규칙의 트리거 조건(`docs/spec/language-presets.md` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageTrigger {
    /// "이 키만 눌려 있고 다른 어떤 modifier 도 없을 때" 의 **단독 탭** — 캡스락·⌘·우⌘.
    /// 발화 시점은 quick press(F-08.2 와 같은 FSM)다 — 길게 누르거나 다른 키와 조합하면
    /// 홀드로 확정되어 본래 동작으로 돌아간다(⌘+W 는 ⌘+W). 명세 §3.6.
    AloneTap { key: KeyCode },
    /// "이 키 + shift 만" — F-19.6 의 shift 행(카탈로그 original 대조, 엄격판).
    ShiftOnly { key: KeyCode },
    /// "이 키만, modifier 없음" — F-19.5·F-19.6 의 비shift 행.
    NoModifier { key: KeyCode },
    /// "이 키 + option 만" — F-19.5 의 역방향(`⌥\` → `\`).
    OptionOnly { key: KeyCode },
}

/// F-19 규칙의 출력.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LanguageOut {
    /// 같은 keycode 재주입(F-16.1 과 같은 출력 경로 — `SynthEvent`). 플래그는
    /// 값 그대로(P5). `option` 경유 합성은 이 flags 로 표현한다(F-19.5, ⛔ 직접
    /// 매핑 금지).
    Key { keycode: KeyCode, flags: EventFlags },
    /// F-19.3 — 일본어 입력 소스 활성이면 `japanese_eisuu`(0x66), 아니면
    /// `japanese_kana`(0x68). 판정 불가(`Unknown`)면 kana(명세 문구 "아니면 かな").
    EisuOrKana,
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
    /// F-16(M? / D-K5). `RuleId` 오름차순으로 정렬된 상태로 주어진다고 전제한다
    /// (`ultrakey-korean::KoreanSettings::to_rules` 가 정렬해서 반환한다).
    pub korean_rules: Vec<KoreanRule>,
    /// F-19(M3 3차 / P17~P23). `RuleId` 오름차순으로 정렬된 상태로 주어진다고 전제한다
    /// (`ultrakey-korean::KoreanSettings::to_language_rules` ·
    /// `ultrakey-language-presets::settings` 가 정렬해서 반환한다). `AloneTap`
    /// 규칙은 추적 슬롯(quick press FSM)에 등록되고, 나머지는 `evaluate_language_rules`
    /// 가 KeyDown/KeyUp 으로 소비한다.
    pub language_rules: Vec<LanguageRule>,
    /// ⭐ F-01 — `Remap key to Seek:` 소스 키(명세 §4). `None` 이 `-`(미설정)이고
    /// 출고 기본값이다. 이 키의 down/up 은 `Effect::SeekTriggerDown`/`SeekTriggerUp`
    /// 으로 올라가며, **계층 2(hyper)·계층 3(preset)보다 먼저** 평가된다 —
    /// `key-remapping-engine.md` §3-b "동일 소스 키 중복 배정 방지" 가 "Seek(화면 탐색
    /// 전용 모드 진입)은 다른 어떤 리매핑보다 명백히 상위 의도" 라고 못박은 그대로다.
    pub seek_trigger: Option<KeyCode>,
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

    /// `k` 를 **단독 탭 트리거**로 주장하는 언어 규칙들(F-19 AloneTap — 캡스락·우⌘·⌘).
    /// 반환 순서는 규칙 ID 오름차순 — `evaluate`가 첫 매치를 소비한다(결정론, architecture
    /// §6.4 서두). 같은 키에 규칙이 둘 이상이어서는 안 되지만(충돌 대화상자가 막는다),
    /// find 로 안전하게 잡는다.
    pub fn language_alone_taps_for(&self, k: KeyCode) -> Vec<&LanguageRule> {
        self.language_rules
            .iter()
            .filter(|r| matches!(r.trigger, LanguageTrigger::AloneTap { key } if key == k))
            .collect()
    }

    /// `k` 가 언어 AloneTap 규칙의 트리거인가 — 추적 슬롯(FSM) 등록 여부 판정에 쓴다.
    pub fn has_language_alone_tap(&self, k: KeyCode) -> bool {
        self.language_rules.iter().any(|r| {
            matches!(r.trigger, LanguageTrigger::AloneTap { key } if key == k)
        })
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

    /// D-K5 · F-19(2026-09-03) — `Preset(_) < Korean(_) < Language(_) < Hyperkey`.
    /// 계층 3 안의 평가 순서(§3.4)의 코드 표현이다: F-08.4(`Preset(4)`)가
    /// F-16.1(`Korean(13)`)보다 먼저 평가되고, F-19(`Language(_)`)는 F-16 뒤다.
    /// ⛔ 같은 키코드(캡스락·위 ⌘·0x5D/0x2A 등)를 소스로 하는 규칙은 충돌 대화상자가
    /// 동시 활성을 막지만, 그렇지 않은 규칙끼리의 평가 순서는 이 ID 로 결정론적이다.
    #[test]
    fn rule_id_korean_and_language_variants_sort_between_preset_and_hyperkey() {
        assert!(RuleId::Preset(16) < RuleId::Korean(13));
        assert!(RuleId::Korean(13) < RuleId::Korean(16));
        assert!(RuleId::Korean(16) < RuleId::Language(17));
        assert!(RuleId::Language(17) < RuleId::Language(23));
        assert!(RuleId::Language(23) < RuleId::Hyperkey);
        assert!(RuleId::Preset(4) < RuleId::Korean(13));
        assert!(RuleId::Korean(16) < RuleId::Language(21));
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
