//! AST -> stack bytecode.

use crate::ast::*;
use crate::ops::{self, CastTo};
use crate::value::{parse_spec, Value};
use std::collections::HashMap;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MethodId {
    Len,
    IsEmpty,
    Push,
    PushStr,
    Pop,
    Clear,
    Insert,
    Remove,
    Reverse,
    Sort,
    Contains,
    First,
    Last,
    Join,
    Trim,
    TrimStart,
    TrimEnd,
    StartsWith,
    EndsWith,
    Replace,
    Repeat,
    ToUpper,
    ToLower,
    Split,
    SplitWhitespace,
    Chars,
    Abs,
    Pow,
    Powf,
    Powi,
    Sqrt,
    Floor,
    Ceil,
    Round,
    Min,
    Max,
    Clamp,
    Signum,
    WrappingAdd,
    WrappingSub,
    WrappingMul,
    SaturatingAdd,
    SaturatingSub,
    SaturatingMul,
    ToString,
    Clone,
}

pub fn method_id(name: &str) -> Option<MethodId> {
    Some(match name {
        "len" => MethodId::Len,
        "is_empty" => MethodId::IsEmpty,
        "push" => MethodId::Push,
        "push_str" => MethodId::PushStr,
        "pop" => MethodId::Pop,
        "clear" => MethodId::Clear,
        "insert" => MethodId::Insert,
        "remove" => MethodId::Remove,
        "reverse" => MethodId::Reverse,
        "sort" => MethodId::Sort,
        "contains" => MethodId::Contains,
        "first" => MethodId::First,
        "last" => MethodId::Last,
        "join" => MethodId::Join,
        "trim" => MethodId::Trim,
        "trim_start" => MethodId::TrimStart,
        "trim_end" => MethodId::TrimEnd,
        "starts_with" => MethodId::StartsWith,
        "ends_with" => MethodId::EndsWith,
        "replace" => MethodId::Replace,
        "repeat" => MethodId::Repeat,
        "to_uppercase" => MethodId::ToUpper,
        "to_lowercase" => MethodId::ToLower,
        "split" => MethodId::Split,
        "split_whitespace" => MethodId::SplitWhitespace,
        "chars" => MethodId::Chars,
        "abs" => MethodId::Abs,
        "pow" => MethodId::Pow,
        "powf" => MethodId::Powf,
        "powi" => MethodId::Powi,
        "sqrt" => MethodId::Sqrt,
        "floor" => MethodId::Floor,
        "ceil" => MethodId::Ceil,
        "round" => MethodId::Round,
        "min" => MethodId::Min,
        "max" => MethodId::Max,
        "clamp" => MethodId::Clamp,
        "signum" => MethodId::Signum,
        "wrapping_add" => MethodId::WrappingAdd,
        "wrapping_sub" => MethodId::WrappingSub,
        "wrapping_mul" => MethodId::WrappingMul,
        "saturating_add" => MethodId::SaturatingAdd,
        "saturating_sub" => MethodId::SaturatingSub,
        "saturating_mul" => MethodId::SaturatingMul,
        "to_string" => MethodId::ToString,
        "clone" => MethodId::Clone,
        _ => return None,
    })
}

#[derive(Clone, Debug)]
pub enum TPiece {
    Lit(String),
    Arg { idx: usize, spec: String },
}

#[derive(Clone, Debug)]
pub enum Op {
    Const(u32),
    Imm(i64),
    Unit,
    Load(u16),
    Store(u16),
    Pop,
    Dup2,
    Bin(BinOp),
    Un(UnOp),
    Cast(CastTo),
    Jump(u32),
    JumpF(u32),
    JumpT(u32),
    Call(u32),
    Exit,
    NewString,
    NewVec,
    MakeArray(u32),
    MakeTuple(u32),
    VecRepeat,
    Index,
    IndexSet,
    TupleGet(u32),
    Method(MethodId, u16),
    Print {
        tpl: u32,
        argc: u16,
        newline: bool,
        stderr: bool,
    },
    Format {
        tpl: u32,
        argc: u16,
    },
    Panic {
        tpl: u32,
        argc: u16,
    },
    AssertEq {
        ne: bool,
    },
    Dump,
    LoadGlobal(u32),
    StoreGlobal(u32),
    IterGet(u32),
    /// Fused `load a; load b; bin op; store dst`.
    BinLocals { dst: u16, a: u16, b: u16, op: BinOp },
    /// Fused `load a; imm k; bin op; store dst`.
    BinLocalImm { dst: u16, a: u16, imm: i64, op: BinOp },
    /// Fused `load a; load b; bin op; jumpF/jumpT target`.
    CmpJump { a: u16, b: u16, op: BinOp, target: u32, jump_if: bool },
    /// Fused `load a; imm k; bin op; jumpF/jumpT target`.
    CmpImmJump { a: u16, imm: i64, op: BinOp, target: u32, jump_if: bool },
    /// Fused `load slot; jumpF/jumpT target`.
    JumpLocal { slot: u16, target: u32, jump_if: bool },
    Return,
}

pub struct Func {
    pub name: String,
    pub nparams: u16,
    pub nlocals: u16,
    pub code: Vec<Op>,
}

pub struct Bytecode {
    pub funcs: Vec<Rc<Func>>,
    pub consts: Vec<Value>,
    pub templates: Vec<Vec<TPiece>>,
    pub globals: Vec<Value>,
    pub main: u32,
}

pub struct Compiler {
    pub funcs: Vec<Rc<Func>>,
    pub consts: Vec<Value>,
    pub templates: Vec<Vec<TPiece>>,
    pub globals: Vec<Value>,
    pub global_index: HashMap<String, u32>,
    pub const_map: HashMap<String, Value>,
    pub fn_index: HashMap<String, u32>,
    pub main: u32,
}

