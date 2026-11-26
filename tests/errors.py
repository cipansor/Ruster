#!/usr/bin/env python3
"""Check exit codes and error behaviour of Ruster itself."""
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MINIRUST = os.path.join(ROOT, "bin", "rust")

CASES = [
    ("tests/errors/index_oob.rs", 101, "index out of bounds"),
    ("tests/errors/assert_fail.rs", 101, "assertion"),
    ("tests/errors/panic.rs", 101, "boom 7"),
    ("tests/errors/exit.rs", 3, "before"),
]

failed = 0
for rel, want_code, want_text in CASES:
    path = os.path.join(ROOT, rel)
    p = subprocess.run([MINIRUST, path], capture_output=True, text=True)
    ok = p.returncode == want_code and (
        (want_text in p.stdout) or (want_text in p.stderr) or want_text == ""
    )
    print(f"{'ok  ' if ok else 'FAIL'} {rel}: exit={p.returncode} (want {want_code})")
    if not ok:
        print("  stdout:", p.stdout)
        print("  stderr:", p.stderr)
        failed += 1

print(f"\n{len(CASES)-failed}/{len(CASES)} error cases behave as expected")
sys.exit(1 if failed else 0)
