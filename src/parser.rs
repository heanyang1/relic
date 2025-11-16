//! The parser module.

use std::cell::RefCell;
use std::rc::Rc;
use std::str::FromStr;

use crate::error::ParseError;
use crate::lexer::LexerMonad;
use crate::lexer::Token;
use crate::nil;
use crate::node::Node;
use crate::node::NodeRef;
use crate::symbol::{SpecialForm, Symbol};

#[cfg(test)]
use crate::number::Number;

pub fn new_pair(car: NodeRef, cdr: NodeRef) -> LexerMonad<Node> {
    let cdr_fp = cdr.borrow().get_fp();
    let car_fp = car.borrow().get_fp();
    LexerMonad::from_other(
        Node::Pair(car.clone(), cdr.clone()),
        car_fp.bind(|_| cdr_fp.clone()).join(),
    )
}

/// Generate a list node from a vector of node monads.
pub fn vec_to_list(vec: &[NodeRef]) -> LexerMonad<Node> {
    match vec {
        [] => unreachable!(),
        [e] => new_pair(e.clone(), nil!(e.borrow().get_end()).into()),
        [e, rest @ ..] => new_pair(e.clone(), vec_to_list(rest).into()),
    }
}

impl LexerMonad<()> {
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
    ///
    /// The monad's data will be dropped and be replaced by the parsed node.
    fn parse_list(self) -> Result<LexerMonad<Node>, ParseError> {
        let begin = self.get_end().next_token()?;
        match begin.get() {
            Token::RParem => {
                // case 1
                Ok::<LexerMonad<Node>, ParseError>(nil!(self.consume(Token::RParem)?.get_end()))
            }
            _ => {
                let car = self.get_fp().parse()?;
                let cur = car.get_end();
                let cdr = cur.get_end().next_token()?;
                let cdr = match cdr.get() {
                    Token::Dot => {
                        // case 2
                        cur.get_begin()
                            .consume(Token::Dot)?
                            .parse()?
                            .consume(Token::RParem)
                    }
                    _ => {
                        // case 3
                        cur.get_begin().parse_list()
                    }
                }?;
                Ok(new_pair(car.into(), cdr.into()))
            }
        }
    }

    /// Parse a node.
    ///
    /// The monad's data will be dropped and be replaced by the parsed node.
    pub fn parse(self) -> Result<LexerMonad<Node>, ParseError> {
        let first_token = self.get_end().next_token()?;
        let behind_first = first_token.get_end();
        match first_token.get() {
            Token::LParem => {
                let second_token = behind_first.get_end().next_token()?;
                let behind_second = second_token.get_end();
                match second_token.get() {
                    Token::Symbol(symbol) => match SpecialForm::from_str(symbol.as_str()) {
                        Ok(form) => {
                            let car = LexerMonad::from_other(
                                Node::SpecialForm(form),
                                second_token.get_fp(),
                            );
                            let cdr = behind_second.get_end().parse_list()?;
                            let end_fp = cdr.get_end();
                            Ok(LexerMonad::from_other(
                                Node::Pair(car.into(), cdr.into()),
                                end_fp,
                            ))
                        }
                        Err(_) => behind_first.get_end().parse_list(),
                    },
                    _ => behind_first.get_end().parse_list(),
                }
            }
            // '(...) is equivalent to (quote (...)).
            Token::Quote => {
                let car = LexerMonad::from_other(
                    Node::SpecialForm(SpecialForm::Quote),
                    first_token.get_fp(),
                );
                Ok(vec_to_list(&[
                    car.into(),
                    behind_first.get_end().parse()?.into(),
                ]))
            }
            Token::Number(i) => Ok(LexerMonad::from_other(
                Node::Number(i.clone()),
                first_token.get_fp(),
            )),
            // If a special form appears here, it will become a symbol that
            // has the same name as the special form. This is what the user
            // wants when creating a metacircular interpreter.
            Token::Symbol(symbol) => Ok(LexerMonad::from_other(
                Node::Symbol(symbol.into()),
                first_token.get_fp(),
            )),
            Token::String(value) => Ok(LexerMonad::from_other(
                Node::String(value.clone()),
                first_token.get_fp(),
            )),
            Token::RParem => Err(ParseError::SyntaxError(
                first_token.error(" Unexpected \")\""),
            )),
            Token::Dot => Err(ParseError::SyntaxError(
                first_token.error("Unexpected \".\""),
            )),
        }
    }

