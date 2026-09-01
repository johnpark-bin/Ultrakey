//! §4.2 — 오버레이 렌더링 색상·두께·애니메이션 상수.
//!
//! ⚠️ 명세 §4.2 는 이 절의 값이 "하나도 실측되지 않았다"고 명시적으로
//! 경고한다 — Seek 를 실제로 발동시켜 관찰하면 Screen Recording 권한
//! 프롬프트와 전체 화면 캡처가 뒤따르는 부작용이 있어 조사 시점에 시도하지
//! 않았다(`app-bundle-analysis.md` §8 항목 3). 아래 상수는 명세 §4.2 표의
//! 제안 기본값을 그대로 옮긴 것이며, 표에 값이 있는 세 자리(비선택 채움색
//! 라이트/다크, 선택 강조색)만 명세 그대로다. 표에 없는 나머지 필드는 이
//! 구현이 채운 `(추정)` 값이고, 그렇게 표시해 둔다.

use serde::Serialize;

/// 시스템 외관 모드(§3.5 — 배경이 아니라 시스템 외관을 기준으로 팔레트를
/// 고른다. 오버레이가 실제로 얹히는 배경은 임의의 앱 화면이라 실시간 배경
/// 색 분석은 범위 밖이기 때문).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Appearance {
    /// 라이트 모드.
    Light,
    /// 다크 모드.
    Dark,
}

/// 하이라이트·연결선·검색 바 색상 팔레트. CSS 로 바로 쓸 수 있는 문자열이다.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Palette {
    /// 비선택 하이라이트 채움색. 라이트 `rgba(255, 214, 0, 0.35)`, 다크
    /// `rgba(255, 214, 0, 0.45)` — 명세 §4.2 표 값(값 자체는 표에서도 `(추정)`).
    pub highlight_fill: String,
    /// 비선택 하이라이트 테두리색. `(추정)` — 채움색보다 진한 동일 계열.
    pub highlight_stroke: String,
    /// 선택 하이라이트 채움색. `(추정)` — 선택 강조색(`#0A84FF`)의 반투명.
    pub selected_fill: String,
    /// 선택 하이라이트 테두리색. `#0A84FF`(시스템 accent, 라이트/다크
    /// 공통) — 명세 §4.2 표.
    pub selected_stroke: String,
    /// 연결선 색상. 명세 §4.2: "선택 매치 강조 색상과 동일".
    pub line_stroke: String,
    /// 라벨(매치 순번) 글자색. `(추정)`.
    pub label_fg: String,
    /// 라벨(매치 순번) 배경색. `(추정)`.
    pub label_bg: String,
    /// 검색 바 배경색. `(추정)` — 실행 파일 문자열 단서 `NSVisualEffectMaterial`
    /// (§4.2)로 미루어 반투명 재질을 가정했다.
    pub bar_bg: String,
    /// 검색 바 글자색. `(추정)`.
    pub bar_fg: String,
    /// 비선택 하이라이트 테두리 두께(pt). 명세 §4.2 표: 1pt.
    pub unselected_border_pt: f64,
    /// 선택 하이라이트 테두리 두께(pt). 명세 §4.2 표: 2pt.
    pub selected_border_pt: f64,
    /// 연결선 두께(pt). 명세 §4.2 표: 1.5pt.
    pub line_width_pt: f64,
    /// 등장/이동 애니메이션 시간(ms). 명세 §4.2 표: 120ms(ease-out).
    /// 축소된 모션(§3.5)일 때는 팔레트가 아니라
    /// [`crate::model::OverlayFrame::animate`] 가 `false` 로 애니메이션 자체를
    /// 끈다 — 이 값은 켜져 있을 때의 지속시간이다.
    pub animation_ms: u32,
}

