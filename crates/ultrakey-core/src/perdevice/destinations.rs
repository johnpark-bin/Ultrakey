//! F-17 기능 2 목적지 카탈로그(`docs/spec/per-device-settings.md` §3.5) — 313종.
//!
//! ⭐ 이슈 #31 ②. 사용자 보고: "Function Keys 옵션이 거의 구현되지 않았다."
//! 예전에는 [`super::SystemFunction`] 12종(그중 경로 B 로 표현 가능한 것은 8종)
//! 뿐이었다. 이 모듈이 그 자리를 대신한다.
//!
//! ## 이 표의 출처
//!
//! 두 가지 **사실**을 합쳐 만들었다. ⛔ 어느 쪽 코드·리소스도 복사하지 않았다 —
//! 목록과 숫자라는 사실만 참조하고 자료구조는 우리 것이다.
//!
//! 1. **어떤 목적지가 존재하는가 · 어느 카테고리인가** — 이 머신에 설치된
//!    `/Applications/Karabiner-Elements.app/Contents/Resources/simple_modifications.json`
//!    (읽기 전용으로 조사). 이슈 #31 의 스크린샷에 보이는 그 트리다.
//! 2. **각 이름의 HID usage 숫자** — `pqrs-org/cpp-hid` 의
//!    `include/pqrs/hid/usage.hpp`(USB HID Usage Tables 와 Apple 의 벤더 정의
//!    usage page 를 그대로 옮긴 표)와, Karabiner 의 이름 별칭 표
//!    (`src/share/types/momentary_switch_event_details/*.hpp`)를 조인해 얻었다.
//!    usage page 번호는 이 머신의 SDK 헤더
//!    (`IOKit.framework/Headers/hid/IOHIDUsageTables.h`)와 교차 확인했다.
//!
//! ## 무엇을 뺐는가 — 경로 B 로 표현할 수 없는 것
//!
//! 이슈 #31 의 요구: "경로 B 로 실제 표현 가능한 것만 넣어라 … 고를 수는 있는데
//! 아무 일도 안 일어나는 UX 를 만들지 마라." `UserKeyMapping` 의 `Dst` 는
//! `(page << 32) | usage` 한 쌍이므로, **usage page/usage 쌍으로 표현되지 않는
//! 것은 애초에 후보가 될 수 없다.** 그래서 아래 카테고리를 통째로 뺐다:
//!
//! | 뺀 카테고리 | 이유 |
//! | :--- | :--- |
//! | Mouse buttons (255종) | Button page(0x09) — `UserKeyMapping` 은 키보드 usage 만 다룬다 |
//! | Mouse keys (28종) | Karabiner 내부 가상 기능(포인터 이동). HID usage 가 아니다 |
//! | Sticky modifier keys (18종) | Karabiner 내부 상태 기계. HID usage 가 아니다 |
//! | Software function (9종) | Karabiner 내부 기능(설정 창 열기 등). HID usage 가 아니다 |
//! | D-pad · Generic desktop keys | Generic Desktop page(0x01) — 우리 닫힌 어휘(D-17-4)의 허용 page 밖 |
//! | `do_not_disturb` | 같은 이유(Generic Desktop page). §3.5 의 12종 중 유일하게 여기서 탈락한다 |
//!
//! 또 **별칭은 값 기준으로 중복 제거했다.** Karabiner 의 `Others` 카테고리는
//! 대부분 같은 usage 를 가리키는 옛 이름들이고(`left_alt` = `left_option`,
//! `vk_mission_control` = `mission_control` …), `Japanese` 5종도 전부
//! `International keys` 의 `lang1`/`lang2`/`international2`/`4`/`5` 와 같은 값이다.
//! 같은 값을 두 번 보여 주면 사용자가 "무엇이 다른가"를 알 수 없다 — 카테고리
//! 순서상 먼저 나오는 것 하나만 남겼다. 그래서 카테고리는 15개다(Karabiner 의
//! 19개에서 위 표의 넷과 `Japanese` 가 빠진 것).
//!
//! ## ⚠️ 동작 근거 등급
//!
//! [`PathBSupport`] 를 보라. **usage 숫자는 전부 표준·벤더 표에서 온 사실이지만,
//! 그 page 를 `UserKeyMapping` 의 목적지로 썼을 때 실제로 동작하는지는 page 마다
//! 근거가 다르다.** 물리 키를 눌러 보지 않고는 검증할 수 없어, 미검증인 것은
//! 미검증이라고 표시하고 UI 가 그 사실을 사용자에게 보여 준다.

use super::Evidence;

/// 목적지 카테고리 — 팝업의 `<optgroup>` 하나에 대응한다(§3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DestinationCategory {
    Disable,
    ModifierKeys,
    ControlsAndSymbols,
    ArrowKeys,
    LetterKeys,
    NumberKeys,
    FunctionKeys,
    MediaControls,
    KeypadKeys,
    PcKeyboardKeys,
    InternationalKeys,
    ApplicationLaunchKeys,
    GuiApplicationControlKeys,
    RemoteControlButtons,
    Others,
}

impl DestinationCategory {
    /// 팝업 표시 순서 그대로 15종 전량.
    pub fn all() -> &'static [DestinationCategory] {
        use DestinationCategory::*;
        &[
            Disable,
            ModifierKeys,
            ControlsAndSymbols,
            ArrowKeys,
            LetterKeys,
            NumberKeys,
            FunctionKeys,
            MediaControls,
            KeypadKeys,
            PcKeyboardKeys,
            InternationalKeys,
            ApplicationLaunchKeys,
            GuiApplicationControlKeys,
            RemoteControlButtons,
            Others,
        ]
    }

    /// `preferences.keyboards.functionKeys.category.<세그먼트>` i18n 키의 뒷부분.
    /// ⭐ 카테고리 **이름만** 번역한다 — 개별 목적지 라벨은 HID usage 이름
    /// 그대로 두고 번역하지 않는다(`SourceKey` 의 키캡 각인과 같은 규약).
    pub fn label_segment(self) -> &'static str {
        use DestinationCategory::*;
        match self {
            Disable => "disable",
            ModifierKeys => "modifierKeys",
            ControlsAndSymbols => "controlsAndSymbols",
            ArrowKeys => "arrowKeys",
            LetterKeys => "letterKeys",
            NumberKeys => "numberKeys",
            FunctionKeys => "functionKeys",
            MediaControls => "mediaControls",
            KeypadKeys => "keypadKeys",
            PcKeyboardKeys => "pcKeyboardKeys",
            InternationalKeys => "internationalKeys",
            ApplicationLaunchKeys => "applicationLaunchKeys",
            GuiApplicationControlKeys => "guiApplicationControlKeys",
            RemoteControlButtons => "remoteControlButtons",
            Others => "others",
        }
    }
}

