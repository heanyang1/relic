pub mod compile;
mod env;
pub mod lexer;
pub mod logger;
pub mod node;
pub mod package;
pub mod parser;
pub mod preprocess;
pub mod runtime;
pub mod symbol;
mod util;
use std::sync::{LazyLock, Mutex};

use crate::{
    env::Env,
    lexer::Number,
    logger::{log_error, unwrap_result},
    package::{call_library_fn, load_library},
    runtime::{Closure, LoadToRuntime, Runtime, RuntimeNode, StackMachine},
    symbol::Symbol,
    util::CVoidFunc,
};

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
    rt.api_called(format!("rt_new_closure({id}, <func>, {nargs}, {variadic})"));
    rt.try_gc();

    let val = Closure::new(id, func, nargs, variadic, &rt);
    val.load_to(&mut rt).unwrap();
}

/// The tail-recursive version of [call_closure].
fn tail_call_closure(nparams: usize) -> Result<(), String> {
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
                runtime.zip_stack_nodes(nparams - c.nargs + 1);
            }

            // Add the last argument.
            let last = runtime.pop();
            runtime.current_env().define(
                &format!("#{}_func_{}", c.nargs - 1, c.id),
                last,
                &mut runtime,
            );
        }
        c
    }; // unlocks runtime

    // Call function in the environment. Locks runtime.
    (c.body)();
    Ok(())
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
                runtime.zip_stack_nodes(nparams - c.nargs + 1);
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
    {
        let mut runtime = RT.lock().unwrap();
        runtime.api_called(format!("rt_call_closure({nparams})"));
    }
    if call_closure(nparams).is_ok() {
        1
    } else {
        log_error("Error in rt_call_closure: invalid parameters");
        0
    }
}
/// Calls a closure.
///
/// This is the tail-recursive version of `rt_call_closure`.
/// TODO: Actually it is not tail recursive, we should move everything into C
/// and make the C compiler optimize the code so it can be truly tail-recursive.
#[unsafe(no_mangle)]
pub extern "C" fn rt_tail_call_closure(nparams: usize) -> i32 {
    {
        let mut runtime = RT.lock().unwrap();
        runtime.api_called(format!("rt_tail_call_closure({nparams})"));
    }
    if tail_call_closure(nparams).is_ok() {
        1
    } else {
        log_error("Error in rt_tail_call_closure: invalid parameters");
        0
    }
}

/// Push a node index onto the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_push(index: usize) {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_push({index})"));
    rt.push(index);
}

/// Pop a node index from the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_pop() -> usize {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_pop()"));
    rt.pop()
}

/// Get the stack top
#[unsafe(no_mangle)]
pub extern "C" fn rt_top() -> usize {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_top()"));
    rt.top()
}

/// Display a node by index as string
#[unsafe(no_mangle)]
pub extern "C" fn rt_display_node_idx(index: usize) -> *mut i8 {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_display_node_idx({index})"));
    let result = rt.display_node_idx(index);
    let c_str = std::ffi::CString::new(result).unwrap();
    c_str.into_raw()
}

/// Evaluate an expression and push the result onto the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_apply(nargs: usize) -> usize {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_apply({nargs})"));
    match rt.apply(nargs) {
        Ok(()) => 1,
        Err(e) => {
            log_error(format!("Error in rt_apply: {e}"));
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
        rt.api_called(format!("rt_new_constant({expr_str})"));
        unwrap_result(expr_str.load_to(&mut rt), ());
    } else {
        log_error("Error in rt_new_constant: invalid string");
    }
}

/// Create a new symbol and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_symbol(name: *const u8) {
    let mut rt = RT.lock().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.api_called(format!("rt_new_symbol({name_str})"));
        unwrap_result(Symbol::from(name_str.to_string()).load_to(&mut rt), ());
    } else {
        log_error("Error in rt_new_symbol: invalid string");
    }
}

/// Create a new number and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_integer(value: i64) {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_new_integer({value})"));
    Number::Int(value).load_to(&mut rt).unwrap()
}

/// Create a new float and push the result to the stack
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_float(value: f64) {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_new_float({value})"));
    Number::Float(value).load_to(&mut rt).unwrap()
}

/// Create a new environment
#[unsafe(no_mangle)]
pub extern "C" fn rt_new_env(name: *const u8, outer: usize) -> usize {
    let mut rt = RT.lock().unwrap();
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        rt.api_called(format!("rt_new_env({name_str}, {outer})"));
        rt.new_env(name_str.to_string(), outer)
    } else {
        log_error("Error in rt_new_env: invalid string");
        0
    }
}

