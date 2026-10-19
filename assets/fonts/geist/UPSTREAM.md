# assets/fonts/geist provenance

Native-only font asset for the Windows backend. The frozen HTML reference
(`reference/shadcn-gallery-v0.1/vendor/geist/`) ships WOFF2 — the canonical
comparison font. DirectWrite/GDI font registration on Windows requires TTF;
this directory carries the TTF build of the SAME canonical release.

- Package: `geist@1.7.2` (Vercel Geist font, npm) — identical package as the
  frozen reference (`reference/shadcn-gallery-v0.1/vendor/geist/UPSTREAM.md`).
- Tarball: `https://registry.npmjs.org/geist/-/geist-1.7.2.tgz`
- npm integrity (package-lock.json): `sha512-Gu5lDFa3pLRyoBlBPf0QIFHVdWAnpco7fS1bJm41jyLPFoguBgiubseUN2oLXMgqZ7uxAxDoXcHMhCY/fOTTgg==`
- License: SIL Open Font License 1.1 — `LICENSE` is the package's
  `LICENSE.txt` copied verbatim. OFL permits redistribution of the font
  files themselves.
- The Geist variable fonts have a single `wght` axis (100-900); weight
  mapping: 400 normal / 500 medium / 600 semibold / 700 bold / 800 extrabold.

## Verbatim files (byte-identical to the published package)

| File | Package source | sha256 | bytes |
|---|---|---|---|
| `Geist-Variable.ttf` | `dist/fonts/geist-sans/Geist-Variable.ttf` | cdcc4815cbf5f9882fa74e48f8ab410a0495781a58ff7316570f664e7e987753 | 169056 |
| `GeistMono-Variable.ttf` | `dist/fonts/geist-mono/GeistMono-Variable.ttf` | 0e1af3f507a1c8dfbb03d13ffad585834cd45ed7ccb78c756c7ce7873d180d30 | 171200 |
