param([Parameter(Mandatory=$true)][string]$ActivityWatchDirectory,[switch]$ValidateOnly)
$ErrorActionPreference='Stop'
$root=(Resolve-Path -LiteralPath $ActivityWatchDirectory).Path
$exe=Join-Path $root 'aw-qt.exe'
if(-not (Test-Path -LiteralPath $exe -PathType Leaf)){throw 'ActivityWatchDirectory must contain aw-qt.exe'}
# A newly activated Shell.Application object can retain the tool's isolated context.
# Obtain the already-running desktop shell's dispatch object instead.
$shell=New-Object -ComObject Shell.Application
$handle=0
$desktop=$shell.Windows().FindWindowSW(0,0,8,[ref]$handle,1)
if(-not $desktop -or -not $desktop.Document.Application){throw 'Interactive desktop shell unavailable. Start ActivityWatch manually; no tool-owned fallback is allowed.'}
if($ValidateOnly){'Desktop shell dispatch available; no process launched.';return}
if(Get-Process aw-qt -ErrorAction SilentlyContinue | Where-Object {$_.Path -eq $exe}){'ActivityWatch is already running; no duplicate launched.';return}
$desktop.Document.Application.ShellExecute($exe,'',$root,'open',0)
'Launch requested through the existing Windows desktop shell.'
'Verify the configured server and storage identity before any history migration.'
