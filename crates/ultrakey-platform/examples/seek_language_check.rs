//! F-02 Phase 1 (계속) — `recognitionLanguages` 실측 (§3.2.2, §9 Q-h/Q-j).
//!
//! 1차 품질 비교에서 **한글이 심하게 깨져 인식**되는 것이 드러났다. 명세는
//! `recognitionLanguages` 의 실제 값을 `(추정)` 으로 남겼는데, 이 클론의 1차
//! 사용자는 한국어 사용자이므로 기본값이 무엇이어야 하는지가 실제 문제다.
//!
//! 실행: cargo run -p ultrakey-platform --release --example seek_language_check

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    use std::time::Instant;
    use ultrakey_platform::image_preprocess::{preprocess, PreprocessPreset};
    use ultrakey_platform::screen_capture;
    use ultrakey_platform::vision_ocr::{recognize_text, RecognitionParams};

    let configs: Vec<(&str, Vec<String>)> = vec![
        ("기본값(미설정)", vec![]),
        ("en-US", vec!["en-US".into()]),
        ("ko-KR, en-US", vec!["ko-KR".into(), "en-US".into()]),
        ("en-US, ko-KR", vec!["en-US".into(), "ko-KR".into()]),
    ];

    for d in screen_capture::active_displays() {
        let Some(cap) = screen_capture::capture_display(d) else {
            continue;
        };
        println!("# 디스플레이 id={}", d.display_id);
        for (label, langs) in &configs {
            let params = RecognitionParams {
                languages: langs.clone(),
                ..RecognitionParams::default()
            };
            let img = preprocess(&cap.image, PreprocessPreset::None);
            let t = Instant::now();
            let obs = recognize_text(&img, &params).unwrap_or_default();
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            let hangul = obs
                .iter()
                .filter(|o| o.text.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c)))
                .count();
            let conf = if obs.is_empty() {
                0.0
            } else {
                obs.iter().map(|o| o.confidence).sum::<f32>() / obs.len() as f32
            };
            println!(
                "  {label:<16} {ms:>7.1}ms  관측 {:>4}개  한글포함 {hangul:>3}개  평균신뢰도 {conf:.3}",
                obs.len()
            );
            let sample: Vec<&str> = obs
                .iter()
                .filter(|o| o.text.chars().any(|c| ('\u{AC00}'..='\u{D7A3}').contains(&c)))
                .take(6)
                .map(|o| o.text.as_str())
                .collect();
            if !sample.is_empty() {
                println!("    한글 표본: {sample:?}");
            }
        }
        println!();
    }
}
