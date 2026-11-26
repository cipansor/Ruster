//! Arithmetic, comparison and cast semantics shared by the VM and the
//! compile-time constant evaluator.

use crate::ast::{BinOp, IntKind, Type, UnOp};
use crate::value::{compare, Value};
use std::cmp::Ordering;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CastTo {
    Int(IntKind),
    F32,
    F64,
    Char,
}

pub fn cast_to_from_type(ty: &Type) -> Option<CastTo> {
    Some(match ty {
        Type::Int(k) => CastTo::Int(*k),
        Type::F32 => CastTo::F32,
        Type::F64 => CastTo::F64,
        Type::Char => CastTo::Char,
        Type::Bool => return None,
        _ => return None,
    })
}

fn narrow(v: i64, k: IntKind) -> i64 {
    let bits = k.bits();
    if bits >= 64 {
        return v;
    }
    if k.is_signed() {
        let shift = 64 - bits;
        (v << shift) >> shift
    } else {
        let mask = (1i64 << bits) - 1;
        v & mask
    }
}

fn float_to_int(f: f64, k: IntKind) -> i64 {
    if f.is_nan() {
        return 0;
    }
    let bits = k.bits();
    let (min, max) = if k.is_signed() {
        if bits == 64 {
            (i64::MIN as f64, i64::MAX as f64)
        } else {
            (-(1i64 << (bits - 1)) as f64, ((1i64 << (bits - 1)) - 1) as f64)
        }
    } else if bits == 64 {
        (0.0, u64::MAX as f64)
    } else {
        (0.0, ((1i64 << bits) - 1) as f64)
    };
    if f <= min {
        return narrow(min as i64, k);
    }
    if f >= max {
        return narrow(max as i64, k);
    }
    narrow(f as i64, k)
}

pub fn cast(v: &Value, to: CastTo) -> Result<Value, String> {
    Ok(match to {
        CastTo::Int(k) => Value::Int(match v {
            Value::Int(x) => narrow(*x, k),
            Value::Float(f) => float_to_int(*f, k),
            Value::Bool(b) => *b as i64,
            Value::Char(c) => *c as i64,
            other => {
                return Err(format!("cannot cast {} to an integer", other.type_name()));
            }
        }),
        CastTo::F32 => Value::Float(match v {
            Value::Int(x) => *x as f32 as f64,
            Value::Float(f) => *f as f32 as f64,
            Value::Bool(b) => *b as i64 as f32 as f64,
            Value::Char(c) => *c as u32 as f32 as f64,
            other => return Err(format!("cannot cast {} to f32", other.type_name())),
        }),
        CastTo::F64 => Value::Float(match v {
            Value::Int(x) => *x as f64,
            Value::Float(f) => *f,
            Value::Bool(b) => *b as i64 as f64,
            Value::Char(c) => *c as u32 as f64,
            other => return Err(format!("cannot cast {} to f64", other.type_name())),
        }),
        CastTo::Char => Value::Char(match v {
            Value::Int(x) => char::from_u32(*x as u32).unwrap_or('\u{fffd}'),
            Value::Char(c) => *c,
            other => return Err(format!("cannot cast {} to char", other.type_name())),
        }),
    })
}

pub fn unop(op: UnOp, a: &Value) -> Result<Value, String> {
    Ok(match op {
        UnOp::Neg => match a {
            Value::Int(x) => Value::Int(x.wrapping_neg()),
            Value::Float(x) => Value::Float(-x),
            other => return Err(format!("cannot negate {}", other.type_name())),
        },
        UnOp::Not => match a {
            Value::Bool(b) => Value::Bool(!b),
            Value::Int(x) => Value::Int(!x),
            other => return Err(format!("cannot apply `!` to {}", other.type_name())),
        },
        UnOp::Deref => a.clone(),
    })
}

/// Fast paths for the common numeric cases; returns `None` when the slow,
/// fully general `binop` is needed.
#[inline]
pub fn binop_fast(op: BinOp, a: &Value, b: &Value) -> Option<Value> {
    use BinOp::*;
    Some(match (op, a, b) {
        (Add, Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_add(*y)),
        (Sub, Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_sub(*y)),
        (Mul, Value::Int(x), Value::Int(y)) => Value::Int(x.wrapping_mul(*y)),
        (Div, Value::Int(x), Value::Int(y)) if *y != 0 => Value::Int(x.wrapping_div(*y)),
        (Rem, Value::Int(x), Value::Int(y)) if *y != 0 => Value::Int(x.wrapping_rem(*y)),
        (BitAnd, Value::Int(x), Value::Int(y)) => Value::Int(x & y),
        (BitOr, Value::Int(x), Value::Int(y)) => Value::Int(x | y),
        (BitXor, Value::Int(x), Value::Int(y)) => Value::Int(x ^ y),
        (Shl, Value::Int(x), Value::Int(y)) => {
            Value::Int(x.wrapping_shl((*y as u64 & 63) as u32))
        }
        (Shr, Value::Int(x), Value::Int(y)) => {
            Value::Int(x.wrapping_shr((*y as u64 & 63) as u32))
        }
        (Add, Value::Float(x), Value::Float(y)) => Value::Float(x + y),
        (Sub, Value::Float(x), Value::Float(y)) => Value::Float(x - y),
        (Mul, Value::Float(x), Value::Float(y)) => Value::Float(x * y),
        (Div, Value::Float(x), Value::Float(y)) => Value::Float(x / y),
        (Lt, Value::Int(x), Value::Int(y)) => Value::Bool(x < y),
        (Le, Value::Int(x), Value::Int(y)) => Value::Bool(x <= y),
        (Gt, Value::Int(x), Value::Int(y)) => Value::Bool(x > y),
        (Ge, Value::Int(x), Value::Int(y)) => Value::Bool(x >= y),
        (Lt, Value::Float(x), Value::Float(y)) => Value::Bool(x < y),
        (Le, Value::Float(x), Value::Float(y)) => Value::Bool(x <= y),
        (Gt, Value::Float(x), Value::Float(y)) => Value::Bool(x > y),
        (Ge, Value::Float(x), Value::Float(y)) => Value::Bool(x >= y),
        (Eq, Value::Int(x), Value::Int(y)) => Value::Bool(x == y),
        (Ne, Value::Int(x), Value::Int(y)) => Value::Bool(x != y),
        (Eq, Value::Bool(x), Value::Bool(y)) => Value::Bool(x == y),
        (Ne, Value::Bool(x), Value::Bool(y)) => Value::Bool(x != y),
        _ => return None,
    })
}

