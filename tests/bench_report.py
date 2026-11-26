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


def fmt_x(value, digits):
    return "n/a" if value != value else f"{value:.{digits}f}x"


def ratios(rows):
    out = {}
    for r in rows:
        native = r["native_run"]["wall"]
        ours = r["jit_run"]["wall"]
        e2e_native = r["native_compile"]["wall"] + native
        # `jit_run` is the full `bin/rust <file>` invocation, which already
        # includes the front-end; adding `front_end` here would count it twice.
        e2e_ours = ours
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
    cur_rows = load(current_path)
    cur = ratios(cur_rows)
    timed_bad = {r["name"] for r in cur_rows if r.get("timed_ok") is False}

    lines = [
        "## Benchmark",
        "",
        "| program | runtime (ours) | vs native | vs baseline | RSS vs native |",
        "|---|---:|---:|---:|---:|",
    ]
    regressions = []
    for name, c in cur.items():
        if name in timed_bad:
            lines.append(f"| {name} :x: | — | — | — | — |")
            continue
        b = base.get(name)
        delta = c["runtime"] / b["runtime"] if b and b["runtime"] else float("nan")
        flag = ""
        if delta == delta and delta > WORSE_THRESHOLD:
            flag = " :warning:"
            regressions.append((name, delta))
        lines.append(
            f"| {name} | {c['ms']:.1f} ms | {c['runtime']:.1f}x | {delta:.2f}x{flag} | {c['rss']:.2f}x |"
        )

    valid = [name for name in cur if name in base and name not in timed_bad]
    cur_runtime = geo([cur[n]["runtime"] for n in valid]) if valid else float("nan")
    cur_e2e = geo([cur[n]["e2e"] for n in valid]) if valid else float("nan")
    base_runtime = geo([base[n]["runtime"] for n in valid]) if valid else float("nan")
    wins = sum(1 for n in valid if cur[n]["e2e"] < 1)
    vals = [r.get("native_parity") for r in cur_rows]
    parity_failed = [r["name"] for r in cur_rows if r.get("native_parity") is False]
    if all(v is True for v in vals):
        parity = "verified"
    elif parity_failed:
        parity = f"FAILED ({', '.join(parity_failed)})"
    else:
        parity = "not recorded (legacy report)"
    timed_vals = [r.get("timed_ok") for r in cur_rows]
    timed_failed = [r["name"] for r in cur_rows if r.get("timed_ok") is False]
    if all(v is True for v in timed_vals):
        timed = "verified"
    elif timed_failed:
        timed = f"FAILED ({', '.join(timed_failed)})"
    else:
        timed = "not recorded (legacy report)"
    lines += [
        "",
        f"- geometric mean runtime slowdown: **{fmt_x(cur_runtime, 1)}** (baseline {fmt_x(base_runtime, 1)})",
        f"- geometric mean end-to-end (compile + run): **{fmt_x(cur_e2e, 2)}** of rustc",
        f"- programs won end-to-end: **{wins}** / {len(valid)}",
        f"- timed runs (all engines exited 0): **{timed}**",
        f"- captured parity (native / JIT / no-JIT stdout+exit): **{parity}**",
    ]
    if timed_bad:
        lines.append(
            f"- :x: timed run failed for: **{', '.join(sorted(timed_bad))}**; "
            "those timings are excluded from the geometric means and win count."
        )
    lines.append("")

    summary = os.environ.get("GITHUB_STEP_SUMMARY")
    text = "\n".join(lines) + "\n"
    if summary:
        with open(summary, "a") as fh:
            fh.write(text)
    print(text)

    for name, delta in regressions:
        print(f"::warning::{name} is {delta:.2f}x slower than the committed baseline")

    if timed_failed:
        print(f"::error::benchmark timed runs failed for: {', '.join(timed_failed)}")
    if parity_failed:
        print(
            "::error::benchmark captured stdout/exit parity failed for: "
            + ", ".join(parity_failed)
        )


if __name__ == "__main__":
    main()
