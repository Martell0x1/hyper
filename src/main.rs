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

            while let Some(ch) = chars.next() {
                match ch {
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
                    _ => {
                        eprintln!("[line 1] Error: Unexpected character: {}", ch);
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