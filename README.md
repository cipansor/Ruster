# minirust — interpreter Rust dalam satu binary, tanpa cargo

Eksperimen: seberapa jauh sebuah *interpreter* Rust murni (tanpa LLVM, tanpa JIT,
tanpa cargo, tanpa crate eksternal) bisa mendekati kecepatan program Rust yang
dikompilasi native?

Jawaban singkat dan jujur: **untuk program yang berjalan lama, tidak bisa
mendekati native** — interpreter ini 15x–192x lebih lambat (geometric mean
**44.5x**) pada benchmark di bawah. Tetapi ada sisi lain yang menarik:

| Fase | rustc -O + native | minirust | Pemenang |
|---|---:|---:|---|
| "Compile" | 76–124 ms, 90–111 MB RAM | **~1 ms**, 15.6 MB RAM | minirust ~75–115x |
| Runtime (program kecil, hello) | 0.8 ms | 1.0 ms | seri (startup-dominated) |
| Runtime (program berat) | 2.2–8.6 ms | 19–1645 ms | native 15–192x |
| Ukuran | 4.3 MB **per program** | 4.8 MB **untuk semua program** | minirust (jika banyak program) |

Untuk program pendek, *end-to-end* (`sumber -> output`) minirust menang karena
tidak ada langkah kompilasi sama sekali. Untuk program yang berjalan lebih dari
beberapa milidetik, native menang telak.

---

## 1. Build & pakai

Tidak perlu cargo. Cukup `rustc` (yang memang dipakai untuk membangun
interpreter ini):

```bash
rustc --edition 2021 -O minirust/src/main.rs -o bin/rust
```

Lalu:

```bash
$ bin/rust hello.rs
Halo, dunia! 1 + 1 = 2

$ bin/rust hello          # ekstensi .rs otomatis dicoba
Halo, dunia! 1 + 1 = 2

$ bin/rust --stats hello.rs
Halo, dunia! 1 + 1 = 2
minirust stats: front-end 0.019 ms, run 0.017 ms

$ bin/rust --compile-only hello.rs
minirust front-end: 0.019 ms | 1 function(s), 7 instruction(s), 2 constant(s), 1 template(s), 0 global(s)

$ bin/rust --dump hello.rs
fn main [0] params=0 locals=0
  0000  Imm(1)
  0001  Imm(1)
  0002  Bin(Add)
  ...
```

Opsi lain: `--version`, `--help`. Kalau ingin benar-benar dipanggil sebagai
`rust hallo.rs`, salin binary-nya:

```bash
install -m755 bin/rust ~/.local/bin/rust
```

(Tidak ada dependency runtime: binary-nya statis terhadap Rust std, tanpa file
pendamping.)

### Menjalankan test

```bash
# 11 program fitur: output minirust dibandingkan byte-per-byte dengan rustc -O
python3 tests/compare.py

# 4 kasus error: exit code 101 (panic) / 3 (process::exit)
python3 tests/errors.py
```

### Menjalankan benchmark

```bash
python3 bench/bench.py --runs 5 --compile-runs 5
# hasil: bench/results.md dan bench/results.json
```

Harness hanya memakai Python untuk *mengukur* (fork/exec + `os.wait4`), bukan
bagian dari runtime interpreter.

---

## 2. Arsitektur

```
sumber .rs
   |
   v
lexer.rs     -> token (angka hex/bin/oct, string escape, komentar bersarang, dst)
   |
   v
parser.rs    -> AST (precedence climbing, jalur `a::b`, turbofish, macro)
   |
   v
compiler.rs  -> bytecode stack-based + peephole optimizer + evaluasi const
   |
   v
vm.rs        -> VM dengan frame stack eksplisit (bukan rekursi host)
```

Poin desain yang penting untuk kecepatan dan ketahanan:

- **Bytecode, bukan tree-walking.** Setiap ekspresi dikompilasi ke instruksi
  stack machine. Variabel lokal menjadi indeks slot (`Load(u16)`/`Store(u16)`),
  bukan pencarian nama saat runtime.
