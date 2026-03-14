//! The compiler module.
//!
//! This module provides compilation of AST nodes into C code. The compiler transforms
//! Lisp expressions into C code that uses the Relic runtime C API.
//!
//! ## Compilation Pipeline
//!
//! 1. **AST to C**: The [`Compile`] trait transforms [`Node`] objects into C code
//! 2. **C to Shared Library**: The C code is compiled using GCC/Clang into a `.so`/`.dylib`
//! 3. **Dynamic Loading**: The shared library is loaded via [`libloading`](crate::package)
//!
//! ## Code Generation
//!
//! The [`CodeGen`] struct manages code generation with three modes:
//! - [`CodeGenType::Main`]: Generates a `main()` function for standalone programs
//! - [`CodeGenType::Library`]: Generates a named function for packages
//! - [`CodeGenType::Internal`]: Generates internal functions for closures
//!
//! Each closure compiles to a separate C function. The main/library function calls these
//! closure functions as needed.
//!
//! ## Runtime API Generation
//!
//! The compiler generates calls to the Relic C runtime API:
//! - `rt_new_integer(n)`, `rt_new_float(n)`: Create numbers
//! - `rt_new_symbol("name")`: Create symbols
//! - `rt_push(idx)`, `rt_pop()`: Stack operations
//! - `rt_define("name", value)`: Define variables
//! - `rt_get("name")`: Get variable values
//! - `rt_new_closure(id, func, nargs, variadic)`: Create closures
//! - `rt_apply()`: Apply operators
//!
//! ## Special Forms
//!
//! Special forms are handled specially during compilation:
//! - [`SpecialForm::Lambda`]: Creates closure functions
//! - [`SpecialForm::Define`]: Defines variables
//! - [`SpecialForm::Set`], [`SpecialForm::SetCar`], [`SpecialForm::SetCdr`]: Assignment
//! - [`SpecialForm::If`]: Conditional compilation
//! - [`SpecialForm::Begin`]: Sequence compilation
//! - [`SpecialForm::Quote`]: Constant generation
//! - [`SpecialForm::Import`]: Package import
//!
//! ## Optimization: ContexInfo
//!
//! The [`ContexInfo`] struct controls code generation optimization:
//! - `drop_env`: If true, skip environment preservation when no side effects
//! - `drop_ret`: If true, skip return value when caller doesn't need it
//!
//! Lambda bodies use `drop_env=true` for tail-call optimization since the
//! environment is a copy that won't be used after return.
//!
//! ## Procedure Calls
//!
//! The [`call_procedure`] function generates code to call procedures:
//! 1. Check if operator is a built-in symbol (use `rt_apply()`)
//! 2. Otherwise, treat as closure:
//!    - If `drop_env`: Simple tail-call (no environment preservation)
//!    - Otherwise: Full call with environment save/restore
//!

use std::{collections::HashMap, fmt::Display};

use crate::{
    lexer::LexerMonad,
    node::Node,
    number::Number,
    symbol::{SpecialForm, Symbol},
    util::{get_n_params, inc, Vectorize},
};

/// Type of code generators.
pub enum CodeGenType {
    /// The generator is generating a closure that will not be used by other
    /// programs.
    Internal(usize),
    /// The generator is generating `main` function.
    Main,
    /// The generator is generating the top-level function that can be used
    /// by other programs.
    Library(String),
}

/// Code generator.
///
/// A code generator is responsible for writing one function's code.
///
/// When the compiler needs to create a closure, it creates a new generator
/// to write the closure's code, then merge the new generator into the old one.
///
/// After compilation, the generator for the main function will have the same
/// layout as the compiled C source code.
pub struct CodeGen {
    /// The code generator's type.
    ty: CodeGenType,
    /// Closures. Values are function body without boilerplate.
    closures: HashMap<usize, String>,
    /// Body of the function the generator is writing.
    body: String,
}

impl CodeGen {
    /// Creates a new CodeGen for a main function.
    pub fn new_main() -> Self {
        CodeGen {
            ty: CodeGenType::Main,
            closures: HashMap::new(),
            body: String::new(),
        }
    }
    fn new_internal(id: usize) -> Self {
        CodeGen {
            ty: CodeGenType::Internal(id),
            closures: HashMap::new(),
            body: String::new(),
        }
    }
    /// Creates a new CodeGen for a library/package function.
    ///
    /// # Parameters
    ///
    /// * `name` - The name of the package
    pub fn new_library(name: String) -> Self {
        CodeGen {
            ty: CodeGenType::Library(name),
            closures: HashMap::new(),
            body: String::new(),
        }
    }

