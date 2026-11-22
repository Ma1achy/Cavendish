#!/bin/sh
# Cold build (dependencies and the 120-type lib) and the lib alone, in the dev profile; crates in the tree.
for f in "" sc bv fc; do
  feat=${f:+--features $f}; td=/tmp/refl_td_${f:-s}; rm -rf $td
  s=$(date +%s.%N); cargo build $feat --target-dir $td >/dev/null 2>&1; e=$(date +%s.%N)
  ts=""
  for i in 1 2 3; do touch src/lib.rs; a=$(date +%s.%N); cargo build $feat --target-dir $td >/dev/null 2>&1; b=$(date +%s.%N); ts="$ts $(printf '%.2f' $(echo "$b - $a" | bc))"; done
  echo "'${f:-serde only}': cold total $(printf '%.1f' $(echo "$e - $s" | bc)) s; lib alone:$ts s; crates in tree: $(cargo tree $feat --prefix none --edges normal | sort -u | wc -l)"
done
