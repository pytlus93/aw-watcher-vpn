param([switch]$Test, [switch]$Package, [string]$TargetDirectory = 'target')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $cargoCommand = 'cargo'
    $cargoOptions = @('--target-dir', $TargetDirectory)
    if (Test-Path -LiteralPath "$env:USERPROFILE\.cargo\bin\cargo.exe") {
        $cargoCommand = "$env:USERPROFILE\.cargo\bin\cargo.exe"
    }
    if ($Test) {
        & $cargoCommand test @cargoOptions --locked
        if ($LASTEXITCODE -ne 0) { throw 'Tests failed' }
    }
    & $cargoCommand build @cargoOptions --release --locked
    if ($LASTEXITCODE -ne 0) { throw 'Build failed' }
    if ($Package) {
        New-Item -ItemType Directory -Path dist -Force | Out-Null
        $binary = Join-Path $TargetDirectory 'release\aw-watcher-vpn.exe'
        Compress-Archive -LiteralPath $binary,README.md,config.example.json,LICENSE -DestinationPath dist\aw-watcher-vpn-0.1.0-windows-x64.zip -Force
    }
} finally { Pop-Location }
