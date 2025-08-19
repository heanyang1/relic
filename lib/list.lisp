;; List manipulation procedures. Most procedures should have the same behaviour
;; as described in R5RS or SRFI 1.
;;
;; Add `(import list)` to use this package.

(define (caar x) (car (car x)))
(define (cadr x) (car (cdr x)))
(define (cadar x) (car (cdr (car x))))
(define (caddr x) (car (cdr (cdr x))))

(define (length l)
    (if (eq? l '())
        0
        (+ (length (cdr l)) 1)))

(define (list-tail x k)
    (if (= k 0)
        x
        (list-tail (cdr x) (- k 1))))
(define (list-ref x k)
    (car (list-tail x k)))

;; `(iota n)` returns a list `'(0 1 2 ... n-1)`.
;; `(iota n start step)` returns `'(start start+step ... start+(n-1)*step)`.
(define (iota count . others)
    (define (iota-impl result count end step)
        (if (= count 0)
            result
            (iota-impl (cons end result) (- count 1) (- end step) step)))
  (if (eq? others '())
      (iota-impl '() count (- count 1) 1)
      (let ((start (car others))
            (step (cadr others)))
        (iota-impl '() count (+ start (* (- count 1) step)) step))))

(define (map f . lists)
    (define (smap f l)
        (if (eq? l '())
            '()
            (cons (f (car l)) (smap f (cdr l)))))
  (define (map-impl f lists)
    (if (eq? (car lists) '())
        '()
        (cons (apply f (smap car lists)) (map-impl f (smap cdr lists)))))
  (map-impl f lists))
