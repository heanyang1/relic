//! The compiler module.

use std::{
    collections::HashMap,
    fmt::Display,
    sync::{LazyLock, Mutex},
};

use crate::{
    lexer::Number,
    logger::log_debug,
    node::{Node, Pattern},
    symbol::{SpecialForm, Symbol},
    util::{get_n_params, vectorize},
};

static COUNTER: LazyLock<Mutex<usize>> = LazyLock::new(|| Mutex::new(0));

fn inc() -> usize {
    let mut counter = COUNTER.lock().unwrap();
    *counter += 1;
    *counter
}

/// Type of code generators.
pub enum CodeGenType {
    /// The generator is generating a closure that will not be used by other
    /// programs.
    Internal(usize),
    /// The generator is generating `main` function.
    Main,
    /// The generator is generating the top-level function that can be used.
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
    pub fn new_library(name: String) -> Self {
        CodeGen {
            ty: CodeGenType::Library(name),
            closures: HashMap::new(),
            body: String::new(),
        }
    }

    fn append_code(&mut self, code: &str) {
        self.body += code;
    }
    /// Merge the generator of a function created by this generator's function.
    fn merge(&mut self, func: Self) {
        if let CodeGenType::Internal(id) = func.ty {
            self.closures.extend(func.closures);
            assert!(self.closures.insert(id, func.body).is_none());
        } else {
            panic!("Merging top-level generator: {func}");
        }
    }
}

