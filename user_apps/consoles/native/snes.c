#include "bridge.h"
#include "../vendor/lakesnes/snes.h"
#include "../vendor/lakesnes/spc.h"
#include "../vendor/lakesnes/dsp.h"

typedef struct {
    Snes snes;
    Cpu cpu;
    Apu apu;
    Spc spc;
    Dsp dsp;
    Dma dma;
    Ppu ppu;
    Cart cart;
    Input input1, input2;
    uint8_t ram[128 * 1024];
} Session;
_Static_assert(_Alignof(Session) <= 8, "Rust session alignment");

size_t tufty_snes_size(void) { return sizeof(Session); }

bool tufty_snes_init(void* memory, uint8_t* rom, size_t length, uint32_t mapper, uint32_t ram, bool pal) {
    if ((mapper != 1 && mapper != 2) || ram > 128 * 1024 || length < 32768 || (length & (length - 1))) return false;
    Session* state = memory;
    memset(state, 0, sizeof(*state));
    Snes* snes = &state->snes;
    snes->cpu = &state->cpu;
    snes->apu = &state->apu;
    snes->dma = &state->dma;
    snes->ppu = &state->ppu;
    snes->cart = &state->cart;
    snes->input1 = &state->input1;
    snes->input2 = &state->input2;
    state->cpu.mem = snes;
    state->cpu.read = snes_cpuRead;
    state->cpu.write = snes_cpuWrite;
    state->cpu.idle = snes_cpuIdle;
    state->apu.snes = snes;
    state->apu.spc = &state->spc;
    state->apu.dsp = &state->dsp;
    state->spc.mem = &state->apu;
    state->spc.read = apu_spcRead;
    state->spc.write = apu_spcWrite;
    state->spc.idle = apu_spcIdle;
    state->dsp.apu = &state->apu;
    state->dma.snes = snes;
    state->ppu.snes = snes;
    state->cart.snes = snes;
    state->input1.snes = state->input2.snes = snes;
    state->input1.type = state->input2.type = 1;
    state->cart.type = mapper;
    state->cart.rom = rom;
    state->cart.romSize = length;
    state->cart.ram = state->ram;
    state->cart.ramSize = ram;
    ppu_setPixelOutputFormat(&state->ppu, ppu_pixelOutputFormatXBGR);
    snes_reset(snes, true);
    snes->palTiming = pal;
    return true;
}

bool tufty_snes_frame(void* memory, uint32_t buttons, uint64_t seconds) {
    Session* state = memory;
    const uint32_t mapping[12] = {
        1 << 5, 1 << 8, 1 << 10, (1 << 6) | (1 << 9),
        1, 2, 4, 8, 1 << 4, 1 << 7,
        (1 << 6) | (1 << 7), (1 << 6) | (1 << 8),
    };
    uint16_t input = 0;
    for (unsigned index = 0; index < 10; ++index) {
        if (buttons & mapping[index]) input |= 1 << index;
    }
    if ((buttons & mapping[10]) == mapping[10]) input |= 1 << 10;
    if ((buttons & mapping[11]) == mapping[11]) input |= 1 << 11;
    if (input & 0xc00) input &= ~((1 << 3) | (1 << 1) | (1 << 9));
    state->input1.currentState = input;
    snes_runFrame(&state->snes);
    return true;
}

void tufty_snes_pixels(void* memory, unsigned row, uint16_t* output) {
    Session* state = memory;
    if (row >= 240) return;
    Ppu* ppu = &state->ppu;
    unsigned top = ppu->frameOverscan ? 1 : 8;
    unsigned height = ppu->frameOverscan ? 239 : 224;
    for (unsigned column = 0; column < 320; ++column) {
        uint16_t pixel = 0;
        if (column >= 32 && column < 288 && row >= top && row - top < height) {
            unsigned source_row = row - top + (ppu->evenFrame ? 0 : 239);
            const uint8_t* color = &ppu->pixelBuffer[source_row * 2048 + (column - 32) * 8 + 4];
            pixel = rgb565(color[2], color[1], color[0]);
        }
        output[column] = pixel;
    }
}
