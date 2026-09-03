//! Memory-mapped files for the compile path (`with open_mmap(...) as m:`).

use crate::error;
use crate::fileio::MappedFile;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;

fn fatal(line: i64, msg: impl Into<String>) -> ! {
    error::runtime(line as u32, msg.into());
}

fn cstr_payload(text: &str) -> i64 {
    CString::new(text).unwrap_or_default().into_raw() as i64
}

fn str_arg(payload: i64, kind: i64, line: i64, context: &str) -> String {
    const KIND_STR: i64 = 2;
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

fn mmap_mut(handle: i64, line: i64) -> &'static mut MappedFile {
    if handle == 0 {
        fatal(line, "invalid mapped file handle");
    }
    unsafe { &mut *(handle as *mut MappedFile) }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_mmap_open(
    path: i64,
    path_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let path_str = str_arg(path, path_kind, line, "open_mmap");
    match MappedFile::open(&path_str) {
        Ok(map) => Box::into_raw(Box::new(map)) as i64,
        Err(e) => fatal(
            line,
            format!("could not map file '{path_str}': {e}"),
        ),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_mmap_close(
    handle: i64,
    _handle_kind: i64,
    _line: i64,
    _line_kind: i64,
) {
    if handle == 0 {
        return;
    }
    unsafe {
        drop(Box::from_raw(handle as *mut MappedFile));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_mmap_read_chunk(
    handle: i64,
    _handle_kind: i64,
    offset: i64,
    offset_kind: i64,
    size: i64,
    size_kind: i64,
    line: i64,
    _line_kind: i64,
) -> i64 {
    let map = mmap_mut(handle, line);
    const KIND_I64: i64 = 0;
    let off = if offset_kind == KIND_I64 {
        offset.max(0) as usize
    } else {
        0
    };
    let n = if size_kind == KIND_I64 {
        size.max(0) as usize
    } else {
        0
    };
    cstr_payload(&map.chunk(off, n))
}
