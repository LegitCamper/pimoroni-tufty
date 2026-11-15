#include <stdbool.h>
#include <stdint.h>
#include <stddef.h>
#include <time.h>

uint64_t tufty_emu_seconds;

void tufty_emu_clock(uint64_t seconds) { tufty_emu_seconds = seconds; }

time_t tufty_emu_time(time_t* output) {
    time_t value = (time_t)(946684800 + tufty_emu_seconds);
    if (output) *output = value;
    return value;
}

struct tm* tufty_emu_localtime(const time_t* value) {
    static struct tm result;
    uint64_t seconds = *value < 0 ? 0 : (uint64_t)*value;
    uint64_t days = seconds / 86400;
    result.tm_sec = seconds % 60;
    result.tm_min = seconds / 60 % 60;
    result.tm_hour = seconds / 3600 % 24;
    result.tm_wday = (days + 4) % 7;
    unsigned year = 1970;
    for (;;) {
        unsigned leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
        if (days < 365 + leap) break;
        days -= 365 + leap;
        ++year;
    }
    result.tm_year = year - 1900;
    result.tm_yday = days;
    static const unsigned months[] = {31,28,31,30,31,30,31,31,30,31,30,31};
    unsigned month = 0;
    for (; month < 11; ++month) {
        unsigned count = months[month] + (month == 1 && year % 4 == 0 && (year % 100 != 0 || year % 400 == 0));
        if (days < count) break;
        days -= count;
    }
    result.tm_mon = month;
    result.tm_mday = days + 1;
    result.tm_isdst = 0;
    return &result;
}

bool se_load_bios_file(const char* name, const char* base, const char* file, uint8_t* data, size_t size) {
    return false;
}
