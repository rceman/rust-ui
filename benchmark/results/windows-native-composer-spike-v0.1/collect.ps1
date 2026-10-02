param([string]$Scenario = "all")
$ErrorActionPreference='Stop'
# Scenario runner — orchestration ONLY. Every native semantic (input
# packing, coordinate conversion, UIA interpretation, DPI math, IME,
# assertions) lives in native_probe.exe; this file sequences and stores.
#
#   .\collect.ps1 -Scenario all
#   .\collect.ps1 -Scenario typing,undo,dpi
. "$PSScriptRoot\native.ps1"

New-Item -ItemType Directory -Force -Path $script:OutDir | Out-Null

# ---- scenarios ------------------------------------------------------------

function S-Smoke {
  $p = Launch-Composer
  P-Geometry $p.MainWindowHandle | ConvertTo-Json | Set-Content "$($script:OutDir)\smoke.json"
  Shot-PrintWindow $p.MainWindowHandle "composer-dark.png"
  Stop-Composer $p
  "smoke ok" | Set-Content "$($script:OutDir)\smoke.txt"
}

function S-Typing {
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  P-Text $p.MainWindowHandle "hello world" | Out-Null
  Shot-PrintWindow $p.MainWindowHandle "typing.png"
  "draft_value=$(P-Value $p.MainWindowHandle 'draft')" | Set-Content "$($script:OutDir)\typing.txt"
  Stop-Composer $p
}

function S-Unicode {
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  P-Text $p.MainWindowHandle "héllo 世界🦀" | Out-Null
  Shot-PrintWindow $p.MainWindowHandle "unicode.png"
  "draft_value=$(P-Value $p.MainWindowHandle 'draft')" | Set-Content "$($script:OutDir)\unicode.txt"
  Stop-Composer $p
}

function S-Selection {
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  P-Text $p.MainWindowHandle "first" | Out-Null
  P-KeyMod $p.MainWindowHandle 0x41 "ctrl"   # Ctrl+A
  P-Text $p.MainWindowHandle "SECOND" | Out-Null   # replaces the selection
  Shot-PrintWindow $p.MainWindowHandle "selection.png"
  "draft_value=$(P-Value $p.MainWindowHandle 'draft')" | Set-Content "$($script:OutDir)\selection.txt"
  Stop-Composer $p
}

function S-Undo {
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  P-Text $p.MainWindowHandle "typed text" | Out-Null
  $mid = P-Value $p.MainWindowHandle 'draft'
  # native undo — repeat until empty (undo unit grouping is msftedit's)
  for ($u = 0; $u -lt 14; $u++) {
    P-KeyMod $p.MainWindowHandle 0x5A "ctrl"  # Ctrl+Z
    if ((P-Value $p.MainWindowHandle 'draft') -eq '') { break }
  }
  Shot-PrintWindow $p.MainWindowHandle "undo.png"
  "draft_mid=$mid`ndraft_after_undos=$(P-Value $p.MainWindowHandle 'draft')" |
    Set-Content "$($script:OutDir)\undo.txt"
  Stop-Composer $p
}

function S-ReadOnly {
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  P-Text $p.MainWindowHandle "before" | Out-Null
  P-Invoke $p.MainWindowHandle "ro on" | Out-Null   # staged read_only -> ECO_READONLY
  Start-Sleep -Milliseconds 400
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  P-Text $p.MainWindowHandle "REJECTED" | Out-Null  # must NOT land
  Shot-PrintWindow $p.MainWindowHandle "readonly.png"
  "draft_value=$(P-Value $p.MainWindowHandle 'draft')" | Set-Content "$($script:OutDir)\readonly.txt"
  Stop-Composer $p
}

function S-Disabled {
  $p = Launch-Composer
  P-Invoke $p.MainWindowHandle "send" | Out-Null
  Start-Sleep -Milliseconds 300               # mid-flight
  $send = P-Enabled $p.MainWindowHandle "send"
  $stop = P-Enabled $p.MainWindowHandle "stop"
  "send_enabled_mid=$send stop_enabled_mid=$stop" |
    Set-Content "$($script:OutDir)\disabled.txt"
  Shot-PrintWindow $p.MainWindowHandle "disabled-mid.png"
  Stop-Composer $p
}

function S-Multiline {
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "body" | Out-Null
  for ($i = 0; $i -lt 6; $i++) {
    P-Text $p.MainWindowHandle "line$i" | Out-Null
    P-KeyMod $p.MainWindowHandle 0x0D "shift"  # Shift+Enter = soft newline
  }
  Shot-PrintWindow $p.MainWindowHandle "multiline.png"
  $r = P-Rect $p.MainWindowHandle "body"
  "body_rect=$($r.rect_px -join ',')" | Set-Content "$($script:OutDir)\multiline.txt"
  Stop-Composer $p
}

