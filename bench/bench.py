#!/usr/bin/env python3
"""Benchmark Ruster against rustc/native code.

Measures, per program:
  compile phase: wall time, CPU time, peak RSS, output size
  runtime phase: wall time, CPU time, peak RSS

Everything is measured with os.wait4() so CPU time and ru_maxrss come from
the kernel for the exact child process (no /usr/bin/time needed).
"""
import argparse
import json
import os
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
PROGRAMS = os.path.join(ROOT, "bench", "programs")
SRC = os.path.join(ROOT, "ruster", "src", "main.rs")
BIN = os.path.join(ROOT, "bin", "rust")
RESULTS_JSON = os.path.join(ROOT, "bench", "results.json")
RESULTS_MD = os.path.join(ROOT, "bench", "results.md")

# name -> (description, scale knob note)
CASES = [
    ("hello", "hello world + one format arg"),
    ("fib", "naive recursive fib(30), 1.6M calls"),
    ("loop_sum", "10M-iteration integer loop (JIT-eligible helper)"),
    ("collatz", "200k Collatz sequences (branchy JIT code)"),
    ("sieve", "prime sieve up to 1,000,000"),
    ("matmul", "100x100 i64 matrix multiply (1M mul-adds)"),
    ("quicksort", "quicksort of 50,000 i64 values"),
    ("strings", "100k push_str/push on a String"),
]


def check_environment():
    if "MINIRUST_NO_JIT" in os.environ:
        raise SystemExit(
            "MINIRUST_NO_JIT is set: the JIT column would silently measure the "
            "interpreter. Unset it and re-run."
        )


check_environment()


def _stdout_equal(src):
    """JIT and interpreter must agree on stdout, not just on the exit code."""
    a = subprocess.run([BIN, src], capture_output=True)
    b = subprocess.run([BIN, "--no-jit", src], capture_output=True)
    return a.stdout == b.stdout and a.returncode == b.returncode


def rss_mb(kb):
    return kb / 1024.0


def run_measured(cmd, cwd=None):
    """Run cmd once, return wall, cpu, peak RSS (KB) and exit code."""
    with open(os.devnull, "wb") as devnull:
        t0 = time.perf_counter()
        proc = subprocess.Popen(
            cmd,
            cwd=cwd,
            stdout=devnull,
            stderr=devnull,
            stdin=subprocess.DEVNULL,
        )
        _, status, usage = os.wait4(proc.pid, 0)
        wall = time.perf_counter() - t0
        proc.returncode = os.waitstatus_to_exitcode(status)
    return {
        "wall": wall,
        "cpu": usage.ru_utime + usage.ru_stime,
        "rss_kb": usage.ru_maxrss,
        "code": proc.returncode,
    }


def summarize(rows):
    walls = [r["wall"] for r in rows]
    cpus = [r["cpu"] for r in rows]
    rss = [r["rss_kb"] for r in rows]
    return {
        "wall": statistics.median(walls),
        "cpu": statistics.median(cpus),
        "rss_kb": max(rss),
        "cpu_pct": 100.0 * statistics.median(cpus) / statistics.median(walls),
        "runs": len(rows),
        "codes": [r["code"] for r in rows],
    }


