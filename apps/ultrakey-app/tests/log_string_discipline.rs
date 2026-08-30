//! ⭐⭐ **로그 문자열 경로와 UI 문자열 경로가 섞이지 않는다**를 강제하는 테스트.
//! 근거: `docs/spec/localization-and-input-sources.md` §3.1.6(이슈 #39).
//!
//! 사용자 요구 원문:
//!
//! > 로깅을 할 때에는 모두 영어로 표기를 했으면 좋겠음. 이것은 언어의 선택과
//! > 무관하게 추후 문제 상황을 분석함에 있어서 편리함을 제공하기 때문임.
//!
//! **이 요구의 핵심은 문구 치환이 아니라 경로 분리다.** 지금까지 이 저장소에는
//! "이 문자열은 사용자에게 가는가, 로그로 가는가"라는 구분이 아예 없었고, 그래서
//! 두 방향의 사고가 실제로 일어났다 — 로그가 한국어로 쓰였고(문서 규약이 코드
//! 안까지 흘러들어왔다), 반대로 **온보딩 모달의 UI 문구가 그대로 로그에 찍히고
//! 있었다**(사용자가 언어를 바꾸면 로그 내용이 따라 바뀐다).
//!
//! ⭐ 문구만 고치면 6개월 뒤 같은 일이 다시 생긴다. 그래서 규약을 문서에만 두지
//! 않고 **깨면 빌드가 깨지는 장치**로 만든다. 이 저장소는 이미 같은 형태의 장치를
//! 쓰고 있다 — `frontend_wiring.rs` 가 `settings.html`·`tauri.conf.json` 의 내용을
//! 직접 읽어 검사한다. 이 파일은 그 자리에 하나를 더 놓는다.
//!
//! ⛔ **이 테스트가 검사하지 않는 것**(전부 의도적이다):
//! - 주석(`//`, `///`, `//!`)의 한국어 — 저장소의 문서 규약은 한국어다(AGENTS.md §3).
//!   스캐너가 주석을 건너뛰는 이유가 이것이다.
//! - `expect()`·`panic!`·`assert!` 메시지 — 개발 중에만 보이고 사용자의 로그 파일에
//!   나타나지 않는다.
//! - 문자열 카탈로그(`resources/i18n/`) 자체 — 그쪽이 UI 경로의 정본이다.

use std::path::{Path, PathBuf};

/// 워크스페이스 루트(`apps/ultrakey-app` 에서 두 단계 위).
fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("워크스페이스 루트를 찾지 못했다")
}

/// 검사 대상 트리 — 빌드 산출물(`target/`)은 제외한다.
fn scanned_roots() -> Vec<PathBuf> {
    let root = workspace_root();
    vec![root.join("crates"), root.join("apps")]
}

fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                // 빌드 산출물과 숨김 디렉터리는 건너뛴다.
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name == "target" || name.starts_with('.') {
                    continue;
                }
                walk(&path, out);
            } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
                out.push(path);
            }
        }
    }

    let mut out = Vec::new();
    for root in scanned_roots() {
        walk(&root, &mut out);
    }
    out.sort();
    assert!(
        !out.is_empty(),
        "스캔 대상 .rs 파일을 하나도 찾지 못했다 — 경로 계산이 틀렸다"
    );
    out
}

/// 소스에서 찾아낸 `tracing::<level>!( ... )` 호출 하나.
#[derive(Debug)]
struct LogCall {
    file: PathBuf,
    line: usize,
    /// 괄호 안의 원문(주석은 제거된 상태).
    body: String,
}

const LEVELS: [&str; 5] = ["trace", "debug", "info", "warn", "error"];

