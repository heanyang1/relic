# AGENTS.md - Relic Project Guidelines

Relic (Rust-Enabled LIsp Compiler) is a minimal Lisp system written in Rust that compiles to C and LLVM IR.

## Build, Lint, and Test Commands

### Building
```bash
cargo build              # Build the project
cargo build --release    # Release mode
cargo run -- --help      # CLI help
cargo run -- compile -i program.lisp -o program.c         # Compile to C
cargo run -- compile --backend llvm -i program.lisp -o program.ll  # Compile to LLVM IR
```

### Testing
```bash
cargo test                    # Run all tests
cargo test <test_name>        # Run single test by name
cargo test --test <filename>  # Run tests in file
cargo test -- --nocapture     # Show print output
cargo test -v                 # Verbose output
```

For C runtime tests: `cargo test` then `cd c_runtime && ./run_tests.py`

### Linting/Formatting
```bash
cargo fmt --check  # Check formatting
cargo fmt          # Format code
cargo clippy       # Run clippy
cargo clippy -- -D warnings  # All warnings as errors
```

### REPL
```bash
cargo run          # Start REPL
cargo run -- -d    # With debug logging
```

## Code Style

### Formatting
- **Indentation**: 4 spaces (Rust), 2 spaces (.lisp, C, YAML)
- **Line endings**: LF
- **Final newline**: Always

### Imports
Group imports from same crate with `use crate::{...}`:
```rust
use std::{collections::HashMap, fs::File, io::Write};
use rustyline::completion::{Completer, Pair};
use relic::{compile::compile, env::Env, error::ParseError};
```

### Naming
- Types: `PascalCase` (e.g., `LexerMonad`, `ParseError`)
- Functions/methods: `snake_case` (e.g., `next_token`)
- Variables: `snake_case` (e.g., `mut runtime`)
- Modules: `snake_case` (e.g., `pub mod lexer`)

### Error Handling
Use custom error types with `Debug`, `Display`, `Error`:
```rust
#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    SyntaxError(String),
    EOF,
}
impl Display for ParseError { fn fmt(&self, f: &mut Formatter) -> std::fmt::Result { ... } }
impl Error for ParseError {}
impl From<String> for ParseError { fn from(value: String) -> Self { ... } }
```
Use `Result<T, E>`, `?` operator, and `map_err`.

### Documentation
Use `///` for public API, `//!` for modules:
```rust
/// The lexer is a monad that stores location and source.
pub struct LexerMonad<T> { ... }
```

### Unsafe Code
FFI functions need `#[unsafe(no_mangle)]`:
```rust
#[unsafe(no_mangle)]
pub extern "C" fn rt_get_integer(index: usize) -> i64 { ... }
```

### Testing
- Tests in `tests/` directory
- Use `#[test]` attribute
- Use `serial_test` for serial tests: `#[test] #[serial] fn test_name() { ... }`
- Naming: `<feature>_<behavior>` (e.g., `param`, `numeric`)

## Common Patterns

- **Monads**: `LexerMonad<T>` pattern tracking file position
- **Runtime**: Global via `pub static RT: LazyLock<RwLock<Runtime>>`
- **C FFI**: Extensive bindings in `src/lib.rs`
- **Node system**: `RuntimeNode` enum for AST types
- **Traits**: `LoadToRuntime`, `MapErr` (see `src/error.rs`)

## Project Structure

```
src/
  lib.rs, main.rs, lexer.rs, parser.rs, compile.rs, compile_llvm.rs, runtime.rs
  error.rs, env.rs, node.rs, symbol.rs, number.rs, package.rs
  preprocess.rs, logger.rs, util.rs
test_driver/
  test_gdb_driver.c, Makefile, README.md, .gitignore
tests/
  lexer_test.rs, parser_test.rs, compile_test.rs
  runtime_test.rs, preprocess_test.rs
```

## Key Dependencies
`clap`, `rustyline`, `libloading`, `colored`, `serial_test`, `inkwell`

## Runtime Access
```rust
let mut rt = RT.write().unwrap();  // Write lock
let rt = RT.read().unwrap();         // Read lock
```

