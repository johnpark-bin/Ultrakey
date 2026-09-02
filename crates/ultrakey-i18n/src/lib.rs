//! F-14(A) 문자열 카탈로그 — 제품 결정 D4(한국어 + 영어 현지화 채택, 이슈 #5),
//! 이슈 #39 로 `zh`·`es`·`ja` 를 더해 5종으로 확장.
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
//!
//! ## 왜 5종인가
//!
//! §3.1.1 은 원래 D4 로 한국어·영어 두 개만 확정했으나, 이슈 #39(사용자 요구)로
//! `zh`(간체 중국어)·`es`(스페인어)·`ja`(일본어)가 더해져 5종이 됐다. 이 확장도
//! 원본 SuperKey 를 따라가는 것이 아니라 **클론 고유의 제품 결정**이라는 D4 의
//! 성격은 그대로다 — SuperKey 는 애초에 현지화 기능이 없다(§1).

use std::collections::HashMap;

/// 클론이 지원하기로 결정한 로케일(제품 결정 D4, 이슈 #39 로 5종 확장).
///
/// §3.1.1 표: `en`(기본/폴백) · `ko` · `zh`(간체) · `es` · `ja`. 다섯 개 전부
/// LTR 이라 §3.1.4 의 RTL 미러링 규칙은 현재 적용 대상이 없다.
///
/// ⚠️ **번체 중국어(`zh-Hant`·`zh-TW`·`zh-HK`)는 이 목록에 없다.** 간체 카탈로그로
/// 대신 채우지 않는다 — 두 표기 체계는 어휘까지 다르고, 간체를 번체 사용자에게
/// 보여주는 것이 영어를 보여주는 것보다 낫다는 근거가 없다(§3.1.1). 번체가
/// 필요해지면 `zh-Hant` 를 **별도 카탈로그로** 추가하는 것이 맞는 방향이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Locale {
    En,
    Ko,
    Zh,
    Es,
    Ja,
}

