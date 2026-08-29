//! Buffered file objects behind Hyper's `open(...)` builtin.
//!
//! A `HyperFile` owns one OS handle plus a read-ahead buffer and a write-behind
//! buffer, so scripts pay one syscall per buffer instead of one per call. The two
//! buffers are mutually exclusive: writing drops the read-ahead (rewinding the OS
//! cursor to the logical position) and reading flushes pending writes first, which
//! keeps `r+`/`w+`/`a+` handles consistent when a script mixes reads and writes.

use std::cell::RefCell;
use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::rc::Rc;

use memmap2::Mmap;

use crate::environment::HyperValue;

const BUFFER_CAPACITY: usize = 64 * 1024;

pub struct HyperFile {
    file: File,
    path: String,
    mode: String,
    readable: bool,
    writable: bool,
    /// Read-ahead buffer; only `read_buf[read_pos..read_len]` is still unread.
    read_buf: Vec<u8>,
    read_pos: usize,
    read_len: usize,
    write_buf: Vec<u8>,
    closed: bool,
}

impl fmt::Debug for HyperFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("HyperFile")
            .field("path", &self.path)
            .field("mode", &self.mode)
            .field("closed", &self.closed)
            .finish()
    }
}

impl HyperFile {
    pub fn open(path: &str, mode: &str) -> io::Result<HyperFile> {
        let (readable, writable, mut options) = options_for_mode(mode)?;
        let file = options.read(readable).open(path)?;
        Ok(HyperFile {
            file,
            path: path.to_string(),
            mode: mode.to_string(),
            readable,
            writable,
            read_buf: Vec::new(),
            read_pos: 0,
            read_len: 0,
            write_buf: Vec::new(),
            closed: false,
        })
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn mode(&self) -> &str {
        &self.mode
    }

    pub fn is_closed(&self) -> bool {
        self.closed
    }

    fn check_open(&self) -> io::Result<()> {
        if self.closed {
            return Err(io::Error::other("I/O operation on closed file"));
        }
        Ok(())
    }

    fn check_readable(&self) -> io::Result<()> {
        self.check_open()?;
        if !self.readable {
            return Err(io::Error::other(format!(
                "file is not open for reading (mode '{}')",
                self.mode
            )));
        }
        Ok(())
    }

    fn check_writable(&self) -> io::Result<()> {
        self.check_open()?;
        if !self.writable {
            return Err(io::Error::other(format!(
                "file is not open for writing (mode '{}')",
                self.mode
            )));
        }
        Ok(())
    }

    fn unread(&self) -> usize {
        self.read_len - self.read_pos
    }

    fn flush_writes(&mut self) -> io::Result<()> {
        if !self.write_buf.is_empty() {
            self.file.write_all(&self.write_buf)?;
            self.write_buf.clear();
        }
        Ok(())
    }

    /// Give back the bytes we read ahead but never handed to the script.
    fn release_read_ahead(&mut self) -> io::Result<()> {
        let pending = self.unread();
        if pending > 0 {
            self.file.seek(SeekFrom::Current(-(pending as i64)))?;
        }
        self.read_pos = 0;
        self.read_len = 0;
        Ok(())
    }

    /// Refill the read-ahead buffer when empty; returns unread byte count.
    fn fill(&mut self) -> io::Result<usize> {
        if self.read_pos < self.read_len {
            return Ok(self.unread());
        }
        self.flush_writes()?;
        if self.read_buf.is_empty() {
            self.read_buf = vec![0u8; BUFFER_CAPACITY];
        }
        let n = self.file.read(&mut self.read_buf)?;
        self.read_pos = 0;
        self.read_len = n;
        Ok(n)
    }

    /// Remaining bytes from the OS cursor to end of file, for capacity hints.
    fn remaining_hint(&mut self) -> usize {
        let len = match self.file.metadata() {
            Ok(m) => m.len(),
            Err(_) => return 0,
        };
        let pos = self.file.stream_position().unwrap_or(len);
        len.saturating_sub(pos) as usize
    }

    pub fn read_all(&mut self) -> io::Result<String> {
        self.check_readable()?;
        self.flush_writes()?;
        let mut out = Vec::new();
        out.reserve(self.unread() + self.remaining_hint());
        out.extend_from_slice(&self.read_buf[self.read_pos..self.read_len]);
        self.read_pos = self.read_len;
        self.file.read_to_end(&mut out)?;
        Ok(into_string(out))
    }

    pub fn read_n(&mut self, count: usize) -> io::Result<String> {
        self.check_readable()?;
        let mut out: Vec<u8> = Vec::with_capacity(count.min(BUFFER_CAPACITY));
        while out.len() < count {
            let available = self.fill()?;
            if available == 0 {
                break;
            }
            let take = (count - out.len()).min(available);
            let from = self.read_pos;
            out.extend_from_slice(&self.read_buf[from..from + take]);
            self.read_pos += take;
        }
        Ok(into_string(out))
    }

    /// One line without its trailing newline; `None` once the file is exhausted.
    pub fn read_line(&mut self) -> io::Result<Option<String>> {
        self.check_readable()?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let available = self.fill()?;
            if available == 0 {
                break;
            }
            let from = self.read_pos;
            let slice = &self.read_buf[from..self.read_len];
            match slice.iter().position(|b| *b == b'\n') {
                Some(idx) => {
                    out.extend_from_slice(&slice[..idx]);
                    self.read_pos = from + idx + 1;
                    if out.last() == Some(&b'\r') {
                        out.pop();
                    }
                    return Ok(Some(into_string(out)));
                }
                None => {
                    out.extend_from_slice(slice);
                    self.read_pos = self.read_len;
                }
            }
        }
        if out.is_empty() {
            return Ok(None);
        }
        if out.last() == Some(&b'\r') {
            out.pop();
        }
        Ok(Some(into_string(out)))
    }

