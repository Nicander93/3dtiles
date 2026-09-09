#!/usr/bin/env python3
"""CLI wrapper — prefer: python -m apps.desktop_server.app.texture_ktx2"""
from __future__ import annotations
import runpy, sys
from pathlib import Path
ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
runpy.run_module("apps.desktop_server.app.texture_ktx2", run_name="__main__")
