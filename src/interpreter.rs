use std::cell::RefCell;
use std::rc::Rc;
use crate::environment::{Environment, HyperValue};

fn clean_group_expressions(mut input: String) -> String {
    while input.starts_with("(group ") && input.ends_with(')') {
        input = input[7..input.len() - 1].to_string();
    }
    input
}

fn is_truthy(value: &HyperValue) -> bool {
    match value {
        HyperValue::Nil => false,
        HyperValue::Boolean(b) => *b,
        _ => true,
    }
}

fn split_binary_args(inner: &str) -> Option<(String, String)> {
    let mut bracket_count = 0;
    let mut split_idx = None;
    let chars: Vec<char> = inner.chars().collect();

    for (i, &ch) in chars.iter().enumerate() {
        if ch == '(' {
            bracket_count += 1;
        } else if ch == ')' {
            bracket_count -= 1;
        } else if ch == ' ' && bracket_count == 0 {
            split_idx = Some(i);
            break;  
        }
    }

    if let Some(idx) = split_idx {
        let left = chars[..idx].iter().collect::<String>();
        let right = chars[idx + 1..].iter().collect::<String>();
        Some((left, right))
    } else {
        None
    }
}

fn split_block_statements(inner: &str) -> Vec<String> {
    let mut stmts = Vec::new();
    let mut current = String::new();
    let mut bracket_count = 0;

    for ch in inner.chars() {
        if ch == '(' {
            bracket_count += 1;
        } else if ch == ')' {
            bracket_count -= 1;
        }

        current.push(ch);

        if bracket_count == 0 {
            let trimmed = current.trim();
            if !trimmed.is_empty() {
                stmts.push(trimmed.to_string());
            }
            current.clear();
        }
    }
    stmts
}

