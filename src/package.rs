//! Functions related to loading packages and JIT compilation

use std::{process::Command};

use libloading::{Library, Symbol};

use crate::{
    compile::{compile, CodeGen}, node::Node, util::inc, RT
};

pub fn load_library(name: &str) -> Result<Library, String> {
    unsafe { Library::new(name) }.map_err(|e| e.to_string())
}

/// Get a C function from a library.
///
/// This function can not be called when holding [RT].
pub fn call_library_fn(lib: &Library, func_name: &str) -> Result<(), String> {
    unsafe {
        let func: Symbol<unsafe extern "C" fn() -> i32> = lib
            .get(&func_name.to_string().into_bytes())
            .map_err(|e| e.to_string())?;
        let ret_val = func();
        if ret_val == 0 {
            Ok(())
        } else {
            Err(format!("function {func_name} returns {ret_val}"))
        }
    }
}

/// JIT compile a pre-processed, compile-time node, and load it to the static runtime.
/// It has the same effect as evaluating the node at top-level.
///
/// This function can not be called when holding [RT].
impl Node {
    pub fn jit_compile(&self) -> Result<(), String> {
        // make a directory for Relic runtime if it doesn't exist
        std::fs::create_dir_all("/tmp/relic").map_err(|e| e.to_string())?;

        let lib_name = format!("jit_{}", inc());
        let c_source_name = format!("/tmp/relic/{}.c", lib_name);
        let lib_full_name = format!("/tmp/relic/{}.relic", lib_name);

        // node -> .c
        let mut codegen = CodeGen::new_library(lib_name.to_string());
        compile(&self, &mut codegen)?;
        let c_code = codegen.to_string();
        std::fs::write(&c_source_name, c_code).map_err(|e| e.to_string())?;

        // .c -> .relic
        let status = Command::new("gcc")
            .args([
                "-Ic_runtime",
                "-shared",
                "-fPIC",
                "-O3",
                "-o",
                &lib_full_name,
                &c_source_name,
            ])
            .spawn()
            .map_err(|e| e.to_string())?
            .wait()
            .map_err(|e| e.to_string())?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("compilation failed with status {status}"))
        }?;

        // load library
        let lib = load_library(&lib_full_name)?;
        call_library_fn(&lib, &lib_name)?;
        let mut runtime = RT.lock().unwrap();
        runtime.add_package(lib_name, lib);

        Ok(())
    }
}
