//! F-01 의 심장 — 세션 생명주기·모드·활성화 경로·키 라우팅 상태 머신.
//!
//! 대응 명세: `docs/spec/seek-activation-and-session.md` §3.2 상태표.
//!
//! ⭐ **명세와의 차이 하나**: 명세 표의 `Cancelled` 와 `Closed` 를 상태로
//! 두지 않고 [`CloseReason`] + `Idle` 복귀로 접었다. 근거 — 표에서
//! `Cancelled` 는 곧바로 `Closed` 로 가는 내부 전이일 뿐이고, 두 상태의
//! 부수효과("오버레이 닫기 · 키 라우팅 복귀")가 **완전히 같다.** 관측
//! 가능한 차이는 "왜 닫혔는가" 하나뿐이라 그것을 이유 값으로 표현하는 편이
//! 로그·테스트 양쪽에서 정확하다.
//!
//! ⭐ 매치 선택·순환·질의 필터링·200개 상한(R-4)은 [`OverlaySession`]
//! 이 이미 갖고 있다 — 이 머신은 그것을 소유(compose)하고 생명주기만 더한다.

use ultrakey_core::event::{EventKind, InputEvent};
use ultrakey_core::flags::EventFlags;
use ultrakey_overlay::{Appearance, OverlayDisplay, OverlaySession};
use ultrakey_seek::TextCandidate;

use crate::config::{ActivationPath, SeekConfig, SessionMode};
use crate::confirm::ConfirmedMatch;
use crate::keys::{classify, SessionKey};

/// 명세 §3.2 상태 머신의 상태.
///
/// [`SeekSessionMachine::state`] 는 세션이 없을 때 항상 `Idle` 을 돌려준다
/// — 내부적으로는 `Option<Session>` 이 `None` 인 상태와 같은 뜻이지만,
/// 호출자가 매번 `Option` 을 벗기지 않도록 이 열거형 자체에 명시적인
/// `Idle` 항목을 둔다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// 세션이 없다.
    Idle,
    /// 세션이 열렸지만 F-02 검출이 아직 끝나지 않았다(S-6).
    Opening,
    /// 검출이 끝났고 쿼리가 비어 있다.
    Ready,
    /// 쿼리가 있고 필터링이 진행 중이다.
    Querying,
    /// 매치 하나가 명시적으로 선택돼 있다.
    Selected,
    /// 확정 처리 중 — F-04 의 완료 콜백을 기다린다.
    Confirming,
}

/// 세션이 닫히는 이유.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloseReason {
    /// `Enter`, 또는 hold 모드에서 선택된 매치를 둔 채 트리거 키를 뗌 → F-04
    /// 위임 완료(§3.2 Confirming 행).
    Confirmed,
    /// `Esc`(§3.2, §9 #3).
    Cancelled,
    /// toggle 모드에서 같은 트리거가 다시 들어옴(§3.3).
    Toggled,
    /// hold 모드에서 매치가 하나도 없는 채로 트리거 키를 뗌(§5 #4).
    ReleasedWithoutMatch,
    /// 세션 중 디스플레이 구성이 바뀜(§5 #6 — 안전한 기본값으로 취소).
    DisplaysChanged,
    /// ⭐(이슈 #93) 다국어 세션에서 검색 바 창이 키 윈도우 자격을 잃음
    /// (`didResignKey` — 예: 사용자가 다른 앱을 클릭). 인풋 박스 모드에만
    /// 적용된다 — 키가 통과되는 동안 다른 앱으로의 문자 누출(암호 필드 오타
    /// 등)을 막기 위해 복원 없이 닫는다(Plan §3 D7, §9 #11). ⚠️ **확정 처리
    /// 중(`Confirming`)에는 닫지 않는다** — F-04 가 클릭 대상 앱을 활성화하며
    /// 낸 `resignKey` 를 취소로 오인하지 않도록.
    Defocused,
}

/// 세션 상태 머신이 산출하는 부수효과. 호출자(F-09 등 앱 계층)가 이 목록을
/// 순서대로 처리한다.
#[derive(Debug, Clone, PartialEq)]
pub enum SessionEffect {
    /// 세션이 열렸다 — 오버레이를 띄우고 F-02 검출을 시작하라.
    ///
    /// ⭐ **검출을 기다리지 않는다**(S-6). 후보 0개 상태로 먼저 뜬다.
    Opened {
        /// 어느 경로로 열렸는가.
        path: ActivationPath,
        /// 어느 모드로 열렸는가.
        mode: SessionMode,
    },
    /// 렌더 모델이 바뀌었다 — 다시 그려라.
    Repaint,
    /// ⭐ F-04 경계. 받은 쪽이 클릭을 끝내면
    /// [`SeekSessionMachine::notify_click_finished`] 를 불러야 세션이
    /// 닫힌다(§3.2 Confirming 행).
    Confirm(ConfirmedMatch),
    /// 세션이 닫혔다 — 오버레이를 숨기고 키 라우팅을 원래 앱으로 되돌려라.
    Closed {
        /// 왜 닫혔는가.
        reason: CloseReason,
    },
}

/// 열린 세션 하나의 내부 상태(비공개).
struct Session {
    state: SessionState,
    path: ActivationPath,
    mode: SessionMode,
    /// 검색어 버퍼. `overlay` 의 질의와 항상 같은 값을 유지한다 — 이 필드는
    /// [`SeekSessionMachine::query`] 가 참조를 돌려주기 위해 별도로 든다
    /// (`OverlaySession::search_bar_frame` 은 매번 새 값을 만들어 내므로
    /// 참조를 빌릴 수 없다).
    query: String,
    overlay: OverlaySession,
    /// ⭐ 전역 단축키로 연 세션에서, **세션을 연 바로 그 누름**의 키 이벤트를
    /// 재입력으로 오인하지 않기 위한 빗장.
    ///
    /// 전역 단축키는 두 경로로 동시에 관측된다 — ① `global-hotkey` 가 등록한
    /// 시스템 핫키(세션을 여는 경로)와 ② 세션이 열린 직후 계층 1 이 소비해
    /// 넘겨 주는 같은 키의 이벤트다. 둘은 **같은 물리 누름 하나**에서 나오므로,
    /// 아무 장치가 없으면 세션이 열리자마자 자기 자신의 키다운을 재입력으로
    /// 읽고 즉시 닫힌다.
    ///
    /// ⛔ **실기 검증에서 정확히 그렇게 깨졌다** — ⌥Space 로 연 세션이 여는
    /// 순간 `reason=Toggled` 로 닫혀 버렸다. 시간 창(N ms)으로 막지 않는다:
    /// 임계값이 또 하나의 `(미확정)` 상수가 되고 느린 기기에서 다시 깨진다.
    /// 대신 **그 키가 한 번 떼어질 때까지** 재입력 판정을 잠근다 — 물리적으로
    /// 결정적이다.
    awaiting_shortcut_release: bool,
}

/// F-01 Seek 활성화·세션 상태 머신.
pub struct SeekSessionMachine {
    config: SeekConfig,
    session: Option<Session>,
}

impl SeekSessionMachine {
    /// 새 머신을 만든다. 시작 상태는 [`SessionState::Idle`](세션 없음).
    #[must_use]
    pub fn new(config: SeekConfig) -> Self {
        Self {
            config,
            session: None,
        }
    }

