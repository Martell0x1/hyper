//! String methods for the compile path (mirrors interpreter `text_utils`).

use crate::error;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use super::{hyper_rt_list_new, hyper_rt_list_push, KIND_NONE, KIND_STR};

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

fn optional_str_arg(payload: i64, kind: i64, default: &str) -> String {
    if kind == KIND_NONE {
        default.to_string()
    } else if kind == KIND_STR {
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
    } else {
        default.to_string()
    }
}

fn require_str_arg(payload: i64, kind: i64, line: i64, method: &str) -> String {
    if kind != KIND_STR {
        fatal(line, format!("'{method}' expects a string argument"));
    }
    require_str(payload, kind, line)
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

str_transform!(hyper_rt_str_upper, |s: &str| s.to_uppercase());
str_transform!(hyper_rt_str_lower, |s: &str| s.to_lowercase());

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_strip(payload: i64, kind: i64, line: i64, _line_kind: i64) -> i64 {
    let s = require_str(payload, kind, line);
    let trimmed = s.trim();
    if trimmed.len() == s.len() {
        cstr_payload(&s)
    } else {
        cstr_payload(trimmed)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_lstrip(payload: i64, kind: i64, line: i64, _line_kind: i64) -> i64 {
    let s = require_str(payload, kind, line);
    let trimmed = s.trim_start();
    if trimmed.len() == s.len() {
        cstr_payload(&s)
    } else {
        cstr_payload(trimmed)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_rstrip(payload: i64, kind: i64, line: i64, _line_kind: i64) -> i64 {
    let s = require_str(payload, kind, line);
    let trimmed = s.trim_end();
    if trimmed.len() == s.len() {
        cstr_payload(&s)
    } else {
        cstr_payload(trimmed)
    }
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
    if s.starts_with(&prefix) {
        1
    } else {
        0
    }
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
    if s.ends_with(&suffix) {
        1
    } else {
        0
    }
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
    let delimiter = optional_str_arg(delim, delim_kind, " ");
    let list = hyper_rt_list_new();
    if s.is_empty() {
        return list;
    }
    for part in s.split(&delimiter) {
        hyper_rt_list_push(list, cstr_payload(part), KIND_STR);
    }
    list
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
