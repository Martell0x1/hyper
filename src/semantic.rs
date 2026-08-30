use crate::ast::*;
use crate::driver;
use crate::error;
use std::collections::{HashMap, HashSet};
use std::process;

#[derive(Debug, Clone, PartialEq)]
pub enum HyperType {
    None,
    Bool,
    String,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    F32,
    F64,
    List(Box<HyperType>),
    Dict,
    Array(Box<HyperType>),
    Function {
        params: Vec<HyperType>,
        ret: Box<HyperType>,
    },
    Struct(String),
    Trait(String),
    Mmap,
    File,
    Any,
}

#[derive(Debug, Clone)]
struct Binding {
    ty: HyperType,
    mutable: bool,
}

struct Scope {
    bindings: HashMap<String, Binding>,
    /// Names declared without a type annotation, whose numeric type may widen.
    inferred: HashSet<String>,
}

struct TypeChecker {
    scopes: Vec<Scope>,
    errors: Vec<String>,
    expected_return: Option<HyperType>,
    structs: HashMap<String, ()>,
    traits: HashMap<String, ()>,
}

impl TypeChecker {
    fn new() -> Self {
        let mut tc = TypeChecker {
            scopes: Vec::new(),
            errors: Vec::new(),
            expected_return: None,
            structs: HashMap::new(),
            traits: HashMap::new(),
        };
        tc.push_scope();
        // Builtins
        tc.define(
            "print",
            Binding {
                ty: HyperType::Function {
                    params: vec![HyperType::Any],
                    ret: Box::new(HyperType::None),
                },
                mutable: false,
            },
        );
        tc.define(
            "input",
            Binding {
                ty: HyperType::Function {
                    params: vec![HyperType::Any],
                    ret: Box::new(HyperType::String),
                },
                mutable: false,
            },
        );
        tc.define(
            "clock",
            Binding {
                ty: HyperType::Function {
                    params: vec![],
                    ret: Box::new(HyperType::F64),
                },
                mutable: false,
            },
        );
        tc.define(
            "open",
            Binding {
                ty: HyperType::Function {
                    params: vec![HyperType::Any],
                    ret: Box::new(HyperType::File),
                },
                mutable: false,
            },
        );
        tc
    }

