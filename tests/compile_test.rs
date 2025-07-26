use std::{collections::HashMap, path::Path};

use relic::{
    compile::{CodeGen, Compile},
    lexer::Lexer,
    node::Node,
    parser::Parse,
    preprocess::PreProcess,
};

macro_rules! compile {
    ($input:expr, $filename:expr, $output:expr) => {{
        let input = $input;
        let mut tokens = Lexer::new(input);
        let mut macros = HashMap::new();
        let mut codegen = CodeGen::new_main();

        println!("{input}");
        while let Ok(mut node) = Node::parse(&mut tokens) {
            let node = node.preprocess(&mut macros).unwrap();
            println!("{node}");
            node.compile(&mut codegen).unwrap();
        }
        // Create c_runtime/tests directory if it doesn't exist
        let test_dir = Path::new("c_runtime/tests");
        if !test_dir.exists() {
            std::fs::create_dir_all(test_dir).unwrap();
        }

        std::fs::write(
            test_dir.join(format!("{}.c", $filename)),
            codegen.to_string(),
        )
        .unwrap();
        std::fs::write(
            test_dir.join(format!("{}.out", $filename)),
            $output.to_string(),
        )
        .unwrap();
    }};
}

#[test]
fn test_cycle() {
    compile!(
        r#"
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
(display z2)"#,
        "cycle",
        "(a b #0#)\n(a b c . #0#)"
    );
}

#[test]
fn test_delay() {
    compile!(
        r#"
(define-syntax-rule (delay exp) (lambda () exp))
(define (force delayed-object) (delayed-object))
(display (force (delay 1)))"#,
        "delay",
        "1"
    );
}

#[test]
fn test_set_car() {
    compile!(
        r#"
(define x '(1 2 3))
(set-car! x 4)
(display x)"#,
        "set_car",
        "(4 2 3)"
    );
}

#[test]
fn test_set_cdr() {
    compile!(
        r#"
(define x '(1 2 3))
(set-cdr! x '(4 5 6))
(display x)"#,
        "set_cdr",
        "(1 4 5 6)"
    );
}

#[test]
fn test_fact() {
    compile!(
        r#"
(define fact
  (lambda (n acc)
    (cond ((< n 2) acc)
          ('t (fact (- n 1) (* n acc))))))

(display (fact 5 1))"#,
        "fact",
        "120"
    );
}

#[test]
fn test_or() {
    compile!(
        r#"
(display (or))
(newline)
(display (or '() 2 3))
(newline)
(display (or 1 2 3))"#,
        "or",
        "nil\n2\n1"
    );
}

#[test]
fn test_and() {
    compile!(
        r#"
(display (and))
(newline)
(display (and '() 2 3))
(newline)
(display (and 1 2 3))"#,
        "and",
        "t\nnil\n3"
    );
}

#[test]
fn test_cond() {
    compile!(
        r#"
(display (cond ((< 1 2) 1) ((> 1 2) 2)))
(newline)
(display (cond ((> 1 2) 1) ((< 1 2) 2)))
(newline)
(display (cond ((> 1 2) 1)))
(newline)
(display (cond ((> 1 2) 1) ((> 1 2) 2)))"#,
        "cond",
        "1\n2\nnil\nnil"
    );
}

#[test]
fn test_simple_expr() {
    compile!("(display (+ (* 1 2 3) (/ 3 4)))", "simple_expr", 6.75);
}

#[test]
fn test_simple_lambda() {
    compile!(
        r#"
    (display
        ((lambda (x y z) (- x
                            ((lambda (x) z)
                             y)))
         3 4 1))"#,
        "simple_lambda",
        2
    );
}

#[test]
fn test_lambda_pattern_matching() {
    compile!(
        r#"
(define f (lambda x (car x)))
(define (g . x) (car x))
(define h (lambda (x . y) (car y)))
(display (f 'a 'b 3 4))
(newline)
(display (g 2 3 4))
(newline)
(display (h 1 2 3 4))
(newline)
(display (h 1 2))
(newline)
(display (h 1 't))"#,
        "lambda_pattern",
        "a\n2\n2\n2\nt"
    );
}

#[test]
fn test_let() {
    compile!(
        r#"
(let ((x 1) (y 2)) (display (+ x y)))
(newline)
(let ((x 1) (y 2)) (define z (+ x y)) (display z))"#,
        "let",
        "3\n3"
    );
}

#[test]
fn test_set() {
    compile!(
        r#"
(define x 1)
(display x)
(newline)
(set! x 2)
(display x)
(newline)
((lambda (a) (set! x a)) 3)
(display x)
"#,
        "set",
        "1\n2\n3"
    );
}

#[test]
fn test_fib() {
    compile!(
        r#"
(define (fib x)
    (if (< x 2)
        x
        (+ (fib (- x 1))
           (fib (- x 2)))))
(define map
    (lambda (func l)
        (cond ((eq? l '()) '())
              ('t (cons (func (car l)) (map func (cdr l)))))))
(display (map fib '(0 1 2 3 4 5 6 7 8 9 10)))"#,
        "fib",
        "(0 1 1 2 3 5 8 13 21 34 55)"
    );
}

#[test]
fn test_reverse() {
    compile!(
        r#"
(define (aux lst acc)
    (if (eq? lst '())
        acc
        (aux (cdr lst)
             (cons (car lst) acc))))
(define (reverse lst) (aux lst '()))
(display (reverse '(1 2 3 4 5 6 7 8 9 10)))"#,
        "reverse",
        "(10 9 8 7 6 5 4 3 2 1)"
    );
}

#[test]
fn test_reverse_2() {
    compile!(
        r#"
(define (reverse x)
  (define (loop x y)
    (cond ((eq? x '()) y)
          ('t (define temp (cdr x))
              (set-cdr! x y)
              (loop temp x))))
  (loop x '()))
(display (reverse '(1 2 3 4 5 6 7 8 9 10)))"#,
        "reverse_2",
        "(10 9 8 7 6 5 4 3 2 1)"
    );
}

#[test]
fn test_sqrt() {
    compile!(
        r#"
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
(define (abs x)
    (if (> x 0) x (- 0 x)))
(define (sqrt x)
    (sqrt-iter 1.0 x))
(display (sqrt 2))"#,
        "sqrt",
        "1.4142156862745097"
    );
}
