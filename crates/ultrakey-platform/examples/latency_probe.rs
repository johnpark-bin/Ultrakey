//! 검증 보조 도구 — 입력 지연(이슈 #139) 실측용 마우스 이벤트 왕복 측정기.
//!
//! ⭐ 왜 필요한가: 이슈 #139 는 "입력이 통째로 멈추는 시간"을 눈으로만 보고했다.
//! `docs/plan/issue-139-input-latency.md` §5 실측을 하려면 "합성한 시점"과
//! "다시 관측한 시점"의 차이를 기계가 재는 도구가 있어야 한다. 이 도구는 별도
//! 스레드에서 일정 간격으로 마우스 `MouseMoved` 이벤트를 세션에 올리고(현재
//! 커서 위치로 "이동"시키므로 무해하다), 같은 프로세스의 탭이 그 이벤트가 다시
//! 세션 스트림에 나타나는 시점을 재서 왕복 지연을 계산한다. 지연이 크면(경로 B
//! 재적용처럼 탭 런루프를 블로킹하는 작업이 끼면) 이 왕복도 함께 늦어진다 —
//! 그 지연을 스파이크로 즉시 보고하고, 종료 시 분포 요약을 낸다.
//!
//! **측정 원리**: 발행 스레드가 `CGEventSourceCreate(HIDSystemState)` 로 만든
//! 소스로 `MouseMoved` 이벤트를 합성하면서, `EventSourceUserData` 필드(사용자
//! 정의 슬롯, 필드 번호 42)에 `(0x4C << 56) | ns_since_probe_start` 를 실어
//! `CGEventPost(HIDEventTap)` 로 HID 이벤트 탭 위치에 올린다. 메인 스레드는
//! `CGEventTapCreate(SessionEventTap, TailAppendEventTap, ListenOnly, mask=
//! MouseMoved)` 로 만든 리슨 전용 탭으로 이 이벤트가 세션에 다시 나타나는
//! 시점을 잡는다 — 상위 바이트가 `0x4C` 인 이벤트만 우리가 만든 마커로 보고
//! `now_ns - encoded_ns` 를 지연으로 기록한다. 그 외 이벤트(사용자가 실제로
//! 움직인 마우스)는 무시한다.
//!
//! ⛔ **`ULTRAKEY_MAGIC`(`event.rs`)을 쓰지 않는다.** Ultrakey 의 탭은 콜백 0-a
//! 단계에서 그 마커를 보면 자기 합성 이벤트로 판단해 판정 없이 즉시 통과시킨다
//! (`event_tap.rs`, `key_poke.rs` 모듈 문서와 같은 이유). 이 도구가 재려는
//! 것은 정확히 "탭 런루프가 얼마나 지연되는가"이므로, 그 런루프를 우회하는
//! 마커를 쓰면 측정 자체가 무의미해진다. 그래서 `0x4C`(임의로 고른, `ULTRAKEY_
//! MAGIC` 과 겹치지 않는 상위 바이트)를 쓴다.
//!
//! **사용법**:
//! ```sh
//! cargo run -p ultrakey-platform --example latency_probe -- \
//!     --interval-ms 10 --duration-s 30 --spike-ms 20 --csv /tmp/latency.csv
//! ```
//! 인자는 전부 기본값이 있다 — `--interval-ms`(기본 10) `--duration-s`(기본 30)
//! `--spike-ms`(기본 20, 이 값 이상인 샘플은 즉시 stderr 에 보고) `--csv`(옵션,
//! 샘플별 `wall_time_ms,seq,latency_ms` 를 그 경로에 기록).
//!
//! ⛔ **이 도구로 측정되지 않는 것**:
//! - **탭 자체가 아니라 이 도구가 만드는 마우스 이벤트의 왕복**을 잰다. 실제
//!   키보드 이벤트 경로(`event_tap.rs` 트램폴린)의 지연과 다를 수 있다 — 다만
//!   둘 다 같은 탭 런루프를 쓰므로, 런루프가 블로킹되면 둘 다 함께 늦어진다는
//!   전제로 쓴다(§7, D5).
//! - 스파이크 시각은 **UTC** 로 찍는다(타임존 크레이트 없이 로컬 시각을 정확히
//!   구할 수 없다). 앱 로그(로컬 시각)와 대조하려면 오프셋을 사람이 보정한다.
//! - Accessibility 권한이 없으면 탭 생성 자체가 실패한다 — 그 사실을 그대로
//!   보고하고 종료한다(측정 불가로 취급, 조용히 넘어가지 않는다).

