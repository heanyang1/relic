//! The lexer module.

use std::{
    fmt::Display,
    ops::{Add, Div, Mul, Sub},
};

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

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum TokenType {
    /// Token `(`.
    LParem,
    /// Token `)`.
    RParem,
    /// Token `'`.
    Quote,
    /// Token `.`.
    Dot,
    /// Number token, can be either `Int` or `Float`.
    Number(Number),
    /// Symbol token. Lexer doesn't process symbol.
    Symbol(String),
}

pub struct Lexer {
    raw: String,
    cur_pos: usize,
}

impl Lexer {
    pub fn new<T>(s: T) -> Lexer
    where
        T: ToString,
    {
        Lexer {
            raw: s.to_string(),
            cur_pos: 0,
        }
    }

    pub fn get_cur_pos(&self) -> usize {
        self.cur_pos
    }

    fn is_whitespace(&self, x: char) -> bool {
        x == ' ' || x == '\n' || x == '\t'
    }

    fn is_special_char(&self, x: char) -> bool {
        match x {
            '(' | ')' | '\'' => true,
            _ => self.is_whitespace(x),
        }
    }

    pub fn consume(&mut self, token: TokenType) -> Result<(), String> {
        match self.next() {
            Some(actual) if actual == token => Ok(()),
            actual => Err(format!(
                "At position {}: Expected {token:?}, found {actual:?}",
                self.get_cur_pos()
            )),
        }
    }

    pub fn consume_symbol(&mut self) -> Result<String, String> {
        match self.next() {
            Some(TokenType::Symbol(sym)) => Ok(sym),
            actual => Err(format!(
                "At position {}: Expected symbol, found {actual:?}",
                self.get_cur_pos()
            )),
        }
    }

    fn peek_symbol(&self, cur_pos: usize) -> (usize, Option<TokenType>) {
        let mut symbol = String::new();
        let mut cur_pos = cur_pos;
        while let Some(x) = self.raw.chars().nth(cur_pos) {
            if self.is_special_char(x) {
                break;
            }
            symbol.push(x);
            cur_pos += 1;
        }
        (cur_pos, Some(TokenType::Symbol(symbol)))
    }

    fn peek_number(&self, pos: usize) -> (usize, Option<TokenType>) {
        let mut cur_pos = pos;
        while let Some(x) = self.raw.chars().nth(cur_pos) {
            if !x.is_ascii_digit() && x != '.' {
                break;
            }
            cur_pos += 1;
        }
        let num_str = self.raw.as_str()[pos..cur_pos].to_string();
        if num_str.is_empty() {
            return (cur_pos, None);
        }
        if num_str.contains('.') {
            match num_str.parse::<f64>() {
                Ok(num) => (cur_pos, Some(TokenType::Number(Number::Float(num)))),
                Err(_) => (cur_pos, None),
            }
        } else {
            match num_str.parse::<i64>() {
                Ok(num) => (cur_pos, Some(TokenType::Number(Number::Int(num)))),
                Err(_) => (cur_pos, None),
            }
        }
    }

    /// Peek next token, doesn't consume it or change the lexer's state
    /// unless a comment is met (where the comment is consumed and no token
    /// will be generated).
    pub fn peek_next_token(&mut self) -> (usize, Option<TokenType>) {
        let mut cur_pos = self.cur_pos;
        // remove whitespace
        while let Some(x) = self.raw.chars().nth(cur_pos) {
            if self.is_whitespace(x) {
                cur_pos += 1;
            } else {
                break;
            }
        }
        match self.raw.chars().nth(cur_pos) {
            Some(ch) => match ch {
                '(' => (cur_pos + 1, Some(TokenType::LParem)),
                ')' => (cur_pos + 1, Some(TokenType::RParem)),
                '\'' => (cur_pos + 1, Some(TokenType::Quote)),
                '.' => (cur_pos + 1, Some(TokenType::Dot)),
                '\"' => {
                    let mut next_pos = cur_pos + 1;
                    while let Some(x) = self.raw.chars().nth(next_pos) {
                        if x == '\"' {
                            break;
                        }
                        next_pos += 1;
                    }
                    (
                        next_pos + 1,
                        Some(TokenType::Symbol(
                            self.raw.as_str()[cur_pos + 1..next_pos].into(),
                        )),
                    )
                }
                // Comment starts with `;` and ends with `\n`.
                ';' => {
                    let mut next_pos = cur_pos + 1;
                    while let Some(x) = self.raw.chars().nth(next_pos) {
                        if x == '\n' {
                            break;
                        }
                        next_pos += 1;
                    }
                    self.cur_pos = next_pos;
                    self.peek_next_token()
                }
                x if x.is_ascii_digit() => self.peek_number(cur_pos),
                _ => self.peek_symbol(cur_pos),
            },
            None => (cur_pos, None), // EOF
        }
    }
}

impl Iterator for Lexer {
    type Item = TokenType;
    fn next(&mut self) -> Option<Self::Item> {
        let (next_pos, token) = self.peek_next_token();
        self.cur_pos = next_pos;
        token
    }
}
