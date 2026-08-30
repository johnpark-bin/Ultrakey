//! `ultrakey-layout` — F-14(B) 키보드 입력 소스 독립성
//! (`localization-and-input-sources.md` §3.2).
//!
//! ⭐ **이 크레이트가 별도로 나뉜 이유**(`docs/dev/architecture.md` §1): `UCKeyTranslate`
//! 호출을 [`Translator`] 트레이트로 주입할 수 있게 만들어, 실제 macOS 키보드 레이아웃
//! 데이터 없이도 가짜(fake) 번역기로 단위 테스트할 수 있게 한다.
//!
//! ⭐ **M1 의 한계**: 이 크레이트가 검증하는 것은 "역방향 테이블이 올바르게
//! 구축·무효화되는가"까지다. 이 테이블을 실제로 소비해 문자 출력형 리매핑을
//! 수행하는 규칙(quick press 괄호/슬래시 실행, `home row = symbol row` 등)은 전부
//! F-08/M2 소관이며, M1 시점에는 아직 이 테이블을 쓰는 호출자가 없다.

#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::sync::Arc;

use arc_swap::ArcSwap;

use ultrakey_core::keycode::KeyCode;
use ultrakey_platform::text_input_source::LayoutSnapshot;

/// `UCKeyTranslate` 에 넘길 modifier 조합. 역방향 테이블이 훑는 경우의 수다
/// (`localization-and-input-sources.md` §3.2.2 (i)).
///
/// ⭐ 선언 순서가 곧 "단순함" 순서다 — [`LayoutTable::build`] 의 중복 조합 결정론이
/// 이 순서(파생 `Ord`)에 의존한다: `None < Shift < Option < ShiftOption`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ModifierCombo {
    None,
    Shift,
    Option,
    ShiftOption,
}

impl ModifierCombo {
    /// 네 조합 전부, 단순함 순서 그대로.
    pub fn all() -> &'static [ModifierCombo] {
        &[
            ModifierCombo::None,
            ModifierCombo::Shift,
            ModifierCombo::Option,
            ModifierCombo::ShiftOption,
        ]
    }

    /// `UCKeyTranslate` 의 `modifierKeyState` 인자 값.
    ///
    /// Carbon 규약: `EventRecord.modifiers` 필드의 비트를 8비트 오른쪽으로 시프트한
    /// 값을 쓴다(`shiftKey` = `1<<9` = `0x0200` → `0x02`, `optionKey` = `1<<11` =
    /// `0x0800` → `0x08`). `key-remapping-engine.md` §6 표의 "레이아웃 독립 판정" 절 참고.
    pub fn uc_key_modifiers(self) -> u32 {
        match self {
            ModifierCombo::None => 0x00,
            ModifierCombo::Shift => 0x02,
            ModifierCombo::Option => 0x08,
            ModifierCombo::ShiftOption => 0x0A,
        }
    }
}

/// `UCKeyTranslate` 호출을 추상화한다.
///
/// ⭐ 이것이 이 크레이트를 단위 테스트 가능하게 만드는 이음매다 — 실제 구현
/// ([`PlatformTranslator`])은 macOS Carbon API 를 부르고, 테스트는 하드코딩된
/// 가짜 레이아웃 표를 부르는 구현을 주입한다.
pub trait Translator: Send + Sync {
    fn translate(
        &self,
        uchr: &[u8],
        keyboard_type: u32,
        keycode: KeyCode,
        modifiers: u32,
    ) -> Option<String>;
}

/// 실제 Carbon `UCKeyTranslate` 를 부르는 구현
/// (`ultrakey_platform::text_input_source::translate` 에 그대로 위임한다).
///
/// non-macOS 타깃에서는 `ultrakey-platform` 의 스텁 경로(항상 `None`)로 컴파일만
/// 된다 — 이 크레이트 자체는 macOS 가 아닌 타깃에서도 컴파일되어야 한다.
pub struct PlatformTranslator;

impl Translator for PlatformTranslator {
    fn translate(
        &self,
        uchr: &[u8],
        keyboard_type: u32,
        keycode: KeyCode,
        modifiers: u32,
    ) -> Option<String> {
        ultrakey_platform::text_input_source::translate(uchr, keyboard_type, keycode, modifiers)
    }
}

