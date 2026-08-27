use crate::ast::*;
use crate::driver;
use crate::ir::{BlockId, IrFunction, IrInstr, IrModule, IrOp, ValueId};
use crate::module::{self, ModuleLoadState};
use crate::semantic;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process;

struct Lowerer {
    next_value: ValueId,
    next_block: BlockId,
    functions: Vec<IrFunction>,
    current: Vec<IrInstr>,
    /// Instructions to run once for imported module top-level lets.
    module_inits: Vec<IrInstr>,
    /// `import math as m` → m maps to real module name `math`
    module_aliases: HashMap<String, String>,
    /// `from math import add` / same-module calls → local name → mangled IR name
    call_aliases: HashMap<String, String>,
    lowered_modules: HashSet<String>,
    load_state: ModuleLoadState,
}

impl Lowerer {
    fn new(entry_path: &Path) -> Self {
        Lowerer {
            next_value: 0,
            next_block: 0,
            functions: Vec::new(),
            current: Vec::new(),
            module_inits: Vec::new(),
            module_aliases: HashMap::new(),
            call_aliases: HashMap::new(),
            lowered_modules: HashSet::new(),
            load_state: ModuleLoadState::new(entry_path),
        }
    }

    fn resolve_call_name(&self, name: &str) -> String {
        self.call_aliases
            .get(name)
            .cloned()
            .unwrap_or_else(|| name.to_string())
    }

