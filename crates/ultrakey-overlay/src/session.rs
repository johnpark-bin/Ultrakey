//! F-03 Seek 오버레이 세션 — S-6(증분 검출 수신)을 다루는 상태 기계.
//!
//! ⭐ 이 크레이트의 심장이다. `OverlaySession` 은 F-02 가 디스플레이마다,
//! 그리고 AX 처럼 디스플레이가 특정되지 않는 소스마다 **비동기·점진적으로**
//! 밀어 넣는 후보를 받아, 매 순간 [`OverlayFrame`]/[`SearchBarFrame`] 을
//! 산출한다. §3.1 상태표의 "표시" → "갱신" 전이가 정확히 이 모듈이 다루는
//! 문제다.
//!
//! ⭐ **빈 질의 = 매치 없음** (명세 §3.1 "표시(Search-only)" 행 그대로).
//! `ultrakey_seek::query::QueryParams::default()` 의 기본 동작을 그대로
//! 쓴다 — 검색어가 비어 있으면 하이라이트도 연결선도 그리지 않는다.
//!
//! ⚠️ 처음 구현은 반대(`empty_query_matches_all: true`)였다. "S-6 의 증분
//! 채움을 사용자가 타이핑하기 전에도 볼 수 있어야 한다"는 이유였는데, 이는
//! **잘못된 전제**였다. 실제 시나리오에서 사용자는 Seek 을 열자마자 타이핑을
//! 시작하고, 검출 결과는 그 **뒤에**(주 디스플레이 ≈ 335 ms, 전체 ≈ 775 ms)
//! 도착한다 — 즉 증분 채움은 질의가 이미 걸린 상태에서 일어난다. S-6 은
//! 그대로 관찰 가능하고, 대신 화면 전체의 텍스트 수백 개가 세션 개시 순간에
//! 통째로 칠해지는 일(§5 엣지케이스 10 이 경계하는 바로 그 상태)이 사라진다.
//!
//! 검출이 진행 중이라는 사실은 하이라이트가 아니라 [`SearchBarFrame::detecting`]
//! 이 전달한다 — 그것이 §3.1 을 어기지 않고 S-6 을 UI 에 드러내는 자리다.

use std::collections::{BTreeMap, HashSet};

use ultrakey_seek::{merge::sort_reading_order, filter_by_query, QueryParams, Rect, TextCandidate};

use crate::geometry::{self, OverlayDisplay};
use crate::model::{Highlight, MatchRow, OverlayFrame, SearchBarFrame, Segment};
use crate::palette::{Appearance, Palette};

/// 검색 바 폭(pt). §1.1 실측(`defaults NSWindow Frame EntryBarWindow`).
pub const SEARCH_BAR_WIDTH_PT: f64 = 400.0;
/// 검색 바 높이(pt). §1.1 실측.
pub const SEARCH_BAR_HEIGHT_PT: f64 = 40.0;
/// 한 프레임에서 디스플레이별로 그리는 비선택 하이라이트 상한(§4.2).
pub const HIGHLIGHT_CAP_PER_DISPLAY: usize = 200;
/// 검색 바 매치 목록에 보여줄 최대 행 수(§1.1 `EntryOutlineView`).
pub const MATCH_LIST_MAX: usize = 100;

/// 이 세션의 내부 필터링 파라미터. 모듈 상단의 "빈 질의 = 매치 없음" 절 참조.
const QUERY_PARAMS: QueryParams = QueryParams { empty_query_matches_all: false };

/// Seek 오버레이 세션 — 검색어·후보·선택·디스플레이 배치를 함께 들고 있다.
#[derive(Debug)]
pub struct OverlaySession {
    displays: Vec<OverlayDisplay>,
    appearance: Appearance,
    reduce_motion: bool,
    detecting: bool,
    /// ⭐(이슈 #93) 인풋 박스 모드(검색 언어가 명시적 비영어) — `search_bar_frame()`
    /// 의 `input_mode` 로 웹뷰에 전달된다. 세션 하나에 고정(래칭)된다.
    input_mode: bool,
    query: String,
    by_display: BTreeMap<u32, Vec<TextCandidate>>,
    extra: Vec<TextCandidate>,
    /// `by_display` + `extra` 를 합쳐 읽기 순서로 정렬하고 질의 필터까지
    /// 적용한 캐시. `ingest_*`/`set_query`/`set_displays` 가 바뀔 때마다
    /// [`Self::recompute_matching`] 이 갱신한다 — `frames()`/`search_bar_frame()`
    /// 매 호출마다 다시 합치고 정렬하지 않기 위함이다.
    matching: Vec<TextCandidate>,
    selected_index: usize,
    bar_origin: (f64, f64),
}

