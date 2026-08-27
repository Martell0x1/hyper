use crate::ast::*;
use crate::driver;
use crate::ir::{BlockId, IrFunction, IrInstr, IrModule, IrOp, ValueId};
use crate::module::{self, ModuleLoadState};
use crate::semantic;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process;

#[derive(Clone)]
struct StructLayout {
    fields: HashMap<String, u32>,
    field_order: Vec<String>,
    /// Field name → type name key in `structs` (primitives do not match a layout).
    field_types: HashMap<String, String>,
    has_init: bool,
    /// IR / mangled base name for methods (`Point` or `shapes__Point`).
    ir_name: String,
}

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
    /// struct type layouts
    structs: HashMap<String, StructLayout>,
    /// local variable → struct type name (for field/method lowering)
    var_structs: HashMap<String, String>,
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
            structs: HashMap::new(),
            var_structs: HashMap::new(),
        }
    }

    fn mangle_method(ir_name: &str, method: &str) -> String {
        format!("{}__{}", ir_name, method)
    }

    fn note_struct_binding(&mut self, name: &str, initializer: &Expr) {
        match initializer {
            Expr::Call { callee, .. } => {
                if let Expr::Variable { name: sn, .. } = callee.as_ref() {
                    if self.structs.contains_key(sn) {
                        self.var_structs.insert(name.to_string(), sn.clone());
                    }
                }
            }
            Expr::CallMethod {
                object, method, ..
            } => {
                if let Some(mod_name) = self.module_aliases.get(object) {
                    let key = module::mangle_module_fn(mod_name, method);
                    if self.structs.contains_key(&key) {
                        self.var_structs.insert(name.to_string(), key);
                    }
                }
            }
            Expr::GetField { object, field } => {
                if let Some(stype) = self.var_structs.get(object).cloned() {
                    if let Some(ft) = self
                        .structs
                        .get(&stype)
                        .and_then(|l| l.field_types.get(field).cloned())
                    {
                        if self.structs.contains_key(&ft) {
                            self.var_structs.insert(name.to_string(), ft);
                        }
                    }
                }
            }
            Expr::Variable { name: src, .. } => {
                if let Some(st) = self.var_structs.get(src).cloned() {
                    self.var_structs.insert(name.to_string(), st);
                }
            }
            _ => {}
        }
    }

    fn lower_struct_ctor(&mut self, struct_key: &str, args: &[CallArg]) -> ValueId {
        let layout = self
            .structs
            .get(struct_key)
            .cloned()
            .unwrap_or(StructLayout {
                fields: HashMap::new(),
                field_order: Vec::new(),
                field_types: HashMap::new(),
                has_init: false,
                ir_name: struct_key.to_string(),
            });

        let dest = self.fresh_value();
        self.emit(IrInstr::MakeStruct {
            dest,
            nfields: layout.field_order.len() as u32,
        });

        let mut named: HashMap<String, ValueId> = HashMap::new();
        let mut positional: Vec<ValueId> = Vec::new();
        for arg in args {
            match arg {
                CallArg::Named { name, value } => {
                    named.insert(name.clone(), self.lower_expr(value));
                }
                CallArg::Positional(e) => positional.push(self.lower_expr(e)),
            }
        }

        for (i, fname) in layout.field_order.iter().enumerate() {
            if let Some(&vid) = named.get(fname) {
                self.emit(IrInstr::StructSet {
                    object: dest,
                    field: i as u32,
                    value: vid,
                });
            }
        }

        if layout.has_init {
            let mut init_args = vec![dest];
            if positional.is_empty() {
                for fname in &layout.field_order {
                    if let Some(&vid) = named.get(fname) {
                        init_args.push(vid);
                    }
                }
            } else {
                init_args.extend(positional);
            }
            let ret = self.fresh_value();
            self.emit(IrInstr::Call {
                dest: ret,
                func: Self::mangle_method(&layout.ir_name, "__init__"),
                args: init_args,
            });
        }

        dest
    }

    /// Register a struct layout and lower its methods into IR functions.
    fn lower_struct_decl(
        &mut self,
        name: &str,
        fields: &[StructField],
        methods: &[MethodDecl],
        ir_name: &str,
    ) {
        let mut field_map = HashMap::new();
        let mut field_order = Vec::new();
        let mut field_types = HashMap::new();
        for (i, f) in fields.iter().enumerate() {
            field_map.insert(f.name.clone(), i as u32);
            field_order.push(f.name.clone());
            field_types.insert(f.name.clone(), f.type_name.clone());
        }
        let has_init = methods.iter().any(|m| m.function.name == "__init__");
        let layout = StructLayout {
            fields: field_map,
            field_order,
            field_types,
            has_init,
            ir_name: ir_name.to_string(),
        };
        self.structs.insert(name.to_string(), layout.clone());
        if name != ir_name {
            self.structs.insert(ir_name.to_string(), layout);
        }

        for method in methods {
            let mangled = Self::mangle_method(ir_name, &method.function.name);
            let saved = std::mem::take(&mut self.current);
            let saved_next_value = self.next_value;
            let saved_next_block = self.next_block;
            let saved_var_structs = self.var_structs.clone();
            self.next_value = 0;
            self.next_block = 0;
            self.var_structs
                .insert("self".to_string(), name.to_string());

            self.lower_stmt(&method.function.body);

            let body = std::mem::take(&mut self.current);
            self.functions.push(IrFunction {
                name: mangled,
                params: method
                    .function
                    .params
                    .iter()
                    .map(|p| p.name.clone())
                    .collect(),
                body,
            });

            self.current = saved;
            self.next_value = saved_next_value;
            self.next_block = saved_next_block;
            self.var_structs = saved_var_structs;
        }
    }

    fn bind_import_name(&mut self, module: &str, item: &ImportName) {
        let bind = item.alias.as_ref().unwrap_or(&item.name).clone();
        let mangled = module::mangle_module_fn(module, &item.name);
        if let Some(layout) = self.structs.get(&mangled).cloned() {
            self.structs.insert(bind, layout);
        } else {
            self.call_aliases.insert(bind, mangled);
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
        let saved_structs = self.structs.clone();
        for stmt in &stmts {
            if let Stmt::Function(decl) = stmt {
                self.call_aliases.insert(
                    decl.name.clone(),
                    module::mangle_module_fn(module_name, &decl.name),
                );
            }
        }

        // Register module structs under short names while lowering the module body.
        let mut module_struct_shorts: Vec<String> = Vec::new();
        let mut module_structs: Vec<(&str, &[StructField], &[MethodDecl], String)> = Vec::new();
        for stmt in &stmts {
            if let Stmt::Struct {
                name,
                fields,
                methods,
                ..
            } = stmt
            {
                let ir_name = module::mangle_module_fn(module_name, name);
                let mut field_map = HashMap::new();
                let mut field_order = Vec::new();
                let mut field_types = HashMap::new();
                for (i, f) in fields.iter().enumerate() {
                    field_map.insert(f.name.clone(), i as u32);
                    field_order.push(f.name.clone());
                    field_types.insert(f.name.clone(), f.type_name.clone());
                }
                let has_init = methods.iter().any(|m| m.function.name == "__init__");
                let layout = StructLayout {
                    fields: field_map,
                    field_order,
                    field_types,
                    has_init,
                    ir_name: ir_name.clone(),
                };
                self.structs.insert(name.clone(), layout.clone());
                self.structs.insert(ir_name.clone(), layout);
                module_struct_shorts.push(name.clone());
                module_structs.push((name, fields, methods, ir_name));
            }
        }
        // Rewrite struct-valued field types to mangled keys so they survive
        // after short names are dropped from `structs`.
        for short in &module_struct_shorts {
            if let Some(mut layout) = self.structs.get(short).cloned() {
                for ty in layout.field_types.values_mut() {
                    let mangled = module::mangle_module_fn(module_name, ty);
                    if self.structs.contains_key(&mangled) {
                        *ty = mangled;
                    }
                }
                let ir = layout.ir_name.clone();
                self.structs.insert(short.clone(), layout.clone());
                self.structs.insert(ir, layout);
            }
        }
        for (name, _fields, methods, ir_name) in &module_structs {
            for method in *methods {
                let mangled = Self::mangle_method(ir_name, &method.function.name);
                let saved = std::mem::take(&mut self.current);
                let saved_next_value = self.next_value;
                let saved_next_block = self.next_block;
                let saved_var_structs = self.var_structs.clone();
                self.next_value = 0;
                self.next_block = 0;
                self.var_structs
                    .insert("self".to_string(), (*name).to_string());

                self.lower_stmt(&method.function.body);

                let body = std::mem::take(&mut self.current);
                self.functions.push(IrFunction {
                    name: mangled,
                    params: method
                        .function
                        .params
                        .iter()
                        .map(|p| p.name.clone())
                        .collect(),
                    body,
                });

                self.current = saved;
                self.next_value = saved_next_value;
                self.next_block = saved_next_block;
                self.var_structs = saved_var_structs;
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
                        self.bind_import_name(module, item);
                    }
                }
                Stmt::Struct { .. } => {
                    // Already lowered above.
                }
                _ => {
                    // Skip control-flow / print at module top-level on compile path for now.
                }
            }
        }
        self.module_inits.append(&mut self.current);
        self.current = saved_current;
        self.call_aliases = saved_aliases;

        // Drop short names so they do not leak into the importer; keep mangled keys.
        for short in module_struct_shorts {
            self.structs.remove(&short);
        }
        // Restore any importer structs that shared a short name.
        for (k, v) in saved_structs {
            if !self.structs.contains_key(&k) {
                self.structs.insert(k, v);
            }
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
                self.note_struct_binding(name, value);
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
                if let Some(stype) = self.var_structs.get(object).cloned() {
                    if let Some(layout) = self.structs.get(&stype) {
                        if let Some(&idx) = layout.fields.get(field) {
                            let obj = self.fresh_value();
                            self.emit(IrInstr::Load {
                                dest: obj,
                                name: object.clone(),
                            });
                            let dest = self.fresh_value();
                            self.emit(IrInstr::StructGet {
                                dest,
                                object: obj,
                                field: idx,
                            });
                            return dest;
                        }
                    }
                }
                // Soft fallback for unknown field access.
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
                let v = self.lower_expr(value);
                if let Some(stype) = self.var_structs.get(object).cloned() {
                    if let Some(layout) = self.structs.get(&stype) {
                        if let Some(&idx) = layout.fields.get(field) {
                            let obj = self.fresh_value();
                            self.emit(IrInstr::Load {
                                dest: obj,
                                name: object.clone(),
                            });
                            self.emit(IrInstr::StructSet {
                                object: obj,
                                field: idx,
                                value: v,
                            });
                            return v;
                        }
                    }
                }
                let obj = self.fresh_value();
                self.emit(IrInstr::Load {
                    dest: obj,
                    name: object.clone(),
                });
                let dest = self.fresh_value();
                self.emit(IrInstr::Call {
                    dest,
                    func: format!("__set_field__{}.{}", object, field),
                    args: vec![obj, v],
                });
                dest
            }
            Expr::Call { callee, args } => {
                if let Expr::Variable { name, .. } = callee.as_ref() {
                    if self.structs.contains_key(name) {
                        return self.lower_struct_ctor(name, args);
                    }
                }
                let func_name = match callee.as_ref() {
                    Expr::Variable { name, .. } => self.resolve_call_name(name),
                    other => {
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
                    let struct_key = module::mangle_module_fn(&mod_name, method);
                    if self.structs.contains_key(&struct_key) {
                        let call_args: Vec<CallArg> = args
                            .iter()
                            .cloned()
                            .map(CallArg::Positional)
                            .collect();
                        return self.lower_struct_ctor(&struct_key, &call_args);
                    }
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
                if let Some(stype) = self.var_structs.get(object).cloned() {
                    let ir_name = self
                        .structs
                        .get(&stype)
                        .map(|l| l.ir_name.clone())
                        .unwrap_or_else(|| stype.clone());
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
                        func: Self::mangle_method(&ir_name, method),
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
                self.note_struct_binding(name, initializer);
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
            Stmt::Struct {
                name,
                fields,
                methods,
                ..
            } => {
                self.lower_struct_decl(name, fields, methods, name);
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
                    self.bind_import_name(module, item);
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
