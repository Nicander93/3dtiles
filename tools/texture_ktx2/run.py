#!/usr/bin/env python3
"""CLI wrapper for KTX2 post-process (loads experiments desktop_server_py module)."""
from __future__ import annotations
import runpy
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
CANDIDATES = [
    ROOT / "tools" / "experiments" / "desktop_server_py" / "app" / "texture_ktx2.py",
    ROOT / "apps" / "desktop_server" / "app" / "texture_ktx2.py",  # one-release fallback
]
script = next((p for p in CANDIDATES if p.is_file()), None)
if script is None:
    print("texture_ktx2.py not found under tools/experiments/desktop_server_py or apps/desktop_server", file=sys.stderr)
    sys.exit(2)
# Ensure sibling imports under app/ resolve if any; module is mostly stdlib-only.
sys.path.insert(0, str(script.parents[1]))  # .../desktop_server_py or .../desktop_server
sys.path.insert(0, str(ROOT))
runpy.run_path(str(script), run_name="__main__")
