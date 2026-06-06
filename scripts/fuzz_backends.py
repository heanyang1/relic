#!/usr/bin/env python3
"""Fuzz-test Relic's C and LLVM (JIT) compilation backends for consistency.

Generates random Lisp expressions, runs each through the C and/or LLVM
backends, and reports any result mismatches.

The LLVM backend uses inkwell's in-process JIT (JitFunction) — no temp
files, no clang subprocess.  The C backend shells out to gcc and writes
compiled .relic files under /tmp/relic/.

Usage:
  python3 scripts/fuzz_backends.py                              # 2000 tests, both backends
  python3 scripts/fuzz_backends.py --backend llvm               # JIT backend only
  python3 scripts/fuzz_backends.py --backend c                  # C backend only
  python3 scripts/fuzz_backends.py -n 10000 --depth 6           # More coverage
  python3 scripts/fuzz_backends.py --jobs 4 --backend llvm      # Parallel JIT fuzzing
  python3 scripts/fuzz_backends.py --seed 42                    # Reproducible

Parallel execution:
  --jobs >1 is safe for --backend llvm (in-process JIT, no temp files).
  --jobs >1 with --backend c or dual-backend mode will cause spurious
  mismatches because concurrent relic processes race on /tmp/relic/ file names.
"""

import subprocess
import sys
import os
import tempfile
import random
import argparse
from concurrent.futures import ProcessPoolExecutor, as_completed
from collections import Counter

BINARY = "./target/release/relic"

# ---------------------------------------------------------------------------
# Lisp expression generators
# ---------------------------------------------------------------------------

INT_RANGE = (-50, 50)
VAR_NAMES = ["x", "y", "z", "a", "b", "c", "w", "v", "u", "t"]

ARITH_OPS = ["+", "-", "*", "/"]
COMPARE_OPS = [">", "<", ">=", "<=", "="]
LIST_OPS = ["car", "cdr", "cons", "list"]
PRED_OPS = ["null?", "atom?", "number?", "eq?"]
MATH_FUNS = ["abs", "floor", "ceiling", "sin", "cos"]
ALL_SYMBOLS = ARITH_OPS + COMPARE_OPS + LIST_OPS + PRED_OPS + MATH_FUNS


def random_int():
    return random.randint(*INT_RANGE)


def random_float():
    return round(random.uniform(*INT_RANGE), 4)


def generate_atom():
    """Generate an atomic expression."""
    k = random.random()
    if k < 0.35:
        return str(random_int())
    elif k < 0.55:
        return str(random_float())
    elif k < 0.70:
        return random.choice(["#t", "#f"])
    elif k < 0.85:
        return "'()"
    else:
        return f"'{random.choice(VAR_NAMES)}"


def generate_expr(depth):
    """Generate a random Lisp expression as a string, limited by depth."""
    if depth <= 0 or (depth < 3 and random.random() < 0.35):
        return generate_atom()

    kind = random.random()
    if kind < 0.30:
        return generate_arithmetic(depth)
    elif kind < 0.42:
        return generate_list_expr(depth)
    elif kind < 0.55:
        return generate_special_form(depth)
    elif kind < 0.70:
        return generate_lambda_call(depth)
    elif kind < 0.82:
        return generate_let(depth)
    else:
        return generate_predicate(depth)


def generate_arithmetic(depth):
    op = random.choice(ARITH_OPS + COMPARE_OPS + MATH_FUNS)
    if op in MATH_FUNS:
        return f"({op} {generate_expr(depth - 1)})"
    n = random.randint(2, 4)
    args = " ".join(generate_expr(depth - 1) for _ in range(n))
    return f"({op} {args})"


def generate_list_expr(depth):
    op = random.choice(LIST_OPS + ["quote", "list"])
    if op == "quote":
        return f"'{generate_atom_quoted()}"
    if op == "list":
        n = random.randint(0, 4)
        args = " ".join(generate_expr(depth - 1) for _ in range(n))
    elif op == "cons":
        args = f"{generate_expr(depth - 1)} {generate_expr(depth - 1)}"
    else:  # car, cdr
        args = generate_expr(depth - 1)
    return f"({op} {args})"


