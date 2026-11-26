//! A tiny self-contained x86-64 baseline JIT for integer-only functions.
//!
//! No external crates: executable memory comes from `mmap`, and the encoder
//! emits machine code byte by byte. Local variables live in callee-saved
//! registers (rbx, r12-r15); the operand stack lives in the JIT frame.
//!
//! A function is eligible when:
//!   * every parameter and the return type are integer types,
//!   * it has at most 6 parameters and at most 5 locals,
//!   * its bytecode contains only integer ops (no division/remainder, no
//!     methods, no strings/Vec, no globals, no unboxed-unit returns),
//!   * every function it calls is eligible too.
//!
//! Non-eligible functions keep running in the interpreter; the VM falls back
//! per call, and only when all arguments are `Value::Int`.

use crate::ast::BinOp;
use crate::compiler::{Bytecode, Func, Op};
use std::collections::HashMap;

const PAGE: usize = 4096;
const PROT_RWX: i32 = 7;
const MAP_PRIVATE_ANON: i32 = 0x22;

extern "C" {
    fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    fn munmap(addr: *mut u8, len: usize) -> i32;
    fn mprotect(addr: *mut u8, len: usize, prot: i32) -> i32;
}

const RAX: u8 = 0;
const RCX: u8 = 1;
const RBX: u8 = 3;
const RBP: u8 = 5;
const RDI: u8 = 7;
const RSI: u8 = 6;
const RDX: u8 = 2;
const R8: u8 = 8;
const R9: u8 = 9;
const R12: u8 = 12;
const R13: u8 = 13;
const R14: u8 = 14;
const R15: u8 = 15;

const LOCAL_REGS: [u8; 5] = [RBX, R12, R13, R14, R15];
const ARG_REGS: [u8; 6] = [RDI, RSI, RDX, RCX, R8, R9];

pub struct Jit {
    ptrs: HashMap<u32, *const u8>,
    pages: Vec<(*mut u8, usize)>,
    names: Vec<String>,
}

impl Default for Jit {
    fn default() -> Self {
        Jit::empty()
    }
}

impl Drop for Jit {
    fn drop(&mut self) {
        for (ptr, len) in &self.pages {
            unsafe {
                munmap(*ptr, *len);
            }
        }
    }
}

impl Jit {
    pub fn empty() -> Jit {
        Jit { ptrs: HashMap::new(), pages: Vec::new(), names: Vec::new() }
    }

    pub fn get(&self, idx: u32) -> Option<*const u8> {
        self.ptrs.get(&idx).copied()
    }

    pub fn names(&self) -> &[String] {
        &self.names
    }
}

unsafe fn call_int_fn(ptr: *const u8, args: &[i64]) -> i64 {
    match args.len() {
        0 => std::mem::transmute::<*const u8, extern "C" fn() -> i64>(ptr)(),
        1 => std::mem::transmute::<*const u8, extern "C" fn(i64) -> i64>(ptr)(args[0]),
        2 => std::mem::transmute::<*const u8, extern "C" fn(i64, i64) -> i64>(ptr)(args[0], args[1]),
        3 => std::mem::transmute::<*const u8, extern "C" fn(i64, i64, i64) -> i64>(ptr)(
            args[0], args[1], args[2],
        ),
        4 => std::mem::transmute::<*const u8, extern "C" fn(i64, i64, i64, i64) -> i64>(ptr)(
            args[0], args[1], args[2], args[3],
        ),
        5 => std::mem::transmute::<*const u8, extern "C" fn(i64, i64, i64, i64, i64) -> i64>(ptr)(
            args[0], args[1], args[2], args[3], args[4],
        ),
        6 => std::mem::transmute::<*const u8, extern "C" fn(i64, i64, i64, i64, i64, i64) -> i64>(
            ptr,
        )(args[0], args[1], args[2], args[3], args[4], args[5]),
        _ => unreachable!("JIT functions take at most 6 arguments"),
    }
}