pub fn binop(op: BinOp, a: &Value, b: &Value) -> Result<Value, String> {
    use BinOp::*;
    let bad = || Err(format!("cannot apply `{:?}` to {} and {}", op, a.type_name(), b.type_name()));
    let num = |x: &Value| -> Option<f64> {
        match x {
            Value::Int(i) => Some(*i as f64),
            Value::Float(f) => Some(*f),
            _ => None,
        }
    };
    match op {
        Add | Sub | Mul | Div | Rem => {
            if let (Value::Int(x), Value::Int(y)) = (a, b) {
                let r = match op {
                    Add => x.wrapping_add(*y),
                    Sub => x.wrapping_sub(*y),
                    Mul => x.wrapping_mul(*y),
                    Div => {
                        if *y == 0 {
                            return Err("attempt to divide by zero".into());
                        }
                        x.wrapping_div(*y)
                    }
                    _ => {
                        if *y == 0 {
                            return Err("attempt to calculate the remainder with a divisor of zero".into());
                        }
                        x.wrapping_rem(*y)
                    }
                };
                return Ok(Value::Int(r));
            }
            if op == Add {
                if let (Value::Str(x), Value::Str(y)) = (a, b) {
                    let x = x.borrow();
                    let y = y.borrow();
                    let mut s = String::with_capacity(x.len() + y.len());
                    s.push_str(&x);
                    s.push_str(&y);
                    return Ok(Value::new_str(s));
                }
            }
            if let (Some(x), Some(y)) = (num(a), num(b)) {
                return Ok(Value::Float(match op {
                    Add => x + y,
                    Sub => x - y,
                    Mul => x * y,
                    Div => x / y,
                    _ => x % y,
                }));
            }
            bad()
        }
        BitAnd | BitOr | BitXor => match (a, b) {
            (Value::Int(x), Value::Int(y)) => Ok(Value::Int(match op {
                BitAnd => x & y,
                BitOr => x | y,
                _ => x ^ y,
            })),
            (Value::Bool(x), Value::Bool(y)) => Ok(Value::Bool(match op {
                BitAnd => *x & *y,
                BitOr => *x | *y,
                _ => *x ^ *y,
            })),
            _ => bad(),
        },
        Shl | Shr => match (a, b) {
            (Value::Int(x), Value::Int(y)) => {
                let sh = (*y as u64 & 63) as u32;
                Ok(Value::Int(if op == Shl {
                    x.wrapping_shl(sh)
                } else {
                    x.wrapping_shr(sh)
                }))
            }
            _ => bad(),
        },
        Eq => Ok(Value::Bool(a.equals(b))),
        Ne => Ok(Value::Bool(!a.equals(b))),
        Lt | Le | Gt | Ge => {
            let ord = compare(a, b)?;
            Ok(Value::Bool(match op {
                Lt => ord == Ordering::Less,
                Le => ord != Ordering::Greater,
                Gt => ord == Ordering::Greater,
                _ => ord != Ordering::Less,
            }))
        }
        And | Or => match (a, b) {
            (Value::Bool(x), Value::Bool(y)) => {
                Ok(Value::Bool(if op == And { *x && *y } else { *x || *y }))
            }
            _ => bad(),
        },
    }
}

pub fn index_get(container: &Value, idx: &Value) -> Result<Value, String> {
    index_get_i(container, idx.as_index()?)
}

/// Index with an already-coerced integer, avoiding a temporary `Value`.
pub fn index_get_i(container: &Value, i: i64) -> Result<Value, String> {
    if i < 0 {
        return Err(format!("index out of bounds: the len is smaller than the index {i}"));
    }
    let i = i as usize;
    match container {
        Value::Vec(v) => {
            let v = v.borrow();
            v.get_value(i)
                .ok_or_else(|| format!("index out of bounds: the len is {} but the index is {}", v.len(), i))
        }
        Value::Tuple(t) => t
            .get(i)
            .cloned()
            .ok_or_else(|| format!("tuple index out of bounds: {i}")),
        other => Err(format!("cannot index into {}", other.type_name())),
    }
}
