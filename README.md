# Ruster — a Rust interpreter and x86-64 JIT in one binary, without Cargo

[![CI](https://github.com/cipansor/Ruster/actions/workflows/ci.yml/badge.svg)](https://github.com/cipansor/Ruster/actions/workflows/ci.yml)

> **Ruster** is short for **Rust Interpreter**. It is the project's name, not
> the name of the language being interpreted.

An experiment: how close can a from-scratch Rust implementation get to native
Rust speed without LLVM, without external crates, and without Cargo — and what
happens when we add our own JIT on top?

The single binary now contains two engines:

1. **A bytecode interpreter** that runs the whole language subset.
2. **A baseline x86-64 JIT** that emits machine code for functions whose
   parameters, locals, calls, and return values are all integers.

Representative results (8 benchmark programs, 5 runs, median):

| Metric | Result |
|---|---|
| Geometric-mean slowdown, interpreter only | **40.3x** vs native |
| Geometric-mean slowdown, with the JIT | **15.9x** vs native |
| JIT-eligible integer kernels (10M loop, collatz) | **3.1x–3.9x** vs native |
| Recursive `fib` (not JIT-eligible) | 64x vs native (runs in the interpreter) |
| `Vec`/index-heavy code (sieve, matmul, quicksort) | ~52x–85x vs native |
| "Compile" phase (lex + parse + bytecode) | **~1 ms**, ~16 MB RSS (vs 74–126 ms, 90–111 MB for `rustc -O`) |
| Size | 5.0 MB for one binary that runs every program (native: 4.3 MB per program) |

The JIT closes the gap from ~40x to ~15.9x, but only for functions it can
compile. Directly or mutually recursive functions (such as `fib`) are excluded
because generated code has no native stack-depth guard, and `Vec`/`String`-heavy
code is not covered yet; both keep running in the interpreter. For the two
JIT-eligible integer kernels we are at **3–4x native** — not native, but much
closer than any interpreter I had written before.

## Table of contents

1. [Build and usage](#1-build-and-usage)
2. [Architecture](#2-architecture)
3. [Supported language subset](#3-supported-language-subset)
4. [Benchmark results](#4-benchmark-results)
5. [Analysis: where does the gap come from?](#5-analysis-where-does-the-gap-come-from)
6. [Semantic deviations](#6-semantic-deviations)
7. [Bytecode example](#7-bytecode-example)
8. [Repository layout](#8-repository-layout)
9. [Roadmap and limitations](#9-roadmap-and-limitations)

---

## 1. Build and usage

No Cargo required. All you need is `rustc`. Ruster uses the **Rust 2024
edition**, so it needs rustc **1.85 or newer** (the latest stable is
recommended):

```bash
rustc --edition 2024 -O ruster/src/main.rs -o bin/rust
```

Then:

```bash
$ bin/rust hello.rs
Hello, world! 1 + 1 = 2

$ bin/rust hello                 # the .rs extension is tried automatically
Hello, world! 1 + 1 = 2

$ bin/rust --stats hello.rs
Hello, world! 1 + 1 = 2
Ruster stats: front-end 0.019 ms, run 0.017 ms
Ruster jit: no functions were eligible

$ bin/rust --stats bench/programs/fib.rs
fib(30) = 832040
Ruster stats: front-end 0.081 ms, run 194.586 ms
Ruster jit: no functions were eligible

$ bin/rust --compile-only hello.rs
Ruster front-end: 0.019 ms | 1 function(s), 7 instruction(s), 2 constant(s), 1 template(s), 0 global(s)

$ bin/rust --dump bench/programs/fib.rs
fn fib [0] params=1 locals=1
  0000  CmpImmJump { a: 0, imm: 2, op: Lt, target: 3, jump_if: false }
  ...
```

Other options: `--no-jit` (force the interpreter only), `--version`, `--help`.
The environment variable `MINIRUST_NO_JIT=1` does the same as `--no-jit`.

To invoke it as `rust hello.rs`:

```bash
install -m755 bin/rust ~/.local/bin/rust
```

There are no runtime dependencies and no companion files.

### Running the tests

```bash
# 18 feature programs: rustc vs Ruster (JIT) vs Ruster --no-jit
python3 tests/compare.py

# 4 error cases: exit code 101 (panic) / 3 (process::exit)
python3 tests/errors.py

# Assert that recursive call cycles stay in the interpreter (x86-64 only)
python3 tests/jit_fallback.py

# Assert the benchmark's end-to-end time does not double-count the front-end
python3 tests/bench_math.py
```

`compare.py` runs all three engines and requires identical stdout and exit
codes. That cross-validates the JIT path against the interpreter path, not just
against `rustc`. `jit_fallback.py` reads the JIT function list from `--stats`
and checks that direct and mutual recursion are never JIT-compiled while an
acyclic function still is; it also asserts that the benchmark's `fib` stays
interpreted and that `loop_sum`/`collatz` compile. `bench_math.py` checks that
the benchmark reporters use the full `bin/rust <file>` invocation as the
end-to-end time, so the front-end is not counted twice.

### Running the benchmarks

```bash
python3 bench/bench.py --runs 5 --compile-runs 5
# writes bench/results.md and bench/results.json

# a subset: an unmatched name is rejected before anything is written
python3 bench/bench.py --only hello,fib --runs 5
```

An unmatched `--only` exits before building or overwriting the reports, so a
typo cannot replace the saved results with an empty run.

The harness uses Python only for measurement (`fork`/`exec` + `os.wait4`); it is
not part of the runtime. Each program is measured three ways: native, Ruster
with the JIT, and Ruster with `--no-jit`.

---

## 2. Architecture

```
.rs source
   |
   v
lexer.rs     -> tokens
   |
   v
parser.rs    -> AST
   |
   v
compiler.rs  -> stack-based bytecode + const eval + peephole optimizer
   |
   +--> jit.rs   (pure integer functions) -> x86-64 machine code
   |
   v
vm.rs        -> frame-stack interpreter; calls JIT-compiled code when possible
```

### Interpreter

- Stack-based bytecode machine; locals are slot indices, not name lookups.
- An explicit frame stack (`Vec<Frame>` plus a flat `Vec<Value>`), so 100,000
  levels of recursion run without touching the host stack.
- A peephole optimizer fuses common patterns:
  - `Load a; Load b; Bin op; Store c` -> `BinLocals`
  - `Load a; Imm k; Bin op; Store c` -> `BinLocalImm`
  - `Load a; Load b; Bin op` -> `PushBinLocals`
  - `Load a; Imm k; Bin op` -> `PushBinLocalImm`
  - `Imm k; Bin op` -> `TopBinImm`
  - `Bin op; Store d` -> `BinStore`
  - `Imm a; Imm b; Bin op` -> `FoldImm` (constant folding)
  - compare + jump -> `CmpJump` / `CmpImmJump`
  - `Load v; Load i; Index` -> `IndexL` (the `as usize` cast is folded in)
  - `Load v; Load i; Load x; IndexSet` -> `IndexSetLL` (plus `LI`/`LC` variants)
  - `v[i] op= x` -> `IndexRmw` (one dispatch instead of seven)
  - `v[i] = v[j]` -> `IndexSetFromIndex` (six opcodes become one)
  - `Load src; Store dst` -> `CopyLocal`
  - `Unit; Pop` -> removed (unless the `Pop` is a jump merge point)
- Every fusion checks whether a jump targets an instruction inside the pattern;
  if so, the fusion is skipped. This matters: an early bug made `if/else` lose
  the `Pop` belonging to one branch.

### x86-64 JIT (`jit.rs`, ~760 lines, no crates)

Every encoder is written by hand; there is no Cranelift or LLVM.

**Eligibility (checked per function, with call cycles excluded):**

- all parameters and the return type are integer types;
- at most 6 parameters and 5 locals;
- not on a call cycle (direct or mutual recursion): generated code has no
  native stack-depth guard;
- the bytecode contains only `Imm`, `Load`, `Store`, `Pop`, `FoldImm`, `Bin`
  (`+ - * & | ^ << >> == != < <= > >=`), `Un(Neg)`, all jump/fusion variants,
  `Call` to another eligible function, and `Return`;
- no `Div`/`Rem` (they need a division-by-zero check), no methods, `Vec`,
  `String`, globals, or `Unit` (the function must return an integer).

**Code generation:**

- Locals map to callee-saved registers: `rbx, r12, r13, r14, r15`. SysV
  parameters (`rdi, rsi, rdx, rcx, r8, r9`) are copied into the local registers
  in the prologue.
- The operand stack stays in the frame (`[rbp-8*(d+1)]`), so temporary
  operations do not clobber the local registers.
- Comparisons use `cmp` + `setcc` + `movzx`; branches use `jcc rel32`, patched
  once all offsets are known.
- Calls take arguments from the operand stack into SysV registers, then
  `mov rax, <ptr>; call rax`. Function addresses are patched after every page is
  allocated, so cross-calls work.
- Frames are always 16-byte aligned, so calls obey the SysV ABI.
- Code is written to anonymous `mmap` pages, then `mprotect(PROT_READ|PROT_EXEC)`
  after finalization (W^X). No page stays writable and executable.
- If a call from the interpreter has a non-`Int` argument, the VM falls back to
  the interpreter for that call (per call, not per program).

Call cycles (direct or mutual recursion) are detected with one Tarjan SCC pass
and excluded from the JIT, because generated code has no native stack-depth
guard.

### `Vec` representation (packed)

`Value::Vec` stores an `Rc<RefCell<VecData>>`, and `VecData` has three modes:
generic `Vec<Value>`, packed `Vec<i64>`, or packed `Vec<bool>`.
`vec![0; n]` / `vec![true; n]` create a scalar buffer directly, so indexing no
longer touches 16-byte `Value`s. Methods without a packed path (`sort`,
`insert`, ...) convert the contents of the same cell to `Vec<Value>` once, so
`Rc` identity and mutation stay correct. Measured effect: sieve 1M RSS drops
from 18.0 MB to 15.6 MB, and the index loop gets slightly faster.

Consequence: JIT-eligible integer kernels (such as `loop_sum` and `collatz`)
run as machine code; `fib` (recursive) and `Vec`/index-heavy code
(sieve, matmul, quicksort) stay in the interpreter.

---

## 3. Supported language subset

| Category | Supported |
|---|---|
| Items | `fn`, `const`, `static`/`static mut`, inline `mod { ... }`, `use` (ignored) |
| Types | `i8..i64`, `u8..u64`, `usize`/`isize`, `f32`/`f64`, `bool`, `char`, `String`, `&str`, `Vec<T>`, arrays `[T; N]`, tuples, references `&T`/`&mut T` |
| Statements | `let` (+`mut`, type annotations, tuple destructuring), blocks, `if`/`else if`/`else`, `while`, `loop`, `for i in a..b`/`a..=b`, `for x in vec`, `break`/`continue`, `loop` as an expression + `break value` |
| Expressions | arithmetic, bitwise, comparison, short-circuit `&&`/`||`, assignment and compound assignment, `as`, `&`/`&mut`/`*`, indexing `v[i]`, tuple `.0`, method calls, macros |
| Functions | recursion, parameters, return; top-level functions and functions inside inline `mod` |
| Macros | `println!`, `print!`, `eprintln!`, `eprint!`, `format!`, `vec![]`, `vec![x; n]`, `panic!`, `assert!`, `assert_eq!`, `assert_ne!`, `dbg!`, `unreachable!`/`todo!`/`unimplemented!` |
| Methods | `len`, `is_empty`, `push`, `push_str`, `pop`, `clear`, `insert`, `remove`, `reverse`, `sort`, `contains`, `first`, `last`, `join`, `trim*`, `starts_with`, `ends_with`, `replace`, `repeat`, `to_uppercase`, `to_lowercase`, `split`, `split_whitespace`, `chars`, `abs`, `pow`, `powf`, `powi`, `sqrt`, `floor`, `ceil`, `round`, `min`, `max`, `clamp`, `signum`, `wrapping_*`, `saturating_*`, `to_string`, `clone`, `.iter()/.into_iter()/.copied()/.as_str()` |
| Associated functions | `String::new/from/with_capacity`, `Vec::new/with_capacity`, `std::process::exit` |
| Path constants | `i64::MAX/MIN`, `u8::MAX`, `f64::PI/E/INFINITY/NAN/...` |

**Not supported:** `struct`, `enum`, `impl`, `trait`, closures, iterator
adapters (`map`/`filter`/`fold`), `Option`/`Result` and `?`, `match`, slicing,
`HashMap`, the borrow checker, lifetimes, real generics, and separate module
files.

**The JIT subset** is much smaller than the interpreter subset: integer
functions only, no division, no `Vec`/`String` access, at most 5 locals.

---

## 4. Benchmark results

Environment: Linux x86-64, 4-vCPU container, `rustc 1.100.0`, `-O`. Each
program: 5 compilations and 5 executions; median wall/CPU, maximum peak RSS.

The JIT only covers functions whose parameters, locals, calls, and return value
are all integers (no division, no methods, no `Vec`/`String`). In the benchmark
set that is `loop_sum::work` and `collatz::steps`; `fib` is directly recursive
and is therefore excluded, so it runs in the interpreter even with the JIT on.

### 4.1 Compile phase (per program)

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

"Front-end" means lexing + parsing + bytecode generation + `const` evaluation,
measured through `rust --compile-only`. No native code is produced at this
stage.

Building the interpreter once: `rustc -O` = **2.8 s wall, 7.7 s CPU
(parallel), 262 MB RSS**.

### 4.2 Runtime phase (with the JIT)

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

Interpreter CPU usage is ~99–100% (single-threaded). `rustc` can exceed 100%
because it compiles in parallel.

### 4.3 Effect of the JIT (same binary, `--no-jit` vs JIT)

| program | interpreter only | with JIT | JIT speedup | native | JIT vs native |
|---|---:|---:|---:|---:|---:|
| hello | 1.0 ms | 1.0 ms | 0.97x | 0.9 ms | 1.2x |
| fib | 192.7 ms | 192.9 ms | **1.00x** | 3.0 ms | 64.4x |
| loop_sum | 942.4 ms | 14.2 ms | 66.25x | 3.6 ms | 3.9x |
| collatz | 1917.6 ms | 70.4 ms | 27.25x | 22.8 ms | 3.1x |
| sieve | 192.0 ms | 191.7 ms | 1.00x | 2.4 ms | 81.1x |
| matmul | 159.4 ms | 159.7 ms | 1.00x | 1.9 ms | 84.8x |
| quicksort | 214.1 ms | 213.9 ms | 1.00x | 4.1 ms | 51.9x |
| strings | 14.6 ms | 14.5 ms | 1.01x | 1.2 ms | 12.1x |

`fib` shows **1.00x** because it is recursive and therefore not JIT-compiled:
its "with JIT" time is the interpreter time. Only `loop_sum` (66x) and `collatz`
(27x) benefit. The JIT never changes observable behaviour; it only speeds up
functions it is allowed to compile.

### 4.4 End-to-end (source -> output)

| program | rustc compile + run | Ruster source -> output | Ruster vs rustc |
|---|---:|---:|---:|
| hello | 74.9 ms | 1.0 ms | 73.9x faster |
| fib | 79.6 ms | 192.9 ms | 2.42x slower |
| loop_sum | 82.4 ms | 14.2 ms | 5.8x faster |
| collatz | 102.3 ms | 70.4 ms | 1.5x faster |
| sieve | 99.1 ms | 191.7 ms | 1.93x slower |
| matmul | 127.5 ms | 159.7 ms | 1.25x slower |
| quicksort | 108.3 ms | 213.9 ms | 1.98x slower |
| strings | 120.2 ms | 14.5 ms | 8.3x faster |

`Ruster source -> output` is a single `bin/rust <file>` invocation: the same
process lexes, parses, compiles, and runs the program. The separate front-end
measurement in 4.1 is for analysis only and is **not** added again here.

**Geometric-mean runtime slowdown: 15.9x with the JIT, 40.3x without.** On
end-to-end time (compile + run) Ruster is about **2.1x faster than `rustc` on
the geometric mean**, winning 4 of 8 programs.

Because `rustc -O` takes 74–126 ms per program, Ruster wins end-to-end for
short-running programs — including the 10-million-iteration loop and the string
benchmark. It loses on `fib` (interpreter-only) and on `Vec`/index-heavy code.

The harness enforces two independent checks: the captured comparison (native,
JIT, and `--no-jit` must print the same stdout and exit with the same code) and
the timed runs (every measured run must exit 0). A failure in either check
fails the run, but CI still publishes the report and the per-program summary;
timings from failed timed runs are excluded from the aggregates.

### 4.5 Size and memory

| Aspect | native | Ruster |
|---|---:|---:|
| hello binary | 4.3 MB | 5.0 MB (one binary for every program) |
| quicksort binary | 4.3 MB | 5.0 MB |
| RSS during compilation | 90–111 MB | ~16 MB |
| Runtime RSS (hello) | 15.5 MB | 15.5 MB |
| Runtime RSS (sieve, 1M bools) | 15.9 MB | 15.9 MB |

Note: `Vec<bool>` in Ruster stores one 16-byte `Value` per element, not 1 bit or
1 byte as in Rust. The native 4.3 MB is mostly statically linked Rust std;
Ruster's 5.0 MB is also mostly std, but one binary runs every program without
per-program compilation.

---

## 5. Analysis: where does the gap come from?

### 5.1 Interpreter (40.3x)

The classic causes: per-instruction dispatch, 16-byte tagged `Value`s, no
register allocation, no inlining or vectorization, and `Rc`/`RefCell` for
`Vec`/`String`. The 10-million-iteration `loop_sum` takes ~942 ms
(≈94 ns/iteration) because each iteration executes ~7 bytecodes at ~10 ns each.

### 5.2 JIT-eligible integer kernels (3.1–3.9x)

The JIT removes dispatch and tag checks: locals live in registers, arithmetic
is direct `add/sub/imul`, and branches are `cmp+jcc`. For the two eligible
kernels it is 27–66x faster than the interpreter.

The remaining 3–4x comes from:

- **The operand stack is still in memory.** Every expression writes to and
  reads from a stack slot instead of a register.
- **No register allocation for temporaries**, no common-subexpression
  elimination, and no inlining.
- **No vectorization.** Native `loop_sum` takes 3.6 ms for 10 million iterations
  (≈0.36 ns/iteration) because LLVM vectorizes it; our JIT takes 14.2 ms
  (≈1.4 ns/iteration) — only ~4x slower, but still not SIMD.
- **Calls between JIT-compiled functions are not inlined**, and **recursive
  functions are excluded entirely** (no native stack-depth guard). This is why
  `fib`, despite being integer-only, runs in the interpreter: it is directly
  recursive.

For the eligible integer kernels this is already "close to native" in a
meaningful sense: 3x, not 100x. `fib` remains the clearest example of what the
recursion exclusion costs.

### 5.3 `Vec`/`String` code (~52x–85x)

Sieve, matmul, and quicksort spend their time in `Op::Index`/`Op::IndexSet`:
checking the `Value::Vec(Rc<RefCell<Vec<Value>>>)` tag, dynamic borrow, bounds
check, `Value` clone, then arithmetic. The JIT deliberately does not handle this
yet because it needs a boxed `Value` ABI and runtime helpers.

Index fusion (`IndexL`, `IndexSetLL/LI/LC`, `IndexJumpL`, `IndexRmw`,
`IndexSetFromIndex`) removes 2–5 dispatches per index operation: sieve 309 ->
218 ms and matmul 227 -> 171 ms. The next limit is the cost of a 16-byte
`Value` + `RefCell` per element, which can only be avoided with a dedicated
`Vec<i64>` representation or a JIT helper.

This is **the biggest remaining optimization opportunity**:

1. Add JIT opcodes for `Index`/`IndexSet` with inline caches (the
   `Rc<RefCell<Vec<Value>>>` layout plus one tag check per loop), or
2. Build a second JIT with a `Value*` ABI and call helpers for non-integer
   operations, or
3. Specialize `Vec<i64>` in the interpreter (a `Vec<i64>` buffer plus a special
   tag) so indexing no longer touches `Value`.

A realistic estimate: 5–20x faster for sieve/matmul/quicksort, which would bring
the geometric mean to the 2–4x-native range.

### 5.4 Why not compile to native?

Because `rustc -O` is already the best native compiler: 73–125 ms per program,
then full speed. Ruster only wins when that compilation cost is larger than the
runtime savings — that is, for programs that finish in a few milliseconds. The
JIT is a way to get part of native speed without paying compilation up front;
AOT (emitting native code and caching it) would remove JIT startup but add cache
complexity.

### 5.5 Complexity audit: linear vs quadratic

Time measured as a function of input size (1x, 2x, 4x, 8x); a ratio of ~2.0 per
doubling means linear.

| path | 1x | 2x | 4x | 8x | ratio | status |
|---|---:|---:|---:|---:|---|---|
| `let` + local lookup (compile) | 1.5 ms | 2.6 ms | 5.4 ms | 10.4 ms | 1.8–2.1 | linear |
| long expression chain (compile) | 0.9 ms | 1.9 ms | 3.9 ms | 7.8 ms | 2.0 | linear |
| reversed `const` chain (compile) | 2.5 ms | 4.9 ms | 9.9 ms | 19.7 ms | 2.0 | linear* |
| integer loop (interpreter) | 8.8 ms | 16.9 ms | 34.1 ms | — | 1.9–2.0 | linear |
| `Vec::push` (interpreter) | 17.5 ms | 34.1 ms | 76.8 ms | — | 2.0–2.3 | linear |
| `String::push_str` (interpreter) | 15.6 ms | 29.6 ms | 59.5 ms | — | 1.9–2.0 | linear |

\*Before the fix: 500/1000/2000 consts = 17.8/94.0/264.6 ms (quadratic).

Two quadratic behaviours were found and fixed:

1. **Local lookup** (`resolve_local`) scanned the whole local list for every
   identifier, so functions with many `let`s became O(n²) during compilation.
   There is now a `name -> stack slot` index (a stack, for shadowing);
   `scope_exit` pops the slots that leave scope.
2. **`const` resolution** used a "retry everything unfinished" loop, so a chain
   written in reverse (`const A = B + 1; const B = 1;`) needed O(n) passes. It
   is now recursive on-demand resolution with memoization and a `HashSet` for
   cycle detection: linear.

Audited and found reasonable: `sort` uses `sort_by` (O(n log n)), `push_str` is
amortized O(1), the other string methods delegate to `std`, and
`Vec::insert/remove` are O(n) per operation (the same as Rust; using them in a
loop is O(n²) by data-structure design, not an implementation bug).

---

## 6. Semantic deviations

| Aspect | Rust | Ruster |
|---|---|---|
| Integer width | `i8`/`u8`/... differ | all stored as `i64`; overflow wraps |
| `u64` > `i64::MAX` | large value | negative bit pattern; comparisons can be wrong |
| `f32` | 32-bit | stored as `f64` |
| Overflow | panic (debug) / wrap (release) | always wraps (interpreter and JIT) |
| Shift >= width | panic (debug) | masked (`& 63`) |
| Division by zero | panic | panic (exit 101); not supported by the JIT |
| `Vec::pop()` | `Option<T>` | direct value / `()` if empty |
| `first()`/`last()` | `Option<&T>` | value / `()` |
| Borrow/move | checked by the compiler | none; `let b = a;` shares the `Rc` |
| `static mut` | requires `unsafe` | allowed anywhere |
| Struct/enum/match/closure | available | not available |

The JIT preserves wrapping semantics and signed `i64` comparisons; programs that
use features outside the JIT subset keep running in the interpreter.

---

## 7. Bytecode example

```text
$ bin/rust --dump bench/programs/fib.rs
fn fib [0] params=1 locals=1
  0000  CmpImmJump { a: 0, imm: 2, op: Lt, target: 3, jump_if: false }
  0001  Load(0)
  0002  Jump(8)
  0003  PushBinLocalImm { a: 0, imm: 1, op: Sub }
  0004  Call(0)
  0005  PushBinLocalImm { a: 0, imm: 2, op: Sub }
  0006  Call(0)
  0007  Bin(Add)
  0008  Return
```

Nine instructions for all of `fib`. Without fusion, the naive version is ~20
instructions. `CmpImmJump` is a fused `Load; Imm; Bin(Lt); JumpF`;
`PushBinLocalImm` is a fused `Load; Imm; Bin` that pushes the result directly.

---

## 8. Repository layout

```
ruster/src/
  lexer.rs      tokenizer
  ast.rs        AST + types
  parser.rs     recursive descent
  compiler.rs   AST -> bytecode, const eval, peephole
  ops.rs        arithmetic/cast semantics (VM and const eval)
  value.rs      Value, comparisons, Rust-like formatting
  vm.rs         frame-stack interpreter + method dispatch + JIT fallback
  jit.rs        baseline x86-64 JIT (encoder, mmap, eligibility)
  main.rs       CLI
bin/rust        build output (gitignored)
bench/
  programs/*.rs 8 benchmark programs
  bench.py      harness (wall, CPU, peak RSS, JIT vs --no-jit)
  results.md    latest results
  results.json  raw data
tests/
  programs/*.rs 18 feature programs (compared with rustc, JIT, and no-JIT)
  errors/*.rs   4 error cases
  compare.py    three-engine differential runner
  errors.py     exit-code checks
  jit_fallback.py  asserts recursive cycles stay interpreted
  bench_math.py    guards benchmark end-to-end arithmetic
```

Source size: ~6,300 lines of Rust (compiler 2,009, parser 945, vm 872, jit 761,
value 579, lexer 407, ops 275, ast 206, main 209).

---

## 9. Roadmap and limitations

1. **JIT for `Vec`/array indexing** (biggest impact: sieve, matmul, quicksort).
   Inline caches for the `Vec<i64>` type, or a boxed `Value` ABI with runtime
   helpers.
2. **JIT support for recursion**: add a native stack-depth guard (or a
   trampoline) so recursive functions such as `fib` can be compiled; today every
   call cycle stays in the interpreter.
3. **Register allocation for the operand stack** in the JIT (remove
   per-expression memory traffic), then inline small functions.
4. **Loop superinstructions** (range `for` -> one opcode) and strength
   reduction.
5. **JIT for `f64`** (SSE2 arithmetic) so float benchmarks get faster too.
6. **AOT cache**: save bytecode/machine code to disk; the front end is already
   ~1 ms, so this matters more for JIT startup on large programs.
7. **Expand the language subset**: `struct`, `match`, closures, iterator
   adapters, `Option`/`Result`.

Until steps 1 and 2 are done, 15.9x (and 3–4x for the eligible integer kernels)
is the honest limit of this implementation.

### Subset limits (rejected with clear messages)

- Labelled `break`/`continue` (`break 'outer`) — not modelled.
- `*x = ...` (assignment through a reference) — references to scalars are
  passed by value, so the write would not reach the caller.
- Recursive functions (direct or indirect) stay in the interpreter: the JIT has
  no native stack-depth guard, so only acyclic calls are JIT-compiled.

---

## License

Ruster is released under the Apache License 2.0. See [LICENSE](LICENSE).
