use crate::ast::*;
use crate::driver;
use crate::ir::{BlockId, IrFunction, IrInstr, IrModule, IrOp, ValueId};
use crate::semantic;
use std::process;

struct Lowerer {
    next_value: ValueId,
    next_block: BlockId,
    functions: Vec<IrFunction>,
    current: Vec<IrInstr>,
}

impl Lowerer {
    fn new() -> Self {
        Lowerer {
            next_value: 0,
            next_block: 0,
            functions: Vec::new(),
            current: Vec::new(),
        }
    }

    fn fresh_value(&mut self) -> ValueId {
        let id = self.next_value;
        self.next_value += 1;
        id
    }

    fn fresh_block(&mut self) -> BlockId {
        let id = self.next_block;
        self.next_block += 1;
        id
    }

    fn emit(&mut self, instr: IrInstr) {
        self.current.push(instr);
    }

    fn bin_op(op: &BinOp) -> IrOp {
        match op {
            BinOp::Add => IrOp::Add,
            BinOp::Sub => IrOp::Sub,
            BinOp::Mul => IrOp::Mul,
            BinOp::Div => IrOp::Div,
            BinOp::Rem => IrOp::Rem,
            BinOp::Pow => IrOp::Pow,
            BinOp::Eq => IrOp::Eq,
            BinOp::Ne => IrOp::Ne,
            BinOp::Lt => IrOp::Lt,
            BinOp::Le => IrOp::Le,
            BinOp::Gt => IrOp::Gt,
            BinOp::Ge => IrOp::Ge,
            BinOp::And => IrOp::And,
            BinOp::Or => IrOp::Or,
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> ValueId {
        match expr {
            Expr::Literal(lit) => {
                let dest = self.fresh_value();
                match lit {
                    Literal::None => self.emit(IrInstr::ConstNone { dest }),
                    Literal::Bool(b) => self.emit(IrInstr::ConstBool {
                        dest,
                        value: *b,
                    }),
                    Literal::String(s) => self.emit(IrInstr::ConstStr {
                        dest,
                        value: s.clone(),
                    }),
                    Literal::Number(n) => {
                        if n.contains('.') || n.contains('e') || n.contains('E') {
                            let value = n.parse::<f64>().unwrap_or(0.0);
                            self.emit(IrInstr::ConstF64 { dest, value });
                        } else if let Ok(value) = n.parse::<i64>() {
                            self.emit(IrInstr::ConstI64 { dest, value });
                        } else {
                            let value = n.parse::<f64>().unwrap_or(0.0);
                            self.emit(IrInstr::ConstF64 { dest, value });
                        }
                    }
                }
                dest
            }
            Expr::Variable { name, .. } => {
                let dest = self.fresh_value();
                self.emit(IrInstr::Load {
                    dest,
                    name: name.clone(),
                });
                dest
            }
            Expr::Group(inner) => self.lower_expr(inner),
            Expr::Unary { op, right } => {
                let src = self.lower_expr(right);
                let dest = self.fresh_value();
                let ir_op = match op {
                    UnaryOp::Neg => IrOp::Neg,
                    UnaryOp::Not => IrOp::Not,
                };
                self.emit(IrInstr::Unary {
                    dest,
                    op: ir_op,
                    src,
                });
                dest
            }
            Expr::Binary { op, left, right } => {
                let l = self.lower_expr(left);
                let r = self.lower_expr(right);
                let dest = self.fresh_value();
                self.emit(IrInstr::Binary {
                    dest,
                    op: Self::bin_op(op),
                    left: l,
                    right: r,
                });
                dest
            }
            Expr::Assign { name, value } => {
                let v = self.lower_expr(value);
                self.emit(IrInstr::Store {
                    name: name.clone(),
                    value: v,
                });
                v
            }
            Expr::GetField { object, field } => {
                // Soft: load object then call a synthetic getter.
                let obj = self.fresh_value();
                self.emit(IrInstr::Load {
                    dest: obj,
                    name: object.clone(),
                });
                let dest = self.fresh_value();
                self.emit(IrInstr::Call {
                    dest,
                    func: format!("__get_field__{}.{}", object, field),
                    args: vec![obj],
                });
                dest
            }
            Expr::SetField {
                object,
                field,
                value,
            } => {
                let obj = self.fresh_value();
                self.emit(IrInstr::Load {
                    dest: obj,
                    name: object.clone(),
                });
                let v = self.lower_expr(value);
                let dest = self.fresh_value();
                self.emit(IrInstr::Call {
                    dest,
                    func: format!("__set_field__{}.{}", object, field),
                    args: vec![obj, v],
                });
                dest
            }
            Expr::Call { callee, args } => {
                let func_name = match callee.as_ref() {
                    Expr::Variable { name, .. } => name.clone(),
                    other => {
                        // Evaluate callee then call through a temp name.
                        let _ = self.lower_expr(other);
                        "__indirect__".to_string()
                    }
                };
                let mut arg_ids = Vec::new();
                for arg in args {
                    match arg {
                        CallArg::Positional(e) => arg_ids.push(self.lower_expr(e)),
                        CallArg::Named { value, .. } => arg_ids.push(self.lower_expr(value)),
                    }
                }
                let dest = self.fresh_value();
                self.emit(IrInstr::Call {
                    dest,
                    func: func_name,
                    args: arg_ids,
                });
                dest
            }
            Expr::CallMethod {
                object,
                method,
                args,
            } => {
                let obj = self.fresh_value();
                self.emit(IrInstr::Load {
                    dest: obj,
                    name: object.clone(),
                });
                let mut arg_ids = vec![obj];
                for a in args {
                    arg_ids.push(self.lower_expr(a));
                }
                let dest = self.fresh_value();
                self.emit(IrInstr::Call {
                    dest,
                    func: format!("{}.{}", object, method),
                    args: arg_ids,
                });
                dest
            }
            Expr::List(items) => {
                let mut arg_ids = Vec::new();
                for item in items {
                    arg_ids.push(self.lower_expr(item));
                }
                let dest = self.fresh_value();
                self.emit(IrInstr::Call {
                    dest,
                    func: "__list__".to_string(),
                    args: arg_ids,
                });
                dest
            }
            Expr::Dict(entries) => {
                let mut arg_ids = Vec::new();
                for (k, v) in entries {
                    arg_ids.push(self.lower_expr(k));
                    arg_ids.push(self.lower_expr(v));
                }
                let dest = self.fresh_value();
                self.emit(IrInstr::Call {
                    dest,
                    func: "__dict__".to_string(),
                    args: arg_ids,
                });
                dest
            }
            Expr::FString { parts, .. } => {
                let mut arg_ids = Vec::new();
                for part in parts {
                    match part {
                        FStringPart::Literal(s) => {
                            let dest = self.fresh_value();
                            self.emit(IrInstr::ConstStr {
                                dest,
                                value: s.clone(),
                            });
                            arg_ids.push(dest);
                        }
                        FStringPart::Expr(e) => arg_ids.push(self.lower_expr(e)),
                    }
                }
                let dest = self.fresh_value();
                self.emit(IrInstr::Call {
                    dest,
                    func: "__fstring__".to_string(),
                    args: arg_ids,
                });
                dest
            }
            Expr::Ternary {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(condition);
                let then_b = self.fresh_block();
                let else_b = self.fresh_block();
                let merge_b = self.fresh_block();
                let result_name = format!("__ternary_{}", then_b);

                self.emit(IrInstr::Branch {
                    cond,
                    then_block: then_b,
                    else_block: else_b,
                });

                self.emit(IrInstr::Label { block: then_b });
                let tv = self.lower_expr(then_branch);
                self.emit(IrInstr::Store {
                    name: result_name.clone(),
                    value: tv,
                });
                self.emit(IrInstr::Jump { target: merge_b });

                self.emit(IrInstr::Label { block: else_b });
                let ev = self.lower_expr(else_branch);
                self.emit(IrInstr::Store {
                    name: result_name.clone(),
                    value: ev,
                });
                self.emit(IrInstr::Jump { target: merge_b });

                self.emit(IrInstr::Label { block: merge_b });
                let dest = self.fresh_value();
                self.emit(IrInstr::Load {
                    dest,
                    name: result_name,
                });
                dest
            }
        }
    }

    fn lower_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name, initializer, ..
            } => {
                let v = self.lower_expr(initializer);
                self.emit(IrInstr::Store {
                    name: name.clone(),
                    value: v,
                });
            }
            Stmt::Print { values, .. } => {
                let mut args = Vec::new();
                for v in values {
                    args.push(self.lower_expr(v));
                }
                self.emit(IrInstr::Print { args });
            }
            Stmt::Expr { expr, .. } => {
                let _ = self.lower_expr(expr);
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.lower_stmt(s);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let cond = self.lower_expr(condition);
                let then_b = self.fresh_block();
                let else_b = self.fresh_block();
                let merge_b = self.fresh_block();

                self.emit(IrInstr::Branch {
                    cond,
                    then_block: then_b,
                    else_block: else_b,
                });

                self.emit(IrInstr::Label { block: then_b });
                self.lower_stmt(then_branch);
                self.emit(IrInstr::Jump { target: merge_b });

                self.emit(IrInstr::Label { block: else_b });
                if let Some(else_s) = else_branch {
                    self.lower_stmt(else_s);
                }
                self.emit(IrInstr::Jump { target: merge_b });

                self.emit(IrInstr::Label { block: merge_b });
            }
            Stmt::While {
                condition, body, ..
            } => {
                let header = self.fresh_block();
                let body_b = self.fresh_block();
                let exit_b = self.fresh_block();

                self.emit(IrInstr::Jump { target: header });
                self.emit(IrInstr::Label { block: header });
                let cond = self.lower_expr(condition);
                self.emit(IrInstr::Branch {
                    cond,
                    then_block: body_b,
                    else_block: exit_b,
                });

                self.emit(IrInstr::Label { block: body_b });
                self.lower_stmt(body);
                self.emit(IrInstr::Jump { target: header });

                self.emit(IrInstr::Label { block: exit_b });
            }
            Stmt::For {
                kind: _,
                var,
                start,
                end,
                body,
                ..
            } => {
                // Always sequential label/branch loop so @parallel/@vectorize still
                // compile via sequential codegen. ForKind is kept in AST only.
                let start_v = self.lower_expr(start);
                let end_v = self.lower_expr(end);

                self.emit(IrInstr::Store {
                    name: var.clone(),
                    value: start_v,
                });
                let header = self.fresh_block();
                let body_b = self.fresh_block();
                let exit_b = self.fresh_block();

                self.emit(IrInstr::Jump { target: header });
                self.emit(IrInstr::Label { block: header });

                let i_val = self.fresh_value();
                self.emit(IrInstr::Load {
                    dest: i_val,
                    name: var.clone(),
                });
                let cmp = self.fresh_value();
                self.emit(IrInstr::Binary {
                    dest: cmp,
                    op: IrOp::Lt,
                    left: i_val,
                    right: end_v,
                });
                self.emit(IrInstr::Branch {
                    cond: cmp,
                    then_block: body_b,
                    else_block: exit_b,
                });

                self.emit(IrInstr::Label { block: body_b });
                self.lower_stmt(body);

                let i2 = self.fresh_value();
                self.emit(IrInstr::Load {
                    dest: i2,
                    name: var.clone(),
                });
                let one = self.fresh_value();
                self.emit(IrInstr::ConstI64 {
                    dest: one,
                    value: 1,
                });
                let next = self.fresh_value();
                self.emit(IrInstr::Binary {
                    dest: next,
                    op: IrOp::Add,
                    left: i2,
                    right: one,
                });
                self.emit(IrInstr::Store {
                    name: var.clone(),
                    value: next,
                });
                self.emit(IrInstr::Jump { target: header });

                self.emit(IrInstr::Label { block: exit_b });
            }
            Stmt::Function(decl) => {
                let saved = std::mem::take(&mut self.current);
                let saved_next_value = self.next_value;
                let saved_next_block = self.next_block;
                self.next_value = 0;
                self.next_block = 0;

                // Params are referenced by name via Load in the body.
                self.lower_stmt(&decl.body);

                let body = std::mem::take(&mut self.current);
                self.functions.push(IrFunction {
                    name: decl.name.clone(),
                    params: decl.params.iter().map(|p| p.name.clone()).collect(),
                    body,
                });

                self.current = saved;
                self.next_value = saved_next_value;
                self.next_block = saved_next_block;
            }
            Stmt::Return { value, .. } => {
                // Bare `return` may be Literal::None from parser.
                let is_none = matches!(value, Expr::Literal(Literal::None));
                if is_none {
                    self.emit(IrInstr::Return { value: None });
                } else {
                    let v = self.lower_expr(value);
                    self.emit(IrInstr::Return { value: Some(v) });
                }
            }
            Stmt::Struct { name, .. } => {
                // Soft: store type name marker for now.
                let dest = self.fresh_value();
                self.emit(IrInstr::ConstStr {
                    dest,
                    value: format!("struct:{}", name),
                });
                self.emit(IrInstr::Store {
                    name: name.clone(),
                    value: dest,
                });
            }
            Stmt::Trait { name, .. } => {
                let dest = self.fresh_value();
                self.emit(IrInstr::ConstStr {
                    dest,
                    value: format!("trait:{}", name),
                });
                self.emit(IrInstr::Store {
                    name: name.clone(),
                    value: dest,
                });
            }
            Stmt::WithMmap {
                path, var, body, ..
            } => {
                let path_v = self.lower_expr(path);
                let mmap = self.fresh_value();
                self.emit(IrInstr::Call {
                    dest: mmap,
                    func: "__mmap_open__".to_string(),
                    args: vec![path_v],
                });
                self.emit(IrInstr::Store {
                    name: var.clone(),
                    value: mmap,
                });
                self.lower_stmt(body);
            }
        }
    }
}

