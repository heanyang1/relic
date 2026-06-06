//! The LLVM compiler module.
//!
//! This module provides compilation of AST nodes into LLVM IR using the
//! [`inkwell`](https://crates.io/crates/inkwell) crate. The compiler transforms
//! Lisp expressions into LLVM IR that calls the Relic runtime C API.
//!
//! ## Compilation Pipeline
//!
//! 1. **AST to LLVM IR**: The [`CompileLlvm`] trait transforms AST nodes into LLVM IR
//! 2. **LLVM IR to .ll file**: The IR is serialized to a text file
//! 3. **Clang compilation**: The .ll file is compiled with Clang into a `.relic` shared library
//! 4. **Dynamic Loading**: The shared library is loaded via [`libloading`](crate::package)
//!
//! ## Code Generation
//!
//! The [`LlvmCodeGen`] struct manages code generation with three modes:
//! - `new_main`: Generates a `main()` function calling `rt_start()`
//! - `new_library`: Generates a named function for packages (no `rt_start()`)
//!
//! Each closure compiles to a separate LLVM function. The main/library function
//! calls these closure functions as needed.
//!
//! ## Runtime API Generation
//!
//! The compiler generates calls to the Relic C runtime API, declared as LLVM
//! `declare` functions:
//! - `rt_new_integer(n)`, `rt_new_float(n)`: Create numbers
//! - `rt_new_symbol("name")`: Create symbols
//! - `rt_push(idx)`, `rt_pop()`: Stack operations
//! - `rt_define("name", value)`: Define variables
//! - `rt_get("name")`: Get variable values
//! - `rt_new_closure(name, func, nargs, variadic)`: Create closures
//! - `rt_apply()`: Apply operators
//! - `rt_get_c_func(closure)`, `rt_prepare_args(closure)`: Closure dispatch
//!
//! ## Special Forms
//!
//! Special forms are handled specially during compilation, mirroring the C backend:
//! - [`SpecialForm::Lambda`]: Creates closure LLVM functions inline
//! - [`SpecialForm::Define`]: Defines variables in the environment
//! - [`SpecialForm::Set`], [`SpecialForm::SetCar`], [`SpecialForm::SetCdr`]: Assignment
//! - [`SpecialForm::If`]: Generates LLVM conditional branches
//! - [`SpecialForm::Begin`]: Sequence compilation
//! - [`SpecialForm::Quote`]: Constant generation
//! - [`SpecialForm::Apply`]: Pushes list elements to stack and calls procedure
//!
//! ## Key Differences from C Backend
//!
//! - Uses `Builder::build_indirect_call` for closure dispatch (LLVM opaque pointers)
//! - String constants are stored as global arrays with null terminators
//! - Control flow uses LLVM basic blocks instead of C `if`/`else`
//! - The `current_fn` field must be swapped when compiling closure bodies to ensure
//!   new basic blocks are appended to the correct function
//! - After `rt_prepare_args` may trigger GC, so `rt_remove_root` must re-fetch
//!   the closure index (unlike SSA registers which may become stale)

use std::{cell::Cell, collections::HashMap};

use inkwell::{
    AddressSpace, IntPredicate, OptimizationLevel,
    builder::Builder,
    context::Context,
    execution_engine::ExecutionEngine,
    module::Module,
    values::{FunctionValue, IntValue, PointerValue},
};

use crate::{
    lexer::LexerMonad,
    node::Node,
    number::Number,
    symbol::{SpecialForm, Symbol},
    util::{Vectorize, get_n_params, inc},
};

#[derive(Clone, Copy)]
pub struct ContexInfo {
    pub drop_env: bool,
    pub drop_ret: bool,
}

pub struct LlvmCodeGen<'ctx> {
    context: &'ctx Context,
    module: Module<'ctx>,
    builder: Builder<'ctx>,
    current_fn: Option<FunctionValue<'ctx>>,
    strings: HashMap<String, PointerValue<'ctx>>,
    unique_counter: Cell<usize>,
}

impl<'ctx> LlvmCodeGen<'ctx> {
    pub fn new_main(context: &'ctx Context) -> Self {
        let module = context.create_module("relic_main");
        let builder = context.create_builder();

        let mut codegen = LlvmCodeGen {
            context,
            module,
            builder,
            current_fn: None,
            strings: HashMap::new(),
            unique_counter: Cell::new(0),
        };

        codegen.create_main_or_library_fn("main", true);
        codegen
    }

