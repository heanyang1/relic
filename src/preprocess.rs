//! The preprocessor module.

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    body, nil,
    node::{Node, Pattern, pattern_matching},
    symbol::{SpecialForm, Symbol},
    util::vectorize,
};

/// Pre-process the AST before evaluating or compiling.
///
/// Things done in this stage:
/// 1. Expanding macros
/// 2. Syntax desugaring (e.g. `cond` -> `if`)
pub trait PreProcess {
    fn preprocess(&mut self, macros: &mut HashMap<String, Macro>) -> Result<Self, String>
    where
        Self: Sized;
}

/// Generate a list node from a vector of nodes.
#[macro_export]
macro_rules! vec_to_list {
    ($e:expr) => {
        Node::Pair($e, nil!().into())
    };
    ($e:expr, $($rest:tt)*) => {
        Node::Pair($e, vec_to_list!($($rest)*).into())
    };
}

/// Macros.
/// The fields are:
/// - Pattern
/// - Template (represented by a node)
pub struct Macro {
    pattern: Pattern,
    template: Rc<RefCell<Node>>,
}

impl Macro {
    fn new(pattern: Pattern, template: Rc<RefCell<Node>>) -> Self {
        Macro { pattern, template }
    }
}

impl Node {
    pub fn deep_copy(&self) -> Node {
        match self {
            Node::Number(num) => Node::Number(num.clone()),
            Node::Symbol(sym) => Node::Symbol(sym.clone()),
            Node::Pair(car, cdr) => Node::Pair(
                car.borrow().deep_copy().into(),
                cdr.borrow().deep_copy().into(),
            ),
            Node::SpecialForm(form) => Node::SpecialForm(form.clone()),
            Node::Procedure(_, _, _) => unreachable!(),
        }
    }

    pub fn replace(&mut self, src: &Node, dst: &Node) {
        if *self == *src {
            *self = dst.clone();
            return;
        }
        match self {
            Node::Procedure(_, _, _) => unreachable!(),
            Node::Number(_) | Node::Symbol(_) | Node::SpecialForm(_) => {
                // do nothing
            }
            Node::Pair(car, cdr) => {
                car.borrow_mut().replace(src, dst);
                cdr.borrow_mut().replace(src, dst);
            }
        }
    }
}

