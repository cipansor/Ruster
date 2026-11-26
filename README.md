# minirust — interpreter Rust + JIT x86-64 dalam satu binary, tanpa cargo

Eksperimen: seberapa jauh implementasi Rust dari nol (tanpa LLVM, tanpa crate
eksternal, tanpa cargo) bisa mendekati kecepatan Rust native — dan apa yang
terjadi kalau kita menambahkan JIT sendiri?

Sekarang ada dua mesin di dalam satu binary:

1. **Interpreter bytecode** untuk seluruh subset bahasa.
2. **JIT x86-64 baseline** yang menulis kode mesin untuk fungsi-fungsi yang
   seluruh parameter, lokal, pemanggilan, dan nilai kembaliannya integer.

Hasilnya (8 program benchmark, 5 run, median):

| Metrik | Hasil |
|---|---|
| Geometric-mean slowdown, interpreter saja | **43.0x** vs native |
| Geometric-mean slowdown, dengan JIT | **11.3x** vs native |
| Fungsi integer murni (fib, loop 10M, collatz) | **3.1x–3.9x** vs native |
| Kode berat `Vec`/indeks (sieve, matmul, quicksort) | 52x–135x vs native |
| Fase "compile" (lex+parse+bytecode) | **~1 ms**, 15.9 MB RAM (vs 73–125 ms, 90–111 MB untuk `rustc -O`) |
| Ukuran | 4.9 MB satu binary untuk semua program (native: 4.3 MB per program) |

Jadi: JIT memangkas jarak dari ~43x menjadi ~11.3x, dan untuk kode integer murni
kita berada di **3–4x native** — bukan native, tapi jauh lebih dekat daripada
interpreter mana pun yang saya tulis sebelumnya. Sisa jarak besar ada di kode
yang mengindeks `Vec`/`String`, yang belum dicakup JIT.

---

## 1. Build & pakai

Tidak perlu cargo. Cukup `rustc`:

```bash
rustc --edition 2021 -O minirust/src/main.rs -o bin/rust
```

Lalu:

```bash
$ bin/rust hello.rs
Halo, dunia! 1 + 1 = 2

$ bin/rust hello                 # ekstensi .rs otomatis dicoba
Halo, dunia! 1 + 1 = 2

$ bin/rust --stats hello.rs
Halo, dunia! 1 + 1 = 2
minirust stats: front-end 0.019 ms, run 0.017 ms
minirust jit: no functions were eligible

$ bin/rust --stats bench/programs/fib.rs
fib(30) = 832040
minirust stats: front-end 0.037 ms, run 8.310 ms
minirust jit: 1 function(s): fib

$ bin/rust --compile-only hello.rs
minirust front-end: 0.019 ms | 1 function(s), 7 instruction(s), 2 constant(s), 1 template(s), 0 global(s)

$ bin/rust --dump bench/programs/fib.rs
fn fib [0] params=1 locals=1
  0000  CmpImmJump { a: 0, imm: 2, op: Lt, target: 3, jump_if: false }
  ...
```

Opsi lain: `--no-jit` (paksa interpreter saja), `--version`, `--help`.
Variabel lingkungan `MINIRUST_NO_JIT=1` melakukan hal yang sama.

Kalau ingin benar-benar dipanggil `rust hallo.rs`:

```bash
install -m755 bin/rust ~/.local/bin/rust
```

Tidak ada dependency runtime dan tidak ada file pendamping.

### Menjalankan test

```bash
# 12 program fitur: rustc vs minirust (JIT) vs minirust --no-jit
python3 tests/compare.py

# 4 kasus error: exit code 101 (panic) / 3 (process::exit)
python3 tests/errors.py
```

`compare.py` menjalankan ketiga mesin dan menuntut stdout + exit code identik.
Itu berarti jalur JIT dan jalur interpreter divalidasi silang, bukan hanya
dibandingkan dengan rustc.

### Menjalankan benchmark

```bash
python3 bench/bench.py --runs 5 --compile-runs 5
# hasil: bench/results.md dan bench/results.json
```

