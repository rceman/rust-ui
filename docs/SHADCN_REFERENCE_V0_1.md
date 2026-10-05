# Shadcn Gallery Reference v0.1 - pointer

`reference/shadcn-gallery-v0.1/` contains a deterministic, isolated
HTML/CSS reference of the current official shadcn visual language
(`base-nova` = Base UI + Nova style + `neutral` base color + Lucide +
radius default), frozen as the measurement target for a later native
rust-ui Gallery.

Status: `reference_version: "0.1"`, `candidate_revision: 2` - a pre-freeze
candidate revision; freeze policy applies after split review approval.

Hierarchy:

```
ui.shadcn.com + upstream repo @ 295a1f114a138f23b5dfee0e0c6812394dfeb90c
  -> reference/shadcn-gallery-v0.1/ (HTML/CSS, vendored upstream recipes)
    -> contract.json + tokens.json + coverage.json + token-bindings.json
       + reference.json
      -> screenshots/{light,dark}/*.png
        -> future native rust-ui implementation compared via rust-ui-devctl
```

Rules:

- The reference is dev-only tooling (Node/Tailwind/Playwright inside the
  reference dir). It adds zero rust-ui runtime/Cargo dependency.
- Product scope is canonical in `docs/PRODUCT_SCOPE.md`: rust-ui is
  desktop-only and fully desktop-responsive; mobile is out of scope
  (responsive != mobile). The 1440x1000 DPR1 viewport is the deterministic
  canonical capture viewport, not a fixed-size layout contract. Upstream
  mobile paths (e.g. Sidebar Sheet/offcanvas/useIsMobile) stay vendored
  verbatim but are never transcribed.
- `contract.json`, `tokens.json`, `coverage.json`, `token-bindings.json`,
  `reference.json`, `static/gallery.css` and `screenshots/` are the frozen
  artifacts. The freeze rule (any change requires a `reference_version`
  bump and a fresh `npm run reference:capture` + `reference:check`) applies
  after split review approval.
- `tokens.json` is the frozen reference palette, NOT the rust-ui runtime
  style authority - the typed theme API stays canonical.
- Fonts: Geist + Geist Mono (upstream canonical), vendored OFL woff2 in
  `vendor/geist/`; font resolution is proven exhaustively at capture
  (fail-closed: every text-bearing node must resolve a Geist custom font).
- Catalog specimens carry `tier` (core/later/reference-only) + `family` -
  the catalog `tier` field is the source of specimen tiers;
  `docs/COMPONENT_SUPPORT.md` remains the durable component support ledger.
  `coverage.json` is the bounded upstream-selector-to-specimen map and
  `token-bindings.json` binds live computed values to tokens.
- Automation ids are the future `data-automation-id` / UIA `AutomationId`
  bridge; repeated parts are keyed semantically (`item[x].indicator`).

Git bases: foundation review `e827f3dfb5d29e50bcc64f90e5d735624641d878`;
frozen foundation production `00dc29acfeb686d6a190d91624de7ab9a48e1e92`.

See `reference/shadcn-gallery-v0.1/README.md` for the full contract.
