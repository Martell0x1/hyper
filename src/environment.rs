use std::collections::HashMap;
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum HyperValue {
    I8(i8),
    I16(i16),
    I32(i32),
    I64(i64),

    U8(u8),
    U16(u16),
    U32(u32),
    U64(u64),

    F32(f32),
    F64(f64),

    String(String),
    Boolean(bool),
    None,

    List(Vec<HyperValue>),
    Array {
        element_type: String,
        elements: Vec<HyperValue>,
    },
    Dict {
        key_type: String,
        val_type: String,
        entries: HashMap<String, HyperValue>,
    },
    MmapFile {
        file: Rc<RefCell<File>>,
        path: String,
    },

    NativeFunction(String),
    Function {
        name: String,
        params: Vec<String>,
        body: String,
        is_strict: bool,
        closure: Rc<RefCell<Environment>>,
    },
}

impl PartialEq for HyperValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (HyperValue::I8(a), HyperValue::I8(b)) => a == b,
            (HyperValue::I16(a), HyperValue::I16(b)) => a == b,
            (HyperValue::I32(a), HyperValue::I32(b)) => a == b,
            (HyperValue::I64(a), HyperValue::I64(b)) => a == b,
            (HyperValue::U8(a), HyperValue::U8(b)) => a == b,
            (HyperValue::U16(a), HyperValue::U16(b)) => a == b,
            (HyperValue::U32(a), HyperValue::U32(b)) => a == b,
            (HyperValue::U64(a), HyperValue::U64(b)) => a == b,
            (HyperValue::F32(a), HyperValue::F32(b)) => a == b,
            (HyperValue::F64(a), HyperValue::F64(b)) => a == b,
            (HyperValue::String(a), HyperValue::String(b)) => a == b,
            (HyperValue::Boolean(a), HyperValue::Boolean(b)) => a == b,
            (HyperValue::None, HyperValue::None) => true,
            (HyperValue::List(a), HyperValue::List(b)) => a == b,
            (HyperValue::Array { element_type: et1, elements: el1 }, HyperValue::Array { element_type: et2, elements: el2 }) => {
                et1 == et2 && el1 == el2
            }
            (HyperValue::Dict { key_type: kt1, val_type: vt1, entries: e1 }, HyperValue::Dict { key_type: kt2, val_type: vt2, entries: e2 }) => {
                kt1 == kt2 && vt1 == vt2 && e1 == e2
            }
            (HyperValue::MmapFile { path: a, .. }, HyperValue::MmapFile { path: b, .. }) => a == b,
            (HyperValue::NativeFunction(a), HyperValue::NativeFunction(b)) => a == b,
            (HyperValue::Function { name: n1, params: p1, body: b1, is_strict: s1, .. }, HyperValue::Function { name: n2, params: p2, body: b2, is_strict: s2, .. }) => {
                n1 == n2 && p1 == p2 && b1 == b2 && s1 == s2
            }
            _ => false,
        }
    }
}

macro_rules! impl_binary_op {
    ($self:expr, $other:expr, $op:tt) => {
        match ($self, $other) {
            (HyperValue::I8(a), HyperValue::I8(b)) => Some(HyperValue::I8(a $op b)),
            (HyperValue::I16(a), HyperValue::I16(b)) => Some(HyperValue::I16(a $op b)),
            (HyperValue::I32(a), HyperValue::I32(b)) => Some(HyperValue::I32(a $op b)),
            (HyperValue::I64(a), HyperValue::I64(b)) => Some(HyperValue::I64(a $op b)),
            (HyperValue::U8(a), HyperValue::U8(b)) => Some(HyperValue::U8(a $op b)),
            (HyperValue::U16(a), HyperValue::U16(b)) => Some(HyperValue::U16(a $op b)),
            (HyperValue::U32(a), HyperValue::U32(b)) => Some(HyperValue::U32(a $op b)),
            (HyperValue::U64(a), HyperValue::U64(b)) => Some(HyperValue::U64(a $op b)),
            (HyperValue::F32(a), HyperValue::F32(b)) => Some(HyperValue::F32(a $op b)),
            (HyperValue::F64(a), HyperValue::F64(b)) => Some(HyperValue::F64(a $op b)),
            _ => None,
        }
    };
}

