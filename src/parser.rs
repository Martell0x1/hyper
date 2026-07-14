use crate::scanner::{Token, TokenType};

pub struct Parser {
    tokens: Vec<Token>,
    current: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Parser { tokens, current: 0 }
    }

    pub fn parse(&mut self) -> Result<String, ()> {
        let result = self.expression()?;

        if !self.is_at_end() && self.peek().token_type != TokenType::EOF {
            let token = self.peek().clone();
            eprintln!("[line {}] Error at '{}': Expect expression.", token.line, token.lexeme);
            return Err(());
        }
        Ok(result)
    }

    fn expression(&mut self) -> Result<String, ()> {
        self.assignment()
    }

    fn assignment(&mut self) -> Result<String, ()> {
        let expr = self.equality()?;
        if self.match_types(&[TokenType::Equal]) {
            let value = self.assignment()?;

            if expr.starts_with("var_ref:") {
                let var_name = &expr[8..];
                return  Ok(format!("(assign {} {}", var_name, value));
            }
        }
        Ok(expr)
    }

    fn equality(&mut self) -> Result<String, ()> {
        let mut expr = self.comparison()?;
        while self.match_types(&[TokenType::BangEqual, TokenType::EqualEqual]) {
            let operator = self.previous().lexeme.clone();
            let right = self.comparison()?;
            expr = format!("({} {} {})", operator, expr, right);
        }
        Ok(expr)
    }

    fn comparison(&mut self) -> Result<String, ()> {
        let mut expr = self.term()?;
        while self.match_types(&[TokenType::Greater, TokenType::GreaterEqual, TokenType::Less, TokenType::LessEqual]) {
            let operator = self.previous().lexeme.clone();
            let right = self.term()?;
            expr = format!("({} {} {})", operator, expr, right);
        }
        Ok(expr)
    }

    fn term(&mut self) -> Result<String, ()> {
        let mut expr = self.factor()?;
        while self.match_types(&[TokenType::Minus, TokenType::Plus]) {
            let operator = self.previous().lexeme.clone();
            let right = self.factor()?;
            expr = format!("({} {} {})", operator, expr, right);
        }
        Ok(expr)
    }

    fn factor(&mut self) -> Result<String, ()> {
        let mut expr = self.unary()?;
        while self.match_types(&[TokenType::Slash, TokenType::Star]) {
            let operator = self.previous().lexeme.clone();
            let right = self.unary()?;
            expr = format!("({} {} {})", operator, expr, right);
        }
        Ok(expr)
    }

    fn unary(&mut self) -> Result<String, ()> {
        if self.match_types(&[TokenType::Bang, TokenType::Minus]) {
            let operator = self.previous().lexeme.clone();
            let right = self.unary()?;
            return Ok(format!("({} {})", operator, right));
        }
        self.primary()
    }

    fn primary(&mut self) -> Result<String, ()> {
        if self.match_types(&[TokenType::False]) { return Ok("false".to_string()); }
        if self.match_types(&[TokenType::True]) { return Ok("true".to_string()); }
        if self.match_types(&[TokenType::Nil]) { return Ok("nil".to_string()); }

        if self.match_types(&[TokenType::Identifier]) {
            return Ok(format!("var_ref:{}", self.previous().lexeme));
        }
        
        if self.match_types(&[TokenType::Number, TokenType::StringLit]) {
            return Ok(self.previous().literal.clone());
        }

        if self.match_types(&[TokenType::LeftParen]) {
            let expr = self.expression()?;
            self.consume(TokenType::RightParen, "Expect ')' after expression.")?;
            return Ok(format!("(group {})", expr));
        }

        let token = self.peek().clone();
        if token.token_type == TokenType::EOF {
            eprintln!("[line {}] Error at end: Expect expression.", token.line);
        } else {
            eprintln!("[line {}] Error at '{}': Expect expression.", token.line, token.lexeme);
        }
        Err(())
    }

    fn match_types(&mut self, types: &[TokenType]) -> bool {
        for t in types {
            if self.check(t) {
                self.advance();
                return true;
            }
        }
        false
    }

    fn check(&self, token_type: &TokenType) -> bool {
        if self.is_at_end() { return false; }
        &self.peek().token_type == token_type
    }

    fn advance(&mut self) -> &Token {
        if !self.is_at_end() { self.current += 1; }
        self.previous()
    }

    fn is_at_end(&self) -> bool {
        self.peek().token_type == TokenType::EOF
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.current]
    }

    fn previous(&self) -> &Token {
        &self.tokens[self.current - 1]
    }

    fn consume(&mut self, token_type: TokenType, message: &str) -> Result<&Token, ()> {
        if self.check(&token_type) { return Ok(self.advance()); }
        let token = self.peek();
        eprintln!("[line {}] Error at '{}': {}", token.line, token.lexeme, message);
        Err(())
    }

    pub fn parse_statements(&mut self) -> Result<Vec<String>, ()> {
        let mut statements = Vec::new();
        while !self.is_at_end() && self.peek().token_type != TokenType::EOF {
            statements.push(self.declaration()?);
        }
        Ok(statements)
    }

    fn declaration(&mut self) -> Result<String, ()> {
        if self.match_types(&[TokenType::Var]) {
            return self.var_declaration();
        }
        self.statement()
    }

    fn var_declaration(&mut self) -> Result<String, ()> {
        let line = self.peek().line;
        let name_token = self.consume(TokenType::Identifier, "Expect variable name.")?.clone();
        let var_name = name_token.lexeme;

        let mut initializer = "nil".to_string();
        if self.match_types(&[TokenType::Equal]) {
            initializer = self.expression()?;
        }

        self.consume(TokenType::Semicolon, "Expect ';' after variable declaration")?;
        Ok(format!("(var line:{} {} {})", line, var_name, initializer))
    }

    fn statement(&mut self) -> Result<String, ()> {
        if self.match_types(&[TokenType::Print]) {
            return self.print_statement();
        }
        self.expression_statement()
    }

    fn print_statement(&mut self) -> Result<String, ()> {
        let line = self.peek().line;
        let value = self.expression()?;
        self.consume(TokenType::Semicolon, "Expect ';' after value.")?;
        Ok(format!("(print line:{} {})", line, value))
    }

    fn expression_statement(&mut self) -> Result<String, ()> {
        let line = self.peek().line;
        let expr = self.expression()?;
        self.consume(TokenType::Semicolon, "Expect ';' after value.")?;
        Ok(format!("(expr line:{} {})", line, expr))
    }
}

pub fn run_parse(file_contents: String) {
    let (tokens, error) = crate::scanner::scan_tokens(&file_contents);
    
    if error {
        std::process::exit(65);
    }

    let mut parser = Parser::new(tokens);

    match parser.parse() {
        Ok(ast_string) => {
            println!("{}", ast_string);
        }
        Err(_) => {
            std::process::exit(65);
        }
    }
}