    fn ensure_module(&mut self, module_name: &str, line: u32) {
        if !self.lowered_modules.insert(module_name.to_string()) {
            return;
        }

        let stmts = match self.load_state.load_stmts(module_name) {
            Ok((_path, stmts)) => stmts,
            Err(msg) => {
                eprintln!("[line {}] Error: {}.", line, msg);
                process::exit(70);
            }
        };

        // Nested imports first.
        for stmt in &stmts {
            match stmt {
                Stmt::Import {
                    module, line, ..
                } => self.ensure_module(module, *line),
                Stmt::ImportFrom {
                    module, line, ..
                } => self.ensure_module(module, *line),
                _ => {}
            }
        }

        let saved_aliases = self.call_aliases.clone();
        for stmt in &stmts {
            if let Stmt::Function(decl) = stmt {
                self.call_aliases.insert(
                    decl.name.clone(),
                    module::mangle_module_fn(module_name, &decl.name),
                );
            }
        }

        let saved_current = std::mem::take(&mut self.current);
        for stmt in &stmts {
            match stmt {
                Stmt::Function(decl) => {
                    let saved_body_current = std::mem::take(&mut self.current);
                    let saved_next_value = self.next_value;
                    let saved_next_block = self.next_block;
                    self.next_value = 0;
                    self.next_block = 0;

                    self.lower_stmt(&decl.body);

                    let body = std::mem::take(&mut self.current);
                    self.functions.push(IrFunction {
                        name: module::mangle_module_fn(module_name, &decl.name),
                        params: decl.params.iter().map(|p| p.name.clone()).collect(),
                        body,
                    });

                    self.current = saved_body_current;
                    self.next_value = saved_next_value;
                    self.next_block = saved_next_block;
                }
                Stmt::Let {
                    name, initializer, ..
                } => {
                    let v = self.lower_expr(initializer);
                    self.current.push(IrInstr::Store {
                        name: module::mangle_module_fn(module_name, name),
                        value: v,
                    });
                }
                Stmt::Import {
                    module,
                    alias,
                    line,
                } => {
                    self.ensure_module(module, *line);
                    let bind = alias.as_ref().unwrap_or(module).clone();
                    self.module_aliases.insert(bind, module.clone());
                }
                Stmt::ImportFrom {
                    module,
                    names,
                    line,
                } => {
                    self.ensure_module(module, *line);
                    for item in names {
                        let mangled = module::mangle_module_fn(module, &item.name);
                        let bind = item.alias.as_ref().unwrap_or(&item.name).clone();
                        self.call_aliases.insert(bind, mangled);
                    }
                }
                _ => {
                    // Skip control-flow / print at module top-level on compile path for now.
                }
            }
        }
        self.module_inits.append(&mut self.current);
        self.current = saved_current;
        self.call_aliases = saved_aliases;
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
                if let Some(mod_name) = self.module_aliases.get(object).cloned() {
                    let dest = self.fresh_value();
                    self.emit(IrInstr::Load {
                        dest,
                        name: module::mangle_module_fn(&mod_name, field),
                    });
                    return dest;
                }
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
                    Expr::Variable { name, .. } => self.resolve_call_name(name),
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
                if let Some(mod_name) = self.module_aliases.get(object).cloned() {
                    let mut arg_ids = Vec::new();
                    for a in args {
                        arg_ids.push(self.lower_expr(a));
                    }
                    let dest = self.fresh_value();
                    self.emit(IrInstr::Call {
                        dest,
                        func: module::mangle_module_fn(&mod_name, method),
                        args: arg_ids,
                    });
                    return dest;
                }
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
                let mut item_ids = Vec::new();
                for item in items {
                    item_ids.push(self.lower_expr(item));
                }
                let dest = self.fresh_value();
                self.emit(IrInstr::MakeList {
                    dest,
                    items: item_ids,
                });
                dest
            }
            Expr::Dict(entries) => {
                let mut entry_ids = Vec::new();
                for (k, v) in entries {
                    let key = self.lower_expr(k);
                    let val = self.lower_expr(v);
                    entry_ids.push((key, val));
                }
                let dest = self.fresh_value();
                self.emit(IrInstr::MakeDict {
                    dest,
                    entries: entry_ids,
                });
                dest
            }
            Expr::Index { object, index } => {
                let obj = self.lower_expr(object);
                let idx = self.lower_expr(index);
                let dest = self.fresh_value();
                self.emit(IrInstr::IndexGet {
                    dest,
                    object: obj,
                    index: idx,
                });
                dest
            }
            Expr::IndexSet {
                object,
                index,
                value,
            } => {
                let obj = self.lower_expr(object);
                let idx = self.lower_expr(index);
                let val = self.lower_expr(value);
                self.emit(IrInstr::IndexSet {
                    object: obj,
                    index: idx,
                    value: val,
                });
                val
            }
            Expr::FString { parts, .. } => {
                let empty = self.fresh_value();
                self.emit(IrInstr::ConstStr {
                    dest: empty,
                    value: String::new(),
                });
                let mut acc = empty;
                for part in parts {
                    let piece = match part {
                        FStringPart::Literal(s) => {
                            let dest = self.fresh_value();
                            self.emit(IrInstr::ConstStr {
                                dest,
                                value: s.clone(),
                            });
                            dest
                        }
                        FStringPart::Expr(e) => {
                            let src = self.lower_expr(e);
                            let dest = self.fresh_value();
                            self.emit(IrInstr::ValueToStr { dest, src });
                            dest
                        }
                    };
                    let dest = self.fresh_value();
                    self.emit(IrInstr::StrConcat {
                        dest,
                        left: acc,
                        right: piece,
                    });
                    acc = dest;
                }
                acc
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
                iter,
                body,
                ..
            } => {
                match iter {
                    ForIter::Range { start, end } => {
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
                    ForIter::Iterable(iterable) => {
                        let list = self.lower_expr(iterable);
                        let len = self.fresh_value();
                        self.emit(IrInstr::ListLen { dest: len, list });

                        let idx_name = format!("__for_i_{}", var);
                        let zero = self.fresh_value();
                        self.emit(IrInstr::ConstI64 {
                            dest: zero,
                            value: 0,
                        });
                        self.emit(IrInstr::Store {
                            name: idx_name.clone(),
                            value: zero,
                        });

                        let header = self.fresh_block();
                        let body_b = self.fresh_block();
                        let exit_b = self.fresh_block();

                        self.emit(IrInstr::Jump { target: header });
                        self.emit(IrInstr::Label { block: header });

                        let i_val = self.fresh_value();
                        self.emit(IrInstr::Load {
                            dest: i_val,
                            name: idx_name.clone(),
                        });
                        let cmp = self.fresh_value();
                        self.emit(IrInstr::Binary {
                            dest: cmp,
                            op: IrOp::Lt,
                            left: i_val,
                            right: len,
                        });
                        self.emit(IrInstr::Branch {
                            cond: cmp,
                            then_block: body_b,
                            else_block: exit_b,
                        });

                        self.emit(IrInstr::Label { block: body_b });
                        let elem = self.fresh_value();
                        self.emit(IrInstr::IndexGet {
                            dest: elem,
                            object: list,
                            index: i_val,
                        });
                        self.emit(IrInstr::Store {
                            name: var.clone(),
                            value: elem,
                        });
                        self.lower_stmt(body);

                        let i2 = self.fresh_value();
                        self.emit(IrInstr::Load {
                            dest: i2,
                            name: idx_name.clone(),
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
                            name: idx_name,
                            value: next,
                        });
                        self.emit(IrInstr::Jump { target: header });

                        self.emit(IrInstr::Label { block: exit_b });
                    }
                }
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
            Stmt::Import {
                line,
                module,
                alias,
            } => {
                self.ensure_module(module, *line);
                let bind = alias.as_ref().unwrap_or(module).clone();
                self.module_aliases.insert(bind, module.clone());
            }
            Stmt::ImportFrom {
                line,
                module,
                names,
            } => {
                self.ensure_module(module, *line);
                for item in names {
                    let mangled = module::mangle_module_fn(module, &item.name);
                    let bind = item.alias.as_ref().unwrap_or(&item.name).clone();
                    self.call_aliases.insert(bind, mangled);
                }
            }
        }
    }
}

pub fn lower(stmts: &[Stmt], entry_path: &Path) -> IrModule {
    let mut lowerer = Lowerer::new(entry_path);
    for stmt in stmts {
        lowerer.lower_stmt(stmt);
    }
    let mut main = lowerer.module_inits;
    main.extend(lowerer.current);
    IrModule {
        functions: lowerer.functions,
        main,
    }
}

pub enum CompileMode {
    Jit,
    EmitIr,
    EmitObj { path: String },
    EmitExe { path: String },
}

pub fn run_compile(file_contents: String, entry_path: &str, mode: CompileMode) {
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

    let module = lower(&stmts, Path::new(entry_path));
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
        CompileMode::EmitExe { path } => match crate::codegen::emit_exe(&module, &path) {
            Ok(()) => {}
            Err(msg) => {
                eprintln!("codegen: {}", msg);
                process::exit(70);
            }
        },
    }
}
