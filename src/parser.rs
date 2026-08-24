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
        let expr = self.ternary()?;

        if self.match_types(&[TokenType::Equal]) {
            let value = self.assignment()?;

            if expr.starts_with("let_ref:") {
                let let_name = &expr[8..];
                return Ok(format!("(assign {} {})", let_name, value));
            }
        }
        Ok(expr)
    }

    fn ternary(&mut self) -> Result<String, ()> {
        let mut expr = self.or_expr()?;

        if self.match_types(&[TokenType::If]) {
            let condition = self.or_expr()?;

            if !self.match_types(&[TokenType::Else]) {
                let line = self.peek().line;
                eprintln!("[line {}] Error: Expected 'else' in ternary expression.", line);
                return Err(());
            }

            let else_expr = self.ternary()?;
            expr = format!("(if {} {} {})", condition, expr, else_expr);
        }

        Ok(expr)
    }

    fn or_expr(&mut self) -> Result<String, ()> {
        let mut expr = self.and_expr()?;
        while self.match_types(&[TokenType::Or]) {
            let right = self.and_expr()?;
            expr = format!("(or {} {})", expr, right);
        }
        Ok(expr)
    }

    fn and_expr(&mut self) -> Result<String, ()> {
        let mut expr = self.equality()?;
        while self.match_types(&[TokenType::And]) {
            let right = self.equality()?;
            expr = format!("(and {} {})", expr, right);
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
        if self.match_types(&[TokenType::None]) { return Ok("None".to_string()); }

        if self.match_types(&[TokenType::LeftBrace]) {
            let mut entries = Vec::new();

            if !self.check(&TokenType::RightBrace) {
                let first_key = self.expression()?;
                
                if self.match_types(&[TokenType::Colon]) {
                    let first_val = self.expression()?;
                    entries.push(format!("{}:{}", first_key, first_val));

                    while self.match_types(&[TokenType::Comma]) {
                        if self.check(&TokenType::RightBrace) {
                            break;
                        }
                        let key = self.expression()?;
                        self.consume(TokenType::Colon, "Expect ':' after dictionary key.")?;
                        let value = self.expression()?;
                        entries.push(format!("{}:{}", key, value));
                    }
                    
                    self.consume(TokenType::RightBrace, "Expect '}' after dictionary entries.")?;
                    return Ok(format!("(dict {})", entries.join(" ")));
                } else {
                    entries.push(first_key);
                    while self.match_types(&[TokenType::Comma]) {
                        if self.check(&TokenType::RightBrace) {
                            break;
                        }
                        entries.push(self.expression()?);
                    }
                    
                    self.consume(TokenType::RightBrace, "Expect '}' after elements.")?;
                    return Ok(format!("(list {})", entries.join(" ")));
                }
            }

            self.consume(TokenType::RightBrace, "Expect '}' after empty braces.")?;
            return Ok("(list)".to_string());
        }

        if self.match_types(&[TokenType::Input]) {
            let line = self.previous().line;
            let mut prompt_expr = "None".to_string();

            self.consume(TokenType::LeftParen, "Expect '(' after input.")?;
            if !self.check(&TokenType::RightParen) {
                prompt_expr = self.expression()?;
            }
            self.consume(TokenType::RightParen, "Expect ')' after input argument.")?;

            return Ok(format!("input line:{} {}", line, prompt_expr));
        }

        if self.match_types(&[TokenType::FString]) {
            let f_content = self.previous().literal.clone();
            let line = self.previous().line;
            return self.parse_f_string(&f_content, line);
        }

        if self.match_types(&[TokenType::Identifier]) {
            let name = self.previous().lexeme.clone();
            
            if self.match_types(&[TokenType::Dot]) {
                let method_token = self.consume(TokenType::Identifier, "Expect method name after '.'")?.clone();
                let method_name = method_token.lexeme;
        
                self.consume(TokenType::LeftParen, "Expect '(' after method name.")?;
                let mut args = Vec::new();
                if !self.check(&TokenType::RightParen) {
                    loop {
                        args.push(self.expression()?);
                        if !self.match_types(&[TokenType::Comma]) {
                            break;
                        }
                    }
                }
                self.consume(TokenType::RightParen, "Expect ')' after arguments.")?;
        
                return Ok(format!("(call_method {} {} [{}])", name, method_name, args.join(" ")));
            }
        
            return Ok(format!("let_ref:{}", name));
        }
        
        if self.match_types(&[TokenType::Number, TokenType::String]) {
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

        if token.token_type == TokenType::EOF {
            eprintln!("[line {}] Error at end: {}", token.line, message);
        } else {
            eprintln!("[line {}] Error at '{}': {}", token.line, token.lexeme, message);
        }
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
        let mut is_parallel = false;
        let mut is_vectorized = false;

        while self.match_types(&[TokenType::At]) {
            let dec_token = self.consume(TokenType::Identifier, "Expect decorator name after '@'.")?.clone();
            match dec_token.lexeme.as_str() {
                "parallel" => is_parallel = true,
                "vectorize" => is_vectorized = true,
                _=> {
                    eprintln!("[line {}] Unknown decorator '@{}'.", dec_token.line, dec_token.lexeme);
                    return Err(());
                }
            }
            if self.check(&TokenType::Newline) {
                self.advance();
            }
        }

        if self.match_types(&[TokenType::Struct]) {
            return self.struct_declaration();
        }

        if self.match_types(&[TokenType::Trait]) {
            return self.trait_declaration();
        }

        if self.match_types(&[TokenType::Fn]) {
            return self.function_declaration();
        }

        if self.match_types(&[TokenType::Let]) {
            return self.let_declaration();
        }

        if self.match_types(&[TokenType::For]) {
            return self.for_statement(is_parallel, is_vectorized);
        }

        self.statement()
    }

    fn struct_declaration(&mut self) -> Result<String, ()> {
        let name_token = self.consume(TokenType::Identifier, "Expect struct name.")?.clone();
        let struct_name = name_token.lexeme;

        let mut implemented_trait = String::new();
        if self.match_types(&[TokenType::LeftParen]) {
            implemented_trait = self.consume(TokenType::Identifier, "Expect trait name inside parentheses.")?.lexeme.clone();
            self.consume(TokenType::RightParen, "Expect ')' after trait name.")?;
        }

        self.consume(TokenType::Indent, "Expect indented block for struct body.")?;

        let mut fields = Vec::new();
        let mut methods = Vec::new();

        while !self.check(&TokenType::Dedent) && !self.is_at_end() {
            if self.match_types(&[TokenType::Let]) {
                let is_mutable = self.match_types(&[TokenType::Mut]);
                let field_name = self.consume(TokenType::Identifier, "Expect field name.")?.lexeme.clone();
                self.consume(TokenType::Colon, "Expect ':' after field name.")?;
                let field_type = self.consume(TokenType::Identifier, "Expect field type.")?.lexeme.clone();
                fields.push(format!("{}:{} (mut:{})", field_name, field_type, is_mutable));
                
                if self.check(&TokenType::Newline) { self.advance(); }
            } else if self.match_types(&[TokenType::Fn]) || self.match_types(&[TokenType::Def]) {
                let method_ast = self.function_declaration()?;
                methods.push(method_ast);
            } else {
                self.advance();
            }
        }

        self.consume(TokenType::Dedent, "Expect dedent after struct body.")?;

        Ok(format!(
            "(struct {} trait:{} fields:[{}] methods:[{}])",
            struct_name, implemented_trait, fields.join(", "), methods.join(" ")
        ))
    }

    fn trait_declaration(&mut self) -> Result<String, ()> {
        let name_token = self.consume(TokenType::Identifier, "Expect trait name.")?.clone();
        let trait_name = name_token.lexeme;

        self.consume(TokenType::Indent, "Expect indented block for trait body.")?;

        let mut methods = Vec::new();
        while !self.check(&TokenType::Dedent) && !self.is_at_end() {
            if self.match_types(&[TokenType::Fn]) || self.match_types(&[TokenType::Def]) {
                let method_ast = self.function_declaration()?;
                methods.push(method_ast);
            } else {
                self.advance();
            }
        }

        self.consume(TokenType::Dedent, "Expect dedent after trait body.")?;
        Ok(format!("(trait {} methods:[{}])", trait_name, methods.join(" ")))
    }

    fn function_declaration(&mut self) -> Result<String, ()> {
        let is_strict = self.match_types(&[TokenType::Fn]);
        if !is_strict {
            self.consume(TokenType::Def, "Expect 'def' or 'fn' for function declaration.")?;
        }

        let name_token = self.consume(TokenType::Identifier, "Expect function name.")?.clone();
        let fn_name = name_token.lexeme;

        self.consume(TokenType::LeftParen, "Expect '(' after function name.")?;
        
        let mut params = Vec::new();
        if !self.check(&TokenType::RightParen) {
            loop {
                let is_ref = self.match_types(&[TokenType::Ref]);
                let param_token = self.consume(TokenType::Identifier, "Expect parameter name.")?;
                let param_name = param_token.lexeme.clone();
                
                let mut param_type = "any".to_string();
                if self.match_types(&[TokenType::Colon]) {
                    param_type = self.peek().lexeme.clone();
                    self.advance();
                }

                let ref_str = if is_ref { "ref" } else { "val" };
                params.push(format!("{}:{} ({})", param_name, param_type, ref_str));

                if !self.match_types(&[TokenType::Comma]) {
                    break;
                }
            }
        }
        self.consume(TokenType::RightParen, "Expect ')' after parameters.")?;

        let mut return_type = "None".to_string();
        if self.match_types(&[TokenType::Arrow]) {
            return_type = self.peek().lexeme.clone();
            self.advance();
        }

        self.consume(TokenType::Colon, "Expect ':' before function body.")?;

        Ok(format!("(fn {} strict:{} returns:{} params:[{}])", fn_name, is_strict, return_type, params.join(", ")))
    }

    fn let_declaration(&mut self) -> Result<String, ()> {
        let line = self.peek().line;
        let is_mutable = self.match_types(&[TokenType::Mut]);

        let name_token = self.consume(TokenType::Identifier, "Expect variable name.")?.clone();
        let let_name = name_token.lexeme;

        let mut type_annotation = "None".to_string();
        if self.match_types(&[TokenType::Colon]) {
            if self.match_types(&[TokenType::Array]) {
                self.consume(TokenType::LeftBrace, "Expect '[' after 'Array'.")?;
                let inner_type = self.peek().lexeme.clone();
                self.advance();
                self.consume(TokenType::RightBrace, "Expect ']' after array type.")?;
                type_annotation = format!("Array[{}]", inner_type);
            } else if self.match_types(&[TokenType::Dict]) {
                self.consume(TokenType::LeftBrace, "Expect '[' after 'Dict'.")?;
                let key_type = self.peek().lexeme.clone();
                self.advance();
                self.consume(TokenType::Comma, "Expect ',' between dictionary key and value types.")?;
                let val_type = self.peek().lexeme.clone();
                self.advance();
                self.consume(TokenType::RightBrace, "Expect ']' after dictionary types.")?;
                type_annotation = format!("Dict[{}, {}]", key_type, val_type);
            } else {
                let type_token = self.peek().clone();
                self.advance();
                type_annotation = type_token.lexeme;
            }
        }

        let mut initializer = "None".to_string();
        if self.match_types(&[TokenType::Equal]) {
            initializer = self.expression()?;
        }

        let mut_str = if is_mutable { "mut" } else { "immut" };
        Ok(format!("(let line:{} {} {} type:{} {})", line, mut_str, let_name, type_annotation, initializer))
    }

    fn statement(&mut self) -> Result<String, ()> {
        if self.match_types(&[TokenType::With]) {
            return self.with_mmap_statement();
        }

        if self.match_types(&[TokenType::Return]) {
            return self.return_statement();
        }

        if self.match_types(&[TokenType::Print]) {
            return self.print_statement();
        }

        if self.match_types(&[TokenType::Indent, TokenType::LeftBrace]) {
            return self.block();
        }

        if self.match_types(&[TokenType::If]) {
            return self.if_statement();
        }

        if self.match_types(&[TokenType::While]) {
            return self.while_statement();
        }

        if self.match_types(&[TokenType::For]) {
            return self.for_statement(false, false);
        }

        self.expression_statement()
    }

    fn with_mmap_statement(&mut self) -> Result<String, ()> {
        let line = self.previous().line;
        self.consume(TokenType::OpenMmap, "Expect 'open_mmap after 'with'.")?;
        self.consume(TokenType::LeftParen, "Expect '(' after open_mmap.")?;
        let path_expr = self.expression()?;
        self.consume(TokenType::RightParen, "Expect ')' after path.")?;
        
        self.consume(TokenType::As, "Expect 'as' in mmap block.")?;
        let var_token = self.consume(TokenType::Identifier, "Expect variable name after 'as'.")?.clone();
        let var_name = var_token.lexeme;

        if self.check(&TokenType::Colon) { self.advance(); }
        if self.check(&TokenType::Newline) { self.advance(); }

        let body = self.statement()?;
        Ok(format!("(with_mmap line:{} {} {} {})", line, path_expr, var_name, body))
    }

    fn parse_f_string(&mut self, content: &str, line: usize) -> Result<String, ()> {
        let mut parts = Vec::new();
        let mut chars = content.chars().peekable();
        let mut current_text = String::new();

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                if let Some(next_ch) = chars.next() {
                    current_text.push('\\');
                    current_text.push(next_ch);
                } else {
                    current_text.push('\\');
                }
            } else if ch == '{' {
                if chars.peek() == Some(&'{') {
                    chars.next();
                    current_text.push('{');
                    continue;
                }

                if !current_text.is_empty() {
                    parts.push(format!("\"{}\"", current_text));
                    current_text.clear();
                }

                let mut expr_str = String::new();
                let mut brace_depth = 1;
                let mut in_string = false;
                let mut string_char = ' ';

                while let Some(c) = chars.next() {
                    if in_string {
                        expr_str.push(c);
                        if c == string_char {
                            in_string = false;
                        }
                    } else {
                        if c == '"' || c == '\'' {
                            in_string = true;
                            string_char = c;
                            expr_str.push(c);
                        } else if c == '{' {
                            brace_depth += 1;
                            expr_str.push(c);
                        } else if c == '}' {
                            brace_depth -= 1;
                            if brace_depth == 0 {
                                break;
                            } else {
                                expr_str.push(c);
                            }
                        } else {
                            expr_str.push(c);
                        }
                    }
                }

                if brace_depth > 0 {
                    eprintln!("[line {}] Error: Unterminated expression in f-string.", line);
                    return Err(());
                }

                let (sub_tokens, err) = crate::scanner::scan_tokens(&expr_str);
                if err {
                    eprintln!("[line {}] Error: Failed to parse expression inside f-string: '{}'.", line, expr_str);
                    return Err(());
                }
                let mut sub_parser = Parser::new(sub_tokens);
                let sub_ast = sub_parser.expression()?;
                parts.push(sub_ast);
            } else if ch == '}' {
                if chars.peek() == Some(&'}') {
                    chars.next();
                    current_text.push('}');
                    continue;
                }
                current_text.push('}');
            } else {
                current_text.push(ch);
            }
        }

        if !current_text.is_empty() {
            parts.push(format!("\"{}\"", current_text));
        }

        Ok(format!("(f_string line:{} [{}])", line, parts.join(" ")))
    }

    fn if_statement(&mut self) -> Result<String, ()> {
        let condition = self.expression()?;

        if self.check(&TokenType::Colon) {
            self.advance();
        }

        let then_branch = self.statement()?;

        if self.match_types(&[TokenType::Elif]) {
            let else_branch = self.if_statement()?;
            Ok(format!("(if {} {} {})", condition, then_branch, else_branch))
        } else if self.match_types(&[TokenType::Else]) {
            if self.check(&TokenType::Colon) {
                self.advance();
            }
            let else_branch = self.statement()?;
            Ok(format!("(if {} {} {})", condition, then_branch, else_branch)) 
        } else {
            Ok(format!("(if {} {})", condition, then_branch))
        }
    }

    fn while_statement(&mut self) -> Result<String, ()> {
        let line = self.previous().line;
        let condition = self.expression()?;

        if self.check(&TokenType::Colon) {
            self.advance();
        }

        if self.check(&TokenType::Newline) {
            self.advance();
        }

        let body = self.statement()?;

        Ok(format!("(while line:{} {} {})", line, condition, body))
    }

    fn for_statement(&mut self, is_parallel: bool, is_vectorized: bool) -> Result<String, ()> {
        let line = self.previous().line;
        let var_token = self.consume(TokenType::Identifier, "Expect variable name after 'for'.")?.clone();
        let var_name = var_token.lexeme;

        self.consume(TokenType::In, "Expect 'in' after loop variable.")?;

        let start_expr;
        let end_expr ;
        
        if self.match_types(&[TokenType::Range]) {
            self.consume(TokenType::LeftParen, "Expect '(' after 'range'.")?;
            let first_arg = self.expression()?;

            if self.match_types(&[TokenType::Comma]) {
                start_expr = first_arg;
                end_expr = self.expression()?;
            } else {
                start_expr = "0".to_string();
                end_expr = first_arg;
            }

            self.consume(TokenType::RightParen, "Expect ')' after range arguments.")?;
        } else {
            start_expr = "0".to_string();
            end_expr = self.expression()?;
        }

        if self.check(&TokenType::Colon) {
            self.advance();
        }

        if self.check(&TokenType::Newline) {
            self.advance();
        }

        let body = self.statement()?;

        let loop_tag = match (is_parallel, is_vectorized) {
            (true, true) => "for_par_vec",
            (true, false) => "for_par",
            (false, true) => "for_vec",
            (false, false) => "for_seq",
        };

        Ok(format!("({} line:{} {} {} {} {})", loop_tag, line, var_name, start_expr, end_expr, body))
    }

    fn return_statement(&mut self) -> Result<String, ()> {
        let line = self.previous().line;
        let mut value = "None".to_string();

        if !self.check(&TokenType::EOF) {
            value = self.expression()?;
        }

        Ok(format!("(return line:{} {})", line, value))
    }

    fn print_statement(&mut self) -> Result<String, ()> {
        let line = self.previous().line;
        let mut value_exprs = Vec::new();

        self.consume(TokenType::LeftParen, "Expect '(' after 'print'.")?;

        if !self.check(&TokenType::RightParen) {
            loop {
                value_exprs.push(self.expression()?);
                if !self.match_types(&[TokenType::Comma]) {
                    break;
                }
            }
        }

        self.consume(TokenType::RightParen, "Expect ')' after print arguments.")?;
        Ok(format!("(print line:{} {})", line, value_exprs.join(" ")))
    }

    fn block(&mut self) -> Result<String, ()> {
        let mut statements = Vec::new();

        self.consume(TokenType::Indent, "Expect indentation at start of block.")?;

        while !self.check(&TokenType::Dedent) && !self.is_at_end() {
            statements.push(self.declaration()?);
        }

        self.consume(TokenType::Dedent, "Expect dedent at end of block.")?;
        
        let inner_stmts = statements.join(" ");
        Ok(format!("(block {})", inner_stmts))
    }

    fn expression_statement(&mut self) -> Result<String, ()> {
        let line = self.peek().line;
        let expr = self.expression()?;
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