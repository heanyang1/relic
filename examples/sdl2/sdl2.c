#include "../../c_runtime/runtime.h"

#include <SDL.h>

// Wrapper for SDL_Init
void sdl_init_wrapper() {
  long long flags = rt_get_integer(rt_get("#0_func_sdl_init"));
  int result = SDL_Init((Uint32)flags);
  rt_new_integer(result);
}

// Wrapper for SDL_CreateWindow
void sdl_create_window_wrapper() {
  const char *title = rt_get_symbol(rt_get("#0_func_sdl_create_window"));
  int x = (int)rt_get_integer(rt_get("#1_func_sdl_create_window"));
  int y = (int)rt_get_integer(rt_get("#2_func_sdl_create_window"));
  int w = (int)rt_get_integer(rt_get("#3_func_sdl_create_window"));
  int h = (int)rt_get_integer(rt_get("#4_func_sdl_create_window"));
  Uint32 flags = (Uint32)rt_get_integer(rt_get("#5_func_sdl_create_window"));

  SDL_Window *window = SDL_CreateWindow(title, x, y, w, h, flags);
  rt_new_integer((long long)window);
}

// Wrapper for SDL_CreateRenderer
void sdl_create_renderer_wrapper() {
  SDL_Window *window =
      (SDL_Window *)rt_get_integer(rt_get("#0_func_sdl_create_renderer"));
  SDL_Renderer *renderer =
      SDL_CreateRenderer(window, -1, SDL_RENDERER_ACCELERATED);

  // Clear screen
  SDL_SetRenderDrawColor(renderer, 0, 0, 0, 255);
  SDL_RenderClear(renderer);

  // Set line color
  SDL_SetRenderDrawColor(renderer, 255, 255, 255, 255);

  rt_new_integer((long long)renderer);
}

// Wrapper for SDL_GetWindowSurface
void sdl_get_window_surface_wrapper() {
  SDL_Window *window =
      (SDL_Window *)rt_get_integer(rt_get("#0_func_sdl_get_window_surface"));
  SDL_Surface *surface = SDL_GetWindowSurface(window);
  rt_new_integer((long long)surface);
}

// Wrapper for SDL_FillRect to fill entire surface
void sdl_fill_rect_wrapper() {
  SDL_Surface *surface =
      (SDL_Surface *)rt_get_integer(rt_get("#0_func_sdl_fill_rect"));
  Uint8 r = (Uint8)rt_get_integer(rt_get("#1_func_sdl_fill_rect"));
  Uint8 g = (Uint8)rt_get_integer(rt_get("#2_func_sdl_fill_rect"));
  Uint8 b = (Uint8)rt_get_integer(rt_get("#3_func_sdl_fill_rect"));
  Uint32 color = SDL_MapRGB(surface->format, r, g, b);

  int result = SDL_FillRect(surface, NULL, color);
  rt_new_integer(result);
}

// Wrapper for SDL_FillRect to fill a specific rectangle
void sdl_fill_rect_xywh_wrapper() {
  SDL_Surface *surface =
      (SDL_Surface *)rt_get_integer(rt_get("#0_func_sdl_fill_rect_xywh"));
  Uint8 r = (Uint8)rt_get_integer(rt_get("#1_func_sdl_fill_rect_xywh"));
  Uint8 g = (Uint8)rt_get_integer(rt_get("#2_func_sdl_fill_rect_xywh"));
  Uint8 b = (Uint8)rt_get_integer(rt_get("#3_func_sdl_fill_rect_xywh"));
  SDL_Rect rect;
  rect.x = (int)rt_get_integer(rt_get("#4_func_sdl_fill_rect_xywh"));
  rect.y = (int)rt_get_integer(rt_get("#5_func_sdl_fill_rect_xywh"));
  rect.w = (int)rt_get_integer(rt_get("#6_func_sdl_fill_rect_xywh"));
  rect.h = (int)rt_get_integer(rt_get("#7_func_sdl_fill_rect_xywh"));

  Uint32 color = SDL_MapRGB(surface->format, r, g, b);
  int result = SDL_FillRect(surface, &rect, color);
  rt_new_integer(result);
}