Harness memakai Python hanya untuk mengukur (`fork/exec` + `os.wait4`), bukan
bagian dari runtime. Setiap program diukur tiga kali: native, minirust (JIT),
dan minirust `--no-jit`.

---

## 2. Arsitektur

```
sumber .rs
   |
   v
lexer.rs     -> token
   |
   v
parser.rs    -> AST
   |
   v
compiler.rs  -> bytecode stack-based + const eval + peephole optimizer
   |
   +--> jit.rs   (fungsi integer murni)  -> kode mesin x86-64
   |
   v
vm.rs        -> interpreter frame-stack; memanggil JIT bila memungkinkan
```

### Interpreter

- Bytecode stack machine; lokal adalah indeks slot, bukan lookup nama.
- Frame stack eksplisit (`Vec<Frame>` + `Vec<Value>` datar), jadi rekursi
  100.000 level jalan tanpa membebani stack host.
- Peephole optimizer memfusikan pola umum:
  - `Load a; Load b; Bin op; Store c` -> `BinLocals`
  - `Load a; Imm k; Bin op; Store c` -> `BinLocalImm`
  - `Load a; Load b; Bin op` -> `PushBinLocals`
  - `Load a; Imm k; Bin op` -> `PushBinLocalImm`
  - `Imm k; Bin op` -> `TopBinImm`
  - `Bin op; Store d` -> `BinStore`
  - `Imm a; Imm b; Bin op` -> `FoldImm` (konstanta)
  - bandingkan + lompat -> `CmpJump` / `CmpImmJump`
  - `Unit; Pop` -> dihapus (kecuali `Pop` adalah titik merge lompatan)
- Semua fusi memeriksa apakah ada lompatan yang menargetkan instruksi di
  tengah pola; kalau ada, fusi dibatalkan. Ini penting: satu bug awal membuat
  `if/else` kehilangan `Pop` milik satu cabang.

### JIT x86-64 (`jit.rs`, ~650 baris, tanpa crate)

Semua encoder ditulis manual; tidak ada Cranelift/LLVM.

**Syarat kelayakan (dicek per fungsi, fixed point atas graf panggilan):**

- semua parameter dan tipe kembali adalah tipe integer;
- maksimal 6 parameter dan 5 lokal;
- bytecode hanya berisi: `Imm`, `Load`, `Store`, `Pop`, `FoldImm`, `Bin`
  (`+ - * & | ^ << >> == != < <= > >=`), `Un(Neg)`, semua varian jump/fusi,
  `Call` ke fungsi yang juga layak, dan `Return`;
- tidak ada `Div`/`Rem` (perlu cek pembagi nol), tidak ada method, `Vec`,
  `String`, global, atau `Unit` (fungsi harus mengembalikan integer).

**Codegen:**

- Lokal dipetakan ke register callee-saved: `rbx, r12, r13, r14, r15`.
  Parameter SysV (`rdi, rsi, rdx, rcx, r8, r9`) disalin ke register lokal di
  prologue.
- Operand stack tetap di frame (`[rbp-8*(d+1)]`), sehingga register lokal tidak
  terganggu oleh operasi sementara.
- Perbandingan memakai `cmp` + `setcc` + `movzx`; cabang memakai `jcc rel32`
  yang ditambal setelah semua offset diketahui.
- Pemanggilan: argumen diambil dari operand stack ke register SysV, lalu
  `mov rax, <ptr>; call rax`. Alamat fungsi ditambal setelah semua halaman
  dialokasikan, jadi rekursi dan panggilan silang bekerja.
- Frame selalu 16-byte aligned, jadi panggilan mematuhi ABI SysV.
- Kode ditulis ke halaman `mmap` anonim, lalu `mprotect(PROT_READ|PROT_EXEC)`
  setelah final (W^X). Tidak ada halaman yang tetap writable+executable.
- Kalau pemanggilan dari interpreter memiliki argumen non-`Int`, VM jatuh ke
  jalur interpreter untuk panggilan itu (per call, bukan per program).

Konsekuensinya: `fib`, loop integer, dan collatz berjalan sebagai kode mesin;
sieve/matmul/quicksort tetap di interpreter karena mengindeks `Vec`.

---

## 3. Subset bahasa yang didukung

