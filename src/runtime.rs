use crate::error;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

pub const KIND_I64: i64 = 0;
pub const KIND_F64: i64 = 1;
pub const KIND_STR: i64 = 2;
pub const KIND_BOOL: i64 = 3;
pub const KIND_NONE: i64 = 4;
pub const KIND_LIST: i64 = 5;
pub const KIND_DICT: i64 = 6;
pub const KIND_STRUCT: i64 = 7;
pub const KIND_FILE: i64 = 8;

mod file;
pub use file::{
    hyper_rt_file_close, hyper_rt_file_flush, hyper_rt_file_is_closed, hyper_rt_file_mode,
    hyper_rt_file_open, hyper_rt_file_path, hyper_rt_file_read_all, hyper_rt_file_read_n,
    hyper_rt_file_readline, hyper_rt_file_readlines, hyper_rt_file_seek, hyper_rt_file_size,
    hyper_rt_file_tell, hyper_rt_file_write, hyper_rt_file_writelines,
};

#[derive(Clone)]
pub(crate) struct RtValue {
    kind: i64,
    payload: i64,
}

pub(crate) struct RtList {
    items: Vec<RtValue>,
}

struct RtDict {
    entries: Vec<(String, RtValue)>,
}

struct RtStruct {
    fields: Vec<RtValue>,
}

pub(crate) fn format_value(v: &RtValue) -> String {
    match v.kind {
        KIND_I64 => format!("{}", v.payload),
        KIND_F64 => format!("{}", f64::from_bits(v.payload as u64)),
        KIND_STR => {
            if v.payload == 0 {
                String::new()
            } else {
                let cstr = unsafe { CStr::from_ptr(v.payload as *const c_char) };
                cstr.to_str().unwrap_or("<?>").to_string()
            }
        }
        KIND_BOOL => {
            if v.payload != 0 {
                "true".to_string()
            } else {
                "false".to_string()
            }
        }
        KIND_NONE => "None".to_string(),
        KIND_LIST => {
            let list = unsafe { &*(v.payload as *const RtList) };
            format_list(list)
        }
        KIND_DICT => {
            let dict = unsafe { &*(v.payload as *const RtDict) };
            format_dict(dict)
        }
        KIND_STRUCT => {
            let st = unsafe { &*(v.payload as *const RtStruct) };
            format_struct(st)
        }
        KIND_FILE => "<file>".to_string(),
        _ => format!("<?>"),
    }
}

fn format_list(list: &RtList) -> String {
    let parts: Vec<String> = list.items.iter().map(format_value).collect();
    format!("[{}]", parts.join(", "))
}

fn format_dict(dict: &RtDict) -> String {
    let parts: Vec<String> = dict
        .entries
        .iter()
        .map(|(k, v)| format!("{}: {}", k, format_value(v)))
        .collect();
    format!("{{{}}}", parts.join(", "))
}

fn format_struct(st: &RtStruct) -> String {
    let parts: Vec<String> = st.fields.iter().map(format_value).collect();
    format!("{{{}}}", parts.join(", "))
}

