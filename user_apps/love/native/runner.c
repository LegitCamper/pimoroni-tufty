#include "runner.h"
#include "lua.h"
#include "lauxlib.h"
#include "lualib.h"
#include <math.h>
#include <stdbool.h>
#include <string.h>
#ifdef __arm__
#include <sys/reent.h>
#endif

#define WIDTH 320
#define HEIGHT 240
#define HEAP_BYTES (512 * 1024)
#define SOURCE_BYTES (64 * 1024)
#define ALIGN_BYTES 16

typedef union Block Block;
typedef struct Pico Pico;
union Block {
    struct { size_t size; Block *next; bool free; } data;
    max_align_t alignment;
};

typedef struct {
#ifdef __arm__
    struct _reent libc;
#endif
    lua_State *lua;
    Pico *pico;
    const uint8_t *assets;
    Block *blocks;
    const char *source;
    size_t length;
    TuftyLoveText text;
    uint32_t buttons, previous;
    float delta, elapsed;
    unsigned instructions, pixels_left;
    bool quit, failed;
    uint16_t color, background;
    char error[192];
    uint16_t pixels[WIDTH * HEIGHT];
    _Alignas(ALIGN_BYTES) unsigned char heap[HEAP_BYTES];
} Session;

_Static_assert(_Alignof(Session) <= ALIGN_BYTES, "session alignment");
_Static_assert(sizeof(Block) % ALIGN_BYTES == 0, "block alignment");

static Session *session(lua_State *state) {
    return *(Session **)lua_getextraspace(state);
}

static void *allocate(void *context, void *pointer, size_t old_size, size_t size) {
    Session *run = context;
    Block *old = pointer ? (Block *)pointer - 1 : NULL;
    if (size > HEAP_BYTES) return NULL;
    size = (size + ALIGN_BYTES - 1) & ~(size_t)(ALIGN_BYTES - 1);
    if (old && size && size <= old->data.size) return pointer;
    void *result = NULL;
    if (size) {
        for (Block *block = run->blocks; block; block = block->data.next) {
            if (!block->data.free || block->data.size < size) continue;
            if (block->data.size >= size + sizeof(Block) + ALIGN_BYTES) {
                Block *tail = (Block *)((char *)(block + 1) + size);
                tail->data.size = block->data.size - size - sizeof(Block);
                tail->data.next = block->data.next;
                tail->data.free = true;
                block->data.size = size;
                block->data.next = tail;
            }
            block->data.free = false;
            result = block + 1;
            if (old) memcpy(result, pointer, old_size < size ? old_size : size);
            break;
        }
        if (!result) return NULL;
    }
    if (old) {
        old->data.free = true;
        for (Block *block = run->blocks; block && block->data.next;) {
            Block *next = block->data.next;
            if (block->data.free && next->data.free) {
                block->data.size += sizeof(Block) + next->data.size;
                block->data.next = next->data.next;
            } else block = next;
        }
    }
    return result;
}

#ifdef __arm__
static Session *active;

void *_realloc_r(struct _reent *context, void *pointer, size_t size) {
    if (!active || context != &active->libc) return NULL;
    size_t previous = pointer ? ((Block *)pointer - 1)->data.size : 0;
    void *result = allocate(active, pointer, previous, size);
    if (size && !result) luaL_error(active->lua, "Lua libc heap exhausted");
    return result;
}

void *_malloc_r(struct _reent *context, size_t size) { return _realloc_r(context, NULL, size); }
void _free_r(struct _reent *context, void *pointer) { (void)_realloc_r(context, pointer, 0); }
void *_calloc_r(struct _reent *context, size_t count, size_t size) {
    if (size && count > SIZE_MAX / size) {
        if (active) luaL_error(active->lua, "Lua libc allocation overflow");
        return NULL;
    }
    void *result = _malloc_r(context, count * size);
    if (result) memset(result, 0, count * size);
    return result;
}
#endif

static void hook(lua_State *state, lua_Debug *debug) {
    (void)debug;
    if (++session(state)->instructions >= 200) luaL_error(state, "Lua instruction budget exceeded");
}

static void arity(lua_State *state, int minimum, int maximum) {
    int count = lua_gettop(state);
    if (count < minimum || count > maximum) luaL_error(state, "Unsupported argument count");
}

static float number(lua_State *state, int index, float minimum, float maximum) {
    lua_Number value = luaL_checknumber(state, index);
    if (!isfinite(value) || value < minimum || value > maximum)
        luaL_argerror(state, index, "finite number outside supported range");
    return value;
}

static uint16_t color(lua_State *state) {
    arity(state, 3, 4);
    unsigned red = number(state, 1, 0, 1) * 31 + 0.5f;
    unsigned green = number(state, 2, 0, 1) * 63 + 0.5f;
    unsigned blue = number(state, 3, 0, 1) * 31 + 0.5f;
    if (lua_gettop(state) == 4 && number(state, 4, 0, 1) != 1)
        luaL_error(state, "Alpha blending unsupported; alpha must be 1");
    return (uint16_t)((red << 11) | (green << 5) | blue);
}

