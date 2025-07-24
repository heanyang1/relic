//! Symbols and special forms.

use std::{fmt::Display, str::FromStr};

/// A special form is a symbol that does not fit in the applicative model.
/// See chapter 1.1.3 of SICP for details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecialForm {
    /// Special form `quote`.
    ///
    /// `(quote x)` return `x` as is. `'x` is equivalent to `(quote x)`.
    Quote,
    /// Special form `cond`.
    ///
    /// `(cond (p1 e11 e12 ...) (p2 e21 e22 ...) ...)` evaluates the predicate
    /// `p1`, if it is not `nil`, then evaluate `e11`, `e12`, ... and return the
    /// result of the last expression, otherwise evaluate the next predicate.
    /// The evaluation stops once a predicate is not `nil`.
    ///
    /// `nil` is returned if all predicates are `nil`.
    Cond,
    /// Special form `if`.
    ///
    /// `(if p e1 e2)` evaluates `p`, if it is not `nil`, then evaluate `e1`
    /// and return the result, otherwise evaluate `e2` and return the result.
    If,
    /// Special form `begin`.
    ///
    /// `(begin e1 e2 ...)` evaluates `e1`, `e2`, ... in order and return the
    /// value of the last expression.
    Begin,
    /// Special form `lambda`.
    ///
    /// `(lambda (pattern) e1 e2 ...)` returns a procedure object. When a
    /// procedure object is evaluated, it matches the pattern with the argument
    /// to get the variable bindings, creates a new environment with the
    /// bindings, evaluates `e1`, `e2`, ... in order and return the value of
    /// the last expression.
    Lambda,
    /// Special form `let`.
    ///
    /// `(let ((x1 e11) (x2 e12) ...) e21 e22 ...)` evaluates `e11`, `e12`, ... and bind
    /// the results to `x1`, `x2`, ... respectively, then evaluate `e21`, `e22`, ... and
    /// return the result of the last expression.
    Let,
    /// Special form `define`.
    ///
    /// `(define x e)` evaluates `e` and bind the result to `x` in current
    /// environment.
    ///
    /// `(define (func pattern) e1 e2 ...)` is the shorthand version of
    /// `(define func (lambda (pattern) e1 e2 ...))`.
    ///
    /// The form returns `nil`. The variable defined is inserted into current
    /// environment directly.
    Define,
    /// Special form `define-syntax-rule`.
    ///
    /// `(define-syntax-rule (macro pattern) template)` defines a macro
    /// named `macro`.
    ///
    /// This is a simplified version of
    /// [Racket's `define-syntax-rule`](https://docs.racket-lang.org/guide/pattern-macros.html),
    /// with the exception that our macro has dynamic scope instead of lexical
    /// scope.
    ///
    /// The macro defined is inserted into current environment directly.
    DefineSyntaxRule,
    /// Special form `set!`.
    ///
    /// `(set! x e)` evaluates `e`, find the variable `x` in the (current and
    /// parent) environmets and change its value. Returns `nil`. Raise an error
    /// if `x` is not in any of the environments.
    Set,
    /// Special form `set-car!`.
    ///
    /// `(set-car! x e)` sets the car of `x` to be the value of `e`. `x` must
    /// be a user-defined symbol that points to a pair. Returns `nil`.
    SetCar,
    /// Special form `set-cdr!`.
    ///
    /// `(set-cdr! x e)` sets the cdr of `x` to be the value of `e`. `x` must
    /// be a user-defined symbol that points to a pair. Returns `nil`.
    SetCdr,
    /// Special form `and`.
    ///
    /// `(and x1 x2 ...)` evaluates `x1`, `x2`, ... in order and return the
    /// first value that is `nil`. The evaluation stops once a value is `nil`.
    /// If all values are not `nil`, return the last value. If there are no
    /// values, return `t`.
    ///
    /// This symbol's behaviour is the same as R5RS.
    And,
    /// Special form `or`.
    ///
    /// `(or x1 x2 ...)` evaluates `x1`, `x2`, ... in order and return the
    /// first value that is not `nil`. The evaluation stops once a value is not
    /// `nil`. If all values are `nil`, return `nil`. If there are no values,
    /// return `nil`.
    ///
    /// This symbol's behaviour is the same as R5RS.
    Or,
    /// Special form `display`.
    ///
    /// `(display x)` evaluates `x` and print the result.
    Display,
    /// Special form `newline`.
    ///
    /// `(newline)` prints a newline.
    NewLine,
    /// Special form `graphviz`.
    ///
    /// `(graphviz)` prints current environment as a graphviz graph.
    Graphviz,
    /// Special form `breakpoint`.
    ///
    /// `(breakpoint)` creates a breakpoint that stops the debugger. It is a
    /// noop in non-debug environments.
    BreakPoint,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Symbol {
    /// Built-in symbol `nil`.
    ///
    /// `nil` is equivalent to the empty list `'()`.
    Nil,
    /// Built-in symbol `atom?`.
    ///
    /// `(atom? x)` evaluates `x` and return `t` if the value of `x` is not a
    /// list, otherwise return `f`.
    Atom,
    /// Built-in symbol `number?`.
    ///
    /// `(number? x)` evaluates `x` and return `t` if the value of `x` is a
    /// number, otherwise return `f`.
    Number,
    /// Built-in symbol `eq?`.
    ///
    /// `(eq? x y)` evaluates `x` and `y` and return `t` if their values are
    /// the same, otherwise return `f`.
    Eq,
    /// Built-in symbol `car`.
    ///
    /// `(car x)` evaluates `x` and return the first element of the list.
    Car,
    /// Built-in symbol `cdr`.
    ///
    /// `(cdr x)` evaluates `x` and return the list without the first element.
    Cdr,
    /// Built-in symbol `cons`.
    ///
    /// `(cons x y)` evaluates `x` and `y` and return a new list with `x` as
    /// the first element and `y` as the rest of the list.
    Cons,
    /// Built-in symbol `t`.
    ///
    /// `t` is the true value.
    T,
    /// Built-in symbol `list`.
    ///
    /// `(list x1 x2 ...)` evaluates `x1`, `x2`, ... and return a list of the
    ///  results.
    List,
    /// Built-in symbol `+`.
    ///
    /// `(+ x1 x2)` evaluates `x1` and `x2` and return the sum of the results.
    Add,
    /// Built-in symbol `-`.
    ///
    /// `(- x1 x2)` evaluates `x1` and `x2` and return the difference of the
    /// results.
    Sub,
    /// Built-in symbol `*`.
    ///
    /// `(* x1 x2)` evaluates `x1` and `x2` and return the product of the
    /// results.
    Mul,
    /// Built-in symbol `/`.
    ///
    /// `(/ x1 x2)` evaluates `x1` and `x2` and return the quotient of the
    /// results.
    Div,
    /// Built-in symbol `>`.
    ///
    /// `(> x1 x2)` evaluates `x1` and `x2` and return `t` if `x1` is greater
    /// than `x2`, otherwise return `f`.
    Gt,
    /// Built-in symbol `<`.
    ///
    /// `(< x1 x2)` evaluates `x1` and `x2` and return `t` if `x1` is less than
    /// `x2`, otherwise return `f`.
    Lt,
    /// Built-in symbol `>=`.
    ///
    /// `(>= x1 x2)` evaluates `x1` and `x2` and return `t` if `x1` is greater
    /// than or equal to `x2`, otherwise return `f`.
    Ge,
    /// Built-in symbol `<=`.
    ///
    /// `(<= x1 x2)` evaluates `x1` and `x2` and return `t` if `x1` is less
    /// than or equal to `x2`, otherwise return `f`.
    Le,
    /// Built-in symbol `=`.
    ///
    /// `(= x1 x2)` evaluates `x1` and `x2` and return `t` if `x1` is equal to
    /// `x2`, otherwise return `f`.
    EqNum,
    /// User-defined symbol.
    User(String),
}

