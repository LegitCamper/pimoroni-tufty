# PICO-8 Mini

PICO-8 Mini runs a small, compatible subset of PICO-8 games. It is not the
official PICO-8 software and does not emulate full PICO-8 hardware. It reuses
the bounded Lua runner from LOVE Mini.

Put text carts in `dist/content/games/pico8/*.p8`, or in the root of the content
pack. Then rebuild with `just image`. The bundled demo is an original game
called Star Run.

## Controls

PICO buttons 0..5 map to LEFT, RIGHT, UP, DOWN, A and B. Without the optional
gamepad, use UP, DOWN, A and B. Pressing HOME always returns to the OS.
Player 1 and higher never report button presses.

## Supported subset

- Text `.p8` files up to 160 KiB, versions 1..42, with Lua first. Valid
  `__gfx__`, `__map__`, `__gff__`, `__sfx__`, `__music__` and `__label__`
  sections are accepted. Graphics, map and flag data decode to 0x3100 bytes.
  The lower half of the sprite sheet shares memory with map rows 32..63.
- Callbacks: `_init`, `_update` or `_update60`, and `_draw`.
- Drawing: `cls`, `color`, `pset`, `pget`, `line`, `rect`, `rectfill`, `circ`,
  `circfill`, `camera`, `clip`, `pal`, `palt`, `spr`, `sspr`, `print` and
  `cursor`.
- Sprites and maps: `sget`, `sset`, `map`, `mget`, `mset`, `fget` and `fset`.
- Input and time: `btn`, `btnp`, `time`, `t` and `stop`.
- Helpers: `flr`, `ceil`, `abs`, `sqrt`, `min`, `max`, `mid`, `sgn`, `sin`,
  `cos`, `atan2`, `rnd`, `srand`, `add`, `del`, `deli`, `all`, `foreach`,
  `count`, `sub`, `tostr` and `tonum`.
- `sfx` and `music` check their arguments but make no sound. The badge has no
  audio output.

The 128x128 screen is scaled to 240x240 and centered with black side bars.
`!=` becomes `~=`, except inside strings and comments. Other code must use
standard Lua 5.4 syntax.

## Not supported

These features are not supported: PNG carts, SPLORE or online downloads,
`peek`/`poke`/`memcpy`/`reload`, save data, audio, compound assignment
(`+=`), shorthand `if`/`print`, binary literals, extended palettes, palette
tables, `palt` bitfields, secondary palettes and 16.16 fixed-point math.
Integers and floats use 32 bits. Random numbers use Lua's random generator,
not PICO-8's exact sequence. Unsupported functions are reported as normal Lua
errors when a game calls them.

The memory, instruction and drawing limits are the same as in
[LOVE Mini](README.md). Games target 30 or 60 Hz. Frame rate on the badge has
not been measured.