def build_interpreter():
    rustc = shutil.which("rustc")
    if not rustc:
        sys.exit("rustc not found in PATH")
    os.makedirs(os.path.dirname(BIN), exist_ok=True)
    tmp = BIN + ".tmp"
    row = run_measured([rustc, "--edition", "2021", "-O", SRC, "-o", tmp])
    if row["code"] != 0:
        sys.exit("interpreter build failed")
    os.replace(tmp, BIN)
    return row


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--runs", type=int, default=3)
    ap.add_argument("--compile-runs", type=int, default=3)
    ap.add_argument("--no-build", action="store_true")
    ap.add_argument("--only", default=None, help="comma separated program names")
    ap.add_argument("--render-only", action="store_true", help="re-render results.md from results.json")
    args = ap.parse_args()

    if args.render_only:
        with open(RESULTS_JSON) as f:
            res = json.load(f)
        md = render_markdown(res)
        with open(RESULTS_MD, "w") as f:
            f.write(md)
        print(md)
        return

    only = set(args.only.split(",")) if args.only else None
    cases = [c for c in CASES if not only or c[0] in only]

    build_row = None
    if not args.no_build:
        print("building interpreter with rustc -O ...", flush=True)
        build_row = build_interpreter()
        print(
            f"  build: {build_row['wall']*1000:.0f} ms wall, "
            f"{build_row['cpu']*1000:.0f} ms CPU, "
            f"{rss_mb(build_row['rss_kb']):.0f} MB RSS",
            flush=True,
        )

    interp_size = os.path.getsize(BIN)
    results = {
        "meta": {
            "date": time.strftime("%Y-%m-%dT%H:%M:%S"),
            "host": platform.platform(),
            "cpu_count": os.cpu_count(),
            "rustc": subprocess.run(
                ["rustc", "--version"], capture_output=True, text=True
            ).stdout.strip(),
            "interpreter_size_bytes": interp_size,
            "interpreter_build": build_row,
            "runs": args.runs,
            "compile_runs": args.compile_runs,
        },
        "programs": [],
    }

    for name, desc in cases:
        src = os.path.join(PROGRAMS, name + ".rs")
        src_size = os.path.getsize(src)
        print(f"benchmarking {name} ...", flush=True)
        with tempfile.TemporaryDirectory() as td:
            exe = os.path.join(td, "native")
            compile_rows = []
            for _ in range(args.compile_runs):
                row = run_measured([shutil.which("rustc"), "--edition", "2021", "-O", src, "-o", exe])
                if row["code"] != 0:
                    sys.exit(f"rustc failed for {name}")
                compile_rows.append(row)
            native_compile = summarize(compile_rows)
            native_size = os.path.getsize(exe)

            native_runs = [run_measured([exe]) for _ in range(args.runs)]
            jit_runs = [run_measured([BIN, src]) for _ in range(args.runs)]
            interp_runs = [run_measured([BIN, "--no-jit", src]) for _ in range(args.runs)]

            fe_rows = []
            for _ in range(args.compile_runs):
                fe_rows.append(run_measured([BIN, "--compile-only", src]))

        native_run = summarize(native_runs)
        jit_run = summarize(jit_runs)
        interp_run = summarize(interp_runs)
        front_end = summarize(fe_rows)

        entry = {
            "name": name,
            "description": desc,
            "source_bytes": src_size,
            "native_compile": native_compile,
            "native_size_bytes": native_size,
            "front_end": front_end,
            "native_run": native_run,
            "jit_run": jit_run,
            "interp_run": interp_run,
            "slowdown": jit_run["wall"] / native_run["wall"]
            if native_run["wall"] > 0
            else None,
            "interp_slowdown": interp_run["wall"] / native_run["wall"]
            if native_run["wall"] > 0
            else None,
            "jit_speedup": interp_run["wall"] / jit_run["wall"]
            if jit_run["wall"] > 0
            else None,
            "code_match": (
                native_run["codes"] == jit_run["codes"] == interp_run["codes"]
                and all(code == 0 for code in native_run["codes"])
                and _stdout_equal(src)
            ),
        }
        results["programs"].append(entry)
        print(
            f"  native run {native_run['wall']*1000:.1f} ms | "
            f"jit run {jit_run['wall']*1000:.1f} ms | "
            f"interp-only {interp_run['wall']*1000:.1f} ms | "
            f"slowdown {entry['slowdown']:.1f}x | jit speedup {entry['jit_speedup']:.1f}x",
            flush=True,
        )

    with open(RESULTS_JSON, "w") as f:
        json.dump(results, f, indent=2)

    md = render_markdown(results)
    with open(RESULTS_MD, "w") as f:
        f.write(md)
    print("\n" + md)
    print(f"\nwrote {RESULTS_JSON} and {RESULTS_MD}")


def fmt_ms(s):
    return f"{s*1000:.1f}"


def fmt_bytes(n):
    if n < 1024:
        return f"{n} B"
    if n < 1024 * 1024:
        return f"{n/1024:.1f} KB"
    return f"{n/1024/1024:.1f} MB"


