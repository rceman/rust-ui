# rust-ui-devctl v0.1 — native calibration evidence

**Status: RUST_UI_DEVCTL_V0_1_READY_FOR_NATIVE_GALLERY**

Implementation: `6375a4e537f0ec396c92154738f7bfced4324c5c`.
Base publication: `dc70f6644c0384170f3a6929eb2caaf33fc711a0`.
Collection: native Windows 10.0.26200, rustc 1.94.0, actual DPI 120 / scale 1.25.
The subsequent publication commit adds this evidence/report, not another implementation.
[identity.json](identity.json) binds the source, both executable SHA256 hashes,
collection UTC, compiler/environment and exit statuses. [SHA256.json](SHA256.json)
hashes the evidence files (excluding itself). Repository attributes pin receipt
text to LF so hashes also survive a native Windows checkout. No binary, token or live session receipt is committed.

## Executed checks

| Check | Result | Receipt |
|---|---|---|
| cargo fmt --check | PASS | [fmt.txt](fmt.txt) |
| Windows default all-targets check | PASS | [windows-default-check.txt](windows-default-check.txt) |
| Windows default library, parallel | 123/123 | [windows-default-tests.txt](windows-default-tests.txt) |
| Windows devtools all-targets check/build | PASS | [windows-devtools-check.txt](windows-devtools-check.txt), [windows-build.txt](windows-build.txt) |
| Windows devtools library, parallel | 139/139 | [windows-devtools-tests.txt](windows-devtools-tests.txt) |
| Linux default library | 87/87 | [linux-default-tests.txt](linux-default-tests.txt) |
| Linux devtools library | 98/98 | [linux-devtools-tests.txt](linux-devtools-tests.txt) |
| Linux devtools all-targets check | PASS | [linux-check.txt](linux-check.txt) |
| Real CLI/native workflow, three complete sessions | PASS, 28.48 s | [workflow.txt](workflow.txt) |

Logs preserve command warnings and test results, normalized from PowerShell UTF-16
to UTF-8/LF with trailing whitespace removed. The removed temporary output directory
is represented by a placeholder;
no measured values or failures were removed. Formatting succeeds with empty output.
Linux is portable helper verification, not a Linux UI/backend claim.

## Native proof

`tests/devctl_windows.rs` launches the actual CLI and probe, performs authenticated
pipe discovery, waits runtime/presentation idle, inspects IDs and physical/logical
rectangles, and exercises hover, held press, release, click, keyboard focus/Enter,
and native End/Backspace. Shared application state reaches `Clicks: 3` and committed
input `Calibration tex`. Missing IDs and an incorrect nonce fail explicitly.

Each session captures WGC client and button images, verifies PNG contents and
physical client dimensions, compares deterministic snapshots, runs layout self/+1 Dp
negative controls, image self/one-pixel negative controls, and writes element crops
and a diff. It then waits for process exit, removes its receipt and verifies stale
receipt cleanup. All three sessions and their temporary directories are removed.

| Artifact | Meaning |
|---|---|
| [client-0.png](probe/client-0.png) | WGC client content, 962 × 853 physical pixels |
| [client-0.capture.json](probe/client-0.capture.json) | WGC item 964 × 892, measured client offset (1,38), scale 1.25, stable retained scene |
| [normal-0.png](probe/normal-0.png), [hover-0.png](probe/hover-0.png), [pressed-0.png](probe/pressed-0.png) | Distinct real button states, 87 × 45 physical pixels; held press before release |
| [snapshot-0.json](probe/snapshot-0.json) | Deterministic retained semantic scene after keyboard editing |
| [changed-0.json](probe/changed-0.json) | Intentional +1 Dp button x mutation, comparator exits 6 with delta +1 |
| [pixel-0.png](probe/pixel-0.png) | Intentional first-pixel mutation, one mismatch with bbox (0,0,1,1) |
| [element/reference.png](probe/element/reference.png), [native.png](probe/element/native.png), [diff.png](probe/element/diff.png) | Generic snapshot fixture element comparison; exact zero diff |

The actual client and state crops were visually inspected: text is aligned with
chrome and controls; no text overlap was observed. This is calibration, not Shadcn
visual parity. An additional read-only comparison against frozen
`components/default`, ID `button.default`, correctly reports one missing node
(exit 6), since this generic probe intentionally has no Shadcn IDs.

## Bounded resource/idle observation

Each target starts at 225 handles, reaches 233 after native focus/editing warmup,
and remains **233 throughout six subsequent captures**. Each two-second idle sample
records zero process CPU tick increase, `native_timer_fires=0` and unchanged
`requested_redraws=71`. These are short smoke measurements, not an exhaustive COM,
GPU allocation or long-duration leak/performance guarantee. CLI processes exit;
the probe process exits on shutdown. Per-capture native ownership also uses RAII.

## Preservation and limits

The entire frozen `reference/shadcn-gallery-v0.1/` tree is unchanged from the base;
all 18 PNG Git blobs equal approved payload `2e14e417a69ef8967b8fb17120282e3de6449fae`.
No Gallery, MCP, web server, CI, other-platform backend or main merge occurred.

The separately recorded `native_probe_richedit_paints_text` parallel/DPI120 isolation
issue remains unresolved. Both final Windows parallel observations passed; that
does not establish that the pre-existing intermittent issue has been fixed.

See [DEVCTL.md](../../../docs/DEVCTL.md) for reproducible commands, supported key
subset, capture requirements and explicit comparison/idle limitations.