fn evaluate_str(ast_string: String, line: u32, env: Rc<RefCell<Environment>>) -> Option<HyperValue> {
    let cleaned = clean_group_expressions(ast_string);

    if cleaned.starts_with("var_ref:") {
        let var_name = &cleaned[8..];
        return Some(env.borrow().get(var_name, line));
    }

    if cleaned.starts_with("(assign ") && cleaned.ends_with(')') {
        let inner = &cleaned[8..cleaned.len() - 1];
        if let Some(space_idx) = inner.find(' ') {
            let var_name = &inner[..space_idx];
            let value_expr = &inner[space_idx + 1..];
    
            if let Some(value) = evaluate_str(value_expr.to_string(), line, Rc::clone(&env)) {
                env.borrow_mut().assign(var_name, value.clone(), line);
                return Some(value);
            }
        }
    }

    let is_or = cleaned.starts_with("(or ");
    let is_and = cleaned.starts_with("(and ");

    if (is_or || is_and) && cleaned.ends_with(')') {
        let offset = if is_or {4} else {5};
        let inner = &cleaned[offset..cleaned.len() - 1];

        if let Some((left_str, right_str)) = split_binary_args(inner) {
            if let Some(left_val) = evaluate_str(left_str, line, Rc::clone(&env)) {
                let left_truthy = is_truthy(&left_val);

                if (is_or && left_truthy) || (is_and && !left_truthy) {
                    return Some(left_val);
                }

                return evaluate_str(right_str, line, Rc::clone(&env));
            }
        }
    }

    if cleaned.starts_with("(call ") && cleaned.ends_with(')') {
        let inner = cleaned[6..cleaned.len() - 1].trim();

        // Funksiya sarlavhasi (callee) va uning argumentlarini ajratamiz
        let (callee_str, args_str) = if let Some((left, right)) = split_binary_args(inner) {
            (left, Some(right))
        } else {
            (inner.to_string(), None)
        };

        // inner.to_string() o'rniga callee_str berildi! 🌟
        if let Some(call_val) = evaluate_str(callee_str, line, Rc::clone(&env)) {
            match call_val {
                HyperValue::NativeFunction(name) => {
                    if name == "clock" {
                        let duration = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap();
                        return Some(HyperValue::Number(duration.as_secs_f64()));
                    }
                }
                HyperValue::Function { name: _, params, body } => {
                    let mut evaluated_args = Vec::new();
                    if let Some(args_raw) = args_str {
                        let mut current_args = args_raw;
                        while let Some((arg, rest)) = split_binary_args(&current_args) {
                            if let Some(val) = evaluate_str(arg, line, Rc::clone(&env)) {
                                evaluated_args.push(val);
                            }
                            current_args = rest;
                        }

                        if !current_args.trim().is_empty() {
                            if let Some(val) = evaluate_str(current_args.trim().to_string(), line, Rc::clone(&env)) {
                                evaluated_args.push(val);
                            }
                        }
                    }

                    // Funksiya chaqirilganda yangi yopiq muhit (scope) yaratiladi
                    let closure_env = Rc::new(std::cell::RefCell::new(Environment::new_with_enclosing(Rc::clone(&env))));

                    for (param_name, arg_value) in params.iter().zip(evaluated_args.iter()) {
                        closure_env.borrow_mut().define(param_name.clone(), arg_value.clone());
                    }
                    execute_statement(&body, closure_env);
                    
                    return Some(HyperValue::Nil);
                }
                _=> {
                    eprintln!("Can only call functions and classes.");
                    std::process::exit(70);
                }
            }
        }
        return None;
    }

    if cleaned.starts_with("(+ ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(HyperValue::Number(l)), Some(HyperValue::Number(r))) => return Some(HyperValue::Number(l + r)),
                (Some(HyperValue::StringLit(l)), Some(HyperValue::StringLit(r))) => {
                    return Some(HyperValue::StringLit(format!("{}{}", l, r)));
                }
                _ => {
                    eprintln!("Operands must be two numbers or two strings.");
                    std::process::exit(70);
                }
            }
        }
    }

    if cleaned.starts_with("(- ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(HyperValue::Number(l)), Some(HyperValue::Number(r))) => return Some(HyperValue::Number(l - r)),
                _ => {
                    eprintln!("Operand must be numbers.");
                    std::process::exit(70);
                }
            }
        } else {
            if let Some(val) = evaluate_str(inner.trim().to_string(), line, Rc::clone(&env)) {
                match val {
                    HyperValue::Number(n) => return Some(HyperValue::Number(-n)),
                    _ => {
                        eprintln!("Operand must be a number.");
                        std::process::exit(70);
                    }
                }
            }
        }
    }

    if cleaned.starts_with("(! ") && cleaned.ends_with(')') {
        let inner = cleaned[3..cleaned.len() - 1].to_string();
        if let Some(val) = evaluate_str(inner, line, Rc::clone(&env)) {
            return Some(HyperValue::Boolean(!is_truthy(&val)));
        }
    }

    if cleaned.starts_with("(* ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(HyperValue::Number(l)), Some(HyperValue::Number(r))) => return Some(HyperValue::Number(l * r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(/ ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(HyperValue::Number(l)), Some(HyperValue::Number(r))) => return Some(HyperValue::Number(l / r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(> ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(HyperValue::Number(l)), Some(HyperValue::Number(r))) => return Some(HyperValue::Boolean(l > r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(< ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(HyperValue::Number(l)), Some(HyperValue::Number(r))) => return Some(HyperValue::Boolean(l < r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(>= ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(HyperValue::Number(l)), Some(HyperValue::Number(r))) => return Some(HyperValue::Boolean(l >= r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(<= ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(HyperValue::Number(l)), Some(HyperValue::Number(r))) => return Some(HyperValue::Boolean(l <= r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(== ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(HyperValue::Number(l)), Some(HyperValue::Number(r))) => return Some(HyperValue::Boolean(l == r)),
                (Some(HyperValue::StringLit(l)), Some(HyperValue::StringLit(r))) => return Some(HyperValue::Boolean(l == r)),
                (Some(HyperValue::Boolean(l)), Some(HyperValue::Boolean(r))) => return Some(HyperValue::Boolean(l == r)),
                (Some(HyperValue::Nil), Some(HyperValue::Nil)) => return Some(HyperValue::Boolean(true)),
                (Some(_), Some(_)) => return Some(HyperValue::Boolean(false)),
                _ => return None,
            }
        }
    }

    if cleaned.starts_with("(!= ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(HyperValue::Number(l)), Some(HyperValue::Number(r))) => return Some(HyperValue::Boolean(l != r)),
                (Some(HyperValue::StringLit(l)), Some(HyperValue::StringLit(r))) => return Some(HyperValue::Boolean(l != r)),
                (Some(HyperValue::Boolean(l)), Some(HyperValue::Boolean(r))) => return Some(HyperValue::Boolean(l != r)),
                (Some(HyperValue::Nil), Some(HyperValue::Nil)) => return Some(HyperValue::Boolean(false)),
                (Some(_), Some(_)) => return Some(HyperValue::Boolean(true)),
                _ => return None,
            }
        }
    }

    match cleaned.as_str() {
        "true" => Some(HyperValue::Boolean(true)),
        "false" => Some(HyperValue::Boolean(false)),
        "nil" => Some(HyperValue::Nil),
        _ => {
            if let Ok(num) = cleaned.parse::<f64>() {
                Some(HyperValue::Number(num))
            } else if cleaned.starts_with('"') && cleaned.ends_with('"') {
                let clean_str = &cleaned[1..cleaned.len() - 1];
                Some(HyperValue::StringLit(clean_str.to_string()))
            } else {
                None
            }
        }
    }
}