    /// 설정을 교체한다.
    ///
    /// ⚠️ 열려 있는 세션의 모드는 **바꾸지 않는다** — 세션 도중에 hold ↔
    /// toggle 이 뒤집히면 사용자가 누르고 있는 키의 의미가 중간에 변한다.
    /// 세션의 `mode` 는 활성화 시점에 [`SeekConfig::mode_for`] 로 한 번
    /// 결정돼 세션 안에 고정되고, 이후 `set_config` 는 그 필드를 건드리지
    /// 않는다.
    pub fn set_config(&mut self, config: SeekConfig) {
        self.config = config;
    }

    /// 현재 설정.
    #[must_use]
    pub fn config(&self) -> SeekConfig {
        self.config
    }

    /// 현재 상태. 세션이 없으면 [`SessionState::Idle`].
    #[must_use]
    pub fn state(&self) -> SessionState {
        self.session
            .as_ref()
            .map_or(SessionState::Idle, |s| s.state)
    }

    /// 세션이 열려 있는가.
    #[must_use]
    pub fn is_active(&self) -> bool {
        self.session.is_some()
    }

    /// 현재 세션의 모드. 세션이 없으면 `None`.
    #[must_use]
    pub fn mode(&self) -> Option<SessionMode> {
        self.session.as_ref().map(|s| s.mode)
    }

    /// 현재 세션을 연 경로. 세션이 없으면 `None`.
    #[must_use]
    pub fn path(&self) -> Option<ActivationPath> {
        self.session.as_ref().map(|s| s.path)
    }

    /// 현재 검색어. 세션이 없으면 빈 문자열.
    #[must_use]
    pub fn query(&self) -> &str {
        self.session.as_ref().map_or("", |s| s.query.as_str())
    }

    /// 활성화 시도(§3.2 1~4행, §3.3).
    #[must_use]
    pub fn activate(
        &mut self,
        path: ActivationPath,
        displays: Vec<OverlayDisplay>,
        appearance: Appearance,
        reduce_motion: bool,
        search_bar_origin: (f64, f64),
    ) -> Vec<SessionEffect> {
        match &self.session {
            None => {
                // ⚠️ path 가 그 설정에서 실제로 가능한 경로인지 확인한다 —
                // 예컨대 `RemapKey` 인데 `config.remap_key == None` 이면
                // 열지 않는다(명세 §1 온보딩 함의: 미설정 경로는 발동
                // 수단이 아니다).
                let configured = match path {
                    ActivationPath::GlobalShortcut => self.config.global_shortcut.is_some(),
                    ActivationPath::RemapKey => self.config.remap_key.is_some(),
                    ActivationPath::QuickPressCapsLock => self.config.quick_press_opens,
                };
                if !configured {
                    return Vec::new();
                }

                let mode = self.config.mode_for(path);
                let mut overlay = OverlaySession::open(displays, appearance, reduce_motion);
                overlay.set_search_bar_origin(search_bar_origin.0, search_bar_origin.1);
                // ⭐(이슈 #93) 검색 언어가 명시적 비영어면 이 세션은 인풋 박스
                // 모드다 — 세션 열림 시점에 래칭된다(중간에 설정이 바뀌어도
                // 이 세션의 모드는 유지).
                overlay.set_input_mode(self.config.input_box_mode);

                self.session = Some(Session {
                    state: SessionState::Opening,
                    path,
                    mode,
                    query: String::new(),
                    overlay,
                    // 전역 단축키로 열었으면, 그 누름이 떼어질 때까지 재입력
                    // 판정을 잠근다(위 필드 문서 — 실기 검증이 찾은 결함).
                    awaiting_shortcut_release: path == ActivationPath::GlobalShortcut,
                });

                vec![SessionEffect::Opened { path, mode }, SessionEffect::Repaint]
            }

            Some(session) if session.path == path => match session.mode {
                // §3.2 토글 재입력 행 — 재입력은 닫기로 처리한다.
                SessionMode::Toggle => {
                    self.session = None;
                    vec![SessionEffect::Closed {
                        reason: CloseReason::Toggled,
                    }]
                }
                // §3.3 autorepeat 항 — 새 활성화 시도가 아니라 눌림 유지의
                // 연속이므로 무시한다(상태 불변).
                SessionMode::Hold => Vec::new(),
            },

            // §3.2 마지막 행 · §3.3 — 다른 경로의 트리거는 무시한다.
            Some(_) => Vec::new(),
        }
    }

    /// 트리거 키가 떼졌다. hold 모드의 확정 신호다.
    ///
    /// `modifiers` 는 **키를 떼는 그 순간의 flags** 다 — F-04 명세 §5 #10 이
    /// "hold 모드 확정 시 클릭 모드 판정에 쓰이는 modifier 스냅샷은 리매핑 키를
    /// 떼는 그 순간의 상태를 반영한다" 고 요구한다. 호출자는 `Effect::SeekTriggerUp`
    /// 을 만든 원본 이벤트의 `flags` 를 그대로 넘긴다.
    #[must_use]
    pub fn trigger_released(&mut self, modifiers: EventFlags) -> Vec<SessionEffect> {
        let Some(session) = &mut self.session else {
            return Vec::new();
        };

        match session.mode {
            // toggle 모드에서 키를 떼는 것은 아무 의미가 없다.
            SessionMode::Toggle => Vec::new(),
            SessionMode::Hold => match confirm_selected(session, modifiers) {
                Some(effect) => vec![effect],
                // §5 #4 — 확정할 대상이 없으므로 취소로 처리한다.
                None => {
                    self.session = None;
                    vec![SessionEffect::Closed {
                        reason: CloseReason::ReleasedWithoutMatch,
                    }]
                }
            },
        }
    }