    /// Appends C code to the function body.
    fn append_code(&mut self, code: &str) {
        self.body += code;
    }
    /// Merge the generator of a function created by this generator's function.
    ///
    /// This is used when compiling lambda expressions - the lambda's body
    /// becomes a separate internal function that is merged into the parent.
    fn merge(&mut self, func: Self) {
        if let CodeGenType::Internal(id) = func.ty {
            self.closures.extend(func.closures);
            assert!(self.closures.insert(id, func.body).is_none());
        } else {
            panic!("Merging top-level generator: {func}");
        }
    }
}

macro_rules! return_nil {
    ($codegen:expr, $ctx:expr) => {
        if !$ctx.drop_ret {
            $codegen.append_code("rt_new_symbol(\"nil\");");
        }
    };
}

macro_rules! set_family {
    ($func_name:expr, $target:expr, $cdr:expr, $codegen:expr, $ctx:expr, $dbg_info:expr) => {{
        let params = get_n_params($cdr.clone(), 2)?;
        let sym = &params[0];
        let expr = &params[1];
        let name = sym.borrow().as_user_symbol()?;
        expr.borrow().compile(
            $codegen,
            ContexInfo {
                drop_env: false,
                drop_ret: false,
            },
            $dbg_info,
        )?;
        $codegen.append_code(&format!("rt_{}({}, rt_pop());", $func_name, $target(name)));
        return_nil!($codegen, $ctx);
        Ok(())
    }};
}

impl Display for CodeGen {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (func_name, start_code) = match &self.ty {
            CodeGenType::Internal(id) => panic!("Writing internal closure {id}"),
            CodeGenType::Main => ("main".to_string(), "rt_start();".to_string()),
            CodeGenType::Library(name) => (name.to_string(), String::new()),
        };
        let main_body = &self.body;

        for name in self.closures.keys() {
            writeln!(f, "static void func_{name}();")?;
        }
        writeln!(
            f,
            r#"
#include"runtime.h"
int {func_name}() {{
    {start_code}
    {main_body}
    return 0;
}}"#,
        )?;
        for (name, body) in &self.closures {
            writeln!(
                f,
                r#"
static void func_{name}() {{
    {body}
}}"#
            )?;
        }
        Ok(())
    }
}

/// Context information that can be used in optimization.
#[derive(Debug, Clone, Copy)]
struct ContexInfo {
    /// Whether to drop the current environment or keep it in the stack.
    ///
    /// When this field is `true`:
    /// - If the statement does not have side effect, then the entire code
    ///   won't be generated.
    /// - If the statement has side effect, then the return value won't be
    ///   pushed to the stack.
    drop_env: bool,
    /// Whether to drop the return value or keep it in the stack.
    ///
    /// In the code generated by our compiler, environment is callee-saved,
    /// i.e. the callee should save and restore the enviromnemt. If this field
    /// is true, then the callee doesn't need to restore the environment.
    drop_ret: bool,
}

/// ContexInfo that does not drop anything.
macro_rules! no_drop {
    () => {
        ContexInfo {
            drop_env: false,
            drop_ret: false,
        }
    };
}

/// Calls [LexerMonad<Node>::compile] with no optimization at the top level.
pub fn compile(
    node: &LexerMonad<Node>,
    codegen: &mut CodeGen,
    dbg_info: bool,
) -> Result<(), String> {
    node.compile(codegen, no_drop!(), dbg_info)
}

/// The trait that defines a way to compile the object.
trait Compile {
    /// Compile the object.
    ///
    /// The semantics of the compiled code is to evaluate this object and push
    /// its value to the stack.
    ///
    /// If `dbg_info` is true, a special statement will be inserted at the end
    /// of each evaluation to support `n` command in the debugger.
    fn compile(&self, codegen: &mut CodeGen, ctx: ContexInfo, dbg_info: bool)
        -> Result<(), String>;
}

