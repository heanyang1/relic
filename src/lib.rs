pub mod compile;
pub mod env;
pub mod eval;
pub mod graph;
pub mod lexer;
pub mod logger;
pub mod node;
pub mod parser;
pub mod preprocess;
pub mod runtime;
pub mod symbol;
mod util;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
    sync::{LazyLock, Mutex},
};

use crate::{
    env::Env,
    eval::{Eval, EvalResult},
    graph::PrintState,
    lexer::{Lexer, Number},
    logger::{log_error, unwrap_result},
    node::{Node, NodeEnv},
    parser::Parse,
    preprocess::PreProcess,
    runtime::{Closure, LoadToRuntime, Runtime, RuntimeNode, StackMachine},
    symbol::Symbol,
    util::CVoidFunc,
};
#[cfg(not(target_arch = "wasm32"))]
use libloading::{self, Library};
use wasm_bindgen::prelude::*;

#[derive(Clone)]
struct WebEval {
    node: Rc<RefCell<Node>>,
    graph_cnt: usize,
}

impl WebEval {
    fn new() -> Self {
        WebEval {
            node: nil!().into(),
            graph_cnt: 1,
        }
    }
}

impl EvalResult for WebEval {
    fn bind_node(mut self, new_node: Rc<RefCell<Node>>) -> Self {
        self.node = new_node;
        self
    }
    fn node(&self) -> Rc<RefCell<Node>> {
        self.node.clone()
    }
    fn bind_display(self, output: &str) -> Self {
        writeStdout(output);
        self
    }
    fn bind_graph(mut self, env: Rc<RefCell<NodeEnv>>) -> Self {
        let state = PrintState::new(env, format!("graph_{}", self.graph_cnt));
        let output = format!("{state}");
        writeGraph(&output, self.graph_cnt);
        self.graph_cnt += 1;
        self
    }
    fn bind_break(self, _: Rc<RefCell<NodeEnv>>) -> Self {
        self
    }
    fn bind_eval(
        self,
        _: Rc<RefCell<Node>>,
        _: Rc<RefCell<Node>>,
        _: Rc<RefCell<NodeEnv>>,
    ) -> Self {
        self
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn writeOutput(text: &str);

    #[wasm_bindgen(js_namespace = window)]
    fn writeStdout(text: &str);

    #[wasm_bindgen(js_namespace = window)]
    fn writeGraph(text: &str, graph_count: usize);
}

#[wasm_bindgen]
pub fn evaluate(input: &str) {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    let mut result = WebEval::new();
    let mut tokens = Lexer::new(input);
    let mut macros = HashMap::new();

    while let Ok(mut node) = Node::parse(&mut tokens) {
        let node = node.preprocess(&mut macros);
        if node.is_err() {
            writeOutput(&format!(
                "Error preprocessing expression: {}",
                node.err().unwrap()
            ));
            return;
        }
        let eval = node.unwrap().eval(env.clone(), result.clone());
        if eval.is_err() {
            writeOutput(&format!(
                "Error evaluating expression: {}",
                eval.err().unwrap()
            ));
            return;
        }
        result = eval.unwrap();
    }
    writeOutput(&format!("{}\n", result.node.borrow()));
}

/// The runtime that is pointed by all C bindings.
pub static RT: LazyLock<Mutex<Runtime>> = LazyLock::new(|| Mutex::new(Runtime::new(1)));

/// Initialize the runtime environment.
#[unsafe(no_mangle)]
pub extern "C" fn rt_start() {
    let mut rt = RT.lock().unwrap();
    rt.top_env();
}

/// Create a new closure and push the result to the stack.
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_closure(id: usize, func: CVoidFunc, nargs: usize, variadic: bool) {
    let mut rt = RT.lock().unwrap();
    rt.try_gc();

    let val = Closure::new(id, func, nargs, variadic, &rt);
    val.load_to(&mut rt).unwrap();
}

/// The function called by [rt_call_closure]. It is designed not to be a method
/// of [Runtime] to avoid deadlock when calling the closure.
fn call_closure(nparams: usize) -> Result<(), String> {
    let c = {
        let mut runtime = RT.lock().unwrap();
        let index = runtime.pop();
        let c = if let RuntimeNode::Closure(c) = runtime.get_node(true, index) {
            Ok(c)
        } else {
            Err(format!(
                "{} is not a closure",
                runtime.display_node_idx(index)
            ))
        }?
        .clone();

        if !c.variadic && c.nargs != nparams {
            return Err(format!(
                "arity mismatch: expect {}, found {}",
                c.nargs, nparams
            ));
        }

        // We set `old_env` to root temporarily so that it won't be mixed up with
        // parameters in the stack.
        let old_env = runtime.current_env();
        runtime.add_root("__old_env".to_string(), old_env);

        // Construct and move to an environment.
        let env = runtime.new_env("closure".to_string(), c.env);
        runtime.move_to_env(env);

        if c.nargs > 0 {
            // Add arguments to the environment.
            for i in 0..c.nargs - 1 {
                let value = runtime.pop();
                runtime
                    .current_env()
                    .define(&format!("#{i}_func_{}", c.id), value, &mut runtime);
            }

            // Zip the rest of the arguments (args[c.nargs-1..nparams])
            // if the closure is variadic.
            if c.variadic {
                let values = runtime.node_vec_from_stack(nparams - c.nargs + 1);
                values.load_to(&mut runtime)?;
            }

            // Add the last argument.
            let last = runtime.pop();
            runtime.current_env().define(
                &format!("#{}_func_{}", c.nargs - 1, c.id),
                last,
                &mut runtime,
            );
        }

        // here we can save `old_env` to the stack.
        let old_env = runtime.remove_root("__old_env");
        runtime.push(old_env);
        c
    }; // unlocks runtime

    // Call function in the environment. Locks runtime.
    (c.body)();

    let mut runtime = RT.lock().unwrap();
    let ret = runtime.pop();

    // Move back to the previous environment.
    let old_env = runtime.pop();
    runtime.move_to_env(old_env);

    runtime.push(ret);
    Ok(())
}

/// Calls a closure.
///
/// The parameters are in the stack. The first element popped has name `#0`,
/// the second element popped has name `#1`, etc.
/// Pushes the return value to the stack when returned.
///
/// When an error occurs, the closure is popped from the stack and environment
/// remains unchanged.
#[unsafe(no_mangle)]
pub extern "C" fn rt_call_closure(nparams: usize) -> i32 {
    if call_closure(nparams).is_ok() {
        1
    } else {
        log_error("Error in rt_call_closure: invalid parameters");
        0
    }
}

/// Push a node index onto the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_push(index: usize) {
    let mut rt = RT.lock().unwrap();
    rt.push(index);
}

