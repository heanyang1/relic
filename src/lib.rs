pub mod compile;
pub mod env;
pub mod error;
pub mod lexer;
pub mod logger;
pub mod node;
pub mod package;
pub mod parser;
pub mod preprocess;
pub mod runtime;
pub mod symbol;
mod util;
use std::sync::{LazyLock, RwLock};

use crate::{
    env::Env,
    error::ParseError,
    lexer::Number,
    logger::log_warning,
    node::Node,
    package::load_package,
    runtime::{Closure, LoadToRuntime, Runtime, RuntimeNode, StackMachine},
    symbol::Symbol,
    util::CVoidFunc,
};

pub fn unwrap_result<T, E>(result: Result<T, E>, rt: &mut Runtime) -> T
where
    E: ToString,
{
    match result {
        Ok(x) => x,
        Err(msg) => {
            rt.error(&msg.to_string());
            std::process::abort()
        }
    }
}

pub fn run_node(node: Node) -> Result<String, String> {
    node.jit_compile(false)?;
    let mut runtime = RT.write().unwrap();
    let index = runtime.pop();
    Ok(runtime.display_node_idx(index))
}

/// The runtime that is pointed by all C bindings.
pub static RT: LazyLock<RwLock<Runtime>> = LazyLock::new(|| RwLock::new(Runtime::new(1)));

/// Calls [Runtime::top_env].
#[unsafe(no_mangle)]
pub extern "C" fn rt_start() {
    let mut rt = RT.write().unwrap();
    rt.top_env();
}

/// Calls [Runtime::add_root].
#[unsafe(no_mangle)]
pub extern "C" fn rt_add_root(name: *const u8, value: usize) -> usize {
    let mut rt = RT.write().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.add_root(name_str.to_string(), value);
        1
    } else {
        rt.error("Error in rt_set_root: invalid string");
        0
    }
}

/// Calls [Runtime::set_root].
#[unsafe(no_mangle)]
pub extern "C" fn rt_set_root(name: *const u8, value: usize) -> usize {
    let mut rt = RT.write().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.set_root(name_str.to_string(), value);
        1
    } else {
        rt.error("Error in rt_set_root: invalid string");
        0
    }
}

/// Calls [Runtime::get_root].
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_root(name: *const u8) -> usize {
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        let rt = RT.read().unwrap();
        rt.get_root(name_str)
    } else {
        let mut rt = RT.write().unwrap();
        rt.error("Error in rt_get_root: invalid string");
        0
    }
}

/// Calls [Runtime::remove_root].
#[unsafe(no_mangle)]
pub extern "C" fn rt_remove_root(name: *const u8) -> usize {
    let mut rt = RT.write().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.remove_root(name_str)
    } else {
        rt.error("Error in rt_remove_root: invalid string");
        0
    }
}

/// Calls [Closure::new] and pushes the result to the stack.
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_closure(name: *const u8, func: CVoidFunc, nargs: usize, variadic: bool) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    let mut rt = RT.write().unwrap();
    if let Ok(name) = c_str.to_str() {
        rt.api_called(format!(
            "rt_new_closure({name}, <func>, {nargs}, {variadic})"
        ));
        rt.try_gc();

        let val = Closure::new(name.to_string(), func, nargs, variadic, &rt);
        val.load_to(&mut rt).unwrap();
    } else {
        rt.error("Error in rt_remove_root: invalid string");
    }
}

/// Calls [Runtime::get_c_func].
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_c_func(cid: usize) -> Option<CVoidFunc> {
    let mut runtime = RT.write().unwrap();
    runtime.api_called(format!("rt_get_c_func({cid})"));
    unwrap_result(runtime.get_c_func(cid), &mut runtime)
}

/// Calls [Runtime::list_to_stack].
#[unsafe(no_mangle)]
pub extern "C" fn rt_list_to_stack() {
    let mut runtime = RT.write().unwrap();
    runtime.api_called("rt_list_to_stack()");
    unwrap_result(runtime.list_to_stack(), &mut runtime);
}

/// Calls [Runtime::prepare_args].
#[unsafe(no_mangle)]
pub extern "C" fn rt_prepare_args(cid: usize) {
    let mut runtime = RT.write().unwrap();
    runtime.api_called(format!("rt_prepare_args({cid})"));
    unwrap_result(runtime.prepare_args(cid), &mut runtime);
}

/// Calls [Runtime::push].
#[unsafe(no_mangle)]
pub extern "C" fn rt_push(index: usize) {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_push({index})"));
    rt.push(index);
}

