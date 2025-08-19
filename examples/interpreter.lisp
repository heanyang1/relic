;; A Lisp interpreter written in Lisp.
;; Most parts of the interpreter are from SICP lecture 7a,
;; others are from [MaxXing's lisp.lisp](https://github.com/pku-minic/awesome-sysy/blob/master/lisp/lisp.lisp)
;; Relic does not have EOF so far. type `nil` to exit the interpreter. It won't
;; exit when typing `'()` or anything that is not `nil` but evals to `nil`.

(import list)

(define (eval exp env)
  (cond ((number? exp) exp)
        ((atom? exp) (lookup exp env))
        ((eq? (car exp) 'quote) (cadr exp))
        ((eq? (car exp) 'lambda) (list (cdr exp) env))
        ((eq? (car exp) 'cond) (evcond (cdr exp) env))
        ((eq? (car exp) 'car) (car (eval (cadr exp) env)))
        ((eq? (car exp) 'cdr) (cdr (eval (cadr exp) env)))
        ((eq? (car exp) 'cons)
         (cons (eval (cadr exp) env) (eval (caddr exp) env)))
        ((eq? (car exp) '+)
         (+ (eval (cadr exp) env) (eval (caddr exp) env)))
        ((eq? (car exp) '-)
         (- (eval (cadr exp) env) (eval (caddr exp) env)))
        ((eq? (car exp) '*)
         (* (eval (cadr exp) env) (eval (caddr exp) env)))
        ((eq? (car exp) '/)
         (/ (eval (cadr exp) env) (eval (caddr exp) env)))
        ((eq? (car exp) 'eq?)
         (eq? (eval (cadr exp) env) (eval (caddr exp) env)))
        ((eq? (car exp) '=)
         (= (eval (cadr exp) env) (eval (caddr exp) env)))
        ((eq? (car exp) 'define)
         (set-car! env (cons (cons (cadr exp) (eval (caddr exp) env)) (car env))))
        ('t
         (my/apply (eval (car exp) env)
                (evlist (cdr exp) env)))))

(define (my/apply proc args)
  (eval (cadar proc)
        (bind (caar proc) args (cadr proc))))

(define (evlist l env)
    (if (eq? l '())
        '()
        (cons (eval (car l) env)
              (evlist (cdr l) env))))

(define (evcond clauses env)
    (cond ((eq? clauses '()) '())
          ((eval (caar clauses) env) (eval (cadar clauses) env))
          ('t (evcond (cdr clauses) env))))

(define (bind vars vals env)
  (cons (pair-up vars vals) env))

(define (pair-up vars vals)
    (if (eq? vars '())
        '()
        (cons (cons (car vars) (car vals))
              (pair-up (cdr vars) (cdr vals)))))

(define (lookup sym env)
    (if (eq? env '())
        (display "ERROR: unbound environment\n")
        (let ((vcell (assq sym (car env))))
          (if (eq? vcell '())
              (lookup sym (cdr env))
              (cdr vcell)))))

(define (assq sym alist)
  (cond ((eq? alist '()) '())
        ((eq? sym (caar alist)) (car alist))
        ('t (assq sym (cdr alist)))))

(define cur_env '(((t . t))))

(define (loop)
  (display "> ")
  (let ((value (read)))
    (if (eq? value '())
        '()
        (begin
          (display "= ")
          (display (eval value cur_env))
          (newline)
          (loop)))))

(loop)
