# Writes the NSIS include that lists what the installer ships: winwin.exe and
# the Windows App Runtime files the build staged beside it (build.rs,
# windows-reactor-setup). Run it after `cargo build --release`:
#
#   pwsh installer/payload.ps1 -Dir target/release -Out installer/payload.nsh
#
# The list is written out file by file so that the uninstaller removes exactly
# what was installed and nothing else a user keeps in the same folder.
param(
    [Parameter(Mandatory)] [string] $Dir,
    [Parameter(Mandatory)] [string] $Out
)
$ErrorActionPreference = 'Stop'

$root = (Resolve-Path $Dir).Path
$exe = Join-Path $root 'winwin.exe'
if (-not (Test-Path $exe)) { throw "no winwin.exe in $root" }
# The rest of the folder is Cargo's own.
$cargo = @('build', 'deps', 'examples', 'incremental', '.fingerprint')
$files = Get-ChildItem -LiteralPath $root -File | Where-Object { $_.Extension -in '.dll', '.pri' }
$dirs = Get-ChildItem -LiteralPath $root -Directory | Where-Object { $_.Name -notin $cargo }
if (-not ($files | Where-Object Name -eq 'Microsoft.WindowsAppRuntime.dll')) {
    throw "the Windows App Runtime is not staged in $root"
}

$lines = @('!macro INSTALL_PAYLOAD', "  File `"$exe`"")
$lines += $files | ForEach-Object { "  File `"$($_.FullName)`"" }
$lines += $dirs | ForEach-Object { "  File /r `"$($_.FullName)`"" }
$lines += '!macroend', '', '!macro UNINSTALL_PAYLOAD', '  Delete "$INSTDIR\winwin.exe"'
$lines += $files | ForEach-Object { "  Delete `"`$INSTDIR\$($_.Name)`"" }
$lines += $dirs | ForEach-Object { "  RMDir /r `"`$INSTDIR\$($_.Name)`"" }
$lines += '!macroend'
Set-Content -LiteralPath $Out -Value $lines -Encoding utf8
