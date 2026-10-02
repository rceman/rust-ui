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
  "draft_value=$(P-Value $p.MainWindowHandle 'draft')" | Set-Content "$($script:OutDir)\typing.txt" -Encoding UTF8
  Stop-Composer $p
}

function S-Unicode {
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  # unicode payload as HEX UTF-8 — the .ps1 source stays ASCII (a BOM-less
  # UTF-8 script is parsed as ANSI by PS5.1, mangling literal payloads)
  P-TextHex $p.MainWindowHandle "68c3a96c6c6f20e4b896e7958cf09fa680" | Out-Null
  Shot-PrintWindow $p.MainWindowHandle "unicode.png"
  "draft_value=$(P-Value $p.MainWindowHandle 'draft')" | Set-Content "$($script:OutDir)\unicode.txt" -Encoding UTF8
  Stop-Composer $p
}

function S-Selection {
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  P-Text $p.MainWindowHandle "first" | Out-Null
  P-KeyMod $p.MainWindowHandle 0x41 "ctrl"   # Ctrl+A
  P-Text $p.MainWindowHandle "SECOND" | Out-Null   # replaces the selection
  Shot-PrintWindow $p.MainWindowHandle "selection.png"
  "draft_value=$(P-Value $p.MainWindowHandle 'draft')" | Set-Content "$($script:OutDir)\selection.txt" -Encoding UTF8
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
  "draft_value=$(P-Value $p.MainWindowHandle 'draft')" | Set-Content "$($script:OutDir)\readonly.txt" -Encoding UTF8
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
  # PRECONDITION for mid-flight evidence: busy must be OBSERVED — poll the
  # observable stop-enabled signal (bounded), never assume timing
  $busy = $false
  for ($i = 0; $i -lt 20; $i++) {
    Start-Sleep -Milliseconds 100
    $e = P-Enabled $p.MainWindowHandle "stop"
    if ($e -eq "true" -or $e -eq $true) { $busy = $true; break }
  }
  if (-not $busy) { throw "send never entered the busy state - mid-flight evidence impossible" }
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
  # the submit counter + committed echo live inside the details group —
  # expand it so the probe can REQUIRE the observable signal
  P-ClickNamed $p.MainWindowHandle "show details" | Out-Null
  Start-Sleep -Milliseconds 400
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  $r = P-Ime $p.MainWindowHandle "draft" "arigato"
  $r | ConvertTo-Json | Set-Content "$($script:OutDir)\ime.json" -Encoding UTF8
  Shot-PrintWindow $p.MainWindowHandle "ime.png"
  "draft_value=$(P-Value $p.MainWindowHandle 'draft')" | Set-Content "$($script:OutDir)\ime.txt" -Encoding UTF8
  Stop-Composer $p
}

function S-Details {
  # F12 — the details diagnostics are THREE distinct vertically-ordered
  # rects (real layout, not a PrintWindow artifact). UIA screen rects are
  # the authority; the screen capture is the visual receipt.
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "show details" | Out-Null
  Start-Sleep -Milliseconds 400
  $rects = @()
  foreach ($pre in @("turn ", "draft-echo ", "draft-edits ")) {
    $r = P-RectPrefix $p.MainWindowHandle $pre
    $rects += ,$r
  }
  if ($rects.Count -ne 3) { throw "expected 3 detail labels, got $($rects.Count)" }
  $ok = $true
  for ($i = 0; $i -lt 2; $i++) {
    $a = $rects[$i].rect_px; $b = $rects[$i+1].rect_px
    # strict vertical order: a's bottom must be at/above b's top (no overlap)
    if ($a[3] -gt $b[1]) { $ok = $false }
  }
  if (-not $ok) { throw "detail labels overlap: $($rects | ForEach-Object { $_.rect_px -join ',' } | Out-String)" }
  # F11 — complete JSON escaping: a backslash in committed text must
  # round-trip through BOTH 'type' and 'value' emissions as valid JSON
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  P-KeyMod $p.MainWindowHandle 0x41 "ctrl" | Out-Null   # Ctrl+A — exact content
  P-Text $p.MainWindowHandle "draft-prefill\q" | Out-Null
  $v = P-Value $p.MainWindowHandle "draft"
  if ($v -ne "draft-prefill\q") { throw "value round-trip failed: $v" }
  ($rects | ConvertTo-Json -Depth 5) | Set-Content "$($script:OutDir)\details-layout.json" -Encoding UTF8
  Shot-Screen $p.MainWindowHandle "details-screen.png"
  Shot-PrintWindow $p.MainWindowHandle "details-pw.png"
  Stop-Composer $p
  "details ok" | Set-Content "$($script:OutDir)\details.txt"
}

function S-Foundation {
  $p = Launch-Composer
  P-ClickNamed $p.MainWindowHandle "draft" | Out-Null
  Shot-PrintWindow $p.MainWindowHandle "final-foundation.png"
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
    ($r | ConvertTo-Json -Compress) | Set-Content "$($script:OutDir)\dpi-$dpi.txt" -Encoding UTF8 
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
         "multiline","reorder","send","ime","details","uia","dpi","idle",
         "scale","theme","foundation")
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
    "details"   { S-Details }
    "uia"       { S-Uia }
    "dpi"       { S-Dpi }
    "idle"      { S-Idle }
    "scale"     { S-Scale }
    "theme"     { S-Theme }
    "foundation"{ S-Foundation }
    default     { Write-Warning "unknown scenario $s" }
  }
  Write-Output "scenario $s done"
}
Write-Output "evidence collected"