def render_markdown(res):
    meta = res["meta"]
    lines = []
    lines.append("# Ruster benchmark results\n")
    lines.append(f"- Date: {meta['date']}")
    lines.append(f"- Host: {meta['host']} ({meta['cpu_count']} CPUs)")
    lines.append(f"- rustc: `{meta['rustc']}`")
    lines.append(f"- Interpreter: single binary, {fmt_bytes(meta['interpreter_size_bytes'])}")
    if meta.get("interpreter_build"):
        b = meta["interpreter_build"]
        lines.append(
            f"- One-time interpreter build (`rustc -O`, no cargo): "
            f"{fmt_ms(b['wall'])} ms wall, {fmt_ms(b['cpu'])} ms CPU, "
            f"{rss_mb(b['rss_kb']):.0f} MB RSS"
        )
    lines.append(
        f"- Each program: {meta['compile_runs']} compile runs, {meta['runs']} run repetitions; "
        "median wall/CPU, max peak RSS.\n"
    )

    lines.append("## Compile phase (per program)\n")
    lines.append(
        "| program | rustc -O wall | rustc CPU | rustc RSS | native binary | "
        "Ruster front-end wall | front-end CPU | front-end RSS |"
    )
    lines.append("|---|---:|---:|---:|---:|---:|---:|---:|")
    for p in res["programs"]:
        c = p["native_compile"]
        fe = p["front_end"]
        lines.append(
            f"| {p['name']} | {fmt_ms(c['wall'])} ms | {fmt_ms(c['cpu'])} ms | "
            f"{rss_mb(c['rss_kb']):.1f} MB | {fmt_bytes(p['native_size_bytes'])} | "
            f"{fmt_ms(fe['wall'])} ms | {fmt_ms(fe['cpu'])} ms | "
            f"{rss_mb(fe['rss_kb']):.1f} MB |"
        )
    lines.append("\n`Ruster front-end` is lexing + parsing + bytecode generation, " f"measured through `rust --compile-only`.\n")

    lines.append("## Runtime phase (per program)\n")
    lines.append(
        "| program | native wall | native CPU | native CPU% | native RSS | "
        "Ruster (JIT) wall | CPU | CPU% | RSS | slowdown |"
    )
    lines.append("|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|")
    for p in res["programs"]:
        n = p["native_run"]
        i = p.get("jit_run", p["interp_run"])
        lines.append(
            f"| {p['name']} | {fmt_ms(n['wall'])} ms | {fmt_ms(n['cpu'])} ms | "
            f"{n['cpu_pct']:.0f}% | {rss_mb(n['rss_kb']):.1f} MB | "
            f"{fmt_ms(i['wall'])} ms | {fmt_ms(i['cpu'])} ms | {i['cpu_pct']:.0f}% | "
            f"{rss_mb(i['rss_kb']):.1f} MB | {p['slowdown']:.1f}x |"
        )

    lines.append("\n## JIT effect (same binary, `--no-jit` vs JIT)\n")
    lines.append(
        "| program | interpreter only | with JIT | JIT speedup | native | JIT vs native |"
    )
    lines.append("|---|---:|---:|---:|---:|---:|")
    for p in res["programs"]:
        i = p["interp_run"]
        j = p.get("jit_run", i)
        n = p["native_run"]
        lines.append(
            f"| {p['name']} | {fmt_ms(i['wall'])} ms | {fmt_ms(j['wall'])} ms | "
            f"{(i['wall']/j['wall'] if j['wall'] > 0 else 1):.2f}x | "
            f"{fmt_ms(n['wall'])} ms | {p['slowdown']:.1f}x |"
        )
    lines.append(
        "\nThe JIT only covers functions whose parameters, locals, calls and "
        "return value are all integers (no division, no methods, no Vec/String). "
        "Everything else keeps running in the interpreter.\n"
    )

    lines.append("\n## End-to-end (source -> output)\n")
    lines.append("| program | rustc compile + run | Ruster front-end + run | ratio |")
    lines.append("|---|---:|---:|---:|")
    for p in res["programs"]:
        native_total = p["native_compile"]["wall"] + p["native_run"]["wall"]
        interp_total = p["front_end"]["wall"] + p.get("jit_run", p["interp_run"])["wall"]
        ratio = interp_total / native_total
        label = f"{ratio:.2f}x" if ratio >= 1 else f"1/{1/ratio:.1f} faster"
        lines.append(
            f"| {p['name']} | {fmt_ms(native_total)} ms | {fmt_ms(interp_total)} ms | "
            f"{label} |"
        )

    geo = 1.0
    interp_geo = 1.0
    for p in res["programs"]:
        geo *= max(p["slowdown"], 1e-9)
        interp_geo *= max(p.get("interp_slowdown") or p["slowdown"], 1e-9)
    n = len(res["programs"])
    geo = geo ** (1.0 / n)
    interp_geo = interp_geo ** (1.0 / n)
    lines.append(
        f"\nGeometric-mean runtime slowdown across the {n} programs: "
        f"**{geo:.1f}x** with the JIT, **{interp_geo:.1f}x** interpreter-only.\n"
    )
    lines.append(
        "All programs produced identical exit codes under both engines; stdout "
        "parity (native vs Ruster) is enforced by `tests/compare.py`. "
        f"({all(p['code_match'] for p in res['programs'])}).\n"
    )

    lines.append("## Binary / source size\n")
    lines.append("| program | source | native binary | interpreter binary | native/source |")
    lines.append("|---|---:|---:|---:|---:|")
    for p in res["programs"]:
        lines.append(
            f"| {p['name']} | {fmt_bytes(p['source_bytes'])} | "
            f"{fmt_bytes(p['native_size_bytes'])} | {fmt_bytes(meta['interpreter_size_bytes'])} | "
            f"{p['native_size_bytes']/p['source_bytes']:.1f}x |"
        )
    return "\n".join(lines) + "\n"


if __name__ == "__main__":
    main()
