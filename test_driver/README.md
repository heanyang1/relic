# GDB Debug Driver

A minimal C host program for debugging Relic-compiled shared libraries with GDB.

When you compile a Lisp program with `--backend llvm -g`, the generated LLVM IR
contains DWARF debug info that maps source lines to compiled code. Use this
driver to load and step through that code in GDB.

## Quick Start

```bash
# 1. Build the relic runtime
cargo build

# 2. Build the driver
cc -o test_driver/test_gdb_driver test_driver/test_gdb_driver.c -ldl

# 3. Compile your Lisp program to LLVM IR with debug info
relic compile --backend llvm -g -i program.lisp -o program.ll

# 4. Compile the IR to a shared library
#    The RPATH is critical — it tells the library where to find librelic.so.
clang -shared -g -fPIC program.ll \
    -L target/debug -lrelic \
    -Wl,-rpath,$(pwd)/target/debug \
    -o program.relic
```

## Debug with GDB

Use **absolute paths** so GDB finds everything regardless of working directory:

```bash
gdb --args $(pwd)/test_driver/test_gdb_driver $(pwd)/program.relic
```

Inside GDB:

```
(gdb) break program.lisp:5    # break by source file and line
(gdb) run
(gdb) list                    # show source around current line
(gdb) step                    # step into function
(gdb) next                    # step over
(gdb) bt                      # backtrace — shows function names
```

## Driver Options

```
usage: test_gdb_driver [--fn NAME] <library.relic>
```

| Option            | Description                    | Default  |
|-------------------|--------------------------------|----------|
| `<library.relic>` | Path to the compiled library   | required |
| `--fn NAME`       | Function to call               | `main`   |

## Complete Example

```lisp
;; program.lisp
(define (square x)
  (* x x))

(define (sum-of-squares a b)
  (+ (square a) (square b)))

(display (sum-of-squares 3 4))
```

```bash
cargo build
cc -o test_driver/test_gdb_driver test_driver/test_gdb_driver.c -ldl

relic compile --backend llvm -g -i program.lisp -o program.ll
clang -shared -g -fPIC program.ll \
    -L target/debug -lrelic \
    -Wl,-rpath,$(pwd)/target/debug \
    -o program.relic

gdb --args "$(pwd)/test_driver/test_gdb_driver" "$(pwd)/program.relic"
(gdb) break program.lisp:5
(gdb) break program.lisp:2
(gdb) run
```

## Package Libraries

If you compiled with `--package-name mylib`, call the named function:

```bash
gdb --args $(pwd)/test_driver/test_gdb_driver --fn mylib $(pwd)/mylib.relic
```

## Troubleshooting

**"error: prog.relic: cannot open shared object file: No such file or directory"**

Use an absolute path for the `.relic` file. Within GDB this is especially
important because GDB may change the working directory.

```bash
# wrong (fails in GDB):
gdb --args ./test_driver/test_gdb_driver ./prog.relic

# correct:
gdb --args $(pwd)/test_driver/test_gdb_driver $(pwd)/prog.relic
```

**"error: librelic.so: cannot open shared object file"**

The `.relic` library can't find `librelic.so`. Recompile with the proper RPATH:

```bash
clang -shared -g -fPIC program.ll -L target/debug -lrelic \
    -Wl,-rpath,$(pwd)/target/debug -o program.relic
```

Verify the RPATH is set:

```bash
readelf -d program.relic | grep RPATH
# Should show: 0x... (RPATH)  Library rpath: [/path/to/project/target/debug]
```
