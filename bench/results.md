# Ruster benchmark results

- Date: 2026-11-26T22:42:53
- Host: Linux-6.8.0-1055-gke-x86_64-with-glibc2.41 (4 CPUs)
- rustc: `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- Interpreter: single binary, 5.0 MB
- One-time interpreter build (`rustc -O`, no cargo): 2766.3 ms wall, 7669.2 ms CPU, 262 MB RSS
- Each program: 5 compile runs, 5 run repetitions; median wall/CPU, max peak RSS.

## Compile phase (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | native binary | Ruster front-end wall | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 74.0 ms | 89.1 ms | 89.6 MB | 4.3 MB | 1.1 ms | 0.9 ms | 15.5 MB |
| fib | 76.6 ms | 91.4 ms | 90.5 MB | 4.3 MB | 1.0 ms | 1.0 ms | 15.8 MB |
| loop_sum | 78.8 ms | 93.7 ms | 90.9 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.9 MB |
| collatz | 79.5 ms | 94.2 ms | 93.1 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.9 MB |
| sieve | 96.7 ms | 111.9 ms | 107.0 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.9 MB |
| matmul | 125.6 ms | 141.3 ms | 107.6 MB | 4.3 MB | 1.1 ms | 1.1 ms | 15.9 MB |
| quicksort | 104.2 ms | 119.3 ms | 107.4 MB | 4.3 MB | 1.2 ms | 1.1 ms | 15.9 MB |
| strings | 119.0 ms | 135.1 ms | 111.4 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.9 MB |

`Ruster front-end` is lexing + parsing + bytecode generation, measured through `rust --compile-only`.

## Runtime phase (per program)

| program | native wall | native CPU | native CPU% | native RSS | Ruster (JIT) wall | CPU | CPU% | RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.9 ms | 0.8 ms | 91% | 15.5 MB | 1.0 ms | 0.9 ms | 93% | 15.5 MB | 1.2x |
| fib | 3.0 ms | 2.9 ms | 97% | 15.6 MB | 192.9 ms | 192.7 ms | 100% | 15.8 MB | 64.4x |
| loop_sum | 3.6 ms | 3.5 ms | 97% | 15.9 MB | 14.2 ms | 14.1 ms | 99% | 15.9 MB | 3.9x |
| collatz | 22.8 ms | 22.6 ms | 99% | 15.9 MB | 70.4 ms | 70.1 ms | 100% | 15.9 MB | 3.1x |
| sieve | 2.4 ms | 2.3 ms | 97% | 15.9 MB | 191.7 ms | 191.5 ms | 100% | 15.9 MB | 81.1x |
| matmul | 1.9 ms | 1.8 ms | 95% | 15.9 MB | 159.7 ms | 159.4 ms | 100% | 15.9 MB | 84.8x |
| quicksort | 4.1 ms | 4.0 ms | 98% | 15.9 MB | 213.9 ms | 213.7 ms | 100% | 15.9 MB | 51.9x |
| strings | 1.2 ms | 1.1 ms | 94% | 15.9 MB | 14.5 ms | 14.3 ms | 99% | 15.9 MB | 12.1x |

## JIT effect (same binary, `--no-jit` vs JIT)

| program | interpreter only | with JIT | JIT speedup | native | JIT vs native |
|---|---:|---:|---:|---:|---:|
| hello | 1.0 ms | 1.0 ms | 0.97x | 0.9 ms | 1.2x |
| fib | 192.7 ms | 192.9 ms | 1.00x | 3.0 ms | 64.4x |
| loop_sum | 942.4 ms | 14.2 ms | 66.25x | 3.6 ms | 3.9x |
| collatz | 1917.6 ms | 70.4 ms | 27.25x | 22.8 ms | 3.1x |
| sieve | 192.0 ms | 191.7 ms | 1.00x | 2.4 ms | 81.1x |
| matmul | 159.4 ms | 159.7 ms | 1.00x | 1.9 ms | 84.8x |
| quicksort | 214.1 ms | 213.9 ms | 1.00x | 4.1 ms | 51.9x |
| strings | 14.6 ms | 14.5 ms | 1.01x | 1.2 ms | 12.1x |

The JIT only covers functions whose parameters, locals, calls and return value are all integers (no division, no methods, no Vec/String). It also excludes recursive call cycles because generated code has no native stack-depth guard, so `fib` stays in the interpreter.


## End-to-end (source -> output)

| program | rustc compile + run | Ruster source -> output | ratio |
|---|---:|---:|---:|
| hello | 74.9 ms | 1.0 ms | 1/73.9 faster |
| fib | 79.6 ms | 192.9 ms | 2.42x |
| loop_sum | 82.4 ms | 14.2 ms | 1/5.8 faster |
| collatz | 102.3 ms | 70.4 ms | 1/1.5 faster |
| sieve | 99.1 ms | 191.7 ms | 1.93x |
| matmul | 127.5 ms | 159.7 ms | 1.25x |
| quicksort | 108.3 ms | 213.9 ms | 1.98x |
| strings | 120.2 ms | 14.5 ms | 1/8.3 faster |

`Ruster source -> output` is a single `bin/rust <file>` invocation (front-end + execution); the separate front-end measurement above is for analysis only and is not added again.


Geometric-mean runtime slowdown across the 8 programs: **15.9x** with the JIT, **40.3x** interpreter-only.

Timed runs: every engine exited 0 in all measured runs.

Parity: native `rustc`, Ruster (JIT), and Ruster `--no-jit` produced identical stdout and exit code for every benchmark program. `tests/compare.py` runs the same three-way check for the feature fixtures.

## Binary / source size

| program | source | native binary | interpreter binary | native/source |
|---|---:|---:|---:|---:|
| hello | 63 B | 4.3 MB | 5.0 MB | 71308.6x |
| fib | 156 B | 4.3 MB | 5.0 MB | 28802.1x |
| loop_sum | 206 B | 4.3 MB | 5.0 MB | 21811.6x |
| collatz | 368 B | 4.3 MB | 5.0 MB | 12209.4x |
| sieve | 562 B | 4.3 MB | 5.0 MB | 7995.6x |
| matmul | 718 B | 4.3 MB | 5.0 MB | 6263.1x |
| quicksort | 948 B | 4.3 MB | 5.0 MB | 4741.2x |
| strings | 349 B | 4.3 MB | 5.0 MB | 12884.4x |
