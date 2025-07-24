;; Exercise 3.27 defines a `memoize` function. This program prints the
;; environment graph before and after a memoized `fib` is called.
;;
;; To see the result, you'll need to interpret or debug the program.

(define (caar x) (car (car x)))

(define (lookup key table)
  (let ((record (assoc key (cdr table))))
    (cond (record (cdr record))
          ('t '()))))

(define (assoc key records)
  (cond ((eq? records '()) '())
        ((eq? key (caar records)) (car records))
        ('t (assoc key (cdr records)))))

(define (insert! key value table)
  (let ((record (assoc key (cdr table))))
    (cond (record
            (set-cdr! record value))
          ('t
            (define old-cdr (cdr table))
            (set-cdr! table
                  (cons (cons key value)
                        old-cdr))))))

(define (make-table)
  (list '*table*))


(define (fib n)
  (cond ((= n 0) 0)
        ((= n 1) 1)
        ('t (+ (fib (- n 1)) (fib (- n 2))))))

(define (memoize f)
  (let ((table (make-table)))
    (lambda (x)
      (let ((previously-computed-result
             (lookup x table)))
        (if previously-computed-result
            previously-computed-result
            (let ((result (f x)))
              (insert! x result table)
              result))))))

(define memo-fib
  (memoize
   (lambda (n)
     (cond ((= n 0) 0)
           ((= n 1) 1)
           ('t (+ (memo-fib (- n 1))
                  (memo-fib (- n 2))))))))

(graphviz)
(memo-fib 3)

(graphviz)
