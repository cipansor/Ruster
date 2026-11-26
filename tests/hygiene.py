#!/usr/bin/env python3
"""Repository hygiene checks that do not need a Rust toolchain."""
import os
import subprocess
import sys

LIMIT = 1_000_000
BAD_DIRS = ("bin/", ".openhands/", "target/")
REQUIRED = ("README.md", "ruster/src/main.rs", "tests/compare.py", "tests/errors.py")

tracked = subprocess.check_output(["git", "ls-files"]).decode().split()
problems = []

for path in tracked:
    if not os.path.exists(path):
        continue
    size = os.path.getsize(path)
    if size > LIMIT:
        problems.append(f"tracked file larger than 1 MB: {path} ({size} bytes)")

for path in tracked:
    for bad in BAD_DIRS:
        if path.startswith(bad):
            problems.append(f"build artefact or local state is tracked: {path}")

for path in REQUIRED:
    if path not in tracked:
        problems.append(f"required file missing from the repository: {path}")

if problems:
    for p in problems:
        print(f"::error::{p}")
    sys.exit(1)
print(f"hygiene ok: {len(tracked)} tracked files, none over {LIMIT} bytes")
