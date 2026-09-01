# 이슈 #78 조사 — Bartender 환경에서 트레이 아이콘이 반복적으로 사라졌다 나타난다

> **성격**: 조사·계측 보고서. 구현 위임(단계 1) 산출물이다 — 코드 경로 **확정 사실**, `tray-icon` 0.24.2 macOS 구현 **실측**, 가설 판정까지를 기록한다. 원인 자체는 **Bartender 가 설치된 실기기에서만 확정 가능**하므로, 이 문서의 결론은 "실기기 관찰로 판정할 재료가 갖춰졌다"까지다.
>
> 관할 계획: [`docs/plan/issue-78-tray-icon-bartender.md`](../plan/issue-78-tray-icon-bartender.md) · 연관 명세: [`docs/spec/menu-bar-and-lifecycle.md`](../spec/menu-bar-and-lifecycle.md) §4·§5
> 조사일: 2026-09-02 · 대상: 이슈 #78

---

## 1. 한눈에

| 질문 | 판정 |
| :--- | :--- |
| 우리 코드가 아이콘을 **재생성**하는가 | **아니다.** `TrayIcon`(NSStatusItem)은 부팅 시 한 번만 생성되고 이후 재생성 경로가 없다 |
| 우리 코드가 런타임에 `set_icon` 을 호출하는가 | **아니다.** 호출 지점 0건 |
| 기본 설정에서 우리 코드가 `set_visible` 을 호출하는가 | **아니다.** `hideMenuBarIcon == true` 일 때(부팅)·체크박스 토글 시에만 |
| `set_menu` 교체가 아이콘 리페인트를 유발하는가 | **아니다.** NSStatusItem 을 유지한 채 메뉴만 교체한다(`tray-icon` 0.24.2 실측) |
| 그렇다면 깜빡임의 원인 후보는 | **Bartender 의 숨김·복원 상호작용(H1/H4)** 가 우세 — 실기기로만 판정 가능 |

⭐ 두 줄 요약: 기본 상황에서 **우리 코드가 아이콘의 가시성·이미지에 개입하는 경로가 존재하지 않는다.** 깜빡임을 만드는 쪽은 아이콘을 숨겼다 복원하는 Bartender 일 가능성이 가장 높으며, 이는 Bartender 내부 동작이라 1차 출처 확인이 불가능해(`(추정)`) **Bartender 실기기에서의 관찰로만 확정**할 수 있다. 추정 수정은 넣지 않는다 — 근거가 없다(AGENTS.md §3 "확인하지 못한 동작을 지어내지 않는다" 직접 적용).

---

## 2. 코드 경로 확정 사실 (원본: `apps/ultrakey-app/src/main.rs`)

| 사실 | 근거(파일:줄) | 비고 |
| :--- | :--- | :--- |
| `setup_tray` 는 부팅 시 **한 번만** 불린다 | `main.rs:4200`(`setup()` 안) → `main.rs:5085` | 재생성 경로 없음 |
| `TrayIcon`(NSStatusItem) 생성은 `TrayIconBuilder::build()` 1회 | `main.rs:5113-5132` | 아이콘은 `.icon(icon)` + `.icon_as_template(true)` 로 빌더에서 한 번만 설정 |
| `set_icon` 런타임 호출은 **0건** | `main.rs` 전체 grep | 빌더 설정 이후 런타임 교체·재설정 없음 |
| `set_visible` 호출은 2곳뿐 | 부팅: `main.rs:5134-5139`(`hide_menu_bar_icon == true` 일 때만) · 토글: `main.rs:5712-5724`(`general_set_hide_menu_bar_icon` 커맨드) | **기본 설정(☐)에서는 부팅 경로조차 타지 않는다** |
| `set_menu` 호출은 `apply_tray_menu_for_permission` 한 곳 | `main.rs:5219-5251` | 호출 주체 2종: (a) 권한 전이(`main.rs:4312·4319`, 전용 `reason="permission"`), (b) 언어 변경 `rebuild_tray_menu`(`main.rs:5257-5322` → `main.rs:5311`, 전용 `reason="language"`) |
| `refresh_ignore_menu_item` 은 NSMenuItem 수준만 건드린다 | `main.rs:5338-5362` — `set_text`/`set_enabled`/`set_checked` | NSStatusItem 의 아이콘·가시성과 무관 |
| `show_menu_on_left_click(true)` | `main.rs:5122` | 클릭 시 메뉴를 여는 동작만 정함 — 아이콘 가시성과 무관 |
| 계측 — 주기적 NSStatusItem 스냅샷 폴러 | `main.rs:5164`(`spawn_tray_snapshot_poller`), 기동: `main.rs:4205` | `debug` 레벨 1초 간격. `ULTRAKEY_LOG=debug` 일 때만 기록 |

