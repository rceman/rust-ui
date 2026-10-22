# Manual Gallery testing

The native Shadcn Gallery launches via a one-click Windows Desktop shortcut:

- **Shortcut:** `rust-ui Gallery.lnk` on the user's Desktop.
- **Source:** authoritative WSL checkout `/home/therceman/git/rust-ui`,
  resolved at launch time via `wsl.exe wslpath -w` — the launcher does not
  hard-code a distro name; never the abandoned `W:\devin_folder\rust-ui` tree.
- **Launcher:** `tools/windows/run-gallery.ps1` (committed). It runs the
  native Windows Cargo toolchain (`cargo.exe build --locked --features
  devtools --bin rust-ui-gallery`) into `W:\devin_folder\rust-ui-gallery-target`
  and starts `debug\rust-ui-gallery.exe`.
- **Success path:** hidden PowerShell builds (incremental — fast if nothing
  changed) and exits; the Gallery window opens, no terminal is left behind.
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
  $l.TargetPath = 'powershell.exe'
  $l.Arguments = "-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File `"$repo\tools\windows\run-gallery.ps1`""
  $l.Save()
  ```

- `RUI_GALLERY_PAGE=<page>` and `RUI_SETTINGS_NAV_COLLAPSED=1` environment
  overrides stage a specific page / collapsed rail for testing.