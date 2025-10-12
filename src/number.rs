//! Number representation.

use std::{fmt::Display, ops::{Add, Div, Mul, Sub}};

/// Numbers in Relic is an `i64` or an `f64`. Integer numbers automatically
/// cast to floating-point number when needed.
#[derive(Debug, Clone)]
pub enum Number {
    Int(i64),
    Float(f64),
}

impl Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Number::Int(i) => write!(f, "{i}"),
            Number::Float(fl) => write!(f, "{fl}"),
        }
    }
}

macro_rules! arith_op {
    ($op:tt, $lhs:expr, $rhs:expr) => {
        match ($lhs, $rhs) {
            (Number::Int(a), Number::Int(b)) => Number::Int(a $op b),
            (Number::Float(a), Number::Float(b)) => Number::Float(a $op b),
            (Number::Int(a), Number::Float(b)) => Number::Float(a as f64 $op b),
            (Number::Float(a), Number::Int(b)) => Number::Float(a $op b as f64),
        }
    };
}

macro_rules! rel_op {
    ($op:tt, $lhs:expr, $rhs:expr) => {
        match ($lhs, $rhs) {
            (Number::Int(a), Number::Int(b)) => a $op b,
            (Number::Float(a), Number::Float(b)) => a $op b,
            (Number::Int(a), Number::Float(b)) => (*a as f64) $op *b,
            (Number::Float(a), Number::Int(b)) => *a $op (*b as f64),
        }
    };
}

impl Add for Number {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        arith_op!(+, self, rhs)
    }
}

impl Sub for Number {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        arith_op!(-, self, rhs)
    }
}

impl Mul for Number {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self::Output {
        arith_op!(*, self, rhs)
    }
}

impl From<Number> for f64 {
    fn from(value: Number) -> Self {
        match value {
            Number::Int(i) => i as f64,
            Number::Float(fl) => fl,
        }
    }
}

impl TryFrom<Number> for usize {
    type Error = String;
    fn try_from(value: Number) -> Result<Self, Self::Error> {
        match value {
            Number::Int(i) if i >= 0 => Ok(i as usize),
            _ => Err(format!("Can not cast {value} to usize")),
        }
    }
}

impl Div for Number {
    type Output = Self;
    fn div(self, rhs: Self) -> Self::Output {
        Number::Float(f64::from(self) / f64::from(rhs))
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        rel_op!(==, self, other)
    }
}

impl PartialOrd for Number {
    fn gt(&self, other: &Self) -> bool {
        rel_op!(>, self, other)
    }
    fn ge(&self, other: &Self) -> bool {
        rel_op!(>=, self, other)
    }
    fn lt(&self, other: &Self) -> bool {
        rel_op!(<, self, other)
    }
    fn le(&self, other: &Self) -> bool {
        rel_op!(<=, self, other)
    }
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        if self < other {
            Some(std::cmp::Ordering::Less)
        } else if self > other {
            Some(std::cmp::Ordering::Greater)
        } else {
            Some(std::cmp::Ordering::Equal)
        }
    }
}

impl Eq for Number {}
