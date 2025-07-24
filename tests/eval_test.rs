use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use relic::env::Env;
use relic::eval::{ConsoleEval, Eval};
use relic::lexer::{Lexer, Number};
use relic::node::{Node, NodeEnv};
use relic::parser::Parse;
use relic::preprocess::PreProcess;
use relic::symbol::Symbol;
use relic::{nil, vec_to_list};

macro_rules! assert_eval {
    ($input:expr, $expected:expr) => {{
        let input = $input;
        let mut lexer = Lexer::new(input);
        let mut result = Node::parse(&mut lexer).unwrap();
        let node = result.preprocess(&mut HashMap::new()).unwrap();
        let eval = node.eval(
            RefCell::new(NodeEnv::top(&mut ())).into(),
            ConsoleEval::new(),
        );
        assert_eq!(eval.unwrap().node, $expected.into());
    }};

    ($input:expr, $env:expr, $expected:expr, $macros: expr) => {{
        let input = $input;
        let mut lexer = Lexer::new(input);
        let mut result = Node::parse(&mut lexer).unwrap();
        let node = result.preprocess(&mut $macros).unwrap();
        let eval = node.eval($env, ConsoleEval::new());
        assert_eq!(eval.unwrap().node, $expected.into());
    }};
}

macro_rules! assert_eval_eq {
    ($input:expr, $env:expr, $expected:expr, $macros:expr) => {{
        let input = $input;
        let mut lexer = Lexer::new(input);
        let mut result = Node::parse(&mut lexer).unwrap();
        let node = result.preprocess(&mut $macros).unwrap();
        let eval = node.eval($env.clone(), ConsoleEval::new());
        let expected_eval = node.eval($env, ConsoleEval::new());
        assert_eq!(eval.unwrap().node, expected_eval.unwrap().node);
    }};
}

macro_rules! assert_eval_print {
    ($input:expr, $env:expr) => {{
        let input = $input;
        let mut lexer = Lexer::new(input);
        let mut result = Node::parse(&mut lexer).unwrap();
        let node = result.preprocess(&mut HashMap::new()).unwrap();
        let eval = node.eval($env, ConsoleEval::new()).unwrap();
        println!("{:?} {:?}", &eval.display_output, &eval.graphviz_output);
    }};
    ($input:expr, $env:expr, $result:expr, $macros:expr) => {{
        let input = $input;
        let mut lexer = Lexer::new(input);
        let mut result = Node::parse(&mut lexer).unwrap();
        let node = result.preprocess(&mut $macros).unwrap();
        node.eval($env, $result.clone()).unwrap()
    }};
}

macro_rules! assert_error {
    ($input:expr, $expected:expr) => {
        let input = $input;
        let mut lexer = Lexer::new(input);
        let mut node = Node::parse(&mut lexer).unwrap();
        let node = node.preprocess(&mut HashMap::new()).unwrap();
        let eval = node.eval(
            RefCell::new(NodeEnv::top(&mut ())).into(),
            ConsoleEval::new(),
        );
        assert_eq!(eval, Err($expected.to_string()));
    };

    ($input:expr, $env:expr, $expected:expr) => {
        let input = $input;
        let mut lexer = Lexer::new(input);
        let mut result = Node::parse(&mut lexer).unwrap();
        let node = result.preprocess(&mut HashMap::new()).unwrap();
        let eval = node.eval($env, ConsoleEval::new());
        assert_eq!(eval, Err($expected.to_string()));
    };
}

#[test]
fn test_int() {
    assert_eval!("42", Node::Number(Number::Int(42)));
}

#[test]
fn test_simple_arithmetic() {
    assert_eval!("(+ 1 2 3 4)", Node::Number(Number::Int(10)));
    assert_eval!("(- 3 2 1)", Node::Number(Number::Int(0)));
    assert_eval!("(* 2 3)", Node::Number(Number::Int(6)));
    assert_eval!("(/ 6 3)", Node::Number(Number::Int(2)));
    assert_eval!("(/ 5 2)", Node::Number(Number::Float(2.5)));
    assert_eval!("(+ 1.0 2.0 3)", Node::Number(Number::Float(6.0)));
    assert_eval!("(- 3.0 2.0)", Node::Number(Number::Float(1.0)));
    assert_eval!("(* 2.0 3.0)", Node::Number(Number::Float(6.0)));
    assert_eval!("(/ 6.0 3.0)", Node::Number(Number::Float(2.0)));
    assert_eval!("(+ 1 2.0)", Node::Number(Number::Float(3.0)));
    assert_eval!("(- 3.0 2)", Node::Number(Number::Float(1.0)));
}

