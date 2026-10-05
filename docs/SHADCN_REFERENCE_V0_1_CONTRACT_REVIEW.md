# Shadcn reference v0.1 — independent contract freeze review

## Decision and identity

**CONTRACT_REFERENCE_REWORK_REQUIRED**

The candidate is substantially improved, but its contract loses capture-specific data, omits some CORE text, and still permits incorrect or incomplete data to pass validation. Fresh-clone byte reproduction also failed. These are bounded reference/exporter corrections; the accepted platform architecture and selected Nova visual language are not reopened.

| Identity | Value |
|---|---|
| Run | `RUI-A-261005-1604` |
| Reviewer | Astra Advisor — non-visual contract/tooling authority |
| Candidate source | `2fe97eaf5438ea928a85224956248f859d6ac464` |
| Candidate branch | `agent/shadcn-gallery-reference-v0.1-opus55` |
| Review branch | `review/shadcn-reference-v0.1-contract-astra`, based directly on the exact candidate |
| Prior contract review | `bb39e27a2a65006cd5557aeafb5032c619ff159b` — `docs/SHADCN_REFERENCE_V0_1_FREEZE_REVIEW.md` |
| Foundation review base | `e827f3dfb5d29e50bcc64f90e5d735624641d878` |
| Frozen Rust production | `00dc29acfeb686d6a190d91624de7ab9a48e1e92` |
| Review started | `2026-10-05 18:30:52 UTC+03:00` |
| Upstream snapshot | `shadcn-ui/ui@295a1f114a138f23b5dfee0e0c6812394dfeb90c` |

Only this new document is published. Candidate files and previous independent reviews remain unchanged. UI taste, polish, visual state fidelity and visual approval belong to the parallel Opus review. This report does not grant either native Gallery or devctl implementation clearance.

## Independently executed verification

The orchestration environment was Linux/WSL; reference execution used native Windows 10.0.26200, Node `v24.21.0`, npm `11.19.0`, Playwright `1.63.0`, Chromium `153.0.8010.12`. Browser flags matched the capture script: software rendering, sRGB profile and grayscale text antialiasing. Canonical contexts used 1440×1000 CSS px, DPR 1, reduced motion and explicit Light/Dark. Screenshots are full-document images, so their heights exceed the viewport.

A separate native Git clone was created with `--no-hardlinks` and checked out at the exact candidate. It inherited tracked Git content, not ignored prerequisites. From its reference directory, the documented sequence was executed:

```text
npm ci
npx playwright install chromium
npm run reference:capture
npm run reference:check
npm run reference:check
```

| Check | Independent result |
|---|---|
| Clean checkout before setup | PASS; exact candidate, no tracked changes |
| `npm ci` and Chromium prerequisite | PASS; no uncommitted/global package dependency needed |
| Documented capture | PASS exit status; 227 exported elements and 18 screenshots; generated diff in Light Navigation PNG and its manifest |
| First `reference:check` | **FAIL**, exit 1: `screenshots/light/navigation.png` hash mismatch; contract and rebuilt CSS identical |
| Second `reference:check` | **FAIL**, exit 1: same mismatch; contract and rebuilt CSS identical |
| Additional capture to isolated output | PASS execution; reproduced the other of the two observed Navigation image hashes |
| Committed PNG manifest | PASS independently: all 18 actual committed PNG bytes match their recorded hashes |
| Screenshot inventory | PASS: seven page pairs, Native Text selection pair, collapsed Navigation pair; all widths 1440 and corresponding Light/Dark image dimensions equal |
| Pressed geometry | PASS: capture records 44 pairs; checker checks 22 pairs; independent browser samples also agree with contract and Nova's +1px displacement |
| Inline-start icon | PASS: actual `data-icon="inline-start"`, 8px left padding, 16px icon; effective browser output agrees with contract/check |
| Parts | PASS for duplicate prevention/key enumeration: checker enumerates 794 part occurrences across pages/states; see C01 for lost per-state geometry |
| Tokens | 594 comparisons execute successfully on the unmodified data; independent mutations expose C03 |
| Coverage | 25 families / 75 mapping entries exist; independent deletions expose C04 |
| Live theme transitions | PASS: independent System/Light/Dark/OS-change/navigation sequence; instrumented actual `matchMedia` subscriptions remain exactly one |
| Fonts | PASS: capture's nonempty CDP proof reports 1209/1209 text-bearing elements/control mirrors; checker additionally proves 572/572 on All |
| Font fallback discrimination | PASS: independent CDP query returns custom Geist for the real button, then system Arial after an in-memory override and settled layout; capture's explicit family/custom-font predicate rejects the latter |
| Runtime requests | Independent browser probes issued only `file:` requests; local fonts/assets require no render-time network |
| Vendored Shadcn | PASS: all 63 claimed verbatim Git blobs match the pinned upstream commit byte-for-byte |
| Fonts/notices | PASS: committed Geist assets and OFL match the installed pinned package; committed Shadcn MIT and Lucide ISC/retained Feather MIT notices and attribution paths exist |
| Rust immutability | PASS: Git diff against the foundation review base is empty for `src/`, `examples/`, `Cargo.toml`, `Cargo.lock` |
| Native Windows `cargo fmt --check` | PASS |
| Native Windows `cargo test --locked --lib` | PASS, 121/121, none ignored; existing RichEdit ink assertion passes |