/// Calls [Runtime::pop].
#[unsafe(no_mangle)]
pub extern "C" fn rt_pop() -> usize {
    let mut rt = RT.write().unwrap();
    rt.api_called("rt_pop()");
    rt.pop()
}

/// Calls [Runtime::swap].
#[unsafe(no_mangle)]
pub extern "C" fn rt_swap() {
    let mut rt = RT.write().unwrap();
    rt.api_called("rt_swap()");
    rt.swap()
}

/// Calls [Runtime::top].
#[unsafe(no_mangle)]
pub extern "C" fn rt_top() -> usize {
    let mut rt = RT.write().unwrap();
    rt.api_called("rt_top()");
    rt.top()
}

/// Calls [Runtime::display_node_idx].
#[unsafe(no_mangle)]
pub extern "C" fn rt_display_node_idx(index: usize) -> *mut i8 {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_display_node_idx({index})"));
    let result = rt.display_node_idx(index);
    let c_str = std::ffi::CString::new(result).unwrap();
    c_str.into_raw()
}

/// Calls [Runtime::apply].
#[unsafe(no_mangle)]
pub extern "C" fn rt_apply() -> usize {
    let mut rt = RT.write().unwrap();
    rt.api_called("rt_apply()".to_string());
    match rt.apply() {
        Ok(()) => 1,
        Err(e) => {
            rt.error(&format!("Error in rt_apply: {e}"));
            0
        }
    }
}

/// The `(read)` special form.
#[unsafe(no_mangle)]
pub extern "C" fn rt_read() {
    let mut rt = RT.write().unwrap();
    rt.api_called("rt_read()");
    let mut input = String::new();
    loop {
        let mut current = String::new();
        std::io::stdin().read_line(&mut current).unwrap();
        input.push_str(&current);
        match input.load_to(&mut rt) {
            Ok(()) => break,
            Err(ParseError::EOF) => {
                continue;
            }
            Err(e) => {
                rt.error(&format!("Error in rt_read: {e}"));
                RuntimeNode::Symbol(Symbol::Nil).load_to(&mut rt).unwrap();
                break;
            }
        }
    }
}

/// Parse an expression from a string and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_constant(expr: *const u8) {
    let mut rt = RT.write().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(expr as *const i8) };
    if let Ok(expr_str) = c_str.to_str() {
        rt.api_called(format!("rt_new_constant({expr_str})"));
        unwrap_result(expr_str.load_to(&mut rt), &mut rt);
    } else {
        rt.error("Error in rt_new_constant: invalid string");
    }
}

/// Create a new symbol and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_symbol(name: *const u8) {
    let mut rt = RT.write().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.api_called(format!("rt_new_symbol({name_str})"));
        unwrap_result(Symbol::from(name_str).load_to(&mut rt), &mut rt);
    } else {
        rt.error("Error in rt_new_symbol: invalid string");
    }
}

/// Create a new number and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_integer(value: i64) {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_new_integer({value})"));
    Number::Int(value).load_to(&mut rt).unwrap()
}

/// Create a new float and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_float(value: f64) {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_new_float({value})"));
    Number::Float(value).load_to(&mut rt).unwrap()
}

/// Calls [Runtime::current_env].
#[unsafe(no_mangle)]
pub extern "C" fn rt_current_env() -> usize {
    let mut rt = RT.write().unwrap();
    rt.api_called("rt_current_env()");
    rt.current_env()
}

/// Calls [Runtime::move_to_env].
#[unsafe(no_mangle)]
pub extern "C" fn rt_move_to_env(env: usize) {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_move_to_env({env})"));
    rt.move_to_env(env);
}

/// Calls [Env::define].
#[unsafe(no_mangle)]
pub extern "C" fn rt_define(key: *const u8, value: usize) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(key as *const i8) };
    let mut env = rt_current_env();
    if let Ok(key_str) = c_str.to_str() {
        RT.write()
            .unwrap()
            .api_called(format!("rt_define({key_str}, {value})"));
        env.define(&key_str.to_string(), value, &mut RT.write().unwrap());
    } else {
        RT.write()
            .unwrap()
            .error("Error in rt_define: invalid string");
    }
}
/// Calls [Env::set].
#[unsafe(no_mangle)]
pub extern "C" fn rt_set(key: *const u8, value: usize) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(key as *const i8) };
    let mut env = rt_current_env();
    if let Ok(key_str) = c_str.to_str() {
        RT.write()
            .unwrap()
            .api_called(format!("rt_set({key_str}, {value})"));
        if env
            .set(&key_str.to_string(), value, &mut RT.write().unwrap())
            .is_none()
        {
            RT.write()
                .unwrap()
                .error(&format!("Error in rt_set: variable {key_str} not found"));
        }
    } else {
        RT.write().unwrap().error("Error in rt_set: invalid string");
    }
}
/// Calls [Env::get].
#[unsafe(no_mangle)]
pub extern "C" fn rt_get(key: *const u8) -> usize {
    let c_str = unsafe { std::ffi::CStr::from_ptr(key as *const i8) };
    let env = rt_current_env();
    if let Ok(key_str) = c_str.to_str() {
        RT.write().unwrap().api_called(format!("rt_get({key_str})"));
        let mut runtime = RT.write().unwrap();
        match env.get(&key_str.to_string(), &runtime) {
            Some(val) => val,
            None => {
                log_warning(format!(
                    "Error in rt_get: variable {key_str} not found, returning nil"
                ));
                runtime.new_node_with_gc(RuntimeNode::Symbol(Symbol::Nil))
            }
        }
    } else {
        RT.write().unwrap().error("Error in rt_get: invalid string");
        0
    }
}

