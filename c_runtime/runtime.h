#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

// Misc
extern void rt_start();
extern void rt_import(const char *name);

// Predicate
extern int rt_is_symbol(size_t index);

// Constructor
extern void rt_new_symbol(const char *name);
extern void rt_new_integer(long long value);
extern void rt_new_float(double value);
extern void rt_new_constant(const char *expr);

// Getter and setter
extern long long rt_get_integer(size_t index);
extern double rt_get_float(size_t index);
extern int rt_get_bool(size_t index);
extern char *rt_get_symbol(size_t index);
extern size_t rt_set_car(size_t index, size_t target);
extern size_t rt_set_cdr(size_t index, size_t target);

// stack
extern void rt_push(size_t index);
extern size_t rt_pop(void);
extern size_t rt_top(void);
extern void rt_swap(void);

// IO
extern char *rt_display_node_idx(size_t index);
extern void rt_read();

// Environment
extern void rt_move_to_env(size_t env);
extern size_t rt_current_env();
extern void rt_define(const char *name, size_t value);
extern void rt_set(const char *name, size_t value);
extern size_t rt_get(const char *name);

// Closures
extern void rt_apply();
extern void rt_new_closure(const char *name, void (*func)(void), size_t nargs,
                           int variadic);
extern void rt_prepare_args(size_t cid);
extern void rt_list_to_stack();
typedef void (*c_func)();
extern c_func rt_get_c_func(size_t cid);

// Debug information
extern void rt_evaluated(const char *name, int optimized);
extern void rt_breakpoint();

// Root registers
extern void rt_add_root(const char *name, size_t value);
extern void rt_set_root(const char *name, size_t value);
extern size_t rt_get_root(const char *name);
extern size_t rt_remove_root(const char *name);
