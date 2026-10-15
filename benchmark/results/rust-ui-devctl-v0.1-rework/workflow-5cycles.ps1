# H3c step 2 — scripted 5-cycle devctl workflow against the fresh clone.
# Every command line + stdout JSON + exit code lands in cycle-<n>-attempt-<a>.txt.
param(
    [string]$Root = "C:\Users\therceman\AppData\Local\Temp\h3c-gate",
    [string]$Repo = "C:\Users\therceman\AppData\Local\Temp\devctl-fresh-cf8ab91"
)
$ErrorActionPreference = 'Continue'
$ctl   = Join-Path $Repo 'target\debug\rust-ui-devctl.exe'
$probe = Join-Path $Repo 'target\debug\examples\devctl_probe.exe'
$out   = Join-Path $Root 'cycles'
New-Item -ItemType Directory -Force $out | Out-Null

$results = @()
$guardEvents = 0

function Find-Node {
    param($n, [string]$id)
    if ($n.automation_id -eq $id) { return $n }
    foreach ($c in $n.children) {
        $r = Find-Node $c $id
        if ($null -ne $r) { return $r }
    }
    return $null
}

function Invoke-Ctl {
    param([string[]]$Argv, [System.IO.StreamWriter]$T)
    $line = "rust-ui-devctl " + ($Argv -join ' ')
    $T.WriteLine("`n`$ $line")
    $json = & $ctl @Argv 2>&1 | Out-String
    $code = $LASTEXITCODE
    $T.WriteLine($json.TrimEnd())
    $T.WriteLine("exit=$code")
    return @{ code = $code; json = $json.Trim() }
}