Font proof counts include repeated specimens across All/category pages and control mirrors; they are not 1209 unique strings or unique catalog specimens. The real catalog has **195** entries: 173 core, 21 later, one reference-only, with no duplicate catalog IDs. Exported shell/child targets explain the larger contract count of 227.

The Rust checks used the shared native checkout with production/test inputs verified unchanged against this candidate. Another reviewer added a visual-review-only commit to that checkout during this run. Publication was consequently isolated in a separate native clone at the exact candidate; the visual review commit is not a parent of this report.

No Linux canonical capture, macOS rendering, physical-display fidelity, Japanese IME cleanup, broad native matrix or visual approval is claimed. Worker claims of fully byte-identical fresh reproduction are **not independently confirmed**; the documented sequence above contradicts that claim in this environment. No unavailable environmental case is used as a blocker.

## Bounded findings

Paths below are relative to `reference/shadcn-gallery-v0.1/` unless stated otherwise.

### C01 — MAJOR: capture-specific metrics and styles are overwritten

**CONCRETE CONTRACT/TOOLING DEFECT:** `capture.js:651–660` appends each outer placement but replaces `elements[id].rec` on every Light capture. Lines 835–839 export only that last record's box, text and parts. The wide and collapsed SettingsNav therefore share one collapsed metric record. Independent browser measurement of wide `settings-nav.appearance` gives a 240×32 control, label part `(32,6,78.828125,20)` and visible text `(32,7,78.828125,18)`. The contract instead retains the collapsed label part `(-112,-1049.5,0,0)` and no text runs, even alongside the wide placement. The container similarly retains only its 48px sidebar, not the wide 256px sidebar geometry.

This also loses actual selection focus state: in the default Native Text capture, `native-text.single-line.selection` is focused with ring border and a 3px ring. In the multiline-selection capture it is unfocused. Its final contract box describes only the latter: input border and no shadow.

**WHY IT CAN BREAK NATIVE PARITY OR FREEZE TRUTH:** A native consumer cannot recover wide label/content placement or the default selection chrome from the declared contract without returning to DOM measurements or screenshots. Stable identity must not collapse distinct presentation measurements.

**EXACT ARTIFACT:** `scripts/capture.js:651–660,835–839`; `contract.json` entries `settings-nav.default`, `settings-nav.appearance` and `native-text.single-line.selection`.

**SMALLEST CORRECTION:** Retain metric/style snapshots for each applicable page/capture state, with theme-specific effective paint where needed. Keep automation IDs stable. Represent hidden parts explicitly rather than presenting their zero/off-document rectangles as visible-target geometry.

**PROOF REQUIRED:** Compare exported box/text/parts to the same post-state browser instances for both wide/collapsed Navigation and both selection captures. A fixture that overwrites one state's data with another must fail, even if all outer placements and part names remain correct.

### C02 — MAJOR: CORE checkbox labels have no measured owner

**CONCRETE CONTRACT/TOOLING DEFECT:** `catalog.js:609–618` puts the label span beside the checkbox automation root, outside any automation owner, with neither a semantic part nor an ID. Independent measurement of the unchecked label gives page rect `(228,1266.5,71.984375,14)` and effective `500 14px / 14px Geist`. No corresponding label/text-run metric is exported. `checkbox.default.unchecked` contains only the 16×16 control. Font proof includes the label, but font proof does not supply its missing geometry.

