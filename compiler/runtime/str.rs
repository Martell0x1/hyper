//! String methods for the compile path (Python-compatible surface, native speed).

use crate::error;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use super::{
    format_value, hyper_rt_list_get, hyper_rt_list_len, hyper_rt_list_new, hyper_rt_list_push,
    RtValue, KIND_I64, KIND_LIST, KIND_NONE, KIND_STR,
};

fn fatal(line: i64, msg: impl Into<String>) -> ! {
    error::runtime(line as u32, msg.into());
}

fn cstr_payload(text: &str) -> i64 {
    CString::new(text).unwrap_or_default().into_raw() as i64
}

fn require_str(payload: i64, kind: i64, line: i64) -> String {
    if kind != KIND_STR {
        fatal(line, "expected a string receiver");
    }
    if payload == 0 {
        String::new()
    } else {
        unsafe {
            CStr::from_ptr(payload as *const c_char)
                .to_str()
                .unwrap_or("")
                .to_string()
        }
    }
}

fn require_str_arg(payload: i64, kind: i64, line: i64, method: &str) -> String {
    if kind != KIND_STR {
        fatal(line, format!("'{method}' expects a string argument"));
    }
    require_str(payload, kind, line)
}

fn require_i64(payload: i64, kind: i64, line: i64, method: &str) -> i64 {
    if kind != KIND_I64 {
        fatal(line, format!("'{method}' expects an integer argument"));
    }
    payload
}

fn optional_fill(payload: i64, kind: i64, line: i64, method: &str) -> char {
    if kind == KIND_NONE {
        return ' ';
    }
    let s = require_str_arg(payload, kind, line, method);
    let mut chars = s.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => c,
        _ => fatal(line, format!("'{method}' fill character must be a single character")),
    }
}