static void spend(lua_State *state, unsigned amount) {
    Session *run = session(state);
    if (amount > run->pixels_left) luaL_error(state, "Drawing budget exceeded");
    run->pixels_left -= amount;
}

static void pixel(Session *run, int column, int row) {
    if (column >= 0 && column < WIDTH && row >= 0 && row < HEIGHT)
        run->pixels[row * WIDTH + column] = run->color;
}

static int set_color(lua_State *state) { session(state)->color = color(state); return 0; }
static int set_background(lua_State *state) { session(state)->background = color(state); return 0; }
static int push_color(lua_State *state, uint16_t value) {
    arity(state, 0, 0);
    lua_pushnumber(state, (value >> 11) / 31.0f);
    lua_pushnumber(state, ((value >> 5) & 63) / 63.0f);
    lua_pushnumber(state, (value & 31) / 31.0f);
    lua_pushnumber(state, 1);
    return 4;
}
static int get_color(lua_State *state) { return push_color(state, session(state)->color); }
static int get_background(lua_State *state) { return push_color(state, session(state)->background); }

static int clear(lua_State *state) {
    uint16_t value = lua_gettop(state) ? color(state) : session(state)->background;
    spend(state, WIDTH * HEIGHT);
    for (size_t index = 0; index < WIDTH * HEIGHT; ++index) session(state)->pixels[index] = value;
    return 0;
}

static bool fill_mode(lua_State *state) {
    size_t length;
    const char *mode = luaL_checklstring(state, 1, &length);
    if (length == 4 && !memcmp(mode, "fill", 4)) return true;
    if (length == 4 && !memcmp(mode, "line", 4)) return false;
    luaL_argerror(state, 1, "mode must be fill or line");
    return false;
}

static int rectangle(lua_State *state) {
    arity(state, 5, 5);
    bool fill = fill_mode(state);
    int left = number(state, 2, -4096, 4096), top = number(state, 3, -4096, 4096);
    int width = number(state, 4, 0, 4096), height = number(state, 5, 0, 4096);
    int right = left + width, bottom = top + height;
    int first_column = left < 0 ? 0 : left, last_column = right > WIDTH ? WIDTH : right;
    int first_row = top < 0 ? 0 : top, last_row = bottom > HEIGHT ? HEIGHT : bottom;
    if (first_column >= last_column || first_row >= last_row) return 0;
    spend(state, (last_column - first_column) * (last_row - first_row));
    for (int row = first_row; row < last_row; ++row)
        for (int column = first_column; column < last_column; ++column)
            if (fill || column == left || column == right - 1 || row == top || row == bottom - 1)
                pixel(session(state), column, row);
    return 0;
}

static int circle(lua_State *state) {
    arity(state, 4, 4);
    bool fill = fill_mode(state);
    int center_x = number(state, 2, -4096, 4096), center_y = number(state, 3, -4096, 4096);
    int radius = number(state, 4, 0, 512);
    spend(state, WIDTH * HEIGHT);
    int inner = radius ? (radius - 1) * (radius - 1) : -1;
    for (int row = 0; row < HEIGHT; ++row) for (int column = 0; column < WIDTH; ++column) {
        int horizontal = column - center_x, vertical = row - center_y;
        int distance = horizontal * horizontal + vertical * vertical;
        if (distance <= radius * radius && (fill || distance > inner)) pixel(session(state), column, row);
    }
    return 0;
}

static int line(lua_State *state) {
    arity(state, 4, 4);
    int start_x = number(state, 1, -4096, 4096), start_y = number(state, 2, -4096, 4096);
    int end_x = number(state, 3, -4096, 4096), end_y = number(state, 4, -4096, 4096);
    int distance_x = end_x - start_x, distance_y = end_y - start_y;
    int steps_x = distance_x < 0 ? -distance_x : distance_x;
    int steps_y = distance_y < 0 ? -distance_y : distance_y;
    int steps = steps_x > steps_y ? steps_x : steps_y;
    spend(state, steps + 1);
    pixel(session(state), start_x, start_y);
    for (int step = 1; step <= steps; ++step)
        pixel(session(state), start_x + distance_x * step / steps, start_y + distance_y * step / steps);
    return 0;
}

