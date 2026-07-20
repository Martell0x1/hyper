use std::cell::RefCell;
use std::rc::Rc;

use crate::environment::{Environment, LoxValue};

fn clean_group_expressions(mut input: String) -> String {
    while input.starts_with("(group ") && input.ends_with(')') {
        input = input[7..input.len() - 1].to_string();
    }
    input
}

fn is_truthy(value: &LoxValue) -> bool {
    match value {
        LoxValue::Nil => false,
        LoxValue::Boolean(b) => *b,
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

fn evaluate_str(ast_string: String, line: u32, env: Rc<RefCell<Environment>>) -> Option<LoxValue> {
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

    if cleaned.starts_with("(+ ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => return Some(LoxValue::Number(l + r)),
                (Some(LoxValue::StringLit(l)), Some(LoxValue::StringLit(r))) => {
                    return Some(LoxValue::StringLit(format!("{}{}", l, r)));
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
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => return Some(LoxValue::Number(l - r)),
                _ => {
                    eprintln!("Operand must be numbers.");
                    std::process::exit(70);
                }
            }
        } else {
            if let Some(val) = evaluate_str(inner.trim().to_string(), line, Rc::clone(&env)) {
                match val {
                    LoxValue::Number(n) => return Some(LoxValue::Number(-n)),
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
            return Some(LoxValue::Boolean(!is_truthy(&val)));
        }
    }

    if cleaned.starts_with("(* ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => return Some(LoxValue::Number(l * r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(/ ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => return Some(LoxValue::Number(l / r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(> ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => return Some(LoxValue::Boolean(l > r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(< ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => return Some(LoxValue::Boolean(l < r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(>= ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => return Some(LoxValue::Boolean(l >= r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(<= ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => return Some(LoxValue::Boolean(l <= r)),
                _ => { eprintln!("Operand must be numbers."); std::process::exit(70); }
            }
        }
    }

    if cleaned.starts_with("(== ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => return Some(LoxValue::Boolean(l == r)),
                (Some(LoxValue::StringLit(l)), Some(LoxValue::StringLit(r))) => return Some(LoxValue::Boolean(l == r)),
                (Some(LoxValue::Boolean(l)), Some(LoxValue::Boolean(r))) => return Some(LoxValue::Boolean(l == r)),
                (Some(LoxValue::Nil), Some(LoxValue::Nil)) => return Some(LoxValue::Boolean(true)),
                (Some(_), Some(_)) => return Some(LoxValue::Boolean(false)),
                _ => return None,
            }
        }
    }

    if cleaned.starts_with("(!= ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, Rc::clone(&env)), evaluate_str(right_str, line, Rc::clone(&env))) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => return Some(LoxValue::Boolean(l != r)),
                (Some(LoxValue::StringLit(l)), Some(LoxValue::StringLit(r))) => return Some(LoxValue::Boolean(l != r)),
                (Some(LoxValue::Boolean(l)), Some(LoxValue::Boolean(r))) => return Some(LoxValue::Boolean(l != r)),
                (Some(LoxValue::Nil), Some(LoxValue::Nil)) => return Some(LoxValue::Boolean(false)),
                (Some(_), Some(_)) => return Some(LoxValue::Boolean(true)),
                _ => return None,
            }
        }
    }

    match cleaned.as_str() {
        "true" => Some(LoxValue::Boolean(true)),
        "false" => Some(LoxValue::Boolean(false)),
        "nil" => Some(LoxValue::Nil),
        _ => {
            if let Ok(num) = cleaned.parse::<f64>() {
                Some(LoxValue::Number(num))
            } else if cleaned.starts_with('"') && cleaned.ends_with('"') {
                let clean_str = &cleaned[1..cleaned.len() - 1];
                Some(LoxValue::StringLit(clean_str.to_string()))
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
                LoxValue::Nil
            } else {
                match evaluate_str(initializer_expr, line_num, Rc::clone(&env)) {
                    Some(val) => val,
                    None => LoxValue::Nil,
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

    match parser.parse_statements() {
        Ok(statements) => {
            for stmt in statements {
                execute_statement(&stmt, Rc::clone(&env));
            }
        }
        Err(_) => { std::process::exit(65); }
    }
}