//! §3.4 — 병합 계층.
//!
//! 두 소스의 목록을 하나의 후보 집합으로 합친다. 전제: 두 목록 모두 이미
//! 같은 좌표계(전역·포인트·좌상단 원점)로 정규화되어 있다.
//!
//! **M3 우선순위는 확정 사실이다** — ⓘ 팝오버 원문 "Seek matches using Optical
//! Character Recognition (OCR) will display in place of duplicate matches from
//! Accessibility." 즉 겹치는 쌍에서 **OCR 이 AX 를 대체**한다.

use crate::candidate::{CandidateSource, TextCandidate};

/// 중복 판정 임계값 (§3.4 M2). 명세가 `(추정)` 으로 남긴 자리를 이 구현이
/// 채운 값이며, 근거는 각 필드 문서에 있다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MergeParams {
    /// 두 프레임을 같은 실체로 볼 IoU 하한.
    ///
    /// **0.30 을 골랐다.** OCR 프레임은 글자 덩어리에 딱 맞고 AX 프레임은 요소
    /// 전체라 같은 라벨이라도 IoU 가 0.5 를 넘기 어렵다. 반대로 0.1 까지
    /// 낮추면 한 버튼 안의 서로 다른 두 줄이 같은 실체로 묶인다.
    pub iou_threshold: f64,
    /// OCR 프레임이 AX 프레임 안에 얼마나 들어가야 "그 요소의 텍스트"로 볼지.
    ///
    /// **0.80 을 골랐다.** AX 요소가 OCR 덩어리를 대부분 감싸면 같은 실체로
    /// 본다 — IoU 만으로는 넓은 AX 프레임과의 중복을 놓치기 때문이다.
    pub containment_threshold: f64,
}

impl Default for MergeParams {
    fn default() -> Self {
        Self {
            iou_threshold: 0.30,
            containment_threshold: 0.80,
        }
    }
}

/// 두 목록을 병합한다 (§3.4 M1 → M2 → M3 → 정렬).
///
/// - M1: 합집합
/// - M2: 프레임이 겹치고(IoU 또는 포함 관계) 텍스트가 대소문자 무시 후
///   동일하거나 포함 관계이면 같은 실체
/// - M3: 같은 실체이면 **OCR 이 AX 를 대체**한다
/// - 정렬: 읽기 순서(y 오름차순, 동률이면 x 오름차순)
#[must_use]
pub fn merge_candidates(
    ocr: Vec<TextCandidate>,
    ax: Vec<TextCandidate>,
    params: MergeParams,
) -> Vec<TextCandidate> {
    // M3 이 "OCR 이 AX 를 대체" 이므로, OCR 목록은 전부 남기고 AX 목록에서
    // OCR 과 겹치는 것만 떨어뜨리면 된다. AX 끼리의 중복은 제거하지 않는다 —
    // 같은 창의 서로 다른 요소가 같은 텍스트를 가질 수 있고(예: 표의 반복
    // 셀), 그것들은 클릭 대상이 실제로 서로 다르기 때문이다.
    let mut out = ocr;
    for ax_candidate in ax {
        let duplicated = out
            .iter()
            .filter(|c| c.source == CandidateSource::Ocr)
            .any(|ocr_candidate| is_same_entity(ocr_candidate, &ax_candidate, params));
        if !duplicated {
            out.push(ax_candidate);
        }
    }
    sort_reading_order(&mut out);
    out
}

/// M2 — 두 후보가 같은 실체인가.
#[must_use]
pub fn is_same_entity(a: &TextCandidate, b: &TextCandidate, params: MergeParams) -> bool {
    if !frames_overlap(a, b, params) {
        return false;
    }
    texts_related(&a.text, &b.text)
}

fn frames_overlap(a: &TextCandidate, b: &TextCandidate, params: MergeParams) -> bool {
    if a.frame.iou(&b.frame) >= params.iou_threshold {
        return true;
    }
    // 포함 관계는 양방향으로 본다 — 어느 쪽이 더 넓은 프레임인지 미리 알 수 없다.
    a.frame.containment_in(&b.frame) >= params.containment_threshold
        || b.frame.containment_in(&a.frame) >= params.containment_threshold
}