/// Get current environment
#[unsafe(no_mangle)]
pub extern "C" fn rt_current_env() -> usize {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_current_env()"));
    rt.current_env()
}

/// Move to other environment
#[unsafe(no_mangle)]
pub extern "C" fn rt_move_to_env(env: usize) {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_move_to_env({env})"));
    rt.move_to_env(env);
}

/// `define` keyword.
#[unsafe(no_mangle)]
pub extern "C" fn rt_define(key: *const u8, value: usize) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(key as *const i8) };
    let mut env = rt_current_env();
    if let Ok(key_str) = c_str.to_str() {
        {
            let mut rt = RT.lock().unwrap();
            rt.api_called(format!("rt_define({key_str}, {value})"));
        }
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
        {
            let mut rt = RT.lock().unwrap();
            rt.api_called(format!("rt_set({key_str}, {value})"));
        }
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
        {
            let mut rt = RT.lock().unwrap();
            rt.api_called(format!("rt_get({key_str})"));
        }
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
    rt.api_called(format!("rt_set_car({index}, {target})"));
    match rt.set_car(true, index, target) {
        Ok(()) => index,
        Err(e) => {
            log_error(format!("Error in rt_set_car: {e}"));
            0
        }
    }
}

/// Set the cdr of a pair
#[unsafe(no_mangle)]
pub extern "C" fn rt_set_cdr(index: usize, target: usize) -> usize {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_set_cdr({index}, {target})"));
    match rt.set_cdr(true, index, target) {
        Ok(()) => index,
        Err(e) => {
            log_error(format!("Error in rt_set_cdr: {e}"));
            0
        }
    }
}

/// Get the number value
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_integer(index: usize) -> i64 {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_get_integer({index})"));
    match rt.get_number(index) {
        Ok(Number::Int(val)) => val,
        Ok(_) => {
            log_error("Error in rt_get_integer: expected integer number");
            0
        }
        Err(e) => {
            log_error(format!("Error in rt_get_integer: {e}"));
            0
        }
    }
}

/// Get the float value
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_float(index: usize) -> f64 {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_get_float({index})"));
    match rt.get_number(index) {
        Ok(Number::Float(val)) => val,
        Ok(_) => {
            log_error("Error in rt_get_float: expected float number");
            0.0
        }
        Err(e) => {
            log_error(format!("Error in rt_get_float: {e}"));
            0.0
        }
    }
}

/// Get the symbol value
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_symbol(index: usize) -> *mut i8 {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_get_symbol({index})"));
    match rt.get_symbol(index) {
        Ok(sym) => {
            let bytes = format!("{sym}").into_bytes();
            let c_str = std::ffi::CString::new(bytes).unwrap();
            c_str.into_raw()
        }
        Err(e) => {
            log_error(format!("Error in rt_get_symbol: {e}"));
            std::ptr::null_mut()
        }
    }
}

/// Get the boolean value
/// Returns 1 if the symbol is not nil, 0 if it is nil.
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_bool(index: usize) -> i32 {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_get_bool({index})"));
    if let Ok(Symbol::Nil) = rt.get_symbol(index) {
        0
    } else {
        1
    }
}

/// Check if a node is a symbol
#[unsafe(no_mangle)]
pub extern "C" fn rt_is_symbol(index: usize) -> i32 {
    let mut rt = RT.lock().unwrap();
    rt.api_called(format!("rt_is_symbol({index})"));
    if rt.get_symbol(index).is_ok() { 1 } else { 0 }
}

/// Import a package.
#[unsafe(no_mangle)]
pub extern "C" fn rt_import(name: *const u8) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(name as *const i8) };
    if let Ok(name_str) = c_str.to_str() {
        {
            let mut rt = RT.lock().unwrap();
            rt.api_called(format!("rt_import({name_str})"));
        }
        if RT.lock().unwrap().has_package(name_str) {
            return;
        }
        let lib =
            load_library(&format!("./lib/{name_str}.relic")).expect("error importing package");
        unwrap_result(call_library_fn(&lib, name_str), ());
        let mut runtime = RT.lock().unwrap();
        runtime.add_package(name_str.to_string(), lib);
    } else {
        log_error("Error in rt_import: invalid string");
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn rt_breakpoint() {
    RT.lock().unwrap().breakpoint();
}

/// This statement is inserted by the compiler as debug information.
#[unsafe(no_mangle)]
pub extern "C" fn rt_evaluated(info: *const u8) {
    let c_str = unsafe { std::ffi::CStr::from_ptr(info as *const i8) };
    if let Ok(info) = c_str.to_str() {
        RT.lock().unwrap().evaluated(info);
    } else {
        log_error("Error in rt_import: invalid string");
    }
}
