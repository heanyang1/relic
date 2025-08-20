;; Simulation of `(Set, 1, *)` as a monoidal category
;; See chapter 4 of [Seven Sketches in Compositionality: An Invitation to Applied Category Theory](https://arxiv.org/abs/1803.05316v3) for more details.

(import list)

;; The objects of `(Set, 1, *)` can be any Lisp objects.

;; A morphism can be described as `(number of input, number of output, function)`.
;; The function can accept multiple objects, but it must return a list of objects.
(define (morphism/make i o f)
    (lambda (query)
      (cond ((eq? query 'in) i)
            ((eq? query 'out) o)
            ((eq? query 'func) f))))

(define (morphism/compose m1 m2)
  (let ((in1 (m1 'in))
        (in2 (m2 'in))
        (out1 (m1 'out))
        (out2 (m2 'out))
        (f1 (m1 'func))
        (f2 (m2 'func)))
    (if (= out1 in2)
        (morphism/make
         in1 out2
         (lambda x (apply f2 (apply f1 x))))
        (error "arity mismatch"))))

(define morphism/identity (morphism/make 1 1 (lambda x x)))

(define (morphism/product m1 m2)
    (let ((in1 (m1 'in))
          (in2 (m2 'in))
          (out1 (m1 'out))
          (out2 (m2 'out))
          (f1 (m1 'func))
          (f2 (m2 'func)))
      (morphism/make
       (+ in1 in2) (+ out1 out2)
       (lambda x
         (append (apply f1 (take x in1))
                 (apply f2 (drop x in1)))))))

(define f (morphism/make
           1 2
           (lambda (a) (list (abs a) (* 5 a)))))
(define g (morphism/make
           2 2
           (lambda (d b) (list (<= d b) (- d b)))))
(define h (morphism/make
           2 1
           (lambda (c e)
             (list (if e c (- 1 c))))))

;; This is the solution to exercise 4.50 of the applied category book.
(display ((g 'func) 5 3))
(newline)
(display ((g 'func) 3 5))
(newline)
(display ((h 'func) 5 't))
(newline)
(display ((h 'func) -5 't))
(newline)
(display ((h 'func) -5 '()))
(newline)
;; q = (f * id) ; (id * g) ; (h * id)
(define q (morphism/compose
           (morphism/product f morphism/identity)
           (morphism/compose
            (morphism/product morphism/identity g)
            (morphism/product h morphism/identity))))
(display ((q 'func) -2 3))
(newline)
(display ((q 'func) 2 3))