def generate_atom_quoted():
    k = random.random()
    if k < 0.25:
        return str(random_int())
    elif k < 0.50:
        return str(random_float())
    elif k < 0.75:
        return "()"
    else:
        return random.choice(VAR_NAMES)


def generate_predicate(depth):
    op = random.choice(PRED_OPS)
    if op == "eq?":
        return f"({op} {generate_expr(depth - 1)} {generate_expr(depth - 1)})"
    return f"({op} {generate_expr(depth - 1)})"


def generate_special_form(depth):
    form = random.choice(["if", "begin", "and", "or"])
    if form == "if":
        cond = generate_expr(depth - 1)
        texpr = generate_expr(depth - 1)
        fexpr = generate_expr(depth - 1)
        return f"(if {cond} {texpr} {fexpr})"
    elif form == "begin":
        n = random.randint(2, 4)
        body = " ".join(generate_expr(depth - 1) for _ in range(n))
        return f"(begin {body})"
    else:  # and / or
        n = random.randint(2, 4)
        body = " ".join(generate_expr(depth - 1) for _ in range(n))
        return f"({form} {body})"


def generate_lambda_call(depth):
    nargs = random.randint(0, 3)
    params = [f"v{random.randint(0, 999)}" for _ in range(nargs)]
    body = generate_expr(depth - 1)
    args = " ".join(generate_expr(depth - 1) for _ in params)
    return f"((lambda ({' '.join(params)}) {body}) {args})"


def generate_let(depth):
    n = random.randint(1, 3)
    bindings = []
    for _ in range(n):
        v = f"v{random.randint(0, 999)}"
        val = generate_expr(depth - 1)
        bindings.append(f"({v} {val})")
    body = generate_expr(depth - 1)
    return f"(let ({' '.join(bindings)}) {body})"


# ---------------------------------------------------------------------------
# Test runner
# ---------------------------------------------------------------------------

def run_backend(lisp_code, backend, timeout):
    """Run a single Lisp expression through one backend.

    Returns (returncode, stdout, stderr) or None on timeout.
    """
    with tempfile.NamedTemporaryFile(mode="w", suffix=".lisp", delete=False) as f:
        f.write(lisp_code + "\n")
        fname = f.name
    try:
        proc = subprocess.run(
            [BINARY, "run", "--backend", backend, "-i", fname],
            capture_output=True,
            text=True,
            timeout=timeout,
        )
        return proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired:
        return None
    finally:
        os.unlink(fname)


def extract_result(stdout):
    """Extract the result line from stdout."""
    for line in stdout.splitlines():
        line = line.strip()
        if line.startswith("result:"):
            return line[len("result:"):].strip()
    return stdout.strip()


def compare_one(expr, timeout, backend=None):
    """Run one expr through the specified backend(s).

    If backend is "c" or "llvm", test only that backend.
    If backend is None, test both and compare.
    Returns (expr, diff_msg or None).
    """
    if backend is not None:
        res = run_backend(expr, backend, timeout)
        if res is None:
            return expr, f"{backend} backend TIMEOUT"
        rc, out, err = res
        if rc != 0:
            return expr, f"{backend} backend FAILED (rc={rc}): {err[:200]!r}"
        return expr, None

    c_res = run_backend(expr, "c", timeout)
    l_res = run_backend(expr, "llvm", timeout)

    # Both timed out -> consistent
    if c_res is None and l_res is None:
        return expr, None
    # One timed out -> mismatch
    if c_res is None:
        return expr, f"C backend TIMEOUT, LLVM returned ({l_res[0]}, {l_res[1][:200]!r})"
    if l_res is None:
        return expr, f"LLVM backend TIMEOUT, C returned ({c_res[0]}, {c_res[1][:200]!r})"

    c_rc, c_out, c_err = c_res
    l_rc, l_out, l_err = l_res

    c_result = extract_result(c_out)
    l_result = extract_result(l_out)

    # If return codes differ -> likely a crash in one backend
    if (c_rc == 0) != (l_rc == 0):
        return expr, (
            f"Diff return codes:\n"
            f"  C:    rc={c_rc} result={c_result!r}\n"
            f"  LLVM: rc={l_rc} result={l_result!r}"
        )

    # If both succeeded, compare results
    if c_rc == 0:
        if c_result != l_result:
            return expr, (
                f"Diff results:\n"
                f"  C:    {c_result!r}\n"
                f"  LLVM: {l_result!r}"
            )
        return expr, None

    # Both crashed -- that's consistent
    return expr, None


