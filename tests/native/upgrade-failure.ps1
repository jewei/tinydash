$ErrorActionPreference = 'Stop'
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Upgrade failure checks require a disposable GitHub-hosted runner.'
}

$source = (Get-Location).Path
$fixture = Join-Path $env:RUNNER_TEMP ("tinydash-upgrade-failure-" + [Guid]::NewGuid())
$evidence = Join-Path $source 'test-results/upgrade/partial-install-failure'
$names = @('APPDATA', 'LOCALAPPDATA', 'RUNNER_TEMP', 'TINYDASH_UPGRADE_TAG', 'TINYDASH_UPGRADE_SOURCE')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name) }
$changedDirectory = $false

function Invoke-FixtureGit {
    & git -C $fixture @args
    if ($LASTEXITCODE -ne 0) { throw 'Fixture Git command failed.' }
}

try {
    New-Item -ItemType Directory -Force $fixture, $evidence | Out-Null
    Invoke-FixtureGit init --quiet
    Invoke-FixtureGit -c user.name=Test -c user.email=test@example.invalid -c commit.gpgsign=false -c "core.hooksPath=$fixture/no-hooks" commit --quiet --allow-empty -m Test
    Invoke-FixtureGit tag v9.9.9
    $commit = Invoke-FixtureGit rev-parse HEAD
    foreach ($directory in @('native-build', 'payload', 'tests/native', 'src-tauri/target/release/bundle/nsis', 'temporary')) {
        New-Item -ItemType Directory -Force (Join-Path $fixture $directory) | Out-Null
    }
    Set-Content (Join-Path $fixture 'payload/tinydash.exe') 'Synthetic candidate bytes'
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    [IO.Compression.ZipFile]::CreateFromDirectory((Join-Path $fixture 'payload'), (Join-Path $fixture 'native-build/TinyDash_9.9.9_x64-setup.exe'))
    @{ source = @{ commit = $commit } } | ConvertTo-Json | Set-Content (Join-Path $fixture 'native-build/build.json')
    Set-Content (Join-Path $fixture 'src-tauri/target/release/bundle/nsis/TinyDash_0.0.0_x64-setup.exe') 'Installer fixture'
    @'
param([string]$Action, [string]$Package)
$ErrorActionPreference = 'Stop'
if ($Action -eq 'Install') {
    $directory = Join-Path $env:LOCALAPPDATA 'TinyDash'
    New-Item -ItemType Directory -Force $directory | Out-Null
    Set-Content (Join-Path $directory 'tinydash.exe') 'Partial installation'
    Write-Error 'Synthetic installer failure after writing the executable.'
} else {
    & (Join-Path $env:TINYDASH_UPGRADE_SOURCE 'tests/native/upgrade-installer.ps1') $Action
}
'@ | Set-Content (Join-Path $fixture 'tests/native/upgrade-installer.ps1')
    $env:APPDATA = Join-Path $fixture 'config'
    $env:LOCALAPPDATA = Join-Path $fixture 'installed'
    $env:RUNNER_TEMP = Join-Path $fixture 'temporary'
    $env:TINYDASH_UPGRADE_TAG = 'v9.9.9'
    $env:TINYDASH_UPGRADE_SOURCE = $source
    Push-Location $fixture
    $changedDirectory = $true
    & bun (Join-Path $source 'tests/native/upgrade.ts') *> (Join-Path $evidence 'exercise.log')
    $exitCode = $LASTEXITCODE
    $resultPath = Join-Path $fixture 'test-results/upgrade/result.json'
    Copy-Item $resultPath (Join-Path $evidence 'result.json')
    Copy-Item (Join-Path $fixture 'test-results/upgrade/processes.log') $evidence
    $result = Get-Content -Raw $resultPath | ConvertFrom-Json
    if ($exitCode -eq 0 -or $result.phase -ne 'install-older' -or -not $result.failure) {
        throw 'The fixture did not fail during installation.'
    }
    if (-not (Test-Path (Join-Path $env:LOCALAPPDATA 'TinyDash/tinydash.exe'))) {
        throw 'The fixture did not leave its partial installation.'
    }
    if ($result.cleanupComplete -ne $false -or $result.cleanupErrors.Count -eq 0) {
        throw 'Partial installation was incorrectly reported as clean.'
    }
    Write-Output 'PASS: partial installation without an uninstaller reports incomplete cleanup.'
} finally {
    if ($changedDirectory) { Pop-Location }
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name]) }
    if (Test-Path $fixture) { Remove-Item -Recurse -Force $fixture }
}
exit 0
