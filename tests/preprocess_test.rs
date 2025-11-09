use relic::lexer::LexerMonad;
use relic::preprocess::PreProcess;
use std::collections::HashMap;

macro_rules! assert_preprocess {
    ($code:expr, $expected:expr) => {{
        let mut macros = HashMap::new();
        let tokens = LexerMonad::new_unnamed($code);
        let mut node = tokens.parse().unwrap();
        node = node.preprocess(&mut macros).unwrap();
        assert_eq!(format!("{}", node), $expected);
    }};

    ($code:expr, $expected:expr, $macros:expr) => {{
        let mut tokens = Lexer::new($code);
        let mut node = Node::parse(&mut tokens).unwrap();
        node = node.preprocess(&mut $macros).unwrap();
        assert_eq!(format!("{}", node), $expected);
    }};
}

#[test]
fn test_unchange_expr() {
    let x = "(+ (* 1 2 3) (/ 3 4))";
    assert_preprocess!(x, x);
    let x = "(define x (+ 42 3.14))";
    assert_preprocess!(x, x);
}

#[test]
fn test_lambda() {
    assert_preprocess!(
        "(lambda (x y) (+ x 1) (* x y))",
        "(lambda (x y) (begin (+ x 1) (* x y)))"
    );
}

#[test]
fn test_defn() {
    assert_preprocess!(
        "(define (f x) (+ x 1))",
        "(define f (lambda (x) (begin (+ x 1))))"
    );
}

#[test]
fn test_cond() {
    assert_preprocess!(
        r#"
        (cond ((= x 1) (+ x 1) (* y 2))
              ((= 2 y) (- x 2)))"#,
        "(if (= x 1) (begin (+ x 1) (* y 2)) (if (= 2 y) (begin (- x 2)) ()))"
    );
}

#[test]
fn test_and() {
    assert_preprocess!(
        "(and x (+ x 1) '())",
        "(if (eq? x ()) x (if (eq? (+ x 1) ()) (+ x 1) (if (eq? (quote ()) ()) (quote ()) (quote ()))))"
    );
}

#[test]
fn test_or() {
    assert_preprocess!(
        "(or x (+ x 1) '())",
        "(if x x (if (+ x 1) (+ x 1) (if (quote ()) (quote ()) ())))"
    );
}

#[test]
fn test_let() {
    assert_preprocess!(
        r#"
        (let ((x 1) (y (+ x 2)) (z 3))
             (+ x y)
             (* (- y z) 5))"#,
        "((lambda (x y z) (begin (+ x y) (* (- y z) 5))) 1 (+ x 2) 3)"
    );
}

#[test]
fn test_defind_and_or() {
    assert_preprocess!(
        "(define x (and 1 2))",
        "(define x (if (eq? 1 ()) 1 (if (eq? 2 ()) 2 2)))"
    );
    assert_preprocess!(
        "(define (f x) (or x))",
        "(define f (lambda (x) (begin (if x x ()))))"
    );
}
