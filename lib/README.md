# How to Create and Use Relic Packages

## Package from Lisp Code

Suppose that you have a `mylib.lisp` and want to use its variables elsewhere:
```lisp
; mylib.lisp
(define (add x y) (+ x y))
(define (neg x) (- 0 x))
(define answer 42)
```

All you need to do is to compile the Lisp code to a shared library and move it to this folder:
```sh
cargo run -- compile -i mylib.lisp -o mylib.c -p mylib
clang -Ic_runtime -shared -o lib/mylib.relic mylib.c
```

Then you can import use it in other lisp files:
```lisp
; other_file.lisp
(import mylib)
(add 2 3)
(neg answer)
```

The compilation flag of `other_file.lisp` is the same as normal files.

### How does it Work

When compiling `mylib` as a package (with `-p` flag), instead of generating a `main` function, the compiler generates a `mylib` function that defines `add`, `neg` and `answer` symbol.

The runtime will try to load `lib/mylib.relic` if the program calls `(import mylib)`. Then it runs the `mylib` function, adding the symbols to the current environment.

The runtime maintains a hash map of opened packages. `mylib` will be added to the map when you call `(import mylib)`. If `mylib` is in the hash map, the `mylib` function will not be called when `(import mylib)` is called.

## Wrapping C Functions to a Package

TODO
