use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    LeftParen, 
    RightParen, 
    LeftBrace, 
    RightBrace,
    
    Comma, 
    Dot, 
    Minus, 
    Plus, 
    Semicolon, 
    Slash, 
    Star,

    Bang, 
    BangEqual, 
    Equal, 
    EqualEqual,
    Greater, 
    GreaterEqual, 
    Less, 
    LessEqual,

    TypeI8, TypeI16, TypeI32, TypeI64,
    TypeU8, TypeU16, TypeU32, TypeU64,
    TypeF32, TypeF64,
    TypeString, TypeBool,
    
    Identifier, 
    StringLit, 
    Number,
    None, 

    True,
    False,
    And, 
    Or,
    Not,

    Else,  
    Fun, 
    While, 
    For, 
    If, 
    Print, 
    Return,  

    Let,
    Mut,
    EOF,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub token_type: TokenType,
    pub lexeme: String,
    pub literal: String,
    pub line: usize,
}

pub fn num_literals(ch: char, chars: &mut Peekable<Chars>) -> String {
    let mut num_str = String::from(ch);
    while chars.peek().map_or(false, |c| c.is_ascii_digit()) {
        num_str.push(chars.next().unwrap());
    }
    if chars.peek() == Some(&'.') {
        let mut clone = chars.clone();
        clone.next();
        if chars.peek().map_or(false, |c| c.is_ascii_digit()) {
            num_str.push(chars.next().unwrap());
            while chars.peek().map_or(false, |c| c.is_ascii_digit()) {
                num_str.push(chars.next().unwrap());
            }
        }
    }
    num_str
}

pub fn str_literals(chars: &mut Peekable<Chars>, line: &mut usize) -> Option<String> {
    let mut str_val = String::new();
    while let Some(next_ch) = chars.next() {
        if next_ch == '"' { return Some(str_val); }
        if next_ch == '\n' { *line += 1; }
        str_val.push(next_ch);
    }
    None
}

pub fn scan_tokens(file_contents: &str) -> (Vec<Token>, bool) {
    let mut chars = file_contents.chars().peekable();
    let mut tokens = Vec::new();
    let mut error = false;
    let mut line = 1;

    macro_rules! add_token {
        ($t:expr, $lex:expr, $lit:expr) => {
            tokens.push(Token { token_type: $t, lexeme: $lex.to_string(), literal: $lit.to_string(), line })
        };
    }

    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\t' | '\r' => {}
            '\n' => { line += 1; } 

            '(' => add_token!(TokenType::LeftParen, "(", "null"),
            ')' => add_token!(TokenType::RightParen, ")", "null"),
            '{' => add_token!(TokenType::LeftBrace, "{", "null"),
            '}' => add_token!(TokenType::RightBrace, "}", "null"),
            '.' => add_token!(TokenType::Dot, ".", "null"),
            ',' => add_token!(TokenType::Comma, ",", "null"),
            '-' => add_token!(TokenType::Minus, "-", "null"),
            '+' => add_token!(TokenType::Plus, "+", "null"),
            ';' => add_token!(TokenType::Semicolon, ";", "null"),
            '*' => add_token!(TokenType::Star, "*", "null"),
            '=' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    add_token!(TokenType::EqualEqual, "==", "null");
                } else {
                    add_token!(TokenType::Equal, "=", "null");
                }
            }
            '!' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    add_token!(TokenType::BangEqual, "!=", "null");
                } else {
                    add_token!(TokenType::Bang, "!", "null");
                }
            }
            '<' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    add_token!(TokenType::LessEqual, "<=", "null");
                } else {
                    add_token!(TokenType::Less, "<", "null");
                }
            }
            '>' => {
                if chars.peek() == Some(&'=') {
                    chars.next();
                    add_token!(TokenType::GreaterEqual, ">=", "null");
                } else {
                    add_token!(TokenType::Greater, ">", "null");
                }
            }
            '/' => {
                if chars.peek() == Some(&'/') {
                    while chars.peek() != Some(&'\n') && chars.peek().is_some() {
                        chars.next();
                    }
                } else {
                    add_token!(TokenType::Slash, "/", "null");
                }
            }
            '"' => {
                if let Some(str_val) = str_literals(&mut chars, &mut line) {
                    add_token!(TokenType::StringLit, format!("\"{}\"", str_val), str_val);
                } else {
                    eprint!("[line {}] Error: Unterminated string.", line);
                    error = true;
                }
            }
            '0'..='9' => {
                let num_str = num_literals(ch, &mut chars);
                let num_val: f64 = num_str.parse().unwrap(); 

                let lit = if num_val.fract() == 0.0 {
                    format!("{:.1}", num_val)
                } else {
                    num_val.to_string()
                };

                add_token!(TokenType::Number, num_str, lit);
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident = String::from(ch);
                while chars.peek().map_or(false, |c| c.is_ascii_alphanumeric() || *c == '_') {
                    ident.push(chars.next().unwrap());
                }

                let t_type = match ident.as_str() {
                    "true" => TokenType::True,
                    "false" => TokenType::False,
                    "and" => TokenType::And,
                    "or" => TokenType::Or,
                    "not" => TokenType::Not,
                    "None" => TokenType::None,
                    "else" => TokenType::Else,
                    "while" => TokenType::While,
                    "for" => TokenType::For,
                    "fun" => TokenType::Fun,
                    "if" => TokenType::If,
                    "print" => TokenType::Print,
                    "return" => TokenType::Return,
                    "let" => TokenType::Let,
                    "mut" => TokenType::Mut, 

                    "i8" => TokenType::TypeI8,
                    "i16" => TokenType::TypeI16,
                    "i32" => TokenType::TypeI32,
                    "i64" => TokenType::TypeI64,
                    "u8" => TokenType::TypeU8,
                    "u16" => TokenType::TypeU16,
                    "u32" => TokenType::TypeU32,
                    "u64" => TokenType::TypeU64,
                    "f32" => TokenType::TypeF32,
                    "f64" => TokenType::TypeF64,
                    "string" => TokenType::TypeString,
                    "bool" => TokenType::TypeBool,
                
                    _ => TokenType::Identifier,
                };
                add_token!(t_type, ident, "null");
            }
            _ => {
                eprintln!("[line {}] Error: Unexpected character: {}", line, ch);
                error = true;
            }
        }
    }
    add_token!(TokenType::EOF, "", "null");
    (tokens, error)
}