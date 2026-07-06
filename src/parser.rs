pub fn run_parse(file_contents: String) {
    let mut chars = file_contents.chars().peekable();

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
            '"' => {
                let (str_val, is_terminated, _) = crate::scanner::str_literals(&mut chars);

                if is_terminated {
                    println!("{}", str_val);
                } else {
                    std::process::exit(65);
                }
            }
            '0'..='9' => {
                let num_str = crate::scanner::num_literals(ch, &mut chars);
                let num_val: f64 = num_str.parse().unwrap(); 

                if num_val.fract() == 0.0 {
                    println!("{:.1}", num_val)
                } else {
                    println!("{}", num_val)
                };
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
                    "true" => println!("true"),
                    "false" => println!("false"),
                    "nil" => println!("nil"),
                    _ => std::process::exit(65),
                }
            }
            _ => std::process::exit(65),
        }
    }
}