| Kategori | Yang didukung |
|---|---|
| Item | `fn`, `const`, `static`/`static mut`, `mod { ... }` inline, `use` (diabaikan) |
| Tipe | `i8..i64`, `u8..u64`, `usize`/`isize`, `f32`/`f64`, `bool`, `char`, `String`, `&str`, `Vec<T>`, array `[T; N]`, tuple, referensi `&T`/`&mut T` |
| Statement | `let` (+`mut`, anotasi tipe, destructuring tuple), blok, `if`/`else if`/`else`, `while`, `loop`, `for i in a..b`/`a..=b`, `for x in vec`, `break`/`continue`, `loop` sebagai ekspresi + `break nilai` |
| Ekspresi | aritmatika, bitwise, perbandingan, `&&`/`||` short-circuit, assignment & compound assignment, `as`, `&`/`&mut`/`*`, index `v[i]`, tuple `.0`, method call, macro |
| Fungsi | rekursi, parameter, return; fungsi top-level dan di dalam `mod` inline |
| Macro | `println!`, `print!`, `eprintln!`, `eprint!`, `format!`, `vec![]`, `vec![x; n]`, `panic!`, `assert!`, `assert_eq!`, `assert_ne!`, `dbg!`, `unreachable!`/`todo!`/`unimplemented!` |
| Method | `len`, `is_empty`, `push`, `push_str`, `pop`, `clear`, `insert`, `remove`, `reverse`, `sort`, `contains`, `first`, `last`, `join`, `trim*`, `starts_with`, `ends_with`, `replace`, `repeat`, `to_uppercase`, `to_lowercase`, `split`, `split_whitespace`, `chars`, `abs`, `pow`, `powf`, `powi`, `sqrt`, `floor`, `ceil`, `round`, `min`, `max`, `clamp`, `signum`, `wrapping_*`, `saturating_*`, `to_string`, `clone`, `.iter()/.into_iter()/.copied()/.as_str()` |
| Associated | `String::new/from/with_capacity`, `Vec::new/with_capacity`, `std::process::exit` |
| Konstanta path | `i64::MAX/MIN`, `u8::MAX`, `f64::PI/E/INFINITY/NAN/...` |

**Tidak didukung:** `struct`, `enum`, `impl`, `trait`, closure, adapter
iterator (`map`/`filter`/`fold`), `Option`/`Result` dan `?`, `match`, slicing,
`HashMap`, borrow checker, lifetime, generic nyata, file modul terpisah.

**Subset JIT** jauh lebih kecil dari subset interpreter: hanya fungsi integer
tanpa pembagian, tanpa akses `Vec`/`String`, maksimal 5 lokal.

---

## 4. Hasil benchmark

Lingkungan: Linux x86-64, 4 vCPU container, `rustc 1.100.0`, `-O`. Setiap
program: 5 kompilasi, 5 eksekusi; median wall/CPU, peak RSS maksimum.

### 4.1 Fase kompilasi (per program)

| program | rustc -O wall | rustc CPU | rustc RSS | binary native | front-end minirust | front-end CPU | front-end RSS |
|---|---:|---:|---:|---:|---:|---:|---:|
| hello | 72.8 ms | 87.9 ms | 89.5 MB | 4.3 MB | **1.0 ms** | 0.9 ms | 15.6 MB |
| fib | 75.9 ms | 90.8 ms | 90.6 MB | 4.3 MB | **1.0 ms** | 0.9 ms | 15.8 MB |
| loop_sum | 77.7 ms | 91.8 ms | 90.8 MB | 4.3 MB | **1.0 ms** | 1.0 ms | 15.8 MB |
| collatz | 77.4 ms | 92.0 ms | 92.1 MB | 4.3 MB | **1.1 ms** | 1.0 ms | 15.8 MB |
| sieve | 95.1 ms | 109.7 ms | 106.9 MB | 4.3 MB | **1.1 ms** | 1.0 ms | 15.9 MB |
| matmul | 124.6 ms | 139.9 ms | 107.5 MB | 4.3 MB | **1.2 ms** | 1.1 ms | 15.9 MB |
| quicksort | 103.1 ms | 117.7 ms | 108.7 MB | 4.3 MB | **1.1 ms** | 1.1 ms | 15.9 MB |
| strings | 116.5 ms | 131.5 ms | 111.2 MB | 4.3 MB | **1.1 ms** | 1.0 ms | 15.9 MB |