impl Locale {
    /// ISO 639-1 언어 코드. 카탈로그 파일명·리소스 조회 키로도 쓰인다.
    pub fn code(self) -> &'static str {
        match self {
            Locale::En => "en",
            Locale::Ko => "ko",
            Locale::Zh => "zh",
            Locale::Es => "es",
            Locale::Ja => "ja",
        }
    }

    /// `General` 탭 언어 선택 UI 에 쓰는, **그 언어 자신의 이름**(endonym).
    ///
    /// ⭐ 현재 UI 로케일이 무엇이든 항상 같은 값을 반환한다 — `Korean`·`Chinese`
    /// 처럼 현재 UI 언어로 이름을 쓰면, 잘못된 언어로 앱이 떠서 아무것도 못
    /// 읽는 사용자가 자기 언어를 목록에서 찾지 못한다. 언어 선택 팝업은 "지금
    /// UI 를 읽을 수 없는 사람"이 쓰는 유일한 컨트롤이므로, 그 목록만은 현재
    /// 로케일과 무관해야 한다(`localization-and-input-sources.md` §3.1.2-a).
    /// (`System` 항목은 이 함수의 범위 밖이다 — 그것을 고르는 사람은 이미 UI 를
    /// 읽을 수 있는 상태이므로 일반 카탈로그 키로 번역한다.)
    pub fn endonym(self) -> &'static str {
        match self {
            Locale::En => "English",
            Locale::Ko => "한국어",
            Locale::Zh => "中文",
            Locale::Es => "Español",
            Locale::Ja => "日本語",
        }
    }

    /// BCP-47 언어 태그에서 로케일을 결정한다.
    ///
    /// §3.1.2 절차 3 은 "정확히 일치하는 지역 변형이 없으면 언어 코드만으로
    /// 폴백한다"고 정했다 — 이 함수는 그 폴백까지 포함해, 태그의 첫 서브태그
    /// (언어 서브태그)만 보고 판정한다. 스크립트·지역 서브태그(`ko-Kore-KR` 의
    /// `Kore`·`KR`)는 원칙적으로 무시한다.
    ///
    /// ⭐ 단 하나의 예외가 `zh` 다 — 번체 중국어를 간체 카탈로그로 접지 않기
    /// 위해(§3.1.1), `zh` 는 언어 서브태그만으로 판정을 끝내지 않고 스크립트·
    /// 지역 서브태그까지 함께 본다. `Hant`·`TW`·`HK` 서브태그가 하나라도 있으면
    /// 번체로 보고 `None` 을 돌려줘 `en` 으로 폴백시킨다(대소문자 무관).
    pub fn from_language_tag(tag: &str) -> Option<Self> {
        let mut subtags = tag.split('-');
        let lang = subtags.next().unwrap_or(tag).to_ascii_lowercase();
        match lang.as_str() {
            "en" => Some(Locale::En),
            "ko" => Some(Locale::Ko),
            "es" => Some(Locale::Es),
            "ja" => Some(Locale::Ja),
            "zh" => {
                let is_traditional = subtags.any(|sub| {
                    let sub = sub.to_ascii_lowercase();
                    sub == "hant" || sub == "tw" || sub == "hk"
                });
                if is_traditional {
                    None
                } else {
                    Some(Locale::Zh)
                }
            }
            _ => None,
        }
    }

    /// 클론이 지원하는 로케일 전체 목록. `En` 이 기본·폴백이므로 항상 첫 번째다.
    pub fn all() -> &'static [Locale] {
        &[Locale::En, Locale::Ko, Locale::Zh, Locale::Es, Locale::Ja]
    }

    /// F-02 Seek OCR(`VNRecognizeTextRequest.recognitionLanguages`) 언어 목록.
    ///
    /// ⭐ 이슈 #48 — UI 로케일이 곧 검색 언어 기준이다. ⚠️ **첫 원소가 어느
    /// 모델을 쓸지를 가른다**(`docs/dev/seek-ocr-latency-spike.md` §3 실측:
    /// `["en-US","ko-KR"]` 은 영어 단일과 결과가 동일 — 한글 0개). 그래서
    /// 로케일 언어를 **항상 첫 원소**로 두고 `en-US` 를 둘째로 붙인다.
    ///
    /// `en` 은 **빈 목록**을 돌려준다 — Vision 기본값(영어)이 곧 미국 영어
    /// 모델이라 `["en-US"]` 명시와 결과가 동일하고(실측), 기존 동작(미설정 =
    /// Vision 기본)과 정확히 같게 유지해 회귀를 막는 것이 목적이다.
    pub fn ocr_recognition_languages(self) -> &'static [&'static str] {
        match self {
            Locale::En => &[],
            Locale::Ko => &["ko-KR", "en-US"],
            Locale::Zh => &["zh-Hans", "en-US"],
            Locale::Es => &["es-ES", "en-US"],
            Locale::Ja => &["ja-JP", "en-US"],
        }
    }

    /// ⭐(이슈 #93) — `seek.searchLanguage`(검색 언어) 값 `"ko"`·`"zh"`·`"ja"`·
    /// `"es"` → [`Locale`] 로 매핑한다. `"en"`(영어 단일 명시)은 `Locale::En` 을
    /// 준다(`ocr_recognition_languages` 가 `[]` 반환). 모르는 값은 `None` — 호출자가
    /// 로케일 폴백으로 처리한다(부재 = 로케일 폴백, Plan D1·D6 §9 #1~#2).
    #[must_use]
    pub fn from_search_language(code: &str) -> Option<Locale> {
        match code {
            "en" => Some(Locale::En),
            "ko" => Some(Locale::Ko),
            "zh" => Some(Locale::Zh),
            "ja" => Some(Locale::Ja),
            "es" => Some(Locale::Es),
            _ => None,
        }
    }
}

