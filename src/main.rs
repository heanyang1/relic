//! Relic - A minimal Lisp compiler and interpreter.
//!
//! This binary provides:
//! - REPL mode for interactive evaluation
//! - Run mode for executing files
//! - Compile mode for generating C code
//! - Debug mode for debugging Lisp programs
//!
//! ## Usage
//!
//! ```bash
//! relic run -i program.lisp    # Run a Lisp program
//! relic compile -i program.lisp -o program.c  # Compile to C
//! relic repl                   # Start REPL
//! relic debug -i program.lisp  # Debug mode
//! ```

use std::{collections::HashMap, fs::File, io::Write, path::PathBuf, sync::Arc};

use rustyline::Context;
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::history::FileHistory;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use rustyline::{Editor, error::ReadlineError};

use relic::{
    RT,
    compile::{CodeGen, compile},
    compile_llvm::{self, LlvmCodeGen},
    error::ParseError,
    lexer::LexerMonad,
    logger::log_error,
    package::file_to_node,
    preprocess::PreProcess,
    rt_start, run_node, run_node_llvm,

    unwrap_result,
};

use clap::{Parser, ValueEnum};

/// Autocomplete provider for the REPL.
///
/// Provides tab completion for Lisp symbols and special forms.
pub struct RelicCompleter {
    pub candidates: Arc<Vec<String>>,
}

// Implement Helper as a marker trait
impl rustyline::Helper for RelicCompleter {}

// Implement Hinter as a no-op
impl rustyline::hint::Hinter for RelicCompleter {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        None
    }
}

// Implement Highlighter as a no-op
impl Highlighter for RelicCompleter {}

// Implement Validator as always valid
impl Validator for RelicCompleter {
    fn validate(
        &self,
        _ctx: &mut ValidationContext,
    ) -> Result<ValidationResult, rustyline::error::ReadlineError> {
        Ok(ValidationResult::Valid(None))
    }
}

impl Completer for RelicCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), rustyline::error::ReadlineError> {
        let start = line[..pos]
            .rfind(|c: char| c.is_whitespace() || c == '(')
            .map_or(0, |i| i + 1);
        let word = &line[start..pos];
        let matches = self
            .candidates
            .iter()
            .filter(|s| s.starts_with(word))
            .map(|s| Pair {
                display: s.clone(),
                replacement: s.clone(),
            })
            .collect();
        Ok((start, matches))
    }
}

/// Program execution modes.
#[derive(Debug, Clone, ValueEnum)]
enum Mode {
    /// Runs a REPL. If there is an input file, interprets it and modifies
    /// the environment.
    Repl,
    /// Runs the file and exit.
    Run,
    /// Compiles the input file to C code and write to output file.
    /// If the output file is not specified, print the code to stdout.
    Compile,
}

/// Compilation backend.
#[derive(Debug, Clone, ValueEnum)]
enum Backend {
    /// Generate C code (default).
    C,
    /// Generate LLVM IR code.
    Llvm,
}

/// Command-line arguments for Relic.
#[derive(Parser)]
struct Cli {
    /// Program mode.
    #[arg(value_enum)]
    mode: Mode,

    /// Input file path.
    #[arg(short, long, value_name = "FILE")]
    input_path: Option<PathBuf>,

    /// Output file path.
    #[arg(short, long, value_name = "FILE")]
    output_path: Option<PathBuf>,

    /// The name of the package.
    ///
    /// You can create a package from your code by compiling it to a shared
    /// library, move it to `lib` folder and call `(import [package name])`
    /// to use it in lisp code. See `lib/README.md` for details.
    ///
    /// The package name must be a valid variable name.
    #[arg(short, long, value_name = "NAME")]
    package_name: Option<String>,

    /// Whether to add debug information when compiling.
    #[arg(short = 'g')]
    debug_info: bool,

    /// Compilation backend to use.
    #[arg(long, value_enum, default_value = "c")]
    backend: Backend,
}

