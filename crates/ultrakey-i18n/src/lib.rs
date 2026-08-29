//! F-14(A) 문자열 카탈로그 — 제품 결정 D4(한국어 + 영어 현지화 채택, 이슈 #5).
//!
//! ⭐ `docs/spec/localization-and-input-sources.md` §3.1.5 "단일 소스" 결정을 그대로
//! 따른다: 로케일당 카탈로그 파일 1개(`resources/i18n/<locale>.json`, 평평한 키-값
//! JSON)를 앱 리소스로 두고, 네이티브 Rust 와 WebView 양쪽이 **같은 파일**을 읽는다.
//! 카탈로그가 두 벌이면 번역 drift 가 생기고 검증 대상이 두 배로 늘어난다는 것이
//! 그 근거다.
//!
//! 이 크레이트는 그 결정 중 **네이티브 측 절반**만 맡는다 — 카탈로그 파일을
//! [`include_str!`] 로 컴파일 타임에 임베드해, 번들 안에서 리소스 파일 경로를
//! 런타임에 찾아야 하는 문제 자체를 없앤다(§7 구현 접근). 같은 파일을 Tauri 정적
//! 리소스로도 서빙해 WebView 가 `fetch` 로 읽게 하는 설정은 이 크레이트의 책임이
//! 아니라 `apps/ultrakey-app` 소관이다 — 카탈로그 파일은 여기서 소유하되, 번들링
//! 방법은 앱 크레이트가 결정한다.

use std::collections::HashMap;

/// 클론이 지원하기로 결정한 로케일(제품 결정 D4: 한국어 + 영어).
///
/// §3.1.1 은 "확정된 로케일 목록 없음, 클론이 정할 문제"라고 판정했고, D4 로
/// 한국어·영어 두 개로 확정됐다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    En,
    Ko,
}

impl Locale {
    /// ISO 639-1 언어 코드. 카탈로그 파일명·리소스 조회 키로도 쓰인다.
    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ko => "ko",
        }
    }

    /// BCP-47 언어 태그에서 로케일을 결정한다.
    ///
    /// §3.1.2 절차 3 은 "정확히 일치하는 지역 변형이 없으면 언어 코드만으로
    /// 폴백한다"고 정했다 — 이 함수는 그 폴백까지 포함해, 태그의 첫 서브태그
    /// (언어 서브태그)만 보고 판정한다. 스크립트·지역 서브태그(`ko-Kore-KR` 의
    /// `Kore`·`KR`)는 무시한다.
    pub fn from_language_tag(tag: &str) -> Option<Self> {
        let lang = tag.split('-').next().unwrap_or(tag);
        match lang.to_ascii_lowercase().as_str() {
            "en" => Some(Locale::En),
            "ko" => Some(Locale::Ko),
            _ => None,
        }
    }

    /// 클론이 지원하는 로케일 전체 목록.
    pub fn all() -> &'static [Locale] {
        &[Locale::En, Locale::Ko]
    }
}

/// en 카탈로그 원본. 컴파일 타임에 임베드된다.
const EN_JSON: &str = include_str!("../../../resources/i18n/en.json");
/// ko 카탈로그 원본. 컴파일 타임에 임베드된다.
const KO_JSON: &str = include_str!("../../../resources/i18n/ko.json");

/// 임베드된 카탈로그 JSON 을 평평한 키-값 맵으로 파싱한다.
///
/// 임베드된 파일은 빌드 산출물의 일부이므로, 파싱 실패는 리소스 파일 자체가
/// 손상되었다는 뜻이다 — 커버리지 테스트(하단)가 이 조건을 미리 잡아내므로
/// 런타임 방어보다 `expect` 로 즉시 드러내는 쪽을 택한다.
fn parse(json: &'static str) -> HashMap<String, String> {
    serde_json::from_str(json)
        .expect("내장된 i18n 카탈로그 JSON 파싱 실패 — resources/i18n 파일이 손상되었습니다")
}

fn raw_for(locale: Locale) -> &'static str {
    match locale {
        Locale::En => EN_JSON,
        Locale::Ko => KO_JSON,
    }
}

/// 로케일 하나에 대해 로드된 문자열 카탈로그.
pub struct Catalog {
    locale: Locale,
    entries: HashMap<String, String>,
    /// `locale` 이 En 이 아닐 때만 채워지는 en 폴백 맵(§5 항목 11).
    fallback: Option<HashMap<String, String>>,
}