for ($cycle = 1; $cycle -le 5; $cycle++) {
    $attempt = 0
    $done = $false
    while (-not $done) {
        $attempt++
        $tx = Join-Path $out "cycle-$cycle-attempt-$attempt.txt"
        $dir = Join-Path $out "cycle-$cycle"
        New-Item -ItemType Directory -Force $dir | Out-Null
        $T = [System.IO.StreamWriter]::new($tx)
        $T.WriteLine("# devctl workflow cycle $cycle attempt $attempt ; repo=$Repo")
        $session = Join-Path $dir "session.json"
        $guardHit = $false
        $handlesBefore = 0; $handlesAfter = 0
        try {
            # launch -> wait-idle -> tree -> rect
            $r = Invoke-Ctl @('launch','--exe',$probe,'--session',$session) $T
            if ($r.code -ne 0) { throw "launch failed exit $($r.code)" }
            $launch = $r.json | ConvertFrom-Json
            $scale = $launch.result.window.scale_factor
            $pidv = $launch.result.pid
            $T.WriteLine("# scale_factor=$scale pid=$pidv")
            Invoke-Ctl @('wait-idle','--session',$session) $T | Out-Null
            $r = Invoke-Ctl @('tree','--session',$session) $T
            $tree0 = $r.json | ConvertFrom-Json
            Invoke-Ctl @('rect','probe.button','--session',$session) $T | Out-Null
            # hover -> wait-idle -> element screenshot
            Invoke-Ctl @('hover','probe.button','--session',$session) $T | Out-Null
            Invoke-Ctl @('wait-idle','--session',$session) $T | Out-Null
            $r = Invoke-Ctl @('tree','--session',$session) $T
            $hb = Find-Node ($r.json | ConvertFrom-Json).result.root 'probe.button'
            $T.WriteLine("# ASSERT probe.button.hovered == $($hb.hovered) (expect True)")
            $r = Invoke-Ctl @('screenshot','--id','probe.button','--session',$session,'--out',(Join-Path $dir 'hover.png')) $T
            if ($r.code -eq 5) { $guardHit = $true; throw 'CAPTURE_FAILED' }
            # leave -> wait-idle -> tree must report hovered=false
            Invoke-Ctl @('leave','--session',$session) $T | Out-Null
            Invoke-Ctl @('wait-idle','--session',$session) $T | Out-Null
            $r = Invoke-Ctl @('tree','--session',$session) $T
            $t2 = Find-Node ($r.json | ConvertFrom-Json).result.root 'probe.button'
            $T.WriteLine("# ASSERT probe.button.hovered == $($t2.hovered) (expect False)")
            # press -> wait-idle -> pressed screenshot
            Invoke-Ctl @('press','probe.button','--session',$session) $T | Out-Null
            Invoke-Ctl @('wait-idle','--session',$session) $T | Out-Null
            $r = Invoke-Ctl @('screenshot','--id','probe.button','--session',$session,'--out',(Join-Path $dir 'pressed.png')) $T
            if ($r.code -eq 5) { $guardHit = $true; throw 'CAPTURE_FAILED' }
            Invoke-Ctl @('release','probe.button','--session',$session) $T | Out-Null
            # editor: focus -> End -> Backspace
            Invoke-Ctl @('focus','probe.input','--session',$session) $T | Out-Null
            Invoke-Ctl @('key','End','--session',$session) $T | Out-Null
            Invoke-Ctl @('key','Backspace','--session',$session) $T | Out-Null
            Invoke-Ctl @('wait-idle','--session',$session) $T | Out-Null
            $r = Invoke-Ctl @('tree','--session',$session) $T
            $ed = Find-Node ($r.json | ConvertFrom-Json).result.root 'probe.input'
            $T.WriteLine("# ASSERT probe.input.label == '$($ed.label)'")
            $handlesBefore = (Get-Process -Id $pidv).HandleCount
            $T.WriteLine("# target handles before captures: $handlesBefore")
            # snapshot-layout -> full client WGC -> element WGC
            Invoke-Ctl @('snapshot-layout','--session',$session,'--out',(Join-Path $dir 'snapshot.json')) $T | Out-Null
            $r = Invoke-Ctl @('screenshot','--session',$session,'--out',(Join-Path $dir 'client.png')) $T
            if ($r.code -eq 5) { $guardHit = $true; throw 'CAPTURE_FAILED' }
            $r = Invoke-Ctl @('screenshot','--id','probe.button','--session',$session,'--out',(Join-Path $dir 'element.png')) $T
            if ($r.code -eq 5) { $guardHit = $true; throw 'CAPTURE_FAILED' }
            # comparators
            $r = Invoke-Ctl @('compare-layout','--reference',(Join-Path $dir 'snapshot.json'),'--snapshot',(Join-Path $dir 'snapshot.json')) $T
            $T.WriteLine("# ASSERT compare-layout self pass=$($r.code -eq 0)")
            $r = Invoke-Ctl @('compare-image','--reference-image',(Join-Path $dir 'client.png'),'--native-image',(Join-Path $dir 'client.png')) $T
            $T.WriteLine("# ASSERT compare-image self pass=$($r.code -eq 0)")
            # cross-authority: frozen contract vs live probe
            $cref = Join-Path $Repo 'reference\shadcn-gallery-v0.1\contract.json'
            $cimg = Join-Path $Repo 'reference\shadcn-gallery-v0.1\screenshots\light\components.png'
            $r = Invoke-Ctl @('compare-element','--reference',$cref,'--capture','components/default',
                            '--id','button.default','--native-id','probe.button',
                            '--reference-image',$cimg,'--native-image',(Join-Path $dir 'client.png'),
                            '--snapshot',(Join-Path $dir 'snapshot.json'),
                            '--out-dir',(Join-Path $dir 'cross')) $T
            $T.WriteLine("# EXPECT cross-authority exit=6 pass=false; got exit=$($r.code)")
            $r = Invoke-Ctl @('compare-layout','--reference',$cref,'--capture','components/default',
                            '--snapshot',(Join-Path $dir 'snapshot.json')) $T
            $T.WriteLine("# EXPECT contract compare-layout exit=6 (missing core ids); got exit=$($r.code)")
            $handlesAfter = (Get-Process -Id $pidv).HandleCount
            $T.WriteLine("# target handles after captures: $handlesAfter")
            Invoke-Ctl @('shutdown','--session',$session) $T | Out-Null
            $done = $true
            $results += [pscustomobject]@{ cycle=$cycle; attempt=$attempt; pass=$true;
                scale=$scale; handles_before=$handlesBefore; handles_after=$handlesAfter; guard='' }
        } catch {
            $T.WriteLine("# CYCLE FAILED: $($_.Exception.Message)")
            if ($guardHit) { $guardEvents++ }
            # best-effort shutdown + receipt cleanup before retrying
            & $ctl shutdown --session $session *>$null
            Start-Sleep -Milliseconds 300
            $results += [pscustomobject]@{ cycle=$cycle; attempt=$attempt; pass=$false;
                scale=$scale; handles_before=$handlesBefore; handles_after=$handlesAfter;
                guard=$(if ($guardHit){'CAPTURE_FAILED'}else{$_.Exception.Message}) }
        } finally {
            $T.Close()
        }
    }
}
$results | Format-Table -AutoSize | Out-String | Write-Host
"# guard (CAPTURE_FAILED) events: $guardEvents" | Write-Host
$results | ConvertTo-Json | Set-Content (Join-Path $out 'summary.json')
