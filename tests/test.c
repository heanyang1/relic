#include "../c_runtime/runtime.h"
#include <math.h>

int test() {
  rt_new_integer(1234);
  rt_set_root("...", rt_pop());
  
  rt_new_float(1.234);
  double f = rt_get_float(rt_pop());
  assert(fabs(f - 1.234) < 0.01);
  
  rt_new_symbol("1234");
  char *s = rt_get_symbol(rt_pop());
  assert(strcmp(s, "1234") == 0);
  rt_breakpoint();

  s = rt_get_symbol(rt_get("..."));
  assert(strcmp(s, "nil") == 0);

  int i = rt_get_integer(rt_get_root("..."));
  assert(i == 1234);
  return 0;
}
