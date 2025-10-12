//! File pointer module.

use std::fmt::Display;

/// A file pointer records the file name, line number and column number to be
/// used in debug information.
///
/// It provides function to increase line and column, but it is file-agnostic
/// and does not check anything.
#[derive(Clone)]
pub struct FilePointer {
    filename: String,
    line_number: usize,
    column_number: usize,
    /// The index of the file string that is used by the lexer.
    str_idx: usize,
}

impl FilePointer {
    pub fn new<T>(filename: T) -> FilePointer
    where
        T: ToString,
    {
        FilePointer {
            filename: filename.to_string(),
            line_number: 0,
            column_number: 0,
            str_idx: 0,
        }
    }
    pub fn consume_until(&mut self, end: usize, content: &str) {
        for ch in content.chars().skip(self.str_idx).take(end - self.str_idx) {
            self.consume(ch);
        }
    }
    fn consume(&mut self, c: char) {
        if c == '\n' {
            self.new_line()
        } else {
            self.new_character()
        }
    }

    pub fn get_str_idx(&self) -> usize {
        self.str_idx
    }

    pub fn new_line(&mut self) {
        self.line_number += 1;
        self.str_idx += 1;
    }

    pub fn new_character(&mut self) {
        self.column_number += 1;
        self.str_idx += 1;
    }

    pub fn get_char(&self, file_str: &str) -> Option<char> {
        file_str.chars().nth(self.str_idx)
    }
}

impl Display for FilePointer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}:{}:{}",
            self.filename, self.line_number, self.column_number
        )
    }
}
