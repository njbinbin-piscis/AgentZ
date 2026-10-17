$ErrorActionPreference = 'Stop'
$sampler = Join-Path $PSScriptRoot 'upgrade-resource-sampler.ps1'
$nonce = [guid]::NewGuid().ToString('N')
$fixture = Join-Path ([System.IO.Path]::GetTempPath()) "agentz-sampler-fixture-$nonce.ps1"
$childPidFile = Join-Path ([System.IO.Path]::GetTempPath()) "agentz-sampler-child-$nonce.txt"
$output = Join-Path ([System.IO.Path]::GetTempPath()) "agentz-sampler-output-$nonce.jsonl"
$rootProcess = $null
try {
  @'
param([string]$ChildPidFile)
$child = Start-Process -FilePath powershell.exe -ArgumentList '-NoProfile -Command Start-Sleep -Seconds 12' -WindowStyle Hidden -PassThru
$child.Id | Set-Content -LiteralPath $ChildPidFile
Start-Sleep -Seconds 4
'@ | Set-Content -LiteralPath $fixture
  $arguments = '-NoProfile -ExecutionPolicy Bypass -File "' + $fixture + '" -ChildPidFile "' + $childPidFile + '"'
  $rootProcess = Start-Process -FilePath powershell.exe -ArgumentList $arguments -WindowStyle Hidden -PassThru
  & $sampler -RootProcessId $rootProcess.Id -OutputPath $output -DurationSeconds 7 -IntervalSeconds 1
  $samples = @(Get-Content -LiteralPath $output | ConvertFrom-Json)
  if ($samples.Count -lt 2 -or -not (Test-Path -LiteralPath $childPidFile)) {
    throw 'Sampler fixture did not start its child or emit multiple samples.'
  }
  if (-not @($samples | Where-Object { $_.root_alive -eq $false -and $_.measured_processes -gt 0 }).Count) {
    throw 'Sampler lost a live descendant when the root exited.'
  }
  Write-Output "resource sampler orphan tracking verified: $($samples.Count) samples"
} finally {
  foreach ($file in @($fixture, $childPidFile, $output)) {
    Remove-Item -LiteralPath $file -Force -ErrorAction SilentlyContinue
  }
}
