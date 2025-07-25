#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>

// Runtime functions.
extern void rt_start();
extern void rt_new_symbol(const char *name);
extern void rt_new_integer(long long value);
extern void rt_new_float(double value);
extern void rt_new_constant(const char *expr);
extern size_t rt_set_car(size_t index, size_t target);
extern size_t rt_set_cdr(size_t index, size_t target);
extern long long rt_get_integer(size_t index);
extern double rt_get_float(size_t index);
extern int rt_get_bool(size_t index);
extern void rt_add_root(const char *name, size_t value);
extern void rt_set_root(const char *name, size_t value);
extern size_t rt_get_root(const char *name);
extern void rt_push(size_t index);
extern size_t rt_pop(void);
extern size_t rt_top(void);
extern size_t rt_remove_root(const char *name);
extern char *rt_display_node_idx(size_t index);
extern size_t rt_new_env(const char *name, size_t outer);
extern void rt_move_to_env(size_t env);
extern size_t rt_current_env();
extern void rt_define(const char *name, size_t value);
extern void rt_set(const char *name, size_t value);
extern size_t rt_get(const char *name);
extern char *rt_get_symbol(size_t index);
extern void rt_apply(size_t nargs);
extern void rt_call_closure(size_t nargs);
extern void rt_new_closure(size_t id, void (*func)(void), size_t nargs, int variadic);
extern int rt_is_symbol(size_t index);
