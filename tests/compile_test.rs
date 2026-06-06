use std::{collections::HashMap, ffi::CString, process::Command};

use relic::lexer::LexerMonad;
use relic::logger::{LogLevel, set_log_level};
use relic::number::Number;
use relic::preprocess::PreProcess;
use relic::runtime::{DbgState, Runtime, RuntimeNode, StackMachine};
use relic::symbol::Symbol;
use relic::{RT, rt_pop, rt_start};
use relic::{
    compile::{self, CodeGen},
    error::ParseError,
    rt_get, rt_import,
};
use serial_test::serial;
use std::sync::atomic::AtomicUsize;
use std::{io::Write, process::Stdio};

fn compile_and_load(input: &str, lib_name: &str) {
    let mut lexer = LexerMonad::new_unnamed(input);
    let mut macros = HashMap::new();
    let mut codegen = CodeGen::new_library(lib_name.to_string());
    loop {
        match lexer.parse() {
            Ok(node) => {
                let node = node.preprocess(&mut macros).unwrap();
                compile::compile(&node, &mut codegen, false).unwrap();
                lexer = node.get_end();
            }
            Err(ParseError::EOF) => break,
            Err(_) => panic!(),
        }
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
        let tokens = LexerMonad::new_unnamed($code);
        let mut node = tokens.parse().unwrap();
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
        let tokens = LexerMonad::new_unnamed($code);
        let mut node = tokens.parse().unwrap();
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
        let tokens = LexerMonad::new_unnamed($code);
        let mut node = tokens.parse().unwrap();
        node = node.preprocess(&mut macros).unwrap();

        node.jit_compile(true).unwrap();
        let index = rt_pop();
        let actual = RT.read().unwrap().display_node_idx(index);
        assert_eq!(actual, $expected);
    }};

    ($code:expr, $expected:expr, $macros:expr) => {{
        let tokens = LexerMonad::new_unnamed($code);
        let mut node = tokens.parse().unwrap();
        node = node.preprocess(&mut $macros).unwrap();

        node.jit_compile(true).unwrap();
        let index = rt_pop();
        let actual = RT.read().unwrap().display_node_idx(index);
        assert_eq!(actual, $expected);
    }};
}

macro_rules! assert_eval_node_dual {
    ($code:expr, $expected:expr) => {{
        let mut macros = HashMap::new();
        let tokens = LexerMonad::new_unnamed($code);
        let mut node = tokens.parse().unwrap();
        node = node.preprocess(&mut macros).unwrap();

        node.jit_compile(true).unwrap();
        let expected = {
            {
                let mut runtime = RT.write().unwrap();
                runtime.new_node_with_gc($expected)
            }
        };
        let c_index = rt_pop();
        assert!(
            RT.read().unwrap().node_eq(c_index, expected),
            "C backend: {} != expected",
            stringify!($code)
        );

        let mut macros2 = HashMap::new();
        let tokens2 = LexerMonad::new_unnamed($code);
        let mut node2 = tokens2.parse().unwrap();
        node2 = node2.preprocess(&mut macros2).unwrap();
        node2.jit_compile_llvm(true).unwrap();
        let expected2 = {
            {
                let mut runtime = RT.write().unwrap();
                runtime.new_node_with_gc($expected)
            }
        };
        let llvm_index = rt_pop();
        assert!(
            RT.read().unwrap().node_eq(llvm_index, expected2),
            "LLVM backend: {}",
            stringify!($code)
        );
    }};

    ($code:expr, $expected:expr, $macros:expr) => {{
        let tokens = LexerMonad::new_unnamed($code);
        let mut node = tokens.parse().unwrap();
        node = node.preprocess(&mut $macros).unwrap();

        node.jit_compile(true).unwrap();
        let expected = {
            {
                let mut runtime = RT.write().unwrap();
                runtime.new_node_with_gc($expected)
            }
        };
        let c_index = rt_pop();
        assert!(
            RT.read().unwrap().node_eq(c_index, expected),
            "C backend: {} != expected",
            stringify!($code)
        );

        let tokens2 = LexerMonad::new_unnamed($code);
        let mut node2 = tokens2.parse().unwrap();
        node2 = node2.preprocess(&mut $macros).unwrap();
        node2.jit_compile_llvm(true).unwrap();
        let expected2 = {
            {
                let mut runtime = RT.write().unwrap();
                runtime.new_node_with_gc($expected)
            }
        };
        let llvm_index = rt_pop();
        assert!(
            RT.read().unwrap().node_eq(llvm_index, expected2),
            "LLVM backend: {}",
            stringify!($code)
        );
    }};
}