impl OverlaySession {
    /// 세션을 연다. **후보 0개로 즉시 연다** — 검출을 기다리지 않는다(S-6).
    /// `detecting` 은 `true` 로 시작한다.
    #[must_use]
    pub fn open(displays: Vec<OverlayDisplay>, appearance: Appearance, reduce_motion: bool) -> Self {
        let bar_origin = displays.first().map_or((0.0, 0.0), geometry::default_bar_origin);

        Self {
            displays,
            appearance,
            reduce_motion,
            detecting: true,
            input_mode: false,
            query: String::new(),
            by_display: BTreeMap::new(),
            extra: Vec::new(),
            matching: Vec::new(),
            selected_index: 0,
            bar_origin,
        }
    }

    /// F-02 `on_display` 콜백의 소비자. 같은 `display_id` 가 다시 오면
    /// **교체**한다(재검출 대비).
    pub fn ingest_display(&mut self, display_id: u32, candidates: Vec<TextCandidate>) {
        tracing::trace!(
            display_id,
            count = candidates.len(),
            "seek overlay: received per-display candidates"
        );
        self.by_display.insert(display_id, candidates);
        self.recompute_matching();
    }

    /// AX 등 디스플레이가 특정되지 않는 후보(`display_id: None`)를 받는다.
    /// 호출할 때마다 이전 값을 통째로 교체한다 — AX 패스는 디스플레이별로
    /// 나뉘어 오지 않으므로 부분 갱신할 키가 없다.
    pub fn ingest_extra(&mut self, candidates: Vec<TextCandidate>) {
        tracing::trace!(count = candidates.len(), "seek overlay: received candidates with no display assigned");
        self.extra = candidates;
        self.recompute_matching();
    }

    /// 검출이 끝났음을 표시한다.
    pub fn finish_detection(&mut self) {
        self.detecting = false;
    }

    /// 질의를 갱신한다. 필터링은 [`ultrakey_seek::filter_by_query`] 를 그대로
    /// 쓴다(직접 구현하지 않는다 — F-02 의 책임). 선택 인덱스는 0 으로 리셋한다.
    pub fn set_query(&mut self, query: &str) {
        self.query = query.to_string();
        self.selected_index = 0;
        self.recompute_matching();
    }

    /// ⭐(이슈 #93) 인풋 박스 모드를 켠다 — `search_bar_frame().input_mode` 가
    /// 웹뷰 `<input>`/`<span>` 표시를 가른다. 세션 하나에 고정되며(래칭) 세션
    /// 중에 바뀌지 않는다(`SeekConfig::input_box_mode` 래칭과 같은 규약).
    pub fn set_input_mode(&mut self, enabled: bool) {
        self.input_mode = enabled;
    }

    /// 다음 매치로 선택을 옮긴다(§3.6 트리거: ↑/↓/Tab/`;`). 끝에서 처음으로
    /// 되감는다(wrap). 후보가 0개면 아무 일도 하지 않는다.
    pub fn cycle_next(&mut self) {
        if self.matching.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.matching.len();
    }

    /// 이전 매치로 선택을 옮긴다. 처음에서 끝으로 되감는다(wrap). 후보가
    /// 0개면 아무 일도 하지 않는다.
    pub fn cycle_prev(&mut self) {
        if self.matching.is_empty() {
            return;
        }
        self.selected_index = if self.selected_index == 0 {
            self.matching.len() - 1
        } else {
            self.selected_index - 1
        };
    }

    /// 현재 선택된 매치.
    #[must_use]
    pub fn selected(&self) -> Option<&TextCandidate> {
        self.matching.get(self.selected_index)
    }

    /// 검색 바 좌상단(전역 pt).
    #[must_use]
    pub fn search_bar_origin(&self) -> (f64, f64) {
        self.bar_origin
    }