## Faster Development
```bash
cargo check        # Quick check without build
cargo build -j 1   # Sequential if parallel issues
```

## Common Development Tasks

### Adding a New Built-in Function
1. Implement the function in `src/runtime.rs` within the appropriate special form handler
2. If exposing to C, add FFI wrapper in `src/lib.rs` with `#[unsafe(no_mangle)]`
3. Add tests in `tests/runtime_test.rs`

### Adding a New Token
1. Add variant to `Token` enum in `src/lexer.rs`
2. Update token parsing logic in the lexer
3. Add tests in `tests/lexer_test.rs`

### Debugging
- Use `-d` flag with `cargo run` for debug logging
- Check `logger.rs` for available log levels: `log_debug`, `log_warning`, `log_error`
- Use `rt_breakpoint()` in compiled code for debugger integration
- To inspect generated LLVM IR, compile with `relic compile --backend llvm -i program.lisp -o program.ll`
- Generated C files are at `/tmp/relic/jit_*.c`
- **GDB debugging (LLVM backend)**: Use `-g` flag to emit DWARF debug symbols. Compile the `.ll` to a shared library, then use `test_driver/test_gdb_driver` to load it under GDB:
  ```bash
  relic compile --backend llvm -g -i program.lisp -o program.ll
  clang -shared -g -fPIC program.ll -L target/debug -lrelic \
      -Wl,-rpath,$(pwd)/target/debug -o program.relic
  cc -o test_driver/test_gdb_driver test_driver/test_gdb_driver.c -ldl
  gdb --args $(pwd)/test_driver/test_gdb_driver $(pwd)/program.relic
  (gdb) break program.lisp:5
  (gdb) run
  ```

### Adding LLVM Backend Support for a New Feature
1. If the feature affects code generation, update `src/compile.rs` (C backend) and `src/compile_llvm.rs` (LLVM backend)
2. The LLVM backend uses `inkwell` to build LLVM IR in-memory, then JIT-compiles it via `ExecutionEngine::get_function` / `JitFunction::call` (no file I/O or clang subprocess)
3. `LlvmCodeGen` mirrors `CodeGen` but generates LLVM IR instead of C strings
4. All runtime API calls (`rt_push`, `rt_pop`, `rt_apply`, etc.) are declared as `declare` in the LLVM module
5. String constants are deduplicated as global `@.str_N` arrays
6. Closures compile to separate LLVM functions (`func_{id}`) within the same module
7. Use `assert_eval_node_dual!` / `assert_eval_text_dual!` in tests to verify both backends produce identical results
8. For debug info support, see `init_debug_info()` and `create_function_di()` in `compile_llvm.rs`

### Fuzz Testing Both Backends
Run the Python fuzzer to compare C and LLVM/JIT backends on random Lisp expressions:
```bash
python3 scripts/fuzz_backends.py                       # 2000 random tests, both backends
python3 scripts/fuzz_backends.py --backend llvm        # JIT backend only
python3 scripts/fuzz_backends.py -n 5000               # More coverage
python3 scripts/fuzz_backends.py --seed 42             # Reproducible
python3 scripts/fuzz_backends.py --jobs 4 --backend llvm  # Parallel JIT fuzzing (safe)
```
The fuzzer defaults to `--jobs 1` because the C backend writes temp files under
`/tmp/relic/` that race under parallel invocations. The LLVM/JIT backend
uses in-process compilation (no temp files) so `--jobs >1 --backend llvm` is safe.

## Notes
- The project uses Rust edition 2024 (experimental)
- C code generation output requires linking with `librelic.so`
- LLVM IR generation requires LLVM 18 development libraries and clang
- The `.cargo/config.toml` sets `LLVM_SYS_180_PREFIX` for the LLVM installation path
- The compiler generates C code or LLVM IR, not standalone executables
- Both backends share the same runtime and produce identical results
- DWARF debug symbols are emitted by the LLVM backend when `-g` is passed. See `test_driver/README.md` for GDB usage.
