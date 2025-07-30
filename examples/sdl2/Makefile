CC = clang
CFLAGS = -I../../c_runtime `sdl2-config --cflags` -fPIC -shared -g
LDFLAGS = `sdl2-config --libs` -lSDL2

LIB_FOLDER = ../../lib

all: $(LIB_FOLDER)/sdl2.relic

$(LIB_FOLDER)/sdl2.relic: sdl2.c
	$(CC) $(CFLAGS) -o $@ $< $(LDFLAGS)

clean:
	rm -f $(LIB_FOLDER)/sdl2.relic
