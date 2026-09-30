# Style Customization Model

Status: architecture concept only — proposed API exercise, not implemented,
not type-checked, and not approved by the owner. This document is the single
authoritative styling definition; [LAYOUT_STYLE_MOTION.md](LAYOUT_STYLE_MOTION.md)
keeps layout, motion and theme mechanics and links here rather than restating
style rules.

Written on branch `agent/style-customization-model-review` against the
committed architecture at `768eb172509a45ed15cdf5156fe57268ccaf8b08`.

## Three workflows, one renderer

The review keeps the owner's three first-class workflows — they are three
ways of authoring UI over one paint/style pipeline, not three renderers:

- **FAST** — a standard component with variants, sizes, theme recipe and
  typed interaction behavior:

```rust
ui.button("Send")
    .variant(ButtonVariant::Primary)
    .size(ControlSize::Md)
    .icon(Icon::ArrowUp)
    .on_press(|| Msg::Send);
```

- **CUSTOM** — bespoke UI composed from public typed primitives.
- **SURGICAL** — a standard component plus a field-level partial patch:

```rust
ui.button("Send")
    .variant(ButtonVariant::Primary)
    .style(
        ButtonStylePatch::new()
            .border_bottom_width(dp(2.0))
            .border_bottom_color(Color::rgb(255, 0, 0)),
    );
```

Built-in painted components resolve through the **same** public box/text
style representation and core draw/hit/focus/accessibility machinery.
Recipes decompose into boxes, text and icons through inspectable ordinary
Rust functions and tables — there is no private parallel renderer and no
second hidden styling system. Native editable text stays a native peer;
it is never painted. Typed messages, `update`/`view`, the retained arena,
the task model and the native-control contract are all unchanged: styling
decides how a node looks, never how events flow.

## Primitive vocabulary (v0.1)

| Primitive | Role |
|---|---|
| `ui.box_(BoxProps, draw)` | noninteractive painted box container |
| `ui.text(text) -> TextBuilder` | styled text leaf; `ui.label` is a recipe convenience over it, not a second text renderer |
| `ui.icon`, `ui.image`, `ui.custom` | existing leaves, unchanged |
| `Row`/`Column`/`Stack`/`Surface` | layout containers; `Surface` is a recipe convenience over `box_` |
| `ui.action(Action, draw) -> ActionBuilder<'_, M>` | semantic activation wrapper over arbitrary box content |
| `ui.text_input`, `ui.text_area` | native editable peers via `TextValue`, unchanged — no painted `NativeText` primitive |

`box_`/`BoxProps` are named to avoid colliding with `std::boxed::Box`.

```rust
pub struct BoxProps {
    _private: (),
}
impl BoxProps {
    pub fn new() -> Self;
    pub fn style(self, style: BoxStyle) -> Self;
    pub fn width(self, width: Length) -> Self;
    pub fn height(self, height: Length) -> Self;
    pub fn min_width(self, min: Dp) -> Self;
    pub fn max_width(self, max: Dp) -> Self;
    pub fn min_height(self, min: Dp) -> Self;
    pub fn max_height(self, max: Dp) -> Self;
}
```

`BoxProps` is opaque — it bundles the authored `BoxStyle` plus the retained
layout props, kept separate inside so style never overwrites size. The same
`.width`/`.height` (`Length`) and `.min_*`/`.max_*` (`Dp`) setter vocabulary
exists on every node and container builder — rows, columns, `ui.action`,
built-ins and native inputs — so size is uniformly layout, not style. `Row`
gap/alignment and `Length` stay layout; styling moves none of them.

### Action — semantic bespoke controls

```rust
pub struct Action {
    pub label: String,
    pub style: ActionStyle,
    pub disabled: bool,
}
impl Action {
    pub fn new() -> Self;
    pub fn label(self, name: &str) -> Self;
    pub fn style(self, style: ActionStyle) -> Self;
    pub fn disabled(self, disabled: bool) -> Self;
}

impl<'ui, M> Ui<'ui, M> {
    pub fn action(
        &mut self,
        props: Action,
        draw: impl FnOnce(&mut Ui<M>),
    ) -> ActionBuilder<'_, M>;
}
```