macro_rules! impl_cmp_op {
    ($self:expr, $other:expr, $op:tt) => {
        match ($self, $other) {
            (HyperValue::I8(a), HyperValue::I8(b)) => Some(HyperValue::Boolean(a $op b)),
            (HyperValue::I16(a), HyperValue::I16(b)) => Some(HyperValue::Boolean(a $op b)),
            (HyperValue::I32(a), HyperValue::I32(b)) => Some(HyperValue::Boolean(a $op b)),
            (HyperValue::I64(a), HyperValue::I64(b)) => Some(HyperValue::Boolean(a $op b)),
            (HyperValue::U8(a), HyperValue::U8(b)) => Some(HyperValue::Boolean(a $op b)),
            (HyperValue::U16(a), HyperValue::U16(b)) => Some(HyperValue::Boolean(a $op b)),
            (HyperValue::U32(a), HyperValue::U32(b)) => Some(HyperValue::Boolean(a $op b)),
            (HyperValue::U64(a), HyperValue::U64(b)) => Some(HyperValue::Boolean(a $op b)),
            (HyperValue::F32(a), HyperValue::F32(b)) => Some(HyperValue::Boolean(a $op b)),
            (HyperValue::F64(a), HyperValue::F64(b)) => Some(HyperValue::Boolean(a $op b)),
            _ => None,
        }
    };
}

