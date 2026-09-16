param(
    [Parameter(Mandatory = $true)][string]$Extension,
    [Parameter(Mandatory = $true)][string]$Binary,
    [switch]$Remove
)
$ErrorActionPreference = 'Stop'
# A unique test extension. Never change an existing document association.
if ($Extension -notmatch '^\.tinydash[0-9]+$') { throw 'Invalid test extension' }
$ProgId = "TinyDash.Native$Extension"
$ExtensionKey = "HKCU:\Software\Classes\$Extension"
$ProgramKey = "HKCU:\Software\Classes\$ProgId"
if ($Remove) {
    Remove-Item -LiteralPath $ExtensionKey -Recurse -Force -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $ProgramKey -Recurse -Force -ErrorAction SilentlyContinue
    exit
}
if ((Test-Path $ExtensionKey) -or (Test-Path $ProgramKey)) { throw 'Test association already exists' }
New-Item -Path $ExtensionKey -Force | Out-Null
Set-Item -LiteralPath $ExtensionKey -Value $ProgId
$CommandKey = "$ProgramKey\shell\open\command"
New-Item -Path $CommandKey -Force | Out-Null
Set-Item -LiteralPath $CommandKey -Value ('"' + $Binary + '" "%1"')
