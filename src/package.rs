//! Functions related to loading packages and JIT compilation.
//!
//! This module provides functionality for:
//! 1. Loading packages (both compiled `.relic` and source `.lisp` files)
//! 2. JIT compilation from Lisp source to compiled shared library
//!
//! ## Package Loading
//!
//! Packages can be loaded from two sources:
//!
//! ### Binary Packages (.relic)
//!
//! Pre-compiled shared libraries (`.relic` files) in the `lib/` directory.
//! These are loaded using `libloading` and their initialization function is called.
//!
//! ### Source Packages (.lisp)
//!
//! Lisp source files (`.lisp`) in the `lib/` directory. These are JIT compiled
//! to C, then to a shared library, and loaded.
//!
//! ## Package Resolution Order
//!
//! When `(import name)` is called:
//! 1. Check if `lib/{name}.relic` exists -> load as binary
//! 2. Check if `lib/{name}.lisp` exists -> JIT compile and load
//! 3. Error if neither exists
//!
//! Once loaded, packages are cached in the runtime's package map. Subsequent
//! `(import name)` calls skip re-loading if the package is already cached.
//!
//! ## JIT Compilation
//!
//! The [`Node::jit_compile`] function performs JIT compilation:
//!
//! 1. **Create temp directory**: `/tmp/relic/` for intermediate files
//! 2. **Node to C**: Use [`CodeGen`] to generate C source code
//! 3. **C to SO**: Compile C with GCC/Clang to shared library
//! 4. **Load**: Use `libloading` to dynamically load the library
//! 5. **Initialize**: Call the package's initialization function
//!
//! ### GCC Command Flags
//!
//! - `-Ic_runtime`: Include path for runtime headers
//! - `-shared`: Create shared library
//! - `-fPIC`: Position-independent code
//! - `-O3`: High optimization
//! - `-g`: Debug symbols
//! - `-Wl,-undefined,dynamic_lookup` (macOS): Allow undefined symbols
//!
//! ## Binary Package Structure
//!
//! A binary package must export a function named after the package that:
//! 1. Creates closures with `rt_new_closure()`
//! 2. Defines them in the environment with `rt_define()`
//!
//! See [the wiki](https://github.com/heanyang1/relic/wiki/Create-and-Use-Relic-Packages)
//! for detailed instructions on creating packages from C or Lisp code.
//!
//! ## Thread Safety
//!
//! Package loading must not be called while holding the runtime lock (`RT`).
//! The functions in this module release the lock before loading.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    process::Command,
};

use libloading::{Library, Symbol};

use crate::{
    compile::{compile, CodeGen},
    lexer::LexerMonad,
    node::Node,
    parser::new_pair,
    preprocess::{Macro, PreProcess},
    symbol::SpecialForm,
    util::inc,
    RT,
};

/// Reads text from a file, parses and preprocesses it, then returns a node.
pub fn file_to_node(
    input_path: PathBuf,
    macros: &mut HashMap<String, Macro>,
) -> Result<LexerMonad<Node>, String> {
    // Add a `begin` in the beginning
    let nodes = LexerMonad::new(input_path)?
        .parse_all()
        .map_err(|e| e.to_string())?;
    new_pair(
        LexerMonad::from_other(
            Node::SpecialForm(SpecialForm::Begin),
            nodes.borrow().get_begin(),
        )
        .into(),
        nodes.clone(),
    )
    .preprocess(macros)
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
impl LexerMonad<Node> {
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
