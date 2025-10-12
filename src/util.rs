//! Utility functions.

use std::{
    cell::RefCell,
    collections::HashMap,
    ffi::c_void,
    fmt::Display,
    rc::Rc,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::{number::Number, nil, node::Node, symbol::Symbol};

pub fn no_less_than_n_params<T>(lst: &[T], n: usize) -> Result<(), String> {
    let x = lst.len();
    if x < n {
        return Err("Fewer parameters than requested".to_string());
    }
    Ok(())
}

pub fn exactly_n_params<T>(lst: &[T], n: usize) -> Result<(), String> {
    let x = lst.len();
    if x > n {
        return Err("More parameters than requested".to_string());
    }
    no_less_than_n_params(lst, n)
}

pub fn get_n_params(lst: Rc<RefCell<Node>>, n: usize) -> Result<Vec<Rc<RefCell<Node>>>, String> {
    let result = vectorize(lst)?;
    exactly_n_params(&result, n)?;
    Ok(result)
}

pub fn map_to_assoc_lst<K, V>(map: &HashMap<K, V>) -> Vec<(K, V)>
where
    K: Clone,
    V: Clone,
{
    map.iter()
        .map(|(name, val)| (name.clone(), val.clone()))
        .collect()
}

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

pub fn vectorize(lst: Rc<RefCell<Node>>) -> Result<Vec<Rc<RefCell<Node>>>, String> {
    let mut cur = lst;
    let mut result = Vec::new();
    loop {
        let next = {
            let node = cur.borrow();
            match &*node {
                Node::Pair(car, cdr) => {
                    result.push(car.clone());
                    Some(cdr.clone())
                }
                _ => None,
            }
        };
        if let Some(next_cur) = next {
            cur = next_cur;
        } else {
            break;
        }
    }
    if *cur.as_ref().borrow() != nil!() {
        return Err("Not a proper list".to_string());
    }
    Ok(result)
}

pub type CVoidFunc = extern "C" fn() -> c_void;

static COUNTER: AtomicUsize = AtomicUsize::new(0);
pub fn inc() -> usize {
    COUNTER.fetch_add(1, Ordering::Relaxed)
}
