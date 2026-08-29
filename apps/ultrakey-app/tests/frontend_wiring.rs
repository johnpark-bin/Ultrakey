//! 재발 방지 테스트 — `tauri.conf.json` 과 `ui/index.html` 이 어긋나면 잡는다.
//!
//! ⭐ 이번 버그의 본질: `ui/index.html` 이 `window.__TAURI__` 를 쓰는데
//! `tauri.conf.json` 의 `app.withGlobalTauri` 가 없어서(기본값 `false`)
//! Tauri v2 가 그 전역을 주입하지 않았다. 모듈 평가 시점에 `TypeError` 가 나
//! `render()` 가 한 번도 실행되지 못했고, 정적 HTML 의 `h1`/`p` 는 비어 있고
//! 콘텐츠 `div` 는 전부 `hidden` 이라 웹뷰가 완전히 백지로 보였다. 창 자체는
//! 정상적으로 최상단·포커스 상태였다 — 컴파일러도 기존 테스트도 이 어긋남을
//! 잡지 못했다. 이 파일은 그 어긋남을 정적으로 검사한다.

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
