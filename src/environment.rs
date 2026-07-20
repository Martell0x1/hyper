use std::collections::HashMap;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum LoxValue {
    Boolean(bool),
    Nil,
    Number(f64),
    StringLit(String),
}

pub struct Environment {
    values: HashMap<String, LoxValue>,
    enclosing: Option<Rc<RefCell<Environment>>>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            values: HashMap::new(),
            enclosing: None,
        }
    }

    pub fn new_with_enclosing(enclosing: Rc<RefCell<Environment>>) -> Self {
        Environment {
            values: HashMap::new(),
            enclosing: Some(enclosing),
        }
    }

    pub fn define(&mut self, name: String, value: LoxValue) {
        self.values.insert(name, value);
    }

    pub fn get(&self, name: &str, line: u32) -> LoxValue {
        if let Some(value) = self.values.get(name) {
            value.clone()
        } else if let Some(ref enclosing) = self.enclosing {
            enclosing.borrow().get(name, line)
        } else {
            eprintln!("Undefined variable '{}'.", name);
            eprintln!("[line {}]", line);
            std::process::exit(70);
        }
    }

    pub fn assign(&mut self, name: &str, value: LoxValue, line: u32) {
        if self.values.contains_key(name) {
            self.values.insert(name.to_string(), value);
        } else if let Some(ref enclosing) = self.enclosing {
            enclosing.borrow_mut().assign(name, value, line);
        } else {
            eprintln!("Undefined variable '{}'.", name);
            eprintln!("[line {}]", line);
            std::process::exit(70);
        }
    }
}

impl std::fmt::Display for LoxValue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoxValue::Boolean(b) => write!(f, "{}", b),
            LoxValue::Nil => write!(f, "nil"),
            LoxValue::Number(n) => write!(f, "{}", n),
            LoxValue::StringLit(s) => write!(f, "{}", s),
        }
    }
}