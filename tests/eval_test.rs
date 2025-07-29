use relic::lexer::{Lexer, Number};
use relic::node::Node;
use relic::parser::Parse;
use relic::preprocess::PreProcess;
use relic::runtime::RuntimeNode;
use relic::symbol::Symbol;
use relic::{RT, rt_pop, rt_start};
use serial_test::serial;
use std::collections::HashMap;

macro_rules! assert_eval_node {
    ($code:expr, $expected:expr) => {{
        let mut macros = HashMap::new();
        let mut tokens = Lexer::new($code);
        let mut node = Node::parse(&mut tokens).unwrap();
        node = node.preprocess(&mut macros).unwrap();
        node.jit_compile().unwrap();
        let expected = {
            let mut runtime = RT.lock().unwrap();
            runtime.new_node_with_gc($expected)
        };
        let index = rt_pop();
        println!("{}", RT.lock().unwrap().display_node_idx(index));
        assert!(RT.lock().unwrap().node_eq(index, expected))
    }};

    ($code:expr, $expected:expr, $macros:expr) => {{
        let mut tokens = Lexer::new($code);
        let mut node = Node::parse(&mut tokens).unwrap();
        node = node.preprocess(&mut $macros).unwrap();
        node.jit_compile().unwrap();
        let expected = {
            let mut runtime = RT.lock().unwrap();
            runtime.new_node_with_gc($expected)
        };
        let index = rt_pop();
        assert!(RT.lock().unwrap().node_eq(index, expected))
    }};
}

macro_rules! assert_eval_text {
    ($code:expr, $expected:expr) => {{
        let mut macros = HashMap::new();
        let mut tokens = Lexer::new($code);
        let mut node = Node::parse(&mut tokens).unwrap();
        node = node.preprocess(&mut macros).unwrap();
        node.jit_compile().unwrap();
        let index = rt_pop();
        let actual = RT.lock().unwrap().display_node_idx(index);
        assert_eq!(actual, $expected)
    }};

    ($code:expr, $expected:expr, $macros:expr) => {{
        let mut tokens = Lexer::new($code);
        let mut node = Node::parse(&mut tokens).unwrap();
        node = node.preprocess(&mut $macros).unwrap();
        node.jit_compile().unwrap();
        let index = rt_pop();
        let actual = RT.lock().unwrap().display_node_idx(index);
        assert_eq!(actual, $expected)
    }};
}

#[test]
#[serial]
fn test_int() {
    rt_start();
    assert_eval_node!("42", RuntimeNode::Number(Number::Int(42)));

    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}
