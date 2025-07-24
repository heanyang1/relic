#![allow(clippy::new_without_default)]

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    env::Env,
    graph::PrintState,
    lexer::Number,
    nil,
    node::{Node, NodeEnv, Pattern, pattern_matching},
    symbol::{SpecialForm, Symbol},
    util::{eval_arith, eval_rel, exactly_n_params, get_n_params, vectorize},
};

impl TryFrom<Rc<RefCell<Node>>> for Number {
    type Error = String;
    fn try_from(value: Rc<RefCell<Node>>) -> Result<Self, Self::Error> {
        value.borrow().as_num()
    }
}

macro_rules! arith_op {
    ($params:expr, $op:tt) => {{
        Ok(Node::Number(eval_arith($params, |a, b| a $op b)?).into())
    }};
}

macro_rules! rel_op {
    ($params:expr, $op:tt) => {{
        Ok(Node::Symbol(eval_rel($params, |a, b| a $op b)?).into())
    }};
}

/// Process a list of expression to evaluate, such as the function body of
/// `lambda`.
#[macro_export]
macro_rules! body {
    ($node: expr) => {
        Node::Pair(Node::SpecialForm(SpecialForm::Begin).into(), $node)
    };
}

pub trait EvalResult {
    /// Add a node as eval result.
    fn bind_node(self, node: Rc<RefCell<Node>>) -> Self;
    /// `(display ...)` is executed.
    fn bind_display(self, output: &str) -> Self;
    /// `(graphviz)` is executed.
    fn bind_graph(self, env: Rc<RefCell<NodeEnv>>) -> Self;
    /// `(breakpoint)` is executed.
    fn bind_break(self, env: Rc<RefCell<NodeEnv>>) -> Self;
    /// Logs a call to `eval`. The parameters mean that `src` evals to `dst`
    /// in environment `env`.
    fn bind_eval(
        self,
        src: Rc<RefCell<Node>>,
        dst: Rc<RefCell<Node>>,
        env: Rc<RefCell<NodeEnv>>,
    ) -> Self;
    fn node(&self) -> Rc<RefCell<Node>>;
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct ConsoleEval {
    pub node: Rc<RefCell<Node>>,
    pub display_output: Option<String>,
    pub graphviz_output: Option<String>,
}

impl ConsoleEval {
    pub fn new() -> Self {
        Self {
            node: nil!().into(),
            display_output: None,
            graphviz_output: None,
        }
    }
}

impl EvalResult for ConsoleEval {
    fn bind_node(mut self, node: Rc<RefCell<Node>>) -> Self {
        self.node = node;
        self
    }

    fn bind_display(mut self, output: &str) -> Self {
        self.display_output = if let Some(old_output) = self.display_output {
            Some(old_output + output)
        } else {
            Some(output.to_string())
        };
        self
    }

