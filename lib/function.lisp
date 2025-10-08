;; Procedures that may be useful in functional programming. Most procedures
;; are also defined in Racket and they should have the same behaviour.
;;
;; Add `(import function)` to use this package.

(define (compose . functions)
    (if (eq? functions '())
        (lambda (x) x)
        (lambda (x) ((car functions) ((apply compose (cdr functions)) x)))))
