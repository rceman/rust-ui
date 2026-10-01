# native.ps1 — shared evidence helpers for the composer spike.
# Dot-source from collect.ps1 and probes:  . "$PSScriptRoot\native.ps1"
#
# Everything is deterministic PostMessage/UIA — NO SetForegroundWindow,
# SendKeys or real mouse_event (those race window-manager focus and are
# the #1 source of flake). WM_CHAR/WM_KEYDOWN and WM_LBUTTON* go straight
# to the app WndProc — the same dispatch path real input takes.

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class NW {
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern bool PostMessageW(IntPtr h, uint m, UIntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern uint GetDpiForWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern IntPtr SetThreadDpiAwarenessContext(IntPtr ctx);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, IntPtr extra);
  public struct RECT { public int l, t, r, b; }
  public struct POINT { public int x, y; }
}
"@

# Gate 16 environment equivalence: the composer is Per-Monitor-V2 — this
# harness must be too, or GetWindowRect/UIA/ClientToScreen return VIRTUALIZED
# logical coords that disagree with the target's physical-px client space
# (98 vs 96-DPI virtualization silently corrupts every posted coordinate).
[NW]::SetThreadDpiAwarenessContext([IntPtr]-4) | Out-Null

$script:OutDir = "W:\devin_folder\rust-ui\benchmark\results\windows-native-composer-spike-v0.1"
$script:ComposerExe = "W:\devin_folder\rust-ui\target\debug\examples\composer.exe"
# repo-local Rust harness — owns every native semantic (geometry, UIA,
# input, DPI injection); ps1 calls it for measurement/invariant checks
$script:ProbeExe = "W:\devin_folder\rust-ui\target\debug\examples\native_probe.exe"

# ---- lifecycle ------------------------------------------------------------

function Kill-Composers {
  # leftover windows from interrupted runs — the "bunch of windows" bug
  Get-Process composer -ErrorAction SilentlyContinue | Stop-Process -Force
}

function Launch-Composer([hashtable]$Env = @{}) {
  Kill-Composers
  foreach ($k in $Env.Keys) { [Environment]::SetEnvironmentVariable($k, $Env[$k], 'Process') }
  $p = Start-Process -FilePath $script:ComposerExe -PassThru
  foreach ($k in $Env.Keys) { [Environment]::SetEnvironmentVariable($k, $null, 'Process') }
  Start-Sleep -Milliseconds 1800
  if ($p.HasExited) { throw "composer exited early (code $($p.ExitCode))" }
  [NW]::ShowWindow($p.MainWindowHandle, 9) | Out-Null
  return $p
}

# ---- capture --------------------------------------------------------------

function Shot-PrintWindow($h, $name) {
  # PrintWindow — offscreen-safe, DIP-space raster (client only)
  $bmp = New-Object System.Drawing.Bitmap 1000, 950
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $dc = $g.GetHdc()
  [NW]::PrintWindow($h, $dc, 2) | Out-Null
  $g.ReleaseHdc($dc); $g.Dispose()
  $bmp.Save("$($script:OutDir)\$name"); $bmp.Dispose()
}

function Shot-Screen($h, $name) {
  # CopyFromScreen — real device pixels including border, DPI-correct
  $wr = New-Object NW+RECT
  [NW]::GetWindowRect($h, [ref]$wr) | Out-Null
  $bmp = New-Object System.Drawing.Bitmap ($wr.r - $wr.l), ($wr.b - $wr.t)
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($wr.l, $wr.t, 0, 0, $bmp.Size)
  $g.Dispose(); $bmp.Save("$($script:OutDir)\$name"); $bmp.Dispose()
}

function Shot-Crop($file, $x, $y, $w, $h, $name) {
  $bmp = [System.Drawing.Bitmap]::FromFile("$($script:OutDir)\$file")
  $crop = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($crop)
  $g.DrawImage($bmp, (New-Object System.Drawing.Rectangle 0,0,$w,$h),
               (New-Object System.Drawing.Rectangle $x,$y,$w,$h),
               [System.Drawing.GraphicsUnit]::Pixel)
  $g.Dispose(); $crop.Save("$($script:OutDir)\$name"); $crop.Dispose(); $bmp.Dispose()
}

# ---- deterministic input --------------------------------------------------

function Post-Click($h, [int]$cx, [int]$cy) {
  # WM_LBUTTONDOWN/UP in CLIENT px — real hit-test path, no foreground needed
  $lp = [IntPtr]((($cy -band 0xffff) -shl 16) -bor ($cx -band 0xffff))
  [NW]::PostMessageW($h, 0x0201, [UIntPtr]::Zero, $lp) | Out-Null
  Start-Sleep -Milliseconds 60
  [NW]::PostMessageW($h, 0x0202, [UIntPtr]::Zero, $lp) | Out-Null
  Start-Sleep -Milliseconds 200
}

function Post-Text($h, [string]$text) {
  # WM_CHAR ONLY per UTF-16 code unit — posting WM_KEYDOWN too makes
  # msftedit double-insert (keydown unshifted + char shifted: "xXyY").
  # Surrogate pairs land as two chars, which is what a real IME produces.
  foreach ($c in $text.ToCharArray()) {
    $w = [UIntPtr][uint32][uint16][char]$c
    [NW]::PostMessageW($h, 0x0102, $w, [IntPtr]::Zero) | Out-Null
    Start-Sleep -Milliseconds 30
  }
  Start-Sleep -Milliseconds 150
}

