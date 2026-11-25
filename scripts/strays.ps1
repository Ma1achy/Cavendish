<#
.SYNOPSIS
  Lists (and with -Stop, stops) processes that look like an agent's leftovers on this PC.

.DESCRIPTION
  A process is a candidate by its command line or its path, and three build tools by name alone:
    python/python3 running a script from stdin (`python -`) or `python -m http.server`;
    `xtask.exe ... web serve`;
    cav.exe, cav-app.exe or cav-web.exe built in this repository's target folder (or a worktree's);
    headless Chrome or Edge (the browser process) whose --user-data-dir is under a temp or
      scratchpad folder;
    node.exe running wasm-bindgen-test-runner or scripts/web_screenshot.mjs, and
      wasm-bindgen-test-runner.exe itself;
    wasm-opt.exe and cargo-nextest.exe (build tools, by name alone).
  A candidate is a stray when it is older than -AgeMinutes or has used more than -CpuSeconds of CPU.

  Who may be stopped (S17's second review, item 3, and S4's review, I10: the owner runs some of
  these too, and the owner's own Claude Code session is a claude.exe like an agent's): only a
  process an agent's `cargo xtask` started. A `cargo xtask` run by an agent (Claude Code sets
  CLAUDECODE in its shells; the owner's own runs write nothing, but a run through the owner's own
  interactive Claude Code session counts as an agent's) writes a record of itself under
  `target/strays/` of its checkout or worktree: its id, when it started, and how long it has been
  alive, renewed every 30 s; it removes the record when it ends normally, and one killed part-way
  leaves it. A candidate is xtask's when it is a recorded xtask, or a recorded xtask is among its
  ancestors: its parent, or its parent's parent while each is still running, each started no
  later than its child; or a parent that has ended, when the candidate started while that run was
  alive. A recorded id counts only for a process that started within the record's window, so a
  reused id is not taken for xtask's.
    stoppable when xtask's and older than -AgeMinutes (wall-clock age only: CPU time alone never
      makes a stop): `xtask web serve`, the repository's cav binaries, `python -`, headless
      browsers, node running the web test runner or the screenshot script;
    report only, whoever started them: `python -m http.server`, wasm-opt, cargo-nextest and
      wasm-bindgen-test-runner.exe, which match by name or by a command line anyone types;
    report only: every candidate that is not xtask's (an agent's own `python -`, a process whose
      xtask ended normally, one whose parent has ended and was not recorded).
  A stray that may not be stopped is listed as such and does not fail the report.

  -Records names the folders of records to use instead of every checkout's `target/strays` (the
  self-test uses it); a folder that does not exist holds no records.

  Report mode (the default) changes nothing: exit 0 when there is no stray that may be stopped,
  1 when there is.
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
    [string] $ProcessId,
    # Semicolon-separated folders of xtask's records; by default every checkout's target\strays.
    [string] $Records
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

# Who may stop a process of each signature: 'xtask' (when xtask started it and it is old) or
# 'report' (never). A signature missing here is 'report'.
$stopRules = @{
    'python - (script on stdin)'     = 'xtask'
    'headless browser, scratchpad'   = 'xtask'
    'xtask web serve'                = 'xtask'
    'headless browser, temp profile' = 'xtask'
    'node wasm-bindgen-test-runner'  = 'xtask'
    'node web_screenshot.mjs'        = 'xtask'
    'python -m http.server'          = 'report'
    'wasm-bindgen-test-runner'       = 'report'
    'wasm-opt'                       = 'report'
    'cargo-nextest'                  = 'report'
}
foreach ($name in @('cav.exe', 'cav-app.exe', 'cav-web.exe')) { $stopRules["$name from target"] = 'xtask' }

