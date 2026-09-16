$ErrorActionPreference = 'Continue'
Get-Process -Name tinydash,msedgewebview2,msedgedriver -ErrorAction SilentlyContinue |
    Select-Object Id,ProcessName,MainWindowHandle,Responding,StartTime |
    Format-Table -AutoSize
$processes = Get-CimInstance Win32_Process |
    Where-Object { $_.Name -in 'tinydash.exe','msedgewebview2.exe','msedgedriver.exe' }
$processes |
    Select-Object ProcessId,ParentProcessId,Name,CommandLine |
    Format-List
Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue |
    Where-Object { $_.OwningProcess -in $processes.ProcessId } |
    Select-Object LocalAddress,LocalPort,OwningProcess |
    Format-Table -AutoSize
foreach ($root in 'HKCU:','HKLM:') {
    foreach ($key in 'Software\Policies\Microsoft\Edge','Software\Policies\Microsoft\Edge\WebView2') {
        Get-ItemProperty -Path "$root\$key" -Name RemoteDebuggingAllowed,DeveloperToolsAvailability -ErrorAction SilentlyContinue |
            Format-List
    }
}
Get-WinEvent -FilterHashtable @{ LogName = 'Application'; StartTime = (Get-Date).AddMinutes(-5) } -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -match 'tinydash|msedgewebview2|msedgedriver' } |
    Select-Object TimeCreated,Id,Message |
    Format-List
