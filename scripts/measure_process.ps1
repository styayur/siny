param([Parameter(Mandatory=$true)][string]$Executable, [string]$Output = 'work/acceptance/process.json', [int]$Seconds = 10)
$ErrorActionPreference = 'Stop'
$probeStarted = [Diagnostics.Stopwatch]::StartNew()
$probeProcess = Start-Process -FilePath (Resolve-Path -LiteralPath $Executable) -PassThru -WindowStyle Hidden
try {
  $windowMs = $null
  while ($probeStarted.ElapsedMilliseconds -lt 10000 -and -not $probeProcess.HasExited) {
    $probeProcess.Refresh()
    if ($probeProcess.MainWindowHandle -ne 0) { $windowMs = $probeStarted.ElapsedMilliseconds; break }
    Start-Sleep -Milliseconds 50
  }
  $cpuBefore = $probeProcess.TotalProcessorTime.TotalSeconds
  Start-Sleep -Seconds $Seconds
  $probeProcess.Refresh()
  $result = [ordered]@{ executable = $Executable; binary_bytes = (Get-Item -LiteralPath $Executable).Length; startup_to_window_ms = $windowMs; sample_seconds = $Seconds; idle_cpu_seconds = ($probeProcess.TotalProcessorTime.TotalSeconds - $cpuBefore); working_set_bytes = $probeProcess.WorkingSet64; peak_working_set_bytes = $probeProcess.PeakWorkingSet64; models_loaded = $false; limits = 'No inference; hidden idle window. Startup depends on OS/cache.' }
  New-Item -ItemType Directory -Force (Split-Path -Parent $Output) | Out-Null
  $result | ConvertTo-Json | Set-Content -LiteralPath $Output -Encoding utf8
  $result | ConvertTo-Json
} finally {
  if (-not $probeProcess.HasExited) { Stop-Process -Id $probeProcess.Id }
}