    pub fn read_lines(&mut self) -> io::Result<Vec<String>> {
        let mut lines = Vec::new();
        while let Some(line) = self.read_line()? {
            lines.push(line);
        }
        Ok(lines)
    }

    pub fn write_str(&mut self, text: &str) -> io::Result<usize> {
        self.check_writable()?;
        self.release_read_ahead()?;
        let bytes = text.as_bytes();
        if bytes.len() >= BUFFER_CAPACITY {
            self.flush_writes()?;
            self.file.write_all(bytes)?;
        } else {
            if self.write_buf.len() + bytes.len() > BUFFER_CAPACITY {
                self.flush_writes()?;
            }
            if self.write_buf.capacity() == 0 {
                self.write_buf.reserve(BUFFER_CAPACITY);
            }
            self.write_buf.extend_from_slice(bytes);
        }
        Ok(bytes.len())
    }

    pub fn flush(&mut self) -> io::Result<()> {
        self.check_open()?;
        self.flush_writes()?;
        self.file.flush()
    }

    pub fn seek(&mut self, offset: i64, whence: i64) -> io::Result<u64> {
        self.check_open()?;
        self.flush_writes()?;
        self.release_read_ahead()?;
        let target = match whence {
            1 => SeekFrom::Current(offset),
            2 => SeekFrom::End(offset),
            _ => SeekFrom::Start(offset.max(0) as u64),
        };
        self.file.seek(target)
    }

    pub fn tell(&mut self) -> io::Result<u64> {
        self.check_open()?;
        let pos = self.file.stream_position()?;
        Ok(pos.saturating_add(self.write_buf.len() as u64) - self.unread() as u64)
    }

    pub fn size(&mut self) -> io::Result<u64> {
        self.check_open()?;
        self.flush_writes()?;
        Ok(self.file.metadata()?.len())
    }

    pub fn close(&mut self) -> io::Result<()> {
        if self.closed {
            return Ok(());
        }
        self.flush_writes()?;
        self.file.flush()?;
        self.closed = true;
        Ok(())
    }
}

impl Drop for HyperFile {
    fn drop(&mut self) {
        let _ = self.close();
    }
}

fn options_for_mode(mode: &str) -> io::Result<(bool, bool, OpenOptions)> {
    // Python accepts binary/text flags in the mode string; Hyper reads text either way.
    let normalized: String = mode.chars().filter(|c| *c != 'b' && *c != 't').collect();
    let mut options = OpenOptions::new();
    let (readable, writable) = match normalized.as_str() {
        "" | "r" => (true, false),
        "r+" | "+r" => {
            options.write(true);
            (true, true)
        }
        "w" => {
            options.write(true).create(true).truncate(true);
            (false, true)
        }
        "w+" | "+w" => {
            options.write(true).create(true).truncate(true);
            (true, true)
        }
        "a" => {
            options.append(true).create(true);
            (false, true)
        }
        "a+" | "+a" => {
            options.append(true).create(true);
            (true, true)
        }
        "x" => {
            options.write(true).create_new(true);
            (false, true)
        }
        "x+" | "+x" => {
            options.write(true).create_new(true);
            (true, true)
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid file mode '{}'", other),
            ));
        }
    };
    Ok((readable, writable, options))
}

fn into_string(bytes: Vec<u8>) -> String {
    match String::from_utf8(bytes) {
        Ok(s) => s,
        Err(e) => String::from_utf8_lossy(e.as_bytes()).into_owned(),
    }
}

/// A file mapped into the address space, used by `with open_mmap(...)`.
/// Zero-length files cannot be mapped, so they are represented explicitly.
pub enum MappedFile {
    Empty,
    Mapped(Mmap),
}

