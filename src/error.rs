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

#[allow(dead_code)]
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

#[allow(dead_code)]
pub fn warning(message: &str) {
    let _ = writeln!(io::stderr(), "warning: {}", message);
}

pub fn runtime(line: u32, message: impl AsRef<str>) -> ! {
    fatal(ErrorKind::Runtime, line, message);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_error_labels() {
        assert_eq!(
            format_error(ErrorKind::Syntax, 3, "expected ':'"),
            "SyntaxError: line 3: expected ':'"
        );
        assert_eq!(
            format_error(ErrorKind::Indentation, 2, "unexpected indent"),
            "IndentationError: line 2: unexpected indent"
        );
        assert_eq!(
            format_error(ErrorKind::Runtime, 10, "division by zero"),
            "RuntimeError: line 10: division by zero"
        );
    }
}
