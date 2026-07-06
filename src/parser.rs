pub fn run_parse(file_contents: String) {
    let mut chars = file_contents.chars().peekable();
    let mut unary_closes = 0; 
    let mut curr_expr = String::new();

    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {}
            '/' => {
                if chars.peek() == Some(&'/') {
                    while chars.peek() != Some(&'\n') && chars.peek().is_some() {
                        chars.next();
                    }
                } else {
                    curr_expr = format!("(/ {} ", curr_expr.trim());
                    unary_closes += 1;
                }
            }
            '*' => {
                curr_expr = format!("(* {} ", curr_expr.trim());
                unary_closes += 1;
            }
            '+' => {
                curr_expr = format!("(+ {} ", curr_expr.trim());
                unary_closes += 1;
            }
            '-' => {
                if curr_expr.trim().is_empty() {
                    curr_expr.push_str("(- ");
                    unary_closes += 1;
                } else {
                    curr_expr = format!("(- {} ", curr_expr.trim());
                    unary_closes += 1;
                }
            }
            '(' => {
                curr_expr.push_str("(group ");
            }
            ')' => {
                curr_expr.push(')');
                if unary_closes > 0 {
                    curr_expr.push(')');
                    unary_closes -= 1;
                }
            }
            '!' => {
                curr_expr.push_str("(! ");
                unary_closes += 1;
            }
            '"' => {
                let (str_val, is_terminated, _) = crate::scanner::str_literals(&mut chars);

                if is_terminated {
                    curr_expr.push_str(&str_val);
                    
                    while unary_closes > 0 {
                        curr_expr.push(')');
                        unary_closes -= 1;
                    }
                } else {
                    std::process::exit(65);
                }
            }
            '0'..='9' => {
                let num_str = crate::scanner::num_literals(ch, &mut chars);
                let num_val: f64 = num_str.parse().unwrap(); 

                let formatted_num = if num_val.fract() == 0.0 {
                    format!("{:.1}", num_val)
                } else {
                    format!("{}", num_val)
                };

                curr_expr.push_str(&formatted_num);

                while unary_closes > 0 {
                    curr_expr.push(')');
                    unary_closes -= 1;
                }
            }
            'a'..='z' | 'A'..='Z' | '_' => {
                let mut ident_str = String::new();
                ident_str.push(ch);

                while let Some(&next_ch) = chars.peek() {
                    if next_ch.is_ascii_alphanumeric() || next_ch == '_' {
                        ident_str.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }

                match ident_str.as_str() {
                    "true" => curr_expr.push_str("true"),
                    "false" => curr_expr.push_str("false"),
                    "nil" => curr_expr.push_str("nil"),
                    _ => std::process::exit(65),
                }

                while unary_closes > 0 {
                    curr_expr.push(')');
                    unary_closes -= 1;
                }
            }
            _ => std::process::exit(65),
        }
    }

    println!("{}", curr_expr.trim());
}