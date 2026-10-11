param([string]$Executable='target\updated\release\aw-watcher-vpn.exe')
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$exe=[IO.Path]::GetFullPath((Join-Path $root $Executable))
$keys=@('LOCALAPPDATA','AW_PROFILE','AW_TESTING','AW_SERVER_URL','AW_MODULE_ACTION_PROTOCOL','AW_MODULE_ACTION')
$prior=@{};foreach($key in $keys){$prior[$key]=[Environment]::GetEnvironmentVariable($key)}
$fixture=Join-Path $root ('local\actions-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$lock=$null
function Fail-Command($arguments) {
    $old=$ErrorActionPreference;$ErrorActionPreference='Continue'
    try { & $exe @arguments 2> (Join-Path $fixture 'expected-error.txt') | Out-Null; $code=$LASTEXITCODE } finally { $ErrorActionPreference=$old }
    if($code -eq 0){throw 'Invalid action unexpectedly succeeded'}
}
try {
    $env:LOCALAPPDATA=$fixture;$env:AW_PROFILE='synthetic-actions';$env:AW_TESTING='1';$env:AW_SERVER_URL='http://127.0.0.1:1'
    $env:AW_MODULE_ACTION_PROTOCOL='1';$env:AW_MODULE_ACTION='configure'
    $manifest=Get-Content -Raw -LiteralPath ($exe+'.actions.json') | ConvertFrom-Json
    if($manifest.schema_version -ne 1 -or $manifest.module -ne 'aw-watcher-vpn' -or $manifest.actions.Count -ne 3){throw 'Manifest identity mismatch'}
    foreach($action in $manifest.actions){
        $env:AW_MODULE_ACTION=$action.id
        $context=& $exe @($action.args) --print-context | ConvertFrom-Json
        if($LASTEXITCODE -ne 0 -or -not $context.testing -or $context.config -notlike '*activitywatch-synthetic-actions*config-testing.json' -or $context.server_url -ne 'http://127.0.0.1:1'){throw 'Action context mismatch'}
    }
    # No server is present on port 1; dispatch resolves context without network requests.
    $env:AW_MODULE_ACTION='configure';$env:AW_TESTING='0'
    $normal=& $exe configure --print-context | ConvertFrom-Json
    if($normal.testing -or $normal.config -notlike '*config.json'){throw 'Explicit testing false was not respected'}
    $manual=& $exe configure --testing --print-context | ConvertFrom-Json
    if(-not $manual.testing){throw 'Manual testing override failed'}
    $explicit=Join-Path $fixture 'explicit.json'
    [IO.File]::WriteAllText($explicit,'{}')
    $lock=[IO.File]::Open($explicit+'.lock','OpenOrCreate','ReadWrite','None')
    $selected=& $exe configure --config $explicit --print-context | ConvertFrom-Json
    if($selected.config -ne $explicit){throw 'Explicit config precedence failed'}
    # Probe must not acquire singleton even while held by the synthetic producer.
    Fail-Command @('not-an-action')
    $env:AW_MODULE_ACTION='about';Fail-Command @('configure','--print-context')
    $env:AW_MODULE_ACTION_PROTOCOL='2';Fail-Command @('about','--print-context')
    $env:AW_MODULE_ACTION_PROTOCOL='1';$env:AW_TESTING='invalid';Fail-Command @('about','--print-context')
    $env:AW_TESTING='0';$env:AW_SERVER_URL='http://remote.example:5600';Fail-Command @('about','--print-context')
    $env:AW_SERVER_URL='http://127.0.0.1:1';$env:AW_MODULE_ACTION_PROTOCOL=$null;$env:AW_MODULE_ACTION=$null
    $manual=& $exe configure --config $explicit --print-context | ConvertFrom-Json
    if($manual.config -ne $explicit){throw 'Manual fallback failed'}
    if(Get-ChildItem -LiteralPath $fixture -Recurse -Filter 'config*.json'){throw 'Dispatch unexpectedly initialized producer config'}
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $version=(& $exe --version).Split(' ')[-1]
    $zip=[IO.Compression.ZipFile]::OpenRead((Join-Path $root "dist\aw-watcher-vpn-$version-windows-x64.zip"))
    try{if($zip.Entries.FullName -notcontains 'aw-watcher-vpn/aw-watcher-vpn.exe.actions.json'){throw 'Manifest absent from package'}}finally{$zip.Dispose()}
    'Action tests passed: manifest, all commands, testing/profile/server context, server unavailable, singleton independence, invalid commands/context, manual fallback and package consistency.'
}finally{if($lock){$lock.Dispose()};foreach($key in $keys){[Environment]::SetEnvironmentVariable($key,$prior[$key])}}
