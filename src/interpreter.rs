#[derive(Debug, Clone)]
pub enum LoxValue {
    Boolean(bool),
    Nil,
    Number(f64),
    StringLit(String),
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

fn evaluate_str(ast_string: String) -> Option<LoxValue> {
    let cleaned = clean_group_expressions(ast_string);

    if cleaned.starts_with("(+ ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            if let (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) = (evaluate_str(left_str), evaluate_str(right_str)) {
                return Some(LoxValue::Number(l + r));
            }
        }
    }

    if cleaned.starts_with("(- ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            if let (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) = (evaluate_str(left_str), evaluate_str(right_str)) {
                return Some(LoxValue::Number(l - r));
            }
        } else {
            if let Some(LoxValue::Number(n)) = evaluate_str(inner.trim().to_string()) {
                return Some(LoxValue::Number(-n));
            }
        }
    }

    if cleaned.starts_with("(! ") && cleaned.ends_with(')') {
        let inner = cleaned[3..cleaned.len() - 1].to_string();
        if let Some(val) = evaluate_str(inner) {
            return Some(LoxValue::Boolean(!is_truthy(&val)));
        }
    }

    if cleaned.starts_with("(* ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            if let (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) = (evaluate_str(left_str), evaluate_str(right_str)) {
                return Some(LoxValue::Number(l * r));
            }
        }
    }

    if cleaned.starts_with("(/ ") && cleaned.ends_with(')') {
        let inner = &cleaned[3..cleaned.len() - 1];
        if let Some((left_str, right_str)) = split_binary_args(inner) {
            if let (Some(LoxValue::Number(l)), Some(LoxValue::Number(r))) = (evaluate_str(left_str), evaluate_str(right_str)) {
                return Some(LoxValue::Number(l / r));
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
            if let Some(result) = evaluate_str(ast_string) {
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