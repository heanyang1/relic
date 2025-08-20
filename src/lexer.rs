//! The lexer module.

use std::{
    fmt::Display,
    ops::{Add, Div, Mul, Sub},
};

use crate::error::ParseError;

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
    /// String literal token.
    String(String),
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

    pub fn consume(&mut self, token: TokenType) -> Result<(), ParseError> {
        match self.try_next() {
            Ok(actual) if actual == token => Ok(()),
            Ok(actual) => Err(ParseError::SyntaxError(format!(
                "At position {}: Expected {token:?}, found {actual:?}",
                self.get_cur_pos()
            ))),
            Err(e) => Err(e),
        }
    }

    pub fn consume_symbol(&mut self) -> Result<String, ParseError> {
        match self.try_next() {
            Ok(TokenType::Symbol(sym)) => Ok(sym),
            Ok(actual) => Err(ParseError::SyntaxError(format!(
                "At position {}: Expected symbol, found {actual:?}",
                self.get_cur_pos()
            ))),
            Err(e) => Err(e),
        }
    }

    fn peek_symbol(&self, cur_pos: usize) -> Result<(usize, TokenType), ParseError> {
        let mut symbol = String::new();
        let mut cur_pos = cur_pos;
        while let Some(x) = self.raw.chars().nth(cur_pos) {
            if self.is_special_char(x) {
                break;
            }
            symbol.push(x);
            cur_pos += 1;
        }
        Ok((cur_pos, TokenType::Symbol(symbol)))
    }

    fn peek_number(&self, pos: usize) -> Result<(usize, TokenType), ParseError> {
        let mut cur_pos = pos;
        enum State {
            Start,
            NegSym,
            Integer,
            Dot,
            Float,
        }
        let mut state = State::Start;
        while let Some(x) = self.raw.chars().nth(cur_pos) {
            match (x, &mut state) {
                ('-', State::Start) => state = State::NegSym,
                (x, State::Start) | (x, State::NegSym) | (x, State::Integer)
                    if x.is_ascii_digit() =>
                {
                    state = State::Integer
                }
                ('.', State::Integer) => state = State::Dot,
                (x, State::Dot) | (x, State::Float) if x.is_ascii_digit() => state = State::Float,
                _ => break,
            };
            cur_pos += 1;
        }
        let num_str = self.raw.as_str()[pos..cur_pos].to_string();
        match state {
            // A `-` that does not followed by a digit is parsed as `-` symbol.
            State::NegSym => Ok((cur_pos, TokenType::Symbol("-".to_string()))),
            State::Integer => Ok((
                cur_pos,
                TokenType::Number(Number::Int(
                    num_str.parse::<i64>().unwrap(),
                )),
            )),
            State::Float => Ok((
                cur_pos,
                TokenType::Number(Number::Float(
                    num_str.parse::<f64>().unwrap(),
                )),
            )),
            _ => Err(ParseError::SyntaxError(format!(
                "At position {}: Expected number, found {}",
                pos,
                num_str
            ))),
        }
    }

    /// Peek next token, doesn't consume it or change the lexer's state
    /// unless a comment is met (where the comment is consumed and no token
    /// will be generated).
    pub fn peek_next_token(&mut self) -> Result<(usize, TokenType), ParseError> {
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
                '(' => Ok((cur_pos + 1, TokenType::LParem)),
                ')' => Ok((cur_pos + 1, TokenType::RParem)),
                '\'' => Ok((cur_pos + 1, TokenType::Quote)),
                '.' => Ok((cur_pos + 1, TokenType::Dot)),
                '\"' => {
                    let mut next_pos = cur_pos + 1;
                    while let Some(x) = self.raw.chars().nth(next_pos) {
                        if x == '\"' {
                            break;
                        }
                        next_pos += 1;
                    }
                    Ok((
                        next_pos + 1,
                        TokenType::String(
                            self.raw.as_str()[cur_pos + 1..next_pos].into(),
                        ),
                    ))
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
                x if x == '-' || x.is_ascii_digit() => self.peek_number(cur_pos),
                _ => self.peek_symbol(cur_pos),
            },
            None => Err(ParseError::EOF), // EOF
        }
    }

    pub fn try_next(&mut self) -> Result<TokenType, ParseError> {
        let (next_pos, token) = self.peek_next_token()?;
        self.cur_pos = next_pos;
        Ok(token)
    }
}

impl Iterator for Lexer {
    type Item = TokenType;
    fn next(&mut self) -> Option<Self::Item> {
        match self.try_next() {
            Ok(token) => Some(token),
            Err(ParseError::EOF) => None,
            Err(e) => panic!("lexer error: {e}"),
        }
    }
}
