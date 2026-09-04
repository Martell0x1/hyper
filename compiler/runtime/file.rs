//! Compile-path file handles — wraps [`HyperFile`](crate::fileio::HyperFile) as an opaque pointer.

use crate::error;
use crate::fileio::HyperFile;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use super::{
    format_value, hyper_rt_list_new, hyper_rt_list_push, RtList, RtValue, KIND_I64, KIND_LIST,
    KIND_NONE, KIND_STR,
};

fn fatal(line: i64, msg: impl Into<String>) -> ! {
    error::runtime(line as u32, msg.into());
}

fn line_no(line: i64, _kind: i64) -> i64 {
    line
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

fn value_as_string(payload: i64, kind: i64) -> String {
    format_value(&RtValue { kind, payload })
}

fn file_mut(handle: i64, line: i64) -> &'static mut HyperFile {
    if handle == 0 {
        fatal(line, "invalid file handle");
    }
    unsafe { &mut *(handle as *mut HyperFile) }
}

fn io_err(line: i64, path: &str, op: &str, err: impl std::fmt::Display) -> ! {
    fatal(line, format!("{op} failed on '{path}': {err}"));
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_open(
    path: i64,
    path_kind: i64,
    mode: i64,
    mode_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let path_str = str_arg(path, path_kind, line, "open");
    let mode_str = if mode_kind == KIND_STR && mode != 0 {
        str_arg(mode, mode_kind, line, "open")
    } else {
        "r".to_string()
    };
    match HyperFile::open(&path_str, &mode_str) {
        Ok(file) => Box::into_raw(Box::new(file)) as i64,
        Err(e) => fatal(line, format!("could not open '{path_str}': {e}")),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_close(
    handle: i64,
    _handle_kind: i64,
    line: i64,
    _line_kind: i64,
) {
    if handle == 0 {
        return;
    }
    let line = line_no(line, _line_kind);
    // Take ownership so the Box and OS handle are released (mirrors mmap_close).
    let mut file = unsafe { Box::from_raw(handle as *mut HyperFile) };
    let path = file.path().to_string();
    if let Err(e) = file.close() {
        // Still drop the box; surface the close error.
        fatal(line, format!("could not close '{path}': {e}"));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_read_all(
    handle: i64,
    _handle_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let file = file_mut(handle, line);
    let path = file.path().to_string();
    match file.read_all() {
        Ok(text) => cstr_payload(&text),
        Err(e) => io_err(line, &path, "read", e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_read_n(
    handle: i64,
    _handle_kind: i64,
    count: i64,
    count_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let n = if count_kind == KIND_I64 {
        count.max(0) as usize
    } else {
        0
    };
    let file = file_mut(handle, line);
    let path = file.path().to_string();
    match file.read_n(n) {
        Ok(text) => cstr_payload(&text),
        Err(e) => io_err(line, &path, "read", e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_readline(
    handle: i64,
    _handle_kind: i64,
    line: i64,
    _line_kind: i64,
    out_kind: *mut i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let file = file_mut(handle, line);
    let path = file.path().to_string();
    match file.read_line() {
        Ok(Some(text)) => {
            if !out_kind.is_null() {
                unsafe { *out_kind = KIND_STR };
            }
            cstr_payload(&text)
        }
        Ok(None) => {
            if !out_kind.is_null() {
                unsafe { *out_kind = KIND_NONE };
            }
            0
        }
        Err(e) => io_err(line, &path, "readline", e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_readlines(
    handle: i64,
    _handle_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let file = file_mut(handle, line);
    let path = file.path().to_string();
    match file.read_lines() {
        Ok(lines) => {
            let list = hyper_rt_list_new();
            for line_text in lines {
                hyper_rt_list_push(list, cstr_payload(&line_text), KIND_STR);
            }
            list
        }
        Err(e) => io_err(line, &path, "readlines", e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_write(
    handle: i64,
    _handle_kind: i64,
    text: i64,
    text_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let file = file_mut(handle, line);
    let path = file.path().to_string();
    let content = value_as_string(text, text_kind);
    match file.write_str(&content) {
        Ok(n) => n as i64,
        Err(e) => io_err(line, &path, "write", e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_writelines(
    handle: i64,
    _handle_kind: i64,
    list: i64,
    list_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    if list_kind != KIND_LIST || list == 0 {
        fatal(line, "writelines expects a list");
    }
    let file = file_mut(handle, line);
    let path = file.path().to_string();
    let items = unsafe { &*(list as *const RtList) };
    let mut total = 0i64;
    for item in &items.items {
        let text = value_as_string(item.payload, item.kind);
        match file.write_str(&text) {
            Ok(n) => total += n as i64,
            Err(e) => io_err(line, &path, "writelines", e),
        }
    }
    total
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_seek(
    handle: i64,
    _handle_kind: i64,
    offset: i64,
    offset_kind: i64,
    whence: i64,
    whence_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let off = if offset_kind == KIND_I64 { offset } else { 0 };
    let wh = if whence_kind == KIND_I64 { whence } else { 0 };
    let file = file_mut(handle, line);
    let path = file.path().to_string();
    match file.seek(off, wh) {
        Ok(pos) => pos as i64,
        Err(e) => io_err(line, &path, "seek", e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_tell(
    handle: i64,
    _handle_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let file = file_mut(handle, line);
    let path = file.path().to_string();
    match file.tell() {
        Ok(pos) => pos as i64,
        Err(e) => io_err(line, &path, "tell", e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_size(
    handle: i64,
    _handle_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let file = file_mut(handle, line);
    let path = file.path().to_string();
    match file.size() {
        Ok(n) => n as i64,
        Err(e) => io_err(line, &path, "size", e),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_flush(
    handle: i64,
    _handle_kind: i64,
    line: i64,
    _line_kind: i64,
) {
    let line = line_no(line, _line_kind);
    let file = file_mut(handle, line);
    let path = file.path().to_string();
    if let Err(e) = file.flush() {
        io_err(line, &path, "flush", e);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_is_closed(handle: i64, _handle_kind: i64) -> i64 {
    if handle == 0 {
        return 1;
    }
    let file = unsafe { &*(handle as *const HyperFile) };
    file.is_closed() as i64
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_path(
    handle: i64,
    _handle_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let file = file_mut(handle, line);
    cstr_payload(file.path())
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_file_mode(
    handle: i64,
    _handle_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let line = line_no(line, _line_kind);
    let file = file_mut(handle, line);
    cstr_payload(file.mode())
}