fn execute_statement(stmt: &str, env: Rc<RefCell<Environment>>) {
    if stmt.starts_with("(var line:") {
        let trimmed = &stmt[10..&stmt.len() - 1];
        let parts: Vec<&str> = trimmed.splitn(3, ' ').collect();

        if parts.len() == 3 {
            let line_num: u32 = parts[0].parse().unwrap();
            let var_name = parts[1].to_string();
            let initializer_expr = parts[2].to_string();

            let value = if initializer_expr == "nil" {
                HyperValue::Nil
            } else {
                match evaluate_str(initializer_expr, line_num, Rc::clone(&env)) {
                    Some(val) => val,
                    None => HyperValue::Nil,
                }
            };

            env.borrow_mut().define(var_name, value);
        }
    } else if stmt.starts_with("(expr line:") {
        let rest = &stmt[11..];
        let space_idx = rest.find(' ').unwrap();
        let line_num: u32 = rest[..space_idx].parse().unwrap();
        let inner_expr = &rest[space_idx + 1..rest.len() - 1];

        evaluate_str(inner_expr.to_string(), line_num, Rc::clone(&env));
    } else if stmt.starts_with("(print line:") {
        let rest = &stmt[12..];
        let space_idx = rest.find(' ').unwrap();
        let line_num: u32 = rest[..space_idx].parse().unwrap();
        let inner_expr = &rest[space_idx + 1..rest.len() - 1];

        if let Some(result) = evaluate_str(inner_expr.to_string(), line_num, Rc::clone(&env)) {
            println!("{}", result);
        } else {
            std::process::exit(70);
        }
    } else if stmt.starts_with("(block ") && stmt.ends_with(')') {
        let inner = &stmt[7..stmt.len() - 1];
        let sub_statements = split_block_statements(inner);

        let block_env = Rc::new(RefCell::new(Environment::new_with_enclosing(Rc::clone(&env))));

        for sub_stmt in sub_statements {
            execute_statement(&sub_stmt, Rc::clone(&block_env));
        }
    } else if stmt.starts_with("(if ") && stmt.ends_with(')') {
        let inner = &stmt[4 ..stmt.len() - 1];

        if let Some((cond_str, rest)) = split_binary_args(inner) {
            if let Some(cond_val) = evaluate_str(cond_str, 1, Rc::clone(&env)) {
                if let Some((then_str, else_str)) = split_binary_args(&rest) {
                    if is_truthy(&cond_val) {
                        execute_statement(&then_str, Rc::clone(&env));
                    } else {
                        execute_statement(&else_str, Rc::clone(&env));
                    }
                } else {
                    if is_truthy(&cond_val) {
                        execute_statement(&rest, Rc::clone(&env));
                    }
                }
            }
        }
    } else if stmt.starts_with("(while ") && stmt.ends_with(')') {
        let inner = &stmt[7 ..stmt.len() - 1];

        if let Some((cond_str, body_str)) = split_binary_args(inner) {
            loop {
                if let Some(cond_val) = evaluate_str(cond_str.clone(), 1, Rc::clone(&env)) {
                    if is_truthy(&cond_val) {
                        execute_statement(&body_str, Rc::clone(&env));
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
        }
    } else if stmt.starts_with("(fun ") {
        let trimmed = &stmt[5..stmt.len() - 1];
        let space_idx = trimmed.find(' ').unwrap();
        let func_name = trimmed[..space_idx].to_string();
        let rest = &trimmed[&space_idx + 1..];

        let params_start = rest.find("(params ").unwrap() + 8;
        let params_end = rest.find(')').unwrap();
        let params_str = &rest[params_start..params_end];
        let params: Vec<String> = params_str
            .split_whitespace()
            .map(|s| s.to_string())
            .collect();

        let body_str = rest[params_end + 2..].to_string();

        env.borrow_mut().define(
            func_name.clone(),
            HyperValue::Function { name: func_name, params , body: body_str }
        );
    }
}

pub fn run_evaluate(file_contents: String) {
    let (tokens, error) = crate::scanner::scan_tokens(&file_contents);
    if error { std::process::exit(65); }

    let mut parser = crate::parser::Parser::new(tokens);
    let env = Rc::new(RefCell::new(Environment::new()));

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
    env.borrow_mut().define("clock".to_string(), HyperValue::NativeFunction("clock".to_string()));

    match parser.parse_statements() {
        Ok(statements) => {
            for stmt in statements {
                execute_statement(&stmt, Rc::clone(&env));
            }
        }
        Err(_) => { std::process::exit(65); }
    }
}