use indexmap::IndexMap;
use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::{cell::RefCell, io};
use std::rc::Rc;
use crate::ast::*;
use crate::environment::{Environment, HyperValue};
use crate::error;
use crate::module;

thread_local! {
    static MODULE_RUNTIME: RefCell<Option<ModuleRuntime>> = const { RefCell::new(None) };
}

struct ModuleRuntime {
    base_dir: PathBuf,
    cache: HashMap<String, HyperValue>,
    loading: HashSet<String>,
}

impl ModuleRuntime {
    fn new(entry_file: &Path) -> Self {
        let base_dir = entry_file
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        ModuleRuntime {
            base_dir,
            cache: HashMap::new(),
            loading: HashSet::new(),
        }
    }
}

/// Modules implemented in Rust rather than in Hyper source.
fn builtin_module(module_name: &str) -> Option<HyperValue> {
    let exports = module::builtin_module_members(module_name)?
        .iter()
        .map(|member| {
            (
                (*member).to_string(),
                HyperValue::NativeFunction(format!("{}.{}", module_name, member)),
            )
        })
        .collect();
    Some(HyperValue::Module {
        name: module_name.to_string(),
        exports,
    })
}

fn load_module(module_name: &str, line: u32) -> HyperValue {
    let (path, already) = MODULE_RUNTIME.with(|cell| {
        let mut rt = cell.borrow_mut();
        let rt = rt
            .as_mut()
            .expect("module runtime not initialized; pass entry file path to run/compile");
        if let Some(cached) = rt.cache.get(module_name) {
            return (None, Some(cached.clone()));
        }
        if rt.loading.contains(module_name) {
            error::runtime(
                line,
                format!("circular import involving module '{}'", module_name),
            );
        }
        let path = match module::resolve_module_path(&rt.base_dir, module_name) {
            Ok(p) => p,
            // A builtin module only steps in when no local file shadows it.
            Err(msg) => match builtin_module(module_name) {
                Some(builtin) => return (None, Some(builtin)),
                None => {
                    error::runtime(line, msg);
                }
            },
        };
        rt.loading.insert(module_name.to_string());
        (Some(path), None)
    });

    if let Some(cached) = already {
        return cached;
    }
    let path = path.expect("module path");

    let source = match module::read_module_source(&path) {
        Ok(s) => s,
        Err(msg) => {
            error::runtime(line, msg);
        }
    };
    let stmts = match crate::driver::parse_program(&source) {
        Ok(s) => s,
        Err(()) => {
            error::syntax(line, format!("syntax error in module '{}'", module_name));
        }
    };

    let mod_env = Rc::new(RefCell::new(Environment::new()));
    for stmt in &stmts {
        execute(stmt, Rc::clone(&mod_env));
    }
    let exports = mod_env.borrow().snapshot_bindings();
    let module_val = HyperValue::Module {
        name: module_name.to_string(),
        exports,
    };

    MODULE_RUNTIME.with(|cell| {
        let mut rt = cell.borrow_mut();
        let rt = rt.as_mut().unwrap();
        rt.loading.remove(module_name);
        rt.cache
            .insert(module_name.to_string(), module_val.clone());
    });

    module_val
}

pub enum ExecResult {
    Ok,
    Return(HyperValue),
}

fn is_truthy(value: &HyperValue) -> bool {
    match value {
        HyperValue::None => false,
        HyperValue::Boolean(b) => *b,
        _ => true,
    }
}

fn coerce_named(value: HyperValue, type_name: &str, line: u32) -> HyperValue {
    let ty = match type_name {
        "int8" => "i8",
        "int16" => "i16",
        "int32" => "i32",
        "int64" => "i64",
        "uint8" => "u8",
        "uint16" => "u16",
        "uint32" => "u32",
        "uint64" => "u64",
        "float32" => "f32",
        "float64" => "f64",
        "boolean" => "bool",
        other => other,
    };

    if ty == "None" || ty == "any" {
        return value;
    }

    let as_i64 = |v: &HyperValue| -> Option<i64> {
        match v {
            HyperValue::I8(n) => Some(*n as i64),
            HyperValue::I16(n) => Some(*n as i64),
            HyperValue::I32(n) => Some(*n as i64),
            HyperValue::I64(n) => Some(*n),
            HyperValue::U8(n) => Some(*n as i64),
            HyperValue::U16(n) => Some(*n as i64),
            HyperValue::U32(n) => Some(*n as i64),
            HyperValue::U64(n) => Some(*n as i64),
            HyperValue::F32(n) => Some(*n as i64),
            HyperValue::F64(n) => Some(*n as i64),
            _ => None,
        }
    };
    let as_f64 = |v: &HyperValue| -> Option<f64> {
        match v {
            HyperValue::I8(n) => Some(*n as f64),
            HyperValue::I16(n) => Some(*n as f64),
            HyperValue::I32(n) => Some(*n as f64),
            HyperValue::I64(n) => Some(*n as f64),
            HyperValue::U8(n) => Some(*n as f64),
            HyperValue::U16(n) => Some(*n as f64),
            HyperValue::U32(n) => Some(*n as f64),
            HyperValue::U64(n) => Some(*n as f64),
            HyperValue::F32(n) => Some(*n as f64),
            HyperValue::F64(n) => Some(*n),
            _ => None,
        }
    };

    match ty {
        "i8" => as_i64(&value).map(|n| HyperValue::I8(n as i8)).unwrap_or(value),
        "i16" => as_i64(&value).map(|n| HyperValue::I16(n as i16)).unwrap_or(value),
        "i32" => as_i64(&value).map(|n| HyperValue::I32(n as i32)).unwrap_or(value),
        "i64" => as_i64(&value).map(HyperValue::I64).unwrap_or(value),
        "u8" => as_i64(&value).map(|n| HyperValue::U8(n as u8)).unwrap_or(value),
        "u16" => as_i64(&value).map(|n| HyperValue::U16(n as u16)).unwrap_or(value),
        "u32" => as_i64(&value).map(|n| HyperValue::U32(n as u32)).unwrap_or(value),
        "u64" => as_i64(&value).map(|n| HyperValue::U64(n as u64)).unwrap_or(value),
        "f32" => as_f64(&value).map(|n| HyperValue::F32(n as f32)).unwrap_or(value),
        "f64" => as_f64(&value).map(HyperValue::F64).unwrap_or(value),
        "bool" => match value {
            HyperValue::Boolean(b) => HyperValue::Boolean(b),
            other => other,
        },
        "string" => match value {
            HyperValue::String(s) => HyperValue::String(s),
            other => HyperValue::String(other.to_string()),
        },
        _ => {
            let _ = line;
            value
        }
    }
}

