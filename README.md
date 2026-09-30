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
- CSS/Tailwind runtime;
- Electron/WebView;
- a Qt-sized universal framework;
- copying the entire shadcn catalog;
- mascot/agent/provider-specific logic.

Start with:

- `docs/RUST_UI_ARCHITECTURE_CONCEPT_TASK.md`
- `docs/MASCOT_PROVEN_CONTEXT.md`
- `docs/ASTRA_HANDOFF.md`
