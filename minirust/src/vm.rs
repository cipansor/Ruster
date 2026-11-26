//! The bytecode interpreter: an explicit-stack VM (no host recursion).

use crate::compiler::{Func, MethodId, Op, TPiece};
use crate::jit;
use crate::ops;
use crate::value::{compare, format_value, parse_spec, Value};
use std::io::{BufWriter, Stderr, Stdout, Write};
use std::rc::Rc;

pub enum Trap {
    Panic(String),
    Exit(i32),
}

struct Frame {
    func: Rc<Func>,
    pc: usize,
    base: usize,
}

fn index_of(
    v: &Value,
    cast: Option<crate::ast::IntKind>,
) -> Result<i64, String> {
    match cast {
        Some(k) => ops::cast(v, ops::CastTo::Int(k))?.as_int(),
        None => v.as_index(),
    }
}

fn store_index(container: &Value, idx: i64, val: Value) -> Result<(), Trap> {
    match container {
        Value::Vec(rc) => {
            let mut v = rc.borrow_mut();
            if idx < 0 || idx as usize >= v.len() {
                return Err(Trap::Panic(format!(
                    "index out of bounds: the len is {} but the index is {}",
                    v.len(),
                    idx
                )));
            }
            v[idx as usize] = val;
            Ok(())
        }
        other => Err(Trap::Panic(format!(
            "cannot assign into {}",
            other.type_name()
        ))),
    }
}

pub struct Vm {
    prog: crate::compiler::Bytecode,
    globals: Vec<Value>,
    out: BufWriter<Stdout>,
    err: BufWriter<Stderr>,
    jit: jit::Jit,
}

fn pop(stack: &mut Vec<Value>) -> Result<Value, Trap> {
    stack
        .pop()
        .ok_or_else(|| Trap::Panic("operand stack underflow (compiler bug)".into()))
}

fn take(stack: &mut Vec<Value>, n: usize) -> Result<Vec<Value>, Trap> {
    if stack.len() < n {
        return Err(Trap::Panic("operand stack underflow (compiler bug)".into()));
    }
    Ok(stack.split_off(stack.len() - n))
}

impl Vm {
    pub fn new(prog: crate::compiler::Bytecode, use_jit: bool) -> Vm {
        let jit = if use_jit {
            jit::compile(&prog)
        } else {
            jit::Jit::empty()
        };
        let globals = prog.globals.clone();
        Vm {
            prog,
            globals,
            out: BufWriter::new(std::io::stdout()),
            err: BufWriter::new(std::io::stderr()),
            jit,
        }
    }

    pub fn jit_names(&self) -> Vec<String> {
        let mut names = self.jit.names().to_vec();
        names.sort();
        names
    }

    pub fn run(&mut self) -> Result<(), Trap> {
        let main = self.prog.main;
        let result = self.call(main, Vec::new()).map(|_| ());
        let _ = self.out.flush();
        let _ = self.err.flush();
        result
    }

    fn format_template(&self, tpl: &[TPiece], args: &[Value]) -> Result<String, String> {
        let mut out = String::new();
        for piece in tpl {
            match piece {
                TPiece::Lit(s) => out.push_str(s),
                TPiece::Arg { idx, spec } => {
                    let v = args.get(*idx).ok_or_else(|| {
                        format!(
                            "not enough arguments for format string (needed {}, got {})",
                            idx + 1,
                            args.len()
                        )
                    })?;
                    let parsed = parse_spec(spec);
                    out.push_str(&format_value(v, &parsed));
                }
            }
        }
        Ok(out)
    }