/// 이 목적지가 경로 B 에서 **실제로 동작하는지**에 대한 근거 등급.
/// ⚠️ usage **숫자**의 근거가 아니다 — 숫자는 전부 표준·벤더 표에서 온 사실이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathBSupport {
    /// 이 usage page 를 `UserKeyMapping` 의 목적지로 쓰면 실제로 동작하는 것이
    /// 실측으로 확인됐다 — Keyboard page(0x07)는 F-17 실기 확인, Consumer
    /// page(0x0C)는 스파이크 S-8(`docs/research/per-device-hid-spike.md` §8,
    /// 키 입력으로 확인).
    Measured,
    /// usage 값은 Apple 의 벤더 정의 page(`AppleVendorTopCase` 0x00FF /
    /// `AppleVendorKeyboard` 0xFF01) 표에서 온 사실이지만, **그 page 를 목적지로
    /// 썼을 때 실제로 동작하는지는 확인하지 못했다.** 물리 키 입력 없이는
    /// 검증할 수 없다(§9 미해결 질문). 이 page 자체는 D-17-4 의 허용 집합에 이미
    /// 들어 있고, `SourceKey::Globe`(0xFF00000003)로 이미 출하돼 있다.
    VendorPageUnverified,
    /// `Disable this key` — Keyboard page 의 usage `0x00`("Reserved (no event
    /// indicated)")을 목적지로 쓴다. page 자체는 실측됐지만 **usage 0 을 목적지로
    /// 줬을 때 IOHID 가 이벤트를 버리는지**는 확인하지 못했다(같은 이유).
    DisableUnverified,
}

impl PathBSupport {
    /// 동작이 실측으로 확인된 등급인가. UI 가 "동작 미확인" 힌트를 붙일지 가른다.
    pub fn is_verified(self) -> bool {
        matches!(self, PathBSupport::Measured)
    }

    /// 기존 `SystemFunction::evidence()` 가 쓰는 [`Evidence`] 등급으로 옮긴다 —
    /// 두 축("숫자의 출처" 대 "동작 확인 여부")을 한 자리에서 비교할 수 있게.
    pub fn evidence(self) -> Evidence {
        match self {
            PathBSupport::Measured => Evidence::StandardTable,
            PathBSupport::VendorPageUnverified | PathBSupport::DisableUnverified => {
                Evidence::Unknown
            }
        }
    }
}

/// 목적지 하나.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FunctionDestination {
    /// 저장 표현 — `perDevice.<scope>.functionKeys.<f1..f12>` 에 이 문자열이 그대로
    /// 들어간다. `"<네임스페이스>.<HID usage 이름>"` 형태이고, `Disable this key`
    /// 만 예외로 `"disable"` 이다. ⭐ 라벨과 분리돼 있어 라벨을 바꿔도 저장된
    /// 설정이 깨지지 않는다(`SourceKey`/`KeyRemapRow` 와 같은 규약).
    pub id: &'static str,
    /// UI 라벨 — HID usage 이름 그대로다. ⛔ 번역하지 않는다(키캡 각인 규약).
    /// `disable` 만 예외로 UI 가 i18n 카탈로그에서 문구를 가져온다.
    pub label: &'static str,
    pub category: DestinationCategory,
    /// `(page << 32) | usage` — `UserKeyMapping` 의 `Dst`.
    pub value: u64,
    pub support: PathBSupport,
}

impl FunctionDestination {
    /// usage page — `value` 의 상위 32비트.
    pub fn page(&self) -> u64 {
        self.value >> 32
    }

    /// usage — `value` 의 하위 32비트.
    pub fn usage(&self) -> u64 {
        self.value & 0xFFFF_FFFF
    }
}

/// 카탈로그 전량, 팝업 표시 순서 그대로.
pub fn all() -> &'static [FunctionDestination] {
    ALL
}

/// 저장된 id 로 목적지를 찾는다. 모르는 id(손상된 설정·옛 버전이 쓴 값)면 `None`.
pub fn find(id: &str) -> Option<&'static FunctionDestination> {
    ALL.iter().find(|d| d.id == id)
}