impl FromStr for SpecialForm {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "quote" => Ok(SpecialForm::Quote),
            "cond" => Ok(SpecialForm::Cond),
            "if" => Ok(SpecialForm::If),
            "begin" => Ok(SpecialForm::Begin),
            "lambda" => Ok(SpecialForm::Lambda),
            "let" => Ok(SpecialForm::Let),
            "define" => Ok(SpecialForm::Define),
            "define-syntax-rule" => Ok(SpecialForm::DefineSyntaxRule),
            "set!" => Ok(SpecialForm::Set),
            "set-car!" => Ok(SpecialForm::SetCar),
            "set-cdr!" => Ok(SpecialForm::SetCdr),
            "and" => Ok(SpecialForm::And),
            "or" => Ok(SpecialForm::Or),
            "display" => Ok(SpecialForm::Display),
            "newline" => Ok(SpecialForm::NewLine),
            "graphviz" => Ok(SpecialForm::Graphviz),
            "breakpoint" => Ok(SpecialForm::BreakPoint),
            _ => Err("Not a special form".to_string()),
        }
    }
}

impl<T: Into<String>> From<T> for Symbol {
    fn from(value: T) -> Self {
        let value = value.into();
        match value.as_str() {
            "atom?" => Symbol::Atom,
            "number?" => Symbol::Number,
            "eq?" => Symbol::Eq,
            "car" => Symbol::Car,
            "cdr" => Symbol::Cdr,
            "cons" => Symbol::Cons,
            "t" => Symbol::T,
            "list" => Symbol::List,
            "nil" => Symbol::Nil,
            "+" => Symbol::Add,
            "-" => Symbol::Sub,
            "*" => Symbol::Mul,
            "/" => Symbol::Div,
            ">" => Symbol::Gt,
            "<" => Symbol::Lt,
            ">=" => Symbol::Ge,
            "<=" => Symbol::Le,
            "=" => Symbol::EqNum,
            _ => Symbol::User(value),
        }
    }
}

