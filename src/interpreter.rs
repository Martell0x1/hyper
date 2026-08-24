use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::{cell::RefCell, io};
use std::rc::Rc;
use crate::environment::{Environment, HyperValue};

pub enum ExecResult {
    Ok,
    Return(HyperValue),
}

fn clean_group_expressions(mut input: String) -> String {
    while input.starts_with("(group ") && input.ends_with(')') {
        input = input[7..input.len() - 1].to_string();
    } 
    input
}

fn is_truthy(value: &HyperValue) -> bool {
    match value {
        HyperValue::None => false,
        HyperValue::Boolean(b) => *b,
        _ => true,
    }
}

fn split_binary_args(inner: &str) -> Option<(String, String)> {
    let mut bracket_count = 0;
    let chars: Vec<char> = inner.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch == '(' { bracket_count += 1; }
        else if ch == ')' { bracket_count -= 1; }
        else if ch == ' ' && bracket_count == 0 {
            let left = chars[..i].iter().collect::<String>();
            let right = chars[i + 1..].iter().collect::<String>();
            return Some((left, right));
        }
    }
    None
}

fn split_block_statements(inner: &str) -> Vec<String> {
    let mut stmts = Vec::new();
    let mut current = String::new();
    let mut bracket_count = 0;

    for ch in inner.chars() {
        if ch == '(' { bracket_count += 1; }
        else if ch == ')' { bracket_count -= 1; }
        current.push(ch);

        if bracket_count == 0 {
            let trimmed = current.trim();
            if !trimmed.is_empty() { stmts.push(trimmed.to_string()); }
            current.clear();
        }
    }
    stmts
}

fn eval_binary_op<F>(inner: &str, line: u32, env: Rc<RefCell<Environment>>, op: F) -> Option<HyperValue>
where
    F: FnOnce(&HyperValue, &HyperValue) -> Option<HyperValue>,
{
    let (left_str, right_str) = split_binary_args(inner)?;
    let left_val = evaluate_str(left_str, line, Rc::clone(&env))?;
    let right_val = evaluate_str(right_str, line, Rc::clone(&env))?;

    if let Some(res) = op(&left_val, &right_val) {
        Some(res)
    } else {
        eprintln!("[line {}] Type Error: Invalid operand types for operation.", line);
        std::process::exit(70);
    }
}