**WHY IT CAN BREAK NATIVE PARITY OR FREEZE TRUTH:** Checkbox label placement, control-to-label gap and baseline are CORE targets explicitly required by this review. A native implementation must currently guess them. The text validator iterates exported runs, so omission itself escapes its ink/baseline checks.

**EXACT ARTIFACT:** `src/catalog.js:609–618`; checkbox entries in `contract.json`; text ownership/filtering in `scripts/capture.js`.

**SMALLEST CORRECTION:** Give the label a stable semantic owner/part, or measure a semantic specimen composition containing both control and label while preserving the interactive control identity. Assert coverage of visible CORE specimen text, with explicit exceptions for gallery captions and intentionally hidden text.

**PROOF REQUIRED:** All checkbox labels receive nonempty, positive text bounds, font/line-height/baseline data and control-relative geometry. Removing one visible label's exported metrics must fail. Preserve the working Radio Group, Input/Textarea and overlay leaf-run measurements.

**Documentation correction:** README's baseline formula adds half-leading; implementation and contract notes use `Range line top + fontBoundingBoxAscent`. State one actual proxy definition. The control mirror also sets border width to zero, so its claim of *identical box styles* needs a precise account of the approximation or correction. No browser editing semantics are requested.

### C03 — MAJOR: token checks still accept invalid or unverified token output

**CONCRETE CONTRACT/TOOLING DEFECT:** In-memory replacement of `tokens.control_heights_px["button.default"]` with `"WRONG"` still yields zero validator errors across all 594 comparisons. `validate.js:213–219` validates the live number but not the parsed expected number; comparison against `NaN` silently passes. Replacing `shadows["shadow-md"]` with a valid but unrelated `0 100px 100px 0 rgb(255 0 0 / 1)` also yields zero errors because no binding consumes the overlay shadow token. There is an existing data contradiction: `shadow-sm` is advertised as “tabs trigger active,” while both exported active tab variants have `box_shadow=null`. Version validation excludes `tokens.json` entirely, despite README's claim of consistency across all committed JSON.

**WHY IT CAN BREAK NATIVE PARITY OR FREEZE TRUTH:** The claimed numeric/reference authority can silently supply unusable dimensions or different effects from the effective browser target. Correctly comparing 594 selected rows does not establish unchecked values or token schema identity.

**EXACT ARTIFACT:** `scripts/validate.js:6–20,213–219`; `scripts/bindings.js`; `scripts/tokens.js:221–225`; `tokens.json.shadows`; README validation claims.

**SMALLEST CORRECTION:** Reject nonfinite/unparseable expected numeric values as well as live values. Validate the token schema/version contract explicitly. Correct or clearly classify unused shadow entries and bind representative used overlay shadow parameters to the effective computed output, separating the surface ring from the actual blur/offset layers.

**PROOF REQUIRED:** The unchanged data passes. Wrong numeric token values, wrong token schema/version, and changed used shadow offset/blur/color/alpha fail the production check. A correct shadow record agrees with the browser and per-capture contract. No general effects subsystem is required.

### C04 — MAJOR: deleting required coverage passes; the fault test is confounded

**CONCRETE CONTRACT/TOOLING DEFECT:** Deleting only `coverage.families.button.states["hover:"]` returns no coverage errors. Deleting the whole Button family also returns no errors. The validator traverses only surviving mappings. `selftest.js:43–45` deletes hover **and** strips references from all remaining Button mappings; its failure therefore does not prove missing-rule detection.

**WHY IT CAN BREAK NATIVE PARITY OR FREEZE TRUTH:** A required CORE rule/family can disappear while the checker certifies coverage. This preserves the prior failure class of repeatably generating incomplete authority with passing validation. The policy re-scope to representative Later components is accepted and does not require expanding them.

**EXACT ARTIFACT:** `scripts/validate.js:25–53`; `scripts/selftest.js:41–46`; `coverage.json`; `scripts/coverage.js`.

**SMALLEST CORRECTION:** Check generated coverage against the existing authoritative bounded mapping and the catalog's CORE family set, including required rule presence. Keep one mapping authority; a regeneration/check mode can avoid duplicating the table. Test a single deleted required rule and a deleted family independently, without unrelated mutations.