#[test]
fn test_with_symbol() {
    let env = Rc::new(RefCell::new(NodeEnv::new(
        None,
        HashMap::from([("x".to_string(), Node::Number(Number::Int(2)).into())]),
        "env",
    )));
    assert_eval!(
        "(- x 2)",
        env.clone(),
        Node::Number(Number::Int(0)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(* 3 x)",
        env.clone(),
        Node::Number(Number::Int(6)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(/ 6 x)",
        env.clone(),
        Node::Number(Number::Int(3)),
        &mut HashMap::new()
    );
}

#[test]
fn test_nested_arithmetic() {
    assert_eval!("(+ (- 1 2) 3)", Node::Number(Number::Int(2)));
    assert_eval!("(* (/ 1 2) (+ 3 4))", Node::Number(Number::Float(3.5)));
}

#[test]
fn test_relational_operators() {
    assert_eval!("t", Node::Symbol(Symbol::T));
    assert_eval!("(< 1.0 2)", Node::Symbol(Symbol::T));
    assert_eval!("(< 2 1)", Node::Symbol(Symbol::Nil));
    assert_eval!("(> 1 2.0)", Node::Symbol(Symbol::Nil));
    assert_eval!("(> (+ 1 1) 1)", Node::Symbol(Symbol::T));
    assert_eval!("(<= 1 1.0)", Node::Symbol(Symbol::T));
    assert_eval!("(<= 1.0 2)", Node::Symbol(Symbol::T));
    assert_eval!("(<= 2 1)", Node::Symbol(Symbol::Nil));
    assert_eval!("(>= 1 1)", Node::Symbol(Symbol::T));
    assert_eval!("(>= 1 2)", Node::Symbol(Symbol::Nil));
    assert_eval!("(>= 2 1)", Node::Symbol(Symbol::T));
}

#[test]
fn test_toplevel_symbol() {
    assert_eval!("+", Node::Symbol(Symbol::Add));
    assert_eval!("-", Node::Symbol(Symbol::Sub));
    assert_eval!("*", Node::Symbol(Symbol::Mul));
    assert_eval!("/", Node::Symbol(Symbol::Div));
}

#[test]
fn error_cases() {
    assert_error!("(+ 1)", "Fewer parameters than requested");
    assert_error!("(> 1)", "Fewer parameters than requested");
    assert_error!("(= 1 2 3)", "More parameters than requested");
    assert_error!("(1 2 3)", "1 can not be the head of a list");
    assert_error!("(+ + 1)", "+ is not a number");

    let env = Rc::new(RefCell::new(NodeEnv::new(
        None,
        HashMap::from([("x".to_string(), Node::Number(Number::Int(2)).into())]),
        "env",
    )));
    assert_error!("(+ x y)", env, "Symbol y not found");
}

#[test]
fn test_list_simple() {
    assert_eval!(
        "'(1 2 3)",
        vec_to_list!(
            Node::Number(Number::Int(1)).into(),
            Node::Number(Number::Int(2)).into(),
            Node::Number(Number::Int(3)).into()
        )
    );
    assert_eval!(
        "(list 1 2 3)",
        vec_to_list!(
            Node::Number(Number::Int(1)).into(),
            Node::Number(Number::Int(2)).into(),
            Node::Number(Number::Int(3)).into()
        )
    );
    assert_eval!(
        "(list 1 2)",
        vec_to_list!(
            Node::Number(Number::Int(1)).into(),
            Node::Number(Number::Int(2)).into()
        )
    );
    assert_eval!(
        "(list 1)",
        Node::Pair(Node::Number(Number::Int(1)).into(), nil!().into())
    );
    assert_eval!("(list)", nil!());
}

#[test]
fn test_list_nested() {
    assert_eval!(
        "(list (list 1 2 3) (list 4 5 6))",
        vec_to_list!(
            vec_to_list!(
                Node::Number(Number::Int(1)).into(),
                Node::Number(Number::Int(2)).into(),
                Node::Number(Number::Int(3)).into()
            )
            .into(),
            vec_to_list!(
                Node::Number(Number::Int(4)).into(),
                Node::Number(Number::Int(5)).into(),
                Node::Number(Number::Int(6)).into()
            )
            .into()
        )
    );
    assert_eval!(
        "(list 1 '(2 3) 4)",
        vec_to_list!(
            Node::Number(Number::Int(1)).into(),
            vec_to_list!(
                Node::Number(Number::Int(2)).into(),
                Node::Number(Number::Int(3)).into()
            )
            .into(),
            Node::Number(Number::Int(4)).into()
        )
    );
}

#[test]
fn test_list_manipulation() {
    assert_eval!("(car (list 1 2 3))", Node::Number(Number::Int(1)));
    assert_eval!(
        "(cdr '(1 2 3))",
        vec_to_list!(
            Node::Number(Number::Int(2)).into(),
            Node::Number(Number::Int(3)).into()
        )
    );
    assert_eval!(
        "(cons 1 (list 2 3))",
        vec_to_list!(
            Node::Number(Number::Int(1)).into(),
            Node::Number(Number::Int(2)).into(),
            Node::Number(Number::Int(3)).into()
        )
    );
}

#[test]
fn test_define() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!("(define x 1)", env.clone(), nil!(), &mut HashMap::new());
    assert_eval!(
        "(define y (+ x 1))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "x",
        env.clone(),
        Node::Number(Number::Int(1)),
        &mut HashMap::new()
    );
    assert_eval!(
        "y",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
}

#[test]
fn test_define_syntax_rule() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    let mut macros = HashMap::new();
    assert_eval!(
        "(define-syntax-rule (macro1 x) (display 1) (+ x 1))",
        env.clone(),
        nil!(),
        &mut macros
    );
    assert_eval!(
        "(define-syntax-rule (macro2 x) (car x))",
        env.clone(),
        nil!(),
        &mut macros
    );
    assert_eval!(
        "(macro1 2)",
        env.clone(),
        Node::Number(Number::Int(3)),
        &mut macros
    );
    assert_eval!(
        "(macro2 '(1 2 3))",
        env.clone(),
        Node::Number(Number::Int(1)),
        &mut macros
    );
}

#[test]
fn test_lambda() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        "((lambda (x) (+ x 1)) 2)",
        env.clone(),
        Node::Number(Number::Int(3)),
        &mut HashMap::new()
    );
    assert_eval!(
        "((lambda (x y) (+ x y)) 2 3)",
        env.clone(),
        Node::Number(Number::Int(5)),
        &mut HashMap::new()
    );
}

