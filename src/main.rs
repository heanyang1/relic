use std::{
    cell::RefCell,
    collections::HashMap,
    fs::{File, read_to_string},
    io::{self, Write},
    path::PathBuf,
    rc::Rc,
    str::FromStr,
};

use relic::{
    compile::{CodeGen, Compile}, env::Env, eval::{ConsoleEval, Eval, EvalResult}, graph::PrintState, lexer::Lexer, logger::{log_error, unwrap_result}, nil, node::{Node, NodeEnv}, parser::{Parse, ParseError}, preprocess::PreProcess, rt_import, symbol::Symbol
};

use clap::{Parser, ValueEnum};

/// All debug commands.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum DbgCmd {
    /// Steps through current evaluation.
    Step,
    /// Continue. The debugger won't stop until the evaluation is over or until
    /// it hits a breakpoint.
    Continue,
    /// Print the value of expression in current environment.
    Print(String),
    /// Print the environment graph in DOT code.
    Graph,
}

#[derive(Debug, Clone)]
pub struct DbgResult {
    /// Current evaluation result.
    node: Rc<RefCell<Node>>,
    /// Whether the step evaluation is on.
    step_is_on: bool,
}

impl DbgResult {
    pub fn new() -> Self {
        DbgResult {
            node: nil!().into(),
            step_is_on: false,
        }
    }
    pub fn poll_cmd(&mut self, env: Rc<RefCell<NodeEnv>>) {
        loop {
            match get_cmd() {
                DbgCmd::Step => {
                    self.step_is_on = true;
                    break;
                }
                DbgCmd::Continue => {
                    self.step_is_on = false;
                    break;
                }
                DbgCmd::Graph => {
                    let state = PrintState::new(env.clone(), "state".to_string());
                    println!("{state}");
                }
                DbgCmd::Print(var) => {
                    match env.borrow().get(&var, &()) {
                        Some(val) => println!("{} : {}", var, val.borrow()),
                        None => println!("Variable {var} not found"),
                    };
                }
            };
        }
    }
}

impl EvalResult for DbgResult {
    fn bind_display(self, output: &str) -> Self {
        println!("[stdout] {output}");
        self
    }
    fn bind_graph(self, env: Rc<RefCell<NodeEnv>>) -> Self {
        let state = PrintState::new(env.clone(), "state".to_string());
        println!("[graph] {state}");
        self
    }
    fn bind_eval(
        mut self,
        src: Rc<RefCell<Node>>,
        dst: Rc<RefCell<Node>>,
        env: Rc<RefCell<NodeEnv>>,
    ) -> Self {
        if self.step_is_on {
            println!("steps: {} |-> {}", src.borrow(), dst.borrow());
            self.poll_cmd(env);
        }
        self
    }
    fn bind_break(mut self, env: Rc<RefCell<NodeEnv>>) -> Self {
        println!("hit a breakpoint");
        self.poll_cmd(env);
        self
    }
    fn bind_node(mut self, node: Rc<RefCell<Node>>) -> Self {
        self.node = node;
        self
    }
    fn node(&self) -> Rc<RefCell<Node>> {
        self.node.clone()
    }
}

fn get_cmd() -> DbgCmd {
    loop {
        print!("dbg> ");
        io::stdout().flush().unwrap();
        let mut buf = String::new();
        unwrap_result(io::stdin().read_line(&mut buf), 0);
        match buf.as_str().trim_end() {
            "s" | "step" => return DbgCmd::Step,
            "c" | "continue" => return DbgCmd::Continue,
            "g" | "graph" => return DbgCmd::Graph,
            input => {
                match input
                    .strip_prefix("p ")
                    .or_else(|| input.strip_prefix("print "))
                {
                    Some(var) => return DbgCmd::Print(var.to_string()),
                    None => log_error(
                        "Wrong input. Available commands: (s)tep, (c)ontinue, (g)raph, (p)rint. Press C-c to quit.",
                    ),
                }
            }
        };
    }
}

#[derive(Debug, Clone, ValueEnum)]
enum Mode {
    /// Runs a REPL. If there is an input file, interprets it and modifies
    /// the environment.
    Repl,
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
}

fn main() {
    let cli = Cli::parse();

    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    let mut macros = HashMap::new();
    let input_node = cli.input_path.map(|file_path| {
        let file = unwrap_result(read_to_string(file_path), "".to_string());
        let mut node = unwrap_result(Node::from_str(&file), Node::Symbol(Symbol::Nil));
        unwrap_result(node.preprocess(&mut macros), Node::Symbol(Symbol::Nil))
    });

    match cli.mode {
        Mode::Repl => {
            let mut result = ConsoleEval::new();

            input_node.map(|node| {
                result = unwrap_result(node.eval(env.clone(), result.clone()), ConsoleEval::new());
                if *result.node.borrow() != nil!() {
                    println!("{}", result.node.borrow());
                }

                println!("stdout:");
                if let Some(output) = &result.display_output {
                    println!("{output}");
                }
                println!("graph:");
                if let Some(output) = &result.graphviz_output {
                    println!("{output}");
                }
            });

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
                        log_error(&msg);
                        continue;
                    }
                };
                node = unwrap_result(node.preprocess(&mut macros), Node::Symbol(Symbol::Nil));
                result = unwrap_result(node.eval(env.clone(), result.clone()), ConsoleEval::new());
                if *result.node.borrow() != nil!() {
                    println!("{}", result.node.borrow());
                }
                if let Some(ref output) = result.display_output {
                    println!("{output}");
                }
                if let Some(ref output) = result.graphviz_output {
                    println!("{output}");
                }
            }
        }
        Mode::Compile => {
            let mut codegen = match cli.package_name {
                Some(name) => CodeGen::new_library(name),
                None => CodeGen::new_main(),
            };
            match input_node {
                Some(node) => {
                    unwrap_result(node.compile(&mut codegen), ());
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
                    return;
                }
            };
        }
        Mode::Debug => {
            let mut result = DbgResult::new();
            let node = match input_node {
                Some(node) => node,
                None => {
                    eprintln!("No files to debug");
                    return;
                }
            };

            // stop before debugging
            result.poll_cmd(env.clone());
            result = unwrap_result(node.eval(env.clone(), result.clone()), result);
            if *result.node().borrow() != nil!() {
                println!("{}", result.node().borrow());
            }
        }
    }
}
