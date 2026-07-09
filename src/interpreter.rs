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

pub fn run_evaluate(file_contents: String) {
    let (tokens, error) = crate::scanner::scan_tokens(&file_contents);
    if error {
        std::process::exit(65);
    }

    let mut parser = crate::parser::Parser::new(tokens);
    
    match parser.parse() {
        Ok(ast_string) => {
            match ast_string.as_str() {
                "true" => println!("{}", LoxValue::Boolean(true)),
                "false" => println!("{}", LoxValue::Boolean(false)),
                "nil" => println!("{}", LoxValue::Nil),
                _ => {
                    if let Ok(num) = ast_string.parse::<f64>() {
                        println!("{}", LoxValue::Number(num));
                    } else if ast_string.starts_with('"') && ast_string.ends_with('"') {
                        let clean_str = &ast_string[1..ast_string.len()-1];
                        println!("{}", LoxValue::StringLit(clean_str.to_string()));
                    } else {
                        println!("{}", ast_string);
                    }
                }
            }
        }
        Err(_) => {
            std::process::exit(65);
        }
    }
}