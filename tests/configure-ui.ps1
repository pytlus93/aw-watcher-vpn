param([string]$Executable='target\updated\release\aw-watcher-vpn.exe')
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$exe=[IO.Path]::GetFullPath((Join-Path $root $Executable))
$fixture=Join-Path $root ('local\configure-ui-'+[Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $fixture | Out-Null
$cfg=Join-Path $fixture 'config.json'
& $exe --config $cfg --init-config | Out-Null
$keys=@('AW_MODULE_ACTION_PROTOCOL','AW_MODULE_ACTION','AW_TESTING','AW_PROFILE','AW_SERVER_URL')
$prior=@{};foreach($key in $keys){$prior[$key]=[Environment]::GetEnvironmentVariable($key)}
$process=$null;$lock=$null
try {
    $env:AW_MODULE_ACTION_PROTOCOL='1';$env:AW_MODULE_ACTION='configure';$env:AW_TESTING='1';$env:AW_PROFILE='synthetic-ui';$env:AW_SERVER_URL='http://127.0.0.1:1'
    $lock=[IO.File]::Open($cfg+'.lock','OpenOrCreate','ReadWrite','None')
    $process=Start-Process -FilePath $exe -ArgumentList @('configure','--config',$cfg) -PassThru -WindowStyle Hidden -RedirectStandardOutput (Join-Path $fixture 'stdout.log') -RedirectStandardError (Join-Path $fixture 'stderr.log')
    $retainedHandle=$process.Handle
    $deadline=[DateTime]::UtcNow.AddSeconds(10);$window=$null
    do {
        if($process.HasExited){Get-Content (Join-Path $fixture 'stderr.log');throw 'Configure action exited before showing UI'}
        $child=Get-CimInstance Win32_Process -Filter ('ParentProcessId='+$process.Id) | Where-Object Name -eq 'powershell.exe' | Select-Object -First 1
        if($child){$window=Get-Process -Id $child.ProcessId; if($window.MainWindowHandle -ne [IntPtr]::Zero){break}}
        Start-Sleep -Milliseconds 200
    } while([DateTime]::UtcNow -lt $deadline)
    if(-not $window -or $window.MainWindowHandle -eq [IntPtr]::Zero -or $window.MainWindowTitle -notlike 'ActivityWatch - VPN connections*'){throw 'Expected configuration window was not displayed'}
    Add-Type -AssemblyName System.Drawing
    Add-Type @"
using System;
using System.Runtime.InteropServices;
public class ConfigureUiCapture {
 [StructLayout(LayoutKind.Sequential)] public struct Rect { public int Left,Top,Right,Bottom; }
 [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr handle,out Rect rect);
 [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr handle,IntPtr dc,uint flags);
}
"@
    $rect=New-Object ConfigureUiCapture+Rect
    [void][ConfigureUiCapture]::GetWindowRect($window.MainWindowHandle,[ref]$rect)
    $bitmap=New-Object Drawing.Bitmap(($rect.Right-$rect.Left),($rect.Bottom-$rect.Top))
    $graphics=[Drawing.Graphics]::FromImage($bitmap);$dc=$graphics.GetHdc()
    try {if(-not [ConfigureUiCapture]::PrintWindow($window.MainWindowHandle,$dc,0)){throw 'Window capture failed'}}finally{$graphics.ReleaseHdc($dc);$graphics.Dispose()}
    $image=Join-Path $fixture 'configure.png';$bitmap.Save($image,[Drawing.Imaging.ImageFormat]::Png);$bitmap.Dispose()
    if(-not $window.CloseMainWindow()){throw 'Could not close test window'}
    if(-not $process.WaitForExit(5000)){throw 'Action did not exit after window close'}
    $process.Refresh()
    Write-Output ('Configure exit code: '+$process.ExitCode)
    if($process.ExitCode -ne 0){throw 'Action exited with an error'}
    'Configure UI passed: actual protocol launch, window displayed with held producer lock and unavailable server, normal close and successful exit.'
    'UI image: '+$image
}finally{if($process -and -not $process.HasExited){$process.Kill();$process.WaitForExit()};if($lock){$lock.Dispose()};foreach($key in $keys){[Environment]::SetEnvironmentVariable($key,$prior[$key])}}
