param(
  [Parameter(Mandatory = $true)][int]$RootProcessId,
  [Parameter(Mandatory = $true)][string]$OutputPath,
  [ValidateRange(1, 86400)][int]$DurationSeconds = 120,
  [ValidateRange(1, 3600)][int]$IntervalSeconds = 5
)

$ErrorActionPreference = 'Stop'
if ($IntervalSeconds -gt $DurationSeconds) {
  throw 'IntervalSeconds must not exceed DurationSeconds.'
}

function Get-ProcessTreeIds($tracked, $rows) {
  $live = @{}
  foreach ($row in $rows) { $live[[int]$row.ProcessId] = $row }
  foreach ($trackedId in @($tracked.Keys)) {
    if (-not $live.ContainsKey([int]$trackedId) -or
        [DateTime]$live[[int]$trackedId].CreationDate -ne $tracked[$trackedId]) {
      [void]$tracked.Remove($trackedId)
    }
  }
  do {
    $added = $false
    foreach ($row in $rows) {
      $childId = [int]$row.ProcessId
      $parentId = [int]$row.ParentProcessId
      if (-not $tracked.ContainsKey($childId) -and $tracked.ContainsKey($parentId) -and
          [DateTime]$row.CreationDate -ge $tracked[$parentId]) {
        $tracked[$childId] = [DateTime]$row.CreationDate
        $added = $true
      }
    }
  } while ($added)
  return @($tracked.Keys | Sort-Object)
}

$root = Get-CimInstance Win32_Process -Filter "ProcessId=$RootProcessId"
if (-not $root) { throw "Root process $RootProcessId is not running." }
$rootCreated = [DateTime]$root.CreationDate
$tracked = @{}
$tracked[$RootProcessId] = $rootCreated
$destination = [System.IO.Path]::GetFullPath($OutputPath)
$directory = [System.IO.Path]::GetDirectoryName($destination)
[System.IO.Directory]::CreateDirectory($directory) | Out-Null
$writer = [System.IO.StreamWriter]::new($destination, $false, [System.Text.UTF8Encoding]::new($false))
$started = [DateTimeOffset]::UtcNow

try {
  while ($true) {
    $rows = @(Get-CimInstance Win32_Process | Select-Object ProcessId, ParentProcessId, CreationDate)
    $currentRoot = $rows | Where-Object { [int]$_.ProcessId -eq $RootProcessId } | Select-Object -First 1
    $rootAlive = $null -ne $currentRoot -and [DateTime]$currentRoot.CreationDate -eq $rootCreated
    $ids = @(Get-ProcessTreeIds $tracked $rows)
    $workingSet = [int64]0
    $handles = 0
    $threads = 0
    $cpuSeconds = [double]0
    $measured = 0
    foreach ($processId in $ids) {
      try {
        $process = Get-Process -Id $processId -ErrorAction Stop
        $workingSet += [int64]$process.WorkingSet64
        $handles += [int]$process.HandleCount
        $threads += [int]$process.Threads.Count
        $cpuSeconds += [double]$process.CPU
        $measured += 1
      } catch {
        # Process exited during the census or counters were inaccessible.
      }
    }
    $elapsed = [DateTimeOffset]::UtcNow - $started
    $record = [ordered]@{
      at_utc = [DateTimeOffset]::UtcNow.ToString('o')
      elapsed_seconds = [Math]::Round($elapsed.TotalSeconds, 3)
      root_process_id = $RootProcessId
      root_creation_utc = $rootCreated.ToUniversalTime().ToString('o')
      root_alive = $rootAlive
      discovered_processes = $ids.Count
      measured_processes = $measured
      working_set_bytes = $workingSet
      handles = $handles
      threads = $threads
      cumulative_cpu_seconds = [Math]::Round($cpuSeconds, 3)
    }
    $writer.WriteLine(($record | ConvertTo-Json -Compress))
    $writer.Flush()
    if ($ids.Count -eq 0 -or $elapsed.TotalSeconds -ge $DurationSeconds) { break }
    Start-Sleep -Seconds $IntervalSeconds
  }
} finally {
  $writer.Dispose()
}
