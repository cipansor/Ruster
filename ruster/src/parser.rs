//! Recursive-descent parser: tokens -> AST.

use crate::ast::*;
use crate::lexer::{Tok, Token};

pub struct Parser {
    toks: Vec<Token>,
    pos: usize,
}

impl Parser {
    pub fn new(toks: Vec<Token>) -> Self {
        Parser { toks, pos: 0 }
    }

    pub fn parse_program(mut self) -> Result<ProgramAst, String> {
        let mut items = Vec::new();
        self.parse_items(&mut items, false)?;
        Ok(ProgramAst { items })
    }

    fn parse_items(&mut self, items: &mut Vec<Item>, in_mod: bool) -> Result<(), String> {
        loop {
            self.skip_attrs()?;
            if matches!(self.tok(), Tok::Eof) {
                if in_mod {
                    return Err(self.err("unexpected end of file inside `mod`"));
                }
                return Ok(());
            }
            if in_mod && self.is_punct("}") {
                return Ok(());
            }
            if self.eat_punct(";") {
                continue;
            }
            self.eat_kw("pub");
            if self.is_punct("(") {
                self.skip_balanced("(", ")")?;
            }
            // modifiers that may precede an item
            loop {
                if self.eat_kw("unsafe") || self.eat_kw("async") {
                    continue;
                }
                if self.is_kw("const") {
                    // `const fn` or a const item: peek ahead
                    let next_is_fn = matches!(
                        self.toks.get(self.pos + 1).map(|t| &t.tok),
                        Some(Tok::Ident(w)) if w == "fn"
                    );
                    if next_is_fn {
                        self.bump();
                        continue;
                    }
                }
                if self.is_kw("extern") {
                    self.bump();
                    if let Tok::Str(_) = self.tok() {
                        self.bump();
                    }
                    if self.is_punct("{") {
                        self.skip_balanced("{", "}")?;
                        continue;
                    }
                    continue;
                }
                break;
            }

            let word = match self.tok() {
                Tok::Ident(w) => w.clone(),
                other => {
                    return Err(self.err(&format!("expected an item, found {other}")));
                }
            };
            match word.as_str() {
                "fn" => {
                    let f = self.parse_fn()?;
                    items.push(Item::Func(f));
                }
                "const" => {
                    self.bump();
                    let name = self.expect_ident()?;
                    if self.eat_punct(":") {
                        let _ = self.parse_type()?;
                    }
                    self.expect_punct("=")?;
                    let value = self.expr()?;
                    self.expect_punct(";")?;
                    items.push(Item::Const { name, value });
                }
                "static" => {
                    self.bump();
                    self.eat_kw("mut");
                    let name = self.expect_ident()?;
                    if self.eat_punct(":") {
                        let _ = self.parse_type()?;
                    }
                    self.expect_punct("=")?;
                    let value = self.expr()?;
                    self.expect_punct(";")?;
                    items.push(Item::Static { name, value });
                }
                "mod" => {
                    self.bump();
                    let _name = self.expect_ident()?;
                    if self.eat_punct(";") {
                        // out-of-line module: ignore
                    } else {
                        self.expect_punct("{")?;
                        self.parse_items(items, true)?;
                        self.expect_punct("}")?;
                    }
                }
                "use" | "type" => {
                    while !self.is_punct(";") {
                        if matches!(self.tok(), Tok::Eof) {
                            return Err(self.err("unexpected end of file"));
                        }
                        self.bump();
                    }
                    self.bump();
                }
                "macro_rules" => {
                    self.bump();
                    self.eat_punct("!");
                    let _name = self.expect_ident()?;
                    if self.is_punct("{") {
                        self.skip_balanced("{", "}")?;
                    }
                    self.eat_punct(";");
                }
                "struct" | "enum" | "impl" | "trait" | "union" => {
                    return Err(self.err(&format!(
                        "`{word}` is outside the mini-rust subset (only fn/const/static are supported)"
                    )));
                }
                other => {
                    return Err(self.err(&format!("unexpected item `{other}`")));
                }
            }
            if in_mod && self.is_punct("}") {
                return Ok(());
            }
        }
    }