- `.on_press(|| M)` and `.disabled(..)` reuse the standard factories and
  semantics — a custom action activates identically on pointer release,
  Enter/Space and accessibility Invoke, and participates in focus traversal.
  An accessible name is mandatory.
- The action node is the single semantic hit/focus owner; its children are
  decorative content for this action and may not contain another actionable
  or focusable node or a native text peer — that nesting is a structural
  `UiDiagnostic::InvalidComposition`, not bubbling.
- `ui.box_` is purely noninteractive; there is no pointer-only shortcut and
  no `clicked()`-style immediate dispatch anywhere. Consumers need no
  `CustomRender` or backend handle for an ordinary bespoke control.
- Action shares the runtime interaction-state machinery with built-ins:
  hover/pressed/focus styling is runtime-owned and needs no app messages —
  while consumers can still explicitly bind the ordinary observational
  `.on_pointer_*`/`.on_focus`/`.on_blur` handlers from
  [STATE_AND_EVENTS.md](STATE_AND_EVENTS.md).

## Authored style types

```rust
#[derive(Copy, Clone, Eq, PartialEq)]
pub enum Color {
    Role(ColorRole),
    Rgba { r: u8, g: u8, b: u8, a: u8 },
}
impl Color {
    pub const fn role(role: ColorRole) -> Self;
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self;
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self;
}

impl Default for Color {
    fn default() -> Self;
}
```

```rust
#[derive(Copy, Clone, Default, PartialEq)]
pub struct Dp(f32);
```

`ColorRole` derives `Copy, Clone, Eq, PartialEq` (its canonical declaration
lives in [LAYOUT_STYLE_MOTION.md](LAYOUT_STYLE_MOTION.md)); `Color::default()`
is transparent `Color::rgba(0, 0, 0, 0)`, which keeps every composite
`Default` below coherent. `Dp` is `Copy, Clone, PartialEq` with a zero
`Default`; numeric validation (finite, non-negative — positive for text
size) is staged as diagnostics, never constructor panics.

- Straight sRGB `u8` channels only — no float channels, so malformed colors
  are unrepresentable; backends convert to premultiplied form internally.
- `Color::rgb` sets `a = 255`; `rgba` takes alpha explicitly. `a` is
  paint-color alpha, **not** group/node opacity.
- `ColorRole` stays the preferred path; `DestructiveForeground` is added so
  the destructive pair is complete.

```rust
#[derive(Copy, Clone, Default, PartialEq)]
pub struct Insets {
    pub top: Dp,
    pub right: Dp,
    pub bottom: Dp,
    pub left: Dp,
}
impl Insets {
    pub fn all(v: Dp) -> Self;
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct CornerRadii {
    pub top_left: Dp,
    pub top_right: Dp,
    pub bottom_right: Dp,
    pub bottom_left: Dp,
}
impl CornerRadii {
    pub fn all(v: Dp) -> Self;
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct BorderSide {
    pub width: Dp,
    pub color: Color,
}
impl BorderSide {
    pub fn new(width: Dp, color: Color) -> Self;
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct Border {
    pub top: BorderSide,
    pub right: BorderSide,
    pub bottom: BorderSide,
    pub left: BorderSide,
}
impl Border {
    pub fn all(side: BorderSide) -> Self;
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct BoxStyle {
    pub background: Color,
    pub border: Border,
    pub radii: CornerRadii,
    pub padding: Insets,
}
impl BoxStyle {
    pub fn new() -> Self;
    pub fn background(self, color: Color) -> Self;
    pub fn border(self, border: Border) -> Self;
    pub fn radii(self, radii: CornerRadii) -> Self;
    pub fn padding(self, padding: Insets) -> Self;
}

#[derive(Copy, Clone, Eq, PartialEq)]
pub enum TextWeight {
    Normal,
    Medium,
    Bold,
}

#[derive(Copy, Clone, PartialEq)]
pub enum TextSize {
    Body,
    Exact(Dp),
}

#[derive(Copy, Clone, PartialEq)]
pub struct TextStyle {
    pub foreground: Color,
    pub size: TextSize,
    pub weight: TextWeight,
}
impl TextStyle {
    pub fn new() -> Self;
    pub fn foreground(self, color: Color) -> Self;
    pub fn size(self, size: Dp) -> Self;
    pub fn weight(self, weight: TextWeight) -> Self;
}
impl Default for TextStyle {
    fn default() -> Self;
}

#[derive(Copy, Clone, PartialEq)]
pub struct VisualStyle {
    pub box_style: BoxStyle,
    pub text_style: TextStyle,
}
```

