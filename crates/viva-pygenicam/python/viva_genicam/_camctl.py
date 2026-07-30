"""Entry point for the ``viva-camctl`` console script.

``pip install viva-genicam`` installs a ``viva-camctl`` command backed by this
module. The CLI itself is Rust, linked into the extension module, so this is only
argument hand-off and exit-code propagation.
"""

from __future__ import annotations

import sys
from typing import Optional, Sequence


def main(argv: Optional[Sequence[str]] = None) -> int:
    """Run ``viva-camctl`` and return its exit code.

    ``argv`` excludes the program name, matching ``sys.argv[1:]``.
    """
    from ._native import camctl_main

    if argv is None:
        argv = sys.argv[1:]
    return camctl_main(list(argv))


if __name__ == "__main__":  # pragma: no cover
    raise SystemExit(main())