// ⚠️ `rustfmt` 를 끈다 — 항목 하나가 한 줄인 **표**로 읽히는 것이 이 상수의
// 존재 이유다. 자동 포맷은 313개 항목을 각각 6줄로 펼쳐 2,700줄짜리 파일로
// 만들고, 그러면 "어떤 목적지가 어느 카테고리에 어떤 값으로 있는가"를 눈으로
// 훑을 수 없게 된다.
#[rustfmt::skip]
const ALL: &[FunctionDestination] = &[
    FunctionDestination { id: "disable", label: "vk_none", category: DestinationCategory::Disable, value: 0x700000000, support: PathBSupport::DisableUnverified },
    FunctionDestination { id: "key.caps_lock", label: "caps_lock", category: DestinationCategory::ModifierKeys, value: 0x700000039, support: PathBSupport::Measured },
    FunctionDestination { id: "key.left_control", label: "left_control", category: DestinationCategory::ModifierKeys, value: 0x7000000E0, support: PathBSupport::Measured },
    FunctionDestination { id: "key.left_shift", label: "left_shift", category: DestinationCategory::ModifierKeys, value: 0x7000000E1, support: PathBSupport::Measured },
    FunctionDestination { id: "key.left_option", label: "left_option", category: DestinationCategory::ModifierKeys, value: 0x7000000E2, support: PathBSupport::Measured },
    FunctionDestination { id: "key.left_command", label: "left_command", category: DestinationCategory::ModifierKeys, value: 0x7000000E3, support: PathBSupport::Measured },
    FunctionDestination { id: "key.right_control", label: "right_control", category: DestinationCategory::ModifierKeys, value: 0x7000000E4, support: PathBSupport::Measured },
    FunctionDestination { id: "key.right_shift", label: "right_shift", category: DestinationCategory::ModifierKeys, value: 0x7000000E5, support: PathBSupport::Measured },
    FunctionDestination { id: "key.right_option", label: "right_option", category: DestinationCategory::ModifierKeys, value: 0x7000000E6, support: PathBSupport::Measured },
    FunctionDestination { id: "key.right_command", label: "right_command", category: DestinationCategory::ModifierKeys, value: 0x7000000E7, support: PathBSupport::Measured },
    FunctionDestination { id: "appleTopCase.keyboard_fn", label: "keyboard_fn", category: DestinationCategory::ModifierKeys, value: 0xFF00000003, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "key.return_or_enter", label: "return_or_enter", category: DestinationCategory::ControlsAndSymbols, value: 0x700000028, support: PathBSupport::Measured },
    FunctionDestination { id: "key.escape", label: "escape", category: DestinationCategory::ControlsAndSymbols, value: 0x700000029, support: PathBSupport::Measured },
    FunctionDestination { id: "key.delete_or_backspace", label: "delete_or_backspace", category: DestinationCategory::ControlsAndSymbols, value: 0x70000002A, support: PathBSupport::Measured },
    FunctionDestination { id: "key.delete_forward", label: "delete_forward", category: DestinationCategory::ControlsAndSymbols, value: 0x70000004C, support: PathBSupport::Measured },
    FunctionDestination { id: "key.tab", label: "tab", category: DestinationCategory::ControlsAndSymbols, value: 0x70000002B, support: PathBSupport::Measured },
    FunctionDestination { id: "key.spacebar", label: "spacebar", category: DestinationCategory::ControlsAndSymbols, value: 0x70000002C, support: PathBSupport::Measured },
    FunctionDestination { id: "key.hyphen", label: "hyphen", category: DestinationCategory::ControlsAndSymbols, value: 0x70000002D, support: PathBSupport::Measured },
    FunctionDestination { id: "key.equal_sign", label: "equal_sign", category: DestinationCategory::ControlsAndSymbols, value: 0x70000002E, support: PathBSupport::Measured },
    FunctionDestination { id: "key.open_bracket", label: "open_bracket", category: DestinationCategory::ControlsAndSymbols, value: 0x70000002F, support: PathBSupport::Measured },
    FunctionDestination { id: "key.close_bracket", label: "close_bracket", category: DestinationCategory::ControlsAndSymbols, value: 0x700000030, support: PathBSupport::Measured },
    FunctionDestination { id: "key.backslash", label: "backslash", category: DestinationCategory::ControlsAndSymbols, value: 0x700000031, support: PathBSupport::Measured },
    FunctionDestination { id: "key.non_us_pound", label: "non_us_pound", category: DestinationCategory::ControlsAndSymbols, value: 0x700000032, support: PathBSupport::Measured },
    FunctionDestination { id: "key.semicolon", label: "semicolon", category: DestinationCategory::ControlsAndSymbols, value: 0x700000033, support: PathBSupport::Measured },
    FunctionDestination { id: "key.quote", label: "quote", category: DestinationCategory::ControlsAndSymbols, value: 0x700000034, support: PathBSupport::Measured },
    FunctionDestination { id: "key.grave_accent_and_tilde", label: "grave_accent_and_tilde", category: DestinationCategory::ControlsAndSymbols, value: 0x700000035, support: PathBSupport::Measured },
    FunctionDestination { id: "key.comma", label: "comma", category: DestinationCategory::ControlsAndSymbols, value: 0x700000036, support: PathBSupport::Measured },
    FunctionDestination { id: "key.period", label: "period", category: DestinationCategory::ControlsAndSymbols, value: 0x700000037, support: PathBSupport::Measured },
    FunctionDestination { id: "key.slash", label: "slash", category: DestinationCategory::ControlsAndSymbols, value: 0x700000038, support: PathBSupport::Measured },
    FunctionDestination { id: "key.non_us_backslash", label: "non_us_backslash", category: DestinationCategory::ControlsAndSymbols, value: 0x700000064, support: PathBSupport::Measured },
    FunctionDestination { id: "key.up_arrow", label: "up_arrow", category: DestinationCategory::ArrowKeys, value: 0x700000052, support: PathBSupport::Measured },
    FunctionDestination { id: "key.down_arrow", label: "down_arrow", category: DestinationCategory::ArrowKeys, value: 0x700000051, support: PathBSupport::Measured },
    FunctionDestination { id: "key.left_arrow", label: "left_arrow", category: DestinationCategory::ArrowKeys, value: 0x700000050, support: PathBSupport::Measured },
    FunctionDestination { id: "key.right_arrow", label: "right_arrow", category: DestinationCategory::ArrowKeys, value: 0x70000004F, support: PathBSupport::Measured },
    FunctionDestination { id: "key.page_up", label: "page_up", category: DestinationCategory::ArrowKeys, value: 0x70000004B, support: PathBSupport::Measured },
    FunctionDestination { id: "key.page_down", label: "page_down", category: DestinationCategory::ArrowKeys, value: 0x70000004E, support: PathBSupport::Measured },
    FunctionDestination { id: "key.home", label: "home", category: DestinationCategory::ArrowKeys, value: 0x70000004A, support: PathBSupport::Measured },
    FunctionDestination { id: "key.end", label: "end", category: DestinationCategory::ArrowKeys, value: 0x70000004D, support: PathBSupport::Measured },
    FunctionDestination { id: "key.a", label: "a", category: DestinationCategory::LetterKeys, value: 0x700000004, support: PathBSupport::Measured },
    FunctionDestination { id: "key.b", label: "b", category: DestinationCategory::LetterKeys, value: 0x700000005, support: PathBSupport::Measured },
    FunctionDestination { id: "key.c", label: "c", category: DestinationCategory::LetterKeys, value: 0x700000006, support: PathBSupport::Measured },
    FunctionDestination { id: "key.d", label: "d", category: DestinationCategory::LetterKeys, value: 0x700000007, support: PathBSupport::Measured },
    FunctionDestination { id: "key.e", label: "e", category: DestinationCategory::LetterKeys, value: 0x700000008, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f", label: "f", category: DestinationCategory::LetterKeys, value: 0x700000009, support: PathBSupport::Measured },
    FunctionDestination { id: "key.g", label: "g", category: DestinationCategory::LetterKeys, value: 0x70000000A, support: PathBSupport::Measured },
    FunctionDestination { id: "key.h", label: "h", category: DestinationCategory::LetterKeys, value: 0x70000000B, support: PathBSupport::Measured },
    FunctionDestination { id: "key.i", label: "i", category: DestinationCategory::LetterKeys, value: 0x70000000C, support: PathBSupport::Measured },
    FunctionDestination { id: "key.j", label: "j", category: DestinationCategory::LetterKeys, value: 0x70000000D, support: PathBSupport::Measured },
    FunctionDestination { id: "key.k", label: "k", category: DestinationCategory::LetterKeys, value: 0x70000000E, support: PathBSupport::Measured },
    FunctionDestination { id: "key.l", label: "l", category: DestinationCategory::LetterKeys, value: 0x70000000F, support: PathBSupport::Measured },
    FunctionDestination { id: "key.m", label: "m", category: DestinationCategory::LetterKeys, value: 0x700000010, support: PathBSupport::Measured },
    FunctionDestination { id: "key.n", label: "n", category: DestinationCategory::LetterKeys, value: 0x700000011, support: PathBSupport::Measured },
    FunctionDestination { id: "key.o", label: "o", category: DestinationCategory::LetterKeys, value: 0x700000012, support: PathBSupport::Measured },
    FunctionDestination { id: "key.p", label: "p", category: DestinationCategory::LetterKeys, value: 0x700000013, support: PathBSupport::Measured },
    FunctionDestination { id: "key.q", label: "q", category: DestinationCategory::LetterKeys, value: 0x700000014, support: PathBSupport::Measured },
    FunctionDestination { id: "key.r", label: "r", category: DestinationCategory::LetterKeys, value: 0x700000015, support: PathBSupport::Measured },
    FunctionDestination { id: "key.s", label: "s", category: DestinationCategory::LetterKeys, value: 0x700000016, support: PathBSupport::Measured },
    FunctionDestination { id: "key.t", label: "t", category: DestinationCategory::LetterKeys, value: 0x700000017, support: PathBSupport::Measured },
    FunctionDestination { id: "key.u", label: "u", category: DestinationCategory::LetterKeys, value: 0x700000018, support: PathBSupport::Measured },
    FunctionDestination { id: "key.v", label: "v", category: DestinationCategory::LetterKeys, value: 0x700000019, support: PathBSupport::Measured },
    FunctionDestination { id: "key.w", label: "w", category: DestinationCategory::LetterKeys, value: 0x70000001A, support: PathBSupport::Measured },
    FunctionDestination { id: "key.x", label: "x", category: DestinationCategory::LetterKeys, value: 0x70000001B, support: PathBSupport::Measured },
    FunctionDestination { id: "key.y", label: "y", category: DestinationCategory::LetterKeys, value: 0x70000001C, support: PathBSupport::Measured },
    FunctionDestination { id: "key.z", label: "z", category: DestinationCategory::LetterKeys, value: 0x70000001D, support: PathBSupport::Measured },
    FunctionDestination { id: "key.1", label: "1", category: DestinationCategory::NumberKeys, value: 0x70000001E, support: PathBSupport::Measured },
    FunctionDestination { id: "key.2", label: "2", category: DestinationCategory::NumberKeys, value: 0x70000001F, support: PathBSupport::Measured },
    FunctionDestination { id: "key.3", label: "3", category: DestinationCategory::NumberKeys, value: 0x700000020, support: PathBSupport::Measured },
    FunctionDestination { id: "key.4", label: "4", category: DestinationCategory::NumberKeys, value: 0x700000021, support: PathBSupport::Measured },
    FunctionDestination { id: "key.5", label: "5", category: DestinationCategory::NumberKeys, value: 0x700000022, support: PathBSupport::Measured },
    FunctionDestination { id: "key.6", label: "6", category: DestinationCategory::NumberKeys, value: 0x700000023, support: PathBSupport::Measured },
    FunctionDestination { id: "key.7", label: "7", category: DestinationCategory::NumberKeys, value: 0x700000024, support: PathBSupport::Measured },
    FunctionDestination { id: "key.8", label: "8", category: DestinationCategory::NumberKeys, value: 0x700000025, support: PathBSupport::Measured },
    FunctionDestination { id: "key.9", label: "9", category: DestinationCategory::NumberKeys, value: 0x700000026, support: PathBSupport::Measured },
    FunctionDestination { id: "key.0", label: "0", category: DestinationCategory::NumberKeys, value: 0x700000027, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f1", label: "f1", category: DestinationCategory::FunctionKeys, value: 0x70000003A, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f2", label: "f2", category: DestinationCategory::FunctionKeys, value: 0x70000003B, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f3", label: "f3", category: DestinationCategory::FunctionKeys, value: 0x70000003C, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f4", label: "f4", category: DestinationCategory::FunctionKeys, value: 0x70000003D, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f5", label: "f5", category: DestinationCategory::FunctionKeys, value: 0x70000003E, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f6", label: "f6", category: DestinationCategory::FunctionKeys, value: 0x70000003F, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f7", label: "f7", category: DestinationCategory::FunctionKeys, value: 0x700000040, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f8", label: "f8", category: DestinationCategory::FunctionKeys, value: 0x700000041, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f9", label: "f9", category: DestinationCategory::FunctionKeys, value: 0x700000042, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f10", label: "f10", category: DestinationCategory::FunctionKeys, value: 0x700000043, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f11", label: "f11", category: DestinationCategory::FunctionKeys, value: 0x700000044, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f12", label: "f12", category: DestinationCategory::FunctionKeys, value: 0x700000045, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f13", label: "f13", category: DestinationCategory::FunctionKeys, value: 0x700000068, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f14", label: "f14", category: DestinationCategory::FunctionKeys, value: 0x700000069, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f15", label: "f15", category: DestinationCategory::FunctionKeys, value: 0x70000006A, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f16", label: "f16", category: DestinationCategory::FunctionKeys, value: 0x70000006B, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f17", label: "f17", category: DestinationCategory::FunctionKeys, value: 0x70000006C, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f18", label: "f18", category: DestinationCategory::FunctionKeys, value: 0x70000006D, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f19", label: "f19", category: DestinationCategory::FunctionKeys, value: 0x70000006E, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f20", label: "f20", category: DestinationCategory::FunctionKeys, value: 0x70000006F, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f21", label: "f21", category: DestinationCategory::FunctionKeys, value: 0x700000070, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f22", label: "f22", category: DestinationCategory::FunctionKeys, value: 0x700000071, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f23", label: "f23", category: DestinationCategory::FunctionKeys, value: 0x700000072, support: PathBSupport::Measured },
    FunctionDestination { id: "key.f24", label: "f24", category: DestinationCategory::FunctionKeys, value: 0x700000073, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.display_brightness_decrement", label: "display_brightness_decrement", category: DestinationCategory::MediaControls, value: 0xC00000070, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.display_brightness_increment", label: "display_brightness_increment", category: DestinationCategory::MediaControls, value: 0xC0000006F, support: PathBSupport::Measured },
    FunctionDestination { id: "appleKeyboard.mission_control", label: "mission_control", category: DestinationCategory::MediaControls, value: 0xFF0100000010, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleKeyboard.spotlight", label: "spotlight", category: DestinationCategory::MediaControls, value: 0xFF0100000001, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "consumer.dictation", label: "dictation", category: DestinationCategory::MediaControls, value: 0xC000000CF, support: PathBSupport::Measured },
    FunctionDestination { id: "appleKeyboard.launchpad", label: "launchpad", category: DestinationCategory::MediaControls, value: 0xFF0100000004, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleKeyboard.dashboard", label: "dashboard", category: DestinationCategory::MediaControls, value: 0xFF0100000002, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleTopCase.illumination_down", label: "illumination_down", category: DestinationCategory::MediaControls, value: 0xFF00000009, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleTopCase.illumination_up", label: "illumination_up", category: DestinationCategory::MediaControls, value: 0xFF00000008, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "consumer.rewind", label: "rewind", category: DestinationCategory::MediaControls, value: 0xC000000B4, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.play_or_pause", label: "play_or_pause", category: DestinationCategory::MediaControls, value: 0xC000000CD, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.fast_forward", label: "fast_forward", category: DestinationCategory::MediaControls, value: 0xC000000B3, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.mute", label: "consumer.mute", category: DestinationCategory::MediaControls, value: 0xC000000E2, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.volume_decrement", label: "volume_decrement", category: DestinationCategory::MediaControls, value: 0xC000000EA, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.volume_increment", label: "volume_increment", category: DestinationCategory::MediaControls, value: 0xC000000E9, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.menu", label: "consumer.menu", category: DestinationCategory::MediaControls, value: 0xC00000040, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_terminal_lock_or_screensaver", label: "al_terminal_lock_or_screensaver", category: DestinationCategory::MediaControls, value: 0xC0000019E, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.eject", label: "eject", category: DestinationCategory::MediaControls, value: 0xC000000B8, support: PathBSupport::Measured },
    FunctionDestination { id: "appleKeyboard.brightness_down", label: "appleKeyboard.brightness_down", category: DestinationCategory::MediaControls, value: 0xFF0100000021, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleKeyboard.brightness_up", label: "appleKeyboard.brightness_up", category: DestinationCategory::MediaControls, value: 0xFF0100000020, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleTopCase.brightness_down", label: "appleTopCase.brightness_down", category: DestinationCategory::MediaControls, value: 0xFF00000005, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleTopCase.brightness_up", label: "appleTopCase.brightness_up", category: DestinationCategory::MediaControls, value: 0xFF00000004, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "consumer.scan_previous_track", label: "scan_previous_track", category: DestinationCategory::MediaControls, value: 0xC000000B6, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.scan_next_track", label: "scan_next_track", category: DestinationCategory::MediaControls, value: 0xC000000B5, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.stop", label: "consumer.stop", category: DestinationCategory::MediaControls, value: 0xC000000B7, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.bass_boost", label: "bass_boost", category: DestinationCategory::MediaControls, value: 0xC000000E5, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.loudness", label: "loudness", category: DestinationCategory::MediaControls, value: 0xC000000E7, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.bass_increment", label: "bass_increment", category: DestinationCategory::MediaControls, value: 0xC00000152, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.bass_decrement", label: "bass_decrement", category: DestinationCategory::MediaControls, value: 0xC00000153, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_num_lock", label: "keypad_num_lock", category: DestinationCategory::KeypadKeys, value: 0x700000053, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_slash", label: "keypad_slash", category: DestinationCategory::KeypadKeys, value: 0x700000054, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_asterisk", label: "keypad_asterisk", category: DestinationCategory::KeypadKeys, value: 0x700000055, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_hyphen", label: "keypad_hyphen", category: DestinationCategory::KeypadKeys, value: 0x700000056, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_plus", label: "keypad_plus", category: DestinationCategory::KeypadKeys, value: 0x700000057, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_enter", label: "keypad_enter", category: DestinationCategory::KeypadKeys, value: 0x700000058, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_1", label: "keypad_1", category: DestinationCategory::KeypadKeys, value: 0x700000059, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_2", label: "keypad_2", category: DestinationCategory::KeypadKeys, value: 0x70000005A, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_3", label: "keypad_3", category: DestinationCategory::KeypadKeys, value: 0x70000005B, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_4", label: "keypad_4", category: DestinationCategory::KeypadKeys, value: 0x70000005C, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_5", label: "keypad_5", category: DestinationCategory::KeypadKeys, value: 0x70000005D, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_6", label: "keypad_6", category: DestinationCategory::KeypadKeys, value: 0x70000005E, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_7", label: "keypad_7", category: DestinationCategory::KeypadKeys, value: 0x70000005F, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_8", label: "keypad_8", category: DestinationCategory::KeypadKeys, value: 0x700000060, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_9", label: "keypad_9", category: DestinationCategory::KeypadKeys, value: 0x700000061, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_0", label: "keypad_0", category: DestinationCategory::KeypadKeys, value: 0x700000062, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_period", label: "keypad_period", category: DestinationCategory::KeypadKeys, value: 0x700000063, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_equal_sign", label: "keypad_equal_sign", category: DestinationCategory::KeypadKeys, value: 0x700000067, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_comma", label: "keypad_comma", category: DestinationCategory::KeypadKeys, value: 0x700000085, support: PathBSupport::Measured },
    FunctionDestination { id: "key.print_screen", label: "print_screen", category: DestinationCategory::PcKeyboardKeys, value: 0x700000046, support: PathBSupport::Measured },
    FunctionDestination { id: "key.scroll_lock", label: "scroll_lock", category: DestinationCategory::PcKeyboardKeys, value: 0x700000047, support: PathBSupport::Measured },
    FunctionDestination { id: "key.pause", label: "pause", category: DestinationCategory::PcKeyboardKeys, value: 0x700000048, support: PathBSupport::Measured },
    FunctionDestination { id: "key.insert", label: "insert", category: DestinationCategory::PcKeyboardKeys, value: 0x700000049, support: PathBSupport::Measured },
    FunctionDestination { id: "key.application", label: "application", category: DestinationCategory::PcKeyboardKeys, value: 0x700000065, support: PathBSupport::Measured },
    FunctionDestination { id: "key.help", label: "help", category: DestinationCategory::PcKeyboardKeys, value: 0x700000075, support: PathBSupport::Measured },
    FunctionDestination { id: "key.power", label: "power", category: DestinationCategory::PcKeyboardKeys, value: 0x700000066, support: PathBSupport::Measured },
    FunctionDestination { id: "key.execute", label: "execute", category: DestinationCategory::PcKeyboardKeys, value: 0x700000074, support: PathBSupport::Measured },
    FunctionDestination { id: "key.menu", label: "key.menu", category: DestinationCategory::PcKeyboardKeys, value: 0x700000076, support: PathBSupport::Measured },
    FunctionDestination { id: "key.select", label: "select", category: DestinationCategory::PcKeyboardKeys, value: 0x700000077, support: PathBSupport::Measured },
    FunctionDestination { id: "key.stop", label: "key.stop", category: DestinationCategory::PcKeyboardKeys, value: 0x700000078, support: PathBSupport::Measured },
    FunctionDestination { id: "key.again", label: "again", category: DestinationCategory::PcKeyboardKeys, value: 0x700000079, support: PathBSupport::Measured },
    FunctionDestination { id: "key.undo", label: "undo", category: DestinationCategory::PcKeyboardKeys, value: 0x70000007A, support: PathBSupport::Measured },
    FunctionDestination { id: "key.cut", label: "cut", category: DestinationCategory::PcKeyboardKeys, value: 0x70000007B, support: PathBSupport::Measured },
    FunctionDestination { id: "key.copy", label: "copy", category: DestinationCategory::PcKeyboardKeys, value: 0x70000007C, support: PathBSupport::Measured },
    FunctionDestination { id: "key.paste", label: "paste", category: DestinationCategory::PcKeyboardKeys, value: 0x70000007D, support: PathBSupport::Measured },
    FunctionDestination { id: "key.find", label: "find", category: DestinationCategory::PcKeyboardKeys, value: 0x70000007E, support: PathBSupport::Measured },
    FunctionDestination { id: "key.international1", label: "international1", category: DestinationCategory::InternationalKeys, value: 0x700000087, support: PathBSupport::Measured },
    FunctionDestination { id: "key.international2", label: "international2", category: DestinationCategory::InternationalKeys, value: 0x700000088, support: PathBSupport::Measured },
    FunctionDestination { id: "key.international3", label: "international3", category: DestinationCategory::InternationalKeys, value: 0x700000089, support: PathBSupport::Measured },
    FunctionDestination { id: "key.international4", label: "international4", category: DestinationCategory::InternationalKeys, value: 0x70000008A, support: PathBSupport::Measured },
    FunctionDestination { id: "key.international5", label: "international5", category: DestinationCategory::InternationalKeys, value: 0x70000008B, support: PathBSupport::Measured },
    FunctionDestination { id: "key.international6", label: "international6", category: DestinationCategory::InternationalKeys, value: 0x70000008C, support: PathBSupport::Measured },
    FunctionDestination { id: "key.international7", label: "international7", category: DestinationCategory::InternationalKeys, value: 0x70000008D, support: PathBSupport::Measured },
    FunctionDestination { id: "key.international8", label: "international8", category: DestinationCategory::InternationalKeys, value: 0x70000008E, support: PathBSupport::Measured },
    FunctionDestination { id: "key.international9", label: "international9", category: DestinationCategory::InternationalKeys, value: 0x70000008F, support: PathBSupport::Measured },
    FunctionDestination { id: "key.lang1", label: "lang1", category: DestinationCategory::InternationalKeys, value: 0x700000090, support: PathBSupport::Measured },
    FunctionDestination { id: "key.lang2", label: "lang2", category: DestinationCategory::InternationalKeys, value: 0x700000091, support: PathBSupport::Measured },
    FunctionDestination { id: "key.lang3", label: "lang3", category: DestinationCategory::InternationalKeys, value: 0x700000092, support: PathBSupport::Measured },
    FunctionDestination { id: "key.lang4", label: "lang4", category: DestinationCategory::InternationalKeys, value: 0x700000093, support: PathBSupport::Measured },
    FunctionDestination { id: "key.lang5", label: "lang5", category: DestinationCategory::InternationalKeys, value: 0x700000094, support: PathBSupport::Measured },
    FunctionDestination { id: "key.lang6", label: "lang6", category: DestinationCategory::InternationalKeys, value: 0x700000095, support: PathBSupport::Measured },
    FunctionDestination { id: "key.lang7", label: "lang7", category: DestinationCategory::InternationalKeys, value: 0x700000096, support: PathBSupport::Measured },
    FunctionDestination { id: "key.lang8", label: "lang8", category: DestinationCategory::InternationalKeys, value: 0x700000097, support: PathBSupport::Measured },
    FunctionDestination { id: "key.lang9", label: "lang9", category: DestinationCategory::InternationalKeys, value: 0x700000098, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_consumer_control_configuration", label: "al_consumer_control_configuration", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000183, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_word_processor", label: "al_word_processor", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000184, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_text_editor", label: "al_text_editor", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000185, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_spreadsheet", label: "al_spreadsheet", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000186, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_graphics_editor", label: "al_graphics_editor", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000187, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_presentation_app", label: "al_presentation_app", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000188, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_database_app", label: "al_database_app", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000189, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_email_reader", label: "al_email_reader", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000018A, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_newsreader", label: "al_newsreader", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000018B, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_voicemail", label: "al_voicemail", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000018C, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_contacts_or_address_book", label: "al_contacts_or_address_book", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000018D, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_Calendar_Or_Schedule", label: "al_Calendar_Or_Schedule", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000018E, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_task_or_project_manager", label: "al_task_or_project_manager", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000018F, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_log_or_journal_or_timecard", label: "al_log_or_journal_or_timecard", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000190, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_checkbook_or_finance", label: "al_checkbook_or_finance", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000191, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_calculator", label: "al_calculator", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000192, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_a_or_v_capture_or_playback", label: "al_a_or_v_capture_or_playback", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000193, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_local_machine_browser", label: "al_local_machine_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000194, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_lan_or_wan_browser", label: "al_lan_or_wan_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000195, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_internet_browser", label: "al_internet_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000196, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_remote_networking_or_isp_connect", label: "al_remote_networking_or_isp_connect", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000197, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_network_conference", label: "al_network_conference", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000198, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_network_chat", label: "al_network_chat", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC00000199, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_telephony_or_dialer", label: "al_telephony_or_dialer", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000019A, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_logon", label: "al_logon", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000019B, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_logoff", label: "al_logoff", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000019C, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_logon_or_logoff", label: "al_logon_or_logoff", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000019D, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_control_panel", label: "al_control_panel", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC0000019F, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_command_line_processor_or_run", label: "al_command_line_processor_or_run", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001A0, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_process_or_task_manager", label: "al_process_or_task_manager", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001A1, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_select_task_or_application", label: "al_select_task_or_application", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001A2, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_next_task_or_application", label: "al_next_task_or_application", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001A3, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_previous_task_or_application", label: "al_previous_task_or_application", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001A4, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_preemptive_halt_task_or_application", label: "al_preemptive_halt_task_or_application", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001A5, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_integrated_help_center", label: "al_integrated_help_center", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001A6, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_documents", label: "al_documents", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001A7, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_thesaurus", label: "al_thesaurus", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001A8, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_dictionary", label: "al_dictionary", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001A9, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_desktop", label: "al_desktop", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001AA, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_spell_check", label: "al_spell_check", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001AB, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_grammer_check", label: "al_grammer_check", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001AC, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_wireless_status", label: "al_wireless_status", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001AD, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_keyboard_layout", label: "al_keyboard_layout", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001AE, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_virus_protection", label: "al_virus_protection", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001AF, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_encryption", label: "al_encryption", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001B0, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_screen_saver", label: "al_screen_saver", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001B1, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_alarms", label: "al_alarms", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001B2, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_clock", label: "al_clock", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001B3, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_file_browser", label: "al_file_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001B4, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_power_status", label: "al_power_status", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001B5, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_image_browser", label: "al_image_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001B6, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_audio_browser", label: "al_audio_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001B7, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_movie_browser", label: "al_movie_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001B8, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_digital_rights_manager", label: "al_digital_rights_manager", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001B9, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_digital_wallet", label: "al_digital_wallet", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001BA, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_instant_messaging", label: "al_instant_messaging", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001BC, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_oem_feature_browser", label: "al_oem_feature_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001BD, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_oem_help", label: "al_oem_help", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001BE, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_online_community", label: "al_online_community", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001BF, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_entertainment_content_browser", label: "al_entertainment_content_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001C0, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_online_shopping_browswer", label: "al_online_shopping_browswer", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001C1, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_smart_card_information_or_help", label: "al_smart_card_information_or_help", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001C2, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_market_monitor_or_finance_browser", label: "al_market_monitor_or_finance_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001C3, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_customized_corporate_news_browser", label: "al_customized_corporate_news_browser", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001C4, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_online_activity_browswer", label: "al_online_activity_browswer", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001C5, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_research_or_search_browswer", label: "al_research_or_search_browswer", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001C6, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_audio_player", label: "al_audio_player", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001C7, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_message_status", label: "al_message_status", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001C8, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_contact_sync", label: "al_contact_sync", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001C9, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_navigation", label: "al_navigation", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001CA, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.al_contextaware_desktop_assistant", label: "al_contextaware_desktop_assistant", category: DestinationCategory::ApplicationLaunchKeys, value: 0xC000001CB, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_search", label: "ac_search", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC00000221, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_home", label: "ac_home", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC00000223, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_back", label: "ac_back", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC00000224, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_forward", label: "ac_forward", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC00000225, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_refresh", label: "ac_refresh", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC00000227, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_bookmarks", label: "ac_bookmarks", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC0000022A, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_zoom_in", label: "ac_zoom_in", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC0000022D, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_zoom_out", label: "ac_zoom_out", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC0000022E, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_keyboard_layout_select", label: "ac_keyboard_layout_select", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC0000029D, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_desktop_show_all_windows", label: "ac_desktop_show_all_windows", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC0000029F, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_soft_key_left", label: "ac_soft_key_left", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC000002A0, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.ac_desktop_show_all_applications", label: "ac_desktop_show_all_applications", category: DestinationCategory::GuiApplicationControlKeys, value: 0xC000002A2, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.menu_pick", label: "menu_pick", category: DestinationCategory::RemoteControlButtons, value: 0xC00000041, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.menu_up", label: "menu_up", category: DestinationCategory::RemoteControlButtons, value: 0xC00000042, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.menu_down", label: "menu_down", category: DestinationCategory::RemoteControlButtons, value: 0xC00000043, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.menu_left", label: "menu_left", category: DestinationCategory::RemoteControlButtons, value: 0xC00000044, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.menu_right", label: "menu_right", category: DestinationCategory::RemoteControlButtons, value: 0xC00000045, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.menu_escape", label: "menu_escape", category: DestinationCategory::RemoteControlButtons, value: 0xC00000046, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.menu_value_increase", label: "menu_value_increase", category: DestinationCategory::RemoteControlButtons, value: 0xC00000047, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.menu_value_decrease", label: "menu_value_decrease", category: DestinationCategory::RemoteControlButtons, value: 0xC00000048, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.data_on_screen", label: "data_on_screen", category: DestinationCategory::RemoteControlButtons, value: 0xC00000060, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.closed_caption", label: "closed_caption", category: DestinationCategory::RemoteControlButtons, value: 0xC00000061, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.closed_caption_select", label: "closed_caption_select", category: DestinationCategory::RemoteControlButtons, value: 0xC00000062, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.vcr_or_tv", label: "vcr_or_tv", category: DestinationCategory::RemoteControlButtons, value: 0xC00000063, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.broadcast_mode", label: "broadcast_mode", category: DestinationCategory::RemoteControlButtons, value: 0xC00000064, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.snapshot", label: "snapshot", category: DestinationCategory::RemoteControlButtons, value: 0xC00000065, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.still", label: "still", category: DestinationCategory::RemoteControlButtons, value: 0xC00000066, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.picture_in_picture_toggle", label: "picture_in_picture_toggle", category: DestinationCategory::RemoteControlButtons, value: 0xC00000067, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.picture_in_picture_swap", label: "picture_in_picture_swap", category: DestinationCategory::RemoteControlButtons, value: 0xC00000068, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.red_menu_button", label: "red_menu_button", category: DestinationCategory::RemoteControlButtons, value: 0xC00000069, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.green_menu_button", label: "green_menu_button", category: DestinationCategory::RemoteControlButtons, value: 0xC0000006A, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.blue_menu_button", label: "blue_menu_button", category: DestinationCategory::RemoteControlButtons, value: 0xC0000006B, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.yellow_menu_button", label: "yellow_menu_button", category: DestinationCategory::RemoteControlButtons, value: 0xC0000006C, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.aspect", label: "aspect", category: DestinationCategory::RemoteControlButtons, value: 0xC0000006D, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.three_dimensional_mode_select", label: "three_dimensional_mode_select", category: DestinationCategory::RemoteControlButtons, value: 0xC0000006E, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.microphone", label: "microphone", category: DestinationCategory::RemoteControlButtons, value: 0xC00000004, support: PathBSupport::Measured },
    FunctionDestination { id: "consumer.selection", label: "selection", category: DestinationCategory::RemoteControlButtons, value: 0xC00000080, support: PathBSupport::Measured },
    FunctionDestination { id: "key.keypad_equal_sign_as400", label: "keypad_equal_sign_as400", category: DestinationCategory::Others, value: 0x700000086, support: PathBSupport::Measured },
    FunctionDestination { id: "key.locking_caps_lock", label: "locking_caps_lock", category: DestinationCategory::Others, value: 0x700000082, support: PathBSupport::Measured },
    FunctionDestination { id: "key.locking_num_lock", label: "locking_num_lock", category: DestinationCategory::Others, value: 0x700000083, support: PathBSupport::Measured },
    FunctionDestination { id: "key.locking_scroll_lock", label: "locking_scroll_lock", category: DestinationCategory::Others, value: 0x700000084, support: PathBSupport::Measured },
    FunctionDestination { id: "key.alternate_erase", label: "alternate_erase", category: DestinationCategory::Others, value: 0x700000099, support: PathBSupport::Measured },
    FunctionDestination { id: "key.sys_req_or_attention", label: "sys_req_or_attention", category: DestinationCategory::Others, value: 0x70000009A, support: PathBSupport::Measured },
    FunctionDestination { id: "key.cancel", label: "cancel", category: DestinationCategory::Others, value: 0x70000009B, support: PathBSupport::Measured },
    FunctionDestination { id: "key.clear", label: "clear", category: DestinationCategory::Others, value: 0x70000009C, support: PathBSupport::Measured },
    FunctionDestination { id: "key.prior", label: "prior", category: DestinationCategory::Others, value: 0x70000009D, support: PathBSupport::Measured },
    FunctionDestination { id: "key.return", label: "return", category: DestinationCategory::Others, value: 0x70000009E, support: PathBSupport::Measured },
    FunctionDestination { id: "key.separator", label: "separator", category: DestinationCategory::Others, value: 0x70000009F, support: PathBSupport::Measured },
    FunctionDestination { id: "key.out", label: "out", category: DestinationCategory::Others, value: 0x7000000A0, support: PathBSupport::Measured },
    FunctionDestination { id: "key.oper", label: "oper", category: DestinationCategory::Others, value: 0x7000000A1, support: PathBSupport::Measured },
    FunctionDestination { id: "key.clear_or_again", label: "clear_or_again", category: DestinationCategory::Others, value: 0x7000000A2, support: PathBSupport::Measured },
    FunctionDestination { id: "key.cr_sel_or_props", label: "cr_sel_or_props", category: DestinationCategory::Others, value: 0x7000000A3, support: PathBSupport::Measured },
    FunctionDestination { id: "key.ex_sel", label: "ex_sel", category: DestinationCategory::Others, value: 0x7000000A4, support: PathBSupport::Measured },
    FunctionDestination { id: "key.volume_down", label: "volume_down", category: DestinationCategory::Others, value: 0x700000081, support: PathBSupport::Measured },
    FunctionDestination { id: "key.volume_up", label: "volume_up", category: DestinationCategory::Others, value: 0x700000080, support: PathBSupport::Measured },
    FunctionDestination { id: "key.mute", label: "key.mute", category: DestinationCategory::Others, value: 0x70000007F, support: PathBSupport::Measured },
    FunctionDestination { id: "appleKeyboard.function", label: "function", category: DestinationCategory::Others, value: 0xFF0100000003, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleKeyboard.expose_desktop", label: "expose_desktop", category: DestinationCategory::Others, value: 0xFF0100000011, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleKeyboard.language", label: "language", category: DestinationCategory::Others, value: 0xFF0100000030, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleTopCase.video_mirror", label: "video_mirror", category: DestinationCategory::Others, value: 0xFF00000006, support: PathBSupport::VendorPageUnverified },
    FunctionDestination { id: "appleTopCase.illumination_toggle", label: "illumination_toggle", category: DestinationCategory::Others, value: 0xFF00000007, support: PathBSupport::VendorPageUnverified },
];

// ── 옛 저장값 호환(F-17 → 이슈 #31) ─────────────────────────────────────────

/// PR #29 가 출하한 [`super::SystemFunction`] variant 이름으로 저장돼 있는 값을 새
/// 목적지 id 로 옮긴다. ⭐ 저장 파일을 다시 쓰지는 않는다 — 읽을 때만 옮긴다
/// (F-15 "한 번이라도 건드린 항목만 저장"). 사용자가 그 F-키를 다시 고르는 순간
/// 새 id 로 덮어써진다.
///
/// `DoNotDisturb` 만 `None` 이다 — Generic Desktop page(0x01)라 경로 B 로 표현할
/// 수 없어 이 카탈로그에 없다(모듈 문서의 "무엇을 뺐는가" 표). 예전에도
/// `SystemFunction::hid_usage()` 가 `None` 이라 아무 매핑도 만들지 않았으므로,
/// 동작은 그대로다.
pub fn migrate_legacy_system_function(variant_name: &str) -> Option<&'static str> {
    let id = match variant_name {
        "DisplayBrightnessDown" => "consumer.display_brightness_decrement",
        "DisplayBrightnessUp" => "consumer.display_brightness_increment",
        "MissionControl" => "appleKeyboard.mission_control",
        "Spotlight" => "appleKeyboard.spotlight",
        "Dictation" => "consumer.dictation",
        "Rewind" => "consumer.rewind",
        "PlayPause" => "consumer.play_or_pause",
        "FastForward" => "consumer.fast_forward",
        "Mute" => "consumer.mute",
        "VolumeDown" => "consumer.volume_decrement",
        "VolumeUp" => "consumer.volume_increment",
        // ⛔ 경로 B 로 표현할 수 없다 — 예전에도 매핑을 만들지 않았다.
        "DoNotDisturb" => return None,
        _ => return None,
    };
    Some(id)
}

/// 저장된 문자열 하나를 목적지로 푼다 — 새 id 를 먼저 보고, 없으면 옛
/// `SystemFunction` variant 이름으로 한 번 더 시도한다.
pub fn resolve_stored(value: &str) -> Option<&'static FunctionDestination> {
    find(value).or_else(|| migrate_legacy_system_function(value).and_then(find))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// D-17-4 규칙 3 이 허용하는 page 밖의 값이 카탈로그에 있으면 안 된다 —
    /// `validate()` 가 쓰기 직전에 거부해 버려, 고를 수는 있는데 아무 일도 일어나지
    /// 않는 항목이 된다(이슈 #31 의 명시적 요구).
    #[test]
    fn every_destination_uses_an_allowed_usage_page() {
        for d in all() {
            assert!(
                matches!(d.page(), 0x07 | 0x0C | 0xFF | 0xFF01),
                "{}: page={:#x} 는 D-17-4 허용 집합 밖이다",
                d.id,
                d.page()
            );
            assert!(
                d.usage() <= 0xFFFF,
                "{}: usage={:#x} 가 범위를 벗어났다",
                d.id,
                d.usage()
            );
        }
    }

    /// id 와 라벨은 둘 다 카탈로그 안에서 유일해야 한다. id 가 겹치면 저장값이
    /// 어느 목적지인지 정해지지 않고, 라벨이 겹치면 팝업에서 같은 이름이 두 번
    /// 보여 사용자가 무엇이 다른지 알 수 없다.
    #[test]
    fn ids_and_labels_are_unique() {
        let mut ids = HashSet::new();
        let mut labels = HashSet::new();
        for d in all() {
            assert!(ids.insert(d.id), "id 중복: {}", d.id);
            assert!(labels.insert(d.label), "라벨 중복: {} ({})", d.label, d.id);
        }
    }

    /// 같은 `value` 를 가리키는 항목이 둘 있으면 안 된다 — 별칭을 값 기준으로
    /// 중복 제거한 결정(모듈 문서)이 무너졌다는 뜻이다.
    #[test]
    fn values_are_unique() {
        let mut seen = HashSet::new();
        for d in all() {
            assert!(seen.insert(d.value), "값 중복: {:#x} ({})", d.value, d.id);
        }
    }

    /// 카테고리는 팝업 순서대로 **묶여** 있어야 한다 — 흩어져 있으면
    /// `<optgroup>` 을 순서대로 만들 수 없다.
    #[test]
    fn entries_are_grouped_by_category_in_display_order() {
        let mut order: Vec<DestinationCategory> = Vec::new();
        for d in all() {
            if order.last() != Some(&d.category) {
                assert!(
                    !order.contains(&d.category),
                    "카테고리 {:?} 가 두 군데로 흩어져 있다({})",
                    d.category,
                    d.id
                );
                order.push(d.category);
            }
        }
        assert_eq!(
            order,
            DestinationCategory::all(),
            "카테고리 순서가 all() 과 다르다"
        );
    }

    /// ⭐ 값 회귀 방지 — 다른 경로로 이미 실측·확정된 값과 일치하는지 본다.
    /// 기대값을 카탈로그에서 가져오지 않고 리터럴로 적는다(PR #17 함정 회피 원칙,
    /// `keycode.rs` 의 `hid_usage` 테스트와 같은 방식).
    #[test]
    fn known_values_match_independently_confirmed_literals() {
        let expect: &[(&str, u64)] = &[
            // 스파이크 S-8 — 키 입력으로 "볼륨이 실제로 올라간다"를 확인한 값.
            ("consumer.volume_increment", 0xC000000E9),
            // `SourceKey::Globe` 로 이미 출하된 값(PR #23 사용자 확인).
            ("appleTopCase.keyboard_fn", 0xFF00000003),
            // `SourceKey::hid_usage()` 와 같은 Keyboard page 값들.
            ("key.caps_lock", 0x700000039),
            ("key.f1", 0x70000003A),
            ("key.f12", 0x700000045),
            ("key.left_command", 0x7000000E3),
            ("key.left_option", 0x7000000E2),
            ("key.a", 0x700000004),
        ];
        for (id, want) in expect {
            let d = find(id).unwrap_or_else(|| panic!("카탈로그에 {id} 가 없다"));
            assert_eq!(d.value, *want, "{id} 의 값이 독립 확인값과 다르다");
        }
    }

    /// 옛 `SystemFunction` 12종이 전부 처리된다 — 11종은 같은 값의 새 목적지로,
    /// `DoNotDisturb` 만 `None`.
    #[test]
    fn legacy_system_functions_migrate_to_the_same_values() {
        use crate::perdevice::SystemFunction;

        for f in SystemFunction::all() {
            let variant = match serde_json::to_value(f).unwrap() {
                serde_json::Value::String(s) => s,
                other => panic!("SystemFunction 직렬화가 문자열이 아니다: {other:?}"),
            };
            match (f.hid_usage(), resolve_stored(&variant)) {
                // 예전에 값이 있던 것은 **같은 값**으로 옮겨져야 한다.
                (Some(old), Some(d)) => {
                    assert_eq!(d.value, old, "{variant} 의 값이 마이그레이션에서 달라졌다")
                }
                // 예전에 값이 없던 4종 중 3종은 이제 값이 생겼다(카탈로그가 더 넓다).
                (None, Some(_)) => {
                    assert!(
                        matches!(
                            variant.as_str(),
                            "MissionControl" | "Spotlight" | "Dictation"
                        ),
                        "예상 밖으로 값이 생긴 항목: {variant}"
                    )
                }
                // 경로 B 로 표현 불가 — 예전에도 매핑을 만들지 않았다.
                (None, None) => assert_eq!(variant, "DoNotDisturb"),
                (Some(_), None) => panic!("{variant} 가 마이그레이션에서 사라졌다"),
            }
        }
    }

    /// 모르는 문자열은 조용히 `None` — 손상된 설정 하나가 앱을 죽이지 않는다.
    #[test]
    fn unknown_stored_value_resolves_to_none() {
        assert!(resolve_stored("nope.not_a_key").is_none());
        assert!(resolve_stored("").is_none());
    }

    /// 미검증 등급이 정확히 어디에 붙어 있는지 못 박는다 — 벤더 page(0xFF·0xFF01)
    /// 전부와 `disable` 하나뿐이고, 나머지는 실측된 page 다.
    #[test]
    fn unverified_support_is_exactly_the_vendor_pages_and_disable() {
        for d in all() {
            let want = match (d.id, d.page()) {
                ("disable", _) => PathBSupport::DisableUnverified,
                (_, 0xFF | 0xFF01) => PathBSupport::VendorPageUnverified,
                _ => PathBSupport::Measured,
            };
            assert_eq!(d.support, want, "{} 의 근거 등급이 어긋났다", d.id);
        }
        assert!(!find("disable").unwrap().support.is_verified());
        assert!(find("consumer.volume_increment")
            .unwrap()
            .support
            .is_verified());
    }
}
