use std::env;
use std::fs;

mod ast;
mod scanner;
mod parser;
mod frontend;
mod environment;
mod text_utils;
mod interpreter;
mod semantic;
mod ir;
mod compiler;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <tokenize|parse|evaluate|run|typecheck|compile> <filename>",
            args[0]
        );
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
        "typecheck" => {
            semantic::run_typecheck(file_contents);
        }
        "compile" => {
            compiler::run_compile(file_contents);
        }
        _ => {
            println!("Unknown command: {}", command);
        }
    }
}
