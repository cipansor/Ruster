# minirust benchmark results

- Date: 2026-11-26T14:36:18
- Host: Linux-6.8.0-1055-gke-x86_64-with-glibc2.41 (4 CPUs)
- rustc: `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- Interpreter: single binary, 5.0 MB
- One-time interpreter build (`rustc -O`, no cargo): 2591.8 ms wall, 7468.8 ms CPU, 263 MB RSS
- Each program: 5 compile runs, 5 run repetitions; median wall/CPU, max peak RSS.

## Compile phase (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | native binary | minirust front-end wall | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 71.9 ms | 87.0 ms | 89.5 MB | 4.3 MB | 1.0 ms | 0.9 ms | 15.6 MB |
| fib | 75.1 ms | 89.5 ms | 90.6 MB | 4.3 MB | 1.0 ms | 1.0 ms | 15.8 MB |
| loop_sum | 75.8 ms | 90.0 ms | 90.9 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| collatz | 76.7 ms | 91.3 ms | 91.9 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| sieve | 95.9 ms | 111.5 ms | 107.1 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| matmul | 123.4 ms | 138.5 ms | 107.4 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| quicksort | 101.9 ms | 117.0 ms | 107.8 MB | 4.3 MB | 1.1 ms | 1.1 ms | 15.8 MB |
| strings | 115.9 ms | 131.4 ms | 111.2 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |

`minirust front-end` is lexing + parsing + bytecode generation, measured through `rust --compile-only`.

## Runtime phase (per program)

| program | native wall | native CPU | native CPU% | native RSS | minirust (JIT) wall | CPU | CPU% | RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.8 ms | 0.8 ms | 92% | 15.6 MB | 1.0 ms | 0.9 ms | 92% | 15.6 MB | 1.2x |
| fib | 3.0 ms | 2.9 ms | 97% | 15.6 MB | 9.4 ms | 9.3 ms | 99% | 15.6 MB | 3.2x |
| loop_sum | 3.6 ms | 3.5 ms | 98% | 15.8 MB | 14.5 ms | 14.3 ms | 99% | 15.8 MB | 4.1x |
| collatz | 22.6 ms | 22.5 ms | 99% | 15.8 MB | 70.4 ms | 70.2 ms | 100% | 15.8 MB | 3.1x |
| sieve | 2.3 ms | 2.2 ms | 96% | 15.8 MB | 189.4 ms | 189.2 ms | 100% | 15.8 MB | 81.5x |
| matmul | 1.8 ms | 1.8 ms | 96% | 15.8 MB | 160.3 ms | 160.0 ms | 100% | 15.8 MB | 87.6x |
| quicksort | 4.3 ms | 4.1 ms | 97% | 15.8 MB | 213.5 ms | 213.0 ms | 100% | 15.8 MB | 50.0x |
| strings | 1.2 ms | 1.1 ms | 93% | 15.8 MB | 12.6 ms | 12.4 ms | 99% | 15.8 MB | 10.7x |

## JIT effect (same binary, `--no-jit` vs JIT)

| program | interpreter only | with JIT | JIT speedup | native | JIT vs native |
|---|---:|---:|---:|---:|---:|
| hello | 1.0 ms | 1.0 ms | 1.00x | 0.8 ms | 1.2x |
| fib | 190.6 ms | 9.4 ms | 20.17x | 3.0 ms | 3.2x |
| loop_sum | 935.0 ms | 14.5 ms | 64.37x | 3.6 ms | 4.1x |
| collatz | 1912.6 ms | 70.4 ms | 27.16x | 22.6 ms | 3.1x |
| sieve | 190.8 ms | 189.4 ms | 1.01x | 2.3 ms | 81.5x |
| matmul | 160.0 ms | 160.3 ms | 1.00x | 1.8 ms | 87.6x |
| quicksort | 212.9 ms | 213.5 ms | 1.00x | 4.3 ms | 50.0x |
| strings | 12.5 ms | 12.6 ms | 1.00x | 1.2 ms | 10.7x |

The JIT only covers functions whose parameters, locals, calls and return value are all integers (no division, no methods, no Vec/String). Everything else keeps running in the interpreter.


## End-to-end (source -> output)

| program | rustc compile + run | minirust front-end + run | ratio |
|---|---:|---:|---:|
| hello | 72.7 ms | 2.0 ms | 1/36.8 faster |
| fib | 78.0 ms | 10.5 ms | 1/7.4 faster |
| loop_sum | 79.4 ms | 15.6 ms | 1/5.1 faster |
| collatz | 99.3 ms | 71.5 ms | 1/1.4 faster |
| sieve | 98.2 ms | 190.5 ms | 1.94x |
| matmul | 125.3 ms | 161.4 ms | 1.29x |
| quicksort | 106.2 ms | 214.6 ms | 2.02x |
| strings | 117.0 ms | 13.7 ms | 1/8.6 faster |

Geometric-mean runtime slowdown across the 8 programs: **10.8x** with the JIT, **40.0x** interpreter-only.

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
