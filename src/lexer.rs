//! The lexer module.

use crate::{error::ParseError, number::Number};

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
                TokenType::Number(Number::Int(num_str.parse::<i64>().unwrap())),
            )),
            State::Float => Ok((
                cur_pos,
                TokenType::Number(Number::Float(num_str.parse::<f64>().unwrap())),
            )),
            _ => Err(ParseError::SyntaxError(format!(
                "At position {pos}: Expected number, found {num_str}"
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
                        TokenType::String(self.raw.as_str()[cur_pos + 1..next_pos].into()),
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
