;; An infinite loop that won't crash (if you turn off `-g` flag of Relic and
;; compile the C code with a high optimization level like `-O3`).
;; This demonstrates that Relic can be tail recursive.

(define (loop x)
  (display x)
  (newline)
  (loop (+ x 1)))

(loop 0)
