//! F-02 Phase 1 (계속) — 전처리 프리셋의 **품질** 비교.
//!
//! 1차 스파이크는 프리셋별 후보 *개수* 만 셌다. 개수가 늘어도 그것이 쓸모없는
//! 조각이면 의미가 없으므로, 실제 인식 문자열을 비교해 §3.2.1 Q-k("어떤 필터를
//! 고를 것인가")의 기본값을 근거 위에서 고른다.
//!
//! 실행: cargo run -p ultrakey-platform --release --example seek_preset_quality

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    use std::collections::BTreeSet;
    use ultrakey_platform::image_preprocess::{preprocess, PreprocessPreset};
    use ultrakey_platform::screen_capture;
    use ultrakey_platform::vision_ocr::{recognize_text, RecognitionParams};

    let displays = screen_capture::active_displays();
    let params = RecognitionParams::default();
    let presets = [
        ("None", PreprocessPreset::None),
        ("Noir", PreprocessPreset::Noir),
        ("MaximumComponent", PreprocessPreset::MaximumComponent),
        ("MinimumComponent", PreprocessPreset::MinimumComponent),
    ];

    for d in &displays {
        let Some(cap) = screen_capture::capture_display(*d) else {
            continue;
        };
        println!("# 디스플레이 id={} ({}x{}px)", d.display_id, cap.image_width_px, cap.image_height_px);

        let mut sets: Vec<(&str, BTreeSet<String>, f32)> = Vec::new();
        for (name, preset) in presets {
            let img = preprocess(&cap.image, preset);
            let obs = recognize_text(&img, &params).unwrap_or_default();
            let mean_conf = if obs.is_empty() {
                0.0
            } else {
                obs.iter().map(|o| o.confidence).sum::<f32>() / obs.len() as f32
            };
            let set: BTreeSet<String> = obs.iter().map(|o| o.text.trim().to_string()).collect();
            println!("  {name:<18} 관측 {:>4}개 / 고유 {:>4}개 / 평균신뢰도 {mean_conf:.3}", obs.len(), set.len());
            sets.push((name, set, mean_conf));
        }

        // None 을 기준으로 각 프리셋이 무엇을 더 찾고 무엇을 잃었는지 본다.
        let base = sets[0].1.clone();
        for (name, set, _) in sets.iter().skip(1) {
            let gained: Vec<&String> = set.difference(&base).collect();
            let lost: Vec<&String> = base.difference(set).collect();
            println!("  --- {name} vs None: +{} / -{}", gained.len(), lost.len());
            print_sample("    추가로 찾은 것", &gained);
            print_sample("    놓친 것      ", &lost);
        }
        println!();
    }
}

#[cfg(target_os = "macos")]
fn print_sample(label: &str, items: &[&String]) {
    let sample: Vec<&str> = items.iter().take(12).map(|s| s.as_str()).collect();
    println!("{label}: {:?}", sample);
}
