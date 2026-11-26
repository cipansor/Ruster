#!/usr/bin/env python3
"""Run every tests/programs/*.rs with rustc, Ruster (JIT on) and
Ruster --no-jit, and require all three to agree on stdout and exit code."""
import glob
import platform
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MINIRUST = os.path.join(ROOT, "bin", "rust")
RUSTC = os.environ.get("RUSTC", "rustc")


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


REQUIRE_JIT = platform.machine() in ("x86_64", "AMD64") and os.environ.get(
    "REQUIRE_JIT", "1"
) != "0"


def main():
    files = sorted(glob.glob(os.path.join(ROOT, "tests", "programs", "*.rs")))
    failed = 0
    jit_functions = 0
    for path in files:
        name = os.path.basename(path)
        with tempfile.TemporaryDirectory() as td:
            exe = os.path.join(td, "prog")
            compiled = run([RUSTC, "--edition", "2024", "-O", path, "-o", exe])
            if compiled.returncode != 0:
                print(f"RUSTC-FAIL {name}\n{compiled.stderr}")
                failed += 1
                continue
            native = run([exe])
            interp = run([MINIRUST, path])
            interp_nojit = run([MINIRUST, "--no-jit", path])
            stats = run([MINIRUST, "--stats", path])
        for line in stats.stderr.splitlines():
            if line.startswith("Ruster jit:") and "function(s)" in line:
                jit_functions += int(line.split("jit:")[1].split("function")[0].strip())
        if interp.stdout != interp_nojit.stdout or interp.returncode != interp_nojit.returncode:
            print(f"JIT-DIFF {name}")
            print("--- jit ---")
            print(interp.stdout)
            print("--- no-jit ---")
            print(interp_nojit.stdout)
            failed += 1
        elif native.stdout != interp.stdout:
            print(f"DIFF {name}")
            print("--- native ---")
            print(native.stdout)
            print("--- Ruster ---")
            print(interp.stdout)
            if interp.stderr:
                print("--- Ruster stderr ---")
                print(interp.stderr)
            failed += 1
        elif native.returncode != interp.returncode:
            print(f"EXIT {name}: native={native.returncode} Ruster={interp.returncode}")
            if interp.stderr:
                print(interp.stderr)
            failed += 1
        else:
            print(f"ok   {name}")
    if REQUIRE_JIT and jit_functions == 0:
        print("JIT-NOT-EXERCISED: no test program executed JIT-compiled code")
        failed += 1

    total = len(files)
    print(f"\n{total - failed}/{total} programs match")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
