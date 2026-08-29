//! 핫플러그 스레드 친화성 대조 실험(이슈 #10).
//!
//! ⭐ **이 프로브는 TCC 권한이 전혀 필요 없다.** `IOServiceAddMatchingNotification`
//! 은 Accessibility/Input Monitoring 없이 동작하므로(`hotplug.rs` 모듈 문서),
//! 앱·서명·권한 토글 없이 터미널에서 바로 돌릴 수 있다. 즉 이슈 #10 의 원인
//! 가설만 **격리해서** 확인할 수 있다.
//!
//! 두 스레드에 각각 `watch_keyboards` 를 등록하고, 한쪽만 `CFRunLoopRun` 을 돈다:
//!
//! | 스레드 | 런루프 구동 | 대응하는 실제 경로 |
//! | :--- | :--- | :--- |
//! | `no-runloop`   | ⛔ 안 돈다 (`recv_timeout` 루프) | 경로 B — `PermissionMonitor` 폴링 스레드 |
//! | `with-runloop` | ✅ `CFRunLoopRun()`              | 경로 A — Tauri 메인 스레드 |
//!
//! 실행: `cargo run -p ultrakey-platform --example hotplug_thread_probe`
//! 그 뒤 외장 USB 키보드를 뺐다 꽂고, 어느 스레드가 콜백을 받았는지 본다.

fn main() {
    #[cfg(not(target_os = "macos"))]
    {
        eprintln!("이 프로브는 macOS 전용이다.");
    }

    #[cfg(target_os = "macos")]
    {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::{Duration, Instant};
        use ultrakey_platform::hotplug::{watch_keyboards, HotplugEvent};
        use ultrakey_platform::runloop::RunLoopHandle;

        let no_runloop_hits = Arc::new(AtomicUsize::new(0));
        let with_runloop_hits = Arc::new(AtomicUsize::new(0));

        // ── 경로 B 재현: 런루프를 돌리지 않는 스레드에서 등록한다 ──
        let hits_b = Arc::clone(&no_runloop_hits);
        let t_b = std::thread::Builder::new()
            .name("no-runloop".to_string())
            .spawn(move || {
                let _watcher = watch_keyboards(Box::new(move |ev| {
                    let label = match ev {
                        HotplugEvent::Attached => "연결",
                        HotplugEvent::Detached => "해제",
                    };
                    hits_b.fetch_add(1, Ordering::SeqCst);
                    println!("  [no-runloop]   ⚠️ 콜백 도착 — {label}");
                }));
                println!("[no-runloop]   등록 완료 — CFRunLoopRun 을 호출하지 않고 대기한다");
                // 폴링 스레드처럼 그냥 잠들어 있는다. 런루프는 돌지 않는다.
                std::thread::sleep(Duration::from_secs(90));
            })
            .expect("no-runloop 스레드 생성 실패");

        // ── 경로 A 재현: 등록 후 그 스레드에서 CFRunLoopRun 을 돈다 ──
        let hits_a = Arc::clone(&with_runloop_hits);
        let t_a = std::thread::Builder::new()
            .name("with-runloop".to_string())
            .spawn(move || {
                let _watcher = watch_keyboards(Box::new(move |ev| {
                    let label = match ev {
                        HotplugEvent::Attached => "연결",
                        HotplugEvent::Detached => "해제",
                    };
                    hits_a.fetch_add(1, Ordering::SeqCst);
                    println!("  [with-runloop] ✅ 콜백 도착 — {label}");
                }));
                println!("[with-runloop] 등록 완료 — CFRunLoopRun 을 돈다");
                let deadline = Instant::now() + Duration::from_secs(90);
                while Instant::now() < deadline {
                    RunLoopHandle::run();
                    std::thread::sleep(Duration::from_millis(50));
                }
            })
            .expect("with-runloop 스레드 생성 실패");

        println!();
        println!("=== 이제 외장 USB 키보드를 뽑았다가 다시 꽂아라 (90초 대기) ===");
        println!();

        let _ = t_b.join();
        let _ = t_a.join();

        let b = no_runloop_hits.load(Ordering::SeqCst);
        let a = with_runloop_hits.load(Ordering::SeqCst);
        println!();
        println!("=== 결과 ===");
        println!("  런루프를 돌지 않는 스레드(경로 B 재현): {b} 건");
        println!("  런루프를 도는 스레드(경로 A 재현):      {a} 건");
        if a > 0 && b == 0 {
            println!("⭐ 진단 확증 — 등록 스레드의 런루프가 돌아야만 IOKit 콜백이 발화한다.");
        } else if a == 0 && b == 0 {
            println!("⚠️ 양쪽 다 0 — 착탈이 감지되지 않았다. 매칭 조건이나 무장(arm)을 의심하라.");
        } else {
            println!("⚠️ 예상과 다르다 — 결과를 그대로 기록하고 다시 판단하라.");
        }
    }
}