impl PreProcess for Node {
    fn preprocess(&mut self, macros: &mut HashMap<String, Macro>) -> Result<Node, String> {
        match self {
            // `Procedure`s are created during eval.
            Node::Procedure(_, _, _) => unreachable!(),
            Node::Number(_) | Node::Symbol(_) | Node::SpecialForm(_) => Ok(self.deep_copy()),
            Node::Pair(car, cdr) => {
                let car = car.borrow_mut().preprocess(macros)?;
                let cdr = cdr.borrow_mut().preprocess(macros)?;
                match &car {
                    Node::Procedure(_, _, _) => unreachable!(),
                    Node::Symbol(Symbol::User(sym)) if macros.contains_key(sym) => {
                        let Macro { pattern, template } = macros.get(sym).unwrap();
                        let mut bindings = HashMap::new();
                        let params = vectorize(cdr.into())?;
                        pattern_matching(pattern, &params, &mut bindings)?;

                        let mut body = template.borrow().deep_copy();
                        for (name, param) in bindings {
                            body.replace(&Node::Symbol(Symbol::User(name)), &param.borrow());
                        }
                        Ok(body)
                    }
                    Node::SpecialForm(SpecialForm::DefineSyntaxRule) => {
                        let (sym, body) = cdr.as_pair()?;
                        let (car, cdr) = sym.borrow().as_pair()?;
                        let name = car.borrow().as_user_symbol()?;
                        macros.insert(
                            name,
                            Macro::new(Pattern::try_from(cdr.clone())?, body!(body).into()),
                        );
                        Ok(nil!())
                    }
                    Node::SpecialForm(SpecialForm::Define) => {
                        // `(define (f ...) ...)` -> `(define f (lambda (...) ...))`
                        let (pattern, body) = cdr.as_pair()?;
                        if let Node::Pair(func, params) = &*pattern.borrow() {
                            let ret = vec_to_list!(
                                car.into(),
                                func.clone(),
                                Node::Pair(
                                    Node::SpecialForm(SpecialForm::Lambda).into(),
                                    Node::Pair(params.clone(), body).into()
                                )
                                .into()
                            );
                            Ok(ret)
                        } else {
                            Ok(Node::Pair(car.into(), cdr.into()))
                        }
                    }
                    Node::SpecialForm(SpecialForm::Cond) => {
                        // (cond (c1 v1) (c2 v2) ...)
                        // -> (if c1
                        //        (begin v1)
                        //        (if c2
                        //            (begin v2)
                        //            ...))
                        let params = vectorize(cdr.into())?;
                        let mut body = nil!();
                        for node in params.iter().rev() {
                            let (cond, value) = node.borrow().as_pair()?;
                            body = vec_to_list!(
                                Node::SpecialForm(SpecialForm::If).into(),
                                cond,
                                Node::Pair(Node::SpecialForm(SpecialForm::Begin).into(), value)
                                    .into(),
                                body.into()
                            );
                        }
                        Ok(body)
                    }
                    Node::SpecialForm(SpecialForm::And) => {
                        // (and x1 x2 ... xn)
                        // -> (if (eq? x1 nil)
                        //        x1
                        //        (if (eq? x2 nil)
                        //            x2
                        //            ...
                        //              (if (eq? xn nil)
                        //                  xn
                        //                  xn)...))
                        let params = vectorize(cdr.into())?;
                        if params.is_empty() {
                            Ok(Node::Symbol(Symbol::T))
                        } else {
                            let value = params.last().unwrap();
                            let mut body = vec_to_list!(
                                Node::SpecialForm(SpecialForm::If).into(),
                                vec_to_list!(
                                    Node::Symbol(Symbol::Eq).into(),
                                    value.clone(),
                                    nil!().into()
                                )
                                .into(),
                                value.clone(),
                                value.clone()
                            );
                            for value in params.iter().rev().skip(1) {
                                body = vec_to_list!(
                                    Node::SpecialForm(SpecialForm::If).into(),
                                    vec_to_list!(
                                        Node::Symbol(Symbol::Eq).into(),
                                        value.clone(),
                                        nil!().into()
                                    )
                                    .into(),
                                    value.clone(),
                                    body.into()
                                );
                            }
                            Ok(body)
                        }
                    }
                    Node::SpecialForm(SpecialForm::Or) => {
                        // (or x1 x2 ... xn)
                        // -> (if x1
                        //        x1
                        //        (if x2
                        //            x2
                        //            ...
                        //              (if xn
                        //                  xn
                        //                  nil)...))
                        let params = vectorize(cdr.into())?;
                        let mut body = nil!();
                        for param in params.iter().rev() {
                            body = vec_to_list!(
                                Node::SpecialForm(SpecialForm::If).into(),
                                param.clone(),
                                param.clone(),
                                body.into()
                            )
                        }
                        Ok(body)
                    }
                    Node::SpecialForm(SpecialForm::Let) => {
                        // (let ((x1 e11) (x2 e12) ...) e21 e22 ...)
                        // -> ((lambda (x1 x2 ...) e21 e22 ...) e11 e12 ...)
                        let (bindings, body) = cdr.as_pair()?;
                        let mut keys = vec![];
                        let mut values = vec![];
                        for binding in vectorize(bindings)? {
                            let (k, v) = binding.borrow().as_pair()?;
                            keys.push(k);
                            let (car, _) = v.borrow().as_pair()?;
                            values.push(car);
                        }
                        let keys_node = Node::from_iter(keys);
                        let values_node = Node::from_iter(values);
                        let ret = Node::Pair(
                            Node::Pair(
                                Node::SpecialForm(SpecialForm::Lambda).into(),
                                Node::Pair(keys_node.into(), body).into(),
                            )
                            .into(),
                            values_node.into(),
                        );
                        Ok(ret)
                    }
                    _ => Ok(Node::Pair(car.into(), cdr.into())),
                }
            }
        }
    }
}
