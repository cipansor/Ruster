# minirust benchmark results

- Date: 2026-11-26T14:30:12
- Host: Linux-6.8.0-1055-gke-x86_64-with-glibc2.41 (4 CPUs)
- rustc: `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- Interpreter: single binary, 4.9 MB
- One-time interpreter build (`rustc -O`, no cargo): 2515.1 ms wall, 6841.3 ms CPU, 254 MB RSS
- Each program: 5 compile runs, 5 run repetitions; median wall/CPU, max peak RSS.

## Compile phase (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | native binary | minirust front-end wall | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 72.4 ms | 86.6 ms | 89.5 MB | 4.3 MB | 1.0 ms | 0.9 ms | 15.6 MB |
| fib | 75.2 ms | 89.4 ms | 90.4 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.6 MB |
| loop_sum | 76.3 ms | 90.6 ms | 90.9 MB | 4.3 MB | 1.0 ms | 0.9 ms | 15.8 MB |
| collatz | 76.4 ms | 90.4 ms | 91.9 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| sieve | 97.0 ms | 112.8 ms | 107.1 MB | 4.3 MB | 1.0 ms | 1.0 ms | 15.8 MB |
| matmul | 121.4 ms | 136.7 ms | 107.2 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| quicksort | 100.8 ms | 115.2 ms | 107.2 MB | 4.3 MB | 1.1 ms | 1.1 ms | 15.8 MB |
| strings | 117.3 ms | 133.0 ms | 112.6 MB | 4.3 MB | 1.1 ms | 0.9 ms | 15.8 MB |

`minirust front-end` is lexing + parsing + bytecode generation, measured through `rust --compile-only`.

## Runtime phase (per program)

| program | native wall | native CPU | native CPU% | native RSS | minirust (JIT) wall | CPU | CPU% | RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.9 ms | 0.8 ms | 92% | 15.5 MB | 1.0 ms | 0.9 ms | 92% | 15.5 MB | 1.1x |
| fib | 3.0 ms | 2.9 ms | 97% | 15.6 MB | 9.3 ms | 9.1 ms | 99% | 15.6 MB | 3.1x |
| loop_sum | 3.6 ms | 3.5 ms | 97% | 15.8 MB | 14.2 ms | 14.1 ms | 99% | 15.8 MB | 4.0x |
| collatz | 22.5 ms | 22.3 ms | 99% | 15.8 MB | 70.9 ms | 70.7 ms | 100% | 15.8 MB | 3.2x |
| sieve | 2.3 ms | 2.2 ms | 96% | 15.8 MB | 221.3 ms | 221.1 ms | 100% | 18.0 MB | 95.3x |
| matmul | 1.8 ms | 1.8 ms | 96% | 15.8 MB | 160.9 ms | 160.7 ms | 100% | 15.8 MB | 87.2x |
| quicksort | 4.0 ms | 3.9 ms | 98% | 15.8 MB | 206.3 ms | 206.1 ms | 100% | 15.8 MB | 51.4x |
| strings | 1.2 ms | 1.1 ms | 94% | 15.8 MB | 12.8 ms | 12.7 ms | 99% | 15.8 MB | 10.8x |

## JIT effect (same binary, `--no-jit` vs JIT)

| program | interpreter only | with JIT | JIT speedup | native | JIT vs native |
|---|---:|---:|---:|---:|---:|
| hello | 1.0 ms | 1.0 ms | 0.97x | 0.9 ms | 1.1x |
| fib | 193.0 ms | 9.3 ms | 20.85x | 3.0 ms | 3.1x |
| loop_sum | 935.2 ms | 14.2 ms | 65.91x | 3.6 ms | 4.0x |
| collatz | 1912.9 ms | 70.9 ms | 26.97x | 22.5 ms | 3.2x |
| sieve | 215.3 ms | 221.3 ms | 0.97x | 2.3 ms | 95.3x |
| matmul | 162.8 ms | 160.9 ms | 1.01x | 1.8 ms | 87.2x |
| quicksort | 204.4 ms | 206.3 ms | 0.99x | 4.0 ms | 51.4x |
| strings | 12.6 ms | 12.8 ms | 0.98x | 1.2 ms | 10.8x |

The JIT only covers functions whose parameters, locals, calls and return value are all integers (no division, no methods, no Vec/String). Everything else keeps running in the interpreter.


## End-to-end (source -> output)

| program | rustc compile + run | minirust front-end + run | ratio |
|---|---:|---:|---:|
| hello | 73.3 ms | 2.0 ms | 1/36.4 faster |
| fib | 78.2 ms | 10.3 ms | 1/7.6 faster |
| loop_sum | 79.9 ms | 15.2 ms | 1/5.3 faster |
| collatz | 98.9 ms | 72.0 ms | 1/1.4 faster |
| sieve | 99.3 ms | 222.4 ms | 2.24x |
| matmul | 123.3 ms | 161.9 ms | 1.31x |
| quicksort | 104.8 ms | 207.5 ms | 1.98x |
| strings | 118.5 ms | 13.9 ms | 1/8.5 faster |

Geometric-mean runtime slowdown across the 8 programs: **10.9x** with the JIT, **40.4x** interpreter-only.

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
