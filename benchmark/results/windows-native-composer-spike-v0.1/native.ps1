# native.ps1 — orchestration helpers for the composer spike evidence runs.
#
# BOUNDARY (docs/PLATFORM_CONTRACTS.md §evidence):
#   PowerShell  = orchestration ONLY — process lifecycle, screenshots,
#                 timeouts, scenario sequencing, artifact paths.
#   Rust probe  = ALL native semantics — LPARAM packing, client/screen
#                 conversion, UIA property interpretation, modifier state,
#                 DPI math, IME driving, assertion.
#
# No DllImport input/Uia calls may appear here. If a scenario needs a new
# native operation, add a subcommand to examples/native_probe.rs.

Add-Type -AssemblyName System.Drawing

$script:OutDir = "W:\devin_folder\rust-ui\benchmark\results\windows-native-composer-spike-v0.1"
$script:ComposerExe = "W:\devin_folder\rust-ui\target\debug\examples\composer.exe"
$script:ProbeExe = "W:\devin_folder\rust-ui\target\debug\examples\native_probe.exe"
$script:ComposerTitle = "rust-ui composer"   # resolve_window accepts substrings

function Launch-Composer($env = @{}) {
  Get-Process composer -ErrorAction SilentlyContinue | Stop-Process -Force
  $psi = New-Object System.Diagnostics.ProcessStartInfo $script:ComposerExe
  $psi.UseShellExecute = $false
  foreach ($k in $env.Keys) { $psi.Environment[$k] = $env[$k] }
  $p = [System.Diagnostics.Process]::Start($psi)
  # settle: the window must exist before any probe call
  for ($i = 0; $i -lt 50; $i++) {
    $p.Refresh()
    if ($p.MainWindowHandle -ne 0) { break }
    Start-Sleep -Milliseconds 100
  }
  Start-Sleep -Milliseconds 400
  return $p
}

function Stop-Composer($p) {
  # GRACEFUL close first — RUI_PERF counters are written at clean shutdown;
  # force is only a fallback so perf evidence is never fabricated
  $p.CloseMainWindow() | Out-Null
  if (-not $p.WaitForExit(5000)) { $p | Stop-Process -Force }
}

# ---- probe seam (the ONLY native-semantics surface) ------------------------

function Probe($a) {
  $out = & $script:ProbeExe @a 2>&1
  if ($LASTEXITCODE -ne 0) { throw "native_probe $($a[0]) failed: $out" }
  return ($out | ConvertFrom-Json)
}

function P-Geometry($h)      { Probe @("geometry",  "$h") }
function P-UiaTree($h)       { Probe @("uia",       "$h") }
function P-Rect($h,$n)       { Probe @("uia-rect",  "$h", $n) }
function P-Enabled($h,$n)    { (Probe @("uia-enabled","$h",$n)).enabled }
function P-Count($h)         { (Probe @("uia-count","$h")).children }
function P-Value($h,$n)      { (Probe @("value",    "$h", $n)).value }
function P-Invoke($h,$n)     { Probe @("invoke",    "$h", $n) }
function P-ClickNamed($h,$n) { Probe @("click-named","$h",$n) }
function P-Click($h,$x,$y)   { Probe @("click-post","$h","$x","$y") }
function P-Text($h,$t)       { Probe @("type-post", "$h", $t) }
function P-KeyMod($h,$vk,$mod) {
  # $mod = "ctrl"|"shift"|"ctrl shift"|"" (empty = unmodified key)
  if ($mod) {
    $parts = @("key-post", "$h", "$vk") + ($mod -split ' ')
  } else {
    $parts = @("key-post", "$h", "$vk")
  }
  Probe $parts
}
function P-Dpi($h,$dpi)      { Probe @("dpichange", "$h", "$dpi") }
function P-Ime($h,$ed,$keys) { Probe @("ime", "$h", $ed, $keys) }
# real SendInput (acceptance) variants — foreground-driven
function P-RealClick($h,$x,$y) { Probe @("click", "$h", "$x", "$y") }
function P-RealType($h,$t)     { Probe @("type",  "$h", $t) }

# ---- captures (artifact generation is orchestration) ----------------------

function Shot-PrintWindow($h, $name) {
  Add-Type -AssemblyName System.Drawing -ErrorAction SilentlyContinue
  $bmp = New-Object System.Drawing.Bitmap 1000, 950
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $dc = $g.GetHdc()
  # PrintWindow is the raster pipeline under evidence — kept here as the
  # capture mechanism (artifact), NOT a semantic assertion
  $sig = '[System.Runtime.InteropServices.DllImport("user32.dll")] public static extern bool PrintWindow(System.IntPtr h, System.IntPtr dc, uint f);'
  $nw = Add-Type -MemberDefinition $sig -Name PW -Namespace Cap -PassThru
  $nw::PrintWindow($h, $dc, 2) | Out-Null
  $g.ReleaseHdc($dc); $g.Dispose()
  $bmp.Save("$($script:OutDir)\$name"); $bmp.Dispose()
}
