//! The parser module.

use std::str::FromStr;

use crate::error::ParseError;
use crate::lexer::{Lexer, TokenType};
use crate::nil;
use crate::node::Node;
use crate::symbol::{SpecialForm, Symbol};

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
        match tokens.peek_next_token() {
            Ok((_, TokenType::RParem)) => {
                // case 1
                tokens.consume(TokenType::RParem)?;
                Ok(nil!())
            }
            _ => {
                let car = Node::parse(tokens)?;
                let cdr = if let Ok((_, TokenType::Dot)) = tokens.peek_next_token() {
                    // case 2
                    tokens.consume(TokenType::Dot)?;
                    let cdr = Node::parse(tokens)?;
                    tokens.consume(TokenType::RParem)?;
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
        match tokens.try_next() {
            Ok(TokenType::LParem) => match tokens.peek_next_token() {
                Ok((_, TokenType::Symbol(symbol))) => match SpecialForm::from_str(symbol.as_str()) {
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
            // '(...) is equivalent to (quote (...)).
            Ok(TokenType::Quote) => Ok(Node::Pair(
                Node::SpecialForm(SpecialForm::Quote).into(),
                Node::Pair(Self::parse(tokens)?.into(), nil!().into()).into(),
            )),
            Ok(TokenType::Number(i)) => Ok(Node::Number(i)),
            // If a special form appears here, it will become a symbol that
            // has the same name as the special form. This is what the user
            // wants when creating a metacircular interpreter.
            Ok(TokenType::Symbol(symbol)) => Ok(Node::Symbol(symbol.into())),
            Ok(TokenType::String(value)) => Ok(Node::String(value)),
            Ok(TokenType::RParem) => Err(ParseError::SyntaxError(format!(
                "At position {}: Unexpected \")\"",
                tokens.get_cur_pos()
            ))),
            Ok(TokenType::Dot) => Err(ParseError::SyntaxError(format!(
                "At position {}: Unexpected \".\"",
                tokens.get_cur_pos()
            ))),
            Err(e) => Err(e),
        }
    }
}
