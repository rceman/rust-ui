# Shadcn Native Gallery — Slice 3 evidence

Production code candidate: **`b14c79a`** on `agent/shadcn-native-gallery-slice3-opus55` (base `4e38097`).
Frozen reference: `reference/shadcn-gallery-v0.1/` @ `2e14e417a69ef8967b8fb17120282e3de6449fae` (unmodified).
Capture host: native Windows 11 VM, scale_factor 1.0, WGC (`Windows.Graphics.Capture`), `stable_retained_scene` verified per shot.

Slice 3 ships: **Tabs** (default + line), **Select** (combobox + listbox popup), **Dropdown Menu** (menu items/checkbox/radio), **SettingsNav** (256dp / collapsed 48dp rail), the generic **popup/overlay foundation**, and the **Navigation + Overlays** Gallery pages.

## Layout

- `visual/fullpages/{light,dark}/` — full client captures per page (`components`, `forms`, `navigation`, `typography`, `overlays`, `native-text`). Staged via `RUI_GALLERY_PAGE=<page>`; dark via `shell.theme.dark` click.
- `visual/states/{light,dark}/` — element crops: `tabs.default.active`, `tabs.line.active`, `settings-nav.expanded`, `settings-nav.collapsed` (via `RUI_SETTINGS_NAV_COLLAPSED=1`), `select.open`, `dropdown-menu.open`, `button.default`, `card.default`.
- `interaction/devctl-slice3-verify.txt` — scripted `rust-ui-devctl` run: semantic ids, `expanded`/`selected`/`checked`/`focused`/`enabled` fields, Escape topmost dismissal, focus-on-open into content, arrow nav, Enter pick → close+value+focus restore, outside-press dismiss + restore.
- `resize-sanity/` — 3× `resize`+`wait-idle` cycles (800/1200/960 wide) = 130 ms wall total incl. RPC; `resized-960.png` + `.capture.json`.
- `*.capture.json` — devctl capture metadata (backend, client crop, scale).

## Verified interaction behavior (devctl, native)

- `focus_on_open`: open select lands focus on the **selected option** (`Production`); open menu lands focus on first item (`Profile`).
- Option/menu arrow nav: `Down` walks enabled items and **skips disabled** (`Billing` skipped between `Team members`→`Show toolbar`).
- `Enter` on focused option → `Pick` → popup closes, trigger label updates (`Preview`), focus restored to trigger.
- `Escape` closes the **topmost** popup (menu then select, in paint order); `press-at` outside dismisses and restores trigger focus.
- Tabs: `press-at` on a `list.tab[*]` part focuses+activates; `Right`/`Left` move focus **and activate** (Radix automatic); `Home`/`End` jump ends.
- SettingsNav items report `selected` + `navigation-item` kind; `expanded` on `settings-nav.theme` combobox.

## Gate results on this candidate

- Linux: `cargo fmt --check`, `cargo check --locked`, `cargo test --locked --lib` — **123/123**.
- Windows native: `cargo test --locked --lib` **165/165**; `--features devtools --lib` **194/194**; `cargo check --all-targets --features devtools` clean.
- `cargo test --locked --test devctl_windows --features devtools` — **1/1 (39.02 s)**: real `rust-ui-gallery` app end-to-end (launch/session/handshake/wait-idle/tree/rect/TARGET_NOT_FOUND/token rejection/hover/leave/press/release/click/focus/key/native editing/snapshot-layout/screenshot/compare-layout/compare-image/compare-element/handles/idle/shutdown/cleanup, 3 cycles).

## Notes

- `stable_retained_scene` + paint-epoch guard on every WGC shot; the CLI retries the capture boundedly on the documented transient `CAPTURE_FAILED` (stray OS pointer message mid-acquisition).
- No synthetic devctl UI exists. `rust-ui-gallery` [[bin]] compiles `examples/gallery.rs` verbatim via `include!`; `tests/devctl_windows.rs` launches it through `CARGO_BIN_EXE_`.