fn coerce_to_type(value: HyperValue, type_ann: &TypeAnn, line: u32) -> HyperValue {
    match type_ann {
        TypeAnn::None => value,
        TypeAnn::Named(name) => coerce_named(value, name, line),
        TypeAnn::Array { inner } => match value {
            HyperValue::List(elements) | HyperValue::Array { elements, .. } => {
                let elements = elements
                    .into_iter()
                    .map(|el| coerce_named(el, inner, line))
                    .collect();
                HyperValue::Array {
                    element_type: inner.clone(),
                    elements,
                }
            }
            other => {
                error::warning(&format!(
                    "line {}: cannot coerce value to Array[{}]; expected a list",
                    line, inner
                ));
                other
            }
        },
        TypeAnn::Dict { key, value: val_ty } => match value {
            HyperValue::Dict { entries, .. } => {
                let entries = entries
                    .into_iter()
                    .map(|(k, v)| (k, coerce_named(v, val_ty, line)))
                    .collect();
                HyperValue::Dict {
                    key_type: key.clone(),
                    val_type: val_ty.clone(),
                    entries,
                }
            }
            other => {
                error::warning(&format!(
                    "line {}: cannot coerce value to Dict[{}, {}]; expected a dict",
                    line, key, val_ty
                ));
                other
            }
        },
    }
}

/// Integer division by zero stops the program; float division yields infinity,
/// which is what the compiled path does too.
fn divides_by_integer_zero(left: &HyperValue, right: &HyperValue) -> bool {
    let is_float = |v: &HyperValue| matches!(v, HyperValue::F32(_) | HyperValue::F64(_));
    !is_float(left) && !is_float(right) && right.to_int() == Some(0)
}

fn literal_to_value(lit: &Literal) -> HyperValue {
    match lit {
        Literal::None => HyperValue::None,
        Literal::Bool(b) => HyperValue::Boolean(*b),
        Literal::Number(n) => {
            // i64 like the compiled path, so the same program cannot overflow
            // in one and not the other.
            if let Ok(num) = n.parse::<i64>() {
                HyperValue::I64(num)
            } else if let Ok(num) = n.parse::<f64>() {
                HyperValue::F64(num)
            } else {
                HyperValue::None
            }
        }
        Literal::String(s) => HyperValue::String(s.clone()),
    }
}

fn function_from_decl(
    decl: &FunctionDecl,
    env: Rc<RefCell<Environment>>,
) -> HyperValue {
    HyperValue::Function {
        name: decl.name.clone(),
        params: decl.params.iter().map(|p| p.name.clone()).collect(),
        body: Rc::new(*decl.body.clone()),
        is_strict: decl.is_strict,
        closure: env,
    }
}

fn to_i64(value: &HyperValue) -> i64 {
    match value {
        HyperValue::I64(val) => *val,
        HyperValue::I32(val) => *val as i64,
        HyperValue::I16(val) => *val as i64,
        HyperValue::I8(val) => *val as i64,
        HyperValue::U64(val) => *val as i64,
        HyperValue::U32(val) => *val as i64,
        HyperValue::U16(val) => *val as i64,
        HyperValue::U8(val) => *val as i64,
        HyperValue::F64(val) => *val as i64,
        HyperValue::F32(val) => *val as i64,
        _ => 0,
    }
}

fn index_get(object: &HyperValue, index: &HyperValue, line: u32) -> HyperValue {
    match object {
        HyperValue::List(items) | HyperValue::Array { elements: items, .. } => {
            let i = to_i64(index);
            if i < 0 || i as usize >= items.len() {
                error::runtime(
                    line,
                    format!(
                        "list index {} out of range (len {})",
                        i,
                        items.len()
                    ),
                );
            }
            items[i as usize].clone()
        }
        HyperValue::Dict { entries, .. } => {
            let key = index.to_string();
            entries.get(&key).cloned().unwrap_or(HyperValue::None)
        }
        HyperValue::String(s) => {
            let i = to_i64(index);
            let chars: Vec<char> = s.chars().collect();
            if i < 0 || i as usize >= chars.len() {
                error::runtime(
                    line,
                    format!(
                        "string index {} out of range (len {})",
                        i,
                        chars.len()
                    ),
                );
            }
            HyperValue::String(chars[i as usize].to_string())
        }
        _ => {
            error::runtime(line, "indexed value is not a list, dict, or string");
        }
    }
}

