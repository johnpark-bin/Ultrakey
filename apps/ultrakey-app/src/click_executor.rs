//! ⭐ F-04 — 실 클릭 실행기. 명세: `docs/spec/seek-click-execution.md`.
//!
//! `ultrakey_seek_session::ClickExecutor` 트레이트의 실제 구현이다 — F-01
//! 워커가 `ConfirmedMatch` 를 넘기면 다음 순서로 실행한다(계획 초안 §2.3):
//!
//! 1. 모드 해석(설정 × modifier 스냅샷)·클릭 계획 산출 — 순수(`ultrakey-click`)
//! 2. 화면 밖 검증 — 벗어나면 조용히 취소(엣지 7, §8 수용 기준 14)
//! 3. `Focus window before clicking` ON — AX 재조회 → 창 조상 → 활성화(§3.7)
//! 4. AX 후보 + 단일 클릭 → `kAXPressAction` — 실패 시 좌표 폴백(§3.3.1)
//! 5. 좌표 클릭 합성(`ultrakey_platform::click_synthesis`) + 워프 복귀 + ⌘C(D11)
//!
//! ⭐ 이 파일은 **워커 스레드**에서 실행된다(`seek.rs` 문서). `AppHandle` 을
//! 들고 있는 이유는 포커스 전환(`NSRunningApplication`)을 메인 스레드에
//! 디스패치하기 위해서다 — AppKit 호출은 메인 스레드 전용이 불확실하므로(D8,
//! 이슈 #44), 폴링 조회까지 `seek.rs:138-159` 의 채널 왕복 패턴
//! (`run_on_main_thread` + 응답 채널)으로 감싼다.

use std::thread;
use std::time::Duration;

use axuielement::ax_action::{AX_PRESS_ACTION, AX_RAISE_ACTION};
use axuielement::ax_attribute::attributes as ax_attr;
use axuielement::ax_attribute::roles::AX_WINDOW_ROLE;
use axuielement::AXUIElement;
use objc2_app_kit::{NSApplicationActivationOptions, NSRunningApplication};
use ultrakey_core::flags::EventFlags;
use ultrakey_core::keycode::KeyCode;
use ultrakey_platform::click_synthesis;
use ultrakey_platform::event::SyntheticEvent;
use ultrakey_seek::Rect;
use ultrakey_seek_session::confirm::{ClickError, ClickSettings, ConfirmAction, ConfirmedMatch};
use ultrakey_click::path::{self, AxOutcome, Press, Requery};
use ultrakey_click::plan::ClickPlan;
use ultrakey_click::{mode, point, visible};

/// AX 호출 메시징 타임아웃(초) — `ax_text.rs` 의 기본값과 같다. 응답 없는 앱이
/// 워커를 영원히 블로킹하지 않게 한다(§5 #1, 엣지 14).
const AX_TIMEOUT_SECS: f32 = 0.5;
/// ⭐(이슈 #133) 창 전면화 — `kAXWindowsAttribute` 순회 상한(개). 병적 앱
/// (브라우저 DOM 미러 등이 창 수백 개를 노출하는 경우) 방어 — 초과 시 AX 경로를
/// 포기하고 앱 수준 폴백으로 내려간다.
const MAX_AX_WINDOW_ITERATION: usize = 200;
/// `Focus window before clicking` — `isActive` 폴링 간격(결정 D8, 이슈 #44 —
/// 설계 판단, 실기기 조정 지점).
const FOCUS_POLL_INTERVAL: Duration = Duration::from_millis(20);
/// 상동 — 폴링 상한(회). 20ms × 15 = 300ms, 초과 시 클릭을 그대로 진행한다
/// (열화 — 첫 클릭이 활성화에 소모되는 macOS 기본 동작으로 넘어간다).
const FOCUS_POLL_LIMIT: usize = 15;
/// 부모 순회(`AXWindow` 조상 탐색) 깊이 상한(결정 J4, 이슈 #44) — 버튼→도구
/// 모음→창이 보통 2~3 단계고, 브라우저 DOM 미러 같은 병적 트리 방어용.
const MAX_WINDOW_ANCESTOR_DEPTH: usize = 10;

