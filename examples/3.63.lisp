;; In exercise 3.63, Louis Reasoner proposed a simpler but slower version of
;; `sqrt-stream`. This program compares the difference in the environment
;; between the two streams.
;;
;; To see the result, you'll need to interpret or debug the program.

(define (memo-proc proc)
  (let ((already-run? '()) (result '()))
    (lambda ()
      (if (eq? already-run? '())
          (begin (set! result (proc))
                 (set! already-run? 't)
                 result)
          result))))

(define-syntax-rule (delay exp) (memo-proc (lambda () exp)))
(define (force delayed-object) (delayed-object))

(define the-empty-stream '())
(define (stream-null? s) (eq? s '()))
(define stream-car car)
(define (stream-cdr s) (force (cdr s)))
(define-syntax-rule (cons-stream x y)
  (cons x (delay y)))

(define (stream-ref s n)
  (if (= n 0)
      (stream-car s)
      (stream-ref (stream-cdr s) (- n 1))))

(define (stream-map proc s)
  (if (stream-null? s)
      the-empty-stream
      (cons-stream (proc (stream-car s))
                   (stream-map proc (stream-cdr s)))))

(define (average x y)
  (/ (+ x y) 2))

(define (sqrt-improve guess x)
  (average guess (/ x guess)))

(define (sqrt-stream x)
    (define guesses
	(cons-stream
	 1.0
	 (stream-map (lambda (guess) (sqrt-improve guess x))
                     guesses))) ; `stream-map` finds `guess` and maps it
  guesses)

(define (sqrt-stream-slow x)
    (cons-stream 1.0 (stream-map
		      (lambda (guess)
			(sqrt-improve guess x))
		      (sqrt-stream-slow x)))) ; `stream-map` creates a new stream and maps it

; Uncomment one of the two to see the environment graph
; Warning: the graph is very big. Don't uncomment both of them.
; (define x (sqrt-stream 2)) ; there is only one stream.
; (display (stream-ref x 3))
; (graphviz)

; (define y (sqrt-stream-slow 2)) ; there will be `n` streams for the `n`-th element.
; (display (stream-ref y 3))
; (graphviz)
