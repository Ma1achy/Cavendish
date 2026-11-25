<#
.SYNOPSIS
  Lists (and with -Stop, stops) processes that look like an agent's leftovers on this PC.

.DESCRIPTION
  A process is a candidate only by its command line or its path, never by its name alone:
    python/python3 running a script from stdin (`python -`) or `python -m http.server`;
    `xtask.exe ... web serve`;
    cav.exe, cav-app.exe or cav-web.exe built in this repository's target folder (or a worktree's);
    headless Chrome or Edge (the browser process) whose --user-data-dir is under a temp or
      scratchpad folder;
    node.exe running wasm-bindgen-test-runner or scripts/web_screenshot.mjs, and
      wasm-bindgen-test-runner.exe itself;
    wasm-opt.exe and cargo-nextest.exe (build tools only agents and the tiers run).
  A candidate is a stray when it is older than -AgeMinutes or has used more than -CpuSeconds of CPU.

  Report mode (the default) changes nothing: exit 0 when there is no stray, 1 when there is.
  -Stop stops the strays, and only them: each one's command line is read again just before it is
  stopped, and it is stopped only if it still matches the same signature, is the same process (same
  start time) and is still a stray. Exit 0 when none is left, 1 when one could not be stopped.
  The owner asked for this clean-up (plan/PROGRESS.md, Owner's rulings, 24 Nov 2026).

  -ProcessId narrows everything to the given ids (the self-test uses it so that it stops only what
  it started); it never widens what matches.

  `cargo xtask strays [--stop]` runs this script. PowerShell 5.1, no dependencies.
#>
[CmdletBinding()]
param(
    [switch] $Stop,
    [double] $AgeMinutes = 30,
    [double] $CpuSeconds = 600,
    # Comma-separated ids (a string, so that `powershell -File` can pass it).
    [string] $ProcessId
)

$ErrorActionPreference = 'Stop'

# The main checkout: a worktree lives under <main>\.claude\worktrees\<name>, and its builds count too.
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$cut = $repo.IndexOf('\.claude\worktrees\', [StringComparison]::OrdinalIgnoreCase)
if ($cut -ge 0) { $repo = $repo.Substring(0, $cut) }
$repo = $repo.TrimEnd('\')

$tempRoots = @($env:TEMP, $env:TMP, [IO.Path]::GetTempPath()) |
    Where-Object { $_ } | ForEach-Object { $_.TrimEnd('\') } | Select-Object -Unique

$names = @('python.exe', 'python3.exe', 'xtask.exe', 'cav.exe', 'cav-app.exe', 'cav-web.exe',
    'chrome.exe', 'msedge.exe', 'chrome-headless-shell.exe', 'node.exe',
    'wasm-bindgen-test-runner.exe', 'wasm-opt.exe', 'cargo-nextest.exe')

# The arguments after the executable, as one string.
function Get-StrayArguments([string] $commandLine) {
    if ($commandLine -match '^\s*("[^"]*"|\S+)(.*)$') { return $Matches[2] }
    return ''
}

# Whether `python [options] -` reads its script from stdin: the first argument that is not an
# option is a lone dash. Options that take a value (-X, -W, -Q) take the next word.
function Test-StdinScript([string] $arguments) {
    $words = @($arguments -split '\s+' | Where-Object { $_ })
    for ($i = 0; $i -lt $words.Count; $i++) {
        $word = $words[$i]
        if ($word -eq '-') { return $true }
        if ($word -in @('-X', '-W', '-Q')) { $i++; continue }
        if ($word -in @('-c', '-m')) { return $false }
        if ($word.StartsWith('-')) { continue }
        return $false
    }
    return $false
}

function Test-UnderTemp([string] $path) {
    if (-not $path) { return $false }
    $path = $path.Trim('"').TrimEnd('\')
    if ($path -match '\\scratchpad(\\|$)') { return $true }
    foreach ($root in $tempRoots) {
        if ($path.StartsWith($root + '\', [StringComparison]::OrdinalIgnoreCase)) { return $true }
    }
    return $false
}

function Test-BuiltHere([string] $exe) {
    if (-not $exe) { return $false }
    return $exe.StartsWith($repo + '\', [StringComparison]::OrdinalIgnoreCase) -and
        ($exe -match '\\target\\')
}

# The signature a process matches, or $null. Never by name alone.
function Get-StraySignature($process) {
    $name = $process.Name.ToLowerInvariant()
    $line = [string] $process.CommandLine
    $arguments = Get-StrayArguments $line
    switch ($name) {
        { $_ -in @('python.exe', 'python3.exe') } {
            if ($arguments -match '(^|\s)-m\s+http\.server(\s|$)') { return 'python -m http.server' }
            if (Test-StdinScript $arguments) { return 'python - (script on stdin)' }
            return $null
        }
        'xtask.exe' {
            if ($arguments -match '(^|\s)web\s+serve(\s|$)') { return 'xtask web serve' }
            return $null
        }
        { $_ -in @('cav.exe', 'cav-app.exe', 'cav-web.exe') } {
            if (Test-BuiltHere $process.ExecutablePath) { return "$name from target" }
            return $null
        }
        { $_ -in @('chrome.exe', 'msedge.exe', 'chrome-headless-shell.exe') } {
            # The browser process only (its children carry --type=); stopping it ends them.
            if ($arguments -match '(^|\s)--type=') { return $null }
            if ($name -ne 'chrome-headless-shell.exe' -and $arguments -notmatch '(^|\s)--headless') { return $null }
            if ($arguments -match '--user-data-dir=("([^"]*)"|(\S+))') {
                $dir = if ($Matches[2]) { $Matches[2] } else { $Matches[3] }
                if (Test-UnderTemp $dir) { return 'headless browser, temp profile' }
            }
            return $null
        }
        'node.exe' {
            if ($arguments -match 'wasm-bindgen-test-runner') { return 'node wasm-bindgen-test-runner' }
            if ($arguments -match 'web_screenshot\.mjs') { return 'node web_screenshot.mjs' }
            return $null
        }
        'wasm-bindgen-test-runner.exe' { return 'wasm-bindgen-test-runner' }
        'wasm-opt.exe' { return 'wasm-opt' }
        'cargo-nextest.exe' { return 'cargo-nextest' }
    }
    return $null
}

function Get-StrayCandidates([int[]] $only) {
    $filter = ($names | ForEach-Object { "Name='$_'" }) -join ' OR '
    if ($only) { $filter = "($filter) AND (" + (($only | ForEach-Object { "ProcessId=$_" }) -join ' OR ') + ')' }
    $now = Get-Date
    foreach ($p in @(Get-CimInstance -ClassName Win32_Process -Filter $filter)) {
        $signature = Get-StraySignature $p
        if (-not $signature) { continue }
        $age = if ($p.CreationDate) { ($now - $p.CreationDate).TotalMinutes } else { 0 }
        $cpu = ([double] $p.KernelModeTime + [double] $p.UserModeTime) / 1e7
        [pscustomobject] @{
            Id         = [int] $p.ProcessId
            Signature  = $signature
            AgeMinutes = [math]::Round($age, 1)
            CpuSeconds = [math]::Round($cpu, 1)
            WorkingMB  = [math]::Round([double] $p.WorkingSetSize / 1MB, 1)
            Stray      = ($age -gt $AgeMinutes) -or ($cpu -gt $CpuSeconds)
            Started    = $p.CreationDate
            Command    = $p.CommandLine
        }
    }
}

$only = @($ProcessId -split ',' | Where-Object { $_.Trim() } | ForEach-Object { [int] $_.Trim() })
$found = @(Get-StrayCandidates $only | Sort-Object -Property Stray, CpuSeconds -Descending)
$strays = @($found | Where-Object Stray)

if ($found.Count -eq 0) {
    Write-Output 'strays: no process matches an agent-leftover signature.'
    exit 0
}

Write-Output ("strays: {0} matching process(es), {1} stray (older than {2} min or over {3} CPU s):" -f
    $found.Count, $strays.Count, $AgeMinutes, $CpuSeconds)
foreach ($c in $found) {
    $mark = if ($c.Stray) { 'STRAY' } else { 'young' }
    $command = [string] $c.Command
    if ($command.Length -gt 160) { $command = $command.Substring(0, 157) + '...' }
    Write-Output ("  {0} pid {1,-6} {2,-30} age {3} min, cpu {4} s, {5} MB" -f
        $mark, $c.Id, $c.Signature, $c.AgeMinutes, $c.CpuSeconds, $c.WorkingMB)
    Write-Output ("        $command")
}

if (-not $Stop) {
    if ($strays.Count -gt 0) {
        Write-Output 'strays: report only; `cargo xtask strays --stop` stops the STRAY ones by signature.'
        exit 1
    }
    exit 0
}

$left = 0
foreach ($c in $strays) {
    # Read it again: the id may have been reused, or the process may have changed or ended.
    $again = @(Get-StrayCandidates @($c.Id) | Where-Object {
            $_.Signature -eq $c.Signature -and $_.Started -eq $c.Started -and $_.Stray })
    if ($again.Count -ne 1) {
        Write-Output ("strays: pid {0} no longer matches; left alone." -f $c.Id)
        continue
    }
    try {
        Stop-Process -Id $c.Id -Force -ErrorAction Stop
        Write-Output ("strays: stopped pid {0} ({1})." -f $c.Id, $c.Signature)
    } catch {
        if (Get-Process -Id $c.Id -ErrorAction SilentlyContinue) {
            Write-Output ("strays: could not stop pid {0}: {1}" -f $c.Id, $_.Exception.Message)
            $left++
        } else {
            Write-Output ("strays: pid {0} had already ended." -f $c.Id)
        }
    }
}
if ($left -gt 0) { exit 1 }
exit 0