# The folders of xtask's records: -Records, else target\strays of the main checkout and of each
# worktree under it.
$recordFolders = if ($Records) {
    @($Records -split ';' | Where-Object { $_.Trim() } | ForEach-Object { $_.Trim() })
} else {
    @(Join-Path $repo 'target\strays') + @(Get-ChildItem -Directory -ErrorAction SilentlyContinue `
            (Join-Path $repo '.claude\worktrees') | ForEach-Object { Join-Path $_.FullName 'target\strays' })
}

# xtask's records: each file holds `<id> <earliest start> <latest start> <alive until> ...`, Unix
# ms. `alive until` is moved on every 30 s while the run lives (a heartbeat), so a run killed
# part-way leaves a record whose window has an end; a record of three numbers is taken as alive
# until its latest start.
$recorded = @{}
foreach ($folder in $recordFolders) {
    if (-not (Test-Path -LiteralPath $folder -PathType Container)) { continue }
    foreach ($file in @(Get-ChildItem -LiteralPath $folder -File -Filter '*.txt' -ErrorAction SilentlyContinue)) {
        $words = @(([string] (Get-Content -LiteralPath $file.FullName -TotalCount 1 -ErrorAction SilentlyContinue)) -split '\s+' |
                Where-Object { $_ })
        if ($words.Count -lt 3) { continue }
        $id = 0; $from = [long] 0; $to = [long] 0; $alive = [long] 0
        if ([int]::TryParse($words[0], [ref] $id) -and [long]::TryParse($words[1], [ref] $from) -and
            [long]::TryParse($words[2], [ref] $to)) {
            if ($words.Count -lt 4 -or -not [long]::TryParse($words[3], [ref] $alive)) { $alive = $to }
            $recorded[$id] = @($from, $to, [math]::Max($alive, $to))
        }
    }
}

# A CIM date as Unix milliseconds.
function ConvertTo-UnixMs($date) {
    return ([DateTimeOffset] $date).ToUnixTimeMilliseconds()
}

# Whether $process is a recorded xtask: its id recorded, and it started within the record's window.
function Test-Recorded($process) {
    $window = $recorded[[int] $process.ProcessId]
    if (-not $window -or -not $process.CreationDate) { return $false }
    $at = ConvertTo-UnixMs $process.CreationDate
    return $at -ge $window[0] -and $at -le $window[1]
}

# Whether xtask started $process: it is a recorded xtask, or one is among its ancestors. The walk
# goes up through parents that are still running, each started no later than its child (an id can
# be reused); a parent that has ended counts only when its id is recorded and the child started
# while that run was alive (from its start to the record's last heartbeat): a later process that
# took the id cannot pass for it. Anything else (claude.exe included) is not xtask's.
function Test-XtaskStarted($process) {
    if (Test-Recorded $process) { return $true }
    $child = $process
    for ($depth = 0; $depth -lt 32; $depth++) {
        $parentId = [int] $child.ParentProcessId
        if ($parentId -le 0) { return $false }
        $parent = @(Get-CimInstance -ClassName Win32_Process -Filter "ProcessId=$parentId")
        if ($parent.Count -ne 1) {
            # The parent has ended: xtask's only when its id is recorded and the child was
            # started while that run was alive.
            $window = $recorded[$parentId]
            if (-not $window -or -not $child.CreationDate) { return $false }
            $at = ConvertTo-UnixMs $child.CreationDate
            return [bool] ($at -ge $window[0] -and $at -le $window[2])
        }
        $parent = $parent[0]
        if ($parent.CreationDate -and $child.CreationDate -and $parent.CreationDate -gt $child.CreationDate) {
            return $false
        }
        if (Test-Recorded $parent) { return $true }
        $child = $parent
    }
    return $false
}

# The signature a process matches, or $null.
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
                if ($dir -match '\\scratchpad(\\|$|")') { return 'headless browser, scratchpad' }
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
        $rule = $stopRules[$signature]
        if (-not $rule) { $rule = 'report' }
        $xtasks = ($rule -eq 'xtask') -and (Test-XtaskStarted $p)
        [pscustomobject] @{
            Id         = [int] $p.ProcessId
            Signature  = $signature
            AgeMinutes = [math]::Round($age, 1)
            CpuSeconds = [math]::Round($cpu, 1)
            WorkingMB  = [math]::Round([double] $p.WorkingSetSize / 1MB, 1)
            Stray      = ($age -gt $AgeMinutes) -or ($cpu -gt $CpuSeconds)
            # Stopped only when xtask started it and it is old: CPU time alone never stops one.
            Stoppable  = $xtasks -and ($age -gt $AgeMinutes)
            Xtasks     = $xtasks
            Started    = $p.CreationDate
            Command    = $p.CommandLine
        }
    }
}

$only = @($ProcessId -split ',' | Where-Object { $_.Trim() } | ForEach-Object { [int] $_.Trim() })
$found = @(Get-StrayCandidates $only | Sort-Object -Property Stray, CpuSeconds -Descending)
$strays = @($found | Where-Object { $_.Stray -and $_.Stoppable })
$kept = @($found | Where-Object { $_.Stray -and -not $_.Stoppable })

if ($found.Count -eq 0) {
    Write-Output 'strays: no process matches an agent-leftover signature.'
    exit 0
}

Write-Output ("strays: {0} matching process(es); {1} stray that may be stopped, {2} stray to report only (older than {3} min or over {4} CPU s):" -f
    $found.Count, $strays.Count, $kept.Count, $AgeMinutes, $CpuSeconds)
foreach ($c in $found) {
    $mark = if (-not $c.Stray) { 'young' } elseif ($c.Stoppable) { 'STRAY' } else { 'stray (report only)' }
    $who = if ($c.Xtasks) { 'xtask started it' } else { 'not started by xtask' }
    $command = [string] $c.Command
    if ($command.Length -gt 160) { $command = $command.Substring(0, 157) + '...' }
    Write-Output ("  {0} pid {1,-6} {2,-30} age {3} min, cpu {4} s, {5} MB; {6}" -f
        $mark, $c.Id, $c.Signature, $c.AgeMinutes, $c.CpuSeconds, $c.WorkingMB, $who)
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
            $_.Signature -eq $c.Signature -and $_.Started -eq $c.Started -and $_.Stoppable })
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
