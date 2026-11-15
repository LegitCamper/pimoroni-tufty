#include "bridge.h"
#include "portable.h"
#include <math.h>
#include "../vendor/skyemu/sb_types.h"
#define gb_tick_rtc sky_gb_tick_rtc
#include "../vendor/skyemu/gb.h"
#include "sky_input.h"

typedef struct { sb_emu_state_t emu; sb_gb_t gb; gb_scratch_t scratch; } Session;
_Static_assert(_Alignof(Session) <= 8, "Rust session alignment");

size_t tufty_gbc_size(void) { return sizeof(Session); }

bool tufty_gbc_init(void* memory, uint8_t* rom, size_t length, uint32_t mapper, uint32_t ram, bool pal) {
    if (length < 32768 || length > 4 * 1024 * 1024) return false;
    Session* state = memory;
    memset(state, 0, sizeof(*state));
    state->emu.rom_data = rom;
    state->emu.rom_size = length;
    memcpy(state->emu.rom_path, "game.gbc", 9);
    return sb_load_rom(&state->emu, &state->gb, &state->scratch);
}

bool tufty_gbc_frame(void* memory, uint32_t buttons, uint64_t seconds) {
    Session* state = memory;
    sky_input(&state->emu, buttons, seconds);
    sb_tick(&state->emu, &state->gb, &state->scratch);
    return state->emu.run_mode == SB_MODE_RUN;
}

void tufty_gbc_pixels(void* memory, unsigned row, uint16_t* output) {
    Session* state = memory;
    if (row >= 240) return;
    for (unsigned column = 0; column < 320; ++column) {
        uint16_t pixel = 0;
        if (column >= 27 && column < 293) {
            const uint8_t* color = &state->scratch.framebuffer[(row * 144 / 240 * 160 + (column - 27) * 160 / 266) * 4];
            pixel = rgb565(color[0], color[1], color[2]);
        }
        output[column] = pixel;
    }
}
