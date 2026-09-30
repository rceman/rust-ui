# rust-ui

A small native cross-platform Rust UI component system.

Current phase: **architecture concept only**.

The project is intentionally being designed before implementation is migrated from `rceman/mascot`.

## Direction

- concise native Rust programming model;
- shadcn-first visual and motion language;
- Lucide icons;
- Windows and macOS native backends;
- native editable text controls where appropriate;
- strongly typed components, state, events and styles;
- event-driven rendering with no permanent idle frame loop;
- component scope driven by real product needs.

## Non-goals

- browser/DOM runtime;
- JavaScript;
- CSS/Tailwind parser or selector runtime in core or v0.1;
- Electron/WebView;
- a Qt-sized universal framework;
- copying the entire shadcn catalog;
- mascot/agent/provider-specific logic.

Future optional CSS/generated styling frontends are architecturally supported
through the same typed style/layout inputs, not dependencies of core or
features of v0.1. The reserved boundary lives in
[the canonical styling model](docs/STYLE_CUSTOMIZATION_MODEL.md#future-optional-authoring-frontends-reserved).
No frontend or watcher is implemented now.

Start with:

- `docs/RUST_UI_ARCHITECTURE_CONCEPT_TASK.md`
- `docs/MASCOT_PROVEN_CONTEXT.md`
- `docs/ASTRA_HANDOFF.md`

## Architecture concept deliverables

Proposed-model documentation only; no implementation exists yet.

- [docs/ARCHITECTURE_OPTIONS.md](docs/ARCHITECTURE_OPTIONS.md) — candidate models A-F comparison and references
- [docs/PROGRAMMING_MODEL.md](docs/PROGRAMMING_MODEL.md) — recommended model, signatures, tree/keys/invalidation
- [docs/STATE_AND_EVENTS.md](docs/STATE_AND_EVENTS.md) — state ownership, event table, async contracts
- [docs/LAYOUT_STYLE_MOTION.md](docs/LAYOUT_STYLE_MOTION.md) — layout primitives, typed theme/style, motion
- [docs/NATIVE_CONTROL_BOUNDARY.md](docs/NATIVE_CONTROL_BOUNDARY.md) — text peer contract, geometry, custom rendering
- [docs/BACKEND_ARCHITECTURE.md](docs/BACKEND_ARCHITECTURE.md) — crate packaging, Windows/macOS boundaries
- [docs/API_EXAMPLES.md](docs/API_EXAMPLES.md) — ten core exercises plus three style workflows
- [docs/MASCOT_MIGRATION_PLAN.md](docs/MASCOT_MIGRATION_PLAN.md) — future extraction mapping (post-approval)
- [docs/ASTRA_ARCHITECTURE_REPORT.md](docs/ASTRA_ARCHITECTURE_REPORT.md) — recommendation, risks, owner questions, spike plan
- [docs/STYLE_CUSTOMIZATION_MODEL.md](docs/STYLE_CUSTOMIZATION_MODEL.md) — authoritative styling/customization model (style review update on `agent/style-customization-model-review`)
