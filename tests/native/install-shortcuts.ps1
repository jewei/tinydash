param(
    [Parameter(Mandatory = $true)][string]$Binary,
    [Parameter(Mandatory = $true)][string]$Marker,
    [Parameter(Mandatory = $true)][string]$Prefix,
    [Parameter(Mandatory = $true)][string]$DirectoryName
)

$ErrorActionPreference = 'Stop'
$programs = [Environment]::GetFolderPath([Environment+SpecialFolder]::Programs)
if (-not $programs) { throw 'The user Start menu folder is unavailable.' }
$directory = Join-Path $programs $DirectoryName
New-Item -ItemType Directory -Path $directory -ErrorAction Stop | Out-Null
try {
    $shell = New-Object -ComObject WScript.Shell
    foreach ($label in @('Alpha', 'Beta')) {
        $shortcut = $shell.CreateShortcut((Join-Path $directory "$Prefix $label.lnk"))
        $shortcut.TargetPath = $Binary
        $shortcut.Arguments = '"' + $Marker + '" ' + $label
        $shortcut.WorkingDirectory = Split-Path $Binary
        $shortcut.Save()
    }
    Write-Output $directory
} catch {
    Remove-Item -Recurse -Force $directory
    throw
}
