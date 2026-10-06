# Shadcn reference v0.1 — independent contract closure review

## Verdict and immutable identities

**CONTRACT_REFERENCE_REWORK_REQUIRED**

This review is limited to C01–C05 and regressions directly introduced by their corrections. C01, C04 and C05 are closed. C02's actual text ownership/geometry is corrected, with one stale measurement description remaining. C03 still permits invalid token values through the production checker. The remaining correction set is small and requires no visual redesign, new architecture, native Gallery or devctl work.

| Identity | Value |
|---|---|
| Run / reviewer | `RUI-A-261005-2114` / Astra Advisor |
| Candidate | `411b96804732442d526be4db25d4636679fdd8ff` |
| Candidate branch | `agent/shadcn-gallery-reference-v0.1-opus55` |
| Review branch | `review/shadcn-reference-v0.1-contract-closure-astra`, created directly from the exact candidate |
| Previous contract review | `0d8c47df49853347fc0068cd13d83109474df069`, based on `2fe97eaf5438ea928a85224956248f859d6ac464` |
| Frozen Rust production | `00dc29acfeb686d6a190d91624de7ab9a48e1e92` |
| Foundation review base | `e827f3dfb5d29e50bcc64f90e5d735624641d878` |
| Review started | `2026-10-06 07:30:16 UTC+03:00` |

Only this new review document is committed. Candidate files, previous reviews, tests and evidence remain unchanged. Opus retains visual/UI authority; no aesthetic approval or rejection is made here.

## Closure matrix

```text
C01 CLOSED
C02 PARTIALLY_CLOSED
C03 PARTIALLY_CLOSED
C04 CLOSED
C05 CLOSED
```

## Independent execution and environment

Orchestration ran from Linux/WSL, with native execution on Windows `10.0.26200`, Node `v24.21.0`, npm `11.19.0`, Playwright `1.63.0`, Chromium `153.0.8010.12`. Two separate native Git clones were used: a clean detached reproduction checkout at the exact candidate and a clean publication checkout with the new review branch based directly on it. Neither imported ignored working-copy prerequisites. Review-only probe files and output directories were kept outside the publication checkout.

The documented procedure was executed in the fresh reproduction checkout:

```text
npm ci
npx playwright install chromium
npm run reference:capture
npm run reference:check
npm run reference:check
```

All exited successfully. Three additional complete captures were run consecutively into separate output directories. Independent byte/hash comparisons against the candidate's Git blobs established:

- Every one of the 18 PNGs matched the candidate in every retained output set.
- Both documented checks returned zero and reported all 18 screenshots byte-identical.
- The documented capture, both check captures and three additional captures produced one Light Navigation hash:

  `66d802896f7c136071b4f6080b3c6ab9320c3f51b65e2277fd97e7ccbba29c9c`

- `contract.json` remained byte-identical. The regenerated `tokens.json`, `coverage.json`, `reference.json`, `token-bindings.json` and compiled CSS matched candidate blobs.
- Native Git confirmed no tracked generated diff and HEAD remained the exact candidate. Extra capture output directories were scratch outputs in the reproduction clone, not publication changes.

The unmodified checks independently executed 602 token comparisons, a live audit of 227 multi-capture IDs, ownership checks for 616 text nodes, 282 same-state pairs, and selftests reporting **11 positive controls and 29 negative mutations** separately. Passing those existing tests does not cover the two independently demonstrated C03 counterexamples below.

## C01 — closed: capture ownership and state records

`contract/0.2` now stores each element's independent `captures["<page>/<state>"]` record. SettingsNav preserves the same action ID across its presentations:

| Browser / contract sample | Wide `navigation/default` | Collapsed `navigation/settings-nav-collapsed` |
|---|---|---|
| ID | `settings-nav.appearance` | `settings-nav.appearance` |
| Control rect | `(112,1125.5,240,32)` | `(112,1049.5,32,32)` |
| Sidebar width | 256px | 48px |
| Label part, control-local | `(32,6,78.828,20)` | `{visible:false, reason:"not-rendered"}` |

