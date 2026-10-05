# rust-ui product scope

Canonical, durable project authority. Component, layout, reference and
backend work must stay inside this scope.

## Rule

```text
rust-ui = desktop-only + fully desktop-responsive
```

- rust-ui targets **desktop application UI**.
- rust-ui is **fully responsive to desktop window size and available layout
  constraints**. Desktop responsiveness is required, not optional.
- **Mobile platforms and mobile product behavior are intentionally
  unsupported.**
- **Responsive != mobile.** "No mobile support" never means "no responsive
  layout".
- Future Windows, macOS and Linux desktop backends share the same semantic
  and layout contracts (see `PLATFORM_CONTRACTS.md`); Windows is the current
  backend, not the semantic definition.

## Supported: desktop responsiveness

Layouts and component contracts must remain compatible with:

- dynamic window resizing; layout driven by available width/height, never by
  OS or device identity;
- `Content` / `Fixed` / `Fill` sizing with min/max constraints
  (`LAYOUT_STYLE_MOTION.md`);
- Row <-> Column restructuring, adaptive wrapping, dynamic column counts,
  responsive spacing;
- compact / normal / wide component presentations and desktop breakpoints;
- responsive navigation widths (e.g. SettingsNav / Sidebar wide, compact,
  collapsed icon rail);
- split panes, resizable regions, overflow handling, scrolling, content
  reflow, conditional visibility of secondary desktop UI.

Breakpoints are expressed in logical `Dp` of available space. Example (an
illustration, not a frozen API):

```text
>= 1100 dp   wide SettingsNav + content
700-1099 dp  compact SettingsNav + content
< 700 dp     collapsed desktop rail or another desktop composition
```

## Out of scope

Phones; tablet/mobile product modes; iOS/Android UI conventions; touch-first
mobile contracts; mobile navigation drawers, mobile Sheet substitution,
phone offcanvas navigation; safe areas, notches, orientation behavior;
mobile viewport/browser quirks; mobile-only responsive providers; any
device-class switching whose purpose is to turn the application into a
mobile app.

## Upstream references

Upstream design sources (e.g. shadcn) may contain mobile behavior. That does
not make it part of rust-ui:

```text
vendored upstream source = provenance / reference (kept verbatim)
rust-ui contract         = desktop subset / adaptation
```

Vendored upstream files are never edited to strip mobile code; the rust-ui
contract simply does not transcribe it.

## Visual reference capture vs. runtime layout

A deterministic reference capture uses fixed canonical viewport(s) (e.g.
`reference/shadcn-gallery-v0.1/`: 1440x1000 CSS px, DPR 1). A canonical
capture size is a measurement convention only; it does not imply a
fixed-size rust-ui layout system. Native components and layouts may have
multiple desktop-responsive presentations.

## Fonts

Canonical default visual family: **Geist Sans + Geist Mono**, so text
metrics are identical across desktop operating systems. Application-provided
fallbacks (e.g. Noto Sans JP for Japanese) and platform fallback as the last
resort are future font-registration work; font rendering and IME input are
separate concerns.

## Related

- `COMPONENT_SUPPORT.md` - what rust-ui supports today.
- `REVIEW_AUTHORITY.md` - who reviews what for UI milestones.
