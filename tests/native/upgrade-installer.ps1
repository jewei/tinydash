param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Install', 'Remove', 'Version')]
    [string]$Action,
    [string]$Package
)

$ErrorActionPreference = 'Stop'
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Upgrade installation requires a disposable GitHub-hosted runner.'
}
$installDir = Join-Path $env:LOCALAPPDATA 'TinyDash'
$registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\TinyDash'
if ($Action -eq 'Version') {
    Write-Output (Get-ItemProperty $registry).DisplayVersion
    exit 0
}
if ($Action -eq 'Install') {
    $process = Start-Process -FilePath $Package -ArgumentList '/S' -PassThru
} else {
    $uninstaller = Join-Path $env:RUNNER_TEMP 'TinyDash-upgrade-uninstall.exe'
    Copy-Item (Join-Path $installDir 'uninstall.exe') $uninstaller
    $process = Start-Process -FilePath $uninstaller -ArgumentList "/S _?=$installDir" -PassThru
}
if (-not $process.WaitForExit(120000)) {
    $process.Kill()
    throw 'The installer did not finish within two minutes.'
}
if ($process.ExitCode -ne 0) { throw "Installer exit code: $($process.ExitCode)" }
if ($Action -eq 'Remove') {
    $shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'TinyDash.lnk'
    foreach ($path in @((Join-Path $installDir 'tinydash.exe'), $shortcut, $registry)) {
        if (Test-Path $path) { throw "Removal left an installed item: $path" }
    }
}