Defaults: `BoxStyle::new()` is transparent background, zero-width borders,
zero radii, zero padding. `TextStyle::new()`/`TextStyle::default()` is
`ColorRole::Foreground`, `Normal` weight, and `TextSize::Body` — a *token*
that resolves to the theme base text size adjusted for OS text scaling, not
a literal `Dp` baked into a parameterless constructor. `TextSize` is the
recommended shape: it makes the theme-resolved default explicit instead of
faking a constant. `TextSize::Exact(dp)` requests an explicit logical size —
`Exact` still receives OS text scaling like everything else; it is a
requested unscaled dp value, not a bypass. Text sizes must be positive
finite (stricter than the non-negative rule for padding/borders). System
font is fixed for v0.1: no public font-family or string font loading, no
text decorations, no letter spacing, no arbitrary inheritance. `.wrap` stays
a text behavior prop. `Space`/`Radius` tokens remain for recipes and layout
and expand to concrete `Insets`/`CornerRadii` during resolution — likewise
`Surface::padding(Space::Md)` is a recipe convenience that populates the
same `BoxStyle.padding` before any consumer patch, not a second padding.
No shadow type is declared.

## Partial patches

```rust
#[derive(Copy, Clone, Default, PartialEq)]
pub struct BorderSidePatch {
    pub width: Option<Dp>,
    pub color: Option<Color>,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct BorderPatch {
    pub top: BorderSidePatch,
    pub right: BorderSidePatch,
    pub bottom: BorderSidePatch,
    pub left: BorderSidePatch,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct CornerRadiiPatch {
    pub top_left: Option<Dp>,
    pub top_right: Option<Dp>,
    pub bottom_right: Option<Dp>,
    pub bottom_left: Option<Dp>,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct InsetsPatch {
    pub top: Option<Dp>,
    pub right: Option<Dp>,
    pub bottom: Option<Dp>,
    pub left: Option<Dp>,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct BoxStylePatch {
    pub background: Option<Color>,
    pub border: BorderPatch,
    pub radii: CornerRadiiPatch,
    pub padding: InsetsPatch,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct TextStylePatch {
    pub foreground: Option<Color>,
    pub size: Option<Dp>,
    pub weight: Option<TextWeight>,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct VisualStylePatch {
    pub box_style: BoxStylePatch,
    pub text_style: TextStylePatch,
}
```

Rules:

- `None` means untouched; explicit `Some(dp(0.0))` clears border width,
  padding or radius, and `Some(Color::rgba(0, 0, 0, 0))` clears painted color.
  These values still obey field constraints: zero text size is invalid, and
  native backing/foreground must remain opaque. There is no "set this Option
  to clear" ambiguity.
- The override unit is the named field, recursively — patches never carry a
  whole `Border` or `BoxStyle` to swap in, and default-filled full styles are
  not accepted as component overrides.
- `Some(Color::Role(..))` stores the authored role value, not a
  theme-resolved equality bypass: resolution compares concrete values.
- `TextStylePatch.size: Some(dp)` maps to `TextSize::Exact(dp)`; `None`
  leaves the full style's size token (e.g. `Body`) untouched.
- Patches are Copy-sized sparse POD-like values — no string maps, no `Vec`s.

`ButtonStylePatch` is the named wrapper for the props valid on button and
icon-button — the common patch plus fixed typed state slots:

