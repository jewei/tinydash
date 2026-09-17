param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Install', 'Remove')]
    [string]$Action
)

$ErrorActionPreference = 'Stop'
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Installer checks require a disposable GitHub-hosted runner.'
}
$installDir = Join-Path $env:LOCALAPPDATA 'TinyDash'
$binary = Join-Path $installDir 'tinydash.exe'
$shortcut = Join-Path ([Environment]::GetFolderPath('Programs')) 'TinyDash.lnk'
$registry = 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\TinyDash'
$settings = Join-Path $env:APPDATA 'dev.tinydash.launcher\settings.json'
$marker = Join-Path $env:LOCALAPPDATA 'dev.tinydash.launcher\installer-check.txt'
$settingsText = '{"currencyRatesEnabled":false,"clearQueryOnOpen":true}'

function Run-Installer([string]$Path, [string]$Arguments) {
    $process = Start-Process -FilePath $Path -ArgumentList $Arguments -PassThru
    if (-not $process.WaitForExit(120000)) {
        $process.Kill()
        throw 'The installer did not finish within two minutes.'
    }
    if ($process.ExitCode -ne 0) { throw "Installer exit code: $($process.ExitCode)" }
}

function Check-Data {
    if ((Get-Content -Raw $settings).Trim() -ne $settingsText) { throw 'Settings changed during installation or removal.' }
    if ((Get-Content -Raw $marker).Trim() -ne 'preserve user data') { throw 'Local user data changed.' }
}

if ($Action -eq 'Install') {
    if ((Test-Path $installDir) -or (Test-Path $registry) -or (Test-Path $settings)) {
        throw 'The installer test needs a clean TinyDash profile.'
    }
    $installers = @(Get-ChildItem 'native-build/*-setup.exe')
    if ($installers.Count -ne 1) { throw 'Expected one NSIS installer.' }
    Run-Installer $installers[0].FullName '/S'
    if (-not (Test-Path $binary)) { throw 'The installed executable is missing.' }
    # Tauri changes its bundle-type marker during packaging. Compare with the
    # NSIS payload, not the standalone executable. Hosted Windows includes 7-Zip.
    $payloadDir = Join-Path $env:RUNNER_TEMP 'TinyDash-package'
    & 7z x $installers[0].FullName "-o$payloadDir" -y tinydash.exe -r | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Could not extract the NSIS payload.' }
    $payload = @(Get-ChildItem $payloadDir -Filter 'tinydash.exe' -Recurse)
    if ($payload.Count -ne 1) { throw 'Expected one packaged executable.' }
    $expected = (Get-FileHash $payload[0].FullName -Algorithm SHA256).Hash
    if ((Get-FileHash $binary -Algorithm SHA256).Hash -ne $expected) { throw 'The installed executable differs from the package.' }
    $shell = New-Object -ComObject WScript.Shell
    if (-not (Test-Path $shortcut)) { throw 'The Start menu shortcut is missing.' }
    if ($shell.CreateShortcut($shortcut).TargetPath -ne $binary) { throw 'The Start menu shortcut has the wrong target.' }
    if ((Get-ItemProperty $registry).DisplayName -ne 'TinyDash') { throw 'The uninstall entry is missing.' }

    New-Item -ItemType Directory -Force (Split-Path $settings), (Split-Path $marker) | Out-Null
    Set-Content $settings $settingsText -NoNewline
    Set-Content $marker 'preserve user data' -NoNewline
    # Check same-version replacement without deleting the saved profile.
    Run-Installer $installers[0].FullName '/S'
    Check-Data
    "TINYDASH_NATIVE_BINARY=$binary" | Out-File $env:GITHUB_ENV -Encoding utf8 -Append
    Write-Output 'PASS: per-user installation, executable, shortcut, uninstall entry, and reinstall'
} else {
    # _?= prevents NSIS from detaching a temporary child process. Keep the path
    # last and unquoted, as required by NSIS, even when it contains spaces.
    $uninstaller = Join-Path $env:RUNNER_TEMP 'TinyDash-uninstall-check.exe'
    Copy-Item (Join-Path $installDir 'uninstall.exe') $uninstaller
    Run-Installer $uninstaller "/S _?=$installDir"
    foreach ($path in @($binary, $shortcut, $registry)) {
        if (Test-Path $path) { throw "Removal left an installed item: $path" }
    }
    Check-Data
    Write-Output 'PASS: removal deleted the app and shortcut and preserved user data'
}