- **Frame stack eksplisit.** Pemanggilan fungsi tidak memakai rekursi Rust;
  `frames: Vec<Frame>` dan satu `locals: Vec<Value>` datar. Rekursi 100.000
  level berjalan (lihat `tests/programs/recursion.rs`), dibatasi RAM, bukan
  stack 8 MB host.
- **Peephole optimizer.** Pola umum difusikan:
  - `Load a; Load b; Bin op; Store c` -> `BinLocals`
  - `Load a; Imm k; Bin op; Store c` -> `BinLocalImm`
  - `Load a; Load b; Bin op; JumpF/JumpT t` -> `CmpJump`
  - `Load a; JumpF/JumpT t` -> `JumpLocal`

  Ini memangkas jumlah instruksi loop numerik hampir setengah.
- **Fast path numerik.** `i64 op i64`, `f64 op f64`, dan perbandingan punya
  jalur khusus di VM; tipe lain jatuh ke implementasi umum yang sama
  (`ops.rs`, satu sumber kebenaran untuk VM dan evaluator `const`).
- **Tanpa crate eksternal.** Hanya `std`: `Rc`, `RefCell`, `HashMap`,
  `BufWriter`.
- **Format engine sendiri.** `println!`, `format!`, `{:?}`, `{:.2}`, `{:>8}`,
  `{:08}`, `{:x}`, `{:b}`, `{{`/`}}`, argumen posisional.

Ukuran kode: ~4.600 baris Rust (lexer 407, parser 936, compiler 1526,
vm 729, value 376, ops 271, ast 222, main 194).

---

## 3. Subset bahasa yang didukung

| Kategori | Yang didukung |
|---|---|
| Item | `fn`, `const`, `static`/`static mut`, `mod { ... }` inline, `use` (diabaikan) |
| Tipe | `i8..i64`, `u8..u64`, `usize`/`isize`, `f32`/`f64`, `bool`, `char`, `String`, `&str`, `Vec<T>`, array `[T; N]`, tuple, referensi `&T`/`&mut T` (identitas) |
| Statement | `let` (+`mut`, type annotation, destructuring tuple), blok, `if`/`else if`/`else`, `while`, `loop`, `for i in a..b`/`a..=b`, `for x in vec`, `break`/`continue`, `loop` sebagai ekspresi + `break nilai` |
| Ekspresi | aritmatika, bitwise, perbandingan, `&&`/`||` (short-circuit), assignment & compound assignment, `as` cast, `&`/`&mut`/`*`, index `v[i]`, tuple `.0`, method call, macro |
| Fungsi | rekursi, parameter, return; fungsi top-level (dan di dalam `mod` inline) |
| Macro | `println!`, `print!`, `eprintln!`, `eprint!`, `format!`, `vec![]`, `vec![x; n]`, `panic!`, `assert!`, `assert_eq!`, `assert_ne!`, `dbg!`, `unreachable!`/`todo!`/`unimplemented!` |
| Method | `len`, `is_empty`, `push`, `push_str`, `pop`, `clear`, `insert`, `remove`, `reverse`, `sort`, `contains`, `first`, `last`, `join`, `trim*`, `starts_with`, `ends_with`, `replace`, `repeat`, `to_uppercase`, `to_lowercase`, `split`, `split_whitespace`, `chars`, `abs`, `pow`, `powf`, `powi`, `sqrt`, `floor`, `ceil`, `round`, `min`, `max`, `clamp`, `signum`, `wrapping_*`, `saturating_*`, `to_string`, `clone`, dan `.iter()/.into_iter()/.copied()/.as_str()` (identitas) |
| Associated | `String::new/from/with_capacity`, `Vec::new/with_capacity`, `std::process::exit` |
| Konstanta path | `i64::MAX/MIN`, `u8::MAX`, `f64::PI/E/INFINITY/NAN/...` |

**Tidak didukung (sengaja, di luar cakupan):** `struct`, `enum`, `impl`,
`trait`, closure, adapter iterator (`map`/`filter`/`fold`), `Option`/`Result`
dan operator `?`, `match`, slicing `&s[a..b]`, `HashMap`, borrow checker,
lifetime, generic nyata (hanya diparse lalu diabaikan), file modul terpisah.

