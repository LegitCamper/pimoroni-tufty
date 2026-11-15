#ifndef TUFTY_LOVE_RUNNER_H
#define TUFTY_LOVE_RUNNER_H
#include <stddef.h>
#include <stdint.h>

typedef void (*TuftyLoveText)(uint16_t *, const char *, size_t, int, int, uint16_t);
size_t tufty_love_size(void);
int tufty_love_init(void *memory, const char *source, size_t length, TuftyLoveText text);
int tufty_love_tick(void *memory, uint32_t buttons, float delta);
int tufty_pico8_init(void *memory, const char *source, size_t length, TuftyLoveText text,
                    const uint8_t *assets, size_t asset_length);
unsigned tufty_love_rate(void *memory);
const char *tufty_love_error(void *memory);
const uint16_t *tufty_love_pixels(void *memory);
#endif