    /// 검색 바 좌상단을 설정한다(사용자 드래그, §3.4 위치 저장은 이 값을
    /// 호출하는 쪽— F-09/설정 저장소 — 의 책임이다. 이 세션은 값만 들고
    /// 있는다).
    pub fn set_search_bar_origin(&mut self, x: f64, y: f64) {
        self.bar_origin = (x, y);
    }

    /// 디스플레이 배치를 갱신한다(핫플러그, §5 #1). 사라진 디스플레이의
    /// 후보는 버린다. 검색 바가 사라진 디스플레이에 있었으면 남은 디스플레이
    /// 중 하나로 재배치한다(§5 #14).
    pub fn set_displays(&mut self, displays: Vec<OverlayDisplay>) {
        let surviving_ids: HashSet<u32> = displays.iter().map(|d| d.display_id).collect();
        let removed = self.by_display.keys().filter(|id| !surviving_ids.contains(id)).count();
        if removed > 0 {
            tracing::debug!(removed, "seek overlay: dropping candidates for displays removed by hotplug");
        }
        self.by_display.retain(|id, _| surviving_ids.contains(id));
        self.displays = displays;
        self.recompute_matching();

        let bar_still_visible =
            geometry::display_for_point(&self.displays, self.bar_origin.0, self.bar_origin.1).is_some();
        if !bar_still_visible {
            if let Some(first) = self.displays.first() {
                tracing::debug!("seek overlay: display with the search bar disappeared; repositioning");
                self.bar_origin = geometry::default_bar_origin(first);
            }
            // 남은 디스플레이가 하나도 없으면(극단 케이스) 위치를 그대로
            // 둔다 — 다음 set_displays 로 디스플레이가 복귀하면 그때 다시
            // 판정한다.
        }
    }

    /// 지금 덮고 있는 디스플레이 목록.
    ///
    /// 핫플러그 알림(`didChangeScreenParameters`)은 배치와 무관한 변화에도
    /// 발화하므로, 호출자가 **실제로 달라졌을 때만** 재배치하도록 비교
    /// 대상을 노출한다.
    #[must_use]
    pub fn displays(&self) -> &[OverlayDisplay] {
        &self.displays
    }

    /// 검출이 아직 진행 중인가.
    #[must_use]
    pub fn is_detecting(&self) -> bool {
        self.detecting
    }

    /// ⭐(이슈 #134, 결정 1) **질의로 필터되지 않은** 후보 우주 전체
    /// (`by_display` 값 전부 + `extra`)를 돌려준다 — "사용자가 이 세션에서
    /// 본" 그 후보들이다. 워커가 완료 시점에 이 값을 스냅샷해 세션 간
    /// 캐시를 만들고, 재진입 시 `display_id` 로 다시 나눠 주입한다.
    ///
    /// ⭐ [`Self::search_bar_frame`] 의 `matches`(질의 필터 결과·`matching`
    /// 캐시)가 **아니다** — 필터된 목록을 캐시에 담으면 쿼리 밖 후보가
    /// 유실되어 재진입 첫 탐색의 질을 떨어뜨린다. 반드시 후보 우주 전체를
    /// 돌려줘야 한다.
    #[must_use]
    pub fn all_candidates(&self) -> Vec<TextCandidate> {
        let mut all: Vec<TextCandidate> = self.by_display.values().flatten().cloned().collect();
        all.extend(self.extra.iter().cloned());
        all
    }

