//! Host-side values used by the JSON bridge (`src/json.rs` ↔ compile runtime).
//!
//! Program execution uses Cranelift + `RtValue` only; this enum is not an interpreter.

use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::ast::Stmt;
use crate::fileio::{HyperFile, MappedFile};

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
        field_indices: Rc<HashMap<String, usize>>,
        field_visibility: Rc<HashMap<String, bool>>,
        field_mutability: Rc<HashMap<String, bool>>,
        methods: Rc<HashMap<String, (bool, HyperValue)>>,
    },
    TraitDef {
        name: String,
        methods: Vec<crate::ast::MethodSig>,
    },

    List(Rc<RefCell<Vec<HyperValue>>>),
    Array {
        element_type: String,
        elements: Rc<RefCell<Vec<HyperValue>>>,
    },
    Dict {
        key_type: String,
        val_type: String,
        entries: Rc<RefCell<IndexMap<String, HyperValue>>>,
    },
    Module {
        name: String,
        exports: HashMap<String, HyperValue>,
    },
    MmapFile {
        map: Rc<MappedFile>,
        path: String,
    },
    File {
        file: Rc<RefCell<HyperFile>>,
        path: String,
    },

    NativeFunction(String),
    Function {
        name: String,
        params: Vec<String>,
        param_refs: Vec<bool>,
        body: Rc<Stmt>,
        is_strict: bool,
        raises: bool,
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
            (HyperValue::List(a), HyperValue::List(b)) => *a.borrow() == *b.borrow(),
            (
                HyperValue::Array {
                    element_type: et1,
                    elements: el1,
                },
                HyperValue::Array {
                    element_type: et2,
                    elements: el2,
                },
            ) => et1 == et2 && *el1.borrow() == *el2.borrow(),
            (
                HyperValue::Dict {
                    key_type: kt1,
                    val_type: vt1,
                    entries: e1,
                },
                HyperValue::Dict {
                    key_type: kt2,
                    val_type: vt2,
                    entries: e2,
                },
            ) => kt1 == kt2 && vt1 == vt2 && *e1.borrow() == *e2.borrow(),
            (HyperValue::Module { name: n1, .. }, HyperValue::Module { name: n2, .. }) => n1 == n2,
            (HyperValue::MmapFile { path: a, .. }, HyperValue::MmapFile { path: b, .. }) => a == b,
            (HyperValue::File { path: a, .. }, HyperValue::File { path: b, .. }) => a == b,
            (HyperValue::NativeFunction(a), HyperValue::NativeFunction(b)) => a == b,
            (
                HyperValue::Function {
                    name: n1,
                    params: p1,
                    is_strict: s1,
                    raises: r1,
                    ..
                },
                HyperValue::Function {
                    name: n2,
                    params: p2,
                    is_strict: s2,
                    raises: r2,
                    ..
                },
            ) => n1 == n2 && p1 == p2 && s1 == s2 && r1 == r2,
            _ => false,
        }
    }
}

impl HyperValue {
    /// Integer view for JSON number serialization of integer-like values.
    pub fn to_int(&self) -> Option<i64> {
        match self {
            HyperValue::F32(n) => Some(*n as i64),
            HyperValue::F64(n) => Some(*n as i64),
            HyperValue::Boolean(b) => Some(*b as i64),
            other => other.to_i64_value(),
        }
    }

    fn to_i64_value(&self) -> Option<i64> {
        match self {
            HyperValue::I8(n) => Some(*n as i64),
            HyperValue::I16(n) => Some(*n as i64),
            HyperValue::I32(n) => Some(*n as i64),
            HyperValue::I64(n) => Some(*n),
            HyperValue::U8(n) => Some(*n as i64),
            HyperValue::U16(n) => Some(*n as i64),
            HyperValue::U32(n) => Some(*n as i64),
            HyperValue::U64(n) if *n <= i64::MAX as u64 => Some(*n as i64),
            _ => None,
        }
    }
}

impl fmt::Display for HyperValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
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
            HyperValue::String(s) => write!(f, "{}", s),
            HyperValue::Boolean(b) => write!(f, "{}", b),
            HyperValue::None => write!(f, "None"),
            HyperValue::StructDef { name, .. } => write!(f, "<struct {}>", name),
            HyperValue::Instance { struct_name, .. } => write!(f, "instance of {}", struct_name),
            HyperValue::TraitDef { name, .. } => write!(f, "<trait {}>", name),
            HyperValue::List(items) => {
                let items_str: Vec<String> = items.borrow().iter().map(|i| i.to_string()).collect();
                write!(f, "[{}]", items_str.join(", "))
            }
            HyperValue::Array { elements, .. } => {
                let items_str: Vec<String> =
                    elements.borrow().iter().map(|i| i.to_string()).collect();
                write!(f, "[{}]", items_str.join(", "))
            }
            HyperValue::Dict { entries, .. } => {
                let entries_str: Vec<String> = entries
                    .borrow()
                    .iter()
                    .map(|(k, v)| format!("{}: {}", k, v))
                    .collect();
                write!(f, "{{{}}}", entries_str.join(", "))
            }
            HyperValue::Module { name, .. } => write!(f, "<module {}>", name),
            HyperValue::MmapFile { path, .. } => write!(f, "<mmap file {}>", path),
            HyperValue::File { path, .. } => write!(f, "<file {}>", path),
            HyperValue::NativeFunction(name) => write!(f, "<native fn {}>", name),
            HyperValue::Function { name, .. } => write!(f, "<fn {}>", name),
        }
    }
}
