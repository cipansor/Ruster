# minirust benchmark results

- Date: 2026-11-26T13:55:44
- Host: Linux-6.8.0-1055-gke-x86_64-with-glibc2.41 (4 CPUs)
- rustc: `rustc 1.100.0 (0ef1bc386 2026-11-09)`
- Interpreter: single binary, 4.8 MB
- One-time interpreter build (`rustc -O`, no cargo): 1886.6 ms wall, 5206.2 ms CPU, 239 MB RSS
- Each program: 5 compile runs, 5 run repetitions; median wall/CPU, max peak RSS.

## Compile phase (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | native binary | minirust front-end wall | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 76.0 ms | 89.3 ms | 94.6 MB | 4.3 MB | 1.0 ms | 0.9 ms | 15.6 MB |
| fib | 76.3 ms | 91.9 ms | 94.8 MB | 4.3 MB | 1.0 ms | 1.0 ms | 15.6 MB |
| loop_sum | 78.6 ms | 92.9 ms | 90.6 MB | 4.3 MB | 1.2 ms | 1.1 ms | 15.8 MB |
| sieve | 95.4 ms | 110.3 ms | 106.9 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| matmul | 124.5 ms | 140.3 ms | 107.4 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| quicksort | 102.6 ms | 118.6 ms | 107.3 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| strings | 118.5 ms | 134.0 ms | 111.3 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.9 MB |

`minirust front-end` is lexing + parsing + bytecode generation, measured through `rust --compile-only`.

## Runtime phase (per program)

| program | native wall | native CPU | native CPU% | native RSS | minirust wall | minirust CPU | minirust CPU% | minirust RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.8 ms | 0.8 ms | 90% | 15.6 MB | 1.0 ms | 0.9 ms | 91% | 15.6 MB | 1.2x |
| fib | 3.0 ms | 2.9 ms | 97% | 15.6 MB | 233.4 ms | 233.1 ms | 100% | 15.6 MB | 78.0x |
| loop_sum | 8.6 ms | 8.5 ms | 99% | 15.6 MB | 1645.0 ms | 1644.6 ms | 100% | 15.8 MB | 191.5x |
| sieve | 2.3 ms | 2.2 ms | 97% | 15.8 MB | 380.5 ms | 380.3 ms | 100% | 18.0 MB | 164.0x |
| matmul | 2.2 ms | 2.1 ms | 96% | 15.8 MB | 254.4 ms | 254.0 ms | 100% | 15.8 MB | 115.7x |
| quicksort | 4.0 ms | 3.9 ms | 98% | 15.8 MB | 258.1 ms | 257.9 ms | 100% | 15.8 MB | 64.7x |
| strings | 1.2 ms | 1.1 ms | 94% | 15.8 MB | 18.7 ms | 18.5 ms | 99% | 15.9 MB | 15.8x |

## End-to-end (source -> output)

| program | rustc compile + run | minirust front-end + run | ratio |
|---|---:|---:|---:|
| hello | 76.8 ms | 2.0 ms | 1/38.3 faster |
| fib | 79.3 ms | 234.4 ms | 2.95x |
| loop_sum | 87.2 ms | 1646.2 ms | 18.88x |
| sieve | 97.8 ms | 381.6 ms | 3.90x |
| matmul | 126.7 ms | 255.5 ms | 2.02x |
| quicksort | 106.6 ms | 259.2 ms | 2.43x |
| strings | 119.6 ms | 19.7 ms | 1/6.1 faster |

Geometric-mean runtime slowdown across the 7 programs: **44.5x**.

All programs produced identical output and exit code under both engines (True).

## Binary / source size

| program | source | native binary | interpreter binary | native/source |
|---|---:|---:|---:|---:|
| hello | 62 B | 4.3 MB | 4.8 MB | 72458.7x |
| fib | 156 B | 4.3 MB | 4.8 MB | 28802.1x |
| loop_sum | 196 B | 4.3 MB | 4.8 MB | 22924.1x |
| sieve | 562 B | 4.3 MB | 4.8 MB | 7995.6x |
| matmul | 718 B | 4.3 MB | 4.8 MB | 6263.1x |
| quicksort | 948 B | 4.3 MB | 4.8 MB | 4741.1x |
| strings | 349 B | 4.3 MB | 4.8 MB | 12884.4x |