fn bool_i(v: bool) -> i64 {
    if v {
        1
    } else {
        0
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
    let mut out = String::with_capacity(s.len() + pad);
    for _ in 0..left {
        out.push(fill);
    }
    out.push_str(s);
    for _ in 0..right {
        out.push(fill);
    }
    out
}

fn pad_left(s: &str, width: usize, fill: char) -> String {
    let len = s.chars().count();
    if width <= len {
        return s.to_string();
    }
    let mut out = String::new();
    for _ in 0..(width - len) {
        out.push(fill);
    }
    out.push_str(s);
    out
}

fn pad_right(s: &str, width: usize, fill: char) -> String {
    let len = s.chars().count();
    if width <= len {
        return s.to_string();
    }
    let mut out = s.to_string();
    for _ in 0..(width - len) {
        out.push(fill);
    }
    out
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
    let mut out = String::new();
    out.push_str(sign);
    for _ in 0..zeros {
        out.push('0');
    }
    out.push_str(body);
    out
}

fn split_whitespace(s: &str) -> Vec<String> {
    s.split_whitespace().map(|p| p.to_string()).collect()
}

fn split_sep(s: &str, sep: &str) -> Vec<String> {
    s.split(sep).map(|p| p.to_string()).collect()
}

fn rsplit_sep(s: &str, sep: &str) -> Vec<String> {
    // Without maxsplit, Python rsplit order matches split.
    let mut parts: Vec<String> = s.rsplit(sep).map(|p| p.to_string()).collect();
    parts.reverse();
    parts
}

fn push_str_list(parts: &[String]) -> i64 {
    let list = hyper_rt_list_new();
    for p in parts {
        hyper_rt_list_push(list, cstr_payload(p), KIND_STR);
    }
    list
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

fn count_sub(s: &str, sub: &str) -> i64 {
    if sub.is_empty() {
        (s.chars().count() + 1) as i64
    } else {
        s.matches(sub).count() as i64
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

macro_rules! str_transform {
    ($name:ident, $op:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(payload: i64, kind: i64, line: i64, _line_kind: i64) -> i64 {
            let s = require_str(payload, kind, line);
            cstr_payload(&$op(&s))
        }
    };
}

macro_rules! str_predicate {
    ($name:ident, $op:expr) => {
        #[unsafe(no_mangle)]
        pub extern "C" fn $name(payload: i64, kind: i64, line: i64, _line_kind: i64) -> i64 {
            let s = require_str(payload, kind, line);
            bool_i($op(&s))
        }
    };
}

str_transform!(hyper_rt_str_upper, |s: &str| s.to_uppercase());
str_transform!(hyper_rt_str_lower, |s: &str| s.to_lowercase());
str_transform!(hyper_rt_str_capitalize, capitalize);
str_transform!(hyper_rt_str_title, title_case);
str_transform!(hyper_rt_str_swapcase, swapcase);

str_predicate!(hyper_rt_str_isdigit, |s: &str| !s.is_empty()
    && s.chars().all(|c| c.is_ascii_digit()));
str_predicate!(hyper_rt_str_isalpha, |s: &str| !s.is_empty()
    && s.chars().all(|c| c.is_alphabetic()));
str_predicate!(hyper_rt_str_isalnum, |s: &str| !s.is_empty()
    && s.chars().all(|c| c.is_alphanumeric()));
str_predicate!(hyper_rt_str_isspace, |s: &str| !s.is_empty()
    && s.chars().all(|c| c.is_whitespace()));
str_predicate!(hyper_rt_str_islower, |s: &str| {
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
});
str_predicate!(hyper_rt_str_isupper, |s: &str| {
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
});
str_predicate!(hyper_rt_str_istitle, is_title);
str_predicate!(hyper_rt_str_isascii, |s: &str| s.is_ascii());

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_strip(payload: i64, kind: i64, line: i64, _line_kind: i64) -> i64 {
    let s = require_str(payload, kind, line);
    cstr_payload(s.trim())
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_lstrip(payload: i64, kind: i64, line: i64, _line_kind: i64) -> i64 {
    let s = require_str(payload, kind, line);
    cstr_payload(s.trim_start())
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_rstrip(payload: i64, kind: i64, line: i64, _line_kind: i64) -> i64 {
    let s = require_str(payload, kind, line);
    cstr_payload(s.trim_end())
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_startswith(
    payload: i64,
    kind: i64,
    prefix: i64,
    prefix_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let prefix = require_str_arg(prefix, prefix_kind, line, "startswith");
    bool_i(s.starts_with(&prefix))
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_endswith(
    payload: i64,
    kind: i64,
    suffix: i64,
    suffix_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let suffix = require_str_arg(suffix, suffix_kind, line, "endswith");
    bool_i(s.ends_with(&suffix))
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_split(
    payload: i64,
    kind: i64,
    delim: i64,
    delim_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let parts = if delim_kind == KIND_NONE {
        split_whitespace(&s)
    } else {
        let sep = require_str_arg(delim, delim_kind, line, "split");
        split_sep(&s, &sep)
    };
    push_str_list(&parts)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_rsplit(
    payload: i64,
    kind: i64,
    delim: i64,
    delim_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let parts = if delim_kind == KIND_NONE {
        split_whitespace(&s)
    } else {
        let sep = require_str_arg(delim, delim_kind, line, "rsplit");
        rsplit_sep(&s, &sep)
    };
    push_str_list(&parts)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_replace(
    payload: i64,
    kind: i64,
    old: i64,
    old_kind: i64,
    new: i64,
    new_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let old_s = require_str_arg(old, old_kind, line, "replace");
    let new_s = require_str_arg(new, new_kind, line, "replace");
    cstr_payload(&s.replace(&old_s, &new_s))
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_join(
    payload: i64,
    kind: i64,
    list: i64,
    list_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let sep = require_str(payload, kind, line);
    if list_kind != KIND_LIST {
        fatal(line, "'join' expects a list argument");
    }
    let n = hyper_rt_list_len(list);
    let mut parts = Vec::with_capacity(n as usize);
    for i in 0..n {
        let mut out_kind = 0i64;
        let item = hyper_rt_list_get(list, i, &mut out_kind);
        parts.push(format_value(&RtValue {
            kind: out_kind,
            payload: item,
        }));
    }
    cstr_payload(&parts.join(&sep))
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_find(
    payload: i64,
    kind: i64,
    sub: i64,
    sub_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let sub = require_str_arg(sub, sub_kind, line, "find");
    char_find(&s, &sub)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_rfind(
    payload: i64,
    kind: i64,
    sub: i64,
    sub_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let sub = require_str_arg(sub, sub_kind, line, "rfind");
    char_rfind(&s, &sub)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_index(
    payload: i64,
    kind: i64,
    sub: i64,
    sub_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let sub = require_str_arg(sub, sub_kind, line, "index");
    let idx = char_find(&s, &sub);
    if idx < 0 {
        fatal(line, "substring not found");
    }
    idx
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_rindex(
    payload: i64,
    kind: i64,
    sub: i64,
    sub_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let sub = require_str_arg(sub, sub_kind, line, "rindex");
    let idx = char_rfind(&s, &sub);
    if idx < 0 {
        fatal(line, "substring not found");
    }
    idx
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_count(
    payload: i64,
    kind: i64,
    sub: i64,
    sub_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let sub = require_str_arg(sub, sub_kind, line, "count");
    count_sub(&s, &sub)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_center(
    payload: i64,
    kind: i64,
    width: i64,
    width_kind: i64,
    fill: i64,
    fill_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let w = require_i64(width, width_kind, line, "center");
    let fill_ch = optional_fill(fill, fill_kind, line, "center");
    cstr_payload(&pad_center(&s, w.max(0) as usize, fill_ch))
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_ljust(
    payload: i64,
    kind: i64,
    width: i64,
    width_kind: i64,
    fill: i64,
    fill_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let w = require_i64(width, width_kind, line, "ljust");
    let fill_ch = optional_fill(fill, fill_kind, line, "ljust");
    cstr_payload(&pad_right(&s, w.max(0) as usize, fill_ch))
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_rjust(
    payload: i64,
    kind: i64,
    width: i64,
    width_kind: i64,
    fill: i64,
    fill_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let w = require_i64(width, width_kind, line, "rjust");
    let fill_ch = optional_fill(fill, fill_kind, line, "rjust");
    cstr_payload(&pad_left(&s, w.max(0) as usize, fill_ch))
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_zfill(
    payload: i64,
    kind: i64,
    width: i64,
    width_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let w = require_i64(width, width_kind, line, "zfill");
    cstr_payload(&zfill(&s, w.max(0) as usize))
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_removeprefix(
    payload: i64,
    kind: i64,
    prefix: i64,
    prefix_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let prefix = require_str_arg(prefix, prefix_kind, line, "removeprefix");
    cstr_payload(s.strip_prefix(&prefix).unwrap_or(&s))
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_removesuffix(
    payload: i64,
    kind: i64,
    suffix: i64,
    suffix_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let suffix = require_str_arg(suffix, suffix_kind, line, "removesuffix");
    cstr_payload(s.strip_suffix(&suffix).unwrap_or(&s))
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_partition(
    payload: i64,
    kind: i64,
    sep: i64,
    sep_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let sep = require_str_arg(sep, sep_kind, line, "partition");
    let (a, b, c) = match s.split_once(&sep) {
        Some((before, after)) => (before.to_string(), sep, after.to_string()),
        None => (s, String::new(), String::new()),
    };
    push_str_list(&[a, b, c])
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_rpartition(
    payload: i64,
    kind: i64,
    sep: i64,
    sep_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let s = require_str(payload, kind, line);
    let sep = require_str_arg(sep, sep_kind, line, "rpartition");
    let (a, b, c) = match s.rsplit_once(&sep) {
        Some((before, after)) => (before.to_string(), sep, after.to_string()),
        None => (String::new(), String::new(), s),
    };
    push_str_list(&[a, b, c])
}