    /// 세션 중 키 하나. `typed` 는 호출자가 레이아웃으로 해석한 문자.
    #[must_use]
    pub fn handle_key(&mut self, ev: &InputEvent, typed: Option<char>) -> Vec<SessionEffect> {
        let Some(session) = &mut self.session else {
            return Vec::new();
        };

        // ⭐ 확정 처리 중에는 상태가 바뀌지 않는다(§3.2 Confirming 행) —
        // F-04 의 완료 콜백을 기다리는 동안 들어오는 키는 전부 버린다.
        if session.state == SessionState::Confirming {
            return Vec::new();
        }

        // ⭐ **전역 단축키 재입력**(§3.2 토글 재입력 행). 세션이 열려 있는 동안에는
        // F-07 의 계층 1 이 모든 키를 소비해 이 조합이 윈도 서버의 핫키 디스패치까지
        // 가지 못하므로(`SeekConfig::global_shortcut` 문서 ②), 여기서 직접 알아본다.
        // ⚠️ `classify` 보다 **먼저** 봐야 한다 — 그러지 않으면 조합의 키가 검색어
        // 문자로 먼저 소비된다(⌥Space 면 공백이 검색어에 들어간다).
        if self.config.matches_global_shortcut(ev) {
            // 세션을 연 그 누름이 아직 떼어지지 않았다 — 이 이벤트는 재입력이
            // 아니라 **같은 누름의 잔향**이다(`awaiting_shortcut_release` 문서).
            if session.awaiting_shortcut_release {
                if ev.kind == EventKind::KeyUp {
                    session.awaiting_shortcut_release = false;
                }
                return Vec::new();
            }
            return match (session.path, session.mode) {
                // 같은 경로 + toggle → 닫는다.
                (ActivationPath::GlobalShortcut, SessionMode::Toggle) => {
                    self.session = None;
                    vec![SessionEffect::Closed { reason: CloseReason::Toggled }]
                }
                // §3.3 — 다른 경로로 열린 세션에 들어온 트리거는 무시한다.
                _ => Vec::new(),
            };
        }

        match classify(ev, typed, self.config.semicolon_cycles) {
            SessionKey::Ignore => Vec::new(),

            // §3.2, §8 — 어떤 상태에서든 클릭 실행 없이 즉시 닫는다.
            SessionKey::Cancel => {
                self.session = None;
                vec![SessionEffect::Closed {
                    reason: CloseReason::Cancelled,
                }]
            }

            SessionKey::Confirm => match confirm_selected(session, ev.flags) {
                Some(effect) => vec![effect],
                // §5 #2 — 선택된 매치가 없으면 무시(상태 불변).
                None => Vec::new(),
            },

            key @ (SessionKey::Next | SessionKey::Prev | SessionKey::CycleSemicolon) => {
                // §3.2 표는 이 전이를 Ready/Querying/Selected 에서만 정의한다
                // — Opening 중에는 아직 순환할 매치 목록이 확정되지 않았다고
                // 보고 무시한다(부수효과 없음).
                if matches!(
                    session.state,
                    SessionState::Ready | SessionState::Querying | SessionState::Selected
                ) {
                    match key {
                        SessionKey::Prev => session.overlay.cycle_prev(),
                        // `;` 순환은 `cycle_next` 와 같다(§3.2 `;` 행).
                        _ => session.overlay.cycle_next(),
                    }
                    // 매치가 1개 이상이면 Selected 로 옮긴다. 0개면
                    // `OverlaySession::cycle_next`/`cycle_prev` 가 이미
                    // 아무 일도 하지 않았으므로(§5 #2·§8 마지막 수용 기준)
                    // 여기서도 상태를 유지한다.
                    if session.overlay.selected().is_some() {
                        session.state = SessionState::Selected;
                    }
                }
                vec![SessionEffect::Repaint]
            }

            key @ (SessionKey::Backspace | SessionKey::Text(_)) => {
                match key {
                    SessionKey::Backspace => {
                        // ⚠️ `String::pop` — 유니코드 스칼라 단위. 한글 조합은
                        // 이 크레이트가 다루지 않는다.
                        session.query.pop();
                    }
                    SessionKey::Text(c) => session.query.push(c),
                    _ => unreachable!(),
                }

                if session.state == SessionState::Opening {
                    // ⭐ S-6 의 절반 — 캡처·후보 생성이 아직 안 끝났어도
                    // 입력을 버리지 않고 쿼리 버퍼에 누적한다(§3.2 6행,
                    // §5 #3). `OverlaySession::set_query` 도 지금 걸어
                    // 둔다 — `ingest_display`/`ingest_extra` 가 Opening
                    // 중에 도착해도(S-6 은 검출을 비동기·증분으로 낸다)
                    // 그 즉시 이미 걸린 쿼리로 필터링된 결과가 나와야
                    // 하기 때문이다. `Opening` 상태 자체는 바뀌지 않는다
                    // — 그 전이는 `finish_detection` 의 몫이다.
                    session.overlay.set_query(&session.query);
                } else {
                    // Ready/Querying/Selected — 질의를 갱신한다.
                    //
                    // ⚠️ `OverlaySession::set_query` 는 선택 인덱스를 0 으로
                    // 되돌린다 — 그래서 `Selected` 였어도 여기서
                    // `Querying` 으로 돌아간다.
                    session.overlay.set_query(&session.query);
                    session.state = SessionState::Querying;
                }
                vec![SessionEffect::Repaint]
            }
        }
    }

    /// ⭐(이슈 #93) 다국어(인풋 박스) 세션에서 **웹뷰 `<input>` 이 보낸 조합된
    /// 쿼리**를 반영한다 — 검색어 편집이 Rust 가 아니라 macOS IME 가 소유하기
    /// 때문에, `handle_key`(문자 하나씩) 대신 **완성된 문자열**로 들어온다
    /// (Plan §3 D5).
    ///
    /// `handle_key` 의 `Text`/`Backspace` 분기와 같은 효과를 낸다:
    /// - `Opening` 중이면 쿼리 버퍼에 누적하고 `overlay.set_query` 만 걸어
    ///   둔다 — 검출(특히 한국어 OCR 약 3.1s)이 끝나 `ingest_display` 가
    ///   도착하면 그 쿼리로 즉시 필터링된다(`machine.rs` §5 #3 버퍼링과 동일).
    /// - 그 뒤면 `overlay.set_query` 후 `Querying` 으로 전이한다(빈 쿼리여도
    ///   `handle_key` 의 Backspace 분기와 같은 결 — 상태 유일성 유지).
    /// - `Confirming` 중에는 아무것도 바꾸지 않는다(확정 처리 중 입력 무시
    ///   계약, `handle_key` 와 동일).
    ///
    /// 검출 재실행·재캡처는 유발하지 **않는다** — 쿼리는 세션 시작에 캡처된
    /// 후보의 필터일 뿐이다(Plan §2 사실 6·8).
    #[must_use]
    pub fn set_query_external(&mut self, query: String) -> Vec<SessionEffect> {
        let Some(session) = &mut self.session else {
            return Vec::new();
        };
        if session.state == SessionState::Confirming {
            return Vec::new();
        }
        session.query = query;
        if session.state == SessionState::Opening {
            session.overlay.set_query(&session.query);
        } else {
            session.overlay.set_query(&session.query);
            session.state = SessionState::Querying;
        }
        vec![SessionEffect::Repaint]
    }

    /// ⭐(이슈 #93) 다국어 세션에서 검색 바 창이 키 윈도우 자격을 잃었다 —
    /// `didResignKey`(타 앱 클릭). 인풋 박스 모드의 문자 통과가 다른 앱으로
    /// 누출되는 것을 막기 위해 복원 없이 세션을 닫는다(Plan §3 D7, §9 #11).
    ///
    /// ⚠️ **`Confirming` 상태에서는 닫지 않는다** — F-04 가 클릭 대상 앱을
    /// 활성화하며 낸 `resignKey` 를 취소로 오인하는 것을 막는 비대칭 처리.
    #[must_use]
    pub fn defocused(&mut self) -> Vec<SessionEffect> {
        let Some(session) = &self.session else {
            return Vec::new();
        };
        if session.state == SessionState::Confirming {
            return Vec::new();
        }
        self.session = None;
        vec![SessionEffect::Closed {
            reason: CloseReason::Defocused,
        }]
    }

    /// F-02 `on_display` 증분 콜백의 소비자(S-1·S-6).
    #[must_use]
    pub fn ingest_display(
        &mut self,
        display_id: u32,
        candidates: Vec<TextCandidate>,
    ) -> Vec<SessionEffect> {
        let Some(session) = &mut self.session else {
            return Vec::new();
        };
        session.overlay.ingest_display(display_id, candidates);
        // ⭐ `OverlaySession::set_query` 로 이미 걸어 둔 쿼리가 새로 도착한
        // 후보에도 자동 적용된다 — `ingest_display` 내부에서
        // `recompute_matching` 이 현재 질의로 다시 필터링하기 때문이다.
        vec![SessionEffect::Repaint]
    }

    /// AX 등 디스플레이가 특정되지 않는 후보의 소비자.
    #[must_use]
    pub fn ingest_extra(&mut self, candidates: Vec<TextCandidate>) -> Vec<SessionEffect> {
        let Some(session) = &mut self.session else {
            return Vec::new();
        };
        session.overlay.ingest_extra(candidates);
        vec![SessionEffect::Repaint]
    }