/// 실제 클릭 실행기. `SeekController`(워커)가 `Box<dyn ClickExecutor>` 로
/// 소유한다.
pub struct ClickExecutor {
    app: tauri::AppHandle,
    settings: ClickSettings,
}

impl ClickExecutor {
    /// 워커 기동 시점의 초기 설정으로 만든다.
    #[must_use]
    pub fn new(app: tauri::AppHandle, settings: ClickSettings) -> Self {
        Self { app, settings }
    }
}

impl ultrakey_seek_session::ClickExecutor for ClickExecutor {
    fn configure(&mut self, settings: ClickSettings) {
        tracing::debug!(
            focus_window = settings.focus_window_before_clicking,
            change_modes = settings.change_click_modes_with_modifiers,
            "click settings applied"
        );
        self.settings = settings;
    }

    fn execute(&mut self, target: &ConfirmedMatch) -> Result<(), ClickError> {
        // ⭐(이슈 #133 판정 조건 4) — 확정 동작은 소스로 갈린다: 창 제목 후보는
        // 좌표 클릭이 아니라 **창 전면화**다. `FrontWindow` 는 이 자리에서 즉시
        // 반환해 클릭 경로(아래 1~5단계)로 **들어가지 않는다** — F-01 의
        // `notify_click_finished` 는 executor 반환 후 호출되므로 세션 닫힘은
        // 그 경로 그대로 성립한다.
        match target.action() {
            ConfirmAction::FrontWindow => return self.front_window(target),
            ConfirmAction::Click => {}
        }

        // 1) 모드 해석 · 클릭 계획 산출(순수).
        let click_mode = mode::resolve_click_mode(
            self.settings.change_click_modes_with_modifiers,
            target.modifiers,
        );
        let flags = if self.settings.change_click_modes_with_modifiers {
            EventFlags::NONE
        } else {
            // §3.2 — 설정 OFF 상태에서는 modifier 가 클릭 이벤트에 그대로 실린다.
            ultrakey_click::plan::passthrough_flags(target.modifiers)
        };
        let use_ax_path = path::should_use_ax_path(target.source, click_mode);
        let mut plan = ultrakey_click::plan::plan_for(click_mode, target.frame, flags, use_ax_path);

        // 2) 화면 밖 검증(엣지 7) — 벗어나면 조용히 취소.
        let bounds: Vec<Rect> = click_synthesis::active_display_bounds()
            .into_iter()
            .map(|d| Rect {
                x: d.x,
                y: d.y,
                width: d.width,
                height: d.height,
            })
            .collect();
        if !visible::point_visible(plan.point, &bounds) {
            let x = plan.point.x;
            let y = plan.point.y;
            tracing::warn!(
                x,
                y,
                "click point ({x}, {y}) is off-screen; canceling click"
            );
            return Ok(());
        }

        // 3) 포커스 전환 — ON 일 때만(§3.2, §8 수용 기준 11: OFF 면 활성화 API
        // 호출이 전혀 없다). 재조회 실패(요소 없음/IPC 오류)는 §5 #1(창 닫힘)로
        // 취급해 조용히 취소한다 — 좌표 폴백을 계속하면 엉뚱한 창을 클릭할 수
        // 있다(시나리오 E, J5).
        let mut focus_element: Option<AXUIElement> = None;
        if self.settings.focus_window_before_clicking {
            match requery_element(plan.point) {
                Ok(Some(element)) => {
                    focus_element = Some(element.clone());
                    self.focus_window_at(&element);
                }
                _ => {
                    let x = plan.point.x;
                    let y = plan.point.y;
                    tracing::warn!(
                        x,
                        y,
                        "failed to get AX element at click point ({x}, {y}); canceling click"
                    );
                    return Ok(());
                }
            }
        }

        // 4) AX press 시도 — 좁은 조건(source==AX && 단일 클릭)에서만(§3.3.1).
        if plan.use_ax_path {
            // 재조회 — 포커스 단계에서 얻은 요소를 재사용해 IPC 를 아낀다(계획
            // 초안 §2.3). `requery_element` 의 `Err` 와 `Ok(None)` 은 둘 다
            // `Requery::Missing` 으로 뭉개서 `ax_path_outcome` 의 Cancel 로
            // 떨어뜨린다(J5 — 프로세스 실패와 요소 부재를 실행 정책에서 구분할
            // 이유가 없고, 둘 다 §5 #1 이다).
            let element: Option<AXUIElement> = match &focus_element {
                Some(element) => Some(element.clone()),
                None => requery_element(plan.point).ok().flatten(),
            };
            let requery = if element.is_some() {
                Requery::Found
            } else {
                Requery::Missing
            };

            // D9 — `action_names` 는 참고·로그용이지 게이트가 아니다(판정은
            // `perform_action` 결과가 한다). 실패 사유는 폴백 로그에 실어 보낸다.
            let (press, press_error) =
                match element.as_ref().map(|e| e.perform_action(AX_PRESS_ACTION)) {
                    Some(Ok(())) => (Press::Succeeded, None),
                    Some(Err(e)) => (Press::Failed, Some(e.to_string())),
                    None => (Press::Failed, None),
                };

            // ⭐ 판정은 `ax_path_outcome` 하나가 3분기로 결정한다(J5).
            match path::ax_path_outcome(requery, press) {
                AxOutcome::Pressed => {
                    let x = plan.point.x;
                    let y = plan.point.y;
                    tracing::debug!(
                        x,
                        y,
                        "performed AX press on element at ({x}, {y})"
                    );
                    return Ok(());
                }
                AxOutcome::CoordinateFallback => {
                    let x = plan.point.x;
                    let y = plan.point.y;
                    let error = press_error.unwrap_or_default();
                    tracing::warn!(
                        %error,
                        x,
                        y,
                        "AX press action failed ({error}); falling back to coordinate click"
                    );
                    // 좌표 폴백 — 5단계로 계속.
                }
                AxOutcome::Cancel => {
                    let x = plan.point.x;
                    let y = plan.point.y;
                    tracing::warn!(
                        x,
                        y,
                        "failed to get AX element at click point ({x}, {y}); canceling click"
                    );
                    return Ok(());
                }
            }
        }

        // 5) 좌표 클릭 — AX 경로가 아닌 조합(OCR 후보·6종 모드·press 폴백) 전부.
        self.coordinate_execute(&mut plan)
    }
}

