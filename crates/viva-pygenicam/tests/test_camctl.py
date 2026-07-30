"""The wheel ships the ``viva-camctl`` CLI.

We ask reporters to run ``viva-camctl report`` / ``viva-camctl xml`` when we
cannot open their camera, right after telling them to ``pip install
viva-genicam``. For 0.3.0 that instruction was wrong — the wheel had no such
command (#45). These tests fail if it goes missing again.
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

import pytest

from viva_genicam._camctl import main


def test_cli_is_wired_into_the_extension():
    """``--version`` exercises the whole path: shim, PyO3, clap."""
    assert main(["--version"]) == 0


def test_a_usage_error_reports_a_nonzero_code():
    # clap's own code for a bad invocation. The console script has to propagate
    # it, or `viva-camctl ... || echo failed` silently succeeds in a user's shell.
    assert main(["definitely-not-a-command"]) == 2


def test_the_commands_we_ask_reporters_for_are_present():
    completed = _run(["--help"])
    for command in ("report", "xml", "list"):
        assert command in completed.stdout, f"`viva-camctl {command}` is missing"


def test_console_script_is_installed():
    """The entry point, as `pip install` lays it down."""
    script = _console_script()
    if script is None:
        pytest.skip("no console script: an editable/`maturin develop` install")
    completed = subprocess.run(
        [str(script), "--version"], capture_output=True, text=True, timeout=60
    )
    assert completed.returncode == 0
    assert "viva-camctl" in completed.stdout


def _console_script() -> Path | None:
    name = "viva-camctl.exe" if sys.platform == "win32" else "viva-camctl"
    # Next to the running interpreter, not on PATH: that is where this
    # environment's scripts live, whether or not the venv is activated.
    candidate = Path(sys.executable).parent / name
    return candidate if candidate.exists() else None


def _run(args: list[str]) -> subprocess.CompletedProcess:
    """Run the CLI out-of-process so its stdout can be inspected."""
    return subprocess.run(
        [sys.executable, "-m", "viva_genicam._camctl", *args],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