---

## 4. Hasil benchmark

Lingkungan: Linux x86-64, 4 vCPU container, `rustc 1.100.0`, Rust `-O`
(opt-level 3). Setiap program: 5x kompilasi, 5x eksekusi; angka adalah median
wall/CPU dan peak RSS maksimum.

### 4.1 Fase kompilasi (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | native binary | minirust front-end wall | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 76.0 ms | 89.3 ms | 94.6 MB | 4.3 MB | 1.0 ms | 0.9 ms | 15.6 MB |
| fib | 76.3 ms | 91.9 ms | 94.8 MB | 4.3 MB | 1.0 ms | 1.0 ms | 15.6 MB |
| loop_sum | 78.6 ms | 92.9 ms | 90.6 MB | 4.3 MB | 1.2 ms | 1.1 ms | 15.8 MB |
| sieve | 95.4 ms | 110.3 ms | 106.9 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| matmul | 124.5 ms | 140.3 ms | 107.4 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| quicksort | 102.6 ms | 118.6 ms | 107.3 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.8 MB |
| strings | 118.5 ms | 134.0 ms | 111.3 MB | 4.3 MB | 1.1 ms | 1.0 ms | 15.9 MB |

"Front-end" = lex + parse + generate bytecode + evaluasi `const`, diukur lewat
`rust --compile-only` (termasuk ~1 ms startup proses). Tidak ada kode native
yang dihasilkan; program langsung dijalankan dari bytecode.

Build interpreter sekali saja: `rustc -O` = **1.9 s wall, 5.2 s CPU (paralel),
239 MB RSS**. Setelah itu tidak ada biaya kompilasi per program.

### 4.2 Fase runtime (per program)

| program | native wall | native CPU | native CPU% | native RSS | minirust wall | minirust CPU | minirust CPU% | minirust RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.8 ms | 0.8 ms | 90% | 15.6 MB | 1.0 ms | 0.9 ms | 91% | 15.6 MB | 1.2x |
| fib | 3.0 ms | 2.9 ms | 97% | 15.6 MB | 233.4 ms | 233.1 ms | 100% | 15.6 MB | 78.0x |
| loop_sum | 8.6 ms | 8.5 ms | 99% | 15.6 MB | 1645.0 ms | 1644.6 ms | 100% | 15.8 MB | 191.5x |
| sieve | 2.3 ms | 2.2 ms | 97% | 15.8 MB | 380.5 ms | 380.3 ms | 100% | 18.0 MB | 164.0x |
| matmul | 2.2 ms | 2.1 ms | 96% | 15.8 MB | 254.4 ms | 254.0 ms | 100% | 15.8 MB | 115.7x |
| quicksort | 4.0 ms | 3.9 ms | 98% | 15.8 MB | 258.1 ms | 257.9 ms | 100% | 15.8 MB | 64.7x |
| strings | 1.2 ms | 1.1 ms | 94% | 15.8 MB | 18.7 ms | 18.5 ms | 99% | 15.9 MB | 15.8x |

`CPU%` = CPU time / wall time. Interpreter selalu ~100% (satu core, memang
single-threaded). `rustc` bisa >100% karena kompilasi paralel.

### 4.3 End-to-end (sumber -> output)

| program | rustc compile + run | minirust front-end + run | ratio |
|---|---:|---:|---:|
| hello | 76.8 ms | 2.0 ms | 1/38.3 faster |
| fib | 79.3 ms | 234.4 ms | 2.95x |
| loop_sum | 87.2 ms | 1646.2 ms | 18.88x |
| sieve | 97.8 ms | 381.6 ms | 3.90x |
| matmul | 126.7 ms | 255.5 ms | 2.02x |
| quicksort | 106.6 ms | 259.2 ms | 2.43x |
| strings | 119.6 ms | 19.7 ms | 1/6.1 faster |

**Geometric mean slowdown runtime: 44.5x.**

Titik impas kasar: karena kompilasi `rustc -O` makan ~75–125 ms, interpreter
menang end-to-end selama program native berjalan kira-kira < 2–3 ms. Di atas
itu, native menang dan selisihnya terus membesar.