---

## 3. `tray-icon` 0.24.2 macOS 구현 실측

버전: `Cargo.lock` `tray-icon 0.24.2`(채택 경로: Tauri v2 내장 — `apps/ultrakey-app/Cargo.toml:45-53`). 판독 대상: `src/platform_impl/macos/mod.rs`.

### 3-1. `set_menu` — NSStatusItem 을 유지한 채 메뉴만 교체

```rust
pub fn set_menu(&mut self, menu: Option<Box<dyn menu::ContextMenu>>) {
    if let (Some(ns_status_item), Some(tray_target)) = ... {
        unsafe {
            ns_status_item.setMenu(menu.as_deref());        // NSStatusItem.setMenu: 메뉴 포인터만 바꾼다
            ...
        }
    }
    self.attrs.menu = menu;
}
```

- `mod.rs:125-142`. `ns_status_item` 은 `Option` 유지 — **아이콘(`button.image`)·가시성·NSStatusItem 생성을 전혀 건드리지 않는다.**
- `setMenu:` 는 AppKit 이 NSStatusItem 의 표시 동작(아이콘)과 독립적인 메뉴 참조만 바꾸는 호출이다.
- ⭐ 결론: **`set_menu` 는 아이콘 리페인트를 직접 유발할 수 없다.** 깜빡임과의 상관은 (가능성 있는 한) "교체 시점과 Bartender 의 메뉴 추적이 겹치는 타이밍"으로만 설명 가능(H2)한다.

### 3-2. `set_visible(false)` = `removeStatusItem` (아이템 완전 제거), `set_visible(true)` = `create()` 재호출 (새 NSStatusItem 생성)

```rust
pub fn set_visible(&mut self, visible: bool) -> crate::Result<()> {
    if visible {
        if self.ns_status_item.is_none() {
            let (ns_status_item, tray_target) = Self::create(...);  // statusItemWithLength + 아이콘·메뉴 재설정
            ...
        }
    } else {
        self.remove();  // NSStatusBar::systemStatusBar().removeStatusItem(ns_status_item)
    }
    Ok(())
}
```

- `mod.rs:193-205`(`set_visible`) · `mod.rs:102-113`(`remove`) · `mod.rs:49-100`(`create`).
- 즉 **숨김 = 제거**(`removeStatusItem`)이고, **다시 보임 = 새 NSStatusItem 생성**이다 — 같은 NSStatusItem 의 `hidden` 뷰 플래그를 끄는 것이 아니다.
- ⭐ 이 때문에 D3 의 스냅샷 폴링이 "숨김 구간에 `status_item_exists == false`"를 그대로 관찰한다 — **Bartender 가 우리 아이콘을 숨길 때 그 값이 바뀌는지, 안 바뀌는지가 곧 "제거하냐 숨기냐"의 판정**이다(단, 그 값이 바뀐다면 이는 Bartender 가 `removeStatusItem` 류를 부른 것이고, 우리 코드가 아니다 — 위 §2).

### 3-3. `set_show_menu_on_left_click` — ivar 플래그만

```rust
pub fn set_show_menu_on_left_click(&mut self, enable: bool) {
    if let Some(tray_target) = &self.tray_target {
        tray_target.ivars().menu_on_left_click.set(enable);  // TrayTarget 의 ivar 하나
    }
    self.attrs.menu_on_left_click = enable;
}
```

- `mod.rs:240-245`. 클릭 동작(메뉴 열기)만 바꾸고 **아이콘 가시성·이미지와 아무 관계가 없다.**
- ⭐ 이슈 본문이 후보로 언급한 `menu_on_left_click` 은 이 사유로 **기각**한다(계획 D4 행 5, 상급 리뷰 수용).

