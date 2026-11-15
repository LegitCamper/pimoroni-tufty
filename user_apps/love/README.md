# Lua / LOVE Mini

Builtin-only, single-script 2D game runner for Tufty 2350. Uses Lua 5.4.9,
not LuaJIT or the desktop LÖVE engine. Existing LÖVE games need porting to this
small API subset; arbitrary `.love` files will not run.

## Launch

Choose **Lua / LOVE Mini**, then **ORBIT DODGE** for the original bundled demo.
UP/DOWN move; A starts/retries; HOME returns to the OS. No external files needed.

For your own game, place one UTF-8 script at
`dist/content/games/love/my-game.lua`, run `just image`, then install that image.
Each `.lua` file is a complete game, not a module or a `main.lua` directory.
The picker also discovers `.lua` files at the pack root. Maximum 64 KiB per
script; the existing directory listing limit is 25 entries per directory.
No Lua bytecode, ZIP `.love` packages, imports, assets or writable saves.

```lua
local position = 100
function love.update(delta)
    if love.keyboard.isDown("down") then position = position + 80 * delta end
    if love.keyboard.isDown("up") then position = position - 80 * delta end
    position = math.max(0, math.min(220, position))
end
function love.draw()
    love.graphics.setColor(0.3, 1, 0.7)
    love.graphics.rectangle("fill", 150, position, 20, 20)
    love.graphics.print("HELLO TUFTY", 10, 10)
end
```

## Supported API

| API | Supported form |
| --- | --- |
| Callbacks | `love.load()`, `love.update(dt)`, `love.draw()` |
| Input callbacks | `love.keypressed(key, scancode, false)`, `love.keyreleased(key, scancode)` |
| Input query | `love.keyboard.isDown(key, ...)`, 1..16 supported keys |
| Color | `love.graphics.setColor(r,g,b[,a])`, `getColor()` |
| Background | `setBackgroundColor(r,g,b[,a])`, `getBackgroundColor()`, `clear([r,g,b[,a]])` |
| Shapes | `rectangle("fill"/"line",x,y,w,h)`, `circle("fill"/"line",x,y,r)`, `line(x1,y1,x2,y2)` |
| Text | `print(text[,x[,y]])`, fixed 6x10 font, max 128 printable ASCII bytes/newlines |
| Screen | `getDimensions()`, `getWidth()`, `getHeight()`; fixed 320x240 |
| Timer | `love.timer.getTime()`, `getDelta()`, `getFPS()` |
| Exit | `love.event.quit()`; HOME is reserved by OS |

Graphics functions in the table live under `love.graphics`. Colors use 0..1;
alpha must be 1. No alpha blending, color-table overloads, transforms, images,
fonts, rounded rectangles, polylines, shaders, audio, physics, mouse, filesystem,
networking, Bluetooth, `love.conf`, custom `love.run`, or LuaJIT extensions.
Unsupported module/function access and invalid arguments produce an error screen.
Only the listed callbacks are invoked; extra user fields on `love` are allowed.

| Physical button | Key and scancode |
| --- | --- |
| UP / DOWN / LEFT / RIGHT | `up` / `down` / `left` / `right` |
| A / B / C | `z` / `x` / `c` |
| Pad X / Y | `a` / `s` |
| Pad START / SELECT | `return` / `backspace` |

No synthetic key-repeat events. Launch-held buttons are ignored until released.
LEFT/RIGHT and extra pad controls require the optional Qw/ST Pad. A/B/C and
UP/DOWN work without it. Error screens dismiss on a new A/B/HOME press.

## Runtime bounds

- Code stays in XIP flash. Each launch owns one roughly 664 KiB PSRAM session:
  a 512 KiB Lua/C-library allocator pool, 150 KiB RGB565 framebuffer, and metadata.
  Script file bytes also use PSRAM. No Lua allocations spill into app SRAM.
- ARM newlib numeric-conversion allocations share that same bounded pool.
  A session-local reentrancy record prevents retained libc pointers outliving
  the session. Only core1 uses this libc bridge; do not call allocating newlib
  functions from core0 or interrupts. Rust peripheral code does not use it.
- Only a 2.5 KiB row-transfer strip and launcher data use the existing app SRAM
  and core1 stack. All session storage is released on normal exit or Lua error.
- 200,000 Lua VM instructions per initialization/frame and 1,000,000 drawing
  pixel attempts per call. Infinite Lua loops terminate with a visible error.
  These are work budgets, not a hard realtime deadline; native library calls
  are not preempted. No hardware timing or security-isolation claim.
- Lua integer and float values are both 32-bit. Integers wrap at 32 bits; floats
  have about seven decimal digits. This differs from desktop Lua/LuaJIT.
- `math`, `string`, `table`, and a restricted base library are available.
  No `io`, `os`, `debug`, `package`, `require`, `coroutine`, dynamic `load`,
  `loadfile`, `dofile`, `pcall`, `xpcall`, `collectgarbage`, `getmetatable`,
  `setmetatable`, console `print` or `warn`. Error-catching, hooks and finalizers
  cannot be used to bypass the runner's VM budget. GC remains automatic.
- String pattern APIs (`find`, `match`, `gmatch`, `gsub`) are excluded because
  native backtracking is not covered by Lua instruction hooks. `table.move` is
  excluded and `string.rep` permits at most 65,536 repetitions, including empty
  strings, to prevent billion-iteration native loops with tiny inputs.
- Coordinates are finite and within +/-4096; sizes 0..4096; circle radius 0..512.
  Shapes clip to the screen. Display state persists, with background cleared
  before each draw. No antialiasing. Target cadence is about 30 Hz, not a
  measured performance claim. `dt` clamps to 1..100 ms; elapsed time sums those
  steps, and FPS is the reciprocal of the last step rather than an average.
- Lua errors/longjmp remain within C protected calls, never across Rust frames.
  The synchronous text callback only rasterizes into the provided buffer.

`just test` runs the real interpreter/adapter under host AddressSanitizer and
UndefinedBehaviorSanitizer, plus pure Rust source-validation tests. Tests cover
callback order, input edges, pixels/clipping, syntax/runtime errors, denied APIs,
instruction/drawing/heap limits, GC churn, repeat launches and the demo.
Host tests use host libc; ARM newlib integration is build-checked only.
Hardware execution, stack high-water measurement and badge FPS remain unverified.

Smallest path: port simple procedural games to this subset. Add bounded PNG
sprites and read-only asset loading only when a game needs them; full desktop
LÖVE/SDL is not part of this runner.
