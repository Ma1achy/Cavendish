#!/bin/sh
# Runs the study in four queues of one thread each (the build PC's limit for this task).
export OMP_NUM_THREADS=1 OPENBLAS_NUM_THREADS=1 MKL_NUM_THREADS=1
cd "$(dirname "$0")"
( python budget.py truth bunny && python budget.py run bunny 0 && python budget.py run bunny 1 ) > log-q1.txt 2>&1 &
( python budget.py truth bracket && python budget.py run bracket 0 && python budget.py run bracket 1 ) > log-q2.txt 2>&1 &
( python budget.py run cube 0 && python budget.py run cube 1 && python budget.py run cube 2 ) > log-q3.txt 2>&1 &
( until [ -f truth-bunny.json ]; do sleep 20; done; python budget.py run bunny 2;
  until [ -f truth-bracket.json ]; do sleep 20; done; python budget.py run bracket 2 ) > log-q4.txt 2>&1 &
wait