    fn bind_graph(mut self, env: Rc<RefCell<NodeEnv>>) -> Self {
        let state = PrintState::new(env, "state".to_string());
        let output = format!("{state}");
        self.graphviz_output = if let Some(old_output) = self.graphviz_output {
            Some(old_output + &output)
        } else {
            Some(output)
        };
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

    fn node(&self) -> Rc<RefCell<Node>> {
        self.node.clone()
    }
}

/// The trait for evaluating a node. It performs monadic `bind` to `EvalResult`.
pub trait Eval<T>
where
    T: EvalResult,
{
    fn eval(&self, env: Rc<RefCell<NodeEnv>>, result: T) -> Result<T, String>;
}

/// The trait for applying `params` to a node. It performs monadic bind to
/// `EvalResult`.
pub trait Apply<T>
where
    T: EvalResult,
{
    fn apply(
        &self,
        env: Rc<RefCell<NodeEnv>>,
        params: Rc<RefCell<Node>>,
        result: T,
    ) -> Result<T, String>;
}

impl<T> Apply<T> for SpecialForm
where
    T: EvalResult,
{
    fn apply(
        &self,
        env: Rc<RefCell<NodeEnv>>,
        cdr: Rc<RefCell<Node>>,
        result: T,
    ) -> Result<T, String> {
        match self {
            SpecialForm::Quote => Ok(result.bind_node(get_n_params(cdr.clone(), 1)?[0].clone())),
            SpecialForm::If => {
                let params = get_n_params(cdr.clone(), 3)?;
                let result = params[0].borrow().eval(env.clone(), result)?;
                let body = if *result.node().borrow() != nil!() {
                    params[1].clone()
                } else {
                    params[2].clone()
                };
                body.borrow().eval(env.clone(), result)
            }
            SpecialForm::Define => {
                let params = get_n_params(cdr.clone(), 2)?;
                if let Node::Symbol(Symbol::User(name)) = &*params[0].borrow() {
                    let result = params[1].borrow().eval(env.clone(), result)?;
                    // Does not check whether the node is in the environment.
                    env.borrow_mut().define(name, result.node(), &mut ());
                    Ok(result.bind_node(nil!().into()))
                } else {
                    Err(format!(
                        "{} is not a user defined symbol",
                        params[0].borrow()
                    ))
                }
            }
            SpecialForm::Set => {
                // `set` does not change the node; it just adds a binding to the environment
                let params = get_n_params(cdr.clone(), 2)?;
                let sym = &params[0];
                let expr = &params[1];
                let name = sym.borrow().as_user_symbol()?;
                let result = expr.borrow().eval(env.clone(), result)?;
                env.borrow_mut().set(&name, result.node(), &mut ());
                Ok(result.bind_node(nil!().into()))
            }
            SpecialForm::SetCar => {
                let params = get_n_params(cdr.clone(), 2)?;
                let sym = &params[0];
                let expr = &params[1];
                let name = sym.borrow().as_user_symbol()?;
                let result = expr.borrow().eval(env.clone(), result)?;
                env.borrow()
                    .get(&name, &())
                    .ok_or(format!("{} is not defined", &name))?
                    .borrow_mut()
                    .set_car(result.node())?;
                Ok(result.bind_node(nil!().into()))
            }
            SpecialForm::SetCdr => {
                let params = get_n_params(cdr.clone(), 2)?;
                let sym = &params[0];
                let expr = &params[1];
                let name = sym.borrow().as_user_symbol()?;
                let result = expr.borrow().eval(env.clone(), result)?;
                env.borrow()
                    .get(&name, &())
                    .ok_or(format!("{} is not defined", &name))?
                    .borrow_mut()
                    .set_cdr(result.node())?;
                Ok(result.bind_node(nil!().into()))
            }
            // lambda
            SpecialForm::Lambda => {
                let (params, body) = cdr.borrow().as_pair()?;
                let pattern = Pattern::try_from(params)?;
                Ok(result.bind_node(Node::Procedure(pattern, body!(body).into(), env).into()))
            }
            // begin
            SpecialForm::Begin => {
                let params = vectorize(cdr.clone())?;
                let mut cur_result = result;
                for expr in params {
                    cur_result = expr.borrow().eval(env.clone(), cur_result)?;
                }
                Ok(cur_result)
            }
            // SpecialForm::Or => {
            //     let params = vectorize(cdr)?;
            //     if params.is_empty() {
            //         return Ok(result.bind_node(nil!().into()));
            //     }
            //     let mut cur_result = result;
            //     for param in params {
            //         cur_result = param.borrow().eval(env.clone(), cur_result)?;
            //         if *cur_result.node().borrow() != nil!() {
            //             return Ok(cur_result);
            //         }
            //     }
            //     Ok(cur_result)
            // }
            // display
            SpecialForm::Display => {
                let params = get_n_params(cdr, 1)?;
                let result = params[0].borrow().eval(env.clone(), result)?;
                let output = format!("{}", result.node().borrow());
                Ok(result.bind_node(nil!().into()).bind_display(&output))
            }
            SpecialForm::NewLine => {
                let _params = get_n_params(cdr, 0)?;
                Ok(result
                    .bind_node(nil!().into())
                    .bind_display(&String::from("\n")))
            }
            SpecialForm::Graphviz => Ok(result.bind_node(nil!().into()).bind_graph(env)),
            SpecialForm::BreakPoint => Ok(result.bind_node(nil!().into()).bind_break(env)),
            _ => unreachable!(),
        }
    }
}

impl<T> Eval<T> for Symbol
where
    T: EvalResult,
{
    fn eval(&self, env: Rc<RefCell<NodeEnv>>, result: T) -> Result<T, String> {
        match self {
            Symbol::User(str) => match env.borrow().get(str, &()) {
                Some(node) => Ok(result.bind_node(node)),
                None => Err(format!("Symbol {str} not found")),
            },
            _ => Ok(result.bind_node(Node::Symbol(self.clone()).into())),
        }
    }
}

impl<T> Apply<T> for Symbol
where
    T: EvalResult,
{
    fn apply(
        &self,
        env: Rc<RefCell<NodeEnv>>,
        cdr: Rc<RefCell<Node>>,
        result: T,
    ) -> Result<T, String> {
        // dirty trick for deep recursive functions. See `stacker` package's documentation.
        stacker::maybe_grow(32 * 1024, 1024 * 1024, || {
            let mut result = result;
            let nodes = vectorize(cdr)?;
            let mut params = Vec::new();
            for node in nodes {
                result = node.borrow().eval(env.clone(), result)?;
                params.push(result.node());
            }

            let node =
                match self {
                    Symbol::User(_) => panic!("Should have been evaluated"),
                    Symbol::T | Symbol::Nil => Err(format!("{self} can not be the head of a list")),
                    // arithmetic
                    Symbol::Add => arith_op!(params, +),
                    Symbol::Sub => arith_op!(params, -),
                    Symbol::Mul => arith_op!(params, *),
                    Symbol::Div => arith_op!(params, /),
                    // relational
                    Symbol::EqNum => rel_op!(params, ==),
                    Symbol::Lt => rel_op!(params, <),
                    Symbol::Gt => rel_op!(params, >),
                    Symbol::Le => rel_op!(params, <=),
                    Symbol::Ge => rel_op!(params, >=),
                    // list
                    Symbol::List => Ok(Node::from_iter(params.into_iter()).into()),
                    Symbol::Car => exactly_n_params(&params, 1)
                        .and_then(|_| Ok(params[0].borrow().as_pair()?.0)),
                    Symbol::Cdr => exactly_n_params(&params, 1)
                        .and_then(|_| Ok(params[0].borrow().as_pair()?.1)),
                    Symbol::Cons => exactly_n_params(&params, 2)
                        .map(|_| Node::Pair(params[0].clone(), params[1].clone()).into()),
                    Symbol::Atom => exactly_n_params(&params, 1).map(|_| {
                        let node = params[0].borrow().as_pair();
                        Node::Symbol(if node.is_err() {
                            Symbol::T
                        } else {
                            Symbol::Nil
                        })
                        .into()
                    }),
                    Symbol::Eq => exactly_n_params(&params, 2).map(|_| {
                        Node::Symbol(if params[0] == params[1] {
                            Symbol::T
                        } else {
                            Symbol::Nil
                        })
                        .into()
                    }),
                    Symbol::Number => exactly_n_params(&params, 1).map(|_| {
                        let node = params[0].borrow().as_int();
                        Node::Symbol(if node.is_ok() { Symbol::T } else { Symbol::Nil }).into()
                    }),
                }?;
            Ok(result.bind_node(node))
        })
    }
}

impl<T> Eval<T> for Node
where
    T: EvalResult,
{
    fn eval(&self, env: Rc<RefCell<NodeEnv>>, result: T) -> Result<T, String> {
        stacker::maybe_grow(32 * 1024, 1024 * 1024, || {
            let self_ref = Rc::new(RefCell::new(self.clone()));
            let result = match self {
                Node::Symbol(sym) => sym.eval(env.clone(), result),
                Node::Number(_) | Node::Procedure(_, _, _) | Node::SpecialForm(_) => {
                    Ok(result.bind_node(self.clone().into()))
                }
                Node::Pair(car, cdr) => {
                    let result = car.borrow().eval(env.clone(), result)?;
                    result
                        .node()
                        .borrow()
                        .apply(env.clone(), cdr.clone(), result)
                }
            }?;
            let node = result.node();
            Ok(result.bind_eval(self_ref, node, env))
        })
    }
}

impl<T> Apply<T> for Node
where
    T: EvalResult,
{
    fn apply(
        &self,
        env: Rc<RefCell<NodeEnv>>,
        cdr: Rc<RefCell<Node>>,
        result: T,
    ) -> Result<T, String> {
        match self {
            Node::Number(_) | Node::Pair(_, _) => {
                Err(format!("{self} can not be the head of a list"))
            }
            Node::SpecialForm(sym) => sym.apply(env, cdr.clone(), result),
            Node::Symbol(sym) => sym.apply(env, cdr.clone(), result),
            Node::Procedure(args, body, lambda_env) => {
                // Evaluate each parameters and use them to create new environment
                let params = vectorize(cdr)?;
                let mut params_val = vec![];
                let mut result = result;
                for param in params {
                    result = param.borrow().eval(env.clone(), result)?;
                    params_val.push(result.node());
                }
                let mut bindings = HashMap::new();
                pattern_matching(args, &params_val, &mut bindings)?;
                let new_env = NodeEnv::new(Some(lambda_env.clone()), bindings, &format!("{self}"));
                body.borrow().eval(Rc::new(RefCell::new(new_env)), result)
            }
        }
    }
}
