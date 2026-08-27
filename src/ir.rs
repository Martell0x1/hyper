use std::fmt;

pub type ValueId = u32;
pub type BlockId = u32;

#[derive(Debug, Clone, PartialEq)]
pub enum IrOp {
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
    Neg,
    Not,
}

#[derive(Debug, Clone, PartialEq)]
pub enum IrInstr {
    ConstI64 { dest: ValueId, value: i64 },
    ConstF64 { dest: ValueId, value: f64 },
    ConstBool { dest: ValueId, value: bool },
    ConstStr { dest: ValueId, value: String },
    ConstNone { dest: ValueId },
    Load { dest: ValueId, name: String },
    Store { name: String, value: ValueId },
    Unary { dest: ValueId, op: IrOp, src: ValueId },
    Binary {
        dest: ValueId,
        op: IrOp,
        left: ValueId,
        right: ValueId,
    },
    Call {
        dest: ValueId,
        func: String,
        args: Vec<ValueId>,
    },
    Print { args: Vec<ValueId> },
    Return { value: Option<ValueId> },
    Jump { target: BlockId },
    Branch {
        cond: ValueId,
        then_block: BlockId,
        else_block: BlockId,
    },
    Label { block: BlockId },
    ParallelForBegin {
        var: String,
        start: ValueId,
        end: ValueId,
        vectorized: bool,
    },
    ParallelForEnd,
}

#[derive(Debug, Clone)]
pub struct IrFunction {
    pub name: String,
    pub params: Vec<String>,
    pub body: Vec<IrInstr>,
}

#[derive(Debug, Clone)]
pub struct IrModule {
    pub functions: Vec<IrFunction>,
    pub main: Vec<IrInstr>,
}

impl fmt::Display for IrOp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            IrOp::Add => "add",
            IrOp::Sub => "sub",
            IrOp::Mul => "mul",
            IrOp::Div => "div",
            IrOp::Rem => "rem",
            IrOp::Pow => "pow",
            IrOp::Eq => "eq",
            IrOp::Ne => "ne",
            IrOp::Lt => "lt",
            IrOp::Le => "le",
            IrOp::Gt => "gt",
            IrOp::Ge => "ge",
            IrOp::And => "and",
            IrOp::Or => "or",
            IrOp::Neg => "neg",
            IrOp::Not => "not",
        };
        write!(f, "{}", s)
    }
}

impl fmt::Display for IrInstr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IrInstr::ConstI64 { dest, value } => write!(f, "  v{} = const.i64 {}", dest, value),
            IrInstr::ConstF64 { dest, value } => write!(f, "  v{} = const.f64 {}", dest, value),
            IrInstr::ConstBool { dest, value } => write!(f, "  v{} = const.bool {}", dest, value),
            IrInstr::ConstStr { dest, value } => {
                let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
                write!(f, "  v{} = const.str \"{}\"", dest, escaped)
            }
            IrInstr::ConstNone { dest } => write!(f, "  v{} = const.none", dest),
            IrInstr::Load { dest, name } => write!(f, "  v{} = load {}", dest, name),
            IrInstr::Store { name, value } => write!(f, "  store {} v{}", name, value),
            IrInstr::Unary { dest, op, src } => write!(f, "  v{} = {} v{}", dest, op, src),
            IrInstr::Binary {
                dest,
                op,
                left,
                right,
            } => write!(f, "  v{} = {} v{} v{}", dest, op, left, right),
            IrInstr::Call { dest, func, args } => {
                if args.is_empty() {
                    write!(f, "  v{} = call {}()", dest, func)
                } else {
                    let args_str: Vec<String> = args.iter().map(|a| format!("v{}", a)).collect();
                    write!(f, "  v{} = call {}({})", dest, func, args_str.join(", "))
                }
            }
            IrInstr::Print { args } => {
                let args_str: Vec<String> = args.iter().map(|a| format!("v{}", a)).collect();
                write!(f, "  print {}", args_str.join(", "))
            }
            IrInstr::Return { value } => match value {
                Some(v) => write!(f, "  return v{}", v),
                None => write!(f, "  return"),
            },
            IrInstr::Jump { target } => write!(f, "  jump b{}", target),
            IrInstr::Branch {
                cond,
                then_block,
                else_block,
            } => write!(
                f,
                "  branch v{} then b{} else b{}",
                cond, then_block, else_block
            ),
            IrInstr::Label { block } => write!(f, "b{}:", block),
            IrInstr::ParallelForBegin {
                var,
                start,
                end,
                vectorized,
            } => write!(
                f,
                "  parallel_for_begin {} v{}..v{} vectorized={}",
                var, start, end, vectorized
            ),
            IrInstr::ParallelForEnd => write!(f, "  parallel_for_end"),
        }
    }
}

impl fmt::Display for IrFunction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "fn {}({}) {{", self.name, self.params.join(", "))?;
        for instr in &self.body {
            writeln!(f, "{}", instr)?;
        }
        write!(f, "}}")
    }
}

impl fmt::Display for IrModule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for func in &self.functions {
            writeln!(f, "{}", func)?;
            writeln!(f)?;
        }
        writeln!(f, "fn __main__() {{")?;
        for instr in &self.main {
            writeln!(f, "{}", instr)?;
        }
        write!(f, "}}")
    }
}
