use std::cell::RefCell;
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
                _ => {
                    eprintln!("Can only call functions and classes.\n[line {}]", line);
                    std::process::exit(70);
                }
            }
        }
        return None;
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
                Some(HyperValue::StringLit(cleaned[1..cleaned.len() - 1].to_string()))
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
    } else if stmt.starts_with("(if ") && stmt.ends_with(')') {
        let inner = &stmt[4..stmt.len() - 1];
        if let Some((cond_str, rest)) = split_binary_args(inner) {
            if let Some(cond_val) = evaluate_str(cond_str, 1, Rc::clone(&env)) {
                if let Some((then_str, else_str)) = split_binary_args(&rest) {
                    let target = if is_truthy(&cond_val) { then_str } else { else_str };
                    if let ExecResult::Return(val) = execute_statement(&target, Rc::clone(&env)) {
                        return ExecResult::Return(val);
                    }
                } else if is_truthy(&cond_val) {
                    if let ExecResult::Return(val) = execute_statement(&rest, Rc::clone(&env)) {
                        return ExecResult::Return(val);
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
    } else if stmt.starts_with("(fun ") {
        let trimmed = &stmt[5..stmt.len() - 1];
        let space_idx = trimmed.find(' ').unwrap();
        let func_name = trimmed[..space_idx].to_string();
        let rest = &trimmed[space_idx + 1..];

        let params_start = rest.find("(params ").unwrap() + 8;
        let params_end = rest.find(')').unwrap();
        let params = rest[params_start..params_end].split_whitespace().map(|s| s.to_string()).collect();
        let body_str = rest[params_end + 2..].to_string();

        env.borrow_mut().define(
            func_name.clone(),
            HyperValue::Function { name: func_name, params, body: body_str, closure: Rc::clone(&env) },
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
    }

    ExecResult::Ok
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