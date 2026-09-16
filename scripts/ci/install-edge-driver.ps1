$ErrorActionPreference = 'Stop'

# Match the WebView2 runtime used by Tauri, rather than the installed Edge browser.
$client = 'Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}'
$version = $null
foreach ($root in @('HKLM:\SOFTWARE\WOW6432Node', 'HKLM:\SOFTWARE', 'HKCU:\SOFTWARE\WOW6432Node', 'HKCU:\SOFTWARE')) {
    $candidate = Get-ItemPropertyValue -Path "$root\$client" -Name pv -ErrorAction SilentlyContinue
    if ($candidate -match '^\d+\.\d+\.\d+\.\d+$') {
        $version = $candidate
        break
    }
}
if (-not $version) { throw 'A WebView2 Runtime installation is required.' }
if ($env:RUNNER_ARCH -ne 'X64') { throw 'This CI driver installer targets Windows x64.' }

$directory = Join-Path $env:RUNNER_TEMP 'tinydash-edge-driver'
New-Item -ItemType Directory -Force -Path $directory | Out-Null
$archive = Join-Path $directory 'driver.zip'
$url = "https://msedgedriver.microsoft.com/$version/edgedriver_win64.zip"
Write-Output "WebView2 runtime: $version"
Invoke-WebRequest -Uri $url -OutFile $archive
Expand-Archive -Path $archive -DestinationPath $directory -Force
$driver = Join-Path $directory 'msedgedriver.exe'
$reportedVersion = & $driver --version
if ($LASTEXITCODE -ne 0 -or $reportedVersion -notmatch [regex]::Escape($version)) {
    throw "The driver version does not match WebView2: $reportedVersion"
}
Write-Output $reportedVersion
$directory | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