    pub fn new_library(context: &'ctx Context, name: String) -> Self {
        let module = context.create_module(&name);
        let builder = context.create_builder();

        let mut codegen = LlvmCodeGen {
            context,
            module,
            builder,
            current_fn: None,
            strings: HashMap::new(),
            unique_counter: Cell::new(0),
        };

        codegen.create_main_or_library_fn(&name, false);
        codegen
    }

    fn next_id(&self) -> usize {
        let id = self.unique_counter.get();
        self.unique_counter.set(id + 1);
        id
    }

    fn void_type(&self) -> inkwell::types::VoidType<'ctx> {
        self.context.void_type()
    }
    fn i32_type(&self) -> inkwell::types::IntType<'ctx> {
        self.context.i32_type()
    }
    fn i64_type(&self) -> inkwell::types::IntType<'ctx> {
        self.context.i64_type()
    }
    fn f64_type(&self) -> inkwell::types::FloatType<'ctx> {
        self.context.f64_type()
    }
    fn ptr_type(&self) -> inkwell::types::PointerType<'ctx> {
        self.context.ptr_type(AddressSpace::default())
    }

    fn create_main_or_library_fn(&mut self, name: &str, with_start: bool) {
        let fn_type = self.i32_type().fn_type(&[], false);
        let fn_val = self.module.add_function(name, fn_type, None);
        let entry = self.context.append_basic_block(fn_val, "entry");
        self.builder.position_at_end(entry);

        if with_start {
            let rt_start = self.declare_rt_fn("rt_start", self.void_type().fn_type(&[], false));
            self.builder.build_call(rt_start, &[], "").unwrap();
        }

        self.current_fn = Some(fn_val);
    }

    fn declare_rt_fn(
        &self,
        name: &str,
        fn_type: inkwell::types::FunctionType<'ctx>,
    ) -> FunctionValue<'ctx> {
        if let Some(func) = self.module.get_function(name) {
            return func;
        }
        self.module.add_function(name, fn_type, None)
    }

    fn get_rt_fn(&self, name: &str) -> FunctionValue<'ctx> {
        let fn_type = match name {
            "rt_start" => self.void_type().fn_type(&[], false),
            "rt_new_integer" => self.void_type().fn_type(&[self.i64_type().into()], false),
            "rt_new_float" => self.void_type().fn_type(&[self.f64_type().into()], false),
            "rt_new_symbol" => self.void_type().fn_type(&[self.ptr_type().into()], false),
            "rt_new_closure" => self.void_type().fn_type(
                &[
                    self.ptr_type().into(),
                    self.ptr_type().into(),
                    self.i64_type().into(),
                    self.i32_type().into(),
                ],
                false,
            ),
            "rt_push" => self.void_type().fn_type(&[self.i64_type().into()], false),
            "rt_pop" => self.i64_type().fn_type(&[], false),
            "rt_swap" => self.void_type().fn_type(&[], false),
            "rt_top" => self.i64_type().fn_type(&[], false),
            "rt_apply" => self.i64_type().fn_type(&[], false),
            "rt_define" => self
                .void_type()
                .fn_type(&[self.ptr_type().into(), self.i64_type().into()], false),
            "rt_set" => self
                .void_type()
                .fn_type(&[self.ptr_type().into(), self.i64_type().into()], false),
            "rt_get" => self.i64_type().fn_type(&[self.ptr_type().into()], false),
            "rt_set_car" => self
                .i64_type()
                .fn_type(&[self.i64_type().into(), self.i64_type().into()], false),
            "rt_set_cdr" => self
                .i64_type()
                .fn_type(&[self.i64_type().into(), self.i64_type().into()], false),
            "rt_current_env" => self.i64_type().fn_type(&[], false),
            "rt_move_to_env" => self.void_type().fn_type(&[self.i64_type().into()], false),
            "rt_add_root" => self
                .i64_type()
                .fn_type(&[self.ptr_type().into(), self.i64_type().into()], false),
            "rt_get_root" => self.i64_type().fn_type(&[self.ptr_type().into()], false),
            "rt_remove_root" => self.i64_type().fn_type(&[self.ptr_type().into()], false),
            "rt_prepare_args" => self.void_type().fn_type(&[self.i64_type().into()], false),
            "rt_get_c_func" => self.ptr_type().fn_type(&[self.i64_type().into()], false),
            "rt_is_symbol" => self.i32_type().fn_type(&[self.i64_type().into()], false),
            "rt_get_bool" => self.i32_type().fn_type(&[self.i64_type().into()], false),
            "rt_list_to_stack" => self.void_type().fn_type(&[], false),
            "rt_new_constant" => self.void_type().fn_type(&[self.ptr_type().into()], false),
            "rt_display_node_idx" => self.ptr_type().fn_type(&[self.i64_type().into()], false),
            "rt_evaluated" => self
                .void_type()
                .fn_type(&[self.ptr_type().into(), self.i32_type().into()], false),
            "rt_breakpoint" => self.void_type().fn_type(&[], false),
            "rt_import" => self.void_type().fn_type(&[self.ptr_type().into()], false),
            "rt_read" => self.void_type().fn_type(&[], false),
            "printf" => self.void_type().fn_type(&[self.ptr_type().into()], true),
            "fflush" => self.void_type().fn_type(&[self.ptr_type().into()], false),
            _ => panic!("Unknown runtime function: {name}"),
        };
        self.declare_rt_fn(name, fn_type)
    }

    fn get_string_ptr(&mut self, s: &str) -> PointerValue<'ctx> {
        if let Some(ptr) = self.strings.get(s) {
            return *ptr;
        }

        let id = self.next_id();
        let str_val = format!("{s}\0");
        let str_bytes = str_val.as_bytes();

        let string_arr = self.context.const_string(str_bytes, true);
        let global = self
            .module
            .add_global(string_arr.get_type(), None, &format!(".str_{id}"));
        global.set_initializer(&string_arr);

        let ptr = global.as_pointer_value();
        let i8_ptr = self
            .builder
            .build_bit_cast(ptr, self.ptr_type(), "")
            .unwrap()
            .into_pointer_value();

        self.strings.insert(s.to_string(), i8_ptr);
        i8_ptr
    }

    fn build_push(&self, val: IntValue<'ctx>) {
        let rt_fn = self.get_rt_fn("rt_push");
        self.builder.build_call(rt_fn, &[val.into()], "").unwrap();
    }

    fn build_pop(&self) -> IntValue<'ctx> {
        let rt_fn = self.get_rt_fn("rt_pop");
        self.builder
            .build_call(rt_fn, &[], "pop")
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value()
    }

    fn call_procedure(&mut self, ctx: ContexInfo) {
        let is_symbol_fn = self.get_rt_fn("rt_is_symbol");
        let top_fn = self.get_rt_fn("rt_top");
        let apply_fn = self.get_rt_fn("rt_apply");

        let top_val = self
            .builder
            .build_call(top_fn, &[], "proc_top")
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value();

        let is_sym = self
            .builder
            .build_call(is_symbol_fn, &[top_val.into()], "proc_is_sym")
            .unwrap()
            .try_as_basic_value()
            .left()
            .unwrap()
            .into_int_value();

        let zero = self.i32_type().const_int(0, false);
        let cond = self
            .builder
            .build_int_compare(IntPredicate::NE, is_sym, zero, "proc_cmp")
            .unwrap();

        let current_fn = self.current_fn.unwrap();
        let apply_block = self.context.append_basic_block(current_fn, "apply_path");
        let closure_block = self.context.append_basic_block(current_fn, "closure_path");
        let merge_block = self.context.append_basic_block(current_fn, "merge_path");

        self.builder
            .build_conditional_branch(cond, apply_block, closure_block)
            .unwrap();

        self.builder.position_at_end(apply_block);
        self.builder.build_call(apply_fn, &[], "").unwrap();
        self.builder
            .build_unconditional_branch(merge_block)
            .unwrap();

        self.builder.position_at_end(closure_block);

        if ctx.drop_env {
            let closure_str = self.get_string_ptr("__closure");

            let closure_val = self.build_pop();
            self.builder
                .build_call(
                    self.get_rt_fn("rt_add_root"),
                    &[closure_str.into(), closure_val.into()],
                    "",
                )
                .unwrap();
            let closure_root = self
                .builder
                .build_call(
                    self.get_rt_fn("rt_get_root"),
                    &[closure_str.into()],
                    "closure_val",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_int_value();
            self.builder
                .build_call(
                    self.get_rt_fn("rt_prepare_args"),
                    &[closure_root.into()],
                    "",
                )
                .unwrap();
            let closure_root2 = self
                .builder
                .build_call(
                    self.get_rt_fn("rt_remove_root"),
                    &[closure_str.into()],
                    "closure_val2",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_int_value();
            let func_ptr = self
                .builder
                .build_call(
                    self.get_rt_fn("rt_get_c_func"),
                    &[closure_root2.into()],
                    "func_ptr",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_pointer_value();

            let fn_type = self.void_type().fn_type(&[], false);
            self.builder
                .build_indirect_call(fn_type, func_ptr, &[], "")
                .unwrap();
        } else {
            let old_env_str = self.get_string_ptr("__old_env");
            let closure_str = self.get_string_ptr("__closure");

            let old_env = self
                .builder
                .build_call(self.get_rt_fn("rt_current_env"), &[], "old_env")
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_int_value();
            self.builder
                .build_call(
                    self.get_rt_fn("rt_add_root"),
                    &[old_env_str.into(), old_env.into()],
                    "",
                )
                .unwrap();

            let closure_val = self.build_pop();
            self.builder
                .build_call(
                    self.get_rt_fn("rt_add_root"),
                    &[closure_str.into(), closure_val.into()],
                    "",
                )
                .unwrap();
            let closure_root = self
                .builder
                .build_call(
                    self.get_rt_fn("rt_get_root"),
                    &[closure_str.into()],
                    "closure_val",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_int_value();

            self.builder
                .build_call(
                    self.get_rt_fn("rt_prepare_args"),
                    &[closure_root.into()],
                    "",
                )
                .unwrap();

            let old_env_val = self
                .builder
                .build_call(
                    self.get_rt_fn("rt_remove_root"),
                    &[old_env_str.into()],
                    "old_env_val",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_int_value();
            self.builder
                .build_call(self.get_rt_fn("rt_push"), &[old_env_val.into()], "")
                .unwrap();

            let closure_root2 = self
                .builder
                .build_call(
                    self.get_rt_fn("rt_remove_root"),
                    &[closure_str.into()],
                    "closure_val2",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_int_value();

            let func_ptr = self
                .builder
                .build_call(
                    self.get_rt_fn("rt_get_c_func"),
                    &[closure_root2.into()],
                    "func_ptr",
                )
                .unwrap()
                .try_as_basic_value()
                .left()
                .unwrap()
                .into_pointer_value();

            let fn_type = self.void_type().fn_type(&[], false);
            self.builder
                .build_indirect_call(fn_type, func_ptr, &[], "")
                .unwrap();

            self.builder
                .build_call(self.get_rt_fn("rt_swap"), &[], "")
                .unwrap();
            let new_env = self.build_pop();
            self.builder
                .build_call(self.get_rt_fn("rt_move_to_env"), &[new_env.into()], "")
                .unwrap();
        }

        self.builder
            .build_unconditional_branch(merge_block)
            .unwrap();
        self.builder.position_at_end(merge_block);

        if ctx.drop_ret {
            let _ = self.build_pop();
        }
    }

    fn emit_dbg_info(&mut self, node: &LexerMonad<Node>, ctx: ContexInfo) {
        let info = node.to_string().replace('"', "'");
        let info_str = self.get_string_ptr(&info);
        let drop_val = if ctx.drop_ret { 1u64 } else { 0u64 };
        let i32_val = self.i32_type().const_int(drop_val, false);
        let rt_fn = self.get_rt_fn("rt_evaluated");
        self.builder
            .build_call(rt_fn, &[info_str.into(), i32_val.into()], "")
            .unwrap();
    }

    pub fn finalize(&self) {
        if self.current_fn.is_some() {
            self.builder
                .build_return(Some(&self.i32_type().const_int(0, false)))
                .unwrap();
        }
    }

    pub fn create_jit_execution_engine(&self) -> Result<ExecutionEngine<'ctx>, String> {
        self.module
            .create_jit_execution_engine(OptimizationLevel::Default)
            .map_err(|e| e.to_str().unwrap_or("unknown LLVM error").to_string())
    }

    pub fn write_to_file(&self, path: &str) -> Result<(), String> {
        let ir = self.module.print_to_string();
        let ir_str = ir.to_str().unwrap_or("");
        std::fs::write(path, ir_str).map_err(|e| e.to_string())
    }
}

impl<'ctx> std::fmt::Display for LlvmCodeGen<'ctx> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let ir = self.module.print_to_string();
        let ir_str = ir.to_str().unwrap_or("");
        write!(f, "{ir_str}")
    }
}

pub fn compile_llvm(
    node: &LexerMonad<Node>,
    codegen: &mut LlvmCodeGen,
    dbg_info: bool,
) -> Result<(), String> {
    node.compile_llvm(
        codegen,
        ContexInfo {
            drop_env: false,
            drop_ret: false,
        },
        dbg_info,
    )
}

trait CompileLlvm {
    fn compile_llvm(
        &self,
        codegen: &mut LlvmCodeGen,
        ctx: ContexInfo,
        dbg_info: bool,
    ) -> Result<(), String>;
}

macro_rules! return_nil {
    ($codegen:expr, $ctx:expr) => {
        if !$ctx.drop_ret {
            let name_ptr = $codegen.get_string_ptr("nil");
            let rt_fn = $codegen.get_rt_fn("rt_new_symbol");
            $codegen
                .builder
                .build_call(rt_fn, &[name_ptr.into()], "")
                .unwrap();
        }
    };
}

macro_rules! set_family {
    ($codegen:expr, $func_name:expr, $get_target:expr, $cdr:expr, $ctx:expr) => {{
        let params = get_n_params($cdr.clone(), 2)?;
        let sym = &params[0];
        let expr = &params[1];
        let name = sym.borrow().as_user_symbol()?;
        expr.borrow().compile_llvm(
            $codegen,
            ContexInfo {
                drop_env: false,
                drop_ret: false,
            },
            false,
        )?;

        let target_bv: inkwell::values::BasicValueEnum = $get_target(&name);
        let target_val: inkwell::values::BasicMetadataValueEnum = target_bv.into();
        let arg2: inkwell::values::BasicMetadataValueEnum = $codegen.build_pop().into();
        let rt_fn = $codegen.get_rt_fn($func_name);
        $codegen
            .builder
            .build_call(rt_fn, &[target_val, arg2], "")
            .unwrap();
        return_nil!($codegen, $ctx);
        Ok(())
    }};
}

impl CompileLlvm for Symbol {
    fn compile_llvm(
        &self,
        codegen: &mut LlvmCodeGen,
        ctx: ContexInfo,
        _dbg_info: bool,
    ) -> Result<(), String> {
        if !ctx.drop_ret {
            match self {
                Symbol::User(name) => {
                    let name_ptr = codegen.get_string_ptr(name);
                    let rt_get = codegen.get_rt_fn("rt_get");
                    let result = codegen
                        .builder
                        .build_call(rt_get, &[name_ptr.into()], "")
                        .unwrap()
                        .try_as_basic_value()
                        .left()
                        .unwrap()
                        .into_int_value();
                    codegen.build_push(result);
                }
                _ => {
                    let sym_str = self.to_string();
                    let name_ptr = codegen.get_string_ptr(&sym_str);
                    let rt_fn = codegen.get_rt_fn("rt_new_symbol");
                    codegen
                        .builder
                        .build_call(rt_fn, &[name_ptr.into()], "")
                        .unwrap();
                }
            }
        }
        Ok(())
    }
}

impl CompileLlvm for LexerMonad<Node> {
    fn compile_llvm(
        &self,
        codegen: &mut LlvmCodeGen,
        ctx: ContexInfo,
        dbg_info: bool,
    ) -> Result<(), String> {
        match self.get() {
            Node::String(val) => {
                if !ctx.drop_ret {
                    let name_ptr = codegen.get_string_ptr(val);
                    let rt_fn = codegen.get_rt_fn("rt_new_symbol");
                    codegen
                        .builder
                        .build_call(rt_fn, &[name_ptr.into()], "")
                        .unwrap();
                }
                Ok(())
            }
            Node::Number(Number::Float(val)) => {
                if !ctx.drop_ret {
                    let rt_fn = codegen.get_rt_fn("rt_new_float");
                    let f64_val = codegen.f64_type().const_float(*val);
                    codegen
                        .builder
                        .build_call(rt_fn, &[f64_val.into()], "")
                        .unwrap();
                }
                Ok(())
            }
            Node::Number(Number::Int(val)) => {
                if !ctx.drop_ret {
                    let rt_fn = codegen.get_rt_fn("rt_new_integer");
                    let i64_val = codegen.i64_type().const_int(*val as u64, true);
                    codegen
                        .builder
                        .build_call(rt_fn, &[i64_val.into()], "")
                        .unwrap();
                }
                Ok(())
            }
            Node::Pair(car, cdr) => match car.borrow().get() {
                Node::Number(num) => Err(format!("{num} can not be the head of a list")),
                Node::SpecialForm(form) => match form {
                    SpecialForm::Lambda => {
                        if !ctx.drop_ret {
                            let (pattern, cddr) = cdr.borrow().as_pair()?;
                            let mut body = cddr.borrow().as_pair()?.0.borrow().clone();
                            let lambda_id = inc();

                            let (is_proper_list, pvec) = pattern.clone().vectorize();
                            for (i, sym) in pvec.iter().enumerate() {
                                let sym_monad = sym.borrow();
                                if let Node::Symbol(Symbol::User(_)) = sym_monad.get() {
                                    body = body.replace_node(
                                        sym_monad.get(),
                                        &Node::Symbol(Symbol::User(format!(
                                            "#{i}_func_{lambda_id}"
                                        ))),
                                    );
                                } else {
                                    return Err(self.error(format!(
                                        "arg {} is not a user symbol",
                                        sym.borrow()
                                    )));
                                }
                            }

                            let void_type = codegen.void_type();
                            let fn_type = void_type.fn_type(&[], false);
                            let func_name = format!("func_{lambda_id}");
                            let closure_fn = codegen.module.add_function(&func_name, fn_type, None);
                            let entry = codegen.context.append_basic_block(closure_fn, "entry");

                            let saved_pos = codegen.builder.get_insert_block();
                            let saved_fn = codegen.current_fn;
                            codegen.current_fn = Some(closure_fn);
                            codegen.builder.position_at_end(entry);

                            let lambda_ctx = ContexInfo {
                                drop_env: true,
                                drop_ret: false,
                            };
                            body.compile_llvm(codegen, lambda_ctx, dbg_info)?;

                            codegen.builder.build_return(None).unwrap();

                            codegen.current_fn = saved_fn;
                            if let Some(block) = saved_pos {
                                codegen.builder.position_at_end(block);
                            }

                            let name_ptr = codegen.get_string_ptr(&lambda_id.to_string());
                            let closure_fn_ptr = closure_fn.as_global_value().as_pointer_value();

                            let nargs_val = codegen.i64_type().const_int(pvec.len() as u64, false);
                            let variadic_val = codegen
                                .i32_type()
                                .const_int((!is_proper_list) as u64, false);

                            let rt_fn = codegen.get_rt_fn("rt_new_closure");
                            codegen
                                .builder
                                .build_call(
                                    rt_fn,
                                    &[
                                        name_ptr.into(),
                                        closure_fn_ptr.into(),
                                        nargs_val.into(),
                                        variadic_val.into(),
                                    ],
                                    "",
                                )
                                .unwrap();
                        }
                        Ok(())
                    }
                    SpecialForm::Display => {
                        let params = get_n_params(cdr.clone(), 1)?;
                        params[0].borrow().compile_llvm(
                            codegen,
                            ContexInfo {
                                drop_env: ctx.drop_env,
                                drop_ret: false,
                            },
                            dbg_info,
                        )?;

                        let rt_display = codegen.get_rt_fn("rt_display_node_idx");
                        let idx = codegen.build_pop();
                        let result = codegen
                            .builder
                            .build_call(rt_display, &[idx.into()], "")
                            .unwrap()
                            .try_as_basic_value()
                            .left()
                            .unwrap()
                            .into_pointer_value();

                        let fmt_str = codegen.get_string_ptr("%s");
                        let printf_fn = codegen.get_rt_fn("printf");
                        codegen
                            .builder
                            .build_call(printf_fn, &[fmt_str.into(), result.into()], "")
                            .unwrap();

                        let null_ptr = codegen.ptr_type().const_null();
                        let fflush_fn = codegen.get_rt_fn("fflush");
                        codegen
                            .builder
                            .build_call(fflush_fn, &[null_ptr.into()], "")
                            .unwrap();

                        return_nil!(codegen, ctx);
                        Ok(())
                    }
                    SpecialForm::NewLine => {
                        let _ = get_n_params(cdr.clone(), 0)?;
                        let nl_str = codegen.get_string_ptr("\\n");
                        let printf_fn = codegen.get_rt_fn("printf");
                        codegen
                            .builder
                            .build_call(printf_fn, &[nl_str.into()], "")
                            .unwrap();
                        return_nil!(codegen, ctx);
                        Ok(())
                    }
                    SpecialForm::BreakPoint => {
                        let _ = get_n_params(cdr.clone(), 0)?;
                        let rt_fn = codegen.get_rt_fn("rt_breakpoint");
                        codegen.builder.build_call(rt_fn, &[], "").unwrap();
                        return_nil!(codegen, ctx);
                        Ok(())
                    }
                    SpecialForm::Define => {
                        let params = get_n_params(cdr.clone(), 2)?;
                        if ctx.drop_env {
                            Ok(())
                        } else if let Node::Symbol(Symbol::User(name)) = params[0].borrow().get() {
                            params[1].borrow().compile_llvm(
                                codegen,
                                ContexInfo {
                                    drop_env: false,
                                    drop_ret: false,
                                },
                                dbg_info,
                            )?;
                            let name_ptr = codegen.get_string_ptr(name);
                            let val = codegen.build_pop();
                            let rt_fn = codegen.get_rt_fn("rt_define");
                            codegen
                                .builder
                                .build_call(rt_fn, &[name_ptr.into(), val.into()], "")
                                .unwrap();
                            return_nil!(codegen, ctx);
                            Ok(())
                        } else {
                            Err(format!(
                                "{} is not a user defined symbol",
                                params[0].borrow()
                            ))
                        }
                    }
                    SpecialForm::Set => {
                        set_family!(
                            codegen,
                            "rt_set",
                            |name: &String| -> inkwell::values::BasicValueEnum {
                                let name_ptr = codegen.get_string_ptr(name);
                                name_ptr.into()
                            },
                            cdr,
                            ctx
                        )
                    }
                    SpecialForm::SetCar => {
                        set_family!(
                            codegen,
                            "rt_set_car",
                            |name: &String| {
                                let name_ptr = codegen.get_string_ptr(name);
                                let rt_get = codegen.get_rt_fn("rt_get");
                                let result = codegen
                                    .builder
                                    .build_call(rt_get, &[name_ptr.into()], "")
                                    .unwrap()
                                    .try_as_basic_value()
                                    .left()
                                    .unwrap()
                                    .into_int_value();
                                result.into()
                            },
                            cdr,
                            ctx
                        )
                    }
                    SpecialForm::SetCdr => {
                        set_family!(
                            codegen,
                            "rt_set_cdr",
                            |name: &String| {
                                let name_ptr = codegen.get_string_ptr(name);
                                let rt_get = codegen.get_rt_fn("rt_get");
                                let result = codegen
                                    .builder
                                    .build_call(rt_get, &[name_ptr.into()], "")
                                    .unwrap()
                                    .try_as_basic_value()
                                    .left()
                                    .unwrap()
                                    .into_int_value();
                                result.into()
                            },
                            cdr,
                            ctx
                        )
                    }
                    SpecialForm::If => {
                        let params = get_n_params(cdr.clone(), 3)?;
                        params[0].borrow().compile_llvm(
                            codegen,
                            ContexInfo {
                                drop_env: false,
                                drop_ret: false,
                            },
                            dbg_info,
                        )?;

                        let idx = codegen.build_pop();
                        let rt_get_bool = codegen.get_rt_fn("rt_get_bool");
                        let bool_val = codegen
                            .builder
                            .build_call(rt_get_bool, &[idx.into()], "bool_val")
                            .unwrap()
                            .try_as_basic_value()
                            .left()
                            .unwrap()
                            .into_int_value();

                        let zero = codegen.i32_type().const_int(0, false);
                        let cond = codegen
                            .builder
                            .build_int_compare(IntPredicate::SGT, bool_val, zero, "if_cond")
                            .unwrap();

                        let current_fn = codegen.current_fn.unwrap();
                        let then_block = codegen.context.append_basic_block(current_fn, "then");
                        let else_block = codegen.context.append_basic_block(current_fn, "else");
                        let merge_block =
                            codegen.context.append_basic_block(current_fn, "if_merge");

                        codegen
                            .builder
                            .build_conditional_branch(cond, then_block, else_block)
                            .unwrap();

                        codegen.builder.position_at_end(then_block);
                        params[1].borrow().compile_llvm(codegen, ctx, dbg_info)?;
                        codegen
                            .builder
                            .build_unconditional_branch(merge_block)
                            .unwrap();

                        codegen.builder.position_at_end(else_block);
                        params[2].borrow().compile_llvm(codegen, ctx, dbg_info)?;
                        codegen
                            .builder
                            .build_unconditional_branch(merge_block)
                            .unwrap();

                        codegen.builder.position_at_end(merge_block);
                        Ok(())
                    }
                    SpecialForm::Quote => {
                        if !ctx.drop_ret {
                            let params = get_n_params(cdr.clone(), 1)?;
                            let expr_str = params[0].borrow().to_string();
                            let name_ptr = codegen.get_string_ptr(&expr_str);
                            let rt_fn = codegen.get_rt_fn("rt_new_constant");
                            codegen
                                .builder
                                .build_call(rt_fn, &[name_ptr.into()], "")
                                .unwrap();
                        }
                        Ok(())
                    }
                    SpecialForm::Begin => {
                        let (_, operands) = cdr.clone().vectorize();
                        if !operands.is_empty() {
                            for (i, operand) in operands.iter().enumerate() {
                                let is_last = i == operands.len() - 1;
                                let context = if is_last {
                                    ctx
                                } else {
                                    ContexInfo {
                                        drop_env: false,
                                        drop_ret: true,
                                    }
                                };
                                operand.borrow().compile_llvm(codegen, context, dbg_info)?;
                            }
                        }
                        Ok(())
                    }
                    SpecialForm::Import => {
                        let params = get_n_params(cdr.clone(), 1)?;
                        let name = params[0].borrow().to_string();
                        let name_ptr = codegen.get_string_ptr(&name);
                        let rt_fn = codegen.get_rt_fn("rt_import");
                        codegen
                            .builder
                            .build_call(rt_fn, &[name_ptr.into()], "")
                            .unwrap();
                        return_nil!(codegen, ctx);
                        Ok(())
                    }
                    SpecialForm::Read => {
                        let rt_fn = codegen.get_rt_fn("rt_read");
                        codegen.builder.build_call(rt_fn, &[], "").unwrap();
                        Ok(())
                    }
                    SpecialForm::Apply => {
                        let params = get_n_params(cdr.clone(), 2)?;
                        params[1].borrow().compile_llvm(
                            codegen,
                            ContexInfo {
                                drop_env: false,
                                drop_ret: false,
                            },
                            dbg_info,
                        )?;
                        let rt_fn = codegen.get_rt_fn("rt_list_to_stack");
                        codegen.builder.build_call(rt_fn, &[], "").unwrap();
                        params[0].borrow().compile_llvm(
                            codegen,
                            ContexInfo {
                                drop_env: false,
                                drop_ret: false,
                            },
                            dbg_info,
                        )?;

                        codegen.call_procedure(ctx);
                        Ok(())
                    }
                    form => unreachable!("{form}"),
                },
                _ => {
                    let operands = cdr.clone().vectorize_proper_list()?;

                    for operand in operands.iter().rev() {
                        operand.borrow().compile_llvm(
                            codegen,
                            ContexInfo {
                                drop_env: false,
                                drop_ret: false,
                            },
                            dbg_info,
                        )?;
                    }

                    let nargs_val = codegen.i64_type().const_int(operands.len() as u64, false);
                    let rt_fn = codegen.get_rt_fn("rt_new_integer");
                    codegen
                        .builder
                        .build_call(rt_fn, &[nargs_val.into()], "")
                        .unwrap();

                    car.borrow().compile_llvm(
                        codegen,
                        ContexInfo {
                            drop_env: false,
                            drop_ret: false,
                        },
                        dbg_info,
                    )?;

                    codegen.call_procedure(ctx);
                    Ok(())
                }
            },
            Node::SpecialForm(_) => unreachable!("{self}"),
            Node::Symbol(sym) => sym.compile_llvm(codegen, ctx, dbg_info),
        }?;

        if dbg_info {
            codegen.emit_dbg_info(self, ctx);
        }
        Ok(())
    }
}