**PROOF REQUIRED:** Those exact single mutations fail; intact 25-family/75-entry coverage passes; Later/reference-only tiers incur no new state-completeness requirement. Describe selftest results accurately: the current 17 checks include six positive controls and eleven negative cases, not 17 deliberately injected faults.

### C05 — MAJOR: documented fresh-clone byte stability does not reproduce

**CONCRETE CONTRACT/TOOLING DEFECT:** Four captures within the same native setup produced two Light Navigation PNG variants. The documented capture changed this tracked PNG and `SHA256.json`; both subsequent checks returned exit 1. A later isolated capture reproduced the first variant. Contract and CSS bytes stayed identical and the other 17 images matched the candidate.

```text
Committed / both check captures:
569746297c4dba09b15772f580ac703f3e90bdf445dbb4d2de61b204f632b69d

Fresh documented capture / additional isolated capture:
501400dc249242eaa4c2eccaaa6329694ae096396c47303a9b2ebb6d82ea27e5
```

The decoded difference is exactly eight pixels, maximum channel difference 1, inside the half-open region `[932,1016) × [1093,1119)` of the 1440×3019 image. This is not a taste, layout or visible-design finding.

**WHY IT CAN BREAK NATIVE PARITY OR FREEZE TRUTH:** Byte stability is the stated freeze/reproduction contract and is enforced by the current script. The documented workflow leaves a generated diff and fails despite an unchanged candidate/environment. It cannot presently be reported as independently reproducible byte-for-byte.

**EXACT ARTIFACT:** `screenshots/light/navigation.png`, `screenshots/SHA256.json`; screenshot capture in `scripts/capture.js`; `scripts/check.js:80–89`; README reproduction/determinism claims.

**SMALLEST CORRECTION:** Stabilize the specific raster/capture path and regenerate the affected authority. Document any required capture-environment constraint concretely. Do not silently weaken the existing byte-identity assertion or describe the discrepancy as a visual defect.

**PROOF REQUIRED:** An independent clean checkout runs the documented capture and two checks with no tracked generated diff and all checks green; repeated Navigation captures have one hash under the documented environment.

## Prior finding closure

| Prior | Non-visual disposition |
|---|---|
| R01 post-state geometry | CLOSED for the original pressed-button error; actual forcing precedes measurement and sampled pressed rects match. C01 is the separate loss of measurements after capture. |
| R02 text metrics | PARTIALLY CLOSED: control values/placeholders and compound leaf runs now exist, but C01/C02 prevent complete CORE target coverage. |
| R03 repeated parts | CLOSED for silent overwrite of duplicate semantic keys: duplicates fail and repeated Radio/Select/Dropdown/table/group parts are keyed. C01 concerns capture-state overwrite, not duplicate keys. |
| R04 token integrity | PARTIALLY CLOSED: backdrop 0.1, representative colors/alpha/dimensions/rings and real version checks improve proof; C03 remains. |
| R05 inline-start branch | CLOSED mechanically; visual judgment belongs to Opus. |
| R06 inventory/state scope | Re-scope accepted; CORE/LATER/REFERENCE_ONLY metadata is explicit and catalog-derived. Coverage proof remains PARTIALLY CLOSED through C04. No Later state expansion requested. |
| R07 theme machine | CLOSED: current-mode listener and independently observed transitions/subscription count are correct. |
| R08 provenance | CLOSED: neutral regeneration matches vendored `themes.ts`; local additions are separately labeled; 63 pinned Git blobs match. |
| R09 notices | CLOSED: self-contained Shadcn/Lucide/Feather/Geist notices and committed font attribution are present. |

## Other requested boundaries