    /// Parse all nodes and make a list node of these nodes.
    pub fn parse_all(self) -> Result<NodeRef, ParseError> {
        let mut cur_lexer = self;
        let mut nodes = vec![];
        loop {
            match cur_lexer.get_fp().parse() {
                Ok(node) => {
                    cur_lexer = node.get_fp();
                    nodes.push(node);
                }
                Err(ParseError::EOF) => break,
                Err(x) => return Err(x),
            }
        }
        if nodes.is_empty() {
            return Err(ParseError::EOF);
        }
        let mut cur_node = Rc::new(RefCell::new(nil!(nodes.last().unwrap().get_end())));
        for node in nodes.iter().rev() {
            cur_node = new_pair(Rc::new(RefCell::new(node.clone())), cur_node).into();
        }
        Ok(cur_node)
    }
}

#[cfg(test)]
macro_rules! test_new {
    ($item:expr, $input:expr, $cb:expr, $len:expr) => {
        LexerMonad::test_new(
            $item,
            $input.to_string(),
            1,
            $cb,
            $cb,
            1,
            $cb + $len,
            $cb + $len,
        )
    };
}

#[cfg(test)]
macro_rules! test_number {
    ($item:expr, $input:expr, $cb:expr, $len:expr) => {
        test_new!(Node::Number($item), $input, $cb, $len)
    };
}

#[cfg(test)]
macro_rules! test_symbol {
    ($item:expr, $input:expr, $cb:expr, $len:expr) => {
        test_new!(Node::Symbol($item), $input, $cb, $len)
    };
}

#[cfg(test)]
macro_rules! test_pair {
    ($car:expr, $cdr:expr, $input:expr, $cb:expr, $len:expr) => {
        test_new!(Node::Pair($car.into(), $cdr.into()), $input, $cb, $len)
    };
}

#[test]
fn test_parse_number() {
    let input = "42";
    assert_eq!(
        LexerMonad::new_unnamed(input).parse().unwrap(),
        test_number!(Number::Int(42), input, 0, 2)
    );
    let input = "3.14159265358979323846";
    assert_eq!(
        LexerMonad::new_unnamed(input).parse().unwrap(),
        test_number!(Number::Float(3.141_592_653_589_793), input, 0, 22)
    );
}

#[test]
fn test_parse_symbol() {
    let input = "x";
    assert_eq!(
        LexerMonad::new_unnamed(input).parse().unwrap(),
        test_symbol!(Symbol::User("x".to_string()), input, 0, 1)
    );
}

#[test]
fn test_parse_add() {
    let input = "+";
    assert_eq!(
        LexerMonad::new_unnamed(input).parse().unwrap(),
        test_symbol!(Symbol::Add, input.to_string(), 0, 1)
    );
}

#[test]
fn test_parse_sexp() {
    let input = "(+ 1 2)";
    assert_eq!(
        LexerMonad::new_unnamed(input).parse().unwrap(),
        test_pair!(
            test_symbol!(Symbol::Add, input, 1, 1),
            test_pair!(
                test_number!(Number::Int(1), input, 3, 1),
                test_pair!(
                    test_number!(Number::Int(2), input, 5, 1),
                    test_symbol!(Symbol::Nil, input, 7, 0),
                    input,
                    5,
                    2
                ),
                input,
                3,
                4
            ),
            input,
            1,
            6
        )
    );
}

