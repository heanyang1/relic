//! The lexer module.

//! File pointer module.

use std::{fmt::Display, fs::read_to_string, path::PathBuf, rc::Rc};

use crate::{
    error::{MapErr, ParseError},
    number::Number,
    util::interval_union,
};

#[derive(Eq, PartialEq, Debug, Clone)]
pub enum Token {
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

fn is_whitespace(x: char) -> bool {
    x == ' ' || x == '\n' || x == '\t'
}

fn is_special_char(x: char) -> bool {
    match x {
        '(' | ')' | '\'' => true,
        _ => is_whitespace(x),
    }
}

/// The lexer is a monad that stores the location and the source string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexerMonad<T> {
    data: T,
    /// Marks the beginning location of the data in the file.
    begin: FilePointer,
    /// Marks the end (+1) location of the data in the file.
    end: FilePointer,
    source: Rc<String>,
}

impl LexerMonad<()> {
    /// Construct a lexer monad from a source file.
    pub fn new(filepath: PathBuf) -> Result<LexerMonad<()>, String> {
        Ok(LexerMonad {
            data: (),
            begin: FilePointer::new(filepath.to_str().unwrap()),
            end: FilePointer::new(filepath.to_str().unwrap()),
            source: read_to_string(&filepath).map_err(|e| e.to_string())?.into(),
        })
    }
    pub fn new_unnamed<T>(source: T) -> LexerMonad<()>
    where
        T: ToString,
    {
        LexerMonad {
            data: (),
            begin: FilePointer::new("<unnamed>"),
            end: FilePointer::new("<unnamed>"),
            source: source.to_string().into(),
        }
    }
}

impl<T> LexerMonad<LexerMonad<T>>
where
    T: Clone,
{
    /// The `join` (or `mu`) operator. The new location range is the union of
    /// the two old ranges.
    pub fn join(self) -> LexerMonad<T> {
        let (begin, end) = interval_union((self.begin, self.end), (self.data.begin, self.data.end));
        LexerMonad {
            data: self.data.data,
            begin,
            end,
            source: self.source,
        }
    }
}

impl LexerMonad<()> {
    /// Get a token from the lexer. Returns a monad whose data is the token
    /// and pointers are set to its begin and end respectively.
    ///
    /// The begin and end pointer must be the same when called.
    pub fn next_token(self) -> Result<LexerMonad<Token>, ParseError> {
        let token_begin = self.end.consume_whitespace(&self.source);
        let (end, data) = token_begin.clone().next_token(&self.source)?;
        Ok(LexerMonad {
            data,
            begin: token_begin,
            end,
            source: self.source,
        })
    }
}

impl<T> LexerMonad<T> {
    #[cfg(test)]
    pub(crate) fn test_new(
        data: T,
        source: String,
        l_begin: usize,
        c_begin: usize,
        s_begin: usize,
        l_end: usize,
        c_end: usize,
        s_end: usize,
    ) -> Self {
        Self {
            data,
            begin: FilePointer::test_new("<unnamed>".to_string(), l_begin, c_begin, s_begin),
            end: FilePointer::test_new("<unnamed>".to_string(), l_end, c_end, s_end),
            source: source.into(),
        }
    }
    /// Monadic bind.
    pub fn bind<U, F>(&self, func: F) -> LexerMonad<U>
    where
        F: Fn(&T) -> U,
    {
        LexerMonad {
            data: func(&self.data),
            begin: self.begin.clone(),
            end: self.end.clone(),
            source: self.source.clone(),
        }
    }
    /// Get an empty monad that has a same file pointer.
    pub fn get_fp(&self) -> LexerMonad<()> {
        LexerMonad {
            data: (),
            begin: self.begin.clone(),
            end: self.end.clone(),
            source: self.source.clone(),
        }
    }
    /// Get an empty monad whose file pointers points to the beginning of `self`.
    pub fn get_begin(&self) -> LexerMonad<()> {
        LexerMonad {
            data: (),
            begin: self.begin.clone(),
            end: self.begin.clone(),
            source: self.source.clone(),
        }
    }
    /// Get an empty monad whose file pointers points to the end of `self`.
    pub fn get_end(&self) -> LexerMonad<()> {
        LexerMonad {
            data: (),
            begin: self.end.clone(),
            end: self.end.clone(),
            source: self.source.clone(),
        }
    }
    /// A backdoor left for creating a node from nowhere.
    pub fn from_other<U>(data: T, other: LexerMonad<U>) -> LexerMonad<T> {
        LexerMonad {
            data,
            begin: other.begin,
            end: other.end,
            source: other.source,
        }
    }
    pub fn get(&self) -> &T {
        &self.data
    }
    pub fn error<Msg>(&self, msg: Msg) -> String
    where
        Msg: Display,
    {
        format!("At {}: {}", self.begin, msg)
    }

    /// Consumes a token and change the lexer state if it is equal to `token`.
    /// Does not change the data.
    pub fn consume(self, token: Token) -> Result<Self, ParseError> {
        let s = self.get_end().next_token()?;
        if s.data == token {
            Ok(LexerMonad {
                data: self.data,
                begin: s.begin,
                end: s.end,
                source: self.source,
            })
        } else {
            Err(self.error(format!("Expected {token:?}, found {:?}", s.data)))
        }
        .map_err_simple()
    }

