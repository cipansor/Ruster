//! mini-rust: a small Rust interpreter with no external crates.
//!
//!     rust hello.rs      # run a file
//!     rust hello         # `.rs` is implied
//!     rust --compile-only hello.rs
//!     rust --dump hello.rs
//!     rust --version

mod ast;
mod compiler;
mod jit;
mod lexer;
mod ops;
mod parser;
mod value;
mod vm;

use std::process::ExitCode;
use std::time::Instant;
use vm::Trap;

const VERSION: &str = "0.1.0";

fn main() -> ExitCode {
    let code = real_main();
    ExitCode::from(code as u8)
}

fn real_main() -> i32 {
    let mut argv: Vec<String> = std::env::args().skip(1).collect();
    if argv.is_empty() {
        usage();
        return 2;
    }
    let mut compile_only = false;
    let mut dump = false;
    let mut stats = false;
    let mut no_jit = false;
    let mut path: Option<String> = None;
    let mut i = 0;
    while i < argv.len() {
        let a = argv[i].clone();
        match a.as_str() {
            "-h" | "--help" => {
                usage();
                return 0;
            }
            "-V" | "--version" => {
                println!("Ruster {VERSION} (single-binary Rust interpreter, no cargo)");
                return 0;
            }
            "-c" | "--compile-only" => compile_only = true,
            "--no-jit" => no_jit = true,
            "-d" | "--dump" => dump = true,
            "--stats" => stats = true,
            "--" => {
                if i + 1 < argv.len() {
                    path = Some(argv[i + 1].clone());
                }
                break;
            }
            other if other.starts_with('-') => {
                eprintln!("error: unknown option `{other}` (try --help)");
                return 2;
            }
            other => {
                path = Some(other.to_string());
                break;
            }
        }
        i += 1;
    }
    let path = match path {
        Some(p) => p,
        None => {
            if compile_only || dump {
                eprintln!("error: no input file");
                return 2;
            }
            argv.clear();
            usage();
            return 2;
        }
    };

    let resolved = resolve_path(&path);
    let src = match std::fs::read_to_string(&resolved) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("error: cannot read `{resolved}`: {e}");
            return 2;
        }
    };

    let t0 = Instant::now();
    let tokens = match lexer::Lexer::new(&src).tokenize() {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    let ast = match parser::Parser::new(tokens).parse_program() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    let bc = match compiler::compile(ast) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: {e}");
            return 1;
        }
    };
    let front_end = t0.elapsed();

    if dump {
        dump_bytecode(&bc);
        return 0;
    }
    if compile_only {
        let instrs: usize = bc.funcs.iter().map(|f| f.code.len()).sum();
        eprintln!(
            "Ruster front-end: {:.3} ms | {} function(s), {} instruction(s), {} constant(s), {} template(s), {} global(s)",
            front_end.as_secs_f64() * 1000.0,
            bc.funcs.len(),
            instrs,
            bc.consts.len(),
            bc.templates.len(),
            bc.globals.len()
        );
        return 0;
    }

    let use_jit = !no_jit && std::env::var_os("MINIRUST_NO_JIT").is_none();
    let run_start = Instant::now();
    let mut machine = vm::Vm::new(bc, use_jit);
    let outcome = machine.run();
    let run_time = run_start.elapsed();
    if stats {
        eprintln!(
            "Ruster stats: front-end {:.3} ms, run {:.3} ms",
            front_end.as_secs_f64() * 1000.0,
            run_time.as_secs_f64() * 1000.0
        );
        if use_jit {
            let names = machine.jit_names();
            if names.is_empty() {
                eprintln!("Ruster jit: no functions were eligible");
            } else {
                eprintln!("Ruster jit: {} function(s): {}", names.len(), names.join(", "));
            }
        } else {
            eprintln!("Ruster jit: disabled");
        }
    }
    match outcome {
        Ok(()) => 0,
        Err(Trap::Exit(code)) => code,
        Err(Trap::Panic(msg)) => {
            eprintln!("thread 'main' panicked:");
            eprintln!("{msg}");
            eprintln!("note: this is a mini-rust panic (the program aborted)");
            101
        }
    }
}

fn resolve_path(path: &str) -> String {
    if std::path::Path::new(path).is_file() {
        return path.to_string();
    }
    if !path.ends_with(".rs") {
        let with_ext = format!("{path}.rs");
        if std::path::Path::new(&with_ext).is_file() {
            return with_ext;
        }
    }
    path.to_string()
}

fn usage() {
    eprintln!(
        "Ruster {VERSION} — a fast, dependency-free interpreter for a subset of Rust

USAGE:
    rust <file.rs> [--stats]     run a program
    rust <file>                  `.rs` is implied when the file has no extension
    rust --compile-only <file>   lex + parse + compile only (front-end timing)
    rust --dump <file>           print the generated bytecode
    rust --version
    rust --help

The interpreter is a single binary built with:
    rustc --edition 2024 -O ruster/src/main.rs -o bin/rust
No cargo, no crates, no network."
    );
}

fn dump_bytecode(bc: &compiler::Bytecode) {
    for (i, f) in bc.funcs.iter().enumerate() {
        println!("fn {} [{}] params={} locals={}", f.name, i, f.nparams, f.nlocals);
        for (pc, op) in f.code.iter().enumerate() {
            println!("  {pc:04}  {op:?}");
        }
    }
}