static int print_text(lua_State *state) {
    arity(state, 1, 3);
    size_t length;
    const char *text = luaL_checklstring(state, 1, &length);
    if (length > 128) return luaL_error(state, "Text exceeds 128 bytes");
    for (size_t index = 0; index < length; ++index)
        if ((text[index] < 32 || text[index] > 126) && text[index] != '\n')
            return luaL_error(state, "Text must be printable ASCII or newline");
    int left = lua_gettop(state) >= 2 ? number(state, 2, -4096, 4096) : 0;
    int top = lua_gettop(state) >= 3 ? number(state, 3, -4096, 4096) : 0;
    spend(state, length * 60);
    Session *run = session(state);
    run->text(run->pixels, text, length, left, top, run->color);
    return 0;
}

static int dimensions(lua_State *state) { arity(state, 0, 0); lua_pushinteger(state, WIDTH); lua_pushinteger(state, HEIGHT); return 2; }
static int width(lua_State *state) { arity(state, 0, 0); lua_pushinteger(state, WIDTH); return 1; }
static int height(lua_State *state) { arity(state, 0, 0); lua_pushinteger(state, HEIGHT); return 1; }
static int get_time(lua_State *state) { arity(state, 0, 0); lua_pushnumber(state, session(state)->elapsed); return 1; }
static int get_delta(lua_State *state) { arity(state, 0, 0); lua_pushnumber(state, session(state)->delta); return 1; }
static int get_fps(lua_State *state) { arity(state, 0, 0); float delta = session(state)->delta; lua_pushinteger(state, delta > 0 ? (int)(1 / delta) : 0); return 1; }
static int quit(lua_State *state) { arity(state, 0, 0); session(state)->quit = true; return 0; }

static const char *keys[] = {"up", "down", "left", "right", "z", "x", "c", "a", "s", "return", "backspace"};
static int is_down(lua_State *state) {
    arity(state, 1, 16);
    bool pressed = false;
    for (int argument = 1; argument <= lua_gettop(state); ++argument) {
        size_t length;
        const char *key = luaL_checklstring(state, argument, &length);
        bool found = false;
        for (unsigned index = 0; index < sizeof(keys) / sizeof(*keys); ++index)
            if (strlen(keys[index]) == length && !memcmp(keys[index], key, length)) {
                pressed |= (session(state)->buttons & (1u << index)) != 0;
                found = true;
            }
        if (!found) return luaL_error(state, "Unsupported key: %s", key);
    }
    lua_pushboolean(state, pressed);
    return 1;
}

static int unsupported(lua_State *state) {
    return luaL_error(state, "Unsupported Tufty LOVE API: %s", luaL_checkstring(state, 2));
}

static void module(lua_State *state, const char *name, const luaL_Reg *functions) {
    lua_newtable(state);
    luaL_setfuncs(state, functions, 0);
    lua_newtable(state);
    lua_pushcfunction(state, unsupported);
    lua_setfield(state, -2, "__index");
    lua_setmetatable(state, -2);
    lua_setfield(state, -2, name);
}

static bool callback(lua_State *state, const char *name) {
    lua_getglobal(state, "love");
    if (!lua_istable(state, -1)) luaL_error(state, "love must remain a table");
    lua_pushstring(state, name);
    lua_rawget(state, -2);
    lua_remove(state, -2);
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return false; }
    if (!lua_isfunction(state, -1)) luaL_error(state, "love.%s must be a function", name);
    return true;
}

static void open_libraries(lua_State *state) {
    luaL_requiref(state, "_G", luaopen_base, 1); lua_pop(state, 1);
    luaL_requiref(state, "math", luaopen_math, 1); lua_pop(state, 1);
    luaL_requiref(state, "string", luaopen_string, 1); lua_pop(state, 1);
    luaL_requiref(state, "table", luaopen_table, 1); lua_pop(state, 1);
}

#include "pico8.inc"

static int setup(lua_State *state) {
    open_libraries(state);
    if (session(state)->assets) return p8_setup(state);
    lua_newtable(state);
    static const luaL_Reg graphics[] = {
        {"setColor", set_color}, {"getColor", get_color}, {"clear", clear},
        {"setBackgroundColor", set_background}, {"getBackgroundColor", get_background},
        {"rectangle", rectangle}, {"circle", circle}, {"line", line}, {"print", print_text},
        {"getDimensions", dimensions}, {"getWidth", width}, {"getHeight", height}, {NULL, NULL}
    };
    static const luaL_Reg keyboard[] = {{"isDown", is_down}, {NULL, NULL}};
    static const luaL_Reg timer[] = {{"getTime", get_time}, {"getDelta", get_delta}, {"getFPS", get_fps}, {NULL, NULL}};
    static const luaL_Reg event[] = {{"quit", quit}, {NULL, NULL}};
    module(state, "graphics", graphics); module(state, "keyboard", keyboard);
    module(state, "timer", timer); module(state, "event", event);
    lua_newtable(state); lua_pushcfunction(state, unsupported); lua_setfield(state, -2, "__index");
    lua_setmetatable(state, -2);
    lua_setglobal(state, "love");
    Session *run = session(state);
    if (luaL_loadbufferx(state, run->source, run->length, "game.lua", "t") != LUA_OK) return lua_error(state);
    lua_call(state, 0, 0);
    if (callback(state, "load")) lua_call(state, 0, 0);
    return 0;
}