/// Pop a node index from the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_pop() -> usize {
    let mut rt = RT.lock().unwrap();
    rt.pop()
}

/// Get the stack top
#[unsafe(no_mangle)]
pub extern "C" fn rt_top() -> usize {
    let mut rt = RT.lock().unwrap();
    rt.top()
}

/// Remove a root variable
#[unsafe(no_mangle)]
pub extern "C" fn rt_remove_root(name: *const u8) -> usize {
    let mut rt = RT.lock().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.remove_root(name_str)
    } else {
        log_error("Error in rt_remove_root: invalid string");
        0
    }
}

/// Display a node by index as string
#[unsafe(no_mangle)]
pub extern "C" fn rt_display_node_idx(index: usize) -> *mut i8 {
    let rt = RT.lock().unwrap();
    let result = rt.display_node_idx(index);
    let c_str = std::ffi::CString::new(result).unwrap();
    c_str.into_raw()
}

/// Evaluate an expression and push the result onto the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_apply(nargs: usize) -> usize {
    let mut rt = RT.lock().unwrap();
    match rt.apply(nargs) {
        Ok(()) => 1,
        Err(e) => {
            log_error(&format!("Error in rt_apply: {e}"));
            0
        }
    }
}

/// Parse an expression from a string and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_constant(expr: *const u8) {
    let mut rt = RT.lock().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(expr as *const i8) };
    if let Ok(expr_str) = c_str.to_str() {
        unwrap_result(expr_str.load_to(&mut rt), ());
    } else {
        log_error("Error in rt_push_constant: invalid string");
    }
}

