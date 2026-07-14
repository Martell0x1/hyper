use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum LoxValue {
    Boolean(bool),
    Nil,
    Number(f64),
    StringLit(String),
}

pub struct Environment {
    values: HashMap<String, LoxValue>,
}

impl Environment {
    pub fn new() -> Self {
        Environment { values: HashMap::new(), }
    }

    pub fn define(&mut self, name: String, value: LoxValue) {
        self.values.insert(name, value);
    }

    pub fn get(&self, name: &str, line: u32 ) -> LoxValue {
        if let Some(value) = self.values.get(name) {
            value.clone()
        } else {
            eprintln!("Undefined variable '{}'.", name);
            eprintln!("[line {}]", line);
            std::process::exit(70);
        }
    }
}

impl std::fmt::Display for LoxValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoxValue::Boolean(b) => write!(f, "{}", b),
            LoxValue::Nil => write!(f, "nil"),
            LoxValue::Number(n) => write!(f, "{}", n),
            LoxValue::StringLit(s) => write!(f, "{}", s),
        }
    }
}

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

fn evaluate_str(ast_string: String, line: u32, env: &Environment) -> Option<LoxValue> {
    let cleaned = clean_group_expressions(ast_string);

    if cleaned.starts_with("var_ref:") {
        let var_name = &cleaned[8..];
        return  Some(env.get(var_name, line));
    }

    if cleaned.starts_with("(+ ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, env), evaluate_str(right_str, line, env)) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => {
                    return Some(LoxValue::Number(l + r));
                }
                (Some(LoxValue::StringLit(l)), Some(LoxValue::StringLit(r))) => {
                    let concatenated = format!("{}{}", l, r);
                    return Some(LoxValue::StringLit(concatenated));
                }
                _ => {
                    eprintln!("Operands must be two numbers or two strings.");
                    eprintln!("[line {}]", line);
                    std::process::exit(70);
                }
            }
        }
    }

    if cleaned.starts_with("(- ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, env), evaluate_str(right_str, line, env)) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => {
                    return Some(LoxValue::Number(l - r));
                }
                _ => {
                    eprintln!("Operand must be numbers.");
                    eprintln!("[line {}]", line);
                    std::process::exit(70);
                }
            }
        } else {
            if let Some(val) = evaluate_str(inner.trim().to_string(), line, env) {
                match val {
                    LoxValue::Number(n) => return Some(LoxValue::Number(-n)),
                    _ => {
                        eprintln!("Operand must be a number.");
                        eprintln!("[line {}]", line);
                        std::process::exit(70);
                    }
                }
            }
        }
    }

    if cleaned.starts_with("(! ") && cleaned.ends_with(')') {
        let inner = cleaned[3..cleaned.len() - 1].to_string();
        if let Some(val) = evaluate_str(inner, line, env) {
            return Some(LoxValue::Boolean(!is_truthy(&val)));
        }
    }

    if cleaned.starts_with("(* ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, env), evaluate_str(right_str, line, env)) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => {
                    return Some(LoxValue::Number(l * r));
                }
                _ => {
                    eprintln!("Operand must be numbers.");
                    eprintln!("[line {}]", line);
                    std::process::exit(70);
                }
            }
        }
    }

    if cleaned.starts_with("(/ ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, env), evaluate_str(right_str, line, env)) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => {
                    return Some(LoxValue::Number(l / r));
                }
                _ => {
                    eprintln!("Operand must be numbers.");
                    eprintln!("[line {}]", line);
                    std::process::exit(70);
                }
            }
        }
    }

    if cleaned.starts_with("(> ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, env), evaluate_str(right_str, line, env)) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => {
                    return Some(LoxValue::Boolean(l > r));
                }
                _ => {
                    eprintln!("Operand must be numbers.");
                    eprintln!("[line {}]", line);
                    std::process::exit(70);
                }
            }
        }
    }

    if cleaned.starts_with("(< ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, env), evaluate_str(right_str, line, env)) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => {
                    return Some(LoxValue::Boolean(l < r));
                }
                _ => {
                    eprintln!("Operand must be numbers.");
                    eprintln!("[line {}]", line);
                    std::process::exit(70);
                }
            }
        }
    }
    

    if cleaned.starts_with("(>= ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, env), evaluate_str(right_str, line, env)) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => {
                    return Some(LoxValue::Boolean(l >= r));
                }
                _ => {
                    eprintln!("Operand must be numbers.");
                    eprintln!("[line {}]", line);
                    std::process::exit(70);
                }
            }
        }
    }

    if cleaned.starts_with("(<= ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, env), evaluate_str(right_str, line, env)) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => {
                    return Some(LoxValue::Boolean(l <= r));
                }
                _ => {
                    eprintln!("Operand must be numbers.");
                    eprintln!("[line {}]", line);
                    std::process::exit(70);
                }
            }
        }
    }

    if cleaned.starts_with("(== ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, env), evaluate_str(right_str, line, env)) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => {
                    return Some(LoxValue::Boolean(l == r));
                }
                (Some(LoxValue::StringLit(l)), Some(LoxValue::StringLit(r))) => {
                    return Some(LoxValue::Boolean(l == r));
                }
                (Some(LoxValue::Boolean(l)), Some(LoxValue::Boolean(r))) => {
                    return Some(LoxValue::Boolean(l == r));
                }
                (Some(LoxValue::Nil), Some(LoxValue::Nil)) => {
                    return Some(LoxValue::Boolean(true));
                }
                (Some(_), Some(_)) => {
                    return Some(LoxValue::Boolean(false));
                }
                _ => return None,
            }
        }
    }

    if cleaned.starts_with("(!= ") && cleaned.ends_with(')') {
        let inner = &cleaned[4..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            match (evaluate_str(left_str, line, env), evaluate_str(right_str, line, env)) {
                (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) => {
                    return Some(LoxValue::Boolean(l != r));
                }
                (Some(LoxValue::StringLit(l)), Some(LoxValue::StringLit(r))) => {
                    return Some(LoxValue::Boolean(l != r));
                }
                (Some(LoxValue::Boolean(l)), Some(LoxValue::Boolean(r))) => {
                    return Some(LoxValue::Boolean(l != r));
                }
                (Some(LoxValue::Nil), Some(LoxValue::Nil)) => {
                    return Some(LoxValue::Boolean(false));
                }
                (Some(_), Some(_)) => {
                    return Some(LoxValue::Boolean(true));
                }
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

pub fn run_evaluate(file_contents: String) {
    let (tokens, error) = crate::scanner::scan_tokens(&file_contents);
    if error {
        std::process::exit(65);
    }

    let mut parser = crate::parser::Parser::new(tokens);

    match parser.parse() {
        Ok(ast_string) => {
            let env = Environment::new();
            if let Some(result) = evaluate_str(ast_string, 1, &env) {
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
    let (tokens, error) = crate::scanner::scan_tokens(&file_contents);
    if error {
        std::process::exit(65);
    }

    let mut parser = crate::parser::Parser::new(tokens);

    match parser.parse_statements() {
        Ok(statements) => {
            let mut env = Environment::new();

            for stmt in statements {
                
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
                            match evaluate_str(initializer_expr, line_num, &env) {
                                Some(val) => val,
                                None => LoxValue::Nil,
                            }
                        };

                        env.define(var_name, value);
                    }
                } else if stmt.starts_with("(expr line:") {
                    let rest = &stmt[11..];
                    let space_idx = rest.find(' ').unwrap();
                    let line_num: u32 = rest[..space_idx].parse().unwrap();
                    let inner_expr = &rest[space_idx + 1..rest.len() - 1];

                    evaluate_str(inner_expr.to_string(), line_num, &env);
                } else if stmt.starts_with("(print line:") {
                    let rest = &stmt[12..];
                    let space_idx = rest.find(' ').unwrap();
                    let line_num: u32 = rest[..space_idx].parse().unwrap();
                    let inner_expr = &rest[space_idx + 1..rest.len() - 1];

                    if let Some(result) = evaluate_str(inner_expr.to_string(), line_num, &env) {
                        println!("{}", result);
                    } else {
                        std::process::exit(70);
                    }
                }
            }
        }
        Err(_) => {
            std::process::exit(65);
        }
    }
}