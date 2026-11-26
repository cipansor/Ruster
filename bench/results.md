# minirust benchmark results

- Date: 2026-11-26T14:10:47
- Host: Linux-6.8.0-1055-gke-x86_64-with-glibc2.41 (4 CPUs)
- rustc: `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- Interpreter: single binary, 4.9 MB
- One-time interpreter build (`rustc -O`, no cargo): 2541.6 ms wall, 6317.0 ms CPU, 247 MB RSS
- Each program: 5 compile runs, 5 run repetitions; median wall/CPU, max peak RSS.

## Compile phase (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | native binary | minirust front-end wall | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 72.8 ms | 87.9 ms | 89.5 MB | 4.3 MB | 1.0 ms | 0.9 ms | 15.6 MB |
| fib | 75.9 ms | 90.8 ms | 90.6 MB | 4.3 MB | 1.0 ms | 0.9 ms | 15.8 MB |
| loop_sum | 77.7 ms | 91.8 ms | 90.8 MB | 4.3 MB | 1.0 ms | 1.0 ms | 15.8 MB |
| collatz | 77.4 ms | 92.0 ms | 92.1 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| sieve | 95.1 ms | 109.7 ms | 106.9 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.9 MB |
| matmul | 124.6 ms | 139.9 ms | 107.5 MB | 4.3 MB | 1.2 ms | 1.1 ms | 15.9 MB |
| quicksort | 103.1 ms | 117.7 ms | 108.7 MB | 4.3 MB | 1.1 ms | 1.1 ms | 15.9 MB |
| strings | 116.5 ms | 131.5 ms | 111.2 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.9 MB |

`minirust front-end` is lexing + parsing + bytecode generation, measured through `rust --compile-only`.

## Runtime phase (per program)

| program | native wall | native CPU | native CPU% | native RSS | minirust (JIT) wall | CPU | CPU% | RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.9 ms | 0.8 ms | 91% | 15.6 MB | 1.1 ms | 1.0 ms | 91% | 15.6 MB | 1.2x |
| fib | 3.0 ms | 2.9 ms | 97% | 15.6 MB | 9.3 ms | 9.2 ms | 99% | 15.6 MB | 3.1x |
| loop_sum | 3.6 ms | 3.5 ms | 97% | 15.8 MB | 14.1 ms | 14.0 ms | 99% | 15.8 MB | 3.9x |
| collatz | 22.9 ms | 22.8 ms | 99% | 15.8 MB | 70.6 ms | 70.4 ms | 100% | 15.8 MB | 3.1x |
| sieve | 2.3 ms | 2.2 ms | 96% | 15.9 MB | 309.4 ms | 309.2 ms | 100% | 18.0 MB | 135.1x |
| matmul | 2.6 ms | 2.5 ms | 95% | 15.9 MB | 226.7 ms | 226.5 ms | 100% | 15.9 MB | 86.0x |
| quicksort | 4.2 ms | 4.1 ms | 97% | 15.9 MB | 217.6 ms | 217.4 ms | 100% | 15.9 MB | 51.6x |
| strings | 1.3 ms | 1.2 ms | 94% | 15.9 MB | 12.5 ms | 12.4 ms | 99% | 15.9 MB | 9.7x |

## JIT effect (same binary, `--no-jit` vs JIT)

| program | interpreter only | with JIT | JIT speedup | native | JIT vs native |
|---|---:|---:|---:|---:|---:|
| hello | 1.3 ms | 1.1 ms | 1.16x | 0.9 ms | 1.2x |
| fib | 193.6 ms | 9.3 ms | 20.75x | 3.0 ms | 3.1x |
| loop_sum | 939.4 ms | 14.1 ms | 66.76x | 3.6 ms | 3.9x |
| collatz | 1966.5 ms | 70.6 ms | 27.85x | 22.9 ms | 3.1x |
| sieve | 307.5 ms | 309.4 ms | 0.99x | 2.3 ms | 135.1x |
| matmul | 225.9 ms | 226.7 ms | 1.00x | 2.6 ms | 86.0x |
| quicksort | 219.0 ms | 217.6 ms | 1.01x | 4.2 ms | 51.6x |
| strings | 12.5 ms | 12.5 ms | 1.00x | 1.3 ms | 9.7x |

The JIT only covers functions whose parameters, locals, calls and return value are all integers (no division, no methods, no Vec/String). Everything else keeps running in the interpreter.


## End-to-end (source -> output)

| program | rustc compile + run | minirust front-end + run | ratio |
|---|---:|---:|---:|
| hello | 73.7 ms | 2.1 ms | 1/34.8 faster |
| fib | 79.0 ms | 10.3 ms | 1/7.6 faster |
| loop_sum | 81.3 ms | 15.1 ms | 1/5.4 faster |
| collatz | 100.3 ms | 71.7 ms | 1/1.4 faster |
| sieve | 97.3 ms | 310.5 ms | 3.19x |
| matmul | 127.3 ms | 227.9 ms | 1.79x |
| quicksort | 107.3 ms | 218.7 ms | 2.04x |
| strings | 117.8 ms | 13.5 ms | 1/8.7 faster |

Geometric-mean runtime slowdown across the 8 programs: **11.3x** with the JIT, **43.0x** interpreter-only.

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