/// Create a new symbol and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_symbol(name: *const u8) {
    let mut rt = RT.lock().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        unwrap_result(Symbol::from(name_str.to_string()).load_to(&mut rt), ());
    } else {
        log_error("Error in rt_new_symbol: invalid string");
    }
}

/// Create a new number and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_integer(value: i64) {
    let mut rt = RT.lock().unwrap();
    Number::Int(value).load_to(&mut rt).unwrap()
}

/// Create a new float and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_float(value: f64) {
    let mut rt = RT.lock().unwrap();
    Number::Float(value).load_to(&mut rt).unwrap()
}

/// Create a new environment
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_env(name: *const u8, outer: usize) -> usize {
    let mut rt = RT.lock().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.new_env(name_str.to_string(), outer)
    } else {
        panic!("Error in rt_define: invalid string")
    }
}

/// Get current environment
#[unsafe(no_mangle)]
pub extern "C" fn rt_current_env() -> usize {
    let rt = RT.lock().unwrap();
    rt.current_env()
}

/// Move to other environment
#[unsafe(no_mangle)]
pub extern "C" fn rt_move_to_env(env: usize) {
    let mut rt = RT.lock().unwrap();
    rt.move_to_env(env);
}

/// `define` keyword.
#[unsafe(no_mangle)]
pub extern "C" fn rt_define(key: *const u8, value: usize) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(key as *const i8) };
    let mut env = rt_current_env();
    if let Ok(key_str) = c_str.to_str() {
        env.define(&key_str.to_string(), value, &mut RT.lock().unwrap());
    } else {
        log_error("Error in rt_define: invalid string");
    }
}
/// `set!` keyword.
#[unsafe(no_mangle)]
pub extern "C" fn rt_set(key: *const u8, value: usize) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(key as *const i8) };
    let mut env = rt_current_env();
    if let Ok(key_str) = c_str.to_str() {
        env.set(&key_str.to_string(), value, &mut RT.lock().unwrap());
    } else {
        log_error("Error in rt_set: invalid string");
    }
}
/// `get` keyword.
#[unsafe(no_mangle)]
pub extern "C" fn rt_get(key: *const u8) -> usize {
    let c_str = unsafe { std::ffi::CStr::from_ptr(key as *const i8) };
    let env = rt_current_env();
    if let Ok(key_str) = c_str.to_str() {
        env.get(&key_str.to_string(), &RT.lock().unwrap()).unwrap()
    } else {
        log_error("Error in rt_get: invalid string");
        0
    }
}

/// Set the car of a pair
#[unsafe(no_mangle)]
pub extern "C" fn rt_set_car(index: usize, target: usize) -> usize {
    let mut rt = RT.lock().unwrap();
    match rt.set_car(true, index, target) {
        Ok(()) => index,
        Err(e) => {
            log_error(&format!("Error in rt_set_car: {e}"));
            0
        }
    }
}

/// Set the cdr of a pair
#[unsafe(no_mangle)]
pub extern "C" fn rt_set_cdr(index: usize, target: usize) -> usize {
    let mut rt = RT.lock().unwrap();
    match rt.set_cdr(true, index, target) {
        Ok(()) => index,
        Err(e) => {
            log_error(&format!("Error in rt_set_cdr: {e}"));
            0
        }
    }
}