/// 한 입력 소스에 대한 정/역방향 조회 테이블(`localization-and-input-sources.md` §3.2.5 항목 1).
pub struct LayoutTable {
    source_id: String,
    used_ascii_fallback: bool,
    /// ⭐ F-16 이 요구해 F-14 가 스냅샷에 실은 값을 그대로 나른다(§3.2.8) — 이
    /// 크레이트는 "한국어인가" 판정을 다시 하지 않는다(`used_ascii_fallback` 과
    /// 같은 규약). 판정은 `ultrakey-core::korean` 소관.
    original_source_id: String,
    original_languages: Vec<String>,
    /// 정방향: (물리 키, modifier 조합) → 문자.
    forward: HashMap<(KeyCode, ModifierCombo), String>,
    /// 역방향(§3.2.2 (i)): 목표 문자 → 그것을 내는 (물리 키, modifier 조합).
    reverse: HashMap<char, (KeyCode, ModifierCombo)>,
}

/// 물리 keycode 는 0..=127 범위를 훑는다 — macOS virtual keycode(`CGKeyCode`)의
/// 실질적인 상한이다(ANSI 키보드 기준 키 전부를 포함한다).
const MAX_KEYCODE: u16 = 127;

impl LayoutTable {
    /// 스냅샷 + 번역기로 테이블을 구축한다.
    ///
    /// ⭐ 순수 함수다 — macOS 나 실제 `UCKeyTranslate` 없이, 가짜 [`Translator`] 로
    /// 완전히 테스트된다.
    ///
    /// 모든 물리 keycode(0..=127) × [`ModifierCombo::all`] 을 정방향 번역해 결과
    /// 문자열을 기록한다. dead key(빈 문자열 또는 `None`)는 건너뛴다.
    ///
    /// **중복 조합 결정론**: 같은 문자를 여러 (keycode, combo) 조합이 낼 수 있으면,
    /// **더 단순한 조합**(`None < Shift < Option < ShiftOption`)을, 조합이 같다면
    /// **더 작은 keycode** 를 역방향 테이블의 대표값으로 고정한다. 이를 위해 바깥
    /// 루프를 조합 단순도 순서로, 안쪽 루프를 keycode 오름차순으로 돌며 "먼저 채워진
    /// 값을 덮어쓰지 않는다" — 이 순서 자체가 우선순위 규칙을 구현한다.
    pub fn build(snapshot: &LayoutSnapshot, translator: &dyn Translator) -> Self {
        let mut forward = HashMap::new();
        let mut reverse = HashMap::new();

        for &combo in ModifierCombo::all() {
            let modifiers = combo.uc_key_modifiers();
            for raw in 0..=MAX_KEYCODE {
                let keycode = KeyCode(raw);
                let Some(text) = translator.translate(
                    &snapshot.uchr_data,
                    snapshot.keyboard_type,
                    keycode,
                    modifiers,
                ) else {
                    continue;
                };
                if text.is_empty() {
                    // dead key — UCKeyTranslate 가 빈 문자열을 돌려준 경우.
                    continue;
                }

                // 역방향 테이블은 "정확히 한 글자"를 내는 조합만 등록한다. 여러
                // 글자를 내는 결과(리거처 등)는 정방향 조회로만 노출한다.
                if let Some(ch) = single_char(&text) {
                    // 먼저 채워진 항목을 유지한다 — 위 루프 순서가 우선순위다.
                    reverse.entry(ch).or_insert((keycode, combo));
                }

                forward.insert((keycode, combo), text);
            }
        }

        LayoutTable {
            source_id: snapshot.source_id.clone(),
            used_ascii_fallback: snapshot.used_ascii_fallback,
            original_source_id: snapshot.original_source_id.clone(),
            original_languages: snapshot.original_languages.clone(),
            forward,
            reverse,
        }
    }

