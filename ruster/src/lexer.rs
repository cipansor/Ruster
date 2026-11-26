use std::fmt;

#[derive(Clone, Debug, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    Char(char),
    Punct(&'static str),
    /// `'a` in `&'a str`; the parser ignores it (mini-rust has no borrow checking).
    Lifetime,
    Eof,
}

impl fmt::Display for Tok {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Tok::Ident(s) => write!(f, "`{s}`"),
            Tok::Int(v) => write!(f, "`{v}`"),
            Tok::Float(v) => write!(f, "`{v}`"),
            Tok::Str(s) => write!(f, "string literal {s:?}"),
            Tok::Char(c) => write!(f, "char literal {c:?}"),
            Tok::Punct(p) => write!(f, "`{p}`"),
            Tok::Lifetime => write!(f, "lifetime"),
            Tok::Eof => write!(f, "end of file"),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Token {
    pub tok: Tok,
    pub line: u32,
    pub col: u32,
}

const PUNCTS: &[&str] = &[
    "<<=", ">>=", "...", "..=", "::", "->", "=>", "==", "!=", "<=", ">=", "&&", "||", "<<",
    ">>", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "..", "+", "-", "*", "/", "%", "!",
    "=", "<", ">", "&", "|", "^", "~", "?", ":", ";", ",", ".", "(", ")", "{", "}", "[", "]",
    "#", "@", "$",
];

pub struct Lexer<'a> {
    s: &'a [u8],
    i: usize,
    line: u32,
    col: u32,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Lexer { s: src.as_bytes(), i: 0, line: 1, col: 1 }
    }

    pub fn tokenize(mut self) -> Result<Vec<Token>, String> {
        let mut out = Vec::new();
        loop {
            self.skip_trivia()?;
            let (line, col) = (self.line, self.col);
            let tok = self.next_token()?;
            let eof = matches!(tok, Tok::Eof);
            out.push(Token { tok, line, col });
            if eof {
                return Ok(out);
            }
        }
    }

    fn peek(&self) -> u8 {
        *self.s.get(self.i).unwrap_or(&0)
    }

    fn peek_at(&self, n: usize) -> u8 {
        *self.s.get(self.i + n).unwrap_or(&0)
    }

    fn bump(&mut self) -> u8 {
        let c = self.peek();
        self.i += 1;
        if c == b'\n' {
            self.line += 1;
            self.col = 1;
        } else {
            self.col += 1;
        }
        c
    }

    fn skip_trivia(&mut self) -> Result<(), String> {
        loop {
            match self.peek() {
                b' ' | b'\t' | b'\r' | b'\n' => {
                    self.bump();
                }
                b'/' if self.peek_at(1) == b'/' => {
                    while self.peek() != b'\n' && self.peek() != 0 {
                        self.bump();
                    }
                }
                b'/' if self.peek_at(1) == b'*' => {
                    self.bump();
                    self.bump();
                    let mut depth = 1u32;
                    while depth > 0 {
                        if self.peek() == 0 {
                            return Err("unterminated block comment".into());
                        }
                        if self.peek() == b'/' && self.peek_at(1) == b'*' {
                            self.bump();
                            self.bump();
                            depth += 1;
                        } else if self.peek() == b'*' && self.peek_at(1) == b'/' {
                            self.bump();
                            self.bump();
                            depth -= 1;
                        } else {
                            self.bump();
                        }
                    }
                }
                _ => return Ok(()),
            }
        }
    }

    fn next_token(&mut self) -> Result<Tok, String> {
        let c = self.peek();
        if c == 0 {
            return Ok(Tok::Eof);
        }
        if is_ident_start(c) {
            let start = self.i;
            while is_ident_cont(self.peek()) {
                self.bump();
            }
            let word = std::str::from_utf8(&self.s[start..self.i]).unwrap().to_string();
            if word == "r" && (self.peek() == b'"' || self.peek() == b'#') {
                // raw string / raw ident: keep the simple forms only
                if self.peek() == b'"' {
                    return self.lex_raw_string(0);
                }
                return Err(self.err("raw identifiers are not supported"));
            }
            return Ok(Tok::Ident(word));
        }
        if c.is_ascii_digit() {
            return self.lex_number();
        }
        if c == b'"' {
            return self.lex_string();
        }
        if c == b'\'' {
            return self.lex_char_or_lifetime();
        }
        for p in PUNCTS {
            if self.s[self.i..].starts_with(p.as_bytes()) {
                for _ in 0..p.len() {
                    self.bump();
                }
                return Ok(Tok::Punct(p));
            }
        }
        Err(self.err(&format!("unexpected character {:?}", c as char)))
    }

    fn err(&self, msg: &str) -> String {
        format!("{msg} at {}:{}", self.line, self.col)
    }

    fn take_digits(&mut self, radix: u32) -> String {
        let mut out = String::new();
        loop {
            let c = self.peek();
            let ok = if radix == 16 {
                c.is_ascii_hexdigit()
            } else {
                c.is_ascii_digit() && (c - b'0') < radix as u8
            };
            if !ok {
                break;
            }
            out.push(c as char);
            self.bump();
            if self.peek() == b'_' {
                self.bump();
            }
        }
        out
    }

    fn take_suffix(&mut self) -> String {
        let start = self.i;
        while self.peek().is_ascii_alphanumeric() {
            self.bump();
        }
        std::str::from_utf8(&self.s[start..self.i]).unwrap().to_string()
    }

    fn lex_number(&mut self) -> Result<Tok, String> {
        if self.peek() == b'0' {
            let r = self.peek_at(1);
            let radix = match r {
                b'x' | b'X' => Some(16),
                b'b' | b'B' => Some(2),
                b'o' | b'O' => Some(8),
                _ => None,
            };
            if let Some(radix) = radix {
                self.bump();
                self.bump();
                let digits = self.take_digits(radix);
                if digits.is_empty() {
                    return Err(self.err("malformed numeric literal"));
                }
                let v = u128::from_str_radix(&digits, radix)
                    .map_err(|_| self.err("integer literal out of range"))?;
                let suf = self.take_suffix();
                return Ok(Tok::Int(apply_int_suffix(v as i64, &suf)));
            }
        }

        let mut text = String::new();
        let mut is_float = false;
        text.push_str(&self.take_digits(10));
        if self.peek() == b'.' && self.peek_at(1).is_ascii_digit() {
            is_float = true;
            text.push('.');
            self.bump();
            text.push_str(&self.take_digits(10));
        }
        if self.peek() == b'e' || self.peek() == b'E' {
            let mut j = 1;
            if self.peek_at(j) == b'+' || self.peek_at(j) == b'-' {
                j += 1;
            }
            if self.peek_at(j).is_ascii_digit() {
                is_float = true;
                text.push('e');
                self.bump();
                if self.peek() == b'+' || self.peek() == b'-' {
                    text.push(self.bump() as char);
                }
                text.push_str(&self.take_digits(10));
            }
        }
        let suffix = self.take_suffix();
        if is_float || suffix.starts_with('f') {
            let v: f64 = text.parse().map_err(|_| self.err("malformed float literal"))?;
            let v = match suffix.as_str() {
                "f32" => v as f32 as f64,
                _ => v,
            };
            return Ok(Tok::Float(v));
        }
        let digits: String = text.chars().filter(|c| *c != '_').collect();
        let v: u128 = digits.parse().map_err(|_| self.err("integer literal out of range"))?;
        Ok(Tok::Int(apply_int_suffix(v as i64, &suffix)))
    }

    fn lex_string(&mut self) -> Result<Tok, String> {
        self.bump(); // opening quote
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.bump();
            match c {
                0 => return Err(self.err("unterminated string literal")),
                b'"' => {
                    let s = String::from_utf8(out).map_err(|_| self.err("invalid UTF-8"))?;
                    return Ok(Tok::Str(s));
                }
                b'\\' => {
                    let ch = self.lex_escape()?;
                    let mut buf = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                }
                _ => out.push(c),
            }
        }
    }

    fn lex_raw_string(&mut self, hashes: usize) -> Result<Tok, String> {
        while self.peek() == b'#' {
            self.bump();
        }
        if self.peek() == b'"' {
            self.bump();
        }
        let mut out: Vec<u8> = Vec::new();
        loop {
            let c = self.bump();
            if c == 0 {
                return Err(self.err("unterminated raw string literal"));
            }
            if c == b'"' {
                let mut n = 0;
                while n < hashes && self.peek() == b'#' {
                    self.bump();
                    n += 1;
                }
                if n == hashes {
                    let s = String::from_utf8(out).map_err(|_| self.err("invalid UTF-8"))?;
                    return Ok(Tok::Str(s));
                }
                out.push(b'"');
                continue;
            }
            out.push(c);
        }
    }

    fn lex_escape(&mut self) -> Result<char, String> {
        let c = self.bump();
        Ok(match c {
            b'n' => '\n',
            b't' => '\t',
            b'r' => '\r',
            b'\\' => '\\',
            b'\'' => '\'',
            b'"' => '"',
            b'0' => '\0',
            b'x' => {
                let hi = self.bump();
                let lo = self.bump();
                let s = [hi, lo];
                let text = std::str::from_utf8(&s).map_err(|_| self.err("bad \\x escape"))?;
                let v = u8::from_str_radix(text, 16).map_err(|_| self.err("bad \\x escape"))?;
                v as char
            }
            b'u' => {
                if self.peek() != b'{' {
                    return Err(self.err("expected `{` after \\u"));
                }
                self.bump();
                let mut hex = String::new();
                while self.peek() != b'}' {
                    let c = self.bump();
                    if c == 0 {
                        return Err(self.err("unterminated \\u escape"));
                    }
                    hex.push(c as char);
                }
                self.bump();
                let v = u32::from_str_radix(&hex, 16).map_err(|_| self.err("bad \\u escape"))?;
                char::from_u32(v).ok_or_else(|| self.err("bad \\u escape"))?
            }
            _ => return Err(self.err("unknown escape sequence")),
        })
    }

    fn lex_char_or_lifetime(&mut self) -> Result<Tok, String> {
        // Distinguish 'a' (char) from 'a (lifetime).
        let save = (self.i, self.line, self.col);
        self.bump(); // '
        if self.peek() == b'\\' {
            self.bump();
            let ch = self.lex_escape()?;
            if self.peek() != b'\'' {
                return Err(self.err("unterminated char literal"));
            }
            self.bump();
            return Ok(Tok::Char(ch));
        }
        let first = self.bump();
        if first == 0 {
            return Err(self.err("unterminated char literal"));
        }
        if self.peek() == b'\'' {
            self.bump();
            return Ok(Tok::Char(first as char));
        }
        // Not a char literal: rewind and treat as a lifetime.
        self.i = save.0;
        self.line = save.1;
        self.col = save.2;
        self.bump(); // '
        if !is_ident_start(self.peek()) {
            return Err(self.err("expected a character or lifetime after `'`"));
        }
        while is_ident_cont(self.peek()) {
            self.bump();
        }
        Ok(Tok::Lifetime)
    }
}

fn is_ident_start(c: u8) -> bool {
    c.is_ascii_alphabetic() || c == b'_'
}

fn is_ident_cont(c: u8) -> bool {
    c.is_ascii_alphanumeric() || c == b'_'
}

fn apply_int_suffix(v: i64, suffix: &str) -> i64 {
    match suffix {
        "i8" => (v as i8) as i64,
        "u8" => (v as u8) as i64,
        "i16" => (v as i16) as i64,
        "u16" => (v as u16) as i64,
        "i32" => (v as i32) as i64,
        "u32" => (v as u32) as i64,
        "u64" | "usize" => (v as u64) as i64,
        _ => v,
    }
}
