param([string]$Scenario = "all")
$ErrorActionPreference='Stop'
# Scenario runner over the deterministic native helpers in native.ps1 —
# PostMessage input, UIA actions, VirtualAllocEx WM_DPICHANGED.
#
#   .\collect.ps1 -Scenario all
#   .\collect.ps1 -Scenario typing,undo,dpi
. "$PSScriptRoot\native.ps1"

New-Item -ItemType Directory -Force -Path $script:OutDir | Out-Null

# ---- scenarios ------------------------------------------------------------

function S-Smoke {
  $p = Launch-Composer
  Shot-PrintWindow $p.MainWindowHandle "composer-dark.png"
  $p | Stop-Process -Force
  "smoke ok" | Set-Content "$($script:OutDir)\smoke.txt"
}

function S-Typing {
  $p = Launch-Composer
  $root = Uia-Root $p.MainWindowHandle
  Click-Element $root $p.MainWindowHandle "draft" "ControlType.Edit"
  Post-Text $p.MainWindowHandle "hello world"
  Shot-PrintWindow $p.MainWindowHandle "typing.png"
  "draft_value=$(Uia-Value $root 'draft')" | Set-Content "$($script:OutDir)\typing.txt"
  $p | Stop-Process -Force
}

function S-Unicode {
  $p = Launch-Composer
  $root = Uia-Root $p.MainWindowHandle
  Click-Element $root $p.MainWindowHandle "draft" "ControlType.Edit"
  Post-Text $p.MainWindowHandle "héllo 世界🦀"   # accents + CJK + surrogate-pair emoji
  Shot-PrintWindow $p.MainWindowHandle "unicode.png"
  "draft_value=$(Uia-Value $root 'draft')" | Set-Content "$($script:OutDir)\unicode.txt"
  $p | Stop-Process -Force
}

function S-Selection {
  $p = Launch-Composer
  $root = Uia-Root $p.MainWindowHandle
  Click-Element $root $p.MainWindowHandle "draft" "ControlType.Edit"
  Post-Text $p.MainWindowHandle "first"
  Post-Key $p.MainWindowHandle 0x41 -ctrl:$true   # Ctrl+A
  Post-Text $p.MainWindowHandle "SECOND"    # replaces the selection
  Shot-PrintWindow $p.MainWindowHandle "selection.png"
  "draft_value=$(Uia-Value $root 'draft')" | Set-Content "$($script:OutDir)\selection.txt"
  $p | Stop-Process -Force
}

function S-Undo {
  $p = Launch-Composer
  $root = Uia-Root $p.MainWindowHandle
  Click-Element $root $p.MainWindowHandle "draft" "ControlType.Edit"
  Post-Text $p.MainWindowHandle "typed text"
  $mid = Uia-Value $root 'draft'
  # native undo — repeat until empty (undo unit grouping is msftedit's)
  for ($u = 0; $u -lt 14; $u++) {
    Post-Key $p.MainWindowHandle 0x5A -ctrl:$true  # Ctrl+Z
    if ((Uia-Value $root 'draft') -eq '') { break }
  }
  Shot-PrintWindow $p.MainWindowHandle "undo.png"
  "draft_mid=$mid`ndraft_after_undos=$(Uia-Value $root 'draft')" |
    Set-Content "$($script:OutDir)\undo.txt"
  $p | Stop-Process -Force
}

function S-ReadOnly {
  $p = Launch-Composer
  $root = Uia-Root $p.MainWindowHandle
  Click-Element $root $p.MainWindowHandle "draft" "ControlType.Edit"
  Post-Text $p.MainWindowHandle "before"
  Uia-Invoke $root "ro on"                    # staged read_only -> ECO_READONLY
  Start-Sleep -Milliseconds 400
  Click-Element $root $p.MainWindowHandle "draft" "ControlType.Edit"
  Post-Text $p.MainWindowHandle "REJECTED"    # must NOT land
  Shot-PrintWindow $p.MainWindowHandle "readonly.png"
  "draft_value=$(Uia-Value $root 'draft')" | Set-Content "$($script:OutDir)\readonly.txt"
  $p | Stop-Process -Force
}

function S-Disabled {
  $p = Launch-Composer
  $root = Uia-Root $p.MainWindowHandle
  Uia-Invoke $root "send"
  Start-Sleep -Milliseconds 300               # mid-flight
  $send = Uia-Find $root "send"; $stop = Uia-Find $root "stop"
  "send_enabled_mid=$($send.Current.IsEnabled) stop_enabled_mid=$($stop.Current.IsEnabled)" |
    Set-Content "$($script:OutDir)\disabled.txt"
  Shot-PrintWindow $p.MainWindowHandle "disabled-mid.png"
  $p | Stop-Process -Force
}

function S-Multiline {
  $p = Launch-Composer
  $root = Uia-Root $p.MainWindowHandle
  Click-Element $root $p.MainWindowHandle "body" "ControlType.Edit"
  for ($i = 0; $i -lt 6; $i++) {
    Post-Text $p.MainWindowHandle "line$i"
    Post-Key $p.MainWindowHandle 0x0D -shift:$true  # Shift+Enter = soft newline
  }
  Shot-PrintWindow $p.MainWindowHandle "multiline.png"
  $r = Uia-Rect $root "body" "ControlType.Edit"
  "body_rect=$r" | Set-Content "$($script:OutDir)\multiline.txt"
  $p | Stop-Process -Force
}

