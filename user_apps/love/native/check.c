#include "runner.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void pico8_checks(void *state, const char *demo_path);

static unsigned text_calls;
static void text(uint16_t *pixels, const char *value, size_t length, int left, int top, uint16_t color) {
    assert(pixels && value && length <= 128);
    assert(left >= -4096 && left <= 4096 && top >= -4096 && top <= 4096);
    (void)color;
    ++text_calls;
}

static int start(void *state, const char *source) {
    return tufty_love_init(state, source, strlen(source), text);
}

static void rejects(void *state, const char *source, const char *expected) {
    int result = start(state, source);
    if (result == 0) result = tufty_love_tick(state, 0, 0.033f);
    if (result != -1 || !strstr(tufty_love_error(state), expected)) {
        fprintf(stderr, "Expected %s; got %d: %s\n", expected, result, tufty_love_error(state));
        abort();
    }
    assert(tufty_love_tick(state, 0, 0.033f) == -1);
}

int main(int argc, char **argv) {
    assert(argc == 3);
    size_t size = tufty_love_size();
    assert(size < 1024 * 1024 && size % 16 == 0);
    unsigned char *allocation = aligned_alloc(16, size + 32);
    assert(allocation);
    memset(allocation, 0xa5, size + 32);
    void *state = allocation + 16;
    const char *source =
        "assert(io == nil and os == nil and debug == nil and package == nil and require == nil) "
        "assert(load == nil and loadfile == nil and dofile == nil and coroutine == nil) "
        "assert(pcall == nil and xpcall == nil and setmetatable == nil and getmetatable == nil) "
        "assert(string.find == nil and string.match == nil and string.gsub == nil and string.gmatch == nil) "
        "assert(table.move == nil) "
        "local loaded, pressed, released = false, false, false "
        "function love.load() loaded = true end "
        "function love.keypressed(key, scan, repeated) assert(key == 'z' and scan == key and not repeated); pressed = true end "
        "function love.keyreleased(key, scan) assert(key == 'z' and scan == key); released = true end "
        "function love.update(dt) assert(loaded and dt > 0 and love.timer.getTime() > 0); "
        "assert(love.keyboard.isDown('z') == (pressed and not released)) end "
        "function love.draw() local width, height = love.graphics.getDimensions(); assert(width == 320 and height == 240); "
        "love.graphics.setColor(1,0,0); love.graphics.rectangle('fill',-3,-3,5,5); "
        "love.graphics.setColor(0,1,0); love.graphics.circle('fill',20,20,2); "
        "love.graphics.setColor(0,0,1); love.graphics.line(30,30,35,30); love.graphics.print('test',1,1) end";
    for (unsigned iteration = 0; iteration < 4; ++iteration) {
        assert(start(state, source) == 0);
        assert(tufty_love_tick(state, 1 << 4, 0.033f) == 0);
        const uint16_t *pixels = tufty_love_pixels(state);
        assert(pixels[0] == 0xf800 && pixels[1] == 0xf800 && pixels[2] == 0);
        assert(pixels[20 * 320 + 20] == 0x07e0 && pixels[30 * 320 + 35] == 0x001f);
        assert(tufty_love_tick(state, 0, 0.033f) == 0);
        assert(tufty_love_tick(state, 1 << 11, 0.033f) == 1);
    }
    rejects(state, "while true do end", "budget");
    rejects(state, "function love.update() while true do end end", "budget");
    rejects(state, "local data = {}; for index=1,10000 do data[index]=string.rep('x',1024) end", "memory");
    rejects(state, "function love.draw() for index=1,100 do love.graphics.clear() end end", "Drawing budget");
    rejects(state, "love.graphics.newImage('test.png')", "Unsupported Tufty LOVE API");
    rejects(state, "love.audio.play()", "Unsupported Tufty LOVE API");
    rejects(state, "love.graphics.setColor(1,1,1,0.5)", "Alpha blending");
    rejects(state, "love.graphics.rectangle('fill',0/0,0,1,1)", "finite number");
    rejects(state, "love.graphics.circle('fill',1,1,1/0)", "finite number");
    rejects(state, "love.graphics.rectangle('wrong',0,0,1,1)", "mode must");
    rejects(state, "love.graphics.rectangle('fill\\0tail',0,0,1,1)", "mode must");
    rejects(state, "love.graphics.print(string.rep('a',129))", "128 bytes");
    rejects(state, "love.keyboard.isDown('mouse1')", "Unsupported key");
    rejects(state, "string.rep('',2147483647)", "repeat count exceeds badge limit");
    rejects(state, "love.graphics.print('\\255')", "ASCII");
    rejects(state, "love.update = 42", "must be a function");
    rejects(state, "love = false", "must remain a table");
    rejects(state, "local function recurse() return 1 + recurse() end recurse()", "memory");
    rejects(state, "function !", "expected");
    assert(tufty_love_init(state, "a\0b", 3, text) == -1);
    assert(tufty_love_init(state, "\x1bLua", 4, text) == -1);
    assert(start(state, "function love.update() love.event.quit() end") == 0);
    assert(tufty_love_tick(state, 0, 0.033f) == 1);
    assert(start(state, "function love.draw() end") == 0);
    assert(tufty_love_tick(state, 0, NAN) == -1);
    assert(start(state, "function love.draw() end") == 0);
    assert(tufty_love_tick(state, 1u << 31, 0.033f) == -1);
    assert(start(state, "assert(string.format('%.2f', 1.25) == '1.25'); assert(tonumber('1.25') == 1.25)") == 0);
    assert(start(state, "function love.update() local data={} for index=1,200 do data[index]=string.rep('x',100) end end") == 0);
    for (unsigned frame = 0; frame < 100; ++frame) assert(tufty_love_tick(state, 0, 0.033f) == 0);
    FILE *demo = fopen(argv[1], "rb");
    assert(demo);
    char buffer[16384];
    size_t length = fread(buffer, 1, sizeof(buffer), demo);
    assert(length && length < sizeof(buffer));
    fclose(demo);
    assert(tufty_love_init(state, buffer, length, text) == 0);
    for (unsigned frame = 0; frame < 300; ++frame) {
        unsigned buttons = frame % 60 == 0 ? 1 << 4 : (frame % 40 < 20 ? 1 : 2);
        assert(tufty_love_tick(state, buttons, 0.033f) == 0);
    }
    assert(text_calls > 100);
    pico8_checks(state, argv[2]);
    for (unsigned index = 0; index < 16; ++index) {
        assert(allocation[index] == 0xa5);
        assert(allocation[size + 16 + index] == 0xa5);
    }
    free(allocation);
    puts("Lua callbacks, input, pixels, limits, errors, GC, demo and repeated sessions passed");
    return 0;
}
