use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum BinOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Pow,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum UnaryOp {
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CallArg {
    Positional(Expr),
    Named { name: String, value: Expr },
}

#[derive(Debug, Clone, PartialEq)]
pub enum FStringPart {
    Literal(String),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Literal {
    None,
    Bool(bool),
    Number(String),
    String(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeAnn {
    None,
    Named(String),
    Array { inner: String },
    Dict { key: String, value: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Literal(Literal),
    Variable { name: String, line: u32 },
    Group(Box<Expr>),
    Unary { op: UnaryOp, right: Box<Expr> },
    Binary { op: BinOp, left: Box<Expr>, right: Box<Expr> },
    Assign { name: String, value: Box<Expr> },
    GetField { object: String, field: String },
    SetField {
        object: String,
        field: String,
        value: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<CallArg>,
    },
    CallMethod {
        object: String,
        method: String,
        args: Vec<Expr>,
    },
    List(Vec<Expr>),
    Dict(Vec<(Expr, Expr)>),
    Index {
        object: Box<Expr>,
        index: Box<Expr>,
    },
    IndexSet {
        object: Box<Expr>,
        index: Box<Expr>,
        value: Box<Expr>,
    },
    FString { line: u32, parts: Vec<FStringPart> },
    Ternary {
        condition: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Param {
    pub name: String,
    pub is_ref: bool,
    pub type_ann: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub is_strict: bool,
    pub params: Vec<Param>,
    pub return_type: Option<String>,
    pub body: Box<Stmt>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructField {
    pub name: String,
    pub type_name: String,
    pub is_pub: bool,
    pub is_mut: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MethodDecl {
    pub is_pub: bool,
    pub function: FunctionDecl,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForKind {
    Seq,
    Parallel,
    Vectorized,
    ParallelVectorized,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForIter {
    Range { start: Expr, end: Expr },
    Iterable(Expr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        line: u32,
        is_mutable: bool,
        name: String,
        type_ann: TypeAnn,
        initializer: Expr,
    },
    Print { line: u32, values: Vec<Expr> },
    Expr { line: u32, expr: Expr },
    Block(Vec<Stmt>),
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    While {
        line: u32,
        condition: Expr,
        body: Box<Stmt>,
    },
    For {
        kind: ForKind,
        line: u32,
        var: String,
        iter: ForIter,
        body: Box<Stmt>,
    },
    Function(FunctionDecl),
    Return { line: u32, value: Expr },
    Struct {
        name: String,
        implemented_trait: Option<String>,
        fields: Vec<StructField>,
        methods: Vec<MethodDecl>,
    },
    Trait {
        name: String,
        methods: Vec<FunctionDecl>,
    },
    WithMmap {
        line: u32,
        path: Expr,
        var: String,
        body: Box<Stmt>,
    },
}

impl fmt::Display for BinOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            BinOp::Add => "+",
            BinOp::Sub => "-",
            BinOp::Mul => "*",
            BinOp::Div => "/",
            BinOp::Rem => "%",
            BinOp::Pow => "**",
            BinOp::Eq => "==",
            BinOp::Ne => "!=",
            BinOp::Lt => "<",
            BinOp::Le => "<=",
            BinOp::Gt => ">",
            BinOp::Ge => ">=",
            BinOp::And => "and",
            BinOp::Or => "or",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for UnaryOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UnaryOp::Neg => write!(f, "-"),
            UnaryOp::Not => write!(f, "not"),
        }
    }
}

impl fmt::Display for CallArg {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CallArg::Positional(e) => write!(f, "{}", e),
            CallArg::Named { name, value } => write!(f, "{}={}", name, value),
        }
    }
}

impl fmt::Display for FStringPart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FStringPart::Literal(s) => {
                let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
                write!(f, "\"{}\"", escaped)
            }
            FStringPart::Expr(e) => write!(f, "{}", e),
        }
    }
}

impl fmt::Display for Literal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Literal::None => write!(f, "None"),
            Literal::Bool(b) => write!(f, "{}", b),
            Literal::Number(n) => write!(f, "{}", n),
            Literal::String(s) => {
                let escaped = s.replace('\\', "\\\\").replace('"', "\\\"");
                write!(f, "\"{}\"", escaped)
            }
        }
    }
}

