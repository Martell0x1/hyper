use std::env;
use std::fs;

mod ast;
mod scanner;
mod parser;
mod driver;
mod environment;
mod text_utils;
mod interpreter;
mod semantic;
mod ir;
mod compiler;
mod runtime;
mod codegen;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        eprintln!(
            "Usage: {} <tokenize|parse|evaluate|run|typecheck|compile> <filename> [options]\n\
             \n\
             compile <file>                    JIT execute\n\
             compile <file> --emit-ir          print IR only\n\
             compile <file> --emit-obj [path]  emit object (default a.o)\n\
             compile <file> --emit-exe [path]  emit executable (default hyper_out)",
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
            let mode = parse_compile_mode(&args[3..]);
            compiler::run_compile(file_contents, mode);
        }
        _ => {
            println!("Unknown command: {}", command);
        }
    }
}

fn parse_compile_mode(args: &[String]) -> compiler::CompileMode {
    if args.is_empty() {
        return compiler::CompileMode::Jit;
    }
    match args[0].as_str() {
        "--emit-ir" => compiler::CompileMode::EmitIr,
        "--emit-obj" => {
            let path = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| "a.o".to_string());
            compiler::CompileMode::EmitObj { path }
        }
        "--emit-exe" => {
            let path = args
                .get(1)
                .cloned()
                .unwrap_or_else(|| "hyper_out".to_string());
            compiler::CompileMode::EmitExe { path }
        }
        other => {
            eprintln!("Unknown compile option: {other}");
            eprintln!(
                "Expected: --emit-ir | --emit-obj [path] | --emit-exe [path]"
            );
            std::process::exit(64);
        }
    }
}
