//! The preprocessor module.
//!
//! This module handles syntactic transformations that occur before compilation:
//! - Macro expansion
//! - Syntax desugaring (cond -> if, let -> lambda, etc.)
//!
//! ## Transformations
//!
//! - `(cond (c1 v1) (c2 v2) ...)` -> nested `(if c1 (begin v1) ...)`
//! - `(let ((x e) ...) body...)` -> `((lambda (x ...) body...) e ...)`
//! - `(define (f x ...) body...)` -> `(define f (lambda (x ...) body...))`
//! - `(lambda (...) body...)` -> `(lambda (...) (begin body...))`

use std::{cell::RefCell, collections::HashMap, rc::Rc};

use crate::{
    lexer::LexerMonad,
    nil,
    node::{Node, NodeRef},
    parser::vec_to_list,
    symbol::{SpecialForm, Symbol},
    util::Vectorize,
};

/// Process a list of expression to evaluate, such as the function body of
/// `lambda`.
macro_rules! body {
    ($node: expr) => {
        LexerMonad::from_other(
            Node::Pair(
                LexerMonad::from_other(
                    Node::SpecialForm(SpecialForm::Begin),
                    $node.borrow().get_begin(),
                )
                .into(),
                $node,
            ),
            $node.borrow().get_begin(),
        )
    };
}

/// A user-defined macro created via `define-syntax-rule`.
///
/// Macros pattern-match on expressions and transform them using templates.
pub struct Macro {
    /// The pattern to match against.
    pattern: NodeRef,
    /// The template to expand to when pattern matches.
    template: NodeRef,
}

impl Macro {
    fn new(pattern: NodeRef, template: NodeRef) -> Self {
        Macro { pattern, template }
    }
}

impl LexerMonad<Node> {
    /// Replaces all occurrences of a source node with a destination node.
    ///
    /// Used for macro expansion and code transformation.
    ///
    /// # Parameters
    ///
    /// * `src` - The node to find
    /// * `dst` - The node to replace with
    ///
    /// # Returns
    ///
    /// A new LexerMonad with the substitution applied
    pub fn replace_node(&self, src: &Node, dst: &Node) -> Self {
        self.bind(move |node| {
            if *node == *src {
                return dst.clone();
            }
            match node {
                Node::Number(_) | Node::Symbol(_) | Node::SpecialForm(_) | Node::String(_) => {
                    node.clone()
                }
                Node::Pair(car, cdr) => Node::Pair(
                    car.clone().borrow().replace_node(src, dst).into(),
                    cdr.clone().borrow().replace_node(src, dst).into(),
                ),
            }
        })
    }

    /// Checks if the node is a valid macro pattern.
    ///
    /// A valid pattern is a proper list where each element is either:
    /// - A user-defined symbol (matches anything)
    /// - nil (matches the empty list)
    fn is_pattern(&self) -> bool {
        match self.get() {
            Node::Pair(car, cdr) => match (car.borrow().get(), cdr.borrow().get()) {
                (Node::Symbol(Symbol::User(_)), Node::Symbol(Symbol::User(_))) => true,
                (Node::Symbol(Symbol::User(_)), nil!()) => true,
                (Node::Symbol(Symbol::User(_)), _) => cdr.borrow().is_pattern(),
                _ => false,
            },
            _ => false,
        }
    }
}

/// Pattern matching for macro expansion.
///
/// Matches a pattern against an actual expression and collects bindings.
///
/// # Parameters
///
/// * `pattern` - The macro pattern
/// * `actual` - The expression to match
/// * `bindings` - HashMap to store matched bindings
///
/// # Returns
///
/// Ok(()) on success, Err(String) on mismatch
pub fn pattern_matching(
    pattern: NodeRef,
    actual: NodeRef,
    bindings: &mut HashMap<String, NodeRef>,
) -> Result<(), String> {
    match (pattern.borrow().get(), actual.borrow().get()) {
        (nil!(), nil!()) => Ok(()),
        (Node::Symbol(sym), _) => {
            bindings.insert(sym.to_string(), actual.clone());
            Ok(())
        }
        (Node::Pair(car, cdr), Node::Pair(car_actual, cdr_actual)) => {
            if let Node::Symbol(car) = car.borrow().get() {
                bindings.insert(car.to_string(), car_actual.clone());
                pattern_matching(cdr.clone(), cdr_actual.clone(), bindings)
            } else {
                Err(pattern
                    .borrow()
                    .error(format!("Pattern {} is not a symbol", car.borrow())))
            }
        }
        _ => Err(format!(
            "Parameter mismatch: expect {}, got {:?}",
            pattern.borrow(),
            actual
        )),
    }
}

