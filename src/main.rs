use std::env;
use std::fs;

mod scanner;
mod parser;

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
            scanner::run_tokenize(file_contents);
        }
        "parse" => {
            parser::run_parse(file_contents);
        }
        _ => {
            println!("Unknown command: {}", command);
        }
    }
}