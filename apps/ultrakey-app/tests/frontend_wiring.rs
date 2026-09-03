//! 재발 방지 테스트 — `tauri.conf.json` 과 `ui/index.html` 이 어긋나면 잡는다.
//!
//! ⭐ 이번 버그의 본질: `ui/index.html` 이 `window.__TAURI__` 를 쓰는데
//! `tauri.conf.json` 의 `app.withGlobalTauri` 가 없어서(기본값 `false`)
//! Tauri v2 가 그 전역을 주입하지 않았다. 모듈 평가 시점에 `TypeError` 가 나
//! `render()` 가 한 번도 실행되지 못했고, 정적 HTML 의 `h1`/`p` 는 비어 있고
//! 콘텐츠 `div` 는 전부 `hidden` 이라 웹뷰가 완전히 백지로 보였다. 창 자체는
//! 정상적으로 최상단·포커스 상태였다 — 컴파일러도 기존 테스트도 이 어긋남을
//! 잡지 못했다. 이 파일은 그 어긋남을 정적으로 검사한다.

use std::collections::BTreeSet;
use std::path::Path;

fn manifest_dir() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn read_tauri_conf() -> serde_json::Value {
    let path = manifest_dir().join("tauri.conf.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("tauri.conf.json 을 읽지 못했다({path:?}): {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("tauri.conf.json 파싱 실패({path:?}): {e}"))
}

fn read_index_html() -> String {
    let path = manifest_dir().join("ui/index.html");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("ui/index.html 을 읽지 못했다({path:?}): {e}"))
}

fn read_settings_html() -> String {
    let path = manifest_dir().join("ui/settings.html");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("ui/settings.html 을 읽지 못했다({path:?}): {e}"))
}

/// F-18 Event Viewer(이슈 #39 Phase 3) — 별도 창 HTML.
fn read_eventviewer_html() -> String {
    let path = manifest_dir().join("ui/eventviewer.html");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("ui/eventviewer.html 을 읽지 못했다({path:?}): {e}"))
}

/// ⭐ 이슈 #87 — 독립 About 정보 창 HTML.
fn read_about_html() -> String {
    let path = manifest_dir().join("ui/about.html");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("ui/about.html 을 읽지 못했다({path:?}): {e}"))
}

/// F-03 Seek 검색 바(이슈 #67) — 오버레이 검색 바 HTML.
fn read_searchbar_html() -> String {
    let path = manifest_dir().join("ui/overlay-searchbar.html");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("ui/overlay-searchbar.html 을 읽지 못했다({path:?}): {e}"))
}

