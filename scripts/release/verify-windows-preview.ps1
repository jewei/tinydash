$ErrorActionPreference = 'Stop'
function Test-UnsignedFile([string]$Path, [string]$Label) {
    if (-not (Test-Path $Path -PathType Leaf)) { throw "Missing ${Label}: $Path" }
    $signature = Get-AuthenticodeSignature $Path
    if ($signature.Status -ne 'NotSigned' -or $signature.SignerCertificate) {
        throw "Expected an unsigned Windows preview. $Label signature is $($signature.Status)."
    }
}
$installer = @(Get-ChildItem 'src-tauri/target/release/bundle/nsis/*-setup.exe' -File)
if ($installer.Count -ne 1) { throw 'Expected one NSIS installer.' }
Test-UnsignedFile $installer[0].FullName 'NSIS installer'
$extract = Join-Path $env:RUNNER_TEMP ('tinydash-installer-' + [guid]::NewGuid().ToString())
$sevenZip = (Get-Command 7z.exe -ErrorAction Stop).Source
try {
    New-Item -ItemType Directory -Path $extract | Out-Null
    & $sevenZip x $installer[0].FullName "-o$extract" -y
    if ($LASTEXITCODE -ne 0) { throw "7-Zip failed with exit code $LASTEXITCODE." }
    $binaries = @(Get-ChildItem $extract -Recurse -File -Filter 'tinydash.exe')
    if ($binaries.Count -ne 1) { throw 'Expected one bundled tinydash.exe in the NSIS installer.' }
    Test-UnsignedFile $binaries[0].FullName 'extracted bundled executable'
}
finally {
    Remove-Item $extract -Recurse -Force -ErrorAction SilentlyContinue
}
$updater = @(Get-ChildItem 'src-tauri/target/release/bundle/nsis/*-setup.exe.sig' -File)
if ($updater.Count -ne 1) { throw 'Expected one Windows updater signature.' }
$artifact = $updater[0].FullName.Substring(0, $updater[0].FullName.Length - 4)
if ($artifact -ne $installer[0].FullName) { throw 'Windows updater signature does not match the installer.' }
if ([string]::IsNullOrWhiteSpace((Get-Content $updater[0].FullName -Raw))) {
    throw 'Windows updater signature is empty.'
}
Write-Output 'PASS: Windows preview has no publisher signature; the updater signature file is present.'