/// Get the number value
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_integer(index: usize) -> i64 {
    let rt = RT.lock().unwrap();
    match rt.get_number(index) {
        Ok(Number::Int(val)) => val,
        Ok(_) => {
            log_error("Error in rt_get_integer: expected integer number");
            0
        }
        Err(e) => {
            log_error(&format!("Error in rt_get_integer: {e}"));
            0
        }
    }
}

/// Get the float value
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_float(index: usize) -> f64 {
    let rt = RT.lock().unwrap();
    match rt.get_number(index) {
        Ok(Number::Float(val)) => val,
        Ok(_) => {
            log_error("Error in rt_get_float: expected float number");
            0.0
        }
        Err(e) => {
            log_error(&format!("Error in rt_get_float: {e}"));
            0.0
        }
    }
}

/// Get the symbol value
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_symbol(index: usize) -> *mut i8 {
    let rt = RT.lock().unwrap();
    match rt.get_symbol(index) {
        Ok(sym) => {
            let bytes = format!("{sym}").into_bytes();
            let c_str = std::ffi::CString::new(bytes).unwrap();
            c_str.into_raw()
        }
        Err(e) => {
            log_error(&format!("Error in rt_get_symbol: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get the boolean value
/// Returns 1 if the symbol is not nil, 0 if it is nil.
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_bool(index: usize) -> i32 {
    let rt = RT.lock().unwrap();
    if let Ok(Symbol::Nil) = rt.get_symbol(index) {
        0
    } else {
        1
    }
}

/// Add a root variable
#[unsafe(no_mangle)]
pub extern "C" fn rt_add_root(name: *const u8, value: usize) -> usize {
    let mut rt = RT.lock().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.add_root(name_str.to_string(), value);
        1
    } else {
        log_error("Error in rt_set_root: invalid string");
        0
    }
}

/// Set a root variable
#[unsafe(no_mangle)]
pub extern "C" fn rt_set_root(name: *const u8, value: usize) -> usize {
    let mut rt = RT.lock().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.set_root(name_str.to_string(), value);
        1
    } else {
        log_error("Error in rt_set_root: invalid string");
        0
    }
}

/// Get the root variable value
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_root(name: *const u8) -> usize {
    let rt = RT.lock().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.get_root(name_str)
    } else {
        log_error("Error in rt_get_root: invalid string");
        0
    }
}

/// Check if a node is a symbol
#[unsafe(no_mangle)]
pub extern "C" fn rt_is_symbol(index: usize) -> i32 {
    let rt = RT.lock().unwrap();
    if rt.get_symbol(index).is_ok() { 1 } else { 0 }
}

/// The function called by [rt_import]. It is designed not to be a method of
/// [Runtime] to avoid deadlock when loading the package.
#[cfg(not(target_arch = "wasm32"))]
fn import(package: &str) -> Result<(), String> {
    {
        let runtime = RT.lock().unwrap();
        if runtime.has_package(package) {
            return Ok(());
        }
    } // unlock RT

    // Find the library and call the function with name `package`, then the
    // function will load everything into current environment.
    let lib = unsafe {
        let lib = Library::new(format!("./lib/{package}.relic")).map_err(|e| e.to_string())?;

        let main: libloading::Symbol<unsafe extern "C" fn() -> i32> = lib
            .get(&package.to_string().into_bytes())
            .map_err(|e| e.to_string())?;

        let return_val = main();

        if return_val == 0 {
            Ok(lib)
        } else {
            Err(format!(
                "Package {package}'s main function returns {return_val}"
            ))
        }
    }?;

    // Add the library to the runtime so it won't be unloaded.
    let mut runtime = RT.lock().unwrap();
    runtime.add_package(package.to_string(), lib);
    Ok(())
}

#[cfg(target_arch = "wasm32")]
fn import(_package: &str) -> Result<(), String> {
    Err("Package imports are not supported in WebAssembly".to_string())
}

/// Import a package.
#[unsafe(no_mangle)]
pub extern "C" fn rt_import(name: *const u8) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        unwrap_result(import(name_str), ());
    } else {
        log_error("Error in rt_import: invalid string");
    }
}
