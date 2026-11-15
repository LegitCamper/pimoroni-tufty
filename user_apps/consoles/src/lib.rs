#![no_std]

extern crate alloc;

use alloc::vec::Vec;
use core::ffi::c_void;
use selection_ui::SelectionUi;
use tufty::{
    Buttons, PsramBuffer,
    cartridge::{Console, MAX_ROM, validate},
    display::Display,
};

struct Core {
    console: Console,
    title: &'static str,
    directory: &'static str,
    extensions: &'static [&'static str],
    size: unsafe extern "C" fn() -> usize,
    init: unsafe extern "C" fn(*mut c_void, *mut u8, usize, u32, u32, bool) -> bool,
    frame: unsafe extern "C" fn(*mut c_void, u32, u64) -> bool,
    pixels: unsafe extern "C" fn(*mut c_void, u32, *mut u16),
}

macro_rules! core {
    ($kind:ident, $title:literal, $path:literal, $extensions:expr, $size:ident, $init:ident, $frame:ident, $pixels:ident) => {{
        unsafe extern "C" {
            fn $size() -> usize;
            fn $init(
                state: *mut c_void,
                rom: *mut u8,
                len: usize,
                mapper: u32,
                ram: u32,
                pal: bool,
            ) -> bool;
            fn $frame(state: *mut c_void, buttons: u32, seconds: u64) -> bool;
            fn $pixels(state: *mut c_void, row: u32, output: *mut u16);
        }
        Core {
            console: Console::$kind,
            title: $title,
            directory: $path,
            extensions: $extensions,
            size: $size,
            init: $init,
            frame: $frame,
            pixels: $pixels,
        }
    }};
}

static CORES: [Core; 4] = [
    core!(
        Nes,
        "NES",
        "/games/nes",
        &["nes"],
        tufty_nes_size,
        tufty_nes_init,
        tufty_nes_frame,
        tufty_nes_pixels
    ),
    core!(
        Snes,
        "SNES",
        "/games/snes",
        &["sfc", "smc"],
        tufty_snes_size,
        tufty_snes_init,
        tufty_snes_frame,
        tufty_snes_pixels
    ),
    core!(
        GameBoyColor,
        "GAME BOY COLOR",
        "/games/gbc",
        &["gbc", "gb"],
        tufty_gbc_size,
        tufty_gbc_init,
        tufty_gbc_frame,
        tufty_gbc_pixels
    ),
    core!(
        GameBoyAdvance,
        "GAME BOY ADVANCE",
        "/games/gba",
        &["gba"],
        tufty_gba_size,
        tufty_gba_init,
        tufty_gba_frame,
        tufty_gba_pixels
    ),
];

pub fn nes() {
    launch(&CORES[0]);
}
pub fn snes() {
    launch(&CORES[1]);
}
pub fn gbc() {
    launch(&CORES[2]);
}
pub fn gba() {
    launch(&CORES[3]);
}

pub fn run() {
    let selected = {
        let Some(mut display) = Display::take() else {
            return;
        };
        SelectionUi::new(&["NES", "SNES", "Game Boy Color", "Game Boy Advance"])
            .with_title("CONSOLE ARCADE")
            .run_selection_ui(&mut display)
    };
    if let Ok(Some(index)) = selected {
        launch(&CORES[index]);
    }
}

fn acknowledge(display: &mut Display, title: &str, message: &str) -> bool {
    selection_ui::notice(display, title, message);
    let mut previous = tufty::buttons();
    loop {
        let held = tufty::buttons();
        let pressed = held & !previous;
        previous = held;
        if pressed.intersects(Buttons::B | Buttons::HOME) {
            return false;
        }
        if pressed.contains(Buttons::A) {
            return true;
        }
        tufty::sleep(10);
    }
}