Independent browser values match the exported records within the declared rounding tolerance. Hidden parts are explicit non-rendered records rather than negative/zero visible targets.

The default Native Text capture retains single-line focus, selection `[0,6]`, the effective ring border and 3px ring. The alternate capture retains multiline focus and selection `[22,33]`; the single-line peer is unfocused there. Their records remain distinct. Default appearance on All and Native Text is consistent.

Independent single-change mutations copied collapsed appearance over wide and alternate Native Text appearance over default. The production validator rejected the former for rect/visibility disagreement and the latter for box/interaction disagreement. Existing same-state and live rendered-ID checks also passed. The prior overwrite counterexample is closed.

## C02 — actual metrics closed; one description remains

The checkbox label now has semantic ownership through `data-part-owner`, an exported `label` part and its own text run. Independent raw browser measurement of `checkbox.default.unchecked` agrees with the contract:

- Control/label gap: **8px**.
- Label part, control-local: `(24,1,71.984,14)`.
- Glyph run: `(24,-1,71.984,18)`, with baseline **13px** from the control top. The negative local y is the font-box extent; it is not an off-document rectangle.
- Geist, size **14px**, weight **500**, line-height **14px**.

Removing only that label's exported text run fails with the intended `text-ownership ... owner=checkbox.default.unchecked part=label has no contract text_run` diagnostic. Unowned visible text fails measurement; the allowed exemptions are explicitly gallery chrome. The existing live ownership audit passes.

README and contract notes correctly define the baseline as **line rect top + fontBoundingBoxAscent** and explain the border-suppressed input/textarea mirror. However, `scripts/measure.js:120` still generates:

```text
mirror-div identical box/font styles ...
```

That stale claim remains in the current input/textarea `captures[].text_runs[].measure` metadata, while the actual mirror deliberately sets border width to zero. This is the documentation correction already included in C02, not a new geometry defect.

**Remaining defect / severity:** MINOR, contradictory current measurement metadata.

**Smallest correction:** Update the existing `measure` description to match the border-suppressed approximation documented in README and contract notes, then regenerate the contract. No checkbox or visual change is required.

**Closure proof:** No current generated measurement record claims identical box styles; the documented baseline and working ownership/gap metrics remain unchanged.

## C03 — partially closed: token domain validation still has holes

The corrected implementation rejects missing height, `"WRONG"`, numeric `NaN`/Infinity at the bound height path, negative height, alpha outside `[0,1]`, wrong token schema/version/revision, and representative incorrect shadow x/y/blur/spread/color/alpha/layer order. These were independently mutated one at a time and rejected for the intended token/property reason.

Used shadows are structured and ordered. Live Popover/Dropdown/Select effect layers match `(0,4,6,-1)` then `(0,2,4,-2)`, black with alpha 0.1. The zero-offset/zero-blur surface and focus rings are excluded from those effect layers. `shadow-lg` is explicitly unused. The earlier missing shadow comparison is corrected.

Two exact domain counterexamples remain:

### A. Invalid numeric string passes the entire checker

```javascript
tokens.control_heights_px["button.default"] = "32garbage";
```

Both `validateTokens` and all 602 binding comparisons return no errors. `validateBindings` at `scripts/validate.js:361–363` accepts `parseFloat`'s numeric prefix and subsequently compares the parsed value 32, silently ignoring the invalid suffix.

This was also reproduced through the **full production `scripts/check.js`**, using a review-only Node preload that changes only its in-memory read of this token file. No candidate file was edited. The complete checker reported **all checks passed**, exit **0**. This is a valid JSON string, so it is an artifact-level invalid-token counterexample, not just malformed JSON syntax.

### B. Used-shadow alpha NaN bypasses validation

```javascript
tokens.shadows["shadow-sm"].layers[0].alpha = NaN;
```

