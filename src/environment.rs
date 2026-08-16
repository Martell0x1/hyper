use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone, PartialEq)]
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

    NativeFunction(String),
    Function {
        name: String,
        params: Vec<String>,
        body: String,
        is_strict: bool,
        closure: Rc<RefCell<Environment>>,
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
            
            HyperValue::NativeFunction(name) => write!(f, "<native fn {}>", name),
            HyperValue::Function { name, .. } => write!(f, "<fn {}>", name),
        }
    }
}