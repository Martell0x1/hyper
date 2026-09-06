//! Python-like builtins for the compile path (`len`, `abs`, `min`, …).

use super::{
    hyper_rt_coll_len, hyper_rt_div_by_zero, hyper_rt_floor_div_f64, hyper_rt_floor_div_i64,
    hyper_rt_list_new, hyper_rt_list_push, hyper_rt_pow_f64, hyper_rt_pow_i64,
    hyper_rt_value_to_str, RtDict, RtList, RtValue, KIND_BOOL, KIND_DICT, KIND_F64, KIND_I64,
    KIND_LIST, KIND_NONE, KIND_STR, KIND_U64,
};
use crate::error;
use std::ffi::CStr;
use std::os::raw::c_char;

fn fatal(line: i64, msg: impl Into<String>) -> ! {
    error::runtime(line as u32, msg.into());
}

fn cstr_payload(text: &str) -> i64 {
    super::heap_cstr(text)
}

fn cstr_to_str<'a>(payload: i64) -> &'a str {
    if payload == 0 {
        return "";
    }
    unsafe { CStr::from_ptr(payload as *const c_char) }
        .to_str()
        .unwrap_or("")
}

fn set_out_kind(out_kind: *mut i64, kind: i64) {
    if !out_kind.is_null() {
        unsafe {
            *out_kind = kind;
        }
    }
}

fn clone_item(payload: i64, kind: i64) -> i64 {
    if kind == KIND_STR {
        cstr_payload(cstr_to_str(payload))
    } else {
        payload
    }
}

fn as_f64(payload: i64, kind: i64, line: i64, ctx: &str) -> f64 {
    match kind {
        KIND_I64 => payload as f64,
        KIND_U64 => (payload as u64) as f64,
        KIND_F64 => f64::from_bits(payload as u64),
        KIND_BOOL => payload as f64,
        _ => fatal(line, format!("{ctx}: expected a number")),
    }
}

fn is_numeric(kind: i64) -> bool {
    matches!(kind, KIND_I64 | KIND_U64 | KIND_F64 | KIND_BOOL)
}

fn list_items(payload: i64, kind: i64, line: i64, ctx: &str) -> &'static [RtValue] {
    if kind != KIND_LIST {
        fatal(line, format!("{ctx}: expected a list"));
    }
    if payload == 0 {
        return &[];
    }
    let list = unsafe { &*(payload as *const RtList) };
    list.items.as_slice()
}

fn is_truthy(payload: i64, kind: i64) -> bool {
    match kind {
        KIND_NONE => false,
        KIND_BOOL => payload != 0,
        KIND_I64 | KIND_U64 => payload != 0,
        KIND_F64 => f64::from_bits(payload as u64) != 0.0,
        KIND_STR => !cstr_to_str(payload).is_empty(),
        KIND_LIST => {
            if payload == 0 {
                false
            } else {
                let list = unsafe { &*(payload as *const RtList) };
                !list.items.is_empty()
            }
        }
        KIND_DICT => {
            if payload == 0 {
                false
            } else {
                let dict = unsafe { &*(payload as *const RtDict) };
                !dict.entries.is_empty()
            }
        }
        _ => true,
    }
}

/// `len(x)` — same semantics as `hyper_rt_coll_len`.
#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_len(
    payload: i64,
    kind: i64,
    line: i64,
    line_kind: i64,
) -> i64 {
    hyper_rt_coll_len(payload, kind, line, line_kind)
}

/// `abs(x)` — i64 / f64 / u64; preserves kind when possible.
#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_abs(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    match kind {
        KIND_I64 => {
            set_out_kind(out_kind, KIND_I64);
            payload.wrapping_abs()
        }
        KIND_U64 => {
            set_out_kind(out_kind, KIND_U64);
            payload
        }
        KIND_F64 => {
            set_out_kind(out_kind, KIND_F64);
            f64::from_bits(payload as u64).abs().to_bits() as i64
        }
        _ => fatal(line, "abs() expected a number"),
    }
}