impl fmt::Display for TypeAnn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TypeAnn::None => write!(f, "None"),
            TypeAnn::Named(n) => write!(f, "{}", n),
            TypeAnn::Array { inner } => write!(f, "Array[{}]", inner),
            TypeAnn::Dict { key, value } => write!(f, "Dict[{}, {}]", key, value),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Expr::Literal(lit) => write!(f, "{}", lit),
            Expr::Variable { name, .. } => write!(f, "let_ref:{}", name),
            Expr::Group(e) => write!(f, "(group {})", e),
            Expr::Unary { op, right } => write!(f, "({} {})", op, right),
            Expr::Binary { op, left, right } => write!(f, "({} {} {})", op, left, right),
            Expr::Assign { name, value } => write!(f, "(assign {} {})", name, value),
            Expr::GetField { object, field } => write!(f, "(get_field {} {})", object, field),
            Expr::SetField {
                object,
                field,
                value,
            } => write!(f, "(set_field {} {} {})", object, field, value),
            Expr::Call { callee, args } => {
                if args.is_empty() {
                    write!(f, "(call {})", callee)
                } else {
                    let args_str: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                    write!(f, "(call {} {})", callee, args_str.join(" "))
                }
            }
            Expr::CallMethod {
                object,
                method,
                args,
            } => {
                let args_str: Vec<String> = args.iter().map(|a| a.to_string()).collect();
                write!(
                    f,
                    "(call_method {} {} [{}])",
                    object,
                    method,
                    args_str.join(" ")
                )
            }
            Expr::List(items) => {
                if items.is_empty() {
                    write!(f, "(list)")
                } else {
                    let items_str: Vec<String> = items.iter().map(|e| e.to_string()).collect();
                    write!(f, "(list {})", items_str.join(" "))
                }
            }
            Expr::Dict(entries) => {
                let entries_str: Vec<String> = entries
                    .iter()
                    .map(|(k, v)| format!("{}:{}", k, v))
                    .collect();
                write!(f, "(dict {})", entries_str.join(" "))
            }
            Expr::Index { object, index } => write!(f, "(index {} {})", object, index),
            Expr::IndexSet {
                object,
                index,
                value,
            } => write!(f, "(index_set {} {} {})", object, index, value),
            Expr::FString { line, parts } => {
                let parts_str: Vec<String> = parts.iter().map(|p| p.to_string()).collect();
                write!(f, "(f_string line:{} [{}])", line, parts_str.join(" "))
            }
            Expr::Ternary {
                condition,
                then_branch,
                else_branch,
            } => write!(f, "(if {} {} {})", condition, then_branch, else_branch),
        }
    }
}

impl fmt::Display for Param {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

impl fmt::Display for FunctionDecl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let params: Vec<String> = self.params.iter().map(|p| p.to_string()).collect();
        write!(
            f,
            "(fn {} strict:{} (params {}) {})",
            self.name,
            self.is_strict,
            params.join(" "),
            self.body
        )
    }
}

impl fmt::Display for ForKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ForKind::Seq => "for_seq",
            ForKind::Parallel => "for_par",
            ForKind::Vectorized => "for_vec",
            ForKind::ParallelVectorized => "for_par_vec",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for ForIter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForIter::Range { start, end } => write!(f, "range({}, {})", start, end),
            ForIter::Iterable(e) => write!(f, "{}", e),
        }
    }
}

impl fmt::Display for Stmt {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Stmt::Let {
                line,
                is_mutable,
                name,
                type_ann,
                initializer,
            } => {
                let mut_str = if *is_mutable { "mut" } else { "immut" };
                write!(
                    f,
                    "(let line:{} {} {} type:{} {})",
                    line, mut_str, name, type_ann, initializer
                )
            }
            Stmt::Print { line, values } => {
                let vals: Vec<String> = values.iter().map(|v| v.to_string()).collect();
                write!(f, "(print line:{} {})", line, vals.join(" "))
            }
            Stmt::Expr { line, expr } => write!(f, "(expr line:{} {})", line, expr),
            Stmt::Block(stmts) => {
                if stmts.is_empty() {
                    write!(f, "(block)")
                } else {
                    let inner: Vec<String> = stmts.iter().map(|s| s.to_string()).collect();
                    write!(f, "(block {})", inner.join(" "))
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if let Some(else_b) = else_branch {
                    write!(f, "(if {} {} {})", condition, then_branch, else_b)
                } else {
                    write!(f, "(if {} {})", condition, then_branch)
                }
            }
            Stmt::While {
                line,
                condition,
                body,
            } => write!(f, "(while line:{} {} {})", line, condition, body),
            Stmt::For {
                kind,
                line,
                var,
                iter,
                body,
            } => write!(
                f,
                "({} line:{} {} in {} {})",
                kind, line, var, iter, body
            ),
            Stmt::Function(decl) => write!(f, "{}", decl),
            Stmt::Return { line, value } => write!(f, "(return line:{} {})", line, value),
            Stmt::Struct {
                name,
                implemented_trait,
                fields,
                methods,
            } => {
                let trait_name = implemented_trait.as_deref().unwrap_or("");
                let fields_str: Vec<String> = fields
                    .iter()
                    .map(|field| {
                        format!(
                            "{}:{} (pub:{}, mut:{})",
                            field.name, field.type_name, field.is_pub, field.is_mut
                        )
                    })
                    .collect();
                let methods_str: Vec<String> = methods
                    .iter()
                    .map(|m| format!("(pub:{} {})", m.is_pub, m.function))
                    .collect();
                write!(
                    f,
                    "(struct {} trait:{} fields:[{}] methods:[{}])",
                    name,
                    trait_name,
                    fields_str.join(", "),
                    methods_str.join(" ")
                )
            }
            Stmt::Trait { name, methods } => {
                let methods_str: Vec<String> = methods.iter().map(|m| m.to_string()).collect();
                write!(
                    f,
                    "(trait {} methods:[{}])",
                    name,
                    methods_str.join(" ")
                )
            }
            Stmt::WithMmap {
                line,
                path,
                var,
                body,
            } => write!(f, "(with_mmap line:{} {} {} {})", line, path, var, body),
        }
    }
}
