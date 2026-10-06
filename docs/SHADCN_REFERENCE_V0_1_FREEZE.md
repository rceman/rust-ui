# rust-ui Shadcn reference v0.1 — canonical freeze

**SHADCN_REFERENCE_V0_1_FROZEN**

## Approved authority

| Identity | Frozen value |
|---|---|
| Reference version | `0.1` |
| APPROVED_REFERENCE_PAYLOAD | `2e14e417a69ef8967b8fb17120282e3de6449fae` |
| Visual approval | `dbe8c992b5f3742cbd8790a5e7bf8d59f8c2bfa0` — [exact review](SHADCN_REFERENCE_V0_1_VISUAL_CLOSURE_REVIEW.md) |
| Visual review base | `411b96804732442d526be4db25d4636679fdd8ff`; all 18 PNG blobs are identical in the approved payload |
| Contract approval | `0b90e755ff97c8bed0f6b727b8381bb2808bd1d1` — [exact review](SHADCN_REFERENCE_V0_1_FINAL_CONTRACT_REVIEW.md) |
| Frozen Rust production | `00dc29acfeb686d6a190d91624de7ab9a48e1e92` |
| Foundation approval | `e827f3dfb5d29e50bcc64f90e5d735624641d878` |
| Shadcn upstream | `shadcn-ui/ui @ 295a1f114a138f23b5dfee0e0c6812394dfeb90c` |
| Visual configuration | Base UI / Nova / neutral / Lucide / default radius |
| Canonical fonts | Geist Sans (CSS family `Geist`) + Geist Mono; `geist@1.7.2` |
| Canonical capture | 1440×1000 CSS px viewport; DPR 1; zoom 100%; 1 CSS px = 1 rust-ui logical Dp |
| Screenshots | 18 canonical Light/Dark PNGs |
| Contract schema | `rust-ui.shadcn-reference.contract/0.2` |
| Approved generation marker | Existing `candidate_revision: 3` retained across artifacts as provenance, not pending review |
| Publication run | `RUI-W-261006-0820` |

`FREEZE_PUBLICATION_HEAD` is the Git commit adding this record, the exact final
review documents and the frozen-status transition. Resolve its immutable SHA:

```bash
git log --diff-filter=A -1 --format=%H -- docs/SHADCN_REFERENCE_V0_1_FREEZE.md
```

That later publication commit is distinct from `APPROVED_REFERENCE_PAYLOAD`.
Neither reviewer reviewed the later publication SHA. Publication changes only
documentation and status metadata, including the capture generator's status
literal; reference version, approved geometry and visual values are unchanged.

## Approved payload identities

These hashes were recorded from Git blobs at the approved payload SHA **before**
publication changes. Paths are relative to `reference/shadcn-gallery-v0.1/`.
The existing `screenshots/SHA256.json` remains unchanged and contains the
individual canonical PNG hashes.

| Artifact | Approved SHA256 |
|---|---|
| `contract.json` | `1f29bab5639ec5438776f4a8672d1dee777dcbaea09110451f77182b735071ef` |
| `tokens.json` | `aa0dc852873aa1a4966cee7eb4aad22df953822ec821a199d9c310be8d157970` |
| `coverage.json` | `213feecc7ec516af8781a71b2033ccab5fa39e459ab8344bd7a31192311e904c` |
| `token-bindings.json` | `8d56aaffc8c603300eb84b9bb11dd67255155470cdf5251a27f2910a29527dae` |
| `static/gallery.css` | `258932331408b12d626785f286e338473b523490f5ca2034f126d6480c145537` |
| `reference.json` | `67306c93d52c0455a34d81c004ff94bf9362d4f0933ad1b8848bb30ec94f5917` |
| `screenshots/SHA256.json` | `d7a13e19201aedde80449d85ea2af173baec5bb418c3b554c102260c2a94db62` |

| Git tree | Approved object ID |
|---|---|
| Entire reference directory | `64c1ccae5a6acc1d3950cb83b851e2ce7b067c77` |
| `screenshots` | `5ce11ac9a1fa46515dca7c02d285a0cdccf23c25` |
| `vendor/geist` | `ed52da18c09dddf426fd289760c0314413181189` |
| `vendor/shadcn` | `00fa797aa3035957d28af588d639e9b81e5c1d24` |
| `vendor/lucide` | `c66b2255add50cd5bd15bdf4685b19bb9d6f1665` |

Only `reference.json.status` changes in the approved generated metadata, to
`SHADCN_REFERENCE_V0_1_FROZEN`; its table hash above identifies the reviewed
pre-publication file. All other approved artifact hashes and the screenshot,
font and vendor trees remain unchanged.

## Active freeze policy

- The HTML reference is now immutable authority for rust-ui Shadcn reference
  v0.1. An intentional future change to frozen visual/reference authority
  requires a new reference version under the existing policy, fresh
  capture/check and renewed split approval.
- Native implementations must not silently modify v0.1 reference artifacts to
  make native parity easier. Tokens are frozen comparison data; Rust's typed
  style/theme model remains production authority.
- CORE / LATER / REFERENCE_ONLY remain exactly as classified in the approved
  catalog. Reference completeness remains tier-specific; no native support
  status is promoted by this freeze.
- rust-ui remains desktop-only and fully desktop-responsive. The canonical
  capture viewport does not impose a fixed runtime window size.
- Canonical fonts remain Geist Sans + Geist Mono. Visual/UI authority remains
  Opus-reviewed; contract/tooling authority remains Astra-reviewed.

## Cleared next work

```text
VISUAL_FREEZE_SIDE_CLEARED
CONTRACT_FREEZE_SIDE_CLEARED
NATIVE_GALLERY_IMPLEMENTATION_CLEARED
RUST_UI_DEVCTL_IMPLEMENTATION_CLEARED
```

The two implementation clearances authorize work to start. They do not mean
native Gallery/components or devctl are implemented. This publication neither
starts those tasks nor merges main or tags a rust-ui release.
