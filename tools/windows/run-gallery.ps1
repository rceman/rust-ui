# run-gallery.ps1 -- one-click native Gallery launcher.
# Builds the current rust-ui-gallery from the AUTHORITATIVE WSL checkout
# (/home/therceman/git/rust-ui) via the native Windows cargo toolchain and
# launches the resulting exe.
#
# The authoritative Linux path is resolved to its Windows-accessible UNC at
# runtime via `wsl.exe wslpath` -- no distro name is hard-coded.
#
# UX contract:
#   success -> hidden PowerShell exits, Gallery opens (no terminal left)
#   failure -> visible native message box with the cargo error tail;
#              nothing launches, no invisible keyboard wait
#
# Desktop shortcut target:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "<resolved repo UNC>\tools\windows\run-gallery.ps1"
$ErrorActionPreference = 'Stop'
$repoWsl   = '/home/therceman/git/rust-ui'          # authoritative source
$targetDir = 'W:\devin_folder\rust-ui-gallery-target'
$bin       = "$targetDir\debug\rust-ui-gallery.exe"

function Show-LaunchError($msg) {
    try {
        Add-Type -AssemblyName System.Windows.Forms | Out-Null
        [System.Windows.Forms.MessageBox]::Show(
            $msg,
            'rust-ui Gallery - launch failed',
            [System.Windows.Forms.MessageBoxButtons]::OK,
            [System.Windows.Forms.MessageBoxIcon]::Error) | Out-Null
    } catch {
        # MessageBox unavailable (headless) -- last-resort console output.
        Write-Host $msg -ForegroundColor Red
    }
}

try {
    # Resolve /home/therceman/git/rust-ui -> \\wsl.localhost\<distro>\... at
    # runtime via the WSL tooling itself; no distro name is assumed.
    $repoWin = (& wsl.exe wslpath -w $repoWsl 2>$null | Select-Object -First 1)
    if ($repoWin) { $repoWin = $repoWin.Trim() }
    if (-not $repoWin -or -not (Test-Path (Join-Path $repoWin 'Cargo.toml'))) {
        throw "authoritative repo not reachable - 'wsl wslpath -w $repoWsl' returned '$repoWin'"
    }

    $env:CARGO_TARGET_DIR = $targetDir
    $env:CARGO_INCREMENTAL = '0'
    Push-Location $repoWin
    # cargo writes progress to stderr; under 'Stop' that throws before the
    # exit-code check -- capture with Continue and gate on $LASTEXITCODE.
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $out = & cargo.exe build --locked --features devtools --bin rust-ui-gallery 2>&1
    $code = $LASTEXITCODE
    $ErrorActionPreference = $prev
    Pop-Location
    if ($code -ne 0) {
        $tail = ($out | Select-Object -Last 12) -join "`n"
        throw "cargo build failed (exit $code)`n`n$tail"
    }
    if (-not (Test-Path $bin)) { throw "build succeeded but $bin missing" }

    # Start-Process rejects UNC working directories -- use the local
    # target dir as the process cwd instead.
    Start-Process $bin -WorkingDirectory $targetDir
    exit 0
} catch {
    Show-LaunchError "rust-ui Gallery could not start.`n`n$_"
    exit 1
}
