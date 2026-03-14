//! The data structure of node and other compiled-time objects.
//!
//! This module defines:
//! - [`Node`]: The AST node type used at compile-time
//! - [`NodeRef`]: A reference-counted pointer to a Node
//! - [`PrintableNode`]: A simplified node type for display purposes
//!
//! ## Node Types
//!
//! - `Symbol`: Lisp symbols (built-in or user-defined)
//! - `String`: String literals
//! - `Number`: Integer or floating-point numbers
//! - `Pair`: Cons cells (car and cdr)
//! - `SpecialForm`: Special form identifiers

use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::{self, Display},
    rc::Rc,
};

use crate::{
    lexer::LexerMonad,
    number::Number,
    symbol::{SpecialForm, Symbol},
};

/// The node that can be printed. To print other kinds of nodes, you can
/// transform them to this kind of node.
///
/// This is a simplified representation used for display purposes.
/// It handles circular references by tracking visited nodes.
///
/// # Variants
///
/// - `String(String)`: A string value
/// - `Pair(Rc<PrintableNode>, Rc<PrintableNode>)`: A cons cell
/// - `Nil`: The empty list
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrintableNode {
    String(String),
    Pair(Rc<PrintableNode>, Rc<PrintableNode>),
    Nil,
}

impl From<String> for PrintableNode {
    fn from(value: String) -> Self {
        PrintableNode::String(value)
    }
}

impl Display for PrintableNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut visited = HashMap::new();
        self.fmt_with_visited(f, &mut visited, 0)
    }
}

impl PrintableNode {
    fn fmt_with_visited(
        &self,
        f: &mut fmt::Formatter<'_>,
        visited: &mut HashMap<*const PrintableNode, usize>,
        id: usize,
    ) -> fmt::Result {
        match self {
            Self::Nil => write!(f, "()"),
            Self::String(value) => write!(f, "{value}"),
            Self::Pair(car, cdr) => {
                let cdr_ptr = Rc::<PrintableNode>::as_ptr(cdr);
                if let Some(prev_id) = visited.get(&cdr_ptr) {
                    return write!(f, "#{prev_id}#");
                }
                visited.insert(cdr_ptr, id);

                let car_ptr = Rc::<PrintableNode>::as_ptr(car);
                if let Some(prev_id) = visited.get(&car_ptr) {
                    write!(f, "(#{prev_id}#")?;
                } else {
                    write!(f, "(")?;
                    car.fmt_with_visited(f, visited, id)?;
                    visited.insert(car_ptr, id);
                }

                let mut current = cdr.clone();
                let mut current_id = id;
                loop {
                    let next = {
                        match (*current).clone() {
                            PrintableNode::Pair(next_car, next_cdr) => {
                                let cdr_ptr = Rc::<PrintableNode>::as_ptr(&next_cdr);

                                if let Some(prev_id) = visited.get(&cdr_ptr) {
                                    write!(f, " . #{prev_id}#",)?;
                                    break;
                                }

                                let next_id = current_id + 1;
                                visited.insert(cdr_ptr, next_id);

                                let car_ptr = Rc::<PrintableNode>::as_ptr(&next_car);
                                if let Some(prev_id) = visited.get(&car_ptr) {
                                    write!(f, " #{prev_id}#",)?;
                                } else {
                                    write!(f, " ")?;
                                    next_car.fmt_with_visited(f, visited, next_id)?;
                                    visited.insert(car_ptr, next_id);
                                }
                                Some((next_cdr.clone(), next_id))
                            }
                            PrintableNode::Nil => None,
                            PrintableNode::String(_) => {
                                write!(f, " . {current}")?;
                                None
                            }
                        }
                    };

                    match next {
                        Some((next_cdr, next_id)) => {
                            current = next_cdr;
                            current_id = next_id;
                        }
                        None => break,
                    }
                }
                write!(f, ")")
            }
        }
    }
}

impl From<&LexerMonad<Node>> for PrintableNode {
    fn from(value: &LexerMonad<Node>) -> Self {
        match value.get() {
            Node::Number(s) => PrintableNode::String(s.to_string()),
            Node::SpecialForm(s) => PrintableNode::String(s.to_string()),
            Node::String(s) => PrintableNode::String(s.to_string()),
            Node::Symbol(Symbol::Nil) => PrintableNode::Nil,
            Node::Symbol(s) => PrintableNode::String(s.to_string()),
            Node::Pair(car, cdr) => {
                let car = PrintableNode::from(&*car.borrow());
                let cdr = PrintableNode::from(&*cdr.borrow());
                PrintableNode::Pair(car.into(), cdr.into())
            }
        }
    }
}

/// A reference-counted pointer to a LexerMonad<Node>.
///
/// This type is used throughout the AST to share nodes efficiently.
pub type NodeRef = Rc<RefCell<LexerMonad<Node>>>;

/// The data structure of the node in reference counting graph.
///
/// This is the compile-time AST representation of Lisp expressions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// A Lisp symbol (built-in or user-defined).
    Symbol(Symbol),
    /// A string literal.
    String(String),
    /// An integer or floating-point number.
    Number(Number),
    /// A cons cell (pair) with car and cdr.
    Pair(NodeRef, NodeRef),
    /// A special form identifier.
    SpecialForm(SpecialForm),
}

impl LexerMonad<Node> {
    /// Extracts the user-defined symbol name.
    ///
    /// # Returns
    ///
    /// The symbol name as a String
    ///
    /// # Errors
    ///
    /// Returns an error if the node is not a user-defined symbol
    pub fn as_user_symbol(&self) -> Result<String, String> {
        match self.get() {
            Node::Symbol(Symbol::User(name)) => Ok(name.clone()),
            _ => Err(self.error(format!("{self} is not a user-defined symbol"))),
        }
    }

    /// Extracts car and cdr from a pair node.
    ///
    /// # Returns
    ///
    /// A tuple of (car, cdr) NodeRefs
    ///
    /// # Errors
    ///
    /// Returns an error if the node is not a pair
    pub fn as_pair(&self) -> Result<(NodeRef, NodeRef), String> {
        match self.get() {
            Node::Pair(car, cdr) => Ok((car.clone(), cdr.clone())),
            _ => Err(self.error(format!("{self} is not a pair"))),
        }
    }
}

impl Display for LexerMonad<Node> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", PrintableNode::from(self))
    }
}

impl From<LexerMonad<Node>> for NodeRef {
    fn from(value: LexerMonad<Node>) -> Self {
        RefCell::new(value).into()
    }
}
