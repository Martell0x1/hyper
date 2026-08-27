use crate::ast::Stmt;
use crate::parser::Parser;
use crate::scanner;

/// Scan and parse a Hyper source string into a statement list.
pub fn parse_program(source: &str) -> Result<Vec<Stmt>, ()> {
    let (tokens, error) = scanner::scan_tokens(source);
    if error {
        return Err(());
    }

    let mut parser = Parser::new(tokens);
    parser.parse_statements()
}
