//! AST for the subset of Rust that mini-rust understands.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IntKind {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Usize,
    Isize,
}

impl IntKind {
    pub fn from_name(name: &str) -> Option<IntKind> {
        Some(match name {
            "i8" => IntKind::I8,
            "i16" => IntKind::I16,
            "i32" => IntKind::I32,
            "i64" => IntKind::I64,
            "u8" => IntKind::U8,
            "u16" => IntKind::U16,
            "u32" => IntKind::U32,
            "u64" => IntKind::U64,
            "usize" => IntKind::Usize,
            "isize" => IntKind::Isize,
            _ => return None,
        })
    }

    pub fn bits(&self) -> u32 {
        match self {
            IntKind::I8 | IntKind::U8 => 8,
            IntKind::I16 | IntKind::U16 => 16,
            IntKind::I32 | IntKind::U32 => 32,
            IntKind::I64 | IntKind::U64 | IntKind::Usize | IntKind::Isize => 64,
        }
    }

    pub fn is_signed(&self) -> bool {
        matches!(
            self,
            IntKind::I8 | IntKind::I16 | IntKind::I32 | IntKind::I64 | IntKind::Isize
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Int(IntKind),
    F32,
    F64,
    Bool,
    Char,
    Str,
    String,
    Vec(Box<Type>),
    Slice(Box<Type>),
    Unit,
    Tuple(Vec<Type>),
    Ref(Box<Type>),
    Other(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnOp {
    Neg,
    Not,
    Deref,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Pat {
    Name(String),
    Wild,
    Tuple(Vec<Pat>),
}

#[derive(Clone, Debug)]
pub enum Stmt {
    Let {
        pat: Pat,
        ty: Option<Type>,
        init: Option<Expr>,
    },
    Expr {
        expr: Expr,
        semi: bool,
    },
}

#[derive(Clone, Debug, Default)]
pub struct Block {
    pub stmts: Vec<Stmt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MacroSep {
    Comma,
    Semicolon,
}

#[derive(Clone, Debug)]
pub enum Expr {
    Int(i64),
    Float(f64),
    Bool(bool),
    Str(String),
    Char(char),
    Ident(String),
    Path(Vec<String>),
    Unary(UnOp, Box<Expr>),
    Binary(BinOp, Box<Expr>, Box<Expr>),
    Assign(Option<BinOp>, Box<Expr>, Box<Expr>),
    Call(Box<Expr>, Vec<Expr>),
    Method(Box<Expr>, String, Vec<Type>, Vec<Expr>),
    Index(Box<Expr>, Box<Expr>),
    Field(Box<Expr>, String),
    Cast(Box<Expr>, Type),
    Ref(Box<Expr>),
    Array(Vec<Expr>),
    Repeat(Box<Expr>, Box<Expr>),
    Tuple(Vec<Expr>),
    Block(Block),
    If {
        cond: Box<Expr>,
        then_b: Block,
        else_b: Option<Box<Expr>>,
    },
    While {
        cond: Box<Expr>,
        body: Block,
    },
    Loop {
        body: Block,
    },
    For {
        pat: Pat,
        iter: Box<Expr>,
        body: Block,
    },
    Range {
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        inclusive: bool,
    },
    Macro {
        name: String,
        args: Vec<Expr>,
        sep: MacroSep,
    },
    Break(Option<Box<Expr>>),
    Continue,
    Return(Option<Box<Expr>>),
}

#[derive(Clone, Debug)]
pub struct FuncAst {
    pub name: String,
    pub params: Vec<(Pat, Option<Type>)>,
    pub body: Block,
}

#[derive(Clone, Debug)]
pub enum Item {
    Func(FuncAst),
    Const {
        name: String,
        value: Expr,
    },
    Static {
        name: String,
        value: Expr,
    },
}

#[derive(Clone, Debug, Default)]
pub struct ProgramAst {
    pub items: Vec<Item>,
}
