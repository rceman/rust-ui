# Layout, Style and Motion

Status: architecture concept only. All signatures are proposed API exercises.

## Layout primitives

Four containers cover the vocabulary; each is a small props struct consumed
with a child closure:

```rust
ui.row(Row::new().gap(Space::Sm).align(Align::Center), |ui| {
    ui.label("Left");
    ui.button("Go").on_press(|| Msg::Go);
});
ui.column(Column::new().gap(Space::Sm), |ui| {
    ui.label("Top");
});
ui.stack(Stack::new(), |ui| {
    ui.image(self.banner.clone());
    ui.badge("New");
});
ui.surface(Surface::new().padding(Space::Md), |ui| {
    ui.label("Inset content");
});
```

Sizing on any node:

```rust
pub fn dp(value: f32) -> Dp;

pub enum Length {
    Content,
    Fixed(Dp),
    Fill(u16),
}

pub enum Align {
    Start,
    Center,
    End,
    Stretch,
}

pub enum Justify {
    Start,
    Center,
    End,
}
```

Leaf usage: `.width(Length::Fill(1)).min_width(dp(160.0)).max_width(dp(480.0))`.

Axis algorithm — a finite water-fill, deterministic and bounded:

1. Validate finite non-negative sizes, `min <= max`, and positive fill
   weights. Measure and clamp fixed/content children; subtract these
   allocations, padding and gaps from the main-axis budget.
2. For the remaining fill budget `R`, choose sizes
   `x_i = clamp(lambda * weight_i, min_i, max_i)`. If `R` is below the sum of
   minima, use the minima and clip overflow. If `R` exceeds the sum of
   maxima, use the maxima and leave surplus unused. An omitted `max` means
   unbounded — it contributes no finite upper breakpoint.
3. Otherwise solve `sum(x_i) = R` by walking the sorted weighted min/max
   breakpoints. There are at most two finite breakpoints per child; within
   each interval the sum is linear. Stop as soon as the interval containing
   the solution is found. This handles mixed min/max constraints without an
   unbounded layout iteration or a one-redistribution shortcut.
4. Pack any unused main-axis space with `Justify::{Start,Center,End}`
   (default `Start`). `.justify(...)` on `Row`/`Column` controls this;
   `.align(...)` remains cross-axis-only. Final physical-pixel rounding
   assigns residual pixels in stable child order so logical allocations do
   not exceed their budget through independent rounding.
5. No wrapping container algorithm, percent sizing, CSS cascade or general
   Flexbox/Grid is introduced. Label text wrapping is the separate
   width-then-height process below.

`Fill` on an unbounded main axis (e.g. inside a horizontally scrolled row) is
a layout error reported to the consumer, never silently resolved.

Any leaf can also carry `.visibility(Visibility::Hidden)`: a hidden node keeps
its arena slot, layout footprint and — for a native text peer — its undo and
committed state, but is removed from hit-testing, the focus order and the
accessibility tree, and its motion is suspended. A focused node going hidden
relocates focus to the next eligible node; native composition follows the
same unmount-adjacent policy as removal. Conditional unmounting (`if` inside
`ui.group`) is the tool for reclaiming layout space; `Hidden` is for
retention.

Cross-axis alignment is `Align::{Start,Center,End,Stretch}` — no baseline
variant in v0.1 and no silent fallback to `Center`; baseline alignment is
deferred until platform baselines are a measured input. Text layout resolves
available widths top-down first, then measures heights at the assigned width;
height may not feed back into width in v0.1, so no multi-pass negotiation is
needed. `ui.label` accepts `.wrap(true)` for wrapping text; a "content/response
surface" is simply the composite `ui.surface` around a wrapping `ui.label`,
not a dedicated chat-domain widget.

`Stack` children share the container's content rectangle and paint in
declaration order; painted children must not overlap native text peers — the
native-island layer rule in [NATIVE_CONTROL_BOUNDARY.md](NATIVE_CONTROL_BOUNDARY.md) applies. A tooltip
overlay never contributes to its ancestor's desired size.

All sizes are logical dp. Snapping to physical pixels happens once, at the
backend boundary; identical sub-pixel text across platforms is not a promise.

## Theme and style

