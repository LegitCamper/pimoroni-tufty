struct Pico {
    uint8_t memory[0x3100], screen[128 * 128];
    uint8_t draw_palette[16], display_palette[16], pen, buttons, pressed;
    uint16_t transparent;
    uint32_t held[6];
    unsigned rate;
    int camera_x, camera_y, clip_x, clip_y, clip_right, clip_bottom, cursor_x, cursor_y;
};

static Pico *p8(lua_State *state) { return session(state)->pico; }
static int p8_integer(lua_State *state, int index, int minimum, int maximum) {
    lua_Integer value = luaL_checkinteger(state, index);
    if (value < minimum || value > maximum) luaL_argerror(state, index, "P8 integer outside supported range");
    return (int)value;
}
static int p8_coordinate(lua_State *state, int index) { return (int)floorf(number(state, index, -4096, 4096)); }
static uint8_t p8_pen(lua_State *state, int index) {
    if (!lua_isnoneornil(state, index)) p8(state)->pen = p8_integer(state, index, 0, 15);
    return p8(state)->pen;
}
static bool p8_boolean(lua_State *state, int index) {
    if (lua_isnoneornil(state, index)) return false;
    luaL_checktype(state, index, LUA_TBOOLEAN);
    return lua_toboolean(state, index);
}
static void p8_pixel(Pico *cart, int column, int row, uint8_t color) {
    column -= cart->camera_x; row -= cart->camera_y;
    if (column >= cart->clip_x && column < cart->clip_right && row >= cart->clip_y && row < cart->clip_bottom)
        cart->screen[row * 128 + column] = cart->draw_palette[color];
}
static int p8_cls(lua_State *state) {
    arity(state, 0, 1);
    unsigned color = lua_isnoneornil(state, 1) ? 0 : p8_integer(state, 1, 0, 15);
    spend(state, 128 * 128);
    memset(p8(state)->screen, p8(state)->draw_palette[color], sizeof(p8(state)->screen));
    p8(state)->cursor_x = p8(state)->cursor_y = 0;
    return 0;
}
static int p8_color(lua_State *state) {
    arity(state, 0, 1);
    unsigned old = p8(state)->pen;
    p8(state)->pen = lua_isnoneornil(state, 1) ? 6 : p8_integer(state, 1, 0, 15);
    lua_pushinteger(state, old); return 1;
}
static int p8_pset(lua_State *state) {
    arity(state, 2, 3); spend(state, 1);
    int column = p8_coordinate(state, 1), row = p8_coordinate(state, 2);
    p8_pixel(p8(state), column, row, p8_pen(state, 3)); return 0;
}
static int p8_pget(lua_State *state) {
    arity(state, 2, 2);
    int column = p8_coordinate(state, 1), row = p8_coordinate(state, 2);
    lua_pushinteger(state, column >= 0 && column < 128 && row >= 0 && row < 128 ? p8(state)->screen[row * 128 + column] : 0);
    return 1;
}
static int p8_rectangle(lua_State *state) {
    arity(state, 4, 5);
    Pico *cart = p8(state);
    int left = p8_coordinate(state, 1), top = p8_coordinate(state, 2);
    int right = p8_coordinate(state, 3), bottom = p8_coordinate(state, 4);
    if (left > right) { int swap = left; left = right; right = swap; }
    if (top > bottom) { int swap = top; top = bottom; bottom = swap; }
    uint8_t color = p8_pen(state, 5);
    bool fill = lua_toboolean(state, lua_upvalueindex(1));
    spend(state, 128 * 128);
    for (int row = cart->clip_y; row < cart->clip_bottom; ++row)
        for (int column = cart->clip_x; column < cart->clip_right; ++column) {
            int world_x = column + cart->camera_x, world_y = row + cart->camera_y;
            if (world_x >= left && world_x <= right && world_y >= top && world_y <= bottom &&
                (fill || world_x == left || world_x == right || world_y == top || world_y == bottom))
                p8_pixel(cart, world_x, world_y, color);
        }
    return 0;
}
static int p8_circle(lua_State *state) {
    arity(state, 2, 4);
    Pico *cart = p8(state);
    int center_x = p8_coordinate(state, 1), center_y = p8_coordinate(state, 2);
    int radius = lua_isnoneornil(state, 3) ? 4 : (int)number(state, 3, 0, 512);
    uint8_t color = p8_pen(state, 4);
    bool fill = lua_toboolean(state, lua_upvalueindex(1));
    int inner = radius ? (radius - 1) * (radius - 1) : -1;
    spend(state, 128 * 128);
    for (int row = cart->clip_y; row < cart->clip_bottom; ++row)
        for (int column = cart->clip_x; column < cart->clip_right; ++column) {
            int horizontal = column + cart->camera_x - center_x, vertical = row + cart->camera_y - center_y;
            int distance = horizontal * horizontal + vertical * vertical;
            if (distance <= radius * radius && (fill || distance > inner))
                p8_pixel(cart, column + cart->camera_x, row + cart->camera_y, color);
        }
    return 0;
}
static int p8_line(lua_State *state) {
    arity(state, 4, 5);
    int left = p8_coordinate(state, 1), top = p8_coordinate(state, 2);
    int horizontal = p8_coordinate(state, 3) - left, vertical = p8_coordinate(state, 4) - top;
    int width = horizontal < 0 ? -horizontal : horizontal, height = vertical < 0 ? -vertical : vertical;
    int steps = width > height ? width : height;
    uint8_t color = p8_pen(state, 5);
    spend(state, steps + 1);
    p8_pixel(p8(state), left, top, color);
    for (int step = 1; step <= steps; ++step) p8_pixel(p8(state), left + horizontal * step / steps, top + vertical * step / steps, color);
    return 0;
}
static int p8_camera(lua_State *state) {
    arity(state, 0, 2);
    Pico *cart = p8(state);
    int old_x = cart->camera_x, old_y = cart->camera_y;
    cart->camera_x = lua_isnoneornil(state, 1) ? 0 : p8_coordinate(state, 1);
    cart->camera_y = lua_isnoneornil(state, 2) ? 0 : p8_coordinate(state, 2);
    lua_pushinteger(state, old_x); lua_pushinteger(state, old_y); return 2;
}
static int p8_clip(lua_State *state) {
    arity(state, 0, 4);
    Pico *cart = p8(state);
    if (!lua_gettop(state)) { cart->clip_x = cart->clip_y = 0; cart->clip_right = cart->clip_bottom = 128; return 0; }
    arity(state, 4, 4);
    int left = p8_coordinate(state, 1), top = p8_coordinate(state, 2);
    int right = left + p8_integer(state, 3, 0, 4096), bottom = top + p8_integer(state, 4, 0, 4096);
    cart->clip_x = left < 0 ? 0 : left > 128 ? 128 : left;
    cart->clip_y = top < 0 ? 0 : top > 128 ? 128 : top;
    cart->clip_right = right < 0 ? 0 : right > 128 ? 128 : right;
    cart->clip_bottom = bottom < 0 ? 0 : bottom > 128 ? 128 : bottom;
    return 0;
}
static void p8_reset_palette(Pico *cart) {
    for (unsigned index = 0; index < 16; ++index) cart->draw_palette[index] = cart->display_palette[index] = index;
    cart->transparent = 1;
}
static int p8_pal(lua_State *state) {
    arity(state, 0, 3);
    Pico *cart = p8(state);
    if (!lua_gettop(state)) { p8_reset_palette(cart); return 0; }
    arity(state, 2, 3);
    unsigned from = p8_integer(state, 1, 0, 15), to = p8_integer(state, 2, 0, 15);
    unsigned mode = lua_isnoneornil(state, 3) ? 0 : p8_integer(state, 3, 0, 1);
    uint8_t *palette = mode ? cart->display_palette : cart->draw_palette;
    unsigned previous = palette[from]; palette[from] = to;
    lua_pushinteger(state, previous); return 1;
}
static int p8_palt(lua_State *state) {
    arity(state, 0, 2);
    Pico *cart = p8(state);
    if (!lua_gettop(state)) { cart->transparent = 1; return 0; }
    arity(state, 2, 2);
    unsigned color = p8_integer(state, 1, 0, 15);
    bool transparent = p8_boolean(state, 2);
    lua_pushboolean(state, (cart->transparent & (1u << color)) != 0);
    if (transparent) cart->transparent |= 1u << color; else cart->transparent &= ~(1u << color);
    return 1;
}
static uint8_t p8_sprite_pixel(Pico *cart, int column, int row) {
    if (column < 0 || column >= 128 || row < 0 || row >= 128) return 0;
    uint8_t byte = cart->memory[row * 64 + column / 2];
    return (byte >> ((column & 1) * 4)) & 15;
}
static int p8_sget(lua_State *state) {
    arity(state, 2, 2);
    int column = p8_coordinate(state, 1), row = p8_coordinate(state, 2);
    lua_pushinteger(state, p8_sprite_pixel(p8(state), column, row)); return 1;
}
static int p8_sset(lua_State *state) {
    arity(state, 2, 3);
    int column = p8_coordinate(state, 1), row = p8_coordinate(state, 2);
    uint8_t color = p8_pen(state, 3);
    if (column >= 0 && column < 128 && row >= 0 && row < 128) {
        uint8_t *byte = &p8(state)->memory[row * 64 + column / 2];
        unsigned shift = (column & 1) * 4;
        *byte = (*byte & ~(15u << shift)) | color << shift;
    }
    return 0;
}
static void p8_blit(lua_State *state, int source_x, int source_y, int source_width, int source_height,
                    int left, int top, int width, int height, bool flip_x, bool flip_y) {
    spend(state, width * height);
    for (int row = 0; row < height; ++row) for (int column = 0; column < width; ++column) {
        int pixel_x = column * source_width / width, pixel_y = row * source_height / height;
        uint8_t color = p8_sprite_pixel(p8(state), source_x + (flip_x ? source_width - 1 - pixel_x : pixel_x),
                                      source_y + (flip_y ? source_height - 1 - pixel_y : pixel_y));
        if (!(p8(state)->transparent & (1u << color))) p8_pixel(p8(state), left + column, top + row, color);
    }
}
static int p8_spr(lua_State *state) {
    arity(state, 3, 7);
    int index = p8_integer(state, 1, 0, 255), left = p8_coordinate(state, 2), top = p8_coordinate(state, 3);
    int width = lua_isnoneornil(state, 4) ? 8 : (int)(number(state, 4, 0, 16) * 8);
    int height = lua_isnoneornil(state, 5) ? 8 : (int)(number(state, 5, 0, 16) * 8);
    p8_blit(state, index % 16 * 8, index / 16 * 8, width, height, left, top, width, height,
            p8_boolean(state, 6), p8_boolean(state, 7)); return 0;
}
static int p8_sspr(lua_State *state) {
    arity(state, 6, 10);
    int source_x = p8_integer(state, 1, 0, 127), source_y = p8_integer(state, 2, 0, 127);
    int source_width = p8_integer(state, 3, 0, 128), source_height = p8_integer(state, 4, 0, 128);
    int left = p8_coordinate(state, 5), top = p8_coordinate(state, 6);
    int width = lua_isnoneornil(state, 7) ? source_width : p8_integer(state, 7, 0, 128);
    int height = lua_isnoneornil(state, 8) ? source_height : p8_integer(state, 8, 0, 128);
    if (source_width && source_height) p8_blit(state, source_x, source_y, source_width, source_height,
        left, top, width, height, p8_boolean(state, 9), p8_boolean(state, 10));
    else { p8_boolean(state, 9); p8_boolean(state, 10); }
    return 0;
}
static unsigned p8_map_address(int column, int row) { return (row < 32 ? 0x2000 : 0x1000) + (row % 32) * 128 + column; }
static int p8_mget(lua_State *state) {
    arity(state, 2, 2);
    int column = p8_integer(state, 1, 0, 127), row = p8_integer(state, 2, 0, 63);
    lua_pushinteger(state, p8(state)->memory[p8_map_address(column, row)]); return 1;
}
static int p8_mset(lua_State *state) {
    arity(state, 3, 3);
    int column = p8_integer(state, 1, 0, 127), row = p8_integer(state, 2, 0, 63);
    p8(state)->memory[p8_map_address(column, row)] = p8_integer(state, 3, 0, 255); return 0;
}
static int p8_fget(lua_State *state) {
    arity(state, 1, 2);
    int index = p8_integer(state, 1, 0, 255);
    uint8_t flags = p8(state)->memory[0x3000 + index];
    if (lua_isnoneornil(state, 2)) lua_pushinteger(state, flags);
    else lua_pushboolean(state, (flags & (1u << p8_integer(state, 2, 0, 7))) != 0);
    return 1;
}
static int p8_fset(lua_State *state) {
    arity(state, 2, 3);
    int index = p8_integer(state, 1, 0, 255);
    uint8_t *flags = &p8(state)->memory[0x3000 + index];
    if (lua_gettop(state) == 2) *flags = p8_integer(state, 2, 0, 255);
    else {
        unsigned bit = p8_integer(state, 2, 0, 7);
        if (p8_boolean(state, 3)) *flags |= 1u << bit; else *flags &= ~(1u << bit);
    }
    return 0;
}
static int p8_map(lua_State *state) {
    arity(state, 4, 7);
    int column = p8_integer(state, 1, 0, 127), row = p8_integer(state, 2, 0, 63);
    int left = p8_coordinate(state, 3), top = p8_coordinate(state, 4);
    int width = lua_isnoneornil(state, 5) ? 128 : p8_integer(state, 5, 0, 128);
    int height = lua_isnoneornil(state, 6) ? 32 : p8_integer(state, 6, 0, 64);
    unsigned layer = lua_isnoneornil(state, 7) ? 0 : p8_integer(state, 7, 0, 255);
    for (int offset_y = 0; offset_y < height && row + offset_y < 64; ++offset_y)
        for (int offset_x = 0; offset_x < width && column + offset_x < 128; ++offset_x) {
            unsigned tile = p8(state)->memory[p8_map_address(column + offset_x, row + offset_y)];
            if (tile && (!layer || (p8(state)->memory[0x3000 + tile] & layer)))
                p8_blit(state, tile % 16 * 8, tile / 16 * 8, 8, 8, left + offset_x * 8, top + offset_y * 8, 8, 8, false, false);
        }
    return 0;
}
static int p8_btn(lua_State *state) {
    arity(state, 0, 2);
    unsigned mask = lua_toboolean(state, lua_upvalueindex(1)) ? p8(state)->pressed : p8(state)->buttons;
    if (!lua_isnoneornil(state, 2) && p8_integer(state, 2, 0, 7)) mask = 0;
    if (lua_isnoneornil(state, 1)) lua_pushinteger(state, mask);
    else lua_pushboolean(state, (mask & (1u << p8_integer(state, 1, 0, 5))) != 0);
    return 1;
}
static int p8_print(lua_State *state) {
    arity(state, 1, 4);
    Pico *cart = p8(state);
    int count = lua_gettop(state);
    int left = count < 3 ? cart->cursor_x : p8_coordinate(state, 2);
    int top = count < 3 ? cart->cursor_y : p8_coordinate(state, 3);
    uint8_t color = p8_pen(state, count == 2 ? 2 : 4);
    size_t length;
    const char *text = luaL_tolstring(state, 1, &length);
    if (length > 128) return luaL_error(state, "P8 print exceeds 128 bytes");
    spend(state, length * 24);
    int column = left, row = top;
    for (size_t index = 0; index < length; ++index) {
        if (text[index] == '\n') { row += 6; column = left; continue; }
        if (text[index] < 32 || text[index] > 126) return luaL_error(state, "P8 print supports ASCII only");
        Session *run = session(state);
        for (unsigned pixel_y = 0; pixel_y < 6; ++pixel_y) memset(run->pixels + pixel_y * WIDTH, 0, 4 * sizeof(uint16_t));
        run->text(run->pixels, text + index, 1, 0, 0, 1);
        for (int pixel_y = 0; pixel_y < 6; ++pixel_y) for (int pixel_x = 0; pixel_x < 4; ++pixel_x)
            if (run->pixels[pixel_y * WIDTH + pixel_x]) p8_pixel(cart, column + pixel_x, row + pixel_y, color);
        column += 4;
    }
    cart->cursor_x = left; cart->cursor_y = row + 6 > 4096 ? 4096 : row + 6;
    lua_pushinteger(state, column); return 1;
}
static int p8_cursor(lua_State *state) {
    arity(state, 2, 3);
    Pico *cart = p8(state);
    int old_x = cart->cursor_x, old_y = cart->cursor_y;
    cart->cursor_x = p8_coordinate(state, 1); cart->cursor_y = p8_coordinate(state, 2);
    p8_pen(state, 3); lua_pushinteger(state, old_x); lua_pushinteger(state, old_y); return 2;
}
static int p8_sfx(lua_State *state) {
    arity(state, 1, 4); p8_integer(state, 1, -2, 63);
    if (!lua_isnoneornil(state, 2)) p8_integer(state, 2, -2, 3);
    if (!lua_isnoneornil(state, 3)) p8_integer(state, 3, 0, 31);
    if (!lua_isnoneornil(state, 4)) p8_integer(state, 4, 0, 32);
    return 0;
}
static int p8_music(lua_State *state) {
    arity(state, 1, 3); p8_integer(state, 1, -1, 63);
    if (!lua_isnoneornil(state, 2)) number(state, 2, 0, 32767);
    if (!lua_isnoneornil(state, 3)) p8_integer(state, 3, 0, 15);
    return 0;
}
static bool p8_callback(lua_State *state, const char *name) {
    lua_getglobal(state, name);
    if (lua_isnil(state, -1)) { lua_pop(state, 1); return false; }
    if (!lua_isfunction(state, -1)) luaL_error(state, "%s must be a function", name);
    return true;
}
static void p8_closure(lua_State *state, const char *name, lua_CFunction function, bool flag) {
    lua_pushboolean(state, flag); lua_pushcclosure(state, function, 1); lua_setglobal(state, name);
}
static int p8_setup(lua_State *state) {
    Session *run = session(state);
    run->pico = allocate(run, NULL, 0, sizeof(Pico));
    if (!run->pico) return luaL_error(state, "P8 state allocation failed");
    memset(run->pico, 0, sizeof(Pico));
    memcpy(run->pico->memory, run->assets, 0x3100);
    run->pico->pen = 6; run->pico->rate = 30;
    run->pico->clip_right = run->pico->clip_bottom = 128;
    p8_reset_palette(run->pico);
    static const luaL_Reg functions[] = {
        {"cls", p8_cls}, {"color", p8_color}, {"pset", p8_pset}, {"pget", p8_pget},
        {"line", p8_line}, {"camera", p8_camera}, {"clip", p8_clip}, {"pal", p8_pal}, {"palt", p8_palt},
        {"spr", p8_spr}, {"sspr", p8_sspr}, {"sget", p8_sget}, {"sset", p8_sset},
        {"map", p8_map}, {"mget", p8_mget}, {"mset", p8_mset}, {"fget", p8_fget}, {"fset", p8_fset},
        {"print", p8_print}, {"cursor", p8_cursor}, {"time", get_time}, {"t", get_time},
        {"sfx", p8_sfx}, {"music", p8_music}, {"stop", quit}, {NULL, NULL}
    };
    lua_pushglobaltable(state); luaL_setfuncs(state, functions, 0); lua_pop(state, 1);
    p8_closure(state, "rect", p8_rectangle, false); p8_closure(state, "rectfill", p8_rectangle, true);
    p8_closure(state, "circ", p8_circle, false); p8_closure(state, "circfill", p8_circle, true);
    p8_closure(state, "btn", p8_btn, false); p8_closure(state, "btnp", p8_btn, true);
    const char helpers[] =
        "flr=math.floor ceil=math.ceil abs=math.abs sqrt=math.sqrt min=math.min max=math.max "
        "function mid(first,second,third) return max(min(first,second),min(max(first,second),third)) end "
        "function sgn(value) if value<0 then return -1 else return 1 end end "
        "function cos(turn) return math.cos(turn*6.283185307) end "
        "function sin(turn) return -math.sin(turn*6.283185307) end "
        "function atan2(horizontal,vertical) return (math.atan(-vertical,horizontal)/6.283185307)%1 end "
        "function rnd(limit) if type(limit)=='table' then if #limit==0 then return nil end return limit[math.random(#limit)] end "
        "limit=limit or 1 assert(type(limit)=='number' and limit>=0 and limit<math.huge,'invalid rnd limit') return math.random()*limit end "
        "function srand(seed) math.randomseed(seed) end "
        "function add(values,value,index) if index then table.insert(values,index,value) else table.insert(values,value) end return value end "
        "function del(values,value) for index=1,#values do if values[index]==value then return table.remove(values,index) end end end "
        "function deli(values,index) return table.remove(values,index or #values) end "
        "function all(values) local index,last=0,nil return function() if index==0 or values[index]==last then index=index+1 end last=values[index] return last end end "
        "function foreach(values,callback) for value in all(values) do callback(value) end end "
        "function count(values,wanted) local result=0 for _,value in pairs(values) do if wanted==nil or value==wanted then result=result+1 end end return result end "
        "function sub(text,first,last) return string.sub(text,first,last) end "
        "function tostr(value) return tostring(value) end function tonum(value) return tonumber(value) end";
    if (luaL_loadbufferx(state, helpers, sizeof(helpers)-1, "p8-api", "t") != LUA_OK) return lua_error(state);
    lua_call(state, 0, 0);
    if (luaL_loadbufferx(state, run->source, run->length, "cart.p8", "t") != LUA_OK) return lua_error(state);
    lua_call(state, 0, 0);
    if (p8_callback(state, "_init")) lua_call(state, 0, 0);
    if (p8_callback(state, "_update60")) { lua_pop(state, 1); run->pico->rate = 60; }
    return 0;
}
static int p8_frame(lua_State *state) {
    Pico *cart = p8(state);
    static const unsigned physical[] = {2, 3, 0, 1, 4, 5};
    cart->buttons = cart->pressed = 0;
    for (unsigned index = 0; index < 6; ++index) {
        if (!(session(state)->buttons & (1u << physical[index]))) { cart->held[index] = 0; continue; }
        cart->buttons |= 1u << index;
        unsigned scale = cart->rate / 30;
        if (cart->held[index] >= 15 * scale + 4 * scale) cart->held[index] -= 4 * scale;
        unsigned held = ++cart->held[index];
        if (held == 1 || (held > 15 * scale && (held - 1 - 15 * scale) % (4 * scale) == 0)) cart->pressed |= 1u << index;
    }
    if (p8_callback(state, cart->rate == 60 ? "_update60" : "_update")) lua_call(state, 0, 0);
    if (!session(state)->quit && p8_callback(state, "_draw")) lua_call(state, 0, 0);
    static const uint32_t palette[] = {0x000000,0x1d2b53,0x7e2553,0x008751,0xab5236,0x5f574f,0xc2c3c7,0xfff1e8,
        0xff004d,0xffa300,0xffec27,0x00e436,0x29adff,0x83769c,0xff77a8,0xffccaa};
    uint16_t output[16];
    for (unsigned index = 0; index < 16; ++index) {
        uint32_t rgb = palette[cart->display_palette[index]];
        output[index] = ((rgb >> 8) & 0xf800) | ((rgb >> 5) & 0x7e0) | ((rgb >> 3) & 31);
    }
    for (int row = 0; row < HEIGHT; ++row) for (int column = 0; column < WIDTH; ++column)
        session(state)->pixels[row * WIDTH + column] = column < 40 || column >= 280 ? 0 :
            output[cart->screen[(row * 128 / 240) * 128 + (column - 40) * 128 / 240]];
    return 0;
}
