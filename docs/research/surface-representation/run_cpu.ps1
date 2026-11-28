# Every CPU measurement of the study, one process at a time, below normal priority, four threads.
#   powershell -File run_cpu.ps1 [check|whole|contrib|time|boxes|all]
param([string]$What = "all")
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot
$b = ".\bench\target\release\srbench.exe"
$shapes = "bracket", "sphere", "torus", "bunny", "dragon"
if ($What -in "all", "check") { .\low.ps1 data\c1-check.json $b check .work }
if ($What -in "all", "whole") {
    foreach ($s in $shapes) { .\low.ps1 "data\whole-$s.json" $b whole ".work\$s" }
    .\low.ps1 data\c2-partition.json $b partition .work\bracket 2
}
if ($What -in "all", "contrib") {
    foreach ($s in $shapes) { foreach ($k in 0..3) { .\low.ps1 "data\contrib-$s-L$k.json" $b contrib ".work\$s" $k } }
}
if ($What -in "all", "time") {
    foreach ($s in $shapes) { foreach ($k in 1, 3) { .\low.ps1 "data\time-$s-L$k.json" $b time ".work\$s" $k } }
}
if ($What -in "all", "boxes") {
    foreach ($s in $shapes) {
        python tree.py boxes $s
        foreach ($kind in "octree", "sweep") { .\low.ps1 "data\boxes-$s-$kind-run.json" $b boxes ".work\$s" ".work\$s\$kind.f64" }
    }
}