fn min_max_list(
    payload: i64,
    kind: i64,
    line: i64,
    out_kind: *mut i64,
    want_max: bool,
    name: &str,
) -> i64 {
    let items = list_items(payload, kind, line, name);
    if items.is_empty() {
        fatal(line, format!("{name}() arg is an empty sequence"));
    }
    let mut best = items[0].clone();
    if !is_numeric(best.kind) {
        fatal(line, format!("{name}() expected numeric list elements"));
    }
    let mut use_float = best.kind == KIND_F64;
    for item in &items[1..] {
        if !is_numeric(item.kind) {
            fatal(line, format!("{name}() expected numeric list elements"));
        }
        if item.kind == KIND_F64 {
            use_float = true;
        }
        let left = as_f64(best.payload, best.kind, line, name);
        let right = as_f64(item.payload, item.kind, line, name);
        let take = if want_max {
            right > left
        } else {
            right < left
        };
        if take {
            best = item.clone();
        }
    }
    if use_float {
        let v = as_f64(best.payload, best.kind, line, name);
        set_out_kind(out_kind, KIND_F64);
        v.to_bits() as i64
    } else if best.kind == KIND_U64 {
        set_out_kind(out_kind, KIND_U64);
        best.payload
    } else {
        set_out_kind(out_kind, KIND_I64);
        // Promote bool to i64.
        best.payload
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_min(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    min_max_list(payload, kind, line, out_kind, false, "min")
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_max(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    min_max_list(payload, kind, line, out_kind, true, "max")
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_sum(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    let items = list_items(payload, kind, line, "sum");
    let mut use_float = false;
    for item in items {
        if !is_numeric(item.kind) {
            fatal(line, "sum() expected a list of numbers");
        }
        if item.kind == KIND_F64 {
            use_float = true;
        }
    }
    if use_float {
        let mut acc = 0.0f64;
        for item in items {
            acc += as_f64(item.payload, item.kind, line, "sum");
        }
        set_out_kind(out_kind, KIND_F64);
        acc.to_bits() as i64
    } else {
        let mut acc: i64 = 0;
        for item in items {
            match item.kind {
                KIND_U64 => acc = acc.wrapping_add(item.payload),
                _ => acc = acc.wrapping_add(item.payload),
            }
        }
        set_out_kind(out_kind, KIND_I64);
        acc
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_round(
    payload: i64,
    kind: i64,
    ndigits: i64,
    ndigits_kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    let x = match kind {
        KIND_I64 => payload as f64,
        KIND_U64 => (payload as u64) as f64,
        KIND_F64 => f64::from_bits(payload as u64),
        KIND_BOOL => payload as f64,
        _ => fatal(line, "round() expected a number"),
    };
    if ndigits_kind == KIND_NONE {
        set_out_kind(out_kind, KIND_I64);
        return x.round() as i64;
    }
    if ndigits_kind != KIND_I64 && ndigits_kind != KIND_U64 {
        fatal(line, "round() ndigits must be an integer");
    }
    let n = ndigits;
    let factor = 10f64.powi(n as i32);
    let rounded = (x * factor).round() / factor;
    set_out_kind(out_kind, KIND_F64);
    rounded.to_bits() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_pow(
    base: i64,
    base_kind: i64,
    exp: i64,
    exp_kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    if !is_numeric(base_kind) || !is_numeric(exp_kind) {
        fatal(line, "pow() expected numbers");
    }
    let base_is_int = matches!(base_kind, KIND_I64 | KIND_U64 | KIND_BOOL);
    let exp_is_int = matches!(exp_kind, KIND_I64 | KIND_U64 | KIND_BOOL);
    if base_is_int && exp_is_int && exp >= 0 {
        set_out_kind(out_kind, KIND_I64);
        return hyper_rt_pow_i64(base, exp);
    }
    let b = as_f64(base, base_kind, line, "pow");
    let e = as_f64(exp, exp_kind, line, "pow");
    set_out_kind(out_kind, KIND_F64);
    hyper_rt_pow_f64(b, e).to_bits() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_divmod(
    a: i64,
    a_kind: i64,
    b: i64,
    b_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    if !is_numeric(a_kind) || !is_numeric(b_kind) {
        fatal(line, "divmod() expected numbers");
    }
    let use_float = a_kind == KIND_F64 || b_kind == KIND_F64;
    let list = hyper_rt_list_new();
    if use_float {
        let af = as_f64(a, a_kind, line, "divmod");
        let bf = as_f64(b, b_kind, line, "divmod");
        if bf == 0.0 {
            hyper_rt_div_by_zero(line);
        }
        let q = hyper_rt_floor_div_f64(af, bf);
        let r = af - q * bf;
        hyper_rt_list_push(list, q.to_bits() as i64, KIND_F64);
        hyper_rt_list_push(list, r.to_bits() as i64, KIND_F64);
    } else {
        if b == 0 {
            hyper_rt_div_by_zero(line);
        }
        let q = hyper_rt_floor_div_i64(a, b);
        let r = a - q.wrapping_mul(b);
        hyper_rt_list_push(list, q, KIND_I64);
        hyper_rt_list_push(list, r, KIND_I64);
    }
    list
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_chr(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    if kind != KIND_I64 && kind != KIND_U64 {
        fatal(line, "chr() expected an integer");
    }
    let code = payload as u32;
    if payload < 0 || payload > 0x10FFFF {
        fatal(line, "chr() arg not in range(0x110000)");
    }
    if (0xD800..=0xDFFF).contains(&code) {
        fatal(line, "chr() arg is a surrogate");
    }
    let ch = match char::from_u32(code) {
        Some(c) => c,
        None => fatal(line, "chr() arg not in range(0x110000)"),
    };
    let mut buf = [0u8; 4];
    let s = ch.encode_utf8(&mut buf);
    cstr_payload(s)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_ord(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    if kind != KIND_STR {
        fatal(line, "ord() expected a string");
    }
    let s = cstr_to_str(payload);
    let mut chars = s.chars();
    match chars.next() {
        Some(c) => {
            if chars.next().is_some() {
                fatal(line, "ord() expected a character string of length 1");
            }
            c as u32 as i64
        }
        None => fatal(line, "ord() expected a character string of length 1"),
    }
}

fn format_radix(payload: i64, kind: i64, line: i64, name: &str, prefix: &str, radix: u32) -> i64 {
    if kind != KIND_I64 && kind != KIND_U64 && kind != KIND_BOOL {
        fatal(line, format!("{name}() expected an integer"));
    }
    let n = payload;
    if kind == KIND_U64 {
        let s = match radix {
            2 => format!("0b{:b}", payload as u64),
            8 => format!("0o{:o}", payload as u64),
            16 => format!("0x{:x}", payload as u64),
            _ => unreachable!(),
        };
        return cstr_payload(&s);
    }
    if n < 0 {
        let abs = n.wrapping_neg() as u64;
        let body = match radix {
            2 => format!("{abs:b}"),
            8 => format!("{abs:o}"),
            16 => format!("{abs:x}"),
            _ => unreachable!(),
        };
        cstr_payload(&format!("-{prefix}{body}"))
    } else {
        let body = match radix {
            2 => format!("{:b}", n as u64),
            8 => format!("{:o}", n as u64),
            16 => format!("{:x}", n as u64),
            _ => unreachable!(),
        };
        cstr_payload(&format!("{prefix}{body}"))
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_bin(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    format_radix(payload, kind, line, "bin", "0b", 2)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_hex(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    format_radix(payload, kind, line, "hex", "0x", 16)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_oct(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    format_radix(payload, kind, line, "oct", "0o", 8)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_int(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    set_out_kind(out_kind, KIND_I64);
    match kind {
        KIND_I64 | KIND_BOOL => payload,
        KIND_U64 => payload,
        KIND_F64 => f64::from_bits(payload as u64) as i64,
        KIND_STR => {
            let s = cstr_to_str(payload).trim();
            match s.parse::<i64>() {
                Ok(n) => n,
                Err(_) => fatal(line, format!("invalid literal for int(): '{s}'")),
            }
        }
        _ => fatal(line, "int() argument must be a number or string"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_float(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    set_out_kind(out_kind, KIND_F64);
    let v = match kind {
        KIND_I64 | KIND_BOOL => payload as f64,
        KIND_U64 => (payload as u64) as f64,
        KIND_F64 => f64::from_bits(payload as u64),
        KIND_STR => {
            let s = cstr_to_str(payload).trim();
            match s.parse::<f64>() {
                Ok(n) => n,
                Err(_) => fatal(line, format!("could not convert string to float: '{s}'")),
            }
        }
        _ => fatal(line, "float() argument must be a number or string"),
    };
    v.to_bits() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_str(
    payload: i64,
    kind: i64,
    _line: i64,
    _line_kind: i64,
) -> i64 {
    hyper_rt_value_to_str(payload, kind)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_bool(
    payload: i64,
    kind: i64,
    _line: i64,
    _line_kind: i64,
) -> i64 {
    if is_truthy(payload, kind) {
        1
    } else {
        0
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_all(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let items = list_items(payload, kind, line, "all");
    for item in items {
        if !is_truthy(item.payload, item.kind) {
            return 0;
        }
    }
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_any(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let items = list_items(payload, kind, line, "any");
    for item in items {
        if is_truthy(item.payload, item.kind) {
            return 1;
        }
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_sorted(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let items = list_items(payload, kind, line, "sorted");
    let out = hyper_rt_list_new();
    if items.is_empty() {
        return out;
    }
    let elem_kind = items[0].kind;
    for item in items {
        if item.kind != elem_kind {
            fatal(line, "sorted() requires a homogeneous list");
        }
    }
    match elem_kind {
        KIND_I64 | KIND_BOOL => {
            let mut vals: Vec<i64> = items.iter().map(|v| v.payload).collect();
            vals.sort_unstable();
            for v in vals {
                hyper_rt_list_push(out, v, elem_kind);
            }
        }
        KIND_F64 => {
            let mut vals: Vec<f64> = items
                .iter()
                .map(|v| f64::from_bits(v.payload as u64))
                .collect();
            vals.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            for v in vals {
                hyper_rt_list_push(out, v.to_bits() as i64, KIND_F64);
            }
        }
        KIND_STR => {
            let mut vals: Vec<String> = items
                .iter()
                .map(|v| cstr_to_str(v.payload).to_string())
                .collect();
            vals.sort();
            for v in vals {
                hyper_rt_list_push(out, cstr_payload(&v), KIND_STR);
            }
        }
        _ => fatal(line, "sorted() supports lists of int, float, or str"),
    }
    out
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_builtin_reversed(
    payload: i64,
    kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let items = list_items(payload, kind, line, "reversed");
    let out = hyper_rt_list_new();
    for item in items.iter().rev() {
        hyper_rt_list_push(out, clone_item(item.payload, item.kind), item.kind);
    }
    out
}