fn texts_related(a: &str, b: &str) -> bool {
    let a = crate::query::normalize_for_match(a);
    let b = crate::query::normalize_for_match(b);
    if a.is_empty() || b.is_empty() {
        return false;
    }
    a == b || a.contains(&b) || b.contains(&a)
}

/// 읽기 순서 정렬 (§3.4). y 오름차순, 동률이면 x 오름차순.
///
/// 명세는 이 기준을 `(추정)`(Q-c) 으로 남겼다 — 커서 거리 기준일 가능성도
/// 배제하지 못했다. 읽기 순서를 고른 근거는 그것이 후보가 몇 개든 **결정적
/// (deterministic)** 이고 화면을 보는 사람의 기대와 어긋나지 않기 때문이다.
/// 커서 거리 기준은 세션 중 마우스가 움직이면 순서가 바뀐다.
pub fn sort_reading_order(candidates: &mut [TextCandidate]) {
    candidates.sort_by(|a, b| {
        a.frame
            .y
            .partial_cmp(&b.frame.y)
            .unwrap_or(core::cmp::Ordering::Equal)
            .then_with(|| {
                a.frame
                    .x
                    .partial_cmp(&b.frame.x)
                    .unwrap_or(core::cmp::Ordering::Equal)
            })
            // 좌표까지 같으면 텍스트로 완전 순서를 만든다 — 정렬이 실행마다
            // 흔들리면 F-01/F-03 의 후보 순환이 재현 불가능해진다.
            .then_with(|| a.text.cmp(&b.text))
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transform::Rect;

    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, width: w, height: h }
    }

    /// §3.4 M3 — OCR 과 AX 가 같은 위치·같은 텍스트일 때 OCR 이 남고 AX 가
    /// 사라진다. 반대가 아니다 — 기존 명세 추정("AX 프레임을 채택")이
    /// 정반대로 틀렸던 지점이라 이 방향을 명시적으로 고정한다.
    #[test]
    fn m3_ocr_replaces_ax_on_duplicate() {
        let ocr = vec![TextCandidate::ocr(
            "Save".into(),
            rect(10.0, 10.0, 40.0, 12.0),
            0.9,
            1,
        )];
        let ax = vec![TextCandidate::accessibility(
            "Save".into(),
            rect(10.0, 10.0, 40.0, 12.0),
        )];
        let merged = merge_candidates(ocr, ax, MergeParams::default());
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].source, CandidateSource::Ocr);
    }

    /// §3.4 — AX 프레임이 OCR 프레임보다 훨씬 넓은 현실적인 경우(버튼 요소
    /// 전체 vs 그 안의 글자 덩어리). IoU 는 임계값 아래라 못 잡지만
    /// containment 로는 중복 판정돼야 한다.
    #[test]
    fn wide_ax_frame_deduped_via_containment_not_iou() {
        let ax_frame = rect(0.0, 0.0, 300.0, 50.0); // 버튼 요소 전체
        let ocr_frame = rect(100.0, 10.0, 50.0, 20.0); // 그 안의 글자 덩어리, ax_frame 에 완전히 포함

        let params = MergeParams::default();
        assert!(
            ocr_frame.iou(&ax_frame) < params.iou_threshold,
            "이 테스트의 취지상 IoU 는 임계값 아래여야 한다"
        );
        assert!(
            ocr_frame.containment_in(&ax_frame) >= params.containment_threshold,
            "containment 는 임계값 이상이어야 한다"
        );

        let ocr = vec![TextCandidate::ocr("Submit".into(), ocr_frame, 0.9, 1)];
        let ax = vec![TextCandidate::accessibility("Submit".into(), ax_frame)];
        let merged = merge_candidates(ocr, ax, params);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].source, CandidateSource::Ocr);
    }

    /// §8 "AX 전용 매치" — OCR 과 겹치지 않는 AX 매치는 그대로 살아남는다.
    #[test]
    fn ax_only_match_survives_when_not_overlapping() {
        let ocr = vec![TextCandidate::ocr(
            "Save".into(),
            rect(0.0, 0.0, 40.0, 12.0),
            0.9,
            1,
        )];
        let ax = vec![TextCandidate::accessibility(
            "Cancel".into(),
            rect(500.0, 500.0, 40.0, 12.0),
        )];
        let merged = merge_candidates(ocr, ax, MergeParams::default());
        assert_eq!(merged.len(), 2);
        assert!(merged
            .iter()
            .any(|c| c.source == CandidateSource::Accessibility && c.text == "Cancel"));
    }

    /// 텍스트가 다르면 위치가 겹쳐도 병합하지 않는다.
    #[test]
    fn overlapping_but_different_text_does_not_merge() {
        let ocr = vec![TextCandidate::ocr(
            "Save".into(),
            rect(0.0, 0.0, 40.0, 12.0),
            0.9,
            1,
        )];
        let ax = vec![TextCandidate::accessibility(
            "Delete".into(),
            rect(0.0, 0.0, 40.0, 12.0),
        )];
        let merged = merge_candidates(ocr, ax, MergeParams::default());
        assert_eq!(merged.len(), 2);
    }

    /// 대소문자만 다른 텍스트는 같은 실체로 본다.
    #[test]
    fn case_only_difference_is_same_entity() {
        let ocr = vec![TextCandidate::ocr(
            "SUBMIT".into(),
            rect(0.0, 0.0, 40.0, 12.0),
            0.9,
            1,
        )];
        let ax = vec![TextCandidate::accessibility(
            "submit".into(),
            rect(0.0, 0.0, 40.0, 12.0),
        )];
        let merged = merge_candidates(ocr, ax, MergeParams::default());
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].source, CandidateSource::Ocr);
    }

    /// §3.4 정렬 — 읽기 순서(y 오름차순, 동률이면 x 오름차순, 그래도 동률이면
    /// 텍스트)이며, 같은 입력에는 항상 같은 순서가 나와야 한다(결정성).
    #[test]
    fn sort_reading_order_is_deterministic() {
        let mut candidates = vec![
            TextCandidate::ocr("B".into(), rect(10.0, 20.0, 5.0, 5.0), 0.9, 1),
            TextCandidate::ocr("A".into(), rect(0.0, 20.0, 5.0, 5.0), 0.9, 1),
            TextCandidate::ocr("Z".into(), rect(0.0, 0.0, 5.0, 5.0), 0.9, 1),
            // y, x 모두 동률 — 텍스트로 갈린다.
            TextCandidate::ocr("D".into(), rect(10.0, 20.0, 5.0, 5.0), 0.9, 1),
            TextCandidate::ocr("C".into(), rect(10.0, 20.0, 5.0, 5.0), 0.9, 1),
        ];
        sort_reading_order(&mut candidates);
        let order: Vec<&str> = candidates.iter().map(|c| c.text.as_str()).collect();
        assert_eq!(order, vec!["Z", "A", "B", "C", "D"]);

        // 결정성 — 같은 입력을 다시 정렬해도 같은 순서가 나와야 한다.
        let mut candidates2 = candidates.clone();
        sort_reading_order(&mut candidates2);
        assert_eq!(candidates, candidates2);
    }

    /// 빈 목록 병합 — 둘 다 비었을 때, 한쪽만 비었을 때.
    #[test]
    fn merge_empty_lists() {
        assert!(merge_candidates(vec![], vec![], MergeParams::default()).is_empty());

        let ocr_only = vec![TextCandidate::ocr(
            "Only".into(),
            rect(0.0, 0.0, 10.0, 10.0),
            0.9,
            1,
        )];
        let merged = merge_candidates(ocr_only.clone(), vec![], MergeParams::default());
        assert_eq!(merged, ocr_only);

        let ax_only = vec![TextCandidate::accessibility(
            "Only".into(),
            rect(0.0, 0.0, 10.0, 10.0),
        )];
        let merged = merge_candidates(vec![], ax_only.clone(), MergeParams::default());
        assert_eq!(merged, ax_only);
    }
}