fn evaluate_str(ast_string: String, line: u32, env: Rc<RefCell<Environment>>) -> Option<HyperValue> {
    let cleaned = clean_group_expressions(ast_string);

    if cleaned.starts_with("let_ref:") {
        return Some(env.borrow().get(&cleaned[8..], line));
    }

    if cleaned.starts_with("(f_string line:") && cleaned.ends_with(')') {
        let space_idx = cleaned.find(' ').unwrap();
        let rest = &cleaned[space_idx + 1..cleaned.len() - 1];
        let line_space_idx = rest.find(' ').unwrap();
        let line_num: u32 = rest[5..line_space_idx].parse().unwrap_or(1);
        let brackets_content = rest[line_space_idx + 1..].trim();
        
        let inner = if brackets_content.starts_with('[') && brackets_content.ends_with(']') {
            &brackets_content[1..brackets_content.len() - 1]
        } else {
            brackets_content
        };

        let mut evaluated_string = String::new();
        if !inner.trim().is_empty() {
            let mut current_parts = inner.to_string();
            while let Some((left, right)) = split_binary_args(&current_parts) {
                if let Some(val) = evaluate_str(left, line_num, Rc::clone(&env)) {
                    evaluated_string.push_str(&val.to_string());
                }
                current_parts = right;
            }
            if !current_parts.trim().is_empty() {
                if let Some(val) = evaluate_str(current_parts.trim().to_string(), line_num, Rc::clone(&env)) {
                    evaluated_string.push_str(&val.to_string());
                }
            }
        }
        return Some(HyperValue::String(evaluated_string));
    }

    if cleaned.starts_with("(call_method ") && cleaned.ends_with(')') {
        let inner = &cleaned[13..cleaned.len() - 1];
        
        let parts: Vec<&str> = inner.splitn(3, ' ').collect();
        if parts.len() >= 2 {
            let var_name = parts[0];
            let method_name = parts[1];
            let args_raw = if parts.len() == 3 { parts[2] } else { "[]" };

            let target_val = env.borrow().get(var_name, line);

            let mut evaluated_args = Vec::new();
            let args_trimmed = args_raw.trim();
            if args_trimmed.starts_with('[') && args_trimmed.ends_with(']') && args_trimmed.len() > 2 {
                let args_inner = &args_trimmed[1..args_trimmed.len() - 1];
                let mut current_args = args_inner.to_string();
                
                while let Some((left, right)) = split_binary_args(&current_args) {
                    if let Some(val) = evaluate_str(left, line, Rc::clone(&env)) {
                        evaluated_args.push(val);
                    }
                    current_args = right;
                }
                if !current_args.trim().is_empty() {
                    if let Some(val) = evaluate_str(current_args.trim().to_string(), line, Rc::clone(&env)) {
                        evaluated_args.push(val);
                    }
                }
            }

            return target_val.call_method(method_name, &evaluated_args, line);
        }
    }

    if cleaned.starts_with("(list ") && cleaned.ends_with(')') {
        let inner = &cleaned[6..cleaned.len() - 1];
        let mut elements = Vec::new();
        if !inner.trim().is_empty() {
            let mut current_elements = inner.to_string();
            while let Some((left, right)) = split_binary_args(&current_elements) {
                if let Some(val) = evaluate_str(left, line, Rc::clone(&env)) {
                    elements.push(val);
                }
                current_elements = right;
            }
            if !current_elements.trim().is_empty() {
                if let Some(val) = evaluate_str(current_elements.trim().to_string(), line, Rc::clone(&env)) {
                    elements.push(val);
                }
            }
        }
        return Some(HyperValue::List(elements));
    }

    if cleaned.starts_with("(dict ") && cleaned.ends_with(')') {
        let entries = HashMap::new();
        return Some(HyperValue::Dict {
            key_type: "string".to_string(),
            val_type: "any".to_string(),
            entries,
        });
    }

    if cleaned.starts_with("(assign ") && cleaned.ends_with(')') {
        let inner = &cleaned[8..cleaned.len() - 1];
        if let Some(space_idx) = inner.find(' ') {
            let let_name = &inner[..space_idx];
            let value_expr = &inner[space_idx + 1..];
            if let Some(value) = evaluate_str(value_expr.to_string(), line, Rc::clone(&env)) {
                env.borrow_mut().assign(let_name, value.clone(), line);
                return Some(value);
            }
        }
    }

    let is_or = cleaned.starts_with("(or ");
    let is_and = cleaned.starts_with("(and ");
    if (is_or || is_and) && cleaned.ends_with(')') {
        let offset = if is_or { 4 } else { 5 };
        let inner = &cleaned[offset..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            if let Some(left_val) = evaluate_str(left_str, line, Rc::clone(&env)) {
                let left_truthy = is_truthy(&left_val);
                if (is_or && left_truthy) || (is_and && !left_truthy) { return Some(left_val); }
                return evaluate_str(right_str, line, Rc::clone(&env));
            }
        }
    }

    if cleaned.starts_with("(call ") && cleaned.ends_with(')') {
        let inner = cleaned[6..cleaned.len() - 1].trim();
        let (callee_str, args_str) = split_binary_args(inner)
            .map(|(l, r)| (l, Some(r)))
            .unwrap_or_else(|| (inner.to_string(), None));

        if let Some(call_val) = evaluate_str(callee_str, line, Rc::clone(&env)) {
            match call_val {
                HyperValue::NativeFunction(name) if name == "input" => {
                    if let Some(args_raw) = args_str {
                        if !args_raw.trim().is_empty() {
                            if let Some(prompt_val) = evaluate_str(args_raw.trim().to_string(), line, Rc::clone(&env)) {
                                println!("{}", prompt_val);
                                let _ = io::stdout().flush();
                            }
                        }
                    }

                    let mut input_buffer = String::new();
                    if io::stdin().read_line(&mut input_buffer).is_ok() {
                        let trimmed = input_buffer.trim_end_matches(&['\r', '\n'][..]).to_string();
                        return Some(HyperValue::String(trimmed));
                    } else {
                        eprintln!("[line {}] Error: Failed to read  line from stdin.", line);
                        std::process::exit(70);
                    }
                }
                HyperValue::NativeFunction(name) if name == "clock" => {
                    if args_str.as_ref().map_or(false, |s| !s.trim().is_empty()) {
                        eprintln!("Expected 0 arguments but got more.\n[line {}]", line);
                        std::process::exit(70);
                    }
                    let duration = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap();
                    return Some(HyperValue::F64(duration.as_secs_f64()));
                }
                HyperValue::Function { params, body, closure, .. } => {
                    let mut evaluated_args = Vec::new();
                    if let Some(args_raw) = args_str {
                        let mut current_args = args_raw;
                        while let Some((arg, rest)) = split_binary_args(&current_args) {
                            if let Some(val) = evaluate_str(arg, line, Rc::clone(&env)) { evaluated_args.push(val); }
                            current_args = rest;
                        }
                        if !current_args.trim().is_empty() {
                            if let Some(val) = evaluate_str(current_args.trim().to_string(), line, Rc::clone(&env)) {
                                evaluated_args.push(val);
                            }
                        }
                    }

                    if evaluated_args.len() != params.len() {
                        eprintln!("Expected {} arguments but got {}.\n[line {}]", params.len(), evaluated_args.len(), line);
                        std::process::exit(70);
                    }

                    let closure_env = Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&closure))));
                    for (param_name, arg_value) in params.iter().zip(evaluated_args) {
                        closure_env.borrow_mut().define(param_name.clone(), arg_value, false);
                    }

                    match execute_statement(&body, closure_env) {
                        ExecResult::Return(val) => return Some(val),
                        ExecResult::Ok => return Some(HyperValue::None),
                    }
                }
                HyperValue::StructDef { name, fields, methods, .. } => {
                    let mut evaluated_args = Vec::new();
                    if let Some(args_raw) = args_str {
                        let mut current_args = args_raw;
                        while let Some((arg, rest)) = split_binary_args(&current_args) {
                            if let Some(val) = evaluate_str(arg, line, Rc::clone(&env)) { evaluated_args.push(val); }
                            current_args = rest;
                        }
                        if !current_args.trim().is_empty() {
                            if let Some(val) = evaluate_str(current_args.trim().to_string(), line, Rc::clone(&env)) {
                                evaluated_args.push(val);
                            }
                        }
                    }
                
                    let mut instance_fields_vec = Vec::new();
                    let mut field_indices = HashMap::new();
                    
                    for (idx, (f_name, _, _, _)) in fields.iter().enumerate() {
                        instance_fields_vec.push(HyperValue::None);
                        field_indices.insert(f_name.clone(), idx);
                    }
                
                    let instance = HyperValue::Instance {
                        struct_name: name.clone(),
                        fields: Rc::new(RefCell::new(instance_fields_vec)),
                        field_indices,
                        methods: methods.clone(),
                    };
                
                    if let Some(HyperValue::Function { params, body, closure, .. }) = methods.get("__init__") {
                        let init_env = Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&closure))));
                        init_env.borrow_mut().define("self".to_string(), instance.clone(), true);
                
                        for (param_name, arg_value) in params.iter().skip(1).zip(evaluated_args) {
                            init_env.borrow_mut().define(param_name.clone(), arg_value, true);
                        }
                
                        execute_statement(&body, init_env);
                    }
                    return Some(instance);
                }
                _ => {
                    eprintln!("Can only call functions and classes.\n[line {}]", line);
                    std::process::exit(70);
                }
            }
        }
        return None;
    }

    if cleaned.starts_with("(call_method ") && cleaned.ends_with(')') {
        let inner = &cleaned[13..cleaned.len() - 1];
        
        let parts: Vec<&str> = inner.splitn(3, ' ').collect();
        if parts.len() >= 2 {
            let var_name = parts[0];
            let method_name = parts[1];
            let args_raw = if parts.len() == 3 { parts[2] } else { "[]" };

            let target_val = env.borrow().get(var_name, line);

            let mut evaluated_args = Vec::new();
            let args_trimmed = args_raw.trim();
            if args_trimmed.starts_with('[') && args_trimmed.ends_with(']') && args_trimmed.len() > 2 {
                let args_inner = &args_trimmed[1..args_trimmed.len() - 1];
                let mut current_args = args_inner.to_string();
                
                while let Some((left, right)) = split_binary_args(&current_args) {
                    if let Some(val) = evaluate_str(left, line, Rc::clone(&env)) {
                        evaluated_args.push(val);
                    }
                    current_args = right;
                }
                if !current_args.trim().is_empty() {
                    if let Some(val) = evaluate_str(current_args.trim().to_string(), line, Rc::clone(&env)) {
                        evaluated_args.push(val);
                    }
                }
            }

            if let HyperValue::Instance { ref methods, .. } = target_val {
                if let Some(HyperValue::Function { params, body, closure, .. }) = methods.get(method_name) {
                    let method_env = Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&closure))));
                    method_env.borrow_mut().define("self".to_string(), target_val.clone(), true);

                    for (param_name, arg_value) in params.iter().skip(1).zip(evaluated_args.iter()) {
                        method_env.borrow_mut().define(param_name.clone(), arg_value.clone(), true);
                    }

                    match execute_statement(&body, method_env) {
                        ExecResult::Return(val) => return Some(val),
                        ExecResult::Ok => return Some(HyperValue::None),
                    }
                } else {
                    eprintln!("[line {}] Error: Method '{}' not found.", line, method_name);
                    std::process::exit(70);
                }
            }

            return target_val.call_method(method_name, &evaluated_args, line);
        }
    }

    if cleaned.ends_with(')') {
        if cleaned.starts_with("(+ ") { return eval_binary_op(&cleaned[3..cleaned.len() - 1], line, Rc::clone(&env), |a, b| a.add(b)); }
        if cleaned.starts_with("(* ") { return eval_binary_op(&cleaned[3..cleaned.len() - 1], line, Rc::clone(&env), |a, b| a.mul(b)); }
        if cleaned.starts_with("(/ ") { return eval_binary_op(&cleaned[3..cleaned.len() - 1], line, Rc::clone(&env), |a, b| a.div(b)); }
        if cleaned.starts_with("(% ") { return eval_binary_op(&cleaned[3..cleaned.len() - 1], line, Rc::clone(&env), |a, b| a.rem(b)); }
        if cleaned.starts_with("(** ") { return eval_binary_op(&cleaned[4..cleaned.len() - 1], line, Rc::clone(&env), |a, b| a.pow(b)); }
        if cleaned.starts_with("(> ") { return eval_binary_op(&cleaned[3..cleaned.len() - 1], line, Rc::clone(&env), |a, b| a.greater(b)); }
        if cleaned.starts_with("(< ") { return eval_binary_op(&cleaned[3..cleaned.len() - 1], line, Rc::clone(&env), |a, b| a.less(b)); }
        if cleaned.starts_with("(>= ") { return eval_binary_op(&cleaned[4..cleaned.len() - 1], line, Rc::clone(&env), |a, b| a.greater_equal(b)); }
        if cleaned.starts_with("(<= ") { return eval_binary_op(&cleaned[4..cleaned.len() - 1], line, Rc::clone(&env), |a, b| a.less_equal(b)); }

        if cleaned.starts_with("(- ") {
            let inner = &cleaned[3..cleaned.len() - 1];
            if let Some((left_str, right_str)) = split_binary_args(inner) {
                let left_val = evaluate_str(left_str, line, Rc::clone(&env))?;
                let right_val = evaluate_str(right_str, line, Rc::clone(&env))?;
                return left_val.sub(&right_val);
            } else {
                let val = evaluate_str(inner.trim().to_string(), line, Rc::clone(&env))?;
                return val.negate();
            }
        }

        if cleaned.starts_with("(not ") {
            let inner = cleaned[5..cleaned.len() - 1].to_string();
            let val = evaluate_str(inner, line, Rc::clone(&env))?;
            return Some(HyperValue::Boolean(!is_truthy(&val)));
        }

        if cleaned.starts_with("(== ") {
            let (left_str, right_str) = split_binary_args(&cleaned[4..cleaned.len() - 1])?;
            let l = evaluate_str(left_str, line, Rc::clone(&env))?;
            let r = evaluate_str(right_str, line, Rc::clone(&env))?;
            return Some(HyperValue::Boolean(l == r));
        }

        if cleaned.starts_with("(!= ") {
            let (left_str, right_str) = split_binary_args(&cleaned[4..cleaned.len() - 1])?;
            let l = evaluate_str(left_str, line, Rc::clone(&env))?;
            let r = evaluate_str(right_str, line, Rc::clone(&env))?;
            return Some(HyperValue::Boolean(l != r));
        }
    }

    match cleaned.as_str() {
        "true" => Some(HyperValue::Boolean(true)),
        "false" => Some(HyperValue::Boolean(false)),
        "None" => Some(HyperValue::None),
        _ => {
            if let Ok(num) = cleaned.parse::<i32>() {
                Some(HyperValue::I32(num))
            } else if let Ok(num) = cleaned.parse::<f64>() {
                Some(HyperValue::F64(num))
            } else if cleaned.starts_with('"') && cleaned.ends_with('"') {
                Some(HyperValue::String(cleaned[1..cleaned.len() - 1].to_string()))
            } else {
                None
            }
        }
    }
}

