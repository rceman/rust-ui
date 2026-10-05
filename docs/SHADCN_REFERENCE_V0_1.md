# Shadcn Gallery Reference v0.1 - pointer

`reference/shadcn-gallery-v0.1/` contains a deterministic, isolated
HTML/CSS reference of the current official shadcn visual language
(`base-nova` = Base UI + Nova style + `neutral` base color + Lucide +
radius default), frozen as the measurement target for a later native
rust-ui Gallery.

Hierarchy:

```
ui.shadcn.com + upstream repo @ 295a1f114a138f23b5dfee0e0c6812394dfeb90c
  -> reference/shadcn-gallery-v0.1/ (HTML/CSS, vendored upstream recipes)
    -> contract.json + tokens.json + reference.json
      -> screenshots/{light,dark}/*.png
        -> future native rust-ui implementation compared via rust-ui-devctl
```

Rules:

- The reference is dev-only tooling (Node/Tailwind/Playwright inside the
  reference dir). It adds zero rust-ui runtime/Cargo dependency.
- `contract.json`, `tokens.json`, `reference.json`, `static/gallery.css`
  and `screenshots/` are FROZEN artifacts. Any change to them requires a
  `reference_version` bump and a fresh `npm run reference:capture` +
  `reference:check` (byte-identical rebuild/re-capture, font proof,
  system-mode, light/dark geometry identity, token closure).
- `tokens.json` is the frozen reference palette, NOT the rust-ui runtime
  style authority - the typed theme API stays canonical.
- Explicit deviation: Nova's canonical font Geist is replaced by Segoe UI
  Variable (mono: Cascadia Mono); recorded in `reference.json` and the
  reference README.
- Automation ids are the future `data-automation-id` / UIA `AutomationId`
  bridge.

Git bases: foundation review `e827f3dfb5d29e50bcc64f90e5d735624641d878`;
frozen foundation production `00dc29acfeb686d6a190d91624de7ab9a48e1e92`.

See `reference/shadcn-gallery-v0.1/README.md` for the full contract.
