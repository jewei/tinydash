param(
    [Parameter(Mandatory = $true)]
    [string]$ExpectedThumbprint
)

$ErrorActionPreference = 'Stop'
function Test-SignedFile([string]$Path, [string]$Label) {
    if (-not (Test-Path $Path -PathType Leaf)) { throw "Missing signed ${Label}: $Path" }
    $signature = Get-AuthenticodeSignature $Path
    if ($signature.Status -ne 'Valid') { throw "$Label signature is $($signature.Status)." }
    if (-not $signature.SignerCertificate) { throw "$Label has no signer certificate." }
    $actualThumbprint = $signature.SignerCertificate.Thumbprint.Replace(' ', '').ToUpperInvariant()
    if ($actualThumbprint -ne $ExpectedThumbprint.Replace(' ', '').ToUpperInvariant()) {
        throw "Unexpected $Label signing certificate: $actualThumbprint"
    }
    if (-not $signature.TimeStamperCertificate) { throw "$Label has no Authenticode timestamp." }
}
$installer = @(Get-ChildItem 'src-tauri/target/release/bundle/nsis/*-setup.exe' -File)
if ($installer.Count -ne 1) { throw 'Expected one NSIS installer.' }
Test-SignedFile $installer[0].FullName 'NSIS installer'
$extract = Join-Path $env:RUNNER_TEMP ('tinydash-installer-' + [guid]::NewGuid().ToString())
$sevenZip = (Get-Command 7z.exe -ErrorAction Stop).Source
try {
    New-Item -ItemType Directory -Path $extract | Out-Null
    & $sevenZip x $installer[0].FullName "-o$extract" -y
    if ($LASTEXITCODE -ne 0) { throw "7-Zip failed with exit code $LASTEXITCODE." }
    $binaries = @(Get-ChildItem $extract -Recurse -File -Filter 'tinydash.exe')
    if ($binaries.Count -ne 1) { throw 'Expected one bundled tinydash.exe in the NSIS installer.' }
    Test-SignedFile $binaries[0].FullName 'extracted bundled executable'
}
finally {
    Remove-Item $extract -Recurse -Force -ErrorAction SilentlyContinue
}
$updater = @(Get-ChildItem 'src-tauri/target/release/bundle/nsis/*-setup.exe.sig')
if ($updater.Count -ne 1) { throw 'Expected one Windows updater signature.' }
$artifact = $updater[0].FullName.Substring(0, $updater[0].FullName.Length - 4)
if (-not (Test-Path $artifact)) { throw 'Windows updater artifact is missing.' }
Write-Output 'PASS: Windows Authenticode signature and updater pair'