/// Call a JIT-compiled function with already-unboxed integer arguments.
///
/// # Safety
/// `ptr` must point to code produced by this module for a function with
/// `args.len()` parameters, and every argument must be an integer.
pub unsafe fn call(ptr: *const u8, args: &[i64]) -> i64 {
    call_int_fn(ptr, args)
}

pub fn compile(prog: &Bytecode) -> Jit {
    #[cfg(not(target_arch = "x86_64"))]
    {
        let _ = prog;
        return Jit::empty();
    }
    #[cfg(target_arch = "x86_64")]
    let funcs = &prog.funcs;
    let n = funcs.len();
    let mut ok = vec![false; n];
    for (i, f) in funcs.iter().enumerate() {
        ok[i] = eligible_preconditions(f)
            && !f.code
                .iter()
                .any(|op| matches!(op, Op::Call(c) if *c as usize == i))
            && analyse(funcs, i).is_some();
    }
    // A function is only eligible if every callee is eligible.
    loop {
        let mut changed = false;
        for i in 0..n {
            if !ok[i] {
                continue;
            }
            for op in &funcs[i].code {
                if let Op::Call(c) = op {
                    if !ok[*c as usize] {
                        ok[i] = false;
                        changed = true;
                        break;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut generated: HashMap<u32, Generated> = HashMap::new();
    for i in 0..n {
        if !ok[i] {
            continue;
        }
        if let Some(g) = codegen(funcs, i as u32) {
            generated.insert(i as u32, g);
        }
    }
    // Drop callers whose callee failed to generate.
    loop {
        let missing: Vec<u32> = generated
            .iter()
            .filter(|(_, g)| {
                g.calls
                    .iter()
                    .any(|(_, callee)| !generated.contains_key(callee))
            })
            .map(|(i, _)| *i)
            .collect();
        if missing.is_empty() {
            break;
        }
        for i in missing {
            generated.remove(&i);
        }
    }
    if generated.is_empty() {
        return Jit::empty();
    }

    let mut jit = Jit::empty();
    jit.names = generated
        .keys()
        .map(|i| funcs[*i as usize].name.clone())
        .collect();

    // Allocate one page per function first, so call targets are known.
    for (&idx, _) in generated.iter() {
        let page = unsafe {
            mmap(
                std::ptr::null_mut(),
                PAGE,
                PROT_RWX,
                MAP_PRIVATE_ANON,
                -1,
                0,
            )
        };
        if page.is_null() || page as isize == -1 {
            return Jit::empty();
        }
        jit.ptrs.insert(idx, page as *const u8);
        jit.pages.push((page, PAGE));
    }

    // Resolve jumps, patch call addresses, then copy into executable pages.
    for (&idx, g) in generated.iter() {
        let mut buf = g.buf.clone();
        for &(pos, target_pc) in &g.jumps {
            let target = *g.pc_off.get(target_pc as usize).unwrap_or(&0) as i32;
            let rel = target - (pos as i32 + 4);
            buf[pos..pos + 4].copy_from_slice(&rel.to_le_bytes());
        }
        for &(pos, callee) in &g.calls {
            let target = match jit.ptrs.get(&callee) {
                Some(p) => *p as usize,
                None => return Jit::empty(),
            };
            buf[pos..pos + 8].copy_from_slice(&(target as u64).to_le_bytes());
        }
        let dst = jit.ptrs[&idx] as *mut u8;
        unsafe {
            std::ptr::copy_nonoverlapping(buf.as_ptr(), dst, buf.len());
            // Drop write permission once the code is final (W^X).
            mprotect(dst, PAGE, 1 | 4);
        }
    }
    jit
}

fn eligible_preconditions(f: &Func) -> bool {
    f.params_int
        && f.ret_int
        && f.nparams <= 6
        && f.nlocals as usize <= LOCAL_REGS.len()
        && !f.code.is_empty()
        && f.code.iter().all(supported_op)
}

fn arith_ok(op: BinOp) -> bool {
    !matches!(op, BinOp::Div | BinOp::Rem | BinOp::And | BinOp::Or)
}

fn cmp_ok(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    )
}

fn supported_op(op: &Op) -> bool {
    match op {
        Op::Imm(_)
        | Op::Load(_)
        | Op::Store(_)
        | Op::Pop
        | Op::FoldImm(_)
        | Op::Jump(_)
        | Op::JumpF(_)
        | Op::JumpT(_)
        | Op::JumpLocal { .. }
        | Op::CopyLocal { .. }
        | Op::Call(_)
        | Op::Return => true,
        Op::CmpJump { op, .. } | Op::CmpImmJump { op, .. } => cmp_ok(*op),
        Op::Bin(op) => arith_ok(*op),
        Op::BinLocals { op, .. }
        | Op::BinLocalImm { op, .. }
        | Op::PushBinLocals { op, .. }
        | Op::PushBinLocalImm { op, .. }
        | Op::TopBinImm { op, .. }
        | Op::BinStore { op, .. } => arith_ok(*op),
        Op::Un(crate::ast::UnOp::Neg) => true,
        _ => false,
    }
}

struct Generated {
    buf: Vec<u8>,
    jumps: Vec<(usize, u32)>,
    calls: Vec<(usize, u32)>,
    pc_off: Vec<u32>,
}

struct Emitter {
    buf: Vec<u8>,
    jumps: Vec<(usize, u32)>,
    calls: Vec<(usize, u32)>,
    pc_off: Vec<u32>,
}

fn rex(w: bool, r: u8, b: u8) -> u8 {
    0x40 | if w { 8 } else { 0 } | ((r >> 3) << 2) | (b >> 3)
}

impl Emitter {
    fn emit(&mut self, bytes: &[u8]) {
        self.buf.extend_from_slice(bytes);
    }

    /// `mov reg, imm64`
    fn mov_imm(&mut self, reg: u8, v: i64) {
        self.buf.push(rex(true, 0, reg));
        self.buf.push(0xB8 + (reg & 7));
        self.buf.extend_from_slice(&v.to_le_bytes());
    }

    /// `mov dst, src` for two 64-bit registers.
    fn mov_rr(&mut self, dst: u8, src: u8) {
        self.buf.push(rex(true, src, dst));
        self.buf.push(0x89);
        self.buf.push(0xC0 | ((src & 7) << 3) | (dst & 7));
    }

    /// `mov reg, [rbp + disp]`
    fn load_mem(&mut self, reg: u8, disp: i32) {
        self.buf.push(rex(true, reg, RBP));
        self.buf.push(0x8B);
        self.buf.push(0x80 | ((reg & 7) << 3) | (RBP & 7));
        self.buf.extend_from_slice(&disp.to_le_bytes());
    }

    /// `mov [rbp + disp], reg`
    fn store_mem(&mut self, disp: i32, reg: u8) {
        self.buf.push(rex(true, reg, RBP));
        self.buf.push(0x89);
        self.buf.push(0x80 | ((reg & 7) << 3) | (RBP & 7));
        self.buf.extend_from_slice(&disp.to_le_bytes());
    }

    fn stack_load(&mut self, reg: u8, depth: usize) {
        self.load_mem(reg, -8 * (depth as i32 + 1));
    }

    fn stack_store(&mut self, depth: usize, reg: u8) {
        self.store_mem(-8 * (depth as i32 + 1), reg);
    }

    fn jcc(&mut self, cc: u8, target_pc: u32) {
        self.buf.push(0x0F);
        self.buf.push(0x80 | cc);
        let pos = self.buf.len();
        self.buf.extend_from_slice(&[0, 0, 0, 0]);
        self.jumps.push((pos, target_pc));
    }

    fn jmp(&mut self, target_pc: u32) {
        self.buf.push(0xE9);
        let pos = self.buf.len();
        self.buf.extend_from_slice(&[0, 0, 0, 0]);
        self.jumps.push((pos, target_pc));
    }

    fn call_patch(&mut self, callee: u32) {
        self.mov_imm(RAX, 0);
        let pos = self.buf.len() - 8;
        self.calls.push((pos, callee));
        self.emit(&[0xFF, 0xD0]); // call rax
    }

    /// Emit `rax = rax OP rcx`. Returns false for unsupported operators.
    fn emit_bin(&mut self, op: BinOp) -> bool {
        match op {
            BinOp::Add => self.emit(&[0x48, 0x01, 0xC8]),
            BinOp::Sub => self.emit(&[0x48, 0x29, 0xC8]),
            BinOp::Mul => self.emit(&[0x48, 0x0F, 0xAF, 0xC1]),
            BinOp::BitAnd => self.emit(&[0x48, 0x21, 0xC8]),
            BinOp::BitOr => self.emit(&[0x48, 0x09, 0xC8]),
            BinOp::BitXor => self.emit(&[0x48, 0x31, 0xC8]),
            BinOp::Shl => {
                self.emit(&[0x48, 0x83, 0xE1, 0x3F]); // and rcx, 63
                self.emit(&[0x48, 0xD3, 0xE0]); // shl rax, cl
            }
            BinOp::Shr => {
                self.emit(&[0x48, 0x83, 0xE1, 0x3F]); // and rcx, 63
                self.emit(&[0x48, 0xD3, 0xF8]); // sar rax, cl
            }
            BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let cc = cond_code(op);
                self.emit(&[0x48, 0x39, 0xC8]); // cmp rax, rcx
                self.emit(&[0x0F, 0x90 | cc, 0xC0]); // setcc al
                self.emit(&[0x48, 0x0F, 0xB6, 0xC0]); // movzx rax, al
            }
            _ => return false,
        }
        true
    }

    fn epilogue(&mut self, max_stack: usize, nlocals: usize) {
        for j in 0..nlocals {
            self.load_mem(LOCAL_REGS[j], -8 * (max_stack as i32 + 1 + j as i32));
        }
        self.emit(&[0x48, 0x89, 0xEC]); // mov rsp, rbp
        self.emit(&[0x5D]); // pop rbp
        self.emit(&[0xC3]); // ret
    }
}

fn cond_code(op: BinOp) -> u8 {
    match op {
        BinOp::Eq => 0x4,
        BinOp::Ne => 0x5,
        BinOp::Lt => 0xC,
        BinOp::Le => 0xE,
        BinOp::Gt => 0xF,
        BinOp::Ge => 0xD,
        _ => unreachable!(),
    }
}

/// `a OP b` where the jump is taken when `(a OP b) == jump_if`.
fn invert(cc: u8) -> u8 {
    match cc {
        0x4 => 0x5,
        0x5 => 0x4,
        0xC => 0xD,
        0xD => 0xC,
        0xE => 0xF,
        0xF => 0xE,
        _ => cc,
    }
}

/// Compute the operand-stack height at every reachable instruction.
fn analyse(funcs: &[std::rc::Rc<Func>], idx: usize) -> Option<(Vec<Option<usize>>, usize)> {
    let f = &funcs[idx];
    let code = &f.code;
    let n = code.len();
    let mut height: Vec<Option<usize>> = vec![None; n + 1];
    let mut max_stack = 0usize;
    let mut work = vec![0usize];
    height[0] = Some(0);
    while let Some(pc) = work.pop() {
        if pc >= n {
            return None;
        }
        let h = height[pc]?;
        let (pops, pushes, succs) = effect(funcs, &code[pc], pc)?;
        if h < pops {
            return None;
        }
        let nh = h - pops + pushes;
        max_stack = max_stack.max(nh);
        for s in succs {
            if s > n {
                return None;
            }
            if s == n {
                return None; // fell off the end without a `return`
            }
            match height[s] {
                None => {
                    height[s] = Some(nh);
                    work.push(s);
                }
                Some(existing) if existing != nh => return None,
                _ => {}
            }
        }
    }
    Some((height, max_stack))
}

fn effect(
    funcs: &[std::rc::Rc<Func>],
    op: &Op,
    pc: usize,
) -> Option<(usize, usize, Vec<usize>)> {
    let next = vec![pc + 1];
    Some(match op {
        Op::Imm(_) | Op::Load(_) | Op::FoldImm(_) => (0, 1, next),
        Op::Store(_) | Op::Pop => (1, 0, next),
        Op::Bin(_) => (2, 1, next),
        Op::Un(_) => (1, 1, next),
        Op::PushBinLocals { .. } | Op::PushBinLocalImm { .. } => (0, 1, next),
        Op::TopBinImm { .. } => (1, 1, next),
        Op::BinStore { .. } => (2, 0, next),
        Op::BinLocals { .. } | Op::BinLocalImm { .. } | Op::CopyLocal { .. } => (0, 0, next),
        Op::Call(c) => (funcs[*c as usize].nparams as usize, 1, next),
        Op::Jump(t) => (0, 0, vec![*t as usize]),
        Op::JumpF(t) | Op::JumpT(t) => (1, 0, vec![pc + 1, *t as usize]),
        Op::JumpLocal { target, .. }
        | Op::CmpJump { target, .. }
        | Op::CmpImmJump { target, .. } => (0, 0, vec![pc + 1, *target as usize]),
        Op::Return => (1, 0, Vec::new()),
        _ => return None,
    })
}

fn codegen(funcs: &[std::rc::Rc<Func>], idx: u32) -> Option<Generated> {
    let f = &funcs[idx as usize];
    let (height, max_stack) = analyse(funcs, idx as usize)?;
    let nlocals = f.nlocals as usize;
    let frame_slots = max_stack + nlocals + 1;
    let frame_size = ((frame_slots * 8 + 15) / 16) * 16;
    if frame_size > PAGE - 64 {
        return None;
    }

    let mut e = Emitter {
        buf: Vec::with_capacity(512),
        jumps: Vec::new(),
        calls: Vec::new(),
        pc_off: vec![0; f.code.len() + 1],
    };

    // Prologue.
    e.emit(&[0x55]); // push rbp
    e.emit(&[0x48, 0x89, 0xE5]); // mov rbp, rsp
    e.emit(&[0x48, 0x81, 0xEC]); // sub rsp, imm32
    e.buf.extend_from_slice(&(frame_size as i32).to_le_bytes());
    for j in 0..nlocals {
        e.store_mem(-8 * (max_stack as i32 + 1 + j as i32), LOCAL_REGS[j]);
    }
    for i in 0..f.nparams as usize {
        e.mov_rr(LOCAL_REGS[i], ARG_REGS[i]);
    }

    for pc in 0..f.code.len() {
        e.pc_off[pc] = e.buf.len() as u32;
        let h = match height[pc] {
            Some(h) => h,
            None => continue, // unreachable
        };
        match &f.code[pc] {
            Op::Imm(v) | Op::FoldImm(v) => {
                e.mov_imm(RAX, *v);
                e.stack_store(h, RAX);
            }
            Op::Load(s) => {
                e.mov_rr(RAX, LOCAL_REGS[*s as usize]);
                e.stack_store(h, RAX);
            }
            Op::Store(s) => {
                e.stack_load(RAX, h - 1);
                e.mov_rr(LOCAL_REGS[*s as usize], RAX);
            }
            Op::Pop => {}
            Op::CopyLocal { dst, src } => e.mov_rr(LOCAL_REGS[*dst as usize], LOCAL_REGS[*src as usize]),
            Op::Bin(op) => {
                e.stack_load(RAX, h - 2);
                e.stack_load(RCX, h - 1);
                if !e.emit_bin(*op) {
                    return None;
                }
                e.stack_store(h - 2, RAX);
            }
            Op::Un(crate::ast::UnOp::Neg) => {
                e.stack_load(RAX, h - 1);
                e.emit(&[0x48, 0xF7, 0xD8]); // neg rax
                e.stack_store(h - 1, RAX);
            }
            Op::PushBinLocals { a, b, op } => {
                e.mov_rr(RAX, LOCAL_REGS[*a as usize]);
                e.mov_rr(RCX, LOCAL_REGS[*b as usize]);
                if !e.emit_bin(*op) {
                    return None;
                }
                e.stack_store(h, RAX);
            }
            Op::PushBinLocalImm { a, imm, op } => {
                e.mov_rr(RAX, LOCAL_REGS[*a as usize]);
                e.mov_imm(RCX, *imm);
                if !e.emit_bin(*op) {
                    return None;
                }
                e.stack_store(h, RAX);
            }
            Op::TopBinImm { imm, op } => {
                e.stack_load(RAX, h - 1);
                e.mov_imm(RCX, *imm);
                if !e.emit_bin(*op) {
                    return None;
                }
                e.stack_store(h - 1, RAX);
            }
            Op::BinStore { op, dst } => {
                e.stack_load(RAX, h - 2);
                e.stack_load(RCX, h - 1);
                if !e.emit_bin(*op) {
                    return None;
                }
                e.mov_rr(LOCAL_REGS[*dst as usize], RAX);
            }
            Op::BinLocals { dst, a, b, op } => {
                e.mov_rr(RAX, LOCAL_REGS[*a as usize]);
                e.mov_rr(RCX, LOCAL_REGS[*b as usize]);
                if !e.emit_bin(*op) {
                    return None;
                }
                e.mov_rr(LOCAL_REGS[*dst as usize], RAX);
            }
            Op::BinLocalImm { dst, a, imm, op } => {
                e.mov_rr(RAX, LOCAL_REGS[*a as usize]);
                e.mov_imm(RCX, *imm);
                if !e.emit_bin(*op) {
                    return None;
                }
                e.mov_rr(LOCAL_REGS[*dst as usize], RAX);
            }
            Op::Jump(t) => e.jmp(*t),
            Op::JumpF(t) => {
                e.stack_load(RAX, h - 1);
                e.emit(&[0x48, 0x85, 0xC0]); // test rax, rax
                e.jcc(0x4, *t); // jz
            }
            Op::JumpT(t) => {
                e.stack_load(RAX, h - 1);
                e.emit(&[0x48, 0x85, 0xC0]);
                e.jcc(0x5, *t); // jnz
            }
            Op::JumpLocal { slot, target, jump_if } => {
                e.mov_rr(RAX, LOCAL_REGS[*slot as usize]);
                e.emit(&[0x48, 0x85, 0xC0]);
                e.jcc(if *jump_if { 0x5 } else { 0x4 }, *target);
            }
            Op::CmpJump { a, b, op, target, jump_if } => {
                e.mov_rr(RAX, LOCAL_REGS[*a as usize]);
                e.mov_rr(RCX, LOCAL_REGS[*b as usize]);
                e.emit(&[0x48, 0x39, 0xC8]); // cmp rax, rcx
                let mut cc = cond_code(*op);
                if !*jump_if {
                    cc = invert(cc);
                }
                e.jcc(cc, *target);
            }
            Op::CmpImmJump { a, imm, op, target, jump_if } => {
                e.mov_rr(RAX, LOCAL_REGS[*a as usize]);
                e.mov_imm(RCX, *imm);
                e.emit(&[0x48, 0x39, 0xC8]);
                let mut cc = cond_code(*op);
                if !*jump_if {
                    cc = invert(cc);
                }
                e.jcc(cc, *target);
            }
            Op::Call(callee) => {
                let n = funcs[*callee as usize].nparams as usize;
                for i in 0..n {
                    e.stack_load(ARG_REGS[i], h - n + i);
                }
                e.call_patch(*callee);
                e.stack_store(h - n, RAX);
            }
            Op::Return => {
                e.stack_load(RAX, h - 1);
                e.epilogue(max_stack, nlocals);
            }
            _ => return None,
        }
    }
    e.pc_off[f.code.len()] = e.buf.len() as u32;
    if e.buf.len() + 16 > PAGE {
        return None;
    }
    Some(Generated {
        buf: e.buf,
        jumps: e.jumps,
        calls: e.calls,
        pc_off: e.pc_off,
    })
}
