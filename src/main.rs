use std::env;
use std::fs;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} tokenize <filename>", args[0]);
        return;
    }

    let command = &args[1];
    let filename = &args[2];

    match command.as_str() {
        "tokenize" => { 
            eprintln!("Logs from program will appear here!"); 

            let file_contents = fs::read_to_string(filename).unwrap_or_else(|_| {
                eprintln!("Failed to read file {}", filename);
                String::new()
            });

            let mut chars = file_contents.chars().peekable();
            let mut error = false;
            let mut line = 1;

            while let Some(ch) = chars.next() {
                match ch {
                    ' ' | '\t' | '\r' => {}
                    '\n' => { line += 1; } 

                    '(' => println!("LEFT_PAREN ( null"),
                    ')' => println!("RIGHT_PAREN ) null"),
                    '{' => println!("LEFT_BRACE {{ null"),
                    '}' => println!("RIGHT_BRACE }} null"),
                    '.' => println!("DOT . null"),
                    ',' => println!("COMMA , null"),
                    '-' => println!("MINUS - null"),
                    '+' => println!("PLUS + null"),
                    ';' => println!("SEMICOLON ; null"),
                    '*' => println!("STAR * null"),
                    '=' => {
                        if chars.peek() == Some(&'=') {
                            chars.next();
                            println!("EQUAL_EQUAL == null");
                        } else {
                            println!("EQUAL = null")
                        }
                    }
                    '!' => {
                        if chars.peek() == Some(&'=') {
                            chars.next();
                            println!("BANG_EQUAL != null");
                        } else {
                            println!("BANG ! null")
                        }
                    }
                    '<' => {
                        if chars.peek() == Some(&'=') {
                            chars.next();
                            println!("LESS_EQUAL <= null");
                        } else {
                            println!("LESS < null")
                        }
                    }
                    '>' => {
                        if chars.peek() == Some(&'=') {
                            chars.next();
                            println!("GREATER_EQUAL >= null");
                        } else {
                            println!("GREATER > null")
                        }
                    }
                    '/' => {
                        if chars.peek() == Some(&'/') {
                            while chars.peek() != Some(&'\n') && chars.peek().is_some() {
                                chars.next();
                            }
                        } else {
                            println!("SLASH / null")
                        }
                    }
                    _ => {
                        eprintln!("[line {}] Error: Unexpected character: {}", line, ch);
                        error = true;
                    }
                }
            }

            println!("EOF  null");

            if error {
                std::process::exit(65);
            }
        }
        _ => {
            println!("Unknown command: {}", command);
        }
    }
}