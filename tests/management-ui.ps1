param([string]$Executable = 'target\updated\release\aw-watcher-vpn.exe')
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$watcherExe = [IO.Path]::GetFullPath((Join-Path $root $Executable))
$testDir = Join-Path $root ('local\ui-test-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $testDir | Out-Null
$configPath = Join-Path $testDir 'config.json'
@{schema_version=1;poll_seconds=5;profiles=@()} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $configPath -Encoding UTF8
$discoveries = @(
    @{id='synthetic-a';name='Synthetic VPN A';suggested_name='customer-a.example';rules=@{adapter_names=@('Synthetic VPN A')}},
    @{id='synthetic-b';name='Synthetic VPN B';suggested_name='customer-b.example';rules=@{adapter_names=@('Synthetic VPN B')}}
)
ConvertTo-Json -InputObject $discoveries -Depth 8 | Set-Content -LiteralPath ($configPath + '.discovered.json') -Encoding UTF8
$notificationMessage = $false
$managementTestOutput = Join-Path $testDir 'management.png'
. (Join-Path $root 'scripts\manage.ps1')
$config = Get-Content -Raw -LiteralPath $configPath | ConvertFrom-Json
if ($config.profiles.Count -ne 1 -or $config.profiles[0].customer_hint -ne 'Synthetic Customer Renamed') { throw 'Saved customer is incorrect' }
"Rendered UI: $managementTestOutput"