Both production validators return no errors. The recursive token walk descends only into object children, missing numeric leaves such as this alpha. The shadow alpha guard checks type/range but not finiteness; `NaN` evades range comparisons. The subsequent comparison `Math.abs(actualAlpha - NaN) > tolerance` also evaluates false.

This second reproduction is an **in-memory NaN mutation**, as explicitly requested for validator proof; NaN itself is not a valid JSON literal. The first reproduction establishes a real serializable invalid artifact independently of that limitation.

**Remaining invariant / severity:** MAJOR — declared numeric/effect token values must be finite and fully valid before comparison; invalid data must never be certified through permissive prefix parsing or NaN arithmetic.

**Exact locations:** `scripts/validate.js:38–69` token walk; used-shadow alpha guard; `tokenNumOk` at 361–363; structured effect-layer comparisons at 415–428.

**Smallest correction:** Validate declared numeric tokens as finite numeric values (or fully parse any explicitly permitted numeric-string format), visit numerical leaves in the existing domain authority, and reject nonfinite alpha/effect fields before arithmetic comparisons. Retain one domain authority rather than adding a general schema/effects subsystem.

**Closure proof:** Add these two exact single-change mutations. Require an intended diagnostic, including nonzero full-check outcome for the valid-JSON height string. Preserve baseline passes and the already-working shadow/order/domain mutations.

## C04 — closed: bounded coverage authority

`scripts/coverage.js` is the single bounded mapping authority used for generation and verification. Required families derive from CORE contract/catalog metadata plus the gallery shell policy. Committed family names match this set; rule keys are exhaustively compared against the existing authority. Upstream hook-prefix claims are also checked against the pinned Nova source.

Independent mutations establish:

| Single mutation | Result |
|---|---|
| None | PASS |
| Delete only Button `hover:` | FAIL: missing rule and unclaimed upstream hover prefix |
| Delete only Button family | FAIL: missing family `button` |
| Add unknown family | FAIL: unknown family |
| Add unknown Button rule | FAIL: unknown rule |

The actual artifact contains **25 families, 78 named rule entries and 26 hook patterns**. The task's reported number 178 is not the raw rule-entry count in this candidate. There are also 114 selector claims; those are a different quantity. No missing required rule was demonstrated. This count correction is recorded without inventing extra coverage requirements.

LATER and REFERENCE_ONLY do not acquire CORE completeness obligations. Selftest positive controls and negative mutations are now labeled and counted separately; the old confounded deletion proof is replaced by isolated intended-error assertions.

## C05 — closed: canonical byte reproducibility

The independent fresh-checkout evidence above closes the prior alternating-hash counterexample for the corrected candidate and documented environment. Byte equality has not been weakened.

Every capture/check browser launch uses `scripts/browser.js::launchCanonical`, including the required `--disable-partial-raster` argument. `reference:check` verifies that argument is present. Repository search of the reference scripts finds the single native Chromium launch there, so checker sessions cannot quietly use an alternate flag set. The Worker’s partial-raster diagnosis is consistent with the successful correction; this review establishes repeated current-candidate reproduction, not a universal claim about every Chromium/environment combination.

## Capture sequencing regression check

Each capture loads a fresh document with its exact page/state query. The state table is applied by `render.js` during mount, including All when the selection specimen is present. After readiness/fonts, the Light/default font proof runs and removes its marker/mirror mutations. CDP forcing then runs, followed by three animation frames and font readiness. State is asserted before measurement and again immediately before the screenshot. Measurement, binding/pressed proof, screenshot, ink/pixel checks and capture-key storage preserve the same page/state identity; sessions detach and theme contexts close.

The concrete source ordering has two harmless bookkeeping differences from a conceptual pipeline diagram: state setup occurs during mount, before font proof, and records are accumulated after measurement before screenshot validation completes. The state assertions guard the effective state, and any accumulated error makes capture fail. Six successful complete captures, independent live comparisons and wrong-state mutations establish no state leakage or renewed pre-state measurement bug. These differences do not introduce a new contract finding.

## Rust foundation classification only

