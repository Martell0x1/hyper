//! Standard input for the compile path (`input()`).

use super::{format_value, RtValue, KIND_NONE, KIND_STR};
use crate::error;
use std::ffi::CString;
use std::io::{self, Write};
use std::os::raw::c_char;

fn cstr_payload(text: &str) -> i64 {
    CString::new(text).unwrap_or_default().into_raw() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_input(
    prompt: i64,
    prompt_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    if prompt_kind != KIND_NONE {
        if prompt_kind == KIND_STR {
            if prompt != 0 {
                let cstr = unsafe { std::ffi::CStr::from_ptr(prompt as *const c_char) };
                if let Ok(text) = cstr.to_str() {
                    print!("{}", text);
                }
            }
        } else {
            print!(
                "{}",
                format_value(&RtValue {
                    kind: prompt_kind,
                    payload: prompt,
                })
            );
        }
        println!();
        let _ = io::stdout().flush();
    }

    let mut buffer = String::new();
    if io::stdin().read_line(&mut buffer).is_err() {
        error::runtime(line as u32, "failed to read line from stdin");
    }
    let trimmed = buffer.trim_end_matches(&['\r', '\n'][..]).to_string();
    cstr_payload(&trimmed)
}
