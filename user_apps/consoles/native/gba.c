#include "bridge.h"
#include "portable.h"
#include "../vendor/skyemu/gba.h"
#include "sky_input.h"

typedef struct { sb_emu_state_t emu; gba_t gba; gba_scratch_t scratch; } Session;
_Static_assert(_Alignof(Session) <= 8, "Rust session alignment");

size_t tufty_gba_size(void) { return sizeof(Session); }

bool tufty_gba_init(void* memory, uint8_t* rom, size_t length, uint32_t mapper, uint32_t ram, bool pal) {
    if (length < 192 || length > 4 * 1024 * 1024 || length % 4) return false;
    Session* state = memory;
    memset(state, 0, sizeof(*state));
    state->emu.rom_data = rom;
    state->emu.rom_size = length;
    memcpy(state->emu.rom_path, "game.gba", 9);
    return gba_load_rom(&state->emu, &state->gba, &state->scratch);
}

bool tufty_gba_frame(void* memory, uint32_t buttons, uint64_t seconds) {
    Session* state = memory;
    sky_input(&state->emu, buttons, seconds);
    gba_tick(&state->emu, &state->gba, &state->scratch);
    return state->emu.run_mode == SB_MODE_RUN;
}

void tufty_gba_pixels(void* memory, unsigned row, uint16_t* output) {
    Session* state = memory;
    if (row >= 240) return;
    for (unsigned column = 0; column < 320; ++column) {
        uint16_t pixel = 0;
        if (row >= 13 && row < 226) {
            const uint8_t* color = &state->scratch.framebuffer[((row - 13) * 160 / 213 * 240 + column * 240 / 320) * 4];
            pixel = rgb565(color[0], color[1], color[2]);
        }
        output[column] = pixel;
    }
}