fn launch(core: &Core) {
    let Some(mut display) = Display::take() else {
        return;
    };
    let mut files = Vec::new();
    for extension in core.extensions {
        match tufty::fs::list_media(core.directory, extension) {
            Ok(mut found) => files.append(&mut found),
            Err(_) => {
                acknowledge(&mut display, "LIBRARY ERROR", "Unable to read content pack");
                return;
            }
        }
    }
    files.sort();
    files.dedup();
    let labels: Vec<&str> = files.iter().map(|path| path.as_str()).collect();
    let Ok(Some(index)) = SelectionUi::new(&labels)
        .with_title(core.title)
        .with_hint(core.directory)
        .run_selection_ui(&mut display)
    else {
        return;
    };
    if !acknowledge(
        &mut display,
        core.title,
        "Saves lost when app closes. No audio.\nQw/ST Pad needed for full controls.\nSNES/GBA speed is experimental.\nHOME exits. START+SELECT exits.\nGBA: X/Y = L/R. SNES: C+X/Y = L/R.",
    ) {
        return;
    }
    if let Err(message) = play(core, &files[index], &mut display) {
        acknowledge(&mut display, "CARTRIDGE ERROR", message);
    }
}

fn play(core: &Core, path: &str, display: &mut Display) -> Result<(), &'static str> {
    let length = tufty::fs::file_len(path).map_err(|_| "Cannot read cartridge size")?;
    if length == 0 || length > MAX_ROM + 512 {
        return Err("Cartridge must fit 4 MiB limit");
    }
    let mut rom_storage = PsramBuffer::new(length + 7).map_err(|_| "Not enough PSRAM for ROM")?;
    let rom_offset = rom_storage.as_mut_ptr().align_offset(8);
    let rom = &mut rom_storage[rom_offset..rom_offset + length];
    if tufty::fs::read_file(path, 0, rom) != Ok(length) {
        return Err("Short cartridge read");
    }
    let cartridge = validate(core.console, rom)?;
    // SAFETY: linked C core returns sizeof its plain-data session, without accessing memory.
    let size = unsafe { (core.size)() };
    if size == 0 || size > 2 * 1024 * 1024 {
        return Err("Core state exceeds 2 MiB budget");
    }
    let mut sram = Vec::<u64>::new();
    let mut state_storage;
    let state = if core.console == Console::Nes {
        let words = size.div_ceil(8);
        sram.try_reserve_exact(words)
            .map_err(|_| "Not enough app SRAM for NES")?;
        sram.resize(words, 0);
        sram.as_mut_ptr().cast::<c_void>()
    } else {
        state_storage =
            PsramBuffer::new(size + 7).map_err(|_| "Not enough PSRAM for emulator state")?;
        let offset = state_storage.as_mut_ptr().align_offset(8);
        state_storage[offset..offset + size]
            .as_mut_ptr()
            .cast::<c_void>()
    };
    let rom = &mut rom[cartridge.offset..];
    unsafe extern "C" {
        fn tufty_emu_clock(seconds: u64);
    }
    // SAFETY: core1 alone updates this plain clock value; no native code runs concurrently.
    unsafe {
        tufty_emu_clock(tufty::get_ms() / 1000);
    }
    // SAFETY: validated cartridge, aligned exclusive state and ROM live until all synchronous C calls finish.
    if !unsafe {
        (core.init)(
            state,
            rom.as_mut_ptr(),
            rom.len(),
            cartridge.mapper,
            cartridge.ram_size,
            cartridge.pal,
        )
    } {
        return Err("Core rejected cartridge");
    }
    let mut strip = [0u16; 320 * 4];
    let rate = if cartridge.pal { 50 } else { 60 };
    let mut deadline = tufty::get_ms() * rate;
    loop {
        let buttons = tufty::buttons();
        if buttons.intersects(Buttons::HOME) || buttons.contains(Buttons::START | Buttons::SELECT) {
            return Ok(());
        }
        // SAFETY: core owns only the live session and cartridge; no callbacks or concurrent access.
        if !unsafe { (core.frame)(state, buttons.bits(), tufty::get_ms() / 1000) } {
            return Err("Emulator stopped on invalid execution");
        }
        for top in (0..240).step_by(4) {
            for row in 0..4 {
                // SAFETY: row is below 240 and each output has capacity for exactly 320 u16 pixels.
                unsafe {
                    (core.pixels)(state, (top + row) as u32, strip[row * 320..].as_mut_ptr());
                }
            }
            display
                .blit(0, top as u16, 320, 4, &strip)
                .ok_or("Display transfer failed")?;
        }
        deadline += 1000;
        let now = tufty::get_ms();
        if deadline / rate > now {
            tufty::sleep(deadline / rate - now);
        } else if now - deadline / rate > 100 {
            deadline = now * rate;
        }
    }
}
