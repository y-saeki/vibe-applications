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
# windows-reactor-setup also stages the WebView2 projection, which only the
# WebView2 control loads; winwin has none.
$unused = @('Microsoft.Web.WebView2.Core.dll')
$files = Get-ChildItem -LiteralPath $root -File |
    Where-Object { ($_.Extension -in '.dll', '.pri') -and ($_.Name -notin $unused) }
# WinUI's messages come in a folder per language. winwin speaks Japanese, and
# en-US is what Windows falls back to; the other 90-odd folders only add weight.
$languages = @('ja-JP', 'en-US')
$dirs = Get-ChildItem -LiteralPath $root -Directory |
    Where-Object { $_.Name -notin $cargo } |
    Where-Object { $_.Name -notmatch '^[a-z]{2,3}(-[A-Za-z]+)+$' -or $_.Name -in $languages }
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

# What each part weighs, for whoever wonders why the installer is the size it is.
$sizes = @(Get-Item -LiteralPath $exe) + $files | ForEach-Object { [pscustomobject]@{ Name = $_.Name; KB = [math]::Round($_.Length / 1KB) } }
$sizes += $dirs | ForEach-Object {
    $bytes = (Get-ChildItem -LiteralPath $_.FullName -Recurse -File | Measure-Object Length -Sum).Sum
    [pscustomobject]@{ Name = "$($_.Name)\"; KB = [math]::Round($bytes / 1KB) }
}
$sizes | Sort-Object KB -Descending | Format-Table -AutoSize | Out-String | Write-Host
