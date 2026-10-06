# Shadcn reference v0.1 — final contract closure

**CONTRACT_REFERENCE_APPROVED**

**CONTRACT_FREEZE_SIDE_CLEARED**

## Identity and scope

| Identity | Value |
|---|---|
| Run / role | `RUI-A-261006-0810` / Astra Advisor |
| Reviewed candidate | `2e14e417a69ef8967b8fb17120282e3de6449fae` |
| Candidate parent | `411b96804732442d526be4db25d4636679fdd8ff` |
| Previous contract closure review | `443e62ebda1d7ec69f5cbd38bd0adc75bbe816c9` |
| Approved visual closure review | `dbe8c992b5f3742cbd8790a5e7bf8d59f8c2bfa0` |
| Frozen Rust production | `00dc29acfeb686d6a190d91624de7ab9a48e1e92` |
| Review branch | `review/shadcn-reference-v0.1-final-contract-astra`, based directly on the exact candidate |
| Started | `2026-10-06 08:05:04 UTC+03:00` |

This review closes only the remaining C02 metadata and C03 numeric/effect-domain defects and checks preservation of C01/C04/C05 and the approved visual artifacts. It makes no new visual judgment. Only this new review document is published; candidate files and previous reviews remain unchanged.

## Closure matrix

```text
C01 CLOSED — preserved
C02 CLOSED
C03 CLOSED
C04 CLOSED — preserved
C05 CLOSED — preserved
```

## C02 — truthful measurement metadata

Repository/current-reference searches found no remaining `mirror-div identical box/font styles` or equivalent identical-box-styles claim in current measurement descriptions. `scripts/measure.js`, README and generated contract notes now consistently describe effective font/text/padding measurement with the control border deliberately suppressed. The actual mirror still sets `border-width:0`.

Recursive comparison of parent and candidate contracts found exactly **66 changed `text_runs[].measure` descriptions**, with every other value unchanged. The baseline remains **line rect top + fontBoundingBoxAscent**, with the existing rounding. Checkbox label ownership, geometry, font metrics and 8px gap are unchanged. The fresh capture/check also passed the live ownership audit for 616 text nodes.

No measurement algorithm or geometry correction remains necessary.

## C03 — fail-closed numeric and effect domains

Source inspection confirms `scripts/validate.js::tokenNumOk` requires an actual number, `Number.isFinite`, and the relevant sign constraint. Expected numeric tokens are no longer accepted or compared through `parseFloat`. Browser-computed CSS strings still have their separate live-value parsing.

The existing traversal now visits scalar numeric leaves. The explicitly documented inherited/null and named `leading-snug` typography line-height entries remain recipe metadata; they do not bypass numeric binding validation. `validateShadowLayers` is shared by the token walk and binding validator, and is called before effect comparisons. It requires finite x/y/blur/spread/alpha, non-negative blur, alpha in [0,1], and a valid hexadecimal color. Signed offsets/spread remain supported.

### Independent mutation proof

Review-only probes outside the checkout called the actual production validators. Each test cloned canonical tokens and changed exactly one value. Canonical tokens and the height/shadow binding fixtures passed three baseline controls before the negative cases.

| Isolated mutation | Observed outcome |
|---|---|
| `control_heights_px["button.default"] = "32garbage"` | Both token and binding validators reject with the exact token path and invalid finite-number diagnostic. |
| Same height = `"32px"`, `"1foo"`, `"NaN"`, or `"Infinity"` | Each independently rejected by both validators. |
| `shadows["shadow-sm"].layers[0].alpha = NaN`, Infinity or -Infinity | Each independently rejected by both validators with an invalid finite-alpha diagnostic before comparison. |
| Each shadow x/y/blur/spread = NaN, Infinity or -Infinity | Each independently rejected by both validators for its own field. |
| Shadow alpha = -0.1 or 1.1 | Each independently rejected as outside the alpha domain. |

**22 isolated negative mutations passed their intended failure assertions.** NaN/Infinity proofs are in-memory domain tests, not claims that JSON can encode those literals.

The serializable `"32garbage"` counterexample was additionally run through the **complete production `scripts/check.js`**, the `reference:check` entry point. A task-only Node preload changed only that token's in-memory file read; no candidate file changed. The checker produced the intended token-domain and binding diagnostics and exited **1**, rather than certifying the artifact. Its other capture/integration checks continued successfully. This closes the former full-check exit-zero counterexample.

### Selftest truth

The fresh checker executed **11 explicitly labeled positive controls and 97 explicitly labeled negative mutations**. Inspection confirms the numeric-prefix case and nonfinite shadow-alpha cases are real isolated mutations with diagnostic assertions, not count-only checks. Existing schema/version, numeric-domain, structured-shadow, coverage and ownership failures remain exercised.

## Preservation and fresh-checkout reproduction

An independent native Windows clone was checked out at the exact candidate with no inherited `node_modules`. Environment: Windows 10.0.26200, Node v24.21.0, npm 11.19.0, Playwright 1.63.0, Chromium 153.0.8010.12.

Executed documented setup:

```text
npm ci
npx playwright install chromium
npm run reference:capture
npm run reference:check
```

All exited **0**. Native Git confirmed no tracked generated diff and the reproduction checkout remained at the exact candidate.

- **Visual artifacts:** all **18 PNG Git blobs** are byte-identical to `411b968`; fresh capture/check reproduced them. The existing Opus visual approval is preserved without a visual re-review.
- **C01:** capture records, IDs, geometry and ownership are unchanged apart from the truthful descriptions. Live capture-authority checks and wrong-state mutation proofs pass.
- **C04:** coverage data/authority are unchanged. Isolated missing Button-hover, missing-family, unknown-family and unknown-rule tests still fail as intended.
- **C05:** documented capture and checker re-capture reproduce canonical screenshot and contract bytes; deterministic launch settings remain unchanged.
- Tokens, coverage, reference metadata, bindings and compiled CSS remain byte-identical to the parent. No font, specimen, state or visual-value changes were introduced.

## Rust foundation boundary

Diff against frozen production is empty for `src/`, `examples/`, `Cargo.toml` and `Cargo.lock`. Native `cargo fmt --check` passed. Rust tests were not rerun for this final reference closure.

The previously reported parallel/DPI RichEdit test-isolation issue remains separately classified and unresolved; this review neither erases it nor claims the Rust suite is reliably green:

```text
SEPARATE_FOUNDATION_TEST_ISOLATION_DEFECT
NON_BLOCKING_FOR_REFERENCE_CONTRACT_FREEZE
```

## Decision

Both remaining contract defects are closed, with no demonstrated regression of C01/C04/C05 or the approved visual artifacts. No further bounded correction is required by this contract review. The contract freeze side is cleared; no native Gallery, devctl or merge work is performed here.

CONTRACT_REFERENCE_APPROVED
CONTRACT_FREEZE_SIDE_CLEARED
