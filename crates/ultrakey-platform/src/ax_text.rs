//! F-02 소스 B — Accessibility 트리 파싱 (B1~B4).
//!
//! `docs/spec/seek-text-detection.md` §3.3 이 확정한 대로, 이 소스는
//! **언제나 최전면 창 하나**로 범위가 고정된다 — `Only Seek in the frontmost
//! window` 설정(그 설정은 소스 A/OCR 의 캡처 범위만 좁힌다, §3.3.2)과 무관하다.
//! `Seek using macOS accessibility` 를 켤지 말지는 상위 계층(호출자)이 판단한다
//! — 이 모듈 자체는 "켜졌다고 가정하고 스캔한다" 는 순수 동작만 담당한다.
//!
//! ⭐ `ultrakey-platform` 은 `ultrakey-seek` 에 의존하지 않는다(의존 방향을
//! 만들지 않는다) — 그래서 이 모듈은 `TextCandidate` 대신 자체 출력 타입
//! [`AxTextElement`] 를 쓴다. 좌표계는 `kAXPositionAttribute`/`kAXSizeAttribute`
//! 가 이미 돌려주는 그대로 — 전역 화면 좌표, 포인트 단위, 좌상단 원점이라
//! §3.3 이 말하는 대로 추가 변환이 필요 없다(소스 A 와 달리).
//!
//! ⚠️ AX 트리 순회는 프로세스 간 동기 IPC 다(§3.3.3). 응답하지 않는 앱이
//! 순회 전체를 무기한 블로킹하지 못하게 두 가지를 강제한다:
//! - **메시징 타임아웃**(`AXUIElementSetMessagingTimeout`, §5 #7) — 시스템 와이드
//!   요소와 대상 앱 요소 양쪽에 건다.
//! - **깊이·요소 수 상한** — 병적으로 깊거나 넓은 트리(브라우저 DOM 미러,
//!   §3.3.3)에서 순회 시간이 폭증하지 않게 막는다.
//!
//! 한 요소·한 하위 트리에서 난 `AXError` 는 그 부분만 건너뛰고 순회를
//! 계속한다(§5 #6·#7) — 한 앱이 죽어도 전체 스캔이 멈추면 안 된다.

/// AX 트리에서 읽어낸 원시 텍스트 요소 하나.
///
/// 좌표는 `kAXPositionAttribute`/`kAXSizeAttribute` 가 준 그대로 — 전역 화면
/// 좌표, 포인트 단위, 좌상단 원점이다(CoreGraphics 관례). AX 매치는 텍스트의
/// 정확한 위치를 알 수 없으므로(§3.3 ⓘ 팝오버 원문) 이 사각형은 **요소
/// 전체**를 가리킨다 — 문자 단위로 정밀한 소스 A 의 `boundingBox` 와 다르다.
#[derive(Debug, Clone, PartialEq)]
pub struct AxTextElement {
    /// `kAXValueAttribute` → `kAXTitleAttribute` → `kAXDescriptionAttribute`
    /// 순으로 첫 번째로 얻어진 문자열(B2).
    pub text: String,
    /// `kAXPositionAttribute.x` — 전역 화면 좌표(포인트).
    pub x: f64,
    /// `kAXPositionAttribute.y` — 전역 화면 좌표(포인트).
    pub y: f64,
    /// `kAXSizeAttribute.width` — 포인트.
    pub width: f64,
    /// `kAXSizeAttribute.height` — 포인트.
    pub height: f64,
    /// `kAXRoleAttribute` — 조회에 실패하거나 값이 없으면 `None`. 순회 로직에
    /// 영향을 주지 않는 부가 정보(디버깅·상위 계층의 필터링 용도).
    pub role: Option<String>,
}

/// [`scan_frontmost_window`] 스캔 파라미터.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AxScanParams {
    /// 각 AX 호출의 메시징 타임아웃(초). `AXUIElementSetMessagingTimeout` 에
    /// 그대로 전달된다(§3.3.3).
    pub messaging_timeout_secs: f32,
    /// 트리 순회 깊이 상한(§3.3.3) — 루트(최전면 창)를 깊이 0 으로 센다.
    pub max_depth: usize,
    /// 방문(스택에서 꺼내 처리한) 요소 수 상한 — 병적으로 넓은 트리(브라우저
    /// DOM 미러 등) 방어. 스택에 쌓아 둘 항목 수도 이 상한 근처로 억제한다.
    pub max_elements: usize,
    /// `minAxCharCount`(§3.3.1). 이 글자 수(`chars().count()`, 바이트 수
    /// 아님) 미만인 텍스트는 버린다.
    pub min_char_count: usize,
}