    /// 디스플레이마다 한 장씩 렌더 프레임을 만든다. **전역 → 로컬 변환과
    /// 연결선 클리핑을 여기서 한다.**
    #[must_use]
    pub fn frames(&self) -> Vec<OverlayFrame> {
        let selected_ref = self.selected();
        let bar_bottom_center = (
            self.bar_origin.0 + SEARCH_BAR_WIDTH_PT / 2.0,
            self.bar_origin.1 + SEARCH_BAR_HEIGHT_PT,
        );
        // 연결선은 검색 바 하단 중앙 → 선택 매치의 가장 가까운 변 중점까지,
        // 전역 좌표에서 한 번만 정의한다(geometry::clip_segment 문서 참조).
        let global_line = selected_ref.map(|c| {
            let target = nearest_edge_midpoint(&c.frame, bar_bottom_center);
            Segment {
                x1: bar_bottom_center.0,
                y1: bar_bottom_center.1,
                x2: target.0,
                y2: target.1,
            }
        });

        let palette = Palette::for_appearance(self.appearance);
        let animate = !self.reduce_motion;

        self.displays
            .iter()
            .map(|display| {
                let mut selected_highlight: Option<Highlight> = None;
                let mut unselected_highlights: Vec<Highlight> = Vec::new();

                for (idx, candidate) in self.matching.iter().enumerate() {
                    // 안 A(디스플레이별 창) — 후보가 어느 창에 속하는지는
                    // display_id 필드가 아니라 프레임 중심점의 소속으로
                    // 판정한다. AX 후보(display_id: None)도 이렇게 자연스럽게
                    // 배정된다.
                    let (cx, cy) = candidate.frame.center();
                    let owner = match geometry::display_for_point(&self.displays, cx, cy) {
                        Some(owner) => owner,
                        // 어느 디스플레이에도 속하지 않는 후보(화면 밖) — 그리지 않는다.
                        None => continue,
                    };
                    if owner.display_id != display.display_id {
                        continue;
                    }

                    // 포인터 동일성 — self.matching 안의 바로 그 원소인지
                    // 확인한다. 값이 우연히 같은 다른 후보와 "선택됨"이
                    // 혼동되지 않게 한다.
                    let is_selected = selected_ref.is_some_and(|s| std::ptr::eq(s, candidate));

                    let highlight = Highlight {
                        rect: geometry::to_local(display, &candidate.frame),
                        label: (idx + 1).to_string(),
                        selected: is_selected,
                        source: candidate.source,
                    };

                    if is_selected {
                        selected_highlight = Some(highlight);
                    } else {
                        unselected_highlights.push(highlight);
                    }
                }

                // §4.2 상한 — 비선택 하이라이트만 자른다. 선택된 매치는 위에서
                // 이미 상한과 무관하게 확보돼 있다.
                let omitted =
                    unselected_highlights.len().saturating_sub(HIGHLIGHT_CAP_PER_DISPLAY);
                unselected_highlights.truncate(HIGHLIGHT_CAP_PER_DISPLAY);

                let mut highlights = unselected_highlights;
                if let Some(sel) = selected_highlight {
                    highlights.push(sel);
                }

                let line = global_line.and_then(|seg| geometry::clip_segment(display, seg));

                OverlayFrame {
                    display_id: display.display_id,
                    highlights,
                    line,
                    palette: palette.clone(),
                    animate,
                    omitted,
                }
            })
            .collect()
    }

    /// 검색 바 창에 보낼 프레임. 매치 목록은 [`MATCH_LIST_MAX`] 까지.
    #[must_use]
    pub fn search_bar_frame(&self) -> SearchBarFrame {
        let matches: Vec<MatchRow> = self
            .matching
            .iter()
            .take(MATCH_LIST_MAX)
            .enumerate()
            .map(|(index, c)| MatchRow {
                index,
                text: c.text.clone(),
                display_id: c.display_id,
                source: c.source,
            })
            .collect();

        SearchBarFrame {
            query: self.query.clone(),
            matches,
            selected_index: if self.matching.is_empty() {
                None
            } else {
                Some(self.selected_index)
            },
            total_matches: self.matching.len(),
            palette: Palette::for_appearance(self.appearance),
            detecting: self.detecting,
            input_mode: self.input_mode,
        }
    }

    /// `by_display` + `extra` 를 합쳐 읽기 순서로 정렬하고, 현재 질의로
    /// 필터링해 `matching` 캐시를 갱신한다. 선택 인덱스가 새 범위를 벗어나면
    /// 마지막 유효 인덱스로 당긴다(0개면 0으로).
    fn recompute_matching(&mut self) {
        let mut all: Vec<TextCandidate> = self.by_display.values().flatten().cloned().collect();
        all.extend(self.extra.iter().cloned());
        // F-02(ultrakey-seek)의 병합 계층이 쓰는 것과 같은 읽기 순서 정렬 —
        // 결정적이고, 검색 바 매치 목록·순환 순서가 화면을 읽는 순서와
        // 일치하게 한다(merge.rs 문서와 동일한 근거).
        sort_reading_order(&mut all);
        self.matching = filter_by_query(&all, &self.query, QUERY_PARAMS);

        if self.matching.is_empty() {
            self.selected_index = 0;
        } else if self.selected_index >= self.matching.len() {
            self.selected_index = self.matching.len() - 1;
        }
    }
}