```rust
#[derive(Copy, Clone, Default, PartialEq)]
pub struct StateStyles<P> {
    pub base: P,
    pub hover: Option<P>,
    pub pressed: Option<P>,
    pub disabled: Option<P>,
    pub focus_visible: Option<P>,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct ButtonStylePatch {
    pub styles: StateStyles<VisualStylePatch>,
}
impl ButtonStylePatch {
    pub fn new() -> Self;
    pub fn background(self, color: Color) -> Self;
    pub fn foreground(self, color: Color) -> Self;
    pub fn border_bottom_width(self, width: Dp) -> Self;
    pub fn border_bottom_color(self, color: Color) -> Self;
    pub fn hover(self, patch: VisualStylePatch) -> Self;
    pub fn pressed(self, patch: VisualStylePatch) -> Self;
    pub fn disabled(self, patch: VisualStylePatch) -> Self;
    pub fn focus_visible(self, patch: VisualStylePatch) -> Self;
}
```

Base setters route into `styles.base` — there is no separate base semantics;
`ButtonStylePatch::new().border_bottom_width(dp(2.0)).border_bottom_color(..)`
sets exactly those two fields and nothing else. No sprawling generic styling
trait system.

- Repeated `.style(..)` on a component merges nested fields left to right:
  a later explicit `Some` wins; `None` never clears an earlier value —
  **within the current node build in one view transaction only**. Each
  subsequent view pass reconstructs the authored patch from the new
  declaration: a field absent this pass resolves from recipe defaults, not
  from last pass's patch history. The same applies to repeated
  `.hover(..)`/`.pressed(..)` calls, which merge fieldwise into that state
  slot rather than replacing the whole state patch. Primitive *full* styles
  (`ui.text`, `ui.box_`) are replacements, not sparse merges. Clearing
  deliberately via an explicit zero width or transparent rgba is unaffected.
- Capability-typed `.style(..)` surfaces: `button`/`icon_button` take
  `ButtonStylePatch`, `surface` takes `BoxStylePatch`, `label` takes
  `TextStylePatch`, `badge` takes `VisualStylePatch` — enough without a
  per-widget wrapper zoo.
- `ui.text`'s `.style(TextStyle)` and `BoxProps::style(BoxStyle)` take full
  authored styles — primitives want explicit complete styling, not patches.
- `.color_role(role)` on label/icon remains a typed shorthand that sets only
  the foreground role inside that control's recipe patch — not a second path.
- An optional typed state example, `.hover(...)` on a button patch, is shown
  below in the state model.

## State styles and the deterministic resolution path

```rust
#[derive(Copy, Clone, PartialEq)]
pub struct ActionStyle {
    pub base: BoxStyle,
    pub hover: Option<BoxStylePatch>,
    pub pressed: Option<BoxStylePatch>,
    pub disabled: Option<BoxStylePatch>,
    pub focus_visible: Option<BoxStylePatch>,
}
impl ActionStyle {
    pub fn new(base: BoxStyle) -> Self;
    pub fn hover(self, patch: BoxStylePatch) -> Self;
    pub fn pressed(self, patch: BoxStylePatch) -> Self;
    pub fn disabled(self, patch: BoxStylePatch) -> Self;
    pub fn focus_visible(self, patch: BoxStylePatch) -> Self;
}
```

`ActionStyle` owns a full authored `BoxStyle` base plus optional named
state patches; it carries no text style — box/text children inside an
`ui.action` declare their own explicit styles, and nothing inherits down.
A built-in `Button` resolves one `VisualStyle` and binds its own recipe's
text and icon parts to that resolved text style — a closed recipe-part
mapping, not ancestor inheritance.

Precedence, in exactly this order — "why did this property get this value?"
always has one answer:

```text
theme tokens
  -> component defaults
  -> variant
  -> size
  -> recipe interaction-state overlay
  -> consumer BASE partial patch
  -> consumer ACTIVE state patch
  -> accessibility / OS enforcement
```

- The interaction branch is exclusive: `disabled`, else `pressed`, else
  `hover`, else `normal` — a priority chain, never "hover+pressed"
  specificity. `focus_visible` is orthogonal: the recipe focus overlay
  applies after the recipe's active branch; the consumer `focus_visible`
  patch applies after the consumer's active branch.
- Disabled removes focus eligibility and suppresses press/hover/focus style
  branches and activation.
- A consumer base patch applies across all states; a consumer state patch
  (`hover`, `pressed`, ...) contributes only its supplied fields on top of
  that state's resolved values.
