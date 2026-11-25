<#
.SYNOPSIS
  Self-test of scripts/strays.ps1, run by `cargo xtask strays --self-test`.

.DESCRIPTION
  Starts four hidden processes and holds their objects: `python -` with a sleeping script on
  stdin, a python whose command line names `-m http.server` but which only sleeps (a report-only
  signature: the owner may run a server; this one serves nothing), and as controls `python -c "...sleep..."` (python by name, no signature) and
  `ping -n 600 127.0.0.1` (nothing). Their parent is this script's PowerShell. Records of xtask
  (as `cargo xtask` writes under target/strays) are written to folders of the test's own under
  the temp folder and passed with -Records, never the real ones:
    unrecorded: no record: `python -` is a stray to report only, and -Stop stops nothing (the
      ancestry rule does not take any parent, claude.exe included, for xtask's);
    recorded: this PowerShell recorded with its real start: `python -` is xtask's, a stray at
      zero thresholds, and -Stop stops it and nothing else;
    CPU only: recorded, but young by the clock and over the CPU threshold: report only;
    reused id: this PowerShell's id recorded with a window an hour before it started: not xtask's;
    an orphan (its parent ended): not xtask's when it started after the parent's record was last
      alive, xtask's (and stopped) when it started while the parent was alive.
  The owner's own runs write no record (`a_run_records_itself_and_removes_the_record`), so they
  are the unrecorded case.
  Every process it started is stopped in `finally`, and the test fails if one is left. -ProcessId
  keeps the script to these four: it never touches anything else on the PC.
#>
$ErrorActionPreference = 'Stop'
$script = Join-Path $PSScriptRoot 'strays.ps1'
$failures = New-Object System.Collections.Generic.List[string]
$started = @()

function Start-Hidden([string] $file, [string] $arguments, [string] $stdin) {
    $info = New-Object System.Diagnostics.ProcessStartInfo
    $info.FileName = $file
    $info.Arguments = $arguments
    $info.UseShellExecute = $false
    $info.CreateNoWindow = $true
    $info.RedirectStandardInput = $true
    $info.RedirectStandardOutput = $true
    $process = [System.Diagnostics.Process]::Start($info)
    if ($stdin) { $process.StandardInput.Write($stdin) }
    $process.StandardInput.Close()
    return $process
}

function Invoke-Strays([string[]] $arguments) {
    $output = & powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -File $script @arguments
    return [pscustomobject] @{ Code = $LASTEXITCODE; Text = ($output -join "`n") }
}

function Assert-That([bool] $condition, [string] $what, [string] $detail) {
    if ($condition) { Write-Output "  ok    $what" }
    else { Write-Output "  FAIL  $what`n$detail"; $failures.Add($what) }
}

$python = (Get-Command python.exe -ErrorAction SilentlyContinue).Source
if (-not $python) { Write-Output 'strays self-test: python.exe is not on PATH'; exit 1 }

try {
    $stdinScript = Start-Hidden $python '-' "import time`ntime.sleep(600)`n"
    $started += $stdinScript
    $pythonControl = Start-Hidden $python '-c "import time; time.sleep(600)"' $null
    $started += $pythonControl
    $ping = Start-Hidden "$env:SystemRoot\System32\PING.EXE" '-n 600 127.0.0.1' $null
    $started += $ping
    # The report-only signature by its command line alone: `-m http.server` after `-c`, so python
    # sleeps and serves nothing (no server is started on the PC).
    $server = Start-Hidden $python '-c "import time; time.sleep(600)" -m http.server' $null
    $started += $server
    Start-Sleep -Milliseconds 500
    $ids = @($stdinScript.Id, $pythonControl.Id, $ping.Id, $server.Id) -join ','
    Write-Output "strays self-test: python - is pid $($stdinScript.Id); controls pid $($pythonControl.Id) (python -c), $($ping.Id) (ping)"

    # The records: none; this PowerShell with its real start; and with a window an hour too early.
    $scratch = Join-Path ([IO.Path]::GetTempPath()) "cav-strays-test-$PID"
    $none = Join-Path $scratch 'none'
    $real = Join-Path $scratch 'real'
    $early = Join-Path $scratch 'early'
    foreach ($dir in @($none, $real, $early)) { [void] (New-Item -ItemType Directory -Force -Path $dir) }
    $start = ([DateTimeOffset] (Get-Process -Id $PID).StartTime).ToUnixTimeMilliseconds()
    Set-Content -LiteralPath (Join-Path $real "xtask-$PID.txt") -Value "$PID $($start - 2000) $($start + 2000) self-test"
    Set-Content -LiteralPath (Join-Path $early "xtask-$PID.txt") -Value "$PID $($start - 3600000) $($start - 3500000) self-test"

    $young = Invoke-Strays @('-ProcessId', $ids, '-Records', $real)
    Assert-That ($young.Code -eq 0 -and $young.Text -match "young pid $($stdinScript.Id)\b") `
        'at the default thresholds `python -` is listed, young, and the report exits 0' $young.Text

    # Unrecorded: report only, and -Stop stops nothing.
    $report = Invoke-Strays @('-AgeMinutes', '0', '-CpuSeconds', '0', '-ProcessId', $ids, '-Records', $none)
    Assert-That ($report.Code -eq 0 -and $report.Text -match "stray \(report only\) pid $($stdinScript.Id)\b.*not started by xtask") `
        'unrecorded: `python -` is a stray to report only' $report.Text
    Assert-That ($report.Text -notmatch "pid $($pythonControl.Id)\b" -and $report.Text -notmatch "pid $($ping.Id)\b") `
        'control: python -c and ping are not reported' $report.Text
    $stop = Invoke-Strays @('-Stop', '-AgeMinutes', '0', '-CpuSeconds', '0', '-ProcessId', $ids, '-Records', $none)
    Assert-That ($stop.Text -notmatch 'stopped pid' -and -not $stdinScript.HasExited) `
        'unrecorded: -Stop stops nothing' $stop.Text

    # A reused id: the record's window is not when this PowerShell started.
    $reused = Invoke-Strays @('-AgeMinutes', '0', '-CpuSeconds', '0', '-ProcessId', $ids, '-Records', $early)
    Assert-That ($reused.Text -match "stray \(report only\) pid $($stdinScript.Id)\b.*not started by xtask") `
        'a recorded id whose window is not the process''s start is not xtask''s' $reused.Text

    # CPU only: young by the clock, over the CPU threshold: report only, never stopped.
    $cpu = Invoke-Strays @('-Stop', '-AgeMinutes', '1000', '-CpuSeconds', '0', '-ProcessId', $ids, '-Records', $real)
    Assert-That ($cpu.Text -match "stray \(report only\) pid $($stdinScript.Id)\b.*xtask started it" -and
        $cpu.Text -notmatch 'stopped pid' -and -not $stdinScript.HasExited) `
        'CPU time alone never stops a process, even one xtask started' $cpu.Text

    # Recorded and old: a stray that may be stopped; the server stays report only.
    $report = Invoke-Strays @('-AgeMinutes', '0', '-CpuSeconds', '0', '-ProcessId', $ids, '-Records', $real)
    Assert-That ($report.Code -eq 1 -and $report.Text -match "STRAY pid $($stdinScript.Id)\b.*xtask started it") `
        'recorded: at zero thresholds it is a stray xtask started, and the report exits 1' $report.Text
    Assert-That (-not $stdinScript.HasExited) 'report mode stopped nothing' ''
    Assert-That ($report.Text -match "stray \(report only\) pid $($server.Id)\b") `
        'python -m http.server is listed as a stray to report only' $report.Text

    $stop = Invoke-Strays @('-Stop', '-AgeMinutes', '0', '-CpuSeconds', '0', '-ProcessId', $ids, '-Records', $real)
    $stopped = @([regex]::Matches($stop.Text, 'stopped pid (\d+)') | ForEach-Object { $_.Groups[1].Value })
    Assert-That ($stop.Code -eq 0 -and $stopped.Count -eq 1 -and $stopped[0] -eq "$($stdinScript.Id)") `
        'recorded: -Stop stops exactly `python -`' $stop.Text
    Assert-That ($stdinScript.WaitForExit(5000)) '`python -` has ended' ''
    Assert-That ((-not $pythonControl.HasExited) -and (-not $ping.HasExited)) `
        'control: python -c and ping are still running after -Stop' ''
    Assert-That (-not $server.HasExited) 'the report-only server is still running after -Stop' ''

    # An orphan (S4's second review, J1): `python -` started by a PowerShell that then ends, so
    # its parent is gone. It is xtask's only when it started while the recorded run was alive.
    $sleeper = Join-Path $scratch 'sleep.py'
    Set-Content -LiteralPath $sleeper -Value "import time`ntime.sleep(600)"
    # The orphan's id comes back in a file: it inherits the parent's output, which would not end
    # while it runs.
    $idFile = Join-Path $scratch 'orphan.txt'
    $command = "(Start-Process -FilePath '$python' -ArgumentList '-' -RedirectStandardInput '$sleeper' " +
        "-NoNewWindow -PassThru).Id | Set-Content -LiteralPath '$idFile'"
    $parent = Start-Hidden 'powershell.exe' "-NoProfile -NonInteractive -Command `"$command`"" $null
    $parentStart = ([DateTimeOffset] $parent.StartTime).ToUnixTimeMilliseconds()
    [void] $parent.WaitForExit(20000)
    $orphanId = [int] ((Get-Content -LiteralPath $idFile -TotalCount 1).Trim())
    $orphan = [System.Diagnostics.Process]::GetProcessById($orphanId)
    $started += $orphan
    $orphanStart = ([DateTimeOffset] $orphan.StartTime).ToUnixTimeMilliseconds()
    Assert-That ($parent.HasExited -and $orphanStart -gt $parentStart + 1) 'the orphan''s parent has ended' ''
    $ended = Join-Path $scratch 'ended'
    $alive = Join-Path $scratch 'alive'
    foreach ($dir in @($ended, $alive)) { [void] (New-Item -ItemType Directory -Force -Path $dir) }
    # The parent recorded, alive until just before the orphan started: a later process with the
    # parent's id could not be told from this, so it must not count.
    Set-Content -LiteralPath (Join-Path $ended "xtask-$($parent.Id).txt") `
        -Value "$($parent.Id) $($parentStart - 2000) $($parentStart + 1) $($orphanStart - 1) self-test"
    Set-Content -LiteralPath (Join-Path $alive "xtask-$($parent.Id).txt") `
        -Value "$($parent.Id) $($parentStart - 2000) $($parentStart + 1) $($orphanStart + 60000) self-test"
    $gone = Invoke-Strays @('-Stop', '-AgeMinutes', '0', '-CpuSeconds', '0', '-ProcessId', "$orphanId", '-Records', $ended)
    Assert-That ($gone.Text -match "stray \(report only\) pid $orphanId\b.*not started by xtask" -and
        $gone.Text -notmatch 'stopped pid' -and -not $orphan.HasExited) `
        'an orphan started after its recorded parent''s last heartbeat is not xtask''s, and is not stopped' $gone.Text
    $owned = Invoke-Strays @('-Stop', '-AgeMinutes', '0', '-CpuSeconds', '0', '-ProcessId', "$orphanId", '-Records', $alive)
    Assert-That ($owned.Text -match "stopped pid $orphanId\b" -and $orphan.WaitForExit(5000)) `
        'control: an orphan started while its recorded parent was alive is xtask''s, and is stopped' $owned.Text
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
} finally {
    foreach ($p in $started) {
        if (-not $p.HasExited) { $p.Kill(); [void] $p.WaitForExit(5000) }
    }
    $left = @($started | Where-Object { -not $_.HasExited })
    Assert-That ($left.Count -eq 0) 'every process the test started has ended' ($left.Id -join ', ')
}

if ($failures.Count -gt 0) { Write-Output "strays self-test: $($failures.Count) failed"; exit 1 }
Write-Output 'strays self-test: passed'
exit 0
