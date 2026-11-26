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
    Vec(Rc<RefCell<VecData>>),
    Tuple(Rc<Vec<Value>>),
}

/// Backing store for `Vec`: either generic `Value`s or a packed scalar buffer.
/// Packed buffers keep `Vec<i64>`/`Vec<bool>` loops free of 16-byte `Value`s.
pub struct VecData {
    values: Vec<Value>,
    ints: Option<Vec<i64>>,
    bools: Option<Vec<bool>>,
}

impl VecData {
    pub fn new(values: Vec<Value>) -> VecData {
        VecData { values, ints: None, bools: None }
    }

    pub fn from_int(n: usize, v: i64) -> VecData {
        VecData { values: Vec::new(), ints: Some(vec![v; n]), bools: None }
    }

    pub fn from_bool(n: usize, b: bool) -> VecData {
        VecData { values: Vec::new(), ints: None, bools: Some(vec![b; n]) }
    }

    fn materialize(&mut self) {
        if let Some(ints) = self.ints.take() {
            self.values = ints.into_iter().map(Value::Int).collect();
        }
        if let Some(bools) = self.bools.take() {
            self.values = bools.into_iter().map(Value::Bool).collect();
        }
    }

    pub fn len(&self) -> usize {
        match (&self.ints, &self.bools) {
            (Some(v), _) => v.len(),
            (_, Some(v)) => v.len(),
            _ => self.values.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn get_value(&self, i: usize) -> Option<Value> {
        if let Some(v) = &self.ints {
            return v.get(i).map(|x| Value::Int(*x));
        }
        if let Some(v) = &self.bools {
            return v.get(i).map(|x| Value::Bool(*x));
        }
        self.values.get(i).cloned()
    }

    pub fn set_value(&mut self, i: usize, val: Value) -> Result<(), String> {
        if let Some(v) = &mut self.ints {
            return match val {
                Value::Int(x) => {
                    v[i] = x;
                    Ok(())
                }
                other => Err(format!("cannot store {} in a Vec<i64>", other.type_name())),
            };
        }
        if let Some(v) = &mut self.bools {
            return match val {
                Value::Bool(b) => {
                    v[i] = b;
                    Ok(())
                }
                other => Err(format!("cannot store {} in a Vec<bool>", other.type_name())),
            };
        }
        self.values[i] = val;
        Ok(())
    }

    pub fn to_value_vec(&self) -> Vec<Value> {
        if let Some(v) = &self.ints {
            return v.iter().map(|x| Value::Int(*x)).collect();
        }
        if let Some(v) = &self.bools {
            return v.iter().map(|x| Value::Bool(*x)).collect();
        }
        self.values.clone()
    }

    pub fn push(&mut self, val: Value) {
        if let Some(v) = &mut self.ints {
            if let Value::Int(x) = val {
                v.push(x);
                return;
            }
        } else if let Some(v) = &mut self.bools {
            if let Value::Bool(b) = val {
                v.push(b);
                return;
            }
        } else {
            self.values.push(val);
            return;
        }
        self.materialize();
        self.values.push(val);
    }

    pub fn pop(&mut self) -> Option<Value> {
        if let Some(v) = &mut self.ints {
            return v.pop().map(Value::Int);
        }
        if let Some(v) = &mut self.bools {
            return v.pop().map(Value::Bool);
        }
        self.values.pop()
    }

    pub fn clear(&mut self) {
        if let Some(v) = &mut self.ints {
            v.clear();
        } else if let Some(v) = &mut self.bools {
            v.clear();
        } else {
            self.values.clear();
        }
    }

    pub fn insert(&mut self, i: usize, val: Value) {
        self.materialize();
        self.values.insert(i, val);
    }

    pub fn remove(&mut self, i: usize) -> Value {
        self.materialize();
        self.values.remove(i)
    }

    pub fn reverse(&mut self) {
        if let Some(v) = &mut self.ints {
            v.reverse();
        } else if let Some(v) = &mut self.bools {
            v.reverse();
        } else {
            self.values.reverse();
        }
    }

    pub fn sort_by<F: FnMut(&Value, &Value) -> std::cmp::Ordering>(&mut self, f: F) {
        self.materialize();
        self.values.sort_by(f);
    }

    pub fn contains_value(&self, n: &Value) -> bool {
        if let Some(v) = &self.ints {
            return matches!(n, Value::Int(x) if v.contains(x));
        }
        if let Some(v) = &self.bools {
            return matches!(n, Value::Bool(x) if v.contains(x));
        }
        self.values.iter().any(|x| x.equals(n))
    }

    pub fn first_value(&self) -> Option<Value> {
        self.get_value(0)
    }

    pub fn last_value(&self) -> Option<Value> {
        let n = self.len();
        if n == 0 { None } else { self.get_value(n - 1) }
    }

    pub fn deep_clone(&self) -> VecData {
        VecData {
            values: self.values.iter().map(|x| x.deep_clone()).collect(),
            ints: self.ints.clone(),
            bools: self.bools.clone(),
        }
    }

    pub fn equals(&self, other: &VecData) -> bool {
        if let (Some(a), Some(b)) = (&self.ints, &other.ints) {
            return a == b;
        }
        if let (Some(a), Some(b)) = (&self.bools, &other.bools) {
            return a == b;
        }
        let a = self.to_value_vec();
        let b = other.to_value_vec();
        a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| x.equals(y))
    }
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
        Value::Vec(Rc::new(RefCell::new(VecData::new(v))))
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

    /// Index coercion, mirroring `x as usize` for the scalar types.
    pub fn as_index(&self) -> Result<i64, String> {
        match self {
            Value::Int(v) => Ok(*v),
            Value::Bool(b) => Ok(*b as i64),
            Value::Float(f) => Ok(*f as i64),
            Value::Char(c) => Ok(*c as i64),
            other => Err(format!(
                "expected an integer index, found {}",
                other.type_name()
            )),
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
            Value::Vec(v) => Value::Vec(Rc::new(RefCell::new(v.borrow().deep_clone()))),
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
            (Value::Vec(a), Value::Vec(b)) => a.borrow().equals(&b.borrow()),
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
            let items = items.borrow().to_value_vec();
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
