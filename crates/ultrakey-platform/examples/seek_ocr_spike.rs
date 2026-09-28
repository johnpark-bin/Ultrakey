//! ⭐ F-02 Phase 1 — 전체 화면 OCR **지연 실측 스파이크**.
//!
//! `docs/spec/seek-text-detection.md` 가 M3 의 첫 일로 지목한 것이고,
//! `docs/spec/platform-constraints.md` P3 / 명세 §9 Q-m 이 미실측으로 남긴
//! 숫자다. Seek 은 세션을 열 때 화면 전체를 캡처·인식하므로 이 지연이 곧
//! 제품 성립 여부다.
//!
//! 측정 대상(디스플레이별 · 합계):
//!   1. `CGDisplayCreateImage` 캡처
//!   2. CoreImage 전처리 (프리셋별)
//!   3. Vision OCR 왕복 (`.accurate` / `.fast`)
//!
//! 실행:
//!   cargo run -p ultrakey-platform --release --example seek_ocr_spike
//!
//! ⚠️ 터미널에서 직접 실행하면 TCC 는 **부모 프로세스(터미널)** 의 Screen
//! Recording 권한으로 판정한다. 권한이 없으면 오류가 아니라 **데스크톱 배경만**
//! 담긴 이미지가 돌아온다 — 이 스파이크는 그 상태를 스스로 감지해 경고한다.

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("이 스파이크는 macOS 전용이다.");
}