// Wrapper for SDL_RenderDrawLine
void sdl_draw_line_wrapper() {
  SDL_Renderer *renderer =
      (SDL_Renderer *)rt_get_integer(rt_get("#0_func_sdl_draw_line"));
  int start_x = (int)rt_get_integer(rt_get("#1_func_sdl_draw_line"));
  int start_y = (int)rt_get_integer(rt_get("#2_func_sdl_draw_line"));
  int end_x = (int)rt_get_integer(rt_get("#3_func_sdl_draw_line"));
  int end_y = (int)rt_get_integer(rt_get("#4_func_sdl_draw_line"));
  int result = SDL_RenderDrawLine(renderer, start_x, start_y, end_x, end_y);
  rt_new_integer(result);
}

// Wrapper for SDL_RenderPresent
void sdl_render_present_wrapper() {
  SDL_Renderer *renderer =
      (SDL_Renderer *)rt_get_integer(rt_get("#0_func_sdl_render_present"));
  SDL_RenderPresent(renderer);
  rt_new_symbol("nil");
}

// Wrapper for SDL_UpdateWindowSurface
void sdl_update_window_surface_wrapper() {
  SDL_Window *window =
      (SDL_Window *)rt_get_integer(rt_get("#0_func_sdl_update_window_surface"));
  int result = SDL_UpdateWindowSurface(window);
  rt_new_integer(result);
}

// Wrapper for SDL_Delay
void sdl_delay_wrapper() {
  Uint32 ms = (Uint32)rt_get_integer(rt_get("#0_func_sdl_delay"));
  SDL_Delay(ms);
  rt_new_symbol("nil");
}

// Wrapper for SDL_PollEvent
void sdl_poll_event_wrapper() {
  SDL_Event event;
  int has_event = SDL_PollEvent(&event);
  if (has_event) {
    switch (event.type) {
    case SDL_QUIT:
      rt_new_integer(SDL_QUIT);
      break;
    case SDL_KEYDOWN:
      rt_new_integer(SDL_KEYDOWN);
      break;
    default:
      rt_new_integer(0);
      break;
    }
  } else {
    rt_new_integer(0);
  }
}

// Wrapper for SDL_DestroyWindow
void sdl_destroy_window_wrapper() {
  SDL_Window *window =
      (SDL_Window *)rt_get_integer(rt_get("#0_func_sdl_destroy_window"));
  SDL_DestroyWindow(window);
  rt_new_symbol("nil");
}

// Wrapper for SDL_DestroyRenderer
void sdl_destroy_renderer_wrapper() {
  SDL_Renderer *renderer =
      (SDL_Renderer *)rt_get_integer(rt_get("#0_func_sdl_destroy_renderer"));
  SDL_DestroyRenderer(renderer);
  rt_new_symbol("nil");
}

// Wrapper for SDL_Quit
void sdl_quit_wrapper() {
  SDL_Quit();
  rt_new_symbol("nil");
}

// Helper function to define SDL constants
void define_sdl_constant(const char *name, long long value) {
  rt_new_integer(value);
  rt_define(name, rt_pop());
}

