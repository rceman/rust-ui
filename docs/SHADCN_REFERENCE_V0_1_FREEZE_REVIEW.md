# Independent Shadcn Gallery reference v0.1 freeze review

## Decision and immutable identities

**SHADCN_REFERENCE_V0_1_REWORK_REQUIRED**

The Base UI/Nova/neutral authority choice is accepted. Rust foundation approval remains intact. This reference must not yet become the frozen native visual/geometry authority: its exporter produces incorrect or missing geometry, loses repeated children, and disagrees with some numeric tokens. There are also bounded transcription, state-coverage, theme-switching and attribution corrections.

- RUN_KEY: `RUI-A-261005-1034`
- Role: Astra Advisor, independent review only.
- Branch: `agent/shadcn-gallery-reference-v0.1-opus55`.
- Reviewed implementation: `2478b01387d04f17f26ac65f1f5a23da7c25cd5b`.
- Reviewed publication: `31842042055bdf535f33c848480c535d16516bb5`.
- Foundation review base: `e827f3dfb5d29e50bcc64f90e5d735624641d878`.
- Frozen Rust production: `00dc29acfeb686d6a190d91624de7ab9a48e1e92`.
- Review started: `2026-10-05 11:36:31 UTC+03:00`.

Publication adds only the README's Playwright Chromium prerequisite over the reference implementation. The complete diff from the foundation review base adds the reference directory and its documentation pointer. `src/`, `examples/`, `Cargo.toml` and `Cargo.lock` are unchanged. No browser, Node, Tailwind or Base UI runtime dependency enters Rust production. This review changes only this document; it does not implement native Gallery or devctl, change screenshots, revise an earlier review, or merge main.

## Independently executed verification

The authoritative checkout is `W:\devin_folder\rust-ui`. A separate native Windows Git clone, with no hardlinks, was checked out detached at the exact publication SHA. It contains only committed source and artifacts; it did not inherit the original reference's `node_modules` or ignored files.

Environment: Windows build `26200`, Node `v24.21.0`, npm `11.19.0`, Cargo `1.94.0`, Playwright `1.63.0`, bundled Chromium `153.0.8010.12`. System fonts resolved to Segoe UI Variable and Cascadia Mono. WSL was used for read-only Git/blob analysis and image inspection; canonical reproduction and Rust tests ran natively on Windows.

| Independently executed check | Result |
|---|---|
| Fresh clone, detached exact publication checkout | PASS |
| Documented `npm ci` | PASS |
| Documented `npx playwright install chromium` | PASS |
| Documented `npm run reference:capture` | PASS; generated tracked files unchanged |
| Documented `npm run reference:check`, twice consecutively | PASS both times |
| Actual committed PNG bytes versus `screenshots/SHA256.json` | PASS, all 16 |
| Independent post-pseudo-state Light/Dark outer rectangles and document dimensions | PASS, all seven pages |
| Native Windows `cargo fmt --check` | PASS |
| Native Windows `cargo test --locked --lib` | PASS, 121/121, none ignored |
| Existing RichEdit ink assertion in that Windows run | PASS |
| Git blob comparison of 57 verbatim vendored upstream files | PASS, no differences |
| Browser repro: contract versus actually pressed button position | FAIL, R01 |
| Contract text and repeated-part audit | FAIL, R02/R03 |
| Browser-computed backdrop versus exported token | FAIL, R04 |
| Browser repro: official inline-start icon attribute | FAIL, R05 |
| Browser repro: System -> explicit Light -> OS Dark | FAIL, R07 |

`reference:check` being green proves repeatability of the current exporter, not correctness or completeness of everything it exports. The discovered defects exist in the reviewed, unchanged candidate.

Independent scratch browser probes use the installed reference files and pinned Playwright; they are outside the repository. Logs are `/tmp/rui-shadcn-review-reproduce.log` and `/tmp/rui-shadcn-review-rust.log`. Native scratch probes and their computed-style audit are under `W:\devin_folder\rust-ui-shadcn-review-tools-261005`. These paths identify this review environment only, not prerequisites for reproducing the reference.

No broad native foundation matrix was replayed. No Linux font rendering or Linux canonical screenshot equivalence is claimed. The current font substitution deliberately requires the Windows font environment. No physical display capture was needed or represented as browser screenshot proof.

## Official authority and vendoring

