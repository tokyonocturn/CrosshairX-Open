param([int]$Seconds = 180, [int]$Interval = 15)
$procs = Get-Process crosshairx-open -ErrorAction SilentlyContinue
if (-not $procs) { Write-Output "app-not-running"; exit 2 }
$ids = $procs.Id
$cores = [Environment]::ProcessorCount
$start = @{}
foreach ($p in $procs) { $start[$p.Id] = $p.TotalProcessorTime.TotalMilliseconds }
$gpuSamples = New-Object System.Collections.ArrayList
$cpuSamples = New-Object System.Collections.ArrayList
$prev = $start.Clone()
$elapsed = 0
while ($elapsed -lt $Seconds) {
    Start-Sleep -Seconds $Interval
    $elapsed += $Interval
    $now = Get-Process crosshairx-open -ErrorAction SilentlyContinue
    foreach ($p in $now) {
        if ($prev.ContainsKey($p.Id)) {
            $delta = $p.TotalProcessorTime.TotalMilliseconds - $prev[$p.Id]
            $pct = 100.0 * $delta / ($Interval * 1000.0)
            [void]$cpuSamples.Add($pct)
            Write-Output ("t={0,3}s pid={1} cpuDelta={2,7:N1}ms oneCorePct={3,6:N2}" -f $elapsed, $p.Id, $delta, $pct)
            $prev[$p.Id] = $p.TotalProcessorTime.TotalMilliseconds
        }
    }
    try {
        $counters = (Get-Counter '\GPU Engine(*)\Utilization Percentage' -ErrorAction Stop).CounterSamples
    } catch {
        $counters = @()
    }
    $sum = 0.0
    foreach ($id in $ids) {
        foreach ($c in $counters) {
            if ($c.InstanceName -like "*pid_${id}*") { $sum += $c.CookedValue }
        }
    }
    [void]$gpuSamples.Add($sum)
    Write-Output ("            gpuPct={0,6:N2}" -f $sum)
}
$final = Get-Process crosshairx-open -ErrorAction SilentlyContinue
$totalMs = 0.0
foreach ($p in $final) { if ($start.ContainsKey($p.Id)) { $totalMs += $p.TotalProcessorTime.TotalMilliseconds - $start[$p.Id] } }
$avgCpu = 0.0; if ($cpuSamples.Count -gt 0) { $avgCpu = ($cpuSamples | Measure-Object -Average).Average }
$avgGpu = 0.0; $maxGpu = 0.0
if ($gpuSamples.Count -gt 0) { $avgGpu = ($gpuSamples | Measure-Object -Average).Average; $maxGpu = ($gpuSamples | Measure-Object -Maximum).Maximum }
Write-Output ""
Write-Output "RESULT cores=$cores seconds=$Seconds"
Write-Output ("RESULT totalCpuMs={0:N0} avgOneCorePct={1:N3} avgMachinePct={2:N4}" -f $totalMs, $avgCpu, ($avgCpu / $cores))
Write-Output ("RESULT avgGpuPct={0:N3} maxGpuPct={1:N3}" -f $avgGpu, $maxGpu)
Write-Output ("RESULT gpuSamples={0} cpuSamples={1}" -f $gpuSamples.Count, $cpuSamples.Count)
