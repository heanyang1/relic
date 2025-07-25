#!/usr/bin/env python3
"""
Test driver for compiling and running C files generated by compile_test.rs.

This script will:
1. Find all .c files in the tests directory
2. Use the Makefile to build each test
3. Run the compiled executable
4. Compare the output with the expected .out file
"""

import os
import subprocess
import sys
from pathlib import Path

# Configuration
RUNTIME_DIR = Path(__file__).parent.absolute()
TESTS_DIR = os.path.join(RUNTIME_DIR, "tests")
BUILD_DIR = os.path.join(RUNTIME_DIR, "build")
MAKE = "make"

# ANSI color codes
GREEN = "\033[32m"
RED = "\033[31m"
YELLOW = "\033[33m"
BLUE = "\033[34m"
RESET = "\033[0m"
BOLD = "\033[1m"

# Test status indicators
PASS = f"{GREEN}PASS{RESET}"
FAIL = f"{RED}FAIL{RESET}"
INFO = f"{BLUE}INFO{RESET}"

def find_test_files():
    """Find all test files in the tests directory."""
    test_files = []
    for entry in os.scandir(TESTS_DIR):
        if entry.is_file() and entry.name.endswith('.c'):
            base_name = os.path.splitext(entry.name)[0]
            c_file = os.path.join(TESTS_DIR, f"{base_name}.c")
            out_file = os.path.join(TESTS_DIR, f"{base_name}.out")
            if os.path.exists(out_file):
                test_files.append((base_name, c_file, out_file))
    return test_files

def read_expected_output(out_file):
    """Read the expected output from a .out file."""
    with open(out_file, 'r') as f:
        return f.read().strip()

def build_test(base_name, c_file):
    """Build a test using the Makefile."""
    # The Makefile will put the executable in the build directory
    executable = os.path.join(BUILD_DIR, base_name)
    
    try:
        # Run make to build the specific test
        print(f"{INFO} Building: {base_name}")
        
        # Use a single make command to build the specific test target
        result = subprocess.run(
            [MAKE, f"build/{base_name}"],
            cwd=str(RUNTIME_DIR),
            capture_output=True,
            text=True
        )
        
        if result.returncode != 0:
            print(f"{FAIL} Build failed for {base_name}")
            if result.stderr:
                print(f"{INFO} Error output:")
                print(result.stderr)
            return None
            
        return executable
    except Exception as e:
        print(f"{FAIL} Error building {base_name}: {str(e)}")
        import traceback
        traceback.print_exc()
        return None

def run_test(executable_path):
    """Run a compiled test and return its output."""
    try:
        # Make sure the executable exists and has execute permissions
        if not os.path.exists(executable_path):
            return -1, f"Executable not found: {executable_path}"
            
        if not os.access(executable_path, os.X_OK):
            # Try to add execute permissions if missing
            try:
                os.chmod(executable_path, 0o755)
            except Exception as e:
                return -1, f"No execute permissions and couldn't set them: {str(e)}"
        
        # Get the directory containing the Rust library
        # The library is typically in target/debug/ or target/release/
        rust_target_dir = os.path.abspath(os.path.join(
            os.path.dirname(__file__), 
            '..', 
            'target',
            'debug'  # Using debug build by default
        ))
        
        # Set up environment with LD_LIBRARY_PATH
        env = os.environ.copy()
        if 'LD_LIBRARY_PATH' in env:
            env['LD_LIBRARY_PATH'] = f"{rust_target_dir}:{env['LD_LIBRARY_PATH']}"
        else:
            env['LD_LIBRARY_PATH'] = rust_target_dir
        
        # Run the test with the current working directory set to the executable's directory
        exec_dir = os.path.dirname(executable_path) or '.'
        exec_name = os.path.basename(executable_path)
        
        # Use absolute path to the executable
        abs_exec_path = os.path.abspath(executable_path)
        
        print(f"{INFO} Running: {abs_exec_path}")
        print(f"{INFO} With LD_LIBRARY_PATH: {env['LD_LIBRARY_PATH']}")
        
        result = subprocess.run(
            [abs_exec_path],
            cwd=exec_dir,
            env=env,  # Pass the modified environment
            capture_output=True,
            text=True
        )
        
        # Print debug info if the test fails
        if result.returncode != 0:
            print(f"{INFO} Test failed with return code {result.returncode}")
            if result.stderr:
                print(f"{INFO} Error output:")
                print(result.stderr)
        
        return result.returncode, result.stdout.strip()
    except Exception as e:
        import traceback
        error_msg = f"{FAIL} Error running test: {str(e)}\n{traceback.format_exc()}"
        print(error_msg)
        return -1, error_msg

def run_tests():
    """Run all tests and report results."""
    test_files = find_test_files()
    if not test_files:
        print(f"{FAIL} No test files found in {TESTS_DIR}")
        return 1
    
    print(f"{INFO} Found {len(test_files)} test(s)")
    print("-" * 50)
    
    passed = 0
    failed = 0
    
    for base_name, c_file, out_file in test_files:
        print(f"{INFO} Testing {base_name}...")
        
        # Build the test using the Makefile
        executable = build_test(base_name, c_file)
        if not executable:
            failed += 1
            continue
        
        # Get expected output
        try:
            expected = read_expected_output(out_file)
        except Exception as e:
            print(f"{FAIL} Failed to read expected output for {base_name}: {str(e)}")
            failed += 1
            continue
        
        # Run the test
        return_code, actual = run_test(executable)
        
        # Check results
        if return_code != 0:
            print(f"{FAIL} {base_name}: Test failed with return code {return_code}")
            if actual:
                print(f"{INFO} Output: {actual}")
            failed += 1
        elif str(actual) == expected:
            print(f"{PASS} {base_name}: Passed")
            print(f"{INFO} Expected: {expected}")
            print(f"{INFO} Got:      {actual}")
            passed += 1
        else:
            print(f"{FAIL} {base_name}: Output mismatch")
            print(f"{INFO} Expected: {expected}")
            print(f"{INFO} Got:      {actual}")
            failed += 1
        
        print()
    
    # Print summary
    print("-" * 50)
    if failed > 0:
        print(f"{BOLD}Tests passed: {GREEN}{passed}{RESET}, {BOLD}failed: {RED}{failed}{RESET}")
    else:
        print(f"{BOLD}All {passed} tests {GREEN}passed{RESET}{BOLD}!{RESET}")
    
    return 1 if failed > 0 else 0

if __name__ == "__main__":
    sys.exit(run_tests())