impl Catalog {
    /// 시스템 선호 언어 목록(앞에서부터 우선순위)으로 로케일을 결정해 로드한다.
    ///
    /// §3.1.2: 목록을 앞에서부터 훑어 처음으로 지원 로케일에 매핑되는 태그를
    /// 채택한다. 정확히 일치하는 지역 변형이 없으면 언어 코드만으로 폴백하고
    /// (`Locale::from_language_tag` 가 이미 수행), 끝까지 매핑되는 태그가 없으면
    /// `en` 으로 폴백한다.
    pub fn resolve(preferred_language_tags: &[String]) -> Self {
        for tag in preferred_language_tags {
            if let Some(locale) = Locale::from_language_tag(tag) {
                return Self::for_locale(locale);
            }
        }
        Self::for_locale(Locale::En)
    }

    /// 로케일을 직접 지정해 카탈로그를 로드한다.
    pub fn for_locale(locale: Locale) -> Self {
        let entries = parse(raw_for(locale));
        let fallback = match locale {
            Locale::En => None,
            _ => Some(parse(EN_JSON)),
        };
        Self {
            locale,
            entries,
            fallback,
        }
    }

    /// 이 카탈로그가 로드된 로케일.
    pub fn locale(&self) -> Locale {
        self.locale
    }

    /// 자기 로케일 맵과 en 폴백 맵만 조회한다(입력 키를 그대로 반환하지 않는다).
    fn lookup(&self, key: &str) -> Option<&str> {
        self.entries
            .get(key)
            .or_else(|| self.fallback.as_ref().and_then(|fb| fb.get(key)))
            .map(String::as_str)
    }

