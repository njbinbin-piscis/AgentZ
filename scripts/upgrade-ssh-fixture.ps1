# Owned localhost host-key handshake acceptance only. Never uses system sshd/config.
[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$fixtureRoot = Join-Path ([IO.Path]::GetTempPath()) ("agentz-ssh-fixture-" + [Guid]::NewGuid().ToString('N'))
$records = [Collections.Generic.List[object]]::new()
$checks = [ordered]@{}
$ownedProcesses = [Collections.Generic.List[Diagnostics.Process]]::new()
$status = 'blocked'
$failure = $null
$cleaned = $false

function Text-Hash([string]$text) {
    [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($text))).ToLowerInvariant()
}
function Start-Owned([string]$program, [string[]]$arguments) {
    $info = [Diagnostics.ProcessStartInfo]::new($program)
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $info.RedirectStandardInput = $true
    foreach ($argument in $arguments) { $info.ArgumentList.Add($argument) }
    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $info
    if (-not $process.Start()) { throw "Owned process failed to start" }
    $process.StandardInput.Close()
    $ownedProcesses.Add($process)
    [pscustomobject]@{ process=$process; stdout=$process.StandardOutput.ReadToEndAsync(); stderr=$process.StandardError.ReadToEndAsync() }
}
function Record-Owned($owned, [string]$name, [int]$timeoutMs = 15000) {
    $timedOut = -not $owned.process.WaitForExit($timeoutMs)
    if ($timedOut) { $owned.process.Kill(); $owned.process.WaitForExit() }
    $stdout = $owned.stdout.GetAwaiter().GetResult()
    $stderr = $owned.stderr.GetAwaiter().GetResult()
    $record = [pscustomobject]@{
        name=$name; pid=$owned.process.Id; exitCode=$owned.process.ExitCode; timedOut=$timedOut
        stdoutUtf8Sha256=(Text-Hash $stdout); stderrUtf8Sha256=(Text-Hash $stderr)
        stderrTail=$stderr.Substring([Math]::Max(0, $stderr.Length - 2048))
    }
    $records.Add($record)
    return $record
}
function Stop-Owned($owned, [string]$name) {
    if (-not $owned.process.HasExited) { $owned.process.Kill() }
    $null = Record-Owned $owned $name
}
function Wait-OwnedListener($owned, [int]$port) {
    $deadline = [DateTime]::UtcNow.AddSeconds(15)
    while ([DateTime]::UtcNow -lt $deadline) {
        if ($owned.process.HasExited) { return $false }
        $listeners = @(Get-NetTCPConnection -LocalAddress '127.0.0.1' -LocalPort $port -State Listen -ErrorAction SilentlyContinue)
        if (@($listeners | Where-Object OwningProcess -eq $owned.process.Id).Count -gt 0) { return $true }
        Start-Sleep -Milliseconds 100
    }
    return $false
}
function Known-KeyMatches([string]$known, [string]$publicKey) {
    $key = ([IO.File]::ReadAllText($publicKey).Trim() -split '\s+')[1]
    return @([IO.File]::ReadAllLines($known) | Where-Object { $_ -notmatch '^\s*(#|$)' } | Where-Object { ($_ -split '\s+')[2] -eq $key }).Count -gt 0
}