/// en 카탈로그 원본. 컴파일 타임에 임베드된다.
const EN_JSON: &str = include_str!("../../../resources/i18n/en.json");
/// ko 카탈로그 원본. 컴파일 타임에 임베드된다.
const KO_JSON: &str = include_str!("../../../resources/i18n/ko.json");
/// zh(간체) 카탈로그 원본. 컴파일 타임에 임베드된다.
const ZH_JSON: &str = include_str!("../../../resources/i18n/zh.json");
/// es 카탈로그 원본. 컴파일 타임에 임베드된다.
const ES_JSON: &str = include_str!("../../../resources/i18n/es.json");
/// ja 카탈로그 원본. 컴파일 타임에 임베드된다.
const JA_JSON: &str = include_str!("../../../resources/i18n/ja.json");

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
        Locale::Zh => ZH_JSON,
        Locale::Es => ES_JSON,
        Locale::Ja => JA_JSON,
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

    /// 이 카탈로그의 전체 항목을 en 폴백까지 병합해 돌려준다.
    ///
    /// ⭐ 웹뷰(F-09 환경설정 창)가 문자열을 개별 커맨드로 하나씩 가져가면 왕복이 수십 번
    /// 생긴다. 카탈로그는 작고 정적이므로 한 번에 넘긴다 — 소스는 여전히 하나뿐이라
    /// §3.1.5 "단일 카탈로그" 결정을 그대로 지킨다.
    ///
    /// 자기 로케일 값이 우선하고, 자기 로케일에 없는 키만 en 값으로 채운다
    /// ([`Catalog::get`] 의 개별 폴백 규칙과 동일).
    pub fn entries(&self) -> std::collections::BTreeMap<String, String> {
        let mut merged: std::collections::BTreeMap<String, String> = self
            .fallback
            .iter()
            .flatten()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        merged.extend(self.entries.iter().map(|(k, v)| (k.clone(), v.clone())));
        merged
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

    /// 다섯 카탈로그 전부의 (코드, JSON 원본) 쌍. 테스트들이 공통으로 순회한다.
    fn all_catalog_jsons() -> [(&'static str, &'static str); 5] {
        [
            ("en", EN_JSON),
            ("ko", KO_JSON),
            ("zh", ZH_JSON),
            ("es", ES_JSON),
            ("ja", JA_JSON),
        ]
    }

    /// 문자열에 등장하는 위치 인자 `{0}`, `{1}` … 의 인덱스 집합을 뽑는다.
    /// `format()` 이 실제로 치환하는 패턴(`{숫자}`)만 본다 — 그 밖의 중괄호는
    /// 대상이 아니다.
    fn placeholder_indices(s: &str) -> BTreeSet<u32> {
        let mut indices = BTreeSet::new();
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'{' {
                if let Some(end) = s[i + 1..].find('}') {
                    let inner = &s[i + 1..i + 1 + end];
                    if !inner.is_empty() && inner.chars().all(|c| c.is_ascii_digit()) {
                        if let Ok(n) = inner.parse::<u32>() {
                            indices.insert(n);
                        }
                    }
                    i += end + 2;
                    continue;
                }
            }
            i += 1;
        }
        indices
    }

    /// ⭐ 커버리지 테스트 — §8(A) 수용 기준("경로 사이에 번역 키 누락 차이가
    /// 없다")을 자동으로 지키는 장치. en 을 기준으로 나머지 네 카탈로그의 키
    /// 집합이 완전히 같아야 한다(하위 호환을 위해 옛 이름도 남긴다).
    #[test]
    fn en_ko_key_sets_are_identical() {
        let en = key_set(&raw_value(EN_JSON));
        for (name, json) in all_catalog_jsons() {
            if name == "en" {
                continue;
            }
            let other = key_set(&raw_value(json));
            let only_in_en: Vec<_> = en.difference(&other).cloned().collect();
            let only_in_other: Vec<_> = other.difference(&en).cloned().collect();
            assert!(
                only_in_en.is_empty() && only_in_other.is_empty(),
                "en/{name} 카탈로그 키 집합이 다릅니다.\nen 에만 있음: {only_in_en:?}\n{name} 에만 있음: {only_in_other:?}"
            );
        }
    }

    /// `Locale::all()` 을 돌며 en 과 키 집합이 정확히 같은지 검증한다. 위
    /// `en_ko_key_sets_are_identical` 과 같은 불변식을 로케일 목록(`Locale::all`)
    /// 기준으로 다시 확인한다 — 목록이 늘어나도 이 테스트가 자동으로 따라간다.
    #[test]
    fn all_catalogs_have_identical_key_sets() {
        let en = key_set(&raw_value(EN_JSON));
        for locale in Locale::all() {
            if *locale == Locale::En {
                continue;
            }
            let other = key_set(&raw_value(raw_for(*locale)));
            let only_in_en: Vec<_> = en.difference(&other).cloned().collect();
            let only_in_other: Vec<_> = other.difference(&en).cloned().collect();
            assert!(
                only_in_en.is_empty() && only_in_other.is_empty(),
                "en/{} 카탈로그 키 집합이 다릅니다.\nen 에만 있음: {only_in_en:?}\n{} 에만 있음: {only_in_other:?}",
                locale.code(),
                locale.code(),
            );
        }
    }

    /// 다섯 파일 모두 유효한 flat JSON — 값이 전부 문자열이고 중첩이 없다.
    #[test]
    fn all_catalogs_are_flat_string_maps() {
        for (name, json) in all_catalog_jsons() {
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

    /// ⭐ 각 키에 대해 en 값의 `{0}`,`{1}`… 위치 인자 집합과 다른 로케일 값의
    /// 집합이 같은지 검증한다. 다르면 `format()` 이 조용히 인자 개수가 안 맞는
    /// 문장을 만들어낸다(치환 안 된 `{0}` 이 그대로 노출되거나, 인자가 버려짐).
    #[test]
    fn all_catalogs_preserve_positional_placeholders() {
        let en = raw_value(EN_JSON);
        let en_obj = en.as_object().expect("en 최상위는 객체여야 한다");

        for (name, json) in all_catalog_jsons() {
            if name == "en" {
                continue;
            }
            let value = raw_value(json);
            let obj = value
                .as_object()
                .unwrap_or_else(|| panic!("{name} 카탈로그 최상위는 객체여야 한다"));
            for (key, en_v) in en_obj {
                let Some(other_v) = obj.get(key) else {
                    // 키 누락은 다른 테스트(all_catalogs_have_identical_key_sets)가 잡는다.
                    continue;
                };
                let en_placeholders = placeholder_indices(en_v.as_str().unwrap_or_default());
                let other_placeholders = placeholder_indices(other_v.as_str().unwrap_or_default());
                assert_eq!(
                    en_placeholders, other_placeholders,
                    "{name} 카탈로그의 \"{key}\" 위치 인자 집합이 en 과 다릅니다.\nen: {en_placeholders:?}\n{name}: {other_placeholders:?}"
                );
            }
        }
    }

    /// §3.1.3: 브랜드 고유명사는 번역 대상이 아니다 — `app.name` 은 로케일과
    /// 무관하게 항상 `Ultrakey` 다. 다섯 카탈로그 전부를 확인한다.
    #[test]
    fn app_name_is_never_translated() {
        for locale in Locale::all() {
            let catalog = Catalog::for_locale(*locale);
            assert_eq!(
                catalog.get("app.name"),
                "Ultrakey",
                "{} 카탈로그의 app.name 이 번역되었다",
                locale.code()
            );
        }
    }

    /// ⭐ 이슈 #45 — 메뉴 탭 다국어화 가이드라인: 6개 탭 중 `Seek`·`Hyperkey`
    /// 둘만 **원문(영문) 유지** 대상이다(§3.1.3 브랜드·기능 고유명사 비번역).
    /// 사이드바 탭 라벨과 상세 헤딩 양쪽 전부에서, 다섯 카탈로그 전부를 확인한다
    /// (`app_name_is_never_translated` 와 같은 형태의 재발 방지 장치).
    #[test]
    fn seek과_hyperkey_탭_라벨과_헤딩은_모든_로케일에서_원문_그대로다() {
        for locale in Locale::all() {
            let catalog = Catalog::for_locale(*locale);
            for (key, original) in [
                ("settings.tab.seek", "Seek"),
                ("settings.seek.heading", "Seek"),
                ("settings.tab.hyperkey", "Hyperkey"),
                ("settings.hyperkey.heading", "Hyperkey"),
            ] {
                assert_eq!(
                    catalog.get(key),
                    original,
                    "{} 카탈로그의 {key} 가 원문({original})과 다르다 — \
                     Seek·Hyperkey 는 브랜드·기능 고유명사라 번역하지 않는다(이슈 #45)",
                    locale.code()
                );
            }
        }
    }

    /// ⭐ 이슈 #45 — 전 로케일에서 **탭 라벨 값 == 상세 헤딩 값**이다(이슈가
    /// 해소한 "사이드바는 영문인데 상세 헤딩은 한국어" 어긋남의 재발 방지).
    /// `Keyboards` 는 헤딩이 탭 라벨 키 자체를 재사용하므로(`settings.html` 의
    //  근거 주석 참고 — 명세 §4.1 키 예산) 애초에 어긋날 구조가 없다.
    /// ⚠️ `es` 의 `General` 처럼 공교롭게 영어와 철자가 같은 번역이 있으므로
    /// "비영어 로케일이면 값이 영어와 달라야 한다"는 형태의 검사는 틀리다 —
    /// 여기서 고정하는 불변식은 라벨↔헤딩 일치뿐이다.
    #[test]
    fn 탭_라벨과_상세_헤딩_값은_모든_로케일에서_같다() {
        for locale in Locale::all() {
            let value = raw_value(raw_for(*locale));
            let obj = value
                .as_object()
                .unwrap_or_else(|| panic!("{} 카탈로그 최상위는 객체여야 한다", locale.code()));
            for tab in ["presets", "korean", "general"] {
                let label = obj
                    .get(&format!("settings.tab.{tab}"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| panic!("{} 카탈로그에 settings.tab.{tab} 이 없다", locale.code()));
                let heading = obj
                    .get(&format!("settings.{tab}.heading"))
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| {
                        panic!("{} 카탈로그에 settings.{tab}.heading 이 없다", locale.code())
                    });
                assert_eq!(
                    label, heading,
                    "{} 카탈로그의 탭 라벨({label:?})과 상세 헤딩({heading:?})이 다르다 — \
                     이슈 #45 로 전 탭의 라벨↔헤딩을 일치시켰다",
                    locale.code()
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
        assert_eq!(Locale::from_language_tag("fr"), None);
    }

    /// 지역 변형 태그가 언어 서브태그만으로 올바르게 폴백되는지 확인한다.
    #[test]
    fn from_language_tag_maps_regional_variants() {
        assert_eq!(Locale::from_language_tag("ko-KR"), Some(Locale::Ko));
        assert_eq!(Locale::from_language_tag("zh-Hans-CN"), Some(Locale::Zh));
        assert_eq!(Locale::from_language_tag("es-419"), Some(Locale::Es));
        assert_eq!(Locale::from_language_tag("ja-JP"), Some(Locale::Ja));
        assert_eq!(Locale::from_language_tag("en-GB"), Some(Locale::En));
    }

    /// ⭐ 이 테스트가 §3.1.1 의 결정("번체 중국어를 간체로 접지 않는다")의
    /// 근거를 코드에 붙들어 둔다. `zh-Hant`·`zh-TW`·`zh-HK` 는 전부 `None` 을
    /// 돌려줘 `en` 으로 폴백해야 한다 — 대소문자·서브태그 순서와 무관하게.
    #[test]
    fn traditional_chinese_falls_back_to_english() {
        assert_eq!(Locale::from_language_tag("zh-Hant"), None);
        assert_eq!(Locale::from_language_tag("zh-TW"), None);
        assert_eq!(Locale::from_language_tag("zh-HK"), None);
        assert_eq!(Locale::from_language_tag("ZH-Hant-TW"), None);

        // 대조군: 간체·중립 zh 태그는 정상적으로 Zh 로 매핑된다.
        assert_eq!(Locale::from_language_tag("zh"), Some(Locale::Zh));
        assert_eq!(Locale::from_language_tag("zh-Hans"), Some(Locale::Zh));
        assert_eq!(Locale::from_language_tag("zh-CN"), Some(Locale::Zh));
        assert_eq!(Locale::from_language_tag("zh-SG"), Some(Locale::Zh));
    }

    /// endonym 다섯 개가 서로 다르고 비어 있지 않은지 확인한다.
    #[test]
    fn endonyms_are_distinct_and_non_empty() {
        let names: Vec<&str> = Locale::all().iter().map(|l| l.endonym()).collect();
        for name in &names {
            assert!(!name.is_empty(), "endonym 이 비어 있다");
        }
        let unique: BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(
            unique.len(),
            names.len(),
            "endonym 이 서로 겹친다: {names:?}"
        );
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

    /// 지원하지 않는 언어만 나열되면 `en` 으로 폴백한다.
    #[test]
    fn resolve_falls_back_to_english_for_unsupported_language() {
        let tags = vec!["de-DE".to_string(), "fr".to_string()];
        let catalog = Catalog::resolve(&tags);
        assert_eq!(catalog.locale(), Locale::En);
    }

    /// 뒤가 아니라 **앞에서부터** 처음 매칭되는 태그를 채택해야 한다.
    #[test]
    fn resolve_picks_first_supported_language_in_order() {
        let tags = vec!["de".to_string(), "ja".to_string(), "ko".to_string()];
        let catalog = Catalog::resolve(&tags);
        assert_eq!(catalog.locale(), Locale::Ja);
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

    /// ko 카탈로그의 `entries()` 가 en 과 키 집합이 같고, 값 중 하나 이상이
    /// 실제로 한국어여야 한다 — en 폴백이 조용히 전체를 덮어써 버리는 버그를
    /// 잡아낸다.
    #[test]
    fn entries_merges_en_fallback_and_keeps_own_locale_keys() {
        let en = Catalog::for_locale(Locale::En);
        let ko = Catalog::for_locale(Locale::Ko);

        let en_entries = en.entries();
        let ko_entries = ko.entries();

        let en_keys: BTreeSet<_> = en_entries.keys().cloned().collect();
        let ko_keys: BTreeSet<_> = ko_entries.keys().cloned().collect();
        assert_eq!(en_keys, ko_keys, "entries() 의 키 집합이 en/ko 사이에 달라서는 안 된다");

        assert_eq!(ko_entries.get("common.ok").map(String::as_str), Some("확인"));

        let has_korean_value = ko_entries
            .values()
            .any(|v| v.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c)));
        assert!(has_korean_value, "ko entries() 에 한국어 값이 하나도 없다");
    }

    /// ⭐ 이슈 #48 — `Locale::Ko` 의 OCR 언어 목록은 `["ko-KR", "en-US"]`
    /// **순서 그대로**여야 한다. 실측 제약(`docs/dev/seek-ocr-latency-spike.md`
    /// §3): `["en-US","ko-KR"]` 은 영어 모델을 골라 한글 인식이 영어 단일과
    /// 결과가 동일해진다(한글 0개) — ko-KR 이 반드시 첫 원소다.
    #[test]
    fn ocr_recognition_languages_ko_keeps_ko_kr_first() {
        assert_eq!(
            Locale::Ko.ocr_recognition_languages(),
            &["ko-KR", "en-US"],
            "ko 의 recognitionLanguages 순서가 어긋났다 — ko-KR 이 첫 원소여야 한글 인식이 된다(이슈 #48 실측 제약)"
        );
    }

    /// ⭐ 이슈 #48 — ko 외 로케일도 같은 규칙: 로케일 언어가 **항상 첫 원소**다.
    /// (`zh-Hans` 간체 · `es-ES` · `ja-JP` — 모두 en 폴백과 짝을 이룬다.)
    #[test]
    fn ocr_recognition_languages_other_locales_put_locale_first() {
        assert_eq!(
            Locale::Zh.ocr_recognition_languages(),
            &["zh-Hans", "en-US"],
            "zh 의 recognitionLanguages 가 어긋났다"
        );
        assert_eq!(
            Locale::Es.ocr_recognition_languages(),
            &["es-ES", "en-US"],
            "es 의 recognitionLanguages 가 어긋났다"
        );
        assert_eq!(
            Locale::Ja.ocr_recognition_languages(),
            &["ja-JP", "en-US"],
            "ja 의 recognitionLanguages 가 어긋났다"
        );
    }

    /// ⭐ 이슈 #48(회귀 방지) — `En` 은 **빈 목록**을 돌려줘야 한다. Vision
    /// 기본값(영어)이 곧 미국 영어 모델이라 `["en-US"]` 명시와 결과가 동일하고
    /// (실측), 미설정 = Vision 기본 동작을 그대로 유지한다.
    #[test]
    fn en_ocr_recognition_languages_is_empty() {
        assert!(
            Locale::En.ocr_recognition_languages().is_empty(),
            "en 은 빈 목록이어야 한다 — Vision 기본값(영어)을 그대로 쓰는 기존 동작을 유지한다(이슈 #48)"
        );
    }

    /// ⭐(이슈 #93) — `seek.searchLanguage` 값 → `Locale`. 모르는 값은 `None`.
    #[test]
    fn from_search_language_maps_known_codes_and_ignores_unknown() {
        assert_eq!(Locale::from_search_language("en"), Some(Locale::En));
        assert_eq!(Locale::from_search_language("ko"), Some(Locale::Ko));
        assert_eq!(Locale::from_search_language("zh"), Some(Locale::Zh));
        assert_eq!(Locale::from_search_language("ja"), Some(Locale::Ja));
        assert_eq!(Locale::from_search_language("es"), Some(Locale::Es));
        assert_eq!(Locale::from_search_language("fr"), None);
        assert_eq!(Locale::from_search_language(""), None);
    }

    /// ⭐ 이슈 #48 — 영어 폴백 `en-US` 가 **정확히 2번째 원소**로 들어 있다
    /// (`En` 제외 4개 로케일 전부). 첫 원소가 로케일 언어인 것과 함께 이 목록의
    /// 고정된 규약이다.
    #[test]
    fn en_us_is_second_in_every_non_english_locale() {
        let non_english = [Locale::Ko, Locale::Zh, Locale::Es, Locale::Ja];
        for locale in non_english {
            let langs = locale.ocr_recognition_languages();
            assert_eq!(
                langs.len(),
                2,
                "{} 의 recognitionLanguages 길이가 2 가 아니다: {langs:?}",
                locale.code()
            );
            assert_eq!(
                langs.get(1),
                Some(&"en-US"),
                "{} 의 recognitionLanguages 에 en-US 가 정확히 2번째 원소로 없다: {langs:?}",
                locale.code()
            );
        }
    }

    /// ⭐ 이슈 #48 — 각 로케일의 OCR 언어 목록 원소는 서로 **중복이 없어야**
    /// 한다. 같은 언어가 두 번 들어가면 Vision 이 같은 모델을 중복 요청해
    /// 지연만 늘어난다(스파이크 §3 의 대가와 직결).
    #[test]
    fn ocr_recognition_languages_have_no_duplicates() {
        for locale in Locale::all() {
            let langs = locale.ocr_recognition_languages();
            let unique: BTreeSet<_> = langs.iter().copied().collect();
            assert_eq!(
                unique.len(),
                langs.len(),
                "{} 의 recognitionLanguages 목록에 중복이 있다: {langs:?}",
                locale.code()
            );
        }
    }
}
