// Allow automatic conversion from String to RuntimeError
impl From<String> for RuntimeError {
    fn from(message: String) -> Self {
        RuntimeError::new(message)
    }
}

use std::backtrace::Backtrace;

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

impl std::fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}\nBacktrace:\n{}", self.message, self.backtrace)
    }
}

impl std::error::Error for RuntimeError {}
