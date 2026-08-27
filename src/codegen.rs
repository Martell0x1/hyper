use crate::ir::{BlockId, IrInstr, IrModule, IrOp, ValueId};
use cranelift_codegen::entity::EntityRef;
use cranelift_codegen::ir::condcodes::IntCC;
use cranelift_codegen::ir::{types, AbiParam, Function, InstBuilder, UserFuncName, Value};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext, Variable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};
use std::collections::{HashMap, HashSet};
use std::mem;

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_i64(v: i64) {
    print!("{} ", v);
}

#[unsafe(no_mangle)]
pub extern "C" fn hyper_rt_print_f64(v: f64) {
    print!("{} ", v);
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

pub fn jit_execute(module: &IrModule) -> Result<(), String> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| e.to_string())?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| e.to_string())?;
    let isa_builder =
        cranelift_native::builder().map_err(|msg| format!("host unsupported: {msg}"))?;
    let isa = isa_builder
        .finish(settings::Flags::new(flag_builder))
        .map_err(|e| e.to_string())?;

    let mut jit_builder = JITBuilder::with_isa(isa, default_libcall_names());
    jit_builder.symbol("hyper_rt_print_i64", hyper_rt_print_i64 as *const u8);
    jit_builder.symbol("hyper_rt_print_f64", hyper_rt_print_f64 as *const u8);
    jit_builder.symbol(
        "hyper_rt_print_newline",
        hyper_rt_print_newline as *const u8,
    );
    jit_builder.symbol("hyper_rt_pow_i64", hyper_rt_pow_i64 as *const u8);

    let mut jit = JITModule::new(jit_builder);
    let mut ctx = jit.make_context();
    let mut func_ctx = FunctionBuilderContext::new();

    let print_i64_id = {
        let mut sig = jit.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        jit.declare_function("hyper_rt_print_i64", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let print_nl_id = {
        let sig = jit.make_signature();
        jit.declare_function("hyper_rt_print_newline", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };
    let pow_id = {
        let mut sig = jit.make_signature();
        sig.params.push(AbiParam::new(types::I64));
        sig.params.push(AbiParam::new(types::I64));
        sig.returns.push(AbiParam::new(types::I64));
        jit.declare_function("hyper_rt_pow_i64", Linkage::Import, &sig)
            .map_err(|e| e.to_string())?
    };

    let mut func_ids: HashMap<String, FuncId> = HashMap::new();
    for func in &module.functions {
        let mut sig = jit.make_signature();
        for _ in &func.params {
            sig.params.push(AbiParam::new(types::I64));
        }
        sig.returns.push(AbiParam::new(types::I64));
        let id = jit
            .declare_function(&func.name, Linkage::Local, &sig)
            .map_err(|e| e.to_string())?;
        func_ids.insert(func.name.clone(), id);
    }

    let main_id = {
        let mut sig = jit.make_signature();
        sig.returns.push(AbiParam::new(types::I64));
        jit.declare_function("__main__", Linkage::Export, &sig)
            .map_err(|e| e.to_string())?
    };
    func_ids.insert("__main__".to_string(), main_id);

    let runtime = RuntimeIds {
        print_i64: print_i64_id,
        print_nl: print_nl_id,
        pow: pow_id,
    };

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
    )?;

    jit.finalize_definitions().map_err(|e| e.to_string())?;

    let code = jit.get_finalized_function(main_id);
    let main_fn: extern "C" fn() -> i64 = unsafe { mem::transmute(code) };
    let _ = main_fn();
    Ok(())
}

struct RuntimeIds {
    print_i64: FuncId,
    print_nl: FuncId,
    pow: FuncId,
}

fn define_function(
    jit: &mut JITModule,
    ctx: &mut cranelift_codegen::Context,
    func_ctx: &mut FunctionBuilderContext,
    func_id: FuncId,
    params: &[String],
    body: &[IrInstr],
    func_ids: &HashMap<String, FuncId>,
    runtime: &RuntimeIds,
) -> Result<(), String> {
    let mut sig = jit.make_signature();
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

        // Initialize unnamed locals to 0 in the entry block so use_var is valid.
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
                _ if terminated => {
                    // Dead code until next label.
                }
                IrInstr::ConstI64 { dest, value } => {
                    let v = builder.ins().iconst(types::I64, *value);
                    builder.def_var(value_vars[dest], v);
                }
                IrInstr::ConstF64 { dest, value } => {
                    let v = builder.ins().iconst(types::I64, *value as i64);
                    builder.def_var(value_vars[dest], v);
                }
                IrInstr::ConstBool { dest, value } => {
                    let v = builder
                        .ins()
                        .iconst(types::I64, if *value { 1 } else { 0 });
                    builder.def_var(value_vars[dest], v);
                }
                IrInstr::ConstNone { dest } => {
                    let v = builder.ins().iconst(types::I64, 0);
                    builder.def_var(value_vars[dest], v);
                }
                IrInstr::ConstStr { .. } => {
                    return Err("codegen: ConstStr not supported in JIT MVP".into());
                }
                IrInstr::Load { dest, name } => {
                    let val = builder.use_var(named_vars[name]);
                    builder.def_var(value_vars[dest], val);
                }
                IrInstr::Store { name, value } => {
                    let val = builder.use_var(value_vars[value]);
                    builder.def_var(named_vars[name], val);
                }
                IrInstr::Unary { dest, op, src } => {
                    let s = builder.use_var(value_vars[src]);
                    let v = match op {
                        IrOp::Neg => builder.ins().ineg(s),
                        IrOp::Not => {
                            let zero = builder.ins().iconst(types::I64, 0);
                            let ne = builder.ins().icmp(IntCC::NotEqual, s, zero);
                            let one = builder.ins().iconst(types::I8, 1);
                            let b = builder.ins().bxor(ne, one);
                            builder.ins().uextend(types::I64, b)
                        }
                        other => {
                            return Err(format!("codegen: unsupported unary op {other}"));
                        }
                    };
                    builder.def_var(value_vars[dest], v);
                }
                IrInstr::Binary {
                    dest,
                    op,
                    left,
                    right,
                } => {
                    let l = builder.use_var(value_vars[left]);
                    let r = builder.use_var(value_vars[right]);
                    let v = match op {
                        IrOp::Add => builder.ins().iadd(l, r),
                        IrOp::Sub => builder.ins().isub(l, r),
                        IrOp::Mul => builder.ins().imul(l, r),
                        IrOp::Div => builder.ins().sdiv(l, r),
                        IrOp::Rem => builder.ins().srem(l, r),
                        IrOp::Pow => {
                            let fref = jit.declare_func_in_func(runtime.pow, &mut builder.func);
                            let call = builder.ins().call(fref, &[l, r]);
                            builder.inst_results(call)[0]
                        }
                        IrOp::Eq => {
                            let ne = builder.ins().icmp(IntCC::NotEqual, l, r);
                            let one = builder.ins().iconst(types::I8, 1);
                            let b = builder.ins().bxor(ne, one);
                            builder.ins().uextend(types::I64, b)
                        }
                        IrOp::Ne => {
                            let b = builder.ins().icmp(IntCC::NotEqual, l, r);
                            builder.ins().uextend(types::I64, b)
                        }
                        IrOp::Lt => {
                            let b = builder.ins().icmp(IntCC::SignedLessThan, l, r);
                            builder.ins().uextend(types::I64, b)
                        }
                        IrOp::Le => {
                            let b = builder
                                .ins()
                                .icmp(IntCC::SignedLessThanOrEqual, l, r);
                            builder.ins().uextend(types::I64, b)
                        }
                        IrOp::Gt => {
                            let b = builder.ins().icmp(IntCC::SignedGreaterThan, l, r);
                            builder.ins().uextend(types::I64, b)
                        }
                        IrOp::Ge => {
                            let b = builder
                                .ins()
                                .icmp(IntCC::SignedGreaterThanOrEqual, l, r);
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
                    builder.def_var(value_vars[dest], v);
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
                    let fref = jit.declare_func_in_func(id, &mut builder.func);
                    let call = builder.ins().call(fref, &arg_vals);
                    let ret = builder.inst_results(call)[0];
                    builder.def_var(value_vars[dest], ret);
                }
                IrInstr::Print { args } => {
                    let print_ref =
                        jit.declare_func_in_func(runtime.print_i64, &mut builder.func);
                    for a in args {
                        let v = builder.use_var(value_vars[a]);
                        builder.ins().call(print_ref, &[v]);
                    }
                    let nl_ref = jit.declare_func_in_func(runtime.print_nl, &mut builder.func);
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
                    return Err("codegen: ParallelFor not supported in JIT MVP".into());
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

    jit.define_function(func_id, ctx)
        .map_err(|e| format!("define function: {e}"))?;
    jit.clear_context(ctx);
    Ok(())
}
