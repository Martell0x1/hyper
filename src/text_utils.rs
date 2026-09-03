use crate::environment::HyperValue;
use crate::error;

fn expect_args(method: &str, args: &[HyperValue], min: usize, max: usize, line: u32) {
    if args.len() < min || args.len() > max {
        let expected = if min == max {
            min.to_string()
        } else {
            format!("{min}-{max}")
        };
        error::runtime(
            line,
            format!("{method} expects {expected} argument(s) but got {}", args.len()),
        );
    }
}

fn as_str_arg(method: &str, arg: &HyperValue, line: u32) -> String {
    match arg {
        HyperValue::String(s) => s.clone(),
        _ => error::runtime(line, format!("'{method}' expects a string argument")),
    }
}

fn as_i64_arg(method: &str, arg: &HyperValue, line: u32) -> i64 {
    match arg {
        HyperValue::I64(n) => *n,
        _ => error::runtime(line, format!("'{method}' expects an integer argument")),
    }
}

fn fill_char(method: &str, args: &[HyperValue], line: u32) -> char {
    if args.len() < 2 {
        return ' ';
    }
    let s = as_str_arg(method, &args[1], line);
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c,
        _ => error::runtime(
            line,
            format!("'{method}' fill character must be a single character"),
        ),
    }
}

fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(f) => f
            .to_uppercase()
            .chain(chars.flat_map(|c| c.to_lowercase()))
            .collect(),
    }
}

fn title_case(s: &str) -> String {
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
    result
}

