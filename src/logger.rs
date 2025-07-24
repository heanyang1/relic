use std::{
    str::FromStr,
    sync::{LazyLock, Mutex},
};

use colored::Colorize;

#[derive(PartialEq, PartialOrd)]
enum LogLevel {
    Debug = 0,
    Error = 1,
}

impl FromStr for LogLevel {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "DEBUG" => Ok(LogLevel::Debug),
            "ERROR" => Ok(LogLevel::Error),
            _ => Err(format!("Unknown log level: {s}")),
        }
    }
}

/// A very simple logger.
///
/// I just need something that prints to the terminal and can be opened or
/// closed easily from outside, so I wrote this.
pub struct Logger {
    level: LogLevel,
}

impl Logger {
    fn new() -> Self {
        let level = std::env::var("LOG_LEVEL")
            .unwrap_or_else(|_| "ERROR".into())
            .parse()
            .unwrap();

        Logger { level }
    }

    fn write(&mut self, msg: &str, color: &str) {
        println!("{}", msg.color(color))
    }

    fn debug(&mut self, msg: &str) {
        if self.level <= LogLevel::Debug {
            let message = format!("[DEBUG] {msg}");
            self.write(&message, "blue");
        }
    }
    fn error(&mut self, msg: &str) {
        if self.level <= LogLevel::Error {
            let message = format!("[ERROR] {msg}");
            self.write(&message, "red");
        }
    }
}

pub static LOGGER: LazyLock<Mutex<Logger>> = LazyLock::new(|| Mutex::new(Logger::new()));

pub fn log_debug(msg: &str) {
    LOGGER.lock().unwrap().debug(msg);
}
pub fn log_error(msg: &str) {
    LOGGER.lock().unwrap().error(msg);
}

pub fn unwrap_result<T, E>(result: Result<T, E>, default: T) -> T
where
    E: ToString,
{
    match result {
        Ok(x) => x,
        Err(msg) => {
            log_error(&msg.to_string());
            default
        }
    }
}
