use indexmap::IndexMap;
use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;
use crate::error;
use crate::ast::Stmt;
use crate::fileio::{call_file_method, call_mmap_method, HyperFile, MappedFile};
use crate::collection_utils::{call_dict_method, call_list_method};
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
        field_indices: Rc<HashMap<String, usize>>,
        /// Field name → declared `pub`.
        field_visibility: Rc<HashMap<String, bool>>,
        /// Field name → declared `mut`.
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
        /// Insertion-ordered so printing and iteration are reproducible.
        entries: Rc<RefCell<IndexMap<String, HyperValue>>>,
    },
    /// Loaded Hyper module namespace (`import math` → exports).
    Module {
        name: String,
        exports: HashMap<String, HyperValue>,
    },
    MmapFile {
        map: Rc<MappedFile>,
        path: String,
    },
    /// Buffered file handle returned by `open(...)`.
    File {
        file: Rc<RefCell<HyperFile>>,
        path: String,
    },

    NativeFunction(String),
    Function {
        name: String,
        params: Vec<String>,
        /// Parallel to `params`: true when the parameter was declared `ref`.
        param_refs: Vec<bool>,
        body: Rc<Stmt>,
        is_strict: bool,
        raises: bool,
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
            (HyperValue::List(a), HyperValue::List(b)) => *a.borrow() == *b.borrow(),
            (HyperValue::Array { element_type: et1, elements: el1 }, HyperValue::Array { element_type: et2, elements: el2 }) => {
                et1 == et2 && *el1.borrow() == *el2.borrow()
            }
            (HyperValue::Dict { key_type: kt1, val_type: vt1, entries: e1 }, HyperValue::Dict { key_type: kt2, val_type: vt2, entries: e2 }) => {
                kt1 == kt2 && vt1 == vt2 && *e1.borrow() == *e2.borrow()
            }
            (HyperValue::Module { name: n1, .. }, HyperValue::Module { name: n2, .. }) => n1 == n2,
            (HyperValue::MmapFile { path: a, .. }, HyperValue::MmapFile { path: b, .. }) => a == b,
            (HyperValue::File { path: a, .. }, HyperValue::File { path: b, .. }) => a == b,
            (HyperValue::NativeFunction(a), HyperValue::NativeFunction(b)) => a == b,
            (HyperValue::Function { name: n1, params: p1, is_strict: s1, raises: r1, .. }, HyperValue::Function { name: n2, params: p2, is_strict: s2, raises: r2, .. }) => {
                n1 == n2 && p1 == p2 && s1 == s2 && r1 == r2
            }
            _ => false,
        }
    }
}

