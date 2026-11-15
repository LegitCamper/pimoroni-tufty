#include "runner.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static unsigned text_calls;
static uint8_t assets[0x3100];

static void text(uint16_t *pixels, const char *value, size_t length, int left, int top, uint16_t color) {
    assert(pixels && value && length == 1 && left == 0 && top == 0 && color == 1);
    pixels[0] = color;
    pixels[5 * 320 + 2] = color;
    ++text_calls;
}

static int start(void *state, const char *source) {
    return tufty_pico8_init(state, source, strlen(source), text, assets, sizeof(assets));
}

static void rejects(void *state, const char *source, const char *expected) {
    int result = start(state, source);
    if (result == 0) result = tufty_love_tick(state, 0, 0.033f);
    if (result != -1 || !strstr(tufty_love_error(state), expected)) {
        fprintf(stderr, "P8 expected %s; got %d: %s\n", expected, result, tufty_love_error(state));
        abort();
    }
    assert(tufty_love_tick(state, 0, 0.033f) == -1);
}

void pico8_checks(void *state, const char *demo_path) {
    assets[4] = 0x98;
    assets[0x2000] = 1;
    assets[0x3001] = 4;
    const char *source =
        "assert(love==nil and io==nil and require==nil and pcall==nil) "
        "assert(sget(8,0)==8 and sget(9,0)==9 and mget(0,0)==1 and fget(1,2)) "
        "assert(flr(1.9)==1 and ceil(1.1)==2 and mid(1,8,3)==3 and sgn(-2)==-1) "
        "assert(abs(sin(0.25)+1)<0.001 and abs(cos(0)-1)<0.001) "
        "assert(abs(atan2(0,-1)-0.25)<0.001) "
        "local values={} add(values,1) add(values,2) assert(count(values)==2) "
        "foreach(values,function(value) del(values,value) end) assert(#values==0) "
        "srand(2350) assert(rnd()>=0 and rnd(2)<2 and rnd({8})==8) "
        "assert(sub('abc',2)=='bc' and tonum('0xf')==15 and tostr(8)=='8') "
        "cls() spr(1,0,0) assert(pget(0,0)==8 and pget(1,0)==9) "
        "spr(1,8,0,1,1,true,false) assert(pget(15,0)==8 and pget(14,0)==9) "
        "spr(1,16,0,1,1,false,true) assert(pget(16,7)==8) "
        "sspr(8,0,2,1,0,8,4,2) assert(pget(1,9)==8 and pget(3,9)==9) "
        "cls(3) spr(0,0,0) assert(pget(0,0)==3) palt(0,false) spr(0,0,0) assert(pget(0,0)==0) palt() "
        "cls() pal(8,12) spr(1,0,0) assert(pget(0,0)==12) pal() "
        "camera(10,10) clip(0,0,1,1) rectfill(10,10,12,12,11) "
        "assert(pget(0,0)==11 and pget(1,0)==9) camera() clip() "
        "rect(20,20,22,22,8) assert(pget(20,20)==8 and pget(21,21)==0) "
        "line(20,20,22,22,9) assert(pget(21,21)==9) "
        "circfill(30,30,2,10) assert(pget(30,30)==10 and pget(32,30)==10) "
        "circ(40,40,2,11) assert(pget(40,40)==0 and pget(42,40)==11) "
        "cls() map(0,0,0,0,1,1,4) assert(pget(0,0)==8) "
        "cls() map(0,0,0,0,1,1,2) assert(pget(0,0)==0) "
        "mset(0,32,0xab) assert(sget(0,64)==11 and sget(1,64)==10) "
        "sset(0,64,4) assert(mget(0,32)==0xa4) "
        "fset(1,0,true) assert(fget(1)==5) fset(1,2,false) assert(fget(1)==1) fset(1,255) assert(fget(1)==255) "
        "cls() print('x',4,5,7) assert(pget(4,5)==7 and pget(6,10)==7) "
        "cursor(8,8) print('x',9) assert(pget(8,8)==9) "
        "sfx(0) music(-1) local initialized=false "
        "function _init() initialized=true end "
        "function _update() assert(initialized and time()>0 and t()==time()) end "
        "function _draw() cls(8) pal(8,12,1) end";
    for (unsigned iteration = 0; iteration < 4; ++iteration) {
        int result = start(state, source);
        if (result) fprintf(stderr, "P8 API: %s\n", tufty_love_error(state));
        assert(result == 0 && tufty_love_rate(state) == 30);
        assert(tufty_love_tick(state, 0, 0.033f) == 0);
        const uint16_t *pixels = tufty_love_pixels(state);
        assert(pixels[39] == 0 && pixels[40] == 0x2d7f && pixels[279] == 0x2d7f && pixels[280] == 0);
        assert(pixels[239 * 320 + 40] == 0x2d7f);
    }
    const char *buttons =
        "local frame=0 function _update() frame=frame+1 "
        "if frame==1 then assert(btn()==63 and btnp()==63) "
        "elseif frame==2 then assert(btn()==0 and btnp()==0) "
        "else local held=frame-2 assert(btn(4) and not btn(5) and not btn(4,1)) "
        "assert(btnp(4)==(held==1 or (held>15 and (held-16)%4==0))) end end";
    assert(start(state, buttons) == 0);
    assert(tufty_love_tick(state, 63, 0.033f) == 0);
    assert(tufty_love_tick(state, 0, 0.033f) == 0);
    for (unsigned frame = 0; frame < 150; ++frame) assert(tufty_love_tick(state, 1 << 4, 0.033f) == 0);
    assert(start(state, "function _update() error('wrong callback') end local held=0 "
        "function _update60() held=held+1 assert(btnp(4)==(held==1 or (held>30 and (held-31)%8==0))) end") == 0);
    assert(tufty_love_rate(state) == 60);
    for (unsigned frame = 0; frame < 150; ++frame) assert(tufty_love_tick(state, 1 << 4, 0.016f) == 0);
    static const unsigned physical[] = {2, 3, 0, 1, 4, 5};
    for (unsigned index = 0; index < 6; ++index) {
        char script[128];
        snprintf(script, sizeof(script), "function _update() assert(btn()==%u and btnp()==%u) end", 1u << index, 1u << index);
        assert(start(state, script) == 0);
        assert(tufty_love_tick(state, 1u << physical[index], 0.033f) == 0);
    }
    rejects(state, "while true do end", "budget");
    rejects(state, "function _update() while true do end end", "budget");
    rejects(state, "function _draw() for index=1,100 do cls() end end", "Drawing budget");
    rejects(state, "local data={} for index=1,10000 do data[index]=string.rep('x',1024) end", "memory");
    rejects(state, "pal(16,0)", "range");
    rejects(state, "mget(128,0)", "range");
    rejects(state, "fset(1,8,true)", "range");
    rejects(state, "btn(6)", "range");
    rejects(state, "circ(0/0,0)", "finite");
    rejects(state, "spr(0,0,0,1,1,1)", "boolean");
    rejects(state, "print(string.rep('x',129))", "128 bytes");
    rejects(state, "print('\\255')", "ASCII");
    rejects(state, "_draw=1", "must be a function");
    rejects(state, "function _init() error('cart failure') end", "cart failure");
    assert(tufty_pico8_init(state, "cls()", 5, text, assets, sizeof(assets)-1) == -1);
    assert(tufty_pico8_init(state, "cls()", 5, text, NULL, sizeof(assets)) == -1);
    assert(start(state, "function _update() stop() end function _draw() error('draw after stop') end") == 0);
    assert(tufty_love_tick(state, 0, 0.033f) == 1);
    assert(start(state, "cls()") == 0);
    assert(tufty_love_tick(state, 1u << 11, 0.033f) == 1);
    FILE *demo = fopen(demo_path, "rb");
    assert(demo);
    char buffer[16384];
    size_t length = fread(buffer, 1, sizeof(buffer)-1, demo);
    assert(length && length < sizeof(buffer)-1);
    fclose(demo);
    buffer[length] = 0;
    assert(start(state, buffer) == 0);
    for (unsigned frame = 0; frame < 600; ++frame) {
        unsigned buttons = frame % 120 == 0 ? 1u << 5 : frame % 40 == 1 ? 1u << 4 : frame % 20 < 10 ? 1 : 2;
        int result = tufty_love_tick(state, buttons, 0.033f);
        if (result) fprintf(stderr, "P8 demo frame %u: %s\n", frame, tufty_love_error(state));
        assert(result == 0);
    }
    assert(text_calls > 100);
    puts("P8 API, palettes, assets, input repeat, bounds, demo and repeated sessions passed");
}
