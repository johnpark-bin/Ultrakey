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

fn read_ko_catalog() -> serde_json::Value {
    let path = manifest_dir().join("../../resources/i18n/ko.json");
    let raw = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("resources/i18n/ko.json 을 읽지 못했다({path:?}): {e}"));
    serde_json::from_str(&raw)
        .unwrap_or_else(|e| panic!("resources/i18n/ko.json 파싱 실패({path:?}): {e}"))
}

fn read_main_rs() -> String {
    let path = manifest_dir().join("src/main.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("src/main.rs 을 읽지 못했다({path:?}): {e}"))
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

    let missing: Vec<_> = exact.iter().filter(|k| !known_keys.contains(k.as_str())).collect();
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

/// 탭 5개의 창 크기 상수(main.rs `tab_window_size`)가 명세값과 일치하는지는
/// `src/main.rs` 자체의 유닛 테스트(`tests::tab_window_size_matches_spec_values`)가
/// 이미 검증한다. 여기서는 그 상수들이 `settings.html` 문서 주석에도 정확히
/// 기록되어 있는지만 방어적으로 재확인한다(창 크기는 Rust 가 소유하고, HTML 은
/// 그 사실만 인용한다 — 값 자체를 HTML 이 중복 정의하지 않는다).
///
/// ⭐ `613×484` 는 F-16 `Korean` 탭의 **실측값**이다(korean-input.md §4.1·§9 #5):
/// 이 탭의 마크업을 실제로 렌더해 잰 `panel-korean` 내용 416px + `.panel` 상하
/// 패딩 40px + 타이틀바 28px. 처음 잡았던 잠정치 400 은 84px 모자라 내용이 잘렸다.
#[test]
fn settings_html_문서가_탭_창_크기_명세값을_인용한다() {
    let html = read_settings_html();
    for token in ["555×378", "710×517", "825×527", "613×484", "613×273"] {
        assert!(
            html.contains(token),
            "settings.html 문서 주석에 탭 창 크기 {token} 이 보이지 않는다 — \
             preferences-ui.md §3.1 실측값(Seek 555×378 · Hyperkey 710×517 · \
             Presets 825×527 · Korean 613×484(실측) · General 613×273 pt)과 \
             어긋났을 수 있다."
        );
    }
}

/// `main.rs` 의 `tab_window_size` 가 `korean` 탭을 알고, `settings_html_문서가_탭_창_크기_
/// 명세값을_인용한다` 와 같은 값(613×484)을 쓰는지 소스 텍스트로 확인한다. 실제 반환값
/// 자체는 `src/main.rs` 유닛 테스트(`tests::tab_window_size_matches_spec_values`)가 검증한다
/// — 여기서는 "그 커맨드가 korean 을 다루는가"라는 배선 자체를 재발 방지 관점에서 본다.
#[test]
fn main_rs_의_tab_window_size가_korean_탭_크기를_안다() {
    let main_rs = read_main_rs();
    assert!(
        main_rs.contains("\"korean\" => Some((613, 484))"),
        "main.rs 의 tab_window_size() 가 \"korean\" => Some((613, 484)) 을 반환하지 않는다"
    );
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
        ("korean-han-eng", "korean-han-eng-hint", "korean-han-eng-badge"),
        ("korean-hanja", "korean-hanja-hint", "korean-hanja-badge"),
    ] {
        let input_needle = format!("id=\"{checkbox_id}\"");
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

    let missing_en: Vec<_> = korean_keys.iter().filter(|k| !en_keys.contains(k.as_str())).collect();
    let missing_ko: Vec<_> = korean_keys.iter().filter(|k| !ko_keys.contains(k.as_str())).collect();

    assert!(missing_en.is_empty(), "en.json 에 없는 settings.korean.* 키: {missing_en:?}");
    assert!(missing_ko.is_empty(), "ko.json 에 없는 settings.korean.* 키: {missing_ko:?}");
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

    let missing: Vec<_> = menu_keys.iter().filter(|k| !known_keys.contains(k.as_str())).collect();
    assert!(
        missing.is_empty(),
        "src/main.rs 가 참조하는 다음 트레이 메뉴 카탈로그 키가 resources/i18n/en.json 에 없다: {missing:?}"
    );
}

/// (b) 트레이 메뉴가 참조하는 i18n 키가 en·ko 양쪽에 모두 있다. `ultrakey-i18n` 의
/// `en_ko_key_sets_are_identical` 이 카탈로그 전체에 대해 이미 이걸 검증하지만,
/// 여기서는 "메뉴가 실제로 쓰는 키 집합"을 자체적으로 다시 좁혀서 확인한다 —
/// 이 테스트만 보고도 메뉴 관련 키 누락을 바로 알 수 있어야 한다.
#[test]
fn main_rs_의_menu_점_리터럴은_en_ko_양쪽_카탈로그에_모두_있다() {
    let en_keys = flatten_catalog(&read_en_catalog());
    let ko_keys = flatten_catalog(&read_ko_catalog());

    let menu_keys = menu_dot_literals_in_main_rs();
    assert!(!menu_keys.is_empty());

    let missing_en: Vec<_> = menu_keys.iter().filter(|k| !en_keys.contains(k.as_str())).collect();
    let missing_ko: Vec<_> = menu_keys.iter().filter(|k| !ko_keys.contains(k.as_str())).collect();

    assert!(missing_en.is_empty(), "en.json 에 없는 메뉴 키: {missing_en:?}");
    assert!(missing_ko.is_empty(), "ko.json 에 없는 메뉴 키: {missing_ko:?}");
}

/// 메뉴 항목 id(`menu_ids` 모듈)가 §3.3 이 정한 정상 메뉴 구성(`Ignore <앱>` ·
/// `Settings…` · `About` · `Advanced ▸ (Synthesize Caps Lock Remap / Relaunch)` ·
/// `Quit Ultrakey`)과 unauthorizedMenu 의 `Authorize` 항목을 전부 갖추고 있는지
/// 이름만으로 재확인한다. `Purchase`(F-12)·`Check for Updates…`(F-13)가 실수로
/// 다시 들어오면(범위 밖) 이 테스트가 아니라 코드 리뷰에서 걸러야 하지만, 적어도
/// 필수 항목이 빠지는 회귀는 여기서 잡는다.
#[test]
fn main_rs_가_정상_메뉴의_필수_항목_id를_전부_선언한다() {
    let main_rs = read_main_rs();
    for id in [
        "menu.ignore_app",
        "menu.settings",
        "menu.about",
        "menu.advanced",
        "menu.advanced.synthesize_caps_remap",
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
    assert!(!icons.is_empty(), "tauri.conf.json 의 bundle.icon 이 비어 있다");

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

/// ⭐ 이슈 #19 증상 B 의 회귀 방지 — `Synthesize Caps Lock Remap` 메뉴 항목이
/// **저장만 하고 끝나지 않는지**를 본다.
///
/// 이전 구현은 `SettingsStore::set` 만 부르고 `AppState::presets`(탭 스레드가 보는
/// 정본)도, `Engine::reconfigure`(경로 B 재적용)도 건드리지 않았다. 그래서 이 항목을
/// 눌러도 `caps lock → F18` 커널 매핑이 그대로 남았다 — 실측 로그가 토글 직후의 경로 B
/// 재적용을 `count=1` 로 기록한다. `architecture.md` §6.1 이 이 스위치에 부여한 역할은
/// "켜면 경로 B 를 설치하지 않고 경로 A 만 쓴다" 이므로, 엔진 반영이 없으면 스위치가
/// 제 역할을 못 한다.
///
/// ⚠️ 정적 텍스트 검사다 — 이 파일의 다른 테스트들과 같은 한계를 갖는다(호출이
/// 존재한다는 것만 확인하고 실제 실행 순서까지는 보지 않는다). 그래도 "저장만 하는
/// 핸들러로 되돌아가는" 회귀는 정확히 잡는다.
#[test]
fn synthesize_caps_lock_remap_토글이_엔진에도_반영된다() {
    let main_rs = read_main_rs();
    let start = main_rs
        .find("fn on_menu_toggle_synthesize_caps_lock_remap")
        .expect("on_menu_toggle_synthesize_caps_lock_remap 이 없다");
    // 다음 최상위 `fn ` 선언 전까지를 이 함수의 본문으로 본다.
    let rest = &main_rs[start..];
    let end = rest[1..].find("\nfn ").map(|i| i + 1).unwrap_or(rest.len());
    let body = &rest[..end];

    assert!(
        body.contains("reconfigure_engine"),
        "메뉴 토글이 reconfigure_engine 을 부르지 않는다 — 경로 B 매핑이 설정과 어긋난 채 남는다"
    );
    assert!(
        body.contains("synthesize_caps_lock_remap = next"),
        "메뉴 토글이 AppState::presets 정본을 갱신하지 않는다 — 탭 스레드가 옛 값을 계속 본다"
    );
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
        html.contains(r#"["seek", "hyperkey", "presets", "korean", "keyboards", "general"]"#),
        "settings.html 의 TABS 배열이 Korean 다음·General 앞 순서로 \"keyboards\" 를 \
         갖고 있지 않다(명세 §3.1)"
    );
}

/// 고정 컨트롤 4개(명세 §3.1.5): 디바이스 선택 팝업 1 + 그룹 제목 2("키 변환 세트",
/// "Function Keys") + macOS Function Keys 상태 표시줄(뱃지+버튼) 1.
#[test]
fn settings_html_에_keyboards_탭_고정_컨트롤_4개가_있다() {
    let html = read_settings_html();

    for id in [
        "keyboards-device-picker",       // 1. 디바이스 선택 팝업
        "keyboards-keyremap-heading",    // 2. 그룹 제목 — 키 변환 세트
        "keyboards-functionkeys-heading", // 2. 그룹 제목 — Function Keys
        "keyboards-fnstate-badge",       // 3. macOS 상태 표시줄 — 뱃지
        "keyboards-open-system-settings-btn", // 3. macOS 상태 표시줄 — 버튼
    ] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "settings.html 에 Keyboards 탭 고정 컨트롤 id=\"{id}\" 가 없다"
        );
    }
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

/// Keyboards 탭이 기대하는 두 백엔드 커맨드를 invoke 한다 — `settings_unset`
/// ("공통 따름" = 키 삭제, §3.3) 과 `open_keyboard_settings`(macOS 설정 열기, §3.5.1).
#[test]
fn settings_html_이_settings_unset과_open_keyboard_settings를_invoke한다() {
    let html = read_settings_html();
    for command in ["settings_unset", "open_keyboard_settings"] {
        assert!(
            html.contains(&format!("invoke(\"{command}\"")),
            "ui/settings.html 이 invoke(\"{command}\", …) 를 호출하지 않는다"
        );
    }
}

/// 명세 §4.1 표의 24개 신규 키(탭 라벨 1개 + `preferences.keyboards.*` 23개)가
/// en.json·ko.json 양쪽에 전부 있다. ⚠️ 탭 라벨 키만 명세 제안(`preferences.tabs.
/// keyboards.title`) 대신 기존 코드베이스 관례(`settings.tab.<탭>`)를 따라
/// `settings.tab.keyboards` 로 잡았다 — settings.html 의 근거 주석 참고.
#[test]
fn keyboards_탭_신규_i18n_키_24개가_en_ko_양쪽에_있다() {
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
        "preferences.keyboards.keyRemap.duplicateFromWarning",
        "preferences.keyboards.functionKeys.followCommon",
        "preferences.keyboards.functionKeys.useStandardFKey",
        "preferences.keyboards.functionKeys.macosStatus.label",
        "preferences.keyboards.functionKeys.macosStatus.openButton",
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
    assert_eq!(KEYS.len(), 24, "이 목록 자체가 24개가 아니다 — 명세 §4.1 표와 개수를 다시 맞춰라");

    let missing_en: Vec<_> = KEYS.iter().filter(|k| !en.contains(**k)).collect();
    let missing_ko: Vec<_> = KEYS.iter().filter(|k| !ko.contains(**k)).collect();
    assert!(missing_en.is_empty(), "en.json 에 없는 Keyboards 탭 키: {missing_en:?}");
    assert!(missing_ko.is_empty(), "ko.json 에 없는 Keyboards 탭 키: {missing_ko:?}");
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

    let missing_en: Vec<_> = keyboards_keys.iter().filter(|k| !en_keys.contains(k.as_str())).collect();
    let missing_ko: Vec<_> = keyboards_keys.iter().filter(|k| !ko_keys.contains(k.as_str())).collect();

    assert!(missing_en.is_empty(), "en.json 에 없는 preferences.keyboards.* 키: {missing_en:?}");
    assert!(missing_ko.is_empty(), "ko.json 에 없는 preferences.keyboards.* 키: {missing_ko:?}");
}

/// 탭별 창 크기 문서 주석(§3.1.4)에 `Keyboards` 값이 있고, 그 값이 **잠정값이며
/// 실측이 아니라는 사실**을 정직하게 밝히고 있다 — 명세 §3.1.4 가 이 탭은 신설이라
/// 실측 근거가 없다고 명시했으므로, 다른 탭들(전부 실측)과 같은 방식으로 확정값인
/// 것처럼 적으면 안 된다.
#[test]
fn settings_html_문서가_keyboards_탭_창_크기를_잠정값으로_밝힌다() {
    let html = read_settings_html();
    assert!(
        html.contains("Keyboards 710×517"),
        "settings.html 문서 주석에 Keyboards 탭의 잠정 창 크기(710×517, Hyperkey 값 \
         재사용)가 보이지 않는다"
    );
    assert!(
        html.contains("잠정값, 실측 아님"),
        "settings.html 문서 주석이 Keyboards 탭 창 크기를 잠정값이라고 밝히지 않는다 — \
         per-device-settings.md §3.1.4 는 이 탭이 신설이라 실측 근거가 없다고 명시했다"
    );
}