#[cfg(target_os = "macos")]
fn main() {
    use std::time::Instant;
    use ultrakey_platform::image_preprocess::PreprocessPreset;
    use ultrakey_platform::screen_capture;
    use ultrakey_platform::vision_ocr::{self, RecognitionParams};

    const ROUNDS: usize = 5;

    let displays = screen_capture::active_displays();
    println!("# F-02 OCR 지연 스파이크");
    println!();
    println!("활성 디스플레이 {}개", displays.len());
    for d in &displays {
        println!(
            "  - id={} 전역원점=({:.0}, {:.0}) 논리크기={:.0}x{:.0}pt",
            d.display_id, d.origin_x, d.origin_y, d.width_pt, d.height_pt
        );
    }
    println!();

    // ── 0. 캡처 지연 + 배율 확인 ───────────────────────────────────────────
    println!("## 1. 캡처 (`CGDisplayCreateImage`)");
    let mut captured = Vec::new();
    for d in &displays {
        let mut samples = Vec::new();
        let mut last = None;
        for _ in 0..ROUNDS {
            let t = Instant::now();
            let c = screen_capture::capture_display(*d);
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
            last = c;
        }
        match last {
            Some(c) => {
                println!(
                    "  id={} {}x{}px scale={:.2} — 캡처 {}",
                    d.display_id,
                    c.image_width_px,
                    c.image_height_px,
                    c.scale,
                    stats(&samples)
                );
                captured.push(c);
            }
            None => println!("  id={} — 캡처 실패(None)", d.display_id),
        }
    }
    println!();

    // ── 1. 전처리 지연 (프리셋별) ─────────────────────────────────────────
    println!("## 2. CoreImage 전처리");
    let presets = [
        ("None", PreprocessPreset::None),
        ("Noir", PreprocessPreset::Noir),
        ("MaximumComponent", PreprocessPreset::MaximumComponent),
        ("Downscale(0.5)", PreprocessPreset::Downscale { scale: 0.5 }),
        (
            "NoirDownscale(0.5)",
            PreprocessPreset::NoirDownscale { scale: 0.5 },
        ),
    ];
    // ⚠️ CoreImage 는 지연 평가다 — `imageByApplyingFilter` 자체는 그래프만
    // 만들고 실제 계산은 Vision 이 픽셀을 요구할 때 일어난다. 그래서 이
    // 구간의 숫자는 "그래프 구성 비용" 이고, 진짜 비용은 §3 의 OCR 시간에
    // 흡수되어 나타난다. 그 점을 감안해서 읽어야 한다.
    for (name, preset) in presets {
        for c in &captured {
            let mut samples = Vec::new();
            for _ in 0..ROUNDS {
                let t = Instant::now();
                let img = ultrakey_platform::image_preprocess::preprocess(&c.image, preset);
                std::hint::black_box(&img);
                samples.push(t.elapsed().as_secs_f64() * 1000.0);
            }
            println!(
                "  {:<20} id={} — 그래프 구성 {}",
                name,
                c.geometry.display_id,
                stats(&samples)
            );
        }
    }
    println!();

    // ── 2. OCR 왕복 ───────────────────────────────────────────────────────
    println!("## 3. Vision OCR 왕복 (전처리 + 인식, 실제 픽셀 계산 포함)");
    for accurate in [true, false] {
        let level = if accurate { ".accurate" } else { ".fast" };
        for (name, preset) in presets {
            let params = RecognitionParams {
                accurate,
                ..RecognitionParams::default()
            };
            let mut total_per_round = vec![0.0f64; ROUNDS];
            for c in &captured {
                let mut samples = Vec::new();
                let mut count = 0usize;
                for total in total_per_round.iter_mut() {
                    let t = Instant::now();
                    let img = ultrakey_platform::image_preprocess::preprocess(&c.image, preset);
                    let obs = vision_ocr::recognize_text(&img, &params).unwrap_or_default();
                    let ms = t.elapsed().as_secs_f64() * 1000.0;
                    samples.push(ms);
                    *total += ms;
                    count = obs.len();
                }
                println!(
                    "  {level:<10} {name:<20} id={} 후보 {:>4}개 — {}",
                    c.geometry.display_id,
                    count,
                    stats(&samples)
                );
            }
            println!(
                "  {level:<10} {name:<20} ⭐ 전체 화면 합계(순차) — {}",
                stats(&total_per_round)
            );
        }
    }
    println!();

    // ── 3. 디스플레이별 병렬 ───────────────────────────────────────────────
    println!("## 4. 디스플레이별 병렬 실행 (`.accurate`, 전처리 None)");
    if captured.len() > 1 {
        let mut samples = Vec::new();
        for _ in 0..ROUNDS {
            let t = Instant::now();
            std::thread::scope(|s| {
                let handles: Vec<_> = captured
                    .iter()
                    .map(|c| {
                        s.spawn(move || {
                            let img = ultrakey_platform::image_preprocess::preprocess(
                                &c.image,
                                PreprocessPreset::None,
                            );
                            vision_ocr::recognize_text(&img, &RecognitionParams::default())
                                .unwrap_or_default()
                                .len()
                        })
                    })
                    .collect();
                for h in handles {
                    let _ = h.join();
                }
            });
            samples.push(t.elapsed().as_secs_f64() * 1000.0);
        }
        println!("  ⭐ 전체 화면 합계(병렬) — {}", stats(&samples));
    } else {
        println!("  (디스플레이가 1개라 생략)");
    }
    println!();

    // ── 4. 조용한 실패 감지 ────────────────────────────────────────────────
    println!("## 5. Screen Recording 권한 — 조용한 실패 판정");
    println!(
        "  CGPreflightScreenCaptureAccess() = {}",
        ultrakey_platform::screen_recording::has_screen_recording_access()
    );
    for c in &captured {
        let img = ultrakey_platform::image_preprocess::preprocess(&c.image, PreprocessPreset::None);
        let n = vision_ocr::recognize_text(&img, &RecognitionParams::default())
            .unwrap_or_default()
            .len();
        println!(
            "  id={} OCR 후보 {}개 → {}",
            c.geometry.display_id,
            n,
            if n == 0 {
                "⚠️ 0개 — 권한 없음(배경만 캡처)일 가능성"
            } else {
                "정상"
            }
        );
    }
}

#[cfg(target_os = "macos")]
fn stats(samples: &[f64]) -> String {
    if samples.is_empty() {
        return "샘플 없음".into();
    }
    let mut v = samples.to_vec();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let min = v[0];
    let max = v[v.len() - 1];
    let median = v[v.len() / 2];
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    format!("중앙값 {median:>7.1}ms (평균 {mean:.1}, 최소 {min:.1}, 최대 {max:.1}, n={})", v.len())
}
