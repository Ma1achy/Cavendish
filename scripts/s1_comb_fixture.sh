#!/bin/sh
# How crates/cav-sim/tests/fixtures/s1-comb-signal.txt was made: the comb scene's signal printed by
# the engine as it was at the tag s1-point-mass. It is a record of what S1 computed; run this only
# to check the record, never to replace it with what today's code computes.
set -e
git worktree add --detach target/s1-tag s1-point-mass
(cd target/s1-tag && CARGO_TARGET_DIR=../wt-s1 cargo build -q -p cav)
for n in 1 2 3 4 5; do
    target/wt-s1/debug/cav exec "load comb; signal $n" > "target/tmp/s1-comb-$n.txt"
done
git worktree remove --force target/s1-tag
