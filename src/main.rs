use std::{
    collections::HashMap,
    fs::File,
    io::{self, Write},
    path::PathBuf,
};

use rustyline::{Editor, error::ReadlineError};

use relic::{
    RT,
    compile::{CodeGen, compile},
    env::Env,
    error::ParseError,
    file_to_node,
    lexer::Lexer,
    logger::{LogLevel, log_debug, log_error, set_log_level, unwrap_result},
    node::Node,
    parser::Parse,
    preprocess::PreProcess,
    rt_start, run_node,
    runtime::{DbgState, Runtime, StackMachine},
    symbol::Symbol,
};

use clap::{Parser, ValueEnum};

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
    /// Run a debugger on input file.
    Debug,
}

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
}

fn dbg_loop(runtime: &Runtime) -> DbgState {
    loop {
        print!("dbg> ");
        io::stdout().flush().unwrap();
        let mut buf = String::new();
        unwrap_result(io::stdin().read_line(&mut buf), 0);
        match buf.as_str().trim_end() {
            "s" | "step" => {
                return DbgState::Step;
            }
            "n" | "next" => {
                return DbgState::Next;
            }
            "c" | "continue" => {
                return DbgState::Normal;
            }
            "r" | "runtime" => log_debug(format!("{runtime}")),
            input => {
                match input
                    .strip_prefix("p ")
                    .or_else(|| input.strip_prefix("print "))
                {
                    Some(var) => {
                        let env = runtime.current_env();
                        let idx = env.get(&var.to_string(), &runtime);
                        match idx {
                            Some(idx) => {
                                log_debug(format!("{var} = {}", runtime.display_node_idx(idx)))
                            }
                            None => log_error(format!("variable {var} not found")),
                        };
                    }
                    None => log_error(
                        "Wrong input. Available commands: (s)tep, (n)ext, (c)ontinue, (p)rint, (r)untime. Press C-c to quit.",
                    ),
                }
            }
        };
    }
}
fn main() {
    let cli = Cli::parse();

    let mut macros = HashMap::new();
    let input_node = file_to_node(cli.input_path, &mut macros);

    match cli.mode {
        Mode::Run => {
            rt_start();
            if let Some(node) = input_node {
                println!("result: {}", run_node(node));
            } else {
                eprintln!("No files to run");
            }
        }
        Mode::Repl => {
            rt_start();

            if let Some(node) = input_node {
                println!("result: {}", run_node(node));
            }

            // Initialize rustyline editor with default configuration
            let mut rl = Editor::<(), _>::new().unwrap();
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
                        let mut tokens = Lexer::new(&input_buffer);
                        match Node::parse(&mut tokens) {
                            Ok(mut node) => {
                                // Successful parse, execute and clear buffer
                                node = unwrap_result(
                                    node.preprocess(&mut macros),
                                    Node::Symbol(Symbol::Nil),
                                );
                                println!("= {}", run_node(node));
                                rl.add_history_entry(input_buffer.trim()).unwrap();
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
                        println!("Error: {:?}", err);
                        break;
                    }
                }
            }

            // Save command history
            rl.save_history(".relic_history").unwrap();
        }
        Mode::Compile => {
            let mut codegen = match cli.package_name {
                Some(name) => CodeGen::new_library(name),
                None => CodeGen::new_main(),
            };
            match input_node {
                Some(node) => {
                    unwrap_result(compile(&node, &mut codegen, cli.debug_info), ());
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
                None => {
                    eprintln!("No files to compile");
                }
            }
        }
        Mode::Debug => match input_node {
            Some(node) => {
                rt_start();
                set_log_level(LogLevel::Debug);
                {
                    let mut runtime = RT.write().unwrap();
                    runtime.set_callback(dbg_loop);
                    runtime.begin_debug();
                }
                unwrap_result(node.jit_compile(true), ());
                let mut runtime = RT.write().unwrap();
                let index = runtime.pop();
                println!("result: {}", runtime.display_node_idx(index))
            }
            None => {
                eprintln!("No files to compile");
            }
        },
    }
}
