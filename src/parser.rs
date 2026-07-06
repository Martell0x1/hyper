pub fn run_parse(file_contents: String) {
    let mut chars = file_contents.chars().peekable();
    let mut unary_closes = 0; 

    while let Some(ch) = chars.next() {
        match ch {
            ' ' | '\t' | '\r' | '\n' => {}
            '/' => {
                if chars.peek() == Some(&'/') {
                    while chars.peek() != Some(&'\n') && chars.peek().is_some() {
                        chars.next();
                    }
                }
            }
            '(' => {
                print!("(group ");
            }
            ')' => {
                print!(")");
                if unary_closes > 0 {
                    print!(")");
                    unary_closes -= 1;
                }
            }
            '!' => {
                print!("(! ");
                unary_closes += 1;
            }
            '-' => {
                print!("(-");
                unary_closes += 1;
            }
            '"' => {
                let (str_val, is_terminated, _) = crate::scanner::str_literals(&mut chars);

                if is_terminated {
                    print!("{}", str_val);
                    
                    while unary_closes > 0 {
                        print!(")");
                        unary_closes -= 1;
                    }
                } else {
                    std::process::exit(65);
                }
            }
            '0'..='9' => {
                let num_str = crate::scanner::num_literals(ch, &mut chars);
                let num_val: f64 = num_str.parse().unwrap(); 

                if num_val.fract() == 0.0 {
                    print!("{:.1}", num_val)
                } else {
                    print!("{}", num_val)
                };

                while unary_closes > 0 {
                    print!(")");
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
                    "true" => print!("true"),
                    "false" => print!("false"),
                    "nil" => print!("nil"),
                    _ => std::process::exit(65),
                }

                while unary_closes > 0 {
                    print!(")");
                    unary_closes -= 1;
                }
            }
            _ => std::process::exit(65),
        }
    }

    println!()
}