"Front-end" = lex + parse + bytecode + evaluasi `const`, diukur lewat
`rust --compile-only`. Tidak ada kode native yang dihasilkan pada tahap ini.

Build interpreter sekali saja: `rustc -O` = **2.5 s wall, 6.3 s CPU (paralel),
247 MB RSS**.

### 4.2 Fase runtime (dengan JIT)

| program | native wall | native CPU | native CPU% | native RSS | minirust (JIT) wall | CPU | CPU% | RSS | slowdown |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| hello | 0.9 ms | 0.8 ms | 91% | 15.6 MB | 1.1 ms | 1.0 ms | 91% | 15.6 MB | **1.2x** |
| fib | 3.0 ms | 2.9 ms | 97% | 15.6 MB | 9.3 ms | 9.2 ms | 99% | 15.6 MB | **3.1x** |
| loop_sum | 3.6 ms | 3.5 ms | 97% | 15.8 MB | 14.1 ms | 14.0 ms | 99% | 15.8 MB | **3.9x** |
| collatz | 22.9 ms | 22.8 ms | 99% | 15.8 MB | 70.6 ms | 70.4 ms | 100% | 15.8 MB | **3.1x** |
| sieve | 2.3 ms | 2.2 ms | 96% | 15.9 MB | 309.4 ms | 309.2 ms | 100% | 18.0 MB | 135.1x |
| matmul | 2.6 ms | 2.5 ms | 95% | 15.9 MB | 226.7 ms | 226.5 ms | 100% | 15.9 MB | 86.0x |
| quicksort | 4.2 ms | 4.1 ms | 97% | 15.9 MB | 217.6 ms | 217.4 ms | 100% | 15.9 MB | 51.6x |
| strings | 1.3 ms | 1.2 ms | 94% | 15.9 MB | 12.5 ms | 12.4 ms | 99% | 15.9 MB | 9.7x |

CPU usage interpreter ~99–100% (single-thread). `rustc` bisa >100% karena
kompilasi paralel.

### 4.3 Efek JIT (binary yang sama, `--no-jit` vs JIT)

| program | interpreter saja | dengan JIT | speedup JIT | native | JIT vs native |
|---|---:|---:|---:|---:|---:|
| hello | 1.3 ms | 1.1 ms | 1.16x | 0.9 ms | 1.2x |
| fib | 193.6 ms | **9.3 ms** | **20.75x** | 3.0 ms | 3.1x |
| loop_sum | 939.4 ms | **14.1 ms** | **66.76x** | 3.6 ms | 3.9x |
| collatz | 1966.5 ms | **70.6 ms** | **27.85x** | 22.9 ms | 3.1x |
| sieve | 307.5 ms | 309.4 ms | 0.99x | 2.3 ms | 135.1x |
| matmul | 225.9 ms | 226.7 ms | 1.00x | 2.6 ms | 86.0x |
| quicksort | 219.0 ms | 217.6 ms | 1.01x | 4.2 ms | 51.6x |
| strings | 12.5 ms | 12.5 ms | 1.00x | 1.3 ms | 9.7x |

### 4.4 End-to-end (sumber -> output)

| program | rustc compile + run | minirust front-end + run | rasio |
|---|---:|---:|---:|
| hello | 73.7 ms | **2.1 ms** | minirust 34.8x lebih cepat |
| fib | 79.0 ms | **10.3 ms** | minirust 7.6x lebih cepat |
| loop_sum | 81.3 ms | **15.1 ms** | minirust 5.4x lebih cepat |
| collatz | 100.3 ms | **71.7 ms** | minirust 1.4x lebih cepat |
| strings | 117.8 ms | **13.5 ms** | minirust 8.7x lebih cepat |
| matmul | 127.3 ms | 227.9 ms | native 1.79x |
| quicksort | 107.3 ms | 218.7 ms | native 2.04x |
| sieve | 97.3 ms | 310.5 ms | native 3.19x |