/// `tracing::<level>!(` 호출을 찾아 **괄호 균형이 맞는 지점까지** 잘라 낸다.
///
/// ⭐ 정규식 한 줄로는 안 된다 — 이 저장소의 로그 호출은 대부분 여러 줄이고,
/// 문자열이 `\` 로 이어지는 경우도 있다. 그래서 문자열 리터럴·주석을 인식하는
/// 작은 스캐너를 쓴다. 주석을 **본문에서 제거**하는 것이 핵심이다: 주석의
/// 한국어는 규약상 허용되므로, 제거하지 않으면 매크로 안 주석 때문에 오탐이 난다.
fn find_log_calls(file: &Path, src: &str) -> Vec<LogCall> {
    let bytes = src.as_bytes();
    let mut calls = Vec::new();
    let mut i = 0usize;

    while i < bytes.len() {
        // `tracing::<level>!(` 또는 `<level>!(` 로 시작하는가.
        if !src.is_char_boundary(i) {
            i += 1;
            continue;
        }
        let Some((open_paren, _level)) = match_log_macro_head(src, i) else {
            i += 1;
            continue;
        };

        let line = src[..i].bytes().filter(|b| *b == b'\n').count() + 1;
        let Some((body, end)) = capture_balanced(src, open_paren) else {
            // 균형이 안 맞으면(파싱 실패) 조용히 넘어간다 — 이 테스트가 컴파일러를
            // 대신하려는 것이 아니다.
            i = open_paren + 1;
            continue;
        };

        calls.push(LogCall {
            file: file.to_path_buf(),
            line,
            body,
        });
        i = end;
    }

    calls
}

/// `pos` 에서 로그 매크로 이름이 시작하는지 보고, 시작하면 여는 괄호 위치를 준다.
fn match_log_macro_head(src: &str, pos: usize) -> Option<(usize, &'static str)> {
    let rest = &src[pos..];

    // 식별자 중간에서 매칭되지 않게, 바로 앞 글자가 식별자 문자면 거른다
    // (`my_info!` 같은 것을 `info!` 로 오인하지 않기 위함).
    if pos > 0 {
        let prev = src[..pos].chars().next_back().unwrap_or(' ');
        if prev.is_alphanumeric() || prev == '_' {
            return None;
        }
    }

    for level in LEVELS {
        for prefix in ["tracing::", ""] {
            let head = format!("{prefix}{level}!");
            if !rest.starts_with(&head) {
                continue;
            }
            // `!` 다음의 공백을 건너뛰고 여는 괄호를 찾는다.
            let mut j = pos + head.len();
            while j < src.len() && src.as_bytes()[j].is_ascii_whitespace() {
                j += 1;
            }
            if src.as_bytes().get(j) == Some(&b'(') {
                return Some((j, level));
            }
        }
    }
    None
}