/// Trait for types that can be preprocessed.
///
/// Implementors perform syntactic transformations like macro expansion
/// and syntax desugaring.
pub trait PreProcess
where
    Self: Sized,
{
    /// Preprocess the AST before evaluation or compilation.
    ///
    /// # Parameters
    ///
    /// * `macros` - Map of defined macros
    ///
    /// # Returns
    ///
    /// The transformed AST
    fn preprocess(&self, macros: &mut HashMap<String, Macro>) -> Result<Self, String>;
}

impl PreProcess for LexerMonad<Node> {
    /// Pre-process the AST before evaluating or compiling.
    ///
    /// Things done in this stage:
    /// 1. Expanding macros
    /// 2. Syntax desugaring (e.g. `cond` -> `if`)
    fn preprocess(&self, macros: &mut HashMap<String, Macro>) -> Result<Self, String> {
        match self.get() {
            Node::Number(_) | Node::Symbol(_) | Node::SpecialForm(_) | Node::String(_) => {
                Ok::<LexerMonad<Node>, String>(self.clone())
            }
            Node::Pair(car, cdr) => {
                let car = car.borrow_mut().preprocess(macros)?;
                // skip preprocessing if this expression is quoted
                if *car.get() == Node::SpecialForm(SpecialForm::Quote) {
                    return Ok(LexerMonad::from_other(
                        Node::Pair(car.into(), cdr.clone()),
                        self.get_fp(),
                    ));
                }
                let cdr = cdr.borrow().preprocess(macros)?;
                let ret = match car.get() {
                    Node::Symbol(Symbol::User(sym)) if macros.contains_key(sym) => {
                        let Macro { pattern, template } = macros.get(sym).unwrap();
                        let mut bindings = HashMap::new();
                        pattern_matching(pattern.clone(), cdr.into(), &mut bindings)?;

                        let mut body = template.borrow().clone();
                        for (name, param) in bindings {
                            body = body.replace_node(
                                &Node::Symbol(Symbol::User(name)),
                                param.borrow().get(),
                            );
                        }
                        Ok(body)
                    }
                    Node::SpecialForm(SpecialForm::DefineSyntaxRule) => {
                        let (sym, body) = cdr.as_pair()?;
                        let (car, cdr) = sym.borrow().as_pair()?;
                        let name = car.borrow().as_user_symbol()?;
                        if cdr.borrow().is_pattern() {
                            macros.insert(name, Macro::new(cdr, body!(body.clone()).into()));
                            Ok(LexerMonad::from_other(nil!(), self.get_fp()))
                        } else {
                            Err(self.error(format!("{} is not a valid pattern", cdr.borrow())))
                        }
                    }
                    Node::SpecialForm(SpecialForm::Lambda) => {
                        // `(lambda (...) ...)` -> `(lambda (...) (begin ...))`
                        let (pattern, body) = cdr.as_pair()?;
                        Ok(vec_to_list(&[
                            car.into(),
                            pattern,
                            LexerMonad::from_other(
                                Node::Pair(
                                    LexerMonad::from_other(
                                        Node::SpecialForm(SpecialForm::Begin),
                                        body.borrow().get_fp(),
                                    )
                                    .into(),
                                    body.clone(),
                                ),
                                body.borrow().get_fp(),
                            )
                            .into(),
                        ]))
                    }
                    Node::SpecialForm(SpecialForm::Define) => {
                        // `(define (f ...) ...)` -> `(define f (lambda (...) (begin ...)))`
                        let (pattern, body) = cdr.as_pair()?;
                        if let Node::Pair(func, params) = pattern.borrow().get() {
                            let ret = vec_to_list(&[
                                car.into(),
                                func.clone(),
                                vec_to_list(&[
                                    LexerMonad::from_other(
                                        Node::SpecialForm(SpecialForm::Lambda),
                                        self.get_fp(),
                                    )
                                    .into(),
                                    params.clone(),
                                    LexerMonad::from_other(
                                        Node::Pair(
                                            LexerMonad::from_other(
                                                Node::SpecialForm(SpecialForm::Begin),
                                                self.get_fp(),
                                            )
                                            .into(),
                                            body.clone(),
                                        ),
                                        self.get_fp(),
                                    )
                                    .into(),
                                ])
                                .into(),
                            ]);
                            Ok(ret)
                        } else {
                            Ok(LexerMonad::from_other(
                                Node::Pair(car.into(), cdr.into()),
                                self.get_fp(),
                            ))
                        }
                    }
                    Node::SpecialForm(SpecialForm::Cond) => {
                        // (cond (c1 v1) (c2 v2) ...)
                        // -> (if c1
                        //        (begin v1)
                        //        (if c2
                        //            (begin v2)
                        //            ...))
                        let params = Rc::new(RefCell::new(cdr)).vectorize_proper_list()?;
                        let mut body = nil!(self.get_end());
                        for node in params.iter().rev() {
                            let (cond, value) = node.borrow().as_pair()?;
                            body = vec_to_list(&[
                                LexerMonad::from_other(
                                    Node::SpecialForm(SpecialForm::If),
                                    cond.borrow().get_begin(),
                                )
                                .into(),
                                cond.clone(),
                                LexerMonad::from_other(
                                    Node::Pair(
                                        LexerMonad::from_other(
                                            Node::SpecialForm(SpecialForm::Begin),
                                            value.clone().borrow().get_begin(),
                                        )
                                        .into(),
                                        value.clone(),
                                    ),
                                    value.borrow().get_fp(),
                                )
                                .into(),
                                body.into(),
                            ]);
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
                        let params = Rc::new(RefCell::new(cdr)).vectorize_proper_list()?;
                        if params.is_empty() {
                            Ok(LexerMonad::from_other(
                                Node::Symbol(Symbol::T),
                                self.get_fp(),
                            ))
                        } else {
                            let value = params.last().unwrap();
                            let mut body = vec_to_list(&[
                                LexerMonad::from_other(
                                    Node::SpecialForm(SpecialForm::If),
                                    value.borrow().get_fp(),
                                )
                                .into(),
                                vec_to_list(&[
                                    LexerMonad::from_other(
                                        Node::Symbol(Symbol::Eq),
                                        value.borrow().get_fp(),
                                    )
                                    .into(),
                                    value.clone(),
                                    nil!(value.borrow().get_fp()).into(),
                                ])
                                .into(),
                                value.clone(),
                                value.clone(),
                            ]);
                            for value in params.iter().rev().skip(1) {
                                body = vec_to_list(&[
                                    LexerMonad::from_other(
                                        Node::SpecialForm(SpecialForm::If),
                                        value.borrow().get_fp(),
                                    )
                                    .into(),
                                    vec_to_list(&[
                                        LexerMonad::from_other(
                                            Node::Symbol(Symbol::Eq),
                                            value.borrow().get_fp(),
                                        )
                                        .into(),
                                        value.clone(),
                                        nil!(value.borrow().get_fp()).into(),
                                    ])
                                    .into(),
                                    value.clone(),
                                    body.into(),
                                ]);
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
                        let params = Rc::new(RefCell::new(cdr)).vectorize_proper_list()?;
                        let mut body = nil!(self.get_end());
                        for param in params.iter().rev() {
                            body = vec_to_list(&[
                                LexerMonad::from_other(
                                    Node::SpecialForm(SpecialForm::If),
                                    param.borrow().get_fp(),
                                )
                                .into(),
                                param.clone(),
                                param.clone(),
                                body.into(),
                            ])
                        }
                        Ok(body)
                    }
                    Node::SpecialForm(SpecialForm::Let) => {
                        // (let ((x1 e11) (x2 e12) ...) e21 e22 ...)
                        // -> ((lambda (x1 x2 ...) (begin e21 e22 ...)) e11 e12 ...)
                        let (bindings, body) = cdr.as_pair()?;
                        let mut keys_node = nil!(self.get_end());
                        let mut values_node = nil!(self.get_end());
                        for binding in bindings.vectorize_proper_list()?.iter().rev() {
                            let (k, v) = binding.borrow().as_pair()?;
                            let (car, _) = v.borrow().as_pair()?;
                            keys_node = LexerMonad::from_other(
                                Node::Pair(k.clone(), keys_node.into()),
                                k.borrow().get_fp(),
                            );
                            values_node = LexerMonad::from_other(
                                Node::Pair(car.clone(), values_node.into()),
                                car.borrow().get_fp(),
                            );
                        }
                        // (begin e21 e22 ...)
                        let body = LexerMonad::from_other(
                            Node::Pair(
                                LexerMonad::from_other(
                                    Node::SpecialForm(SpecialForm::Begin),
                                    self.get_fp(),
                                )
                                .into(),
                                body,
                            ),
                            self.get_fp(),
                        );
                        // (lambda (x1 x2 ...) <body>)
                        let lambda = LexerMonad::from_other(
                            Node::Pair(
                                LexerMonad::from_other(
                                    Node::SpecialForm(SpecialForm::Lambda),
                                    self.get_fp(),
                                )
                                .into(),
                                LexerMonad::from_other(
                                    Node::Pair(
                                        keys_node.into(),
                                        LexerMonad::from_other(
                                            Node::Pair(body.into(), nil!(self.get_fp()).into()),
                                            self.get_fp(),
                                        )
                                        .into(),
                                    ),
                                    self.get_fp(),
                                )
                                .into(),
                            ),
                            self.get_fp(),
                        );
                        // (<lambda> e11 e12 ...)
                        Ok(LexerMonad::from_other(
                            Node::Pair(lambda.into(), values_node.into()),
                            self.get_fp(),
                        ))
                    }
                    _ => Ok(LexerMonad::from_other(
                        Node::Pair(car.into(), cdr.into()),
                        self.get_fp(),
                    )),
                }?;
                Ok(ret)
            }
        }
    }
}
