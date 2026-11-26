# minirust benchmark results

- Date: 2026-11-26T14:24:41
- Host: Linux-6.8.0-1055-gke-x86_64-with-glibc2.41 (4 CPUs)
- rustc: `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- Interpreter: single binary, 4.9 MB
- One-time interpreter build (`rustc -O`, no cargo): 2443.0 ms wall, 6766.9 ms CPU, 251 MB RSS
- Each program: 5 compile runs, 5 run repetitions; median wall/CPU, max peak RSS.

## Compile phase (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | native binary | minirust front-end wall | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 73.8 ms | 87.6 ms | 89.6 MB | 4.3 MB | 1.0 ms | 0.9 ms | 16.0 MB |
| fib | 75.9 ms | 90.4 ms | 90.5 MB | 4.3 MB | 1.0 ms | 1.0 ms | 16.0 MB |
| loop_sum | 76.9 ms | 91.6 ms | 90.9 MB | 4.3 MB | 1.1 ms | 1.0 ms | 16.0 MB |
| collatz | 76.8 ms | 90.9 ms | 92.1 MB | 4.3 MB | 1.1 ms | 1.0 ms | 16.0 MB |
| sieve | 94.7 ms | 110.0 ms | 106.9 MB | 4.3 MB | 1.1 ms | 1.0 ms | 16.0 MB |
| matmul | 125.0 ms | 139.3 ms | 107.6 MB | 4.3 MB | 1.2 ms | 1.1 ms | 16.0 MB |
| quicksort | 103.1 ms | 116.6 ms | 107.1 MB | 4.3 MB | 1.2 ms | 1.1 ms | 16.0 MB |
| strings | 118.2 ms | 134.1 ms | 111.3 MB | 4.3 MB | 1.1 ms | 1.0 ms | 16.1 MB |

`minirust front-end` is lexing + parsing + bytecode generation, measured through `rust --compile-only`.

## Runtime phase (per program)

| program | native wall | native CPU | native CPU% | native RSS | minirust (JIT) wall | CPU | CPU% | RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 1.0 ms | 0.9 ms | 88% | 16.0 MB | 1.0 ms | 0.9 ms | 93% | 16.0 MB | 1.0x |
| fib | 3.1 ms | 3.0 ms | 97% | 16.0 MB | 9.5 ms | 9.3 ms | 98% | 16.0 MB | 3.1x |
| loop_sum | 3.6 ms | 3.5 ms | 98% | 16.0 MB | 14.0 ms | 13.9 ms | 99% | 16.0 MB | 3.9x |
| collatz | 22.6 ms | 22.4 ms | 99% | 16.0 MB | 70.4 ms | 70.2 ms | 100% | 16.0 MB | 3.1x |
| sieve | 2.3 ms | 2.2 ms | 96% | 16.0 MB | 217.7 ms | 217.4 ms | 100% | 18.0 MB | 93.3x |
| matmul | 1.8 ms | 1.8 ms | 96% | 16.0 MB | 171.1 ms | 170.8 ms | 100% | 16.0 MB | 93.2x |
| quicksort | 4.0 ms | 4.0 ms | 98% | 16.0 MB | 214.1 ms | 213.9 ms | 100% | 16.0 MB | 52.9x |
| strings | 1.2 ms | 1.1 ms | 93% | 16.0 MB | 12.3 ms | 12.2 ms | 99% | 16.0 MB | 10.2x |

## JIT effect (same binary, `--no-jit` vs JIT)

| program | interpreter only | with JIT | JIT speedup | native | JIT vs native |
|---|---:|---:|---:|---:|---:|
| hello | 1.0 ms | 1.0 ms | 1.05x | 1.0 ms | 1.0x |
| fib | 189.7 ms | 9.5 ms | 19.97x | 3.1 ms | 3.1x |
| loop_sum | 947.2 ms | 14.0 ms | 67.53x | 3.6 ms | 3.9x |
| collatz | 1906.3 ms | 70.4 ms | 27.07x | 22.6 ms | 3.1x |
| sieve | 221.4 ms | 217.7 ms | 1.02x | 2.3 ms | 93.3x |
| matmul | 170.7 ms | 171.1 ms | 1.00x | 1.8 ms | 93.2x |
| quicksort | 212.8 ms | 214.1 ms | 0.99x | 4.0 ms | 52.9x |
| strings | 12.5 ms | 12.3 ms | 1.01x | 1.2 ms | 10.2x |

The JIT only covers functions whose parameters, locals, calls and return value are all integers (no division, no methods, no Vec/String). Everything else keeps running in the interpreter.


## End-to-end (source -> output)

| program | rustc compile + run | minirust front-end + run | ratio |
|---|---:|---:|---:|
| hello | 74.8 ms | 2.0 ms | 1/37.1 faster |
| fib | 79.0 ms | 10.5 ms | 1/7.5 faster |
| loop_sum | 80.5 ms | 15.1 ms | 1/5.3 faster |
| collatz | 99.4 ms | 71.5 ms | 1/1.4 faster |
| sieve | 97.1 ms | 218.8 ms | 2.25x |
| matmul | 126.8 ms | 172.3 ms | 1.36x |
| quicksort | 107.2 ms | 215.2 ms | 2.01x |
| strings | 119.4 ms | 13.4 ms | 1/8.9 faster |

Geometric-mean runtime slowdown across the 8 programs: **10.7x** with the JIT, **40.2x** interpreter-only.

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
