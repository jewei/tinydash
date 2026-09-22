param(
    [ValidateSet('native', 'upgrade')]
    [string]$Test = 'native'
)

$ErrorActionPreference = 'Stop'
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'This script requires a disposable GitHub-hosted runner. Run bun run test:native without elevation on a local desktop.'
}

# Elevated WebView2 hosts ignore environment-based browser arguments.
# Use the supported machine policy for this executable on the disposable VM.
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
$principal = [Security.Principal.WindowsPrincipal]::new($identity)
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
    throw 'The GitHub Windows runner must have its default administrator privileges.'
}
$listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
$listener.Start()
$debugPort = $listener.LocalEndpoint.Port
$listener.Stop()
$key = 'HKLM:\Software\Policies\Microsoft\Edge\WebView2\AdditionalBrowserArguments'
$appName = 'tinydash.exe'
$existing = Get-ItemProperty -Path $key -Name $appName -ErrorAction SilentlyContinue
try {
    New-Item -Path $key -Force | Out-Null
    New-ItemProperty -Path $key -Name $appName -PropertyType String -Value "--remote-debugging-port=$debugPort" -Force | Out-Null
    $env:TINYDASH_NATIVE_DEBUG_PORT = "$debugPort"
    Write-Host "Testing an elevated WebView2 host on loopback port $debugPort."
    if ($Test -eq 'upgrade') {
        bun tests/native/upgrade.ts
    } else {
        bun run verify:native
    }
    if ($LASTEXITCODE -ne 0) { throw "Native test failed with exit code $LASTEXITCODE." }
} finally {
    if ($null -ne $existing) {
        Set-ItemProperty -Path $key -Name $appName -Value $existing.$appName
    } else {
        Remove-ItemProperty -Path $key -Name $appName -ErrorAction SilentlyContinue
    }
    Remove-Item Env:\TINYDASH_NATIVE_DEBUG_PORT -ErrorAction SilentlyContinue
}
