use crate::ir::{BlockId, IrInstr, IrModule, IrOp, ValueId};
use cranelift_codegen::entity::EntityRef;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::immediates::Ieee64;
use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder, MemFlags, StackSlotData, StackSlotKind, UserFuncName, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, DataDescription, DataId, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::collections::{HashMap, HashSet};
use std::mem;
use std::path::PathBuf;
use std::process::Command;

use crate::runtime::{
    hyper_rt_dict_get, hyper_rt_dict_new, hyper_rt_dict_push, hyper_rt_dict_set,
    hyper_rt_list_get, hyper_rt_list_len, hyper_rt_list_new, hyper_rt_list_push,
    hyper_rt_list_set, hyper_rt_pow_f64, hyper_rt_pow_i64, hyper_rt_print_dict,
    hyper_rt_print_f64, hyper_rt_print_i64, hyper_rt_print_list, hyper_rt_print_newline,
    hyper_rt_print_separator, hyper_rt_print_str, hyper_rt_print_struct, hyper_rt_print_value,
    hyper_rt_str_concat,
    hyper_rt_div_by_zero, hyper_rt_struct_get, hyper_rt_struct_new, hyper_rt_struct_set,
    hyper_rt_value_eq, hyper_rt_value_to_str,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    I64,
    F64,
    Str,
    Bool,
    None_,
    List,
    Dict,
    Struct,
    /// Element kind known only at runtime (e.g. after index_get).
    Dynamic,
}

impl ValueKind {
    fn as_i64(self) -> i64 {
        match self {
            ValueKind::I64 => 0,
            ValueKind::F64 => 1,
            ValueKind::Str => 2,
            ValueKind::Bool => 3,
            ValueKind::None_ => 4,
            ValueKind::List => 5,
            ValueKind::Dict => 6,
            ValueKind::Struct => 7,
            ValueKind::Dynamic => 0,
        }
    }
}

struct StringData {
    next: usize,
}

impl StringData {
    fn new() -> Self {
        StringData { next: 0 }
    }

    fn define<M: Module>(&mut self, module: &mut M, s: &str) -> Result<DataId, String> {
        let name = format!(".hyper_str.{}", self.next);
        self.next += 1;
        let id = module
            .declare_data(&name, Linkage::Local, false, false)
            .map_err(|e| e.to_string())?;
        let mut desc = DataDescription::new();
        let mut bytes = s.as_bytes().to_vec();
        bytes.push(0);
        desc.define(bytes.into_boxed_slice());
        module.define_data(id, &desc).map_err(|e| e.to_string())?;
        Ok(id)
    }
}

struct RuntimeIds {
    print_i64: FuncId,
    print_f64: FuncId,
    print_str: FuncId,
    print_nl: FuncId,
    print_sep: FuncId,
    print_list: FuncId,
    print_dict: FuncId,
    print_value: FuncId,
    pow_i64: FuncId,
    pow_f64: FuncId,
    list_new: FuncId,
    list_push: FuncId,
    list_get: FuncId,
    list_set: FuncId,
    list_len: FuncId,
    dict_new: FuncId,
    dict_push: FuncId,
    dict_get: FuncId,
    dict_set: FuncId,
    value_to_str: FuncId,
    value_eq: FuncId,
    div_by_zero: FuncId,
    str_concat: FuncId,
    struct_new: FuncId,
    struct_get: FuncId,
    struct_set: FuncId,
    print_struct: FuncId,
}

fn make_flags(is_pic: bool) -> Result<settings::Flags, String> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| e.to_string())?;
    flag_builder
        .set("is_pic", if is_pic { "true" } else { "false" })
        .map_err(|e| e.to_string())?;
    Ok(settings::Flags::new(flag_builder))
}