/// 클릭 지점의 AX 요소를 시스템 와이드로 재조회한다(A1). 보관된 핸들이 없으므로
/// "창 이동 후의 요소"를 다시 잡는 유일한 수단이 이것이다(명세 §5 #2 로
/// 문서화된 한계 — 정밀한 속성 재조회는 구조적으로 불가능).
fn requery_element(point: point::Point) -> Result<Option<AXUIElement>, ()> {
    let Some(system_wide) = AXUIElement::system_wide() else {
        return Err(());
    };
    let _ = system_wide.set_timeout(AX_TIMEOUT_SECS);
    match system_wide.element_at_position(point.x as f32, point.y as f32) {
        Ok(element) => {
            if let Some(element) = element.as_ref() {
                let _ = element.set_timeout(AX_TIMEOUT_SECS);
            }
            Ok(element)
        }
        Err(_) => Err(()),
    }
}

/// `AXParent` 순회로 `AXWindow` 역할의 조상 요소를 찾는다 — `element` 자신이
/// 창이면 그대로. 깊이 상한 [`MAX_WINDOW_ANCESTOR_DEPTH`] 를 넘거나 조상이
/// 끊기면(요소가 최상위거나 IPC 실패) `None`.
fn find_window_ancestor(mut element: AXUIElement) -> Option<AXUIElement> {
    for _ in 0..MAX_WINDOW_ANCESTOR_DEPTH {
        let role = element
            .string_attribute(ax_attr::AX_ROLE_ATTRIBUTE)
            .ok()
            .flatten();
        if role.as_deref() == Some(AX_WINDOW_ROLE) {
            return Some(element);
        }
        element = element.element_attribute(ax_attr::AX_PARENT_ATTRIBUTE).ok().flatten()?;
    }
    None
}

