# Ruster benchmark results

- Date: 2026-11-26T22:30:03
- Host: Linux-6.8.0-1055-gke-x86_64-with-glibc2.41 (4 CPUs)
- rustc: `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- Interpreter: single binary, 5.0 MB
- One-time interpreter build (`rustc -O`, no cargo): 2655.2 ms wall, 7696.0 ms CPU, 259 MB RSS
- Each program: 5 compile runs, 5 run repetitions; median wall/CPU, max peak RSS.

## Compile phase (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | native binary | Ruster front-end wall | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 74.0 ms | 89.4 ms | 89.7 MB | 4.3 MB | 1.0 ms | 0.9 ms | 15.5 MB |
| fib | 78.2 ms | 92.2 ms | 90.6 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.6 MB |
| loop_sum | 78.7 ms | 93.2 ms | 91.1 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.6 MB |
| collatz | 78.7 ms | 94.0 ms | 92.2 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| sieve | 96.6 ms | 112.1 ms | 107.1 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| matmul | 126.2 ms | 141.6 ms | 107.4 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| quicksort | 106.9 ms | 121.9 ms | 107.2 MB | 4.3 MB | 1.2 ms | 1.1 ms | 15.9 MB |
| strings | 120.2 ms | 136.6 ms | 111.5 MB | 4.3 MB | 1.1 ms | 1.0 ms | 16.0 MB |

`Ruster front-end` is lexing + parsing + bytecode generation, measured through `rust --compile-only`.

## Runtime phase (per program)

| program | native wall | native CPU | native CPU% | native RSS | Ruster (JIT) wall | CPU | CPU% | RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.9 ms | 0.8 ms | 91% | 15.5 MB | 1.0 ms | 1.0 ms | 93% | 15.5 MB | 1.2x |
| fib | 3.0 ms | 2.9 ms | 97% | 15.6 MB | 192.5 ms | 192.3 ms | 100% | 15.6 MB | 64.0x |
| loop_sum | 3.6 ms | 3.5 ms | 97% | 15.6 MB | 14.3 ms | 14.2 ms | 99% | 15.6 MB | 3.9x |
| collatz | 22.7 ms | 22.5 ms | 99% | 15.6 MB | 71.6 ms | 71.5 ms | 100% | 15.8 MB | 3.2x |
| sieve | 2.3 ms | 2.2 ms | 97% | 15.8 MB | 192.7 ms | 192.4 ms | 100% | 15.8 MB | 83.4x |
| matmul | 1.9 ms | 1.8 ms | 94% | 15.8 MB | 160.3 ms | 160.0 ms | 100% | 15.8 MB | 84.4x |
| quicksort | 4.0 ms | 3.9 ms | 98% | 15.9 MB | 215.2 ms | 215.0 ms | 100% | 15.9 MB | 53.5x |
| strings | 1.3 ms | 1.2 ms | 92% | 16.0 MB | 14.8 ms | 14.6 ms | 99% | 16.0 MB | 11.5x |

## JIT effect (same binary, `--no-jit` vs JIT)

| program | interpreter only | with JIT | JIT speedup | native | JIT vs native |
|---|---:|---:|---:|---:|---:|
| hello | 1.1 ms | 1.0 ms | 1.04x | 0.9 ms | 1.2x |
| fib | 192.7 ms | 192.5 ms | 1.00x | 3.0 ms | 64.0x |
| loop_sum | 933.7 ms | 14.3 ms | 65.22x | 3.6 ms | 3.9x |
| collatz | 1918.9 ms | 71.6 ms | 26.78x | 22.7 ms | 3.2x |
| sieve | 193.6 ms | 192.7 ms | 1.00x | 2.3 ms | 83.4x |
| matmul | 161.4 ms | 160.3 ms | 1.01x | 1.9 ms | 84.4x |
| quicksort | 214.9 ms | 215.2 ms | 1.00x | 4.0 ms | 53.5x |
| strings | 14.8 ms | 14.8 ms | 1.00x | 1.3 ms | 11.5x |

The JIT only covers functions whose parameters, locals, calls and return value are all integers (no division, no methods, no Vec/String). It also excludes recursive call cycles because generated code has no native stack-depth guard, so `fib` stays in the interpreter.


## End-to-end (source -> output)

| program | rustc compile + run | Ruster front-end + run | ratio |
|---|---:|---:|---:|
| hello | 74.9 ms | 2.0 ms | 1/36.8 faster |
| fib | 81.2 ms | 193.7 ms | 2.39x |
| loop_sum | 82.4 ms | 15.4 ms | 1/5.4 faster |
| collatz | 101.4 ms | 72.7 ms | 1/1.4 faster |
| sieve | 98.9 ms | 193.8 ms | 1.96x |
| matmul | 128.1 ms | 161.4 ms | 1.26x |
| quicksort | 111.0 ms | 216.4 ms | 1.95x |
| strings | 121.5 ms | 15.9 ms | 1/7.6 faster |

Geometric-mean runtime slowdown across the 8 programs: **15.9x** with the JIT, **40.7x** interpreter-only.

All programs produced identical exit codes under both engines; stdout parity (native vs Ruster) is enforced by `tests/compare.py`. (True).

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