    fn push_scope(&mut self) {
        self.scopes.push(Scope {
            bindings: HashMap::new(),
            inferred: HashSet::new(),
        });
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn define(&mut self, name: &str, binding: Binding) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.bindings.insert(name.to_string(), binding);
        }
    }

    fn lookup(&self, name: &str) -> Option<&Binding> {
        for scope in self.scopes.iter().rev() {
            if let Some(b) = scope.bindings.get(name) {
                return Some(b);
            }
        }
        None
    }

    /// Let an inferred numeric variable adopt a wider type instead of failing:
    /// `let mut sum = 0` must accept an i64 coming out of `range`.
    fn widen_inferred(&mut self, name: &str, ty: &HyperType) -> bool {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(binding) = scope.bindings.get_mut(name) {
                if !scope.inferred.contains(name) {
                    return false;
                }
                binding.ty = ty.clone();
                return true;
            }
        }
        false
    }

    fn error(&mut self, msg: String) {
        self.errors.push(error::format_typecheck(&msg));
    }

    fn resolve_type_name(&self, name: &str) -> HyperType {
        if let Some(inner) = name
            .strip_prefix("Array[")
            .and_then(|rest| rest.strip_suffix(']'))
        {
            return HyperType::Array(Box::new(self.resolve_type_name(inner.trim())));
        }
        if let Some(rest) = name
            .strip_prefix("Dict[")
            .and_then(|body| body.strip_suffix(']'))
        {
            if let Some((_key, val)) = rest.split_once(',') {
                let _ = self.resolve_type_name(val.trim());
            }
            return HyperType::Dict;
        }

        let ty = match name {
            "int8" => "i8",
            "int16" => "i16",
            "int32" => "i32",
            "int64" => "i64",
            "uint8" => "u8",
            "uint16" => "u16",
            "uint32" => "u32",
            "uint64" => "u64",
            "float32" => "f32",
            "float64" => "f64",
            "boolean" => "bool",
            other => other,
        };
        match ty {
            "None" | "none" => HyperType::None,
            "bool" => HyperType::Bool,
            "string" | "str" => HyperType::String,
            "i8" => HyperType::I8,
            "i16" => HyperType::I16,
            "i32" => HyperType::I32,
            "i64" => HyperType::I64,
            "u8" => HyperType::U8,
            "u16" => HyperType::U16,
            "u32" => HyperType::U32,
            "u64" => HyperType::U64,
            "f32" => HyperType::F32,
            "f64" => HyperType::F64,
            "any" | "Any" => HyperType::Any,
            "list" | "List" => HyperType::List(Box::new(HyperType::Any)),
            "dict" | "Dict" => HyperType::Dict,
            "mmap" | "Mmap" => HyperType::Mmap,
            "file" | "File" => HyperType::File,
            other => {
                if self.structs.contains_key(other) {
                    HyperType::Struct(other.to_string())
                } else if self.traits.contains_key(other) {
                    HyperType::Trait(other.to_string())
                } else {
                    // Unknown named type — treat softly as Any for forward compat.
                    HyperType::Any
                }
            }
        }
    }

    fn type_ann_to_hyper(&self, ann: &TypeAnn) -> HyperType {
        match ann {
            TypeAnn::None => HyperType::Any,
            TypeAnn::Named(name) => self.resolve_type_name(name),
            TypeAnn::Array { inner } => HyperType::Array(Box::new(self.resolve_type_name(inner))),
            TypeAnn::Dict { .. } => HyperType::Dict,
        }
    }

    fn is_numeric(ty: &HyperType) -> bool {
        matches!(
            ty,
            HyperType::I8
                | HyperType::I16
                | HyperType::I32
                | HyperType::I64
                | HyperType::U8
                | HyperType::U16
                | HyperType::U32
                | HyperType::U64
                | HyperType::F32
                | HyperType::F64
                | HyperType::Any
        )
    }

    fn is_boolish(ty: &HyperType) -> bool {
        matches!(ty, HyperType::Bool | HyperType::Any | HyperType::None)
            || Self::is_numeric(ty)
            || matches!(ty, HyperType::String)
    }

    fn numeric_rank(ty: &HyperType) -> Option<u8> {
        match ty {
            HyperType::I8 | HyperType::U8 => Some(1),
            HyperType::I16 | HyperType::U16 => Some(2),
            HyperType::I32 | HyperType::U32 => Some(3),
            HyperType::I64 | HyperType::U64 => Some(4),
            HyperType::F32 => Some(5),
            HyperType::F64 => Some(6),
            HyperType::Any => Some(0),
            _ => None,
        }
    }

    fn widen_numeric(a: &HyperType, b: &HyperType) -> HyperType {
        if matches!(a, HyperType::Any) {
            return b.clone();
        }
        if matches!(b, HyperType::Any) {
            return a.clone();
        }
        let ra = Self::numeric_rank(a).unwrap_or(0);
        let rb = Self::numeric_rank(b).unwrap_or(0);
        if ra >= rb {
            a.clone()
        } else {
            b.clone()
        }
    }

    fn is_compatible(dest: &HyperType, src: &HyperType) -> bool {
        if matches!(dest, HyperType::Any) || matches!(src, HyperType::Any) {
            return true;
        }
        if dest == src {
            return true;
        }
        // Numeric widening: source rank <= dest rank, or float destination.
        if let (Some(rd), Some(rs)) = (Self::numeric_rank(dest), Self::numeric_rank(src)) {
            return rs <= rd;
        }
        // List with Any element accepts any list.
        match (dest, src) {
            (HyperType::F32, HyperType::F64) => true,
            (HyperType::List(d), HyperType::List(s)) => {
                matches!(d.as_ref(), HyperType::Any) || Self::is_compatible(d, s)
            }
            (HyperType::Array(d), HyperType::Array(s)) => {
                matches!(d.as_ref(), HyperType::Any) || Self::is_compatible(d, s)
            }
            (HyperType::Array(d), HyperType::List(s)) => {
                matches!(d.as_ref(), HyperType::Any) || Self::is_compatible(d, s)
            }
            (HyperType::Dict, HyperType::Dict) => true,
            (HyperType::Struct(a), HyperType::Struct(b)) => a == b,
            _ => false,
        }
    }

    fn expr_fits_type(expr: &Expr, dest: &HyperType) -> bool {
        match (expr, dest) {
            (Expr::Literal(Literal::None), HyperType::None) => true,
            (Expr::Literal(Literal::Number(n)), HyperType::F32) => n.parse::<f64>().is_ok(),
            (Expr::Literal(Literal::Number(n)), HyperType::F64) => {
                n.parse::<f64>().is_ok()
            }
            (Expr::Literal(Literal::Number(n)), ty) if Self::is_numeric(ty) => {
                if matches!(
                    ty,
                    HyperType::U8 | HyperType::U16 | HyperType::U32 | HyperType::U64
                ) {
                    Self::parse_uint_literal(n).is_some_and(|v| Self::uint_fits(v, ty))
                } else {
                    Self::parse_int_literal(n).is_some_and(|v| Self::int_fits(v, ty))
                }
            }
            (
                Expr::Unary {
                    op: UnaryOp::Neg,
                    right,
                },
                ty,
            ) if Self::is_numeric(ty) => match right.as_ref() {
                Expr::Literal(Literal::Number(n)) => Self::parse_int_literal(n)
                    .and_then(|v| v.checked_neg())
                    .is_some_and(|v| Self::int_fits(v, ty)),
                _ => false,
            },
            _ => false,
        }
    }

    fn parse_int_literal(text: &str) -> Option<i64> {
        text.replace('_', "").parse::<i64>().ok()
    }

    fn int_fits(value: i64, ty: &HyperType) -> bool {
        match ty {
            HyperType::I8 => i8::MIN as i64 <= value && value <= i8::MAX as i64,
            HyperType::I16 => i16::MIN as i64 <= value && value <= i16::MAX as i64,
            HyperType::I32 => i32::MIN as i64 <= value && value <= i32::MAX as i64,
            HyperType::I64 => true,
            HyperType::U8 => 0 <= value && value <= u8::MAX as i64,
            HyperType::U16 => 0 <= value && value <= u16::MAX as i64,
            HyperType::U32 => 0 <= value && value <= u32::MAX as i64,
            HyperType::U64 => value >= 0,
            _ => false,
        }
    }

    fn parse_uint_literal(text: &str) -> Option<u64> {
        text.replace('_', "").parse::<u64>().ok()
    }

    fn uint_fits(value: u64, ty: &HyperType) -> bool {
        match ty {
            HyperType::U8 => value <= u64::from(u8::MAX),
            HyperType::U16 => value <= u64::from(u16::MAX),
            HyperType::U32 => value <= u64::from(u32::MAX),
            HyperType::U64 => true,
            _ => false,
        }
    }

    fn infer_literal(lit: &Literal) -> HyperType {
        match lit {
            Literal::None => HyperType::None,
            Literal::Bool(_) => HyperType::Bool,
            Literal::String(_) => HyperType::String,
            Literal::Number(n) => {
                if n.contains('.') || n.contains('e') || n.contains('E') {
                    HyperType::F64
                } else if n.parse::<i32>().is_ok() {
                    HyperType::I32
                } else if n.parse::<i64>().is_ok() {
                    HyperType::I64
                } else {
                    HyperType::F64
                }
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr) -> HyperType {
        match expr {
            Expr::Literal(lit) => Self::infer_literal(lit),
            Expr::Variable { name, line } => {
                if let Some(b) = self.lookup(name) {
                    b.ty.clone()
                } else if self.structs.contains_key(name) {
                    // Struct name used as constructor / type value.
                    HyperType::Struct(name.clone())
                } else {
                    self.error(format!(
                        "[line {}] Error: Undefined variable '{}'.",
                        line, name
                    ));
                    HyperType::Any
                }
            }
            Expr::Group(inner) => self.check_expr(inner),
            Expr::Unary { op, right } => {
                let rt = self.check_expr(right);
                match op {
                    UnaryOp::Neg => {
                        if !Self::is_numeric(&rt) {
                            self.error(format!(
                                "Type error: unary '-' requires a numeric operand, got {:?}.",
                                rt
                            ));
                        }
                        rt
                    }
                    UnaryOp::Not => {
                        // Soft: Not is always ok (truthiness).
                        HyperType::Bool
                    }
                }
            }
            Expr::Binary { op, left, right } => {
                let lt = self.check_expr(left);
                let rt = self.check_expr(right);
                self.check_binary(op, &lt, &rt)
            }
            Expr::Assign { name, value } => {
                let vt = self.check_expr(value);
                match self.lookup(name).cloned() {
                    Some(b) => {
                        if !b.mutable {
                            self.error(format!(
                                "Error: Cannot reassign immutable variable '{}'. Use 'let mut' to make it mutable.",
                                name
                            ));
                        } else if !Self::is_compatible(&b.ty, &vt)
                            && !matches!(b.ty, HyperType::Any)
                        {
                            let widened = Self::is_numeric(&b.ty)
                                && Self::is_numeric(&vt)
                                && self.widen_inferred(name, &vt);
                            // Soft: allow if annotated Any; otherwise warn-style error.
                            if !widened && !matches!(vt, HyperType::Any) {
                                self.error(format!(
                                    "Type error: cannot assign {:?} to '{}' of type {:?}.",
                                    vt, name, b.ty
                                ));
                            }
                            if widened {
                                return vt;
                            }
                        }
                        b.ty
                    }
                    None => {
                        self.error(format!(
                            "Error: Undefined variable '{}'.",
                            name
                        ));
                        HyperType::Any
                    }
                }
            }
            Expr::GetField { object, .. } => {
                // Soft: ensure object exists; field type is Any for now.
                if self.lookup(object).is_none() && !self.structs.contains_key(object) {
                    self.error(format!(
                        "Error: Undefined variable '{}'.",
                        object
                    ));
                }
                HyperType::Any
            }
            Expr::SetField {
                object,
                value,
                ..
            } => {
                if self.lookup(object).is_none() {
                    self.error(format!(
                        "Error: Undefined variable '{}'.",
                        object
                    ));
                }
                let _ = self.check_expr(value);
                HyperType::Any
            }
            Expr::Call { callee, args } => self.check_call(callee, args),
            Expr::CallMethod { object, args, .. } => {
                if self.lookup(object).is_none() {
                    self.error(format!(
                        "Error: Undefined variable '{}'.",
                        object
                    ));
                }
                for a in args {
                    let _ = self.check_expr(a);
                }
                HyperType::Any
            }
            Expr::List(items) => {
                let mut elem = HyperType::Any;
                for (i, item) in items.iter().enumerate() {
                    let t = self.check_expr(item);
                    if i == 0 {
                        elem = t;
                    } else if !Self::is_compatible(&elem, &t) && !Self::is_compatible(&t, &elem) {
                        elem = HyperType::Any;
                    } else {
                        elem = Self::widen_numeric(&elem, &t);
                    }
                }
                HyperType::List(Box::new(elem))
            }
            Expr::Dict(entries) => {
                for (k, v) in entries {
                    let _ = self.check_expr(k);
                    let _ = self.check_expr(v); // Soft: dict values when unknown
                }
                HyperType::Dict
            }
            Expr::Index { object, index } => {
                let ot = self.check_expr(object);
                let _ = self.check_expr(index);
                match ot {
                    HyperType::List(inner) | HyperType::Array(inner) => *inner,
                    HyperType::Dict => HyperType::Any,
                    HyperType::String => HyperType::String,
                    HyperType::Any => HyperType::Any,
                    other => {
                        self.error(format!(
                            "Type error: cannot index value of type {:?}.",
                            other
                        ));
                        HyperType::Any
                    }
                }
            }
            Expr::IndexSet {
                object,
                index,
                value,
            } => {
                let _ = self.check_expr(object);
                let _ = self.check_expr(index);
                self.check_expr(value)
            }
            Expr::FString { parts, .. } => {
                for part in parts {
                    if let FStringPart::Expr(e) = part {
                        let _ = self.check_expr(e); // Soft: f-string parts
                    }
                }
                HyperType::String
            }
            Expr::Ternary {
                condition,
                then_branch,
                else_branch,
            } => {
                let _ = self.check_expr(condition);
                let tt = self.check_expr(then_branch);
                let et = self.check_expr(else_branch);
                if Self::is_compatible(&tt, &et) || Self::is_compatible(&et, &tt) {
                    if Self::is_numeric(&tt) && Self::is_numeric(&et) {
                        Self::widen_numeric(&tt, &et)
                    } else {
                        tt
                    }
                } else {
                    HyperType::Any
                }
            }
        }
    }

    fn check_binary(&mut self, op: &BinOp, left: &HyperType, right: &HyperType) -> HyperType {
        match op {
            BinOp::Add => {
                if matches!(left, HyperType::String) && matches!(right, HyperType::String) {
                    return HyperType::String;
                }
                if matches!(left, HyperType::String) || matches!(right, HyperType::String) {
                    // Soft: string + other via coercion in interpreter — allow as String if either is string + Any
                    if matches!(left, HyperType::Any) || matches!(right, HyperType::Any) {
                        return HyperType::String;
                    }
                }
                if Self::is_numeric(left) && Self::is_numeric(right) {
                    return Self::widen_numeric(left, right);
                }
                if matches!(left, HyperType::Any) || matches!(right, HyperType::Any) {
                    return HyperType::Any;
                }
                self.error(format!(
                    "Type error: '+' requires numeric or string operands, got {:?} and {:?}.",
                    left, right
                ));
                HyperType::Any
            }
            BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Rem | BinOp::Pow => {
                if Self::is_numeric(left) && Self::is_numeric(right) {
                    return Self::widen_numeric(left, right);
                }
                self.error(format!(
                    "Type error: arithmetic '{}' requires numeric operands, got {:?} and {:?}.",
                    op, left, right
                ));
                HyperType::Any
            }
            BinOp::Eq | BinOp::Ne => HyperType::Bool,
            BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                let ok = (Self::is_numeric(left) && Self::is_numeric(right))
                    || (matches!(left, HyperType::Bool) && matches!(right, HyperType::Bool))
                    || (matches!(left, HyperType::String) && matches!(right, HyperType::String))
                    || matches!(left, HyperType::Any)
                    || matches!(right, HyperType::Any);
                if !ok {
                    self.error(format!(
                        "Type error: comparison requires numeric, bool, or string operands, got {:?} and {:?}.",
                        left, right
                    ));
                }
                HyperType::Bool
            }
            BinOp::And | BinOp::Or => {
                if !Self::is_boolish(left) || !Self::is_boolish(right) {
                    self.error(format!(
                        "Type error: '{}' requires bool-ish operands, got {:?} and {:?}.",
                        op, left, right
                    ));
                }
                HyperType::Bool
            }
        }
    }

    fn check_call(&mut self, callee: &Expr, args: &[CallArg]) -> HyperType {
        let callee_ty = self.check_expr(callee);

        // Collect positional arg types (named args still typechecked).
        let mut arg_tys = Vec::new();
        for arg in args {
            match arg {
                CallArg::Positional(e) => arg_tys.push(self.check_expr(e)),
                CallArg::Named { value, .. } => arg_tys.push(self.check_expr(value)),
            }
        }

        // Struct construction: Call on struct name.
        if let HyperType::Struct(ref name) = callee_ty {
            let _ = name;
            return callee_ty;
        }

        match &callee_ty {
            HyperType::Function { params, ret } => {
                // Arity check when callee type known (skip for print-style varargs soft).
                // print is registered with 1 Any param but accepts any arity — soft skip if Any params.
                let all_any = params.iter().all(|p| matches!(p, HyperType::Any))
                    && params.len() <= 1;
                if !all_any && params.len() != arg_tys.len() {
                    self.error(format!(
                        "Type error: expected {} argument(s) but got {}.",
                        params.len(),
                        arg_tys.len()
                    ));
                } else if !all_any {
                    for (i, (pt, at)) in params.iter().zip(arg_tys.iter()).enumerate() {
                        if !Self::is_compatible(pt, at) {
                            self.error(format!(
                                "Type error: argument {} expected {:?}, got {:?}.",
                                i + 1,
                                pt,
                                at
                            ));
                        }
                    }
                }
                ret.as_ref().clone()
            }
            HyperType::Any => HyperType::Any,
            _ => {
                // Soft: allow calling unknowns (e.g. before full inference).
                HyperType::Any
            }
        }
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                line,
                is_mutable,
                name,
                type_ann,
                initializer,
            } => {
                let init_ty = self.check_expr(initializer);
                let declared = match type_ann {
                    TypeAnn::None => init_ty.clone(),
                    other => {
                        let ann = self.type_ann_to_hyper(other);
                        if !Self::is_compatible(&ann, &init_ty)
                            && !Self::expr_fits_type(initializer, &ann)
                        {
                            self.error(format!(
                                "[line {}] Type error: cannot initialize '{}' of type {:?} with {:?}.",
                                line, name, ann, init_ty
                            ));
                        }
                        // Prefer the annotation when present.
                        if matches!(ann, HyperType::Any) {
                            init_ty
                        } else {
                            ann
                        }
                    }
                };
                self.define(
                    name,
                    Binding {
                        ty: declared,
                        mutable: *is_mutable,
                    },
                );
                if let Some(scope) = self.scopes.last_mut() {
                    if matches!(type_ann, TypeAnn::None) {
                        scope.inferred.insert(name.clone());
                    } else {
                        scope.inferred.remove(name);
                    }
                }
            }
            Stmt::Print { values, .. } => {
                for v in values {
                    let _ = self.check_expr(v);
                }
            }
            Stmt::Expr { expr, .. } => {
                let _ = self.check_expr(expr);
            }
            Stmt::Block(stmts) => {
                self.push_scope();
                self.hoist_functions(stmts);
                for s in stmts {
                    self.check_stmt(s);
                }
                self.pop_scope();
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                let _ = self.check_expr(condition);
                self.check_stmt(then_branch);
                if let Some(else_b) = else_branch {
                    self.check_stmt(else_b);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                let _ = self.check_expr(condition);
                self.check_stmt(body);
            }
            Stmt::For {
                var,
                iter,
                body,
                ..
            } => {
                match iter {
                    ForIter::Range { start, end } => {
                        let st = self.check_expr(start);
                        let et = self.check_expr(end);
                        if !Self::is_numeric(&st) {
                            self.error(format!(
                                "Type error: for-loop start must be numeric, got {:?}.",
                                st
                            ));
                        }
                        if !Self::is_numeric(&et) {
                            self.error(format!(
                                "Type error: for-loop end must be numeric, got {:?}.",
                                et
                            ));
                        }
                        self.push_scope();
                        self.define(
                            var,
                            Binding {
                                ty: HyperType::I64,
                                mutable: false,
                            },
                        );
                    }
                    ForIter::Iterable(iterable) => {
                        let it = self.check_expr(iterable);
                        let elem_ty = match it {
                            HyperType::List(inner) => *inner,
                            HyperType::Array(inner) => *inner,
                            HyperType::Any => HyperType::Any,
                            other => {
                                self.error(format!(
                                    "Type error: for-in iterable must be a list, got {:?}.",
                                    other
                                ));
                                HyperType::Any
                            }
                        };
                        self.push_scope();
                        self.define(
                            var,
                            Binding {
                                ty: elem_ty,
                                mutable: false,
                            },
                        );
                    }
                }
                self.check_stmt(body);
                self.pop_scope();
            }
            Stmt::Function(decl) => self.check_function(decl),
            Stmt::Return { line, value } => {
                let vt = self.check_expr(value);
                if let Some(ref expected) = self.expected_return {
                    if !Self::is_compatible(expected, &vt)
                        && !matches!(expected, HyperType::Any)
                        && !matches!(vt, HyperType::Any | HyperType::None)
                    {
                        self.error(format!(
                            "[line {}] Type error: return type {:?} is not compatible with {:?}.",
                            line, vt, expected
                        ));
                    }
                }
            }
            Stmt::Struct {
                name,
                implemented_trait,
                fields,
                methods,
            } => {
                if let Some(t) = implemented_trait {
                    if !self.traits.contains_key(t) {
                        // Soft: trait may be defined later — warn only if clearly missing later.
                        let _ = t;
                    }
                }
                for field in fields {
                    let _ = self.resolve_type_name(&field.type_name);
                }
                self.structs.insert(name.clone(), ());
                self.define(
                    name,
                    Binding {
                        ty: HyperType::Struct(name.clone()),
                        mutable: false,
                    },
                );
                for m in methods {
                    // Methods checked in a soft scope; register loosely.
                    self.check_function(&m.function);
                }
            }
            Stmt::Trait { name, methods } => {
                self.traits.insert(name.clone(), ());
                self.define(
                    name,
                    Binding {
                        ty: HyperType::Trait(name.clone()),
                        mutable: false,
                    },
                );
                for m in methods {
                    // Soft: just register signatures.
                    let params: Vec<HyperType> = m
                        .params
                        .iter()
                        .map(|p| {
                            p.type_ann
                                .as_ref()
                                .map(|t| self.resolve_type_name(t))
                                .unwrap_or(HyperType::Any)
                        })
                        .collect();
                    let ret = m
                        .return_type
                        .as_ref()
                        .map(|t| self.resolve_type_name(t))
                        .unwrap_or(HyperType::Any);
                    self.define(
                        &m.name,
                        Binding {
                            ty: HyperType::Function {
                                params,
                                ret: Box::new(ret),
                            },
                            mutable: false,
                        },
                    );
                }
            }
            Stmt::WithMmap {
                path, var, body, ..
            } => {
                let _ = self.check_expr(path);
                self.push_scope();
                self.define(
                    var,
                    Binding {
                        ty: HyperType::Mmap,
                        mutable: false,
                    },
                );
                self.check_stmt(body);
                self.pop_scope();
            }
            Stmt::With {
                value, var, body, ..
            } => {
                let ty = self.check_expr(value);
                self.push_scope();
                self.define(var, Binding { ty, mutable: false });
                self.check_stmt(body);
                self.pop_scope();
            }
            Stmt::Import {
                module, alias, ..
            } => {
                let bind = alias.as_ref().unwrap_or(module);
                self.define(
                    bind,
                    Binding {
                        ty: HyperType::Any,
                        mutable: false,
                    },
                );
            }
            Stmt::ImportFrom { names, .. } => {
                for item in names {
                    let bind = item.alias.as_ref().unwrap_or(&item.name);
                    self.define(
                        bind,
                        Binding {
                            ty: HyperType::Any,
                            mutable: false,
                        },
                    );
                }
            }
        }
    }

    /// Register a function signature so calls can appear before the definition.
    fn declare_function(&mut self, decl: &FunctionDecl) {
        let params: Vec<HyperType> = decl
            .params
            .iter()
            .map(|p| {
                p.type_ann
                    .as_ref()
                    .map(|t| self.resolve_type_name(t))
                    .unwrap_or(HyperType::Any)
            })
            .collect();
        let ret = decl
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_name(t))
            .unwrap_or(HyperType::Any);
        self.define(
            &decl.name,
            Binding {
                ty: HyperType::Function {
                    params,
                    ret: Box::new(ret),
                },
                mutable: false,
            },
        );
    }

    fn hoist_functions(&mut self, stmts: &[Stmt]) {
        for stmt in stmts {
            if let Stmt::Function(decl) = stmt {
                self.declare_function(decl);
            }
        }
    }

    fn check_function(&mut self, decl: &FunctionDecl) {
        let params: Vec<HyperType> = decl
            .params
            .iter()
            .map(|p| {
                p.type_ann
                    .as_ref()
                    .map(|t| self.resolve_type_name(t))
                    .unwrap_or(HyperType::Any)
            })
            .collect();
        let ret = decl
            .return_type
            .as_ref()
            .map(|t| self.resolve_type_name(t))
            .unwrap_or(HyperType::Any);

        // Register function in current scope before checking body (allows recursion).
        self.define(
            &decl.name,
            Binding {
                ty: HyperType::Function {
                    params: params.clone(),
                    ret: Box::new(ret.clone()),
                },
                mutable: false,
            },
        );

        self.push_scope();
        for (param, pty) in decl.params.iter().zip(params.iter()) {
            self.define(
                &param.name,
                Binding {
                    ty: pty.clone(),
                    mutable: true,
                },
            );
        }
        let prev_ret = self.expected_return.replace(ret);
        self.check_stmt(&decl.body);
        self.expected_return = prev_ret;
        self.pop_scope();
    }
}