**Geometric-mean slowdown runtime: 11.3x dengan JIT, 43.0x tanpa JIT.**

Karena `rustc -O` makan 73–125 ms per program, minirust menang end-to-end untuk
program yang berjalan singkat — sekarang termasuk fib dan loop 10 juta iterasi.

### 4.5 Ukuran & memori

| hal | native | minirust |
|---|---:|---:|
| binary hello | 4.3 MB | 4.9 MB (satu binary untuk semua) |
| binary quicksort | 4.3 MB | 4.9 MB |
| RSS saat kompilasi | 90–111 MB | 15.9 MB |
| RSS runtime (hello) | 15.6 MB | 15.6 MB |
| RSS runtime (sieve 1M bool) | 15.9 MB | 18.0 MB |

Catatan: `Vec<bool>` di minirust menyimpan satu `Value` 16 byte per elemen,
bukan 1 bit/byte seperti Rust. Native 4.3 MB sebagian besar adalah Rust std
yang di-link statis; minirust 4.9 MB juga sebagian besar std, tetapi satu
binary menjalankan semua program tanpa kompilasi per program.

---

## 5. Analisis: di mana jaraknya sekarang?

### 5.1 Interpreter (43.0x)

Penyebab klasik: dispatch per instruksi, `Value` 16 byte ber-tag, tidak ada
register allocation, tidak ada inlining/vektorisasi, dan `Rc`/`RefCell` untuk
`Vec`/`String`. Loop `loop_sum` 10 juta iterasi butuh ~939 ms (≈94 ns/iterasi)
karena setiap iterasi mengeksekusi ~7 bytecode dengan biaya ~10 ns masing-masing.

### 5.2 JIT integer (3.1–3.9x)

JIT menghapus dispatch dan tag check: lokal di register, aritmetika langsung
`add/sub/imul`, cabang `cmp+jcc`. Hasilnya 20–67x lebih cepat dari interpreter.

Sisa 3–4x berasal dari:

- **Operand stack masih di memori.** Setiap ekspresi menulis/membaca slot
  stack, bukan register.
- **Tidak ada alokasi register untuk nilai sementara**, tidak ada common
  subexpression elimination, tidak ada inlining.
- **Tidak ada vektorisasi.** `loop_sum` native 3.6 ms untuk 10 juta iterasi
  (≈0.36 ns/iterasi) karena LLVM memvektorisasi; JIT kita 14.1 ms
  (≈1.4 ns/iterasi) — hanya ~4x lebih lambat, tetapi tetap bukan SIMD.
- **Pemanggilan tidak di-inline.** `fib` tetap memanggil lewat `call rax`
  (dengan patch alamat absolut), tanpa penyederhanaan rekursi.

Untuk kode integer murni, ini sudah "mendekati" native dalam arti yang masuk
akal: 3x, bukan 100x.

### 5.3 Kode `Vec`/`String` (52–135x)

Sieve, matmul, dan quicksort menghabiskan waktu di `Op::Index`/`Op::IndexSet`:
cek tag `Value::Vec(Rc<RefCell<Vec<Value>>>)`, borrow dinamis, cek batas, clone
`Value`, lalu aritmetika. JIT sengaja belum menangani ini karena memerlukan ABI
`Value` ber-box dan helper runtime.

Ini adalah **peluang optimasi terbesar berikutnya**:

1. Tambahkan opcode JIT untuk `Index`/`IndexSet` dengan inline cache
   (layout `Rc<RefCell<Vec<Value>>>` + pengecekan tag sekali per loop), atau
2. Buat JIT kedua dengan ABI `Value*` dan panggil helper untuk operasi non-int,
3. Atau spesialisasikan `Vec<i64>` di interpreter (buffer `Vec<i64>` + tag
   khusus) supaya index tidak lagi menyentuh `Value`.

Perkiraan realistis: 5–20x lebih cepat untuk sieve/matmul/quicksort, yang akan
menurunkan geometric mean ke kisaran 2–4x native.

### 5.4 Kenapa bukan compile-to-native?

