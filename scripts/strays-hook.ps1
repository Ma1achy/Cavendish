<#
.SYNOPSIS
  The Claude Code Stop and SubagentStop hook (.claude/settings.json): runs the strays report.

.DESCRIPTION
  Says nothing when no stray is running. When there is one, it prints hook JSON: `systemMessage`
  shows the report to the owner, and `hookSpecificOutput.additionalContext` gives it to the model
  (the session, or the subagent that is finishing) once; when the hook is already continuing a
  stop (`stop_hook_active`), it shows the message only, so a stray nobody can stop never loops a
  turn. Never stops anything, and always exits 0: a hook that fails must not hold up a turn.
#>
$ErrorActionPreference = 'Stop'
try {
    $hookEvent = 'Stop'
    $again = $false
    if ([Console]::IsInputRedirected) {
        $raw = [Console]::In.ReadToEnd()
        if ($raw.Trim()) {
            $payload = $raw | ConvertFrom-Json
            if ($payload.hook_event_name) { $hookEvent = [string] $payload.hook_event_name }
            $again = [bool] $payload.stop_hook_active
        }
    }
    $report = & (Join-Path $PSScriptRoot 'strays.ps1')
    if ($LASTEXITCODE -ne 1) { exit 0 }
    $text = ($report -join "`n") + "`nStop what you started (by its handle, or with " +
        '`cargo xtask strays --stop`, which stops only these signatures), then carry on.'
    $out = @{ systemMessage = "Leftover processes:`n$text" }
    if (-not $again) {
        $out.hookSpecificOutput = @{ hookEventName = $hookEvent; additionalContext = $text }
    }
    Write-Output ($out | ConvertTo-Json -Depth 4 -Compress)
} catch {
    Write-Output (@{ systemMessage = "strays hook failed: $($_.Exception.Message)" } | ConvertTo-Json -Compress)
}
exit 0
