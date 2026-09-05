//! ⭐(이슈 #134) 세션 간 검출 캐시 — 재진입 시 캐시 우선 탐색(D13)의 순수 모델.
//!
//! 이슈 본문의 사용자 피드백("기존에 있던 데이터에서 우선 탐색 → 신규 OCR 이
//! 결과를 만들어내면 가장 최신화된 데이터로 다시 한번 탐색")을 구현하는 데
//! 필요한 **세션 밖** 자료형과 순수 정책 함수를 둔다. 상태 머신
//! ([`machine::SeekSessionMachine`]) 은 이 모듈의 모델을 주입·스냅샷하는 두
//! 진입점(`inject_cached`/`all_candidates`)을 더할 뿐, 검출 자체와 기존 주입
//! 경로는 **한 줄도 바꾸지 않는다**(결정 4 — `detect.rs` 무변경, "캐시 최소
//! 증분" 제약).
//!
//! ⭐ 설계 결정(이슈 #134 P1 설계 확정 코멘트의 결정 1~3) — 이 모듈이 왜 이
//! 생김새인가:
//!
//! - **세션 밖 캐시, 메모리 한정, 디스크 금지**(결정 1, 제품 제약 AGENTS.md
//!   §4). 캐시는 워커(`SeekController`) 필드의 `Option<SeekDetectionCache>` 로
//!   산다 — 세션 스코프인 `OverlaySession` 과 달리 세션 종료에도 살아남는다.
//!   "처리한 화면 데이터"의 일부이므로 **디스크에 절대 쓰지 않는다**.
//!   무한히 크지 않다 — **단일 슬롯** 하나만 들고, 내용은 직전 세션의
//!   후보뿐이다.
//! - **단일 슬롯 "직전 세션의 것만", TTL 없음**(결정 2). 시간 만료·캡처 해시
//!   비교가 없다. stale 노출 창은 **유계(bounded)** 다 — 재진입할 때마다
//!   배경 신규 OCR(영어 ≈0.8s, 다국어 ≈3.1s, `docs/dev/seek-ocr-latency-spike.md`
//!   §3)이 항상 돌아 캐시 후보를 통째로 교체하기 때문이다.
//! - **세대 라우팅: 폐기 유지 — 캐시 갱신으로 라우팅하지 않는다**(결정 3).
//!   늦게 끝난 이전 세대 OCR(`Candidates`/`ExtraCandidates`/`DetectionFinished`)
//!   은 현행 그대로 조용히 폐기한다. [`is_current_generation`] 이 그 정책의
//!   **순수 표현**이다. ⚠️ **동작 중 호출처는 없다** — 판정 4 의 "주입
//!   경로·세대 가드 불변"에 따라 워커(`seek.rs`)는 이 함수를 부르지 않고
//!   인라인 `generation != controller.generation` 검사를 그대로 유지한다.
//!   함수의 존재 이유는 그 비교를 **정책으로 승격해 테스트로 고정**하기
//!   위함이다 — 이 한 줄이 "워커 가드가 하는 비교와 같은 것"이라는 계약을
//!   문서화하고, 단위 테스트가 양쪽(같음 → 수용, 다름 → 폐기)을 잠근다.
//! - **캐시는 현재 세대 `DetectionFinished`(세션이 살아 있을 때)에만
//!   갱신된다**(결정 1). 워커는 완료 시점에
//!   [`machine::SeekSessionMachine::all_candidates`] 로 "사용자가 본 그 후보
//!   우주" 를 스냅샷해 이 타입으로 감싼다. 세션이 먼저 닫혀 스냅샷이
//!   불가능하면 갱신하지 않는다(기존 값 유지) — 취소된 세션의 늦은 결과로
//!   캐시를 더럽히지 않는다.
//!
//! 캐시 히트 판정은 [`should_inject`], 세대 일치 판정은
//! [`is_current_generation`] 이며 둘 다 입력만 받는 순수 함수라 단위 테스트가
//! 쉽다.

use ultrakey_seek::TextCandidate;

/// 세션 밖 검출 캐시 — 마지막으로 **완료**된 검출 세션의 후보 전체.
///
/// 후보별 `text`·`frame`·`source`·`confidence`·`display_id`·`window_id`·`pid`
/// 가 모두 담긴 [`TextCandidate`] 그대로를 보관한다(결정 1). 내용은 **질의로
/// 필터된 목록이 아니라 후보 우주 전체** 여야 한다 — 필터된 목록(overlay 의
/// `matching`)을 캐시에 담으면 쿼리 밖 후보가 영영 유실되어 재진입 첫
/// 탐색의 질을 떨어뜨린다.
///
/// ⚠️ 메모리 한정. **디스크에 쓰지 않는다**(제품 제약 AGENTS.md §4) — 이
/// 타입은 실행 중 메모리에만 존재하는 단일 슬롯이다.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SeekDetectionCache {
    /// 직전 완료 세션의 후보 전체. 순서는 의미가 없다(주입 시 `display_id`
    /// 로 다시 나뉘고, 오버레이가 읽기 순서로 다시 정렬한다) — 그래도
    /// 결정성 유지를 위해 스냅샷한 순서 그대로 둔다.
    pub candidates: Vec<TextCandidate>,
}

impl SeekDetectionCache {
    /// 후보 목록으로 캐시를 만든다.
    #[must_use]
    pub fn new(candidates: Vec<TextCandidate>) -> Self {
        Self { candidates }
    }

