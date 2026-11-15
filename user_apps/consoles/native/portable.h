#pragma once
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <time.h>
#include <ctype.h>
#define SE_AUDIO_SAMPLE_RATE 48000
#define TUFTY_SILENT 1

extern uint64_t tufty_emu_seconds;
time_t tufty_emu_time(time_t* output);
struct tm* tufty_emu_localtime(const time_t* seconds);

// ponytail: no host filesystem or debug logs; add explicit SDK save imports when storage is writable.
#define fopen(...) ((FILE*)0)
#define fclose(...) (0)
#define fread(...) ((size_t)0)
#define fwrite(...) ((size_t)0)
#define fseek(...) (-1)
#define ftell(...) (-1L)
#define printf(...) (0)
#define fprintf(...) (0)
#define snprintf(...) (0)
#define time tufty_emu_time
#define localtime tufty_emu_localtime
#undef tolower
#define tolower(value) (((value) >= 'A' && (value) <= 'Z') ? (value) + ('a' - 'A') : (value))
