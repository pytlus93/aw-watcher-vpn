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
    Copy-Item -LiteralPath scripts\aw-watcher-vpn.exe.actions.json -Destination (Join-Path $TargetDirectory 'release\aw-watcher-vpn.exe.actions.json') -Force
    if ($Package) {
        New-Item -ItemType Directory -Path dist -Force | Out-Null
        $binary = Join-Path $TargetDirectory 'release\aw-watcher-vpn.exe'
        $manifest = [IO.File]::ReadAllText((Join-Path $root 'Cargo.toml'))
        $version = [regex]::Match($manifest, '(?m)^version\s*=\s*"([0-9.]+)"').Groups[1].Value
        if (-not $version) { throw 'Cannot determine package version' }
        $stage = Join-Path $root ("dist\package-$version-" + [Guid]::NewGuid().ToString('N') + '\aw-watcher-vpn')
        New-Item -ItemType Directory -Path $stage -Force | Out-Null
        Copy-Item -LiteralPath $binary,README.md,CHANGELOG.md,VALIDATION.md,DATA_FIELDS.md,MODULE_ACTIONS.md,config.example.json,LICENSE,scripts\aw-watcher-vpn.exe.actions.json,scripts\install.ps1,scripts\name-connections.cmd -Destination $stage -Force
        Compress-Archive -LiteralPath $stage -DestinationPath "dist\aw-watcher-vpn-$version-windows-x64.zip" -Force
        $zip = Join-Path $root "dist\aw-watcher-vpn-$version-windows-x64.zip"
        [IO.File]::WriteAllText(($zip + '.sha256'), ((Get-FileHash -LiteralPath $zip -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + [IO.Path]::GetFileName($zip) + "`n"), [Text.UTF8Encoding]::new($false))
    }
} finally { Pop-Location }