impl HyperValue {
    pub fn call_method(&self, method_name: &str, args: &[HyperValue], line: u32) -> Option<HyperValue> {
        match self {
            HyperValue::String(s) => {
                match method_name {
                    "strip" => {
                        let trimmed = s.trim();
                        if trimmed.len() == s.len() {
                            return Some(self.clone());
                        }
                        Some(HyperValue::String(trimmed.to_string()))
                    }
                    "lstrip" => {
                        let trimmed = s.trim_start();
                        if trimmed.len() == s.len() {
                            return Some(self.clone());
                        }
                        Some(HyperValue::String(trimmed.to_string()))
                    }
                    "rstrip" => {
                        let trimmed = s.trim_end();
                        if trimmed.len() == s.len() {
                            return Some(self.clone());
                        }
                        Some(HyperValue::String(trimmed.to_string()))
                    }
                    "upper" => Some(HyperValue::String(s.to_uppercase())),
                    "lower" => Some(HyperValue::String(s.to_lowercase())),
                    "capitalize" => {
                        let mut chars = s.chars();
                        let capitalized = match chars.next() {
                            None => String::new(),
                            Some(f) => f.to_uppercase().chain(chars.flat_map(|c| c.to_lowercase())).collect(),
                        };
                        Some(HyperValue::String(capitalized))
                    }
                    "title" => {
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
                        Some(HyperValue::String(result))
                    }
                    "join" => {
                        if let Some(HyperValue::List(items)) = args.first() {
                            let strs: Vec<String> = items.iter().map(|item| item.to_string()).collect();
                            Some(HyperValue::String(strs.join(s)))
                        } else {
                            eprintln!("[line {}] Type Error: 'join' expects a list argument.", line);
                            std::process::exit(70);
                        }
                    }
                    "len" => Some(HyperValue::I64(s.chars().count() as i64)),
                    "startswith" => {
                        if let Some(HyperValue::String(sub)) = args.first() {
                            Some(HyperValue::Boolean(s.starts_with(sub)))
                        } else {
                            eprintln!("[line {}] Type Error: 'startswith' expects a string argument.", line);
                            std::process::exit(70);
                        }
                    }
                    "endswith" => {
                        if let Some(HyperValue::String(sub)) = args.first() {
                            Some(HyperValue::Boolean(s.ends_with(sub)))
                        } else {
                            eprintln!("[line {}] Type Error: 'endswith' expects a string argument.", line);
                            std::process::exit(70);
                        }
                    }
                    "split" => {
                        let delimiter = args.first().and_then(|v| match v {
                            HyperValue::String(st) => Some(st.clone()),
                            _ => None,
                        }).unwrap_or_else(|| " ".to_string());
    
                        let parts: Vec<HyperValue> = s
                            .split(&delimiter)
                            .map(|part| HyperValue::String(part.to_string()))
                            .collect();
                        Some(HyperValue::List(parts))
                    }
                    "rsplit" => {
                        let delimiter = args.first().and_then(|v| match v {
                            HyperValue::String(st) => Some(st.clone()),
                            _ => None,
                        }).unwrap_or_else(|| " ".to_string());
    
                        let parts: Vec<HyperValue> = s
                            .rsplit(&delimiter)
                            .map(|part| HyperValue::String(part.to_string()))
                            .collect();
                        Some(HyperValue::List(parts))
                    }
                    "replace" => {
                        if args.len() >= 2 {
                            if let (Some(HyperValue::String(old_s)), Some(HyperValue::String(new_s))) = (args.get(0), args.get(1)) {
                                let replaced = s.replace(old_s, new_s);
                                return Some(HyperValue::String(replaced));
                            }
                        }
                        eprintln!("[line {}] Type Error: 'replace' expects two string arguments.", line);
                        std::process::exit(70);
                    }
                    "find" => {
                        if let Some(HyperValue::String(sub)) = args.first() {
                            if let Some(idx) = s.find(sub) {
                                let char_idx = s[..idx].chars().count();
                                Some(HyperValue::I64(char_idx as i64))
                            } else {
                                Some(HyperValue::I64(-1))
                            }
                        } else {
                            eprintln!("[line {}] Type Error: 'find' expects a string argument.", line);
                            std::process::exit(70);
                        }
                    }
                    "rfind" => {
                        if let Some(HyperValue::String(sub)) = args.first() {
                            if let Some(idx) = s.rfind(sub) {
                                let char_idx = s[..idx].chars().count();
                                Some(HyperValue::I64(char_idx as i64))
                            } else {
                                Some(HyperValue::I64(-1))
                            }
                        } else {
                            eprintln!("[line {}] Type Error: 'rfind' expects a string argument.", line);
                            std::process::exit(70);
                        }
                    }
                    "count" => {
                        if let Some(HyperValue::String(sub)) = args.first() {
                            if sub.is_empty() {
                                Some(HyperValue::I64((s.chars().count() + 1) as i64))
                            } else {
                                let count = s.matches(sub).count();
                                Some(HyperValue::I64(count as i64))
                            }
                        } else {
                            eprintln!("[line {}] Type Error: 'count' expects a string argument.", line);
                            std::process::exit(70);
                        }
                    }
                    "isdigit" => {
                        let is_dig = !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
                        Some(HyperValue::Boolean(is_dig))
                    }
                    "isalpha" => {
                        let is_alp = !s.is_empty() && s.chars().all(|c| c.is_alphabetic());
                        Some(HyperValue::Boolean(is_alp))
                    }
                    "isalnum" => {
                        let is_aln = !s.is_empty() && s.chars().all(|c| c.is_alphanumeric());
                        Some(HyperValue::Boolean(is_aln))
                    }
                    "isspace" => {
                        let is_spc = !s.is_empty() && s.chars().all(|c| c.is_whitespace());
                        Some(HyperValue::Boolean(is_spc))
                    }
                    _ => {
                        eprintln!("[line {}] Attribute Error: String has no method '{}'.", line, method_name);
                        std::process::exit(70);
                    }
                }
            }
            HyperValue::MmapFile { file, .. } => {
                match method_name {
                    "read_chunk" => {
                        if args.len() != 2 {
                            eprintln!("[line {}] TypeError: read_chunk expects 2 arguments (offset, size).", line);
                            std::process::exit(70);
                        }
                        let offset = match args[0] {
                            HyperValue::I64(n) => n,
                            HyperValue::I32(n) => n as i64,
                            _ => 0,
                        };
                        let size = match args[1] {
                            HyperValue::I64(n) => n as usize,
                            HyperValue::I32(n) => n as usize,
                            _ => 0,
                        };
            
                        let mut f = file.borrow_mut();
                        if f.seek(SeekFrom::Start(offset as u64)).is_err() {
                            return Some(HyperValue::String("".to_string()));
                        }
                        let mut buffer = vec![0u8; size];
                        match f.read(&mut buffer) {
                            Ok(n) => {
                                buffer.truncate(n);
                                let chunk_str = String::from_utf8_lossy(&buffer).to_string();
                                Some(HyperValue::String(chunk_str))
                            }
                            Err(_) => Some(HyperValue::String("".to_string())),
                        }
                    }
                    _ => {
                        eprintln!("[line {}] Type Error: MmapFile has no method '{}'", line, method_name);
                        std::process::exit(70);
                    }
                }
            }
            _ => {
                eprintln!("[line {}] Type Error: Method calls are only supported on strings.", line);
                std::process::exit(70);
            }
        }
    }