impl ClickExecutor {
    /// ⭐(이슈 #133 판정 조건 4) — 창 제목 후보의 확정 동작: **창 전면화**.
    ///
    /// 순서(설계 결정 — "AXRaise 우선, 실패 시 앱 수준 폴백"):
    /// ① AX 경로 — `AXUIElementCreateApplication`(`AXUIElement::from_pid`) →
    ///    `kAXWindowsAttribute` 창 목록에서 `kAXWindowNumberAttribute` 값이
    ///    `target.window_id` 와 일치하는 창을 찾아 `kAXRaiseAction`. 창 수 상한
    ///    ([`MAX_AX_WINDOW_ITERATION`])으로 병적 앱을 방어하고, 모든 AX 요소에
    ///    [`AX_TIMEOUT_SECS`] 를 건다.
    /// ② 앱 활성화 — `activate_application`(메인 디스패치 + ActivateIgnoringOtherApps)
    ///    을 **어느 경로든 정확히 1회** 호출한다: ①은 창을 앞으로만 끌어오고
    ///    키보드 포커스는 따라오지 않는 macOS 동작이 있어 보완이 필요하고
    ///    (`focus_window_at` 의 관례), ①이 온전히 실패했을 때는 그 호출 자체가
    ///    **앱 수준 폴백**이 된다 — 할 수 있는 전부가 앱 활성화뿐이기 때문이다.
    /// 좌표 클릭은 합성하지 않는다 — 이 경로는 "그 창을 앞으로"만 한다.
    ///
    /// `window_id`·`pid` 중 하나라도 `None` 이면(구조상 불가 — 생성자가 보장)
    /// 로그만 남기고 `Ok(())` 를 돌려준다(조용한 no-op). 단계별 실패도 크래시
    /// 없이 로그 + 앱 활성화 폴백으로 흡수한다.
    fn front_window(&self, target: &ConfirmedMatch) -> Result<(), ClickError> {
        let Some(window_id) = target.window_id else {
            tracing::warn!("FrontWindow confirm has no window_id; treating as a no-op");
            return Ok(());
        };
        let Some(pid) = target.pid else {
            tracing::warn!("FrontWindow confirm has no pid; treating as a no-op");
            return Ok(());
        };

        let ax_raised = self.raise_window_via_ax(window_id, pid);

        // ② — AX 성공 여부와 무관한 독립 1회(위 메서드 문서: 보완이자 폴백).
        if !activate_application(&self.app, pid) {
            tracing::warn!(
                pid,
                "target application could not be activated after window fronting"
            );
        }

        if ax_raised {
            tracing::debug!(window_id, pid, "fronted the target window (AX raise + app activation)");
        } else {
            tracing::warn!(window_id, pid, "AX window raise failed; only app-level activation was applied");
        }
        Ok(())
    }

