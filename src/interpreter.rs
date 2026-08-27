use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::{cell::RefCell, io};
use std::rc::Rc;
use crate::ast::*;
use crate::environment::{Environment, HyperValue};

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
        TypeAnn::Array { .. } | TypeAnn::Dict { .. } => value,
    }
}

fn literal_to_value(lit: &Literal) -> HyperValue {
    match lit {
        Literal::None => HyperValue::None,
        Literal::Bool(b) => HyperValue::Boolean(*b),
        Literal::Number(n) => {
            if let Ok(num) = n.parse::<i32>() {
                HyperValue::I32(num)
            } else if let Ok(num) = n.parse::<i64>() {
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
                eprintln!(
                    "[line {}] Error: List index {} out of bounds (len {}).",
                    line,
                    i,
                    items.len()
                );
                std::process::exit(70);
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
                eprintln!(
                    "[line {}] Error: String index {} out of bounds (len {}).",
                    line,
                    i,
                    chars.len()
                );
                std::process::exit(70);
            }
            HyperValue::String(chars[i as usize].to_string())
        }
        _ => {
            eprintln!("[line {}] Error: Indexed value is not a list, dict, or string.", line);
            std::process::exit(70);
        }
    }
}

fn index_set(object: &mut HyperValue, index: &HyperValue, value: HyperValue, line: u32) {
    match object {
        HyperValue::List(items) | HyperValue::Array { elements: items, .. } => {
            let i = to_i64(index);
            if i < 0 || i as usize >= items.len() {
                eprintln!(
                    "[line {}] Error: List index {} out of bounds (len {}).",
                    line,
                    i,
                    items.len()
                );
                std::process::exit(70);
            }
            items[i as usize] = value;
        }
        HyperValue::Dict { entries, .. } => {
            entries.insert(index.to_string(), value);
        }
        _ => {
            eprintln!("[line {}] Error: Indexed assignment requires a list or dict.", line);
            std::process::exit(70);
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
                        eprintln!("[line {}] Type Error: Invalid operand types for operation.", line);
                        std::process::exit(70);
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
                    let res = match other {
                        BinOp::Add => left_val.add(&right_val),
                        BinOp::Sub => left_val.sub(&right_val),
                        BinOp::Mul => left_val.mul(&right_val),
                        BinOp::Div => left_val.div(&right_val),
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
                        eprintln!("[line {}] Type Error: Invalid operand types for operation.", line);
                        std::process::exit(70);
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
            if let HyperValue::Instance {
                fields,
                field_indices,
                ..
            } = target
            {
                if let Some(&idx) = field_indices.get(field) {
                    Some(fields.borrow()[idx].clone())
                } else {
                    eprintln!("[line {}] Error: Undefined field '{}'.", line, field);
                    std::process::exit(70);
                }
            } else {
                eprintln!("[line {}] Error: Only instances have fields.", line);
                std::process::exit(70);
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
                    eprintln!("[line {}] Error: Undefined field '{}'.", line, field);
                    std::process::exit(70);
                }
            } else {
                eprintln!("[line {}] Error: Only instances have fields.", line);
                std::process::exit(70);
            }
        }
        Expr::Call { callee, args } => evaluate_call(callee, args, line, env),
        Expr::CallMethod {
            object,
            method,
            args,
        } => {
            let target_val = env.borrow().get(object, line);
            let mut evaluated_args = Vec::new();
            for arg in args {
                evaluated_args.push(evaluate(arg, line, Rc::clone(&env))?);
            }

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

                    for (param_name, arg_value) in params.iter().skip(1).zip(evaluated_args.iter()) {
                        method_env
                            .borrow_mut()
                            .define(param_name.clone(), arg_value.clone(), true);
                    }

                    return match execute(body, method_env) {
                        ExecResult::Return(val) => Some(val),
                        ExecResult::Ok => Some(HyperValue::None),
                    };
                } else if methods.contains_key(method) {
                    eprintln!("[line {}] Error: Method '{}' not found.", line, method);
                    std::process::exit(70);
                }
            }

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
            let mut map = HashMap::new();
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
                eprintln!("[line {}] Error: Failed to read  line from stdin.", line);
                std::process::exit(70);
            }
        }
        HyperValue::NativeFunction(name) if name == "clock" => {
            if !args.is_empty() {
                eprintln!("Expected 0 arguments but got more.\n[line {}]", line);
                std::process::exit(70);
            }
            let duration = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap();
            Some(HyperValue::F64(duration.as_secs_f64()))
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
                eprintln!(
                    "Expected {} arguments but got {}.\n[line {}]",
                    params.len(),
                    evaluated_args.len(),
                    line
                );
                std::process::exit(70);
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
            ..
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
                struct_name: name.clone(),
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
                    Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(
                        closure,
                    ))));
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
            Some(instance)
        }
        _ => {
            eprintln!("Can only call functions and classes.\n[line {}]", line);
            std::process::exit(70);
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
                    eprintln!("Error: Trait '{}' is not defined.", trait_name);
                    std::process::exit(70);
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
                            eprintln!(
                                "[line {}] Error: for-in expects a list, got {}.",
                                line, other
                            );
                            std::process::exit(70);
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

            if let Ok(file) = File::open(&file_path) {
                let mmap_val = HyperValue::MmapFile {
                    file: Rc::new(RefCell::new(file)),
                    path: file_path,
                };
                let block_env =
                    Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&env))));
                block_env.borrow_mut().define(var.clone(), mmap_val, false);
                execute(body, block_env);
            } else {
                eprintln!(
                    "[line {}] Error: Could not open file '{}'",
                    line, file_path
                );
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

pub fn run_program(file_contents: String) {
    let statements = match crate::driver::parse_program(&file_contents) {
        Ok(stmts) => stmts,
        Err(()) => {
            eprintln!("Syntax error.");
            std::process::exit(65);
        }
    };

    // Type errors are non-fatal for `run` while the interpreter remains the default backend.
    if let Err(errors) = crate::semantic::typecheck(&statements) {
        for e in &errors {
            eprintln!("warning: {}", e);
        }
    }

    let env = Rc::new(RefCell::new(Environment::new()));
    for stmt in statements {
        execute(&stmt, Rc::clone(&env));
    }
}