macro_rules! assert_eval_text_dual {
    ($code:expr, $expected:expr) => {{
        let mut macros = HashMap::new();
        let tokens = LexerMonad::new_unnamed($code);
        let mut node = tokens.parse().unwrap();
        node = node.preprocess(&mut macros).unwrap();

        node.jit_compile(true).unwrap();
        let c_index = rt_pop();
        let c_actual = RT.read().unwrap().display_node_idx(c_index);
        assert_eq!(c_actual, $expected, "C backend: {}", stringify!($code));

        let mut macros2 = HashMap::new();
        let tokens2 = LexerMonad::new_unnamed($code);
        let mut node2 = tokens2.parse().unwrap();
        node2 = node2.preprocess(&mut macros2).unwrap();
        node2.jit_compile_llvm(true).unwrap();
        let llvm_index = rt_pop();
        let llvm_actual = RT.read().unwrap().display_node_idx(llvm_index);
        assert_eq!(
            llvm_actual,
            $expected,
            "LLVM backend: {} produced '{}', expected '{}'",
            stringify!($code),
            llvm_actual,
            $expected
        );
    }};

    ($code:expr, $expected:expr, $macros:expr) => {{
        let tokens = LexerMonad::new_unnamed($code);
        let mut node = tokens.parse().unwrap();
        node = node.preprocess(&mut $macros).unwrap();

        node.jit_compile(true).unwrap();
        let c_index = rt_pop();
        let c_actual = RT.read().unwrap().display_node_idx(c_index);
        assert_eq!(c_actual, $expected, "C backend: {}", stringify!($code));

        let tokens2 = LexerMonad::new_unnamed($code);
        let mut node2 = tokens2.parse().unwrap();
        node2 = node2.preprocess(&mut $macros).unwrap();
        node2.jit_compile_llvm(true).unwrap();
        let llvm_index = rt_pop();
        let llvm_actual = RT.read().unwrap().display_node_idx(llvm_index);
        assert_eq!(
            llvm_actual,
            $expected,
            "LLVM backend: {} produced '{}', expected '{}'",
            stringify!($code),
            llvm_actual,
            $expected
        );
    }};
}

