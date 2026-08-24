use std::collections::HashMap;
use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::rc::Rc;
use crate::text_utils::call_string_method;

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

    StructDef {
        name: String,
        implemented_trait: String,
        fields: Vec<(String, String, bool, bool, usize)>,
        methods: HashMap<String, (bool, HyperValue)>,
    },
    Instance {
        struct_name: String,
        fields: Rc<RefCell<Vec<HyperValue>>>, 
        field_indices: HashMap<String, usize>,
        field_visibility: HashMap<String, bool>,
        methods: HashMap<String, (bool, HyperValue)>,
    },
    TraitDef {
        name: String,
        methods: Vec<String>,
    },
    

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
            HyperValue::String(s) => call_string_method(s, method_name, args, line),
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
            HyperValue::Instance { struct_name: _, fields: _, methods, .. } => {
                if methods.contains_key(method_name) {
                    Some(HyperValue::None)
                } else {
                    eprintln!("[line {}] Error: Method '{}' not found.", line, method_name);
                    None
                }
            }
            _ => {
                eprintln!("[line {}] Type Error: Method calls are not supported on this type.", line);
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

            HyperValue::StructDef { name, .. } => write!(f, "struct {}", name),
            HyperValue::TraitDef { name, .. } => write!(f, "trait {}", name),
            HyperValue::Instance { struct_name, .. } => write!(f, "instance of {}", struct_name),

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