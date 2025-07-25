CC = gcc
CFLAGS = -g -Wall -O1 -I.
LLVM_CFLAGS = -Wall -O1 -emit-llvm
LIB_DIR = ../target/debug
LIB_NAME = relic
LDFLAGS = -Wl,-rpath,$(LIB_DIR)
BUILD_DIR = build

# Find all .c test files
TEST_SRCS = $(wildcard tests/*.c)
# Generate corresponding executable names in the build directory
TEST_EXES = $(addprefix $(BUILD_DIR)/, $(notdir $(TEST_SRCS:.c=)))

# Default target: build all test executables
all: $(BUILD_DIR) $(TEST_EXES)

# Create build directory if it doesn't exist
$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

library:
	cargo build --lib

format: $(wildcard tests/*.c)
	clang-format -i $^

# Pattern rule to build any test executable
$(BUILD_DIR)/%: tests/%.c library $(BUILD_DIR)
	@$(CC) $(CFLAGS) -o $@ $< -L$(LIB_DIR) -l$(LIB_NAME) $(LDFLAGS)
	@echo "+ CC $< -> $@"

clean:
	rm -rf $(BUILD_DIR)

.PHONY: all clean library format