fn swapcase(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_lowercase() {
                c.to_uppercase().collect::<String>()
            } else if c.is_uppercase() {
                c.to_lowercase().collect::<String>()
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn pad_center(s: &str, width: usize, fill: char) -> String {
    let len = s.chars().count();
    if width <= len {
        return s.to_string();
    }
    let pad = width - len;
    let left = pad / 2;
    let right = pad - left;
    format!("{}{}{}", fill.to_string().repeat(left), s, fill.to_string().repeat(right))
}

fn pad_left(s: &str, width: usize, fill: char) -> String {
    let len = s.chars().count();
    if width <= len {
        return s.to_string();
    }
    format!("{}{}", fill.to_string().repeat(width - len), s)
}

fn pad_right(s: &str, width: usize, fill: char) -> String {
    let len = s.chars().count();
    if width <= len {
        return s.to_string();
    }
    format!("{}{}", s, fill.to_string().repeat(width - len))
}

fn zfill(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if width <= len {
        return s.to_string();
    }
    let (sign, body) = if let Some(rest) = s.strip_prefix('+').or_else(|| s.strip_prefix('-')) {
        (&s[..1], rest)
    } else {
        ("", s)
    };
    let body_len = body.chars().count() + sign.chars().count();
    let zeros = width.saturating_sub(body_len);
    format!("{sign}{}{body}", "0".repeat(zeros))
}

fn char_find(s: &str, sub: &str) -> i64 {
    match s.find(sub) {
        Some(byte_idx) => s[..byte_idx].chars().count() as i64,
        None => -1,
    }
}

fn char_rfind(s: &str, sub: &str) -> i64 {
    match s.rfind(sub) {
        Some(byte_idx) => s[..byte_idx].chars().count() as i64,
        None => -1,
    }
}

fn is_title(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    let mut saw_cased = false;
    let mut expect_upper = true;
    for c in s.chars() {
        if c.is_whitespace() || c.is_ascii_punctuation() {
            expect_upper = true;
            continue;
        }
        if c.is_uppercase() {
            if !expect_upper {
                return false;
            }
            saw_cased = true;
            expect_upper = false;
        } else if c.is_lowercase() {
            if expect_upper {
                return false;
            }
            saw_cased = true;
        }
    }
    saw_cased
}

fn is_lower(s: &str) -> bool {
    let mut has = false;
    for c in s.chars() {
        if c.is_uppercase() {
            return false;
        }
        if c.is_lowercase() {
            has = true;
        }
    }
    has
}

fn is_upper(s: &str) -> bool {
    let mut has = false;
    for c in s.chars() {
        if c.is_lowercase() {
            return false;
        }
        if c.is_uppercase() {
            has = true;
        }
    }
    has
}

pub fn call_string_method(
    s: &str,
    method_name: &str,
    args: &[HyperValue],
    line: u32,
) -> Option<HyperValue> {
    match method_name {
        "strip" => {
            expect_args("strip", args, 0, 0, line);
            Some(HyperValue::String(s.trim().to_string()))
        }
        "lstrip" => {
            expect_args("lstrip", args, 0, 0, line);
            Some(HyperValue::String(s.trim_start().to_string()))
        }
        "rstrip" => {
            expect_args("rstrip", args, 0, 0, line);
            Some(HyperValue::String(s.trim_end().to_string()))
        }
        "upper" => {
            expect_args("upper", args, 0, 0, line);
            Some(HyperValue::String(s.to_uppercase()))
        }
        "lower" => {
            expect_args("lower", args, 0, 0, line);
            Some(HyperValue::String(s.to_lowercase()))
        }
        "capitalize" => {
            expect_args("capitalize", args, 0, 0, line);
            Some(HyperValue::String(capitalize(s)))
        }
        "title" => {
            expect_args("title", args, 0, 0, line);
            Some(HyperValue::String(title_case(s)))
        }
        "swapcase" => {
            expect_args("swapcase", args, 0, 0, line);
            Some(HyperValue::String(swapcase(s)))
        }
        "join" => {
            expect_args("join", args, 1, 1, line);
            if let HyperValue::List(items) = &args[0] {
                let strs: Vec<String> = items.iter().map(|item| item.to_string()).collect();
                Some(HyperValue::String(strs.join(s)))
            } else {
                error::runtime(line, "'join' expects a list argument");
            }
        }
        "len" => {
            expect_args("len", args, 0, 0, line);
            Some(HyperValue::I64(s.chars().count() as i64))
        }
        "startswith" => {
            expect_args("startswith", args, 1, 1, line);
            Some(HyperValue::Boolean(s.starts_with(&as_str_arg(
                "startswith",
                &args[0],
                line,
            ))))
        }
        "endswith" => {
            expect_args("endswith", args, 1, 1, line);
            Some(HyperValue::Boolean(s.ends_with(&as_str_arg(
                "endswith",
                &args[0],
                line,
            ))))
        }
        "split" => {
            expect_args("split", args, 0, 1, line);
            let parts: Vec<HyperValue> = if args.is_empty() {
                s.split_whitespace()
                    .map(|p| HyperValue::String(p.to_string()))
                    .collect()
            } else {
                let delim = as_str_arg("split", &args[0], line);
                s.split(&delim)
                    .map(|p| HyperValue::String(p.to_string()))
                    .collect()
            };
            Some(HyperValue::List(parts))
        }
        "rsplit" => {
            expect_args("rsplit", args, 0, 1, line);
            let parts: Vec<HyperValue> = if args.is_empty() {
                s.split_whitespace()
                    .map(|part| HyperValue::String(part.to_string()))
                    .collect()
            } else {
                let delim = as_str_arg("rsplit", &args[0], line);
                let mut parts: Vec<HyperValue> = s
                    .rsplit(&delim)
                    .map(|p| HyperValue::String(p.to_string()))
                    .collect();
                parts.reverse();
                parts
            };
            Some(HyperValue::List(parts))
        }
        "replace" => {
            expect_args("replace", args, 2, 2, line);
            let old_s = as_str_arg("replace", &args[0], line);
            let new_s = as_str_arg("replace", &args[1], line);
            Some(HyperValue::String(s.replace(&old_s, &new_s)))
        }
        "find" => {
            expect_args("find", args, 1, 1, line);
            Some(HyperValue::I64(char_find(
                s,
                &as_str_arg("find", &args[0], line),
            )))
        }
        "rfind" => {
            expect_args("rfind", args, 1, 1, line);
            Some(HyperValue::I64(char_rfind(
                s,
                &as_str_arg("rfind", &args[0], line),
            )))
        }
        "index" => {
            expect_args("index", args, 1, 1, line);
            let idx = char_find(s, &as_str_arg("index", &args[0], line));
            if idx < 0 {
                error::runtime(line, "substring not found");
            }
            Some(HyperValue::I64(idx))
        }
        "rindex" => {
            expect_args("rindex", args, 1, 1, line);
            let idx = char_rfind(s, &as_str_arg("rindex", &args[0], line));
            if idx < 0 {
                error::runtime(line, "substring not found");
            }
            Some(HyperValue::I64(idx))
        }
        "count" => {
            expect_args("count", args, 1, 1, line);
            let sub = as_str_arg("count", &args[0], line);
            let n = if sub.is_empty() {
                (s.chars().count() + 1) as i64
            } else {
                s.matches(&sub).count() as i64
            };
            Some(HyperValue::I64(n))
        }
        "isdigit" => {
            expect_args("isdigit", args, 0, 0, line);
            Some(HyperValue::Boolean(
                !s.is_empty() && s.chars().all(|c| c.is_ascii_digit()),
            ))
        }
        "isalpha" => {
            expect_args("isalpha", args, 0, 0, line);
            Some(HyperValue::Boolean(
                !s.is_empty() && s.chars().all(|c| c.is_alphabetic()),
            ))
        }
        "isalnum" => {
            expect_args("isalnum", args, 0, 0, line);
            Some(HyperValue::Boolean(
                !s.is_empty() && s.chars().all(|c| c.is_alphanumeric()),
            ))
        }
        "isspace" => {
            expect_args("isspace", args, 0, 0, line);
            Some(HyperValue::Boolean(
                !s.is_empty() && s.chars().all(|c| c.is_whitespace()),
            ))
        }
        "islower" => {
            expect_args("islower", args, 0, 0, line);
            Some(HyperValue::Boolean(is_lower(s)))
        }
        "isupper" => {
            expect_args("isupper", args, 0, 0, line);
            Some(HyperValue::Boolean(is_upper(s)))
        }
        "istitle" => {
            expect_args("istitle", args, 0, 0, line);
            Some(HyperValue::Boolean(is_title(s)))
        }
        "isascii" => {
            expect_args("isascii", args, 0, 0, line);
            Some(HyperValue::Boolean(s.is_ascii()))
        }
        "center" => {
            expect_args("center", args, 1, 2, line);
            let width = as_i64_arg("center", &args[0], line).max(0) as usize;
            let fill = fill_char("center", args, line);
            Some(HyperValue::String(pad_center(s, width, fill)))
        }
        "ljust" => {
            expect_args("ljust", args, 1, 2, line);
            let width = as_i64_arg("ljust", &args[0], line).max(0) as usize;
            let fill = fill_char("ljust", args, line);
            Some(HyperValue::String(pad_right(s, width, fill)))
        }
        "rjust" => {
            expect_args("rjust", args, 1, 2, line);
            let width = as_i64_arg("rjust", &args[0], line).max(0) as usize;
            let fill = fill_char("rjust", args, line);
            Some(HyperValue::String(pad_left(s, width, fill)))
        }
        "zfill" => {
            expect_args("zfill", args, 1, 1, line);
            let width = as_i64_arg("zfill", &args[0], line).max(0) as usize;
            Some(HyperValue::String(zfill(s, width)))
        }
        "removeprefix" => {
            expect_args("removeprefix", args, 1, 1, line);
            let prefix = as_str_arg("removeprefix", &args[0], line);
            Some(HyperValue::String(
                s.strip_prefix(&prefix).unwrap_or(s).to_string(),
            ))
        }
        "removesuffix" => {
            expect_args("removesuffix", args, 1, 1, line);
            let suffix = as_str_arg("removesuffix", &args[0], line);
            Some(HyperValue::String(
                s.strip_suffix(&suffix).unwrap_or(s).to_string(),
            ))
        }
        "partition" => {
            expect_args("partition", args, 1, 1, line);
            let sep = as_str_arg("partition", &args[0], line);
            let (a, b, c) = match s.split_once(&sep) {
                Some((before, after)) => (
                    before.to_string(),
                    sep,
                    after.to_string(),
                ),
                None => (s.to_string(), String::new(), String::new()),
            };
            Some(HyperValue::List(vec![
                HyperValue::String(a),
                HyperValue::String(b),
                HyperValue::String(c),
            ]))
        }
        "rpartition" => {
            expect_args("rpartition", args, 1, 1, line);
            let sep = as_str_arg("rpartition", &args[0], line);
            let (a, b, c) = match s.rsplit_once(&sep) {
                Some((before, after)) => (
                    before.to_string(),
                    sep,
                    after.to_string(),
                ),
                None => (String::new(), String::new(), s.to_string()),
            };
            Some(HyperValue::List(vec![
                HyperValue::String(a),
                HyperValue::String(b),
                HyperValue::String(c),
            ]))
        }
        _ => {
            error::runtime(line, format!("string has no method '{}'", method_name));
        }
    }
}
