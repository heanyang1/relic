use rustyline::completion::{Completer, Pair};
use rustyline::highlight::Highlighter;
use rustyline::Context;
use rustyline::validate::{ValidationContext, ValidationResult, Validator};
use std::sync::Arc;

pub struct RelicCompleter {
    pub candidates: Arc<Vec<String>>,
}

// Implement Helper as a marker trait
impl rustyline::Helper for RelicCompleter {}

// Implement Hinter as a no-op
impl rustyline::hint::Hinter for RelicCompleter {
    type Hint = String;
    fn hint(&self, _line: &str, _pos: usize, _ctx: &Context<'_>) -> Option<String> {
        None
    }
}

// Implement Highlighter as a no-op
impl Highlighter for RelicCompleter {}

// Implement Validator as always valid
impl Validator for RelicCompleter {
    fn validate(
        &self,
        _ctx: &mut ValidationContext,
    ) -> Result<ValidationResult, rustyline::error::ReadlineError> {
        Ok(ValidationResult::Valid(None))
    }
}

impl Completer for RelicCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> Result<(usize, Vec<Pair>), rustyline::error::ReadlineError> {
        let start = line[..pos]
            .rfind(|c: char| c.is_whitespace() || c == '(')
            .map_or(0, |i| i + 1);
        let word = &line[start..pos];
        let matches = self
            .candidates
            .iter()
            .filter(|s| s.starts_with(word))
            .map(|s| Pair {
                display: s.clone(),
                replacement: s.clone(),
            })
            .collect();
        Ok((start, matches))
    }
}
