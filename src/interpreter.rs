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

fn evaluate_str(ast_string: String) -> Option<LoxValue> {
    let cleaned = clean_group_expressions(ast_string);

    if cleaned.starts_with("(- ") && cleaned.ends_with(')') {
        let inner = cleaned[3..cleaned.len() - 1].to_string();
        if let Some(LoxValue::Number(n)) = evaluate_str(inner) {
            return Some(LoxValue::Number(-n));
        }
    }

    if cleaned.starts_with("(! ") && cleaned.ends_with(')') {
        let inner = cleaned[3..cleaned.len() - 1].to_string();
        if let Some(val) = evaluate_str(inner) {
            return Some(LoxValue::Boolean(!is_truthy(&val)));
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