    fn call(&mut self, fidx: u32, args: Vec<Value>) -> Result<Value, Trap> {
        let prog = &self.prog;
        let main_func = prog.funcs[fidx as usize].clone();
        let mut frames: Vec<Frame> = vec![Frame { func: main_func.clone(), pc: 0, base: 0 }];
        let mut locals: Vec<Value> = vec![Value::Unit; main_func.nlocals as usize];
        for (i, a) in args.into_iter().enumerate() {
            if i < locals.len() {
                locals[i] = a;
            }
        }
        let mut stack: Vec<Value> = Vec::with_capacity(64);

        // Every `Func` is kept alive by `prog.funcs` for the whole call, so a
        // pointer to its code stays valid even when `frames` reallocates.
        let mut code: &'static [Op] = unsafe { &(*Rc::as_ptr(&main_func)).code };
        let mut base: usize = 0;
        let mut pc: usize = 0;

        loop {
            let op = match code.get(pc) {
                Some(op) => op,
                None => {
                    return Err(Trap::Panic(
                        "instruction pointer ran past the end of the function".into(),
                    ))
                }
            };
            pc += 1;

            match op {
                Op::Const(i) => stack.push(prog.consts[*i as usize].clone()),
                Op::Imm(v) => stack.push(Value::Int(*v)),
                Op::Unit => stack.push(Value::Unit),
                Op::Load(s) => stack.push(locals[base + *s as usize].clone()),
                Op::Store(s) => {
                    let v = pop(&mut stack)?;
                    locals[base + *s as usize] = v;
                }
                Op::Pop => {
                    pop(&mut stack)?;
                }
                Op::Bin(b) => {
                    let y = pop(&mut stack)?;
                    let x = pop(&mut stack)?;
                    let v = match ops::binop_fast(*b, &x, &y) {
                        Some(v) => v,
                        None => ops::binop(*b, &x, &y).map_err(Trap::Panic)?,
                    };
                    stack.push(v);
                }
                Op::Un(u) => {
                    let x = pop(&mut stack)?;
                    let v = ops::unop(*u, &x).map_err(Trap::Panic)?;
                    stack.push(v);
                }
                Op::Cast(c) => {
                    let x = pop(&mut stack)?;
                    let v = ops::cast(&x, *c).map_err(Trap::Panic)?;
                    stack.push(v);
                }
                Op::Jump(t) => pc = *t as usize,
                Op::JumpF(t) => {
                    let v = pop(&mut stack)?;
                    if !v.as_bool().map_err(Trap::Panic)? {
                        pc = *t as usize;
                    }
                }
                Op::JumpT(t) => {
                    let v = pop(&mut stack)?;
                    if v.as_bool().map_err(Trap::Panic)? {
                        pc = *t as usize;
                    }
                }
                Op::Call(callee) => {
                    let f = prog.funcs[*callee as usize].clone();
                    let n = f.nparams as usize;
                    if stack.len() < n {
                        return Err(Trap::Panic("operand stack underflow in call".into()));
                    }
                    let at_args = stack.len() - n;
                    if let Some(ptr) = self.jit.get(*callee) {
                        if stack[at_args..].iter().all(|v| matches!(v, Value::Int(_))) {
                            let mut args = [0i64; 6];
                            for i in 0..n {
                                if let Value::Int(v) = stack[at_args + i] {
                                    args[i] = v;
                                }
                            }
                            let result = unsafe { jit::call(ptr, &args[..n]) };
                            stack.truncate(at_args);
                            stack.push(Value::Int(result));
                            continue;
                        }
                    }
                    frames.last_mut().unwrap().pc = pc;
                    let at = stack.len() - n;
                    let new_base = locals.len();
                    locals.resize(new_base + f.nlocals as usize, Value::Unit);
                    for i in 0..n {
                        locals[new_base + i] = stack[at + i].clone();
                    }
                    stack.truncate(at);
                    code = unsafe { &(*Rc::as_ptr(&f)).code };
                    base = new_base;
                    pc = 0;
                    frames.push(Frame { func: f, pc: 0, base: new_base });
                }
                Op::BinLocals { dst, a, b, op } => {
                    let x = locals[base + *a as usize].clone();
                    let y = locals[base + *b as usize].clone();
                    let v = match ops::binop_fast(*op, &x, &y) {
                        Some(v) => v,
                        None => ops::binop(*op, &x, &y).map_err(Trap::Panic)?,
                    };
                    locals[base + *dst as usize] = v;
                }
                Op::FoldImm(v) => stack.push(Value::Int(*v)),
                Op::PushBinLocals { a, b, op } => {
                    let x = locals[base + *a as usize].clone();
                    let y = locals[base + *b as usize].clone();
                    let v = match ops::binop_fast(*op, &x, &y) {
                        Some(v) => v,
                        None => ops::binop(*op, &x, &y).map_err(Trap::Panic)?,
                    };
                    stack.push(v);
                }
                Op::PushBinLocalImm { a, imm, op } => {
                    let x = locals[base + *a as usize].clone();
                    let y = Value::Int(*imm);
                    let v = match ops::binop_fast(*op, &x, &y) {
                        Some(v) => v,
                        None => ops::binop(*op, &x, &y).map_err(Trap::Panic)?,
                    };
                    stack.push(v);
                }
                Op::TopBinImm { imm, op } => {
                    let x = pop(&mut stack)?;
                    let y = Value::Int(*imm);
                    let v = match ops::binop_fast(*op, &x, &y) {
                        Some(v) => v,
                        None => ops::binop(*op, &x, &y).map_err(Trap::Panic)?,
                    };
                    stack.push(v);
                }
                Op::BinStore { op, dst } => {
                    let y = pop(&mut stack)?;
                    let x = pop(&mut stack)?;
                    let v = match ops::binop_fast(*op, &x, &y) {
                        Some(v) => v,
                        None => ops::binop(*op, &x, &y).map_err(Trap::Panic)?,
                    };
                    locals[base + *dst as usize] = v;
                }
                Op::BinLocalImm { dst, a, imm, op } => {
                    let x = locals[base + *a as usize].clone();
                    let y = Value::Int(*imm);
                    let v = match ops::binop_fast(*op, &x, &y) {
                        Some(v) => v,
                        None => ops::binop(*op, &x, &y).map_err(Trap::Panic)?,
                    };
                    locals[base + *dst as usize] = v;
                }
                Op::CmpJump { a, b, op, target, jump_if } => {
                    let x = locals[base + *a as usize].clone();
                    let y = locals[base + *b as usize].clone();
                    let v = match ops::binop_fast(*op, &x, &y) {
                        Some(v) => v,
                        None => ops::binop(*op, &x, &y).map_err(Trap::Panic)?,
                    };
                    if v.as_bool().map_err(Trap::Panic)? == *jump_if {
                        pc = *target as usize;
                    }
                }
                Op::CmpImmJump { a, imm, op, target, jump_if } => {
                    let x = locals[base + *a as usize].clone();
                    let y = Value::Int(*imm);
                    let v = match ops::binop_fast(*op, &x, &y) {
                        Some(v) => v,
                        None => ops::binop(*op, &x, &y).map_err(Trap::Panic)?,
                    };
                    if v.as_bool().map_err(Trap::Panic)? == *jump_if {
                        pc = *target as usize;
                    }
                }
                Op::JumpLocal { slot, target, jump_if } => {
                    let v = locals[base + *slot as usize].clone();
                    if v.as_bool().map_err(Trap::Panic)? == *jump_if {
                        pc = *target as usize;
                    }
                }
                Op::Return => {
                    let v = stack.pop().unwrap_or(Value::Unit);
                    let fr = frames.pop().unwrap();
                    locals.truncate(fr.base);
                    if frames.is_empty() {
                        return Ok(v);
                    }
                    stack.push(v);
                    let caller = frames.last().unwrap();
                    code = unsafe { &(*Rc::as_ptr(&caller.func)).code };
                    base = caller.base;
                    pc = caller.pc;
                }
                Op::NewString => stack.push(Value::new_str(String::new())),
                Op::NewVec => stack.push(Value::new_vec(Vec::new())),
                Op::MakeArray(n) => {
                    let items = take(&mut stack, *n as usize)?;
                    stack.push(Value::new_vec(items));
                }
                Op::MakeTuple(n) => {
                    let items = take(&mut stack, *n as usize)?;
                    stack.push(Value::Tuple(Rc::new(items)));
                }
                Op::VecRepeat => {
                    let n = pop(&mut stack)?.as_int().map_err(Trap::Panic)?;
                    let item = pop(&mut stack)?;
                    if n < 0 || n > 50_000_000 {
                        return Err(Trap::Panic(format!(
                            "refusing to allocate a Vec with {n} elements"
                        )));
                    }
                    let mut v = Vec::with_capacity(n as usize);
                    for _ in 0..n {
                        v.push(item.deep_clone());
                    }
                    stack.push(Value::new_vec(v));
                }
                Op::CopyLocal { dst, src } => {
                    locals[base + *dst as usize] = locals[base + *src as usize].clone();
                }
                Op::IndexL { v, i, cast } => {
                    let idx = index_of(&locals[base + *i as usize], *cast).map_err(Trap::Panic)?;
                    let val = ops::index_get_i(&locals[base + *v as usize], idx)
                        .map_err(Trap::Panic)?;
                    stack.push(val);
                }
                Op::IndexImm { v, imm } => {
                    let val = ops::index_get_i(&locals[base + *v as usize], *imm)
                        .map_err(Trap::Panic)?;
                    stack.push(val);
                }
                Op::IndexSetLL { v, i, val, cast } => {
                    let idx = index_of(&locals[base + *i as usize], *cast).map_err(Trap::Panic)?;
                    let value = locals[base + *val as usize].clone();
                    store_index(&locals[base + *v as usize], idx, value)?;
                }
                Op::IndexSetLI { v, i, imm, cast } => {
                    let idx = index_of(&locals[base + *i as usize], *cast).map_err(Trap::Panic)?;
                    store_index(&locals[base + *v as usize], idx, Value::Int(*imm))?;
                }
                Op::IndexSetLC { v, i, k, cast } => {
                    let idx = index_of(&locals[base + *i as usize], *cast).map_err(Trap::Panic)?;
                    let value = prog.consts[*k as usize].clone();
                    store_index(&locals[base + *v as usize], idx, value)?;
                }
                Op::IndexJumpL {
                    v,
                    i,
                    cast,
                    target,
                    jump_if,
                } => {
                    let idx = index_of(&locals[base + *i as usize], *cast).map_err(Trap::Panic)?;
                    let elem = ops::index_get_i(&locals[base + *v as usize], idx)
                        .map_err(Trap::Panic)?;
                    let truth = elem.as_bool().map_err(Trap::Panic)?;
                    if truth == *jump_if {
                        pc = *target as usize;
                    }
                }
                Op::IndexRmw { op } => {
                    let val = pop(&mut stack)?;
                    let idx = pop(&mut stack)?;
                    let obj = pop(&mut stack)?;
                    let i = idx.as_index().map_err(Trap::Panic)?;
                    let elem = ops::index_get(&obj, &Value::Int(i)).map_err(Trap::Panic)?;
                    let res = match ops::binop_fast(*op, &elem, &val) {
                        Some(v) => v,
                        None => ops::binop(*op, &elem, &val).map_err(Trap::Panic)?,
                    };
                    store_index(&obj, i, res)?;
                }
                Op::IndexSetFromIndex { v, i, j, cast } => {
                    let idx = index_of(&locals[base + *i as usize], *cast).map_err(Trap::Panic)?;
                    let val = ops::index_get(
                        &locals[base + *v as usize],
                        &locals[base + *j as usize],
                    )
                    .map_err(Trap::Panic)?;
                    store_index(&locals[base + *v as usize], idx, val)?;
                }
                Op::Index => {
                    let idx = pop(&mut stack)?;
                    let c = pop(&mut stack)?;
                    let v = ops::index_get(&c, &idx).map_err(Trap::Panic)?;
                    stack.push(v);
                }
                Op::IndexSet => {
                    let val = pop(&mut stack)?;
                    let idx = pop(&mut stack)?;
                    let c = pop(&mut stack)?;
                    let i = idx.as_int().map_err(Trap::Panic)?;
                    match c {
                        Value::Vec(rc) => {
                            let mut v = rc.borrow_mut();
                            if i < 0 || i as usize >= v.len() {
                                return Err(Trap::Panic(format!(
                                    "index out of bounds: the len is {} but the index is {}",
                                    v.len(),
                                    i
                                )));
                            }
                            v[i as usize] = val;
                        }
                        other => {
                            return Err(Trap::Panic(format!(
                                "cannot assign into {}",
                                other.type_name()
                            )))
                        }
                    }
                }
                Op::TupleGet(i) => {
                    let t = pop(&mut stack)?;
                    match t {
                        Value::Tuple(items) => {
                            let v = items
                                .get(*i as usize)
                                .cloned()
                                .ok_or_else(|| Trap::Panic("tuple index out of range".into()))?;
                            stack.push(v);
                        }
                        other => {
                            return Err(Trap::Panic(format!(
                                "expected a tuple, found {}",
                                other.type_name()
                            )))
                        }
                    }
                }
                Op::Method(id, argc) => {
                    let args = take(&mut stack, *argc as usize)?;
                    let recv = pop(&mut stack)?;
                    let v = self.dispatch(*id, recv, args).map_err(Trap::Panic)?;
                    stack.push(v);
                }
                Op::Print { tpl, argc, newline, stderr } => {
                    let args = take(&mut stack, *argc as usize)?;
                    let text = self
                        .format_template(&prog.templates[*tpl as usize], &args)
                        .map_err(Trap::Panic)?;
                    let target: &mut dyn Write =
                        if *stderr { &mut self.err } else { &mut self.out };
                    if *newline {
                        write!(target, "{text}\n")
                    } else {
                        write!(target, "{text}")
                    }
                    .map_err(|e| Trap::Panic(format!("I/O error: {e}")))?;
                    stack.push(Value::Unit);
                }
                Op::Format { tpl, argc } => {
                    let args = take(&mut stack, *argc as usize)?;
                    let text = self
                        .format_template(&prog.templates[*tpl as usize], &args)
                        .map_err(Trap::Panic)?;
                    stack.push(Value::new_str(text));
                }
                Op::Panic { tpl, argc } => {
                    let args = take(&mut stack, *argc as usize)?;
                    let text = self
                        .format_template(&prog.templates[*tpl as usize], &args)
                        .map_err(Trap::Panic)?;
                    return Err(Trap::Panic(text));
                }
                Op::AssertEq { ne } => {
                    let b = pop(&mut stack)?;
                    let a = pop(&mut stack)?;
                    if a.equals(&b) == *ne {
                        let op = if *ne { "!=" } else { "==" };
                        return Err(Trap::Panic(format!(
                            "assertion `left {op} right` failed\n  left: {}\n right: {}",
                            a.debug(),
                            b.debug()
                        )));
                    }
                }
                Op::Dump => {
                    let v = stack
                        .last()
                        .ok_or_else(|| Trap::Panic("operand stack underflow".into()))?
                        .clone();
                    let _ = writeln!(self.err, "[dbg] {}", v.debug());
                }
                Op::LoadGlobal(i) => stack.push(self.globals[*i as usize].clone()),
                Op::StoreGlobal(i) => {
                    let v = pop(&mut stack)?;
                    self.globals[*i as usize] = v;
                }
                Op::IterGet(t) => {
                    let idx = pop(&mut stack)?;
                    let c = pop(&mut stack)?;
                    match c {
                        Value::Vec(rc) => {
                            let i = idx.as_int().map_err(Trap::Panic)?;
                            let v = rc.borrow();
                            if i < 0 || i as usize >= v.len() {
                                pc = *t as usize;
                            } else {
                                let item = v[i as usize].clone();
                                drop(v);
                                stack.push(item);
                            }
                        }
                        other => {
                            return Err(Trap::Panic(format!(
                                "cannot iterate over {}",
                                other.type_name()
                            )))
                        }
                    }
                }
                Op::Exit => {
                    let code_arg = pop(&mut stack)?.as_int().map_err(Trap::Panic)?;
                    return Err(Trap::Exit(code_arg as i32));
                }
            }
        }
    }