fn execute_statement(stmt: &str, env: Rc<RefCell<Environment>>) -> ExecResult {
    if stmt.starts_with("(let line:") {
        let trimmed = &stmt[10..stmt.len() - 1];
        let parts: Vec<&str> = trimmed.splitn(4, ' ').collect();
        if parts.len() == 4 {
            let line_num: u32 = parts[0].parse().unwrap();
            let is_mutable = parts[1] == "mut";
            let let_name = parts[2].to_string();
            let initializer_expr = parts[3].to_string();

            let value = if initializer_expr == "None" {
                HyperValue::None
            } else {
                evaluate_str(initializer_expr, line_num, Rc::clone(&env)).unwrap_or(HyperValue::None)
            };
            env.borrow_mut().define(let_name, value, is_mutable);
        }
    } else if stmt.starts_with("(expr line:") {
        let rest = &stmt[11..];
        let space_idx = rest.find(' ').unwrap();
        let line_num: u32 = rest[..space_idx].parse().unwrap();
        evaluate_str(rest[space_idx + 1..rest.len() - 1].to_string(), line_num, Rc::clone(&env));
    } else if stmt.starts_with("(print line:") {
        let rest = &stmt[12..];
        let space_idx = rest.find(' ').unwrap();
        let line_num: u32 = rest[..space_idx].parse().unwrap();
        let exprs_str = rest[space_idx + 1..rest.len() - 1].to_string();
        let mut evaluated_results = Vec::new();

        if exprs_str.contains(' ') {
            let mut current_exprs = exprs_str;
            while let Some((left, right)) = split_binary_args(&current_exprs) {
                if let Some(val) = evaluate_str(left, line_num, Rc::clone(&env)) {
                    evaluated_results.push(val.to_string());
                }
                current_exprs = right;
            }
            if !current_exprs.trim().is_empty() {
                if let Some(val) = evaluate_str(current_exprs.trim().to_string(), line_num, Rc::clone(&env)) {
                    evaluated_results.push(val.to_string());
                }
            }
        } else {
            if let Some(val) = evaluate_str(exprs_str, line_num, Rc::clone(&env)) {
                evaluated_results.push(val.to_string());
            }
        }

        println!("{}", evaluated_results.join(" "));
    } else if stmt.starts_with("(block ") && stmt.ends_with(')') {
        let inner = &stmt[7..stmt.len() - 1];
        let block_env = Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&env))));
        for sub_stmt in split_block_statements(inner) {
            if let ExecResult::Return(val) = execute_statement(&sub_stmt, Rc::clone(&block_env)) {
                return ExecResult::Return(val);
            }
        }
    } else if stmt.starts_with("(struct ") {
        let trimmed = &stmt[8..stmt.len() - 1];
        let space_idx = trimmed.find(' ').unwrap();
        let struct_name = trimmed[..space_idx].to_string();
        let rest = &trimmed[space_idx + 1..];

        let trait_start = rest.find("trait:").unwrap() + 6;
        let trait_end = rest.find(" fields:[").unwrap();
        let implemented_trait = rest[trait_start..trait_end].to_string();

        if !implemented_trait.is_empty() {
            let trait_check = env.borrow().get(&implemented_trait, 0);
            if !matches!(trait_check, HyperValue::TraitDef { .. }) {
                eprintln!("Error: Trait '{}' is not defined.", implemented_trait);
                std::process::exit(70);
            }
        }

        let fields_start = rest.find("fields:[").unwrap() + 8;
        let fields_end = rest.find("] methods:[").unwrap();
        let fields_str = &rest[fields_start..fields_end];

        let mut fields = Vec::new();
        if !fields_str.trim().is_empty() {
            for (idx, f) in fields_str.split(", ").enumerate() {
                fields.push((f.to_string(), "any".to_string(), false, idx));
            }
        }

        let methods_start = rest.find("methods:[").unwrap() + 9;
        let methods_end = rest.len() - 1;
        let methods_str = &rest[methods_start..methods_end];

        let mut methods_map = HashMap::new();
        if !methods_str.trim().is_empty() {
            for m_ast in split_block_statements(methods_str) {
                if m_ast.starts_with("(fn ") {
                    let m_trimmed = &m_ast[4..m_ast.len() - 1];
                    let m_space = m_trimmed.find(' ').unwrap();
                    let m_name = m_trimmed[..m_space].to_string();
                    
                    let rest_m = &m_trimmed[m_space + 1..];
                    let is_strict = rest_m.starts_with("strict:true");
                    let p_start = rest_m.find("(params ").unwrap() + 8;
                    let p_end = rest_m.find(')').unwrap();
                    let params = rest_m[p_start..p_end].split_whitespace().map(|s| s.to_string()).collect();
                    let body_str = rest_m[p_end + 2..].to_string();

                    let func_val = HyperValue::Function {
                        name: m_name.clone(),
                        params,
                        body: body_str,
                        is_strict,
                        closure: Rc::clone(&env),
                    };
                    methods_map.insert(m_name, func_val);
                }
            }
        }

        let struct_def = HyperValue::StructDef {
            name: struct_name.clone(),
            implemented_trait,
            fields,
            methods: methods_map,
        };

        env.borrow_mut().define(struct_name, struct_def, false);
    } else if stmt.starts_with("(trait ") {
        let trimmed = &stmt[7..stmt.len() - 1];
        let space_idx = trimmed.find(' ').unwrap();
        let trait_name = trimmed[..space_idx].to_string();
        
        let trait_def = HyperValue::TraitDef {
            name: trait_name.clone(),
            methods: vec![],
        };
        env.borrow_mut().define(trait_name, trait_def, false);

    } else if stmt.starts_with("(if ") && stmt.ends_with(')') {
        let inner = &stmt[4..stmt.len() - 1];
        if let Some((cond_str, rest)) = split_binary_args(inner) {
            if let Some(cond_val) = evaluate_str(cond_str, 1, Rc::clone(&env)) {
                let target = if is_truthy(&cond_val) {
                    split_binary_args(&rest).map(|(then_s, _)| then_s).unwrap_or(rest.clone())
                } else {
                    split_binary_args(&rest).map(|(_, else_s)| else_s).unwrap_or_default()
                };
    
                if !target.trim().is_empty() {
                    let res = execute_statement(&target, Rc::clone(&env));
                    if matches!(res, ExecResult::Return(_)) {
                        return res;
                    }
                }
            }
        }
    } else if stmt.starts_with("(while ") && stmt.ends_with(')') {
        let inner = &stmt[7..stmt.len() - 1];
        if let Some((cond_str, body_str)) = split_binary_args(inner) {
            while let Some(cond_val) = evaluate_str(cond_str.clone(), 1, Rc::clone(&env)) {
                if is_truthy(&cond_val) {
                    if let ExecResult::Return(val) = execute_statement(&body_str, Rc::clone(&env)) {
                        return ExecResult::Return(val);
                    }
                } else { break; }
            }
        }
    } else if stmt.starts_with("(for_seq line:") || stmt.starts_with("(for_par line:") || stmt.starts_with("(for_vec line:") || stmt.starts_with("(for_par_vec line:") {
        let is_parallel = stmt.starts_with("(for_par ") || stmt.starts_with("(for_par_vec ");
        let is_vectorized = stmt.starts_with("(for_vec ") || stmt.starts_with("(for_par_vec ");

        let tag_len = if stmt.starts_with("(for_seq line:") { 14 }
                      else if stmt.starts_with("(for_par line:") { 14 }
                      else if stmt.starts_with("(for_vec line:") { 14 }
                      else { 18 };

        let rest = &stmt[tag_len..stmt.len() - 1];
        let parts: Vec<&str> = rest.splitn(5, ' ').collect();
        if parts.len() == 5 {
            let line_num: u32 = parts[0].parse().unwrap_or(1);
            let var_name = parts[1].to_string();
            let start_expr = parts[2].to_string();
            let end_expr = parts[3].to_string();
            let body_str = parts[4].to_string();

            let parse_to_i64 = |expr: &str| -> i64 {
                match evaluate_str(expr.to_string(), line_num, Rc::clone(&env)) {
                    Some(HyperValue::I64(val)) => val,
                    Some(HyperValue::F64(val)) => val as i64,
                    Some(HyperValue::I32(val)) => val as i64,
                     _ => 0,
                }
            }; 
            
            let start_val = parse_to_i64(&start_expr);
            let end_val = parse_to_i64(&end_expr);

            if is_parallel && is_vectorized {
                let num_threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
                let total_items = (end_val - start_val).max(0);
                if total_items > 0 {
                    let chunk_size = (total_items + num_threads as i64 - 1) / num_threads as i64;
                    
                    std::thread::scope(|s| {
                        for t in 0..num_threads {
                            let t_start = start_val + t as i64 * chunk_size;
                            let t_end = (t_start + chunk_size).min(end_val);
                            if t_start >= end_val { break; }
                            
                            let var_n = var_name.clone();
                            let b_str = body_str.clone();

                            s.spawn(move || {
                                let thread_local_env = Environment::new();         
                                let thread_local_rc = Rc::new(RefCell::new(thread_local_env));
                                let loop_env = Rc::new(RefCell::new(Environment::new_with_enclosing(thread_local_rc)));
            
                                for i in t_start..t_end {
                                    loop_env.borrow_mut().define(var_n.clone(), HyperValue::I64(i), true);
                                    execute_statement(&b_str, Rc::clone(&loop_env));
                                }
                            });
                        }
                    });
                }
            } else if is_vectorized {
                for i in (start_val..end_val).step_by(1) {
                    let loop_env = Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&env))));
                    loop_env.borrow_mut().define(var_name.clone(), HyperValue::I64(i), true);
                    if let ExecResult::Return(val) = execute_statement(&body_str, loop_env) {
                        return ExecResult::Return(val);
                    }
                }
            } else {
                for i in start_val..end_val {
                    let loop_env = Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&env))));
                    loop_env.borrow_mut().define(var_name.clone(), HyperValue::I64(i), true);
                    if let ExecResult::Return(val) = execute_statement(&body_str, loop_env) {
                        return ExecResult::Return(val);
                    }
                } 
            }
        }
    } else if stmt.starts_with("(fn ") {
        let trimmed = &stmt[5..stmt.len() - 1];
        let space_idx = trimmed.find(' ').unwrap();
        let func_name = trimmed[..space_idx].to_string();
        let rest = &trimmed[space_idx + 1..];

        let is_strict = rest.starts_with("strict:true");
 
        let params_start = rest.find("(params ").unwrap() + 8;
        let params_end = rest.find(')').unwrap();
        let params = rest[params_start..params_end].split_whitespace().map(|s| s.to_string()).collect();
        let body_str = rest[params_end + 2..].to_string();

        env.borrow_mut().define(
            func_name.clone(),
            HyperValue::Function { 
                name: func_name, 
                params, 
                body: body_str, 
                is_strict, 
                closure: Rc::clone(&env) 
            },
            false,
        );
    } else if stmt.starts_with("(return line:") {
        let rest = &stmt[13..stmt.len() - 1];
        let space_idx = rest.find(' ').unwrap();
        let line_num: u32 = rest[..space_idx].parse().unwrap();
        let inner_expr = &rest[space_idx + 1..];

        let value = if inner_expr == "None" {
            HyperValue::None
        } else {
            evaluate_str(inner_expr.to_string(), line_num, Rc::clone(&env)).unwrap_or(HyperValue::None)
        };
        return ExecResult::Return(value);
    } else if stmt.starts_with("(with_mmap line:") {
        let rest = &stmt[16..stmt.len() - 1];
        let parts: Vec<&str> = rest.splitn(3, ' ').collect();
        if parts.len() == 3 {
            let line_num: u32 = parts[0].parse().unwrap();
            let path_expr = parts[1].to_string();
            let rest_inner = parts[2];
    
            let path_val = evaluate_str(path_expr, line_num, Rc::clone(&env)).unwrap_or(HyperValue::None);
            let file_path = match path_val {
                HyperValue::String(s) => s,
                _ => "".to_string(),
            };
    
            let space_idx = rest_inner.find(' ').unwrap();
            let var_name = rest_inner[..space_idx].to_string();
            let body_str = rest_inner[space_idx + 1..].to_string();
    
            if let Ok(file) = File::open(&file_path) {
                let mmap_val = HyperValue::MmapFile {
                    file: Rc::new(RefCell::new(file)),
                    path: file_path,
                };
                let block_env = Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&env))));
                block_env.borrow_mut().define(var_name, mmap_val, false);
                execute_statement(&body_str, block_env);
            } else {
                eprintln!("[line {}] Error: Could not open file '{}'", line_num, file_path);
            }
        }
    }

    ExecResult::Ok
}