pub fn typecheck(stmts: &[Stmt]) -> Result<(), Vec<String>> {
    let mut tc = TypeChecker::new();
    tc.hoist_functions(stmts);
    for stmt in stmts {
        tc.check_stmt(stmt);
    }
    if tc.errors.is_empty() {
        Ok(())
    } else {
        Err(tc.errors)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(source: &str) -> Result<(), Vec<String>> {
        let stmts = driver::parse_program(source).expect("source should parse");
        typecheck(&stmts)
    }

    #[test]
    fn functions_may_be_called_before_they_are_defined() {
        check(
            "fn outer(n: i64) -> i64:\n\
             \x20   return inner(n) + 1\n\
             \n\
             fn inner(n: i64) -> i64:\n\
             \x20   return n * 2\n\
             \n\
             print(outer(4))\n",
        )
        .expect("a forward call should typecheck");
    }

    #[test]
    fn inferred_counter_accepts_a_wider_number() {
        check(
            "let mut total = 0\n\
             for i in range(3):\n\
             \x20   total = total + i\n",
        )
        .expect("an inferred counter should widen");
    }

    #[test]
    fn literal_fits_smaller_integer_annotation() {
        check("let a: i8 = -128\n").expect("i8 literal should fit");
        check("let pi: float32 = 3.14\n").expect("float literal should fit f32");
    }

    #[test]
    fn annotated_variable_keeps_its_type() {
        let errors = check(
            "let mut total: i32 = 0\n\
             for i in range(3):\n\
             \x20   total = i\n",
        )
        .expect_err("an annotated variable should not widen");
        assert!(
            errors.iter().any(|e| e.contains("cannot assign")),
            "unexpected errors: {:?}",
            errors
        );
    }
}

pub fn run_typecheck(file_contents: String) {
    let stmts = match driver::parse_program(&file_contents) {
        Ok(s) => s,
        Err(()) => process::exit(65),
    };

    match typecheck(&stmts) {
        Ok(()) => {
            println!("Typecheck passed.");
        }
        Err(errors) => {
            for e in errors {
                error::report_formatted(&e);
            }
            process::exit(65);
        }
    }
}