    pub fn add(&self, other: &Self) -> Option<HyperValue> {
        if let (HyperValue::String(a), HyperValue::String(b)) = (self, other) {
            return Some(HyperValue::String(format!("{}{}", a, b)));
        }
        impl_binary_op!(self, other, +)
    }

    pub fn sub(&self, other: &Self) -> Option<HyperValue> { impl_binary_op!(self, other, -) }
    pub fn mul(&self, other: &Self) -> Option<HyperValue> { impl_binary_op!(self, other, *) }
    pub fn div(&self, other: &Self) -> Option<HyperValue> { impl_binary_op!(self, other, /) }
    pub fn rem(&self, other: &Self) -> Option<HyperValue> { impl_binary_op!(self, other, %) }
    pub fn pow(&self, other: &Self) -> Option<HyperValue> {
        match (self, other) {
            (HyperValue::I8(a), HyperValue::I8(b)) if *b >= 0 => Some(HyperValue::I8(a.pow(*b as u32))),
            (HyperValue::I16(a), HyperValue::I16(b)) if *b >= 0 => Some(HyperValue::I16(a.pow(*b as u32))),
            (HyperValue::I32(a), HyperValue::I32(b)) if *b >= 0 => Some(HyperValue::I32(a.pow(*b as u32))),
            (HyperValue::I64(a), HyperValue::I64(b)) if *b >= 0 => Some(HyperValue::I64(a.pow(*b as u32))),
            (HyperValue::U8(a), HyperValue::U8(b)) => Some(HyperValue::U8(a.pow(*b as u32))),
            (HyperValue::U16(a), HyperValue::U16(b)) => Some(HyperValue::U16(a.pow(*b as u32))),
            (HyperValue::U32(a), HyperValue::U32(b)) => Some(HyperValue::U32(a.pow(*b))),
            (HyperValue::U64(a), HyperValue::U64(b)) if *b <= u32::MAX as u64 => Some(HyperValue::U64(a.pow(*b as u32))),
            (HyperValue::F32(a), HyperValue::F32(b)) => Some(HyperValue::F32(a.powf(*b))),
            (HyperValue::F64(a), HyperValue::F64(b)) => Some(HyperValue::F64(a.powf(*b))),
            _ => None,
        }
    }

    pub fn greater(&self, other: &Self) -> Option<HyperValue> { impl_cmp_op!(self, other, >) }
    pub fn less(&self, other: &Self) -> Option<HyperValue> { impl_cmp_op!(self, other, <) }
    pub fn greater_equal(&self, other: &Self) -> Option<HyperValue> { impl_cmp_op!(self, other, >=) }
    pub fn less_equal(&self, other: &Self) -> Option<HyperValue> { impl_cmp_op!(self, other, <=) }