fn index_set(object: &mut HyperValue, index: &HyperValue, value: HyperValue, line: u32) {
    match object {
        HyperValue::List(items) | HyperValue::Array { elements: items, .. } => {
            let i = to_i64(index);
            if i < 0 || i as usize >= items.len() {
                error::runtime(
                    line,
                    format!(
                        "list index {} out of range (len {})",
                        i,
                        items.len()
                    ),
                );
            }
            items[i as usize] = value;
        }
        HyperValue::Dict { entries, .. } => {
            entries.insert(index.to_string(), value);
        }
        _ => {
            error::runtime(line, "indexed assignment requires a list or dict");
        }
    }
}

fn evaluate(expr: &Expr, line: u32, env: Rc<RefCell<Environment>>) -> Option<HyperValue> {
    match expr {
        Expr::Literal(lit) => Some(literal_to_value(lit)),
        Expr::Variable { name, line: var_line } => {
            Some(env.borrow().get(name, *var_line))
        }
        Expr::Group(inner) => evaluate(inner, line, env),
        Expr::Unary { op, right } => {
            let val = evaluate(right, line, Rc::clone(&env))?;
            match op {
                UnaryOp::Neg => {
                    if let Some(res) = val.negate() {
                        Some(res)
                    } else {
                        error::runtime(line, "invalid operand types for operation");
                    }
                }
                UnaryOp::Not => Some(HyperValue::Boolean(!is_truthy(&val))),
            }
        }
        Expr::Binary { op, left, right } => {
            match op {
                BinOp::And => {
                    let left_val = evaluate(left, line, Rc::clone(&env))?;
                    if !is_truthy(&left_val) {
                        return Some(left_val);
                    }
                    evaluate(right, line, env)
                }
                BinOp::Or => {
                    let left_val = evaluate(left, line, Rc::clone(&env))?;
                    if is_truthy(&left_val) {
                        return Some(left_val);
                    }
                    evaluate(right, line, env)
                }
                BinOp::Eq => {
                    let l = evaluate(left, line, Rc::clone(&env))?;
                    let r = evaluate(right, line, Rc::clone(&env))?;
                    Some(HyperValue::Boolean(l == r))
                }
                BinOp::Ne => {
                    let l = evaluate(left, line, Rc::clone(&env))?;
                    let r = evaluate(right, line, Rc::clone(&env))?;
                    Some(HyperValue::Boolean(l != r))
                }
                other => {
                    let left_val = evaluate(left, line, Rc::clone(&env))?;
                    let right_val = evaluate(right, line, Rc::clone(&env))?;
                    if matches!(other, BinOp::Div | BinOp::FloorDiv | BinOp::Rem)
                        && divides_by_integer_zero(&left_val, &right_val)
                    {
                        error::runtime(line, "division by zero");
                    }
                    let res = match other {
                        BinOp::Add => left_val.add(&right_val),
                        BinOp::Sub => left_val.sub(&right_val),
                        BinOp::Mul => left_val.mul(&right_val),
                        BinOp::Div => left_val.div(&right_val),
                        BinOp::FloorDiv => left_val.floor_div(&right_val),
                        BinOp::Rem => left_val.rem(&right_val),
                        BinOp::Pow => left_val.pow(&right_val),
                        BinOp::Gt => left_val.greater(&right_val),
                        BinOp::Lt => left_val.less(&right_val),
                        BinOp::Ge => left_val.greater_equal(&right_val),
                        BinOp::Le => left_val.less_equal(&right_val),
                        BinOp::And | BinOp::Or | BinOp::Eq | BinOp::Ne => unreachable!(),
                    };
                    if let Some(v) = res {
                        Some(v)
                    } else {
                        error::runtime(line, "invalid operand types for operation");
                    }
                }
            }
        }
        Expr::Assign { name, value } => {
            let val = evaluate(value, line, Rc::clone(&env))?;
            env.borrow_mut().assign(name, val.clone(), line);
            Some(val)
        }
        Expr::GetField { object, field } => {
            let target = env.borrow().get(object, line);
            match target {
                HyperValue::Instance {
                    fields,
                    field_indices,
                    ..
                } => {
                    if let Some(&idx) = field_indices.get(field) {
                        Some(fields.borrow()[idx].clone())
                    } else {
                        error::runtime(line, format!("undefined field '{}'", field));
                    }
                }
                HyperValue::Module { name, exports } => {
                    if let Some(val) = exports.get(field) {
                        Some(val.clone())
                    } else {
                        error::runtime(
                            line,
                            format!("module '{}' has no export '{}'", name, field),
                        );
                    }
                }
                _ => {
                    error::runtime(line, "only instances and modules have fields");
                }
            }
        }
        Expr::SetField {
            object,
            field,
            value,
        } => {
            let val = evaluate(value, line, Rc::clone(&env))?;
            let target = env.borrow().get(object, line);
            if let HyperValue::Instance {
                fields,
                field_indices,
                ..
            } = target
            {
                if let Some(&idx) = field_indices.get(field) {
                    fields.borrow_mut()[idx] = val.clone();
                    Some(val)
                } else {
                    error::runtime(line, format!("undefined field '{}'", field));
                }
            } else {
                error::runtime(line, "only instances have fields");
            }
        }
        Expr::Call { callee, args } => evaluate_call(callee, args, line, env),
        Expr::CallMethod {
            object,
            method,
            args,
        } => {
            let mut evaluated_args = Vec::new();
            for arg in args {
                evaluated_args.push(evaluate(arg, line, Rc::clone(&env))?);
            }

            if let Expr::Variable { name: object_name, .. } = object.as_ref() {
                let target_val = env.borrow().get(object_name, line);

                if let HyperValue::Instance { ref methods, .. } = target_val {
                    if let Some((
                        _is_pub,
                        HyperValue::Function {
                            params,
                            body,
                            closure,
                            ..
                        },
                    )) = methods.get(method)
                    {
                        let method_env =
                            Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(
                                closure,
                            ))));
                        method_env
                            .borrow_mut()
                            .define("self".to_string(), target_val.clone(), true);

                        for (param_name, arg_value) in params.iter().skip(1).zip(evaluated_args.iter())
                        {
                            method_env
                                .borrow_mut()
                                .define(param_name.clone(), arg_value.clone(), true);
                        }

                        return match execute(body, method_env) {
                            ExecResult::Return(val) => Some(val),
                            ExecResult::Ok => Some(HyperValue::None),
                        };
                    } else if methods.contains_key(method) {
                        error::runtime(line, format!("method '{}' not found", method));
                    }
                }

                if let HyperValue::Module { name, exports } = &target_val {
                    match exports.get(method) {
                        Some(HyperValue::Function {
                            params,
                            body,
                            closure,
                            ..
                        }) => {
                            let call_env =
                                Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(
                                    closure,
                                ))));
                            if params.len() != evaluated_args.len() {
                                error::runtime(
                                    line,
                                    format!(
                                        "expected {} arguments but got {}",
                                        params.len(),
                                        evaluated_args.len()
                                    ),
                                );
                            }
                            for (param_name, arg_value) in params.iter().zip(evaluated_args.iter()) {
                                call_env
                                    .borrow_mut()
                                    .define(param_name.clone(), arg_value.clone(), true);
                            }
                            return match execute(body, call_env) {
                                ExecResult::Return(val) => Some(val),
                                ExecResult::Ok => Some(HyperValue::None),
                            };
                        }
                        Some(HyperValue::StructDef {
                            name: sname,
                            fields,
                            methods,
                            implemented_trait,
                        }) => {
                            return Some(instantiate_struct(
                                sname,
                                fields,
                                methods,
                                implemented_trait,
                                evaluated_args,
                                &HashMap::new(),
                                line,
                            ));
                        }
                        Some(HyperValue::NativeFunction(native)) => {
                            return call_native(native, &evaluated_args, line);
                        }
                        Some(_) => {
                            error::runtime(
                                line,
                                format!("'{}.{}' is not callable", name, method),
                            );
                        }
                        None => {
                            error::runtime(
                                line,
                                format!("module '{}' has no export '{}'", name, method),
                            );
                        }
                    }
                }

                return env.borrow_mut().with_value_mut(object_name, line, |current| {
                    current.call_method(method, &evaluated_args, line)
                });
            }

            let mut target_val = evaluate(object, line, Rc::clone(&env))?;
            target_val.call_method(method, &evaluated_args, line)
        }
        Expr::List(items) => {
            let mut elements = Vec::new();
            for item in items {
                elements.push(evaluate(item, line, Rc::clone(&env))?);
            }
            Some(HyperValue::List(elements))
        }
        Expr::Dict(entries) => {
            let mut map = IndexMap::new();
            for (key_expr, val_expr) in entries {
                let key_val = evaluate(key_expr, line, Rc::clone(&env))?;
                let value = evaluate(val_expr, line, Rc::clone(&env))?;
                map.insert(key_val.to_string(), value);
            }
            Some(HyperValue::Dict {
                key_type: "string".to_string(),
                val_type: "any".to_string(),
                entries: map,
            })
        }
        Expr::Index { object, index } => {
            let obj = evaluate(object, line, Rc::clone(&env))?;
            let idx = evaluate(index, line, Rc::clone(&env))?;
            Some(index_get(&obj, &idx, line))
        }
        Expr::IndexSet {
            object,
            index,
            value,
        } => {
            let idx = evaluate(index, line, Rc::clone(&env))?;
            let val = evaluate(value, line, Rc::clone(&env))?;
            match object.as_ref() {
                Expr::Variable { name, line: var_line } => {
                    let idx_c = idx.clone();
                    let val_c = val.clone();
                    env.borrow_mut().with_value_mut(name, *var_line, |current| {
                        index_set(current, &idx_c, val_c, line);
                    });
                    Some(val)
                }
                _ => {
                    let mut obj = evaluate(object, line, Rc::clone(&env))?;
                    index_set(&mut obj, &idx, val.clone(), line);
                    Some(val)
                }
            }
        }
        Expr::FString { line: f_line, parts } => {
            let mut evaluated_string = String::new();
            for part in parts {
                match part {
                    FStringPart::Literal(s) => evaluated_string.push_str(s),
                    FStringPart::Expr(e) => {
                        if let Some(val) = evaluate(e, *f_line, Rc::clone(&env)) {
                            evaluated_string.push_str(&val.to_string());
                        }
                    }
                }
            }
            Some(HyperValue::String(evaluated_string))
        }
        Expr::Ternary {
            condition,
            then_branch,
            else_branch,
        } => {
            let cond_val = evaluate(condition, line, Rc::clone(&env))?;
            if is_truthy(&cond_val) {
                evaluate(then_branch, line, env)
            } else {
                evaluate(else_branch, line, env)
            }
        }
    }
}