- **Regression invariant (before OS enforcement):** patching only bottom
  border width/color leaves every unrelated authored/resolved style and
  layout-input property — background, foreground, the other three borders,
  radii, padding, size, motion and focus configuration — value-identical in
  *each* supported state. Derived geometry and measurement *may* change (the
  new border width consumes content insets), and OS-enforced colors may
  legitimately differ — the invariant covers inputs before enforcement, not
  pixel-identical output.
- Accessibility/OS enforcement (forced colors, required focus visibility,
  reduced motion) always runs last and may override consumer colors —
  explicitly a separate concern from the regression invariant, which holds
  before that layer.
- Consumer style cannot remove required accessible semantics, disabled
  behavior, the minimum enforced focus indicator or the reduced-motion
  policy. The focus indicator is its own chrome distinct from `Border`:
  zeroing border widths cannot remove it.
- App-driven state styling (a "selected" row, an error tint) is chosen in
  `view` by passing a different variant/style — there is no pseudo-selector
  system. Motion stays `MotionToken` + the earliest-deadline scheduler; no
  style animation selectors or loops.

Typed state example (optional, shows the layered patch shape):

```rust
ButtonStylePatch::new().hover(VisualStylePatch {
    box_style: BoxStylePatch {
        background: Some(Color::role(ColorRole::Background)),
        ..Default::default()
    },
    ..Default::default()
})
```

## v0.1 property decisions

| Candidate | Decision |
|---|---|
| background | **v0.1** — `BoxStyle.background: Color` |
| foreground | **v0.1** — `TextStyle.foreground: Color` |
| border width/color | **v0.1** — `BorderSide{width,color}` |
| independent border sides | **v0.1** — `Border{top,right,bottom,left}` |
| corner radius | **v0.1** — `CornerRadii` |
| independent corner radii | **v0.1** — four named `Dp` fields |
| padding | **v0.1** — `BoxStyle.padding: Insets` |
| independent padding sides | **v0.1** — `Insets{top,right,bottom,left}` |
| width / height | **v0.1 as layout props** — `.width`/`.height` on nodes and containers, `Length`/`Dp` with the same validation; not style fields |
| min / max width | **v0.1 as layout props** — `.min_width`/`.max_width` |
| min / max height | **v0.1 as layout props** — `.min_height`/`.max_height` |
| opacity (group/node) | **deferred** — paint-color alpha suffices for flat fills; real group opacity needs offscreen/ink-bounds work; not rejected forever |
| shadow | **deferred** — same offscreen/ink-bounds/native complexity; not rejected forever |

Rejected outright: CSS parser, selectors, specificity, cascade, arbitrary
inheritance, string property bags, DOM concepts, pseudo-selector strings,
browser layout machinery, percent sizing.

Deferred: gradients, filters, transforms, blend modes, complex border dashes
and border images, general font-family/variable-font loading, authored
transitions, general Flexbox/Grid.

## Geometry and validation

- Borders are **solid only**. Side strokes paint inside the box; at rounded
  joins they split on the deterministic corner bisector — no gaps and no
  double-painted overlap.
- Border widths contribute to content insets and measured extents together
  with padding: a patch that changes a border width may change layout
  without changing any unrelated *resolved* property value.
- `.width`/`.height`/`.min_*`/`.max_*` remain separate layout props — style
  never overwrites them.
- Corner radii use a finite, deterministic proportional clamp so adjacent
  radii sums fit the final box — geometry normalization on already-valid
  input; authored values are not mutated.
- Validation: `Dp` finite and non-negative, `min <= max`. Malformed numeric
  styles produce `UiDiagnostic::InvalidStyle` before any native mutation;
  forbidden interactive nesting produces `UiDiagnostic::InvalidComposition`;
  an impossible platform capability produces `UiError::Unsupported`. Nothing
  is silently ignored or clamped except the declared radii normalization.
- Alpha fills over painted content are permitted; alpha on live text and
  ancestor group opacity are not.

## Resolution internals, equality and damage

- One shared core resolver turns the public authored types and patch
  vocabulary into private `ResolvedBoxStyle`/`ResolvedTextStyle`/
  `ResolvedVisualStyle` — concrete colors (no unresolved roles or tokens),
  concrete dp, plus distinct focus chrome and the existing motion config.
