//! Hyper compiler core — IR, lowering, Cranelift codegen, and compile-time runtime.

#[path = "../../compiler/ir.rs"]
pub mod ir;

#[path = "../../compiler/runtime/mod.rs"]
pub mod runtime;

#[path = "../../compiler/codegen.rs"]
pub mod codegen;

#[path = "../../compiler/lowering.rs"]
pub mod lowering;

pub use lowering::{run_compile, CompileMode};
