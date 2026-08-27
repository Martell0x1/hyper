use crate::ir::{BlockId, IrInstr, IrModule, IrOp, ValueId};
use cranelift_codegen::entity::EntityRef;
use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::immediates::Ieee64;
use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder, MemFlags, UserFuncName, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};
use cranelift_object::{ObjectBuilder, ObjectModule};
use std::collections::{HashMap, HashSet};
use std::ffi::{CStr, CString};
use std::mem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    I64,
    F64,
    Str,
    Bool,
    None_,
}

struct StringPool {
    strings: Vec<CString>,
}

impl StringPool {
    fn new() -> Self {
        StringPool {
            strings: Vec::new(),
        }
    }

    fn intern(&mut self, s: &str) -> i64 {
        let c = CString::new(s.replace('\0', "")).unwrap_or_else(|_| CString::new("").unwrap());
        let ptr = c.as_ptr() as i64;
        self.strings.push(c);
        ptr
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_i64(v: i64) {
    print!("{} ", v);
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_f64(v: f64) {
    print!("{} ", v);
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_str(s: *const i8) {
    if s.is_null() {
        print!(" ");
        return;
    }
    let cstr = unsafe { CStr::from_ptr(s) };
    match cstr.to_str() {
        Ok(text) => print!("{} ", text),
        Err(_) => print!("<?> "),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_newline() {
    println!();
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_pow_i64(base: i64, exp: i64) -> i64 {
    if exp < 0 {
        return 0;
    }
    let mut result: i64 = 1;
    let mut b = base;
    let mut e = exp as u64;
    while e > 0 {
        if e & 1 == 1 {
            result = result.wrapping_mul(b);
        }
        b = b.wrapping_mul(b);
        e >>= 1;
    }
    result
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_pow_f64(base: f64, exp: f64) -> f64 {
    base.powf(exp)
}

struct RuntimeIds {
    print_i64: FuncId,
    print_f64: FuncId,
    print_str: FuncId,
    print_nl: FuncId,
    pow_i64: FuncId,
    pow_f64: FuncId,
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
    Ok(RuntimeIds {
        print_i64,
        print_f64,
        print_str,
        print_nl,
        pow_i64,
        pow_f64,
    })
}

fn declare_user_funcs<M: Module>(
    module: &mut M,
    ir: &IrModule,
) -> Result<HashMap<String, FuncId>, String> {
    let mut func_ids: HashMap<String, FuncId> = HashMap::new();
    for func in &ir.functions {
        let mut sig = module.make_signature();
        for _ in &func.params {
            sig.params.push(AbiParam::new(types::I64));
        }
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

fn named_kind(map: &HashMap<String, ValueKind>, name: &str) -> ValueKind {
    map.get(name).copied().unwrap_or(ValueKind::I64)
}

fn i64_to_f64(builder: &mut FunctionBuilder, v: Value) -> Value {
    builder
        .ins()
        .bitcast(types::F64, MemFlags::new(), v)
}

fn f64_to_i64(builder: &mut FunctionBuilder, v: Value) -> Value {
    builder
        .ins()
        .bitcast(types::I64, MemFlags::new(), v)
}

pub fn dump_ir(module: &IrModule) {
    println!("{}", module);
}

pub fn jit_execute(module: &IrModule) -> Result<(), String> {
    let flags = make_flags(false)?;
    let isa_builder =
        cranelift_native::builder().map_err(|msg| format!("host unsupported: {msg}"))?;
    let isa = isa_builder
        .finish(flags)
        .map_err(|e| e.to_string())?;

    let mut jit_builder = JITBuilder::with_isa(isa, default_libcall_names());
    jit_builder.symbol("hyper_rt_print_i64", hyper_rt_print_i64 as *const u8);
    jit_builder.symbol("hyper_rt_print_f64", hyper_rt_print_f64 as *const u8);
    jit_builder.symbol("hyper_rt_print_str", hyper_rt_print_str as *const u8);
    jit_builder.symbol(
        "hyper_rt_print_newline",
        hyper_rt_print_newline as *const u8,
    );
    jit_builder.symbol("hyper_rt_pow_i64", hyper_rt_pow_i64 as *const u8);
    jit_builder.symbol("hyper_rt_pow_f64", hyper_rt_pow_f64 as *const u8);

    let mut jit = JITModule::new(jit_builder);
    let mut ctx = jit.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut strings = StringPool::new();

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
    )?;

    jit.finalize_definitions().map_err(|e| e.to_string())?;

    let code = jit.get_finalized_function(main_id);
    let main_fn: extern "C" fn() -> i64 = unsafe { mem::transmute(code) };
    let _ = main_fn();
    // Keep interned strings alive for the duration of JIT execution.
    drop(strings);
    Ok(())
}

pub fn emit_object(module: &IrModule, out_path: &str) -> Result<(), String> {
    let flags = make_flags(true)?;
    let isa_builder =
        cranelift_native::builder().map_err(|msg| format!("host unsupported: {msg}"))?;
    let isa = isa_builder
        .finish(flags)
        .map_err(|e| e.to_string())?;

    let builder = ObjectBuilder::new(isa, "hyper", default_libcall_names())
        .map_err(|e| e.to_string())?;
    let mut obj = ObjectModule::new(builder);
    let mut ctx = obj.make_context();
    let mut func_ctx = FunctionBuilderContext::new();
    let mut strings = StringPool::new();

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
    )?;

    let product = obj.finish();
    let bytes = product.emit().map_err(|e| e.to_string())?;
    std::fs::write(out_path, &bytes).map_err(|e| e.to_string())?;
    // String pool must outlive emit if ConstStr baked host pointers (MVP limitation).
    drop(strings);
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
    strings: &mut StringPool,
) -> Result<(), String> {
    let mut sig = module.make_signature();
    for _ in params {
        sig.params.push(AbiParam::new(types::I64));
    }
    sig.returns.push(AbiParam::new(types::I64));

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
            builder.def_var(var, param_vals[i]);
            named_vars.insert(name.clone(), var);
            named_defs.insert(name.clone());
            named_kinds.insert(name.clone(), ValueKind::I64);
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
                    let ptr = strings.intern(value);
                    let v = builder.ins().iconst(types::I64, ptr);
                    builder.def_var(value_vars[dest], v);
                    value_kinds.insert(*dest, ValueKind::Str);
                }
                IrInstr::Load { dest, name } => {
                    let val = builder.use_var(named_vars[name]);
                    builder.def_var(value_vars[dest], val);
                    value_kinds.insert(*dest, named_kind(&named_kinds, name));
                }
                IrInstr::Store { name, value } => {
                    let val = builder.use_var(value_vars[value]);
                    builder.def_var(named_vars[name], val);
                    named_kinds.insert(name.clone(), kind_of(&value_kinds, *value));
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

                    let (v, out_kind) = if is_float
                        && matches!(
                            op,
                            IrOp::Add | IrOp::Sub | IrOp::Mul | IrOp::Div | IrOp::Pow
                        ) {
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
                            IrOp::And => {
                                let zero = builder.ins().iconst(types::I64, 0);
                                let lb = builder.ins().icmp(IntCC::NotEqual, l, zero);
                                let rb = builder.ins().icmp(IntCC::NotEqual, r, zero);
                                let b = builder.ins().band(lb, rb);
                                builder.ins().uextend(types::I64, b)
                            }
                            IrOp::Or => {
                                let zero = builder.ins().iconst(types::I64, 0);
                                let lb = builder.ins().icmp(IntCC::NotEqual, l, zero);
                                let rb = builder.ins().icmp(IntCC::NotEqual, r, zero);
                                let b = builder.ins().bor(lb, rb);
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
                            | IrOp::Ge
                            | IrOp::And
                            | IrOp::Or => ValueKind::Bool,
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
                        .ok_or_else(|| format!("codegen: unknown function '{func}'"))?;
                    let arg_vals: Vec<Value> = args
                        .iter()
                        .map(|a| builder.use_var(value_vars[a]))
                        .collect();
                    let fref = module.declare_func_in_func(id, &mut builder.func);
                    let call = builder.ins().call(fref, &arg_vals);
                    let ret = builder.inst_results(call)[0];
                    builder.def_var(value_vars[dest], ret);
                    value_kinds.insert(*dest, ValueKind::I64);
                }
                IrInstr::Print { args } => {
                    for a in args {
                        let v = builder.use_var(value_vars[a]);
                        match kind_of(&value_kinds, *a) {
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
                            ValueKind::I64 | ValueKind::Bool | ValueKind::None_ => {
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
                    builder.ins().return_(&[v]);
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
                IrInstr::ParallelForBegin { .. } | IrInstr::ParallelForEnd => {
                    // No-op: parallel loops are lowered sequentially in the compiler.
                }
            }
        }

        if !terminated {
            let zero = builder.ins().iconst(types::I64, 0);
            builder.ins().return_(&[zero]);
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