#[test]
fn test_nested_expressions() {
    let input = "(+ (* 2 3) (- 5 1))";

    let ret = test_pair!(
        test_symbol!(Symbol::Add, input, 1, 1),
        test_pair!(
            test_pair!(
                test_symbol!(Symbol::Mul, input, 4, 1),
                test_pair!(
                    test_number!(Number::Int(2), input, 6, 1),
                    test_pair!(
                        test_number!(Number::Int(3), input, 8, 1),
                        test_symbol!(Symbol::Nil, input, 10, 0),
                        input,
                        8,
                        2
                    ),
                    input,
                    6,
                    4
                ),
                input,
                4,
                6
            ),
            test_pair!(
                test_pair!(
                    test_symbol!(Symbol::Sub, input, 12, 1),
                    test_pair!(
                        test_number!(Number::Int(5), input, 14, 1),
                        test_pair!(
                            test_number!(Number::Int(1), input, 16, 1),
                            test_symbol!(Symbol::Nil, input, 18, 0),
                            input,
                            16,
                            2
                        ),
                        input,
                        14,
                        4
                    ),
                    input,
                    12,
                    6
                ),
                test_symbol!(Symbol::Nil, input, 19, 0),
                input,
                12,
                7
            ),
            input,
            4,
            15
        ),
        input,
        1,
        18
    );
    assert_eq!(LexerMonad::new_unnamed(input).parse().unwrap(), ret);
}

#[test]
fn test_pair() {
    let input = "(1 . 2)";
    assert_eq!(
        LexerMonad::new_unnamed(input).parse().unwrap(),
        test_pair!(
            test_number!(Number::Int(1), input, 1, 1),
            test_number!(Number::Int(2), input, 6, 1),
            input,
            1,
            6
        )
    );
}

#[test]
fn test_empty_sexp() {
    let input = "()";
    assert_eq!(
        LexerMonad::new_unnamed(input).parse().unwrap(),
        test_symbol!(Symbol::Nil, input, 2, 0)
    );
}

#[test]
fn test_comment() {
    let input = "(;\n);;";
    assert_eq!(
        LexerMonad::new_unnamed(input).parse().unwrap(),
        LexerMonad::test_new(
            Node::Symbol(Symbol::Nil),
            input.to_string(),
            2,
            0,
            4,
            2,
            0,
            4
        )
    );
}

#[test]
fn test_quote() {
    let input = "'(() '())";
    // (quote (() (quote ())))
    assert_eq!(
        LexerMonad::new_unnamed(input).parse().unwrap(),
        test_pair!(
            test_new!(Node::SpecialForm(SpecialForm::Quote), input, 0, 1),
            test_pair!(
                test_pair!(
                    test_symbol!(Symbol::Nil, input, 4, 0),
                    test_pair!(
                        test_pair!(
                            test_new!(Node::SpecialForm(SpecialForm::Quote), input, 5, 1),
                            test_pair!(
                                test_symbol!(Symbol::Nil, input, 8, 0),
                                test_symbol!(Symbol::Nil, input, 8, 0),
                                input,
                                8,
                                0
                            ),
                            input,
                            5,
                            3
                        ),
                        test_symbol!(Symbol::Nil, input, 9, 0),
                        input,
                        5,
                        4
                    ),
                    input,
                    4,
                    5
                ),
                test_symbol!(Symbol::Nil, input, 9, 0),
                input,
                4,
                5
            ),
            input,
            0,
            9
        )
    );
}

#[test]
fn test_invalid_statement() {
    // "x)" is valid for one statement (which will be parsed as a symbol "x"),
    // but it is not a valid program
    let inputs = [
        "(",
        ")",
        "(def x",
        "(((()(())())",
        "(1 2 .)",
        "(. 1)",
        "(1 . 2 3)",
        ".",
    ];

    for input in &inputs {
        assert!(LexerMonad::new_unnamed(input).parse().is_err());
    }
}