function S-Reorder {
  $p = Launch-Composer
  (P-UiaTree $p.MainWindowHandle | ConvertTo-Json -Depth 6) | Set-Content "$($script:OutDir)\reorder-before.txt"
  P-Invoke $p.MainWindowHandle "reorder" | Out-Null
  Start-Sleep -Milliseconds 400
  (P-UiaTree $p.MainWindowHandle | ConvertTo-Json -Depth 6) | Set-Content "$($script:OutDir)\reorder-after.txt"
  P-Invoke $p.MainWindowHandle "rm B" | Out-Null
  Start-Sleep -Milliseconds 300
  P-Invoke $p.MainWindowHandle "mk B" | Out-Null   # recreate — new generation
  Start-Sleep -Milliseconds 300
  Shot-PrintWindow $p.MainWindowHandle "reorder.png"
  (P-UiaTree $p.MainWindowHandle | ConvertTo-Json -Depth 6) | Set-Content "$($script:OutDir)\reorder-final.txt"
  Stop-Composer $p
}

function S-Send {
  $p = Launch-Composer
  P-Invoke $p.MainWindowHandle "send" | Out-Null
  Start-Sleep -Milliseconds 400
  Shot-PrintWindow $p.MainWindowHandle "send-mid.png"
  P-Invoke $p.MainWindowHandle "stop" | Out-Null
  Start-Sleep -Milliseconds 400
  "send_enabled_after_stop=$(P-Enabled $p.MainWindowHandle 'send')" |
    Set-Content "$($script:OutDir)\send-stop.txt"
  P-Invoke $p.MainWindowHandle "send" | Out-Null   # resend
  Start-Sleep -Milliseconds 2800
  Shot-PrintWindow $p.MainWindowHandle "send-done.png"
  Stop-Composer $p
}

function S-Ime {
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  $r = P-Ime $p.MainWindowHandle "draft" "arigato"
  $r | ConvertTo-Json | Set-Content "$($script:OutDir)\ime.json"
  Shot-PrintWindow $p.MainWindowHandle "ime.png"
  "draft_value=$(P-Value $p.MainWindowHandle 'draft')" | Set-Content "$($script:OutDir)\ime.txt"
  Stop-Composer $p
}

function S-Uia {
  $p = Launch-Composer
  (P-UiaTree $p.MainWindowHandle | ConvertTo-Json -Depth 6) |
    Set-Content "$($script:OutDir)\uia-tree.txt"
  Stop-Composer $p
}

function S-Dpi {
  foreach ($dpi in 96, 120, 144, 192) {
    $p = Launch-Composer
    # the PROBE owns the semantic proof: it computes the expected px size
    # via the shared scale authority and asserts the post-change window
    # rect — this script only records the result
    $r = P-Dpi $p.MainWindowHandle $dpi
    $r | Set-Content "$($script:OutDir)\dpi-$dpi.txt"
    Shot-PrintWindow $p.MainWindowHandle "dpi-$dpi.png"
    Stop-Composer $p
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
  Stop-Composer $p   # graceful — perf-counters.txt arrives at clean shutdown
}

function S-Scale {
  foreach ($n in 100, 1000) {
    $p = Launch-Composer @{"RUI_ROWS" = "$n"}
    $t0 = Get-Date
    $kids = P-Count $p.MainWindowHandle
    $ms = [math]::Round(((Get-Date) - $t0).TotalMilliseconds, 0)
    "rows=$n uia_children=$kids settle_query_ms=$ms" | Add-Content "$($script:OutDir)\scale.txt"
    Shot-PrintWindow $p.MainWindowHandle "scale-$n.png"
    Stop-Composer $p
  }
}

function S-Theme {
  $p = Launch-Composer
  P-Invoke $p.MainWindowHandle "light" | Out-Null
  Start-Sleep -Milliseconds 600
  Shot-PrintWindow $p.MainWindowHandle "composer-light.png"
  P-Invoke $p.MainWindowHandle "dark" | Out-Null
  Start-Sleep -Milliseconds 600
  Shot-PrintWindow $p.MainWindowHandle "theme-back-dark.png"
  Stop-Composer $p
}

# ---- driver ----------------------------------------------------------------

$all = @("smoke","typing","unicode","selection","undo","readonly","disabled",
         "multiline","reorder","send","ime","uia","dpi","idle","scale","theme")
$run = if ($Scenario -eq "all") { $all } else { $Scenario.Split(",") }

$sha = git rev-parse HEAD 2>$null
"HEAD=$sha`nexe_sha256=$((Get-FileHash $script:ComposerExe -Algorithm SHA256).Hash)`nprobe_sha256=$((Get-FileHash $script:ProbeExe -Algorithm SHA256).Hash)`nrun_utc=$((Get-Date).ToUniversalTime().ToString('o'))" |
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
    "ime"       { S-Ime }
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
