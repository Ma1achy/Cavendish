#!/usr/bin/env python3
"""Draw the README's build-progress grid: one SVG, a square per task, grouped by slice.

Adapted from Principia's scripts/progress_banner.py (Ma1achy/Principia).

It is drawn in the README's style (docs/readme/: the title's neo-grotesque lettering and its inks on a transparent
ground), in two themes: `light`, dark ink for a light page, and `dark`, pale ink for a dark page, which the README
picks between with a <picture> as it does the title and signature. A merged task is a square of ink, one in review a
square of ochre wash, one not started a pencil outline.

Each task in plan/tasks.yaml is in one of three states:
  done         its status is `done` in tasks.yaml, or main's history has the merge commit
               "Merge pull request #N from <owner>/task/<TASK-id>": the subject GitHub gives the merge of the PR
               whose head is that task's branch. A branch that only starts with the task id
               (`task/TASK-C1-01-retry`) and a merge of main into a task branch do not count;
  in review    an open PR from this repository (not a fork) has head branch `task/<TASK-id>` (only with
               --open-prs, the JSON `gh pr list --state open --json headRefName,isCrossRepository` prints; a PR
               without `"isCrossRepository": false` is skipped, since anyone can open one from a fork's
               `task/...` branch);
  not started  otherwise.
A task whose status is `open` is never done, whatever main's history says: for a task whose branch name was
used by a PR that merged other work. The PR that does the task removes the line.

Progress is counted by task, not by size: each task weighs the same.

Usage:
  python3 scripts/progress_banner.py -o progress.svg [--open-prs open.json]
      [--merges subjects.txt] [--commit SHA] [--generated-at TEXT] [--theme light|dark]

--merges reads the merge-commit subjects from a file (one per line) instead of `git log --merges --format=%s`; a
shallow clone has no history to read. --commit and --generated-at fix the footer's commit and time (the defaults are
`git rev-parse --short HEAD` and the current UTC time). Needs Python 3 and PyYAML only.
"""

import argparse
import datetime
import json
import re
import subprocess
import sys
from pathlib import Path
from xml.sax.saxutils import escape

import yaml

ROOT = Path(__file__).resolve().parent.parent

MERGE_RE = re.compile(r"^Merge pull request #\d+ from [^/\s]+/task/(TASK-S\d+-\d+)\s*$")
HEAD_RE = re.compile(r"^task/(TASK-S\d+-\d+)$")
# Slice names from the slice table in plan/REBUILD.md: "| S0 | **The shell** | ...".
MILESTONE_RE = re.compile(r"^\| (S\d+) \| \*\*(.+?)\*\* \|")
TASK_ID_RE = re.compile(r"^TASK-S(\d+)-(\d+)$")

DONE, REVIEW, TODO = "done", "in review", "not started"

# Layout, in px: one ledger line per milestone across the README's width. The width is the title's (1600, as
# docs/readme/title-*.svg draws it), so the grid lines up with the title above it at width="100%".
WIDTH = 1600
PAD = 2  # side margin; the title's lettering starts at x = 2
CELL = 22  # task square
PITCH = 28  # square + gap
HEADER_H = 156  # title, overall rule, counts
LINE_H = 40  # a milestone's text line, above its squares
BLOCK_GAP = 30  # between milestones
FOOTER_H = 64

# The README's inks (docs/readme/title-*.svg, signature-*.svg) on a transparent ground, as the title and signature
# are drawn: `ink` the title's lettering, `ink2` its subtitle, `pencil` the signature's. `ochre` is a watercolour
# yellow ochre, the one wash, for tasks in review. A task merged is a square of ink; one not started, a pencil outline.
THEMES = {
    "light": {"ink": "#1d1b19", "ink2": "#3d3a34", "pencil": "#4f4a42", "ochre": "#b5822e"},
    "dark": {"ink": "#efe9dc", "ink2": "#c8c1b2", "pencil": "#c8c1b2", "ochre": "#d9a84e"},
}

# The title's lettering is a neo-grotesque; the runner (Ubuntu) has Liberation Sans or DejaVu Sans, and `sans-serif`
# ends the list.
FONTS = "'Helvetica Neue',Helvetica,Arial,'Liberation Sans',Arimo,'Nimbus Sans','DejaVu Sans',sans-serif"


def style(theme):
    c = THEMES[theme]
    return f"""
.t{{fill:{c['ink']};font-family:{FONTS}}}
.m{{fill:{c['ink2']}}}
.track{{stroke:{c['pencil']};stroke-opacity:0.45;stroke-width:1.5}}
.done{{fill:{c['ink']}}}
.review{{fill:{c['ochre']}}}
.todo{{fill:none;stroke:{c['pencil']};stroke-opacity:0.6;stroke-width:1.5}}
"""


def task_key(task_id):
    m = TASK_ID_RE.match(task_id)
    if not m:
        raise ValueError(f"task id {task_id!r} is not TASK-Sn-nn")
    return int(m.group(1)), int(m.group(2))


