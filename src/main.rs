use std::env;
use std::fs;
use std::io::{self, Write};

mod ast;
mod error;
mod scanner;
mod parser;
mod driver;
mod environment;
mod fileio;
mod json;
mod semantic;
mod module;
mod compiler;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        let _ = writeln!(
            io::stderr(),
            "Usage: {} <tokenize|parse|run|typecheck|compile> <filename> [options]\n\
             \n\
             run <file>                        JIT execute (Cranelift)\n\
             compile <file>                    JIT execute\n\
             compile <file> --emit-ir          print IR only\n\
             compile <file> --emit-obj [path]  emit object (default a.o)\n\
             compile <file> --emit-exe [path]  emit executable (default hyper_out)\n\
             typecheck <file>                  typecheck only",
            args[0]
        );
        return;
    }

    let command = &args[1];
    let filename = &args[2];

    let file_contents = fs::read_to_string(filename).unwrap_or_else(|_| {
        let _ = writeln!(io::stderr(), "Failed to read file {}", filename);
        String::new()
    });

    match command.as_str() {
        "tokenize" => {
            scanner::scan_tokens(&file_contents);
        }
        "parse" => {
            parser::run_parse(file_contents);
        }
        "run" => {
            // Compiler-only: same Cranelift JIT path as `compile`.
            if let Err(errors) = compiler::try_jit(&file_contents, filename) {
                for e in &errors {
                    error::report_formatted(e);
                }
                std::process::exit(65);
            }
        }
        "typecheck" => {
            semantic::run_typecheck(file_contents);
        }
        "compile" => {
            let mode = parse_compile_mode(&args[3..]);
            compiler::run_compile(file_contents, filename, mode);
        }
        "evaluate" => {
            let _ = writeln!(
                io::stderr(),
                "error: 'evaluate' was removed — Hyper is compiler-only.\n\
                 Use a small program with print(...) and `hyper run`, or `hyper compile`."
            );
            std::process::exit(64);
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
            let _ = writeln!(io::stderr(), "Unknown compile option: {other}");
            let _ = writeln!(
                io::stderr(),
                "Expected: --emit-ir | --emit-obj [path] | --emit-exe [path]"
            );
            std::process::exit(64);
        }
    }
}
