//! The logger module.

use std::{
    process::abort,
    str::FromStr,
    sync::{LazyLock, Mutex},
};

use colored::Colorize;

#[derive(PartialEq, PartialOrd)]
pub enum LogLevel {
    Debug = 0,
    Warning = 1,
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

pub struct Logger {
    level: LogLevel,
}

/// A very simple logger.
impl Logger {
    fn new() -> Self {
        let level = std::env::var("LOG_LEVEL")
            .unwrap_or_else(|_| "ERROR".into())
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

pub static LOGGER: LazyLock<Mutex<Logger>> = LazyLock::new(|| Mutex::new(Logger::new()));

pub fn log_debug<T>(msg: T)
where
    T: ToString,
{
    LOGGER.lock().unwrap().debug(msg.to_string());
}
pub fn log_warning<T>(msg: T)
where
    T: ToString,
{
    LOGGER.lock().unwrap().warning(msg.to_string());
}
pub fn log_error<T>(msg: T)
where
    T: ToString,
{
    LOGGER.lock().unwrap().error(msg.to_string());
}
pub fn set_log_level(level: LogLevel) {
    LOGGER.lock().unwrap().set_log_level(level);
}

pub fn unwrap_result<T, E>(result: Result<T, E>, default: T) -> T
where
    E: ToString,
{
    match result {
        Ok(x) => x,
        Err(msg) => {
            log_error(msg.to_string());
            default
        }
    }
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