impl Default for AxScanParams {
    fn default() -> Self {
        Self {
            // ⭐ 근거: 명세는 "수백 ms 단위"라고만 한다(§3.3.3) — 구체적
            // 수치는 이 구현이 정한다. 500ms 는 그 범위의 중간값으로,
            // 로컬 IPC 로는 응답하는 앱을 잘라내지 않으면서도 완전히
            // 멈춘 앱을 사용자가 체감하기 전에 건너뛰기에 충분히 짧다.
            messaging_timeout_secs: 0.5,
            // ⭐ 근거: 명세에 수치가 없다(§3.3.3, §9 미해결). 일반 네이티브
            // macOS 앱 창의 AX 트리는 보통 10~15 단계 안에 끝난다. 20 은
            // 그보다 여유를 두면서도, 브라우저처럼 병적으로 깊은 DOM 미러
            // 트리(수십~수백 단계)에서는 확실히 잘라내는 값이다.
            max_depth: 20,
            // ⭐ 근거: 명세에 수치가 없다. 각 요소 처리마다 최소 5회의 AX
            // IPC 라운드트립(children, value/title/description, position,
            // size)이 발생할 수 있다 — 상한을 너무 크게 두면 IPC 총량이
            // 지연 예산(§5 #8, platform-constraints.md P3)을 넘어설
            // 위험이 커진다. 2000 은 스프레드시트·표처럼 요소가 많은 화면도
            // 대부분 커버하면서 총 IPC 호출을 만 회 안팎으로 묶는 값이다.
            max_elements: 2000,
            // 실측 확정값(§3.3.1, §4) — 아무 설정도 건드리지 않은 plist 에
            // 이미 시딩되어 있는 `minAxCharCount = 2` 와 일치시킨다.
            min_char_count: 2,
        }
    }
}

/// [`scan_frontmost_window`] 실패.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AxScanError {
    /// Accessibility 권한이 없다(`AXIsProcessTrusted() == false`). 권한 획득
    /// UX 자체는 F-11 소관이다 — 이 모듈은 구분만 한다(요구사항 8).
    #[error("Accessibility 권한이 없어 AX 트리를 순회할 수 없음")]
    NotTrusted,
}

/// B3 범위 필터의 순수 규칙 — `min_char_count` 미만이거나 크기가 0/음수인
/// 요소를 버린다. FFI 가 없어 macOS 없이도 단위 테스트할 수 있다.
///
/// ⚠️ `text.len()`(바이트 수)이 아니라 `text.chars().count()`(문자 수)로
/// 센다 — 한글 등 다바이트 문자에서 둘이 갈린다("안"은 3바이트지만 1글자다).
fn passes_filter(text: &str, width: f64, height: f64, min_char_count: usize) -> bool {
    text.chars().count() >= min_char_count && width > 0.0 && height > 0.0
}

#[cfg(target_os = "macos")]
mod macos_impl {
    use super::{passes_filter, AxScanError, AxScanParams, AxTextElement};
    use axuielement::ax_attribute::attributes as ax_attr;
    use axuielement::AXUIElement;

