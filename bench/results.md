# minirust benchmark results

- Date: 2026-11-26T14:33:11
- Host: Linux-6.8.0-1055-gke-x86_64-with-glibc2.41 (4 CPUs)
- rustc: `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- Interpreter: single binary, 5.0 MB
- One-time interpreter build (`rustc -O`, no cargo): 2488.9 ms wall, 7126.1 ms CPU, 253 MB RSS
- Each program: 5 compile runs, 5 run repetitions; median wall/CPU, max peak RSS.

## Compile phase (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | native binary | minirust front-end wall | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 74.8 ms | 90.5 ms | 89.9 MB | 4.3 MB | 1.0 ms | 0.9 ms | 15.6 MB |
| fib | 79.9 ms | 92.5 ms | 90.5 MB | 4.3 MB | 1.0 ms | 1.0 ms | 15.6 MB |
| loop_sum | 77.2 ms | 92.5 ms | 91.0 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.6 MB |
| collatz | 78.1 ms | 92.2 ms | 92.1 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.6 MB |
| sieve | 94.1 ms | 108.2 ms | 107.1 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.7 MB |
| matmul | 130.1 ms | 143.7 ms | 107.2 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.7 MB |
| quicksort | 101.2 ms | 116.5 ms | 107.2 MB | 4.3 MB | 1.1 ms | 1.1 ms | 15.7 MB |
| strings | 117.4 ms | 132.5 ms | 111.2 MB | 4.3 MB | 1.0 ms | 1.0 ms | 15.7 MB |

`minirust front-end` is lexing + parsing + bytecode generation, measured through `rust --compile-only`.

## Runtime phase (per program)

| program | native wall | native CPU | native CPU% | native RSS | minirust (JIT) wall | CPU | CPU% | RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.8 ms | 0.8 ms | 91% | 15.6 MB | 1.0 ms | 0.9 ms | 93% | 15.6 MB | 1.2x |
| fib | 3.0 ms | 2.9 ms | 97% | 15.6 MB | 9.5 ms | 9.4 ms | 99% | 15.6 MB | 3.2x |
| loop_sum | 3.7 ms | 3.6 ms | 98% | 15.6 MB | 14.2 ms | 14.1 ms | 99% | 15.6 MB | 3.9x |
| collatz | 22.5 ms | 22.3 ms | 99% | 15.6 MB | 70.4 ms | 70.2 ms | 100% | 15.6 MB | 3.1x |
| sieve | 2.5 ms | 2.4 ms | 96% | 15.6 MB | 209.1 ms | 208.9 ms | 100% | 15.6 MB | 84.6x |
| matmul | 2.6 ms | 2.5 ms | 96% | 15.7 MB | 170.1 ms | 169.9 ms | 100% | 15.7 MB | 65.1x |
| quicksort | 4.2 ms | 4.2 ms | 98% | 15.7 MB | 208.2 ms | 208.0 ms | 100% | 15.7 MB | 49.0x |
| strings | 1.2 ms | 1.1 ms | 93% | 15.7 MB | 12.5 ms | 12.3 ms | 99% | 15.7 MB | 10.2x |

## JIT effect (same binary, `--no-jit` vs JIT)

| program | interpreter only | with JIT | JIT speedup | native | JIT vs native |
|---|---:|---:|---:|---:|---:|
| hello | 1.0 ms | 1.0 ms | 1.00x | 0.8 ms | 1.2x |
| fib | 189.6 ms | 9.5 ms | 19.99x | 3.0 ms | 3.2x |
| loop_sum | 930.3 ms | 14.2 ms | 65.50x | 3.7 ms | 3.9x |
| collatz | 1907.5 ms | 70.4 ms | 27.11x | 22.5 ms | 3.1x |
| sieve | 208.1 ms | 209.1 ms | 0.99x | 2.5 ms | 84.6x |
| matmul | 169.7 ms | 170.1 ms | 1.00x | 2.6 ms | 65.1x |
| quicksort | 208.0 ms | 208.2 ms | 1.00x | 4.2 ms | 49.0x |
| strings | 12.5 ms | 12.5 ms | 1.01x | 1.2 ms | 10.2x |

The JIT only covers functions whose parameters, locals, calls and return value are all integers (no division, no methods, no Vec/String). Everything else keeps running in the interpreter.


## End-to-end (source -> output)

| program | rustc compile + run | minirust front-end + run | ratio |
|---|---:|---:|---:|
| hello | 75.6 ms | 2.0 ms | 1/38.0 faster |
| fib | 82.9 ms | 10.5 ms | 1/7.9 faster |
| loop_sum | 80.9 ms | 15.3 ms | 1/5.3 faster |
| collatz | 100.6 ms | 71.4 ms | 1/1.4 faster |
| sieve | 96.6 ms | 210.2 ms | 2.18x |
| matmul | 132.7 ms | 171.3 ms | 1.29x |
| quicksort | 105.4 ms | 209.4 ms | 1.99x |
| strings | 118.6 ms | 13.5 ms | 1/8.8 faster |

Geometric-mean runtime slowdown across the 8 programs: **10.3x** with the JIT, **38.1x** interpreter-only.

All programs produced identical output and exit code under both engines (True).

## Binary / source size

| program | source | native binary | interpreter binary | native/source |
|---|---:|---:|---:|---:|
| hello | 62 B | 4.3 MB | 5.0 MB | 72458.7x |
| fib | 156 B | 4.3 MB | 5.0 MB | 28802.1x |
| loop_sum | 206 B | 4.3 MB | 5.0 MB | 21811.6x |
| collatz | 368 B | 4.3 MB | 5.0 MB | 12209.4x |
| sieve | 562 B | 4.3 MB | 5.0 MB | 7995.6x |
| matmul | 718 B | 4.3 MB | 5.0 MB | 6263.1x |
| quicksort | 948 B | 4.3 MB | 5.0 MB | 4741.1x |
| strings | 349 B | 4.3 MB | 5.0 MB | 12884.4x |