fn instantiate_struct(
    name: &str,
    fields: &[(String, String, bool, bool, usize)],
    methods: &HashMap<String, (bool, HyperValue)>,
    _implemented_trait: &str,
    positional_args: Vec<HyperValue>,
    named_args: &HashMap<String, HyperValue>,
    _line: u32,
) -> HyperValue {
    let mut instance_fields_vec = Vec::new();
    let mut field_indices = HashMap::new();
    let mut field_visibility = HashMap::new();

    for (idx, (f_name, _, is_pub, _, _)) in fields.iter().enumerate() {
        let initial = named_args.get(f_name).cloned().unwrap_or(HyperValue::None);
        instance_fields_vec.push(initial);
        field_indices.insert(f_name.clone(), idx);
        field_visibility.insert(f_name.clone(), *is_pub);
    }

    let instance = HyperValue::Instance {
        struct_name: name.to_string(),
        fields: Rc::new(RefCell::new(instance_fields_vec)),
        field_indices,
        field_visibility,
        methods: methods.clone(),
    };

    if let Some((
        _is_pub,
        HyperValue::Function {
            params,
            body,
            closure,
            ..
        },
    )) = methods.get("__init__")
    {
        let init_env =
            Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(closure))));
        init_env
            .borrow_mut()
            .define("self".to_string(), instance.clone(), true);

        let mut init_args = positional_args;
        if init_args.is_empty() && !named_args.is_empty() {
            for (f_name, _, _, _, _) in fields.iter() {
                if let Some(val) = named_args.get(f_name) {
                    init_args.push(val.clone());
                }
            }
        }

        for (param_name, arg_value) in params.iter().skip(1).zip(init_args) {
            init_env
                .borrow_mut()
                .define(param_name.clone(), arg_value, true);
        }

        execute(body, init_env);
    }
    instance
}

