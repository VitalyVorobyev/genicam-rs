---
name: release
description: Cut a release: version bump across all touchpoints, changelog, PR, tags, publish verification
---

# Release

Follow CLAUDE.md "Version Bumps" — one version shared by all workspace
crates plus the Python package. `studio/` crates are unpublished and not
versioned by this process.

## 1. Bump — all five touchpoints together

1. `Cargo.toml` — `[workspace.package] version`
2. `crates/viva-pygenicam/Cargo.toml` — `[package] version` (does not
   inherit from the workspace)
3. `crates/viva-pygenicam/pyproject.toml` — `[project] version`
4. `crates/viva-pygenicam/python/viva_genicam/__init__.py` — `__version__`
5. `CHANGELOG.md` — rename `[Unreleased]` → `## [X.Y.Z] - YYYY-MM-DD`,
   add a fresh empty `[Unreleased]` section, add the footer link line

A missed file breaks the wheel build or publishes wrong metadata.

## 2. Verify locally

- Refresh `Cargo.lock`: `cargo metadata --format-version 1 > /dev/null`
- Run the **quality-gate** skill — all gates must pass.

## 3. Release PR

Feature branch, commit, PR titled `Release X.Y.Z`. Never push to main.

## 4. After merge: tag

```bash
git tag vX.Y.Z <merge-commit>
git tag py-vX.Y.Z <merge-commit>
git push origin vX.Y.Z py-vX.Y.Z
```

## 5. Watch and verify publication

Watch the three workflows: **Publish Rust crates**, **Release**,
**Python wheels** (`gh run list`/`gh run watch`). Then verify:

- crates.io sparse index shows the new version:
  `curl -A viva-release-check https://index.crates.io/vi/va/viva-genicam | tail -1`
- PyPI JSON shows the version and license `MIT AND LGPL-2.1-or-later`:
  `curl -s https://pypi.org/pypi/viva-genicam/json | jq '.info.version, .info.license'`
- GitHub release `vX.Y.Z` exists and has the binary assets attached.

Report a checklist of what was verified and anything still pending.
