# Run a program at below-normal priority, its stdout to a file: low.ps1 <out> <exe> <args...>
param([string]$Out, [string]$Exe, [Parameter(ValueFromRemainingArguments = $true)][string[]]$Rest)
$p = Start-Process -FilePath $Exe -ArgumentList $Rest -NoNewWindow -PassThru -RedirectStandardOutput $Out -RedirectStandardError "$Out.err"
$p.PriorityClass = 'BelowNormal'
$p.WaitForExit()
exit $p.ExitCode