    /// AX 경로의 창 전면화 — 성공하면 `true`. 실패(요소 생성 실패·`AXWindows`
    /// 읽기 실패·창 수 상한 초과·창 미매칭·raise 실패)는 단계마다 로그만 남기고
    /// `false` 를 돌려준다 — 호출자가 앱 수준 활성화 폴백으로 흡수한다.
    fn raise_window_via_ax(&self, window_id: u32, pid: i32) -> bool {
        let Some(app) = AXUIElement::from_pid(pid) else {
            tracing::warn!(pid, "failed to create AXUIElement for pid; falling back to app activation");
            return false;
        };
        let _ = app.set_timeout(AX_TIMEOUT_SECS);

        let windows = match app.attribute(ax_attr::AX_WINDOWS_ATTRIBUTE).ok().flatten() {
            Some(value) => value.as_array().unwrap_or_default(),
            None => {
                tracing::warn!(pid, "failed to read AXWindows; falling back to app activation");
                return false;
            }
        };
        if windows.len() > MAX_AX_WINDOW_ITERATION {
            tracing::warn!(
                window_id,
                pid,
                count = windows.len(),
                "AXWindows count exceeds the iteration cap; falling back to app activation"
            );
            return false;
        }

        // `kAXWindowNumberAttribute` — axuielement 0.9.1 의 ax_attribute 상수에
        // 없고, 공개 SDK(AXAttributeConstants.h 의 kAX* 상수)에도 선언이 없는
        // 미문서(비공개) 속성이다. 값은 "AXWindowNumber" 이며 읽기는 public API
        // 경로와 같다(문자열 리터럴).
        const AX_WINDOW_NUMBER_ATTRIBUTE: &str = "AXWindowNumber";

        for value in windows {
            let Some(window) = value.as_element() else {
                continue;
            };
            let _ = window.set_timeout(AX_TIMEOUT_SECS);
            let Ok(Some(number)) = window.attribute(AX_WINDOW_NUMBER_ATTRIBUTE) else {
                continue;
            };
            if number.as_i64() != Some(i64::from(window_id)) {
                continue;
            }
            match window.perform_action(AX_RAISE_ACTION) {
                Ok(()) => return true,
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        window_id,
                        pid,
                        "kAXRaiseAction failed on the matched window; falling back to app activation"
                    );
                    return false;
                }
            }
        }
        tracing::warn!(
            window_id,
            pid,
            "no matching AXWindow (kAXWindowNumberAttribute); falling back to app activation"
        );
        false
    }

    /// `Focus window before clicking` ON — 대상 창을 활성화한다(명세 §3.7).
    ///
    /// 순서: 클릭 지점 요소 → `AXWindow` 조상(순회 상한 J4) → pid →
    /// `NSRunningApplication.activate`(메인 스레드) + 해당 창 `kAXRaiseAction`
    /// → `isActive` 폴링(20ms × 15, D8). 모든 단계 실패는 클릭을 멈추지 않고
    /// 로그만 남기고 진행한다 — 폴링 상한 초과는 첫 클릭이 활성화에 소모되는
    /// macOS 기본 동작(click-through)으로 자연히 흡수된다(엣지 9).
    fn focus_window_at(&self, element_at_point: &AXUIElement) {
        let Some(window) = find_window_ancestor(element_at_point.clone()) else {
            tracing::debug!("no AXWindow ancestor at the click point; skipping focus activation");
            return;
        };
        // AX 액션은 어느 스레드에서도 호출 가능하다 — 먼저 창을 올린다(§3.7
        // 병행: activate 와 함께 특정 창이 앞에 오도록 보장).
        if let Err(e) = window.perform_action(AX_RAISE_ACTION) {
            tracing::warn!(error = %e, "failed to raise the target window; continuing with click");
        }
        let Ok(pid) = window.pid() else {
            tracing::debug!("failed to get pid from the target window; skipping focus activation");
            return;
        };
        if !activate_application(&self.app, pid) {
            tracing::warn!("target application could not be activated; continuing with click");
            return;
        }
        // --- 지정 좌표로 활성화가 완료될 때까지 폴링. `isActive` 읽기는 메인
        // 디스패치로 감싼다(D8 — 스레드 안전성이 불확실하므로 보수적으로).
        for attempt in 0..FOCUS_POLL_LIMIT {
            if is_application_active(&self.app, pid) {
                return;
            }
            if attempt + 1 < FOCUS_POLL_LIMIT {
                thread::sleep(FOCUS_POLL_INTERVAL);
            }
        }
        let timeout_ms = FOCUS_POLL_INTERVAL.as_millis() * FOCUS_POLL_LIMIT as u128;
        tracing::warn!(
            pid,
            timeout_ms = timeout_ms as u64,
            "focus activation timed out after {timeout_ms} ms; proceeding with click"
        );
    }

    /// 좌표 클릭 합성 — 계획(§2.2 표)을 프리미티브로 실행한다.
    ///
    /// 모드별 순서: `onlyMoveCursor` 는 워프만 / `clickReturnClick` 은 클릭 →
    /// 원위치 워프 → 원위치 재클릭 / `clickAndReturn` 은 클릭 후 원위치 워프 /
    /// `doubleClickCopy`·`tripleClickCopy` 는 클릭 후 ⌘C(D11).
    fn coordinate_execute(&self, plan: &mut ClickPlan) -> Result<(), ClickError> {
        // `clickReturnClick` — 두 번째 클릭은 원래 커서 위치(§3.3 표). 플랜
        // 산출 시점에는 알 수 없어 지금(그 어떤 이동·클릭 전에) 기억한다.
        if plan.second_click_at_origin {
            plan.second_point = click_synthesis::cursor_position().map(|(x, y)| point::Point::new(x, y));
            if plan.second_point.is_none() {
                tracing::debug!("failed to read the current cursor position; reusing the match point for the second click");
            }
        }

        if plan.clicks == 0 {
            // `onlyMoveCursor` — 클릭 이벤트 없이 커서만 이동(§8 수용 기준 6).
            if !click_synthesis::warp_cursor(plan.point.x, plan.point.y) {
                let x = plan.point.x;
                let y = plan.point.y;
                tracing::warn!(x, y, "failed to warp cursor to ({x}, {y}); cursor stays put");
            }
            return Ok(());
        }

        let origin = click_synthesis::cursor_position();
        let mut warped_back = false;
        for (index, &click_state) in plan.click_states.iter().enumerate() {
            if index > 0 && plan.second_click_at_origin {
                // clickReturnClick — 첫 클릭이 끝난 뒤 원위치로 워프하고 그
                // 자리에서 재클릭한다. 워프 실패는 로그만(엣지 12 열화).
                if let Some((ox, oy)) = origin {
                    if !click_synthesis::warp_cursor(ox, oy) {
                        tracing::warn!("failed to warp cursor back; leaving cursor at click point");
                    }
                }
                warped_back = true;
            }

            let (px, py) = if index > 0 && plan.second_click_at_origin {
                plan.second_point.map_or((plan.point.x, plan.point.y), |p| (p.x, p.y))
            } else {
                (plan.point.x, plan.point.y)
            };

            if !click_synthesis::mouse_click(px, py, click_state, plan.flags) {
                return Err(ClickError::Synthesize(format!(
                    "failed to synthesize mouse event at ({px}, {py})"
                )));
            }
            let click_mode = plan.mode;
            let x = plan.point.x;
            let y = plan.point.y;
            tracing::debug!(
                ?click_mode,
                x,
                y,
                click_state,
                "synthesized click: mode={click_mode:?}, point=({x}, {y}), click_state={click_state}"
            );
        }

        // 결정 D11(이슈 #44 — 클론 설계 결정, 미검증) — macOS 더블/트리플클릭은
        // 선택만 만들고 복사는 하지 않으므로, 클립보드 복사를 위해 ⌘C 키 합성을
        // 1 회 보낸다. `SyntheticEvent` 가 자동으로 마커를 심어(A3) 탭 트램폴린
        // 0-a 에서 통과한다.
        if plan.copy_after {
            if let Some(down) = SyntheticEvent::keyboard(KeyCode::ANSI_C, true, EventFlags::COMMAND) {
                down.post();
            }
            if let Some(up) = SyntheticEvent::keyboard(KeyCode::ANSI_C, false, EventFlags::COMMAND) {
                up.post();
            }
        }

        // `clickAndReturn` — 클릭 후 원래 위치로 복귀(이미 워프했다면 중복 없음).
        if plan.warp_back && !warped_back {
            if let Some((ox, oy)) = origin {
                if !click_synthesis::warp_cursor(ox, oy) {
                    tracing::warn!("failed to warp cursor back; leaving cursor at click point");
                }
            }
        }
        Ok(())
    }
}