fn key_to_string(key: i64, key_kind: i64) -> String {
    match key_kind {
        KIND_STR => {
            if key == 0 {
                String::new()
            } else {
                let cstr = unsafe { CStr::from_ptr(key as *const c_char) };
                cstr.to_str().unwrap_or("<?>").to_string()
            }
        }
        KIND_I64 => format!("{}", key),
        _ => format!("{}", key),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_i64(v: i64) {
    print!("{}", v);
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_f64(v: f64) {
    print!("{}", v);
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_str(s: *const i8) {
    if s.is_null() {
        return;
    }
    let cstr = unsafe { CStr::from_ptr(s) };
    match cstr.to_str() {
        Ok(text) => print!("{}", text),
        Err(_) => print!("<?>"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_newline() {
    println!();
}

/// Separator emitted between `print` arguments (interpreter joins with a space).
#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_separator() {
    print!(" ");
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_pow_i64(base: i64, exp: i64) -> i64 {
    if exp < 0 {
        return 0;
    }
    let mut result: i64 = 1;
    let mut b = base;
    let mut e = exp as u64;
    while e > 0 {
        if e & 1 == 1 {
            result = result.wrapping_mul(b);
        }
        b = b.wrapping_mul(b);
        e >>= 1;
    }
    result
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_pow_f64(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_floor_div_i64(a: i64, b: i64) -> i64 {
    a.div_euclid(b)
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_floor_div_f64(a: f64, b: f64) -> f64 {
    (a / b).floor()
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_list_new() -> i64 {
    let list = Box::new(RtList { items: Vec::new() });
    Box::into_raw(list) as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_list_push(list: i64, value: i64, kind: i64) {
    if list == 0 {
        return;
    }
    let list = unsafe { &mut *(list as *mut RtList) };
    list.items.push(RtValue {
        kind,
        payload: value,
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_list(list: i64) {
    if list == 0 {
        print!("[]");
        return;
    }
    let list = unsafe { &*(list as *const RtList) };
    print!("{}", format_list(list));
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_dict_new() -> i64 {
    let dict = Box::new(RtDict {
        entries: Vec::new(),
    });
    Box::into_raw(dict) as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_dict_push(
    dict: i64,
    key: i64,
    key_kind: i64,
    val: i64,
    val_kind: i64,
) {
    if dict == 0 {
        return;
    }
    let dict = unsafe { &mut *(dict as *mut RtDict) };
    let k = key_to_string(key, key_kind);
    dict.entries.push((
        k,
        RtValue {
            kind: val_kind,
            payload: val,
        },
    ));
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_dict(dict: i64) {
    if dict == 0 {
        print!("{{}}");
        return;
    }
    let dict = unsafe { &*(dict as *const RtDict) };
    print!("{}", format_dict(dict));
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_value(payload: i64, kind: i64) {
    print!("{}", format_value(&RtValue { kind, payload }));
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_list_get(list: i64, index: i64, out_kind: *mut i64) -> i64 {
    if list == 0 || out_kind.is_null() {
        return 0;
    }
    let list = unsafe { &*(list as *const RtList) };
    if index < 0 || index as usize >= list.items.len() {
        unsafe {
            *out_kind = KIND_NONE;
        }
        return 0;
    }
    let item = &list.items[index as usize];
    unsafe {
        *out_kind = item.kind;
    }
    item.payload
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_list_set(list: i64, index: i64, value: i64, kind: i64) {
    if list == 0 {
        return;
    }
    let list = unsafe { &mut *(list as *mut RtList) };
    if index < 0 || index as usize >= list.items.len() {
        return;
    }
    list.items[index as usize] = RtValue {
        kind,
        payload: value,
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_dict_get(
    dict: i64,
    key: i64,
    key_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    if dict == 0 || out_kind.is_null() {
        return 0;
    }
    let dict = unsafe { &*(dict as *const RtDict) };
    let k = key_to_string(key, key_kind);
    for (ek, ev) in &dict.entries {
        if ek == &k {
            unsafe {
                *out_kind = ev.kind;
            }
            return ev.payload;
        }
    }
    unsafe {
        *out_kind = KIND_NONE;
    }
    0
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_dict_set(
    dict: i64,
    key: i64,
    key_kind: i64,
    value: i64,
    val_kind: i64,
) {
    if dict == 0 {
        return;
    }
    let dict = unsafe { &mut *(dict as *mut RtDict) };
    let k = key_to_string(key, key_kind);
    for (ek, ev) in &mut dict.entries {
        if ek == &k {
            *ev = RtValue {
                kind: val_kind,
                payload: value,
            };
            return;
        }
    }
    dict.entries.push((
        k,
        RtValue {
            kind: val_kind,
            payload: value,
        },
    ));
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_list_len(list: i64) -> i64 {
    if list == 0 {
        return 0;
    }
    let list = unsafe { &*(list as *const RtList) };
    list.items.len() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_value_to_str(payload: i64, kind: i64) -> i64 {
    let s = format_value(&RtValue { kind, payload });
    match CString::new(s) {
        Ok(c) => c.into_raw() as i64,
        Err(_) => 0,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_str_concat(left: i64, right: i64) -> i64 {
    let a = if left == 0 {
        String::new()
    } else {
        let cstr = unsafe { CStr::from_ptr(left as *const c_char) };
        cstr.to_str().unwrap_or("").to_string()
    };
    let b = if right == 0 {
        String::new()
    } else {
        let cstr = unsafe { CStr::from_ptr(right as *const c_char) };
        cstr.to_str().unwrap_or("").to_string()
    };
    match CString::new(format!("{}{}", a, b)) {
        Ok(c) => c.into_raw() as i64,
        Err(_) => 0,
    }
}

fn cstr_to_str<'a>(payload: i64) -> &'a str {
    if payload == 0 {
        return "";
    }
    unsafe { CStr::from_ptr(payload as *const c_char) }
        .to_str()
        .unwrap_or("")
}

/// Mirrors the interpreter's `==`: strings, lists and dicts compare by content,
/// numbers promote to f64 when either side is a float, and struct instances are
/// never equal to anything.
fn values_equal(a: &RtValue, b: &RtValue) -> bool {
    match (a.kind, b.kind) {
        (KIND_STR, KIND_STR) => cstr_to_str(a.payload) == cstr_to_str(b.payload),
        (KIND_NONE, KIND_NONE) => true,
        (KIND_BOOL, KIND_BOOL) => a.payload == b.payload,
        (KIND_F64, KIND_F64) => f64::from_bits(a.payload as u64) == f64::from_bits(b.payload as u64),
        (KIND_F64, KIND_I64) => f64::from_bits(a.payload as u64) == b.payload as f64,
        (KIND_I64, KIND_F64) => a.payload as f64 == f64::from_bits(b.payload as u64),
        (KIND_I64, KIND_I64) => a.payload == b.payload,
        (KIND_LIST, KIND_LIST) => {
            if a.payload == 0 || b.payload == 0 {
                return a.payload == b.payload;
            }
            let left = unsafe { &*(a.payload as *const RtList) };
            let right = unsafe { &*(b.payload as *const RtList) };
            left.items.len() == right.items.len()
                && left
                    .items
                    .iter()
                    .zip(right.items.iter())
                    .all(|(x, y)| values_equal(x, y))
        }
        (KIND_DICT, KIND_DICT) => {
            if a.payload == 0 || b.payload == 0 {
                return a.payload == b.payload;
            }
            let left = unsafe { &*(a.payload as *const RtDict) };
            let right = unsafe { &*(b.payload as *const RtDict) };
            left.entries.len() == right.entries.len()
                && left.entries.iter().all(|(key, value)| {
                    right
                        .entries
                        .iter()
                        .any(|(other_key, other)| other_key == key && values_equal(value, other))
                })
        }
        _ => false,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_div_by_zero(line: i64) {
    error::runtime(line as u32, "division by zero");
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_value_eq(a: i64, a_kind: i64, b: i64, b_kind: i64) -> i64 {
    let left = RtValue {
        kind: a_kind,
        payload: a,
    };
    let right = RtValue {
        kind: b_kind,
        payload: b,
    };
    values_equal(&left, &right) as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_struct_new(nfields: i64) -> i64 {
    let n = if nfields < 0 { 0 } else { nfields as usize };
    let st = Box::new(RtStruct {
        fields: vec![
            RtValue {
                kind: KIND_NONE,
                payload: 0,
            };
            n
        ],
    });
    Box::into_raw(st) as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_struct_get(obj: i64, field: i64, out_kind: *mut i64) -> i64 {
    if obj == 0 || out_kind.is_null() {
        return 0;
    }
    let st = unsafe { &*(obj as *const RtStruct) };
    if field < 0 || field as usize >= st.fields.len() {
        unsafe {
            *out_kind = KIND_NONE;
        }
        return 0;
    }
    let item = &st.fields[field as usize];
    unsafe {
        *out_kind = item.kind;
    }
    item.payload
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_struct_set(obj: i64, field: i64, value: i64, kind: i64) {
    if obj == 0 {
        return;
    }
    let st = unsafe { &mut *(obj as *mut RtStruct) };
    if field < 0 || field as usize >= st.fields.len() {
        return;
    }
    st.fields[field as usize] = RtValue {
        kind,
        payload: value,
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_struct(obj: i64) {
    if obj == 0 {
        print!("{{}}");
        return;
    }
    let st = unsafe { &*(obj as *const RtStruct) };
    print!("{}", format_struct(st));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn str_payload(text: &str) -> i64 {
        CString::new(text).unwrap().into_raw() as i64
    }

    fn list_of(items: &[(i64, i64)]) -> i64 {
        let list = hyper_rt_list_new();
        for (payload, kind) in items {
            hyper_rt_list_push(list, *payload, *kind);
        }
        list
    }

    #[test]
    fn strings_compare_by_content() {
        let a = str_payload("hyper");
        let b = str_payload("hyper");
        let c = str_payload("rust");
        assert_ne!(a, b, "test needs two distinct allocations");
        assert_eq!(hyper_rt_value_eq(a, KIND_STR, b, KIND_STR), 1);
        assert_eq!(hyper_rt_value_eq(a, KIND_STR, c, KIND_STR), 0);
    }

    #[test]
    fn lists_compare_element_wise() {
        let a = list_of(&[(1, KIND_I64), (2, KIND_I64)]);
        let b = list_of(&[(1, KIND_I64), (2, KIND_I64)]);
        let c = list_of(&[(1, KIND_I64), (3, KIND_I64)]);
        let short = list_of(&[(1, KIND_I64)]);
        assert_eq!(hyper_rt_value_eq(a, KIND_LIST, b, KIND_LIST), 1);
        assert_eq!(hyper_rt_value_eq(a, KIND_LIST, c, KIND_LIST), 0);
        assert_eq!(hyper_rt_value_eq(a, KIND_LIST, short, KIND_LIST), 0);
    }

    #[test]
    fn nested_lists_compare_by_content() {
        let inner_a = list_of(&[(str_payload("x"), KIND_STR)]);
        let inner_b = list_of(&[(str_payload("x"), KIND_STR)]);
        let a = list_of(&[(inner_a, KIND_LIST)]);
        let b = list_of(&[(inner_b, KIND_LIST)]);
        assert_eq!(hyper_rt_value_eq(a, KIND_LIST, b, KIND_LIST), 1);
    }

    #[test]
    fn dicts_ignore_entry_order() {
        let a = hyper_rt_dict_new();
        hyper_rt_dict_push(a, str_payload("x"), KIND_STR, 1, KIND_I64);
        hyper_rt_dict_push(a, str_payload("y"), KIND_STR, 2, KIND_I64);
        let b = hyper_rt_dict_new();
        hyper_rt_dict_push(b, str_payload("y"), KIND_STR, 2, KIND_I64);
        hyper_rt_dict_push(b, str_payload("x"), KIND_STR, 1, KIND_I64);
        assert_eq!(hyper_rt_value_eq(a, KIND_DICT, b, KIND_DICT), 1);
    }

    #[test]
    fn integers_and_floats_compare_numerically() {
        let one = 1i64;
        let one_point_zero = 1.0f64.to_bits() as i64;
        assert_eq!(hyper_rt_value_eq(one, KIND_I64, one_point_zero, KIND_F64), 1);
        assert_eq!(hyper_rt_value_eq(2, KIND_I64, one_point_zero, KIND_F64), 0);
    }

    #[test]
    fn booleans_do_not_equal_integers() {
        assert_eq!(hyper_rt_value_eq(1, KIND_BOOL, 1, KIND_BOOL), 1);
        assert_eq!(hyper_rt_value_eq(1, KIND_BOOL, 1, KIND_I64), 0);
    }

    #[test]
    fn struct_instances_are_never_equal() {
        let obj = hyper_rt_struct_new(1);
        assert_eq!(hyper_rt_value_eq(obj, KIND_STRUCT, obj, KIND_STRUCT), 0);
    }

    #[test]
    fn none_equals_none_only() {
        assert_eq!(hyper_rt_value_eq(0, KIND_NONE, 0, KIND_NONE), 1);
        assert_eq!(hyper_rt_value_eq(0, KIND_NONE, 0, KIND_I64), 0);
    }
}
