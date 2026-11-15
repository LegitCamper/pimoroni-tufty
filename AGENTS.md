# Tufty OS

Single target: Pimoroni Tufty 2350 (RP2350B, 16 MiB flash, 8 MiB PSRAM).
Do not add PicoCalc, RP2040, SD-card, keyboard-controller or calculator support.
Use `just test`, `just kernel`, and `just apps`; apps require separate PIE flags.
Append syscalls only, update table count, and rebuild all apps after ABI changes.
Validate every user pointer, length, file offset, enum and asset at trust boundaries.
Pure logic belongs in `tufty_sys` for host tests. No HAL dependencies there.
Keep unsafe blocks documented with their safety invariant.
Do not claim hardware validation without a connected badge.
