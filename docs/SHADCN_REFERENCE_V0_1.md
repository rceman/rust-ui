# Shadcn Gallery Reference v0.1 - pointer

`reference/shadcn-gallery-v0.1/` contains a deterministic, isolated
HTML/CSS reference of the current official shadcn visual language
(`base-nova` = Base UI + Nova style + `neutral` base color + Lucide +
radius default), frozen as the measurement target for a later native
rust-ui Gallery.

Status: `SHADCN_REFERENCE_V0_1_FROZEN`, `reference_version: "0.1"`.
The approved generation marker `candidate_revision: 3` is retained as
provenance. Freeze policy is active. The canonical freeze record and final
review identities are in [`SHADCN_REFERENCE_V0_1_FREEZE.md`](SHADCN_REFERENCE_V0_1_FREEZE.md).

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
  now that both split reviews are approved.
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
- Capture environment: Playwright bundled Chromium (`channel:'chromium'`)
  launched only via `scripts/browser.js` with
  `--disable-gpu --force-color-profile=srgb --disable-lcd-text
  --disable-partial-raster`. The last flag is load-bearing: diagnosed
  nondeterminism - after paint invalidation (CDP `forcePseudoState` +
  measurement work) Chromium's partial rasterizer rerasters only the
  invalidated rect, and the fractional rounded corners of the SettingsNav
  Select trigger (x=925.96875, w=90.03125) received different AA coverage
  than a full-tile raster.
- Capture pipeline order (per page/state/theme, exactly as implemented):
  goto -> render-done -> fonts.ready -> font-proof mutation + full revert
  (light+default only) -> capture-state assertions -> CDP force -> settle
  -> measure -> bindings/pressed eval -> state assertions -> screenshot ->
  pixel guard/ink validation -> store under `captures[<page>/<state>]` ->
  session detach -> next goto.
- `contract.json` schema `contract/0.2`: per-capture records
  (`elements[].captures["<page>/<capture_state>"]` with rect/box/text/
  text_runs/parts and, for editable elements,
  `interaction {focused, selection|null}`); hidden parts record
  `{visible:false}`; `data-part-owner` attributes out-of-subtree text/parts
  (checkbox labels) to their owner.
- Single-focus model (V04): a document has one focus/selection. The
  `default` capture state focuses `native-text.single-line.selection`
  ([0,6]) on every page where it appears; `native-text.multiline.selection`'s
  focused/selected appearance is captured under
  `native-text/multiline-selection` and recorded unfocused in default
  captures. `validateSameStateConsistency` enforces "same id + same capture
  state on different pages => identical appearance", live-checked by
  `validateCaptureAuthority` (rect + box paint + interaction + part paint).

Git bases: foundation review `e827f3dfb5d29e50bcc64f90e5d735624641d878`;
frozen foundation production `00dc29acfeb686d6a190d91624de7ab9a48e1e92`.

See `reference/shadcn-gallery-v0.1/README.md` for the full contract.
