use std::io::{self, Write};
use std::process;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorKind {
    Syntax,
    Indentation,
    Runtime,
}

impl ErrorKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Syntax => "SyntaxError",
            Self::Indentation => "IndentationError",
            Self::Runtime => "RuntimeError",
        }
    }

    pub fn exit_code(self) -> i32 {
        match self {
            Self::Syntax | Self::Indentation => 65,
            Self::Runtime => 70,
        }
    }
}

pub fn format_error(kind: ErrorKind, line: u32, message: &str) -> String {
    format!("{}: line {}: {}", kind.label(), line, message)
}

pub fn report(kind: ErrorKind, line: u32, message: &str) {
    let _ = writeln!(io::stderr(), "{}", format_error(kind, line, message));
}

pub fn report_formatted(message: &str) {
    let _ = writeln!(io::stderr(), "{}", message);
}

pub fn fatal(kind: ErrorKind, line: u32, message: impl AsRef<str>) -> ! {
    report(kind, line, message.as_ref());
    process::exit(kind.exit_code());
}

pub fn syntax(line: u32, message: impl AsRef<str>) -> ! {
    fatal(ErrorKind::Syntax, line, message);
}

pub fn syntax_at_token(line: u32, token: &str, message: &str) {
    report(
        ErrorKind::Syntax,
        line,
        &format!("at '{}': {}", token, message),
    );
}

pub fn syntax_msg(line: u32, message: &str) {
    report(ErrorKind::Syntax, line, message);
}

pub fn indentation(line: u32, message: impl AsRef<str>) {
    report(ErrorKind::Indentation, line, message.as_ref());
}

pub fn warning(message: &str) {
    let _ = writeln!(io::stderr(), "warning: {}", message);
}

pub fn format_typecheck(msg: &str) -> String {
    if let Some(rest) = msg.strip_prefix("[line ") {
        if let Some((line_str, tail)) = rest.split_once("] ") {
            if let Ok(line) = line_str.parse::<u32>() {
                let message = tail
                    .strip_prefix("Error: ")
                    .or_else(|| tail.strip_prefix("Type error: "))
                    .unwrap_or(tail)
                    .trim_end_matches('.');
                return format_error(ErrorKind::Syntax, line, message);
            }
        }
    }
    let message = msg
        .strip_prefix("Error: ")
        .or_else(|| msg.strip_prefix("Type error: "))
        .unwrap_or(msg)
        .trim_end_matches('.');
    format_error(ErrorKind::Syntax, 0, message)
}

pub fn runtime(line: u32, message: impl AsRef<str>) -> ! {
    fatal(ErrorKind::Runtime, line, message);
}