impl MappedFile {
    pub fn open(path: &str) -> io::Result<MappedFile> {
        let file = File::open(path)?;
        if file.metadata()?.len() == 0 {
            return Ok(MappedFile::Empty);
        }
        // Safe as long as the file is not truncated by another process while mapped.
        let map = unsafe { Mmap::map(&file)? };
        Ok(MappedFile::Mapped(map))
    }

    pub fn bytes(&self) -> &[u8] {
        match self {
            MappedFile::Empty => &[],
            MappedFile::Mapped(map) => map,
        }
    }

    /// Copy out `size` bytes starting at `offset`, clamped to the mapping.
    pub fn chunk(&self, offset: usize, size: usize) -> String {
        let bytes = self.bytes();
        if offset >= bytes.len() {
            return String::new();
        }
        let end = offset.saturating_add(size).min(bytes.len());
        String::from_utf8_lossy(&bytes[offset..end]).into_owned()
    }
}

impl fmt::Debug for MappedFile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MappedFile")
            .field("len", &self.bytes().len())
            .finish()
    }
}

pub fn call_mmap_method(
    map: &Rc<MappedFile>,
    method: &str,
    args: &[HyperValue],
    line: u32,
) -> Option<HyperValue> {
    match method {
        "read_chunk" => {
            if args.len() != 2 {
                fatal(
                    line,
                    "read_chunk expects 2 arguments (offset, size)".to_string(),
                );
            }
            let offset = as_int(&args[0], "read_chunk", line).max(0) as usize;
            let size = as_int(&args[1], "read_chunk", line).max(0) as usize;
            Some(HyperValue::String(map.chunk(offset, size)))
        }
        other => fatal(line, format!("mapped file has no method '{}'", other)),
    }
}

pub fn open_value(path: &str, mode: &str, line: u32) -> HyperValue {
    match HyperFile::open(path, mode) {
        Ok(file) => HyperValue::File {
            path: path.to_string(),
            file: Rc::new(RefCell::new(file)),
        },
        Err(e) => {
            eprintln!("[line {}] Error: could not open '{}': {}.", line, path, e);
            std::process::exit(70);
        }
    }
}

fn fatal(line: u32, message: String) -> ! {
    eprintln!("[line {}] Error: {}.", line, message);
    std::process::exit(70);
}

fn as_int(value: &HyperValue, method: &str, line: u32) -> i64 {
    match value.to_int() {
        Some(n) => n,
        None => fatal(line, format!("{} expects an integer argument", method)),
    }
}

