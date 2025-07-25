//! The parser module.

use std::str::FromStr;

use crate::lexer::{Lexer, TokenType};
use crate::nil;
use crate::node::Node;
use crate::symbol::{SpecialForm, Symbol};

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    SyntaxError(String),
    EOF,
}

impl ToString for ParseError {
    fn to_string(&self) -> String {
        match self {
            ParseError::SyntaxError(s) => s.clone(),
            ParseError::EOF => "Unexpected EOF".to_string(),
        }
    }
}

pub trait Parse {
    fn parse(tokens: &mut Lexer) -> Result<Self, ParseError>
    where
        Self: Sized;
}
impl Node {
    /// Parse the list according to the following BNF:
    ///
    /// ```ignore
    /// List ::= Lparem [SpecialForm] ListWithoutLparem;
    /// ListWithoutLparem ::= Rparem                    // 1
    ///                     | Expr ListWithoutLparem    // 2
    ///                     | Expr "." Expr Rparem;     // 3
    /// ```
    ///
    /// The `Lparem` and `SpecialForm` are already stripped when the function is called.
    fn parse_list(tokens: &mut Lexer) -> Result<Self, ParseError> {
        match tokens.peek_next_token().1 {
            Some(TokenType::RParem) => {
                // case 1
                tokens
                    .consume(TokenType::RParem)
                    .map_err(|e| ParseError::SyntaxError(e))?;
                Ok(nil!())
            }
            Some(TokenType::Comment) => {
                tokens
                    .consume(TokenType::Comment)
                    .map_err(|e| ParseError::SyntaxError(e))?;
                Self::parse_list(tokens)
            }
            _ => {
                let car = Node::parse(tokens)?;
                let cdr = if let Some(TokenType::Dot) = tokens.peek_next_token().1 {
                    // case 2
                    tokens
                        .consume(TokenType::Dot)
                        .map_err(|e| ParseError::SyntaxError(e))?;
                    let cdr = Node::parse(tokens)?;
                    tokens
                        .consume(TokenType::RParem)
                        .map_err(|e| ParseError::SyntaxError(e))?;
                    cdr
                } else {
                    // case 3
                    Self::parse_list(tokens)?
                };

                Ok(Node::Pair(car.into(), cdr.into()))
            }
        }
    }
}

impl Parse for Node {
    fn parse(tokens: &mut Lexer) -> Result<Self, ParseError> {
        match tokens.next() {
            Some(TokenType::LParem) => match tokens.peek_next_token().1 {
                Some(TokenType::Symbol(symbol)) => match SpecialForm::from_str(symbol.as_str()) {
                    Ok(form) => {
                        tokens.consume_symbol().unwrap();
                        Ok(Node::Pair(
                            Node::SpecialForm(form).into(),
                            Self::parse_list(tokens)?.into(),
                        ))
                    }
                    Err(_) => Self::parse_list(tokens),
                },
                _ => Self::parse_list(tokens),
            },
            Some(TokenType::Quote) => {
                // '(...) is equivalent to (quote (...))
                Ok(Node::Pair(
                    Node::SpecialForm(SpecialForm::Quote).into(),
                    Node::Pair(Self::parse(tokens)?.into(), nil!().into()).into(),
                ))
            }
            Some(TokenType::Number(i)) => Ok(Node::Number(i)),
            Some(TokenType::Symbol(symbol)) => {
                // If a special form appears here, it will become a symbol that
                // has the same name as the special form. This is what the user
                // wants when creating a metacircular interpreter.
                Ok(Node::Symbol(symbol.into()))
            }
            Some(TokenType::RParem) => Err(ParseError::SyntaxError(format!(
                "At position {}: Unexpected \")\"",
                tokens.get_cur_pos()
            ))),
            Some(TokenType::Dot) => Err(ParseError::SyntaxError(format!(
                "At position {}: Unexpected \".\"",
                tokens.get_cur_pos()
            ))),
            Some(TokenType::Comment) => Self::parse(tokens),
            None => Err(ParseError::EOF),
        }
    }
}