Current official source was independently fetched from `shadcn-ui/ui`. Observed upstream main was `6b600cf1ff42f8a746747ea587e52af3ee224643`; both that source and the claimed pinned source retain the relevant Nova defaults. The reference's authority commit is `295a1f114a138f23b5dfee0e0c6812394dfeb90c`, dated October 2, 2026.

- The official [component preview source](https://github.com/shadcn-ui/ui/blob/295a1f114a138f23b5dfee0e0c6812394dfeb90c/apps/v4/components/component-preview.tsx) defaults to `base-nova`.
- Official [preset defaults](https://github.com/shadcn-ui/ui/blob/295a1f114a138f23b5dfee0e0c6812394dfeb90c/packages/shadcn/src/preset/defaults.ts) define Nova with neutral colors, Lucide, Geist, default radius, subtle menu accent and default menu color.
- Official [theming documentation](https://ui.shadcn.com/docs/theming) uses `base-nova`; its pinned source agrees. The [components.json documentation](https://ui.shadcn.com/docs/components-json) still recommends `new-york`. That legacy prose does not override the current preview/preset implementation.
- Official [style registry imports](https://github.com/shadcn-ui/ui/blob/295a1f114a138f23b5dfee0e0c6812394dfeb90c/apps/v4/app/style-registry.css) put recipe CSS in the base layer. This reference does likewise, letting component utilities outrank recipes.
- Official [menu transformer](https://github.com/shadcn-ui/ui/blob/295a1f114a138f23b5dfee0e0c6812394dfeb90c/packages/registry/src/utils/transformers/transform-menu.ts) removes both menu placeholder classes for the selected default menu color. Their removal here is correct; the current menu/select surfaces are opaque.

Nova is a defensible current default for this project, not the only possible upstream style. Freeze the selected upstream snapshot/configuration when the reference corrections are accepted. The deliberate Geist -> Segoe UI Variable and mono -> Cascadia Mono substitution is explicitly documented and acceptable for this Windows reference.

The 57 exact Git-blob matches comprise the shadcn MIT license, Nova recipe, helper CSS, 27 component wrappers and 27 examples. Existing working-copy CRLF differences were not mistaken for upstream edits: comparisons used immutable Git blobs. The generated neutral helper is a separate derived artifact and has inaccuracies in its claimed provenance, R08. Lucide's committed geometry needs its complete license notices, R09.

## Actual visual inspection

All seven canonical pages were visually inspected in both themes, plus both multiline-selection screenshots: 16 originals in total. The tall All pages were inspected in consecutive strips covering their entire height; important button, radio, native text and menu details were also inspected at original pixel scale. Temporary inspection crops were outside the repository. Frozen originals were not rewritten.

The shell, grid rhythm, ordinary alignment, button labels, icon presence, radii, thin borders, focus rings, muted hierarchy, selected controls, tabs, pagination, toggle/button groups and form spacing are coherent. Light/Dark presentations have matching layout. No obvious overlapping text, empty controls, missing glyph geometry, misplaced unrelated overlays or visibly clipped overlay shadows was observed. Dialog, Alert Dialog, Popover, Tooltip, Dropdown Menu and Select open each have a separate deterministic stage. Tooltip arrow and menu indicators are visible. Forms textareas deliberately retain the browser resize grip; Native Text removes it, as documented.

This positive visual inspection does not cure numeric parity defects. The 2 px icon-button mismatch in R05 is grounded in upstream attributes and browser measurements. R01 is a mismatch between the screenshot's real pressed state and its own contract. Neither is a personal design preference.

Screenshot inventory is exactly eight Light plus eight Dark files: `all`, `components`, `forms`, `navigation`, `typography`, `overlays`, `native-text`, and `native-text--multiline-selection`. All are 1440 physical pixels wide at DPR 1. Full-page heights are respectively 12593, 2608, 3095, 2168, 2151, 2532 and 1066; the extra selection capture is also 1066. The 1000 px viewport is correctly distinguished from full-page image height.

## Bounded findings

### R01 — MAJOR: measured geometry precedes the frozen pseudo state

**CONCRETE DEFECT:** `capture.js` measures before applying CDP pseudo states and claims those states do not change geometry. Nova's pressed button translates by 1 px. `button.default.pressed` on Components exports `y=300.75`; the actual captured active state has `y=301.75`. Independent browser measurements reproduce this for all six pressed variants on All and Components, in both themes.

**WHY IT MATTERS FOR NATIVE PARITY:** Matching the recorded rectangle places a pressed native button at a different position from the frozen screenshot.

**EXACT ARTIFACT/LOCATION:** `scripts/capture.js:219-270`, `contract.json` pressed-button placements; upstream `.cn-button` recipe's active translation.

**SMALLEST CORRECTION:** Apply forced states and settle layout before measuring the state used for its screenshot. Derive Light/Dark parity from that same effective state. Remove the assertion that forced states cannot affect geometry.

**PROOF REQUIRED TO CLOSE:** Assert all six pressed placements against post-force browser rectangles, including the 1 px translation; repeat capture/check byte-identically and compare Light/Dark documents, parts and text as well as outer rectangles.

### R02 — MAJOR: native text bounds/baselines are absent or invalid

**CONCRETE DEFECT:** Single-line form/native inputs export `text.rect=null` and `baseline_proxy_y=null`. Textareas measure non-rendered DOM value text with Range. For `native-text.multiline.selection`, the export is `rect={x:-540,y:-730.5,width:0,height:0}` and baseline `-715.5`. Across the contract, 23 text-bearing records have null or nonpositive text bounds. Compound records also aggregate differently styled descendants into a parent font/union, rather than describing each actual text run; radio labels are a concrete example.

**WHY IT MATTERS FOR NATIVE PARITY:** The most important RichEdit visual specimens cannot define text placement, baselines or line geometry without someone interpreting pixels by eye. Bogus numeric values must not become native targets.

**EXACT ARTIFACT/LOCATION:** `scripts/capture.js:63-107`; corresponding Input/Textarea/Native Text and compound text entries in `contract.json`.

**SMALLEST CORRECTION:** Supply a documented, browser-validated control text measurement/proxy for value and placeholder content. Measure compound text at meaningful leaf/run boundaries with its effective font. Store valid local line/text bounds and a clearly defined baseline proxy; reject non-rendered zero-size Range results. Keep browser editing semantics outside native authority.

**PROOF REQUIRED TO CLOSE:** Positive in-control text/line bounds and valid baselines for single-line, multiline, placeholder, focused and selected specimens, checked against rendering. Reject the exact negative/zero rectangle above. Verify leaf fonts/line heights and text measurements in both themes and both selection captures.

### R03 — MAJOR: repeated child parts are silently overwritten

**CONCRETE DEFECT:** The exporter assigns `rec.parts[name]=rect`. Repeated names silently retain only the last child. Independent DOM audit found repeated parts in Skeleton, Breadcrumb, Pagination, Toggle Group, Button Group, list/table typography, Dropdown Menu and Select. For example, the table has two `header`, two `row` and four `cell` occurrences; breadcrumb has two links and three separators. Radio Group does not export the actual two radio item/dot rectangles, only one indicator and an outer group.

**WHY IT MATTERS FOR NATIVE PARITY:** A later implementation cannot compare each item, glyph, cell or radio circle using stable semantic targets; valid differences can disappear from the contract.

**EXACT ARTIFACT/LOCATION:** `scripts/capture.js:56-61`; repeated `data-part` declarations in `src/catalog.js`, generated `src/icons.js`; affected `contract.json.parts`.

**SMALLEST CORRECTION:** Give repeated parts stable semantic identities or an explicitly stable keyed collection. Measure both radio items and their visible indicators/dots. Preserve icon ownership within each meaningful child. Do not add DOM-path identity.

**PROOF REQUIRED TO CLOSE:** Enumerate every rendered meaningful part and assert a one-to-one exported match with no overwrite. Specifically require all table cells, breadcrumb links/separators, group icons, menu/options and both radio item geometries. Duplicate local keys must fail generation/check.

### R04 — MAJOR: numeric tokens disagree with CSS, and closure checks omit promised properties

**CONCRETE DEFECT:** `translucent.backdrop-black-10` exports alpha `1` in both themes although the actual dialog/alert backdrop is black at alpha `0.1`; independent computed styles confirm it. The icon-with-label padding token assumes the branch missing in R05. `reference:check` tests radius, border width, font-size and font-weight membership only. It does not check the advertised control heights, padding, line heights, colors/alpha, shadows, focus ring or disabled output. Thus both full checks pass with the incorrect backdrop token. Secondary-hover color is left as a formula rather than a resolved native-ready value.

**WHY IT MATTERS FOR NATIVE PARITY:** Consuming the numeric backdrop produces an opaque surface instead of the frozen visual. A supposedly complete native token target can silently drift from the browser output.

**EXACT ARTIFACT/LOCATION:** `scripts/tokens.js`, `tokens.json.translucent`, `scripts/check.js:187-228`, and README closure claims. The version condition at `check.js:35` also has an empty body and enforces nothing.

**SMALLEST CORRECTION:** Correct the concrete token discrepancies. Derive or validate token values against the authoritative CSS/computed effective output, with explicit specimen/state mappings and resolved colors where needed. Extend closure to the promised used properties, including important parts, and make version validation real. Keep gallery-only tokens explicitly separate.

**PROOF REQUIRED TO CLOSE:** Browser-backed equality for palette/alpha, used control dimensions/padding/line heights, shadows/rings and disabled/state output. Deliberately wrong backdrop alpha, mapped height, padding, line-height and version must fail. Updated current-state documentation must describe the actual checks.

### R05 — MINOR, required before freeze: inline-start icon recipe branch never activates

**CONCRETE DEFECT:** `button.default.icon-inline-start` creates a plus SVG without `data-icon="inline-start"`. Official button examples use that attribute and Nova adjusts start padding through it. Actual reference width is `102.688` with 10 px start padding. Adding only the official attribute in a scratch browser changes width to `100.688` and start padding to 8 px.

**WHY IT MATTERS FOR NATIVE PARITY:** The frozen specimen presents the wrong upstream icon-button geometry, contradicting its semantic name and padding token.

**EXACT ARTIFACT/LOCATION:** `src/catalog.js:445`, button helper, `scripts/build-icons.js`; the pinned upstream `button-example.tsx` and Nova button-size recipe.

**SMALLEST CORRECTION:** Preserve the official icon-position attribute through the icon helper for the relevant specimen, then regenerate its measurements/screenshots.

**PROOF REQUIRED TO CLOSE:** Assert the rendered attribute, computed start padding and measured button/icon/text rectangles; compare with the upstream branch in both themes.

### R06 — MAJOR: included families lack material upstream state/size targets

**CONCRETE DEFECT:** The catalog has no Button invalid state; no Slider thumb hover/active/focus-visible specimens despite the upstream ring rules; no Radio Group invalid specimen; no disabled open menu/select option; no Toggle sm/lg specimens; and no Native Text multiline invalid specimen or explicit canonical reuse mapping to the existing Forms invalid Textarea. These are visually distinct states/sizes of included families, not requests for excluded families.

**WHY IT MATTERS FOR NATIVE PARITY:** Immediate native implementations of these included families would have to invent the missing invalid/ring/disabled/size appearances or resolve them by eye.

**EXACT ARTIFACT/LOCATION:** `src/catalog.js` Button, Radio, Slider, Toggle, overlay options and Native Text entries; pinned Nova rules and component wrappers/examples; corresponding absent contract IDs/captures.

**SMALLEST CORRECTION:** Add representative missing material states/sizes to the existing single catalog, or an explicit reuse mapping where the output is genuinely identical. No full Cartesian product or new component family is required. For states sharing the same output, measured equivalence can justify shared reference data.

**PROOF REQUIRED TO CLOSE:** A bounded upstream-rule-to-specimen coverage table, deterministic semantic IDs, and computed-style/screenshot assertions proving each required branch or documented equivalence. Use upstream pseudo states/data attributes rather than invented inline appearance.

### R07 — MINOR, required before freeze: System listener overrides explicit modes

**CONCRETE DEFECT:** The first System listener closes over `mode="system"` and remains installed when Light or Dark is selected. Independent repro: open System under emulated Light, click Light, change browser preference to Dark. URL still says `theme=light`, but the root has `.dark=true`. The current check tests only fresh System contexts and misses this transition.

**WHY IT MATTERS FOR NATIVE PARITY:** Interactive inspection cannot reliably retain the explicitly selected visual target after an OS preference change. This does not invalidate the fresh explicit-theme screenshot hashes, but violates the promised mode boundary.

**EXACT ARTIFACT/LOCATION:** `src/render.js:143-155`, `scripts/check.js` System-mode checks.

**SMALLEST CORRECTION:** Make the listener consult current mode, or remove/rebind it when mode changes, without accumulating listeners.

**PROOF REQUIRED TO CLOSE:** In one live document, change OS preference in System, explicit Light and explicit Dark, and switch back to System. Only System follows the preference. Repeat navigation/mode changes without stale handlers.

### R08 — MINOR: derived neutral helper is inaccurately presented as verbatim upstream

**CONCRETE DEFECT:** `vendor/shadcn/theme-neutral.css` claims verbatim generation from the pinned `themes.ts`. It adds `destructive-foreground`, absent from that entry, and changes dark sidebar foreground from upstream `oklch(0.985 0 0)` to `oklch(0.145 0 0)` and dark sidebar primary from upstream `oklch(0.488 0.243 264.376)` to `oklch(0.205 0 0)`. These sidebar slots are not current displayed Gallery specimens; this is a provenance defect, not a newly discovered screenshot/sidebar requirement.

**WHY IT MATTERS FOR NATIVE PARITY:** Future consumers must distinguish upstream authority from deliberate local derivations rather than inheriting silently altered source as official palette data.

**EXACT ARTIFACT/LOCATION:** `vendor/shadcn/theme-neutral.css`, `vendor/shadcn/UPSTREAM.md`, README's vendoring claim; pinned `apps/v4/registry/themes.ts`.

**SMALLEST CORRECTION:** Generate the claimed source values faithfully or explicitly identify the helper's local additions/overrides and their authority. Preserve the 57 already-verbatim files. Narrow font-proof wording where CDP skips nodes with no font result; the current loop proves actual fonts for returned results, not exhaustive coverage of every text-bearing node.

**PROOF REQUIRED TO CLOSE:** A deterministic derivation comparison or exact documented local delta; current provenance/font-proof statements match the actual generator/check coverage.

### R09 — MINOR: copied Lucide geometry lacks complete retained notices

**CONCRETE DEFECT:** Generated icons contain a brief ISC label/copyright/link and refer to a `vendor/lucide` notice in `reference.json` that does not exist. The committed reference does not retain the pinned package's ISC permission notice or its applicable Feather MIT notice. The installed `lucide-static@1.21.0/LICENSE` contains both, including icons used here. The shadcn MIT license itself is correctly retained.

**WHY IT MATTERS FOR NATIVE PARITY:** The frozen icon authority must be a self-contained attributable source bundle for later native reuse, not depend on an uncommitted installed package or a nonexistent notice.

**EXACT ARTIFACT/LOCATION:** `scripts/build-icons.js:38-43`, `src/icons.js`, missing referenced notice; installed pinned package `LICENSE`.

**SMALLEST CORRECTION:** Retain the complete pinned icon license/attribution notices and point generated metadata/comments to the real file.

**PROOF REQUIRED TO CLOSE:** Fresh checkout contains the notices with the exact icon package identity; the generated attribution points to an existing committed artifact.

## Other contract and hygiene conclusions

| Area | Independent conclusion |
|---|---|
| One catalog | PASS: one `window.CATALOG`, 146 unique catalog specimens; page filtering derives from category. Contract's 171 unique elements also include shell and overlay/extra child targets, not 171 independent catalog examples. |
| IDs | Top-level IDs are semantic and unique per rendered page, with distinct overlay triggers/content; suitable as future UIA automation IDs. Repeated part identity remains R03. Fixed tab/group indices need explicitly stable meaning, not DOM paths. |
| Inventory scope | Included family inventory is coherent for v0.1. The documented exclusions are a legitimate scope cap; no requirement to add all Shadcn families. Missing states within included families are R06. |
| Native Text boundary | Documented as Input/Textarea visual/chrome reference for future RichEdit. Browser editing behavior does not supersede rust-ui platform semantics. R02 concerns visual measurements. |
| CSS cascade/menu selection | Base-layer import order and default/subtle opaque menu configuration match inspected upstream. No wholesale cascade redesign is needed; R05 is one missed recipe attribute. |
| Overlays | Separate open stages avoid ambiguous stacked modals. Trigger/content identities and anchor side/alignment/offset exist; popup widths are documented fixed reference choices. Meaningful repeated children need R03. |
| Fonts | Real CDP platform-font queries and successful Windows reproduction establish Segoe UI Variable/Cascadia Mono for queried glyphs. CSS declares optical sizing auto and no synthetic weight. The loop skips empty/error CDP results; do not claim exhaustive per-node proof. R02 addresses compound/control metrics. |
| Determinism | Explicit Light/Dark, fixed 1440x1000 viewport, DPR 1, no zoom override, reduced motion, animations/transitions disabled and hidden caret. Local file assets, no random IDs/timestamps or render-time network content found. Byte-stable here; not a cross-OS font/raster guarantee. |
| Tokens ownership | `tokens.json` explicitly is not Rust runtime styling authority. Preserve that boundary while correcting R04. Gallery chrome is separately labeled. |
| Freeze metadata | Version, upstream source/style/font, review date, viewport/DPR, browser/platform/tool, themes/pages and both exact foundation SHAs are present. Git supplies the reference implementation/publication identities. |
| Drift resistance | Exact direct dev versions plus package lock, vendored source, local SVG/CSS, bundled browser; no live upstream rendering fetch. Future upstream adoption must be explicit versioned work. |
| Source line endings | Reference `.gitattributes` specifies LF text/binary PNG. Fresh native clone's canonical outputs remain unchanged and byte-stable; no reliance on the original author's working-copy line ending setting. |
| Repository hygiene | No committed node_modules, browser binaries/cache, fonts, credentials, developer absolute paths or debug captures found in the added surface. Largest new artifacts are the intended tall All screenshots, under 0.8 MB each. This review's local diagnostic paths are not tooling dependencies. |

## Universal Gates 1-20 for this reference milestone

Exactly the repository's twenty gates are used. This table concerns the reference milestone, not a reversal of Windows foundation approval.

| Gate | Status | Evidence |
|---|---|---|
| 1 Requirements/goal completeness | FAIL | Missing text/child targets and material states, R02/R03/R06. |
| 2 Assumptions/approval boundaries | PASS | Deliberate font substitution, fixed capture/configuration and exclusions explicit; review only. |
| 3 Contract preservation/correctness | FAIL | Screenshot-state geometry, numeric tokens and icon recipe mismatch, R01/R04/R05. |
| 4 Scope discipline | PASS | Only isolated reference plus docs added; Rust/existing examples/Cargo unchanged. |
| 5 Minimal correct design | PASS | Static transcriptions and a shared catalog are sufficient; no new runtime framework needed. |
| 6 Authority/ownership | FAIL | Exported native target and derived-source claims do not fully match authority, R01-R05/R08. |
| 7 Duplication/reuse | FAIL | Catalog reuse passes; independently handwritten token palette/quantities lack complete CSS authority validation, R04. |
| 8 No unrequested fallback/shim | PASS | No browser in Rust, no native text fallback; queried font fallback is checked. |
| 9 Deterministic/stable semantics | FAIL | Repeatable wrong geometry and stale System-mode state, R01/R07. |
| 10 Canonical authority/truth | FAIL | Nova choice correct; local derivation/recipe and numeric claims need R04/R05/R08. |
| 11 Failure safety | PASS | Isolated build/check failures nonzero, staging in fresh/temp output; no production or frozen-authority mutation by this review. |
| 12 Boundedness | PASS | Fixed catalog/pages/capture states, bounded offline tooling, finite browser lifecycle. |
| 13 Dependency/blast-radius isolation | PASS | Node/Tailwind/Playwright remain dev-only under reference; Cargo unchanged. |
| 14 Redundant work/performance | PASS | No reference tooling on production paths; finite explicit generation/checks. No unsupported native performance claim. |
| 15 Persistence/infrastructure | N/A | Static committed artifacts; no application persistence/control-plane/CI implementation in scope. |
| 16 Verification sufficiency | FAIL | Two green checks miss independently reproduced contract/token/theme defects, R01-R04/R07. |
| 17 Proportional recovery | PASS | Bounded exporter/specimen/check/document corrections; no architecture restart needed. |
| 18 Immutability/artifact identity | PASS | Exact source/publication reviewed; fresh regeneration unchanged, PNG hashes verified; only this review published. |
| 19 Completion/artifact honesty | FAIL | Current completeness/provenance claims exceed established output, R02-R04/R06/R08/R09. |
| 20 Systemic invariant audit | FAIL | Full meaningful-part/state/token coverage is not enforced; repeated-part audit identifies losses across multiple families. |

## Smallest closure path and handoff

Repair the existing reference's exporter, numeric token checks, omitted upstream attributes/states, current-mode listener and provenance/notices. Keep the selected upstream Nova/configuration and approved Rust foundation. Regenerate the corrected reference artifacts as a new review candidate; check both themes/selection states and every meaningful child, then reproduce from a fresh checkout with two byte-stable checks. Corrections during pre-approval preparation are not a request to redesign the visual style or start native work. Explain any artifact/version change according to the repository's freeze policy.

Approval is withheld for the frozen visual/geometry authority. Native component/Gallery and devctl/control-plane milestones are not cleared by this review. No implementation work on either began.

SHADCN_REFERENCE_V0_1_REWORK_REQUIRED

NATIVE_GALLERY_IMPLEMENTATION_NOT_CLEARED
