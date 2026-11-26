#!/usr/bin/env python3
"""Guard the benchmark arithmetic and the report labels.

`bin/rust <file>` is the full end-to-end invocation: it already lexes, parses,
compiles, and runs the program. The separately measured `--compile-only` time
is for the compile-phase table only, so both benchmark reporters must use the
full invocation as the end-to-end Ruster time.

The fixtures give the JIT and the interpreter *different* wall times, so a
renderer that silently picks the wrong engine cannot pass. They also cover:

  * a legacy report (no `native_parity` / `timed_ok`) must not claim success;
  * a captured parity failure must be named;
  * a timed-run failure must be named even when the captured comparison passes,
    because the timing for that program is not valid.
"""
import importlib.util
import json
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def fixture(native_parity="present", timed_ok=True):
    """One synthetic program; JIT = 0.1 ms, interpreter = 0.9 ms."""
    entry = {
        "name": "x",
        "description": "",
        "source_bytes": 1,
        "native_compile": {"wall": 0.001, "cpu": 0.001, "rss_kb": 100},
        "native_size_bytes": 1,
        "front_end": {
            "wall": 0.0005,
            "cpu": 0.0005,
            "rss_kb": 100,
            "cpu_pct": 100.0,
            "runs": 1,
            "codes": [0],
        },
        "native_run": {
            "wall": 0.0002,
            "cpu": 0.0002,
            "rss_kb": 100,
            "cpu_pct": 100.0,
            "runs": 1,
            "codes": [0],
        },
        "jit_run": {
            "wall": 0.0001,
            "cpu": 0.0001,
            "rss_kb": 100,
            "cpu_pct": 100.0,
            "runs": 1,
            "codes": [0] if timed_ok else [1],
        },
        "interp_run": {
            "wall": 0.0009,
            "cpu": 0.0009,
            "rss_kb": 100,
            "cpu_pct": 100.0,
            "runs": 1,
            "codes": [0],
        },
        "slowdown": 0.5,
        "interp_slowdown": 4.5,
        "jit_speedup": 9.0,
        "timed_ok": timed_ok,
    }
    if native_parity == "present":
        entry["native_parity"] = True
    elif native_parity == "false":
        entry["native_parity"] = False
    elif native_parity == "missing":
        entry.pop("timed_ok", None)
    entry["code_match"] = entry.get("timed_ok", True) and entry.get(
        "native_parity", True
    )
    return {
        "meta": {
            "date": "x",
            "host": "x",
            "cpu_count": 1,
            "rustc": "x",
            "interpreter_size_bytes": 1,
            "interpreter_build": None,
            "runs": 1,
            "compile_runs": 1,
        },
        "programs": [entry],
    }


def report_summary(data):
    with tempfile.NamedTemporaryFile("w", suffix=".json", delete=False) as f:
        json.dump(data, f)
        path = f.name
    try:
        return subprocess.run(
            [sys.executable, os.path.join(ROOT, "tests", "bench_report.py"), path, path],
            capture_output=True,
            text=True,
        )
    finally:
        os.unlink(path)


def expect(text, needles, label, failed):
    for needle in needles:
        if needle not in text:
            print(f"FAIL {label}: missing {needle!r}")
            failed += 1
    return failed


def main():
    failed = 0

    # 1. bench_report: end-to-end is the full run, not front-end + full run.
    row = {
        "name": "x",
        "native_compile": {"wall": 0.001},
        "native_run": {"wall": 0.0002, "rss_kb": 100},
        "front_end": {"wall": 0.0005},
        "jit_run": {"wall": 0.0001, "rss_kb": 100},
    }
    report = load("ruster_bench_report", os.path.join(ROOT, "tests", "bench_report.py"))
    got = report.ratios([row])["x"]
    want = 0.0001 / 0.0012
    if abs(got["e2e"] - want) > 1e-12:
        print(
            f"FAIL bench_report e2e = {got['e2e']:.6f}, want {want:.6f} "
            "(the front-end must not be added twice)"
        )
        failed += 1
    if abs(got["runtime"] - 0.5) > 1e-12:
        print(f"FAIL bench_report runtime = {got['runtime']:.6f}, want 0.500000")
        failed += 1

    # 2. Importing bench.py must not be blocked by MINIRUST_NO_JIT; the check
    #    only applies when the harness actually runs the engines.
    os.environ["MINIRUST_NO_JIT"] = "1"
    try:
        bench = load("ruster_bench", os.path.join(ROOT, "bench", "bench.py"))
    finally:
        os.environ.pop("MINIRUST_NO_JIT", None)

    # 3. The JIT (0.1 ms) must be selected, never the interpreter (0.9 ms).
    md = bench.render_markdown(fixture())
    failed = expect(
        md,
        [
            "| x | 1.2 ms | 0.1 ms |",
            "| x | 0.2 ms | 0.2 ms | 100% | 0.1 MB | 0.1 ms |",
            "| x | 0.9 ms | 0.1 ms | 9.00x |",
            "Timed runs: every engine exited 0 in all measured runs.",
            "Parity: native `rustc`, Ruster (JIT), and Ruster `--no-jit` "
            "produced identical stdout and exit code for every benchmark program.",
        ],
        "clean report",
        failed,
    )
    e2e = md.split("## End-to-end", 1)[-1]
    if "0.9 ms" in e2e:
        print("FAIL end-to-end section contains the interpreter time")
        failed += 1

    # 4. A legacy report (no native_parity / timed_ok) must not claim success.
    legacy = bench.render_markdown(fixture(native_parity="missing"))
    failed = expect(
        legacy,
        [
            "Timed runs: not recorded in this report (legacy format)",
            "Parity: not recorded in this report (legacy format)",
        ],
        "legacy report",
        failed,
    )

    # 5. Captured parity failure: name it; timed runs are still fine.
    parity_bad = bench.render_markdown(fixture(native_parity="false"))
    failed = expect(
        parity_bad,
        [
            "Parity FAILED for: x.",
            "Timed runs: every engine exited 0 in all measured runs.",
        ],
        "parity failure",
        failed,
    )

    # 6. Timed-run failure with a passing captured comparison: name the timed
    #    failure, but keep the captured parity verified.
    timed_bad = bench.render_markdown(fixture(timed_ok=False))
    failed = expect(
        timed_bad,
        [
            "Timed runs FAILED for: x (nonzero exit in a measured run; those timings are invalid).",
            "Parity: native `rustc`, Ruster (JIT), and Ruster `--no-jit` "
            "produced identical stdout and exit code for every benchmark program.",
        ],
        "timed failure",
        failed,
    )

    # 7. The CI summary must distinguish the two checks too.
    out = report_summary(fixture(timed_ok=False))
    failed = expect(
        out.stdout,
        [
            "- timed runs (all engines exited 0): **FAILED (x)**",
            "- captured parity (native / JIT / no-JIT stdout+exit): **verified**",
            "::error::benchmark timed runs failed for: x",
        ],
        "bench_report timed failure",
        failed,
    )
    out = report_summary(fixture(native_parity="false"))
    failed = expect(
        out.stdout,
        [
            "- timed runs (all engines exited 0): **verified**",
            "- captured parity (native / JIT / no-JIT stdout+exit): **FAILED (x)**",
            "::error::benchmark captured stdout/exit parity failed for: x",
        ],
        "bench_report parity failure",
        failed,
    )

    if failed == 0:
        print("ok   benchmark arithmetic, engine choice, and both parity checks")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