def load_tasks(path):
    tasks = yaml.safe_load(Path(path).read_text(encoding="utf-8"))
    if not isinstance(tasks, list):
        raise ValueError(f"{path}: expected a list of tasks")
    # The PC's own work (`machine: pc`) is not a checkpoint, so the grid leaves it out.
    tasks = [t for t in tasks if t.get("machine") != "pc"]
    return sorted(tasks, key=lambda t: task_key(t["id"]))


def milestone_names(path):
    names = {}
    try:
        text = Path(path).read_text(encoding="utf-8")
    except OSError:
        return names
    for line in text.splitlines():
        m = MILESTONE_RE.match(line)
        if m:
            names[m.group(1)] = m.group(2)
    return names


def merged_ids(subjects):
    """The task ids whose PR merge commit is among `subjects`."""
    return {m.group(1) for s in subjects for m in [MERGE_RE.match(s.strip())] if m}


def git(*args):
    return subprocess.run(
        ["git", "-C", str(ROOT), *args], check=True, capture_output=True, text=True
    ).stdout


def open_pr_ids(path):
    """The task ids of open PRs from this repository, from `gh pr list --state open --json
    headRefName,isCrossRepository` output. A PR from a fork, or one the list doesn't mark as from this repository,
    is skipped."""
    prs = json.loads(Path(path).read_text(encoding="utf-8"))
    return {
        m.group(1)
        for pr in prs
        if pr.get("isCrossRepository") is False
        for m in [HEAD_RE.match(pr.get("headRefName", ""))]
        if m
    }


def states(tasks, merged, in_review):
    out = {}
    for t in tasks:
        tid = t["id"]
        if t.get("status") == "open":
            # A PR from the task's branch merged work that was not the task (TASK-S5-23's branch
            # carried W3's held-out read, #265): the task is not done until this line goes.
            out[tid] = REVIEW if tid in in_review else TODO
        elif t.get("status") == "done" or tid in merged:
            out[tid] = DONE
        elif tid in in_review:
            out[tid] = REVIEW
        else:
            out[tid] = TODO
    return out


def pct(n, d):
    return 0 if d == 0 else int(100 * n / d)


def clip(text, n):
    return text if len(text) <= n else text[: n - 1].rstrip() + "…"