    /// F-02 검출이 완전히 끝났다.
    #[must_use]
    pub fn finish_detection(&mut self) -> Vec<SessionEffect> {
        let Some(session) = &mut self.session else {
            return Vec::new();
        };
        session.overlay.finish_detection();

        // Opening 에서만 Ready/Querying 으로 넘어간다. 이미 그 뒤(Selected·
        // Querying·Confirming)로 진행해 있었다면 여기서 되돌리지 않는다 —
        // 특히 `Confirming` 을 덮어써서 확정 처리 중인 세션을 되살리면
        // 안 된다.
        if session.state == SessionState::Opening {
            session.state = if session.query.is_empty() {
                SessionState::Ready
            } else {
                SessionState::Querying
            };
        }
        vec![SessionEffect::Repaint]
    }

    /// 핫플러그(§5 #6). 안전한 기본값으로 세션을 취소한다 — 좌표계가 더
    /// 이상 유효하지 않을 수 있으므로 재계산을 시도하지 않는다.
    #[must_use]
    pub fn displays_changed(&mut self) -> Vec<SessionEffect> {
        if self.session.is_some() {
            self.session = None;
            vec![SessionEffect::Closed {
                reason: CloseReason::DisplaysChanged,
            }]
        } else {
            Vec::new()
        }
    }

    /// F-04 가 클릭을 끝냈다 — 이제 세션을 닫는다(§3.2 Confirming 행).
    #[must_use]
    pub fn notify_click_finished(&mut self) -> Vec<SessionEffect> {
        let Some(session) = &self.session else {
            return Vec::new();
        };
        if session.state == SessionState::Confirming {
            self.session = None;
            vec![SessionEffect::Closed {
                reason: CloseReason::Confirmed,
            }]
        } else {
            Vec::new()
        }
    }

    /// 외부 취소(앱 종료, 권한 회수 등).
    #[must_use]
    pub fn cancel(&mut self) -> Vec<SessionEffect> {
        if self.session.is_some() {
            self.session = None;
            vec![SessionEffect::Closed {
                reason: CloseReason::Cancelled,
            }]
        } else {
            Vec::new()
        }
    }

    /// 렌더링용 — 앱이 `frames()`/`search_bar_frame()` 을 읽는다.
    #[must_use]
    pub fn overlay(&self) -> Option<&OverlaySession> {
        self.session.as_ref().map(|s| &s.overlay)
    }

    /// 렌더링용(가변) — 오버레이 자체를 직접 조작해야 하는 드문 경우(예:
    /// 검색 바 드래그 위치 반영)를 위해 남겨 둔다.
    pub fn overlay_mut(&mut self) -> Option<&mut OverlaySession> {
        self.session.as_mut().map(|s| &mut s.overlay)
    }
}