pub fn compile(ast: ProgramAst) -> Result<Bytecode, String> {
    let mut funcs_ast: Vec<FuncAst> = Vec::new();
    let mut fn_index: HashMap<String, u32> = HashMap::new();
    for item in &ast.items {
        if let Item::Func(f) = item {
            if fn_index.contains_key(&f.name) {
                return Err(format!("duplicate definition of function `{}`", f.name));
            }
            fn_index.insert(f.name.clone(), funcs_ast.len() as u32);
            funcs_ast.push(f.clone());
        }
    }
    let main = *fn_index
        .get("main")
        .ok_or_else(|| "no `main` function found".to_string())?;

    let mut c = Compiler {
        funcs: Vec::new(),
        consts: Vec::new(),
        templates: Vec::new(),
        globals: Vec::new(),
        global_index: HashMap::new(),
        const_map: HashMap::new(),
        fn_index,
        main,
    };
    for f in &funcs_ast {
        c.funcs.push(Rc::new(Func {
            name: f.name.clone(),
            nparams: f.params.len() as u16,
            nlocals: f.params.len() as u16,
            code: Vec::new(),
        }));
    }

    // Constants can refer to each other; loop until everything resolves.
    let mut pending: Vec<&Item> = ast
        .items
        .iter()
        .filter(|i| matches!(i, Item::Const { .. }))
        .collect();
    let mut last_err = String::new();
    while !pending.is_empty() {
        let mut still = Vec::new();
        let mut progress = false;
        for item in pending {
            if let Item::Const { name, value, .. } = item {
                match c.eval_const(value) {
                    Ok(v) => {
                        c.const_map.insert(name.clone(), v);
                        progress = true;
                    }
                    Err(e) => {
                        last_err = format!("in const `{name}`: {e}");
                        still.push(item);
                    }
                }
            }
        }
        if !progress {
            return Err(last_err);
        }
        pending = still;
    }

    let mut pending: Vec<&Item> = ast
        .items
        .iter()
        .filter(|i| matches!(i, Item::Static { .. }))
        .collect();
    while !pending.is_empty() {
        let mut still = Vec::new();
        let mut progress = false;
        for item in pending {
            if let Item::Static { name, value, .. } = item {
                match c.eval_const(value) {
                    Ok(v) => {
                        c.global_index.insert(name.clone(), c.globals.len() as u32);
                        c.globals.push(v);
                        progress = true;
                    }
                    Err(e) => {
                        last_err = format!("in static `{name}`: {e}");
                        still.push(item);
                    }
                }
            }
        }
        if !progress {
            return Err(last_err);
        }
        pending = still;
    }

    for (idx, f) in funcs_ast.iter().enumerate() {
        let compiled = FnCompiler::compile(&mut c, idx as u32, f)?;
        c.funcs[idx] = Rc::new(compiled);
    }

    Ok(Bytecode {
        funcs: c.funcs,
        consts: c.consts,
        templates: c.templates,
        globals: c.globals,
        main: c.main,
    })
}

impl Compiler {
    fn path_const(&self, path: &[String]) -> Option<Value> {
        let joined = path.join("::");
        if let Some(v) = self.const_map.get(&joined) {
            return Some(v.deep_clone());
        }
        if path.len() == 2 {
            let (ty, name) = (path[0].as_str(), path[1].as_str());
            if let Some(v) = int_limit(ty, name) {
                return Some(v);
            }
            if let Some(v) = float_limit(ty, name) {
                return Some(v);
            }
        }
        None
    }

    fn eval_const(&self, e: &Expr) -> Result<Value, String> {
        match e {
            Expr::Int(v) => Ok(Value::Int(*v)),
            Expr::Float(v) => Ok(Value::Float(*v)),
            Expr::Bool(v) => Ok(Value::Bool(*v)),
            Expr::Str(s) => Ok(Value::new_str(s.clone())),
            Expr::Char(c) => Ok(Value::Char(*c)),
            Expr::Ident(name) => self
                .const_map
                .get(name)
                .map(|v| v.deep_clone())
                .ok_or_else(|| format!("unknown constant `{name}`")),
            Expr::Path(path) => self
                .path_const(path)
                .ok_or_else(|| format!("unknown constant `{}`", path.join("::"))),
            Expr::Unary(UnOp::Deref, inner) | Expr::Ref(inner) => self.eval_const(inner),
            Expr::Unary(op, inner) => {
                let v = self.eval_const(inner)?;
                ops::unop(*op, &v)
            }
            Expr::Binary(op, a, b) => {
                let a = self.eval_const(a)?;
                let b = self.eval_const(b)?;
                ops::binop(*op, &a, &b)
            }
            Expr::Cast(inner, ty) => {
                let v = self.eval_const(inner)?;
                let to = ops::cast_to_from_type(ty)
                    .ok_or_else(|| format!("unsupported cast to {ty:?}"))?;
                ops::cast(&v, to)
            }
            Expr::Tuple(items) => {
                let mut vals = Vec::with_capacity(items.len());
                for i in items {
                    vals.push(self.eval_const(i)?);
                }
                Ok(Value::Tuple(Rc::new(vals)))
            }
            Expr::Array(items) => {
                let mut vals = Vec::with_capacity(items.len());
                for i in items {
                    vals.push(self.eval_const(i)?);
                }
                Ok(Value::new_vec(vals))
            }
            Expr::Repeat(item, n) => {
                let item = self.eval_const(item)?;
                let n = self.eval_const(n)?.as_int()?;
                if n < 0 || n > 50_000_000 {
                    return Err(format!("bad repeat count {n}"));
                }
                let mut vals = Vec::with_capacity(n as usize);
                for _ in 0..n {
                    vals.push(item.deep_clone());
                }
                Ok(Value::new_vec(vals))
            }
            other => Err(format!("not a constant expression: {other:?}")),
        }
    }
}