#[test]
#[serial]
fn test_simple_arithmetic() {
    rt_start();
    assert_eval_node!("(+ 1 2 3 4)", RuntimeNode::Number(Number::Int(10)));
    assert_eval_node!("(- 3 2 1)", RuntimeNode::Number(Number::Int(0)));
    assert_eval_node!("(* 2 3)", RuntimeNode::Number(Number::Int(6)));
    assert_eval_node!("(/ 6 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(/ 5 2)", RuntimeNode::Number(Number::Float(2.5)));
    assert_eval_node!("(+ 1.0 2.0 3)", RuntimeNode::Number(Number::Float(6.0)));
    assert_eval_node!("(- 3.0 2.0)", RuntimeNode::Number(Number::Float(1.0)));
    assert_eval_node!("(* 2.0 3.0)", RuntimeNode::Number(Number::Float(6.0)));
    assert_eval_node!("(/ 6.0 3.0)", RuntimeNode::Number(Number::Float(2.0)));
    assert_eval_node!("(+ 1 2.0)", RuntimeNode::Number(Number::Float(3.0)));
    assert_eval_node!("(- 3.0 2)", RuntimeNode::Number(Number::Float(1.0)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_with_symbol() {
    rt_start();
    assert_eval_node!("(define x 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(- x 2)", RuntimeNode::Number(Number::Int(0)));
    assert_eval_node!("(* 3 x)", RuntimeNode::Number(Number::Int(6)));
    assert_eval_node!("(/ 6 x)", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_nested_arithmetic() {
    rt_start();
    assert_eval_node!("(+ (- 1 2) 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!(
        "(* (/ 1 2) (+ 3 4))",
        RuntimeNode::Number(Number::Float(3.5))
    );
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_relational_operators() {
    rt_start();
    assert_eval_node!("t", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(< 1.0 2)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(< 2 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(> 1 2.0)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(> (+ 1 1) 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(<= 1 1.0)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(<= 1.0 2)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(<= 2 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(>= 1 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(>= 1 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(>= 2 1)", RuntimeNode::Symbol(Symbol::T));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_toplevel_symbol() {
    rt_start();
    assert_eval_node!("+", RuntimeNode::Symbol(Symbol::Add));
    assert_eval_node!("-", RuntimeNode::Symbol(Symbol::Sub));
    assert_eval_node!("*", RuntimeNode::Symbol(Symbol::Mul));
    assert_eval_node!("/", RuntimeNode::Symbol(Symbol::Div));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_list_simple() {
    assert_eval_text!("'(1 2 3)", "(1 2 3)");
    assert_eval_text!("(list 1 2 3)", "(1 2 3)");
    assert_eval_text!("(list 1 2)", "(1 2)");
    assert_eval_text!("(list (+ 1 0))", "(1)");
    assert_eval_node!("(list)", RuntimeNode::Symbol(Symbol::Nil));
}

#[test]
#[serial]
fn test_list_nested() {
    assert_eval_text!("(list (list 1 2 3))", "((1 2 3))");
    assert_eval_text!("(list 1 '(2 3) 4)", "(1 (2 3) 4)");
}

#[test]
#[serial]
fn test_list_manipulation() {
    assert_eval_node!("(car (list 1 2 3))", RuntimeNode::Number(Number::Int(1)));
    assert_eval_text!("(cdr '(1 2 3))", "(2 3)");
    assert_eval_text!("(cons 1 (list 2 3))", "(1 2 3)");
}

#[test]
#[serial]
fn test_define() {
    rt_start();
    assert_eval_node!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(define y (+ x 1))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("x", RuntimeNode::Number(Number::Int(1)));
    assert_eval_node!("y", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_define_syntax_rule() {
    rt_start();
    let mut macros = HashMap::new();
    assert_eval_node!(
        "(define-syntax-rule (macro1 x) (display 1) (+ x 1))",
        RuntimeNode::Symbol(Symbol::Nil),
        macros
    );
    assert_eval_node!(
        "(define-syntax-rule (macro2 x) (car x))",
        RuntimeNode::Symbol(Symbol::Nil),
        macros
    );
    assert_eval_node!("(macro1 2)", RuntimeNode::Number(Number::Int(3)), macros);
    assert_eval_node!(
        "(macro2 '(1 2 3))",
        RuntimeNode::Number(Number::Int(1)),
        macros
    );
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_lambda() {
    rt_start();
    assert_eval_node!(
        "((lambda (x) (+ x 1)) 2)",
        RuntimeNode::Number(Number::Int(3))
    );
    assert_eval_node!(
        "((lambda (x y) (+ x y)) 2 3)",
        RuntimeNode::Number(Number::Int(5))
    );
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_lambda_with_define() {
    rt_start();
    assert_eval_node!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("x", RuntimeNode::Number(Number::Int(1)));
    assert_eval_node!(
        "(define func (lambda (x) (+ x 1)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(func 2)", RuntimeNode::Number(Number::Int(3)));
    assert_eval_node!("(func x)", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_lambda_scope() {
    rt_start();
    assert_eval_node!("(define (f) 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!(
        "(define (g) (define (f x) x) (f 2))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(g)", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_lambda_pattern_matching() {
    rt_start();
    assert_eval_node!(
        "(define f (lambda x (car x)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(f 'a 'b 3 4)",
        RuntimeNode::Symbol(Symbol::User("a".to_string()))
    );
    assert_eval_node!("(define (g . x) (car x))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(g 2 3 4)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!(
        "(define h (lambda (x . y) (car y)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(h 1 2 3 4)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(h 1 2)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(h 1 't)", RuntimeNode::Symbol(Symbol::T));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_function_call() {
    rt_start();
    assert_eval_node!(
        "(define g (lambda (x) (+ x 1)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define h (lambda (x) (g (+ x 1))))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(h 2)", RuntimeNode::Number(Number::Int(4)));
    assert_eval_node!(
        "(define a (lambda (x) (car x)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(a '(1 2 3))", RuntimeNode::Number(Number::Int(1)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_atom() {
    rt_start();
    assert_eval_node!("(atom? 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(atom? (+ 1 2))", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(atom? (list 1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(atom? 'a)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(atom? '())", RuntimeNode::Symbol(Symbol::T));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_eq() {
    rt_start();
    assert_eval_node!("(eq? 1 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(eq? (- 2 1) 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(eq? 1 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(eq? 'a 'a)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(eq? 'a 'b)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(eq? '() '())", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!(
        "(eq? '(1 2 3) (list 1 2 3))",
        RuntimeNode::Symbol(Symbol::T)
    );
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_number() {
    rt_start();
    assert_eval_node!("(number? 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(number? (+ 1 2))", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(number? 'a)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(number? '())", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(number? 'a)", RuntimeNode::Symbol(Symbol::Nil));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_cond() {
    rt_start();
    assert_eval_node!(
        "(cond ((< 1 2) 1) ((> 1 2) 2))",
        RuntimeNode::Number(Number::Int(1))
    );
    assert_eval_node!(
        "(cond ((> 1 2) 1) ((< 1 2) 2))",
        RuntimeNode::Number(Number::Int(2))
    );
    assert_eval_node!("(cond ((> 1 2) 1))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!(
        "(cond ((> 1 2) 1) ((> 1 2) 2))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_and() {
    rt_start();
    assert_eval_node!("(and)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(and '() 2 3)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(and 1 2 3)", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_or() {
    rt_start();
    assert_eval_node!("(or)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(or '() 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(or 1 2 3)", RuntimeNode::Number(Number::Int(1)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_fact() {
    rt_start();
    assert_eval_node!(
        r#"
(define fact
  (lambda (n acc)
    (cond ((< n 2) acc)
          ('t (fact (- n 1) (* n acc))))))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(fact 5 1)", RuntimeNode::Number(Number::Int(120)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_fib() {
    rt_start();
    assert_eval_node!(
        r#"(define fib
           (lambda (n)
             (cond ((< n 2) 1)
                   ('t (+ (fib (- n 1)) (fib (- n 2)))))))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        r#"(define map
            (lambda (func l)
            (cond ((eq? l '()) '())
                  ('t (cons (func (car l)) (map func (cdr l)))))))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(fib 9)", RuntimeNode::Number(Number::Int(55)));
    // assert_eval!_eq!(
    //     "(map (lambda (x) (+ x 1)) '(0 1 2 3 4 5 6 7 8 9))",
    //     "(1 2 3 4 5 6 7 8 9 10)",
    // );
    // assert_eval!_eq!(
    //     "(map fib '(0 1 2 3 4 5 6 7 8 9))",
    //     "(1 1 2 3 5 8 13 21 34 55)",
    // );
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_set() {
    rt_start();
    assert_eval_node!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("x", RuntimeNode::Number(Number::Int(1)));
    assert_eval_node!("(set! x 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("x", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!(
        "((lambda (a) (set! x a)) 3)",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("x", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_set_car() {
    rt_start();
    assert_eval_node!("(define x '(1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(set-car! x 4)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text!("x", "(4 2 3)");
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_set_cdr() {
    rt_start();
    assert_eval_node!("(define x '(1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(set-cdr! x '(4 5 6))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text!("x", "(1 4 5 6)");
    assert_eval_node!(
        "(define (g x) (set-cdr! x 1))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(set! x (cons 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(g x)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text!("x", "(2 . 1)");
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_begin() {
    rt_start();
    assert_eval_node!("(begin 1 2 3)", RuntimeNode::Number(Number::Int(3)));
    assert_eval_node!(
        "(begin (define x 1) (define y 2) x)",
        RuntimeNode::Number(Number::Int(1))
    );
    assert_eval_node!("y", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_let() {
    rt_start();
    assert_eval_node!(
        "(let ((x 1) (y 2)) (+ x y))",
        RuntimeNode::Number(Number::Int(3))
    );
    assert_eval_node!(
        "(let ((x 1) (y 2)) (begin (define z (+ x y)) z))",
        RuntimeNode::Number(Number::Int(3))
    );
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_if() {
    rt_start();
    assert_eval_node!("(if 1 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(if 0 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(if 't 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(if '() 2 3)", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_reverse_list() {
    rt_start();
    assert_eval_node!(
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
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        r#"
(define (reverse-sugar x)
  (define (loop x y)
    (cond ((eq? x '()) y)
          ('t (define temp (cdr x))
              (set-cdr! x y)
              (loop temp x))))
  (loop x '()))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text!("(reverse '(1 2 3 4))", "(4 3 2 1)");
    assert_eval_text!("(reverse-sugar '(1 2 3 4))", "(4 3 2 1)");
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_delay() {
    rt_start();
    let mut macros = HashMap::new();
    assert_eval_node!(
        "(define-syntax-rule (delay exp) (lambda () exp))",
        RuntimeNode::Symbol(Symbol::Nil),
        macros
    );
    assert_eval_node!(
        "(define (force delayed-object) (delayed-object))",
        RuntimeNode::Symbol(Symbol::Nil),
        macros
    );
    assert_eval_node!(
        "(force (delay 1))",
        RuntimeNode::Number(Number::Int(1)),
        macros
    );
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_cycle() {
    rt_start();
    assert_eval_node!(
        r#"
(define (last-pair x)
    (if (eq? (cdr x) '()) x (last-pair (cdr x))))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        r#"
(define (make-cycle x)
    (define y (last-pair x))
    (set-car! y x)
    x)"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text!("(make-cycle (list 'a 'b 'c))", "(a b #0#)");
    assert_eval_node!(
        r#"
(define (make-cycle2 x)
    (define y (last-pair x))
    (set-cdr! y x)
    x)"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text!("(make-cycle2 (list 'a 'b 'c))", "(a b c . #0#)");
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}