- Theme recipes are pure named Rust tables/functions — no string lookup and
  no per-node allocation.
- Resolved values are cached per retained node, keyed by theme revision,
  appearance revision, accessibility revision, variant, size, authored patch
  and runtime interaction state — no global unbounded style cache.
- Equality compares concrete final values, not inputs: different roles can
  resolve to the same color (no spurious damage), and one role resolves
  differently across a theme revision (real damage). Damage classes:

  | Changed resolved input | Downstream work |
  |---|---|
  | background / border color / text foreground | paint only |
  | border width, padding | measure + layout + paint (content insets) |
  | radii | paint + clip + hit-test boundary |
  | text size / weight | shaping + measure + layout + paint |
  | theme/appearance revision | re-resolve colors/metrics where changed |
  | semantics / system-enforced a11y | accessibility work only on actual change |

  Same-value patch or repeated unchanged traversal does no downstream
  layout/paint/peer/accessibility work at all.

  Painted box vs native-peer exception: the table above is the painted-box
  contract. On native text, a chrome radius change alters the safety insets
  below — so it costs layout + peer geometry update + chrome paint, unlike
  painted-box radii; a foreground/backing change produces a peer `apply` +
  paint — never `set_text`, a remount, or an undo/selection/IME reset. A
  theme/appearance revision re-resolves all relevant tokens, but only a
  changed concrete value produces damage — equal values never reach the
  peer.
- Damage region is the union of old and new ink/geometry, plus affected
  descendants/neighbours when layout moves; focus chrome is tracked
  separately. No numeric epsilon may hide a difference in documented finite
  values; `+0`/`-0` normalize for deterministic equality; `NaN` is never
  accepted.
- The retained style **target** is kept separate from transient interpolated
  presentation: an unchanged target never restarts a transition, so a
  repeated view pass emits no motion ticks after settle.
- One retained arena — no extra virtual tree, no subscriptions. Closure
  factories and app traversal may allocate as before; style resolution aims
  for allocation-free fixed-size work, which the spike measures rather than
  promises.

## Native editable text boundary

`ui.text_input`/`ui.text_area` and `TextValue` are unchanged — no painted
`NativeText` primitive exists. Their style surface is capability-limited:

```rust
#[derive(Copy, Clone, Default, PartialEq)]
pub struct TextInputStylePatch {
    pub chrome: BoxStylePatch,
    pub foreground: Option<Color>,
}
impl TextInputStylePatch {
    pub fn new() -> Self;
    pub fn foreground(self, color: Color) -> Self;
    pub fn background(self, color: Color) -> Self;
    pub fn border_bottom_width(self, width: Dp) -> Self;
    pub fn border_bottom_color(self, color: Color) -> Self;
}
```

- The chrome patch maps only to the painted frame — background, border,
  radii, padding — around the native peer. `foreground` reaches the peer
  through a typed adapter subject to normal OS adjustment; the peer backing
  derives from `chrome.background` and v0.1 requires both backing and
  editable foreground to resolve **fully opaque**.
- The peer safety inset is mechanical, using *authored* radii
  (pre-normalization values) so width-first layout never depends on final
  radius normalization:

  - `left   = border.left.width   + padding.left   + max(top_left, bottom_left)`
  - `right  = border.right.width  + padding.right  + max(top_right, bottom_right)`
  - `top    = border.top.width    + padding.top    + max(top_left, top_right)`
  - `bottom = border.bottom.width + padding.bottom + max(bottom_left, bottom_right)`

  Minimum measurement reserves these insets plus the peer's intrinsic size.
  The final painted outer radii still normalize as described in the geometry
  rules — normalization never clips the rectangular peer, which is also
  never alpha-composited.
- An explicit constraint too small to host the peer is
  `UiDiagnostic::InvalidLayout`; a nonopaque backing or foreground is
  `UiError::Unsupported`; malformed numbers are `UiDiagnostic::InvalidStyle`;
  computed extents must stay finite — no overflow from sums. Alpha border
  paint remains allowed *outside* the editing region; there are simply no
  transparency-flag, mask or group-opacity fields on native peers.
