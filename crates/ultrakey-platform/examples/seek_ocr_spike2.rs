//! F-02 Phase 1 (계속) — 1차 스파이크가 남긴 세 질문에 답하는 2차 측정.
//!
//! 1. **워밍업 비용** — 첫 OCR 호출이 이후보다 얼마나 비싼가(Vision 모델 로드).
//! 2. **Vision 이 정말 직렬화되는가** — 같은 디스플레이 2장을 두 스레드로
//!    동시에 돌려 1차의 "병렬 이득 0" 을 교차 확인한다.
//! 3. **영역 한정 캡처**(`Only Seek in the frontmost window`, §3.3.2)의 지연 —
//!    전체 화면 대비 얼마나 싸지는가.
//!
//! 실행: cargo run -p ultrakey-platform --release --example seek_ocr_spike2

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    use std::time::Instant;
    use ultrakey_platform::image_preprocess::{preprocess, PreprocessPreset};
    use ultrakey_platform::screen_capture;
    use ultrakey_platform::vision_ocr::{recognize_text, RecognitionParams};

    let displays = screen_capture::active_displays();
    let params = RecognitionParams::default();

    // ── 1. 워밍업 ─────────────────────────────────────────────────────────
    println!("## 6. 워밍업 비용 (프로세스 첫 OCR vs 이후)");
    let Some(&main_display) = displays.first() else {
        eprintln!("디스플레이 없음");
        return;
    };
    let Some(cap) = screen_capture::capture_display(main_display) else {
        eprintln!("캡처 실패");
        return;
    };
    for round in 0..6 {
        let t = Instant::now();
        let img = preprocess(&cap.image, PreprocessPreset::None);
        let n = recognize_text(&img, &params).unwrap_or_default().len();
        println!(
            "  회차 {round}: {:>7.1}ms (후보 {n}개){}",
            t.elapsed().as_secs_f64() * 1000.0,
            if round == 0 { "  ← 콜드" } else { "" }
        );
    }
    println!();

    // ── 2. 같은 이미지 2장 동시 OCR ────────────────────────────────────────
    println!("## 7. Vision 병렬성 — 같은 이미지 2장");
    let seq = bench(5, || {
        for _ in 0..2 {
            let img = preprocess(&cap.image, PreprocessPreset::None);
            let _ = recognize_text(&img, &params);
        }
    });
    let par = bench(5, || {
        std::thread::scope(|s| {
            let handles: Vec<_> = (0..2)
                .map(|_| {
                    s.spawn(|| {
                        let img = preprocess(&cap.image, PreprocessPreset::None);
                        recognize_text(&img, &RecognitionParams::default())
                            .unwrap_or_default()
                            .len()
                    })
                })
                .collect();
            for h in handles {
                let _ = h.join();
            }
        });
    });
    println!("  순차 2회: {seq:>7.1}ms");
    println!("  병렬 2회: {par:>7.1}ms  → 이득 {:.0}%", (1.0 - par / seq) * 100.0);
    println!();

    // ── 3. 영역 한정 캡처 ─────────────────────────────────────────────────
    println!("## 8. 영역 한정 캡처 (`Only Seek in the frontmost window` 경로)");
    for (label, w, h) in [
        ("전체 화면", main_display.width_pt, main_display.height_pt),
        ("창 크기 1600x1000", 1600.0, 1000.0),
        ("창 크기 1200x800", 1200.0, 800.0),
        ("창 크기 800x600", 800.0, 600.0),
    ] {
        let mut n = 0;
        let ms = bench(5, || {
            if let Some(c) = screen_capture::capture_display_rect(
                main_display,
                main_display.origin_x,
                main_display.origin_y,
                w,
                h,
            ) {
                let img = preprocess(&c.image, PreprocessPreset::None);
                n = recognize_text(&img, &params).unwrap_or_default().len();
            }
        });
        println!("  {label:<18} {w:>6.0}x{h:<6.0} — 캡처+OCR {ms:>7.1}ms (후보 {n}개)");
    }
}

#[cfg(target_os = "macos")]
fn bench(rounds: usize, mut f: impl FnMut()) -> f64 {
    use std::time::Instant;
    let mut v: Vec<f64> = (0..rounds)
        .map(|_| {
            let t = Instant::now();
            f();
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    v[v.len() / 2]
}
