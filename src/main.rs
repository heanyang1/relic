use std::{
    collections::HashMap,
    fs::File,
    io::{self, Write},
    path::PathBuf,
};

use relic::{
    RT,
    compile::{CodeGen, compile},
    file_to_node,
    lexer::Lexer,
    logger::{LogLevel, log_error, set_log_level, unwrap_result},
    node::Node,
    parser::{Parse, ParseError},
    preprocess::PreProcess,
    rt_start, run_node,
    runtime::StackMachine,
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

            // start REPL
            let mut input = String::new();
            loop {
                if input.is_empty() {
                    print!("> ");
                }
                io::stdout().flush().unwrap();
                let read_result = unwrap_result(io::stdin().read_line(&mut input), 0);
                if read_result == 0 {
                    // An error occur, or C-d is pressed
                    println!("Quit");
                    return;
                }
                let mut tokens = Lexer::new(&input);
                let mut node = match Node::parse(&mut tokens) {
                    Ok(node) => {
                        input.clear();
                        node
                    }
                    Err(ParseError::EOF) => continue,
                    Err(ParseError::SyntaxError(msg)) => {
                        log_error(msg);
                        continue;
                    }
                };
                node = unwrap_result(node.preprocess(&mut macros), Node::Symbol(Symbol::Nil));
                println!("{}", run_node(node));
            }
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
                    let mut runtime = RT.lock().unwrap();
                    runtime.begin_debug();
                }
                unwrap_result(node.jit_compile(true), ());
                let mut runtime = RT.lock().unwrap();
                let index = runtime.pop();
                println!("result: {}", runtime.display_node_idx(index))
            }
            None => {
                eprintln!("No files to compile");
            }
        },
    }
}