fn int_limit(ty: &str, name: &str) -> Option<Value> {
    let kind = IntKind::from_name(ty)?;
    let bits = kind.bits();
    let (min, max) = if kind.is_signed() {
        if bits == 64 {
            (i64::MIN, i64::MAX)
        } else {
            (-(1i64 << (bits - 1)), (1i64 << (bits - 1)) - 1)
        }
    } else if bits == 64 {
        (0, -1) // u64 range, stored as a bit pattern
    } else {
        (0, (1i64 << bits) - 1)
    };
    Some(Value::Int(match name {
        "MAX" => max,
        "MIN" => min,
        _ => return None,
    }))
}

fn float_limit(ty: &str, name: &str) -> Option<Value> {
    let f = match ty {
        "f64" => match name {
            "INFINITY" => f64::INFINITY,
            "NEG_INFINITY" => f64::NEG_INFINITY,
            "NAN" => f64::NAN,
            "PI" => std::f64::consts::PI,
            "E" => std::f64::consts::E,
            "EPSILON" => f64::EPSILON,
            "MAX" => f64::MAX,
            "MIN" => f64::MIN,
            _ => return None,
        },
        "f32" => match name {
            "INFINITY" => f32::INFINITY as f64,
            "NEG_INFINITY" => f32::NEG_INFINITY as f64,
            "NAN" => f32::NAN as f64,
            "PI" => std::f32::consts::PI as f64,
            "E" => std::f32::consts::E as f64,
            "EPSILON" => f32::EPSILON as f64,
            "MAX" => f32::MAX as f64,
            "MIN" => f32::MIN as f64,
            _ => return None,
        },
        _ => return None,
    };
    Some(Value::Float(f))
}

struct LocalVar {
    name: String,
}

struct LoopCtx {
    breaks: Vec<usize>,
    continues: Vec<usize>,
    value_slot: Option<u16>,
}

struct FnCompiler<'a> {
    c: &'a mut Compiler,
    code: Vec<Op>,
    locals: Vec<LocalVar>,
    max_locals: u16,
    scopes: Vec<usize>,
    loops: Vec<LoopCtx>,
}

impl<'a> FnCompiler<'a> {
    fn compile(c: &'a mut Compiler, _idx: u32, f: &FuncAst) -> Result<Func, String> {
        let mut fc = FnCompiler {
            c,
            code: Vec::new(),
            locals: Vec::new(),
            max_locals: 0,
            scopes: Vec::new(),
            loops: Vec::new(),
        };
        for (pat, _ty) in &f.params {
            match pat {
                Pat::Name(n) => {
                    fc.alloc_local(n);
                }
                Pat::Wild => {
                    fc.alloc_local("_");
                }
                Pat::Tuple(_) => {
                    return Err(format!(
                        "destructuring parameters are not supported in `{}`",
                        f.name
                    ))
                }
            }
        }
        fc.scope_enter();
        fc.compile_block(&f.body)?;
        fc.scope_exit();
        fc.emit(Op::Return);
        Ok(Func {
            name: f.name.clone(),
            nparams: f.params.len() as u16,
            nlocals: fc.max_locals,
            code: peephole(fc.code),
        })
    }

    fn emit(&mut self, op: Op) -> usize {
        let i = self.code.len();
        self.code.push(op);
        i
    }

    fn emit_jump(&mut self, op: Op) -> usize {
        self.emit(op)
    }

    fn patch(&mut self, at: usize, target: u32) {
        match &mut self.code[at] {
            Op::Jump(t) | Op::JumpF(t) | Op::JumpT(t) | Op::IterGet(t) => *t = target,
            other => panic!("cannot patch {:?}", other),
        }
    }

    fn here(&self) -> u32 {
        self.code.len() as u32
    }

    fn push_const(&mut self, v: Value) -> u32 {
        let idx = self.c.consts.len() as u32;
        self.c.consts.push(v);
        idx
    }

    fn emit_const(&mut self, v: Value) {
        if let Value::Int(i) = v {
            self.emit(Op::Imm(i));
            return;
        }
        let idx = self.push_const(v);
        self.emit(Op::Const(idx));
    }

    fn add_template(&mut self, s: &str) -> Result<u32, String> {
        let pieces = parse_template(s)?;
        let idx = self.c.templates.len() as u32;
        self.c.templates.push(pieces);
        Ok(idx)
    }

    fn alloc_local(&mut self, name: &str) -> u16 {
        let slot = self.locals.len() as u16;
        self.locals.push(LocalVar { name: name.to_string() });
        if slot + 1 > self.max_locals {
            self.max_locals = slot + 1;
        }
        slot
    }

    fn scope_enter(&mut self) {
        self.scopes.push(self.locals.len());
    }

    fn scope_exit(&mut self) {
        if let Some(n) = self.scopes.pop() {
            self.locals.truncate(n);
        }
    }

    fn resolve_local(&self, name: &str) -> Option<u16> {
        self.locals
            .iter()
            .rposition(|l| l.name == name)
            .map(|i| i as u16)
    }

    fn load_var(&mut self, name: &str) -> Result<(), String> {
        if let Some(slot) = self.resolve_local(name) {
            self.emit(Op::Load(slot));
            return Ok(());
        }
        if let Some(g) = self.c.global_index.get(name) {
            let g = *g;
            self.emit(Op::LoadGlobal(g));
            return Ok(());
        }
        if let Some(v) = self.c.const_map.get(name) {
            let v = v.deep_clone();
            self.emit_const(v);
            return Ok(());
        }
        if self.c.fn_index.contains_key(name) {
            return Err(format!("function `{name}` used as a value (mini-rust has no first-class functions)"));
        }
        Err(format!("cannot find value `{name}` in this scope"))
    }

    fn store_var(&mut self, name: &str) -> Result<(), String> {
        if let Some(slot) = self.resolve_local(name) {
            self.emit(Op::Store(slot));
            return Ok(());
        }
        if let Some(g) = self.c.global_index.get(name) {
            let g = *g;
            self.emit(Op::StoreGlobal(g));
            return Ok(());
        }
        Err(format!("cannot assign to `{name}` (constants are immutable)"))
    }

