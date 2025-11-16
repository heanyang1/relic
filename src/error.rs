// Allow automatic conversion from String to RuntimeError
impl From<String> for RuntimeError {
    fn from(message: String) -> Self {
        RuntimeError::new(message)
    }
}

use std::{
    backtrace::Backtrace,
    error::Error,
    fmt::{Display, Formatter},
};

#[derive(Debug)]
pub struct RuntimeError {
    pub message: String,
    pub backtrace: Backtrace,
}

impl RuntimeError {
    pub fn new(message: impl Into<String>) -> Self {
        RuntimeError {
            message: message.into(),
            backtrace: Backtrace::capture(),
        }
    }
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\nBacktrace:\n{}", self.message, self.backtrace)
    }
}

impl Error for RuntimeError {}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseError {
    SyntaxError(String),
    EOF,
}

impl Display for ParseError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                ParseError::SyntaxError(s) => s,
                ParseError::EOF => "Unexpected EOF",
            }
        )
    }
}
impl From<String> for ParseError {
    fn from(value: String) -> Self {
        ParseError::SyntaxError(value)
    }
}
impl From<ParseError> for String {
    fn from(val: ParseError) -> Self {
        format!("{val}")
    }
}

impl Error for ParseError {}

/// Map error in result if the error type is convertible.
pub trait MapErr<T, Ein, Eout>
where
    Ein: Into<Eout>,
{
    fn map_err_simple(self) -> Result<T, Eout>;
}

impl<T, Ein, Eout> MapErr<T, Ein, Eout> for Result<T, Ein>
where
    Ein: Into<Eout>,
{
    fn map_err_simple(self) -> Result<T, Eout> {
        self.map_err(|e| e.into())
    }
}
