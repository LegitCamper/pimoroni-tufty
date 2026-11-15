#include "bridge.h"
#include <stdio.h>
#include <stdlib.h>
#undef NDEBUG
#include <assert.h>

void tufty_gbc_mapper_checks(void);

static void exercise(const char* name, size_t size,
    bool (*init)(void*, uint8_t*, size_t, uint32_t, uint32_t, bool),
    bool (*frame)(void*, uint32_t, uint64_t), void (*pixels)(void*, unsigned, uint16_t*),
    uint8_t* rom, size_t length, unsigned mapper, uint16_t expected) {
    assert(size > 0 && size < 2 * 1024 * 1024);
    for (unsigned launch = 0; launch < 3; ++launch) {
        void* state = calloc(1, size);
        assert(state);
        assert(init(state, rom, length, mapper, 0, false));
        for (unsigned count = 0; count < 4; ++count) assert(frame(state, 0, count));
        uint16_t row[322] = {0};
        row[0] = row[321] = 0xcafe;
        pixels(state, 120, &row[1]);
        assert(row[0] == 0xcafe && row[321] == 0xcafe);
        printf("%s: %zu bytes, center=%04x\n", name, size, row[161]);
        assert(row[161] == expected);
        for (unsigned line = 0; line < 240; ++line) pixels(state, line, &row[1]);
        assert(row[0] == 0xcafe && row[321] == 0xcafe);
        free(state);
    }
}

int main(void) {
    tufty_gbc_mapper_checks();
    uint8_t* rom = calloc(1, 32784);
    assert(rom);
    memcpy(rom, "NES\x1a", 4);
    rom[4] = 1;
    const uint8_t nes_program[] = {
        0x78,0xd8,0xa9,0,0x8d,0,0x20,0x8d,1,0x20,
        0xa9,0x3f,0x8d,6,0x20,0xa9,0,0x8d,6,0x20,
        0xa9,0x21,0x8d,7,0x20,0xa9,0,0x8d,6,0x20,0x8d,6,0x20,
        0xa9,0x0a,0x8d,1,0x20,0x4c,0x26,0x80,
    };
    memcpy(rom + 16, nes_program, sizeof(nes_program));
    rom[16 + 0x3ffd] = 0x80;
    exercise("NES", tufty_nes_size(), tufty_nes_init, tufty_nes_frame, tufty_nes_pixels,
        rom, 16400, 0, rgb565(0x3c, 0xbc, 0xfc));

    memset(rom, 0, 32784);
    const uint8_t snes_program[] = {
        0x78,0xe2,0x20,0xa9,0x80,0x8d,0,0x21,0x9c,0x21,0x21,
        0xa9,0x1f,0x8d,0x22,0x21,0xa9,0,0x8d,0x22,0x21,
        0xa9,0x0f,0x8d,0,0x21,0x80,0xfe,
    };
    memcpy(rom, snes_program, sizeof(snes_program));
    rom[0x7fd5] = 0x20;
    rom[0x7fd7] = 5;
    rom[0x7fdc] = rom[0x7fdd] = 255;
    rom[0x7ffd] = 0x80;
    exercise("SNES", tufty_snes_size(), tufty_snes_init, tufty_snes_frame, tufty_snes_pixels,
        rom, 32768, 1, 0xf800);

    memset(rom, 0, 32784);
    rom[0x100] = 0xc3; rom[0x101] = 0x50; rom[0x102] = 1;
    rom[0x143] = 0x80;
    for (unsigned index = 0x134; index <= 0x14c; ++index) rom[0x14d] -= rom[index] + 1;
    const uint8_t gbc_program[] = {
        0x3e,0x80,0xe0,0x68,0x3e,0x1f,0xe0,0x69,0x3e,0,0xe0,0x69,0x18,0xfe,
    };
    memcpy(rom + 0x150, gbc_program, sizeof(gbc_program));
    exercise("GBC", tufty_gbc_size(), tufty_gbc_init, tufty_gbc_frame, tufty_gbc_pixels,
        rom, 32768, 0, 0xf800);

    memset(rom, 0, 32784);
    const uint32_t branch = 0xea00002e;
    memcpy(rom, &branch, 4);
    rom[0xb2] = 0x96;
    rom[0xbd] = (uint8_t)(-0x96 - 0x19);
    const uint32_t gba_program[] = {
        0xe3a00301,0xe3a01c04,0xe2811003,0xe1c010b0,
        0xe3a00406,0xe3a0101f,0xe3a02c96,
        0xe0c010b2,0xe2522001,0x1afffffc,0xeafffffe,
    };
    memcpy(rom + 0xc0, gba_program, sizeof(gba_program));
    exercise("GBA", tufty_gba_size(), tufty_gba_init, tufty_gba_frame, tufty_gba_pixels,
        rom, 1024, 0, 0xf800);
    free(rom);
}
