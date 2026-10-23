# Manual Gallery testing

The native Shadcn Gallery launches via a one-click Windows Desktop shortcut:

- **Shortcut:** `rust-ui Gallery.lnk` on the user's Desktop — targets
  `wscript.exe tools\windows\run-gallery.vbs`, a GUI-subsystem wrapper so
  NO console window is ever created (powershell.exe -WindowStyle Hidden
  still flashes a console before hiding — the VBS removes that flash).
- **Source:** authoritative WSL checkout `/home/therceman/git/rust-ui`,
  resolved at launch time via `wsl.exe wslpath -w` — the launcher does not
  hard-code a distro name; never the abandoned `W:\devin_folder\rust-ui` tree.
- **Launcher:** `tools/windows/run-gallery.ps1` (committed). On a clean
  unchanged repo it launches the already-built
  `W:\devin_folder\rust-ui-gallery-target\debug\rust-ui-gallery.exe`
  **immediately** (a build stamp records HEAD + profile — clean worktree +
  matching HEAD + binary present → no cargo run). A dirty worktree or a
  HEAD change triggers a **headless** cargo build (`--locked --features
  devtools --bin rust-ui-gallery --manifest-path <resolved UNC>`) via
  `ProcessStartInfo` with `CreateNoWindow` — no terminal/conhost window on
  any path.
- **Success path:** hidden PowerShell exits; the Gallery window opens in
  ~1–2s on the fast path, a few seconds on a rebuild; no terminal is left
  behind.
- **Failure path:** a visible native message box shows the cargo error tail;
  the Gallery is not launched and no invisible process waits for input.
- **Manual run (no shortcut):**
  `powershell -NoProfile -ExecutionPolicy Bypass -File <resolved-repo>\tools\windows\run-gallery.ps1`
  where `<resolved-repo>` is `wsl.exe wslpath -w /home/therceman/git/rust-ui`.
- **Recreate the shortcut:** run this in PowerShell —

  ```powershell
  $repo = (& wsl.exe wslpath -w /home/therceman/git/rust-ui).Trim()
  $ws = New-Object -ComObject WScript.Shell
  $l = $ws.CreateShortcut("$([Environment]::GetFolderPath('Desktop'))\rust-ui Gallery.lnk")
  $l.TargetPath = 'wscript.exe'
  $l.Arguments = "`"$repo\tools\windows\run-gallery.vbs`""
  $l.Save()
  ```

- `RUI_GALLERY_PAGE=<page>` and `RUI_SETTINGS_NAV_COLLAPSED=1` environment
  overrides stage a specific page / collapsed rail for testing.