//! The logger module.
//!
//! Provides simple logging functionality with configurable log levels.
//! Messages are colored based on severity and can be filtered.
//!
//! ## Usage
//!
//! Set the `LOG_LEVEL` environment variable to control logging:
//! - `DEBUG`: Show all messages
//! - `WARNING`: Show warnings and errors (default)
//! - `ERROR`: Show only errors
//!
//! ## Functions
//!
//! - [`log_debug`]: Log a debug message
//! - [`log_warning`]: Log a warning message
//! - [`log_error`]: Log an error message
//! - [`set_log_level`]: Change the log level programmatically

use std::{
    str::FromStr,
    sync::{LazyLock, Mutex},
};

use colored::Colorize;

/// Log level for filtering messages.
///
/// Levels are ordered: Debug < Warning < Error
#[derive(PartialEq, PartialOrd)]
pub enum LogLevel {
    /// Debug messages (most verbose)
    Debug = 0,
    /// Warning messages
    Warning = 1,
    /// Error messages (least verbose)
    Error = 2,
}

impl FromStr for LogLevel {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "DEBUG" => Ok(LogLevel::Debug),
            "WARNING" => Ok(LogLevel::Warning),
            "ERROR" => Ok(LogLevel::Error),
            _ => Err(format!("Unknown log level: {s}")),
        }
    }
}

/// The logger that writes colored messages to stdout.
pub struct Logger {
    /// The current minimum log level.
    level: LogLevel,
}

/// A very simple logger.
impl Logger {
    fn new() -> Self {
        let level = std::env::var("LOG_LEVEL")
            .unwrap_or_else(|_| "WARNING".into())
            .parse()
            .unwrap();

        Logger { level }
    }

    fn set_log_level(&mut self, level: LogLevel) {
        self.level = level;
    }

    fn write(&mut self, msg: &str, color: &str) {
        println!("{}", msg.color(color))
    }

    fn debug(&mut self, msg: String) {
        if self.level <= LogLevel::Debug {
            let message = format!("[DEBUG] {msg}");
            self.write(&message, "blue");
        }
    }
    fn warning(&mut self, msg: String) {
        if self.level <= LogLevel::Warning {
            let message = format!("[WARNING] {msg}");
            self.write(&message, "yellow");
        }
    }
    fn error(&mut self, msg: String) {
        if self.level <= LogLevel::Error {
            let message = format!("[ERROR] {msg}");
            self.write(&message, "red");
        }
    }
}

/// Global logger instance.
pub static LOGGER: LazyLock<Mutex<Logger>> = LazyLock::new(|| Mutex::new(Logger::new()));

/// Logs a debug message.
///
/// # Parameters
///
/// * `msg` - The message to log (anything that implements ToString)
pub fn log_debug<T>(msg: T)
where
    T: ToString,
{
    LOGGER.lock().unwrap().debug(msg.to_string());
}
/// Logs a warning message.
pub fn log_warning<T>(msg: T)
where
    T: ToString,
{
    LOGGER.lock().unwrap().warning(msg.to_string());
}
/// Logs an error message.
pub fn log_error<T>(msg: T)
where
    T: ToString,
{
    LOGGER.lock().unwrap().error(msg.to_string());
}
/// Sets the global log level.
///
/// # Parameters
///
/// * `level` - The new log level
pub fn set_log_level(level: LogLevel) {
    LOGGER.lock().unwrap().set_log_level(level);
}

#[test]
fn test_logger() {
    let mut logger = Logger::new();
    logger.set_log_level(LogLevel::Debug);
    logger.debug("This is a debug message".to_string());
    logger.warning("This is a warning message".to_string());
    logger.error("This is an error message".to_string());
    logger.set_log_level(LogLevel::Warning);
    logger.debug("This debug message should not be printed".to_string());
    logger.warning("This is another warning message".to_string());
    logger.error("This is another error message".to_string());
}
