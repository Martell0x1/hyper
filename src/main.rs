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
                    '"' => {
                        let mut str_val = String::new();
                        let mut is_terminated = false;

                        while let Some(&next_ch) = chars.peek() {
                            if next_ch == '"' {
                                chars.next();
                                is_terminated = true;
                                break;
                            }

                            if next_ch == '\n' {
                                line += 1;
                            }

                            str_val.push(chars.next().unwrap());
                        }

                        if is_terminated {
                            println!("STRING \"{}\" {}", str_val, str_val);
                        } else {
                            eprint!("[line {}] Error: Unterminated string.", line);
                            error = true;
                        }
                    }
                    '0'..='9' => {
                        let mut num_str = String::new();
                        num_str.push(ch);

                        while let Some(&next_ch) = chars.peek() {
                            if next_ch.is_ascii_digit() {
                                num_str.push(chars.next().unwrap());
                            } else {
                                break;
                            }
                        }

                        if chars.peek() == Some(&'.') {
                            let mut clone_chars = chars.clone();
                            clone_chars.next();

                            if let Some(&after_dot) = clone_chars.peek() {
                                if after_dot.is_ascii_digit() {
                                    num_str.push(chars.next().unwrap());

                                    while let Some(&next_ch) = chars.peek() {
                                        if next_ch.is_ascii_digit() {
                                            num_str.push(chars.next().unwrap());
                                        } else {
                                            break;
                                        }
                                    }
                                }
                            }
                        }

                        let num_val: f64 = num_str.parse().unwrap();

                        let literal_str = if num_val.fract() == 0.0 {
                            format!("{:.1}", num_val)
                        } else {
                            format!("{}", num_val)
                        };

                        println!("NUMBER {} {}", num_str, literal_str)
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