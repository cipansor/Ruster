#!/usr/bin/env python3
"""Guard the benchmark arithmetic and the report labels.

`bin/rust <file>` is the full end-to-end invocation: it already lexes, parses,
compiles, and runs the program. The separately measured `--compile-only` time
is for the compile-phase table only, so both benchmark reporters must use the
full invocation as the end-to-end Ruster time.

The fixtures give the JIT and the interpreter *different* wall times, so a
renderer that silently picks the wrong engine cannot pass. They also cover the
legacy-report case: an old report has no `native_parity` field, so it must not
claim that native stdout was verified.
"""
import importlib.util
import os
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def fixture(native_parity="present"):
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
            "codes": [0],
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
        "code_match": True,
    }
    if native_parity == "present":
        entry["native_parity"] = True
    elif native_parity == "false":
        entry["native_parity"] = False
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
    checks = [
        ("| x | 1.2 ms | 0.1 ms |", "end-to-end uses the JIT run"),
        (
            "| x | 0.2 ms | 0.2 ms | 100% | 0.1 MB | 0.1 ms |",
            "runtime table uses the JIT run",
        ),
        ("| x | 0.9 ms | 0.1 ms | 9.00x |", "JIT-effect table shows both engines"),
    ]
    for needle, what in checks:
        if needle not in md:
            print(f"FAIL {what}: missing {needle!r}")
            failed += 1
    e2e = md.split("## End-to-end", 1)[-1]
    if "0.9 ms" in e2e:
        print("FAIL end-to-end section contains the interpreter time")
        failed += 1

    # 4. A legacy report (no `native_parity`) must not claim verification.
    legacy = bench.render_markdown(fixture(native_parity="missing"))
    if "not recorded in this report (legacy format)" not in legacy:
        print("FAIL legacy report claims native parity was verified")
        failed += 1
    if "produced identical stdout and exit code for every benchmark program" in legacy:
        print("FAIL legacy report claims native parity was verified")
        failed += 1

    # 5. An explicit native mismatch must be named.
    bad = bench.render_markdown(fixture(native_parity="false"))
    if "Parity FAILED for: x." not in bad:
        print("FAIL native parity failure is not reported")
        failed += 1

    if failed == 0:
        print("ok   benchmark arithmetic, engine choice, and parity labels")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