#[cfg(not(target_os = "macos"))]
fn main() {}

#[cfg(target_os = "macos")]
fn main() {
    use objc2_core_foundation::{
        kCFRunLoopCommonModes, kCFRunLoopDefaultMode, CFMachPort, CFRunLoop, CFRunLoopSource,
    };
    use objc2_core_graphics::{
        CGEvent, CGEventField, CGEventMask, CGEventSource, CGEventSourceStateID,
        CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventTapProxy, CGEventType,
        CGMouseButton,
    };
    use std::ffi::c_void;
    use std::fs::File;
    use std::io::{BufWriter, Write};
    use std::path::PathBuf;
    use std::ptr::NonNull;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};
    use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

    /// 우리가 심는 마커의 상위 바이트. `ULTRAKEY_MAGIC` 과 겹치지 않는 값이면
    /// 되고, 값 자체에 다른 의미는 없다(위 모듈 문서 참고).
    const PROBE_TAG: i64 = 0x4C;
    /// 하위 56비트에 실을 나노초 마스크(≈2.3 년 여유, 30~60 s 짜리 실행에는
    /// 넘칠 일이 없다).
    const NS_MASK: i64 = (1i64 << 56) - 1;

    struct Args {
        interval_ms: u64,
        duration_s: u64,
        spike_ms: f64,
        csv: Option<PathBuf>,
    }

    fn take_value(raw: &[String], i: &mut usize, name: &str, inline: Option<String>) -> String {
        if let Some(v) = inline {
            return v;
        }
        *i += 1;
        match raw.get(*i) {
            Some(v) => v.clone(),
            None => {
                eprintln!("{name} 뒤에 값이 없다");
                std::process::exit(2);
            }
        }
    }

    fn parse_args() -> Args {
        let mut interval_ms: u64 = 10;
        let mut duration_s: u64 = 30;
        let mut spike_ms: f64 = 20.0;
        let mut csv: Option<PathBuf> = None;

        let raw: Vec<String> = std::env::args().skip(1).collect();
        let mut i = 0;
        while i < raw.len() {
            let arg = raw[i].clone();
            let (name, inline) = match arg.split_once('=') {
                Some((n, v)) => (n.to_string(), Some(v.to_string())),
                None => (arg.clone(), None),
            };
            match name.as_str() {
                "--interval-ms" => {
                    let v = take_value(&raw, &mut i, &name, inline);
                    interval_ms = v.parse().unwrap_or_else(|_| {
                        eprintln!("--interval-ms 는 정수여야 한다: {v}");
                        std::process::exit(2);
                    });
                }
                "--duration-s" => {
                    let v = take_value(&raw, &mut i, &name, inline);
                    duration_s = v.parse().unwrap_or_else(|_| {
                        eprintln!("--duration-s 는 정수여야 한다: {v}");
                        std::process::exit(2);
                    });
                }
                "--spike-ms" => {
                    let v = take_value(&raw, &mut i, &name, inline);
                    spike_ms = v.parse().unwrap_or_else(|_| {
                        eprintln!("--spike-ms 는 숫자여야 한다: {v}");
                        std::process::exit(2);
                    });
                }
                "--csv" => {
                    let v = take_value(&raw, &mut i, &name, inline);
                    csv = Some(PathBuf::from(v));
                }
                other => {
                    eprintln!("알 수 없는 인자: {other}");
                    std::process::exit(2);
                }
            }
            i += 1;
        }
        Args {
            interval_ms,
            duration_s,
            spike_ms,
            csv,
        }
    }

    /// 발행(producer)·수신(콜백) 양쪽이 공유하는 측정 상태. 콜백은 우리 탭이
    /// 아니라 측정 도구 자신의 것이므로 `Mutex` 를 써도 §2.2 의 "탭 콜백 안
    /// 잠금 금지" 규약과 무관하다(planner 지시).
    struct ProbeState {
        start: Instant,
        posted: AtomicU64,
        received: AtomicU64,
        latencies_ms: Mutex<Vec<f64>>,
        csv: Mutex<Option<BufWriter<File>>>,
        spike_ms: f64,
    }

    static PROBE: OnceLock<ProbeState> = OnceLock::new();

    /// 현재 UTC 시각을 `HH:MM:SS.mmm` 로 포맷한다. 타임존 크레이트 없이 로컬
    /// 시각은 정확히 구할 수 없으므로 UTC 로 찍고 라벨을 붙인다(모듈 문서 참고).
    fn format_utc_clock() -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        let secs_of_day = now.as_secs() % 86_400;
        let h = secs_of_day / 3600;
        let m = (secs_of_day % 3600) / 60;
        let s = secs_of_day % 60;
        let ms = now.subsec_millis();
        format!("{h:02}:{m:02}:{s:02}.{ms:03} (UTC)")
    }

    /// 정렬된 표본에서 백분위수를 구한다(최근접 순위 방식).
    fn percentile(sorted: &[f64], p: f64) -> f64 {
        if sorted.is_empty() {
            return 0.0;
        }
        let rank = (p / 100.0 * (sorted.len() as f64 - 1.0)).round() as usize;
        sorted[rank.min(sorted.len() - 1)]
    }

    unsafe extern "C-unwind" fn callback(
        _proxy: CGEventTapProxy,
        etype: CGEventType,
        event: NonNull<CGEvent>,
        _ud: *mut c_void,
    ) -> *mut CGEvent {
        if etype == CGEventType::MouseMoved {
            let ev = unsafe { event.as_ref() };
            let ud = CGEvent::integer_value_field(Some(ev), CGEventField::EventSourceUserData);
            let tag = (ud >> 56) & 0xFF;
            if tag == PROBE_TAG {
                if let Some(probe) = PROBE.get() {
                    let encoded_ns = ud & NS_MASK;
                    let now_ns = probe.start.elapsed().as_nanos() as i64;
                    let latency_ns = now_ns - encoded_ns;
                    let latency_ms = latency_ns as f64 / 1_000_000.0;
                    let seq = probe.received.fetch_add(1, Ordering::SeqCst);

                    if let Ok(mut v) = probe.latencies_ms.lock() {
                        v.push(latency_ms);
                    }
                    if let Ok(mut guard) = probe.csv.lock() {
                        if let Some(w) = guard.as_mut() {
                            let wall_ms = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map(|d| d.as_millis())
                                .unwrap_or(0);
                            let _ = writeln!(w, "{wall_ms},{seq},{latency_ms:.3}");
                        }
                    }
                    if latency_ms >= probe.spike_ms {
                        eprintln!(
                            "spike {} latency_ms={:.2} seq={}",
                            format_utc_clock(),
                            latency_ms,
                            seq
                        );
                    }
                }
            }
        }
        event.as_ptr()
    }

    let args = parse_args();

    let csv_writer = args.csv.as_ref().map(|path| {
        let file = File::create(path).unwrap_or_else(|e| {
            eprintln!("--csv 파일을 열 수 없다: {path:?}: {e}");
            std::process::exit(2);
        });
        let mut w = BufWriter::new(file);
        let _ = writeln!(w, "wall_time_ms,seq,latency_ms");
        w
    });

    let start = Instant::now();
    PROBE
        .set(ProbeState {
            start,
            posted: AtomicU64::new(0),
            received: AtomicU64::new(0),
            latencies_ms: Mutex::new(Vec::new()),
            csv: Mutex::new(csv_writer),
            spike_ms: args.spike_ms,
        })
        .ok()
        .expect("PROBE 는 여기서 한 번만 초기화된다");

    // ── 발행 스레드 ──
    let stop = Arc::new(AtomicBool::new(false));
    let producer_stop = Arc::clone(&stop);
    let interval_ms = args.interval_ms;
    let producer = std::thread::Builder::new()
        .name("latency-probe-producer".to_string())
        .spawn(move || {
            let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState);
            let source_ref = source.as_deref();
            let probe = PROBE.get().expect("PROBE 초기화 이후에만 스폰된다");
            while !producer_stop.load(Ordering::Relaxed) {
                // 현재 커서 위치를 얻는다 — `CGEvent::new(None)` 이 만드는 "현재
                // 상태" 이벤트에서 `location` 을 읽는다(모듈 문서 참고).
                let cur = CGEvent::new(None);
                let location = cur
                    .as_deref()
                    .map(|e| CGEvent::location(Some(e)))
                    .unwrap_or_default();

                let ns = probe.start.elapsed().as_nanos() as i64 & NS_MASK;
                let tagged = (PROBE_TAG << 56) | ns;

                if let Some(mv) = CGEvent::new_mouse_event(
                    source_ref,
                    CGEventType::MouseMoved,
                    location,
                    CGMouseButton::Left,
                ) {
                    CGEvent::set_integer_value_field(
                        Some(&mv),
                        CGEventField::EventSourceUserData,
                        tagged,
                    );
                    CGEvent::post(CGEventTapLocation::HIDEventTap, Some(&mv));
                    probe.posted.fetch_add(1, Ordering::SeqCst);
                } else {
                    eprintln!("MouseMoved 이벤트 합성 실패");
                }

                std::thread::sleep(Duration::from_millis(interval_ms));
            }
        })
        .expect("발행 스레드 생성 실패");

    // ── 수신 탭 ──
    let mask: CGEventMask = 1 << CGEventType::MouseMoved.0;
    let port: Option<objc2_core_foundation::CFRetained<CFMachPort>> = unsafe {
        CGEvent::tap_create(
            CGEventTapLocation::SessionEventTap,
            CGEventTapPlacement::TailAppendEventTap,
            CGEventTapOptions::ListenOnly,
            mask,
            Some(callback),
            core::ptr::null_mut(),
        )
    };
    let Some(port) = port else {
        stop.store(true, Ordering::Relaxed);
        let _ = producer.join();
        eprintln!("탭 생성 실패 — Accessibility 권한을 확인한다");
        std::process::exit(1);
    };
    let source = CFMachPort::new_run_loop_source(None, Some(&port), 0).expect("run loop source");
    let rl = CFRunLoop::current().expect("run loop");
    rl.add_source(Some::<&CFRunLoopSource>(&source), unsafe {
        kCFRunLoopCommonModes
    });
    CGEvent::tap_enable(&port, true);

    eprintln!(
        "latency_probe 시작 — interval_ms={} duration_s={} spike_ms={}",
        args.interval_ms, args.duration_s, args.spike_ms
    );

    // duration 만큼 짧은 슬라이스로 런루프를 돌리며 경과 시간을 확인한다.
    let deadline = Instant::now() + Duration::from_secs(args.duration_s);
    while Instant::now() < deadline {
        CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, 0.1, false);
    }
    rl.stop();
    CGEvent::tap_enable(&port, false);

    stop.store(true, Ordering::Relaxed);
    let _ = producer.join();

    let probe = PROBE.get().expect("PROBE 는 종료 시점까지 살아있다");
    let posted = probe.posted.load(Ordering::SeqCst);
    let received = probe.received.load(Ordering::SeqCst);
    let lost = posted.saturating_sub(received);

    let mut sorted = probe
        .latencies_ms
        .lock()
        .map(|v| v.clone())
        .unwrap_or_default();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p50 = percentile(&sorted, 50.0);
    let p95 = percentile(&sorted, 95.0);
    let p99 = percentile(&sorted, 99.0);
    let max = sorted.last().copied().unwrap_or(0.0);
    let spikes = sorted.iter().filter(|&&v| v >= args.spike_ms).count();

    println!("posted={posted} received={received} lost={lost}");
    println!(
        "p50={p50:.2}ms p95={p95:.2}ms p99={p99:.2}ms max={max:.2}ms spikes(>= {:.0}ms)={spikes}",
        args.spike_ms
    );
}