    pub fn consume_symbol(self) -> Result<Self, ParseError> {
        let s = self.get_end().next_token()?;
        match s.data {
            Token::Symbol(_) => Ok(LexerMonad {
                data: self.data,
                begin: s.begin,
                end: s.end,
                source: self.source,
            }),
            _ => Err(self.error(format!("Expected symbol, found {:?}", s.data))),
        }
        .map_err_simple()
    }
}

/// A file pointer records the file name, line number and column number to be
/// used in debug information.
///
/// It provides function to increase line and column, but it is file-agnostic
/// and does not check anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilePointer {
    filename: String,
    line_number: usize,
    column_number: usize,
    /// The index of the file string that is used by the lexer.
    str_idx: usize,
    /// Whether the fp previously points to a `\n`. If this flag is true,
    /// column number will be set to 0 when consuming a character.
    after_newline: bool,
}

impl FilePointer {
    #[cfg(test)]
    pub fn test_new(
        filename: String,
        line_number: usize,
        column_number: usize,
        str_idx: usize,
    ) -> Self {
        FilePointer {
            filename,
            line_number,
            column_number,
            str_idx,
            after_newline: false,
        }
    }
}

impl Ord for FilePointer {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        assert_eq!(self.filename, other.filename);
        self.str_idx.cmp(&other.str_idx)
    }
}

impl PartialOrd for FilePointer {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        assert_eq!(self.filename, other.filename);
        self.str_idx.partial_cmp(&other.str_idx)
    }
}

impl FilePointer {
    pub fn new<T>(filename: T) -> FilePointer
    where
        T: ToString,
    {
        FilePointer {
            filename: filename.to_string(),
            line_number: 1,
            column_number: 0,
            str_idx: 0,
            after_newline: false,
        }
    }
    /// Consume a character.
    fn consume(self, source: &str) -> FilePointer {
        if self.get_char(source).unwrap() == '\n' {
            self.new_line()
        } else {
            self.new_character()
        }
    }
    /// Consume characters until `stop(current_char)` is true.
    fn consume_until<F>(mut self, source: &str, stop: F) -> (Self, String)
    where
        F: Fn(char) -> bool,
    {
        let begin = self.str_idx;
        while let Some(x) = self.get_char(source) {
            if stop(x) {
                break;
            }
            self = self.consume(source);
        }
        let cur_pos = self.str_idx;
        (self, source[begin..cur_pos].to_string())
    }

    pub fn consume_whitespace(self, source: &str) -> Self {
        self.consume_until(source, |x| !is_whitespace(x)).0
    }

    pub fn next_token(mut self, source: &str) -> Result<(Self, Token), ParseError> {
        match self.get_char(source) {
            Some(ch) => match ch {
                '(' => Ok((self.consume(source), Token::LParem)),
                ')' => Ok((self.consume(source), Token::RParem)),
                '\'' => Ok((self.consume(source), Token::Quote)),
                '.' => Ok((self.consume(source), Token::Dot)),
                '\"' => {
                    self = self.consume(source);
                    let (s, t) = self.consume_until(source, |x| x == '\"');
                    Ok((s.consume(source), Token::String(t)))
                }
                ';' => {
                    self = self.consume(source);
                    self.consume_until(source, |x| x == '\n')
                        .0
                        .consume_whitespace(source)
                        .next_token(source)
                }
                x if is_whitespace(x) => unreachable!(),
                x if x == '-' || x.is_ascii_digit() => self.consume_number(source),
                _ => {
                    let (s, t) = self.consume_until(source, is_special_char);
                    Ok((s, Token::Symbol(t)))
                }
            },
            None => Err(ParseError::EOF), // EOF
        }
    }

    pub fn consume_number(mut self, source: &str) -> Result<(Self, Token), ParseError> {
        enum State {
            Start,
            NegSym,
            Integer,
            Dot,
            Float,
        }
        let mut state = State::Start;
        let begin = self.str_idx;
        while let Some(x) = self.get_char(source) {
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
            self = self.consume(source);
        }
        let num_str = source[begin..self.str_idx].to_string();
        match state {
            // A `-` that does not followed by a digit is parsed as `-` symbol.
            State::NegSym => Ok((self, Token::Symbol("-".to_string()))),
            State::Integer => Ok((
                self,
                Token::Number(Number::Int(num_str.parse::<i64>().unwrap())),
            )),
            State::Float => Ok((
                self,
                Token::Number(Number::Float(num_str.parse::<f64>().unwrap())),
            )),
            _ => Err(ParseError::SyntaxError(format!(
                "At position {self}: Expected number, found {num_str}"
            ))),
        }
    }

    pub fn get_str_idx(&self) -> usize {
        self.str_idx
    }

    fn new_line(self) -> FilePointer {
        FilePointer {
            filename: self.filename,
            line_number: self.line_number + 1,
            column_number: 0,
            str_idx: self.str_idx + 1,
            after_newline: true,
        }
    }

    fn new_character(self) -> FilePointer {
        FilePointer {
            filename: self.filename,
            line_number: self.line_number,
            column_number: if self.after_newline {
                0
            } else {
                self.column_number + 1
            },
            str_idx: self.str_idx + 1,
            after_newline: false,
        }
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
