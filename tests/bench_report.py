#!/usr/bin/env python3
"""Turn benchmark JSON into a job summary and flag regressions vs a baseline.

Usage: bench_report.py <baseline.json> <current.json>
Exits 0 even on regression (runner noise); the regression is reported as a
warning in the step summary and the annotation.
"""
import json
import math
import os
import sys

WORSE_THRESHOLD = 1.30


def load(path):
    with open(path) as fh:
        return json.load(fh)["programs"]


def geo(values):
    return math.exp(sum(math.log(v) for v in values) / len(values))


def ratios(rows):
    out = {}
    for r in rows:
        native = r["native_run"]["wall"]
        ours = r["jit_run"]["wall"]
        e2e_native = r["native_compile"]["wall"] + native
        e2e_ours = r["front_end"]["wall"] + ours
        out[r["name"]] = {
            "runtime": ours / native if native else float("nan"),
            "e2e": e2e_ours / e2e_native if e2e_native else float("nan"),
            "rss": r["jit_run"]["rss_kb"] / r["native_run"]["rss_kb"]
            if r["native_run"]["rss_kb"]
            else float("nan"),
            "ms": ours * 1000,
        }
    return out


def main():
    baseline_path, current_path = sys.argv[1], sys.argv[2]
    base = ratios(load(baseline_path))
    cur = ratios(load(current_path))

    lines = [
        "## Benchmark",
        "",
        "| program | runtime (ours) | vs native | vs baseline | RSS vs native |",
        "|---|---:|---:|---:|---:|",
    ]
    regressions = []
    for name, c in cur.items():
        b = base.get(name)
        delta = c["runtime"] / b["runtime"] if b and b["runtime"] else float("nan")
        flag = ""
        if delta == delta and delta > WORSE_THRESHOLD:
            flag = " :warning:"
            regressions.append((name, delta))
        lines.append(
            f"| {name} | {c['ms']:.1f} ms | {c['runtime']:.1f}x | {delta:.2f}x{flag} | {c['rss']:.2f}x |"
        )

    cur_runtime = geo([c["runtime"] for c in cur.values()])
    cur_e2e = geo([c["e2e"] for c in cur.values()])
    base_runtime = geo([b["runtime"] for b in base.values()])
    lines += [
        "",
        f"- geometric mean runtime slowdown: **{cur_runtime:.1f}x** (baseline {base_runtime:.1f}x)",
        f"- geometric mean end-to-end (compile + run): **{cur_e2e:.2f}x** of rustc",
        f"- programs won end-to-end: **{sum(1 for c in cur.values() if c['e2e'] < 1)}** / {len(cur)}",
        "",
    ]

    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    text = "\n".join(lines) + "\n"
    if summary:
        with open(summary, "a") as fh:
            fh.write(text)
    print(text)

    for name, delta in regressions:
        print(f"::warning::{name} is {delta:.2f}x slower than the committed baseline")


if __name__ == "__main__":
    main()