#[test]
fn test_lambda_with_define() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!("(define x 1)", env.clone(), nil!(), &mut HashMap::new());
    assert_eval!(
        "x",
        env.clone(),
        Node::Number(Number::Int(1)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(define func (lambda (x) (+ x 1)))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(func 2)",
        env.clone(),
        Node::Number(Number::Int(3)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(func x)",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
}

#[test]
fn test_lambda_scope() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!("(define (f) 1)", env.clone(), nil!(), &mut HashMap::new());
    assert_eval!(
        "(define (g) (define (f x) x) (f 2))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(g)",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
}

#[test]
fn test_lambda_pattern_matching() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        "(define f (lambda x (car x)))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(f 'a 'b 3 4)",
        env.clone(),
        Node::Symbol(Symbol::User("a".to_string())),
        &mut HashMap::new()
    );
    assert_eval!(
        "(define (g . x) (car x))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(g 2 3 4)",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(define h (lambda (x . y) (car y)))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(h 1 2 3 4)",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(h 1 2)",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(h 1 't)",
        env.clone(),
        Node::Symbol(Symbol::T),
        &mut HashMap::new()
    );
}

#[test]
fn test_function_call() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        "(define g (lambda (x) (+ x 1)))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(define h (lambda (x) (g (+ x 1))))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(h 2)",
        env.clone(),
        Node::Number(Number::Int(4)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(define a (lambda (x) (car x)))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(a '(1 2 3))",
        env.clone(),
        Node::Number(Number::Int(1)),
        &mut HashMap::new()
    );
}

#[test]
fn test_atom() {
    assert_eval!("(atom? 1)", Node::Symbol(Symbol::T));
    assert_eval!("(atom? (+ 1 2))", Node::Symbol(Symbol::T));
    assert_eval!("(atom? (list 1 2 3))", Node::Symbol(Symbol::Nil));
    assert_eval!("(atom? 'a)", Node::Symbol(Symbol::T));
    assert_eval!("(atom? '())", Node::Symbol(Symbol::T));
}

#[test]
fn test_eq() {
    assert_eval!("(eq? 1 1)", Node::Symbol(Symbol::T));
    assert_eval!("(eq? (- 2 1) 1)", Node::Symbol(Symbol::T));
    assert_eval!("(eq? 1 2)", Node::Symbol(Symbol::Nil));
    assert_eval!("(eq? 'a 'a)", Node::Symbol(Symbol::T));
    assert_eval!("(eq? 'a 'b)", Node::Symbol(Symbol::Nil));
    assert_eval!("(eq? '() '())", Node::Symbol(Symbol::T));
    assert_eval!("(eq? '(1 2 3) (list 1 2 3))", Node::Symbol(Symbol::T));
}

#[test]
fn test_number() {
    assert_eval!("(number? 1)", Node::Symbol(Symbol::T));
    assert_eval!("(number? (+ 1 2))", Node::Symbol(Symbol::T));
    assert_eval!("(number? 'a)", Node::Symbol(Symbol::Nil));
    assert_eval!("(number? '())", Node::Symbol(Symbol::Nil));
    assert_eval!("(number? 'a)", Node::Symbol(Symbol::Nil));
}

#[test]
fn test_cond() {
    assert_eval!(
        "(cond ((< 1 2) 1) ((> 1 2) 2))",
        Node::Number(Number::Int(1))
    );
    assert_eval!(
        "(cond ((> 1 2) 1) ((< 1 2) 2))",
        Node::Number(Number::Int(2))
    );
    assert_eval!("(cond ((> 1 2) 1))", nil!());
    assert_eval!("(cond ((> 1 2) 1) ((> 1 2) 2))", nil!());
}

#[test]
fn test_and() {
    assert_eval!("(and)", Node::Symbol(Symbol::T));
    assert_eval!("(and '() 2 3)", nil!());
    assert_eval!("(and 1 2 3)", Node::Number(Number::Int(3)));
}

#[test]
fn test_or() {
    assert_eval!("(or)", nil!());
    assert_eval!("(or '() 2 3)", Node::Number(Number::Int(2)));
    assert_eval!("(or 1 2 3)", Node::Number(Number::Int(1)));
}

#[test]
fn test_fact() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        r#"
(define fact
  (lambda (n acc)
    (cond ((< n 2) acc)
          ('t (fact (- n 1) (* n acc))))))"#,
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(fact 5 1)",
        env.clone(),
        Rc::new(RefCell::new(Node::Number(Number::Int(120)))),
        &mut HashMap::new()
    );
}

#[test]
fn test_fib() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        r#"(define fib
           (lambda (n)
             (cond ((< n 2) 1)
                   ('t (+ (fib (- n 1)) (fib (- n 2)))))))"#,
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        r#"(define map
            (lambda (func l)
            (cond ((eq? l '()) '())
                  ('t (cons (func (car l)) (map func (cdr l)))))))"#,
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(fib 9)",
        env.clone(),
        Node::Number(Number::Int(55)),
        &mut HashMap::new()
    );
    assert_eval_eq!(
        "(map (lambda (x) (+ x 1)) '(0 1 2 3 4 5 6 7 8 9))",
        env.clone(),
        "(1 2 3 4 5 6 7 8 9 10)",
        &mut HashMap::new()
    );
    assert_eval_eq!(
        "(map fib '(0 1 2 3 4 5 6 7 8 9))",
        env.clone(),
        "(1 1 2 3 5 8 13 21 34 55)",
        &mut HashMap::new()
    );
}

#[test]
fn test_set() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!("(define x 1)", env.clone(), nil!(), &mut HashMap::new());
    assert_eval!(
        "x",
        env.clone(),
        Node::Number(Number::Int(1)),
        &mut HashMap::new()
    );
    assert_eval!("(set! x 2)", env.clone(), nil!(), &mut HashMap::new());
    assert_eval!(
        "x",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
    assert_eval!(
        "((lambda (a) (set! x a)) 3)",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "x",
        env.clone(),
        Node::Number(Number::Int(3)),
        &mut HashMap::new()
    );
}

#[test]
fn test_set_car() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        "(define x '(1 2 3))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!("(set-car! x 4)", env.clone(), nil!(), &mut HashMap::new());
    assert_eval!(
        "x",
        env.clone(),
        Node::Pair(
            Node::Number(Number::Int(4)).into(),
            Node::Pair(
                Node::Number(Number::Int(2)).into(),
                Node::Pair(Node::Number(Number::Int(3)).into(), nil!().into()).into()
            )
            .into()
        ),
        &mut HashMap::new()
    );
}

#[test]
fn test_set_cdr() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        "(define x '(1 2 3))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(set-cdr! x '(4 5 6))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "x",
        env.clone(),
        vec_to_list!(
            Node::Number(Number::Int(1)).into(),
            Node::Number(Number::Int(4)).into(),
            Node::Number(Number::Int(5)).into(),
            Node::Number(Number::Int(6)).into()
        ),
        &mut HashMap::new()
    );
    assert_eval!(
        "(define (g x) (set-cdr! x 1))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(set! x (cons 2 3))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!("(g x)", env.clone(), nil!(), &mut HashMap::new());
    assert_eval!(
        "x",
        env.clone(),
        Node::Pair(
            Node::Number(Number::Int(2)).into(),
            Node::Number(Number::Int(1)).into()
        ),
        &mut HashMap::new()
    );
}

#[test]
fn test_begin() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        "(begin 1 2 3)",
        env.clone(),
        Node::Number(Number::Int(3)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(begin (define x 1) (define y 2) x)",
        env.clone(),
        Node::Number(Number::Int(1)),
        &mut HashMap::new()
    );
    assert_eval!(
        "y",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
}

#[test]
fn test_let() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        "(let ((x 1) (y 2)) (+ x y))",
        env.clone(),
        Node::Number(Number::Int(3)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(let ((x 1) (y 2)) (begin (define z (+ x y)) z))",
        env.clone(),
        Node::Number(Number::Int(3)),
        &mut HashMap::new()
    );
}

#[test]
fn test_if() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        "(if 1 2 3)",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(if 0 2 3)",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(if 't 2 3)",
        env.clone(),
        Node::Number(Number::Int(2)),
        &mut HashMap::new()
    );
    assert_eval!(
        "(if '() 2 3)",
        env.clone(),
        Node::Number(Number::Int(3)),
        &mut HashMap::new()
    );
}

#[test]
fn test_reverse_list() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!(
        r#"
(define reverse
  (lambda (x)
    (begin
      (define loop
        (lambda (x y)
          (cond ((eq? x '()) y)
                ('t (begin
                    (define temp (cdr x))
                    (set-cdr! x y)
                    (display x)
                    (display temp)
                    (loop temp x))))))
      (loop x '()))))"#,
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        r#"
(define (reverse-sugar x)
  (define (loop x y)
    (cond ((eq? x '()) y)
          ('t (define temp (cdr x))
              (set-cdr! x y)
              (loop temp x))))
  (loop x '()))"#,
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    assert_eval!(
        "(reverse '(1 2 3 4))",
        env.clone(),
        vec_to_list!(
            Node::Number(Number::Int(4)).into(),
            Node::Number(Number::Int(3)).into(),
            Node::Number(Number::Int(2)).into(),
            Node::Number(Number::Int(1)).into()
        ),
        &mut HashMap::new()
    );
    assert_eval!(
        "(reverse-sugar '(1 2 3 4))",
        env.clone(),
        vec_to_list!(
            Node::Number(Number::Int(4)).into(),
            Node::Number(Number::Int(3)).into(),
            Node::Number(Number::Int(2)).into(),
            Node::Number(Number::Int(1)).into()
        ),
        &mut HashMap::new()
    );
}

#[test]
fn test_delay() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    let mut macros = HashMap::new();
    assert_eval!(
        "(define-syntax-rule (delay exp) (lambda () exp))",
        env.clone(),
        nil!(),
        &mut macros
    );
    assert_eval!(
        "(define (force delayed-object) (delayed-object))",
        env.clone(),
        nil!(),
        &mut macros
    );
    assert_eval!(
        "(force (delay 1))",
        env.clone(),
        Node::Number(Number::Int(1)),
        &mut macros
    );
}

#[test]
fn test_display() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    let mut result = ConsoleEval::new();
    let mut macros = HashMap::new();
    result = assert_eval_print!("(display 1)", env.clone(), result.clone(), macros);
    result = assert_eval_print!("(newline)", env.clone(), result.clone(), macros);
    result = assert_eval_print!("(display '())", env.clone(), result.clone(), macros);
    result = assert_eval_print!("(newline)", env.clone(), result.clone(), macros);
    result = assert_eval_print!("(display '(1 2 3))", env.clone(), result.clone(), macros);
    result = assert_eval_print!("(display '\"4 5\n6\")", env.clone(), result.clone(), macros);
    assert_eq!(
        result.display_output,
        Some("1\nnil\n(1 2 3)4 5\n6".to_string())
    );
    assert_eq!(result.graphviz_output, None);
}

#[test]
fn test_cycle() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval_print!(
        r#"
(define (last-pair x)
    (if (eq? (cdr x) '()) x (last-pair (cdr x))))"#,
        env.clone()
    );
    assert_eval_print!(
        r#"
(define (make-cycle x)
    (define y (last-pair x))
    (set-car! y x)
    x)"#,
        env.clone()
    );
    assert_eval_print!("(define z (make-cycle (list 'a 'b 'c)))", env.clone());
    assert_eval_print!("(display z)", env.clone());
    assert_eval_print!(
        r#"
(define (make-cycle2 x)
    (define y (last-pair x))
    (set-cdr! y x)
    x)"#,
        env.clone()
    );
    assert_eval_print!("(define z2 (make-cycle2 (list 'a 'b 'c)))", env.clone());
    assert_eval_print!("(display z2)", env.clone());
}

#[test]
fn test_env_graphviz_simple() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval_print!(
        "(define x 1)",
        env.clone(),
        ConsoleEval::new(),
        HashMap::new()
    );
    assert_eval_print!(
        "(define y 2)",
        env.clone(),
        ConsoleEval::new(),
        HashMap::new()
    );
    println!("\nSimple Environment Graph:");
    let result = assert_eval_print!(
        "(graphviz)",
        env.clone(),
        ConsoleEval::new(),
        HashMap::new()
    );
    assert!(result.display_output.is_none());
    println!("{}", result.graphviz_output.unwrap());
}

#[test]
fn test_env_graphviz_nested() {
    let outer = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    outer.borrow_mut().define(
        &"x".to_string(),
        Rc::new(RefCell::new(Node::Number(Number::Int(42)))),
        &mut (),
    );

    let inner = Rc::new(RefCell::new(NodeEnv::new(
        Some(outer.clone()),
        HashMap::new(),
        "env",
    )));
    inner.borrow_mut().define(
        &"y".to_string(),
        Rc::new(RefCell::new(Node::Number(Number::Int(24)))),
        &mut (),
    );

    println!("\nNested Environment Graph:");
    let result = assert_eval_print!(
        "(graphviz)",
        inner.clone(),
        ConsoleEval::new(),
        &mut HashMap::new()
    );
    assert!(result.display_output.is_none());
    println!("{}", result.graphviz_output.unwrap());
}

#[test]
fn test_env_graphviz_lambda() {
    let env = Rc::new(RefCell::new(NodeEnv::top(&mut ())));
    assert_eval!("(define x 1)", env.clone(), nil!(), &mut HashMap::new());
    assert_eval!(
        "(define (f x) (+ x 1))",
        env.clone(),
        nil!(),
        &mut HashMap::new()
    );
    println!("\nLambda Environment Graph:");
    let result = assert_eval_print!(
        "(graphviz)",
        env.clone(),
        ConsoleEval::new(),
        &mut HashMap::new()
    );
    assert!(result.display_output.is_none());
    println!("{}", result.graphviz_output.unwrap());
}
