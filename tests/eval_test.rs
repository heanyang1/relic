use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use relic::env::Env;
use relic::eval::{ConsoleEval, Eval};
use relic::lexer::{Lexer, Number};
use relic::node::{self, Node, NodeEnv};
use relic::parser::Parse;
use relic::preprocess::PreProcess;
use relic::runtime::RuntimeNode;
use relic::symbol::Symbol;
use relic::{RT, nil, rt_pop, rt_start, vec_to_list};
use serial_test::serial;

macro_rules! assert_eval {
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

#[test]
#[serial]
fn test_int() {
    rt_start();
    assert_eval!("42", RuntimeNode::Number(Number::Int(42)));

    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}
#[test]
#[serial]
fn test_simple_arithmetic() {
    rt_start();
    assert_eval!("(+ 1 2 3 4)", RuntimeNode::Number(Number::Int(10)));
    assert_eval!("(- 3 2 1)", RuntimeNode::Number(Number::Int(0)));
    assert_eval!("(* 2 3)", RuntimeNode::Number(Number::Int(6)));
    assert_eval!("(/ 6 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval!("(/ 5 2)", RuntimeNode::Number(Number::Float(2.5)));
    assert_eval!("(+ 1.0 2.0 3)", RuntimeNode::Number(Number::Float(6.0)));
    assert_eval!("(- 3.0 2.0)", RuntimeNode::Number(Number::Float(1.0)));
    assert_eval!("(* 2.0 3.0)", RuntimeNode::Number(Number::Float(6.0)));
    assert_eval!("(/ 6.0 3.0)", RuntimeNode::Number(Number::Float(2.0)));
    assert_eval!("(+ 1 2.0)", RuntimeNode::Number(Number::Float(3.0)));
    assert_eval!("(- 3.0 2)", RuntimeNode::Number(Number::Float(1.0)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_with_symbol() {
    rt_start();
    assert_eval!("(define x 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(- x 2)", RuntimeNode::Number(Number::Int(0)));
    assert_eval!("(* 3 x)", RuntimeNode::Number(Number::Int(6)));
    assert_eval!("(/ 6 x)", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_nested_arithmetic() {
    rt_start();
    assert_eval!("(+ (- 1 2) 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval!(
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
    assert_eval!("t", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(< 1.0 2)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(< 2 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(> 1 2.0)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(> (+ 1 1) 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(<= 1 1.0)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(<= 1.0 2)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(<= 2 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(>= 1 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(>= 1 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(>= 2 1)", RuntimeNode::Symbol(Symbol::T));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_toplevel_symbol() {
    rt_start();
    assert_eval!("+", RuntimeNode::Symbol(Symbol::Add));
    assert_eval!("-", RuntimeNode::Symbol(Symbol::Sub));
    assert_eval!("*", RuntimeNode::Symbol(Symbol::Mul));
    assert_eval!("/", RuntimeNode::Symbol(Symbol::Div));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

// #[test]
// #[serial]
// fn error_cases() {
//     assert_error!("(+ 1)", "Fewer parameters than requested");
//     assert_error!("(> 1)", "Fewer parameters than requested");
//     assert_error!("(= 1 2 3)", "More parameters than requested");
//     assert_error!("(1 2 3)", "1 can not be the head of a list");
//     assert_error!("(+ + 1)", "+ is not a number");

//     let env = Rc::new(RefCell::new(RuntimeNodeEnv::new(
//         None,
//         HashMap::from([("x".to_string(), RuntimeNode::Number(Number::Int(2)).into())]),
//         "env",
//     )));
//     assert_error!("(+ x y)", env, "Symbol y not found");
// }

// #[test]
// #[serial]
// fn test_list_simple() {
//     assert_eval!(
//         "'(1 2 3)",
//         vec_to_list!(
//             RuntimeNode::Number(Number::Int(1)).into(),
//             RuntimeNode::Number(Number::Int(2)).into(),
//             RuntimeNode::Number(Number::Int(3)).into()
//         ),
//     );
//     assert_eval!(
//         "(list 1 2 3)",
//         vec_to_list!(
//             RuntimeNode::Number(Number::Int(1)).into(),
//             RuntimeNode::Number(Number::Int(2)).into(),
//             RuntimeNode::Number(Number::Int(3)).into()
//         ),
//     );
//     assert_eval!(
//         "(list 1 2)",
//         vec_to_list!(
//             RuntimeNode::Number(Number::Int(1)).into(),
//             RuntimeNode::Number(Number::Int(2)).into()
//         ),
//     );
//     assert_eval!(
//         "(list 1)",
//         RuntimeNode::Pair(RuntimeNode::Number(Number::Int(1)).into(), RuntimeNode::Symbol(Symbol::Nil).into()),
//     );
//     assert_eval!("(list)", RuntimeNode::Symbol(Symbol::Nil));
// }

// #[test]
// #[serial]
// fn test_list_nested() {
//     assert_eval!(
//         "(list (list 1 2 3) (list 4 5 6))",
//         vec_to_list!(
//             vec_to_list!(
//                 RuntimeNode::Number(Number::Int(1)).into(),
//                 RuntimeNode::Number(Number::Int(2)).into(),
//                 RuntimeNode::Number(Number::Int(3)).into()
//             )
//             .into(),
//             vec_to_list!(
//                 RuntimeNode::Number(Number::Int(4)).into(),
//                 RuntimeNode::Number(Number::Int(5)).into(),
//                 RuntimeNode::Number(Number::Int(6)).into()
//             )
//             .into()
//         ),
//     );
//     assert_eval!(
//         "(list 1 '(2 3) 4)",
//         vec_to_list!(
//             RuntimeNode::Number(Number::Int(1)).into(),
//             vec_to_list!(
//                 RuntimeNode::Number(Number::Int(2)).into(),
//                 RuntimeNode::Number(Number::Int(3)).into()
//             )
//             .into(),
//             RuntimeNode::Number(Number::Int(4)).into()
//         ),
//     );
// }

// #[test]
// #[serial]
// fn test_list_manipulation() {
//     assert_eval!("(car (list 1 2 3))", RuntimeNode::Number(Number::Int(1)));
//     assert_eval!(
//         "(cdr '(1 2 3))",
//         vec_to_list!(
//             RuntimeNode::Number(Number::Int(2)).into(),
//             RuntimeNode::Number(Number::Int(3)).into()
//         ),
//     );
//     assert_eval!(
//         "(cons 1 (list 2 3))",
//         vec_to_list!(
//             RuntimeNode::Number(Number::Int(1)).into(),
//             RuntimeNode::Number(Number::Int(2)).into(),
//             RuntimeNode::Number(Number::Int(3)).into()
//         ),
//     );
// }

#[test]
#[serial]
fn test_define() {
    rt_start();
    assert_eval!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(define y (+ x 1))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("x", RuntimeNode::Number(Number::Int(1)));
    assert_eval!("y", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_define_syntax_rule() {
    rt_start();
    let mut macros = HashMap::new();
    assert_eval!(
        "(define-syntax-rule (macro1 x) (display 1) (+ x 1))",
        RuntimeNode::Symbol(Symbol::Nil),
        macros
    );
    assert_eval!(
        "(define-syntax-rule (macro2 x) (car x))",
        RuntimeNode::Symbol(Symbol::Nil),
        macros
    );
    assert_eval!("(macro1 2)", RuntimeNode::Number(Number::Int(3)), macros);
    assert_eval!(
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
    assert_eval!(
        "((lambda (x) (+ x 1)) 2)",
        RuntimeNode::Number(Number::Int(3))
    );
    assert_eval!(
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
    assert_eval!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("x", RuntimeNode::Number(Number::Int(1)));
    assert_eval!(
        "(define func (lambda (x) (+ x 1)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!("(func 2)", RuntimeNode::Number(Number::Int(3)));
    assert_eval!("(func x)", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_lambda_scope() {
    rt_start();
    assert_eval!("(define (f) 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!(
        "(define (g) (define (f x) x) (f 2))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!("(g)", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_lambda_pattern_matching() {
    rt_start();
    assert_eval!(
        "(define f (lambda x (car x)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!(
        "(f 'a 'b 3 4)",
        RuntimeNode::Symbol(Symbol::User("a".to_string()))
    );
    assert_eval!("(define (g . x) (car x))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(g 2 3 4)", RuntimeNode::Number(Number::Int(2)));
    assert_eval!(
        "(define h (lambda (x . y) (car y)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!("(h 1 2 3 4)", RuntimeNode::Number(Number::Int(2)));
    assert_eval!("(h 1 2)", RuntimeNode::Number(Number::Int(2)));
    assert_eval!("(h 1 't)", RuntimeNode::Symbol(Symbol::T));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_function_call() {
    rt_start();
    assert_eval!(
        "(define g (lambda (x) (+ x 1)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!(
        "(define h (lambda (x) (g (+ x 1))))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!("(h 2)", RuntimeNode::Number(Number::Int(4)));
    assert_eval!(
        "(define a (lambda (x) (car x)))",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!("(a '(1 2 3))", RuntimeNode::Number(Number::Int(1)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_atom() {
    rt_start();
    assert_eval!("(atom? 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(atom? (+ 1 2))", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(atom? (list 1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(atom? 'a)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(atom? '())", RuntimeNode::Symbol(Symbol::T));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_eq() {
    rt_start();
    assert_eval!("(eq? 1 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(eq? (- 2 1) 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(eq? 1 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(eq? 'a 'a)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(eq? 'a 'b)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(eq? '() '())", RuntimeNode::Symbol(Symbol::T));
    assert_eval!(
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
    assert_eval!("(number? 1)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(number? (+ 1 2))", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(number? 'a)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(number? '())", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(number? 'a)", RuntimeNode::Symbol(Symbol::Nil));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_cond() {
    rt_start();
    assert_eval!(
        "(cond ((< 1 2) 1) ((> 1 2) 2))",
        RuntimeNode::Number(Number::Int(1))
    );
    assert_eval!(
        "(cond ((> 1 2) 1) ((< 1 2) 2))",
        RuntimeNode::Number(Number::Int(2))
    );
    assert_eval!("(cond ((> 1 2) 1))", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!(
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
    assert_eval!("(and)", RuntimeNode::Symbol(Symbol::T));
    assert_eval!("(and '() 2 3)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(and 1 2 3)", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_or() {
    rt_start();
    assert_eval!("(or)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("(or '() 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval!("(or 1 2 3)", RuntimeNode::Number(Number::Int(1)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_fact() {
    rt_start();
    assert_eval!(
        r#"
(define fact
  (lambda (n acc)
    (cond ((< n 2) acc)
          ('t (fact (- n 1) (* n acc))))))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!("(fact 5 1)", RuntimeNode::Number(Number::Int(120)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_fib() {
    rt_start();
    assert_eval!(
        r#"(define fib
           (lambda (n)
             (cond ((< n 2) 1)
                   ('t (+ (fib (- n 1)) (fib (- n 2)))))))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!(
        r#"(define map
            (lambda (func l)
            (cond ((eq? l '()) '())
                  ('t (cons (func (car l)) (map func (cdr l)))))))"#,
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!("(fib 9)", RuntimeNode::Number(Number::Int(55)));
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
    assert_eval!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("x", RuntimeNode::Number(Number::Int(1)));
    assert_eval!("(set! x 2)", RuntimeNode::Symbol(Symbol::Nil));
    assert_eval!("x", RuntimeNode::Number(Number::Int(2)));
    assert_eval!(
        "((lambda (a) (set! x a)) 3)",
        RuntimeNode::Symbol(Symbol::Nil)
    );
    assert_eval!("x", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

// #[test]
// #[serial]
// fn test_set_car() {
//     rt_start();
//     assert_eval!("(define x '(1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
//     assert_eval!("(set-car! x 4)", RuntimeNode::Symbol(Symbol::Nil));
//     assert_eval!(
//         "x",
//         RuntimeNode::Pair(
//             RuntimeNode::Number(Number::Int(4)).into(),
//             RuntimeNode::Pair(
//                 RuntimeNode::Number(Number::Int(2)).into(),
//                 RuntimeNode::Pair(
//                     RuntimeNode::Number(Number::Int(3)).into(),
//                     RuntimeNode::Symbol(Symbol::Nil).into(),
//                 )
//                 .into(),
//             )
//             .into(),
//         )
//     );
//     let mut runtime = RT.lock().unwrap();
//     runtime.clear();
// }

// #[test]
// #[serial]
// fn test_set_cdr() {
//     rt_start();
//     assert_eval!("(define x '(1 2 3))", RuntimeNode::Symbol(Symbol::Nil));
//     assert_eval!("(set-cdr! x '(4 5 6))", RuntimeNode::Symbol(Symbol::Nil));
//     assert_eval!(
//         "x",
//         vec_to_list!(
//             RuntimeNode::Number(Number::Int(1)).into(),
//             RuntimeNode::Number(Number::Int(4)).into(),
//             RuntimeNode::Number(Number::Int(5)).into(),
//             RuntimeNode::Number(Number::Int(6)).into()
//         )
//     );
//     assert_eval!(
//         "(define (g x) (set-cdr! x 1))",
//         RuntimeNode::Symbol(Symbol::Nil)
//     );
//     assert_eval!("(set! x (cons 2 3))", RuntimeNode::Symbol(Symbol::Nil));
//     assert_eval!("(g x)", RuntimeNode::Symbol(Symbol::Nil));
//     assert_eval!(
//         "x",
//         RuntimeNode::Pair(
//             RuntimeNode::Number(Number::Int(2)).into(),
//             RuntimeNode::Number(Number::Int(1)).into(),
//         )
//     );
//     let mut runtime = RT.lock().unwrap();
//     runtime.clear();
// }

#[test]
#[serial]
fn test_begin() {
    rt_start();
    assert_eval!("(begin 1 2 3)", RuntimeNode::Number(Number::Int(3)));
    assert_eval!(
        "(begin (define x 1) (define y 2) x)",
        RuntimeNode::Number(Number::Int(1))
    );
    assert_eval!("y", RuntimeNode::Number(Number::Int(2)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

#[test]
#[serial]
fn test_let() {
    rt_start();
    assert_eval!(
        "(let ((x 1) (y 2)) (+ x y))",
        RuntimeNode::Number(Number::Int(3))
    );
    assert_eval!(
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
    assert_eval!("(if 1 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval!("(if 0 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval!("(if 't 2 3)", RuntimeNode::Number(Number::Int(2)));
    assert_eval!("(if '() 2 3)", RuntimeNode::Number(Number::Int(3)));
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

// #[test]
// #[serial]
// fn test_reverse_list() {
//     rt_start();
//     assert_eval!(
//         r#"
// (define reverse
//   (lambda (x)
//     (begin
//       (define loop
//         (lambda (x y)
//           (cond ((eq? x '()) y)
//                 ('t (begin
//                     (define temp (cdr x))
//                     (set-cdr! x y)
//                     (display x)
//                     (display temp)
//                     (loop temp x))))))
//       (loop x '()))))"#,
//         RuntimeNode::Symbol(Symbol::Nil)
//     );
//     assert_eval!(
//         r#"
// (define (reverse-sugar x)
//   (define (loop x y)
//     (cond ((eq? x '()) y)
//           ('t (define temp (cdr x))
//               (set-cdr! x y)
//               (loop temp x))))
//   (loop x '()))"#,
//         RuntimeNode::Symbol(Symbol::Nil)
//     );
//     assert_eval!(
//         "(reverse '(1 2 3 4))",
//         vec_to_list!(
//             RuntimeNode::Number(Number::Int(4)).into(),
//             RuntimeNode::Number(Number::Int(3)).into(),
//             RuntimeNode::Number(Number::Int(2)).into(),
//             RuntimeNode::Number(Number::Int(1)).into()
//         )
//     );
//     assert_eval!(
//         "(reverse-sugar '(1 2 3 4))",
//         vec_to_list!(
//             RuntimeNode::Number(Number::Int(4)).into(),
//             RuntimeNode::Number(Number::Int(3)).into(),
//             RuntimeNode::Number(Number::Int(2)).into(),
//             RuntimeNode::Number(Number::Int(1)).into()
//         )
//     );
//     let mut runtime = RT.lock().unwrap();
//     runtime.clear();
// }

#[test]
#[serial]
fn test_delay() {
    rt_start();
    let mut macros = HashMap::new();
    assert_eval!(
        "(define-syntax-rule (delay exp) (lambda () exp))",
        RuntimeNode::Symbol(Symbol::Nil),
        macros
    );
    assert_eval!(
        "(define (force delayed-object) (delayed-object))",
        RuntimeNode::Symbol(Symbol::Nil),
        macros
    );
    assert_eval!(
        "(force (delay 1))",
        RuntimeNode::Number(Number::Int(1)),
        macros
    );
    let mut runtime = RT.lock().unwrap();
    runtime.clear();
}

// #[test]
// #[serial]
// fn test_display() {
//     rt_start();
//     let mut result = ConsoleEval::new();
//     let mut macros = HashMap::new();
//     result = assert_eval!_print!("(display 1)", result.clone(), macros);
//     result = assert_eval!_print!("(newline)", result.clone(), macros);
//     result = assert_eval!_print!("(display '())", result.clone(), macros);
//     result = assert_eval!_print!("(newline)", result.clone(), macros);
//     result = assert_eval!_print!("(display '(1 2 3))", result.clone(), macros);
//     result = assert_eval!_print!("(display '\"4 5\n6\")", result.clone(), macros);
//     assert_eq!(
//         result.display_output,
//         Some("1\nnil\n(1 2 3)4 5\n6".to_string())
//     );
//     assert_eq!(result.graphviz_output, None);
//     let mut runtime = RT.lock().unwrap();
//     runtime.clear();
// }

// #[test]
// #[serial]
// fn test_cycle() {
//     rt_start();
//     assert_eval!_print!(
//         r#"
// (define (last-pair x)
//     (if (eq? (cdr x) '()) x (last-pair (cdr x))))"#,
//         env.clone()
//     );
//     assert_eval!_print!(
//         r#"
// (define (make-cycle x)
//     (define y (last-pair x))
//     (set-car! y x)
//     x)"#,
//         env.clone()
//     );
//     assert_eval!_print!("(define z (make-cycle (list 'a 'b 'c)))", env.clone());
//     assert_eval!_print!("(display z)", env.clone());
//     assert_eval!_print!(
//         r#"
// (define (make-cycle2 x)
//     (define y (last-pair x))
//     (set-cdr! y x)
//     x)"#,
//         env.clone()
//     );
//     assert_eval!_print!("(define z2 (make-cycle2 (list 'a 'b 'c)))", env.clone());
//     assert_eval!_print!("(display z2)", env.clone());
//     let mut runtime = RT.lock().unwrap();
//     runtime.clear();
// }

// #[test]
// #[serial]
// fn test_env_graphviz_simple() {
//     rt_start();
//     assert_eval!_print!("(define x 1)", ConsoleEval::new(), HashMap::new());
//     assert_eval!_print!("(define y 2)", ConsoleEval::new(), HashMap::new());
//     println!("\nSimple Environment Graph:");
//     let result = assert_eval!_print!("(graphviz)", ConsoleEval::new(), HashMap::new());
//     assert!(result.display_output.is_none());
//     println!("{}", result.graphviz_output.unwrap());
//     let mut runtime = RT.lock().unwrap();
//     runtime.clear();
// }

// #[test]
// #[serial]
// fn test_env_graphviz_nested() {
//     rt_start();
//     let outer = Rc::new(RefCell::new(RuntimeNodeEnv::top(&mut ())));
//     outer.borrow_mut().define(
//         &"x".to_string(),
//         Rc::new(RefCell::new(RuntimeNode::Number(Number::Int(42)))),
//         &mut (),
//     );

//     let inner = Rc::new(RefCell::new(RuntimeNodeEnv::new(
//         Some(outer.clone()),
//         HashMap::new(),
//         "env",
//     )));
//     inner.borrow_mut().define(
//         &"y".to_string(),
//         Rc::new(RefCell::new(RuntimeNode::Number(Number::Int(24)))),
//         &mut (),
//     );

//     println!("\nNested Environment Graph:");
//     let result = assert_eval!_print!("(graphviz)", inner.clone(), ConsoleEval::new(),);
//     assert!(result.display_output.is_none());
//     println!("{}", result.graphviz_output.unwrap());
//     let mut runtime = RT.lock().unwrap();
//     runtime.clear();
// }

// #[test]
// #[serial]
// fn test_env_graphviz_lambda() {
//     rt_start();
//     assert_eval!("(define x 1)", RuntimeNode::Symbol(Symbol::Nil));
//     assert_eval!("(define (f x) (+ x 1))", RuntimeNode::Symbol(Symbol::Nil));
//     println!("\nLambda Environment Graph:");
//     let result = assert_eval!_print!("(graphviz)", ConsoleEval::new(),);
//     assert!(result.display_output.is_none());
//     println!("{}", result.graphviz_output.unwrap());
//     let mut runtime = RT.lock().unwrap();
//     runtime.clear();
// }