### 4.4 Ukuran

| hal | native | minirust |
|---|---:|---:|
| binary hello | 4.3 MB | 4.8 MB (satu binary untuk semua) |
| binary quicksort | 4.3 MB | 4.8 MB |
| RSS saat kompilasi | 90–111 MB | 15.6 MB |
| RSS saat runtime (hello) | 15.5 MB | 15.5 MB |
| RSS runtime (sieve 1M bool) | 15.6 MB | 18.0 MB |

Catatan: `Vec<bool>` di minirust menyimpan satu `Value` (16 byte) per elemen,
bukan 1 bit/1 byte seperti Rust; itu sebabnya sieve memakai RAM lebih besar.
Native binary 4.3 MB itu sebagian besar adalah Rust std yang di-link statis;
minirust 4.8 MB juga sebagian besar std, tetapi bisa menjalankan *semua*
program di atas tanpa kompilasi per program.

---

## 5. Analisis: kenapa tidak bisa mendekati native?

Interpreter ini mencapai ~100–150 juta instruksi bytecode per detik. Itu sudah
cepat untuk interpreter, tetapi program native yang di-`-O` jauh lebih cepat
karena hal-hal berikut, yang semuanya tidak dimiliki interpreter:

1. **Dispatch per instruksi.** Setiap operasi harus: ambil opcode, `match`,
   pop/push stack, cek tag `Value`. Native tidak membayar ini sama sekali.
2. **Tidak ada register allocation.** Semua nilai hidup di stack operand dan
   `Vec<Value>`; native menaruh variabel di register.
3. **Tidak ada spesialisasi tipe.** `Value` adalah enum 16 byte; setiap operasi
   mengecek tag. Native sudah tahu tipenya saat kompilasi.
4. **Tidak ada inlining / constant folding lintas fungsi.** `fib` tetap
   memanggil `fib` lewat frame VM; native meng-inline dan menyusun ulang.
5. **Tidak ada vektorisasi.** `loop_sum` native 10 juta iterasi dalam 8.6 ms
   (~0.9 ns/iter) karena LLVM memvektorisasi loop. Interpreter menjalankan
   ~15 bytecode per iterasi dengan biaya ~10 ns per bytecode.
6. **Bound check + `Rc`/`RefCell` per akses.** `a[i][j]` di minirust =
   borrow + cek batas + clone `Value`; native cukup `lea` + `mov`.
7. **Method call lewat tabel.** `v.len()`, `s.push_str()`, dsb. masuk ke
   `dispatch()` dengan `Vec<Value>` argumen; native inline.

Benchmark ini juga memihak native secara ekstrem: `rustc -O` boleh
memvektorisasi dan menyederhanakan loop, sedangkan interpreter harus
menjalankan semantik per operasi.

### Apa yang bisa mendekati native?

Kalau tujuannya benar-benar "kecepatan mendekati native tanpa cargo", jalur
yang realistis bukan interpreter, melainkan:

- **JIT (Cranelift / LLVM).** Kompilasi bytecode ke kode mesin saat runtime.
  Ini yang dilakukan LuaJIT/PyPy/V8. Bisa mencapai 0.5–2x native untuk kode
  numerik, dengan biaya startup kompilasi.
- **Tracing JIT.** Rekam loop panas, kompilasi khusus loop itu.
- **AOT ke native dengan `rustc` itu sendiri.** Ironisnya, `rustc -O` sudah
  menjadi "JIT" terbaik yang tersedia: 75–125 ms per program, dan setelah itu
  kecepatannya native.
- **Interpreter dengan nilai unboxed + register-based bytecode + inline cache**
  bisa 2–5x lebih cepat dari versi ini, tapi tetap 10–50x dari native.

Yang *tidak* mungkin: interpreter AST/bytecode murni menyamai kode yang
dioptimasi LLVM dengan register allocation, inlining, dan SIMD.

---

## 6. Deviasi semantik (penting)

Ini bukan implementasi Rust yang lengkap atau sepenuhnya setara. Perbedaan yang
disengaja:

| Hal | Rust | minirust |
|---|---|---|
| Lebar integer | `i8`/`u8`/... berbeda | semua disimpan `i64`; overflow membungkus (seperti release) |
| `u64` > `i64::MAX` | nilai besar | disimpan sebagai bit pattern negatif; perbandingan bisa salah |
| `f32` | 32-bit | disimpan `f64` |
| Overflow | panic (debug) / wrap (release) | selalu wrap |
| Shift >= lebar | panic (debug) | dimask (`& 63`) |
| Division by zero | panic | panic (exit 101) |
| `Vec::pop()` | `Option<T>` | nilai langsung / `()` jika kosong |
| `first()`/`last()` | `Option<&T>` | nilai / `()` |
| Borrow/move | dicek compiler | tidak ada; `let b = a;` berbagi `Rc` |
| `static mut` | butuh `unsafe` | boleh di mana saja; `unsafe {}` hanya blok biasa |
| Struct/enum/match/closure | ada | tidak ada |
| `String` slicing | ada | tidak ada |

`tests/compare.py` memastikan 11 program fitur menghasilkan output **identik**
dengan `rustc -O`, jadi untuk subset yang didukung semantiknya cocok pada
kasus-kasus itu.

---

## 7. Contoh bytecode

```text
$ bin/rust --dump bench/programs/fib.rs
fn fib [0] params=1 locals=1
  0000  CmpImmJump { a: 0, imm: 2, op: Lt, target: 3, jump_if: false }
  0001  Load(0)
  0002  Jump(12)
  0003  Load(0)
  0004  Imm(1)
  0005  Bin(Sub)
  0006  Call(0)
  0007  Load(0)
  0008  Imm(2)
  0009  Bin(Sub)
  0010  Call(0)
  0011  Bin(Add)
  0012  Return
fn main [1] params=0 locals=0
  0000  Imm(30)
  0001  Call(0)
  0002  Print { tpl: 0, argc: 1, newline: true, stderr: false }
  0003  Pop
  0004  Unit
  0005  Return
```

Perhatikan `CmpImmJump` dan `BinLocalImm`: hasil peephole optimizer. Tanpa
optimizer itu, loop `i < n` menjadi 4 instruksi; dengan optimizer menjadi 1.

---

## 8. Struktur repo

```
minirust/
  src/
    lexer.rs      tokenizer
    ast.rs        AST + tipe
    parser.rs     recursive descent
    compiler.rs   AST -> bytecode, const eval, peephole
    ops.rs        semantik aritmatika/cast (dipakai VM & const eval)
    value.rs      Value, perbandingan, format Rust-like
    vm.rs         VM frame-stack eksplisit + dispatch method
    main.rs       CLI
bin/rust          hasil build (tidak di-commit)
bench/
  programs/*.rs   7 program benchmark
  bench.py        harness (os.wait4: wall, CPU, peak RSS)
  results.md      hasil terakhir
  results.json    data mentah
tests/
  programs/*.rs   11 program fitur (dibandingkan dengan rustc)
  errors/*.rs     4 kasus error
  compare.py      pembanding output vs rustc
  errors.py       cek exit code
```

---

## 9. Roadmap kalau mau lebih dekat ke native

1. **JIT Cranelift** untuk loop panas: bytecode -> CLIF -> kode mesin, cache
   per fungsi. Target realistis: 2–10x lebih cepat dari versi ini.
2. **Nilai unboxed per tipe**: bytecode khusus `i64`, `f64`, `bool` agar tidak
   ada tag check; locals `Vec<i64>` terpisah.
3. **Register-based bytecode** (bukan stack) + alokasi register linear-scan.
4. **Superinstruction lebih agresif** (mis. `for` range -> satu opcode loop).
5. **AOT cache** bytecode ke disk — tetapi front-end sudah hanya ~1 ms, jadi
   ini tidak penting.
6. **FFI ke `libgccjit`/LLVM** untuk fungsi panas, dengan fallback interpreter.

Sampai salah satu dari itu dikerjakan, angka 44.5x adalah batas jujur dari
pendekatan interpreter murni ini.
