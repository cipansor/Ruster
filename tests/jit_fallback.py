#!/usr/bin/env python3
"""Assert that recursive call cycles stay in the interpreter.

`tests/compare.py` only checks that the JIT and the interpreter agree on
output. That is not enough for mutual recursion: a wrongly JIT-compiled cycle
could still produce the same answer until the native stack overflows. Inspect
the JIT function list reported by `--stats` instead.

The fixture keeps one acyclic function (`leaf`) next to the cycles so this
check cannot pass vacuously on a build where nothing is JIT-compiled.
"""
import os
import platform
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MINIRUST = os.path.join(ROOT, "bin", "rust")
PROGRAM = os.path.join(ROOT, "tests", "programs", "recursion.rs")

JIT_ARCH = platform.machine() in ("x86_64", "AMD64") and os.environ.get(
    "REQUIRE_JIT", "1"
) != "0"

INTERPRETED = ("down", "even", "odd")  # direct recursion + a mutual cycle
COMPILED = ("leaf",)  # acyclic control


def jit_functions(path):
    proc = subprocess.run([MINIRUST, "--stats", path], capture_output=True, text=True)
    if proc.returncode != 0:
        raise SystemExit(f"`--stats` failed for {path}:\n{proc.stderr}")
    for line in proc.stderr.splitlines():
        if not line.startswith("Ruster jit:"):
            continue
        if "no functions were eligible" in line or "disabled" in line:
            return []
        if "function(s):" in line:
            names = line.split("function(s):", 1)[1]
            return [n.strip() for n in names.split(",") if n.strip()]
    raise SystemExit(f"no JIT stats line for {path}:\n{proc.stderr}")


def main():
    if not JIT_ARCH:
        print("skip: JIT fallback checks need an x86_64 host")
        return 0
    names = jit_functions(PROGRAM)
    failed = 0
    for fn in COMPILED:
        if fn not in names:
            print(f"FAIL acyclic `{fn}` should be JIT-compiled; got {names}")
            failed += 1
    for fn in INTERPRETED:
        if fn in names:
            print(f"FAIL recursive `{fn}` must stay in the interpreter; got {names}")
            failed += 1
    if failed == 0:
        print(f"ok   recursive cycles stay interpreted; acyclic {COMPILED} compiled")
        print(f"     jit functions: {', '.join(names)}")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
