//! Runtime values and Rust-like formatting.

use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
pub enum Value {
    Unit,
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
    Str(Rc<RefCell<String>>),
    Vec(Rc<RefCell<Vec<Value>>>),
    Tuple(Rc<Vec<Value>>),
}

impl Value {
    pub fn new_str(s: impl Into<String>) -> Value {
        Value::Str(Rc::new(RefCell::new(s.into())))
    }

    pub fn borrow_str(&self) -> Result<std::cell::Ref<'_, String>, String> {
        match self {
            Value::Str(s) => Ok(s.borrow()),
            other => Err(format!("expected a string, found {}", other.type_name())),
        }
    }

    pub fn new_vec(v: Vec<Value>) -> Value {
        Value::Vec(Rc::new(RefCell::new(v)))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Unit => "()",
            Value::Int(_) => "integer",
            Value::Float(_) => "float",
            Value::Bool(_) => "bool",
            Value::Char(_) => "char",
            Value::Str(_) => "string",
            Value::Vec(_) => "Vec",
            Value::Tuple(_) => "tuple",
        }
    }

    pub fn as_bool(&self) -> Result<bool, String> {
        match self {
            Value::Bool(b) => Ok(*b),
            other => Err(format!("expected a bool, found {}", other.type_name())),
        }
    }

    pub fn as_int(&self) -> Result<i64, String> {
        match self {
            Value::Int(v) => Ok(*v),
            other => Err(format!("expected an integer, found {}", other.type_name())),
        }
    }

    pub fn as_f64(&self) -> Result<f64, String> {
        match self {
            Value::Float(v) => Ok(*v),
            Value::Int(v) => Ok(*v as f64),
            other => Err(format!("expected a number, found {}", other.type_name())),
        }
    }

    /// A clone that mimics Rust's `Clone`: vectors and tuples are copied
    /// structurally instead of sharing their backing store.
    pub fn deep_clone(&self) -> Value {
        match self {
            Value::Vec(v) => Value::new_vec(v.borrow().iter().map(|x| x.deep_clone()).collect()),
            Value::Tuple(t) => Value::Tuple(Rc::new(t.iter().map(|x| x.deep_clone()).collect())),
            Value::Str(s) => Value::Str(Rc::new(RefCell::new(s.borrow().clone()))),
            other => other.clone(),
        }
    }

    pub fn display(&self) -> String {
        let spec = Spec::default();
        format_value(self, &spec)
    }

    pub fn debug(&self) -> String {
        let spec = Spec { debug: true, ..Spec::default() };
        format_value(self, &spec)
    }

    pub fn equals(&self, other: &Value) -> bool {
        match (self, other) {
            (Value::Unit, Value::Unit) => true,
            (Value::Int(a), Value::Int(b)) => a == b,
            (Value::Int(a), Value::Float(b)) => (*a as f64) == *b,
            (Value::Float(a), Value::Int(b)) => *a == (*b as f64),
            (Value::Float(a), Value::Float(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Char(a), Value::Char(b)) => a == b,
            (Value::Str(a), Value::Str(b)) => *a.borrow() == *b.borrow(),
            (Value::Vec(a), Value::Vec(b)) => {
                let a = a.borrow();
                let b = b.borrow();
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            (Value::Tuple(a), Value::Tuple(b)) => {
                a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
            }
            _ => false,
        }
    }
}

/// `Some(a).cmp(&b)`-style ordering used by `sort`, `min`, `max`.
pub fn compare(a: &Value, b: &Value) -> Result<std::cmp::Ordering, String> {
    use std::cmp::Ordering;
    Ok(match (a, b) {
        (Value::Int(x), Value::Int(y)) => x.cmp(y),
        (Value::Float(x), Value::Float(y)) => {
            x.partial_cmp(y).unwrap_or(if x.is_nan() && y.is_nan() {
                Ordering::Equal
            } else if x.is_nan() {
                Ordering::Greater
            } else {
                Ordering::Less
            })
        }
        (Value::Int(x), Value::Float(y)) => (*x as f64)
            .partial_cmp(y)
            .unwrap_or(Ordering::Greater),
        (Value::Float(x), Value::Int(y)) => x
            .partial_cmp(&(*y as f64))
            .unwrap_or(Ordering::Less),
        (Value::Str(x), Value::Str(y)) => x.borrow().cmp(&y.borrow()),
        (Value::Char(x), Value::Char(y)) => x.cmp(y),
        (Value::Bool(x), Value::Bool(y)) => x.cmp(y),
        _ => return Err(format!("cannot compare {} with {}", a.type_name(), b.type_name())),
    })
}

#[derive(Clone, Debug)]
pub struct Spec {
    pub fill: char,
    pub align: Option<char>,
    pub sign: Option<char>,
    pub alternate: bool,
    pub zero: bool,
    pub width: Option<usize>,
    pub precision: Option<usize>,
    pub ty: Option<char>,
    pub debug: bool,
}

impl Default for Spec {
    fn default() -> Self {
        Spec {
            fill: ' ',
            align: None,
            sign: None,
            alternate: false,
            zero: false,
            width: None,
            precision: None,
            ty: None,
            debug: false,
        }
    }
}

pub fn parse_spec(s: &str) -> Spec {
    let mut spec = Spec::default();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    // fill + align
    if chars.len() >= 2 && matches!(chars[1], '<' | '>' | '^') {
        spec.fill = chars[0];
        spec.align = Some(chars[1]);
        i = 2;
    } else if !chars.is_empty() && matches!(chars[0], '<' | '>' | '^') {
        spec.align = Some(chars[0]);
        i = 1;
    }
    if i < chars.len() && matches!(chars[i], '+' | '-' | ' ') {
        spec.sign = Some(chars[i]);
        i += 1;
    }
    if i < chars.len() && chars[i] == '#' {
        spec.alternate = true;
        i += 1;
    }
    if i < chars.len() && chars[i] == '0' {
        spec.zero = true;
        i += 1;
    }
    let mut width = String::new();
    while i < chars.len() && chars[i].is_ascii_digit() {
        width.push(chars[i]);
        i += 1;
    }
    if !width.is_empty() {
        spec.width = width.parse().ok();
    }
    if i < chars.len() && chars[i] == '.' {
        i += 1;
        let mut prec = String::new();
        while i < chars.len() && chars[i].is_ascii_digit() {
            prec.push(chars[i]);
            i += 1;
        }
        spec.precision = prec.parse().ok().or(Some(0));
    }
    if i < chars.len() {
        spec.ty = Some(chars[i]);
        if chars[i] == '?' {
            spec.debug = true;
        }
    }
    spec
}

fn pad(text: String, spec: &Spec, numeric: bool, _negative: bool) -> String {
    let width = match spec.width {
        Some(w) => w,
        None => return text,
    };
    if text.chars().count() >= width {
        return text;
    }
    let align = spec.align.unwrap_or(if numeric { '>' } else { '<' });
    if spec.zero && numeric && spec.align.is_none() {
        let (sign, rest) = if let Some(r) = text.strip_prefix('-') {
            ("-", r.to_string())
        } else if let Some(r) = text.strip_prefix('+') {
            ("+", r.to_string())
        } else {
            ("", text.clone())
        };
        let padn = width.saturating_sub(rest.chars().count() + sign.len());
        let mut out = String::with_capacity(width);
        out.push_str(sign);
        out.extend(std::iter::repeat('0').take(padn));
        out.push_str(&rest);
        return out;
    }
    let fill = spec.fill;
    let padn = width - text.chars().count();
    let mut out = String::with_capacity(width);
    match align {
        '<' => {
            out.push_str(&text);
            out.extend(std::iter::repeat(fill).take(padn));
        }
        '^' => {
            let left = padn / 2;
            out.extend(std::iter::repeat(fill).take(left));
            out.push_str(&text);
            out.extend(std::iter::repeat(fill).take(padn - left));
        }
        _ => {
            out.extend(std::iter::repeat(fill).take(padn));
            out.push_str(&text);
        }
    }
    out
}

pub fn format_value(v: &Value, spec: &Spec) -> String {
    let mut numeric = false;
    let mut negative = false;
    let body = match (v, spec.debug) {
        (Value::Unit, _) => "()".to_string(),
        (Value::Int(x), _) => {
            numeric = true;
            negative = *x < 0;
            match spec.ty {
                Some('x') => format!("{:x}", *x as u64),
                Some('X') => format!("{:X}", *x as u64),
                Some('b') => format!("{:b}", *x as u64),
                Some('o') => format!("{:o}", *x as u64),
                Some('e') => format!("{:e}", *x as f64),
                Some('E') => format!("{:E}", *x as f64),
                _ => {
                    let s = x.to_string();
                    if let Some('+') = spec.sign {
                        if *x >= 0 {
                            format!("+{s}")
                        } else {
                            s
                        }
                    } else {
                        s
                    }
                }
            }
        }
        (Value::Float(x), true) => {
            numeric = true;
            negative = x.is_sign_negative();
            match spec.ty {
                Some('x') | Some('X') | Some('b') | Some('o') => {
                    return format_value(&Value::Int(*x as i64), spec)
                }
                Some('e') | Some('E') => format!("{:?}", x),
                _ => {
                    let mut s = format!("{:?}", x);
                    if let Some('+') = spec.sign {
                        if *x >= 0.0 {
                            s = format!("+{s}");
                        }
                    }
                    s
                }
            }
        }
        (Value::Float(x), false) => {
            numeric = true;
            negative = x.is_sign_negative();
            let s = match (spec.ty, spec.precision) {
                (Some('e'), Some(p)) => format!("{:.*e}", p, x),
                (Some('E'), Some(p)) => format!("{:.*E}", p, x),
                (Some('e'), None) => format!("{:e}", x),
                (Some('E'), None) => format!("{:E}", x),
                (_, Some(p)) => format!("{:.*}", p, x),
                (_, None) => format!("{}", x),
            };
            if let Some('+') = spec.sign {
                if *x >= 0.0 {
                    format!("+{s}")
                } else {
                    s
                }
            } else {
                s
            }
        }
        (Value::Bool(b), _) => b.to_string(),
        (Value::Char(c), true) => format!("{c:?}"),
        (Value::Char(c), false) => c.to_string(),
        (Value::Str(s), true) => format!("{:?}", s.borrow()),
        (Value::Str(s), false) => {
            let mut text = s.borrow().clone();
            if let Some(p) = spec.precision {
                if text.chars().count() > p {
                    text = text.chars().take(p).collect();
                }
            }
            text
        }
        (Value::Vec(items), _) => {
            let items = items.borrow();
            let mut out = String::from("[");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&item.debug());
            }
            out.push(']');
            out
        }
        (Value::Tuple(items), _) => {
            let mut out = String::from("(");
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&item.debug());
            }
            if items.len() == 1 {
                out.push(',');
            }
            out.push(')');
            out
        }
    };
    pad(body, spec, numeric, negative)
}
