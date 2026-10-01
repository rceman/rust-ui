# Style Customization Model

Status: architecture concept only — proposed API exercise, not implemented,
not type-checked, and not finally approved by the owner. This document is
the single authoritative styling definition;
[LAYOUT_STYLE_MOTION.md](LAYOUT_STYLE_MOTION.md)
keeps layout, motion and theme mechanics and links here rather than restating
style rules.

Written on branch `agent/style-customization-model-review` against the
committed architecture at `768eb172509a45ed15cdf5156fe57268ccaf8b08`.

Optional-frontend correction against `0bf0add83fa3a7e4052d058414b55d458deb2f8d`:
the independent review's **APPROVE WITH ARCHITECTURE CHANGES** is addressed
by the reserved boundary below. It supersedes earlier blanket CSS exclusions
for future optional authoring, not the original CSS-free v0.1 implementation
scope. FAST/CUSTOM/SURGICAL and the basic outer-shadow correction remain
accepted; no frontend or integration machinery is implemented by this review.

## Three workflows, one renderer

The review keeps the owner's three first-class workflows — they are three
ways of authoring UI over one paint/style pipeline, not three renderers.
All style/theme/component semantics on this page are platform-neutral —
each backend renders the same recipes through its own graphics stack
(`docs/PLATFORM_CONTRACTS.md`); portability means identical logical sizes
and state behavior, not identical glyph rasterization.

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
layout props, kept separate so visual style never overwrites size. A future
frontend may separately project declarations into typed layout inputs; it
never moves dimensions into visual-style patches. The same
`.width`/`.height` (`Length`) and `.min_*`/`.max_*` (`Dp`) setter vocabulary
exists on every node and container builder — rows, columns, `ui.action`,
built-ins and native inputs — so size is uniformly layout, not style. `Row`
gap/alignment and `Length` stay layout; visual styling moves none of them.

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
`Default`; validation is field-specific, staged as diagnostics and never
constructor panics: shadow `offset_x`/`offset_y` are finite and *signed*;
`blur_sigma`, border widths, radii, padding and layout extents are finite
non-negative; text size is positive.

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

