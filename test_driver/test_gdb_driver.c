#include <dlfcn.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    const char *lib_path = NULL;
    const char *fn_name = "main";

    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--fn") == 0 || strcmp(argv[i], "-f") == 0) {
            if (i + 1 < argc) fn_name = argv[++i];
        } else if (argv[i][0] != '-') {
            lib_path = argv[i];
        }
    }

    if (!lib_path) {
        fprintf(stderr,
                "usage: %s [--fn NAME] <library.relic>\n"
                "\n"
                "  <library.relic>  path to compiled shared library\n"
                "  --fn NAME        function to call (default: main)\n"
                "\n"
                "The .relic library must be linked with an RPATH that points to librelic.so.\n"
                "Example clang invocation:\n"
                "  clang -shared -g -fPIC prog.ll -L target/debug -lrelic \\\n"
                "      -Wl,-rpath,$(pwd)/target/debug -o prog.relic\n",
                argv[0]);
        return 1;
    }

    void *lib = dlopen(lib_path, RTLD_NOW);
    if (!lib) {
        fprintf(stderr, "error: %s\n", dlerror());
        return 1;
    }

    int (*fn)() = dlsym(lib, fn_name);
    if (!fn) {
        fprintf(stderr, "error: symbol '%s': %s\n", fn_name, dlerror());
        dlclose(lib);
        return 1;
    }

    int ret = fn();
    printf("=> %d\n", ret);

    dlclose(lib);
    return 0;
}
