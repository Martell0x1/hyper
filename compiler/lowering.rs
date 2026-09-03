use crate::ast::*;
use crate::driver;
use crate::error::{self, ErrorKind};
use super::ir::{BlockId, IrFunction, IrInstr, IrModule, IrOp, ValueId};
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
    methods: HashSet<String>,
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
    /// local variable → opened file handle (for file method lowering)
    var_files: HashSet<String>,
    /// local variable → memory-mapped file handle (for mmap method lowering)
    var_mmaps: HashSet<String>,
    /// function name → struct type it returns, so `let p = make()` keeps methods
    fn_struct_returns: HashMap<String, String>,
    /// Line of the statement being lowered, used for diagnostics.
    current_line: u32,
    /// Diagnostics collected while lowering, reported before codegen runs.
    errors: Vec<String>,
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
            var_files: HashSet::new(),
            var_mmaps: HashSet::new(),
            fn_struct_returns: HashMap::new(),
            current_line: 0,
            errors: Vec::new(),
        }
    }

    fn error(&mut self, message: impl Into<String>) {
        self.errors.push(error::format_error(
            ErrorKind::Syntax,
            self.current_line,
            &message.into(),
        ));
    }

    /// Value stand-in so lowering can continue and report further errors.
    fn error_value(&mut self) -> ValueId {
        let dest = self.fresh_value();
        self.emit(IrInstr::ConstNone { dest });
        dest
    }

    fn stmt_line(stmt: &Stmt) -> Option<u32> {
        match stmt {
            Stmt::Let { line, .. }
            | Stmt::Print { line, .. }
            | Stmt::Expr { line, .. }
            | Stmt::Return { line, .. }
            | Stmt::WithMmap { line, .. }
            | Stmt::With { line, .. }
            | Stmt::Import { line, .. }
            | Stmt::ImportFrom { line, .. } => Some(*line),
            _ => None,
        }
    }

    fn line_arg(&mut self) -> ValueId {
        let dest = self.fresh_value();
        self.emit(IrInstr::ConstI64 {
            dest,
            value: self.current_line as i64,
        });
        dest
    }

    fn is_open_call(&self, expr: &Expr) -> bool {
        matches!(
            expr,
            Expr::Call { callee, .. }
                if matches!(callee.as_ref(), Expr::Variable { name, .. } if name == "open")
        )
    }

    fn note_file_binding(&mut self, name: &str, initializer: &Expr) {
        if self.is_open_call(initializer) {
            self.var_files.insert(name.to_string());
            return;
        }
        if let Expr::Variable { name: src, .. } = initializer {
            if self.var_files.contains(src) {
                self.var_files.insert(name.to_string());
            }
        }
    }

    fn lower_open(&mut self, args: &[CallArg]) -> ValueId {
        if args.len() > 2 {
            self.error(format!(
                "open expects 1 or 2 argument(s) but got {}",
                args.len()
            ));
            return self.error_value();
        }
        let path = match args.first() {
            Some(CallArg::Positional(e) | CallArg::Named { value: e, .. }) => self.lower_expr(e),
            None => {
                self.error("open expects a file path");
                return self.error_value();
            }
        };
        let mode = match args.get(1) {
            Some(CallArg::Positional(e) | CallArg::Named { value: e, .. }) => self.lower_expr(e),
            None => {
                let dest = self.fresh_value();
                self.emit(IrInstr::ConstStr {
                    dest,
                    value: "r".to_string(),
                });
                dest
            }
        };
        let line = self.line_arg();
        let dest = self.fresh_value();
        self.emit(IrInstr::Call {
            dest,
            func: "hyper_rt_file_open".to_string(),
            args: vec![path, mode, line],
        });
        dest
    }

    fn const_zero(&mut self) -> ValueId {
        let dest = self.fresh_value();
        self.emit(IrInstr::ConstI64 { dest, value: 0 });
        dest
    }

    fn lower_json_call(&mut self, method: &str, args: &[Expr]) -> ValueId {
        let line = self.line_arg();
        let dest = self.fresh_value();
        match method {
            "loads" => {
                if args.len() != 1 {
                    self.error("json.loads expects 1 argument");
                    return self.error_value();
                }
                let text = self.lower_expr(&args[0]);
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_json_loads".to_string(),
                    args: vec![text, line],
                });
            }
            "dumps" => {
                if args.is_empty() || args.len() > 2 {
                    self.error("json.dumps expects 1 or 2 argument(s)");
                    return self.error_value();
                }
                let value = self.lower_expr(&args[0]);
                let indent = if args.len() == 2 {
                    self.lower_expr(&args[1])
                } else {
                    self.const_zero()
                };
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_json_dumps".to_string(),
                    args: vec![value, indent, line],
                });
            }
            "load" => {
                if args.len() != 1 {
                    self.error("json.load expects 1 argument");
                    return self.error_value();
                }
                let handle = self.lower_expr(&args[0]);
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_json_load".to_string(),
                    args: vec![handle, line],
                });
            }
            "dump" => {
                if args.len() < 2 || args.len() > 3 {
                    self.error("json.dump expects 2 or 3 argument(s)");
                    return self.error_value();
                }
                let value = self.lower_expr(&args[0]);
                let handle = self.lower_expr(&args[1]);
                let indent = if args.len() == 3 {
                    self.lower_expr(&args[2])
                } else {
                    self.const_zero()
                };
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_json_dump".to_string(),
                    args: vec![value, handle, indent, line],
                });
            }
            other => {
                self.error(format!("json has no method '{other}'"));
                return self.error_value();
            }
        }
        dest
    }

    fn lower_file_method(&mut self, object: &str, method: &str, args: &[Expr]) -> ValueId {
        let handle = self.fresh_value();
        self.emit(IrInstr::Load {
            dest: handle,
            name: object.to_string(),
        });
        let line = self.line_arg();
        let dest = self.fresh_value();
        match method {
            "read" => {
                let func = if args.is_empty() {
                    "hyper_rt_file_read_all"
                } else if args.len() == 1 {
                    "hyper_rt_file_read_n"
                } else {
                    self.error("read expects 0 or 1 argument(s)");
                    return self.error_value();
                };
                let mut call_args = vec![handle];
                if !args.is_empty() {
                    call_args.push(self.lower_expr(&args[0]));
                }
                call_args.push(line);
                self.emit(IrInstr::Call {
                    dest,
                    func: func.to_string(),
                    args: call_args,
                });
            }
            "readline" => {
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_file_readline".to_string(),
                    args: vec![handle, line],
                });
            }
            "readlines" => {
                if !args.is_empty() {
                    self.error("readlines expects 0 arguments");
                    return self.error_value();
                }
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_file_readlines".to_string(),
                    args: vec![handle, line],
                });
            }
            "write" => {
                if args.len() != 1 {
                    self.error("write expects 1 argument");
                    return self.error_value();
                }
                let text = self.lower_expr(&args[0]);
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_file_write".to_string(),
                    args: vec![handle, text, line],
                });
            }
            "writelines" => {
                if args.len() != 1 {
                    self.error("writelines expects 1 argument");
                    return self.error_value();
                }
                let list = self.lower_expr(&args[0]);
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_file_writelines".to_string(),
                    args: vec![handle, list, line],
                });
            }
            "seek" => {
                let (offset, whence) = match args.len() {
                    1 => (self.lower_expr(&args[0]), {
                        let w = self.fresh_value();
                        self.emit(IrInstr::ConstI64 { dest: w, value: 0 });
                        w
                    }),
                    2 => (self.lower_expr(&args[0]), self.lower_expr(&args[1])),
                    _ => {
                        self.error("seek expects 1 or 2 argument(s)");
                        return self.error_value();
                    }
                };
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_file_seek".to_string(),
                    args: vec![handle, offset, whence, line],
                });
            }
            "tell" | "size" => {
                if !args.is_empty() {
                    self.error(format!("{method} expects 0 arguments"));
                    return self.error_value();
                }
                let func = if method == "tell" {
                    "hyper_rt_file_tell"
                } else {
                    "hyper_rt_file_size"
                };
                self.emit(IrInstr::Call {
                    dest,
                    func: func.to_string(),
                    args: vec![handle, line],
                });
            }
            "flush" | "close" => {
                if !args.is_empty() {
                    self.error(format!("{method} expects 0 arguments"));
                    return self.error_value();
                }
                let func = if method == "flush" {
                    "hyper_rt_file_flush"
                } else {
                    "hyper_rt_file_close"
                };
                let none = self.fresh_value();
                self.emit(IrInstr::ConstNone { dest: none });
                self.emit(IrInstr::Call {
                    dest: none,
                    func: func.to_string(),
                    args: vec![handle, line],
                });
                return none;
            }
            "closed" => {
                if !args.is_empty() {
                    self.error("closed expects 0 arguments");
                    return self.error_value();
                }
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_file_is_closed".to_string(),
                    args: vec![handle],
                });
            }
            "path" | "mode" => {
                if !args.is_empty() {
                    self.error(format!("{method} expects 0 arguments"));
                    return self.error_value();
                }
                let func = if method == "path" {
                    "hyper_rt_file_path"
                } else {
                    "hyper_rt_file_mode"
                };
                self.emit(IrInstr::Call {
                    dest,
                    func: func.to_string(),
                    args: vec![handle, line],
                });
            }
            other => {
                self.error(format!("file has no method '{other}'"));
                return self.error_value();
            }
        }
        dest
    }

    fn lower_clock(&mut self, args: &[CallArg]) -> ValueId {
        if !args.is_empty() {
            self.error(format!(
                "clock expects 0 arguments but got {}",
                args.len()
            ));
            return self.error_value();
        }
        let dest = self.fresh_value();
        self.emit(IrInstr::Call {
            dest,
            func: "hyper_rt_clock".to_string(),
            args: vec![],
        });
        dest
    }

    fn lower_input(&mut self, args: &[CallArg]) -> ValueId {
        if args.len() > 1 {
            self.error(format!(
                "input expects 0 or 1 argument(s) but got {}",
                args.len()
            ));
            return self.error_value();
        }
        let prompt = match args.first() {
            Some(CallArg::Positional(e) | CallArg::Named { value: e, .. }) => self.lower_expr(e),
            None => {
                let dest = self.fresh_value();
                self.emit(IrInstr::ConstNone { dest });
                dest
            }
        };
        let line = self.line_arg();
        let dest = self.fresh_value();
        self.emit(IrInstr::Call {
            dest,
            func: "hyper_rt_input".to_string(),
            args: vec![prompt, line],
        });
        dest
    }

    fn lower_mmap_method(&mut self, object: &str, method: &str, args: &[Expr]) -> ValueId {
        let handle = self.fresh_value();
        self.emit(IrInstr::Load {
            dest: handle,
            name: object.to_string(),
        });
        let line = self.line_arg();
        let dest = self.fresh_value();
        match method {
            "read_chunk" => {
                if args.len() != 2 {
                    self.error("read_chunk expects 2 arguments (offset, size)");
                    return self.error_value();
                }
                let offset = self.lower_expr(&args[0]);
                let size = self.lower_expr(&args[1]);
                self.emit(IrInstr::Call {
                    dest,
                    func: "hyper_rt_mmap_read_chunk".to_string(),
                    args: vec![handle, offset, size, line],
                });
            }
            other => {
                self.error(format!("mapped file has no method '{other}'"));
                return self.error_value();
            }
        }
        dest
    }

    fn field_error(&self, object: &str, field: &str) -> String {
        match self.var_structs.get(object) {
            Some(stype) => {
                let name = self
                    .structs
                    .get(stype)
                    .map(|l| l.ir_name.clone())
                    .unwrap_or_else(|| stype.clone());
                format!("struct '{}' has no field '{}'", name, field)
            }
            None => format!(
                "cannot resolve field '{}' on '{}'; annotate '{}' with its struct type so the compiler can find the field",
                field, object, object
            ),
        }
    }

    fn lower_collection_method(
        &mut self,
        object: &str,
        method: &str,
        args: &[Expr],
    ) -> Option<ValueId> {
        let (func, wanted) = match method {
            "len" => ("hyper_rt_coll_len", 0),
            "append" => ("hyper_rt_coll_append", 1),
            "keys" => ("hyper_rt_coll_keys", 0),
            _ => return None,
        };
        if args.len() != wanted {
            self.error(format!(
                "{method} expects {wanted} argument(s) but got {}",
                args.len()
            ));
            return Some(self.error_value());
        }
        let obj = self.fresh_value();
        self.emit(IrInstr::Load {
            dest: obj,
            name: object.to_string(),
        });
        let mut call_args = vec![obj];
        for arg in args {
            call_args.push(self.lower_expr(arg));
        }
        call_args.push(self.line_arg());
        let dest = self.fresh_value();
        self.emit(IrInstr::Call {
            dest,
            func: func.to_string(),
            args: call_args,
        });
        Some(dest)
    }

    fn lower_string_method(
        &mut self,
        object: &str,
        method: &str,
        args: &[Expr],
    ) -> Option<ValueId> {
        let (func, min_args, max_args) = match method {
            "upper" => ("hyper_rt_str_upper", 0, 0),
            "lower" => ("hyper_rt_str_lower", 0, 0),
            "strip" => ("hyper_rt_str_strip", 0, 0),
            "lstrip" => ("hyper_rt_str_lstrip", 0, 0),
            "rstrip" => ("hyper_rt_str_rstrip", 0, 0),
            "startswith" => ("hyper_rt_str_startswith", 1, 1),
            "endswith" => ("hyper_rt_str_endswith", 1, 1),
            "split" => ("hyper_rt_str_split", 0, 1),
            "replace" => ("hyper_rt_str_replace", 2, 2),
            _ => return None,
        };
        if args.len() < min_args || args.len() > max_args {
            let expected = if min_args == max_args {
                min_args.to_string()
            } else {
                format!("{min_args}-{max_args}")
            };
            self.error(format!(
                "{method} expects {expected} argument(s) but got {}",
                args.len()
            ));
            return Some(self.error_value());
        }
        let obj = self.fresh_value();
        self.emit(IrInstr::Load {
            dest: obj,
            name: object.to_string(),
        });
        let mut call_args = vec![obj];
        if method == "split" && args.is_empty() {
            let none = self.fresh_value();
            self.emit(IrInstr::ConstNone { dest: none });
            call_args.push(none);
        } else {
            for arg in args {
                call_args.push(self.lower_expr(arg));
            }
        }
        call_args.push(self.line_arg());
        let dest = self.fresh_value();
        self.emit(IrInstr::Call {
            dest,
            func: func.to_string(),
            args: call_args,
        });
        Some(dest)
    }

    fn method_error(&self, object: &str, method: &str) -> String {
        if self.var_files.contains(object) {
            return format!("file has no method '{method}'");
        }
        if self.var_mmaps.contains(object) {
            return format!("mapped file has no method '{method}'");
        }
        match self.var_structs.get(object) {
            Some(stype) => {
                let name = self
                    .structs
                    .get(stype)
                    .map(|l| l.ir_name.clone())
                    .unwrap_or_else(|| stype.clone());
                format!("struct '{}' has no method '{}'", name, method)
            }
            None => format!(
                "cannot resolve method '{}' on '{}'; annotate '{}' with its struct type so the compiler can find the method",
                method, object, object
            ),
        }
    }

    fn mangle_method(ir_name: &str, method: &str) -> String {
        format!("{}__{}", ir_name, method)
    }

    /// Struct layout key for a type name, preferring the module-qualified one.
    fn struct_key(&self, name: &str, module: Option<&str>) -> Option<String> {
        if let Some(m) = module {
            let mangled = module::mangle_module_fn(m, name);
            if self.structs.contains_key(&mangled) {
                return Some(mangled);
            }
        }
        if self.structs.contains_key(name) {
            return Some(name.to_string());
        }
        None
    }

    fn visit_returns(stmt: &Stmt, visit: &mut impl FnMut(&Expr)) {
        match stmt {
            Stmt::Return { value, .. } => visit(value),
            Stmt::Block(stmts) => {
                for s in stmts {
                    Self::visit_returns(s, visit);
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                Self::visit_returns(then_branch, visit);
                if let Some(other) = else_branch {
                    Self::visit_returns(other, visit);
                }
            }
            Stmt::While { body, .. }
            | Stmt::For { body, .. }
            | Stmt::With { body, .. }
            | Stmt::WithMmap { body, .. } => Self::visit_returns(body, visit),
            _ => {}
        }
    }

    /// Struct type a function hands back, from its annotation or its returns.
    fn struct_return_of(&self, decl: &FunctionDecl, module: Option<&str>) -> Option<String> {
        if let Some(ret) = &decl.return_type {
            if let Some(key) = self.struct_key(ret, module) {
                return Some(key);
            }
        }
        let mut found: Option<String> = None;
        Self::visit_returns(&decl.body, &mut |expr| {
            if found.is_some() {
                return;
            }
            found = match expr {
                Expr::Call { callee, .. } => match callee.as_ref() {
                    Expr::Variable { name, .. } => self.struct_key(name, module),
                    _ => None,
                },
                Expr::Variable { name, .. } => self.var_structs.get(name).cloned(),
                _ => None,
            };
        });
        found
    }

    /// Bind struct-typed parameters so method calls inside the body resolve.
    fn bind_param_structs(&mut self, params: &[Param], module: Option<&str>) {
        for param in params {
            if let Some(ty) = &param.type_ann {
                if let Some(key) = self.struct_key(ty, module) {
                    self.var_structs.insert(param.name.clone(), key);
                }
            }
        }
    }

    fn note_struct_returns(&mut self, decl: &FunctionDecl, ir_name: &str, module: Option<&str>) {
        if let Some(stype) = self.struct_return_of(decl, module) {
            self.fn_struct_returns.insert(ir_name.to_string(), stype);
        }
    }

    fn note_struct_binding(&mut self, name: &str, initializer: &Expr) {
        match initializer {
            Expr::Call { callee, .. } => {
                if let Expr::Variable { name: sn, .. } = callee.as_ref() {
                    if self.structs.contains_key(sn) {
                        self.var_structs.insert(name.to_string(), sn.clone());
                        return;
                    }
                    let target = self.resolve_call_name(sn);
                    if let Some(stype) = self.fn_struct_returns.get(&target).cloned() {
                        self.var_structs.insert(name.to_string(), stype);
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
                    } else if let Some(stype) = self.fn_struct_returns.get(&key).cloned() {
                        self.var_structs.insert(name.to_string(), stype);
                    }
                    return;
                }
                if let Some(stype) = self.var_structs.get(object).cloned() {
                    let ir_name = self
                        .structs
                        .get(&stype)
                        .map(|l| l.ir_name.clone())
                        .unwrap_or(stype);
                    let key = Self::mangle_method(&ir_name, method);
                    if let Some(ret) = self.fn_struct_returns.get(&key).cloned() {
                        self.var_structs.insert(name.to_string(), ret);
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
                methods: HashSet::new(),
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
            methods: methods
                .iter()
                .map(|m| m.function.name.clone())
                .collect(),
            has_init,
            ir_name: ir_name.to_string(),
        };
        self.structs.insert(name.to_string(), layout.clone());
        if name != ir_name {
            self.structs.insert(ir_name.to_string(), layout);
        }

        for method in methods {
            let mangled = Self::mangle_method(ir_name, &method.function.name);
            self.note_struct_returns(&method.function, &mangled, None);
            let saved = std::mem::take(&mut self.current);
            let saved_next_value = self.next_value;
            let saved_next_block = self.next_block;
            let saved_var_structs = self.var_structs.clone();
            let saved_var_files = self.var_files.clone();
            let saved_var_mmaps = self.var_mmaps.clone();
            self.next_value = 0;
            self.next_block = 0;
            self.var_structs
                .insert("self".to_string(), name.to_string());
            self.bind_param_structs(&method.function.params, None);

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
            self.var_files = saved_var_files;
            self.var_mmaps = saved_var_mmaps;
        }
    }

    fn bind_import_name(&mut self, module: &str, item: &ImportName) {
        let bind = item.alias.as_ref().unwrap_or(&item.name).clone();
        if module == "json" {
            if module::builtin_module_members(module)
                .is_some_and(|members| members.contains(&item.name.as_str()))
            {
                self.call_aliases
                    .insert(bind, format!("hyper_rt_json_{}", item.name));
                return;
            }
        }
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

        if module::builtin_module_members(module_name).is_some() {
            return;
        }

        let stmts = match self.load_state.load_stmts(module_name) {
            Ok((_path, stmts)) => stmts,
            Err(msg) => {
                error::runtime(line, msg);
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
                    methods: methods
                        .iter()
                        .map(|m| m.function.name.clone())
                        .collect(),
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
                self.note_struct_returns(&method.function, &mangled, Some(module_name));
                let saved = std::mem::take(&mut self.current);
                let saved_next_value = self.next_value;
                let saved_next_block = self.next_block;
                let saved_var_structs = self.var_structs.clone();
                let saved_var_files = self.var_files.clone();
            let saved_var_mmaps = self.var_mmaps.clone();
                self.next_value = 0;
                self.next_block = 0;
                self.var_structs
                    .insert("self".to_string(), (*name).to_string());
                self.bind_param_structs(&method.function.params, Some(module_name));

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
                self.var_files = saved_var_files;
            self.var_mmaps = saved_var_mmaps;
            }
        }

        let saved_current = std::mem::take(&mut self.current);
        for stmt in &stmts {
            match stmt {
                Stmt::Function(decl) => {
                    let ir_name = module::mangle_module_fn(module_name, &decl.name);
                    self.note_struct_returns(decl, &ir_name, Some(module_name));
                    let saved_body_current = std::mem::take(&mut self.current);
                    let saved_next_value = self.next_value;
                    let saved_next_block = self.next_block;
                    let saved_var_structs = self.var_structs.clone();
                    let saved_var_files = self.var_files.clone();
            let saved_var_mmaps = self.var_mmaps.clone();
                    self.next_value = 0;
                    self.next_block = 0;
                    self.bind_param_structs(&decl.params, Some(module_name));

                    self.lower_stmt(&decl.body);

                    let body = std::mem::take(&mut self.current);
                    self.functions.push(IrFunction {
                        name: ir_name,
                        params: decl.params.iter().map(|p| p.name.clone()).collect(),
                        body,
                    });

                    self.current = saved_body_current;
                    self.next_value = saved_next_value;
                    self.next_block = saved_next_block;
                    self.var_structs = saved_var_structs;
                    self.var_files = saved_var_files;
            self.var_mmaps = saved_var_mmaps;
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
            BinOp::FloorDiv => IrOp::FloorDiv,
            BinOp::Rem => IrOp::Rem,
            BinOp::Pow => IrOp::Pow,
            BinOp::Eq => IrOp::Eq,
            BinOp::Ne => IrOp::Ne,
            BinOp::Lt => IrOp::Lt,
            BinOp::Le => IrOp::Le,
            BinOp::Gt => IrOp::Gt,
            BinOp::Ge => IrOp::Ge,
            BinOp::And | BinOp::Or => {
                unreachable!("and/or are lowered with short-circuit CFG")
            }
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
                match op {
                    BinOp::And => {
                        // Short-circuit: if left is falsy, yield left; else yield right.
                        let l = self.lower_expr(left);
                        let then_b = self.fresh_block();
                        let else_b = self.fresh_block();
                        let merge_b = self.fresh_block();
                        let result_name = format!("__and_{}", then_b);

                        self.emit(IrInstr::Branch {
                            cond: l,
                            then_block: then_b,
                            else_block: else_b,
                        });

                        self.emit(IrInstr::Label { block: then_b });
                        let r = self.lower_expr(right);
                        self.emit(IrInstr::Store {
                            name: result_name.clone(),
                            value: r,
                        });
                        self.emit(IrInstr::Jump { target: merge_b });

                        self.emit(IrInstr::Label { block: else_b });
                        self.emit(IrInstr::Store {
                            name: result_name.clone(),
                            value: l,
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
                    BinOp::Or => {
                        // Short-circuit: if left is truthy, yield left; else yield right.
                        let l = self.lower_expr(left);
                        let then_b = self.fresh_block();
                        let else_b = self.fresh_block();
                        let merge_b = self.fresh_block();
                        let result_name = format!("__or_{}", then_b);

                        self.emit(IrInstr::Branch {
                            cond: l,
                            then_block: then_b,
                            else_block: else_b,
                        });

                        self.emit(IrInstr::Label { block: then_b });
                        self.emit(IrInstr::Store {
                            name: result_name.clone(),
                            value: l,
                        });
                        self.emit(IrInstr::Jump { target: merge_b });

                        self.emit(IrInstr::Label { block: else_b });
                        let r = self.lower_expr(right);
                        self.emit(IrInstr::Store {
                            name: result_name.clone(),
                            value: r,
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
                    other => {
                        let l = self.lower_expr(left);
                        let r = self.lower_expr(right);
                        if matches!(other, BinOp::Div | BinOp::FloorDiv | BinOp::Rem) {
                            self.emit(IrInstr::GuardDivisor {
                                value: r,
                                line: self.current_line,
                            });
                        }
                        let dest = self.fresh_value();
                        self.emit(IrInstr::Binary {
                            dest,
                            op: Self::bin_op(other),
                            left: l,
                            right: r,
                        });
                        dest
                    }
                }
            }
            Expr::Assign { name, value } => {
                self.note_struct_binding(name, value);
                self.note_file_binding(name, value);
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
                let message = self.field_error(object, field);
                self.error(message);
                self.error_value()
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
                let message = self.field_error(object, field);
                self.error(message);
                v
            }
            Expr::Call { callee, args } => {
                if let Expr::Variable { name, .. } = callee.as_ref() {
                    if name == "open" {
                        return self.lower_open(args);
                    }
                    if name == "input" {
                        return self.lower_input(args);
                    }
                    if name == "clock" {
                        return self.lower_clock(args);
                    }
                    if self.structs.contains_key(name) {
                        return self.lower_struct_ctor(name, args);
                    }
                }
                let func_name = match callee.as_ref() {
                    Expr::Variable { name, .. } => self.resolve_call_name(name),
                    other => {
                        let _ = self.lower_expr(other);
                        self.error(
                            "only calls to named functions are supported by the compiler",
                        );
                        return self.error_value();
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
                    if mod_name == "json" {
                        return self.lower_json_call(method, args);
                    }
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
                if self.var_files.contains(object) {
                    return self.lower_file_method(object, method, args);
                }
                if self.var_mmaps.contains(object) {
                    return self.lower_mmap_method(object, method, args);
                }
                if let Some(stype) = self.var_structs.get(object).cloned() {
                    let layout = self.structs.get(&stype);
                    let ir_name = layout
                        .map(|l| l.ir_name.clone())
                        .unwrap_or_else(|| stype.clone());
                    let known = layout.map(|l| l.methods.contains(method)).unwrap_or(false);
                    if !known {
                        let message = self.method_error(object, method);
                        self.error(message);
                        return self.error_value();
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
                        func: Self::mangle_method(&ir_name, method),
                        args: arg_ids,
                    });
                    return dest;
                }
                if let Some(dest) = self.lower_collection_method(object, method, args) {
                    return dest;
                }
                if let Some(dest) = self.lower_string_method(object, method, args) {
                    return dest;
                }
                let message = self.method_error(object, method);
                self.error(message);
                self.error_value()
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
        if let Some(line) = Self::stmt_line(stmt) {
            self.current_line = line;
        }
        match stmt {
            Stmt::Let {
                name, initializer, ..
            } => {
                self.note_struct_binding(name, initializer);
                self.note_file_binding(name, initializer);
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
                self.note_struct_returns(decl, &decl.name, None);
                let saved = std::mem::take(&mut self.current);
                let saved_next_value = self.next_value;
                let saved_next_block = self.next_block;
                let saved_var_structs = self.var_structs.clone();
                let saved_var_files = self.var_files.clone();
            let saved_var_mmaps = self.var_mmaps.clone();
                self.next_value = 0;
                self.next_block = 0;
                self.bind_param_structs(&decl.params, None);

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
                self.var_structs = saved_var_structs;
                self.var_files = saved_var_files;
            self.var_mmaps = saved_var_mmaps;
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
                path,
                var,
                body,
                ..
            } => {
                let path_val = self.lower_expr(path);
                let line = self.line_arg();
                let handle = self.fresh_value();
                self.emit(IrInstr::Call {
                    dest: handle,
                    func: "hyper_rt_mmap_open".to_string(),
                    args: vec![path_val, line],
                });
                self.var_mmaps.insert(var.clone());
                self.emit(IrInstr::Store {
                    name: var.clone(),
                    value: handle,
                });
                self.lower_stmt(body);
                let loaded = self.fresh_value();
                self.emit(IrInstr::Load {
                    dest: loaded,
                    name: var.clone(),
                });
                let line = self.line_arg();
                let none = self.fresh_value();
                self.emit(IrInstr::ConstNone { dest: none });
                self.emit(IrInstr::Call {
                    dest: none,
                    func: "hyper_rt_mmap_close".to_string(),
                    args: vec![loaded, line],
                });
            }
            Stmt::With {
                value,
                var,
                body,
                ..
            } => {
                let resource = self.lower_expr(value);
                self.var_files.insert(var.clone());
                self.emit(IrInstr::Store {
                    name: var.clone(),
                    value: resource,
                });
                self.lower_stmt(body);
                let handle = self.fresh_value();
                self.emit(IrInstr::Load {
                    dest: handle,
                    name: var.clone(),
                });
                let line = self.line_arg();
                let none = self.fresh_value();
                self.emit(IrInstr::ConstNone { dest: none });
                self.emit(IrInstr::Call {
                    dest: none,
                    func: "hyper_rt_file_close".to_string(),
                    args: vec![handle, line],
                });
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

pub fn lower(stmts: &[Stmt], entry_path: &Path) -> Result<IrModule, Vec<String>> {
    let mut lowerer = Lowerer::new(entry_path);
    for stmt in stmts {
        lowerer.lower_stmt(stmt);
    }
    if !lowerer.errors.is_empty() {
        return Err(lowerer.errors);
    }
    let mut main = lowerer.module_inits;
    main.extend(lowerer.current);
    Ok(IrModule {
        functions: lowerer.functions,
        main,
    })
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
        Err(()) => process::exit(65),
    };

    if let Err(errors) = semantic::typecheck(&stmts) {
        for e in errors {
            error::report_formatted(&e);
        }
        process::exit(65);
    }

    let module = match lower(&stmts, Path::new(entry_path)) {
        Ok(m) => m,
        Err(errors) => {
            for e in errors {
                error::report_formatted(&e);
            }
            process::exit(65);
        }
    };

    let result = match mode {
        CompileMode::Jit => super::codegen::jit_execute(&module),
        CompileMode::EmitIr => {
            super::codegen::dump_ir(&module);
            Ok(())
        }
        CompileMode::EmitObj { path } => super::codegen::emit_object(&module, &path),
        CompileMode::EmitExe { path } => super::codegen::emit_exe(&module, &path),
    };

    if let Err(msg) = result {
        error::report_formatted(&msg);
        process::exit(70);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lower_source(source: &str) -> Result<IrModule, Vec<String>> {
        let stmts = driver::parse_program(source).expect("source should parse");
        lower(&stmts, Path::new("test.hyp"))
    }

    fn errors_of(source: &str) -> Vec<String> {
        lower_source(source).expect_err("lowering should fail")
    }

    #[test]
    fn struct_program_lowers() {
        let module = lower_source(
            "struct Point:\n\
             \x20   let mut x: i32\n\
             \n\
             \x20   fn bump(ref self, dx: i32):\n\
             \x20       self.x = self.x + dx\n\
             \n\
             let p = Point(x: 1)\n\
             p.bump(2)\n\
             print(p.x)\n",
        );
        let module = module.expect("valid struct program should lower");
        assert!(module.functions.iter().any(|f| f.name == "Point__bump"));
    }

    #[test]
    fn struct_returned_by_function_keeps_its_methods() {
        let module = lower_source(
            "struct Point:\n\
             \x20   let mut x: i32\n\
             \n\
             \x20   fn bump(ref self, dx: i32):\n\
             \x20       self.x = self.x + dx\n\
             \n\
             fn origin():\n\
             \x20   return Point(x: 0)\n\
             \n\
             let p = origin()\n\
             p.bump(2)\n",
        );
        module.expect("call returning a struct should lower");
    }

    #[test]
    fn struct_typed_parameter_keeps_its_methods() {
        let module = lower_source(
            "struct Point:\n\
             \x20   let mut x: i32\n\
             \n\
             \x20   fn bump(ref self, dx: i32):\n\
             \x20       self.x = self.x + dx\n\
             \n\
             fn shift(p: Point):\n\
             \x20   p.bump(1)\n\
             \n\
             let p = Point(x: 1)\n\
             shift(p)\n",
        );
        module.expect("struct-typed parameter should lower");
    }

    fn guards_divisor(source: &str) -> bool {
        let module = lower_source(source).expect("source should lower");
        module
            .main
            .iter()
            .chain(module.functions.iter().flat_map(|f| f.body.iter()))
            .any(|i| matches!(i, IrInstr::GuardDivisor { .. }))
    }

    #[test]
    fn division_guards_the_divisor() {
        assert!(guards_divisor("let d = 0\nprint(10 / d)\n"));
        assert!(guards_divisor("let d = 0\nprint(10 % d)\n"));
    }

    #[test]
    fn multiplication_needs_no_guard() {
        assert!(!guards_divisor("print(6 * 7)\n"));
    }

    #[test]
    fn collection_methods_lower_to_runtime_calls() {
        let module = lower_source(
            "let mut items = [1, 2]\n\
             print(items.len())\n\
             items.append(3)\n\
             let scores = {\"a\": 1}\n\
             print(scores.keys())\n",
        )
        .expect("collection methods should lower");
        let calls: Vec<&str> = module
            .main
            .iter()
            .filter_map(|i| match i {
                IrInstr::Call { func, .. } => Some(func.as_str()),
                _ => None,
            })
            .collect();
        assert!(calls.contains(&"hyper_rt_coll_len"));
        assert!(calls.contains(&"hyper_rt_coll_append"));
        assert!(calls.contains(&"hyper_rt_coll_keys"));
    }

    #[test]
    fn string_methods_lower_to_runtime_calls() {
        let module = lower_source(
            "let mut text = \"  hi  \"\n\
             print(text.strip())\n\
             print(text.upper())\n\
             print(text.startswith(\" \"))\n\
             print(text.split())\n\
             print(text.replace(\"hi\", \"yo\"))\n",
        )
        .expect("string methods should lower");
        let calls: Vec<&str> = module
            .main
            .iter()
            .filter_map(|i| match i {
                IrInstr::Call { func, .. } => Some(func.as_str()),
                _ => None,
            })
            .collect();
        assert!(calls.contains(&"hyper_rt_str_strip"));
        assert!(calls.contains(&"hyper_rt_str_upper"));
        assert!(calls.contains(&"hyper_rt_str_startswith"));
        assert!(calls.contains(&"hyper_rt_str_split"));
        assert!(calls.contains(&"hyper_rt_str_replace"));
    }

    #[test]
    fn collection_method_arity_is_checked() {
        let errors = errors_of("let items = [1]\nitems.append()\n");
        assert!(
            errors.iter().any(|e| e.contains("append expects 1 argument")),
            "{errors:?}"
        );
    }

    #[test]
    fn unknown_field_is_reported_with_line() {
        let errors = errors_of(
            "struct Point:\n\
             \x20   let x: i32\n\
             \n\
             let p = Point(x: 1)\n\
             print(p.zzz)\n",
        );
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].starts_with("SyntaxError: line 5:")
                && errors[0].contains("struct 'Point' has no field 'zzz'"),
            "unexpected message: {}",
            errors[0]
        );
    }

    #[test]
    fn unknown_method_is_reported() {
        let errors = errors_of(
            "struct Point:\n\
             \x20   let x: i32\n\
             \n\
             let p = Point(x: 1)\n\
             p.jump(1)\n",
        );
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("struct 'Point' has no method 'jump'"),
            "unexpected message: {}",
            errors[0]
        );
    }

    #[test]
    fn field_access_on_unknown_type_is_reported() {
        let errors = errors_of("let q = 5\nprint(q.field)\n");
        assert_eq!(errors.len(), 1);
        assert!(
            errors[0].contains("cannot resolve field 'field' on 'q'"),
            "unexpected message: {}",
            errors[0]
        );
    }

    #[test]
    fn all_errors_are_collected() {
        let errors = errors_of(
            "struct Point:\n\
             \x20   let x: i32\n\
             \n\
             let p = Point(x: 1)\n\
             print(p.a)\n\
             print(p.b)\n",
        );
        assert_eq!(errors.len(), 2);
    }

    #[test]
    fn clock_lowers_for_compile() {
        let module = lower_source("print(clock())\n").expect("clock should lower");
        assert!(module.main.iter().any(|i| {
            matches!(i, IrInstr::Call { func, .. } if func == "hyper_rt_clock")
        }));
    }

    #[test]
    fn input_lowers_for_compile() {
        let module = lower_source("let x = input()\nprint(x)\n").expect("input should lower");
        assert!(module.main.iter().any(|i| {
            matches!(i, IrInstr::Call { func, .. } if func == "hyper_rt_input")
        }));
    }

    #[test]
    fn with_open_mmap_lowers_for_compile() {
        let module = lower_source(
            "with open_mmap(\"t.txt\") as m:\n\
             \x20   print(m.read_chunk(0, 4))\n",
        )
        .expect("with open_mmap should lower");
        let calls: Vec<_> = module
            .main
            .iter()
            .filter_map(|i| {
                if let IrInstr::Call { func, .. } = i {
                    Some(func.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(calls.contains(&"hyper_rt_mmap_open"));
        assert!(calls.contains(&"hyper_rt_mmap_read_chunk"));
        assert!(calls.contains(&"hyper_rt_mmap_close"));
    }

    #[test]
    fn with_open_lowers_for_compile() {
        let module = lower_source(
            "with open(\"t.txt\", \"w\") as f:\n\
             \x20   f.write(\"x\")\n",
        )
        .expect("with open should lower");
        let calls: Vec<_> = module
            .main
            .iter()
            .filter_map(|i| {
                if let IrInstr::Call { func, .. } = i {
                    Some(func.as_str())
                } else {
                    None
                }
            })
            .collect();
        assert!(calls.contains(&"hyper_rt_file_open"));
        assert!(calls.contains(&"hyper_rt_file_write"));
        assert!(calls.contains(&"hyper_rt_file_close"));
    }

    #[test]
    fn json_import_lowers_for_compile() {
        let module = lower_source(
            "import json\n\
             print(json.dumps({\"a\": 1}))\n",
        )
        .expect("json import should lower");
        assert!(module.main.iter().any(|i| {
            matches!(i, IrInstr::Call { func, .. } if func == "hyper_rt_json_dumps")
        }));
    }
}
