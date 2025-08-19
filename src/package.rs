//! Functions related to loading packages and JIT compilation

use std::{
    collections::HashMap,
    fs::read_to_string,
    path::{Path, PathBuf},
    process::Command,
    str::FromStr,
};

use libloading::{Library, Symbol};

use crate::{
    RT,
    compile::{CodeGen, compile},
    node::Node,
    preprocess::{Macro, PreProcess},
    util::inc,
};

/// Reads text from a file, parses and preprocesses it, then returns a node.
pub fn file_to_node(
    input_path: PathBuf,
    macros: &mut HashMap<String, Macro>,
) -> Result<Node, String> {
    let file = read_to_string(input_path).map_err(|e| e.to_string())?;
    let mut node = Node::from_str(&file)?;
    node.preprocess(macros)
}

/// Loads a package to the runtime.
/// 
/// A package with name `name` can be either a dynamic library `name.relic` or
/// a Lisp source file `name.lisp`.
///
/// This function can not be called when holding [RT].
pub fn load_package(name: &str) -> Result<(), String> {
    let binary_name = format!("./lib/{name}.relic");
    let text_name = format!("./lib/{name}.lisp");
    if Path::new(&binary_name).exists() {
        let lib = load_binary_library(&binary_name)?;
        add_package(lib, name)
    } else if Path::new(&text_name).exists() {
        let node = file_to_node(PathBuf::from(text_name), &mut HashMap::new())?;
        node.jit_compile(true)
    } else {
        Err(format!("library {name} not found"))
    }
}

/// Adds a package. `name` is the package name.
///
/// This function can not be called when holding [RT].
fn add_package(lib: Library, name: &str) -> Result<(), String> {
    call_library_fn(&lib, name)?;
    let mut runtime = RT.write().unwrap();
    runtime.add_package(name.to_string(), lib);
    Ok(())
}

/// Loads a binary library. `name` is the path of the library.
fn load_binary_library(name: &str) -> Result<Library, String> {
    unsafe { Library::new(name) }.map_err(|e| e.to_string())
}

/// Calls the main function of a library.
///
/// This function can not be called when holding [RT].
fn call_library_fn(lib: &Library, func_name: &str) -> Result<(), String> {
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
    pub fn jit_compile(&self, debug_info: bool) -> Result<(), String> {
        // make a directory for Relic runtime if it doesn't exist
        std::fs::create_dir_all("/tmp/relic").map_err(|e| e.to_string())?;

        let lib_name = format!("jit_{}", inc());
        let c_source_name = format!("/tmp/relic/{lib_name}.c");
        let lib_full_name = format!("/tmp/relic/{lib_name}.relic");

        // node -> .c
        let mut codegen = CodeGen::new_library(lib_name.to_string());
        compile(self, &mut codegen, debug_info)?;
        let c_code = codegen.to_string();
        std::fs::write(&c_source_name, c_code).map_err(|e| e.to_string())?;

        // .c -> .relic
        let status = Command::new("gcc")
            .args([
                "-Ic_runtime",
                "-shared",
                "-fPIC",
                "-O3",
                "-g",
                "-o",
                &lib_full_name,
                &c_source_name,
                #[cfg(target_os = "macos")]
                "-Wl,-undefined,dynamic_lookup",
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

        let lib = load_binary_library(&lib_full_name)?;
        add_package(lib, &lib_name)
    }
}