pub fn lower(stmts: &[Stmt]) -> IrModule {
    let mut lowerer = Lowerer::new();
    for stmt in stmts {
        lowerer.lower_stmt(stmt);
    }
    IrModule {
        functions: lowerer.functions,
        main: lowerer.current,
    }
}

pub enum CompileMode {
    Jit,
    EmitIr,
    EmitObj { path: String },
}

pub fn run_compile(file_contents: String, mode: CompileMode) {
    let stmts = match driver::parse_program(&file_contents) {
        Ok(s) => s,
        Err(()) => {
            eprintln!("Syntax error.");
            process::exit(65);
        }
    };

    if let Err(errors) = semantic::typecheck(&stmts) {
        for e in errors {
            eprintln!("{}", e);
        }
        process::exit(65);
    }

    let module = lower(&stmts);
    match mode {
        CompileMode::Jit => match crate::codegen::jit_execute(&module) {
            Ok(()) => {}
            Err(msg) => {
                eprintln!("codegen: {}", msg);
                println!("{}", module);
                process::exit(70);
            }
        },
        CompileMode::EmitIr => {
            crate::codegen::dump_ir(&module);
        }
        CompileMode::EmitObj { path } => match crate::codegen::emit_object(&module, &path) {
            Ok(()) => {}
            Err(msg) => {
                eprintln!("codegen: {}", msg);
                process::exit(70);
            }
        },
    }
}
