#!/usr/bin/env python3
"""Run every tests/programs/*.rs with rustc, minirust (JIT on) and
minirust --no-jit, and require all three to agree on stdout and exit code."""
import glob
import os
import subprocess
import sys
import tempfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MINIRUST = os.path.join(ROOT, "bin", "rust")
RUSTC = os.environ.get("RUSTC", "rustc")


def run(cmd):
    return subprocess.run(cmd, capture_output=True, text=True)


def main():
    files = sorted(glob.glob(os.path.join(ROOT, "tests", "programs", "*.rs")))
    failed = 0
    for path in files:
        name = os.path.basename(path)
        with tempfile.TemporaryDirectory() as td:
            exe = os.path.join(td, "prog")
            compiled = run([RUSTC, "--edition", "2021", "-O", path, "-o", exe])
            if compiled.returncode != 0:
                print(f"RUSTC-FAIL {name}\n{compiled.stderr}")
                failed += 1
                continue
            native = run([exe])
            interp = run([MINIRUST, path])
            interp_nojit = run([MINIRUST, "--no-jit", path])
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
            print("--- minirust ---")
            print(interp.stdout)
            if interp.stderr:
                print("--- minirust stderr ---")
                print(interp.stderr)
            failed += 1
        elif native.returncode != interp.returncode:
            print(f"EXIT {name}: native={native.returncode} minirust={interp.returncode}")
            if interp.stderr:
                print(interp.stderr)
            failed += 1
        else:
            print(f"ok   {name}")
    total = len(files)
    print(f"\n{total - failed}/{total} programs match")
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