    /// 키를 조회한다. 자기 로케일에 없으면 en 으로 개별 폴백한다(§5 항목 11).
    /// 그래도 없으면 키 자체를 반환한다 — 존재하지 않는 키 조회가 패닉으로
    /// 이어지지 않는다.
    ///
    /// `key` 와 반환값의 수명을 함께 묶어 두는 이유: 폴백 경로에서 `key` 인자
    /// 자체를 그대로 돌려줘야 하기 때문이다. 실제 호출부는 거의 항상
    /// `'static` 문자열 리터럴을 넘기므로 이 제약은 실사용에 영향을 주지
    /// 않는다.
    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.lookup(key).unwrap_or(key)
    }

    /// `{0}`, `{1}` … 위치 인자를 순서대로 치환한다.
    pub fn format(&self, key: &str, args: &[&str]) -> String {
        let template = self.lookup(key).unwrap_or(key);
        let mut result = template.to_string();
        for (i, arg) in args.iter().enumerate() {
            result = result.replace(&format!("{{{i}}}"), arg);
        }
        result
    }

    /// macOS 13 Ventura 에서 "System Preferences" 가 "System Settings" 로 개명된
    /// 것에 대응하는 헬퍼(F-11 §2 S2 항목 4 / §3.2). `macos_major` 가 13 미만이면
    /// `<key>.legacy` 를, 13 이상이면 `<key>` 를 조회한다.
    ///
    /// `<key>.legacy` 자체가 카탈로그에 없으면(모든 OS 변형 키가 이를 정의하는
    /// 것은 아니므로) 조용히 기본 키로 되돌아간다 — 이후 폴백 경로(en → 키 자체)는
    /// [`Catalog::get`] 과 동일하다.
    pub fn get_os_variant<'a>(&'a self, key: &'a str, macos_major: u32) -> &'a str {
        if macos_major < 13 {
            let legacy_key = format!("{key}.legacy");
            if let Some(value) = self.lookup(&legacy_key) {
                return value;
            }
        }
        self.get(key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// 리소스 파일을 직접 파싱해 `serde_json::Value` 트리로 돌려준다. 카탈로그
    /// 파일 자체의 구조(평평함·값 타입)를 검증하는 테스트 전용 헬퍼다.
    fn raw_value(json: &str) -> serde_json::Value {
        serde_json::from_str(json).expect("카탈로그 JSON 파싱 실패")
    }

    fn key_set(value: &serde_json::Value) -> BTreeSet<String> {
        value
            .as_object()
            .expect("카탈로그 최상위는 객체여야 한다")
            .keys()
            .cloned()
            .collect()
    }

    /// ⭐ 커버리지 테스트 — §8(A) 수용 기준("두 경로 사이에 번역 키 누락 차이가
    /// 없다")을 자동으로 지키는 장치. en/ko 의 키 집합이 완전히 같아야 한다.
    #[test]
    fn en_ko_key_sets_are_identical() {
        let en = key_set(&raw_value(EN_JSON));
        let ko = key_set(&raw_value(KO_JSON));

        let only_in_en: Vec<_> = en.difference(&ko).cloned().collect();
        let only_in_ko: Vec<_> = ko.difference(&en).cloned().collect();

        assert!(
            only_in_en.is_empty() && only_in_ko.is_empty(),
            "en/ko 카탈로그 키 집합이 다릅니다.\nen 에만 있음: {only_in_en:?}\nko 에만 있음: {only_in_ko:?}"
        );
    }

    /// 두 파일 모두 유효한 flat JSON — 값이 전부 문자열이고 중첩이 없다.
    #[test]
    fn catalogs_are_flat_string_maps() {
        for (name, json) in [("en", EN_JSON), ("ko", KO_JSON)] {
            let value = raw_value(json);
            let object = value
                .as_object()
                .unwrap_or_else(|| panic!("{name} 카탈로그 최상위는 객체여야 한다"));
            for (key, v) in object {
                assert!(
                    v.is_string(),
                    "{name} 카탈로그의 \"{key}\" 값이 문자열이 아닙니다(중첩 금지): {v:?}"
                );
            }
        }
    }

    #[test]
    fn from_language_tag_matches_language_subtag_only() {
        assert_eq!(Locale::from_language_tag("ko"), Some(Locale::Ko));
        assert_eq!(Locale::from_language_tag("ko-KR"), Some(Locale::Ko));
        assert_eq!(Locale::from_language_tag("ko-Kore-KR"), Some(Locale::Ko));
        assert_eq!(Locale::from_language_tag("en-US"), Some(Locale::En));
        assert_eq!(Locale::from_language_tag("de-CH"), None);
        assert_eq!(Locale::from_language_tag("ja"), None);
    }

    #[test]
    fn resolve_scans_preferred_tags_in_order() {
        let tags = vec!["de-CH".to_string(), "ko-KR".to_string()];
        let catalog = Catalog::resolve(&tags);
        assert_eq!(catalog.locale(), Locale::Ko);
    }

    #[test]
    fn resolve_falls_back_to_en_when_nothing_matches() {
        let catalog = Catalog::resolve(&[]);
        assert_eq!(catalog.locale(), Locale::En);
    }

    #[test]
    fn format_substitutes_positional_args() {
        let catalog = Catalog::for_locale(Locale::En);
        let message = catalog.format("menu.ignore_app", &["Ghostty"]);
        assert_eq!(message, "Ignore Ghostty");
    }

    #[test]
    fn get_falls_back_to_en_then_to_key_itself() {
        let catalog = Catalog::for_locale(Locale::Ko);
        // 존재하지 않는 키 — en 에도 없으므로 키 자체를 반환해야 하며 패닉이
        // 나서는 안 된다.
        assert_eq!(catalog.get("no.such.key"), "no.such.key");
    }

    #[test]
    fn get_os_variant_differs_by_macos_major() {
        let catalog = Catalog::for_locale(Locale::En);
        let modern = catalog.get_os_variant("permissions.accessibility.open_settings", 13);
        let legacy = catalog.get_os_variant("permissions.accessibility.open_settings", 12);
        assert_eq!(modern, "Open System Settings");
        assert_eq!(legacy, "Open System Preferences");
        assert_ne!(modern, legacy);
    }

    /// §3.1.3: 브랜드 고유명사는 번역 대상이 아니다 — `app.name` 은 로케일과
    /// 무관하게 항상 `Ultrakey` 다.
    #[test]
    fn app_name_is_not_translated() {
        let en = Catalog::for_locale(Locale::En);
        let ko = Catalog::for_locale(Locale::Ko);
        assert_eq!(en.get("app.name"), "Ultrakey");
        assert_eq!(ko.get("app.name"), "Ultrakey");
    }
}
