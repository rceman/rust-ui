# Style Customization Model — Owner Context

Status: owner direction for architecture review.

This records the intended API/product model. Preserve it unless the review finds a concrete architectural problem.

## Three first-class workflows

```text
FAST
    use a standard component

CUSTOM
    build something new from public low-level primitives

SURGICAL
    use a standard component and override one or two details
```

## Layer 1 — High-level components

Most product code should use ready components through variants, sizes, theme recipes, and normal component properties.

```rust
ui.button("Send")
    .variant(ButtonVariant::Primary)
    .size(ControlSize::Md)
    .icon(Icon::ArrowUp);
```

The consumer should not need to know how border, padding, radius, hover, focus, and similar internals are implemented.

## Layer 2 — Low-level public primitives

The library must expose enough typed low-level primitives to build genuinely custom UI.

The owner does not want built-in components implemented by a private visual system that consumers cannot reproduce or compose from.

Desired direction: much of the useful visual flexibility of HTML/CSS building blocks, expressed as typed Rust rather than a browser styling language.

Capabilities to seriously consider:

```text
background / foreground
border width/color
individual border sides
corner radii
individual corners
padding/insets
individual sides
width/height/min/max
opacity
shadow
```

The review decides the exact v0.1 subset.

## Layer 3 — Component plus partial override

Canonical owner example:

> Use the normal button, but make only its bottom border 2 dp and use a custom red color.

Conceptually:

```rust
ui.button("Send")
    .variant(ButtonVariant::Primary)
    .style(
        ButtonStylePatch::new()
            .border_bottom_width(dp(2.0))
            .border_bottom_color(Color::rgb(255, 0, 0))
    );
```

The exact names are open. The semantic requirement is not: only explicitly patched properties change. Everything else still resolves from the standard component recipe.

## Arbitrary colors

Theme roles remain the preferred/default path, but custom UI must be able to use explicit colors too. A conceptual representation such as Color::Role(...) plus Color::Rgba(...) is acceptable; exact storage/API is for review.

## Per-side borders

Independent sides are specifically desired for bottom-only separators, active indicators, asymmetric surfaces, and surgical component customization.

## Per-corner radii and per-side insets

A coherent model such as Insets { top, right, bottom, left } and CornerRadii { top_left, top_right, bottom_right, bottom_left } is preferred unless the review identifies a better typed representation.

## No CSS engine

Explicitly unwanted: CSS parser, selectors, specificity, cascade, string property bags, DOM, pseudo-selector strings, or full browser layout semantics.

Guiding principle:

> high visual customizability with deterministic typed resolution.

## Resolution principle

Preferred conceptual order:

```text
theme
→ component defaults
→ variant
→ size
→ interaction state
→ consumer partial override
→ accessibility/system enforcement
```

There must be a simple answer to: why did this property get this value?

## Built-ins and primitives

Where practical, Button, Badge, Tooltip, Surface, and similar components should resolve into the same public primitive/style concepts available to consumers.

This does not require native controls to become painted primitives. RichEdit and similar controls may remain special platform peers because their native behavior is the point.

## Scope

Architecture review only. Do not implement the styling system, start Mascot migration, add another platform, add CI, or expand into a browser-like layout model.

The intended follow-up after owner approval is the Windows implementation spike.
