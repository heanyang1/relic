;; List manipulation procedures. Most procedures should have the same behaviour
;; as described in R5RS or SRFI 1.
;;
;; Add `(import list)` to use this package.

(define (caar x) (car (car x)))
(define (cadr x) (car (cdr x)))
(define (cadar x) (car (cdr (car x))))
(define (caddr x) (car (cdr (cdr x))))
(define (cadddr x) (car (cdr (cdr (cdr x)))))

(define (null? x) (eq? x '()))

(define (length l)
    (define (length-impl cnt l)
        (if (null? l)
            cnt
            (length-impl (+ 1 cnt) (cdr l))))
  (length-impl 0 l))

(define (list-tail x k)
    (if (= k 0)
        x
        (list-tail (cdr x) (- k 1))))
(define (list-ref x k)
    (car (list-tail x k)))

(define (take x k)
    (define (take-impl acc cur k)
        (if (= k 0)
            acc
            (take-impl (cons (car cur) acc) (cdr cur) (- k 1))))
  (reverse (take-impl '() x k)))
(define (drop x k) (list-tail x k))

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
        (if (null? l)
            '()
            (cons (f (car l)) (smap f (cdr l)))))
  (define (map-impl f lists)
      (if (null? (car lists))
          '()
          (cons (apply f (smap car lists)) (map-impl f (smap cdr lists)))))
  (map-impl f lists))

(define (reverse l)
    (define (reverse-impl acc cur)
        (if (null? cur)
            acc
            (reverse-impl (cons (car cur) acc) (cdr cur))))
  (reverse-impl '() l))


(define (append . lists)
    (define (append-2 l1 l2)
        (define (append-2-impl l1 l2)
            (if (null? l1)
                l2
                (append-2-impl (cdr l1) (cons (car l1) l2))))
      (append-2-impl (reverse l1) l2))
  (if (null? lists)
      '()
      (append-2 (car lists) (apply append (cdr lists)))))
