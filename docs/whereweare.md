Where we are (2026-07-30, night)

0.3.0 shipped this morning. Six PRs merged since, `main` at `019713e` plus
whatever #82 lands as.

| PR | What | State |
|---|---|---|
| #72 | Windows APIPA acquisition (contributor's; we made it green) | merged |
| #78 | Bookkeeping + the corpus document from #45 | merged |
| #79 | Status-code decoding, U3V pending-ack, ADR-0020 | merged |
| #80 | GA-06 — `pIsLocked` enforced on writes | merged |
| #81 | SR-05 — library-owned control-channel keepalive | merged |
| #82 | CI-12 — `viva-camctl` in the Python wheel | open, CI green |

## The through-line

Every defect this session came from #45's and #70's reporters, each found by
pulling on the previous one:

`Unknown(32774)` → the status table was wrong in three places → the GVCP retry
loop had been retrying `WRITE_PROTECT` and giving up on `BUSY` → and separately,
the *reason* for the error was that `pIsLocked` was never enforced, so we sent a
write the camera's own XML forbade.

SR-05 and CI-12 came out of the same two reports by a different route: the
reporter could not run the CLI we asked them for, and their session lost control
privilege while idle.

## Things worth not forgetting

- **CI-11 (P0, open) covers two workflows, not one.** `ci.yml` *and*
  `studio-ci.yml` both run fmt → clippy → test as sequential steps in one job,
  so the first failure hides the rest. On #72 that hid a streaming regression
  for a day: `cargo test` never ran once across eight contributor commits. The
  backlog entry names only `ci.yml`.
- **CI-02 (P0, open) is smaller than it reads.** Measured: 35 `cargo fmt` hunks
  (30 in `camera.rs`) and exactly **two** clippy findings, both pre-existing in
  `src/stream.rs` (`large_enum_variant`, `explicit_auto_deref`). Mechanical.
  Tests run locally in ~45 s:
  `cd crates/viva-pygenicam && maturin develop --uv && .venv/bin/python -m pytest tests -q`
  (31 tests since #82).
- **The fake camera could not have caught SR-05.** It stored CCP as an inert
  register. It now enforces the heartbeat behind `--enforce-heartbeat`
  (off by default — armed, a 3 s window makes every CCP-holding test sensitive
  to a loaded runner). Whenever a fake-camera test is meant to prove a
  behaviour, check it fails with the behaviour removed; SR-05's did
  (`ccp=0x00000000`).
- **maturin's `data` mechanism cannot be declared statically.** A missing data
  dir is a hard build failure and any non-scheme entry inside it is rejected, so
  no committed placeholder is possible. That is why CI-12 links the CLI into the
  extension module instead. Checked against maturin 1.13.1.
- **Our own review broke #72's CI.** Suggestions must be run through the gates
  first; we push the fix to the contributor's branch. Both now in CLAUDE.md.
- **crates.io needs a `User-Agent`** or `jq -r .crate.max_version` prints `null`.
- **TC-16's remaining half**: per-transport status types, deferred to 0.4.0.
  `0x800B` is the forcing case (GVCP `NO_MSG` vs GenCP `MSG_TIMEOUT`).

## Evidence standing

Status tables rest on Wireshark `packet-gvcp.c` + aravis (agreeing) plus one
hardware observation — **not** spec text we hold. SR-05, GA-06 and TC-15/16 are
covered by fake-camera and loopback tests only; no hardware has run any of them.
aravis corroborates SR-05 twice: its fake camera expires CCP
(`arvgvfakecamera.c:133`) and its client pings by reading CCP
(`arvgvdevice.c:470`), on a fixed 1 s period where we derive one.

CI-12's wheel path is verified end to end into a clean venv. The **sdist** path
is inferred, not run — the tarball carries `_camctl.py`, the crate source and
`[project.scripts]`, but no from-source install has been completed (local disk
ran out). Worth doing on a runner.

## Next

1. **CI-11 + CI-02 as one PR** — both CI-correctness, both small. Formatting
   diff in its own commit so it does not drown the workflow change.
2. **0.3.1** — scope was #72 + status table + SR-05, and CI-12 landed too.
   Gated on acquisition re-validation on #70's JAI. Fold the changelog *before*
   tagging. Six version touchpoints incl. the intra-workspace caret ranges.
3. **SR-06 + ST-16** (unsound `unsafe impl`s).
4. **DOC-14** — #57's Linux APIPA + firewalld guide into the networking
   cookbook, with credit. **And the book has no page on reporting a camera we
   cannot open** — `viva-camctl report` appeared nowhere in `book/src/` before
   #82. That is the highest-value page for the evidence pipeline and it does not
   exist.
5. Docs truth pass (DOC-01/02/03/09/11).

## Open with reporters

Nothing is owed: we are the last commenter on #70, #57 and #45.

- **#70** open deliberately — acquisition not yet re-validated on the JAI.
- **#45** open — connect is fixed and confirmed on their hardware; GA-06, the
  status decoding and now the wheel's CLI are all awaiting their retest.
- **#57** — Linux verification posted; decide whether to close.
- **#35** still owes a 0.3.0 retest.
- **#59** can be closed (REL-02 done — 0.3.0 tagged and verified).

## Environment

**The machine is out of disk**: 244 MB free of 460 GB. `target/` is 2 GB.
Rust and pip builds will fail in confusing ways until this is fixed.
