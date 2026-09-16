$ErrorActionPreference = 'Continue'
Get-Process -Name tinydash,msedgewebview2,msedgedriver -ErrorAction SilentlyContinue |
    Select-Object Id,ProcessName,MainWindowHandle,Responding,StartTime |
    Format-Table -AutoSize
Get-WinEvent -FilterHashtable @{ LogName = 'Application'; StartTime = (Get-Date).AddMinutes(-5) } -ErrorAction SilentlyContinue |
    Where-Object { $_.Message -match 'tinydash|msedgewebview2|msedgedriver' } |
    Select-Object TimeCreated,Id,Message |
    Format-List
