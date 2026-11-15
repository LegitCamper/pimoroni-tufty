#include "gbc.c"
#undef NDEBUG
#include <assert.h>

void tufty_gbc_mapper_checks(void) {
    uint8_t* rom = calloc(1, 32768);
    Session* state = calloc(1, sizeof(Session));
    assert(rom && state);
    rom[0x147] = 5;
    assert(tufty_gbc_init(state, rom, 32768, 0, 0, false));
    sb_gb_t* gb = &state->gb;
    assert(gb->cart.ram_size == 512);
    sb_store8(gb, 0x0000, 10);
    sb_store8(gb, 0x0100, 3);
    assert(gb->cart.ram_write_enable && gb->cart.mapped_rom_bank == 3);
    sb_store8(gb, 0xa123, 0xab);
    assert(sb_read8_direct(gb, 0xa323) == 0xfb);
    sb_store8(gb, 0x0000, 0);
    assert(sb_read8_direct(gb, 0xa123) == 0xff);
    gb->cart.mbc_type = SB_MBC_MBC5;
    gb->cart.ram_size = 2048;
    sb_store8(gb, 0x4000, 7);
    assert(gb->cart.mapped_ram_bank == 0);
    free(state);
    free(rom);
}