#[test]
#[serial]
fn test_simple_expr_eval() {
    rt_start();
    assert_eval_text_dual!("(+ (* 1 2 3) (/ 3 4))", "6.75");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_simple_lambda_eval() {
    rt_start();
    assert_eval_text_dual!("((lambda (x y z) (- x ((lambda (x) z) y))) 3 4 1)", "2");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_lambda_pattern_matching_eval() {
    rt_start();
    assert_eval_node_dual!(
        "(define f (lambda x (car x)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(define (g . x) (car x))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!(
        "(define h (lambda (x . y) (car y)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define x1 (f 'a 'b 3 4))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(define x2 (g 2 3 4))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(define x3 (h 1 2 3 4))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(define x4 (h 1 2))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(define x5 (h 1 't))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text_dual!("x1", "a");
    assert_eval_text_dual!("x2", "2");
    assert_eval_text_dual!("x3", "2");
    assert_eval_text_dual!("x4", "2");
    assert_eval_text_dual!("x5", "t");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_let_eval() {
    rt_start();
    assert_eval_text_dual!("(let ((x 1) (y 2)) (+ x y))", "3");
    assert_eval_node_dual!(
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
    assert_eval_node_dual!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(define x1 x)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(set! x 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(define x2 x)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!(
        "((lambda (a) (set! x a)) 3)",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("x", RuntimeNode::Number(Number::Int(3)));
    assert_eval_node_dual!("x1", RuntimeNode::Number(Number::Int(1)));
    assert_eval_node_dual!("x2", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_fib_eval() {
    rt_start();
    assert_eval_node_dual!(
        "(define (fib x) (if (< x 2) x (+ (fib (- x 1)) (fib (- x 2)))))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define map (lambda (func l) (cond ((eq? l '()) '()) ('t (cons (func (car l)) (map func (cdr l)))))))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define z (map fib '(0 1 2 3 4 5 6 7 8 9 10)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text_dual!("z", "(0 1 1 2 3 5 8 13 21 34 55)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_reverse_eval() {
    rt_start();
    assert_eval_node_dual!(
        "(define (aux lst acc) (if (eq? lst '()) acc (aux (cdr lst) (cons (car lst) acc))))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define (reverse lst) (aux lst '()))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define z (reverse '(1 2 3 4 5 6 7 8 9 10)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text_dual!("z", "(10 9 8 7 6 5 4 3 2 1)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_reverse_2_eval() {
    rt_start();
    assert_eval_node_dual!(
        "(define (reverse x) (define (loop x y) (cond ((eq? x '()) y) ('t (define temp (cdr x)) (set-cdr! x y) (loop temp x)))) (loop x '()))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define z (reverse '(1 2 3 4 5 6 7 8 9 10)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text_dual!("z", "(10 9 8 7 6 5 4 3 2 1)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_sqrt_eval() {
    rt_start();
    assert_eval_node_dual!(
        "(define (sqrt-iter guess x) (if (good-enough? guess x) guess (sqrt-iter (improve guess x) x)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define (improve guess x) (average guess (/ x guess)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define (average x y) (/ (+ x y) 2))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define (good-enough? guess x) (< (abs (- (* guess guess) x)) 0.001))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define (sqrt x) (sqrt-iter 1.0 x))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(define z (sqrt 2))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("z", RuntimeNode::Number(Number::Float(1.4142156862745097)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_primitive() {
    rt_start();
    assert_eval_node_dual!("42", RuntimeNode::Number(Number::Int(42)));
    assert_eval_node_dual!("4.2", RuntimeNode::Number(Number::Float(4.2)));
    assert_eval_node_dual!(
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
    assert_eval_node_dual!("(+ 1 2 3 4)", RuntimeNode::Number(Number::Int(10)));
    assert_eval_node_dual!("(- 3 2 1)", RuntimeNode::Number(Number::Int(0)));
    assert_eval_node_dual!("(remainder 10 3)", RuntimeNode::Number(Number::Int(1)));
    assert_eval_node_dual!("(quotient 20 3)", RuntimeNode::Number(Number::Int(6)));
    assert_eval_node_dual!("(floor 2.5)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!("(ceiling 2.5)", RuntimeNode::Number(Number::Int(3)));
    assert_eval_node_dual!("(sin 0)", RuntimeNode::Number(Number::Float(0.0)));
    assert_eval_node_dual!("(cos 0)", RuntimeNode::Number(Number::Float(1.0)));
    assert_eval_node_dual!("(abs 2)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!("(abs -2)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!("(abs 2.0)", RuntimeNode::Number(Number::Float(2.0)));
    assert_eval_node_dual!("(abs -2.0)", RuntimeNode::Number(Number::Float(2.0)));
    assert_eval_node_dual!("(* 2 3)", RuntimeNode::Number(Number::Int(6)));
    assert_eval_node_dual!("(/ 6 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!("(/ 5 2)", RuntimeNode::Number(Number::Float(2.5)));
    assert_eval_node_dual!("(+ 1.0 2.0 3)", RuntimeNode::Number(Number::Float(6.0)));
    assert_eval_node_dual!("(- 3.0 2.0)", RuntimeNode::Number(Number::Float(1.0)));
    assert_eval_node_dual!("(* 2.0 3.0)", RuntimeNode::Number(Number::Float(6.0)));
    assert_eval_node_dual!("(/ 6.0 3.0)", RuntimeNode::Number(Number::Float(2.0)));
    assert_eval_node_dual!("(+ 1 2.0)", RuntimeNode::Number(Number::Float(3.0)));
    assert_eval_node_dual!("(- 3.0 2)", RuntimeNode::Number(Number::Float(1.0)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_with_symbol() {
    rt_start();
    assert_eval_node_dual!("(define x 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(- x 2)", RuntimeNode::Number(Number::Int(0)));
    assert_eval_node_dual!("(* 3 x)", RuntimeNode::Number(Number::Int(6)));
    assert_eval_node_dual!("(/ 6 x)", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_nested_arithmetic() {
    rt_start();
    assert_eval_node_dual!("(+ (- 1 2) 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!(
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
    assert_eval_node_dual!("t", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(< 1.0 2)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(< 2 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(> 1 2.0)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(> (+ 1 1) 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(<= 1 1.0)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(<= 1.0 2)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(<= 2 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(>= 1 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(>= 1 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(>= 2 1)", RuntimeNode::Symbol(Symbol::T));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_toplevel_symbol() {
    rt_start();
    assert_eval_node_dual!("+", RuntimeNode::Symbol(Symbol::Add));
    assert_eval_node_dual!("-", RuntimeNode::Symbol(Symbol::Sub));
    assert_eval_node_dual!("*", RuntimeNode::Symbol(Symbol::Mul));
    assert_eval_node_dual!("/", RuntimeNode::Symbol(Symbol::Div));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_list_simple() {
    rt_start();
    assert_eval_text_dual!("'(1 2 3)", "(1 2 3)");
    assert_eval_text_dual!("(list 1 2 3)", "(1 2 3)");
    assert_eval_text_dual!("(list 1 2)", "(1 2)");
    assert_eval_text_dual!("(list (+ 1 0))", "(1)");
    assert_eval_node_dual!("(list)", RuntimeNode::Symbol(Symbol::Nil));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_list_nested() {
    rt_start();
    assert_eval_text_dual!("(list (list 1 2 3))", "((1 2 3))");
    assert_eval_text_dual!("(list 1 '(2 3) 4)", "(1 (2 3) 4)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_list_manipulation() {
    rt_start();
    assert_eval_node_dual!("(car (list 1 2 3))", RuntimeNode::Number(Number::Int(1)));
    assert_eval_text_dual!("(cdr '(1 2 3))", "(2 3)");
    assert_eval_text_dual!("(cons 1 (list 2 3))", "(1 2 3)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_define() {
    rt_start();
    assert_eval_node_dual!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(define y (+ x 1))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("x", RuntimeNode::Number(Number::Int(1)));
    assert_eval_node_dual!("y", RuntimeNode::Number(Number::Int(2)));
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
    assert_eval_node_dual!(
        "((lambda (x) (+ x 1)) 2)",
        RuntimeNode::Number(Number::Int(3))
    );
    assert_eval_node_dual!(
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
    assert_eval_node_dual!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("x", RuntimeNode::Number(Number::Int(1)));
    assert_eval_node_dual!(
        "(define func (lambda (x) (+ x 1)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(func 2)", RuntimeNode::Number(Number::Int(3)));
    assert_eval_node_dual!("(func x)", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_lambda_scope() {
    rt_start();
    assert_eval_node_dual!("(define (f) 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!(
        "(define (g) (define (f x) x) (f 2))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(g)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!(
        "(define (h x) (define (f x) x) (f 2))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(h 1)", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_lambda_pattern_matching() {
    rt_start();
    assert_eval_node_dual!(
        "(define f (lambda x (car x)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(f 'a 'b 3 4)",
        RuntimeNode::Symbol(Symbol::User("a".to_string()))
    );

    assert_eval_node_dual!("(define (g . x) (car x))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(g 2 3 4)", RuntimeNode::Number(Number::Int(2)));

    assert_eval_node_dual!(
        "(define (g x . y) (car (cdr y)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(g 2 3 4)", RuntimeNode::Number(Number::Int(4)));

    assert_eval_node_dual!("(define (g x . y) y)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(g 2)", RuntimeNode::Symbol(Symbol::Nil));

    assert_eval_node_dual!(
        "(define h (lambda (x . y) (car y)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(h 1 2 3 4)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!("(h 1 2)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!("(h 1 't)", RuntimeNode::Symbol(Symbol::T));

    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_function_call() {
    rt_start();
    assert_eval_node_dual!(
        "(define g (lambda (x) (+ x 1)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        "(define h (lambda (x) (g (+ x 1))))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(h 2)", RuntimeNode::Number(Number::Int(4)));
    assert_eval_node_dual!(
        "(define a (lambda (x) (car x)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(a '(1 2 3))", RuntimeNode::Number(Number::Int(1)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_apply() {
    rt_start();
    assert_eval_node_dual!(
        "(define f (lambda (x y z) (+ x y)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(apply f '(1 2 3))", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_atom() {
    rt_start();
    assert_eval_node_dual!("(atom? 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(atom? (+ 1 2))", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(atom? (list 1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(atom? 'a)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(atom? '())", RuntimeNode::Symbol(Symbol::T));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_eq() {
    rt_start();
    assert_eval_node_dual!("(eq? 1 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(eq? (- 2 1) 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(eq? 1 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(eq? 'a 'a)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(eq? 'a 'b)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(eq? '() '())", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!(
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
    assert_eval_node_dual!("(number? 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(number? (+ 1 2))", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(number? 'a)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(number? '())", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(number? 'a)", RuntimeNode::Symbol(Symbol::Nil));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_cond() {
    rt_start();
    assert_eval_node_dual!(
        "(cond ((< 1 2) 1) ((> 1 2) 2))",
        RuntimeNode::Number(Number::Int(1))
    );
    assert_eval_node_dual!(
        "(cond ((> 1 2) 1) ((< 1 2) 2))",
        RuntimeNode::Number(Number::Int(2))
    );
    assert_eval_node_dual!("(cond ((> 1 2) 1))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!(
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
    assert_eval_node_dual!("(and)", RuntimeNode::Symbol(Symbol::T));
    assert_eval_node_dual!("(and '() 2 3)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(and 1 2 3)", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_and_eval() {
    rt_start();
    assert_eval_node_dual!("(define x1 (and))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!(
        "(define x2 (and '() 2 3))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(define x3 (and 1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text_dual!("x1", "t");
    assert_eval_text_dual!("x2", "()");
    assert_eval_text_dual!("x3", "3");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_or() {
    rt_start();
    assert_eval_node_dual!("(or)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(or '() 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!("(or 1 2 3)", RuntimeNode::Number(Number::Int(1)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_fact() {
    rt_start();
    assert_eval_node_dual!(
        r#"
(define fact
  (lambda (n acc)
    (cond ((< n 2) acc)
          ('t (fact (- n 1) (* n acc))))))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(fact 5 1)", RuntimeNode::Number(Number::Int(120)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_list_package() {
    rt_start();
    assert_eval_node_dual!("(import list)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(import list)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text_dual!(
        "(map (lambda (x) (+ x 1)) '(0 1 2 3 4 5 6 7 8 9))",
        "(1 2 3 4 5 6 7 8 9 10)"
    );
    assert_eval_text_dual!(
        "(list-tail (map (lambda (x) (- x 1)) (iota 10 2 1)) 5)",
        "(6 7 8 9 10)"
    );
    assert_eval_text_dual!("(map + '(1 2 3) '(3 2 1) '(3 3 3))", "(7 7 7)");
    assert_eval_text_dual!("(append '((1 2) 3) '(4 5) '(6))", "((1 2) 3 4 5 6)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_fib() {
    rt_start();
    assert_eval_node_dual!(
        r#"(define fib
           (lambda (n)
             (cond ((< n 2) 1)
                   ('t (+ (fib (- n 1)) (fib (- n 2)))))))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(fib 9)", RuntimeNode::Number(Number::Int(55)));
    assert_eval_node_dual!("(import list)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text_dual!("(map fib (iota 10))", "(1 1 2 3 5 8 13 21 34 55)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_set() {
    rt_start();
    assert_eval_node_dual!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("x", RuntimeNode::Number(Number::Int(1)));
    assert_eval_node_dual!("(set! x 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("x", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!(
        "((lambda (a) (set! x a)) 3)",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("x", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_set_car() {
    rt_start();
    assert_eval_node_dual!("(define x '(1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(set-car! x 4)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text_dual!("x", "(4 2 3)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_set_cdr() {
    rt_start();
    assert_eval_node_dual!("(define x '(1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(set-cdr! x '(4 5 6))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text_dual!("x", "(1 4 5 6)");
    assert_eval_node_dual!(
        "(define (g x) (set-cdr! x 1))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!("(set! x (cons 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_node_dual!("(g x)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval_text_dual!("x", "(2 . 1)");
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_begin() {
    rt_start();
    assert_eval_node_dual!("(begin 1 2 3)", RuntimeNode::Number(Number::Int(3)));
    assert_eval_node_dual!(
        "(begin (define x 1) (define y 2) x)",
        RuntimeNode::Number(Number::Int(1))
    );
    assert_eval_node_dual!("y", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_let() {
    rt_start();
    assert_eval_node_dual!(
        "(let ((x 1) (y 2)) (+ x y))",
        RuntimeNode::Number(Number::Int(3))
    );
    assert_eval_node_dual!(
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
    assert_eval_node_dual!("(if 1 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!("(if 0 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!("(if 't 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval_node_dual!("(if '() 2 3)", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.write().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_reverse_list() {
    rt_start();
    assert_eval_node_dual!(
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
    assert_eval_node_dual!(
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
    assert_eval_text_dual!("(reverse '(1 2 3 4))", "(4 3 2 1)");
    assert_eval_text_dual!("(reverse-sugar '(1 2 3 4))", "(4 3 2 1)");
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
    assert_eval_node_dual!(
        r#"
(define (last-pair x)
    (if (eq? (cdr x) '()) x (last-pair (cdr x))))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_node_dual!(
        r#"
(define (make-cycle x)
    (define y (last-pair x))
    (set-car! y x)
    x)"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text_dual!("(make-cycle (list 'a 'b 'c))", "(a b #0#)");
    assert_eval_node_dual!(
        r#"
(define (make-cycle2 x)
    (define y (last-pair x))
    (set-cdr! y x)
    x)"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval_text_dual!("(make-cycle2 (list 'a 'b 'c))", "(a b c . #0#)");
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
        "(() 2)
(t -2)
(5)
(-5)
(6)
(2 -13)
(-1 7)result: ()
"
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
        r#"> = ()
> = ()
> = (1 1 2 3 5 8 13 21 34 55)
> result: ()
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
= ()
= ()
= 42
CTRL-D
"#
    );
}
