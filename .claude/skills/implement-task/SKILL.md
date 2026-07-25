---
name: implement-task
description: Implement a backlog task from docs/backlog.md end-to-end (plan → subagent implement → review → PR)
---

# Implement Backlog Task

Argument: a backlog ID (e.g. `SR-01`). Drives the task from plan to PR.

## 1. Understand

- Read the task row in `docs/backlog.md` (priority, size, notes, epic context).
- Read `docs/design.md` — especially the design tenets and any invariants
  the task touches.
- Read any ADR the task touches (`docs/adrs/README.md` index).
- Use codegraph (`codegraph_context`, `codegraph_trace`) to scope the
  affected code — do not grep-explore what the index already knows.

## 2. Plan

Draft a short plan and show it to the user:
- **Affected files** — exact paths, one line each.
- **Test plan** — which tests prove it works; every new pure function gets
  unit tests (happy path + at least one edge case).
- **Invariants at risk** — from docs/design.md tenets; if the change
  conflicts with a tenet, propose an ADR instead of silently deviating.

Stop and ask if there are open questions the codebase cannot answer.

## 3. Implement

Delegate to a subagent with a tight, file-precise prompt (plan, exact
files, test names) to keep the main context lean. Constraints:

- No out-of-scope refactors; no one-use abstractions.
- No `unwrap()`/`expect()` outside tests (docs/design.md tenets).
- Follow existing module/test conventions in the touched crates.

## 4. Verify

Run the **quality-gate** skill. Fix failures (re-delegating narrowly if
needed) until all gates pass.

## 5. Ship

- **NEVER push to main.** Create a feature branch, commit using the repo
  commit-footer conventions, and open a PR whose title/body reference the
  backlog ID.
- Update `CHANGELOG.md` `[Unreleased]` when the change is user-visible.

## 6. After merge

Update the task's row in `docs/backlog.md` to status `done`, and include
that change in the next PR (or a small docs PR) — never as a direct push.