/// 메인 스레드에 `f` 실행을 부탁하고 결과를 받아온다 — `run_on_main_thread` 는
/// 큐잉만 하고 즉시 반환하므로, 응답은 별도 채널로 기다린다(`seek.rs:138-159`
/// 선례). 실패(핸들 소멸·타임아웃) 시 `None` — 호출자가 열화 경로로 흡수한다.
///
/// ⭐ `recv` 가 아니라 **`recv_timeout(500ms)`**(상급 리뷰 결정, 이슈 #44) —
/// 메인 스레드가 멈춰 있을 때 워커가 영구 블로킹되는 것을 막는다. 인용한
/// 선례(`seek.rs:158`)와 같은 규약이다.
fn run_on_main_thread<T: Send + 'static>(
    app: &tauri::AppHandle,
    f: impl FnOnce() -> T + Send + 'static,
) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    app.run_on_main_thread(move || {
        let _ = tx.send(f());
    })
    .ok()?;
    rx.recv_timeout(Duration::from_millis(500)).ok()
}

/// `NSRunningApplication.activateWithOptions` — 메인 스레드 디스패치 1 회
/// (plan draft §2.3: "activate 자신은 메인 디스패치 1회").
///
/// ⭐ 옵션은 `ActivateIgnoringOtherApps`(상급 리뷰 결정, 이슈 #44) — 옵션 0 은
/// "다른 앱이 활성 중이면 활성화하지 않을 수 있다"는 기본 규칙을 따르므로,
/// 이 설정의 목적(§8 수용 기준 10: 비활성 창이 확정 1회로 활성화+클릭 도달)을
/// 배경 창에서 보장할 수 없다. `ActivateAllWindows` 는 대상 앱의 창을 전부
/// 올리므로 제외 — 필요한 창 하나는 `kAXRaiseAction` 가 이미 올린다(§3.7).
///
/// ⚠️ 이 상수는 macOS 14 에서 deprecated 지만("14+ 에서 효과 없음") 최소 지원이
/// 12.0 이라 의도적으로 쓴다 — 12/13 에서는 배경 창 활성화 보장에 필요하고,
/// 14+에서는 no-op 라 동작 차이가 없다.
#[allow(deprecated)]
fn activate_application(app: &tauri::AppHandle, pid: i32) -> bool {
    run_on_main_thread(app, move || {
        let Some(running) = NSRunningApplication::runningApplicationWithProcessIdentifier(pid) else {
            return false;
        };
        running.activateWithOptions(NSApplicationActivationOptions::ActivateIgnoringOtherApps)
    })
    .unwrap_or(false)
}

/// `NSRunningApplication.isActive` 폴링 1 회 — 스레드 안전성이 불확실하므로
/// (D8) 폴링 조회도 메인 디스패치 채널로 감싼다(왕복 비용은 있지만 폴링의
/// 안전이 우선).
fn is_application_active(app: &tauri::AppHandle, pid: i32) -> bool {
    run_on_main_thread(app, move || {
        NSRunningApplication::runningApplicationWithProcessIdentifier(pid)
            .is_some_and(|running| running.isActive())
    })
    .unwrap_or(false)
}