fn native_fatal(line: u32, message: String) -> ! {
    error::runtime(line, message);
}

/// Builtins whose arguments are plain values, shared by direct calls and by
/// native module members such as `json.dumps`.
fn call_native(name: &str, args: &[HyperValue], line: u32) -> Option<HyperValue> {
    match name {
        "open" => {
            let path = match args.first() {
                Some(HyperValue::String(p)) => p.clone(),
                Some(other) => other.to_string(),
                None => native_fatal(line, "open expects a file path".to_string()),
            };
            let mode = match args.get(1) {
                Some(HyperValue::String(m)) => m.clone(),
                Some(other) => other.to_string(),
                None => "r".to_string(),
            };
            if args.len() > 2 {
                native_fatal(
                    line,
                    format!("open expects 1 or 2 argument(s) but got {}", args.len()),
                );
            }
            Some(crate::fileio::open_value(&path, &mode, line))
        }
        "json.loads" => {
            let text = match args {
                [value] => value.to_string(),
                _ => native_fatal(line, "json.loads expects 1 argument".to_string()),
            };
            match crate::json::parse(&text) {
                Ok(value) => Some(value),
                Err(msg) => native_fatal(line, format!("invalid JSON: {}", msg)),
            }
        }
        "json.dumps" => {
            let (value, indent) = match args {
                [value] => (value, 0),
                [value, indent] => (value, indent.to_int().unwrap_or(0).max(0) as usize),
                _ => native_fatal(line, "json.dumps expects 1 or 2 argument(s)".to_string()),
            };
            match crate::json::stringify(value, indent) {
                Ok(text) => Some(HyperValue::String(text)),
                Err(msg) => native_fatal(line, msg),
            }
        }
        "json.load" => {
            let handle = match args {
                [HyperValue::File { file, .. }] => file,
                [_] => native_fatal(line, "json.load expects an open file".to_string()),
                _ => native_fatal(line, "json.load expects 1 argument".to_string()),
            };
            let text = match handle.borrow_mut().read_all() {
                Ok(text) => text,
                Err(e) => native_fatal(line, format!("json.load could not read the file: {}", e)),
            };
            match crate::json::parse(&text) {
                Ok(value) => Some(value),
                Err(msg) => native_fatal(line, format!("invalid JSON: {}", msg)),
            }
        }
        "json.dump" => {
            let (value, handle, indent) = match args {
                [value, HyperValue::File { file, .. }] => (value, file, 0),
                [value, HyperValue::File { file, .. }, indent] => {
                    (value, file, indent.to_int().unwrap_or(0).max(0) as usize)
                }
                [_, _] | [_, _, _] => {
                    native_fatal(line, "json.dump expects an open file as its second argument".to_string())
                }
                _ => native_fatal(line, "json.dump expects 2 or 3 argument(s)".to_string()),
            };
            let text = match crate::json::stringify(value, indent) {
                Ok(text) => text,
                Err(msg) => native_fatal(line, msg),
            };
            match handle.borrow_mut().write_str(&text) {
                Ok(n) => Some(HyperValue::I64(n as i64)),
                Err(e) => native_fatal(line, format!("json.dump could not write the file: {}", e)),
            }
        }
        other => native_fatal(line, format!("'{}' is not callable", other)),
    }
}

