$ErrorActionPreference='Stop'
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
Add-Type -TypeDefinition @"
using System; using System.Runtime.InteropServices;
public class W3 {
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr dc, uint f);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint m, IntPtr w, IntPtr l);
}
"@
$out = "W:\devin_folder\rust-ui\benchmark\results\windows-native-composer-spike-v0.1"
$exe = "W:\devin_folder\rust-ui\target\debug\examples\composer.exe"

function Shot($h, $name) {
  $bmp = New-Object System.Drawing.Bitmap 980, 900
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $dc = $g.GetHdc()
  [W3]::PrintWindow($h, $dc, 2) | Out-Null
  $g.ReleaseHdc($dc); $g.Dispose()
  $bmp.Save("$out\$name"); $bmp.Dispose()
}

function UiaFind($el, $name) {
  if ($el.Current.Name -eq $name) { return $el }
  $tw = [System.Windows.Automation.TreeWalker]::RawViewWalker
  $ch = $tw.GetFirstChild($el)
  while ($null -ne $ch) {
    $r = UiaFind $ch $name
    if ($null -ne $r) { return $r }
    $ch = $tw.GetNextSibling($ch)
  }
  return $null
}
function UiaInvoke($root, $name) {
  $el = UiaFind $root $name
  if ($null -eq $el) { throw "uia element not found: $name" }
  $ip = $el.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
  $ip.Invoke()
}

# --- session 1: dark shot + UIA tree + perf + send flow ---
$p = Start-Process -FilePath $exe -PassThru
Start-Sleep -Milliseconds 1800
$h = $p.MainWindowHandle
Shot $h "composer-dark.png"

# UIA tree dump
$uiawin = [System.Windows.Automation.AutomationElement]::FromHandle($h)
$lines = New-Object System.Collections.Generic.List[string]
function Walk($el, $depth) {
  $c = $el.Current
  $lines.Add(("  " * $depth) + $c.ControlType.ProgrammaticName + " name='" + $c.Name + "' enabled=" + $c.IsEnabled + " focusable=" + $c.IsKeyboardFocusable + " rect=" + [int]$c.BoundingRectangle.Left + "," + [int]$c.BoundingRectangle.Top + "-" + [int]$c.BoundingRectangle.Right + "," + [int]$c.BoundingRectangle.Bottom)
  $tw = [System.Windows.Automation.TreeWalker]::RawViewWalker
  $ch = $tw.GetFirstChild($el)
  while ($null -ne $ch) {
    Walk $ch ($depth+1)
    $ch = $tw.GetNextSibling($ch)
  }
}
Walk $uiawin 0
$lines | Set-Content "$out\uia-tree.txt"

# idle perf — 3s sample
$cpu0 = $p.TotalProcessorTime.TotalSeconds
$ws = [math]::Round($p.WorkingSet64 / 1MB, 1)
Start-Sleep -Milliseconds 3000
$p.Refresh()
$cpu1 = $p.TotalProcessorTime.TotalSeconds
$idle = [math]::Round((($cpu1-$cpu0)/3)*100, 2)
"idle_cpu_pct=$idle working_set_mb=$ws threads=$($p.Threads.Count) scale=1.25" | Set-Content "$out\perf.txt"

# send -> mid-flight -> done
UiaInvoke $uiawin "send"
Start-Sleep -Milliseconds 250
Shot $h "send-mid.png"
Start-Sleep -Milliseconds 2500
Shot $h "send-done.png"
$p | Stop-Process -Force

# --- session 2: light theme ---
$p = Start-Process -FilePath $exe -PassThru
Start-Sleep -Milliseconds 1800
$uiawin = [System.Windows.Automation.AutomationElement]::FromHandle($p.MainWindowHandle)
UiaInvoke $uiawin "light"
Start-Sleep -Milliseconds 500
Shot $p.MainWindowHandle "composer-light.png"
$p | Stop-Process -Force
Write-Output "evidence collected"