/// 여는 괄호에서 시작해 균형이 맞는 닫는 괄호까지의 본문을 돌려준다.
/// 문자열 리터럴(일반·raw)과 문자 리터럴 안의 괄호는 세지 않고, **주석은 본문에서
/// 제거한다**(주석의 한국어는 허용이기 때문이다).
fn capture_balanced(src: &str, open_paren: usize) -> Option<(String, usize)> {
    let b = src.as_bytes();
    let mut depth = 0isize;
    let mut i = open_paren;
    // ⭐ 바이트로 모은 뒤 마지막에 한 번만 문자열로 만든다. 한 바이트씩 `as char`
    // 로 밀어 넣으면 멀티바이트 문자(한글·`—` 등)가 깨져 한글 검출 자체가 실패한다.
    let mut body: Vec<u8> = Vec::new();

    while i < b.len() {
        let c = b[i];

        // 줄 주석 — 끝까지 버린다.
        if c == b'/' && b.get(i + 1) == Some(&b'/') {
            while i < b.len() && b[i] != b'\n' {
                i += 1;
            }
            continue;
        }
        // 블록 주석 — 닫힐 때까지 버린다(중첩은 다루지 않는다).
        if c == b'/' && b.get(i + 1) == Some(&b'*') {
            i += 2;
            while i + 1 < b.len() && !(b[i] == b'*' && b[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(b.len());
            continue;
        }

        // raw 문자열 — `r"`, `r#"`, `r##"` …
        if c == b'r' {
            let mut hashes = 0usize;
            let mut j = i + 1;
            while b.get(j) == Some(&b'#') {
                hashes += 1;
                j += 1;
            }
            if b.get(j) == Some(&b'"') {
                let closing = format!("\"{}", "#".repeat(hashes));
                let start = j + 1;
                if let Some(rel) = src[start..].find(&closing) {
                    body.extend_from_slice(&b[i..start + rel + closing.len()]);
                    i = start + rel + closing.len();
                    continue;
                }
                return None;
            }
        }

        // 일반 문자열.
        if c == b'"' {
            let start = i;
            i += 1;
            while i < b.len() {
                if b[i] == b'\\' {
                    i += 2;
                    continue;
                }
                if b[i] == b'"' {
                    i += 1;
                    break;
                }
                i += 1;
            }
            body.extend_from_slice(&b[start..i]);
            continue;
        }

        // 문자 리터럴 — 수명 표기(`'a`)와 섞이지 않게 보수적으로만 처리한다.
        if c == b'\'' && b.get(i + 1) == Some(&b'(') && b.get(i + 2) == Some(&b'\'') {
            body.extend_from_slice(&b[i..i + 3]);
            i += 3;
            continue;
        }

        if c == b'(' {
            depth += 1;
        } else if c == b')' {
            depth -= 1;
            if depth == 0 {
                body.push(b')');
                return Some((String::from_utf8_lossy(&body).into_owned(), i + 1));
            }
        }

        body.push(c);
        i += 1;
    }

    None
}

fn contains_hangul(s: &str) -> bool {
    s.chars().any(|c| {
        matches!(c,
            '\u{AC00}'..='\u{D7A3}'   // 한글 음절
            | '\u{1100}'..='\u{11FF}' // 자모
            | '\u{3130}'..='\u{318F}' // 호환 자모
        )
    })
}

fn relative(path: &Path) -> String {
    let root = workspace_root();
    path.strip_prefix(&root)
        .unwrap_or(path)
        .display()
        .to_string()
}

// ============================================================================
// ⭐ 장치 1 — 로그는 항상 영어다
// ============================================================================

/// `tracing::{trace,debug,info,warn,error}!` 호출 안에 한글이 있으면 실패한다.
///
/// 사용자 요구를 그대로 옮긴 것이다 — 로그는 **언어 설정과 무관하게** 언제나
/// 영어여야 문제 상황을 분석할 수 있다. 주석은 스캐너가 제거하므로 이 테스트는
/// 주석의 한국어를 문제 삼지 않는다.
#[test]
fn logs_are_english_only() {
    let mut offenders = Vec::new();

    for file in rust_sources() {
        // 이 테스트 파일 자신은 한글 예시 문구를 담고 있으므로 제외한다.
        if file.ends_with("tests/log_string_discipline.rs") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&file) else {
            continue;
        };
        if !src.contains('!') {
            continue;
        }
        for call in find_log_calls(&file, &src) {
            if contains_hangul(&call.body) {
                let snippet: String = call.body.chars().take(120).collect();
                offenders.push(format!("{}:{}  {}", relative(&call.file), call.line, snippet));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "로그 문구에 한국어가 남아 있다 — 로그는 언어 설정과 무관하게 항상 영어여야 한다\n\
         (docs/spec/localization-and-input-sources.md §3.1.6).\n\
         주석의 한국어는 문제가 아니다 — 아래는 전부 tracing 매크로 **안**이다.\n\n{}",
        offenders.join("\n")
    );
}

// ============================================================================
// ⭐ 장치 2 — 로그가 UI 문자열 카탈로그를 읽지 않는다
// ============================================================================

/// `tracing` 매크로 호출 안에서 문자열 카탈로그를 조회하면 실패한다.
///
/// ⭐ **이것이 실제로 일어났던 드리프트를 막는 장치다.** `main.rs` 가 온보딩 모달의
/// UI 문구(`catalog.get(...)` 의 결과)를 그대로 `tracing::warn!` 에 넘기고 있었다 —
/// 사용자가 언어를 바꾸면 **로그의 내용이 따라 바뀐다.** 정확히 사용자가 없애 달라고
/// 한 상황이다.
///
/// 로그에 사용자 대면 문구를 함께 남기고 싶다면 **문구가 아니라 키를 남긴다**:
/// `tracing::warn!(copy_key = "permissions.accessibility.title", "…")`.
/// 키는 로케일과 무관하고, 오히려 문구보다 검색하기 좋다.
#[test]
fn log_macros_do_not_read_the_ui_catalog() {
    // 카탈로그 조회를 뜻하는 표지들. `catalog` 라는 이름의 지역 변수·필드 양쪽을
    // 모두 잡도록 메서드 이름 쪽에 방점을 둔다.
    const CATALOG_READS: [&str; 4] = [
        "catalog.get(",
        "catalog.format(",
        "catalog.entries(",
        "catalog.get_os_variant(",
    ];

    let mut offenders = Vec::new();

    for file in rust_sources() {
        if file.ends_with("tests/log_string_discipline.rs") {
            continue;
        }
        let Ok(src) = std::fs::read_to_string(&file) else {
            continue;
        };
        if !src.contains("catalog") {
            continue;
        }
        for call in find_log_calls(&file, &src) {
            for needle in CATALOG_READS {
                if call.body.contains(needle) {
                    offenders.push(format!(
                        "{}:{}  `{}` 가 로그 매크로 안에 있다",
                        relative(&call.file),
                        call.line,
                        needle
                    ));
                }
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "UI 문자열 카탈로그가 로그 경로로 새고 있다 — 로그 내용이 사용자의 언어 설정에 \
         따라 바뀌게 된다\n(docs/spec/localization-and-input-sources.md §3.1.6).\n\
         문구 대신 **키**를 남겨라: tracing::warn!(copy_key = \"...\", \"english message\").\n\n{}",
        offenders.join("\n")
    );
}

// ============================================================================
// 스캐너 자신에 대한 테스트 — 이 장치가 실제로 잡는지 확인한다
// ============================================================================

/// ⭐ 규약을 강제하는 장치는 **그 장치가 동작한다는 증거**가 있어야 한다. 스캐너가
/// 조용히 아무것도 못 잡는 상태가 되면 위의 두 테스트는 항상 통과하면서 아무것도
/// 지키지 않는다.
#[test]
fn scanner_finds_multiline_calls_and_ignores_comments() {
    let src = r#"
fn a() {
    tracing::info!("plain english");
    tracing::warn!(
        attempt = 1,
        // 이 주석의 한국어는 규약상 허용이다 — 잡히면 안 된다
        "multi line english message"
    );
    tracing::error!(
        "korean here: 실패했다"
    );
    let s = "이건 문자열이지만 매크로 밖이다";
    let _ = s;
}
"#;
    let calls = find_log_calls(Path::new("test.rs"), src);
    assert_eq!(calls.len(), 3, "세 개의 로그 호출을 찾아야 한다: {calls:#?}");

    assert!(!contains_hangul(&calls[0].body), "첫 번째는 영어뿐이다");
    assert!(
        !contains_hangul(&calls[1].body),
        "두 번째는 주석에만 한국어가 있으므로 스캐너가 주석을 제거해 통과해야 한다: {:?}",
        calls[1].body
    );
    assert!(
        contains_hangul(&calls[2].body),
        "세 번째는 문자열 안에 한국어가 있으므로 반드시 잡혀야 한다"
    );
}

/// 카탈로그 검사도 실제로 잡는지 확인한다.
#[test]
fn scanner_detects_catalog_reads_inside_log_macros() {
    let src = r#"
fn a() {
    tracing::warn!(%title, "outside is fine");
    tracing::warn!(copy = catalog.get("some.key"), "leaks the ui string");
}
"#;
    let calls = find_log_calls(Path::new("test.rs"), src);
    assert_eq!(calls.len(), 2);
    assert!(!calls[0].body.contains("catalog.get("));
    assert!(calls[1].body.contains("catalog.get("));
}

/// 식별자 중간을 매크로 이름으로 오인하지 않는지.
#[test]
fn scanner_does_not_match_identifiers_that_merely_end_with_a_level_name() {
    let src = r#"
fn a() {
    my_info!("not a tracing macro");
    let debug_info = 1;
    let _ = debug_info;
}
"#;
    assert!(find_log_calls(Path::new("test.rs"), src).is_empty());
}