def _run_one(args_tuple):
    """Pickle-able entry point for ProcessPoolExecutor.

    args_tuple is (expr, timeout, backend) where backend may be None.
    """
    expr, timeout, backend = args_tuple
    return compare_one(expr, timeout, backend)


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main():
    parser = argparse.ArgumentParser(
        description="Fuzz test Relic's C and LLVM (JIT) backends for consistency"
    )
    parser.add_argument("-n", type=int, default=2000,
                        help="Number of random expressions")
    parser.add_argument("--depth", type=int, default=5,
                        help="Max AST depth")
    parser.add_argument("--timeout", type=int, default=15,
                        help="Timeout per expr (seconds)")
    parser.add_argument("--backend", choices=["c", "llvm"],
                        help="Test only one backend (default: both, compare)")
    parser.add_argument("--jobs", type=int, default=1,
                        help="Parallel workers. Safe with --jobs >1 for --backend llvm "
                             "(JIT, no temp files). Not safe with C backend due to "
                             "/tmp/relic/ file name races.")
    parser.add_argument("--seed", type=int,
                        help="Random seed (for reproducibility)")
    parser.add_argument("--err", action="store_true",
                        help="Show stderr on mismatches")
    args = parser.parse_args()

    if args.seed is not None:
        random.seed(args.seed)

    # Warn about unsafe parallel usage
    if args.jobs > 1 and args.backend != "llvm":
        print("WARNING: --jobs >1 with the C backend (or dual mode) will cause")
        print("         spurious mismatches due to /tmp/relic/ file name races.")
        print()

    if not os.path.exists(BINARY):
        print(f"Building Relic (release)...", end=" ", flush=True)
        r = subprocess.run(["cargo", "build", "--release"],
                           capture_output=True, text=True)
        if r.returncode != 0:
            print("FAILED")
            print(r.stderr)
            sys.exit(1)
        print("done")

    mode_label = (f"backend={args.backend}" if args.backend
                  else "dual (C vs LLVM/JIT)")
    print(f"Generating {args.n} random expressions (depth={args.depth})...")
    exprs = [generate_expr(args.depth) for _ in range(args.n)]
    print(f"Testing {mode_label} with {args.jobs} workers "
          f"(timeout={args.timeout}s)...")
    print()

    mismatches = []
    total = 0
    ok = 0
    skipped = 0
    outcomes = Counter()

    tasks = [(e, args.timeout, args.backend) for e in exprs]

    with ProcessPoolExecutor(max_workers=args.jobs) as pool:
        fut_map = {pool.submit(_run_one, t): t[0] for t in tasks}
        for fut in as_completed(fut_map):
            total += 1
            _, diff = fut.result()
            expr = fut_map[fut]
            if diff is None:
                ok += 1
                outcomes["consistent"] += 1
            else:
                mismatches.append((expr, diff))
                outcomes["mismatch"] += 1

            if total % 200 == 0:
                pct = ok / total * 100
                print(f"  {total}/{args.n} tested  --  {pct:.0f}% ok, "
                      f"{len(mismatches)} mismatches so far")

    print()
    print("=" * 60)
    print(f"RESULTS  ({args.n} expressions, {mode_label})")
    print("=" * 60)
    print(f"  Ok (no issues):      {ok}")
    print(f"  Mismatches/errors:   {len(mismatches)}")
    print()

    if mismatches:
        n_show = min(20, len(mismatches))
        print(f"First {n_show} mismatches:\n")
        for expr, diff in mismatches[:n_show]:
            print(f"  Expression: {expr}")
            for line in diff.splitlines():
                print(f"    {line}")
            print()
        if len(mismatches) > 20:
            print(f"  ... and {len(mismatches) - 20} more mismatches")

    # Exit with non-zero if any mismatch found
    if mismatches:
        sys.exit(1)


if __name__ == "__main__":
    main()
