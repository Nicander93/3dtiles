#!/usr/bin/env python3
"""
Production acceptance pipeline orchestrator (plan §8 / §11).

Stages:
  env → (optional) download/prepare HK → inventory → convert → validate A/B
  → optional Cesium compare → rebuild → validate A/B → metrics/gates → result.md

Data stays under GEOFORGE_ACCEPTANCE_ROOT or /workspace/data/acceptance (never git).
Results under acceptance-results/<run-id>/ (gitignored artifacts OK; reports committed separately).
"""
from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from acceptance_paths import acceptance_root, results_root  # noqa: E402
from capture_environment import capture  # noqa: E402
from inventory_source import inventory  # noqa: E402


def repo_root() -> Path:
    return Path(__file__).resolve().parents[2]


def find_processor(repo: Path) -> Path:
    for c in [
        os.environ.get("GEOFORGE_PROCESSOR"),
        str(repo / "apps/desktop/src-tauri/binaries/processor"),
        str(repo / "target/release/processor"),
        str(repo / "target/debug/processor"),
        shutil.which("processor"),
    ]:
        if c and Path(c).is_file():
            return Path(c)
    raise FileNotFoundError("processor binary not found — build or stage sidecars first")


def run_cmd(cmd: list[str], log_path: Path, env: dict | None = None) -> int:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    print(f"[cmd] {' '.join(cmd)}")
    with log_path.open("w") as log:
        log.write(f"$ {' '.join(cmd)}\n\n")
        log.flush()
        p = subprocess.run(
            cmd,
            stdout=log,
            stderr=subprocess.STDOUT,
            env=env or os.environ.copy(),
        )
        log.write(f"\n\n# exit={p.returncode}\n")
    return p.returncode


def load_thresholds(repo: Path) -> dict:
    p = repo / "tests/acceptance/thresholds.json"
    if p.is_file():
        return json.loads(p.read_text())
    return {}


