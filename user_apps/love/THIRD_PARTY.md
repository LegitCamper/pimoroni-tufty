# Lua provenance

- Lua 5.4.9, https://www.lua.org/ftp/lua-5.4.9.tar.gz
- Archive SHA-256: `2335b6c582a52654f94612bf10d2f4672805d05329aa6568b1d8cd9e5c6fb8e6`
- MIT license, reproduced in full in `vendor/lua/lua.h`; upstream copyright
  notices retained throughout vendored sources.
- Only the files listed in `native/sources.txt` compile into the runner.

Local changes:

- `luaconf.h`: allow build to select 32-bit Lua integers/floats.
- `lbaselib.c`: remove file loading, dynamic compilation, error-catching,
  manual GC, metatable access and console output from the registration table.
  Unused upstream implementations are removed by section garbage collection.
- `lmathlib.c`: deterministic default seed; no host clock/address seed.
- `lstrlib.c`: omit native pattern-matching functions; bound repeat counts.
- `ltablib.c`: omit unbounded sparse-table `move` loops.
- `native/config.h`: 40 nested C calls, deterministic VM hash/sort seeds.

The custom allocator, bindings, demo and host tests are project code, not copied
from desktop LÖVE. No LÖVE/SDL sources or commercial game assets are bundled.
ARM libc/libm come from the existing GNU Arm toolchain; its newlib/FDLIBM license
notices are retained in `NEWLIB-LICENSE` and `LIBGLOSS-LICENSE`.
