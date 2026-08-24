use std::iter::Peekable;
use std::str::Chars;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenType {
    Indent,
    Dedent,
    Newline,

    LeftParen, 
    RightParen, 
    LeftBrace, 
    RightBrace,
    
    Colon,
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
    String, 
    FString,
    Number,
    None, 

    Struct,
    Trait,

    True,
    False,
    And, 
    Or,
    Not,

    If,
    Elif,
    Else,  
    Def,
    Fn, 
    Ref,
    Arrow,
    While, 
    For,  
    In,
    Range,
    Print,
    Input, 
    Return,

    Array,
    Dict,  

    With,
    As,
    OpenMmap,
    ReadChunk,

    At,
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
        let n = chars.next().unwrap();
        if n != '_' {
            num_str.push(n);
        }
    }
    if chars.peek() == Some(&'.') {
        let mut clone = chars.clone();
        clone.next();
        if clone.peek().map_or(false, |c| c.is_ascii_digit()) {
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
        match next_ch {
            '\\' => {
                if let Some(escaped) = chars.next() {
                    match escaped {
                        'n' => str_val.push('\n'),
                        't' => str_val.push('\t'),
                        '"' => str_val.push('"'),
                        '\\' => str_val.push('\\'),
                        _ => {
                            str_val.push('\\');
                            str_val.push(escaped);
                        }
                    }
                }
            }
            '"' => return Some(str_val),
            '\n' => {
                *line += 1;
                str_val.push('\n');
            }
            _ => str_val.push(next_ch),
        }
    }
    None
}

pub fn scan_tokens(file_contents: &str) -> (Vec<Token>, bool) {
    let mut chars = file_contents.chars().peekable();
    let mut tokens = Vec::new();
    let mut error = false;
    let mut line = 1;

    let mut indent_stack = vec![0];
    let mut at_line_start = true;

    macro_rules! add_token {
        ($t:expr, $lex:expr, $lit:expr) => {
            tokens.push(Token {
                token_type: $t,
                lexeme: $lex.to_string(),
                literal: $lit.to_string(),
                line,
            })
        };
    }

    while let Some(ch) = chars.next() {
        if at_line_start {
            let mut indent_level = 0;
            let mut current_ch = Some(ch);

            while let Some(c) = current_ch {
                if c == ' ' {
                    indent_level += 1;
                } else if c == '\t' {
                    indent_level += 4;
                } else {
                    break;
                }
                current_ch = chars.next();
            }

            if current_ch == Some('\n') {
                line += 1;
                continue;
            } else if current_ch == Some('#') {
                while chars.peek() != Some(&'\n') && chars.peek().is_some() {
                    chars.next();
                }
                line += 1;
                continue;
            }

            if let Some(c) = current_ch {
                let current_indent = *indent_stack.last().unwrap();

                if indent_level > current_indent {
                    indent_stack.push(indent_level);
                    add_token!(TokenType::Indent, "INDENT", "null");
                } else if indent_level < current_indent {
                    while *indent_stack.last().unwrap() > indent_level {
                        indent_stack.pop();
                        add_token!(TokenType::Dedent, "DEDENT", "null");
                    }
                }
                at_line_start = false;

                match_char(c, &mut chars, &mut tokens, &mut line, &mut error, &mut at_line_start);
            }
            continue;
        }
        match_char(ch, &mut chars, &mut tokens, &mut line, &mut error, &mut at_line_start)
    }

    while indent_stack.len() > 1 {
        indent_stack.pop();
        add_token!(TokenType::Dedent, "Dedent", "null");
    }

    add_token!(TokenType::EOF, "", "null");
    (tokens, error)
}   

fn match_char (
    ch: char,
    chars: &mut Peekable<Chars>,
    tokens: &mut Vec<Token>,
    line: &mut usize,
    error: &mut bool,
    at_line_start: &mut bool,
) {
    macro_rules! add_token {
        ($t:expr, $lex:expr, $lit:expr) => {
            tokens.push(Token {
                token_type: $t,
                lexeme: $lex.to_string(),
                literal: $lit.to_string(),
                line: *line,
            })
        };
    }

    match ch {
        ' ' | '\t' | '\r' => {}
        '\n' => {
            *line += 1;
            *at_line_start = true;
            add_token!(TokenType::Newline, "\\n", "null");
        } 

        '(' => add_token!(TokenType::LeftParen, "(", "null"),
        ')' => add_token!(TokenType::RightParen, ")", "null"),
        '{' => add_token!(TokenType::LeftBrace, "{", "null"),
        '}' => add_token!(TokenType::RightBrace, "}", "null"),
        '@' => add_token!(TokenType::At, "@", "null"),
        '.' => add_token!(TokenType::Dot, ".", "null"),
        ',' => add_token!(TokenType::Comma, ",", "null"),
        '-' => {
            if chars.peek() == Some(&'>') {
                chars.next();
                add_token!(TokenType::Arrow, "->", "null");
            } else {
                add_token!(TokenType::Minus, "-", "null");
            }
        }
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
        '/' => add_token!(TokenType::Slash, "/", "null"),
        '#' => {
            while chars.peek() != Some(&'\n') && chars.peek().is_some() {
                chars.next();
            }
        }
        '"' => {
            if let Some(str_val) = str_literals(chars, line) {
                add_token!(TokenType::String, format!("\"{}\"", str_val), str_val);
            } else {
                eprint!("[line {}] Error: Unterminated string.", line);
                *error = true;
            }
        }
        '0'..='9' => {
            let num_str = num_literals(ch, chars);
            let num_val: f64 = num_str.parse().unwrap(); 

            let lit = if num_val.fract() == 0.0 {
                format!("{:.1}", num_val)
            } else {
                num_val.to_string()
            };

            add_token!(TokenType::Number, num_str, lit);
        }
        'a'..='z' | 'A'..='Z' | '_' => {
            if (ch == 'f' || ch == 'F') && chars.peek() == Some(&'"') {
                chars.next();
                if let Some(str_val) = str_literals(chars, line) {
                    add_token!(TokenType::FString, format!("f\"{}\"", str_val), str_val);
                } else {
                    eprintln!("[line {}] Error: Unterminated f-string.", line);
                    *error = true;
                }
                return;
            }

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

                "struct" => TokenType::Struct,
                "trait" => TokenType::Trait,

                "if" => TokenType::If,
                "elif" => TokenType::Elif,
                "else" => TokenType::Else,
                "while" => TokenType::While,
                "for" => TokenType::For,
                "in" => TokenType::In,
                "range" => TokenType::Range,
                "def" => TokenType::Def,
                "fn" => TokenType::Fn,
                "ref" => TokenType::Ref,
                "return" => TokenType::Return,

                "print" => TokenType::Print,
                "input" => TokenType::Input,

                "array" => TokenType::Array,
                "dict" => TokenType::Dict,

                "with" => TokenType::With,
                "as" => TokenType::As,
                "open_mmap" => TokenType::OpenMmap,
                "read_chunk" => TokenType::ReadChunk,
                
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
            *error = true;
        }
    }
}