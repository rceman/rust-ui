# run-gallery.ps1 — one-click native Gallery launcher.
# Builds the current rust-ui-gallery from the AUTHORITATIVE WSL checkout
# (/home/therceman/git/rust-ui) via the native Windows cargo toolchain and
# launches the resulting exe. Never launches a stale binary after a build
# failure — errors are shown in the console window instead.
#
# Desktop shortcut target:
#   powershell.exe -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File "<wsl repo>\tools\windows\run-gallery.ps1"
$ErrorActionPreference = 'Stop'
$repoWsl   = '/home/therceman/git/rust-ui'          # authoritative source
$targetDir = 'W:\devin_folder\rust-ui-gallery-target'
$bin       = "$targetDir\debug\rust-ui-gallery.exe"

try {
    $repoUnc = "\\wsl.localhost\Ubuntu" + ($repoWsl -replace '/', '\')
    if (-not (Test-Path "$repoUnc\Cargo.toml")) { throw "authoritative repo not reachable at $repoUnc" }

    $env:CARGO_TARGET_DIR = $targetDir
    $env:CARGO_INCREMENTAL = '0'
    Push-Location $repoUnc
    # cargo writes progress to stderr; under 'Stop' that throws before the
    # exit-code check — capture with Continue and gate on $LASTEXITCODE.
    $prev = $ErrorActionPreference
    $ErrorActionPreference = 'Continue'
    $out = & cargo.exe build --locked --features devtools --bin rust-ui-gallery 2>&1
    $code = $LASTEXITCODE
    $ErrorActionPreference = $prev
    Pop-Location
    if ($code -ne 0) {
        throw "cargo build failed (exit $code)`n$($out | Select-Object -Last 15 | Out-String)"
    }
    if (-not (Test-Path $bin)) { throw "build succeeded but $bin missing" }

    Start-Process $bin -WorkingDirectory $repoUnc
    exit 0
} catch {
    Write-Host "rust-ui Gallery launch failed:`n$_`n" -ForegroundColor Red
    Write-Host "Press Enter to close."
    Read-Host | Out-Null
    exit 1
}