Karena `rustc -O` sudah menjadi kompilasi native terbaik: 73–125 ms per
program, lalu kecepatan penuh. minirust menang hanya ketika biaya kompilasi
itu lebih besar daripada penghematan runtime — yaitu untuk program yang
selesai dalam beberapa milidetik. JIT adalah cara mendapatkan sebagian
kecepatan native tanpa membayar kompilasi di depan; AOT (menghasilkan kode
native lalu menyimpannya) akan menghapus startup JIT tetapi menambah
kompleksitas cache.

---

## 6. Deviasi semantik (penting)

| Hal | Rust | minirust |
|---|---|---|
| Lebar integer | `i8`/`u8`/... berbeda | semua disimpan `i64`; overflow membungkus |
| `u64` > `i64::MAX` | nilai besar | bit pattern negatif; perbandingan bisa salah |
| `f32` | 32-bit | disimpan `f64` |
| Overflow | panic (debug) / wrap (release) | selalu wrap (interpreter dan JIT) |
| Shift >= lebar | panic (debug) | dimask (`& 63`) |
| Division by zero | panic | panic (exit 101); tidak didukung JIT |
| `Vec::pop()` | `Option<T>` | nilai langsung / `()` jika kosong |
| `first()`/`last()` | `Option<&T>` | nilai / `()` |
| Borrow/move | dicek compiler | tidak ada; `let b = a;` berbagi `Rc` |
| `static mut` | butuh `unsafe` | boleh di mana saja |
| Struct/enum/match/closure | ada | tidak ada |

JIT mempertahankan semantik wrapping dan perbandingan signed `i64`; program
yang memakai fitur di luar subset JIT tetap berjalan di interpreter.

---

## 7. Contoh bytecode

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

Sembilan instruksi untuk seluruh `fib`. Tanpa fusi, versi naifnya ~20
instruksi. `CmpImmJump` adalah `Load; Imm; Bin(Lt); JumpF` yang digabung;
`PushBinLocalImm` adalah `Load; Imm; Bin` yang mendorong hasil langsung.

---

## 8. Struktur repo

```
minirust/src/
  lexer.rs      tokenizer
  ast.rs        AST + tipe
  parser.rs     recursive descent
  compiler.rs   AST -> bytecode, const eval, peephole
  ops.rs        semantik aritmatika/cast (VM & const eval)
  value.rs      Value, perbandingan, format Rust-like
  vm.rs         interpreter frame-stack + dispatch method + fallback JIT
  jit.rs        x86-64 baseline JIT (encoder, mmap, eligibility)
  main.rs       CLI
bin/rust        hasil build (gitignored)
bench/
  programs/*.rs 8 program benchmark
  bench.py      harness (wall, CPU, peak RSS, JIT vs --no-jit)
  results.md    hasil terakhir
  results.json  data mentah
tests/
  programs/*.rs 12 program fitur (dibandingkan dengan rustc, JIT dan no-JIT)
  errors/*.rs   4 kasus error
  compare.py    pembanding tiga mesin
  errors.py     cek exit code
```

Ukuran sumber: ~5.500 baris Rust (compiler 1.680, jit 650, vm 790, parser 940,
value 380, lexer 410, ops 270, ast 210, main 210).

---

## 9. Roadmap

1. **JIT untuk `Vec`/array indexing** (dampak terbesar: sieve, matmul,
   quicksort). Inline cache untuk tipe `Vec<i64>`, atau ABI `Value` ber-box
   dengan helper runtime.
2. **Register allocation untuk operand stack** di JIT (hilangkan traffic
   memori per ekspresi), lalu inlining fungsi kecil.
3. **Superinstruction loop** (`for` range -> satu opcode) dan strength
   reduction.
4. **JIT untuk `f64`** (aritmetika SSE2) agar benchmark float ikut cepat.
5. **AOT cache**: simpan bytecode/kode mesin ke disk; front-end sudah ~1 ms,
   jadi ini lebih relevan untuk JIT startup pada program besar.
6. **Perluas subset bahasa**: `struct`, `match`, closure, iterator adapter,
   `Option`/`Result`.

Sampai langkah 1 dikerjakan, angka 11.3x (dan 3–4x untuk kode integer) adalah
batas jujur dari implementasi ini.
