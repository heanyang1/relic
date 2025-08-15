use std::{
    io::Write,
    process::{Command, Stdio},
};

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