fn evaluate_call(
    callee: &Expr,
    args: &[CallArg],
    line: u32,
    env: Rc<RefCell<Environment>>,
) -> Option<HyperValue> {
    let call_val = evaluate(callee, line, Rc::clone(&env))?;

    match call_val {
        HyperValue::NativeFunction(name) if name == "input" => {
            if let Some(CallArg::Positional(prompt_expr)) = args.first() {
                if let Some(prompt_val) = evaluate(prompt_expr, line, Rc::clone(&env)) {
                    println!("{}", prompt_val);
                    let _ = io::stdout().flush();
                }
            } else if let Some(CallArg::Named { value, .. }) = args.first() {
                if let Some(prompt_val) = evaluate(value, line, Rc::clone(&env)) {
                    println!("{}", prompt_val);
                    let _ = io::stdout().flush();
                }
            }

            let mut input_buffer = String::new();
            if io::stdin().read_line(&mut input_buffer).is_ok() {
                let trimmed = input_buffer.trim_end_matches(&['\r', '\n'][..]).to_string();
                Some(HyperValue::String(trimmed))
            } else {
                error::runtime(line, "failed to read line from stdin");
            }
        }
        HyperValue::NativeFunction(name) if name == "clock" => {
            if !args.is_empty() {
                error::runtime(line, format!("expected 0 arguments but got {}", args.len()));
            }
            let duration = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap();
            Some(HyperValue::F64(duration.as_secs_f64()))
        }
        HyperValue::NativeFunction(name) => {
            let mut evaluated_args = Vec::new();
            for arg in args {
                match arg {
                    CallArg::Positional(e) | CallArg::Named { value: e, .. } => {
                        evaluated_args.push(evaluate(e, line, Rc::clone(&env))?);
                    }
                }
            }
            call_native(&name, &evaluated_args, line)
        }
        HyperValue::Function {
            params,
            body,
            closure,
            ..
        } => {
            let mut evaluated_args = Vec::new();
            for arg in args {
                match arg {
                    CallArg::Positional(e) | CallArg::Named { value: e, .. } => {
                        evaluated_args.push(evaluate(e, line, Rc::clone(&env))?);
                    }
                }
            }

            if evaluated_args.len() != params.len() {
                error::runtime(
                    line,
                    format!(
                        "expected {} arguments but got {}",
                        params.len(),
                        evaluated_args.len()
                    ),
                );
            }

            let closure_env =
                Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&closure))));
            for (param_name, arg_value) in params.iter().zip(evaluated_args) {
                closure_env
                    .borrow_mut()
                    .define(param_name.clone(), arg_value, false);
            }

            match execute(&body, closure_env) {
                ExecResult::Return(val) => Some(val),
                ExecResult::Ok => Some(HyperValue::None),
            }
        }
        HyperValue::StructDef {
            name,
            fields,
            methods,
            implemented_trait,
        } => {
            let mut positional_args = Vec::new();
            let mut named_args: HashMap<String, HyperValue> = HashMap::new();

            for arg in args {
                match arg {
                    CallArg::Named { name, value } => {
                        if let Some(val) = evaluate(value, line, Rc::clone(&env)) {
                            named_args.insert(name.clone(), val);
                        }
                    }
                    CallArg::Positional(e) => {
                        if let Some(val) = evaluate(e, line, Rc::clone(&env)) {
                            positional_args.push(val);
                        }
                    }
                }
            }

            Some(instantiate_struct(
                &name,
                &fields,
                &methods,
                &implemented_trait,
                positional_args,
                &named_args,
                line,
            ))
        }
        _ => {
            error::runtime(line, "can only call functions and classes");
        }
    }
}