    /// 캐시가 비어 있는가(후보 0개).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.candidates.is_empty()
    }
}

/// 재진입 시 캐시를 주입할지 판정한다(결정 1·2) — **캐시 히트 = 비어 있지
/// 않은 `Some`** 이다. `None`(아직 캐시가 없음 — 첫 실행)과 빈 캐시(직전
/// 세션 후보 0개)는 같은 미스다.
///
/// `Option::is_some_and` 한 줄이 전부다 — 미스의 양쪽을 한 표현으로 묶는다.
#[must_use]
pub fn should_inject(cache: Option<&SeekDetectionCache>) -> bool {
    cache.is_some_and(|c| !c.is_empty())
}

/// 세대 일치 판정 — 이 신호가 **현재 세대**에서 온 것인가(결정 3).
///
/// 늦게 끝난 이전 세대 OCR 의 결과는 캐시 갱신으로 라우팅하지 않고
/// **폐기**한다. 이 함수는 그 폐기 정책의 **순수 표현**이자, 그 표현을
/// 단위 테스트로 고정하는 **테스트 앵커**다.
///
/// ⚠️ 동작 중 호출처는 없다 — 판정 4 의 "주입 경로·세대 가드 불변"에 따라
/// 워커(`seek.rs`)는 이 함수를 부르지 않고 인라인
/// `generation != controller.generation` 검사를 그대로 유지한다. 이 함수는
/// 그 인라인 가드가 구현하는 비교를 **정책으로 승격해 문서화**하며,
/// 테스트가 양쪽(같음 → 수용, 다름 → 폐기)을 잠그는 자리다.
///
/// ⭐ 이 값이 거짓인 경우는 정상적으로는 "이전 세대(늦게 끝난 OCR)" 의 길
/// 하나뿐이지만, 비교 한 줄이라 시계 역전 같은 이상 상태(신호가 미래 세대)
/// 도 같은 표현으로 걸러진다 — 더 좁은 부등식(`<`)을 쓰지 않는 이유다.
#[must_use]
pub fn is_current_generation(signal_generation: u64, current_generation: u64) -> bool {
    signal_generation == current_generation
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultrakey_seek::Rect;

    fn candidate(text: &str) -> TextCandidate {
        TextCandidate::ocr(
            text.to_string(),
            Rect { x: 0.0, y: 0.0, width: 10.0, height: 10.0 },
            0.9,
            1,
        )
    }

    // ── should_inject — 캐시 히트/미스 판정(결정 1·2) ──────────────────────

    /// 히트: 비어 있지 않은 `Some` → 주입한다.
    #[test]
    fn should_inject_hit_when_some_and_non_empty() {
        let cache = SeekDetectionCache::new(vec![candidate("Settings")]);
        assert!(should_inject(Some(&cache)));
    }

    /// 미스 ①: `None` — 아직 캐시가 없다(첫 실행). 주입하지 않는다.
    #[test]
    fn should_inject_miss_when_none() {
        assert!(!should_inject(None));
    }

    /// 미스 ②: 빈 캐시 — 직전 세션 후보 0개(예: 아무것도 매치되지 않은
    /// 화면). 비어 있는 것을 주입할 이유가 없으므로 주입하지 않는다.
    #[test]
    fn should_inject_miss_when_empty() {
        let cache = SeekDetectionCache::new(Vec::new());
        assert!(!should_inject(Some(&cache)));
    }

    // ── is_current_generation — 세대 폐기 라우팅(결정 3) ───────────────────

    /// 같은 세대 — 현재 세션의 OCR 결과는 받아들인다.
    #[test]
    fn same_generation_is_current() {
        assert!(is_current_generation(7, 7));
    }

    /// 이전 세대(늦게 끝난 OCR) — 폐기 대상이다. 신호 세대가 현재보다 낮은
    /// 경우가 정상 경로이고, 높은 경우(이상 상태)도 같은 한 줄 비교로
    /// 걸러진다.
    #[test]
    fn stale_signal_generation_is_not_current() {
        assert!(!is_current_generation(6, 7));
        assert!(!is_current_generation(8, 7));
    }

    // ── SeekDetectionCache 기본 동작 ───────────────────────────────────────

    /// `new` 는 주어진 목록을 그대로 담는다.
    #[test]
    fn new_holds_candidates_as_given() {
        let cache = SeekDetectionCache::new(vec![candidate("A")]);
        assert_eq!(cache.candidates.len(), 1);
        assert_eq!(cache.candidates[0].text, "A");
    }

    /// `Default`(비어 있음)와 빈 목록 `new` 는 `is_empty` 이고, 후보가 있으면
    /// 그렇지 않다.
    #[test]
    fn is_empty_reflects_candidate_count() {
        assert!(SeekDetectionCache::default().is_empty());
        assert!(SeekDetectionCache::new(Vec::new()).is_empty());
        assert!(!SeekDetectionCache::new(vec![candidate("A")]).is_empty());
    }

    /// 캐시 스냅샷과 주입 사이에 후보가 손실되지 않는가는 동등성으로 고정된다
    /// — `Clone` + `PartialEq` 가 곧 "순수 후보 보관소" 계약의 테스트다.
    #[test]
    fn cache_is_clone_and_equality_comparable() {
        let a = SeekDetectionCache::new(vec![candidate("A"), candidate("B")]);
        let b = a.clone();
        assert_eq!(a, b);
    }
}