/// 선택 매치 사각형에서 `from` 에 가장 가까운 변의 중점(§3.5 연결선 규칙:
/// "선택 매치 bounding box 의 인접 지점까지 직선").
fn nearest_edge_midpoint(rect: &Rect, from: (f64, f64)) -> (f64, f64) {
    let top = (rect.x + rect.width / 2.0, rect.y);
    let bottom = (rect.x + rect.width / 2.0, rect.y + rect.height);
    let left = (rect.x, rect.y + rect.height / 2.0);
    let right = (rect.x + rect.width, rect.y + rect.height / 2.0);

    [top, bottom, left, right]
        .into_iter()
        .min_by(|a, b| {
            dist_sq(*a, from)
                .partial_cmp(&dist_sq(*b, from))
                .unwrap_or(std::cmp::Ordering::Equal)
        })
        .unwrap_or(top)
}

fn dist_sq(a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = a.0 - b.0;
    let dy = a.1 - b.1;
    dx * dx + dy * dy
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultrakey_seek::CandidateSource;

    fn display(id: u32, x: f64, y: f64, w: f64, h: f64) -> OverlayDisplay {
        OverlayDisplay {
            display_id: id,
            frame: Rect { x, y, width: w, height: h },
            backing_scale: 2.0,
        }
    }

    fn candidate(text: &str, display_id: u32, x: f64, y: f64) -> TextCandidate {
        TextCandidate::ocr(
            text.to_string(),
            Rect { x, y, width: 10.0, height: 10.0 },
            0.9,
            display_id,
        )
    }

    // ── S-6 증분 수신 ────────────────────────────────────────────────────

    /// ⭐ 세션을 열면 즉시 `frames()` 가 디스플레이 수만큼 나오고 하이라이트가
    /// 0개인가. 그 뒤 `ingest_display` 를 하나씩 하면 그 디스플레이의
    /// 하이라이트만 늘어나는가(다른 디스플레이는 영향받지 않는가).
    #[test]
    fn s6_open_zero_highlights_then_incremental_ingest_per_display() {
        let displays = vec![display(1, 0.0, 0.0, 1000.0, 1000.0), display(2, 1000.0, 0.0, 1000.0, 1000.0)];
        let mut session = OverlaySession::open(displays, Appearance::Light, false);
        assert!(session.is_detecting());

        let frames0 = session.frames();
        assert_eq!(frames0.len(), 2);
        assert!(frames0.iter().all(|f| f.highlights.is_empty()));

        // ⭐ 실제 시나리오: 사용자는 세션을 열자마자 타이핑하고, 검출 결과는
        // 그 **뒤에** 도착한다(주 디스플레이 ≈ 335 ms, 전체 ≈ 775 ms). 질의를
        // 먼저 걸어 두고 후보가 늘어나는 것을 본다.
        session.set_query("a");
        assert!(session.frames().iter().all(|f| f.highlights.is_empty()), "후보가 없으면 여전히 0개");

        session.ingest_display(1, vec![candidate("Alpha", 1, 10.0, 10.0)]);
        let frames1 = session.frames();
        let d1 = frames1.iter().find(|f| f.display_id == 1).unwrap();
        let d2 = frames1.iter().find(|f| f.display_id == 2).unwrap();
        assert_eq!(d1.highlights.len(), 1);
        assert_eq!(d2.highlights.len(), 0);

        session.ingest_display(2, vec![candidate("Beta", 2, 1010.0, 10.0)]);
        let frames2 = session.frames();
        let d1b = frames2.iter().find(|f| f.display_id == 1).unwrap();
        let d2b = frames2.iter().find(|f| f.display_id == 2).unwrap();
        assert_eq!(d1b.highlights.len(), 1);
        assert_eq!(d2b.highlights.len(), 1);

        session.finish_detection();
        assert!(!session.is_detecting());

        // ⭐ §3.1 "표시(Search-only)" — 질의를 지우면 하이라이트가 사라진다.
        session.set_query("");
        assert!(session.frames().iter().all(|f| f.highlights.is_empty()));
    }

    /// 같은 `display_id` 가 다시 오면 교체돼야 한다(재검출 대비) — 누적되지 않는다.
    #[test]
    fn ingest_display_replaces_not_accumulates() {
        let displays = vec![display(1, 0.0, 0.0, 1000.0, 1000.0)];
        let mut session = OverlaySession::open(displays, Appearance::Light, false);
        session.set_query("alpha");
        session.ingest_display(1, vec![candidate("Alpha Old", 1, 10.0, 10.0)]);
        assert_eq!(session.frames()[0].highlights.len(), 1);

        session.ingest_display(
            1,
            vec![candidate("Alpha New1", 1, 10.0, 10.0), candidate("Alpha New2", 1, 30.0, 10.0)],
        );
        let frame = &session.frames()[0];
        assert_eq!(frame.highlights.len(), 2);
        assert!(session.search_bar_frame().matches.iter().all(|m| m.text != "Alpha Old"));
    }

    /// `ingest_extra`(AX 등 디스플레이 미특정 후보)도 반영되고, 다시 부르면 교체된다.
    #[test]
    fn ingest_extra_is_assigned_by_geometry_and_replaces_on_recall() {
        let displays = vec![display(1, 0.0, 0.0, 1000.0, 1000.0)];
        let mut session = OverlaySession::open(displays, Appearance::Light, false);
        session.set_query("button");
        session.ingest_extra(vec![TextCandidate::accessibility(
            "AX Button".to_string(),
            Rect { x: 100.0, y: 100.0, width: 40.0, height: 20.0 },
        )]);
        assert_eq!(session.frames()[0].highlights.len(), 1);
        assert_eq!(
            session.frames()[0].highlights[0].source,
            CandidateSource::Accessibility
        );

        session.ingest_extra(vec![]);
        assert!(session.frames()[0].highlights.is_empty());
    }

    // ── 상한 정책(§4.2) ──────────────────────────────────────────────────

    /// ⭐ 후보 500개에서 `frames()` 의 하이라이트가 200개 이하(비선택 상한)여야
    /// 하고, **선택된 매치는 상한과 무관하게 반드시 포함**되며(선택 인덱스를
    /// 400번째로 옮긴 뒤에도), `omitted` 가 정확해야 한다.
    #[test]
    fn highlight_cap_keeps_selected_and_reports_omitted() {
        let displays = vec![display(1, 0.0, 0.0, 10_000.0, 10.0)];
        let mut session = OverlaySession::open(displays, Appearance::Light, false);
        session.set_query("item");
        let candidates: Vec<TextCandidate> =
            (0..500).map(|i| candidate(&format!("Item {i}"), 1, f64::from(i) * 10.0, 0.0)).collect();
        session.ingest_display(1, candidates);

        for _ in 0..400 {
            session.cycle_next();
        }
        assert_eq!(session.selected().unwrap().text, "Item 400");

        let frames = session.frames();
        assert_eq!(frames.len(), 1);
        let frame = &frames[0];

        assert_eq!(frame.highlights.len(), HIGHLIGHT_CAP_PER_DISPLAY + 1, "비선택 200 + 선택 1");
        assert!(frame.highlights.iter().any(|h| h.selected && h.label == "401"));
        assert_eq!(frame.omitted, 500 - 1 - HIGHLIGHT_CAP_PER_DISPLAY, "비선택 499개 중 200개 초과분");
    }

    // ── 순환 ────────────────────────────────────────────────────────────

    /// `cycle_next`/`cycle_prev` 가 끝에서 되감는가(wrap). 후보 0개일 때
    /// 패닉하지 않는가.
    #[test]
    fn cycle_wraps_and_does_not_panic_on_empty() {
        let displays = vec![display(1, 0.0, 0.0, 1000.0, 1000.0)];
        let mut session = OverlaySession::open(displays, Appearance::Light, false);

        session.set_query("row");

        // 후보 0개 — 패닉하지 않아야 한다.
        session.cycle_next();
        session.cycle_prev();
        assert!(session.selected().is_none());

        session.ingest_display(
            1,
            vec![
                candidate("Row A", 1, 0.0, 0.0),
                candidate("Row B", 1, 10.0, 0.0),
                candidate("Row C", 1, 20.0, 0.0),
            ],
        );
        assert_eq!(session.selected().unwrap().text, "Row A");
        session.cycle_next();
        assert_eq!(session.selected().unwrap().text, "Row B");
        session.cycle_next();
        assert_eq!(session.selected().unwrap().text, "Row C");
        session.cycle_next(); // 끝에서 처음으로 되감는다.
        assert_eq!(session.selected().unwrap().text, "Row A");

        session.cycle_prev(); // 처음에서 끝으로 되감는다.
        assert_eq!(session.selected().unwrap().text, "Row C");
    }

    // ── 핫플러그(§5 #1, #14) ─────────────────────────────────────────────

    /// `set_displays` 로 디스플레이 하나를 빼면 그 후보가 사라지고 `frames()`
    /// 가 하나 줄어드는가. 검색 바가 사라진 디스플레이에 있었으면 옮겨졌는가.
    #[test]
    fn hotplug_removes_display_candidates_and_relocates_search_bar() {
        let displays = vec![display(1, 0.0, 0.0, 1000.0, 1000.0), display(2, 1000.0, 0.0, 1000.0, 1000.0)];
        let mut session = OverlaySession::open(displays, Appearance::Light, false);
        session.set_query("row");
        session.ingest_display(1, vec![candidate("Row A", 1, 10.0, 10.0)]);
        session.ingest_display(2, vec![candidate("Row B", 2, 1010.0, 10.0)]);
        assert_eq!(session.frames().len(), 2);
        assert_eq!(session.search_bar_frame().total_matches, 2);

        // 검색 바를 디스플레이 2 위에 둔다.
        session.set_search_bar_origin(1500.0, 100.0);

        // 디스플레이 2 를 제거한다.
        session.set_displays(vec![display(1, 0.0, 0.0, 1000.0, 1000.0)]);

        let frames = session.frames();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].display_id, 1);
        // 디스플레이 2 의 후보("Row B")는 버려졌으므로 "Row A" 하나만 남는다.
        assert_eq!(session.search_bar_frame().total_matches, 1);
        assert_eq!(session.search_bar_frame().matches[0].text, "Row A");

        // 검색 바가 사라진 디스플레이 위에 있었으므로 남은 디스플레이(1)로 재배치됐다.
        let (bx, by) = session.search_bar_origin();
        assert!((0.0..=1000.0).contains(&bx));
        assert!((0.0..=1000.0).contains(&by));
    }

    /// 검색 바가 살아남은 디스플레이 위에 있었다면 재배치되지 않는다.
    #[test]
    fn hotplug_does_not_relocate_bar_still_on_surviving_display() {
        let displays = vec![display(1, 0.0, 0.0, 1000.0, 1000.0), display(2, 1000.0, 0.0, 1000.0, 1000.0)];
        let mut session = OverlaySession::open(displays, Appearance::Light, false);
        session.set_search_bar_origin(100.0, 100.0); // 디스플레이 1 위.

        session.set_displays(vec![display(1, 0.0, 0.0, 1000.0, 1000.0)]);
        assert_eq!(session.search_bar_origin(), (100.0, 100.0));
    }

    // ── set_query ───────────────────────────────────────────────────────

    /// ⭐ **빈 질의 = 매치 없음**(§3.1 "표시(Search-only)" 행), 질의가 있으면
    /// `filter_by_query` 로 걸러진다.
    #[test]
    fn set_query_empty_yields_nothing_and_non_empty_filters() {
        let displays = vec![display(1, 0.0, 0.0, 1000.0, 1000.0)];
        let mut session = OverlaySession::open(displays, Appearance::Light, false);
        session.ingest_display(1, vec![candidate("Save", 1, 0.0, 0.0), candidate("Cancel", 1, 100.0, 0.0)]);

        // ⭐ 빈 질의 — 후보가 2개 들어와 있어도 아무것도 그리지 않는다.
        assert!(session.frames()[0].highlights.is_empty());
        assert_eq!(session.search_bar_frame().total_matches, 0);
        assert!(session.selected().is_none());

        session.set_query("sav");
        assert_eq!(session.frames()[0].highlights.len(), 1);
        assert_eq!(session.selected().unwrap().text, "Save");

        session.set_query("zzz-no-match");
        assert!(session.frames()[0].highlights.is_empty());
        assert!(session.selected().is_none());
    }

    /// `set_query` 는 선택 인덱스를 0 으로 리셋한다.
    #[test]
    fn set_query_resets_selected_index() {
        let displays = vec![display(1, 0.0, 0.0, 1000.0, 1000.0)];
        let mut session = OverlaySession::open(displays, Appearance::Light, false);
        session.set_query("row");
        session.ingest_display(1, vec![candidate("Row A", 1, 0.0, 0.0), candidate("Row B", 1, 10.0, 0.0)]);
        session.cycle_next();
        assert_eq!(session.selected().unwrap().text, "Row B");

        session.set_query("row");
        assert_eq!(session.selected().unwrap().text, "Row A");
    }

    // ── ⭐(이슈 #134, 결정 5) 최신화 시 선택 인덱스 유지 + clamp ────────────

    /// 캐시 주입 후 신규 OCR 이 목록을 교체할 때(재검출 — `ingest_display`
    /// 교체 의미론) 선택 인덱스는 **0 으로 리셋되지 않고** 새 범위 안에서는
    /// 유지되고 범위 밖이면 마지막 유효 인덱스로 당겨진다 — 결정 5
    /// "refresh keeps index, clamped" 를 고정하는 회귀 테스트다. 사용자가
    /// 위치시켜 둔 선택이 목록 길이가 허용하는 한 유지돼야 한다.
    ///
    /// ⚠️ 이 동작은 `recompute_matching` 의 **기존** 규칙이다 — 이 테스트는
    /// 그걸 잠그기만 할 뿐 코드를 바꾸지 않는다. 선택을 0 으로 리셋하는
    /// 것은 `set_query` 뿐이다(질의 변경 = 새 탐색, 교체 = 같은 탐색의
    /// 갱신).
    #[test]
    fn refresh_keeps_selected_index_clamped() {
        let displays = vec![display(1, 0.0, 0.0, 1000.0, 1000.0)];
        let mut session = OverlaySession::open(displays, Appearance::Light, false);
        session.set_query("row");

        // 5 개 목록에서 2 번 순환 → 인덱스 2("Row 2") 선택.
        let five: Vec<TextCandidate> = (0..5)
            .map(|i| candidate(&format!("Row {i}"), 1, f64::from(i) * 10.0, 0.0))
            .collect();
        session.ingest_display(1, five.clone());
        session.cycle_next();
        session.cycle_next();
        assert_eq!(session.selected_index, 2);
        assert_eq!(session.selected().unwrap().text, "Row 2");

        // ① 짧아진 교체(2 개) — 인덱스 2 는 새 범위 밖 → 마지막 유효
        //    인덱스 1 로 당긴다(0 은 아니다 — 리셋이 아니라 clamp).
        session.ingest_display(
            1,
            vec![candidate("Row A", 1, 0.0, 0.0), candidate("Row B", 1, 10.0, 0.0)],
        );
        assert_eq!(session.selected_index, 1, "범위 밖 → 마지막 유효 인덱스로 clamp");
        assert_eq!(session.selected().unwrap().text, "Row B");
        assert_eq!(
            session.search_bar_frame().selected_index,
            Some(1),
            "세션이 일관된 상태를 유지한다"
        );

        // ② 원래 길이(5 개)로 다시 교체 — 인덱스 1 은 유효 범위 → 유지된다
        //    (0 으로 리셋하지 않는다 — ingest 는 쿼리가 아니다). 인덱스 1 의
        //    텍스트는 이제 5 개 목록의 "Row 1" 이다(2 개 목록의 "Row B" 와
        //    다르다 — 그래도 위치는 보존됐는지를 보는 것이지 텍스트가
        //    아니다).
        session.ingest_display(1, five);
        assert_eq!(session.selected_index, 1, "같은 길이 교체는 인덱스를 보존한다 (0 리셋 금지)");
        assert_eq!(session.selected().unwrap().text, "Row 1");
    }
}