Git diff against the foundation review base is empty for all Rust source/tests/examples, Cargo manifest/lockfile and Rust build/toolchain/configuration surfaces. Outside documentation/reference files, the reference milestone adds only scope/review guidance in `AGENTS.md`. It introduces no Rust build or test configuration. Frozen production remains `00dc29a`.

Independent native Windows invocations in the clean candidate checkout produced:

| Invocation | Result |
|---|---|
| `cargo fmt --check` | PASS |
| `cargo test --locked --lib` | 121/121 PASS in this invocation |
| Fully qualified exact `tests::native_probe_richedit_paints_text` filter | 1/1 PASS; 120 filtered |
| Full suite with `--test-threads=1` | 121/121 PASS |

The Worker’s reported **120/121 parallel failure at host scale 1.25 / DPI 120 remains unresolved**. It was not reproduced by my parallel invocation. Passing individual invocations does not erase the intermittent defect or certify the foundation test suite as reliably green.

Classification of the reported issue:

```text
SEPARATE_FOUNDATION_TEST_ISOLATION_DEFECT
NON_BLOCKING_FOR_REFERENCE_CONTRACT_FREEZE
```

The host-DPI failure account is Worker-reported; the source/configuration neutrality and the invocation results above are independently verified. No evidence connects this reference correction to the Rust failure. It belongs to a separate foundation-hardening task; no frozen Rust change is made here.

## Gates 1–20 — closure surface only

Exactly the existing twenty gates are used. Accepted architecture, visual choices and unrelated R01–R09/foundation findings are not re-audited.

| Gate | Status | Closure evidence |
|---|---|---|
| 1 Completeness | FAIL | C02 metadata and C03 fail-closed requirement remain. |
| 2 Assumptions/approval | PASS | Exact candidate and narrow, non-visual scope retained. |
| 3 Contracts | FAIL | C03 invalid typed quantities can still be certified. |
| 4 Scope | PASS | Only the new independent review is published. |
| 5 Minimal design | PASS | Remaining corrections fit existing descriptions/domain validators. |
| 6 Ownership/authority | PASS | Per-capture records and semantic text ownership corrected; bounded coverage authority shared. |
| 7 Reuse/duplication | PASS | Shared measurement/validation; one coverage map; no duplicate specimen authority introduced. |
| 8 Fallback/shims | PASS | No unrequested production fallback. |
| 9 Stable semantics | FAIL | Byte determinism is closed; C03 still admits invalid numeric-domain data. |
| 10 Canonical truth | FAIL | C02 generated measurement wording contradicts the documented approximation. |
| 11 Failure safety | FAIL | C03 negative inputs return success, including a full-check counterexample. |
| 12 Boundedness | PASS | No related regression demonstrated. |
| 13 Isolation/dependencies | PASS | Rust-neutral diff; reference tooling remains isolated. |
| 14 Redundant work | PASS | No related regression demonstrated. |
| 15 Infrastructure | N/A | No infrastructure change. |
| 16 Proof sufficiency | FAIL | Existing 29 negatives omit two demonstrated C03 holes. Other closure proofs pass. |
| 17 Proportional recovery | PASS | Two bounded corrections, no architecture/design restart. |
| 18 Identity/immutability | PASS | Exact source, clean tracked reproduction, report-only branch from candidate. |
| 19 Artifact honesty | FAIL | Stale C02 metadata and overbroad token-domain guarantee; counts/results corrected explicitly here. |
| 20 Systemic completeness | FAIL | C03 traversal/parser issue is the remaining known validator class, not an unrelated audit expansion. |

## Smallest remaining correction set

1. **C02:** Replace the stale per-run mirror measurement description and regenerate it.
2. **C03:** Close strict numeric parsing/finite-leaf validation, including used-shadow alpha, and prove the two exact mutations fail through the existing checks.

Keep C01, C04 and C05 closed. No additional visual states, Later-tier expansion, native implementation, IME host cleanup, CI or general verification framework is required by this review.

CONTRACT_REFERENCE_REWORK_REQUIRED