fn main() {
    let cli = Cli::parse();

    let mut macros = HashMap::new();
    let input_node = cli
        .input_path
        .map(|path| unwrap_result(file_to_node(path, &mut macros), &mut RT.write().unwrap()));

    match cli.mode {
        Mode::Run => {
            rt_start();
            if let Some(node) = input_node {
                let result = match cli.backend {
                    Backend::Llvm => unwrap_result(run_node_llvm(node), &mut RT.write().unwrap()),
                    Backend::C => unwrap_result(run_node(node), &mut RT.write().unwrap()),
                };
                println!("result: {result}");
            } else {
                eprintln!("No files to run");
            }
        }
        Mode::Repl => {
            rt_start();

            if let Some(node) = input_node {
                let result = match cli.backend {
                    Backend::Llvm => unwrap_result(run_node_llvm(node), &mut RT.write().unwrap()),
                    Backend::C => unwrap_result(run_node(node), &mut RT.write().unwrap()),
                };
                println!("result: {result}");
            }

            // Gather autocomplete candidates from SYMBOLS and SPECIAL_FORMS
            use relic::symbol::{SPECIAL_FORMS, SYMBOLS};
            use std::sync::Arc;
            let mut candidates: Vec<String> = SYMBOLS.keys().map(|&k| k.to_string()).collect();
            candidates.extend(SPECIAL_FORMS.keys().map(|&k| k.to_string()));
            candidates.sort();
            candidates.dedup();
            let completer = RelicCompleter {
                candidates: Arc::new(candidates),
            };
            let mut rl = Editor::<RelicCompleter, FileHistory>::new().unwrap();
            rl.set_helper(Some(completer));
            let _ = rl.load_history(".relic_history");

            println!("Relic REPL. Press Ctrl+D or type 'exit' to quit.");

            // start REPL
            let mut input_buffer = String::new();
            let prompt = ">>> ";
            let continuation_prompt = "... ";

            loop {
                let current_prompt = if input_buffer.is_empty() {
                    prompt
                } else {
                    continuation_prompt
                };
                let readline = rl.readline(current_prompt);

                match readline {
                    Ok(line) => {
                        // Add the line to our buffer
                        if !input_buffer.is_empty() {
                            input_buffer.push('\n');
                        }
                        input_buffer.push_str(&line);

                        // Check for exit command
                        if input_buffer.trim().eq_ignore_ascii_case("exit") {
                            break;
                        }

                        // Try to parse the input
                        match LexerMonad::new_unnamed(input_buffer.clone()).parse() {
                            Ok(node) => {
                                let backend = cli.backend.clone();
                                let exec_fn = move |n| match backend {
                                    Backend::Llvm => run_node_llvm(n),
                                    Backend::C => run_node(n),
                                };
                                match node.preprocess(&mut macros).and_then(exec_fn) {
                                    Ok(result) => {
                                        println!("= {result}");
                                        rl.add_history_entry(input_buffer.trim()).unwrap();
                                    }
                                    Err(msg) => {
                                        log_error(msg);
                                    }
                                }
                                input_buffer.clear();
                            }
                            Err(ParseError::EOF) => {
                                // Need more input, continue the loop
                                continue;
                            }
                            Err(ParseError::SyntaxError(msg)) => {
                                // Syntax error
                                log_error(msg);
                                input_buffer.clear();
                            }
                        }
                    }
                    Err(ReadlineError::Interrupted) => {
                        // Clear buffer and continue
                        input_buffer.clear();
                        continue;
                    }
                    Err(ReadlineError::Eof) => {
                        // Exit
                        println!("CTRL-D");
                        break;
                    }
                    Err(err) => {
                        println!("Error: {err:?}");
                        break;
                    }
                }
            }

            // Save command history
            rl.save_history(".relic_history").unwrap();
        }
        Mode::Compile => match input_node {
            Some(node) => match cli.backend {
                Backend::C => {
                    let mut codegen = match cli.package_name {
                        Some(name) => CodeGen::new_library(name),
                        None => CodeGen::new_main(),
                    };
                    unwrap_result(
                        compile(&node, &mut codegen),
                        &mut RT.write().unwrap(),
                    );
                    match cli.output_path {
                        Some(output_path) => {
                            let mut output_file = File::create(output_path).unwrap();
                            output_file
                                .write_all(codegen.to_string().as_bytes())
                                .unwrap();
                        }
                        None => {
                            println!("{codegen}");
                        }
                    }
                }
                Backend::Llvm => {
                    let context = inkwell::context::Context::create();
                    let mut codegen = match cli.package_name {
                        Some(name) => LlvmCodeGen::new_library(&context, name),
                        None => LlvmCodeGen::new_main(&context),
                    };
                    unwrap_result(
                        compile_llvm::compile_llvm(&node, &mut codegen, cli.debug_info),
                        &mut RT.write().unwrap(),
                    );
                    codegen.finalize();
                    match cli.output_path {
                        Some(output_path) => {
                            codegen
                                .write_to_file(output_path.to_str().unwrap())
                                .unwrap();
                        }
                        None => {
                            println!("{codegen}");
                        }
                    }
                }
            },
            None => {
                eprintln!("No files to compile");
            }
        },
    }
}
