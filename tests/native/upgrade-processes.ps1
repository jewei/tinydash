param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('Record', 'Stop')]
    [string]$Action,
    [Parameter(Mandatory = $true)]
    [int]$RootPid,
    [Parameter(Mandatory = $true)]
    [string]$Record
)
$ErrorActionPreference = 'Stop'
if ($env:RUNNER_ENVIRONMENT -ne 'github-hosted') {
    throw 'Upgrade process checks require a disposable GitHub-hosted runner.'
}
if ($Action -eq 'Record') {
    $rows = @(Get-CimInstance Win32_Process)
    $ids = [Collections.Generic.HashSet[int]]::new()
    $null = $ids.Add($RootPid)
    do {
        $count = $ids.Count
        foreach ($row in $rows) {
            if ($ids.Contains([int]$row.ParentProcessId)) { $null = $ids.Add([int]$row.ProcessId) }
        }
    } while ($ids.Count -ne $count)
    $owned = @(foreach ($id in $ids) {
        $process = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($process) { @{ id = $id; started = $process.StartTime.ToUniversalTime().ToString('o') } }
    })
    ConvertTo-Json -InputObject @($owned) | Set-Content $Record
} else {
    $owned = @(Get-Content -Raw $Record | ConvertFrom-Json)
    foreach ($entry in $owned) {
        $process = Get-Process -Id $entry.id -ErrorAction SilentlyContinue
        if ($process -and $process.StartTime.ToUniversalTime().ToString('o') -eq $entry.started) {
            Stop-Process -Id $entry.id -Force
            if (-not $process.WaitForExit(10000)) { throw "Owned process did not stop: $($entry.id)" }
        }
    }
    foreach ($entry in $owned) {
        $process = Get-Process -Id $entry.id -ErrorAction SilentlyContinue
        if ($process -and $process.StartTime.ToUniversalTime().ToString('o') -eq $entry.started) {
            throw "Owned process remains: $($entry.id)"
        }
    }
}
