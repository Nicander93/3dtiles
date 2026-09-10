#!/usr/bin/env python3
"""Shared acceptance data-root resolution (data stays OUTSIDE git)."""
from __future__ import annotations

import os
from pathlib import Path


def acceptance_root() -> Path:
    """Prefer GEOFORGE_ACCEPTANCE_ROOT, else /workspace/data/acceptance."""
    env = os.environ.get("GEOFORGE_ACCEPTANCE_ROOT")
    if env:
        return Path(env).expanduser().resolve()
    # Prefer shared workspace data dir when present.
    ws = Path("/workspace/data/acceptance")
    if ws.parent.is_dir():
        ws.mkdir(parents=True, exist_ok=True)
        return ws.resolve()
    # Fallback: sibling of repo under user home /tmp — never inside git worktree by default.
    fallback = Path.home() / "geoforge-acceptance"
    fallback.mkdir(parents=True, exist_ok=True)
    return fallback.resolve()


def testdata_cache(repo: Path) -> Path:
    """Optional in-repo ignore path for tiny indexes only — not for multi-GB ZIPs."""
    p = repo / ".geoforge-testdata"
    p.mkdir(parents=True, exist_ok=True)
    return p


def results_root(repo: Path) -> Path:
    p = repo / "acceptance-results"
    p.mkdir(parents=True, exist_ok=True)
    return p