fn declare_runtime<M: Module>(module: &mut M) -> Result<RuntimeIds, String> {
    let print_i64 = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_print_i64", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let print_f64 = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::F64));
        module
            .declare_function("hyper_rt_print_f64", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let print_str = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_print_str", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let print_nl = {
        let sig = module.make_signature();
        module
            .declare_function("hyper_rt_print_newline", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let print_sep = {
        let sig = module.make_signature();
        module
            .declare_function("hyper_rt_print_separator", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let print_list = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_print_list", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let print_dict = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_print_dict", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let pow_i64 = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_pow_i64", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let pow_f64 = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::F64));
        sig.params.push(AbiParam::new(types::F64));
        sig.returns.push(AbiParam::new(types::F64));
        module
            .declare_function("hyper_rt_pow_f64", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let list_new = {
        let mut sig = module.make_signature();
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_list_new", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let list_push = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_list_push", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let dict_new = {
        let mut sig = module.make_signature();
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_dict_new", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let dict_push = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_dict_push", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let print_value = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_print_value", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let list_get = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_list_get", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let list_set = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_list_set", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let dict_get = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_dict_get", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let dict_set = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_dict_set", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let list_len = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_list_len", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let value_to_str = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_value_to_str", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let value_eq = {
        let mut sig = module.make_signature();
        for _ in 0..4 {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_value_eq", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let div_by_zero = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_div_by_zero", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let str_concat = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_str_concat", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let struct_new = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_struct_new", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let struct_get = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_struct_get", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let struct_set = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_struct_set", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let print_struct = {
        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        module
            .declare_function("hyper_rt_print_struct", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    Ok(RuntimeIds {
        print_i64,
        print_f64,
        print_str,
        print_nl,
        print_sep,
        print_list,
        print_dict,
        print_value,
        pow_i64,
        pow_f64,
        list_new,
        list_push,
        list_get,
        list_set,
        list_len,
        dict_new,
        dict_push,
        dict_get,
        dict_set,
        value_to_str,
        value_eq,
        div_by_zero,
        str_concat,
        struct_new,
        struct_get,
        struct_set,
        print_struct,
    })
}

fn declare_user_funcs<M: Module>(
    module: &mut M,
    ir: &IrModule,
) -> Result<HashMap<String, FuncId>, String> {
    let mut func_ids: HashMap<String, FuncId> = HashMap::new();
    for func in &ir.functions {
        let mut sig = module.make_signature();
        // Arguments and results are passed as (payload, kind) pairs so values
        // keep their type across call boundaries.
        for _ in &func.params {
            sig.params.push(AbiParam::new(types::I64));
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        let id = module
            .declare_function(&func.name, Linkage::Local, &sig)
            .map_err(|e| e.to_string())?;
        func_ids.insert(func.name.clone(), id);
    }

    let mut sig = module.make_signature();
    sig.returns.push(AbiParam::new(types::I64));
    let main_id = module
        .declare_function("__main__", Linkage::Export, &sig)
        .map_err(|e| e.to_string())?;
    func_ids.insert("__main__".to_string(), main_id);
    Ok(func_ids)
}

fn kind_of(map: &HashMap<ValueId, ValueKind>, id: ValueId) -> ValueKind {
    map.get(&id).copied().unwrap_or(ValueKind::I64)
}

/// Kinds whose `==` cannot be a raw payload comparison.
fn needs_runtime_eq(kind: ValueKind) -> bool {
    matches!(
        kind,
        ValueKind::Str | ValueKind::List | ValueKind::Dict | ValueKind::Struct | ValueKind::Dynamic
    )
}

fn kind_operand(
    builder: &mut FunctionBuilder,
    kind_vars: &HashMap<ValueId, Variable>,
    kind: ValueKind,
    id: ValueId,
) -> Value {
    match kind {
        ValueKind::Dynamic => match kind_vars.get(&id) {
            Some(kv) => builder.use_var(*kv),
            None => builder.ins().iconst(types::I64, 0),
        },
        other => builder.ins().iconst(types::I64, other.as_i64()),
    }
}

fn named_kind(map: &HashMap<String, ValueKind>, name: &str) -> ValueKind {
    map.get(name).copied().unwrap_or(ValueKind::I64)
}

fn i64_to_f64(builder: &mut FunctionBuilder, v: Value) -> Value {
    builder.ins().bitcast(types::F64, MemFlags::new(), v)
}

fn f64_to_i64(builder: &mut FunctionBuilder, v: Value) -> Value {
    builder.ins().bitcast(types::I64, MemFlags::new(), v)
}

pub fn dump_ir(module: &IrModule) {
    println!("{}", module);
}

fn register_jit_symbols(jit_builder: &mut JITBuilder) {
    jit_builder.symbol("hyper_rt_print_i64", hyper_rt_print_i64 as *const u8);
    jit_builder.symbol("hyper_rt_print_f64", hyper_rt_print_f64 as *const u8);
    jit_builder.symbol("hyper_rt_print_str", hyper_rt_print_str as *const u8);
    jit_builder.symbol(
        "hyper_rt_print_newline",
        hyper_rt_print_newline as *const u8,
    );
    jit_builder.symbol(
        "hyper_rt_print_separator",
        hyper_rt_print_separator as *const u8,
    );
    jit_builder.symbol("hyper_rt_print_list", hyper_rt_print_list as *const u8);
    jit_builder.symbol("hyper_rt_print_dict", hyper_rt_print_dict as *const u8);
    jit_builder.symbol("hyper_rt_print_value", hyper_rt_print_value as *const u8);
    jit_builder.symbol("hyper_rt_pow_i64", hyper_rt_pow_i64 as *const u8);
    jit_builder.symbol("hyper_rt_pow_f64", hyper_rt_pow_f64 as *const u8);
    jit_builder.symbol("hyper_rt_list_new", hyper_rt_list_new as *const u8);
    jit_builder.symbol("hyper_rt_list_push", hyper_rt_list_push as *const u8);
    jit_builder.symbol("hyper_rt_list_get", hyper_rt_list_get as *const u8);
    jit_builder.symbol("hyper_rt_list_set", hyper_rt_list_set as *const u8);
    jit_builder.symbol("hyper_rt_list_len", hyper_rt_list_len as *const u8);
    jit_builder.symbol("hyper_rt_dict_new", hyper_rt_dict_new as *const u8);
    jit_builder.symbol("hyper_rt_dict_push", hyper_rt_dict_push as *const u8);
    jit_builder.symbol("hyper_rt_dict_get", hyper_rt_dict_get as *const u8);
    jit_builder.symbol("hyper_rt_dict_set", hyper_rt_dict_set as *const u8);
    jit_builder.symbol("hyper_rt_value_to_str", hyper_rt_value_to_str as *const u8);
    jit_builder.symbol("hyper_rt_value_eq", hyper_rt_value_eq as *const u8);
    jit_builder.symbol("hyper_rt_div_by_zero", hyper_rt_div_by_zero as *const u8);
    jit_builder.symbol("hyper_rt_str_concat", hyper_rt_str_concat as *const u8);
    jit_builder.symbol("hyper_rt_struct_new", hyper_rt_struct_new as *const u8);
    jit_builder.symbol("hyper_rt_struct_get", hyper_rt_struct_get as *const u8);
    jit_builder.symbol("hyper_rt_struct_set", hyper_rt_struct_set as *const u8);
    jit_builder.symbol("hyper_rt_print_struct", hyper_rt_print_struct as *const u8);
}

pub fn jit_execute(module: &IrModule) -> Result<(), String> {
    let flags = make_flags(false)?;
    let isa_builder =
        cranelift_native::builder().map_err(|msg| format!("host unsupported: {msg}"))?;
    let isa = isa_builder.finish(flags).map_err(|e| e.to_string())?;

    let mut jit_builder = JITBuilder::with_isa(isa, default_libcall_names());
    register_jit_symbols(&mut jit_builder);

    let mut jit = JITModule::new(jit_builder);
    let mut ctx = jit.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut strings = StringData::new();

    let runtime = declare_runtime(&mut jit)?;
    let func_ids = declare_user_funcs(&mut jit, module)?;
    let main_id = func_ids["__main__"];

    for func in &module.functions {
        let id = func_ids[&func.name];
        define_function(
            &mut jit,
            &mut ctx,
            &mut func_ctx,
            id,
            &func.params,
            &func.body,
            &func_ids,
            &runtime,
            &mut strings,
            true,
        )?;
    }

    define_function(
        &mut jit,
        &mut ctx,
        &mut func_ctx,
        main_id,
        &[],
        &module.main,
        &func_ids,
        &runtime,
        &mut strings,
        false,
    )?;

    jit.finalize_definitions().map_err(|e| e.to_string())?;

    let code = jit.get_finalized_function(main_id);
    let main_fn: extern "C" fn() -> i64 = unsafe { mem::transmute(code) };
    let _ = main_fn();
    Ok(())
}

pub fn emit_object(module: &IrModule, out_path: &str) -> Result<(), String> {
    let flags = make_flags(true)?;
    let isa_builder =
        cranelift_native::builder().map_err(|msg| format!("host unsupported: {msg}"))?;
    let isa = isa_builder.finish(flags).map_err(|e| e.to_string())?;

    let builder = ObjectBuilder::new(isa, "hyper", default_libcall_names())
        .map_err(|e| e.to_string())?;
    let mut obj = ObjectModule::new(builder);
    let mut ctx = obj.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut strings = StringData::new();

    let runtime = declare_runtime(&mut obj)?;
    let func_ids = declare_user_funcs(&mut obj, module)?;
    let main_id = func_ids["__main__"];

    for func in &module.functions {
        let id = func_ids[&func.name];
        define_function(
            &mut obj,
            &mut ctx,
            &mut func_ctx,
            id,
            &func.params,
            &func.body,
            &func_ids,
            &runtime,
            &mut strings,
            true,
        )?;
    }

    define_function(
        &mut obj,
        &mut ctx,
        &mut func_ctx,
        main_id,
        &[],
        &module.main,
        &func_ids,
        &runtime,
        &mut strings,
        false,
    )?;

    let product = obj.finish();
    let bytes = product.emit().map_err(|e| e.to_string())?;
    std::fs::write(out_path, &bytes).map_err(|e| e.to_string())?;
    Ok(())
}

fn runtime_c_path() -> Result<PathBuf, String> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let path = manifest.join("src").join("runtime").join("hyper_rt.c");
    if !path.exists() {
        return Err(format!("runtime source not found: {}", path.display()));
    }
    Ok(path)
}

fn find_cc() -> Result<&'static str, String> {
    for cand in ["cc", "clang", "gcc"] {
        if Command::new(cand)
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return Ok(cand);
        }
    }
    Err("no C compiler found (tried cc, clang, gcc)".to_string())
}

pub fn emit_exe(module: &IrModule, out_path: &str) -> Result<(), String> {
    let tmp_dir = std::env::temp_dir();
    let obj_path = tmp_dir.join(format!(
        "hyper_{}.o",
        std::process::id()
    ));
    let obj_str = obj_path
        .to_str()
        .ok_or_else(|| "temp object path is not valid UTF-8".to_string())?;

    emit_object(module, obj_str)?;

    let rt = runtime_c_path()?;
    let cc = find_cc()?;
    let status = Command::new(cc)
        .arg(obj_str)
        .arg(rt.as_os_str())
        .arg("-o")
        .arg(out_path)
        .arg("-lm")
        .status()
        .map_err(|e| format!("failed to invoke {cc}: {e}"))?;

    let _ = std::fs::remove_file(&obj_path);

    if !status.success() {
        return Err(format!("{cc} failed with status {status}"));
    }
    Ok(())
}

fn define_function<M: Module>(
    module: &mut M,
    ctx: &mut cranelift_codegen::Context,
    func_ctx: &mut FunctionBuilderContext,
    func_id: FuncId,
    params: &[String],
    body: &[IrInstr],
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeIds,
    strings: &mut StringData,
    returns_kind: bool,
) -> Result<(), String> {
    let mut sig = module.make_signature();
    for _ in params {
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
    }
    sig.returns.push(AbiParam::new(types::I64));
    if returns_kind {
        sig.returns.push(AbiParam::new(types::I64));
    }

    ctx.func = Function::new();
    ctx.func.signature = sig;
    ctx.func.name = UserFuncName::user(0, func_id.as_u32());

    {
        let mut builder = FunctionBuilder::new(&mut ctx.func, func_ctx);

        let entry = builder.create_block();
        builder.append_block_params_for_function_params(entry);

        let mut blocks: HashMap<BlockId, cranelift_codegen::ir::Block> = HashMap::new();
        for instr in body {
            if let IrInstr::Label { block } = instr {
                blocks
                    .entry(*block)
                    .or_insert_with(|| builder.create_block());
            }
        }

        builder.switch_to_block(entry);
        builder.seal_block(entry);

        let mut next_var = 0usize;
        let mut value_vars: HashMap<ValueId, Variable> = HashMap::new();
        let mut named_vars: HashMap<String, Variable> = HashMap::new();
        let mut named_defs: HashSet<String> = HashSet::new();
        let mut value_kinds: HashMap<ValueId, ValueKind> = HashMap::new();
        let mut named_kinds: HashMap<String, ValueKind> = HashMap::new();
        let mut kind_vars: HashMap<ValueId, Variable> = HashMap::new();
        let mut named_kind_vars: HashMap<String, Variable> = HashMap::new();
        let mut terminated = false;

        let declare_var = |builder: &mut FunctionBuilder, next_var: &mut usize| {
            let v = Variable::new(*next_var);
            *next_var += 1;
            builder.declare_var(v, types::I64);
            v
        };

        let param_vals: Vec<Value> = builder.block_params(entry).to_vec();
        for (i, name) in params.iter().enumerate() {
            let var = declare_var(&mut builder, &mut next_var);
            builder.def_var(var, param_vals[i * 2]);
            named_vars.insert(name.clone(), var);
            named_defs.insert(name.clone());

            let kv = declare_var(&mut builder, &mut next_var);
            builder.def_var(kv, param_vals[i * 2 + 1]);
            named_kind_vars.insert(name.clone(), kv);
            named_kinds.insert(name.clone(), ValueKind::Dynamic);
        }

        let ensure_val = |id: ValueId,
                          builder: &mut FunctionBuilder,
                          next_var: &mut usize,
                          value_vars: &mut HashMap<ValueId, Variable>| {
            if !value_vars.contains_key(&id) {
                let v = declare_var(builder, next_var);
                value_vars.insert(id, v);
            }
        };
        let ensure_named = |name: &str,
                            builder: &mut FunctionBuilder,
                            next_var: &mut usize,
                            named_vars: &mut HashMap<String, Variable>| {
            if !named_vars.contains_key(name) {
                let v = declare_var(builder, next_var);
                named_vars.insert(name.to_string(), v);
            }
        };

        for instr in body {
            match instr {
                IrInstr::ConstI64 { dest, .. }
                | IrInstr::ConstF64 { dest, .. }
                | IrInstr::ConstBool { dest, .. }
                | IrInstr::ConstNone { dest }
                | IrInstr::ConstStr { dest, .. } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::Load { dest, name } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    ensure_named(name, &mut builder, &mut next_var, &mut named_vars);
                }
                IrInstr::Store { name, value } => {
                    ensure_named(name, &mut builder, &mut next_var, &mut named_vars);
                    ensure_val(*value, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::Unary { dest, src, .. } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*src, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::Binary {
                    dest,
                    left,
                    right,
                    ..
                } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*left, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*right, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::Call { dest, args, .. } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    for a in args {
                        ensure_val(*a, &mut builder, &mut next_var, &mut value_vars);
                    }
                }
                IrInstr::MakeList { dest, items } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    for a in items {
                        ensure_val(*a, &mut builder, &mut next_var, &mut value_vars);
                    }
                }
                IrInstr::MakeDict { dest, entries } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    for (k, v) in entries {
                        ensure_val(*k, &mut builder, &mut next_var, &mut value_vars);
                        ensure_val(*v, &mut builder, &mut next_var, &mut value_vars);
                    }
                }
                IrInstr::IndexGet {
                    dest,
                    object,
                    index,
                } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*object, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*index, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::IndexSet {
                    object,
                    index,
                    value,
                } => {
                    ensure_val(*object, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*index, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*value, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::ListLen { dest, list } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*list, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::GuardDivisor { value, .. } => {
                    ensure_val(*value, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::ValueToStr { dest, src } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*src, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::StrConcat { dest, left, right } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*left, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*right, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::MakeStruct { dest, .. } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::StructGet { dest, object, .. } => {
                    ensure_val(*dest, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*object, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::StructSet { object, value, .. } => {
                    ensure_val(*object, &mut builder, &mut next_var, &mut value_vars);
                    ensure_val(*value, &mut builder, &mut next_var, &mut value_vars);
                }
                IrInstr::Print { args } => {
                    for a in args {
                        ensure_val(*a, &mut builder, &mut next_var, &mut value_vars);
                    }
                }
                IrInstr::Return { value: Some(id) } | IrInstr::Branch { cond: id, .. } => {
                    ensure_val(*id, &mut builder, &mut next_var, &mut value_vars);
                }
                _ => {}
            }
        }

        for (name, var) in &named_vars {
            if !named_defs.contains(name) {
                let zero = builder.ins().iconst(types::I64, 0);
                builder.def_var(*var, zero);
                named_defs.insert(name.clone());
            }
        }

        // Every named slot carries a runtime kind so a name written with
        // different kinds on different paths stays printable/inspectable.
        let named_names: Vec<String> = named_vars.keys().cloned().collect();
        for name in named_names {
            if named_kind_vars.contains_key(&name) {
                continue;
            }
            let kv = declare_var(&mut builder, &mut next_var);
            let init = builder.ins().iconst(types::I64, ValueKind::I64.as_i64());
            builder.def_var(kv, init);
            named_kind_vars.insert(name, kv);
        }

        for instr in body {
            match instr {
                IrInstr::Label { block } => {
                    let b = blocks[block];
                    if !terminated {
                        builder.ins().jump(b, &[]);
                    }
                    builder.switch_to_block(b);
                    terminated = false;
                }
                _ if terminated => {}
                IrInstr::ConstI64 { dest, value } => {
                    let v = builder.ins().iconst(types::I64, *value);
                    builder.def_var(value_vars[dest], v);
                    value_kinds.insert(*dest, ValueKind::I64);
                }
                IrInstr::ConstF64 { dest, value } => {
                    let fv = builder
                        .ins()
                        .f64const(Ieee64::with_bits(value.to_bits()));
                    let v = f64_to_i64(&mut builder, fv);
                    builder.def_var(value_vars[dest], v);
                    value_kinds.insert(*dest, ValueKind::F64);
                }
                IrInstr::ConstBool { dest, value } => {
                    let v = builder
                        .ins()
                        .iconst(types::I64, if *value { 1 } else { 0 });
                    builder.def_var(value_vars[dest], v);
                    value_kinds.insert(*dest, ValueKind::Bool);
                }
                IrInstr::ConstNone { dest } => {
                    let v = builder.ins().iconst(types::I64, 0);
                    builder.def_var(value_vars[dest], v);
                    value_kinds.insert(*dest, ValueKind::None_);
                }
                IrInstr::ConstStr { dest, value } => {
                    let data_id = strings.define(module, value)?;
                    let gv = module.declare_data_in_func(data_id, &mut builder.func);
                    let v = builder.ins().global_value(types::I64, gv);
                    builder.def_var(value_vars[dest], v);
                    value_kinds.insert(*dest, ValueKind::Str);
                }
                IrInstr::Load { dest, name } => {
                    let val = builder.use_var(named_vars[name]);
                    builder.def_var(value_vars[dest], val);
                    let nk = named_kind(&named_kinds, name);
                    value_kinds.insert(*dest, nk);
                    if nk == ValueKind::Dynamic {
                        if let Some(kv) = named_kind_vars.get(name) {
                            let k = builder.use_var(*kv);
                            let dest_kv = declare_var(&mut builder, &mut next_var);
                            builder.def_var(dest_kv, k);
                            kind_vars.insert(*dest, dest_kv);
                        }
                    }
                }
                IrInstr::Store { name, value } => {
                    let val = builder.use_var(value_vars[value]);
                    builder.def_var(named_vars[name], val);
                    let vk = kind_of(&value_kinds, *value);

                    let runtime_kind = if vk == ValueKind::Dynamic {
                        match kind_vars.get(value) {
                            Some(kv) => builder.use_var(*kv),
                            None => builder.ins().iconst(types::I64, 0),
                        }
                    } else {
                        builder.ins().iconst(types::I64, vk.as_i64())
                    };
                    if let Some(kv) = named_kind_vars.get(name) {
                        builder.def_var(*kv, runtime_kind);
                    }

                    let merged = match named_kinds.get(name) {
                        Some(prev) if *prev != vk => ValueKind::Dynamic,
                        _ => vk,
                    };
                    named_kinds.insert(name.clone(), merged);
                }
                IrInstr::Unary { dest, op, src } => {
                    let s = builder.use_var(value_vars[src]);
                    let src_kind = kind_of(&value_kinds, *src);
                    let (v, out_kind) = match op {
                        IrOp::Neg if src_kind == ValueKind::F64 => {
                            let f = i64_to_f64(&mut builder, s);
                            let n = builder.ins().fneg(f);
                            (f64_to_i64(&mut builder, n), ValueKind::F64)
                        }
                        IrOp::Neg => (builder.ins().ineg(s), ValueKind::I64),
                        IrOp::Not => {
                            let zero = builder.ins().iconst(types::I64, 0);
                            let ne = builder.ins().icmp(IntCC::NotEqual, s, zero);
                            let one = builder.ins().iconst(types::I8, 1);
                            let b = builder.ins().bxor(ne, one);
                            (builder.ins().uextend(types::I64, b), ValueKind::Bool)
                        }
                        other => {
                            return Err(format!("codegen: unsupported unary op {other}"));
                        }
                    };
                    builder.def_var(value_vars[dest], v);
                    value_kinds.insert(*dest, out_kind);
                }
                IrInstr::GuardDivisor { value, line } => {
                    let kind = kind_of(&value_kinds, *value);
                    if kind != ValueKind::F64 {
                        let v = builder.use_var(value_vars[value]);
                        let zero = builder.ins().iconst(types::I64, 0);
                        let mut is_zero = builder.ins().icmp(IntCC::Equal, v, zero);
                        if kind == ValueKind::Dynamic {
                            // A dynamic 0.0 has a zero payload but divides fine.
                            let vk = kind_operand(&mut builder, &kind_vars, kind, *value);
                            let f64_kind =
                                builder.ins().iconst(types::I64, ValueKind::F64.as_i64());
                            let not_float = builder.ins().icmp(IntCC::NotEqual, vk, f64_kind);
                            is_zero = builder.ins().band(is_zero, not_float);
                        }
                        let err_block = builder.create_block();
                        let ok_block = builder.create_block();
                        builder.ins().brif(is_zero, err_block, &[], ok_block, &[]);

                        builder.switch_to_block(err_block);
                        builder.seal_block(err_block);
                        let line_val = builder.ins().iconst(types::I64, *line as i64);
                        let fref = module
                            .declare_func_in_func(runtime.div_by_zero, &mut builder.func);
                        builder.ins().call(fref, &[line_val]);
                        builder.ins().jump(ok_block, &[]);

                        builder.switch_to_block(ok_block);
                        builder.seal_block(ok_block);
                    }
                }
                IrInstr::Binary {
                    dest,
                    op,
                    left,
                    right,
                } => {
                    let l = builder.use_var(value_vars[left]);
                    let r = builder.use_var(value_vars[right]);
                    let lk = kind_of(&value_kinds, *left);
                    let rk = kind_of(&value_kinds, *right);
                    let is_float = lk == ValueKind::F64 || rk == ValueKind::F64;

                    let (v, out_kind) = if lk == ValueKind::Str
                        && rk == ValueKind::Str
                        && matches!(op, IrOp::Add)
                    {
                        let fref = module
                            .declare_func_in_func(runtime.str_concat, &mut builder.func);
                        let call = builder.ins().call(fref, &[l, r]);
                        (builder.inst_results(call)[0], ValueKind::Str)
                    } else if matches!(op, IrOp::Eq | IrOp::Ne)
                        && (needs_runtime_eq(lk) || needs_runtime_eq(rk))
                    {
                        // Strings, containers and dynamic values compare by content.
                        let lkind = kind_operand(&mut builder, &kind_vars, lk, *left);
                        let rkind = kind_operand(&mut builder, &kind_vars, rk, *right);
                        let fref =
                            module.declare_func_in_func(runtime.value_eq, &mut builder.func);
                        let call = builder.ins().call(fref, &[l, lkind, r, rkind]);
                        let eq = builder.inst_results(call)[0];
                        let v = if matches!(op, IrOp::Ne) {
                            let one = builder.ins().iconst(types::I64, 1);
                            builder.ins().bxor(eq, one)
                        } else {
                            eq
                        };
                        (v, ValueKind::Bool)
                    } else if is_float
                        && matches!(
                            op,
                            IrOp::Add | IrOp::Sub | IrOp::Mul | IrOp::Div | IrOp::Pow
                        )
                    {
                        let lf = if lk == ValueKind::F64 {
                            i64_to_f64(&mut builder, l)
                        } else {
                            builder.ins().fcvt_from_sint(types::F64, l)
                        };
                        let rf = if rk == ValueKind::F64 {
                            i64_to_f64(&mut builder, r)
                        } else {
                            builder.ins().fcvt_from_sint(types::F64, r)
                        };
                        let fv = match op {
                            IrOp::Add => builder.ins().fadd(lf, rf),
                            IrOp::Sub => builder.ins().fsub(lf, rf),
                            IrOp::Mul => builder.ins().fmul(lf, rf),
                            IrOp::Div => builder.ins().fdiv(lf, rf),
                            IrOp::Pow => {
                                let fref =
                                    module.declare_func_in_func(runtime.pow_f64, &mut builder.func);
                                let call = builder.ins().call(fref, &[lf, rf]);
                                builder.inst_results(call)[0]
                            }
                            _ => unreachable!(),
                        };
                        (f64_to_i64(&mut builder, fv), ValueKind::F64)
                    } else {
                        let v = match op {
                            IrOp::Add => builder.ins().iadd(l, r),
                            IrOp::Sub => builder.ins().isub(l, r),
                            IrOp::Mul => builder.ins().imul(l, r),
                            IrOp::Div => builder.ins().sdiv(l, r),
                            IrOp::Rem => builder.ins().srem(l, r),
                            IrOp::Pow => {
                                let fref =
                                    module.declare_func_in_func(runtime.pow_i64, &mut builder.func);
                                let call = builder.ins().call(fref, &[l, r]);
                                builder.inst_results(call)[0]
                            }
                            IrOp::Eq => {
                                let b = if is_float {
                                    let lf = i64_to_f64(&mut builder, l);
                                    let rf = i64_to_f64(&mut builder, r);
                                    builder.ins().fcmp(FloatCC::Equal, lf, rf)
                                } else {
                                    let ne = builder.ins().icmp(IntCC::NotEqual, l, r);
                                    let one = builder.ins().iconst(types::I8, 1);
                                    builder.ins().bxor(ne, one)
                                };
                                builder.ins().uextend(types::I64, b)
                            }
                            IrOp::Ne => {
                                let b = if is_float {
                                    let lf = i64_to_f64(&mut builder, l);
                                    let rf = i64_to_f64(&mut builder, r);
                                    builder.ins().fcmp(FloatCC::NotEqual, lf, rf)
                                } else {
                                    builder.ins().icmp(IntCC::NotEqual, l, r)
                                };
                                builder.ins().uextend(types::I64, b)
                            }
                            IrOp::Lt => {
                                let b = if is_float {
                                    let lf = i64_to_f64(&mut builder, l);
                                    let rf = i64_to_f64(&mut builder, r);
                                    builder.ins().fcmp(FloatCC::LessThan, lf, rf)
                                } else {
                                    builder.ins().icmp(IntCC::SignedLessThan, l, r)
                                };
                                builder.ins().uextend(types::I64, b)
                            }
                            IrOp::Le => {
                                let b = if is_float {
                                    let lf = i64_to_f64(&mut builder, l);
                                    let rf = i64_to_f64(&mut builder, r);
                                    builder.ins().fcmp(FloatCC::LessThanOrEqual, lf, rf)
                                } else {
                                    builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r)
                                };
                                builder.ins().uextend(types::I64, b)
                            }
                            IrOp::Gt => {
                                let b = if is_float {
                                    let lf = i64_to_f64(&mut builder, l);
                                    let rf = i64_to_f64(&mut builder, r);
                                    builder.ins().fcmp(FloatCC::GreaterThan, lf, rf)
                                } else {
                                    builder.ins().icmp(IntCC::SignedGreaterThan, l, r)
                                };
                                builder.ins().uextend(types::I64, b)
                            }
                            IrOp::Ge => {
                                let b = if is_float {
                                    let lf = i64_to_f64(&mut builder, l);
                                    let rf = i64_to_f64(&mut builder, r);
                                    builder.ins().fcmp(FloatCC::GreaterThanOrEqual, lf, rf)
                                } else {
                                    builder
                                        .ins()
                                        .icmp(IntCC::SignedGreaterThanOrEqual, l, r)
                                };
                                builder.ins().uextend(types::I64, b)
                            }
                            IrOp::Neg | IrOp::Not => {
                                return Err(format!("codegen: {op} is unary, not binary"));
                            }
                        };
                        let out_kind = match op {
                            IrOp::Eq
                            | IrOp::Ne
                            | IrOp::Lt
                            | IrOp::Le
                            | IrOp::Gt
                            | IrOp::Ge => ValueKind::Bool,
                            _ => ValueKind::I64,
                        };
                        (v, out_kind)
                    };
                    builder.def_var(value_vars[dest], v);
                    value_kinds.insert(*dest, out_kind);
                }
                IrInstr::Call {
                    dest,
                    func,
                    args,
                } => {
                    let id = func_ids
                        .get(func)
                        .copied()
                        .ok_or_else(|| match func.as_str() {
                            "open" | "input" => crate::error::format_error(
                                crate::error::ErrorKind::Runtime,
                                0,
                                &format!(
                                    "'{func}' is only available on the interpreter path; run with 'run'"
                                ),
                            ),
                            _ => crate::error::format_error(
                                crate::error::ErrorKind::Runtime,
                                0,
                                &format!("undefined function '{func}'"),
                            ),
                        })?;
                    let mut arg_vals: Vec<Value> = Vec::with_capacity(args.len() * 2);
                    for a in args {
                        let v = builder.use_var(value_vars[a]);
                        let k = match kind_of(&value_kinds, *a) {
                            ValueKind::Dynamic => match kind_vars.get(a) {
                                Some(kv) => builder.use_var(*kv),
                                None => builder.ins().iconst(types::I64, 0),
                            },
                            other => builder.ins().iconst(types::I64, other.as_i64()),
                        };
                        arg_vals.push(v);
                        arg_vals.push(k);
                    }
                    let fref = module.declare_func_in_func(id, &mut builder.func);
                    let call = builder.ins().call(fref, &arg_vals);
                    let results = builder.inst_results(call).to_vec();
                    builder.def_var(value_vars[dest], results[0]);
                    if results.len() > 1 {
                        let kv = declare_var(&mut builder, &mut next_var);
                        builder.def_var(kv, results[1]);
                        kind_vars.insert(*dest, kv);
                        value_kinds.insert(*dest, ValueKind::Dynamic);
                    } else {
                        value_kinds.insert(*dest, ValueKind::I64);
                    }
                }
                IrInstr::MakeList { dest, items } => {
                    let fnew =
                        module.declare_func_in_func(runtime.list_new, &mut builder.func);
                    let call = builder.ins().call(fnew, &[]);
                    let list = builder.inst_results(call)[0];
                    builder.def_var(value_vars[dest], list);
                    value_kinds.insert(*dest, ValueKind::List);

                    let fpush =
                        module.declare_func_in_func(runtime.list_push, &mut builder.func);
                    for item in items {
                        let val = builder.use_var(value_vars[item]);
                        let kind = match kind_of(&value_kinds, *item) {
                            ValueKind::Dynamic => builder.use_var(kind_vars[item]),
                            other => builder.ins().iconst(types::I64, other.as_i64()),
                        };
                        builder.ins().call(fpush, &[list, val, kind]);
                    }
                }
                IrInstr::MakeDict { dest, entries } => {
                    let fnew =
                        module.declare_func_in_func(runtime.dict_new, &mut builder.func);
                    let call = builder.ins().call(fnew, &[]);
                    let dict = builder.inst_results(call)[0];
                    builder.def_var(value_vars[dest], dict);
                    value_kinds.insert(*dest, ValueKind::Dict);

                    let fpush =
                        module.declare_func_in_func(runtime.dict_push, &mut builder.func);
                    for (k, v) in entries {
                        let key = builder.use_var(value_vars[k]);
                        let key_kind = match kind_of(&value_kinds, *k) {
                            ValueKind::Dynamic => builder.use_var(kind_vars[k]),
                            other => builder.ins().iconst(types::I64, other.as_i64()),
                        };
                        let val = builder.use_var(value_vars[v]);
                        let val_kind = match kind_of(&value_kinds, *v) {
                            ValueKind::Dynamic => builder.use_var(kind_vars[v]),
                            other => builder.ins().iconst(types::I64, other.as_i64()),
                        };
                        builder
                            .ins()
                            .call(fpush, &[dict, key, key_kind, val, val_kind]);
                    }
                }
                IrInstr::IndexGet {
                    dest,
                    object,
                    index,
                } => {
                    let obj = builder.use_var(value_vars[object]);
                    let idx = builder.use_var(value_vars[index]);
                    let slot = builder.create_sized_stack_slot(StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        8,
                        0,
                    ));
                    let kind_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                    let payload = match kind_of(&value_kinds, *object) {
                        ValueKind::Dict => {
                            let key_kind = match kind_of(&value_kinds, *index) {
                                ValueKind::Dynamic => builder.use_var(kind_vars[index]),
                                other => builder.ins().iconst(types::I64, other.as_i64()),
                            };
                            let fref = module
                                .declare_func_in_func(runtime.dict_get, &mut builder.func);
                            let call =
                                builder.ins().call(fref, &[obj, idx, key_kind, kind_ptr]);
                            builder.inst_results(call)[0]
                        }
                        _ => {
                            let fref = module
                                .declare_func_in_func(runtime.list_get, &mut builder.func);
                            let call = builder.ins().call(fref, &[obj, idx, kind_ptr]);
                            builder.inst_results(call)[0]
                        }
                    };
                    builder.def_var(value_vars[dest], payload);
                    let kind_val = builder.ins().load(types::I64, MemFlags::new(), kind_ptr, 0);
                    let kv = declare_var(&mut builder, &mut next_var);
                    builder.def_var(kv, kind_val);
                    kind_vars.insert(*dest, kv);
                    value_kinds.insert(*dest, ValueKind::Dynamic);
                }
                IrInstr::IndexSet {
                    object,
                    index,
                    value,
                } => {
                    let obj = builder.use_var(value_vars[object]);
                    let idx = builder.use_var(value_vars[index]);
                    let val = builder.use_var(value_vars[value]);
                    let val_kind = match kind_of(&value_kinds, *value) {
                        ValueKind::Dynamic => builder.use_var(kind_vars[value]),
                        other => builder.ins().iconst(types::I64, other.as_i64()),
                    };
                    match kind_of(&value_kinds, *object) {
                        ValueKind::Dict => {
                            let key_kind = match kind_of(&value_kinds, *index) {
                                ValueKind::Dynamic => builder.use_var(kind_vars[index]),
                                other => builder.ins().iconst(types::I64, other.as_i64()),
                            };
                            let fref = module
                                .declare_func_in_func(runtime.dict_set, &mut builder.func);
                            builder
                                .ins()
                                .call(fref, &[obj, idx, key_kind, val, val_kind]);
                        }
                        _ => {
                            let fref = module
                                .declare_func_in_func(runtime.list_set, &mut builder.func);
                            builder.ins().call(fref, &[obj, idx, val, val_kind]);
                        }
                    }
                }
                IrInstr::ListLen { dest, list } => {
                    let l = builder.use_var(value_vars[list]);
                    let fref =
                        module.declare_func_in_func(runtime.list_len, &mut builder.func);
                    let call = builder.ins().call(fref, &[l]);
                    let ret = builder.inst_results(call)[0];
                    builder.def_var(value_vars[dest], ret);
                    value_kinds.insert(*dest, ValueKind::I64);
                }
                IrInstr::ValueToStr { dest, src } => {
                    let v = builder.use_var(value_vars[src]);
                    let kind = match kind_of(&value_kinds, *src) {
                        ValueKind::Dynamic => builder.use_var(kind_vars[src]),
                        other => builder.ins().iconst(types::I64, other.as_i64()),
                    };
                    let fref =
                        module.declare_func_in_func(runtime.value_to_str, &mut builder.func);
                    let call = builder.ins().call(fref, &[v, kind]);
                    let ret = builder.inst_results(call)[0];
                    builder.def_var(value_vars[dest], ret);
                    value_kinds.insert(*dest, ValueKind::Str);
                }
                IrInstr::StrConcat { dest, left, right } => {
                    let l = builder.use_var(value_vars[left]);
                    let r = builder.use_var(value_vars[right]);
                    let fref =
                        module.declare_func_in_func(runtime.str_concat, &mut builder.func);
                    let call = builder.ins().call(fref, &[l, r]);
                    let ret = builder.inst_results(call)[0];
                    builder.def_var(value_vars[dest], ret);
                    value_kinds.insert(*dest, ValueKind::Str);
                }
                IrInstr::MakeStruct { dest, nfields } => {
                    let n = builder.ins().iconst(types::I64, *nfields as i64);
                    let fref =
                        module.declare_func_in_func(runtime.struct_new, &mut builder.func);
                    let call = builder.ins().call(fref, &[n]);
                    let ret = builder.inst_results(call)[0];
                    builder.def_var(value_vars[dest], ret);
                    value_kinds.insert(*dest, ValueKind::Struct);
                }
                IrInstr::StructGet {
                    dest,
                    object,
                    field,
                } => {
                    let obj = builder.use_var(value_vars[object]);
                    let idx = builder.ins().iconst(types::I64, *field as i64);
                    let slot = builder.create_sized_stack_slot(StackSlotData::new(
                        StackSlotKind::ExplicitSlot,
                        8,
                        0,
                    ));
                    let kind_ptr = builder.ins().stack_addr(types::I64, slot, 0);
                    let fref =
                        module.declare_func_in_func(runtime.struct_get, &mut builder.func);
                    let call = builder.ins().call(fref, &[obj, idx, kind_ptr]);
                    let payload = builder.inst_results(call)[0];
                    builder.def_var(value_vars[dest], payload);
                    let kind_val = builder.ins().load(types::I64, MemFlags::new(), kind_ptr, 0);
                    let kv = declare_var(&mut builder, &mut next_var);
                    builder.def_var(kv, kind_val);
                    kind_vars.insert(*dest, kv);
                    value_kinds.insert(*dest, ValueKind::Dynamic);
                }
                IrInstr::StructSet {
                    object,
                    field,
                    value,
                } => {
                    let obj = builder.use_var(value_vars[object]);
                    let idx = builder.ins().iconst(types::I64, *field as i64);
                    let val = builder.use_var(value_vars[value]);
                    let val_kind = match kind_of(&value_kinds, *value) {
                        ValueKind::Dynamic => builder.use_var(kind_vars[value]),
                        other => builder.ins().iconst(types::I64, other.as_i64()),
                    };
                    let fref =
                        module.declare_func_in_func(runtime.struct_set, &mut builder.func);
                    builder.ins().call(fref, &[obj, idx, val, val_kind]);
                }
                IrInstr::Print { args } => {
                    for (i, a) in args.iter().enumerate() {
                        if i > 0 {
                            let sep_ref = module
                                .declare_func_in_func(runtime.print_sep, &mut builder.func);
                            builder.ins().call(sep_ref, &[]);
                        }
                        let v = builder.use_var(value_vars[a]);
                        match kind_of(&value_kinds, *a) {
                            ValueKind::Dynamic => {
                                let k = builder.use_var(kind_vars[a]);
                                let fref = module.declare_func_in_func(
                                    runtime.print_value,
                                    &mut builder.func,
                                );
                                builder.ins().call(fref, &[v, k]);
                            }
                            ValueKind::F64 => {
                                let f = i64_to_f64(&mut builder, v);
                                let fref = module
                                    .declare_func_in_func(runtime.print_f64, &mut builder.func);
                                builder.ins().call(fref, &[f]);
                            }
                            ValueKind::Str => {
                                let fref = module
                                    .declare_func_in_func(runtime.print_str, &mut builder.func);
                                builder.ins().call(fref, &[v]);
                            }
                            ValueKind::List => {
                                let fref = module
                                    .declare_func_in_func(runtime.print_list, &mut builder.func);
                                builder.ins().call(fref, &[v]);
                            }
                            ValueKind::Dict => {
                                let fref = module
                                    .declare_func_in_func(runtime.print_dict, &mut builder.func);
                                builder.ins().call(fref, &[v]);
                            }
                            ValueKind::Struct => {
                                let fref = module
                                    .declare_func_in_func(runtime.print_struct, &mut builder.func);
                                builder.ins().call(fref, &[v]);
                            }
                            kind @ (ValueKind::Bool | ValueKind::None_) => {
                                let k =
                                    builder.ins().iconst(types::I64, kind.as_i64());
                                let fref = module.declare_func_in_func(
                                    runtime.print_value,
                                    &mut builder.func,
                                );
                                builder.ins().call(fref, &[v, k]);
                            }
                            ValueKind::I64 => {
                                let fref = module
                                    .declare_func_in_func(runtime.print_i64, &mut builder.func);
                                builder.ins().call(fref, &[v]);
                            }
                        }
                    }
                    let nl_ref =
                        module.declare_func_in_func(runtime.print_nl, &mut builder.func);
                    builder.ins().call(nl_ref, &[]);
                }
                IrInstr::Return { value } => {
                    let v = match value {
                        Some(id) => builder.use_var(value_vars[id]),
                        None => builder.ins().iconst(types::I64, 0),
                    };
                    if returns_kind {
                        let k = match value {
                            Some(id) => match kind_of(&value_kinds, *id) {
                                ValueKind::Dynamic => match kind_vars.get(id) {
                                    Some(kv) => builder.use_var(*kv),
                                    None => builder.ins().iconst(types::I64, 0),
                                },
                                other => {
                                    builder.ins().iconst(types::I64, other.as_i64())
                                }
                            },
                            None => builder
                                .ins()
                                .iconst(types::I64, ValueKind::None_.as_i64()),
                        };
                        builder.ins().return_(&[v, k]);
                    } else {
                        builder.ins().return_(&[v]);
                    }
                    terminated = true;
                }
                IrInstr::Jump { target } => {
                    let b = blocks[target];
                    builder.ins().jump(b, &[]);
                    terminated = true;
                }
                IrInstr::Branch {
                    cond,
                    then_block,
                    else_block,
                } => {
                    let c = builder.use_var(value_vars[cond]);
                    let t = blocks[then_block];
                    let e = blocks[else_block];
                    builder.ins().brif(c, t, &[], e, &[]);
                    terminated = true;
                }
                IrInstr::ParallelForBegin { .. } | IrInstr::ParallelForEnd => {}
            }
        }

        if !terminated {
            let zero = builder.ins().iconst(types::I64, 0);
            if returns_kind {
                // Falling off the end yields None, matching the interpreter.
                let none = builder
                    .ins()
                    .iconst(types::I64, ValueKind::None_.as_i64());
                builder.ins().return_(&[zero, none]);
            } else {
                builder.ins().return_(&[zero]);
            }
        }

        builder.seal_all_blocks();
        builder.finalize();
    }

    module
        .define_function(func_id, ctx)
        .map_err(|e| format!("define function: {e}"))?;
    module.clear_context(ctx);
    Ok(())
}