    fn dispatch(
        &self,
        id: MethodId,
        recv: Value,
        args: Vec<Value>,
    ) -> Result<Value, String> {
        let arg = |i: usize| -> Result<&Value, String> {
            args.get(i)
                .ok_or_else(|| format!("`{:?}` needs more arguments", id))
        };
        match id {
            MethodId::Len => match &recv {
                Value::Str(s) => Ok(Value::Int(s.borrow().len() as i64)),
                Value::Vec(v) => Ok(Value::Int(v.borrow().len() as i64)),
                Value::Tuple(t) => Ok(Value::Int(t.len() as i64)),
                other => Err(format!("`len` is not defined for {}", other.type_name())),
            },
            MethodId::IsEmpty => match &recv {
                Value::Str(s) => Ok(Value::Bool(s.borrow().is_empty())),
                Value::Vec(v) => Ok(Value::Bool(v.borrow().is_empty())),
                other => Err(format!("`is_empty` is not defined for {}", other.type_name())),
            },
            MethodId::Push => match recv {
                Value::Vec(v) => {
                    v.borrow_mut().push(arg(0)?.clone());
                    Ok(Value::Unit)
                }
                Value::Str(s) => {
                    let c = match arg(0)? {
                        Value::Char(c) => *c,
                        other => {
                            return Err(format!(
                                "String::push expects a char, found {}",
                                other.type_name()
                            ))
                        }
                    };
                    s.borrow_mut().push(c);
                    Ok(Value::Unit)
                }
                other => Err(format!("`push` is not defined for {}", other.type_name())),
            },
            MethodId::PushStr => match recv {
                Value::Str(s) => {
                    let add = arg(0)?.borrow_str()?;
                    s.borrow_mut().push_str(&add);
                    Ok(Value::Unit)
                }
                other => Err(format!("`push_str` is not defined for {}", other.type_name())),
            },
            MethodId::Pop => match recv {
                Value::Vec(v) => Ok(v.borrow_mut().pop().unwrap_or(Value::Unit)),
                other => Err(format!("`pop` is not defined for {}", other.type_name())),
            },
            MethodId::Clear => match &recv {
                Value::Vec(v) => {
                    v.borrow_mut().clear();
                    Ok(Value::Unit)
                }
                Value::Str(s) => {
                    s.borrow_mut().clear();
                    Ok(Value::Unit)
                }
                other => Err(format!("`clear` is not defined for {}", other.type_name())),
            },
            MethodId::Insert => match &recv {
                Value::Vec(v) => {
                    let i = arg(0)?.as_int()?;
                    let mut v = v.borrow_mut();
                    if i < 0 || i as usize > v.len() {
                        return Err(format!("insert index {i} out of bounds (len {})", v.len()));
                    }
                    v.insert(i as usize, arg(1)?.clone());
                    Ok(Value::Unit)
                }
                other => Err(format!("`insert` is not defined for {}", other.type_name())),
            },
            MethodId::Remove => match &recv {
                Value::Vec(v) => {
                    let i = arg(0)?.as_int()?;
                    let mut v = v.borrow_mut();
                    if i < 0 || i as usize >= v.len() {
                        return Err(format!("removal index {i} out of bounds (len {})", v.len()));
                    }
                    Ok(v.remove(i as usize))
                }
                other => Err(format!("`remove` is not defined for {}", other.type_name())),
            },
            MethodId::Reverse => match &recv {
                Value::Vec(v) => {
                    v.borrow_mut().reverse();
                    Ok(Value::Unit)
                }
                other => Err(format!("`reverse` is not defined for {}", other.type_name())),
            },
            MethodId::Sort => match &recv {
                Value::Vec(v) => {
                    let mut v = v.borrow_mut();
                    if let Some(first) = v.first() {
                        for item in v.iter() {
                            compare(first, item)?;
                        }
                    }
                    v.sort_by(|a, b| compare(a, b).unwrap_or(std::cmp::Ordering::Equal));
                    Ok(Value::Unit)
                }
                other => Err(format!("`sort` is not defined for {}", other.type_name())),
            },
            MethodId::Contains => match &recv {
                Value::Vec(v) => {
                    let needle = arg(0)?;
                    Ok(Value::Bool(v.borrow().iter().any(|x| x.equals(needle))))
                }
                Value::Str(s) => {
                    let needle = arg(0)?.borrow_str()?;
                    Ok(Value::Bool(s.borrow().contains(&*needle)))
                }
                other => Err(format!("`contains` is not defined for {}", other.type_name())),
            },
            MethodId::First | MethodId::Last => match &recv {
                Value::Vec(v) => {
                    let v = v.borrow();
                    let item = if id == MethodId::First { v.first() } else { v.last() };
                    Ok(item.cloned().unwrap_or(Value::Unit))
                }
                other => Err(format!("`{:?}` is not defined for {}", id, other.type_name())),
            },
            MethodId::Join => match &recv {
                Value::Vec(v) => {
                    let sep = arg(0)?.borrow_str()?;
                    let v = v.borrow();
                    let mut parts = Vec::with_capacity(v.len());
                    for item in v.iter() {
                        parts.push(item.borrow_str()?.clone());
                    }
                    Ok(Value::new_str(parts.join(&sep)))
                }
                other => Err(format!("`join` is not defined for {}", other.type_name())),
            },
            MethodId::Trim | MethodId::TrimStart | MethodId::TrimEnd => match &recv {
                Value::Str(s) => {
                    let s = s.borrow();
                    let out = match id {
                        MethodId::Trim => s.trim().to_string(),
                        MethodId::TrimStart => s.trim_start().to_string(),
                        _ => s.trim_end().to_string(),
                    };
                    Ok(Value::new_str(out))
                }
                other => Err(format!("`{:?}` is not defined for {}", id, other.type_name())),
            },
            MethodId::StartsWith | MethodId::EndsWith => match &recv {
                Value::Str(s) => {
                    let pat = arg(0)?.borrow_str()?;
                    let r = if id == MethodId::StartsWith {
                        s.borrow().starts_with(&*pat)
                    } else {
                        s.borrow().ends_with(&*pat)
                    };
                    Ok(Value::Bool(r))
                }
                other => Err(format!("`{:?}` is not defined for {}", id, other.type_name())),
            },
            MethodId::Replace => match &recv {
                Value::Str(s) => {
                    let from = arg(0)?.borrow_str()?.clone();
                    let to = arg(1)?.borrow_str()?.clone();
                    Ok(Value::new_str(s.borrow().replace(&from, &to)))
                }
                other => Err(format!("`replace` is not defined for {}", other.type_name())),
            },
            MethodId::Repeat => match &recv {
                Value::Str(s) => {
                    let n = arg(0)?.as_int()?;
                    if n < 0 || n > 50_000_000 {
                        return Err(format!("bad repeat count {n}"));
                    }
                    Ok(Value::new_str(s.borrow().repeat(n as usize)))
                }
                Value::Vec(v) => {
                    let n = arg(0)?.as_int()?;
                    if n < 0 || n > 50_000_000 {
                        return Err(format!("bad repeat count {n}"));
                    }
                    let v = v.borrow();
                    let mut out = Vec::with_capacity(v.len() * n as usize);
                    for _ in 0..n {
                        for item in v.iter() {
                            out.push(item.deep_clone());
                        }
                    }
                    Ok(Value::new_vec(out))
                }
                other => Err(format!("`repeat` is not defined for {}", other.type_name())),
            },
            MethodId::ToUpper | MethodId::ToLower => match &recv {
                Value::Str(s) => {
                    let s = s.borrow();
                    Ok(Value::new_str(if id == MethodId::ToUpper {
                        s.to_uppercase()
                    } else {
                        s.to_lowercase()
                    }))
                }
                other => Err(format!("`{:?}` is not defined for {}", id, other.type_name())),
            },
            MethodId::Split => match &recv {
                Value::Str(s) => {
                    let sep = arg(0)?.borrow_str()?.clone();
                    let s = s.borrow();
                    let parts: Vec<Value> = s.split(&*sep).map(Value::new_str).collect();
                    Ok(Value::new_vec(parts))
                }
                other => Err(format!("`split` is not defined for {}", other.type_name())),
            },
            MethodId::SplitWhitespace => match &recv {
                Value::Str(s) => {
                    let s = s.borrow();
                    let parts: Vec<Value> = s.split_whitespace().map(Value::new_str).collect();
                    Ok(Value::new_vec(parts))
                }
                other => {
                    Err(format!("`split_whitespace` is not defined for {}", other.type_name()))
                }
            },
            MethodId::Chars => match &recv {
                Value::Str(s) => {
                    let chars: Vec<Value> = s.borrow().chars().map(Value::Char).collect();
                    Ok(Value::new_vec(chars))
                }
                other => Err(format!("`chars` is not defined for {}", other.type_name())),
            },
            MethodId::Abs => match recv {
                Value::Int(x) => Ok(Value::Int(x.wrapping_abs())),
                Value::Float(x) => Ok(Value::Float(x.abs())),
                other => Err(format!("`abs` is not defined for {}", other.type_name())),
            },
            MethodId::Pow => match recv {
                Value::Int(x) => {
                    let e = arg(0)?.as_int()?;
                    if e < 0 {
                        return Err("attempt to raise an integer to a negative power".into());
                    }
                    Ok(Value::Int(x.wrapping_pow(e as u32)))
                }
                Value::Float(x) => {
                    let e = arg(0)?.as_f64()?;
                    Ok(Value::Float(x.powf(e)))
                }
                other => Err(format!("`pow` is not defined for {}", other.type_name())),
            },
            MethodId::Powf => match recv {
                Value::Float(x) => Ok(Value::Float(x.powf(arg(0)?.as_f64()?))),
                other => Err(format!("`powf` is not defined for {}", other.type_name())),
            },
            MethodId::Powi => match recv {
                Value::Float(x) => Ok(Value::Float(x.powi(arg(0)?.as_int()? as i32))),
                other => Err(format!("`powi` is not defined for {}", other.type_name())),
            },
            MethodId::Sqrt | MethodId::Floor | MethodId::Ceil | MethodId::Round => match recv {
                Value::Float(x) => Ok(Value::Float(match id {
                    MethodId::Sqrt => x.sqrt(),
                    MethodId::Floor => x.floor(),
                    MethodId::Ceil => x.ceil(),
                    _ => x.round(),
                })),
                other => Err(format!("`{:?}` is not defined for {}", id, other.type_name())),
            },
            MethodId::Min | MethodId::Max => {
                let a = &recv;
                let b = arg(0)?;
                let ord = compare(a, b)?;
                let take_b = if id == MethodId::Min {
                    ord == std::cmp::Ordering::Greater
                } else {
                    ord == std::cmp::Ordering::Less
                };
                Ok(if take_b { b.clone() } else { recv })
            }
            MethodId::Clamp => {
                let lo = arg(0)?;
                let hi = arg(1)?;
                if compare(&recv, lo)? == std::cmp::Ordering::Less {
                    Ok(lo.clone())
                } else if compare(&recv, hi)? == std::cmp::Ordering::Greater {
                    Ok(hi.clone())
                } else {
                    Ok(recv)
                }
            }
            MethodId::Signum => match recv {
                Value::Int(x) => Ok(Value::Int(x.signum())),
                Value::Float(x) => Ok(Value::Float(if x == 0.0 { 0.0 } else { x.signum() })),
                other => Err(format!("`signum` is not defined for {}", other.type_name())),
            },
            MethodId::WrappingAdd | MethodId::WrappingSub | MethodId::WrappingMul => {
                let x = recv.as_int()?;
                let y = arg(0)?.as_int()?;
                Ok(Value::Int(match id {
                    MethodId::WrappingAdd => x.wrapping_add(y),
                    MethodId::WrappingSub => x.wrapping_sub(y),
                    _ => x.wrapping_mul(y),
                }))
            }
            MethodId::SaturatingAdd | MethodId::SaturatingSub | MethodId::SaturatingMul => {
                let x = recv.as_int()?;
                let y = arg(0)?.as_int()?;
                Ok(Value::Int(match id {
                    MethodId::SaturatingAdd => x.saturating_add(y),
                    MethodId::SaturatingSub => x.saturating_sub(y),
                    _ => x.saturating_mul(y),
                }))
            }
            MethodId::ToString => Ok(Value::new_str(recv.display())),
            MethodId::Clone => Ok(recv.deep_clone()),
        }
    }
}