macro_rules! set_family {
    ($func_name:expr, $target:expr, $cdr:expr, $codegen:expr) => {{
        let params = get_n_params($cdr.clone(), 2)?;
        let sym = &params[0];
        let expr = &params[1];
        let name = sym.borrow().as_user_symbol()?;
        expr.borrow().compile($codegen)?;
        $codegen.append_code(&format!(
            r#"
rt_{}({}, rt_pop());
rt_new_symbol("nil");"#,
            $func_name,
            $target(name)
        ));
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

/// The trait that defines a way to compile the object.
pub trait Compile {
    /// Compile the object.
    ///
    /// The semantics of the compiled code is to evaluate this object
    /// and push its value to the stack.
    fn compile(&self, codegen: &mut CodeGen) -> Result<(), String>;
}

impl Compile for Symbol {
    fn compile(&self, codegen: &mut CodeGen) -> Result<(), String> {
        let code = match self {
            Symbol::User(name) => {
                format!("rt_push(rt_get(\"{name}\"));")
            }
            _ => {
                format!("rt_new_symbol(\"{self}\");")
            }
        };
        codegen.append_code(&code);
        Ok(())
    }
}

impl Compile for Node {
    fn compile(&self, codegen: &mut CodeGen) -> Result<(), String> {
        match self {
            Node::Number(Number::Float(val)) => {
                codegen.append_code(&format!("rt_new_float({val});"));
                Ok(())
            }
            Node::Number(Number::Int(val)) => {
                codegen.append_code(&format!("rt_new_integer({val});"));
                Ok(())
            }
            Node::Pair(car, cdr) => match &*car.borrow() {
                Node::Number(num) => Err(format!("{num} can not be the head of a list")),
                Node::Procedure(_, _, _) => unreachable!(),
                Node::SpecialForm(form) => match form {
                    // This corresponds to the apply part of the interpreter.
                    // Other objects' application are deferred to run-time, but
                    // special forms must be applied at compile-time.
                    SpecialForm::Lambda => {
                        let (pattern, body) = cdr.borrow().as_pair()?;

                        // Use `begin` to support multiple statements.
                        let mut body =
                            Node::Pair(Node::SpecialForm(SpecialForm::Begin).into(), body);

                        let lambda_id = inc();

                        // Replace operands with its index.
                        let pattern = Pattern::try_from(pattern.clone())?;
                        let mut pvec = vec![];
                        pattern.vectorize(&mut pvec);
                        for (i, sym) in pvec.iter().enumerate() {
                            body.replace(
                                &Node::Symbol(Symbol::User(sym.clone())),
                                &Node::Symbol(Symbol::User(format!("#{i}_func_{lambda_id}"))),
                            );
                        }

                        // Generate function body.
                        let mut lambda_gen = CodeGen::new_internal(lambda_id);
                        body.compile(&mut lambda_gen)?;
                        codegen.merge(lambda_gen);

                        let x = pattern.is_proper_list();
                        log_debug(&format!("is_proper_list: {x}"));

                        // Write the code that creates the closure.
                        codegen.append_code(&format!(
                            "rt_new_closure({lambda_id}, func_{lambda_id}, {}, {});",
                            pvec.len(),
                            !pattern.is_proper_list()
                        ));

                        Ok(())
                    }
                    SpecialForm::Display => {
                        let params = get_n_params(cdr.clone(), 1)?;
                        params[0].borrow().compile(codegen)?;
                        codegen.append_code(
                            r#"
printf("%s",rt_display_node_idx(rt_pop()));
fflush(NULL);
rt_new_symbol("nil");"#,
                        );
                        Ok(())
                    }
                    SpecialForm::NewLine => {
                        let _ = get_n_params(cdr.clone(), 0)?;
                        codegen.append_code(
                            r#"
printf("\n");
rt_new_symbol("nil");"#,
                        );
                        Ok(())
                    }
                    SpecialForm::BreakPoint | SpecialForm::Graphviz => {
                        let _ = get_n_params(cdr.clone(), 0)?;
                        codegen.append_code("rt_new_symbol(\"nil\");");
                        Ok(())
                    }
                    SpecialForm::Define => {
                        let params = get_n_params(cdr.clone(), 2)?;
                        if let Node::Symbol(Symbol::User(name)) = &*params[0].borrow() {
                            params[1].borrow().compile(codegen)?;
                            codegen.append_code(&format!(
                                r#"
rt_define("{name}", rt_pop());
rt_new_symbol("nil");"#
                            ));
                            Ok(())
                        } else {
                            Err(format!(
                                "{} is not a user defined symbol",
                                params[0].borrow()
                            ))
                        }
                    }
                    SpecialForm::Set => {
                        set_family!("set", |name| { format!("\"{name}\"") }, cdr, codegen)
                    }
                    SpecialForm::SetCar => {
                        set_family!(
                            "set_car",
                            |name| { format!("rt_get(\"{name}\")") },
                            cdr,
                            codegen
                        )
                    }
                    SpecialForm::SetCdr => {
                        set_family!(
                            "set_cdr",
                            |name| { format!("rt_get(\"{name}\")") },
                            cdr,
                            codegen
                        )
                    }
                    SpecialForm::If => {
                        let params = get_n_params(cdr.clone(), 3)?;
                        params[0].borrow().compile(codegen)?;
                        codegen.append_code("if (rt_get_bool(rt_pop()) > 0) {");
                        params[1].borrow().compile(codegen)?;
                        codegen.append_code("} else {");
                        params[2].borrow().compile(codegen)?;
                        codegen.append_code("}");
                        Ok(())
                    }
                    SpecialForm::Quote => {
                        let params = get_n_params(cdr.clone(), 1)?;
                        codegen
                            .append_code(&format!("rt_new_constant(\"{}\");", params[0].borrow()));
                        Ok(())
                    }
                    SpecialForm::Begin => {
                        let mut pop = false;
                        for operands in vectorize(cdr.clone())? {
                            if pop {
                                codegen.append_code("rt_pop();");
                            } else {
                                pop = true;
                            }
                            operands.borrow().compile(codegen)?;
                        }
                        Ok(())
                    }
                    SpecialForm::Import => {
                        let params = get_n_params(cdr.clone(), 1)?;
                        codegen.append_code(&format!(
                            "rt_import(\"{}\");rt_new_symbol(\"nil\");",
                            params[0].borrow()
                        ));
                        Ok(())
                    }
                    _ => unreachable!(),
                },
                _ => {
                    let operands = vectorize(cdr.clone())?;
                    let len_operands = operands.len();

                    for operand in operands.iter().rev() {
                        operand.borrow().compile(codegen)?;
                    }

                    car.borrow().compile(codegen)?;

                    codegen.append_code(&format!(
                        r#"
if (rt_is_symbol(rt_top())) {{
    rt_apply({len_operands});
}} else {{
    rt_call_closure({len_operands});
}}"#
                    ));
                    Ok(())
                }
            },
            Node::Procedure(_, _, _) | Node::SpecialForm(_) => unreachable!(),
            Node::Symbol(sym) => sym.compile(codegen),
        }
    }
}
