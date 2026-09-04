//! JSON builtins for the compile path (`json.loads`, `dumps`, `load`, `dump`).

use crate::environment::HyperValue;
use crate::error;
use crate::fileio::HyperFile;
use crate::json;
use std::cell::RefCell;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::rc::Rc;

use super::{
    hyper_rt_dict_new, hyper_rt_dict_push, hyper_rt_list_new, hyper_rt_list_push, RtDict, RtList,
    RtValue, KIND_BOOL, KIND_DICT, KIND_F64, KIND_I64, KIND_LIST, KIND_NONE, KIND_STR,
};

fn fatal(line: i64, msg: impl Into<String>) -> ! {
    error::runtime(line as u32, msg.into());
}

fn cstr_payload(text: &str) -> i64 {
    CString::new(text).unwrap_or_default().into_raw() as i64
}

fn str_arg(payload: i64, kind: i64, line: i64, context: &str) -> String {
    if kind != KIND_STR {
        fatal(line, format!("{context}: expected a string"));
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

fn indent_arg(payload: i64, kind: i64) -> usize {
    if kind == KIND_I64 && payload > 0 {
        payload as usize
    } else {
        0
    }
}

fn hyper_to_rt(value: &HyperValue) -> RtValue {
    match value {
        HyperValue::None => RtValue {
            kind: KIND_NONE,
            payload: 0,
        },
        HyperValue::Boolean(b) => RtValue {
            kind: KIND_BOOL,
            payload: if *b { 1 } else { 0 },
        },
        HyperValue::String(s) => RtValue {
            kind: KIND_STR,
            payload: cstr_payload(s),
        },
        HyperValue::I64(n) => RtValue {
            kind: KIND_I64,
            payload: *n,
        },
        HyperValue::I8(n) => RtValue {
            kind: KIND_I64,
            payload: *n as i64,
        },
        HyperValue::I16(n) => RtValue {
            kind: KIND_I64,
            payload: *n as i64,
        },
        HyperValue::I32(n) => RtValue {
            kind: KIND_I64,
            payload: *n as i64,
        },
        HyperValue::U8(n) => RtValue {
            kind: KIND_I64,
            payload: *n as i64,
        },
        HyperValue::U16(n) => RtValue {
            kind: KIND_I64,
            payload: *n as i64,
        },
        HyperValue::U32(n) => RtValue {
            kind: KIND_I64,
            payload: *n as i64,
        },
        HyperValue::U64(n) => RtValue {
            kind: KIND_I64,
            payload: *n as i64,
        },
        HyperValue::F32(n) => RtValue {
            kind: KIND_F64,
            payload: (*n as f64).to_bits() as i64,
        },
        HyperValue::F64(n) => RtValue {
            kind: KIND_F64,
            payload: n.to_bits() as i64,
        },
        HyperValue::List(items) | HyperValue::Array { elements: items, .. } => {
            let list = hyper_rt_list_new();
            for item in items.borrow().iter() {
                let rt = hyper_to_rt(item);
                hyper_rt_list_push(list, rt.payload, rt.kind);
            }
            RtValue {
                kind: KIND_LIST,
                payload: list,
            }
        }
        HyperValue::Dict { entries, .. } => {
            let dict = hyper_rt_dict_new();
            for (key, val) in entries.borrow().iter() {
                let rt = hyper_to_rt(val);
                hyper_rt_dict_push(dict, cstr_payload(key), KIND_STR, rt.payload, rt.kind);
            }
            RtValue {
                kind: KIND_DICT,
                payload: dict,
            }
        }
        HyperValue::Instance {
            fields,
            field_indices,
            ..
        } => {
            let borrowed = fields.borrow();
            let mut names: Vec<&String> = field_indices.keys().collect();
            names.sort_by_key(|name| field_indices[*name]);
            let dict = hyper_rt_dict_new();
            for name in names {
                let idx = field_indices[name];
                let rt = hyper_to_rt(&borrowed[idx]);
                hyper_rt_dict_push(dict, cstr_payload(name), KIND_STR, rt.payload, rt.kind);
            }
            RtValue {
                kind: KIND_DICT,
                payload: dict,
            }
        }
        other => fatal(0, format!("cannot serialize {} to JSON", json_type_label(other))),
    }
}

fn json_type_label(value: &HyperValue) -> &'static str {
    match value {
        HyperValue::Function { .. } | HyperValue::NativeFunction(_) => "a function",
        HyperValue::Module { .. } => "a module",
        HyperValue::File { .. } => "a file",
        HyperValue::MmapFile { .. } => "a mapped file",
        HyperValue::StructDef { .. } => "a struct definition",
        HyperValue::TraitDef { .. } => "a trait",
        _ => "this value",
    }
}

fn rt_to_hyper(payload: i64, kind: i64, line: i64) -> HyperValue {
    match kind {
        KIND_NONE => HyperValue::None,
        KIND_BOOL => HyperValue::Boolean(payload != 0),
        KIND_STR => HyperValue::String(str_arg(payload, kind, line, "JSON value")),
        KIND_I64 => HyperValue::I64(payload),
        KIND_F64 => HyperValue::F64(f64::from_bits(payload as u64)),
        KIND_LIST => {
            if payload == 0 {
                return HyperValue::List(Rc::new(RefCell::new(Vec::new())));
            }
            let list = unsafe { &*(payload as *const RtList) };
            let mut items = Vec::with_capacity(list.items.len());
            for item in &list.items {
                items.push(rt_to_hyper(item.payload, item.kind, line));
            }
            HyperValue::List(Rc::new(RefCell::new(items)))
        }
        KIND_DICT => {
            if payload == 0 {
                return HyperValue::Dict {
                    key_type: "str".to_string(),
                    val_type: "Any".to_string(),
                    entries: Rc::new(RefCell::new(Default::default())),
                };
            }
            let dict = unsafe { &*(payload as *const RtDict) };
            let mut entries = indexmap::IndexMap::new();
            for (key, val) in &dict.entries {
                entries.insert(key.clone(), rt_to_hyper(val.payload, val.kind, line));
            }
            HyperValue::Dict {
                key_type: "str".to_string(),
                val_type: "Any".to_string(),
                entries: Rc::new(RefCell::new(entries)),
            }
        }
        _ => fatal(line, "invalid JSON value kind"),
    }
}

fn file_mut(handle: i64, line: i64) -> &'static mut HyperFile {
    if handle == 0 {
        fatal(line, "json.load expects an open file");
    }
    unsafe { &mut *(handle as *mut HyperFile) }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_json_loads(
    text: i64,
    text_kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    let source = str_arg(text, text_kind, line, "json.loads");
    match json::parse(&source) {
        Ok(value) => {
            let rt = hyper_to_rt(&value);
            if !out_kind.is_null() {
                unsafe { *out_kind = rt.kind };
            }
            rt.payload
        }
        Err(msg) => fatal(line, format!("invalid JSON: {msg}")),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_json_dumps(
    value: i64,
    value_kind: i64,
    indent: i64,
    indent_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let hv = rt_to_hyper(value, value_kind, line);
    let spaces = indent_arg(indent, indent_kind);
    match json::stringify(&hv, spaces) {
        Ok(text) => cstr_payload(&text),
        Err(msg) => fatal(line, msg),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_json_load(
    handle: i64,
    _handle_kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    let file = file_mut(handle, line);
    let text = match file.read_all() {
        Ok(text) => text,
        Err(e) => fatal(line, format!("json.load could not read the file: {e}")),
    };
    match json::parse(&text) {
        Ok(value) => {
            let rt = hyper_to_rt(&value);
            if !out_kind.is_null() {
                unsafe { *out_kind = rt.kind };
            }
            rt.payload
        }
        Err(msg) => fatal(line, format!("invalid JSON: {msg}")),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_json_dump(
    value: i64,
    value_kind: i64,
    handle: i64,
    _handle_kind: i64,
    indent: i64,
    indent_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let hv = rt_to_hyper(value, value_kind, line);
    let spaces = indent_arg(indent, indent_kind);
    let text = match json::stringify(&hv, spaces) {
        Ok(text) => text,
        Err(msg) => fatal(line, msg),
    };
    let file = file_mut(handle, line);
    match file.write_str(&text) {
        Ok(n) => n as i64,
        Err(e) => fatal(
            line,
            format!("json.dump could not write the file: {e}"),
        ),
    }
}
