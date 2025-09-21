use std::{collections::HashMap, ffi::CString, process::Command};

use relic::lexer::{Lexer, Number};
use relic::logger::{LogLevel, set_log_level};
use relic::node::Node;
use relic::parser::Parse;
use relic::preprocess::PreProcess;
use relic::runtime::{DbgState, Runtime, RuntimeNode, StackMachine};
use relic::symbol::Symbol;
use relic::{RT, rt_pop, rt_start};
use relic::{
    compile::{self, CodeGen},
    rt_get, rt_import,
};
use serial_test::serial;
use std::sync::atomic::AtomicUsize;
use std::{io::Write, process::Stdio};

fn compile_and_load(input: &str, lib_name: &str) {
    let mut tokens = Lexer::new(input);
    let mut macros = HashMap::new();
    let mut codegen = CodeGen::new_library(lib_name.to_string());

    while let Ok(mut node) = Node::parse(&mut tokens) {
        let node = node.preprocess(&mut macros).unwrap();
        compile::compile(&node, &mut codegen, false).unwrap();
    }
    let c_code = codegen.to_string();
    std::fs::write(format!("/tmp/relic_{lib_name}.c"), c_code).unwrap();

    let status = Command::new("gcc")
        .args([
            "-Ic_runtime",
            "-shared",
            "-fPIC",
            "-o",
            &format!("./lib/{lib_name}.relic"),
            &format!("/tmp/relic_{lib_name}.c"),
            #[cfg(target_os = "macos")]
            "-Wl,-undefined,dynamic_lookup",
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    assert!(status.success());

    rt_start();
    let c_str = CString::new(lib_name).unwrap();
    rt_import(c_str.as_bytes().as_ptr());
}

fn get_value(name: &str) -> usize {
    let c_str = CString::new(name).unwrap();
    rt_get(c_str.as_bytes().as_ptr())
}

#[test]
#[serial]
fn compile_test_simple() {
    compile_and_load("(define x (+ 1 2))", "mylib");

    let x = get_value("x");
    let mut runtime = RT.write().unwrap();
    let val = runtime.get_number(x).unwrap();
    assert_eq!(val, Number::Int(3));
    runtime.clear();
    std::fs::remove_file("lib/mylib.relic").unwrap();
}

macro_rules! assert_eval_node {
    ($code:expr, $expected:expr) => {{
        let mut macros = HashMap::new();
        let mut tokens = Lexer::new($code);
        let mut node = Node::parse(&mut tokens).unwrap();
        node = node.preprocess(&mut macros).unwrap();

        node.jit_compile(true).unwrap();
        let expected = {
            let mut runtime = RT.write().unwrap();
            runtime.new_node_with_gc($expected)
        };
        let index = rt_pop();
        assert!(RT.read().unwrap().node_eq(index, expected));
    }};

    ($code:expr, $expected:expr, $macros:expr) => {{
        let mut tokens = Lexer::new($code);
        let mut node = Node::parse(&mut tokens).unwrap();
        node = node.preprocess(&mut $macros).unwrap();

        node.jit_compile(true).unwrap();
        let expected = {
            let mut runtime = RT.write().unwrap();
            runtime.new_node_with_gc($expected)
        };
        let index = rt_pop();
        assert!(RT.read().unwrap().node_eq(index, expected));
    }};
}

macro_rules! assert_eval_text {
    ($code:expr, $expected:expr) => {{
        let mut macros = HashMap::new();
        let mut tokens = Lexer::new($code);
        let mut node = Node::parse(&mut tokens).unwrap();
        node = node.preprocess(&mut macros).unwrap();

        node.jit_compile(true).unwrap();
        let index = rt_pop();
        let actual = RT.read().unwrap().display_node_idx(index);
        assert_eq!(actual, $expected);
    }};

    ($code:expr, $expected:expr, $macros:expr) => {{
        let mut tokens = Lexer::new($code);
        let mut node = Node::parse(&mut tokens).unwrap();
        node = node.preprocess(&mut $macros).unwrap();

        node.jit_compile(true).unwrap();
        let index = rt_pop();
        let actual = RT.lock().unwrap().display_node_idx(index);
        assert_eq!(actual, $expected)
    }};
}
#[test]
#[serial]
fn test_cycle_eval() {
    rt_start();
    assert_eval_text!(
        "(define (last-pair x) (if (eq? (cdr x) '()) x (last-pair (cdr x))))",
        "nil"
    );
    assert_eval_text!(
        "(define (make-cycle x) (define y (last-pair x)) (set-car! y x) x)",
        "nil"
    );
    assert_eval_text!("(define z (make-cycle (list 'a 'b 'c)))", "nil");
    assert_eval_text!("z", "(a b #0#)");
    assert_eval_text!(
        "(define (make-cycle2 x) (define y (last-pair x)) (set-cdr! y x) x)",
        "nil"
    );
    assert_eval_text!("(define z2 (make-cycle2 (list 'a 'b 'c)))", "nil");
    assert_eval_text!("z2", "(a b c . #0#)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_set_car_eval() {
    rt_start();
    assert_eval_text!("(define x '(1 2 3))", "nil");
    assert_eval_text!("(set-car! x 4)", "nil");
    assert_eval_text!("x", "(4 2 3)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_set_cdr_eval() {
    rt_start();
    assert_eval_text!("(define x '(1 2 3))", "nil");
    assert_eval_text!("(set-cdr! x '(4 5 6))", "nil");
    assert_eval_text!("x", "(1 4 5 6)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_fact_eval() {
    rt_start();
    assert_eval_node!(
        "(define fact (lambda (n acc) (cond ((< n 2) acc) ('t (fact (- n 1) (* n acc))))))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(define x (fact 5 1))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("x", RuntimeNode::Number(Number::Int(120)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_or_eval() {
    rt_start();
    assert_eval_node!("(define x1 (or))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(define x2 (or '() 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(define x3 (or 1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text!("x1", "nil");
    assert_eval_text!("x2", "2");
    assert_eval_text!("x3", "1");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_and_eval() {
    rt_start();
    assert_eval_node!("(define x1 (and))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!(
        "(define x2 (and '() 2 3))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(define x3 (and 1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text!("x1", "t");
    assert_eval_text!("x2", "nil");
    assert_eval_text!("x3", "3");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_cond_eval() {
    rt_start();
    assert_eval_node!(
        "(define x1 (cond ((< 1 2) 1) ((> 1 2) 2)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define x2 (cond ((> 1 2) 1) ((< 1 2) 2)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define x3 (cond ((> 1 2) 1)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define x4 (cond ((> 1 2) 1) ((> 1 2) 2)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text!("x1", "1");
    assert_eval_text!("x2", "2");
    assert_eval_text!("x3", "nil");
    assert_eval_text!("x4", "nil");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
fn test_simple_expr_eval() {
    assert_eval_text!("(+ (* 1 2 3) (/ 3 4))", "6.75");
}

#[test]
fn test_simple_lambda_eval() {
    assert_eval_text!("((lambda (x y z) (- x ((lambda (x) z) y))) 3 4 1)", "2");
}

#[test]
#[serial]
fn test_lambda_pattern_matching_eval() {
    rt_start();
    assert_eval_node!(
        "(define f (lambda x (car x)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(define (g . x) (car x))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!(
        "(define h (lambda (x . y) (car y)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define x1 (f 'a 'b 3 4))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(define x2 (g 2 3 4))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(define x3 (h 1 2 3 4))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(define x4 (h 1 2))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(define x5 (h 1 't))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text!("x1", "a");
    assert_eval_text!("x2", "2");
    assert_eval_text!("x3", "2");
    assert_eval_text!("x4", "2");
    assert_eval_text!("x5", "t");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_let_eval() {
    rt_start();
    assert_eval_text!("(let ((x 1) (y 2)) (+ x y))", "3");
    assert_eval_node!(
        "(let ((x 1) (y 2)) (define z (+ x y)) z)",
        RuntimeNode::Number(Number::Int(3))
    );
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_set_eval() {
    rt_start();
    assert_eval_node!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(define x1 x)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(set! x 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(define x2 x)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!(
        "((lambda (a) (set! x a)) 3)",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("x", RuntimeNode::Number(Number::Int(3)));
    assert_eval_node!("x1", RuntimeNode::Number(Number::Int(1)));
    assert_eval_node!("x2", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_fib_eval() {
    rt_start();
    assert_eval_node!(
        "(define (fib x) (if (< x 2) x (+ (fib (- x 1)) (fib (- x 2)))))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define map (lambda (func l) (cond ((eq? l '()) '()) ('t (cons (func (car l)) (map func (cdr l)))))))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define z (map fib '(0 1 2 3 4 5 6 7 8 9 10)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text!("z", "(0 1 1 2 3 5 8 13 21 34 55)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_reverse_eval() {
    rt_start();
    assert_eval_node!(
        "(define (aux lst acc) (if (eq? lst '()) acc (aux (cdr lst) (cons (car lst) acc))))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define (reverse lst) (aux lst '()))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define z (reverse '(1 2 3 4 5 6 7 8 9 10)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text!("z", "(10 9 8 7 6 5 4 3 2 1)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_reverse_2_eval() {
    rt_start();
    assert_eval_node!(
        "(define (reverse x) (define (loop x y) (cond ((eq? x '()) y) ('t (define temp (cdr x)) (set-cdr! x y) (loop temp x)))) (loop x '()))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define z (reverse '(1 2 3 4 5 6 7 8 9 10)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text!("z", "(10 9 8 7 6 5 4 3 2 1)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_sqrt_eval() {
    rt_start();
    assert_eval_node!(
        "(define (sqrt-iter guess x) (if (good-enough? guess x) guess (sqrt-iter (improve guess x) x)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define (improve guess x) (average guess (/ x guess)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define (average x y) (/ (+ x y) 2))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define (good-enough? guess x) (< (abs (- (* guess guess) x)) 0.001))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!(
        "(define (sqrt x) (sqrt-iter 1.0 x))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(define z (sqrt 2))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("z", RuntimeNode::Number(Number::Float(1.4142156862745097)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_primitive() {
    rt_start();
    assert_eval_node!("42", RuntimeNode::Number(Number::Int(42)));
    assert_eval_node!("4.2", RuntimeNode::Number(Number::Float(4.2)));
    assert_eval_node!(
        "\"hello  \"",
        RuntimeNode::Symbol(Symbol::User("hello  ".to_string()))
    );

    let mut runtime = RT.write().unwrap();
    runtime.clear();
}
#[test]
#[serial]
fn test_simple_arithmetic() {
    rt_start();
    assert_eval_node!("(+ 1 2 3 4)", RuntimeNode::Number(Number::Int(10)));
    assert_eval_node!("(- 3 2 1)", RuntimeNode::Number(Number::Int(0)));
    assert_eval_node!("(remainder 10 3)", RuntimeNode::Number(Number::Int(1)));
    assert_eval_node!("(quotient 20 3)", RuntimeNode::Number(Number::Int(6)));
    assert_eval_node!("(floor 2.5)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(ceiling 2.5)", RuntimeNode::Number(Number::Int(3)));
    assert_eval_node!("(sin 0)", RuntimeNode::Number(Number::Float(0.0)));
    assert_eval_node!("(cos 0)", RuntimeNode::Number(Number::Float(1.0)));
    assert_eval_node!("(abs 2)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(abs -2)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(abs 2.0)", RuntimeNode::Number(Number::Float(2.0)));
    assert_eval_node!("(abs -2.0)", RuntimeNode::Number(Number::Float(2.0)));
    assert_eval_node!("(* 2 3)", RuntimeNode::Number(Number::Int(6)));
    assert_eval_node!("(/ 6 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(/ 5 2)", RuntimeNode::Number(Number::Float(2.5)));
    assert_eval_node!("(+ 1.0 2.0 3)", RuntimeNode::Number(Number::Float(6.0)));
    assert_eval_node!("(- 3.0 2.0)", RuntimeNode::Number(Number::Float(1.0)));
    assert_eval_node!("(* 2.0 3.0)", RuntimeNode::Number(Number::Float(6.0)));
    assert_eval_node!("(/ 6.0 3.0)", RuntimeNode::Number(Number::Float(2.0)));
    assert_eval_node!("(+ 1 2.0)", RuntimeNode::Number(Number::Float(3.0)));
    assert_eval_node!("(- 3.0 2)", RuntimeNode::Number(Number::Float(1.0)));
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    assert_eval_node!(
        "(define (h x) (define (f x) x) (f 2))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(h 1)", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.write().unwrap();
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
        "(define (g x . y) (car (cdr y)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(g 2 3 4)", RuntimeNode::Number(Number::Int(4)));

    assert_eval_node!("(define (g x . y) y)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(g 2)", RuntimeNode::Symbol(Symbol::Nil));

    assert_eval_node!(
        "(define h (lambda (x . y) (car y)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(h 1 2 3 4)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(h 1 2)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(h 1 't)", RuntimeNode::Symbol(Symbol::T));

    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_apply() {
    rt_start();
    assert_eval_node!(
        "(define f (lambda (x y z) (+ x y)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node!("(apply f '(1 2 3))", RuntimeNode::Number(Number::Int(3)));

    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_and() {
    rt_start();
    assert_eval_node!("(and)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node!("(and '() 2 3)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(and 1 2 3)", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_or() {
    rt_start();
    assert_eval_node!("(or)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(or '() 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node!("(or 1 2 3)", RuntimeNode::Number(Number::Int(1)));
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_list_package() {
    rt_start();
    assert_eval_node!("(import list)", RuntimeNode::Symbol(Symbol::Nil));
    // import twice should not break anything
    assert_eval_node!("(import list)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text!(
        "(map (lambda (x) (+ x 1)) '(0 1 2 3 4 5 6 7 8 9))",
        "(1 2 3 4 5 6 7 8 9 10)"
    );
    assert_eval_text!(
        "(list-tail (map (lambda (x) (- x 1)) (iota 10 2 1)) 5)",
        "(6 7 8 9 10)"
    );
    assert_eval_text!("(map + '(1 2 3) '(3 2 1) '(3 3 3))", "(7 7 7)");
    assert_eval_text!("(append '((1 2) 3) '(4 5) '(6))", "((1 2) 3 4 5 6)");
    let mut runtime = RT.write().unwrap();
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
    assert_eval_node!("(fib 9)", RuntimeNode::Number(Number::Int(55)));
    assert_eval_node!("(import list)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text!("(map fib (iota 10))", "(1 1 2 3 5 8 13 21 34 55)");
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_set_car() {
    rt_start();
    assert_eval_node!("(define x '(1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(set-car! x 4)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text!("x", "(4 2 3)");
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
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
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn run_c_test() {
    set_log_level(relic::logger::LogLevel::Debug);
    let status = Command::new("gcc")
        .args([
            "-Ic_runtime",
            "-shared",
            "-fPIC",
            "-O3",
            "-g",
            "-o",
            "lib/test.relic",
            "tests/test.c",
            #[cfg(target_os = "macos")]
            "-Wl,-undefined,dynamic_lookup",
        ])
        .spawn()
        .unwrap()
        .wait()
        .unwrap();
    println!("{status}");
    assert!(status.success());
    rt_start();
    assert_eval_node!("(import test)", RuntimeNode::Symbol(Symbol::Nil));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
    std::fs::remove_file("lib/test.relic").unwrap();
}

pub static COUNT: AtomicUsize = AtomicUsize::new(0);

#[test]
#[serial]
fn debug_test() {
    fn test_callback(rt: &Runtime) -> DbgState {
        if COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed) == 0 {
            assert!(rt.empty());
        } else {
            println!("{}", rt.display_node_idx(rt.top()));
        }
        println!("{rt}");
        DbgState::Next
    }
    rt_start();
    set_log_level(LogLevel::Debug);
    {
        let mut runtime = RT.write().unwrap();
        runtime.set_callback(test_callback);
    }
    assert_eval_node!("(define (f x) (* x 2))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(breakpoint)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node!("(+ 1 (f 2))", RuntimeNode::Number(Number::Int(5)));
    {
        let mut runtime = RT.write().unwrap();
        runtime.clear();
    }
}

#[test]
fn test_run_monoidal() {
    let cmd = Command::new(env!("CARGO_BIN_EXE_relic"))
        .args(["run", "-i", "examples/monoidal.lisp"])
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let out = cmd.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        r#"(nil 2)
(t -2)
(5)
(-5)
(6)
(2 -13)
(-1 7)result: nil
"#
    );
}

#[test]
fn test_run_repl() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_relic"))
        .args(["run", "-i", "examples/interpreter.lisp"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let mut stdin = cmd.stdin.take().unwrap();
        stdin
            .write_all(
                br#"
(define fib
  (lambda (x)
    (cond ((= x 0) 0)
          ((= x 1) 1)
          ('t (+ (fib (- x 2)) (fib (- x 1)))))))
(define map
  (lambda (f l)
    (cond ((eq? l '()) '())
          ('t (cons (f (car l))
                    (map f (cdr l)))))))
(map fib '(1 2 3 4 5 6 7 8 9 10))
nil"#,
            )
            .unwrap();
    }

    let out = cmd.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        r#"> = nil
> = nil
> = (1 1 2 3 5 8 13 21 34 55)
> result: nil
"#
    );
}

#[test]
fn test_repl() {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_relic"))
        .args(["repl"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();

    {
        let mut stdin = cmd.stdin.take().unwrap();
        stdin
            .write_all(
                br#"
123
(+ 2 3 4 5)
(display "hello\n")
(define (f x) (* x 6))
(f 7)
"#,
            )
            .unwrap();
    }

    let out = cmd.wait_with_output().unwrap();
    assert!(out.status.success());
    assert_eq!(
        String::from_utf8(out.stdout).unwrap(),
        r#"Relic REPL. Press Ctrl+D or type 'exit' to quit.
= 123
= 14
hello
= nil
= nil
= 42
CTRL-D
"#
    );
}