    fn scope_has_local(&self, name: &str) -> bool {
        self.resolve_local(name).is_some()
    }

    fn compile_block(&mut self, b: &Block) -> Result<(), String> {
        self.scope_enter();
        let n = b.stmts.len();
        let mut last_is_value = false;
        for (i, s) in b.stmts.iter().enumerate() {
            match s {
                Stmt::Let { pat, ty, init, .. } => {
                    self.compile_let(pat, ty.as_ref(), init.as_ref())?;
                }
                Stmt::Expr { expr, semi } => {
                    let is_last = i + 1 == n;
                    self.compile_expr(expr)?;
                    let diverges = diverges(expr);
                    if *semi || !is_last {
                        if !diverges {
                            self.emit(Op::Pop);
                        }
                    } else {
                        last_is_value = !diverges;
                    }
                }
            }
        }
        if !last_is_value {
            self.emit(Op::Unit);
        }
        self.scope_exit();
        Ok(())
    }

    fn compile_let(
        &mut self,
        pat: &Pat,
        ty: Option<&Type>,
        init: Option<&Expr>,
    ) -> Result<(), String> {
        match init {
            Some(e) => {
                self.compile_expr(e)?;
                self.bind_pat(pat)?;
            }
            None => {
                self.default_value(ty)?;
                self.bind_pat(pat)?;
            }
        }
        Ok(())
    }

    fn default_value(&mut self, ty: Option<&Type>) -> Result<(), String> {
        match ty {
            Some(Type::Int(_)) => self.emit_const(Value::Int(0)),
            Some(Type::F32) | Some(Type::F64) => self.emit_const(Value::Float(0.0)),
            Some(Type::Bool) => self.emit_const(Value::Bool(false)),
            Some(Type::Char) => self.emit_const(Value::Char('\0')),
            Some(Type::Str) | Some(Type::String) => self.emit_const(Value::new_str("")),
            Some(Type::Vec(_)) | Some(Type::Slice(_)) => {
                self.emit(Op::NewVec);
            }
            Some(Type::Tuple(ts)) => {
                let ts = ts.clone();
                for t in &ts {
                    self.default_value(Some(t))?;
                }
                self.emit(Op::MakeTuple(ts.len() as u32));
            }
            Some(Type::Ref(inner)) => {
                let inner = (**inner).clone();
                self.default_value(Some(&inner))?;
            }
            None => {
                return Err(
                    "a `let` without an initializer needs a type annotation in mini-rust".into(),
                )
            }
            Some(_) => {
                self.emit(Op::Unit);
            }
        }
        Ok(())
    }

    fn bind_pat(&mut self, pat: &Pat) -> Result<(), String> {
        match pat {
            Pat::Name(n) => {
                let slot = self.alloc_local(n);
                self.emit(Op::Store(slot));
            }
            Pat::Wild => {
                self.emit(Op::Pop);
            }
            Pat::Tuple(ps) => {
                let tmp = self.alloc_local("$tuple");
                self.emit(Op::Store(tmp));
                for (i, p) in ps.iter().enumerate() {
                    self.emit(Op::Load(tmp));
                    self.emit(Op::TupleGet(i as u32));
                    self.bind_pat(p)?;
                }
            }
        }
        Ok(())
    }