fn execute(stmt: &Stmt, env: Rc<RefCell<Environment>>) -> ExecResult {
    match stmt {
        Stmt::Let {
            line,
            is_mutable,
            name,
            type_ann,
            initializer,
        } => {
            let raw = evaluate(initializer, *line, Rc::clone(&env)).unwrap_or(HyperValue::None);
            let value = coerce_to_type(raw, type_ann, *line);
            env.borrow_mut()
                .define(name.clone(), value, *is_mutable);
            ExecResult::Ok
        }
        Stmt::Expr { line, expr } => {
            evaluate(expr, *line, env);
            ExecResult::Ok
        }
        Stmt::Print { line, values } => {
            let mut evaluated_results = Vec::new();
            for value in values {
                if let Some(val) = evaluate(value, *line, Rc::clone(&env)) {
                    evaluated_results.push(val.to_string());
                }
            }
            println!("{}", evaluated_results.join(" "));
            ExecResult::Ok
        }
        Stmt::Block(statements) => {
            let block_env =
                Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&env))));
            for sub_stmt in statements {
                if let ExecResult::Return(val) = execute(sub_stmt, Rc::clone(&block_env)) {
                    return ExecResult::Return(val);
                }
            }
            ExecResult::Ok
        }
        Stmt::Struct {
            name,
            implemented_trait,
            fields,
            methods,
        } => {
            let trait_name = implemented_trait.clone().unwrap_or_default();
            if !trait_name.is_empty() {
                let trait_check = env.borrow().get(&trait_name, 0);
                if !matches!(trait_check, HyperValue::TraitDef { .. }) {
                    error::runtime(0, format!("trait '{}' is not defined", trait_name));
                }
            }

            let mut field_defs = Vec::new();
            for (idx, field) in fields.iter().enumerate() {
                field_defs.push((
                    field.name.clone(),
                    field.type_name.clone(),
                    field.is_pub,
                    field.is_mut,
                    idx,
                ));
            }

            let mut methods_map = HashMap::new();
            for method in methods {
                let func_val = function_from_decl(&method.function, Rc::clone(&env));
                methods_map.insert(method.function.name.clone(), (method.is_pub, func_val));
            }

            let struct_def = HyperValue::StructDef {
                name: name.clone(),
                implemented_trait: trait_name,
                fields: field_defs,
                methods: methods_map,
            };
            env.borrow_mut().define(name.clone(), struct_def, false);
            ExecResult::Ok
        }
        Stmt::Trait { name, methods } => {
            let method_names: Vec<String> = methods.iter().map(|m| m.name.clone()).collect();
            let trait_def = HyperValue::TraitDef {
                name: name.clone(),
                methods: method_names,
            };
            env.borrow_mut().define(name.clone(), trait_def, false);
            ExecResult::Ok
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
        } => {
            if let Some(cond_val) = evaluate(condition, 1, Rc::clone(&env)) {
                if is_truthy(&cond_val) {
                    let res = execute(then_branch, env);
                    if matches!(res, ExecResult::Return(_)) {
                        return res;
                    }
                } else if let Some(else_b) = else_branch {
                    let res = execute(else_b, env);
                    if matches!(res, ExecResult::Return(_)) {
                        return res;
                    }
                }
            }
            ExecResult::Ok
        }
        Stmt::While {
            line,
            condition,
            body,
        } => {
            while let Some(cond_val) = evaluate(condition, *line, Rc::clone(&env)) {
                if is_truthy(&cond_val) {
                    if let ExecResult::Return(val) = execute(body, Rc::clone(&env)) {
                        return ExecResult::Return(val);
                    }
                } else {
                    break;
                }
            }
            ExecResult::Ok
        }
        Stmt::For {
            kind,
            line,
            var,
            iter,
            body,
        } => {
            match iter {
                ForIter::Range { start, end } => {
                    let start_val = evaluate(start, *line, Rc::clone(&env))
                        .as_ref()
                        .map(to_i64)
                        .unwrap_or(0);
                    let end_val = evaluate(end, *line, Rc::clone(&env))
                        .as_ref()
                        .map(to_i64)
                        .unwrap_or(0);

                    let is_parallel =
                        matches!(kind, ForKind::Parallel | ForKind::ParallelVectorized);
                    let is_vectorized =
                        matches!(kind, ForKind::Vectorized | ForKind::ParallelVectorized);

                    if is_parallel {
                        let num_threads = std::thread::available_parallelism()
                            .map(|n| n.get())
                            .unwrap_or(4);
                        let total_items = (end_val - start_val).max(0);
                        if total_items > 0 {
                            let chunk_size =
                                (total_items + num_threads as i64 - 1) / num_threads as i64;
                            let vec_step: i64 = if is_vectorized { 4 } else { 1 };
                            let body_stmt = (*body).clone();
                            let var_name = var.clone();

                            std::thread::scope(|s| {
                                for t in 0..num_threads {
                                    let t_start = start_val + t as i64 * chunk_size;
                                    let t_end = (t_start + chunk_size).min(end_val);
                                    if t_start >= end_val {
                                        break;
                                    }

                                    let var_n = var_name.clone();
                                    let b_stmt = body_stmt.clone();

                                    s.spawn(move || {
                                        let thread_local_rc =
                                            Rc::new(RefCell::new(Environment::new()));
                                        let loop_env = Rc::new(RefCell::new(
                                            Environment::new_with_enclosing(thread_local_rc),
                                        ));

                                        let mut i = t_start;
                                        while i < t_end {
                                            let lane_end = (i + vec_step).min(t_end);
                                            while i < lane_end {
                                                loop_env.borrow_mut().define(
                                                    var_n.clone(),
                                                    HyperValue::I64(i),
                                                    true,
                                                );
                                                execute(&b_stmt, Rc::clone(&loop_env));
                                                i += 1;
                                            }
                                        }
                                    });
                                }
                            });
                        }
                    } else if is_vectorized {
                        let mut i = start_val;
                        while i < end_val {
                            let lane_end = (i + 4).min(end_val);
                            while i < lane_end {
                                let loop_env = Rc::new(RefCell::new(
                                    Environment::new_with_enclosing(Rc::clone(&env)),
                                ));
                                loop_env
                                    .borrow_mut()
                                    .define(var.clone(), HyperValue::I64(i), true);
                                if let ExecResult::Return(val) = execute(body, loop_env) {
                                    return ExecResult::Return(val);
                                }
                                i += 1;
                            }
                        }
                    } else {
                        for i in start_val..end_val {
                            let loop_env = Rc::new(RefCell::new(
                                Environment::new_with_enclosing(Rc::clone(&env)),
                            ));
                            loop_env
                                .borrow_mut()
                                .define(var.clone(), HyperValue::I64(i), true);
                            if let ExecResult::Return(val) = execute(body, loop_env) {
                                return ExecResult::Return(val);
                            }
                        }
                    }
                }
                ForIter::Iterable(iterable) => {
                    let collection = evaluate(iterable, *line, Rc::clone(&env))
                        .unwrap_or(HyperValue::None);
                    let items: Vec<HyperValue> = match collection {
                        HyperValue::List(items) => items,
                        HyperValue::Array { elements, .. } => elements,
                        other => {
                            error::runtime(
                                *line,
                                format!("for-in expects a list, got {}", other),
                            );
                        }
                    };
                    for item in items {
                        let loop_env = Rc::new(RefCell::new(Environment::new_with_enclosing(
                            Rc::clone(&env),
                        )));
                        loop_env.borrow_mut().define(var.clone(), item, true);
                        if let ExecResult::Return(val) = execute(body, Rc::clone(&loop_env)) {
                            return ExecResult::Return(val);
                        }
                    }
                }
            }
            ExecResult::Ok
        }
        Stmt::Function(decl) => {
            let func_val = function_from_decl(decl, Rc::clone(&env));
            env.borrow_mut()
                .define(decl.name.clone(), func_val, false);
            ExecResult::Ok
        }
        Stmt::Return { line, value } => {
            let val = evaluate(value, *line, env).unwrap_or(HyperValue::None);
            ExecResult::Return(val)
        }
        Stmt::WithMmap {
            line,
            path,
            var,
            body,
        } => {
            let path_val = evaluate(path, *line, Rc::clone(&env)).unwrap_or(HyperValue::None);
            let file_path = match path_val {
                HyperValue::String(s) => s,
                _ => "".to_string(),
            };

            match crate::fileio::MappedFile::open(&file_path) {
                Ok(map) => {
                    let mmap_val = HyperValue::MmapFile {
                        map: Rc::new(map),
                        path: file_path,
                    };
                    let block_env =
                        Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&env))));
                    block_env.borrow_mut().define(var.clone(), mmap_val, false);
                    execute(body, block_env);
                }
                Err(e) => {
                    error::runtime(
                        *line,
                        format!("could not map file '{}': {}", file_path, e),
                    );
                }
            }
            ExecResult::Ok
        }
        Stmt::With {
            line,
            value,
            var,
            body,
        } => {
            let resource = evaluate(value, *line, Rc::clone(&env)).unwrap_or(HyperValue::None);
            let block_env = Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&env))));
            block_env
                .borrow_mut()
                .define(var.clone(), resource.clone(), false);
            let result = execute(body, block_env);
            if let HyperValue::File { file, path } = &resource {
                if let Err(e) = file.borrow_mut().close() {
                    error::runtime(
                        *line,
                        format!("could not close '{}': {}", path, e),
                    );
                }
            }
            result
        }
        Stmt::Import {
            line,
            module,
            alias,
        } => {
            let module_val = load_module(module, *line);
            let bind_name = alias.as_ref().unwrap_or(module).clone();
            env.borrow_mut().define(bind_name, module_val, false);
            ExecResult::Ok
        }
        Stmt::ImportFrom {
            line,
            module,
            names,
        } => {
            let module_val = load_module(module, *line);
            let HyperValue::Module { name, exports } = module_val else {
                unreachable!("load_module always returns Module");
            };
            for item in names {
                match exports.get(&item.name) {
                    Some(val) => {
                        let bind = item.alias.as_ref().unwrap_or(&item.name).clone();
                        env.borrow_mut().define(bind, val.clone(), false);
                    }
                    None => {
                        error::runtime(
                            *line,
                            format!("module '{}' has no export '{}'", name, item.name),
                        );
                    }
                }
            }
            ExecResult::Ok
        }
    }
}

