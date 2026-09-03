//! 검증 보조 도구 — 경로 C(`IOHIDSetModifierLockState`)로 caps lock 하드웨어 잠금을
//! 켰다가 되돌린다(이슈 #110 실기기 절차 M7).
//!
//! ⭐ 왜 필요한가: 이슈 #110 의 방어선(`Arbiter::arbitrate` 의 D-1 우회 탭 환원)은
//! "`caps_lock_alias` 가 켜진 동안 keycode `0x39` 의 `FlagsChanged` 는 물리 caps lock
//! 에서만 온다"를 전제로 한다. 경로 C 가 잠금 상태를 바꿀 때 세션 스트림에 같은
//! 모양의 `FlagsChanged` 가 실리는지는 코드로 알 수 없고 실측만이 답한다. 이 도구를
//! `tap_listen` 과 함께 돌려 그 이벤트가 찍히는지 본다:
//! ```sh
//! cargo run -p ultrakey-platform --example tap_listen > /tmp/tap.log &
//! cargo run -p ultrakey-platform --example caps_lock_toggle_probe
//! grep -i 'flagsChanged' /tmp/tap.log | grep -i '0x39\|keycode=57'
//! ```
//!
//! ⛔ 이 도구는 잠금을 **원래 상태로 되돌리고** 끝난다 — 사용자의 caps lock 상태를
//! 바꿔 두지 않는다. `UserKeyMapping` 은 건드리지 않는다.

use std::thread::sleep;
use std::time::Duration;

use ultrakey_platform::hid_lock::{caps_lock_state, set_caps_lock_state};

fn main() {
    let before = caps_lock_state();
    println!("before: {before:?}");
    let target = !before.unwrap_or(false);

    let ok = set_caps_lock_state(target);
    println!("set {target}: ok={ok} state={:?}", caps_lock_state());
    sleep(Duration::from_millis(600));

    let restored = set_caps_lock_state(before.unwrap_or(false));
    println!(
        "restore {:?}: ok={restored} state={:?}",
        before.unwrap_or(false),
        caps_lock_state()
    );
}