def gate_eval(name: str, ok: bool, detail: str, gates: list) -> None:
    gates.append({"gate": name, "pass": bool(ok), "detail": detail})
    status = "PASS" if ok else "FAIL"
    print(f"[gate] {status}  {name}: {detail}")


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--run-id", default=None)
    ap.add_argument("--staged", type=Path, help="Already-prepared OSGB root (metadata.xml+Data)")
    ap.add_argument("--width", type=int, default=4)
    ap.add_argument("--height", type=int, default=4)
    ap.add_argument("--region", default="auto")
    ap.add_argument("--skip-download", action="store_true")
    ap.add_argument("--skip-cesium-ref", action="store_true")
    ap.add_argument("--skip-layer-b", action="store_true")
    ap.add_argument("--skip-rebuild", action="store_true")
    ap.add_argument("--texture", default="keep")
    args = ap.parse_args()

    repo = repo_root()
    scripts = Path(__file__).resolve().parent
    start = datetime.now(timezone.utc)
    run_id = args.run_id or start.strftime("hk_%Y%m%dT%H%M%SZ")
    out = results_root(repo) / run_id
    out.mkdir(parents=True, exist_ok=True)
    gates: list[dict] = []
    thr = load_thresholds(repo)

    # environment at start
    env_doc = capture(repo, start_iso=start.isoformat())
    (out / "environment.json").write_text(json.dumps(env_doc, indent=2) + "\n")

    processor = find_processor(repo)
    data_root = acceptance_root()

    # --- download + prepare if needed ---
    staged = args.staged
    if staged is None:
        sel_glob = list((data_root / "hk_pland").glob(f"selection_osgb_{args.width}x{args.height}.json"))
        if not args.skip_download or not sel_glob:
            cmd = [
                sys.executable,
                str(scripts / "download_hk_pland.py"),
                "--width", str(args.width),
                "--height", str(args.height),
                "--region", args.region,
                "--format", "OSGB",
            ]
            if args.skip_download:
                cmd.append("--probe-only")
            rc = run_cmd(cmd, out / "download.log")
            gate_eval("download_hk_pland", rc == 0, f"exit={rc}", gates)
            if rc != 0:
                _write_result(out, gates, start, "NOT READY — download/probe failed")
                return rc
            if args.skip_download:
                # probe-only path for CI without data
                _write_result(out, gates, start, "PROBE ONLY")
                return 0
        sel = sorted((data_root / "hk_pland").glob(f"selection_osgb_{args.width}x{args.height}.json"))[-1]
        shutil.copy2(sel, out / "dataset-selection.json")
        rc = run_cmd(
            [sys.executable, str(scripts / "prepare_hk_pland.py"), "--selection", str(sel)],
            out / "prepare.log",
        )
        gate_eval("prepare_stage", rc == 0, f"exit={rc}", gates)
        if rc != 0:
            _write_result(out, gates, start, "NOT READY — staging failed")
            return rc
        man = json.loads((data_root / "hk_pland" / f"dataset-manifest_{sel.stem.replace('selection_','')}.json").read_text())
        staged = Path(man["stagedRoot"])
        shutil.copy2(staged / "dataset-manifest.json", out / "dataset-manifest.json")
    else:
        staged = Path(staged)
        if (staged / "dataset-manifest.json").is_file():
            shutil.copy2(staged / "dataset-manifest.json", out / "dataset-manifest.json")

    # inventory
    inv = inventory(staged)
    (out / "source-inventory.json").write_text(json.dumps(inv, indent=2) + "\n")
    gate_eval(
        "source_inventory",
        inv["gridCount"] > 0 and inv["osgbFileCount"] > 0,
        f"grids={inv['gridCount']} osgb={inv['osgbFileCount']} bytes={inv['inputBytes']}",
        gates,
    )

    # convert baseline (no rebuild)
    baseline_out = Path(os.environ.get("GEOFORGE_ACCEPTANCE_OUT", str(data_root / "outputs"))) / run_id / "baseline"
    if baseline_out.exists():
        shutil.rmtree(baseline_out)
    baseline_out.mkdir(parents=True, exist_ok=True)
    rc = run_cmd(
        [
            str(processor),
            "convert-osgb",
            "--input", str(staged),
            "--output", str(baseline_out),
            "--texture", args.texture,
            "--task-id", f"{run_id}-baseline",
        ],
        out / "converter.log",
    )
    gate_eval("convert_baseline", rc == 0 and (baseline_out / "tileset.json").is_file(), f"exit={rc}", gates)
    if rc != 0:
        _write_result(out, gates, start, "NOT READY — baseline converter failed on real public data")
        return rc

    # Layer A on baseline
    rc = run_cmd(
        [str(processor), "validate-tileset", "--path", str(baseline_out)],
        out / "validate_baseline_layer_a.log",
    )
    val_a = baseline_out / "validation_internal.json"
    if val_a.is_file():
        shutil.copy2(val_a, out / "validation_internal_baseline.json")
        va = json.loads(val_a.read_text())
        ok = va.get("ok") is True or va.get("passed") is True or va.get("errorCount", 1) == 0
        gate_eval("layer_a_baseline", ok and rc == 0, f"exit={rc} report={val_a.name}", gates)
    else:
        gate_eval("layer_a_baseline", False, "validation_internal.json missing", gates)

    # Layer B optional
    if not args.skip_layer_b:
        report_b = out / "validator-report-baseline.json"
        rc = run_cmd(
            [
                str(repo / "scripts/acceptance/run_layer_b_validator.sh"),
                str(baseline_out / "tileset.json"),
                str(report_b),
            ],
            out / "validate_baseline_layer_b.log",
        )
        num_err = None
        if report_b.is_file():
            try:
                num_err = json.loads(report_b.read_text()).get("numErrors")
            except Exception:  # noqa: BLE001
                num_err = "parse_error"
        gate_eval("layer_b_baseline", rc == 0 and num_err == 0, f"exit={rc} numErrors={num_err}", gates)

    # rebuild
    rebuild_out = None
    if not args.skip_rebuild:
        rebuild_out = Path(os.environ.get("GEOFORGE_ACCEPTANCE_OUT", str(data_root / "outputs"))) / run_id / "rebuild"
        if rebuild_out.exists():
            shutil.rmtree(rebuild_out)
        rebuild_out.mkdir(parents=True, exist_ok=True)
        # process-tileset from baseline with rebuild
        rc = run_cmd(
            [
                str(processor),
                "process-tileset",
                "--input", str(baseline_out),
                "--output", str(rebuild_out),
                "--texture", args.texture,
                "--rebuild-top",
                "--levels", "0",
                "--task-id", f"{run_id}-rebuild",
            ],
            out / "rebuild.log",
        )
        gate_eval("top_rebuild", rc == 0 and (rebuild_out / "tileset.json").is_file(), f"exit={rc}", gates)
        metrics = list(rebuild_out.rglob("rebuild_metrics.json"))
        if metrics:
            shutil.copy2(metrics[0], out / "rebuild_metrics.json")
        # Layer A rebuild
        rc = run_cmd(
            [str(processor), "validate-tileset", "--path", str(rebuild_out)],
            out / "validate_rebuild_layer_a.log",
        )
        val_r = rebuild_out / "validation_internal.json"
        if val_r.is_file():
            shutil.copy2(val_r, out / "validation_internal.json")
            vr = json.loads(val_r.read_text())
            ok = vr.get("ok") is True or vr.get("passed") is True or vr.get("errorCount", 1) == 0
            # detect GRID_SPATIAL_MISMATCH style codes if present
            blob = json.dumps(vr)
            if "GRID_SPATIAL_MISMATCH" in blob:
                gate_eval("grid_spatial", False, "GRID_SPATIAL_MISMATCH present — NOT READY", gates)
            gate_eval("layer_a_rebuild", ok and rc == 0, f"exit={rc}", gates)
        if not args.skip_layer_b:
            report_b = out / "validator-report.json"
            rc = run_cmd(
                [
                    str(repo / "scripts/acceptance/run_layer_b_validator.sh"),
                    str(rebuild_out / "tileset.json"),
                    str(report_b),
                ],
                out / "validate_rebuild_layer_b.log",
            )
            num_err = None
            if report_b.is_file():
                try:
                    num_err = json.loads(report_b.read_text()).get("numErrors")
                except Exception:  # noqa: BLE001
                    num_err = "parse_error"
            gate_eval("layer_b_rebuild", rc == 0 and num_err == 0, f"exit={rc} numErrors={num_err}", gates)

    # subtree preservation heuristic from metrics / inventory
    if (out / "rebuild_metrics.json").is_file():
        m = json.loads((out / "rebuild_metrics.json").read_text())
        (out / "subtree_preservation.json").write_text(
            json.dumps({"fromMetrics": m, "sourceGridCount": inv["gridCount"]}, indent=2) + "\n"
        )

    end = datetime.now(timezone.utc)
    env_doc = capture(repo, start_iso=start.isoformat(), end_iso=end.isoformat())
    (out / "environment.json").write_text(json.dumps(env_doc, indent=2) + "\n")

    failed = [g for g in gates if not g["pass"]]
    status = "PASS" if not failed else "NOT READY"
    _write_result(out, gates, start, status, staged=str(staged), baseline=str(baseline_out), rebuild=str(rebuild_out) if rebuild_out else None)
    return 0 if not failed else 1


def _write_result(out: Path, gates: list, start: datetime, status: str, **paths) -> None:
    end = datetime.now(timezone.utc)
    doc = {
        "status": status,
        "startedAt": start.isoformat(),
        "finishedAt": end.isoformat(),
        "gates": gates,
        "paths": paths,
    }
    (out / "gates.json").write_text(json.dumps(doc, indent=2) + "\n")
    lines = [
        f"# Acceptance result — `{out.name}`",
        "",
        f"- Status: **{status}**",
        f"- Started: {start.isoformat()}",
        f"- Finished: {end.isoformat()}",
        "",
        "## Gates",
        "",
        "| Gate | Pass | Detail |",
        "|------|------|--------|",
    ]
    for g in gates:
        lines.append(f"| `{g['gate']}` | {'PASS' if g['pass'] else 'FAIL'} | {g['detail']} |")
    lines += ["", "## Paths", ""]
    for k, v in paths.items():
        lines.append(f"- `{k}`: `{v}`")
    lines.append("")
    (out / "result.md").write_text("\n".join(lines))
    print(f"[result] {status} -> {out / 'result.md'}")


if __name__ == "__main__":
    raise SystemExit(main())
