;; This demo uses SDL2 bindings to implement (a subset of) the picture language
;; in the chapter 2.2.4 of SICP.

(import sdl2)

;; We use (x y w h) as frame.
(define get-frame-x car)
(define (get-frame-y frame) (car (cdr frame)))
(define (get-frame-w frame) (car (cdr (cdr frame))))
(define (get-frame-h frame) (car (cdr (cdr (cdr frame)))))

(define time 0)

(define pi 3.14)
(define (get-color phase) (abs (floor (* 255 (sin (+ time phase))))))

(define (primitive-painter frame)
    (sdl-fill-rect-xywh
     screen-surface 0 0 0
     (get-frame-x frame) (get-frame-y frame) (get-frame-w frame) (get-frame-h frame))
  (sdl-fill-rect-xywh
   screen-surface (get-color pi) (get-color (/ pi -3)) (get-color (/ pi 3))
   (+ 1 (get-frame-x frame)) (+ 1 (get-frame-y frame)) (- (get-frame-w frame) 2) (- (get-frame-h frame) 2)))

(define (above painter1 painter2)
    (lambda (frame)
      (let ((w (get-frame-w frame))
            (h (quotient (get-frame-h frame) 2))
            (x (get-frame-x frame))
            (y1 (get-frame-y frame))
            (y2 (+ (quotient (get-frame-h frame) 2) (get-frame-y frame))))
        (painter1 (list x y1 w h))
        (painter2 (list x y2 w h)))))

(define (beside painter1 painter2)
    (lambda (frame)
      (let ((w (quotient (get-frame-w frame) 2))
            (h (get-frame-h frame))
            (y (get-frame-y frame))
            (x1 (get-frame-x frame))
            (x2 (+ (quotient (get-frame-w frame) 2) (get-frame-x frame))))
        (painter1 (list x1 y w h))
        (painter2 (list x2 y w h)))))

(define (final-painter n)
    (if (= n 0)
        primitive-painter
        (beside primitive-painter (above (final-painter (- n 1)) (final-painter (- n 1))))))

(define (paint painter) (painter '(0 0 512 512)))

;; Initialize SDL2
(sdl-init SDL_INIT_VIDEO)

;; Create a window
(define window (sdl-create-window "Relic SDL2 Demo" 
                                 SDL_WINDOWPOS_CENTERED SDL_WINDOWPOS_CENTERED 
                                 512 512
                                 SDL_WINDOW_SHOWN))

;; Get the window surface
(define screen-surface (sdl-get-window-surface window))

(define (loop)
    (let ((event (sdl-poll-event)))
      (cond 
        ;; Check for quit event (window close button)
        ((eq? event SDL_QUIT) '())
        
        ;; Check for key press
        ((eq? event SDL_KEYDOWN) '())
        
        ;; Continue running for other events
        ('t 
         ;; Fill the screen with white
         (sdl-fill-rect screen-surface 255 255 255)
         
         ;; Paint the picture
         (paint (final-painter 5))
         
         ;; Update the window
         (sdl-update-window-surface window)
         
         ;; Small delay to prevent high CPU usage
         (sdl-delay 16)
         
         (set! time (+ 0.1 time))
         (loop)))))

(loop)

;; Clean up
(sdl-destroy-window window)
(sdl-quit)