    // ---------- helpers ----------

    fn tok(&self) -> &Tok {
        &self.toks[self.pos].tok
    }

    fn line(&self) -> u32 {
        self.toks[self.pos].line
    }

    fn peek_tok(&self, n: usize) -> Option<&Tok> {
        self.toks.get(self.pos + n).map(|t| &t.tok)
    }

    fn bump(&mut self) -> Tok {
        let t = self.toks[self.pos].tok.clone();
        if self.pos + 1 < self.toks.len() {
            self.pos += 1;
        }
        t
    }

    fn is_punct(&self, p: &str) -> bool {
        matches!(self.tok(), Tok::Punct(x) if *x == p)
    }

    fn peek_is_punct(&self, n: usize, p: &str) -> bool {
        matches!(self.peek_tok(n), Some(Tok::Punct(x)) if *x == p)
    }

    fn is_kw(&self, k: &str) -> bool {
        matches!(self.tok(), Tok::Ident(x) if x == k)
    }

    fn eat_punct(&mut self, p: &str) -> bool {
        if self.is_punct(p) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn eat_kw(&mut self, k: &str) -> bool {
        if self.is_kw(k) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_punct(&mut self, p: &str) -> Result<(), String> {
        if self.eat_punct(p) {
            Ok(())
        } else {
            Err(self.err(&format!("expected `{p}`, found {}", self.tok())))
        }
    }

    fn expect_ident(&mut self) -> Result<String, String> {
        match self.tok().clone() {
            Tok::Ident(s) => {
                self.bump();
                Ok(s)
            }
            other => Err(self.err(&format!("expected an identifier, found {other}"))),
        }
    }

    fn err(&self, msg: &str) -> String {
        let col = self.toks[self.pos].col;
        format!("line {}:{}: {}", self.line(), col, msg)
    }

    fn skip_attrs(&mut self) -> Result<(), String> {
        while self.is_punct("#") {
            self.bump();
            self.eat_punct("!");
            if self.is_punct("[") {
                self.skip_balanced("[", "]")?;
            } else {
                break;
            }
        }
        Ok(())
    }

    fn skip_balanced(&mut self, open: &str, close: &str) -> Result<(), String> {
        self.expect_punct(open)?;
        let mut depth = 1i32;
        while depth > 0 {
            if matches!(self.tok(), Tok::Eof) {
                return Err(self.err("unbalanced delimiter"));
            }
            if self.is_punct(open) {
                depth += 1;
            } else if self.is_punct(close) {
                depth -= 1;
            }
            self.bump();
        }
        Ok(())
    }

    fn skip_balanced_angle(&mut self) -> Result<(), String> {
        self.expect_punct("<")?;
        let mut depth = 1i32;
        while depth > 0 {
            if matches!(self.tok(), Tok::Eof) {
                return Err(self.err("unbalanced `<...>`"));
            }
            if self.is_punct("<") {
                depth += 1;
            } else if self.is_punct(">") {
                depth -= 1;
            } else if self.is_punct(">>") {
                depth -= 2;
                if depth < 0 {
                    return Err(self.err("unbalanced `<...>`"));
                }
            }
            self.bump();
        }
        Ok(())
    }

    // ---------- items ----------

    fn parse_fn(&mut self) -> Result<FuncAst, String> {
        if !self.eat_kw("fn") {
            return Err(self.err("expected `fn`"));
        }
        let name = self.expect_ident()?;
        if self.is_punct("<") {
            self.skip_balanced_angle()?;
        }
        self.expect_punct("(")?;
        let mut params = Vec::new();
        while !self.is_punct(")") {
            self.eat_kw("mut");
            let pat = self.parse_pat()?;
            let ty = if self.eat_punct(":") { Some(self.parse_type()?) } else { None };
            params.push((pat, ty));
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(")")?;
        let ret = if self.eat_punct("->") {
            Some(self.parse_type()?)
        } else {
            None
        };
        if self.eat_kw("where") {
            while !self.is_punct("{") && !matches!(self.tok(), Tok::Eof) {
                self.bump();
            }
        }
        let body = self.parse_block()?;
        Ok(FuncAst { name, params, ret, body })
    }

    // ---------- types ----------

    fn parse_type(&mut self) -> Result<Type, String> {
        if self.eat_punct("&") {
            if matches!(self.tok(), Tok::Lifetime) {
                self.bump();
            }
            self.eat_kw("mut");
            let inner = self.parse_type()?;
            return Ok(Type::Ref(Box::new(inner)));
        }
        if self.eat_punct("(") {
            if self.eat_punct(")") {
                return Ok(Type::Unit);
            }
            let mut ts = vec![self.parse_type()?];
            while self.eat_punct(",") {
                if self.is_punct(")") {
                    break;
                }
                ts.push(self.parse_type()?);
            }
            self.expect_punct(")")?;
            return Ok(if ts.len() == 1 { ts.remove(0) } else { Type::Tuple(ts) });
        }
        if self.eat_punct("[") {
            let inner = self.parse_type()?;
            if self.eat_punct(";") {
                let _ = self.expr()?;
            }
            self.expect_punct("]")?;
            return Ok(Type::Slice(Box::new(inner)));
        }
        if self.eat_punct("!") {
            return Ok(Type::Unit);
        }
        if self.is_punct("*") {
            self.bump();
            self.eat_kw("mut");
            self.eat_kw("const");
            return self.parse_type();
        }
        if self.eat_kw("dyn") || self.eat_kw("impl") {
            let _ = self.parse_type()?;
            return Ok(Type::Other("dyn".into()));
        }
        if self.eat_kw("fn") {
            if self.is_punct("(") {
                self.skip_balanced("(", ")")?;
            }
            if self.eat_punct("->") {
                let _ = self.parse_type()?;
            }
            return Ok(Type::Other("fn".into()));
        }
        let mut path = vec![self.expect_ident()?];
        loop {
            if self.is_punct("::") {
                if self.peek_is_punct(1, "<") {
                    self.bump();
                    self.skip_balanced_angle()?;
                    continue;
                }
                self.bump();
                let seg = self.expect_ident()?;
                path.push(seg);
            } else {
                break;
            }
        }
        if self.is_punct("<") {
            self.skip_balanced_angle()?;
        }
        let last = path.last().unwrap().as_str();
        if let Some(k) = IntKind::from_name(last) {
            return Ok(Type::Int(k));
        }
        Ok(match last {
            "f32" => Type::F32,
            "f64" => Type::F64,
            "bool" => Type::Bool,
            "char" => Type::Char,
            "String" => Type::String,
            "str" => Type::Str,
            "Vec" => Type::Vec(Box::new(Type::Other("T".into()))),
            _ => Type::Other(path.join("::")),
        })
    }

    fn parse_type_list_until_gt(&mut self) -> Result<Vec<Type>, String> {
        let mut out = Vec::new();
        if self.eat_punct(">") {
            return Ok(out);
        }
        loop {
            out.push(self.parse_type()?);
            if self.eat_punct(",") {
                if self.eat_punct(">") {
                    return Ok(out);
                }
                continue;
            }
            self.expect_punct(">")?;
            return Ok(out);
        }
    }

    // ---------- patterns ----------

    fn parse_pat(&mut self) -> Result<Pat, String> {
        if self.eat_punct("&") {
            self.eat_kw("mut");
            return self.parse_pat();
        }
        self.eat_kw("ref");
        self.eat_kw("mut");
        if let Tok::Ident(w) = self.tok().clone() {
            if w == "_" {
                self.bump();
                return Ok(Pat::Wild);
            }
            if w == "mut" {
                self.bump();
                let name = self.expect_ident()?;
                return Ok(Pat::Name(name));
            }
            self.bump();
            if self.is_punct("::") {
                // enum variant pattern: not modelled, treat by last segment
                let mut last = w;
                while self.eat_punct("::") {
                    last = self.expect_ident()?;
                }
                return Ok(Pat::Name(last));
            }
            return Ok(Pat::Name(w));
        }
        if self.eat_punct("(") {
            let mut pats = Vec::new();
            while !self.is_punct(")") {
                pats.push(self.parse_pat()?);
                if !self.eat_punct(",") {
                    break;
                }
            }
            self.expect_punct(")")?;
            if pats.len() == 1 {
                return Ok(pats.remove(0));
            }
            return Ok(Pat::Tuple(pats));
        }
        Err(self.err(&format!("unsupported pattern, found {}", self.tok())))
    }

    // ---------- statements ----------

    fn parse_block(&mut self) -> Result<Block, String> {
        self.expect_punct("{")?;
        let mut stmts = Vec::new();
        while !self.is_punct("}") {
            if matches!(self.tok(), Tok::Eof) {
                return Err(self.err("unexpected end of file inside a block"));
            }
            self.skip_attrs()?;
            if self.is_punct(";") {
                self.bump();
                continue;
            }
            self.parse_stmt(&mut stmts)?;
        }
        self.expect_punct("}")?;
        Ok(Block { stmts })
    }

    fn parse_stmt(&mut self, stmts: &mut Vec<Stmt>) -> Result<(), String> {
        if let Tok::Lifetime = self.tok() {
            self.bump();
            self.eat_punct(":");
        }
        if self.is_kw("let") {
            self.bump();
            let pat = self.parse_pat()?;
            let ty = if self.eat_punct(":") { Some(self.parse_type()?) } else { None };
            let init = if self.eat_punct("=") { Some(self.expr()?) } else { None };
            if self.is_kw("else") {
                return Err(self.err("`let ... else` is not supported"));
            }
            self.expect_punct(";")?;
            stmts.push(Stmt::Let { pat, ty, init });
            return Ok(());
        }
        if self.is_kw("use") {
            while !self.is_punct(";") && !matches!(self.tok(), Tok::Eof) {
                self.bump();
            }
            self.eat_punct(";");
            return Ok(());
        }
        if self.is_kw("struct") || self.is_kw("enum") || self.is_kw("impl") || self.is_kw("trait")
        {
            return Err(self.err("items inside a function body are outside the mini-rust subset"));
        }
        if self.is_kw("fn") {
            return Err(self.err("nested functions are not supported (move it to the top level)"));
        }

        let expr = self.expr()?;
        if self.eat_punct(";") {
            stmts.push(Stmt::Expr { expr, semi: true });
            return Ok(());
        }
        if self.is_punct("}") {
            stmts.push(Stmt::Expr { expr, semi: false });
            return Ok(());
        }
        if is_statement_like(&expr) {
            stmts.push(Stmt::Expr { expr, semi: false });
            return Ok(());
        }
        Err(self.err(&format!("expected `;`, found {}", self.tok())))
    }

    // ---------- expressions ----------

    pub fn expr(&mut self) -> Result<Expr, String> {
        self.assign_expr()
    }

    fn assign_expr(&mut self) -> Result<Expr, String> {
        let lhs = self.range_expr()?;
        let op = match self.tok() {
            Tok::Punct("=") => Some(None),
            Tok::Punct("+=") => Some(Some(BinOp::Add)),
            Tok::Punct("-=") => Some(Some(BinOp::Sub)),
            Tok::Punct("*=") => Some(Some(BinOp::Mul)),
            Tok::Punct("/=") => Some(Some(BinOp::Div)),
            Tok::Punct("%=") => Some(Some(BinOp::Rem)),
            Tok::Punct("&=") => Some(Some(BinOp::BitAnd)),
            Tok::Punct("|=") => Some(Some(BinOp::BitOr)),
            Tok::Punct("^=") => Some(Some(BinOp::BitXor)),
            Tok::Punct("<<=") => Some(Some(BinOp::Shl)),
            Tok::Punct(">>=") => Some(Some(BinOp::Shr)),
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let rhs = self.assign_expr()?;
            return Ok(Expr::Assign(op, Box::new(lhs), Box::new(rhs)));
        }
        Ok(lhs)
    }

    fn can_start_expr(&self) -> bool {
        !matches!(
            self.tok(),
            Tok::Eof | Tok::Punct(")") | Tok::Punct("]") | Tok::Punct("}") | Tok::Punct(",")
                | Tok::Punct(";") | Tok::Punct("..") | Tok::Punct("..=")
        )
    }

    fn range_expr(&mut self) -> Result<Expr, String> {
        if self.is_punct("..") || self.is_punct("..=") {
            let inclusive = self.is_punct("..=");
            self.bump();
            let end = if self.can_start_expr() {
                Some(Box::new(self.binary_expr(0)?))
            } else {
                None
            };
            return Ok(Expr::Range { start: None, end, inclusive });
        }
        let start = self.binary_expr(0)?;
        if self.is_punct("..") || self.is_punct("..=") {
            let inclusive = self.is_punct("..=");
            self.bump();
            let end = if self.can_start_expr() {
                Some(Box::new(self.binary_expr(0)?))
            } else {
                None
            };
            return Ok(Expr::Range { start: Some(Box::new(start)), end, inclusive });
        }
        Ok(start)
    }

    fn binary_expr(&mut self, min_prec: u8) -> Result<Expr, String> {
        let mut lhs = self.cast_expr()?;
        loop {
            let (op, prec) = match self.tok() {
                Tok::Punct("||") => (BinOp::Or, 1),
                Tok::Punct("&&") => (BinOp::And, 2),
                Tok::Punct("|") => (BinOp::BitOr, 3),
                Tok::Punct("^") => (BinOp::BitXor, 4),
                Tok::Punct("&") => (BinOp::BitAnd, 5),
                Tok::Punct("==") => (BinOp::Eq, 6),
                Tok::Punct("!=") => (BinOp::Ne, 6),
                Tok::Punct("<") => (BinOp::Lt, 6),
                Tok::Punct("<=") => (BinOp::Le, 6),
                Tok::Punct(">") => (BinOp::Gt, 6),
                Tok::Punct(">=") => (BinOp::Ge, 6),
                Tok::Punct("<<") => (BinOp::Shl, 7),
                Tok::Punct(">>") => (BinOp::Shr, 7),
                Tok::Punct("+") => (BinOp::Add, 8),
                Tok::Punct("-") => (BinOp::Sub, 8),
                Tok::Punct("*") => (BinOp::Mul, 9),
                Tok::Punct("/") => (BinOp::Div, 9),
                Tok::Punct("%") => (BinOp::Rem, 9),
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            self.bump();
            let rhs = self.binary_expr(prec + 1)?;
            lhs = Expr::Binary(op, Box::new(lhs), Box::new(rhs));
        }
        Ok(lhs)
    }

    fn cast_expr(&mut self) -> Result<Expr, String> {
        let mut e = self.unary_expr()?;
        while self.is_kw("as") {
            self.bump();
            let ty = self.parse_type()?;
            e = Expr::Cast(Box::new(e), ty);
        }
        Ok(e)
    }

    fn unary_expr(&mut self) -> Result<Expr, String> {
        if self.is_punct("-") {
            self.bump();
            let e = self.unary_expr()?;
            if let Expr::Int(v) = e {
                return Ok(Expr::Int(v.wrapping_neg()));
            }
            return Ok(Expr::Unary(UnOp::Neg, Box::new(e)));
        }
        if self.is_punct("!") {
            self.bump();
            let e = self.unary_expr()?;
            return Ok(Expr::Unary(UnOp::Not, Box::new(e)));
        }
        if self.is_punct("*") {
            self.bump();
            let e = self.unary_expr()?;
            return Ok(Expr::Unary(UnOp::Deref, Box::new(e)));
        }
        if self.is_punct("&") {
            self.bump();
            self.eat_kw("mut");
            let e = self.unary_expr()?;
            return Ok(Expr::Ref(Box::new(e)));
        }
        self.postfix_expr()
    }

    fn parse_call_args(&mut self) -> Result<Vec<Expr>, String> {
        let mut args = Vec::new();
        while !self.is_punct(")") {
            args.push(self.expr()?);
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok(args)
    }

    fn postfix_expr(&mut self) -> Result<Expr, String> {
        let mut e = self.primary_expr()?;
        loop {
            if self.eat_punct(".") {
                if let Tok::Int(n) = self.tok().clone() {
                    self.bump();
                    e = Expr::Field(Box::new(e), n.to_string());
                    continue;
                }
                let name = self.expect_ident()?;
                let mut turbofish = Vec::new();
                if self.is_punct("::") && self.peek_is_punct(1, "<") {
                    self.bump();
                    self.expect_punct("<")?;
                    turbofish = self.parse_type_list_until_gt()?;
                }
                if self.is_punct("(") {
                    self.bump();
                    let args = self.parse_call_args()?;
                    e = Expr::Method(Box::new(e), name, turbofish, args);
                } else {
                    e = Expr::Field(Box::new(e), name);
                }
            } else if self.eat_punct("[") {
                let idx = self.expr()?;
                self.expect_punct("]")?;
                e = Expr::Index(Box::new(e), Box::new(idx));
            } else if self.is_punct("(") {
                self.bump();
                let args = self.parse_call_args()?;
                e = Expr::Call(Box::new(e), args);
            } else if self.is_punct("?") {
                return Err(self.err("the `?` operator is outside the mini-rust subset"));
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn primary_expr(&mut self) -> Result<Expr, String> {
        match self.tok().clone() {
            Tok::Int(v) => {
                self.bump();
                Ok(Expr::Int(v))
            }
            Tok::Float(v) => {
                self.bump();
                Ok(Expr::Float(v))
            }
            Tok::Str(s) => {
                self.bump();
                Ok(Expr::Str(s))
            }
            Tok::Char(c) => {
                self.bump();
                Ok(Expr::Char(c))
            }
            Tok::Punct("(") => {
                self.bump();
                if self.eat_punct(")") {
                    return Ok(Expr::Tuple(Vec::new()));
                }
                let first = self.expr()?;
                if self.eat_punct(",") {
                    let mut elems = vec![first];
                    while !self.is_punct(")") {
                        elems.push(self.expr()?);
                        if !self.eat_punct(",") {
                            break;
                        }
                    }
                    self.expect_punct(")")?;
                    return Ok(Expr::Tuple(elems));
                }
                self.expect_punct(")")?;
                Ok(first)
            }
            Tok::Punct("[") => {
                self.bump();
                if self.eat_punct("]") {
                    return Ok(Expr::Array(Vec::new()));
                }
                let first = self.expr()?;
                if self.eat_punct(";") {
                    let n = self.expr()?;
                    self.expect_punct("]")?;
                    return Ok(Expr::Repeat(Box::new(first), Box::new(n)));
                }
                let mut elems = vec![first];
                while self.eat_punct(",") {
                    if self.is_punct("]") {
                        break;
                    }
                    elems.push(self.expr()?);
                }
                self.expect_punct("]")?;
                Ok(Expr::Array(elems))
            }
            Tok::Punct("{") => Ok(Expr::Block(self.parse_block()?)),
            Tok::Punct("|") | Tok::Punct("||") => {
                Err(self.err("closures are outside the mini-rust subset"))
            }
            Tok::Ident(w) => {
                self.bump();
                match w.as_str() {
                    "true" => return Ok(Expr::Bool(true)),
                    "false" => return Ok(Expr::Bool(false)),
                    "if" => return self.parse_if(),
                    "while" => return self.parse_while(),
                    "loop" => {
                        let body = self.parse_block()?;
                        return Ok(Expr::Loop { body });
                    }
                    "for" => return self.parse_for(),
                    "match" => {
                        return Err(self.err("`match` is outside the mini-rust subset"));
                    }
                    "return" => {
                        let value = if self.can_start_expr() {
                            Some(Box::new(self.expr()?))
                        } else {
                            None
                        };
                        return Ok(Expr::Return(value));
                    }
                    "break" => {
                        if matches!(self.tok(), Tok::Lifetime) {
                            return Err(self
                                .err("labelled `break` is outside the mini-rust subset"));
                        }
                        let value = if self.can_start_expr() {
                            Some(Box::new(self.expr()?))
                        } else {
                            None
                        };
                        return Ok(Expr::Break(value));
                    }
                    "continue" => {
                        if matches!(self.tok(), Tok::Lifetime) {
                            return Err(self
                                .err("labelled `continue` is outside the mini-rust subset"));
                        }
                        return Ok(Expr::Continue);
                    }
                    "unsafe" | "async" | "move" => {
                        if self.is_punct("{") {
                            return Ok(Expr::Block(self.parse_block()?));
                        }
                        return Err(self.err("expected `{`"));
                    }
                    "dyn" | "impl" => {
                        return Err(self.err("`dyn`/`impl` types are outside the subset"));
                    }
                    _ => {}
                }
                if self.is_punct("!") {
                    self.bump();
                    return self.parse_macro(w);
                }
                let mut path = vec![w];
                loop {
                    if self.is_punct("::") {
                        if self.peek_is_punct(1, "<") {
                            self.bump();
                            self.skip_balanced_angle()?;
                            continue;
                        }
                        self.bump();
                        match self.tok().clone() {
                            Tok::Ident(seg) => {
                                self.bump();
                                path.push(seg);
                            }
                            other => {
                                return Err(self.err(&format!(
                                    "expected an identifier after `::`, found {other}"
                                )))
                            }
                        }
                    } else {
                        break;
                    }
                }
                if path.len() == 1 {
                    Ok(Expr::Ident(path.pop().unwrap()))
                } else {
                    Ok(Expr::Path(path))
                }
            }
            other => Err(self.err(&format!("expected an expression, found {other}"))),
        }
    }

    fn parse_if(&mut self) -> Result<Expr, String> {
        let cond = self.expr()?;
        let then_b = self.parse_block()?;
        let else_b = if self.eat_kw("else") {
            if self.is_kw("if") {
                self.bump();
                Some(Box::new(self.parse_if()?))
            } else if self.is_punct("{") {
                Some(Box::new(Expr::Block(self.parse_block()?)))
            } else {
                return Err(self.err("expected `if` or `{` after `else`"));
            }
        } else {
            None
        };
        Ok(Expr::If { cond: Box::new(cond), then_b, else_b })
    }

    fn parse_while(&mut self) -> Result<Expr, String> {
        let cond = self.expr()?;
        let body = self.parse_block()?;
        Ok(Expr::While { cond: Box::new(cond), body })
    }

    fn parse_for(&mut self) -> Result<Expr, String> {
        let pat = self.parse_pat()?;
        if !self.eat_kw("in") {
            return Err(self.err("expected `in` after the `for` pattern"));
        }
        let iter = self.expr()?;
        let body = self.parse_block()?;
        Ok(Expr::For { pat, iter: Box::new(iter), body })
    }

    fn parse_macro(&mut self, name: String) -> Result<Expr, String> {
        let close = match self.tok() {
            Tok::Punct("(") => ")",
            Tok::Punct("[") => "]",
            Tok::Punct("{") => "}",
            other => return Err(self.err(&format!("expected a macro delimiter, found {other}"))),
        };
        self.bump();
        let mut args = Vec::new();
        let mut sep = MacroSep::Comma;
        while !self.is_punct(close) {
            args.push(self.expr()?);
            if self.eat_punct(";") {
                sep = MacroSep::Semicolon;
                args.push(self.expr()?);
                break;
            }
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(close)?;
        Ok(Expr::Macro { name, args, sep })
    }
}

fn is_statement_like(e: &Expr) -> bool {
    matches!(
        e,
        Expr::If { .. }
            | Expr::While { .. }
            | Expr::Loop { .. }
            | Expr::For { .. }
            | Expr::Block(_)
            | Expr::Macro { .. }
    )
}
