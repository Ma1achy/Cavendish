# Kickoff — running the rebuild autonomously

## Where it runs
- **On the Mac (main loop).** The GPU, Metal and frame-rate checks need real hardware, and cloud
  sessions only have a software GPU. Run Claude Code in the repo in auto mode:
  `claude --permission-mode auto` (or bypass permissions, if you accept that on your machine).
- **In cloud sessions (parallel side work).** GPU-free pieces — shapes library, scene catalogue, MCP
  tools, docs, research notes. Start each with `claude --cloud "<task>"` and pick auto mode (or bypass
  permissions — the VM is isolated) in the session's mode menu so it never stops for approval.

## The prompt for the main loop

> Read CLAUDE.md, plan/REBUILD.md, plan/VALIDATION-AND-RESEARCH.md and plan/PROGRESS.md. Work on
> `main` (this repository now starts from the plan alone; the old code is at tags `v1-final` and
> `v2-prototype`), one branch per slice merged back when it is done. Then, for each slice
> in order: spawn `researcher` agents for the slice's open method questions (in cloud sessions when
> they need no GPU); spawn the `builder` agent for the slice; when it reports done, spawn a fresh
> `reviewer`; loop builder ↔ reviewer until no important findings remain; tag the slice, update
> PROGRESS.md, push, and continue with the next slice. Hand independent, GPU-free work to cloud
> sessions with `claude --cloud`. Stop only for: money, publishing, deleting history, or a decision that
> cannot be undone — then write the question into PROGRESS.md and carry on with work that does not
> depend on it.

## Keeping it going
Use `/loop` with that prompt, or a scheduled task on the Mac, so the loop resumes after a context
reset or a restart. PROGRESS.md is the memory between runs.
