#include "bridge.h"
#include "../vendor/agnes/agnes.c"
_Static_assert(_Alignof(agnes_t) <= 8, "Rust session alignment");

size_t tufty_nes_size(void) { return sizeof(agnes_t); }

bool tufty_nes_init(void* memory, uint8_t* rom, size_t length, uint32_t mapper, uint32_t ram, bool pal) {
    agnes_t* state = memory;
    memset(state, 0, sizeof(*state));
    memset(state->ram, 255, sizeof(state->ram));
    return agnes_load_ines_data(state, rom, length);
}

bool tufty_nes_frame(void* memory, uint32_t buttons, uint64_t seconds) {
    agnes_t* state = memory;
    agnes_input_t input = {
        .a = buttons & (1 << 4), .b = buttons & (1 << 5),
        .select = buttons & (1 << 10), .start = buttons & ((1 << 6) | (1 << 9)),
        .up = buttons & 1, .down = buttons & 2, .left = buttons & 4, .right = buttons & 8,
    };
    agnes_set_input(state, &input, NULL);
    bool frame = false;
    for (unsigned count = 0; count < 100000 && !frame; ++count) {
        if (!agnes_tick(state, &frame)) return false;
    }
    return frame;
}

void tufty_nes_pixels(void* memory, unsigned row, uint16_t* output) {
    if (row >= 240) return;
    for (unsigned column = 0; column < 320; ++column) {
        uint16_t pixel = 0;
        if (column >= 32 && column < 288) {
            agnes_color_t color = agnes_get_screen_pixel(memory, column - 32, row);
            pixel = rgb565(color.r, color.g, color.b);
        }
        output[column] = pixel;
    }
}
