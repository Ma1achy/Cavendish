"""Summarise folded stacks: top self time, top inclusive, by function (generic args stripped)."""
import re
import sys
from collections import Counter


def clean(name: str) -> str:
    # Strip generic arguments and closure noise for readability.
    out, depth = [], 0
    for ch in name:
        if ch == "<":
            depth += 1
            continue
        if ch == ">":
            depth -= 1
            continue
        if depth == 0:
            out.append(ch)
    s = "".join(out)
    s = re.sub(r"::\{\{closure\}\}", "::{closure}", s)
    return s


def main() -> None:
    path = sys.argv[1]
    top = int(sys.argv[2]) if len(sys.argv) > 2 else 30
    focus = sys.argv[3] if len(sys.argv) > 3 else None
    self_c: Counter[str] = Counter()
    incl: Counter[str] = Counter()
    total = 0
    import gzip

    opener = gzip.open if path.endswith(".gz") else open
    for line in opener(path, "rt", encoding="utf-8", errors="replace"):
        line = line.rstrip("\n")
        if not line:
            continue
        if " " not in line:
            continue
        stack, n = line.rsplit(" ", 1)
        count = int(n)
        frames = [clean(f) for f in stack.split(";")]
        if focus and not any(focus in f for f in frames):
            continue
        total += count
        self_c[frames[-1]] += count
        for f in set(frames):
            incl[f] += count
    print(f"total samples {total}")
    print("--- self ---")
    for f, c in self_c.most_common(top):
        print(f"{100 * c / total:6.2f}%  {f[:150]}")
    print("--- inclusive ---")
    for f, c in incl.most_common(top):
        print(f"{100 * c / total:6.2f}%  {f[:150]}")


main()
