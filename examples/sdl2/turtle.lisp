;; This demo uses SDL2 bindings to implement (a subset of)
;; [Racket's value turtles](https://docs.racket-lang.org/turtles/Value_Turtles.html).

(import sdl2)
(import list)
(import function)

(define pi 3.14159265358979323846)

;; A turtle is defined as (x, y, angle in radians, list of lines to draw),
;; where an line is represented by (x-start y-start x-end y-end).
(define (make-turtle x y angle prev) (list x y angle prev))
(define get-x car)
(define get-y cadr)
(define get-angle caddr)
(define get-prev cadddr)

(define (turn turtle angle)
    (make-turtle (get-x turtle) (get-y turtle) (+ (get-angle turtle) angle) (get-prev turtle)))
(define (go-forward turtle distance)
    (let ((nx (+ (get-x turtle) (* distance (cos (get-angle turtle)))))
          (ny (+ (get-y turtle) (* distance (sin (get-angle turtle))))))
      (make-turtle nx
                   ny
                   (get-angle turtle)
                   (cons (list (get-x turtle) (get-y turtle) nx ny)
                         (get-prev turtle)))))

(define (run turtle window renderer)
  (define (draw lines)
      (if (eq? lines '())
          '()
          (let ((line (car lines)))
            (sdl-draw-line renderer
                           (floor (car line))
                           (floor (cadr line))
                           (floor (caddr line))
                           (floor (cadddr line)))
            (draw (cdr lines)))))

  (define (loop)
      (let ((event (sdl-poll-event)))
        (cond 
          ;; Check for quit event (window close button)
          ((eq? event SDL_QUIT) '())
        
          ;; Check for key press
          ((eq? event SDL_KEYDOWN) '())
        
          ;; Continue running for other events
          ('t 
           ;; Paint the picture
           (draw (get-prev turtle))

           (breakpoint)

           ;; Render the picture
           (sdl-render-present renderer)

           ;; Small delay to prevent high CPU usage
           (sdl-delay 32)
         
           (loop)))))
  (loop))

;; Initialize SDL2
(sdl-init SDL_INIT_VIDEO)

;; Create a window
(define window (sdl-create-window "Relic SDL2 Demo" 
                                  SDL_WINDOWPOS_CENTERED SDL_WINDOWPOS_CENTERED 
                                  1000 1000
                                  SDL_WINDOW_SHOWN))

;; Get the renderer
(define renderer (sdl-create-renderer window))

;; We can draw a Sierpinski triangle using turtle in 11 lines of code!
;; Sierpinski triangle (and many fractals) can be described using
;; [L-systems](https://en.wikipedia.org/wiki/L-system)
(define (a turtle) (go-forward turtle 50)) ;; axiom
(define (p turtle) (turn turtle (* 2 (/ pi 3)))) ;; constant +
(define (m turtle) (turn turtle (* 4 (/ pi 3)))) ;; constant -
(define (st fg)
  (let ((f (car fg))
        (g (cdr fg)))
    (cons (compose f m g p f p g m f) (compose g g))))
(define st-result ((compose st st st st) (cons a a)))
(let ((f (car st-result))
      (g (cdr st-result)))
  (run ((compose f m g m g) (make-turtle 20 800 0 '())) window renderer))

;; Clean up
(sdl-destroy-renderer renderer)
(sdl-destroy-window window)
(sdl-quit)