fn read_en_catalog() -> serde_json::Value {
    // resources/ 는 이 워크트리에서 다른 세션이 동시에 작업 중이라 손대지 않는다
    // — 여기서는 읽기만 한다. 경로는 워크스페이스 루트 기준(`i18n` 크레이트가
    // `include_str!("../../../resources/i18n/en.json")` 로 참조하는 것과 동일).
    let path = manifest_dir().join("../../resources/i18n/en.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("resources/i18n/en.json 을 읽지 못했다({path:?}): {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("resources/i18n/en.json 파싱 실패({path:?}): {e}"))
}

fn read_ko_catalog() -> serde_json::Value {
    let path = manifest_dir().join("../../resources/i18n/ko.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("resources/i18n/ko.json 을 읽지 못했다({path:?}): {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("resources/i18n/ko.json 파싱 실패({path:?}): {e}"))
}

/// ⭐ 클론이 지원하는 로케일 5종(en·ko·zh·es·ja, `ultrakey-i18n::Locale::all()`
/// 과 같은 집합). 이슈 #39 부터 en·ko 두 개만 보던 몇몇 재발 방지 테스트를 이
/// 목록으로 일반화한다 — en·ko 만 보면 zh·es·ja 쪽에서만 키가 빠지는 회귀를
/// 놓친다.
const LOCALES: &[&str] = &["en", "ko", "zh", "es", "ja"];

/// `resources/i18n/<locale>.json` 을 읽는다. `read_en_catalog`/`read_ko_catalog`
/// 와 같은 근거(resources/ 는 다른 세션이 동시에 작업 중이라 읽기만 한다)로
/// zh·es·ja 도 같은 방식으로 연다.
fn read_catalog(locale: &str) -> serde_json::Value {
    let path = manifest_dir().join(format!("../../resources/i18n/{locale}.json"));
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("resources/i18n/{locale}.json 을 읽지 못했다({path:?}): {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("resources/i18n/{locale}.json 파싱 실패({path:?}): {e}"))
}

fn read_main_rs() -> String {
    let path = manifest_dir().join("src/main.rs");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("src/main.rs 을 읽지 못했다({path:?}): {e}"))
}

/// ⭐ `window.__TAURI__` 를 참조하면서 `withGlobalTauri` 가 꺼져 있으면(또는
/// 아예 없으면) 웹뷰가 백지가 된다 — 실제로 이번에 발생한 사고 그대로다.
/// Tauri v2 는 `app.withGlobalTauri` 가 `true` 일 때만 그 전역을 주입하고,
/// 기본값은 `false` 다(`tauri-codegen/src/context.rs:428`).
#[test]
fn index_html_이_tauri_전역을_쓰면_with_global_tauri_가_켜져_있어야_한다() {
    let html = read_index_html();
    let conf = read_tauri_conf();

    let uses_global_tauri = html.contains("window.__TAURI__");
    if !uses_global_tauri {
        // 더 이상 전역을 쓰지 않는다면 이 검사는 대상이 없다 — 통과.
        return;
    }

    let with_global_tauri = conf["app"]["withGlobalTauri"].as_bool().unwrap_or(false);
    assert!(
        with_global_tauri,
        "ui/index.html 이 window.__TAURI__ 를 참조하지만 tauri.conf.json 의 \
         app.withGlobalTauri 가 true 가 아니다. Tauri v2 는 withGlobalTauri 가 \
         true 일 때만 window.__TAURI__ 를 주입하며 기본값은 false 다 — 이 상태로 \
         빌드하면 모듈 평가 시점에 `window.__TAURI__.core` 가 TypeError 를 던져 \
         render() 가 한 번도 실행되지 못하고 권한 온보딩 모달이 완전히 백지로 뜬다."
    );
}

/// `main.rs` 의 `show_modal`/`hide_modal` 은 `app.get_webview_window(\"permissions\")`
/// 로 창을 찾는다 — `app.windows` 에 그 라벨의 창이 없으면 두 함수 모두
/// 아무 일도 하지 않고 조용히 실패한다(로그만 남는다).
#[test]
fn tauri_conf_에_permissions_라벨_창이_있어야_한다() {
    let conf = read_tauri_conf();
    let windows = conf["app"]["windows"]
        .as_array()
        .expect("tauri.conf.json 의 app.windows 가 배열이 아니다");

    let has_permissions_window = windows
        .iter()
        .any(|w| w["label"].as_str() == Some("permissions"));

    assert!(
        has_permissions_window,
        "tauri.conf.json 의 app.windows 에 label == \"permissions\" 인 창이 없다. \
         main.rs 의 show_modal()/hide_modal() 은 이 라벨로 \
         get_webview_window(\"permissions\") 를 호출하므로, 라벨이 어긋나면 \
         두 함수가 조용히 아무 일도 하지 않는다."
    );
}

/// `body { background: transparent }` 는 `tauri.conf.json` 의 창 설정에
/// `transparent: true` 가 없는 한 아무 효과를 내지 못하고, 렌더가 실패했을 때
/// 창이 "텅 비어 보이는" 실패 모드를 더 나쁘게 만든다.
///
/// ⚠️ `tauri.conf.json` 에 `transparent: true` 가 생기면 이 테스트를 갱신하라
/// — 그때는 투명 배경이 의도된 것이므로 이 검사가 더 이상 유효하지 않다.
#[test]
fn index_html_의_body_는_transparent_배경을_쓰지_않는다() {
    let html = read_index_html();
    let conf = read_tauri_conf();

    let windows = conf["app"]["windows"]
        .as_array()
        .expect("tauri.conf.json 의 app.windows 가 배열이 아니다");
    let any_window_transparent = windows
        .iter()
        .any(|w| w["transparent"].as_bool() == Some(true));

    assert!(
        !any_window_transparent,
        "tauri.conf.json 의 창 설정에 transparent: true 가 생겼다 — \
         ui/index.html 의 body 배경이 이제 투명해도 되는지 다시 검토하고 \
         이 테스트를 갱신하라."
    );

    // 스타일 블록만 대상으로 좁혀서(주석 등에 우연히 같은 문자열이 섞이는 것을
    // 피하려고) `body { ... }` 규칙 안에 `background: transparent` 가 있는지
    // 검사한다.
    let style_start = html
        .find("<style>")
        .expect("index.html 에 <style> 블록이 없다");
    let style_end = html
        .find("</style>")
        .expect("index.html 에 </style> 종료 태그가 없다");
    let style = &html[style_start..style_end];

    let body_start = style
        .find("body {")
        .expect("index.html 의 <style> 안에 body 규칙이 없다");
    let body_rule_end = style[body_start..]
        .find('}')
        .map(|i| body_start + i)
        .expect("index.html 의 body 규칙이 닫히지 않았다");
    let body_rule = &style[body_start..body_rule_end];

    assert!(
        !body_rule.contains("background: transparent")
            && !body_rule.contains("background:transparent"),
        "ui/index.html 의 body 규칙이 background: transparent 를 쓴다. \
         tauri.conf.json 의 창 설정에 transparent: true 가 없으므로 이 설정은 \
         아무 효과가 없고, 렌더가 실패했을 때 창이 텅 비어 보이는 실패 모드를 \
         더 나쁘게 만들 뿐이다 — 시스템 색(Canvas/CanvasText) 을 쓰라."
    );
}

// ⭐ F-09(preferences-ui.md) — 환경설정 창 재발 방지 테스트.
//
// `ui/settings.html` 은 `ui/index.html` 과 같은 방어적 부트스트랩 규약을 따른다:
// `window.__TAURI__` 부재·bootstrap 실패·렌더 예외를 전부 잡아 화면에 진단
// 문구를 낸다. 아래 테스트들은 정적 파일 내용만 검사한다 — 기존 index.html
// 테스트 스타일 그대로다.

/// `<script>...</script>` 블록의 원문을 돌려준다(주석 제거 없음).
fn extract_script_block(html: &str) -> &str {
    let start = html
        .find("<script")
        .expect("settings.html 에 <script> 태그가 없다");
    let tag_end = html[start..]
        .find('>')
        .map(|i| start + i + 1)
        .expect("settings.html 의 <script> 시작 태그가 닫히지 않았다");
    let end = html
        .find("</script>")
        .expect("settings.html 에 </script> 종료 태그가 없다");
    assert!(
        tag_end < end,
        "settings.html 의 <script> 블록 범위 계산이 어긋났다"
    );
    &html[tag_end..end]
}

/// 한 줄짜리 `// …` 주석을 잘라낸다. ⭐ 2026-09-02(이슈 #87) — 큰따옴표 안의
/// `//`(예: URL 검증 코드의 `"https://"`)는 주석이 아니다. 큰따옴표를 번갈아
/// 토글하며 **문자열 바깥**의 `//` 만 잘라낸다(문자열 안 `//` 를 지우면 따옴표
/// 균형이 깨져 그 뒤 모든 리터럴 추출이 어긋난다).
fn strip_line_comments(script: &str) -> String {
    script
        .lines()
        .map(|line| {
            let mut in_string = false;
            let mut split = None;
            let mut chars = line.char_indices().peekable();
            while let Some((i, c)) = chars.next() {
                match c {
                    '"' => in_string = !in_string,
                    '/' if !in_string => {
                        if let Some(&(_, '/')) = chars.peek() {
                            split = Some(i);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            match split {
                Some(idx) => &line[..idx],
                None => line,
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `<!-- ... -->` HTML 주석을 잘라낸다(닫히지 않은 주석은 끝까지 버린다). 이
/// 파일의 정신("문자열을 못 믿고 스스로 방어한다")에 따라, 주석 안의 산문이
/// 마크업 스캔을 오염시키는 것을 막기 위한 최소 도구다(이슈 #47 — `<hr />` 를
/// 주석에 쓴 한국어 주석이 구분선 카운트를 흔들 수 있다).
fn strip_html_comments(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find("<!--") {
        out.push_str(&rest[..start]);
        match rest[start..].find("-->") {
            Some(end) => rest = &rest[start + end + 3..],
            None => rest = "",
        }
    }
    out.push_str(rest);
    out
}

/// 큰따옴표로 감싸인 문자열 리터럴을 전부 뽑는다. 이스케이프된 `\"` 는 이
/// 파일에 등장하지 않으므로 단순한 번갈아-토글 스캔으로 충분하다.
fn extract_double_quoted_literals(text: &str) -> Vec<String> {
    let mut literals = Vec::new();
    let mut chars = text.char_indices().peekable();
    while let Some((start, ch)) = chars.next() {
        if ch != '"' {
            continue;
        }
        let content_start = start + 1;
        let mut end = None;
        for (i, c) in text[content_start..].char_indices() {
            if c == '"' {
                end = Some(content_start + i);
                break;
            }
        }
        if let Some(end) = end {
            literals.push(text[content_start..end].to_string());
            // 닫는 따옴표 다음부터 다시 스캔하도록 이터레이터를 앞으로 돌린다.
            while let Some(&(idx, _)) = chars.peek() {
                if idx <= end {
                    chars.next();
                } else {
                    break;
                }
            }
        }
    }
    literals
}

fn flatten_catalog(value: &serde_json::Value) -> BTreeSet<String> {
    value
        .as_object()
        .expect("en.json 최상위는 객체여야 한다")
        .keys()
        .cloned()
        .collect()
}

/// `settings.html` 이 존재하고, 4개 커맨드 이름을 전부 invoke 한다.
#[test]
fn settings_html_이_존재하고_4개_커맨드를_전부_invoke_한다() {
    // ⭐ F-08/F-10 통합(이슈 #15) — 메뉴바에 `Quit` 이 생겨 설정 창은 더 이상
    // `quit_app` 을 invoke 하지 않는다. 대신 F-08 이 `settings_resolve_conflict`
    // (충돌 대화상자 `계속` 버튼)를 새로 추가했다. `quit_app` 커맨드 자체는
    // main.rs 에 남아 있다(메뉴 `Quit Ultrakey` 가 여전히 쓴다) — 이 목록은
    // "settings.html 이 부르는 커맨드"만의 목록이다.
    let html = read_settings_html();
    for command in [
        "settings_bootstrap",
        "settings_set",
        "settings_set_tab",
        "settings_resolve_conflict",
    ] {
        assert!(
            html.contains(&format!("invoke(\"{command}\"")),
            "ui/settings.html 이 invoke(\"{command}\", …) 를 호출하지 않는다"
        );
    }
}

/// `tauri.conf.json` 에 `settings` 라벨 창이 선언되어 있고 `url` 이 `settings.html` 이다.
#[test]
fn tauri_conf_에_settings_라벨_창이_settings_html_을_가리킨다() {
    let conf = read_tauri_conf();
    let windows = conf["app"]["windows"]
        .as_array()
        .expect("tauri.conf.json 의 app.windows 가 배열이 아니다");

    let settings_window = windows
        .iter()
        .find(|w| w["label"].as_str() == Some("settings"))
        .expect("tauri.conf.json 의 app.windows 에 label == \"settings\" 인 창이 없다");

    assert_eq!(
        settings_window["url"].as_str(),
        Some("settings.html"),
        "settings 창의 url 이 settings.html 이 아니다 — main.rs 의 \
         show_settings_window()/resize_settings_window() 가 get_webview_window(\"settings\") \
         로 찾는 창이 실제로 이 파일을 로드하는지 어긋났다."
    );
}

/// ⭐ `settings.html` 안의 `"settings.*"` 문자열 리터럴은 전부 실재하는 카탈로그
/// 키여야 한다 — 오타나 지어낸 키를 정적으로 잡아낸다. (문자열이 `"settings.`
/// 로 시작하는 채로 큰따옴표에 바로 감싸인 경우만 대상으로 하므로, 한국어 주석
/// 안의 산문("settings.json 파일" 등)은 애초에 이 패턴에 걸리지 않는다.)
#[test]
fn settings_html_의_settings_점_리터럴은_전부_카탈로그_키다() {
    let html = read_settings_html();
    let en = read_en_catalog();
    let known_keys = flatten_catalog(&en);

    let literals = extract_double_quoted_literals(&html);
    let settings_keys: BTreeSet<_> = literals
        .into_iter()
        .filter(|s| s.starts_with("settings."))
        .collect();

    assert!(
        !settings_keys.is_empty(),
        "settings.html 에서 \"settings.*\" 리터럴을 하나도 찾지 못했다 — 추출 로직이 깨졌을 수 있다"
    );

    // ⭐ F-08 충돌 대화상자(`showConflict`)는 `t("settings.presets.conflict.title." +
    // conflict.kind)` 처럼 문자열 결합으로 키를 완성한다(`kind` 가
    // `capsLockAlreadyRemapped` 류 런타임 값이기 때문). 이 추출기는 순수 정적
    // 스캔이라 결합 결과를 알 수 없다 — `.` 로 끝나는 리터럴은 완성된 키가 아니라
    // "결합 접두사"로 보고, 정확한 일치 대신 그 접두사로 시작하는 키가 카탈로그에
    // 하나라도 있는지만 확인한다(오탐 없이 접두사 자체의 오타는 여전히 잡는다).
    let (prefixes, exact): (Vec<_>, Vec<_>) = settings_keys.iter().partition(|k| k.ends_with('.'));

    let missing: Vec<_> = exact
        .iter()
        .filter(|k| !known_keys.contains(k.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "settings.html 이 참조하는 다음 카탈로그 키가 resources/i18n/en.json 에 없다: {missing:?}"
    );

    let missing_prefixes: Vec<_> = prefixes
        .iter()
        .filter(|p| !known_keys.iter().any(|k| k.starts_with(p.as_str())))
        .collect();
    assert!(
        missing_prefixes.is_empty(),
        "settings.html 이 문자열 결합으로 참조하는 다음 접두사로 시작하는 카탈로그 키가 \
         resources/i18n/en.json 에 하나도 없다: {missing_prefixes:?}"
    );
}

/// ⭐ `settings.html` 의 `<script>` 안에 사용자 대면 영어 문장이 하드코딩되어
/// 있지 않다 — 카탈로그 키(`t()`/`tf()` 호출의 인자)나 진단 전용 허용 목록
/// 이외의, 공백을 포함한(=문장 형태) 큰따옴표 문자열 리터럴이 없어야 한다.
///
/// 진단 문구는 index.html 과 같은 이유로 예외다 — 카탈로그를 가져오는 경로
/// 자체가 죽었을 때 쓰는 최후 수단이라 카탈로그에 기댈 수 없다.
#[test]
fn settings_html_에_하드코딩된_영어_문장이_없다() {
    const ALLOWED_DIAGNOSTIC_LITERALS: &[&str] = &[
        "Failed to load: window.__TAURI__ is unavailable (check withGlobalTauri in tauri.conf.json).",
        "Failed to render: ",
        "Failed to save setting: ",
        "Failed to switch tab: ",
        // F-17(Keyboards 탭) — `open_keyboard_settings` invoke 실패 진단 문구.
        // 카탈로그를 거치지 않는 이유는 위 3개와 동일하다(진단 전용, 카탈로그
        // 자체가 죽었을 수도 있는 경로).
        "Failed to open keyboard settings: ",
        // D6(General 탭 언어 선택, 이슈 #39) — `settings_set("general.language",
        // …)` 실패 진단 문구. 위와 같은 이유로 카탈로그를 거치지 않는다(언어를
        // 바꾸다가 실패한 경로라 카탈로그 자체를 못 믿는 상황일 수 있다).
        "Failed to change language: ",
        // F-15 §3.4(이슈 #39 Phase 2) — `settings_export` 실패 진단 문구. 대화상자
        // 취소는 `Ok(None)`(오류가 아니다)이라 여기 걸리지 않는다 — 이 문구는
        // 디스크 I/O 등 드문 실패 전용이라 위임 지시서 A-4 의 카탈로그 키 목록에
        // export 전용 실패 키가 없다(반대로 import 실패는 흔한 사용자 실수라
        // `settings.general.import.failed` 카탈로그 키로 안내 줄에 보여준다).
        "Failed to export settings: ",
        // F-18(이슈 #39 Phase 3) — `open_event_viewer` invoke 실패 진단 문구. 위
        // 항목들과 같은 이유(진단 전용, 카탈로그를 못 믿을 수도 있는 경로).
        "Failed to open the Event Viewer: ",
        // ⭐ 이슈 #47 — `open_log_folder` invoke 실패 진단 문구. 위 항목들과 같은
        // 이유(진단 전용, 카탈로그를 못 믿을 수도 있는 경로). 디렉터리 부재는
        // 커맨드가 조용히 Ok(()) 로 처리하므로 이 문구는 `open` spawn 실패에만
        // 도달한다.
        "Failed to open the log folder: ",
        // ⭐ 이슈 #87 — `open_about_window` invoke 실패 진단 문구. 위 항목들과
        // 같은 이유(진단 전용, 카탈로그를 못 믿을 수도 있는 경로).
        "Failed to open the About window: ",
        // ⭐ 이슈 #46 — `settings_copy_common_to_device` invoke 실패 진단 문구.
        // 위 항목들과 같은 이유(진단 전용, 카탈로그를 못 믿을 수도 있는 경로).
        "Failed to copy settings: ",
        // ⭐ UXR-07 — 탭 페이드의 `prefers-reduced-motion` 미디어 쿼리 문자열.
        // 사용자 대면 문장이 아니라 CSS Media Queries API 인자라 카탈로그를 탈
        // 필요가 없다 — 허용 목록에 없으면 오탐이 나 "난독화"(문자열 분할)를
        // 유도하므로, 리터럴을 그대로 두도록 허용 목록에 근거와 함께 추가한다.
        "(prefers-reduced-motion: reduce)",
    ];

    let html = read_settings_html();
    let script = extract_script_block(&html);
    let script_no_comments = strip_line_comments(script);

    let offenders: Vec<String> = extract_double_quoted_literals(&script_no_comments)
        .into_iter()
        // 공백이 없으면 "문장"이 아니다(카탈로그 키·CSS 클래스명·탭 id 등) —
        // 이 필터만으로 "seek"/"hyperkey"/"settings.foo.bar" 류가 전부 걸러진다.
        .filter(|s| s.contains(' '))
        .filter(|s| !ALLOWED_DIAGNOSTIC_LITERALS.contains(&s.as_str()))
        .collect();

    assert!(
        offenders.is_empty(),
        "settings.html 의 <script> 안에서 허용 목록 밖의 영어 문장 리터럴을 찾았다: \
         {offenders:?} — 카탈로그 키를 통해 t()/tf() 로 가져오거나, 정말 진단 전용 \
         문구라면 ALLOWED_DIAGNOSTIC_LITERALS 에 추가하고 그 근거를 남겨라."
    );
}

/// ⭐ 이슈 #32 Phase 1 — 탭별 창 크기 테이블은 없앴다(원본 SuperKey 는 탭마다
/// 창을 리사이즈하지만, 이 클론은 단일 크기로 고정하고 사용자가 조절한 크기를
/// 영속화하기로 결정했다). `settings.html` 문서 주석이 이 새 사실 — "탭 전환은
/// 더 이상 창 크기를 바꾸지 않는다"와, 기본 크기의 근거(825×821, main.rs
/// `SETTINGS_WINDOW_DEFAULT` 주석이 실측값을 댄다) — 을 정확히 인용하는지
/// 방어적으로 확인한다.
#[test]
fn settings_html_문서가_탭_전환은_창_크기를_바꾸지_않는다고_설명한다() {
    let html = read_settings_html();
    assert!(
        html.contains("탭 전환은") && html.contains("창 크기를 바꾸지 않는다"),
        "settings.html 의 탭 목록 주석이 '탭 전환은 더 이상 창 크기를 바꾸지 \
         않는다'는 사실을 더는 설명하지 않는다 — 이슈 #32 Phase 1 로 동작이 \
         바뀌었으니 문서 주석도 같이 갱신해야 한다."
    );
    assert!(
        html.contains("825×821"),
        "settings.html 문서 주석에 설정 창 기본 크기 825×821(main.rs \
         SETTINGS_WINDOW_DEFAULT 의 실측 근거)이 보이지 않는다."
    );
}

/// `main.rs` 의 `KNOWN_TABS` 화이트리스트가 `korean`·`japanese`·`chinese`·`keyboards` 를
/// 아는지 소스 텍스트로 재확인한다. 실제 판정 로직(`is_known_tab`)의 동작 자체는
/// `src/main.rs` 유닛 테스트(`tests::is_known_tab_knows_all_six_tabs`,
/// `tests::every_tab_in_settings_html_is_a_known_tab`)가 검증한다 — 여기서는
/// "그 화이트리스트가 이 탭들을 다루는가"라는 배선 자체를 재발 방지 관점에서
/// 본다(이슈 #28: `Keyboards` 탭이 화이트리스트에 없어 설정 창 전체가 오류
/// 화면으로 죽었던 결함). 이슈 #32 Phase 1 에서 탭별 창 크기 테이블(예전
/// `tab_window_size`)은 없앴지만, 화이트리스트 자체와 이 재발 방지 테스트는
/// 그대로 남는다 — 크기와 무관하게 지켜야 하는 불변조건이기 때문이다.
#[test]
fn main_rs_의_known_tabs가_korean과_keyboards를_안다() {
    let main_rs = read_main_rs();
    assert!(
        main_rs.contains(
            "const KNOWN_TABS: &[&str] = \
             &[\"seek\", \"hyperkey\", \"presets\", \"korean\", \"japanese\", \"chinese\", \
               \"keyboards\", \"general\"];"
        ),
        "main.rs 의 KNOWN_TABS 화이트리스트가 예상한 8개 탭(자식 korean·japanese·chinese 포함)을 \
         그대로 담고 있지 않다"
    );
}

/// ⭐ F-15 §8 회귀 방지 — **환경설정 창을 열기만 해서는 `settings.json` 이 생기면 안 된다.**
///
/// 창을 처음 그릴 때도 탭 상태를 맞추려고 `settings_set_tab` 을 부르는데(이슈 #32
/// Phase 1 부터 이 호출은 창을 리사이즈하지 않는다 — 탭 화이트리스트 검증만
/// 한다), 그때 `ui.lastTab` 까지 저장해 버리면 "설정을 한 번도 건드리지 않으면
/// 저장 파일이 아예 생기지 않는다"가 첫 실행에서 바로 깨진다. 그래서 최초 렌더는
/// `persist: false` 로 부른다 — 이 테스트는 그 호출 규약이 유지되는지를 지킨다
/// (이 규약이 깨져도 다른 테스트는 전부 통과한다).
#[test]
fn 최초_렌더는_탭을_저장하지_않는다() {
    let html = read_settings_html();

    assert!(
        html.contains("invoke(\"settings_set_tab\", { tab, persist })"),
        "settings_set_tab 은 persist 인자를 함께 넘겨야 한다"
    );
    assert!(
        html.contains("{ persist: false }"),
        "최초 렌더의 activateTab 은 persist: false 로 불러야 한다 — 창을 열기만 해도 \
         settings.json 이 생기면 F-15 §8 이 깨진다"
    );
}

// ⭐ F-16(korean-input.md) — `Korean` 탭 재발 방지 테스트.

/// `Korean` 탭 버튼·패널과 5개 `data-key` 가 전부 `settings.html` 에 존재한다
/// (`Presets`·`General` 사이, 명세 §4.1 탭 순서). 2단계(한/영·한자) 항목도 `disabled`
/// 상태로나마 `data-key` 를 갖는다 — 다음 작업(2단계 활성화)이 `disabled` 속성만
/// 걷어내면 되도록 미리 배선해 둔 것이다(D-K8).
#[test]
fn settings_html_에_korean_탭_버튼_패널_5개_data_key가_있다() {
    let html = read_settings_html();

    assert!(
        html.contains(r#"id="tab-korean" data-tab="korean""#),
        "settings.html 에 Korean 탭 버튼(id=\"tab-korean\")이 없다"
    );
    assert!(
        html.contains(r#"id="panel-korean""#),
        "settings.html 에 Korean 탭 패널(id=\"panel-korean\")이 없다"
    );

    for data_key in [
        "korean.shiftSpaceSwitchesInputSource",
        "korean.hanEngSwitchesInputSource",
        "korean.hanjaKeyConvertsHanja",
        "korean.wonKeyTypesBacktick",
        "korean.disableInRemoteDesktop",
    ] {
        assert!(
            html.contains(&format!("data-key=\"{data_key}\"")),
            "settings.html 에 data-key=\"{data_key}\" 컨트롤이 없다"
        );
    }
}

/// ⭐ 이 테스트는 뜻이 뒤집혔다. 1단계 때는 §3.2 의 키코드가 `(미확정)`이라 한/영·
/// 한자 컨트롤이 `disabled` + 배지 + `.hint.why` 사유 문구로 dimmed 출하됐다(D-K8).
/// 2단계에서 그 전제 — `lang1`/`lang2` 의 virtual keycode — 가 근거 3중(W3C
/// `uievents-key` #55·로컬 SDK 헤더 실측·Chromium 구현, `docs/spec/korean-input.md`
/// §3.2)으로 해소됐다. 그래서 이제는 정반대를 검증한다: 두 컨트롤이 `disabled` 가
/// **아니고**, 배지가 **없으며**, 조작 가능한 사실을 담은 부제(`.hint`)를 갖는다.
/// ⛔ "실기기 미검증" 배지는 UI 에 두지 않는다(D-K16) — 그것은 개발자용 증거 등급
/// 메타 정보이지 사용자가 조작할 수 있는 사실이 아니다.
#[test]
fn settings_html_의_한영_한자_컨트롤은_활성이고_hint_문구를_가진다() {
    let html = read_settings_html();

    for (checkbox_id, hint_id, badge_id) in [
        (
            "korean-han-eng",
            "korean-han-eng-hint",
            "korean-han-eng-badge",
        ),
        ("korean-hanja", "korean-hanja-hint", "korean-hanja-badge"),
    ] {        let input_needle = format!("id=\"{checkbox_id}\"");
        let input_pos = html
            .find(&input_needle)
            .unwrap_or_else(|| panic!("settings.html 에 id=\"{checkbox_id}\" 컨트롤이 없다"));
        // 그 <input> 태그가 끝나는 지점(다음 '>') 까지만 봐서 disabled 속성을 확인한다.
        let tag_end = html[input_pos..]
            .find('>')
            .map(|i| input_pos + i)
            .unwrap_or_else(|| panic!("id=\"{checkbox_id}\" 의 <input> 태그가 닫히지 않았다"));
        let tag = &html[input_pos..tag_end];
        assert!(
            !tag.contains("disabled"),
            "id=\"{checkbox_id}\" 체크박스가 여전히 disabled 다 — 2단계는 활성 컨트롤로 \
             출하해야 한다(D-K14, §3.2 해소)"
        );

        assert!(
            html.contains(&format!("id=\"{hint_id}\"")),
            "id=\"{checkbox_id}\" 옆에 부제 문단(id=\"{hint_id}\")이 없다"
        );
        assert!(
            !html.contains(&format!("id=\"{badge_id}\"")),
            "id=\"{checkbox_id}\" 옆에 배지(id=\"{badge_id}\")가 여전히 남아 있다 — \
             활성 컨트롤에는 배지를 두지 않는다"
        );
    }

    // ⛔ D-K16 — 옛 `.why` 카탈로그 키(개발자용 증거 등급 문구)가 새 `.hint` 로
    // 대체되어 더 이상 존재하지 않는다. 두 카탈로그 모두에서 확인한다.
    let en = read_en_catalog();
    let ko = read_ko_catalog();
    for old_key in ["settings.korean.han_eng.why", "settings.korean.hanja.why"] {
        assert!(
            en.get(old_key).is_none(),
            "en.json 에 옛 사유 문구 키 {old_key} 가 남아 있다 — .hint 로 교체돼야 한다(D-K16)"
        );
        assert!(
            ko.get(old_key).is_none(),
            "ko.json 에 옛 사유 문구 키 {old_key} 가 남아 있다 — .hint 로 교체돼야 한다(D-K16)"
        );
    }
}

/// `settings.korean.*` 리터럴이 en·ko 양쪽 카탈로그에 전부 있다. (일반 검사
/// `settings_html_의_settings_점_리터럴은_전부_카탈로그_키다` 는 en 만 보므로, F-16
/// 이 한쪽 카탈로그만 갱신하는 회귀를 여기서 별도로 잡는다.)
#[test]
fn korean_점_리터럴이_en_ko_양쪽_카탈로그에_모두_있다() {
    let html = read_settings_html();
    let en_keys = flatten_catalog(&read_en_catalog());
    let ko_keys = flatten_catalog(&read_ko_catalog());

    let korean_keys: BTreeSet<_> = extract_double_quoted_literals(&html)
        .into_iter()
        .filter(|s| s.starts_with("settings.korean."))
        .collect();

    assert!(
        !korean_keys.is_empty(),
        "settings.html 에서 \"settings.korean.*\" 리터럴을 하나도 찾지 못했다 — \
         Korean 탭 배선이 빠졌을 수 있다"
    );

    let missing_en: Vec<_> = korean_keys
        .iter()
        .filter(|k| !en_keys.contains(k.as_str()))
        .collect();
    let missing_ko: Vec<_> = korean_keys
        .iter()
        .filter(|k| !ko_keys.contains(k.as_str()))
        .collect();

    assert!(
        missing_en.is_empty(),
        "en.json 에 없는 settings.korean.* 키: {missing_en:?}"
    );
    assert!(
        missing_ko.is_empty(),
        "ko.json 에 없는 settings.korean.* 키: {missing_ko:?}"
    );
}

// ⭐ K5·K9(이슈 #73) — Korean 탭 개선의 재발 방지 테스트.

/// K5 — 제외 대상 목록 편집기 3종(헤딩·빈 목록 안내·추가·되돌리기 버튼)과
/// K9 — modifier 소문자 옵션 체크박스가 `settings.html` 에 존재하고, 저장 키가
/// 배선돼 있다.
#[test]
fn settings_html에_korean_제외_목록_편집기와_modifier_소문자_옵션이_있다() {
    let html = read_settings_html();

    for id in [
        "korean-excluded-heading",
        "korean-excluded-list",
        "korean-excluded-empty",
        "korean-excluded-add-btn",
        "korean-excluded-reset-btn",
    ] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "settings.html 에 K5 목록 편집기 요소 id=\"{id}\" 가 없다"
        );
    }
    assert!(
        html.contains("id=\"korean-modifier-lowercase\" data-key=\"korean.modifierKeyTypesLowercase\""),
        "settings.html 에 K9 체크박스(data-key=\"korean.modifierKeyTypesLowercase\")가 없다"
    );
    assert!(
        html.contains("id=\"korean-modifier-lowercase-label\""),
        "settings.html 에 K9 라벨(id=\"korean-modifier-lowercase-label\")이 없다"
    );
    assert!(
        html.contains("id=\"korean-modifier-lowercase-hint\""),
        "settings.html 에 K9 부제(id=\"korean-modifier-lowercase-hint\")가 없다"
    );
}

/// K9 — 백엔드 `main.rs` 가 저장 키 2종(`korean.excludedBundleIds`·
/// `korean.modifierKeyTypesLowercase`)을 다룬다. `apply_korean_setting` 의
/// match 누락(키 추가 후 배선 누락)을 소스 텍스트로 잡는다.
#[test]
fn main_rs가_korean_신규_저장_키를_다룬다() {
    let main_rs = read_main_rs();
    for needle in [
        "KOREAN_EXCLUDED_BUNDLE_IDS",
        "KOREAN_MODIFIER_KEY_TYPES_LOWERCASE",
    ] {
        assert!(
            main_rs.contains(needle),
            "main.rs 에 저장 키 상수 {needle} 사용이 없다 — settings_set 배선이 빠졌다"
        );
    }
    // K5 — 목록이 바뀌면 게이트도 다시 주입해야 한다(D-K17, D-K3 과 같은 잊기 쉬운 지점).
    let set_korean_body_start = main_rs
        .find("fn settings_set_korean(")
        .expect("main.rs 에 settings_set_korean 함수가 없다");
    let body = &main_rs[set_korean_body_start..];
    let body_end = body[10..]
        .find("\nfn ")
        .map(|i| i + 10)
        .unwrap_or(body.len());
    let body = &body[..body_end];
    assert!(
        body.contains("KOREAN_EXCLUDED_BUNDLE_IDS"),
        "settings_set_korean 이 korean.excludedBundleIds 변경을 처리하지 않는다 — \
         목록 편집이 게이트에 반영되지 않는다(D-K17)"
    );
    assert!(
        body.contains("set_korean_excluded_apps"),
        "settings_set_korean 이 set_korean_excluded_apps 를 부르지 않는다 — \
         목록 편집이 게이트에 반영되지 않는다(D-K17)"
    );
}

/// K9 — `ultrakey-core::settings::EngineConfig` 가 `korean_modifier_lowercase` 를
/// 갖고 `build_engine_config` 가 채운다. 이게 빠지면 옵션을 켜도 엔진이 모른다.
#[test]
fn engine_config가_korean_modifier_lowercase를_운반한다() {
    let core_settings = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../crates/ultrakey-core/src/settings/mod.rs"
    ))
    .expect("crates/ultrakey-core/src/settings/mod.rs 를 읽지 못했다");
    assert!(
        core_settings.contains("pub korean_modifier_lowercase: bool"),
        "EngineConfig 에 korean_modifier_lowercase 필드가 없다"
    );

    let main_rs = read_main_rs();
    let fn_start = main_rs
        .find("fn build_engine_config(")
        .expect("main.rs 에 build_engine_config 함수가 없다");
    let body = &main_rs[fn_start..];
    let body_end = body[10..]
        .find("\nfn ")
        .map(|i| i + 10)
        .unwrap_or(body.len());
    let body = &body[..body_end];
    assert!(
        body.contains("korean.modifier_key_types_lowercase"),
        "build_engine_config 가 korean.modifier_key_types_lowercase 를 읽지 않는다 — \
         K9 옵션이 엔진에 반영되지 않는다(D-K18)"
    );
}

// ⭐ F-10(menu-bar-and-lifecycle.md) — 메뉴바(NSStatusItem) 재발 방지 테스트.
//
// 프런트엔드(HTML/JS)와 달리 트레이 메뉴는 `src/main.rs` 안에서 전부 조립된다
// (`ultrakey-app` 은 라이브러리 타깃이 없어 이 통합 테스트가 그 내부 함수를 직접
// 부를 수 없다 — `settings.html`/`index.html` 을 다루는 위 테스트들과 같은 이유로
// 소스 텍스트를 정적으로 스캔한다).

/// `"menu.` 로 시작하는 큰따옴표 문자열 리터럴만 걸러낸다. `menu_ids` 모듈이
/// 문자열 리터럴 자체를 상수로 한 번만 선언하고 나머지 코드는 그 상수를
/// 참조하므로, 이 집합은 (a) `menu_ids::*` 상수 정의 자체 + (b) 상수를 두지 않은
/// 나머지 소수의 카탈로그 키(`menu.ignore_app.none` 등)로 이루어진다.
fn menu_dot_literals_in_main_rs() -> BTreeSet<String> {
    let main_rs = read_main_rs();
    let no_comments = strip_line_comments(&main_rs);
    extract_double_quoted_literals(&no_comments)
        .into_iter()
        .filter(|s| s.starts_with("menu."))
        .collect()
}

/// (a) `main.rs` 의 `"menu.*"` 리터럴이 전부 실재하는 카탈로그 키다(en 기준).
#[test]
fn main_rs_의_menu_점_리터럴은_전부_en_카탈로그_키다() {
    let en = read_en_catalog();
    let known_keys = flatten_catalog(&en);

    let menu_keys = menu_dot_literals_in_main_rs();
    assert!(
        !menu_keys.is_empty(),
        "src/main.rs 에서 \"menu.*\" 리터럴을 하나도 찾지 못했다 — 추출 로직이 깨졌을 수 있다"
    );

    let missing: Vec<_> = menu_keys
        .iter()
        .filter(|k| !known_keys.contains(k.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "src/main.rs 가 참조하는 다음 트레이 메뉴 카탈로그 키가 resources/i18n/en.json 에 없다: {missing:?}"
    );
}

/// (b) 트레이 메뉴가 참조하는 i18n 키가 5개 카탈로그 전부에 있다. `ultrakey-i18n`
/// 의 카탈로그 간 키 집합 동일성 테스트가 전체에 대해 이미 이걸 검증하지만,
/// 여기서는 "메뉴가 실제로 쓰는 키 집합"을 자체적으로 다시 좁혀서 확인한다 —
/// 이 테스트만 보고도 메뉴 관련 키 누락을 바로 알 수 있어야 한다.
///
/// ⚠️ 원래는 en·ko 두 카탈로그만 봤다 — 이슈 #39 부터 클론이 zh·es·ja 로
/// 늘었으므로(`LOCALES`), en·ko 만 보면 그 세 로케일에서만 키가 빠지는 회귀를
/// 이 테스트가 놓친다. `menu.launch_on_login.requires_approval`(이슈 #39, B-3)
/// 도 이 스캔이 자동으로 잡는다 — 별도 목록에 손으로 추가할 필요가 없다.
#[test]
fn main_rs_의_menu_점_리터럴은_다섯_카탈로그_모두에_있다() {
    let menu_keys = menu_dot_literals_in_main_rs();
    assert!(!menu_keys.is_empty());

    for locale in LOCALES {
        let keys = flatten_catalog(&read_catalog(locale));
        let missing: Vec<_> = menu_keys.iter().filter(|k| !keys.contains(k.as_str())).collect();
        assert!(
            missing.is_empty(),
            "{locale}.json 에 없는 메뉴 키: {missing:?}"
        );
    }
}

/// ⭐ F-10 이슈 #92 — 재실행(Finder 더블클릭·`open`)의 설정 창 표시 배선 재발 방지.
///
/// 실측 확정(이슈 #92): 실행 중인 앱을 `open` 으로 다시 실행하면 두 번째 프로세스가
/// **뜨지 않고** `kAEReopenApplication` Apple Event 가 기존 인스턴스로 전달되어
/// tao `applicationShouldHandleReopen:` → `tauri::RunEvent::Reopen` 이 된다. 이
/// 이벤트 암이 "로그만 남기고 끝"이면 재실행 시 설정 창이 열리지 않는다(M2 2차
/// 주석의 잘못된 가정 때문에 실재로 그렇게 회귀했었다). 이 테스트는 그 암이 반드시
/// `show_settings_window` 로 이어지도록 배선을 잠근다. 순수 로직(파일 시스템·OS
/// 독립)은 없고 이 이벤트를 실체로 재현할 수도 없어(OS 거동), 소스 텍스트로 암의
/// 윤곽을 고정한다.
#[test]
fn run_이벤트의_재open_암이_설정_창을_연다() {
    let main_rs = read_main_rs();

    let arm_start = main_rs
        .find("tauri::RunEvent::Reopen { .. } => {")
        .expect("main.rs 에 RunEvent::Reopen 암이 없다");
    let rest = &main_rs[arm_start..];
    let arm_end = rest
        .find("_ => {}")
        .map(|i| i + "_ => {}".len())
        .unwrap_or(rest.len());
    let arm = &rest[..arm_end];

    assert!(
        arm.contains("show_settings_window(app_handle)"),
        "Reopen 암이 show_settings_window 를 부르지 않는다 — 재실행 시 설정 창이 \
         열리지 않는다(이슈 #92)"
    );
    assert!(
        arm.contains("opening the settings window"),
        "Reopen 암이 재실행 처리 로그를 남기지 않는다 — 수동 검증(M4)이 관찰할 신호가 없다"
    );
}

/// 메뉴 항목 id(`menu_ids` 모듈)가 §3.3 이 정한 정상 메뉴 구성(`Ignore <앱>` ·
/// `Settings…` · `Check for Updates…`(F-13) · `About` · `Advanced ▸ (Relaunch)` ·
/// `Quit Ultrakey`)과 unauthorizedMenu 의 `Authorize` 항목을 전부 갖추고 있는지
/// 이름만으로 재확인한다. `Purchase`(F-12) 만 실수로 다시 들어오면(범위 밖) 이
/// 테스트가 아니라 코드 리뷰에서 걸러야 하지만, 적어도 필수 항목이 빠지는 회귀는
/// 여기서 잡는다. ⭐ 이슈 #77 — `menu.advanced.synthesize_caps_remap` 은 이 목록에서
/// 빠졌다(Synthesize 는 설정 화면 General 탭 Advanced 로 이동, D2).
#[test]
fn main_rs_가_정상_메뉴의_필수_항목_id를_전부_선언한다() {
    let main_rs = read_main_rs();
    for id in [
        "menu.ignore_app",
        "menu.settings",
        "menu.check_for_updates",
        "menu.about",
        "menu.advanced",
        "menu.advanced.relaunch",
        "menu.quit",
        "menu.unauthorized.authorize",
    ] {
        assert!(
            main_rs.contains(&format!("\"{id}\"")),
            "src/main.rs 에 메뉴 항목 id \"{id}\" 리터럴이 없다 — menu_ids 모듈에서 빠졌을 수 있다"
        );
    }
}

/// ⭐ F-13 — General 탭 `Check for updates automatically` 체크박스가 **활성** 컨트롤로
/// 출하됐는지 검증한다: disabled 가 아니고, data-key(`general.autoUpdate`)가 붙고,
/// 미구현 배지가 없다. 이전 구현은 이 자리를 disabled+badge 로 비워 두었는데, F-13
/// 배선이 들어왔으므로 활성화됐다(명세 §4.1 "체크박스 1개").
#[test]
fn settings_html_의_자동_업데이트_체크박스는_활성이다() {
    let html = read_settings_html();

    let input_needle = r#"id="auto-update""#;
    let input_pos = html
        .find(input_needle)
        .unwrap_or_else(|| panic!("settings.html 에 id=\"auto-update\" 컨트롤이 없다"));
    let tag_end = html[input_pos..]
        .find('>')
        .map(|i| input_pos + i)
        .unwrap_or_else(|| panic!("id=\"auto-update\" 의 <input> 태그가 닫히지 않았다"));
    let tag = &html[input_pos..tag_end];
    assert!(
        !tag.contains("disabled"),
        "id=\"auto-update\" 체크박스가 여전히 disabled 다 — F-13 은 활성 컨트롤로 출하해야 한다"
    );
    assert!(
        html.contains(r#"data-key="general.autoUpdate""#),
        "'auto-update' 체크박스에 data-key=\"general.autoUpdate\" 가 없다 — settings_set 라우팅과 엮이지 않는다"
    );
    assert!(
        !html.contains(r#"id="auto-update-badge""#),
        "id=\"auto-update-badge\" 가 여전히 남아 있다 — 활성 컨트롤에는 미구현 배지를 두지 않는다"
    );
    assert!(
        html.contains(r#"id="auto-update-why""#),
        "id=\"auto-update-why\" 부제 문단이 없다"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 아이콘(이슈 #16) — 정본 SVG 와 그것을 쓰는 세 자리가 어긋나지 않게 잡는다.
// ─────────────────────────────────────────────────────────────────────────────
fn read_source_svg() -> String {
    // 정본은 워크스페이스 루트의 assets/app-icon/ultrakey.svg 하나뿐이다.
    let path = manifest_dir().join("../../assets/app-icon/ultrakey.svg");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("assets/app-icon/ultrakey.svg 을 읽지 못했다({path:?}): {e}"))
}

/// `<path ... d="..." ...>` 의 `d` 값만 등장 순서대로 뽑는다.
fn path_data(svg: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = svg;
    while let Some(i) = rest.find("d=\"") {
        rest = &rest[i + 3..];
        let Some(j) = rest.find('"') else { break };
        out.push(rest[..j].to_string());
        rest = &rest[j + 1..];
    }
    out
}

/// ⭐ 이슈 #16 은 **원본 SVG 의 도형을 바꾸는 것을 금지한다.** `ui/index.html`
/// 의 온보딩 아이콘은 정본을 인라인으로 복사해 넣은 것이라, 한쪽만 손대면
/// 조용히 갈라진다 — 컴파일러도 다른 테스트도 그걸 잡지 못한다.
#[test]
fn index_html_의_아이콘_path는_정본_svg_와_같다() {
    let source = path_data(&read_source_svg());
    assert_eq!(
        source.len(),
        3,
        "정본 assets/app-icon/ultrakey.svg 의 path 개수가 3 이 아니다 — \
         scripts/generate-icons.sh 도 3 개를 전제한다."
    );

    let html = read_index_html();
    let svg_start = html
        .find("<svg class=\"app-icon\"")
        .expect("ui/index.html 에 class=\"app-icon\" 인 인라인 SVG 가 없다 (이슈 #16)");
    let svg_end = html[svg_start..]
        .find("</svg>")
        .expect("ui/index.html 의 app-icon SVG 에 </svg> 종료 태그가 없다")
        + svg_start;
    let inline = path_data(&html[svg_start..svg_end]);

    assert_eq!(
        inline, source,
        "ui/index.html 의 온보딩 아이콘 path 가 정본 assets/app-icon/ultrakey.svg 와 \
         다르다. 한쪽만 고치면 앱 번들·메뉴바 아이콘과 온보딩 아이콘의 모양이 \
         갈라진다 — 정본을 고친 뒤 그 `d` 를 index.html 에 그대로 옮겨라."
    );
}

/// ⭐ 이슈 #96 — About 창 히어로의 앱 아이콘도 정본의 인라인 사본이다
/// (index.html 과 같은 방식·같은 근거 — docs/dev/icons.md §2.4). 온보딩 아이콘과
/// 마찬가지로 한쪽만 고치면 조용히 갈라지므로 같은 정합 검사로 잡는다.
#[test]
fn about_html_의_아이콘_path는_정본_svg_와_같다() {
    let source = path_data(&read_source_svg());

    let html = read_about_html();
    let svg_start = html
        .find("<svg class=\"app-icon\"")
        .expect("ui/about.html 에 class=\"app-icon\" 인 인라인 SVG 가 없다 (이슈 #96)");
    let svg_end = html[svg_start..]
        .find("</svg>")
        .expect("ui/about.html 의 app-icon SVG 에 </svg> 종료 태그가 없다")
        + svg_start;
    let inline = path_data(&html[svg_start..svg_end]);

    assert_eq!(
        inline, source,
        "ui/about.html 의 히어로 아이콘 path 가 정본 assets/app-icon/ultrakey.svg 와 \
         다르다 — 정본을 고친 뒤 그 `d` 를 about.html 에 그대로 옮겨라 (이슈 #96)."
    );
}

/// 메뉴바는 **template 이미지**를 써야 다크/라이트가 자동 대응된다(이슈 #16).
/// 앱 번들 아이콘(불투명 타일)을 되돌려 쓰면 메뉴바에서 사각 실루엣으로
/// 뭉개지므로, `setup_tray` 가 전용 자산을 가리키는지 정적으로 검사한다.
#[test]
fn setup_tray_는_전용_메뉴바_template_자산을_쓴다() {
    let main_rs = read_main_rs();
    assert!(
        main_rs.contains("include_bytes!(\"../icons/menubar-template.png\")"),
        "setup_tray 가 icons/menubar-template.png 를 포함하지 않는다. 메뉴바 \
         아이콘은 배경 없는 template 이미지여야 macOS 가 라이트/다크에 맞춰 \
         다시 칠한다 — 앱 번들 아이콘(불투명 타일)을 쓰면 사각 실루엣이 된다."
    );

    let asset = manifest_dir().join("icons/menubar-template.png");
    assert!(
        asset.is_file(),
        "icons/menubar-template.png 이 없다({asset:?}). \
         ./scripts/generate-icons.sh 로 다시 만든다."
    );
}

/// `tauri.conf.json` 의 `bundle.icon` 이 실제로 존재하는 파일만 가리키는지 —
/// 아이콘을 갈아 끼우면서 파일명을 바꾸면 번들에 옛 아이콘이 남거나 빌드가
/// 깨진다.
#[test]
fn bundle_icon_목록의_파일이_전부_존재한다() {
    let conf = read_tauri_conf();
    let icons = conf["bundle"]["icon"]
        .as_array()
        .expect("tauri.conf.json 의 bundle.icon 이 배열이 아니다");
    assert!(
        !icons.is_empty(),
        "tauri.conf.json 의 bundle.icon 이 비어 있다"
    );

    for entry in icons {
        let rel = entry.as_str().expect("bundle.icon 항목이 문자열이 아니다");
        let path = manifest_dir().join(rel);
        assert!(
            path.is_file(),
            "tauri.conf.json 의 bundle.icon 이 가리키는 {rel} 가 없다({path:?}). \
             ./scripts/generate-icons.sh 로 다시 만든다."
        );
    }
}

/// ⭐ 이슈 #77 — synthesize 토글의 "엔진 반영" 의도가 **설정 화면 경로**로 유지됨을
/// 단언한다. 트레이 메뉴의 `Synthesize Caps Lock Remap` 항목(과 그 전용 토글 함수
/// `on_menu_toggle_synthesize_caps_lock_remap`)은 제거됐고(D2), 설정 화면 체크박스가
/// 기존 `settings_set` → `settings_set_preset` → `apply_preset_setting` 경로를 타므로
/// 이 테스트는 그 경로가 끝까지 `reconfigure_engine`(경로 B 재적용)에 닿는지를 본다.
///
/// ⚠️ 이는 이슈 #19 증상 B 의 회귀 방지였던 테스트(`Synthesize Caps Lock Remap`
/// 메뉴 토글이 저장만 하고 엔진을 건드리지 않던 결함)의 후속이다. 엔진 반영은
/// 이제 체크박스 → `settings_set_preset` 경로가 책임지므로, 여기서는 (1) `apply_preset_`
/// `setting` 에 `PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP` 분기가 있고 (2) 그 분기를 부르는
/// `settings_set_preset` 이 본문에서 `reconfigure_engine` 을 호출함을 정적으로 확인한다.
/// ⚠️ 정적 텍스트 검사다 — 이 파일의 다른 테스트들과 같은 한계를 갖는다.
#[test]
fn synthesize_토글의_설정_경로가_엔진까지_반영된다() {
    let main_rs = read_main_rs();

    // (1) apply_preset_setting 의 본문 — synthesize 분기가 presets 정본을 바꾼다.
    let ap_start = main_rs
        .find("fn apply_preset_setting")
        .expect("apply_preset_setting 이 없다");
    let ap_rest = &main_rs[ap_start..];
    let ap_end = ap_rest[1..]
        .find("\nfn ")
        .map(|i| i + 1)
        .unwrap_or(ap_rest.len());
    let apply_body = &ap_rest[..ap_end];
    assert!(
        apply_body.contains("PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP"),
        "apply_preset_setting 에 PRESETS_SYNTHESIZE_CAPS_LOCK_REMAP 분기가 없다"
    );
    assert!(
        apply_body.contains("presets.synthesize_caps_lock_remap = parse(value, key)?"),
        "apply_preset_setting 이 synthesize 값을 presets 정본에 반영하지 않는다"
    );

    // (2) settings_set_preset 의 본문 — presets 변경이 reconfigure_engine(force_reset)
    // 까지 도달한다. 이 함수는 presets.* 변경 후 무조건 force_reset=true 로
    // reconfigure_engine 을 부르므로(§3.1 실측 1), 설정 화면 체크박스 하나로
    // 이전 메뉴 토글과 같은 엔진 반영이 완결된다.
    let sp_start = main_rs
        .find("fn settings_set_preset")
        .expect("settings_set_preset 이 없다");
    let sp_rest = &main_rs[sp_start..];
    let sp_end = sp_rest[1..]
        .find("\nfn ")
        .map(|i| i + 1)
        .unwrap_or(sp_rest.len());
    let set_preset_body = &sp_rest[..sp_end];
    assert!(
        set_preset_body.contains("validate_and_apply_preset"),
        "settings_set_preset 이 apply_preset_setting 을 부르지 않는다"
    );
    assert!(
        set_preset_body.contains("reconfigure_engine"),
        "settings_set_preset 이 reconfigure_engine 을 부르지 않는다 — \
         경로 B 매핑이 설정과 어긋난 채 남는다"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// 이슈 #77 — 설정 화면 Advanced 섹션(`<details id="general-advanced">`).
// ─────────────────────────────────────────────────────────────────────────────

/// 이슈 #77 — `Synthesize Caps Lock Remap` 체크박스가 활성 컨트롤로 배선됐는지
/// 본다: `id="synthesize-caps-remap"` + `data-key="presets.synthesizeCapsLockRemap"`
/// (기존 위임 경로 그대로), disabled 가 아니다.
#[test]
fn settings_html의_synthesize_체크박스가_활성이다() {
    let html = read_settings_html();

    let input_needle = r#"id="synthesize-caps-remap""#;
    let input_pos = html
        .find(input_needle)
        .unwrap_or_else(|| panic!("settings.html 에 id=\"synthesize-caps-remap\" 컨트롤이 없다"));
    let tag_end = html[input_pos..]
        .find('>')
        .map(|i| input_pos + i)
        .unwrap_or_else(|| panic!("id=\"synthesize-caps-remap\" 의 <input> 태그가 닫히지 않았다"));
    let tag = &html[input_pos..tag_end];
    assert!(
        !tag.contains("disabled"),
        "id=\"synthesize-caps-remap\" 체크박스가 disabled 다 — 활성 컨트롤로 출하해야 한다"
    );
    assert!(
        html.contains(r#"data-key="presets.synthesizeCapsLockRemap""#),
        "'synthesize-caps-remap' 체크박스에 data-key=\"presets.synthesizeCapsLockRemap\" 가 없다 — \
         settings_set → settings_set_preset 경로와 엮이지 않는다"
    );
    assert!(
        html.contains(r#"id="synthesize-caps-remap-why""#),
        "id=\"synthesize-caps-remap-why\" 위험 고지 문단이 없다"
    );
}

/// 이슈 #77 — Advanced 섹션이 `<details id="general-advanced">` 로 구현됐고
/// **기본 접힘**(`open` 속성 부재)이다. 접힘 상태는 저장하지 않는다(세션 상태 —
/// 신규 저장 키 없음, D1).
#[test]
fn settings_html의_advanced_details가_기본_접힘이다() {
    let html = read_settings_html();

    let details_needle = r#"<details id="general-advanced">"#;
    let details_pos = html
        .find(details_needle)
        .unwrap_or_else(|| panic!("settings.html 에 <details id=\"general-advanced\"> 가 없다"));
    let open_attr_in_open_tag = html[details_pos..details_pos + details_needle.len() + 1]
        .contains("open");
    assert!(
        !open_attr_in_open_tag,
        "<details id=\"general-advanced\"> 에 open 속성이 있다 — 기본 접힘으로 출하해야 한다"
    );
    assert!(
        html.contains(r#"<summary id="general-advanced-summary">"#),
        "<summary id=\"general-advanced-summary\"> 가 없다 — <summary> 가 섹션 헤딩을 겸한다(D4)"
    );
}

/// 이슈 #77 — 이동의 결과가 정적 마크업에서 남아 있지 않다: (1) 진단
/// `#general-diagnostics-heading`(h2.group)은 `<summary>` 로 대체, (2) 트레이 메뉴
/// `menu.advanced.synthesize_caps_remap` 은 main.rs 에서 완전히 제거(복제 아님).
#[test]
fn 이동_결과_구_위치에_남은게_없다() {
    let html = read_settings_html();
    assert!(
        !html.contains(r#"id="general-diagnostics-heading""#),
        "settings.html 에 #general-diagnostics-heading 이 남아 있다 — Advanced <summary> 로 대체해야 한다"
    );
    // 진단 버튼은 제거가 아니라 **이동**이므로 그대로 있어야 한다.
    for id in ["open-event-viewer-btn", "open-log-folder-btn"] {
        assert!(
            html.contains(&format!(r#"id="{id}""#)),
            "settings.html 에 <button id=\"{id}\"> 가 없다 — 진단 버튼은 Advanced 안으로 이동"
        );
    }

    let main_rs = read_main_rs();
    assert!(
        !main_rs.contains("menu.advanced.synthesize_caps_remap"),
        "src/main.rs 에 menu.advanced.synthesize_caps_remap 이 남아 있다 — 트레이 항목은 \
         복제가 아니라 제거해야 한다(D2)"
    );
}

/// 이슈 #77 — 신규 키 3개(`settings.general.advanced.*`)가 5개 카탈로그 모두에
/// 있고, 삭제 키 2개(`menu.advanced.synthesize_caps_remap`·`settings.general.
/// diagnostics`)가 5개 카탈로그 모두에 없다. 카탈로그 간 키 집합 동일성은
/// `ultrakey-i18n` 이 전체에 대해 검증하므로, 여기서는 이번 이슈의 키만 좁혀 본다.
#[test]
fn 이슈_77_신규키는_다섯_카탈로그_모두에_있고_삭제키는_모두에_없다() {
    let added: &[&str] = &[
        "settings.general.advanced",
        "settings.general.advanced.synthesize_caps_remap",
        "settings.general.advanced.synthesize_caps_remap.why",
    ];
    let removed: &[&str] = &[
        "menu.advanced.synthesize_caps_remap",
        "settings.general.diagnostics",
    ];

    for locale in LOCALES {
        let keys = flatten_catalog(&read_catalog(locale));
        for k in added {
            assert!(
                keys.contains(*k),
                "{locale}.json 에 신규 키 \"{k}\" 가 없다"
            );
        }
        for k in removed {
            assert!(
                !keys.contains(*k),
                "{locale}.json 에 삭제 키 \"{k}\" 가 아직 남아 있다"
            );
        }
    }
}

// ⭐ F-17(per-device-settings.md) — `Keyboards` 탭 재발 방지 테스트.
//
// ⚠️ 이 테스트들은 프론트엔드(settings.html) 배선만 정적으로 확인한다. 이 탭이
// 기대하는 백엔드 커맨드(settings_unset · open_keyboard_settings)와 bootstrap
// state.perDevice 필드는 이 워크트리에서 다른 세션이 동시에 구현 중이라, main.rs/
// crates 쪽이 아직 없을 수 있다 — 여기서는 main.rs 를 전혀 참조하지 않는다.

/// `Keyboards` 탭 버튼·패널이 존재하고, `TABS` 배열에 Korean 다음·General 앞
/// 순서로 들어 있다(명세 §3.1: Seek·Hyperkey·Presets·Korean·Keyboards·General).
#[test]
fn settings_html_에_keyboards_탭_버튼_패널이_tabs_배열_순서대로_있다() {
    let html = read_settings_html();

    assert!(
        html.contains(r#"id="tab-keyboards" data-tab="keyboards""#),
        "settings.html 에 Keyboards 탭 버튼(id=\"tab-keyboards\")이 없다"
    );
    assert!(
        html.contains(r#"id="panel-keyboards""#),
        "settings.html 에 Keyboards 탭 패널(id=\"panel-keyboards\")이 없다"
    );
    assert!(
        html.contains(r#"["seek", "hyperkey", "presets", "korean", "japanese", "chinese", "keyboards", "general"]"#),
        "settings.html 의 TABS 배열이 Korean·Japanese·Chinese 다음, General 앞 순서로 \
         \"keyboards\" 를 갖고 있지 않다(명세 §3.1 + F-19 §4.1 언어 트리)"
    );
}

/// 고정 컨트롤 4개(명세 §3.1.5): 디바이스 선택 좌측 패인 1(이슈 #31 ④ 로 팝업에서
/// 재배치) + 그룹 제목 2("키 변환 세트", "Function Keys") + macOS Function Keys
/// 상태 표시줄(뱃지+버튼) 1.
#[test]
fn settings_html_에_keyboards_탭_고정_컨트롤_4개가_있다() {
    let html = read_settings_html();

    for id in [
        "keyboards-device-list",              // 1. 디바이스 선택 좌측 패인
        "keyboards-keyremap-heading",         // 2. 그룹 제목 — 키 변환 세트
        "keyboards-functionkeys-heading",     // 2. 그룹 제목 — Function Keys
        "keyboards-fnstate-badge",            // 3. macOS 상태 표시줄 — 뱃지
        "keyboards-open-system-settings-btn", // 3. macOS 상태 표시줄 — 버튼
    ] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "settings.html 에 Keyboards 탭 고정 컨트롤 id=\"{id}\" 가 없다"
        );
    }
}

/// ⭐ 이슈 #31 ④ 회귀 방지 — 디바이스 선택 팝업(`<select>`)이 완전히 사라지고
/// 좌측 세로 패인(`role="listbox"` + 항목 `role="option"`)으로 대체됐다.
#[test]
fn settings_html_의_디바이스_선택은_팝업이_아니라_좌측_패인이다() {
    let html = read_settings_html();

    assert!(
        !html.contains(r#"id="keyboards-device-picker""#),
        "settings.html 에 옛 디바이스 선택 <select>(id=\"keyboards-device-picker\")가 \
         아직 남아 있다 — 이슈 #31 ④ 가 좌측 패인으로 교체하기로 했다"
    );
    assert!(
        html.contains(r#"id="keyboards-device-list""#) && html.contains(r#"role="listbox""#),
        "settings.html 에 좌측 패인 컨테이너(id=\"keyboards-device-list\", \
         role=\"listbox\")가 없다"
    );

    let js = extract_js_function(&html, "function buildDeviceOption(");
    assert!(
        js.contains(r#"role", "option""#),
        "buildDeviceOption() 이 항목에 role=\"option\" 을 달지 않는다"
    );

    let highlight = extract_js_function(&html, "function highlightSelectedDevice(");
    assert!(
        highlight.contains("aria-selected"),
        "highlightSelectedDevice() 가 aria-selected 를 갱신하지 않는다"
    );

    // 방향키 이동 — 이 탭 목록(#tablist)의 키보드 처리와 같은 수준.
    assert!(
        html.contains(r#"$("keyboards-device-list").addEventListener("keydown""#),
        "좌측 패인에 keydown(방향키) 리스너가 배선돼 있지 않다"
    );
    assert!(
        html.contains(r#"$("keyboards-device-list").addEventListener("click""#),
        "좌측 패인에 click 리스너가 배선돼 있지 않다"
    );
}

/// 기능 2(고정 12개, §3.5): F1~F12 각각 선택 팝업 + `data-fn-key` 배선.
#[test]
fn settings_html_에_keyboards_탭_f1_f12_팝업_12개가_있다() {
    let html = read_settings_html();

    for n in 1..=12 {
        let id = format!("keyboards-fkey-f{n}");
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "settings.html 에 F{n} 선택 팝업(id=\"{id}\")이 없다"
        );
        assert!(
            html.contains(&format!("data-fn-key=\"f{n}\"")),
            "id=\"{id}\" 팝업에 data-fn-key=\"f{n}\" 배선이 없다"
        );
    }
}

/// ⭐ 이슈 #31 ② — 기능 2 팝업이 `<optgroup>` 을 만들고, `표준 F-키로 사용`
/// (`KEYBOARDS_STANDARD_FKEY`)이 카테고리 루프보다 **앞**에 온다. 예전에는 이
/// 옵션이 팝업 맨 끝에 있어 313개 항목 뒤에 있으면 사실상 못 찾았다.
#[test]
fn populate_function_key_select_는_optgroup을_만들고_표준_f_키_옵션을_카테고리_앞에_둔다() {
    let html = read_settings_html();
    let f = extract_js_function(&html, "function populateFunctionKeySelect(");

    assert!(
        f.contains("createElement(\"optgroup\")"),
        "populateFunctionKeySelect() 가 <optgroup> 을 만들지 않는다"
    );

    let standard_at = f
        .find("KEYBOARDS_STANDARD_FKEY")
        .expect("KEYBOARDS_STANDARD_FKEY 참조가 있어야 한다");
    let category_loop_at = f
        .find("destinationCategories")
        .expect("destinationCategories 순회가 있어야 한다");
    assert!(
        standard_at < category_loop_at,
        "표준 F-키로 사용 옵션이 카테고리 optgroup 뒤에 온다 — 313개 항목 뒤에 있으면 \
         사실상 못 찾는다(이슈 #31 ②)"
    );
}

/// ⭐ 이슈 #31 ② — 선택된 목적지가 `verified === false` 면 그 F-키 행 아래에
/// "동작 미확인" 힌트가 배선돼 있다(카탈로그에서 숨기지 않는다).
#[test]
fn render_function_keys_group_이_동작_미확인_힌트를_배선한다() {
    let html = read_settings_html();
    let f = extract_js_function(&html, "function renderFunctionKeysGroup(");

    assert!(
        f.contains("renderHiddenNote"),
        "renderFunctionKeysGroup() 이 renderHiddenNote() 를 호출하지 않는다"
    );
    assert!(
        f.contains("preferences.keyboards.functionKeys.unverifiedHint"),
        "renderFunctionKeysGroup() 이 unverifiedHint 카탈로그 키를 참조하지 않는다"
    );
    assert!(
        f.contains(".verified"),
        "renderFunctionKeysGroup() 이 목적지의 verified 플래그를 보지 않는다"
    );

    // 힌트 호스트가 F1~F12 각각에 있어야 한다.
    for n in 1..=12 {
        let host_id = format!("keyboards-fkey-f{n}-hint");
        assert!(
            html.contains(&format!("id=\"{host_id}\"")),
            "settings.html 에 F{n} 동작 미확인 힌트 호스트(id=\"{host_id}\")가 없다"
        );
    }
}

/// 기능 1(가변 M행, §3.4.1): 목록 컨테이너 + `+ Add item` 버튼.
#[test]
fn settings_html_에_keyboards_탭_가변_길이_목록_편집기_컨테이너와_추가_버튼이_있다() {
    let html = read_settings_html();

    assert!(
        html.contains(r#"id="keyboards-keyremap-list""#),
        "settings.html 에 키 변환 세트 목록 컨테이너(id=\"keyboards-keyremap-list\")가 없다"
    );
    assert!(
        html.contains(r#"id="keyboards-keyremap-add-btn""#),
        "settings.html 에 `+ Add item` 버튼(id=\"keyboards-keyremap-add-btn\")이 없다"
    );
    assert!(
        html.contains(r#"id="keyboards-keyremap-empty""#),
        "settings.html 에 빈 목록 안내(id=\"keyboards-keyremap-empty\")가 없다"
    );
}

/// ⭐ 이슈 #31 회귀 방지 — 편집 중인 미완성 행이 재렌더에 지워지면 안 된다.
///
/// 원인: `saveKeyRemapRows()` 가 **완성된 행 집합이 그대로일 때도** `commit()` 을
/// 불렀고, `commit()` 은 응답 상태로 화면을 다시 그리면서 목록 DOM 을 통째로 새로
/// 만들었다. 저장 대상이 아닌 미완성 행(from 만 고른 상태)이 그때 사라져, 첫 행이
/// 생긴 뒤로는 **두 번째 행을 끝까지 만들 수가 없었다** — 양방향 스왑
/// (`left_option → left_command` + `left_command → left_option`)이 불가능했다.
///
/// 이 테스트는 두 방어선이 소스에 남아 있는지 정적으로 확인한다. 실제 DOM 동작은
/// 이 저장소에 JS 실행 환경이 없어 자동 검증하지 못한다 — 실기 확인으로 보완한다.
#[test]
fn settings_html_이_편집_중인_미완성_키변환_행을_재렌더로_지우지_않는다() {
    let html = read_settings_html();

    // 방어선 ① — 저장할 것이 달라지지 않았으면 commit 하지 않는다.
    let save_fn = extract_js_function(&html, "function saveKeyRemapRows()");
    assert!(
        save_fn.contains("keyRemapRowsEqual"),
        "saveKeyRemapRows() 에 \"완성된 행 집합이 그대로면 저장하지 않는다\" 가드가 없다"
    );

    // 방어선 ② — 목록 DOM 을 **조건 없이** 새로 만들지 않는다.
    let render_fn = extract_js_function(&html, "function renderKeyRemapGroup(");
    assert!(
        render_fn.contains("innerHTML"),
        "renderKeyRemapGroup() 이 목록을 다시 만드는 코드를 잃었다"
    );
    assert!(
        render_fn.contains("keyRemapRowsEqual"),
        "renderKeyRemapGroup() 이 목록 DOM 을 조건 없이 새로 만든다 — 편집 중인 \
         미완성 행이 지워진다(이슈 #31)"
    );
    let reset_at = render_fn
        .find("innerHTML")
        .expect("바로 위에서 존재를 확인했다");
    let guard_at = render_fn
        .find("keyRemapRowsEqual")
        .expect("바로 위에서 존재를 확인했다");
    assert!(
        guard_at < reset_at,
        "renderKeyRemapGroup() 의 목록 초기화가 가드보다 앞선다 — 가드가 무력하다"
    );
}

/// `settings.html` 안의 인라인 JS 함수 하나를 중괄호 균형으로 잘라 낸다.
/// 정적 검사가 "파일 어딘가에 이 문자열이 있다"가 아니라 "**이 함수 안에** 있다"를
/// 볼 수 있게 하기 위한 최소 도구다.
fn extract_js_function<'a>(html: &'a str, signature: &str) -> &'a str {
    let start = html
        .find(signature)
        .unwrap_or_else(|| panic!("settings.html 에서 `{signature}` 를 찾지 못했다"));
    let body_start = html[start..]
        .find('{')
        .unwrap_or_else(|| panic!("`{signature}` 뒤에 여는 중괄호가 없다"))
        + start;
    let mut depth = 0usize;
    for (offset, ch) in html[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return &html[start..body_start + offset + 1];
                }
            }
            _ => {}
        }
    }
    panic!("`{signature}` 의 중괄호가 닫히지 않는다");
}

/// Keyboards 탭이 기대하는 두 백엔드 커맨드를 invoke 한다 — `settings_unset`
/// ("공통 따름" = 키 삭제, §3.3) 과 `open_keyboard_settings`(macOS 설정 열기, §3.5.1).
#[test]
fn settings_html_이_settings_unset과_open_keyboard_settings를_invoke한다() {
    let html = read_settings_html();
    for command in [
        "settings_unset",
        "open_keyboard_settings",
        "keyboard_fn_state",
    ] {
        assert!(
            html.contains(&format!("invoke(\"{command}\"")),
            "ui/settings.html 이 invoke(\"{command}\", …) 를 호출하지 않는다"
        );
    }
}

/// ⭐ 이슈 #46 — Keyboards 탭에 복사·복귀·상태·확인 오버레이 DOM 슬롯이 전부
/// 존재한다(계획 §4.2). 기존 id 는 그대로 두고 **추가분 슬롯**만 검사한다.
#[test]
fn settings_html_에_keyboards_탭_복사_복귀_상태_컨트롤이_있다() {
    let html = read_settings_html();
    for id in [
        // 그룹 제목 행 2개(기능 1·기능 2)의 복사 버튼.
        "keyboards-keyremap-copy-btn",
        "keyboards-functionkeys-copy-btn",
        // 상속 복귀(② 숨김), 상태 뱃지(③ inline), 확인 오버레이(D7) + 버튼 2종.
        "keyboards-keyremap-revert-btn",
        "keyboards-keyremap-status-badge",
        "confirm-overlay",
        "confirm-cancel",
        "confirm-continue",
    ] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "settings.html 에 이슈 #46 컨트롤 id=\"{id}\" 가 없다"
        );
    }
}

/// ⭐ 이슈 #69 K1 — 컨텍스트 헤더(공통/개별 식별). 우측 패인 최상단에 현재
/// 선택 대상 이름을 고정 표시하는 DOM 슬롯과 렌더 배선이 있다. 공통/개별이
/// 같은 표현 양식을 공유해 발생한 식별 문제의 첫 단서다.
#[test]
fn settings_html_에_keyboards_컨텍스트_헤더가_배선돼_있다() {
    let html = read_settings_html();
    assert!(
        html.contains(r#"id="keyboards-context-header""#),
        "settings.html 에 컨텍스트 헤더(id=\"keyboards-context-header\")가 없다"
    );
    let f = extract_js_function(&html, "function renderKeyboardsContextHeader(");
    assert!(
        f.contains("preferences.keyboards.devicePicker.forAllDevices"),
        "컨텍스트 헤더가 공통 계층 문구(forAllDevices)를 쓰지 않는다"
    );
    assert!(
        f.contains("d.name"),
        "컨텍스트 헤더가 디바이스 제품명(고유명사)을 쓰지 않는다"
    );
    // 패인 선택이 바뀔 때도 헤더가 갈린다.
    let sel = extract_js_function(&html, "function selectDevice(");
    assert!(
        sel.contains("renderKeyboardsContextHeader"),
        "selectDevice() 가 컨텍스트 헤더를 다시 그리지 않는다 — 디바이스를 바꿔도 \
         헤더가 이전 대상 이름으로 남는다(이슈 #69 K1)"
    );
}

/// ⭐ 이슈 #69 K1 — 뱃지 소속 강도(state-inherited/state-override)가 배선돼
/// 있다. 상속 중은 옅게, 이 키보드 전용은 굵게 — 같은 pill 모양으로 강도만
/// 계층을 따라간다.
#[test]
fn keyboards_상태_뱃지에_소속_강도_클래스가_배선돼_있다() {
    let html = read_settings_html();
    assert!(
        html.contains("state-inherited") && html.contains("state-override"),
        "settings.html 에 뱃지 소속 강도 클래스(state-inherited/state-override)가 없다"
    );
    let f = extract_js_function(&html, "function setKeyboardsStatusBadge(");
    assert!(
        f.contains("classList.add"),
        "setKeyboardsStatusBadge() 가 소속 강도 클래스를 붙이지 않는다"
    );
}

/// ⭐ 이슈 #69 K1-2 — 기능 2 그룹 소속 뱃지. DOM 슬롯 + 4상태 판정
/// (상속/일부 전용/전부 전용/숨김)이 배선돼 있다.
#[test]
fn settings_html_에_keyboards_기능2_상태_뱃지가_배선돼_있다() {
    let html = read_settings_html();
    assert!(
        html.contains(r#"id="keyboards-functionkeys-status-badge""#),
        "settings.html 에 기능 2 상태 뱃지(id=\"keyboards-functionkeys-status-badge\")가 없다"
    );
    let f = extract_js_function(&html, "function renderFunctionKeysStatusBadge(");
    assert!(
        f.contains("preferences.keyboards.status.mixed"),
        "기능 2 뱃지가 status.mixed(일부 전용) 키를 쓰지 않는다"
    );
    assert!(
        f.contains("commonFunctionKeysHasValue"),
        "기능 2 뱃지가 공통 Value 유무(복사 원천)를 보지 않는다 — 원천이 없으면 \
         상태 개념이 성립하지 않아 숨겨야 한다(이슈 #69 K1-2)"
    );
}

/// ⭐ 이슈 #69 K2 — 기능 2 상속 복귀 버튼. DOM 슬롯 + ② 숨김 조건(디바이스
/// 계층에 functionKeys.* 명시적 키가 1개 이상일 때만) + `settings_unset` 경로.
#[test]
fn settings_html_에_keyboards_기능2_복귀_버튼이_배선돼_있다() {
    let html = read_settings_html();
    assert!(
        html.contains(r#"id="keyboards-functionkeys-revert-btn""#),
        "settings.html 에 기능 2 복귀 버튼(id=\"keyboards-functionkeys-revert-btn\")이 없다"
    );

    let f = extract_js_function(&html, "function renderFunctionKeysRevertControl(");
    assert!(
        f.contains("KEYBOARDS_FN_KEYS.some"),
        "기능 2 복귀 버튼이 F-키 12개 명시적 키 유무를 보지 않는다(② 숨김 조건)"
    );

    let handler_at = html
        .find(r#"keyboards-functionkeys-revert-btn").addEventListener("click""#)
        .expect("기능 2 복귀 버튼 클릭 핸들러가 없다");
    let handler = &html[handler_at..handler_at + 400];
    assert!(
        handler.contains("commitUnset(functionKeyStorageKey(currentDevice, fk))"),
        "기능 2 복귀가 settings_unset 경로(commitUnset)를 쓰지 않는다(§3.7.2)"
    );
}

/// ⭐ 이슈 #69 K3 — 기능 1 복귀 버튼이 그룹 제목 행에 있다. 목록 아래가 아니라
/// 뱃지·복사 버튼과 같은 선상에 있어야 한다(§3.7.2 (a)).
#[test]
fn keyboards_기능1_복귀_버튼은_그룹_제목_행에_있다() {
    let html = read_settings_html();
    let title_row_at = html
        .find(r#"id="keyboards-keyremap-heading""#)
        .expect("기능 1 그룹 제목이 없다");
    let list_at = html
        .find(r#"id="keyboards-keyremap-list""#)
        .expect("기능 1 목록 컨테이너가 없다");
    let revert_at = html
        .find(r#"id="keyboards-keyremap-revert-btn""#)
        .expect("기능 1 복귀 버튼이 없다");

    assert!(
        title_row_at < revert_at && revert_at < list_at,
        "기능 1 복귀 버튼이 그룹 제목 행 밖(목록 아래 등)에 있다 — 정책 조작 \
         버튼군(뱃지·복귀·복사)은 한 선상에 모여야 한다(이슈 #69 K3)"
    );
}

/// ⭐ 이슈 #69 K4 — Function Keys 템플릿. DOM 슬롯(팝업 + 적용 버튼)과
/// 템플릿 2종의 F-키 매핑이 배선돼 있다(§3.7.4).
#[test]
fn settings_html_에_keyboards_기능2_템플릿이_배선돼_있다() {
    let html = read_settings_html();
    for id in [
        "keyboards-functionkeys-template-row",
        "keyboards-functionkeys-template-select",
        "keyboards-functionkeys-template-apply-btn",
    ] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "settings.html 에 Function Keys 템플릿 컨트롤(id=\"{id}\")이 없다"
        );
    }

    // 템플릿 표 — §3.7.4 의 12개 목적지 id(맥 미디어키 세트)와 null 12개(표준).
    let f = extract_js_function(&html, "const FUNCTION_KEY_TEMPLATES");
    for dest_id in [
        "appleKeyboard.brightness_down",
        "appleKeyboard.brightness_up",
        "appleKeyboard.mission_control",
        "appleKeyboard.launchpad",
        "consumer.rewind",
        "consumer.play_or_pause",
        "consumer.scan_previous_track",
        "consumer.scan_next_track",
        "consumer.mute",
        "consumer.volume_decrement",
        "consumer.volume_increment",
        "consumer.eject",
    ] {
        assert!(
            f.contains(dest_id),
            "맥 미디어키 세트에 목적지 {dest_id} 가 없다(§3.7.4 표)"
        );
    }
    assert!(
        f.contains("standardFKeys"),
        "F키 표준 세트 템플릿이 없다(§3.7.4 표)"
    );

    // 적용 흐름 — 디바이스 계층의 개별 저장 키에 settings_set 으로 기록한다
    // (새 저장 키 없음, §3.7 원칙).
    let apply_fn = extract_js_function(&html, "async function commitFunctionKeyTemplate(");
    assert!(
        apply_fn.contains(r#"invoke("settings_set""#),
        "템플릿 적용이 settings_set 경로를 쓰지 않는다 — 새 저장 키가 생긴다(§3.7)"
    );
    assert!(
        apply_fn.contains("functionKeyStorageKey(currentDevice, fk)"),
        "템플릿 적용이 디바이스 계층 F-키 저장 키에 기록하지 않는다"
    );
}

/// ⭐ 이슈 #69 K4 + 이슈 #94 — 템플릿 행은 공통("모든 키보드")·디바이스 어느
/// 선택에서도 보인다. 초판(#69)은 디바이스 선택 중에만 보였는데, 공통에
/// 적용하면 `perDevice.all.functionKeys.*` 로 기록되어 **이 앱의 기본 펑션키
/// 세트**가 된다는 것(사용자 요구의 "또 다른 기본 템플릿" — 이슈 #94)을
/// 사용자가 볼 수 있게 하기 위해서다(§3.7.4 표시 조건).
#[test]
fn keyboards_기능2_템플릿_행은_공통_디바이스_어느_선택에서도_보인다() {
    let html = read_settings_html();
    let f = extract_js_function(&html, "function renderFunctionKeysGroup(");
    let row_at = f
        .find("keyboards-functionkeys-template-row")
        .expect("템플릿 행 갱신 코드가 renderFunctionKeysGroup 에 없다");
    let hidden_at = f[row_at..]
        .find("hidden = false")
        .expect("템플릿 행이 공통 선택 중에도 보이도록(항상 unhide) 설정되어 있지 않다(이슈 #94)");
    assert!(
        hidden_at > 0,
        "도달하지 않는다 — 위 find 가 이미 검사했다"
    );
    assert!(
        !f[row_at..].contains("hidden = device === \"all\""),
        "템플릿 행에 디바이스 선택 중에만 보이게 하는 숨김 조건이 아직 남아 있다(이슈 #94 §3.7.4 표시 조건 갱신)"
    );
}

/// ⭐ 이슈 #94 — 템플릿 적용이 공통 계층("모든 키보드")을 허용한다.
/// `applyFunctionKeyTemplate`/`templateNeedsConfirm` 에 `currentDevice === "all"`
/// 가드가 없어야 하고(공통 선택 중에도 적용 가능), `commitFunctionKeyTemplate`
/// 은 공통 계층의 표준 F-키(null)가 부재 키에 쓰이지 않게 하는 저장 억제 규칙을
/// 가져야 하며, 확인 대화상자는 템플릿 전용 카피를 써야 한다(§3.7.4).
#[test]
fn 키보드_기능2_템플릿_적용이_공통_계층을_허용한다() {
    let html = read_settings_html();

    let apply_fn = extract_js_function(&html, "async function applyFunctionKeyTemplate(");
    assert!(
        !apply_fn.contains("currentDevice === \"all\""),
        "공통 선택 중 템플릿 적용을 막는 가드가 남아 있다(이슈 #94)"
    );
    assert!(
        apply_fn.contains("templates.confirmTitle") && apply_fn.contains("templates.confirmBody"),
        "템플릿 전용 확인 카피(templates.confirmTitle/confirmBody)를 쓰지 않는다(이슈 #94)"
    );

    let confirm_fn = extract_js_function(&html, "function templateNeedsConfirm(");
    assert!(
        !confirm_fn.contains("currentDevice === \"all\""),
        "templateNeedsConfirm 에 공통 선택 중 확인을 생략하는 가드가 남아 있다(이슈 #94)"
    );

    let commit_fn = extract_js_function(&html, "async function commitFunctionKeyTemplate(");
    assert!(
        commit_fn.contains("currentDevice === \"all\" && wanted === null"),
        "공통 계층의 표준 F-키(null)가 부재 키에 쓰이지 않게 하는 규칙이 없다(이슈 #94)"
    );
}

/// ⭐ 이슈 #46 — 복사 커맨드가 프런트 invoke 와 백엔드 등록 양쪽에 배선돼 있다
/// (계획 §6.3): settings.html 이 `settings_copy_common_to_device` 를 부르고,
/// main.rs 에 커맨드 선언과 `generate_handler!` 등록이 존재한다.
#[test]
fn 키보드_복사_커맨드가_invoke_배선되어_있다() {
    let html = read_settings_html();
    assert!(
        html.contains(r#"invoke("settings_copy_common_to_device""#),
        "ui/settings.html 이 invoke(\"settings_copy_common_to_device\", …) 를 호출하지 않는다"
    );

    let main_rs = read_main_rs();
    assert!(
        main_rs.contains("fn settings_copy_common_to_device("),
        "main.rs 에 settings_copy_common_to_device 커맨드 선언이 없다"
    );
    assert!(
        main_rs.contains("settings_copy_common_to_device,"),
        "generate_handler! 등록 목록에 settings_copy_common_to_device 가 없다"
    );
}

/// ⭐ 이슈 #47 — 로그 폴더 열기 커맨드가 프런트 invoke 와 백엔드 등록 양쪽에
/// 배선돼 있다(계획 §6.2-c, `키보드_복사_커맨드가_invoke_배선되어_있다` 패턴):
/// settings.html 이 `open_log_folder` 를 부르고, main.rs 에 커맨드 선언과
/// `generate_handler!` 등록이 존재한다.
#[test]
fn 로그_폴더_커맨드가_invoke_배선되어_있다() {
    let html = read_settings_html();
    assert!(
        html.contains(r#"invoke("open_log_folder""#),
        "ui/settings.html 이 invoke(\"open_log_folder\", …) 를 호출하지 않는다"
    );

    let main_rs = read_main_rs();
    assert!(
        main_rs.contains("fn open_log_folder("),
        "main.rs 에 open_log_folder 커맨드 선언이 없다"
    );
    assert!(
        main_rs.contains("open_log_folder,"),
        "generate_handler! 등록 목록에 open_log_folder 가 없다"
    );
}

/// ⭐ 이슈 #46 — 상속 복귀(디바이스 키 삭제)는 기존 `commitUnset` 경로
/// (= `settings_unset` 커맨드)를 그대로 쓴다(계획 D4 — 백엔드 변경 0줄).
#[test]
fn 키보드_상속_복귀는_settings_unset을_쓴다() {
    let html = read_settings_html();
    assert!(
        html.contains(r#"commitUnset(keyRemapRowsKey(currentDevice))"#),
        "복귀 버튼 핸들러가 commitUnset(keyRemapRowsKey(currentDevice)) 경로를 쓰지 않는다"
    );
    let unset_fn = extract_js_function(&html, "async function commitUnset(");
    assert!(
        unset_fn.contains(r#"invoke("settings_unset""#),
        "commitUnset() 이 settings_unset 을 invoke 하지 않는다"
    );
}

/// ⭐ 이슈 #31 ③ — macOS `Use F1, F2…` 연동 두 가지.
///
/// (a) `시스템 설정 열기` 는 Keyboard 최상단이 아니라 **Function Keys 패널**로
///     바로 들어가야 한다. 사용자 보고: "시스템 세팅스를 열었을 때 해당 메뉴로
///     바로 진입하지 않는다".
/// (b) 그 토글의 **현재 값**을 주기적으로 다시 읽어야 한다 — 사용자가 시스템
///     설정 앱에서 바꾼 것은 우리 쪽에 아무 일도 일으키지 않기 때문이다.
#[test]
fn macos_function_keys_패널_딥링크와_폴링이_배선돼_있다() {
    let main_rs = read_main_rs();
    assert!(
        main_rs.contains(
            "x-apple.systempreferences:com.apple.Keyboard-Settings.extension?FunctionKeys"
        ),
        "open_keyboard_settings 가 Function Keys 패널 딥링크를 쓰지 않는다(이슈 #31 ③a)"
    );
    assert!(
        !main_rs.contains("x-apple.systempreferences:com.apple.preference.keyboard"),
        "Ventura 이전 URL 이 남아 있다 — Keyboard 최상단만 열린다(이슈 #31 ③a)"
    );

    let html = read_settings_html();
    assert!(
        html.contains("setInterval(refreshMacosFnStateBadge"),
        "settings.html 이 macOS Function Keys 토글을 주기적으로 다시 읽지 않는다(이슈 #31 ③b)"
    );
    assert!(
        html.contains("syncFnStatePolling()"),
        "탭 전환이 폴링 시작/중지와 연결돼 있지 않다 — 다른 탭에서도 계속 돈다"
    );
}

/// 명세 §4.1 표의 신규 키(탭 라벨 1개 + `preferences.keyboards.*` 나머지)가
/// en.json·ko.json 양쪽에 전부 있다. ⚠️ 탭 라벨 키만 명세 제안(`preferences.tabs.
/// keyboards.title`) 대신 기존 코드베이스 관례(`settings.tab.<탭>`)를 따라
/// `settings.tab.keyboards` 로 잡았다 — settings.html 의 근거 주석 참고.
///
/// ⭐ 이슈 #31 ② — 옛 `functionKeys.function.*` 12종(시스템 기능 8종짜리 어휘)은
/// 목적지 카탈로그 313종/15카테고리로 갈아치워졌다. 그 12종을 15개 카테고리 +
/// `disable` + `unverifiedHint`(17개)로 대체한다 — 24 - 12 + 17 = 29.
///
/// ⭐ 이슈 #46 — 29개 목록이 빠뜨리고 있던 4개(카탈로그·settings.html 에는
/// 존재하는데 이 테스트 목록에만 없던 것: `keyRemap.inheritedFromCommon` ·
/// `macosStatus.on/off/unknown`)를 같은 패스에 보충하고, 복사·복귀·상태 표시
/// 신규 8개를 더한다 — 29 + 4 + 8 = 41(계획 §4.5 "테스트 영향").
///
/// ⭐ 이슈 #69 — 상태 뱃지 `status.mixed`(기능 2 그룹 뱃지의 "일부 전용")와
/// Function Keys 템플릿 4개(`templates.label/.macMediaKeys/.standardFKeys/
/// .apply`)를 더한다 — 41 + 5 = 46.
///
/// ⭐ 이슈 #94 — 템플릿 전용 확인 카피 2개(`templates.confirmTitle/
/// .confirmBody`)를 더한다 — 46 + 2 = 48.
#[test]
fn keyboards_탭_신규_i18n_키_48개가_en_ko_양쪽에_있다() {
    let en = flatten_catalog(&read_en_catalog());
    let ko = flatten_catalog(&read_ko_catalog());

    const KEYS: &[&str] = &[
        "settings.tab.keyboards",
        "preferences.keyboards.devicePicker.forAllDevices",
        "preferences.keyboards.devicePicker.notConnectedSuffix",
        "preferences.keyboards.group.keyRemap.title",
        "preferences.keyboards.group.functionKeys.title",
        "preferences.keyboards.keyRemap.addRow",
        "preferences.keyboards.keyRemap.emptyList",
        // ⭐ 이슈 #46 — 종전 목록에서 누락돼 있던 4개 중 하나(카탈로그·settings.html
        // 에는 존재). 29 → 41 보충 패스에 포함한다.
        "preferences.keyboards.keyRemap.inheritedFromCommon",
        "preferences.keyboards.keyRemap.duplicateFromWarning",
        // ⭐ 이슈 #46 — 신규: 상속 복귀·복사·확인 오버레이·상태 뱃지 계열 8개.
        "preferences.keyboards.keyRemap.revertToCommon",
        "preferences.keyboards.copy.button",
        "preferences.keyboards.copy.confirmTitle",
        "preferences.keyboards.copy.confirmBody",
        "preferences.keyboards.copy.confirmContinue",
        "preferences.keyboards.functionKeys.followCommon",
        "preferences.keyboards.functionKeys.useStandardFKey",
        "preferences.keyboards.functionKeys.macosStatus.label",
        "preferences.keyboards.functionKeys.macosStatus.openButton",
        // ⭐ 이슈 #46 — 누락돼 있던 4개 중 나머지 3개(macOS 상태 표시줄의 3상태).
        "preferences.keyboards.functionKeys.macosStatus.on",
        "preferences.keyboards.functionKeys.macosStatus.off",
        "preferences.keyboards.functionKeys.macosStatus.unknown",
        "preferences.keyboards.functionKeys.disable",
        "preferences.keyboards.functionKeys.unverifiedHint",
        "preferences.keyboards.functionKeys.category.disable",
        "preferences.keyboards.functionKeys.category.modifierKeys",
        "preferences.keyboards.functionKeys.category.controlsAndSymbols",
        "preferences.keyboards.functionKeys.category.arrowKeys",
        "preferences.keyboards.functionKeys.category.letterKeys",
        "preferences.keyboards.functionKeys.category.numberKeys",
        "preferences.keyboards.functionKeys.category.functionKeys",
        "preferences.keyboards.functionKeys.category.mediaControls",
        "preferences.keyboards.functionKeys.category.keypadKeys",
        "preferences.keyboards.functionKeys.category.pcKeyboardKeys",
        "preferences.keyboards.functionKeys.category.internationalKeys",
        "preferences.keyboards.functionKeys.category.applicationLaunchKeys",
        "preferences.keyboards.functionKeys.category.guiApplicationControlKeys",
        "preferences.keyboards.functionKeys.category.remoteControlButtons",
        "preferences.keyboards.functionKeys.category.others",
        // ⭐ 이슈 #46 — 기능 1 그룹 상태 뱃지 3상태.
        "preferences.keyboards.status.inherited",
        "preferences.keyboards.status.override",
        "preferences.keyboards.status.off",
        // ⭐ 이슈 #69 K1-2 — 기능 2 그룹 뱃지의 "일부 전용" 상태.
        "preferences.keyboards.status.mixed",
        // ⭐ 이슈 #69 K4 — Function Keys 템플릿 4개.
        "preferences.keyboards.functionKeys.templates.label",
        "preferences.keyboards.functionKeys.templates.macMediaKeys",
        "preferences.keyboards.functionKeys.templates.standardFKeys",
        "preferences.keyboards.functionKeys.templates.apply",
        // ⭐ 이슈 #94 — 템플릿 전용 확인 카피 2개.
        "preferences.keyboards.functionKeys.templates.confirmTitle",
        "preferences.keyboards.functionKeys.templates.confirmBody",
    ];
    assert_eq!(
        KEYS.len(),
        48,
        "이 목록 자체가 48개가 아니다 — 명세 §4.1 표와 개수를 다시 맞춰라"
    );

    let missing_en: Vec<_> = KEYS.iter().filter(|k| !en.contains(**k)).collect();
    let missing_ko: Vec<_> = KEYS.iter().filter(|k| !ko.contains(**k)).collect();
    assert!(
        missing_en.is_empty(),
        "en.json 에 없는 Keyboards 탭 키: {missing_en:?}"
    );
    assert!(
        missing_ko.is_empty(),
        "ko.json 에 없는 Keyboards 탭 키: {missing_ko:?}"
    );

    // ⭐ 옛 12종이 양쪽 카탈로그에서 완전히 사라졌는지도 함께 고정한다.
    const REMOVED_FUNCTION_KEYS: &[&str] = &[
        "preferences.keyboards.functionKeys.function.displayBrightnessDown",
        "preferences.keyboards.functionKeys.function.displayBrightnessUp",
        "preferences.keyboards.functionKeys.function.missionControl",
        "preferences.keyboards.functionKeys.function.spotlight",
        "preferences.keyboards.functionKeys.function.dictation",
        "preferences.keyboards.functionKeys.function.doNotDisturb",
        "preferences.keyboards.functionKeys.function.rewind",
        "preferences.keyboards.functionKeys.function.playPause",
        "preferences.keyboards.functionKeys.function.fastForward",
        "preferences.keyboards.functionKeys.function.mute",
        "preferences.keyboards.functionKeys.function.volumeDown",
        "preferences.keyboards.functionKeys.function.volumeUp",
    ];
    assert_eq!(
        REMOVED_FUNCTION_KEYS.len(),
        12,
        "삭제 대상 목록 자체가 12개가 아니다"
    );
    let still_in_en: Vec<_> = REMOVED_FUNCTION_KEYS
        .iter()
        .filter(|k| en.contains(**k))
        .collect();
    let still_in_ko: Vec<_> = REMOVED_FUNCTION_KEYS
        .iter()
        .filter(|k| ko.contains(**k))
        .collect();
    assert!(
        still_in_en.is_empty(),
        "en.json 에 옛 function.* 키가 아직 남아 있다: {still_in_en:?}"
    );
    assert!(
        still_in_ko.is_empty(),
        "ko.json 에 옛 function.* 키가 아직 남아 있다: {still_in_ko:?}"
    );
}

/// `settings.html` 이 문자열 리터럴로 참조하는 모든 `preferences.keyboards.*` 키가
/// en.json·ko.json 양쪽에 있다 — 일반 검사(`settings_html_의_settings_점_리터럴은_
/// 전부_카탈로그_키다`)는 `settings.*` 접두사만 보므로, 이 탭이 쓰는 다른 접두사를
/// 여기서 별도로 잡는다(Korean 탭의 동일 패턴 재사용).
#[test]
fn settings_html_의_preferences_keyboards_점_리터럴이_en_ko_양쪽_카탈로그에_모두_있다() {
    let html = read_settings_html();
    let en_keys = flatten_catalog(&read_en_catalog());
    let ko_keys = flatten_catalog(&read_ko_catalog());

    let keyboards_keys: BTreeSet<_> = extract_double_quoted_literals(&html)
        .into_iter()
        .filter(|s| s.starts_with("preferences.keyboards."))
        .collect();

    assert!(
        !keyboards_keys.is_empty(),
        "settings.html 에서 \"preferences.keyboards.*\" 리터럴을 하나도 찾지 못했다 — \
         Keyboards 탭 배선이 빠졌을 수 있다"
    );

    let missing_en: Vec<_> = keyboards_keys
        .iter()
        .filter(|k| !en_keys.contains(k.as_str()))
        .collect();
    let missing_ko: Vec<_> = keyboards_keys
        .iter()
        .filter(|k| !ko_keys.contains(k.as_str()))
        .collect();

    assert!(
        missing_en.is_empty(),
        "en.json 에 없는 preferences.keyboards.* 키: {missing_en:?}"
    );
    assert!(
        missing_ko.is_empty(),
        "ko.json 에 없는 preferences.keyboards.* 키: {missing_ko:?}"
    );
}

// ⛔ `settings_html_문서가_keyboards_탭_창_크기를_잠정값으로_밝힌다` (예전 테스트) —
// 이슈 #32 Phase 1 에서 탭별 창 크기 테이블 자체를 없앴으므로 "Keyboards 탭만
// 잠정값"이라는 전제가 더는 성립하지 않는다. 걷어냈다. `Keyboards` 665px 는 이제
// `SETTINGS_WINDOW_DEFAULT` 의 실측 목록(2026-08-30) 중 하나로 다른 5개 탭과
// 동등하게 실측되어 있다 — main.rs 의 그 상수 doc 주석 참고.

// ⭐ F-01(seek-activation-and-session.md) — `Seek` 탭 재발 방지 테스트.
// Korean 탭 재발 방지 테스트(위)와 같은 검증 방식을 재사용한다.

fn read_flags_rs() -> String {
    // crates/ 는 이 워크트리에서 다른 세션이 동시에 작업 중이라 손대지 않는다
    // — 여기서는 읽기만 한다. 경로는 워크스페이스 루트 기준.
    let path = manifest_dir().join("../../crates/ultrakey-core/src/flags.rs");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("crates/ultrakey-core/src/flags.rs 를 읽지 못했다({path:?}): {e}"))
}

/// `needle` 바로 뒤에 나오는 첫 `0x…` 16진수 리터럴 값을 읽는다. JS 상수 선언
/// (`const NAME = 0x…;`)과 Rust 상수 선언(`pub const NAME: EventFlags =
/// EventFlags(0x…)`) 양쪽에 같은 방식으로 쓴다.
fn extract_hex_const_value(text: &str, needle: &str) -> u64 {
    let pos = text
        .find(needle)
        .unwrap_or_else(|| panic!("`{needle}` 를 찾지 못했다"));
    let after = &text[pos + needle.len()..];
    let hex_start = after
        .find("0x")
        .unwrap_or_else(|| panic!("`{needle}` 뒤에서 0x 리터럴을 찾지 못했다"));
    let hex_digits = &after[hex_start + 2..];
    let end = hex_digits
        .find(|c: char| !c.is_ascii_hexdigit())
        .unwrap_or(hex_digits.len());
    u64::from_str_radix(&hex_digits[..end], 16)
        .unwrap_or_else(|e| panic!("`{needle}` 뒤의 16진수 파싱 실패: {e}"))
}

/// `Seek` 탭 버튼·패널과 5개 `data-key` 가 전부 `settings.html` 에 존재한다.
/// ⭐ F-04(이슈 #44) — `seek.focusWindowBeforeClicking`·`seek.changeClickModes
/// WithModifiers` 체크박스 2개가 추가됐다(3 → 5).
#[test]
fn settings_html_에_seek_탭_버튼_패널_5개_data_key가_있다() {
    let html = read_settings_html();

    assert!(
        html.contains(r#"id="tab-seek" data-tab="seek""#),
        "settings.html 에 Seek 탭 버튼(id=\"tab-seek\")이 없다"
    );
    assert!(
        html.contains(r#"id="panel-seek""#),
        "settings.html 에 Seek 탭 패널(id=\"panel-seek\")이 없다"
    );

    for data_key in [
        "seek.remapKey",
        "seek.executeOnClose",
        "seek.semicolonCycle",
        "seek.focusWindowBeforeClicking",
        "seek.changeClickModesWithModifiers",
    ] {
        assert!(
            html.contains(&format!("data-key=\"{data_key}\"")),
            "settings.html 에 data-key=\"{data_key}\" 컨트롤이 없다"
        );
    }
}

/// 단축키 레코더 버튼(녹음 시작)과 지우기 버튼이 둘 다 존재한다.
#[test]
fn settings_html_에_seek_단축키_레코더_버튼_2종이_있다() {
    let html = read_settings_html();

    assert!(
        html.contains(r#"id="seek-shortcut-record""#),
        "settings.html 에 단축키 레코더 버튼(id=\"seek-shortcut-record\")이 없다"
    );
    assert!(
        html.contains(r#"id="seek-shortcut-clear""#),
        "settings.html 에 단축키 지우기 버튼(id=\"seek-shortcut-clear\")이 없다"
    );
}

/// `settings.seek.*` 리터럴이 en·ko 양쪽 카탈로그에 전부 있다. (일반 검사는
/// en 만 보므로, Korean 탭과 같은 이유로 여기서 별도로 잡는다.)
#[test]
fn seek_점_리터럴이_en_ko_양쪽_카탈로그에_모두_있다() {
    let html = read_settings_html();
    let en_keys = flatten_catalog(&read_en_catalog());
    let ko_keys = flatten_catalog(&read_ko_catalog());

    let seek_keys: BTreeSet<_> = extract_double_quoted_literals(&html)
        .into_iter()
        .filter(|s| s.starts_with("settings.seek."))
        .collect();

    assert!(
        !seek_keys.is_empty(),
        "settings.html 에서 \"settings.seek.*\" 리터럴을 하나도 찾지 못했다 — \
         Seek 탭 배선이 빠졌을 수 있다"
    );

    let missing_en: Vec<_> = seek_keys
        .iter()
        .filter(|k| !en_keys.contains(k.as_str()))
        .collect();
    let missing_ko: Vec<_> = seek_keys
        .iter()
        .filter(|k| !ko_keys.contains(k.as_str()))
        .collect();

    assert!(
        missing_en.is_empty(),
        "en.json 에 없는 settings.seek.* 키: {missing_en:?}"
    );
    assert!(
        missing_ko.is_empty(),
        "ko.json 에 없는 settings.seek.* 키: {missing_ko:?}"
    );
}

/// `Seek` 탭이 자리표시자에서 실제 탭으로 바뀌었다 — 예전 자리표시자 문구
/// (`settings.placeholder.seek.body`)가 더 이상 HTML 에 없고, 카탈로그
/// 양쪽에서도 지워졌다.
#[test]
fn settings_html_에서_seek_placeholder가_사라졌다() {
    let html = read_settings_html();
    assert!(
        !html.contains("settings.placeholder.seek.body"),
        "settings.html 에 옛 Seek 자리표시자 리터럴(settings.placeholder.seek.body)이 \
         여전히 남아 있다 — Seek 탭이 실제 탭으로 바뀌었으면 이 리터럴은 없어야 한다"
    );

    let en = read_en_catalog();
    let ko = read_ko_catalog();
    for old_key in ["settings.placeholder.title", "settings.placeholder.seek.body"] {
        assert!(
            en.get(old_key).is_none(),
            "en.json 에 옛 Seek 자리표시자 키 {old_key} 가 남아 있다"
        );
        assert!(
            ko.get(old_key).is_none(),
            "ko.json 에 옛 Seek 자리표시자 키 {old_key} 가 남아 있다"
        );
    }
}

/// ⭐ M2-2 사고 재발 방지 — 기대값을 테스트 안에 다시 적지 않고, `flags.rs` 를
/// 직접 읽어 JS 상수와 대조한다. `settings.html` 의 단축키 레코더가 쓰는
/// modifier 비트마스크 4개가 `crates/ultrakey-core/src/flags.rs` 의
/// `EventFlags` 상수와 정확히 일치해야 한다(option 은 JS 쪽 이름이 OPTION,
/// Rust 쪽 이름이 ALTERNATE 로 다를 뿐 같은 비트를 가리켜야 한다).
#[test]
fn seek_modifier_비트값이_ultrakey_core_flags와_일치한다() {
    let html = read_settings_html();
    let flags_rs = read_flags_rs();

    let js_shift = extract_hex_const_value(&html, "SEEK_SHORTCUT_MOD_SHIFT =");
    let js_control = extract_hex_const_value(&html, "SEEK_SHORTCUT_MOD_CONTROL =");
    let js_option = extract_hex_const_value(&html, "SEEK_SHORTCUT_MOD_OPTION =");
    let js_command = extract_hex_const_value(&html, "SEEK_SHORTCUT_MOD_COMMAND =");

    let rs_shift = extract_hex_const_value(&flags_rs, "pub const SHIFT: EventFlags = EventFlags(");
    let rs_control = extract_hex_const_value(&flags_rs, "pub const CONTROL: EventFlags = EventFlags(");
    let rs_option = extract_hex_const_value(&flags_rs, "pub const ALTERNATE: EventFlags = EventFlags(");
    let rs_command = extract_hex_const_value(&flags_rs, "pub const COMMAND: EventFlags = EventFlags(");

    assert_eq!(
        js_shift, rs_shift,
        "settings.html 의 SEEK_SHORTCUT_MOD_SHIFT 가 flags.rs 의 EventFlags::SHIFT 와 다르다"
    );
    assert_eq!(
        js_control, rs_control,
        "settings.html 의 SEEK_SHORTCUT_MOD_CONTROL 이 flags.rs 의 EventFlags::CONTROL 과 다르다"
    );
    assert_eq!(
        js_option, rs_option,
        "settings.html 의 SEEK_SHORTCUT_MOD_OPTION 이 flags.rs 의 EventFlags::ALTERNATE 와 다르다"
    );
    assert_eq!(
        js_command, rs_command,
        "settings.html 의 SEEK_SHORTCUT_MOD_COMMAND 가 flags.rs 의 EventFlags::COMMAND 와 다르다"
    );
}

// ============================================================================
// D6 — `General` 탭 언어 선택 + Launch on login 수정(이슈 #39,
// `localization-and-input-sources.md` §3.1.2-a, `menu-bar-and-lifecycle.md`
// §3.5-a).
// ============================================================================

/// 이번 회차가 5개 카탈로그 전부에 새로 넣은 키 — `settings_html_의_settings_점_
/// 리터럴은_전부_카탈로그_키다`(en 만 봄)와 `ultrakey-i18n` 의 자체 키 집합 동일성
/// 테스트가 이미 이걸 간접적으로 보장하지만, ⚠️ 이 위임은 "5개 카탈로그의 키
/// 집합이 정확히 같아야 한다"가 수용 기준이라 여기서도 명시적으로 5개 전부를
/// 직접 확인한다 — 이 테스트만 보고도 회귀를 바로 알 수 있어야 한다.
const NEW_KEYS_FOR_ISSUE_39: &[&str] = &[
    "settings.general.language",
    "settings.general.language.system",
    "settings.general.language.hint",
    "settings.general.launch_on_login.requires_approval",
    "menu.launch_on_login.requires_approval",
];

#[test]
fn 이슈_39_신규_카탈로그_키가_다섯_카탈로그_모두에_있다() {
    for locale in LOCALES {
        let keys = flatten_catalog(&read_catalog(locale));
        let missing: Vec<_> = NEW_KEYS_FOR_ISSUE_39
            .iter()
            .filter(|k| !keys.contains(**k))
            .collect();
        assert!(
            missing.is_empty(),
            "{locale}.json 에 없는 이슈 #39 신규 키: {missing:?}"
        );
    }
}

/// A-4 — `General` 탭에 언어 선택 팝업이 있다.
#[test]
fn settings_html_에_general_language_select가_있다() {
    let html = read_settings_html();
    assert!(
        html.contains(r#"id="general-language""#),
        "settings.html 에 <select id=\"general-language\"> 가 없다"
    );
    // ⭐ §3.1.2-a — endonym 목록은 프런트에 하드코딩하지 않고 백엔드가 보내는
    // `bootstrap.languages` 로 채운다. `data-key` 로 범용 commit() 위임 경로에
    // 태우면 "System" 선택이 `null`(키 삭제)이 아니라 문자열 "system" 으로
    // 저장되므로, 이 select 에는 `data-key` 가 없어야 한다(전용 change 리스너를
    // 따로 둔다).
    assert!(
        !html.contains(r#"id="general-language" data-key"#),
        "general-language select 에 data-key 가 붙어 있다 — 전용 change 리스너와 \
         충돌한다(범용 commit() 경로로 새면 \"System\" 이 키 삭제가 아니라 문자열로 저장된다)"
    );
    assert!(
        html.contains("bootstrap.languages"),
        "settings.html 이 settings_bootstrap 응답의 languages 필드를 쓰지 않는다"
    );
}

/// B-1/B-4 — `main.rs` 가 `general.language` 키를 다룬다.
#[test]
fn main_rs_가_general_language_키를_다룬다() {
    let main_rs = read_main_rs();
    assert!(
        main_rs.contains("settings_keys::GENERAL_LANGUAGE"),
        "main.rs 에 general.language 저장 키 상수 사용이 없다"
    );
    assert!(
        main_rs.contains(r#""general.language""#),
        "main.rs 에 \"general.language\" 문자열 리터럴이 없다"
    );
}

/// B-1 — `general_view` 가 저장된 거울이 아니라 `login_item::status()`(OS 정본)를
/// 읽는다. 문자열 검사로 충분하다(§3.5-a 결정 1 — "정본은 언제나 OS 다").
#[test]
fn general_view가_login_item_status를_읽는다() {
    let main_rs = read_main_rs();
    let start = main_rs
        .find("fn general_view(")
        .expect("main.rs 에 general_view 함수가 없다");
    // 다음 함수 정의 전까지만 잘라 본다(대략의 함수 본문 범위).
    let body = &main_rs[start..];
    let end = body[10..]
        .find("\nfn ")
        .map(|i| i + 10)
        .unwrap_or(body.len());
    let body = &body[..end];
    assert!(
        body.contains("login_item::status()"),
        "general_view 가 login_item::status() 를 읽지 않는다 — 저장된 거울만 보고 있다"
    );
}

// ============================================================================
// 이슈 #39 Phase 2·3 — 설정 export/import + Event Viewer
// (`settings-store-and-integrity.md` §3.4, `event-viewer.md`).
// ============================================================================

/// A-4·B-3 — 이번 회차가 5개 카탈로그 전부에 새로 넣은 키 13개(export/import 8개 +
/// Event Viewer 5개). 위임 지시서의 수용 기준이 "키 집합이 정확히 같아야 한다"라
/// 기존 `NEW_KEYS_FOR_ISSUE_39`(D6, 앞선 회차)와 별도 목록으로 명시적으로 잡는다.
/// ⭐ 이슈 #77 — `settings.general.diagnostics`(진단 헤딩 키)는 여기서 제외했다.
/// Advanced `<summary>` 가 헤딩을 겸하면서 키가 삭제됐다(계획 §2 D4·D6).
const NEW_KEYS_FOR_ISSUE_39_PHASE_2_3: &[&str] = &[
    "settings.general.transfer",
    "settings.general.export",
    "settings.general.import",
    "settings.general.transfer.hint",
    "settings.general.export.done",
    "settings.general.import.done",
    "settings.general.import.absent_devices",
    "settings.general.import.failed",
    "settings.general.event_viewer",
    "settings.general.event_viewer.hint",
    "eventviewer.notice.no_permission",
    "eventviewer.notice.engine_not_running",
];

#[test]
fn 이슈_39_phase_2_3_신규_카탈로그_키가_다섯_카탈로그_모두에_있다() {
    for locale in LOCALES {
        let keys = flatten_catalog(&read_catalog(locale));
        let missing: Vec<_> = NEW_KEYS_FOR_ISSUE_39_PHASE_2_3
            .iter()
            .filter(|k| !keys.contains(**k))
            .collect();
        assert!(
            missing.is_empty(),
            "{locale}.json 에 없는 이슈 #39 Phase 2·3 신규 키: {missing:?}"
        );
    }
}

/// ⭐ 이슈 #47 — 이번 회차가 5개 카탈로그 전부에 새로 넣은 키 3개(섹션 헤딩
/// `startup`·`license` 2개 + 로그 폴더 버튼 1개). 위 `NEW_KEYS_FOR_ISSUE_39_*` 와
/// 같은 이유로 별도 목록으로 명시적으로 잡는다. ⚠️ `settings.general.license` 는
/// 기존 `settings.general.license.why`(구매·해제 why 문구)와 **별개 키**로 공존한다
/// — 카탈로그는 평평한 키-값 맵이라 충돌하지 않는다(계획 §2 D2).
const NEW_KEYS_FOR_ISSUE_47: &[&str] = &[
    "settings.general.startup",
    "settings.general.license",
    "settings.general.open_log_folder",
];

#[test]
fn 이슈_47_신규_카탈로그_키가_다섯_카탈로그_모두에_있다() {
    for locale in LOCALES {
        let keys = flatten_catalog(&read_catalog(locale));
        let missing: Vec<_> = NEW_KEYS_FOR_ISSUE_47
            .iter()
            .filter(|k| !keys.contains(**k))
            .collect();
        assert!(
            missing.is_empty(),
            "{locale}.json 에 없는 이슈 #47 신규 키: {missing:?}"
        );
    }
}

/// A-3 — `General` 탭에 export/import 버튼이 있고, 대응 커맨드를 invoke 한다.
/// 파일 대화상자는 Rust 쪽에서만 연다는 결정(위임 지시서 A-1)의 프런트 쪽
/// 증거이기도 하다 — `settings.html` 에는 `tauri_plugin_dialog` 관련 문자열이
/// 전혀 없어야 정상이다.
#[test]
fn settings_html_에_export_import_이벤트뷰어_버튼이_있고_커맨드를_invoke_한다() {
    let html = read_settings_html();
    // ⭐ 이슈 #47 — `open-log-folder-btn`(로그 폴더 열기) 추가. 진단 버튼이라
    // open-event-viewer-btn 과 같은 목록에 둔다.
    for id in [
        "settings-export-btn",
        "settings-import-btn",
        "open-event-viewer-btn",
        "open-log-folder-btn",
    ] {
        assert!(
            html.contains(&format!(r#"id="{id}""#)),
            "settings.html 에 <button id=\"{id}\"> 가 없다"
        );
    }
    for command in [
        "settings_export",
        "settings_import",
        "open_event_viewer",
        "open_log_folder",
    ] {
        assert!(
            html.contains(&format!("invoke(\"{command}\"")),
            "ui/settings.html 이 invoke(\"{command}\", …) 를 호출하지 않는다"
        );
    }
    assert!(
        !html.contains("plugin:dialog"),
        "settings.html 이 tauri_plugin_dialog 를 웹뷰에서 직접 부른다 — \
         Rust 쪽(settings_export/settings_import 커맨드)에서만 열어야 한다(위임 지시서 A-1)"
    );
}

/// ⭐ 이슈 #47 — General 탭이 `<hr />` 4개 + 섹션 헤딩 4개로 묶인다(계획 §2 D1):
/// 언어(무헤딩) → Startup & menu bar → License → 설정 파일 → 진단.
///
/// ⭐ 이슈 #77(D4) — 진단 `#general-diagnostics-heading`(h2.group)은 제거됐고
/// Advanced `<details>` 의 `<summary>` 가 그 헤딩을 겸한다. `<details>` 는 `<hr />`
/// 다음에 들어오므로 `<hr />` 개수 4 단언은 그대로 성립한다.
///
/// hr 카운트는 **panel-general 스코프로 한정**한다 — 전체 HTML 에서 세면 다른
/// 탭 수정(Presets·Hyperkey 의 hr)에 취약하다. 또 스코프 추출 후 HTML 주석을
/// 제거하고 센다(이 줄 위 한국어 주석 안의 `<hr />` 산문이 카운트를 흔들 수
/// 있다 — 이 파일의 "문자열을 못 믿고 스스로 방어한다" 정신). 헤딩 id 는
/// 부분 문자열 오탐을 피해 `id="{id}"` 꼴로 검사한다.
#[test]
fn general_탭에_섹션_헤딩과_구분선이_있다() {
    let html = read_settings_html();
    let start = html
        .find(r#"<section id="panel-general""#)
        .expect("settings.html 에 <section id=\"panel-general\"> 가 없다");
    let end = html[start..]
        .find("</section>")
        .expect("panel-general 이 </section> 로 닫히지 않는다")
        + start;
    let panel = strip_html_comments(&html[start..end]);

    let hr_count = panel.matches("<hr").count();
    assert_eq!(
        hr_count, 4,
        "panel-general 안의 <hr /> 개수가 4 가 아니다(주석 제거 후 카운트)"
    );

    for id in [
        "general-startup-heading",
        "general-license-heading",
        "general-transfer-heading",
    ] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "settings.html 에 섹션 헤딩 id=\"{id}\" 가 없다"
        );
    }
}

/// B-1 — `eventviewer.html` 이 `eventviewer_poll`·`eventviewer_clear` 를 invoke 한다.
#[test]
fn eventviewer_html이_eventviewer_poll과_clear를_invoke_한다() {
    let html = read_eventviewer_html();
    for command in ["eventviewer_poll", "eventviewer_clear"] {
        assert!(
            html.contains(&format!("invoke(\"{command}\"")),
            "ui/eventviewer.html 이 invoke(\"{command}\", …) 를 호출하지 않는다"
        );
    }
}

/// B-1 — `main.rs` 의 프리셋/한국어 규칙 번호 ↔ 이름 대응표(`PRESET_RULE_LABEL_KEYS`·
/// `KOREAN_RULE_LABEL_KEYS`)가 가리키는 카탈로그 키가 실제로 존재한다.
///
/// ⭐ `event-viewer.md` §9 항목 1 이 지적한 위험 — "이 대응이 깨져도 아무도 못
/// 잡는다" — 을 막는 테스트다. `main.rs` 안에서 `"settings.presets.*"`·
/// `"settings.korean.*"`·`"settings.tab.hyperkey"` 꼴의 문자열 리터럴을 전부
/// 긁어 5개 카탈로그 모두에 있는지 확인한다 — 대응표 두 개만이 아니라 같은
/// 접두사를 쓰는 다른 위치(예: 충돌 대화상자 라벨)의 오타도 함께 잡는 부수효과가
/// 있지만, 그것도 이 스캔의 의도에 부합한다("카탈로그의 실제 키를 가리킨다").
#[test]
fn main_rs의_프리셋_한국어_규칙_라벨_키가_카탈로그에_실재한다() {
    let main_rs = read_main_rs();
    let no_comments = strip_line_comments(&main_rs);
    let literals: Vec<String> = extract_double_quoted_literals(&no_comments)
        .into_iter()
        .filter(|s| {
            s.starts_with("settings.presets.") || s.starts_with("settings.korean.") || s == "settings.tab.hyperkey"
        })
        .collect();

    // 새 대응표가 실제로 이 스캔에 걸리는지부터 확인한다 — 추출 로직 자체가
    // 깨지면(예: 상수 배열 문법이 바뀌면) 아래 검증이 공허하게 통과해 버린다.
    for must_have in [
        "settings.presets.caps_wasd",
        "settings.presets.remap_delete",
        "settings.korean.won_backtick",
        "settings.tab.hyperkey",
    ] {
        assert!(
            literals.iter().any(|s| s == must_have),
            "main.rs 스캔에서 \"{must_have}\" 를 찾지 못했다 — 추출 로직이 깨졌을 수 있다"
        );
    }

    for locale in LOCALES {
        let keys = flatten_catalog(&read_catalog(locale));
        let missing: Vec<_> = literals.iter().filter(|k| !keys.contains(k.as_str())).collect();
        assert!(
            missing.is_empty(),
            "{locale}.json 에 없는, main.rs 의 규칙 라벨 카탈로그 키: {missing:?}"
        );
    }
}

// ⭐ F-03(이슈 #67) — 검색 바 글자색 재발 방지 테스트.
//
// 이번 결함의 본질: `applyPalette()` 는 `#root` 요소에만 CSS 변수를 inline 으로
// 세팅하는데, 글자색 `color: var(--bar-fg)` 선언이 `html, body` 쪽에 있었다.
// `var()` 는 선언이 붙은 요소 기준으로 해석되므로 JS 가 갱신한 값이 글자색에
// 닿지 않고, 첫 페인트용 폴백(흰색)이 영원히 쓰였다 — 라이트 테마에서 흰 배경
// 위 흰 글자가 됐다(다크에서는 폴백 흰색이 다크 팔레트와 우연히 같아
// 정상처럼 보였다). 아래 두 테스트가 "글자색 선언은 #root 에, JS 매핑은
// bar_fg 를 갱신한다"는 쌍을 지킨다.

/// 검색바 HTML 에서 `selector` 로 시작하는 CSS 규칙 본문(`{ ... }` 내부)을
/// 찾는다. 이 파일의 방어적 스캔 스타일과 같은 원리의 최소 추출기다.
fn css_rule_body(html: &str, selector: &str) -> Option<String> {
    let pos = html.find(selector)?;
    let brace = html[pos..].find('{')?;
    let end = html[pos + brace..].find('}')?;
    Some(html[pos + brace + 1..pos + brace + end].to_string())
}

/// ⭐ 이슈 #67 — 글자색 `color: var(--bar-fg)` 는 `#root` 규칙 안에 있어야 한다.
///
/// `applyPalette()` 가 `root.style.setProperty("--bar-fg", …)` 로 갱신하는
/// 변수가 실제 텍스트에 닿으려면, 그 변수를 쓰는 `color` 선언이 **그 요소
/// 자신(또는 자손)** 에 붙어 있어야 한다. `html, body` 로 돌아가면 폴백이
/// 영원히 쓰인다(위 파일 헤더 주석의 결함 재발).
#[test]
fn searchbar의_글자색_선언은_root_규칙_안에_있다() {
    let html = read_searchbar_html();

    let root_rule = css_rule_body(&html, "#root {")
        .unwrap_or_else(|| panic!("overlay-searchbar.html 에 #root 규칙이 없다"));
    assert!(
        root_rule.contains("color: var(--bar-fg)") || root_rule.contains("color:var(--bar-fg)"),
        "overlay-searchbar.html 의 #root 규칙이 color: var(--bar-fg) 를 선언하지 \
         않는다. 글자색 선언을 html, body 로 옮기면 applyPalette() 가 갱신한 값이 \
         닿지 않아 첫 페인트 폴백(흰색)이 영원히 쓰인다 — 이슈 #67 의 결함 그대로다."
    );

    // html/body 규칙 쪽에는 글자색 선언이 없어야 한다 — 있으면 #root 가 없는
    // 텍스트가 폴백 계보를 따라 흰 글자로 남는 경로가 다시 생긴다.
    let body_rule = css_rule_body(&html, "html, body {")
        .unwrap_or_else(|| panic!("overlay-searchbar.html 에 html, body 규칙이 없다"));
    assert!(
        !body_rule.contains("var(--bar-fg)"),
        "overlay-searchbar.html 의 html, body 규칙이 var(--bar-fg) 를 쓴다 — \
         JS 가 #root 에만 세팅하는 값이 이 자리에는 닿지 않는다(이슈 #67)."
    );
}

/// ⭐ 이슈 #67 — JS 매핑이 실제로 `bar_fg` 를 `--bar-fg` 로 갱신하는지.
///
/// 팔레트 전달(이벤트)이 정상이어도 매핑 키가 어긋나면 같은 증상이 난다.
/// `SearchBarFrame.palette` 는 serde 기본(스네이크 케이스)으로 직렬화되므로
/// 키는 `bar_fg` 다.
#[test]
fn searchbar의_apply_팔레트는_bar_fg를_갱신한다() {
    let html = read_searchbar_html();
    assert!(
        html.contains("bar_fg: \"--bar-fg\""),
        "overlay-searchbar.html 의 PALETTE_VAR_MAP 에 bar_fg → --bar-fg 매핑이 \
         없다 — SearchBarFrame.palette.bar_fg 가 검색바 글자색에 반영되지 않는다"
    );
}

/// ⭐(이슈 #102) — 검색 바 HTML 의 카운터 분기 블록을 추출한다.
///
/// `const total = payload.total_matches` 부터 if/else 체인의 닫는 `}` 까지다.
/// 이 파일의 방어적 스캔 스타일(`css_rule_body`)과 같은 원리의 최소 추출기다.
fn searchbar_counter_block(html: &str) -> Option<String> {
    let start = html.find("const total = payload.total_matches")?;
    let tail = &html[start..];
    // 마지막 분기(`- / " + total;`)의 `counter.textContent` 뒤 첫 `}` 가
    // 카운터 if/else 체인의 끝이다(그 사이에 중괄호가 없다).
    let last_branch = tail.find("counter.textContent = \"- / \" + total;")?;
    let close = tail[last_branch..].find('}')?;
    Some(tail[..last_branch + close].to_string())
}

/// ⭐ 이슈 #102 — 검출 중 카운터가 "찾는 중…"을 표시한다(분기 순서 고정).
///
/// 이번 결함의 본질: 카운터가 `total <= 0` 을 먼저 판정해, 검출이 아직 진행
/// 중(한국어 OCR 최대 약 3.1s)인데도 "없음"(= 검색 완료 + 0개)을 내보냈다 —
/// 사용자가 "프로그램이 동작하지 않는다"고 오독했다. 분기의 **존재**만 단언하면
/// 순서가 뒤집혀 detecting 분기가 도달 불가로 죽는 회귀를 잡지 못하므로,
/// 추출한 블록 안에서 판정 **순서**까지 고정한다.
#[test]
fn searchbar_카운터는_검출_중_찾는중을_표시한다() {
    let html = read_searchbar_html();
    let block = searchbar_counter_block(&html).unwrap_or_else(|| {
        panic!("overlay-searchbar.html 에 카운터 블록(const total = payload.total_matches)이 없다")
    });

    assert!(
        block.contains("찾는 중"),
        "overlay-searchbar.html 카운터 블록에 검출 중 문구(\"찾는 중…\")가 없다 — 이슈 #102"
    );
    let detecting_at = block
        .find("payload.detecting")
        .unwrap_or_else(|| panic!("카운터 블록이 payload.detecting 을 읽지 않는다 — 이슈 #102"));
    let no_matches_at = block
        .find("total <= 0")
        .unwrap_or_else(|| panic!("카운터 블록에 total <= 0 분기가 없다 — 확정 '없음' 판정 소실"));
    assert!(
        detecting_at < no_matches_at,
        "카운터 블록에서 payload.detecting 판정이 total <= 0 보다 뒤다 — \
         검출 중에도 '없음'이 나오는 이슈 #102 결함 재발. detecting 분기가 먼저여야 한다"
    );
    assert!(
        block.contains("\"없음\""),
        "overlay-searchbar.html 카운터 블록에 검출 완료 후 확정 문구(\"없음\")가 없다"
    );
}

// ============================================================================
// 이슈 #79 — UI/UX 1차 현대화(P0) 재발 방지 테스트.
//
// P0 전제(컨트롤 id·저장 키·카탈로그 키·invoke 배선 무변경)는 이 파일의 기존
// 테스트 전부가 이미 지킨다. 여기서는 **새로 들어간 시각 규약 자체**가 다시
// 사라지거나 후퇴하는 회귀만 잡는다 — 정적 스캔이라 "효과"가 아니라 "배선 존재"
// 를 검증하는 최소 단언이다.
// ============================================================================

/// UXR-01 — 선택 상태 표현이 시스템 강조색(Highlight) tint 를 쓴다. 사이드바
/// 탭과 Keyboards 디바이스 목록 둘 다에서 회색(currentColor)으로 후퇴하면 잡는다.
#[test]
fn settings_html_의_선택_상태는_highlight_tint를_쓴다() {
    let html = read_settings_html();

    let sidebar_rule = css_rule_body(&html, ".sidebar button[aria-selected=\"true\"] {")
        .unwrap_or_else(|| panic!("settings.html 에 .sidebar button[aria-selected=true] 규칙이 없다"));
    assert!(
        sidebar_rule.contains("Highlight"),
        "사이드바 선택 상태가 Highlight(CSS 시스템 강조색) tint 를 쓰지 않는다 — \
         currentColor 회색으로 후퇴하면 hover 와 구분이 약해진다(UXR-01)"
    );

    let pane_rule = css_rule_body(&html, ".keyboards-device-pane [role=\"option\"][aria-selected=\"true\"] {")
        .unwrap_or_else(|| panic!("settings.html 에 디바이스 목록 선택 상태 규칙이 없다"));
    assert!(
        pane_rule.contains("Highlight"),
        "Keyboards 디바이스 목록 선택 상태가 Highlight tint 를 쓰지 않는다(UXR-01)"
    );
}

/// UXR-06 — `#seek-not-configured` 가 일반 부제가 아니라 `hint warn`(경고
/// 표현)으로 마크업되어 있다. `renderSeek()` 의 hidden 토글(id·문구 무변경)을
/// 건드리지 않으므로, 클래스만 확인하면 충분하다.
#[test]
fn settings_html_의_seek_미설정_경고는_warn_클래스를_가진다() {
    let html = read_settings_html();
    let needle = r#"<p class="hint warn" id="seek-not-configured""#;
    assert!(
        html.contains(needle),
        "settings.html 의 #seek-not-configured 가 class=\"hint warn\" 이 아니다 — \
         활성화 경로 부재 알림이 일반 부제로 후퇴하면 구분이 사라진다(UXR-06)"
    );
}

/// UXR-07 — 탭 패널 페이드 인의 배선(rAF + 인라인 opacity 0 → 지움)과
/// 감소 모션 대응이 남아 있는지. `activateTab` 의 invoke·hidden 토글 무변경은
/// `최초_렌더는_탭을_저장하지_않는다` 등 기존 테스트가 이미 지킨다.
#[test]
fn settings_html_에_탭_전환_페이드_배선이_있다() {
    let html = read_settings_html();
    assert!(
        html.contains("function revealTabPanel("),
        "settings.html 에 탭 페이드 헬퍼(revealTabPanel)가 없다(UXR-07)"
    );
    assert!(
        html.contains("requestAnimationFrame"),
        "settings.html 의 탭 페이드가 requestAnimationFrame 을 쓰지 않는다(UXR-07)"
    );
    assert!(
        html.contains("prefers-reduced-motion: reduce"),
        "settings.html 의 탭 페이드가 감소 모션 대응(prefers-reduced-motion)을 갖지 않는다(UXR-07)"
    );
}

// ─────────────────────────────────────────────────────────────────────────────
// ⭐⭐ 이슈 #87 — 독립 About 정보 창(plan/issue-87-about-window.md D1·D3·D5·D6).
// ─────────────────────────────────────────────────────────────────────────────

/// 이슈 #87(D5) — 트레이 메뉴 `About` 이 설정 창을 여는 라우팅에서 분리됐다:
/// `SETTINGS` 는 설정 창, `ABOUT` 은 `show_about_window` 일을 각각 담당하고,
/// `ABOUT_WINDOW_LABEL`·`show_about_window`·`about.html` 이 존재한다. 이전의
/// 결합 라우팅(`SETTINGS | ABOUT`)은 사라졌다.
#[test]
fn main_rs의_메뉴_라우팅이_settings와_about을_분리한다() {
    let main_rs = read_main_rs();
    assert!(
        main_rs.contains("menu_ids::SETTINGS => show_settings_window(app)"),
        "main.rs 에 SETTINGS → show_settings_window 라우팅이 없다(#87 D5)"
    );
    assert!(
        main_rs.contains("menu_ids::ABOUT => show_about_window(app, state)"),
        "main.rs 에 ABOUT → show_about_window 라우팅이 없다(#87 D5)"
    );
    assert!(
        !main_rs.contains("menu_ids::SETTINGS | menu_ids::ABOUT"),
        "main.rs 에 SETTINGS | ABOUT 결합 라우팅이 남아 있다 — About 이 여전히 설정 창을 연다(#87 D5)"
    );
    assert!(
        main_rs.contains("const ABOUT_WINDOW_LABEL: &str = \"about\";"),
        "main.rs 에 ABOUT_WINDOW_LABEL 상수가 없다(#87 D5)"
    );
    assert!(
        main_rs.contains("fn show_about_window("),
        "main.rs 에 show_about_window 함수가 없다(#87 D5)"
    );
    assert!(
        main_rs.contains("about.html"),
        "main.rs 의 show_about_window 가 about.html 을 로드하지 않는다(#87 D1)"
    );
}

/// 이슈 #87(D1·#88 정합) — `show_about_window` 는 About 창을 닫으면 파괴하지
/// 않고 숨긴다(설정 창의 close→hide 상주 정책과 동일). 가드: 이 배선이 없으면
/// "About 을 닫았다가 다시 열 수 없다"(#88 결함의 재현)로 회귀한다.
#[test]
fn about_창이_닫기_시_숨김_상주로_배선된다() {
    let main_rs = read_main_rs();
    let about_start = main_rs
        .find("fn show_about_window(")
        .expect("main.rs 에 show_about_window 가 없다");
    let about_body = &main_rs[about_start..];
    assert!(
        about_body.contains("CloseRequested"),
        "show_about_window 의 CloseRequested 배선이 없다(#88 정책)"
    );
    assert!(
        about_body.contains("prevent_close"),
        "show_about_window 에 prevent_close() 가 없다(#88 정책)"
    );
    assert!(
        about_body.contains(".hide()"),
        "show_about_window 에 hide() 가 없다(#88 정책)"
    );
}

/// ⭐ 이슈 #96 — About 창은 "본문 전체가 스크롤바 없이 보이는 고정 크기" 정책이다:
/// 5개 언어 × 번들 내·외 2상태 전부를 헤드리스 렌더로 실측해 가장 높은 조합에
/// 여백을 더한 540×480pt 로 고정하고 `resizable(false)` 를 유지한다. 이 배선이
/// 바뀌면(크기 축소·리사이즈 허용) "스크롤바 없는 정보 창" 약속이 깨진다.
#[test]
fn about_창은_스크롤바_없는_고정_크기로_열린다() {
    let main_rs = read_main_rs();
    let about_start = main_rs
        .find("fn show_about_window(")
        .expect("main.rs 에 show_about_window 가 없다");
    let about_body = &main_rs[about_start..];
    assert!(
        about_body.contains(".inner_size(540.0, 480.0)"),
        "show_about_window 가 540×480pt 로 열지 않는다 — 5언어×번들 내·외 실측 \
         기준 고정 크기다(이슈 #96)"
    );
    assert!(
        about_body.contains(".resizable(false)"),
        "show_about_window 에 resizable(false) 가 없다 — 정보 창은 고정 크기다(이슈 #96)"
    );
}

/// 이슈 #87(D3·§9 #2) — General 탭 상단의 `#about` 블록은 제거됐고(로그 위치는
/// About 창으로, 번들 ID·설정 파일 경로 행은 Advanced 섹션으로), 버전 버튼
/// `#version-btn` 은 **유지**하되 `open_about_window` 를 invoke 하는 About 창
/// 오픈 버튼으로 바뀌었다.
#[test]
fn settings_html의_about_블록이_제거되고_버전_버튼은_about_창을_연다() {
    let html = read_settings_html();
    assert!(
        !html.contains(r#"id="about""#) && !html.contains(r#"id="about" "#),
        "settings.html 에 General 탭 #about 블록이 남아 있다 — About 정보는 About 창으로 이동했다(#87 D3)"
    );
    assert!(
        !html.contains("about-log-path"),
        "settings.html 에 about-log-path(로그 위치 행)가 남아 있다 — About 창으로 이동했다(#87 D3)"
    );
    assert!(
        html.contains(r#"id="about-bundle-id""#) && html.contains(r#"id="about-settings-path""#),
        "settings.html 에 번들 ID·설정 파일 경로 행이 없다 — Advanced 로 이동해야 한다(#87 D3)"
    );
    assert!(
        html.contains("general-path-info"),
        "settings.html Advanced 섹션에 #general-path-info(번들 ID·설정 경로) 블록이 없다(#87 D3)"
    );
    assert!(
        html.contains(r#"id="version-btn""#),
        "settings.html 의 #version-btn 이 제거됐다 — §9 #2 는 유지다(#87)"
    );
    assert!(
        html.contains(r#"invoke("open_about_window""#),
        "settings.html 의 #version-btn 리스너가 open_about_window 를 invoke 하지 않는다(#87 §9 #2)"
    );
}

/// 이슈 #87(D1·§3.2) — `about.html` 이 존재하고 핵심 커맨드 3종을 invoke 한다.
/// `open_about_window` 는 settings.html(버전 버튼)이 호출하는 것이므로 여기 목록에
/// 없다 — 시기가 다르게 배선된 진입점이다.
#[test]
fn about_html이_부트스트랩_등_커맨드를_invoke_한다() {
    let html = read_about_html();
    for command in ["about_bootstrap", "check_for_updates", "open_external_url"] {
        assert!(
            html.contains(&format!("invoke(\"{command}\"")),
            "ui/about.html 이 invoke(\"{command}\", …) 를 호출하지 않는다"
        );
    }
}

/// ⭐ 이슈 #87 — About 창도 번역 표면이다(카탈로그를 태워 온다). `<script>` 안에
/// 사용자 대면 영어 문장이 하드코딩되어 있지 않아야 한다 — settings.html 의 같은
/// 규약을 그대로 따른다. 허용 목록은 부트스트랩 경로가 죽었을 때의 진단 문구
/// 뿐이다(카탈로그에 기댈 수 없는 최후 수단).
#[test]
fn about_html_에_하드코딩된_영어_문장이_없다() {
    const ALLOWED_DIAGNOSTIC_LITERALS: &[&str] = &[
        "Failed to load: window.__TAURI__ is unavailable (check withGlobalTauri in tauri.conf.json).",
        "Failed to load the About window: ",
        "Failed to open link: ",
        "Failed to check for updates: ",
    ];

    let html = read_about_html();
    let script = extract_script_block(&html);
    let script_no_comments = strip_line_comments(script);

    let offenders: Vec<String> = extract_double_quoted_literals(&script_no_comments)
        .into_iter()
        .filter(|s| s.contains(' '))
        .filter(|s| !ALLOWED_DIAGNOSTIC_LITERALS.contains(&s.as_str()))
        .collect();

    assert!(
        offenders.is_empty(),
        "about.html 의 <script> 안에서 허용 목록 밖의 영어 문장 리터럴을 찾았다: \
         {offenders:?} — 카탈로그 키를 통해 t()/tf() 로 가져오거나, 정말 진단 전용 \
         문구라면 ALLOWED_DIAGNOSTIC_LITERALS 에 추가하고 그 근거를 남겨라."
    );
}

/// 이슈 #87(D5) — About 창과 관련된 신규 커맨드 4종이 `generate_handler!` 등록
/// 목록에 전부 있다(선언만 하고 등록 누락하면 프런트 invoke 가 즉시 실패한다 —
/// `키보드_복사_커맨드가_invoke_배선되어_있다` 패턴).
#[test]
fn 이슈87_커맨드가_generate_handler_에_등록되어_있다() {
    let main_rs = read_main_rs();
    for command in [
        "about_bootstrap",
        "open_about_window",
        "check_for_updates",
        "open_external_url",
    ] {
        assert!(
            main_rs.contains(&format!("fn {command}(")),
            "main.rs 에 {command} 커맨드 선언이 없다"
        );
        assert!(
            main_rs.contains(&format!("{command},")),
            "generate_handler! 등록 목록에 {command} 가 없다"
        );
    }
}

/// 이슈 #87(D6) — `about.*` 키가 5개 카탈로그 전부에 있고, About 창 행 값에 쓰는
/// `settings.general.version` 도 유지된다. ⭐ URL 라벨 3종의 코드 값(`GitHub`·
/// `GitHub Issues`)도 카탈로그 값과 일치해야 한다(§9 #1 — 번역 없이 고유 브랜드).
/// ⭐ 이슈 #96 — `about.app_name`·`about.version` 두 라벨 키는 제거됐다: 히어로가
/// 앱 이름(브랜드 상수)·버전(`settings.general.version`)을 직접 표시하므로 라벨
/// 행이 사라졌기 때문이다(미사용 키 방치 금지 — #87 D6 의 log_path 재배치 선례).
#[test]
fn 이슈87의_about_점_키가_다섯_카탈로그_모두에_있다() {
    let required = [
        "about.title",
        "about.author",
        "about.website",
        "about.contact",
        "about.issues",
        "about.app_path",
        "about.log_path",
        "about.outside_bundle",
        "settings.general.version",
    ];
    for locale in LOCALES {
        let keys = flatten_catalog(&read_catalog(locale));
        for key in required {
            assert!(
                keys.contains(key),
                "{locale}.json 에 {key} 키가 없다 (#87 D6)"
            );
        }
        // ⭐ §9 #1 — 웹사이트·연락 라벨은 번역하지 않는다(고유 브랜드). 값은
        // 카탈로그 프런트가 그대로 그린다(이 테스트의 실측 선언).
        let catalog = read_catalog(locale);
        assert_eq!(
            catalog["about.website"].as_str(),
            Some("GitHub"),
            "{locale}.json 의 about.website 가 GitHub 브랜드가 아니다(#87 §9 #1)"
        );
        assert_eq!(
            catalog["about.contact"].as_str(),
            Some("GitHub Issues"),
            "{locale}.json 의 about.contact 가 GitHub Issues 브랜드가 아니다(#87 §9 #1)"
        );
    }
}

/// 이슈 #87(D6) — `settings.general.about.log_path` 키는 About 창 키로 재배치
/// 됐다(`about.log_path`). 흔적만 남기고 지우지 않은 것과 달리, 카탈로그에서는
/// **실제로 제거**되어야 한다 — 못 쓰이는 키가 남으면 로케일 정합 테스트가
/// 어긋나진 않아도 drift 가 된다.
#[test]
fn 이슈87에서_settings_about_log_path_키가_제거되었다() {
    for locale in LOCALES {
        let keys = flatten_catalog(&read_catalog(locale));
        assert!(
            !keys.contains("settings.general.about.log_path"),
            "{locale}.json 에 settings.general.about.log_path 가 남아 있다 — about.log_path 로 재배치했다(#87 D6)"
        );
    }
}

/// 이슈 #87(D3) — `AppMeta` 에 설치 위치 필드(`app_path`)가 있고 그 원천 함수
/// (`app_bundle_path`)가 존재한다. 카메라 케이스 직렬화(`camelCase`)의 `appPath`
/// 도 함께 확인한다 — 프런트 `about.html` 이 `meta.appPath` 를 읽는다.
#[test]
fn main_rs의_app_meta에_app_path_필드와_원천_함수가_있다() {
    let main_rs = read_main_rs();
    assert!(
        main_rs.contains("fn app_bundle_path("),
        "main.rs 에 app_bundle_path() 함수가 없다(#87 D3)"
    );
    assert!(
        main_rs.contains("app_path: app_bundle_path()"),
        "build_app_meta 가 app_path 필드를 app_bundle_path() 로 채우지 않는다(#87 D3)"
    );
    assert!(
        read_about_html().contains("meta.appPath"),
        "about.html 이 meta.appPath 를 읽지 않는다 — AppMeta 직렬화 이름이 appPath 임이 어긋났다(#87 D3)"
    );
}

/// ⭐ 이슈 #87 상급 리뷰 #1 — 상주 About 창의 언어 변경 스테일니스 수정이 배선돼
/// 있다(#88 상주로 부트스트랩이 창 생성 시 1회뿐이라, 열어 본 채로 언어를 바꾸면
/// 앱 재시작까지 이전 언어로 남는다):
///
/// ① Rust 가 카탈로그 교체 시점 둘(언어 선택·설정 import 교체)에
///    `notify_about_catalog_changed` 를 부른다(네이티브 타이틀바 `set_title` +
///    `catalog-changed` emit).
/// ② `about.html` 이 `catalog-changed` 를 listen 하여 `about_bootstrap` 을 다시
///    불러 정보 행을 갱신한다.
/// ③ about 창이 listen 하려면 capability 로 `core:event:default`(listen 포함)가
///    열려 있어야 한다 — 없으면 ACL 이 막아 이 배선이 통째로 조용히 죽는다
///    (overlay.json 의 같은 근거).
#[test]
fn about_창_언어_변경_즉시_반영_배선이_있다() {
    let main_rs = read_main_rs();
    for needle in [
        "fn notify_about_catalog_changed(",
        "const CATALOG_CHANGED_EVENT: &str = \"catalog-changed\";",
        "window.set_title(catalog.get(\"about.title\"))",
        "window.emit(CATALOG_CHANGED_EVENT, ())",
        // 언어 선택 경로(settings_set_general_language)와 import 교체 경로
        // (reload_settings_after_replace) 두 지점에 모두 배선돼 있어야 한다.
        "notify_about_catalog_changed(app, state);\n\n    current_settings_state(state)",
        "notify_about_catalog_changed(app, state);\n    Ok(())",
    ] {
        assert!(
            main_rs.contains(needle),
            "main.rs 에 {needle:?} 이 없다 — About 창 언어 갱신 배선이 빠졌다(이슈 #87 상급 #1)"
        );
    }

    let about = read_about_html();
    assert!(
        about.contains(r#"listen("catalog-changed""#),
        "about.html 이 catalog-changed 를 listen 하지 않는다 — 재부트스트랩 경로가 없다(이슈 #87 상급 #1)"
    );

    let cap_path = manifest_dir().join("capabilities/about.json");
    let cap_raw = std::fs::read_to_string(&cap_path)
        .unwrap_or_else(|e| panic!("capabilities/about.json 을 읽지 못했다({cap_path:?}): {e}"));
    let cap: serde_json::Value = serde_json::from_str(&cap_raw)
        .unwrap_or_else(|e| panic!("capabilities/about.json 파싱 실패({cap_path:?}): {e}"));
    assert_eq!(
        cap["windows"],
        serde_json::json!(["about"]),
        "capabilities/about.json 의 windows 가 about 창만 가리키지 않는다(이슈 #87 상급 #1)"
    );
    let has_event_listen = cap["permissions"]
        .as_array()
        .map(|a| a.iter().any(|p| p == "core:event:default"))
        .unwrap_or(false);
    assert!(
        has_event_listen,
        "capabilities/about.json 에 core:event:default 가 없다 — about 창 listen 이 ACL 에 막힌다(이슈 #87 상급 #1)"
    );
}