static int frame(lua_State *state) {
    Session *run = session(state);
    if (run->pico) return p8_frame(state);
    for (unsigned index = 0; index < sizeof(keys) / sizeof(*keys); ++index) {
        unsigned bit = 1u << index;
        if (!((run->buttons ^ run->previous) & bit)) continue;
        bool pressed = (run->buttons & bit) != 0;
        if (callback(state, pressed ? "keypressed" : "keyreleased")) {
            lua_pushstring(state, keys[index]); lua_pushstring(state, keys[index]);
            if (pressed) lua_pushboolean(state, false);
            lua_call(state, pressed ? 3 : 2, 0);
        }
    }
    if (run->quit) return 0;
    if (callback(state, "update")) { lua_pushnumber(state, run->delta); lua_call(state, 1, 0); }
    if (run->quit) return 0;
    for (size_t index = 0; index < WIDTH * HEIGHT; ++index) run->pixels[index] = run->background;
    if (callback(state, "draw")) lua_call(state, 0, 0);
    return 0;
}

static int protected_call(Session *run, lua_CFunction function) {
    run->instructions = 0;
    run->pixels_left = 1000000;
    lua_pushcfunction(run->lua, function);
#ifdef __arm__
    struct _reent *previous = _impure_ptr;
    active = run;
    _impure_ptr = &run->libc;
#endif
    int result = lua_pcall(run->lua, 0, 0, 0);
#ifdef __arm__
    _impure_ptr = previous;
    active = NULL;
#endif
    if (result == LUA_OK) return run->quit ? 1 : 0;
    const char *message = lua_type(run->lua, -1) == LUA_TSTRING ? lua_tostring(run->lua, -1) : "Lua callback failed";
    size_t length = strlen(message);
    if (length >= sizeof(run->error)) length = sizeof(run->error) - 1;
    memcpy(run->error, message, length); run->error[length] = 0;
    lua_settop(run->lua, 0);
    run->failed = true;
    return -1;
}

size_t tufty_love_size(void) { return sizeof(Session); }
const char *tufty_love_error(void *memory) { return ((Session *)memory)->error; }
const uint16_t *tufty_love_pixels(void *memory) { return ((Session *)memory)->pixels; }

static int init(void *memory, const char *source, size_t length, TuftyLoveText text, const uint8_t *assets) {
    Session *run = memory;
    memset(run, 0, sizeof(*run));
#ifdef __arm__
    _REENT_INIT_PTR_ZEROED(&run->libc);
#endif
    if (!source || !text || !length || length > SOURCE_BYTES || memchr(source, 0, length)) {
        strcpy(run->error, "Script must be 1..65536 bytes of text without NUL");
        run->failed = true; return -1;
    }
    run->source = source; run->length = length; run->text = text;
    run->assets = assets;
    run->color = 0xffff;
    run->blocks = (Block *)run->heap;
    run->blocks->data.size = HEAP_BYTES - sizeof(Block);
    run->blocks->data.free = true;
    run->lua = lua_newstate(allocate, run);
    if (!run->lua) { strcpy(run->error, "Lua heap unavailable"); run->failed = true; return -1; }
    *(Session **)lua_getextraspace(run->lua) = run;
    lua_sethook(run->lua, hook, LUA_MASKCOUNT, 1000);
    return protected_call(run, setup);
}

int tufty_love_init(void *memory, const char *source, size_t length, TuftyLoveText text) {
    return init(memory, source, length, text, NULL);
}

int tufty_pico8_init(void *memory, const char *source, size_t length, TuftyLoveText text,
                    const uint8_t *assets, size_t asset_length) {
    if (!assets || asset_length != 0x3100) {
        Session *run = memory;
        memset(run, 0, sizeof(*run));
        strcpy(run->error, "Wrong P8 asset buffer size");
        run->failed = true;
        return -1;
    }
    return init(memory, source, length, text, assets);
}

unsigned tufty_love_rate(void *memory) {
    Session *run = memory;
    return run->pico ? run->pico->rate : 30;
}

int tufty_love_tick(void *memory, uint32_t buttons, float delta) {
    Session *run = memory;
    if (run->failed || !run->lua) return -1;
    if (!isfinite(delta) || delta < 0.001f || delta > 0.1f || (buttons & ~0xfffu)) {
        strcpy(run->error, "Invalid frame input"); run->failed = true; return -1;
    }
    if (run->quit || (buttons & (1u << 11))) return 1;
    run->previous = run->buttons; run->buttons = buttons;
    run->delta = delta; run->elapsed += delta;
    return protected_call(run, frame);
}
