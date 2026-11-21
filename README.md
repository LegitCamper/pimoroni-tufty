# Tufty OS

Game-focused, bare-metal Rust OS for **Pimoroni Tufty 2350 only**. Forked from
`picocalc-os-rs` commit `0e270f5e679a8768cfe907576cd3ec60affef9ca`.
The original checkout is unchanged. Project crates are `tufty-kernel`, `tufty_sys`
(ABI), and `tufty` (app SDK).

**Status: build-tested port, not hardware-validated firmware.** No badge was
flashed or exercised during development. Display orientation/timing, PSRAM,
controller hot-plugging, and radio behavior still need bench testing.

## Hardware corrections

This is the newer RP2350B Tufty, **not Tufty 2040**. It has 16 MiB flash, 8 MiB
PSRAM, and a 320x240 ST7789 on an **8-bit parallel bus**, not PicoCalc's SPI panel.
The onboard radio is **CYW43439/CYW43, not ESP32**. There is no onboard SD socket
or speaker/audio output. The directional gamepad is the optional **Qw/ST Pad**
TCA9555 accessory; five front buttons are built in.

Verified against primary sources:
- [Tufty board definition](https://github.com/pimoroni/tufty2350/blob/main/board/pimoroni_tufty2350.h)
- [Tufty pin map](https://github.com/pimoroni/tufty2350/blob/main/board/pins.csv)
- [Badgeware hardware overview](https://badgewa.re/docs/introduction/meet-your-badge.md)
- [Pimoroni parallel ST7789 implementation](https://github.com/pimoroni/badgeware-cpp/blob/main/board/tufty/st7789.cpp)
- [Official Qw/ST Pad driver](https://github.com/pimoroni/qwstpad-micropython/blob/main/src/qwstpad.py)

| Function | Pins / implementation |
| --- | --- |
| UP, DOWN, A, B, C, HOME | GPIO 11, 6, 7, 9, 10, 22; active low, 20 ms debounce |
| RESET hold / front-button wake | GPIO 14 / 15; active low |
| Qw/ST Pad | I2C0 SDA 4 / SCL 5; first device at 0x21, 0x23, 0x25, 0x27 |
| Pad inputs | D-pad, A/B/X/Y, start/select; simultaneous held-state bitmask |
| Pad LEDs | Four active-low TCA9555 outputs; asynchronous requested-state API |
| LCD | CS 27, DC 28, WR 30, RD 31, data 32-39; PIO1/DMA0 |
| Backlight / peripheral power | GPIO 26 / 41 |
| PSRAM CS | GPIO 8; QMI mapped at 0x11000000 |
| Radio | ON 23, data/IRQ 24, CS 25, CLK 29; PIO0/DMA1 |

Kernel uses the HAL's 150 MHz default, not an unvalidated overclock. GPIO42/43
sensors, RTC, battery measurement and rear LEDs are not yet implemented.
Tap RESET to reboot; hold RESET for two seconds to request deep sleep. Press a
front button (UP, DOWN, A, B or C), or tap RESET, to boot back to the launcher.
Sleep is checked during reset startup, before apps or peripherals start; it does
not preserve a running game. The sleep/wake path is build-tested, not yet
hardware-validated. Badgeware's double-tap disk mode is not implemented.

## Included and removed

- Retains dual-core execution, validated PIE ELF loader, relocation checks,
  allocation tracking, pointer validation and framebuffer syscalls.
- Replaces keyboard MCU with debounced GPIO + hot-pluggable Qw/ST Pad input.
- Adds a paginated six-card launcher, titles, linked PNG icons and fallback art.
- Links **NES**, **SNES**, **Game Boy**, **Game Boy Color**, **Game Boy Advance**,
  **CHIP-8 / Super-CHIP** and Snake into kernel flash.
  These builtins launch without an app pack or ELF loading. Optional external
  apps still use the validated ELF loader.
- Shared ROM picker lives in `user_apps/selection_ui`: five cartridge rows,
  high-contrast mint selection, page/count indicators, clipped filenames,
  gamepad navigation, key repeat and an empty-library screen.
- Game Boy fits the whole picture into 266x240, centered, with nearest-neighbor
  scaling. No cropped 320x288 image. Simultaneous buttons use the new bitmask API.
- CHIP-8 includes an original animated demo when no ROMs are installed.
  `.ch8` selects classic quirks; `.sc8` selects Super-CHIP resolution, scrolling,
  16x16 sprites and RPL flags. This is not a claim of universal ROM compatibility.
- Removes calculator, gallery/GIF apps, keyboard backlight/battery-MCU code,
  SD/FAT/MTP stack, other board targets, unused PIO PSRAM experiments, Doom build
  tooling and picolibc checkout. WAV Player and Game Boy audio synthesis are
  removed. Required third-party licenses are retained.

Native apps run as **trusted code**, not isolated processes. Validation protects
syscall boundaries and rejects malformed files; it cannot sandbox malicious ARM
code. HOME is cooperative: a crashed or hung app may require RESET.

## Build and package

Requirements: Rust/rustup (pinned nightly in `rust-toolchain.toml`), `just`, Python
3, `arm-none-eabi-gcc`, ARM libc headers, and libclang for Game Boy's bindgen.
Host tests additionally require a C compiler with AddressSanitizer support.
Game Boy defaults to `/usr/arm-none-eabi/include`; set `ARM_INCLUDE` if different.
No picolibc build, Python packages, USB utilities or new flashing tool needed.

```sh
just test          # host ABI, assets, runtime memory, emulator, loader and pack tests
just kernel        # single RP2350B target, including builtin games
just apps          # optional standalone builds of the three games for ELF testing
just check         # tests, kernel, apps, formatting, ELF/icon/XIP placement validation
just image         # dist/tufty-os.uf2, kernel/builtins + ROM/external-app pack
```

`just image` builds files only; it does not flash hardware. It uses a checked
ELF-to-UF2 packer with RP2350 ARM secure family ID `0xe48bff59`. Older
`elf2uf2-rs` builds can emit the RP2040 ID and are deliberately not used.

**Before flashing, back up Badgeware files and firmware. This UF2 replaces the
firmware and writes its own app partition; do not expect the stock filesystem
to survive.** With a backup secured, hold HOME/BOOT while tapping RESET, then
copy `dist/tufty-os.uf2` to the RP2350 boot drive. No device was flashed here.

## Flash layout and games

| Region | Address | Size |
| --- | --- | --- |
| Kernel firmware, radio blob when enabled | 0x10000000 | 4 MiB reserved |
| Immutable app/file pack | 0x10400000 | 12 MiB |
| App data heap (PSRAM) | 0x11000000 | 8 MiB |

Put legally obtained ROMs beneath `dist/content`, then rerun `just image`:

```text
dist/content/
  apps/              # optional third-party .bin ELFs; no builtin copies required
  games/gameboy/     # .gb ROMs, monochrome Game Boy; not GBC
  games/nes/         # .nes, iNES 1.0 NTSC, mappers 0/1/2/4
  games/snes/        # .sfc / .smc, plain LoROM/HiROM
  games/gbc/         # .gbc / .gb, SkyEmu color/backward-compatible core
  games/gba/         # .gba, up to 4 MiB; experimental speed
  games/chip8/       # .ch8 / .sc8 ROMs
  games/love/        # single-file .lua games for LOVE Mini
  games/pico8/       # text .p8 carts for PICO-8 Mini
```

No commercial ROMs are bundled. Packs support 256 files, UTF-8 paths shorter than
256 bytes, and at most eight path components. The inherited SDK directory API
returns at most 25 entries per directory; keep each game folder within that limit.
Invalid offsets, duplicate/prefix-conflicting paths, unsupported metadata,
truncated PNGs and oversized image output are rejected.

**Storage is read-only. Game saves do not persist.** Writes return `Unsupported`;
the system never silently claims to save. Packaged `.sav` files can be read using
`/games/gameboy/saves/<ROM_TITLE>.sav`, but updates cannot be written back.
Cartridge RAM is limited to 32 KiB. Persistent
saves and USB upload require a separately tested flash-write design that safely
pauses both cores and preserves data across power loss. Rebuilding the pack is
the deliberately smaller alternative for now.

## Builtins and shared runtime SRAM

Builtins are ordinary Rust library dependencies linked into the firmware, not
ELF files embedded as bytes by `build.rs`. Code and immutable constants execute
or read directly from XIP flash. Embedding a relocatable ELF blob alone would
not make it executable in place. The registry in `kernel/src/builtin.rs` holds
titles, PNG bytes and entry functions. Core1 runs one selected entry at a time.

The previous 250 KiB `ARENA` was the kernel's global heap, not an exclusive app
slab. It is now split into a **48 KiB kernel heap** and **200 KiB app arena**:

- Core0 allocations stay in the kernel heap, independent of app lifetime.
- Builtin core1 allocations use the app arena. Game Boy's context and cartridge
  RAM are allocated at launch, not permanently reserved in kernel `.bss`.
  CHIP-8 state initializes in place too, avoiding large stack temporaries. NES
  allocates its roughly 83 KiB native state in this same reusable SRAM arena.
- ELF code/data/BSS is loaded into that same app arena (200 KiB image ceiling,
  subject to alignment and free capacity); standalone SDK heap allocations
  retain their existing PSRAM behavior.
- Large builtin ROM byte buffers use `tufty::PsramBuffer` explicitly. Default
  builtin allocations never silently spill into PSRAM, keeping exclusive/CAS
  atomics in SRAM. All four new consoles keep ROMs in PSRAM; SNES/GBC/GBA also
  put their larger plain C state and framebuffers there. Only one session is live. GBA's
  decode tables are part of that session, not a permanent SRAM reservation.
  A 2.5 KiB SRAM strip transfers four scanlines at a time. No second full-screen
  conversion buffer or separate per-emulator heap is reserved.
- Normal return drops state, clears Game Boy callbacks/display ownership,
  reclaims remaining tracked app allocations, then releases any ELF image.
  An SRAM-empty assertion guards repeated launches. Kernel memory is never reset.

This is a reusable variable-size arena, not fixed-size slab slots. The arena's
backing SRAM is reserved at boot; its allocations exist only while an app runs.
Native apps are trusted and must not retain pointers or background callbacks
after return. Resetting a hung app or providing process isolation is out of scope.
The 32 KiB core1 stack and 150 KiB display framebuffer remain separate reservations.

`just image` no longer copies builtin ELF files into `dist/content/apps`. Remove
old generated copies there if migrating an existing pack, otherwise they appear
as separate external apps. `just apps` remains useful for testing loader support.
The new `consoles` library is builtin-only: its combined code exceeds the 200 KiB
ELF arena. `just apps` still builds standalone GB, CHIP-8 and Snake, not fake or
unloadable console ELFs.

## Lua / LOVE Mini

The launcher also includes a small Lua 5.4 game runner with LÖVE-style
load/update/draw callbacks, button events, basic shapes and text. The original
**Orbit Dodge** demo is bundled. Put additional single-file scripts in
`dist/content/games/love/*.lua` and rebuild with `just image`.

This is **not full desktop LÖVE compatibility**: no `.love` archives, images,
audio, shaders, filesystem, network or physics APIs. Lua uses a bounded 512 KiB
PSRAM heap and a PSRAM framebuffer; instruction and drawing budgets stop
runaway scripts. See `user_apps/love/README.md` for API, limits and controls.

## PICO-8 Mini

PICO-8 Mini reuses the same bounded Lua runner. It loads text `.p8` carts that
use graphics, maps, flags, palettes and buttons. It is not official PICO-8 and
not full PICO-8 emulation. It does not support PNG carts, audio,
memory-poke APIs or PICO-only syntax such as `+=`. See
`user_apps/love/PICO8.md`.
No badge execution or FPS has been validated.

## Console compatibility

These are real native CPU/PPU cores, not placeholder launchers. **No badge FPS or
commercial-game compatibility is claimed. SNES and GBA are experimental and may
be too slow for useful play at the stock clock.** Host smoke tests execute original
tiny programs, check rendered pixels, and repeatedly release/recreate each core
under AddressSanitizer. Hardware testing remains required.

| System | Core | Admitted cartridge subset |
| --- | --- | --- |
| NES | agnes | NTSC iNES 1.0; mappers 0/1/2/4; power-of-two PRG/CHR banks; no trainers, NES 2.0 or four-screen mirroring |
| SNES | LakeSnes | 32 KiB-4 MiB power-of-two LoROM/HiROM, optional 512-byte copier header, up to 128 KiB cartridge RAM; no SA-1, Super FX, DSP or other enhancement chips |
| GB/GBC | SkyEmu | Header-size/checksum checked, up to 4 MiB, ROM-only/MBC1/MBC2/MBC3/MBC5; no SGB borders, link cable, camera or motion cartridges |
| GBA | SkyEmu | Header-checksummed, four-byte-aligned ROMs up to 4 MiB; larger games rejected rather than overcommitting PSRAM |

GB/GBC skips proprietary boot ROMs. GBA uses the MIT Cult-of-GBA replacement BIOS
included upstream; no Nintendo BIOS is bundled. New cores keep cartridge RAM only
for the current session: **saves disappear on exit**, with acknowledgement before
launch. RTC is synthetic, not battery-backed or synchronized. No audio output;
guest sound timers and the SNES APU still run where games depend on them.
See `user_apps/consoles/THIRD_PARTY.md` for pinned commits, licenses and local patches.

NES is centered at 256x240. SNES reduces its 512-wide output to 256x240, retaining
letterboxing and one interlaced field. GBC scales to 266x240; GBA to 320x213.
No arbitrary BIOS discovery, save-state UI, cheats or per-game settings included.

## Controls and app API

Badge UP/DOWN adjust backlight brightness by 10 percentage points per press,
between 10% and 100%. Holding does not repeat; pressing both does nothing.
Brightness starts at 100% after reboot and changes even on a static screen.
The backlight stays off until the startup black frame is complete.

Badge A/B/C reach userland as `Buttons::BADGE_A`, `BADGE_B`, and `BADGE_C` through
`buttons()`. Apps may handle or ignore them; the kernel assigns no actions or
legacy key events. Badge A/B no longer alias controller A/B. `BADGE_C` aliases
the existing `C` bit for compatibility with app-defined C actions. Badge
UP/DOWN never reach app input. Dedicated HOME remains `Buttons::HOME` (or Esc
through `get_key()`); app exit is cooperative, not forced.

Launcher: controller UP/DOWN/LEFT/RIGHT browse; controller A launches. Snake uses
the controller D-pad (A/B remain left/right aliases). Game Boy uses controller
A/B for actions, badge C or controller START for Start, and SELECT for Select.
Dedicated HOME exits; controller X no longer aliases HOME.

The controller's `+` button is `Buttons::START`; `-` is `Buttons::SELECT`.
Both reach apps independently, including when held together. The legacy
`get_key()` adapter emits Tab for `+` and Del for `-`; neither adjusts brightness
or aliases HOME. Each app decides what Start/Select do.

Shared ROM picker: controller UP/DOWN moves, A/START launches, B/HOME backs out,
badge C or controller RIGHT pages forward and LEFT pages back. New console games use pad directions, A/B, C/START
and SELECT. SNES also uses X/Y; hold C+X for L or C+Y for R (chord suppresses Start
and face actions). GBA maps X/Y to L/R. HOME or START+SELECT returns to launcher.
Qw/ST Pad is required for launcher navigation and directional game controls.

CHIP-8 maps UP/DOWN/LEFT/RIGHT to 2/8/4/6, A/B to 5/0, X/Y to 1/3,
C or START to F and SELECT to C. HOME or START+SELECT exits. Games requiring the
other six keypad keys need configurable mapping, which is not yet implemented.

```rust
use tufty::{Buttons, buttons, gamepad_leds};

let held = buttons();
if held.contains(Buttons::LEFT | Buttons::A) {
    // Move and fire in the same frame.
}
let _ = gamepad_leds(0b0101);
```

`get_key()` remains an event adapter for ported apps, under `tufty::input`, not
a keyboard driver. Games should use `buttons()` for held/simultaneous input so
event queue overflow cannot leave a button stuck. Syscall slots are append-only;
this fork has 23 slots and requires rebuilding all apps. The badge input split
adds button bits without adding or renumbering syscall slots; rebuild apps to
use the new `BADGE_A` and `BADGE_B` flags.

## PNG icon embedded in an app

```rust
tufty::app_metadata!("My Game", "../assets/icon.png");
// Or explicitly no icon:
// tufty::app_metadata!("My Game");
```

Use exactly one macro at module scope. It emits a pointer-free `#[repr(C)]`
record into the retained `.tufty_app` ELF section, outside loaded app memory.
The enum is `IconKind::{None = 0, Png = 1}`. Header layout: magic `TUFTYAPP`
(8 bytes), NUL-terminated UTF-8 title (32), little-endian kind (4), PNG length
(4), then PNG bytes. The launcher looks up the named ELF section, never scans
arbitrary code for a magic string. Missing/invalid metadata falls back to filename
and a generated gamepad tile. PNG failure does not prevent launching an app.

PNG support: noninterlaced 8-bit RGB or RGBA, all five filters, CRC checks,
at most 48x48 pixels and 32 KiB compressed file size. Palette/grayscale/interlaced
PNGs and tRNS are unsupported; export RGBA instead. Decompression output is
bounded before allocation. `python3 tools/icons.py` regenerates the original
32x32 icons included here.

## Wireless and audio limitations

```sh
just wifi    # fetch hash-verified firmware, then build optional CYW43 support
python3 tools/pack.py dist/content dist/tufty-wifi.uf2 \
  --kernel target/thumbv8m.main-none-eabihf/release/tufty-kernel
```

Do not run `just image` between these commands: it builds the radio-off default
kernel. The radio is off in default builds to reduce power and flash footprint.

`tufty::wifi_scan()` starts a bounded asynchronous scan. Poll
`tufty::device_status() & tufty::wireless::WIFI_SCANNING`, then query indices
0..16 with `tufty::wifi_network(index)` until `NotFound`. `Busy`, `Io`, and
`Unsupported` are explicit errors. SSIDs are raw bytes with a bounded length,
not assumed UTF-8. Firmware readiness is **not network connectivity**.
Association, DHCP, sockets, Bluetooth, multiplayer and OTA are not implemented.

WAV Player, WAV parser and Game Boy audio processing are removed. Reserved audio
syscall slots remain in place for ABI compatibility and return `Unsupported`
(readiness is false). Game Boy runs silently. CHIP-8 sound timers still run but
do not produce sound. No audio pins or output driver are configured.

## Bench validation still needed

1. Boot with no pack; builtin cards and the CHIP-8 demo must remain available.
2. Confirm full-screen orientation, RGB order, backlight, and HOME/RESET behavior.
3. Check badge UP/DOWN brightness limits and reboot default; verify badge A/B/C
   are distinct from pad actions, dedicated HOME exits, and pad hot-plug works.
4. Run Snake, bundled CHIP-8 demo, and legally obtained GB, NES, SNES, GBC and GBA
   cartridges within the documented limits. Measure speed; do not assume 60 FPS.
5. Alternate builtin/ELF launches, including missing/invalid ROM errors; verify
   display ownership resets, arena reuse, and PSRAM cleanup.
6. With the wifi build, verify scan results and timeout behavior.

## Attribution

Original kernel/SDK: [LegitCamper/picocalc-os-rs](https://github.com/LegitCamper/picocalc-os-rs),
license in `LICENSE`. Vendored Peanut-GB subset comes from commit
`c13c99cd967d72dbd9e600795cd66169ca3945c7`; see `LICENSE`. The removed minigb APU's
license is retained for provenance. QMI PSRAM code retains its upstream attribution. Radio firmware revision
and SHA-256 hashes are pinned in `tools/firmware.py`; its separate binary license
is retained in `firmware/`. No upstream repository was modified.