// Initialize the SDL2 package
int sdl2() {
  // SDL initialization flags
  define_sdl_constant("SDL_INIT_TIMER", SDL_INIT_TIMER);
  define_sdl_constant("SDL_INIT_AUDIO", SDL_INIT_AUDIO);
  define_sdl_constant("SDL_INIT_VIDEO", SDL_INIT_VIDEO);
  define_sdl_constant("SDL_INIT_JOYSTICK", SDL_INIT_JOYSTICK);
  define_sdl_constant("SDL_INIT_HAPTIC", SDL_INIT_HAPTIC);
  define_sdl_constant("SDL_INIT_GAMECONTROLLER", SDL_INIT_GAMECONTROLLER);
  define_sdl_constant("SDL_INIT_EVENTS", SDL_INIT_EVENTS);
  define_sdl_constant("SDL_INIT_EVERYTHING", SDL_INIT_EVERYTHING);

  // Window flags
  define_sdl_constant("SDL_WINDOW_FULLSCREEN", SDL_WINDOW_FULLSCREEN);
  define_sdl_constant("SDL_WINDOW_OPENGL", SDL_WINDOW_OPENGL);
  define_sdl_constant("SDL_WINDOW_SHOWN", SDL_WINDOW_SHOWN);
  define_sdl_constant("SDL_WINDOW_HIDDEN", SDL_WINDOW_HIDDEN);
  define_sdl_constant("SDL_WINDOW_BORDERLESS", SDL_WINDOW_BORDERLESS);
  define_sdl_constant("SDL_WINDOW_RESIZABLE", SDL_WINDOW_RESIZABLE);
  define_sdl_constant("SDL_WINDOW_MINIMIZED", SDL_WINDOW_MINIMIZED);
  define_sdl_constant("SDL_WINDOW_MAXIMIZED", SDL_WINDOW_MAXIMIZED);
  define_sdl_constant("SDL_WINDOW_INPUT_GRABBED", SDL_WINDOW_INPUT_GRABBED);

  // Event types
  define_sdl_constant("SDL_QUIT", SDL_QUIT);
  define_sdl_constant("SDL_KEYDOWN", SDL_KEYDOWN);
  define_sdl_constant("SDL_KEYUP", SDL_KEYUP);

  // Window position constants
  define_sdl_constant("SDL_WINDOWPOS_UNDEFINED", SDL_WINDOWPOS_UNDEFINED);
  define_sdl_constant("SDL_WINDOWPOS_CENTERED", SDL_WINDOWPOS_CENTERED);

  // Register SDL functions
  rt_new_closure("sdl_init", sdl_init_wrapper, 1, false);
  rt_define("sdl-init", rt_pop());

  rt_new_closure("sdl_create_window", sdl_create_window_wrapper, 6, false);
  rt_define("sdl-create-window", rt_pop());

  rt_new_closure("sdl_get_window_surface", sdl_get_window_surface_wrapper, 1,
                 false);
  rt_define("sdl-get-window-surface", rt_pop());

  rt_new_closure("sdl_create_renderer", sdl_create_renderer_wrapper, 1, false);
  rt_define("sdl-create-renderer", rt_pop());

  rt_new_closure("sdl_fill_rect", sdl_fill_rect_wrapper, 4, false);
  rt_define("sdl-fill-rect", rt_pop());

  rt_new_closure("sdl_fill_rect_xywh", sdl_fill_rect_xywh_wrapper, 8, false);
  rt_define("sdl-fill-rect-xywh", rt_pop());

  rt_new_closure("sdl_draw_line", sdl_draw_line_wrapper, 5, false);
  rt_define("sdl-draw-line", rt_pop());

  rt_new_closure("sdl_render_present", sdl_render_present_wrapper, 1, false);
  rt_define("sdl-render-present", rt_pop());

  rt_new_closure("sdl_update_window_surface", sdl_update_window_surface_wrapper,
                 1, false);
  rt_define("sdl-update-window-surface", rt_pop());

  rt_new_closure("sdl_delay", sdl_delay_wrapper, 1, false);
  rt_define("sdl-delay", rt_pop());

  rt_new_closure("sdl_poll_event", sdl_poll_event_wrapper, 0, false);
  rt_define("sdl-poll-event", rt_pop());

  rt_new_closure("sdl_destroy_renderer", sdl_destroy_renderer_wrapper, 1,
                 false);
  rt_define("sdl-destroy-renderer", rt_pop());

  rt_new_closure("sdl_destroy_window", sdl_destroy_window_wrapper, 1, false);
  rt_define("sdl-destroy-window", rt_pop());

  rt_new_closure("sdl_quit", sdl_quit_wrapper, 0, false);
  rt_define("sdl-quit", rt_pop());

  return 0;
}
