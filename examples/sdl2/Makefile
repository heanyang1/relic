CC = clang
CFLAGS = -I../../c_runtime `pkg-config --cflags sdl2` -fPIC -shared -g
LDFLAGS = `pkg-config --libs sdl2`

# Add MacOS-specific linker flag
ifeq ($(shell uname -s),Darwin)
    LDFLAGS += -Wl,-undefined,dynamic_lookup
endif

LIB_FOLDER = ../../lib

all: $(LIB_FOLDER)/sdl2.relic

$(LIB_FOLDER)/sdl2.relic: sdl2.c
	$(CC) $(CFLAGS) -o $@ $< $(LDFLAGS)

clean:
	rm -f $(LIB_FOLDER)/sdl2.relic

run: $(LIB_FOLDER)/sdl2.relic
	cd ../.. && cargo run -- run -i examples/sdl2/demo.lisp

.PHONY: all clean run