function S-Reorder {
  $p = Launch-Composer
  $root = Uia-Root $p.MainWindowHandle
  Uia-Tree $root "$($script:OutDir)\reorder-before.txt"
  Uia-Invoke $root "reorder"
  Start-Sleep -Milliseconds 400
  Uia-Tree $root "$($script:OutDir)\reorder-after.txt"
  Uia-Invoke $root "rm B"
  Start-Sleep -Milliseconds 300
  Uia-Invoke $root "mk B"                     # recreate — new generation
  Start-Sleep -Milliseconds 300
  Shot-PrintWindow $p.MainWindowHandle "reorder.png"
  Uia-Tree $root "$($script:OutDir)\reorder-final.txt"
  $p | Stop-Process -Force
}

function S-Send {
  $p = Launch-Composer
  $root = Uia-Root $p.MainWindowHandle
  Uia-Invoke $root "send"
  Start-Sleep -Milliseconds 400
  Shot-PrintWindow $p.MainWindowHandle "send-mid.png"
  Uia-Invoke $root "stop"
  Start-Sleep -Milliseconds 400
  $send = Uia-Find $root "send"
  "send_enabled_after_stop=$($send.Current.IsEnabled)" | Set-Content "$($script:OutDir)\send-stop.txt"
  Uia-Invoke $root "send"                     # resend
  Start-Sleep -Milliseconds 2800
  Shot-PrintWindow $p.MainWindowHandle "send-done.png"
  $p | Stop-Process -Force
}

function S-Uia {
  $p = Launch-Composer
  Uia-Tree (Uia-Root $p.MainWindowHandle) "$($script:OutDir)\uia-tree.txt"
  (cargo run --example uia_probe 2>&1 | Out-String) | Set-Content "$($script:OutDir)\uia-probe.txt"
  $p | Stop-Process -Force
}

function S-Dpi {
  foreach ($dpi in 96, 120, 144, 192) {
    $p = Launch-Composer
    Post-DpiChanged $p $dpi
    Shot-PrintWindow $p.MainWindowHandle "dpi-$dpi.png"
    $p | Stop-Process -Force
  }
}

function S-Idle {
  $p = Launch-Composer @{"RUI_PERF" = "$($script:OutDir)\perf-counters.txt"}
  $cpu0 = $p.TotalProcessorTime.TotalSeconds
  $ws = [math]::Round($p.WorkingSet64 / 1MB, 1)
  Start-Sleep -Seconds 30
  $p.Refresh()
  $cpu1 = $p.TotalProcessorTime.TotalSeconds
  $idle = [math]::Round((($cpu1 - $cpu0) / 30) * 100, 3)
  "idle_cpu_pct_30s=$idle working_set_mb=$ws threads=$($p.Threads.Count)" |
    Set-Content "$($script:OutDir)\perf.txt"
  $p | Stop-Process -Force
}

function S-Scale {
  foreach ($n in 100, 1000) {
    $p = Launch-Composer @{"RUI_ROWS" = "$n"}
    $t0 = Get-Date
    $root = Uia-Root $p.MainWindowHandle
    $kids = 0
    $tw = [System.Windows.Automation.TreeWalker]::RawViewWalker
    $ch = $tw.GetFirstChild($root)
    while ($null -ne $ch) { $kids++; $ch = $tw.GetNextSibling($ch) }
    $ms = [math]::Round(((Get-Date) - $t0).TotalMilliseconds, 0)
    "rows=$n uia_children=$kids settle_query_ms=$ms" | Add-Content "$($script:OutDir)\scale.txt"
    Shot-PrintWindow $p.MainWindowHandle "scale-$n.png"
    $p | Stop-Process -Force
  }
}

function S-Theme {
  $p = Launch-Composer
  $root = Uia-Root $p.MainWindowHandle
  Uia-Invoke $root "light"
  Start-Sleep -Milliseconds 600
  Shot-PrintWindow $p.MainWindowHandle "composer-light.png"
  Uia-Invoke $root "dark"
  Start-Sleep -Milliseconds 600
  Shot-PrintWindow $p.MainWindowHandle "theme-back-dark.png"
  $p | Stop-Process -Force
}

# ---- driver ----------------------------------------------------------------

$all = @("smoke","typing","unicode","selection","undo","readonly","disabled",
         "multiline","reorder","send","uia","dpi","idle","scale","theme")
$run = if ($Scenario -eq "all") { $all } else { $Scenario.Split(",") }

$sha = git rev-parse HEAD 2>$null
"HEAD=$sha`nexe_sha256=$((Get-FileHash $script:ComposerExe -Algorithm SHA256).Hash)`nrun_utc=$((Get-Date).ToUniversalTime().ToString('o'))" |
  Set-Content "$($script:OutDir)\identity.txt"

foreach ($s in $run) {
  switch ($s) {
    "smoke"     { S-Smoke }
    "typing"    { S-Typing }
    "unicode"   { S-Unicode }
    "selection" { S-Selection }
    "undo"      { S-Undo }
    "readonly"  { S-ReadOnly }
    "disabled"  { S-Disabled }
    "multiline" { S-Multiline }
    "reorder"   { S-Reorder }
    "send"      { S-Send }
    "uia"       { S-Uia }
    "dpi"       { S-Dpi }
    "idle"      { S-Idle }
    "scale"     { S-Scale }
    "theme"     { S-Theme }
    default     { Write-Warning "unknown scenario $s" }
  }
  Write-Output "scenario $s done"
}
Write-Output "evidence collected"
