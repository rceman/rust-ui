// The native devctl E2E launch target for `tests/devctl_windows.rs`.
// Cargo supplies `CARGO_BIN_EXE_` for [[bin]] targets only, so the test
// launches Gallery through this alias. This IS the real Gallery
// application — the same compilation unit included verbatim — not a
// synthetic fixture and not a second implementation. The single
// authoritative Gallery source lives in `examples/gallery.rs`.
include!("../../examples/gallery.rs");
