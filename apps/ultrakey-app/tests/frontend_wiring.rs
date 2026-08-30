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
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("ui/index.html 을 읽지 못했다({path:?}): {e}"))
}

fn read_settings_html() -> String {
    let path = manifest_dir().join("ui/settings.html");
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("ui/settings.html 을 읽지 못했다({path:?}): {e}"))
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
    let style_start = html.find("<style>").expect("index.html 에 <style> 블록이 없다");
    let style_end = html.find("</style>").expect("index.html 에 </style> 종료 태그가 없다");
    let style = &html[style_start..style_end];

    let body_start = style.find("body {").expect("index.html 의 <style> 안에 body 규칙이 없다");
    let body_rule_end = style[body_start..]
        .find('}')
        .map(|i| body_start + i)
        .expect("index.html 의 body 규칙이 닫히지 않았다");
    let body_rule = &style[body_start..body_rule_end];

    assert!(
        !body_rule.contains("background: transparent") && !body_rule.contains("background:transparent"),
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
    let start = html.find("<script").expect("settings.html 에 <script> 태그가 없다");
    let tag_end = html[start..]
        .find('>')
        .map(|i| start + i + 1)
        .expect("settings.html 의 <script> 시작 태그가 닫히지 않았다");
    let end = html.find("</script>").expect("settings.html 에 </script> 종료 태그가 없다");
    assert!(tag_end < end, "settings.html 의 <script> 블록 범위 계산이 어긋났다");
    &html[tag_end..end]
}

/// 한 줄짜리 `// ...` 주석을 잘라낸다(이 파일은 문자열 리터럴 안에 `//` 를 쓰지
/// 않으므로 줄 단위로 첫 `//` 부터 잘라내는 순수한 방식으로 충분하다).
fn strip_line_comments(script: &str) -> String {
    script
        .lines()
        .map(|line| match line.find("//") {
            Some(idx) => &line[..idx],
            None => line,
        })
        .collect::<Vec<_>>()
        .join("\n")
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
    let html = read_settings_html();
    for command in ["settings_bootstrap", "settings_set", "settings_set_tab", "quit_app"] {
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

    let missing: Vec<_> = settings_keys
        .iter()
        .filter(|k| !known_keys.contains(k.as_str()))
        .collect();
    assert!(
        missing.is_empty(),
        "settings.html 이 참조하는 다음 카탈로그 키가 resources/i18n/en.json 에 없다: {missing:?}"
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

/// 탭 4개의 창 크기 상수(main.rs `tab_window_size`)가 명세값과 일치하는지는
/// `src/main.rs` 자체의 유닛 테스트(`tests::tab_window_size_matches_spec_values`)가
/// 이미 검증한다. 여기서는 그 상수들이 `settings.html` 문서 주석에도 정확히
/// 기록되어 있는지만 방어적으로 재확인한다(창 크기는 Rust 가 소유하고, HTML 은
/// 그 사실만 인용한다 — 값 자체를 HTML 이 중복 정의하지 않는다).
#[test]
fn settings_html_문서가_탭_창_크기_명세값을_인용한다() {
    let html = read_settings_html();
    for token in ["555×378", "710×517", "825×527", "613×273"] {
        assert!(
            html.contains(token),
            "settings.html 문서 주석에 탭 창 크기 {token} 이 보이지 않는다 — \
             preferences-ui.md §3.1 실측값(Seek 555×378 · Hyperkey 710×517 · \
             Presets 825×527 · General 613×273 pt)과 어긋났을 수 있다."
        );
    }
}

/// ⭐ F-15 §8 회귀 방지 — **환경설정 창을 열기만 해서는 `settings.json` 이 생기면 안 된다.**
///
/// 창을 처음 그릴 때도 탭 크기를 맞추려고 `settings_set_tab` 을 부르는데, 그때 `ui.lastTab`
/// 까지 저장해 버리면 "설정을 한 번도 건드리지 않으면 저장 파일이 아예 생기지 않는다"가
/// 첫 실행에서 바로 깨진다. 그래서 최초 렌더는 `persist: false` 로 부른다 — 이 테스트는
/// 그 호출 규약이 유지되는지를 지킨다(이 규약이 깨져도 다른 테스트는 전부 통과한다).
#[test]
fn 최초_렌더는_탭을_저장하지_않고_리사이즈만_한다() {
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