pub fn call_file_method(
    handle: &Rc<RefCell<HyperFile>>,
    method: &str,
    args: &[HyperValue],
    line: u32,
) -> Option<HyperValue> {
    let expect_args = |wanted: usize| {
        if args.len() != wanted {
            fatal(
                line,
                format!(
                    "{} expects {} argument(s) but got {}",
                    method,
                    wanted,
                    args.len()
                ),
            );
        }
    };

    let mut file = handle.borrow_mut();
    let result = match method {
        "read" => {
            if args.is_empty() {
                file.read_all().map(HyperValue::String)
            } else if args.len() == 1 {
                let count = as_int(&args[0], "read", line).max(0) as usize;
                file.read_n(count).map(HyperValue::String)
            } else {
                fatal(line, "read expects 0 or 1 argument(s)".to_string());
            }
        }
        "readline" => {
            expect_args(0);
            file.read_line().map(|maybe_line| match maybe_line {
                Some(text) => HyperValue::String(text),
                None => HyperValue::None,
            })
        }
        "readlines" => {
            expect_args(0);
            file.read_lines().map(|lines| {
                HyperValue::List(lines.into_iter().map(HyperValue::String).collect())
            })
        }
        "write" => {
            expect_args(1);
            let text = args[0].to_string();
            file.write_str(&text).map(|n| HyperValue::I64(n as i64))
        }
        "writelines" => {
            expect_args(1);
            match &args[0] {
                HyperValue::List(items) | HyperValue::Array { elements: items, .. } => {
                    let mut total = 0usize;
                    let mut error = None;
                    for item in items {
                        match file.write_str(&item.to_string()) {
                            Ok(n) => total += n,
                            Err(e) => {
                                error = Some(e);
                                break;
                            }
                        }
                    }
                    match error {
                        Some(e) => Err(e),
                        None => Ok(HyperValue::I64(total as i64)),
                    }
                }
                _ => fatal(line, "writelines expects a list of strings".to_string()),
            }
        }
        "seek" => {
            let (offset, whence) = match args.len() {
                1 => (as_int(&args[0], "seek", line), 0),
                2 => (
                    as_int(&args[0], "seek", line),
                    as_int(&args[1], "seek", line),
                ),
                _ => fatal(line, "seek expects 1 or 2 argument(s)".to_string()),
            };
            file.seek(offset, whence).map(|p| HyperValue::I64(p as i64))
        }
        "tell" => {
            expect_args(0);
            file.tell().map(|p| HyperValue::I64(p as i64))
        }
        "size" => {
            expect_args(0);
            file.size().map(|n| HyperValue::I64(n as i64))
        }
        "flush" => {
            expect_args(0);
            file.flush().map(|_| HyperValue::None)
        }
        "close" => {
            expect_args(0);
            file.close().map(|_| HyperValue::None)
        }
        "closed" => {
            expect_args(0);
            Ok(HyperValue::Boolean(file.is_closed()))
        }
        "path" => {
            expect_args(0);
            Ok(HyperValue::String(file.path().to_string()))
        }
        "mode" => {
            expect_args(0);
            Ok(HyperValue::String(file.mode().to_string()))
        }
        other => fatal(line, format!("file has no method '{}'", other)),
    };

    match result {
        Ok(value) => Some(value),
        Err(e) => {
            let path = file.path().to_string();
            drop(file);
            fatal(line, format!("{} failed on '{}': {}", method, path, e));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    fn temp_path(name: &str) -> String {
        env::temp_dir()
            .join(format!("hyper_fileio_{}_{}", std::process::id(), name))
            .to_string_lossy()
            .into_owned()
    }

    #[test]
    fn write_then_read_roundtrip() {
        let path = temp_path("roundtrip.txt");
        let mut out = HyperFile::open(&path, "w").expect("open for write");
        out.write_str("first\nsecond\n").expect("write");
        out.close().expect("close");

        let mut input = HyperFile::open(&path, "r").expect("open for read");
        assert_eq!(input.read_all().expect("read"), "first\nsecond\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_line_strips_newlines() {
        let path = temp_path("lines.txt");
        let mut out = HyperFile::open(&path, "w").expect("open");
        out.write_str("a\r\nb\n\nc").expect("write");
        out.close().expect("close");

        let mut input = HyperFile::open(&path, "r").expect("open");
        assert_eq!(input.read_lines().expect("lines"), vec!["a", "b", "", "c"]);
        assert_eq!(input.read_line().expect("eof"), None);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn read_and_write_share_one_handle() {
        let path = temp_path("update.txt");
        let mut out = HyperFile::open(&path, "w").expect("open");
        out.write_str("0123456789").expect("write");
        out.close().expect("close");

        let mut file = HyperFile::open(&path, "r+").expect("open r+");
        assert_eq!(file.read_n(4).expect("read"), "0123");
        assert_eq!(file.tell().expect("tell"), 4);
        file.write_str("ABC").expect("write");
        file.seek(0, 0).expect("seek");
        assert_eq!(file.read_all().expect("read"), "0123ABC789");
        file.close().expect("close");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn append_mode_keeps_existing_content() {
        let path = temp_path("append.txt");
        let mut first = HyperFile::open(&path, "w").expect("open");
        first.write_str("one\n").expect("write");
        first.close().expect("close");

        let mut second = HyperFile::open(&path, "a").expect("open append");
        second.write_str("two\n").expect("write");
        second.close().expect("close");

        let mut input = HyperFile::open(&path, "r").expect("open read");
        assert_eq!(input.read_all().expect("read"), "one\ntwo\n");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn writing_to_read_only_handle_fails() {
        let path = temp_path("readonly.txt");
        let mut out = HyperFile::open(&path, "w").expect("open");
        out.write_str("x").expect("write");
        out.close().expect("close");

        let mut input = HyperFile::open(&path, "r").expect("open");
        assert!(input.write_str("nope").is_err());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn mapped_file_reads_chunks_and_clamps() {
        let path = temp_path("mapped.bin");
        let mut out = HyperFile::open(&path, "w").expect("open");
        out.write_str("0123456789").expect("write");
        out.close().expect("close");

        let map = MappedFile::open(&path).expect("map");
        assert_eq!(map.chunk(0, 4), "0123");
        assert_eq!(map.chunk(8, 100), "89");
        assert_eq!(map.chunk(50, 4), "");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn empty_file_maps_to_an_empty_region() {
        let path = temp_path("empty.bin");
        HyperFile::open(&path, "w")
            .expect("open")
            .close()
            .expect("close");

        let map = MappedFile::open(&path).expect("map");
        assert!(map.bytes().is_empty());
        assert_eq!(map.chunk(0, 8), "");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn invalid_mode_is_rejected() {
        assert!(HyperFile::open(&temp_path("bad.txt"), "q").is_err());
    }

    #[test]
    fn closed_file_rejects_further_reads() {
        let path = temp_path("closed.txt");
        let mut out = HyperFile::open(&path, "w").expect("open");
        out.close().expect("close");
        assert!(out.write_str("x").is_err());
        let _ = std::fs::remove_file(&path);
    }
}
