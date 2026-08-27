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

#[derive(Clone)]
struct RtValue {
    kind: i64,
    payload: i64,
}

struct RtList {
    items: Vec<RtValue>,
}

struct RtDict {
    entries: Vec<(String, RtValue)>,
}

struct RtStruct {
    fields: Vec<RtValue>,
}

fn format_value(v: &RtValue) -> String {
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
    print!("{} ", v);
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_f64(v: f64) {
    print!("{} ", v);
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_str(s: *const i8) {
    if s.is_null() {
        print!(" ");
        return;
    }
    let cstr = unsafe { CStr::from_ptr(s) };
    match cstr.to_str() {
        Ok(text) => print!("{} ", text),
        Err(_) => print!("<?> "),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_newline() {
    println!();
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
        print!("[] ");
        return;
    }
    let list = unsafe { &*(list as *const RtList) };
    print!("{} ", format_list(list));
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
        print!("{{}} ");
        return;
    }
    let dict = unsafe { &*(dict as *const RtDict) };
    print!("{} ", format_dict(dict));
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_value(payload: i64, kind: i64) {
    print!("{} ", format_value(&RtValue { kind, payload }));
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
        print!("{{}} ");
        return;
    }
    let st = unsafe { &*(obj as *const RtStruct) };
    print!("{} ", format_struct(st));
}