/// Integer arithmetic wraps like the compiled path does, instead of aborting
/// the interpreter on overflow.
macro_rules! impl_binary_op {
    ($self:expr, $other:expr, $op:tt, $wrapping:ident) => {
        match ($self, $other) {
            (HyperValue::I8(a), HyperValue::I8(b)) => Some(HyperValue::I8(a.$wrapping(*b))),
            (HyperValue::I16(a), HyperValue::I16(b)) => Some(HyperValue::I16(a.$wrapping(*b))),
            (HyperValue::I32(a), HyperValue::I32(b)) => Some(HyperValue::I32(a.$wrapping(*b))),
            (HyperValue::I64(a), HyperValue::I64(b)) => Some(HyperValue::I64(a.$wrapping(*b))),
            (HyperValue::U8(a), HyperValue::U8(b)) => Some(HyperValue::U8(a.$wrapping(*b))),
            (HyperValue::U16(a), HyperValue::U16(b)) => Some(HyperValue::U16(a.$wrapping(*b))),
            (HyperValue::U32(a), HyperValue::U32(b)) => Some(HyperValue::U32(a.$wrapping(*b))),
            (HyperValue::U64(a), HyperValue::U64(b)) => Some(HyperValue::U64(a.$wrapping(*b))),
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
    pub fn call_method(&mut self, method_name: &str, args: &[HyperValue], line: u32) -> Option<HyperValue> {
        match self {
            HyperValue::String(s) => call_string_method(s, method_name, args, line),
            HyperValue::File { file, .. } => call_file_method(file, method_name, args, line),
            HyperValue::MmapFile { map, .. } => call_mmap_method(map, method_name, args, line),
            HyperValue::List(items) => {
                call_list_method(&mut items.borrow_mut(), method_name, args, line)
            }
            HyperValue::Array { elements, .. } => {
                call_list_method(&mut elements.borrow_mut(), method_name, args, line)
            }
            HyperValue::Dict { entries, .. } => {
                call_dict_method(&mut entries.borrow_mut(), method_name, args, line)
            }
            HyperValue::Instance { struct_name: _, fields: _, methods, .. } => {
                if methods.contains_key(method_name) {
                    Some(HyperValue::None)
                } else {
                    error::runtime(line, format!("method '{}' not found", method_name));
                }
            }
            _ => {
                error::runtime(line, format!("this type has no method '{}'", method_name));
            }
        }
    }

    pub fn add(&self, other: &Self) -> Option<HyperValue> {
        if let (HyperValue::String(a), HyperValue::String(b)) = (self, other) {
            let mut out = String::with_capacity(a.len() + b.len());
            out.push_str(a);
            out.push_str(b);
            return Some(HyperValue::String(out));
        }
        if let Some(v) = { impl_binary_op!(self, other, +, wrapping_add) } {
            return Some(v);
        }
        Self::numeric_promote_bin(self, other, |a, b| a.wrapping_add(b), |a, b| a + b)
    }

    pub fn sub(&self, other: &Self) -> Option<HyperValue> {
        if let Some(v) = { impl_binary_op!(self, other, -, wrapping_sub) } {
            return Some(v);
        }
        Self::numeric_promote_bin(self, other, |a, b| a.wrapping_sub(b), |a, b| a - b)
    }
    pub fn mul(&self, other: &Self) -> Option<HyperValue> {
        if let Some(v) = { impl_binary_op!(self, other, *, wrapping_mul) } {
            return Some(v);
        }
        Self::numeric_promote_bin(self, other, |a, b| a.wrapping_mul(b), |a, b| a * b)
    }
    pub fn div(&self, other: &Self) -> Option<HyperValue> {
        if let Some(v) = { impl_binary_op!(self, other, /, wrapping_div) } {
            return Some(v);
        }
        Self::numeric_promote_bin(self, other, |a, b| a.wrapping_div(b), |a, b| a / b)
    }
    pub fn floor_div(&self, other: &Self) -> Option<HyperValue> {
        if let (Some(a), Some(b)) = (self.to_int(), other.to_int()) {
            if b == 0 {
                return None;
            }
            return Some(HyperValue::I64(a.div_euclid(b)));
        }
        if let (Some(a), Some(b)) = (self.to_f64_value(), other.to_f64_value()) {
            if b == 0.0 {
                return None;
            }
            return Some(HyperValue::F64((a / b).floor()));
        }
        None
    }
    pub fn rem(&self, other: &Self) -> Option<HyperValue> {
        if let Some(v) = { impl_binary_op!(self, other, %, wrapping_rem) } {
            return Some(v);
        }
        Self::numeric_promote_bin(self, other, |a, b| a.wrapping_rem(b), |a, b| a % b)
    }

    /// Integer view of a value, for builtins that need counts or offsets.
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

    fn to_f64_value(&self) -> Option<f64> {
        match self {
            HyperValue::F32(n) => Some(*n as f64),
            HyperValue::F64(n) => Some(*n),
            other => other.to_i64_value().map(|n| n as f64),
        }
    }

    fn numeric_promote_bin<FI, FF>(
        left: &Self,
        right: &Self,
        int_op: FI,
        float_op: FF,
    ) -> Option<HyperValue>
    where
        FI: Fn(i64, i64) -> i64,
        FF: Fn(f64, f64) -> f64,
    {
        let left_float = matches!(left, HyperValue::F32(_) | HyperValue::F64(_));
        let right_float = matches!(right, HyperValue::F32(_) | HyperValue::F64(_));
        if left_float || right_float {
            let a = left.to_f64_value()?;
            let b = right.to_f64_value()?;
            return Some(HyperValue::F64(float_op(a, b)));
        }
        let a = left.to_i64_value()?;
        let b = right.to_i64_value()?;
        Some(HyperValue::I64(int_op(a, b)))
    }
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
            _ => Self::numeric_promote_bin(
                self,
                other,
                |a, b| if b < 0 { 0 } else { a.pow(b as u32) },
                |a, b| a.powf(b),
            ),
        }
    }

    pub fn greater(&self, other: &Self) -> Option<HyperValue> {
        if let Some(v) = { impl_cmp_op!(self, other, >) } {
            return Some(v);
        }
        Self::numeric_promote_cmp(self, other, |a, b| a > b, |a, b| a > b)
    }
    pub fn less(&self, other: &Self) -> Option<HyperValue> {
        if let Some(v) = { impl_cmp_op!(self, other, <) } {
            return Some(v);
        }
        Self::numeric_promote_cmp(self, other, |a, b| a < b, |a, b| a < b)
    }
    pub fn greater_equal(&self, other: &Self) -> Option<HyperValue> {
        if let Some(v) = { impl_cmp_op!(self, other, >=) } {
            return Some(v);
        }
        Self::numeric_promote_cmp(self, other, |a, b| a >= b, |a, b| a >= b)
    }
    pub fn less_equal(&self, other: &Self) -> Option<HyperValue> {
        if let Some(v) = { impl_cmp_op!(self, other, <=) } {
            return Some(v);
        }
        Self::numeric_promote_cmp(self, other, |a, b| a <= b, |a, b| a <= b)
    }

    fn numeric_promote_cmp<FI, FF>(
        left: &Self,
        right: &Self,
        int_op: FI,
        float_op: FF,
    ) -> Option<HyperValue>
    where
        FI: Fn(i64, i64) -> bool,
        FF: Fn(f64, f64) -> bool,
    {
        let left_float = matches!(left, HyperValue::F32(_) | HyperValue::F64(_));
        let right_float = matches!(right, HyperValue::F32(_) | HyperValue::F64(_));
        if left_float || right_float {
            let a = left.to_f64_value()?;
            let b = right.to_f64_value()?;
            return Some(HyperValue::Boolean(float_op(a, b)));
        }
        let a = left.to_i64_value()?;
        let b = right.to_i64_value()?;
        Some(HyperValue::Boolean(int_op(a, b)))
    }

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

        env.define(
            "open".to_string(),
            HyperValue::NativeFunction("open".to_string()),
            false,
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

    pub fn snapshot_bindings(&self) -> HashMap<String, HyperValue> {
        self.values
            .iter()
            .map(|(k, v)| (k.clone(), v.value.clone()))
            .collect()
    }

    /// Flatten this environment and its enclosing chain for parallel workers.
    pub fn snapshot_all_bindings(&self) -> HashMap<String, HyperValue> {
        let mut out = HashMap::new();
        if let Some(ref enclosing) = self.enclosing {
            out.extend(enclosing.borrow().snapshot_all_bindings());
        }
        out.extend(self.snapshot_bindings());
        out
    }

    pub fn get(&self, name: &str, line: u32) -> HyperValue {
        if let Some(let_entry) = self.values.get(name) {
            let_entry.value.clone()
        } else if let Some(ref enclosing) = self.enclosing {
            enclosing.borrow().get(name, line)
        } else {
            error::runtime(line, format!("name '{}' is not defined", name));
        }
    }

    pub fn assign(&mut self, name: &str, value: HyperValue, line: u32) {
        if let Some(let_entry) = self.values.get_mut(name) {
            if !let_entry.is_mutable {
                error::runtime(
                    line,
                    format!(
                        "cannot assign to immutable variable '{}'; use 'let mut'",
                        name
                    ),
                );
            }
            let_entry.value = value;
            return;
        }

        if let Some(ref enclosing) = self.enclosing {
            enclosing.borrow_mut().assign(name, value, line);
            return;
        }

        error::runtime(line, format!("name '{}' is not defined", name));
    }

    /// Mutate a binding in place (e.g. list/dict element update) without rebinding.
    pub fn with_value_mut<F, R>(&mut self, name: &str, line: u32, f: F) -> R
    where
        F: FnOnce(&mut HyperValue) -> R,
    {
        if let Some(let_entry) = self.values.get_mut(name) {
            return f(&mut let_entry.value);
        }
        if let Some(ref enclosing) = self.enclosing {
            return enclosing.borrow_mut().with_value_mut(name, line, f);
        }
        error::runtime(line, format!("name '{}' is not defined", name));
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
                let items_str: Vec<String> =
                    items.borrow().iter().map(|item| item.to_string()).collect();
                write!(f, "[{}]", items_str.join(", "))
            }
            HyperValue::Array { elements, .. } => {
                let items_str: Vec<String> = elements
                    .borrow()
                    .iter()
                    .map(|item| item.to_string())
                    .collect();
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