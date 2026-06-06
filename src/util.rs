//! Utility functions.
//!
//! This module provides various helper functions used throughout Relic:
//! - Parameter validation
//! - Arithmetic and relational evaluation
//! - List/vector operations
//! - Atomic counters

use std::{
    collections::HashMap,
    ffi::c_void,
    fmt::Display,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{
    nil,
    node::{Node, NodeRef},
    number::Number,
    symbol::Symbol,
};

/// Validates that a list has at least n parameters.
///
/// # Parameters
///
/// * `lst` - The list to check
/// * `n` - Minimum required length
///
/// # Errors
/// Returns an error if list length < n
pub fn no_less_than_n_params<T>(lst: &[T], n: usize) -> Result<(), String> {
    let x = lst.len();
    if x < n {
        return Err("Fewer parameters than requested".to_string());
    }
    Ok(())
}

/// Validates that a list has exactly n parameters.
///
/// # Parameters
///
/// * `lst` - The list to check
/// * `n` - Exact required length
///
/// # Errors
/// Returns an error if list length != n
pub fn exactly_n_params<T>(lst: &[T], n: usize) -> Result<(), String> {
    let x = lst.len();
    if x > n {
        return Err("More parameters than requested".to_string());
    }
    no_less_than_n_params(lst, n)
}

/// Extracts exactly n parameters from a proper list node.
///
/// # Parameters
///
/// * `lst` - A NodeRef pointing to a proper list
/// * `n` - Number of parameters to extract
///
/// # Returns
/// A vector of NodeRefs
///
/// # Errors
/// Returns error if not a proper list or wrong parameter count
pub fn get_n_params(lst: NodeRef, n: usize) -> Result<Vec<NodeRef>, String> {
    let result = lst.vectorize_proper_list()?;
    exactly_n_params(&result, n)?;
    Ok(result)
}

/// Converts a HashMap to a vector of key-value pairs.
///
/// Used for GC to iterate over environment bindings.
pub fn map_to_assoc_lst<K, V>(map: &HashMap<K, V>) -> Vec<(K, V)>
where
    K: Clone,
    V: Clone,
{
    map.iter()
        .map(|(name, val)| (name.clone(), val.clone()))
        .collect()
}

/// Evaluates an arithmetic operation on a list of values.
///
/// # Parameters
///
/// * `values` - Values to operate on
/// * `op` - The binary operation function
///
/// # Returns
/// The result of folding the operation
///
/// # Errors
/// Returns error if fewer than 2 values provided
pub fn eval_arith<N, Op>(values: Vec<N>, op: Op) -> Result<Number, String>
where
    Op: Fn(Number, Number) -> Number,
    N: TryInto<Number>,
    <N as TryInto<Number>>::Error: Display,
{
    no_less_than_n_params(&values, 2)?;
    let mut numbers: Vec<Number> = vec![];
    for value in values {
        numbers.push(value.try_into().map_err(|e| format!("{e}"))?);
    }
    let first = numbers[0].clone();
    Ok(numbers.into_iter().skip(1).fold(first, op))
}

/// Evaluates a relational operation on exactly two values.
///
/// # Parameters
///
/// * `values` - Two values to compare
/// * `op` - The binary comparison function
///
/// # Returns
/// Symbol::T if true, Symbol::Nil if false
///
/// # Errors
/// Returns error if not exactly 2 values
pub fn eval_rel<N, Op>(values: Vec<N>, op: Op) -> Result<Symbol, String>
where
    Op: Fn(Number, Number) -> bool,
    N: TryInto<Number>,
    <N as TryInto<Number>>::Error: Display,
{
    exactly_n_params(&values, 2)?;
    let mut numbers: Vec<Number> = vec![];
    for value in values {
        numbers.push(value.try_into().map_err(|e| format!("{e}"))?);
    }
    Ok(if op(numbers[0].clone(), numbers[1].clone()) {
        Symbol::T
    } else {
        Symbol::Nil
    })
}

pub trait Vectorize
where
    Self: Sized,
{
    type ErrorType;
    /// Parse a node as a vector of nodes:
    /// - If it the node is a proper list, return (true, nodes),
    /// - If it the node is a improper list, return (false, nodes).
    fn vectorize(self) -> (bool, Vec<Self>);
    /// A wrapper for the [Vectorize::vectorize] function.
    fn vectorize_proper_list(self) -> Result<Vec<Self>, Self::ErrorType>;
}

impl Vectorize for NodeRef {
    type ErrorType = String;
    fn vectorize(self) -> (bool, Vec<Self>) {
        let mut cur = self.clone();
        let mut result = Vec::new();
        enum State {
            Continue(NodeRef),
            ProperList,
            ImproperList(NodeRef),
        }
        loop {
            let next = {
                match cur.borrow().get() {
                    Node::Pair(car, cdr) => {
                        result.push(car.clone());
                        State::Continue(cdr.clone())
                    }
                    nil!() => State::ProperList,
                    _ => State::ImproperList(cur.clone()),
                }
            };
            match next {
                State::Continue(next_cur) => cur = next_cur,
                State::ProperList => return (true, result),
                State::ImproperList(last) => {
                    result.push(last);
                    return (false, result);
                }
            }
        }
    }
    fn vectorize_proper_list(self) -> Result<Vec<Self>, Self::ErrorType> {
        let (is_proper_list, value) = self.clone().vectorize();
        if !is_proper_list {
            Err(self
                .borrow()
                .error(format!("{} is not a proper list", self.borrow())))
        } else {
            Ok(value)
        }
    }
}

/// A C-compatible function pointer type.
///
/// Used for closure FFI with the Relic runtime.
pub type CVoidFunc = extern "C" fn() -> c_void;

/// Atomically increments a counter and returns the new value.
///
/// Used for generating unique IDs for closures and JIT compilations.
static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// Increments the global counter and returns the new value.
pub fn inc() -> usize {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Returns the maximum of two values.
pub fn max<T>(a: T, b: T) -> T
where
    T: PartialOrd,
{
    if a >= b { a } else { b }
}
/// Returns the minimum of two values.
pub fn min<T>(a: T, b: T) -> T
where
    T: PartialOrd,
{
    if a <= b { a } else { b }
}
/// Computes the union of two intervals.
///
/// Used for combining source location ranges in the lexer.
///
/// # Parameters
///
/// * `a` - First interval as (min, max)
/// * `b` - Second interval as (min, max)
///
/// # Returns
/// The combined interval (min, max)
pub fn interval_union<T>(a: (T, T), b: (T, T)) -> (T, T)
where
    T: PartialOrd,
{
    (min(a.0, b.0), max(a.1, b.1))
}

#[test]
fn test_min() {
    assert_eq!(min(1, 2), 1);
    assert_eq!(min(2, 1), 1);
    assert_eq!(min(2.0, 1.0), 1.0);
}

#[test]
fn test_max() {
    assert_eq!(max(1, 2), 2);
    assert_eq!(max(2, 1), 2);
    assert_eq!(max(2.0, 1.0), 2.0);
}

#[test]
fn test_union() {
    assert_eq!(interval_union((2, 3), (1, 2)), (1, 3));
    assert_eq!(interval_union((2, 4), (1, 3)), (1, 4));
    assert_eq!(interval_union((1, 2), (2, 3)), (1, 3));
    assert_eq!(interval_union((1, 3), (2, 4)), (1, 4));
    assert_eq!(interval_union((1, 2), (3, 4)), (1, 4));
}
