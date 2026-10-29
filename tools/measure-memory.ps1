# Samples whole-process memory for a running PirateLink window.
#
#   powershell -ExecutionPolicy Bypass -File tools\measure-memory.ps1 -Seconds 30
#
# The Tauri process alone is meaningless: WebView2 spawns a browser process plus
# GPU/utility/renderer children, and the budget has to be measured across all of
# them. This walks the child tree from the app's PID.

param(
    [int]$Seconds = 30,
    [string]$ProcessName = "piratelink",
    [int]$IntervalMs = 2000
)

$app = Get-Process -Name $ProcessName -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $app) {
    Write-Error "No '$ProcessName' process found. Start the release build first."
    exit 1
}

function Get-Family([int]$RootPid) {
    $all = Get-CimInstance Win32_Process -ErrorAction SilentlyContinue
    $ids = [System.Collections.Generic.HashSet[int]]::new()
    [void]$ids.Add($RootPid)

    # Repeat until no new children appear (Chromium nests several levels deep).
    $changed = $true
    while ($changed) {
        $changed = $false
        foreach ($proc in $all) {
            if ($ids.Contains([int]$proc.ParentProcessId) -and -not $ids.Contains([int]$proc.ProcessId)) {
                [void]$ids.Add([int]$proc.ProcessId)
                $changed = $true
            }
        }
    }

    $rows = foreach ($id in $ids) {
        $p = Get-Process -Id $id -ErrorAction SilentlyContinue
        if ($p) {
            [pscustomobject]@{
                Pid  = $id
                Name = $p.ProcessName
                MB   = [math]::Round($p.WorkingSet64 / 1MB, 1)
            }
        }
    }
    return $rows
}

Write-Output "sampling $ProcessName (pid $($app.Id)) for $Seconds s every $($IntervalMs) ms"
Write-Output ""

$samples = @()
$deadline = (Get-Date).AddSeconds($Seconds)

while ((Get-Date) -lt $deadline) {
    $family = Get-Family -RootPid $app.Id
    $total = ($family | Measure-Object MB -Sum).Sum
    $samples += [pscustomobject]@{
        Time  = (Get-Date).ToString("HH:mm:ss")
        Total = [math]::Round($total, 1)
        Procs = $family.Count
    }
    Start-Sleep -Milliseconds $IntervalMs
}

$samples | Format-Table -AutoSize

$peak = ($samples | Measure-Object Total -Maximum).Maximum
$avg = [math]::Round(($samples | Measure-Object Total -Average).Average, 1)
$last = $samples[-1].Total

Write-Output ""
"steady : {0:N1} MB" -f $last
"average: {0:N1} MB" -f $avg
"peak   : {0:N1} MB" -f $peak
""

Write-Output "per process at end:"
Get-Family -RootPid $app.Id | Sort-Object MB -Descending | Format-Table -AutoSize
