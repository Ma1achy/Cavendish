$ErrorActionPreference = "Continue"
$s = "C:\Users\Georg\AppData\Local\Temp\claude\06e6b27cf58b2ff3\29333d60-b123-4fc9-a131-a04033e79cc8\scratchpad"
$cav = "C:\src\Cavendish\.claude\worktrees\perf\target\release\cav.exe"
function Run-Low($exe, $argList, $out) {
    $p = Start-Process $exe -ArgumentList $argList -NoNewWindow -PassThru -RedirectStandardOutput "$out.out" -RedirectStandardError "$out.err"
    $p.PriorityClass = 'BelowNormal'
    $p.WaitForExit()
}
function Load { (Get-CimInstance Win32_Processor).LoadPercentage }
"start load $(Load) $(Get-Date -Format T)"
# 1. The flythrough over town, warm cache, plain (CPU and wait split) then GPU-timed.
Remove-Item Env:CAV_GPU_TIMING -ErrorAction SilentlyContinue
Remove-Item Env:CAV_THREADS -ErrorAction SilentlyContinue
Run-Low $cav @("bench","flythrough","--scene","town","--frames","600","--size","1280x720","--max-frame-ms","1000","--cache","$s\cache","--out","$s\q-fly-plain.csv") "$s\q-fly-plain"
"fly plain done load $(Load) $(Get-Date -Format T)"
$env:CAV_GPU_TIMING = "1"
Run-Low $cav @("bench","flythrough","--scene","town","--frames","600","--size","1280x720","--max-frame-ms","1000","--cache","$s\cache","--out","$s\q-fly-timed.csv") "$s\q-fly-timed"
Run-Low $cav @("bench","flythrough","--scene","town","--frames","300","--size","2560x1440","--max-frame-ms","1000","--cache","$s\cache","--out","$s\q-fly-timed-1440.csv") "$s\q-fly-timed-1440"
Remove-Item Env:CAV_GPU_TIMING
"fly timed done load $(Load) $(Get-Date -Format T)"
# 2. One thread (the page's shape of work).
$env:CAV_THREADS = "1"
Run-Low $cav @("bench","flythrough","--scene","town","--frames","600","--size","1280x720","--max-frame-ms","1000","--cache","$s\cache","--out","$s\q-fly-1t.csv") "$s\q-fly-1t"
# 3. cav run town on 1 and on the default threads.
foreach ($t in "1","4","default") {
    if ($t -eq "default") { Remove-Item Env:CAV_THREADS -ErrorAction SilentlyContinue } else { $env:CAV_THREADS = $t }
    Remove-Item -Recurse -Force "$s\q-run-$t" -ErrorAction SilentlyContinue
    $w = Measure-Command { Run-Low $cav @("run","town","--out","$s\q-run-$t") "$s\q-run-$t-log" }
    "run town threads $t : $($w.TotalSeconds) s load $(Load)"
}
Remove-Item Env:CAV_THREADS -ErrorAction SilentlyContinue
# 4. The kernel bench.
Run-Low "$s\kbench\target\release\kbench.exe" @() "$s\q-kbench"
"done $(Get-Date -Format T) load $(Load)"