- Selection, caret, IME and undo remain native: their colors and fonts are
  primarily OS-owned and are not public `TextStyle` knobs; system fonts and
  text scaling remain in force.
- Theme, style and geometry updates must not remount the peer, set text, or
  reset undo/selection/composition — resolved-value equality prevents
  redundant `apply` calls. A style update during active composition is never
  converted into a text replace; host callbacks stay memory-safe. Backends
  validate adapter capability before committing to the native side.

## Exercises

### A — high-level component (FAST)

```rust
ui.button("Send")
    .variant(ButtonVariant::Primary)
    .size(ControlSize::Md)
    .icon(Icon::ArrowUp)
    .on_press(|| Msg::Send);
```

### B — custom action from public primitives (CUSTOM)

A "Build succeeded — Open build log" banner built only from `ui.action`,
`BoxStyle`/`ActionStyle`, `Row`, `ui.icon` and `ui.text` — no `CustomRender`,
no backend handle. The app supplies `succeeded` from its own state; runtime
hover/pressed state is not app data. The hover patch touches only the box
background — text is explicit on `ui.text` because nothing inherits.

```rust
enum Msg {
    OpenBuildLog,
}

fn build_banner(ui: &mut Ui<Msg>, succeeded: bool) {
    let style = ActionStyle::new(
        BoxStyle::new()
            .background(Color::role(ColorRole::Muted))
            .border(Border::all(BorderSide::new(
                dp(1.0),
                Color::role(ColorRole::Border),
            )))
            .radii(CornerRadii {
                top_left: dp(6.0),
                top_right: dp(6.0),
                ..Default::default()
            })
            .padding(Insets {
                left: dp(12.0),
                right: dp(12.0),
                ..Default::default()
            }),
    )
    .hover(BoxStylePatch {
        background: Some(Color::role(ColorRole::Background)),
        ..Default::default()
    });

    ui.group("build-banner", |ui| {
        if succeeded {
            ui.action(Action::new().label("Build succeeded; open build log").style(style), |ui| {
                ui.row(Row::new().gap(Space::Sm).align(Align::Center), |ui| {
                    ui.icon(Icon::ArrowUp).color_role(ColorRole::Foreground);
                    ui.text("Build succeeded — open build log")
                        .style(TextStyle::new().foreground(Color::role(ColorRole::Foreground)));
                });
            })
            .on_press(|| Msg::OpenBuildLog);
        }
    });
}
```

A noninteractive `ui.box_` uses the same style vocabulary without activation:

```rust
ui.box_(
    BoxProps::new().style(
        BoxStyle::new()
            .background(Color::role(ColorRole::Muted))
            .padding(Insets::all(dp(12.0))),
    ),
    |ui| {
        ui.text("Read-only summary");
    },
);
```

### C — surgical override (SURGICAL)

```rust
ui.button("Send")
    .variant(ButtonVariant::Primary)
    .style(
        ButtonStylePatch::new()
            .border_bottom_width(dp(2.0))
            .border_bottom_color(Color::rgb(255, 0, 0)),
    )
    .on_press(|| Msg::Send);
```

Only the two named fields change; every unrelated authored/resolved style
and layout-input property in every state stays identical before OS
enforcement — the regression invariant above (derived geometry may shift
with the new border extent).

## Required Windows spike tests (planned — none executed)

Each is a concrete test to be written during the Windows spike; golden
resolved-style fixtures are recorded when an implementation exists.

1. **Golden recipe resolution.** Fixed light and dark token fixtures; for
   each `ButtonVariant` and `ControlSize`, record the full expected concrete
   resolved style — every field — with expected values derived independently
   from the written recipe specification, never captured output accepted
   blindly (no fabricated goldens are needed in this document).
2. **Primitive-only action.** A consumer action built solely from public
   prelude items emits the identical `Msg` on pointer release, Enter/Space
   and accessibility Invoke; disabled emits nothing; Tab moves focus only.
   No private painter or backend imports.
3. **Mandatory regression invariant.** Resolve an unpatched `Primary` button
   and the bottom-only patched one in normal, hover, pressed, disabled,
   focus-visible and the hover+focus, pressed+hover and pressed+focus
   combinations: before OS enforcement only `border.bottom.width`/`color`
   differ; compare every unrelated field — other three borders, background,
   foreground, radii, padding, text size/weight, layout inputs, motion and
   focus configuration — while derived geometry may legitimately shift with
   the new border extent.