    /// 최전면 창 하나를 순회해 텍스트 요소 목록을 만든다(B1~B4).
    ///
    /// **B1 루트 획득 경로**: `AXUIElementCreateSystemWide()` →
    /// `kAXFocusedApplicationAttribute` → 그 앱의 `kAXFocusedWindowAttribute`.
    ///
    /// 이 경로를 고른 근거: `kAXFocusedApplicationAttribute` 는 이미 포커스된
    /// 앱의 pid 로 스코프된 `AXUIElement` 를 직접 돌려준다. 대안(시스템
    /// 와이드의 `kAXFocusedUIElement` 로 임의 요소를 얻은 뒤 `AXUIElementGetPid`
    /// 로 pid 를 뽑아 `AXUIElementCreateApplication(pid)` 를 다시 호출하는
    /// 경로)은 라운드트립이 하나 더 필요하고, pid 조회와 앱 요소 생성 사이에
    /// 포커스가 바뀌는 경쟁 상태의 여지도 생긴다. 채택한 경로는 그 여지가
    /// 없다.
    pub fn scan_frontmost_window(
        params: &AxScanParams,
    ) -> Result<Vec<AxTextElement>, AxScanError> {
        // 요구사항 8: 권한 부재는 다른 실패와 구분되는 오류로 보고한다.
        if !crate::accessibility::is_process_trusted() {
            return Err(AxScanError::NotTrusted);
        }

        let Some(system_wide) = AXUIElement::system_wide() else {
            // `AXUIElementCreateSystemWide` 자체가 실패하는 경우 — 극히
            // 드물다. §5 #2 취급과 동일하게 빈 결과로 정상 진행한다.
            return Ok(Vec::new());
        };
        // ⭐ 시스템 와이드 요소에도 타임아웃을 건다(§5 #7 요구사항) — 이
        // 요소를 거쳐 포커스된 앱을 조회하는 것도 프로세스 간 IPC 다.
        let _ = system_wide.set_timeout(params.messaging_timeout_secs);

        let Ok(Some(focused_app)) =
            system_wide.element_attribute(ax_attr::AX_FOCUSED_APPLICATION_ATTRIBUTE)
        else {
            // 포커스된 앱이 없거나(예: 모든 창이 최소화됨) 조회 자체가 실패.
            return Ok(Vec::new());
        };
        // ⭐ 대상 앱 요소에도 타임아웃을 건다 — 실제 트리 순회 대부분이
        // 상대할 프로세스가 바로 이것이다. §3.3.3 이 말하는 "각 대상
        // 프로세스"가 여기다.
        let _ = focused_app.set_timeout(params.messaging_timeout_secs);

        let Ok(Some(root)) =
            focused_app.element_attribute(ax_attr::AX_FOCUSED_WINDOW_ATTRIBUTE)
        else {
            // 포커스된 창이 없음(§5 #6 과 유사한 사례 — 이 앱은 창이 없다) —
            // 이 앱은 건너뛰고 빈 결과로 정상 진행한다.
            return Ok(Vec::new());
        };

        Ok(walk(root, params))
    }

    /// B2 순회. 재귀 대신 **명시적 스택**을 쓴다(구현 규칙 5) — 깊은 트리에서
    /// 재귀 호출 스택이 아니라 힙에 쌓인 `Vec` 가 자라므로 스택 오버플로가
    /// 나지 않는다.
    fn walk(root: AXUIElement, params: &AxScanParams) -> Vec<AxTextElement> {
        let mut out = Vec::new();
        // (요소, 이 요소의 깊이) — 루트는 깊이 0.
        let mut stack: Vec<(AXUIElement, usize)> = vec![(root, 0)];
        let mut visited = 0usize;

        while let Some((element, depth)) = stack.pop() {
            if visited >= params.max_elements {
                break;
            }
            visited += 1;

            if let Some(candidate) = extract(&element, params.min_char_count) {
                out.push(candidate);
            }

            if depth >= params.max_depth {
                // 깊이 상한 — 더 내려가지 않는다. 이 요소 자체는 이미
                // 처리했다.
                continue;
            }

            // 요구사항 6: `children()` 이 AXError 를 내도(예: 응답 없는
            // 하위 트리) 이 하위 트리만 건너뛰고 나머지 스택은 계속 처리한다.
            if let Ok(children) = element.children() {
                for child in children {
                    if visited + stack.len() >= params.max_elements {
                        break;
                    }
                    stack.push((child, depth + 1));
                }
            }
        }

        out
    }