pub fn run_evaluate(file_contents: String) {
    let (tokens, error) = crate::scanner::scan_tokens(&file_contents);
    if error {
        std::process::exit(65);
    }

    let mut parser = crate::parser::Parser::new(tokens);
    let env = Rc::new(RefCell::new(Environment::new()));

    match parser.parse() {
        Ok(ast) => {
            if let Some(result) = evaluate(&ast, 1, Rc::clone(&env)) {
                println!("{}", result);
            } else {
                std::process::exit(65);
            }
        }
        Err(_) => {
            std::process::exit(65);
        }
    }
}

pub fn run_program(file_contents: String, entry_path: &str) {
    MODULE_RUNTIME.with(|cell| {
        *cell.borrow_mut() = Some(ModuleRuntime::new(Path::new(entry_path)));
    });

    let statements = match crate::driver::parse_program(&file_contents) {
        Ok(stmts) => stmts,
        Err(()) => std::process::exit(65),
    };

    if let Err(errors) = crate::semantic::typecheck(&statements) {
        for e in &errors {
            error::report_formatted(&format!("warning: {}", e));
        }
    }

    let env = Rc::new(RefCell::new(Environment::new()));
    for stmt in statements {
        execute(&stmt, Rc::clone(&env));
    }

    MODULE_RUNTIME.with(|cell| {
        *cell.borrow_mut() = None;
    });
}