#[derive(Copy, Clone, PartialEq)]
pub struct Shadow {
    pub color: Color,
    pub offset_x: Dp,
    pub offset_y: Dp,
    pub blur_sigma: Dp,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct BoxStyle {
    pub background: Color,
    pub border: Border,
    pub radii: CornerRadii,
    pub padding: Insets,
    pub shadow: Option<Shadow>,
}
impl BoxStyle {
    pub fn new() -> Self;
    pub fn background(self, color: Color) -> Self;
    pub fn border(self, border: Border) -> Self;
    pub fn radii(self, radii: CornerRadii) -> Self;
    pub fn padding(self, padding: Insets) -> Self;
    pub fn shadow(self, shadow: Option<Shadow>) -> Self;
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

Defaults: `BoxStyle::new()`/`BoxStyle::default()` is transparent background,
zero-width borders, zero radii, zero padding, `shadow: None`.
`TextStyle::new()`/`TextStyle::default()` is `ColorRole::Foreground`,
`Normal` weight, and `TextSize::Body` — a *token*
that resolves to the theme base text size adjusted for OS text scaling, not
a literal `Dp` baked into a parameterless constructor. `TextSize` is the
recommended shape: it makes the theme-resolved default explicit instead of
faking a constant. `TextSize::Exact(dp)` requests an explicit logical size —
`Exact` still receives OS text scaling like everything else; it is a
requested unscaled dp value, not a bypass. Text sizes must be positive
finite (stricter than the non-negative rule for padding/borders). System
font is fixed for v0.1: no public font-family or string font loading, no
text decorations, no letter spacing, no arbitrary core inheritance. `.wrap` stays
a text behavior prop. `Space`/`Radius` tokens remain for recipes and layout
and expand to concrete `Insets`/`CornerRadii` during resolution — likewise
`Surface::padding(Space::Md)` is a recipe convenience that populates the
same `BoxStyle.padding` before any consumer patch, not a second padding.

`Shadow` is v0.1's *only* effect primitive — one optional outer box shadow
per box, shared by `BoxStyle` (and thus `Surface`, `Button`, `Action`)
through the same style path. `blur_sigma` is deliberately named: a Gaussian
standard deviation in logical dp, not a CSS blur-radius. `offset_x`/`offset_y`
are finite and *signed* (negative is legitimate); `blur_sigma` is finite
non-negative, and `dp(0.0)` is a sharp offset silhouette. Alpha comes only
from `Shadow.color` — a new semantic `ColorRole::Shadow`, or an explicit
rgba. `Shadow` has no constructors; examples use named struct fields. There
is no spread field, no inset, no shadow list, no filter/effects API — the
full contract is in [Box shadow (v0.1)](#outer-box-shadow-v01) below.

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
pub enum ShadowPatch {
    #[default]
    Unchanged,
    Set(Shadow),
    Remove,
}

#[derive(Copy, Clone, Default, PartialEq)]
pub struct BoxStylePatch {
    pub background: Option<Color>,
    pub border: BorderPatch,
    pub radii: CornerRadiiPatch,
    pub padding: InsetsPatch,
    pub shadow: ShadowPatch,
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
- `shadow` is the one deliberate exception to recursive field merging:
  `ShadowPatch` is an *atomic* optional-property command, not a nested
  `Option` — `Set(Shadow)` supplies a complete descriptor and changes only
  shadow, `Remove` forces none, `Unchanged` preserves the recipe/earlier
  current-build value. There is no per-parameter partial shadow patch
  (modifying a shadow that may not exist is undefined); a different shadow
  means a complete explicit descriptor. Within one build's merge chain, a
  later `Set`/`Remove` wins and `Unchanged` never erases earlier values;
  the next view pass still reconstructs from the declaration — no history.
  This is optional-property set/remove, not `BoxStyle` replacement and no
  generic effect-patch trait.
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
`ui.action` declare their own explicit styles; core never implicitly inherits
styles down.
A built-in `Button` resolves one `VisualStyle` and binds its own recipe's
text and icon parts to that resolved text style — a closed recipe-part
mapping, not ancestor inheritance.

Ordered source composition — "why did this property get this value?" has
one deterministic answer. The stylesheet slot is **reserved for a future
optional frontend**, absent in v0.1; without it the existing Rust resolution
order and outputs are unchanged:

```text
theme tokens
  -> component defaults
  -> variant
  -> size
  -> recipe interaction/focus overlays
  -> effective stylesheet patch for CURRENT runtime state (future optional)
  -> inline Rust BASE partial patch
  -> inline Rust ACTIVE/focus_visible patch
  -> accessibility / OS enforcement
```

Specificity/cascade completes inside the optional frontend before its patch
enters this slot. Inline Rust is stronger host policy, not a browser style
attribute; CSS `!important`, if supported later, cannot jump over that host
layer or accessibility/OS enforcement. No specificity enters the core resolver.

- The Rust recipe/inline interaction branch is exclusive: `disabled`, else
  `pressed`, else `hover`, else `normal` — a priority chain, never "hover+pressed"
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
  radii, padding, shadow, size, motion and focus configuration — value-
  identical in *each* supported state. Derived geometry and measurement
  *may* change (the new border width consumes content insets), and
  OS-enforced colors may legitimately differ — the invariant covers inputs
  before enforcement, not pixel-identical output.
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
  system in core or v0.1. Motion stays `MotionToken` + the earliest-deadline
  scheduler; no style animation selectors or loops.

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
| shadow | **v0.1 basic** — one optional outer `Shadow` per `BoxStyle`; spread, inset and shadow lists stay deferred |

CSS parser, selectors, specificity and cascade are **rejected as core
dependencies**, **out of scope/deferred for v0.1 implementation**, and
**architecturally supported in a future optional authoring frontend**. That
frontend lowers to the same typed inputs; it does not make the core a CSS
engine. The reserved boundary is defined below.

Genuinely rejected from core: DOM dependency, browser layout model, a general
browser Flexbox/Grid requirement, percentage/browser-relative layout
semantics, renderer dependence on CSS, string/dynamic property bags,
arbitrary implicit inheritance and pseudo-selector strings in core APIs.
Future native layout additions require separate review, not browser parity.

Deferred native visual properties: gradients, filters, transforms, blend
modes, complex border dashes/images, general font-family/variable-font
loading, authored transitions, shadow spread/inset/shadow lists. Their
appearance in CSS syntax does not approve them for core.

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
- Validation is field-specific: finite signed shadow offsets; finite
  non-negative `blur_sigma`, border widths, radii, padding and layout
  extents; positive text size; `min <= max`. Malformed numeric
  styles produce `UiDiagnostic::InvalidStyle` before any native mutation;
  forbidden interactive nesting produces `UiDiagnostic::InvalidComposition`;
  an impossible platform capability produces `UiError::Unsupported`. Nothing
  is silently ignored or clamped except the declared radii normalization.
- Alpha fills over painted content are permitted; alpha on live text and
  ancestor group opacity are not.

## Outer box shadow (v0.1)

**Correction provenance.** The earlier review deferred all shadows; that
deferral was too broad for elevated surfaces and for public-primitive
equivalence. On re-review, committed Mascot source at tracking snapshot
`14e576ff1e4270c59a271143827940efbb599395` (inspected read-only via
`git show`, no checkout) — `crates/mascot-ui-win32/src/paint.rs` — confirms a
*cached* native shadow path exists: a `Painter` `Option<ShadowCache>` keyed
on window-px/scale/bubble-geometry/radius/theme, early-returning on an equal
key, rasterizing a rounded white silhouette through `CLSID_D2D1Shadow` and
drawing it beneath the bubble. That shows the pattern is implementable; it
is not a fresh runtime or performance validation, nothing is ported or
copied, and this recommendation is independent of that implementation.

Contract:

- Exactly one optional outer shadow per box — the four-field `Shadow`
  descriptor above — shared by `BoxStyle` consumers (`box_`, `Surface`,
  `Button`, `Action`) through the existing style path. No shadow lists, no
  inset, no spread, no filter/effects API, no CSS runtime semantics in core.
  Spread is deferred deliberately: ordinary subtle Surface/Card elevation
  needs only silhouette translation + blur; spread adds shape dilation/erosion, the
  negative-spread/corner-radius interaction and extra edge behavior for no
  current basic requirement — a separate future review may revisit it, as
  with inset and lists. Group/node opacity stays deferred unchanged.
- The default elevated `Surface` recipe proposes
  `Some(Shadow { color: Color::role(ColorRole::Shadow), offset_x: dp(0.0), offset_y: dp(1.0), blur_sigma: dp(2.0) })`,
  with `Theme::light` `Shadow` = `Color::rgba(0, 0, 0, 32)` and
  `Theme::dark` = `Color::rgba(0, 0, 0, 64)` — proposed recipe choices, not
  measured values or copied semantics. `ui.box_` and `Button` default flat;
  a flat surface is
  `Surface::new().style(BoxStylePatch { shadow: ShadowPatch::Remove, ..Default::default() })`,
  and `Surface::padding`/existing props are unaffected. Forced-colors/
  high-contrast resolves a decorative shadow to `None` *after* patches; a
  shadow is never the sole focus or status indication.
- Source silhouette: the node's own opaque outer rounded box using the
  already-normalized corner geometry — never captured children, text or
  native-peer pixels. Fill alpha does not scale the cast silhouette; only
  `Shadow.color` alpha does.
- Paint order: outer-only — the shadow is clipped out of the original
  rounded-box interior (so no inset effect, and a contained opaque native
  peer is never under shadow alpha) and painted before the box's own
  fill/border/children in existing declaration order. No new z-index.
- Layout isolation: a shadow never contributes to desired size, layout or
  content insets, hit-testing, focus, accessibility or window autosizing.
  It may extend outside its own rect but respects inherited clip and the
  window client clip — never its own content rectangle, which would erase
  it; a caller wanting the full halo supplies room.
- Finite bound: at resolved final box `B` and positive finite DPI scale `s`,
  in device pixels `B_s = scale(B, s)`, `r_px = ceil(3 * blur_sigma * s)`,
  `o_px = (offset_x * s, offset_y * s)`, and the shadow extent is
  `S_px = inflate(translate(B_s, o_px), r_px)` — a mandated finite 3σ
  truncated Gaussian plus one physical-pixel AA/sampling guard. Backends
  crop to this support; nothing here asserts an infinite Gaussian has
  finite support or that backend defaults already match it.
- Damage: node paint ink includes its own box + `S_px` + existing
  focus/descendant ink. The exact recipe on change/removal/move: retain the
  old effective device-pixel bound *with its old clipping*; compute the
  guarded outward old ink intersected with the *old* inherited clip and the
  guarded outward new ink intersected with the *new* inherited clip
  separately; union those two regions, then intersect with the current
  client region. Old ink is never re-clipped by a new ancestor clip — at
  unchanged DPI this clears prior pixels on move/remove/clip change. On a
  DPI/render-target recreation (a different coordinate generation), the
  whole current client is invalidated once and the affected raster cache
  invalidated, rather than mixing rectangles from incompatible generations —
  genuine external invalidation, not a shadow layout mutation or an idle
  loop. No stale halos.
- Validation: malformed numerics — non-finite signed offsets, non-finite or
  negative `blur_sigma`, a zero/negative/non-finite DPI scale, or overflow
  in computed extents/allocation sizes — are
  `UiDiagnostic::InvalidStyle` before native mutation; a valid but
  unallocatable raster/cache target returns the existing typed platform
  error — no silent smaller blur. `alpha = 0` normalizes to no resolved
  shadow: transitions between zero-alpha authored descriptors produce
  neither allocation nor repaint, while the authored prop still updates.
- Equality/cache: a private `ResolvedShadow` (concrete color + dp) joins
  `ResolvedBoxStyle`; comparison covers presence, color, signed offsets and
  sigma, while the normalized radii/box geometry feed the draw bounds and
  raster silhouette. A shadow-only descriptor change is paint+damage only —
  no layout, no `TextPeer.apply`, no a11y/focus mutation; shape/size changes
  reach the silhouette through the existing layout causes. An unchanged
  resolved shadow produces no repaint, raster rebuild or scheduled work,
  and never restarts unrelated motion.
- Backends may keep a dedicated bounded silhouette/blur raster cache keyed
  on snapped silhouette size + normalized radii + `blur_sigma` + concrete
  color + DPI + render-target/device generation (plus sampling phase where
  needed). Offset/location are draw placement: translation *reuses* the
  local raster while snapped silhouette and the applicable sampling-phase
  key stay unchanged — a fractional-phase or geometry change may invalidate
  that key, which is genuine invalidation work, not a no-op traversal.
  Caches budget by bytes, evict
  safely, and reclaim on unmount/`Remove`/device loss; at most one shadow
  entry per node plus explicitly budgeted renderer sharing — no
  ever-growing map. Device-loss/OS-expose invalidation is genuine re-render
  work, distinct from no-op re-traversal. This is a fixed-size primitive,
  not a generic effects graph; no zero-allocation or benchmark promise —
  native Windows validates cache behavior in the spike.
- No shadow animation: descriptor changes apply statically on commit — no
  frame or timer demand; unrelated motion continues independently.

Planned bound oracle (arithmetic test expectations, not measurements):
`B = (x10, y20, w100, h40)`, offsets `(-3, +4)`, `blur_sigma = 2`. At
`s = 1`: `r = 6`, `S_px = (left 1, top 18, right 113, bottom 70)`; the
old/new ink union + 1 px guard yields `(0, 17, 114, 71)` before tighter
clip. At `s = 1.25`: `r = 8`, `S_px = (0.75, 22, 141.75, 88)`; the guarded
union is `(-1, 21, 143, 89)`, and a client clip at `left = 0` yields
`(0, 21, 143, 89)`. Planned edge tests cover signed offsets, zero sigma,
old+new halo unions on move/remove/unmount, ancestor/client clipping, radii
and DPI variation — all planned, none executed.

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
  | background / border color / text foreground / shadow descriptor | paint + finite damage only |
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
- One retained arena — no extra virtual tree or required subscriptions in
  Rust-only core. Closure factories and app traversal may allocate as before;
  style resolution aims for allocation-free fixed-size work, which the spike
  measures rather than promises.

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
- Chrome `shadow` is painted chrome only — never a peer theme/editor style,
  and it never blurs, clips or composites live editing pixels. The outer-
  only interior exclusion above keeps the contained rectangular peer
  untouched; safety-inset and opaque-backing/foreground rules are exactly
  unchanged — a shadow is ignored by minimum host-size and safety
  calculations and a shadow-only change produces no layout or
  `TextPeer.apply`. Shadow ink joins the existing native-island overlap
  validation: it may not bleed over an unrelated native peer's editing
  rectangle — keep such peers outside the shadow footprint or diagnose
  `UiError::Unsupported` before native mutation; full bounds clipping is
  not an excuse for arbitrary peer overlap. The shadow silhouette comes
  from the node's own authored geometry, never from stroking or capturing
  the editor.
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

A shadow works the same way — the elevated `Surface` default, an identical
public-primitive descriptor, and an explicit `Remove` for a flat surface:

```rust
ui.surface(Surface::new().padding(Space::Md), |ui| {
    ui.label("Elevated by default");
});

ui.box_(
    BoxProps::new().style(BoxStyle::new().shadow(Some(Shadow {
        color: Color::role(ColorRole::Shadow),
        offset_x: dp(0.0),
        offset_y: dp(1.0),
        blur_sigma: dp(2.0),
    }))),
    |ui| {
        ui.text("Same shadow vocabulary on a primitive");
    },
);

ui.surface(Surface::new().style(BoxStylePatch {
    shadow: ShadowPatch::Set(Shadow {
        color: Color::rgba(0, 0, 0, 48),
        offset_x: dp(-2.0),
        offset_y: dp(3.0),
        blur_sigma: dp(3.0),
    }),
    ..Default::default()
}), |ui| {
    ui.label("Signed offsets and a custom shadow color");
});

ui.surface(Surface::new().style(BoxStylePatch {
    shadow: ShadowPatch::Remove,
    ..Default::default()
}), |ui| {
    ui.label("Flat");
});
```

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
   foreground, radii, padding, shadow, text size/weight, layout inputs,
   motion and focus configuration — while derived geometry may legitimately
   shift with the new border extent.
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
12. **Shadow recipe defaults.** The default `Surface` resolves the exact
    descriptor in the shadow section (`Shadow` role, `(0,1)` offset,
    `sigma = 2`) with the proposed concrete light `rgba(0,0,0,32)` and dark
    `rgba(0,0,0,64)` role values; a flat `BoxStyle`/`Button` resolves
    `None`; forced-colors/high-contrast resolves the shadow to `None` after
    patches.
13. **Primitive parity.** A public-primitive `ui.box_`/`ui.action` using the
    same `Shadow` descriptor produces the same `ResolvedShadow` and the same
    shadow render operation/ink boundary as a built-in surface under
    fixtures with identical box bounds, normalized radii, DPI and clip —
    unrelated box props may differ; no private painter access anywhere.
14. **Atomic patch semantics.** `Set`/`Remove` change only the optional
    shadow; before OS enforcement every unrelated property, recipe, state,
    layout and motion value is identical; `Unchanged` merges preserving an
    earlier `Set`/`Remove` within one build; omitting the patch next pass
    restores the recipe value. (High-contrast suppression of the shadow is
    checked separately in group 12.)
15. **Shadow-only damage.** A changed descriptor produces paint + finite
    damage only — zero layout, text measurement, native `apply`, unrelated
    a11y or focus change.
16. **Bound oracle.** The numerical bound above: signed positive/negative
    offsets, `sigma = 0`, old+new halo unions on move/remove/unmount and on
    an *ancestor clip change* (old ink cleared under its old clip), client/
    ancestor clipping, radii and DPI variation — no pixels beyond the
    mandated finite bound. Also malformed-input coverage: non-finite signed
    offsets, non-finite or negative `blur_sigma`, and zero/negative/
    non-finite DPI scale are rejected as `InvalidStyle` before mutation.
17. **No-op and cache.** An unchanged resolved shadow produces no repaint,
    raster rebuild, timer, frame or motion restart; cache-key coverage plus
    device-loss invalidation and bounded byte-cache reclamation. Included
    edge cases — all still *only-shadow* changes, so unrelated paint updates
    aren't forbidden: an `alpha = 0` descriptor normalizing to no resolved
    shadow without repaint, and the native-peer overlap rule — shadow ink
    bleeding onto an unrelated native editing rectangle is diagnosed before
    native mutation rather than silently clipped; the `TextPeer`
    generation/rect/undo/IME stays unchanged through any chrome shadow
    update.

All shadow gates are planned, not executed; render/pixel-containment checks
must exercise real native output in the spike, not merely reassert the
formula. These styling gates augment the existing spike — they replace
nothing in the native/identity/task/cancellation/resource baseline.

## Ergonomics and inspectability

The Rust model stays agent- and human-friendly without new machinery: reusable
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

## Future optional authoring frontends (reserved)

### Core ownership and canonical typed inputs

**Architecture requirement:** rust-ui core owns the typed deterministic
styling model. Rust builders are its first authoring frontend. Future
optional CSS, generated or design-tool frontends may parse/compile to the
same typed style and layout inputs without replacing components, layout
algorithms, the retained arena, the resolved-style family, renderer, damage
rules or native-peer contracts. Additive integration interfaces may be needed;
this is not a promise that no future core/API extension is necessary.

Frontends are orthogonal to FAST/CUSTOM/SURGICAL, not a fourth UI layer:

```text
Rust builders / future CSS / future generated authoring
                         |
               FAST / CUSTOM / SURGICAL
                         |
             same typed style/layout inputs
                         |
           core ordered composition / validation
                         |
          same Resolved* family + typed layout inputs
                         |
                    layout / paint
```

`BoxStyle`, `TextStyle`, `VisualStyle`, their existing patch family
(including `ButtonStylePatch`/`TextInputStylePatch`), `StateStyles`,
`Shadow`/`ShadowPatch` and private `ResolvedBoxStyle`/`ResolvedTextStyle`/
`ResolvedVisualStyle`/`ResolvedShadow` remain the canonical core vocabulary
for the supported subset. No generic `StyleProperty`, `StyleValue`,
`StyleRule` or dynamic property-bag IR is introduced. CSS may have its own
AST, selectors, cascade data, variable environment and declaration ordering;
those are frontend authoring machinery, not another core representation.

Visual patches do not contain width/height/min/max. A future frontend may
need a typed projection into the existing separate layout inputs, using the
same validation and layout algorithms. No projection API is designed here;
no browser sizing or box-sizing equivalence is assumed.

### Ordered contributions and interaction state

The reserved stylesheet slot in the resolution path above receives one
effective typed patch for the **current** runtime state. The frontend
finishes all supported selector/state matching and cascade first. It must
not map CSS declarations directly into Rust's exclusive hover/pressed slots:
`button:hover { color: red; }` and `button:active { background: blue; }` can
both contribute while hovered and pressed. Rust `StateStyles` and its
exclusive branch priority plus orthogonal focus overlay are unchanged.

Predicate sources remain authoritative runtime state: `:hover` uses hover,
`:active` pressed/activation, `:disabled` disabled, and `:focus-visible`
focus-visible. The frontend owns no duplicate interaction-state tracking.
Keyboard/capture, ancestor predicates and native disabled/focus behavior need
explicit profile semantics; native policy wins and browser equivalence is
not automatic. State changes that bypass app `view` must also notify the
future opt-in style integration, without requiring app messages for hover.

Primitive defaults untouched by the author live below stylesheet input.
An explicitly supplied full `BoxStyle`/`TextStyle` is a complete inline
authored value and overrides corresponding stylesheet fields; full primitive
styles remain replacements, not sparse patches. This also applies to an
explicit full `ActionStyle` base. Do not infer explicitness by comparing
values with defaults: future source composition must distinguish absence
from an explicit full value. A partial-inline primitive API may be considered
later but is not required or specified now.

### Optional dependency boundary

```text
optional development watcher -> optional CSS frontend -> rust-ui core
```

| Owner | Responsibilities |
|---|---|
| Core | typed styles/layout inputs, recipes, ordered resolution, validation, concrete resolved values, layout, paint, damage, native peers |
| Optional CSS frontend | parsing, selectors, specificity/cascade, shorthand expansion, units, variables, declaration ordering, compiled matching, typed lowering |
| Optional dev watcher | filesystem notifications, debounce/coalescing, compile requests, diagnostics |

No crates, parser, matcher, watcher, metadata or integration APIs are created
now. Core never depends on these frontends; neither renderer nor platform
adapter interprets CSS. Future restricted style-target capability, ancestry
and state access may be necessary, but frontend code cannot bypass validation
by mutating the arena or injecting final `Resolved*` values directly.

### Build-time and recoverable runtime authoring

A future compiler may emit typed/generated/static rust-ui data. For explicit
or static bindings, production may have no parser, source CSS, runtime
CSS-value string parsing, filesystem access or general selector engine.
Const-capable construction or one-time generated typed initialization is a
future detail, not a new data representation. General selectors over dynamic
nodes/classes/ancestry/states may still require compiled runtime matching;
build-time parsing is **not** a promise of zero runtime authoring work.

A future external stylesheet update follows this reserved transaction:

```text
parse/lower candidate -> validate candidate
 -> UI-thread matching/resolution staging
 -> layout/native-capability validation
 -> atomic valid stylesheet + style-target publication
 -> ordinary concrete-equality-driven damage
```

Parsing, lowering or candidate-validation failure keeps the previous valid
stylesheet and current UI, reporting recoverable diagnostics. It is distinct
from the fatal programmer-invalid Rust build transaction. No recoverable
update machinery is implemented in v0.1. Atomicity describes logical
publication after preflight, not guaranteed rollback of every OS/device or
resource failure after side effects; existing typed platform failures remain.

Stylesheet revisions fence stale compilation completion; generational node
identity fences stale target updates. Workers handle CPU data, never native
handles; matching uses current UI/state, not a stale worker snapshot.
Filesystem events/coalesced wakeups replace polling redraw loops. Compare
concrete resolved targets, not just stylesheet revisions: unchanged targets
never restart motion. Changed targets follow existing motion/reduced-motion
policy, and shadow parameters remain static as specified above.

### Selector metadata semantics only

No `.id()`/`.class()` APIs, metadata storage or indexes are added to v0.1
solely for CSS. Reserved invariants:

- Style IDs/classes are not reconciliation keys; changing them never remounts
  a node or changes its generation.
- Matching uses author-facing component/primitive identity, not private
  renderer storage types. Private recipe decomposition is not automatically
  selector-addressable.
- Logical styling ancestry is separate from message/key scaffolding.
- Future metadata/indexes may be opt-in generation-keyed sidecars over the
  same arena, never a second retained style/virtual tree.

Exact APIs, naming and selector vocabulary remain deferred.

### Constrained future CSS profile, units and themes

CSS syntax compatibility is not browser layout/rendering compatibility.
Representable authoring may include these declarations:

```css
background: #171717;
color: white;
border-bottom-width: 2px;
border-bottom-color: red;
border-radius: 8px;
padding: 8px 12px;
box-shadow: 0 4px 8px rgba(0,0,0,.15);
```

This is a profile reservation, not a CSS conformance specification or v0.1
feature. Supported shorthand expansion and declaration order must be correct;
unsupported declarations/forms fail deterministically rather than silently
approximating browser behavior. Initially exclude/defer browser Flexbox/Grid,
percentage sizing, generated content, arbitrary stacking contexts, transforms,
filters, group opacity, unsupported paint effects, elliptical/percentage
radii, general CSS animations and unsupported native text styling.
`display: none` is not `Visibility::Hidden`: the latter retains layout space
and has existing focus/composition semantics. Inheritance, `currentColor`,
`initial`/`unset`/`revert` require deliberate frontend semantics, not implicit
core inheritance or cascade.

Reserve `px -> logical Dp` and unitless zero where valid. Core retains DPI
snapping and accessibility text scaling; lowering must not double-scale.
Defer `rem`/`em` until reference metrics and scaling are specified; reject
`%`/`vw`/`vh` from the initial profile. No browser-relative layout algorithms.

No core CSS-variable system is needed. The frontend owns substitution,
scope, fallbacks, cycle detection and dependency tracking. An intentionally
imported `--foreground` binding may lower `color: var(--foreground)` to
`Color::role(ColorRole::Foreground)`, preserving role resolution in core.
Explicit CSS colors lower to explicit typed `Color`, with the existing
channel representation. Generic custom properties do not silently rewrite
core theme roles; actual theme customization needs explicit typed integration.

CSS `box-shadow` blur-radius is **not** native `blur_sigma`: for the proposed
profile, `sigma = blur-radius / 2` ([CSS shadow blur reference](https://www.w3.org/TR/css-backgrounds-3/#shadow-blur)).
With `px -> Dp`, the example's offsets are `(0dp, 4dp)` and its sigma is
`4dp`, not `8dp`. Accept initially one outer shadow with omitted or zero
spread; reject nonzero spread, inset and multiple shadows. `none` may lower
to `ShadowPatch::Remove`; an absent declaration remains untouched. The
native finite-support bounds, cache/equality and peer limits are unchanged;
browser pixel equivalence is not promised.

### Retention, inspectability and Rust-only cost

Retain authored/base inputs separately from stylesheet and inline
contributions. Optional stylesheet dependencies/revisions exist only in the
enabled integration. Removing a contribution recomputes from source layers,
not yesterday's resolved result; ancestry/class/scoped-variable changes may
require descendant re-resolution. Concrete `Resolved*` equality and old/new
ink bounds/clipping remain authoritative. A revision alone causes no layout
or repaint when concrete output is unchanged. Targeted damage does not
promise sublinear reload, matching or layout. Reject features requiring
unbounded style/layout fixed-point iteration.

All frontends lower native editing through the capability-limited
`TextInputStylePatch` path: opaque backing/foreground, rectangular islands,
system editing/accessibility ownership and peer identity/generation are
unchanged. Styles do not become text replacement or remounting; unsafe
candidate geometry, colors or overlap are rejected before native mutation.

CSS adds nonlocal selector/cascade reasoning, so local grepability is not
identical to Rust builders. Reserve optional/on-demand frontend provenance:
stylesheet revision, winning rule/declaration IDs, source span, matched
target/state, resolution layer and subsequent OS/accessibility enforcement.
Source maps remain frontend-owned; do not attach full provenance strings or
histories to every production node. Readable/named generated data helps
inspection; no token-efficiency or performance result is claimed.

Rust-only builds retain essentially the current cost model **by design**:
no parser, matcher, watcher, CSS AST, variable environment, selector
metadata/index allocation, dynamic property map, mandatory per-property
provenance or virtual dispatch solely for future CSS. Optional dependencies
and integration storage/work must be absent when unused, not mandatory
per-node overhead. CSS-enabled matching has its own costs; no zero-cost
matching promise or benchmark is made.

These are architectural reservations only. Future profile details and exact
interfaces need separate approval; none expands the current Windows spike,
adds CSS tests, or blocks approval of the typed-style/native-control spike.

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

- FAST/CUSTOM/SURGICAL and the basic outer `Shadow` correction are accepted;
  final overall architecture/spike approval remains with the owner. Group/node
  opacity and shadow spread/inset/lists remain deferred.
- The future optional-frontend boundary is now an explicit architecture
  requirement. Exact CSS profile/interfaces and implementation need separate
  approval, not implementation in or a blocker for the current Windows spike.
  The corrected typed-style architecture is ready for Windows spike approval;
  this document does not itself authorize starting that spike.
- `TextSize::{Body, Exact(Dp)}` is recorded as a *recommended* shape —
  accepted as part of overall API approval, not an unresolved alternative:
  it makes the theme-resolved default explicit instead of storing a fake
  literal in a parameterless constructor.
- Everything else is captured by the usual approval set: architecture,
  Windows spike, minimum Windows/RichEdit version, and real UIA/DPI/IME
  capability on actual machines.
