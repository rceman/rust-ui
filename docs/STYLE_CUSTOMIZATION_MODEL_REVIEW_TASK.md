# Style Customization Model Review Task

Status: docs-only architecture review task.

Baseline: `768eb172509a45ed15cdf5156fe57268ccaf8b08`

Branch: `agent/style-customization-model-review`

## Mission

Review and refine the public styling/customization architecture for `rust-ui` before implementation proceeds.

The existing Astra architecture remains the provisional source of truth for the programming model, retained tree, native-control boundary, layout, motion, async model, and API shape.

This review is narrowly about how visual components are constructed and customized.

Do not implement runtime/library code in this task.

## Read first

Read these existing architecture documents in full:

- `docs/ASTRA_ARCHITECTURE_REPORT.md`
- `docs/PROGRAMMING_MODEL.md`
- `docs/LAYOUT_STYLE_MOTION.md`
- `docs/NATIVE_CONTROL_BOUNDARY.md`
- `docs/API_EXAMPLES.md`
- `docs/BACKEND_ARCHITECTURE.md`
- `docs/STYLE_CUSTOMIZATION_MODEL_OWNER_CONTEXT.md`

## Proposed three-layer model

### Layer 1 — High-level components

Normal application code uses components, variants, sizes, theme recipes, and typed interaction behavior.

```rust
ui.button("Send")
    .variant(ButtonVariant::Primary)
    .size(ControlSize::Md)
    .icon(Icon::ArrowUp)
    .on_press(|| Msg::Send);
```

This should cover the large majority of product UI without requiring low-level drawing knowledge.

### Layer 2 — Public low-level primitives

Consumers must also be able to construct bespoke UI using public typed primitives. Built-in components should, where practical, use the same primitive/style vocabulary rather than an inaccessible parallel renderer.

Candidate vocabulary to review:

```text
Box
Text
Icon
Image
CustomRender
NativeText
Row / Column / Stack

Color
BoxStyle
TextStyle
Border / BorderSide
Insets
CornerRadii
Shadow
Length
Align
```

Do not approve this list mechanically. Reduce or reshape it where useful. Native controls such as RichEdit remain special native peers where required.

### Layer 3 — High-level component plus low-level partial override

A consumer may start from a normal component recipe and surgically override selected low-level properties.

```rust
ui.button("Send")
    .variant(ButtonVariant::Primary)
    .style(
        ButtonStylePatch::new()
            .border_bottom_width(dp(2.0))
            .border_bottom_color(Color::rgb(255, 0, 0))
    );
```

The exact API names are not fixed. The semantic requirement is fixed: partial patching, not replacement.

Changing only the bottom-border width/color must preserve every untouched resolved property, including background, foreground, other border sides, radius, padding, size, hover, pressed, disabled, focus, and motion.

## Required design questions

Resolve, or explicitly leave for owner approval:

1. Should the three layers be explicit public concepts?
2. What is the smallest sufficient public primitive vocabulary for v0.1?
3. What is the smallest sufficient low-level box/text style vocabulary?
4. Should built-in components resolve through the same low-level style representation?
5. How should partial component overrides be represented?
6. Should there be generic StylePatch types, component-specific patch types, or both?
7. How should interaction-state overrides work without selectors?
8. Should arbitrary colors be allowed in addition to theme roles?
9. Should borders support independent sides?
10. Should corner radii support independent corners?
11. Should padding support independent sides?
12. Which size/layout properties belong in style versus layout props?
13. How should style resolution avoid unnecessary allocations/copies?
14. How should resolved-style equality interact with retained-node equality and damage detection?
15. Which properties are invalid/constrained for native peers?
16. Does the model materially complicate the Astra programming model?
17. How should built-in recipes remain inspectable and agent-friendly?
18. Which CSS-like properties should be deliberately deferred or rejected?

## Strong constraints

Do not introduce CSS syntax/parser, selectors, specificity, cascade, arbitrary inheritance, string-keyed property bags, DOM concepts, pseudo-selector strings, browser-style layout machinery, or a second hidden styling system.

Target: CSS-like visual expressiveness for desktop UI without becoming a CSS engine.

## Deterministic resolution

Review and formalize one deterministic precedence path, starting from:

```text
theme tokens
    -> component defaults
    -> variant
    -> size
    -> interaction state
    -> consumer partial style patch
    -> accessibility / OS-enforced adjustment
```

There must be no specificity system. Define whether consumer patches apply after state resolution, per state, or through a structured state-patch model.

## Candidate v0.1 properties

Review each item and classify it as v0.1, deferred, or rejected:

```text
background
foreground
border width/color
independent border sides
corner radius
independent corner radii
padding
independent padding sides
width / height
min / max width
min / max height
opacity
shadow
```

Do not add a property just because CSS has it.

## Required API exercises

### A. Pure high-level component

```rust
ui.button("Send")
    .variant(ButtonVariant::Primary)
    .size(ControlSize::Md);
```

### B. Fully custom control

Show realistic consumer-shaped pseudocode for a useful custom visual control built entirely from public primitives, with no private painter/backend access.

### C. Standard component with surgical override

Show the API and resolution semantics for a standard Primary button with only bottom border width = 2 dp and bottom border color = arbitrary custom color. Every unrelated resolved property must remain unchanged.

## Tests required from the later Windows spike

Define concrete implementation tests for:

- high-level component recipe resolution;
- primitive-only custom control construction;
- partial patch semantics;
- untouched properties remaining unchanged;
- per-side borders if approved;
- per-corner radii if approved;
- arbitrary color vs theme-role color;
- interaction-state resolution;
- deterministic precedence;
- equality/damage detection after one-property patch;
- no private painter/backend access needed for ordinary custom UI.

Mandatory regression invariant:

```text
resolve Primary Button
    ↓
apply only:
    border-bottom-width = 2 dp
    border-bottom-color = custom red
    ↓
assert every unrelated resolved property
is value-identical to the unpatched Primary Button
```

## Deliverables

Create `docs/STYLE_CUSTOMIZATION_MODEL.md`.

Update canonical architecture documents where necessary so they do not contradict the reviewed model, especially:

- `docs/LAYOUT_STYLE_MOTION.md`
- `docs/API_EXAMPLES.md`
- `docs/PROGRAMMING_MODEL.md`
- `docs/ASTRA_ARCHITECTURE_REPORT.md`

Do not leave two competing styling specifications.

No implementation code. No Mascot changes. No CI. No macOS/Linux work.

## Final report

Return the recommended three-layer model; approved/deferred/rejected v0.1 properties; proposed public API; internal resolution model; state-style model; color/border/radius/inset decisions; performance and retained-tree implications; native-peer constraints; Windows-spike tests; docs changed; unresolved owner decisions; and the exact recommended next Windows step.

End with exactly:

`RUST_UI_STYLE_MODEL_REVIEW_COMPLETE`
