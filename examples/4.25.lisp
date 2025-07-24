(define (unless condition usual-value exceptional-value)
  (if condition exceptional-value usual-value))
(define (factorial n)
  (breakpoint)
  ; Step through a few evaluations and you'll find that `(factorial (- n 1))`
  ; is evaluated when `n` is 1 because `unless` evaluates its oprands.
  (unless (= n 1)
    (* n (factorial (- n 1)))
    1))
(factorial 5)
