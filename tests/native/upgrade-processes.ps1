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
    # Windows PowerShell returns the JSON array as one pipeline object.
    # Wrapping that pipeline in @() would add an unwanted nested array.
    $owned = ConvertFrom-Json -InputObject (Get-Content -Raw $Record)
    foreach ($entry in $owned) {
        $process = Get-Process -Id $entry.id -ErrorAction SilentlyContinue
        if (-not $process) { continue }
        try {
            # Keep an OS handle open so PID reuse cannot change the target.
            try {
                $null = $process.Handle
            } catch {
                if ($process.HasExited) { continue }
                throw
            }
            if ($process.StartTime.ToUniversalTime().ToString('o') -ne $entry.started) { continue }
            try {
                Stop-Process -InputObject $process -Force -ErrorAction Stop
            } catch {
                # WebView children can exit after the parent is stopped.
                # Suppress that race only when this exact process has exited.
                if (-not $process.HasExited) { throw }
            }
            if (-not $process.WaitForExit(10000)) { throw "Owned process did not stop: $($entry.id)" }
        } finally {
            $process.Dispose()
        }
    }
    foreach ($entry in $owned) {
        $process = Get-Process -Id $entry.id -ErrorAction SilentlyContinue
        if ($process -and $process.StartTime.ToUniversalTime().ToString('o') -eq $entry.started) {
            throw "Owned process remains: $($entry.id)"
        }
    }
}
