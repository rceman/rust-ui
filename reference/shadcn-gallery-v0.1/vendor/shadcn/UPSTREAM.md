# Vendored upstream — shadcn/ui

Source: https://github.com/shadcn-ui/ui at commit
`295a1f114a138f23b5dfee0e0c6812394dfeb90c` (2026-10-02), MIT license
(`LICENSE.md` retained verbatim).

Canonical style: `base-nova` — Base UI library + Nova style + `neutral`
base color + Lucide icons + radius `default` (`--radius: 0.625rem`) +
menuAccent `subtle` + menuColor `default`. Evidence: `component-preview`
defaults to `styleName = "base-nova"`; `packages/shadcn/src/preset/defaults.ts`
lists `nova` first; https://ui.shadcn.com/docs/theming documents
`"style": "base-nova"`. (The components.json docs page still says
`new-york` — stale; `vega` is the legacy New York look.)

Vendored verbatim, upstream paths preserved:

- `apps/v4/registry/styles/style-nova.css` — `.style-nova` recipes
  (`.cn-*` hook classes via `@apply`).
- `packages/shadcn/src/tailwind.css` — custom `data-*` variants
  (data-open/data-checked/data-unchecked/data-disabled/data-active/…)
  and keyframes.
- `theme-neutral.css` — generated verbatim from `apps/v4/registry/themes.ts`
  entry `name: "neutral"` (`cssVars.light` / `cssVars.dark`), matching the
  "Default Theme CSS" published on /docs/theming.
- `apps/v4/registry/bases/base/ui/*.tsx` — component sources used to
  transcribe rendered DOM (element types, `data-slot`, `cn-*` classes,
  utility classes, Base UI rendered attributes).
- `apps/v4/registry/bases/base/examples/*-example.tsx` — canonical
  example content sources.

Deviation (rust-ui): Nova's default font is Geist; this reference renders
with Segoe UI Variable — an explicit documented deviation, not a hybrid.
