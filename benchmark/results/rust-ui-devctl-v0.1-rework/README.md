# rust-ui-devctl v0.1 rework evidence — candidate `cf8ab919b399ca4465a17710741f1c888b84cfa0`

Bounded correction of `rust-ui-devctl` v0.1 per `W:\devin_folder\devctl-rework-design.md`:
D01 canonical comparison raster (raw vs canonical + `area-box/1`), D02 single
coordinate snap authority (`snap_rect` -> `Rect::physical`), D03 contract/0.2
adapter (tiers, parts, `--native-id`, `owner#part`), D04 unconditional
automation-ID invariants, D05 focus drain + `leave` command.

All outputs below were produced by a **fresh clone** of the candidate commit
(`git clone --no-checkout <repo> ... && git checkout cf8ab91`), verified
`git rev-parse HEAD == cf8ab91` and clean status, with its own `target/` dir.
Host scale **1.25 (DPI 120)** — the probe's `launch` output reports it.

## Layout

- `identity.json` — exact candidate SHA, host scale/dpi, toolchains.
- `windows-build.txt` — `cargo build --locked --features devtools --examples --bins` (clone).
- `fmt.txt` — `cargo fmt --check` (clone, exit 0).
- `windows-check-default.txt` / `windows-check-devtools.txt` — `cargo check --locked [–features devtools]` (exit 0).
- `windows-lib-parallel.txt` / `windows-lib-serial.txt` — `cargo test --locked --lib` (128/128 both).
- `windows-devtools-parallel.txt` / `windows-devtools-serial.txt` — `cargo test --locked --features devtools --lib` (156/156 both; the known
  `native_probe_richedit_paints_text` flake did NOT trigger in these runs).
- `invalid-automation-id.txt` — `cargo test --locked --test invalid_automation_id` (2/2; D04 proof without cfg(test)).
- `windows-devctl-windows.txt` — `cargo test --locked --features devtools --test devctl_windows -- --nocapture`
  (1/1; 3 launch/capture/shutdown cycles; handles 199→207 stable; idle CPU delta 0).
- `workflow-5cycles.ps1` — the scripted 5-cycle CLI workflow (authored evidence script).
- `cycles/` — one transcript per cycle (every command line + stdout JSON + exit code) + `summary.json`.
  Each transcript records: `hover`→`hovered:true`, `leave`→`hovered:false`, `key End`+`key Backspace`→
  `probe.input.label == "Calibration tex"`, self `compare-layout`/`compare-image` pass,
  cross-authority `compare-element` exit 6, contract `compare-layout` exit 6 (missing core ids).
- `probe/` — cycle-1 captures: `client.png` (full WGC client, 962×853 physical), `normal.png`,
  `hover.png`, `pressed.png` (element crops, 87×45 physical), `*.capture.json` metas, `snapshot.json`.
- `cross/` — cross-authority artifacts: `reference.png`, `native-raw.png`, `native-canonical.png`,
  `diff.png`, `comparison.json`.
- `layout-self.json` / `image-self.json` — extracted self-comparison results (both pass).
- `systemic-audit-test.txt` / `systemic-audit-counts.txt` — D01/D02 systemic audit over the frozen
  contract: **581 element records (421 old-vs-new disagreements), 806 visible-part records (578),
  999/1387 total** — counted by an independent scratch reimplementation (Rust half-away-from-zero
  semantics), matching the test's asserted constants.
- `linux-*.txt` — WSL2 Ubuntu cargo 1.93.1 on a copy verified content-identical to cf8ab91:
  `cargo test --locked --lib` 90/90; `cargo test --locked --features devtools --lib` 111/111
  (portable devtools tests incl. the systemic audit); `cargo check --locked --features devtools --bins` exit 0.
- `flake-summary.txt` — H3b CAPTURE_FAILED characterization (16/16 pass across both trees; cause
  not proven; pre-existing susceptibility).
- `frozen-reference.txt` — `HEAD:reference` tree == `dc70f66:reference` (`e0bd8eb…`); 18/18 PNG
  hashes match `screenshots/SHA256.json`; `git diff dc70f66 HEAD --stat -- reference` is empty.

## Verify

```sh
cd benchmark/results/rust-ui-devctl-v0.1-rework
python -c "
import json, hashlib, os
sha = json.load(open('SHA256.json'))
bad = []
for root, _, fs in os.walk('.'):
    for f in fs:
        if f == 'SHA256.json': continue
        p = os.path.join(root, f).replace(os.sep, '/').lstrip('./')
        if sha.get(p) != hashlib.sha256(open(os.path.join(root,f),'rb').read()).hexdigest():
            bad.append(p)
print('files:', len(sha), 'mismatched:', bad)"
```

## Cross-authority observed values (host scale 1.25)

`compare-element` frozen `button.default` (contract `components/default`, 70×32 canonical) vs live
`probe.button`: exit 6, `pass:false` — probe styling differs, honest failure. Raw native raster at
1.25 px/Dp: dims `[87,45]` == snapshot `rect_px`; canonical `69×36` at 1.0 (snap of the native logical
rect); `normalization.source_scale` 1.25, `resampler` `area-box/1`, footprint `{66.25,105,86.25,45}`;
`visual_canonical.missing_pixels` 312 = 70×36 − 69×32 (logical size delta only);
`visual_raw.computed` false. This is NOT a visual pass and does not claim glyph-identical raster.
