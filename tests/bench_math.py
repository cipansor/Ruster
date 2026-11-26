#!/usr/bin/env python3
"""Guard the benchmark arithmetic.

`bin/rust <file>` is the full end-to-end invocation: it already lexes, parses,
compiles, and runs the program. The separately measured `--compile-only` time
is for the compile-phase table only. Both benchmark reporters must therefore
use the full invocation as the end-to-end Ruster time; adding `front_end` again
would double-count it.
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


def main():
    failed = 0
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

    bench = load("ruster_bench", os.path.join(ROOT, "bench", "bench.py"))
    res = {
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
        "programs": [
            {
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
                    "wall": 0.0001,
                    "cpu": 0.0001,
                    "rss_kb": 100,
                    "cpu_pct": 100.0,
                    "runs": 1,
                    "codes": [0],
                },
                "slowdown": 0.5,
                "interp_slowdown": 0.5,
                "jit_speedup": 1.0,
                "code_match": True,
            }
        ],
    }
    md = bench.render_markdown(res)
    if "| x | 1.2 ms | 0.1 ms |" not in md:
        print("FAIL bench.py end-to-end row double-counts the front-end")
        print(md)
        failed += 1

    if failed == 0:
        print("ok   benchmark end-to-end uses one full invocation (no double count)")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
