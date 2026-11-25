<#
.SYNOPSIS
  Self-test of scripts/strays.ps1, run by `cargo xtask strays --self-test`.

.DESCRIPTION
  Starts four hidden processes and holds their objects: `python -` with a sleeping script on
  stdin (a signature only an agent makes), `python -m http.server 0 --bind 127.0.0.1` (a
  report-only signature: the owner may run one), and as controls `python -c "...sleep..."` (python
  by name, no signature) and `ping -n 600 127.0.0.1` (nothing). Checks that the report finds the
  first, young at the default thresholds and a stray at zero thresholds, lists the server as a
  stray to report only, and that -Stop stops the first and nothing else. Every process it started is stopped in `finally`, and the test fails if one is
  left. -ProcessId keeps the script to these three: it never touches anything else on the PC.
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
    $server = Start-Hidden $python '-m http.server 0 --bind 127.0.0.1' $null
    $started += $server
    Start-Sleep -Milliseconds 500
    $ids = @($stdinScript.Id, $pythonControl.Id, $ping.Id, $server.Id) -join ','
    Write-Output "strays self-test: python - is pid $($stdinScript.Id); controls pid $($pythonControl.Id) (python -c), $($ping.Id) (ping)"

    $young = Invoke-Strays @('-ProcessId', $ids)
    Assert-That ($young.Code -eq 0 -and $young.Text -match "young pid $($stdinScript.Id)\b") `
        'at the default thresholds `python -` is listed, young, and the report exits 0' $young.Text

    $report = Invoke-Strays @('-AgeMinutes', '0', '-CpuSeconds', '0', '-ProcessId', $ids)
    Assert-That ($report.Code -eq 1 -and $report.Text -match "STRAY pid $($stdinScript.Id)\b") `
        'at zero thresholds it is a stray and the report exits 1' $report.Text
    Assert-That ($report.Text -notmatch "pid $($pythonControl.Id)\b" -and $report.Text -notmatch "pid $($ping.Id)\b") `
        'control: python -c and ping are not reported' $report.Text
    Assert-That (-not $stdinScript.HasExited) 'report mode stopped nothing' ''
    Assert-That ($report.Text -match "stray \(report only\) pid $($server.Id)\b") `
        'python -m http.server is listed as a stray to report only' $report.Text

    $stop = Invoke-Strays @('-Stop', '-AgeMinutes', '0', '-CpuSeconds', '0', '-ProcessId', $ids)
    $stopped = @([regex]::Matches($stop.Text, 'stopped pid (\d+)') | ForEach-Object { $_.Groups[1].Value })
    Assert-That ($stop.Code -eq 0 -and $stopped.Count -eq 1 -and $stopped[0] -eq "$($stdinScript.Id)") `
        '-Stop stops exactly `python -`' $stop.Text
    Assert-That ($stdinScript.WaitForExit(5000)) '`python -` has ended' ''
    Assert-That ((-not $pythonControl.HasExited) -and (-not $ping.HasExited)) `
        'control: python -c and ping are still running after -Stop' ''
    Assert-That (-not $server.HasExited) 'the report-only server is still running after -Stop' ''
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