    /// 한 요소에서 텍스트·위치·크기·role 을 뽑는다(B2). 텍스트가 없거나
    /// 위치/크기를 읽지 못했거나 B3 필터를 통과하지 못하면 `None`.
    fn extract(element: &AXUIElement, min_char_count: usize) -> Option<AxTextElement> {
        // B2: kAXValueAttribute → kAXTitleAttribute → kAXDescriptionAttribute
        // 순으로 첫 번째로 문자열이 있는 것. 개별 속성 조회가 AXError 를
        // 내도(요구사항 6) 이 요소 전체를 포기하지 않고 다음 속성으로
        // 폴백한다 — `.ok()` 가 그 폴백이다.
        let text = element
            .string_attribute(ax_attr::AX_VALUE_ATTRIBUTE)
            .ok()
            .flatten()
            .or_else(|| {
                element
                    .string_attribute(ax_attr::AX_TITLE_ATTRIBUTE)
                    .ok()
                    .flatten()
            })
            .or_else(|| {
                element
                    .string_attribute(ax_attr::AX_DESCRIPTION_ATTRIBUTE)
                    .ok()
                    .flatten()
            })?;

        // 위치/크기를 읽지 못한 요소는 후보가 될 수 없다(구현 요구사항 4).
        let position = element
            .point_attribute(ax_attr::AX_POSITION_ATTRIBUTE)
            .ok()
            .flatten()?;
        let size = element
            .size_attribute(ax_attr::AX_SIZE_ATTRIBUTE)
            .ok()
            .flatten()?;

        if !passes_filter(&text, size.width, size.height, min_char_count) {
            return None;
        }

        // role 은 부가 정보라 조회 실패를 그냥 `None` 으로 흡수한다.
        let role = element
            .string_attribute(ax_attr::AX_ROLE_ATTRIBUTE)
            .ok()
            .flatten();

        Some(AxTextElement {
            text,
            x: position.x,
            y: position.y,
            width: size.width,
            height: size.height,
            role,
        })
    }
}

#[cfg(target_os = "macos")]
pub use macos_impl::scan_frontmost_window;

#[cfg(not(target_os = "macos"))]
mod stub_impl {
    use super::{AxScanError, AxScanParams, AxTextElement};

    /// macOS 가 아닌 타깃에서는 항상 빈 결과 — `accessibility.rs` 의 스텁
    /// 관례와 동일하다.
    pub fn scan_frontmost_window(
        _params: &AxScanParams,
    ) -> Result<Vec<AxTextElement>, AxScanError> {
        Ok(Vec::new())
    }
}

#[cfg(not(target_os = "macos"))]
pub use stub_impl::scan_frontmost_window;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_out_short_korean_text_by_char_count_not_byte_count() {
        // "안" 은 1글자지만 UTF-8 로는 3바이트다 — 바이트 길이로 잘못 셌다면
        // min_char_count=2 를 통과해버린다.
        assert!(!passes_filter("안", 10.0, 10.0, 2));
    }

    #[test]
    fn passes_two_char_korean_text() {
        assert!(passes_filter("안녕", 10.0, 10.0, 2));
    }

    #[test]
    fn passes_english_text_meeting_min_char_count() {
        assert!(passes_filter("OK", 10.0, 10.0, 2));
    }

    #[test]
    fn filters_out_single_char_english_text() {
        assert!(!passes_filter("A", 10.0, 10.0, 2));
    }

    #[test]
    fn min_char_count_one_allows_single_char() {
        assert!(passes_filter("A", 10.0, 10.0, 1));
    }

    #[test]
    fn filters_out_zero_width() {
        assert!(!passes_filter("hello", 0.0, 10.0, 2));
    }

    #[test]
    fn filters_out_zero_height() {
        assert!(!passes_filter("hello", 10.0, 0.0, 2));
    }

    #[test]
    fn filters_out_negative_size() {
        assert!(!passes_filter("hello", -1.0, 10.0, 2));
    }

    #[test]
    fn filters_out_empty_text() {
        assert!(!passes_filter("", 10.0, 10.0, 1));
    }

    #[test]
    fn default_min_char_count_matches_seeded_plist_value() {
        // §3.3.1 실측 — 아무 설정도 안 건드린 plist 에 이미 `minAxCharCount = 2` 가
        // 시딩되어 있다.
        assert_eq!(AxScanParams::default().min_char_count, 2);
    }

    #[cfg(not(target_os = "macos"))]
    #[test]
    fn stub_returns_empty_ok_on_non_macos() {
        assert_eq!(
            scan_frontmost_window(&AxScanParams::default()),
            Ok(Vec::new())
        );
    }
}