/// 현재 선택된 매치를 확정한다 — `Enter`(toggle)와 hold 모드 릴리즈가
/// 공유하는 경로.
///
/// ⭐ `OverlaySession` 은 선택 인덱스를 0 으로 두므로 매치가 있으면 항상 첫
/// 매치가 선택돼 있다 — 이것이 명세 §3.4 가 번들 문자열 `highlightFirst`
/// 로 시사하고 §9 #4 로 남긴 자리에 대한 **이 구현의 답**이다. F-03 이
/// 이미 그렇게 만들었고(선택 없는 상태가 존재하지 않는다), 사용자가
/// `Enter` 를 눌렀는데 "아무것도 선택 안 됨"으로 아무 일도 안 일어나는
/// 편보다 첫 매치를 확정하는 편이 명세 §2 시나리오 A 의 서술("원하는
/// 매치가 선택된 상태에서 Enter")에 가깝다.
///
/// 선택된 매치가 없으면(§5 #2 — 후보 0개) `None` 을 반환하고 아무것도
/// 바꾸지 않는다.
///
/// `modifiers` 는 **확정을 일으킨 그 이벤트의 flags** 다(F-04 §5 #10) — `Enter`
/// 경로는 그 `Enter` 의 flags, hold 릴리즈 경로는 트리거 키를 뗀 이벤트의 flags.
fn confirm_selected(session: &mut Session, modifiers: EventFlags) -> Option<SessionEffect> {
    let candidate = session.overlay.selected().cloned()?;
    let match_index = session
        .overlay
        .search_bar_frame()
        .selected_index
        .unwrap_or(0);

    let confirmed = ConfirmedMatch {
        text: candidate.text,
        frame: candidate.frame,
        source: candidate.source,
        display_id: candidate.display_id,
        query: session.query.clone(),
        match_index,
        modifiers,
    };

    session.state = SessionState::Confirming;
    Some(SessionEffect::Confirm(confirmed))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ultrakey_core::event::EventKind;
    use ultrakey_core::flags::EventFlags;
    use ultrakey_core::keycode::KeyCode;
    use ultrakey_seek::Rect;

    fn display(id: u32) -> OverlayDisplay {
        OverlayDisplay {
            display_id: id,
            frame: Rect {
                x: 0.0,
                y: 0.0,
                width: 1000.0,
                height: 1000.0,
            },
            backing_scale: 2.0,
        }
    }

    fn displays() -> Vec<OverlayDisplay> {
        vec![display(1)]
    }

    fn candidate(text: &str, x: f64) -> TextCandidate {
        TextCandidate::ocr(
            text.to_string(),
            Rect {
                x,
                y: 0.0,
                width: 10.0,
                height: 10.0,
            },
            0.9,
            1,
        )
    }

    fn key_down(keycode: KeyCode, flags: EventFlags) -> InputEvent {
        InputEvent {
            kind: EventKind::KeyDown,
            keycode,
            flags,
            autorepeat: false,
        }
    }

    /// caps lock 릴리즈를 실제 macOS 이벤트 모양으로 만든다 — `EventKind::FlagsChanged`
    /// 종류에 `KeyCode::CAPS_LOCK` 키코드, 비트가 꺼진 flags(뗌)를 실은
    /// 이벤트다. 이 머신은 이 이벤트를 직접 소비하지 않는다
    /// (`trigger_released()` 가 그 환원 이후를 대표한다) — 하지만 헬퍼로
    /// 남겨 실제 이벤트 모양을 문서화한다.
    #[allow(dead_code)]
    fn caps_lock_flags_changed(pressed: bool) -> InputEvent {
        InputEvent {
            kind: EventKind::FlagsChanged,
            keycode: KeyCode::CAPS_LOCK,
            flags: if pressed {
                EventFlags::CAPS_LOCK
            } else {
                EventFlags::NONE
            },
            autorepeat: false,
        }
    }

    /// ⌥Space 를 `Toggle Seek with shortcut:` 로 둔 설정.
    fn global_shortcut_config() -> SeekConfig {
        SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        }
    }

    fn caps_lock_remap_config(execute_on_close: bool) -> SeekConfig {
        SeekConfig {
            remap_key: Some(KeyCode::CAPS_LOCK),
            execute_on_close,
            ..SeekConfig::default()
        }
    }

    // ── 활성화 3경로(§3.2 1~4행, §1) ─────────────────────────────────────

    /// 세 경로 각각이 설정돼 있으면 세션을 연다.
    #[test]
    fn each_of_three_paths_opens_session_when_configured() {
        for (config, path) in [
            (
                SeekConfig {
                    global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
                    ..SeekConfig::default()
                },
                ActivationPath::GlobalShortcut,
            ),
            (caps_lock_remap_config(false), ActivationPath::RemapKey),
            (
                SeekConfig {
                    quick_press_opens: true,
                    ..SeekConfig::default()
                },
                ActivationPath::QuickPressCapsLock,
            ),
        ] {
            let mut machine = SeekSessionMachine::new(config);
            let effects = machine.activate(path, displays(), Appearance::Light, false, (0.0, 0.0));
            assert!(machine.is_active(), "{path:?} 는 열려야 한다");
            assert_eq!(effects.len(), 2);
            assert!(matches!(effects[0], SessionEffect::Opened { .. }));
            assert_eq!(effects[1], SessionEffect::Repaint);
        }
    }

    /// ⭐ 명세 §1 — 출고 기본값(전부 미설정)에서는 세 경로 중 어느 것도
    /// 세션을 열지 못한다.
    #[test]
    fn no_path_opens_session_when_unconfigured() {
        for path in [
            ActivationPath::GlobalShortcut,
            ActivationPath::RemapKey,
            ActivationPath::QuickPressCapsLock,
        ] {
            let mut machine = SeekSessionMachine::new(SeekConfig::default());
            let effects = machine.activate(path, displays(), Appearance::Light, false, (0.0, 0.0));
            assert!(
                effects.is_empty(),
                "{path:?} 는 미설정이면 열리지 않아야 한다"
            );
            assert!(!machine.is_active());
        }
    }

    // ── mode_for 3경로 × execute_on_close(§3.2, §4) — machine 을 통해서도 확인 ──

    #[test]
    fn activate_records_mode_from_config_mode_for() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert_eq!(machine.mode(), Some(SessionMode::Toggle));

        let mut machine = SeekSessionMachine::new(caps_lock_remap_config(false));
        let _ = machine.activate(
            ActivationPath::RemapKey,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert_eq!(machine.mode(), Some(SessionMode::Toggle));

        let mut machine = SeekSessionMachine::new(caps_lock_remap_config(true));
        let _ = machine.activate(
            ActivationPath::RemapKey,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert_eq!(machine.mode(), Some(SessionMode::Hold));

        let mut machine = SeekSessionMachine::new(SeekConfig {
            quick_press_opens: true,
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::QuickPressCapsLock,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert_eq!(machine.mode(), Some(SessionMode::Toggle));
    }

    // ── 재진입·중복 활성화(§3.3) ─────────────────────────────────────────

    /// toggle 재입력이 닫는다.
    #[test]
    fn toggle_reentry_closes_session() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert!(machine.is_active());

        let effects = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert_eq!(
            effects,
            vec![SessionEffect::Closed {
                reason: CloseReason::Toggled
            }]
        );
        assert!(!machine.is_active());
    }

    /// hold 재입력(autorepeat)은 무시 — 상태 유지.
    #[test]
    fn hold_reentry_is_ignored() {
        let mut machine = SeekSessionMachine::new(caps_lock_remap_config(true));
        let _ = machine.activate(
            ActivationPath::RemapKey,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert!(machine.is_active());

        let effects = machine.activate(
            ActivationPath::RemapKey,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert!(effects.is_empty());
        assert!(machine.is_active());
        assert_eq!(machine.mode(), Some(SessionMode::Hold));
    }

    /// 다른 경로의 트리거는 무시된다.
    #[test]
    fn different_path_trigger_is_ignored() {
        let mut config = caps_lock_remap_config(false);
        config.global_shortcut = Some((KeyCode::SPACE, EventFlags::ALTERNATE));
        let mut machine = SeekSessionMachine::new(config);
        let _ = machine.activate(
            ActivationPath::RemapKey,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert_eq!(machine.path(), Some(ActivationPath::RemapKey));

        let effects = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert!(effects.is_empty());
        assert_eq!(
            machine.path(),
            Some(ActivationPath::RemapKey),
            "세션이 바뀌면 안 된다"
        );
    }

    // ── Opening 중 타이핑 유실 방지(S-6, §5 #3) ─────────────────────────

    /// ⭐ Opening 중 타이핑이 유실되지 않는다 — activate → 문자 3개 →
    /// ingest_display → 그 쿼리로 필터링된 결과가 나온다.
    #[test]
    fn typing_during_opening_is_not_lost() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert_eq!(machine.state(), SessionState::Opening);

        for c in ['s', 'e', 't'] {
            let ev = key_down(KeyCode(0), EventFlags::NONE); // keycode 는 무관, typed 만 본다.
            let _ = machine.handle_key(&ev, Some(c));
        }
        assert_eq!(machine.query(), "set");
        assert_eq!(
            machine.state(),
            SessionState::Opening,
            "검출이 끝나기 전에는 Opening 을 유지한다"
        );

        let _ = machine.ingest_display(
            1,
            vec![candidate("Settings", 0.0), candidate("Cancel", 100.0)],
        );
        let bar = machine.overlay().unwrap().search_bar_frame();
        assert_eq!(
            bar.total_matches, 1,
            "누적된 쿼리 'set' 으로 즉시 필터링돼야 한다"
        );
        assert_eq!(bar.matches[0].text, "Settings");

        let effects = machine.finish_detection();
        assert_eq!(effects, vec![SessionEffect::Repaint]);
        assert_eq!(
            machine.state(),
            SessionState::Querying,
            "쿼리가 비어 있지 않으므로 Querying"
        );
    }

    /// ⭐ 도착 중 타이핑 — 쿼리를 먼저 걸고 후보를 두 번에 나눠 넣으면,
    /// 두 번째 배치에도 같은 쿼리 필터가 적용된다.
    #[test]
    fn query_filter_applies_to_candidates_arriving_in_two_batches() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let _ = machine.handle_key(&key_down(KeyCode(0), EventFlags::NONE), Some('r'));
        let _ = machine.handle_key(&key_down(KeyCode(0), EventFlags::NONE), Some('o'));
        let _ = machine.handle_key(&key_down(KeyCode(0), EventFlags::NONE), Some('w'));
        assert_eq!(machine.query(), "row");

        let _ = machine.ingest_display(1, vec![candidate("Row A", 0.0)]);
        assert_eq!(
            machine.overlay().unwrap().search_bar_frame().total_matches,
            1
        );

        // 두 번째 배치 — 같은 display_id 는 교체가 아니라 ingest_extra 로
        // 다른 소스를 흉내 낸다(디스플레이별 ingest_display 는 재검출
        // 대비로 교체 의미이므로, "나뉘어 도착"을 더 명확히 보이려면
        // ingest_extra 를 함께 쓴다).
        let _ = machine.ingest_extra(vec![candidate("Row B", 200.0)]);
        let bar = machine.overlay().unwrap().search_bar_frame();
        assert_eq!(
            bar.total_matches, 2,
            "두 번째 배치에도 같은 쿼리 필터가 적용돼야 한다"
        );
        assert!(bar.matches.iter().all(|m| m.text.starts_with("Row")));
    }

    /// ⭐ 후보 0개로 즉시 열린다 — S-6.
    #[test]
    fn opens_immediately_with_zero_candidates_while_detecting() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let overlay = machine.overlay().unwrap();
        assert!(overlay.is_detecting());
        assert_eq!(overlay.search_bar_frame().total_matches, 0);
    }

    // ── 순환(↑↓Tab⇧Tab`;`) ────────────────────────────────────────────

    fn open_ready_with_matches(machine: &mut SeekSessionMachine, semicolon_cycles: bool) {
        let config = SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            semicolon_cycles,
            ..SeekConfig::default()
        };
        *machine = SeekSessionMachine::new(config);
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let _ = machine.ingest_display(
            1,
            vec![
                candidate("Row A", 0.0),
                candidate("Row B", 100.0),
                candidate("Row C", 200.0),
            ],
        );
        let _ = machine.finish_detection();
        // 빈 질의로는 매치가 0개다(F-03 "빈 질의 = 매치 없음") — 순환
        // 테스트를 위해 전부 매치되는 질의를 건다.
        for c in ['r', 'o', 'w'] {
            let _ = machine.handle_key(&key_down(KeyCode(0), EventFlags::NONE), Some(c));
        }
    }

    #[test]
    fn arrows_tab_shift_tab_semicolon_cycle_with_wrap() {
        let mut machine = SeekSessionMachine::new(SeekConfig::default());
        open_ready_with_matches(&mut machine, true);
        assert_eq!(machine.overlay().unwrap().selected().unwrap().text, "Row A");

        let _ = machine.handle_key(&key_down(KeyCode::DOWN_ARROW, EventFlags::NONE), None);
        assert_eq!(machine.overlay().unwrap().selected().unwrap().text, "Row B");
        assert_eq!(machine.state(), SessionState::Selected);

        let _ = machine.handle_key(&key_down(KeyCode::TAB, EventFlags::NONE), Some('\t'));
        assert_eq!(machine.overlay().unwrap().selected().unwrap().text, "Row C");

        // 끝에서 되감는다(wrap).
        let _ = machine.handle_key(&key_down(KeyCode::TAB, EventFlags::NONE), Some('\t'));
        assert_eq!(machine.overlay().unwrap().selected().unwrap().text, "Row A");

        // ⇧Tab 은 역방향 — 처음에서 끝으로 되감는다.
        let _ = machine.handle_key(&key_down(KeyCode::TAB, EventFlags::SHIFT), Some('\t'));
        assert_eq!(machine.overlay().unwrap().selected().unwrap().text, "Row C");

        let _ = machine.handle_key(&key_down(KeyCode::UP_ARROW, EventFlags::NONE), None);
        assert_eq!(machine.overlay().unwrap().selected().unwrap().text, "Row B");

        let _ = machine.handle_key(
            &key_down(KeyCode::ANSI_SEMICOLON, EventFlags::NONE),
            Some(';'),
        );
        assert_eq!(machine.overlay().unwrap().selected().unwrap().text, "Row C");
    }

    /// `;` 는 설정이 꺼져 있으면 문자로 들어간다(쿼리가 늘어난다).
    #[test]
    fn semicolon_is_query_char_when_setting_disabled() {
        let mut machine = SeekSessionMachine::new(SeekConfig::default());
        open_ready_with_matches(&mut machine, false);
        let before = machine.query().to_string();

        let _ = machine.handle_key(
            &key_down(KeyCode::ANSI_SEMICOLON, EventFlags::NONE),
            Some(';'),
        );
        assert_eq!(machine.query(), format!("{before};"));
    }

    // ── 후보 0개에서의 무해성(§5 #2, §8 마지막 항) ───────────────────────

    #[test]
    fn all_navigation_keys_are_harmless_with_zero_candidates() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            semicolon_cycles: true,
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let _ = machine.finish_detection();
        assert_eq!(machine.state(), SessionState::Ready);

        for (keycode, flags, typed) in [
            (KeyCode::UP_ARROW, EventFlags::NONE, None),
            (KeyCode::DOWN_ARROW, EventFlags::NONE, None),
            (KeyCode::TAB, EventFlags::NONE, Some('\t')),
            (KeyCode::TAB, EventFlags::SHIFT, Some('\t')),
            (KeyCode::ANSI_SEMICOLON, EventFlags::NONE, Some(';')),
            (KeyCode::RETURN, EventFlags::NONE, Some('\r')),
        ] {
            let effects = machine.handle_key(&key_down(keycode, flags), typed);
            assert!(machine.is_active(), "세션이 유지돼야 한다");
            assert!(
                effects.iter().all(|e| !matches!(
                    e,
                    SessionEffect::Confirm(_) | SessionEffect::Closed { .. }
                )),
                "후보 0개에서는 확정도 종료도 일어나면 안 된다: {effects:?}"
            );
        }
    }

    // ── Enter 확정 → notify_click_finished(§3.2, §8) ────────────────────

    #[test]
    fn enter_confirms_then_notify_click_finished_closes() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let _ = machine.ingest_display(1, vec![candidate("Settings", 0.0)]);
        let _ = machine.finish_detection();
        for c in ['s', 'e', 't'] {
            let _ = machine.handle_key(&key_down(KeyCode(0), EventFlags::NONE), Some(c));
        }
        assert_eq!(
            machine.overlay().unwrap().selected().unwrap().text,
            "Settings"
        );

        // ⭐ F-04 §5 #10 — Enter 경로도 그 `Enter` 이벤트의 flags 를 스냅샷으로
        // 싣는다. ⌥ 를 누른 채 Enter 를 친 상황이다.
        let effects = machine.handle_key(
            &key_down(KeyCode::RETURN, EventFlags::ALTERNATE),
            Some('\r'),
        );
        assert_eq!(effects.len(), 1);
        let SessionEffect::Confirm(confirmed) = &effects[0] else {
            panic!("Confirm 이어야 한다: {effects:?}")
        };
        assert_eq!(confirmed.text, "Settings");
        assert_eq!(confirmed.query, "set");
        assert_eq!(confirmed.match_index, 0);
        assert_eq!(confirmed.modifiers, EventFlags::ALTERNATE);
        assert_eq!(machine.state(), SessionState::Confirming);

        // Confirming 중 추가 키 입력은 전부 무시된다.
        let ignored = machine.handle_key(&key_down(KeyCode::ESCAPE, EventFlags::NONE), None);
        assert!(ignored.is_empty());
        assert!(machine.is_active());

        let closed = machine.notify_click_finished();
        assert_eq!(
            closed,
            vec![SessionEffect::Closed {
                reason: CloseReason::Confirmed
            }]
        );
        assert!(!machine.is_active());
    }

    // ── Esc 가 모든 상태에서 닫는다(§3.2, §8) ────────────────────────────

    #[test]
    fn escape_closes_from_every_reachable_state() {
        // Opening.
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert_eq!(machine.state(), SessionState::Opening);
        assert_eq!(
            machine.handle_key(&key_down(KeyCode::ESCAPE, EventFlags::NONE), None),
            vec![SessionEffect::Closed {
                reason: CloseReason::Cancelled
            }]
        );
        assert!(!machine.is_active());

        // Ready.
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let _ = machine.finish_detection();
        assert_eq!(machine.state(), SessionState::Ready);
        assert!(
            machine
                .handle_key(&key_down(KeyCode::ESCAPE, EventFlags::NONE), None)
                .len()
                == 1
        );
        assert!(!machine.is_active());

        // Querying.
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let _ = machine.finish_detection();
        let _ = machine.handle_key(&key_down(KeyCode(0), EventFlags::NONE), Some('a'));
        assert_eq!(machine.state(), SessionState::Querying);
        assert!(!machine
            .handle_key(&key_down(KeyCode::ESCAPE, EventFlags::NONE), None)
            .is_empty());
        assert!(!machine.is_active());

        // Selected.
        let mut machine = SeekSessionMachine::new(SeekConfig::default());
        open_ready_with_matches(&mut machine, false);
        let _ = machine.handle_key(&key_down(KeyCode::DOWN_ARROW, EventFlags::NONE), None);
        assert_eq!(machine.state(), SessionState::Selected);
        assert!(!machine
            .handle_key(&key_down(KeyCode::ESCAPE, EventFlags::NONE), None)
            .is_empty());
        assert!(!machine.is_active());
    }

    // ── ⭐ 전역 단축키 재입력(§3.2 토글 재입력 행) — 실기 검증이 찾은 결함 ──

    /// ⭐ **회귀 방지 — 실기기에서 실제로 깨져 있던 것이다.**
    ///
    /// 명세 §3.2 는 "toggle 모드에서 동일한 전역 단축키의 재입력은 닫기로
    /// 처리한다" 고 정한다. 그런데 세션이 열려 있는 동안에는 F-07 의 계층 1 이
    /// **모든 키 이벤트를 소비**하므로(§3.1) 그 조합이 윈도 서버의 핫키
    /// 디스패치까지 도달하지 못한다 — `global-hotkey` 는 두 번째 입력을 영영
    /// 보지 못한다. 실기 검증에서 ⌥Space 로 연 세션이 같은 ⌥Space 로 닫히지
    /// 않고 **열린 채 남았다**(그 상태에서는 키보드 전체가 먹통이 된다).
    ///
    /// 그래서 상태 머신이 세션 중 키 라우팅에서 조합을 직접 알아본다.
    #[test]
    fn global_shortcut_re_entry_closes_toggle_session() {
        let mut machine = SeekSessionMachine::new(global_shortcut_config());
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert!(machine.is_active());

        // ⌥Space 를 다시 누른다 — macOS 가 늘 얹는 시스템 비트(0x20000000)와
        // caps lock 잠금 비트가 함께 실려 와도 판정이 성립해야 한다(실측:
        // 릴리즈 flags 가 `0x20010000` 이었다).
        // ⭐ ① 세션을 연 **그 누름**의 키 이벤트가 계층 1 을 통해 되돌아온다 —
        // 이것은 재입력이 아니라 같은 누름의 잔향이므로 무시돼야 한다. 실기
        // 검증에서 이걸 막지 않아 세션이 여는 순간 닫혔다.
        let opening_down = key_down(KeyCode::SPACE, EventFlags::ALTERNATE);
        assert!(machine.handle_key(&opening_down, Some(' ')).is_empty());
        assert!(machine.is_active(), "세션을 연 그 누름이 세션을 닫으면 안 된다");

        // ② 그 키가 떼어진다 — 이제 빗장이 풀린다.
        let opening_up = InputEvent { kind: EventKind::KeyUp, ..opening_down };
        assert!(machine.handle_key(&opening_up, None).is_empty());
        assert!(machine.is_active());

        // ③ 비로소 재입력이다. macOS 가 늘 얹는 시스템 비트(0x20000000)와 caps
        // lock 잠금 비트가 함께 실려 와도 판정이 성립해야 한다(실측: 릴리즈
        // flags 가 `0x20010000` 이었다).
        let ev = key_down(
            KeyCode::SPACE,
            EventFlags(EventFlags::ALTERNATE.0 | 0x2000_0000 | EventFlags::CAPS_LOCK.0),
        );
        let effects = machine.handle_key(&ev, Some(' '));
        assert_eq!(
            effects,
            vec![SessionEffect::Closed { reason: CloseReason::Toggled }]
        );
        assert!(!machine.is_active());
    }

    /// 같은 조합이라도 **다른 경로**로 열린 세션은 닫지 않는다(§3.3 마지막 항).
    #[test]
    fn global_shortcut_re_entry_is_ignored_for_remap_key_session() {
        let mut config = caps_lock_remap_config(true);
        config.global_shortcut = Some((KeyCode::SPACE, EventFlags::ALTERNATE));
        let mut machine = SeekSessionMachine::new(config);
        let _ = machine.activate(
            ActivationPath::RemapKey,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );

        let ev = key_down(KeyCode::SPACE, EventFlags::ALTERNATE);
        assert!(machine.handle_key(&ev, Some(' ')).is_empty());
        assert!(machine.is_active(), "다른 경로의 트리거는 세션을 닫지 않는다");
    }

    /// modifier 가 다르면 그냥 검색어 문자다 — 조합이 아니다.
    #[test]
    fn plain_space_without_modifier_is_query_text_not_a_toggle() {
        let mut machine = SeekSessionMachine::new(global_shortcut_config());
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let ev = key_down(KeyCode::SPACE, EventFlags::NONE);
        let _ = machine.handle_key(&ev, Some(' '));
        assert!(machine.is_active(), "modifier 없는 space 는 닫기가 아니다");
        assert_eq!(machine.query(), " ");
    }

    // ── hold 릴리즈(§3.2, §5 #4, §8) ─────────────────────────────────────

    #[test]
    fn hold_release_with_match_confirms() {
        let mut machine = SeekSessionMachine::new(caps_lock_remap_config(true));
        let _ = machine.activate(
            ActivationPath::RemapKey,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let _ = machine.ingest_display(1, vec![candidate("Error message", 0.0)]);
        let _ = machine.finish_detection();
        for c in ['e', 'r', 'r'] {
            let _ = machine.handle_key(&key_down(KeyCode(0), EventFlags::NONE), Some(c));
        }
        assert!(machine.overlay().unwrap().selected().is_some());

        // ⭐ F-04 §5 #10 — 확정 순간의 modifier 스냅샷이 그대로 실려 나가야 한다.
        // hold 모드에서 그 순간은 "리매핑 키를 떼는 그 순간" 이고, 그때를 아는 것은
        // F-01 뿐이다. 여기서는 ⌘ 을 누른 채 뗀 상황을 만든다.
        let effects = machine.trigger_released(EventFlags::COMMAND);
        assert_eq!(effects.len(), 1);
        let SessionEffect::Confirm(confirmed) = &effects[0] else {
            panic!("hold 릴리즈가 Confirm 을 내지 않았다");
        };
        assert_eq!(confirmed.modifiers, EventFlags::COMMAND);
        assert_eq!(machine.state(), SessionState::Confirming);
    }

    #[test]
    fn hold_release_without_match_closes_with_released_without_match() {
        let mut machine = SeekSessionMachine::new(caps_lock_remap_config(true));
        let _ = machine.activate(
            ActivationPath::RemapKey,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        // 후보가 없다 — Opening 상태에서 바로 뗀다.
        let effects = machine.trigger_released(EventFlags::NONE);
        assert_eq!(
            effects,
            vec![SessionEffect::Closed {
                reason: CloseReason::ReleasedWithoutMatch
            }]
        );
        assert!(!machine.is_active());
    }

    /// toggle 모드에서 릴리즈는 무시된다.
    #[test]
    fn toggle_mode_release_is_ignored() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let effects = machine.trigger_released(EventFlags::NONE);
        assert!(effects.is_empty());
        assert!(machine.is_active());
    }

    // ── modifier 필터링 ──────────────────────────────────────────────────

    #[test]
    fn command_control_option_keys_are_not_added_to_query() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let _ = machine.handle_key(&key_down(KeyCode::ANSI_A, EventFlags::COMMAND), Some('a'));
        let _ = machine.handle_key(&key_down(KeyCode::ANSI_A, EventFlags::CONTROL), Some('a'));
        let _ = machine.handle_key(&key_down(KeyCode::ANSI_A, EventFlags::ALTERNATE), Some('a'));
        assert_eq!(machine.query(), "");
    }

    // ── Backspace(§3.2 Querying 행) ──────────────────────────────────────

    #[test]
    fn backspace_shrinks_query() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let _ = machine.finish_detection();
        for c in ['a', 'b', 'c'] {
            let _ = machine.handle_key(&key_down(KeyCode(0), EventFlags::NONE), Some(c));
        }
        assert_eq!(machine.query(), "abc");
        let _ = machine.handle_key(&key_down(KeyCode::DELETE, EventFlags::NONE), None);
        assert_eq!(machine.query(), "ab");
    }

    // ── 200개 상한(R-4, F-03 §4.2) ───────────────────────────────────────

    /// 후보 300개를 넣어도 `frames()` 의 하이라이트가 상한을 넘지 않는다.
    #[test]
    fn highlight_cap_holds_with_300_candidates() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            vec![OverlayDisplay {
                display_id: 1,
                frame: Rect {
                    x: 0.0,
                    y: 0.0,
                    width: 10_000.0,
                    height: 10.0,
                },
                backing_scale: 2.0,
            }],
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let candidates: Vec<TextCandidate> = (0..300)
            .map(|i| candidate(&format!("Item {i}"), f64::from(i) * 10.0))
            .collect();
        let _ = machine.ingest_display(1, candidates);
        let _ = machine.finish_detection();
        for c in ['i', 't', 'e', 'm'] {
            let _ = machine.handle_key(&key_down(KeyCode(0), EventFlags::NONE), Some(c));
        }

        let frame = &machine.overlay().unwrap().frames()[0];
        assert!(frame.highlights.len() <= ultrakey_overlay::HIGHLIGHT_CAP_PER_DISPLAY + 1);
        assert!(frame.omitted > 0);
    }

    // ── 핫플러그(§5 #6) ──────────────────────────────────────────────────

    #[test]
    fn displays_changed_cancels_session() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let effects = machine.displays_changed();
        assert_eq!(
            effects,
            vec![SessionEffect::Closed {
                reason: CloseReason::DisplaysChanged
            }]
        );
        assert!(!machine.is_active());

        // 세션이 없으면 아무 일도 하지 않는다.
        assert!(machine.displays_changed().is_empty());
    }

    // ── set_config 는 열려 있는 세션의 모드를 바꾸지 않는다 ──────────────

    #[test]
    fn set_config_does_not_change_open_session_mode() {
        let mut machine = SeekSessionMachine::new(caps_lock_remap_config(true));
        let _ = machine.activate(
            ActivationPath::RemapKey,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        assert_eq!(machine.mode(), Some(SessionMode::Hold));

        machine.set_config(caps_lock_remap_config(false));
        assert_eq!(
            machine.mode(),
            Some(SessionMode::Hold),
            "세션 도중 모드가 뒤집히면 안 된다"
        );
        assert!(!machine.config().execute_on_close);
    }

    // ── ⭐(이슈 #93) set_query_external — 다국어(인풋 박스) 세션의 웹뷰 쿼리 ──

    fn open_global(machine: &mut SeekSessionMachine) {
        *machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
    }

    /// T10 — 외부 쿼리("한글")가 필터에 반영되고 Querying 로 전이한다.
    #[test]
    fn set_query_external_filters_and_enters_querying() {
        let mut machine = SeekSessionMachine::new(SeekConfig::default());
        open_global(&mut machine);
        let _ = machine.ingest_display(
            1,
            vec![candidate("안녕하세요", 0.0), candidate("hello", 100.0)],
        );
        let _ = machine.finish_detection();
        assert_eq!(machine.state(), SessionState::Ready);

        let effects = machine.set_query_external("안녕".to_string());
        assert_eq!(effects, vec![SessionEffect::Repaint]);
        assert_eq!(machine.query(), "안녕");
        assert_eq!(machine.state(), SessionState::Querying);
        let bar = machine.overlay().unwrap().search_bar_frame();
        assert_eq!(bar.total_matches, 1, "한글 쿼리로 후보가 필터링돼야 한다");
        assert_eq!(bar.matches[0].text, "안녕하세요");
    }

    /// T10-b(§9 #8) — Opening 중(한국어 OCR 약 3.1s) 도착한 외부 쿼리는 버퍼에
    /// 누적되고, 후보가 배치로 도착하면 즉시 그 쿼리로 필터된다(유실 방지).
    #[test]
    fn set_query_external_during_opening_buffers_and_filters() {
        let mut machine = SeekSessionMachine::new(SeekConfig::default());
        open_global(&mut machine);
        assert_eq!(machine.state(), SessionState::Opening);

        let effects = machine.set_query_external("검색".to_string());
        assert_eq!(effects, vec![SessionEffect::Repaint]);
        assert_eq!(machine.query(), "검색");
        assert_eq!(machine.state(), SessionState::Opening, "검출 전엔 Opening 유지");

        // 후보가 도착하면 누적된 쿼리로 즉시 필터된다(`machine.rs` §5 #3 버퍼링).
        let _ = machine.ingest_display(
            1,
            vec![candidate("검색기록", 0.0), candidate("설정", 100.0)],
        );
        let bar = machine.overlay().unwrap().search_bar_frame();
        assert_eq!(bar.total_matches, 1, "버퍼된 쿼리가 도착 후보에 적용돼야 한다");
        assert_eq!(bar.matches[0].text, "검색기록");
    }

    /// 외부 쿼리를 비우면 Backspace 분기와 같은 결(빈 채 Querying)이 되고 매치
    /// 0 개로 돌아간다.
    #[test]
    fn set_query_external_empty_returns_to_ready() {
        let mut machine = SeekSessionMachine::new(SeekConfig::default());
        open_global(&mut machine);
        let _ = machine.ingest_display(1, vec![candidate("abc", 0.0)]);
        let _ = machine.finish_detection();
        assert_eq!(machine.state(), SessionState::Ready);

        let _ = machine.set_query_external("abc".to_string());
        assert_eq!(machine.state(), SessionState::Querying);

        let effects = machine.set_query_external(String::new());
        assert_eq!(effects, vec![SessionEffect::Repaint]);
        assert!(machine.query().is_empty());
        assert_eq!(machine.state(), SessionState::Querying, "Backspace 분기와 같은 결");
        assert_eq!(
            machine.overlay().unwrap().search_bar_frame().total_matches,
            0
        );
    }

    /// 세션이 없으면 외부 쿼리는 무해하다.
    #[test]
    fn set_query_external_without_session_is_noop() {
        let mut machine = SeekSessionMachine::new(SeekConfig::default());
        assert!(machine.set_query_external("안녕".to_string()).is_empty());
    }

    // ── ⭐(이슈 #93) defocused — D7 키 상실 자동 닫힘 ────────────────────

    /// 세션이 열려 있으면(인풋 박스 모드와 무관하게 판정은 같다) 닫는다.
    #[test]
    fn defocused_closes_active_session() {
        let mut machine = SeekSessionMachine::new(SeekConfig::default());
        open_global(&mut machine);
        assert!(machine.is_active());

        let effects = machine.defocused();
        assert_eq!(
            effects,
            vec![SessionEffect::Closed {
                reason: CloseReason::Defocused
            }]
        );
        assert!(!machine.is_active());

        // 세션이 없으면 no-op.
        assert!(machine.defocused().is_empty());
    }

    /// ⚠️ Confirming(확정 처리 중)에는 닫지 않는다 — F-04 가 대상 앱을
    /// 활성화하며 낸 resignKey 를 취소로 오인하지 않게(Plan §9 #11).
    #[test]
    fn defocused_does_not_close_while_confirming() {
        let mut machine = SeekSessionMachine::new(SeekConfig {
            global_shortcut: Some((KeyCode::SPACE, EventFlags::ALTERNATE)),
            ..SeekConfig::default()
        });
        let _ = machine.activate(
            ActivationPath::GlobalShortcut,
            displays(),
            Appearance::Light,
            false,
            (0.0, 0.0),
        );
        let _ = machine.ingest_display(1, vec![candidate("설정", 0.0)]);
        let _ = machine.finish_detection();
        let _ = machine.set_query_external("설".to_string());
        assert_eq!(machine.overlay().unwrap().search_bar_frame().total_matches, 1);
        let _ = machine.handle_key(&key_down(KeyCode::RETURN, EventFlags::NONE), Some('\r'));
        assert_eq!(machine.state(), SessionState::Confirming);

        assert!(machine.defocused().is_empty(), "Confirming 은 닫으면 안 된다");
        assert!(machine.is_active());
    }
}
