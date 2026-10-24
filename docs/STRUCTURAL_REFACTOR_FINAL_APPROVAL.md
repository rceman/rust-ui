# Pre-P12 Structural Refactor — Final Argon Approval

- **Project:** `rceman/rust-ui`
- **Candidate Reviewed:** `d11209e821b19ce9fa634df414f5f8d02e718d4c`
- **Verified Code SHA:** `a56f727301f399fefcc86f155d7d8840e7b970bc`
- **Production Base:** `3344c0cf53800ca8935f6fc4c0a68ccf37be46f4`
- **Previous Delta Review:** `142426ae2afb8364a53ca5c8be5c53429fe964d9`
- **Review Branch:** `review/pre-p12-structural-refactor-final-argon`
- **Decision:** `STRUCTURAL_REFACTOR_FINAL_APPROVED`
- **Merge Authorization (`refactor/pre-p12-structural-decomposition` -> `main`):** **AUTHORIZED**

---

## 1. Script Fixes Verification (`a56f727301f399fefcc86f155d7d8840e7b970bc`) — **PASS**

- **`.githooks/pre-commit` (`L48`):** Now includes `*/target-*/*|` in the path exclusion `case` pattern, matching `tools/check-file-tokens.sh`. Staged-index inspection (`git diff-index --cached -z --name-status --diff-filter=AMR` + `git cat-file -s ":$p"`) remains NUL-safe and fail-closed (`>= 20000` bytes fails with `exit 1`).
- **`tools/check-file-tokens.sh` (`L37-44`):** `BEGIN { lim=int(max/4) }` is initialized before row processing, and running `./tools/check-file-tokens.sh` exits `0` with `OK — every file under 5000 tokens`.
- **No Devin Stop Hook Reintroduced:** Confirmed `.devin/hooks.v1.json` and `tools/token-budget-gate.sh` remain absent.

---

## 2. Deterministic 12-State Equivalence Verification (`d11209e821b19ce9fa634df414f5f8d02e718d4c`) — **PASS (`12/12`)**

Independently inspected `benchmark/results/pre-p12-structural-refactor/equivalence.json`, `REPORT.md`, retained `before/`, `before_clean/` (captured from baseline `3344c0cf53800ca8935f6fc4c0a68ccf37be46f4`), and `after/` (captured from candidate `a56f727301f399fefcc86f155d7d8840e7b970bc`):

| State | Layout JSON SHA-256 (`before_clean` == `after`) | PNG SHA-256 (`before_clean` == `after`) | Viewport (px) | Mismatched Pixels |
|---|---|---|---|---|
| `light/components` | `947c4dc3026396d2` == `947c4dc3026396d2` | `4b247788c82dd156` == `4b247788c82dd156` | `964 × 1515` | `0` |
| `light/forms` | `b509347ff8413b2b` == `b509347ff8413b2b` | `2f0ac835c7005ed6` == `2f0ac835c7005ed6` | `964 × 1690` | `0` |
| `light/navigation` | `b10a4103418fb6b5` == `b10a4103418fb6b5` | `8f6dc1a53cbbe84d` == `8f6dc1a53cbbe84d` | `964 × 1176` | `0` |
| `light/overlays` | `21072044388c4bfb` == `21072044388c4bfb` | `aa571ce5980c904e` == `aa571ce5980c904e` | `964 × 1036` | `0` |
| `light/typography` | `e57da1a42b662d46` == `e57da1a42b662d46` | `7a10cad28eeb5c80` == `7a10cad28eeb5c80` | `964 × 775` | `0` |
| `light/native-text` | `660837dc56ed2b86` == `660837dc56ed2b86` | `14fe3075124aff94` == `14fe3075124aff94` | `964 × 1048` | `0` |
| `dark/components` | `8e17f49c85fcc49d` == `8e17f49c85fcc49d` | `aa3754e49a9f1f69` == `aa3754e49a9f1f69` | `964 × 1515` | `0` |
| `dark/forms` | `9e80ae1513218019` == `9e80ae1513218019` | `8d4b24fa55c113ee` == `8d4b24fa55c113ee` | `964 × 1690` | `0` |
| `dark/navigation` | `e904af70f0006be5` == `e904af70f0006be5` | `b378558b61e14257` == `b378558b61e14257` | `964 × 1176` | `0` |
| `dark/overlays` | `7eadb743cb5a1bee` == `7eadb743cb5a1bee` | `5b90816e8bad0524` == `5b90816e8bad0524` | `964 × 1036` | `0` |
| `dark/typography` | `2e44bd008c228578` == `2e44bd008c228578` | `cb6ce80dd6242076` == `cb6ce80dd6242076` | `964 × 775` | `0` |
| `dark/native-text` | `b8ab687c589eb37b` == `b8ab687c589eb37b` | `51d439eb974a828e` == `51d439eb974a828e` | `964 × 1048` | `0` |

- **12/12 semantic/layout snapshots:** Byte-for-byte identical (`0` semantic tree diffs, `0` geometry diffs, `0` part-visibility diffs).
- **12/12 WGC raster captures:** Byte-for-byte and pixel-for-pixel identical (`0` mismatched pixels).
- **Focus / hover / expansion / viewport states:** Identical across all 12 pairs (`shell.theme.{light,dark}` focused and hovered; `select.default.open.trigger` `expanded: true` on `light/overlays` and `dark/overlays`; `label.default` parts clean on `light/forms` and `dark/forms`).
- **Historical `before/` directory:** Retained intact alongside `before_clean/` and `after/`.

---

## 3. Scope, Integrity & Deferred Observations — **PASS**

- **Diff scope (`92821c7..d11209e`):** Touched only `.githooks/pre-commit`, `tools/check-file-tokens.sh`, and `benchmark/results/pre-p12-structural-refactor/**`. Zero production Rust files were modified.
- **P10 cross-monitor:** Structural-only verification in the single-display environment remains accepted (`G19`).
- **Deferred technical debt (recorded, not a refactor regression):** Pre-existing `Runtime::update_node` (`src/runtime/reconcile_nodes.rs:163-206`) behavior where unkeyed `Box` slot reuse across page switches preserves an earlier `n.part_key` (`"list"` from `Navigation`) is recorded as pre-existing technical debt outside the scope of this zero-behavior-change refactor.

---

## 4. Final Decision

```text
STRUCTURAL_REFACTOR_FINAL_APPROVED
P12_TOPOLOGY_READY
```
