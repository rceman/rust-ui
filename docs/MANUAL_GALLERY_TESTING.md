# Manual Gallery testing

The native Shadcn Gallery launches via a one-click Windows Desktop shortcut:

- **Shortcut:** `rust-ui Gallery.lnk` on the user's Desktop.
- **Source:** authoritative WSL checkout `/home/therceman/git/rust-ui`
  (via `\\wsl.localhost\Ubuntu\...`); never the abandoned `W:\devin_folder\rust-ui` tree.
- **Launcher:** `tools/windows/run-gallery.ps1` (committed). It runs the
  native Windows Cargo toolchain (`cargo.exe build --locked --features
  devtools --bin rust-ui-gallery`) into `W:\devin_folder\rust-ui-gallery-target`
  and starts `debug\rust-ui-gallery.exe`. On build failure it prints the
  cargo error and waits for Enter — it never launches a stale binary.
- **Manual run (no shortcut):**
  `powershell -NoProfile -ExecutionPolicy Bypass -File \\wsl.localhost\Ubuntu\home\therceman\git\rust-ui\tools\windows\run-gallery.ps1`
- **Recreate the shortcut:** run this in PowerShell —

  ```powershell
  $ws = New-Object -ComObject WScript.Shell
  $l = $ws.CreateShortcut("$([Environment]::GetFolderPath('Desktop'))\rust-ui Gallery.lnk")
  $l.TargetPath = 'powershell.exe'
  $l.Arguments = '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "\\wsl.localhost\Ubuntu\home\therceman\git\rust-ui\tools\windows\run-gallery.ps1"'
  $l.Save()
  ```

- **Errors:** build failures appear in the launcher console window. The
  shortcut runs hidden on success; keep the console visible (`-WindowStyle
  Hidden` removed) for diagnostics.
- `RUI_GALLERY_PAGE=<page>` and `RUI_SETTINGS_NAV_COLLAPSED=1` environment
  overrides stage a specific page / collapsed rail for testing.