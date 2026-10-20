param([string]$Executable = 'target\updated\release\aw-watcher-vpn.exe')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$Executable = [IO.Path]::GetFullPath((Join-Path $root $Executable))
$testRoot = Join-Path $root ('local\module-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testRoot | Out-Null
$oldAppData = $env:LOCALAPPDATA
$oldProfile = $env:AW_PROFILE
$oldSystemRoot = $env:SystemRoot
$process = $null
try {
    $env:LOCALAPPDATA = Join-Path $testRoot 'appdata'
    $env:AW_PROFILE = ''
    $mockInstall = Join-Path $testRoot 'activitywatch'
    New-Item -ItemType Directory -Path $mockInstall | Out-Null
    [IO.File]::WriteAllText((Join-Path $mockInstall 'aw-qt.exe'), 'synthetic launcher marker')
    $qtConfig = Join-Path $env:LOCALAPPDATA 'activitywatch\activitywatch\aw-qt\aw-qt.toml'
    New-Item -ItemType Directory -Path (Split-Path -Parent $qtConfig) -Force | Out-Null
    $cases = @('', '[aw-qt]', "[aw-qt]`r`nautostart_modules = [`"aw-server-rust`", `"aw-calendar-sync`"]`r`n[aw-qt-testing]`r`nmarker = true`r`n", "[aw-qt]`r`nautostart_modules = [`r`n  'aw-server', # comment`r`n  `"aw-calendar-sync`",`r`n]`r`n")
    foreach ($text in $cases) {
        [IO.File]::WriteAllText($qtConfig, $text)
        & (Join-Path $root 'scripts\install.ps1') -ActivityWatchDirectory $mockInstall -Executable $Executable | Out-Null
        $first = [IO.File]::ReadAllText($qtConfig)
        if ([regex]::Matches($first, '"aw-watcher-vpn"').Count -ne 1) { throw 'VPN autostart duplicated or missing' }
        if ($text.Contains('aw-calendar-sync') -and -not $first.Contains('aw-calendar-sync')) { throw 'Other module lost' }
        if ($text.Contains('[aw-qt-testing]') -and -not $first.Contains('[aw-qt-testing]')) { throw 'Testing section lost' }
        & (Join-Path $root 'scripts\install.ps1') -ActivityWatchDirectory $mockInstall -Executable $Executable | Out-Null
        if ([IO.File]::ReadAllText($qtConfig) -ne $first) { throw 'Installer not idempotent' }
    }
    $installed = Join-Path $mockInstall 'aw-watcher-vpn\aw-watcher-vpn.exe'
    $configPath = & $installed --config-path
    $default = Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json
    if ($default.profiles.Count -ne 0 -or -not $default.autolearn -or -not $default.notify) { throw 'Default settings wrong' }
    $default.notify = $false; $default.autolearn = $false
    [IO.File]::WriteAllText($configPath, ($default | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    & (Join-Path $root 'scripts\install.ps1') -ActivityWatchDirectory $mockInstall -Executable $Executable -NoAutostart | Out-Null
    if ((Get-Content -Raw $configPath | ConvertFrom-Json).notify) { throw 'Existing config overwritten' }
    $normalPath = $configPath
    $env:AW_PROFILE = 'synthetic'
    $profilePath = & $installed --init-config
    if ($profilePath -eq $normalPath -or $profilePath -notlike '*activitywatch-synthetic*') { throw 'Profile isolation failed' }
    $env:AW_PROFILE = ''
    # Import a synthetic pre-module profile into a fresh AW profile, and verify
    # a repeated import cannot silently replace already installed settings.
    $importSource = Join-Path $testRoot 'old-config.json'
    $import = @{schema_version=1;poll_seconds=7;profiles=@(@{id='customer-a';customer_hint='Synthetic Customer A';rules=@{adapter_names=@('Synthetic VPN')}})}
    [IO.File]::WriteAllText($importSource, ($import | ConvertTo-Json -Depth 10), [Text.UTF8Encoding]::new($false))
    $env:AW_PROFILE = 'import-check'
    & (Join-Path $root 'scripts\install.ps1') -ActivityWatchDirectory $mockInstall -Executable $Executable -ProfileConfig $importSource -NoAutostart | Out-Null
    $importPath = & $installed --config-path
    $imported = Get-Content -Raw -LiteralPath $importPath | ConvertFrom-Json
    if ($imported.profiles[0].id -ne 'customer-a' -or $imported.poll_seconds -ne 7) { throw 'Profile import lost data' }
    $rejected = $false
    try { & (Join-Path $root 'scripts\install.ps1') -ActivityWatchDirectory $mockInstall -Executable $Executable -ProfileConfig $importSource -NoAutostart | Out-Null } catch { $rejected = $true }
    if (-not $rejected) { throw 'Existing imported settings were not protected' }
    $env:AW_PROFILE = ''
    $testingPath = & $installed --testing --init-config
    if ($testingPath -eq $normalPath -or $testingPath -notlike '*config-testing.json') { throw 'Testing isolation failed' }
    if (-not (Test-Path -LiteralPath ($installed + '.actions.json'))) { throw 'Installer omitted action sidecar' }
    $legacySource = Join-Path $testRoot 'legacy\aw-watcher-vpn.exe'
    New-Item -ItemType Directory -Path (Split-Path -Parent $legacySource) | Out-Null
    Copy-Item -LiteralPath $Executable -Destination $legacySource
    & (Join-Path $root 'scripts\install.ps1') -ActivityWatchDirectory $mockInstall -Executable $legacySource -NoAutostart | Out-Null
    if (Test-Path -LiteralPath ($installed + '.actions.json')) { throw 'Rollback left a stale action sidecar' }
    & (Join-Path $root 'scripts\install.ps1') -ActivityWatchDirectory $mockInstall -Executable $Executable -NoAutostart | Out-Null
    if (-not (Test-Path -LiteralPath ($installed + '.actions.json'))) { throw 'Reinstallation did not restore matching sidecar' }
    # Force collector failure: launcher lifetime is tested without sampling real
    # network data or ever enqueueing anything to the user's AW server.
    $env:SystemRoot = Join-Path $testRoot 'missing-windows'
    $process = Start-Process -FilePath $installed -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $testRoot 'out.log') -RedirectStandardError (Join-Path $testRoot 'err.log')
    Start-Sleep -Seconds 2
    if ($process.HasExited) { throw 'No-argument AW launch exited' }
    $savedPreference = $ErrorActionPreference
    $ErrorActionPreference = 'Continue' # PS 5.1 treats expected native stderr as an error record.
    & $installed --once 2> (Join-Path $testRoot 'duplicate.log') | Out-Null
    $duplicateExit = $LASTEXITCODE
    $ErrorActionPreference = $savedPreference
    if ($duplicateExit -eq 0) { throw 'Duplicate module instance accepted' }
    Stop-Process -Id $process.Id; $process.WaitForExit(); $process = $null
    # Host-style crash/relaunch: only this synthetic installed process is terminated.
    $process = Start-Process -FilePath $installed -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $testRoot ('relaunch-' + [Guid]::NewGuid().ToString('N') + '.out')) -RedirectStandardError (Join-Path $testRoot ('relaunch-' + [Guid]::NewGuid().ToString('N') + '.err'))
    Start-Sleep -Seconds 1
    if ($process.HasExited) { throw 'Relaunch after explicit stop failed' }
    $process.Kill(); $process.WaitForExit(); $process = $null
    $process = Start-Process -FilePath $installed -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $testRoot ('relaunch-' + [Guid]::NewGuid().ToString('N') + '.out')) -RedirectStandardError (Join-Path $testRoot ('relaunch-' + [Guid]::NewGuid().ToString('N') + '.err'))
    Start-Sleep -Seconds 1
    if ($process.HasExited) { throw 'Relaunch after crash failed to release instance lock' }
    Stop-Process -Id $process.Id; $process.WaitForExit(); $process = $null
    Start-Sleep -Seconds 1
    if (Get-Process aw-watcher-vpn -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq $installed }) { throw 'Module relaunched itself after an intentional stop' }
    $failedLaunch = $false
    try { Start-Process -FilePath (Join-Path $testRoot 'missing-module.exe') -WindowStyle Hidden -ErrorAction Stop | Out-Null } catch { $failedLaunch = $true }
    if (-not $failedLaunch) { throw 'Missing executable did not fail launch' }
    $policy = & $installed payload $normalPath | ConvertFrom-Json
    if ($policy.mode -ne 'strict' -or -not $policy.customers -or $policy.connection_ids -or $policy.connection_names -or $policy.suggested_names) { throw 'Unsafe payload migration defaults' }
    $policy.mode = 'extended'; $policy.customers = $false; $policy.connection_ids = $true
    $encoded = [Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes(($policy | ConvertTo-Json -Compress)))
    & $installed payload $normalPath --base64 $encoded
    if ($LASTEXITCODE -ne 0) { throw 'Payload settings save failed' }
    $saved = & $installed payload $normalPath | ConvertFrom-Json
    if ($saved.mode -ne 'extended' -or $saved.customers -or -not $saved.connection_ids) { throw 'Payload settings were not persisted' }
    $tokens = $null; $parseErrors = $null
    [void][Management.Automation.Language.Parser]::ParseFile((Join-Path $root 'scripts\manage.ps1'), [ref]$tokens, [ref]$parseErrors)
    if ($parseErrors.Count) { throw ($parseErrors | Out-String) }
    $version = & $installed --version
    if ($LASTEXITCODE -ne 0 -or $version -ne 'aw-watcher-vpn 0.2.10') { throw 'Incorrect module version' }
    $process = Start-Process -FilePath $installed -ArgumentList '--testing' -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $testRoot 'testing-out.log') -RedirectStandardError (Join-Path $testRoot 'testing-err.log')
    Start-Sleep -Seconds 2
    if ($process.HasExited) { throw 'AW --testing launch exited' }
    Stop-Process -Id $process.Id; $process.WaitForExit(); $process = $null
    'Module smoke tests passed: installer preservation/idempotence, default/profile/testing configuration, no-argument/testing lifetime, duplicate-instance guard, crash/relaunch lock release, intentional stop, failed launch and payload settings persistence.'
} finally {
    if ($process -and -not $process.HasExited) { Stop-Process -Id $process.Id }
    $env:LOCALAPPDATA = $oldAppData; $env:AW_PROFILE = $oldProfile; $env:SystemRoot = $oldSystemRoot
}