impl Palette {
    /// 시스템 외관에 맞는 팔레트를 만든다.
    #[must_use]
    pub fn for_appearance(appearance: Appearance) -> Self {
        match appearance {
            Appearance::Light => Self {
                highlight_fill: "rgba(255, 214, 0, 0.35)".to_string(),
                highlight_stroke: "rgba(191, 149, 0, 0.9)".to_string(),
                selected_fill: "rgba(10, 132, 255, 0.28)".to_string(),
                selected_stroke: "#0A84FF".to_string(),
                line_stroke: "#0A84FF".to_string(),
                label_fg: "#1A1A1A".to_string(),
                label_bg: "rgba(255, 255, 255, 0.85)".to_string(),
                bar_bg: "rgba(255, 255, 255, 0.72)".to_string(),
                bar_fg: "#1A1A1A".to_string(),
                unselected_border_pt: 1.0,
                selected_border_pt: 2.0,
                line_width_pt: 1.5,
                animation_ms: 120,
            },
            Appearance::Dark => Self {
                // 다크모드는 채도를 살짝 올려 대비를 유지한다(명세 §4.2 근거란).
                highlight_fill: "rgba(255, 214, 0, 0.45)".to_string(),
                highlight_stroke: "rgba(255, 214, 0, 0.9)".to_string(),
                selected_fill: "rgba(10, 132, 255, 0.35)".to_string(),
                selected_stroke: "#0A84FF".to_string(),
                line_stroke: "#0A84FF".to_string(),
                label_fg: "#F5F5F5".to_string(),
                label_bg: "rgba(20, 20, 20, 0.85)".to_string(),
                bar_bg: "rgba(30, 30, 30, 0.72)".to_string(),
                bar_fg: "#F5F5F5".to_string(),
                unselected_border_pt: 1.0,
                selected_border_pt: 2.0,
                line_width_pt: 1.5,
                animation_ms: 120,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 라이트/다크가 실제로 다른 값을 내는가 — 비선택 하이라이트 채움색은
    /// 명세 §4.2 표에서 알파값이 다르게 확정돼 있다(0.35 vs 0.45).
    #[test]
    fn light_and_dark_palettes_differ() {
        let light = Palette::for_appearance(Appearance::Light);
        let dark = Palette::for_appearance(Appearance::Dark);
        assert_eq!(light.highlight_fill, "rgba(255, 214, 0, 0.35)");
        assert_eq!(dark.highlight_fill, "rgba(255, 214, 0, 0.45)");
        assert_ne!(light.highlight_fill, dark.highlight_fill);
        assert_ne!(light.bar_bg, dark.bar_bg);
        assert_ne!(light.label_bg, dark.label_bg);
    }

    /// ⭐ 이슈 #67 — 검색 바 글자색은 라이트에서 검정(#1A1A1A), 다크에서 흰색
    /// 계열(#F5F5F5)로 고정돼야 한다. 라이트에서 흰 글자가 그려진 결함의 회귀
    /// 방지: 웹뷰 CSS 폴백(`overlay-searchbar.html` 의 `--bar-fg: #ffffff`)이
    /// 팔레트 전달 실패를 덮어 보이는 일이 없게, 양쪽 값을 여기서도 고정한다.
    #[test]
    fn bar_fg_is_dark_on_light_and_light_on_dark() {
        let light = Palette::for_appearance(Appearance::Light);
        let dark = Palette::for_appearance(Appearance::Dark);
        assert_eq!(light.bar_fg, "#1A1A1A");
        assert_eq!(dark.bar_fg, "#F5F5F5");
        assert_ne!(light.bar_fg, dark.bar_fg);
    }

    /// 선택 강조색은 명세 §4.2 표대로 라이트/다크 공통 `#0A84FF` 다.
    #[test]
    fn selected_stroke_is_same_across_appearances() {
        let light = Palette::for_appearance(Appearance::Light);
        let dark = Palette::for_appearance(Appearance::Dark);
        assert_eq!(light.selected_stroke, "#0A84FF");
        assert_eq!(dark.selected_stroke, "#0A84FF");
    }

    /// 두께·애니메이션 상수는 명세 §4.2 표 값과 일치해야 한다.
    #[test]
    fn thickness_and_animation_constants_match_spec_table() {
        let p = Palette::for_appearance(Appearance::Light);
        assert!((p.unselected_border_pt - 1.0).abs() < f64::EPSILON);
        assert!((p.selected_border_pt - 2.0).abs() < f64::EPSILON);
        assert!((p.line_width_pt - 1.5).abs() < f64::EPSILON);
        assert_eq!(p.animation_ms, 120);
    }
}