4. **Per-side geometry.** Each border side paints independently; per-corner
   radii fields are preserved through resolution; asymmetric `Insets` apply
   per side; the proportional radii clamp produces deterministic final
   geometry.
5. **Role vs rgba equality.** A role and a `Color::rgba` that resolve to the
   same concrete value produce no new damage; a theme revision that changes
   the role's concrete value repaints only while the explicit rgba path is
   unchanged.
6. **Exclusive states and merge.** `disabled > pressed > hover` exclusivity
   plus independent `focus_visible`; partial state patches layer over base;
   repeated `.style(..)` merges field-wise; `None` preserves and explicit
   zero clears.
7. **Damage classes.** A same-value patch produces no layout/paint/peer/a11y
   update; a color-only border patch is paint-only; a width patch is
   layout+paint; neither triggers unrelated accessibility work.
8. **Enforcement last.** High-contrast/focus-visibility enforcement overrides
   arbitrary consumer colors (e.g. the custom red) while required focus and
   disabled semantics survive.
9. **Native text safety.** Chrome/foreground updates during active IME keep
   peer generation, undo, selection and marked text with no `set_text`;
   representable-but-invalid inputs — an rgba alpha<255 backing or
   foreground, chrome geometry too small to host the peer, a malformed
   numeric style, or actionable/native nesting inside `ui.action` — are
   rejected before any native mutation. Group opacity, masks and a general
   `VisualStylePatch` on native text are excluded by the typed API itself,
   demonstrated as compile-fail capability examples rather than runtime
   rejections.
10. **Motion settle.** A repeated unchanged view/style target never restarts
    a transition or emits ticks after settle; the existing zero-idle-loop
    guarantee still holds.
11. **Patch removal.** Apply the red bottom patch for one view pass, then
    omit it on the next pass on the same keyed node: the resolved style
    returns to recipe defaults for those fields and retained node/peer
    identity is unchanged — patches carry no history across view passes.

These styling gates augment the existing spike — they replace nothing in the
native/identity/task/cancellation/resource baseline.

## Ergonomics and inspectability

The model stays agent- and human-friendly without new machinery: reusable
custom controls are ordinary helper functions over `Ui<M>`; styles are finite
named property/state structs rather than selector strings; one recipe source
of truth (named Rust tables) covers built-ins and consumers; a patch never
expands the event or state model — styling is data in `view`. The added type
vocabulary (`*Patch`, `StateStyles`, `Action`) exists to make custom
authoring typed; an agent can grep `ButtonStylePatch.border_*`, follow one
fixed resolution order, and read non-sensitive diagnostics when something is
invalid. No inspector, selector engine or recipe-registration system is
required. No source-size or token-efficiency claims are made here — the
A–F counter metrics in [ARCHITECTURE_OPTIONS.md](ARCHITECTURE_OPTIONS.md)
are unchanged and unrelated to this vocabulary.

## Recommended next step

After owner approval: the Windows-only phase of the Native Composer Contract
Spike, beginning with pure resolver/patch regression tests and one opaque
native Windows proof window containing a baseline and surgically patched
`Primary` button, a custom `ui.action` built from public primitives, and one
native `TextArea`. The proof window is a starter harness, not phase
completion; the pre-existing native/DPI/IME/UIA/identity/task/cancellation/
resource and transparent-borderless gates remain required. macOS validation
is a later, separately approved phase — no platform work starts now.

## Decisions for the owner

- Approve the v0.1 property set and the deferred list above (opacity and
  shadow are deferred, not rejected — they need offscreen/ink-bounds design
  before re-review).
- `TextSize::{Body, Exact(Dp)}` is recorded as a *recommended* shape —
  accepted as part of overall API approval, not an unresolved alternative:
  it makes the theme-resolved default explicit instead of storing a fake
  literal in a parameterless constructor.
- Everything else is captured by the usual approval set: architecture,
  Windows spike, minimum Windows/RichEdit version, and real UIA/DPI/IME
  capability on actual machines.
