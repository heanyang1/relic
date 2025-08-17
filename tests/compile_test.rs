use std::{collections::HashMap, ffi::CString, path::Path, process::Command};

use relic::{
    RT,
    compile::{self, CodeGen},
    lexer::{Lexer, Number},
    node::Node,
    parser::Parse,
    preprocess::PreProcess,
    rt_get, rt_import, rt_start,
    symbol::Symbol,
};
use serial_test::serial;

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

fn compile(input: &str, filename: &str, output: &str) {
    let mut tokens = Lexer::new(input);
    let mut macros = HashMap::new();
    let mut codegen = CodeGen::new_main();

    while let Ok(mut node) = Node::parse(&mut tokens) {
        let node = node.preprocess(&mut macros).unwrap();
        compile::compile(&node, &mut codegen, false).unwrap();
    }
    // Create c_runtime/tests directory if it doesn't exist
    let test_dir = Path::new("c_runtime/tests");
    if !test_dir.exists() {
        std::fs::create_dir_all(test_dir).unwrap();
    }

    std::fs::write(
        test_dir.join(format!("{filename}.c")),
        codegen.to_string(),
    )
    .unwrap();
    std::fs::write(
        test_dir.join(format!("{filename}.out")),
        output,
    )
    .unwrap();
}

#[test]
#[serial]
fn test_cycle() {
    let code = r#"
(define (last-pair x)
    (if (eq? (cdr x) '()) x (last-pair (cdr x))))
(define (make-cycle x)
    (define y (last-pair x))
    (set-car! y x)
    x)
(define z (make-cycle (list 'a 'b 'c)))
(display z)
(newline)

(define (make-cycle2 x)
    (define y (last-pair x))
    (set-cdr! y x)
    x)
(define z2 (make-cycle2 (list 'a 'b 'c)))
(display z2)"#;
    compile(code, "cycle", "(a b #0#)\n(a b c . #0#)");
    compile_and_load(code, "cycle");
    let z = get_value("z");
    let z2 = get_value("z2");

    let mut runtime = RT.write().unwrap();
    let val = runtime.display_node_idx(z);
    assert_eq!(val, "(a b #0#)");
    let val = runtime.display_node_idx(z2);
    assert_eq!(val, "(a b c . #0#)");
    runtime.clear();
    std::fs::remove_file("lib/cycle.relic").unwrap();
}

#[test]
#[serial]
fn test_delay() {
    let code = r#"
(define-syntax-rule (delay exp) (lambda () exp))
(define (force delayed-object) (delayed-object))
(define x (force (delay 1)))
(display x)"#;
    compile(code, "delay", "1");
    compile_and_load(code, "delay");
    let x = get_value("x");
    let mut runtime = RT.write().unwrap();
    let val = runtime.get_number(x).unwrap();
    assert_eq!(val, Number::Int(1));
    runtime.clear();
    std::fs::remove_file("lib/delay.relic").unwrap();
}

#[test]
#[serial]
fn test_set_car() {
    let code = r#"
(define x '(1 2 3))
(set-car! x 4)
(display x)"#;
    compile(code, "set_car", "(4 2 3)");
    compile_and_load(code, "set_car");
    let x = get_value("x");
    let mut runtime = RT.write().unwrap();
    let val = runtime.display_node_idx(x);
    assert_eq!(val, "(4 2 3)");
    runtime.clear();
    std::fs::remove_file("lib/set_car.relic").unwrap();
}

#[test]
#[serial]
fn test_set_cdr() {
    let code = r#"
(define x '(1 2 3))
(set-cdr! x '(4 5 6))
(display x)"#;
    compile(code, "set_cdr", "(1 4 5 6)");
    compile_and_load(code, "set_cdr");
    let x = get_value("x");
    let mut runtime = RT.write().unwrap();
    let val = runtime.display_node_idx(x);
    assert_eq!(val, "(1 4 5 6)");
    runtime.clear();
    std::fs::remove_file("lib/set_cdr.relic").unwrap();
}

#[test]
#[serial]
fn test_fact() {
    let code = r#"
(define fact
  (lambda (n acc)
    (cond ((< n 2) acc)
          ('t (fact (- n 1) (* n acc))))))
(define x (fact 5 1))
(display x)"#;
    compile(code, "fact", "120");
    compile_and_load(code, "fact");
    let x = get_value("x");
    let mut runtime = RT.write().unwrap();
    let val = runtime.get_number(x).unwrap();
    assert_eq!(val, Number::Int(120));
    runtime.clear();
    std::fs::remove_file("lib/fact.relic").unwrap();
}

#[test]
#[serial]
fn test_or() {
    let code = r#"
(define x1 (or))
(display x1)
(newline)
(define x2 (or '() 2 3))
(display x2)
(newline)
(define x3 (or 1 2 3))
(display x3)"#;
    compile(code, "or", "nil\n2\n1");
    compile_and_load(code, "or");
    let x1 = get_value("x1");
    let x2 = get_value("x2");
    let x3 = get_value("x3");
    let mut runtime = RT.write().unwrap();
    let val = runtime.get_symbol(x1).unwrap();
    assert_eq!(val, Symbol::Nil);
    let val = runtime.get_number(x2).unwrap();
    assert_eq!(val, Number::Int(2));
    let val = runtime.get_number(x3).unwrap();
    assert_eq!(val, Number::Int(1));
    runtime.clear();
    std::fs::remove_file("lib/or.relic").unwrap();
}

#[test]
#[serial]
fn test_and() {
    let code = r#"
(define x1 (and))
(display x1)
(newline)
(define x2 (and '() 2 3))
(display x2)
(newline)
(define x3 (and 1 2 3))
(display x3)"#;
    compile(code, "and", "t\nnil\n3");
    compile_and_load(code, "and");
    let x1 = get_value("x1");
    let x2 = get_value("x2");
    let x3 = get_value("x3");
    let mut runtime = RT.write().unwrap();
    let val = runtime.get_symbol(x1).unwrap();
    assert_eq!(val, Symbol::T);
    let val = runtime.get_symbol(x2).unwrap();
    assert_eq!(val, Symbol::Nil);
    let val = runtime.get_number(x3).unwrap();
    assert_eq!(val, Number::Int(3));
    runtime.clear();
    std::fs::remove_file("lib/and.relic").unwrap();
}

#[test]
#[serial]
fn test_cond() {
    let code = r#"
(define x1 (cond ((< 1 2) 1) ((> 1 2) 2)))
(display x1)
(newline)
(define x2 (cond ((> 1 2) 1) ((< 1 2) 2)))
(display x2)
(newline)
(define x3 (cond ((> 1 2) 1)))
(display x3)
(newline)
(define x4 (cond ((> 1 2) 1) ((> 1 2) 2)))
(display x4)"#;
    compile(code, "cond", "1\n2\nnil\nnil");
    compile_and_load(code, "cond");
    let x1 = get_value("x1");
    let x2 = get_value("x2");
    let x3 = get_value("x3");
    let x4 = get_value("x4");
    let mut runtime = RT.write().unwrap();
    let val = runtime.get_number(x1).unwrap();
    assert_eq!(val, Number::Int(1));
    let val = runtime.get_number(x2).unwrap();
    assert_eq!(val, Number::Int(2));
    let val = runtime.get_symbol(x3).unwrap();
    assert_eq!(val, Symbol::Nil);
    let val = runtime.get_symbol(x4).unwrap();
    assert_eq!(val, Symbol::Nil);
    runtime.clear();
    std::fs::remove_file("lib/cond.relic").unwrap();
}

#[test]
fn test_simple_expr() {
    compile("(display (+ (* 1 2 3) (/ 3 4)))", "simple_expr", "6.75");
}

#[test]
fn test_simple_lambda() {
    compile(
        r#"
    (display
        ((lambda (x y z) (- x
                            ((lambda (x) z)
                             y)))
         3 4 1))"#,
        "simple_lambda",
        "2",
    );
}

#[test]
#[serial]
fn test_lambda_pattern_matching() {
    let code = r#"
(define f (lambda x (car x)))
(define (g . x) (car x))
(define h (lambda (x . y) (car y)))
(define x1 (f 'a 'b 3 4))
(display x1)
(newline)
(define x2 (g 2 3 4))
(display x2)
(newline)
(define x3 (h 1 2 3 4))
(display x3)
(newline)
(define x4 (h 1 2))
(display x4)
(newline)
(define x5 (h 1 't))
(display x5)
(newline)"#;
    compile(code, "lambda_pattern", "a\n2\n2\n2\nt\n");
    compile_and_load(code, "lambda_pattern");
    let x1 = get_value("x1");
    let x2 = get_value("x2");
    let x3 = get_value("x3");
    let x4 = get_value("x4");
    let x5 = get_value("x5");
    let mut runtime = RT.write().unwrap();
    let val = runtime.get_symbol(x1).unwrap();
    assert_eq!(val, Symbol::User("a".to_string()));
    let val = runtime.get_number(x2).unwrap();
    assert_eq!(val, Number::Int(2));
    let val = runtime.get_number(x3).unwrap();
    assert_eq!(val, Number::Int(2));
    let val = runtime.get_number(x4).unwrap();
    assert_eq!(val, Number::Int(2));
    let val = runtime.get_symbol(x5).unwrap();
    assert_eq!(val, Symbol::T);
    runtime.clear();
    std::fs::remove_file("lib/lambda_pattern.relic").unwrap();
}

#[test]
#[serial]
fn test_let() {
    compile(
        r#"
(let ((x 1) (y 2)) (display (+ x y)))
(newline)
(let ((x 1) (y 2)) (define z (+ x y)) (display z))"#,
        "let",
        "3\n3",
    );
}

#[test]
#[serial]
fn test_set() {
    let code = r#"
(define x 1)
(define x1 x)
(display x1)
(newline)
(set! x 2)
(define x2 x)
(display x2)
(newline)
((lambda (a) (set! x a)) 3)
(display x)
(newline)
"#;
    compile(code, "set", "1\n2\n3");
    compile_and_load(code, "set");
    let x = get_value("x");
    let x1 = get_value("x1");
    let x2 = get_value("x2");
    let mut runtime = RT.write().unwrap();
    let val = runtime.get_number(x1).unwrap();
    assert_eq!(val, Number::Int(1));
    let val = runtime.get_number(x2).unwrap();
    assert_eq!(val, Number::Int(2));
    let val = runtime.get_number(x).unwrap();
    assert_eq!(val, Number::Int(3));
    runtime.clear();
    std::fs::remove_file("lib/set.relic").unwrap();
}

#[test]
#[serial]
fn test_fib() {
    let code = r#"
(define (fib x)
    (if (< x 2)
        x
        (+ (fib (- x 1))
           (fib (- x 2)))))
(define map
    (lambda (func l)
        (cond ((eq? l '()) '())
              ('t (cons (func (car l)) (map func (cdr l)))))))
(define z (map fib '(0 1 2 3 4 5 6 7 8 9 10)))
(display z)
(newline)"#;
    compile(code, "fib", "(0 1 1 2 3 5 8 13 21 34 55)");
    compile_and_load(code, "fib");
    let z = get_value("z");
    let mut runtime = RT.write().unwrap();
    let val = runtime.display_node_idx(z);
    assert_eq!(val, "(0 1 1 2 3 5 8 13 21 34 55)");
    runtime.clear();
    std::fs::remove_file("lib/fib.relic").unwrap();
}

#[test]
#[serial]
fn test_reverse() {
    let code = r#"
(define (aux lst acc)
    (if (eq? lst '())
        acc
        (aux (cdr lst)
             (cons (car lst) acc))))
(define (reverse lst) (aux lst '()))
(define z (reverse '(1 2 3 4 5 6 7 8 9 10)))
(display z)
(newline)"#;
    compile(code, "reverse", "(10 9 8 7 6 5 4 3 2 1)");
    compile_and_load(code, "reverse");
    let z = get_value("z");
    let mut runtime = RT.write().unwrap();
    let val = runtime.display_node_idx(z);
    assert_eq!(val, "(10 9 8 7 6 5 4 3 2 1)");
    runtime.clear();
    std::fs::remove_file("lib/reverse.relic").unwrap();
}

#[test]
#[serial]
fn test_reverse_2() {
    let code = r#"
(define (reverse x)
  (define (loop x y)
    (cond ((eq? x '()) y)
          ('t (define temp (cdr x))
              (set-cdr! x y)
              (loop temp x))))
  (loop x '()))
(define z (reverse '(1 2 3 4 5 6 7 8 9 10)))
(display z)
(newline)"#;
    compile(code, "reverse_2", "(10 9 8 7 6 5 4 3 2 1)");
    compile_and_load(code, "reverse_2");
    let z = get_value("z");
    let mut runtime = RT.write().unwrap();
    let val = runtime.display_node_idx(z);
    assert_eq!(val, "(10 9 8 7 6 5 4 3 2 1)");
    runtime.clear();
    std::fs::remove_file("lib/reverse_2.relic").unwrap();
}

#[test]
#[serial]
fn test_sqrt() {
    let code = r#"
(define (sqrt-iter guess x)
    (if (good-enough? guess x)
        guess
        (sqrt-iter (improve guess x) x)))
(define (improve guess x)
    (average guess (/ x guess)))
(define (average x y)
    (/ (+ x y) 2))
(define (good-enough? guess x)
    (< (abs (- (* guess guess) x)) 0.001))
(define (sqrt x)
    (sqrt-iter 1.0 x))
(define z (sqrt 2))
(display z)
(newline)"#;
    compile(code, "sqrt", "1.4142156862745097");
    compile_and_load(code, "sqrt");
    let z = get_value("z");
    let mut runtime = RT.write().unwrap();
    let val = runtime.get_number(z).unwrap();
    assert_eq!(val, Number::Float(1.4142156862745097));
    runtime.clear();
    std::fs::remove_file("lib/sqrt.relic").unwrap();
}