impl Compile for Symbol {
    fn compile(
        &self,
        codegen: &mut CodeGen,
        ctx: ContexInfo,
        _dbg_info: bool,
    ) -> Result<(), String> {
        if !ctx.drop_ret {
            let code = match self {
                Symbol::User(name) => {
                    format!("rt_push(rt_get(\"{name}\"));")
                }
                _ => {
                    format!("rt_new_symbol(\"{self}\");")
                }
            };
            codegen.append_code(&code);
        }
        Ok(())
    }
}

impl Compile for LexerMonad<Node> {
    fn compile(
        &self,
        codegen: &mut CodeGen,
        ctx: ContexInfo,
        dbg_info: bool,
    ) -> Result<(), String> {
        match self.get() {
            Node::String(val) => {
                if !ctx.drop_ret {
                    codegen.append_code(&format!("rt_new_symbol(\"{val}\");"))
                }
                Ok(())
            }
            Node::Number(Number::Float(val)) => {
                if !ctx.drop_ret {
                    codegen.append_code(&format!("rt_new_float({val});"))
                }
                Ok(())
            }
            Node::Number(Number::Int(val)) => {
                if !ctx.drop_ret {
                    codegen.append_code(&format!("rt_new_integer({val});"))
                }
                Ok(())
            }
            Node::Pair(car, cdr) => match car.borrow().get() {
                Node::Number(num) => Err(format!("{num} can not be the head of a list")),
                Node::SpecialForm(form) => match form {
                    // This corresponds to the apply part of the interpreter.
                    // Other objects' application are deferred to run-time, but
                    // special forms must be applied at compile-time.
                    SpecialForm::Lambda => {
                        if !ctx.drop_ret {
                            let (pattern, cddr) = cdr.borrow().as_pair()?;
                            let mut body = cddr.borrow().as_pair()?.0.borrow().clone();
                            let lambda_id = inc();

                            // Replace operands with its index.
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

                            // Generate function body.
                            let mut lambda_gen = CodeGen::new_internal(lambda_id);
                            // The lambda body should not drop the return value,
                            // but it can drop the environment as it is just a
                            // copy of current environment and no one will use it.
                            // This is how Relic do tail-recursive optimization.
                            let ctx = ContexInfo {
                                drop_env: true,
                                drop_ret: false,
                            };
                            body.compile(&mut lambda_gen, ctx, dbg_info)?;
                            codegen.merge(lambda_gen);

                            // Write the code that creates the closure.
                            codegen.append_code(&format!(
                                "rt_new_closure(\"{lambda_id}\", func_{lambda_id}, {}, {});",
                                pvec.len(),
                                !is_proper_list
                            ));
                        }
                        Ok(())
                    }
                    SpecialForm::Display => {
                        let params = get_n_params(cdr.clone(), 1)?;
                        // Keep the node to display.
                        params[0].borrow().compile(
                            codegen,
                            ContexInfo {
                                drop_env: ctx.drop_env,
                                drop_ret: false,
                            },
                            dbg_info,
                        )?;
                        codegen.append_code(
                            r#"
printf("%s",rt_display_node_idx(rt_pop()));
fflush(NULL);"#,
                        );
                        return_nil!(codegen, ctx);
                        Ok(())
                    }
                    SpecialForm::NewLine => {
                        let _ = get_n_params(cdr.clone(), 0)?;
                        codegen.append_code("printf(\"\\n\");");
                        return_nil!(codegen, ctx);
                        Ok(())
                    }
                    SpecialForm::BreakPoint => {
                        let _ = get_n_params(cdr.clone(), 0)?;
                        codegen.append_code("rt_breakpoint();");
                        return_nil!(codegen, ctx);
                        Ok(())
                    }
                    SpecialForm::Define => {
                        let params = get_n_params(cdr.clone(), 2)?;
                        if ctx.drop_env {
                            // The environment will be dropped anyway.
                            Ok(())
                        } else if let Node::Symbol(Symbol::User(name)) = params[0].borrow().get() {
                            // `define` uses both of the environment and the return value.
                            // So do `set*`.
                            params[1].borrow().compile(codegen, no_drop!(), dbg_info)?;
                            codegen.append_code(&format!("rt_define(\"{name}\", rt_pop());"));
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
                            "set",
                            |name| { format!("\"{name}\"") },
                            cdr,
                            codegen,
                            ctx,
                            dbg_info
                        )
                    }
                    SpecialForm::SetCar => {
                        set_family!(
                            "set_car",
                            |name| { format!("rt_get(\"{name}\")") },
                            cdr,
                            codegen,
                            ctx,
                            dbg_info
                        )
                    }
                    SpecialForm::SetCdr => {
                        set_family!(
                            "set_cdr",
                            |name| { format!("rt_get(\"{name}\")") },
                            cdr,
                            codegen,
                            ctx,
                            dbg_info
                        )
                    }
                    SpecialForm::If => {
                        // The value and environment of precondition must be preserved;
                        // those of the branches can be dropped.
                        let params = get_n_params(cdr.clone(), 3)?;
                        params[0].borrow().compile(codegen, no_drop!(), dbg_info)?;
                        codegen.append_code("if (rt_get_bool(rt_pop()) > 0) {");
                        params[1].borrow().compile(codegen, ctx, dbg_info)?;
                        codegen.append_code("} else {");
                        params[2].borrow().compile(codegen, ctx, dbg_info)?;
                        codegen.append_code("}");
                        Ok(())
                    }
                    SpecialForm::Quote => {
                        if !ctx.drop_ret {
                            let params = get_n_params(cdr.clone(), 1)?;
                            codegen.append_code(&format!(
                                "rt_new_constant(\"{}\");",
                                params[0].borrow()
                            ));
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
                                    // For all but the last operand, keep environment but drop return value
                                    ContexInfo {
                                        drop_env: false,
                                        drop_ret: true,
                                    }
                                };
                                operand.borrow().compile(codegen, context, dbg_info)?;
                            }
                        }
                        Ok(())
                    }
                    SpecialForm::Import => {
                        let params = get_n_params(cdr.clone(), 1)?;
                        codegen.append_code(&format!("rt_import(\"{}\");", params[0].borrow()));
                        return_nil!(codegen, ctx);
                        Ok(())
                    }
                    SpecialForm::Read => {
                        codegen.append_code("rt_read();");
                        Ok(())
                    }
                    SpecialForm::Apply => {
                        let params = get_n_params(cdr.clone(), 2)?;
                        // operand list
                        params[1].borrow().compile(codegen, no_drop!(), dbg_info)?;

                        // list -> stack
                        codegen.append_code("rt_list_to_stack();");

                        // operator
                        params[0].borrow().compile(codegen, no_drop!(), dbg_info)?;

                        call_procedure(ctx, codegen);
                        Ok(())
                    }
                    form => unreachable!("{form}"),
                },
                _ => {
                    let operands = cdr.clone().vectorize_proper_list()?;

                    // operands
                    for operand in operands.iter().rev() {
                        operand.borrow().compile(codegen, no_drop!(), dbg_info)?;
                    }

                    // nargs
                    codegen.append_code(&format!("rt_new_integer({});", operands.len()));

                    // operator
                    car.borrow().compile(codegen, no_drop!(), dbg_info)?;

                    call_procedure(ctx, codegen);
                    Ok(())
                }
            },
            Node::SpecialForm(_) => unreachable!("{self}"),
            Node::Symbol(sym) => sym.compile(codegen, ctx, dbg_info),
        }?;
        if dbg_info {
            let self_str = self.to_string();
            let self_str = self_str.replace("\"", "'");
            codegen.append_code(&format!(
                "rt_evaluated(\"{}\", {});",
                self_str,
                if ctx.drop_ret { 1 } else { 0 }
            ));
        }
        Ok(())
    }
}

fn call_procedure(ctx: ContexInfo, codegen: &mut CodeGen) {
    let call_closure = if ctx.drop_env {
        r#"
rt_add_root("__closure", rt_pop());
rt_prepare_args(rt_get_root("__closure"));
c_func func = rt_get_c_func(rt_remove_root("__closure"));
func();
"#
    } else {
        r#"
rt_add_root("__old_env", rt_current_env());
rt_add_root("__closure", rt_pop());
rt_prepare_args(rt_get_root("__closure"));
rt_push(rt_remove_root("__old_env"));
c_func func = rt_get_c_func(rt_remove_root("__closure"));
func();
rt_swap();
rt_move_to_env(rt_pop());
"#
    };

    codegen.append_code(&format!(
        r#"
if (rt_is_symbol(rt_top())) {{
    rt_apply();
}} else {{
    {call_closure}
}}"#
    ));

    // Drop the result if the caller wants to drop it.
    if ctx.drop_ret {
        codegen.append_code("rt_pop();");
    }
}