    /// 입력 소스가 없을 때의 빈 테이블(모든 조회가 `None`).
    pub fn empty() -> Self {
        LayoutTable {
            source_id: String::new(),
            used_ascii_fallback: false,
            original_source_id: String::new(),
            original_languages: Vec::new(),
            forward: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    /// ⭐ CJK IME 활성 시 ASCII 가능 레이아웃으로 폴백했는가(§3.2.6). 스냅샷의 값을
    /// 그대로 실어 나른다 — 이 크레이트가 그 판정을 다시 하지 않는다.
    pub fn used_ascii_fallback(&self) -> bool {
        self.used_ascii_fallback
    }

    /// ⭐ F-16 요구(§3.2.8) — ASCII 폴백 교체 **이전** 원본 입력 소스의 ID.
    /// 스냅샷 값을 그대로 실어 나른다.
    pub fn original_source_id(&self) -> &str {
        &self.original_source_id
    }

    /// ⭐ F-16 요구(§3.2.8) — 위와 같은 원본 소스의 언어 태그 목록. "한국어인가"
    /// 판정은 이 크레이트가 하지 않는다 — `ultrakey-core::korean` 소관.
    pub fn original_languages(&self) -> &[String] {
        &self.original_languages
    }

    /// 정방향: 물리 키 + 조합 → 문자.
    pub fn char_for(&self, keycode: KeyCode, combo: ModifierCombo) -> Option<&str> {
        self.forward.get(&(keycode, combo)).map(String::as_str)
    }

    /// ⭐ 역방향(§3.2.2 (i)): 목표 문자 → 그것을 내는 물리 키 + 조합.
    ///
    /// 탐색이 실패하면 `None` 을 돌려준다 — 호출자(F-08 등)가 이를 §3.2.2 (ii)
    /// `CGEventKeyboardSetUnicodeString` 폴백으로 이어받을지 판단할 수 있도록,
    /// 조용한 무동작이 되지 않게 한다(§5 엣지 케이스 12).
    pub fn keycode_for_char(&self, c: char) -> Option<(KeyCode, ModifierCombo)> {
        self.reverse.get(&c).copied()
    }

    pub fn len(&self) -> usize {
        self.forward.len()
    }

    pub fn is_empty(&self) -> bool {
        self.forward.is_empty()
    }
}

/// 문자열이 정확히 한 글자(유니코드 스칼라 값 하나)로 이루어져 있으면 그 글자를
/// 돌려준다.
fn single_char(s: &str) -> Option<char> {
    let mut chars = s.chars();
    let first = chars.next()?;
    if chars.next().is_some() {
        None
    } else {
        Some(first)
    }
}

/// 현재 테이블을 무잠금으로 게시·조회한다(`docs/dev/architecture.md` §2.2 —
/// "설정·규칙 테이블, 레이아웃 역산 테이블" 행: 콜백이 매 이벤트 읽는 쪽은 대기
/// 없이(wait-free) 읽고, 갱신은 원자적 포인터 교체로 이루어진다).
pub struct LayoutResolver {
    current: ArcSwap<LayoutTable>,
}

impl LayoutResolver {
    /// 빈 테이블로 시작한다.
    pub fn new() -> Self {
        LayoutResolver {
            current: ArcSwap::from_pointee(LayoutTable::empty()),
        }
    }

    /// ⭐ 콜백(임계 경로)이 부르는 쪽. 대기 없이 현재 테이블을 얻는다.
    pub fn current(&self) -> Arc<LayoutTable> {
        self.current.load_full()
    }

    /// TIS 를 다시 읽어 테이블을 재구축하고 원자적으로 교체한다.
    ///
    /// §3.2.5 항목 3: "즉시 폐기하고 재구축" — 지연 재구축이 아니다. 다만 `TIS`
    /// 조회 자체가 실패하면(§3-e 5단계 절차 실패, `current_layout()` 이 `None`),
    /// **기존 테이블을 그대로 유지하고 `false` 를 반환한다** — 일시적인 조회 실패
    /// 때문에 이미 쓸 수 있는 테이블을 버리지 않는다.
    pub fn rebuild(&self) -> bool {
        match ultrakey_platform::text_input_source::current_layout() {
            Some(snapshot) => self.rebuild_with(&snapshot, &PlatformTranslator),
            None => {
                tracing::debug!(
                    "레이아웃 스냅샷 조회 실패 — 기존 LayoutTable 을 유지한다"
                );
                false
            }
        }
    }

    /// 테스트·주입용: 주어진 스냅샷 + 번역기로 재구축하고 항상 교체·`true` 를
    /// 반환한다.
    pub fn rebuild_with(&self, snapshot: &LayoutSnapshot, translator: &dyn Translator) -> bool {
        self.publish(LayoutTable::build(snapshot, translator));
        true
    }

    pub fn publish(&self, table: LayoutTable) {
        self.current.store(Arc::new(table));
    }
}

impl Default for LayoutResolver {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// US-QWERTY 일부만 흉내 낸 가짜 번역기.
    struct FakeQwerty;

    impl Translator for FakeQwerty {
        fn translate(
            &self,
            _uchr: &[u8],
            _keyboard_type: u32,
            keycode: KeyCode,
            modifiers: u32,
        ) -> Option<String> {
            let shift = modifiers == ModifierCombo::Shift.uc_key_modifiers();
            let none = modifiers == ModifierCombo::None.uc_key_modifiers();
            match keycode {
                KeyCode::ANSI_A if none => Some("a".into()),
                KeyCode::ANSI_A if shift => Some("A".into()),
                KeyCode::ANSI_W if none => Some("w".into()),
                KeyCode::ANSI_W if shift => Some("W".into()),
                KeyCode::ANSI_SEMICOLON if none => Some(";".into()),
                KeyCode::ANSI_SEMICOLON if shift => Some(":".into()),
                // 숫자 줄 일부: kVK_ANSI_9 = 0x19, kVK_ANSI_0 = 0x1D.
                KeyCode(0x19) if none => Some("9".into()),
                KeyCode(0x19) if shift => Some("(".into()),
                KeyCode(0x1D) if none => Some("0".into()),
                KeyCode(0x1D) if shift => Some(")".into()),
                // dead key 흉내: kVK_ANSI_LeftBracket(0x21) + Option → 빈 문자열.
                KeyCode(0x21) if modifiers == ModifierCombo::Option.uc_key_modifiers() => {
                    Some(String::new())
                }
                _ => None,
            }
        }
    }

    /// ⭐ AZERTY 흉내 — 숫자 줄이 Shift 없이 기호를 내고, Shift 를 눌러야 숫자가
    /// 나오는 반전 배치(§3.2.4). `(`/`)` 가 QWERTY 와 다른 keycode/조합에서
    /// 나온다. 실제 French AZERTY 를 본떠, `;` 물리 위치(`ANSI_SEMICOLON`)에는
    /// `M`/`?` 가 배정된다.
    struct FakeAzerty;

    impl Translator for FakeAzerty {
        fn translate(
            &self,
            _uchr: &[u8],
            _keyboard_type: u32,
            keycode: KeyCode,
            modifiers: u32,
        ) -> Option<String> {
            let shift = modifiers == ModifierCombo::Shift.uc_key_modifiers();
            let none = modifiers == ModifierCombo::None.uc_key_modifiers();
            match keycode {
                // 물리 "5" 키(kVK_ANSI_5 = 0x17): 반전 배치로 unshifted 가 "(".
                KeyCode(0x17) if none => Some("(".into()),
                KeyCode(0x17) if shift => Some("5".into()),
                // 물리 "-" 키(kVK_ANSI_Minus = 0x1B): unshifted 가 ")".
                KeyCode(0x1B) if none => Some(")".into()),
                KeyCode(0x1B) if shift => Some("0".into()),
                // ANSI_SEMICOLON 물리 위치 — 프랑스어 AZERTY 실제 배치처럼 M/?.
                KeyCode::ANSI_SEMICOLON if none => Some("m".into()),
                KeyCode::ANSI_SEMICOLON if shift => Some("M".into()),
                _ => None,
            }
        }
    }

    /// 아무것도 번역하지 못하는 가짜 번역기.
    struct FakeEmpty;

    impl Translator for FakeEmpty {
        fn translate(
            &self,
            _uchr: &[u8],
            _keyboard_type: u32,
            _keycode: KeyCode,
            _modifiers: u32,
        ) -> Option<String> {
            None
        }
    }

    fn fake_snapshot(source_id: &str, used_ascii_fallback: bool) -> LayoutSnapshot {
        LayoutSnapshot {
            source_id: source_id.to_string(),
            is_ascii_capable: true,
            used_ascii_fallback,
            keyboard_type: 0,
            uchr_data: Vec::new(),
            original_source_id: String::new(),
            original_languages: Vec::new(),
        }
    }

    // 1. LayoutTable::build — 정방향 조회가 맞다.
    #[test]
    fn forward_lookup_matches_translator() {
        let table = LayoutTable::build(&fake_snapshot("qwerty", false), &FakeQwerty);
        assert_eq!(table.char_for(KeyCode::ANSI_A, ModifierCombo::None), Some("a"));
        assert_eq!(table.char_for(KeyCode::ANSI_A, ModifierCombo::Shift), Some("A"));
        assert_eq!(table.char_for(KeyCode::ANSI_W, ModifierCombo::None), Some("w"));
        assert_eq!(
            table.char_for(KeyCode::ANSI_SEMICOLON, ModifierCombo::Shift),
            Some(":")
        );
        // 등록되지 않은 조합은 None.
        assert_eq!(table.char_for(KeyCode::ANSI_A, ModifierCombo::Option), None);
    }

    // 2. 역방향 조회 — keycode_for_char('a') 가 기대 keycode 를 준다.
    #[test]
    fn reverse_lookup_finds_expected_keycode() {
        let table = LayoutTable::build(&fake_snapshot("qwerty", false), &FakeQwerty);
        assert_eq!(
            table.keycode_for_char('a'),
            Some((KeyCode::ANSI_A, ModifierCombo::None))
        );
        assert_eq!(
            table.keycode_for_char('W'),
            Some((KeyCode::ANSI_W, ModifierCombo::Shift))
        );
    }

    // 3. ⭐ 레이아웃 독립성의 핵심: '(' / ')' 가 QWERTY·AZERTY 에서 서로 다른
    //    (keycode, combo) 로 나온다 — §8 (B) 첫 수용 기준의 자동 검증.
    #[test]
    fn parenthesis_come_from_different_physical_keys_per_layout() {
        let qwerty = LayoutTable::build(&fake_snapshot("qwerty", false), &FakeQwerty);
        let azerty = LayoutTable::build(&fake_snapshot("azerty", false), &FakeAzerty);

        let qwerty_open = qwerty.keycode_for_char('(').expect("qwerty 는 ( 를 낼 수 있어야 한다");
        let azerty_open = azerty.keycode_for_char('(').expect("azerty 는 ( 를 낼 수 있어야 한다");
        assert_ne!(qwerty_open, azerty_open);

        let qwerty_close = qwerty.keycode_for_char(')').expect("qwerty 는 ) 를 낼 수 있어야 한다");
        let azerty_close = azerty.keycode_for_char(')').expect("azerty 는 ) 를 낼 수 있어야 한다");
        assert_ne!(qwerty_close, azerty_close);
    }

    // 4. ⭐ 반대로 물리 키 판정은 레이아웃과 무관하다: 같은 KeyCode::ANSI_SEMICOLON
    //    이 두 레이아웃에서 서로 다른 문자를 내지만 keycode 자체는 동일하다.
    #[test]
    fn same_physical_keycode_yields_different_chars_per_layout() {
        let qwerty = LayoutTable::build(&fake_snapshot("qwerty", false), &FakeQwerty);
        let azerty = LayoutTable::build(&fake_snapshot("azerty", false), &FakeAzerty);

        assert_eq!(
            qwerty.char_for(KeyCode::ANSI_SEMICOLON, ModifierCombo::None),
            Some(";")
        );
        assert_eq!(
            azerty.char_for(KeyCode::ANSI_SEMICOLON, ModifierCombo::None),
            Some("m")
        );
        assert_ne!(
            qwerty.char_for(KeyCode::ANSI_SEMICOLON, ModifierCombo::None),
            azerty.char_for(KeyCode::ANSI_SEMICOLON, ModifierCombo::None)
        );
    }

    // 5. 중복 조합 결정론 — 같은 문자를 두 조합이 내면 항상 더 단순한 쪽을
    //    고른다. 여러 번 build 해도 같은 결과.
    struct DupTranslator;

    impl Translator for DupTranslator {
        fn translate(
            &self,
            _uchr: &[u8],
            _keyboard_type: u32,
            keycode: KeyCode,
            modifiers: u32,
        ) -> Option<String> {
            // 'x' 를 세 조합에서 낸다: (keycode=5, Shift), (keycode=3, None),
            // (keycode=2, None). 우선순위 규칙대로면 (2, None) 이 이겨야 한다 —
            // combo 단순도가 같은 None 그룹 안에서 keycode 가 더 작다.
            match (keycode.0, modifiers) {
                (5, m) if m == ModifierCombo::Shift.uc_key_modifiers() => Some("x".into()),
                (3, m) if m == ModifierCombo::None.uc_key_modifiers() => Some("x".into()),
                (2, m) if m == ModifierCombo::None.uc_key_modifiers() => Some("x".into()),
                _ => None,
            }
        }
    }

    #[test]
    fn duplicate_combo_resolution_is_deterministic() {
        for _ in 0..5 {
            let table = LayoutTable::build(&fake_snapshot("dup", false), &DupTranslator);
            assert_eq!(
                table.keycode_for_char('x'),
                Some((KeyCode(2), ModifierCombo::None))
            );
        }
    }

    // 6. dead key(빈 문자열)는 테이블에 들어가지 않는다.
    #[test]
    fn dead_key_is_excluded_from_table() {
        let table = LayoutTable::build(&fake_snapshot("qwerty", false), &FakeQwerty);
        assert_eq!(
            table.char_for(KeyCode(0x21), ModifierCombo::Option),
            None
        );
    }

    // 7. FakeEmpty → is_empty(), 모든 조회가 None.
    #[test]
    fn empty_translator_yields_empty_table() {
        let table = LayoutTable::build(&fake_snapshot("empty", false), &FakeEmpty);
        assert!(table.is_empty());
        assert_eq!(table.len(), 0);
        assert_eq!(table.char_for(KeyCode::ANSI_A, ModifierCombo::None), None);
        assert_eq!(table.keycode_for_char('a'), None);
    }

    #[test]
    fn empty_table_constructor_matches_empty_translator_result() {
        let table = LayoutTable::empty();
        assert!(table.is_empty());
        assert_eq!(table.source_id(), "");
        assert!(!table.used_ascii_fallback());
        assert_eq!(table.keycode_for_char('a'), None);
    }

    // 8. used_ascii_fallback 이 스냅샷에서 그대로 전달된다.
    #[test]
    fn used_ascii_fallback_is_passed_through_from_snapshot() {
        let fallback_table = LayoutTable::build(&fake_snapshot("cjk", true), &FakeQwerty);
        assert!(fallback_table.used_ascii_fallback());

        let normal_table = LayoutTable::build(&fake_snapshot("qwerty", false), &FakeQwerty);
        assert!(!normal_table.used_ascii_fallback());
    }

    // 8-b. ⭐ F-16 요구(§3.2.8) — original_source_id/original_languages 가 스냅샷에서
    // 테이블로 그대로 전달된다. 이 크레이트는 "한국어인가" 판정을 다시 하지 않는다.
    #[test]
    fn original_source_fields_are_passed_through_from_snapshot() {
        let mut snapshot = fake_snapshot("us-ascii-fallback", true);
        snapshot.original_source_id = "com.apple.inputmethod.Korean.2SetKorean".to_string();
        snapshot.original_languages = vec!["ko".to_string()];

        let table = LayoutTable::build(&snapshot, &FakeQwerty);
        assert_eq!(
            table.original_source_id(),
            "com.apple.inputmethod.Korean.2SetKorean"
        );
        assert_eq!(table.original_languages(), &["ko".to_string()]);

        // 원본이 비어 있으면 그대로 비어 있는 채 전달된다.
        let empty = LayoutTable::build(&fake_snapshot("qwerty", false), &FakeQwerty);
        assert_eq!(empty.original_source_id(), "");
        assert!(empty.original_languages().is_empty());
    }

    #[test]
    fn empty_table_has_empty_original_source_fields() {
        let table = LayoutTable::empty();
        assert_eq!(table.original_source_id(), "");
        assert!(table.original_languages().is_empty());
    }

    // 9. LayoutResolver — publish 후 current() 가 새 테이블을 준다. rebuild_with
    //    로 교체되면 이전 Arc 를 들고 있던 쪽은 여전히 옛 테이블을 본다(무잠금
    //    교체의 의미론).
    #[test]
    fn resolver_publish_and_lock_free_swap_semantics() {
        let resolver = LayoutResolver::new();
        assert!(resolver.current().is_empty());

        let old = resolver.current();

        let replaced = resolver.rebuild_with(&fake_snapshot("qwerty", false), &FakeQwerty);
        assert!(replaced);

        // 교체 전에 얻은 Arc 는 여전히 옛(빈) 테이블을 가리킨다.
        assert!(old.is_empty());

        let new = resolver.current();
        assert!(!new.is_empty());
        assert_eq!(new.source_id(), "qwerty");

        // publish() 로도 직접 교체할 수 있다.
        resolver.publish(LayoutTable::build(&fake_snapshot("azerty", false), &FakeAzerty));
        let latest = resolver.current();
        assert_eq!(latest.source_id(), "azerty");
        // 이전 Arc(`new`)는 여전히 qwerty 를 가리킨다.
        assert_eq!(new.source_id(), "qwerty");
    }

    #[test]
    fn resolver_default_starts_empty() {
        let resolver = LayoutResolver::default();
        assert!(resolver.current().is_empty());
    }
}