    fn compile_expr(&mut self, e: &Expr) -> Result<(), String> {
        match e {
            Expr::Int(v) => self.emit_const(Value::Int(*v)),
            Expr::Float(v) => self.emit_const(Value::Float(*v)),
            Expr::Bool(v) => self.emit_const(Value::Bool(*v)),
            Expr::Str(s) => self.emit_const(Value::new_str(s.clone())),
            Expr::Char(c) => self.emit_const(Value::Char(*c)),
            Expr::Ident(name) => self.load_var(name)?,
            Expr::Path(path) => {
                let v = self
                    .c
                    .path_const(path)
                    .ok_or_else(|| format!("cannot resolve `{}`", path.join("::")))?;
                self.emit_const(v);
            }
            Expr::Unary(UnOp::Deref, inner) => self.compile_expr(inner)?,
            Expr::Unary(op, inner) => {
                self.compile_expr(inner)?;
                self.emit(Op::Un(*op));
            }
            Expr::Binary(BinOp::And, a, b) => {
                self.compile_expr(a)?;
                let jf = self.emit_jump(Op::JumpF(0));
                self.compile_expr(b)?;
                let jend = self.emit_jump(Op::Jump(0));
                let short = self.here();
                self.patch(jf, short);
                self.emit_const(Value::Bool(false));
                let end = self.here();
                self.patch(jend, end);
            }
            Expr::Binary(BinOp::Or, a, b) => {
                self.compile_expr(a)?;
                let jt = self.emit_jump(Op::JumpT(0));
                self.compile_expr(b)?;
                let jend = self.emit_jump(Op::Jump(0));
                let short = self.here();
                self.patch(jt, short);
                self.emit_const(Value::Bool(true));
                let end = self.here();
                self.patch(jend, end);
            }
            Expr::Binary(op, a, b) => {
                self.compile_expr(a)?;
                self.compile_expr(b)?;
                self.emit(Op::Bin(*op));
            }
            Expr::Assign(op, place, rhs) => {
                self.compile_assign(op, place, rhs)?;
                self.emit(Op::Unit);
            }
            Expr::Call(callee, args) => self.compile_call(callee, args)?,
            Expr::Method(recv, name, turbofish, args) => {
                self.compile_method(recv, name, turbofish, args)?;
            }
            Expr::Index(obj, idx) => {
                self.compile_expr(obj)?;
                self.compile_expr(idx)?;
                self.emit(Op::Index);
            }
            Expr::Field(obj, name) => {
                if let Ok(i) = name.parse::<usize>() {
                    self.compile_expr(obj)?;
                    self.emit_const(Value::Int(i as i64));
                    self.emit(Op::Index);
                } else {
                    return Err(format!(
                        "field access `.{name}` is outside the mini-rust subset (no structs)"
                    ));
                }
            }
            Expr::Cast(inner, ty) => {
                self.compile_expr(inner)?;
                let to = ops::cast_to_from_type(ty)
                    .ok_or_else(|| format!("unsupported cast to {ty:?}"))?;
                self.emit(Op::Cast(to));
            }
            Expr::Ref(inner) => self.compile_expr(inner)?,
            Expr::Array(items) => {
                for i in items {
                    self.compile_expr(i)?;
                }
                self.emit(Op::MakeArray(items.len() as u32));
            }
            Expr::Repeat(item, n) => {
                self.compile_expr(item)?;
                self.compile_expr(n)?;
                self.emit(Op::VecRepeat);
            }
            Expr::Tuple(items) => {
                for i in items {
                    self.compile_expr(i)?;
                }
                self.emit(Op::MakeTuple(items.len() as u32));
            }
            Expr::Block(b) => self.compile_block(b)?,
            Expr::If { cond, then_b, else_b } => {
                self.compile_expr(cond)?;
                let jf = self.emit_jump(Op::JumpF(0));
                self.compile_block(then_b)?;
                let jend = self.emit_jump(Op::Jump(0));
                let else_start = self.here();
                self.patch(jf, else_start);
                match else_b {
                    Some(e) => self.compile_expr(e)?,
                    None => {
                        self.emit(Op::Unit);
                    }
                }
                let end = self.here();
                self.patch(jend, end);
            }
            Expr::While { cond, body, .. } => {
                let start = self.here();
                self.loops.push(LoopCtx {
                    breaks: Vec::new(),
                    continues: Vec::new(),
                    value_slot: None,
                });
                self.compile_expr(cond)?;
                let jf = self.emit_jump(Op::JumpF(0));
                self.compile_block(body)?;
                self.emit(Op::Pop);
                let start_addr = start;
                self.emit(Op::Jump(start_addr));
                let end = self.here();
                self.patch(jf, end);
                let ctx = self.loops.pop().unwrap();
                for j in ctx.breaks {
                    self.patch(j, end);
                }
                for j in ctx.continues {
                    self.patch(j, start_addr);
                }
                self.emit(Op::Unit);
            }
            Expr::Loop { body, .. } => {
                // `loop` is an expression: `break value` produces the value.
                let tmp = self.alloc_local("$loop_result");
                self.emit(Op::Unit);
                self.emit(Op::Store(tmp));
                let start = self.here();
                self.loops.push(LoopCtx {
                    breaks: Vec::new(),
                    continues: Vec::new(),
                    value_slot: Some(tmp),
                });
                self.compile_block(body)?;
                self.emit(Op::Pop);
                self.emit(Op::Jump(start));
                let end = self.here();
                let ctx = self.loops.pop().unwrap();
                for j in ctx.breaks {
                    self.patch(j, end);
                }
                for j in ctx.continues {
                    self.patch(j, start);
                }
                self.emit(Op::Load(tmp));
            }
            Expr::For { pat, iter, body, .. } => self.compile_for(pat, iter, body)?,
            Expr::Range { .. } => {
                return Err("a range is only supported as the iterator of a `for` loop".into())
            }
            Expr::Macro { name, args, sep } => self.compile_macro(name, args, *sep)?,
            Expr::Break(value) => {
                let slot = self
                    .loops
                    .last()
                    .ok_or_else(|| "`break` outside of a loop".to_string())?
                    .value_slot;
                match value {
                    Some(v) => self.compile_expr(v)?,
                    None => {
                        self.emit(Op::Unit);
                    }
                }
                match slot {
                    Some(s) => {
                        self.emit(Op::Store(s));
                    }
                    None => {
                        self.emit(Op::Pop);
                    }
                }
                let j = self.emit_jump(Op::Jump(0));
                self.loops.last_mut().unwrap().breaks.push(j);
            }
            Expr::Continue => {
                if self.loops.is_empty() {
                    return Err("`continue` outside of a loop".into());
                }
                let j = self.emit_jump(Op::Jump(0));
                self.loops.last_mut().unwrap().continues.push(j);
            }
            Expr::Return(value) => {
                match value {
                    Some(v) => self.compile_expr(v)?,
                    None => {
                        self.emit(Op::Unit);
                    }
                }
                self.emit(Op::Return);
            }
        }
        Ok(())
    }

    fn compile_assign(
        &mut self,
        op: &Option<BinOp>,
        place: &Expr,
        rhs: &Expr,
    ) -> Result<(), String> {
        match place {
            Expr::Ident(name) => {
                if let Some(binop) = op {
                    self.load_var(name)?;
                    self.compile_expr(rhs)?;
                    self.emit(Op::Bin(*binop));
                } else {
                    self.compile_expr(rhs)?;
                }
                self.store_var(name)?;
            }
            Expr::Index(obj, idx) => {
                self.compile_expr(obj)?;
                self.compile_expr(idx)?;
                if let Some(binop) = op {
                    self.emit(Op::Dup2);
                    self.emit(Op::Index);
                    self.compile_expr(rhs)?;
                    self.emit(Op::Bin(*binop));
                } else {
                    self.compile_expr(rhs)?;
                }
                self.emit(Op::IndexSet);
            }
            Expr::Unary(UnOp::Deref, inner) => {
                return self.compile_assign(op, inner, rhs);
            }
            Expr::Field(_, name) => {
                return Err(format!(
                    "assignment to field `.{name}` is outside the mini-rust subset"
                ))
            }
            _ => return Err("invalid assignment target".into()),
        }
        Ok(())
    }

