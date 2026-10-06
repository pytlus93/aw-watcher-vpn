param(
    [Parameter(Mandatory=$true)][string]$ActivityWatchDirectory,
    [string]$Executable,
    [string]$ProfileConfig,
    [switch]$NoAutostart
)
$ErrorActionPreference = 'Stop'
if (-not $Executable) {
    $Executable = Join-Path $PSScriptRoot 'aw-watcher-vpn.exe'
    if (-not (Test-Path -LiteralPath $Executable)) { $Executable = Join-Path (Split-Path -Parent $PSScriptRoot) 'target\release\aw-watcher-vpn.exe' }
}
$installRoot = (Resolve-Path -LiteralPath $ActivityWatchDirectory).Path
if (-not (Test-Path -LiteralPath (Join-Path $installRoot 'aw-qt.exe') -PathType Leaf)) { throw 'ActivityWatchDirectory must contain aw-qt.exe' }
if (-not (Test-Path -LiteralPath $Executable -PathType Leaf)) { throw 'Build the executable first' }
$Executable = (Resolve-Path -LiteralPath $Executable).Path
if (Get-Process aw-qt -ErrorAction SilentlyContinue | Where-Object { $_.Path -eq (Join-Path $installRoot 'aw-qt.exe') }) { throw 'Exit ActivityWatch before installing or updating the module' }
$configPath = (& $Executable --config-path | Select-Object -Last 1)
if ($LASTEXITCODE -ne 0 -or -not $configPath) { throw 'Cannot determine AW configuration path' }
if ($ProfileConfig) {
    $ProfileConfig = (Resolve-Path -LiteralPath $ProfileConfig).Path
    if (Test-Path -LiteralPath $configPath) { throw 'AW module config already exists; it will not be overwritten. Omit -ProfileConfig or merge profiles manually.' }
    & $Executable --config $ProfileConfig --validate-config | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'ProfileConfig is not a valid watcher config' }
}
$profile = $env:AW_PROFILE
if ($profile -and $profile -ne 'default') {
    if ($profile -notmatch '^[A-Za-z0-9_-]+$') { throw 'Invalid AW_PROFILE' }
    $appName = "activitywatch-$profile"
} else { $appName = 'activitywatch' }
$qtDir = Join-Path (Join-Path (Join-Path $env:LOCALAPPDATA $appName) $appName) 'aw-qt'
$qtConfig = Join-Path $qtDir 'aw-qt.toml'
$original = if (Test-Path -LiteralPath $qtConfig) { [IO.File]::ReadAllText($qtConfig) } else { '' }
$text = $original
if (-not $NoAutostart) {
    $section = [regex]::Match($text, '(?ms)^\[aw-qt\][^\r\n]*(?:\r?\n|\z)(?<body>.*?)(?=^\[|\z)')
    $server = if (Test-Path -LiteralPath (Join-Path $installRoot 'aw-server\aw-server.exe')) { 'aw-server' }
              elseif (Test-Path -LiteralPath (Join-Path $installRoot 'aw-server-rust\aw-server-rust.exe')) { 'aw-server-rust' }
              else { 'aw-server' }
    $defaults = 'autostart_modules = ["' + $server + '", "aw-watcher-afk", "aw-watcher-window", "aw-watcher-vpn"]'
    if ($section.Success) {
        $body = $section.Groups['body'].Value
        $assignment = [regex]::Match($body, '(?ms)^\s*autostart_modules\s*=\s*\[(?<items>.*?)\]')
        if ($assignment.Success) {
            $items = [regex]::Replace($assignment.Groups['items'].Value, '(?m)#[^\r\n]*', '')
            if ($items -notmatch '(?:"aw-watcher-vpn"|''aw-watcher-vpn'')') {
                if ($items -notmatch '^\s*(?:(?:"[A-Za-z0-9_.-]+"|''[A-Za-z0-9_.-]+'')\s*,?\s*)*$') { throw 'Unusual autostart_modules syntax; add aw-watcher-vpn manually, or use -NoAutostart' }
                $items = $items.Trim().TrimEnd(',').Trim()
                if ($items) { $items += ', ' }
                $replacement = 'autostart_modules = [' + $items + '"aw-watcher-vpn"]'
                $body = $body.Substring(0, $assignment.Index) + $replacement + $body.Substring($assignment.Index + $assignment.Length)
            }
        } else { $body = "`r`n" + $defaults + "`r`n" + $body }
        $offset = $section.Groups['body'].Index
        $text = $text.Substring(0, $offset) + $body + $text.Substring($offset + $section.Groups['body'].Length)
    } else { $text += "`r`n[aw-qt]`r`n$defaults`r`n" }
}
$destination = Join-Path $installRoot 'aw-watcher-vpn'
New-Item -ItemType Directory -Path $destination -Force | Out-Null
$installedExe = Join-Path $destination 'aw-watcher-vpn.exe'
Copy-Item -LiteralPath $Executable -Destination $installedExe -Force
$namingCommand = Join-Path $PSScriptRoot 'name-connections.cmd'
if (Test-Path -LiteralPath $namingCommand) { Copy-Item -LiteralPath $namingCommand -Destination $destination -Force }
if ($ProfileConfig) {
    New-Item -ItemType Directory -Path (Split-Path -Parent $configPath) -Force | Out-Null
    [IO.File]::Copy($ProfileConfig, $configPath, $false)
    if (Test-Path -LiteralPath ($ProfileConfig + '.discovered.json')) { [IO.File]::Copy(($ProfileConfig + '.discovered.json'), ($configPath + '.discovered.json'), $false) }
}
& $installedExe --init-config | Out-Null
if ($LASTEXITCODE -ne 0) { throw 'Config initialization failed' }
if (-not $NoAutostart -and $text -ne $original) {
    New-Item -ItemType Directory -Path $qtDir -Force | Out-Null
    if (Test-Path -LiteralPath $qtConfig) { Copy-Item -LiteralPath $qtConfig -Destination ($qtConfig + '.before-vpn-' + [Guid]::NewGuid().ToString('N')) }
    [IO.File]::WriteAllText($qtConfig, $text, [Text.UTF8Encoding]::new($false))
}
Write-Host "Installed: $installedExe"
Write-Host "Configuration: $configPath"
Write-Host 'Restart ActivityWatch. Modules -> aw-watcher-vpn starts/stops this module.'
Write-Host 'Use name-connections.cmd beside the installed executable to name unfamiliar connections.'
