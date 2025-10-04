pub mod env;
pub mod eval;
pub mod graph;
pub mod lexer;
pub mod node;
pub mod parser;
pub mod preprocess;
pub mod symbol;
mod util;
use std::{
    cell::RefCell,
    collections::HashMap,
    rc::Rc,
};

use crate::{
    env::Env,
    eval::{Eval, EvalResult},
    graph::PrintState,
    lexer::{Lexer},
    node::{Node, NodeEnv},
    parser::Parse,
    preprocess::PreProcess,
    symbol::Symbol,
};
use wasm_bindgen::prelude::*;

#[derive(Clone)]
struct WebEval {
    node: Rc<RefCell<Node>>,
    graph_cnt: usize,
}

impl WebEval {
    fn new() -> Self {
        WebEval {
            node: nil!().into(),
            graph_cnt: 1,
        }
    }
}

impl EvalResult for WebEval {
    fn bind_node(mut self, new_node: Rc<RefCell<Node>>) -> Self {
        self.node = new_node;
        self
    }
    fn node(&self) -> Rc<RefCell<Node>> {
        self.node.clone()
    }
    fn bind_display(self, output: &str) -> Self {
        writeStdout(output);
        self
    }
    fn bind_graph(mut self, env: Rc<RefCell<NodeEnv>>) -> Self {
        let state = PrintState::new(env, format!("graph_{}", self.graph_cnt));
        let output = format!("{state}");
        writeGraph(&output, self.graph_cnt);
        self.graph_cnt += 1;
        self
    }
    fn bind_break(self, _: Rc<RefCell<NodeEnv>>) -> Self {
        self
    }
    fn bind_eval(
        self,
        _: Rc<RefCell<Node>>,
        _: Rc<RefCell<Node>>,
        _: Rc<RefCell<NodeEnv>>,
    ) -> Self {
        self
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = window)]
    fn writeOutput(text: &str);

    #[wasm_bindgen(js_namespace = window)]
    fn writeStdout(text: &str);

    #[wasm_bindgen(js_namespace = window)]
    fn writeGraph(text: &str, graph_count: usize);
}

#[wasm_bindgen]
pub fn evaluate(input: &str) {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    let mut result = WebEval::new();
    let mut tokens = Lexer::new(input);
    let mut macros = HashMap::new();

    while let Ok(mut node) = Node::parse(&mut tokens) {
        let node = node.preprocess(&mut macros);
        if node.is_err() {
            writeOutput(&format!(
                "Error preprocessing expression: {}",
                node.err().unwrap()
            ));
            return;
        }
        let eval = node.unwrap().eval(env.clone(), result.clone());
        if eval.is_err() {
            writeOutput(&format!(
                "Error evaluating expression: {}",
                eval.err().unwrap()
            ));
            return;
        }
        result = eval.unwrap();
    }
    writeOutput(&format!("{}\n", result.node.borrow()));
}