def svg(tasks, state, names, commit, generated_at, theme="light"):
    milestones = sorted({t["milestone"] for t in tasks}, key=lambda m: int(m[1:]))
    by_m = {m: [t for t in tasks if t["milestone"] == m] for m in milestones}
    total = len(tasks)
    done = sum(1 for s in state.values() if s == DONE)
    review = sum(1 for s in state.values() if s == REVIEW)

    inner = WIDTH - 2 * PAD
    per_row = (inner + (PITCH - CELL)) // PITCH

    def block_h(m):
        rows = -(-len(by_m[m]) // per_row)
        return LINE_H + rows * PITCH - (PITCH - CELL)

    height = HEADER_H + sum(block_h(m) for m in milestones) + BLOCK_GAP * (len(milestones) - 1) + FOOTER_H
    right = WIDTH - PAD

    o = []
    o.append(
        f'<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{height}" '
        f'viewBox="0 0 {WIDTH} {height}" role="img" aria-label="Cavendish build progress: '
        f'{pct(done, total)}% ({done} of {total} tasks done)">'
    )
    o.append(f"<title>Cavendish build progress: {done} of {total} tasks done</title>")
    o.append(f"<style>{style(theme)}</style>")

    # Header: title and overall % in the title's weight, a ruled line filled in ink as tasks merge, then the counts.
    o.append(f'<g data-part="header" data-done="{done}" data-total="{total}" data-pct="{pct(done, total)}">')
    o.append(
        f'<text class="t" x="{PAD}" y="56" font-size="56" font-weight="700" letter-spacing="-1.5">'
        f"Build progress</text>"
    )
    o.append(
        f'<text class="t" x="{right}" y="56" font-size="56" font-weight="700" letter-spacing="-1.5" '
        f'text-anchor="end">{pct(done, total)}%</text>'
    )
    rule_y = 86
    o.append(f'<line class="track" x1="{PAD}" y1="{rule_y}" x2="{right}" y2="{rule_y}"/>')
    if total:
        dw = inner * done / total
        rw = inner * review / total
        if rw:
            o.append(f'<rect class="review" x="{PAD + dw:.1f}" y="{rule_y - 3}" width="{rw:.1f}" height="6"/>')
        if dw:
            o.append(f'<rect class="done" x="{PAD}" y="{rule_y - 3}" width="{dw:.1f}" height="6"/>')
    o.append(
        f'<text class="t m" x="{PAD}" y="124" font-size="26">{done} done · {review} in review · '
        f"{total - done - review} not started · {total} tasks in {len(milestones)} slices</text>"
    )
    o.append("</g>")

    # One ledger line per milestone: its number and name, its count and %, then a square per task.
    y = HEADER_H
    for m in milestones:
        ts = by_m[m]
        md = sum(1 for t in ts if state[t["id"]] == DONE)
        mr = sum(1 for t in ts if state[t["id"]] == REVIEW)
        o.append(
            f'<g data-milestone="{m}" data-done="{md}" data-review="{mr}" data-total="{len(ts)}" '
            f'data-pct="{pct(md, len(ts))}">'
        )
        base = y + 26
        o.append(f'<text class="t" x="{PAD}" y="{base}" font-size="30" font-weight="700">{m}</text>')
        name = names.get(m, "")
        if name:
            o.append(
                f'<text class="t m" x="{PAD + 84}" y="{base}" font-size="26">'
                f"<title>{escape(m)} — {escape(name)}</title>{escape(clip(name, 72))}</text>"
            )
        o.append(
            f'<text class="t" x="{right}" y="{base}" font-size="30" font-weight="700" '
            f'text-anchor="end">{pct(md, len(ts))}%</text>'
        )
        count = f"{md}/{len(ts)}" + (f" · {mr} in review" if mr else "")
        o.append(
            f'<text class="t m" x="{right - 96}" y="{base}" font-size="24" '
            f'text-anchor="end">{escape(count)}</text>'
        )
        gy = y + LINE_H
        for k, t in enumerate(ts):
            s = state[t["id"]]
            cls = {DONE: "done", REVIEW: "review", TODO: "todo"}[s]
            cx = PAD + (k % per_row) * PITCH
            cy = gy + (k // per_row) * PITCH
            tip = f"{t['id']}: {t['title']} ({s})"
            # An outline is inset by half its stroke, so every square covers the same area.
            inset = 0.75 if cls == "todo" else 0
            o.append(
                f'<rect class="{cls}" x="{cx + inset}" y="{cy + inset}" width="{CELL - 2 * inset}" '
                f'height="{CELL - 2 * inset}" rx="2" '
                f'data-task="{escape(t["id"])}" data-state="{s}"><title>{escape(tip)}</title></rect>'
            )
        o.append("</g>")
        y += block_h(m) + BLOCK_GAP

    # Footer: legend, then provenance.
    fy = y - BLOCK_GAP + FOOTER_H - 14
    lx = PAD
    for cls, label in (("done", "done (merged)"), ("review", "in review (open PR)"), ("todo", "not started")):
        inset = 0.75 if cls == "todo" else 0
        o.append(
            f'<rect class="{cls}" x="{lx + inset}" y="{fy - 17 + inset}" width="{18 - 2 * inset}" '
            f'height="{18 - 2 * inset}" rx="2"/>'
        )
        o.append(f'<text class="t m" x="{lx + 28}" y="{fy}" font-size="22">{label}</text>')
        lx += 28 + int(11.5 * len(label)) + 36
    o.append(
        f'<text class="t m" x="{right}" y="{fy}" font-size="22" text-anchor="end">'
        f"one square per task · generated {escape(generated_at)} from main @ {escape(commit)}</text>"
    )
    o.append("</svg>")
    return "\n".join(o) + "\n"


def main(argv=None):
    p = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    p.add_argument("-o", "--output", required=True, help="the SVG to write ('-' for stdout)")
    p.add_argument("--tasks", default=str(ROOT / "plan/tasks.yaml"))
    p.add_argument("--milestones", default=str(ROOT / "plan/REBUILD.md"))
    p.add_argument("--open-prs", help="JSON from `gh pr list --state open --json headRefName`")
    p.add_argument("--merges", help="merge-commit subjects, one per line, in place of git log")
    p.add_argument("--commit", help="the commit the banner reflects (default: git rev-parse --short HEAD)")
    p.add_argument("--generated-at", help="the footer's time (default: now, UTC)")
    p.add_argument(
        "--theme",
        choices=sorted(THEMES),
        default="light",
        help="the inks: light (dark ink, for a light page) or dark (pale ink, for a dark page); default light",
    )
    a = p.parse_args(argv)

    tasks = load_tasks(a.tasks)
    if a.merges:
        subjects = Path(a.merges).read_text(encoding="utf-8").splitlines()
    else:
        subjects = git("log", "--merges", "--format=%s").splitlines()
    in_review = open_pr_ids(a.open_prs) if a.open_prs else set()
    state = states(tasks, merged_ids(subjects), in_review)
    commit = a.commit or git("rev-parse", "--short", "HEAD").strip()
    generated_at = a.generated_at or datetime.datetime.now(datetime.timezone.utc).strftime("%Y-%m-%d %H:%M UTC")

    out = svg(tasks, state, milestone_names(a.milestones), commit, generated_at, a.theme)
    if a.output == "-":
        sys.stdout.write(out)
    else:
        Path(a.output).write_text(out, encoding="utf-8")
    done = sum(1 for s in state.values() if s == DONE)
    review = sum(1 for s in state.values() if s == REVIEW)
    print(f"progress_banner: {done}/{len(tasks)} done, {review} in review", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
