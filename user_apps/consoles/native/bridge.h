#pragma once
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>

static inline uint16_t rgb565(uint8_t red, uint8_t green, uint8_t blue) {
    return ((red >> 3) << 11) | ((green >> 2) << 5) | (blue >> 3);
}

#define DECLARE_CORE(name) \
    size_t tufty_##name##_size(void); \
    bool tufty_##name##_init(void*, uint8_t*, size_t, uint32_t, uint32_t, bool); \
    bool tufty_##name##_frame(void*, uint32_t, uint64_t); \
    void tufty_##name##_pixels(void*, unsigned, uint16_t*)

DECLARE_CORE(nes);
DECLARE_CORE(snes);
DECLARE_CORE(gbc);
DECLARE_CORE(gba);