### 3-4. `ns_status_item()` 접근자는 실재한다

- `tray-icon lib.rs:531-533` — `pub fn ns_status_item(&self) -> Option<Retained<NSStatusItem>>`. 계측의 폴링이 이 접근자 + `NSStatusItem::button(mtm)` + `NSButton::image()` 로 "존재·버튼 이미지 유무"를 읽는다 = 계획 리스크 #6 이 예고한 메인 스레드 제약에 맞춰 `run_on_main_thread` 로 디스패치한다(구현: `main.rs:5164`).

---

## 4. 가설 판정

| # | 가설 | 성격 | 판정 | 판정 근거 |
| :--- | :--- | :--- | :--- | :--- |
| **H1** | Bartender 가 아이콘을 숨겼다 복원하는 과정에서 NSStatusItem 의 **상태(메뉴·아이콘)가 어긋난다** | Bartender 측 동작 | **1순위 — 기각할 근거 없음** | 우리 코드는 기본 상황에서 가시성에 개입하지 않으므로(§2), 숨김·복원을 하는 쪽은 Bartender 뿐. Bartender 내부 동작은 1차 출처 확인 불가 `(추정)`. **실기기 관찰로만 판정** |
| **H2** | `set_menu` 교체 시점과 Bartender 의 메뉴 추적이 겹쳐 재배치 시점에 깜빡임 | 상호작용 | **2순위 — 빈도 낮음** | `set_menu` 는 아이콘을 건드리지 않지만(§3-1), Bartender 가 메뉴 구조를 추적 중이면 교체 순간에 아이콘을 다시 그릴 수 있다 `(추정)`. 권한 전이·언어 변경은 드문 이벤트라 빈도가 낮다 |
| **H3** | 최전면 앱 변경 시 `refresh_ignore_menu_item` 의 **NSMenuItem 수준 변경**이 아이콘 깜빡임을 유발 | 우리 코드 | **3순위 — 메커니즘 간접적** | 이 호출들은 NSStatusItem 의 아이콘·가시성과 무관(§2). 성립하려면 Bartender 가 메뉴 항목 변경까지 추적해 아이콘을 다시 그려야 한다 `(추정)` |
| **H4** | Bartender 의 **메뉴바 공간 부족 자동 숨김**이 원인 | Bartender 측 동작 | H1 과 병행 — 같은 관찰로 판정 | `menu-bar-and-lifecycle.md` §5 항목 2 — macOS 자체도 공간 부족 시 상태 아이템을 숨긴다. Bartender 는 이 동작을 확장한 도구다. "반복적으로 사라졌다 나타난다"는 이 숨김·복원이 잦다는 뜻일 수 있다 `(추정)` |

### 기각한 가설

| 가설 | 기각 근거 |
| :--- | :--- |
| **"아이콘 재생성으로 인한 사라짐"** | 코드상 재생성 경로가 없다(§2) — 이슈 본문의 기존 조사와 동일 결론 |
| **"`icon_as_template(true)` 가 원인"** | 템플릿 모드는 빌더에서 한 번 설정되고 런타임 `set_icon_as_template` 호출이 0건(§2) |
| **"`menu_on_left_click` 이 원인"** | ivar 플래그만 바꾸고 아이콘 가시성과 무관(§3-3) |

---

## 5. 결론 — 실기기 재현 대기

1. **우리 코드는 기본 상황에서 `set_visible`/`set_icon` 을 호출하지 않으므로**, 이 현상의 1차 원인 후보는 **Bartender 의 숨김·복원 상호작용(H1/H4)** 이다.
2. Bartender 내부 동작은 1차 출처 확인 불가 `(추정)` — **실기기에서만 판정 가능**하다.
3. 이번 회차는 **계측(§2 마지막 행 + `info` 계측 4종: `tray set_visible called`/`tray set_menu replaced`/`tray ignore menu item refreshed`/`tray status_item snapshot`)만 갖춰 두고**, 사용자가 `docs/dev/manual-verification.md` 의 대조 실험(제외 목록·Bartender 종료)을 수행해 로그를 회신하면 가설을 확정한다.
4. 원인 확정 전에는 **수정하지 않는다**(계획 D5 — 실험 브랜치 추정 수정 기각).