- `PRODUCT_SCOPE.md`, `REVIEW_AUTHORITY.md`, reference README and metadata correctly require desktop responsiveness and exclude mobile product/platform behavior. Fixed capture dimensions do not imply fixed-size runtime layout. Wide/collapsed actions retain the same semantic IDs.
- One `window.CATALOG` owns specimens and tiers; page filtering and export metadata derive from it. Top-level automation IDs are unique and semantic. Child identities use named `data-part`/`data-key` chains, not CSS selectors or DOM paths. Fixed table row/cell keys are within the reference-only specimen.
- `COMPONENT_SUPPORT.md` explicitly distinguishes web reference, planned native API, Windows foundation primitives and unverified parity. No native Gallery family is marked verified merely because HTML exists. Its responsive `layout`/`modes` flags on planned rows should be read as target capabilities, not proof of a shipped native component; that distinction would benefit from an explicit vocabulary sentence.
- Canonical Light/Dark captures do not use System. Effective System mode is interactive only. Animations/transitions/caret are disabled, content has no random/date noise, dependency versions and lockfile are pinned, and rendering uses committed local assets.
- `reference/.gitattributes` pins text to LF and PNGs to binary. The original Windows worktree still contains some CRLF vendored files; comparison of **Git blobs**, not checkout newline translation, establishes the 63 verbatim matches. Fresh generated text/CSS was stable.
- No Node/browser/Tailwind dependency entered Rust; no CI/control-plane/native implementation was added. No tracked node_modules, browser binaries/cache, temporary debug files or developer-path dependency was found in the reference. Required OFL font assets are intentional, licensed content.
- Browser Native Text remains a visual/chrome/geometry target; native editing/input/IME semantics remain rust-ui platform authority. Reference tests make no Japanese IME prerequisite or administrative host mutation necessary.

## Universal Gates 1–20 — non-visual reference surface

These are the repository's existing twenty gates. They do not override Opus's separate visual decision or re-review the Rust foundation.

| Gate | Status | Evidence |
|---|---|---|
| 1 Requirements/completeness | FAIL | C01/C02 omit required effective CORE measurements; C05 reproduction fails. |
| 2 Assumptions/approval boundaries | PASS | Accepted Nova/font/tier/scope choices retained; visual authority respected. |
| 3 Contract preservation | FAIL | C01 collapses distinct capture-state targets; C03 token authority can disagree. |
| 4 Surgical scope | PASS | Review-only publication; Rust/reference inputs untouched. |
| 5 Minimal design | PASS | Existing catalog/exporter/validators can receive bounded corrections. |
| 6 Authority/ownership | FAIL | Visible text lacks a semantic owner; capture-specific authority is lost, C01/C02. |
| 7 Duplication/reuse | PASS | One specimen catalog, named keyed parts, shared validators; no second native authority. |
| 8 Unrequested fallback/shim | PASS | No production fallback or compatibility path added. |
| 9 Determinism/stable contracts | FAIL | C01 data loss and C05 two raster hashes under one documented setup. |
| 10 Authority/spec truth | FAIL | C03 shadow attribution and C02 baseline narrative disagree with effective/exported values. |
| 11 Failure safety | FAIL | C03/C04 silently accept invalid token data and missing coverage; otherwise check failures exit nonzero. |
| 12 Boundedness | PASS | Finite catalog/capture inventory and isolated tooling; no periodic runtime/browser dependency introduced. |
| 13 Dependencies/isolation | PASS | Exact Rust diff empty; tooling/assets remain within reference/dev surface. |
| 14 Redundant work | PASS | Frozen artifacts/check regeneration are intentional; no runtime work added. |
| 15 Persistence/infrastructure | N/A | No database/service/infrastructure milestone. |
| 16 Verification quality | FAIL | C01–C04 expose unchecked known defect classes; C05 contradicts fresh reproduction. |
| 17 Proportional recovery | PASS | Corrections remain bounded to existing reference data/checks/docs. |
| 18 Immutability/artifact identity | PASS | Exact candidate and upstream blobs reviewed; all committed hashes valid; report-only branch parent is exact candidate. Reproducibility failure is recorded separately. |
| 19 Artifact honesty | FAIL | Claims exceed verification in C02–C05; this report distinguishes execution from approval and font count from unique specimens. |
| 20 Systemic completeness | FAIL | Shared exporter overwriting and omission/check holes affect the reference authority, not only an isolated screenshot. |

## Freeze condition

Close C01–C05 in the existing reference, regenerate its pre-freeze artifacts and publish a new exact candidate. Demonstrate per-capture ownership, CORE text coverage, fail-closed token/coverage mutations and clean repeated byte reproduction. Preserve selected Nova/Geist, desktop tier policy and immutable Rust production. Opus must separately clear the visual side; this review prescribes no design changes.

CONTRACT_REFERENCE_REWORK_REQUIRED
