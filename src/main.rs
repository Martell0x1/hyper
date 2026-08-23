use std::env;
use std::fs;

mod scanner;
mod parser;
mod environment;
mod text_utils;
mod interpreter;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!("Usage: {} <command> <filename>", args[0]);
        return;
    }

    let command = &args[1];
    let filename = &args[2];

    let file_contents = fs::read_to_string(filename).unwrap_or_else(|_| {
        eprintln!("Failed to read file {}", filename);
        String::new()
    });

    match command.as_str() {
        "tokenize" => {
            scanner::scan_tokens(&file_contents);
        }
        "parse" => {
            parser::run_parse(file_contents);
        }
        "evaluate" => {
            interpreter::run_evaluate(file_contents);
        }
        "run" => {
            crate::interpreter::run_program(file_contents);
        }
        _ => {
            println!("Unknown command: {}", command);
        }
    }
}