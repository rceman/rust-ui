# Spike API review — composer contract v0.1

**Current candidate:** `d410e39` on `agent/windows-native-composer-contract-spike-v0.1-swe2`
**Reviewed candidate (historical):** `0405a80` — the API excerpts below
reflect that review-time snapshot; the public API surface has not
changed since.
**Baseline:** Astra architecture `29c67025dc88221899a8ab1b381f08491d44b85b`

How the approved architecture reads when compiled against a real native
composer. All snippets come from `examples/composer.rs` on this branch.

## FAST — recipe defaults

```rust
ui.button("send")
    .disabled(self.busy)
    .on_press(|| Msg::Send);
```

The `Primary` recipe drives paint — filled pill, resolved theme colors,
state transitions — with zero authored style data.

## CUSTOM — full typed box model + typed shadow

```rust
ui.action(
    Action::new()
        .label("custom tile action")
        .style(
            ActionStyle::new(
                BoxStyle::new()
                    .background(Color::role(ColorRole::Muted))
                    .radii(CornerRadii::all(dp(8.0)))
                    .padding(Insets::all(dp(10.0)))
                    .shadow(Some(Shadow {
                        color: Color::role(ColorRole::Shadow),
                        offset_x: dp(0.0),
                        offset_y: dp(3.0),
                        blur_sigma: dp(6.0),
                    })),
            )
            .hover(BoxStylePatch::new().background(Color::role(ColorRole::Border))),
        ),
    |ui| {
        ui.row(Row::new().gap(Space::Sm), |ui| {
            ui.label("custom action").color_role(ColorRole::Foreground);
            ui.custom(self.tile.clone());
        });
    },
)
.on_press(|| Msg::ToggleDetails);
```

The `Shadow` is a typed value (offset/sigma/color) painted by the
renderer as a CPU-blurred gradient under the action box — verified in
screenshot evidence.

## SURGICAL — authored patch on a recipe

```rust
ui.button("send")
    .disabled(self.busy)
    .style(
        ButtonStylePatch::new()
            .border_bottom_width(dp(2.0))
            .border_bottom_color(Color::rgb(255, 0, 0)),
    )
    .on_press(|| Msg::Send);
```

The `send` button paints a red bottom border while every other
resolved recipe field stays at the default — the patch is a fieldwise
`Option` map over the recipe's resolved `VisualStyle`.

## SURGICAL on an editor — `TextInputStylePatch`

```rust
ui.text_input(&self.draft)
    .placeholder("draft")
    .label("draft")
    .style(if self.draft_bold {
        TextInputStylePatch::new()
            .foreground(Color::rgb(180, 60, 40))
            .border_bottom_color(Color::rgb(180, 60, 40))
            .border_bottom_width(dp(2.0))
    } else {
        TextInputStylePatch::new()
    })
    .on_edit(Msg::DraftEdited)
    .on_conflict(Msg::DraftConflict)
    .on_submit(|| Msg::Send);
```

The native-editor patch is deliberately capability-limited: `chrome`
fields paint the retained frame around the peer; ONLY `foreground`
reaches the native text service (resolved `Color` -> `EM_SETCHARFORMAT`
/`SCF_ALL` — re-resolved on theme flips since the authored `Color` is
what the peer stores). Font face/size/weight and selection colors stay
OS-owned — this is the approved native-control boundary, not a gap.
The `bold`/`unbold` toggle recolors + re-borders a mounted editor live.

## Keyed reconciliation

```rust
ui.keyed(
    &self.rows,
    |e| e.id,
    |ui, e| {
        let id = e.id;
        ui.text_input(&e.name)
            .label(&format!("row {}", id))
            .on_edit(move |t| Msg::RowEdited(id, t))
            .on_conflict(move |c| Msg::RowConflict(id, c));
    },
);
```

Each row mounts a keyed `RichEdit` peer; `ui.group("details")` with a
conditional surface was intentionally placed *before* the keyed list —
toggling details never remounts the sibling editors (peer identity is
keyed, not positional).

## Disabled gating

```rust
ui.button("send").disabled(self.busy).on_press(|| Msg::Send);
ui.button("stop").disabled(!self.busy).on_press(|| Msg::Stop);
```

`disabled` flows to `n.interactive()` — the node is skipped by
`hit_test` (no hover/press/focus), `dispatch` gates the invoke, and the
UIA fragment reports `IsEnabled=false`, `IsKeyboardFocusable=false`
(covered by the `uia_disabled_state` test).

## Theme — resolved, not ambient

```rust
ui.theme(Theme::resolve(self.theme_mode, ui.appearance())
    .reduced_motion(self.reduced));
```

The runtime stores the resolved `Theme`; `peer_ctx.colors` re-reads it
each turn after `pump()`. A UIA-invoked `SetTheme(Light)` repaints the
whole shell in light palette on the next frame (verified live).

## Custom paint — `CustomRender`

```rust
impl CustomRender for Tile {
    fn measure(&self, _c: Constraints) -> Size {
        Size { width: 96.0, height: 48.0 }
    }
    fn paint(&self, canvas: &mut dyn Canvas, bounds: Rect) {
        let mut p = Path2d::default();
        p.ops.push(PathOp::MoveTo(Point { x: bounds.x, y: bounds.y }));
        p.ops.push(PathOp::LineTo(Point { x: bounds.x + bounds.width, y: bounds.y }));
        p.ops.push(PathOp::LineTo(Point {
            x: bounds.x + bounds.width / 2.0,
            y: bounds.y + bounds.height,
        }));
        p.ops.push(PathOp::Close);
        canvas.path(&p, Paint::fill_role(ColorRole::Accent));
    }
    fn semantics(&self) -> Semantics {
        Semantics { role: Role::Image, label: "decorative tile".into(), actions: Vec::new() }
    }
}
```

`ui.custom(self.tile.clone())` mounts the tile as a painted node;
`semantics()` feeds UIA (`Role::Image`, `label`).

## What the composer exercises end-to-end

- 4 windowless RichEdit editors (draft + 3 keyed rows), real typing
- button, label, group/row/column, surface, custom action, custom tile
- surgically-patched send button (red bottom border)
- styled action with typed shadow
- theme toggle (dark/light/system)
- reduced-motion toggle wired to `SetReduced`
- disabled `send`/`stop` gating (no hover/press/focus/invoke)
- live `TextInputStylePatch` re-application on a mounted editor
- keyed row removal through UIA Invoke → state mutation → relayout
