# minirust benchmark results

- Date: 2026-11-26T14:18:36
- Host: Linux-6.8.0-1055-gke-x86_64-with-glibc2.41 (4 CPUs)
- rustc: `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- Interpreter: single binary, 4.9 MB
- One-time interpreter build (`rustc -O`, no cargo): 2352.6 ms wall, 6492.2 ms CPU, 252 MB RSS
- Each program: 5 compile runs, 5 run repetitions; median wall/CPU, max peak RSS.

## Compile phase (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | native binary | minirust front-end wall | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 74.2 ms | 89.7 ms | 89.8 MB | 4.3 MB | 1.0 ms | 0.9 ms | 15.6 MB |
| fib | 77.2 ms | 91.9 ms | 90.4 MB | 4.3 MB | 1.0 ms | 1.0 ms | 15.8 MB |
| loop_sum | 78.6 ms | 93.6 ms | 91.0 MB | 4.3 MB | 1.0 ms | 1.0 ms | 15.8 MB |
| collatz | 77.3 ms | 91.4 ms | 92.1 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| sieve | 96.1 ms | 111.6 ms | 107.0 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.9 MB |
| matmul | 124.2 ms | 139.4 ms | 107.4 MB | 4.3 MB | 1.2 ms | 1.1 ms | 15.9 MB |
| quicksort | 102.2 ms | 116.9 ms | 107.4 MB | 4.3 MB | 1.2 ms | 1.1 ms | 15.9 MB |
| strings | 117.9 ms | 133.3 ms | 111.2 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.9 MB |

`minirust front-end` is lexing + parsing + bytecode generation, measured through `rust --compile-only`.

## Runtime phase (per program)

| program | native wall | native CPU | native CPU% | native RSS | minirust (JIT) wall | CPU | CPU% | RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.9 ms | 0.8 ms | 91% | 15.6 MB | 1.0 ms | 0.9 ms | 93% | 15.6 MB | 1.2x |
| fib | 3.2 ms | 3.1 ms | 97% | 15.6 MB | 9.6 ms | 9.4 ms | 98% | 15.8 MB | 3.0x |
| loop_sum | 3.6 ms | 3.5 ms | 97% | 15.8 MB | 14.1 ms | 14.0 ms | 99% | 15.8 MB | 3.9x |
| collatz | 22.5 ms | 22.3 ms | 99% | 15.8 MB | 70.8 ms | 70.6 ms | 100% | 15.8 MB | 3.1x |
| sieve | 2.3 ms | 2.2 ms | 97% | 15.9 MB | 237.8 ms | 237.4 ms | 100% | 18.0 MB | 103.6x |
| matmul | 1.8 ms | 1.8 ms | 96% | 15.9 MB | 173.9 ms | 173.7 ms | 100% | 15.9 MB | 94.9x |
| quicksort | 3.9 ms | 3.9 ms | 98% | 15.9 MB | 216.6 ms | 216.4 ms | 100% | 15.9 MB | 55.0x |
| strings | 1.2 ms | 1.1 ms | 93% | 15.9 MB | 12.6 ms | 12.5 ms | 99% | 15.9 MB | 10.5x |

## JIT effect (same binary, `--no-jit` vs JIT)

| program | interpreter only | with JIT | JIT speedup | native | JIT vs native |
|---|---:|---:|---:|---:|---:|
| hello | 1.0 ms | 1.0 ms | 0.97x | 0.9 ms | 1.2x |
| fib | 191.2 ms | 9.6 ms | 19.98x | 3.2 ms | 3.0x |
| loop_sum | 932.0 ms | 14.1 ms | 65.88x | 3.6 ms | 3.9x |
| collatz | 1972.7 ms | 70.8 ms | 27.87x | 22.5 ms | 3.1x |
| sieve | 236.3 ms | 237.8 ms | 0.99x | 2.3 ms | 103.6x |
| matmul | 172.6 ms | 173.9 ms | 0.99x | 1.8 ms | 94.9x |
| quicksort | 219.0 ms | 216.6 ms | 1.01x | 3.9 ms | 55.0x |
| strings | 12.8 ms | 12.6 ms | 1.01x | 1.2 ms | 10.5x |

The JIT only covers functions whose parameters, locals, calls and return value are all integers (no division, no methods, no Vec/String). Everything else keeps running in the interpreter.


## End-to-end (source -> output)

| program | rustc compile + run | minirust front-end + run | ratio |
|---|---:|---:|---:|
| hello | 75.1 ms | 2.0 ms | 1/37.3 faster |
| fib | 80.3 ms | 10.6 ms | 1/7.6 faster |
| loop_sum | 82.2 ms | 15.2 ms | 1/5.4 faster |
| collatz | 99.8 ms | 71.9 ms | 1/1.4 faster |
| sieve | 98.4 ms | 238.8 ms | 2.43x |
| matmul | 126.0 ms | 175.0 ms | 1.39x |
| quicksort | 106.1 ms | 217.8 ms | 2.05x |
| strings | 119.1 ms | 13.7 ms | 1/8.7 faster |

Geometric-mean runtime slowdown across the 8 programs: **11.2x** with the JIT, **41.5x** interpreter-only.

All programs produced identical output and exit code under both engines (True).

## Binary / source size

| program | source | native binary | interpreter binary | native/source |
|---|---:|---:|---:|---:|
| hello | 62 B | 4.3 MB | 4.9 MB | 72458.7x |
| fib | 156 B | 4.3 MB | 4.9 MB | 28802.1x |
| loop_sum | 206 B | 4.3 MB | 4.9 MB | 21811.6x |
| collatz | 368 B | 4.3 MB | 4.9 MB | 12209.4x |
| sieve | 562 B | 4.3 MB | 4.9 MB | 7995.6x |
| matmul | 718 B | 4.3 MB | 4.9 MB | 6263.1x |
| quicksort | 948 B | 4.3 MB | 4.9 MB | 4741.1x |
| strings | 349 B | 4.3 MB | 4.9 MB | 12884.4x |