    fn compile_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<(), String> {
        match callee {
            Expr::Ident(name) => {
                if self.scope_has_local(name) && self.c.fn_index.contains_key(name) {
                    return Err(format!("`{name}` is a local variable, not a function"));
                }
                let idx = *self
                    .c
                    .fn_index
                    .get(name)
                    .ok_or_else(|| format!("cannot find function `{name}`"))?;
                self.check_arity(name, idx, args.len())?;
                for a in args {
                    self.compile_expr(a)?;
                }
                self.emit(Op::Call(idx));
            }
            Expr::Path(path) => {
                let joined = path.join("::");
                match joined.as_str() {
                    "String::new" => {
                        if !args.is_empty() {
                            return Err("String::new takes no arguments".into());
                        }
                        self.emit(Op::NewString);
                    }
                    "String::from" => {
                        if args.len() != 1 {
                            return Err("String::from takes exactly one argument".into());
                        }
                        self.compile_expr(&args[0])?;
                        self.emit(Op::Method(MethodId::ToString, 0));
                    }
                    "String::with_capacity" => {
                        if args.len() != 1 {
                            return Err("String::with_capacity takes exactly one argument".into());
                        }
                        self.compile_expr(&args[0])?;
                        self.emit(Op::Pop);
                        self.emit(Op::NewString);
                    }
                    "Vec::new" => {
                        if !args.is_empty() {
                            return Err("Vec::new takes no arguments".into());
                        }
                        self.emit(Op::NewVec);
                    }
                    "Vec::with_capacity" => {
                        if args.len() != 1 {
                            return Err("Vec::with_capacity takes exactly one argument".into());
                        }
                        self.compile_expr(&args[0])?;
                        self.emit(Op::Pop);
                        self.emit(Op::NewVec);
                    }
                    "std::process::exit" | "process::exit" => {
                        if args.len() != 1 {
                            return Err("exit takes exactly one argument".into());
                        }
                        self.compile_expr(&args[0])?;
                        self.emit(Op::Exit);
                    }
                    _ => {
                        let last = path.last().unwrap().clone();
                        if let Some(idx) = self
                            .c
                            .fn_index
                            .get(&joined)
                            .or_else(|| self.c.fn_index.get(&last))
                        {
                            let idx = *idx;
                            let name = last;
                            self.check_arity(&name, idx, args.len())?;
                            for a in args {
                                self.compile_expr(a)?;
                            }
                            self.emit(Op::Call(idx));
                        } else {
                            return Err(format!(
                                "cannot find function `{joined}` (mini-rust only has the functions in this file plus a tiny stdlib)"
                            ));
                        }
                    }
                }
            }
            _ => return Err("expected a function name before `(`".into()),
        }
        Ok(())
    }

    fn check_arity(&self, name: &str, idx: u32, argc: usize) -> Result<(), String> {
        let f = &self.c.funcs[idx as usize];
        if f.nparams as usize != argc {
            return Err(format!(
                "function `{name}` takes {} argument(s) but {argc} were supplied",
                f.nparams
            ));
        }
        Ok(())
    }

    fn compile_method(
        &mut self,
        recv: &Expr,
        name: &str,
        _turbofish: &[Type],
        args: &[Expr],
    ) -> Result<(), String> {
        if matches!(
            name,
            "iter" | "into_iter" | "iter_mut" | "copied" | "as_str" | "as_ref"
        ) && args.is_empty()
        {
            return self.compile_expr(recv);
        }
        if name == "to_owned" && args.is_empty() {
            self.compile_expr(recv)?;
            self.emit(Op::Method(MethodId::Clone, 0));
            return Ok(());
        }
        let id = method_id(name).ok_or_else(|| {
            format!("method `{name}` is outside the mini-rust subset")
        })?;
        self.compile_expr(recv)?;
        for a in args {
            self.compile_expr(a)?;
        }
        self.emit(Op::Method(id, args.len() as u16));
        Ok(())
    }

    fn compile_for(&mut self, pat: &Pat, iter: &Expr, body: &Block) -> Result<(), String> {
        let iter = normalize_iter(iter);
        self.scope_enter();
        match iter {
            Expr::Range { start, end, inclusive } => {
                let start = start
                    .as_deref()
                    .ok_or_else(|| "for loops need a lower bound (`0..n`)".to_string())?;
                let end = end
                    .as_deref()
                    .ok_or_else(|| "for loops need an upper bound (`0..n`)".to_string())?;
                self.compile_expr(start)?;
                let i_slot = self.alloc_local("$range_i");
                self.emit(Op::Store(i_slot));
                self.compile_expr(end)?;
                let end_slot = self.alloc_local("$range_end");
                self.emit(Op::Store(end_slot));
                let var_slot = match pat {
                    Pat::Name(n) => Some(self.alloc_local(n)),
                    Pat::Wild => None,
                    _ => return Err("unsupported `for` pattern".into()),
                };
                let start_addr = self.here();
                self.loops.push(LoopCtx {
                    breaks: Vec::new(),
                    continues: Vec::new(),
                    value_slot: None,
                });
                self.emit(Op::Load(i_slot));
                self.emit(Op::Load(end_slot));
                self.emit(Op::Bin(if *inclusive { BinOp::Le } else { BinOp::Lt }));
                let exit = self.emit_jump(Op::JumpF(0));
                if let Some(vs) = var_slot {
                    self.emit(Op::Load(i_slot));
                    self.emit(Op::Store(vs));
                }
                self.compile_block(body)?;
                self.emit(Op::Pop);
                let incr = self.here();
                self.emit(Op::Load(i_slot));
                self.emit_const(Value::Int(1));
                self.emit(Op::Bin(BinOp::Add));
                self.emit(Op::Store(i_slot));
                self.emit(Op::Jump(start_addr));
                let end_addr = self.here();
                self.patch(exit, end_addr);
                let ctx = self.loops.pop().unwrap();
                for j in ctx.breaks {
                    self.patch(j, end_addr);
                }
                for j in ctx.continues {
                    self.patch(j, incr);
                }
                self.emit(Op::Unit);
            }
            other => {
                self.compile_expr(other)?;
                let it_slot = self.alloc_local("$iter");
                self.emit(Op::Store(it_slot));
                self.emit_const(Value::Int(0));
                let idx_slot = self.alloc_local("$index");
                self.emit(Op::Store(idx_slot));
                let var_slot = match pat {
                    Pat::Name(n) => Some(self.alloc_local(n)),
                    Pat::Wild => None,
                    _ => return Err("unsupported `for` pattern".into()),
                };
                let start_addr = self.here();
                self.loops.push(LoopCtx {
                    breaks: Vec::new(),
                    continues: Vec::new(),
                    value_slot: None,
                });
                self.emit(Op::Load(it_slot));
                self.emit(Op::Load(idx_slot));
                let exit = self.emit_jump(Op::IterGet(0));
                if let Some(vs) = var_slot {
                    self.emit(Op::Store(vs));
                } else {
                    self.emit(Op::Pop);
                }
                self.compile_block(body)?;
                self.emit(Op::Pop);
                let incr = self.here();
                self.emit(Op::Load(idx_slot));
                self.emit_const(Value::Int(1));
                self.emit(Op::Bin(BinOp::Add));
                self.emit(Op::Store(idx_slot));
                self.emit(Op::Jump(start_addr));
                let end_addr = self.here();
                self.patch(exit, end_addr);
                let ctx = self.loops.pop().unwrap();
                for j in ctx.breaks {
                    self.patch(j, end_addr);
                }
                for j in ctx.continues {
                    self.patch(j, incr);
                }
                self.emit(Op::Unit);
            }
        }
        self.scope_exit();
        Ok(())
    }