impl Display for SpecialForm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SpecialForm::Quote => write!(f, "quote"),
            SpecialForm::Cond => write!(f, "cond"),
            SpecialForm::If => write!(f, "if"),
            SpecialForm::Begin => write!(f, "begin"),
            SpecialForm::Lambda => write!(f, "lambda"),
            SpecialForm::Let => write!(f, "let"),
            SpecialForm::Define => write!(f, "define"),
            SpecialForm::DefineSyntaxRule => write!(f, "define-syntax-rule"),
            SpecialForm::Set => write!(f, "set!"),
            SpecialForm::SetCar => write!(f, "set-car!"),
            SpecialForm::SetCdr => write!(f, "set-cdr!"),
            SpecialForm::And => write!(f, "and"),
            SpecialForm::Or => write!(f, "or"),
            SpecialForm::Display => write!(f, "display"),
            SpecialForm::NewLine => write!(f, "newline"),
            SpecialForm::Graphviz => write!(f, "graphviz"),
            SpecialForm::BreakPoint => write!(f, "breakpoint"),
        }
    }
}

impl Display for Symbol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Symbol::User(str) => write!(f, "{str}"),
            Symbol::Nil => write!(f, "nil"),
            Symbol::Atom => write!(f, "atom?"),
            Symbol::Number => write!(f, "number?"),
            Symbol::Eq => write!(f, "eq?"),
            Symbol::Car => write!(f, "car"),
            Symbol::Cdr => write!(f, "cdr"),
            Symbol::Cons => write!(f, "cons"),
            Symbol::T => write!(f, "t"),
            Symbol::List => write!(f, "list"),
            Symbol::Add => write!(f, "+"),
            Symbol::Sub => write!(f, "-"),
            Symbol::Mul => write!(f, "*"),
            Symbol::Div => write!(f, "/"),
            Symbol::Gt => write!(f, ">"),
            Symbol::Lt => write!(f, "<"),
            Symbol::Ge => write!(f, ">="),
            Symbol::Le => write!(f, "<="),
            Symbol::EqNum => write!(f, "="),
        }
    }
}

/// The shorthand for `Node::Symbol(Symbol::Nil)`.
#[macro_export]
macro_rules! nil {
    () => {
        Node::Symbol(Symbol::Nil)
    };
}