function Post-Key($h, [uint32]$vk, [switch]$ctrl, [switch]$shift) {
  # Modifier chords can't be faked in the message itself — msftedit reads
  # the REAL keyboard via GetKeyState. keybd_event pokes the global key
  # state (no foreground needed); the posted WM_KEYDOWN then sees the
  # modifier as held. Posted KEYDOWN for a printable vk must still reach
  # the peer — its richedit keydown path checks the real modifier state.
  if ($ctrl)  { [NW]::keybd_event(0x11, 0, 0, [IntPtr]::Zero) }
  if ($shift) { [NW]::keybd_event(0x10, 0, 0, [IntPtr]::Zero) }
  Start-Sleep -Milliseconds 40
  [NW]::PostMessageW($h, 0x0100, [UIntPtr]::new($vk), [IntPtr]::Zero) | Out-Null
  Start-Sleep -Milliseconds 60
  [NW]::PostMessageW($h, 0x0101, [UIntPtr]::new($vk), [IntPtr]::Zero) | Out-Null
  if ($shift) { [NW]::keybd_event(0x10, 0, 2, [IntPtr]::Zero) }
  if ($ctrl)  { [NW]::keybd_event(0x11, 0, 2, [IntPtr]::Zero) }
  Start-Sleep -Milliseconds 120
}

function Post-DpiChanged($p, [uint32]$dpi) {
  # synthetic WM_DPICHANGED — deterministic regression evidence (NOT a real
  # monitor transition). All native math (process memory, suggested-rect
  # scale, message) lives in the Rust harness; this is orchestration only.
  & $script:ProbeExe dpi-changed $p.MainWindowHandle $p.Id $dpi | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "native_probe dpi-changed failed ($LASTEXITCODE)" }
}

# ---- UIA ------------------------------------------------------------------

function Uia-Root($h) {
  [System.Windows.Automation.AutomationElement]::FromHandle($h)
}

function Uia-Find($el, $name, $type = $null) {
  if ($el.Current.Name -eq $name -and
      ($null -eq $type -or $el.Current.ControlType.ProgrammaticName -eq $type)) {
    return $el
  }
  $tw = [System.Windows.Automation.TreeWalker]::RawViewWalker
  $ch = $tw.GetFirstChild($el)
  while ($null -ne $ch) {
    $r = Uia-Find $ch $name $type
    if ($null -ne $r) { return $r }
    $ch = $tw.GetNextSibling($ch)
  }
  return $null
}

function Uia-Invoke($root, $name) {
  $el = Uia-Find $root $name
  if ($null -eq $el) { throw "uia element not found: $name" }
  $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke()
}

function Uia-Value($root, $name) {
  $el = Uia-Find $root $name "ControlType.Edit"
  if ($null -eq $el) { $el = Uia-Find $root $name }
  if ($null -eq $el) { return "<missing>" }
  try {
    $vp = $el.GetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern)
    if ($vp.Current.Value) { return $vp.Current.Value }
  } catch {}
  try {
    $tp = $el.GetCurrentPattern([System.Windows.Automation.TextPattern]::Pattern)
    return $tp.DocumentRange.GetText(-1)
  } catch { return "<novalue>" }
}

function Uia-Rect($root, $name, $type = $null) {
  $el = Uia-Find $root $name $type
  if ($null -eq $el) { return $null }
  return $el.Current.BoundingRectangle
}

function Uia-Tree($root, $file) {
  $lines = New-Object System.Collections.Generic.List[string]
  $walk = {
    param($el, $depth)
    $c = $el.Current
    $ct = try { [string]$c.ControlType.ProgrammaticName } catch { "?" }
    $nm = try { [string]$c.Name } catch { "?" }
    $en = try { [string]$c.IsEnabled } catch { "?" }
    $fo = try { [string]$c.IsKeyboardFocusable } catch { "?" }
    $rc = try {
      $r = $c.BoundingRectangle
      "$([int]$r.Left),$([int]$r.Top)-$([int]$r.Right),$([int]$r.Bottom)"
    } catch { "none" }
    $lines.Add(("  " * $depth) + $ct + " name='" + $nm +
      "' enabled=" + $en + " focusable=" + $fo + " rect=" + $rc)
    $tw = [System.Windows.Automation.TreeWalker]::RawViewWalker
    $ch = $tw.GetFirstChild($el)
    while ($null -ne $ch) { & $walk $ch ($depth + 1); $ch = $tw.GetNextSibling($ch) }
  }
  & $walk $root 0
  $lines | Set-Content $file
}

# click an element by name — UIA rect (screen px) -> client px -> posted click
function Click-Element($root, $h, $name, $type = $null) {
  $r = Uia-Rect $root $name $type
  if ($null -eq $r) { throw "uia element not found: $name" }
  # WM_LBUTTON* coords are CLIENT PIXELS (the wndproc converts px->DIP)
  $origin = New-Object NW+POINT
  [NW]::ClientToScreen($h, [ref]$origin) | Out-Null
  $cx = [int]($r.Left + $r.Width / 2 - $origin.x)
  $cy = [int]($r.Top + $r.Height / 2 - $origin.y)
  Post-Click $h $cx $cy
}