    fn compile_macro(&mut self, name: &str, args: &[Expr], sep: MacroSep) -> Result<(), String> {
        match name {
            "println" | "print" | "eprintln" | "eprint" | "format" => {
                let fmt = match args.first() {
                    Some(Expr::Str(s)) => s.clone(),
                    _ => {
                        return Err(format!(
                            "`{name}!` needs a string literal format string in mini-rust"
                        ))
                    }
                };
                let rest = &args[1..];
                let tpl = self.add_template(&fmt)?;
                let argc = rest.len();
                for a in rest {
                    self.compile_expr(a)?;
                }
                if name == "format" {
                    self.emit(Op::Format { tpl, argc: argc as u16 });
                } else {
                    self.emit(Op::Print {
                        tpl,
                        argc: argc as u16,
                        newline: name.ends_with("ln"),
                        stderr: name.starts_with('e'),
                    });
                }
            }
            "vec" => {
                if sep == MacroSep::Semicolon {
                    if args.len() != 2 {
                        return Err("`vec![x; n]` takes exactly two arguments".into());
                    }
                    self.compile_expr(&args[0])?;
                    self.compile_expr(&args[1])?;
                    self.emit(Op::VecRepeat);
                } else {
                    for a in args {
                        self.compile_expr(a)?;
                    }
                    self.emit(Op::MakeArray(args.len() as u32));
                }
            }
            "panic" | "unreachable" | "todo" | "unimplemented" => {
                let (tpl_str, rest): (String, Vec<&Expr>) = match args.first() {
                    Some(Expr::Str(s)) => (s.clone(), args[1..].iter().collect()),
                    Some(_) => ("{}".to_string(), args.iter().collect()),
                    None => ("explicit panic".to_string(), Vec::new()),
                };
                let tpl = self.add_template(&tpl_str)?;
                for a in &rest {
                    self.compile_expr(a)?;
                }
                self.emit(Op::Panic { tpl, argc: rest.len() as u16 });
            }
            "assert" => {
                if args.is_empty() {
                    return Err("`assert!` needs a condition".into());
                }
                self.compile_expr(&args[0])?;
                let jt = self.emit_jump(Op::JumpT(0));
                let (tpl_str, rest): (String, Vec<&Expr>) = if args.len() > 1 {
                    match &args[1] {
                        Expr::Str(s) => (s.clone(), args[2..].iter().collect()),
                        other => ("{}".to_string(), vec![other]),
                    }
                } else {
                    ("assertion failed".to_string(), Vec::new())
                };
                let tpl = self.add_template(&tpl_str)?;
                for a in &rest {
                    self.compile_expr(a)?;
                }
                self.emit(Op::Panic { tpl, argc: rest.len() as u16 });
                let end = self.here();
                self.patch(jt, end);
                self.emit(Op::Unit);
            }
            "assert_eq" | "assert_ne" => {
                if args.len() < 2 {
                    return Err(format!("`{name}!` needs two arguments"));
                }
                self.compile_expr(&args[0])?;
                self.compile_expr(&args[1])?;
                self.emit(Op::AssertEq { ne: name == "assert_ne" });
                self.emit(Op::Unit);
            }
            "dbg" => {
                for a in args {
                    self.compile_expr(a)?;
                    self.emit(Op::Dump);
                }
                if args.is_empty() {
                    self.emit(Op::Unit);
                }
            }
            _ => {
                return Err(format!(
                    "macro `{name}!` is outside the mini-rust subset"
                ))
            }
        }
        Ok(())
    }
}