pub fn run_evaluate(file_contents: String) {
    let (tokens, error) = crate::scanner::scan_tokens(&file_contents);
    if error { std::process::exit(65); }

    let mut parser = crate::parser::Parser::new(tokens);
    let env = Rc::new(RefCell::new(Environment::new()));

    env.borrow_mut().define("input".to_string(), HyperValue::NativeFunction("input".to_string()), false);
    env.borrow_mut().define("clock".to_string(), HyperValue::NativeFunction("clock".to_string()), false);

    match parser.parse() {
        Ok(ast_string) => {
            if let Some(result) = evaluate_str(ast_string, 1, Rc::clone(&env)) {
                println!("{}", result);
            } else {
                std::process::exit(65);
            }
        }
        Err(_) => { std::process::exit(65); }
    }
}

pub fn run_program(file_contents: String) {
    let (tokens, error) = crate::scanner::scan_tokens(&file_contents);
    if error { std::process::exit(65); }

    let mut parser = crate::parser::Parser::new(tokens);
    let env = Rc::new(RefCell::new(Environment::new()));
    env.borrow_mut().define("clock".to_string(), HyperValue::NativeFunction("clock".to_string()), false);

    env.borrow_mut().define("input".to_string(), HyperValue::NativeFunction("input".to_string()), false);
    env.borrow_mut().define("clock".to_string(), HyperValue::NativeFunction("clock".to_string()), false);

    match parser.parse_statements() {
        Ok(statements) => {
            for stmt in statements {
                execute_statement(&stmt, Rc::clone(&env));
            }
        }
        Err(_) => {
            eprintln!("[line 1] Syntax error in function declaration.");
            std::process::exit(65);
        }
    }
}