try {
    $ssh = (Get-Command ssh -ErrorAction Stop).Source
    $sshd = (Get-Command sshd -ErrorAction Stop).Source
    $keygen = (Get-Command ssh-keygen -ErrorAction Stop).Source
    $null = Get-Command Get-NetTCPConnection -ErrorAction Stop
    $null = [IO.Directory]::CreateDirectory($fixtureRoot)
    $hostA = Join-Path $fixtureRoot 'host-a'
    $hostB = Join-Path $fixtureRoot 'host-b'
    $client = Join-Path $fixtureRoot 'client'
    foreach ($item in @(@('host-a', $hostA), @('host-b', $hostB), @('client', $client))) {
        $record = Record-Owned (Start-Owned $keygen @('-q','-t','ed25519','-N','','-f',$item[1])) "keygen-$($item[0])"
        if ($record.exitCode -ne 0) { throw "Fixture key generation failed" }
    }
    $knownA = Join-Path $fixtureRoot 'known-a'
    $knownB = Join-Path $fixtureRoot 'known-b'
    $globalKnown = Join-Path $fixtureRoot 'known-global'
    $clientConfig = Join-Path $fixtureRoot 'client-config'
    foreach ($file in @($knownA, $knownB, $globalKnown, $clientConfig)) { [IO.File]::WriteAllText($file, '') }
    $reservation = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $reservation.Start()
    $port = $reservation.LocalEndpoint.Port
    $reservation.Stop()
    $checks.port = $port
    $config = Join-Path $fixtureRoot 'sshd-config'
    $pidFile = Join-Path $fixtureRoot 'sshd.pid'
    $authorized = Join-Path $fixtureRoot 'authorized-keys'
    [IO.File]::WriteAllText($authorized, '') # nonexistent fixture username is never authenticated
    function Set-FixtureConfig([string]$hostKey) {
        $text = "Port $port`nListenAddress 127.0.0.1`nHostKey `"$($hostKey.Replace('\','/'))`"`nPidFile `"$($pidFile.Replace('\','/'))`"`nAuthorizedKeysFile `"$($authorized.Replace('\','/'))`"`nPasswordAuthentication no`nPubkeyAuthentication yes`nKbdInteractiveAuthentication no`nStrictModes yes`nLogLevel VERBOSE`nAllowUsers agentz-fixture-nonexistent`n"
        [IO.File]::WriteAllText($config, $text)
    }
    function Invoke-FixtureClient([string]$known, [string]$name) {
        $arguments = @('-F',$clientConfig,'-p',"$port",'-o',"UserKnownHostsFile=$known",'-o',"GlobalKnownHostsFile=$globalKnown",'-o','IdentityAgent=none','-o','IdentitiesOnly=yes','-i',$client,'-o','BatchMode=yes','-o','ConnectTimeout=5','-o','StrictHostKeyChecking=accept-new','-l','agentz-fixture-nonexistent','127.0.0.1','echo fixture')
        Record-Owned (Start-Owned $ssh $arguments) $name
    }
    Set-FixtureConfig $hostA
    $preflight = Record-Owned (Start-Owned $sshd @('-t','-f',$config)) 'sshd-config-preflight'
    if ($preflight.exitCode -ne 0) { throw 'Owned sshd preflight failed; no system modifications attempted' }
    $serverA = Start-Owned $sshd @('-D','-e','-f',$config)
    if (-not (Wait-OwnedListener $serverA $port)) { Stop-Owned $serverA 'sshd-a-unready'; throw 'Owned sshd A did not listen; no service/permission modifications attempted' }
    $first = Invoke-FixtureClient $knownA 'first-a-accept-new'
    $checks.firstAcceptWritesOwnedKnownHosts = Known-KeyMatches $knownA "$hostA.pub"
    $checks.firstAAuthenticationDenied = $first.stderrTail -match 'Permission denied|Authentication failed'
    Stop-Owned $serverA 'sshd-a-stop'
    Set-FixtureConfig $hostB
    $serverB = Start-Owned $sshd @('-D','-e','-f',$config)
    if (-not (Wait-OwnedListener $serverB $port)) { Stop-Owned $serverB 'sshd-b-unready'; throw 'Owned sshd B did not listen' }
    $changed = Invoke-FixtureClient $knownA 'changed-b-rejected'
    $checks.changedKeyRejected = $changed.exitCode -ne 0 -and $changed.stderrTail -match 'REMOTE HOST IDENTIFICATION HAS CHANGED'
    $checks.changedKeyDidNotReplaceA = Known-KeyMatches $knownA "$hostA.pub"
    $freshB = Invoke-FixtureClient $knownB 'fresh-b-accept-new-control'
    $checks.freshBPositiveControl = Known-KeyMatches $knownB "$hostB.pub"
    Stop-Owned $serverB 'sshd-b-stop'
    if (-not ($checks.firstAcceptWritesOwnedKnownHosts -and $checks.changedKeyRejected -and $checks.changedKeyDidNotReplaceA -and $checks.freshBPositiveControl)) { throw 'Owned SSH host-key handshake checks did not all pass' }
    $status = 'passed-host-key-handshake-only'
} catch {
    $failure = $_.Exception.Message
} finally {
    foreach ($process in $ownedProcesses) {
        try { if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit(5000) | Out-Null }; $process.Dispose() } catch { $failure = "$failure; owned process cleanup: $($_.Exception.Message)" }
    }
    if ([IO.Directory]::Exists($fixtureRoot)) {
        $resolved = [IO.Path]::GetFullPath($fixtureRoot)
        $expected = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
        if (-not $resolved.StartsWith($expected, [StringComparison]::OrdinalIgnoreCase) -or [IO.Path]::GetFileName($resolved) -notmatch '^agentz-ssh-fixture-[a-f0-9]{32}$') { throw 'Refusing cleanup outside owned fixture directory' }
        Remove-Item -LiteralPath $resolved -Recurse -Force
    }
    $cleaned = -not [IO.Directory]::Exists($fixtureRoot)
}
[pscustomobject]@{
    schemaVersion=1; status=$status; failure=$failure; checks=$checks; records=$records
    cleanedOwnedDirectory=$cleaned; authenticatedRemoteExecutionVerified=$false; reconnectWriteVerified=$false
    scope='Owned localhost SSH host-key handshake only; no user services/config/keys/hosts; stderr hashes use UTF8 decoded text'
} | ConvertTo-Json -Depth 8
if ($status -eq 'passed-host-key-handshake-only') { exit 0 } else { exit 2 }