```rust
impl Theme {
    pub fn light() -> Self;
    pub fn dark() -> Self;
}

#[derive(Copy, Clone, Eq, PartialEq, Default)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

pub enum ColorRole {
    Background, Foreground,
    Muted, MutedForeground,
    Accent, AccentForeground,
    Border, Destructive, Focus,
}

pub enum Space { Xs, Sm, Md, Lg }
pub enum Radius { None, Sm, Md, Lg }
pub enum MotionToken { Hover, Tooltip, Focus }

pub enum ButtonVariant { Primary, Secondary, Ghost, Destructive }
pub enum ControlSize { Sm, Md, Lg }

#[derive(Default)]
pub struct ButtonStyle {
    pub foreground: Option<ColorRole>,
    pub radius: Option<Radius>,
    pub padding: Option<Space>,
}

pub enum Visibility {
    Visible,
    Hidden,
}

pub struct Appearance {
    pub dark: bool,
}

impl Theme {
    pub fn reduced_motion(self, policy: ReducedMotion) -> Self;
    pub fn resolve(mode: ThemeMode, appearance: Appearance) -> Self;
}
```

Where each belongs: built-in enums (`ButtonVariant`, `ControlSize`,
`ColorRole`, `Space`, `Radius`, `MotionToken`) express common variation;
typed style structs (`ButtonStyle` and siblings) express rare overrides.
There is no string property bag and no CSS runtime.

Style precedence, in order:

```text
built-in defaults
  -> theme recipe (light/dark token resolution)
  -> variant + size
  -> typed overrides (e.g. ButtonStyle fields)
  -> accessibility forced-colors / focus-visibility layer
```

Interaction states (hover/pressed/disabled) resolve inside the recipe, not as
consumer-visible selectors. Per-node escape hatches stay typed:

```rust
ui.button("Accent")
    .variant(ButtonVariant::Primary)
    .style(ButtonStyle {
        foreground: Some(ColorRole::AccentForeground),
        ..ButtonStyle::default()
    })
    .motion(MotionToken::Hover);
```

`ui.theme(theme)` is called at the window root before building children and
consumes the `Theme` value — it is copied/resolved into the window's token
set, not a cascade. A stored `self.dark: bool` maps to
`ui.theme(if self.dark { Theme::dark() } else { Theme::light() })`. Following
the OS is explicit, not a second mechanism: `Theme::resolve(mode, ui.appearance())`
maps a `ThemeMode` (including `ThemeMode::System`) through the current
`Appearance`, the runtime invalidates the view when the OS appearance
changes, and `Theme::dark().reduced_motion(ReducedMotion::System)` binds a
reduced-motion policy to the theme.

Platform fonts default to the system UI face — Segoe UI family on Windows,
the system font on macOS — with user text scaling honored.

Icons are a typed `Icon` enum (Lucide source, converted offline to compact
Rust path data with a pinned upstream version and license). There is no
SVG/XML parsing at runtime and no runtime asset path parsing for icons.
`ui.icon_button(Icon::Sun).label("Use light theme")` — the label supplies the
mandatory accessible name; a tooltip is not a substitute. Colour is never the
sole carrier of status.

## Motion

Motion is token-driven, not an animation rig:

- `MotionToken::Hover` — hover/focus colour transitions on controls;
- `MotionToken::Tooltip` — fade/scale on tooltip overlay chrome only;
- `MotionToken::Focus` — focus ring appearance.

There is no dedicated spinner widget in v0.1: a `custom` node with
`frame_events` enabled covers genuinely active, visible work, and a
reduced-motion user or idle UI gets static `Busy` text instead. Chrome may
animate; live text contents never do (see [NATIVE_CONTROL_BOUNDARY.md](NATIVE_CONTROL_BOUNDARY.md)).

`frame_events` demand is decorative and doubly gated: the node must be
mounted and `Visible`, **and** the effective reduced-motion policy must allow
it — under `Reduce` no frame events are delivered and the node retains its
static snapshot. Resuming after a hidden or reduced-motion suspension resets
the `FrameTime` delta clock rather than delivering the suspended interval as
one giant first delta. Product async progress/text updates are unaffected;
essential real-time media clocks are outside v0.1.

Scheduling contract:

- One earliest-deadline scheduler per window drives transitions.
- Invariant: no active transition **and** no visible `frame_events` demand →
  no rust-ui animation tick and no requested redraw. There is no permanent
  frame loop; a pending tooltip-delay timer is one-shot interaction work, not
  an idle render loop.
- Caveat: native caret blink, IME UI, accessibility and OS expose events can
  wake the pipeline independently; "zero wakeups" is never promised while a
  focused caret blinks. The idle guarantee applies to rust-ui-driven work.
- Timers are paused/cancelled when the window hides or the owner node
  unmounts; motion resources are reclaimed at node removal.

Reduced motion:

```rust
pub enum ReducedMotion { System, Reduce, NoPreference }
```

`ReducedMotion::System` is the default: spatial motion and spinners disable
under the platform preference in favour of instant/static presentation.
