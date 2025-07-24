void func_7();
void func_10();
void func_17();
void func_4();
void func_15();
void func_14();
void func_13();
void func_2();
void func_1();
void func_11();
void func_3();
void func_16();
void func_6();
void func_5();
void func_9();
void func_12();
void func_8();

#include"runtime.h"
int main() {
    rt_start();
    rt_push(rt_new_closure(1, func_1, 1, false));
rt_define("memo-proc", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_closure(4, func_4, 1, false));
rt_define("force", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_constant("nil"));
rt_define("the-empty-stream", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_closure(5, func_5, 1, false));
rt_define("stream-null?", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_symbol("car"));
rt_define("stream-car", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_closure(6, func_6, 1, false));
rt_define("stream-cdr", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_closure(7, func_7, 2, false));
rt_define("stream-ref", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_closure(8, func_8, 2, false));
rt_define("stream-map", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_closure(10, func_10, 2, false));
rt_define("average", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_closure(11, func_11, 2, false));
rt_define("sqrt-improve", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_closure(12, func_12, 1, false));
rt_define("sqrt-stream", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_closure(15, func_15, 1, false));
rt_define("sqrt-stream-slow", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_integer(2));rt_push(rt_get("sqrt-stream"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}
rt_define("x", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_integer(3));rt_push(rt_get("x"));rt_push(rt_get("stream-ref"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
printf("%s",rt_display_node_idx(rt_pop()));
fflush(NULL);
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_symbol("nil"));
    return 0;
}

void func_7() {
    rt_push(rt_new_integer(0));rt_push(rt_get("#1_func_7"));rt_push(rt_new_symbol("="));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}if (rt_get_bool(rt_pop()) > 0) {rt_push(rt_get("#0_func_7"));rt_push(rt_get("stream-car"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}} else {rt_push(rt_new_integer(1));rt_push(rt_get("#1_func_7"));rt_push(rt_new_symbol("-"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}rt_push(rt_get("#0_func_7"));rt_push(rt_get("stream-cdr"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}rt_push(rt_get("stream-ref"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}}
}

void func_10() {
    rt_push(rt_new_integer(2));rt_push(rt_get("#1_func_10"));rt_push(rt_get("#0_func_10"));rt_push(rt_new_symbol("+"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}rt_push(rt_new_symbol("/"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
}

void func_17() {
    rt_push(rt_get("#0_func_15"));rt_push(rt_get("#0_func_17"));rt_push(rt_get("sqrt-improve"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
}

void func_4() {
    rt_push(rt_get("#0_func_4"));
if (rt_is_symbol(rt_top())) {
    rt_apply(0);
} else {
    rt_call_closure(0);
}
}

void func_15() {
    rt_push(rt_new_closure(16, func_16, 0, false));rt_push(rt_get("memo-proc"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}rt_push(rt_new_float(1));rt_push(rt_new_symbol("cons"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
}

void func_14() {
    rt_push(rt_get("#0_func_12"));rt_push(rt_get("#0_func_14"));rt_push(rt_get("sqrt-improve"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
}

void func_13() {
    rt_push(rt_get("guesses"));rt_push(rt_new_closure(14, func_14, 1, false));rt_push(rt_get("stream-map"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
}

void func_2() {
    rt_push(rt_new_closure(3, func_3, 0, false));
}

void func_1() {
    rt_push(rt_new_constant("nil"));rt_push(rt_new_constant("nil"));rt_push(rt_new_closure(2, func_2, 2, false));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
}

void func_11() {
    rt_push(rt_get("#0_func_11"));rt_push(rt_get("#1_func_11"));rt_push(rt_new_symbol("/"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}rt_push(rt_get("#0_func_11"));rt_push(rt_get("average"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
}

void func_3() {
    rt_push(rt_new_constant("nil"));rt_push(rt_get("#0_func_2"));rt_push(rt_new_symbol("eq?"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}if (rt_get_bool(rt_pop()) > 0) {rt_push(rt_get("#0_func_1"));
if (rt_is_symbol(rt_top())) {
    rt_apply(0);
} else {
    rt_call_closure(0);
}
rt_set("#1_func_2", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_new_constant("t"));
rt_set("#0_func_2", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_get("#1_func_2"));} else {rt_push(rt_get("#1_func_2"));}
}

void func_16() {
    rt_push(rt_get("#0_func_15"));rt_push(rt_get("sqrt-stream-slow"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}rt_push(rt_new_closure(17, func_17, 1, false));rt_push(rt_get("stream-map"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
}

void func_6() {
    rt_push(rt_get("#0_func_6"));rt_push(rt_new_symbol("cdr"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}rt_push(rt_get("force"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}
}

void func_5() {
    rt_push(rt_new_constant("nil"));rt_push(rt_get("#0_func_5"));rt_push(rt_new_symbol("eq?"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
}

void func_9() {
    rt_push(rt_get("#1_func_8"));rt_push(rt_get("stream-cdr"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}rt_push(rt_get("#0_func_8"));rt_push(rt_get("stream-map"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
}

void func_12() {
    rt_push(rt_new_closure(13, func_13, 0, false));rt_push(rt_get("memo-proc"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}rt_push(rt_new_float(1));rt_push(rt_new_symbol("cons"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}
rt_define("guesses", rt_pop());
rt_push(rt_new_symbol("nil"));rt_pop();rt_push(rt_get("guesses"));
}

void func_8() {
    rt_push(rt_get("#1_func_8"));rt_push(rt_get("stream-null?"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}if (rt_get_bool(rt_pop()) > 0) {rt_push(rt_get("the-empty-stream"));} else {rt_push(rt_new_closure(9, func_9, 0, false));rt_push(rt_get("memo-proc"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}rt_push(rt_get("#1_func_8"));rt_push(rt_get("stream-car"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}rt_push(rt_get("#0_func_8"));
if (rt_is_symbol(rt_top())) {
    rt_apply(1);
} else {
    rt_call_closure(1);
}rt_push(rt_new_symbol("cons"));
if (rt_is_symbol(rt_top())) {
    rt_apply(2);
} else {
    rt_call_closure(2);
}}
}