    pub fn negate(&self) -> Option<HyperValue> {
        match self {
            HyperValue::I8(n) => Some(HyperValue::I8(-n)),
            HyperValue::I16(n) => Some(HyperValue::I16(-n)),
            HyperValue::I32(n) => Some(HyperValue::I32(-n)),
            HyperValue::I64(n) => Some(HyperValue::I64(-n)),
            HyperValue::F32(n) => Some(HyperValue::F32(-n)),
            HyperValue::F64(n) => Some(HyperValue::F64(-n)),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub value: HyperValue,
    pub is_mutable: bool,
}

#[derive(Debug, Clone, PartialEq)] 
pub struct Environment {
    pub values: HashMap<String, Variable>,
    pub enclosing: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    pub fn new() -> Self {
        let mut env = Environment {
            values: HashMap::new(),
            enclosing: None,
        };

        env.define(
            "clock".to_string(),
            HyperValue::NativeFunction("clock".to_string()),
            false
        );

        env.define(
            "input".to_string(), 
            HyperValue::NativeFunction("input".to_string()), 
            false
        );

        env
    }

    pub fn new_with_enclosing(enclosing: Rc<RefCell<Environment>>) -> Self {
        Environment {
            values: HashMap::new(),
            enclosing: Some(enclosing),
        }
    }

    pub fn define(&mut self, name: String, value: HyperValue, is_mutable: bool) {
        self.values.insert(name, Variable { value, is_mutable });
    }

    pub fn get(&self, name: &str, line: u32) -> HyperValue {
        if let Some(let_entry) = self.values.get(name) {
            let_entry.value.clone()
        } else if let Some(ref enclosing) = self.enclosing {
            enclosing.borrow().get(name, line)
        } else {
            eprintln!("Undefined variable '{}'.", name);
            eprintln!("[line {}]", line);
            std::process::exit(70);
        }
    }

    pub fn assign(&mut self, name: &str, value: HyperValue, line: u32) {
        if let Some(let_entry) = self.values.get_mut(name) {
            if !let_entry.is_mutable {
                eprintln!("[line {}] Error: Cannot reassign immutable variable '{}'. Use 'let mut' to make it mutable.", line, name);
                std::process::exit(70);
            }
            let_entry.value = value;
            return;
        }

        if let Some(ref enclosing) = self.enclosing {
            enclosing.borrow_mut().assign(name, value, line);
            return;
        }

        eprintln!("[line {}] Error: Undefined variable '{}'.", line, name);
        std::process::exit(70);
    }
}

impl std::fmt::Display for HyperValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HyperValue::I8(n) => write!(f, "{}", n),
            HyperValue::I16(n) => write!(f, "{}", n),
            HyperValue::I32(n) => write!(f, "{}", n),
            HyperValue::I64(n) => write!(f, "{}", n),

            HyperValue::U8(n) => write!(f, "{}", n),
            HyperValue::U16(n) => write!(f, "{}", n),
            HyperValue::U32(n) => write!(f, "{}", n),
            HyperValue::U64(n) => write!(f, "{}", n),

            HyperValue::F32(n) => write!(f, "{}", n),
            HyperValue::F64(n) => write!(f, "{}", n),

            HyperValue::Boolean(b) => write!(f, "{}", b),
            HyperValue::String(s) => write!(f, "{}", s),
            HyperValue::None => write!(f, "None"),

            HyperValue::List(items) => {
                let items_str: Vec<String> = items.iter().map(|item| item.to_string()).collect();
                write!(f, "[{}]", items_str.join(", "))
            }
            HyperValue::Array { elements, .. } => {
                let items_str: Vec<String> = elements.iter().map(|item| item.to_string()).collect();
                write!(f, "[{}]", items_str.join(", "))
            }
            HyperValue::Dict { entries, .. } => {
                let entries_str: Vec<String> = entries.iter().map(|(k, v)| format!("{}: {}", k, v)).collect();
                write!(f, "{{{}}}", entries_str.join(", "))
            }
            HyperValue::MmapFile { path, .. } => write!(f, "<mmap file {}>", path),
            
            HyperValue::NativeFunction(name) => write!(f, "<native fn {}>", name),
            HyperValue::Function { name, .. } => write!(f, "<fn {}>", name),
        }
    }
}