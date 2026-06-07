.PHONY: build clean

build: test_gdb_driver

test_gdb_driver: test_gdb_driver.c
	$(CC) -o $@ $< -ldl

clean:
	rm -f test_gdb_driver