/// A tiny peephole pass: fuse the most common 2-4 instruction sequences into
/// single instructions and remap jump targets afterwards.
fn peephole(code: Vec<Op>) -> Vec<Op> {
    let mut out: Vec<Op> = Vec::with_capacity(code.len());
    let mut map = vec![0usize; code.len() + 1];
    let mut i = 0;
    while i < code.len() {
        map[i] = out.len();
        if i + 3 < code.len() {
            let fused = match (&code[i], &code[i + 1], &code[i + 2], &code[i + 3]) {
                (Op::Load(a), Op::Load(b), Op::Bin(op), Op::Store(c)) => Some(Op::BinLocals {
                    dst: *c,
                    a: *a,
                    b: *b,
                    op: *op,
                }),
                (Op::Load(a), Op::Imm(k), Op::Bin(op), Op::Store(c)) => Some(Op::BinLocalImm {
                    dst: *c,
                    a: *a,
                    imm: *k,
                    op: *op,
                }),
                (Op::Load(a), Op::Load(b), Op::Bin(op), Op::JumpF(t)) if is_compare(*op) => {
                    Some(Op::CmpJump {
                        a: *a,
                        b: *b,
                        op: *op,
                        target: *t,
                        jump_if: false,
                    })
                }
                (Op::Load(a), Op::Load(b), Op::Bin(op), Op::JumpT(t)) if is_compare(*op) => {
                    Some(Op::CmpJump {
                        a: *a,
                        b: *b,
                        op: *op,
                        target: *t,
                        jump_if: true,
                    })
                }
                (Op::Load(a), Op::Imm(k), Op::Bin(op), Op::JumpF(t)) if is_compare(*op) => {
                    Some(Op::CmpImmJump {
                        a: *a,
                        imm: *k,
                        op: *op,
                        target: *t,
                        jump_if: false,
                    })
                }
                (Op::Load(a), Op::Imm(k), Op::Bin(op), Op::JumpT(t)) if is_compare(*op) => {
                    Some(Op::CmpImmJump {
                        a: *a,
                        imm: *k,
                        op: *op,
                        target: *t,
                        jump_if: true,
                    })
                }
                _ => None,
            };
            if let Some(op) = fused {
                let at = out.len();
                out.push(op);
                map[i + 1] = at;
                map[i + 2] = at;
                map[i + 3] = at;
                i += 4;
                continue;
            }
        }
        if i + 1 < code.len() {
            let fused = match (&code[i], &code[i + 1]) {
                (Op::Load(a), Op::JumpF(t)) => Some(Op::JumpLocal {
                    slot: *a,
                    target: *t,
                    jump_if: false,
                }),
                (Op::Load(a), Op::JumpT(t)) => Some(Op::JumpLocal {
                    slot: *a,
                    target: *t,
                    jump_if: true,
                }),
                _ => None,
            };
            if let Some(op) = fused {
                let at = out.len();
                out.push(op);
                map[i + 1] = at;
                i += 2;
                continue;
            }
        }
        out.push(code[i].clone());
        i += 1;
    }
    map[code.len()] = out.len();
    for op in out.iter_mut() {
        match op {
            Op::Jump(t) | Op::JumpF(t) | Op::JumpT(t) | Op::IterGet(t) => {
                *t = map[*t as usize] as u32
            }
            Op::CmpJump { target, .. }
            | Op::CmpImmJump { target, .. }
            | Op::JumpLocal { target, .. } => *target = map[*target as usize] as u32,
            _ => {}
        }
    }
    out
}

fn is_compare(op: BinOp) -> bool {
    matches!(
        op,
        BinOp::Eq | BinOp::Ne | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge
    )
}

fn normalize_iter(e: &Expr) -> &Expr {
    match e {
        Expr::Unary(UnOp::Deref, inner) => normalize_iter(inner),
        Expr::Ref(inner) => normalize_iter(inner),
        Expr::Method(recv, name, _, args)
            if args.is_empty()
                && matches!(
                    name.as_str(),
                    "iter" | "into_iter" | "iter_mut" | "copied"
                ) =>
        {
            normalize_iter(recv)
        }
        _ => e,
    }
}

fn diverges(e: &Expr) -> bool {
    match e {
        Expr::Return(_) | Expr::Break(_) | Expr::Continue => true,
        Expr::Macro { name, .. } => matches!(
            name.as_str(),
            "panic" | "unreachable" | "todo" | "unimplemented"
        ),
        Expr::Block(b) => b
            .stmts
            .last()
            .map(|s| matches!(s, Stmt::Expr { expr, semi: false } if diverges(expr)))
            .unwrap_or(false),
        Expr::If { then_b, else_b, .. } => {
            let then_div = then_b
                .stmts
                .last()
                .map(|s| matches!(s, Stmt::Expr { expr, semi: false } if diverges(expr)))
                .unwrap_or(false);
            let else_div = else_b
                .as_ref()
                .map(|e| diverges(e))
                .unwrap_or(false);
            then_div && else_div
        }
        _ => false,
    }
}

pub fn parse_template(s: &str) -> Result<Vec<TPiece>, String> {
    let mut pieces = Vec::new();
    let mut lit = String::new();
    let mut chars = s.chars().peekable();
    let mut auto = 0usize;
    while let Some(c) = chars.next() {
        match c {
            '{' => {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    lit.push('{');
                    continue;
                }
                if !lit.is_empty() {
                    pieces.push(TPiece::Lit(std::mem::take(&mut lit)));
                }
                let mut inner = String::new();
                let mut closed = false;
                for c2 in chars.by_ref() {
                    if c2 == '}' {
                        closed = true;
                        break;
                    }
                    inner.push(c2);
                }
                if !closed {
                    return Err("unclosed `{` in format string".into());
                }
                let (arg, spec) = match inner.split_once(':') {
                    Some((a, s)) => (a, s),
                    None => (inner.as_str(), ""),
                };
                let idx = if arg.is_empty() {
                    let i = auto;
                    auto += 1;
                    i
                } else if let Ok(i) = arg.parse::<usize>() {
                    i.max(auto)
                } else {
                    return Err(format!(
                        "named format argument `{arg}` is not supported in mini-rust"
                    ));
                };
                // Validate the spec early so errors point at the source.
                parse_spec(spec);
                pieces.push(TPiece::Arg { idx, spec: spec.to_string() });
            }
            '}' => {
                if chars.peek() == Some(&'}') {
                    chars.next();
                }
                lit.push('}');
            }
            other => lit.push(other),
        }
    }
    if !lit.is_empty() {
        pieces.push(TPiece::Lit(lit));
    }
    Ok(pieces)
}
