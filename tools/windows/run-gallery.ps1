# run-gallery.ps1 -- one-click native Gallery launcher.
#
# Launches W:\devin_folder\rust-ui-gallery-target\debug\rust-ui-gallery.exe
# built from the AUTHORITATIVE WSL checkout /home/therceman/git/rust-ui.
#
# Fast path: when a build stamp in the target dir proves the current
# binary already matches HEAD on a clean worktree, the Gallery launches
# immediately -- no cargo invocation.
#
# Otherwise cargo builds HEADLESSLY (CreateNoWindow + redirected output)
# so no terminal/conhost window ever appears. A failed build shows a
# visible message box with the cargo error tail; nothing launches.
#
# The Linux path resolves via `wsl.exe wslpath` -- no distro name assumed.
#
# Desktop shortcut target:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "<resolved repo UNC>\tools\windows\run-gallery.ps1"
$ErrorActionPreference = 'Stop'
$repoWsl   = '/home/therceman/git/rust-ui'
$targetDir = 'W:\devin_folder\rust-ui-gallery-target'
$bin       = "$targetDir\debug\rust-ui-gallery.exe"
$stamp     = "$targetDir\.rust-ui-gallery-build.json"
$profileId = 'debug-devtools'

function Show-LaunchError($msg) {
    try {
        Add-Type -AssemblyName System.Windows.Forms | Out-Null
        [System.Windows.Forms.MessageBox]::Show(
            $msg,
            'rust-ui Gallery - launch failed',
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Error) | Out-Null
    } catch {
        Write-Host $msg -ForegroundColor Red
    }
}

# Run a console child with NO window: detached console creation plus
# redirected output. This is what keeps cargo/wsl children from surfacing
# a Windows Terminal or conhost window under the hidden launcher.
function Invoke-Headless($file, $argline, $workdir) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $file
    $psi.Arguments = $argline
    $psi.WorkingDirectory = $workdir
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $p = [System.Diagnostics.Process]::Start($psi)
    # async reads on BOTH streams — sequential ReadToEnd deadlocks when the
    # child fills one pipe buffer while we wait on the other
    $outTask = $p.StandardOutput.ReadToEndAsync()
    $errTask = $p.StandardError.ReadToEndAsync()
    $p.WaitForExit()
    return @{ code = $p.ExitCode; out = $outTask.Result; err = $errTask.Result }
}

# Start the Gallery without a console window. rust-ui-gallery.exe is a
# console-subsystem binary and Start-Process under Windows PowerShell 5.1
# uses ShellExecuteEx, which would open a persistent Terminal/conhost
# window. CreateNoWindow + UseShellExecute=$false suppresses it.
function Start-Gallery {
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $bin
    $psi.WorkingDirectory = $targetDir
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    [System.Diagnostics.Process]::Start($psi) | Out-Null
}

# Read the build stamp; a missing OR corrupt stamp simply means "build".
function Read-Stamp {
    if (-not (Test-Path $stamp)) { return $null }
    try { return Get-Content $stamp -Raw | ConvertFrom-Json }
    catch { return $null }
}

try {
    # Resolve /home/therceman/git/rust-ui -> \\wsl.localhost\<distro>\...
    $r = Invoke-Headless 'wsl.exe' "wslpath -w $repoWsl" $targetDir
    $repoWin = ($r.out -split "`n" | Select-Object -First 1).Trim()
    if (-not $repoWin -or -not (Test-Path (Join-Path $repoWin 'Cargo.toml'))) {
        throw "authoritative repo not reachable - 'wsl wslpath -w $repoWsl' returned '$repoWin'"
    }

    # worktree identity for the fast-path decision
    $head  = (Invoke-Headless 'wsl.exe' "-e git -C $repoWsl rev-parse HEAD" $targetDir).out.Trim()
    $dirty = ((Invoke-Headless 'wsl.exe' "-e git -C $repoWsl status --porcelain" $targetDir).out.Trim() -ne '')

    # fast path: stamp + clean worktree + HEAD match + binary identity
    if ((Test-Path $bin) -and ($s = Read-Stamp) -and $s.head -eq $head -and
        $s.profile -eq $profileId -and -not $dirty -and $head -and
        $s.bin_mtime -eq (Get-Item $bin).LastWriteTimeUtc.Ticks -and
        $s.bin_size -eq (Get-Item $bin).Length) {
        Start-Gallery
        exit 0
    }

    # build -- headless cargo; UNC source is referenced via --manifest-path
    # because Windows process APIs reject UNC working directories.
    # CARGO_TARGET_DIR is inherited by the child environment.
    $env:CARGO_TARGET_DIR = $targetDir
    $r = Invoke-Headless 'cargo.exe' "build --locked --features devtools --bin rust-ui-gallery --manifest-path `"$repoWin\Cargo.toml`"" $targetDir
    if ($r.code -ne 0) {
        $tail = (($r.out + "`n" + $r.err) -split "`n" | Select-Object -Last 12) -join "`n"
        throw "cargo build failed (exit $($r.code))`n`n$tail"
    }

    # stamp only a clean-tree build -- a dirty worktree must always rebuild;
    # the stamp pins the built binary's identity so an out-of-band exe
    # replacement can't false-fast-path
    if (-not $dirty -and $head) {
        $b = Get-Item $bin
        @{ head = $head; profile = $profileId; bin_mtime = $b.LastWriteTimeUtc.Ticks;
           bin_size = $b.Length; built = [DateTime]::UtcNow.ToString('o') } |
            ConvertTo-Json | Set-Content -Encoding ASCII $stamp
    } else {
        Remove-Item $stamp -Force -ErrorAction SilentlyContinue
    }

    if (-not (Test-Path $bin)) { throw "build succeeded but $bin is missing" }
    Start-Gallery
    exit 0
} catch {
    Show-LaunchError $_.Exception.Message
    exit 1
}
