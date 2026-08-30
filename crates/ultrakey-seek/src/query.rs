//! §3.6 — 질의 매칭 규칙.
//!
//! 명세가 이 절 전체를 `(추정)`/`❓미확인`(Q-d) 으로 남겼다. 이 구현이 고른
//! 값과 근거는 [`QueryParams`] 문서에 있다.

use crate::candidate::TextCandidate;

/// 질의 매칭 파라미터.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct QueryParams {
    /// 빈 질의일 때 후보를 전부 보여줄 것인가.
    ///
    /// **`false`(후보 0개)를 기본값으로 골랐다** — `Default` 파생이 주는 값과
    /// 같으므로 별도 `impl Default` 를 두지 않는다. 명세가 두 동작 모두 가능하다고 남긴
    /// 자리인데(§3.6), 전체 노출을 고르면 세션을 여는 순간 화면의 모든 텍스트에
    /// 하이라이트가 깔린다 — Spotlight 류 검색 UI 가 빈 질의에서 전체 결과를
    /// 쏟지 않는 것과 같은 이유다. F-03 오버레이 부담도 그만큼 커진다.
    pub empty_query_matches_all: bool,
}

/// 비교용 정규화 — 소문자화, 앞뒤 공백 트림, 연속 공백을 단일 공백으로.
///
/// ⭐ 대소문자 무시는 `to_lowercase`(유니코드 전체 매핑)로 한다. `to_ascii_
/// lowercase` 는 비ASCII 텍스트에서 아무 일도 하지 않아 독일어·그리스어 화면
/// 에서 매칭이 조용히 실패한다.
#[must_use]
pub fn normalize_for_match(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for ch in s.trim().chars() {
        if ch.is_whitespace() {
            pending_space = true;
            continue;
        }
        if pending_space && !out.is_empty() {
            out.push(' ');
        }
        pending_space = false;
        for lower in ch.to_lowercase() {
            out.push(lower);
        }
    }
    out
}

/// 병합된 후보 집합에 질의를 적용한다 (§3.6).
///
/// 매치 방식은 **부분 문자열 포함**이다 — 접두사 매치만이면 버튼 가운데 단어를
/// 찾을 수 없어 UX 가 부자연스럽다는 명세의 판단을 따른다.
#[must_use]
pub fn filter_by_query(
    candidates: &[TextCandidate],
    query: &str,
    params: QueryParams,
) -> Vec<TextCandidate> {
    let needle = normalize_for_match(query);
    if needle.is_empty() {
        return if params.empty_query_matches_all {
            candidates.to_vec()
        } else {
            Vec::new()
        };
    }
    candidates
        .iter()
        .filter(|c| normalize_for_match(&c.text).contains(&needle))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::Rect;

    fn candidate(text: &str) -> TextCandidate {
        TextCandidate::ocr(
            text.to_string(),
            Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
            0.9,
            1,
        )
    }

    /// §3.6 공백 처리 — 앞뒤 트림, 연속 공백은 단일 공백으로, 대소문자 무시.
    #[test]
    fn normalize_trims_collapses_and_lowercases() {
        assert_eq!(normalize_for_match("  Hello    World  "), "hello world");
    }

    /// ⭐ 비ASCII — 독일어 움라우트 대문자에서 소문자화가 실제로 동작하는가.
    /// `to_ascii_lowercase` 였다면 비ASCII 문자는 그대로 남아 매칭이 조용히
    /// 실패한다 — 아래에서 그 차이를 직접 대조한다.
    #[test]
    fn normalize_lowercases_non_ascii_german() {
        assert_eq!(normalize_for_match("ÄÖÜ"), "äöü");
        // to_ascii_lowercase 는 ASCII 범위 밖 문자를 건드리지 않는다는 것을
        // 대조로 확인 — 이게 바로 avoid 해야 할 실패 케이스다.
        assert_eq!("ÄÖÜ".to_ascii_lowercase(), "ÄÖÜ");
    }

    /// ⭐ 비ASCII — 그리스어 대문자에서도 소문자화가 동작해야 한다.
    #[test]
    fn normalize_lowercases_non_ascii_greek() {
        assert_eq!(normalize_for_match("ΑΒΓΔ"), "αβγδ");
        assert_eq!("ΑΒΓΔ".to_ascii_lowercase(), "ΑΒΓΔ");
    }

    /// 한글은 대소문자 구분이 없으므로 트림·공백 정규화만 확인한다.
    #[test]
    fn normalize_handles_korean_whitespace() {
        assert_eq!(normalize_for_match("  안녕   하세요  "), "안녕 하세요");
    }

    /// §3.6 매치 방식 — 부분 문자열 매치. 버튼 텍스트 가운데 단어로도 찾아져야 한다.
    #[test]
    fn filter_matches_middle_word_substring() {
        let candidates = vec![candidate("Open Settings Panel")];
        let matched = filter_by_query(&candidates, "Settings", QueryParams::default());
        assert_eq!(matched.len(), 1);
    }

    /// §3.6 빈 질의 — 기본값(`empty_query_matches_all: false`)에서는 0개.
    #[test]
    fn empty_query_matches_none_by_default() {
        let candidates = vec![candidate("Anything")];
        let matched = filter_by_query(&candidates, "", QueryParams::default());
        assert!(matched.is_empty());
    }

    /// §3.6 빈 질의 — `empty_query_matches_all: true` 면 전부 반환한다.
    #[test]
    fn empty_query_matches_all_when_configured() {
        let candidates = vec![candidate("A"), candidate("B")];
        let params = QueryParams { empty_query_matches_all: true };
        let matched = filter_by_query(&candidates, "", params);
        assert_eq!(matched.len(), 2);
    }

    /// 공백만 있는 질의도 빈 질의로 취급되어야 한다(트림 후 빈 문자열이므로).
    #[test]
    fn whitespace_only_query_treated_as_empty() {
        let candidates = vec![candidate("Anything")];
        let matched = filter_by_query(&candidates, "   ", QueryParams::default());
        assert!(matched.is_empty());

        let params = QueryParams { empty_query_matches_all: true };
        let matched_all = filter_by_query(&candidates, "   ", params);
        assert_eq!(matched_all.len(), 1);
    }
}
