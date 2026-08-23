use crate::environment::HyperValue;

pub fn call_string_method(s: &str, method_name: &str, args: &[HyperValue], line: u32) -> Option<HyperValue> {
    match method_name {
        "strip" => {
            let trimmed = s.trim();
            if trimmed.len() == s.len() {
                return Some(HyperValue::String(s.to_string()));
            }
            Some(HyperValue::String(trimmed.to_string()))
        }
        "lstrip" => {
            let trimmed = s.trim_start();
            if trimmed.len() == s.len() {
                return Some(HyperValue::String(s.to_string()));
            }
            Some(HyperValue::String(trimmed.to_string()))
        }
        "rstrip" => {
            let trimmed = s.trim_end();
            if trimmed.len() == s.len() {
                return Some(HyperValue::String(s.to_string()));
            }
            Some(HyperValue::String(trimmed.to_string()))
        }
        "upper" => Some(HyperValue::String(s.to_uppercase())),
        "lower" => Some(HyperValue::String(s.to_lowercase())),
        "capitalize" => {
            let mut chars = s.chars();
            let capitalized = match chars.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().chain(chars.flat_map(|c| c.to_lowercase())).collect(),
            };
            Some(HyperValue::String(capitalized))
        }
        "title" => {
            let mut result = String::new();
            let mut capitalize_next = true;
            for c in s.chars() {
                if c.is_whitespace() || c.is_ascii_punctuation() {
                    result.push(c);
                    capitalize_next = true;
                } else if capitalize_next {
                    for uc in c.to_uppercase() {
                        result.push(uc);
                    }
                    capitalize_next = false;
                } else {
                    for lc in c.to_lowercase() {
                        result.push(lc);
                    }
                }
            }
            Some(HyperValue::String(result))
        }
        "join" => {
            if let Some(HyperValue::List(items)) = args.first() {
                let strs: Vec<String> = items.iter().map(|item| item.to_string()).collect();
                Some(HyperValue::String(strs.join(s)))
            } else {
                eprintln!("[line {}] Type Error: 'join' expects a list argument.", line);
                std::process::exit(70);
            }
        }
        "len" => Some(HyperValue::I64(s.chars().count() as i64)),
        "startswith" => {
            if let Some(HyperValue::String(sub)) = args.first() {
                Some(HyperValue::Boolean(s.starts_with(sub)))
            } else {
                eprintln!("[line {}] Type Error: 'startswith' expects a string argument.", line);
                std::process::exit(70);
            }
        }
        "endswith" => {
            if let Some(HyperValue::String(sub)) = args.first() {
                Some(HyperValue::Boolean(s.ends_with(sub)))
            } else {
                eprintln!("[line {}] Type Error: 'endswith' expects a string argument.", line);
                std::process::exit(70);
            }
        }
        "split" => {
            let delimiter = args.first().and_then(|v| match v {
                HyperValue::String(st) => Some(st.clone()),
                _ => None,
            }).unwrap_or_else(|| " ".to_string());

            let parts: Vec<HyperValue> = s
                .split(&delimiter)
                .map(|part| HyperValue::String(part.to_string()))
                .collect();
            Some(HyperValue::List(parts))
        }
        "rsplit" => {
            let delimiter = args.first().and_then(|v| match v {
                HyperValue::String(st) => Some(st.clone()),
                _ => None,
            }).unwrap_or_else(|| " ".to_string());

            let parts: Vec<HyperValue> = s
                .rsplit(&delimiter)
                .map(|part| HyperValue::String(part.to_string()))
                .collect();
            Some(HyperValue::List(parts))
        }
        "replace" => {
            if args.len() >= 2 {
                if let (Some(HyperValue::String(old_s)), Some(HyperValue::String(new_s))) = (args.get(0), args.get(1)) {
                    let replaced = s.replace(old_s, new_s);
                    return Some(HyperValue::String(replaced));
                }
            }
            eprintln!("[line {}] Type Error: 'replace' expects two string arguments.", line);
            std::process::exit(70);
        }
        "find" => {
            if let Some(HyperValue::String(sub)) = args.first() {
                if let Some(idx) = s.find(sub) {
                    let char_idx = s[..idx].chars().count();
                    Some(HyperValue::I64(char_idx as i64))
                } else {
                    Some(HyperValue::I64(-1))
                }
            } else {
                eprintln!("[line {}] Type Error: 'find' expects a string argument.", line);
                std::process::exit(70);
            }
        }
        "rfind" => {
            if let Some(HyperValue::String(sub)) = args.first() {
                if let Some(idx) = s.rfind(sub) {
                    let char_idx = s[..idx].chars().count();
                    Some(HyperValue::I64(char_idx as i64))
                } else {
                    Some(HyperValue::I64(-1))
                }
            } else {
                eprintln!("[line {}] Type Error: 'rfind' expects a string argument.", line);
                std::process::exit(70);
            }
        }
        "count" => {
            if let Some(HyperValue::String(sub)) = args.first() {
                if sub.is_empty() {
                    Some(HyperValue::I64((s.chars().count() + 1) as i64))
                } else {
                    let count = s.matches(sub).count();
                    Some(HyperValue::I64(count as i64))
                }
            } else {
                eprintln!("[line {}] Type Error: 'count' expects a string argument.", line);
                std::process::exit(70);
            }
        }
        "isdigit" => {
            let is_dig = !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
            Some(HyperValue::Boolean(is_dig))
        }
        "isalpha" => {
            let is_alp = !s.is_empty() && s.chars().all(|c| c.is_alphabetic());
            Some(HyperValue::Boolean(is_alp))
        }
        "isalnum" => {
            let is_aln = !s.is_empty() && s.chars().all(|c| c.is_alphanumeric());
            Some(HyperValue::Boolean(is_aln))
        }
        "isspace" => {
            let is_spc = !s.is_empty() && s.chars().all(|c| c.is_whitespace());
            Some(HyperValue::Boolean(is_spc))
        }
        _ => {
            eprintln!("[line {}] Attribute Error: String has no method '{}'.", line, method_name);
            std::process::exit(70);
        }
    }
}