/// Calls [Runtime::set_car].
#[unsafe(no_mangle)]
pub extern "C" fn rt_set_car(index: usize, target: usize) -> usize {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_set_car({index}, {target})"));
    match rt.set_car(true, index, target) {
        Ok(()) => index,
        Err(e) => {
            rt.error(&format!("Error in rt_set_car: {e}"));
            0
        }
    }
}

/// Calls [Runtime::set_cdr].
#[unsafe(no_mangle)]
pub extern "C" fn rt_set_cdr(index: usize, target: usize) -> usize {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_set_cdr({index}, {target})"));
    match rt.set_cdr(true, index, target) {
        Ok(()) => index,
        Err(e) => {
            rt.error(&format!("Error in rt_set_cdr: {e}"));
            0
        }
    }
}

/// Get the integer value
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_integer(index: usize) -> i64 {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_get_integer({index})"));
    match rt.get_number(index) {
        Ok(Number::Int(val)) => val,
        Ok(_) => {
            rt.error("Error in rt_get_integer: expected integer number");
            0
        }
        Err(e) => {
            rt.error(&format!("Error in rt_get_integer: {e}"));
            0
        }
    }
}

/// Get the float value
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_float(index: usize) -> f64 {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_get_float({index})"));
    match rt.get_number(index) {
        Ok(Number::Float(val)) => val,
        Ok(_) => {
            rt.error("Error in rt_get_float: expected float number");
            0.0
        }
        Err(e) => {
            rt.error(&format!("Error in rt_get_float: {e}"));
            0.0
        }
    }
}

/// Get the symbol value
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_symbol(index: usize) -> *mut i8 {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_get_symbol({index})"));
    match rt.get_symbol(index) {
        Ok(sym) => {
            let bytes = format!("{sym}").into_bytes();
            let c_str = std::ffi::CString::new(bytes).unwrap();
            c_str.into_raw()
        }
        Err(e) => {
            rt.error(&format!("Error in rt_get_symbol: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get the boolean value
/// Returns 1 if the symbol is not nil, 0 if it is nil.
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_bool(index: usize) -> i32 {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_get_bool({index})"));
    if let Ok(Symbol::Nil) = rt.get_symbol(index) {
        0
    } else {
        1
    }
}

/// Checks if a node is a symbol.
///
/// Returns 1 if the node is a symbol, 0 otherwise.
#[unsafe(no_mangle)]
pub extern "C" fn rt_is_symbol(index: usize) -> i32 {
    let mut rt = RT.write().unwrap();
    rt.api_called(format!("rt_is_symbol({index})"));
    if rt.get_symbol(index).is_ok() { 1 } else { 0 }
}

/// Import a package.
#[unsafe(no_mangle)]
pub extern "C" fn rt_import(name: *const u8) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        RT.write()
            .unwrap()
            .api_called(format!("rt_import({name_str})"));
        if RT.read().unwrap().has_package(name_str) {
            return;
        }
        unwrap_result(load_package(name_str), &mut RT.write().unwrap());
    } else {
        RT.write()
            .unwrap()
            .error("Error in rt_import: invalid string");
    }
}

/// Calls [Runtime::breakpoint].
#[unsafe(no_mangle)]
pub extern "C" fn rt_breakpoint() {
    RT.write().unwrap().breakpoint();
}

/// Calls [Runtime::evaluated].
#[unsafe(no_mangle)]
pub extern "C" fn rt_evaluated(info: *const u8, optimized: i32) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(info as *const i8) };
    let mut rt = RT.write().unwrap();
    if let Ok(info) = c_str.to_str() {
        rt.evaluated(info, optimized == 1);
    } else {
        rt.error("Error in rt_import: invalid string");
    }
}
